//! `statscraft mcp`: the command line's describe, help, check and run as MCP tools over standard
//! input and output. Standard output carries the protocol only; messages go to standard error.

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
use statscraft_engine::live;
use statscraft_engine::session::Failure;
use std::path::PathBuf;
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
When the Statscraft window is open, calls go to it by default and the person sees every result.";

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
    /// Also write every table and chart of this run to this Excel workbook (.xlsx), one sheet per
    /// table, a chart as its data table with an editable chart beside it;
    /// an existing file is replaced. A relative path starts from folder.
    xlsx: Option<String>,
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
        Ok(result) => {
            let value = result.to_json(args.vega_lite);
            let mut reply = CallToolResult::structured(value);
            let output = result.output_with(false, check_only, args.vega_lite);
            let text = format!("ok: {}\n{}", result.ok, output);
            reply.content = vec![ContentBlock::text(text)];
            reply
        }
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
            match crate::describe(input.file, session, true) {
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
        Parameters(input): Parameters<RunInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(blocking(move || check_or_run(input.syntax, false, input.xlsx)).await)
    }

    /// Says whether a Statscraft window is open on this computer. When one is, calls without a
    /// session go to it and the person sees each result appear; the reply then lists its
    /// active dataset, open datasets and output documents.
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
            let file = live::endpoint_file();
            if !live::running(&file) {
                return CallToolResult::structured(json!({ "running": false }));
            }
            let window = crate::describe(None, Some(live::SESSION.into()), true);
            let state = window.map_or(Value::Null, |text| {
                serde_json::from_str(&text).unwrap_or(Value::Null)
            });
            // a window without data or output yet has nothing to describe
            CallToolResult::structured(json!({ "running": true, "window": state }))
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
