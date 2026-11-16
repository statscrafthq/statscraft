//! COMPARE DATASETS: the values and the dictionary of the active dataset against another
//! dataset or .sav file.
//!
//! From the command's description: cases pair in file order, or by the CASEID keys when given
//! (both files sorted by them in ascending order). The flag variable is 1 when the paired case
//! has other values, 0 when it has the same, -1 when the other file has no such case; it is
//! made unless FLAGMISMATCHES=NO. MATCHDATASET and MISMATCHDATASET take the active cases that
//! have an exact match, or not. The case table lists the differing values of the first cases
//! that differ. SPLIT FILE and FILTER are ignored.
//!
//! Choices where the manual is silent:
//! - A variable compares when the other file has it with the same type. Numbers compare
//!   exactly and system-missing equals only system-missing; strings compare without trailing
//!   blanks, so different widths do not count. The CASEID variables and the flag variable
//!   itself are not compared.
//! - A key that a file holds several times pairs in file order: the first with the first.
//!   Cases only the other file has are counted in the summary, not listed.
//! - The case table also lists active cases without a partner. A flag variable that already
//!   exists (an earlier run) is overwritten when numeric.
//! - VARPROPERTIES lists only the properties that differ, for the listed variables both files
//!   have. A display property the file does not state counts as its default: 8 columns, left
//!   for strings and right for numbers, the input role. WIDTH is the format width of a number
//!   and the byte width of a string; ATTRIBUTES compares the custom attributes, not those
//!   starting with $ or @.

use crate::merge::{Key, compare};
use crate::shown::Show;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Value, Values, Variable};
use statscraft_syntax::{CompareDatasets, CompareOutput, CompareProperty};

/// The key of every case.
fn keys<'a>(data: &'a Dataset, names: &[String]) -> Result<Vec<Vec<Key<'a>>>, Error> {
    let mut columns = Vec::new();
    for name in names {
        columns.push(data.variable(name)?.values());
    }
    Ok((0..data.case_count())
        .map(|case| {
            columns
                .iter()
                .map(|values| match values {
                    Values::Numeric(v) => Key::Number(v.get(case)),
                    Values::String(v) => Key::Text(v.get(case)),
                })
                .collect()
        })
        .collect())
}

fn sorted(keys: &[Vec<Key>], file: &str, names: &[String]) -> Result<(), Error> {
    if let Some(case) = (1..keys.len()).find(|&c| compare(&keys[c - 1], &keys[c]).is_gt()) {
        return Err(Error(format!(
            "{file} is not sorted by {}: case {} has a lower key than case {case}; sort both \
             files with SORT CASES BY {} first",
            names.join(" "),
            case + 1,
            names.join(" ")
        )));
    }
    Ok(())
}

/// For each active case its partner in the other dataset; and the number of other cases
/// without a partner.
fn pairs(active: &[Vec<Key>], other: &[Vec<Key>]) -> (Vec<Option<usize>>, usize) {
    let (mut partner, mut unpaired) = (vec![None; active.len()], 0);
    let (mut a, mut b) = (0, 0);
    while a < active.len() && b < other.len() {
        match compare(&active[a], &other[b]) {
            std::cmp::Ordering::Less => a += 1,
            std::cmp::Ordering::Greater => {
                unpaired += 1;
                b += 1;
            }
            std::cmp::Ordering::Equal => {
                partner[a] = Some(b);
                a += 1;
                b += 1;
            }
        }
    }
    (partner, unpaired + other.len() - b)
}

fn same(a: &Variable, case_a: usize, b: &Variable, case_b: usize) -> bool {
    match (a.values(), b.values()) {
        (Values::Numeric(x), Values::Numeric(y)) => x.get(case_a) == y.get(case_b),
        (Values::String(x), Values::String(y)) => {
            x.get(case_a).trim_end() == y.get(case_b).trim_end()
        }
        _ => false,
    }
}

fn row(header: &str, value: Cell) -> Vec<Cell> {
    vec![Cell::Text(header.into()), value]
}

/// A property of a variable as text, its default when the file states none.
fn property(variable: &Variable, property: CompareProperty) -> String {
    let string = variable.is_string();
    match property {
        CompareProperty::Measure => match variable.measure() {
            Measure::Unknown => "Unknown".into(),
            Measure::Nominal => "Nominal".into(),
            Measure::Ordinal => "Ordinal".into(),
            Measure::Scale => "Scale".into(),
        },
        CompareProperty::Label => variable.label().unwrap_or_default().into(),
        CompareProperty::ValueLabels => {
            let mut labels: Vec<String> = variable
                .value_labels()
                .iter()
                .map(|(value, label)| match value {
                    Value::Number(n) => format!("{n} '{label}'"),
                    Value::String(s) => format!("'{s}' '{label}'"),
                })
                .collect();
            labels.sort();
            labels.join(", ")
        }
        // shortcut: 8 is the writer's default for a variable without a display width; read the
        // default from the .sav writer if it ever depends on the format
        CompareProperty::Columns => variable.display_width().unwrap_or(8).to_string(),
        CompareProperty::Missing => {
            crate::session_commands::missing_text(variable.missing_values())
        }
        CompareProperty::Align => {
            let default = if string { "left" } else { "right" };
            variable.alignment().unwrap_or(default).into()
        }
        CompareProperty::Role => variable.role().unwrap_or("input").into(),
        CompareProperty::Attributes => {
            let mut attributes: Vec<String> = variable
                .attributes()
                .iter()
                .filter(|(name, _)| !name.starts_with(['$', '@']))
                .map(|(name, values)| format!("{name}={}", values.join(" | ")))
                .collect();
            attributes.sort();
            attributes.join(", ")
        }
        CompareProperty::Width => match variable.string_width() {
            Some(width) => width.to_string(),
            None => {
                let format = variable.format();
                let digits = format.trim_start_matches(|c: char| c.is_ascii_alphabetic());
                digits.split('.').next().unwrap_or_default().to_string()
            }
        },
    }
}

const PROPERTY_NAMES: [(CompareProperty, &str); 9] = [
    (CompareProperty::Measure, "Measurement level"),
    (CompareProperty::Label, "Variable label"),
    (CompareProperty::ValueLabels, "Value labels"),
    (CompareProperty::Columns, "Columns"),
    (CompareProperty::Missing, "Missing values"),
    (CompareProperty::Align, "Alignment"),
    (CompareProperty::Role, "Role"),
    (CompareProperty::Attributes, "Custom attributes"),
    (CompareProperty::Width, "Width"),
];

impl Engine {
    /// A value as the case table shows it: in the variable's format, "." for system-missing.
    fn compared_value(&self, variable: &Variable, case: usize) -> Cell {
        Cell::Text(match variable.values() {
            Values::Numeric(v) => match v.get(case) {
                Some(n) => {
                    let mut shown = self.shown();
                    shown.values = Show::Plain;
                    shown.number(variable, n)
                }
                None => ".".into(),
            },
            Values::String(v) => v.get(case).to_string(),
        })
    }

    /// Writes the cases where `keep` is true to the dataset or file that `to` names.
    fn compare_output(
        &mut self,
        to: &CompareOutput,
        data: &Dataset,
        keep: &[bool],
    ) -> Result<(), Error> {
        if !to.file && !self.is_dataset_name(&to.name) {
            return Err(Error(format!(
                "/SAVE: there is no dataset {0}; declare it first with DATASET \
                 DECLARE {0}, or quote a file name",
                to.name
            )));
        }
        if to.file && to.password {
            return Err(Error(format!(
                "/SAVE: Statscraft cannot write encrypted data files, so {} was \
                 not written; leave out the password or write a dataset",
                to.name
            )));
        }
        let mut out = data.clone();
        out.select_cases(keep)?;
        self.write_data_file(&to.name, out)
    }

    pub(crate) fn compare_datasets(
        &mut self,
        command: &CompareDatasets,
    ) -> Result<Vec<Table>, Error> {
        let other = match command.other_file {
            true => statscraft_data::Dataset::read_sav(self.path(&command.other))?,
            false => self.named_dataset(&command.other).cloned().ok_or_else(|| {
                Error(format!(
                    "there is no open dataset {}; quote the name of a .sav file",
                    command.other
                ))
            })?,
        };
        let other_name = match command.other_file {
            true => format!("'{}'", command.other),
            false => command.other.clone(),
        };
        let active = self.active()?.clone();
        let flag = command.flag.as_deref();
        if let Some(name) = flag
            && let Ok(existing) = active.variable(name)
            && existing.is_string()
        {
            return Err(Error(format!(
                "{name} is a string variable and cannot hold the flag; name \
                 another with /SAVE VARNAME"
            )));
        }
        let ids = self.expand(&command.case_id)?;
        let is = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let mut listed: Vec<String> = Vec::new();
        for name in self.expand(&command.variables)? {
            let left_out = ids.iter().chain(&listed).any(|n| is(n, &name));
            if !left_out && !flag.is_some_and(|f| is(f, &name)) {
                listed.push(name);
            }
        }

        // the variables to compare, and those that cannot be
        let mut compared = Vec::new();
        let mut skipped = Vec::new();
        for name in &listed {
            let mine = active.variable(name)?;
            match other.variable(name) {
                Err(_) => skipped.push((name.clone(), "Not in the other dataset")),
                Ok(theirs) if theirs.is_string() != mine.is_string() => {
                    skipped.push((name.clone(), "String in one dataset, numeric in the other"))
                }
                Ok(_) => compared.push(name.clone()),
            }
        }

        // pair the cases
        let (partner, other_only) = if ids.is_empty() {
            let n = active.case_count();
            let partner = (0..n)
                .map(|c| (c < other.case_count()).then_some(c))
                .collect();
            (partner, other.case_count().saturating_sub(n))
        } else {
            for id in &ids {
                let mine = active.variable(id)?;
                let theirs = other.variable(id).map_err(|_| {
                    Error(format!("the CASEID variable {id} is not in {other_name}"))
                })?;
                if mine.is_string() != theirs.is_string() {
                    return Err(Error(format!(
                        "the CASEID variable {id} is a string in one dataset \
                         and a number in the other"
                    )));
                }
            }
            let (mine, theirs) = (keys(&active, &ids)?, keys(&other, &ids)?);
            sorted(&mine, "the active dataset", &ids)?;
            sorted(&theirs, &other_name, &ids)?;
            pairs(&mine, &theirs)
        };

        // compare the values
        let pairs_of: Vec<(Variable, Variable)> = compared
            .iter()
            .map(|name| Ok((active.variable(name)?, other.variable(name)?)))
            .collect::<Result<_, Error>>()?;
        let mut differing: Vec<Vec<usize>> = Vec::with_capacity(active.case_count());
        let mut per_variable = vec![0usize; compared.len()];
        let mut flags = Vec::with_capacity(active.case_count());
        for (case, partner) in partner.iter().enumerate() {
            let Some(theirs) = *partner else {
                differing.push(Vec::new());
                flags.push(Some(-1.0));
                continue;
            };
            let found: Vec<usize> = (0..pairs_of.len())
                .filter(|&v| !same(&pairs_of[v].0, case, &pairs_of[v].1, theirs))
                .collect();
            for &v in &found {
                per_variable[v] += 1;
            }
            flags.push(Some(if found.is_empty() { 0.0 } else { 1.0 }));
            differing.push(found);
        }
        let paired = partner.iter().flatten().count();
        let differ = flags.iter().filter(|f| **f == Some(1.0)).count();
        let active_only = active.case_count() - paired;

        let mut tables = vec![Table {
            title: "Dataset Comparison".into(),
            row_headers: 1,
            columns: vec![String::new(), "Value".into()],
            rows: vec![
                row("Other dataset", Cell::Text(other_name.clone())),
                row("Variables compared", Cell::Number(compared.len() as f64)),
                row(
                    "Variables with differences",
                    Cell::Number(per_variable.iter().filter(|n| **n > 0).count() as f64),
                ),
                row("Cases compared", Cell::Number(paired as f64)),
                row("Cases with differences", Cell::Number(differ as f64)),
                row(
                    "Cases only in the active dataset",
                    Cell::Number(active_only as f64),
                ),
                row(
                    "Cases only in the other dataset",
                    Cell::Number(other_only as f64),
                ),
            ],
            ..Table::default()
        }];
        let shown = self.shown();
        if !skipped.is_empty() {
            tables.push(Table {
                title: "Variables Not Compared".into(),
                row_headers: 1,
                columns: vec!["Variable".into(), "Reason".into()],
                rows: skipped
                    .iter()
                    .map(|(name, why)| {
                        vec![
                            Cell::Text(shown.named(&active, name)),
                            Cell::Text(why.to_string()),
                        ]
                    })
                    .collect(),
                ..Table::default()
            });
        }
        if per_variable.iter().any(|n| *n > 0) {
            tables.push(Table {
                title: "Differences by Variable".into(),
                row_headers: 1,
                columns: vec!["Variable".into(), "Cases with differences".into()],
                rows: compared
                    .iter()
                    .zip(&per_variable)
                    .filter(|(_, n)| **n > 0)
                    .map(|(name, n)| {
                        vec![
                            Cell::Text(shown.named(&active, name)),
                            Cell::Number(*n as f64),
                        ]
                    })
                    .collect(),
                ..Table::default()
            });
        }
        if let Some(limit) = command.case_table {
            let listed: Vec<usize> = (0..active.case_count())
                .filter(|&c| flags[c] != Some(0.0))
                .collect();
            let shown_cases = limit.map_or(listed.len(), |l| l.min(listed.len()));
            let id_vars: Vec<Variable> = ids
                .iter()
                .map(|id| active.variable(id))
                .collect::<Result<_, _>>()?;
            let mut columns = vec!["Case".to_string()];
            columns.extend(id_vars.iter().map(|v| shown.variable(v)));
            columns.extend(["Variable", "Active dataset", "Other dataset"].map(String::from));
            let mut rows = Vec::new();
            for &case in &listed[..shown_cases] {
                let mut lead = vec![Cell::Number((case + 1) as f64)];
                lead.extend(id_vars.iter().map(|v| self.compared_value(v, case)));
                let Some(theirs) = partner[case] else {
                    let mut row = lead;
                    row.extend([
                        Cell::Text("(no matching case)".into()),
                        Cell::Empty,
                        Cell::Empty,
                    ]);
                    rows.push(row);
                    continue;
                };
                for &v in &differing[case] {
                    let (mine, other_var) = &pairs_of[v];
                    let mut row = lead.clone();
                    row.extend([
                        Cell::Text(shown.variable(mine)),
                        self.compared_value(mine, case),
                        self.compared_value(other_var, theirs),
                    ]);
                    rows.push(row);
                }
            }
            tables.push(Table {
                title: "Case Comparison".into(),
                row_headers: 1 + ids.len(),
                columns,
                rows,
                caption: (shown_cases < listed.len()).then(|| {
                    format!(
                        "The first {shown_cases} of {} cases that differ are listed.",
                        listed.len()
                    )
                }),
                ..Table::default()
            });
        }
        if !command.properties.is_empty() {
            let mut rows = Vec::new();
            for name in &listed {
                let (Ok(mine), Ok(theirs)) = (active.variable(name), other.variable(name)) else {
                    continue;
                };
                for (property, title) in PROPERTY_NAMES {
                    if !command.properties.contains(&property) {
                        continue;
                    }
                    let (a, b) = (
                        self::property(&mine, property),
                        self::property(&theirs, property),
                    );
                    if a != b {
                        rows.push(vec![
                            Cell::Text(shown.variable(&mine)),
                            Cell::Text(title.into()),
                            Cell::Text(a),
                            Cell::Text(b),
                        ]);
                    }
                }
            }
            let none = rows.is_empty();
            tables.push(Table {
                title: "Dictionary Differences".into(),
                row_headers: 2,
                columns: ["Variable", "Property", "Active dataset", "Other dataset"]
                    .map(String::from)
                    .to_vec(),
                rows,
                caption: none
                    .then(|| "The requested properties are the same in both datasets.".into()),
                ..Table::default()
            });
        }

        // the flag and the datasets of matching and differing cases
        let mut flagged = active;
        if let Some(name) = flag {
            flagged.set_numeric_values(name, flags.clone())?;
            flagged.set_format(name, "F2.0")?;
            flagged.set_measure(name, Measure::Nominal)?;
            flagged.set_variable_label(name, Some("Comparison with the other dataset"))?;
            flagged.set_value_labels(
                name,
                vec![
                    (Value::Number(-1.0), "No matching case".into()),
                    (Value::Number(0.0), "Same values".into()),
                    (Value::Number(1.0), "Different values".into()),
                ],
            )?;
        }
        let matched: Vec<bool> = flags.iter().map(|f| *f == Some(0.0)).collect();
        if let Some(to) = &command.matches {
            self.compare_output(to, &flagged, &matched)?;
        }
        if let Some(to) = &command.mismatches {
            let unmatched: Vec<bool> = matched.iter().map(|m| !m).collect();
            self.compare_output(to, &flagged, &unmatched)?;
        }
        if flag.is_some() {
            self.dataset = Some(flagged);
        }
        Ok(tables)
    }
}

#[cfg(test)]
#[test]
fn keys_pair_in_order_and_count_the_cases_left_over() {
    let k = |values: &[f64]| -> Vec<Vec<Key<'static>>> {
        values.iter().map(|v| vec![Key::Number(Some(*v))]).collect()
    };
    // a key held twice in the active file pairs with the one case the other file has
    let (partner, other_only) = pairs(&k(&[1.0, 2.0, 2.0, 4.0]), &k(&[2.0, 3.0, 4.0, 5.0]));
    assert_eq!(partner, [None, Some(0), None, Some(2)]);
    assert_eq!(other_only, 2);
}
