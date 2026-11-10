//! MySQL and MariaDB connections: the catalog lives in the dedicated database the URL names.
//! The product is decided by positive identity on the connection, not by the protocol.
//! Plain TCP, no TLS.

use crate::catalog::prepare;
use crate::db::{Db, DbError, Dialect, Limits, MARIADB, MYSQL, Result, Sql};
use mysql::prelude::Queryable;
use mysql::{Conn, Opts, OptsBuilder, Params, Value};

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
    dialect: &'static Dialect,
    database: String,
}

/// Claimed release series of the server version policy.
const MYSQL_SERIES: [&str; 2] = ["8.4", "9.7"];
const MARIADB_SERIES: [&str; 3] = ["11.4", "11.8", "12.3"];

fn unsupported(message: String) -> DbError {
    DbError::new("server_unsupported", message)
}

/// "11.8.9-MariaDB-ubu2404" and "8.4.9" give "11.8" and "8.4".
fn series(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// Connects to a `mysql://user[:password]@host[:port]/database` URL and verifies the catalog
/// in that database, or installs it into an empty one when `create` is set. `password` is used
/// when the URL has none. The server must identify itself as MySQL 8.4.x or 9.7.x, or MariaDB
/// 11.4.x, 11.8.x or 12.3.x, with 16 KiB InnoDB pages; anything else, Dolt included, is
/// refused before anything is read or written.
pub fn open(url: &str, password: Option<&str>, create: bool) -> Result<MyDb> {
    let opts = Opts::from_url(url)
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
    let conn =
        Conn::new(builder).map_err(|e| DbError::new("database_unavailable", e.to_string()))?;
    let mut db = MyDb {
        conn,
        dialect: &MYSQL,
        database,
    };
    let row = db
        .query("SELECT VERSION(), @@version_comment", &[])?
        .remove(0);
    let (version, comment) = (row[0].text()?, row[1].text()?);
    let product = if comment.trim().eq_ignore_ascii_case("dolt") {
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
}

impl Db for MyDb {
    fn dialect(&self) -> &'static Dialect {
        self.dialect
    }

    fn exec(&mut self, sql: &str, values: &[Sql]) -> Result<u64> {
        self.conn.exec_drop(sql, params(values))?;
        Ok(self.conn.affected_rows())
    }

    fn query(&mut self, sql: &str, values: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        let rows: Vec<mysql::Row> = self.conn.exec(sql, params(values))?;
        rows.into_iter()
            .map(|row| row.unwrap().into_iter().map(from_value).collect())
            .collect()
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

    /// 1,017 InnoDB columns; identifiers of 64 characters; a TEXT value of 65,535 bytes; the
    /// worst-case DYNAMIC record of 8,126 bytes that a 16 KiB page allows; one statement of
    /// `max_allowed_packet` bytes.
    fn limits(&mut self) -> Result<Limits> {
        let packet = self.query("SELECT @@max_allowed_packet", &[])?[0][0].int()?;
        Ok(Limits {
            columns: 1017,
            identifier: 64,
            value_bytes: 65535,
            row_bytes: 8126,
            statement_bytes: packet as u64,
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
}
