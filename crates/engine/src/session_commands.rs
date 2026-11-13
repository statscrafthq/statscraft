//! Commands about the session rather than the data: macros and inserted files (already
//! handled before parsing), program blocks, settings, named datasets, the working folder,
//! and the DISPLAY and LIST listings.
//!
//! Choices where the manual is silent:
//! - A SET subcommand name after PRESERVE or RESTORE (`PRESERVE LOCALE.`, as survey tools write)
//!   is accepted: the command stores or brings back every setting, LOCALE among them. RESTORE
//!   warns when it brings back settings besides the named ones (and DECIMAL, which a locale
//!   sets) that changed after PRESERVE. Other words are ignored with a warning. RESTORE without
//!   PRESERVE does nothing, with a warning.
//! - SET LOCALE sets DECIMAL to the locale's decimal mark; OSLOCALE gives DOT, the default.
//! - DATASET ACTIVATE warns when the dataset it replaces has no name, since that one is closed.
//! - SET FORMAT applies to the new numeric variables of the commands that follow, except those
//!   that read a data file.

use crate::format::{Currencies, Currency};
use crate::{Cell, Engine, Error, Table, format_value};
use statscraft_data::{Dataset, MissingValues, Value, Values};
use statscraft_syntax::{
    Command, DatasetCommand, Display, List, MacroState, SETTINGS, SettingEffect, UseRange,
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
    /// The highest n of a DataSetn name given in this session, open or closed since.
    numbered: usize,
    /// The macros, and the macro settings, of the runs before this one. The preprocessor reads
    /// and updates them; the commands only list them.
    pub macros: MacroState,
    /// The macros defined by the DEFINE commands of this run so far.
    defined: Vec<String>,
}

impl State {
    /// Only the settings, for the engines that run one case of a case-by-case pass.
    pub(crate) fn settings_only(&self) -> State {
        State {
            settings: self.settings.clone(),
            ..State::default()
        }
    }
}

/// The n of a DataSetn name, in any case.
fn dataset_number(name: &str) -> Option<usize> {
    let prefix = name.get(..7)?;
    let digits = &name[7..];
    let numeric = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    (prefix.eq_ignore_ascii_case("DataSet") && numeric)
        .then(|| digits.parse().ok())
        .flatten()
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
    /// A file name as the commands use it, a relative one resolved by `relative_base`. A file
    /// handle in it stands for its path.
    pub(crate) fn path(&self, file: &str) -> PathBuf {
        let (file, _) = self.session.macros.resolve_file(file);
        match self.relative_base(false) {
            Some(dir) if Path::new(&file).is_relative() => dir.join(file),
            _ => PathBuf::from(file),
        }
    }

    /// Where every relative name starts, the first found: the folder of the last CD; for a
    /// syntax file (INSERT, INCLUDE, CD) the folder of the syntax file that runs, which syntax
    /// sent by a caller does not have; the folder of the window's caller; else None, the
    /// process's folder.
    pub(crate) fn relative_base(&self, syntax: bool) -> Option<&Path> {
        let file = self.base_dir.as_deref().filter(|_| syntax);
        self.session
            .cwd
            .as_deref()
            .or(file)
            .or(self.work_dir.as_deref())
    }

    /// The encoding FILE HANDLE gave the handle that `file` uses, if any.
    pub(crate) fn handle_encoding(&self, file: &str) -> Option<String> {
        self.session.macros.resolve_file(file).1.map(str::to_string)
    }

    /// `relative_base` for syntax files, with "." for the process's folder.
    pub(crate) fn syntax_folder(&self) -> PathBuf {
        self.relative_base(true)
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    /// The value of a setting: as SET gave it, else its default.
    pub(crate) fn setting(&self, name: &str) -> String {
        self.session.settings.get(name).cloned().unwrap_or_else(|| {
            let default = SETTINGS.iter().find(|s| s.0 == name);
            default.map_or_else(String::new, |s| s.1.to_string())
        })
    }

    /// SET MXLOOPS: the iterations of a loop without an index, and of every MATRIX loop.
    pub(crate) fn mxloops(&self) -> usize {
        self.setting("MXLOOPS")
            .parse::<usize>()
            .map_or(40, |n| n.max(1))
    }

    /// SET BLANKS and EPOCH, as reading text data and dates follows them.
    pub(crate) fn reading(&self) -> crate::data_list::Reading {
        crate::data_list::Reading {
            blanks: self.setting("BLANKS").parse().ok(),
            epoch: self.setting("EPOCH").parse().ok(),
        }
    }

    /// SET FUZZBITS: the bits RND and TRUNC allow when their third argument is left out.
    pub(crate) fn fuzzbits(&self) -> f64 {
        self.setting("FUZZBITS").parse().unwrap_or(6.0)
    }

    /// SET SCALEMIN: the different valid values that make a new numeric variable scale.
    pub(crate) fn scale_min(&self) -> usize {
        self.setting("SCALEMIN").parse().unwrap_or(24)
    }

    /// The SET CCA to CCE specifications, to show custom currency values.
    pub fn currencies(&self) -> Currencies {
        Currencies(["CCA", "CCB", "CCC", "CCD", "CCE"].map(|cc| self.setting(cc)))
    }

    /// A display format with what `format_value` needs beyond its name: a custom currency
    /// format (CCA to CCE) gets the SET specification, when it is not the default, after a space.
    pub(crate) fn currency_format(&self, format: &str) -> String {
        self.currencies().resolve(format)
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
                | Command::StarJoin(_)
                | Command::Update(_)
                | Command::Flip(_)
                | Command::VarsToCases(_)
                | Command::CasesToVars(_)
                | Command::Dataset(_)
                | Command::Set(_)
                | Command::GetData(_)
                | Command::Import(_)
                | Command::GetTranslate(_)
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

    /// Notes a DataSetn name, so that the next automatic name comes after it.
    fn note_name(&mut self, name: &str) {
        if let Some(n) = dataset_number(name) {
            self.session.numbered = self.session.numbered.max(n);
        }
    }

    /// The name a data file opened from the window gets: DataSet1, DataSet2 and so on, after
    /// every DataSetn name the session has given, so a closed one is not given again.
    pub fn next_dataset_name(&self) -> String {
        let s = &self.session;
        let names = s.datasets.iter().map(|(n, _)| n.as_str());
        let names = names.chain(s.active_name.as_deref());
        let last = names
            .filter_map(dataset_number)
            .fold(s.numbered, usize::max);
        format!("DataSet{}", last + 1)
    }

    fn find_dataset(&self, name: &str) -> Option<usize> {
        self.session
            .datasets
            .iter()
            .position(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    pub(crate) fn is_active_name(&self, name: &str) -> bool {
        self.session
            .active_name
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case(name))
    }

    /// Whether `name` is the name of a dataset, open or only declared.
    pub(crate) fn is_dataset_name(&self, name: &str) -> bool {
        self.is_active_name(name) || self.find_dataset(name).is_some()
    }

    /// An open dataset by name: the active one or another named one with data.
    pub(crate) fn named_dataset(&self, name: &str) -> Option<&Dataset> {
        if self.is_active_name(name) {
            return self.dataset.as_ref();
        }
        self.find_dataset(name)
            .and_then(|i| self.session.datasets[i].1.as_ref())
    }

    /// The data a file command names: an open dataset of that name, which comes before a file
    /// of that name, else the data file.
    pub(crate) fn data_file(&self, file: &str) -> Result<Dataset, Error> {
        match self.named_dataset(file) {
            Some(data) => Ok(data.clone()),
            None => Ok(Dataset::read_sav(self.path(file))?),
        }
    }

    /// Writes `data` where an OUTFILE names: into the dataset of that name when DATASET NAME or
    /// DECLARE defined one, else to the data file.
    pub(crate) fn write_data_file(&mut self, file: &str, data: Dataset) -> Result<(), Error> {
        if self.is_active_name(file) {
            return Err(Error(format!(
                "{file} is the active dataset; write to it with OUTFILE=*"
            )));
        }
        match self.find_dataset(file) {
            Some(i) => self.session.datasets[i].1 = Some(data),
            None if !self.dry_run => data.write_sav(self.path(file))?,
            None => {}
        }
        Ok(())
    }

    /// The named datasets other than the active one, and the name of the active one.
    pub fn named_datasets(&self) -> (&[(String, Option<Dataset>)], Option<&str>) {
        (&self.session.datasets, self.session.active_name.as_deref())
    }

    /// The macros and macro settings kept between runs.
    pub fn macros(&self) -> &MacroState {
        &self.session.macros
    }

    /// Keeps `macros` for the next run; the DEFINE commands of the run that made them are done.
    pub fn set_macros(&mut self, macros: MacroState) {
        self.session.macros = macros;
        self.session.defined.clear();
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

    /// The file handles, SET values, PRESERVE levels and CD folder as syntax that
    /// `restore_settings` reads back: the handles, then the outermost level first, each later
    /// level after a PRESERVE with what it changed.
    pub fn settings_syntax(&self) -> String {
        let s = &self.session;
        let current = (&s.settings, &s.cwd);
        let levels = s.preserved.iter().map(|(set, cwd)| (set, cwd));
        let (mut settings_before, mut cwd_before) = (&BTreeMap::new(), &None);
        let mut out = String::new();
        let quoted = |text: &str| format!("'{}'", text.replace('\'', "''"));
        for (name, handle) in s.macros.handles() {
            out.push_str(&format!(
                "FILE HANDLE {name} /NAME={}",
                quoted(&handle.path)
            ));
            if let Some(encoding) = &handle.encoding {
                out.push_str(&format!(" /ENCODING={}", quoted(encoding)));
            }
            out.push_str(".\n");
        }
        for (i, (settings, cwd)) in levels.chain([current]).enumerate() {
            if i > 0 {
                out.push_str("PRESERVE.\n");
            }
            // LOCALE sets DECIMAL, so it comes first and DECIMAL is written after it
            let locale = settings_before.get("LOCALE") != settings.get("LOCALE");
            let mut changed: Vec<String> = settings
                .iter()
                .filter(|(name, value)| {
                    settings_before.get(*name) != Some(*value) || (locale && *name == "DECIMAL")
                })
                .map(|(name, value)| match value.starts_with('[') {
                    true => format!("{name}={value}"),
                    false => format!("{name}='{}'", value.replace('\'', "''")),
                })
                .collect();
            changed.sort_by_key(|set| !set.starts_with("LOCALE="));
            if !changed.is_empty() {
                out.push_str(&format!("SET {}.\n", changed.join(" ")));
            }
            if let (Some(dir), true) = (cwd, cwd != cwd_before) {
                let dir = dir.display().to_string().replace('\'', "''");
                out.push_str(&format!("CD '{dir}'.\n"));
            }
            (settings_before, cwd_before) = (settings, cwd);
        }
        out
    }

    /// Puts back what `settings_syntax` wrote.
    pub fn restore_settings(&mut self, text: &str) -> Result<(), Error> {
        let (parsed, errors) = statscraft_syntax::parse(text);
        if let Some(e) = errors.first() {
            return Err(Error(format!("line {}: {}", e.span.line, e.message)));
        }
        for p in parsed {
            match &p.command {
                Command::Set(_) | Command::Preserve(_) => {
                    self.execute_session(&p.command)?;
                }
                // as written: the folder was checked when CD ran
                Command::Cd(folder) => self.session.cwd = Some(PathBuf::from(folder)),
                // the path was completed when FILE HANDLE ran
                Command::FileHandle {
                    name,
                    path,
                    encoding,
                    ..
                } => {
                    let handle = statscraft_syntax::FileHandle {
                        path: path.clone(),
                        encoding: encoding.clone(),
                    };
                    self.session.macros.set_handle(name, handle);
                }
                _ => {
                    return Err(Error(format!(
                        "line {}: only FILE HANDLE, SET, PRESERVE and CD belong here",
                        p.first_line
                    )));
                }
            }
        }
        self.warnings.clear();
        Ok(())
    }

    /// What `describe` shows of the session besides the active dataset.
    pub(crate) fn session_state(&self) -> crate::session::SessionState {
        let s = &self.session;
        let entry = |(name, data): (&String, Option<&Dataset>)| crate::session::NamedDataset {
            name: name.clone(),
            cases: data.map(Dataset::case_count),
            variables: data.map(Dataset::variable_count),
        };
        let active = s
            .active_name
            .iter()
            .map(|name| (name, self.dataset.as_ref()));
        let others = s.datasets.iter().map(|(name, data)| (name, data.as_ref()));
        crate::session::SessionState {
            settings: s.settings.clone(),
            preserved: s.preserved.len(),
            // where relative file names start: the CD folder, else the caller's
            directory: self.relative_base(false).map(|d| d.display().to_string()),
            active_dataset: s.active_name.clone(),
            datasets: active.chain(others).map(entry).collect(),
            macros: s.macros.names(),
            output_documents: self.output_documents(),
            designated_output: self.designated_output().map(str::to_string),
        }
    }

    /// The commands of this module; the main `execute` hands them over.
    pub(crate) fn execute_session(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        match command {
            Command::Define(name) => self.session.defined.push(name.to_uppercase()),
            Command::Cache => {}
            Command::Insert { file, include } => {
                // the commands of the file were read in already; here only a file that could
                // not be read is reported
                let path = self
                    .syntax_folder()
                    .join(self.session.macros.resolve_file(file).0);
                if let Err(e) = std::fs::metadata(&path) {
                    let name = if *include { "INCLUDE" } else { "INSERT" };
                    return Err(Error(format!("{name}: cannot read {file}: {e}")));
                }
            }
            Command::BeginProgram => self.warn(
                "the program block was not run: Statscraft runs no Python or R code, so what the \
                 block would make (files, variables, commands) is missing",
            ),
            Command::Set(settings) => self.set(settings)?,
            Command::Show(names) => return Ok(vec![self.show(names)]),
            Command::Preserve(words) => {
                self.ignored_words("PRESERVE", "stores", words);
                if self.session.preserved.len() == MAX_PRESERVE {
                    return Err(Error(format!(
                        "PRESERVE nests at most {MAX_PRESERVE} levels: RESTORE first"
                    )));
                }
                let state = (self.session.settings.clone(), self.session.cwd.clone());
                self.session.preserved.push(state);
            }
            Command::Restore(words) => {
                self.ignored_words("RESTORE", "brings back", words);
                match self.session.preserved.pop() {
                    Some((settings, cwd)) => {
                        self.restore_also(words, &settings, &cwd);
                        self.session.settings = settings;
                        self.session.cwd = cwd;
                        self.random.get_mut().mt_active = self.setting("RNG") == "MT";
                    }
                    None => self.warn("RESTORE without PRESERVE: there was nothing to restore"),
                }
            }
            Command::Cd(folder) => self.cd(folder)?,
            Command::FileHandle {
                name,
                path,
                encoding,
                ignored,
            } => {
                if !ignored.is_empty() {
                    self.warn(format!(
                        "{} ignored: Statscraft reads text files line by line, whatever their \
                         record format and length",
                        ignored.join(" and ")
                    ));
                }
                // a relative name starts from the working folder of this moment
                let (resolved, _) = self.session.macros.resolve_file(path);
                let path = match self.relative_base(false) {
                    Some(dir) if Path::new(&resolved).is_relative() => {
                        dir.join(&resolved).display().to_string()
                    }
                    _ => resolved,
                };
                let handle = statscraft_syntax::FileHandle {
                    path,
                    encoding: encoding.clone(),
                };
                self.session.macros.set_handle(name, handle);
            }
            Command::Dataset(command) => return self.dataset_command(command),
            Command::Use(range) => self.use_cases(range.as_ref())?,
            Command::Display(display) => return self.display_command(display),
            Command::List(_) => return self.procedure(command),
            Command::GetData(get) => self.get_data(get)?,
            Command::AddValueLabels(_)
            | Command::FileLabel(_)
            | Command::AddDocument(_)
            | Command::DropDocuments
            | Command::VariableRole(_)
            | Command::VariableAlignment(_)
            | Command::Attribute(_)
            | Command::AlterType(_)
            | Command::ApplyDictionary(_) => self.execute_dictionary(command)?,
            _ => return self.execute_output(command),
        }
        Ok(Vec::new())
    }

    /// The words after PRESERVE or RESTORE, which `does` every setting anyway. A SET subcommand
    /// name (`PRESERVE LOCALE`, as survey tools write) is among every setting, so only other
    /// words get a warning.
    fn ignored_words(&mut self, command: &str, does: &str, words: &[String]) {
        let other: Vec<&str> = words
            .iter()
            .filter(|w| !SETTINGS.iter().any(|s| s.0 == w.as_str()))
            .map(String::as_str)
            .collect();
        if !other.is_empty() {
            self.warn(format!(
                "{command} takes no specifications and {does} every setting; {} ignored",
                other.join(" ")
            ));
        }
    }

    /// Warns when `RESTORE LOCALE` brings back more than the settings it names: those that
    /// changed after PRESERVE. DECIMAL goes with LOCALE, since a locale sets it.
    fn restore_also(
        &mut self,
        words: &[String],
        stored: &BTreeMap<String, String>,
        cwd: &Option<PathBuf>,
    ) {
        let named = |name: &str| {
            words.iter().any(|w| w == name)
                || (name == "DECIMAL" && words.iter().any(|w| w == "LOCALE"))
        };
        if !words
            .iter()
            .any(|w| SETTINGS.iter().any(|s| s.0 == w.as_str()))
        {
            return;
        }
        let current = &self.session.settings;
        let value = |settings: &BTreeMap<String, String>, name: &str| {
            let default = SETTINGS.iter().find(|s| s.0 == name).map(|s| s.1);
            settings.get(name).cloned().or(default.map(String::from))
        };
        let mut also: Vec<String> = current
            .keys()
            .chain(stored.keys())
            .filter(|name| !named(name) && value(current, name) != value(stored, name))
            .cloned()
            .collect();
        also.sort();
        also.dedup();
        if *cwd != self.session.cwd {
            also.push("the CD folder".into());
        }
        if !also.is_empty() {
            self.warn(format!(
                "every setting comes back, not only {}: also {}, changed after PRESERVE",
                words.join(" "),
                also.join(", ")
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
                "TVARS" | "OVARS" | "TNUMBERS" | "ONUMBERS" => {
                    let choices = match name.ends_with("VARS") {
                        true => crate::shown::VARIABLE_CHOICES,
                        false => crate::shown::VALUE_CHOICES,
                    };
                    if !choices.contains(&value.to_uppercase().as_str()) {
                        return Err(Error(format!(
                            "SET {name} takes {}, got {value}",
                            choices.join(", ")
                        )));
                    }
                }
                "CCA" | "CCB" | "CCC" | "CCD" | "CCE" => {
                    Currency::parse(value).map_err(|e| Error(format!("SET {name}: {e}")))?;
                    self.session.settings.insert(name.clone(), value.clone());
                    continue;
                }
                "UNICODE" => {
                    if !on_off(name, value)? {
                        self.warn(
                            "SET UNICODE=OFF has no effect: Statscraft always works in Unicode",
                        );
                        continue;
                    }
                }
                "LOCALE" => {
                    // a locale sets the decimal mark, as a later SET DECIMAL may change again;
                    // Statscraft starts with DOT whatever the system, so OSLOCALE brings DOT back
                    let os = value.eq_ignore_ascii_case("OSLOCALE");
                    match if os {
                        Some("DOT")
                    } else {
                        crate::locale::decimal(value)
                    } {
                        Some(mark) => {
                            self.session.settings.insert("DECIMAL".into(), mark.into());
                        }
                        None => self.warn(format!(
                            "SET LOCALE: the decimal mark of locale {value} is not known, so \
                             DECIMAL stays {}; give SET DECIMAL to be sure",
                            self.setting("DECIMAL")
                        )),
                    }
                }
                "SEED" => {
                    let seed = match value.to_uppercase().as_str() {
                        "RANDOM" => crate::random::time_seed(),
                        _ => match value.parse::<f64>() {
                            Ok(n) if (1.0..2e9).contains(&n) => n.trunc() as u64,
                            _ => {
                                return Err(Error(format!(
                                    "SET SEED takes a whole number from 1 to 1999999999 or \
                                     RANDOM, got {value}"
                                )));
                            }
                        },
                    };
                    self.random.get_mut().seed = seed;
                    // RANDOM is kept as the seed it chose, so a session can go on from it
                    self.session.settings.insert(name.clone(), seed.to_string());
                    continue;
                }
                "MTINDEX" => {
                    let index = match value.to_uppercase().as_str() {
                        "RANDOM" => crate::random::time_seed() as f64,
                        _ => value.parse::<f64>().map_err(|_| {
                            Error(format!("SET MTINDEX takes a number or RANDOM, got {value}"))
                        })?,
                    };
                    self.random.get_mut().set_mtindex(index);
                    self.session
                        .settings
                        .insert(name.clone(), index.to_string());
                    continue;
                }
                "RNG" => {
                    let mt = match value.to_uppercase().as_str() {
                        "MC" => false,
                        "MT" => true,
                        _ => return Err(Error(format!("SET RNG takes MC or MT, got {value}"))),
                    };
                    self.random.get_mut().mt_active = mt;
                }
                "BLANKS"
                    if !value.eq_ignore_ascii_case("SYSMIS") && value.parse::<f64>().is_err() =>
                {
                    return Err(Error(format!(
                        "SET BLANKS takes a number or SYSMIS, got {value}"
                    )));
                }
                "EPOCH"
                    if !value.eq_ignore_ascii_case("AUTOMATIC")
                        && !value
                            .parse::<u16>()
                            .is_ok_and(|y| (1582..=9999).contains(&y)) =>
                {
                    return Err(Error(format!(
                        "SET EPOCH takes a year from 1582 to 9999 or AUTOMATIC, got {value}"
                    )));
                }
                "FUZZBITS" if !value.parse::<u8>().is_ok_and(|n| n <= 20) => {
                    return Err(Error(format!(
                        "SET FUZZBITS takes a whole number from 0 to 20, got {value}"
                    )));
                }
                "MNEST" | "MITERATE" | "MXLOOPS" | "SCALEMIN"
                    if value.parse::<u32>().map_or(true, |n| n == 0) =>
                {
                    return Err(Error(format!(
                        "SET {name} takes a whole number from 1, got {value}"
                    )));
                }
                _ => {}
            }
            if effect == Some(SettingEffect::Appearance) {
                self.warn(format!(
                    "SET {name} only changes how output looks; Statscraft ignores it"
                ));
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
        let shown: Vec<(&String, String, &str)> = names
            .iter()
            .map(|name| {
                let none = || "none".to_string();
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
                    "HANDLES" => {
                        let handles = self.session.macros.handles().iter();
                        let all: Vec<String> = handles
                            .map(|(name, h)| format!("{name} = {}", h.path))
                            .collect();
                        if all.is_empty() {
                            "none".into()
                        } else {
                            all.join("; ")
                        }
                    }
                    // the state of the generator, as the description of SHOW SEED says
                    "SEED" => self.random.borrow().seed.to_string(),
                    "VERSION" => format!("Statscraft {}", env!("CARGO_PKG_VERSION")),
                    "CC" => ["CCA", "CCB", "CCC", "CCD", "CCE"]
                        .iter()
                        .map(|c| format!("{c} {}", self.setting(c)))
                        .collect::<Vec<_>>()
                        .join("; "),
                    "ENVIRONMENT" => {
                        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
                    }
                    "LICENSE" => "MIT".into(),
                    "SCOMPRESSION" => "ON".into(),
                    "TMSRECORDING" => "NO".into(),
                    "$VARS" => "$CASENUM = the case number; $SYSMIS = system-missing".into(),
                    "EXTPATHS" | "PLUGINS" | "EXTENSIONS" | "MXMEMORY" => none(),
                    other => self.setting(other),
                };
                let note = match name.as_str() {
                    "LICENSE" => "open source: every part of Statscraft is included",
                    "SCOMPRESSION" => "SAVE writes compressed files, zlib compressed for .zsav",
                    "TMSRECORDING" => "Statscraft has no TMS BEGIN",
                    "$VARS" => "$DATE, $JDATE, $TIME, $LENGTH and $WIDTH are not supported yet",
                    "EXTPATHS" => {
                        "Statscraft has no custom dialogs, extension commands or R packages"
                    }
                    "PLUGINS" => "Statscraft runs no Python or R code",
                    "EXTENSIONS" | "MXMEMORY" => "no longer in use",
                    "LOCALE" => "files are always Unicode; the locale sets DECIMAL",
                    "UNICODE" => "Statscraft always works in Unicode",
                    other => match SETTINGS.iter().find(|s| s.0 == other).map(|s| s.2) {
                        Some(SettingEffect::Silent) => "about speed, memory or messages: no effect",
                        Some(SettingEffect::Appearance) => "output does not follow it",
                        _ => "",
                    },
                };
                (name, value, note)
            })
            .collect();
        // a Note column only when a shown setting has one
        let notes = shown.iter().any(|(_, _, note)| !note.is_empty());
        let rows = shown
            .into_iter()
            .map(|(name, value, note)| {
                let mut row = vec![text(name.clone()), text(value)];
                if notes {
                    row.push(if note.is_empty() {
                        Cell::Empty
                    } else {
                        text(note)
                    });
                }
                row
            })
            .collect();
        let columns: &[&str] = if notes {
            &["Setting", "Value", "Note"]
        } else {
            &["Setting", "Value"]
        };
        let mut table = table("Settings", columns, rows);
        table.row_headers = 1;
        table
    }

    fn cd(&mut self, folder: &str) -> Result<(), Error> {
        let path = self
            .syntax_folder()
            .join(self.session.macros.resolve_file(folder).0);
        // the last part may name a file, which is left out
        let dir = if path.is_dir() {
            path
        } else {
            match path.parent() {
                Some(parent) if parent.is_dir() => parent.to_path_buf(),
                _ => return Err(Error(format!("CD: no folder {folder}"))),
            }
        };
        // kept absolute: a later run from another folder still means this one
        self.session.cwd = Some(std::path::absolute(&dir).unwrap_or(dir));
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
                self.note_name(name);
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
                self.note_name(name);
            }
            DatasetCommand::Declare(name) => {
                if self.find_dataset(name).is_none() && !self.is_active_name(name) {
                    self.session.datasets.push((name.clone(), None));
                }
                self.note_name(name);
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

    pub(crate) fn display_command(&mut self, display: &Display) -> Result<Vec<Table>, Error> {
        let what = display.what.as_str();
        if what == "MACROS" {
            let mut names = self.session.macros.names();
            names.extend(self.session.defined.iter().cloned());
            names.sort();
            names.dedup();
            let rows = names.into_iter().map(|n| vec![text(n)]).collect();
            return Ok(vec![table("Macros", &["Macro"], rows)]);
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
                            text(v.write_format()),
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
                            text(v.write_format()),
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
        let formats: Vec<String> = variables
            .iter()
            .map(|v| self.currency_format(v.format()))
            .collect();
        let mut rows = Vec::new();
        for case in cases {
            let mut row = Vec::new();
            if list.numbered {
                row.push(Cell::Number(case as f64));
            }
            for (v, format) in variables.iter().zip(&formats) {
                row.push(match v.values() {
                    Values::Numeric(values) => match values.get(case - 1) {
                        Some(x) => text(format_value(x, format, true)),
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
