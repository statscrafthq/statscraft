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
            file.1
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
        stderr.contains("no StatsCraft window is running"),
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
        source: String::new(),
        lines: None,
        json: true,
        cwd: None,
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
