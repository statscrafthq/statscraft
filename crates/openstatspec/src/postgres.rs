//! PostgreSQL connections: the catalog lives in one dedicated schema, fixed as the only
//! schema on the connection's search path. Plain TCP, no TLS.

use crate::catalog::prepare;
use crate::db::{Db, DbError, Dialect, Limits, POSTGRESQL, Result, Sql};
use bytes::{BufMut, BytesMut};
use postgres::error::SqlState;
use postgres::types::{FromSql, IsNull, ToSql, Type, to_sql_checked};
use postgres::{Client, NoTls, Statement};
use std::collections::HashMap;
use std::error::Error;

/// The catalog schema when the URL names none.
pub const DEFAULT_SCHEMA: &str = "openstatspec";

/// Claimed release series of the server version policy: 17.x and 18.x.
const SUPPORTED: std::ops::Range<i64> = 170000..190000;

impl From<postgres::Error> for DbError {
    fn from(e: postgres::Error) -> Self {
        let message = e
            .as_db_error()
            .map_or_else(|| e.to_string(), |d| d.message().to_string());
        // a tuple over the page limit ("row is too big") is the engine's own row check
        if e.code() == Some(&SqlState::PROGRAM_LIMIT_EXCEEDED) {
            let mut error = DbError::new("target_capability_exceeded", message);
            error.source_item = Some("row".into());
            return error;
        }
        DbError::new("database_error", message)
    }
}

pub struct PgDb {
    client: Client,
    schema: String,
    statements: HashMap<String, Statement>,
}

/// Splits the `schema` query parameter off a postgres:// URL; the client library does not
/// know it.
fn split_schema(url: &str) -> (String, String) {
    let Some((base, query)) = url.split_once('?') else {
        return (url.to_string(), DEFAULT_SCHEMA.to_string());
    };
    let mut schema = DEFAULT_SCHEMA.to_string();
    let rest: Vec<&str> = query
        .split('&')
        .filter(|p| match p.strip_prefix("schema=") {
            Some(s) => {
                schema = s.to_string();
                false
            }
            None => true,
        })
        .collect();
    let url = if rest.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", rest.join("&"))
    };
    (url, schema)
}

/// Connects to a `postgres://user[:password]@host[:port]/database[?schema=NAME]` URL and
/// verifies the catalog in schema NAME (default `openstatspec`), or installs it into a missing
/// or empty schema when `create` is set. `password` is used when the URL has none. Server
/// versions outside 17.x and 18.x and servers whose encoding is not UTF8 are refused before
/// anything is read or written.
pub fn open(url: &str, password: Option<&str>, create: bool) -> Result<PgDb> {
    let (url, schema) = split_schema(url);
    let mut config: postgres::Config = url
        .parse()
        .map_err(|_| DbError::new("database_unavailable", "the PostgreSQL URL is not valid"))?;
    if let (None, Some(password)) = (config.get_password(), password) {
        config.password(password);
    }
    let client = config
        .connect(NoTls)
        .map_err(|e| DbError::new("database_unavailable", e.to_string()))?;
    let mut db = PgDb {
        client,
        schema,
        statements: HashMap::new(),
    };
    let setting = |db: &mut PgDb, name: &str| -> Result<String> {
        db.query(&format!("SELECT current_setting('{name}')"), &[])?[0][0].text()
    };
    let version: i64 = setting(&mut db, "server_version_num")?.parse().unwrap_or(0);
    if !SUPPORTED.contains(&version) {
        return Err(DbError::new(
            "server_unsupported",
            format!("PostgreSQL {version}: the PostgreSQL profile supports 17.x and 18.x"),
        ));
    }
    let encoding = setting(&mut db, "server_encoding")?;
    if encoding != "UTF8" {
        return Err(DbError::new(
            "server_unsupported",
            format!("server encoding {encoding}: the PostgreSQL profile needs UTF8"),
        ));
    }
    let path = format!("SET search_path TO {}", POSTGRESQL.quote(&db.schema));
    db.batch(&path)?;
    prepare(&mut db, create)?;
    Ok(db)
}

/// Rewrites `?` placeholders outside string literals as `$1`, `$2`, ...
fn numbered(sql: &str) -> String {
    let (mut out, mut n, mut quoted) = (String::with_capacity(sql.len()), 0, false);
    for c in sql.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                out.push(c);
            }
            '?' if !quoted => {
                n += 1;
                out += &format!("${n}");
            }
            _ => out.push(c),
        }
    }
    out
}

impl PgDb {
    fn statement(&mut self, sql: &str) -> Result<Statement> {
        if let Some(s) = self.statements.get(sql) {
            return Ok(s.clone());
        }
        let statement = self.client.prepare(&numbered(sql))?;
        self.statements.insert(sql.to_string(), statement.clone());
        Ok(statement)
    }
}

impl Db for PgDb {
    fn dialect(&self) -> &'static Dialect {
        &POSTGRESQL
    }

    fn exec(&mut self, sql: &str, params: &[Sql]) -> Result<u64> {
        let statement = self.statement(sql)?;
        let refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as _).collect();
        Ok(self.client.execute(&statement, &refs)?)
    }

    fn query(&mut self, sql: &str, params: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        let statement = self.statement(sql)?;
        let refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as _).collect();
        self.client
            .query(&statement, &refs)?
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| Ok(row.try_get::<_, Sql>(i)?))
                    .collect()
            })
            .collect()
    }

    fn batch(&mut self, sql: &str) -> Result<()> {
        // DDL can change what a prepared statement returns
        if !sql.starts_with("BEGIN") && sql != "COMMIT" && sql != "ROLLBACK" {
            self.statements.clear();
        }
        Ok(self.client.batch_execute(sql)?)
    }

    fn objects(&mut self) -> Result<Vec<String>> {
        let schema = self.schema.clone();
        self.query(
            "SELECT c.relname::text FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = ? AND c.relkind IN ('r', 'p', 'v', 'm', 'f', 'S')",
            &[Sql::Text(schema)],
        )?
        .iter()
        .map(|r| r[0].text())
        .collect()
    }

    fn namespace(&self) -> Option<String> {
        Some(self.schema.clone())
    }

    /// 1,600 columns; the active `max_identifier_length` in bytes of the UTF8 server encoding;
    /// 1 GiB less one byte for a text value and a protocol message; a heap tuple of the block
    /// size less the page and item headers (8,160 bytes with 8 KiB blocks).
    fn limits(&mut self) -> Result<Limits> {
        let mut setting = |name: &str| -> Result<u64> {
            let text =
                self.query(&format!("SELECT current_setting('{name}')"), &[])?[0][0].text()?;
            text.parse()
                .map_err(|_| DbError::new("database_error", format!("{name} = {text}")))
        };
        let identifier = setting("max_identifier_length")? as usize;
        let block = setting("block_size")?;
        Ok(Limits {
            columns: 1600,
            identifier,
            value_bytes: (1 << 30) - 1,
            row_bytes: block - 32,
            statement_bytes: (1 << 30) - 1,
        })
    }

    fn create_namespace(&mut self) -> Result<()> {
        let sql = format!(
            "CREATE SCHEMA IF NOT EXISTS {}",
            POSTGRESQL.quote(&self.schema)
        );
        self.batch(&sql)
    }
}

type BoxError = Box<dyn Error + Sync + Send>;

impl ToSql for Sql {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> std::result::Result<IsNull, BoxError> {
        match self {
            Sql::Null => Ok(IsNull::Yes),
            Sql::Int(v) if *ty == Type::INT8 => v.to_sql(ty, out),
            Sql::Int(v) if *ty == Type::INT4 => i32::try_from(*v)?.to_sql(ty, out),
            Sql::Int(v) if *ty == Type::FLOAT8 => (*v as f64).to_sql(ty, out),
            // a timestamp read back from the server, in its microseconds since 2000-01-01
            Sql::Int(v) if *ty == Type::TIMESTAMPTZ || *ty == Type::TIMESTAMP => {
                out.put_i64(*v);
                Ok(IsNull::No)
            }
            Sql::Int(v) if <&str as ToSql>::accepts(ty) => v.to_string().as_str().to_sql(ty, out),
            Sql::Real(v) if *ty == Type::FLOAT8 => v.to_sql(ty, out),
            Sql::Text(s) if *ty == Type::UUID => {
                let hex: String = s.chars().filter(|c| *c != '-').collect();
                if hex.len() != 32 {
                    return Err(format!("{s} is not a UUID").into());
                }
                for i in (0..32).step_by(2) {
                    out.put_u8(u8::from_str_radix(&hex[i..i + 2], 16)?);
                }
                Ok(IsNull::No)
            }
            Sql::Text(s) if <&str as ToSql>::accepts(ty) => s.as_str().to_sql(ty, out),
            _ => Err(format!("{self:?} cannot be sent as {ty}").into()),
        }
    }

    fn accepts(_: &Type) -> bool {
        true
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for Sql {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> std::result::Result<Self, BoxError> {
        Ok(if *ty == Type::INT8 {
            Sql::Int(i64::from_sql(ty, raw)?)
        } else if *ty == Type::INT4 {
            Sql::Int(i32::from_sql(ty, raw)?.into())
        } else if *ty == Type::INT2 {
            Sql::Int(i16::from_sql(ty, raw)?.into())
        } else if *ty == Type::FLOAT8 {
            Sql::Real(f64::from_sql(ty, raw)?)
        } else if *ty == Type::BOOL {
            Sql::Int(bool::from_sql(ty, raw)?.into())
        } else if *ty == Type::TIMESTAMPTZ || *ty == Type::TIMESTAMP {
            Sql::Int(i64::from_sql(&Type::INT8, raw)?)
        } else if *ty == Type::UUID {
            let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            Sql::Text(format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            ))
        } else {
            Sql::Text(<&str>::from_sql(ty, raw)?.to_string())
        })
    }

    fn from_sql_null(_: &Type) -> std::result::Result<Self, BoxError> {
        Ok(Sql::Null)
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_numbered_outside_string_literals() {
        assert_eq!(
            numbered("SELECT ?, '?', x ~ '^a?$' FROM t WHERE y = ?"),
            "SELECT $1, '?', x ~ '^a?$' FROM t WHERE y = $2"
        );
    }

    #[test]
    fn the_schema_parameter_is_taken_off_the_url() {
        assert_eq!(
            split_schema("postgres://u@h/db?schema=s1&connect_timeout=5"),
            ("postgres://u@h/db?connect_timeout=5".into(), "s1".into())
        );
        assert_eq!(
            split_schema("postgres://u@h/db"),
            ("postgres://u@h/db".into(), DEFAULT_SCHEMA.into())
        );
    }
}
