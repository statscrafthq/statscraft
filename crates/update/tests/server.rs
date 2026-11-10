//! The updater against a tiny HTTP server in the test, so no test calls GitHub.

use sha2::{Digest, Sha256};
use statscraft_update::{Client, Release};
// only the tarball tests use these, and they need tar, so they run on Unix only
#[cfg(unix)]
use statscraft_update::{Done, Install};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

/// Serves `routes` (path to status and body) on a random local port; returns its base URL.
fn serve(routes: impl FnOnce(&str) -> HashMap<String, (u16, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = routes(&base);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            // skip the headers up to the blank line
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
                    break;
                }
            }
            let (status, body) = routes.get(&path).cloned().unwrap_or((404, b"{}".to_vec()));
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    base
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn release_json(base: &str, names: &[&str]) -> Vec<u8> {
    let assets: Vec<_> = names
        .iter()
        .map(|n| serde_json::json!({"name": n, "browser_download_url": format!("{base}/dl/{n}")}))
        .collect();
    serde_json::json!({
        "tag_name": "v9.9.9",
        "html_url": format!("{base}/notes"),
        "draft": false,
        "prerelease": false,
        "assets": assets,
    })
    .to_string()
    .into_bytes()
}

const LATEST: &str = "/repos/statscrafthq/statscraft/releases/latest";

fn temp(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-update-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn no_published_release_is_no_update() {
    let base = serve(|_| HashMap::new());
    assert_eq!(Client::at(&base).latest().unwrap(), None);
}

#[test]
fn a_download_is_kept_only_when_it_matches_sha256sums() {
    let deb = b"a package".to_vec();
    let good = "statscraft_9.9.9_amd64.deb";
    let bad = "statscraft-9.9.9-windows-x86_64-setup.exe";
    let sums = format!("{}  {good}\n{}  {bad}\n", sha256(&deb), sha256(b"other"));
    let base = serve(|base| {
        HashMap::from([
            (
                LATEST.into(),
                (200, release_json(base, &["SHA256SUMS", good, bad])),
            ),
            ("/dl/SHA256SUMS".into(), (200, sums.into_bytes())),
            (format!("/dl/{good}"), (200, deb.clone())),
            (format!("/dl/{bad}"), (200, b"tampered".to_vec())),
        ])
    });
    let client = Client::at(&base);
    let release = client.latest().unwrap().unwrap();
    assert_eq!(release.version.to_string(), "9.9.9");
    assert!(release.is_newer());
    assert_eq!(release.notes, format!("{base}/notes"));

    let dir = temp("download");
    let file = client.download(&release, good, &dir).unwrap();
    assert_eq!(file, dir.join(good));
    assert_eq!(std::fs::read(&file).unwrap(), deb);

    let error = client.download(&release, bad, &dir).unwrap_err();
    assert!(error.contains("does not match SHA256SUMS"), "{error}");
    // the mismatching file is gone, part file included
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);

    // a file that SHA256SUMS does not list is refused before it is downloaded
    let release = Release {
        assets: [
            release.assets.clone(),
            vec![statscraft_update::Asset {
                name: "extra.tar.gz".into(),
                url: format!("{base}/dl/extra.tar.gz"),
            }],
        ]
        .concat(),
        ..release
    };
    let error = client.download(&release, "extra.tar.gz", &dir).unwrap_err();
    assert!(error.contains("does not list"), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// The tarball update from download to replaced programs. Needs tar, as the updater does.
#[cfg(unix)]
#[test]
fn a_tarball_install_gets_the_new_programs() {
    let root = temp("tarball");
    let top = "statscraft-9.9.9-linux-x86_64";
    let tree = root.join("pack").join(top).join("bin");
    std::fs::create_dir_all(&tree).unwrap();
    std::fs::write(tree.join("statscraft"), "new cli").unwrap();
    std::fs::write(tree.join("statscraft-gui"), "new gui").unwrap();
    let name = format!("{top}.tar.gz");
    let archive = root.join(&name);
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(root.join("pack"))
        .arg(top)
        .status()
        .unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&archive).unwrap();
    let sums = format!("{}  {name}\n", sha256(&bytes));
    let base = serve(|base| {
        HashMap::from([
            (
                LATEST.into(),
                (200, release_json(base, &["SHA256SUMS", &name])),
            ),
            ("/dl/SHA256SUMS".into(), (200, sums.into_bytes())),
            (format!("/dl/{name}"), (200, bytes)),
        ])
    });

    let bin = root.join("prefix/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("statscraft"), "old cli").unwrap();
    std::fs::write(bin.join("statscraft-gui"), "old gui").unwrap();
    let client = Client::at(&base);
    let release = client.latest().unwrap().unwrap();
    let done = client
        .update(&release, &Install::Tarball(bin.clone()), false)
        .unwrap();
    assert_eq!(done, Done::Replaced(bin.clone()));
    assert_eq!(
        std::fs::read_to_string(bin.join("statscraft")).unwrap(),
        "new cli"
    );
    assert_eq!(
        std::fs::read_to_string(bin.join("statscraft-gui")).unwrap(),
        "new gui"
    );
    assert_eq!(std::fs::read_dir(&bin).unwrap().count(), 2);
    assert!(!std::env::temp_dir().join("statscraft-update").exists());

    // a copy that no package installed is refused
    let error = client
        .update(&release, &Install::Unmanaged(root.clone()), false)
        .unwrap_err();
    assert!(
        error.contains("update it the way you installed it"),
        "{error}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
