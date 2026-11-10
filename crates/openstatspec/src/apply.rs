//! Catalog binding of a transformation plan and its in-place apply to a dataset in SQLite
//! (In-Place Transformation Binding 0.1 and 0.2): the same dataset and the same physical
//! table, direct UPDATE and ALTER TABLE ... ADD COLUMN, catalog updates and one compact audit
//! row, all in one transaction. Nothing is copied.

use crate::plan::{
    Comparator, Connective, Match, Op, Operand, Outcome, PLAN_V01, Plan, Predicate, TargetMode,
    Typed, validate_plan,
};
use crate::sqlite::{
    CatalogVariable, DbError, NOW, Result, SQLITE, catalog_variables, find_dataset, physical_names,
    store_value_labels, uuid,
};
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, Transaction, TransactionBehavior, params, params_from_iter};
use std::collections::HashSet;

pub const BINDING_V01: &str = "openstatspec-in-place-transformation-v0.1";
pub const BINDING_V02: &str = "openstatspec-in-place-transformation-v0.2";

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
pub fn bind_plan(conn: &Connection, name: &str, plan: &Plan) -> Result<()> {
    validate_plan(&plan.to_json())?;
    let (id, _) = find_dataset(conn, name)?;
    let mut variables = catalog_variables(conn, &id)?;
    for op in &plan.operations {
        step(op, &mut variables, None)?;
    }
    Ok(())
}

/// Applies `plan` to dataset `name` in place. `source_hash` is the SHA-256 of the
/// LF-normalized syntax the plan came from; `actor` names who applies it. Every check runs
/// before the first change; any failure leaves data, catalog and audit as they were.
pub fn apply_plan(
    conn: &mut Connection,
    name: &str,
    plan: &Plan,
    source_hash: &str,
    actor: &str,
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
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let started: String = tx.query_row(&format!("SELECT {NOW}"), [], |r| r.get(0))?;
    let (dataset_id, table) = find_dataset(&tx, name)?;
    let identity = |tx: &Transaction| -> Result<(Option<String>, String)> {
        Ok(tx.query_row(
            "SELECT physical_table_schema, physical_table_name FROM dataset WHERE dataset_id = ?1",
            [&dataset_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    };
    let before = identity(&tx)?;
    let mut variables = catalog_variables(&tx, &dataset_id)?;
    // binding: every operation checked against the evolving schema before any mutation
    let mut bound = variables.clone();
    for op in &plan.operations {
        step(op, &mut bound, None)?;
    }
    let exec = Exec {
        tx: &tx,
        dataset_id: &dataset_id,
        table: &table,
    };
    for op in &plan.operations {
        step(op, &mut variables, Some(&exec))?;
    }
    if identity(&tx)? != before {
        return Err(DbError::new(
            "dataset_identity_changed",
            "the dataset table identity changed during the apply",
        ));
    }
    let apply_id = uuid(&tx)?;
    let contract = if plan.contract == PLAN_V01 {
        BINDING_V01
    } else {
        BINDING_V02
    };
    tx.execute(
        &format!(
            "INSERT INTO transformation_apply VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
             'succeeded', NULL, NULL, NULL, ?11, ?12, {NOW})"
        ),
        params![
            apply_id,
            contract,
            SQLITE.profile,
            dataset_id,
            before.0,
            table,
            source_hash,
            plan.hash(),
            plan.canonical(),
            actor,
            plan.operations.len() as i64,
            started
        ],
    )?;
    tx.commit()?;
    Ok(Applied {
        apply_id,
        dataset_id,
        physical_table_name: table,
        operation_count: plan.operations.len(),
    })
}

/// Where an executed step writes.
struct Exec<'a> {
    tx: &'a Transaction<'a>,
    dataset_id: &'a str,
    table: &'a str,
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

fn quote(name: &str) -> String {
    SQLITE.quote(name)
}

/// Adds a typed literal as the next parameter and returns its placeholder.
fn literal(t: &Typed, params: &mut Vec<Sql>) -> String {
    params.push(match t {
        Typed::Binary64 { bits } => Sql::Real(bits.value()),
        Typed::String { value } => Sql::Text(value.clone()),
    });
    format!("?{}", params.len())
}

/// A numeric operand as SQL; a string variable is outside the bounded expressions.
fn operand(o: &Operand, variables: &[CatalogVariable], params: &mut Vec<Sql>) -> Result<String> {
    match o {
        Operand::Literal { value } => Ok(literal(value, params)),
        Operand::Variable { variable } => {
            let v = need(variables, variable)?;
            if v.string_width.is_some() {
                return fail(
                    "expression_type_unsupported",
                    format!("{variable} is a string; operands must be numeric"),
                );
            }
            Ok(quote(&v.physical))
        }
    }
}

/// SQL three-valued logic is the plan's: NULL comparisons are UNKNOWN, and UPDATE ... WHERE
/// changes only TRUE rows.
fn predicate(
    p: &Predicate,
    variables: &[CatalogVariable],
    params: &mut Vec<Sql>,
) -> Result<String> {
    match p {
        Predicate::Comparison {
            left,
            operator,
            right,
        } => {
            let left = operand(left, variables, params)?;
            let right = operand(right, variables, params)?;
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
                .map(|o| predicate(o, variables, params))
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
    exec: Option<&Exec>,
) -> Result<CatalogVariable> {
    if find(variables, name).is_some() {
        return fail("target_already_exists", format!("{name} already exists"));
    }
    if !SQLITE.atomic_schema_change {
        return fail(
            "schema_change_not_atomic",
            format!("creating {name} cannot share one transaction with the apply"),
        );
    }
    let mut taken: HashSet<String> = variables
        .iter()
        .map(|v| v.physical.to_ascii_lowercase())
        .chain(["__case_ordinal".to_string()])
        .collect();
    let ordinal = variables.len() + 1;
    let physical = physical_names(&[(ordinal, name)], &mut taken, &SQLITE).remove(0);
    let mut variable = CatalogVariable {
        id: String::new(),
        name: name.to_string(),
        physical,
        string_width,
    };
    if let Some(x) = exec {
        let kind = match string_width {
            None => SQLITE.numeric_type.to_string(),
            Some(_) => format!("{} DEFAULT ''", SQLITE.text_type),
        };
        x.tx.execute(
            &format!(
                "ALTER TABLE {} ADD COLUMN {} {kind}",
                quote(x.table),
                quote(&variable.physical)
            ),
            [],
        )?;
        variable.id = uuid(x.tx)?;
        x.tx.execute(
            "INSERT INTO variable (variable_id, dataset_id, source_ordinal, source_name,
                                   physical_name, storage_kind, declared_string_width)
             VALUES (?1, ?2, (SELECT coalesce(max(source_ordinal), 0) + 1 FROM variable
                              WHERE dataset_id = ?2), ?3, ?4, ?5, ?6)",
            params![
                variable.id,
                x.dataset_id,
                variable.name,
                variable.physical,
                if string_width.is_some() {
                    "string"
                } else {
                    "numeric"
                },
                string_width.map(|w| w as i64)
            ],
        )?;
    }
    variables.push(variable.clone());
    Ok(variable)
}

/// Binds one operation against `variables` (the schema as earlier operations left it) and,
/// with `exec`, carries it out.
fn step(op: &Op, variables: &mut Vec<CatalogVariable>, exec: Option<&Exec>) -> Result<()> {
    let mut params: Vec<Sql> = vec![];
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
            let target = match target_mode {
                TargetMode::Create => create(variables, &target.name, target.string_width, exec)?,
                TargetMode::Replace => target,
            };
            let Some(x) = exec else { return Ok(()) };
            let src_sql = quote(&src.physical);
            let result = |o: &Outcome, params: &mut Vec<Sql>| match o {
                Outcome::Literal { value } => literal(value, params),
                Outcome::SystemMissing {} => "NULL".to_string(),
                Outcome::Copy {} => src_sql.clone(),
            };
            let mut case = String::from("CASE");
            for rule in rules {
                let condition = match &rule.matches {
                    Match::Values { values } => {
                        let list: Vec<String> =
                            values.iter().map(|v| literal(v, &mut params)).collect();
                        format!("{src_sql} IN ({})", list.join(", "))
                    }
                    Match::Range { lower, upper } => {
                        let lower = literal(lower, &mut params);
                        let upper = literal(upper, &mut params);
                        format!("{src_sql} BETWEEN {lower} AND {upper}")
                    }
                    Match::SystemMissing {} => format!("{src_sql} IS NULL"),
                };
                let then = result(&rule.result, &mut params);
                case += &format!(" WHEN {condition} THEN {then}");
            }
            case += &format!(" ELSE {} END", result(unmatched, &mut params));
            x.tx.execute(
                &format!(
                    "UPDATE {} SET {} = {case}",
                    quote(x.table),
                    quote(&target.physical)
                ),
                params_from_iter(params),
            )?;
        }
        Op::Assign {
            target,
            target_mode,
            value,
        } => {
            // the value reads the row as it was before this operation
            let value = operand(value, variables, &mut params)?;
            let target = match target_mode {
                TargetMode::Create => create(variables, target, None, exec)?,
                TargetMode::Replace => numeric_target(variables, target, "unknown_variable")?,
            };
            if let Some(x) = exec {
                x.tx.execute(
                    &format!(
                        "UPDATE {} SET {} = {value}",
                        quote(x.table),
                        quote(&target.physical)
                    ),
                    params_from_iter(params),
                )?;
            }
        }
        Op::ConditionalAssign {
            condition,
            target,
            value,
        } => {
            let target = numeric_target(variables, target, "conditional_target_missing")?;
            let value = operand(value, variables, &mut params)?;
            let condition = predicate(condition, variables, &mut params)?;
            if let Some(x) = exec {
                x.tx.execute(
                    &format!(
                        "UPDATE {} SET {} = {value} WHERE {condition}",
                        quote(x.table),
                        quote(&target.physical)
                    ),
                    params_from_iter(params),
                )?;
            }
        }
        Op::SetVariableLabel { variable, label } => {
            let v = need(variables, variable)?;
            if let Some(x) = exec {
                x.tx.execute(
                    "UPDATE variable SET variable_label = ?1 WHERE variable_id = ?2",
                    params![label, v.id],
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
                let old: Vec<String> = x
                    .tx
                    .prepare("SELECT value_label_set_id FROM variable_value_label_set WHERE variable_id = ?1")?
                    .query_map([&v.id], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?;
                x.tx.execute(
                    "DELETE FROM variable_value_label_set WHERE variable_id = ?1",
                    [&v.id],
                )?;
                for set in old {
                    x.tx.execute(
                        "DELETE FROM value_label WHERE value_label_set_id = ?1",
                        [&set],
                    )?;
                    x.tx.execute(
                        "DELETE FROM value_label_set WHERE value_label_set_id = ?1",
                        [&set],
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
                store_value_labels(x.tx, x.dataset_id, &v.id, &labels)?;
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
                x.tx.execute(
                    "UPDATE variable SET print_format_family = ?1, print_format_width = ?2,
                         print_format_decimals = ?3, write_format_family = ?1,
                         write_format_width = ?2, write_format_decimals = ?3
                     WHERE variable_id = ?4",
                    params![family, *width as i64, *decimals as i64, v.id],
                )?;
            }
        }
        Op::SetMeasurementLevel { variable, level } => {
            let v = need(variables, variable)?;
            if let Some(x) = exec {
                let level = serde_json::to_value(level).expect("plain enum");
                x.tx.execute(
                    "UPDATE variable SET measurement_level = ?1 WHERE variable_id = ?2",
                    params![level.as_str(), v.id],
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
    use crate::sqlite::{prepare, read_dataset, write_dataset};
    use serde_json::{Value, json};
    use statscraft_data::Dataset;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        prepare(&mut conn, true).unwrap();
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

    fn code(conn: &mut Connection, p: Plan) -> &'static str {
        apply_plan(conn, "d", &p, HASH, "t").unwrap_err().code
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
            apply_plan(&mut conn, "d", &p, HASH, "").unwrap_err().code,
            "actor_required"
        );
        // nothing was applied
        let audits: i64 = conn
            .query_row("SELECT count(*) FROM transformation_apply", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(audits, 0);
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
        let applied = apply_plan(&mut conn, "d", &p, HASH, "t").unwrap();
        assert_eq!(applied.operation_count, 3);
        let d = read_dataset(&mut conn, "d", false).unwrap();
        let s2 = d.variable("s2").unwrap();
        assert_eq!(s2.string_width(), Some(4));
        assert_eq!(s2.label(), Some("Copy"));
        assert!(s2.string().unwrap().iter().eq(["long", "yy", ""]));
        let audit: (String, String) = conn
            .query_row(
                "SELECT contract_id, database_profile FROM transformation_apply",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(audit, (BINDING_V01.into(), "sqlite".into()));
    }
}
