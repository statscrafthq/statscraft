//! Catalog binding of a transformation plan and its in-place apply to a dataset (In-Place
//! Transformation Binding 0.1 and 0.2): the same dataset and the same physical table, direct
//! UPDATE and, where DDL is transactional, ALTER TABLE ... ADD COLUMN, catalog updates and one
//! compact audit row, all in one transaction. Nothing is copied.

use crate::args;
use crate::catalog::{
    CatalogVariable, catalog_variables, find_dataset, physical_names, rollback, store_value_labels,
};
use crate::db::{Db, DbError, Dialect, Result, Sql, first, uuid};
use crate::plan::{
    Comparator, Connective, Match, Op, Operand, Outcome, PLAN_V01, Plan, Predicate, TargetMode,
    Typed, validate_plan,
};
use std::collections::HashSet;

pub const BINDING_V01: &str = "openstatspec-in-place-transformation-v0.1";
pub const BINDING_V02: &str = "openstatspec-in-place-transformation-v0.2";

/// The controlled Dolt context of an apply: the active branch and the HEAD commit the caller
/// expects. Other profiles ignore it.
#[derive(Clone, Debug, PartialEq)]
pub struct DoltContext {
    pub branch: String,
    pub head: String,
}

/// What a successful apply recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub apply_id: String,
    pub dataset_id: String,
    pub physical_table_name: String,
    pub operation_count: usize,
}

/// Checks a plan against the catalog of dataset `name` (names, target state, storage kinds)
/// without changing anything.
pub fn bind_plan(db: &mut dyn Db, name: &str, plan: &Plan) -> Result<()> {
    validate_plan(&plan.to_json())?;
    let (id, _) = find_dataset(db, name)?;
    let mut variables = catalog_variables(db, &id)?;
    let dialect = db.dialect();
    for op in &plan.operations {
        step(op, &mut variables, dialect, None)?;
    }
    Ok(())
}

/// Applies `plan` to dataset `name` in place. `source_hash` is the SHA-256 of the
/// LF-normalized syntax the plan came from; `actor` names who applies it. Every check runs
/// before the first change; any failure leaves data, catalog and audit as they were.
///
/// On Dolt, `dolt` is required: the session must be on that branch at that HEAD with a clean
/// working set before anything changes, and the branch must still point at that HEAD (seen
/// from a second session) before the transaction commits. The apply never commits to Dolt;
/// its changes stay in the working set.
pub fn apply_plan(
    db: &mut dyn Db,
    name: &str,
    plan: &Plan,
    source_hash: &str,
    actor: &str,
    dolt: Option<&DoltContext>,
) -> Result<Applied> {
    if actor.is_empty() {
        return Err(DbError::new("actor_required", "an apply needs an actor"));
    }
    if source_hash.len() != 64
        || !source_hash
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(DbError::new(
            "source_hash_invalid",
            "the source hash must be 64 lowercase hex digits",
        ));
    }
    validate_plan(&plan.to_json())?;
    db.write_gate()?;
    let dialect = db.dialect();
    let dolt = if dialect.profile == "dolt" {
        let Some(expected) = dolt else {
            return Err(DbError::new(
                "dolt_context_required",
                "an apply on Dolt needs the expected branch and HEAD",
            ));
        };
        check_dolt_context(db, expected)?;
        Some(expected)
    } else {
        None
    };
    db.batch(dialect.begin)?;
    let result = apply_in_transaction(db, name, plan, source_hash, actor, dolt);
    if result.is_err() {
        rollback(db);
    }
    result
}

/// The session's branch and HEAD, and a clean working set, before the first change.
fn check_dolt_context(db: &mut dyn Db, expected: &DoltContext) -> Result<()> {
    let row = first(db, "SELECT active_branch(), HASHOF('HEAD')", &[])?
        .ok_or_else(|| DbError::new("dolt_context_required", "no Dolt session context"))?;
    let (branch, head) = (row[0].text()?, row[1].text()?);
    if branch != expected.branch {
        return Err(DbError::new(
            "dolt_branch_mismatch",
            format!("the session is on {branch}, not {}", expected.branch),
        ));
    }
    if head != expected.head {
        return Err(DbError::new(
            "dolt_head_mismatch",
            format!("HEAD is {head}, not {}", expected.head),
        ));
    }
    let changes =
        first(db, "SELECT count(*) FROM dolt_status", &[])?.map_or(Ok(0), |r| r[0].int())?;
    if changes != 0 {
        return Err(DbError::new(
            "dolt_working_set_dirty",
            format!("the working set has {changes} uncommitted table changes"),
        ));
    }
    Ok(())
}

fn apply_in_transaction(
    db: &mut dyn Db,
    name: &str,
    plan: &Plan,
    source_hash: &str,
    actor: &str,
    dolt: Option<&DoltContext>,
) -> Result<Applied> {
    let dialect = db.dialect();
    let started =
        first(db, &format!("SELECT {}", dialect.now), &[])?.map_or(Sql::Null, |r| r[0].clone());
    let (dataset_id, table) = find_dataset(db, name)?;
    let identity = |db: &mut dyn Db| {
        first(
            db,
            "SELECT physical_table_schema, physical_table_name FROM dataset WHERE dataset_id = ?",
            args![&dataset_id],
        )
    };
    let before = identity(db)?;
    let mut variables = catalog_variables(db, &dataset_id)?;
    // binding: every operation checked against the evolving schema before any mutation
    let mut bound = variables.clone();
    for op in &plan.operations {
        step(op, &mut bound, dialect, None)?;
    }
    let identifier = db.limits()?.identifier;
    let mut exec = Exec {
        db,
        dataset_id: &dataset_id,
        table: &table,
        identifier,
    };
    for op in &plan.operations {
        step(op, &mut variables, dialect, Some(&mut exec))?;
    }
    if identity(db)? != before {
        return Err(DbError::new(
            "dataset_identity_changed",
            "the dataset table identity changed during the apply",
        ));
    }
    // the branch must still point at the expected HEAD: a commit by another session would
    // otherwise absorb this apply's changes unseen
    let dolt_audit = match dolt {
        Some(expected) => {
            let branch = first(db, "SELECT active_branch()", &[])?
                .map_or(Ok(String::new()), |r| r[0].text())?;
            let head = db.committed_head(&expected.branch)?;
            if branch != expected.branch || head != expected.head {
                return Err(DbError::new(
                    "dolt_context_changed",
                    format!(
                        "{} moved from {} to {head} (session on {branch}) during the apply",
                        expected.branch, expected.head
                    ),
                ));
            }
            [
                Sql::from(&expected.branch),
                Sql::from(&expected.head),
                Sql::from(head),
            ]
        }
        None => [Sql::Null, Sql::Null, Sql::Null],
    };
    let apply_id = uuid();
    let contract = if plan.contract == PLAN_V01 {
        BINDING_V01
    } else {
        BINDING_V02
    };
    let schema = before.map_or(Sql::Null, |r| r[0].clone());
    let [dolt_branch, dolt_head_before, dolt_head_after] = dolt_audit;
    db.exec(
        &format!(
            "INSERT INTO transformation_apply VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
             'succeeded', ?, ?, ?, ?, ?, {})",
            dialect.now
        ),
        &[
            Sql::from(&apply_id),
            contract.into(),
            dialect.profile.into(),
            Sql::from(&dataset_id),
            schema,
            Sql::from(&table),
            source_hash.into(),
            plan.hash().into(),
            plan.canonical().into(),
            actor.into(),
            dolt_branch,
            dolt_head_before,
            dolt_head_after,
            plan.operations.len().into(),
            started,
        ],
    )?;
    db.batch("COMMIT")?;
    Ok(Applied {
        apply_id,
        dataset_id,
        physical_table_name: table,
        operation_count: plan.operations.len(),
    })
}

/// Where an executed step writes.
struct Exec<'a> {
    db: &'a mut dyn Db,
    dataset_id: &'a str,
    table: &'a str,
    identifier: usize,
}

fn find<'v>(variables: &'v [CatalogVariable], name: &str) -> Option<&'v CatalogVariable> {
    // plans carry the exact catalog names
    variables.iter().find(|v| v.name == name)
}

fn need<'v>(variables: &'v [CatalogVariable], name: &str) -> Result<&'v CatalogVariable> {
    find(variables, name)
        .ok_or_else(|| DbError::new("unknown_variable", format!("unknown variable {name}")))
}

fn fail<T>(code: &'static str, message: String) -> Result<T> {
    Err(DbError::new(code, message))
}

fn is_string(t: &Typed) -> bool {
    matches!(t, Typed::String { .. })
}

/// Adds a typed literal as the next parameter and returns its placeholder.
fn literal(t: &Typed, params: &mut Vec<Sql>, dialect: &Dialect) -> String {
    params.push(match t {
        Typed::Binary64 { bits } => Sql::Real(bits.value()),
        Typed::String { value } => Sql::Text(value.clone()),
    });
    dialect.param(is_string(t)).to_string()
}

/// A numeric operand as SQL; a string variable is outside the bounded expressions.
fn operand(
    o: &Operand,
    variables: &[CatalogVariable],
    params: &mut Vec<Sql>,
    dialect: &Dialect,
) -> Result<String> {
    match o {
        Operand::Literal { value } => Ok(literal(value, params, dialect)),
        Operand::Variable { variable } => {
            let v = need(variables, variable)?;
            if v.string_width.is_some() {
                return fail(
                    "expression_type_unsupported",
                    format!("{variable} is a string; operands must be numeric"),
                );
            }
            Ok(dialect.quote(&v.physical))
        }
    }
}

/// SQL three-valued logic is the plan's: NULL comparisons are UNKNOWN, and UPDATE ... WHERE
/// changes only TRUE rows.
fn predicate(
    p: &Predicate,
    variables: &[CatalogVariable],
    params: &mut Vec<Sql>,
    dialect: &Dialect,
) -> Result<String> {
    match p {
        Predicate::Comparison {
            left,
            operator,
            right,
        } => {
            let left = operand(left, variables, params, dialect)?;
            let right = operand(right, variables, params, dialect)?;
            let op = match operator {
                Comparator::Eq => "=",
                Comparator::Lt => "<",
                Comparator::Le => "<=",
                Comparator::Gt => ">",
                Comparator::Ge => ">=",
            };
            Ok(format!("({left} {op} {right})"))
        }
        Predicate::Boolean { operator, operands } => {
            let join = match operator {
                Connective::And => " AND ",
                Connective::Or => " OR ",
            };
            let parts: Vec<String> = operands
                .iter()
                .map(|o| predicate(o, variables, params, dialect))
                .collect::<Result<_>>()?;
            Ok(format!("({})", parts.join(join)))
        }
    }
}

/// Appends a new variable after the last one: a physical column plus its catalog row with no
/// label, value labels, format, level or missing values.
fn create(
    variables: &mut Vec<CatalogVariable>,
    name: &str,
    string_width: Option<usize>,
    dialect: &Dialect,
    exec: Option<&mut Exec>,
) -> Result<CatalogVariable> {
    if find(variables, name).is_some() {
        return fail("target_already_exists", format!("{name} already exists"));
    }
    if !dialect.atomic_schema_change {
        return fail(
            "schema_change_not_atomic",
            format!("creating {name} cannot share one transaction with the apply"),
        );
    }
    let mut variable = CatalogVariable {
        id: String::new(),
        name: name.to_string(),
        physical: String::new(),
        string_width,
    };
    if let Some(x) = exec {
        let mut taken: HashSet<String> = variables
            .iter()
            .map(|v| v.physical.to_ascii_lowercase())
            .chain(["__case_ordinal".to_string()])
            .collect();
        let ordinal = variables.len() + 1;
        variable.physical = physical_names(&[(ordinal, name)], &mut taken, x.identifier).remove(0);
        let kind = match string_width {
            None => dialect.numeric_type.to_string(),
            Some(_) => format!("{} DEFAULT ''", dialect.text_type),
        };
        x.db.batch(&format!(
            "ALTER TABLE {} ADD COLUMN {} {kind}",
            dialect.quote(x.table),
            dialect.quote(&variable.physical)
        ))?;
        variable.id = uuid();
        let next = first(
            x.db,
            "SELECT coalesce(max(source_ordinal), 0) + 1 FROM variable WHERE dataset_id = ?",
            args![x.dataset_id],
        )?
        .map_or(Ok(1), |r| r[0].int())?;
        x.db.exec(
            "INSERT INTO variable (variable_id, dataset_id, source_ordinal, source_name,
                                   physical_name, storage_kind, declared_string_width)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            args![
                &variable.id,
                x.dataset_id,
                next,
                &variable.name,
                &variable.physical,
                if string_width.is_some() {
                    "string"
                } else {
                    "numeric"
                },
                string_width
            ],
        )?;
    }
    variables.push(variable.clone());
    Ok(variable)
}

/// Binds one operation against `variables` (the schema as earlier operations left it) and,
/// with `exec`, carries it out.
fn step(
    op: &Op,
    variables: &mut Vec<CatalogVariable>,
    dialect: &Dialect,
    exec: Option<&mut Exec>,
) -> Result<()> {
    let mut params: Vec<Sql> = vec![];
    let quote = |name: &str| dialect.quote(name);
    match op {
        Op::Recode {
            source,
            target,
            target_mode,
            rules,
            unmatched,
        } => {
            let src = need(variables, source)?.clone();
            let numeric = src.string_width.is_none();
            let kind = |string: bool| if string { "string" } else { "numeric" };
            for rule in rules {
                match &rule.matches {
                    Match::Values { values } => {
                        if let Some(v) = values.iter().find(|v| is_string(v) == numeric) {
                            return fail(
                                "type_mismatch",
                                format!("{source} is {}, {v:?} is not", kind(!numeric)),
                            );
                        }
                    }
                    Match::Range { .. } if !numeric => {
                        return fail("type_mismatch", format!("{source} is a string"));
                    }
                    Match::SystemMissing {} if !numeric => {
                        return fail("system_missing_for_string", format!("{source} is a string"));
                    }
                    _ => {}
                }
            }
            let results: Vec<&Outcome> =
                rules.iter().map(|r| &r.result).chain([unmatched]).collect();
            let target = match target_mode {
                TargetMode::Replace => {
                    let t = need(variables, target)?;
                    if t.id != src.id || t.name != src.name {
                        return fail(
                            "target_already_exists",
                            format!("replace needs target {target} to be the source {source}"),
                        );
                    }
                    src.clone()
                }
                TargetMode::Create => {
                    if find(variables, target).is_some() {
                        return fail("target_already_exists", format!("{target} already exists"));
                    }
                    let kinds: HashSet<bool> = results
                        .iter()
                        .filter_map(|o| match o {
                            Outcome::Literal { value } => Some(is_string(value)),
                            Outcome::Copy {} => Some(!numeric),
                            Outcome::SystemMissing {} => None,
                        })
                        .collect();
                    if kinds.len() > 1 {
                        return fail(
                            "mixed_result_types",
                            "the results mix numbers and strings".into(),
                        );
                    }
                    let Some(&string) = kinds.iter().next() else {
                        return fail(
                            "target_type_ambiguous",
                            format!("the results do not determine the storage kind of {target}"),
                        );
                    };
                    // a new string target is as wide as its widest possible result
                    let width = string.then(|| {
                        results
                            .iter()
                            .map(|o| match o {
                                Outcome::Literal {
                                    value: Typed::String { value },
                                } => value.len(),
                                Outcome::Copy {} => src.string_width.unwrap_or(0),
                                _ => 0,
                            })
                            .max()
                            .unwrap_or(0)
                            .max(1)
                    });
                    // checked like an existing target below; created only once all checks pass
                    CatalogVariable {
                        id: String::new(),
                        name: target.clone(),
                        physical: String::new(),
                        string_width: width,
                    }
                }
            };
            for result in &results {
                match result {
                    Outcome::Literal { value }
                        if is_string(value) != target.string_width.is_some() =>
                    {
                        return fail(
                            "type_mismatch",
                            format!(
                                "the result {value:?} does not fit {target}",
                                target = target.name
                            ),
                        );
                    }
                    Outcome::Literal {
                        value: Typed::String { value },
                    } if value.len() > target.string_width.unwrap_or(0) => {
                        return fail(
                            "type_mismatch",
                            format!("{value:?} is wider than {}", target.name),
                        );
                    }
                    Outcome::SystemMissing {} if target.string_width.is_some() => {
                        return fail(
                            "system_missing_for_string",
                            format!("{} is a string", target.name),
                        );
                    }
                    _ => {}
                }
            }
            let mut exec = exec;
            let target = match target_mode {
                TargetMode::Create => create(
                    variables,
                    &target.name,
                    target.string_width,
                    dialect,
                    exec.as_deref_mut(),
                )?,
                TargetMode::Replace => target,
            };
            let Some(x) = exec else { return Ok(()) };
            let src_sql = quote(&src.physical);
            let result = |o: &Outcome, params: &mut Vec<Sql>| match o {
                Outcome::Literal { value } => literal(value, params, dialect),
                Outcome::SystemMissing {} => "NULL".to_string(),
                Outcome::Copy {} => src_sql.clone(),
            };
            let mut case = String::from("CASE");
            for rule in rules {
                let condition = match &rule.matches {
                    Match::Values { values } => {
                        let list: Vec<String> = values
                            .iter()
                            .map(|v| literal(v, &mut params, dialect))
                            .collect();
                        format!("{src_sql} IN ({})", list.join(", "))
                    }
                    Match::Range { lower, upper } => {
                        let lower = literal(lower, &mut params, dialect);
                        let upper = literal(upper, &mut params, dialect);
                        format!("{src_sql} BETWEEN {lower} AND {upper}")
                    }
                    Match::SystemMissing {} => format!("{src_sql} IS NULL"),
                };
                let then = result(&rule.result, &mut params);
                case += &format!(" WHEN {condition} THEN {then}");
            }
            case += &format!(" ELSE {} END", result(unmatched, &mut params));
            x.db.exec(
                &format!(
                    "UPDATE {} SET {} = {case}",
                    quote(x.table),
                    quote(&target.physical)
                ),
                &params,
            )?;
        }
        Op::Assign {
            target,
            target_mode,
            value,
        } => {
            // the value reads the row as it was before this operation
            let value = operand(value, variables, &mut params, dialect)?;
            let mut exec = exec;
            let target = match target_mode {
                TargetMode::Create => {
                    create(variables, target, None, dialect, exec.as_deref_mut())?
                }
                TargetMode::Replace => numeric_target(variables, target, "unknown_variable")?,
            };
            if let Some(x) = exec {
                x.db.exec(
                    &format!(
                        "UPDATE {} SET {} = {value}",
                        quote(x.table),
                        quote(&target.physical)
                    ),
                    &params,
                )?;
            }
        }
        Op::ConditionalAssign {
            condition,
            target,
            value,
        } => {
            let target = numeric_target(variables, target, "conditional_target_missing")?;
            let value = operand(value, variables, &mut params, dialect)?;
            let condition = predicate(condition, variables, &mut params, dialect)?;
            if let Some(x) = exec {
                x.db.exec(
                    &format!(
                        "UPDATE {} SET {} = {value} WHERE {condition}",
                        quote(x.table),
                        quote(&target.physical)
                    ),
                    &params,
                )?;
            }
        }
        Op::SetVariableLabel { variable, label } => {
            let v = need(variables, variable)?;
            if let Some(x) = exec {
                x.db.exec(
                    "UPDATE variable SET variable_label = ? WHERE variable_id = ?",
                    args![label, &v.id],
                )?;
            }
        }
        Op::ReplaceValueLabels { variable, labels } => {
            let v = need(variables, variable)?;
            for label in labels {
                let fits = match (&label.value, v.string_width) {
                    (Typed::Binary64 { .. }, None) => true,
                    (Typed::String { value }, Some(width)) => value.len() <= width,
                    _ => false,
                };
                if !fits {
                    return fail(
                        "type_mismatch",
                        format!("the value {:?} does not fit {variable}", label.value),
                    );
                }
            }
            if let Some(x) = exec {
                let old = x.db.query(
                    "SELECT value_label_set_id FROM variable_value_label_set WHERE variable_id = ?",
                    args![&v.id],
                )?;
                x.db.exec(
                    "DELETE FROM variable_value_label_set WHERE variable_id = ?",
                    args![&v.id],
                )?;
                for set in old {
                    x.db.exec("DELETE FROM value_label WHERE value_label_set_id = ?", &set)?;
                    x.db.exec(
                        "DELETE FROM value_label_set WHERE value_label_set_id = ?",
                        &set,
                    )?;
                }
                let labels: Vec<(statscraft_data::Value, String)> = labels
                    .iter()
                    .map(|l| {
                        let value = match &l.value {
                            Typed::Binary64 { bits } => {
                                statscraft_data::Value::Number(bits.value())
                            }
                            Typed::String { value } => {
                                statscraft_data::Value::String(value.clone())
                            }
                        };
                        (value, l.label.clone())
                    })
                    .collect();
                store_value_labels(x.db, x.dataset_id, &v.id, &labels)?;
            }
        }
        Op::SetFormat {
            variable,
            family,
            width,
            decimals,
        } => {
            let v = need(variables, variable)?;
            if v.string_width.is_some() {
                return fail(
                    "expression_type_unsupported",
                    format!("{variable} is a string; set_format needs a numeric variable"),
                );
            }
            if let Some(x) = exec {
                let (width, decimals) = (*width as i64, *decimals as i64);
                x.db.exec(
                    "UPDATE variable SET print_format_family = ?, print_format_width = ?,
                         print_format_decimals = ?, write_format_family = ?,
                         write_format_width = ?, write_format_decimals = ?
                     WHERE variable_id = ?",
                    args![family, width, decimals, family, width, decimals, &v.id],
                )?;
            }
        }
        Op::SetMeasurementLevel { variable, level } => {
            let v = need(variables, variable)?;
            if let Some(x) = exec {
                let level = serde_json::to_value(level).expect("plain enum");
                x.db.exec(
                    "UPDATE variable SET measurement_level = ? WHERE variable_id = ?",
                    args![level.as_str(), &v.id],
                )?;
            }
        }
        Op::Execute {} => {}
    }
    Ok(())
}

/// An existing numeric target; `missing` is the code when it does not exist.
fn numeric_target(
    variables: &[CatalogVariable],
    name: &str,
    missing: &'static str,
) -> Result<CatalogVariable> {
    let Some(v) = find(variables, name) else {
        return fail(missing, format!("{name} does not exist"));
    };
    if v.string_width.is_some() {
        return fail(
            "expression_type_unsupported",
            format!("{name} is a string; assignments need a numeric target"),
        );
    }
    Ok(v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{read_dataset, write_dataset};
    use crate::sqlite::SqliteDb;
    use serde_json::{Value, json};
    use statscraft_data::Dataset;

    fn setup() -> SqliteDb {
        let mut conn = SqliteDb::memory().unwrap();
        let mut d = Dataset::new(3);
        d.set_numeric_values("a", vec![Some(1.0), Some(2.0), None])
            .unwrap();
        d.set_string_values("s", 3, vec!["x".into(), "yy".into(), "".into()])
            .unwrap();
        write_dataset(&mut conn, &d, "d").unwrap();
        conn
    }

    fn plan(contract: &str, ops: Value) -> Plan {
        validate_plan(&json!({"contract": contract, "input_alias": "d", "operations": ops}))
            .unwrap()
    }

    fn v02(ops: Value) -> Plan {
        plan(crate::plan::PLAN_V02, ops)
    }

    fn one() -> Value {
        json!({"type": "binary64", "bits": "3ff0000000000000"})
    }

    const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn code(conn: &mut SqliteDb, p: Plan) -> &'static str {
        apply_plan(conn, "d", &p, HASH, "t", None).unwrap_err().code
    }

    #[test]
    fn binding_rejects_what_only_the_catalog_can_tell() {
        let mut conn = setup();
        let assign = |target: &str, mode: &str, value: Value| {
            v02(json!([{"op": "assign", "target": target, "target_mode": mode, "value": value}]))
        };
        let lit = json!({"kind": "literal", "value": one()});
        let var = |n: &str| json!({"kind": "variable", "variable": n});
        assert_eq!(
            code(&mut conn, assign("a", "create", lit.clone())),
            "target_already_exists"
        );
        assert_eq!(
            code(&mut conn, assign("zz", "replace", lit.clone())),
            "unknown_variable"
        );
        assert_eq!(
            code(&mut conn, assign("t", "create", var("nope"))),
            "unknown_variable"
        );
        assert_eq!(
            code(&mut conn, assign("t", "create", var("s"))),
            "expression_type_unsupported"
        );
        assert_eq!(
            code(&mut conn, assign("s", "replace", lit.clone())),
            "expression_type_unsupported"
        );
        let cond = v02(
            json!([{"op": "conditional_assign", "target": "zz", "value": lit,
            "condition": {"expression": "comparison", "left": var("a"), "operator": "=", "right": lit}}]),
        );
        assert_eq!(code(&mut conn, cond), "conditional_target_missing");

        let recode = |source: &str, target: &str, mode: &str, rules: Value, unmatched: Value| {
            plan(
                crate::plan::PLAN_V01,
                json!([{"op": "recode", "source": source, "target": target,
                "target_mode": mode, "rules": rules, "unmatched": unmatched}]),
            )
        };
        let sysmis = json!({"kind": "system_missing"});
        let copy = json!({"kind": "copy"});
        let only_sysmis = json!([{"match": {"kind": "system_missing"}, "result": sysmis}]);
        assert_eq!(
            code(
                &mut conn,
                recode("a", "n", "create", only_sysmis.clone(), sysmis.clone())
            ),
            "target_type_ambiguous"
        );
        assert_eq!(
            code(
                &mut conn,
                recode(
                    "s",
                    "s",
                    "replace",
                    json!([{"match": {"kind": "values",
                "values": [{"type": "string", "value": "x"}]}, "result": sysmis}]),
                    copy.clone()
                )
            ),
            "system_missing_for_string"
        );
        assert_eq!(
            code(
                &mut conn,
                recode("s", "s", "replace", only_sysmis, copy.clone())
            ),
            "system_missing_for_string"
        );
        assert_eq!(
            code(
                &mut conn,
                recode(
                    "a",
                    "a",
                    "replace",
                    json!([{"match": {"kind": "values",
                "values": [{"type": "string", "value": "x"}]}, "result": copy}]),
                    copy.clone()
                )
            ),
            "type_mismatch"
        );
        assert_eq!(
            code(
                &mut conn,
                recode(
                    "a",
                    "a",
                    "replace",
                    json!([{"match": {"kind": "values",
                "values": [one()]}, "result": {"kind": "literal", "value": {"type": "string", "value": "x"}}}]),
                    copy.clone()
                )
            ),
            "type_mismatch"
        );
        assert_eq!(
            code(
                &mut conn,
                recode(
                    "s",
                    "n",
                    "create",
                    json!([{"match": {"kind": "values",
                "values": [{"type": "string", "value": "x"}]}, "result": {"kind": "literal", "value": one()}}]),
                    copy
                )
            ),
            "mixed_result_types"
        );
        let format = v02(
            json!([{"op": "set_format", "variable": "s", "family": "F", "width": 3, "decimals": 0}]),
        );
        assert_eq!(code(&mut conn, format), "expression_type_unsupported");
        let labels = plan(
            crate::plan::PLAN_V01,
            json!([{"op": "replace_value_labels", "variable": "s",
            "labels": [{"value": {"type": "string", "value": "toolong"}, "label": "x"}]}]),
        );
        assert_eq!(code(&mut conn, labels), "type_mismatch");
        let p = v02(json!([{"op": "execute"}]));
        assert_eq!(
            apply_plan(&mut conn, "d", &p, HASH, "", None)
                .unwrap_err()
                .code,
            "actor_required"
        );
        // nothing was applied
        let audits = conn
            .query("SELECT count(*) FROM transformation_apply", &[])
            .unwrap();
        assert_eq!(audits[0][0], Sql::Int(0));
    }

    #[test]
    fn create_then_label_in_one_plan_and_read_back() {
        let mut conn = setup();
        let p = plan(
            crate::plan::PLAN_V01,
            json!([
                {"op": "recode", "source": "s", "target": "s2", "target_mode": "create",
                 "rules": [{"match": {"kind": "values", "values": [{"type": "string", "value": "x"}]},
                            "result": {"kind": "literal", "value": {"type": "string", "value": "long"}}}],
                 "unmatched": {"kind": "copy"}},
                {"op": "set_variable_label", "variable": "s2", "label": "Copy"},
                {"op": "replace_value_labels", "variable": "s2",
                 "labels": [{"value": {"type": "string", "value": "long"}, "label": "L"}]}
            ]),
        );
        let applied = apply_plan(&mut conn, "d", &p, HASH, "t", None).unwrap();
        assert_eq!(applied.operation_count, 3);
        let (d, _) = read_dataset(&mut conn, "d", false).unwrap();
        let s2 = d.variable("s2").unwrap();
        assert_eq!(s2.string_width(), Some(4));
        assert_eq!(s2.label(), Some("Copy"));
        assert!(s2.string().unwrap().iter().eq(["long", "yy", ""]));
        let audit = conn
            .query(
                "SELECT contract_id, database_profile FROM transformation_apply",
                &[],
            )
            .unwrap();
        assert_eq!(audit[0], [Sql::from(BINDING_V01), Sql::from("sqlite")]);
    }
}
