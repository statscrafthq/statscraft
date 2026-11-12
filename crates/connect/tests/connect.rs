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
        wsl: None,
        wsl_root: PathBuf::new(),
    }
}

/// Answers the question whether Claude Desktop runs with no; any other command fails the test.
fn no_commands() -> Runner {
    Box::new(|program, _| match program.to_str() {
        Some("pgrep" | "powershell") => Ok((false, String::new())),
        _ => panic!("no command may run here, asked for {}", program.display()),
    })
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

#[test]
fn plain_path_drops_only_the_local_prefix() {
    use statscraft_connect::plain_path;
    let plain = |s: &str| plain_path(PathBuf::from(s));
    assert_eq!(
        plain(r"\\?\C:\Programs\statscraft.exe"),
        PathBuf::from(r"C:\Programs\statscraft.exe")
    );
    // a network share keeps its long form: without the prefix it would be a different path
    assert_eq!(
        plain(r"\\?\UNC\server\share\statscraft.exe"),
        PathBuf::from(r"\\?\UNC\server\share\statscraft.exe")
    );
    assert_eq!(plain(SERVER), PathBuf::from(SERVER));
}

/// A connector whose process list shows `programs` (pgrep's answer, or the paths PowerShell gives).
fn with_processes(home: &Path, os: Os, running: &'static [&'static str]) -> Connector {
    let runner: Runner = Box::new(move |program, args| {
        let program = program.to_str().unwrap();
        Ok(match (program, os) {
            ("powershell", Os::Windows) => {
                assert!(args.last().unwrap().contains("Get-Process -Name claude"));
                let lines: Vec<String> = running.iter().map(|p| format!("process:{p}")).collect();
                (true, lines.join("\r\n"))
            }
            ("pgrep", Os::Mac) => (running.contains(&args[1]), String::new()),
            ("pgrep", Os::Linux) => (running.iter().any(|p| p.contains(args[2])), String::new()),
            _ => panic!("unexpected {program} on {os:?}"),
        })
    });
    Connector::new(places(home, os), runner)
}

#[test]
fn the_running_desktop_app_is_told_from_the_claude_code_program() {
    let home = temp_home();
    let windows = |running| with_processes(&home, Os::Windows, running).desktop_running();
    assert!(!windows(&[]));
    assert!(!windows(&[r"C:\Users\anna\.local\bin\claude.exe"]));
    assert!(windows(&[
        r"C:\Users\anna\.local\bin\claude.exe",
        r"C:\Program Files\WindowsApps\Claude_1.0.0_x64\app\claude.exe",
    ]));
    // a path the process list may not show still counts as the app
    assert!(windows(&[""]));
    assert!(with_processes(&home, Os::Mac, &["Claude"]).desktop_running());
    assert!(!with_processes(&home, Os::Mac, &["claude"]).desktop_running());
    assert!(with_processes(&home, Os::Linux, &["/opt/claude-desktop/electron"]).desktop_running());
    assert!(!with_processes(&home, Os::Linux, &[]).desktop_running());
    // no process list at all: nothing to warn about
    let broken = Connector::new(
        places(&home, Os::Linux),
        Box::new(|_, _| Err(std::io::Error::other("no pgrep"))),
    );
    assert!(!broken.desktop_running());
}

#[test]
fn a_change_while_the_desktop_app_runs_says_to_quit_it_first() {
    let home = temp_home();
    fs::create_dir_all(home.join(".config/Claude")).unwrap();
    let running = with_processes(&home, Os::Linux, &["claude-desktop"]);
    let out = running
        .connect("claude-desktop", Path::new(SERVER))
        .unwrap();
    assert_eq!(out.warning, Some(statscraft_connect::DESKTOP_RUNNING));
    // nothing changed, nothing to warn about
    let again = running
        .connect("claude-desktop", Path::new(SERVER))
        .unwrap();
    assert_eq!(again.warning, None);
    let quiet = linux(&home).disconnect("claude-desktop").unwrap();
    assert!(quiet.changed && quiet.warning.is_none());
}

#[test]
fn entry_in_reads_one_file_afresh() {
    use statscraft_connect::entry_in;
    let home = temp_home();
    let json = home.join("a.json");
    let toml = home.join("config.toml");
    assert_eq!(entry_in(&json), Ok(None));
    write(
        &json,
        r#"{"mcpServers":{"statscraft":{"command":"/x/statscraft"}}}"#,
    );
    assert_eq!(entry_in(&json), Ok(Some("/x/statscraft".into())));
    write(&json, r#"{"mcpServers":{}}"#);
    assert_eq!(entry_in(&json), Ok(None));
    write(
        &toml,
        "[mcp_servers.statscraft]\ncommand = \"/y/statscraft\"\n",
    );
    assert_eq!(entry_in(&toml), Ok(Some("/y/statscraft".into())));
    write(&json, "{ // comment\n}");
    assert!(entry_in(&json).is_err());
}

#[test]
fn codex_tells_where_the_tools_show_up() {
    use statscraft_connect::note;
    assert!(note("codex").unwrap().contains("Codex view"));
    assert_eq!(note("chatgpt"), note("codex"));
    assert_eq!(note("cursor"), None);
}

const WIN_SERVER: &str = r"C:\Users\anna\AppData\Local\Programs\StatsCraft\statscraft.exe";
const WSL_SERVER: &str = "/mnt/c/Users/anna/AppData/Local/Programs/StatsCraft/statscraft.exe";

/// A Windows computer with WSL: `wsl.exe` is a fake that lists Ubuntu and Docker Desktop's
/// distribution (in UTF-16, as older versions print it), answers the probe of Ubuntu and keeps
/// Claude Code's list in the fake home; `\\wsl$` is a folder in the temporary home. Every
/// command line wsl.exe gets is logged.
fn with_wsl(home: &Path, wslpath_works: bool) -> (Connector, PathBuf, Rc<RefCell<Vec<String>>>) {
    let root = home.join("wsl");
    let linux = root.join("Ubuntu/home/anna");
    fs::create_dir_all(&linux).unwrap();
    let wsl = home.join("System32/wsl.exe");
    write(&wsl, "");
    let mut places = places(home, Os::Windows);
    places.wsl = Some(wsl.clone());
    places.wsl_root = root;
    let calls: Rc<RefCell<Vec<String>>> = Rc::default();
    let log = calls.clone();
    let claude_json = linux.join(".claude.json");
    let runner: Runner = Box::new(move |program, args| {
        match program.to_str() {
            Some("powershell") => return Ok((true, String::new())),
            _ => assert_eq!(program, wsl),
        }
        log.borrow_mut().push(args.join(" "));
        if args == ["-l", "-q"] {
            let utf16: String = "\u{feff}Ubuntu\r\ndocker-desktop\r\n"
                .chars()
                .flat_map(|c| [c, '\0'])
                .collect();
            return Ok((true, utf16));
        }
        assert_eq!(&args[..5], ["-d", "Ubuntu", "-e", "sh", "-lc"]);
        let script = args[5];
        if script.starts_with("printf") {
            assert!(
                script.contains(&format!("wslpath -u '{WIN_SERVER}'")),
                "{script}"
            );
            let server = if wslpath_works { WSL_SERVER } else { "" };
            // a login script that prints a greeting
            return Ok((
                true,
                format!(
                    "Welcome!\nhome:/home/anna\nclaude:/home/anna/.local/bin/claude\nserver:{server}\n"
                ),
            ));
        }
        let mut root = fs::read_to_string(&claude_json)
            .map(|t| serde_json::from_str(&t).unwrap())
            .unwrap_or(json!({"mcpServers": {}}));
        if script.starts_with("claude 'mcp' 'add'") {
            let words: Vec<&str> = script.split(' ').collect();
            let unquote = |w: &str| w.trim_matches('\'').to_string();
            let env = |w: &str| {
                let (k, v) = w.trim_matches('\'').split_once('=').unwrap();
                (k.to_string(), Value::from(v))
            };
            root["mcpServers"]["statscraft"] = json!({
                "command": unquote(words[words.len() - 2]),
                "args": ["mcp"],
                "env": serde_json::Map::from_iter([env(words[7]), env(words[9])]),
            });
        } else {
            assert_eq!(
                script,
                "claude 'mcp' 'remove' '--scope' 'user' 'statscraft'"
            );
            root["mcpServers"]
                .as_object_mut()
                .unwrap()
                .remove("statscraft");
        }
        fs::write(&claude_json, root.to_string()).unwrap();
        Ok((true, String::new()))
    });
    (Connector::new(places, runner), linux, calls)
}

#[test]
fn agents_inside_wsl_are_listed_by_distribution() {
    let home = temp_home();
    let (connector, linux, calls) = with_wsl(&home, true);
    fs::create_dir_all(linux.join(".codex")).unwrap();
    fs::create_dir_all(linux.join(".gemini")).unwrap();
    let list = connector.clients(Path::new(WIN_SERVER));
    let wsl: Vec<(&str, &str)> = list
        .iter()
        .skip(7)
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();
    // Antigravity has no folder there, Docker Desktop's distribution is not asked
    assert_eq!(
        wsl,
        [
            ("claude-code-wsl:Ubuntu", "Claude Code (WSL: Ubuntu)"),
            ("codex-wsl:Ubuntu", "Codex (WSL: Ubuntu)"),
            ("gemini-wsl:Ubuntu", "Gemini CLI (WSL: Ubuntu)"),
        ]
    );
    assert!(list[7..].iter().all(|c| c.detected && !c.connected));
    assert_eq!(list[8].configs, [linux.join(".codex/config.toml")]);
    assert!(
        statscraft_connect::note("codex-wsl:Ubuntu")
            .unwrap()
            .contains("WSL")
    );
    assert_eq!(statscraft_connect::note("gemini-wsl:Ubuntu"), None);
    // WSL is asked once per connector, also when connecting afterwards
    connector
        .connect("codex-wsl:ubuntu", Path::new(WIN_SERVER))
        .unwrap();
    connector.clients(Path::new(WIN_SERVER));
    assert_eq!(calls.borrow().len(), 2);
    assert!(
        connector
            .connect("cursor-wsl:Ubuntu", Path::new(WIN_SERVER))
            .unwrap_err()
            .contains("does not run inside WSL")
    );
    assert!(
        connector
            .connect("codex-wsl:Debian", Path::new(WIN_SERVER))
            .unwrap_err()
            .contains("no WSL distribution named Debian")
    );
}

#[test]
fn codex_and_gemini_inside_wsl_start_the_windows_program_with_the_distribution() {
    let home = temp_home();
    let (connector, linux, _) = with_wsl(&home, true);
    let toml = linux.join(".codex/config.toml");
    // the entry the owner wrote by hand: right command, no env yet
    write(
        &toml,
        &format!(
            "model = \"x\"\n\n[mcp_servers.statscraft]\ncommand = \"{WSL_SERVER}\"\nargs = [\"mcp\"]\n"
        ),
    );
    let gemini = linux.join(".gemini/settings.json");
    write(
        &gemini,
        r#"{"mcpServers":{"statscraft":{"command":"old","env":{"LANG":"et_EE.UTF-8"}}}}"#,
    );

    let out = connector
        .connect("codex-wsl:Ubuntu", Path::new(WIN_SERVER))
        .unwrap();
    assert!(out.changed, "the env is added to a hand-written entry");
    assert_eq!(out.name, "Codex (WSL: Ubuntu)");
    assert_eq!(
        fs::read_to_string(&toml).unwrap(),
        format!(
            "model = \"x\"\n\n[mcp_servers.statscraft]\ncommand = \"{WSL_SERVER}\"\nargs = [\"mcp\"]\n\
             env = {{ STATSCRAFT_WSL_DISTRO = \"Ubuntu\", WSLENV = \"STATSCRAFT_WSL_DISTRO\" }}\n"
        )
    );
    let again = connector
        .connect("codex-wsl:Ubuntu", Path::new(WIN_SERVER))
        .unwrap();
    assert!(!again.changed);

    connector
        .connect("gemini-wsl:Ubuntu", Path::new(WIN_SERVER))
        .unwrap();
    assert_eq!(
        read_json(&gemini)["mcpServers"]["statscraft"],
        json!({"command": WSL_SERVER, "args": ["mcp"], "env": {
            "LANG": "et_EE.UTF-8",
            "STATSCRAFT_WSL_DISTRO": "Ubuntu",
            "WSLENV": "STATSCRAFT_WSL_DISTRO",
        }})
    );
    // files of the Linux side keep Linux line endings
    assert!(!fs::read_to_string(&gemini).unwrap().contains('\r'));

    let list = connector.clients(Path::new(WIN_SERVER));
    let codex = list.iter().find(|c| c.id == "codex-wsl:Ubuntu").unwrap();
    assert!(codex.connected && codex.points_here);
    // the Windows Codex is another agent, untouched
    assert!(!list.iter().find(|c| c.id == "codex").unwrap().connected);

    connector.disconnect("codex-wsl:Ubuntu").unwrap();
    assert_eq!(fs::read_to_string(&toml).unwrap(), "model = \"x\"\n");
}

#[test]
fn claude_code_inside_wsl_runs_its_command_there() {
    let home = temp_home();
    let (connector, _, calls) = with_wsl(&home, false);
    let out = connector
        .connect("claude-code-wsl:Ubuntu", Path::new(WIN_SERVER))
        .unwrap();
    // without wslpath the drive path is translated by rule
    let add = format!(
        "-d Ubuntu -e sh -lc claude 'mcp' 'add' '--scope' 'user' 'statscraft' '-e' \
         'STATSCRAFT_WSL_DISTRO=Ubuntu' '-e' 'WSLENV=STATSCRAFT_WSL_DISTRO' '--' '{WSL_SERVER}' 'mcp'"
    );
    assert_eq!(calls.borrow()[2], add);
    assert!(out.command.unwrap().ends_with("(in WSL: Ubuntu)"));
    let list = connector.clients(Path::new(WIN_SERVER));
    let code = list
        .iter()
        .find(|c| c.id == "claude-code-wsl:Ubuntu")
        .unwrap();
    assert!(code.connected && code.points_here);
    assert!(
        !connector
            .connect("claude-code-wsl:Ubuntu", Path::new(WIN_SERVER))
            .unwrap()
            .changed
    );
    assert!(
        connector
            .disconnect("claude-code-wsl:Ubuntu")
            .unwrap()
            .changed
    );
    assert_eq!(calls.borrow().len(), 4);
}

#[test]
fn a_failed_or_missing_wsl_lists_no_wsl_agents() {
    let home = temp_home();
    let mut places = places(&home, Os::Windows);
    places.wsl = Some(home.join("wsl.exe"));
    let silent = Connector::new(
        places.clone(),
        Box::new(|program, _| match program.to_str() {
            Some("powershell") => Ok((false, String::new())),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "no answer",
            )),
        }),
    );
    assert!(silent.looks_into_wsl());
    assert_eq!(silent.clients(Path::new(WIN_SERVER)).len(), 7);
    places.wsl = None;
    let none = Connector::new(places, no_commands());
    assert!(!none.looks_into_wsl());
    assert_eq!(none.clients(Path::new(WIN_SERVER)).len(), 7);
}

#[test]
fn paths_between_windows_and_wsl() {
    use statscraft_connect::{linux_to_windows, windows_to_linux};
    assert_eq!(windows_to_linux(WIN_SERVER), WSL_SERVER);
    assert_eq!(windows_to_linux(r"D:\"), "/mnt/d");
    assert_eq!(
        windows_to_linux(r"\\wsl$\Ubuntu\home\anna\a.sav"),
        "/home/anna/a.sav"
    );
    assert_eq!(
        windows_to_linux(r"\\WSL.localhost\Ubuntu\home\anna"),
        "/home/anna"
    );
    assert_eq!(
        windows_to_linux("syntax window Agent"),
        "syntax window Agent"
    );
    assert_eq!(linux_to_windows(WSL_SERVER, "Ubuntu"), WIN_SERVER);
    assert_eq!(linux_to_windows("/mnt/d", "Ubuntu"), r"D:\");
    assert_eq!(
        linux_to_windows("/home/anna/data/a.sav", "Ubuntu"),
        r"\\wsl$\Ubuntu\home\anna\data\a.sav"
    );
    // /mnt/wsl is not a drive
    assert_eq!(
        linux_to_windows("/mnt/wsl/x", "Ubuntu"),
        r"\\wsl$\Ubuntu\mnt\wsl\x"
    );
    assert_eq!(
        linux_to_windows(r"C:\data\a.sav", "Ubuntu"),
        r"C:\data\a.sav"
    );
    assert_eq!(linux_to_windows("data/a.sav", "Ubuntu"), "data/a.sav");
}

#[test]
fn the_system_runner_reads_both_outputs_of_a_real_program() {
    let (ok, text) =
        statscraft_connect::system_runner()(Path::new("rustc"), &["--version"]).unwrap();
    assert!(ok && text.starts_with("rustc "), "{text}");
    let (ok, _) =
        statscraft_connect::system_runner()(Path::new("rustc"), &["--no-such-flag"]).unwrap();
    assert!(!ok);
}
