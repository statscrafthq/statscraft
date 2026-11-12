//! Adds the Statscraft MCP server to the AI agents installed on this computer, and takes it out
//! again. Each agent gets an entry named `statscraft` that starts `<binary> mcp` directly, without
//! a shell. Config files are parsed, merged and written back atomically; a file that does not
//! parse is left alone, and the first change of a file keeps a copy of it next to it.

use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

/// The name of our entry in every agent's list of servers.
pub const KEY: &str = "statscraft";
/// Appended to a config file's name for the one copy kept before the first change.
pub const BACKUP_SUFFIX: &str = ".statscraft-backup";
/// The page that explains every change and how to make it by hand.
pub const DOCS: &str = "https://docs.statscraft.org/connect.html";

/// `path` without the `\\?\` prefix Windows adds when it resolves a path, which some agents
/// cannot start. The command line and the window both name the server this way, so an entry one
/// of them wrote counts as connected in the other.
pub fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

/// The folders and settings the agents' config locations depend on. Tests point them at a
/// temporary folder so they never touch the real user's files.
#[derive(Clone, Debug)]
pub struct Places {
    pub os: Os,
    pub home: PathBuf,
    /// Windows %APPDATA%
    pub appdata: Option<PathBuf>,
    /// Windows %LOCALAPPDATA%
    pub local_appdata: Option<PathBuf>,
    /// $XDG_CONFIG_HOME
    pub xdg_config: Option<PathBuf>,
    /// $CODEX_HOME
    pub codex_home: Option<PathBuf>,
    /// $COPILOT_HOME
    pub copilot_home: Option<PathBuf>,
    /// $CLAUDE_CONFIG_DIR
    pub claude_config_dir: Option<PathBuf>,
    /// The folders of PATH, searched for the `claude` command
    pub path: Vec<PathBuf>,
}

impl Places {
    pub fn from_env() -> Places {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Places {
            os: if cfg!(windows) {
                Os::Windows
            } else if cfg!(target_os = "macos") {
                Os::Mac
            } else {
                Os::Linux
            },
            home: std::env::home_dir().unwrap_or_default(),
            appdata: var("APPDATA"),
            local_appdata: var("LOCALAPPDATA"),
            xdg_config: var("XDG_CONFIG_HOME"),
            codex_home: var("CODEX_HOME"),
            copilot_home: var("COPILOT_HOME"),
            claude_config_dir: var("CLAUDE_CONFIG_DIR"),
            path: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
        }
    }

    fn which(&self, program: &str) -> Option<PathBuf> {
        let names: Vec<String> = match self.os {
            Os::Windows => ["exe", "cmd"]
                .iter()
                .map(|ext| format!("{program}.{ext}"))
                .collect(),
            _ => vec![program.to_string()],
        };
        self.path
            .iter()
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|p| p.is_file())
    }
}

/// Runs a program with arguments and returns whether it succeeded and what it printed.
pub type Runner = Box<dyn Fn(&Path, &[&str]) -> io::Result<(bool, String)>>;

/// Runs the program for real, without a shell.
pub fn system_runner() -> Runner {
    Box::new(|program, args| {
        let mut command = std::process::Command::new(program);
        command.args(args);
        // no console window flashing up over the Statscraft window
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        let out = command.output()?;
        let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stdout));
        Ok((out.status.success(), text.trim().to_string()))
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    /// `mcpServers.statscraft` in a JSON file; the fields our entry sets.
    Json(Entry),
    /// `[mcp_servers.statscraft]` in a TOML file
    Toml,
    /// The `claude mcp` command
    ClaudeCode,
}

#[derive(Clone, Copy, PartialEq)]
enum Entry {
    /// command and args
    Plain,
    /// type stdio, command and args
    Typed,
    /// type stdio, command, args and all tools allowed
    Copilot,
}

struct Client {
    id: &'static str,
    name: &'static str,
    restart: &'static str,
    /// What the user should know about where the tools show up.
    note: Option<&'static str>,
    format: Format,
}

const CLIENTS: [Client; 7] = [
    Client {
        id: "claude-desktop",
        name: "Claude Desktop",
        restart: "quit Claude Desktop completely (also from the tray or menu bar) and open it again",
        note: None,
        format: Format::Json(Entry::Plain),
    },
    Client {
        id: "claude-code",
        name: "Claude Code",
        restart: "start a new Claude Code session",
        note: None,
        format: Format::ClaudeCode,
    },
    Client {
        id: "codex",
        name: "Codex",
        restart: "quit Codex or the ChatGPT app and open it again",
        note: Some(
            "Also the Codex view of the ChatGPT app. Ordinary ChatGPT chats cannot use tools on \
             this computer; open the Codex view.",
        ),
        format: Format::Toml,
    },
    Client {
        id: "cursor",
        name: "Cursor",
        restart: "quit Cursor and open it again",
        note: None,
        format: Format::Json(Entry::Typed),
    },
    Client {
        id: "copilot",
        name: "VS Code with Copilot",
        restart: "quit VS Code and open it again",
        note: None,
        format: Format::Json(Entry::Copilot),
    },
    Client {
        id: "gemini",
        name: "Gemini CLI",
        restart: "start a new Gemini CLI session",
        note: None,
        format: Format::Json(Entry::Plain),
    },
    Client {
        id: "antigravity",
        name: "Antigravity",
        restart: "quit Antigravity and open it again",
        note: None,
        format: Format::Json(Entry::Plain),
    },
];

/// Other names people use for the same agents.
const ALIASES: [(&str, &str); 4] = [
    ("chatgpt", "codex"),
    ("vscode", "copilot"),
    ("claude", "claude-desktop"),
    ("gemini-cli", "gemini"),
];

/// The ids `connect` and `disconnect` take.
pub fn ids() -> Vec<&'static str> {
    CLIENTS.iter().map(|c| c.id).collect()
}

fn client(id: &str) -> Result<&'static Client, String> {
    let id = id.to_ascii_lowercase();
    let id = ALIASES
        .iter()
        .find(|(alias, _)| *alias == id)
        .map_or(id.as_str(), |(_, to)| to);
    CLIENTS
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("unknown agent {id}; the agents are {}", ids().join(", ")))
}

/// What the user should know about where an agent shows the tools, if anything.
pub fn note(id: &str) -> Option<&'static str> {
    client(id).ok()?.note
}

/// Said after a change of Claude Desktop's settings file while Claude Desktop runs.
pub const DESKTOP_RUNNING: &str = "Claude Desktop is running and may write its settings file \
     over this change. Quit it completely (also from the tray or menu bar), then do this step again";

/// The `statscraft` entry in one config file: the command it starts, or None when the file or the
/// entry is not there. Read afresh, so a check shows whether an app took the entry out again.
pub fn entry_in(path: &Path) -> Result<Option<String>, String> {
    if path.extension().is_some_and(|e| e == "toml") {
        toml_entry(path)
    } else {
        json_entry(path)
    }
}

/// What `clients` knows about one agent.
#[derive(Serialize, Debug, Clone)]
pub struct ClientStatus {
    pub id: &'static str,
    pub name: &'static str,
    /// The agent is installed: its config folder exists (Claude Code: `claude` is on PATH).
    pub detected: bool,
    /// The config files our entry goes into (Claude Desktop on Windows can have two).
    pub configs: Vec<PathBuf>,
    /// An entry named statscraft is there.
    pub connected: bool,
    /// That entry starts the given binary.
    pub points_here: bool,
    /// Why the state could not be read, for example a config file that does not parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// What `connect` or `disconnect` did for one agent.
#[derive(Serialize, Debug, Clone)]
pub struct Outcome {
    pub id: &'static str,
    pub name: &'static str,
    /// Something was written or run; false when the agent was already as asked.
    pub changed: bool,
    /// Config files written.
    pub files: Vec<PathBuf>,
    /// Copies made before the first change of a file.
    pub backups: Vec<PathBuf>,
    /// The command run (Claude Code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// What the user must do before the agent sees the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<&'static str>,
    /// Why the change may not last, for example an app that runs and may write its file back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<&'static str>,
}

pub struct Connector {
    places: Places,
    runner: Runner,
}

impl Connector {
    pub fn new(places: Places, runner: Runner) -> Connector {
        Connector { places, runner }
    }

    /// The real places of this computer, and real commands.
    pub fn from_env() -> Connector {
        Connector::new(Places::from_env(), system_runner())
    }

    /// Every supported agent with its state; `server` is the binary an entry should start.
    pub fn clients(&self, server: &Path) -> Vec<ClientStatus> {
        CLIENTS.iter().map(|c| self.status(c, server)).collect()
    }

    /// Adds or updates the entry that starts `<server> mcp` in one agent's config.
    pub fn connect(&self, id: &str, server: &Path) -> Result<Outcome, String> {
        let client = client(id)?;
        let server = server
            .to_str()
            .ok_or_else(|| format!("the path {} is not valid text", server.display()))?;
        let (files, detected) = self.configs(client);
        if !detected {
            return Err(format!("{} was not found on this computer", client.name));
        }
        let mut outcome = outcome(client);
        match client.format {
            Format::Json(entry) => {
                for file in &files {
                    edit_json(file, Some(entry_json(entry, server)), &mut outcome)?;
                }
            }
            Format::Toml => edit_toml(&files[0], Some(server), &mut outcome)?,
            Format::ClaudeCode => {
                let state = claude_code_entry(&files[0])?;
                if state.as_deref() != Some(server) {
                    if state.is_some() {
                        self.claude(&["mcp", "remove", "--scope", "user", KEY], &mut outcome)?;
                    }
                    let args = ["mcp", "add", "--scope", "user", KEY, "--", server, "mcp"];
                    self.claude(&args, &mut outcome)?;
                }
            }
        }
        self.finish(client, &mut outcome);
        Ok(outcome)
    }

    /// Takes our entry out of one agent's config; everything else stays.
    pub fn disconnect(&self, id: &str) -> Result<Outcome, String> {
        let client = client(id)?;
        let (files, _) = self.configs(client);
        let mut outcome = outcome(client);
        match client.format {
            Format::Json(_) => {
                for file in &files {
                    edit_json(file, None, &mut outcome)?;
                }
            }
            Format::Toml => edit_toml(&files[0], None, &mut outcome)?,
            Format::ClaudeCode => {
                if claude_code_entry(&files[0])?.is_some() {
                    self.claude(&["mcp", "remove", "--scope", "user", KEY], &mut outcome)?;
                }
            }
        }
        self.finish(client, &mut outcome);
        Ok(outcome)
    }

    fn finish(&self, client: &Client, outcome: &mut Outcome) {
        outcome.restart = outcome.changed.then_some(client.restart);
        if outcome.changed && client.id == "claude-desktop" && self.desktop_running() {
            outcome.warning = Some(DESKTOP_RUNNING);
        }
    }

    /// Whether Claude Desktop runs now; it may write its settings file back over a change. A
    /// process list that cannot be read counts as not running.
    pub fn desktop_running(&self) -> bool {
        let (program, args): (&str, &[&str]) = match self.places.os {
            // Claude Code's own program is claude.exe too, in ~\.local\bin; it keeps no such file
            Os::Windows => (
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Get-Process -Name claude -ErrorAction SilentlyContinue | \
                     ForEach-Object { 'process:' + $_.Path }",
                ],
            ),
            // the app's process is Claude; Claude Code's is claude, and pgrep tells case apart
            Os::Mac => ("pgrep", &["-x", "Claude"]),
            // shortcut: the community builds' name; check others when an official build exists
            Os::Linux => ("pgrep", &["-f", "-i", "claude-desktop"]),
        };
        match (self.runner)(Path::new(program), args) {
            Ok((ok, text)) if self.places.os == Os::Windows => {
                ok && text.lines().any(|line| {
                    line.strip_prefix("process:").is_some_and(|path| {
                        !path
                            .to_ascii_lowercase()
                            .replace('/', "\\")
                            .ends_with("\\.local\\bin\\claude.exe")
                    })
                })
            }
            Ok((ok, _)) => ok,
            Err(_) => false,
        }
    }

    fn claude(&self, args: &[&str], outcome: &mut Outcome) -> Result<(), String> {
        let program = self
            .places
            .which("claude")
            .ok_or("Claude Code was not found on this computer")?;
        let shown = format!("claude {}", args.join(" "));
        let (ok, text) = (self.runner)(&program, args)
            .map_err(|e| format!("could not run {}: {e}", program.display()))?;
        if !ok {
            return Err(format!("{shown} failed: {text}"));
        }
        outcome.changed = true;
        outcome.command = Some(shown);
        Ok(())
    }

    fn status(&self, client: &'static Client, server: &Path) -> ClientStatus {
        let (configs, detected) = self.configs(client);
        let mut status = ClientStatus {
            id: client.id,
            name: client.name,
            detected,
            configs: configs.clone(),
            connected: false,
            points_here: false,
            problem: None,
        };
        if !detected {
            return status;
        }
        if client.format == Format::ClaudeCode {
            // The user-scope list lives in this file; `claude mcp get` would start every server to check it.
            status.configs.clear();
        }
        let server = server.to_str();
        for file in &configs {
            let found = match client.format {
                Format::Json(_) => json_entry(file),
                Format::Toml => toml_entry(file),
                Format::ClaudeCode => claude_code_entry(file),
            };
            match found {
                Ok(Some(command)) => {
                    status.connected = true;
                    status.points_here = server == Some(command.as_str());
                }
                Ok(None) => {}
                Err(e) => status.problem = Some(e),
            }
        }
        status
    }

    /// The agent's config files, and whether the agent is installed.
    fn configs(&self, client: &Client) -> (Vec<PathBuf>, bool) {
        let p = &self.places;
        let home = &p.home;
        let in_folder = |folder: PathBuf, file: &str| {
            let detected = folder.is_dir();
            (vec![folder.join(file)], detected)
        };
        match client.id {
            "claude-desktop" => {
                let folders = self.desktop_folders();
                let found: Vec<PathBuf> = folders.iter().filter(|f| f.is_dir()).cloned().collect();
                let detected = !found.is_empty();
                let folders = if detected { found } else { folders };
                let files = folders
                    .into_iter()
                    .map(|f| f.join("claude_desktop_config.json"))
                    .collect();
                (files, detected)
            }
            "claude-code" => {
                let folder = p.claude_config_dir.clone().unwrap_or_else(|| home.clone());
                (
                    vec![folder.join(".claude.json")],
                    p.which("claude").is_some(),
                )
            }
            "codex" => in_folder(
                p.codex_home.clone().unwrap_or_else(|| home.join(".codex")),
                "config.toml",
            ),
            "cursor" => in_folder(home.join(".cursor"), "mcp.json"),
            "copilot" => in_folder(
                p.copilot_home
                    .clone()
                    .unwrap_or_else(|| home.join(".copilot")),
                "mcp-config.json",
            ),
            "gemini" => in_folder(home.join(".gemini"), "settings.json"),
            "antigravity" => {
                let gemini = home.join(".gemini");
                let detected =
                    gemini.join("config").is_dir() || gemini.join("antigravity").is_dir();
                (
                    vec![gemini.join("config").join("mcp_config.json")],
                    detected,
                )
            }
            _ => unreachable!("every client has its places"),
        }
    }

    fn desktop_folders(&self) -> Vec<PathBuf> {
        let p = &self.places;
        match p.os {
            Os::Mac => vec![p.home.join("Library/Application Support/Claude")],
            Os::Linux => vec![
                p.xdg_config
                    .clone()
                    .unwrap_or_else(|| p.home.join(".config"))
                    .join("Claude"),
            ],
            Os::Windows => {
                let roaming = p
                    .appdata
                    .clone()
                    .unwrap_or_else(|| p.home.join("AppData").join("Roaming"));
                let local = p
                    .local_appdata
                    .clone()
                    .unwrap_or_else(|| p.home.join("AppData").join("Local"));
                let mut folders = vec![roaming.join("Claude")];
                // The Store package keeps its own copy of the roaming folder.
                if let Ok(entries) = fs::read_dir(local.join("Packages")) {
                    let mut packages: Vec<PathBuf> = entries
                        .flatten()
                        .filter(|e| e.file_name().to_string_lossy().starts_with("Claude_"))
                        .map(|e| e.path().join("LocalCache").join("Roaming").join("Claude"))
                        .collect();
                    packages.sort();
                    folders.extend(packages);
                }
                folders
            }
        }
    }
}

fn outcome(client: &'static Client) -> Outcome {
    Outcome {
        id: client.id,
        name: client.name,
        changed: false,
        files: Vec::new(),
        backups: Vec::new(),
        command: None,
        restart: None,
        warning: None,
    }
}

fn entry_json(entry: Entry, server: &str) -> Map<String, Value> {
    let mut map = Map::new();
    if entry != Entry::Plain {
        map.insert("type".into(), "stdio".into());
    }
    map.insert("command".into(), server.into());
    map.insert("args".into(), vec!["mcp"].into());
    if entry == Entry::Copilot {
        map.insert("tools".into(), vec!["*"].into());
    }
    map
}

/// A config file as read: its text without a byte order mark, and how it was written.
struct Text {
    body: String,
    bom: bool,
    crlf: bool,
}

fn read(path: &Path) -> Result<Option<Text>, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let body = text.strip_prefix('\u{feff}');
            Ok(Some(Text {
                bom: body.is_some(),
                crlf: text.contains("\r\n"),
                body: body.unwrap_or(&text).to_string(),
            }))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("could not read {}: {e}", path.display())),
    }
}

fn not_parsed(path: &Path, why: impl std::fmt::Display) -> String {
    format!(
        "{} could not be read as plain {} ({why}); Statscraft left it unchanged. Add the entry by hand, see {DOCS}",
        path.display(),
        if path.extension().is_some_and(|e| e == "toml") {
            "TOML"
        } else {
            "JSON"
        }
    )
}

/// The JSON file's root object; a missing or empty file is an empty object.
fn parse_json(path: &Path, text: Option<&Text>) -> Result<Map<String, Value>, String> {
    match text {
        Some(t) if !t.body.trim().is_empty() => match serde_json::from_str(&t.body) {
            Ok(Value::Object(root)) => Ok(root),
            Ok(_) => Err(not_parsed(path, "it is not an object")),
            Err(e) => Err(not_parsed(path, e)),
        },
        _ => Ok(Map::new()),
    }
}

/// The command of our entry in a JSON file, if there is one.
fn json_entry(path: &Path) -> Result<Option<String>, String> {
    let root = parse_json(path, read(path)?.as_ref())?;
    Ok(root.get("mcpServers").and_then(|s| s.get(KEY)).map(|e| {
        e.get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }))
}

/// Claude Code keeps its user-scope servers under `mcpServers` in `.claude.json`.
fn claude_code_entry(path: &Path) -> Result<Option<String>, String> {
    json_entry(path)
}

/// Sets our entry (`Some`) or takes it out (`None`), keeping every other key.
fn edit_json(
    path: &Path,
    entry: Option<Map<String, Value>>,
    outcome: &mut Outcome,
) -> Result<(), String> {
    let path = &real_path(path);
    let text = read(path)?;
    let mut root = parse_json(path, text.as_ref())?;
    let servers = match (root.get_mut("mcpServers"), &entry) {
        (Some(Value::Object(servers)), _) => servers,
        (Some(_), _) => return Err(not_parsed(path, "mcpServers is not an object")),
        (None, None) => return Ok(()),
        (None, Some(_)) => root
            .entry("mcpServers")
            .or_insert(Value::Object(Map::new()))
            .as_object_mut()
            .expect("just inserted an object"),
    };
    match entry {
        Some(fields) => {
            // Fields the user added to our entry (env, for example) stay.
            let old = servers.get(KEY);
            let mut new = old.and_then(Value::as_object).cloned().unwrap_or_default();
            new.extend(fields);
            let new = Value::Object(new);
            if old == Some(&new) {
                return Ok(());
            }
            servers.insert(KEY.into(), new);
        }
        None => {
            if servers.shift_remove(KEY).is_none() {
                return Ok(());
            }
        }
    }
    let indent = text
        .as_ref()
        .map_or("  ".to_string(), |t| indent_of(&t.body));
    let mut body = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut serializer = serde_json::Serializer::with_formatter(&mut body, formatter);
    Value::Object(root)
        .serialize(&mut serializer)
        .map_err(|e| e.to_string())?;
    let mut body = String::from_utf8(body).expect("serde_json writes UTF-8");
    body.push('\n');
    save(path, text.as_ref(), body, outcome)
}

/// The indentation of the first indented line, two spaces when there is none.
fn indent_of(body: &str) -> String {
    body.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| &l[..l.len() - l.trim_start().len()])
        .find(|w| !w.is_empty())
        .unwrap_or("  ")
        .to_string()
}

fn parse_toml(path: &Path, text: Option<&Text>) -> Result<DocumentMut, String> {
    text.map_or("", |t| &t.body)
        .parse()
        .map_err(|e: toml_edit::TomlError| not_parsed(path, e.message()))
}

fn toml_entry(path: &Path) -> Result<Option<String>, String> {
    let doc = parse_toml(path, read(path)?.as_ref())?;
    Ok(doc.get("mcp_servers").and_then(|s| s.get(KEY)).map(|e| {
        e.get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    }))
}

/// Sets our `[mcp_servers.statscraft]` table (`Some`) or takes it out (`None`).
fn edit_toml(path: &Path, server: Option<&str>, outcome: &mut Outcome) -> Result<(), String> {
    let path = &real_path(path);
    let text = read(path)?;
    let mut doc = parse_toml(path, text.as_ref())?;
    match server {
        Some(server) => {
            let old = doc.get("mcp_servers").and_then(|s| s.get(KEY));
            let same = old.is_some_and(|e| {
                let args = e.get("args").and_then(|a| a.as_array());
                e.get("command").and_then(|c| c.as_str()) == Some(server)
                    && args.is_some_and(|a| {
                        a.len() == 1 && a.get(0).and_then(|v| v.as_str()) == Some("mcp")
                    })
            });
            if same {
                return Ok(());
            }
            let servers = doc.entry("mcp_servers").or_insert_with(|| {
                let mut table = toml_edit::Table::new();
                table.set_implicit(true);
                toml_edit::Item::Table(table)
            });
            let Some(servers) = servers.as_table_like_mut() else {
                return Err(not_parsed(path, "mcp_servers is not a table"));
            };
            let entry = servers
                .entry(KEY)
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if !entry.is_table_like() {
                return Err(not_parsed(path, "mcp_servers.statscraft is not a table"));
            }
            entry["command"] = toml_edit::value(server);
            entry["args"] = toml_edit::value(toml_edit::Array::from_iter(["mcp"]));
        }
        None => {
            let removed = doc
                .get_mut("mcp_servers")
                .and_then(|s| s.as_table_like_mut())
                .and_then(|s| s.remove(KEY));
            if removed.is_none() {
                return Ok(());
            }
        }
    }
    save(path, text.as_ref(), doc.to_string(), outcome)
}

/// The file a symbolic link points at, so a linked config stays linked.
fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Writes the new text atomically, with the old file's line endings, byte order mark and
/// permissions; the first change of an existing file keeps a copy of it next to it.
fn save(
    path: &Path,
    old: Option<&Text>,
    body: String,
    outcome: &mut Outcome,
) -> Result<(), String> {
    let failed = |e: io::Error| format!("could not write {}: {e}", path.display());
    let name = path
        .file_name()
        .expect("config paths name a file")
        .to_string_lossy();
    match old {
        Some(_) => {
            let backup = path.with_file_name(format!("{name}{BACKUP_SUFFIX}"));
            if !backup.exists() {
                fs::copy(path, &backup).map_err(failed)?;
                outcome.backups.push(backup);
            }
        }
        None => fs::create_dir_all(path.parent().expect("config paths have a folder"))
            .map_err(failed)?,
    }
    let mut body = body.replace("\r\n", "\n");
    if let Some(old) = old {
        if !old.body.ends_with('\n') && !old.body.is_empty() {
            body.truncate(body.trim_end_matches('\n').len());
        }
        if old.crlf {
            body = body.replace('\n', "\r\n");
        }
        if old.bom {
            body.insert(0, '\u{feff}');
        }
    }
    let temp = path.with_file_name(format!("{name}.statscraft-temp"));
    let written = fs::write(&temp, body)
        .and_then(|()| match fs::metadata(path) {
            Ok(meta) => fs::set_permissions(&temp, meta.permissions()),
            Err(_) => Ok(()),
        })
        .and_then(|()| fs::rename(&temp, path));
    if let Err(e) = written {
        let _ = fs::remove_file(&temp);
        return Err(failed(e));
    }
    outcome.changed = true;
    outcome.files.push(path.to_path_buf());
    Ok(())
}
