//! Runs the published OpenStatSpec conformance manifests, read in place from a local checkout
//! of the specification. Nothing from it is copied into this repository. The folder is found
//! through OPENSTATSPEC_DIR or as `openStatSpec/specification` beside an ancestor of this
//! crate; without it the tests skip with a message (CI has no checkout).

mod common;

use common::{cases, load, with_spec};
use serde_json::Value;
use statscraft_openstatspec::{canonical_json, compile_request, validate_plan, wire};
use std::collections::HashMap;
use std::path::Path;

/// Plan cases by id: the plan and its expected hash.
fn plan_cases(dir: &Path, name: &str) -> HashMap<String, (Value, Value)> {
    cases(&load(dir, name))
        .iter()
        .map(|c| {
            let id = c["id"].as_str().unwrap().to_string();
            (id, (c["plan"].clone(), c["expected_plan_hash"].clone()))
        })
        .collect()
}

#[test]
fn transformation_plan_manifests() {
    with_spec(|dir| {
        let mut failures = vec![];
        for name in [
            "transformation-plan-0.1.json",
            "transformation-plan-0.2.json",
        ] {
            let manifest = load(dir, name);
            let mut passed = 0;
            for case in cases(&manifest) {
                let ok = match (&case["expected_error"], validate_plan(&case["plan"])) {
                    (Value::String(code), Err(e)) => e.code == code,
                    (Value::Null, Ok(plan)) => {
                        plan.hash() == case["expected_plan_hash"]
                            && canonical_json(&case["plan"]) == Some(plan.canonical())
                    }
                    _ => false,
                };
                if ok {
                    passed += 1;
                } else {
                    failures.push(format!("{name} {}", case["id"]));
                }
            }
            println!("{name}: {passed}/{} cases pass", cases(&manifest).len());
        }
        assert!(failures.is_empty(), "failing cases: {failures:#?}");
    });
}

#[test]
fn syntax_frontend_manifests() {
    with_spec(|dir| {
        let plan01 = plan_cases(dir, "transformation-plan-0.1.json");
        let plan02 = plan_cases(dir, "transformation-plan-0.2.json");
        // `expected_plan_case_0_1` names a Frontend 0.1 case; its plan is inline or a plan case.
        let frontend01: HashMap<String, (Value, Value)> =
            cases(&load(dir, wire::FRONTEND_MANIFESTS[0]))
                .iter()
                .filter(|c| c["expected_error"].is_null())
                .map(|c| {
                    let (plan, hash) = match c["expected_plan_case"].as_str() {
                        Some(id) => plan01[id].clone(),
                        None => (c["expected_plan"].clone(), c["expected_plan_hash"].clone()),
                    };
                    (c["id"].as_str().unwrap().to_string(), (plan, hash))
                })
                .collect();
        // `expected_plan_case` names a case of the plan version the manifest itself targets.
        let own_plans = |manifest: &str| {
            if manifest.ends_with("0.1.json") {
                &plan01
            } else {
                &plan02
            }
        };
        let mut failures = vec![];
        for name in wire::FRONTEND_MANIFESTS {
            let manifest = load(dir, name);
            // (case, contract override, manifest that defines it)
            let mut effective: Vec<(Value, Option<Value>, String)> = vec![];
            for inherited in manifest["inherited_manifests"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let source = inherited["manifest"].as_str().unwrap();
                let superseded = &inherited["superseded_cases"];
                for case in cases(&load(dir, source)) {
                    if superseded.get(case["id"].as_str().unwrap()).is_none() {
                        let contract = inherited["request_contract_override"].clone();
                        effective.push((case.clone(), Some(contract), source.to_string()));
                    }
                }
            }
            for case in cases(&manifest) {
                effective.push((case.clone(), None, name.to_string()));
            }

            let mut passed = 0;
            for (case, contract, source) in &effective {
                let mut request = case["request"].clone();
                if let Some(contract) = contract {
                    request["contract"] = contract.clone();
                }
                let out = compile_request(&request);
                let referenced = case
                    .get("expected_plan_case")
                    .map(|id| &own_plans(source)[id.as_str().unwrap()])
                    .or_else(|| {
                        case.get("expected_plan_case_0_1")
                            .map(|id| &frontend01[id.as_str().unwrap()])
                    });
                let expected_plan = case
                    .get("expected_plan")
                    .or_else(|| case.get("expected_plan_0_1"))
                    .or(referenced.map(|r| &r.0));
                let expected_hash = case.get("expected_plan_hash").or(referenced.map(|r| &r.1));
                let source_ok = out.source_hash.as_deref() == case["expected_source_hash"].as_str();
                let result_ok = match (&case["expected_error"], &out.result) {
                    (Value::String(code), Err(d)) => d.code == code,
                    (Value::Null, Ok(c)) => {
                        let contract_ok = case
                            .get("expected_plan_contract")
                            .is_none_or(|k| *k == c.plan.contract);
                        contract_ok
                            && expected_plan.and_then(canonical_json) == Some(c.canonical.clone())
                            && expected_hash.and_then(Value::as_str) == Some(c.plan_hash.as_str())
                            && validate_plan(&c.plan.to_json()).is_ok()
                    }
                    _ => false,
                };
                if source_ok && result_ok {
                    passed += 1;
                } else {
                    failures.push(format!(
                        "{name} {} (from {source}): got {:?}",
                        case["id"],
                        out.result
                            .as_ref()
                            .map(|c| &c.canonical)
                            .map_err(|d| (d.code, &d.message))
                    ));
                }
            }
            println!("{name}: {passed}/{} effective cases pass", effective.len());
            if name.ends_with("0.3.json") {
                // The 0.3 profile text declares 90 effective cases.
                assert_eq!(effective.len(), 90);
            }
        }
        assert!(failures.is_empty(), "failing cases: {failures:#?}");
    });
}
