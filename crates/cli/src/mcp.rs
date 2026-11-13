//! `statscraft mcp`: the command line's describe, help, check and run as MCP tools over standard
//! input and output, and the syntax windows of the open window. Standard output carries the
//! protocol only; messages go to standard error.

use crate::{RunArgs, XlsxArgs};
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, PromptMessage, Role, ServerCapabilities,
    ServerConfig,
};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{ErrorData, ServerHandler, ServiceExt, prompt, prompt_handler, prompt_router};
use rmcp::{tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};
use statscraft_connect::{WSL_DISTRO_ENV, linux_to_windows, windows_to_linux};
use statscraft_engine::live::{self, Action, Edit, Request};
use statscraft_engine::session::{Failure, RunResult};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

/// One tool call at a time: a call moves the process to its folder and may write a session.
static BUSY: Mutex<()> = Mutex::new(());

const INSTRUCTIONS: &str = "\
Statscraft runs statistics command syntax (GET FILE, COMPUTE, RECODE, FREQUENCIES, CROSSTABS, \
CTABLES and so on) on the user's data files. Work in this loop:
1. Call describe on the data file to learn variable names, labels, value codes and missing values.
2. Call list_commands, then command_help for each command you plan to use; use only what the help shows.
3. Call check_syntax; it runs on a copy and writes nothing.
4. Call run_syntax in small steps; the session keeps the data, settings and macros between calls.
5. Read ok, errors (with line, column, expected and hint) and the warnings of each command. Fix the \
line an error points to and run the same syntax again; a failed run changes nothing.
6. Report numbers exactly as the result tables give them. Never compute statistics yourself.
Nothing is written to disk unless the syntax says so (SAVE, AGGREGATE OUTFILE, the xlsx argument). \
When the Statscraft window is open, calls go to it by default and the person sees every result.
Every successful run leaves its syntax for the person: in the open window it is added to the \
syntax window Agent, in a file session to the file the result names under trail. Say where it is.
Syntax windows (only with the window open): when the person asks you to fix, explain or extend \
their syntax, call list_syntax_windows and read_syntax, say what you would change, make the \
change with edit_syntax and run it with run_syntax and window (and lines), as their own Run \
does. When the person works in a syntax window, prefer putting the syntax there and running it \
from there. save_syntax is the only one of these tools that writes a file.
Charts: procedures draw their own. For a chart no procedure makes, call add_chart with chart JSON \
as run_syntax returns it, or a simple Vega-Lite specification; it lands in the output like any \
other chart. Draw only numbers the result tables gave.
Paths: window_status gives the platform the server runs on and, when the agent was started inside \
WSL, wsl_distro. Tool arguments then take Linux paths too; the server translates them. File names \
inside the syntax (GET FILE, SAVE and so on) are read as written: on a Windows server write them \
as Windows paths, such as C:\\data\\survey.sav, or \\\\wsl$\\DISTRO\\home\\... for a file inside WSL.";

/// What `describe` takes.
#[derive(Deserialize, JsonSchema)]
struct DescribeInput {
    /// Path of a .sav or .zsav file to describe; give an absolute path. Leave it out to describe a session.
    file: Option<String>,
    /// Session to describe instead of a file: its active dataset and what it keeps. "gui" is the
    /// open Statscraft window. Default: "gui" when a window is open, else "work".
    session: Option<String>,
}

/// What `command_help` takes.
#[derive(Deserialize, JsonSchema)]
struct HelpInput {
    /// A command name as list_commands shows it, for example FREQUENCIES or "VALUE LABELS".
    command: String,
}

/// What `check_syntax` takes.
#[derive(Deserialize, JsonSchema)]
struct SyntaxInput {
    /// Syntax text: one or more commands, each ending with a period. Give this or file.
    syntax: Option<String>,
    /// Absolute path of a .sps syntax file. Give this or syntax.
    file: Option<String>,
    /// Only the commands within these lines, inclusive, for example "40-55". Line numbers stay
    /// those of the whole text or file.
    lines: Option<String>,
    /// Session that keeps the data, settings and macros between calls. "gui" is the open
    /// Statscraft window. Default: "gui" when a window is open, else "work".
    session: Option<String>,
    /// Absolute folder that relative file names in the syntax start from. Default: the user's
    /// home folder.
    folder: Option<String>,
    /// Also give every chart as a Vega-Lite specification (member vega_lite of the chart), with
    /// the numbers inline. Default: false; the chart's own description is always there.
    vega_lite: Option<bool>,
}

/// What `run_syntax` takes.
#[derive(Deserialize, JsonSchema)]
struct RunInput {
    #[serde(flatten)]
    syntax: SyntaxInput,
    /// Run the text of this syntax window of the open window instead of syntax or file: its id
    /// or name from list_syntax_windows. With lines, only the commands those lines touch, as the
    /// window's own Run does. The window's text is the record of the run.
    window: Option<String>,
    /// Also write every table and chart of this run to this Excel workbook (.xlsx), one sheet per
    /// table, a chart as its data table with an editable chart beside it;
    /// an existing file is replaced. A relative path starts from folder.
    xlsx: Option<String>,
}

/// What `add_chart` takes.
#[derive(Deserialize, JsonSchema)]
struct ChartInput {
    /// The chart: an object as run_syntax gives it in charts (the same JSON reads back), or a
    /// Vega-Lite specification of one view with inline data.values, mark bar, line, area, point
    /// or arc and encodings x, y, color, theta and xOffset; no aggregate, bin or transform.
    chart: serde_json::Map<String, Value>,
    /// Session whose output gets the chart. "gui" is the open Statscraft window. Default: "gui"
    /// when a window is open, else "work". A file session keeps no output between calls: to
    /// write the chart to a file there, run BEGIN CHART with OUTPUT EXPORT in run_syntax.
    session: Option<String>,
    /// Also give the chart as Statscraft drew it as a Vega-Lite specification. Default: false.
    vega_lite: Option<bool>,
}

/// Names a syntax window.
#[derive(Deserialize, JsonSchema)]
struct WindowInput {
    /// The syntax window: its id or its name, as list_syntax_windows shows them.
    window: String,
}

/// What `edit_syntax` takes.
#[derive(Deserialize, JsonSchema)]
struct EditInput {
    /// The syntax window: its id or its name, as list_syntax_windows shows them.
    window: String,
    /// "replace": the lines in lines get text (an empty text deletes them); "insert": text goes
    /// before line before; "append": text goes after the last line; "replace_all": text becomes
    /// the whole content.
    mode: EditMode,
    /// The new text, one or more lines.
    text: String,
    /// For replace: the lines to replace, for example "12-14", as read_syntax numbers them.
    lines: Option<String>,
    /// For insert: the line the text goes before; one more than the last line adds it at the end.
    before: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum EditMode {
    Replace,
    Insert,
    Append,
    ReplaceAll,
}

/// What `new_syntax_window` takes.
#[derive(Deserialize, JsonSchema)]
struct NewInput {
    /// The window's name. Default: the next free Syntax1, Syntax2 and so on.
    name: Option<String>,
    /// The text to start with. Default: empty.
    text: Option<String>,
}

/// What `open_syntax_file` takes.
#[derive(Deserialize, JsonSchema)]
struct OpenInput {
    /// Absolute path of a .sps syntax file.
    path: String,
}

/// What `save_syntax` takes.
#[derive(Deserialize, JsonSchema)]
struct SaveInput {
    /// The syntax window: its id or its name, as list_syntax_windows shows them.
    window: String,
    /// Absolute path to save to. Default: the window's own file; a window without one needs it.
    path: Option<String>,
    /// Replace path even though it is a file other than the window's own. Default: false; ask
    /// the person first.
    overwrite: Option<bool>,
}

#[derive(Clone)]
struct Server {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

/// Starts the server and answers until the client closes standard input.
pub fn serve() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let _ = crate::SESSIONS.set(sessions_folder());
    let served = runtime.block_on(async {
        let server = Server {
            tool_router: Server::tool_router(),
            prompt_router: Server::prompt_router(),
        };
        server
            .serve(rmcp::transport::stdio())
            .await?
            .waiting()
            .await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    });
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The per-user data folder for file sessions: clients start servers in any folder, so the
/// command line's `.statscraft/sessions` would scatter them.
fn sessions_folder() -> PathBuf {
    let env = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let home = std::env::home_dir().unwrap_or_else(std::env::temp_dir);
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join("Statscraft")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Statscraft")
    } else {
        env("XDG_DATA_HOME")
            .unwrap_or_else(|| home.join(".local/share"))
            .join("statscraft")
    };
    base.join("sessions")
}

/// The WSL distribution of the agent that started this server, which `statscraft connect` names
/// in the entry of an agent inside WSL. Only a Windows server has a use for it.
fn wsl_distro() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    std::env::var(WSL_DISTRO_ENV).ok().filter(|d| !d.is_empty())
}

/// A path argument as this server reads it: from an agent inside WSL, a Linux path becomes the
/// Windows path of the same file. The syntax text itself is never changed.
fn local(path: String) -> String {
    match wsl_distro() {
        Some(distro) => linux_to_windows(&path, &distro),
        None => path,
    }
}

/// A path of a result as written, and for an agent inside WSL also as it reaches it there.
fn both(path: &str) -> (String, Option<String>) {
    let linux = wsl_distro()
        .map(|_| windows_to_linux(path))
        .filter(|linux| linux != path);
    let shown = match &linux {
        Some(linux) => format!("{path} (in WSL: {linux})"),
        None => path.to_string(),
    };
    (shown, linux)
}

/// The session a call names, else the window when one is open, else `work`.
fn session_or_default(session: Option<String>) -> String {
    session.unwrap_or_else(|| {
        let window = live::running(&live::endpoint_file());
        if window { live::SESSION } else { "work" }.to_string()
    })
}

/// Runs `job` on a blocking thread, one call at a time.
async fn blocking<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(move || {
        let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        job()
    })
    .await
    .expect("tool call panicked")
}

/// Errors in the syntax or while running it are a normal result for the agent to fix; the
/// others (arguments, files, the window, the session) mean the tool itself could not work.
fn failed(errors: Vec<Failure>) -> CallToolResult {
    let mut text = String::from("ok: false\n");
    for e in &errors {
        text.push_str(&format!("error: {}\n", e.text()));
    }
    let tool_level = errors
        .iter()
        .any(|e| !matches!(e.kind.as_str(), "syntax" | "run"));
    let value = json!({ "ok": false, "errors": errors, "saved": false });
    let mut result = if tool_level {
        CallToolResult::structured_error(value)
    } else {
        CallToolResult::structured(value)
    };
    result.content = vec![ContentBlock::text(text)];
    result
}

fn usage(message: &str) -> CallToolResult {
    failed(vec![Failure::plain("usage", message)])
}

/// A JSON reply as the command line prints it with `--json`.
fn json_reply(text: &str) -> CallToolResult {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => CallToolResult::structured(value),
        Err(e) => usage(&format!("reply is not JSON: {e}")),
    }
}

fn text_reply(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

fn check_or_run(input: SyntaxInput, check_only: bool, xlsx: Option<String>) -> CallToolResult {
    let input = SyntaxInput {
        file: input.file.map(local),
        folder: input.folder.map(local),
        ..input
    };
    let xlsx = xlsx.map(local);
    if input.syntax.is_some() == input.file.is_some() {
        return usage("give either syntax or file");
    }
    // also keeps out "-", which would read standard input: it carries the protocol
    if input
        .file
        .as_deref()
        .is_some_and(|f| !std::path::Path::new(f).is_absolute())
    {
        return usage("file must be an absolute path");
    }
    let folder = match input.folder.map(PathBuf::from) {
        Some(folder) if !folder.is_absolute() => return usage("folder must be an absolute path"),
        Some(folder) => folder,
        None => std::env::home_dir().unwrap_or_else(std::env::temp_dir),
    };
    if let Err(e) = std::env::set_current_dir(&folder) {
        return usage(&format!("folder {}: {e}", folder.display()));
    }
    let args = RunArgs {
        file: input.file,
        text: input.syntax,
        session: Some(session_or_default(input.session)),
        lines: input.lines,
        json: true,
        vega_lite: input.vega_lite.unwrap_or(false),
        encoding: None,
    };
    let xlsx = XlsxArgs {
        xlsx: xlsx.map(PathBuf::from),
        xlsx_one_sheet: false,
    };
    match crate::run_result(&args, check_only, &xlsx) {
        Ok(result) => run_reply(&result, check_only, args.vega_lite, xlsx.xlsx.as_deref()),
        Err(errors) => failed(errors),
    }
}

/// The JSON of a run as structured content, and as text its tables, a summary and where its
/// syntax was kept. For an agent inside WSL, the trail file and the workbook come in both forms.
fn run_reply(
    result: &RunResult,
    check_only: bool,
    vega_lite: bool,
    xlsx: Option<&Path>,
) -> CallToolResult {
    let mut value = result.to_json(vega_lite);
    let output = result.output_with(false, check_only, vega_lite);
    let mut text = format!("ok: {}\n", result.ok);
    if let Some(trail) = &result.trail {
        let (shown, linux) = both(trail);
        text.push_str(&format!("syntax kept in: {shown}\n"));
        if let Some(linux) = linux {
            value["trail_wsl"] = linux.into();
        }
    }
    let workbook = xlsx.filter(|_| result.ok && !check_only);
    if let Some(path) = workbook.and_then(|p| std::path::absolute(p).ok()) {
        let path = path.to_string_lossy();
        if let (shown, Some(linux)) = both(&path) {
            text.push_str(&format!("workbook: {shown}\n"));
            value["xlsx"] = path.as_ref().into();
            value["xlsx_wsl"] = linux.into();
        }
    }
    let mut reply = CallToolResult::structured(value);
    text.push_str(&output);
    reply.content = vec![ContentBlock::text(text)];
    reply
}

/// Runs the text of a syntax window of the open window, or its lines.
fn run_window(input: RunInput, window: String) -> CallToolResult {
    let syntax = input.syntax;
    if syntax.syntax.is_some() || syntax.file.is_some() {
        return usage("give window, or syntax or file, not both");
    }
    if syntax
        .session
        .as_deref()
        .is_some_and(|s| s != live::SESSION)
    {
        return usage("a syntax window runs in the open window: leave out session");
    }
    let lines = match syntax.lines.as_deref().map(crate::parse_lines).transpose() {
        Ok(lines) => lines,
        Err(errors) => return failed(errors),
    };
    let xlsx = XlsxArgs {
        xlsx: input.xlsx.map(local).map(PathBuf::from),
        xlsx_one_sheet: false,
    };
    // for a relative xlsx, and for the relative names of a window without a file: one with a
    // file starts from its own folder, as its Run does
    let folder = syntax.folder.map(local).map(PathBuf::from);
    let folder = folder.unwrap_or_else(|| std::env::home_dir().unwrap_or_else(std::env::temp_dir));
    if !folder.is_absolute() || std::env::set_current_dir(&folder).is_err() {
        return usage("folder must be an absolute folder that exists");
    }
    let request = Request {
        action: Action::RunWindow,
        window: Some(window),
        lines,
        json: true,
        cwd: Some(folder),
        ..Request::default()
    };
    match crate::run_in_window(request, &xlsx) {
        Ok(result) => run_reply(
            &result,
            false,
            syntax.vega_lite.unwrap_or(false),
            xlsx.xlsx.as_deref(),
        ),
        Err(errors) => failed(errors),
    }
}

/// Sends a request about syntax windows to the open window.
fn window_call(request: Request) -> CallToolResult {
    let read = request.action == Action::Read;
    match live::send(&live::endpoint_file(), request) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let shown = if read {
                    let window = &value["window"];
                    format!(
                        "{} (id {}, {} lines{})\n{}",
                        window["name"].as_str().unwrap_or_default(),
                        window["id"],
                        window["lines"],
                        if window["unsaved"] == true {
                            ", unsaved changes"
                        } else {
                            ""
                        },
                        value["text"].as_str().unwrap_or_default()
                    )
                } else {
                    serde_json::to_string_pretty(&value).expect("plain data")
                };
                let mut reply = CallToolResult::structured(value);
                reply.content = vec![ContentBlock::text(shown)];
                reply
            }
            Err(e) => usage(&format!("reply is not JSON: {e}")),
        },
        Err(errors) => failed(errors),
    }
}

#[tool_router]
impl Server {
    /// Shows the dictionary of a data file or of a session: every variable with its type,
    /// label, value labels, missing values and valid count, the case count, the weight variable
    /// and multiple response sets. For a session it also shows what the session keeps: settings,
    /// macros, named datasets, output documents. Call it before writing syntax so that names and
    /// codes are right, and again after the data change.
    #[tool(
        title = "Describe data",
        annotations(
            title = "Describe data",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn describe(
        &self,
        Parameters(input): Parameters<DescribeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            let session = match input.file {
                Some(_) if input.session.is_some() => {
                    return usage("give either file or session");
                }
                Some(_) => None,
                None => Some(session_or_default(input.session)),
            };
            match crate::describe(input.file.map(local), session, true) {
                Ok(text) => json_reply(&text),
                Err(errors) => failed(errors),
            }
        })
        .await)
    }

    /// Lists every command Statscraft supports, each with a one-line summary, and the general
    /// rules of the syntax. Look here before writing syntax; a command that is not listed fails.
    #[tool(
        title = "List commands",
        annotations(
            title = "List commands",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_commands(&self) -> Result<CallToolResult, ErrorData> {
        Ok(match crate::help("") {
            Ok(text) => text_reply(text),
            Err(errors) => failed(errors),
        })
    }

    /// Shows the syntax of one command: its subcommands and keywords, defaults, rules and an
    /// example. Use only what it shows.
    #[tool(
        title = "Command help",
        annotations(title = "Command help", read_only_hint = true, open_world_hint = false)
    )]
    async fn command_help(
        &self,
        Parameters(input): Parameters<HelpInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(match crate::help(input.command.trim()) {
            Ok(text) => text_reply(text),
            Err(errors) => failed(errors),
        })
    }

    /// Checks syntax without keeping or writing anything: every command is parsed and run on a
    /// copy of the session's data, so unknown variables and type errors are found too. The
    /// result has ok, errors (kind, message, line, column, expected, hint) and the warnings and
    /// tables of each command.
    #[tool(
        title = "Check syntax",
        annotations(title = "Check syntax", read_only_hint = true, open_world_hint = false)
    )]
    async fn check_syntax(
        &self,
        Parameters(input): Parameters<SyntaxInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || check_or_run(input, true, None)).await)
    }

    /// Runs syntax in a session and keeps the result there, so the next call builds on it. The
    /// result has ok, the tables of each command with raw numbers, the warnings, the variables
    /// added, changed and removed, and the case counts. A run is all or nothing: when ok is
    /// false nothing changed; fix the line the error names and run again. Syntax can write
    /// files (SAVE, AGGREGATE OUTFILE, OUTPUT EXPORT), so run only what the person wants.
    /// A successful run keeps its syntax where trail says: the Agent syntax window of the open
    /// window, or syntax.sps of a file session. With window, it runs the text of that syntax
    /// window (or its lines) instead, which is then the record itself.
    #[tool(
        title = "Run syntax",
        annotations(
            title = "Run syntax",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn run_syntax(
        &self,
        Parameters(mut input): Parameters<RunInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || match input.window.take() {
            Some(window) => run_window(input, window),
            None => check_or_run(input.syntax, false, input.xlsx),
        })
        .await)
    }

    /// Adds a chart of your own to the output, drawn the Statscraft way: in the open window it
    /// appears in the designated output window, is saved with it and exported by OUTPUT
    /// EXPORT, Copy and Open in the office programs, like a procedure's chart. Give the chart
    /// JSON that run_syntax returns (change labels, numbers or kind) or a simple Vega-Lite
    /// specification. Values are drawn as given: nothing is aggregated. The chart is checked
    /// first; an error says what to fix. It runs as the syntax BEGIN CHART (a Statscraft
    /// extension), kept in the trail like any run; the result names the output document.
    /// For statistics, run the procedure instead: compute nothing yourself.
    #[tool(
        title = "Add chart",
        annotations(
            title = "Add chart",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_chart(
        &self,
        Parameters(input): Parameters<ChartInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            let json = serde_json::to_string_pretty(&input.chart).expect("plain data");
            let syntax = SyntaxInput {
                syntax: Some(crate::chart_syntax(&json)),
                file: None,
                lines: None,
                session: input.session,
                folder: None,
                vega_lite: input.vega_lite,
            };
            let mut reply = check_or_run(syntax, false, None);
            let document = reply
                .structured_content
                .as_ref()
                .and_then(|v| v["commands"][0]["document"].as_str())
                .map(str::to_string);
            if let (Some(document), Some(ContentBlock::Text(t))) =
                (document, reply.content.first_mut())
            {
                t.text = format!("chart added to output document {document}\n{}", t.text);
            }
            reply
        })
        .await)
    }

    /// Lists the syntax windows of the open Statscraft window: id, name, file, whether it has
    /// unsaved changes, its line count, whether it is the Agent window that keeps the syntax of
    /// your runs, the designated one (where pasted syntax goes) and the one the person used
    /// last. Fails when no window is open.
    #[tool(
        title = "List syntax windows",
        annotations(
            title = "List syntax windows",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_syntax_windows(&self) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(|| {
            window_call(Request {
                action: Action::Windows,
                ..Request::default()
            })
        })
        .await)
    }

    /// Reads the text of a syntax window in the open window, each line with its number, as
    /// the person sees it now, saved or not. Use the numbers for edit_syntax and for the
    /// lines of run_syntax.
    #[tool(
        title = "Read syntax window",
        annotations(
            title = "Read syntax window",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn read_syntax(
        &self,
        Parameters(input): Parameters<WindowInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            window_call(Request {
                action: Action::Read,
                window: Some(input.window),
                ..Request::default()
            })
        })
        .await)
    }

    /// Changes the text of a syntax window in the open window: replaces lines, inserts or
    /// appends text, or replaces all of it. The person sees the change at once and can undo it
    /// in the editor. Runs nothing and writes no file. The reply gives the lines the new text
    /// now takes, ready for run_syntax with window and lines. Read the window first: the
    /// person may have changed it.
    #[tool(
        title = "Edit syntax window",
        annotations(
            title = "Edit syntax window",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_syntax(
        &self,
        Parameters(input): Parameters<EditInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            let text = input.text;
            let edit = match input.mode {
                EditMode::Replace => {
                    let Some(lines) = input.lines else {
                        return usage("replace needs lines, for example \"12-14\"");
                    };
                    match crate::parse_lines(&lines) {
                        Ok(lines) => Edit::Replace { lines, text },
                        Err(errors) => return failed(errors),
                    }
                }
                EditMode::Insert => match input.before {
                    Some(before) => Edit::Insert { before, text },
                    None => return usage("insert needs before, the line the text goes before"),
                },
                EditMode::Append => Edit::Append { text },
                EditMode::ReplaceAll => Edit::ReplaceAll { text },
            };
            window_call(Request {
                action: Action::Edit,
                window: Some(input.window),
                edit: Some(edit),
                ..Request::default()
            })
        })
        .await)
    }

    /// Opens a new syntax window in the open Statscraft window, empty or with a text, where
    /// the person can read, change and run it. Writes no file.
    #[tool(
        title = "New syntax window",
        annotations(
            title = "New syntax window",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn new_syntax_window(
        &self,
        Parameters(input): Parameters<NewInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            window_call(Request {
                action: Action::New,
                name: input.name,
                source: input.text.unwrap_or_default(),
                ..Request::default()
            })
        })
        .await)
    }

    /// Opens a .sps syntax file in a syntax window of the open Statscraft window, or gives the
    /// window that has it open already. Runs nothing.
    #[tool(
        title = "Open syntax file",
        annotations(
            title = "Open syntax file",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn open_syntax_file(
        &self,
        Parameters(input): Parameters<OpenInput>,
    ) -> Result<CallToolResult, ErrorData> {
        // the window refuses a relative path
        Ok(blocking(move || {
            window_call(Request {
                action: Action::Open,
                path: Some(local(input.path).into()),
                ..Request::default()
            })
        })
        .await)
    }

    /// Saves a syntax window of the open Statscraft window to its own file, or to path. It
    /// refuses to replace any other existing file unless overwrite is true, and a file another
    /// syntax window holds. Save only when the person asks.
    #[tool(
        title = "Save syntax window",
        annotations(
            title = "Save syntax window",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn save_syntax(
        &self,
        Parameters(input): Parameters<SaveInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || {
            window_call(Request {
                action: Action::Save,
                window: Some(input.window),
                path: input.path.map(local).map(PathBuf::from),
                overwrite: input.overwrite.unwrap_or(false),
                ..Request::default()
            })
        })
        .await)
    }

    /// Says whether a Statscraft window is open on this computer. When one is, calls without a
    /// session go to it and the person sees each result appear; the reply then lists its
    /// active dataset, open datasets and output documents. It also gives the platform the
    /// server runs on (windows, macos, linux) and wsl_distro when the agent runs inside WSL:
    /// file names inside syntax must then be Windows paths.
    #[tool(
        title = "Window status",
        annotations(
            title = "Window status",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn window_status(&self) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(|| {
            let platform = std::env::consts::OS;
            let wsl_distro = wsl_distro();
            let file = live::endpoint_file();
            if !live::running(&file) {
                return CallToolResult::structured(json!({
                    "running": false,
                    "platform": platform,
                    "wsl_distro": wsl_distro,
                }));
            }
            let window = crate::describe(None, Some(live::SESSION.into()), true);
            let state = window.map_or(Value::Null, |text| {
                serde_json::from_str(&text).unwrap_or(Value::Null)
            });
            // a window without data or output yet has nothing to describe
            CallToolResult::structured(json!({
                "running": true,
                "window": state,
                "platform": platform,
                "wsl_distro": wsl_distro,
            }))
        })
        .await)
    }
}

#[prompt_router]
impl Server {
    /// The working loop for analysing data with Statscraft.
    #[prompt(name = "analysis")]
    async fn analysis(&self) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(Role::User, INSTRUCTIONS)]
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_prompts()
            .build();
        ServerConfig::new(capabilities)
            .with_server_info(
                Implementation::new("statscraft", env!("CARGO_PKG_VERSION"))
                    .with_title("Statscraft"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}
