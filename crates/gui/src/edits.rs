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
    Measure,
}

impl Field {
    pub const ALL: [Field; 9] = [
        Field::Name,
        Field::Type,
        Field::Format,
        Field::Width,
        Field::Decimals,
        Field::Label,
        Field::Values,
        Field::Missing,
        Field::Measure,
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
            Field::Measure => "Measure",
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
        Field::Type => if variable.is_string() {
            "String"
        } else {
            "Numeric"
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
        Field::Measure => measure_name(variable.measure()).into(),
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
        assert_eq!(gender[8], "Nominal");
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
        });
        with(&session, "gender", |v| {
            assert_eq!(field_text(v, Field::Values), "1 = Man; 2 = Woman's");
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
