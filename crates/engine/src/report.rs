//! REPORT: case listings and summary statistics by break groups.
//!
//! Rules, from the command's description:
//! - Each BREAK with variables has a break column (one for all with ONEBREAKCOLUMN, its values
//!   indented by INDENT per level); the report columns follow in the order of VARIABLES. A
//!   stacked column (a + b) lists each variable on a line of its own.
//! - A group of a BREAK starts when one of its variables or of an earlier BREAK changes value,
//!   so the cases must be sorted. Its values head it (VALUE by default with MANUAL, LABEL with
//!   AUTOMATIC); its SUMMARY lines follow when it ends, those of inner breaks first. BRKSPACE
//!   blank lines follow the head (MANUAL 1); with -1 the next line shares the head's line and
//!   a summary title there is left out.
//! - Summary statistics use only valid values; MISSING=VAR leaves user-missing values out of
//!   the statistics but lists them, NONE takes them as valid, LIST(vars n) drops the cases
//!   missing on n or more of the variables. The missing symbol (FORMAT MISSING) shows a
//!   system-missing value. VALIDN is the only function for strings.
//! - An aggregate goes in the column of its variable, without variables in every report
//!   column; a composite goes in its column, else in that of its first argument variable
//!   that is a report variable. Two results in one column of a line are an error.
//! - Default titles and formats are those of the manual's tables: VALIDN F5.0, SUM the
//!   dictionary format two wider, MEAN STDDEV VARIANCE MIN MAX MEDIAN MODE the dictionary
//!   format, SKEWNESS KURTOSIS F5.2, PGT PLT PIN PCT6.1, PERCENT F6.1, FREQUENCY F5.0, the
//!   composites F with the dictionary width and 0 decimals, PCT PCT6.2. PLAIN is F; (d)
//!   sets the decimals.
//! - STRING joins variables and literals; a numeric part is padded with zeros on the left,
//!   or blanks with BLANK, DOLLAR or COMMA; a missing part is blank.
//! - Column widths: the widest print or summary format, string, heading line (or longest
//!   word of a label) and with LABEL the longest value label (at most 20 with MANUAL); at
//!   least 8 with MANUAL. COLSPACE (4) blanks between columns.
//!
//! Choices where the manual is silent:
//! - The weight counts as in the other procedures: VALIDN is the sum of weights.
//! - MEDIAN, MODE, FREQUENCY and PERCENT cut each value to a whole number and count those in
//!   the range; the median is the middle value, the mean of the two middle values when the
//!   weights split evenly between them, the mode the most frequent value (the lowest of a
//!   tie). PERCENT is per cent of all valid values. FREQUENCY and PERCENT give a line per value
//!   of the range, titled with the value, then one with the total, titled by the summary title.
//! - TOTAL repeats the first SUMMARY of the first BREAK for all cases, under a line Grand
//!   Total. A report without break columns puts summary titles in a column of their own.
//! - The report is one page without page breaks: LENGTH, PAGE and PREVIEW change nothing, and
//!   )PAGE is PAGE1. In the shown table blank lines are left out.
//! - OUTFILE writes the text and shows no table; the first REPORT of a session that names a
//!   file replaces it, later ones append.

use crate::descriptives::{case_weights, summarize};
use crate::{Cell, Engine, Error, Table, format_value};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{
    ColumnContents, Composite, CompositeArg, Report, ReportAlign, ReportFunction, ReportMissing,
    ReportOffset, ReportSummary, StringPart, SummaryFormat, SummaryItem, VarItem,
};
use std::io::Write as _;

/// Where the values of a report column come from.
#[derive(Clone)]
enum Source {
    Var(String),
    /// A STRING of the report, by index.
    Str(usize),
    Dummy,
}

struct Column {
    /// The variable and those stacked below it, each with LABEL.
    sources: Vec<(Source, bool)>,
    name: String,
    heading: Option<Vec<String>>,
    width: Option<u32>,
    offset: Option<ReportOffset>,
    align: Option<ReportAlign>,
}

/// One value of a case for display and for statistics.
enum Datum {
    Number(Option<f64>),
    Text(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Head,
    Case,
    Summary,
    Blank,
}

/// One line of the report: a cell for each break column, then each report column. A cell
/// holds its text and whether it is a number (aligned right).
struct Line {
    kind: Kind,
    cells: Vec<Option<(String, bool)>>,
}

fn function_title(f: &ReportFunction) -> String {
    let n = |x: f64| {
        format_value(x, "F40.2", false)
            .trim()
            .trim_end_matches(".00")
            .to_string()
    };
    match f {
        ReportFunction::ValidN => "N".into(),
        ReportFunction::Variance => "Variance".into(),
        ReportFunction::Sum => "Sum".into(),
        ReportFunction::Mean => "Mean".into(),
        ReportFunction::StdDev => "StdDev".into(),
        ReportFunction::Min => "Minimum".into(),
        ReportFunction::Max => "Maximum".into(),
        ReportFunction::Skewness => "Skewness".into(),
        ReportFunction::Kurtosis => "Kurtosis".into(),
        ReportFunction::Pgt(x) => format!(">{}", n(*x)),
        ReportFunction::Plt(x) => format!("<{}", n(*x)),
        ReportFunction::Pin(a, b) => format!("In {} to {}", n(*a), n(*b)),
        ReportFunction::Frequency(..) | ReportFunction::Percent(..) => "Total".into(),
        ReportFunction::Median(..) => "Median".into(),
        ReportFunction::Mode(..) => "Mode".into(),
    }
}

fn composite_title(f: Composite) -> &'static str {
    match f {
        Composite::Divide => "Divide",
        Composite::Multiply => "Multiply",
        Composite::Pct => "Pct",
        Composite::Subtract => "Subtract",
        Composite::Add => "Add",
        Composite::Great => "Great",
        Composite::Least => "Least",
        Composite::Average => "Average",
    }
}

/// Splits a format such as "DOLLAR10.2" into type, width and decimals.
fn parts(format: &str) -> (String, u32, u32) {
    let letters = format
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(format.len());
    let (kind, size) = format.split_at(letters);
    let (w, d) = size.split_once('.').unwrap_or((size, "0"));
    (
        kind.to_string(),
        w.parse().unwrap_or(8),
        d.parse().unwrap_or(0),
    )
}

/// The format of a function's result, from the dictionary format `dict` of its variable and
/// the format the summary gives.
fn result_format(
    function: Option<&ReportFunction>,
    composite: Option<Composite>,
    dict: &str,
    given: &SummaryFormat,
) -> String {
    let (dict_kind, dict_width, dict_decimals) = parts(dict);
    let (kind, width, decimals) = match (function, composite) {
        (_, Some(Composite::Pct)) => ("PCT".to_string(), 6, 2),
        (_, Some(_)) => ("F".to_string(), dict_width, 0),
        (Some(f), None) => match f {
            ReportFunction::ValidN | ReportFunction::Frequency(..) => ("F".into(), 5, 0),
            ReportFunction::Skewness | ReportFunction::Kurtosis => ("F".into(), 5, 2),
            ReportFunction::Pgt(_) | ReportFunction::Plt(_) | ReportFunction::Pin(..) => {
                ("PCT".into(), 6, 1)
            }
            ReportFunction::Percent(..) => ("F".into(), 6, 1),
            ReportFunction::Sum => (dict_kind, dict_width + 2, dict_decimals),
            _ => (dict_kind, dict_width, dict_decimals),
        },
        (None, None) => (dict_kind, dict_width, dict_decimals),
    };
    let (mut kind, mut width, mut decimals) = (kind, width, decimals);
    if given.plain {
        kind = "F".into();
    }
    if let Some(k) = given.kind {
        kind = k.name().to_string();
    }
    if let Some(f) = given.format {
        kind = f.kind.name().to_string();
        width = u32::from(f.width);
        decimals = u32::from(f.decimals);
    }
    if let Some(d) = given.decimals {
        decimals = u32::from(d);
        width = width.max(decimals + 2);
    }
    if kind.is_empty() || kind == "A" {
        kind = "F".into();
    }
    format!("{kind}{width}.{decimals}")
}

/// The statistics of one variable in one group: valid values with their weights.
struct Values2 {
    cells: Vec<(f64, f64)>,
    /// Valid cases of a string variable, by weight.
    strings: f64,
    string: bool,
}

/// Weighted values cut to whole numbers that lie in [min, max], sorted.
fn bins(cells: &[(f64, f64)], min: f64, max: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut whole: Vec<(f64, f64)> = cells
        .iter()
        .map(|(x, w)| (x.trunc(), *w))
        .filter(|(x, w)| *x >= min && *x <= max && *w > 0.0)
        .collect();
    whole.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (x, w) in whole {
        match out.last_mut() {
            Some(last) if last.0 == x => last.1 += w,
            _ => out.push((x, w)),
        }
    }
    out
}

fn aggregate(f: &ReportFunction, v: &Values2) -> Result<Option<f64>, String> {
    if v.string {
        return match f {
            ReportFunction::ValidN => Ok(Some(v.strings)),
            _ => Err("only VALIDN works on a string variable".into()),
        };
    }
    let total: f64 = v.cells.iter().map(|(_, w)| w).sum();
    let share = |keep: &dyn Fn(f64) -> bool| -> Option<f64> {
        (total > 0.0).then(|| {
            100.0
                * v.cells
                    .iter()
                    .filter(|(x, _)| keep(*x))
                    .map(|(_, w)| w)
                    .sum::<f64>()
                / total
        })
    };
    let s = summarize(&v.cells);
    Ok(match f {
        ReportFunction::ValidN => Some(total),
        ReportFunction::Sum => s.sum.filter(|_| !v.cells.is_empty()),
        ReportFunction::Min => s.min,
        ReportFunction::Max => s.max,
        ReportFunction::Mean => s.mean,
        ReportFunction::StdDev => s.sd,
        ReportFunction::Variance => s.sd.map(|sd| sd * sd),
        ReportFunction::Kurtosis => s.kurtosis,
        ReportFunction::Skewness => s.skewness,
        ReportFunction::Pgt(n) => share(&|x| x > *n),
        ReportFunction::Plt(n) => share(&|x| x < *n),
        ReportFunction::Pin(a, b) => share(&|x| x >= *a && x <= *b),
        ReportFunction::Median(a, b) => {
            let bins = bins(&v.cells, *a, *b);
            let total: f64 = bins.iter().map(|(_, w)| w).sum();
            let mut cumulative = 0.0;
            let mut median = None;
            for (i, (x, w)) in bins.iter().enumerate() {
                cumulative += w;
                if cumulative >= total / 2.0 - 1e-9 * total {
                    let even = (cumulative - total / 2.0).abs() <= 1e-9 * total;
                    median = match bins.get(i + 1) {
                        Some((next, _)) if even => Some((x + next) / 2.0),
                        _ => Some(*x),
                    };
                    break;
                }
            }
            median
        }
        ReportFunction::Mode(a, b) => bins(&v.cells, *a, *b)
            .into_iter()
            .fold(None, |best: Option<(f64, f64)>, (x, w)| match best {
                Some((_, bw)) if bw >= w => best,
                _ => Some((x, w)),
            })
            .map(|(x, _)| x),
        ReportFunction::Frequency(..) | ReportFunction::Percent(..) => None,
    })
}

fn composite(f: Composite, args: &[Option<f64>]) -> Option<f64> {
    let args: Vec<f64> = args.iter().copied().collect::<Option<_>>()?;
    let value = match f {
        Composite::Divide => {
            let factor = args.get(2).copied().unwrap_or(1.0);
            (args[1] != 0.0).then(|| args[0] / args[1] * factor)?
        }
        Composite::Pct => (args[1] != 0.0).then(|| 100.0 * args[0] / args[1])?,
        Composite::Subtract => args[0] - args[1],
        Composite::Multiply => args.iter().product(),
        Composite::Add => args.iter().sum(),
        Composite::Great => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        Composite::Least => args.iter().copied().fold(f64::INFINITY, f64::min),
        Composite::Average => args.iter().sum::<f64>() / args.len() as f64,
    };
    value.is_finite().then_some(value)
}

/// The value label of `value`, if the variable has one.
fn label(var: &Variable, value: &Value) -> Option<String> {
    var.value_labels()
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| l.clone())
}

/// A numeric part of a STRING: in `width` characters, as few decimals dropped as needed, padded
/// with zeros (or blanks).
fn padded_number(x: f64, format: &str, width: usize, blank: bool) -> String {
    let (kind, _, decimals) = parts(format);
    let blank = blank || kind == "DOLLAR" || kind == "COMMA";
    // the value as its format shows it, else without decimals
    for d in [decimals, 0] {
        let text = format_value(x, &format!("{kind}40.{d}"), false);
        let text = text.trim();
        if text.chars().count() <= width {
            let pad = width - text.chars().count();
            return match (blank, text.strip_prefix('-')) {
                (true, _) => format!("{}{text}", " ".repeat(pad)),
                (false, Some(digits)) => format!("-{}{digits}", "0".repeat(pad)),
                (false, None) => format!("{}{text}", "0".repeat(pad)),
            };
        }
    }
    "*".repeat(width)
}

struct Ctx<'a> {
    data: &'a Dataset,
    report: &'a Report,
    columns: Vec<Column>,
    /// The values of each STRING for every case.
    strings: Vec<(String, usize, Vec<String>)>,
    weights: Option<Vec<f64>>,
    include_user_missing: bool,
    missing_symbol: String,
    currency: Box<dyn Fn(&str) -> String + 'a>,
}

impl Ctx<'_> {
    fn datum(&self, source: &Source, case: usize) -> Datum {
        match source {
            Source::Var(name) => {
                let var = self.data.variable(name).expect("checked");
                match var.values() {
                    Values::Numeric(v) => Datum::Number(v.get(case)),
                    Values::String(v) => Datum::Text(v.get(case).to_string()),
                }
            }
            Source::Str(i) => Datum::Text(self.strings[*i].2[case].clone()),
            Source::Dummy => Datum::Text(String::new()),
        }
    }

    /// The text of a value in a listing or a break head: the label with `labels`, else the
    /// value in its print format; and whether it is a number.
    fn shown(&self, source: &Source, case: usize, labels: bool) -> (String, bool) {
        let Source::Var(name) = source else {
            return (self.datum(source, case).text(), false);
        };
        let var = self.data.variable(name).expect("checked");
        match var.values() {
            Values::Numeric(v) => match v.get(case) {
                None => (self.missing_symbol.clone(), true),
                Some(x) => {
                    if labels && let Some(l) = label(&var, &Value::Number(x)) {
                        return (l, false);
                    }
                    let format = (self.currency)(var.format());
                    (format_value(x, &format, true).trim().to_string(), true)
                }
            },
            Values::String(v) => {
                let text = v.get(case).trim_end().to_string();
                if labels && let Some(l) = label(&var, &Value::String(text.clone())) {
                    return (l, false);
                }
                (text, false)
            }
        }
    }

    fn is_missing(&self, source: &Source, case: usize) -> bool {
        let Source::Var(name) = source else {
            return false;
        };
        let var = self.data.variable(name).expect("checked");
        match var.values() {
            Values::Numeric(v) => match v.get(case) {
                None => true,
                Some(x) => !self.include_user_missing && var.is_user_missing(x),
            },
            Values::String(v) => !self.include_user_missing && var.is_user_missing_str(v.get(case)),
        }
    }

    fn weight(&self, case: usize) -> f64 {
        self.weights.as_ref().map_or(1.0, |w| w[case])
    }

    fn values(&self, source: &Source, cases: &[usize]) -> Values2 {
        let mut out = Values2 {
            cells: Vec::new(),
            strings: 0.0,
            string: false,
        };
        for &case in cases {
            let w = self.weight(case);
            if w <= 0.0 || self.is_missing(source, case) {
                continue;
            }
            match self.datum(source, case) {
                Datum::Number(Some(x)) => out.cells.push((x, w)),
                Datum::Number(None) => {}
                Datum::Text(_) => {
                    out.string = true;
                    out.strings += w;
                }
            }
        }
        if matches!(source, Source::Str(_) | Source::Dummy) {
            out.string = true;
        }
        out
    }

    fn dict_format(&self, source: &Source) -> String {
        match source {
            Source::Var(name) => {
                (self.currency)(self.data.variable(name).expect("checked").format())
            }
            _ => "F8.2".into(),
        }
    }

    fn source(&self, name: &str) -> Result<Source, Error> {
        if let Some(i) = self
            .strings
            .iter()
            .position(|s| s.0.eq_ignore_ascii_case(name))
        {
            return Ok(Source::Str(i));
        }
        let var = self
            .data
            .variable(name)
            .map_err(|_| Error(format!("REPORT: unknown variable {name}")))?;
        Ok(Source::Var(var.name().to_string()))
    }

    /// The report column a variable named on SUMMARY stands in.
    fn column_of(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }
}

impl Datum {
    fn text(&self) -> String {
        match self {
            Datum::Number(Some(x)) => format_value(*x, "F8.2", true).trim().to_string(),
            Datum::Number(None) => String::new(),
            Datum::Text(t) => t.trim_end().to_string(),
        }
    }
}

struct Level {
    sources: Vec<Source>,
    labels: bool,
    name: bool,
    /// The break column, None for a BREAK without variables.
    column: Option<usize>,
    /// The levels of ONEBREAKCOLUMN indent their values.
    indent: usize,
    summaries: Vec<SummaryLine>,
    skip: u32,
}

#[derive(Clone)]
struct SummaryLine {
    items: Vec<SummaryItem>,
    title: Option<String>,
    title_column: Option<u32>,
    skip: u32,
}

impl Engine {
    pub(crate) fn report(&mut self, report: &Report) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let format = &report.format;
        let automatic = format.automatic;
        // STRING variables
        let mut strings: Vec<(String, usize, Vec<String>)> = Vec::new();
        for s in &report.strings {
            if data.variable_index(&s.name).is_some()
                || strings
                    .iter()
                    .any(|(n, _, _)| n.eq_ignore_ascii_case(&s.name))
            {
                return Err(Error(format!(
                    "REPORT /STRING: the name {} is taken; give the string a new name",
                    s.name
                )));
            }
            let mut width = 0;
            let mut values = vec![String::new(); data.case_count()];
            for part in &s.parts {
                match part {
                    StringPart::Literal(text) => {
                        width += text.chars().count();
                        values.iter_mut().for_each(|v| v.push_str(text));
                    }
                    StringPart::Variable {
                        name,
                        width: given,
                        blank,
                    } => {
                        let var = data.variable(name).map_err(|_| {
                            Error(format!("REPORT /STRING: unknown variable {name}"))
                        })?;
                        let (_, dict_width, _) = parts(var.format());
                        let w = given.map_or_else(
                            || var.string_width().unwrap_or(dict_width as usize),
                            |w| w as usize,
                        );
                        let w = if var.is_string() { w } else { w.min(16) };
                        width += w;
                        for (case, value) in values.iter_mut().enumerate() {
                            let piece = match var.values() {
                                Values::String(v) => {
                                    let text: String = v.get(case).chars().take(w).collect();
                                    format!("{text:<w$}")
                                }
                                Values::Numeric(v) => match v.get(case) {
                                    None => " ".repeat(w),
                                    Some(x) => padded_number(x, var.format(), w, *blank),
                                },
                            };
                            value.push_str(&piece);
                        }
                    }
                }
            }
            strings.push((s.name.clone(), width, values));
        }
        let currency = |f: &str| self.currency_format(f);
        let mut ctx = Ctx {
            data,
            report,
            columns: Vec::new(),
            strings,
            weights: case_weights(data)?,
            include_user_missing: matches!(report.missing, ReportMissing::None),
            missing_symbol: format.missing.to_string(),
            currency: Box::new(currency),
        };
        // the report columns
        for spec in &report.columns {
            let names: Vec<Vec<(String, ColumnContents)>> = match &spec.variables[..] {
                [(VarItem::Range(..) | VarItem::All, contents)] => self
                    .expand(&[spec.variables[0].0.clone()])?
                    .into_iter()
                    .map(|n| vec![(n, *contents)])
                    .collect(),
                stacked => vec![
                    stacked
                        .iter()
                        .map(|(item, contents)| match item {
                            VarItem::Name(n) => Ok((n.clone(), *contents)),
                            _ => Err(Error(
                                "REPORT /VARIABLES: a stacked column takes single variable names"
                                    .into(),
                            )),
                        })
                        .collect::<Result<_, _>>()?,
                ],
            };
            for stack in names {
                if stack.len() > 1 && format.list.is_none() {
                    return Err(Error(
                        "REPORT: variables stack in one column (with +) only with FORMAT=LIST"
                            .into(),
                    ));
                }
                let mut sources = Vec::new();
                for (name, contents) in &stack {
                    let source = if *contents == ColumnContents::Dummy {
                        if ctx.data.variable_index(name).is_some() {
                            return Err(Error(format!(
                                "REPORT: {name} is a variable of the dataset, so it cannot be a \
                                 DUMMY column"
                            )));
                        }
                        Source::Dummy
                    } else {
                        ctx.source(name)?
                    };
                    sources.push((source, *contents == ColumnContents::Label));
                }
                ctx.columns.push(Column {
                    sources,
                    name: stack[0].0.clone(),
                    heading: spec.heading.clone(),
                    width: spec.width,
                    offset: spec.offset,
                    align: spec.align,
                });
            }
        }
        // the break levels
        let mut levels: Vec<Level> = Vec::new();
        let mut break_columns = 0;
        for b in &report.breaks {
            let mut sources = Vec::new();
            for name in self.expand_or_strings(&b.variables, &ctx)? {
                sources.push(ctx.source(&name)?);
            }
            let column = (!sources.is_empty()).then(|| {
                let at = if format.one_break_column {
                    0
                } else {
                    break_columns
                };
                break_columns = at + 1;
                at
            });
            levels.push(Level {
                indent: if format.one_break_column {
                    levels.iter().filter(|l| l.column.is_some()).count() * format.indent as usize
                } else {
                    0
                },
                sources,
                labels: b.label.unwrap_or(automatic),
                name: b.name,
                column,
                summaries: Vec::new(),
                skip: b.skip,
            });
        }
        for (i, b) in report.breaks.iter().enumerate() {
            let mut lines = Vec::new();
            for s in &b.summaries {
                match s {
                    ReportSummary::Line {
                        items,
                        title,
                        title_column,
                        skip,
                    } => lines.push(SummaryLine {
                        items: items.clone(),
                        title: title.clone(),
                        title_column: *title_column,
                        skip: *skip,
                    }),
                    ReportSummary::Previous(n) => {
                        let target = n.map_or(i - 1, |n| n as usize - 1);
                        lines.extend(levels[target].summaries.clone());
                    }
                }
            }
            levels[i].summaries = lines;
        }
        let labels_column = break_columns == 0 && levels.iter().any(|l| !l.summaries.is_empty());
        let break_count = if labels_column { 1 } else { break_columns };
        // MISSING=LIST drops cases
        let mut cases: Vec<usize> = (0..ctx.data.case_count()).collect();
        if let ReportMissing::List(vars, n) = &report.missing {
            let sources: Vec<Source> = if vars.is_empty() {
                ctx.columns
                    .iter()
                    .flat_map(|c| c.sources.iter().map(|(s, _)| s.clone()))
                    .collect()
            } else {
                self.expand_or_strings(vars, &ctx)?
                    .iter()
                    .map(|n| ctx.source(n))
                    .collect::<Result<_, _>>()?
            };
            cases.retain(|&case| {
                sources.iter().filter(|s| ctx.is_missing(s, case)).count() < *n as usize
            });
        }
        let total_line = report
            .breaks
            .first()
            .filter(|b| b.total && !b.variables.is_empty());
        let width = break_count + ctx.columns.len();
        let mut lines: Vec<Line> = Vec::new();
        let blank = |lines: &mut Vec<Line>, n: u32| {
            for _ in 0..n {
                lines.push(Line {
                    kind: Kind::Blank,
                    cells: vec![None; width],
                });
            }
        };
        let brkspace = format.brkspace.unwrap_or(if automatic { -1 } else { 1 });
        // the key of a case at each level: the values of its variables and of all before
        let key = |case: usize, level: usize| -> Vec<String> {
            levels[..=level]
                .iter()
                .flat_map(|l| &l.sources)
                .map(|s| match ctx.datum(s, case) {
                    Datum::Number(n) => format!("{n:?}"),
                    Datum::Text(t) => t,
                })
                .collect()
        };
        let mut groups: Vec<Vec<usize>> = vec![Vec::new(); levels.len()];
        let mut merge: Option<usize> = None;
        let mut listed = 0usize;
        let push = |lines: &mut Vec<Line>, merge: &mut Option<usize>, line: Line| {
            if let Some(at) = merge.take() {
                let head = &mut lines[at];
                for (c, cell) in line.cells.into_iter().enumerate() {
                    // the head keeps its break values; a summary title there is left out
                    if c >= break_count && cell.is_some() {
                        head.cells[c] = cell;
                    }
                }
                head.kind = line.kind;
                return;
            }
            lines.push(line);
        };
        for (pos, &case) in cases.iter().enumerate() {
            let changed = match pos {
                0 => Some(0),
                _ => (0..levels.len()).find(|&l| key(case, l) != key(cases[pos - 1], l)),
            };
            if let Some(from) = changed {
                if pos > 0 {
                    // close the groups from the innermost out
                    for level in (from..levels.len()).rev() {
                        let cases = std::mem::take(&mut groups[level]);
                        let summary = self.summary_lines(
                            &ctx,
                            &levels,
                            level,
                            &cases,
                            break_count,
                            &mut lines,
                            &mut merge,
                        )?;
                        if summary {
                            blank(&mut lines, levels[level].skip);
                        }
                    }
                }
                // the heads of the groups that start, each variable on a line of its own
                let opened: Vec<&Level> = levels[from..]
                    .iter()
                    .filter(|l| l.column.is_some())
                    .collect();
                let height = opened.iter().map(|l| l.sources.len()).max().unwrap_or(0);
                for row in 0..height {
                    let mut cells = vec![None; width];
                    for level in &opened {
                        let Some(source) = level.sources.get(row) else {
                            continue;
                        };
                        let (mut text, number) = ctx.shown(source, case, level.labels);
                        if level.name
                            && let Source::Var(name) = source
                        {
                            text = format!("{name}: {text}");
                        }
                        let text = format!("{}{text}", " ".repeat(level.indent));
                        cells[level.column.expect("opened")] = Some((text, number));
                    }
                    lines.push(Line {
                        kind: Kind::Head,
                        cells,
                    });
                }
                if height > 0 {
                    if brkspace < 0 {
                        merge = Some(lines.len() - 1);
                    } else {
                        blank(&mut lines, brkspace as u32);
                    }
                }
                listed = 0;
            }
            for group in &mut groups {
                group.push(case);
            }
            if let Some(every) = format.list {
                let height = ctx
                    .columns
                    .iter()
                    .map(|c| c.sources.len())
                    .max()
                    .unwrap_or(1);
                for row in 0..height {
                    let mut cells = vec![None; width];
                    for (c, column) in ctx.columns.iter().enumerate() {
                        if let Some((source, labels)) = column.sources.get(row) {
                            cells[break_count + c] = Some(ctx.shown(source, case, *labels));
                        }
                    }
                    push(
                        &mut lines,
                        &mut merge,
                        Line {
                            kind: Kind::Case,
                            cells,
                        },
                    );
                }
                listed += 1;
                if every > 0 && listed.is_multiple_of(every as usize) {
                    blank(&mut lines, 1);
                }
            }
        }
        if !cases.is_empty() {
            for level in (0..levels.len()).rev() {
                let cases = std::mem::take(&mut groups[level]);
                if self.summary_lines(
                    &ctx,
                    &levels,
                    level,
                    &cases,
                    break_count,
                    &mut lines,
                    &mut merge,
                )? {
                    blank(&mut lines, levels[level].skip);
                }
            }
            if total_line.is_some() {
                let mut cells = vec![None; width];
                cells[0] = Some(("Grand Total".to_string(), false));
                // a title like a summary's: it runs on to the right
                lines.push(Line {
                    kind: Kind::Summary,
                    cells,
                });
                let mut total = Level {
                    sources: Vec::new(),
                    labels: false,
                    name: false,
                    column: Some(0),
                    indent: 0,
                    summaries: levels[0].summaries.iter().take(1).cloned().collect(),
                    skip: 0,
                };
                total
                    .summaries
                    .iter_mut()
                    .for_each(|s| s.title_column = Some(1));
                self.summary_lines(
                    &ctx,
                    std::slice::from_ref(&total),
                    0,
                    &cases,
                    break_count,
                    &mut lines,
                    &mut merge,
                )?;
            }
        }
        while lines.last().is_some_and(|l| l.kind == Kind::Blank) {
            lines.pop();
        }
        // titles, footnotes and headings
        let title_lines = self.report_titles(&ctx, &report.titles, cases.first().copied(), true);
        let footnote_lines =
            self.report_titles(&ctx, &report.footnotes, cases.first().copied(), false);
        let mut headings: Vec<Vec<String>> = Vec::new();
        for level in levels.iter().filter(|l| l.column.is_some()) {
            if format.one_break_column && !headings.is_empty() {
                break;
            }
            let b = &report.breaks[levels.iter().position(|l| std::ptr::eq(l, level)).unwrap()];
            headings.push(
                b.heading
                    .clone()
                    .unwrap_or_else(|| vec![self.default_heading(&ctx, &level.sources[0])]),
            );
        }
        if labels_column {
            headings.push(vec![String::new()]);
        }
        for column in &ctx.columns {
            headings.push(
                column
                    .heading
                    .clone()
                    .unwrap_or_else(|| match &column.sources[0].0 {
                        Source::Dummy => vec![column.name.clone()],
                        source => vec![self.default_heading(&ctx, source)],
                    }),
            );
        }
        if let Some(file) = &report.outfile {
            let text = self.report_text(
                &ctx,
                &levels,
                &headings,
                &lines,
                break_count,
                &title_lines,
                &footnote_lines,
            );
            drop(ctx);
            if !self.dry_run {
                let path = self.path(file);
                let append = self.documents.report_files.contains(&path);
                let mut handle = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(append)
                    .truncate(!append)
                    .open(&path)
                    .map_err(|e| Error(format!("REPORT: cannot write {file}: {e}")))?;
                handle
                    .write_all(text.as_bytes())
                    .map_err(|e| Error(format!("REPORT: cannot write {file}: {e}")))?;
                self.documents.report_files.insert(path);
            }
            return Ok(Vec::new());
        }
        let rows: Vec<Vec<Cell>> = lines
            .iter()
            .filter(|l| l.kind != Kind::Blank)
            .map(|l| {
                l.cells
                    .iter()
                    .map(|c| match c {
                        Some((text, _)) if !text.is_empty() => Cell::Text(text.clone()),
                        _ => Cell::Empty,
                    })
                    .collect()
            })
            .collect();
        let title = if title_lines.is_empty() {
            "Report".to_string()
        } else {
            title_lines
                .iter()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let caption = (!footnote_lines.is_empty()).then(|| {
            footnote_lines
                .iter()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(" ")
        });
        Ok(vec![Table {
            title,
            row_headers: break_count,
            columns: headings
                .iter()
                .map(|h| h.join(" ").trim().to_string())
                .collect(),
            rows,
            caption,
            ..Table::default()
        }])
    }

    /// Variable names of a BREAK or MISSING list: STRING names as they are, the others
    /// expanded against the dataset.
    fn expand_or_strings(&self, items: &[VarItem], ctx: &Ctx) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        for item in items {
            match item {
                VarItem::Name(n) if ctx.strings.iter().any(|s| s.0.eq_ignore_ascii_case(n)) => {
                    out.push(n.clone())
                }
                _ => out.extend(self.expand(std::slice::from_ref(item))?),
            }
        }
        Ok(out)
    }

    fn default_heading(&self, ctx: &Ctx, source: &Source) -> String {
        match source {
            Source::Var(name) => {
                let var = ctx.data.variable(name).expect("checked");
                var.label().unwrap_or(var.name()).to_string()
            }
            Source::Str(i) => ctx.strings[*i].0.clone(),
            Source::Dummy => String::new(),
        }
    }

    /// The summary lines of `level` for the group of `cases`; false when it has none.
    #[allow(clippy::too_many_arguments)]
    fn summary_lines(
        &self,
        ctx: &Ctx,
        levels: &[Level],
        level: usize,
        cases: &[usize],
        break_count: usize,
        lines: &mut Vec<Line>,
        merge: &mut Option<usize>,
    ) -> Result<bool, Error> {
        let width = break_count + ctx.columns.len();
        let summaries = &levels[level].summaries;
        for (i, summary) in summaries.iter().enumerate() {
            // SUMSPACE parts the listed cases from the first summary line
            let after_list = i == 0 && ctx.report.format.list.is_some();
            let skip = summary.skip
                + if after_list {
                    ctx.report.format.sumspace
                } else {
                    0
                };
            if merge.is_none() {
                for _ in 0..skip {
                    lines.push(Line {
                        kind: Kind::Blank,
                        cells: vec![None; width],
                    });
                }
            }
            let title_at = match summary.title_column {
                Some(n) if (n as usize) <= break_count => n as usize - 1,
                _ => levels[level]
                    .column
                    .unwrap_or(0)
                    .min(break_count.saturating_sub(1)),
            };
            let first_title = match summary.items.first() {
                Some(SummaryItem::Aggregate { function, .. }) => function_title(function),
                Some(SummaryItem::Composite { function, .. }) => {
                    composite_title(*function).to_string()
                }
                None => String::new(),
            };
            let title = summary.title.clone().unwrap_or(first_title);
            // FREQUENCY and PERCENT: a line per value of the range, then the total
            if let [
                SummaryItem::Aggregate {
                    function:
                        f @ (ReportFunction::Frequency(min, max) | ReportFunction::Percent(min, max)),
                    variables,
                },
            ] = &summary.items[..]
            {
                let percent = matches!(f, ReportFunction::Percent(..));
                let targets = self.summary_targets(ctx, variables)?;
                let mut value = min.trunc();
                let mut totals = vec![0.0; targets.len()];
                let format_for =
                    |given: &SummaryFormat| result_format(Some(f), None, "F8.0", given);
                while value <= *max {
                    let mut cells = vec![None; width];
                    if break_count > 0 {
                        cells[title_at] = Some((
                            format_value(value, "F40.0", false).trim().to_string(),
                            false,
                        ));
                    }
                    for (t, (column, source, given)) in targets.iter().enumerate() {
                        let v = ctx.values(source, cases);
                        if v.string {
                            return Err(Error(
                                "REPORT: only VALIDN works on a string variable".into(),
                            ));
                        }
                        let all: f64 = v.cells.iter().map(|(_, w)| w).sum();
                        let count: f64 = bins(&v.cells, value, value).iter().map(|(_, w)| w).sum();
                        let shown = if percent {
                            (all > 0.0).then(|| 100.0 * count / all)
                        } else {
                            Some(count)
                        };
                        totals[t] += shown.unwrap_or(0.0);
                        cells[break_count + column] =
                            Some(number_cell(shown, &format_for(given), ctx));
                    }
                    lines.push(Line {
                        kind: Kind::Summary,
                        cells,
                    });
                    value += 1.0;
                }
                let mut cells = vec![None; width];
                if break_count > 0 {
                    cells[title_at] = Some((title.clone(), false));
                }
                for (t, (column, _, given)) in targets.iter().enumerate() {
                    cells[break_count + column] =
                        Some(number_cell(Some(totals[t]), &format_for(given), ctx));
                }
                lines.push(Line {
                    kind: Kind::Summary,
                    cells,
                });
                continue;
            }
            let mut cells: Vec<Option<(String, bool)>> = vec![None; width];
            let mut put = |column: usize, cell: (String, bool)| -> Result<(), Error> {
                let at = break_count + column;
                if cells[at].is_some() {
                    return Err(Error(format!(
                        "REPORT: two results of one SUMMARY line go in the column of {}; give \
                         them SUMMARY lines of their own",
                        ctx.columns[column].name
                    )));
                }
                cells[at] = Some(cell);
                Ok(())
            };
            for item in &summary.items {
                match item {
                    SummaryItem::Aggregate {
                        function,
                        variables,
                    } => {
                        for (column, source, given) in self.summary_targets(ctx, variables)? {
                            if matches!(source, Source::Dummy) {
                                continue;
                            }
                            let v = ctx.values(&source, cases);
                            let value = aggregate(function, &v).map_err(|e| {
                                Error(format!("REPORT: {}: {e}", ctx.columns[column].name))
                            })?;
                            let format = result_format(
                                Some(function),
                                None,
                                &ctx.dict_format(&source),
                                &given,
                            );
                            put(column, number_cell(value, &format, ctx))?;
                        }
                    }
                    SummaryItem::Composite {
                        function,
                        args,
                        column,
                    } => {
                        let mut values = Vec::new();
                        let mut first_var = None;
                        for arg in args {
                            match arg {
                                CompositeArg::Constant(x) => values.push(Some(*x)),
                                CompositeArg::Aggregate(f, name) => {
                                    let source = ctx.source(name)?;
                                    if first_var.is_none() && ctx.column_of(name).is_some() {
                                        first_var = Some(name.clone());
                                    }
                                    let v = ctx.values(&source, cases);
                                    values.push(
                                        aggregate(f, &v)
                                            .map_err(|e| Error(format!("REPORT: {name}: {e}")))?,
                                    );
                                }
                            }
                        }
                        let (target, given) = match column {
                            Some((name, given)) => (name.clone(), given.clone()),
                            None => match first_var {
                                Some(name) => (name, SummaryFormat::default()),
                                None => {
                                    return Err(Error(format!(
                                        "REPORT: {} has no report variable among its arguments; \
                                         give the column in parentheses after it",
                                        composite_title(*function).to_uppercase()
                                    )));
                                }
                            },
                        };
                        let at = ctx.column_of(&target).ok_or_else(|| {
                            Error(format!("REPORT: {target} is no report column"))
                        })?;
                        let dict = match &ctx.columns[at].sources[0].0 {
                            source @ Source::Var(_) => ctx.dict_format(source),
                            _ => format!("F{}.0", ctx.columns[at].width.unwrap_or(8)),
                        };
                        let format = result_format(None, Some(*function), &dict, &given);
                        put(at, number_cell(composite(*function, &values), &format, ctx))?;
                    }
                }
            }
            if break_count > 0 && merge.is_none() {
                cells[title_at] = Some((title, false));
            }
            let line = Line {
                kind: Kind::Summary,
                cells,
            };
            if let Some(at) = merge.take() {
                let head = &mut lines[at];
                for (c, cell) in line.cells.into_iter().enumerate() {
                    if c >= break_count && cell.is_some() {
                        head.cells[c] = cell;
                    }
                }
            } else {
                lines.push(line);
            }
        }
        Ok(!summaries.is_empty())
    }

    /// The report columns an aggregate goes in: those named, else every report column.
    fn summary_targets(
        &self,
        ctx: &Ctx,
        variables: &[(String, SummaryFormat)],
    ) -> Result<Vec<(usize, Source, SummaryFormat)>, Error> {
        if variables.is_empty() {
            return Ok(ctx
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| (i, c.sources[0].0.clone(), SummaryFormat::default()))
                .collect());
        }
        variables
            .iter()
            .map(|(name, given)| {
                let at = ctx.column_of(name).ok_or_else(|| {
                    Error(format!("REPORT /SUMMARY: {name} is not named on VARIABLES"))
                })?;
                Ok((at, ctx.columns[at].sources[0].0.clone(), given.clone()))
            })
            .collect()
    }

    /// The lines of TITLE or FOOTNOTE as text: LEFT, CENTER and RIGHT parts of each line
    /// joined with blanks. The TITLE command's text when TITLE is not given.
    fn report_titles(
        &self,
        ctx: &Ctx,
        groups: &[(ReportAlign, Vec<String>)],
        first: Option<usize>,
        title: bool,
    ) -> Vec<String> {
        if groups.is_empty() {
            return match (&self.documents.title, title) {
                (Some(text), true) => vec![text.clone()],
                _ => Vec::new(),
            };
        }
        let height = groups.iter().map(|(_, l)| l.len()).max().unwrap_or(0);
        let order = [ReportAlign::Left, ReportAlign::Center, ReportAlign::Right];
        (0..height)
            .map(|row| {
                let parts: Vec<String> = order
                    .iter()
                    .filter_map(|a| groups.iter().find(|(g, _)| g == a))
                    .filter_map(|(_, lines)| lines.get(row))
                    .map(|l| self.title_text(ctx, l, first))
                    .collect();
                parts.join("    ")
            })
            .collect()
    }

    /// )PAGE, )DATE and )var in a title line.
    fn title_text(&self, ctx: &Ctx, line: &str, first: Option<usize>) -> String {
        let mut out = String::new();
        let mut rest = line;
        while let Some(at) = rest.find(')') {
            out.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let word: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || "_@#$.".contains(*c))
                .collect();
            let word = word.trim_end_matches('.').to_string();
            if word.eq_ignore_ascii_case("PAGE") {
                out.push_str(&format!("{:>5}", ctx.report.format.page1));
            } else if word.eq_ignore_ascii_case("DATE") {
                let date = chrono::Local::now()
                    .format("%d/%b/%y")
                    .to_string()
                    .to_uppercase();
                out.push_str(&format!("{date:>9}"));
            } else if let (Ok(_), Some(case)) = (ctx.data.variable(&word), first) {
                let source = Source::Var(
                    ctx.data
                        .variable(&word)
                        .expect("checked")
                        .name()
                        .to_string(),
                );
                out.push_str(&ctx.shown(&source, case, true).0);
            } else {
                out.push(')');
                rest = after;
                continue;
            }
            rest = &after[word.len()..];
        }
        out.push_str(rest);
        out
    }

    /// The report as text, as OUTFILE writes it.
    #[allow(clippy::too_many_arguments)]
    fn report_text(
        &self,
        ctx: &Ctx,
        levels: &[Level],
        headings: &[Vec<String>],
        lines: &[Line],
        break_count: usize,
        titles: &[String],
        footnotes: &[String],
    ) -> String {
        let format = &ctx.report.format;
        let automatic = format.automatic;
        let count = headings.len();
        // the widths
        let mut widths = vec![0usize; count];
        let longest_word = |text: &str| {
            text.split_whitespace()
                .map(|w| w.chars().count())
                .max()
                .unwrap_or(0)
        };
        for (c, heading) in headings.iter().enumerate() {
            let user = if c < break_count {
                levels.iter().filter(|l| l.column == Some(c)).any(|l| {
                    let i = levels.iter().position(|x| std::ptr::eq(x, l)).unwrap();
                    ctx.report.breaks[i].heading.is_some()
                })
            } else {
                ctx.columns[c - break_count].heading.is_some()
            };
            widths[c] = heading
                .iter()
                .map(|l| {
                    if user {
                        l.chars().count()
                    } else {
                        longest_word(l)
                    }
                })
                .max()
                .unwrap_or(0);
        }
        for line in lines {
            for (c, cell) in line.cells.iter().enumerate() {
                if let Some((text, _)) = cell
                    && !(line.kind == Kind::Summary && c < break_count)
                {
                    let number = matches!(cell, Some((_, true)));
                    let cap = if automatic || number { usize::MAX } else { 20 };
                    widths[c] = widths[c].max(text.chars().count().min(cap));
                }
            }
        }
        for (c, column) in ctx.columns.iter().enumerate() {
            let at = break_count + c;
            for (source, labels) in &column.sources {
                match source {
                    Source::Var(name) => {
                        let var = ctx.data.variable(name).expect("checked");
                        widths[at] = widths[at].max(parts(var.format()).1 as usize);
                        if *labels {
                            let longest = var
                                .value_labels()
                                .iter()
                                .map(|(_, l)| l.chars().count())
                                .max()
                                .unwrap_or(0);
                            widths[at] =
                                widths[at].max(if automatic { longest } else { longest.min(20) });
                        }
                    }
                    Source::Str(i) => widths[at] = widths[at].max(ctx.strings[*i].1),
                    Source::Dummy => {}
                }
            }
        }
        for (c, w) in widths.iter_mut().enumerate() {
            if !automatic {
                *w = (*w).max(8);
            }
            let given = if c < break_count {
                levels
                    .iter()
                    .position(|l| l.column == Some(c))
                    .and_then(|i| ctx.report.breaks[i].width)
            } else {
                ctx.columns[c - break_count].width
            };
            if let Some(given) = given {
                *w = given as usize;
            }
        }
        let space = format.colspace.unwrap_or(4) as usize;
        let total: usize = widths.iter().sum::<usize>() + space * count.saturating_sub(1);
        let starts: Vec<usize> = widths
            .iter()
            .scan(0, |at, w| {
                let start = *at;
                *at += w + space;
                Some(start)
            })
            .collect();
        let (left_margin, right_margin) = (
            format.margins.0.unwrap_or(1) as usize,
            format
                .margins
                .1
                .unwrap_or_else(|| self.setting("WIDTH").parse().unwrap_or(80))
                as usize,
        );
        let room = right_margin.saturating_sub(left_margin) + 1;
        let indent = left_margin - 1
            + match format.align {
                ReportAlign::Left => 0,
                ReportAlign::Center => room.saturating_sub(total) / 2,
                ReportAlign::Right => room.saturating_sub(total),
            };
        let centered = |text: &str, width: usize| {
            let n = text.chars().count();
            let left = width.saturating_sub(n) / 2;
            format!("{}{text}", " ".repeat(left))
        };
        let mut out = String::new();
        let emit = |out: &mut String, text: &str| {
            let line = format!("{}{}", " ".repeat(indent), text);
            out.push_str(line.trim_end());
            out.push('\n');
        };
        for title in titles {
            emit(&mut out, &centered(title, total));
        }
        if !titles.is_empty() {
            for _ in 0..format.tspace {
                emit(&mut out, "");
            }
        }
        // the column headings, wrapped to their columns
        let numeric_column = |c: usize| {
            lines.iter().any(|l| matches!(&l.cells[c], Some((_, true))))
                && !lines.iter().any(|l| {
                    l.kind != Kind::Summary
                        && matches!(&l.cells[c], Some((t, false)) if !t.is_empty())
                })
        };
        let wrapped: Vec<Vec<String>> = headings
            .iter()
            .zip(&widths)
            .map(|(lines, w)| lines.iter().flat_map(|l| wrap_text(l, *w)).collect())
            .collect();
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(0);
        let top = format.chalign_top.unwrap_or(!automatic);
        for row in 0..height {
            let mut buffer = vec![' '; total];
            for (c, lines) in wrapped.iter().enumerate() {
                let shift = if top { 0 } else { height - lines.len() };
                let Some(text) = row.checked_sub(shift).and_then(|r| lines.get(r)) else {
                    continue;
                };
                let align = if c >= break_count {
                    ctx.columns[c - break_count].align
                } else {
                    None
                };
                let align = align.unwrap_or(if automatic {
                    ReportAlign::Center
                } else if numeric_column(c) {
                    ReportAlign::Right
                } else {
                    ReportAlign::Left
                });
                place(&mut buffer, starts[c], widths[c], text, align, false);
            }
            emit(&mut out, &buffer.iter().collect::<String>());
        }
        if format.underscore.unwrap_or(automatic) {
            let mut buffer = vec![' '; total];
            for c in 0..count {
                buffer[starts[c]..starts[c] + widths[c]]
                    .iter_mut()
                    .for_each(|b| *b = '-');
            }
            emit(&mut out, &buffer.iter().collect::<String>());
        }
        for _ in 0..format.chdspace {
            emit(&mut out, "");
        }
        for line in lines {
            // a text cell wider than its column wraps; a summary title runs on to the right
            let wrapped: Vec<Vec<String>> = line
                .cells
                .iter()
                .enumerate()
                .map(|(c, cell)| match cell {
                    None => Vec::new(),
                    Some((text, _)) if line.kind == Kind::Summary && c < break_count => {
                        vec![text.clone()]
                    }
                    Some((text, true)) => vec![text.clone()],
                    Some((text, false)) => wrap_text(text, widths[c]),
                })
                .collect();
            let height = wrapped.iter().map(Vec::len).max().unwrap_or(0).max(1);
            for row in 0..height {
                let mut buffer = vec![' '; total];
                for (c, texts) in wrapped.iter().enumerate() {
                    let Some(text) = texts.get(row) else { continue };
                    let number = matches!(&line.cells[c], Some((_, true)));
                    let offset = if c >= break_count {
                        ctx.columns[c - break_count].offset
                    } else {
                        levels
                            .iter()
                            .position(|l| l.column == Some(c))
                            .and_then(|i| ctx.report.breaks[i].offset)
                    };
                    let offset = offset.or(automatic.then_some(ReportOffset::Center));
                    let (start, width) = match offset {
                        Some(ReportOffset::Columns(n)) if number => {
                            (starts[c], widths[c].saturating_sub(n as usize))
                        }
                        Some(ReportOffset::Columns(n)) => {
                            let n = (n as usize).min(widths[c]);
                            (starts[c] + n, widths[c] - n)
                        }
                        _ => (starts[c], widths[c]),
                    };
                    let align = match offset {
                        Some(ReportOffset::Center)
                            if !(line.kind == Kind::Summary && c < break_count) =>
                        {
                            ReportAlign::Center
                        }
                        _ if number => ReportAlign::Right,
                        _ => ReportAlign::Left,
                    };
                    let overflow = line.kind == Kind::Summary && c < break_count;
                    place(&mut buffer, start, width, text, align, overflow && !number);
                }
                emit(&mut out, &buffer.iter().collect::<String>());
            }
        }
        if !footnotes.is_empty() {
            for _ in 0..format.ftspace {
                emit(&mut out, "");
            }
            for footnote in footnotes {
                emit(&mut out, &centered(footnote, total));
            }
        }
        out
    }
}

/// Writes `text` into `buffer` in the field from `start`, `width` wide; a number too wide for
/// it becomes asterisks, a text may run on to the right with `overflow`, else it is cut.
fn place(
    buffer: &mut Vec<char>,
    start: usize,
    width: usize,
    text: &str,
    align: ReportAlign,
    overflow: bool,
) {
    let chars: Vec<char> = text.chars().collect();
    let numeric = !overflow
        && chars
            .first()
            .is_some_and(|c| c.is_ascii_digit() || "-.$".contains(*c))
        && chars.len() > width
        && text.parse::<f64>().is_ok();
    let chars: Vec<char> = if numeric {
        vec!['*'; width]
    } else if chars.len() > width && !overflow {
        chars[..width].to_vec()
    } else {
        chars
    };
    let pad = width.saturating_sub(chars.len());
    let at = start
        + match align {
            ReportAlign::Left => 0,
            ReportAlign::Center => pad / 2,
            ReportAlign::Right => pad,
        };
    if buffer.len() < at + chars.len() {
        buffer.resize(at + chars.len(), ' ');
    }
    buffer[at..at + chars.len()].copy_from_slice(&chars);
}

/// Text cut into lines of at most `width` characters at blanks; a longer word is cut.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let used = line.chars().count();
            let room = if used == 0 {
                width
            } else {
                width.saturating_sub(used + 1)
            };
            if word.len() <= room {
                if used > 0 {
                    line.push(' ');
                }
                line.extend(&word);
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            lines.push(word[..width].iter().collect());
            word.drain(..width);
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

fn number_cell(value: Option<f64>, format: &str, ctx: &Ctx) -> (String, bool) {
    match value {
        Some(x) => (format_value(x, format, true).trim().to_string(), true),
        None => (ctx.missing_symbol.clone(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_mode_count_whole_numbers() {
        let v = |cells: &[(f64, f64)]| Values2 {
            cells: cells.to_vec(),
            strings: 0.0,
            string: false,
        };
        let data = v(&[(1.0, 1.0), (2.7, 1.0), (3.0, 1.0), (4.0, 1.0), (9.0, 1.0)]);
        // 9 is outside the range; 2.7 counts as 2
        assert_eq!(
            aggregate(&ReportFunction::Median(1.0, 5.0), &data),
            Ok(Some(2.5))
        );
        assert_eq!(
            aggregate(&ReportFunction::Median(1.0, 9.0), &data),
            Ok(Some(3.0))
        );
        let modal = v(&[(1.0, 1.0), (2.0, 2.0), (3.0, 2.0)]);
        assert_eq!(
            aggregate(&ReportFunction::Mode(1.0, 3.0), &modal),
            Ok(Some(2.0))
        );
        assert_eq!(aggregate(&ReportFunction::Pgt(1.5), &modal), Ok(Some(80.0)));
        assert_eq!(
            aggregate(&ReportFunction::Pin(1.0, 2.0), &modal),
            Ok(Some(60.0))
        );
    }

    #[test]
    fn string_parts_pad_numbers() {
        assert_eq!(padded_number(241.0, "F8.2", 8, false), "00241.00");
        assert_eq!(padded_number(241.0, "F8.2", 3, false), "241");
        assert_eq!(padded_number(241.0, "F8.2", 4, true), " 241");
        assert_eq!(padded_number(-5.0, "F8.0", 4, false), "-005");
        assert_eq!(padded_number(12345.0, "F8.0", 3, false), "***");
    }

    #[test]
    fn composites() {
        assert_eq!(
            composite(Composite::Divide, &[Some(6.0), Some(3.0), Some(10.0)]),
            Some(20.0)
        );
        assert_eq!(
            composite(Composite::Pct, &[Some(1.0), Some(4.0)]),
            Some(25.0)
        );
        assert_eq!(composite(Composite::Pct, &[Some(1.0), Some(0.0)]), None);
        assert_eq!(
            composite(Composite::Average, &[Some(1.0), Some(4.0)]),
            Some(2.5)
        );
        assert_eq!(composite(Composite::Add, &[Some(1.0), None]), None);
    }

    #[test]
    fn titles_wrap() {
        assert_eq!(
            wrap_text("Annual Sales 1981", 6),
            ["Annual", "Sales", "1981"]
        );
        assert_eq!(wrap_text("", 6), [""]);
    }
}
