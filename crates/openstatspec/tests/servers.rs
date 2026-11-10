//! The published cases and the profile boundaries on database servers. Every test is ignored
//! by default and needs a throwaway server named by an environment variable; each test creates
//! its own schema or database there and leaves it behind.
//!
//! ```text
//! STATSCRAFT_PG_URL=postgres://postgres:PASSWORD@127.0.0.1:PORT/postgres \
//! STATSCRAFT_MYSQL_URL=mysql://root:PASSWORD@127.0.0.1:PORT/any_database \
//! STATSCRAFT_MARIADB_URL=mysql://root:PASSWORD@127.0.0.1:PORT/any_database \
//!   cargo test -p statscraft-openstatspec --test servers -- --ignored
//! ```
//!
//! STATSCRAFT_MARIADB10_URL, when set, names a MariaDB 10.11 server, which must be refused.

mod common;

use common::runner::{
    Backend, FailAt, in_place_conformance, query, round_trip_fixtures, sav_conformance,
};
use common::{differences, with_spec};
use mysql::prelude::Queryable;
use statscraft_data::{Dataset, Values};
use statscraft_openstatspec::catalog::{prepare, read_dataset, write_dataset};
use statscraft_openstatspec::db::{Db, Sql, uuid};
use statscraft_openstatspec::{mysql as my, postgres};

/// The server URL in `variable`, or a skip message.
fn server(variable: &str) -> Option<String> {
    let url = std::env::var(variable).ok();
    if url.is_none() {
        eprintln!("skipped: set {variable} to a throwaway server");
    }
    url
}

/// A new name for a schema or database.
fn fresh_name() -> String {
    let id = uuid().replace('-', "");
    format!("statscraft_test_{}", &id[id.len() - 16..])
}

fn text(s: &str) -> Sql {
    Sql::Text(s.into())
}

fn column_names(db: &mut dyn Db, sql: &str, table: &str) -> Vec<String> {
    query(db, sql, &[text(table)])
        .into_iter()
        .map(|r| r[0].as_str().unwrap().to_string())
        .collect()
}

/// Writes `d`, reads it back and compares every value bit for bit.
fn assert_values_round_trip(db: &mut dyn Db, d: &Dataset, name: &str) {
    write_dataset(db, d, name).unwrap();
    let (back, _) = read_dataset(db, name, false).unwrap();
    for (a, b) in d.variables().zip(back.variables()) {
        match (a.values(), b.values()) {
            (Values::Numeric(x), Values::Numeric(y)) => {
                let bits = |v: Option<f64>| v.map(f64::to_bits);
                let (x, y): (Vec<_>, Vec<_>) =
                    (x.iter().map(bits).collect(), y.iter().map(bits).collect());
                assert_eq!(x, y, "{}", a.name());
            }
            (Values::String(x), Values::String(y)) => assert!(x.iter().eq(y.iter())),
            _ => panic!("storage kind changed"),
        }
    }
}

/// A dataset with one numeric variable per value.
fn numbers(values: &[f64]) -> Dataset {
    let mut d = Dataset::new(1);
    for (i, v) in values.iter().enumerate() {
        d.set_numeric_values(&format!("x{i}"), vec![Some(*v)])
            .unwrap();
    }
    d
}

/// Signed zero, the subnormal boundaries, the smallest normal, the largest finite and
/// ordinary values.
const FINITE: [f64; 9] = [
    0.0,
    -0.0,
    5e-324,
    -5e-324,
    2.225_073_858_507_201e-308,
    2.225_073_858_507_201_4e-308,
    f64::MAX,
    f64::MIN,
    -1.5,
];

// ---------------------------------------------------------------- PostgreSQL

struct Postgres(String);

impl Postgres {
    fn get() -> Option<Postgres> {
        server("STATSCRAFT_PG_URL").map(Postgres)
    }

    fn url(&self, schema: &str) -> String {
        let sep = if self.0.contains('?') { '&' } else { '?' };
        format!("{}{sep}schema={schema}", self.0)
    }
}

impl Backend for Postgres {
    fn fresh(&self) -> Box<dyn Db> {
        Box::new(postgres::open(&self.url(&fresh_name()), None, true).unwrap())
    }

    fn columns(&self, db: &mut dyn Db, table: &str) -> Vec<String> {
        column_names(
            db,
            "SELECT column_name::text FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = ?",
            table,
        )
    }

    fn temporary(&self, db: &mut dyn Db) -> i64 {
        query(
            db,
            "SELECT count(*) FROM pg_catalog.pg_class WHERE relpersistence = 't'",
            &[],
        )[0][0]
            .as_i64()
            .unwrap()
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_round_trips_every_fixture() {
    if let Some(pg) = Postgres::get() {
        round_trip_fixtures(&pg);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_sav_zsav_1_0() {
    if let Some(pg) = Postgres::get() {
        with_spec(|dir| sav_conformance(&pg, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_in_place_bindings() {
    if let Some(pg) = Postgres::get() {
        with_spec(|dir| in_place_conformance(&pg, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_keeps_every_binary64_value_bit_for_bit() {
    let Some(pg) = Postgres::get() else { return };
    let mut db = pg.fresh();
    let mut values = FINITE.to_vec();
    // infinities and NaN, with a payload and with the sign bit set
    values.extend([
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::from_bits(0xfff8_0000_0000_0000),
    ]);
    assert_values_round_trip(db.as_mut(), &numbers(&values), "special");
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_preflight_refuses_what_the_server_cannot_hold() {
    let Some(pg) = Postgres::get() else { return };
    let mut db = pg.fresh();
    let tables = db.objects().unwrap().len();
    // 1,019 non-NULL doubles need at least 32 + 8152 = 8184 bytes, over the 8160 of a tuple
    let wide = numbers(&vec![1.0; 1019]);
    let e = write_dataset(db.as_mut(), &wide, "wide").unwrap_err();
    assert_eq!(
        (e.code, e.source_item.as_deref()),
        ("target_capability_exceeded", Some("row"))
    );
    assert_eq!(e.detail.unwrap()["limit"], "structural_row");
    // 1,016 fit: 32 + 8128
    write_dataset(db.as_mut(), &numbers(&vec![1.0; 1016]), "fits").unwrap();
    // NULLs take no space in a tuple
    let mut sparse = Dataset::new(1);
    for i in 0..1599 {
        sparse
            .set_numeric_values(&format!("v{i}"), vec![(i < 100).then_some(1.0)])
            .unwrap();
    }
    write_dataset(db.as_mut(), &sparse, "sparse").unwrap();
    // text cannot hold U+0000
    let mut nul = Dataset::new(1);
    nul.set_string_values("s", 3, vec!["a\0b".into()]).unwrap();
    let e = write_dataset(db.as_mut(), &nul, "nul").unwrap_err();
    assert_eq!(
        (e.code, e.source_item.as_deref()),
        ("target_capability_exceeded", Some("s"))
    );
    // data_ plus 59 characters is over the 63 bytes of max_identifier_length
    let e = write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(59)).unwrap_err();
    assert_eq!(e.detail.unwrap()["limit"], "identifier");
    write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(58)).unwrap();
    // failures left no table behind: two datasets were stored
    assert_eq!(db.objects().unwrap().len(), tables + 3);
    let failed = query(
        db.as_mut(),
        "SELECT count(*) FROM operation WHERE status = 'failed'",
        &[],
    );
    assert_eq!(failed[0][0], 3);
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL"]
fn postgres_refuses_a_foreign_schema_untouched() {
    let Some(pg) = Postgres::get() else { return };
    let schema = fresh_name();
    let mut db = pg.fresh();
    db.batch(&format!("CREATE SCHEMA {schema}")).unwrap();
    db.batch(&format!("CREATE TABLE {schema}.app (x int)"))
        .unwrap();
    let e = postgres::open(&pg.url(&schema), None, true).err().unwrap();
    assert_eq!(e.code, "catalog_foreign");
    let e = postgres::open(&pg.url(&fresh_name()), None, false)
        .err()
        .unwrap();
    assert_eq!(e.code, "catalog_missing");
    // the catalog of a fresh schema verifies again
    prepare(db.as_mut(), false).unwrap();
}

// ---------------------------------------------------------------- MySQL and MariaDB

/// A MySQL-protocol server; each fresh connection gets a new database.
struct MySql(String);

impl MySql {
    fn get(variable: &str) -> Option<MySql> {
        server(variable).map(MySql)
    }

    /// The URL with its database replaced by a new, empty one.
    fn new_database(&self) -> String {
        let name = fresh_name();
        let mut admin = mysql::Conn::new(mysql::Opts::from_url(&self.0).unwrap()).unwrap();
        admin.query_drop(format!("CREATE DATABASE {name}")).unwrap();
        let base = &self.0[..self.0.rfind('/').unwrap()];
        format!("{base}/{name}")
    }
}

impl Backend for MySql {
    fn fresh(&self) -> Box<dyn Db> {
        Box::new(my::open(&self.new_database(), None, true).unwrap())
    }

    fn columns(&self, db: &mut dyn Db, table: &str) -> Vec<String> {
        column_names(
            db,
            "SELECT COLUMN_NAME FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?",
            table,
        )
    }
}

const MYSQL: &str = "STATSCRAFT_MYSQL_URL";
const MARIADB: &str = "STATSCRAFT_MARIADB_URL";

fn mysql_family() -> Vec<MySql> {
    [MYSQL, MARIADB]
        .iter()
        .filter_map(|v| MySql::get(v))
        .collect()
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_round_trip_every_fixture() {
    for server in mysql_family() {
        round_trip_fixtures(&server);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_sav_zsav_1_0() {
    for server in mysql_family() {
        with_spec(|dir| sav_conformance(&server, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_in_place_bindings() {
    for server in mysql_family() {
        with_spec(|dir| in_place_conformance(&server, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_keep_finite_values_and_refuse_negative_zero_and_the_others() {
    for server in mysql_family() {
        let mut db = server.fresh();
        let finite: Vec<f64> = FINITE
            .into_iter()
            .filter(|v| v.to_bits() != 1 << 63)
            .collect();
        assert_values_round_trip(db.as_mut(), &numbers(&finite), "finite");
        for bad in [-0.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let e = write_dataset(db.as_mut(), &numbers(&[1.0, bad]), "bad").unwrap_err();
            assert_eq!(
                (e.code, e.source_item.as_deref()),
                ("target_capability_exceeded", Some("x1"))
            );
        }
        // a NUL inside text is kept
        let mut nul = Dataset::new(1);
        nul.set_string_values("s", 3, vec!["a\0b".into()]).unwrap();
        assert_values_round_trip(db.as_mut(), &nul, "nul");
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_preflight_matches_the_innodb_boundaries() {
    for server in mysql_family() {
        let mut db = server.fresh();
        let profile = db.dialect().profile;
        let texts = if profile == "mysql" { 197 } else { 385 };
        let strings = |n: usize| {
            let mut d = Dataset::new(1);
            for i in 0..n {
                d.set_string_values(&format!("s{i}"), 1, vec!["a".into()])
                    .unwrap();
            }
            d
        };
        let tables = db.objects().unwrap().len();
        // the largest tables the server creates, and one column more
        write_dataset(db.as_mut(), &numbers(&vec![1.0; 996]), "doubles").unwrap();
        write_dataset(db.as_mut(), &strings(texts), "texts").unwrap();
        for (d, name) in [
            (numbers(&vec![1.0; 997]), "doubles2"),
            (strings(texts + 1), "texts2"),
        ] {
            let e = write_dataset(db.as_mut(), &d, name).unwrap_err();
            assert_eq!(e.detail.unwrap()["limit"], "structural_row", "{profile}");
        }
        // 1,017 variables need 1,018 columns
        let e = write_dataset(db.as_mut(), &numbers(&vec![1.0; 1017]), "wide").unwrap_err();
        assert_eq!(e.detail.unwrap()["limit"], "physical_columns");
        // data_ plus 60 characters is over 64
        let e = write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(60)).unwrap_err();
        assert_eq!(e.detail.unwrap()["limit"], "identifier");
        write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(59)).unwrap();
        assert_eq!(db.objects().unwrap().len(), tables + 3, "{profile}");
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_and_mariadb_drop_the_table_when_a_later_write_fails() {
    for server in mysql_family() {
        let mut db = server.fresh();
        let tables = db.objects().unwrap();
        // the table and its rows exist when the catalog insert fails
        let mut failing = FailAt {
            inner: db.as_mut(),
            prefix: "INSERT INTO variable ",
        };
        let e = write_dataset(&mut failing, &numbers(&[1.0, 2.0]), "lost").unwrap_err();
        assert_eq!(e.code, "injected_fault");
        assert_eq!(db.objects().unwrap(), tables);
        let state = query(
            db.as_mut(),
            "SELECT (SELECT count(*) FROM dataset), (SELECT count(*) FROM variable),
                    (SELECT status FROM operation),
                    (SELECT event_code FROM fidelity_event WHERE dataset_id IS NULL)",
            &[],
        );
        assert_eq!(
            serde_json::json!(state[0]),
            serde_json::json!([0, 0, "failed", "injected_fault"])
        );
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MYSQL_URL and STATSCRAFT_MARIADB_URL"]
fn mysql_refuses_foreign_databases_other_products_and_other_versions() {
    for server in mysql_family() {
        let url = server.new_database();
        let mut admin = mysql::Conn::new(mysql::Opts::from_url(&url).unwrap()).unwrap();
        admin.query_drop("CREATE TABLE app (x int)").unwrap();
        assert_eq!(
            my::open(&url, None, true).err().unwrap().code,
            "catalog_foreign"
        );
        let tables: Vec<String> = admin.query("SHOW TABLES").unwrap();
        assert_eq!(tables, ["app"]);
        assert_eq!(
            my::open(&server.new_database(), None, false)
                .err()
                .unwrap()
                .code,
            "catalog_missing"
        );
    }
    for other in ["STATSCRAFT_MARIADB10_URL", "STATSCRAFT_DOLT_URL"] {
        if let Ok(url) = std::env::var(other) {
            let e = my::open(&url, None, true).err().unwrap();
            assert_eq!(e.code, "server_unsupported", "{other}: {e}");
        }
    }
}

// ---------------------------------------------------------------- Dolt

/// A Dolt sql-server; each fresh connection gets a new database whose catalog is committed on
/// `main`, with the session on a new branch `feature/recode`.
struct Dolt(String);

const DOLT: [&str; 2] = ["STATSCRAFT_DOLT_URL", "STATSCRAFT_DOLT2_URL"];

fn dolts() -> Vec<Dolt> {
    DOLT.iter().filter_map(|v| server(v).map(Dolt)).collect()
}

impl Dolt {
    fn url(&self, database: &str) -> String {
        format!("{}/{database}", &self.0[..self.0.rfind('/').unwrap()])
    }

    fn new_database(&self) -> String {
        let name = fresh_name();
        let mut admin = mysql::Conn::new(mysql::Opts::from_url(&self.0).unwrap()).unwrap();
        admin.query_drop(format!("CREATE DATABASE {name}")).unwrap();
        self.url(&name)
    }
}

impl Backend for Dolt {
    fn fresh(&self) -> Box<dyn Db> {
        let mut db = my::open_dolt(&self.new_database(), None, true).unwrap();
        db.batch("CALL DOLT_ADD('-A')").unwrap();
        db.batch("CALL DOLT_COMMIT('-m', 'catalog')").unwrap();
        db.batch("CALL DOLT_CHECKOUT('-b', 'feature/recode')")
            .unwrap();
        Box::new(db)
    }

    fn columns(&self, db: &mut dyn Db, table: &str) -> Vec<String> {
        column_names(
            db,
            "SELECT COLUMN_NAME FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?",
            table,
        )
    }

    fn reopen(&self, db: &mut dyn Db) -> Box<dyn Db> {
        let url = self.url(&db.namespace().unwrap());
        Box::new(my::open_dolt(&url, None, false).unwrap())
    }
}

/// Branch, HEAD, commit count and working-set status.
fn dolt_state(db: &mut dyn Db) -> serde_json::Value {
    serde_json::json!([
        query(db, "SELECT active_branch(), HASHOF('HEAD')", &[]),
        query(db, "SELECT count(*) FROM dolt_log", &[]),
        query(
            db,
            "SELECT table_name, staged, status FROM dolt_status ORDER BY table_name",
            &[]
        ),
    ])
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_round_trips_every_fixture() {
    for server in dolts() {
        round_trip_fixtures(&server);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_sav_zsav_1_0() {
    for server in dolts() {
        with_spec(|dir| sav_conformance(&server, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_in_place_bindings() {
    for server in dolts() {
        with_spec(|dir| in_place_conformance(&server, dir));
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_keeps_finite_values_and_refuses_negative_zero_and_the_others() {
    for server in dolts() {
        let mut db = server.fresh();
        let finite: Vec<f64> = FINITE
            .into_iter()
            .filter(|v| v.to_bits() != 1 << 63)
            .collect();
        assert_values_round_trip(db.as_mut(), &numbers(&finite), "finite");
        for bad in [-0.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let e = write_dataset(db.as_mut(), &numbers(&[1.0, bad]), "bad").unwrap_err();
            assert_eq!(
                (e.code, e.source_item.as_deref()),
                ("target_capability_exceeded", Some("x1"))
            );
        }
        let mut text = Dataset::new(2);
        let long = "õ".repeat(16_000);
        text.set_string_values("s", long.len(), vec!["a\0b".into(), long.clone()])
            .unwrap();
        assert_values_round_trip(db.as_mut(), &text, "text");
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_preflight_matches_the_measured_boundaries() {
    for server in dolts() {
        let mut db = server.fresh();
        let strings = |n: usize| {
            let mut d = Dataset::new(1);
            for i in 0..n {
                d.set_string_values(&format!("s{i}"), 1, vec!["a".into()])
                    .unwrap();
            }
            d
        };
        let tables = db.objects().unwrap().len();
        // 4,096 variables are 4,097 columns; 3,274 LONGTEXT columns are a 65,488-byte definition
        let mut wide = numbers(&vec![1.0; 4095]);
        wide.set_string_values("s", 2, vec!["ab".into()]).unwrap();
        write_dataset(db.as_mut(), &wide, "doubles").unwrap();
        // one text INSERT of 4,097 values; the prepared SELECT reads them back bit for bit
        let (back, _) = read_dataset(db.as_mut(), "doubles", false).unwrap();
        assert_eq!(differences(&wide, &back), Vec::<String>::new());
        write_dataset(db.as_mut(), &strings(3274), "texts").unwrap();
        let e = write_dataset(db.as_mut(), &numbers(&vec![1.0; 4097]), "doubles2").unwrap_err();
        assert_eq!(e.detail.unwrap()["limit"], "physical_columns");
        let e = write_dataset(db.as_mut(), &strings(3275), "texts2").unwrap_err();
        assert_eq!(e.detail.unwrap()["limit"], "structural_row");
        let e = write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(60)).unwrap_err();
        assert_eq!(e.detail.unwrap()["limit"], "identifier");
        write_dataset(db.as_mut(), &numbers(&[1.0]), &"n".repeat(59)).unwrap();
        assert_eq!(db.objects().unwrap().len(), tables + 3);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_import_never_commits_and_export_writes_nothing() {
    for server in dolts() {
        let mut db = server.fresh();
        let before = dolt_state(db.as_mut());
        write_dataset(db.as_mut(), &numbers(&[1.0, 2.0]), "d").unwrap();
        let after = dolt_state(db.as_mut());
        assert_eq!(after[0], before[0], "branch and HEAD");
        assert_eq!(after[1], before[1], "no commit");
        // commit the import, then make one item the model cannot hold
        db.exec(
            "UPDATE variable SET write_format_width = write_format_width + 1
             WHERE source_name = 'x0'",
            &[],
        )
        .unwrap();
        db.batch("CALL DOLT_ADD('-A')").unwrap();
        db.batch("CALL DOLT_COMMIT('-m', 'import')").unwrap();
        let clean = dolt_state(db.as_mut());
        let operations = query(db.as_mut(), "SELECT count(*) FROM operation", &[]);
        let e = read_dataset(db.as_mut(), "d", false).err().unwrap();
        assert_eq!(e.code, "export_fidelity_loss");
        let (_, losses) = read_dataset(db.as_mut(), "d", true).unwrap();
        assert_eq!(losses, ["x0: write_format"]);
        // a fresh read-only session sees the same
        let mut reader = server.reopen(db.as_mut());
        reader
            .batch("CALL DOLT_CHECKOUT('feature/recode')")
            .unwrap();
        read_dataset(reader.as_mut(), "d", true).unwrap();
        assert_eq!(dolt_state(db.as_mut()), clean);
        assert_eq!(
            query(db.as_mut(), "SELECT count(*) FROM operation", &[]),
            operations
        );
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_apply_needs_the_controlled_context() {
    for server in dolts() {
        let mut db = server.fresh();
        write_dataset(db.as_mut(), &numbers(&[1.0]), "d").unwrap();
        let plan = statscraft_openstatspec::validate_plan(&serde_json::json!({
            "contract": "openstatspec-transformation-plan-v0.2",
            "input_alias": "d",
            "operations": [{"op": "execute"}],
        }))
        .unwrap();
        let e = statscraft_openstatspec::apply::apply_plan(
            db.as_mut(),
            "d",
            &plan,
            &"0".repeat(64),
            "runner",
            None,
        )
        .unwrap_err();
        assert_eq!(e.code, "dolt_context_required");
    }
}

#[test]
#[ignore = "needs STATSCRAFT_DOLT_URL"]
fn dolt_and_mysql_refuse_each_other() {
    for server in dolts() {
        assert_eq!(
            my::open(&server.0, None, false).err().unwrap().code,
            "server_unsupported"
        );
    }
    for other in [MYSQL, MARIADB] {
        if let Ok(url) = std::env::var(other) {
            let e = my::open_dolt(&url, None, false).err().unwrap();
            assert_eq!(e.code, "server_unsupported", "{other}: {e}");
        }
    }
}

// ---------------------------------------------------------------- Bulk rows

/// 20,000 cases of 30 numbers and a string with every character that needs escaping: about
/// 12 MB of INSERT text on the MySQL protocol, so several batches, and one COPY on PostgreSQL.
fn many_cases() -> Dataset {
    let cases = 20_000;
    let mut d = Dataset::new(cases);
    for v in 0..30 {
        let values = (0..cases)
            .map(|c| match (c + v) % 7 {
                0 => None,
                1 => Some(5e-324),
                2 => Some(f64::MAX),
                3 => Some(-1.0 / 3.0),
                k => Some((c * 31 + k) as f64),
            })
            .collect();
        d.set_numeric_values(&format!("x{v}"), values).unwrap();
    }
    let strings = (0..cases).map(|c| format!("it's \\ a\0b õ {c}")).collect();
    d.set_string_values("s", 40, strings).unwrap();
    d
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL, STATSCRAFT_MARIADB_URL and STATSCRAFT_DOLT_URL"]
fn many_cases_round_trip_in_batches_on_every_server() {
    let d = many_cases();
    if let Some(pg) = Postgres::get() {
        // PostgreSQL text cannot hold U+0000
        let mut d = d.clone();
        let strings = (0..d.case_count())
            .map(|c| format!("it's \\ õ {c}"))
            .collect();
        d.set_string_values("s", 40, strings).unwrap();
        assert_values_round_trip(pg.fresh().as_mut(), &d, "many");
    }
    for server in mysql_family() {
        assert_values_round_trip(server.fresh().as_mut(), &d, "many");
    }
    for server in dolts() {
        assert_values_round_trip(server.fresh().as_mut(), &d, "many");
    }
}

// ---------------------------------------------------------------- TLS

/// STATSCRAFT_TLS_DIR holds `ca.crt`, the CA that signed the servers' certificate for the host
/// name `localhost` only, and `wrong-ca.crt`, another CA.
fn tls_files() -> Option<(String, String)> {
    let dir = std::path::PathBuf::from(server("STATSCRAFT_TLS_DIR")?);
    let file = |name: &str| dir.join(name).display().to_string();
    Some((file("ca.crt"), file("wrong-ca.crt")))
}

fn with_query(url: &str, params: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}{params}")
}

/// Opens `url` in each TLS mode with the right CA, a wrong one and none, and checks which are
/// accepted; `encrypted` tells whether an accepted session runs over TLS. `url` names the host
/// 127.0.0.1, which the certificate does not hold; the cases with `localhost` use the name it
/// does hold. `names` are the mode parameter, the CA parameter and the modes require,
/// verify-ca, verify-full and prefer.
fn check_tls<D: Db>(
    url: &str,
    open: impl Fn(&str) -> statscraft_openstatspec::db::Result<D>,
    encrypted: impl Fn(&mut D) -> bool,
    names: [&str; 6],
) {
    let Some((ca, wrong)) = tls_files() else {
        return;
    };
    let [mode, ca_file, require, verify_ca, verify_full, prefer] = names;
    let local = url.replace("@127.0.0.1:", "@localhost:");
    let ip = url.to_string();
    let cases = [
        // the right CA and the host name in the certificate
        (&local, verify_full, Some(&ca), true),
        (&local, require, Some(&ca), true),
        (&local, verify_ca, Some(&ca), true),
        // a CA file alone verifies everything
        (&local, "", Some(&ca), true),
        // the right CA, a host name the certificate does not hold
        (&ip, verify_full, Some(&ca), false),
        (&ip, require, Some(&ca), false),
        (&ip, "", Some(&ca), false),
        (&ip, verify_ca, Some(&ca), true),
        // a wrong CA, in every mode
        (&local, verify_full, Some(&wrong), false),
        (&local, require, Some(&wrong), false),
        (&local, verify_ca, Some(&wrong), false),
        (&ip, verify_ca, Some(&wrong), false),
        // the bundled roots do not hold the test CA
        (&local, verify_full, None, false),
        // a mode that falls back to plain TCP
        (&local, prefer, Some(&ca), false),
    ];
    for (base, m, file, accepted) in cases {
        let mut params = vec![];
        if !m.is_empty() {
            params.push(format!("{mode}={m}"));
        }
        if let Some(file) = file {
            params.push(format!("{ca_file}={file}"));
        }
        let target = with_query(base, &params.join("&"));
        let shown = target.replace(base.split('@').next().unwrap(), "...");
        match open(&target) {
            Ok(mut db) => {
                assert!(accepted, "{shown} was accepted");
                assert!(encrypted(&mut db), "{shown} is not encrypted");
            }
            Err(e) => {
                assert!(!accepted, "{shown}: {e}");
                assert_eq!(e.code, "database_unavailable", "{shown}: {e}");
            }
        }
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PG_URL and STATSCRAFT_TLS_DIR"]
fn postgres_tls_verifies_the_certificate_chain_and_host_name() {
    let Some(pg) = Postgres::get() else { return };
    let schema = fresh_name();
    check_tls(
        &pg.url(&schema),
        |url| postgres::open(url, None, true),
        |db| {
            let sql = "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()";
            query(db, sql, &[])[0][0] == 1
        },
        [
            "sslmode",
            "sslrootcert",
            "require",
            "verify-ca",
            "verify-full",
            "prefer",
        ],
    );
    // a dataset round trip over TLS
    if let Some((ca, _)) = tls_files() {
        let url = pg.url(&schema).replace("@127.0.0.1:", "@localhost:");
        let url = format!("{url}&sslmode=verify-full&sslrootcert={ca}");
        let mut db = postgres::open(&url, None, false).unwrap();
        assert_values_round_trip(&mut db, &numbers(&FINITE), "tls");
    }
}

#[test]
#[ignore = "needs STATSCRAFT_MARIADB_URL, STATSCRAFT_DOLT_URL and STATSCRAFT_TLS_DIR"]
fn mysql_family_tls_verifies_the_certificate_chain_and_host_name() {
    let names = [
        "ssl-mode",
        "ssl-ca",
        "REQUIRED",
        "VERIFY_CA",
        "VERIFY_IDENTITY",
        "PREFERRED",
    ];
    // Dolt shows an empty Ssl_cipher even over TLS; that it refuses a wrong CA shows the
    // handshake
    let cipher = |db: &mut my::MyDb| {
        let rows = query(db, "SHOW SESSION STATUS LIKE 'Ssl_cipher'", &[]);
        db.dialect().profile == "dolt" || rows[0][1].as_str().is_some_and(|c| !c.is_empty())
    };
    for server in mysql_family() {
        let url = server.new_database();
        check_tls(&url, |u| my::open(u, None, true), cipher, names);
        if let Some((ca, _)) = tls_files() {
            let url = url.replace("@127.0.0.1:", "@localhost:");
            let url = format!("{url}?ssl-mode=VERIFY_IDENTITY&ssl-ca={ca}");
            let mut db = my::open(&url, None, false).unwrap();
            assert_values_round_trip(&mut db, &numbers(&[1.5, 5e-324]), "tls");
        }
    }
    for server in dolts() {
        let url = server.new_database();
        check_tls(&url, |u| my::open_dolt(u, None, true), cipher, names);
    }
}
