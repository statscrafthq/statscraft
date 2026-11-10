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
    /// Appends rows to `table` (unquoted) inside the open transaction; each row holds every
    /// column in table order. The default runs one prepared INSERT per row.
    fn insert_rows(&mut self, table: &str, rows: &mut dyn Iterator<Item = Vec<Sql>>) -> Result<()> {
        let mut insert = String::new();
        for row in rows {
            if insert.is_empty() {
                let marks = vec!["?"; row.len()].join(", ");
                insert = format!(
                    "INSERT INTO {} VALUES ({marks})",
                    self.dialect().quote(table)
                );
            }
            self.exec(&insert, &row)?;
        }
        Ok(())
    }
}

/// Replaces each `?` outside string literals with `fill(n)` for the n-th placeholder, from 1.
pub(crate) fn placeholders(sql: &str, mut fill: impl FnMut(usize) -> String) -> String {
    let (mut out, mut n, mut quoted) = (String::with_capacity(sql.len()), 0, false);
    for c in sql.chars() {
        match c {
            '?' if !quoted => {
                n += 1;
                out += &fill(n);
            }
            '\'' => {
                quoted = !quoted;
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Takes the query parameters `names` off a URL; the client libraries do not know them. Returns
/// the rest of the URL and the value of each name, the last one where a name repeats.
pub(crate) fn take_params<const N: usize>(
    url: &str,
    names: [&str; N],
) -> (String, [Option<String>; N]) {
    let mut values = [const { None }; N];
    let Some((base, query)) = url.split_once('?') else {
        return (url.to_string(), values);
    };
    let rest: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            match names.iter().position(|n| *n == key) {
                Some(i) => {
                    values[i] = Some(value.to_string());
                    false
                }
                None => true,
            }
        })
        .collect();
    let url = if rest.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", rest.join("&"))
    };
    (url, values)
}

/// A TLS connection. The certificate chain is always verified: against the CA file when one is
/// given, otherwise against the bundled Mozilla roots.
#[derive(Clone, Debug, PartialEq)]
pub struct Tls {
    /// The certificate must also name the host of the URL.
    pub verify_host: bool,
    pub ca: Option<std::path::PathBuf>,
}

/// The TLS setting of the URL parameters `mode` and `ca` (`sslmode` and `sslrootcert` for
/// PostgreSQL, `ssl-mode` and `ssl-ca` for MySQL). No mode and no CA file is plain TCP; a CA
/// file alone means full verification. `require` verifies like `verify-full`: StatsCraft never
/// makes an unverified TLS connection. Modes that fall back to plain TCP are refused.
pub(crate) fn tls(mode: Option<&str>, ca: Option<String>) -> Result<Option<Tls>> {
    let ca = ca.filter(|c| !c.is_empty()).map(Into::into);
    let mode = mode.map(|m| m.to_ascii_lowercase().replace('_', "-"));
    let verify_host = match mode.as_deref() {
        None if ca.is_none() => return Ok(None),
        Some("disable" | "disabled") if ca.is_none() => return Ok(None),
        None | Some("require" | "required" | "verify-full" | "verify-identity") => true,
        Some("verify-ca") => false,
        Some(other) => {
            return Err(DbError::new(
                "database_unavailable",
                format!(
                    "TLS mode {other:?}: use disable, require, verify-ca or verify-full \
                     (MySQL: DISABLED, REQUIRED, VERIFY_CA, VERIFY_IDENTITY), with a CA file \
                     only when TLS is on"
                ),
            ));
        }
    };
    Ok(Some(Tls { verify_host, ca }))
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
    /// Parameters of one prepared statement.
    pub max_params: usize,
    /// Data rows travel as SQL text literals, so a case's INSERT grows with escaping.
    pub literal_rows: bool,
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

/// MySQL, MariaDB and Dolt DOUBLE columns hold finite values only, and a parameter of negative
/// zero comes back as positive zero (MySQL 8.4.9, MariaDB 11.8.9, Dolt 2.2.2 and 2.2.3).
fn finite(v: f64) -> bool {
    v.is_finite() && !(v == 0.0 && v.is_sign_negative())
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
    max_params: 32766,
    literal_rows: false,
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
    max_params: 65535,
    literal_rows: false,
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
    max_params: 65535,
    literal_rows: false,
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
    // a prepared statement with 1,024 or more parameters gets no answer (2.2.3)
    max_params: 1023,
    literal_rows: true,
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

    /// The bytes of the INSERT of one case with `n` values besides the ordinal; `text` is the
    /// sum over its strings of [`Dialect::text_bytes`]. Framing and the table name take at
    /// most 128 bytes; as parameters a value takes a type and a length (11 bytes) besides its
    /// bytes, as a literal a number takes at most 25 bytes with its comma.
    pub(crate) fn insert_bytes(&self, n: usize, text: u64) -> u64 {
        let per_value = if self.literal_rows { 25 } else { 11 };
        128 + per_value * (n as u64 + 1) + text
    }

    /// The bytes of one string value in an INSERT: as a literal with quotes, a comma and the
    /// escapes of quote, backslash and NUL.
    pub(crate) fn text_bytes(&self, value: &str) -> u64 {
        if self.literal_rows {
            let escapes = value
                .bytes()
                .filter(|b| matches!(b, b'\'' | b'\\' | 0))
                .count();
            (value.len() + escapes + 3) as u64
        } else {
            value.len() as u64
        }
    }
}

/// The OpenStatSpec commit this adapter was tested against: untagged work on `main`.
pub const SPECIFICATION_COMMIT: &str = "8594ee12ccf57526cf1142df4b216432c366da84";

/// The adapter's machine-readable capability declaration: per SQL profile the claimed and
/// tested server versions, the effective limits with their units and sources, and the
/// atomicity. Dolt selects Database I/O Execution Policy v1: writes follow the packaged tested
/// versions instead of an external declaration, and reads write nothing.
pub fn capabilities() -> Value {
    let limit = |value: u64, unit: &str, source: &str| json!({"value": value, "unit": unit, "source": source});
    let innodb = |tested: &[&str], claimed: &[&str], text: u64| {
        json!({
            "engine": "InnoDB, ROW_FORMAT=DYNAMIC, 16 KiB pages",
            "claimed_release_series": claimed,
            "tested_versions": tested,
            "physical_columns": limit(1017, "columns", "InnoDB column limit"),
            "identifier": limit(64, "characters", "MySQL and MariaDB identifier limit"),
            "value": limit(65535, "bytes", "TEXT"),
            "structural_row": {
                "value": 8126, "unit": "bytes",
                "source": format!("worst-case record at CREATE TABLE: 31 + ceil(n/8) + 8 per \
                    DOUBLE + {text} per TEXT, measured"),
            },
            "emitted_statement": {"unit": "bytes", "source": "active max_allowed_packet; \
                prepared multi-row INSERTs of at most 4 MiB and 65,535 parameters, each case \
                preflighted at 11 bytes per value plus its string bytes"},
            "numeric": "DOUBLE; NaN, infinities and negative zero refused",
            "text": "TEXT NOT NULL, binary collation without padding",
            "atomic_schema_change": false,
            "failure_cleanup": "the data table is dropped when the rows or catalog fail",
        })
    };
    json!({
        "adapter_implementation_id": "statscraft-openstatspec",
        "adapter_version": env!("CARGO_PKG_VERSION"),
        "specification_commit": SPECIFICATION_COMMIT,
        "specification_status": "release_candidate",
        "specification_release": null,
        "source_profile": crate::catalog::SPEC_VERSION,
        "directions": ["import", "export", "semantic_round_trip"],
        "transformation_bindings": [
            "openstatspec-in-place-transformation-v0.1",
            "openstatspec-in-place-transformation-v0.2",
        ],
        "transport": {
            "default": "plain TCP",
            "tls": "rustls, TLS 1.2 and 1.3; the certificate chain is always verified, against \
                the given CA file or the bundled Mozilla roots; the host name too unless the mode \
                is verify-ca; modes that fall back to plain TCP are refused",
        },
        "profiles": {
            "sqlite": {
                "claimed_versions": ">=3.24.0,<4.0.0",
                "tested_versions": [rusqlite::version()],
                "physical_columns": limit(2000, "columns", "profile default and SQLITE_LIMIT_COLUMN"),
                "identifier": limit(255, "bytes", "OpenStatSpec adapter policy"),
                "value": {"unit": "bytes", "source": "SQLITE_LIMIT_LENGTH"},
                "structural_row": {"unit": "bytes", "source": "SQLITE_LIMIT_LENGTH"},
                "numeric": "REAL; NaN and negative zero refused",
                "text": "TEXT NOT NULL",
                "atomic_schema_change": true,
            },
            "postgresql": {
                "claimed_release_series": ["17.x", "18.x"],
                "tested_versions": ["17.11"],
                "physical_columns": limit(1600, "columns", "PostgreSQL column limit"),
                "identifier": {"unit": "bytes", "source": "active max_identifier_length, UTF8"},
                "value": limit((1 << 30) - 1, "bytes", "varlena limit"),
                "structural_row": {"unit": "bytes", "source": "block_size - 32; lower bound per \
                    case: 32 + 8 per non-NULL double + min(bytes + 1, 9) per string"},
                "numeric": "DOUBLE PRECISION, every bit pattern",
                "text": "TEXT NOT NULL; U+0000 refused",
                "atomic_schema_change": true,
            },
            "mysql": innodb(&["8.4.9"], &["8.4.x", "9.7.x"], 41),
            "mariadb": innodb(&["11.8.9"], &["11.4.x", "11.8.x", "12.3.x"], 21),
            "dolt": {
                "database_io_policy": "openstatspec-database-io-v1",
                "claimed_product_versions": ">=2.2.2,<2.3.0",
                "tested_product_versions": crate::mysql::DOLT_TESTED,
                "product_identity": "@@version_comment trimmed and case-folded is dolt; DOLT_VERSION()",
                "physical_columns": limit(4097, "columns", "Dolt tuple limit, measured"),
                "identifier": limit(64, "characters", "measured"),
                "value": {"unit": "bytes", "source": "LONGTEXT and active max_allowed_packet"},
                "structural_row": limit(65503, "bytes", "table definition below 65,504 bytes: \
                    8 per BIGINT and DOUBLE, 20 per LONGTEXT, measured"),
                "emitted_statement": {"unit": "bytes", "source": "active max_allowed_packet; \
                rows as literals in multi-row INSERTs of at most 4 MiB, each case's INSERT \
                preflighted at 25 bytes per number plus its escaped strings"},
                "numeric": "DOUBLE; NaN, infinities and negative zero refused (Dolt stores -0 as 0)",
                "text": "LONGTEXT NOT NULL",
                "atomic_schema_change": false,
                "failure_cleanup": "the data table is dropped when the rows or catalog fail",
                "version_control": "never commits, branches, merges or resets; reads write nothing",
            },
        },
    })
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
    fn url_parameters_of_our_own_are_taken_off() {
        assert_eq!(
            take_params(
                "postgres://u@h/db?schema=s1&connect_timeout=5&sslmode=verify-ca",
                ["schema", "sslmode"]
            ),
            (
                "postgres://u@h/db?connect_timeout=5".into(),
                [Some("s1".into()), Some("verify-ca".into())]
            )
        );
        assert_eq!(
            take_params("mysql://u@h/db", ["ssl-ca"]),
            ("mysql://u@h/db".into(), [None])
        );
    }

    #[test]
    fn tls_is_off_without_parameters_and_always_verified_when_on() {
        let full = |ca: Option<&str>| {
            Some(Tls {
                verify_host: true,
                ca: ca.map(Into::into),
            })
        };
        assert_eq!(tls(None, None), Ok(None));
        assert_eq!(tls(Some("DISABLED"), None), Ok(None));
        assert_eq!(tls(Some("require"), None), Ok(full(None)));
        assert_eq!(tls(Some("VERIFY_IDENTITY"), None), Ok(full(None)));
        assert_eq!(tls(None, Some("ca.pem".into())), Ok(full(Some("ca.pem"))));
        assert_eq!(
            tls(Some("VERIFY_CA"), Some("ca.pem".into())),
            Ok(Some(Tls {
                verify_host: false,
                ca: Some("ca.pem".into())
            }))
        );
        for refused in ["prefer", "PREFERRED", "allow", "verify", "disable"] {
            let ca = (refused == "disable").then(|| "ca.pem".to_string());
            assert!(tls(Some(refused), ca).is_err(), "{refused}");
        }
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
