use clap::{Args, Parser, Subcommand};
use statscraft_data::Dataset;
use statscraft_engine::live::{self, Action, Request};
use statscraft_engine::session::{Failure, Session, describe_output};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

mod db;
mod plan;

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
        #[arg(long)]
        json: bool,
    },
    /// Run syntax. With --session the dataset is kept between runs
    Run(RunArgs),
    /// Validate syntax against the session's dataset without saving or writing anything
    Check(RunArgs),
    /// Show the syntax of a command, or list all commands
    Help { command: Vec<String> },
    /// Lower bounded syntax to an OpenStatSpec transformation plan (JSON)
    Plan(plan::PlanArgs),
    /// Store datasets in an OpenStatSpec SQLite database and read them back
    Db(db::DbArgs),
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
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (json, outcome) = match cli.command {
        Cmd::Describe {
            file,
            session,
            json,
        } => (json, describe(file, session, json)),
        Cmd::Run(args) => (args.json, run(args, false)),
        Cmd::Check(args) => (args.json, run(args, true)),
        Cmd::Help { command } => (false, help(&command.join(" "))),
        Cmd::Plan(args) => return plan::run(args),
        Cmd::Db(args) => return db::run(args),
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
    Dataset::read_sav(&path)
        .map(Some)
        .map_err(|e| vec![Failure::plain("session", e.to_string())])
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
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| fail(e.to_string()))?;
            Ok(s)
        }
        (None, Some(file)) => {
            std::fs::read_to_string(file).map_err(|e| fail(format!("{file}: {e}")))
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

fn run(args: RunArgs, check_only: bool) -> Result<(), Vec<Failure>> {
    let source = read_source(&args)?;
    let lines = args.lines.as_deref().map(parse_lines).transpose()?;
    if args.session.as_deref() == Some(live::SESSION) {
        return remote(Request {
            token: String::new(),
            action: if check_only {
                Action::Check
            } else {
                Action::Run
            },
            source,
            lines,
            json: args.json,
            cwd: std::env::current_dir().ok(),
        });
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
    if let (false, Some(name), Some(dataset)) = (check_only, &args.session, session.dataset()) {
        let path = session_file(name)?;
        if changed || !path.exists() {
            let fail = |m: String| vec![Failure::plain("session", m)];
            std::fs::create_dir_all(path.parent().expect("session path has a parent"))
                .map_err(|e| fail(e.to_string()))?;
            dataset.write_sav(&path).map_err(|e| fail(e.to_string()))?;
            result.saved = true;
        }
    }
    print!("{}", result.output(args.json, check_only));
    Ok(())
}
