//! BOOTSTRAP: bias, standard errors and confidence intervals from resamples, for the next
//! procedure.
//!
//! Rules from the description of BOOTSTRAP and the bootstrapping algorithms, in our words:
//! - BOOTSTRAP waits for the next procedure and applies to that one only.
//! - A case with a missing value in a TARGET, INPUT or STRATA variable leaves the analysis, the
//!   run on the original data included: these variables fix the cases, whatever the procedure's
//!   own MISSING says. System-missing is always missing; a user-missing value is missing unless
//!   USERMISSING=INCLUDE and the variable is nominal or ordinal (strings count as nominal).
//! - A resample draws, with replacement, as many cases as there are; STRATIFIED draws within
//!   each combination of the strata variables as many cases as it holds. Under WEIGHT the
//!   weights are frequencies: a resample draws the total weight (rounded) in units, each case
//!   with a chance in proportion to its weight.
//! - Bias is the mean of the resample values less the original value; the standard error is
//!   their standard deviation, divisor B - 1.
//! - Percentile interval with tails a = (1 - level) / 2: the ordered resample values at ranks
//!   (B + 1) a and (B + 1)(1 - a). Between two ranks the value is interpolated on the normal
//!   quantile scale; below the first rank or past the last one the extreme value stands.
//! - BCa: w = Phi^-1(#(T* <= T) / (B + 1)). The jackknife leaves one case (one unit of a
//!   weighted case) out at a time: l = (N_s - 1)(T - T_-k) in stratum s of N_s, times N / N_s.
//!   The acceleration is a = sum f l^3 / (6 (sum f l^2)^1.5), f the case's frequency, and each
//!   tail a' = Phi(w + (w + z) / (1 - a (w + z))) with z = Phi^-1(a) replaces a in the ranks.
//!
//! Choices where the manual is silent (docs/differences.md): the draws are uniform picks from
//! the session's generator; a resample in which the procedure fails or lacks a statistic leaves
//! that statistic out, and B counts the resamples that gave it; the statistics bootstrapped and
//! the layout of the extra columns and rows.

use crate::dist::{normal_cdf, normal_quantile};
use crate::random::Random;
use crate::{Cell, Engine, Error, Table, cell_text};
use statscraft_data::{Dataset, Measure, Values};
use statscraft_syntax::{Bootstrap, Command};
use std::collections::HashMap;

/// Where a procedure's bootstrapped statistics stand: the rows or the columns labelled with
/// one of `labels` in the tables whose title starts with `title`.
struct Target {
    title: &'static str,
    rows: bool,
    labels: &'static [&'static str],
}

const SPREAD: &[&str] = &["Mean", "Std Dev", "Variance", "Skewness", "Kurtosis"];

fn targets(command: &Command) -> Option<&'static [Target]> {
    Some(match command {
        Command::Descriptives(_) => &[Target {
            title: "Descriptive Statistics",
            rows: false,
            labels: SPREAD,
        }],
        Command::Frequencies(_) => &[Target {
            title: "Statistics",
            rows: true,
            labels: &[
                "Mean",
                "Median",
                "Std. Deviation",
                "Variance",
                "Skewness",
                "Kurtosis",
                "Percentiles",
            ],
        }],
        Command::Means(_) => &[Target {
            title: "Report",
            rows: false,
            labels: &[
                "Mean",
                "Median",
                "Grouped Median",
                "Std. Deviation",
                "Variance",
                "Skewness",
                "Kurtosis",
                "Harmonic Mean",
                "Geometric Mean",
            ],
        }],
        Command::Correlations(_) => &[Target {
            title: "Correlations",
            rows: true,
            labels: &["Pearson Correlation"],
        }],
        Command::TTest(_) => &[Target {
            title: "",
            rows: false,
            labels: &["Mean", "Std. Deviation", "Mean Difference"],
        }],
        Command::Regression(_) => &[Target {
            title: "Coefficients",
            rows: false,
            labels: &["B"],
        }],
        _ => return None,
    })
}

/// Whether BOOTSTRAP adds results to this procedure.
pub(crate) fn supports(command: &Command) -> bool {
    targets(command).is_some()
}

/// The procedure's name for a message, from its syntax tree: ONEWAY, NPARTESTS.
pub(crate) fn name(command: &Command) -> String {
    let text = format!("{command:?}");
    let name = text.split(['(', ' ', '{']).next().unwrap_or_default();
    name.to_uppercase()
}

/// One bootstrapped cell of the original tables, and how to find it again in a resample's.
struct Spot {
    table: usize,
    row: usize,
    col: usize,
    rows: bool,
    /// (table title, how many tables of that title came before, row key, column title)
    key: (String, usize, String, String),
}

/// Every row's labels, the outer ones carried down over blank cells.
fn row_labels(table: &Table) -> Vec<Vec<String>> {
    let mut outer = vec![String::new(); table.row_headers];
    (0..table.rows.len())
        .map(|r| {
            (0..table.row_headers)
                .map(|h| {
                    let text = cell_text(table, r, h);
                    if h + 1 == table.row_headers {
                        return text;
                    }
                    if !text.is_empty() {
                        outer[h] = text;
                    }
                    outer[h].clone()
                })
                .collect()
        })
        .collect()
}

/// A key per row that a resample's table gives the same row: its labels, numbered when they
/// repeat.
fn row_keys(table: &Table) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    row_labels(table)
        .into_iter()
        .map(|labels| {
            let key = labels.join("\u{1f}");
            let n = seen.entry(key.clone()).or_insert(0);
            *n += 1;
            format!("{key}\u{1f}{n}")
        })
        .collect()
}

/// (title, occurrence) of every table.
fn table_keys(tables: &[Table]) -> Vec<(String, usize)> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    tables
        .iter()
        .map(|t| {
            let n = seen.entry(&t.title).or_insert(0);
            *n += 1;
            (t.title.clone(), *n)
        })
        .collect()
}

fn spots(tables: &[Table], targets: &[Target]) -> Vec<Spot> {
    let mut out = Vec::new();
    for (t, (table, (title, occurrence))) in tables.iter().zip(table_keys(tables)).enumerate() {
        let Some(target) = targets.iter().find(|g| table.title.starts_with(g.title)) else {
            continue;
        };
        let labels = row_labels(table);
        let keys = row_keys(table);
        let titles = table.column_titles();
        let chosen_column = |c: usize| {
            let leaf = table.columns[c].as_str();
            let group = table.column_groups.get(c).and_then(|g| g.last());
            target.labels.contains(&leaf)
                || (leaf == "Statistic" && group.is_some_and(|g| target.labels.contains(&&**g)))
        };
        for (r, row) in table.rows.iter().enumerate() {
            let chosen_row = labels[r]
                .iter()
                .any(|l| target.labels.contains(&l.as_str()));
            for (c, column) in titles.iter().enumerate().skip(table.row_headers) {
                let chosen = match target.rows {
                    true => chosen_row,
                    false => chosen_column(c),
                };
                if chosen && matches!(row.get(c), Some(Cell::Number(n)) if n.is_finite()) {
                    out.push(Spot {
                        table: t,
                        row: r,
                        col: c,
                        rows: target.rows,
                        key: (title.clone(), occurrence, keys[r].clone(), column.clone()),
                    });
                }
            }
        }
    }
    out
}

/// A table with the place of each row key and column title.
type Located<'a> = (&'a Table, HashMap<String, usize>, HashMap<String, usize>);

/// The values of the spots in a resample's tables; NaN where a value is missing.
fn lookup(tables: &[Table], spots: &[Spot]) -> Vec<f64> {
    let mut at: HashMap<(&str, usize), Located> = HashMap::new();
    for (table, (title, occurrence)) in tables.iter().zip(table_keys(tables)) {
        if !spots.iter().any(|s| s.key.0 == title) {
            continue;
        }
        let rows = row_keys(table).into_iter().enumerate();
        let rows = rows.map(|(r, k)| (k, r)).collect();
        let cols = table.column_titles().into_iter().enumerate();
        let cols = cols.map(|(c, k)| (k, c)).collect();
        at.insert((&table.title, occurrence), (table, rows, cols));
    }
    spots
        .iter()
        .map(|s| {
            let (title, occurrence, row, col) = &s.key;
            let Some((table, rows, cols)) = at.get(&(title.as_str(), *occurrence)) else {
                return f64::NAN;
            };
            match (rows.get(row), cols.get(col)) {
                (Some(&r), Some(&c)) => match table.rows[r].get(c) {
                    Some(Cell::Number(n)) => *n,
                    _ => f64::NAN,
                },
                _ => f64::NAN,
            }
        })
        .collect()
}

/// The bootstrap results of one statistic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pooled {
    pub bias: f64,
    pub std_error: f64,
    pub lower: f64,
    pub upper: f64,
}

/// The value at rank (B + 1) `alpha` of the ordered resample values, interpolated on the
/// normal quantile scale between ranks; the extreme value outside them.
pub(crate) fn rank_value(sorted: &[f64], alpha: f64) -> f64 {
    let b = sorted.len();
    let rank = (b + 1) as f64 * alpha;
    let nearest = rank.round();
    if (rank - nearest).abs() <= 1e-9 * nearest.max(1.0) {
        return sorted[(nearest as usize).clamp(1, b) - 1];
    }
    let k = rank.floor();
    if k < 1.0 {
        return sorted[0];
    }
    let k = k as usize;
    if k >= b {
        return sorted[b - 1];
    }
    let q = |i: usize| normal_quantile(i as f64 / (b + 1) as f64);
    let share = (normal_quantile(alpha) - q(k)) / (q(k + 1) - q(k));
    sorted[k - 1] + share * (sorted[k] - sorted[k - 1])
}

/// Bias, standard error and interval of `t` from its resample values (NaN ones left out) at
/// `level` percent; `acceleration` gives BCa, None the percentile interval. None with fewer
/// than two values.
pub(crate) fn pool(
    t: f64,
    values: &[f64],
    level: f64,
    acceleration: Option<f64>,
) -> Option<Pooled> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.len() < 2 {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let b = sorted.len() as f64;
    let mean = sorted.iter().sum::<f64>() / b;
    let variance = sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (b - 1.0);
    let alpha = (1.0 - level / 100.0) / 2.0;
    let (low, high) = match acceleration {
        None => (alpha, 1.0 - alpha),
        Some(a) => {
            let below = sorted.iter().filter(|&&v| v <= t).count() as f64;
            let w = normal_quantile(below / (b + 1.0));
            let tail = |p: f64| {
                let z = normal_quantile(p);
                let adjusted = normal_cdf(w + (w + z) / (1.0 - a * (w + z)));
                // no resample at or below t: w is minus infinity and the tail the lowest
                match adjusted.is_nan() {
                    true => 0.0,
                    false => adjusted,
                }
            };
            (tail(alpha), tail(1.0 - alpha))
        }
    };
    Some(Pooled {
        bias: mean - t,
        std_error: variance.sqrt(),
        lower: rank_value(&sorted, low),
        upper: rank_value(&sorted, high),
    })
}

/// The BCa acceleration from jackknife influence values and frequencies.
pub(crate) fn acceleration(influence: &[(f64, f64)]) -> f64 {
    let (mut cubes, mut squares) = (0.0, 0.0);
    for &(l, f) in influence.iter().filter(|(l, _)| l.is_finite()) {
        cubes += f * l.powi(3);
        squares += f * l * l;
    }
    match squares > 0.0 {
        true => cubes / (6.0 * squares.powf(1.5)),
        false => 0.0,
    }
}

/// The case picks of one resample: per stratum as many cases as it holds (its total weight,
/// rounded, under WEIGHT, each case drawn in proportion to its weight).
pub(crate) fn draw(
    random: &mut Random,
    strata: &[Vec<usize>],
    weights: Option<&[f64]>,
) -> Vec<Option<usize>> {
    let mut picks = Vec::new();
    for stratum in strata.iter().filter(|s| !s.is_empty()) {
        let n = stratum.len();
        match weights {
            None => {
                for _ in 0..n {
                    let i = (random.uniform() * n as f64) as usize;
                    picks.push(Some(stratum[i.min(n - 1)]));
                }
            }
            Some(weights) => {
                let cumulative: Vec<f64> = stratum
                    .iter()
                    .scan(0.0, |sum, &c| {
                        *sum += weights[c];
                        Some(*sum)
                    })
                    .collect();
                let total = cumulative[n - 1];
                for _ in 0..total.round() as usize {
                    let u = random.uniform() * total;
                    let i = cumulative.partition_point(|&c| c <= u);
                    picks.push(Some(stratum[i.min(n - 1)]));
                }
            }
        }
    }
    picks
}

/// Whether a case's value of `name` counts as valid for the case basis.
fn valid(data: &Dataset, name: &str, case: usize, include: bool) -> Result<bool, Error> {
    let variable = data.variable(name)?;
    let categorical = matches!(variable.measure(), Measure::Nominal | Measure::Ordinal);
    Ok(match variable.values() {
        Values::Numeric(values) => values
            .get(case)
            .is_some_and(|v| !variable.is_user_missing(v) || (include && categorical)),
        Values::String(values) => !variable.is_user_missing_str(values.get(case)) || include,
    })
}

/// The strata of the cases: per combination of the values of `names`, its cases in order.
fn strata(data: &Dataset, names: &[String]) -> Result<Vec<Vec<usize>>, Error> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Vec<usize>> = Vec::new();
    for case in 0..data.case_count() {
        let mut key = String::new();
        for name in names {
            match data.variable(name)?.values() {
                Values::Numeric(v) => key.push_str(&format!("{:?}\u{1f}", v.get(case))),
                Values::String(v) => key.push_str(&format!("{}\u{1f}", v.get(case))),
            }
        }
        let next = out.len();
        let i = *index.entry(key).or_insert(next);
        if i == next {
            out.push(Vec::new());
        }
        out[i].push(case);
    }
    Ok(out)
}

impl Engine {
    /// The errors BOOTSTRAP finds before the procedure runs: imputed data, or a split variable
    /// among its variables.
    pub(crate) fn check_bootstrap(&self, spec: &Bootstrap) -> Result<(), Error> {
        let data = self.active()?;
        if data
            .variable_index(crate::multiple_imputation::IMPUTATION)
            .is_some()
        {
            return Err(Error(
                "BOOTSTRAP cannot run on multiply imputed data (the dataset has an Imputation_ \
                 variable)"
                    .into(),
            ));
        }
        let mut names = self.expand(&spec.strata)?;
        names.extend(self.expand(&spec.target)?);
        names.extend(self.expand(&spec.input)?);
        let split = data
            .split_file()
            .map_or(&[][..], |s| s.variables.as_slice());
        if let Some(name) = names
            .iter()
            .find(|n| split.iter().any(|s| s.eq_ignore_ascii_case(n)))
        {
            return Err(Error(format!(
                "BOOTSTRAP cannot use the split variable {name}"
            )));
        }
        Ok(())
    }

    /// Runs the procedure on the data again with an empty dataset swapped in: the values of
    /// the spots, NaN where it failed. Warnings and charts of the run are dropped.
    fn rerun(&mut self, command: &Command, data: Dataset, spots: &[Spot]) -> Vec<f64> {
        let charts = self.charts.len();
        self.dataset = Some(data);
        let result = self.run_procedure(command);
        self.charts.truncate(charts);
        self.warnings.clear();
        match result {
            Ok(tables) => lookup(&tables, spots),
            Err(_) => vec![f64::NAN; spots.len()],
        }
    }

    /// The procedure under BOOTSTRAP, on the current dataset (one split group's).
    pub(crate) fn bootstrapped(
        &mut self,
        command: &Command,
        spec: &Bootstrap,
    ) -> Result<Vec<Table>, Error> {
        let full = self.active()?.clone();
        let strata_names = self.expand(&spec.strata)?;
        let mut checked = strata_names.clone();
        checked.extend(self.expand(&spec.target)?);
        checked.extend(self.expand(&spec.input)?);
        let weights = crate::descriptives::case_weights(&full)?;
        let mut kept = Vec::new();
        for case in 0..full.case_count() {
            let mut ok = weights.as_ref().is_none_or(|w| w[case] > 0.0);
            for name in &checked {
                ok = ok && valid(&full, name, case, spec.include_user_missing)?;
            }
            if ok {
                kept.push(case);
            }
        }
        let reduced = kept.len() < full.case_count();
        let mut data = full.clone();
        if reduced {
            data.take_cases(&kept.iter().map(|&c| Some(c)).collect::<Vec<_>>())?;
        }

        // the original run, on the cases BOOTSTRAP keeps
        let numbers = self.case_numbers.take();
        if reduced {
            let whole = |c: usize| numbers.as_ref().map_or(c, |n| n[c]);
            self.case_numbers = Some(kept.iter().map(|&c| whole(c)).collect());
        } else {
            self.case_numbers = numbers.clone();
        }
        let charts = self.charts.len();
        self.dataset = Some(data.clone());
        let result = self.run_procedure(command);
        self.case_numbers = None;
        let after = self.dataset.take().expect("procedures keep a dataset");
        let after = match (&result, reduced) {
            (Err(_), _) => full.clone(),
            (Ok(_), true) => crate::split::write_back_cases(full.clone(), kept, &after)?,
            (Ok(_), false) => after,
        };
        let mut tables = result.inspect_err(|_| self.dataset = Some(full.clone()))?;
        // the charts of FREQUENCIES are left out under BOOTSTRAP, as the manual says
        if matches!(command, Command::Frequencies(_)) {
            self.charts.truncate(charts);
        }
        let spots = spots(&tables, targets(command).unwrap_or_default());
        if spots.is_empty() || data.case_count() == 0 {
            self.dataset = Some(after);
            self.case_numbers = numbers;
            return Ok(tables);
        }
        let original: Vec<f64> = spots
            .iter()
            .map(|s| match tables[s.table].rows[s.row][s.col] {
                Cell::Number(n) => n,
                _ => f64::NAN,
            })
            .collect();

        let warnings = std::mem::take(&mut self.warnings);
        let groups = strata(&data, &strata_names)?;
        let case_weights = crate::descriptives::case_weights(&data)?;
        let weight_name = data.weight_variable().map(str::to_string);
        let mut values: Vec<Vec<f64>> = vec![Vec::new(); spots.len()];
        for _ in 0..spec.samples {
            let picks = draw(
                &mut self.random.borrow_mut(),
                &groups,
                case_weights.as_deref(),
            );
            let mut sample = data.clone();
            sample.take_cases(&picks)?;
            if let Some(name) = &weight_name {
                sample.set_numeric_values(name, vec![Some(1.0); picks.len()])?;
            }
            for (v, x) in values.iter_mut().zip(self.rerun(command, sample, &spots)) {
                v.push(x);
            }
        }
        let accelerations: Option<Vec<f64>> = match spec.bca {
            false => None,
            true => {
                let frequency = |c: usize| case_weights.as_ref().map_or(1.0, |w| w[c]);
                let total: f64 = (0..data.case_count()).map(frequency).sum();
                let mut influence: Vec<Vec<(f64, f64)>> = vec![Vec::new(); spots.len()];
                for stratum in &groups {
                    let size: f64 = stratum.iter().map(|&c| frequency(c)).sum();
                    for &case in stratum {
                        let f = frequency(case);
                        let mut sample = data.clone();
                        match (&weight_name, &case_weights) {
                            (Some(name), Some(w)) if f > 1.0 => {
                                let mut less: Vec<Option<f64>> =
                                    w.iter().map(|&x| Some(x)).collect();
                                less[case] = Some(f - 1.0);
                                sample.set_numeric_values(name, less)?;
                            }
                            _ => {
                                let keep: Vec<bool> =
                                    (0..data.case_count()).map(|c| c != case).collect();
                                sample.select_cases(&keep)?;
                            }
                        }
                        let left = self.rerun(command, sample, &spots);
                        for ((list, t), x) in influence.iter_mut().zip(&original).zip(left) {
                            let l = (size - 1.0) * (t - x) * total / size;
                            list.push((l, f));
                        }
                    }
                }
                Some(influence.iter().map(|l| acceleration(l)).collect())
            }
        };
        self.dataset = Some(after);
        self.case_numbers = numbers;
        self.warnings = warnings;

        let level = spec.cilevel;
        let short = values
            .iter()
            .filter(|v| v.iter().any(|x| !x.is_finite()))
            .count();
        if short > 0 {
            self.warn(format!(
                "BOOTSTRAP: in some resamples the procedure gave no value for {short} of the \
                 bootstrapped statistics; their results rest on the resamples that did"
            ));
        }
        if level > 100.0 * (1.0 - 2.0 / (spec.samples as f64 + 1.0)) {
            self.warn(format!(
                "BOOTSTRAP: {} resamples are too few for a {level}% interval; its limits are the \
                 extreme resample values",
                spec.samples
            ));
        }
        let pooled: Vec<Option<Pooled>> = (0..spots.len())
            .map(|i| {
                let a = accelerations.as_ref().map(|a| a[i]);
                pool(original[i], &values[i], level, a)
            })
            .collect();
        annotate(&mut tables, &spots, &pooled, spec);
        Ok(tables)
    }
}

/// Adds the Bootstrap columns or rows to the tables.
fn annotate(tables: &mut [Table], spots: &[Spot], pooled: &[Option<Pooled>], spec: &Bootstrap) {
    let level = match spec.cilevel.fract() == 0.0 {
        true => format!("{:.0}", spec.cilevel),
        false => format!("{}", spec.cilevel),
    };
    let interval = match spec.bca {
        true => format!("BCa {level}% Confidence Interval"),
        false => format!("{level}% Confidence Interval"),
    };
    let caption = format!(
        "Bootstrap results are based on {} {}resamples.",
        spec.samples,
        if spec.strata.is_empty() {
            ""
        } else {
            "stratified "
        }
    );
    let cells = |p: Option<&Pooled>| match p {
        Some(p) => [p.bias, p.std_error, p.lower, p.upper].map(Cell::Number),
        None => std::array::from_fn(|_| Cell::Empty),
    };
    for (t, table) in tables.iter_mut().enumerate() {
        let mine: Vec<(&Spot, Option<&Pooled>)> = spots
            .iter()
            .zip(pooled)
            .filter(|(s, _)| s.table == t)
            .map(|(s, p)| (s, p.as_ref()))
            .collect();
        let Some((first, _)) = mine.first() else {
            continue;
        };
        let has_formats = !table.formats.is_empty();
        let format = |table: &Table, r: usize, c: usize| {
            table
                .formats
                .get(r)
                .and_then(|f| f.get(c))
                .cloned()
                .unwrap_or_default()
        };
        if first.rows {
            let mut rows: Vec<usize> = mine.iter().map(|(s, _)| s.row).collect();
            rows.dedup();
            for &r in rows.iter().rev() {
                let width = table.columns.len();
                let names = [
                    "Bootstrap Bias".to_string(),
                    "Bootstrap Std. Error".to_string(),
                    format!("Bootstrap {interval} Lower"),
                    format!("Bootstrap {interval} Upper"),
                ];
                for (k, name) in names.into_iter().enumerate().rev() {
                    let mut row = vec![Cell::Empty; width];
                    row[table.row_headers - 1] = Cell::Text(name);
                    let mut formats = vec![String::new(); width];
                    for (s, p) in mine.iter().filter(|(s, _)| s.row == r) {
                        let [bias, se, lower, upper] = cells(*p);
                        row[s.col] = [bias, se, lower, upper].into_iter().nth(k).expect("four");
                        formats[s.col] = format(table, r, s.col);
                    }
                    table.rows.insert(r + 1, row);
                    if has_formats {
                        table.formats.insert(r + 1, formats);
                    }
                }
            }
        } else {
            table.column_groups.resize(table.columns.len(), Vec::new());
            let mut cols: Vec<usize> = mine.iter().map(|(s, _)| s.col).collect();
            cols.sort_unstable();
            cols.dedup();
            for &c in cols.iter().rev() {
                let leaf = std::mem::replace(&mut table.columns[c], "Statistic".into());
                let mut groups = table.column_groups[c].clone();
                if leaf != "Statistic" {
                    groups.push(leaf);
                    table.column_groups[c] = groups.clone();
                }
                groups.push("Bootstrap".into());
                let mut inner = groups.clone();
                inner.push(interval.clone());
                let added = [
                    (groups.clone(), "Bias"),
                    (groups, "Std. Error"),
                    (inner.clone(), "Lower"),
                    (inner, "Upper"),
                ];
                for (k, (path, name)) in added.into_iter().enumerate() {
                    table.columns.insert(c + 1 + k, name.into());
                    table.column_groups.insert(c + 1 + k, path);
                }
                for r in 0..table.rows.len() {
                    let p = mine.iter().find(|(s, _)| s.row == r && s.col == c);
                    let new = match p {
                        Some((_, p)) => cells(*p),
                        None => std::array::from_fn(|_| Cell::Empty),
                    };
                    let f = match p {
                        Some(_) => format(table, r, c),
                        None => String::new(),
                    };
                    for (k, cell) in new.into_iter().enumerate() {
                        table.rows[r].insert(c + 1 + k, cell);
                        if has_formats && let Some(formats) = table.formats.get_mut(r) {
                            formats.insert(c + 1 + k, f.clone());
                        }
                    }
                }
            }
        }
        table.caption = Some(match table.caption.take() {
            Some(text) => format!("{text}\n{caption}"),
            None => caption.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_interpolate_on_the_normal_scale_and_stop_at_the_extremes() {
        let sorted: Vec<f64> = (1..=9).map(f64::from).collect();
        // (9 + 1) * 0.3 = 3: the third value exactly
        assert_eq!(rank_value(&sorted, 0.3), 3.0);
        assert_eq!(rank_value(&sorted, 0.01), 1.0);
        assert_eq!(rank_value(&sorted, 0.99), 9.0);
        // rank 3.5 lies between the third and fourth values, nearer the fourth on this scale
        let x = rank_value(&sorted, 0.35);
        let q = |p: f64| normal_quantile(p);
        let expected = 3.0 + (q(0.35) - q(0.3)) / (q(0.4) - q(0.3));
        assert!((x - expected).abs() < 1e-15);
    }

    #[test]
    fn stratified_draws_keep_each_stratum_size() {
        let mut random = Random::seeded(20_261_118);
        let strata = vec![vec![0, 2, 4], vec![1, 3], vec![5]];
        for _ in 0..50 {
            let picks = draw(&mut random, &strata, None);
            let picks: Vec<usize> = picks.into_iter().flatten().collect();
            assert_eq!(picks.len(), 6);
            assert!(picks[..3].iter().all(|c| [0, 2, 4].contains(c)));
            assert!(picks[3..5].iter().all(|c| [1, 3].contains(c)));
            assert_eq!(picks[5], 5);
        }
    }

    #[test]
    fn weighted_draws_take_the_total_weight_in_units() {
        let mut random = Random::seeded(77);
        let weights = [3.0, 0.0, 2.0];
        let picks = draw(&mut random, &[vec![0, 1, 2]], Some(&weights));
        assert_eq!(picks.len(), 5);
        assert!(picks.iter().flatten().all(|&c| c != 1));
    }
}
