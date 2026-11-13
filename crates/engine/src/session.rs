//! Syntax text run against a dataset that is kept between runs: the one path shared by
//! `statscraft run`, the window's Run button and the window's live session. Splits, selects,
//! parses and executes; a run is all or nothing.

use crate::{Cell, Changes, Engine, Table, cell_text};
use serde::{Deserialize, Serialize};
use statscraft_data::Dataset;
use statscraft_syntax::{Command, RawCommand, SyntaxError};
use std::collections::VecDeque;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

/// One error, at a line and column of the syntax when known. `kind` is "syntax", "run",
/// "usage", "session", "file" or "window" (a syntax window of the open window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Failure {
    pub fn plain(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            command: None,
            line: None,
            column: None,
            expected: vec![],
            hint: None,
        }
    }

    pub fn syntax(e: SyntaxError) -> Self {
        Self {
            line: Some(e.span.line),
            column: Some(e.span.column),
            expected: e.expected,
            hint: e.hint,
            ..Self::plain("syntax", e.message)
        }
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        if let (Some(line), Some(column)) = (self.line, self.column) {
            s.push_str(&format!("line {line}, column {column}: "));
        } else if let Some(line) = self.line {
            s.push_str(&format!("line {line}: "));
        }
        match &self.command {
            Some(command) => s.push_str(&with_command(command, &self.message)),
            None => s.push_str(&self.message),
        }
        if !self.expected.is_empty() {
            s.push_str(&format!("; expected: {}", self.expected.join(", ")));
        }
        if let Some(hint) = &self.hint {
            s.push_str(&format!("; hint: {hint}"));
        }
        s
    }
}

/// The message after the name of its command, once: many commands name themselves in their
/// messages already.
fn with_command(command: &str, message: &str) -> String {
    match message.starts_with(&format!("{command}: ")) {
        true => message.to_string(),
        false => format!("{command}: {message}"),
    }
}

/// The tables of one command, with the lines it came from.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub name: String,
    pub first_line: u32,
    pub last_line: u32,
    pub tables: Vec<Table>,
    /// The charts it made, each with its numbers and labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<crate::Chart>,
    /// What the command did differently from what was asked, without failing.
    pub warnings: Vec<String>,
    /// The output document its tables went to: the designated one when it ran. None when it
    /// made no tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    /// OUTPUT OPEN: the file as written and the items it read into `document`, for the window
    /// to show. Agents read the file itself, so it is not in the JSON.
    #[serde(skip)]
    pub opened: Option<(String, Vec<crate::output_file::Item>)>,
    /// OUTPUT MODIFY: per document the items that changed or went, for the window and for
    /// agents that follow the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modified: Vec<crate::output_modify::Modified>,
}

/// Lines `first` to `last` (from 1, inclusive) of a source: the syntax of one command.
pub fn source_lines(source: &str, first: u32, last: u32) -> String {
    let count = last.saturating_sub(first) as usize + 1;
    let lines: Vec<&str> = source
        .lines()
        .skip(first.saturating_sub(1) as usize)
        .take(count)
        .collect();
    lines.join("\n")
}

/// What a run did. Serialized, it is the JSON that `statscraft run --json` prints; the command
/// line reads it back from the window to write a workbook.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Where the syntax of the run was kept: the syntax.sps file of a file session, or the
    /// syntax window of the open window. None for a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail: Option<String>,
    pub commands: Vec<CommandResult>,
    pub changes: Changes,
    /// Cases and variables of the active dataset after the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset: Option<[usize; 2]>,
    /// Set by `Session` when the run changed the dataset it keeps; the command line sets it
    /// when it wrote the session file.
    pub saved: bool,
}

impl RunResult {
    /// What `statscraft run` prints: pretty JSON, or Markdown tables and a summary.
    pub fn output(&self, json: bool, check_only: bool) -> String {
        self.output_with(json, check_only, false)
    }

    /// `output`, and with `vega_lite` the Vega-Lite specification of every chart: in JSON as
    /// its member `vega_lite`, in Markdown as a code block after its data table.
    pub fn output_with(&self, json: bool, check_only: bool, vega_lite: bool) -> String {
        if json {
            let value = self.to_json(vega_lite);
            return serde_json::to_string_pretty(&value).expect("plain data") + "\n";
        }
        let mut out = String::new();
        for command in &self.commands {
            for table in &command.tables {
                markdown(&mut out, table);
            }
            for chart in &command.charts {
                markdown(&mut out, &chart.data_table());
                if vega_lite {
                    let spec = serde_json::to_string_pretty(&chart.vega_lite());
                    let _ = writeln!(out, "```json\n{}\n```\n", spec.expect("plain data"));
                }
            }
            // OUTPUT MODIFY: the items as they are now, by their place in the document
            for document in &command.modified {
                for item in &document.items {
                    let place = format!("{}, item {}", document.document, item.position + 1);
                    match item.table.as_ref().filter(|_| !item.deleted) {
                        Some(table) => {
                            let hidden = if table.hidden { " (hidden)" } else { "" };
                            let _ = writeln!(out, "changed in {place}{hidden}:\n");
                            markdown(&mut out, table);
                        }
                        None => {
                            let _ = writeln!(out, "deleted from {place}\n");
                        }
                    }
                }
            }
            for warning in &command.warnings {
                let _ = writeln!(
                    out,
                    "warning: line {}: {}",
                    command.first_line,
                    with_command(&command.name, warning)
                );
            }
        }
        let c = &self.changes;
        let list = |out: &mut String, title: &str, names: &[String]| {
            if !names.is_empty() {
                let _ = writeln!(out, "{title}: {}", names.join(", "));
            }
        };
        if let (None, Some([cases, variables])) = (c.cases_before, self.dataset) {
            let _ = writeln!(out, "dataset loaded: {cases} cases, {variables} variables");
        } else {
            list(&mut out, "variables added", &c.variables_added);
        }
        list(&mut out, "variables changed", &c.variables_changed);
        list(&mut out, "variables removed", &c.variables_removed);
        if c.cases_before.is_some() && c.cases_before != c.cases_after {
            let count = |n: Option<usize>| n.map_or("none".into(), |n| n.to_string());
            let _ = writeln!(
                out,
                "cases: {} -> {}",
                count(c.cases_before),
                count(c.cases_after)
            );
        }
        let verb = if check_only { "checked" } else { "ran" };
        let _ = writeln!(
            out,
            "{verb} {} command(s){}",
            self.commands.len(),
            if self.saved { ", session saved" } else { "" }
        );
        out
    }
}

impl RunResult {
    /// The result as JSON; with `vega_lite` every chart also holds its Vega-Lite specification.
    pub fn to_json(&self, vega_lite: bool) -> serde_json::Value {
        let mut value = serde_json::to_value(self).expect("plain data");
        if !vega_lite {
            return value;
        }
        for (i, command) in self.commands.iter().enumerate() {
            for (j, chart) in command.charts.iter().enumerate() {
                value["commands"][i]["charts"][j]["vega_lite"] = chart.vega_lite();
            }
        }
        value
    }
}

fn markdown(out: &mut String, table: &Table) {
    let _ = writeln!(out, "### {}\n", table.title);
    let _ = writeln!(out, "| {} |", table.column_titles().join(" | "));
    let _ = writeln!(out, "|{}", " --- |".repeat(table.columns.len()));
    for (r, row) in table.rows.iter().enumerate() {
        let cells: Vec<String> = (0..row.len())
            .map(|c| cell_text(table, r, c).replace('|', "\\|"))
            .collect();
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    }
    if let Some(caption) = &table.caption {
        let _ = writeln!(out, "\n{caption}");
    }
    out.push('\n');
}

/// What `statscraft describe` prints for a dataset.
pub fn describe_output(dataset: &Dataset, json: bool) -> String {
    let d = dataset.describe();
    if json {
        return serde_json::to_string_pretty(&d).expect("plain data") + "\n";
    }
    let mut out = format!("{} cases, {} variables\n", d.case_count, d.variable_count);
    for v in &d.variables {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\tvalid {}\t{}",
            v.name,
            v.format,
            v.measure,
            v.n_valid,
            v.label.as_deref().unwrap_or("")
        );
    }
    out
}

/// What a session keeps besides its datasets, as `describe` shows it.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct SessionState {
    /// The SET values given, by subcommand name; the others have their defaults.
    pub settings: std::collections::BTreeMap<String, String>,
    /// How many PRESERVE levels wait for a RESTORE.
    pub preserved: usize,
    /// The folder of the last CD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    /// The name of the active dataset; None when it has none, or when there is no data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_dataset: Option<String>,
    /// Every named dataset, the active one first, then the others open or only declared.
    pub datasets: Vec<NamedDataset>,
    pub macros: Vec<String>,
    /// The open output documents, in the order they were opened.
    pub output_documents: Vec<crate::OutputDocument>,
    /// The document new tables go to; None when none is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub designated_output: Option<String>,
}

/// A named dataset as `describe` lists it; a name only declared has no counts.
#[derive(Debug, PartialEq, Serialize)]
pub struct NamedDataset {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cases: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<usize>,
}

/// One entry of the log: the syntax as it was run and a one-line summary or the errors.
pub struct LogEntry {
    pub syntax: String,
    pub result: Result<String, Vec<Failure>>,
    /// The warnings of the run's commands, each as "line N: COMMAND: message".
    pub warnings: Vec<String>,
    /// Taken back by `Session::undo`, so a script made from the log leaves it out.
    pub undone: bool,
}

/// The active dataset kept between runs, and the log of every run.
#[derive(Default)]
pub struct Session {
    engine: Engine,
    pub log: Vec<LogEntry>,
    /// The state before the last run that changed the dataset, and that run's log entry.
    previous: Option<(Engine, usize)>,
}

/// A run taken from a session's current state. `execute` it on any thread, then hand the
/// result to `Session::apply` of the same session, before any other run is applied.
pub struct Job {
    engine: Engine,
    source: String,
    lines: Option<(u32, u32)>,
}

/// A finished `Job`.
pub struct Done {
    check: bool,
    syntax: String,
    result: Result<(Engine, RunResult, String), Vec<Failure>>,
}

impl Session {
    pub fn with_dataset(dataset: Option<Dataset>) -> Self {
        Self {
            engine: Engine::with_dataset(dataset),
            ..Self::default()
        }
    }

    /// The folder of the syntax file, where INSERT, INCLUDE and CD start from; None for the
    /// current folder.
    pub fn set_base_dir(&mut self, dir: Option<std::path::PathBuf>) {
        self.engine.base_dir = dir;
    }

    /// The named datasets other than the active one, and the active one's name.
    pub fn named_datasets(&self) -> (&[(String, Option<Dataset>)], Option<&str>) {
        self.engine.named_datasets()
    }

    /// The syntax that opens a data file from the window: `open` (such as `GET FILE='a.sav'.`)
    /// and a DATASET NAME with the next DataSetn name, so the dataset open before stays open.
    pub fn open_syntax(&self, open: &str) -> String {
        let name = self.engine.next_dataset_name();
        format!("{}\nDATASET NAME {name}.", open.trim_end())
    }

    /// The open output documents, in the order they were opened.
    pub fn output_documents(&self) -> Vec<crate::OutputDocument> {
        self.engine.output_documents()
    }

    /// The output document the next tables go to; None when none is open.
    pub fn designated_output(&self) -> Option<&str> {
        self.engine.designated_output()
    }

    /// The items of an open output document, as OUTPUT SAVE writes them.
    pub fn output_items(&self, document: &str) -> Option<&[crate::output_file::Item]> {
        self.engine.output_items(document)
    }

    /// Deletes items from an output document, as the output window does: `positions` count its
    /// tables, charts and other items in order (`output_file::leaves`). The log shows it as a
    /// comment, so a script made from the log still runs. Call it only while no run goes on: a
    /// run works on a copy of the session and would bring the items back.
    pub fn delete_output(&mut self, document: &str, positions: &[usize]) -> Result<(), String> {
        let titles = self
            .engine
            .delete_output(document, positions)
            .map_err(|e| e.0)?;
        let what = match titles.len() {
            1 => "1 item".to_string(),
            n => format!("{n} items"),
        };
        let titles: Vec<String> = titles
            .iter()
            .map(|t| t.replace(['\n', '\r'], " "))
            .collect();
        self.log.push(LogEntry {
            syntax: format!(
                "* The output window deleted {what} from {document}: {}.",
                titles.join("; ")
            ),
            result: Ok(format!("deleted {what} from {document}")),
            warnings: Vec::new(),
            undone: false,
        });
        Ok(())
    }

    /// The macros and macro settings the runs so far defined.
    pub fn macros(&self) -> &statscraft_syntax::MacroState {
        self.engine.macros()
    }

    /// Puts back the macros a session kept.
    pub fn set_macros(&mut self, macros: statscraft_syntax::MacroState) {
        self.engine.set_macros(macros);
    }

    /// Puts back the named datasets a session kept.
    pub fn set_named_datasets(
        &mut self,
        datasets: Vec<(String, Option<Dataset>)>,
        active: Option<String>,
    ) {
        self.engine.set_named_datasets(datasets, active);
    }

    pub fn dataset(&self) -> Option<&Dataset> {
        self.engine.dataset()
    }

    /// The SET CCA to CCE specifications, to show custom currency values as the output does.
    pub fn currencies(&self) -> crate::Currencies {
        self.engine.currencies()
    }

    /// The value of a SET subcommand, such as TVARS: as SET gave it, else its default.
    pub fn setting(&self, name: &str) -> String {
        self.engine.setting(name)
    }

    /// The SET values, PRESERVE levels and CD folder, as SET, PRESERVE and CD commands.
    pub fn settings_syntax(&self) -> String {
        self.engine.settings_syntax()
    }

    /// Puts back the settings that `settings_syntax` wrote.
    pub fn restore_settings(&mut self, text: &str) -> Result<(), Vec<Failure>> {
        self.engine
            .restore_settings(text)
            .map_err(|e| vec![Failure::plain("session", format!("settings: {e}"))])
    }

    /// Where the random numbers stand after the runs so far: the next run goes on from there.
    pub fn random_state(&self) -> crate::RandomState {
        self.engine.random_state()
    }

    /// Goes on from `state`; call it after `restore_settings`, which puts back the seeds.
    pub fn set_random_state(&mut self, state: crate::RandomState) {
        self.engine.set_random_state(state);
    }

    /// What `statscraft describe --session` prints: the active dataset and what the session
    /// keeps besides it. None when it holds neither.
    pub fn describe(&self, json: bool) -> Option<String> {
        let state = self.engine.session_state();
        let dataset = self.dataset();
        if dataset.is_none() && state == SessionState::default() {
            return None;
        }
        if json {
            #[derive(Serialize)]
            struct Described<'a> {
                #[serde(flatten)]
                dataset: Option<statscraft_data::Description>,
                session: &'a SessionState,
            }
            let described = Described {
                dataset: dataset.map(Dataset::describe),
                session: &state,
            };
            return Some(serde_json::to_string_pretty(&described).expect("plain data") + "\n");
        }
        let mut out = dataset.map_or_else(
            || "no active dataset\n".into(),
            |d| describe_output(d, false),
        );
        let settings: Vec<String> = state
            .settings
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        let mut line = |title: &str, text: String| {
            if !text.is_empty() {
                let _ = writeln!(out, "{title}: {text}");
            }
        };
        line("settings", settings.join(" "));
        if state.preserved > 0 {
            line("PRESERVE levels", state.preserved.to_string());
        }
        line("directory", state.directory.unwrap_or_default());
        line("active dataset", state.active_dataset.unwrap_or_default());
        let datasets = state.datasets.iter().map(|d| match (d.cases, d.variables) {
            (Some(cases), Some(variables)) => {
                format!("{} ({cases} cases, {variables} variables)", d.name)
            }
            _ => format!("{} (declared)", d.name),
        });
        line("datasets", datasets.collect::<Vec<_>>().join(", "));
        line("macros", state.macros.join(", "));
        let documents = state.output_documents.iter();
        let documents = documents.map(|d| format!("{} ({} tables)", d.name, d.tables));
        line("output documents", documents.collect::<Vec<_>>().join(", "));
        line(
            "designated output",
            state.designated_output.unwrap_or_default(),
        );
        Some(out)
    }

    /// Runs `source`, or with `lines` only the commands inside that inclusive line range. All or
    /// nothing: when one command fails, the dataset stays as it was. Every call is logged with
    /// the syntax that ran, which for a line range is just those lines.
    pub fn run(
        &mut self,
        source: &str,
        lines: Option<(u32, u32)>,
    ) -> Result<RunResult, Vec<Failure>> {
        let done = self.job(source, lines, false).execute();
        self.apply(done)
    }

    /// Runs on a copy with file writes skipped; keeps and logs nothing.
    pub fn check(
        &mut self,
        source: &str,
        lines: Option<(u32, u32)>,
    ) -> Result<RunResult, Vec<Failure>> {
        let done = self.job(source, lines, true).execute();
        self.apply(done)
    }

    /// A run of `source` on the current dataset, to `execute` elsewhere. The engine and its
    /// dataset are reference counted, so the copy is cheap and a failed run just drops it.
    pub fn job(&self, source: &str, lines: Option<(u32, u32)>, check: bool) -> Job {
        let mut engine = self.engine.clone();
        engine.dry_run = check;
        Job {
            engine,
            source: source.to_string(),
            lines,
        }
    }

    /// Keeps the dataset of a successful run and logs it; a check changes nothing.
    pub fn apply(&mut self, done: Done) -> Result<RunResult, Vec<Failure>> {
        match done.result {
            Ok((mut engine, result, summary)) => {
                if !done.check {
                    engine.dry_run = false;
                    let before = std::mem::replace(&mut self.engine, engine);
                    if result.saved {
                        // shortcut: one level, holding the changed columns twice; keep a
                        // longer history when someone needs more than the last step back.
                        self.previous = Some((before, self.log.len()));
                    }
                    let warnings = result.commands.iter().flat_map(|c| {
                        c.warnings.iter().map(move |w| {
                            format!("line {}: {}", c.first_line, with_command(&c.name, w))
                        })
                    });
                    self.log.push(LogEntry {
                        syntax: done.syntax,
                        result: Ok(summary),
                        warnings: warnings.collect(),
                        undone: false,
                    });
                }
                Ok(result)
            }
            Err(errors) => {
                if !done.check {
                    self.log.push(LogEntry {
                        syntax: done.syntax,
                        result: Err(errors.clone()),
                        warnings: Vec::new(),
                        undone: false,
                    });
                }
                Err(errors)
            }
        }
    }

    /// The syntax of the run that `undo` would take back.
    pub fn undoable(&self) -> Option<&str> {
        self.previous
            .as_ref()
            .map(|(_, entry)| self.log[*entry].syntax.as_str())
    }

    /// Puts back the dataset from before the last run that changed it, once.
    pub fn undo(&mut self) -> bool {
        let Some((engine, entry)) = self.previous.take() else {
            return false;
        };
        // the output stays: the window keeps showing it, and undo is about the data
        let documents = std::mem::take(&mut self.engine.documents);
        self.engine = engine;
        self.engine.documents = documents;
        self.log[entry].undone = true;
        true
    }
}

impl Job {
    /// The first command's name, for a busy indicator.
    pub fn label(&self) -> String {
        statscraft_syntax::split_commands(&self.source)
            .ok()
            .and_then(|all| all.into_iter().next().map(|c| c.name))
            .unwrap_or_default()
    }

    pub fn execute(self) -> Done {
        let syntax = match self.lines {
            Some((first, last)) => self
                .source
                .lines()
                // a range from 0 or reversed (from a client of the window) fails in `run`
                .skip(first.saturating_sub(1) as usize)
                .take(last.saturating_sub(first) as usize + 1)
                .collect::<Vec<_>>()
                .join("\n"),
            None => self.source.trim_end().to_string(),
        };
        Done {
            check: self.engine.dry_run,
            syntax,
            result: run(self.engine, &self.source, self.lines),
        }
    }
}

/// A run waiting in a `RunQueue`.
pub struct Queued<T> {
    pub source: String,
    pub lines: Option<(u32, u32)>,
    pub check: bool,
    /// The folder of the syntax file the run came from (see `Session::set_base_dir`).
    pub base_dir: Option<PathBuf>,
    /// What the caller wants back with the result, such as the window that asked.
    pub tag: T,
}

struct Running<T> {
    tag: T,
    label: String,
    done: mpsc::Receiver<Done>,
}

/// The runs of every window, one at a time in the order they came: a run asked for while
/// another runs waits its turn, and a failed run does not stop the ones after it. Each run
/// starts from the session as the runs before it left it.
pub struct RunQueue<T> {
    waiting: VecDeque<Queued<T>>,
    running: Option<Running<T>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl<T> RunQueue<T> {
    /// `wake` is called on the worker thread when a run ends, for example to repaint the window.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            waiting: VecDeque::new(),
            running: None,
            wake: Arc::new(wake),
        }
    }

    /// Puts a run at the end of the queue; it starts at once when nothing runs.
    pub fn push(&mut self, session: &mut Session, run: Queued<T>) {
        self.waiting.push_back(run);
        if self.running.is_none() {
            self.start(session);
        }
    }

    /// The tag and the first command's name of the run on the worker thread.
    pub fn running(&self) -> Option<(&T, &str)> {
        self.running.as_ref().map(|r| (&r.tag, r.label.as_str()))
    }

    /// How many runs wait behind the running one.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    pub fn is_idle(&self) -> bool {
        self.running.is_none()
    }

    /// A finished run, applied to the session, with its tag; the next run has started by then.
    /// None while the run goes on or when nothing runs. Call it from the window's update loop.
    pub fn poll(&mut self, session: &mut Session) -> Option<(T, Result<RunResult, Vec<Failure>>)> {
        self.finish(session, false)
    }

    /// Like `poll`, but waits for the running run to end. None when nothing runs.
    pub fn wait(&mut self, session: &mut Session) -> Option<(T, Result<RunResult, Vec<Failure>>)> {
        self.finish(session, true)
    }

    fn finish(
        &mut self,
        session: &mut Session,
        block: bool,
    ) -> Option<(T, Result<RunResult, Vec<Failure>>)> {
        let running = self.running.as_ref()?;
        let done = match block {
            true => running.done.recv().ok(),
            false => match running.done.try_recv() {
                Ok(done) => Some(done),
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => None,
            },
        };
        let running = self.running.take().expect("checked above");
        let result = match done {
            Some(done) => session.apply(done),
            None => Err(vec![Failure::plain("run", "the run stopped unexpectedly")]),
        };
        self.start(session);
        Some((running.tag, result))
    }

    /// Starts the next waiting run, from the session as it is now.
    fn start(&mut self, session: &mut Session) {
        let Some(run) = self.waiting.pop_front() else {
            return;
        };
        session.set_base_dir(run.base_dir);
        let job = session.job(&run.source, run.lines, run.check);
        let label = job.label();
        let (sender, done) = mpsc::channel();
        let wake = Arc::clone(&self.wake);
        std::thread::spawn(move || {
            let _ = sender.send(job.execute());
            wake();
        });
        self.running = Some(Running {
            tag: run.tag,
            label,
            done,
        });
    }
}

fn run(
    mut engine: Engine,
    source: &str,
    lines: Option<(u32, u32)>,
) -> Result<(Engine, RunResult, String), Vec<Failure>> {
    // macros of earlier runs serve this one; those of the lines that run are kept after it
    let start = engine.macros().clone();
    let mut macros = start.clone();
    let all = statscraft_syntax::split_commands_with(
        source,
        engine.base_dir.as_deref(),
        &mut macros,
        lines,
    )
    .map_err(|e| vec![Failure::syntax(e)])?;
    let selected: &[RawCommand] = match lines {
        Some((first, last)) => statscraft_syntax::select_lines(&all, first, last)
            .map_err(|e| vec![Failure::syntax(e)])?,
        None => &all,
    };
    let mut commands: Vec<(RawCommand, Command)> = Vec::new();
    let mut errors = Vec::new();
    for (range, parsed) in statscraft_syntax::parse_raws(selected) {
        match parsed {
            Ok(command) => {
                // a DO REPEAT runs to the line of its END REPEAT
                let raw = RawCommand {
                    last_line: selected[range.end - 1].last_line,
                    ..selected[range.start].clone()
                };
                commands.push((raw, command));
            }
            Err(e) => errors.push(Failure::syntax(e)),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if commands.is_empty() {
        return Err(vec![Failure {
            line: lines.map(|(first, _)| first),
            ..Failure::plain("usage", "no command to run")
        }]);
    }

    let before = engine.dataset().cloned();
    let named = |engine: &Engine| {
        let (datasets, active) = engine.named_datasets();
        (datasets.to_vec(), active.map(str::to_string))
    };
    let named_before = named(&engine);
    let kept = |engine: &Engine| (engine.settings_syntax(), engine.random_state());
    let kept_before = kept(&engine);
    let mut outputs = Vec::new();
    for (raw, command) in &commands {
        let fail = |e: crate::Error| {
            vec![Failure {
                command: Some(raw.name.clone()),
                line: Some(raw.first_line),
                hint: raw.origin.as_ref().map(|origin| format!("in {origin}")),
                ..Failure::plain("run", e.to_string())
            }]
        };
        // SET FORMAT gives the numeric variables a command creates its format
        let names: Vec<String> = match engine.new_variable_format() {
            Some(_) => engine.dataset().map_or_else(Vec::new, |d| {
                d.variable_names().map(str::to_string).collect()
            }),
            None => Vec::new(),
        };
        let mut tables = match engine.execute(command) {
            Ok(tables) => tables,
            Err(e) => {
                let mut failure = fail(e);
                // a statement of a MATRIX program knows its own place
                if let Some((line, column)) = engine.error_position() {
                    failure[0].line = Some(line);
                    failure[0].column = Some(column);
                }
                return Err(failure);
            }
        };
        engine.format_new_variables(command, &names).map_err(fail)?;
        // SET MPRINT=YES: the command as macro expansion made it, before its results
        if let Some(echo) = &raw.echo {
            let rows = echo.lines().map(|l| vec![Cell::Text(l.to_string())]);
            let table = Table {
                title: "Macro expansion".into(),
                columns: vec!["Syntax".into()],
                rows: rows.collect(),
                ..Table::default()
            };
            tables.insert(0, table);
        }
        let syntax = source_lines(source, raw.first_line, raw.last_line);
        let mut document = engine.record_output(&raw.name, &syntax, &tables, false);
        let opened = engine.take_opened().map(|(name, file, items)| {
            document = Some(name);
            (file, items)
        });
        outputs.push(CommandResult {
            name: raw.name.clone(),
            first_line: raw.first_line,
            last_line: raw.last_line,
            tables,
            charts: engine.take_charts(),
            warnings: engine.warnings().to_vec(),
            document,
            opened,
            modified: engine.take_modified(),
        });
    }
    engine
        .finish()
        .map_err(|e| vec![Failure::plain("run", e.to_string())])?;
    // what the pass at the end of the run printed shows with the last command
    let printed = std::mem::take(&mut engine.flushed);
    if let Some(last) = outputs.last_mut() {
        let syntax = source_lines(source, last.first_line, last.last_line);
        let append = last.document.is_some() && last.opened.is_none();
        if let Some(document) = engine.record_output(&last.name, &syntax, &printed, append) {
            last.document = Some(document);
        }
        last.tables.extend(printed);
    }
    // the handles are those the FILE HANDLE commands that ran defined
    macros.set_handles(engine.macros().handles().clone());
    let macros_changed = macros != start;
    engine.set_macros(macros);
    let after = engine.dataset();
    let changes = Changes::between(before.as_ref(), after);
    let summary = summary(&changes, after, &outputs);
    let result = RunResult {
        ok: true,
        session: None,
        trail: None,
        changes,
        dataset: after.map(|d| [d.case_count(), d.variable_count()]),
        // compare whole datasets: a weight change shows in no variable
        saved: !engine.dry_run
            && (before.as_ref() != after
                || named_before != named(&engine)
                || macros_changed
                || kept_before != kept(&engine)),
        commands: outputs,
    };
    Ok((engine, result, summary))
}

/// What a run did, in one line for the log, built from the engine's `Changes`.
pub fn summary(changes: &Changes, after: Option<&Dataset>, outputs: &[CommandResult]) -> String {
    let mut parts = Vec::new();
    let tables: usize = outputs.iter().map(|o| o.tables.len()).sum();
    parts.push(format!(
        "{} command{}",
        outputs.len(),
        if outputs.len() == 1 { "" } else { "s" }
    ));
    if tables > 0 {
        parts.push(format!(
            "{tables} table{}",
            if tables == 1 { "" } else { "s" }
        ));
    }
    let list = |title: &str, names: &[String]| {
        const SHOWN: usize = 8;
        let mut text = names[..names.len().min(SHOWN)].join(", ");
        if names.len() > SHOWN {
            text.push_str(&format!(" and {} more", names.len() - SHOWN));
        }
        format!("{title}: {text}")
    };
    if let (None, Some(data)) = (changes.cases_before, after) {
        parts.push(format!(
            "dataset loaded: {} cases, {} variables",
            data.case_count(),
            data.variable_count()
        ));
    } else if !changes.variables_added.is_empty() {
        parts.push(list("added", &changes.variables_added));
    }
    if !changes.variables_changed.is_empty() {
        parts.push(list("changed", &changes.variables_changed));
    }
    if !changes.variables_removed.is_empty() {
        parts.push(list("removed", &changes.variables_removed));
    }
    if changes.cases_before.is_some() && changes.cases_before != changes.cases_after {
        let count = |n: Option<usize>| n.map_or("none".into(), |n| n.to_string());
        parts.push(format!(
            "cases: {} to {}",
            count(changes.cases_before),
            count(changes.cases_after)
        ));
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn fixture(name: &str) -> String {
        format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    pub fn survey() -> Session {
        let mut session = Session::default();
        let get = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
        session.run(&get, None).unwrap();
        session
    }

    #[test]
    fn a_run_returns_tables_and_logs_its_syntax_and_changes() {
        let mut session = survey();
        assert_eq!(
            session.log[0].result.as_ref().unwrap(),
            "1 command; dataset loaded: 200 cases, 19 variables"
        );
        let result = session
            .run("FREQUENCIES VARIABLES=gender.\nCOMPUTE x = age * 2.", None)
            .unwrap();
        let outputs = &result.commands;
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name, "FREQUENCIES");
        assert_eq!(outputs[0].tables.len(), 1);
        assert_eq!((outputs[1].first_line, outputs[1].last_line), (2, 2));
        assert!(result.saved);
        assert_eq!(result.dataset, Some([200, 20]));
        let entry = &session.log[1];
        assert_eq!(
            entry.syntax,
            "FREQUENCIES VARIABLES=gender.\nCOMPUTE x = age * 2."
        );
        assert_eq!(
            entry.result.as_ref().unwrap(),
            "2 commands; 1 table; added: x"
        );
        assert!(!session.run("FREQUENCIES gender.", None).unwrap().saved);
    }

    #[test]
    fn warnings_reach_the_text_the_json_and_the_log() {
        let mut session = survey();
        let result = session
            .run("COMPUTE x = 1.\nDESCRIPTIVES age plan.", None)
            .unwrap();
        let warning = "no statistics for string variables: plan";
        assert!(result.commands[0].warnings.is_empty());
        assert_eq!(result.commands[1].warnings, [warning]);
        let text = result.output(false, false);
        assert!(
            text.contains(&format!("warning: line 2: DESCRIPTIVES: {warning}\n")),
            "{text}"
        );
        let json: serde_json::Value = serde_json::from_str(&result.output(true, false)).unwrap();
        assert_eq!(json["commands"][0]["warnings"], serde_json::json!([]));
        assert_eq!(json["commands"][1]["warnings"][0], warning);
        assert_eq!(
            session.log[1].warnings,
            [format!("line 2: DESCRIPTIVES: {warning}")]
        );
    }

    #[test]
    fn a_warning_names_its_command_once() {
        let mut session = survey();
        let result = session
            .run("COMPUTE z = $SYSMIS.\nPPLOT age z.", None)
            .unwrap();
        let warning = "PPLOT: z has fewer than two values to plot";
        assert_eq!(result.commands[1].warnings, [warning]);
        let text = result.output(false, false);
        assert!(
            text.contains(&format!("warning: line 2: {warning}\n")),
            "{text}"
        );
        let json: serde_json::Value = serde_json::from_str(&result.output(true, false)).unwrap();
        assert_eq!(json["commands"][1]["warnings"][0], warning);
        assert_eq!(session.log[1].warnings, [format!("line 2: {warning}")]);
    }

    #[test]
    fn a_check_keeps_and_logs_nothing() {
        let mut session = survey();
        let result = session.check("COMPUTE y = 1.", None).unwrap();
        assert_eq!(result.changes.variables_added, ["y"]);
        assert!(!result.saved);
        assert!(session.dataset().unwrap().variable("y").is_err());
        assert_eq!(session.log.len(), 1);
        assert!(
            result
                .output(false, true)
                .ends_with("checked 1 command(s)\n")
        );
    }

    #[test]
    fn a_line_range_runs_only_the_commands_inside_it_and_logs_those_lines() {
        let mut session = survey();
        let source = "COMPUTE a = 1.\nCOMPUTE b = 2.\nCOMPUTE c = 3.";
        session.run(source, Some((2, 3))).unwrap();
        let names: Vec<_> = session.dataset().unwrap().variable_names().collect();
        assert!(!names.contains(&"a") && names.contains(&"b") && names.contains(&"c"));
        assert_eq!(session.log[1].syntax, "COMPUTE b = 2.\nCOMPUTE c = 3.");

        let errors = session
            .run("COMPUTE a\n  = 1.\nCOMPUTE b = 2.", Some((2, 3)))
            .unwrap_err();
        assert!(errors[0].message.contains("cuts through command COMPUTE"));
        assert_eq!(errors[0].line, Some(1));
    }

    #[test]
    fn a_failing_run_changes_nothing_and_reports_line_and_column() {
        let mut session = survey();
        let errors = session
            .run("COMPUTE ok = 1.\nCOMPUTE bad = .", None)
            .unwrap_err();
        assert_eq!((errors[0].line, errors[0].column), (Some(2), Some(15)));
        assert!(session.dataset().unwrap().variable("ok").is_err());

        let errors = session
            .run("COMPUTE ok = 1.\nCOMPUTE bad = nowhere + 1.", None)
            .unwrap_err();
        assert_eq!(errors[0].line, Some(2));
        assert_eq!(
            errors[0].text(),
            "line 2: COMPUTE: unknown variable nowhere"
        );
        assert!(session.dataset().unwrap().variable("ok").is_err());
        assert!(session.log[2].result.is_err());
    }

    #[test]
    fn an_empty_selection_is_an_error_not_a_silent_success() {
        let mut session = Session::default();
        let errors = session.run("\n\n* a comment.\n", None).unwrap_err();
        assert_eq!(errors[0].message, "no command to run");
    }

    #[test]
    fn the_summary_names_removed_variables_and_case_counts() {
        let mut session = survey();
        session.run("DELETE VARIABLES q5_1 TO q5_5.", None).unwrap();
        assert_eq!(
            session.log[1].result.as_ref().unwrap(),
            "1 command; removed: q5_1, q5_2, q5_3, q5_4, q5_5"
        );
        session.run("SELECT IF age > 60.", None).unwrap();
        let summary = session.log[2].result.as_ref().unwrap();
        assert!(summary.contains(" and 6 more; cases: 200 to "), "{summary}");
    }

    #[test]
    fn undo_takes_back_the_last_change_once() {
        let mut session = survey();
        session.run("COMPUTE x = 1.", None).unwrap();
        session.run("FREQUENCIES gender.", None).unwrap();
        // a run that changes nothing leaves the change before it to undo
        assert_eq!(session.undoable(), Some("COMPUTE x = 1."));
        assert!(session.undo());
        assert!(session.dataset().unwrap().variable("x").is_err());
        assert!(session.log[1].undone && !session.log[2].undone);
        assert!(!session.undo());
        assert_eq!(session.dataset().unwrap().variable_count(), 19);
    }

    #[test]
    fn a_job_runs_on_another_thread_and_is_applied_after() {
        let mut session = survey();
        let job = session.job("COMPUTE z = 2.", None, false);
        assert_eq!(job.label(), "COMPUTE");
        let done = std::thread::spawn(move || job.execute()).join().unwrap();
        assert!(session.dataset().unwrap().variable("z").is_err());
        session.apply(done).unwrap();
        assert!(session.dataset().unwrap().variable("z").is_ok());
    }

    #[test]
    fn macros_last_from_run_to_run_but_not_from_a_failed_run_or_a_check() {
        let mut session = survey();
        let result = session
            .run("DEFINE !ten () 10 !ENDDEFINE.\nSET MEXPAND=ON.", None)
            .unwrap();
        // a run that only defines a macro changes the session
        assert!(result.saved);
        session.run("COMPUTE t = !ten.", None).unwrap();
        session
            .run("DEFINE !lost () 1 !ENDDEFINE.\nCOMPUTE x = nowhere.", None)
            .unwrap_err();
        session
            .check("DEFINE !checked () 1 !ENDDEFINE.", None)
            .unwrap();
        assert_eq!(session.macros().names(), ["!TEN"]);
        let errors = session.run("COMPUTE l = !lost.", None).unwrap_err();
        assert!(errors[0].message.contains('!'), "{errors:?}");
        // SET MEXPAND=OFF lasts too
        session.run("SET MEXPAND=OFF.", None).unwrap();
        assert!(session.run("COMPUTE u = !ten.", None).is_err());
    }

    #[test]
    fn settings_preserve_levels_and_cd_round_trip_through_their_syntax() {
        let mut session = survey();
        let result = session
            .run(
                "SET FORMAT=F10.3 CCA='-,EUR ,,' LOCALE='et_EE' DECIMAL=DOT.\nPRESERVE.\n\
                 SET DECIMAL=COMMA.",
                None,
            )
            .unwrap();
        // a run that only changes settings changes the session
        assert!(result.saved);
        let text = session.settings_syntax();
        assert_eq!(
            text,
            // LOCALE first: it sets DECIMAL, which the DECIMAL after it overrides
            "SET LOCALE='et_EE' CCA='-,EUR ,,' DECIMAL='DOT' FORMAT='F10.3'.\nPRESERVE.\n\
             SET DECIMAL='COMMA'.\n"
        );
        let mut next = Session::default();
        next.restore_settings(&text).unwrap();
        assert_eq!(next.settings_syntax(), text);
        // the PRESERVE level came back too: RESTORE returns to the outer settings
        next.run("RESTORE.", None).unwrap();
        assert!(next.settings_syntax().contains("DECIMAL='DOT'"));
        assert!(!next.settings_syntax().contains("COMMA"));
        assert!(next.describe(false).unwrap().contains("FORMAT=F10.3"));

        let cd = "PRESERVE.\nCD 'it''s here'.";
        let mut folder = Session::default();
        folder.restore_settings(cd).unwrap();
        assert_eq!(folder.settings_syntax(), "PRESERVE.\nCD 'it''s here'.\n");
        let wrong = folder.restore_settings("COMPUTE x = 1.").unwrap_err();
        assert!(
            wrong[0]
                .message
                .contains("only FILE HANDLE, SET, PRESERVE and CD"),
            "{wrong:?}"
        );
    }

    #[test]
    fn settings_last_from_run_to_run_but_not_from_a_failed_run_or_a_check() {
        let mut session = survey();
        session.run("SET FORMAT=F6.1.", None).unwrap();
        session
            .run("SET FORMAT=F4.0.\nCOMPUTE x = nowhere.", None)
            .unwrap_err();
        session.check("SET FORMAT=F3.0.", None).unwrap();
        assert_eq!(session.settings_syntax(), "SET FORMAT='F6.1'.\n");
        session.run("COMPUTE y = 1.", None).unwrap();
        let y = session.dataset().unwrap().variable("y").unwrap();
        assert_eq!(y.format(), "F6.1");
    }

    #[test]
    fn describe_shows_the_session_state_beside_the_dataset() {
        let mut session = Session::default();
        assert!(session.describe(true).is_none());
        session
            .run("SET MPRINT=YES.\nDEFINE !m () 1 !ENDDEFINE.", None)
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&session.describe(true).unwrap()).unwrap();
        assert_eq!(json["session"]["settings"]["MPRINT"], "YES");
        assert_eq!(json["session"]["macros"][0], "!M");
        assert!(json.get("variables").is_none());
        let mut session = survey();
        session.run("DATASET NAME main.", None).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&session.describe(true).unwrap()).unwrap();
        assert_eq!(json["case_count"], 200);
        assert_eq!(json["session"]["active_dataset"], "main");
        let text = session.describe(false).unwrap();
        assert!(
            text.ends_with("active dataset: main\ndatasets: main (200 cases, 19 variables)\n"),
            "{text}"
        );
    }

    #[test]
    fn a_run_ends_temporary_n_of_cases_and_an_unfinished_data_list() {
        let mut session = survey();
        session
            .run("TEMPORARY.\nSELECT IF age > 60.", None)
            .unwrap();
        assert_eq!(session.dataset().unwrap().case_count(), 200);
        session.run("N OF CASES 10.", None).unwrap();
        assert_eq!(session.dataset().unwrap().case_count(), 10);
        let errors = session.run("DATA LIST LIST /x.", None).unwrap_err();
        assert!(errors[0].message.contains("BEGIN DATA"), "{errors:?}");
    }

    #[test]
    fn random_numbers_go_on_from_the_last_run() {
        let mut session = survey();
        let result = session
            .run("TEMPORARY.\nSAMPLE .5.\nFREQUENCIES gender.", None)
            .unwrap();
        // the draws changed the session though the dataset did not
        assert!(result.saved);
        let state = session.random_state();
        assert_ne!(state, survey().random_state());
        let mut next = survey();
        next.set_random_state(state);
        session.run("SAMPLE .5.", None).unwrap();
        next.run("SAMPLE .5.", None).unwrap();
        assert!(session.dataset() == next.dataset());
    }

    fn queued(source: &str, tag: u32) -> Queued<u32> {
        Queued {
            source: source.into(),
            lines: None,
            check: false,
            base_dir: None,
            tag,
        }
    }

    #[test]
    fn the_queue_runs_in_the_order_runs_came_each_on_the_state_before_it() {
        let mut session = survey();
        let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        let mut queue = RunQueue::new(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(queue.is_idle() && queue.wait(&mut session).is_none());
        queue.push(&mut session, queued("COMPUTE a = 1.", 1));
        queue.push(&mut session, queued("COMPUTE b = a + 1.", 2));
        queue.push(&mut session, queued("COMPUTE c = b + 1.", 3));
        // the first runs, the others wait instead of failing
        assert_eq!(queue.running(), Some((&1, "COMPUTE")));
        assert_eq!(queue.waiting(), 2);
        let mut tags = Vec::new();
        while let Some((tag, result)) = queue.wait(&mut session) {
            assert!(result.unwrap().saved);
            tags.push(tag);
        }
        assert_eq!(tags, [1, 2, 3]);
        assert!(session.dataset().unwrap().variable("c").is_ok());
        // the worker wakes the window just after it sends the result, so the last wake may
        // come a moment after `wait` returned
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while woken.load(std::sync::atomic::Ordering::SeqCst) < 3
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 3);
        let syntax: Vec<_> = session.log.iter().map(|e| e.syntax.as_str()).collect();
        assert_eq!(
            syntax[1..],
            ["COMPUTE a = 1.", "COMPUTE b = a + 1.", "COMPUTE c = b + 1."]
        );
    }

    #[test]
    fn a_failed_run_in_the_queue_does_not_stop_the_next() {
        let mut session = survey();
        let mut queue = RunQueue::new(|| {});
        queue.push(&mut session, queued("COMPUTE a = 1.", 1));
        queue.push(&mut session, queued("COMPUTE bad = nowhere.", 2));
        queue.push(&mut session, queued("COMPUTE c = a + 1.", 3));
        let mut results = Vec::new();
        while let Some((tag, result)) = queue.wait(&mut session) {
            results.push((tag, result.is_ok()));
        }
        assert_eq!(results, [(1, true), (2, false), (3, true)]);
        let data = session.dataset().unwrap();
        assert!(data.variable("bad").is_err() && data.variable("c").is_ok());
        assert!(session.log[2].result.is_err() && session.log[3].result.is_ok());
        // poll returns nothing once the queue is empty
        assert!(queue.poll(&mut session).is_none());
    }

    #[test]
    fn a_file_opened_from_the_window_gets_the_next_dataset_name_and_keeps_the_one_before() {
        let get = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
        let mut session = Session::default();
        let first = session.open_syntax(&get);
        assert_eq!(first, format!("{get}\nDATASET NAME DataSet1."));
        session.run(&first, None).unwrap();
        session.run("COMPUTE x = 1.", None).unwrap();
        let second = session.open_syntax(&get);
        assert!(second.ends_with("DATASET NAME DataSet2."), "{second}");
        session.run(&second, None).unwrap();
        let (others, active) = session.named_datasets();
        assert_eq!(active, Some("DataSet2"));
        assert_eq!(others[0].0, "DataSet1");
        // DataSet1 kept its state from before
        assert!(others[0].1.as_ref().unwrap().variable("x").is_ok());
        // a closed name is not given again, and a typed DataSetn counts
        session.run("DATASET CLOSE DataSet1.", None).unwrap();
        assert!(session.open_syntax(&get).ends_with("DataSet3."));
        session.run("DATASET COPY dataset7.", None).unwrap();
        assert!(session.open_syntax(&get).ends_with("DataSet8."));
        // a failed run gives no name away
        let missing = session.open_syntax("GET FILE='no such file.sav'.");
        session.run(&missing, None).unwrap_err();
        assert!(session.open_syntax(&get).ends_with("DataSet8."));
    }

    #[test]
    fn an_unnamed_active_dataset_is_gone_when_another_file_is_opened() {
        let get = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
        let mut session = survey();
        let open = session.open_syntax(&get);
        session.run(&open, None).unwrap();
        let (others, active) = session.named_datasets();
        assert_eq!((others.len(), active), (0, Some("DataSet1")));
    }

    #[test]
    fn dataset_close_keeps_the_active_data_without_a_name() {
        let mut session = survey();
        session
            .run("DATASET NAME first.\nDATASET COPY second.", None)
            .unwrap();
        session.run("DATASET CLOSE first.", None).unwrap();
        // the active dataset stays active, unnamed; a closed other one is gone
        assert_eq!(session.dataset().unwrap().case_count(), 200);
        let display = session.run("DATASET DISPLAY.", None).unwrap();
        let rows = &display.commands[0].tables[0].rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Cell::Text("(unnamed)".into()));
        session.run("DATASET CLOSE second.", None).unwrap();
        assert!(session.named_datasets().0.is_empty());
        let error = session.run("DATASET CLOSE second.", None).unwrap_err();
        assert!(error[0].message.contains("no dataset named second"));
        // DATASET CLOSE * of a named active dataset only takes the name
        session
            .run("DATASET NAME again.\nDATASET CLOSE *.", None)
            .unwrap();
        assert_eq!(session.named_datasets().1, None);
        assert!(session.dataset().is_some());
    }

    #[test]
    fn each_command_names_the_output_document_its_tables_went_to() {
        let mut session = survey();
        assert_eq!(session.designated_output(), None);
        let result = session
            .run(
                "FREQUENCIES gender.\nOUTPUT NEW NAME=second.\nCOMPUTE x = 1.\nFREQUENCIES gender.",
                None,
            )
            .unwrap();
        let documents: Vec<_> = result
            .commands
            .iter()
            .map(|c| c.document.as_deref())
            .collect();
        assert_eq!(documents, [Some("Document1"), None, None, Some("second")]);
        assert_eq!(session.designated_output(), Some("second"));
        let json: serde_json::Value = serde_json::from_str(&result.output(true, false)).unwrap();
        assert_eq!(json["commands"][3]["document"], "second");
        assert!(json["commands"][1].get("document").is_none());
        session.run("OUTPUT ACTIVATE Document1.", None).unwrap();
        assert_eq!(session.designated_output(), Some("Document1"));
        let listed = session.output_documents();
        let names: Vec<_> = listed.iter().map(|d| (d.name.as_str(), d.tables)).collect();
        assert_eq!(names, [("Document1", 1), ("second", 1)]);
        session.run("OUTPUT CLOSE Document1.", None).unwrap();
        assert_eq!(session.designated_output(), None);
    }

    #[test]
    fn describe_lists_the_datasets_and_output_documents() {
        let get = format!("GET FILE='{}'.", fixture("survey_basic.sav"));
        let mut session = Session::default();
        session.run(&session.open_syntax(&get), None).unwrap();
        let second = session.open_syntax(&get);
        session.run(&second, None).unwrap();
        session
            .run(
                "DATASET DECLARE later.\nFREQUENCIES gender.\nOUTPUT NEW NAME=men.",
                None,
            )
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&session.describe(true).unwrap()).unwrap();
        let state = &json["session"];
        assert_eq!(state["active_dataset"], "DataSet2");
        assert_eq!(
            state["datasets"],
            serde_json::json!([
                {"name": "DataSet2", "cases": 200, "variables": 19},
                {"name": "DataSet1", "cases": 200, "variables": 19},
                {"name": "later"}
            ])
        );
        assert_eq!(
            state["output_documents"],
            serde_json::json!([
                {"name": "Document1", "tables": 1, "unsaved": true},
                {"name": "men", "tables": 0}
            ])
        );
        assert_eq!(state["designated_output"], "men");
        let text = session.describe(false).unwrap();
        assert!(
            text.contains(
                "datasets: DataSet2 (200 cases, 19 variables), DataSet1 (200 cases, 19 \
                 variables), later (declared)\n"
            ),
            "{text}"
        );
        assert!(
            text.ends_with(
                "output documents: Document1 (1 tables), men (0 tables)\ndesignated output: men\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_line_range_keeps_only_the_macros_its_lines_define() {
        let mut session = survey();
        let source = "DEFINE !a () 1 !ENDDEFINE.\nDEFINE !b () 2 !ENDDEFINE.\nCOMPUTE b = !b + !a.";
        // the macro on line 1 serves the call on line 3, as before, but is not kept
        session.run(source, Some((2, 3))).unwrap();
        assert_eq!(session.macros().names(), ["!B"]);
        let result = session.run(
            "DISPLAY MACROS.\nDEFINE !c () 3 !ENDDEFINE.\nDISPLAY MACROS.",
            None,
        );
        let commands = result.unwrap().commands;
        let names = |c: &CommandResult| c.tables[0].rows.len();
        assert_eq!((names(&commands[0]), names(&commands[2])), (1, 2));
    }
}
