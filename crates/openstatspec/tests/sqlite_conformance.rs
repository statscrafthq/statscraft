//! Runs the cases of `spss-sav-zsav-1.0.json` and `in-place-transformation-0.1.json` /
//! `0.2.json` that apply to SQLite, read in place from the specification checkout (see
//! `common::spec_dir`); without it the tests skip with a message.
//!
//! In-place cases written for MySQL or Dolt whose expectations do not depend on that product
//! (rows, metadata, identity, audit) also run on SQLite as "adapted" cases: the audit then
//! names the sqlite profile and its Dolt fields are NULL. Cases about Dolt branch/HEAD
//! context or MySQL-family non-atomic DDL do not apply to SQLite.

mod common;

use common::{cases, differences, load, via_sav, with_spec};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use statscraft_data::{Dataset, MissingValues};
use statscraft_openstatspec::apply::apply_plan;
use statscraft_openstatspec::sqlite::{SQLITE, input_schema, prepare, read_dataset, write_dataset};
use statscraft_openstatspec::{canonical_json, compile_request};
use std::collections::BTreeMap;
use std::path::Path;

fn memory() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    prepare(&mut conn, true).unwrap();
    conn
}

/// Rows of a query as JSON arrays.
fn query(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Vec<Vec<Value>> {
    let mut statement = conn.prepare(sql).unwrap();
    let n = statement.column_count();
    statement
        .query_map(p, |r| {
            (0..n)
                .map(|i| {
                    Ok(match r.get_ref(i)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(v) => json!(v),
                        ValueRef::Real(v) => json!(v),
                        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        ValueRef::Blob(_) => json!("<blob>"),
                    })
                })
                .collect()
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn scalar(conn: &Connection, sql: &str) -> Value {
    query(conn, sql, [])[0][0].clone()
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

// ---------------------------------------------------------------- SAV/ZSAV profile 1.0

/// Compares the catalog with a fixture's `expected_catalog`.
fn catalog_problems(conn: &Connection, expected: &Value) -> Vec<String> {
    let mut problems = vec![];
    for e in expected["value_labels"].as_array().into_iter().flatten() {
        let got = query(
            conn,
            "SELECT l.code_kind, coalesce(l.numeric_code, l.string_code), l.label
             FROM variable v JOIN variable_value_label_set s USING (variable_id)
             JOIN value_label l USING (value_label_set_id)
             WHERE v.source_name = ?1 AND l.ordinal = ?2",
            params![e["variable"].as_str(), e["ordinal"].as_i64()],
        );
        let want = json!([[e["kind"], e["value"], e["label"]]]);
        expect(&mut problems, "value label", json!(got), &want);
    }
    if let Some(want) = expected.get("dataset_attributes") {
        let mut got: Vec<Value> = vec![];
        for row in query(
            conn,
            "SELECT attribute_name, attribute_value FROM dataset_attribute
             ORDER BY attribute_name, array_ordinal",
            [],
        ) {
            match got.last_mut() {
                Some(last) if last["name"] == row[0] => {
                    last["values"].as_array_mut().unwrap().push(row[1].clone())
                }
                _ => got.push(json!({"name": row[0], "values": [row[1]]})),
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
            conn,
            "SELECT a.attribute_value FROM variable v JOIN variable_attribute a USING (variable_id)
             WHERE v.source_name = ?1 AND a.attribute_name = ?2 ORDER BY a.array_ordinal",
            params![e["variable"].as_str(), e["name"].as_str()],
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
        let got: Vec<Value> = query(
            conn,
            "SELECT variable_set_id, source_ordinal, set_name FROM variable_set ORDER BY source_ordinal",
            [],
        )
        .into_iter()
        .map(|r| {
            let members = members(conn, "variable_set_member", "variable_set_id", &r[0]);
            json!({"ordinal": r[1], "name": r[2], "members": members})
        })
        .collect();
        expect(&mut problems, "variable sets", json!(got), want);
    }
    if let Some(want) = expected.get("multiple_response_sets") {
        let got: Vec<Value> = query(
            conn,
            "SELECT multiple_response_set_id, source_ordinal, set_name, set_kind, set_label,
                    counted_value_kind, coalesce(counted_numeric_value, counted_string_value),
                    category_label_behavior, label_source
             FROM multiple_response_set ORDER BY source_ordinal",
            [],
        )
        .into_iter()
        .map(|r| {
            let members = members(
                conn,
                "multiple_response_member",
                "multiple_response_set_id",
                &r[0],
            );
            json!({"ordinal": r[1], "name": r[2], "kind": r[3], "label": r[4],
                   "counted_kind": r[5], "counted_value": r[6], "category_labels": r[7],
                   "label_source": r[8], "members": members})
        })
        .collect();
        expect(&mut problems, "multiple response sets", json!(got), want);
    }
    if let Some(want) = expected.get("weight_variable") {
        let got = scalar(
            conn,
            "SELECT v.source_name FROM dataset_weight_variable w JOIN variable v USING (variable_id)",
        );
        expect(&mut problems, "weight variable", got, want);
    }
    problems
}

fn members(conn: &Connection, table: &str, key: &str, id: &Value) -> Vec<Value> {
    query(
        conn,
        &format!(
            "SELECT v.source_name FROM {table} m JOIN variable v USING (variable_id)
             WHERE m.{key} = ?1 ORDER BY m.source_ordinal"
        ),
        [id.as_str()],
    )
    .into_iter()
    .map(|r| r[0].clone())
    .collect()
}

/// The fixture `preflight-too-wide.sav` is not published: its README asks the runner to
/// generate a source one variable wider than the profile allows.
fn preflight_problems() -> Vec<String> {
    let mut problems = vec![];
    let mut d = Dataset::new(1);
    for i in 0..SQLITE.max_columns {
        d.set_numeric_values(&format!("v{i}"), vec![Some(1.0)])
            .unwrap();
    }
    let mut conn = memory();
    let tables = scalar(
        &conn,
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
    );
    match write_dataset(&mut conn, &d, "too_wide") {
        Ok(_) => problems.push("a source of 2000 variables (2001 columns) was accepted".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &json!("target_capability_exceeded"),
        ),
    }
    let state = json!(
        query(
            &conn,
            "SELECT (SELECT count(*) FROM dataset),
                (SELECT count(*) FROM sqlite_schema WHERE type = 'table'),
                (SELECT operation_kind || ' ' || status FROM operation),
                (SELECT count(*) FROM fidelity_event WHERE dataset_id IS NULL
                   AND direction = 'import' AND severity = 'error'
                   AND event_code = 'target_capability_exceeded')",
            [],
        )[0]
    );
    expect(
        &mut problems,
        "state after",
        state,
        &json!([0, tables, "import failed", 1]),
    );
    problems
}

/// Known gaps, each caused by the StatsCraft dataset model or its .sav reader, not by the
/// SQLite binding. None at present; a new gap is listed here with its reason.
const KNOWN_SAV_GAPS: [(&str, &str); 0] = [];

#[test]
fn sav_sav_zsav_1_0_on_sqlite() {
    with_spec(|dir| {
        let manifest = load(dir, "spss-sav-zsav-1.0.json");
        let scratch =
            std::env::temp_dir().join(format!("statscraft-sav-conf-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let fixtures = manifest["fixtures"].as_array().unwrap();
        let mut failed: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for fixture in fixtures {
            let id = fixture["id"].as_str().unwrap();
            let problems = if id == "preflight-failure" {
                preflight_problems()
            } else {
                sav_fixture_problems(dir, fixture, &scratch)
            };
            if !problems.is_empty() {
                failed.insert(id.to_string(), problems);
            }
        }
        std::fs::remove_dir(&scratch).unwrap();
        println!(
            "spss-sav-zsav-1.0.json on SQLite: {} of {} fixtures pass",
            fixtures.len() - failed.len(),
            fixtures.len()
        );
        for (id, problems) in &failed {
            let reason = KNOWN_SAV_GAPS
                .iter()
                .find(|g| g.0 == id)
                .map_or("", |g| g.1);
            println!("  {id} ({reason}): {problems:?}");
        }
        let known: Vec<&str> = KNOWN_SAV_GAPS.iter().map(|g| g.0).collect();
        let failing: Vec<&str> = failed.keys().map(String::as_str).collect();
        assert_eq!(failing, known, "failures other than the known model gaps");
    });
}

fn sav_fixture_problems(dir: &Path, fixture: &Value, scratch: &Path) -> Vec<String> {
    let source = fixture["source"].as_str().unwrap();
    let original = Dataset::read_sav(dir.join(source)).unwrap();
    let mut conn = memory();
    let mut problems = vec![];
    if let Err(e) = write_dataset(&mut conn, &original, "fixture") {
        return vec![format!("import: {e}")];
    }
    problems.extend(catalog_problems(&conn, &fixture["expected_catalog"]));
    let back = match read_dataset(&mut conn, "fixture", false) {
        Ok(d) => d,
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
fn make_dataset(conn: &mut Connection, name: &str, rows: &[Value], missing: &Value) -> String {
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
    write_dataset(conn, &d, name).unwrap()
}

/// The rows of dataset `name` with the keys of `like`.
fn table_rows(conn: &Connection, table: &str, like: &[Value]) -> Value {
    let keys: Vec<&String> = like[0].as_object().unwrap().keys().collect();
    let mut columns = vec![];
    for key in &keys {
        let physical = if *key == "__case_ordinal" {
            key.to_string()
        } else {
            query(
                conn,
                "SELECT physical_name FROM variable WHERE source_name = ?1",
                [key],
            )[0][0]
                .as_str()
                .unwrap()
                .to_string()
        };
        columns.push(SQLITE.quote(&physical));
    }
    let sql = format!(
        "SELECT {} FROM {} ORDER BY \"__case_ordinal\"",
        columns.join(", "),
        SQLITE.quote(table)
    );
    json!(
        query(conn, &sql, [])
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
fn snapshot(conn: &Connection) -> Value {
    json!([
        query(
            conn,
            "SELECT dataset_id, physical_table_schema, physical_table_name FROM dataset",
            []
        ),
        query(
            conn,
            "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name",
            []
        ),
        query(conn, "SELECT count(*) FROM sqlite_temp_schema", []),
    ])
}

/// Label, value labels, format, level and missing rules of a variable, in the manifests' shape.
fn metadata(conn: &Connection, variable: &str) -> Value {
    let v = &query(
        conn,
        "SELECT variable_id, variable_label, print_format_family, print_format_width,
                print_format_decimals, measurement_level, source_ordinal
         FROM variable WHERE source_name = ?1",
        [variable],
    )[0];
    let labels = query(
        conn,
        "SELECT coalesce(l.numeric_code, l.string_code), l.label
         FROM variable_value_label_set s JOIN value_label l USING (value_label_set_id)
         WHERE s.variable_id = ?1 ORDER BY l.ordinal",
        [v[0].as_str()],
    );
    let missing: Vec<Value> = query(
        conn,
        "SELECT ordinal, rule_kind, code_kind, numeric_value FROM missing_rule
         WHERE variable_id = ?1 ORDER BY ordinal",
        [v[0].as_str()],
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

/// The audit row against `expected_audit`; Dolt fields must be NULL on SQLite.
fn audit_problems(conn: &Connection, case: &Value, dataset_id: &str) -> Vec<String> {
    let mut problems = vec![];
    let statement = conn.prepare("SELECT * FROM transformation_apply").unwrap();
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    drop(statement);
    let rows = query(conn, "SELECT * FROM transformation_apply", []);
    if rows.len() != 1 {
        return vec![format!("{} audit rows", rows.len())];
    }
    let audit: Map<String, Value> = names.into_iter().zip(rows[0].clone()).collect();
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
    for (key, want) in case["expected_audit"].as_object().into_iter().flatten() {
        let want = match key.as_str() {
            k if k.starts_with("dolt_") => Value::Null,
            "database_profile" => json!("sqlite"),
            "dataset_id" => json!(dataset_id),
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
            &Value::Null,
        );
    }
    expect(
        &mut problems,
        "audit database_profile",
        audit["database_profile"].clone(),
        &json!("sqlite"),
    );
    problems
}

fn case_by_id<'m>(manifest: &'m Value, id: &str) -> &'m Value {
    cases(manifest)
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("case {id}"))
}

/// Runs one success case of the 0.2 manifest (native or adapted).
fn binding_02_problems(dir: &Path, case: &Value) -> Vec<String> {
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

    let mut conn = memory();
    let dataset_id = make_dataset(
        &mut conn,
        name,
        before_rows,
        &before["target_metadata"]["missing_values"],
    );
    // the frontend request of the case, with the input schema taken from our catalog
    let mut request =
        case_by_id(&frontend, case["applied_frontend_case"].as_str().unwrap())["request"].clone();
    request["input_schema"] = input_schema(&conn, name, true).unwrap();
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

    if let Some(probe) = case.get("rollback_probe") {
        // fail at the audit insert, after the column, data and catalog changes
        conn.execute_batch(
            "CREATE TEMP TRIGGER probe BEFORE INSERT ON main.transformation_apply
             BEGIN SELECT RAISE(ABORT, 'rollback probe'); END",
        )
        .unwrap();
        let rows_before = table_rows(&conn, table, before_rows);
        if apply_plan(&mut conn, name, &compiled.plan, &source_hash, "probe").is_ok() {
            problems.push("rollback probe: the apply succeeded".into());
        }
        conn.execute_batch("DROP TRIGGER temp.probe").unwrap();
        let column = scalar(
            &conn,
            &format!("SELECT count(*) FROM pragma_table_info('{table}') WHERE name = 'target'"),
        );
        let variable = scalar(
            &conn,
            "SELECT count(*) FROM variable WHERE source_name = 'target'",
        );
        let audit = scalar(&conn, "SELECT count(*) FROM transformation_apply");
        let changed = table_rows(&conn, table, before_rows) != rows_before;
        expect(
            &mut problems,
            "rollback probe",
            json!({"physical_column_present": column != 0, "catalog_variable_present": variable != 0,
                   "row_values_changed": changed, "audit_row_present": audit != 0}),
            &json!({"physical_column_present": probe["physical_column_present"],
                    "catalog_variable_present": probe["catalog_variable_present"],
                    "row_values_changed": probe["row_values_changed"],
                    "audit_row_present": probe["audit_row_present"]}),
        );
    }

    let target_before =
        (before.get("target_identity").is_some()).then(|| metadata(&conn, "target"));
    let snap = snapshot(&conn);
    let actor = case["actor"].as_str().unwrap();
    if let Err(e) = apply_plan(&mut conn, name, &compiled.plan, &source_hash, actor) {
        problems.push(format!("apply: {e}"));
        return problems;
    }
    expect(
        &mut problems,
        "dataset, tables and temporary objects",
        snapshot(&conn),
        &snap,
    );
    expect(
        &mut problems,
        "rows",
        table_rows(&conn, table, after_rows),
        &json!(after_rows),
    );
    problems.extend(audit_problems(&conn, case, &dataset_id));
    if let Some(want) = after.get("target_metadata") {
        compare_metadata(&mut problems, &metadata(&conn, "target"), want);
    }
    if let Some(target) = after.get("target") {
        let got = metadata(&conn, "target");
        compare_metadata(&mut problems, &got, &target["initial_metadata"]);
        expect(
            &mut problems,
            "target ordinal",
            got["ordinal"].clone(),
            &target["ordinal"],
        );
        let count = scalar(&conn, "SELECT count(*) FROM variable");
        expect(
            &mut problems,
            "variable count",
            count,
            &after["variable_count"],
        );
    }
    if let Some(before_target) = target_before {
        let got = metadata(&conn, "target");
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

/// The 0.1 MySQL success case on SQLite: rows are not given, so a synthetic `score` column
/// 1..4 is used and the recode rules decide the result.
fn binding_01_problems(case: &Value) -> Vec<String> {
    let mut problems = vec![];
    let mut conn = memory();
    let rows: Vec<Value> = (1..=4)
        .map(|i| json!({"__case_ordinal": i, "score": i}))
        .collect();
    let dataset_id = make_dataset(&mut conn, "survey", &rows, &Value::Null);
    let request = json!({
        "contract": "openstatspec-spss-syntax-frontend-v0.1",
        "input_alias": "input",
        "input_schema": input_schema(&conn, "survey", false).unwrap(),
        "source_text": case["source_text"],
    });
    let out = compile_request(&request);
    let compiled = match out.result {
        Ok(c) => c,
        Err(d) => return vec![format!("frontend: {} {}", d.code, d.message)],
    };
    let snap = snapshot(&conn);
    if let Err(e) = apply_plan(
        &mut conn,
        "survey",
        &compiled.plan,
        &out.source_hash.unwrap(),
        "runner",
    ) {
        return vec![format!("apply: {e}")];
    }
    expect(
        &mut problems,
        "dataset, tables and temporary objects",
        snapshot(&conn),
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
        table_rows(&conn, "data_survey", &after),
        &json!(after),
    );
    let got = metadata(&conn, "score");
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
    problems.extend(audit_problems(&conn, &audit, &dataset_id));
    problems
}

/// Every apply needs an actor, on every profile.
fn empty_actor_problems(dir: &Path) -> Vec<String> {
    let plans = load(dir, "transformation-plan-0.2.json");
    let plan = statscraft_openstatspec::validate_plan(
        &case_by_id(&plans, "sqlite-create-target-source-copy")["plan"],
    )
    .unwrap();
    let mut conn = memory();
    let rows = vec![json!({"__case_ordinal": 1, "source_a": 2})];
    make_dataset(&mut conn, "atomic_create", &rows, &Value::Null);
    let snap = snapshot(&conn);
    let hash = "0".repeat(64);
    let mut problems = vec![];
    match apply_plan(&mut conn, "atomic_create", &plan, &hash, "") {
        Ok(_) => problems.push("an empty actor was accepted".into()),
        Err(e) => expect(
            &mut problems,
            "code",
            json!(e.code),
            &json!("actor_required"),
        ),
    }
    expect(&mut problems, "state", snapshot(&conn), &snap);
    expect(
        &mut problems,
        "columns",
        scalar(
            &conn,
            "SELECT count(*) FROM pragma_table_info('data_atomic_create')",
        ),
        &json!(2),
    );
    problems
}

#[test]
fn in_place_transformation_bindings_on_sqlite() {
    with_spec(|dir| {
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
                let profile = case["database_profile"].as_str().unwrap();
                let problems = match id {
                    "mysql-recode-and-labels-preserve-dataset-and-table-identity" => {
                        binding_01_problems(case)
                    }
                    "dolt-empty-actor-fails-before-mutation" => empty_actor_problems(dir),
                    _ if case.get("applied_frontend_case").is_some()
                        && case["expected_error"].is_null() =>
                    {
                        binding_02_problems(dir, case)
                    }
                    _ => {
                        not_applicable.push(id);
                        continue;
                    }
                };
                let ok = problems.is_empty();
                if profile == "sqlite" {
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
                "{name}: SQLite cases {passed_native} of {native} pass; adapted cases \
                 {passed_adapted} of {adapted} pass; {} of {} cases not applicable to SQLite: {not_applicable:?}",
                not_applicable.len(),
                cases(&manifest).len()
            );
        }
        assert!(failures.is_empty(), "failing cases: {failures:#?}");
    });
}
