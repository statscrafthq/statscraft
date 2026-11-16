//! Executes parsed commands on a dataset and returns result tables.

mod aggregate;
mod agreement;
mod alscal;
mod anova;
mod areg;
mod arima;
mod autorecode;
mod bayes;
mod catpca;
pub mod chart;
mod chart_input;
mod cluster;
mod codebook;
mod compare_datasets;
mod complex_samples;
mod contrast;
mod correlations;
mod correspondence;
mod coxreg;
mod crosstabs;
mod ctables;
mod curvefit;
mod data_list;
mod date;
mod descriptives;
mod dictionary;
mod discriminant;
// its own crate so that dev builds can optimize it (see the workspace Cargo.toml)
use statscraft_dist as dist;
mod distributions;
mod examine;
pub mod excel;
mod expr;
mod expr_functions;
mod exsmooth;
mod factor;
mod format;
mod frequencies;
mod genlin;
mod genlog;
mod get_data;
mod get_translate;
mod glm;
mod graph;
mod hiloglinear;
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
mod mds;
mod means;
mod merge;
mod mixed;
mod model_terms;
mod mrsets;
mod mult_response;
mod multiple_correspondence;
mod multiple_imputation;
mod mva;
mod nomreg;
mod nonpar_corr;
mod npar_groups;
mod npar_tests;
mod nptests;
pub mod office;
mod oneway;
mod optimal_scaling;
mod output;
mod output_docs;
pub mod output_file;
pub mod output_modify;
mod partial_corr;
mod pass;
pub mod pdf;
pub mod picture;
pub mod plot;
pub mod plugin;
mod plum;
mod pooling;
mod portable;
mod power;
mod pplot;
mod print;
mod probit;
mod proximities;
mod proxscal;
mod quick_cluster;
mod random;
mod rank;
mod ratio_statistics;
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
mod spectra;
mod split;
mod star_join;
mod stata;
mod summarize;
mod timeseries;
mod transform;
mod tree;
mod ts_plot;
mod ts_range;
mod ttest;
mod two_stage;
mod validatedata;
mod variables;
pub mod vega_lite;
mod wls;
pub mod workbook;
pub mod xlsx;

pub use chart::Chart;
pub use expr::functions;
pub use format::{Currencies, currency_parts, format_value, is_date_time_or_currency};
pub use get_data::delimited;
pub use html::{
    STYLE as HTML_STYLE, cell_attributes, header_attributes, label_attributes, table_attributes,
    table_html,
};
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
    /// What a plugin needs to judge the cells: their estimates and bases, and which rows and
    /// columns are totals. Only CTABLES fills it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_stats: Option<Box<CellStats>>,
    /// Cell colours and comments a plugin gave (docs/plugins.md), sorted by row and column.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decorations: Vec<Decoration>,
    /// The font and the header cell styles a plugin gave.
    #[serde(default, skip_serializing_if = "TableStyle::is_empty")]
    pub style: TableStyle,
}

/// The statistics behind the cells of a table, for plugins. Rows and columns count as in
/// `Table::rows`, the row label columns included.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct CellStats {
    /// The rows and the columns that hold a total or a subtotal.
    pub total_rows: Vec<usize>,
    pub total_columns: Vec<usize>,
    /// The cells with a statistic, row by row.
    pub cells: Vec<CellStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CellStat {
    pub row: usize,
    pub col: usize,
    /// The summary function as written in syntax, such as COLPCT.COUNT, COUNT or MEAN.
    pub kind: String,
    /// A percentage as a proportion from 0 to 1, any other statistic as shown.
    pub estimate: f64,
    /// The cases the estimate rests on, unweighted: the percentage's base for a percentage,
    /// the cell's valid cases otherwise.
    pub base: f64,
    /// The same base, weighted.
    pub weighted_base: f64,
    /// The same base as (sum of weights)^2 / (sum of squared weights); `base` without weights.
    pub effective_base: f64,
    /// The standard deviation, for a mean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd: Option<f64>,
}

/// How a plugin marks one cell: colours as #rrggbb, bold or italic text, coloured borders,
/// the horizontal alignment of its text and a comment.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Decoration {
    pub row: usize,
    pub col: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// "left", "center" or "right"; the exports' own alignment without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// Thin lines on the sides of a cell, each in its colour #rrggbb.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Border {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
}

/// The look of a cell without its place and comment: cells that look alike share it.
pub type CellLook<'a> = (
    Option<&'a str>,
    Option<&'a str>,
    bool,
    bool,
    Option<&'a Border>,
    Option<&'a str>,
);

impl Decoration {
    pub fn look(&self) -> CellLook<'_> {
        let (fill, text) = (self.fill.as_deref(), self.text.as_deref());
        let align = self.align.as_deref();
        (
            fill,
            text,
            self.bold,
            self.italic,
            self.border.as_ref(),
            align,
        )
    }

    /// Colours that are not #rrggbb left out, a border without a side and an unknown
    /// alignment.
    fn clean(&mut self) {
        let keep = |c: &mut Option<String>| *c = c.take().filter(|c| parse_colour(c).is_some());
        keep(&mut self.fill);
        keep(&mut self.text);
        if let Some(b) = &mut self.border {
            [&mut b.top, &mut b.bottom, &mut b.left, &mut b.right]
                .into_iter()
                .for_each(keep);
        }
        self.border = self.border.take().filter(|b| *b != Border::default());
        let known = |a: &String| ["left", "center", "right"].contains(&a.as_str());
        self.align = self.align.take().filter(known);
    }
}

/// What a plugin gave a table as a whole: the font of its exports, the styles of its header
/// cells (`row` counting the lines of `header_rows` from the top) and the layout of its exports.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TableStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// In points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_cells: Vec<Decoration>,
    /// The exports leave out their own borders, header fill and bold header: only the
    /// plugin's styles show.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub plain: bool,
    /// "stacked": the exports show each block's outer row label on a row of its own above the
    /// block, see `Table::stacked`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_labels: Option<String>,
    /// Excel column widths, by column as in `rows`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_widths: Vec<ColumnWidth>,
    /// Excel: the sheet columns kept in view when scrolling right, counted from the left of
    /// the exported table (after a stacked layout dropped its outer column).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_columns: Option<u16>,
}

/// The width of a column in Excel, in characters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ColumnWidth {
    pub col: usize,
    pub width: f64,
}

impl TableStyle {
    pub fn is_empty(&self) -> bool {
        *self == TableStyle::default()
    }
}

/// A colour written #rrggbb, as red, green and blue.
pub fn parse_colour(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#').filter(|h| h.len() == 6)?;
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

impl Table {
    /// The decoration of a cell, if a plugin gave one.
    pub fn decoration(&self, row: usize, col: usize) -> Option<&Decoration> {
        let found = self
            .decorations
            .binary_search_by(|d| (d.row, d.col).cmp(&(row, col)));
        found.ok().map(|i| &self.decorations[i])
    }

    /// The style a plugin gave a header cell, `line` counting the lines of `header_rows`.
    pub fn header_decoration(&self, line: usize, col: usize) -> Option<&Decoration> {
        let cells = &self.style.header_cells;
        let found = cells.binary_search_by(|d| (d.row, d.col).cmp(&(line, col)));
        found.ok().map(|i| &cells[i])
    }

    /// Whether the exports show the table's row labels stacked: the outer row label column
    /// (column 0) leaves the table, and each run of rows that share its label gets a row of its
    /// own above it holding that label in the first column. Only with two row label columns
    /// or more.
    pub fn stacked(&self) -> bool {
        self.row_headers >= 2 && self.style.row_labels.as_deref() == Some("stacked")
    }

    /// With stacked row labels, the rows that start a block and get a label row above them:
    /// each row whose outer label is not empty and differs from the label of the row above.
    /// Empty when the labels are not stacked.
    pub fn block_starts(&self) -> Vec<usize> {
        if !self.stacked() {
            return Vec::new();
        }
        let label = |r: usize| cell_text(self, r, 0);
        let starts = (0..self.rows.len()).filter(|&r| {
            let text = label(r);
            !text.is_empty() && (r == 0 || label(r - 1) != text)
        });
        starts.collect()
    }

    /// Puts a plugin's decorations and table style on the table: cells outside the body or the
    /// header, colours that are not #rrggbb, a font name that is not plain (letters, digits,
    /// spaces and hyphens, at most 31 characters), a size outside 1 to 409 points, row labels
    /// other than "stacked" and widths of columns off the table are left out; a width is kept
    /// within 1 to 255 characters.
    pub fn set_decorations(&mut self, decorations: Vec<Decoration>, style: TableStyle) {
        let columns = self.columns.len();
        let tidy = |mut cells: Vec<Decoration>, rows: usize| {
            cells.retain(|d| d.row < rows && d.col < columns);
            cells.iter_mut().for_each(Decoration::clean);
            cells.sort_by_key(|d| (d.row, d.col));
            cells.dedup_by_key(|d| (d.row, d.col));
            cells
        };
        self.decorations = tidy(decorations, self.rows.len());
        let mut header_cells = tidy(style.header_cells, header_rows(self).len());
        header_cells.iter_mut().for_each(|d| d.comment = None);
        // a plain name, so it cannot break out of the CSS or the workbook's XML
        let plain = |f: &String| {
            let ok = |c: char| c.is_alphanumeric() || c == ' ' || c == '-';
            (1..=31).contains(&f.chars().count()) && f.chars().all(ok)
        };
        self.style = TableStyle {
            font: style.font.filter(plain),
            font_size: style.font_size.filter(|s| (1.0..=409.0).contains(s)),
            header_cells,
            plain: style.plain,
            row_labels: style.row_labels.filter(|r| r == "stacked"),
            column_widths: {
                let mut widths = style.column_widths;
                widths.retain(|w| w.col < columns && w.width.is_finite());
                widths
                    .iter_mut()
                    .for_each(|w| w.width = w.width.clamp(1.0, 255.0));
                widths.sort_by_key(|w| w.col);
                widths.dedup_by_key(|w| w.col);
                widths
            },
            // counted on the sheet: a stacked layout shows one column fewer
            freeze_columns: {
                let stacked =
                    style.row_labels.as_deref() == Some("stacked") && self.row_headers >= 2;
                let shown = columns - usize::from(stacked);
                style.freeze_columns.filter(|&n| usize::from(n) <= shown)
            },
        };
    }

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
    /// TIME PROGRAM: the transformations that define time-dependent covariates for COXREG.
    time_program: Option<coxreg::TimeProgram>,
    /// The time program the running COXREG took up.
    running_program: Option<coxreg::TimeProgram>,
    /// The running COXREG failed for a fault in its time program.
    program_fault: std::cell::Cell<bool>,
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
    /// While a procedure runs on a split group: each split variable with the group's value.
    split_group: Option<Vec<(String, Option<crosstabs::Key>)>>,
    /// PROXIMITIES /MATRIX=OUT: the split variables and the matrix of every split group.
    proximity_out: (Option<statscraft_data::SplitFile>, Vec<Dataset>),
    /// MULTIPLE IMPUTATION: per split group, the stacked imputations (original data first)
    /// and the FCS iteration history.
    imputed: Vec<(Vec<Dataset>, Option<Dataset>)>,
    /// While a procedure runs on one imputation of data split by Imputation_: what it gives
    /// for the pooled results.
    pool_items: Option<Vec<pooling::PoolItem>>,
    /// The specifications of the last PPLOT, for PPLOT APPLY.
    pplot_model: Option<Box<statscraft_syntax::Pplot>>,
    /// TSET: the settings of the time series procedures.
    tset: statscraft_syntax::TimeSettings,
    /// The last ACF, PACF, CCF, SEASON, EXSMOOTH and ARIMA, for APPLY.
    ts_models: timeseries::Models,
    /// USE: the historical period of the time series procedures.
    ts_use: Option<statscraft_syntax::UseRange>,
    /// PREDICT: the forecast period.
    ts_predict: Option<statscraft_syntax::UseRange>,
    /// The last DATE, to date the cases PREDICT adds.
    date_spec: Option<statscraft_syntax::DateSpec>,
    /// While EXSMOOTH or ARIMA runs: the cases PREDICT added at the end of the file, which
    /// are not part of the series.
    ts_added: usize,
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

/// The commands that run case by case on the data as they come.
fn is_transformation(command: &Command) -> bool {
    matches!(
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
    )
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
        // the transformations after TIME PROGRAM, loops among them, wait for COXREG
        if let Some(program) = self.time_program.as_mut().filter(|p| p.collecting) {
            if is_transformation(command)
                || matches!(command, Command::Loop(_) | Command::EndLoop(_))
            {
                program.commands.push(command.clone());
                return Ok(Vec::new());
            }
            program.collecting = false;
        }
        // the next command that reads the data uses a time program up (begin_pass drops it);
        // only COXREG runs it
        if !matches!(command, Command::Coxreg(_)) && output::stage(command) == output::Stage::Pass {
            self.time_program = None;
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
            self.time_program = None;
        }
        // LEAVE runs the waiting transformations again itself
        if let Command::Leave(items) = command
            && self.input.is_none()
        {
            self.leave(items)?;
            return Ok(std::mem::take(&mut self.flushed));
        }
        if self.row.is_none() && self.loop_depth == 0 {
            let transformation = is_transformation(command);
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
        if matches!(command, Command::Coxreg(_)) {
            // the vectors go along: the data pass of COXREG clears them before it runs the program
            // the vectors in force now, over those a program a failed COXREG kept had then
            self.running_program = self.time_program.take().map(|mut program| {
                program.vectors.extend(self.vectors.clone());
                program
            });
        }
        self.program_fault.set(false);
        let tables = self.execute_command(command);
        if matches!(command, Command::Coxreg(_)) {
            let program = self.running_program.take();
            // a COXREG that fails leaves its time program for the corrected one, unless the fault
            // is in the program itself
            if tables.is_err() && !self.program_fault.get() {
                self.time_program = program;
            }
        }
        let tables = tables?;
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
                let path = self.path(file);
                let mut data = self.file_options(Dataset::read_sav(&path)?, options, false)?;
                data.set_file(Some(std::path::absolute(&path).unwrap_or(path)));
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
                    let path = self.path(outfile);
                    data.write_sav(&path)?;
                    let path = std::path::absolute(&path).unwrap_or(path);
                    self.active_mut()?.set_file(Some(path));
                }
                let before = self.dataset.clone();
                self.end_pass(before.as_ref());
            }
            Command::Export(command) => return self.export_portable(command),
            Command::Import(command) => return self.import_portable(command),
            Command::GetTranslate(command) => return self.get_translate(command),
            Command::GetStata(command) => return self.get_stata(command),
            Command::SaveTranslate(command) => {
                self.begin_pass()?;
                let tables = self.save_translate(command)?;
                let before = self.dataset.clone();
                self.end_pass(before.as_ref());
                return Ok(tables);
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
            Command::Tset(items) => self.tset_command(items),
            Command::Tshow => return Ok(self.tshow()),
            Command::Create(equations) => return self.create(equations),
            Command::Correlogram(spec) => {
                let spec = self.correlogram_spec(spec)?;
                return self.procedure(&Command::Correlogram(Box::new(spec)));
            }
            Command::Season(spec) => {
                let spec = Command::Season(Box::new(self.season_spec(spec)?));
                return self.with_current_names(&spec);
            }
            Command::Exsmooth(spec) => {
                let spec = Command::Exsmooth(Box::new(self.exsmooth_spec(spec)?));
                return self.with_current_names(&spec);
            }
            Command::Arima(spec) => {
                let spec = Command::Arima(Box::new(self.arima_spec(spec)?));
                return self.with_current_names(&spec);
            }
            Command::Areg(spec) => {
                let spec = Command::Areg(Box::new(self.areg_spec(spec)?));
                return self.with_current_names(&spec);
            }
            Command::TwoStage(spec) => {
                let spec = Command::TwoStage(Box::new(self.two_stage_spec(spec)?));
                return self.with_current_names(&spec);
            }
            Command::Spectra(spec) => {
                let spec = self.spectra_spec(spec)?;
                return self.procedure(&Command::Spectra(Box::new(spec)));
            }
            Command::Predict(range) => self.ts_predict = Some(range.clone()),
            Command::SeriesPlot(spec) => {
                let spec = self.series_plot_spec(spec)?;
                return self.procedure(&Command::SeriesPlot(Box::new(spec)));
            }
            Command::Curvefit(_) | Command::Wls(_) => return self.with_current_names(command),
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
            | Command::Nptests(_)
            | Command::PartialCorr(_)
            | Command::Examine(_)
            | Command::Regression(_)
            | Command::Reliability(_)
            | Command::WeightedKappa(_)
            | Command::FleissKappa(_)
            | Command::RatioStatistics(_)
            | Command::Bayes(_)
            | Command::Factor(_)
            | Command::QuickCluster(_)
            | Command::Logistic(_)
            | Command::Coxreg(_)
            | Command::Nomreg(_)
            | Command::Mva(_)
            | Command::Hiloglinear(_)
            | Command::Genlog(_)
            | Command::Probit(_)
            | Command::Plum(_)
            | Command::Correspondence(_)
            | Command::MultipleCorrespondence(_)
            | Command::Catpca(_)
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
            | Command::Proxscal(_)
            | Command::Alscal(_)
            | Command::Discriminant(_)
            | Command::Anova(_)
            | Command::Tree(_)
            | Command::Graph(_)
            | Command::Report(_)
            | Command::Summarize(_)
            | Command::Validatedata(_)
            | Command::Rank(_)
            | Command::Aggregate(_) => return self.procedure(command),
            Command::Codebook(command) => return self.codebook(command),
            Command::CompareDatasets(command) => {
                self.begin_pass()?;
                let before = self.dataset.clone();
                let tables = self.compare_datasets(command);
                self.end_pass(before.as_ref());
                return tables;
            }
            Command::Power(command) => return self.power(command),
            Command::TimeProgram => {
                self.time_program = Some(coxreg::TimeProgram {
                    collecting: true,
                    ..Default::default()
                });
            }
            Command::ClearTimeProgram => self.time_program = None,
            Command::MultipleImputation(imputation) => {
                self.imputed.clear();
                let split = self.active()?.split_file().cloned();
                let tables = self.procedure(command)?;
                self.write_imputations(imputation, split)?;
                return Ok(tables);
            }
            Command::Proximities(proximities) => {
                let split = self.active()?.split_file().cloned();
                self.proximity_out = (split, Vec::new());
                let tables = self.procedure(command)?;
                self.proximities_out(proximities)?;
                return Ok(tables);
            }
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
                self.ts_use = None;
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
                // FILTER and USE exclude each other
                self.ts_use = None;
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
            Command::Nptests(command) => self.nptests(command),
            Command::PartialCorr(command) => self.partial_corr(command),
            Command::Examine(command) => self.examine(command),
            Command::Regression(command) => self.regression(command),
            Command::Reliability(command) => self.reliability(command),
            Command::WeightedKappa(command) => self.weighted_kappa(command),
            Command::FleissKappa(command) => self.fleiss_kappa(command),
            Command::RatioStatistics(command) => self.ratio_statistics(command),
            Command::Bayes(command) => self.bayes(command),
            Command::Factor(command) => self.factor(command),
            Command::QuickCluster(command) => self.quick_cluster(command),
            Command::Logistic(command) => self.logistic(command),
            Command::Coxreg(command) => self.coxreg(command),
            Command::Nomreg(command) => self.nomreg(command),
            Command::Mva(command) => self.mva(command),
            Command::Hiloglinear(command) => self.hiloglinear(command),
            Command::Genlog(command) => self.genlog(command),
            Command::Probit(command) => self.probit(command),
            Command::Curvefit(command) => self.curvefit(command),
            Command::Wls(command) => self.wls(command),
            Command::Correlogram(command) => self.correlogram(command),
            Command::Season(command) => self.season(command),
            Command::Exsmooth(command) => self.exsmooth(command),
            Command::Arima(command) => self.arima(command),
            Command::Areg(command) => self.areg(command),
            Command::TwoStage(command) => self.two_stage(command),
            Command::Spectra(command) => self.spectra(command),
            Command::SeriesPlot(command) => self.series_plot(command),
            Command::MultipleImputation(command) => self.multiple_imputation(command),
            Command::Plum(command) => self.plum(command),
            Command::Correspondence(command) => self.correspondence(command),
            Command::MultipleCorrespondence(command) => self.multiple_correspondence(command),
            Command::Catpca(command) => self.catpca(command),
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
            Command::Proximities(command) => self.proximities(command),
            Command::Proxscal(command) => self.proxscal(command),
            Command::Alscal(command) => self.alscal(command),
            Command::Discriminant(command) => self.discriminant(command),
            Command::Anova(command) => self.anova(command),
            Command::Tree(command) => self.tree(command),
            Command::Graph(command) => self.graph(command),
            Command::Report(command) => self.report(command),
            Command::Summarize(command) => self.summarize(command),
            Command::Validatedata(command) => self.validatedata(command),
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

    /// A procedure that saves variables under TSET NEWVAR: under CURRENT the variables of the
    /// names it will use are dropped first; all comes back when it fails.
    fn with_current_names(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let names = self.current_names(command)?;
        let before = self.dataset.clone();
        let temporary = self.in_temporary();
        self.replace_current(&names)?;
        let result = self
            .extend_for_predict(command)
            .and_then(|()| self.procedure(command));
        self.ts_added = 0;
        // after TEMPORARY the procedure already went back to the dataset before it
        if result.is_err() && !temporary {
            self.dataset = before;
        }
        result
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
