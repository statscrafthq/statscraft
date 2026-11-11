//! TITLE, SUBTITLE, ECHO, NEW FILE, CLEAR TRANSFORMATIONS, ERASE, SYSFILE INFO, PERMISSIONS,
//! HOST, PRINT FORMATS and WRITE FORMATS.
//!
//! Rules, from the descriptions of these commands:
//! - TITLE and SUBTITLE take at most 60 bytes; a longer text is cut. ECHO shows its text.
//! - NEW FILE starts an empty active dataset; it neither reads the data nor runs the pending
//!   transformations.
//! - CLEAR TRANSFORMATIONS drops the transformations given since the last command that read
//!   the data; the dataset is as that command left it, dictionary changes included.
//! - ERASE deletes one file, named in full.
//! - SYSFILE INFO shows the dictionary of a data file without reading it as the active dataset.
//! - PERMISSIONS READONLY makes a file read-only for everyone; WRITEABLE lets its owner write it
//!   and leaves the others as they are.
//! - PRINT FORMATS sets the print format only, WRITE FORMATS the write format only.
//!
//! Choices where the manual is silent:
//! - HOST is never run: a syntax file that runs operating system commands can do anything the
//!   user can, so shared syntax would be a security risk. The command is an error that says so.
//! - Statscraft runs transformations as they come, so CLEAR TRANSFORMATIONS returns to a copy of
//!   the state the last data pass left (the dataset, N OF CASES, TEMPORARY and the vectors).
//!   The end of a run ends the data pass, like a procedure. Commands that touch no data (SET,
//!   TITLE, ECHO, DISPLAY and the like) are not undone.
//! - A blank TITLE or SUBTITLE shows nothing. ERASE of a file that does not exist is an error.

use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{Command, Display, Format};
use std::path::Path;

/// How a command stands to CLEAR TRANSFORMATIONS.
#[derive(PartialEq)]
pub(crate) enum Stage {
    /// It reads the data or replaces the dataset: what came before can no longer be cleared.
    Pass,
    /// It touches no data.
    Neutral,
    /// A transformation or dictionary change, which CLEAR TRANSFORMATIONS undoes.
    Change,
}

pub(crate) fn stage(command: &Command) -> Stage {
    match command {
        Command::Frequencies(_)
        | Command::Descriptives(_)
        | Command::Crosstabs(_)
        | Command::MultResponse(_)
        | Command::Ctables(_)
        | Command::Means(_)
        | Command::TTest(_)
        | Command::Oneway(_)
        | Command::Correlations(_)
        | Command::NonparCorr(_)
        | Command::NparTests(_)
        | Command::PartialCorr(_)
        | Command::Examine(_)
        | Command::Regression(_)
        | Command::Reliability(_)
        | Command::Factor(_)
        | Command::Cluster(_)
        | Command::Discriminant(_)
        | Command::Anova(_)
        | Command::QuickCluster(_)
        | Command::Tree(_)
        | Command::Rank(_)
        | Command::Aggregate(_)
        | Command::Autorecode(_)
        | Command::List(_)
        | Command::Execute
        | Command::Save { .. }
        | Command::SaveTranslate(_)
        | Command::Get { .. }
        | Command::GetData(_)
        | Command::DataList(_)
        | Command::MatrixData(_)
        | Command::Report(_)
        | Command::BeginData(_)
        | Command::AddFiles(_)
        | Command::MatchFiles(_)
        | Command::StarJoin(_)
        | Command::Update(_)
        | Command::SortCases(_)
        | Command::Flip(_)
        | Command::VarsToCases(_)
        | Command::CasesToVars(_)
        | Command::Dataset(_)
        | Command::NewFile
        | Command::Date(_) => Stage::Pass,
        Command::Title { .. }
        | Command::Echo(_)
        | Command::Erase(_)
        | Command::SysfileInfo(_)
        | Command::Permissions { .. }
        | Command::Host(_)
        | Command::Set(_)
        | Command::Show(_)
        | Command::Preserve(_)
        | Command::Restore(_)
        | Command::Cd(_)
        | Command::FileHandle { .. }
        | Command::Define(_)
        | Command::Insert { .. }
        | Command::BeginProgram
        | Command::Cache
        | Command::Display(_)
        | Command::Output(_)
        | Command::Finish
        | Command::ClearTransformations => Stage::Neutral,
        _ => Stage::Change,
    }
}

/// What CLEAR TRANSFORMATIONS returns to.
#[derive(Clone)]
pub(crate) struct Clean {
    dataset: Option<Dataset>,
    case_limit: Option<usize>,
    temporary: Option<Box<crate::split::Saved>>,
    vectors: std::collections::HashMap<String, Vec<String>>,
    /// The commands since then, which LEAVE runs again.
    commands: Vec<Command>,
}

fn one_line(title: &str, text: &str) -> Table {
    Table {
        title: title.into(),
        columns: vec!["Text".into()],
        rows: vec![vec![Cell::Text(text.into())]],
        ..Table::default()
    }
}

/// The text cut to at most `bytes` bytes, at a character boundary.
fn cut(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Makes a file read-only for everyone, or writeable for its owner.
pub(crate) fn set_permissions(path: &Path, read_only: bool) -> Result<(), Error> {
    let fail = |e: std::io::Error| Error(format!("{}: {e}", path.display()));
    let mut permissions = std::fs::metadata(path).map_err(fail)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        permissions.set_mode(if read_only {
            mode & !0o222
        } else {
            mode | 0o200
        });
    }
    #[cfg(not(unix))]
    permissions.set_readonly(read_only);
    std::fs::set_permissions(path, permissions).map_err(fail)
}

fn format_text(format: &Format) -> String {
    let mut text = format!("{}{}", format.kind.name(), format.width);
    if format.decimals > 0 {
        text.push_str(&format!(".{}", format.decimals));
    }
    text
}

impl Engine {
    /// At every command, before it runs: keeps what CLEAR TRANSFORMATIONS returns to.
    pub(crate) fn track_clean(&mut self, command: &Command) {
        if self.row.is_some() || self.loop_depth > 0 || self.input.is_some() {
            return;
        }
        if self.collecting.is_none() {
            match stage(command) {
                Stage::Change if self.clean.is_none() => {
                    self.clean = Some(Box::new(Clean {
                        dataset: self.dataset.clone(),
                        case_limit: self.case_limit,
                        temporary: self.temporary.clone(),
                        vectors: self.vectors.clone(),
                        commands: Vec::new(),
                    }))
                }
                Stage::Pass => self.clean = None,
                _ => {}
            }
        }
        if let Some(clean) = self.clean.as_mut()
            && !matches!(command, Command::Leave(_))
        {
            clean.commands.push(command.clone());
        }
    }

    /// True when a command since the last data pass names the variable.
    pub(crate) fn defined_since_pass(&self, name: &str) -> bool {
        self.clean.as_ref().is_some_and(|clean| {
            let refs = crate::rows::Refs::of(&clean.commands);
            refs.names.iter().any(|n| n.eq_ignore_ascii_case(name))
        })
    }

    /// LEAVE outside an input program. The variables are new in this pass, so the
    /// transformations since the last pass run again, case by case, with the variables kept
    /// from case to case.
    pub(crate) fn leave_in_pass(&mut self, names: Vec<String>) -> Result<(), Error> {
        let new = |clean: &Clean| {
            let old = clean.dataset.as_ref();
            names
                .iter()
                .all(|n| old.is_none_or(|d| d.variable_index(n).is_none()))
        };
        let clean = match self.clean.take() {
            Some(clean) if new(&clean) => clean,
            other => {
                self.clean = other;
                return Err(Error(format!(
                    "LEAVE takes variables that the transformations since the data were last \
                     read created; {} existed before",
                    names.join(", ")
                )));
            }
        };
        self.leave.extend(names.iter().map(|n| n.to_lowercase()));
        self.dataset = clean.dataset;
        self.case_limit = clean.case_limit;
        self.temporary = clean.temporary;
        self.vectors = clean.vectors;
        self.blocks.clear();
        self.scratch_assigned.clear();
        self.pending = Some(Vec::new());
        self.pending_depth = 0;
        let warnings = std::mem::take(&mut self.warnings);
        for command in &clean.commands {
            self.execute(command)?;
        }
        self.warnings = warnings;
        Ok(())
    }

    /// The transformations kept for the case-by-case pass are dropped, unrun.
    pub(crate) fn drop_pending(&mut self) {
        self.pending = None;
        self.pending_depth = 0;
    }

    fn clear_transformations(&mut self) {
        self.blocks.clear();
        self.scratch_assigned.clear();
        match self.clean.take() {
            Some(clean) => {
                self.dataset = clean.dataset;
                self.case_limit = clean.case_limit;
                self.temporary = clean.temporary;
                self.vectors = clean.vectors;
            }
            None => self.warn("there are no transformations to clear"),
        }
    }

    pub(crate) fn execute_output(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        match command {
            Command::Title { text, subtitle } => {
                let text = cut(text, 60);
                if text.trim().is_empty() {
                    return Ok(Vec::new());
                }
                if !*subtitle {
                    self.documents.title = Some(text.to_string());
                }
                let title = if *subtitle { "Subtitle" } else { "Title" };
                return Ok(vec![one_line(title, text)]);
            }
            Command::Echo(text) => return Ok(vec![one_line("Echo", text)]),
            Command::NewFile => {
                self.park_active();
                self.dataset = None;
                self.temporary = None;
                self.case_limit = None;
                self.scratch_assigned.clear();
                self.vectors.clear();
            }
            Command::ClearTransformations => self.clear_transformations(),
            Command::Erase(file) => {
                let path = self.path(file);
                let fail = |e: std::io::Error| Error(format!("{}: {e}", path.display()));
                if !path.is_file() {
                    return Err(Error(format!("{}: no such file", path.display())));
                }
                if !self.dry_run {
                    std::fs::remove_file(&path).map_err(fail)?;
                }
            }
            Command::Permissions { file, read_only } => {
                let path = self.path(file);
                if !path.exists() {
                    return Err(Error(format!("{}: no such file", path.display())));
                }
                if !self.dry_run {
                    set_permissions(&path, *read_only)?;
                }
            }
            Command::Host(_) => {
                return Err(Error(
                    "HOST is refused: Statscraft never runs operating system commands from \
                     syntax, because a shared syntax file could then do anything you can; run \
                     the commands outside Statscraft"
                        .into(),
                ));
            }
            Command::SysfileInfo(file) => return self.sysfile_info(file),
            Command::PrintFormats(groups) | Command::WriteFormats(groups) => {
                let print = matches!(command, Command::PrintFormats(_));
                for (vars, format) in groups {
                    let text = format_text(format);
                    for name in self.expand(vars)? {
                        let data = self.active_mut()?;
                        if print {
                            let write = data.variable(&name)?.write_format().to_string();
                            data.set_format(&name, &text)?;
                            data.set_write_format(&name, &write)?;
                        } else {
                            data.set_write_format(&name, &text)?;
                        }
                    }
                }
            }
            Command::Date(spec) => return self.date(spec),
            Command::Print(_) | Command::PrintSpace(_) | Command::Xsave(_) => {
                return self.case_output(command);
            }
            _ => return Err(Error("this command cannot run in the engine yet".into())),
        }
        Ok(Vec::new())
    }

    fn sysfile_info(&mut self, file: &str) -> Result<Vec<Table>, Error> {
        let path = self.path(file);
        let data = Dataset::read_sav(&path)?;
        let mut rows = vec![
            ("File", Cell::Text(path.display().to_string())),
            (
                "Label",
                Cell::Text(data.file_label().unwrap_or_default().into()),
            ),
            ("Cases", Cell::Number(data.case_count() as f64)),
            ("Variables", Cell::Number(data.variable_count() as f64)),
            (
                "Weight",
                Cell::Text(data.weight_variable().unwrap_or("none").into()),
            ),
            ("Encoding", Cell::Text(data.encoding().into())),
            ("Compression", Cell::Text(data.compression().into())),
            ("Documents", Cell::Number(data.documents().len() as f64)),
            (
                "Multiple response sets",
                Cell::Number(data.multiple_response_sets().len() as f64),
            ),
        ];
        let rows = rows
            .drain(..)
            .map(|(name, value)| vec![Cell::Text(name.into()), value])
            .collect();
        let mut tables = vec![Table {
            title: "File information".into(),
            row_headers: 1,
            columns: vec![String::new(), "Value".into()],
            rows,
            ..Table::default()
        }];
        let documents: Vec<Vec<Cell>> = data
            .documents()
            .iter()
            .map(|line| vec![Cell::Text(line.clone())])
            .collect();
        let mut other = Engine::with_dataset(Some(data));
        tables.extend(other.display_command(&Display {
            what: "DICTIONARY".into(),
            sorted: false,
            variables: None,
        })?);
        if !documents.is_empty() {
            tables.push(Table {
                title: "Documents".into(),
                columns: vec!["Document".into()],
                rows: documents,
                ..Table::default()
            });
        }
        Ok(tables)
    }
}
