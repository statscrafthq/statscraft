//! The published cases on SQLite: the round trip of every fixture, `spss-sav-zsav-1.0.json`
//! and the in-place binding manifests (see `common::runner`). Without a specification
//! checkout the manifest tests skip with a message.

mod common;

use common::runner::{Backend, in_place_conformance, query, round_trip_fixtures, sav_conformance};
use common::with_spec;
use statscraft_openstatspec::db::{Db, Sql};
use statscraft_openstatspec::sqlite::SqliteDb;

struct Sqlite;

impl Backend for Sqlite {
    fn fresh(&self) -> Box<dyn Db> {
        Box::new(SqliteDb::memory().unwrap())
    }

    fn columns(&self, db: &mut dyn Db, table: &str) -> Vec<String> {
        query(
            db,
            "SELECT name FROM pragma_table_info(?)",
            &[Sql::Text(table.into())],
        )
        .into_iter()
        .map(|r| r[0].as_str().unwrap().to_string())
        .collect()
    }

    fn temporary(&self, db: &mut dyn Db) -> i64 {
        query(db, "SELECT count(*) FROM sqlite_temp_schema", &[])[0][0]
            .as_i64()
            .unwrap()
    }
}

#[test]
fn every_fixture_round_trips_through_sqlite_and_back_to_sav() {
    round_trip_fixtures(&Sqlite);
}

#[test]
fn sav_sav_zsav_1_0_on_sqlite() {
    with_spec(|dir| sav_conformance(&Sqlite, dir));
}

#[test]
fn in_place_transformation_bindings_on_sqlite() {
    with_spec(|dir| in_place_conformance(&Sqlite, dir));
}
