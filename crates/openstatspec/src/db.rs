//! The SQL dialect profiles and the small database interface that the catalog and the in-place
//! apply run on. The differences between SQLite, PostgreSQL, MySQL, MariaDB and Dolt live here;
//! the connection modules (`sqlite`, `postgres`, `mysql`) only move values and statements.

use serde_json::{Value, json};
use std::hash::{BuildHasher, Hasher};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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

    pub(crate) fn capability(
        item: &str,
        limit: &str,
        maximum: u64,
        required: u64,
        unit: &str,
    ) -> Self {
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

/// One SQL parameter or result cell.
#[derive(Clone, Debug, PartialEq)]
pub enum Sql {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

fn cell_error(want: &str, got: &Sql) -> DbError {
    DbError::new("catalog_invalid", format!("expected {want}, found {got:?}"))
}

impl Sql {
    pub fn is_null(&self) -> bool {
        *self == Sql::Null
    }

    pub fn int(&self) -> Result<i64> {
        match self {
            Sql::Int(v) => Ok(*v),
            // MySQL sends DECIMAL results as text
            Sql::Text(t) => t.parse().map_err(|_| cell_error("an integer", self)),
            _ => Err(cell_error("an integer", self)),
        }
    }

    pub fn opt_int(&self) -> Result<Option<i64>> {
        (!self.is_null()).then(|| self.int()).transpose()
    }

    pub fn real(&self) -> Result<f64> {
        match self {
            Sql::Real(v) => Ok(*v),
            Sql::Int(v) => Ok(*v as f64),
            _ => Err(cell_error("a number", self)),
        }
    }

    pub fn opt_real(&self) -> Result<Option<f64>> {
        (!self.is_null()).then(|| self.real()).transpose()
    }

    pub fn text(&self) -> Result<String> {
        match self {
            Sql::Text(t) => Ok(t.clone()),
            _ => Err(cell_error("text", self)),
        }
    }

    pub fn opt_text(&self) -> Result<Option<String>> {
        (!self.is_null()).then(|| self.text()).transpose()
    }
}

impl From<i64> for Sql {
    fn from(v: i64) -> Self {
        Sql::Int(v)
    }
}

impl From<usize> for Sql {
    fn from(v: usize) -> Self {
        Sql::Int(v as i64)
    }
}

impl From<u32> for Sql {
    fn from(v: u32) -> Self {
        Sql::Int(v.into())
    }
}

impl From<f64> for Sql {
    fn from(v: f64) -> Self {
        Sql::Real(v)
    }
}

impl From<&str> for Sql {
    fn from(v: &str) -> Self {
        Sql::Text(v.to_string())
    }
}

impl From<String> for Sql {
    fn from(v: String) -> Self {
        Sql::Text(v)
    }
}

impl From<&String> for Sql {
    fn from(v: &String) -> Self {
        Sql::Text(v.clone())
    }
}

impl<T: Into<Sql>> From<Option<T>> for Sql {
    fn from(v: Option<T>) -> Self {
        v.map_or(Sql::Null, Into::into)
    }
}

/// `args![a, b]` is a parameter slice for [`Db::exec`] and [`Db::query`].
#[macro_export]
macro_rules! args {
    ($($x:expr),* $(,)?) => { &[$($crate::db::Sql::from($x)),*][..] };
}

/// The effective limits of one connection, discovered from the server where it has a setting.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Physical columns of the data table, `__case_ordinal` included.
    pub columns: usize,
    /// Generated identifiers, in [`Dialect::identifier_unit`].
    pub identifier: usize,
    /// One string value, in bytes.
    pub value_bytes: u64,
    /// The structural row under [`Dialect::row_bytes`], in bytes.
    pub row_bytes: u64,
    /// One emitted INSERT of one case, in bytes.
    pub statement_bytes: u64,
}

/// A connection to a database that holds one OpenStatSpec catalog. All statements use `?`
/// placeholders; a connection that needs another style rewrites them.
pub trait Db {
    fn dialect(&self) -> &'static Dialect;
    /// Runs one statement and returns the number of rows it changed.
    fn exec(&mut self, sql: &str, params: &[Sql]) -> Result<u64>;
    fn query(&mut self, sql: &str, params: &[Sql]) -> Result<Vec<Vec<Sql>>>;
    /// Runs one statement without parameters: DDL and transaction control.
    fn batch(&mut self, sql: &str) -> Result<()>;
    /// The tables and views of the catalog namespace.
    fn objects(&mut self) -> Result<Vec<String>>;
    /// The schema or database that holds the catalog, recorded as `physical_table_schema`.
    fn namespace(&self) -> Option<String>;
    fn limits(&mut self) -> Result<Limits>;
    /// Creates the catalog namespace, inside the install transaction where DDL is atomic.
    fn create_namespace(&mut self) -> Result<()> {
        Ok(())
    }
    /// Fails when this server may be read but not written (a Dolt version outside the tested
    /// policy).
    fn write_gate(&self) -> Result<()> {
        Ok(())
    }
    /// Dolt only: the commit `branch` points at, as a new session sees it, outside the
    /// snapshot of this connection's open transaction.
    fn committed_head(&mut self, _branch: &str) -> Result<String> {
        Err(DbError::new("dolt_context_required", "not a Dolt database"))
    }
}

/// The first row of a query, if any.
pub(crate) fn first(db: &mut dyn Db, sql: &str, params: &[Sql]) -> Result<Option<Vec<Sql>>> {
    Ok(db.query(sql, params)?.into_iter().next())
}

/// The structural row rule of a profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RowRule {
    /// SQLite: nine bytes per column plus the string bytes, against the length limit.
    Record,
    /// PostgreSQL: a lower bound of the heap tuple after TOAST, per case.
    HeapTuple,
    /// InnoDB DYNAMIC on a 16 KiB page: the worst-case record the table definition allows;
    /// each `TEXT` column counts this many bytes. Measured on MySQL 8.4.9 and MariaDB 11.8.9.
    InnoDb { text_bytes: u64 },
    /// Dolt: the fixed part of the table definition (8 bytes per numeric column and
    /// `__case_ordinal`, 20 per `LONGTEXT`), which Dolt 2.2 keeps below 65,504 bytes.
    DoltSchema,
}

/// The physical choices of one SQL dialect profile.
pub struct Dialect {
    /// The name `transformation_apply.database_profile` records.
    pub profile: &'static str,
    /// Identifier quote character; an embedded one is doubled.
    pub quote: char,
    pub identifier_unit: &'static str,
    pub numeric_type: &'static str,
    pub text_type: &'static str,
    pub ordinal_type: &'static str,
    /// Appended to every CREATE TABLE.
    pub table_options: &'static str,
    /// True when CREATE and ALTER TABLE, data, catalog and audit commit or roll back together.
    pub atomic_schema_change: bool,
    pub begin: &'static str,
    /// The current UTC time as a value of the catalog's timestamp type.
    pub now: &'static str,
    pub row_rule: RowRule,
    /// Catalog column types: ids, timestamps, indexed text, long text, binary64, integers.
    types: [&'static str; 6],
    /// Text values may hold U+0000.
    pub text_nul: bool,
    /// True when export and every other read writes nothing to the database: Database I/O
    /// Execution Policy v1, selected for Dolt so that reads leave the working set clean.
    pub read_only_export: bool,
    accepts: fn(f64) -> bool,
}

/// Every binary64 value round-trips bit for bit.
fn any_value(_: f64) -> bool {
    true
}

/// A SQLite REAL column turns negative zero into 0 and NaN into NULL.
fn sqlite_value(v: f64) -> bool {
    !v.is_nan() && !(v == 0.0 && v.is_sign_negative())
}

/// MySQL, MariaDB and Dolt DOUBLE columns hold finite values only.
fn finite(v: f64) -> bool {
    v.is_finite()
}

pub const SQLITE: Dialect = Dialect {
    profile: "sqlite",
    quote: '"',
    identifier_unit: "bytes",
    numeric_type: "REAL",
    text_type: "TEXT NOT NULL",
    ordinal_type: "INTEGER NOT NULL PRIMARY KEY",
    table_options: "",
    atomic_schema_change: true,
    begin: "BEGIN IMMEDIATE",
    now: "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    row_rule: RowRule::Record,
    types: ["TEXT", "TEXT", "TEXT", "TEXT", "REAL", "INTEGER"],
    text_nul: true,
    read_only_export: false,
    accepts: sqlite_value,
};

pub const POSTGRESQL: Dialect = Dialect {
    profile: "postgresql",
    quote: '"',
    identifier_unit: "bytes",
    numeric_type: "DOUBLE PRECISION",
    text_type: "TEXT NOT NULL",
    ordinal_type: "BIGINT NOT NULL PRIMARY KEY",
    table_options: "",
    atomic_schema_change: true,
    begin: "BEGIN",
    // the time of the statement, not of the transaction
    now: "clock_timestamp()",
    row_rule: RowRule::HeapTuple,
    types: [
        "UUID",
        "TIMESTAMPTZ",
        "TEXT",
        "TEXT",
        "DOUBLE PRECISION",
        "BIGINT",
    ],
    text_nul: false,
    read_only_export: false,
    accepts: any_value,
};

const MYSQL_TYPES: [&str; 6] = [
    "CHAR(36)",
    "DATETIME(6)",
    "VARCHAR(255)",
    "LONGTEXT",
    "DOUBLE",
    "BIGINT",
];

pub const MYSQL: Dialect = Dialect {
    profile: "mysql",
    quote: '`',
    identifier_unit: "characters",
    numeric_type: "DOUBLE",
    text_type: "TEXT NOT NULL",
    ordinal_type: "BIGINT NOT NULL PRIMARY KEY",
    // a binary collation without trailing-space padding compares strings exactly
    table_options: " ENGINE=InnoDB ROW_FORMAT=DYNAMIC DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin",
    atomic_schema_change: false,
    begin: "START TRANSACTION",
    now: "UTC_TIMESTAMP(6)",
    row_rule: RowRule::InnoDb { text_bytes: 41 },
    types: MYSQL_TYPES,
    text_nul: true,
    read_only_export: false,
    accepts: finite,
};

pub const MARIADB: Dialect = Dialect {
    profile: "mariadb",
    table_options: " ENGINE=InnoDB ROW_FORMAT=DYNAMIC DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_nopad_bin",
    row_rule: RowRule::InnoDb { text_bytes: 21 },
    ..MYSQL
};

pub const DOLT: Dialect = Dialect {
    profile: "dolt",
    text_type: "LONGTEXT NOT NULL",
    table_options: " DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin",
    row_rule: RowRule::DoltSchema,
    read_only_export: true,
    ..MYSQL
};

impl Dialect {
    pub fn quote(&self, identifier: &str) -> String {
        let q = self.quote.to_string();
        format!("{q}{}{q}", identifier.replace(q.as_str(), &q.repeat(2)))
    }

    pub fn accepts(&self, v: f64) -> bool {
        (self.accepts)(v)
    }

    /// A placeholder for a typed literal: PostgreSQL cannot infer the type of a parameter
    /// that is compared with another parameter or returned from CASE.
    pub fn param(&self, string: bool) -> &'static str {
        match (self.profile, string) {
            ("postgresql", false) => "CAST(? AS DOUBLE PRECISION)",
            ("postgresql", true) => "CAST(? AS TEXT)",
            _ => "?",
        }
    }

    /// A CHECK that `column` holds 64 lowercase hex digits.
    fn hex64(&self, column: &str) -> String {
        match self.profile {
            "sqlite" => format!("length({column}) = 64 AND {column} NOT GLOB '*[^0-9a-f]*'"),
            "postgresql" => format!("{column} ~ '^[0-9a-f]{{64}}$'"),
            _ => format!("{column} REGEXP '^[0-9a-f]{{64}}$'"),
        }
    }

    /// The catalog DDL template in this dialect's types, one statement per item.
    pub(crate) fn ddl(&self, template: &str) -> Vec<String> {
        let mut ddl = template.to_string();
        for (token, ty) in ["{id}", "{ts}", "{key}", "{text}", "{real}", "{int}"]
            .iter()
            .zip(self.types)
        {
            ddl = ddl.replace(token, ty);
        }
        let nocase = if self.profile == "sqlite" {
            " COLLATE NOCASE"
        } else {
            ""
        };
        ddl = ddl
            .replace("{nocase}", nocase)
            .replace("{hex source_hash}", &self.hex64("source_hash"))
            .replace("{hex plan_hash}", &self.hex64("plan_hash"))
            .replace("{options}", self.table_options);
        ddl.split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// The bytes of one case under the profile's structural row rule. `numeric` holds one flag
    /// per numeric variable (true when the value is not NULL), `strings` the byte length of
    /// each string value.
    pub(crate) fn row_bytes(&self, numeric: &[bool], strings: &[u64]) -> u64 {
        let n = numeric.len() as u64;
        let s = strings.len() as u64;
        match self.row_rule {
            RowRule::Record => 9 * (n + s + 1) + strings.iter().sum::<u64>(),
            // header and ordinal, 8 bytes per non-NULL double, and each string at most what
            // it can shrink to: a 1-byte header plus its bytes, or a compressed or
            // out-of-line datum of at least 9 bytes
            RowRule::HeapTuple => {
                32 + 8 * numeric.iter().filter(|v| **v).count() as u64
                    + strings.iter().map(|b| (b + 1).min(9)).sum::<u64>()
            }
            RowRule::InnoDb { text_bytes } => 31 + n.div_ceil(8) + 8 * n + text_bytes * s,
            RowRule::DoltSchema => 8 + 8 * n + 20 * s,
        }
    }
}

/// A time-ordered version 7 UUID (RFC 9562): 48 bits of Unix milliseconds, a 12-bit counter
/// that keeps ids of one process in creation order, and 62 bits from the process's random
/// hash keys. Catalog rows without an ordinal of their own (attribute names) are read back in
/// id order, which is insertion order.
pub fn uuid() -> String {
    static LAST: Mutex<(u64, u64)> = Mutex::new((0, 0));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let (ms, seq) = {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if now > last.0 {
            *last = (now, 0);
        } else if last.1 == 0xfff {
            *last = (last.0 + 1, 0);
        } else {
            last.1 += 1;
        }
        *last
    };
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    let hi = ((ms & 0xffff_ffff_ffff) << 16) | 0x7000 | seq;
    let lo = (random >> 2) | 0x8000_0000_0000_0000;
    let hex = format!("{hi:016x}{lo:016x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_version_7_and_in_creation_order() {
        let ids: Vec<String> = (0..5000).map(|_| uuid()).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        let id = &ids[0];
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "7");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn identifiers_are_quoted_with_doubled_quotes() {
        assert_eq!(SQLITE.quote("a\"b"), "\"a\"\"b\"");
        assert_eq!(MYSQL.quote("a`b"), "`a``b`");
    }

    #[test]
    fn innodb_row_rule_matches_the_measured_boundaries() {
        // MySQL 8.4.9 accepts 996 doubles and 197 texts, MariaDB 11.8.9 996 doubles and 385
        // texts; one more column fails with "Row size too large (> 8126)".
        let fits =
            |d: &Dialect, n: usize, s: usize| d.row_bytes(&vec![true; n], &vec![0; s]) <= 8126;
        for (dialect, texts) in [(&MYSQL, 197), (&MARIADB, 385)] {
            assert!(fits(dialect, 996, 0) && !fits(dialect, 997, 0));
            assert!(fits(dialect, 0, texts) && !fits(dialect, 0, texts + 1));
        }
        assert!(fits(&MYSQL, 500, 98) && !fits(&MYSQL, 500, 99));
        assert!(fits(&MARIADB, 500, 192) && !fits(&MARIADB, 500, 193));
    }
}
