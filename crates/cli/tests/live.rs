//! `--session gui` end to end without a display: the window's half (a listener answering with a
//! `Session`, as the window does) runs in this process, the real command line drives it.

use statscraft_engine::live::{self, Action, Request};
use statscraft_engine::session::Session;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Workdir(PathBuf);

impl Workdir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("statscraft-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Where `endpoint_file()` points for the command line run by `cli`.
    fn endpoint(&self) -> PathBuf {
        self.0.join("runtime/statscraft/gui.json")
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statscraft"))
            .args(args)
            .current_dir(&self.0)
            .env("XDG_RUNTIME_DIR", self.0.join("runtime"))
            .output()
            .unwrap()
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn start_window(file: &Path) -> live::Server {
    let mut session = Session::default();
    live::serve(file.to_path_buf(), move |request| {
        live::answer(&mut session, &request)
    })
    .unwrap()
}

/// The output without the line that says where the syntax was kept: a file session keeps it in
/// its folder, and the window of these tests keeps none.
fn without_trail(out: &str) -> String {
    out.lines()
        .filter(|line| !line.contains("\"trail\": "))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn text(out: &Output) -> (bool, String, String) {
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_window_answers_exactly_like_a_file_session() {
    let dir = Workdir::new("same");
    let server = start_window(&dir.endpoint());
    // a relative path: the window runs in the caller's folder
    let fixture = format!(
        "{}/../../tests/fixtures/survey_basic.sav",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::copy(fixture, dir.0.join("survey.sav")).unwrap();
    let steps: Vec<Vec<&str>> = vec![
        vec!["run", "--json", "-e", "GET FILE='survey.sav'."],
        vec!["describe", "--json"],
        vec!["describe"],
        vec![
            "run",
            "--json",
            "-e",
            "FREQUENCIES gender.\nCOMPUTE x = age * 2.",
        ],
        vec![
            "run",
            "-e",
            "CROSSTABS gender BY region.\nVARIABLE LABELS x 'Twice'.",
        ],
        vec!["check", "--json", "-e", "COMPUTE y = 1."],
        vec!["check", "-e", "COMPUTE y = 1."],
        vec![
            "run",
            "--json",
            "--lines",
            "2-2",
            "-e",
            "COMPUTE a = 1.\nCOMPUTE b = 2.",
        ],
        vec!["run", "--json", "-e", "COMPUTE ok = 1.\nFREQUENCIES nope."],
        vec!["run", "-e", "COMPUTE bad = ."],
        vec!["run", "--json", "-e", "FREQUENCEIS gender."],
    ];
    for step in &steps {
        let with = |session: &str| {
            let mut args = vec![step[0], "-s", session];
            args.extend(&step[1..]);
            text(&dir.cli(&args))
        };
        let file = with("t1");
        // a file session keeps its data as .zsav; the window keeps the file as it was read
        let file = (
            file.0,
            without_trail(&file.1)
                .replace("\"compression\": \"zlib\"", "\"compression\": \"none\""),
            file.2,
        );
        let window = with("gui");
        let window = (
            window.0,
            window
                .1
                .replace("\"session\": \"gui\"", "\"session\": \"t1\""),
            window.2,
        );
        assert_eq!(window, file, "{step:?}");
    }
    let report: serde_json::Value = serde_json::from_slice(
        &dir.cli(&["run", "-s", "gui", "--json", "-e", "COMPUTE z = 3."])
            .stdout,
    )
    .unwrap();
    assert_eq!(report["session"], "gui");
    assert_eq!(report["saved"], true);

    drop(server);
    assert!(
        !dir.endpoint().exists(),
        "the endpoint file goes with the window"
    );
    let (ok, _, stderr) = text(&dir.cli(&["run", "-s", "gui", "-e", "FREQUENCIES gender."]));
    assert!(!ok);
    assert!(
        stderr.contains("no Statscraft window is running"),
        "{stderr}"
    );
}

#[test]
fn a_request_without_the_token_is_refused() {
    let dir = Workdir::new("token");
    let _server = start_window(&dir.endpoint());
    let endpoint: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.endpoint()).unwrap()).unwrap();
    let address = endpoint["address"].as_str().unwrap();
    assert!(address.starts_with("127.0.0.1:"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.endpoint())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let request = Request {
        token: "guess".into(),
        action: Action::Describe,
        json: true,
        ..Request::default()
    };
    let stream = std::net::TcpStream::connect(address).unwrap();
    writeln!(&stream, "{}", serde_json::to_string(&request).unwrap()).unwrap();
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply).unwrap();
    assert!(reply.contains("refused: wrong token"), "{reply}");

    // the client reads the token from the file
    let reply = live::send(&dir.endpoint(), request);
    assert_eq!(
        reply.unwrap_err()[0].message,
        "session gui has no dataset yet"
    );
}

/// The parts of a workbook, without docProps/core.xml, which holds the time it was written.
fn workbook_parts(path: &Path) -> Vec<(String, String)> {
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

#[test]
fn the_window_run_writes_the_same_workbook() {
    let dir = Workdir::new("xlsx");
    let _server = start_window(&dir.endpoint());
    let fixture = format!(
        "{}/../../tests/fixtures/survey_basic.sav",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::copy(fixture, dir.0.join("survey.sav")).unwrap();
    for session in ["t1", "gui"] {
        let out = dir.cli(&["run", "-s", session, "-e", "GET FILE='survey.sav'."]);
        assert!(out.status.success());
    }
    let syntax = "FREQUENCIES gender.\nCTABLES /TABLE region BY gender.";
    for json in [false, true] {
        let with = |session: &str| {
            let file = format!("{session}.xlsx");
            let mut args = vec!["run", "-s", session, "--xlsx", &file];
            if json {
                args.push("--json");
            }
            args.extend(["-e", syntax]);
            let (ok, out, err) = text(&dir.cli(&args));
            let out = without_trail(&out).replace("\"session\": \"gui\"", "\"session\": \"t1\"");
            (ok, out, err, workbook_parts(&dir.0.join(&file)))
        };
        let file = with("t1");
        let window = with("gui");
        assert!(file.0, "{}", file.2);
        assert_eq!(window, file, "json {json}");
    }
}

#[test]
fn runs_sent_to_the_window_leave_their_syntax_and_file_sessions_keep_theirs() {
    let dir = Workdir::new("trail");
    let mut desk = live::Desk::default();
    let _server = live::serve(dir.endpoint(), move |request| desk.answer(&request)).unwrap();
    let run = |session: &str, syntax: &str| {
        let out = dir.cli(&["run", "-s", session, "--json", "-e", syntax]);
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
    };
    assert_eq!(run("gui", "SET SEED=11.")["trail"], "syntax window Agent");
    assert_eq!(run("gui", "SET SEDE=1.")["ok"], false);
    let windows = live::send(
        &dir.endpoint(),
        Request {
            action: Action::Read,
            window: Some("Agent".into()),
            ..Request::default()
        },
    )
    .unwrap();
    let windows: serde_json::Value = serde_json::from_str(&windows).unwrap();
    let text = windows["text"].as_str().unwrap();
    assert!(
        text.starts_with("1  * Agent ") && text.ends_with("\n2  SET SEED=11.\n"),
        "{text}"
    );

    // a file session keeps every successful run in syntax.sps beside its data, with the date
    let first = run("t1", "SET SEED=11.");
    let trail = dir.0.join(".statscraft/sessions/t1/syntax.sps");
    assert_eq!(first["trail"], trail.to_str().unwrap());
    assert_eq!(run("t1", "SET SEDE=1.")["ok"], false);
    let out = dir.cli(&[
        "run",
        "-s",
        "t1",
        "--lines",
        "2-2",
        "-e",
        "SHOW SEED.\nSET SEED=12.",
    ]);
    assert!(out.status.success());
    let kept = std::fs::read_to_string(&trail).unwrap();
    let blocks: Vec<&str> = kept.split("\n\n").collect();
    assert_eq!(blocks.len(), 2, "{kept}");
    let stamp = blocks[0].lines().next().unwrap();
    assert!(stamp.starts_with("* Agent 20") && stamp.len() == "* Agent 2026-11-12 14:32.".len());
    assert!(blocks[0].ends_with("\nSET SEED=11."));
    assert!(blocks[1].ends_with("\nSET SEED=12.\n"));
    // a check keeps nothing
    dir.cli(&["check", "-s", "t1", "-e", "SET SEED=13."]);
    assert_eq!(std::fs::read_to_string(&trail).unwrap(), kept);
}

/// A window from before a request existed answers "bad request: unknown variant"; the client
/// says to update instead of passing that on.
#[test]
fn an_older_window_asks_for_an_update() {
    let dir = Workdir::new("older");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = serde_json::json!({ "address": listener.local_addr().unwrap(), "token": "t" });
    std::fs::create_dir_all(dir.endpoint().parent().unwrap()).unwrap();
    std::fs::write(dir.endpoint(), endpoint.to_string()).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        let old = r#"{"Err":[{"kind":"session","message":"bad request: unknown variant `windows`, expected one of `run`, `check`, `describe`"}]}"#;
        writeln!(&stream, "{old}").unwrap();
    });
    let request = Request {
        action: Action::Windows,
        ..Request::default()
    };
    let errors = live::send(&dir.endpoint(), request).unwrap_err();
    assert!(
        errors[0].message.contains("update Statscraft"),
        "{errors:?}"
    );
}
