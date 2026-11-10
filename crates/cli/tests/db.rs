//! `statscraft db` and `statscraft plan --apply`, end to end in a temporary working directory.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

struct Workdir(PathBuf);

impl Workdir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("statscraft-db-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Runs the CLI; returns success and stdout parsed as JSON.
    fn run(&self, args: &[&str]) -> (bool, Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_statscraft"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1, "one line of JSON: {stdout}");
        (out.status.success(), serde_json::from_str(&stdout).unwrap())
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn export_apply_and_import_through_sqlite() {
    let w = Workdir::new("flow");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/survey_basic.sav"
    );
    let (ok, out) = w.run(&[
        "db", "export", fixture, "--sqlite", "s.db", "--name", "survey",
    ]);
    assert!(ok, "{out}");
    assert_eq!(out["table"], "data_survey");
    assert_eq!(out["case_count"], 200);
    let (ok, out) = w.run(&[
        "db", "export", fixture, "--sqlite", "s.db", "--name", "survey",
    ]);
    assert!(!ok);
    assert_eq!(out["error"]["code"], "dataset_exists");

    let syntax = "RECODE age (LOWEST THRU 29 = 1) (ELSE = 0) INTO young.\n\
                  VARIABLE LABELS young 'Under 30'.";
    let apply = |actor: &str| {
        w.run(&[
            "plan", "-e", syntax, "--sqlite", "s.db", "--name", "survey", "--apply", "--actor",
            actor,
        ])
    };
    let (ok, out) = apply("");
    assert!(!ok);
    assert_eq!(out["error"]["code"], "actor_required");
    let (ok, out) = apply("tester");
    assert!(ok, "{out}");
    assert_eq!(out["applied"]["operation_count"], 2);
    let (ok, out) = apply("tester");
    assert!(!ok, "the target exists now");
    assert_eq!(out["error"]["code"], "target_already_exists");

    let (ok, out) = w.run(&[
        "db", "import", "--sqlite", "s.db", "--name", "survey", "--out", "b.sav",
    ]);
    assert!(ok, "{out}");
    assert_eq!(out["variable_count"], 20);
    let describe = Command::new(env!("CARGO_BIN_EXE_statscraft"))
        .args(["describe", "b.sav", "--json"])
        .current_dir(&w.0)
        .output()
        .unwrap();
    let d: Value = serde_json::from_slice(&describe.stdout).unwrap();
    let young = &d["variables"][19];
    assert_eq!(young["name"], "young");
    assert_eq!(young["label"], "Under 30");
    assert_eq!(young["n_valid"], 200);

    let (ok, out) = w.run(&[
        "db",
        "import",
        "--sqlite",
        "missing.db",
        "--name",
        "x",
        "--out",
        "x.sav",
    ]);
    assert!(!ok);
    assert_eq!(out["error"]["code"], "database_unavailable");
}
