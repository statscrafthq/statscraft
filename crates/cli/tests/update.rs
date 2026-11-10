//! `statscraft update --check` against a local server named in STATSCRAFT_UPDATE_URL, so the
//! test never calls GitHub.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::Command;

/// Answers every request with `status` and `body`; returns the base URL.
fn serve(status: u16, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 && !line.ends_with("\r\n\r\n") {}
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body.as_bytes());
        }
    });
    base
}

fn check(base: &str, json: bool) -> (Option<i32>, String) {
    let mut args = vec!["update", "--check"];
    if json {
        args.push("--json");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_statscraft"))
        .args(args)
        .env("STATSCRAFT_UPDATE_URL", base)
        .output()
        .unwrap();
    (out.status.code(), String::from_utf8(out.stdout).unwrap())
}

#[test]
fn update_check_reports_a_newer_release_with_exit_code_10() {
    let base = serve(
        200,
        r#"{"tag_name": "v99.0.0", "html_url": "https://example.org/notes", "assets": []}"#,
    );
    let (code, out) = check(&base, true);
    assert_eq!(code, Some(10), "{out}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["latest"], "99.0.0");
    assert_eq!(report["update"], true);
    assert_eq!(report["notes"], "https://example.org/notes");
    assert_eq!(report["current"], env!("CARGO_PKG_VERSION"));
    let (code, out) = check(&base, false);
    assert_eq!(code, Some(10));
    assert!(out.contains("latest: 99.0.0"), "{out}");
}

#[test]
fn update_check_without_a_published_release_is_up_to_date() {
    let base = serve(404, r#"{"message": "Not Found"}"#);
    let (code, out) = check(&base, true);
    assert_eq!(code, Some(0), "{out}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["latest"], serde_json::Value::Null);
    assert_eq!(report["update"], false);
    let (_, out) = check(&base, false);
    assert!(out.contains("no published release yet"), "{out}");
}
