//! Executes parsed commands on a dataset and returns result tables.

mod aggregate;
mod anova;
mod autorecode;
pub mod chart;
mod chart_input;
mod cluster;
mod complex_samples;
mod contrast;
mod correlations;
mod correspondence;
mod crosstabs;
mod ctables;
mod data_list;
mod date;
mod descriptives;
mod dictionary;
mod discriminant;
mod dist;
mod distributions;
mod examine;
pub mod excel;
mod expr;
mod expr_functions;
mod factor;
mod format;
mod frequencies;
mod genlin;
mod get_data;
mod get_translate;
mod glm;
mod graph;
mod html;
mod input;
mod km;
mod life_table;
pub mod live;
mod locale;
mod logistic;
mod loops;
mod matrix;
mod matrix_data;
mod matrix_functions;
mod matrix_lang;
mod means;
mod merge;
mod mixed;
mod mrsets;
mod mult_response;
mod nomreg;
mod nonpar_corr;
mod npar_groups;
mod npar_tests;
pub mod office;
mod oneway;
mod output;
mod output_docs;
pub mod output_file;
pub mod output_modify;
mod partial_corr;
mod pass;
pub mod pdf;
pub mod picture;
pub mod plot;
mod plum;
mod portable;
mod pplot;
mod print;
mod quick_cluster;
mod random;
mod rank;
mod recode;
mod regression;
mod reliability;
mod report;
mod restructure;
mod roc;
mod rows;
mod save_translate;
pub mod session;
mod session_commands;
mod shown;
mod sort;
mod split;
mod star_join;
mod transform;
mod tree;
mod ttest;
mod variables;
pub mod vega_lite;
pub mod workbook;
pub mod xlsx;

pub use chart::Chart;
pub use expr::functions;
pub use format::{Currencies, currency_parts, format_value, is_date_time_or_currency};
pub use get_data::delimited;
pub use output_docs::OutputDocument;
pub use random::RandomState;
use serde::{Deserialize, Serialize};
use statscraft_data::{Dataset, Measure, MissingValues, Value};
use statscraft_syntax as syntax;
use statscraft_syntax::{Bound, Command, VarItem};

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<statscraft_data::Error> for Error {
    fn from(e: statscraft_data::Error) -> Self {
        Self(e.to_string())
    }
}

/// A result table, the same for every output format. The first `row_headers` columns label the
/// rows; the remaining columns hold results. A column header can have several levels: `columns`
/// holds the innermost label and `column_groups` the labels above it.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Table {
    pub title: String,
    pub row_headers: usize,
    pub columns: Vec<String>,
    /// For every column the labels above it, outermost first. Empty for a one-level header.
    pub column_groups: Vec<Vec<String>>,
    pub rows: Vec<Vec<Cell>>,
    /// The display format of every cell, in the shape of `rows`, such as "PCT40.1"; empty
    /// strings where there is none. Empty when the table sets no formats. Numbers in `rows`
    /// stay raw; `format_value` shows them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// The label in the window's outline when OUTPUT MODIFY gave one; else the title shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<String>,
    /// Hidden by OUTPUT MODIFY: kept in the document and its file, left out of the window's
    /// pages and of OUTPUT EXPORT with CONTENTS EXPORT=VISIBLE.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

impl Table {
    /// The full header of every column, levels joined with " / ".
    pub fn column_titles(&self) -> Vec<String> {
        self.columns
            .iter()
            .enumerate()
            .map(|(i, leaf)| {
                let mut path = self.column_groups.get(i).cloned().unwrap_or_default();
                path.push(leaf.clone());
                path.retain(|level| !level.is_empty());
                path.join(" / ")
            })
            .collect()
    }
}

/// One header cell spanning `span` columns from `first`.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderCell {
    pub first: usize,
    pub span: usize,
    pub label: String,
}

/// The header rows of a table, outermost first. Every level covers all columns; neighbouring
/// columns that share a label and everything above it are merged into one cell.
pub fn header_rows(table: &Table) -> Vec<Vec<HeaderCell>> {
    let depth = 1 + table.column_groups.iter().map(Vec::len).max().unwrap_or(0);
    let label = |column: usize, level: usize| -> &str {
        let groups = table
            .column_groups
            .get(column)
            .map_or(&[][..], Vec::as_slice);
        if level == depth - 1 {
            &table.columns[column]
        } else {
            groups.get(level).map_or("", String::as_str)
        }
    };
    let path = |column: usize, level: usize| (0..=level).map(move |l| label(column, l));
    (0..depth)
        .map(|level| {
            let mut cells: Vec<HeaderCell> = Vec::new();
            for column in 0..table.columns.len() {
                match cells.last_mut() {
                    Some(cell)
                        if column >= table.row_headers
                            && cell.first >= table.row_headers
                            && path(cell.first, level).eq(path(column, level)) =>
                    {
                        cell.span += 1
                    }
                    _ => cells.push(HeaderCell {
                        first: column,
                        span: 1,
                        label: label(column, level).to_string(),
                    }),
                }
            }
            cells
        })
        .collect()
}

/// A result cell as text, in its display format. The same rules as the command line output.
pub fn cell_text(table: &Table, row: usize, column: usize) -> String {
    let format = table
        .formats
        .get(row)
        .and_then(|f| f.get(column))
        .filter(|f| !f.is_empty());
    match (&table.rows[row][column], format) {
        (Cell::Text(t), _) => t.clone(),
        (Cell::Number(n), Some(format)) => format_value(*n, format, true),
        (Cell::Number(n), None) if n.fract() == 0.0 => format!("{n:.0}"),
        (Cell::Number(n), None) => format!("{n:.2}"),
        (Cell::Empty, _) => String::new(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Number(f64),
    #[default]
    Empty,
}

/// What a run changed in the dataset, so a caller need not diff dictionaries itself.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Changes {
    pub cases_before: Option<usize>,
    pub cases_after: Option<usize>,
    pub variables_added: Vec<String>,
    pub variables_removed: Vec<String>,
    pub variables_changed: Vec<String>,
}

impl Changes {
    /// A variable counts as changed when its dictionary entry or any of its values differ, in
    /// case order, so COMPUTE and SORT CASES show up even when the summary stays the same.
    pub fn between(before: Option<&Dataset>, after: Option<&Dataset>) -> Self {
        let mut changes = Changes {
            cases_before: before.map(Dataset::case_count),
            cases_after: after.map(Dataset::case_count),
            ..Changes::default()
        };
        // by name, so 10 000 variables cost no 100 million comparisons
        let previous: std::collections::HashMap<&str, _> = before
            .into_iter()
            .flat_map(Dataset::variables)
            .map(|v| (v.name(), v))
            .collect();
        let now: std::collections::HashSet<&str> = after
            .into_iter()
            .flat_map(Dataset::variable_names)
            .collect();
        for variable in after.into_iter().flat_map(Dataset::variables) {
            let name = variable.name();
            match previous.get(name) {
                None => changes.variables_added.push(name.to_string()),
                Some(previous) if *previous != variable => {
                    changes.variables_changed.push(name.to_string())
                }
                Some(_) => {}
            }
        }
        for name in before.into_iter().flat_map(Dataset::variable_names) {
            if !now.contains(name) {
                changes.variables_removed.push(name.to_string());
            }
        }
        changes
    }

    pub fn is_empty(&self) -> bool {
        self.cases_before == self.cases_after
            && self.variables_added.is_empty()
            && self.variables_removed.is_empty()
            && self.variables_changed.is_empty()
    }
}

/// The state commands run against: at most one active dataset.
#[derive(Default, Clone)]
pub struct Engine {
    dataset: Option<Dataset>,
    /// When set, commands that write files are checked but not carried out.
    pub dry_run: bool,
    /// The open DO IF blocks, innermost last. See `Engine::active_cases`.
    blocks: Vec<transform::Block>,
    /// What the last command did differently from what was asked, without failing.
    warnings: Vec<String>,
    /// The charts of the last command.
    charts: Vec<Chart>,
    /// A DATA LIST without FILE or an input program, waiting for its BEGIN DATA, and the
    /// commands that came in between, to run after the data.
    pending_data: Option<data_list::Pending>,
    queued: Vec<Command>,
    /// An INPUT PROGRAM or FILE TYPE whose commands are being collected.
    collecting_input: Option<input::Program>,
    /// Set in the engine that builds the cases of an input program.
    input: Option<Box<input::InputState>>,
    /// LEAVE: the variables (lower case) that keep their value from case to case in this pass.
    leave: std::collections::HashSet<String>,
    /// What TEMPORARY returns to after the next command that reads the data.
    temporary: Option<Box<split::Saved>>,
    /// N OF CASES, waiting for the next data pass.
    case_limit: Option<usize>,
    /// The random number generators; expressions draw from them through a shared reference.
    random: std::cell::RefCell<random::Random>,
    /// While a procedure runs on a split group or the filtered cases: the case number in the
    /// whole dataset (0-based) of each case it sees.
    case_numbers: Option<Vec<usize>>,
    /// Per scratch variable (lower case): the cases that gave it a value in this pass.
    scratch_assigned: std::collections::HashMap<String, Vec<bool>>,
    /// The transformations kept to run case by case at the end of the pass, and how many
    /// DO IF blocks are open after them.
    pending: Option<Vec<Command>>,
    pending_depth: usize,
    /// Set in an engine that runs one case of a case-by-case pass.
    row: Option<Box<rows::Row>>,
    /// Tables of the case-by-case pass, shown with the command that ended it.
    flushed: Vec<Table>,
    /// A LOOP whose commands are being collected.
    collecting: Option<loops::Collecting>,
    /// How many loops are running.
    loop_depth: usize,
    /// The vectors by lower-case name, with their variables.
    vectors: std::collections::HashMap<String, Vec<String>>,
    /// The folder of the syntax file: INSERT and CD start from it. None: the current folder.
    pub base_dir: Option<std::path::PathBuf>,
    /// The folder of the window's caller: relative names start from it while no CD is in
    /// force (see `Session::set_work_dir`). None: the process's folder.
    pub work_dir: Option<std::path::PathBuf>,
    /// Settings, named datasets and the CD folder (see `session_commands`).
    session: session_commands::State,
    /// What PRINT, WRITE and XSAVE gave in the case-by-case pass, in order.
    case_outputs: Vec<print::CaseOutput>,
    /// What CLEAR TRANSFORMATIONS returns to; None when nothing changed since the last pass.
    clean: Option<Box<output::Clean>>,
    /// The output documents and the workbooks OUTPUT EXPORT wrote.
    documents: output_docs::Documents,
    /// See `error_position`.
    error_at: Option<(u32, u32)>,
    /// The specifications of the last PPLOT, for PPLOT APPLY.
    pplot_model: Option<Box<statscraft_syntax::Pplot>>,
}

impl Engine {
    pub fn with_dataset(dataset: Option<Dataset>) -> Self {
        Self {
            dataset,
            ..Self::default()
        }
    }

    pub fn dataset(&self) -> Option<&Dataset> {
        self.dataset.as_ref()
    }

    /// The warnings of the last `execute`, such as a variable a procedure skipped. A command
    /// with warnings still succeeded.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// The charts of the last `execute`, taken out of the engine.
    pub fn take_charts(&mut self) -> Vec<Chart> {
        std::mem::take(&mut self.charts)
    }

    fn active(&self) -> Result<&Dataset, Error> {
        self.dataset
            .as_ref()
            .ok_or_else(|| Error("no active dataset: load one with GET FILE first".into()))
    }

    fn active_mut(&mut self) -> Result<&mut Dataset, Error> {
        self.dataset
            .as_mut()
            .ok_or_else(|| Error("no active dataset: load one with GET FILE first".into()))
    }

    /// Variable names in dictionary order and spelling for a syntax variable list.
    fn expand(&self, items: &[VarItem]) -> Result<Vec<String>, Error> {
        expand_in(self.active()?, items)
    }

    /// Where a failed `execute` went wrong inside its command, as (line, column), when the
    /// command knows: a statement of a MATRIX program.
    pub fn error_position(&mut self) -> Option<(u32, u32)> {
        self.error_at.take()
    }
}

/// Variable names of `data` in dictionary order and spelling for a syntax variable list.
pub(crate) fn expand_in(data: &Dataset, items: &[VarItem]) -> Result<Vec<String>, Error> {
    let index = |name: &str| {
        data.variable_index(name)
            .ok_or_else(|| Error(format!("unknown variable {name}")))
    };
    let mut names = Vec::new();
    for item in items {
        let (first, last) = match item {
            VarItem::All => (0, data.variable_count().saturating_sub(1)),
            VarItem::Name(name) => (index(name)?, index(name)?),
            VarItem::Range(a, b) => (index(a)?, index(b)?),
        };
        if first > last {
            return Err(Error(format!(
                "in a TO list the first variable must come before the second in the file ({} TO {})",
                data.variable_at(first).name(),
                data.variable_at(last).name()
            )));
        }
        let in_range = (first..=last).map(|i| data.variable_at(i).name());
        let scratch = in_range.clone().filter(|n| rows::is_scratch(n)).count();
        match item {
            VarItem::All => names.extend(
                in_range
                    .filter(|n| !rows::is_scratch(n))
                    .map(str::to_string),
            ),
            _ if scratch > 0 && scratch <= last - first => {
                return Err(Error(format!(
                    "a TO list cannot hold scratch and other variables at once ({} TO {})",
                    data.variable_at(first).name(),
                    data.variable_at(last).name()
                )));
            }
            _ => names.extend(in_range.map(str::to_string)),
        }
    }
    Ok(names)
}

impl Engine {
    pub fn execute(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        self.warnings.clear();
        self.charts.clear();
        self.error_at = None;
        if let Some(result) = self.collect_input(command) {
            let tables = result?;
            let mut out = std::mem::take(&mut self.flushed);
            out.extend(tables);
            return Ok(out);
        }
        // transformations between DATA LIST or an input program and its BEGIN DATA run after it
        if self.pending_data.is_some()
            && !matches!(command, Command::BeginData(_))
            && output::stage(command) != output::Stage::Pass
        {
            self.queued.push(command.clone());
            return Ok(Vec::new());
        }
        self.track_clean(command);
        if let Some(result) = self.collect_loop(command) {
            let tables = result?;
            let mut out = std::mem::take(&mut self.flushed);
            out.extend(tables);
            return Ok(out);
        }
        self.check_block(command)?;
        // neither runs the transformations that wait for the data pass
        if matches!(command, Command::ClearTransformations | Command::NewFile) {
            self.drop_pending();
        }
        // LEAVE runs the waiting transformations again itself
        if let Command::Leave(items) = command
            && self.input.is_none()
        {
            self.leave(items)?;
            return Ok(std::mem::take(&mut self.flushed));
        }
        if self.row.is_none() && self.loop_depth == 0 {
            let transformation = matches!(
                command,
                Command::Compute { .. }
                    | Command::If { .. }
                    | Command::SelectIf(_)
                    | Command::Recode(_)
                    | Command::Count(_)
                    | Command::String(_)
                    | Command::Numeric(_)
                    | Command::DoIf(_)
                    | Command::ElseIf(_)
                    | Command::Else
                    | Command::EndIf
                    | Command::Break
                    | Command::Vector(_)
                    | Command::VectorAssign { .. }
                    | Command::DoRepeat(_)
                    | Command::Print(_)
                    | Command::PrintSpace(_)
                    | Command::Xsave(_)
            );
            if self.pending.is_none() && transformation && self.needs_rows(command)? {
                self.pending = Some(Vec::new());
                self.pending_depth = self.blocks.len();
            }
            if self.pending.is_some() {
                if transformation {
                    match command {
                        Command::DoIf(_) => self.pending_depth += 1,
                        Command::EndIf if self.pending_depth == 0 => {
                            return Err(Error("END IF without DO IF".into()));
                        }
                        Command::EndIf => self.pending_depth -= 1,
                        _ => {}
                    }
                    self.pending
                        .as_mut()
                        .expect("checked")
                        .push(command.clone());
                    return Ok(Vec::new());
                }
                if self.pending_depth > 0 {
                    return Err(Error(
                        "this command cannot follow, inside DO IF, a transformation that runs case \
                         by case (LAG, or a scratch variable carried from case to case)"
                            .into(),
                    ));
                }
                self.flush()?;
            }
        }
        let tables = self.execute_command(command)?;
        let mut out = std::mem::take(&mut self.flushed);
        out.extend(tables);
        Ok(out)
    }

    fn execute_command(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        if self.pending_data.is_some() && !matches!(command, Command::BeginData(_)) {
            return Err(Error(
                "the DATA LIST before this command has no data yet: put BEGIN DATA, the data \
                 lines and END DATA right after it"
                    .into(),
            ));
        }
        match command {
            Command::Get { file, options } => {
                let data =
                    self.file_options(Dataset::read_sav(self.path(file))?, options, false)?;
                self.park_active();
                self.dataset = Some(data);
                self.scratch_assigned.clear();
                self.temporary = None;
                self.case_limit = None;
            }
            Command::Save {
                outfile,
                compression,
                options,
            } => {
                if matches!(compression, Some(syntax::Compression::Uncompressed)) {
                    return Err(Error("SAVE /UNCOMPRESSED is not supported yet".into()));
                }
                self.begin_pass()?;
                let data = self.file_options(self.active()?.clone(), options, false)?;
                if !self.dry_run {
                    data.write_sav(self.path(outfile))?;
                }
                let before = self.dataset.clone();
                self.end_pass(before.as_ref());
            }
            Command::Export(command) => return self.export_portable(command),
            Command::Import(command) => return self.import_portable(command),
            Command::GetTranslate(command) => return self.get_translate(command),
            Command::SaveTranslate(command) => {
                self.begin_pass()?;
                self.save_translate(command)?;
                let before = self.dataset.clone();
                self.end_pass(before.as_ref());
            }
            Command::VariableLabels(pairs) => {
                no_scratch(pairs.iter().map(|(n, _)| n), "VARIABLE LABELS")?;
                for (name, label) in pairs {
                    self.active_mut()?.set_variable_label(name, Some(label))?;
                }
            }
            Command::ValueLabels(groups) => {
                for group in groups {
                    no_scratch(&self.expand(&group.vars)?, "VALUE LABELS")?;
                    for name in self.expand(&group.vars)? {
                        let labels = group
                            .labels
                            .iter()
                            .map(|(v, l)| (value(v), l.clone()))
                            .collect();
                        self.active_mut()?.set_value_labels(&name, labels)?;
                    }
                }
            }
            Command::MissingValues(groups) => {
                for (vars, values) in groups {
                    no_scratch(&self.expand(vars)?, "MISSING VALUES")?;
                    for name in self.expand(vars)? {
                        self.active_mut()?
                            .set_missing_values(&name, missing(values)?)?;
                    }
                }
            }
            Command::VariableLevel(groups) => {
                for (vars, level) in groups {
                    let measure = match level {
                        syntax::Level::Scale => Measure::Scale,
                        syntax::Level::Ordinal => Measure::Ordinal,
                        syntax::Level::Nominal => Measure::Nominal,
                    };
                    for name in self.expand(vars)? {
                        self.active_mut()?.set_measure(&name, measure)?;
                    }
                }
            }
            Command::VariableWidth(groups) => {
                for (vars, width) in groups {
                    no_scratch(&self.expand(vars)?, "VARIABLE WIDTH")?;
                    for name in self.expand(vars)? {
                        let data = self.active_mut()?;
                        let v = data.variable(&name)?;
                        let (alignment, role) = (v.alignment(), v.role());
                        data.set_display(&name, Some(*width), alignment, role)?;
                    }
                }
            }
            Command::Formats(groups) => {
                for (vars, format) in groups {
                    let mut text = format!("{}{}", format.kind.name(), format.width);
                    if format.decimals > 0 {
                        text.push_str(&format!(".{}", format.decimals));
                    }
                    for name in self.expand(vars)? {
                        self.active_mut()?.set_format(&name, &text)?;
                    }
                }
            }
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
            | Command::QuickCluster(_)
            | Command::Logistic(_)
            | Command::Nomreg(_)
            | Command::Plum(_)
            | Command::Correspondence(_)
            | Command::Genlin(_)
            | Command::Pplot(_)
            | Command::Roc(_)
            | Command::Km(_)
            | Command::Survival(_)
            | Command::Glm(_)
            | Command::Mixed(_)
            | Command::Csdescriptives(_)
            | Command::Cstabulate(_)
            | Command::CsModel(_)
            | Command::Cluster(_)
            | Command::Discriminant(_)
            | Command::Anova(_)
            | Command::Tree(_)
            | Command::Graph(_)
            | Command::Report(_)
            | Command::Rank(_)
            | Command::Aggregate(_) => return self.procedure(command),
            Command::Execute => {
                self.begin_pass()?;
                let before = self.dataset.clone();
                self.end_pass(before.as_ref());
            }
            Command::Recode(groups) => {
                let inputs = groups
                    .iter()
                    .flat_map(|g| &g.mappings)
                    .flat_map(|m| &m.inputs);
                self.warn_reversed(inputs);
                self.all_or_nothing(|engine| engine.recode(groups))?
            }
            Command::Count(counts) => {
                let inputs = counts.iter().flat_map(|c| &c.criteria).flat_map(|(_, i)| i);
                self.warn_reversed(inputs);
                self.all_or_nothing(|engine| engine.count(counts))?
            }
            Command::SortCases(keys) => {
                self.not_temporary("SORT CASES")?;
                self.begin_pass()?;
                self.sort_cases(keys)?;
                if self.active()?.split_file().is_some() {
                    self.active_mut()?.set_split_file(None)?;
                    self.warn("SORT CASES turns SPLIT FILE off; give SPLIT FILE again to split");
                }
            }
            Command::RenameVariables(groups) => {
                self.all_or_nothing(|engine| engine.rename_variables(groups))?
            }
            Command::DeleteVariables(vars) => {
                let names = self.expand(vars)?;
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                self.active_mut()?.delete_variables(&names)?;
            }
            Command::Weight(variable) => {
                no_scratch(variable, "WEIGHT")?;
                self.active_mut()?
                    .set_weight_variable(variable.as_deref())?;
            }
            Command::Compute { target, expr } => self.assign(target, expr, None)?,
            Command::If {
                condition,
                target,
                expr,
            } => self.assign(target, expr, Some(condition))?,
            Command::SelectIf(condition) => self.select_if(condition)?,
            Command::String(groups) => {
                self.declare(groups.iter().map(|(names, f)| (&names[..], Some(f))))?
            }
            Command::Numeric(groups) => {
                self.declare(groups.iter().map(|(names, f)| (&names[..], f.as_ref())))?
            }
            Command::DoIf(condition) => self.do_if(condition)?,
            Command::ElseIf(condition) => self.next_branch(Some(condition))?,
            Command::Else => self.next_branch(None)?,
            Command::EndIf => self.end_if()?,
            Command::Mrsets(subcommands) => return self.mrsets(subcommands),
            Command::Csplan(command) => return self.csplan(command),
            Command::AddFiles(merge) => {
                self.not_temporary("ADD FILES")?;
                self.begin_pass()?;
                return self.merge_files(merge, true);
            }
            Command::MatchFiles(merge) => {
                self.not_temporary("MATCH FILES")?;
                self.begin_pass()?;
                return self.merge_files(merge, false);
            }
            Command::StarJoin(join) => {
                self.not_temporary("STAR JOIN")?;
                self.begin_pass()?;
                self.star_join(join)?
            }
            Command::Update(merge) => {
                if merge.inputs.iter().any(|i| i.file.is_none()) {
                    self.not_temporary("UPDATE with FILE=*")?;
                }
                self.begin_pass()?;
                self.temporary = None;
                return self.update_files(merge);
            }
            Command::SortVariables { key, descending } => self.sort_variables(key, *descending)?,
            Command::Flip(flip) => return self.flip(flip),
            Command::VarsToCases(command) => self.vars_to_cases(command)?,
            Command::CasesToVars(command) => self.cases_to_vars(command)?,
            Command::DataList(list) if self.input.is_some() => self.program_data_list(list)?,
            Command::DataList(list) => {
                self.leave.clear();
                self.park_active();
                self.scratch_assigned.clear();
                self.temporary = None;
                self.case_limit = None;
                self.data_list(list)?
            }
            Command::MatrixData(_) if self.input.is_some() => {
                return Err(Error(
                    "MATRIX DATA defines a dataset of its own and cannot stand inside an input \
                     program"
                        .into(),
                ));
            }
            Command::MatrixData(matrix) => {
                self.leave.clear();
                self.park_active();
                self.scratch_assigned.clear();
                self.temporary = None;
                self.case_limit = None;
                match &matrix.file {
                    None => self.pending_data = Some(data_list::Pending::Matrix(matrix.clone())),
                    Some(file) => {
                        let records = self.file_records(file)?;
                        self.read_matrix(matrix, records)?;
                    }
                }
            }
            Command::BeginData(lines) => return self.begin_data(lines),
            Command::BeginChart(lines) => {
                let chart = chart_input::read(lines).map_err(Error)?;
                self.charts.push(chart);
            }
            Command::Output(output) => return self.output_command(output),
            Command::Finish => {}
            Command::EndCase => self.end_case()?,
            Command::EndFile => self.end_file()?,
            Command::Reread { file, column } => self.reread(file.as_deref(), column.as_ref())?,
            Command::RepeatingData(repeating) => self.repeating_data(repeating)?,
            Command::Leave(items) => self.leave(items)?,
            Command::InputProgram
            | Command::EndInputProgram
            | Command::FileType(_)
            | Command::RecordType(_)
            | Command::EndFileType => {
                return Err(Error(
                    "this command belongs to an INPUT PROGRAM or FILE TYPE block of its own".into(),
                ));
            }
            Command::SplitFile(split) => {
                let split = match split {
                    Some(split) => {
                        let variables = self.expand(&split.variables)?;
                        no_scratch(&variables, "SPLIT FILE")?;
                        if variables.len() > 8 {
                            return Err(Error("SPLIT FILE takes at most eight variables".into()));
                        }
                        Some(statscraft_data::SplitFile {
                            variables,
                            layered: !split.separate,
                        })
                    }
                    None => None,
                };
                self.active_mut()?.set_split_file(split)?;
            }
            Command::Filter { variable, bare } => {
                if *bare {
                    self.warn("FILTER without BY or OFF turns the filter off");
                }
                no_scratch(variable, "FILTER")?;
                self.active_mut()?
                    .set_filter_variable(variable.as_deref())?;
            }
            Command::Temporary => {
                self.drop_scratch()?;
                self.temporary()
            }
            Command::NOfCases(n) => {
                let n = usize::try_from(*n).unwrap_or(usize::MAX);
                self.case_limit = Some(self.case_limit.map_or(n, |limit| limit.min(n)));
            }
            Command::Sample(sample) => self.sample(sample)?,
            Command::DoRepeat(repeat) => return self.do_repeat(repeat),
            Command::Matrix(program) => return self.matrix(program),
            Command::Loop(_) => unreachable!("collect_loop takes LOOP"),
            Command::EndLoop(_) => return Err(Error("END LOOP without LOOP".into())),
            Command::Break => self.break_loop()?,
            Command::Vector(specs) => self.vector(specs)?,
            Command::Autorecode(command) => {
                self.begin_pass()?;
                let before = self.dataset.clone();
                let tables = self.autorecode(command);
                self.end_pass(before.as_ref());
                return tables;
            }
            Command::VectorAssign {
                condition,
                vector,
                index,
                expr,
            } => self.vector_assign(condition.as_ref(), vector, index, expr)?,
            _ => return self.execute_session(command),
        }
        Ok(Vec::new())
    }

    /// The procedures, on the dataset as it is now.
    fn run_procedure(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        let mut tables = match command {
            Command::Frequencies(command) => self.frequencies(command),
            Command::Ctables(command) => self.ctables(command),
            Command::Descriptives(command) => self.descriptives(command),
            Command::Crosstabs(command) => self.crosstabs(command),
            Command::MultResponse(command) => self.mult_response(command),
            Command::Means(command) => self.means(command),
            Command::TTest(command) => self.t_test(command),
            Command::Oneway(command) => self.oneway(command),
            Command::Correlations(command) => self.correlations(command),
            Command::NonparCorr(command) => self.nonpar_corr(command),
            Command::NparTests(command) => self.npar_tests(command),
            Command::PartialCorr(command) => self.partial_corr(command),
            Command::Examine(command) => self.examine(command),
            Command::Regression(command) => self.regression(command),
            Command::Reliability(command) => self.reliability(command),
            Command::Factor(command) => self.factor(command),
            Command::QuickCluster(command) => self.quick_cluster(command),
            Command::Logistic(command) => self.logistic(command),
            Command::Nomreg(command) => self.nomreg(command),
            Command::Plum(command) => self.plum(command),
            Command::Correspondence(command) => self.correspondence(command),
            Command::Genlin(command) => self.genlin(command),
            Command::Pplot(command) => self.pplot(command),
            Command::Roc(command) => self.roc(command),
            Command::Km(command) => self.km(command),
            Command::Survival(command) => self.life_tables(command),
            Command::Glm(command) => self.glm(command),
            Command::Mixed(command) => self.mixed_model(command),
            Command::Csdescriptives(command) => self.csdescriptives(command),
            Command::Cstabulate(command) => self.cstabulate(command),
            Command::CsModel(command) => self.cs_model(command),
            Command::Cluster(command) => self.cluster(command),
            Command::Discriminant(command) => self.discriminant(command),
            Command::Anova(command) => self.anova(command),
            Command::Tree(command) => self.tree(command),
            Command::Graph(command) => self.graph(command),
            Command::Report(command) => self.report(command),
            Command::Rank(command) => self.rank(command),
            Command::Aggregate(command) => self.aggregate(command).map(|()| Vec::new()),
            Command::List(list) => self.list(list),
            _ => unreachable!("only procedures come here"),
        }?;
        for format in tables.iter_mut().flat_map(|t| &mut t.formats).flatten() {
            *format = self.currency_format(format);
        }
        Ok(tables)
    }

    fn not_temporary(&self, command: &str) -> Result<(), Error> {
        if self.in_temporary() {
            return Err(Error(format!(
                "{command} cannot follow TEMPORARY: run a procedure or EXECUTE first"
            )));
        }
        Ok(())
    }
}

/// Scratch variables have no dictionary entries of their own and no part in procedures.
fn no_scratch<'a>(names: impl IntoIterator<Item = &'a String>, command: &str) -> Result<(), Error> {
    match names.into_iter().find(|n| rows::is_scratch(n)) {
        Some(name) => Err(Error(format!(
            "{command} cannot name the scratch variable {name}"
        ))),
        None => Ok(()),
    }
}

fn value(v: &syntax::Value) -> Value {
    match v {
        syntax::Value::Number(n) => Value::Number(*n),
        syntax::Value::String(s) => Value::String(s.clone()),
    }
}

fn missing(values: &[syntax::MissingValue]) -> Result<MissingValues, Error> {
    let bound = |b: &Bound| match b {
        Bound::Lowest => f64::MIN,
        Bound::Highest => f64::MAX,
        Bound::Number(n) => *n,
    };
    let (mut numbers, mut strings, mut range) = (Vec::new(), Vec::new(), None);
    for v in values {
        match v {
            syntax::MissingValue::Value(syntax::Value::Number(n)) => numbers.push(*n),
            syntax::MissingValue::Value(syntax::Value::String(s)) => strings.push(s.clone()),
            syntax::MissingValue::Range(low, high) => range = Some((bound(low), bound(high))),
        }
    }
    Ok(match (range, numbers.as_slice(), strings.is_empty()) {
        (None, [], true) => MissingValues::None,
        (None, _, true) => MissingValues::Numbers(numbers),
        (None, [], false) => MissingValues::Strings(strings),
        (Some((low, high)), extra, true) if extra.len() <= 1 => MissingValues::Range {
            low,
            high,
            extra: extra.first().copied(),
        },
        _ => return Err(Error("unsupported combination of missing values".into())),
    })
}
