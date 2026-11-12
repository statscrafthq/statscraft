//! The window as a live session: `statscraft run --session gui` sends syntax to a running
//! statscraft-gui, which runs it like its own Run button and replies with what the command line
//! prints. One JSON line each way over TCP on 127.0.0.1: the standard library has it on every
//! platform, where Unix sockets and named pipes would need a separate path for Windows.
//!
//! Discovery and security: the window listens on a random local port and writes the address and
//! a random token to `endpoint_file()`, readable only by the user. A request without the token
//! is refused, and the client sends the token only to a loopback address.
//!
//! Besides runs, a client can list, read, edit, open, save and run the window's syntax windows.
//! Every run sent to the window leaves its syntax in the syntax window Agent, so the person can
//! read, change and run again what was done. A window older than the client refuses a request
//! it does not know, and `send` turns that refusal into the advice to update.

use crate::session::{Failure, RunResult, Session};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The session name that means the running window.
pub const SESSION: &str = "gui";

/// Syntax larger than this is refused, so a stray client cannot fill the memory.
const MAX_REQUEST: u64 = 64 << 20;

/// The syntax window that keeps the syntax of the runs sent to the window.
pub const AGENT: &str = "Agent";

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    Run,
    Check,
    Describe,
    /// Lists the syntax windows.
    Windows,
    /// The text of `window` with line numbers.
    Read,
    /// Changes the text of `window` as `edit` says; runs nothing.
    Edit,
    /// Opens a syntax window named `name` (or the next free name) holding `source`.
    New,
    /// Opens the syntax file `path` in a syntax window.
    Open,
    /// Writes `window` to its file, or to `path`.
    Save,
    /// Runs the text of `window`, or its `lines`, as the window's own Run does.
    RunWindow,
}

/// A change to the text of a syntax window. Lines count from 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Edit {
    /// Puts `text` in place of the lines from FIRST to LAST; an empty text deletes them.
    Replace { lines: (u32, u32), text: String },
    /// Puts `text` before line `before`; one past the last line adds it at the end.
    Insert { before: u32, text: String },
    /// Adds `text` after the last line.
    Append { text: String },
    /// Puts `text` in place of everything.
    ReplaceAll { text: String },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Request {
    /// Filled in by `send` from the endpoint file.
    #[serde(default)]
    pub token: String,
    pub action: Action,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub lines: Option<(u32, u32)>,
    /// Reply with JSON instead of text, like `--json`.
    #[serde(default)]
    pub json: bool,
    /// The caller's working folder, so relative paths in the syntax mean what they mean in a
    /// file session.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// The syntax window a request names: its id or its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<Edit>,
    /// The file to open, or to save to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The name of a new syntax window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Save may replace a file that is not the window's own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overwrite: bool,
}

impl Request {
    /// Moves the window to the caller's working folder; it stays there for later runs.
    pub fn enter_folder(&self) {
        if let Some(dir) = &self.cwd {
            let _ = std::env::set_current_dir(dir);
        }
    }
}

/// What the command line prints on standard output, or the errors it reports.
pub type Response = Result<String, Vec<Failure>>;

#[derive(Serialize, Deserialize)]
struct Endpoint {
    address: SocketAddr,
    token: String,
}

/// Where the window writes its endpoint: the user's runtime folder on Linux, the local
/// application data folder on Windows, else ~/.config.
pub fn endpoint_file() -> PathBuf {
    let base = ["XDG_RUNTIME_DIR", "LOCALAPPDATA"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty()))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("statscraft").join("gui.json")
}

/// The reply to a run or a check: exactly what `statscraft run --session gui` prints.
pub fn reply(result: &mut Result<RunResult, Vec<Failure>>, request: &Request) -> Response {
    match result {
        Ok(result) => {
            result.session = Some(SESSION.into());
            Ok(result.output(request.json, request.action == Action::Check))
        }
        Err(errors) => Err(errors.clone()),
    }
}

/// The reply to `describe --session gui`.
pub fn describe(session: &Session, json: bool) -> Response {
    session.describe(json).ok_or_else(|| {
        let message = format!("session {SESSION} has no dataset yet");
        vec![Failure::plain("session", message)]
    })
}

/// Answers a request on the calling thread. The window runs on a worker thread instead, with
/// the same `Session` calls and `reply`.
pub fn answer(session: &mut Session, request: &Request) -> Response {
    request.enter_folder();
    match request.action {
        Action::Describe => describe(session, request.json),
        Action::Run => reply(&mut session.run(&request.source, request.lines), request),
        Action::Check => reply(&mut session.check(&request.source, request.lines), request),
        _ => Err(window_failure("this session has no syntax windows")),
    }
}

/// A syntax window as the requests about them see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyntaxInfo {
    pub id: u64,
    /// The file name, or Syntax1 and the like before the syntax has a file.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// The text differs from what its file holds, or new syntax is not empty.
    pub unsaved: bool,
    pub lines: usize,
    /// The window that keeps the syntax of the runs sent to the window.
    pub agent: bool,
    /// Syntax pasted from a dialog goes here.
    pub designated: bool,
    /// The window the person had the keyboard in last.
    pub focused: bool,
}

/// The syntax windows of a window. The real window implements it over its editors, `Desk` over
/// plain text; `answer_windows` and `keep_trail` work the same on both.
pub trait SyntaxWindows {
    /// Every syntax window, in the order of the Window menu.
    fn list(&self) -> Vec<SyntaxInfo>;
    fn text_mut(&mut self, id: u64) -> Option<&mut String>;
    /// A new syntax window: named `name`, else the next Syntax1, Syntax2, or after its file.
    /// With `designate` false, pasted syntax keeps going where it went before.
    fn create(
        &mut self,
        name: Option<String>,
        text: String,
        file: Option<PathBuf>,
        designate: bool,
    ) -> u64;
    /// The text of the window was written to `file`, which the window holds from now on.
    fn saved(&mut self, id: u64, file: PathBuf);
    /// The window that keeps the trail, once there is one.
    fn agent(&mut self) -> &mut Option<u64>;
}

fn window_failure(message: impl Into<String>) -> Vec<Failure> {
    vec![Failure::plain("window", message)]
}

/// The lines of a text as the line numbers of a run count them: a final line break ends the
/// last line and does not start another.
fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

/// The number of lines of a text, as a run numbers them.
pub fn line_count(text: &str) -> usize {
    split_lines(text).len()
}

/// The text with its line number before each line.
pub fn numbered(text: &str) -> String {
    let lines = split_lines(text);
    let width = lines.len().to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>width$}  {}\n", i + 1, line.trim_end_matches('\r')))
        .collect()
}

/// Changes `text` as `edit` says. Returns the lines the new text now takes, if any.
pub fn apply_edit(text: &mut String, edit: &Edit) -> Result<Option<(u32, u32)>, Vec<Failure>> {
    let mut lines = split_lines(text);
    let count = lines.len();
    let (range, new) = match edit {
        Edit::ReplaceAll { text: new } => {
            *text = new.clone();
            let count = split_lines(text).len() as u32;
            return Ok((count > 0).then_some((1, count)));
        }
        Edit::Replace {
            lines: (first, last),
            text: new,
        } => {
            if *first < 1 || first > last || *last as usize > count {
                return Err(window_failure(format!(
                    "lines {first}-{last} are not in the text, which has {count} lines"
                )));
            }
            (*first as usize - 1..*last as usize, new)
        }
        Edit::Insert { before, text: new } => {
            if *before < 1 || *before as usize > count + 1 {
                return Err(window_failure(format!(
                    "cannot insert before line {before}: the text has {count} lines"
                )));
            }
            (*before as usize - 1..*before as usize - 1, new)
        }
        Edit::Append { text: new } => (count..count, new),
    };
    let start = range.start as u32;
    let added = split_lines(new);
    let taken = added.len() as u32;
    let ends_with_break = text.is_empty() || text.ends_with('\n');
    lines.splice(range, added);
    let mut joined = lines.join("\n");
    if ends_with_break && !joined.is_empty() {
        joined.push('\n');
    }
    *text = joined;
    Ok((taken > 0).then_some((start + 1, start + taken)))
}

/// The syntax window `key` names: its id, its name or its file.
pub fn find(windows: &impl SyntaxWindows, key: Option<&str>) -> Result<SyntaxInfo, Vec<Failure>> {
    let all = windows.list();
    let names = || {
        let names: Vec<String> = all.iter().map(|w| format!("{} {}", w.id, w.name)).collect();
        if names.is_empty() {
            "there is none".to_string()
        } else {
            format!("open: {}", names.join(", "))
        }
    };
    let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else {
        return Err(window_failure(format!(
            "name the syntax window by its id or name; {}",
            names()
        )));
    };
    let found: Vec<&SyntaxInfo> = all
        .iter()
        .filter(|w| {
            w.id.to_string() == key
                || w.name.eq_ignore_ascii_case(key)
                || w.file.as_deref() == Some(Path::new(key))
        })
        .collect();
    match found[..] {
        [one] => Ok(one.clone()),
        [] => Err(window_failure(format!(
            "no syntax window {key}; {}",
            names()
        ))),
        _ => Err(window_failure(format!(
            "{} syntax windows are named {key}; give the id: {}",
            found.len(),
            found
                .iter()
                .map(|w| w.id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn json_reply(value: serde_json::Value) -> Response {
    Ok(serde_json::to_string(&value).expect("plain data"))
}

fn info(windows: &impl SyntaxWindows, id: u64) -> Option<SyntaxInfo> {
    windows.list().into_iter().find(|w| w.id == id)
}

/// Answers the requests about syntax windows, as JSON. None of them runs anything; only Save
/// writes a file, and it does not replace another file without `overwrite`.
pub fn answer_windows(windows: &mut impl SyntaxWindows, request: &Request) -> Response {
    let absolute = |what: &str| -> Result<PathBuf, Vec<Failure>> {
        match &request.path {
            Some(path) if path.is_absolute() => Ok(path.clone()),
            Some(path) => Err(window_failure(format!(
                "{} is not an absolute path",
                path.display()
            ))),
            None => Err(window_failure(format!("give the path of the file {what}"))),
        }
    };
    match request.action {
        Action::Windows => json_reply(serde_json::json!({ "windows": windows.list() })),
        Action::Read => {
            let window = find(windows, request.window.as_deref())?;
            let text = windows.text_mut(window.id).map(|t| numbered(t));
            json_reply(serde_json::json!({ "window": window, "text": text }))
        }
        Action::Edit => {
            let window = find(windows, request.window.as_deref())?;
            let edit = request
                .edit
                .as_ref()
                .ok_or_else(|| window_failure("say what to change"))?;
            let text = windows.text_mut(window.id).expect("listed");
            let lines = apply_edit(text, edit)?;
            json_reply(serde_json::json!({ "window": info(windows, window.id), "lines": lines }))
        }
        Action::New => {
            let name = request.name.as_deref().map(str::trim);
            if let Some(name) = name {
                if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
                    return Err(window_failure(
                        "a syntax window name needs a letter; leave it out for the next free name",
                    ));
                }
                if windows
                    .list()
                    .iter()
                    .any(|w| w.name.eq_ignore_ascii_case(name))
                {
                    return Err(window_failure(format!(
                        "a syntax window named {name} is open already"
                    )));
                }
            }
            let id = windows.create(name.map(str::to_string), request.source.clone(), None, true);
            json_reply(serde_json::json!({ "window": info(windows, id) }))
        }
        Action::Open => {
            let path = absolute("to open")?;
            let open = windows
                .list()
                .into_iter()
                .find(|w| w.file.as_ref() == Some(&path));
            let id = match open {
                Some(window) => window.id,
                None => {
                    let bytes = std::fs::read(&path)
                        .map_err(|e| window_failure(format!("{}: {e}", path.display())))?;
                    let text = statscraft_syntax::decode_syntax(&bytes, None)
                        .map_err(|e| window_failure(format!("{}: {e}", path.display())))?;
                    windows.create(None, text, Some(path), true)
                }
            };
            json_reply(serde_json::json!({ "window": info(windows, id) }))
        }
        Action::Save => {
            let window = find(windows, request.window.as_deref())?;
            let path = match (&request.path, &window.file) {
                (None, Some(file)) => file.clone(),
                (None, None) => {
                    return Err(window_failure(format!(
                        "{} has no file yet; give the path to save it to",
                        window.name
                    )));
                }
                (Some(_), _) => absolute("to save to")?,
            };
            if window.file.as_ref() != Some(&path) {
                let holder = windows
                    .list()
                    .into_iter()
                    .find(|w| w.id != window.id && w.file.as_ref() == Some(&path));
                if let Some(holder) = holder {
                    return Err(window_failure(format!(
                        "syntax window {} holds {}; save that window or close it first",
                        holder.name,
                        path.display()
                    )));
                }
                if path.exists() && !request.overwrite {
                    return Err(window_failure(format!(
                        "{} exists and is not the file of {}; say overwrite to replace it",
                        path.display(),
                        window.name
                    )));
                }
            }
            let text = windows.text_mut(window.id).expect("listed").clone();
            std::fs::write(&path, text)
                .map_err(|e| window_failure(format!("{}: {e}", path.display())))?;
            windows.saved(window.id, path.clone());
            json_reply(serde_json::json!({ "window": info(windows, window.id), "saved": path }))
        }
        _ => Err(window_failure("not a request about syntax windows")),
    }
}

/// The window a RunWindow request names, and its text.
pub fn window_source(
    windows: &mut impl SyntaxWindows,
    request: &Request,
) -> Result<(SyntaxInfo, String), Vec<Failure>> {
    let window = find(windows, request.window.as_deref())?;
    let text = windows.text_mut(window.id).expect("listed").clone();
    Ok((window, text))
}

/// The local time for a trail, as 14:32, or with `date` as 2026-11-12 14:32.
pub fn stamp(date: bool) -> String {
    let format = if date { "%Y-%m-%d %H:%M" } else { "%H:%M" };
    chrono::Local::now().format(format).to_string()
}

/// The comment and syntax of one run in a trail.
pub fn trail_block(stamp: &str, syntax: &str) -> String {
    format!("* {AGENT} {stamp}.\n{}\n", syntax.trim_end())
}

/// The syntax of the last run of `session`, as it ran: only its lines, for a line range.
pub fn last_syntax(session: &Session) -> String {
    session
        .log
        .last()
        .map(|e| e.syntax.clone())
        .unwrap_or_default()
}

/// Adds the syntax of a run to the Agent window, which opens when there is none (the person may
/// have closed it). Returns where it went, for the reply.
pub fn keep_trail(windows: &mut impl SyntaxWindows, syntax: &str) -> String {
    let stamp = stamp(false);
    let agent = *windows.agent();
    let id = match agent {
        Some(id) if windows.text_mut(id).is_some() => id,
        _ => {
            let id = windows.create(Some(AGENT.into()), String::new(), None, false);
            *windows.agent() = Some(id);
            id
        }
    };
    let text = windows.text_mut(id).expect("just made");
    let kept = text.trim_end().len();
    text.truncate(kept);
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str(&trail_block(&stamp, syntax));
    let name = info(windows, id).map_or_else(|| AGENT.to_string(), |w| w.name);
    format!("syntax window {name}")
}

/// A window without a display: a session and syntax windows held as text. The tests drive the
/// command line and the MCP server against it. The real window answers the same requests with
/// the same functions; it also widens a line range to whole commands, which this one does not.
#[derive(Default)]
pub struct Desk {
    pub session: Session,
    scripts: Vec<Script>,
    agent: Option<u64>,
    untitled: usize,
}

struct Script {
    id: u64,
    name: String,
    text: String,
    saved: String,
    file: Option<PathBuf>,
}

impl SyntaxWindows for Desk {
    fn list(&self) -> Vec<SyntaxInfo> {
        let last = self.scripts.iter().rev().find(|s| Some(s.id) != self.agent);
        self.scripts
            .iter()
            .map(|s| SyntaxInfo {
                id: s.id,
                name: s
                    .file
                    .as_deref()
                    .and_then(Path::file_name)
                    .map_or_else(|| s.name.clone(), |n| n.to_string_lossy().into_owned()),
                file: s.file.clone(),
                unsaved: s.text != s.saved,
                lines: line_count(&s.text),
                agent: self.agent == Some(s.id),
                designated: last.is_some_and(|l| l.id == s.id),
                focused: false,
            })
            .collect()
    }

    fn text_mut(&mut self, id: u64) -> Option<&mut String> {
        self.scripts
            .iter_mut()
            .find(|s| s.id == id)
            .map(|s| &mut s.text)
    }

    fn create(
        &mut self,
        name: Option<String>,
        text: String,
        file: Option<PathBuf>,
        _: bool,
    ) -> u64 {
        let id = self.scripts.last().map_or(1, |s| s.id + 1);
        let name = name.unwrap_or_else(|| {
            self.untitled += 1;
            format!("Syntax{}", self.untitled)
        });
        let saved = if file.is_some() {
            text.clone()
        } else {
            String::new()
        };
        self.scripts.push(Script {
            id,
            name,
            text,
            saved,
            file,
        });
        id
    }

    fn saved(&mut self, id: u64, file: PathBuf) {
        if let Some(s) = self.scripts.iter_mut().find(|s| s.id == id) {
            s.saved = s.text.clone();
            s.file = Some(file);
        }
    }

    fn agent(&mut self) -> &mut Option<u64> {
        &mut self.agent
    }
}

impl Desk {
    /// Answers a request as the window does.
    pub fn answer(&mut self, request: &Request) -> Response {
        match request.action {
            Action::Run => {
                request.enter_folder();
                let mut result = self.session.run(&request.source, request.lines);
                if let Ok(result) = &mut result {
                    let syntax = last_syntax(&self.session);
                    result.trail = Some(keep_trail(self, &syntax));
                }
                reply(&mut result, request)
            }
            Action::RunWindow => {
                let (window, text) = window_source(self, request)?;
                let mut result = self.session.run(&text, request.lines);
                if let Ok(result) = &mut result {
                    // the window's own text is the trail of this run
                    result.trail = Some(format!("syntax window {}", window.name));
                }
                reply(&mut result, request)
            }
            Action::Check | Action::Describe => answer(&mut self.session, request),
            _ => answer_windows(self, request),
        }
    }
}

/// A listening window. Dropping it removes the endpoint file, if it is still this one's.
pub struct Server {
    file: PathBuf,
    token: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        if std::fs::read_to_string(&self.file).is_ok_and(|text| text.contains(&self.token)) {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

/// Listens on a random port of 127.0.0.1, writes the endpoint to `file` and answers each
/// request with `handler`, one connection at a time, on a thread of its own.
pub fn serve(
    file: PathBuf,
    mut handler: impl FnMut(Request) -> Response + Send + 'static,
) -> io::Result<Server> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let token = new_token();
    let endpoint = Endpoint {
        address: listener.local_addr()?,
        token: token.clone(),
    };
    write_private(&file, &serde_json::to_string(&endpoint)?)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = connection(stream, &endpoint.token, &mut handler);
        }
    });
    Ok(Server { file, token })
}

fn connection(
    stream: TcpStream,
    token: &str,
    handler: &mut impl FnMut(Request) -> Response,
) -> io::Result<()> {
    // a client that connects and sends nothing must not hold up the next one for long
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut line = String::new();
    BufReader::new((&stream).take(MAX_REQUEST)).read_line(&mut line)?;
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) if same(&request.token, token) => handler(request),
        Ok(_) => Err(vec![Failure::plain("session", "refused: wrong token")]),
        Err(e) => Err(vec![Failure::plain("session", format!("bad request: {e}"))]),
    };
    let mut text = serde_json::to_string(&response)?;
    text.push('\n');
    (&stream).write_all(text.as_bytes())
}

/// Sends a request to the window named in `file` and waits for the reply, however long the run.
pub fn send(file: &Path, mut request: Request) -> Response {
    let not_running = |why: String| {
        let message = format!("no Statscraft window is running ({why}); start statscraft-gui");
        vec![Failure::plain("session", message)]
    };
    let text = std::fs::read_to_string(file)
        .map_err(|_| not_running(format!("{} not found", file.display())))?;
    let endpoint: Endpoint =
        serde_json::from_str(&text).map_err(|e| not_running(format!("{}: {e}", file.display())))?;
    if !endpoint.address.ip().is_loopback() {
        return Err(not_running(format!(
            "{} names {}, not this computer",
            file.display(),
            endpoint.address
        )));
    }
    request.token = endpoint.token;
    let stream = TcpStream::connect_timeout(&endpoint.address, Duration::from_secs(5))
        .map_err(|e| not_running(format!("{}: {e}", endpoint.address)))?;
    let failed = |e: String| vec![Failure::plain("session", e)];
    let mut line = serde_json::to_string(&request).map_err(|e| failed(e.to_string()))?;
    line.push('\n');
    (&stream)
        .write_all(line.as_bytes())
        .map_err(|e| failed(e.to_string()))?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|e| failed(e.to_string()))?;
    let response: Response = serde_json::from_str(&reply)
        .map_err(|_| failed("the window closed the connection without a reply".into()))?;
    response.map_err(|errors| {
        // a window from before this request was added cannot read it
        let unknown = |e: &Failure| e.message.starts_with("bad request: unknown variant");
        if errors.iter().any(unknown) {
            let message = "the Statscraft window is older than this command line and does not \
                 know this request; update Statscraft and start the window again";
            failed(message.into())
        } else {
            errors
        }
    })
}

/// Whether a window listens at the endpoint `file` names. It only connects, so it does not wait
/// behind a run the window is busy with.
pub fn running(file: &Path) -> bool {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Endpoint>(&text).ok())
        .is_some_and(|endpoint| {
            endpoint.address.ip().is_loopback()
                && TcpStream::connect_timeout(&endpoint.address, Duration::from_secs(1)).is_ok()
        })
}

/// Compares in time independent of where the strings differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// 256 random bits as hex. The standard library seeds `RandomState` from the operating
/// system's random source; SipHash under that secret key gives unpredictable output.
fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher, RandomState};
    (0..4)
        .map(|i: u64| {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(i);
            hasher.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// Writes `text` to `path` readable by the owner only, through a rename so a reader never sees
/// half a file.
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&temp)?.write_all(text.as_bytes())?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_differ_and_compare_exactly() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(same(&a, &a.clone()));
        assert!(!same(&a, &b) && !same(&a, &a[..63]));
    }

    fn edited(text: &str, edit: Edit) -> (String, Option<(u32, u32)>) {
        let mut text = text.to_string();
        let lines = apply_edit(&mut text, &edit).unwrap();
        (text, lines)
    }

    #[test]
    fn edits_replace_insert_and_append_whole_lines() {
        let text = "GET FILE='a.sav'.\nFREQ x.\nDESC y.\n";
        let replace = |lines, text: &str| Edit::Replace {
            lines,
            text: text.into(),
        };
        assert_eq!(
            edited(text, replace((2, 2), "FREQ x z.\nFREQ w.")),
            (
                "GET FILE='a.sav'.\nFREQ x z.\nFREQ w.\nDESC y.\n".into(),
                Some((2, 3))
            )
        );
        // an empty text deletes the lines
        assert_eq!(
            edited(text, replace((1, 2), "")),
            ("DESC y.\n".into(), None)
        );
        let insert = |before, text: &str| Edit::Insert {
            before,
            text: text.into(),
        };
        assert_eq!(
            edited(text, insert(1, "* first.")).0,
            "* first.\nGET FILE='a.sav'.\nFREQ x.\nDESC y.\n"
        );
        assert_eq!(edited(text, insert(4, "SHOW.\n")).1, Some((4, 4)));
        let append = Edit::Append {
            text: "SHOW.".into(),
        };
        assert_eq!(edited("", append.clone()), ("SHOW.\n".into(), Some((1, 1))));
        // a text without a final line break keeps it that way
        assert_eq!(edited("A.", append), ("A.\nSHOW.".into(), Some((2, 2))));
        let all = Edit::ReplaceAll {
            text: "X.\nY.\n".into(),
        };
        assert_eq!(edited(text, all), ("X.\nY.\n".into(), Some((1, 2))));
        for bad in [
            replace((0, 1), "x"),
            replace((3, 4), "x"),
            replace((2, 1), "x"),
            insert(5, "x"),
        ] {
            let mut copy = text.to_string();
            let error = apply_edit(&mut copy, &bad).unwrap_err();
            assert!(error[0].message.contains("3 lines"), "{bad:?}: {error:?}");
            assert_eq!(copy, text, "a refused edit changes nothing");
        }
    }

    #[test]
    fn line_numbers_are_those_of_a_run() {
        assert_eq!(numbered("A.\r\n\nB.\n"), "1  A.\n2  \n3  B.\n");
        assert_eq!(numbered(""), "");
        let long = "X.\n".repeat(10);
        assert!(numbered(&long).starts_with(" 1  X.\n") && numbered(&long).ends_with("10  X.\n"));
    }

    fn request(action: Action) -> Request {
        Request {
            action,
            json: true,
            ..Request::default()
        }
    }

    fn reply_json(response: Response) -> serde_json::Value {
        serde_json::from_str(&response.unwrap()).unwrap()
    }

    #[test]
    fn the_desk_keeps_a_trail_of_runs_and_none_of_failures() {
        let mut desk = Desk::default();
        let run = |desk: &mut Desk, source: &str| {
            desk.answer(&Request {
                source: source.into(),
                ..request(Action::Run)
            })
        };
        let first = reply_json(run(&mut desk, "SET SEED=3."));
        assert_eq!(first["trail"], "syntax window Agent");
        assert!(run(&mut desk, "FREQUENCIS x.").is_err());
        reply_json(run(&mut desk, "SET SEED=4.\nSHOW SEED."));
        let windows = reply_json(desk.answer(&request(Action::Windows)));
        assert_eq!(windows["windows"][0]["name"], "Agent");
        assert_eq!(windows["windows"][0]["agent"], true);
        assert_eq!(windows["windows"][0]["designated"], false);
        let text = desk.text_mut(1).unwrap().clone();
        let blocks: Vec<&str> = text.split("\n\n").collect();
        assert_eq!(blocks.len(), 2, "{text}");
        assert!(blocks[0].starts_with("* Agent ") && blocks[0].ends_with(".\nSET SEED=3."));
        assert!(blocks[1].ends_with(".\nSET SEED=4.\nSHOW SEED.\n"));
        assert!(!text.contains("FREQUENCIS"));
        // a run of lines keeps only those lines
        desk.answer(&Request {
            source: "SET SEED=5.\nSET SEED=6.".into(),
            lines: Some((2, 2)),
            ..request(Action::Run)
        })
        .unwrap();
        assert!(desk.text_mut(1).unwrap().ends_with(".\nSET SEED=6.\n"));
    }

    #[test]
    fn windows_are_found_by_id_or_name_and_names_do_not_repeat() {
        let mut desk = Desk::default();
        let new = |desk: &mut Desk, name: Option<&str>| {
            desk.answer(&Request {
                name: name.map(str::to_string),
                source: "SHOW.".into(),
                ..request(Action::New)
            })
        };
        let made = reply_json(new(&mut desk, None));
        assert_eq!(made["window"]["name"], "Syntax1");
        assert_eq!(made["window"]["unsaved"], true);
        reply_json(new(&mut desk, Some("Recodes")));
        assert!(new(&mut desk, Some("recodes")).is_err());
        assert!(new(&mut desk, Some("12")).is_err());
        assert_eq!(find(&desk, Some("2")).unwrap().name, "Recodes");
        assert_eq!(find(&desk, Some("RECODES")).unwrap().id, 2);
        let missing = find(&desk, Some("nope")).unwrap_err();
        assert_eq!(
            missing[0].message,
            "no syntax window nope; open: 1 Syntax1, 2 Recodes"
        );
        assert!(find(&desk, None).is_err());
    }

    #[test]
    fn save_writes_its_own_file_and_replaces_another_only_when_told() {
        let dir = std::env::temp_dir().join(format!("statscraft-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let other = dir.join("other.sps");
        std::fs::write(&other, "KEEP.").unwrap();
        let mut desk = Desk::default();
        desk.create(None, "SHOW.".into(), None, true);
        let save = |desk: &mut Desk, path: Option<&Path>, overwrite| {
            desk.answer(&Request {
                window: Some("1".into()),
                path: path.map(Path::to_path_buf),
                overwrite,
                ..request(Action::Save)
            })
        };
        assert!(
            save(&mut desk, None, false).unwrap_err()[0]
                .message
                .contains("no file yet")
        );
        assert!(save(&mut desk, Some(Path::new("rel.sps")), false).is_err());
        let refused = save(&mut desk, Some(&other), false).unwrap_err();
        assert!(refused[0].message.contains("say overwrite"), "{refused:?}");
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "KEEP.");
        let own = dir.join("own.sps");
        let saved = reply_json(save(&mut desk, Some(&own), false));
        assert_eq!(saved["window"]["name"], "own.sps");
        assert_eq!(saved["window"]["unsaved"], false);
        desk.text_mut(1).unwrap().push_str("\nSHOW N.");
        // its own file again, without a path
        reply_json(save(&mut desk, None, false));
        assert_eq!(std::fs::read_to_string(&own).unwrap(), "SHOW.\nSHOW N.");
        // a file another window holds is never replaced
        desk.create(None, String::new(), Some(other.clone()), true);
        let held = save(&mut desk, Some(&other), true).unwrap_err();
        assert!(held[0].message.contains("holds"), "{held:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
