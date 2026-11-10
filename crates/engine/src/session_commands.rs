//! Commands about the session rather than the data: macros and inserted files (already
//! handled before parsing), program blocks, settings, named datasets, the working folder,
//! and the DISPLAY and LIST listings.
//!
//! Choices where the manual is silent:
//! - Words after PRESERVE or RESTORE (`PRESERVE LOCALE.`, as survey tools write) are ignored
//!   with a warning; RESTORE without PRESERVE does nothing, with a warning.
//! - DATASET ACTIVATE warns when the dataset it replaces has no name, since that one is closed.
//! - SET FORMAT applies to the new numeric variables of the commands that follow, except those
//!   that read a data file.

use crate::{Cell, Engine, Error, Table, ctables::format_value};
use statscraft_data::{Dataset, MissingValues, Value, Values};
use statscraft_syntax::{
    Command, DatasetCommand, Display, List, SETTINGS, SettingEffect, UseRange,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_PRESERVE: usize = 5;

/// What the session commands keep between commands.
#[derive(Default, Clone)]
pub(crate) struct State {
    /// The folder CD named; relative file names start there. None: the current folder.
    pub cwd: Option<PathBuf>,
    /// The SET subcommands given, in upper case, with their values.
    settings: BTreeMap<String, String>,
    /// What PRESERVE stored, innermost last.
    preserved: Vec<(BTreeMap<String, String>, Option<PathBuf>)>,
    /// The named datasets other than the active one; None for a name only declared.
    datasets: Vec<(String, Option<Dataset>)>,
    /// The name of the active dataset.
    active_name: Option<String>,
}

fn table(title: &str, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        rows,
        ..Table::default()
    }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn on_off(name: &str, value: &str) -> Result<bool, Error> {
    match value.to_uppercase().as_str() {
        "ON" | "YES" => Ok(true),
        "OFF" | "NO" => Ok(false),
        _ => Err(Error(format!(
            "SET {name} takes ON or OFF (YES or NO), got {value}"
        ))),
    }
}

/// `Fw.d` as (width, decimals).
fn simple_format(value: &str) -> Option<(u32, u32)> {
    let rest = value.strip_prefix(['F', 'f'])?;
    let (width, decimals) = rest.split_once('.').unwrap_or((rest, "0"));
    let (width, decimals) = (width.parse::<u32>().ok()?, decimals.parse::<u32>().ok()?);
    ((1..=40).contains(&width) && decimals < width.min(17)).then_some((width, decimals))
}

pub(crate) fn missing_text(missing: &MissingValues) -> String {
    let n = |v: f64| format!("{v}");
    let bound = |v: f64| match v {
        v if v == f64::MIN => "LOWEST".to_string(),
        v if v == f64::MAX => "HIGHEST".to_string(),
        v => n(v),
    };
    match missing {
        MissingValues::None => String::new(),
        MissingValues::Numbers(values) => {
            values.iter().map(|&v| n(v)).collect::<Vec<_>>().join(", ")
        }
        MissingValues::Range { low, high, extra } => {
            let range = format!("{} THRU {}", bound(*low), bound(*high));
            match extra {
                Some(e) => format!("{range}, {}", n(*e)),
                None => range,
            }
        }
        MissingValues::Strings(values) => values
            .iter()
            .map(|v| format!("'{v}'"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

impl Engine {
    /// A file name as the commands use it: relative names start from the folder of the last CD.
    pub(crate) fn path(&self, file: &str) -> PathBuf {
        match &self.session.cwd {
            Some(dir) if Path::new(file).is_relative() => dir.join(file),
            _ => PathBuf::from(file),
        }
    }

    /// The folder relative names of syntax files start from.
    fn syntax_folder(&self) -> PathBuf {
        self.session
            .cwd
            .clone()
            .or_else(|| self.base_dir.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// The value of a setting: as SET gave it, else its default.
    pub(crate) fn setting(&self, name: &str) -> String {
        self.session.settings.get(name).cloned().unwrap_or_else(|| {
            let default = SETTINGS.iter().find(|s| s.0 == name);
            default.map_or_else(String::new, |s| s.1.to_string())
        })
    }

    /// The format SET FORMAT gives new numeric variables, when it is not the default F8.2.
    pub fn new_variable_format(&self) -> Option<String> {
        let format = self.setting("FORMAT").to_uppercase();
        (format != "F8.2").then_some(format)
    }

    /// Gives the numeric variables a command created, which are not in `before`, the SET FORMAT
    /// format when they still have the default one.
    pub fn format_new_variables(
        &mut self,
        command: &Command,
        before: &[String],
    ) -> Result<(), Error> {
        let reads_file = matches!(
            command,
            Command::Get { .. }
                | Command::AddFiles(_)
                | Command::MatchFiles(_)
                | Command::Dataset(_)
                | Command::Set(_)
        );
        let Some(format) = self.new_variable_format().filter(|_| !reads_file) else {
            return Ok(());
        };
        let Some(data) = self.dataset.as_mut() else {
            return Ok(());
        };
        let new: Vec<String> = data
            .variables()
            .filter(|v| {
                !v.is_string() && v.format() == "F8.2" && !before.iter().any(|b| b == v.name())
            })
            .map(|v| v.name().to_string())
            .collect();
        for name in new {
            data.set_format(&name, &format)?;
        }
        Ok(())
    }

    /// Before another data source becomes active: a named active dataset stays open.
    pub(crate) fn park_active(&mut self) {
        if let (Some(name), Some(data)) = (self.session.active_name.take(), self.dataset.take()) {
            self.store_dataset(name, Some(data));
        }
    }

    fn store_dataset(&mut self, name: String, data: Option<Dataset>) {
        let datasets = &mut self.session.datasets;
        datasets.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
        datasets.push((name, data));
    }

    fn find_dataset(&self, name: &str) -> Option<usize> {
        self.session
            .datasets
            .iter()
            .position(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    fn is_active_name(&self, name: &str) -> bool {
        self.session
            .active_name
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case(name))
    }

    /// The named datasets other than the active one, and the name of the active one.
    pub fn named_datasets(&self) -> (&[(String, Option<Dataset>)], Option<&str>) {
        (&self.session.datasets, self.session.active_name.as_deref())
    }

    /// Puts back named datasets, as `named_datasets` gave them.
    pub fn set_named_datasets(
        &mut self,
        datasets: Vec<(String, Option<Dataset>)>,
        active: Option<String>,
    ) {
        self.session.datasets = datasets;
        self.session.active_name = active;
    }

    /// The commands of this module; the main `execute` hands them over.
    pub(crate) fn execute_session(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        match command {
            Command::Define(_) | Command::Cache => {}
            Command::Insert { file, include } => {
                // the commands of the file were read in already; here only a file that could
                // not be read is reported
                let path = self.syntax_folder().join(file);
                if let Err(e) = std::fs::metadata(&path) {
                    let name = if *include { "INCLUDE" } else { "INSERT" };
                    return Err(Error(format!("{name}: cannot read {file}: {e}")));
                }
            }
            Command::BeginProgram => self.warn(
                "the program block was not run: StatsCraft runs no programs in other languages",
            ),
            Command::Set(settings) => self.set(settings)?,
            Command::Show(names) => return Ok(vec![self.show(names)]),
            Command::Preserve(words) => {
                self.ignored_words("PRESERVE", words);
                if self.session.preserved.len() == MAX_PRESERVE {
                    return Err(Error(format!(
                        "PRESERVE nests at most {MAX_PRESERVE} levels: RESTORE first"
                    )));
                }
                let state = (self.session.settings.clone(), self.session.cwd.clone());
                self.session.preserved.push(state);
            }
            Command::Restore(words) => {
                self.ignored_words("RESTORE", words);
                match self.session.preserved.pop() {
                    Some((settings, cwd)) => {
                        self.session.settings = settings;
                        self.session.cwd = cwd;
                    }
                    None => self.warn("RESTORE without PRESERVE: there was nothing to restore"),
                }
            }
            Command::Cd(folder) => self.cd(folder)?,
            Command::Dataset(command) => return self.dataset_command(command),
            Command::Use(range) => self.use_cases(range.as_ref())?,
            Command::Display(display) => return self.display_command(display),
            Command::List(_) => return self.procedure(command),
            _ => return Err(Error("this command cannot run in the engine yet".into())),
        }
        Ok(Vec::new())
    }

    fn ignored_words(&mut self, command: &str, words: &[String]) {
        if !words.is_empty() {
            self.warn(format!(
                "{command} takes no specifications; {} ignored",
                words.join(" ")
            ));
        }
    }

    fn set(&mut self, settings: &[(String, String)]) -> Result<(), Error> {
        for (name, value) in settings {
            let effect = SETTINGS.iter().find(|s| s.0 == name).map(|s| s.2);
            match name.as_str() {
                "FORMAT" => {
                    let Some((width, decimals)) = simple_format(value) else {
                        return Err(Error(format!(
                            "SET FORMAT takes a simple F format such as F8.2, got {value}"
                        )));
                    };
                    let value = format!("F{width}.{decimals}");
                    self.session.settings.insert(name.clone(), value);
                    continue;
                }
                "DECIMAL" => {
                    if !["DOT", "COMMA"].contains(&value.to_uppercase().as_str()) {
                        return Err(Error(format!(
                            "SET DECIMAL takes DOT or COMMA, got {value}"
                        )));
                    }
                }
                "MPRINT" | "MEXPAND" => {
                    on_off(name, value)?;
                }
                "UNICODE" => {
                    if !on_off(name, value)? {
                        self.warn(
                            "SET UNICODE=OFF has no effect: StatsCraft always works in Unicode",
                        );
                    }
                }
                "MNEST" | "MITERATE" => {
                    if value.parse::<u32>().map_or(true, |n| n == 0) {
                        return Err(Error(format!(
                            "SET {name} takes a whole number from 1, got {value}"
                        )));
                    }
                }
                _ => {}
            }
            match effect {
                Some(SettingEffect::Appearance) => self.warn(format!(
                    "SET {name} only changes how output looks; StatsCraft ignores it"
                )),
                Some(SettingEffect::NotYet) => self.warn(format!(
                    "SET {name} is kept, but results do not follow it yet"
                )),
                _ => {}
            }
            let value = match effect {
                Some(SettingEffect::Used) if name != "LOCALE" => value.to_uppercase(),
                _ => value.clone(),
            };
            self.session.settings.insert(name.clone(), value);
        }
        Ok(())
    }

    fn show(&self, names: &[String]) -> Table {
        const MAIN: [&str; 12] = [
            "BLANKS",
            "DECIMAL",
            "DIRECTORY",
            "FORMAT",
            "LOCALE",
            "MEXPAND",
            "MPRINT",
            "N",
            "UNICODE",
            "VERSION",
            "WEIGHT",
            "FILTER",
        ];
        let all: Vec<String>;
        let names: &[String] = if names.is_empty() || names.iter().any(|n| n == "ALL") {
            all = MAIN.iter().map(|s| s.to_string()).collect();
            &all
        } else {
            names
        };
        let data = self.dataset.as_ref();
        let rows = names
            .iter()
            .map(|name| {
                let value = match name.as_str() {
                    "N" => data.map_or_else(|| "UNKNOWN".into(), |d| d.case_count().to_string()),
                    "WEIGHT" => data
                        .and_then(Dataset::weight_variable)
                        .map_or_else(|| "OFF".into(), |w| format!("Weighted by {w}")),
                    "FILTER" => data
                        .and_then(Dataset::filter_variable)
                        .map_or_else(|| "OFF".into(), |f| format!("Filtered by {f}")),
                    "SPLIT" => data.and_then(Dataset::split_file).map_or_else(
                        || "OFF".into(),
                        |s| format!("Split by {}", s.variables.join(" ")),
                    ),
                    "DIRECTORY" => self.syntax_folder().display().to_string(),
                    "SYSMIS" => "System-missing".into(),
                    "VERSION" => format!("StatsCraft {}", env!("CARGO_PKG_VERSION")),
                    "CC" => ["CCA", "CCB", "CCC", "CCD", "CCE"]
                        .iter()
                        .map(|c| format!("{c} {}", self.setting(c)))
                        .collect::<Vec<_>>()
                        .join("; "),
                    other => self.setting(other),
                };
                vec![text(name.clone()), text(value)]
            })
            .collect();
        let mut table = table("Settings", &["Setting", "Value"], rows);
        table.row_headers = 1;
        table
    }

    fn cd(&mut self, folder: &str) -> Result<(), Error> {
        let path = self.syntax_folder().join(folder);
        // the last part may name a file, which is left out
        let dir = if path.is_dir() {
            path
        } else {
            match path.parent() {
                Some(parent) if parent.is_dir() => parent.to_path_buf(),
                _ => return Err(Error(format!("CD: no folder {folder}"))),
            }
        };
        self.session.cwd = Some(dir);
        Ok(())
    }

    fn dataset_command(&mut self, command: &DatasetCommand) -> Result<Vec<Table>, Error> {
        match command {
            DatasetCommand::Name(name) => {
                self.active()?;
                if let Some(i) = self.find_dataset(name) {
                    self.session.datasets.remove(i);
                }
                self.session.active_name = Some(name.clone());
            }
            DatasetCommand::Activate(name) => {
                if self.is_active_name(name) {
                    return Ok(Vec::new());
                }
                let Some(i) = self.find_dataset(name) else {
                    return Err(Error(format!("DATASET ACTIVATE: no dataset named {name}")));
                };
                if self.session.datasets[i].1.is_none() {
                    return Err(Error(format!(
                        "DATASET ACTIVATE: dataset {name} was declared but holds no data yet"
                    )));
                }
                self.begin_pass()?;
                if self.session.active_name.is_none() && self.dataset.is_some() {
                    self.warn("the active dataset had no name and is closed");
                }
                self.park_active();
                let (name, data) = self.session.datasets.remove(i);
                self.dataset = data;
                self.session.active_name = Some(name);
                self.scratch_assigned.clear();
                self.temporary = None;
                self.case_limit = None;
            }
            DatasetCommand::Copy(name) => {
                self.begin_pass()?;
                let copy = self.active()?.clone();
                if self.is_active_name(name) {
                    self.session.active_name = None;
                } else if self
                    .find_dataset(name)
                    .is_some_and(|i| self.session.datasets[i].1.is_some())
                {
                    self.warn(format!("dataset {name} is replaced by the copy"));
                }
                self.store_dataset(name.clone(), Some(copy));
            }
            DatasetCommand::Declare(name) => {
                if self.find_dataset(name).is_none() && !self.is_active_name(name) {
                    self.session.datasets.push((name.clone(), None));
                }
            }
            DatasetCommand::Close(None) => self.session.active_name = None,
            DatasetCommand::Close(Some(name)) => {
                if self.is_active_name(name) {
                    self.session.active_name = None;
                } else if let Some(i) = self.find_dataset(name) {
                    self.session.datasets.remove(i);
                } else {
                    return Err(Error(format!("DATASET CLOSE: no dataset named {name}")));
                }
            }
            DatasetCommand::CloseAll => {
                self.session.datasets.clear();
                self.session.active_name = None;
            }
            DatasetCommand::Display => {
                let size = |d: Option<&Dataset>| match d {
                    Some(d) => vec![
                        Cell::Number(d.case_count() as f64),
                        Cell::Number(d.variable_count() as f64),
                    ],
                    None => vec![Cell::Empty, Cell::Empty],
                };
                let mut rows = Vec::new();
                if let Some(data) = &self.dataset {
                    let name = self
                        .session
                        .active_name
                        .clone()
                        .unwrap_or_else(|| "(unnamed)".into());
                    let mut row = vec![text(name)];
                    row.extend(size(Some(data)));
                    row.push(text("yes"));
                    rows.push(row);
                }
                for (name, data) in &self.session.datasets {
                    let mut row = vec![text(name.clone())];
                    row.extend(size(data.as_ref()));
                    row.push(text(if data.is_some() { "" } else { "declared" }));
                    rows.push(row);
                }
                let mut table = table(
                    "Datasets",
                    &["Dataset", "Cases", "Variables", "Active"],
                    rows,
                );
                table.row_headers = 1;
                return Ok(vec![table]);
            }
        }
        Ok(Vec::new())
    }

    fn use_cases(&mut self, range: Option<&UseRange>) -> Result<(), Error> {
        let Some(range) = range else {
            // USE and FILTER exclude each other: USE ALL turns the filter off
            if let Some(data) = self
                .dataset
                .as_mut()
                .filter(|d| d.filter_variable().is_some())
            {
                data.set_filter_variable(None)?;
            }
            return Ok(());
        };
        if !range.permanent {
            return Err(Error(
                "USE with a range of cases is supported only with /PERMANENT for now".into(),
            ));
        }
        self.begin_pass()?;
        let data = self.active_mut()?;
        let n = data.case_count();
        let first = range.first.unwrap_or(1) as usize;
        let last = range.last.map_or(n, |l| (l as usize).min(n));
        let keep: Vec<bool> = (1..=n).map(|c| c >= first && c <= last).collect();
        data.select_cases(&keep)?;
        Ok(())
    }

    fn display_command(&mut self, display: &Display) -> Result<Vec<Table>, Error> {
        let what = display.what.as_str();
        if what == "MACROS" {
            self.warn("DISPLAY MACROS lists nothing: macros are expanded before the commands run");
            return Ok(Vec::new());
        }
        if what == "VECTOR" {
            let mut vectors: Vec<(&String, &Vec<String>)> = self.vectors.iter().collect();
            vectors.sort();
            let rows = vectors
                .into_iter()
                .map(|(name, vars)| vec![text(name.clone()), text(vars.join(" "))])
                .collect();
            return Ok(vec![table("Vectors", &["Vector", "Variables"], rows)]);
        }
        let data = self.active()?;
        if what == "DOCUMENTS" {
            let rows = data
                .documents()
                .iter()
                .map(|l| vec![text(l.clone())])
                .collect();
            return Ok(vec![table("Documents", &["Document"], rows)]);
        }
        if what == "FILE LABEL" {
            let label = data.file_label().unwrap_or_default();
            return Ok(vec![table(
                "File label",
                &["Label"],
                vec![vec![text(label)]],
            )]);
        }
        let mut positions: Vec<usize> = match &display.variables {
            Some(items) => {
                let names = self.expand(items)?;
                let mut positions: Vec<usize> = names
                    .iter()
                    .filter_map(|n| data.variable_index(n))
                    .collect();
                positions.sort_unstable();
                positions.dedup();
                positions
            }
            None => (0..data.variable_count()).collect(),
        };
        if what == "SCRATCH" {
            positions.retain(|&i| data.variable_at(i).name().starts_with('#'));
        }
        if display.sorted {
            positions.sort_by_key(|&i| data.variable_at(i).name().to_lowercase());
        }
        let vars = positions.iter().map(|&i| (i + 1, data.variable_at(i)));
        let mut tables = Vec::new();
        match what {
            "NAMES" | "SCRATCH" => {
                let rows = vars.map(|(_, v)| vec![text(v.name())]).collect();
                tables.push(table("Variables", &["Name"], rows));
            }
            "INDEX" => {
                let rows = vars
                    .map(|(p, v)| vec![text(v.name()), Cell::Number(p as f64)])
                    .collect();
                tables.push(table("Variables", &["Name", "Position"], rows));
            }
            "LABELS" => {
                let rows = vars
                    .map(|(p, v)| {
                        vec![
                            text(v.name()),
                            Cell::Number(p as f64),
                            text(v.label().unwrap_or_default()),
                        ]
                    })
                    .collect();
                tables.push(table(
                    "Variable labels",
                    &["Name", "Position", "Label"],
                    rows,
                ));
            }
            "VARIABLES" => {
                let rows = vars
                    .map(|(p, v)| {
                        vec![
                            text(v.name()),
                            Cell::Number(p as f64),
                            text(v.format()),
                            text(v.format()),
                            text(missing_text(v.missing_values())),
                        ]
                    })
                    .collect();
                let columns = [
                    "Name",
                    "Position",
                    "Print Format",
                    "Write Format",
                    "Missing Values",
                ];
                tables.push(table("Variables", &columns, rows));
            }
            "DICTIONARY" => {
                let mut labels = Vec::new();
                let rows = vars
                    .map(|(p, v)| {
                        for (value, label) in v.value_labels() {
                            let value = match value {
                                Value::Number(n) => Cell::Number(*n),
                                Value::String(s) => text(s.clone()),
                            };
                            labels.push(vec![text(v.name()), value, text(label.clone())]);
                        }
                        vec![
                            text(v.name()),
                            Cell::Number(p as f64),
                            text(v.label().unwrap_or_default()),
                            text(v.measure().as_str()),
                            text(v.role().unwrap_or("input")),
                            v.display_width()
                                .map_or(Cell::Empty, |w| Cell::Number(w as f64)),
                            text(v.alignment().unwrap_or(if v.is_string() {
                                "left"
                            } else {
                                "right"
                            })),
                            text(v.format()),
                            text(v.format()),
                            text(missing_text(v.missing_values())),
                        ]
                    })
                    .collect();
                let columns = [
                    "Name",
                    "Position",
                    "Label",
                    "Measurement Level",
                    "Role",
                    "Column Width",
                    "Alignment",
                    "Print Format",
                    "Write Format",
                    "Missing Values",
                ];
                tables.push(table("Variable information", &columns, rows));
                if !labels.is_empty() {
                    tables.push(table(
                        "Variable values",
                        &["Variable", "Value", "Label"],
                        labels,
                    ));
                }
            }
            _ => {
                // ATTRIBUTES or @ATTRIBUTES
                let all = what.starts_with('@');
                let shown = |name: &str| all || !(name.starts_with('@') || name.starts_with("$@"));
                let mut rows = Vec::new();
                for (name, values) in data.attributes() {
                    for (k, value) in values.iter().enumerate().filter(|_| shown(name)) {
                        let name = if values.len() > 1 {
                            format!("{name}[{}]", k + 1)
                        } else {
                            name.clone()
                        };
                        rows.push(vec![text("(data file)"), text(name), text(value.clone())]);
                    }
                }
                for (_, v) in vars {
                    for (name, values) in v.attributes() {
                        for (k, value) in values.iter().enumerate().filter(|_| shown(name)) {
                            let name = if values.len() > 1 {
                                format!("{name}[{}]", k + 1)
                            } else {
                                name.clone()
                            };
                            rows.push(vec![text(v.name()), text(name), text(value.clone())]);
                        }
                    }
                }
                tables.push(table(
                    "Attributes",
                    &["Variable", "Attribute", "Value"],
                    rows,
                ));
            }
        }
        Ok(tables)
    }

    /// LIST, on the cases a procedure sees.
    pub(crate) fn list(&mut self, list: &List) -> Result<Vec<Table>, Error> {
        let names = match &list.variables {
            Some(items) => self.expand(items)?,
            None => self
                .active()?
                .variable_names()
                .map(str::to_string)
                .collect(),
        };
        let data = self.active()?;
        let variables: Vec<_> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let last = list
            .to
            .map_or(data.case_count(), |t| (t as usize).min(data.case_count()));
        let cases = (list.from as usize..=last).step_by(list.by as usize);
        let mut rows = Vec::new();
        for case in cases {
            let mut row = Vec::new();
            if list.numbered {
                row.push(Cell::Number(case as f64));
            }
            for v in &variables {
                row.push(match v.values() {
                    Values::Numeric(values) => match values.get(case - 1) {
                        Some(x) => text(format_value(x, v.format(), true)),
                        None => text("."),
                    },
                    Values::String(values) => text(values.get(case - 1)),
                });
            }
            rows.push(row);
        }
        let mut columns: Vec<&str> = Vec::new();
        if list.numbered {
            columns.push("Case");
        }
        columns.extend(names.iter().map(String::as_str));
        let mut table = table("List", &columns, rows);
        table.row_headers = usize::from(list.numbered);
        Ok(vec![table])
    }
}
