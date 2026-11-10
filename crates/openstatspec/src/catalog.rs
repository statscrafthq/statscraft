//! SQL binding of the SAV SAV/ZSAV profile 1.0: one dataset is one wide table plus the
//! normative catalog, in a namespace dedicated to OpenStatSpec (a SQLite file, a PostgreSQL
//! schema, a MySQL, MariaDB or Dolt database).
//!
//! The specification calls SAV to SQL "import" and SQL to SAV "export"; the operation and
//! fidelity rows use those words. In this crate `write_dataset` is the spec import and
//! `read_dataset` the spec export.

use crate::args;
use crate::db::{Db, DbError, Result, Sql, first, uuid};
use serde_json::{Value, json};
use statscraft_data::{Dataset, Measure, MissingValues, MultipleResponseSet, Values, VariableSet};
use std::collections::HashSet;

pub const CONTRACT_ID: &str = "openstatspec-strict-wide-table-v1";
/// Version of the catalog DDL below. The specification does not number catalog schemas.
pub const SCHEMA_VERSION: i64 = 1;
/// Written to `dataset.spec_version`: the profile the dataset follows.
pub const SPEC_VERSION: &str = "spss-sav-zsav-1.0";

/// The catalog relations, in creation order.
pub const CATALOG_TABLES: [&str; 18] = [
    "catalog_identity",
    "dataset",
    "operation",
    "variable",
    "dataset_weight_variable",
    "value_label_set",
    "value_label",
    "variable_value_label_set",
    "missing_rule",
    "dataset_attribute",
    "variable_attribute",
    "document",
    "variable_set",
    "variable_set_member",
    "multiple_response_set",
    "multiple_response_member",
    "fidelity_event",
    "transformation_apply",
];

/// `sql/schema-outline.sql` and `sql/transformation-plan-profile-schema.sql` with type tokens
/// that `Dialect::ddl` replaces: `{id}` UUID, `{ts}` TIMESTAMP, `{key}` text that is part of a
/// key, `{text}` other text, `{real}` DOUBLE PRECISION, `{int}` BIGINT.
const CATALOG_DDL: &str = r#"
CREATE TABLE catalog_identity (
  catalog_identity_key INTEGER PRIMARY KEY CHECK (catalog_identity_key = 1),
  contract_id {key} NOT NULL UNIQUE CHECK (contract_id = 'openstatspec-strict-wide-table-v1'),
  schema_version {int} NOT NULL,
  created_at {ts} NOT NULL
){options};
CREATE TABLE dataset (
  dataset_id {id} PRIMARY KEY,
  spec_version {key} NOT NULL,
  source_format {key} NOT NULL,
  physical_table_schema {key} NULL,
  physical_table_name {key} NOT NULL UNIQUE{nocase},
  dataset_name {key} NULL UNIQUE{nocase},
  dataset_label {text} NULL,
  source_encoding {key} NULL,
  source_hash {key} NULL,
  source_case_count {int} NOT NULL,
  imported_at {ts} NOT NULL
){options};
CREATE TABLE operation (
  operation_id {id} PRIMARY KEY,
  operation_kind {key} NOT NULL CHECK (operation_kind IN ('import', 'export')),
  status {key} NOT NULL CHECK (status IN ('started', 'succeeded', 'failed')),
  source_format {key} NULL,
  started_at {ts} NOT NULL,
  completed_at {ts} NULL
){options};
CREATE TABLE variable (
  variable_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal {int} NOT NULL,
  source_name {key} NOT NULL,
  physical_name {key} NOT NULL{nocase},
  storage_kind {key} NOT NULL CHECK (storage_kind IN ('numeric', 'string')),
  declared_string_width {int} NULL,
  variable_label {text} NULL,
  print_format_family {key} NULL,
  print_format_width {int} NULL,
  print_format_decimals {int} NULL,
  write_format_family {key} NULL,
  write_format_width {int} NULL,
  write_format_decimals {int} NULL,
  measurement_level {key} NULL,
  variable_role {key} NULL,
  display_width {int} NULL,
  display_alignment {key} NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, source_name),
  UNIQUE (dataset_id, physical_name)
){options};
CREATE TABLE dataset_weight_variable (
  dataset_id {id} PRIMARY KEY REFERENCES dataset(dataset_id),
  variable_id {id} NOT NULL UNIQUE REFERENCES variable(variable_id)
){options};
CREATE TABLE value_label_set (
  value_label_set_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  name {key} NULL
){options};
CREATE TABLE value_label (
  value_label_id {id} PRIMARY KEY,
  value_label_set_id {id} NOT NULL REFERENCES value_label_set(value_label_set_id),
  ordinal {int} NOT NULL,
  code_kind {key} NOT NULL CHECK (code_kind IN ('numeric', 'string')),
  numeric_code {real} NULL,
  string_code {text} NULL,
  label {text} NOT NULL,
  UNIQUE (value_label_set_id, ordinal)
){options};
CREATE TABLE variable_value_label_set (
  variable_id {id} PRIMARY KEY REFERENCES variable(variable_id),
  value_label_set_id {id} NOT NULL REFERENCES value_label_set(value_label_set_id)
){options};
CREATE TABLE missing_rule (
  missing_rule_id {id} PRIMARY KEY,
  variable_id {id} NOT NULL REFERENCES variable(variable_id),
  ordinal {int} NOT NULL,
  rule_kind {key} NOT NULL CHECK (rule_kind IN ('discrete', 'numeric_range')),
  code_kind {key} NULL,
  numeric_value {real} NULL,
  string_value {text} NULL,
  numeric_lower {real} NULL,
  numeric_upper {real} NULL,
  lower_special {key} NULL,
  upper_special {key} NULL,
  UNIQUE (variable_id, ordinal)
){options};
CREATE TABLE dataset_attribute (
  dataset_attribute_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  attribute_name {key} NOT NULL,
  array_ordinal {int} NOT NULL DEFAULT 1,
  attribute_value {text} NOT NULL,
  UNIQUE (dataset_id, attribute_name, array_ordinal)
){options};
CREATE TABLE variable_attribute (
  variable_attribute_id {id} PRIMARY KEY,
  variable_id {id} NOT NULL REFERENCES variable(variable_id),
  attribute_name {key} NOT NULL,
  array_ordinal {int} NOT NULL DEFAULT 1,
  attribute_value {text} NOT NULL,
  UNIQUE (variable_id, attribute_name, array_ordinal)
){options};
CREATE TABLE document (
  document_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal {int} NOT NULL,
  document_text {text} NOT NULL,
  UNIQUE (dataset_id, source_ordinal)
){options};
CREATE TABLE variable_set (
  variable_set_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal {int} NOT NULL,
  set_name {key} NOT NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, set_name)
){options};
CREATE TABLE variable_set_member (
  variable_set_id {id} NOT NULL REFERENCES variable_set(variable_set_id),
  variable_id {id} NOT NULL REFERENCES variable(variable_id),
  source_ordinal {int} NOT NULL,
  PRIMARY KEY (variable_set_id, source_ordinal),
  UNIQUE (variable_set_id, variable_id)
){options};
CREATE TABLE multiple_response_set (
  multiple_response_set_id {id} PRIMARY KEY,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal {int} NOT NULL,
  set_name {key} NOT NULL,
  set_label {text} NULL,
  set_kind {key} NOT NULL CHECK (set_kind IN ('MD', 'MC')),
  counted_value_kind {key} NULL,
  counted_numeric_value {real} NULL,
  counted_string_value {text} NULL,
  category_label_behavior {key} NULL,
  label_source {key} NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, set_name)
){options};
CREATE TABLE multiple_response_member (
  multiple_response_set_id {id} NOT NULL REFERENCES multiple_response_set(multiple_response_set_id),
  variable_id {id} NOT NULL REFERENCES variable(variable_id),
  source_ordinal {int} NOT NULL,
  PRIMARY KEY (multiple_response_set_id, source_ordinal),
  UNIQUE (multiple_response_set_id, variable_id)
){options};
CREATE TABLE fidelity_event (
  fidelity_event_id {id} PRIMARY KEY,
  operation_id {id} NOT NULL REFERENCES operation(operation_id),
  dataset_id {id} NULL REFERENCES dataset(dataset_id),
  direction {key} NOT NULL CHECK (direction IN ('import', 'export')),
  severity {key} NOT NULL CHECK (severity IN ('error', 'warning')),
  event_code {key} NOT NULL,
  source_item {text} NULL,
  detail_json {text} NOT NULL,
  created_at {ts} NOT NULL
){options};
CREATE TABLE transformation_apply (
  apply_id {id} PRIMARY KEY,
  contract_id {key} NOT NULL,
  database_profile {key} NOT NULL,
  dataset_id {id} NOT NULL REFERENCES dataset(dataset_id),
  physical_table_schema {key} NULL,
  physical_table_name {key} NOT NULL,
  source_hash {key} NOT NULL CHECK ({hex source_hash}),
  plan_hash {key} NOT NULL CHECK ({hex plan_hash}),
  canonical_plan_json {text} NOT NULL,
  actor {text} NOT NULL,
  status {key} NOT NULL,
  dolt_branch {text} NULL,
  dolt_head_before {key} NULL,
  dolt_head_after {key} NULL,
  operation_count {int} NOT NULL,
  started_at {ts} NOT NULL,
  completed_at {ts} NOT NULL,
  CHECK (contract_id IN (
    'openstatspec-in-place-transformation-v0.1',
    'openstatspec-in-place-transformation-v0.2'
  )),
  CHECK (database_profile IN ('sqlite', 'postgresql', 'mysql', 'mariadb', 'dolt')),
  CHECK (status IN ('succeeded', 'failed')),
  CHECK (operation_count > 0),
  CHECK (
    (NOT (database_profile = 'dolt')
      AND dolt_branch IS NULL AND dolt_head_before IS NULL AND dolt_head_after IS NULL)
    OR
    (database_profile = 'dolt'
      AND dolt_branch IS NOT NULL AND dolt_head_before IS NOT NULL
      AND (status = 'failed' OR dolt_head_after = dolt_head_before))
  )
){options};
"#;

/// Rolls back an open transaction, if any. A failure here changes nothing the caller reports.
pub(crate) fn rollback(db: &mut dyn Db) {
    let _ = db.batch("ROLLBACK");
}

/// Verifies the catalog of a connection, or installs it into an empty namespace when
/// `install` is set. Ownership: the namespace holds only the catalog relations and the data
/// tables the catalog names, and exactly one identity row with this contract and schema
/// version. Anything else fails without being touched.
pub fn prepare(db: &mut dyn Db, install: bool) -> Result<()> {
    let objects = db.objects()?;
    if objects.is_empty() {
        if !install {
            return Err(DbError::new(
                "catalog_missing",
                "the database holds no OpenStatSpec catalog",
            ));
        }
        db.write_gate()?;
        return install_catalog(db);
    }
    let foreign = |what: String| {
        Err(DbError::new(
            "catalog_foreign",
            format!("the database is not a dedicated OpenStatSpec catalog: {what}"),
        ))
    };
    if !objects.iter().any(|o| o == "catalog_identity") {
        return foreign("no catalog_identity".into());
    }
    let identity: Vec<(String, i64)> = db
        .query(
            "SELECT contract_id, schema_version FROM catalog_identity",
            &[],
        )?
        .iter()
        .map(|r| Ok((r[0].text()?, r[1].int()?)))
        .collect::<Result<_>>()?;
    if identity != [(CONTRACT_ID.to_string(), SCHEMA_VERSION)] {
        return foreign(format!("catalog identity {identity:?}"));
    }
    let data_tables: Vec<String> = db
        .query("SELECT physical_table_name FROM dataset", &[])?
        .iter()
        .map(|r| r[0].text())
        .collect::<Result<_>>()?;
    let owned = |name: &String| {
        CATALOG_TABLES.contains(&name.as_str())
            || data_tables.iter().any(|t| t.eq_ignore_ascii_case(name))
    };
    if let Some(other) = objects.iter().find(|o| !owned(o)) {
        return foreign(format!("unknown object {other}"));
    }
    if let Some(missing) = CATALOG_TABLES
        .iter()
        .find(|t| !objects.iter().any(|o| o == *t))
    {
        return foreign(format!("missing catalog table {missing}"));
    }
    Ok(())
}

/// Creates the catalog in one transaction where DDL is transactional; elsewhere each table
/// is created on its own and every created one is dropped again if a later step fails.
fn install_catalog(db: &mut dyn Db) -> Result<()> {
    let dialect = db.dialect();
    let atomic = dialect.atomic_schema_change;
    let mut created = vec![];
    let result = (|| {
        if atomic {
            db.batch(dialect.begin)?;
        }
        db.create_namespace()?;
        for (statement, table) in dialect.ddl(CATALOG_DDL).iter().zip(CATALOG_TABLES) {
            db.batch(statement)?;
            created.push(table);
        }
        db.exec(
            &format!(
                "INSERT INTO catalog_identity VALUES (1, ?, ?, {})",
                dialect.now
            ),
            args![CONTRACT_ID, SCHEMA_VERSION],
        )?;
        if atomic {
            db.batch("COMMIT")?;
        }
        Ok(())
    })();
    if result.is_err() {
        rollback(db);
        if !atomic {
            for table in created.iter().rev() {
                let _ = db.batch(&format!("DROP TABLE {}", dialect.quote(table)));
            }
        }
    }
    result
}

/// Deterministic physical names: a source name made only of ASCII letters, digits and `_`,
/// starting with a letter, is used as is unless it collides (case-insensitively) with an
/// earlier one; every other name becomes `v<ordinal>`, with `_` appended until unique.
/// All identifiers are quoted, so SQL reserved words need no renaming. `taken` holds the
/// lowercase names already in use and receives the new ones. Generated names are ASCII, so
/// their length is the same in bytes and in characters.
pub(crate) fn physical_names(
    names: &[(usize, &str)],
    taken: &mut HashSet<String>,
    identifier_limit: usize,
) -> Vec<String> {
    let mut out: Vec<Option<String>> = names
        .iter()
        .map(|(_, name)| {
            let safe = name.len() <= identifier_limit
                && name.starts_with(|c: char| c.is_ascii_alphabetic())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            (safe && taken.insert(name.to_ascii_lowercase())).then(|| name.to_string())
        })
        .collect();
    for (slot, (ordinal, _)) in out.iter_mut().zip(names) {
        if slot.is_none() {
            let mut name = format!("v{ordinal}");
            while !taken.insert(name.clone()) {
                name.push('_');
            }
            *slot = Some(name);
        }
    }
    out.into_iter()
        .map(|n| n.expect("every name set"))
        .collect()
}

/// "F8.2" gives ("F", 8, Some(2)); "A10" and "DATE11" give no decimals.
pub(crate) fn split_format(format: &str) -> (String, Option<i64>, Option<i64>) {
    let digits = format
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(format.len());
    let (family, size) = format.split_at(digits);
    let (width, decimals) = match size.split_once('.') {
        Some((w, d)) => (w, d.parse().ok()),
        None => (size, None),
    };
    (family.to_string(), width.parse().ok(), decimals)
}

const LOWEST: f64 = f64::from_bits(0xffef_ffff_ffff_fffe);
const HIGHEST: f64 = f64::MAX;

fn measure_text(m: Measure) -> Option<&'static str> {
    (m != Measure::Unknown).then(|| m.as_str())
}

/// Stores `dataset` as dataset `name`: one new wide table `data_<name>` and its catalog rows,
/// after a capability preflight. Where DDL is transactional everything is one transaction;
/// elsewhere the table is created first, the rows and catalog follow in one transaction, and
/// a failure drops the table again. Every attempt leaves an `operation` row; a failure also
/// leaves a `fidelity_event` with no dataset and nothing else. Returns the new dataset id.
pub fn write_dataset(db: &mut dyn Db, dataset: &Dataset, name: &str) -> Result<String> {
    db.write_gate()?;
    let dialect = db.dialect();
    let atomic = dialect.atomic_schema_change;
    let operation = uuid();
    let format = if dataset.compression() == "zlib" {
        "zsav"
    } else {
        "sav"
    };
    let table = format!("data_{name}");
    let mut created = false;
    let result = (|| {
        if atomic {
            db.batch(dialect.begin)?;
        }
        let physical = check_target(db, dataset, name)?;
        let mut ddl = format!(
            "CREATE TABLE {} ({} {}",
            dialect.quote(&table),
            dialect.quote("__case_ordinal"),
            dialect.ordinal_type
        );
        for (variable, column) in dataset.variables().zip(&physical) {
            let kind = if variable.is_string() {
                dialect.text_type
            } else {
                dialect.numeric_type
            };
            ddl += &format!(", {} {kind}", dialect.quote(column));
        }
        db.batch(&format!("{ddl}){}", dialect.table_options))?;
        created = true;
        if !atomic {
            db.batch(dialect.begin)?;
        }
        db.exec(
            &format!(
                "INSERT INTO operation VALUES (?, 'import', 'started', ?, {}, NULL)",
                dialect.now
            ),
            args![&operation, format],
        )?;
        let id = store(db, dataset, name, format, &physical)?;
        db.exec(
            &format!(
                "UPDATE operation SET status = 'succeeded', completed_at = {} WHERE operation_id = ?",
                dialect.now
            ),
            args![&operation],
        )?;
        db.batch("COMMIT")?;
        Ok(id)
    })();
    let e: DbError = match result {
        Ok(id) => return Ok(id),
        Err(e) => e,
    };
    rollback(db);
    if created && !atomic {
        let drop = format!("DROP TABLE {}", dialect.quote(&table));
        if let Err(fault) = db.batch(&drop) {
            let mut cleanup = DbError::new(
                "cleanup_failed",
                format!("{e}; removing {table} failed: {fault}"),
            );
            cleanup.detail = Some(Box::new(json!({
                "original_cause": {"code": e.code, "message": e.message},
                "cleanup_fault": fault.message,
                "residual_object_inventory": [table],
                "deterministic_recovery_evidence": drop,
            })));
            // the audit is written only while the catalog still verifies
            if prepare(db, false).is_ok() {
                record_failure(db, &operation, "import", Some(format), &cleanup);
            }
            return Err(cleanup);
        }
    }
    record_failure(db, &operation, "import", Some(format), &e);
    Err(e)
}

/// Writes the failed operation and its error event after the work was rolled back. A failure
/// here leaves the original error as the one reported.
fn record_failure(
    db: &mut dyn Db,
    operation: &str,
    direction: &str,
    format: Option<&str>,
    e: &DbError,
) {
    let now = db.dialect().now;
    let result = (|| -> Result<()> {
        db.batch(db.dialect().begin)?;
        db.exec(
            &format!("INSERT INTO operation VALUES (?, ?, 'failed', ?, {now}, {now})"),
            args![operation, direction, format],
        )?;
        insert_event(db, operation, None, direction, "error", e)?;
        db.batch("COMMIT")
    })();
    if result.is_err() {
        rollback(db);
    }
}

fn insert_event(
    db: &mut dyn Db,
    operation: &str,
    dataset: Option<&str>,
    direction: &str,
    severity: &str,
    e: &DbError,
) -> Result<()> {
    let mut detail = json!({"message": e.message});
    if let Some(Value::Object(extra)) = e.detail.as_deref() {
        detail
            .as_object_mut()
            .expect("object")
            .extend(extra.clone());
    }
    db.exec(
        &format!(
            "INSERT INTO fidelity_event VALUES (?, ?, ?, ?, ?, ?, ?, ?, {})",
            db.dialect().now
        ),
        args![
            uuid(),
            operation,
            dataset,
            direction,
            severity,
            e.code,
            e.source_item.as_deref(),
            detail.to_string()
        ],
    )?;
    Ok(())
}

fn check_dataset_name(name: &str) -> Result<()> {
    let ok = (1..=64).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(DbError::new(
            "dataset_name_invalid",
            format!("dataset name {name:?}: use 1 to 64 ASCII letters, digits and _"),
        ))
    }
}

/// The name, a free dataset slot and the preflight; nothing is written. Returns the
/// physical names.
fn check_target(db: &mut dyn Db, dataset: &Dataset, name: &str) -> Result<Vec<String>> {
    check_dataset_name(name)?;
    let table = format!("data_{name}");
    let clash = first(
        db,
        "SELECT dataset_name FROM dataset
         WHERE lower(dataset_name) = lower(?) OR lower(physical_table_name) = lower(?)",
        args![name, &table],
    )?;
    if clash.is_some() {
        return Err(DbError::new(
            "dataset_exists",
            format!("the database already holds a dataset {name}"),
        ));
    }
    preflight(db, dataset, &table)
}

/// Every check that needs no write: column count, identifiers, values, row size and
/// statement size against the dialect and the connection's effective limits.
fn preflight(db: &mut dyn Db, dataset: &Dataset, table: &str) -> Result<Vec<String>> {
    let dialect = db.dialect();
    let limits = db.limits()?;
    let n = dataset.variable_count();
    if n + 1 > limits.columns {
        let item = dataset.variable_at(limits.columns - 1).name();
        return Err(DbError::capability(
            item,
            "physical_columns",
            limits.columns as u64,
            n as u64 + 1,
            "columns",
        ));
    }
    if table.len() > limits.identifier {
        return Err(DbError::capability(
            table,
            "identifier",
            limits.identifier as u64,
            table.len() as u64,
            dialect.identifier_unit,
        ));
    }
    let names: Vec<(usize, &str)> = dataset
        .variable_names()
        .enumerate()
        .map(|(i, v)| (i + 1, v))
        .collect();
    let mut taken = HashSet::from(["__case_ordinal".to_string()]);
    let physical = physical_names(&names, &mut taken, limits.identifier);
    if let Some((i, p)) = physical
        .iter()
        .enumerate()
        .find(|(_, p)| p.len() > limits.identifier)
    {
        return Err(DbError::capability(
            names[i].1,
            "identifier",
            limits.identifier as u64,
            p.len() as u64,
            dialect.identifier_unit,
        ));
    }
    let cannot_store = |variable: &str, what: String, value: Value| {
        let mut e = DbError::capability(variable, "binary64", 0, 1, "values");
        e.message = format!(
            "{variable}: the {} profile cannot store {what}",
            dialect.profile
        );
        e.detail = Some(Box::new(value));
        e
    };
    for variable in dataset.variables() {
        match variable.values() {
            Values::Numeric(values) => {
                for v in values.iter().flatten() {
                    if !dialect.accepts(v) {
                        return Err(cannot_store(
                            variable.name(),
                            format!("the value {v:?}"),
                            json!({"limit": "binary64", "value": format!("{:016x}", v.to_bits())}),
                        ));
                    }
                }
            }
            Values::String(values) => {
                for (case, value) in values.iter().enumerate() {
                    let bytes = value.len() as u64;
                    if bytes > limits.value_bytes {
                        return Err(DbError::capability(
                            variable.name(),
                            "value",
                            limits.value_bytes,
                            bytes,
                            "bytes",
                        ));
                    }
                    if !dialect.text_nul && value.contains('\0') {
                        return Err(cannot_store(
                            variable.name(),
                            "U+0000 in text".into(),
                            json!({"limit": "text", "case": case + 1}),
                        ));
                    }
                }
            }
        }
    }
    // case by case, with one buffer per case instead of one per cell
    let columns: Vec<Values> = dataset.variables().map(|v| v.values()).collect();
    let (mut numeric, mut strings) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for case in 0..dataset.case_count() {
        numeric.clear();
        strings.clear();
        let mut text = 0;
        for values in &columns {
            match values {
                Values::Numeric(v) => numeric.push(v.get(case).is_some()),
                Values::String(v) => {
                    let value = v.get(case);
                    strings.push(value.len() as u64);
                    text += dialect.text_bytes(value);
                }
            }
        }
        let row = dialect.row_bytes(&numeric, &strings);
        if row > limits.row_bytes {
            return Err(DbError::capability(
                "row",
                "structural_row",
                limits.row_bytes,
                row,
                "bytes",
            ));
        }
        // the INSERT of one case; a batch of several cases is cut to the same limit
        let statement = dialect.insert_bytes(n, text);
        if statement > limits.statement_bytes {
            return Err(DbError::capability(
                "row",
                "emitted_statement",
                limits.statement_bytes,
                statement,
                "bytes",
            ));
        }
    }
    Ok(physical)
}

fn store(
    db: &mut dyn Db,
    dataset: &Dataset,
    name: &str,
    format: &str,
    physical: &[String],
) -> Result<String> {
    let dialect = db.dialect();
    let table = format!("data_{name}");
    let columns: Vec<Values> = dataset.variables().map(|v| v.values()).collect();
    let mut rows = (0..dataset.case_count()).map(|case| {
        std::iter::once(Sql::Int(case as i64 + 1))
            .chain(columns.iter().map(|values| match values {
                Values::Numeric(v) => v.get(case).map_or(Sql::Null, Sql::Real),
                Values::String(v) => Sql::Text(v.get(case).to_string()),
            }))
            .collect()
    });
    db.insert_rows(&table, &mut rows)?;

    let id = uuid();
    db.exec(
        &format!(
            "INSERT INTO dataset VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, {})",
            dialect.now
        ),
        args![
            &id,
            SPEC_VERSION,
            format,
            db.namespace(),
            &table,
            name,
            dataset.file_label(),
            dataset.encoding(),
            dataset.case_count()
        ],
    )?;
    let mut variable_ids = Vec::with_capacity(physical.len());
    for (i, (variable, column)) in dataset.variables().zip(physical).enumerate() {
        let variable_id = uuid();
        let (family, width, decimals) = split_format(variable.format());
        db.exec(
            "INSERT INTO variable VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            args![
                &variable_id,
                &id,
                i + 1,
                variable.name(),
                column,
                if variable.is_string() {
                    "string"
                } else {
                    "numeric"
                },
                variable.string_width(),
                variable.label(),
                family.as_str(),
                width,
                decimals,
                family.as_str(),
                width,
                decimals,
                measure_text(variable.measure()),
                variable.role(),
                variable.display_width(),
                variable.alignment()
            ],
        )?;
        store_value_labels(db, &id, &variable_id, variable.value_labels())?;
        store_missing(db, &variable_id, variable.missing_values())?;
        for (name, values) in variable.attributes() {
            for (k, value) in values.iter().enumerate() {
                db.exec(
                    "INSERT INTO variable_attribute VALUES (?, ?, ?, ?, ?)",
                    args![uuid(), &variable_id, name, k + 1, value],
                )?;
            }
        }
        variable_ids.push(variable_id);
    }
    let id_of = |name: &str| {
        dataset
            .variable_names()
            .position(|n| n == name)
            .map(|i| variable_ids[i].clone())
            .ok_or_else(|| DbError::new("data_invalid", format!("unknown variable {name}")))
    };
    if let Some(weight) = dataset.weight_variable() {
        db.exec(
            "INSERT INTO dataset_weight_variable VALUES (?, ?)",
            args![&id, id_of(weight)?],
        )?;
    }
    for (k, line) in dataset.documents().iter().enumerate() {
        db.exec(
            "INSERT INTO document VALUES (?, ?, ?, ?)",
            args![uuid(), &id, k + 1, line],
        )?;
    }
    for (name, values) in dataset.attributes() {
        for (k, value) in values.iter().enumerate() {
            db.exec(
                "INSERT INTO dataset_attribute VALUES (?, ?, ?, ?, ?)",
                args![uuid(), &id, name, k + 1, value],
            )?;
        }
    }
    for (k, set) in dataset.variable_sets().iter().enumerate() {
        let set_id = uuid();
        db.exec(
            "INSERT INTO variable_set VALUES (?, ?, ?, ?)",
            args![&set_id, &id, k + 1, &set.name],
        )?;
        for (m, member) in set.variables.iter().enumerate() {
            db.exec(
                "INSERT INTO variable_set_member VALUES (?, ?, ?)",
                args![&set_id, id_of(member)?, m + 1],
            )?;
        }
    }
    for (k, set) in dataset.multiple_response_sets().iter().enumerate() {
        let set_id = uuid();
        let string_set = set
            .variables
            .first()
            .and_then(|v| dataset.variable(v).ok())
            .is_some_and(|v| v.is_string());
        let (kind, counted_kind, number, text) = match (&set.counted_value, string_set) {
            (None, _) => ("MC", None, None, None),
            (Some(v), true) => ("MD", Some("string"), None, Some(v.as_str())),
            (Some(v), false) => {
                let number = v.trim().parse::<f64>().map_err(|_| {
                    DbError::new("data_invalid", format!("{}: counted value {v}", set.name))
                })?;
                ("MD", Some("numeric"), Some(number), None)
            }
        };
        let category_labels = if set.counted_value_labels {
            "counted_values"
        } else {
            "variable_labels"
        };
        let label_source = if set.label_from_variables {
            "variable_label"
        } else {
            "set_label"
        };
        db.exec(
            "INSERT INTO multiple_response_set VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            args![
                &set_id,
                &id,
                k + 1,
                &set.name,
                &set.label,
                kind,
                counted_kind,
                number,
                text,
                category_labels,
                label_source
            ],
        )?;
        for (m, member) in set.variables.iter().enumerate() {
            db.exec(
                "INSERT INTO multiple_response_member VALUES (?, ?, ?)",
                args![&set_id, id_of(member)?, m + 1],
            )?;
        }
    }
    Ok(id)
}

pub(crate) fn store_value_labels(
    db: &mut dyn Db,
    dataset_id: &str,
    variable_id: &str,
    labels: &[(statscraft_data::Value, String)],
) -> Result<()> {
    if labels.is_empty() {
        return Ok(());
    }
    let set_id = uuid();
    db.exec(
        "INSERT INTO value_label_set VALUES (?, ?, NULL)",
        args![&set_id, dataset_id],
    )?;
    for (k, (value, label)) in labels.iter().enumerate() {
        let (kind, number, text) = match value {
            statscraft_data::Value::Number(v) => ("numeric", Some(*v), None),
            statscraft_data::Value::String(s) => ("string", None, Some(s.as_str())),
        };
        db.exec(
            "INSERT INTO value_label VALUES (?, ?, ?, ?, ?, ?, ?)",
            args![uuid(), &set_id, k + 1, kind, number, text, label],
        )?;
    }
    db.exec(
        "INSERT INTO variable_value_label_set VALUES (?, ?)",
        args![variable_id, &set_id],
    )?;
    Ok(())
}

fn store_missing(db: &mut dyn Db, variable_id: &str, missing: &MissingValues) -> Result<()> {
    let discrete = |db: &mut dyn Db, ordinal: usize, number: Option<f64>, text: Option<&str>| {
        let kind = if text.is_some() { "string" } else { "numeric" };
        db.exec(
            "INSERT INTO missing_rule VALUES (?, ?, ?, 'discrete', ?, ?, ?, NULL, NULL, NULL, NULL)",
            args![uuid(), variable_id, ordinal, kind, number, text],
        )
    };
    match missing {
        MissingValues::None => {}
        MissingValues::Numbers(values) => {
            for (k, v) in values.iter().enumerate() {
                discrete(db, k + 1, Some(*v), None)?;
            }
        }
        MissingValues::Strings(values) => {
            for (k, v) in values.iter().enumerate() {
                discrete(db, k + 1, None, Some(v))?;
            }
        }
        MissingValues::Range { low, high, extra } => {
            // a LOWEST or HIGHEST endpoint is named, and its numeric column stays NULL
            let lower = (*low != LOWEST).then_some(*low);
            let upper = (*high != HIGHEST).then_some(*high);
            db.exec(
                "INSERT INTO missing_rule VALUES (?, ?, 1, 'numeric_range', NULL, NULL, NULL, ?, ?, ?, ?)",
                args![
                    uuid(),
                    variable_id,
                    lower,
                    upper,
                    lower.is_none().then_some("LOWEST"),
                    upper.is_none().then_some("HIGHEST")
                ],
            )?;
            if let Some(v) = extra {
                discrete(db, 2, Some(*v), None)?;
            }
        }
    }
    Ok(())
}

/// One catalog variable of a dataset.
#[derive(Clone, Debug)]
pub(crate) struct CatalogVariable {
    pub id: String,
    pub name: String,
    pub physical: String,
    /// `None` for a numeric variable.
    pub string_width: Option<usize>,
}

/// The dataset id and physical table of dataset `name`.
pub(crate) fn find_dataset(db: &mut dyn Db, name: &str) -> Result<(String, String)> {
    let row = first(
        db,
        "SELECT dataset_id, physical_table_name FROM dataset WHERE lower(dataset_name) = lower(?)",
        args![name],
    )?
    .ok_or_else(|| DbError::new("dataset_unknown", format!("no dataset named {name}")))?;
    Ok((row[0].text()?, row[1].text()?))
}

pub(crate) fn catalog_variables(db: &mut dyn Db, dataset_id: &str) -> Result<Vec<CatalogVariable>> {
    db.query(
        "SELECT variable_id, source_name, physical_name, storage_kind, declared_string_width
         FROM variable WHERE dataset_id = ? ORDER BY source_ordinal",
        args![dataset_id],
    )?
    .iter()
    .map(|r| {
        Ok(CatalogVariable {
            id: r[0].text()?,
            name: r[1].text()?,
            physical: r[2].text()?,
            string_width: match r[3].text()?.as_str() {
                "string" => Some(r[4].opt_int()?.unwrap_or(0) as usize),
                _ => None,
            },
        })
    })
    .collect()
}

/// Value labels of a variable, in ordinal order.
fn value_labels(
    db: &mut dyn Db,
    variable_id: &str,
) -> Result<Vec<(statscraft_data::Value, String)>> {
    db.query(
        "SELECT l.code_kind, l.numeric_code, l.string_code, l.label
         FROM variable_value_label_set s
         JOIN value_label l ON l.value_label_set_id = s.value_label_set_id
         WHERE s.variable_id = ? ORDER BY l.ordinal",
        args![variable_id],
    )?
    .iter()
    .map(|r| {
        let value = if r[0].text()? == "string" {
            statscraft_data::Value::String(r[2].text()?)
        } else {
            statscraft_data::Value::Number(r[1].real()?)
        };
        Ok((value, r[3].text()?))
    })
    .collect()
}

fn missing_values(db: &mut dyn Db, variable_id: &str) -> Result<MissingValues> {
    let rules = db.query(
        "SELECT rule_kind, numeric_value, string_value, numeric_lower, numeric_upper,
                lower_special, upper_special
         FROM missing_rule WHERE variable_id = ? ORDER BY ordinal",
        args![variable_id],
    )?;
    let (mut numbers, mut strings, mut range) = (vec![], vec![], None);
    for r in rules {
        let bad = || DbError::new("catalog_invalid", format!("missing_rule of {variable_id}"));
        if r[0].text()? == "numeric_range" {
            let low = match r[5].opt_text()?.as_deref() {
                Some("LOWEST") => LOWEST,
                _ => r[3].opt_real()?.ok_or_else(bad)?,
            };
            let high = match r[6].opt_text()?.as_deref() {
                Some("HIGHEST") => HIGHEST,
                _ => r[4].opt_real()?.ok_or_else(bad)?,
            };
            range = Some((low, high));
        } else if let Some(text) = r[2].opt_text()? {
            strings.push(text);
        } else {
            numbers.push(r[1].opt_real()?.ok_or_else(bad)?);
        }
    }
    Ok(match range {
        Some((low, high)) => MissingValues::Range {
            low,
            high,
            extra: numbers.first().copied(),
        },
        None if !strings.is_empty() => MissingValues::Strings(strings),
        None if !numbers.is_empty() => MissingValues::Numbers(numbers),
        None => MissingValues::None,
    })
}

/// Reads dataset `name` back: rows by `__case_ordinal`, variables by `source_ordinal`, with
/// the dictionary from the catalog. Catalog content the dataset model cannot hold (dataset
/// attributes, variable sets, a write format other than the print format, the category label
/// and label source settings of a multiple response set) fails with `export_fidelity_loss`
/// before anything is returned, unless `allow_loss` is set. Returns the dataset and the items
/// not exported.
///
/// The attempt is recorded as an export operation, and each allowed loss as a warning event,
/// except on a dialect with `read_only_export` (Dolt), where reading writes nothing and the
/// losses travel only in the result.
pub fn read_dataset(
    db: &mut dyn Db,
    name: &str,
    allow_loss: bool,
) -> Result<(Dataset, Vec<String>)> {
    let fidelity = |losses: &[String]| {
        let mut e = DbError::new(
            "export_fidelity_loss",
            format!("the dataset model cannot hold: {}", losses.join(", ")),
        );
        e.detail = Some(Box::new(json!({"losses": losses})));
        e
    };
    let dialect = db.dialect();
    if dialect.read_only_export {
        let (dataset, losses, _) = load(db, name)?;
        if !losses.is_empty() && !allow_loss {
            return Err(fidelity(&losses));
        }
        return Ok((dataset, losses));
    }
    let operation = uuid();
    let result = (|| {
        db.batch(dialect.begin)?;
        db.exec(
            &format!(
                "INSERT INTO operation VALUES (?, 'export', 'started', NULL, {}, NULL)",
                dialect.now
            ),
            args![&operation],
        )?;
        let (dataset, losses, dataset_id) = load(db, name)?;
        if !losses.is_empty() && !allow_loss {
            return Err(fidelity(&losses));
        }
        for loss in &losses {
            let mut e = DbError::new("export_loss_allowed", format!("not exported: {loss}"));
            e.source_item = Some(loss.clone());
            insert_event(db, &operation, Some(&dataset_id), "export", "warning", &e)?;
        }
        let source_format = first(
            db,
            "SELECT source_format FROM dataset WHERE dataset_id = ?",
            args![&dataset_id],
        )?
        .map(|r| r[0].clone())
        .unwrap_or(Sql::Null);
        db.exec(
            &format!(
                "UPDATE operation SET status = 'succeeded', source_format = ?, completed_at = {}
                 WHERE operation_id = ?",
                dialect.now
            ),
            &[source_format, Sql::from(&operation)],
        )?;
        db.batch("COMMIT")?;
        Ok((dataset, losses))
    })();
    result.inspect_err(|e| {
        rollback(db);
        record_failure(db, &operation, "export", None, e)
    })
}

/// Attribute rows in insertion order: the catalog has no ordinal for attribute names, and
/// the ids are time-ordered.
fn attributes(db: &mut dyn Db, sql: &str, id: &str) -> Result<Vec<(String, Vec<String>)>> {
    let mut attributes: Vec<(String, Vec<String>)> = vec![];
    for r in db.query(sql, args![id])? {
        let (name, value) = (r[0].text()?, r[1].text()?);
        match attributes.iter_mut().find(|(n, _)| *n == name) {
            Some((_, values)) => values.push(value),
            None => attributes.push((name, vec![value])),
        }
    }
    Ok(attributes)
}

/// The dataset, the catalog items it could not take, and the dataset id.
fn load(db: &mut dyn Db, name: &str) -> Result<(Dataset, Vec<String>, String)> {
    let dialect = db.dialect();
    let (id, table) = find_dataset(db, name)?;
    let row = first(
        db,
        "SELECT source_format, dataset_label, source_encoding, source_case_count
         FROM dataset WHERE dataset_id = ?",
        args![&id],
    )?
    .ok_or_else(|| DbError::new("dataset_unknown", format!("no dataset named {name}")))?;
    let (format, label, encoding, cases) = (
        row[0].text()?,
        row[1].opt_text()?,
        row[2].opt_text()?,
        row[3].int()?,
    );
    let mut losses = vec![];

    let variables = catalog_variables(db, &id)?;
    let ordinal = dialect.quote("__case_ordinal");
    let mut select = format!("SELECT {ordinal}");
    for v in &variables {
        select += &format!(", {}", dialect.quote(&v.physical));
    }
    select += &format!(" FROM {} ORDER BY {ordinal}", dialect.quote(&table));
    // the columns grow row by row as the rows stream in; the catalog's case count sizes them
    // (bounded, in case the catalog is wrong)
    let reserve = (cases.max(0) as usize).min(1 << 24);
    let column = |string: bool, wanted: bool| if string == wanted { reserve } else { 0 };
    let mut numbers: Vec<Vec<Option<f64>>> = variables
        .iter()
        .map(|v| Vec::with_capacity(column(v.string_width.is_some(), false)))
        .collect();
    let mut strings: Vec<Vec<String>> = variables
        .iter()
        .map(|v| Vec::with_capacity(column(v.string_width.is_some(), true)))
        .collect();
    let mut count = 0i64;
    db.query_each(&select, &[], &mut |row| {
        count += 1;
        let mut cells = row.into_iter();
        if cells.next().map(|c| c.int()) != Some(Ok(count)) {
            return Err(DbError::new(
                "data_invalid",
                format!("{table}: __case_ordinal does not run 1, 2, 3, ... at case {count}"),
            ));
        }
        for ((i, v), cell) in variables.iter().enumerate().zip(cells) {
            let invalid = |e: DbError| {
                DbError::new(
                    "data_invalid",
                    format!("{table}.{} case {count}: {}", v.physical, e.message),
                )
            };
            match (v.string_width, cell) {
                (None, cell) => numbers[i].push(cell.opt_real().map_err(invalid)?),
                (Some(_), Sql::Text(text)) => strings[i].push(text),
                (Some(_), cell) => return Err(invalid(cell.text().unwrap_err())),
            }
        }
        Ok(())
    })?;
    if count != cases {
        return Err(DbError::new(
            "data_invalid",
            format!("{table} has {count} rows, the catalog says {cases}"),
        ));
    }

    let mut dataset = Dataset::new(cases as usize);
    for (i, v) in variables.iter().enumerate() {
        match v.string_width {
            None => dataset.set_numeric_values(&v.name, std::mem::take(&mut numbers[i]))?,
            Some(width) => {
                dataset.set_string_values(&v.name, width, std::mem::take(&mut strings[i]))?
            }
        }
        let m = first(
            db,
            "SELECT variable_label, print_format_family, print_format_width, print_format_decimals,
                    write_format_family, write_format_width, write_format_decimals,
                    measurement_level, variable_role, display_width, display_alignment
             FROM variable WHERE variable_id = ?",
            args![&v.id],
        )?
        .ok_or_else(|| DbError::new("catalog_invalid", format!("variable {}", v.id)))?;
        let (family, width, decimals) = (m[1].opt_text()?, m[2].opt_int()?, m[3].opt_int()?);
        let write = (m[4].opt_text()?, m[5].opt_int()?, m[6].opt_int()?);
        if (&family, width, decimals) != (&write.0, write.1, write.2) {
            losses.push(format!("{}: write_format", v.name));
        }
        dataset.set_variable_label(&v.name, m[0].opt_text()?.as_deref())?;
        if let (Some(family), Some(width), None) = (&family, width, v.string_width) {
            let format = match decimals {
                Some(d) => format!("{family}{width}.{d}"),
                None => format!("{family}{width}"),
            };
            dataset.set_format(&v.name, &format)?;
        }
        let measure = match m[7].opt_text()?.as_deref() {
            Some("nominal") => Measure::Nominal,
            Some("ordinal") => Measure::Ordinal,
            Some("scale") => Measure::Scale,
            _ => Measure::Unknown,
        };
        dataset.set_measure(&v.name, measure)?;
        let labels = value_labels(db, &v.id)?;
        dataset.set_value_labels(&v.name, labels)?;
        let missing = missing_values(db, &v.id)?;
        dataset.set_missing_values(&v.name, missing)?;
        let display = m[9].opt_int()?.map(|w| w as u32);
        dataset.set_display(
            &v.name,
            display,
            m[10].opt_text()?.as_deref(),
            m[8].opt_text()?.as_deref(),
        )?;
        let attributes = attributes(
            db,
            "SELECT attribute_name, attribute_value FROM variable_attribute
             WHERE variable_id = ? ORDER BY variable_attribute_id",
            &v.id,
        )?;
        dataset.set_variable_attributes(&v.name, attributes)?;
    }
    let name_of = |variable_id: &str| {
        variables
            .iter()
            .find(|v| v.id == variable_id)
            .map(|v| v.name.clone())
            .ok_or_else(|| {
                DbError::new("catalog_invalid", format!("unknown variable {variable_id}"))
            })
    };
    if let Some(weight) = first(
        db,
        "SELECT variable_id FROM dataset_weight_variable WHERE dataset_id = ?",
        args![&id],
    )? {
        dataset.set_weight_variable(Some(&name_of(&weight[0].text()?)?))?;
    }
    dataset.set_attributes(attributes(
        db,
        "SELECT attribute_name, attribute_value FROM dataset_attribute
         WHERE dataset_id = ? ORDER BY dataset_attribute_id",
        &id,
    )?);
    let members = |db: &mut dyn Db, table: &str, key: &str, set_id: &str| -> Result<Vec<String>> {
        db.query(
            &format!("SELECT variable_id FROM {table} WHERE {key} = ? ORDER BY source_ordinal"),
            args![set_id],
        )?
        .iter()
        .map(|r| name_of(&r[0].text()?))
        .collect()
    };
    let mut sets = vec![];
    for r in db.query(
        "SELECT variable_set_id, set_name FROM variable_set
         WHERE dataset_id = ? ORDER BY source_ordinal",
        args![&id],
    )? {
        let variables = members(db, "variable_set_member", "variable_set_id", &r[0].text()?)?;
        sets.push(VariableSet {
            name: r[1].text()?,
            variables,
        });
    }
    dataset.set_variable_sets(sets)?;
    for r in db.query(
        "SELECT multiple_response_set_id, set_name, set_label, set_kind,
                counted_numeric_value, counted_string_value,
                category_label_behavior, label_source
         FROM multiple_response_set WHERE dataset_id = ? ORDER BY source_ordinal",
        args![&id],
    )? {
        let variables = members(
            db,
            "multiple_response_member",
            "multiple_response_set_id",
            &r[0].text()?,
        )?;
        let counted_value = match (r[4].opt_real()?, r[5].opt_text()?) {
            (Some(v), _) if v.fract() == 0.0 && v.abs() < 9.0e15 => Some((v as i64).to_string()),
            (Some(v), _) => Some(v.to_string()),
            (None, text) => text,
        };
        dataset.set_multiple_response_set(MultipleResponseSet {
            name: r[1].text()?,
            kind: if r[3].text()? == "MD" {
                "dichotomy"
            } else {
                "category"
            },
            label: r[2].opt_text()?.unwrap_or_default(),
            counted_value,
            variables,
            counted_value_labels: r[6].opt_text()?.as_deref() == Some("counted_values"),
            label_from_variables: r[7].opt_text()?.as_deref() == Some("variable_label"),
        })?;
    }
    let documents = db
        .query(
            "SELECT document_text FROM document WHERE dataset_id = ? ORDER BY source_ordinal",
            args![&id],
        )?
        .iter()
        .map(|r| r[0].text())
        .collect::<Result<_>>()?;
    dataset.set_documents(documents);
    dataset.set_file_label(label.as_deref());
    // the catalog keeps "sav" or "zsav", not the compression inside a sav file: a sav dataset
    // comes back bytecode-compressed, the way StatsCraft writes it
    let compression = if format == "zsav" { "zlib" } else { "bytecode" };
    dataset.set_source(encoding.as_deref().unwrap_or("UTF-8"), compression)?;
    Ok((dataset, losses, id))
}

/// The input schema of the frontend request for dataset `name`, from the catalog. The format
/// and measurement fields exist from frontend profile 0.2 on.
pub fn input_schema(db: &mut dyn Db, name: &str, with_format: bool) -> Result<Value> {
    let (id, _) = find_dataset(db, name)?;
    let mut variables = vec![];
    for v in catalog_variables(db, &id)? {
        let m = first(
            db,
            "SELECT variable_label, print_format_family, print_format_width,
                    print_format_decimals, measurement_level
             FROM variable WHERE variable_id = ?",
            args![&v.id],
        )?
        .ok_or_else(|| DbError::new("catalog_invalid", format!("variable {}", v.id)))?;
        let labels: Vec<Value> = value_labels(db, &v.id)?
            .into_iter()
            .map(|(value, label)| {
                let value = match value {
                    // positive zero is the canonical zero
                    statscraft_data::Value::Number(n) => json!({
                        "type": "binary64",
                        "bits": format!("{:016x}", if n == 0.0 { 0 } else { n.to_bits() }),
                    }),
                    statscraft_data::Value::String(s) => json!({"type": "string", "value": s}),
                };
                json!({"value": value, "label": label})
            })
            .collect();
        let mut out = json!({
            "name": v.name,
            "storage_kind": if v.string_width.is_some() { "string" } else { "numeric" },
            "variable_label": m[0].opt_text()?,
            "value_labels": labels,
        });
        if with_format {
            out["format_family"] = json!(m[1].opt_text()?);
            out["width"] = json!(m[2].opt_int()?);
            out["decimals"] = json!(m[3].opt_int()?);
            out["measurement_level"] = json!(m[4].opt_text()?);
        }
        variables.push(out);
    }
    Ok(json!({ "variables": variables }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SQLITE;

    #[test]
    fn physical_names_are_deterministic_and_unique() {
        let names = [
            (1, "age"),
            (2, "AGE2"),
            (3, "v4"),
            (4, "linn_õ"),
            (5, "x.y"),
            (6, "Age"),
        ];
        let mut taken = HashSet::from(["__case_ordinal".to_string()]);
        let got = physical_names(&names, &mut taken, 255);
        // "Age" folds onto "age"; v4 is a source name, so ordinal 4 gets v4_
        assert_eq!(got, ["age", "AGE2", "v4", "v4_", "v5", "v6"]);
    }

    #[test]
    fn catalog_ddl_has_one_statement_per_catalog_table() {
        for dialect in [
            &SQLITE,
            &crate::db::POSTGRESQL,
            &crate::db::MYSQL,
            &crate::db::DOLT,
        ] {
            let ddl = dialect.ddl(CATALOG_DDL);
            assert_eq!(ddl.len(), CATALOG_TABLES.len());
            for (statement, table) in ddl.iter().zip(CATALOG_TABLES) {
                assert!(statement.starts_with(&format!("CREATE TABLE {table} (")));
                assert!(!statement.contains("{id}") && !statement.contains("{options}"));
            }
        }
    }
}
