//! Syntax text run against a dataset that is kept between runs: the one path shared by
//! `statscraft run`, the window's Run button and the window's live session. Splits, selects,
//! parses and executes; a run is all or nothing.

use crate::{Cell, Changes, Engine, Table, cell_text};
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
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub name: String,
    pub first_line: u32,
    pub last_line: u32,
    pub tables: Vec<Table>,
    /// What the command did differently from what was asked, without failing.
    pub warnings: Vec<String>,
}

/// What a run did. Serialized, it is the JSON that `statscraft run --json` prints; the command
/// line reads it back from the window to write a workbook.
#[derive(Debug, Serialize, Deserialize)]
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
        for command in &self.commands {
            for table in &command.tables {
                markdown(&mut out, table);
            }
            for warning in &command.warnings {
                let _ = writeln!(
                    out,
                    "warning: line {}: {}: {warning}",
                    command.first_line, command.name
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_dataset: Option<String>,
    /// The other named datasets, open or declared.
    pub datasets: Vec<String>,
    pub macros: Vec<String>,
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
        line("other datasets", state.datasets.join(", "));
        line("macros", state.macros.join(", "));
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
                        let place = format!("line {}: {}", c.first_line, c.name);
                        c.warnings.iter().map(move |w| format!("{place}: {w}"))
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
        let mut tables = engine.execute(command).map_err(fail)?;
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
        outputs.push(CommandResult {
            name: raw.name.clone(),
            first_line: raw.first_line,
            last_line: raw.last_line,
            tables,
            warnings: engine.warnings().to_vec(),
        });
    }
    engine
        .finish()
        .map_err(|e| vec![Failure::plain("run", e.to_string())])?;
    // what the pass at the end of the run printed shows with the last command
    let printed = std::mem::take(&mut engine.flushed);
    if let Some(last) = outputs.last_mut() {
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
                "SET FORMAT=F10.3 CCA='-,EUR ,,' LOCALE='et_EE'.\nPRESERVE.\nSET DECIMAL=COMMA.",
                None,
            )
            .unwrap();
        // a run that only changes settings changes the session
        assert!(result.saved);
        let text = session.settings_syntax();
        assert_eq!(
            text,
            "SET CCA='-,EUR ,,' FORMAT='F10.3' LOCALE='et_EE'.\nPRESERVE.\nSET DECIMAL='COMMA'.\n"
        );
        let mut next = Session::default();
        next.restore_settings(&text).unwrap();
        assert_eq!(next.settings_syntax(), text);
        // the PRESERVE level came back too: RESTORE returns to the outer settings
        next.run("RESTORE.", None).unwrap();
        assert!(!next.settings_syntax().contains("DECIMAL"));
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
        assert!(text.ends_with("active dataset: main\n"), "{text}");
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
