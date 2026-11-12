//! `statscraft connect` with HOME in a temporary folder and an empty PATH, so the test never
//! touches the real user's agents.
#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn connect(home: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_statscraft"))
        .arg("connect")
        .args(args)
        .arg("--json")
        .env("HOME", home)
        .env("PATH", home.join("no-bin"))
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("CODEX_HOME")
        .env_remove("COPILOT_HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    (out.status.code(), serde_json::from_str(&text).expect(&text))
}

#[test]
fn list_connect_and_remove_in_a_temporary_home() {
    let home: PathBuf =
        std::env::temp_dir().join(format!("statscraft-cli-connect-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    let cursor = home.join(".cursor/mcp.json");
    fs::create_dir_all(cursor.parent().unwrap()).unwrap();
    fs::write(&cursor, "{\"mcpServers\":{}}").unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    let server = fs::canonicalize(env!("CARGO_BIN_EXE_statscraft")).unwrap();

    let (code, list) = connect(&home, &["--list"]);
    assert_eq!(code, Some(0));
    assert_eq!(list["server"], server.to_str().unwrap());
    let detected: Vec<&str> = list["clients"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["detected"] == true)
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(detected, ["codex", "cursor"]);
    assert_eq!(fs::read_to_string(&cursor).unwrap(), "{\"mcpServers\":{}}");
    assert!(!home.join(".codex/config.toml").exists());

    let (code, done) = connect(&home, &[]);
    assert_eq!(code, Some(0), "{done}");
    assert_eq!(done["results"].as_array().unwrap().len(), 2);
    assert!(
        done["results"][1]["restart"]
            .as_str()
            .unwrap()
            .contains("Cursor")
    );
    let config: Value = serde_json::from_str(&fs::read_to_string(&cursor).unwrap()).unwrap();
    assert_eq!(
        config["mcpServers"]["statscraft"]["command"],
        server.to_str().unwrap()
    );

    let (code, again) = connect(&home, &["cursor"]);
    assert_eq!(code, Some(0));
    assert_eq!(again["results"][0]["changed"], false);

    let (code, unknown) = connect(&home, &["nonsense"]);
    assert_eq!(code, Some(1));
    assert_eq!(unknown["ok"], false);

    let (code, removed) = connect(&home, &["--remove"]);
    assert_eq!(code, Some(0), "{removed}");
    let config: Value = serde_json::from_str(&fs::read_to_string(&cursor).unwrap()).unwrap();
    assert_eq!(config, serde_json::json!({"mcpServers": {}}));
    let _ = fs::remove_dir_all(&home);
}
