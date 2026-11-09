//! `statscraft plan`, end to end, each test in its own temporary working directory.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

struct Workdir(PathBuf);

impl Workdir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("statscraft-plan-{tag}-{}", std::process::id()));
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

const SCHEMA: &str = r#"{"variables": [{"name": "Q1", "storage_kind": "numeric"},
                                       {"name": "q2", "storage_kind": "numeric"}]}"#;

#[test]
fn plan_from_a_file_and_an_input_schema() {
    let w = Workdir::new("file");
    std::fs::write(w.0.join("schema.json"), SCHEMA).unwrap();
    std::fs::write(
        w.0.join("job.sps"),
        "RECODE q1 (1 THRU 3 = 1) (ELSE = 0) INTO low.\r\n",
    )
    .unwrap();
    let (ok, out) = w.run(&["plan", "job.sps", "--input-schema", "schema.json"]);
    assert!(ok, "{out}");
    let plan = &out["plan"];
    assert_eq!(plan["contract"], "openstatspec-transformation-plan-v0.1");
    assert_eq!(plan["input_alias"], "input");
    assert_eq!(plan["operations"][0]["source"], "Q1");
    assert_eq!(
        plan["operations"][0]["unmatched"]["value"]["bits"],
        "0000000000000000"
    );
    assert_eq!(out["plan_hash"].as_str().unwrap().len(), 64);
    assert_eq!(out["source_hash"].as_str().unwrap().len(), 64);

    // The same program from -e with LF line ends has the same source hash.
    let (_, again) = w.run(&[
        "plan",
        "-e",
        "RECODE q1 (1 THRU 3 = 1) (ELSE = 0) INTO low.\n",
        "--input-schema",
        "schema.json",
    ]);
    assert_eq!(again["source_hash"], out["source_hash"]);
    assert_eq!(again["plan_hash"], out["plan_hash"]);
}

#[test]
fn errors_are_json_with_code_and_position() {
    let w = Workdir::new("error");
    std::fs::write(w.0.join("schema.json"), SCHEMA).unwrap();
    let schema = ["--input-schema", "schema.json"];
    let (ok, out) = w.run(&[&["plan", "-e", "EXECUTE.\nFREQUENCIES q1."], &schema[..]].concat());
    assert!(!ok);
    assert_eq!(out["error"]["code"], "unsupported_spss_command");
    assert_eq!(
        (&out["error"]["line"], &out["error"]["column"]),
        (&2.into(), &1.into())
    );

    let (ok, out) = w.run(
        &[
            &["plan", "--profile", "0.1", "-e", "COMPUTE x = 1."],
            &schema[..],
        ]
        .concat(),
    );
    assert!(!ok);
    assert_eq!(out["error"]["code"], "unsupported_spss_command");

    let (ok, out) = w.run(&["plan", "-e", "EXECUTE."]);
    assert!(!ok);
    assert_eq!(out["error"]["code"], "usage");
}

#[test]
fn input_schema_from_a_session() {
    let w = Workdir::new("session");
    let get = format!(
        "GET FILE='{}/../../tests/fixtures/edge_cases.sav'.",
        env!("CARGO_MANIFEST_DIR")
    );
    let loaded = Command::new(env!("CARGO_BIN_EXE_statscraft"))
        .args(["run", "-s", "work", "-e", &get])
        .current_dir(&w.0)
        .output()
        .unwrap();
    assert!(loaded.status.success());
    let (ok, out) = w.run(&[
        "plan",
        "-s",
        "work",
        "-e",
        "ADD VALUE LABELS three_missing 2 'Two'.",
    ]);
    assert!(ok, "{out}");
    let labels = out["plan"]["operations"][0]["labels"].as_array().unwrap();
    // The five labels of the file keep their order and the new one is appended.
    assert_eq!(labels.len(), 6);
    assert_eq!(labels[5]["label"], "Two");
    assert_eq!(labels[5]["value"]["bits"], "4000000000000000");
}
