//! Adds the Statscraft MCP server to the AI agents installed on this computer, and takes it out
//! again. Each agent gets an entry named `statscraft` that starts `<binary> mcp` directly, without
//! a shell. Config files are parsed, merged and written back atomically; a file that does not
//! parse is left alone, and the first change of a file keeps a copy of it next to it.
//!
//! On Windows the agents inside WSL are listed too, once per distribution: their config files are
//! reached through `\\wsl$`, and their entries start the Windows program through the
//! distribution's `/mnt/c` path, so the server keeps its link to the open Windows window.

use serde::Serialize;
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use toml_edit::DocumentMut;

/// The name of our entry in every agent's list of servers.
pub const KEY: &str = "statscraft";
/// Appended to a config file's name for the one copy kept before the first change.
pub const BACKUP_SUFFIX: &str = ".statscraft-backup";
/// The page that explains every change and how to make it by hand.
pub const DOCS: &str = "https://docs.statscraft.org/connect.html";
/// Set in the entry of an agent inside WSL to the distribution's name, so the server knows it was
/// started from there and can translate the Linux paths it is given.
pub const WSL_DISTRO_ENV: &str = "STATSCRAFT_WSL_DISTRO";
/// The longest a command may take; WSL may first have to start its virtual machine.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// Between an agent's id and a distribution's name in the id of an agent inside WSL:
/// `codex-wsl:Ubuntu`.
const WSL_MARK: &str = "-wsl:";
/// The agents that keep their settings in the Linux home, so they can be connected inside WSL.
const WSL_CLIENTS: [&str; 4] = ["claude-code", "codex", "gemini", "antigravity"];

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
    /// wsl.exe, on Windows where it is installed; None skips WSL
    pub wsl: Option<PathBuf>,
    /// The folder that holds each WSL distribution's files under its name: `\\wsl$`
    pub wsl_root: PathBuf,
}

impl Places {
    pub fn from_env() -> Places {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let mut places = Places {
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
            wsl: None,
            // works on every Windows version with WSL; \\wsl.localhost only on newer ones
            wsl_root: PathBuf::from(r"\\wsl$"),
        };
        if places.os == Os::Windows {
            places.wsl = places.which("wsl");
        }
        places
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

/// Runs the program for real, without a shell, and stops it after [`TIMEOUT`].
pub fn system_runner() -> Runner {
    Box::new(|program, args| {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            // no console window flashing up over the Statscraft window
            std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
            // wsl.exe prints its own messages in UTF-16 without this
            command.env("WSL_UTF8", "1");
        }
        let mut child = command.spawn()?;
        let reader = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = pipe.read_to_end(&mut bytes);
                bytes
            })
        };
        let out = reader(Box::new(child.stdout.take().expect("piped")));
        let err = reader(Box::new(child.stderr.take().expect("piped")));
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("it gave no answer in {} seconds", TIMEOUT.as_secs()),
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut text = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.join().unwrap_or_default()));
        Ok((status.success(), text.trim().to_string()))
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

/// The agent an id names, and the WSL distribution it names after `-wsl:`, if any.
fn client(id: &str) -> Result<(&'static Client, Option<&str>), String> {
    let lower = id.to_ascii_lowercase();
    let (base, distro) = match lower.find(WSL_MARK) {
        Some(at) => (&lower[..at], Some(&id[at + WSL_MARK.len()..])),
        None => (lower.as_str(), None),
    };
    let base = ALIASES
        .iter()
        .find(|(alias, _)| *alias == base)
        .map_or(base, |(_, to)| to);
    let client = CLIENTS
        .iter()
        .find(|c| c.id == base)
        .ok_or_else(|| format!("unknown agent {id}; the agents are {}", ids().join(", ")))?;
    if distro.is_some() && !WSL_CLIENTS.contains(&client.id) {
        return Err(format!(
            "{} does not run inside WSL; the agents there are {}",
            client.name,
            WSL_CLIENTS.join(", ")
        ));
    }
    Ok((client, distro))
}

/// Said about Codex inside WSL.
const WSL_CODEX_NOTE: &str = "The Codex view of the ChatGPT app reads this file when Codex runs \
     in WSL. Ordinary ChatGPT chats cannot use tools on this computer.";

/// What the user should know about where an agent shows the tools, if anything.
pub fn note(id: &str) -> Option<&'static str> {
    match client(id).ok()? {
        (client, None) => client.note,
        (client, Some(_)) => (client.id == "codex").then_some(WSL_CODEX_NOTE),
    }
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
    /// `codex`, or `codex-wsl:Ubuntu` for Codex inside a WSL distribution
    pub id: String,
    /// `Codex`, or `Codex (WSL: Ubuntu)`
    pub name: String,
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
    pub id: String,
    pub name: String,
    /// Something was written or run; false when the agent was already as asked.
    pub changed: bool,
    /// Config files written.
    pub files: Vec<PathBuf>,
    /// Copies made before the first change of a file.
    pub backups: Vec<PathBuf>,
    /// The command run (Claude Code); inside WSL, the command run there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// What the user must do before the agent sees the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<&'static str>,
    /// Why the change may not last, for example an app that runs and may write its file back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<&'static str>,
}

/// A WSL distribution with the places of its Linux home, as Windows reaches them.
#[derive(Clone, Debug)]
struct Distro {
    name: String,
    /// The home through `\\wsl$`; no environment of the distribution is known here
    places: Places,
    /// `claude` is on the PATH of a login shell there
    claude: bool,
    /// The Windows server as the distribution names it, such as /mnt/c/...; empty when not asked
    server: String,
}

pub struct Connector {
    places: Places,
    runner: Runner,
    /// The WSL distributions found, for the server they were asked about
    wsl: RefCell<Option<(String, Vec<Distro>)>>,
}

impl Connector {
    pub fn new(places: Places, runner: Runner) -> Connector {
        Connector {
            places,
            runner,
            wsl: RefCell::new(None),
        }
    }

    /// The real places of this computer, and real commands.
    pub fn from_env() -> Connector {
        Connector::new(Places::from_env(), system_runner())
    }

    /// Whether listing the agents asks WSL, which can take a few seconds while it starts.
    pub fn looks_into_wsl(&self) -> bool {
        self.places.wsl.is_some()
    }

    /// Every supported agent with its state; `server` is the binary an entry should start. The
    /// agents inside WSL are listed only where they are installed.
    pub fn clients(&self, server: &Path) -> Vec<ClientStatus> {
        let mut list: Vec<ClientStatus> = CLIENTS
            .iter()
            .map(|c| self.status(c, None, server))
            .collect();
        for distro in self.distros(server.to_str().unwrap_or("")) {
            for client in CLIENTS.iter().filter(|c| WSL_CLIENTS.contains(&c.id)) {
                let status = self.status(client, Some(&distro), server);
                if status.detected {
                    list.push(status);
                }
            }
        }
        list
    }

    /// The agent an id names, with its distribution when it runs inside WSL.
    fn target(&self, id: &str, server: &str) -> Result<(&'static Client, Option<Distro>), String> {
        let (client, distro) = client(id)?;
        let Some(name) = distro else {
            return Ok((client, None));
        };
        let distro = self
            .distros(server)
            .into_iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("no WSL distribution named {name} was found"))?;
        Ok((client, Some(distro)))
    }

    /// Adds or updates the entry that starts `<server> mcp` in one agent's config.
    pub fn connect(&self, id: &str, server: &Path) -> Result<Outcome, String> {
        let server = server
            .to_str()
            .ok_or_else(|| format!("the path {} is not valid text", server.display()))?;
        let (client, wsl) = self.target(id, server)?;
        let wsl = wsl.as_ref();
        let (files, detected) = self.configs(client, wsl);
        let mut outcome = outcome(client, wsl);
        if !detected {
            return Err(format!("{} was not found on this computer", outcome.name));
        }
        let server = wsl.map_or(server, |d| d.server.as_str());
        let distro = wsl.map(|d| d.name.as_str());
        match client.format {
            Format::Json(entry) => {
                for file in &files {
                    edit_json(file, Some(entry_json(entry, server, distro)), &mut outcome)?;
                }
            }
            Format::Toml => edit_toml(&files[0], Some((server, distro)), &mut outcome)?,
            Format::ClaudeCode => {
                let state = json_server(&files[0])?;
                let same = state.as_ref().is_some_and(|e| {
                    e.get("command").and_then(Value::as_str) == Some(server)
                        && distro.is_none_or(|d| env_names(e.get("env"), d))
                });
                if !same {
                    if state.is_some() {
                        self.claude(
                            &["mcp", "remove", "--scope", "user", KEY],
                            wsl,
                            &mut outcome,
                        )?;
                    }
                    let mut args = vec!["mcp", "add", "--scope", "user", KEY];
                    let env = distro.map(|d| (format!("{WSL_DISTRO_ENV}={d}"), wslenv()));
                    if let Some((name, pass)) = &env {
                        args.extend(["-e", name, "-e", pass]);
                    }
                    args.extend(["--", server, "mcp"]);
                    self.claude(&args, wsl, &mut outcome)?;
                }
            }
        }
        self.finish(client, &mut outcome);
        Ok(outcome)
    }

    /// Takes our entry out of one agent's config; everything else stays.
    pub fn disconnect(&self, id: &str) -> Result<Outcome, String> {
        let (client, wsl) = self.target(id, "")?;
        let wsl = wsl.as_ref();
        let (files, _) = self.configs(client, wsl);
        let mut outcome = outcome(client, wsl);
        match client.format {
            Format::Json(_) => {
                for file in &files {
                    edit_json(file, None, &mut outcome)?;
                }
            }
            Format::Toml => edit_toml(&files[0], None, &mut outcome)?,
            Format::ClaudeCode => {
                if json_server(&files[0])?.is_some() {
                    self.claude(
                        &["mcp", "remove", "--scope", "user", KEY],
                        wsl,
                        &mut outcome,
                    )?;
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

    /// Runs `claude` with `args`: here, or inside a WSL distribution through a login shell,
    /// where it finds the PATH the user's own terminal has.
    fn claude(
        &self,
        args: &[&str],
        wsl: Option<&Distro>,
        outcome: &mut Outcome,
    ) -> Result<(), String> {
        let shown = format!("claude {}", args.join(" "));
        let script;
        let (program, run, shown) = match wsl {
            None => (
                self.places
                    .which("claude")
                    .ok_or("Claude Code was not found on this computer")?,
                args.to_vec(),
                shown,
            ),
            Some(d) => {
                let quoted: Vec<String> = args.iter().map(|a| sh_quote(a)).collect();
                script = format!("claude {}", quoted.join(" "));
                let wsl = self.places.wsl.clone().ok_or("wsl.exe was not found")?;
                let run = vec!["-d", d.name.as_str(), "-e", "sh", "-lc", script.as_str()];
                (wsl, run, format!("{shown} (in WSL: {})", d.name))
            }
        };
        let (ok, text) = (self.runner)(&program, &run)
            .map_err(|e| format!("could not run {}: {e}", program.display()))?;
        if !ok {
            return Err(format!("{shown} failed: {text}"));
        }
        outcome.changed = true;
        outcome.command = Some(shown);
        Ok(())
    }

    fn status(&self, client: &'static Client, wsl: Option<&Distro>, server: &Path) -> ClientStatus {
        let (configs, detected) = self.configs(client, wsl);
        let named = outcome(client, wsl);
        let mut status = ClientStatus {
            id: named.id,
            name: named.name,
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
        let server = match wsl {
            Some(d) => Some(d.server.as_str()),
            None => server.to_str(),
        };
        for file in &configs {
            let found = match client.format {
                Format::Json(_) | Format::ClaudeCode => json_entry(file),
                Format::Toml => toml_entry(file),
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
    fn configs(&self, client: &Client, wsl: Option<&Distro>) -> (Vec<PathBuf>, bool) {
        let p = wsl.map_or(&self.places, |d| &d.places);
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
                let detected = wsl.map_or_else(|| p.which("claude").is_some(), |d| d.claude);
                (vec![folder.join(".claude.json")], detected)
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

    /// The WSL distributions with a Linux home, each asked once per connector; `server` is
    /// translated to the distribution's form. A disconnect, which needs no server, takes what an
    /// earlier listing found.
    fn distros(&self, server: &str) -> Vec<Distro> {
        let Some(wsl) = &self.places.wsl else {
            return Vec::new();
        };
        if let Some((asked, found)) = &*self.wsl.borrow()
            && (asked == server || server.is_empty())
        {
            return found.clone();
        }
        let names = match (self.runner)(wsl, &["-l", "-q"]) {
            Ok((true, text)) => distro_names(&text),
            _ => Vec::new(),
        };
        let found: Vec<Distro> = names
            .into_iter()
            .filter_map(|name| self.probe(wsl, name, server))
            .collect();
        *self.wsl.borrow_mut() = Some((server.to_string(), found.clone()));
        found
    }

    /// One call into a distribution for its home, its `claude` and the server's Linux path.
    /// Each answer comes on its own marked line, so whatever a login script prints is skipped.
    fn probe(&self, wsl: &Path, name: String, server: &str) -> Option<Distro> {
        let translate = if server.is_empty() {
            String::new()
        } else {
            format!("wslpath -u {} 2>/dev/null", sh_quote(server))
        };
        let script = format!(
            "printf 'home:%s\\n' \"$HOME\"; printf 'claude:%s\\n' \"$(command -v claude)\"; \
             printf 'server:%s\\n' \"$({translate})\""
        );
        let (_, text) = (self.runner)(wsl, &["-d", &name, "-e", "sh", "-lc", &script]).ok()?;
        let field = |key: &str| {
            text.lines()
                .find_map(|line| line.trim_end_matches('\r').strip_prefix(key))
                .map(str::trim)
        };
        let home = field("home:").filter(|h| h.starts_with('/'))?;
        let mut folder = self.places.wsl_root.join(&name);
        folder.extend(home.split('/').filter(|part| !part.is_empty()));
        let server = match field("server:") {
            Some(path) if path.starts_with('/') => path.to_string(),
            _ => windows_to_linux(server),
        };
        Some(Distro {
            claude: field("claude:").is_some_and(|c| !c.is_empty()),
            places: Places {
                os: Os::Linux,
                home: folder,
                appdata: None,
                local_appdata: None,
                xdg_config: None,
                // the variables of the distribution are not known from here: the defaults
                codex_home: None,
                copilot_home: None,
                claude_config_dir: None,
                path: Vec::new(),
                wsl: None,
                wsl_root: PathBuf::new(),
            },
            server,
            name,
        })
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

fn outcome(client: &'static Client, wsl: Option<&Distro>) -> Outcome {
    Outcome {
        id: wsl.map_or(client.id.to_string(), |d| {
            format!("{}{WSL_MARK}{}", client.id, d.name)
        }),
        name: wsl.map_or(client.name.to_string(), |d| {
            format!("{} (WSL: {})", client.name, d.name)
        }),
        changed: false,
        files: Vec::new(),
        backups: Vec::new(),
        command: None,
        restart: None,
        warning: None,
    }
}

/// Our entry; inside WSL with the env that tells the server the distribution.
fn entry_json(entry: Entry, server: &str, distro: Option<&str>) -> Map<String, Value> {
    let mut map = Map::new();
    if entry != Entry::Plain {
        map.insert("type".into(), "stdio".into());
    }
    map.insert("command".into(), server.into());
    map.insert("args".into(), vec!["mcp"].into());
    if entry == Entry::Copilot {
        map.insert("tools".into(), vec!["*"].into());
    }
    if let Some(distro) = distro {
        let mut env = Map::new();
        env.insert(WSL_DISTRO_ENV.into(), distro.into());
        env.insert("WSLENV".into(), WSL_DISTRO_ENV.into());
        map.insert("env".into(), env.into());
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

/// Our whole entry in a JSON file, such as Claude Code's `.claude.json`, which keeps its
/// user-scope servers under `mcpServers`.
fn json_server(path: &Path) -> Result<Option<Value>, String> {
    let root = parse_json(path, read(path)?.as_ref())?;
    Ok(root.get("mcpServers").and_then(|s| s.get(KEY)).cloned())
}

/// `WSLENV`, which makes WSL hand our variable over to the Windows program it starts.
fn wslenv() -> String {
    format!("WSLENV={WSL_DISTRO_ENV}")
}

/// Whether an entry's env names the distribution, and passes it on to Windows.
fn env_names(env: Option<&Value>, distro: &str) -> bool {
    env.is_some_and(|e| {
        e.get(WSL_DISTRO_ENV).and_then(Value::as_str) == Some(distro)
            && e.get("WSLENV").and_then(Value::as_str) == Some(WSL_DISTRO_ENV)
    })
}

/// The names `wsl.exe -l -q` prints, one per line; older versions print UTF-16 whatever
/// WSL_UTF8 says, which arrives here with a zero byte after each letter. Docker Desktop's own
/// distributions hold no user's agents.
fn distro_names(text: &str) -> Vec<String> {
    text.replace(['\0', '\u{feff}', '\u{fffd}'], "")
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with("docker-desktop"))
        .map(str::to_string)
        .collect()
}

/// A word for `sh`: in single quotes, a single quote inside closed, escaped and reopened.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A Windows path as WSL mounts it, `C:\x\y` as `/mnt/c/x/y`; a path inside WSL,
/// `\\wsl$\Ubuntu\home\anna` or `\\wsl.localhost\Ubuntu\home\anna`, as `/home/anna`.
/// Anything else stays as it is. Used where `wslpath` cannot be asked.
pub fn windows_to_linux(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = path[2..].replace('\\', "/");
        let rest = rest.trim_matches('/');
        let drive = path[..1].to_ascii_lowercase();
        return if rest.is_empty() {
            format!("/mnt/{drive}")
        } else {
            format!("/mnt/{drive}/{rest}")
        };
    }
    let lower = path.to_ascii_lowercase();
    for share in [r"\\wsl$\", r"\\wsl.localhost\"] {
        if lower.starts_with(share) {
            let rest = &path[share.len()..];
            let inside = rest.find('\\').map_or("", |at| &rest[at..]);
            return if inside.is_empty() {
                "/".into()
            } else {
                inside.replace('\\', "/")
            };
        }
    }
    path.to_string()
}

/// A Linux path an agent inside WSL gives, as Windows reaches it: `/mnt/c/x` as `C:\x`, any
/// other absolute path through `\\wsl$\<distro>`. A path that does not start with `/` stays.
pub fn linux_to_windows(path: &str, distro: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix("/mnt/") {
        let (drive, tail) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        if drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic() {
            let tail = tail.trim_start_matches('/').replace('/', "\\");
            return format!("{}:\\{tail}", drive.to_ascii_uppercase());
        }
    }
    format!(r"\\wsl$\{distro}{}", path.replace('/', "\\"))
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
            // Fields the user added to our entry stay, and so do the variables they added to env.
            let old = servers.get(KEY);
            let mut new = old.and_then(Value::as_object).cloned().unwrap_or_default();
            for (key, value) in fields {
                match (new.get_mut(&key), value) {
                    (Some(Value::Object(had)), Value::Object(add)) => had.extend(add),
                    (_, value) => {
                        new.insert(key, value);
                    }
                }
            }
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

/// Sets our `[mcp_servers.statscraft]` table (`Some`, with the WSL distribution for its env) or
/// takes it out (`None`).
fn edit_toml(
    path: &Path,
    entry: Option<(&str, Option<&str>)>,
    outcome: &mut Outcome,
) -> Result<(), String> {
    let path = &real_path(path);
    let text = read(path)?;
    let mut doc = parse_toml(path, text.as_ref())?;
    match entry {
        Some((server, distro)) => {
            let old = doc.get("mcp_servers").and_then(|s| s.get(KEY));
            let same = old.is_some_and(|e| {
                let args = e.get("args").and_then(|a| a.as_array());
                let env = |key| {
                    e.get("env")
                        .and_then(|v| v.get(key))
                        .and_then(|v| v.as_str())
                };
                e.get("command").and_then(|c| c.as_str()) == Some(server)
                    && args.is_some_and(|a| {
                        a.len() == 1 && a.get(0).and_then(|v| v.as_str()) == Some("mcp")
                    })
                    && distro.is_none_or(|d| {
                        env(WSL_DISTRO_ENV) == Some(d) && env("WSLENV") == Some(WSL_DISTRO_ENV)
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
            if let Some(distro) = distro {
                let env = entry
                    .as_table_like_mut()
                    .expect("checked above")
                    .entry("env")
                    .or_insert(toml_edit::value(toml_edit::InlineTable::new()));
                let Some(env) = env.as_table_like_mut() else {
                    return Err(not_parsed(
                        path,
                        "mcp_servers.statscraft.env is not a table",
                    ));
                };
                env.insert(WSL_DISTRO_ENV, toml_edit::value(distro));
                env.insert("WSLENV", toml_edit::value(WSL_DISTRO_ENV));
            }
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
