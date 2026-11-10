//! SQLite connections: a database file dedicated to the catalog.

use crate::catalog::prepare;
use crate::db::{Db, DbError, Dialect, Limits, Result, SQLITE, Sql};
use rusqlite::limits::Limit;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags, params_from_iter};
use std::path::Path;

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::new("database_error", e.to_string())
    }
}

pub struct SqliteDb {
    pub conn: Connection,
}

impl SqliteDb {
    /// Wraps a connection; foreign keys are enforced.
    pub fn new(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        Ok(SqliteDb { conn })
    }

    /// A private in-memory database with the catalog installed.
    pub fn memory() -> Result<Self> {
        let mut db = SqliteDb::new(Connection::open_in_memory()?)?;
        prepare(&mut db, true)?;
        Ok(db)
    }
}

/// Opens a database file dedicated to the catalog. With `create`, a missing or empty file gets
/// the catalog installed; without it the file must exist and already hold the catalog. Any
/// other content fails without being touched.
pub fn open(path: &Path, create: bool) -> Result<SqliteDb> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(path, flags)
        .map_err(|e| DbError::new("database_unavailable", format!("{}: {e}", path.display())))?;
    let mut db = SqliteDb::new(conn)?;
    prepare(&mut db, create)?;
    Ok(db)
}

fn param(v: &Sql) -> Value {
    match v {
        Sql::Null => Value::Null,
        Sql::Int(i) => Value::Integer(*i),
        Sql::Real(r) => Value::Real(*r),
        Sql::Text(t) => Value::Text(t.clone()),
    }
}

impl Db for SqliteDb {
    fn dialect(&self) -> &'static Dialect {
        &SQLITE
    }

    fn exec(&mut self, sql: &str, params: &[Sql]) -> Result<u64> {
        // cached: the INSERT of the data rows runs once per case
        let mut statement = self.conn.prepare_cached(sql)?;
        Ok(statement.execute(params_from_iter(params.iter().map(param)))? as u64)
    }

    fn query(&mut self, sql: &str, params: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        let mut statement = self.conn.prepare_cached(sql)?;
        let n = statement.column_count();
        let rows = statement.query_map(params_from_iter(params.iter().map(param)), |r| {
            (0..n)
                .map(|i| {
                    Ok(match r.get_ref(i)? {
                        ValueRef::Null => Sql::Null,
                        ValueRef::Integer(v) => Sql::Int(v),
                        ValueRef::Real(v) => Sql::Real(v),
                        ValueRef::Text(t) => Sql::Text(String::from_utf8_lossy(t).into_owned()),
                        ValueRef::Blob(_) => Sql::Null,
                    })
                })
                .collect()
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn batch(&mut self, sql: &str) -> Result<()> {
        Ok(self.conn.execute_batch(sql)?)
    }

    fn objects(&mut self) -> Result<Vec<String>> {
        self.query(
            "SELECT name FROM sqlite_schema WHERE type IN ('table', 'view', 'trigger')",
            &[],
        )?
        .iter()
        .map(|r| r[0].text())
        .collect()
    }

    fn namespace(&self) -> Option<String> {
        None
    }

    /// The profile's 2,000 columns or the build's lower column limit; the length limit bounds
    /// a value and a row. Parameters are not part of the SQL text, so no statement limit.
    fn limits(&mut self) -> Result<Limits> {
        let length = self.conn.limit(Limit::SQLITE_LIMIT_LENGTH)? as u64;
        Ok(Limits {
            columns: 2000.min(self.conn.limit(Limit::SQLITE_LIMIT_COLUMN)? as usize),
            identifier: 255,
            value_bytes: length,
            row_bytes: length,
            statement_bytes: u64::MAX,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{read_dataset, write_dataset};
    use statscraft_data::{Dataset, Values};

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

    fn scalar(db: &mut SqliteDb, sql: &str) -> Sql {
        db.query(sql, &[]).unwrap()[0][0].clone()
    }

    #[test]
    fn values_round_trip_bit_exact_with_reserved_words_and_generated_names() {
        let mut db = SqliteDb::memory().unwrap();
        let d = sample();
        write_dataset(&mut db, &d, "s").unwrap();
        let (back, losses) = read_dataset(&mut db, "s", false).unwrap();
        assert!(losses.is_empty());
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
        let physical = scalar(
            &mut db,
            "SELECT physical_name FROM variable WHERE source_name = 'Ä'",
        );
        assert_eq!(physical, Sql::Text("v3".into()));
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
            let mut db = SqliteDb::memory().unwrap();
            let mut d = sample();
            d.set_numeric_values("x", vec![Some(1.0), Some(bad), None])
                .unwrap();
            let e = write_dataset(&mut db, &d, "s").unwrap_err();
            assert_eq!(e.code, "target_capability_exceeded");
            let state = db
                .query(
                    "SELECT (SELECT count(*) FROM dataset),
                            (SELECT count(*) FROM sqlite_schema WHERE name = 'data_s'),
                            (SELECT status FROM operation),
                            (SELECT source_item FROM fidelity_event WHERE dataset_id IS NULL)",
                    &[],
                )
                .unwrap();
            assert_eq!(
                state[0],
                [
                    Sql::Int(0),
                    Sql::Int(0),
                    Sql::Text("failed".into()),
                    Sql::Text("x".into())
                ]
            );
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
        let mut db = SqliteDb::memory().unwrap();
        write_dataset(&mut db, &wide(1999), "fits").unwrap();
        let e = write_dataset(&mut db, &wide(2000), "over").unwrap_err();
        assert_eq!(e.code, "target_capability_exceeded");
        assert_eq!(e.source_item.as_deref(), Some("v1999"));
        assert_eq!(e.detail.unwrap()["required"], 2001);
    }

    #[test]
    fn foreign_databases_are_refused_untouched() {
        let mut db = SqliteDb::new(Connection::open_in_memory().unwrap()).unwrap();
        db.batch("CREATE TABLE app (x)").unwrap();
        assert_eq!(prepare(&mut db, true).unwrap_err().code, "catalog_foreign");
        let mut empty = SqliteDb::new(Connection::open_in_memory().unwrap()).unwrap();
        assert_eq!(
            prepare(&mut empty, false).unwrap_err().code,
            "catalog_missing"
        );
        let mut ours = SqliteDb::memory().unwrap();
        ours.batch("CREATE TABLE extra (x)").unwrap();
        assert_eq!(
            prepare(&mut ours, false).unwrap_err().code,
            "catalog_foreign"
        );
    }

    #[test]
    fn duplicate_dataset_names_are_refused() {
        let mut db = SqliteDb::memory().unwrap();
        write_dataset(&mut db, &sample(), "s").unwrap();
        assert_eq!(
            write_dataset(&mut db, &sample(), "S").unwrap_err().code,
            "dataset_exists"
        );
        assert_eq!(
            write_dataset(&mut db, &sample(), "a b").unwrap_err().code,
            "dataset_name_invalid"
        );
    }

    #[test]
    fn catalog_content_the_model_cannot_hold_needs_allow_loss() {
        let mut db = SqliteDb::memory().unwrap();
        write_dataset(&mut db, &sample(), "s").unwrap();
        // the model has no write format of its own: one that differs from the print format
        db.exec(
            "UPDATE variable SET write_format_width = write_format_width + 1
             WHERE source_ordinal = 1",
            &[],
        )
        .unwrap();
        let e = read_dataset(&mut db, "s", false).err().unwrap();
        assert_eq!(e.code, "export_fidelity_loss");
        let (_, losses) = read_dataset(&mut db, "s", true).unwrap();
        assert_eq!(losses, ["x: write_format"]);
        let events = scalar(
            &mut db,
            "SELECT count(*) FROM fidelity_event WHERE event_code = 'export_loss_allowed'",
        );
        assert_eq!(events, Sql::Int(1));
    }
}
