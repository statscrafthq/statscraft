use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use statscraft_data::Dataset;
use statscraft_engine::{Cell, Changes, Engine, Table, format_value};
use statscraft_syntax::{Command, RawCommand, SyntaxError};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

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
        /// Describe the dataset of this session instead of a file
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
}

#[derive(Args)]
struct RunArgs {
    /// Syntax file, or - for standard input
    file: Option<String>,
    /// Syntax text given directly
    #[arg(short = 'e', long = "execute", conflicts_with = "file")]
    text: Option<String>,
    /// Session name: the dataset is loaded from and saved to .statscraft/sessions/<name>
    #[arg(short, long)]
    session: Option<String>,
    /// Run only the commands inside this inclusive line range, for example 40-55
    #[arg(long, value_name = "FIRST-LAST")]
    lines: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct Failure {
    kind: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

impl Failure {
    fn plain(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            command: None,
            line: None,
            column: None,
            expected: vec![],
            hint: None,
        }
    }

    fn syntax(e: SyntaxError) -> Self {
        Self {
            kind: "syntax",
            message: e.message,
            command: None,
            line: Some(e.span.line),
            column: Some(e.span.column),
            expected: e.expected,
            hint: e.hint,
        }
    }

    fn text(&self) -> String {
        let mut s = String::new();
        if let (Some(line), Some(column)) = (self.line, self.column) {
            s.push_str(&format!("line {line}, column {column}: "));
        } else if let Some(line) = self.line {
            s.push_str(&format!("line {line}: "));
        }
        if let Some(command) = &self.command {
            s.push_str(&format!("{command}: "));
        }
        s.push_str(&self.message);
        if !self.expected.is_empty() {
            s.push_str(&format!("; expected: {}", self.expected.join(", ")));
        }
        if let Some(hint) = &self.hint {
            s.push_str(&format!("; hint: {hint}"));
        }
        s
    }
}

#[derive(Serialize)]
struct CommandResult {
    name: String,
    first_line: u32,
    last_line: u32,
    tables: Vec<Table>,
}

#[derive(Serialize)]
struct RunResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<String>,
    commands: Vec<CommandResult>,
    changes: Changes,
    /// Cases and variables of the active dataset after the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    dataset: Option<[usize; 2]>,
    saved: bool,
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
    let d = dataset.describe();
    if json {
        println!("{}", serde_json::to_string_pretty(&d).expect("plain data"));
        return Ok(());
    }
    println!("{} cases, {} variables", d.case_count, d.variable_count);
    for v in &d.variables {
        println!(
            "{}\t{}\t{}\tvalid {}\t{}",
            v.name,
            v.format,
            v.measure,
            v.n_valid,
            v.label.as_deref().unwrap_or("")
        );
    }
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
    let all = statscraft_syntax::split_commands(&source).map_err(|e| vec![Failure::syntax(e)])?;
    let selected: &[RawCommand] = match &args.lines {
        Some(spec) => {
            let (first, last) = parse_lines(spec)?;
            statscraft_syntax::select_lines(&all, first, last)
                .map_err(|e| vec![Failure::syntax(e)])?
        }
        None => &all,
    };

    let mut commands: Vec<(&RawCommand, Command)> = Vec::new();
    let mut failures = Vec::new();
    for raw in selected {
        match statscraft_syntax::parse_command(raw) {
            Ok(command) => commands.push((raw, command)),
            Err(e) => failures.push(Failure::syntax(e)),
        }
    }
    if !failures.is_empty() {
        return Err(failures);
    }

    let loaded = match &args.session {
        Some(name) => load_session(name)?,
        None => None,
    };
    let before = loaded.clone();
    let mut engine = Engine::with_dataset(loaded);
    // shortcut: check runs the commands on a copy (file writes skipped) instead of a separate
    // validation pass, so it costs as much as a run; add a validation pass if that gets slow.
    engine.dry_run = check_only;

    let mut results = Vec::new();
    for (raw, command) in &commands {
        // A failing command aborts the whole run and nothing is saved, so the same range can be
        // run again after the fix.
        let tables = engine.execute(command).map_err(|e| {
            vec![Failure {
                kind: "run",
                message: e.to_string(),
                command: Some(raw.name.clone()),
                line: Some(raw.first_line),
                column: None,
                expected: vec![],
                hint: None,
            }]
        })?;
        results.push(CommandResult {
            name: raw.name.clone(),
            first_line: raw.first_line,
            last_line: raw.last_line,
            tables,
        });
    }

    engine
        .finish()
        .map_err(|e| vec![Failure::plain("run", e.to_string())])?;

    let after = engine.dataset().map(Dataset::describe);
    let changes = Changes::between(before.as_ref(), engine.dataset());
    let mut saved = false;
    if let (false, Some(name), Some(dataset)) = (check_only, &args.session, engine.dataset()) {
        let path = session_file(name)?;
        // compare whole datasets: a weight change shows in no variable
        if before.as_ref() != Some(dataset) || !path.exists() {
            let fail = |m: String| vec![Failure::plain("session", m)];
            std::fs::create_dir_all(path.parent().expect("session path has a parent"))
                .map_err(|e| fail(e.to_string()))?;
            dataset.write_sav(&path).map_err(|e| fail(e.to_string()))?;
            saved = true;
        }
    }

    let result = RunResult {
        ok: true,
        session: args.session.clone(),
        commands: results,
        changes,
        dataset: after.as_ref().map(|d| [d.case_count, d.variable_count]),
        saved,
    };
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("plain data")
        );
    } else {
        print_text(&result, check_only);
    }
    Ok(())
}

fn print_text(result: &RunResult, check_only: bool) {
    for table in result.commands.iter().flat_map(|c| &c.tables) {
        println!("### {}\n", table.title);
        println!("| {} |", table.column_titles().join(" | "));
        println!("|{}", " --- |".repeat(table.columns.len()));
        for (r, row) in table.rows.iter().enumerate() {
            let format = |c: usize| {
                table
                    .formats
                    .get(r)
                    .and_then(|f| f.get(c))
                    .filter(|f| !f.is_empty())
            };
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(c, cell)| match (cell, format(c)) {
                    (Cell::Text(t), _) => t.replace('|', "\\|"),
                    (Cell::Number(n), Some(format)) => format_value(*n, format, true),
                    (Cell::Number(n), None) if n.fract() == 0.0 => format!("{n:.0}"),
                    (Cell::Number(n), None) => format!("{n:.2}"),
                    (Cell::Empty, _) => String::new(),
                })
                .collect();
            println!("| {} |", cells.join(" | "));
        }
        if let Some(caption) = &table.caption {
            println!("\n{caption}");
        }
        println!();
    }
    let c = &result.changes;
    let list = |title: &str, names: &[String]| {
        if !names.is_empty() {
            println!("{title}: {}", names.join(", "));
        }
    };
    if let (None, Some([cases, variables])) = (c.cases_before, result.dataset) {
        println!("dataset loaded: {cases} cases, {variables} variables");
    } else {
        list("variables added", &c.variables_added);
    }
    list("variables changed", &c.variables_changed);
    list("variables removed", &c.variables_removed);
    if c.cases_before.is_some() && c.cases_before != c.cases_after {
        println!(
            "cases: {} -> {}",
            c.cases_before.map_or("none".into(), |n| n.to_string()),
            c.cases_after.map_or("none".into(), |n| n.to_string())
        );
    }
    let verb = if check_only { "checked" } else { "ran" };
    println!(
        "{verb} {} command(s){}",
        result.commands.len(),
        if result.saved { ", session saved" } else { "" }
    );
}
