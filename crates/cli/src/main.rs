use clap::{Args, Parser, Subcommand};
use statscraft_data::Dataset;
use statscraft_engine::live::{self, Action, Request};
use statscraft_engine::session::{Failure, RunResult, Session, describe_output};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

mod db;
mod plan;
mod update;

#[derive(Parser)]
#[command(
    name = "statscraft",
    version,
    about = "Statistics workbench",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the dictionary and basic counts of a data file or of a session
    Describe {
        /// A .sav or .zsav file
        file: Option<String>,
        /// Describe the dataset of this session instead of a file; gui is the running window
        #[arg(short, long)]
        session: Option<String>,
        /// Print the result as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run syntax. With --session the dataset is kept between runs
    Run {
        #[command(flatten)]
        args: RunArgs,
        #[command(flatten)]
        xlsx: XlsxArgs,
    },
    /// Validate syntax against the session's dataset without saving or writing anything
    Check(RunArgs),
    /// Show the syntax of a command, or list all commands
    Help { command: Vec<String> },
    /// Lower bounded syntax to an OpenStatSpec transformation plan (JSON)
    Plan(plan::PlanArgs),
    /// Store datasets in an OpenStatSpec SQLite database and read them back
    Db(db::DbArgs),
    /// Update this installation to the latest published release
    Update {
        /// Only check: exit code 0 when up to date, 10 when a newer release exists
        #[arg(long)]
        check: bool,
        /// With --check, print the result as JSON
        #[arg(long, requires = "check")]
        json: bool,
    },
}

#[derive(Args)]
struct RunArgs {
    /// Syntax file, or - for standard input
    file: Option<String>,
    /// Syntax text given directly
    #[arg(short = 'e', long = "execute", conflicts_with = "file")]
    text: Option<String>,
    /// Session name: the dataset is loaded from and saved to .statscraft/sessions/<name>.
    /// The name gui sends the syntax to the running statscraft-gui window instead
    #[arg(short, long)]
    session: Option<String>,
    /// Run only the commands inside this inclusive line range, for example 40-55
    #[arg(long, value_name = "FIRST-LAST")]
    lines: Option<String>,
    /// Print tables, changes, warnings and errors as JSON
    #[arg(long)]
    json: bool,
    /// Encoding of the syntax file, such as windows-1257 or 1252. Default: UTF-8 when the file
    /// is valid UTF-8 (with or without a byte order mark), else the system's legacy code page
    #[arg(long, value_name = "NAME")]
    encoding: Option<String>,
}

#[derive(Args, Default)]
struct XlsxArgs {
    /// Also write every table of the run to this Excel workbook, one sheet per table
    #[arg(long, value_name = "FILE")]
    xlsx: Option<PathBuf>,
    /// With --xlsx, put all tables on one sheet, one below the other
    #[arg(long, requires = "xlsx")]
    xlsx_one_sheet: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (json, outcome) = match cli.command {
        Cmd::Describe {
            file,
            session,
            json,
        } => (json, describe(file, session, json)),
        Cmd::Run { args, xlsx } => (args.json, run(args, false, &xlsx)),
        Cmd::Check(args) => (args.json, run(args, true, &XlsxArgs::default())),
        Cmd::Help { command } => (false, help(&command.join(" "))),
        Cmd::Plan(args) => return plan::run(args),
        Cmd::Db(args) => return db::run(args),
        Cmd::Update { check, json } => match update::run(check, json) {
            Ok(code) => return code,
            Err(failures) => (json, Err(failures)),
        },
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(failures) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "ok": false, "errors": failures, "saved": false })
                );
            } else {
                for f in &failures {
                    eprintln!("error: {}", f.text());
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn session_file(name: &str) -> Result<PathBuf, Vec<Failure>> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(vec![Failure::plain(
            "usage",
            "a session name may contain only letters, digits, - and _",
        )]);
    }
    Ok(PathBuf::from(".statscraft/sessions")
        .join(name)
        .join("data.zsav"))
}

fn load_session(name: &str) -> Result<Option<Dataset>, Vec<Failure>> {
    let path = session_file(name)?;
    if !path.exists() {
        return Ok(None);
    }
    let fail = |e: String| vec![Failure::plain("session", e)];
    let mut dataset = Dataset::read_sav(&path).map_err(|e| fail(e.to_string()))?;
    // a data file holds no split or filter, so the session keeps them next to it
    if let Ok(text) = std::fs::read_to_string(state_file(&path)) {
        let state: SessionState = serde_json::from_str(&text).map_err(|e| fail(e.to_string()))?;
        dataset
            .set_split_file(state.split_file)
            .and_then(|()| dataset.set_filter_variable(state.filter_variable.as_deref()))
            .map_err(|e| fail(e.to_string()))?;
    }
    Ok(Some(dataset))
}

/// What a session keeps beside its data file.
#[derive(serde::Serialize, serde::Deserialize)]
struct SessionState {
    split_file: Option<statscraft_data::SplitFile>,
    filter_variable: Option<String>,
}

fn state_file(data: &std::path::Path) -> PathBuf {
    data.with_file_name("state.json")
}

/// Writes the dataset of a session and the state that its file cannot hold.
fn save_session(path: &std::path::Path, dataset: &Dataset) -> Result<(), String> {
    dataset.write_sav(path).map_err(|e| e.to_string())?;
    let state = SessionState {
        split_file: dataset.split_file().cloned(),
        filter_variable: dataset.filter_variable().map(str::to_string),
    };
    let text = serde_json::to_string_pretty(&state).expect("plain data");
    std::fs::write(state_file(path), text).map_err(|e| e.to_string())
}

fn describe(file: Option<String>, session: Option<String>, json: bool) -> Result<(), Vec<Failure>> {
    let dataset = match (file, session) {
        (None, Some(name)) if name == live::SESSION => {
            return remote(Request {
                token: String::new(),
                action: Action::Describe,
                source: String::new(),
                lines: None,
                json,
                cwd: None,
            });
        }
        (Some(file), None) => {
            Dataset::read_sav(&file).map_err(|e| vec![Failure::plain("file", e.to_string())])?
        }
        (None, Some(name)) => load_session(&name)?.ok_or_else(|| {
            vec![Failure::plain(
                "session",
                format!("session {name} has no dataset yet"),
            )]
        })?,
        _ => {
            return Err(vec![Failure::plain(
                "usage",
                "give either a file or --session",
            )]);
        }
    };
    print!("{}", describe_output(&dataset, json));
    Ok(())
}

/// Sends the request to the running window and prints its reply.
fn remote(request: Request) -> Result<(), Vec<Failure>> {
    print!("{}", live::send(&live::endpoint_file(), request)?);
    Ok(())
}

fn help(command: &str) -> Result<(), Vec<Failure>> {
    let topic = (!command.is_empty()).then_some(command);
    match statscraft_syntax::help(topic) {
        Some(text) => {
            println!("{text}");
            Ok(())
        }
        None => Err(vec![Failure::plain(
            "usage",
            format!("unknown command {command}; run `statscraft help` for the list"),
        )]),
    }
}

fn read_source(args: &RunArgs) -> Result<String, Vec<Failure>> {
    let fail = |m: String| vec![Failure::plain("usage", m)];
    match (&args.text, args.file.as_deref()) {
        (Some(text), _) => Ok(text.clone()),
        (None, Some("-")) => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .read_to_end(&mut bytes)
                .map_err(|e| fail(e.to_string()))?;
            statscraft_syntax::decode_syntax(&bytes, args.encoding.as_deref()).map_err(fail)
        }
        (None, Some(file)) => {
            let bytes = std::fs::read(file).map_err(|e| fail(format!("{file}: {e}")))?;
            statscraft_syntax::decode_syntax(&bytes, args.encoding.as_deref()).map_err(fail)
        }
        (None, None) => Err(fail(
            "give a syntax file, - for standard input, or -e 'SYNTAX.'".into(),
        )),
    }
}

fn parse_lines(spec: &str) -> Result<(u32, u32), Vec<Failure>> {
    let parsed = spec
        .split_once('-')
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)));
    match parsed {
        Some((first, last)) if first >= 1 && first <= last => Ok((first, last)),
        _ => Err(vec![Failure::plain(
            "usage",
            format!("--lines expects FIRST-LAST, for example 40-55, got {spec}"),
        )]),
    }
}

fn run(args: RunArgs, check_only: bool, xlsx: &XlsxArgs) -> Result<(), Vec<Failure>> {
    let source = read_source(&args)?;
    let lines = args.lines.as_deref().map(parse_lines).transpose()?;
    if args.session.as_deref() == Some(live::SESSION) {
        let request = Request {
            token: String::new(),
            action: if check_only {
                Action::Check
            } else {
                Action::Run
            },
            source,
            lines,
            // the tables for the workbook come back in the JSON form
            json: args.json || xlsx.xlsx.is_some(),
            cwd: std::env::current_dir().ok(),
        };
        if xlsx.xlsx.is_none() {
            return remote(request);
        }
        let reply = live::send(&live::endpoint_file(), request)?;
        let result: RunResult = serde_json::from_str(&reply).map_err(|e| {
            vec![Failure::plain(
                "session",
                format!("reply of the window: {e}"),
            )]
        })?;
        write_workbook(&result, xlsx)?;
        print!("{}", result.output(args.json, check_only));
        return Ok(());
    }
    let loaded = match &args.session {
        Some(name) => load_session(name)?,
        None => None,
    };
    let mut session = Session::with_dataset(loaded);
    // shortcut: check runs the commands on a copy (file writes skipped) instead of a separate
    // validation pass, so it costs as much as a run; add a validation pass if that gets slow.
    let mut result = if check_only {
        session.check(&source, lines)
    } else {
        session.run(&source, lines)
    }?;
    result.session = args.session.clone();
    // a run is all or nothing: a failure above returned before anything was saved
    let changed = std::mem::take(&mut result.saved);
    // before the session is saved, so a workbook that cannot be written leaves the session as it was
    write_workbook(&result, xlsx)?;
    if let (false, Some(name), Some(dataset)) = (check_only, &args.session, session.dataset()) {
        let path = session_file(name)?;
        if changed || !path.exists() {
            let fail = |m: String| vec![Failure::plain("session", m)];
            std::fs::create_dir_all(path.parent().expect("session path has a parent"))
                .map_err(|e| fail(e.to_string()))?;
            save_session(&path, dataset).map_err(fail)?;
            result.saved = true;
        }
    }
    print!("{}", result.output(args.json, check_only));
    Ok(())
}

/// Writes every table of the run to the workbook `--xlsx` names, if any.
fn write_workbook(result: &RunResult, xlsx: &XlsxArgs) -> Result<(), Vec<Failure>> {
    let Some(path) = &xlsx.xlsx else {
        return Ok(());
    };
    let tables = result.commands.iter().flat_map(|c| &c.tables);
    statscraft_export::write_xlsx(path, tables, xlsx.xlsx_one_sheet)
        .map_err(|e| vec![Failure::plain("file", format!("{}: {e}", path.display()))])
}
