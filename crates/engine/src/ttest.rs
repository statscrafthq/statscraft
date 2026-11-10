//! T-TEST: one-sample, independent-samples and paired-samples t tests, with effect sizes.
//!
//! Rules from the Command Syntax Reference (T-TEST) and the Algorithms manual (T Test, and
//! Effect Size for t-Test):
//! - Weights are used as they are for the tests. The effect sizes use frequency weights
//!   rounded to whole numbers, leaving out cases weighted below 0.5, as their section says.
//! - Independent samples: Levene's test on absolute deviations from the group means, then a
//!   row with the pooled variance and a row with separate variances (Satterthwaite's df).
//!   GROUPS without values compares 1 with 2; one value splits at it (at or above against
//!   below); two values compare those values and leave out other cases.
//! - Paired samples: without WITH every variable with every later one; with WITH every left
//!   with every right one; (PAIRED) the n-th with the n-th, a warning naming what is left over.
//! - MISSING=ANALYSIS leaves out cases missing in each analysis, LISTWISE cases missing on any
//!   variable of the test type (VARIABLES and GROUPS, or all PAIRS variables).
//! - Effect sizes: Cohen's d and Hedges' correction, with Glass's delta for independent
//!   samples; intervals from the noncentral t, except the corrected d of paired samples.

use crate::descriptives::{case_weights, number};
use crate::dist::{hedges_j, noncentral_t_parameter, t_p, t_quantile};
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Value as Stored, Values, Variable};
use statscraft_syntax::{Standardizer, TTest, TTestGroups, Value};

/// Weighted count, mean and variance of (value, weight) pairs.
#[derive(Clone, Copy)]
struct Moments {
    w: f64,
    mean: f64,
    variance: f64,
}

fn moments(pairs: &[(f64, f64)]) -> Moments {
    let w: f64 = pairs.iter().map(|(_, w)| w).sum();
    let mean = pairs.iter().map(|(x, w)| x * w).sum::<f64>() / w;
    let squares: f64 = pairs.iter().map(|(x, w)| w * (x - mean) * (x - mean)).sum();
    Moments {
        w,
        mean,
        variance: squares / (w - 1.0),
    }
}

impl Moments {
    fn sd(&self) -> f64 {
        self.variance.sqrt()
    }
}

/// An effect size row: name, standardizer, point estimate, lower and upper limit.
type Effect = (&'static str, f64, f64, f64, f64);

/// The limits of d from the noncentral t: the noncentrality bounds for |t| with `df` degrees
/// of freedom, divided by `root`, mirrored for a negative t.
fn noncentral_limits(t: f64, df: f64, alpha: f64, root: f64) -> (f64, f64) {
    if !t.is_finite() || df <= 0.0 {
        return (f64::NAN, f64::NAN);
    }
    let low = noncentral_t_parameter(t.abs(), df, 1.0 - alpha / 2.0) / root;
    let high = noncentral_t_parameter(t.abs(), df, alpha / 2.0) / root;
    if t >= 0.0 { (low, high) } else { (-high, -low) }
}

/// Cohen's d and Hedges' correction with standardizer `s`, limits `limits` and factor `j`.
fn cohen_hedges(d_name: &'static str, s: f64, d: f64, limits: (f64, f64), j: f64) -> Vec<Effect> {
    vec![
        (d_name, s, d, limits.0, limits.1),
        (
            "Hedges' correction",
            s / j,
            d * j,
            limits.0 * j,
            limits.1 * j,
        ),
    ]
}

fn effect_rows(label: &[Cell], effects: Vec<Effect>) -> Vec<Vec<Cell>> {
    effects
        .into_iter()
        .map(|(name, s, d, low, high)| {
            let mut row = label.to_vec();
            row.push(Cell::Text(name.into()));
            row.extend([s, d, low, high].map(|v| number(Some(v))));
            row
        })
        .collect()
}

fn table(title: &str, columns: &[&str], groups: &[&[&str]], rows: Vec<Vec<Cell>>) -> Table {
    let headers = columns.iter().take_while(|c| c.is_empty()).count();
    Table {
        title: title.into(),
        row_headers: headers,
        columns: columns.iter().map(|c| c.to_string()).collect(),
        column_groups: (0..columns.len())
            .map(|i| {
                groups
                    .get(i)
                    .map_or(Vec::new(), |g| g.iter().map(|s| s.to_string()).collect())
            })
            .collect(),
        rows,
        ..Table::default()
    }
}

/// Column groups for `n` leading columns without a group, then the given ones.
fn interval_groups(lead: usize, before: usize, level: &str, after: usize) -> Vec<Vec<String>> {
    let mut groups = vec![Vec::new(); lead + before];
    groups.extend([vec![level.to_string()], vec![level.to_string()]]);
    groups.extend(vec![Vec::new(); after]);
    groups
}

fn ci_title(ci: f64) -> String {
    let percent = 100.0 * ci;
    let shown = if (percent - percent.round()).abs() < 1e-9 {
        format!("{percent:.0}")
    } else {
        format!("{}", (percent * 1e6).round() / 1e6)
    };
    format!("{shown}% Confidence Interval")
}

/// The value of a numeric variable at a case, NaN when system-missing.
fn numeric(variable: &Variable, case: usize) -> f64 {
    match variable.values() {
        Values::Numeric(values) => values.get(case).unwrap_or(f64::NAN),
        Values::String(_) => f64::NAN,
    }
}

/// Whether a value of the grouping variable equals a group value.
fn same(variable: &Variable, case: usize, value: &Value) -> bool {
    match (variable.values(), value) {
        (Values::Numeric(values), Value::Number(n)) => values.get(case) == Some(*n),
        (Values::String(values), Value::String(s)) => values.get(case) == s.trim_end(),
        _ => false,
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim_end().to_string(),
    }
}

/// The label of a group value: its value label, else the value.
fn group_label(variable: &Variable, value: &Value) -> String {
    let found = variable
        .value_labels()
        .iter()
        .find(|(v, _)| match (v, value) {
            (Stored::Number(a), Value::Number(b)) => a == b,
            (Stored::String(a), Value::String(b)) => a.trim_end() == b.trim_end(),
            _ => false,
        });
    found.map_or_else(|| shown(value), |(_, label)| label.clone())
}

struct Run<'d> {
    data: &'d Dataset,
    weights: Option<Vec<f64>>,
    include: bool,
    alpha: f64,
    ci: String,
    warnings: Vec<String>,
}

impl Run<'_> {
    fn weight(&self, case: usize) -> f64 {
        self.weights.as_ref().map_or(1.0, |w| w[case])
    }

    /// The weight of a case for the effect sizes: rounded, nothing below 0.5.
    fn rounded(&self, case: usize) -> f64 {
        let w = self.weight(case);
        if w < 0.5 { 0.0 } else { w.round() }
    }

    fn cases(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.data.case_count()).filter(|&c| self.weight(c) > 0.0)
    }

    fn valid(&self, variable: &Variable, case: usize) -> bool {
        is_valid(variable, case, self.include)
    }

    fn pairs(&self, variable: &Variable, cases: &[usize], rounded: bool) -> Vec<(f64, f64)> {
        cases
            .iter()
            .map(|&c| {
                let w = if rounded {
                    self.rounded(c)
                } else {
                    self.weight(c)
                };
                (numeric(variable, c), w)
            })
            .filter(|(_, w)| *w > 0.0)
            .collect()
    }
}

impl Engine {
    pub(crate) fn t_test(&mut self, command: &TTest) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut run = Run {
            data,
            weights: case_weights(data)?,
            include: command.include,
            alpha: 1.0 - command.ci,
            ci: ci_title(command.ci),
            warnings: Vec::new(),
        };
        let mut out = Vec::new();
        let variables: Vec<Variable> = self
            .expand(&command.variables)?
            .iter()
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        for variable in &variables {
            if variable.is_string() {
                return Err(Error(format!(
                    "T-TEST needs numeric variables, but {} is a string variable",
                    variable.name()
                )));
            }
        }
        if let Some(value) = command.testval {
            out.extend(one_sample(&mut run, command, &variables, value));
        }
        if let Some((name, groups)) = &command.groups {
            let grouping = data.variable(name)?;
            out.extend(independent(
                &mut run, command, &variables, &grouping, groups,
            )?);
        }
        if !command.pairs.is_empty() {
            let mut pairs = Vec::new();
            for list in &command.pairs {
                let left = self.expand(&list.left)?;
                match &list.right {
                    None => {
                        for (i, a) in left.iter().enumerate() {
                            for b in &left[i + 1..] {
                                pairs.push((a.clone(), b.clone()));
                            }
                        }
                    }
                    Some(right) => {
                        let right = self.expand(right)?;
                        if list.paired {
                            if left.len() != right.len() {
                                run.warnings.push(format!(
                                    "(PAIRED) has {} variables before WITH and {} after; the unmatched ones are left out",
                                    left.len(),
                                    right.len()
                                ));
                            }
                            pairs.extend(left.iter().cloned().zip(right.iter().cloned()));
                        } else {
                            for a in &left {
                                for b in &right {
                                    pairs.push((a.clone(), b.clone()));
                                }
                            }
                        }
                    }
                }
            }
            let pairs: Vec<(Variable, Variable)> = pairs
                .iter()
                .map(|(a, b)| Ok::<_, Error>((data.variable(a)?, data.variable(b)?)))
                .collect::<Result<_, _>>()?;
            for (a, b) in &pairs {
                for v in [a, b] {
                    if v.is_string() {
                        return Err(Error(format!(
                            "T-TEST /PAIRS needs numeric variables, but {} is a string variable",
                            v.name()
                        )));
                    }
                }
            }
            out.extend(paired(&mut run, command, &pairs));
        }
        let warnings = std::mem::take(&mut run.warnings);
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}

fn one_sample(run: &mut Run, command: &TTest, variables: &[Variable], value: f64) -> Vec<Table> {
    let complete: Vec<usize> = run
        .cases()
        .filter(|&c| variables.iter().all(|v| run.valid(v, c)))
        .collect();
    let (mut statistics, mut tests, mut effects) = (Vec::new(), Vec::new(), Vec::new());
    for variable in variables {
        let cases: Vec<usize> = if command.listwise {
            complete.clone()
        } else {
            run.cases().filter(|&c| run.valid(variable, c)).collect()
        };
        let m = moments(&run.pairs(variable, &cases, false));
        let name = Cell::Text(variable.name().to_string());
        let se = m.sd() / m.w.sqrt();
        statistics.push(vec![
            name.clone(),
            Cell::Number(m.w),
            number(Some(m.mean)),
            number(Some(m.sd())),
            number(Some(se)),
        ]);
        if m.variance == 0.0 {
            run.warnings.push(format!(
                "no t for {}: its standard deviation is 0",
                variable.name()
            ));
        }
        let (d, df) = (m.mean - value, m.w - 1.0);
        let t = d / se;
        let half = t_quantile(run.alpha / 2.0, df) * se;
        tests.push(vec![
            name.clone(),
            number(Some(t)),
            number(Some(df)),
            number(Some(t_p(t, df))),
            number(Some(d)),
            number(Some(d - half)),
            number(Some(d + half)),
        ]);
        if command.effect_sizes {
            let r = moments(&run.pairs(variable, &cases, true));
            let (d, s) = (r.mean - value, r.sd());
            let t = d / (s / r.w.sqrt());
            let limits = noncentral_limits(t, r.w - 1.0, run.alpha, r.w.sqrt());
            let rows = cohen_hedges("Cohen's d", s, d / s, limits, hedges_j(r.w - 1.0));
            effects.extend(effect_rows(&[name], rows));
        }
    }
    let mut out = vec![table(
        "One-Sample Statistics",
        &["", "N", "Mean", "Std. Deviation", "Std. Error Mean"],
        &[],
        statistics,
    )];
    let mut test = table(
        &format!("One-Sample Test (Test Value = {value})"),
        &[
            "",
            "t",
            "df",
            "Sig. (2-tailed)",
            "Mean Difference",
            "Lower",
            "Upper",
        ],
        &[],
        tests,
    );
    test.column_groups = interval_groups(1, 4, &run.ci, 0);
    out.push(test);
    if command.effect_sizes {
        out.push(effects_table(
            "One-Sample Effect Sizes",
            1,
            &run.ci,
            effects,
        ));
    }
    out
}

fn effects_table(title: &str, labels: usize, ci: &str, rows: Vec<Vec<Cell>>) -> Table {
    let mut columns = vec![""; labels + 1];
    columns.extend(["Standardizer", "Point Estimate", "Lower", "Upper"]);
    let mut t = table(title, &columns, &[], rows);
    t.column_groups = interval_groups(labels + 1, 2, ci, 0);
    t
}

fn independent(
    run: &mut Run,
    command: &TTest,
    variables: &[Variable],
    grouping: &Variable,
    groups: &TTestGroups,
) -> Result<Vec<Table>, Error> {
    let (first, second, labels) = match groups {
        TTestGroups::Default if grouping.is_string() => {
            return Err(Error(format!(
                "GROUPS={} needs its group values in parentheses: a string variable has no default groups",
                grouping.name()
            )));
        }
        TTestGroups::Default => {
            let (a, b) = (Value::Number(1.0), Value::Number(2.0));
            let labels = [group_label(grouping, &a), group_label(grouping, &b)];
            (Some(a), Some(b), labels)
        }
        TTestGroups::Two(a, b) => {
            let labels = [group_label(grouping, a), group_label(grouping, b)];
            (Some(a.clone()), Some(b.clone()), labels)
        }
        TTestGroups::Cut(cut) => (
            Some(cut.clone()),
            None,
            [format!(">= {}", shown(cut)), format!("< {}", shown(cut))],
        ),
    };
    let numeric_type = matches!(grouping.values(), Values::Numeric(_));
    for value in [&first, &second].into_iter().flatten() {
        if matches!(value, Value::Number(_)) != numeric_type {
            return Err(Error(format!(
                "the GROUPS values of {} must be {}",
                grouping.name(),
                if numeric_type {
                    "numbers"
                } else {
                    "quoted strings"
                }
            )));
        }
    }
    // the group of a case: 0, 1 or none
    let group_of = |case: usize| -> Option<usize> {
        if !run.valid(grouping, case) {
            return None;
        }
        match (&first, &second) {
            (Some(a), Some(b)) => {
                if same(grouping, case, a) {
                    Some(0)
                } else if same(grouping, case, b) {
                    Some(1)
                } else {
                    None
                }
            }
            (Some(cut), None) => {
                let above = match (grouping.values(), cut) {
                    (Values::Numeric(v), Value::Number(n)) => v.get(case).map(|x| x >= *n),
                    (Values::String(v), Value::String(s)) => Some(v.get(case) >= s.trim_end()),
                    _ => None,
                }?;
                Some(if above { 0 } else { 1 })
            }
            _ => None,
        }
    };
    let membership: Vec<Option<usize>> = (0..run.data.case_count()).map(group_of).collect();
    let complete: Vec<bool> = (0..run.data.case_count())
        .map(|c| variables.iter().all(|v| run.valid(v, c)))
        .collect();
    let (mut statistics, mut tests, mut effects) = (Vec::new(), Vec::new(), Vec::new());
    for variable in variables {
        let in_group = |g: usize| -> Vec<usize> {
            run.cases()
                .filter(|&c| membership[c] == Some(g))
                .filter(|&c| {
                    if command.listwise {
                        complete[c]
                    } else {
                        run.valid(variable, c)
                    }
                })
                .collect()
        };
        let cases = [in_group(0), in_group(1)];
        let m = [0, 1].map(|g| moments(&run.pairs(variable, &cases[g], false)));
        let name = Cell::Text(variable.name().to_string());
        for g in 0..2 {
            statistics.push(vec![
                if g == 0 { name.clone() } else { Cell::Empty },
                Cell::Text(labels[g].clone()),
                Cell::Number(m[g].w),
                number(Some(m[g].mean)),
                number(Some(m[g].sd())),
                number(Some(m[g].sd() / m[g].w.sqrt())),
            ]);
        }
        let d = m[0].mean - m[1].mean;
        // Levene's test on the absolute deviations from the group means
        let deviations: Vec<Vec<(f64, f64)>> = (0..2)
            .map(|g| {
                run.pairs(variable, &cases[g], false)
                    .into_iter()
                    .map(|(x, w)| ((x - m[g].mean).abs(), w))
                    .collect()
            })
            .collect();
        let z = [moments(&deviations[0]), moments(&deviations[1])];
        let w = m[0].w + m[1].w;
        let z_all = (z[0].w * z[0].mean + z[1].w * z[1].mean) / w;
        let spread: f64 = (0..2).map(|g| z[g].w * (z[g].mean - z_all).powi(2)).sum();
        let inside: f64 = (0..2)
            .map(|g| {
                deviations[g]
                    .iter()
                    .map(|(x, w)| w * (x - z[g].mean).powi(2))
                    .sum::<f64>()
            })
            .sum();
        let levene = (w - 2.0) * spread / inside;
        let levene_p = crate::dist::f_p(levene, 1.0, w - 2.0);
        let pooled = ((m[0].w - 1.0) * m[0].variance + (m[1].w - 1.0) * m[1].variance) / (w - 2.0);
        let pooled_se = (pooled * (1.0 / m[0].w + 1.0 / m[1].w)).sqrt();
        let separate = [m[0].variance / m[0].w, m[1].variance / m[1].w];
        let separate_se = (separate[0] + separate[1]).sqrt();
        let share = |g: usize| (separate[g] / (separate[0] + separate[1])).powi(2) / (m[g].w - 1.0);
        let welch_df = 1.0 / (share(0) + share(1));
        for (row, (se, df)) in [(pooled_se, w - 2.0), (separate_se, welch_df)]
            .into_iter()
            .enumerate()
        {
            let t = d / se;
            let half = t_quantile(run.alpha / 2.0, df) * se;
            let mut cells = vec![
                if row == 0 { name.clone() } else { Cell::Empty },
                Cell::Text(
                    if row == 0 {
                        "Equal variances assumed"
                    } else {
                        "Equal variances not assumed"
                    }
                    .into(),
                ),
            ];
            cells.extend([
                number((row == 0).then_some(levene)),
                number((row == 0).then_some(levene_p)),
            ]);
            cells.extend([t, df, t_p(t, df), d, se, d - half, d + half].map(|v| number(Some(v))));
            tests.push(cells);
        }
        if m[0].w == 0.0 || m[1].w == 0.0 {
            run.warnings.push(format!(
                "no t for {}: one of the groups has no cases",
                variable.name()
            ));
        }
        if command.effect_sizes {
            let r = [0, 1].map(|g| moments(&run.pairs(variable, &cases[g], true)));
            let d = r[0].mean - r[1].mean;
            let df = r[0].w + r[1].w - 2.0;
            let sp =
                (((r[0].w - 1.0) * r[0].variance + (r[1].w - 1.0) * r[1].variance) / df).sqrt();
            let tilde = 1.0 / (1.0 / r[0].w + 1.0 / r[1].w);
            let t = d / (sp / tilde.sqrt());
            let limits = noncentral_limits(t, df, run.alpha, tilde.sqrt());
            let mut rows = cohen_hedges("Cohen's d", sp, d / sp, limits, hedges_j(df));
            let s2 = r[1].sd();
            let ts = d / (s2 / tilde.sqrt());
            let (low, high) = noncentral_limits(ts, r[1].w - 1.0, run.alpha, tilde.sqrt());
            rows.push(("Glass's delta", s2, d / s2, low, high));
            effects.extend(effect_rows(&[name], rows));
        }
    }
    let mut out = vec![table(
        "Group Statistics",
        &["", "", "N", "Mean", "Std. Deviation", "Std. Error Mean"],
        &[],
        statistics,
    )];
    let mut test = table(
        "Independent Samples Test",
        &[
            "",
            "",
            "F",
            "Sig.",
            "t",
            "df",
            "Sig. (2-tailed)",
            "Mean Difference",
            "Std. Error Difference",
            "Lower",
            "Upper",
        ],
        &[],
        tests,
    );
    let mut groups = vec![Vec::new(); 2];
    groups.extend(vec![
        vec![
            "Levene's Test for Equality of Variances".to_string()
        ];
        2
    ]);
    groups.extend(vec![vec!["t-test for Equality of Means".to_string()]; 5]);
    groups.extend(vec![
        vec![
            "t-test for Equality of Means".to_string(),
            run.ci.clone()
        ];
        2
    ]);
    test.column_groups = groups;
    out.push(test);
    if command.effect_sizes {
        out.push(effects_table(
            "Independent Samples Effect Sizes",
            1,
            &run.ci,
            effects,
        ));
    }
    Ok(out)
}

fn paired(run: &mut Run, command: &TTest, pairs: &[(Variable, Variable)]) -> Vec<Table> {
    let complete: Vec<bool> = (0..run.data.case_count())
        .map(|c| {
            pairs
                .iter()
                .all(|(a, b)| run.valid(a, c) && run.valid(b, c))
        })
        .collect();
    let (mut statistics, mut correlations, mut tests, mut effects) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut warnings = Vec::new();
    for (index, (a, b)) in pairs.iter().enumerate() {
        let pair = Cell::Text(format!("Pair {}", index + 1));
        let cases: Vec<usize> = run
            .cases()
            .filter(|&c| {
                if command.listwise {
                    complete[c]
                } else {
                    run.valid(a, c) && run.valid(b, c)
                }
            })
            .collect();
        let summary = |rounded: bool| {
            let x = run.pairs(a, &cases, rounded);
            let y = run.pairs(b, &cases, rounded);
            let (mx, my) = (moments(&x), moments(&y));
            let covariance = x
                .iter()
                .zip(&y)
                .map(|((x, w), (y, _))| w * (x - mx.mean) * (y - my.mean))
                .sum::<f64>()
                / (mx.w - 1.0);
            (mx, my, covariance)
        };
        let (mx, my, sxy) = summary(false);
        for (i, (v, m)) in [(a, mx), (b, my)].into_iter().enumerate() {
            statistics.push(vec![
                if i == 0 { pair.clone() } else { Cell::Empty },
                Cell::Text(v.name().to_string()),
                number(Some(m.mean)),
                Cell::Number(m.w),
                number(Some(m.sd())),
                number(Some(m.sd() / m.w.sqrt())),
            ]);
        }
        let w = mx.w;
        let r = sxy / (mx.sd() * my.sd());
        let tr = r * ((w - 2.0) / (1.0 - r * r)).sqrt();
        correlations.push(vec![
            pair.clone(),
            Cell::Text(format!("{} & {}", a.name(), b.name())),
            Cell::Number(w),
            number(Some(r)),
            number(Some(t_p(tr, w - 2.0))),
        ]);
        let d = mx.mean - my.mean;
        let sd = (mx.variance + my.variance - 2.0 * sxy).max(0.0).sqrt();
        let se = sd / w.sqrt();
        let t = d / se;
        let df = w - 1.0;
        let half = t_quantile(run.alpha / 2.0, df) * se;
        let label = Cell::Text(format!("{} - {}", a.name(), b.name()));
        let mut row = vec![pair.clone(), label.clone()];
        row.extend([d, sd, se, d - half, d + half, t, df, t_p(t, df)].map(|v| number(Some(v))));
        tests.push(row);
        if sd == 0.0 {
            warnings.push(format!(
                "no t for {} - {}: the standard deviation of the differences is 0",
                a.name(),
                b.name()
            ));
        }
        if command.effect_sizes {
            let (rx, ry, rxy) = summary(true);
            let w = rx.w;
            let d = rx.mean - ry.mean;
            let sd = (rx.variance + ry.variance - 2.0 * rxy).max(0.0).sqrt();
            let r = rxy / (rx.sd() * ry.sd());
            let j = hedges_j(2.0 * w - 2.0);
            let rows = match command.standardizer {
                Standardizer::Sd => {
                    let t = d / (sd / w.sqrt());
                    let limits = noncentral_limits(t, w - 1.0, run.alpha, w.sqrt());
                    cohen_hedges("Cohen's d", sd, d / sd, limits, j)
                }
                Standardizer::CorrectedSd => {
                    let s = sd / (2.0 * (1.0 - r)).sqrt();
                    let estimate = d / s;
                    let se = (2.0 * (1.0 - r) / w + estimate * estimate / (2.0 * (w - 1.0))).sqrt();
                    let half = t_quantile(run.alpha / 2.0, w - 1.0) * se;
                    cohen_hedges(
                        "Cohen's d",
                        s,
                        estimate,
                        (estimate - half, estimate + half),
                        j,
                    )
                }
                Standardizer::Average => {
                    let s = ((rx.variance + ry.variance) / 2.0).sqrt();
                    let t = d / (s / w.sqrt());
                    let limits = noncentral_limits(t, w - 1.0, run.alpha, w.sqrt());
                    cohen_hedges("Cohen's d", s, d / s, limits, j)
                }
            };
            effects.extend(effect_rows(&[pair, label], rows));
        }
    }
    run.warnings.extend(warnings);
    let mut out = vec![
        table(
            "Paired Samples Statistics",
            &["", "", "Mean", "N", "Std. Deviation", "Std. Error Mean"],
            &[],
            statistics,
        ),
        table(
            "Paired Samples Correlations",
            &["", "", "N", "Correlation", "Sig."],
            &[],
            correlations,
        ),
    ];
    let mut test = table(
        "Paired Samples Test",
        &[
            "",
            "",
            "Mean",
            "Std. Deviation",
            "Std. Error Mean",
            "Lower",
            "Upper",
            "t",
            "df",
            "Sig. (2-tailed)",
        ],
        &[],
        tests,
    );
    let mut groups = vec![Vec::new(); 2];
    groups.extend(vec![vec!["Paired Differences".to_string()]; 3]);
    groups.extend(vec![
        vec!["Paired Differences".to_string(), run.ci.clone()];
        2
    ]);
    groups.extend(vec![Vec::new(); 3]);
    test.column_groups = groups;
    out.push(test);
    if command.effect_sizes {
        out.push(effects_table(
            "Paired Samples Effect Sizes",
            2,
            &run.ci,
            effects,
        ));
    }
    out
}
