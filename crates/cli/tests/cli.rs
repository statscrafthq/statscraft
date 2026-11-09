//! End-to-end tests of the command line, each in its own temporary working directory.

use std::path::PathBuf;
use std::process::{Command, Output};

struct Workdir(PathBuf);

impl Workdir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("statscraft-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statscraft"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        serde_json::from_str(&self.ok(args)).unwrap()
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn get_fixture(name: &str) -> String {
    format!(
        "GET FILE='{}/../../tests/fixtures/{name}'.",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn label_of(description: &serde_json::Value, variable: &str) -> String {
    description["variables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == variable)
        .unwrap()["label"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn a_session_keeps_the_dataset_between_runs() {
    let dir = Workdir::new("session");
    let first = dir.json(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        &get_fixture("survey_basic.sav"),
    ]);
    assert_eq!(first["dataset"], serde_json::json!([200, 19]));
    assert_eq!(first["saved"], true);

    let second = dir.json(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        "VARIABLE LABELS age 'Age at interview'.",
    ]);
    assert_eq!(
        second["changes"]["variables_changed"],
        serde_json::json!(["age"])
    );

    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_eq!(label_of(&description, "age"), "Age at interview");

    let session = dir.0.join(".statscraft/sessions/t1/data.zsav");
    let modified = std::fs::metadata(&session).unwrap().modified().unwrap();
    let third = dir.json(&["run", "-s", "t1", "--json", "-e", "FREQUENCIES gender."]);
    assert_eq!(
        third["saved"], false,
        "a run that changes nothing does not rewrite the session"
    );
    assert_eq!(
        std::fs::metadata(&session).unwrap().modified().unwrap(),
        modified
    );
    assert_eq!(third["commands"][0]["tables"][0]["rows"][0][3], 106.0);
}

fn numbers(data: &statscraft_data::Dataset, name: &str) -> Vec<Option<f64>> {
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

#[test]
fn session_value_changes_survive_a_separate_run() {
    let dir = Workdir::new("session-values");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let session = dir.0.join(".statscraft/sessions/t1/data.zsav");
    let before = statscraft_data::Dataset::read_sav(&session).unwrap();
    for (syntax, name) in [
        ("COMPUTE age = age + 100.", "age"),
        ("COMPUTE plan = 'PLUS'.", "plan"),
    ] {
        let report = dir.json(&["run", "-s", "t1", "--json", "-e", syntax]);
        assert_eq!(report["saved"], true, "{syntax}");
        let changes = &report["changes"];
        assert_eq!(changes["variables_changed"], serde_json::json!([name]));
        assert_eq!(changes["variables_added"], serde_json::json!([]));
        assert_eq!(changes["variables_removed"], serde_json::json!([]));
    }
    dir.ok(&["run", "-s", "t1", "-e", "SAVE OUTFILE='reloaded.sav'."]);
    let after = statscraft_data::Dataset::read_sav(dir.0.join("reloaded.sav")).unwrap();
    let plus_100: Vec<_> = numbers(&before, "age")
        .into_iter()
        .map(|v| v.map(|v| v + 100.0))
        .collect();
    assert_eq!(numbers(&after, "age"), plus_100);
    let plan = after.variable("plan").unwrap();
    assert!(plan.string().unwrap().iter().all(|v| v == "PLUS"));

    let modified = std::fs::metadata(&session).unwrap().modified().unwrap();
    let same = "COMPUTE age = age.\nCOMPUTE plan = plan.";
    let unchanged = dir.json(&["run", "-s", "t1", "--json", "-e", same]);
    assert_eq!(unchanged["saved"], false);
    assert_eq!(
        unchanged["changes"]["variables_changed"],
        serde_json::json!([])
    );
    assert_eq!(
        std::fs::metadata(&session).unwrap().modified().unwrap(),
        modified
    );
}

#[test]
fn session_case_order_survives_a_separate_run() {
    let dir = Workdir::new("session-order");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let sort = "SORT CASES BY resp_id (D).";
    let report = dir.json(&["run", "-s", "t1", "--json", "-e", sort]);
    assert_eq!(report["saved"], true);
    let changed = report["changes"]["variables_changed"].as_array().unwrap();
    assert!(changed.contains(&serde_json::json!("resp_id")));
    dir.ok(&["run", "-s", "t1", "-e", "SAVE OUTFILE='reloaded.sav'."]);
    let after = statscraft_data::Dataset::read_sav(dir.0.join("reloaded.sav")).unwrap();
    let descending: Vec<_> = (1001..=1200).rev().map(|id| Some(f64::from(id))).collect();
    assert_eq!(numbers(&after, "resp_id"), descending);

    let unchanged = dir.json(&["run", "-s", "t1", "--json", "-e", sort]);
    assert_eq!(unchanged["saved"], false);
    assert_eq!(
        unchanged["changes"]["variables_changed"],
        serde_json::json!([])
    );
}

#[test]
fn session_weight_changes_survive_a_separate_run() {
    let dir = Workdir::new("session-weight");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    for (syntax, expected) in [
        ("WEIGHT BY weight.", serde_json::json!("weight")),
        ("WEIGHT OFF.", serde_json::Value::Null),
    ] {
        let report = dir.json(&["run", "-s", "t1", "--json", "-e", syntax]);
        assert_eq!(report["saved"], true, "{syntax}");
        assert_eq!(
            report["changes"]["variables_changed"],
            serde_json::json!([])
        );
        let description = dir.json(&["describe", "-s", "t1", "--json"]);
        assert_eq!(description["weight_variable"], expected, "{syntax}");
    }
}

#[test]
fn a_line_range_runs_only_the_commands_inside_it() {
    let dir = Workdir::new("lines");
    let syntax = format!(
        "{}\nVARIABLE LABELS age 'First'.\nVARIABLE LABELS\n  gender 'Second'.\nFREQUENCIES region.\n",
        get_fixture("survey_basic.sav")
    );
    std::fs::write(dir.0.join("job.sps"), syntax).unwrap();
    dir.ok(&["run", "-s", "t1", "--lines", "1-1", "job.sps"]);

    let partial = dir.json(&["run", "-s", "t1", "--json", "--lines", "3-4", "job.sps"]);
    assert_eq!(partial["commands"].as_array().unwrap().len(), 1);
    assert_eq!(
        partial["changes"]["variables_changed"],
        serde_json::json!(["gender"])
    );
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_ne!(
        label_of(&description, "age"),
        "First",
        "line 2 was not in the range"
    );

    let cut = dir.run(&["run", "-s", "t1", "--lines", "3-3", "job.sps"]);
    assert!(!cut.status.success());
    assert!(
        String::from_utf8_lossy(&cut.stderr).contains("VARIABLE LABELS"),
        "{cut:?}"
    );
}

#[test]
fn a_failing_run_saves_nothing() {
    let dir = Workdir::new("atomic");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let session = dir.0.join(".statscraft/sessions/t1/data.zsav");
    let bytes = std::fs::read(&session).unwrap();
    let out = dir.run(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        "VARIABLE LABELS age 'Changed'.\nCOMPUTE age = age + 100.\nWEIGHT BY weight.\n\
         FREQUENCIES no_such_variable.",
    ]);
    assert!(!out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"][0]["line"], 4);
    assert_eq!(report["errors"][0]["command"], "FREQUENCIES");
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_ne!(label_of(&description, "age"), "Changed");
    assert_eq!(std::fs::read(&session).unwrap(), bytes);
}

#[test]
fn check_reports_problems_without_changing_the_session() {
    let dir = Workdir::new("check");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let session = dir.0.join(".statscraft/sessions/t1/data.zsav");
    let bytes = std::fs::read(&session).unwrap();
    assert!(
        dir.ok(&[
            "check",
            "-s",
            "t1",
            "-e",
            "VARIABLE LABELS age 'Checked only'.\nCOMPUTE age = age + 100.\n\
             COMPUTE plan = 'PLUS'.\nWEIGHT BY weight."
        ])
        .contains("checked 4 command")
    );
    assert_ne!(
        label_of(&dir.json(&["describe", "-s", "t1", "--json"]), "age"),
        "Checked only"
    );
    assert_eq!(std::fs::read(&session).unwrap(), bytes);

    let syntax_error = dir.run(&["check", "-e", "FREQUENCEIS gender."]);
    assert!(!syntax_error.status.success());
    assert!(
        String::from_utf8_lossy(&syntax_error.stderr).contains("FREQUENCIES"),
        "did-you-mean hint expected"
    );

    let unknown = dir.run(&["check", "-s", "t1", "-e", "FREQUENCIES nope."]);
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown variable nope"));
}

#[test]
fn help_comes_from_the_command_table() {
    let dir = Workdir::new("help");
    assert!(dir.ok(&["help", "frequencies"]).contains("FREQUENCIES"));
    assert!(dir.ok(&["help"]).contains("VALUE LABELS"));
    assert!(!dir.run(&["help", "nonsense"]).status.success());
    assert!(dir.ok(&["--version"]).starts_with("statscraft "));
}

#[test]
fn transformations_run_in_a_session_and_are_summarised() {
    let dir = Workdir::new("transform");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let syntax = "COMPUTE channels = SUM(q5_1 TO q5_5).
DO IF (age < 30).
COMPUTE age_group = 1.
ELSE.
COMPUTE age_group = 2.
END IF.
STRING tier (A8).
IF (plan = 'BASIC') tier = 'low'.
SELECT IF q1_satisfaction > 3.";
    let report = dir.json(&["run", "-s", "t1", "--json", "-e", syntax]);
    let changes = &report["changes"];
    assert_eq!(
        changes["variables_added"],
        serde_json::json!(["channels", "age_group", "tier"])
    );
    assert_eq!(
        (
            changes["cases_before"].as_u64(),
            changes["cases_after"].as_u64()
        ),
        (Some(200), Some(103))
    );
    assert_eq!(report["saved"], true);
    // the next run sees the new variables
    let report = dir.json(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        "COMPUTE twice = channels * 2.",
    ]);
    assert_eq!(report["dataset"], serde_json::json!([103, 23]));

    let open = dir.run(&[
        "run",
        "-s",
        "t1",
        "-e",
        "DO IF (age < 30).\nCOMPUTE young = 1.",
    ]);
    assert!(!open.status.success());
    assert!(String::from_utf8_lossy(&open.stderr).contains("DO IF without END IF"));
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_eq!(description["variable_count"], 23);
}
