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
fn session_split_and_filter_survive_a_separate_run() {
    let dir = Workdir::new("session-split");
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let report = dir.json(&[
        "run",
        "-s",
        "t1",
        "--json",
        "-e",
        "SORT CASES BY gender.\nSPLIT FILE SEPARATE BY gender.\nFILTER BY q5_1.",
    ]);
    assert_eq!(report["saved"], true);
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert_eq!(
        description["split_file"],
        serde_json::json!({"variables": ["gender"], "layered": false})
    );
    assert_eq!(description["filter_variable"], "q5_1");
    let report = dir.json(&["run", "-s", "t1", "--json", "-e", "DESCRIPTIVES age."]);
    let titles: Vec<&str> = report["commands"][0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles.len(), 3, "{titles:?}");
    assert!(
        titles[0].starts_with("Descriptive Statistics (Gender = "),
        "{titles:?}"
    );
    dir.ok(&["run", "-s", "t1", "-e", "SPLIT FILE OFF.\nFILTER OFF."]);
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    assert!(
        description.get("split_file").is_none() && description.get("filter_variable").is_none()
    );
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

/// The parts of a workbook, without docProps/core.xml, which holds the time it was written.
fn workbook_parts(path: &std::path::Path) -> Vec<(String, String)> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut parts = Vec::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        let name = file.name().unwrap().to_string();
        if name != "docProps/core.xml" {
            parts.push((name, text));
        }
    }
    parts
}

fn sheet_names(path: &std::path::Path) -> Vec<String> {
    let parts = workbook_parts(path);
    let (_, workbook) = parts.iter().find(|(n, _)| n == "xl/workbook.xml").unwrap();
    workbook
        .split("<sheet name=\"")
        .skip(1)
        .map(|s| s[..s.find('"').unwrap()].to_string())
        .collect()
}

#[test]
fn run_writes_every_table_to_a_workbook() {
    let dir = Workdir::new("xlsx");
    let syntax = format!(
        "{}\nFREQUENCIES gender.\nCROSSTABS /TABLES=gender BY region /STATISTICS=CHISQ.",
        get_fixture("survey_basic.sav")
    );
    let plain = dir.ok(&["run", "-e", &syntax]);
    // the normal output stays the same
    assert_eq!(dir.ok(&["run", "--xlsx", "out.xlsx", "-e", &syntax]), plain);
    assert_eq!(
        sheet_names(&dir.0.join("out.xlsx")),
        [
            "Gender",
            "Gender by Region of residence",
            "Chi-Square Tests Gender by Regi"
        ]
    );
    dir.ok(&[
        "run",
        "--xlsx",
        "one.xlsx",
        "--xlsx-one-sheet",
        "-e",
        &syntax,
    ]);
    assert_eq!(sheet_names(&dir.0.join("one.xlsx")), ["Tables"]);
    let out = dir.run(&["run", "--xlsx-one-sheet", "-e", &syntax]);
    assert!(!out.status.success(), "--xlsx-one-sheet needs --xlsx");

    // in a session, the tables of this run only, with --json as well
    dir.ok(&["run", "-s", "t1", "-e", &get_fixture("survey_basic.sav")]);
    let report = dir.json(&[
        "run",
        "-s",
        "t1",
        "--json",
        "--xlsx",
        "session.xlsx",
        "-e",
        "FREQUENCIES gender.",
    ]);
    assert_eq!(report["ok"], true);
    assert_eq!(sheet_names(&dir.0.join("session.xlsx")), ["Gender"]);
    let first_sheet = |file: &str| {
        workbook_parts(&dir.0.join(file))
            .into_iter()
            .find(|(n, _)| n == "xl/worksheets/sheet1.xml")
            .unwrap()
            .1
    };
    assert_eq!(first_sheet("session.xlsx"), first_sheet("out.xlsx"));

    // a workbook that cannot be written fails the run before the session is saved
    let out = dir.run(&[
        "run",
        "-s",
        "t2",
        "--xlsx",
        "no/such/folder/out.xlsx",
        "-e",
        &get_fixture("survey_basic.sav"),
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no/such/folder/out.xlsx"));
    assert!(!dir.0.join(".statscraft/sessions/t2").exists());
}

#[test]
fn a_session_keeps_text_its_code_page_cannot_hold() {
    let dir = Workdir::new("session-code-page");
    dir.ok(&[
        "run",
        "-e",
        "DATA LIST FREE /x.\nBEGIN DATA\n1 2\nEND DATA.\nSAVE OUTFILE='u.sav'.",
    ]);
    let mut data = statscraft_data::Dataset::read_sav(dir.0.join("u.sav")).unwrap();
    data.set_encoding("windows-1252").unwrap();
    data.write_sav(dir.0.join("w.sav")).unwrap();
    dir.ok(&["run", "-s", "t1", "-e", "GET FILE='w.sav'."]);
    // Latvian a with macron: no code in Windows-1252
    let label = dir.json(&["run", "-s", "t1", "--json", "-e", "VALUE LABELS x 1 'ā'."]);
    assert_eq!(label["saved"], true);
    let next = dir.json(&["run", "-s", "t1", "--json", "-e", "COMPUTE y = x * 2."]);
    assert_eq!(next["dataset"], serde_json::json!([2, 2]));
    let description = dir.json(&["describe", "-s", "t1", "--json"]);
    let labels = &description["variables"][0]["value_labels"];
    assert_eq!(labels.to_string(), r#"{"1":"ā"}"#);
}

#[test]
fn a_syntax_file_in_a_legacy_code_page_is_read_by_detection_or_by_name() {
    let dir = Workdir::new("encoding");
    // "Šoti" in Windows-1257, where 0xD0 is S with caron; not valid UTF-8
    let mut bytes = get_fixture("survey_basic.sav").into_bytes();
    bytes.extend_from_slice(b"\nVARIABLE LABELS age '\xD0oti'.\nSAVE OUTFILE='out.sav'.\n");
    std::fs::write(dir.0.join("legacy.sps"), &bytes).unwrap();
    dir.ok(&["run", "legacy.sps", "--encoding", "windows-1257"]);
    let description = dir.json(&["describe", "out.sav", "--json"]);
    assert_eq!(label_of(&description, "age"), "Šoti");
    // without --encoding the system's legacy code page decodes the byte to one character
    dir.ok(&["run", "legacy.sps"]);
    let description = dir.json(&["describe", "out.sav", "--json"]);
    assert_eq!(label_of(&description, "age").chars().count(), 4);
    let out = dir.run(&["run", "legacy.sps", "--encoding", "klingon"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown encoding klingon"));
}

#[test]
fn a_session_keeps_its_named_datasets_between_runs() {
    let dir = Workdir::new("datasets");
    let data = "DATA LIST LIST /x.\nBEGIN DATA\n1\n2\n3\nEND DATA.\nDATASET NAME full.\nDATASET COPY part.";
    dir.ok(&["run", "-s", "d", "-e", data]);
    dir.ok(&[
        "run",
        "-s",
        "d",
        "-e",
        "DATASET ACTIVATE part.\nSELECT IF x > 1.",
    ]);
    let result = dir.json(&[
        "run",
        "-s",
        "d",
        "--json",
        "-e",
        "DATASET ACTIVATE full.\nDATASET DISPLAY.",
    ]);
    let rows = &result["commands"][1]["tables"][0]["rows"];
    assert_eq!(rows[0], serde_json::json!(["full", 3.0, 1.0, "yes"]));
    assert_eq!(rows[1], serde_json::json!(["part", 2.0, 1.0, ""]));
}

#[test]
fn a_session_keeps_its_macros_and_macro_settings_between_runs() {
    let dir = Workdir::new("macros");
    // a library of macros run before there is any data, as an INCLUDE would bring it in
    let library = "DEFINE !double (!POSITIONAL !TOKENS(1)) (!1 * 2) !ENDDEFINE.\nSET MPRINT=YES.";
    assert!(
        dir.ok(&["run", "-s", "m", "-e", library])
            .contains("session saved")
    );
    let data = "DATA LIST LIST /x.\nBEGIN DATA\n1\n4\nEND DATA.\nCOMPUTE y = !double x.";
    let result = dir.json(&["run", "-s", "m", "--json", "-e", data]);
    // SET MPRINT=YES of the first run shows the expansion in the second
    let echo = &result["commands"][2]["tables"][0];
    assert_eq!(echo["title"], "Macro expansion");
    assert_eq!(echo["rows"][0][0], "COMPUTE y = (x * 2)");
    let result = dir.json(&["check", "-s", "m", "--json", "-e", "DISPLAY MACROS."]);
    assert_eq!(
        result["commands"][0]["tables"][0]["rows"],
        serde_json::json!([["!DOUBLE"]])
    );
    let listed = dir.json(&["run", "-s", "m", "--json", "-e", "LIST y."]);
    let rows = &listed["commands"][0]["tables"][0]["rows"];
    assert_eq!(rows, &serde_json::json!([["2.00"], ["8.00"]]));
}

/// The setting and value rows of the SHOW table of a command.
fn shown(report: &serde_json::Value, command: usize) -> Vec<[String; 2]> {
    let rows = report["commands"][command]["tables"][0]["rows"]
        .as_array()
        .unwrap();
    let text = |cell: &serde_json::Value| cell.as_str().unwrap().to_string();
    rows.iter().map(|r| [text(&r[0]), text(&r[1])]).collect()
}

fn pair(name: &str, value: &str) -> [String; 2] {
    [name.into(), value.into()]
}

#[test]
fn a_session_keeps_its_settings_preserve_levels_and_folder_between_runs() {
    let dir = Workdir::new("settings");
    std::fs::create_dir_all(dir.0.join("data")).unwrap();
    // settings before there is any data are kept too
    let first = "SET FORMAT=F6.1 DECIMAL=COMMA.\nCD 'data'.\nPRESERVE.\nSET FORMAT=F4.0.";
    assert!(
        dir.ok(&["run", "-s", "s", "-e", first])
            .contains("session saved")
    );
    let file = dir.0.join(".statscraft/sessions/s/settings.sps");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("\nSET DECIMAL='COMMA' FORMAT='F6.1'.\nCD '")
            && text.ends_with("data'.\nPRESERVE.\nSET FORMAT='F4.0'.\n"),
        "{text}"
    );
    let description = dir.json(&["describe", "-s", "s", "--json"]);
    assert_eq!(description["session"]["settings"]["FORMAT"], "F4.0");
    assert_eq!(description["session"]["preserved"], 1);
    let folder = description["session"]["directory"].as_str().unwrap();
    assert!(folder.ends_with("data"), "{folder}");

    // a relative SAVE goes to the CD folder of the first run
    let data = "DATA LIST LIST /x.\nBEGIN DATA\n1\nEND DATA.\nSAVE OUTFILE='x.sav'.";
    dir.ok(&["run", "-s", "s", "-e", data]);
    assert!(dir.0.join("data/x.sav").exists());
    // RESTORE returns to the settings before the PRESERVE of the first run
    let show = "SHOW FORMAT.\nRESTORE.\nSHOW FORMAT DECIMAL.";
    let report = dir.json(&["run", "-s", "s", "--json", "-e", show]);
    assert_eq!(shown(&report, 0), [pair("FORMAT", "F4.0")]);
    assert_eq!(
        shown(&report, 2),
        [pair("FORMAT", "F6.1"), pair("DECIMAL", "COMMA")]
    );
    dir.ok(&["run", "-s", "s", "-e", "COMPUTE y = 2."]);
    let description = dir.json(&["describe", "-s", "s", "--json"]);
    let y = &description["variables"][1];
    assert_eq!([&y["name"], &y["format"]], ["y", "F6.1"]);
    assert_eq!(description["session"]["preserved"], 0);
}

#[test]
fn settings_that_change_results_steer_the_next_run() {
    let dir = Workdir::new("settings-results");
    dir.ok(&[
        "run",
        "-s",
        "s",
        "-e",
        "SET BLANKS=9 EPOCH=1900 FUZZBITS=0 SCALEMIN=2.",
    ]);
    let text = std::fs::read_to_string(dir.0.join(".statscraft/sessions/s/settings.sps")).unwrap();
    assert!(
        text.contains("SET BLANKS='9' EPOCH='1900' FUZZBITS='0' SCALEMIN='2'.\n"),
        "{text}"
    );
    let data = "DATA LIST FIXED /x 1-2 d 4-11 (ADATE).\nBEGIN DATA\n 1 1/1/05\n\n 2 1/1/05\n\
                END DATA.\nCOMPUTE r = RND(0.285 * 100).";
    dir.ok(&["run", "-s", "s", "-e", data]);
    let saved = dir.0.join(".statscraft/sessions/s/data.zsav");
    let saved = statscraft_data::Dataset::read_sav(&saved).unwrap();
    assert_eq!(numbers(&saved, "x"), [Some(1.0), Some(9.0), Some(2.0)]);
    // 1 January 1905, in seconds from 14 October 1582
    assert_eq!(numbers(&saved, "d")[0], Some(117_687.0 * 86_400.0));
    assert_eq!(numbers(&saved, "r"), [Some(28.0); 3]);
    // x has three different values below 10: nominal by default, scale with SCALEMIN=2
    let description = dir.json(&["describe", "-s", "s", "--json"]);
    assert_eq!(
        description["variables"][0]["measure"], "scale",
        "{description}"
    );
}

#[test]
fn a_failed_run_or_a_check_keeps_no_settings() {
    let dir = Workdir::new("settings-atomic");
    dir.ok(&["run", "-s", "s", "-e", "SET FORMAT=F6.1."]);
    let file = dir.0.join(".statscraft/sessions/s/settings.sps");
    let before = std::fs::read_to_string(&file).unwrap();
    let failed = "SET FORMAT=F3.0.\nPRESERVE.\nFREQUENCIES x.";
    assert!(!dir.run(&["run", "-s", "s", "-e", failed]).status.success());
    dir.ok(&["check", "-s", "s", "-e", "SET FORMAT=F2.0.\nCD '.'."]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let report = dir.json(&["run", "-s", "s", "--json", "-e", "SHOW FORMAT."]);
    assert_eq!(shown(&report, 0), [pair("FORMAT", "F6.1")]);
    assert_eq!(report["saved"], false);
}

#[test]
fn named_datasets_keep_their_split_filter_and_declared_names_between_runs() {
    let dir = Workdir::new("datasets-state");
    let data = "DATA LIST LIST /g x.\nBEGIN DATA\n1 1\n1 0\n2 1\nEND DATA.\n\
                SPLIT FILE BY g.\nFILTER BY x.\nDATASET NAME first.\nDATASET DECLARE later.\n\
                DATA LIST LIST /y.\nBEGIN DATA\n5\nEND DATA.";
    dir.ok(&["run", "-s", "d", "-e", data]);
    let description = dir.json(&["describe", "-s", "d", "--json"]);
    assert_eq!(
        description["session"]["datasets"],
        serde_json::json!(["later", "first"])
    );
    let show = "DATASET ACTIVATE first.\nSHOW SPLIT FILTER.";
    let report = dir.json(&["run", "-s", "d", "--json", "-e", show]);
    assert_eq!(
        shown(&report, 1),
        [pair("SPLIT", "Split by g"), pair("FILTER", "Filtered by x")]
    );
}

#[test]
fn temporary_ends_with_its_run_and_random_numbers_go_on() {
    let dir = Workdir::new("temporary");
    let get = get_fixture("survey_basic.sav");
    dir.ok(&["run", "-s", "t", "-e", &get]);
    let sample = "TEMPORARY.\nSAMPLE .5.";
    let report = dir.json(&["run", "-s", "t", "--json", "-e", sample]);
    // the sample was temporary; the random numbers it drew changed the session
    assert_eq!(report["dataset"][0], 200);
    assert_eq!(report["saved"], true);
    let state = dir.0.join(".statscraft/sessions/t/state.json");
    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(state).unwrap()).unwrap();
    assert_ne!(
        state["random"]["seed"].as_u64().unwrap(),
        2_000_000,
        "{state}"
    );
    // two samples in two runs keep the cases they keep in one run
    let split = dir.json(&["run", "-s", "t", "--json", "-e", "SAMPLE .5."]);
    let one_run = format!("{get}\n{sample}\nEXECUTE.\nSAMPLE .5.");
    let whole = dir.json(&["run", "--json", "-e", &one_run]);
    assert_eq!(split["dataset"], whole["dataset"]);
}
