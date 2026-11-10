//! MySQL, MariaDB and Dolt connections: the catalog lives in the dedicated database the URL
//! names. The product is decided by positive identity on the connection, never by the
//! protocol, so a MySQL connection refuses Dolt and a Dolt connection refuses MySQL.
//! Plain TCP, or TLS through rustls.

use crate::catalog::prepare;
use crate::db::{
    DOLT, Db, DbError, Dialect, Limits, MARIADB, MYSQL, Result, Sql, placeholders, take_params, tls,
};
use mysql::prelude::Queryable;
use mysql::{Conn, Opts, OptsBuilder, Params, SslOpts, Value};

impl From<mysql::Error> for DbError {
    fn from(e: mysql::Error) -> Self {
        match e {
            mysql::Error::MySqlError(e) => DbError::new("database_error", e.message),
            e => DbError::new("database_error", e.to_string()),
        }
    }
}

pub struct MyDb {
    conn: Conn,
    opts: Opts,
    dialect: &'static Dialect,
    database: String,
    /// Dolt: a product version that may be read but not written.
    read_only: Option<String>,
    /// Dolt: a second session that sees branch heads outside this session's transaction.
    probe: Option<Conn>,
}

/// Claimed release series of the server version policy.
const MYSQL_SERIES: [&str; 2] = ["8.4", "9.7"];
const MARIADB_SERIES: [&str; 3] = ["11.4", "11.8", "12.3"];

/// The exact Dolt versions this adapter release was tested on and writes to (Database I/O
/// Execution Policy v1, packaged policy). Other 2.2.x versions from 2.2.2 on may be read.
pub const DOLT_TESTED: [&str; 2] = ["2.2.2", "2.2.3"];

fn unsupported(message: String) -> DbError {
    DbError::new("server_unsupported", message)
}

/// "11.8.9-MariaDB-ubu2404" and "8.4.9" give "11.8" and "8.4".
fn series(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// The Dolt family claim: `>=2.2.2,<2.3.0`, exact versions only.
fn dolt_claimed(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    matches!(parts[..], ["2", "2", patch]
        if patch.bytes().all(|b| b.is_ascii_digit()) && patch.parse::<u32>().is_ok_and(|p| p >= 2))
}

/// Opens the connection and reads `VERSION()` and `@@version_comment`; nothing is written.
/// `ssl-mode` (`DISABLED`, `REQUIRED`, `VERIFY_CA`, `VERIFY_IDENTITY`) and `ssl-ca` (a PEM file
/// of trusted CAs) select TLS, as [`crate::db::tls`] describes.
fn connect(url: &str, password: Option<&str>) -> Result<(MyDb, String, String)> {
    let (url, [mode, ca]) = take_params(url, ["ssl-mode", "ssl-ca"]);
    let tls = tls(mode.as_deref(), ca)?;
    let opts = Opts::from_url(&url)
        .map_err(|_| DbError::new("database_unavailable", "the MySQL URL is not valid"))?;
    let Some(database) = opts.get_db_name().map(str::to_string) else {
        return Err(DbError::new(
            "database_unavailable",
            "the URL must name the database dedicated to the catalog",
        ));
    };
    let mut builder = OptsBuilder::from_opts(opts.clone());
    if let (None, Some(password)) = (opts.get_pass(), password) {
        builder = builder.pass(Some(password));
    }
    if let Some(tls) = tls {
        // the client library trusts its bundled roots besides the CA file; a local socket
        // would skip TLS
        let ssl = SslOpts::default()
            .with_root_cert_path(tls.ca)
            .with_danger_skip_domain_validation(!tls.verify_host);
        builder = builder.ssl_opts(Some(ssl)).prefer_socket(false);
    }
    let opts = Opts::from(builder);
    let conn =
        Conn::new(opts.clone()).map_err(|e| DbError::new("database_unavailable", e.to_string()))?;
    let mut db = MyDb {
        conn,
        opts,
        dialect: &MYSQL,
        database,
        read_only: None,
        probe: None,
    };
    let row = db
        .query("SELECT VERSION(), @@version_comment", &[])?
        .remove(0);
    let (version, comment) = (row[0].text()?, row[1].text()?);
    Ok((db, version, comment))
}

fn is_dolt(comment: &str) -> bool {
    comment.trim().to_lowercase() == "dolt"
}

/// Connects to a `mysql://user[:password]@host[:port]/database` URL and verifies the catalog
/// in that database, or installs it into an empty one when `create` is set. `password` is used
/// when the URL has none. The server must identify itself as MySQL 8.4.x or 9.7.x, or MariaDB
/// 11.4.x, 11.8.x or 12.3.x, with 16 KiB InnoDB pages; anything else, Dolt included, is
/// refused before anything is read or written.
pub fn open(url: &str, password: Option<&str>, create: bool) -> Result<MyDb> {
    let (mut db, version, comment) = connect(url, password)?;
    let product = if is_dolt(&comment) {
        return Err(unsupported(
            "the server is Dolt, which has its own profile; use the Dolt connection".into(),
        ));
    } else if version.contains("MariaDB") {
        db.dialect = &MARIADB;
        MARIADB_SERIES.contains(&series(&version).as_str())
    } else if comment.contains("MySQL") {
        MYSQL_SERIES.contains(&series(&version).as_str())
    } else {
        return Err(unsupported(format!(
            "the server identifies as {version} ({comment}), neither MySQL nor MariaDB"
        )));
    };
    if !product {
        return Err(unsupported(format!(
            "{} {version}: the profile supports MySQL 8.4.x and 9.7.x, MariaDB 11.4.x, 11.8.x and 12.3.x",
            db.dialect.profile
        )));
    }
    let page = db.query("SELECT @@innodb_page_size", &[])?[0][0].int()?;
    if page != 16384 {
        return Err(unsupported(format!(
            "innodb_page_size {page}: the row limits of the profile are for 16 KiB pages"
        )));
    }
    db.session()?;
    prepare(&mut db, create)?;
    Ok(db)
}

/// Connects to a Dolt sql-server at a `mysql://...` URL like [`open`]. The same connection
/// must show `@@version_comment` Dolt and a `DOLT_VERSION()` in the claimed family 2.2.x from
/// 2.2.2 on; writes (catalog install, import, apply) need one of [`DOLT_TESTED`]. Reading
/// writes nothing, so the working set stays as it was. The connection stays on the branch the
/// session starts on (the database's default branch, or `database/branch` in the URL).
pub fn open_dolt(url: &str, password: Option<&str>, create: bool) -> Result<MyDb> {
    let (mut db, version, comment) = connect(url, password)?;
    if !is_dolt(&comment) {
        return Err(unsupported(format!(
            "the server identifies as {version} ({comment}), not Dolt"
        )));
    }
    db.dialect = &DOLT;
    let version = db.query("SELECT DOLT_VERSION()", &[])?[0][0].text()?;
    if !dolt_claimed(&version) {
        return Err(unsupported(format!(
            "Dolt {version:?}: the Dolt profile supports 2.2.x from 2.2.2 on"
        )));
    }
    if !DOLT_TESTED.contains(&version.as_str()) {
        db.read_only = Some(version);
    }
    db.session()?;
    prepare(&mut db, create)?;
    Ok(db)
}

fn to_value(v: &Sql) -> Value {
    match v {
        Sql::Null => Value::NULL,
        Sql::Int(i) => Value::Int(*i),
        Sql::Real(r) => Value::Double(*r),
        Sql::Text(t) => Value::Bytes(t.as_bytes().to_vec()),
    }
}

fn from_value(v: Value) -> Result<Sql> {
    Ok(match v {
        Value::NULL => Sql::Null,
        Value::Int(i) => Sql::Int(i),
        Value::UInt(u) => Sql::Int(u as i64),
        Value::Float(f) => Sql::Real(f.into()),
        Value::Double(d) => Sql::Real(d),
        Value::Bytes(b) => Sql::Text(
            String::from_utf8(b).map_err(|_| DbError::new("data_invalid", "text is not UTF-8"))?,
        ),
        Value::Date(y, mo, d, h, mi, s, us) => Sql::Text(format!(
            "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{us:06}"
        )),
        Value::Time(..) => Sql::Text(v.as_sql(true)),
    })
}

/// A value as a SQL literal. Numbers keep their exact bits (the shortest digits that parse
/// back to them); text escapes the quote, the backslash and NUL, as the session's sql_mode
/// keeps backslash escapes.
fn literal(v: &Sql) -> String {
    let mut out = String::new();
    push_literal(&mut out, v);
    out
}

fn push_literal(out: &mut String, v: &Sql) {
    use std::fmt::Write;
    match v {
        Sql::Null => out.push_str("NULL"),
        Sql::Int(i) => write!(out, "{i}").expect("a String takes any text"),
        Sql::Real(r) => write!(out, "{r:e}").expect("a String takes any text"),
        Sql::Text(t) => {
            out.reserve(t.len() + 2);
            out.push('\'');
            for c in t.chars() {
                match c {
                    '\'' => out.push_str("''"),
                    '\\' => out.push_str("\\\\"),
                    '\0' => out.push_str("\\0"),
                    c => out.push(c),
                }
            }
            out.push('\'');
        }
    }
}

/// The text of a multi-row INSERT that is worth sending at once: larger statements are no
/// faster and cost memory on both sides.
const BATCH_BYTES: usize = 4 << 20;

/// A statement over the dialect's parameter limit fails here instead of waiting forever.
fn check_params(dialect: &Dialect, values: &[Sql]) -> Result<()> {
    if values.len() > dialect.max_params {
        return Err(DbError::capability(
            "statement",
            "emitted_statement",
            dialect.max_params as u64,
            values.len() as u64,
            "parameters",
        ));
    }
    Ok(())
}

fn params(values: &[Sql]) -> Params {
    if values.is_empty() {
        Params::Empty
    } else {
        Params::Positional(values.iter().map(to_value).collect())
    }
}

impl MyDb {
    /// Strict mode, so that nothing is truncated or substituted silently.
    fn session(&mut self) -> Result<()> {
        self.batch("SET SESSION sql_mode = 'STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION'")
    }

    /// One INSERT of `rows` rows whose parameters are `batch`, which it empties.
    fn insert_prepared(&mut self, head: &str, rows: usize, batch: &mut Vec<Value>) -> Result<()> {
        let one = format!("({})", vec!["?"; batch.len() / rows].join(","));
        let sql = format!("{head}{}", vec![one.as_str(); rows].join(","));
        self.conn
            .exec_drop(sql, Params::Positional(std::mem::take(batch)))?;
        Ok(())
    }

    /// Multi-row INSERTs of literals; `limit` is the packet's room for the statement.
    fn insert_literals(
        &mut self,
        head: &str,
        limit: usize,
        rows: &mut dyn Iterator<Item = Vec<Sql>>,
    ) -> Result<()> {
        let (mut sql, mut values) = (String::new(), String::new());
        for row in rows {
            values.clear();
            for (i, v) in row.iter().enumerate() {
                values.push(if i == 0 { '(' } else { ',' });
                push_literal(&mut values, v);
            }
            values.push(')');
            if head.len() + values.len() > limit {
                return Err(DbError::capability(
                    "row",
                    "emitted_statement",
                    limit as u64,
                    (head.len() + values.len()) as u64,
                    "bytes",
                ));
            }
            if !sql.is_empty() && sql.len() + 1 + values.len() > limit.min(BATCH_BYTES) {
                self.conn.query_drop(&sql)?;
                sql.clear();
            }
            sql.push_str(if sql.is_empty() { head } else { "," });
            sql.push_str(&values);
        }
        if !sql.is_empty() {
            self.conn.query_drop(&sql)?;
        }
        Ok(())
    }
}

impl Db for MyDb {
    fn dialect(&self) -> &'static Dialect {
        self.dialect
    }

    fn exec(&mut self, sql: &str, values: &[Sql]) -> Result<u64> {
        if self.dialect.profile == "dolt" {
            // Dolt prepares a statement on a wide table slowly, and one with 1,024 or more
            // parameters not at all; a text statement with literals takes milliseconds
            let mut values = values.iter();
            let sql = placeholders(sql, |_| values.next().map_or_else(String::new, literal));
            self.conn.query_drop(sql)?;
            return Ok(self.conn.affected_rows());
        }
        check_params(self.dialect, values)?;
        self.conn.exec_drop(sql, params(values))?;
        Ok(self.conn.affected_rows())
    }

    fn query(&mut self, sql: &str, values: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        let mut rows = vec![];
        self.query_each(sql, values, &mut |row| {
            rows.push(row);
            Ok(())
        })?;
        Ok(rows)
    }

    /// The rows come off the connection one by one; the client buffers no result.
    fn query_each(
        &mut self,
        sql: &str,
        values: &[Sql],
        f: &mut dyn FnMut(Vec<Sql>) -> Result<()>,
    ) -> Result<()> {
        check_params(self.dialect, values)?;
        for row in self.conn.exec_iter(sql, params(values))? {
            let row = row?
                .unwrap()
                .into_iter()
                .map(from_value)
                .collect::<Result<_>>()?;
            f(row)?;
        }
        Ok(())
    }

    fn batch(&mut self, sql: &str) -> Result<()> {
        Ok(self.conn.query_drop(sql)?)
    }

    fn objects(&mut self) -> Result<Vec<String>> {
        self.query(
            "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE()",
            &[],
        )?
        .iter()
        .map(|r| r[0].text())
        .collect()
    }

    fn namespace(&self) -> Option<String> {
        Some(self.database.clone())
    }

    /// MySQL and MariaDB: 1,017 InnoDB columns; identifiers of 64 characters; a TEXT value of
    /// 65,535 bytes; the worst-case DYNAMIC record of 8,126 bytes that a 16 KiB page allows.
    /// Dolt 2.2.2 and 2.2.3 (measured): 4,096 columns besides `__case_ordinal`, identifiers of
    /// 64 characters, a table definition below 65,504 bytes, LONGTEXT values. Every profile:
    /// one statement of `max_allowed_packet` bytes.
    fn limits(&mut self) -> Result<Limits> {
        let packet = self.query("SELECT @@max_allowed_packet", &[])?[0][0].int()? as u64;
        Ok(if self.dialect.profile == "dolt" {
            Limits {
                columns: 4097,
                identifier: 64,
                value_bytes: packet.min(u32::MAX.into()),
                row_bytes: 65503,
                statement_bytes: packet,
            }
        } else {
            Limits {
                columns: 1017,
                identifier: 64,
                value_bytes: 65535,
                row_bytes: 8126,
                statement_bytes: packet,
            }
        })
    }

    /// Multi-row INSERTs, each within `max_allowed_packet` and [`BATCH_BYTES`]: prepared, with
    /// at most `max_params` parameters, on MySQL and MariaDB; as literals on Dolt, which
    /// prepares wide statements slowly. A case's bytes are counted as the preflight counts them.
    fn insert_rows(&mut self, table: &str, rows: &mut dyn Iterator<Item = Vec<Sql>>) -> Result<()> {
        let packet = self.query("SELECT @@max_allowed_packet", &[])?[0][0].int()? as usize;
        // the packet also holds the command byte
        let limit = packet.saturating_sub(1);
        let head = format!("INSERT INTO {} VALUES ", self.dialect.quote(table));
        if self.dialect.literal_rows {
            return self.insert_literals(&head, limit, rows);
        }
        let limit = limit.min(BATCH_BYTES);
        let (mut batch, mut count, mut bytes) = (vec![], 0, 128);
        for row in rows {
            let text: usize = row
                .iter()
                .map(|v| if let Sql::Text(t) = v { t.len() } else { 0 })
                .sum();
            let size = 11 * row.len() + text;
            if count > 0
                && (batch.len() + row.len() > self.dialect.max_params || bytes + size > limit)
            {
                self.insert_prepared(&head, count, &mut batch)?;
                (count, bytes) = (0, 128);
            }
            batch.extend(row.into_iter().map(|v| match v {
                Sql::Text(t) => Value::Bytes(t.into_bytes()),
                v => to_value(&v),
            }));
            count += 1;
            bytes += size;
        }
        if count > 0 {
            self.insert_prepared(&head, count, &mut batch)?;
        }
        Ok(())
    }

    fn write_gate(&self) -> Result<()> {
        match &self.read_only {
            Some(version) => Err(unsupported(format!(
                "Dolt {version} may be read but not written: writes are tested on {}",
                DOLT_TESTED.join(" and ")
            ))),
            None => Ok(()),
        }
    }

    fn committed_head(&mut self, branch: &str) -> Result<String> {
        if self.probe.is_none() {
            self.probe = Some(Conn::new(self.opts.clone())?);
        }
        let probe = self.probe.as_mut().expect("opened above");
        let head: Option<String> =
            probe.exec_first("SELECT hash FROM dolt_branches WHERE name = ?", (branch,))?;
        head.ok_or_else(|| {
            DbError::new(
                "dolt_context_changed",
                format!("branch {branch} no longer exists"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_series_come_from_the_version_string() {
        assert_eq!(series("11.8.9-MariaDB-ubu2404"), "11.8");
        assert_eq!(series("8.4.9"), "8.4");
        assert!(!MARIADB_SERIES.contains(&series("10.11.18-MariaDB").as_str()));
    }

    #[test]
    fn literals_keep_bits_and_escape_text() {
        assert_eq!(literal(&Sql::Real(5e-324)), "5e-324");
        assert_eq!(literal(&Sql::Real(-1.5)), "-1.5e0");
        assert_eq!(literal(&Sql::Real(f64::MAX)), "1.7976931348623157e308");
        for v in [0.1, 1.0 / 3.0, 2.225_073_858_507_201e-308, -123456.789e-200] {
            assert_eq!(
                literal(&Sql::Real(v)).parse::<f64>().unwrap().to_bits(),
                v.to_bits()
            );
        }
        assert_eq!(literal(&Sql::Text("a'b\\c\0d".into())), "'a''b\\\\c\\0d'");
        assert_eq!(literal(&Sql::Null), "NULL");
    }

    #[test]
    fn the_dolt_claim_is_2_2_x_from_2_2_2_on_with_exact_versions() {
        for (version, claimed) in [
            ("2.2.2", true),
            ("2.2.3", true),
            ("2.2.17", true),
            ("2.2.1", false),
            ("2.3.0", false),
            ("2.2", false),
            ("2.2.x", false),
            ("2.2.+3", false),
            ("2.2.3-rc1", false),
            ("8.0.31", false),
        ] {
            assert_eq!(dolt_claimed(version), claimed, "{version}");
        }
    }
}
