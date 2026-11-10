//! SQLite binding of the SAV SAV/ZSAV profile 1.0: one dataset is one wide table plus the
//! normative catalog, in a database file dedicated to OpenStatSpec.
//!
//! The specification calls SAV to SQL "import" and SQL to SAV "export"; the operation and
//! fidelity rows use those words. In this crate `write_dataset` is the spec import and
//! `read_dataset` the spec export.

use rusqlite::types::Value as Sql;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Value, json};
use statscraft_data::{Dataset, Measure, MissingValues, MultipleResponseSet, Values};
use std::collections::HashSet;
use std::path::Path;

pub const CONTRACT_ID: &str = "openstatspec-strict-wide-table-v1";
/// Version of the catalog DDL below. The specification does not number catalog schemas.
pub const SCHEMA_VERSION: i64 = 1;
/// Written to `dataset.spec_version`: the profile the dataset follows.
pub const SPEC_VERSION: &str = "spss-sav-zsav-1.0";

/// The physical choices of one SQL dialect profile. Only SQLite is implemented; another
/// profile adds a value here and its own statements where SQLite syntax is used directly.
pub struct Dialect {
    /// The name `transformation_apply.database_profile` records.
    pub profile: &'static str,
    /// Inclusive physical column limit, `__case_ordinal` included.
    pub max_columns: usize,
    /// Identifier limit in bytes of the ASCII names this binding generates.
    pub identifier_bytes: usize,
    pub numeric_type: &'static str,
    pub text_type: &'static str,
    /// True when ALTER TABLE, data, catalog and audit commit or roll back together.
    pub atomic_schema_change: bool,
}

pub const SQLITE: Dialect = Dialect {
    profile: "sqlite",
    max_columns: 2000,
    identifier_bytes: 255,
    numeric_type: "REAL",
    text_type: "TEXT NOT NULL",
    atomic_schema_change: true,
};

impl Dialect {
    pub fn quote(&self, identifier: &str) -> String {
        format!("\"{}\"", identifier.replace('"', "\"\""))
    }
}

/// A failure with a stable code. `source_item` and `detail` go into the fidelity event.
#[derive(Clone, Debug, PartialEq)]
pub struct DbError {
    pub code: &'static str,
    pub message: String,
    pub source_item: Option<String>,
    /// Machine-readable details, merged into `detail_json`.
    pub detail: Option<Box<Value>>,
}

impl DbError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        DbError {
            code,
            message: message.into(),
            source_item: None,
            detail: None,
        }
    }

    fn capability(item: &str, limit: &str, maximum: u64, required: u64, unit: &str) -> Self {
        DbError {
            code: "target_capability_exceeded",
            message: format!("{item}: {limit} needs {required} {unit}, the limit is {maximum}"),
            source_item: Some(item.to_string()),
            detail: Some(Box::new(
                json!({"limit": limit, "maximum": maximum, "required": required, "unit": unit}),
            )),
        }
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::new("database_error", e.to_string())
    }
}

impl From<statscraft_data::Error> for DbError {
    fn from(e: statscraft_data::Error) -> Self {
        DbError::new("data_invalid", e.to_string())
    }
}

impl From<crate::PlanError> for DbError {
    fn from(e: crate::PlanError) -> Self {
        DbError::new(e.code, e.message)
    }
}

pub type Result<T> = std::result::Result<T, DbError>;

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

/// `sql/schema-outline.sql` and `sql/transformation-plan-profile-schema.sql` in SQLite types:
/// UUID and TIMESTAMP are TEXT, DOUBLE PRECISION is REAL, BIGINT is INTEGER.
const CATALOG_DDL: &str = r#"
CREATE TABLE catalog_identity (
  catalog_identity_key INTEGER PRIMARY KEY CHECK (catalog_identity_key = 1),
  contract_id TEXT NOT NULL UNIQUE CHECK (contract_id = 'openstatspec-strict-wide-table-v1'),
  schema_version INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE dataset (
  dataset_id TEXT PRIMARY KEY,
  spec_version TEXT NOT NULL,
  source_format TEXT NOT NULL,
  physical_table_schema TEXT NULL,
  physical_table_name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  dataset_name TEXT NULL UNIQUE COLLATE NOCASE,
  dataset_label TEXT NULL,
  source_encoding TEXT NULL,
  source_hash TEXT NULL,
  source_case_count INTEGER NOT NULL,
  imported_at TEXT NOT NULL
);
CREATE TABLE operation (
  operation_id TEXT PRIMARY KEY,
  operation_kind TEXT NOT NULL CHECK (operation_kind IN ('import', 'export')),
  status TEXT NOT NULL CHECK (status IN ('started', 'succeeded', 'failed')),
  source_format TEXT NULL,
  started_at TEXT NOT NULL,
  completed_at TEXT NULL
);
CREATE TABLE variable (
  variable_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal INTEGER NOT NULL,
  source_name TEXT NOT NULL,
  physical_name TEXT NOT NULL COLLATE NOCASE,
  storage_kind TEXT NOT NULL CHECK (storage_kind IN ('numeric', 'string')),
  declared_string_width INTEGER NULL,
  variable_label TEXT NULL,
  print_format_family TEXT NULL,
  print_format_width INTEGER NULL,
  print_format_decimals INTEGER NULL,
  write_format_family TEXT NULL,
  write_format_width INTEGER NULL,
  write_format_decimals INTEGER NULL,
  measurement_level TEXT NULL,
  variable_role TEXT NULL,
  display_width INTEGER NULL,
  display_alignment TEXT NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, source_name),
  UNIQUE (dataset_id, physical_name)
);
CREATE TABLE dataset_weight_variable (
  dataset_id TEXT PRIMARY KEY REFERENCES dataset(dataset_id),
  variable_id TEXT NOT NULL UNIQUE REFERENCES variable(variable_id)
);
CREATE TABLE value_label_set (
  value_label_set_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  name TEXT NULL
);
CREATE TABLE value_label (
  value_label_id TEXT PRIMARY KEY,
  value_label_set_id TEXT NOT NULL REFERENCES value_label_set(value_label_set_id),
  ordinal INTEGER NOT NULL,
  code_kind TEXT NOT NULL CHECK (code_kind IN ('numeric', 'string')),
  numeric_code REAL NULL,
  string_code TEXT NULL,
  label TEXT NOT NULL,
  UNIQUE (value_label_set_id, ordinal)
);
CREATE TABLE variable_value_label_set (
  variable_id TEXT PRIMARY KEY REFERENCES variable(variable_id),
  value_label_set_id TEXT NOT NULL REFERENCES value_label_set(value_label_set_id)
);
CREATE TABLE missing_rule (
  missing_rule_id TEXT PRIMARY KEY,
  variable_id TEXT NOT NULL REFERENCES variable(variable_id),
  ordinal INTEGER NOT NULL,
  rule_kind TEXT NOT NULL CHECK (rule_kind IN ('discrete', 'numeric_range')),
  code_kind TEXT NULL,
  numeric_value REAL NULL,
  string_value TEXT NULL,
  numeric_lower REAL NULL,
  numeric_upper REAL NULL,
  lower_special TEXT NULL,
  upper_special TEXT NULL,
  UNIQUE (variable_id, ordinal)
);
CREATE TABLE dataset_attribute (
  dataset_attribute_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  attribute_name TEXT NOT NULL,
  array_ordinal INTEGER NOT NULL DEFAULT 1,
  attribute_value TEXT NOT NULL,
  UNIQUE (dataset_id, attribute_name, array_ordinal)
);
CREATE TABLE variable_attribute (
  variable_attribute_id TEXT PRIMARY KEY,
  variable_id TEXT NOT NULL REFERENCES variable(variable_id),
  attribute_name TEXT NOT NULL,
  array_ordinal INTEGER NOT NULL DEFAULT 1,
  attribute_value TEXT NOT NULL,
  UNIQUE (variable_id, attribute_name, array_ordinal)
);
CREATE TABLE document (
  document_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal INTEGER NOT NULL,
  document_text TEXT NOT NULL,
  UNIQUE (dataset_id, source_ordinal)
);
CREATE TABLE variable_set (
  variable_set_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal INTEGER NOT NULL,
  set_name TEXT NOT NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, set_name)
);
CREATE TABLE variable_set_member (
  variable_set_id TEXT NOT NULL REFERENCES variable_set(variable_set_id),
  variable_id TEXT NOT NULL REFERENCES variable(variable_id),
  source_ordinal INTEGER NOT NULL,
  PRIMARY KEY (variable_set_id, source_ordinal),
  UNIQUE (variable_set_id, variable_id)
);
CREATE TABLE multiple_response_set (
  multiple_response_set_id TEXT PRIMARY KEY,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  source_ordinal INTEGER NOT NULL,
  set_name TEXT NOT NULL,
  set_label TEXT NULL,
  set_kind TEXT NOT NULL CHECK (set_kind IN ('MD', 'MC')),
  counted_value_kind TEXT NULL,
  counted_numeric_value REAL NULL,
  counted_string_value TEXT NULL,
  category_label_behavior TEXT NULL,
  label_source TEXT NULL,
  UNIQUE (dataset_id, source_ordinal),
  UNIQUE (dataset_id, set_name)
);
CREATE TABLE multiple_response_member (
  multiple_response_set_id TEXT NOT NULL REFERENCES multiple_response_set(multiple_response_set_id),
  variable_id TEXT NOT NULL REFERENCES variable(variable_id),
  source_ordinal INTEGER NOT NULL,
  PRIMARY KEY (multiple_response_set_id, source_ordinal),
  UNIQUE (multiple_response_set_id, variable_id)
);
CREATE TABLE fidelity_event (
  fidelity_event_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operation(operation_id),
  dataset_id TEXT NULL REFERENCES dataset(dataset_id),
  direction TEXT NOT NULL CHECK (direction IN ('import', 'export')),
  severity TEXT NOT NULL CHECK (severity IN ('error', 'warning')),
  event_code TEXT NOT NULL,
  source_item TEXT NULL,
  detail_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE transformation_apply (
  apply_id TEXT PRIMARY KEY,
  contract_id TEXT NOT NULL,
  database_profile TEXT NOT NULL,
  dataset_id TEXT NOT NULL REFERENCES dataset(dataset_id),
  physical_table_schema TEXT NULL,
  physical_table_name TEXT NOT NULL,
  source_hash TEXT NOT NULL CHECK (length(source_hash) = 64 AND source_hash NOT GLOB '*[^0-9a-f]*'),
  plan_hash TEXT NOT NULL CHECK (length(plan_hash) = 64 AND plan_hash NOT GLOB '*[^0-9a-f]*'),
  canonical_plan_json TEXT NOT NULL,
  actor TEXT NOT NULL,
  status TEXT NOT NULL,
  dolt_branch TEXT NULL,
  dolt_head_before TEXT NULL,
  dolt_head_after TEXT NULL,
  operation_count INTEGER NOT NULL,
  started_at TEXT NOT NULL,
  completed_at TEXT NOT NULL,
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
);
"#;

pub(crate) const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// Opens a database file dedicated to the catalog. With `create`, a missing or empty file gets
/// the catalog installed; without it the file must exist and already hold the catalog. Any
/// other content fails without being touched.
pub fn open(path: &Path, create: bool) -> Result<Connection> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let mut conn = Connection::open_with_flags(path, flags)
        .map_err(|e| DbError::new("database_unavailable", format!("{}: {e}", path.display())))?;
    prepare(&mut conn, create)?;
    Ok(conn)
}

/// Verifies the catalog of a connection, or installs it into an empty database when `install`
/// is set. Ownership: the database holds only the catalog relations and the data tables the
/// catalog names, and exactly one identity row with this contract and schema version.
pub fn prepare(conn: &mut Connection, install: bool) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let objects: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type IN ('table', 'view', 'trigger')")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    if objects.is_empty() {
        if !install {
            return Err(DbError::new(
                "catalog_missing",
                "the database holds no OpenStatSpec catalog",
            ));
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(CATALOG_DDL)?;
        tx.execute(
            &format!("INSERT INTO catalog_identity VALUES (1, ?1, ?2, {NOW})"),
            params![CONTRACT_ID, SCHEMA_VERSION],
        )?;
        return Ok(tx.commit()?);
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
    let identity: Vec<(String, i64)> = conn
        .prepare("SELECT contract_id, schema_version FROM catalog_identity")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    if identity != [(CONTRACT_ID.to_string(), SCHEMA_VERSION)] {
        return foreign(format!("catalog identity {identity:?}"));
    }
    let data_tables: Vec<String> = conn
        .prepare("SELECT physical_table_name FROM dataset")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
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

/// A random version 4 UUID from SQLite's random source.
pub(crate) fn uuid(conn: &Connection) -> Result<String> {
    let mut b: Vec<u8> = conn.query_row("SELECT randomblob(16)", [], |r| r.get(0))?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

/// Deterministic physical names: a source name made only of ASCII letters, digits and `_`,
/// starting with a letter, is used as is unless it collides (case-insensitively) with an
/// earlier one; every other name becomes `v<ordinal>`, with `_` appended until unique.
/// All identifiers are quoted, so SQL reserved words need no renaming. `taken` holds the
/// lowercase names already in use and receives the new ones.
pub(crate) fn physical_names(
    names: &[(usize, &str)],
    taken: &mut HashSet<String>,
    dialect: &Dialect,
) -> Vec<String> {
    let mut out: Vec<Option<String>> = names
        .iter()
        .map(|(_, name)| {
            let safe = name.len() <= dialect.identifier_bytes
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
/// in one transaction after a capability preflight. Every attempt leaves an `operation` row;
/// a failure also leaves a `fidelity_event` with no dataset and nothing else. Returns the new
/// dataset id.
pub fn write_dataset(conn: &mut Connection, dataset: &Dataset, name: &str) -> Result<String> {
    let operation = uuid(conn)?;
    let format = if dataset.compression() == "zlib" {
        "zsav"
    } else {
        "sav"
    };
    let result = (|| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            &format!("INSERT INTO operation VALUES (?1, 'import', 'started', ?2, {NOW}, NULL)"),
            params![operation, format],
        )?;
        let id = store(&tx, dataset, name, format)?;
        tx.execute(
            &format!(
                "UPDATE operation SET status = 'succeeded', completed_at = {NOW} WHERE operation_id = ?1"
            ),
            [&operation],
        )?;
        tx.commit()?;
        Ok(id)
    })();
    result.inspect_err(|e| record_failure(conn, &operation, "import", Some(format), e))
}

/// Writes the failed operation and its error event after the work was rolled back. A failure
/// here leaves the original error as the one reported.
fn record_failure(
    conn: &mut Connection,
    operation: &str,
    direction: &str,
    format: Option<&str>,
    e: &DbError,
) {
    let _ = (|| -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            &format!("INSERT INTO operation VALUES (?1, ?2, 'failed', ?3, {NOW}, {NOW})"),
            params![operation, direction, format],
        )?;
        insert_event(&tx, operation, None, direction, "error", e)?;
        Ok(tx.commit()?)
    })();
}

fn insert_event(
    tx: &Transaction,
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
    tx.execute(
        &format!("INSERT INTO fidelity_event VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, {NOW})"),
        params![
            uuid(tx)?,
            operation,
            dataset,
            direction,
            severity,
            e.code,
            e.source_item,
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

/// Every check that needs no write: names, column count, identifiers, values and row size
/// against the dialect and the connection's effective limits. Returns the physical names.
fn preflight(conn: &Connection, dataset: &Dataset, dialect: &Dialect) -> Result<Vec<String>> {
    let columns = dialect
        .max_columns
        .min(conn.limit(rusqlite::limits::Limit::SQLITE_LIMIT_COLUMN)? as usize);
    let length = conn.limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH)? as u64;
    let n = dataset.variable_count();
    if n + 1 > columns {
        let item = dataset.variable_at(columns - 1).name();
        return Err(DbError::capability(
            item,
            "physical_columns",
            columns as u64,
            n as u64 + 1,
            "columns",
        ));
    }
    let names: Vec<(usize, &str)> = dataset
        .variable_names()
        .enumerate()
        .map(|(i, v)| (i + 1, v))
        .collect();
    let mut taken = HashSet::from(["__case_ordinal".to_string()]);
    let physical = physical_names(&names, &mut taken, dialect);
    if let Some((i, p)) = physical
        .iter()
        .enumerate()
        .find(|(_, p)| p.len() > dialect.identifier_bytes)
    {
        return Err(DbError::capability(
            names[i].1,
            "identifier",
            dialect.identifier_bytes as u64,
            p.len() as u64,
            "bytes",
        ));
    }
    let mut row_bytes = vec![9u64 * (n as u64 + 1); dataset.case_count()];
    for variable in dataset.variables() {
        match variable.values() {
            Values::Numeric(values) => {
                // SQLite turns NaN into NULL and stores a zero REAL as integer 0, so neither
                // NaN nor negative zero would come back
                if let Some(v) = values
                    .iter()
                    .flatten()
                    .find(|v| v.is_nan() || (*v == 0.0 && v.is_sign_negative()))
                {
                    let mut e = DbError::capability(variable.name(), "binary64", 0, 1, "values");
                    e.message = format!(
                        "{}: the SQLite profile cannot store the value {v:?}",
                        variable.name()
                    );
                    e.detail = Some(Box::new(
                        json!({"limit": "binary64", "value": format!("{:016x}", v.to_bits())}),
                    ));
                    return Err(e);
                }
            }
            Values::String(values) => {
                for (case, value) in values.iter().enumerate() {
                    let bytes = value.len() as u64;
                    if bytes > length {
                        return Err(DbError::capability(
                            variable.name(),
                            "value",
                            length,
                            bytes,
                            "bytes",
                        ));
                    }
                    row_bytes[case] += bytes;
                }
            }
        }
    }
    if let Some(&bytes) = row_bytes.iter().find(|&&b| b > length) {
        return Err(DbError::capability(
            "row",
            "structural_row",
            length,
            bytes,
            "bytes",
        ));
    }
    Ok(physical)
}

fn store(tx: &Transaction, dataset: &Dataset, name: &str, format: &str) -> Result<String> {
    let dialect = &SQLITE;
    check_dataset_name(name)?;
    let table = format!("data_{name}");
    let clash: Option<String> = tx
        .query_row(
            "SELECT dataset_name FROM dataset WHERE dataset_name = ?1 OR physical_table_name = ?2",
            params![name, table],
            |r| r.get(0),
        )
        .optional()?;
    if clash.is_some() {
        return Err(DbError::new(
            "dataset_exists",
            format!("the database already holds a dataset {name}"),
        ));
    }
    let physical = preflight(tx, dataset, dialect)?;

    let mut ddl = format!(
        "CREATE TABLE {} (\"__case_ordinal\" INTEGER NOT NULL PRIMARY KEY",
        dialect.quote(&table)
    );
    for (variable, column) in dataset.variables().zip(&physical) {
        let kind = if variable.is_string() {
            dialect.text_type
        } else {
            dialect.numeric_type
        };
        ddl += &format!(", {} {kind}", dialect.quote(column));
    }
    tx.execute(&(ddl + ")"), [])?;
    let placeholders = vec!["?"; physical.len() + 1].join(", ");
    let mut insert = tx.prepare(&format!(
        "INSERT INTO {} VALUES ({placeholders})",
        dialect.quote(&table)
    ))?;
    let columns: Vec<Values> = dataset.variables().map(|v| v.values()).collect();
    for case in 0..dataset.case_count() {
        let row =
            std::iter::once(Sql::Integer(case as i64 + 1)).chain(columns.iter().map(|values| {
                match values {
                    Values::Numeric(v) => v.get(case).map_or(Sql::Null, Sql::Real),
                    Values::String(v) => Sql::Text(v.get(case).to_string()),
                }
            }));
        insert.execute(rusqlite::params_from_iter(row))?;
    }

    let id = uuid(tx)?;
    tx.execute(
        &format!("INSERT INTO dataset VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, NULL, ?8, {NOW})"),
        params![
            id,
            SPEC_VERSION,
            format,
            table,
            name,
            dataset.file_label(),
            dataset.encoding(),
            dataset.case_count() as i64
        ],
    )?;
    let mut variable_ids = Vec::with_capacity(physical.len());
    for (i, (variable, column)) in dataset.variables().zip(&physical).enumerate() {
        let variable_id = uuid(tx)?;
        let (family, width, decimals) = split_format(variable.format());
        tx.execute(
            "INSERT INTO variable VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                variable_id,
                id,
                i as i64 + 1,
                variable.name(),
                column,
                if variable.is_string() { "string" } else { "numeric" },
                variable.string_width().map(|w| w as i64),
                variable.label(),
                family,
                width,
                decimals,
                measure_text(variable.measure()),
                variable.role(),
                variable.display_width(),
                variable.alignment()
            ],
        )?;
        store_value_labels(tx, &id, &variable_id, variable.value_labels())?;
        store_missing(tx, &variable_id, variable.missing_values())?;
        for (name, values) in variable.attributes() {
            for (k, value) in values.iter().enumerate() {
                tx.execute(
                    "INSERT INTO variable_attribute VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![uuid(tx)?, variable_id, name, k as i64 + 1, value],
                )?;
            }
        }
        variable_ids.push(variable_id);
    }
    let id_of = |name: &str| {
        dataset
            .variable_names()
            .position(|n| n == name)
            .map(|i| variable_ids[i].as_str())
            .ok_or_else(|| DbError::new("data_invalid", format!("unknown variable {name}")))
    };
    if let Some(weight) = dataset.weight_variable() {
        tx.execute(
            "INSERT INTO dataset_weight_variable VALUES (?1, ?2)",
            params![id, id_of(weight)?],
        )?;
    }
    for (k, line) in dataset.documents().iter().enumerate() {
        tx.execute(
            "INSERT INTO document VALUES (?1, ?2, ?3, ?4)",
            params![uuid(tx)?, id, k as i64 + 1, line],
        )?;
    }
    for (k, set) in dataset.multiple_response_sets().iter().enumerate() {
        let set_id = uuid(tx)?;
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
        // category_label_behavior and label_source stay NULL: the model does not carry them
        tx.execute(
            "INSERT INTO multiple_response_set VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)",
            params![set_id, id, k as i64 + 1, set.name, set.label, kind, counted_kind, number, text],
        )?;
        for (m, member) in set.variables.iter().enumerate() {
            tx.execute(
                "INSERT INTO multiple_response_member VALUES (?1, ?2, ?3)",
                params![set_id, id_of(member)?, m as i64 + 1],
            )?;
        }
    }
    Ok(id)
}

pub(crate) fn store_value_labels(
    tx: &Transaction,
    dataset_id: &str,
    variable_id: &str,
    labels: &[(statscraft_data::Value, String)],
) -> Result<()> {
    if labels.is_empty() {
        return Ok(());
    }
    let set_id = uuid(tx)?;
    tx.execute(
        "INSERT INTO value_label_set VALUES (?1, ?2, NULL)",
        params![set_id, dataset_id],
    )?;
    for (k, (value, label)) in labels.iter().enumerate() {
        let (kind, number, text) = match value {
            statscraft_data::Value::Number(v) => ("numeric", Some(*v), None),
            statscraft_data::Value::String(s) => ("string", None, Some(s.as_str())),
        };
        tx.execute(
            "INSERT INTO value_label VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![uuid(tx)?, set_id, k as i64 + 1, kind, number, text, label],
        )?;
    }
    tx.execute(
        "INSERT INTO variable_value_label_set VALUES (?1, ?2)",
        params![variable_id, set_id],
    )?;
    Ok(())
}

fn store_missing(tx: &Transaction, variable_id: &str, missing: &MissingValues) -> Result<()> {
    let discrete = |ordinal: i64, number: Option<f64>, text: Option<&str>| -> Result<()> {
        let kind = if text.is_some() { "string" } else { "numeric" };
        tx.execute(
            "INSERT INTO missing_rule VALUES (?1, ?2, ?3, 'discrete', ?4, ?5, ?6, NULL, NULL, NULL, NULL)",
            params![uuid(tx)?, variable_id, ordinal, kind, number, text],
        )?;
        Ok(())
    };
    match missing {
        MissingValues::None => {}
        MissingValues::Numbers(values) => {
            for (k, v) in values.iter().enumerate() {
                discrete(k as i64 + 1, Some(*v), None)?;
            }
        }
        MissingValues::Strings(values) => {
            for (k, v) in values.iter().enumerate() {
                discrete(k as i64 + 1, None, Some(v))?;
            }
        }
        MissingValues::Range { low, high, extra } => {
            // a LOWEST or HIGHEST endpoint is named, and its numeric column stays NULL
            let lower = (*low != LOWEST).then_some(*low);
            let upper = (*high != HIGHEST).then_some(*high);
            tx.execute(
                "INSERT INTO missing_rule VALUES (?1, ?2, 1, 'numeric_range', NULL, NULL, NULL, ?3, ?4, ?5, ?6)",
                params![
                    uuid(tx)?,
                    variable_id,
                    lower,
                    upper,
                    lower.is_none().then_some("LOWEST"),
                    upper.is_none().then_some("HIGHEST")
                ],
            )?;
            if let Some(v) = extra {
                discrete(2, Some(*v), None)?;
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
pub(crate) fn find_dataset(conn: &Connection, name: &str) -> Result<(String, String)> {
    conn.query_row(
        "SELECT dataset_id, physical_table_name FROM dataset WHERE dataset_name = ?1",
        [name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| DbError::new("dataset_unknown", format!("no dataset named {name}")))
}

pub(crate) fn catalog_variables(
    conn: &Connection,
    dataset_id: &str,
) -> Result<Vec<CatalogVariable>> {
    conn.prepare(
        "SELECT variable_id, source_name, physical_name, storage_kind, declared_string_width
         FROM variable WHERE dataset_id = ?1 ORDER BY source_ordinal",
    )?
    .query_map([dataset_id], |r| {
        let kind: String = r.get(3)?;
        let width: Option<i64> = r.get(4)?;
        Ok(CatalogVariable {
            id: r.get(0)?,
            name: r.get(1)?,
            physical: r.get(2)?,
            string_width: (kind == "string").then(|| width.unwrap_or(0) as usize),
        })
    })?
    .collect::<std::result::Result<_, _>>()
    .map_err(Into::into)
}

/// Value labels of a variable, in ordinal order.
fn value_labels(
    conn: &Connection,
    variable_id: &str,
) -> Result<Vec<(statscraft_data::Value, String)>> {
    conn.prepare(
        "SELECT l.code_kind, l.numeric_code, l.string_code, l.label
         FROM variable_value_label_set s JOIN value_label l USING (value_label_set_id)
         WHERE s.variable_id = ?1 ORDER BY l.ordinal",
    )?
    .query_map([variable_id], |r| {
        let kind: String = r.get(0)?;
        let value = if kind == "string" {
            statscraft_data::Value::String(r.get(2)?)
        } else {
            statscraft_data::Value::Number(r.get(1)?)
        };
        Ok((value, r.get(3)?))
    })?
    .collect::<std::result::Result<_, _>>()
    .map_err(Into::into)
}

fn missing_values(conn: &Connection, variable_id: &str) -> Result<MissingValues> {
    type Rule = (
        String,
        Option<f64>,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<String>,
        Option<String>,
    );
    let rules: Vec<Rule> = conn
        .prepare(
            "SELECT rule_kind, numeric_value, string_value, numeric_lower, numeric_upper,
                    lower_special, upper_special
             FROM missing_rule WHERE variable_id = ?1 ORDER BY ordinal",
        )?
        .query_map([variable_id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    let (mut numbers, mut strings, mut range) = (vec![], vec![], None);
    for (kind, number, text, lower, upper, lower_special, upper_special) in rules {
        let bad = || DbError::new("catalog_invalid", format!("missing_rule of {variable_id}"));
        if kind == "numeric_range" {
            let low = match lower_special.as_deref() {
                Some("LOWEST") => LOWEST,
                _ => lower.ok_or_else(bad)?,
            };
            let high = match upper_special.as_deref() {
                Some("HIGHEST") => HIGHEST,
                _ => upper.ok_or_else(bad)?,
            };
            range = Some((low, high));
        } else if let Some(text) = text {
            strings.push(text);
        } else {
            numbers.push(number.ok_or_else(bad)?);
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
/// before anything is returned, unless `allow_loss` is set; then each loss is recorded as a
/// warning event. The attempt is recorded as an export operation.
pub fn read_dataset(conn: &mut Connection, name: &str, allow_loss: bool) -> Result<Dataset> {
    let operation = uuid(conn)?;
    let result = (|| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            &format!("INSERT INTO operation VALUES (?1, 'export', 'started', NULL, {NOW}, NULL)"),
            [&operation],
        )?;
        let (dataset, losses, dataset_id) = load(&tx, name)?;
        if !losses.is_empty() && !allow_loss {
            let mut e = DbError::new(
                "export_fidelity_loss",
                format!("the dataset model cannot hold: {}", losses.join(", ")),
            );
            e.detail = Some(Box::new(json!({"losses": losses})));
            return Err(e);
        }
        for loss in &losses {
            let mut e = DbError::new("export_loss_allowed", format!("not exported: {loss}"));
            e.source_item = Some(loss.clone());
            insert_event(&tx, &operation, Some(&dataset_id), "export", "warning", &e)?;
        }
        tx.execute(
            &format!(
                "UPDATE operation SET status = 'succeeded', source_format = (SELECT source_format FROM dataset WHERE dataset_id = ?2), completed_at = {NOW} WHERE operation_id = ?1"
            ),
            params![operation, dataset_id],
        )?;
        tx.commit()?;
        Ok(dataset)
    })();
    result.inspect_err(|e| record_failure(conn, &operation, "export", None, e))
}

/// The dataset, the catalog items it could not take, and the dataset id.
fn load(conn: &Connection, name: &str) -> Result<(Dataset, Vec<String>, String)> {
    let dialect = &SQLITE;
    let (id, table) = find_dataset(conn, name)?;
    let (format, label, encoding, cases): (String, Option<String>, Option<String>, i64) = conn
        .query_row(
            "SELECT source_format, dataset_label, source_encoding, source_case_count
             FROM dataset WHERE dataset_id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let mut losses = vec![];
    let count = |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [&id], |r| r.get(0))?) };
    if count("SELECT count(*) FROM dataset_attribute WHERE dataset_id = ?1")? > 0 {
        losses.push("dataset_attributes".to_string());
    }
    if count("SELECT count(*) FROM variable_set WHERE dataset_id = ?1")? > 0 {
        losses.push("variable_sets".to_string());
    }
    if count(
        "SELECT count(*) FROM multiple_response_set WHERE dataset_id = ?1
         AND (category_label_behavior IS NOT NULL OR label_source IS NOT NULL)",
    )? > 0
    {
        losses.push("multiple_response_category_labels".to_string());
    }

    let variables = catalog_variables(conn, &id)?;
    let mut select = String::from("SELECT \"__case_ordinal\"");
    for v in &variables {
        select += &format!(", {}", dialect.quote(&v.physical));
    }
    select += &format!(
        " FROM {} ORDER BY \"__case_ordinal\"",
        dialect.quote(&table)
    );
    let mut numbers: Vec<Vec<Option<f64>>> = vec![vec![]; variables.len()];
    let mut strings: Vec<Vec<String>> = vec![vec![]; variables.len()];
    let mut statement = conn.prepare(&select)?;
    let mut rows = statement.query([])?;
    let mut ordinal = 0i64;
    while let Some(row) = rows.next()? {
        ordinal += 1;
        if row.get::<_, i64>(0)? != ordinal {
            return Err(DbError::new(
                "data_invalid",
                format!("{table}: __case_ordinal does not run 1, 2, 3, ... at case {ordinal}"),
            ));
        }
        for (i, v) in variables.iter().enumerate() {
            let invalid = |e: rusqlite::Error| {
                DbError::new(
                    "data_invalid",
                    format!("{table}.{} case {ordinal}: {e}", v.physical),
                )
            };
            match v.string_width {
                None => numbers[i].push(row.get(i + 1).map_err(invalid)?),
                Some(_) => strings[i].push(row.get(i + 1).map_err(invalid)?),
            }
        }
    }
    if ordinal != cases {
        return Err(DbError::new(
            "data_invalid",
            format!("{table} has {ordinal} rows, the catalog says {cases}"),
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
        type Meta = (
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<u32>,
            Option<String>,
        );
        let meta: Meta = conn.query_row(
            "SELECT variable_label, print_format_family, print_format_width, print_format_decimals,
                    write_format_family, write_format_width, write_format_decimals,
                    measurement_level, variable_role, display_width, display_alignment
             FROM variable WHERE variable_id = ?1",
            [&v.id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                ))
            },
        )?;
        let (
            label,
            family,
            width,
            decimals,
            w_family,
            w_width,
            w_decimals,
            level,
            role,
            display,
            align,
        ) = meta;
        if (&family, width, decimals) != (&w_family, w_width, w_decimals) {
            losses.push(format!("{}: write_format", v.name));
        }
        dataset.set_variable_label(&v.name, label.as_deref())?;
        if let (Some(family), Some(width), None) = (&family, width, v.string_width) {
            let format = match decimals {
                Some(d) => format!("{family}{width}.{d}"),
                None => format!("{family}{width}"),
            };
            dataset.set_format(&v.name, &format)?;
        }
        let measure = match level.as_deref() {
            Some("nominal") => Measure::Nominal,
            Some("ordinal") => Measure::Ordinal,
            Some("scale") => Measure::Scale,
            _ => Measure::Unknown,
        };
        dataset.set_measure(&v.name, measure)?;
        dataset.set_value_labels(&v.name, value_labels(conn, &v.id)?)?;
        dataset.set_missing_values(&v.name, missing_values(conn, &v.id)?)?;
        dataset.set_display(&v.name, display, align.as_deref(), role.as_deref())?;
        let mut attributes: Vec<(String, Vec<String>)> = vec![];
        // attribute names keep their insertion order: the catalog has no ordinal for them
        let rows: Vec<(String, String)> = conn
            .prepare(
                "SELECT attribute_name, attribute_value FROM variable_attribute
                 WHERE variable_id = ?1 ORDER BY rowid",
            )?
            .query_map([&v.id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (name, value) in rows {
            match attributes.iter_mut().find(|(n, _)| *n == name) {
                Some((_, values)) => values.push(value),
                None => attributes.push((name, vec![value])),
            }
        }
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
    let weight: Option<String> = conn
        .query_row(
            "SELECT variable_id FROM dataset_weight_variable WHERE dataset_id = ?1",
            [&id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(weight) = weight {
        dataset.set_weight_variable(Some(&name_of(&weight)?))?;
    }
    // id, name, label, kind, counted number, counted text
    type SetRow = (
        String,
        String,
        Option<String>,
        String,
        Option<f64>,
        Option<String>,
    );
    let sets: Vec<SetRow> = conn
        .prepare(
            "SELECT multiple_response_set_id, set_name, set_label, set_kind,
                    counted_numeric_value, counted_string_value
             FROM multiple_response_set WHERE dataset_id = ?1 ORDER BY source_ordinal",
        )?
        .query_map([&id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    for (set_id, set_name, set_label, kind, number, text) in sets {
        let members: Vec<String> = conn
            .prepare(
                "SELECT variable_id FROM multiple_response_member
                 WHERE multiple_response_set_id = ?1 ORDER BY source_ordinal",
            )?
            .query_map([&set_id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let counted_value = match (number, text) {
            (Some(v), _) if v.fract() == 0.0 && v.abs() < 9.0e15 => Some((v as i64).to_string()),
            (Some(v), _) => Some(v.to_string()),
            (None, text) => text,
        };
        dataset.set_multiple_response_set(MultipleResponseSet {
            name: set_name,
            kind: if kind == "MD" {
                "dichotomy"
            } else {
                "category"
            },
            label: set_label.unwrap_or_default(),
            counted_value,
            variables: members.iter().map(|m| name_of(m)).collect::<Result<_>>()?,
        })?;
    }
    let documents: Vec<String> = conn
        .prepare(
            "SELECT document_text FROM document WHERE dataset_id = ?1 ORDER BY source_ordinal",
        )?
        .query_map([&id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
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
pub fn input_schema(conn: &Connection, name: &str, with_format: bool) -> Result<Value> {
    let (id, _) = find_dataset(conn, name)?;
    let mut variables = vec![];
    for v in catalog_variables(conn, &id)? {
        let [label, family, width, decimals, level] = conn.query_row(
            "SELECT variable_label, print_format_family, print_format_width,
                    print_format_decimals, measurement_level
             FROM variable WHERE variable_id = ?1",
            [&v.id],
            |r| {
                Ok([
                    json!(r.get::<_, Option<String>>(0)?),
                    json!(r.get::<_, Option<String>>(1)?),
                    json!(r.get::<_, Option<i64>>(2)?),
                    json!(r.get::<_, Option<i64>>(3)?),
                    json!(r.get::<_, Option<String>>(4)?),
                ])
            },
        )?;
        let labels: Vec<Value> = value_labels(conn, &v.id)?
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
            "variable_label": label,
            "value_labels": labels,
        });
        if with_format {
            out["format_family"] = json!(family);
            out["width"] = json!(width);
            out["decimals"] = json!(decimals);
            out["measurement_level"] = json!(level);
        }
        variables.push(out);
    }
    Ok(json!({ "variables": variables }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        prepare(&mut conn, true).unwrap();
        conn
    }

    fn sample() -> Dataset {
        let mut d = Dataset::new(3);
        d.set_numeric_values("x", vec![Some(1.5), None, Some(f64::INFINITY)])
            .unwrap();
        d.set_string_values("select", 4, vec!["ab".into(), "".into(), "abcd".into()])
            .unwrap();
        d.set_numeric_values("Ä", vec![Some(5e-324), Some(f64::MAX), Some(-2.0)])
            .unwrap();
        d
    }

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
        let got = physical_names(&names, &mut taken, &SQLITE);
        // "Age" folds onto "age"; v4 is a source name, so ordinal 4 gets v4_
        assert_eq!(got, ["age", "AGE2", "v4", "v4_", "v5", "v6"]);
    }

    #[test]
    fn values_round_trip_bit_exact_with_reserved_words_and_generated_names() {
        let mut conn = memory();
        let d = sample();
        write_dataset(&mut conn, &d, "s").unwrap();
        let back = read_dataset(&mut conn, "s", false).unwrap();
        for (a, b) in d.variables().zip(back.variables()) {
            assert_eq!(a.name(), b.name());
            match (a.values(), b.values()) {
                (Values::Numeric(x), Values::Numeric(y)) => {
                    let bits = |v: Option<f64>| v.map(f64::to_bits);
                    assert!(x.iter().map(bits).eq(y.iter().map(bits)));
                }
                (Values::String(x), Values::String(y)) => assert!(x.iter().eq(y.iter())),
                _ => panic!("storage kind changed"),
            }
        }
        let physical: String = conn
            .query_row(
                "SELECT physical_name FROM variable WHERE source_name = 'Ä'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(physical, "v3");
    }

    #[test]
    fn sqlite_real_columns_lose_negative_zero_and_nan() {
        // the reason for the preflight rule below
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (x REAL)").unwrap();
        for v in [-0.0, f64::NAN] {
            conn.execute("INSERT INTO t VALUES (?1)", [v]).unwrap();
        }
        let back: Vec<Option<f64>> = conn
            .prepare("SELECT x FROM t ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(back[0].map(f64::to_bits), Some(0));
        assert_eq!(back[1], None);
    }

    #[test]
    fn negative_zero_and_nan_fail_preflight_atomically() {
        for bad in [-0.0, f64::NAN] {
            let mut conn = memory();
            let mut d = sample();
            d.set_numeric_values("x", vec![Some(1.0), Some(bad), None])
                .unwrap();
            let e = write_dataset(&mut conn, &d, "s").unwrap_err();
            assert_eq!(e.code, "target_capability_exceeded");
            let state: (i64, i64, String, Option<String>) = conn
                .query_row(
                    "SELECT (SELECT count(*) FROM dataset),
                            (SELECT count(*) FROM sqlite_schema WHERE name = 'data_s'),
                            (SELECT status FROM operation),
                            (SELECT source_item FROM fidelity_event WHERE dataset_id IS NULL)",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .unwrap();
            assert_eq!(state, (0, 0, "failed".into(), Some("x".into())));
        }
    }

    #[test]
    fn column_limit_is_inclusive_of_the_ordinal() {
        let wide = |n: usize| {
            let mut d = Dataset::new(1);
            for i in 0..n {
                d.set_numeric_values(&format!("v{i}"), vec![Some(i as f64)])
                    .unwrap();
            }
            d
        };
        let mut conn = memory();
        write_dataset(&mut conn, &wide(1999), "fits").unwrap();
        let e = write_dataset(&mut conn, &wide(2000), "over").unwrap_err();
        assert_eq!(e.code, "target_capability_exceeded");
        assert_eq!(e.source_item.as_deref(), Some("v1999"));
        assert_eq!(e.detail.unwrap()["required"], 2001);
    }

    #[test]
    fn foreign_databases_are_refused_untouched() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app (x)").unwrap();
        assert_eq!(
            prepare(&mut conn, true).unwrap_err().code,
            "catalog_foreign"
        );
        let mut empty = Connection::open_in_memory().unwrap();
        assert_eq!(
            prepare(&mut empty, false).unwrap_err().code,
            "catalog_missing"
        );
        let mut ours = memory();
        ours.execute_batch("CREATE TABLE extra (x)").unwrap();
        assert_eq!(
            prepare(&mut ours, false).unwrap_err().code,
            "catalog_foreign"
        );
    }

    #[test]
    fn duplicate_dataset_names_are_refused() {
        let mut conn = memory();
        write_dataset(&mut conn, &sample(), "s").unwrap();
        assert_eq!(
            write_dataset(&mut conn, &sample(), "S").unwrap_err().code,
            "dataset_exists"
        );
        assert_eq!(
            write_dataset(&mut conn, &sample(), "a b").unwrap_err().code,
            "dataset_name_invalid"
        );
    }

    #[test]
    fn catalog_content_the_model_cannot_hold_needs_allow_loss() {
        let mut conn = memory();
        let id = write_dataset(&mut conn, &sample(), "s").unwrap();
        conn.execute(
            "INSERT INTO dataset_attribute VALUES ('a', ?1, 'origin', 1, 'test')",
            [&id],
        )
        .unwrap();
        let e = read_dataset(&mut conn, "s", false).err().unwrap();
        assert_eq!(e.code, "export_fidelity_loss");
        read_dataset(&mut conn, "s", true).unwrap();
        let events: i64 = conn
            .query_row(
                "SELECT count(*) FROM fidelity_event WHERE event_code = 'export_loss_allowed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
    }
}
