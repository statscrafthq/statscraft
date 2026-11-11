//! The window as a live session: `statscraft run --session gui` sends syntax to a running
//! statscraft-gui, which runs it like its own Run button and replies with what the command line
//! prints. One JSON line each way over TCP on 127.0.0.1: the standard library has it on every
//! platform, where Unix sockets and named pipes would need a separate path for Windows.
//!
//! Discovery and security: the window listens on a random local port and writes the address and
//! a random token to `endpoint_file()`, readable only by the user. A request without the token
//! is refused, and the client sends the token only to a loopback address.

use crate::session::{Failure, RunResult, Session, describe_output};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The session name that means the running window.
pub const SESSION: &str = "gui";

/// Syntax larger than this is refused, so a stray client cannot fill the memory.
const MAX_REQUEST: u64 = 64 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Run,
    Check,
    Describe,
}

#[derive(Debug, Serialize, Deserialize)]
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
    session
        .dataset()
        .map(|d| describe_output(d, json))
        .ok_or_else(|| {
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
    serde_json::from_str(&reply)
        .map_err(|_| failed("the window closed the connection without a reply".into()))?
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
}
