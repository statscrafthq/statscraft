//! Syntax text run against a dataset that is kept between runs: the one path shared by
//! `statscraft run`, the window's Run button and the window's live session. Splits, selects,
//! parses and executes; a run is all or nothing.

use crate::{Cell, Changes, Engine, Table, format_value};
use serde::{Deserialize, Serialize};
use statscraft_data::Dataset;
use statscraft_syntax::{Command, RawCommand, SyntaxError};
use std::fmt::Write;

/// One error, at a line and column of the syntax when known. `kind` is "syntax", "run",
/// "usage", "session" or "file".
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

/// The tables of one command, with the lines it came from.
#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub name: String,
    pub first_line: u32,
    pub last_line: u32,
    pub tables: Vec<Table>,
}

/// What a run did. Serialized, it is the JSON that `statscraft run --json` prints.
#[derive(Debug, Serialize)]
pub struct RunResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
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
        if json {
            return serde_json::to_string_pretty(self).expect("plain data") + "\n";
        }
        let mut out = String::new();
        for table in self.commands.iter().flat_map(|c| &c.tables) {
            markdown(&mut out, table);
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

fn markdown(out: &mut String, table: &Table) {
    let _ = writeln!(out, "### {}\n", table.title);
    let _ = writeln!(out, "| {} |", table.column_titles().join(" | "));
    let _ = writeln!(out, "|{}", " --- |".repeat(table.columns.len()));
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

/// One entry of the log: the syntax as it was run and a one-line summary or the errors.
pub struct LogEntry {
    pub syntax: String,
    pub result: Result<String, Vec<Failure>>,
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

    pub fn dataset(&self) -> Option<&Dataset> {
        self.engine.dataset()
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
                    self.log.push(LogEntry {
                        syntax: done.syntax,
                        result: Ok(summary),
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
        self.engine = engine;
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
                .skip(first as usize - 1)
                .take((last - first + 1) as usize)
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

fn run(
    mut engine: Engine,
    source: &str,
    lines: Option<(u32, u32)>,
) -> Result<(Engine, RunResult, String), Vec<Failure>> {
    let all = statscraft_syntax::split_commands(source).map_err(|e| vec![Failure::syntax(e)])?;
    let selected: &[RawCommand] = match lines {
        Some((first, last)) => statscraft_syntax::select_lines(&all, first, last)
            .map_err(|e| vec![Failure::syntax(e)])?,
        None => &all,
    };
    let mut commands: Vec<(&RawCommand, Command)> = Vec::new();
    let mut errors = Vec::new();
    for raw in selected {
        match statscraft_syntax::parse_command(raw) {
            Ok(command) => commands.push((raw, command)),
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
    let mut outputs = Vec::new();
    for (raw, command) in &commands {
        let tables = engine.execute(command).map_err(|e| {
            vec![Failure {
                command: Some(raw.name.clone()),
                line: Some(raw.first_line),
                ..Failure::plain("run", e.to_string())
            }]
        })?;
        outputs.push(CommandResult {
            name: raw.name.clone(),
            first_line: raw.first_line,
            last_line: raw.last_line,
            tables,
        });
    }
    engine
        .finish()
        .map_err(|e| vec![Failure::plain("run", e.to_string())])?;
    let after = engine.dataset();
    let changes = Changes::between(before.as_ref(), after);
    let summary = summary(&changes, after, &outputs);
    let result = RunResult {
        ok: true,
        session: None,
        changes,
        dataset: after.map(|d| [d.case_count(), d.variable_count()]),
        // compare whole datasets: a weight change shows in no variable
        saved: !engine.dry_run && before.as_ref() != after,
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
}
