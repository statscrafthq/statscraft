//! MEANS: statistics of dependent variables within the groups of control variables, with an
//! optional one-way analysis of variance and test of linearity.
//!
//! Rules from the Command Syntax Reference (MEANS) and the Algorithms manual (MEANS):
//! - Each table list gives one Report per dependent and combination of control variables (one
//!   from each BY dimension). Rows are the groups of the first control, the next control nested
//!   inside, each level closed by a Total; only groups with cases are shown.
//! - Without /MISSING a case counts for a dependent when it is valid on that dependent and the
//!   controls of the table. Any /MISSING makes it tablewise: valid on every variable of the
//!   list. INCLUDE makes user-missing values valid, DEPENDENT only those of the controls.
//! - Weights are used as they are; a case with a missing, zero or negative weight is left out.
//! - Skewness needs W >= 3, kurtosis W >= 4, both a positive standard deviation. The median is
//!   the value at position (W + 1) / 2, interpolated. Harmonic and geometric means need
//!   positive values (where the manual is silent: other values leave them empty). The grouped
//!   median follows the appendix Grouped Percentiles.
//! - NPCT(var) and SPCT(var) take as base the cases in the row's category of that control, all
//!   cases on its Total rows (the manual says "within the specified variable" and no more);
//!   a report without that control leaves them empty with a warning.
//! - A string dependent gets COUNT, FIRST and LAST only (and the percentages of N).
//! - ANOVA uses the first control; it needs two groups, the test of linearity three groups of
//!   a numeric control. What cannot be done is reported as a warning.

use crate::aggregate::median;
use crate::crosstabs::{Key, categories};
use crate::descriptives::{Summary, case_weights, number, summarize};
use crate::dist::f_p;
use crate::format::{Stat, formats_of, procedure_format};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Means, MeansCell as M, MeansMissing};

fn cell_name(cell: M, within: &[String]) -> String {
    match cell {
        M::NpctIn(i) => return format!("% of Total N in {}", within[i]),
        M::SpctIn(i) => return format!("% of Total Sum in {}", within[i]),
        _ => {}
    }
    match cell {
        M::Mean => "Mean",
        M::Count => "N",
        M::StdDev => "Std. Deviation",
        M::Median => "Median",
        M::SeMean => "Std. Error of Mean",
        M::Sum => "Sum",
        M::Min => "Minimum",
        M::Max => "Maximum",
        M::Range => "Range",
        M::Variance => "Variance",
        M::Kurtosis => "Kurtosis",
        M::SeKurtosis => "Std. Error of Kurtosis",
        M::Skewness => "Skewness",
        M::SeSkewness => "Std. Error of Skewness",
        M::First => "First",
        M::Last => "Last",
        M::Npct => "% of Total N",
        M::Spct => "% of Total Sum",
        M::Harmonic => "Harmonic Mean",
        M::Geometric => "Geometric Mean",
        M::GroupedMedian => "Grouped Median",
        M::NpctIn(_) | M::SpctIn(_) => unreachable!("named above"),
    }
    .to_string()
}

/// How a cell relates to the values of the dependent, for its format; None for one without
/// their unit.
fn kind(cell: M) -> Option<Stat> {
    match cell {
        M::Mean
        | M::Median
        | M::Min
        | M::Max
        | M::First
        | M::Last
        | M::Harmonic
        | M::Geometric
        | M::GroupedMedian => Some(Stat::Value),
        M::StdDev | M::SeMean | M::Range => Some(Stat::Spread),
        M::Sum => Some(Stat::Sum),
        M::Variance => Some(Stat::Other),
        M::Count
        | M::Kurtosis
        | M::SeKurtosis
        | M::Skewness
        | M::SeSkewness
        | M::Npct
        | M::Spct
        | M::NpctIn(_)
        | M::SpctIn(_) => None,
    }
}

/// The grouped median (Algorithms manual, appendix Grouped Percentiles): each distinct value is
/// the midpoint of its interval, and the cumulative weight up to the midpoints, cc_l = the
/// weight below value l plus half its own, is interpolated linearly at half the total.
fn grouped_median(cells: &[(f64, f64)]) -> Option<f64> {
    let mut cells: Vec<(f64, f64)> = cells.iter().copied().filter(|(_, w)| *w > 0.0).collect();
    cells.sort_by(|a, b| a.0.total_cmp(&b.0));
    cells.dedup_by(|next, kept| {
        let same = next.0 == kept.0;
        if same {
            kept.1 += next.1;
        }
        same
    });
    let half = cells.iter().map(|(_, w)| w).sum::<f64>() / 2.0;
    let (mut below, mut previous) = (0.0, None);
    for (value, weight) in cells {
        let cumulative = below + weight / 2.0;
        if half < cumulative {
            // half the total never lies below the first midpoint, where the manual leaves the
            // percentile undefined
            let (x, c): (f64, f64) = previous?;
            let r = (half - c) / (cumulative - c);
            return Some((1.0 - r) * x + r * value);
        }
        previous = Some((value, cumulative));
        below += weight;
    }
    previous.map(|(x, _)| x)
}

/// True when the value of `variable` at `case` is valid; `include` makes user-missing valid.
pub(crate) fn is_valid(variable: &Variable, case: usize, include: bool) -> bool {
    match variable.values() {
        Values::Numeric(values) => values
            .get(case)
            .is_some_and(|v| include || !variable.is_user_missing(v)),
        Values::String(values) => include || !variable.is_user_missing_str(values.get(case)),
    }
}

/// Every choice of one name from each list, the first list outermost.
pub(crate) fn combinations(lists: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = vec![Vec::new()];
    for list in lists {
        out = out
            .iter()
            .flat_map(|prefix| {
                list.iter().map(move |name| {
                    let mut next = prefix.clone();
                    next.push(name.clone());
                    next
                })
            })
            .collect();
    }
    out
}

/// The statistics of one group of a numeric dependent: (value, weight, case) of its cases.
/// `within` holds the count and sum of the group of each `NPCT(var)` and `SPCT(var)` variable,
/// None where the report does not have that control.
fn statistic(
    cell: M,
    cases: &[(f64, f64, usize)],
    (pairs, s): (&[(f64, f64)], &Summary),
    totals: (f64, f64),
    within: &[Option<(f64, f64)>],
) -> Cell {
    let w = s.n;
    let positive = cases.iter().all(|&(x, _, _)| x > 0.0) && w > 0.0;
    let value = match cell {
        M::Mean => s.mean,
        M::Count => Some(w),
        M::StdDev => s.sd,
        M::Median => median(pairs),
        M::SeMean => s.sd.map(|sd| sd / w.sqrt()),
        M::Sum => s.sum,
        M::Min => s.min,
        M::Max => s.max,
        M::Range => s.max.zip(s.min).map(|(max, min)| max - min),
        M::Variance => s.sd.map(|sd| sd * sd),
        M::Kurtosis => s.kurtosis.filter(|_| w >= 4.0),
        M::SeKurtosis => s.se_kurtosis.filter(|_| w >= 4.0),
        M::Skewness => s.skewness.filter(|_| w >= 3.0),
        M::SeSkewness => s.se_skewness.filter(|_| w >= 3.0),
        M::First => cases.first().map(|c| c.0),
        M::Last => cases.last().map(|c| c.0),
        M::Npct => Some(100.0 * w / totals.0),
        M::Spct => s.sum.map(|sum| 100.0 * sum / totals.1),
        M::Harmonic => positive.then(|| w / pairs.iter().map(|(x, w)| w / x).sum::<f64>()),
        M::Geometric => {
            positive.then(|| (pairs.iter().map(|(x, w)| w * x.ln()).sum::<f64>() / w).exp())
        }
        M::GroupedMedian => grouped_median(pairs),
        M::NpctIn(i) => within[i].map(|t| 100.0 * w / t.0),
        M::SpctIn(i) => s.sum.zip(within[i]).map(|(sum, t)| 100.0 * sum / t.1),
    };
    number(value)
}

/// A string dependent: counts, first and last values only.
fn string_statistic(
    cell: M,
    variable: &Variable,
    cases: &[(f64, f64, usize)],
    total: f64,
    within: &[Option<(f64, f64)>],
) -> Cell {
    let Values::String(values) = variable.values() else {
        return Cell::Empty;
    };
    let w: f64 = cases.iter().map(|c| c.1).sum();
    match cell {
        M::Count => Cell::Number(w),
        M::Npct => number(Some(100.0 * w / total)),
        M::NpctIn(i) => number(within[i].map(|t| 100.0 * w / t.0)),
        M::First => cases
            .first()
            .map_or(Cell::Empty, |c| Cell::Text(values.get(c.2).to_string())),
        M::Last => cases
            .last()
            .map_or(Cell::Empty, |c| Cell::Text(values.get(c.2).to_string())),
        _ => Cell::Empty,
    }
}

/// One row of a report: for each control its category, None for Total.
type Group = Vec<Option<Key>>;

/// The groups of a report in display order: categories ascending, Total last, nested. Every
/// combination is there, also those without cases.
fn groups(keys: &[Vec<Option<Key>>], included: &[usize]) -> Vec<Group> {
    let mut rows: Vec<Group> = vec![Vec::new()];
    for level in keys {
        let found = distinct(level, included);
        let choices: Vec<Option<Key>> =
            found.into_iter().cloned().map(Some).chain([None]).collect();
        rows = rows
            .iter()
            .flat_map(|prefix| {
                choices.iter().map(move |choice| {
                    let mut next = prefix.clone();
                    next.push(choice.clone());
                    next
                })
            })
            .collect();
    }
    rows
}

/// The categories of one control among the cases, ascending: what sorting all of them and
/// dropping the repeats gives, without sorting every case.
fn distinct<'k>(level: &'k [Option<Key>], included: &[usize]) -> Vec<&'k Key> {
    struct Ordered<'k>(&'k Key);
    impl PartialEq for Ordered<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.cmp(other).is_eq()
        }
    }
    impl Eq for Ordered<'_> {}
    impl PartialOrd for Ordered<'_> {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Ordered<'_> {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.compare(other.0)
        }
    }
    let set: std::collections::BTreeSet<Ordered> = (included.iter())
        .filter_map(|&c| level[c].as_ref().map(Ordered))
        .collect();
    let mut found: Vec<&Key> = set.into_iter().map(|k| k.0).collect();
    found.dedup();
    found
}

/// The cases of every group, in case order, from one pass over the cases: each case goes to
/// the groups of its own categories and of every Total above them.
fn members(groups: &[Group], keys: &[Vec<Option<Key>>], included: &[usize]) -> Vec<Vec<usize>> {
    // the categories of each control as numbers 1, 2, ...; 0 is Total
    let found: Vec<Vec<&Key>> = keys.iter().map(|level| distinct(level, included)).collect();
    // the same equality as in_group: a binary search, then == for the values it cannot place
    let number = |l: usize, key: &Key| {
        let found = &found[l];
        let at = found.binary_search_by(|k| k.compare(key)).ok();
        at.or_else(|| found.iter().position(|k| *k == key))
            .map(|i| i + 1)
    };
    let radix: Vec<usize> = found.iter().map(|f| f.len() + 1).collect();
    let fits = radix
        .iter()
        .try_fold(1usize, |all, r| all.checked_mul(*r))
        .is_some();
    if !fits || keys.len() >= usize::BITS as usize {
        // more combinations than numbers: test every case against every group
        let of = |group: &Group| {
            (included.iter().copied())
                .filter(|&c| in_group(group, keys, c))
                .collect()
        };
        return groups.iter().map(of).collect();
    }
    let code = |numbers: &mut dyn Iterator<Item = usize>| {
        numbers
            .zip(&radix)
            .fold(0usize, |code, (n, r)| code * r + n)
    };
    let index: std::collections::HashMap<usize, usize> = (groups.iter().enumerate())
        .map(|(g, group)| {
            let mut numbers = group.iter().enumerate().map(|(l, choice)| {
                choice
                    .as_ref()
                    .map_or(0, |k| number(l, k).expect("a found category"))
            });
            (code(&mut numbers), g)
        })
        .collect();
    let mut members = vec![Vec::new(); groups.len()];
    let levels = keys.len();
    let mut own = vec![0usize; levels];
    for &case in included {
        for (l, level) in keys.iter().enumerate() {
            own[l] = level[case].as_ref().and_then(|k| number(l, k)).unwrap_or(0);
        }
        for totals in 0..1usize << levels {
            // a control the case is missing on can only be a Total
            if (0..levels).any(|l| totals & (1 << l) == 0 && own[l] == 0) {
                continue;
            }
            let mut numbers = (0..levels).map(|l| if totals & (1 << l) == 0 { own[l] } else { 0 });
            if let Some(&g) = index.get(&code(&mut numbers)) {
                members[g].push(case);
            }
        }
    }
    members
}

fn in_group(row: &Group, keys: &[Vec<Option<Key>>], case: usize) -> bool {
    row.iter()
        .zip(keys)
        .all(|(choice, level)| choice.is_none() || *choice == level[case])
}

/// Sums of one group of the analysis of variance.
struct GroupSums {
    x: f64,
    w: f64,
    sum: f64,
    /// The corrected sum of squares.
    css: f64,
}

impl Engine {
    pub(crate) fn means(&mut self, command: &Means) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let counted: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let total: f64 = counted.iter().map(|&c| weight(c)).sum();
        let (include_dependent, include_control) = match command.missing {
            MeansMissing::Include => (true, true),
            MeansMissing::Dependent => (false, true),
            _ => (false, false),
        };
        let tablewise = command.missing != MeansMissing::Separate;
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        for list in &command.tables {
            let dimensions: Vec<Vec<String>> = list
                .iter()
                .map(|items| self.expand(items))
                .collect::<Result<_, _>>()?;
            let dependents = &dimensions[0];
            let combos = combinations(&dimensions[1..]);
            for name in &command.within {
                let control = |v: &String| v.eq_ignore_ascii_case(name);
                if !dimensions[1..].iter().flatten().any(control) {
                    return Err(Error(format!(
                        "NPCT({name}) and SPCT({name}) need {name} as a control variable of every table list"
                    )));
                }
                if combos.iter().any(|combo| !combo.iter().any(control)) {
                    warnings.push(format!(
                        "the percentages within {name} are empty in the reports without {name}"
                    ));
                }
            }
            let lookup = |names: &[String]| -> Result<Vec<Variable>, Error> {
                Ok(names
                    .iter()
                    .map(|n| data.variable(n))
                    .collect::<Result<_, _>>()?)
            };
            let valid_dependent = |v: &Variable, case| is_valid(v, case, include_dependent);
            let valid_control = |v: &Variable, case| is_valid(v, case, include_control);
            // tablewise: valid on every variable of the list
            let mut complete = vec![true; data.case_count()];
            if tablewise {
                let dependent_variables = lookup(dependents)?;
                let controls: Vec<String> = dimensions[1..].iter().flatten().cloned().collect();
                let control_variables = lookup(&controls)?;
                for &case in &counted {
                    for v in &dependent_variables {
                        complete[case] &= valid_dependent(v, case);
                    }
                    for v in &control_variables {
                        complete[case] &= valid_control(v, case);
                    }
                }
            }
            let mut summary_rows = Vec::new();
            let mut reports = Vec::new();
            for dependent in dependents {
                let variable = data.variable(dependent)?;
                for combo in &combos {
                    let combo_variables = lookup(combo)?;
                    let mut included = Vec::new();
                    for &case in &counted {
                        let mut ok = complete[case] && valid_dependent(&variable, case);
                        for v in &combo_variables {
                            ok &= valid_control(v, case);
                        }
                        if ok {
                            included.push(case);
                        }
                    }
                    let n: f64 = included.iter().map(|&c| weight(c)).sum();
                    let mut name = vec![shown.named(data, dependent)];
                    name.extend(combo.iter().map(|c| shown.named(data, c)));
                    let mut row = vec![Cell::Text(name.join(" * "))];
                    for value in [n, total - n, total] {
                        row.push(Cell::Number(value));
                        row.push(number(Some(100.0 * value / total)));
                    }
                    summary_rows.push(row);
                    let controls: Vec<Variable> = combo
                        .iter()
                        .map(|name| data.variable(name))
                        .collect::<Result<_, _>>()?;
                    reports.extend(report(
                        command,
                        &variable,
                        &controls,
                        &included,
                        &weight,
                        include_control,
                        (&shown, &outline),
                        &mut warnings,
                    ));
                }
            }
            let mut columns = vec![String::new()];
            let mut column_groups = vec![Vec::new()];
            for group in ["Included", "Excluded", "Total"] {
                for leaf in ["N", "Percent"] {
                    columns.push(leaf.into());
                    column_groups.push(vec!["Cases".to_string(), group.to_string()]);
                }
            }
            out.push(Table {
                title: "Case Processing Summary".into(),
                row_headers: 1,
                columns,
                column_groups,
                rows: summary_rows,
                ..Table::default()
            });
            out.extend(reports);
        }
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}

/// The Report of one dependent by one combination of controls, then its ANOVA tables.
#[allow(clippy::too_many_arguments)]
fn report(
    command: &Means,
    dependent: &Variable,
    controls: &[Variable],
    included: &[usize],
    weight: &dyn Fn(usize) -> f64,
    include_control: bool,
    (shown, outline): (&Shown, &Shown),
    warnings: &mut Vec<String>,
) -> Vec<Table> {
    let keys: Vec<Vec<Option<Key>>> = controls
        .iter()
        .map(|v| categories(v, include_control))
        .collect();
    let values: Vec<f64> = match dependent.values() {
        Values::Numeric(values) => (0..included.last().map_or(0, |l| l + 1))
            .map(|c| values.get(c).unwrap_or(f64::NAN))
            .collect(),
        Values::String(_) => Vec::new(),
    };
    let cases_of = |group: &Group| -> Vec<(f64, f64, usize)> {
        included
            .iter()
            .filter(|&&c| in_group(group, &keys, c))
            .map(|&c| (values.get(c).copied().unwrap_or(0.0), weight(c), c))
            .collect()
    };
    let everything = cases_of(&vec![None; controls.len()]);
    let totals = (
        everything.iter().map(|c| c.1).sum::<f64>(),
        everything.iter().map(|c| c.0 * c.1).sum::<f64>(),
    );
    // the position of each NPCT(var) and SPCT(var) variable among the controls
    let positions: Vec<Option<usize>> = command
        .within
        .iter()
        .map(|name| {
            controls
                .iter()
                .position(|v| v.name().eq_ignore_ascii_case(name))
        })
        .collect();
    let mut rows = Vec::new();
    let groups = groups(&keys, included);
    let mut members = members(&groups, &keys, included);
    for (group, cases) in groups.iter().zip(members.iter_mut()) {
        // a combination without cases is not shown
        if cases.is_empty() {
            continue;
        }
        let cases: Vec<(f64, f64, usize)> = std::mem::take(cases)
            .into_iter()
            .map(|c| (values.get(c).copied().unwrap_or(0.0), weight(c), c))
            .collect();
        let pairs: Vec<(f64, f64)> = cases.iter().map(|&(x, w, _)| (x, w)).collect();
        let summary = (!dependent.is_string()).then(|| summarize(&pairs));
        // the count and sum of the cases in the same category of that control (all cases on
        // its Total rows)
        let within: Vec<Option<(f64, f64)>> = positions
            .iter()
            .map(|position| {
                position.map(|p| {
                    let mut only = vec![None; controls.len()];
                    only[p] = group[p].clone();
                    let base = cases_of(&only);
                    let w: f64 = base.iter().map(|c| c.1).sum();
                    (w, base.iter().map(|c| c.0 * c.1).sum::<f64>())
                })
            })
            .collect();
        let mut row: Vec<Cell> = group
            .iter()
            .zip(controls)
            .map(|(choice, v)| {
                Cell::Text(
                    choice
                        .as_ref()
                        .map_or("Total".into(), |k| shown.value(v, k)),
                )
            })
            .collect();
        for &cell in &command.cells {
            row.push(if dependent.is_string() {
                string_statistic(cell, dependent, &cases, totals.0, &within)
            } else {
                let summary = summary.as_ref().expect("numeric");
                statistic(cell, &cases, (&pairs, summary), totals, &within)
            });
        }
        rows.push(row);
    }
    let mut names = vec![outline.variable(dependent)];
    names.extend(controls.iter().map(|v| outline.variable(v)));
    let mut columns: Vec<String> = controls.iter().map(|v| shown.variable(v)).collect();
    columns.extend(command.cells.iter().map(|c| cell_name(*c, &command.within)));
    let formats = formats_of(&rows, |_, c| {
        let cell = c.checked_sub(controls.len()).map(|i| command.cells[i]);
        cell.and_then(kind).map_or_else(String::new, |kind| {
            procedure_format(dependent.format(), kind)
        })
    });
    let mut out = vec![Table {
        title: format!("Report: {}", names.join(" by ")),
        formats,
        row_headers: controls.len(),
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        ..Table::default()
    }];
    if command.anova {
        let title = format!(
            "{} by {}",
            names[0],
            names.get(1).map_or("", String::as_str)
        );
        // warnings name the variables
        let name = dependent.name();
        let named = format!("{name} by {}", controls.first().map_or("", |c| c.name()));
        match controls.first() {
            None => warnings.push(format!(
                "no analysis of variance for {name}: the table has no control variable"
            )),
            Some(_) if dependent.is_string() => warnings.push(format!(
                "no analysis of variance for {named}: {name} is a string variable"
            )),
            Some(control) => {
                let cell = format!(
                    "{} * {}",
                    shown.variable(dependent),
                    shown.variable(control)
                );
                out.extend(anova(
                    command,
                    control,
                    &keys[0],
                    included,
                    &values,
                    weight,
                    [&title, &cell, &named],
                    warnings,
                ))
            }
        }
    }
    out
}

/// The ANOVA table and the Measures of Association of one dependent by its first control.
#[allow(clippy::too_many_arguments)]
fn anova(
    command: &Means,
    control: &Variable,
    keys: &[Option<Key>],
    included: &[usize],
    values: &[f64],
    weight: &dyn Fn(usize) -> f64,
    [title, cell, named]: [&str; 3],
    warnings: &mut Vec<String>,
) -> Vec<Table> {
    let mut found: Vec<&Key> = included.iter().filter_map(|&c| keys[c].as_ref()).collect();
    found.sort_by(|a, b| a.compare(b));
    found.dedup();
    let sums: Vec<GroupSums> = found
        .iter()
        .map(|key| {
            let pairs: Vec<(f64, f64)> = included
                .iter()
                .filter(|&&c| keys[c].as_ref() == Some(*key))
                .map(|&c| (values[c], weight(c)))
                .collect();
            let w: f64 = pairs.iter().map(|(_, w)| w).sum();
            let sum: f64 = pairs.iter().map(|(x, w)| x * w).sum();
            let mean = sum / w;
            GroupSums {
                x: match key {
                    Key::Number(n) => *n,
                    Key::Text(_) => 0.0,
                },
                w,
                sum,
                css: pairs.iter().map(|(x, w)| w * (x - mean) * (x - mean)).sum(),
            }
        })
        .collect();
    let j = sums.len() as f64;
    if sums.len() < 2 {
        warnings.push(format!(
            "no analysis of variance for {named}: it needs at least two groups"
        ));
        return Vec::new();
    }
    let w: f64 = sums.iter().map(|g| g.w).sum();
    let sum: f64 = sums.iter().map(|g| g.sum).sum();
    let within: f64 = sums.iter().map(|g| g.css).sum();
    let between: f64 = sums.iter().map(|g| g.sum * g.sum / g.w).sum::<f64>() - sum * sum / w;
    let total_ss = between + within;
    let df_within = w - j;
    let ms_within = within / df_within;
    // kind 0: a tested source; 1: within groups (mean square only); 2: total
    let row = |source: &str, part: &str, ss: f64, df: f64, kind: u8| {
        let ms = ss / df;
        let f = ms / ms_within;
        vec![
            Cell::Text(source.into()),
            Cell::Text(part.into()),
            number(Some(ss)),
            number(Some(df)),
            number((kind < 2).then_some(ms)),
            number((kind == 0).then_some(f)),
            number((kind == 0).then(|| f_p(f, df, df_within))),
        ]
    };
    let numeric = !control.is_string();
    let linearity = command.linearity && numeric && sums.len() >= 3;
    if command.linearity && !linearity {
        warnings.push(format!(
            "no test of linearity for {named}: it needs three or more groups of a numeric control variable"
        ));
    }
    let wx: f64 = sums.iter().map(|g| g.w * g.x).sum();
    let wxx: f64 = sums.iter().map(|g| g.w * g.x * g.x).sum();
    let xy: f64 = sums.iter().map(|g| g.x * g.sum).sum();
    let sxy = xy - wx * sum / w;
    let sxx = wxx - wx * wx / w;
    let regression = sxy * sxy / sxx;
    let mut rows = vec![row("Between Groups", "(Combined)", between, j - 1.0, 0)];
    if linearity {
        rows.push(row("", "Linearity", regression, 1.0, 0));
        rows.push(row(
            "",
            "Deviation from Linearity",
            between - regression,
            j - 2.0,
            0,
        ));
    }
    rows.push(row("Within Groups", "", within, df_within, 1));
    rows.push(row("Total", "", total_ss, w - 1.0, 2));
    let columns = ["", "", "Sum of Squares", "df", "Mean Square", "F", "Sig."];
    let mut out = vec![Table {
        title: format!("ANOVA Table: {title}"),
        row_headers: 2,
        column_groups: vec![Vec::new(); columns.len()],
        columns: columns.map(String::from).into(),
        rows,
        ..Table::default()
    }];
    let eta_squared = between / total_ss;
    let mut columns = vec!["".to_string()];
    let mut row = vec![Cell::Text(cell.to_string())];
    if linearity {
        let r = sxy / (sxx * total_ss).sqrt();
        columns.extend(["R".to_string(), "R Squared".to_string()]);
        row.extend([number(Some(r)), number(Some(r * r))]);
    }
    columns.extend(["Eta".to_string(), "Eta Squared".to_string()]);
    row.extend([number(Some(eta_squared.sqrt())), number(Some(eta_squared))]);
    out.push(Table {
        title: format!("Measures of Association: {title}"),
        row_headers: 1,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows: vec![row],
        ..Table::default()
    });
    out
}

#[cfg(test)]
mod tests {
    use super::{Key, grouped_median, groups, in_group, members};

    #[test]
    fn one_pass_groups_hold_the_cases_a_scan_finds() {
        // 0 and -0 are one category; a missing key belongs to the Totals only
        let numbers = [1.0, -0.0, 2.0, 0.0, 1.0, f64::NAN, 2.0, 0.0];
        let first: Vec<Option<Key>> = (numbers.iter())
            .map(|&n| (!n.is_nan()).then_some(Key::Number(n)))
            .collect();
        let second: Vec<Option<Key>> = ["b", "a", "b", "c", "a", "a", "c", "b"]
            .iter()
            .map(|t| Some(Key::Text(t.to_string())))
            .collect();
        let keys = vec![first, second];
        let included = [0, 1, 2, 3, 4, 5, 6, 7];
        let groups = groups(&keys, &included);
        let scanned: Vec<Vec<usize>> = (groups.iter())
            .map(|g| {
                included
                    .iter()
                    .copied()
                    .filter(|&c| in_group(g, &keys, c))
                    .collect()
            })
            .collect();
        assert_eq!(members(&groups, &keys, &included), scanned);
        assert!(scanned.iter().any(Vec::is_empty) && scanned.iter().any(|m| m.len() > 1));
    }

    #[test]
    fn grouped_median_interpolates_between_midpoints() {
        // midpoints 0.5 and 2.5 of the cumulative weight; half the total is 2
        assert_eq!(grouped_median(&[(1.0, 1.0), (2.0, 3.0)]), Some(1.75));
        // half the total on a midpoint gives that value
        assert_eq!(
            grouped_median(&[(3.0, 1.0), (1.0, 1.0), (2.0, 1.0)]),
            Some(2.0)
        );
        assert_eq!(grouped_median(&[(5.0, 2.0), (5.0, 1.5)]), Some(5.0));
        assert_eq!(grouped_median(&[(5.0, 0.0)]), None);
    }
}
