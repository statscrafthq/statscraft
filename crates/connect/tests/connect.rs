//! Every test works in its own temporary home folder; none reads the real user's files or runs
//! the real `claude` command.

use serde_json::{Value, json};
use statscraft_connect::{Connector, Os, Places, Runner};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const SERVER: &str = "/opt/statscraft/bin/statscraft";

fn temp_home() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "statscraft-connect-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn places(home: &Path, os: Os) -> Places {
    Places {
        os,
        home: home.to_path_buf(),
        appdata: None,
        local_appdata: None,
        xdg_config: None,
        codex_home: None,
        copilot_home: None,
        claude_config_dir: None,
        path: Vec::new(),
    }
}

fn no_commands() -> Runner {
    Box::new(|program, _| panic!("no command may run here, asked for {}", program.display()))
}

fn linux(home: &Path) -> Connector {
    Connector::new(places(home, Os::Linux), no_commands())
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn desktop_file(home: &Path) -> PathBuf {
    home.join(".config/Claude/claude_desktop_config.json")
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn backup(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.statscraft-backup", path.display()))
}

#[test]
fn connect_merges_into_the_desktop_config_and_keeps_other_keys_and_servers() {
    let home = temp_home();
    let file = desktop_file(&home);
    write(
        &file,
        r#"{"theme":"dark","mcpServers":{"notes":{"command":"notes-server"}},"zoom":1.25}"#,
    );
    let out = linux(&home)
        .connect("claude-desktop", Path::new(SERVER))
        .unwrap();
    assert!(out.changed);
    assert_eq!(out.files, vec![fs::canonicalize(&file).unwrap()]);
    assert!(out.restart.unwrap().contains("quit Claude Desktop"));
    let config = read_json(&file);
    assert_eq!(config["theme"], "dark");
    assert_eq!(config["zoom"], 1.25);
    assert_eq!(
        config["mcpServers"]["notes"],
        json!({"command": "notes-server"})
    );
    assert_eq!(
        config["mcpServers"]["statscraft"],
        json!({"command": SERVER, "args": ["mcp"]})
    );
    // Key order stays as the file had it, our server goes last.
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.find("theme").unwrap() < text.find("mcpServers").unwrap());
    assert!(text.find("mcpServers").unwrap() < text.find("zoom").unwrap());
    assert!(text.find("notes").unwrap() < text.find("statscraft").unwrap());
}

#[test]
fn second_connect_changes_nothing() {
    let home = temp_home();
    let file = home.join(".cursor/mcp.json");
    write(&file, "{}\n");
    let connector = linux(&home);
    assert!(
        connector
            .connect("cursor", Path::new(SERVER))
            .unwrap()
            .changed
    );
    let first = fs::read_to_string(&file).unwrap();
    let again = connector.connect("cursor", Path::new(SERVER)).unwrap();
    assert!(!again.changed);
    assert!(again.files.is_empty() && again.restart.is_none());
    assert_eq!(fs::read_to_string(&file).unwrap(), first);
    assert_eq!(
        read_json(&file)["mcpServers"]["statscraft"],
        json!({"type": "stdio", "command": SERVER, "args": ["mcp"]})
    );
}

#[test]
fn connect_updates_our_entry_and_keeps_fields_the_user_added() {
    let home = temp_home();
    let file = home.join(".gemini/settings.json");
    write(
        &file,
        r#"{"mcpServers":{"statscraft":{"command":"/old/statscraft","args":["mcp"],"env":{"LANG":"et_EE.UTF-8"}}}}"#,
    );
    let out = linux(&home).connect("gemini", Path::new(SERVER)).unwrap();
    assert!(out.changed);
    assert_eq!(
        read_json(&file)["mcpServers"]["statscraft"],
        json!({"command": SERVER, "args": ["mcp"], "env": {"LANG": "et_EE.UTF-8"}})
    );
}

#[test]
fn disconnect_removes_only_our_entry() {
    let home = temp_home();
    let file = home.join(".copilot/mcp-config.json");
    write(
        &file,
        r#"{"mcpServers":{"notes":{"command":"notes-server"}},"other":true}"#,
    );
    let connector = linux(&home);
    connector.connect("copilot", Path::new(SERVER)).unwrap();
    assert_eq!(
        read_json(&file)["mcpServers"]["statscraft"]["tools"],
        json!(["*"])
    );
    let out = connector.disconnect("copilot").unwrap();
    assert!(out.changed);
    assert_eq!(
        read_json(&file),
        json!({"mcpServers": {"notes": {"command": "notes-server"}}, "other": true})
    );
    let again = connector.disconnect("copilot").unwrap();
    assert!(!again.changed);
}

#[test]
fn a_file_that_does_not_parse_is_refused_and_left_untouched() {
    let home = temp_home();
    let file = home.join(".gemini/settings.json");
    let text = "{\n  // my settings\n  \"theme\": \"light\"\n}\n";
    write(&file, text);
    let connector = linux(&home);
    let err = connector.connect("gemini", Path::new(SERVER)).unwrap_err();
    assert!(
        err.contains("settings.json") && err.contains("unchanged"),
        "{err}"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), text);
    assert!(!backup(&file).exists());
    let gemini = connector
        .clients(Path::new(SERVER))
        .into_iter()
        .find(|c| c.id == "gemini")
        .unwrap();
    assert!(gemini.detected && gemini.problem.is_some());
}

#[test]
fn the_backup_is_written_once_before_the_first_change() {
    let home = temp_home();
    let file = desktop_file(&home);
    let original = r#"{"theme":"dark"}"#;
    write(&file, original);
    let connector = linux(&home);
    let out = connector
        .connect("claude-desktop", Path::new(SERVER))
        .unwrap();
    assert_eq!(out.backups, vec![backup(&fs::canonicalize(&file).unwrap())]);
    assert_eq!(fs::read_to_string(backup(&file)).unwrap(), original);
    let out = connector
        .connect("claude-desktop", Path::new("/elsewhere/statscraft"))
        .unwrap();
    assert!(out.changed && out.backups.is_empty());
    connector.disconnect("claude-desktop").unwrap();
    assert_eq!(fs::read_to_string(backup(&file)).unwrap(), original);
}

#[test]
fn toml_comments_and_order_stay() {
    let home = temp_home();
    let file = home.join(".codex/config.toml");
    let text = "# my settings\nmodel = \"gpt\" # the default\n\n[mcp_servers.notes]\ncommand = \"notes-server\"\n\n[profiles.work]\nmodel = \"other\"\n";
    write(&file, text);
    let connector = linux(&home);
    let out = connector.connect("codex", Path::new(SERVER)).unwrap();
    assert!(out.changed);
    // Our table goes next to the other server.
    let ours = format!("[mcp_servers.statscraft]\ncommand = \"{SERVER}\"\nargs = [\"mcp\"]\n\n");
    let expected = text.replace("[profiles.work]", &format!("{ours}[profiles.work]"));
    assert_eq!(fs::read_to_string(&file).unwrap(), expected);
    assert!(
        !connector
            .connect("codex", Path::new(SERVER))
            .unwrap()
            .changed
    );
    connector.disconnect("codex").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn toml_file_is_created_when_codex_is_installed() {
    let home = temp_home();
    fs::create_dir_all(home.join(".codex")).unwrap();
    linux(&home).connect("chatgpt", Path::new(SERVER)).unwrap();
    let written = fs::read_to_string(home.join(".codex/config.toml")).unwrap();
    let doc: toml_edit::DocumentMut = written.parse().unwrap();
    assert_eq!(
        doc["mcp_servers"]["statscraft"]["command"].as_str(),
        Some(SERVER)
    );
}

#[test]
fn windows_paths_are_escaped_and_both_desktop_folders_are_written() {
    let home = temp_home();
    let server = r"C:\Program Files\Statscraft\statscraft.exe";
    let mut places = places(&home, Os::Windows);
    places.appdata = Some(home.join("Roaming"));
    places.local_appdata = Some(home.join("Local"));
    let classic = home.join("Roaming/Claude");
    let store = home.join("Local/Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude");
    fs::create_dir_all(&classic).unwrap();
    fs::create_dir_all(&store).unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    let connector = Connector::new(places, no_commands());

    let out = connector
        .connect("claude-desktop", Path::new(server))
        .unwrap();
    assert_eq!(out.files.len(), 2);
    for folder in [&classic, &store] {
        let file = folder.join("claude_desktop_config.json");
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            text.contains(r#""C:\\Program Files\\Statscraft\\statscraft.exe""#),
            "{text}"
        );
        assert_eq!(
            read_json(&file)["mcpServers"]["statscraft"]["command"],
            server
        );
    }

    connector.connect("codex", Path::new(server)).unwrap();
    let doc: toml_edit::DocumentMut = fs::read_to_string(home.join(".codex/config.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        doc["mcp_servers"]["statscraft"]["command"].as_str(),
        Some(server)
    );

    let status = connector.clients(Path::new(server));
    let desktop = status.iter().find(|c| c.id == "claude-desktop").unwrap();
    assert!(desktop.detected && desktop.connected && desktop.points_here);
    assert_eq!(desktop.configs.len(), 2);
}

#[test]
fn line_endings_indentation_and_byte_order_mark_stay() {
    let home = temp_home();
    let file = home.join(".cursor/mcp.json");
    write(&file, "\u{feff}{\r\n\t\"mcpServers\": {}\r\n}\r\n");
    linux(&home).connect("cursor", Path::new(SERVER)).unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.starts_with("\u{feff}{\r\n\t\"mcpServers\": {\r\n\t\t\"statscraft\""),
        "{text:?}"
    );
    assert!(!text.replace("\r\n", "").contains('\n'), "{text:?}");
    assert!(text.ends_with("}\r\n"));
}

#[test]
fn nothing_is_created_for_an_agent_that_is_not_installed() {
    let home = temp_home();
    let connector = linux(&home);
    let err = connector.connect("cursor", Path::new(SERVER)).unwrap_err();
    assert!(err.contains("not found"), "{err}");
    assert!(!home.join(".cursor").exists());
    assert!(!connector.disconnect("cursor").unwrap().changed);
    assert!(!home.join(".cursor").exists());
    assert!(connector.connect("nonsense", Path::new(SERVER)).is_err());
    let status = connector.clients(Path::new(SERVER));
    assert_eq!(status.len(), 7);
    assert!(status.iter().all(|c| !c.detected && !c.connected));
}

#[test]
fn a_missing_file_is_created_when_the_agent_is_installed() {
    let home = temp_home();
    fs::create_dir_all(home.join(".gemini/antigravity")).unwrap();
    let out = linux(&home)
        .connect("antigravity", Path::new(SERVER))
        .unwrap();
    assert!(out.changed && out.backups.is_empty());
    let file = home.join(".gemini/config/mcp_config.json");
    assert_eq!(
        read_json(&file),
        json!({"mcpServers": {"statscraft": {"command": SERVER, "args": ["mcp"]}}})
    );
}

#[test]
fn claude_code_goes_through_its_own_command() {
    let home = temp_home();
    let bin = home.join("bin");
    write(&bin.join("claude"), "");
    let mut places = places(&home, Os::Linux);
    places.path = vec![bin.clone()];
    let calls: Rc<RefCell<Vec<String>>> = Rc::default();
    let log = calls.clone();
    let config = home.join(".claude.json");
    // Stands in for the real command: records the call and keeps the list in .claude.json.
    let runner: Runner = Box::new(move |program, args| {
        assert_eq!(program, bin.join("claude"));
        log.borrow_mut().push(args.join(" "));
        let mut root = fs::read_to_string(&config)
            .map(|t| serde_json::from_str(&t).unwrap())
            .unwrap_or(json!({"mcpServers": {}}));
        match args[1] {
            "add" => {
                root["mcpServers"]["statscraft"] = json!({"command": args[6], "args": [args[7]]})
            }
            "remove" => {
                root["mcpServers"]
                    .as_object_mut()
                    .unwrap()
                    .remove("statscraft");
            }
            _ => unreachable!(),
        }
        fs::write(&config, root.to_string()).unwrap();
        Ok((true, String::new()))
    });
    let connector = Connector::new(places, runner);

    let out = connector.connect("claude-code", Path::new(SERVER)).unwrap();
    assert!(out.changed && out.files.is_empty());
    assert_eq!(
        out.command.as_deref(),
        Some(format!("claude mcp add --scope user statscraft -- {SERVER} mcp").as_str())
    );
    assert!(
        !connector
            .connect("claude-code", Path::new(SERVER))
            .unwrap()
            .changed
    );
    let status = connector.clients(Path::new(SERVER));
    let code = status.iter().find(|c| c.id == "claude-code").unwrap();
    assert!(code.detected && code.connected && code.points_here);

    connector
        .connect("claude-code", Path::new("/new/statscraft"))
        .unwrap();
    connector.disconnect("claude-code").unwrap();
    assert!(!connector.disconnect("claude-code").unwrap().changed);
    assert_eq!(
        *calls.borrow(),
        [
            format!("mcp add --scope user statscraft -- {SERVER} mcp"),
            "mcp remove --scope user statscraft".to_string(),
            "mcp add --scope user statscraft -- /new/statscraft mcp".to_string(),
            "mcp remove --scope user statscraft".to_string(),
        ]
    );
}

#[test]
fn a_failing_claude_command_is_reported() {
    let home = temp_home();
    write(&home.join("bin/claude"), "");
    let mut places = places(&home, Os::Linux);
    places.path = vec![home.join("bin")];
    let connector = Connector::new(places, Box::new(|_, _| Ok((false, "not logged in".into()))));
    let err = connector
        .connect("claude-code", Path::new(SERVER))
        .unwrap_err();
    assert!(err.contains("not logged in"), "{err}");
}
