use clap::{Args, Parser, Subcommand};
use statscraft_data::Dataset;
use statscraft_engine::live::{self, Action, Request};
use statscraft_engine::session::{Failure, RunResult, Session, describe_output};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

mod connect;
mod db;
mod mcp;
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
        /// Describe this session (dataset, settings, macros) instead of a file; gui is the window
        #[arg(short, long)]
        session: Option<String>,
        /// Print the result as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run syntax. With --session the dataset, settings and macros are kept between runs
    Run {
        #[command(flatten)]
        args: RunArgs,
        #[command(flatten)]
        xlsx: XlsxArgs,
        #[command(flatten)]
        charts: ChartArgs,
    },
    /// Add a chart given as JSON to the output: the chart --json writes, or a Vega-Lite
    /// specification of a simple view. Runs it as the syntax BEGIN CHART, a Statscraft extension
    Chart {
        #[command(flatten)]
        args: ChartFileArgs,
        #[command(flatten)]
        xlsx: XlsxArgs,
        #[command(flatten)]
        charts: ChartArgs,
    },
    /// Validate syntax without saving or writing anything; with --session, against its dataset
    Check(RunArgs),
    /// Show the syntax of a command, or list all commands
    Help { command: Vec<String> },
    /// Lower bounded syntax to an OpenStatSpec transformation plan (JSON), or apply it in a database
    Plan(plan::PlanArgs),
    /// Store datasets in an OpenStatSpec database (SQLite, PostgreSQL, MySQL, MariaDB or Dolt) and read them back
    Db(db::DbArgs),
    /// Update this installation to the latest published release; --check only checks for one
    Update {
        /// Only check: exit code 0 when up to date, 10 when a newer release exists
        #[arg(long)]
        check: bool,
        /// With --check, print the result as JSON
        #[arg(long, requires = "check")]
        json: bool,
    },
    /// Serve the tools of this program to an AI agent over MCP (standard input and output)
    Mcp,
    /// Connect Statscraft's MCP server to the AI agents on this computer (Claude, ChatGPT and
    /// Codex, Cursor, Copilot, Gemini); --list only shows them, --remove takes it out again
    Connect(connect::ConnectArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Syntax file, or - for standard input
    file: Option<String>,
    /// Syntax text given directly
    #[arg(short = 'e', long = "execute", conflicts_with = "file")]
    text: Option<String>,
    /// Session name: the dataset, settings and macros are loaded from and saved to .statscraft/sessions/<name>.
    /// The name gui sends the syntax to the running statscraft-gui window instead
    #[arg(short, long)]
    session: Option<String>,
    /// Run only the commands inside this inclusive line range, for example 40-55
    #[arg(long, value_name = "FIRST-LAST")]
    lines: Option<String>,
    /// Print tables, charts, changes, warnings and errors as JSON
    #[arg(long)]
    json: bool,
    /// Also give every chart as a Vega-Lite specification: with --json as its member vega_lite,
    /// in Markdown as a code block after the chart's data table
    #[arg(long)]
    vega_lite: bool,
    /// Encoding of the syntax file, such as windows-1257 or 1252. Default: UTF-8 when the file
    /// is valid UTF-8 (with or without a byte order mark), else the system's legacy code page
    #[arg(long, value_name = "NAME")]
    encoding: Option<String>,
}

#[derive(Args)]
struct ChartFileArgs {
    /// The chart as a JSON file, or - for standard input
    file: String,
    /// Session name, as for run; gui adds the chart to the output of the running window
    #[arg(short, long)]
    session: Option<String>,
    /// Print the result as JSON
    #[arg(long)]
    json: bool,
    /// Also give the chart as a Vega-Lite specification, as for run
    #[arg(long)]
    vega_lite: bool,
}

#[derive(Args, Default)]
struct XlsxArgs {
    /// Also write every table and chart of the run to this Excel workbook, one sheet per table;
    /// a chart as its data table with an editable chart beside it
    #[arg(long, value_name = "FILE")]
    xlsx: Option<PathBuf>,
    /// With --xlsx, put all tables on one sheet, one below the other
    #[arg(long, requires = "xlsx")]
    xlsx_one_sheet: bool,
}

#[derive(Args)]
struct ChartArgs {
    /// Also write every chart of the run as a picture into this folder, made if missing:
    /// 01-<title>.svg, 02-<title>.svg and so on
    #[arg(long, value_name = "DIR")]
    charts: Option<PathBuf>,
    /// With --charts, the picture format
    #[arg(long, value_enum, default_value = "svg", requires = "charts")]
    chart_format: ChartFormat,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum ChartFormat {
    Svg,
    Png,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (json, outcome) = match cli.command {
        Cmd::Describe {
            file,
            session,
            json,
        } => (
            json,
            describe(file, session, json).map(|text| print!("{text}")),
        ),
        Cmd::Run { args, xlsx, charts } => (args.json, run(args, false, &xlsx, Some(&charts))),
        Cmd::Chart { args, xlsx, charts } => (args.json, add_chart(args, &xlsx, &charts)),
        Cmd::Check(args) => (args.json, run(args, true, &XlsxArgs::default(), None)),
        Cmd::Help { command } => (
            false,
            help(&command.join(" ")).map(|text| println!("{text}")),
        ),
        Cmd::Plan(args) => return plan::run(args),
        Cmd::Db(args) => return db::run(args),
        Cmd::Mcp => return mcp::serve(),
        Cmd::Connect(args) => return connect::run(args),
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

/// Where file sessions live when not in `.statscraft/sessions` of the working folder: set by
/// `statscraft mcp`, whose working folder is whatever the client chose.
static SESSIONS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

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
    let root = SESSIONS.get().map(PathBuf::as_path);
    Ok(root
        .unwrap_or(std::path::Path::new(".statscraft/sessions"))
        .join(name)
        .join("data.zsav"))
}

fn load_session(name: &str) -> Result<Option<Dataset>, Vec<Failure>> {
    let path = session_file(name)?;
    if !path.exists() {
        return Ok(None);
    }
    let mut dataset = Dataset::read_sav(&path).map_err(|e| session_failure(e.to_string()))?;
    if let Some(state) = read_state(&path)? {
        state.data.apply(&mut dataset)?;
    }
    Ok(Some(dataset))
}

fn session_failure(message: String) -> Vec<Failure> {
    vec![Failure::plain("session", message)]
}

/// The session with everything it keeps: its datasets, macros, settings and random numbers.
/// A session that does not exist yet is empty.
fn open_session(name: &str) -> Result<Session, Vec<Failure>> {
    let path = session_file(name)?;
    let mut session = Session::with_dataset(load_session(name)?);
    load_named(&path, &mut session)?;
    if let Ok(text) = std::fs::read_to_string(macros_file(&path)) {
        session.set_macros(statscraft_syntax::MacroState::from_syntax(&text));
    }
    if let Ok(text) = std::fs::read_to_string(settings_file(&path)) {
        session.restore_settings(&text)?;
    }
    if let Some(random) = read_state(&path)?.and_then(|state| state.random) {
        session.set_random_state(random);
    }
    Ok(session)
}

/// What a data file cannot hold of a dataset, kept beside it.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct DataState {
    split_file: Option<statscraft_data::SplitFile>,
    filter_variable: Option<String>,
}

impl DataState {
    fn of(dataset: &Dataset) -> Self {
        Self {
            split_file: dataset.split_file().cloned(),
            filter_variable: dataset.filter_variable().map(str::to_string),
        }
    }

    fn apply(self, dataset: &mut Dataset) -> Result<(), Vec<Failure>> {
        dataset
            .set_split_file(self.split_file)
            .and_then(|()| dataset.set_filter_variable(self.filter_variable.as_deref()))
            .map_err(|e| session_failure(e.to_string()))
    }
}

/// What a session keeps in `state.json` beside its data file.
#[derive(serde::Serialize, serde::Deserialize)]
struct SessionState {
    #[serde(flatten)]
    data: DataState,
    /// Where the random numbers stand, so SAMPLE and the random functions go on where the
    /// last run stopped.
    #[serde(default)]
    random: Option<statscraft_engine::RandomState>,
}

fn read_state(data: &std::path::Path) -> Result<Option<SessionState>, Vec<Failure>> {
    let Ok(text) = std::fs::read_to_string(state_file(data)) else {
        return Ok(None);
    };
    serde_json::from_str(&text).map_err(|e| session_failure(e.to_string()))
}

/// The macros of a session (DEFINE) and the SET values that steer them, kept as syntax.
fn macros_file(data: &std::path::Path) -> PathBuf {
    data.with_file_name("macros.sps")
}

/// The SET values, PRESERVE levels and CD folder of a session, kept as syntax.
fn settings_file(data: &std::path::Path) -> PathBuf {
    data.with_file_name("settings.sps")
}

fn state_file(data: &std::path::Path) -> PathBuf {
    data.with_file_name("state.json")
}

/// The named datasets of a session (DATASET NAME, COPY, DECLARE), kept in `datasets/` beside
/// its data: `names.json` holds the active dataset's name and the other names, in order, each
/// with its split and filter and the file `<position>.zsav`, except the names only declared.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct NamedDatasets {
    active: Option<String>,
    names: Vec<String>,
    #[serde(default)]
    states: Vec<DataState>,
    #[serde(default)]
    declared: Vec<String>,
}

fn datasets_folder(data: &std::path::Path) -> PathBuf {
    data.with_file_name("datasets")
}

fn load_named(data: &std::path::Path, session: &mut Session) -> Result<(), Vec<Failure>> {
    let folder = datasets_folder(data);
    let Ok(text) = std::fs::read_to_string(folder.join("names.json")) else {
        return Ok(());
    };
    let mut named: NamedDatasets =
        serde_json::from_str(&text).map_err(|e| session_failure(e.to_string()))?;
    let mut states = std::mem::take(&mut named.states).into_iter();
    let mut datasets = Vec::new();
    for (i, name) in named.names.into_iter().enumerate() {
        let state = states.next().unwrap_or_default();
        if named.declared.contains(&name) {
            datasets.push((name, None));
            continue;
        }
        let file = folder.join(format!("{i}.zsav"));
        let mut data = Dataset::read_sav(&file).map_err(|e| session_failure(e.to_string()))?;
        state.apply(&mut data)?;
        datasets.push((name, Some(data)));
    }
    session.set_named_datasets(datasets, named.active);
    Ok(())
}

fn save_named(data: &std::path::Path, session: &Session) -> Result<(), String> {
    let folder = datasets_folder(data);
    let _ = std::fs::remove_dir_all(&folder);
    let (all, active) = session.named_datasets();
    if all.is_empty() && active.is_none() {
        return Ok(());
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut named = NamedDatasets {
        active: active.map(str::to_string),
        ..NamedDatasets::default()
    };
    for (i, (name, data)) in all.iter().enumerate() {
        named.names.push(name.clone());
        match data {
            Some(data) => {
                write_kept(data, &folder.join(format!("{i}.zsav")))?;
                named.states.push(DataState::of(data));
            }
            None => {
                named.declared.push(name.clone());
                named.states.push(DataState::default());
            }
        }
    }
    let text = serde_json::to_string_pretty(&named).expect("plain data");
    std::fs::write(folder.join("names.json"), text).map_err(|e| e.to_string())
}

/// Writes a dataset the session keeps. Text the file's code page cannot hold, such as a label
/// typed in another language, is kept in UTF-8 instead of leaving the session unsaved.
fn write_kept(dataset: &Dataset, path: &std::path::Path) -> Result<(), String> {
    match dataset.write_sav_quick(path) {
        Err(e) if dataset.encoding() != "UTF-8" => {
            let mut unicode = dataset.clone();
            unicode.set_encoding("UTF-8").map_err(|e| e.to_string())?;
            unicode.write_sav_quick(path).map_err(|_| e.to_string())
        }
        written => written.map_err(|e| e.to_string()),
    }
}

/// Writes the active dataset of a session and what its data file cannot hold.
fn save_dataset(
    path: &std::path::Path,
    dataset: &Dataset,
    random: Option<statscraft_engine::RandomState>,
) -> Result<(), String> {
    write_kept(dataset, path)?;
    let state = SessionState {
        data: DataState::of(dataset),
        random,
    };
    let text = serde_json::to_string_pretty(&state).expect("plain data");
    std::fs::write(state_file(path), text).map_err(|e| e.to_string())
}

/// Replaces the active dataset of a session and keeps the rest of it.
fn replace_dataset(path: &std::path::Path, dataset: &Dataset) -> Result<(), String> {
    let random = read_state(path).ok().flatten().and_then(|s| s.random);
    save_dataset(path, dataset, random)
}

/// Writes everything a session keeps. A session left without an active dataset keeps its
/// last data file, as before.
fn save_session(path: &std::path::Path, session: &Session) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().expect("session path has a parent"))
        .map_err(|e| e.to_string())?;
    if let Some(dataset) = session.dataset() {
        save_dataset(path, dataset, Some(session.random_state()))?;
    }
    save_named(path, session)?;
    std::fs::write(macros_file(path), session.macros().to_syntax()).map_err(|e| e.to_string())?;
    let settings = format!(
        "* The SET values, PRESERVE levels and CD folder of this session.\n{}",
        session.settings_syntax()
    );
    std::fs::write(settings_file(path), settings).map_err(|e| e.to_string())
}

/// What `statscraft describe` prints.
fn describe(
    file: Option<String>,
    session: Option<String>,
    json: bool,
) -> Result<String, Vec<Failure>> {
    Ok(match (file, session) {
        (None, Some(name)) if name == live::SESSION => live::send(
            &live::endpoint_file(),
            Request {
                action: Action::Describe,
                json,
                ..Request::default()
            },
        )?,
        (Some(file), None) => {
            let dataset = Dataset::read_sav(&file)
                .map_err(|e| vec![Failure::plain("file", e.to_string())])?;
            describe_output(&dataset, json)
        }
        (None, Some(name)) => open_session(&name)?.describe(json).ok_or_else(|| {
            session_failure(format!("session {name} has no dataset or settings yet"))
        })?,
        _ => {
            return Err(vec![Failure::plain(
                "usage",
                "give either a file or --session",
            )]);
        }
    })
}

/// What `statscraft help` prints, without the final line break.
fn help(command: &str) -> Result<String, Vec<Failure>> {
    let topic = (!command.is_empty()).then_some(command);
    match statscraft_syntax::help(topic) {
        Some(text) => Ok(text),
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

fn run(
    args: RunArgs,
    check_only: bool,
    xlsx: &XlsxArgs,
    charts: Option<&ChartArgs>,
) -> Result<(), Vec<Failure>> {
    let result = run_result(&args, check_only, xlsx)?;
    if let Some(charts) = charts {
        write_charts(&result, charts)?;
    }
    print!(
        "{}",
        result.output_with(args.json, check_only, args.vega_lite)
    );
    Ok(())
}

/// The syntax that adds the chart `json` to the output: BEGIN CHART, a Statscraft extension.
fn chart_syntax(json: &str) -> String {
    format!("BEGIN CHART\n{}\nEND CHART.\n", json.trim())
}

/// `statscraft chart`: reads the JSON and runs it as BEGIN CHART, as `statscraft run` would.
fn add_chart(
    input: ChartFileArgs,
    xlsx: &XlsxArgs,
    charts: &ChartArgs,
) -> Result<(), Vec<Failure>> {
    let mut args = RunArgs {
        file: Some(input.file),
        text: None,
        session: input.session,
        lines: None,
        json: input.json,
        vega_lite: input.vega_lite,
        encoding: None,
    };
    args.text = Some(chart_syntax(&read_source(&args)?));
    args.file = None;
    run(args, false, xlsx, Some(charts))
}

/// Runs or checks the syntax `args` name in its session, or in a fresh one, and keeps the
/// session. `args.json` is not looked at: the caller chooses how to show the result.
fn run_result(
    args: &RunArgs,
    check_only: bool,
    xlsx: &XlsxArgs,
) -> Result<RunResult, Vec<Failure>> {
    let source = read_source(args)?;
    let lines = args.lines.as_deref().map(parse_lines).transpose()?;
    if args.session.as_deref() == Some(live::SESSION) {
        let request = Request {
            action: if check_only {
                Action::Check
            } else {
                Action::Run
            },
            source,
            lines,
            // the result comes back in the JSON form and is shown here
            json: true,
            cwd: std::env::current_dir().ok(),
            ..Request::default()
        };
        return run_in_window(request, xlsx);
    }
    let mut session = match &args.session {
        Some(name) => open_session(name)?,
        None => Session::default(),
    };
    // INSERT and INCLUDE in a syntax file start from its folder
    let folder = args.file.as_deref().filter(|f| *f != "-");
    session.set_base_dir(
        folder.and_then(|f| std::path::Path::new(f).parent().map(|p| p.to_path_buf())),
    );
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
    if let (false, Some(name)) = (check_only, &args.session) {
        let path = session_file(name)?;
        // also before: a run whose syntax cannot be kept changes nothing
        let trail = keep_trail(&path, &statscraft_engine::live::last_syntax(&session))
            .map_err(session_failure)?;
        result.trail = Some(trail.display().to_string());
        if changed || (session.dataset().is_some() && !path.exists()) {
            save_session(&path, &session).map_err(session_failure)?;
            result.saved = true;
        }
    }
    Ok(result)
}

/// Sends a run or check to the window and writes the workbook from its reply.
fn run_in_window(request: Request, xlsx: &XlsxArgs) -> Result<RunResult, Vec<Failure>> {
    let reply = live::send(&live::endpoint_file(), request)?;
    let result: RunResult = serde_json::from_str(&reply).map_err(|e| {
        vec![Failure::plain(
            "session",
            format!("reply of the window: {e}"),
        )]
    })?;
    write_workbook(&result, xlsx)?;
    Ok(result)
}

/// Adds the syntax of a run to `syntax.sps` beside the session's data, so the session's folder
/// holds every step that built it. Returns the file.
fn keep_trail(data: &std::path::Path, syntax: &str) -> Result<PathBuf, String> {
    let file = std::path::absolute(data.with_file_name("syntax.sps")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(file.parent().expect("session path has a parent"))
        .map_err(|e| e.to_string())?;
    // a file grows over days, so its stamps carry the date
    let mut block = live::trail_block(&live::stamp(true), syntax);
    if std::fs::metadata(&file).is_ok_and(|m| m.len() > 0) {
        block.insert(0, '\n');
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut f| f.write_all(block.as_bytes()))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

/// Writes every table and chart of the run to the workbook `--xlsx` names, if any.
fn write_workbook(result: &RunResult, xlsx: &XlsxArgs) -> Result<(), Vec<Failure>> {
    let Some(path) = &xlsx.xlsx else {
        return Ok(());
    };
    use statscraft_export::Item;
    let items = result.commands.iter().flat_map(|c| {
        let tables = c.tables.iter().map(Item::Table);
        tables.chain(c.charts.iter().map(Item::Chart))
    });
    statscraft_export::write_xlsx(path, items, xlsx.xlsx_one_sheet)
        .map_err(|e| vec![Failure::plain("file", format!("{}: {e}", path.display()))])
}

/// Writes every chart of the run as a picture into the folder `--charts` names, if any, on light
/// paper. A kind that has no drawing yet is left out; its data table is in the output.
fn write_charts(result: &RunResult, args: &ChartArgs) -> Result<(), Vec<Failure>> {
    use statscraft_export::picture::{self, Paper};
    let Some(dir) = &args.charts else {
        return Ok(());
    };
    let failed = |path: &std::path::Path, e: &dyn std::fmt::Display| {
        vec![Failure::plain("file", format!("{}: {e}", path.display()))]
    };
    std::fs::create_dir_all(dir).map_err(|e| failed(dir, &e))?;
    let charts = result.commands.iter().flat_map(|c| &c.charts);
    for (n, chart) in charts.enumerate() {
        let name: String = chart
            .title
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join("-")
            .chars()
            .take(60)
            .collect();
        let bytes = match args.chart_format {
            ChartFormat::Svg => picture::chart_svg(chart, Paper::Light).map(|s| Ok(s.into_bytes())),
            ChartFormat::Png => picture::chart_png(chart, Paper::Light),
        };
        let extension = match args.chart_format {
            ChartFormat::Svg => "svg",
            ChartFormat::Png => "png",
        };
        let path = dir.join(format!("{:02}-{name}.{extension}", n + 1));
        if let Some(bytes) = bytes {
            let bytes = bytes.map_err(|e| failed(&path, &e))?;
            std::fs::write(&path, bytes).map_err(|e| failed(&path, &e))?;
        }
    }
    Ok(())
}
