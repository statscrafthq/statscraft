//! The published OpenStatSpec cases on any backend: the SAV/ZSAV 1.0 fixtures, the in-place
//! binding cases 0.1 and 0.2, and the round trip of every fixture in tests/fixtures. The
//! manifests are read in place from the specification checkout (see `spec_dir`).
//!
//! A case written for another profile whose expectations do not depend on that product
//! (rows, metadata, identity, audit) runs as an "adapted" case: the audit then names the
//! backend's profile, and `dataset_id` and `physical_table_schema` are compared with the
//! backend's own catalog. Cases about Dolt branch and HEAD context run only on Dolt; cases about
//! non-atomic DDL run only where DDL is not atomic, and the atomic create-target case only where
//! it is.
//!
//! On Dolt every apply runs in the controlled context of the cases: the backend starts each
//! fresh database on branch `feature/recode`, the runner commits the prepared dataset as the
//! provisioning commit and passes that branch and HEAD; afterwards the working set must hold
//! the changes, HEAD must be unchanged and no commit must have been made.

use super::{cases, differences, load, via_sav};
use serde_json::{Map, Value, json};
use statscraft_data::{Dataset, MissingValues};
use statscraft_openstatspec::apply::{DoltContext, apply_plan};
use statscraft_openstatspec::catalog::{input_schema, read_dataset, write_dataset};
use statscraft_openstatspec::db::{Db, Dialect, Limits, Result, Sql, uuid};
use statscraft_openstatspec::{canonical_json, compile_request};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub trait Backend {
    /// A connection to a new, empty namespace with the catalog installed.
    fn fresh(&self) -> Box<dyn Db>;
    /// The column names of `table`.
    fn columns(&self, db: &mut dyn Db, table: &str) -> Vec<String>;
    /// Temporary objects of the session, where the engine shows them.
    fn temporary(&self, _db: &mut dyn Db) -> i64 {
        0
    }
    /// A second session on the database of `db` (Dolt only).
    fn reopen(&self, _db: &mut dyn Db) -> Box<dyn Db> {
        unimplemented!("a second session is needed only on Dolt")
    }
}

fn to_json(v: &Sql) -> Value {
    match v {
        Sql::Null => Value::Null,
        Sql::Int(i) => json!(i),
        Sql::Real(r) => json!(r),
        Sql::Text(t) => json!(t),
    }
}

/// Rows of a query as JSON arrays.
pub fn query(db: &mut dyn Db, sql: &str, params: &[Sql]) -> Vec<Vec<Value>> {
    db.query(sql, params)
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
        .iter()
        .map(|r| r.iter().map(to_json).collect())
        .collect()
}

pub fn scalar(db: &mut dyn Db, sql: &str, params: &[Sql]) -> Value {
    query(db, sql, params)[0][0].clone()
}

fn text(s: &str) -> Sql {
    Sql::Text(s.to_string())
}

/// The first non-NULL of two cells: the numeric and the string column of one typed value.
fn either(a: &Value, b: &Value) -> Value {
    if a.is_null() { b.clone() } else { a.clone() }
}

/// JSON equality where 1 and 1.0 are the same number.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn expect(problems: &mut Vec<String>, what: &str, got: Value, want: &Value) {
    if !same(&got, want) {
        problems.push(format!("{what}: got {got}, expected {want}"));
    }
}

// ---------------------------------------------------------------- round trip of the fixtures

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sav" || e == "zsav"))
        .collect();
    files.sort();
    files
}

/// A scratch folder of its own: tests of one process run in parallel.
fn scratch(what: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-{what}-{}", uuid()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every fixture in tests/fixtures goes to the database and back, and then to a .sav file and
/// back. The dictionary and all values must be identical. The one lossy point the profile
/// defines is the compression inside a .sav file: the catalog keeps "sav" or "zsav" only, so a
/// sav dataset comes back as "bytecode" (what StatsCraft writes) and a zsav one as "zlib".
pub fn round_trip_fixtures(backend: &dyn Backend) {
    let dir = scratch("rt");
    let files = fixtures();
    assert_eq!(files.len(), 7, "fixtures missing");
    for path in &files {
        let what = path.file_name().unwrap().to_string_lossy().to_string();
        let original = Dataset::read_sav(path).unwrap();
        let mut db = backend.fresh();
        write_dataset(db.as_mut(), &original, "fixture").unwrap_or_else(|e| panic!("{what}: {e}"));
        let (back, losses) = read_dataset(db.as_mut(), "fixture", false).unwrap();
        assert!(losses.is_empty());
        assert_eq!(
            differences(&original, &back),
            Vec::<String>::new(),
            "{what}"
        );
        let zsav = what.ends_with(".zsav");
        assert_eq!(back.compression(), if zsav { "zlib" } else { "bytecode" });

        // The sav writer writes the default role "input" where the source gave none, with or
        // without the database in between, so the sav leg compares against a direct write.
        let direct = via_sav(&original, &dir, &format!("direct-{what}"));
        let via = via_sav(&back, &dir, &what);
        assert_eq!(
            differences(&direct, &via),
            Vec::<String>::new(),
            "{what} via sav"
        );
        assert_eq!(direct.compression(), via.compression());
        println!(
            "{what}: identical after {} and after {} plus sav",
            db.dialect().profile,
            db.dialect().profile
        );
    }
    std::fs::remove_dir(&dir).unwrap();
}

// ---------------------------------------------------------------- SAV/ZSAV profile 1.0

/// Compares the catalog with a fixture's `expected_catalog`.
fn catalog_problems(db: &mut dyn Db, expected: &Value) -> Vec<String> {
    let mut problems = vec![];
    for e in expected["value_labels"].as_array().into_iter().flatten() {
        let got: Vec<Value> = query(
            db,
            "SELECT l.code_kind, l.numeric_code, l.string_code, l.label
             FROM variable v JOIN variable_value_label_set s USING (variable_id)
             JOIN value_label l USING (value_label_set_id)
             WHERE v.source_name = ? AND l.ordinal = ?",
            &[
                text(e["variable"].as_str().unwrap()),
                Sql::Int(e["ordinal"].as_i64().unwrap()),
            ],
        )
        .into_iter()
        .map(|r| json!([r[0], either(&r[1], &r[2]), r[3]]))
        .collect();
        let want = json!([[e["kind"], e["value"], e["label"]]]);
        expect(&mut problems, "value label", json!(got), &want);
    }
    if let Some(want) = expected.get("dataset_attributes") {
        let mut rows = query(
            db,
            "SELECT attribute_name, array_ordinal, attribute_value FROM dataset_attribute",
            &[],
        );
        // byte order of the names, whatever the server's collation
        rows.sort_by(|a, b| (a[0].as_str(), a[1].as_i64()).cmp(&(b[0].as_str(), b[1].as_i64())));
        let mut got: Vec<Value> = vec![];
        for row in rows {
            match got.last_mut() {
                Some(last) if last["name"] == row[0] => {
                    last["values"].as_array_mut().unwrap().push(row[2].clone())
                }
                _ => got.push(json!({"name": row[0], "values": [row[2]]})),
            }
        }
        expect(&mut problems, "dataset attributes", json!(got), want);
    }
    for e in expected["variable_attributes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let values: Vec<Value> = query(
            db,
            "SELECT a.attribute_value FROM variable v JOIN variable_attribute a USING (variable_id)
             WHERE v.source_name = ? AND a.attribute_name = ? ORDER BY a.array_ordinal",
            &[
                text(e["variable"].as_str().unwrap()),
                text(e["name"].as_str().unwrap()),
            ],
        )
        .into_iter()
        .map(|r| r[0].clone())
        .collect();
        expect(
            &mut problems,
            "variable attribute",
            json!(values),
            &e["values"],
        );
    }
    if let Some(want) = expected.get("variable_sets") {
        let sets = query(
            db,
            "SELECT variable_set_id, source_ordinal, set_name FROM variable_set ORDER BY source_ordinal",
            &[],
        );
        let got: Vec<Value> = sets
            .into_iter()
            .map(|r| {
                let members = members(db, "variable_set_member", "variable_set_id", &r[0]);
                json!({"ordinal": r[1], "name": r[2], "members": members})
            })
            .collect();
        expect(&mut problems, "variable sets", json!(got), want);
    }
    if let Some(want) = expected.get("multiple_response_sets") {
        let sets = query(
            db,
            "SELECT multiple_response_set_id, source_ordinal, set_name, set_kind, set_label,
                    counted_value_kind, counted_numeric_value, counted_string_value,
                    category_label_behavior, label_source
             FROM multiple_response_set ORDER BY source_ordinal",
            &[],
        );
        let got: Vec<Value> = sets
            .into_iter()
            .map(|r| {
                let members = members(
                    db,
                    "multiple_response_member",
                    "multiple_response_set_id",
                    &r[0],
                );
                json!({"ordinal": r[1], "name": r[2], "kind": r[3], "label": r[4],
                       "counted_kind": r[5], "counted_value": either(&r[6], &r[7]),
                       "category_labels": r[8], "label_source": r[9], "members": members})
            })
            .collect();
        expect(&mut problems, "multiple response sets", json!(got), want);
    }
    if let Some(want) = expected.get("weight_variable") {
        let got = scalar(
            db,
            "SELECT v.source_name FROM dataset_weight_variable w JOIN variable v USING (variable_id)",
            &[],
        );
        expect(&mut problems, "weight variable", got, want);
    }
    problems
}

fn members(db: &mut dyn Db, table: &str, key: &str, id: &Value) -> Vec<Value> {
    query(
        db,
        &format!(
            "SELECT v.source_name FROM {table} m JOIN variable v USING (variable_id)
             WHERE m.{key} = ? ORDER BY m.source_ordinal"
        ),
        &[text(id.as_str().unwrap())],
    )
    .into_iter()
    .map(|r| r[0].clone())
    .collect()
}

fn limits(db: &mut dyn Db) -> Limits {
    db.limits().unwrap()
}

/// The fixture `preflight-too-wide.sav` is not published: its README asks the runner to
/// generate a source one variable wider than the profile allows. With the column limit L
/// (the ordinal included), L variables need L + 1 columns.
fn preflight_problems(backend: &dyn Backend) -> Vec<String> {
    let mut problems = vec![];
    let mut db = backend.fresh();
    let columns = limits(db.as_mut()).columns;
    let mut d = Dataset::new(1);
    for i in 0..columns {
        d.set_numeric_values(&format!("v{i}"), vec![Some(1.0)])
            .unwrap();
    }
    let tables = db.objects().unwrap().len();
    match write_dataset(db.as_mut(), &d, "too_wide") {
        Ok(_) => problems.push(format!(
            "a source of {columns} variables ({} columns) was accepted",
            columns + 1
        )),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &json!("target_capability_exceeded"),
        ),
    }
    let state = json!([
        scalar(db.as_mut(), "SELECT count(*) FROM dataset", &[]),
        db.objects().unwrap().len(),
        query(
            db.as_mut(),
            "SELECT operation_kind, status FROM operation",
            &[]
        ),
        scalar(
            db.as_mut(),
            "SELECT count(*) FROM fidelity_event WHERE dataset_id IS NULL
               AND direction = 'import' AND severity = 'error'
               AND event_code = 'target_capability_exceeded'",
            &[]
        ),
    ]);
    expect(
        &mut problems,
        "state after",
        state,
        &json!([0, tables, [["import", "failed"]], 1]),
    );
    problems
}

/// Runs `spss-sav-zsav-1.0.json`; every fixture must pass.
pub fn sav_conformance(backend: &dyn Backend, dir: &Path) {
    let manifest = load(dir, "spss-sav-zsav-1.0.json");
    let scratch = scratch("sav-conf");
    let fixtures = manifest["fixtures"].as_array().unwrap();
    let mut failed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut profile = "";
    for fixture in fixtures {
        let id = fixture["id"].as_str().unwrap();
        let problems = if id == "preflight-failure" {
            preflight_problems(backend)
        } else {
            let mut db = backend.fresh();
            profile = db.dialect().profile;
            sav_fixture_problems(db.as_mut(), dir, fixture, &scratch)
        };
        if !problems.is_empty() {
            failed.insert(id.to_string(), problems);
        }
    }
    std::fs::remove_dir(&scratch).unwrap();
    println!(
        "spss-sav-zsav-1.0.json on {profile}: {} of {} fixtures pass",
        fixtures.len() - failed.len(),
        fixtures.len()
    );
    assert!(failed.is_empty(), "failing fixtures: {failed:#?}");
}

fn sav_fixture_problems(
    db: &mut dyn Db,
    dir: &Path,
    fixture: &Value,
    scratch: &Path,
) -> Vec<String> {
    let source = fixture["source"].as_str().unwrap();
    let original = Dataset::read_sav(dir.join(source)).unwrap();
    let mut problems = vec![];
    if let Err(e) = write_dataset(db, &original, "fixture") {
        return vec![format!("import: {e}")];
    }
    problems.extend(catalog_problems(db, &fixture["expected_catalog"]));
    let back = match read_dataset(db, "fixture", false) {
        Ok((d, _)) => d,
        Err(e) => return vec![format!("export: {e}")],
    };
    problems.extend(differences(&original, &back));
    let file = Path::new(source).file_name().unwrap().to_string_lossy();
    let direct = via_sav(&original, scratch, &format!("direct-{file}"));
    let reread = via_sav(&back, scratch, &file);
    problems.extend(
        differences(&direct, &reread)
            .into_iter()
            .map(|p| format!("via sav: {p}")),
    );
    if direct.compression() != reread.compression() {
        problems.push("via sav: compression".into());
    }
    problems
}

// ---------------------------------------------------------------- in-place bindings

/// A dataset `name` with the numeric columns of `rows` (objects in case order).
fn make_dataset(db: &mut dyn Db, name: &str, rows: &[Value], missing: &Value) -> String {
    let mut d = Dataset::new(rows.len());
    for key in rows[0]
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| *k != "__case_ordinal")
    {
        let values = rows.iter().map(|r| r[key].as_f64()).collect();
        d.set_numeric_values(key, values).unwrap();
    }
    if let Some(rules) = missing.as_array() {
        let numbers = rules
            .iter()
            .map(|r| r["numeric_value"].as_f64().unwrap())
            .collect();
        d.set_missing_values("target", MissingValues::Numbers(numbers))
            .unwrap();
    }
    write_dataset(db, &d, name).unwrap()
}

/// The rows of table `table` with the keys of `like`.
fn table_rows(db: &mut dyn Db, table: &str, like: &[Value]) -> Value {
    let dialect: &Dialect = db.dialect();
    let keys: Vec<&String> = like[0].as_object().unwrap().keys().collect();
    let mut columns = vec![];
    for key in &keys {
        let physical = if *key == "__case_ordinal" {
            key.to_string()
        } else {
            scalar(
                db,
                "SELECT physical_name FROM variable WHERE source_name = ?",
                &[text(key)],
            )
            .as_str()
            .unwrap()
            .to_string()
        };
        columns.push(dialect.quote(&physical));
    }
    let sql = format!(
        "SELECT {} FROM {} ORDER BY {}",
        columns.join(", "),
        dialect.quote(table),
        dialect.quote("__case_ordinal")
    );
    json!(
        query(db, &sql, &[])
            .into_iter()
            .map(|row| {
                let object: Map<String, Value> =
                    keys.iter().map(|k| k.to_string()).zip(row).collect();
                Value::Object(object)
            })
            .collect::<Vec<_>>()
    )
}

/// What must not change: datasets, persistent tables, temporary objects.
fn snapshot(backend: &dyn Backend, db: &mut dyn Db) -> Value {
    let mut objects = db.objects().unwrap();
    objects.sort();
    json!([
        query(
            db,
            "SELECT dataset_id, physical_table_schema, physical_table_name FROM dataset",
            &[]
        ),
        objects,
        backend.temporary(db),
    ])
}

/// Label, value labels, format, level and missing rules of a variable, in the manifests' shape.
fn metadata(db: &mut dyn Db, variable: &str) -> Value {
    let v = query(
        db,
        "SELECT variable_id, variable_label, print_format_family, print_format_width,
                print_format_decimals, measurement_level, source_ordinal
         FROM variable WHERE source_name = ?",
        &[text(variable)],
    )
    .remove(0);
    let id = text(v[0].as_str().unwrap());
    let labels: Vec<Value> = query(
        db,
        "SELECT l.numeric_code, l.string_code, l.label
         FROM variable_value_label_set s JOIN value_label l USING (value_label_set_id)
         WHERE s.variable_id = ? ORDER BY l.ordinal",
        std::slice::from_ref(&id),
    )
    .into_iter()
    .map(|r| json!([either(&r[0], &r[1]), r[2]]))
    .collect();
    let missing: Vec<Value> = query(
        db,
        "SELECT ordinal, rule_kind, code_kind, numeric_value FROM missing_rule
         WHERE variable_id = ? ORDER BY ordinal",
        &[id],
    )
    .into_iter()
    .map(|r| json!({"ordinal": r[0], "rule_kind": r[1], "code_kind": r[2], "numeric_value": r[3]}))
    .collect();
    let format = match (&v[2], &v[3], &v[4]) {
        (Value::String(f), w, d) if !w.is_null() && !d.is_null() => json!(format!("{f}{w}.{d}")),
        _ => Value::Null,
    };
    json!({"variable_id": v[0], "ordinal": v[6], "variable_label": v[1], "value_labels": labels,
           "format": format, "measurement_level": v[5], "missing_values": missing})
}

fn compare_metadata(problems: &mut Vec<String>, got: &Value, want: &Value) {
    for (key, value) in want.as_object().into_iter().flatten() {
        expect(problems, key, got[key].clone(), value);
    }
}

const AUDIT_FIELDS: [&str; 17] = [
    "apply_id",
    "contract_id",
    "database_profile",
    "dataset_id",
    "physical_table_schema",
    "physical_table_name",
    "source_hash",
    "plan_hash",
    "canonical_plan_json",
    "actor",
    "status",
    "dolt_branch",
    "dolt_head_before",
    "dolt_head_after",
    "operation_count",
    "started_at",
    "completed_at",
];

/// The audit row against `expected_audit`. The dataset id and schema are the backend's own;
/// Dolt fields hold the context on Dolt and are NULL on every other profile.
fn audit_problems(
    db: &mut dyn Db,
    case: &Value,
    dataset_id: &str,
    dolt: Option<&DoltContext>,
) -> Vec<String> {
    let mut problems = vec![];
    let profile = db.dialect().profile;
    let rows = query(
        db,
        &format!(
            "SELECT {} FROM transformation_apply",
            AUDIT_FIELDS.join(", ")
        ),
        &[],
    );
    if rows.len() != 1 {
        return vec![format!("{} audit rows", rows.len())];
    }
    let audit: Map<String, Value> = AUDIT_FIELDS
        .iter()
        .map(|f| f.to_string())
        .zip(rows[0].clone())
        .collect();
    let schema = scalar(
        db,
        "SELECT physical_table_schema FROM dataset WHERE dataset_id = ?",
        &[text(dataset_id)],
    );
    for field in case["required_audit_fields"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let field = field.as_str().unwrap();
        let nullable = field.starts_with("dolt_") || field == "physical_table_schema";
        match audit.get(field) {
            None => problems.push(format!("audit field {field} missing")),
            Some(Value::Null) if !nullable => problems.push(format!("audit field {field} is NULL")),
            _ => {}
        }
    }
    let dolt_fields = match dolt {
        Some(c) => json!({"dolt_branch": c.branch, "dolt_head_before": c.head,
                          "dolt_head_after": c.head}),
        None => json!({"dolt_branch": null, "dolt_head_before": null, "dolt_head_after": null}),
    };
    for (key, want) in case["expected_audit"].as_object().into_iter().flatten() {
        let want = match key.as_str() {
            k if k.starts_with("dolt_") => dolt_fields[k].clone(),
            "database_profile" => json!(profile),
            "dataset_id" => json!(dataset_id),
            "physical_table_schema" => schema.clone(),
            _ => want.clone(),
        };
        expect(
            &mut problems,
            &format!("audit {key}"),
            audit[key].clone(),
            &want,
        );
    }
    for key in ["dolt_branch", "dolt_head_before", "dolt_head_after"] {
        expect(
            &mut problems,
            &format!("audit {key}"),
            audit[key].clone(),
            &dolt_fields[key],
        );
    }
    expect(
        &mut problems,
        "audit database_profile",
        audit["database_profile"].clone(),
        &json!(profile),
    );
    problems
}

fn case_by_id<'m>(manifest: &'m Value, id: &str) -> &'m Value {
    cases(manifest)
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("case {id}"))
}

/// Fails the first statement that starts with `prefix`: a fault injected after the earlier
/// statements of the same transaction ran.
pub struct FailAt<'a> {
    pub inner: &'a mut dyn Db,
    pub prefix: &'static str,
}

impl Db for FailAt<'_> {
    fn dialect(&self) -> &'static Dialect {
        self.inner.dialect()
    }
    fn exec(&mut self, sql: &str, params: &[Sql]) -> Result<u64> {
        if sql.starts_with(self.prefix) {
            return Err(statscraft_openstatspec::db::DbError::new(
                "injected_fault",
                "rollback probe",
            ));
        }
        self.inner.exec(sql, params)
    }
    fn query(&mut self, sql: &str, params: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        self.inner.query(sql, params)
    }
    fn batch(&mut self, sql: &str) -> Result<()> {
        self.inner.batch(sql)
    }
    fn objects(&mut self) -> Result<Vec<String>> {
        self.inner.objects()
    }
    fn namespace(&self) -> Option<String> {
        self.inner.namespace()
    }
    fn limits(&mut self) -> Result<Limits> {
        self.inner.limits()
    }
    fn write_gate(&self) -> Result<()> {
        self.inner.write_gate()
    }
    fn committed_head(&mut self, branch: &str) -> Result<String> {
        self.inner.committed_head(branch)
    }
    fn insert_rows(&mut self, table: &str, rows: &mut dyn Iterator<Item = Vec<Sql>>) -> Result<()> {
        self.inner.insert_rows(table, rows)
    }
    fn query_each(
        &mut self,
        sql: &str,
        params: &[Sql],
        f: &mut dyn FnMut(Vec<Sql>) -> Result<()>,
    ) -> Result<()> {
        self.inner.query_each(sql, params, f)
    }
}

/// On Dolt: commits the working set as the provisioning commit and returns the context an
/// apply expects. Elsewhere nothing.
pub fn provision(db: &mut dyn Db) -> Option<DoltContext> {
    if db.dialect().profile != "dolt" {
        return None;
    }
    db.batch("CALL DOLT_ADD('-A')").unwrap();
    db.batch("CALL DOLT_COMMIT('-m', 'provisioning')").unwrap();
    let row = query(db, "SELECT active_branch(), HASHOF('HEAD')", &[]).remove(0);
    Some(DoltContext {
        branch: row[0].as_str().unwrap().to_string(),
        head: row[1].as_str().unwrap().to_string(),
    })
}

fn dolt_state(db: &mut dyn Db) -> Value {
    json!([
        query(db, "SELECT active_branch(), HASHOF('HEAD')", &[]),
        query(db, "SELECT count(*) FROM dolt_log", &[]),
        query(
            db,
            "SELECT table_name, staged, status FROM dolt_status ORDER BY table_name",
            &[]
        ),
    ])
}

/// After a successful apply on Dolt: same branch and HEAD, no commit, a dirty working set.
fn dolt_after(db: &mut dyn Db, before: &Value) -> Vec<String> {
    let mut problems = vec![];
    let after = dolt_state(db);
    expect(
        &mut problems,
        "Dolt branch and HEAD",
        after[0].clone(),
        &before[0],
    );
    expect(&mut problems, "Dolt commits", after[1].clone(), &before[1]);
    if after[2].as_array().is_none_or(Vec::is_empty) {
        problems.push("the working set holds no changes".into());
    }
    problems
}

/// Runs one success case of the 0.2 manifest (native or adapted).
fn binding_02_problems(backend: &dyn Backend, dir: &Path, case: &Value) -> Vec<String> {
    let frontend = load(dir, "spss-syntax-frontend-0.2.json");
    let plans = load(dir, "transformation-plan-0.2.json");
    let before = case.get("before").cloned().unwrap_or(json!({}));
    let after = case.get("after").cloned().unwrap_or(json!({}));
    let before_rows = before
        .get("rows")
        .or(case.get("before_rows"))
        .unwrap()
        .as_array()
        .unwrap();
    let after_rows = after
        .get("rows")
        .or(case.get("after_rows"))
        .unwrap()
        .as_array()
        .unwrap();
    let table = before
        .get("physical_table_name")
        .or(case.get("physical_table_name"))
        .and_then(Value::as_str)
        .unwrap();
    let name = table.strip_prefix("data_").unwrap();
    let mut problems = vec![];

    let mut db = backend.fresh();
    let db = db.as_mut();
    let dataset_id = make_dataset(
        db,
        name,
        before_rows,
        &before["target_metadata"]["missing_values"],
    );
    // the frontend request of the case, with the input schema taken from our catalog
    let mut request =
        case_by_id(&frontend, case["applied_frontend_case"].as_str().unwrap())["request"].clone();
    request["input_schema"] = input_schema(db, name, true).unwrap();
    let out = compile_request(&request);
    let compiled = match out.result {
        Ok(c) => c,
        Err(d) => return vec![format!("frontend: {} {}", d.code, d.message)],
    };
    let expected_plan = &case_by_id(&plans, case["applied_plan_case"].as_str().unwrap())["plan"];
    if canonical_json(expected_plan) != Some(compiled.canonical.clone()) {
        problems.push(format!("plan differs: {}", compiled.canonical));
    }
    let source_hash = out.source_hash.unwrap();
    let dolt = provision(db);

    if let Some(probe) = case.get("rollback_probe") {
        // fail at the audit insert, after the column, data and catalog changes
        let rows_before = table_rows(db, table, before_rows);
        let mut failing = FailAt {
            inner: &mut *db,
            prefix: "INSERT INTO transformation_apply",
        };
        let applied = apply_plan(
            &mut failing,
            name,
            &compiled.plan,
            &source_hash,
            "probe",
            None,
        );
        if applied.is_ok() {
            problems.push("rollback probe: the apply succeeded".into());
        }
        let column = backend.columns(db, table).iter().any(|c| c == "target");
        let variable = scalar(
            db,
            "SELECT count(*) FROM variable WHERE source_name = 'target'",
            &[],
        );
        let audit = scalar(db, "SELECT count(*) FROM transformation_apply", &[]);
        let changed = table_rows(db, table, before_rows) != rows_before;
        expect(
            &mut problems,
            "rollback probe",
            json!({"physical_column_present": column, "catalog_variable_present": variable != 0,
                   "row_values_changed": changed, "audit_row_present": audit != 0}),
            &json!({"physical_column_present": probe["physical_column_present"],
                    "catalog_variable_present": probe["catalog_variable_present"],
                    "row_values_changed": probe["row_values_changed"],
                    "audit_row_present": probe["audit_row_present"]}),
        );
    }

    let target_before = (before.get("target_identity").is_some()).then(|| metadata(db, "target"));
    let snap = snapshot(backend, db);
    let versioned = dolt.is_some().then(|| dolt_state(db));
    let actor = case["actor"].as_str().unwrap();
    if let Err(e) = apply_plan(db, name, &compiled.plan, &source_hash, actor, dolt.as_ref()) {
        problems.push(format!("apply: {e}"));
        return problems;
    }
    if let Some(before) = &versioned {
        problems.extend(dolt_after(db, before));
    }
    expect(
        &mut problems,
        "dataset, tables and temporary objects",
        snapshot(backend, db),
        &snap,
    );
    expect(
        &mut problems,
        "rows",
        table_rows(db, table, after_rows),
        &json!(after_rows),
    );
    problems.extend(audit_problems(db, case, &dataset_id, dolt.as_ref()));
    if let Some(want) = after.get("target_metadata") {
        compare_metadata(&mut problems, &metadata(db, "target"), want);
    }
    if let Some(target) = after.get("target") {
        let got = metadata(db, "target");
        compare_metadata(&mut problems, &got, &target["initial_metadata"]);
        expect(
            &mut problems,
            "target ordinal",
            got["ordinal"].clone(),
            &target["ordinal"],
        );
        let count = scalar(db, "SELECT count(*) FROM variable", &[]);
        expect(
            &mut problems,
            "variable count",
            count,
            &after["variable_count"],
        );
    }
    if let Some(before_target) = target_before {
        let got = metadata(db, "target");
        expect(
            &mut problems,
            "target identity",
            got["variable_id"].clone(),
            &before_target["variable_id"],
        );
        expect(
            &mut problems,
            "target ordinal",
            got["ordinal"].clone(),
            &after["target_identity"]["ordinal"],
        );
    }
    problems
}

/// The 0.1 success case: rows are not given, so a synthetic `score` column 1..4 is used and
/// the recode rules decide the result.
fn binding_01_problems(backend: &dyn Backend, case: &Value) -> Vec<String> {
    let mut problems = vec![];
    let mut db = backend.fresh();
    let db = db.as_mut();
    let rows: Vec<Value> = (1..=4)
        .map(|i| json!({"__case_ordinal": i, "score": i}))
        .collect();
    let dataset_id = make_dataset(db, "survey", &rows, &Value::Null);
    let request = json!({
        "contract": "openstatspec-spss-syntax-frontend-v0.1",
        "input_alias": "input",
        "input_schema": input_schema(db, "survey", false).unwrap(),
        "source_text": case["source_text"],
    });
    let out = compile_request(&request);
    let compiled = match out.result {
        Ok(c) => c,
        Err(d) => return vec![format!("frontend: {} {}", d.code, d.message)],
    };
    let dolt = provision(db);
    let snap = snapshot(backend, db);
    let versioned = dolt.is_some().then(|| dolt_state(db));
    let source_hash = out.source_hash.unwrap();
    if let Err(e) = apply_plan(
        db,
        "survey",
        &compiled.plan,
        &source_hash,
        "runner",
        dolt.as_ref(),
    ) {
        return vec![format!("apply: {e}")];
    }
    if let Some(before) = &versioned {
        problems.extend(dolt_after(db, before));
    }
    expect(
        &mut problems,
        "dataset, tables and temporary objects",
        snapshot(backend, db),
        &snap,
    );
    let after: Vec<Value> = [0, 0, 1, 4]
        .iter()
        .enumerate()
        .map(|(i, v)| json!({"__case_ordinal": i + 1, "score": v}))
        .collect();
    expect(
        &mut problems,
        "rows",
        table_rows(db, "data_survey", &after),
        &json!(after),
    );
    let got = metadata(db, "score");
    expect(
        &mut problems,
        "variable label",
        got["variable_label"].clone(),
        &case["after"]["variable_label"],
    );
    expect(
        &mut problems,
        "value labels",
        got["value_labels"].clone(),
        &case["after"]["value_labels"],
    );
    let audit = json!({"required_audit_fields": case["required_audit_fields"],
        "expected_audit": {"contract_id": "openstatspec-in-place-transformation-v0.1",
                           "plan_hash": compiled.plan_hash, "status": "succeeded",
                           "physical_table_name": "data_survey", "actor": "runner"}});
    problems.extend(audit_problems(db, &audit, &dataset_id, dolt.as_ref()));
    problems
}

/// Every apply needs an actor, on every profile.
fn empty_actor_problems(backend: &dyn Backend, dir: &Path) -> Vec<String> {
    let plans = load(dir, "transformation-plan-0.2.json");
    let plan = statscraft_openstatspec::validate_plan(
        &case_by_id(&plans, "sqlite-create-target-source-copy")["plan"],
    )
    .unwrap();
    let mut db = backend.fresh();
    let db = db.as_mut();
    let rows = vec![json!({"__case_ordinal": 1, "source_a": 2})];
    make_dataset(db, "atomic_create", &rows, &Value::Null);
    let snap = snapshot(backend, db);
    let hash = "0".repeat(64);
    let mut problems = vec![];
    match apply_plan(db, "atomic_create", &plan, &hash, "", None) {
        Ok(_) => problems.push("an empty actor was accepted".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &json!("actor_required"),
        ),
    }
    expect(&mut problems, "state", snapshot(backend, db), &snap);
    expect(
        &mut problems,
        "columns",
        json!(backend.columns(db, "data_atomic_create").len()),
        &json!(2),
    );
    problems
}

/// A create-target plan on a profile whose DDL is not atomic fails with
/// `schema_change_not_atomic` before any change. The 0.1 case gives syntax; the 0.2 cases give
/// only the mode, so the 0.2 create-target plan of the SQLite case is used.
fn create_fails_problems(backend: &dyn Backend, dir: &Path, case: &Value) -> Vec<String> {
    let mut db = backend.fresh();
    let db = db.as_mut();
    let (plan, name, column) = match case["source_text"].as_str() {
        Some(source) => {
            let rows: Vec<Value> = (1..=3)
                .map(|i| json!({"__case_ordinal": i, "score": i}))
                .collect();
            make_dataset(db, "survey", &rows, &Value::Null);
            let request = json!({
                "contract": "openstatspec-spss-syntax-frontend-v0.1",
                "input_alias": "input",
                "input_schema": input_schema(db, "survey", false).unwrap(),
                "source_text": source,
            });
            let plan = compile_request(&request).result.unwrap().plan;
            (plan, "survey", "score_band")
        }
        None => {
            let plans = load(dir, "transformation-plan-0.2.json");
            let plan = statscraft_openstatspec::validate_plan(
                &case_by_id(&plans, "sqlite-create-target-source-copy")["plan"],
            )
            .unwrap();
            let rows = vec![json!({"__case_ordinal": 1, "source_a": 2})];
            make_dataset(db, "atomic_create", &rows, &Value::Null);
            (plan, "atomic_create", "target")
        }
    };
    let dolt = provision(db);
    let snap = snapshot(backend, db);
    let versioned = dolt.is_some().then(|| dolt_state(db));
    let catalog = query(db, "SELECT variable_id, physical_name FROM variable", &[]);
    let mut problems = vec![];
    let actor = case["actor"].as_str().unwrap_or("runner");
    match apply_plan(db, name, &plan, &"0".repeat(64), actor, dolt.as_ref()) {
        Ok(_) => problems.push("a create-target plan was applied".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &case["expected_error"],
        ),
    }
    expect(&mut problems, "state", snapshot(backend, db), &snap);
    expect(
        &mut problems,
        "catalog variables",
        json!(query(
            db,
            "SELECT variable_id, physical_name FROM variable",
            &[]
        )),
        &json!(catalog),
    );
    let table = format!("data_{name}");
    if backend.columns(db, &table).iter().any(|c| c == column) {
        problems.push(format!("column {column} was created"));
    }
    expect(
        &mut problems,
        "audit rows",
        scalar(db, "SELECT count(*) FROM transformation_apply", &[]),
        &json!(0),
    );
    if let Some(before) = versioned {
        expect(&mut problems, "Dolt state", dolt_state(db), &before);
    }
    problems
}

/// A Dolt dataset `survey` with `score` 1..4 and its label, committed on `feature/recode`.
fn dolt_survey(backend: &dyn Backend) -> (Box<dyn Db>, DoltContext, statscraft_openstatspec::Plan) {
    let mut db = backend.fresh();
    let rows: Vec<Value> = (1..=4)
        .map(|i| json!({"__case_ordinal": i, "score": i}))
        .collect();
    make_dataset(db.as_mut(), "survey", &rows, &Value::Null);
    let request = json!({
        "contract": "openstatspec-spss-syntax-frontend-v0.2",
        "input_alias": "input",
        "input_schema": input_schema(db.as_mut(), "survey", true).unwrap(),
        "source_text": "COMPUTE score = 9.\nVARIABLE LABELS score 'After'.",
    });
    let plan = compile_request(&request).result.unwrap().plan;
    let dolt = provision(db.as_mut()).unwrap();
    (db, dolt, plan)
}

/// What a failed Dolt apply must leave as it was: rows, labels, audit, branch, HEAD, status.
/// On another branch the dataset may not exist.
fn dolt_unchanged(db: &mut dyn Db) -> Value {
    let rows = vec![json!({"__case_ordinal": 1, "score": 1})];
    let present = db.objects().unwrap().iter().any(|o| o == "data_survey");
    json!([
        present.then(|| table_rows(db, "data_survey", &rows)),
        query(
            db,
            "SELECT source_name, variable_label FROM variable ORDER BY source_name",
            &[]
        ),
        scalar(db, "SELECT count(*) FROM transformation_apply", &[]),
        dolt_state(db),
    ])
}

/// The 0.1 rejections: another branch, another HEAD, or a dirty working set fail before any
/// change with their stable codes.
fn dolt_reject_problems(backend: &dyn Backend, case: &Value) -> Vec<String> {
    let (mut db, mut expected, plan) = dolt_survey(backend);
    let db = db.as_mut();
    let observed = &case["observed_context"];
    if observed["branch"] != case["expected_context"]["branch"] {
        db.batch("CALL DOLT_CHECKOUT('main')").unwrap();
    }
    if observed["head"] != case["expected_context"]["head"] {
        // a real commit that is not the one the session is at
        expected.head = query(db, "SELECT HASHOF('HEAD~1')", &[])[0][0]
            .as_str()
            .unwrap()
            .to_string();
    }
    if observed["working_set_clean"] == false {
        db.exec(
            "UPDATE variable SET variable_label = 'Unrelated' WHERE source_name = 'score'",
            &[],
        )
        .unwrap();
    }
    let before = dolt_unchanged(db);
    let mut problems = vec![];
    match apply_plan(
        db,
        "survey",
        &plan,
        &"0".repeat(64),
        "runner",
        Some(&expected),
    ) {
        Ok(_) => problems.push("the apply succeeded".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &case["expected_error"],
        ),
    }
    expect(&mut problems, "state", dolt_unchanged(db), &before);
    problems
}

/// Commits on the expected branch from another session when the apply looks at the branch
/// head before completing: after the data and metadata changes, before the audit row.
struct ConcurrentCommit<'a> {
    inner: &'a mut dyn Db,
    other: Box<dyn Db>,
}

impl Db for ConcurrentCommit<'_> {
    fn dialect(&self) -> &'static Dialect {
        self.inner.dialect()
    }
    fn exec(&mut self, sql: &str, params: &[Sql]) -> Result<u64> {
        self.inner.exec(sql, params)
    }
    fn query(&mut self, sql: &str, params: &[Sql]) -> Result<Vec<Vec<Sql>>> {
        self.inner.query(sql, params)
    }
    fn batch(&mut self, sql: &str) -> Result<()> {
        self.inner.batch(sql)
    }
    fn objects(&mut self) -> Result<Vec<String>> {
        self.inner.objects()
    }
    fn namespace(&self) -> Option<String> {
        self.inner.namespace()
    }
    fn limits(&mut self) -> Result<Limits> {
        self.inner.limits()
    }
    fn committed_head(&mut self, branch: &str) -> Result<String> {
        self.other
            .batch(&format!("CALL DOLT_CHECKOUT('{branch}')"))
            .unwrap();
        self.other
            .batch("CALL DOLT_COMMIT('--allow-empty', '-m', 'concurrent')")
            .unwrap();
        self.inner.committed_head(branch)
    }
}

/// The 0.2 case: HEAD moves between the changes and the audit; the apply fails with
/// `dolt_context_changed` and its changes roll back.
fn dolt_changed_problems(backend: &dyn Backend, case: &Value) -> Vec<String> {
    let (mut db, expected, plan) = dolt_survey(backend);
    let db = db.as_mut();
    let rows = vec![json!({"__case_ordinal": 1, "score": 1})];
    let before = json!([
        table_rows(db, "data_survey", &rows),
        metadata(db, "score")["variable_label"],
        scalar(db, "SELECT count(*) FROM transformation_apply", &[]),
    ]);
    let other = backend.reopen(db);
    let mut concurrent = ConcurrentCommit {
        inner: &mut *db,
        other,
    };
    let mut problems = vec![];
    match apply_plan(
        &mut concurrent,
        "survey",
        &plan,
        &"0".repeat(64),
        "runner",
        Some(&expected),
    ) {
        Ok(_) => problems.push("the apply succeeded".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &case["expected_error"],
        ),
    }
    let after = json!([
        table_rows(db, "data_survey", &rows),
        metadata(db, "score")["variable_label"],
        scalar(db, "SELECT count(*) FROM transformation_apply", &[]),
    ]);
    expect(&mut problems, "rows, label and audit", after, &before);
    problems
}

/// How a case runs on a backend, or why it does not.
enum Run {
    Binding01,
    Binding02,
    EmptyActor,
    CreateFails,
    DoltReject,
    DoltChanged,
    NotApplicable,
}

fn plan_for(case: &Value, profile: &str, atomic: bool) -> Run {
    let id = case["id"].as_str().unwrap();
    let dolt_only = case["database_profile"] == "dolt"
        && (case.get("expected_context").is_some() || id.starts_with("dolt-recode"));
    if dolt_only && profile != "dolt" {
        return Run::NotApplicable;
    }
    match id {
        "mysql-recode-and-labels-preserve-dataset-and-table-identity"
        | "dolt-recode-and-labels-preserve-controlled-context" => Run::Binding01,
        "dolt-context-changed-after-mutation-rolls-back" => Run::DoltChanged,
        _ if case.get("observed_context").is_some() => Run::DoltReject,
        "dolt-empty-actor-fails-before-mutation" => Run::EmptyActor,
        _ if case["expected_error"] == "schema_change_not_atomic" => {
            if atomic {
                Run::NotApplicable
            } else {
                Run::CreateFails
            }
        }
        "sqlite-create-target-atomic-success" if !atomic => Run::NotApplicable,
        _ if case.get("applied_frontend_case").is_some() && case["expected_error"].is_null() => {
            Run::Binding02
        }
        _ => Run::NotApplicable,
    }
}

/// Runs both in-place manifests; every applicable case must pass.
pub fn in_place_conformance(backend: &dyn Backend, dir: &Path) {
    let (profile, atomic) = {
        let db = backend.fresh();
        (db.dialect().profile, db.dialect().atomic_schema_change)
    };
    let mut failures = vec![];
    for name in [
        "in-place-transformation-0.1.json",
        "in-place-transformation-0.2.json",
    ] {
        let manifest = load(dir, name);
        let (mut native, mut adapted, mut passed_native, mut passed_adapted) = (0, 0, 0, 0);
        let mut not_applicable = vec![];
        for case in cases(&manifest) {
            let id = case["id"].as_str().unwrap();
            let problems = match plan_for(case, profile, atomic) {
                Run::Binding01 => binding_01_problems(backend, case),
                Run::Binding02 => binding_02_problems(backend, dir, case),
                Run::EmptyActor => empty_actor_problems(backend, dir),
                Run::CreateFails => create_fails_problems(backend, dir, case),
                Run::DoltReject => dolt_reject_problems(backend, case),
                Run::DoltChanged => dolt_changed_problems(backend, case),
                Run::NotApplicable => {
                    not_applicable.push(id);
                    continue;
                }
            };
            let ok = problems.is_empty();
            if case["database_profile"] == profile {
                native += 1;
                passed_native += ok as usize;
            } else {
                adapted += 1;
                passed_adapted += ok as usize;
            }
            if !ok {
                failures.push(format!("{name} {id}: {problems:#?}"));
            }
        }
        println!(
            "{name} on {profile}: native cases {passed_native} of {native} pass; adapted cases \
             {passed_adapted} of {adapted} pass; {} of {} cases not applicable: {not_applicable:?}",
            not_applicable.len(),
            cases(&manifest).len()
        );
    }
    assert!(failures.is_empty(), "failing cases: {failures:#?}");
}
