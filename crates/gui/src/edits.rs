//! What the Data View and Variable View show, and the syntax an edit in them becomes.

use crate::syntax_text::{number, quote, value};
use statscraft_data::{Measure, MissingValues, Value, Values, Variable};
use statscraft_engine::format_value;

const SECONDS_PER_DAY: f64 = 86_400.0;
/// Days from the start of the date scale, 1582-10-14, to 1970-01-01.
const DAYS_TO_1970: i64 = 141_428;

/// Whether a format shows a date (true) or a date and a time (false); None for other formats.
fn date_kind(format: &str) -> Option<bool> {
    let kind: String = format
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase();
    match kind.as_str() {
        "DATE" | "ADATE" | "EDATE" | "SDATE" => Some(true),
        "DATETIME" => Some(false),
        _ => None,
    }
}

/// (year, month, day) of a number of days from 1970-01-01 (proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// A number shown in its variable's format. Dates are shown as yyyy-mm-dd (with hh:mm:ss for a
/// date and time), the other formats through the engine's `format_value`.
pub fn number_text(v: f64, format: &str) -> String {
    match date_kind(format) {
        Some(date_only) if v.is_finite() && v.abs() < 1e13 => {
            let days = (v / SECONDS_PER_DAY).floor();
            let (year, month, day) = civil_from_days(days as i64 - DAYS_TO_1970);
            let mut text = format!("{year:04}-{month:02}-{day:02}");
            if !date_only {
                let seconds = (v - days * SECONDS_PER_DAY).round() as i64;
                let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
                text.push_str(&format!(" {h:02}:{m:02}:{s:02}"));
            }
            text
        }
        _ => format_value(v, format, true),
    }
}

fn label_of<'a>(variable: &Variable<'a>, value: &Value) -> Option<&'a str> {
    variable
        .value_labels()
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, label)| label.as_str())
}

/// A cell of the Data View; `case` counts from 0. System-missing shows as an empty cell.
pub fn cell_text(variable: &Variable, case: usize, labels: bool) -> String {
    match variable.values() {
        Values::Numeric(values) => match values.get(case) {
            None => String::new(),
            Some(v) => match labels
                .then(|| label_of(variable, &Value::Number(v)))
                .flatten()
            {
                Some(label) => label.to_string(),
                None => number_text(v, variable.format()),
            },
        },
        Values::String(values) => {
            let text = values.get(case);
            match labels
                .then(|| label_of(variable, &Value::String(text.to_string())))
                .flatten()
            {
                Some(label) => label.to_string(),
                None => text.to_string(),
            }
        }
    }
}

/// What the cell editor starts with: the full value, not rounded to the display format.
pub fn edit_text(variable: &Variable, case: usize) -> String {
    match variable.values() {
        Values::Numeric(values) => match values.get(case) {
            None => String::new(),
            Some(v) if date_kind(variable.format()).is_some() => number_text(v, variable.format()),
            Some(v) => number(v),
        },
        Values::String(values) => values.get(case).to_string(),
    }
}

/// yyyy-mm-dd, optionally followed by hh:mm or hh:mm:ss, as an expression.
fn date_expression(text: &str) -> Result<String, String> {
    let bad = || format!("{text:?} is not a date; type it as yyyy-mm-dd");
    let (date, time) = text.split_once(' ').unwrap_or((text, ""));
    let parts: Vec<i64> = date
        .split('-')
        .map(|p| p.parse().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let [year, month, day] = parts[..] else {
        return Err(bad());
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1583 {
        return Err(bad());
    }
    let mut expression = format!("DATE.DMY({day}, {month}, {year})");
    let time = time.trim();
    if !time.is_empty() {
        let parts: Vec<u32> = time
            .split(':')
            .map(|p| p.parse().map_err(|_| bad()))
            .collect::<Result<_, _>>()?;
        let seconds = match parts[..] {
            [h, m] if h < 24 && m < 60 => h * 3600 + m * 60,
            [h, m, s] if h < 24 && m < 60 && s < 60 => h * 3600 + m * 60 + s,
            _ => return Err(bad()),
        };
        expression.push_str(&format!(" + {seconds}"));
    }
    Ok(expression)
}

/// The syntax that sets one cell: `case` counts from 0, `$CASENUM` from 1. An empty numeric
/// cell becomes system-missing.
pub fn cell_edit(variable: &Variable, case: usize, text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    let new = if variable.is_string() {
        quote(text)
    } else if trimmed.is_empty() {
        "$SYSMIS".to_string()
    } else if date_kind(variable.format()).is_some() && trimmed.parse::<f64>().is_err() {
        date_expression(trimmed)?
    } else {
        value(trimmed, false)?
    };
    Ok(format!(
        "IF ($CASENUM = {}) {} = {new}.",
        case + 1,
        variable.name()
    ))
}

/// The columns of the Variable View.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Name,
    Type,
    Format,
    Width,
    Decimals,
    Label,
    Values,
    Missing,
    /// The display width: how many characters wide the Data View shows the variable.
    Columns,
    Align,
    Measure,
    Role,
}

impl Field {
    pub const ALL: [Field; 12] = [
        Field::Name,
        Field::Type,
        Field::Format,
        Field::Width,
        Field::Decimals,
        Field::Label,
        Field::Values,
        Field::Missing,
        Field::Columns,
        Field::Align,
        Field::Measure,
        Field::Role,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Field::Name => "Name",
            Field::Type => "Type",
            Field::Format => "Format",
            Field::Width => "Width",
            Field::Decimals => "Decimals",
            Field::Label => "Label",
            Field::Values => "Value labels",
            Field::Missing => "Missing values",
            Field::Columns => "Columns",
            Field::Align => "Align",
            Field::Measure => "Measure",
            Field::Role => "Role",
        }
    }

    /// The values a field chosen from a list can take, as shown.
    pub fn choices(self) -> Option<&'static [&'static str]> {
        match self {
            Field::Align => Some(&ALIGNMENTS),
            Field::Measure => Some(&["Nominal", "Ordinal", "Scale"]),
            Field::Role => Some(&ROLES),
            _ => None,
        }
    }

    /// The type cannot change yet, and neither can the width of a string variable.
    pub fn editable(self, string: bool) -> bool {
        match self {
            Field::Type => false,
            Field::Width | Field::Decimals => !string,
            _ => true,
        }
    }
}

/// "F8.2" as ("F", 8, 2).
fn split_format(format: &str) -> (String, usize, usize) {
    let letters = format
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(format.len());
    let (kind, size) = format.split_at(letters);
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    (
        kind.to_ascii_uppercase(),
        width.parse().unwrap_or(0),
        decimals.parse().unwrap_or(0),
    )
}

fn bound_text(v: f64) -> String {
    if v == f64::MIN {
        "LO".into()
    } else if v == f64::MAX {
        "HI".into()
    } else {
        number(v)
    }
}

const ALIGNMENTS: [&str; 3] = ["Left", "Right", "Center"];
const ROLES: [&str; 6] = ["Input", "Target", "Both", "None", "Partition", "Split"];

/// "left" as "Left".
fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn measure_name(measure: Measure) -> &'static str {
    match measure {
        Measure::Unknown => "",
        Measure::Nominal => "Nominal",
        Measure::Ordinal => "Ordinal",
        Measure::Scale => "Scale",
    }
}

/// The text of one Variable View cell. Value labels and missing values are written the way the
/// editor reads them back: `1 = Yes; 2 = No` and `98, 99` or `LO THRU -1, 999`.
pub fn field_text(variable: &Variable, field: Field) -> String {
    let (_, width, decimals) = split_format(variable.format());
    match field {
        Field::Name => variable.name().to_string(),
        Field::Type => match variable.is_string() {
            true => "String",
            false if date_kind(variable.format()).is_some() || is_time(variable.format()) => "Date",
            false => "Numeric",
        }
        .into(),
        Field::Format => variable.format().to_string(),
        Field::Width => variable.string_width().unwrap_or(width).to_string(),
        Field::Decimals => decimals.to_string(),
        Field::Label => variable.label().unwrap_or("").to_string(),
        Field::Values => variable
            .value_labels()
            .iter()
            .map(|(v, label)| match v {
                Value::Number(n) => format!("{} = {label}", number(*n)),
                Value::String(s) => format!("{s} = {label}"),
            })
            .collect::<Vec<_>>()
            .join("; "),
        Field::Missing => match variable.missing_values() {
            MissingValues::None => String::new(),
            MissingValues::Numbers(values) => values
                .iter()
                .map(|v| number(*v))
                .collect::<Vec<_>>()
                .join(", "),
            MissingValues::Range { low, high, extra } => {
                let mut text = format!("{} THRU {}", bound_text(*low), bound_text(*high));
                if let Some(extra) = extra {
                    text.push_str(&format!(", {}", number(*extra)));
                }
                text
            }
            MissingValues::Strings(values) => values.join(", "),
        },
        Field::Columns => variable
            .display_width()
            .map_or_else(String::new, |w| w.to_string()),
        // without a setting in the file: numbers to the right, strings to the left, all input
        Field::Align => capitalized(variable.alignment().unwrap_or(if variable.is_string() {
            "left"
        } else {
            "right"
        })),
        Field::Measure => measure_name(variable.measure()).into(),
        Field::Role => capitalized(variable.role().unwrap_or("input")),
    }
}

fn missing_item(item: &str) -> Result<String, String> {
    let bound = |text: &str| match text.trim().to_ascii_uppercase().as_str() {
        "LO" | "LOWEST" => Ok("LOWEST".to_string()),
        "HI" | "HIGHEST" => Ok("HIGHEST".to_string()),
        _ => value(text, false),
    };
    let upper = item.to_ascii_uppercase();
    match upper.find(" THRU ") {
        Some(at) => Ok(format!(
            "{} THRU {}",
            bound(&item[..at])?,
            bound(&item[at + 6..])?
        )),
        None => value(item, false),
    }
}

/// A display format the Type and Format dialog offers: keyword, label, the narrowest and
/// widest width, and whether it takes decimals.
pub struct FormatKind {
    pub keyword: &'static str,
    pub label: &'static str,
    pub min: usize,
    pub max: usize,
    pub decimals: bool,
}

const fn kind(
    keyword: &'static str,
    label: &'static str,
    min: usize,
    decimals: bool,
) -> FormatKind {
    FormatKind {
        keyword,
        label,
        min,
        max: 40,
        decimals,
    }
}

/// The formats of the dialog: numbers, dates and times, and strings (last). The narrowest
/// widths are those a format needs to show its values.
pub const FORMAT_KINDS: [FormatKind; 23] = [
    kind("F", "Numeric", 1, true),
    kind("COMMA", "Comma (1,234.5)", 1, true),
    kind("DOT", "Dot (1.234,5)", 1, true),
    kind("DOLLAR", "Dollar ($1,234.50)", 2, true),
    kind("PCT", "Percent (12.5%)", 1, true),
    kind("E", "Scientific (1.2E+03)", 6, true),
    kind("N", "Leading zeros (00123)", 1, false),
    kind("DATE", "Date dd-mmm-yyyy", 9, false),
    kind("ADATE", "Date mm/dd/yyyy", 8, false),
    kind("EDATE", "Date dd.mm.yyyy", 8, false),
    kind("SDATE", "Date yyyy/mm/dd", 8, false),
    kind("JDATE", "Day of the year yyyyddd", 5, false),
    kind("QYR", "Quarter and year", 6, false),
    kind("MOYR", "Month and year", 6, false),
    kind("WKYR", "Week and year", 8, false),
    kind("WKDAY", "Day of the week", 2, false),
    kind("MONTH", "Month", 3, false),
    kind("DATETIME", "Date and time", 17, true),
    kind("YMDHMS", "Date and time yyyy-mm-dd", 16, false),
    kind("TIME", "Time hh:mm:ss", 5, true),
    kind("MTIME", "Minutes mm:ss", 5, true),
    kind("DTIME", "Days and time", 8, false),
    FormatKind {
        keyword: "A",
        label: "String",
        min: 1,
        max: 32_767,
        decimals: false,
    },
];

/// The index of the string format in `FORMAT_KINDS`.
const STRING_KIND: usize = FORMAT_KINDS.len() - 1;

/// Whether a format shows a date, a time or a part of one: the formats from DATE to DTIME.
fn is_time(format: &str) -> bool {
    let (keyword, ..) = split_format(format);
    FORMAT_KINDS[7..STRING_KIND]
        .iter()
        .any(|k| k.keyword == keyword)
}

/// The Type and Format dialog of one variable.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatChoice {
    pub name: String,
    /// The variable is a string now.
    pub string: bool,
    /// Its format now, such as F8.2.
    pub current: String,
    /// Index into `FORMAT_KINDS`.
    pub kind: usize,
    pub width: String,
    pub decimals: String,
}

impl FormatChoice {
    /// The dialog as it opens, showing the variable's format. A format the dialog does not
    /// offer shows as Numeric.
    pub fn of(variable: &Variable) -> Self {
        let (keyword, width, decimals) = split_format(variable.format());
        let kind = FORMAT_KINDS
            .iter()
            .position(|k| k.keyword == keyword)
            .unwrap_or(if variable.is_string() { STRING_KIND } else { 0 });
        let width = variable.string_width().unwrap_or(width);
        Self {
            name: variable.name().to_string(),
            string: variable.is_string(),
            current: variable.format().to_string(),
            kind,
            width: width.to_string(),
            decimals: decimals.to_string(),
        }
    }

    /// After another format is chosen: the width within its limits, no decimals where it takes
    /// none.
    pub fn fit(&mut self) {
        let kind = &FORMAT_KINDS[self.kind];
        let width = self.width.trim().parse().unwrap_or(kind.min);
        self.width = width.clamp(kind.min, kind.max).to_string();
        if !kind.decimals {
            self.decimals = "0".into();
        }
    }

    /// Whether the choice changes the variable's type, which drops its value labels and
    /// missing values.
    pub fn changes_type(&self) -> bool {
        self.string != (self.kind == STRING_KIND)
    }

    /// FORMATS for another numeric format, ALTER TYPE for a new type or string width.
    pub fn syntax(&self) -> Result<String, String> {
        let kind = &FORMAT_KINDS[self.kind];
        let width: usize = self
            .width
            .trim()
            .parse()
            .map_err(|_| "the width must be a whole number".to_string())?;
        if width < kind.min || width > kind.max {
            return Err(format!(
                "{} takes a width from {} to {}",
                kind.label, kind.min, kind.max
            ));
        }
        let decimals: usize = match self.decimals.trim() {
            "" => 0,
            text => text
                .parse()
                .map_err(|_| "the decimals must be a whole number".to_string())?,
        };
        if decimals > 0 && !kind.decimals {
            return Err(format!("{} takes no decimals", kind.label));
        }
        if decimals > 16 || (decimals > 0 && decimals >= width) {
            return Err("the decimals must be fewer than the width, at most 16".into());
        }
        let format = match decimals {
            0 => format!("{}{width}", kind.keyword),
            d => format!("{}{width}.{d}", kind.keyword),
        };
        let name = &self.name;
        // F8 and F8.0 are the same format
        if split_format(&format) == split_format(&self.current) {
            return Err("choose another format, width or decimals".into());
        }
        if self.string && self.kind != STRING_KIND && !["F", "COMMA", "DOT"].contains(&kind.keyword)
        {
            return Err(
                "a string becomes a number as Numeric, Comma or Dot; choose another format after"
                    .into(),
            );
        }
        Ok(if self.string || self.kind == STRING_KIND {
            format!("ALTER TYPE {name} ({format}).")
        } else {
            format!("FORMATS {name} ({format}).")
        })
    }
}

/// Lines of `code<separator>label` pasted at once. The separator is the first of tab,
/// semicolon, equals sign and comma that the text holds; a line splits at its first separator.
/// Empty lines are left out.
pub fn parse_labels(text: &str) -> Result<Vec<(String, String)>, String> {
    let separator = ['\t', ';', '=', ',']
        .into_iter()
        .find(|s| text.contains(*s))
        .ok_or("put a tab, semicolon, equals sign or comma between each code and its label")?;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once(separator)
                .map(|(code, label)| (code.trim().to_string(), label.trim().to_string()))
                .ok_or_else(|| format!("{line:?} has no {}", separator_name(separator)))
        })
        .collect()
}

fn separator_name(separator: char) -> &'static str {
    match separator {
        '\t' => "tab",
        ';' => "semicolon",
        '=' => "equals sign",
        _ => "comma",
    }
}

/// The value labels dialog: codes and labels one by one, or many pasted at once.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelsEditor {
    /// The variable the dialog opened on.
    pub name: String,
    pub string: bool,
    /// The other selected variables of the same type, which can get the same labels.
    pub others: Vec<String>,
    pub apply_to_others: bool,
    /// Code and label as typed.
    pub pairs: Vec<(String, String)>,
    original: Vec<(String, String)>,
    /// Pasting many at once instead of the list.
    pub quick: bool,
    pub pasted: String,
    /// With pasting: the pasted list becomes the whole set (VALUE LABELS) instead of adding
    /// and changing codes (ADD VALUE LABELS).
    pub replace: bool,
}

impl LabelsEditor {
    pub fn of(variable: &Variable, others: Vec<String>) -> Self {
        let pairs: Vec<(String, String)> = variable
            .value_labels()
            .iter()
            .map(|(v, label)| {
                let code = match v {
                    Value::Number(n) => number(*n),
                    Value::String(s) => s.trim_end().to_string(),
                };
                (code, label.clone())
            })
            .collect();
        Self {
            name: variable.name().to_string(),
            string: variable.is_string(),
            others,
            apply_to_others: false,
            original: pairs.clone(),
            pairs,
            quick: false,
            pasted: String::new(),
            replace: false,
        }
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut names = vec![self.name.as_str()];
        if self.apply_to_others {
            names.extend(self.others.iter().map(String::as_str));
        }
        let names = names.join(" ");
        let (pairs, command) = if self.quick {
            let pairs = parse_labels(&self.pasted)?;
            let command = if self.replace {
                "VALUE LABELS"
            } else {
                "ADD VALUE LABELS"
            };
            (pairs, command)
        } else {
            let pairs: Vec<(String, String)> = self
                .pairs
                .iter()
                .filter(|(code, label)| !code.trim().is_empty() || !label.trim().is_empty())
                .cloned()
                .collect();
            if pairs == self.original && !self.apply_to_others {
                return Err("change, add or remove a label".into());
            }
            (pairs, "VALUE LABELS")
        };
        if pairs.is_empty() && command == "ADD VALUE LABELS" {
            return Err("paste at least one code and label".into());
        }
        let mut items = Vec::new();
        for (code, label) in &pairs {
            if code.trim().is_empty() {
                return Err(format!("give the code of the label {label:?}"));
            }
            let code = match self.string {
                true => quote(code.trim_end()),
                false => value(code, false)?,
            };
            items.push(format!("{code} {}", quote(label)));
        }
        Ok(match items.is_empty() {
            true => format!("{command} {names}."),
            false => format!("{command} {names}\n  {}.", items.join("\n  ")),
        })
    }
}

/// The missing values dialog: up to three values, or a range and one value more.
#[derive(Clone, Debug, PartialEq)]
pub struct MissingEditor {
    pub name: String,
    pub string: bool,
    pub others: Vec<String>,
    pub apply_to_others: bool,
    pub range: bool,
    pub values: [String; 3],
    pub low: String,
    pub high: String,
    pub extra: String,
    original: String,
}

impl MissingEditor {
    pub fn of(variable: &Variable, others: Vec<String>) -> Self {
        let mut d = Self {
            name: variable.name().to_string(),
            string: variable.is_string(),
            others,
            apply_to_others: false,
            range: false,
            values: Default::default(),
            low: String::new(),
            high: String::new(),
            extra: String::new(),
            original: String::new(),
        };
        match variable.missing_values() {
            MissingValues::None => {}
            MissingValues::Numbers(values) => {
                for (slot, v) in d.values.iter_mut().zip(values) {
                    *slot = number(*v);
                }
            }
            MissingValues::Strings(values) => {
                for (slot, v) in d.values.iter_mut().zip(values) {
                    *slot = v.trim_end().to_string();
                }
            }
            MissingValues::Range { low, high, extra } => {
                d.range = true;
                d.low = bound_text(*low);
                d.high = bound_text(*high);
                d.extra = extra.map(number).unwrap_or_default();
            }
        }
        d.original = d.items().unwrap_or_default();
        d
    }

    /// The list in parentheses.
    fn items(&self) -> Result<String, String> {
        let mut items = Vec::new();
        if self.range {
            if self.string {
                return Err("a range works only with numeric variables".into());
            }
            match (self.low.trim(), self.high.trim()) {
                ("", "") => {}
                ("", _) | (_, "") => return Err("give both ends of the range".into()),
                (low, high) => items.push(missing_item(&format!("{low} THRU {high}"))?),
            }
            if !self.extra.trim().is_empty() {
                items.push(value(&self.extra, false)?);
            }
        } else {
            for v in self.values.iter().filter(|v| !v.trim().is_empty()) {
                items.push(match self.string {
                    true => quote(v.trim_end()),
                    false => missing_item(v)?,
                });
            }
        }
        Ok(items.join(", "))
    }

    pub fn syntax(&self) -> Result<String, String> {
        let items = self.items()?;
        if items == self.original && !self.apply_to_others {
            return Err("change, add or remove a missing value".into());
        }
        let mut names = vec![self.name.as_str()];
        if self.apply_to_others {
            names.extend(self.others.iter().map(String::as_str));
        }
        Ok(format!("MISSING VALUES {} ({items}).", names.join(" ")))
    }
}

/// The syntax that deletes variables.
pub fn delete_variables(names: &[String]) -> String {
    format!("DELETE VARIABLES {}.", names.join(" "))
}

/// Names for a question: `age`, `age and region`, `age, region and q1`, or for many the count
/// and the first few.
pub fn listed(names: &[String]) -> String {
    match names {
        [] => "nothing".into(),
        [one] => one.clone(),
        [first @ .., last] if names.len() <= 6 => format!("{} and {last}", first.join(", ")),
        _ => format!("{} variables ({}, ...)", names.len(), names[..4].join(", ")),
    }
}

/// The syntax for an edit of a Variable View cell, or None when the text did not change.
pub fn field_edit(variable: &Variable, field: Field, text: &str) -> Result<Option<String>, String> {
    if text == field_text(variable, field) {
        return Ok(None);
    }
    let name = variable.name();
    let string = variable.is_string();
    let (kind, width, decimals) = split_format(variable.format());
    let whole = |what: &str| {
        text.trim()
            .parse::<usize>()
            .map_err(|_| format!("the {what} must be a whole number"))
    };
    let format = |width: usize, decimals: usize| {
        if decimals > 0 {
            format!("FORMATS {name} ({kind}{width}.{decimals}).")
        } else {
            format!("FORMATS {name} ({kind}{width}).")
        }
    };
    let syntax = match field {
        Field::Type => return Err("the type cannot be changed yet".into()),
        Field::Name => {
            let new = text.trim();
            if new.is_empty() {
                return Err("a variable needs a name".into());
            }
            format!("RENAME VARIABLES ({name} = {new}).")
        }
        Field::Format => {
            let format = text.trim().to_ascii_uppercase();
            let (new_kind, new_width, _) = split_format(&format);
            let valid = !new_kind.is_empty()
                && new_width > 0
                && format[new_kind.len()..]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.');
            if !valid {
                return Err(format!("{text:?} is not a format such as F8.2 or A20"));
            }
            format!("FORMATS {name} ({format}).")
        }
        Field::Width => format(whole("width")?, decimals),
        Field::Decimals => format(width, whole("number of decimals")?),
        Field::Label => format!("VARIABLE LABELS {name} {}.", quote(text.trim())),
        Field::Values => {
            // shortcut: a label cannot hold ";", which separates the pairs here; add a value
            // label dialog when someone needs one
            let mut pairs = Vec::new();
            for pair in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                let (v, label) = pair
                    .split_once('=')
                    .ok_or_else(|| format!("{pair:?} needs the form value = label"))?;
                pairs.push(format!(
                    "{} {}",
                    value(v.trim(), string)?,
                    quote(label.trim())
                ));
            }
            match pairs.is_empty() {
                true => format!("VALUE LABELS {name}."),
                false => format!("VALUE LABELS {name} {}.", pairs.join(" ")),
            }
        }
        Field::Missing => {
            let items = text
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| match string {
                    true => Ok(quote(item)),
                    false => missing_item(item),
                })
                .collect::<Result<Vec<_>, _>>()?;
            format!("MISSING VALUES {name} ({}).", items.join(", "))
        }
        Field::Columns => match whole("number of columns")? {
            0 => return Err("the number of columns must be at least 1".into()),
            n => format!("VARIABLE WIDTH {name} ({n})."),
        },
        Field::Align => {
            let align = text.trim().to_ascii_uppercase();
            if !ALIGNMENTS.iter().any(|a| a.eq_ignore_ascii_case(&align)) {
                return Err("choose left, right or center".into());
            }
            format!("VARIABLE ALIGNMENT {name} ({align}).")
        }
        Field::Role => {
            let role = text.trim().to_ascii_uppercase();
            if !ROLES.iter().any(|r| r.eq_ignore_ascii_case(&role)) {
                return Err("choose input, target, both, none, partition or split".into());
            }
            format!("VARIABLE ROLE /{role} {name}.")
        }
        Field::Measure => {
            let level = match text.trim().to_ascii_lowercase().as_str() {
                "nominal" => "NOMINAL",
                "ordinal" => "ORDINAL",
                "scale" => "SCALE",
                _ => return Err("choose nominal, ordinal or scale".into()),
            };
            format!("VARIABLE LEVEL {name} ({level}).")
        }
    };
    Ok(Some(syntax))
}

#[cfg(test)]
mod tests {
    use super::*;
    use statscraft_engine::session::Session;

    fn survey() -> Session {
        let mut session = Session::default();
        let file = format!(
            "{}/../../tests/fixtures/survey_basic.sav",
            env!("CARGO_MANIFEST_DIR")
        );
        session
            .run(&format!("GET FILE={}.", quote(&file)), None)
            .unwrap();
        session
    }

    fn with<R>(session: &Session, name: &str, f: impl FnOnce(&Variable) -> R) -> R {
        f(&session.dataset().unwrap().variable(name).unwrap())
    }

    #[test]
    fn dates_show_as_iso_dates() {
        // 1970-01-01 is 141 428 days after the start of the date scale
        assert_eq!(number_text(141_428.0 * 86_400.0, "DATE11"), "1970-01-01");
        assert_eq!(number_text(13_918_176_000.0, "ADATE10"), "2023-11-01");
        assert_eq!(
            number_text(13_918_176_000.0 + 3_723.0, "DATETIME20"),
            "2023-11-01 01:02:03"
        );
        assert_eq!(number_text(2.5, "F8.2"), "2.50");
        assert_eq!(
            date_expression("2026-11-09").unwrap(),
            "DATE.DMY(9, 11, 2026)"
        );
        assert_eq!(
            date_expression("2026-11-09 10:30").unwrap(),
            "DATE.DMY(9, 11, 2026) + 37800"
        );
        assert!(date_expression("09.11.2026").is_err());
        assert!(date_expression("2026-13-01").is_err());
    }

    #[test]
    fn cells_show_values_or_labels() {
        let session = survey();
        with(&session, "gender", |v| {
            assert_eq!(cell_text(v, 0, false), "1");
            assert_eq!(cell_text(v, 0, true), "Male");
        });
        with(&session, "monthly_spend", |v| {
            assert_eq!(cell_text(v, 0, false), "78.28");
            assert_eq!(edit_text(v, 0), "78.28");
        });
        with(&session, "q6_mention3", |v| {
            assert_eq!(cell_text(v, 0, false), "");
            assert_eq!(edit_text(v, 0), "");
        });
        with(&session, "plan", |v| {
            assert_eq!(cell_text(v, 1, false), "PREMIUM")
        });
        with(&session, "signup_date", |v| {
            assert_eq!(cell_text(v, 0, false), "2023-11-01");
        });
    }

    #[test]
    fn a_cell_edit_is_an_if_on_casenum_and_changes_only_that_case() {
        let mut session = survey();
        let edits = [
            with(&session, "age", |v| cell_edit(v, 1, " 33 ")).unwrap(),
            with(&session, "plan", |v| cell_edit(v, 2, "GOLD")).unwrap(),
            with(&session, "monthly_spend", |v| cell_edit(v, 0, "")).unwrap(),
            with(&session, "signup_date", |v| cell_edit(v, 0, "2024-02-29")).unwrap(),
        ];
        assert_eq!(
            edits,
            [
                "IF ($CASENUM = 2) age = 33.",
                "IF ($CASENUM = 3) plan = 'GOLD'.",
                "IF ($CASENUM = 1) monthly_spend = $SYSMIS.",
                "IF ($CASENUM = 1) signup_date = DATE.DMY(29, 2, 2024).",
            ]
        );
        for edit in &edits {
            session.run(edit, None).unwrap();
        }
        with(&session, "age", |v| {
            assert_eq!(
                (cell_text(v, 0, false), cell_text(v, 1, false)),
                ("49".into(), "33".into())
            );
        });
        with(&session, "plan", |v| {
            assert_eq!(cell_text(v, 2, false), "GOLD")
        });
        with(&session, "monthly_spend", |v| {
            assert_eq!(cell_text(v, 0, false), "")
        });
        with(&session, "signup_date", |v| {
            assert_eq!(cell_text(v, 0, false), "2024-02-29")
        });
        let error = with(&session, "age", |v| cell_edit(v, 0, "old")).unwrap_err();
        assert_eq!(error, "\"old\" is not a number");
    }

    #[test]
    fn the_type_and_format_dialog_runs_formats_or_alter_type() {
        let mut session = survey();
        let index = |keyword: &str| FORMAT_KINDS.iter().position(|k| k.keyword == keyword);
        let choice = |s: &Session, name: &str| with(s, name, FormatChoice::of);

        let mut d = choice(&session, "monthly_spend");
        assert_eq!(
            (d.kind, d.width.as_str(), d.decimals.as_str()),
            (0, "8", "2")
        );
        assert!(!d.changes_type());
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose another format, width or decimals"
        );
        d.kind = index("COMMA").unwrap();
        d.width = "10".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "FORMATS monthly_spend (COMMA10.2).");
        session.run(&syntax, None).unwrap();
        with(&session, "monthly_spend", |v| {
            assert_eq!(v.format(), "COMMA10.2")
        });

        // a date format: the width grows to what it needs, decimals go
        let mut d = choice(&session, "age");
        d.kind = index("DATE").unwrap();
        d.decimals = "2".into();
        d.fit();
        assert_eq!((d.width.as_str(), d.decimals.as_str()), ("9", "0"));
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "FORMATS age (DATE9).");
        session.run(&syntax, None).unwrap();

        // a string gets another width, a number becomes a string and back
        let mut d = choice(&session, "plan");
        assert_eq!((d.kind, d.width.as_str()), (STRING_KIND, "10"));
        d.width = "20".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "ALTER TYPE plan (A20).");
        session.run(&syntax, None).unwrap();
        with(&session, "plan", |v| assert_eq!(v.string_width(), Some(20)));
        d = choice(&session, "plan");
        d.kind = index("DOLLAR").unwrap();
        assert!(d.changes_type());
        assert!(
            d.syntax()
                .unwrap_err()
                .starts_with("a string becomes a number")
        );
        d.kind = 0;
        d.fit();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "ALTER TYPE plan (F20).");
        session.run(&syntax, None).unwrap();
        with(&session, "plan", |v| assert!(!v.is_string()));
        let mut d = choice(&session, "q1_satisfaction");
        d.kind = STRING_KIND;
        d.fit();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "ALTER TYPE q1_satisfaction (A2).");
        session.run(&syntax, None).unwrap();
        with(&session, "q1_satisfaction", |v| assert!(v.is_string()));

        // what cannot be
        let mut d = choice(&session, "weight");
        d.kind = index("E").unwrap();
        d.width = "4".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "Scientific (1.2E+03) takes a width from 6 to 40"
        );
        d.kind = index("N").unwrap();
        d.width = "8".into();
        d.decimals = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "Leading zeros (00123) takes no decimals"
        );
        d.kind = 0;
        d.decimals = "8".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the decimals must be fewer than the width, at most 16"
        );
        d.width = "wide".into();
        assert_eq!(d.syntax().unwrap_err(), "the width must be a whole number");
    }

    #[test]
    fn pasted_labels_split_at_the_separator_they_use() {
        let pairs = |list: &[(&str, &str)]| {
            list.iter()
                .map(|(c, l)| (c.to_string(), l.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            parse_labels("1\tVery good\n2\tGood, mostly\n\n").unwrap(),
            pairs(&[("1", "Very good"), ("2", "Good, mostly")])
        );
        assert_eq!(
            parse_labels("1; Yes = sure\r\n 2 ;No").unwrap(),
            pairs(&[("1", "Yes = sure"), ("2", "No")])
        );
        assert_eq!(
            parse_labels("1 = Yes, sure\n2=No").unwrap(),
            pairs(&[("1", "Yes, sure"), ("2", "No")])
        );
        assert_eq!(
            parse_labels("1,Yes\n2,No").unwrap(),
            pairs(&[("1", "Yes"), ("2", "No")])
        );
        assert_eq!(
            parse_labels("1,Yes\n2 No").unwrap_err(),
            "\"2 No\" has no comma"
        );
        assert!(parse_labels("1 Yes").is_err());
    }

    #[test]
    fn the_value_labels_dialog_replaces_or_adds_labels() {
        let mut session = survey();
        let mut d = with(&session, "gender", |v| {
            LabelsEditor::of(v, vec!["region".into()])
        });
        assert_eq!(d.pairs[0], ("1".to_string(), "Male".to_string()));
        assert_eq!(d.syntax().unwrap_err(), "change, add or remove a label");
        // the list: change one, remove one, add one
        d.pairs[0].1 = "Man".into();
        d.pairs.remove(1);
        d.pairs.push(("9".into(), "No answer".into()));
        d.pairs.push((" ".into(), String::new()));
        let syntax = d.syntax().unwrap();
        assert!(
            syntax.starts_with("VALUE LABELS gender\n  1 'Man'\n"),
            "{syntax}"
        );
        assert!(syntax.ends_with("\n  9 'No answer'."), "{syntax}");
        session.run(&syntax, None).unwrap();
        with(&session, "gender", |v| {
            let text = field_text(v, Field::Values);
            assert!(
                text.starts_with("1 = Man; ") && !text.contains("Female"),
                "{text}"
            );
        });
        d.pairs.push(("x".into(), "Bad".into()));
        assert_eq!(d.syntax().unwrap_err(), "\"x\" is not a number");
        d.pairs.pop();
        d.pairs.push((String::new(), "No code".into()));
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the code of the label \"No code\""
        );

        // pasting: update adds and changes, replace makes the whole set, for two variables
        let mut d = with(&session, "gender", |v| {
            LabelsEditor::of(v, vec!["region".into()])
        });
        d.quick = true;
        d.pasted = "1\tMan\n7\tSeven".into();
        d.apply_to_others = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "ADD VALUE LABELS gender region\n  1 'Man'\n  7 'Seven'."
        );
        session.run(&syntax, None).unwrap();
        with(&session, "region", |v| {
            let text = field_text(v, Field::Values);
            // code 1 changed, 7 added, the others kept
            assert!(
                text.contains("1 = Man")
                    && text.contains("2 = South")
                    && text.contains("7 = Seven"),
                "{text}"
            );
        });
        d.replace = true;
        let syntax = d.syntax().unwrap();
        assert!(syntax.starts_with("VALUE LABELS gender region\n"));
        session.run(&syntax, None).unwrap();
        with(&session, "region", |v| {
            assert_eq!(field_text(v, Field::Values), "1 = Man; 7 = Seven");
        });

        // strings get quotes; an empty list clears the labels
        let mut d = with(&session, "plan", |v| LabelsEditor::of(v, Vec::new()));
        d.pairs = vec![("BASIC".into(), "Basic plan".into())];
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "VALUE LABELS plan\n  'BASIC' 'Basic plan'.");
        session.run(&syntax, None).unwrap();
        let mut d = with(&session, "plan", |v| LabelsEditor::of(v, Vec::new()));
        d.pairs.clear();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "VALUE LABELS plan.");
        session.run(&syntax, None).unwrap();
        with(&session, "plan", |v| assert!(v.value_labels().is_empty()));
    }

    #[test]
    fn the_missing_values_dialog_sets_values_or_a_range() {
        let mut session = survey();
        let mut d = with(&session, "q1_satisfaction", |v| {
            MissingEditor::of(v, vec!["q2_recommend".into()])
        });
        assert_eq!(d.values[..2], ["98".to_string(), "99".to_string()]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "change, add or remove a missing value"
        );
        d.values[2] = "97".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MISSING VALUES q1_satisfaction (98, 99, 97).");
        session.run(&syntax, None).unwrap();

        let mut d = with(&session, "q2_recommend", |v| {
            MissingEditor::of(v, Vec::new())
        });
        assert!(d.range);
        assert_eq!((d.low.as_str(), d.high.as_str()), ("97", "99"));
        d.low = "lo".into();
        d.high = "-1".into();
        d.extra = "999".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MISSING VALUES q2_recommend (LOWEST THRU -1, 999).");
        session.run(&syntax, None).unwrap();
        with(&session, "q2_recommend", |v| {
            assert_eq!(field_text(v, Field::Missing), "LO THRU -1, 999");
        });
        d.high.clear();
        assert_eq!(d.syntax().unwrap_err(), "give both ends of the range");

        // none at all, for two variables at once
        let mut d = with(&session, "q1_satisfaction", |v| {
            MissingEditor::of(v, vec!["q2_recommend".into()])
        });
        d.values = Default::default();
        d.apply_to_others = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MISSING VALUES q1_satisfaction q2_recommend ().");
        session.run(&syntax, None).unwrap();
        with(&session, "q2_recommend", |v| {
            assert_eq!(field_text(v, Field::Missing), "")
        });

        let mut d = with(&session, "plan", |v| MissingEditor::of(v, Vec::new()));
        d.values[0] = "NONE".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MISSING VALUES plan ('NONE').");
        session.run(&syntax, None).unwrap();
        d.range = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "a range works only with numeric variables"
        );
    }

    #[test]
    fn selected_variables_are_deleted_by_syntax() {
        let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(listed(&names(&["age"])), "age");
        assert_eq!(listed(&names(&["age", "region"])), "age and region");
        assert_eq!(listed(&names(&["a", "b", "c"])), "a, b and c");
        assert_eq!(
            listed(&names(&["a", "b", "c", "d", "e", "f", "g"])),
            "7 variables (a, b, c, d, ...)"
        );
        let mut session = survey();
        let count = session.dataset().unwrap().variable_count();
        let syntax = delete_variables(&names(&["q5_1", "plan"]));
        assert_eq!(syntax, "DELETE VARIABLES q5_1 plan.");
        session.run(&syntax, None).unwrap();
        let data = session.dataset().unwrap();
        assert_eq!(data.variable_count(), count - 2);
        assert!(data.variable("plan").is_err());
        // the one-step undo brings them back
        assert!(session.undo());
        assert!(session.dataset().unwrap().variable("plan").is_ok());
    }

    #[test]
    fn variable_view_shows_the_dictionary() {
        let session = survey();
        let row = |name: &str| {
            with(&session, name, |v| {
                Field::ALL.map(|field| field_text(v, field))
            })
        };
        let gender = row("gender");
        assert_eq!(gender[..5], ["gender", "Numeric", "F1.0", "1", "0"]);
        assert_eq!(gender[5], "Gender");
        assert!(
            gender[6].starts_with("1 = Male; 2 = Female"),
            "{}",
            gender[6]
        );
        // the display width of the file
        assert_eq!(gender[8..], ["8", "Right", "Nominal", "Input"]);
        // without a display width, the cell is empty
        let mut session = survey();
        session.run("COMPUTE new = 1.", None).unwrap();
        with(&session, "new", |v| {
            assert_eq!(field_text(v, Field::Columns), "")
        });
        assert_eq!(row("plan")[9], "Left");
        assert_eq!(row("signup_date")[1], "Date");
        assert_eq!(row("monthly_spend")[4], "2");
        let plan = row("plan");
        assert_eq!(plan[..4], ["plan", "String", "A10", "10"]);
        assert!(!Field::Width.editable(true) && Field::Width.editable(false));
        assert!(!Field::Type.editable(false));
    }

    #[test]
    fn variable_view_edits_become_dictionary_commands() {
        let mut session = survey();
        let edit = |s: &Session, name: &str, field: Field, text: &str| {
            with(s, name, |v| field_edit(v, field, text))
        };
        let cases = [
            (
                "age",
                Field::Label,
                " Age in years ",
                "VARIABLE LABELS age 'Age in years'.",
            ),
            (
                "gender",
                Field::Values,
                "1 = Man; 2 = Woman's; ",
                "VALUE LABELS gender 1 'Man' 2 'Woman''s'.",
            ),
            (
                "plan",
                Field::Values,
                "b = Basic",
                "VALUE LABELS plan 'b' 'Basic'.",
            ),
            ("region", Field::Values, " ", "VALUE LABELS region."),
            (
                "age",
                Field::Missing,
                "98, 99",
                "MISSING VALUES age (98, 99).",
            ),
            (
                "q2_recommend",
                Field::Missing,
                "lo thru -1, 999",
                "MISSING VALUES q2_recommend (LOWEST THRU -1, 999).",
            ),
            (
                "plan",
                Field::Missing,
                "none",
                "MISSING VALUES plan ('none').",
            ),
            (
                "q1_satisfaction",
                Field::Missing,
                "",
                "MISSING VALUES q1_satisfaction ().",
            ),
            (
                "age",
                Field::Measure,
                "Ordinal",
                "VARIABLE LEVEL age (ORDINAL).",
            ),
            (
                "monthly_spend",
                Field::Format,
                "comma10.2",
                "FORMATS monthly_spend (COMMA10.2).",
            ),
            (
                "monthly_spend",
                Field::Decimals,
                "1",
                "FORMATS monthly_spend (COMMA10.1).",
            ),
            ("age", Field::Width, "5", "FORMATS age (F5)."),
            ("plan", Field::Columns, " 12 ", "VARIABLE WIDTH plan (12)."),
            (
                "age",
                Field::Align,
                "Center",
                "VARIABLE ALIGNMENT age (CENTER).",
            ),
            (
                "gender",
                Field::Role,
                "Target",
                "VARIABLE ROLE /TARGET gender.",
            ),
            (
                "resp_id",
                Field::Name,
                "id",
                "RENAME VARIABLES (resp_id = id).",
            ),
        ];
        for (name, field, text, expected) in cases {
            let syntax = edit(&session, name, field, text).unwrap().unwrap();
            assert_eq!(syntax, expected);
            session.run(&syntax, None).unwrap();
        }
        with(&session, "age", |v| {
            assert_eq!(field_text(v, Field::Label), "Age in years");
            assert_eq!(field_text(v, Field::Measure), "Ordinal");
            assert_eq!(field_text(v, Field::Format), "F5.0");
            assert_eq!(field_text(v, Field::Align), "Center");
        });
        with(&session, "plan", |v| {
            assert_eq!(field_text(v, Field::Columns), "12");
        });
        assert_eq!(edit(&session, "plan", Field::Columns, "12").unwrap(), None);
        with(&session, "gender", |v| {
            assert_eq!(field_text(v, Field::Values), "1 = Man; 2 = Woman's");
            assert_eq!(field_text(v, Field::Role), "Target");
        });
        with(&session, "q1_satisfaction", |v| {
            assert_eq!(field_text(v, Field::Missing), "");
        });
        with(&session, "q2_recommend", |v| {
            assert_eq!(field_text(v, Field::Missing), "LO THRU -1, 999");
        });
        assert!(session.dataset().unwrap().variable("id").is_ok());

        // unchanged text runs nothing, bad text says why
        assert_eq!(
            edit(&session, "age", Field::Label, "Age in years").unwrap(),
            None
        );
        for (field, text, error) in [
            (Field::Measure, "big", "choose nominal, ordinal or scale"),
            (Field::Align, "top", "choose left, right or center"),
            (
                Field::Role,
                "main",
                "choose input, target, both, none, partition or split",
            ),
            (
                Field::Columns,
                "0",
                "the number of columns must be at least 1",
            ),
            (
                Field::Columns,
                "wide",
                "the number of columns must be a whole number",
            ),
            (
                Field::Format,
                "8.2",
                "\"8.2\" is not a format such as F8.2 or A20",
            ),
            (Field::Width, "wide", "the width must be a whole number"),
            (
                Field::Values,
                "1 Male",
                "\"1 Male\" needs the form value = label",
            ),
            (Field::Missing, "a, b", "\"a\" is not a number"),
            (Field::Type, "String", "the type cannot be changed yet"),
            (Field::Name, " ", "a variable needs a name"),
        ] {
            assert_eq!(edit(&session, "age", field, text).unwrap_err(), error);
        }
    }
}
