//! The one path by which the window changes data or makes output: syntax text, split, parsed and
//! executed by the engine exactly like `statscraft run` does it, and written to the log.

use statscraft_data::Dataset;
use statscraft_engine::{Changes, Engine, Table};
use statscraft_syntax::{Command, RawCommand, SyntaxError};

/// One error of a run, at a line and column of the syntax that was run when known.
#[derive(Debug, Clone, PartialEq)]
pub struct RunError {
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

impl RunError {
    fn syntax(e: SyntaxError) -> Self {
        let mut message = e.message;
        if !e.expected.is_empty() {
            message.push_str(&format!("; expected: {}", e.expected.join(", ")));
        }
        if let Some(hint) = e.hint {
            message.push_str(&format!("; hint: {hint}"));
        }
        Self {
            line: Some(e.span.line),
            column: Some(e.span.column),
            message,
        }
    }

    pub fn text(&self) -> String {
        match (self.line, self.column) {
            (Some(line), Some(column)) => format!("line {line}, column {column}: {}", self.message),
            (Some(line), None) => format!("line {line}: {}", self.message),
            _ => self.message.clone(),
        }
    }
}

/// The tables of one command, with the lines it came from.
#[derive(Debug)]
pub struct CommandOutput {
    pub name: String,
    pub first_line: u32,
    pub last_line: u32,
    pub tables: Vec<Table>,
}

/// One entry of the log: the syntax as it was run and what came of it.
pub struct LogEntry {
    pub syntax: String,
    pub result: Result<String, Vec<RunError>>,
}

#[derive(Default)]
pub struct Session {
    engine: Engine,
    pub log: Vec<LogEntry>,
}

impl Session {
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
    ) -> Result<Vec<CommandOutput>, Vec<RunError>> {
        let syntax = match lines {
            Some((first, last)) => source
                .lines()
                .skip(first as usize - 1)
                .take((last - first + 1) as usize)
                .collect::<Vec<_>>()
                .join("\n"),
            None => source.trim_end().to_string(),
        };
        let result = self.execute(source, lines);
        self.log.push(LogEntry {
            syntax,
            result: result
                .as_ref()
                .map(|(_, summary)| summary.clone())
                .map_err(Clone::clone),
        });
        result.map(|(outputs, _)| outputs)
    }

    fn execute(
        &mut self,
        source: &str,
        lines: Option<(u32, u32)>,
    ) -> Result<(Vec<CommandOutput>, String), Vec<RunError>> {
        let all =
            statscraft_syntax::split_commands(source).map_err(|e| vec![RunError::syntax(e)])?;
        let selected: &[RawCommand] = match lines {
            Some((first, last)) => statscraft_syntax::select_lines(&all, first, last)
                .map_err(|e| vec![RunError::syntax(e)])?,
            None => &all,
        };
        let mut commands: Vec<(&RawCommand, Command)> = Vec::new();
        let mut errors = Vec::new();
        for raw in selected {
            match statscraft_syntax::parse_command(raw) {
                Ok(command) => commands.push((raw, command)),
                Err(e) => errors.push(RunError::syntax(e)),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        if commands.is_empty() {
            return Err(vec![RunError {
                line: lines.map(|(first, _)| first),
                column: None,
                message: "no command to run".into(),
            }]);
        }

        // the engine and its dataset are reference counted, so a failed run just drops the copy
        let mut engine = self.engine.clone();
        let mut outputs = Vec::new();
        for (raw, command) in &commands {
            let tables = engine.execute(command).map_err(|e| {
                vec![RunError {
                    line: Some(raw.first_line),
                    column: None,
                    message: format!("{}: {e}", raw.name),
                }]
            })?;
            outputs.push(CommandOutput {
                name: raw.name.clone(),
                first_line: raw.first_line,
                last_line: raw.last_line,
                tables,
            });
        }
        engine.finish().map_err(|e| {
            vec![RunError {
                line: None,
                column: None,
                message: e.to_string(),
            }]
        })?;
        let changes = Changes::between(self.engine.dataset(), engine.dataset());
        let summary = summary(&changes, engine.dataset(), &outputs);
        self.engine = engine;
        Ok((outputs, summary))
    }
}

/// What a run did, in one line for the log, built from the engine's `Changes`.
pub fn summary(changes: &Changes, after: Option<&Dataset>, outputs: &[CommandOutput]) -> String {
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
        let outputs = session
            .run("FREQUENCIES VARIABLES=gender.\nCOMPUTE x = age * 2.", None)
            .unwrap();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name, "FREQUENCIES");
        assert_eq!(outputs[0].tables.len(), 1);
        assert_eq!((outputs[1].first_line, outputs[1].last_line), (2, 2));
        let entry = &session.log[1];
        assert_eq!(
            entry.syntax,
            "FREQUENCIES VARIABLES=gender.\nCOMPUTE x = age * 2."
        );
        assert_eq!(
            entry.result.as_ref().unwrap(),
            "2 commands; 1 table; added: x"
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
}
