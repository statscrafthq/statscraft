//! WEIGHTED KAPPA and FLEISS MULTIRATER KAPPA: agreement between raters.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - WEIGHTED KAPPA: every pair gets its own square table. Its categories are the union of the
//!   values of both raters, sorted; the agreement weight of cells d places apart among k
//!   categories is 1 - d/(k - 1) or 1 - d^2/(k - 1)^2. Kappa is (po - pe) / (1 - pe) with po and
//!   pe the weighted observed and chance proportions. The interval uses the large sample standard
//!   error of Fleiss, Cohen and Everitt; z divides kappa by the standard error under chance
//!   agreement. A pair needs two cases and two categories, and both raters of one type.
//! - FLEISS MULTIRATER KAPPA: a case missing on any rater is dropped and numeric ratings are
//!   truncated. With n raters, W subjects and p_j the share of ratings in category j, kappa
//!   compares the mean share of agreeing pairs of ratings within a subject with the sum of the
//!   p_j squared. The variance under chance agreement is the large sample one of Fleiss, Nee and
//!   Landis for the overall kappa and 2 / (W n (n - 1)) for a category's kappa.
//! - Both: case weights are rounded to whole numbers, and 0.5 or less drops the case. The
//!   interval is kappa plus and minus the normal quantile at 1/2 + level/200 times the standard
//!   error (the Fleiss chapter prints 1 - level/200, which is not a quantile of a two-sided
//!   interval; its own example table matches 1/2 + level/200).

use crate::crosstabs::Key;
use crate::descriptives::{case_weights, number};
use crate::dist::{normal_p, normal_quantile};
use crate::npar_tests::{table, text};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{FleissKappa, KappaTabulation, VarItem, WeightedKappa};

/// A case weight as a whole number; 0.5 or less becomes 0, so the case is not used.
fn whole_weight(weight: f64) -> f64 {
    if weight > 0.5 { weight.round() } else { 0.0 }
}

/// The rating of `case`, None when missing. Strings lose trailing blanks and, with
/// `ignore_case`, their case; numbers lose their fraction when `truncate`.
fn rating(
    variable: &Variable,
    case: usize,
    include: bool,
    ignore_case: bool,
    truncate: bool,
) -> Option<Key> {
    match variable.values() {
        Values::Numeric(values) => values
            .get(case)
            .filter(|v| include || !variable.is_user_missing(*v))
            .map(|v| Key::Number(if truncate { v.trunc() } else { v })),
        Values::String(values) => {
            let value = values.get(case);
            (include || !variable.is_user_missing_str(value)).then(|| {
                let value = value.trim_end();
                Key::Text(match ignore_case {
                    true => value.to_uppercase(),
                    false => value.to_string(),
                })
            })
        }
    }
}

/// How a category shows: strings compared regardless of case in the case asked for, else as
/// the variable shows the value.
fn category(
    shown: &Shown,
    variable: &Variable,
    key: &Key,
    ignore_case: bool,
    lower: bool,
) -> String {
    match key {
        Key::Text(t) if ignore_case && lower => t.to_lowercase(),
        Key::Text(t) if ignore_case => t.clone(),
        _ => shown.value(variable, key),
    }
}

/// The sorted distinct categories.
fn sorted(mut keys: Vec<Key>) -> Vec<Key> {
    keys.sort_by(|a, b| a.compare(b));
    keys.dedup();
    keys
}

fn position(categories: &[Key], key: &Key) -> usize {
    categories
        .binary_search_by(|c| c.compare(key))
        .expect("every key is a category")
}

/// The columns from kappa on: kappa, standard error, z, significance and the interval. `se0`
/// is the standard error under chance agreement, which z uses.
fn test_cells(kappa: f64, se: f64, se0: f64, level: f64) -> Vec<Cell> {
    let half = normal_quantile(0.5 + level / 200.0) * se;
    let z = kappa / se0;
    let finite = |v: f64| number(Some(v).filter(|v| v.is_finite()));
    vec![
        finite(kappa),
        finite(se),
        finite(z),
        finite(normal_p(z)),
        finite(kappa - half),
        finite(kappa + half),
    ]
}

const TEST_COLUMNS: [&str; 6] = [
    "Kappa",
    "Asymptotic Standard Error",
    "z",
    "Sig.",
    "Lower Bound",
    "Upper Bound",
];

/// A table whose last two columns are the interval at `level` percent.
fn interval_table(title: &str, columns: Vec<String>, rows: Vec<Vec<Cell>>, level: f64) -> Table {
    let n = columns.len();
    let mut t = table(title, 1, columns, rows);
    let group = format!("{level}% Asymptotic Confidence Interval");
    t.column_groups[n - 2] = vec![group.clone()];
    t.column_groups[n - 1] = vec![group];
    t
}

/// Weighted kappa of a square table of counts `f` with agreement weights `w`: kappa, its
/// asymptotic standard error and its standard error under chance agreement.
pub(crate) fn weighted_kappa(f: &[Vec<f64>], w: &[Vec<f64>]) -> (f64, f64, f64) {
    let k = f.len();
    let n: f64 = f.iter().flatten().sum();
    let cells = || (0..k).flat_map(move |i| (0..k).map(move |j| (i, j)));
    let p = |i: usize, j: usize| f[i][j] / n;
    let row: Vec<f64> = (0..k).map(|i| (0..k).map(|j| p(i, j)).sum()).collect();
    let col: Vec<f64> = (0..k).map(|j| (0..k).map(|i| p(i, j)).sum()).collect();
    let po: f64 = cells().map(|(i, j)| w[i][j] * p(i, j)).sum();
    let pe: f64 = cells().map(|(i, j)| w[i][j] * row[i] * col[j]).sum();
    // the mean weight of a row category over the columns' shares, and the other way round
    let wr: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| w[i][j] * col[j]).sum())
        .collect();
    let wc: Vec<f64> = (0..k)
        .map(|j| (0..k).map(|i| w[i][j] * row[i]).sum())
        .collect();
    let kappa = (po - pe) / (1.0 - pe);
    let a: f64 = cells()
        .map(|(i, j)| p(i, j) * (w[i][j] * (1.0 - pe) - (wr[i] + wc[j]) * (1.0 - po)).powi(2))
        .sum();
    let se =
        (a - (po * pe - 2.0 * pe + po).powi(2)).max(0.0).sqrt() / (n.sqrt() * (1.0 - pe).powi(2));
    let b: f64 = cells()
        .map(|(i, j)| row[i] * col[j] * (w[i][j] - wr[i] - wc[j]).powi(2))
        .sum();
    let se0 = (b - pe * pe).max(0.0).sqrt() / (n.sqrt() * (1.0 - pe));
    (kappa, se, se0)
}

/// The agreement weights of `k` ordered categories.
fn agreement_weights(k: usize, quadratic: bool) -> Vec<Vec<f64>> {
    let span = (k - 1) as f64;
    (0..k)
        .map(|i| {
            (0..k)
                .map(|j| {
                    let d = i.abs_diff(j) as f64 / span;
                    if quadratic { 1.0 - d * d } else { 1.0 - d }
                })
                .collect()
        })
        .collect()
}

/// The conditional probability, kappa and variance under chance agreement of one category.
type CategoryAgreement = (f64, f64, f64);

/// Fleiss' kappa from the category counts of every subject with its weight: the overall row
/// (kappa, variance under chance agreement) and per category (conditional probability, kappa,
/// variance).
pub(crate) fn fleiss(
    counts: &[(Vec<f64>, f64)],
    raters: f64,
) -> ((f64, f64), Vec<CategoryAgreement>) {
    let k = counts.first().map_or(0, |c| c.0.len());
    let w: f64 = counts.iter().map(|c| c.1).sum();
    let n = raters;
    let total = w * n;
    let p: Vec<f64> = (0..k)
        .map(|j| counts.iter().map(|(c, f)| f * c[j]).sum::<f64>() / total)
        .collect();
    let squares: Vec<f64> = (0..k)
        .map(|j| counts.iter().map(|(c, f)| f * c[j] * c[j]).sum())
        .collect();
    let observed = (squares.iter().sum::<f64>() - total) / (total * (n - 1.0));
    let chance: f64 = p.iter().map(|p| p * p).sum();
    let kappa = (observed - chance) / (1.0 - chance);
    let q: f64 = p.iter().map(|p| p * (1.0 - p)).sum();
    let skew: f64 = p.iter().map(|p| p * (1.0 - p) * (1.0 - 2.0 * p)).sum();
    let scale = 2.0 / (total * (n - 1.0));
    let variance = scale * (q * q - skew) / (q * q);
    let categories = (0..k)
        .map(|j| {
            let conditional = (squares[j] - total * p[j]) / (total * (n - 1.0) * p[j]);
            let kappa = (conditional - p[j]) / (1.0 - p[j]);
            (conditional, kappa, scale)
        })
        .collect();
    ((kappa, variance), categories)
}

/// The names of a list, each once.
fn distinct(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

impl Engine {
    fn tabulation_subset(
        &self,
        list: &[VarItem],
        of: &[String],
        what: &str,
    ) -> Result<Vec<String>, Error> {
        let names = self.expand(list)?;
        if let Some(name) = names.iter().find(|n| !of.contains(n)) {
            return Err(Error(format!(
                "WEIGHTED KAPPA /PRINT TABULATION: {name} is not on the {what} list"
            )));
        }
        Ok(names)
    }

    pub(crate) fn weighted_kappa(&mut self, command: &WeightedKappa) -> Result<Vec<Table>, Error> {
        let rows = distinct(self.expand(&command.rows)?);
        let columns = match &command.columns {
            Some(list) => Some(distinct(self.expand(list)?)),
            None => None,
        };
        let pairs: Vec<(String, String)> = match &columns {
            None => {
                if rows.len() < 2 {
                    return Err(Error(
                        "WEIGHTED KAPPA needs at least two different variables".into(),
                    ));
                }
                let pairs = rows
                    .iter()
                    .enumerate()
                    .flat_map(|(i, a)| rows[i + 1..].iter().map(move |b| (a.clone(), b.clone())));
                pairs.collect()
            }
            Some(columns) => rows
                .iter()
                .flat_map(|a| columns.iter().map(move |b| (a.clone(), b.clone())))
                .filter(|(a, b)| a != b)
                .collect(),
        };
        if pairs.is_empty() {
            return Err(Error(
                "WEIGHTED KAPPA: the lists pair no two different variables".into(),
            ));
        }
        let tabulated: Vec<bool> = match &command.tabulation {
            None => vec![false; pairs.len()],
            Some(KappaTabulation::All) => vec![true; pairs.len()],
            Some(KappaTabulation::Pairs(r, c)) => {
                let (row_list, column_list) = match &columns {
                    Some(columns) => (("row", &rows), ("column", columns)),
                    None => (("variable", &rows), ("variable", &rows)),
                };
                let r = self.tabulation_subset(r, row_list.1, row_list.0)?;
                let c = self.tabulation_subset(c, column_list.1, column_list.0)?;
                pairs
                    .iter()
                    // with one list a pair runs one way only, so either order names it
                    .map(|(a, b)| {
                        (r.contains(a) && c.contains(b))
                            || (columns.is_none() && r.contains(b) && c.contains(a))
                    })
                    .collect()
            }
        };
        let data = self.active()?;
        let weights = case_weights(data)?;
        let shown = self.shown();
        let (mut kappa_rows, mut crosstabs, mut warnings) = (Vec::new(), Vec::new(), Vec::new());
        let (mut strings, mut numbers) = (false, false);
        for ((a, b), &tabulate) in pairs.iter().zip(&tabulated) {
            let (va, vb) = (data.variable(a)?, data.variable(b)?);
            if va.is_string() != vb.is_string() {
                warnings.push(format!(
                    "WEIGHTED KAPPA: {a} and {b} are not both numeric or both string; the pair is \
                     skipped"
                ));
                continue;
            }
            let (include, ignore) = (command.include, command.ignore_case);
            let mut cells = Vec::new();
            for case in 0..data.case_count() {
                let weight = whole_weight(weights.as_ref().map_or(1.0, |w| w[case]));
                let x = rating(&va, case, include, ignore, false);
                let y = rating(&vb, case, include, ignore, false);
                if let (Some(x), Some(y), true) = (x, y, weight > 0.0) {
                    cells.push((x, y, weight));
                }
            }
            let keys = cells.iter().flat_map(|(x, y, _)| [x.clone(), y.clone()]);
            let categories = sorted(keys.collect());
            let k = categories.len();
            let total: f64 = cells.iter().map(|c| c.2).sum();
            if total < 2.0 || k < 2 {
                warnings.push(format!(
                    "WEIGHTED KAPPA: {a} and {b} need at least two cases and two categories; the \
                     pair is skipped"
                ));
                continue;
            }
            let mut f = vec![vec![0.0; k]; k];
            for (x, y, weight) in &cells {
                f[position(&categories, x)][position(&categories, y)] += weight;
            }
            let (kappa, se, se0) = weighted_kappa(&f, &agreement_weights(k, command.quadratic));
            let mut row = vec![
                text(format!("{} - {}", shown.variable(&va), shown.variable(&vb))),
                Cell::Number(total),
            ];
            row.extend(test_cells(kappa, se, se0, command.level));
            kappa_rows.push(row);
            strings |= va.is_string();
            numbers |= !va.is_string();
            if tabulate {
                let mut order: Vec<usize> = (0..k).collect();
                if command.descending {
                    order.reverse();
                }
                let show = |v: &Variable, i: usize| {
                    category(&shown, v, &categories[i], ignore, command.lower)
                };
                let mut columns = vec![shown.variable(&va)];
                columns.extend(order.iter().map(|&j| show(&vb, j)));
                columns.push("Total".into());
                let mut body: Vec<Vec<Cell>> = order
                    .iter()
                    .map(|&i| {
                        let mut row = vec![text(show(&va, i))];
                        row.extend(order.iter().map(|&j| Cell::Number(f[i][j])));
                        row.push(Cell::Number(f[i].iter().sum()));
                        row
                    })
                    .collect();
                let mut last = vec![text("Total")];
                last.extend(
                    order
                        .iter()
                        .map(|&j| Cell::Number((0..k).map(|i| f[i][j]).sum())),
                );
                last.push(Cell::Number(total));
                body.push(last);
                let title = format!(
                    "{} * {} Crosstabulation",
                    shown.variable(&va),
                    shown.variable(&vb)
                );
                let mut t = table(title, 1, columns, body);
                for group in &mut t.column_groups[1..=k] {
                    *group = vec![shown.variable(&vb)];
                }
                crosstabs.push(t);
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        let mut tables = Vec::new();
        if !kappa_rows.is_empty() {
            let mut columns = vec!["Ratings".to_string(), "N".into(), "Weighted Kappa".into()];
            columns.extend(TEST_COLUMNS[1..].iter().map(|c| c.to_string()));
            let mut t = interval_table("Weighted Kappa", columns, kappa_rows, command.level);
            let mut caption = String::from(match command.quadratic {
                true => {
                    "Quadratic agreement weights. z uses the standard error under chance agreement."
                }
                false => {
                    "Linear agreement weights. z uses the standard error under chance agreement."
                }
            });
            if strings {
                caption.push_str(match command.ignore_case {
                    true => " Strings are compared regardless of case.",
                    false => " Strings are compared with their case.",
                });
                caption.push_str(
                    " String categories are in alphabetical order, which may differ from the order \
                     of the scale.",
                );
            }
            if numbers && strings {
                caption.push_str(" Numeric categories are in numeric order.");
            }
            t.caption = Some(caption);
            tables.push(t);
        }
        tables.extend(crosstabs);
        Ok(tables)
    }

    pub(crate) fn fleiss_kappa(&mut self, command: &FleissKappa) -> Result<Vec<Table>, Error> {
        let names = self.expand(&command.variables)?;
        let repeated = names
            .iter()
            .enumerate()
            .find(|(i, n)| names[..*i].contains(n));
        if let Some((_, name)) = repeated {
            return Err(Error(format!(
                "FLEISS MULTIRATER KAPPA: {name} is named twice; each rater is one variable"
            )));
        }
        if names.len() < 2 {
            return Err(Error(
                "FLEISS MULTIRATER KAPPA needs at least two rating variables".into(),
            ));
        }
        let data = self.active()?;
        let variables: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let string = variables[0].is_string();
        if variables.iter().any(|v| v.is_string() != string) {
            return Err(Error(
                "FLEISS MULTIRATER KAPPA needs raters that are all numeric or all string".into(),
            ));
        }
        let weights = case_weights(data)?;
        let mut subjects: Vec<(Vec<Key>, f64)> = Vec::new();
        for case in 0..data.case_count() {
            let weight = whole_weight(weights.as_ref().map_or(1.0, |w| w[case]));
            if weight == 0.0 {
                continue;
            }
            let ratings: Option<Vec<Key>> = variables
                .iter()
                .map(|v| rating(v, case, command.include, command.ignore_case, true))
                .collect();
            if let Some(ratings) = ratings {
                subjects.push((ratings, weight));
            }
        }
        let categories = sorted(subjects.iter().flat_map(|s| s.0.clone()).collect());
        let w: f64 = subjects.iter().map(|s| s.1).sum();
        if w < 2.0 {
            self.warn("FLEISS MULTIRATER KAPPA: fewer than two complete subjects; no kappa");
            return Ok(Vec::new());
        }
        if categories.len() < 2 {
            self.warn("FLEISS MULTIRATER KAPPA: every rating is the same; no kappa");
            return Ok(Vec::new());
        }
        let counts: Vec<(Vec<f64>, f64)> = subjects
            .iter()
            .map(|(ratings, weight)| {
                let mut c = vec![0.0; categories.len()];
                for r in ratings {
                    c[position(&categories, r)] += 1.0;
                }
                (c, *weight)
            })
            .collect();
        let n = names.len();
        let ((kappa, variance), per_category) = fleiss(&counts, n as f64);
        let mut caption = format!("{w} subjects and {n} raters.");
        if string {
            caption.push_str(match command.ignore_case {
                true => " Rating categories are compared regardless of case.",
                false => " Rating categories are compared with their case.",
            });
        }
        let se = variance.sqrt();
        let mut columns = vec![String::new()];
        columns.extend(TEST_COLUMNS.iter().map(|c| c.to_string()));
        let mut row = vec![text("Overall Agreement")];
        row.extend(test_cells(kappa, se, se, command.level));
        let mut overall = interval_table("Overall Agreement", columns, vec![row], command.level);
        overall.caption = Some(caption.clone());
        let mut tables = vec![overall];
        if command.category {
            let shown = self.shown();
            let rows = per_category
                .iter()
                .zip(&categories)
                .map(|((conditional, kappa, variance), key)| {
                    let label = category(
                        &shown,
                        &variables[0],
                        key,
                        command.ignore_case,
                        command.lower,
                    );
                    let mut row = vec![text(label), number(Some(*conditional))];
                    let se = variance.sqrt();
                    row.extend(test_cells(*kappa, se, se, command.level));
                    row
                })
                .collect();
            let mut columns = vec![
                "Rating Category".to_string(),
                "Conditional Probability".into(),
            ];
            columns.extend(TEST_COLUMNS.iter().map(|c| c.to_string()));
            let mut t = interval_table(
                "Agreement on Individual Categories",
                columns,
                rows,
                command.level,
            );
            t.caption = Some(caption);
            tables.push(t);
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_round_and_drop_small_ones() {
        assert_eq!(whole_weight(0.5), 0.0);
        assert_eq!(whole_weight(0.6), 1.0);
        assert_eq!(whole_weight(2.5), 3.0);
        assert_eq!(whole_weight(-1.0), 0.0);
    }

    #[test]
    fn perfect_agreement_gives_kappa_one() {
        let f = vec![
            vec![3.0, 0.0, 0.0],
            vec![0.0, 4.0, 0.0],
            vec![0.0, 0.0, 5.0],
        ];
        let (kappa, se, _) = weighted_kappa(&f, &agreement_weights(3, false));
        assert!((kappa - 1.0).abs() < 1e-12);
        assert!(se.abs() < 1e-12);
    }
}
