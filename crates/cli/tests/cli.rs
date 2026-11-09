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

    let third = dir.json(&["run", "-s", "t1", "--json", "-e", "FREQUENCIES gender."]);
    assert_eq!(
        third["saved"], false,
        "a run that changes nothing does not rewrite the session"
    );
    assert_eq!(third["commands"][0]["tables"][0]["rows"][0][3], 106.0);
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
    let out = dir.run(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        "VARIABLE LABELS age 'Changed'.\nFREQUENCIES no_such_variable.",
    ]);
    assert!(!out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"][0]["line"], 2);
    assert_eq!(report["errors"][0]["command"], "FREQUENCIES");
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_ne!(label_of(&description, "age"), "Changed");
}

#[test]
fn check_reports_problems_without_changing_the_session() {
    let dir = Workdir::new("check");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    assert!(
        dir.ok(&[
            "check",
            "-s",
            "t1",
            "-e",
            "VARIABLE LABELS age 'Checked only'."
        ])
        .contains("checked 1 command")
    );
    assert_ne!(
        label_of(&dir.json(&["describe", "-s", "t1", "--json"]), "age"),
        "Checked only"
    );

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
