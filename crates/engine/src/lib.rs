//! Executes parsed commands on a dataset and returns result tables.

mod aggregate;
mod correlations;
mod crosstabs;
mod ctables;
mod descriptives;
mod dist;
mod expr;
mod frequencies;
pub mod live;
mod means;
mod merge;
mod mrsets;
mod oneway;
mod recode;
pub mod session;
mod sort;
mod transform;
mod ttest;
mod variables;

pub use ctables::format_value;
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
#[derive(Debug, Default, Serialize, Deserialize)]
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
#[derive(Debug, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Number(f64),
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
        for variable in after.into_iter().flat_map(Dataset::variables) {
            let name = variable.name();
            match before.and_then(|d| d.variables().find(|v| v.name() == name)) {
                None => changes.variables_added.push(name.to_string()),
                Some(previous) if previous != variable => {
                    changes.variables_changed.push(name.to_string())
                }
                Some(_) => {}
            }
        }
        for name in before.into_iter().flat_map(Dataset::variable_names) {
            if !after.is_some_and(|d| d.variable_names().any(|n| n == name)) {
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
        let data = self.active()?;
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
            names.extend((first..=last).map(|i| data.variable_at(i).name().to_string()));
        }
        Ok(names)
    }

    pub fn execute(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        self.warnings.clear();
        self.check_block(command)?;
        match command {
            Command::Get { file } => {
                self.dataset = Some(Dataset::read_sav(file)?);
            }
            Command::Save {
                outfile,
                compression,
            } => {
                if matches!(compression, Some(syntax::Compression::Uncompressed)) {
                    return Err(Error("SAVE /UNCOMPRESSED is not supported yet".into()));
                }
                let data = self.active()?;
                if !self.dry_run {
                    data.write_sav(outfile)?;
                }
            }
            Command::VariableLabels(pairs) => {
                for (name, label) in pairs {
                    self.active_mut()?.set_variable_label(name, Some(label))?;
                }
            }
            Command::ValueLabels(groups) => {
                for group in groups {
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
            Command::Frequencies { variables } => {
                let data = self.active()?;
                return self
                    .expand(variables)?
                    .iter()
                    .map(|name| frequencies::table(data, name))
                    .collect();
            }
            Command::Execute => {}
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
            Command::SortCases(keys) => self.sort_cases(keys)?,
            Command::RenameVariables(groups) => {
                self.all_or_nothing(|engine| engine.rename_variables(groups))?
            }
            Command::DeleteVariables(vars) => {
                let names = self.expand(vars)?;
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                self.active_mut()?.delete_variables(&names)?;
            }
            Command::Weight(variable) => {
                self.active_mut()?
                    .set_weight_variable(variable.as_deref())?;
            }
            Command::Aggregate(command) => self.aggregate(command)?,
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
            Command::Ctables(command) => return self.ctables(command),
            Command::Mrsets(subcommands) => return self.mrsets(subcommands),
            Command::Descriptives(command) => return self.descriptives(command),
            Command::Crosstabs(command) => return self.crosstabs(command),
            Command::Means(command) => return self.means(command),
            Command::TTest(command) => return self.t_test(command),
            Command::Oneway(command) => return self.oneway(command),
            Command::Correlations(command) => return self.correlations(command),
            Command::AddFiles(merge) => self.merge_files(merge, true)?,
            Command::MatchFiles(merge) => self.merge_files(merge, false)?,
        }
        Ok(Vec::new())
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
