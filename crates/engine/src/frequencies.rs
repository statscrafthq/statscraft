use crate::{Cell, Error, Table};
use statscraft_data::{Dataset, Value, Values, Variable};

struct Row {
    value: Cell,
    label: Option<String>,
    count: f64,
    missing: bool,
}

/// One frequency table. Cases are weighted by the weight variable when one is set; a case whose
/// weight is missing, zero or negative is left out, as statistics packages do.
pub fn table(data: &Dataset, name: &str) -> Result<Table, Error> {
    let variable = data.variable(name)?;
    let weights: Option<Vec<f64>> = match data.weight_variable() {
        Some(w) => Some(
            data.variable(w)?
                .numeric()?
                .iter()
                .map(|v| v.unwrap_or(0.0).max(0.0))
                .collect(),
        ),
        None => None,
    };
    let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);

    let mut rows = match variable.values() {
        Values::Numeric(values) => {
            let mut cases: Vec<(f64, f64)> = Vec::new();
            let mut system_missing = 0.0;
            for (case, v) in values.iter().enumerate() {
                match v {
                    Some(v) => cases.push((v, weight(case))),
                    None => system_missing += weight(case),
                }
            }
            cases.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut rows: Vec<Row> = Vec::new();
            for (v, w) in cases {
                match rows.last_mut() {
                    Some(Row {
                        value: Cell::Number(last),
                        count,
                        ..
                    }) if *last == v => *count += w,
                    _ => rows.push(Row {
                        value: Cell::Number(v),
                        label: label_of(&variable, |l| matches!(l, Value::Number(n) if *n == v)),
                        count: w,
                        missing: variable.is_user_missing(v),
                    }),
                }
            }
            if system_missing > 0.0 {
                rows.push(Row {
                    value: Cell::Empty,
                    label: Some("System".into()),
                    count: system_missing,
                    missing: true,
                });
            }
            rows
        }
        Values::String(values) => {
            let mut cases: Vec<(&str, f64)> = values
                .iter()
                .enumerate()
                .map(|(c, v)| (v, weight(c)))
                .collect();
            cases.sort_by(|a, b| a.0.cmp(b.0));
            let mut rows: Vec<Row> = Vec::new();
            for (v, w) in cases {
                match rows.last_mut() {
                    Some(Row {
                        value: Cell::Text(last),
                        count,
                        ..
                    }) if last == v => *count += w,
                    _ => rows.push(Row {
                        value: Cell::Text(v.to_string()),
                        label: label_of(
                            &variable,
                            |l| matches!(l, Value::String(s) if s.trim_end() == v),
                        ),
                        count: w,
                        missing: variable.is_user_missing_str(v),
                    }),
                }
            }
            rows
        }
    };
    rows.retain(|r| r.count > 0.0);
    // valid values first, then missing ones, each in ascending order
    rows.sort_by_key(|r| r.missing);

    let total: f64 = rows.iter().map(|r| r.count).sum();
    let valid: f64 = rows.iter().filter(|r| !r.missing).map(|r| r.count).sum();
    let percent = |part: f64, whole: f64| {
        if whole > 0.0 {
            Cell::Number(100.0 * part / whole)
        } else {
            Cell::Empty
        }
    };
    let mut cumulative = 0.0;
    let mut out: Vec<Vec<Cell>> = rows
        .into_iter()
        .map(|r| {
            let (valid_percent, cumulative_percent) = if r.missing {
                (Cell::Empty, Cell::Empty)
            } else {
                cumulative += r.count;
                (percent(r.count, valid), percent(cumulative, valid))
            };
            vec![
                Cell::Text(if r.missing { "missing" } else { "valid" }.into()),
                r.value,
                r.label.map_or(Cell::Empty, Cell::Text),
                Cell::Number(r.count),
                percent(r.count, total),
                valid_percent,
                cumulative_percent,
            ]
        })
        .collect();
    out.push(vec![
        Cell::Text("total".into()),
        Cell::Empty,
        Cell::Empty,
        Cell::Number(total),
        percent(total, total),
        Cell::Empty,
        Cell::Empty,
    ]);

    Ok(Table {
        title: match variable.label() {
            Some(label) => format!("{}: {label}", variable.name()),
            None => variable.name().to_string(),
        },
        row_headers: 3,
        column_groups: vec![Vec::new(); 7],
        columns: [
            "Category",
            "Value",
            "Label",
            "Frequency",
            "Percent",
            "Valid Percent",
            "Cumulative Percent",
        ]
        .map(String::from)
        .to_vec(),
        rows: out,
        ..Table::default()
    })
}

fn label_of(variable: &Variable<'_>, is_value: impl Fn(&Value) -> bool) -> Option<String> {
    variable
        .value_labels()
        .iter()
        .find(|(v, _)| is_value(v))
        .map(|(_, l)| l.clone())
}
