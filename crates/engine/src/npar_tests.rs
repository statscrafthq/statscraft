//! NPAR TESTS: one-sample tests, tests of two and k related samples, and (in npar_groups.rs)
//! tests of two and k independent samples.
//!
//! Rules from the Command Syntax Reference (NPAR TESTS) and the Algorithms manual (NPAR TESTS
//! Algorithms; the Nonparametric Tests chapter for the Jonckheere-Terpstra and marginal
//! homogeneity tests and the Lilliefors significance):
//! - A weight repeats a case as many times as it rounds to; a case that rounds to 0 drops out.
//! - MISSING=ANALYSIS leaves a case out of a test when a variable of that test is missing;
//!   LISTWISE when any variable of the command is. INCLUDE makes user-missing values valid.
//! - A string variable anywhere stops the command. A variable named twice in one test list
//!   counts once; a pair of a variable with itself (from ALL) is skipped with a warning.
//! - Chi-square: categories are the distinct values, or the whole numbers lo to hi of the range
//!   (values truncated, others left out); expected frequencies are proportions of N; the number
//!   of expected values must match the categories. df = k - 1.
//! - Binomial: exact. With p = 0.5 two-tailed, min(1, 2 P(X <= min(n1, n2))); otherwise one-
//!   tailed in the observed direction (P(X >= n1) when n1/N > p, else P(X <= n1)), as the
//!   Command Syntax Reference describes; the Algorithms formula sums over the smaller category,
//!   which is not a tail probability when the smaller category is the first and above p.
//! - Runs: cut point by mean, median or mode (the largest of several modes, with a warning);
//!   values below the cut are one sign, the others the other; runs counted in file order. Z with
//!   a correction of 0.5 toward the mean when N < 50.
//! - One-sample K-S on the distinct values: D_i = F(x_i-1) - F0(x_i) and F(x_i) - F0(x_i), the
//!   Poisson at x_i - 1 and x_i; Z = sqrt(N) max |D|, significance by Smirnov's three terms. A
//!   normal distribution with parameters taken from the data gets the Lilliefors significance.
//! - Wilcoxon: D = X - Y, average ranks of the nonzero |D|; Z from min(Sp, Sn) with the tie
//!   correction. Sign and McNemar: exact binomial (doubled) up to 25 changes, else Z with a
//!   continuity correction (sign) or chi-square with one (McNemar).
//! - Marginal homogeneity: the values are the scores; two values make it the McNemar test.
//! - Friedman with the tie correction; Kendall's W = chi-square / (N (k - 1)); Cochran's Q on two
//!   values (the choice of success does not change Q).

use crate::crosstabs::{Key, label};
use crate::descriptives::{case_weights, number};
use crate::dist::{beta_i, chi_square_p, gamma_q, ln_gamma, normal_cdf, normal_p};
use crate::examine::{Distribution, lilliefors};
use crate::format::Currencies;
use crate::means::is_valid;
use crate::nonpar_corr::ranks;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    BinomialSplit, KsDistribution, NparTest, NparTests, PairedTest, RelatedTest, RunsCut,
    TTestPairs, VarItem,
};

/// The data of one NPAR TESTS run and what it shares between the tests.
pub(crate) struct Run<'d> {
    pub data: &'d Dataset,
    /// Every case's weight rounded to a whole number; 1 without a weight variable.
    pub weights: Vec<f64>,
    pub include: bool,
    /// With LISTWISE, the cases valid on every variable of the command.
    pub complete: Option<Vec<bool>>,
    pub currencies: Currencies,
    pub exact: bool,
    pub warnings: Vec<String>,
}

impl Run<'_> {
    /// The cases with a weight that are valid on every one of `variables`, in file order.
    pub fn cases(&self, variables: &[&Variable]) -> Vec<usize> {
        (0..self.data.case_count())
            .filter(|&c| self.weights[c] > 0.0)
            .filter(|&c| self.complete.as_ref().is_none_or(|m| m[c]))
            .filter(|&c| variables.iter().all(|v| is_valid(v, c, self.include)))
            .collect()
    }

    pub fn value(variable: &Variable, case: usize) -> f64 {
        match variable.values() {
            Values::Numeric(values) => values.get(case).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        }
    }

    /// (value, weight) of `variable` at `cases`.
    pub fn pairs(&self, variable: &Variable, cases: &[usize]) -> Vec<(f64, f64)> {
        cases
            .iter()
            .map(|&c| (Self::value(variable, c), self.weights[c]))
            .collect()
    }

    pub fn label(&self, variable: &Variable, value: f64) -> String {
        label(variable, &Key::Number(value), &self.currencies)
    }

    /// Warns once that METHOD gives no exact significance for `test`.
    pub fn no_exact(&mut self, test: &str) {
        let message = format!(
            "/METHOD: the exact significance of the {test} is not computed; the asymptotic one is shown"
        );
        if self.exact && !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
}

/// ln of n choose k.
pub(crate) fn ln_choose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

/// P(X <= k) for X binomial with n trials and probability p.
pub(crate) fn binomial_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 {
        0.0
    } else if k >= n {
        1.0
    } else {
        beta_i(n - k, k + 1.0, 1.0 - p)
    }
}

/// P(X >= k) for X binomial with n trials and probability p.
fn binomial_upper(k: f64, n: f64, p: f64) -> f64 {
    if k <= 0.0 {
        1.0
    } else if k > n {
        0.0
    } else {
        beta_i(k, n - k + 1.0, p)
    }
}

fn binomial_pmf(k: f64, n: f64, p: f64) -> f64 {
    (ln_choose(n, k) + k * p.ln() + (n - k) * (1.0 - p).ln()).exp()
}

/// The exact sign test of r = min(n1, n2) among n = n1 + n2 changes: two-tailed, one-tailed and
/// point probability.
pub(crate) fn sign_exact(n1: f64, n2: f64) -> [f64; 3] {
    let n = n1 + n2;
    let r = n1.min(n2);
    let one = binomial_cdf(r, n, 0.5);
    [(2.0 * one).min(1.0), one, binomial_pmf(r, n, 0.5)]
}

/// The two-tailed significance of a Kolmogorov-Smirnov Z by the first terms of Smirnov's formula.
pub(crate) fn smirnov_p(z: f64) -> f64 {
    if z < 0.27 {
        1.0
    } else if z < 1.0 {
        let q = (-1.233701 / (z * z)).exp();
        1.0 - 2.506628 / z * (q + q.powi(9) + q.powi(25))
    } else if z < 3.1 {
        let q = (-2.0 * z * z).exp();
        2.0 * (q - q.powi(4) + q.powi(9) - q.powi(16))
    } else {
        0.0
    }
}

/// Average ranks of weighted values and the sum of t^3 - t over the groups of t tied cases.
pub(crate) fn tied_ranks(pairs: &[(f64, f64)]) -> (Vec<f64>, f64) {
    let values: Vec<f64> = pairs.iter().map(|p| p.0).collect();
    let weights: Vec<f64> = pairs.iter().map(|p| p.1).collect();
    let (ranks, ties) = ranks(&values, &weights);
    // t^3 - t = (t^2 - t)(t - 2) + 3 (t^2 - t)
    (ranks, ties[1] + 3.0 * ties[0])
}

/// Z of a runs count with mean and standard deviation of the runs test; below 50 cases
/// corrected by 0.5 toward the mean.
pub(crate) fn runs_z(runs: f64, n1: f64, n2: f64) -> Option<f64> {
    let n = n1 + n2;
    let mean = 2.0 * n1 * n2 / n + 1.0;
    let variance = 2.0 * n1 * n2 * (2.0 * n1 * n2 - n) / (n * n * (n - 1.0));
    if variance.is_nan() || variance <= 0.0 {
        return None;
    }
    let sd = variance.sqrt();
    let d = runs - mean;
    Some(if n >= 50.0 {
        d / sd
    } else if d <= -0.5 {
        (d + 0.5) / sd
    } else if d >= 0.5 {
        (d - 0.5) / sd
    } else {
        0.0
    })
}

/// The probability of exactly r runs of n1 and n2 cases in random order.
pub(crate) fn runs_pmf(r: usize, n1: f64, n2: f64) -> f64 {
    let total = ln_choose(n1 + n2, n1);
    let term = |a: f64, b: f64| {
        if a < 0.0 || b < 0.0 || a > n1 - 1.0 || b > n2 - 1.0 {
            0.0
        } else {
            (ln_choose(n1 - 1.0, a) + ln_choose(n2 - 1.0, b) - total).exp()
        }
    };
    if r < 2 {
        return 0.0;
    }
    let k = (r / 2) as f64;
    if r.is_multiple_of(2) {
        2.0 * term(k - 1.0, k - 1.0)
    } else {
        // r = 2k + 1: one group has k + 1 runs, the other k
        term(k, k - 1.0) + term(k - 1.0, k)
    }
}

/// Exact P(R <= runs), the two-sided significance (runs at least as far from the mean) and the
/// point probability.
pub(crate) fn runs_exact(runs: f64, n1: f64, n2: f64) -> [f64; 3] {
    let n = n1 + n2;
    let mean = 2.0 * n1 * n2 / n + 1.0;
    let most = (2.0 * n1.min(n2) + 1.0).min(n) as usize;
    let (mut lower, mut two) = (0.0, 0.0);
    for r in 2..=most {
        let p = runs_pmf(r, n1, n2);
        if r as f64 <= runs {
            lower += p;
        }
        if (r as f64 - mean).abs() >= (runs - mean).abs() - 1e-9 {
            two += p;
        }
    }
    [
        lower.min(1.0),
        two.min(1.0),
        runs_pmf(runs.round() as usize, n1, n2),
    ]
}

/// A table with `row_headers` label columns, one header level and the given rows.
pub(crate) fn table(
    title: impl Into<String>,
    row_headers: usize,
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
) -> Table {
    Table {
        title: title.into(),
        row_headers,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        ..Table::default()
    }
}

pub(crate) fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

/// A table of statistics in rows and one column per tested variable or pair.
pub(crate) fn statistics_table(
    title: &str,
    names: &[String],
    rows: Vec<(String, Vec<Option<f64>>)>,
) -> Table {
    let mut columns = vec![String::new()];
    columns.extend(names.iter().cloned());
    let rows = rows
        .into_iter()
        .map(|(name, values)| {
            let mut row = vec![text(name)];
            row.extend(values.into_iter().map(number));
            row
        })
        .collect();
    table(title, 1, columns, rows)
}

/// Keeps rows that have at least one value.
pub(crate) fn used(rows: Vec<(String, Vec<Option<f64>>)>) -> Vec<(String, Vec<Option<f64>>)> {
    rows.into_iter()
        .filter(|(_, values)| values.iter().any(Option::is_some))
        .collect()
}

/// A test as the engine runs it: the variables expanded to names.
enum Plan {
    Single(usize, Vec<String>),
    Pairs(PairedTest, Vec<(String, String)>),
    Related(RelatedTest, Vec<String>),
    Groups(usize, Vec<String>, String),
}

fn dedup(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        if !out.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            out.push(name);
        }
    }
    out
}

impl Engine {
    fn npar_pairs(
        &self,
        pairs: &TTestPairs,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<(String, String)>, Error> {
        let left = self.expand(&pairs.left)?;
        let mut out = Vec::new();
        match &pairs.right {
            None => {
                for (i, a) in left.iter().enumerate() {
                    for b in &left[i + 1..] {
                        out.push((a.clone(), b.clone()));
                    }
                }
            }
            Some(right) => {
                let right = self.expand(right)?;
                if pairs.paired {
                    if left.len() != right.len() {
                        return Err(Error(format!(
                            "(PAIRED) needs as many variables after WITH as before it, got {} and {}",
                            left.len(),
                            right.len()
                        )));
                    }
                    out.extend(left.iter().cloned().zip(right.iter().cloned()));
                } else {
                    for a in &left {
                        for b in &right {
                            out.push((a.clone(), b.clone()));
                        }
                    }
                }
            }
        }
        let before = out.len();
        out.retain(|(a, b)| !a.eq_ignore_ascii_case(b));
        if out.len() < before {
            warnings.push("a variable paired with itself is not tested".into());
        }
        let mut unique: Vec<(String, String)> = Vec::new();
        for pair in out {
            if !unique.contains(&pair) {
                unique.push(pair);
            }
        }
        Ok(unique)
    }

    fn npar_list(&self, items: &[VarItem]) -> Result<Vec<String>, Error> {
        Ok(dedup(self.expand(items)?))
    }

    pub(crate) fn npar_tests(&mut self, command: &NparTests) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut warnings = Vec::new();
        let mut plans = Vec::new();
        for (i, test) in command.tests.iter().enumerate() {
            plans.push(match test {
                NparTest::ChiSquare { variables, .. }
                | NparTest::Binomial { variables, .. }
                | NparTest::Runs { variables, .. }
                | NparTest::KolmogorovSmirnov { variables, .. } => {
                    Plan::Single(i, self.npar_list(variables)?)
                }
                NparTest::Paired { test, pairs } => {
                    Plan::Pairs(*test, self.npar_pairs(pairs, &mut warnings)?)
                }
                NparTest::Related { test, variables } => {
                    let names = self.npar_list(variables)?;
                    if names.len() < 2 {
                        return Err(Error(format!(
                            "{} needs at least two variables",
                            related_name(*test)
                        )));
                    }
                    Plan::Related(*test, names)
                }
                NparTest::Groups {
                    variables, group, ..
                } => Plan::Groups(
                    i,
                    self.npar_list(variables)?,
                    self.expand(&[VarItem::Name(group.clone())])?.remove(0),
                ),
            });
        }
        // every variable of the command, in order, for LISTWISE and the descriptives
        let mut all: Vec<String> = Vec::new();
        for plan in &plans {
            let names: Vec<&String> = match plan {
                Plan::Single(_, names) | Plan::Related(_, names) => names.iter().collect(),
                Plan::Pairs(_, pairs) => pairs.iter().flat_map(|(a, b)| [a, b]).collect(),
                Plan::Groups(_, names, group) => names.iter().chain([group]).collect(),
            };
            all.extend(names.into_iter().cloned());
        }
        let all = dedup(all);
        let variables: Vec<Variable> = all
            .iter()
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "NPAR TESTS needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let weights: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; data.case_count()],
        };
        let complete = command.listwise.then(|| {
            (0..data.case_count())
                .map(|c| variables.iter().all(|v| is_valid(v, c, command.include)))
                .collect()
        });
        let mut run = Run {
            data,
            weights,
            include: command.include,
            complete,
            currencies: self.currencies(),
            exact: command.exact,
            warnings,
        };
        let var = |name: &str| data.variable(name);
        let mut out = Vec::new();
        if command.descriptives || command.quartiles {
            let refs: Vec<&Variable> = variables.iter().collect();
            out.push(descriptives(&run, &refs, command));
        }
        for plan in &plans {
            match plan {
                Plan::Single(i, names) => {
                    let vars: Vec<Variable> =
                        names.iter().map(|n| var(n)).collect::<Result<_, _>>()?;
                    out.extend(match &command.tests[*i] {
                        NparTest::ChiSquare {
                            range, expected, ..
                        } => chi_square(&mut run, &vars, *range, expected)?,
                        NparTest::Binomial { p, split, .. } => {
                            binomial(&mut run, &vars, *p, *split)
                        }
                        NparTest::Runs { cut, .. } => runs(&mut run, &vars, *cut),
                        NparTest::KolmogorovSmirnov {
                            distribution,
                            parameters,
                            ..
                        } => one_sample_ks(&mut run, &vars, *distribution, parameters),
                        _ => unreachable!("single-variable tests"),
                    });
                }
                Plan::Pairs(test, pairs) => {
                    let vars: Vec<(Variable, Variable)> = pairs
                        .iter()
                        .map(|(a, b)| Ok::<_, Error>((var(a)?, var(b)?)))
                        .collect::<Result<_, _>>()?;
                    out.extend(match test {
                        PairedTest::Wilcoxon => wilcoxon(&mut run, &vars),
                        PairedTest::Sign => sign(&mut run, &vars),
                        PairedTest::McNemar => mcnemar(&mut run, &vars),
                        PairedTest::MarginalHomogeneity => marginal_homogeneity(&mut run, &vars),
                    });
                }
                Plan::Related(test, names) => {
                    let vars: Vec<Variable> =
                        names.iter().map(|n| var(n)).collect::<Result<_, _>>()?;
                    out.extend(related(&mut run, &vars, *test));
                }
                Plan::Groups(i, names, group) => {
                    let vars: Vec<Variable> =
                        names.iter().map(|n| var(n)).collect::<Result<_, _>>()?;
                    let NparTest::Groups { test, values, .. } = &command.tests[*i] else {
                        unreachable!("group tests")
                    };
                    out.extend(crate::npar_groups::groups(
                        &mut run,
                        &vars,
                        &var(group)?,
                        *test,
                        *values,
                    )?);
                }
            }
        }
        let warnings = std::mem::take(&mut run.warnings);
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}

fn related_name(test: RelatedTest) -> &'static str {
    match test {
        RelatedTest::Friedman => "FRIEDMAN",
        RelatedTest::Kendall => "KENDALL",
        RelatedTest::Cochran => "COCHRAN",
    }
}

/// N, mean, standard deviation, minimum and maximum, and the quartiles, of every variable.
fn descriptives(run: &Run, variables: &[&Variable], command: &NparTests) -> Table {
    let mut columns = vec![String::new()];
    let mut groups = vec![Vec::new()];
    columns.push("N".into());
    groups.push(Vec::new());
    if command.descriptives {
        for c in ["Mean", "Std. Deviation", "Minimum", "Maximum"] {
            columns.push(c.into());
            groups.push(Vec::new());
        }
    }
    if command.quartiles {
        for c in ["25th", "50th (Median)", "75th"] {
            columns.push(c.into());
            groups.push(vec!["Percentiles".to_string()]);
        }
    }
    let mut rows = Vec::new();
    for v in variables {
        let cases = run.cases(&[v]);
        let pairs = run.pairs(v, &cases);
        let w: f64 = pairs.iter().map(|p| p.1).sum();
        let mut row = vec![text(v.name()), Cell::Number(w)];
        if command.descriptives {
            let mean = pairs.iter().map(|(x, w)| x * w).sum::<f64>() / w;
            let squares: f64 = pairs.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
            let sd = (w > 1.0).then(|| (squares / (w - 1.0)).sqrt());
            let min = pairs.iter().map(|p| p.0).reduce(f64::min);
            let max = pairs.iter().map(|p| p.0).reduce(f64::max);
            row.extend([number(Some(mean)), number(sd), number(min), number(max)]);
        }
        if command.quartiles {
            let dist = Distribution::new(&pairs);
            for percent in [25.0, 50.0, 75.0] {
                row.push(number(dist.frequencies_percentile(percent)));
            }
        }
        rows.push(row);
    }
    Table {
        title: "Descriptive Statistics".into(),
        row_headers: 1,
        columns,
        column_groups: groups,
        rows,
        ..Table::default()
    }
}

/// Distinct values in ascending order with their summed weights.
pub(crate) fn tally(pairs: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut sorted = pairs.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (x, w) in sorted {
        match out.last_mut() {
            Some(last) if last.0 == x => last.1 += w,
            _ => out.push((x, w)),
        }
    }
    out
}

fn chi_square(
    run: &mut Run,
    variables: &[Variable],
    range: Option<(f64, f64)>,
    expected: &[f64],
) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    let names: Vec<String> = variables.iter().map(|v| v.name().to_string()).collect();
    let mut stats = vec![
        ("Chi-Square".to_string(), Vec::new()),
        ("df".to_string(), Vec::new()),
        ("Asymp. Sig.".to_string(), Vec::new()),
    ];
    let mut notes = Vec::new();
    // the categories of the range, when there is one
    let fixed: Option<Vec<f64>> = range.map(|(lo, hi)| {
        (lo.trunc() as i64..=hi.trunc() as i64)
            .map(|v| v as f64)
            .collect()
    });
    let mut combined: Vec<Vec<Cell>> = fixed
        .as_ref()
        .map(|cats| cats.iter().map(|c| vec![Cell::Number(*c)]).collect())
        .unwrap_or_default();
    let mut combined_total = vec![text("Total")];
    for v in variables {
        let cases = run.cases(&[v]);
        let pairs: Vec<(f64, f64)> = run
            .pairs(v, &cases)
            .into_iter()
            .map(|(x, w)| (if fixed.is_some() { x.trunc() } else { x }, w))
            .collect();
        let cells: Vec<(f64, f64)> = match &fixed {
            Some(cats) => cats
                .iter()
                .map(|&c| {
                    let w = pairs.iter().filter(|p| p.0 == c).map(|p| p.1).sum();
                    (c, w)
                })
                .collect(),
            None => tally(&pairs),
        };
        let n: f64 = cells.iter().map(|c| c.1).sum();
        let k = cells.len();
        if !expected.is_empty() && expected.len() != k {
            return Err(Error(format!(
                "CHISQUARE: EXPECTED gives {} frequencies, but {} has {k} categories",
                expected.len(),
                v.name()
            )));
        }
        if expected.iter().any(|e| *e <= 0.0) {
            return Err(Error(
                "CHISQUARE: every expected frequency must be above 0".into(),
            ));
        }
        let total_expected: f64 = expected.iter().sum();
        let expect = |i: usize| {
            if expected.is_empty() {
                n / k as f64
            } else {
                expected[i] / total_expected * n
            }
        };
        let mut chi = 0.0;
        let mut low = 0;
        let mut min_expected = f64::INFINITY;
        let mut rows = Vec::new();
        for (i, &(value, observed)) in cells.iter().enumerate() {
            let e = expect(i);
            chi += (observed - e).powi(2) / e;
            if e < 5.0 {
                low += 1;
            }
            min_expected = min_expected.min(e);
            let values = [
                Cell::Number(observed),
                Cell::Number(e),
                Cell::Number(observed - e),
            ];
            match &fixed {
                Some(_) => combined[i].extend(values),
                None => {
                    let mut row = vec![text(run.label(v, value))];
                    row.extend(values);
                    rows.push(row);
                }
            }
        }
        if fixed.is_some() {
            combined_total.extend([Cell::Number(n), Cell::Empty, Cell::Empty]);
        } else {
            rows.push(vec![
                text("Total"),
                Cell::Number(n),
                Cell::Empty,
                Cell::Empty,
            ]);
            let columns = ["", "Observed N", "Expected N", "Residual"].map(String::from);
            out.push(table(
                format!("Chi-Square Frequencies: {}", v.name()),
                1,
                columns.to_vec(),
                rows,
            ));
        }
        let usable = k >= 2 && n > 0.0;
        if !usable {
            run.warnings.push(format!(
                "CHISQUARE: {} has fewer than two categories with cases; no test",
                v.name()
            ));
        }
        stats[0].1.push(usable.then_some(chi));
        stats[1].1.push(usable.then_some(k as f64 - 1.0));
        stats[2]
            .1
            .push(usable.then(|| chi_square_p(chi, k as f64 - 1.0)));
        if usable {
            let share = 100.0 * low as f64 / k as f64;
            notes.push(format!(
                "{}: {low} cells ({share:.1}%) have expected frequencies less than 5. The minimum expected cell frequency is {}.",
                v.name(),
                crate::examine::short(min_expected)
            ));
        }
    }
    if fixed.is_some() {
        let mut columns = vec!["Category".to_string()];
        let mut groups = vec![Vec::new()];
        for name in &names {
            for c in ["Observed N", "Expected N", "Residual"] {
                columns.push(c.into());
                groups.push(vec![name.clone()]);
            }
        }
        combined.push(combined_total);
        out.push(Table {
            title: "Chi-Square Frequencies".into(),
            row_headers: 1,
            columns,
            column_groups: groups,
            rows: combined,
            ..Table::default()
        });
    }
    let mut stats_table = statistics_table("Chi-Square Test Statistics", &names, stats);
    stats_table.caption = (!notes.is_empty()).then(|| notes.join(" "));
    out.push(stats_table);
    Ok(out)
}

fn binomial(run: &mut Run, variables: &[Variable], p: f64, split: BinomialSplit) -> Vec<Table> {
    let tails = if p == 0.5 {
        "Exact Sig. (2-tailed)"
    } else {
        "Exact Sig. (1-tailed)"
    };
    let columns = [
        "",
        "",
        "Category",
        "N",
        "Observed Prop.",
        "Test Prop.",
        tails,
    ]
    .map(String::from)
    .to_vec();
    let mut rows = Vec::new();
    for v in variables {
        let cases = run.cases(&[v]);
        let pairs = run.pairs(v, &cases);
        let (first, second, labels) = match split {
            BinomialSplit::Cut(cut) => {
                let (a, b): (Vec<_>, Vec<_>) = pairs.iter().partition(|p| p.0 <= cut);
                let shown = crate::format::category_text(cut, &run.currencies.resolve(v.format()));
                (a, b, [format!("<= {shown}"), format!("> {shown}")])
            }
            BinomialSplit::Values(x, y) => {
                let a = pairs.iter().filter(|p| p.0 == x).copied().collect();
                let b = pairs.iter().filter(|p| p.0 == y).copied().collect();
                (a, b, [run.label(v, x), run.label(v, y)])
            }
            BinomialSplit::Dichotomy => {
                let distinct = tally(&pairs);
                if distinct.len() != 2 {
                    run.warnings.push(format!(
                        "BINOMIAL: {} has {} values; give a cut point or two values to test it",
                        v.name(),
                        distinct.len()
                    ));
                    continue;
                }
                // the value met first in the file is the first category
                let x = pairs[0].0;
                let y = distinct.iter().map(|d| d.0).find(|&d| d != x).unwrap_or(x);
                let a = pairs.iter().filter(|p| p.0 == x).copied().collect();
                let b = pairs.iter().filter(|p| p.0 == y).copied().collect();
                (a, b, [run.label(v, x), run.label(v, y)])
            }
        };
        let n1: f64 = first.iter().map(|p| p.1).sum();
        let n2: f64 = second.iter().map(|p| p.1).sum();
        let n = n1 + n2;
        if n == 0.0 {
            run.warnings
                .push(format!("BINOMIAL: {} has no cases to test", v.name()));
            continue;
        }
        let sig = if p == 0.5 {
            (2.0 * binomial_cdf(n1.min(n2), n, 0.5)).min(1.0)
        } else if n1 / n > p {
            binomial_upper(n1, n, p)
        } else {
            binomial_cdf(n1, n, p)
        };
        let [l1, l2] = labels;
        rows.push(vec![
            text(v.name()),
            text("Group 1"),
            text(l1),
            Cell::Number(n1),
            Cell::Number(n1 / n),
            Cell::Number(p),
            Cell::Number(sig),
        ]);
        rows.push(vec![
            Cell::Empty,
            text("Group 2"),
            text(l2),
            Cell::Number(n2),
            Cell::Number(n2 / n),
            Cell::Empty,
            Cell::Empty,
        ]);
        rows.push(vec![
            Cell::Empty,
            text("Total"),
            Cell::Empty,
            Cell::Number(n),
            Cell::Number(1.0),
            Cell::Empty,
            Cell::Empty,
        ]);
    }
    if rows.is_empty() {
        return Vec::new();
    }
    vec![table("Binomial Test", 2, columns, rows)]
}

/// The median of weighted values as the runs and median tests define it: the middle case, or
/// the mean of the two middle cases.
pub(crate) fn middle(pairs: &[(f64, f64)]) -> Option<f64> {
    let sorted = tally(pairs);
    let n: f64 = sorted.iter().map(|p| p.1).sum();
    if n <= 0.0 {
        return None;
    }
    // the value of the case at 1-based position k
    let at = |k: f64| {
        let mut seen = 0.0;
        for (x, w) in &sorted {
            seen += w;
            if seen >= k {
                return *x;
            }
        }
        sorted[sorted.len() - 1].0
    };
    Some(if n % 2.0 == 0.0 {
        (at(n / 2.0) + at(n / 2.0 + 1.0)) / 2.0
    } else {
        at((n + 1.0) / 2.0)
    })
}

fn runs(run: &mut Run, variables: &[Variable], cut: RunsCut) -> Vec<Table> {
    let names: Vec<String> = variables.iter().map(|v| v.name().to_string()).collect();
    let mut rows: Vec<(String, Vec<Option<f64>>)> = [
        "Test Value",
        "Cases < Test Value",
        "Cases >= Test Value",
        "Total Cases",
        "Number of Runs",
        "Z",
        "Asymp. Sig. (2-tailed)",
        "Exact Sig. (2-tailed)",
        "Point Probability",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    for v in variables {
        let cases = run.cases(&[v]);
        let pairs = run.pairs(v, &cases);
        let n: f64 = pairs.iter().map(|p| p.1).sum();
        let value = match cut {
            RunsCut::Value(x) => Some(x),
            RunsCut::Mean => (n > 0.0).then(|| pairs.iter().map(|(x, w)| x * w).sum::<f64>() / n),
            RunsCut::Median => middle(&pairs),
            RunsCut::Mode => {
                let counts = tally(&pairs);
                let top = counts.iter().map(|c| c.1).fold(0.0, f64::max);
                let modes: Vec<f64> = counts.iter().filter(|c| c.1 == top).map(|c| c.0).collect();
                if modes.len() > 1 {
                    run.warnings.push(format!(
                        "RUNS: {} has several modes; the largest, {}, is the cut point",
                        v.name(),
                        modes[modes.len() - 1]
                    ));
                }
                modes.last().copied()
            }
        };
        let Some(value) = value else {
            for row in rows.iter_mut() {
                row.1.push(None);
            }
            continue;
        };
        let (mut below, mut above, mut count, mut last) = (0.0, 0.0, 0.0, None);
        for (x, w) in &pairs {
            let side = *x >= value;
            if side {
                above += w;
            } else {
                below += w;
            }
            if last != Some(side) {
                count += 1.0;
                last = Some(side);
            }
        }
        let z = runs_z(count, below, above);
        let exact =
            (run.exact && below > 0.0 && above > 0.0).then(|| runs_exact(count, below, above));
        let values = [
            Some(value),
            Some(below),
            Some(above),
            Some(n),
            Some(count),
            z,
            z.map(normal_p),
            exact.map(|e| e[1]),
            exact.map(|e| e[2]),
        ];
        for (row, value) in rows.iter_mut().zip(values) {
            row.1.push(value);
        }
    }
    let mut keep = rows;
    if !run.exact {
        keep.truncate(7);
    }
    let mut t = statistics_table("Runs Test", &names, keep);
    t.caption = Some(format!(
        "Test value: {}.",
        match cut {
            RunsCut::Mean => "the mean",
            RunsCut::Median => "the median",
            RunsCut::Mode => "the mode",
            RunsCut::Value(_) => "as given",
        }
    ));
    vec![t]
}

fn one_sample_ks(
    run: &mut Run,
    variables: &[Variable],
    distribution: KsDistribution,
    parameters: &[f64],
) -> Vec<Table> {
    let (group, labels): (&str, &[&str]) = match distribution {
        KsDistribution::Normal => ("Normal Parameters", &["Mean", "Std. Deviation"]),
        KsDistribution::Uniform => ("Uniform Parameters", &["Minimum", "Maximum"]),
        KsDistribution::Poisson => ("Poisson Parameter", &["Mean"]),
        KsDistribution::Exponential => ("Exponential Parameter", &["Mean"]),
    };
    let lilliefors_used = distribution == KsDistribution::Normal && parameters.is_empty();
    let mut columns = vec![String::new(), String::new()];
    columns.extend(variables.iter().map(|v| v.name().to_string()));
    let mut row_labels: Vec<(String, String)> = vec![("N".into(), String::new())];
    for (i, l) in labels.iter().enumerate() {
        row_labels.push((
            if i == 0 { group.into() } else { String::new() },
            l.to_string(),
        ));
    }
    for (i, l) in ["Absolute", "Positive", "Negative"].iter().enumerate() {
        let first = if i == 0 {
            "Most Extreme Differences"
        } else {
            ""
        };
        row_labels.push((first.into(), l.to_string()));
    }
    row_labels.push(("Kolmogorov-Smirnov Z".into(), String::new()));
    row_labels.push(("Asymp. Sig. (2-tailed)".into(), String::new()));
    let mut cells: Vec<Vec<Cell>> = vec![Vec::new(); row_labels.len()];
    for v in variables {
        let cases = run.cases(&[v]);
        let pairs = run.pairs(v, &cases);
        let n: f64 = pairs.iter().map(|p| p.1).sum();
        let values = tally(&pairs);
        let mean = pairs.iter().map(|(x, w)| x * w).sum::<f64>() / n;
        let estimated: Vec<f64> = match distribution {
            KsDistribution::Normal => {
                let squares: f64 = pairs.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
                vec![mean, (squares / (n - 1.0)).sqrt()]
            }
            KsDistribution::Uniform => vec![
                values.first().map_or(f64::NAN, |v| v.0),
                values.last().map_or(f64::NAN, |v| v.0),
            ],
            _ => vec![mean],
        };
        let params = if parameters.is_empty() {
            estimated
        } else {
            parameters.to_vec()
        };
        let problem = if n < 1.0 {
            Some("has no valid cases")
        } else {
            match distribution {
                KsDistribution::Normal if params[1].is_nan() || params[1] <= 0.0 => {
                    Some("has no spread")
                }
                KsDistribution::Uniform
                    if values.first().is_some_and(|f| f.0 < params[0])
                        || values.last().is_some_and(|l| l.0 > params[1]) =>
                {
                    Some("has values outside the uniform range")
                }
                KsDistribution::Uniform if params[1].is_nan() || params[1] <= params[0] => {
                    Some("has no spread")
                }
                KsDistribution::Poisson
                    if values.iter().any(|(x, _)| *x < 0.0 || x.fract() != 0.0) =>
                {
                    Some("has values that are not whole numbers of at least 0")
                }
                KsDistribution::Exponential if values.iter().any(|(x, _)| *x < 0.0) => {
                    Some("has negative values")
                }
                KsDistribution::Poisson | KsDistribution::Exponential
                    if params[0].is_nan() || params[0] <= 0.0 =>
                {
                    Some("has a mean of 0")
                }
                _ => None,
            }
        };
        let mut column: Vec<Cell> = vec![Cell::Number(n)];
        column.extend(params.iter().map(|p| number(Some(*p))));
        if let Some(problem) = problem {
            run.warnings
                .push(format!("K-S: {} {problem}; the test is not done", v.name()));
            column.resize(row_labels.len(), Cell::Empty);
        } else {
            let cdf = |x: f64| -> f64 {
                match distribution {
                    KsDistribution::Normal => normal_cdf((x - params[0]) / params[1]),
                    KsDistribution::Uniform => (x - params[0]) / (params[1] - params[0]),
                    KsDistribution::Exponential => 1.0 - (-x / params[0]).exp(),
                    KsDistribution::Poisson if params[0] >= 100_000.0 => {
                        normal_cdf((x - params[0]) / params[0].sqrt())
                    }
                    KsDistribution::Poisson if x < 0.0 => 0.0,
                    KsDistribution::Poisson => gamma_q(x + 1.0, params[0]),
                }
            };
            let (mut positive, mut negative) = (f64::NEG_INFINITY, f64::INFINITY);
            let mut before = 0.0;
            for &(x, w) in &values {
                let after = before + w;
                let d1 = if distribution == KsDistribution::Poisson {
                    if x > 0.0 {
                        before / n - cdf(x - 1.0)
                    } else {
                        0.0
                    }
                } else {
                    before / n - cdf(x)
                };
                let d2 = after / n - cdf(x);
                positive = positive.max(d1).max(d2);
                negative = negative.min(d1).min(d2);
                before = after;
            }
            let absolute = positive.abs().max(negative.abs());
            let z = n.sqrt() * absolute;
            let sig = if lilliefors_used {
                lilliefors(absolute, n)
            } else {
                Cell::Number(smirnov_p(z))
            };
            column.extend([
                Cell::Number(absolute),
                Cell::Number(positive),
                Cell::Number(negative),
                Cell::Number(z),
                sig,
            ]);
        }
        for (cell, value) in cells.iter_mut().zip(column) {
            cell.push(value);
        }
    }
    let rows = row_labels
        .into_iter()
        .zip(cells)
        .map(|((a, b), values)| {
            let mut row = vec![text(a), text(b)];
            row.extend(values);
            row
        })
        .collect();
    let mut t = table("One-Sample Kolmogorov-Smirnov Test", 2, columns, rows);
    if lilliefors_used {
        t.caption = Some(
            "The significance is the Lilliefors significance, since the normal parameters come from the data."
                .into(),
        );
    }
    vec![t]
}

/// The pairs whose label joins the two names with `between`.
fn pair_names(pairs: &[(Variable, Variable)], between: &str) -> Vec<String> {
    pairs
        .iter()
        .map(|(x, y)| format!("{}{between}{}", x.name(), y.name()))
        .collect()
}

/// (x, y, weight) of the cases valid on both variables of a pair.
fn paired_values(run: &Run, x: &Variable, y: &Variable) -> Vec<(f64, f64, f64)> {
    run.cases(&[x, y])
        .into_iter()
        .map(|c| (Run::value(x, c), Run::value(y, c), run.weights[c]))
        .collect()
}

/// The distribution of the sum of doubled ranks given a random sign each (Wilcoxon), or of
/// `take` items drawn without order from `ranks` (Mann-Whitney), as probabilities by sum. None
/// when the work would be too large.
pub(crate) fn rank_sum_distribution(ranks: &[u64], take: Option<usize>) -> Option<Vec<f64>> {
    let total: u64 = ranks.iter().sum();
    let n = ranks.len();
    let work = match take {
        None => n as f64 * total as f64,
        Some(k) => n as f64 * k as f64 * total as f64,
    };
    if work > 3e8 {
        return None;
    }
    let size = total as usize + 1;
    match take {
        None => {
            let mut p = vec![0.0; size];
            p[0] = 1.0;
            let mut top = 0;
            for &r in ranks {
                let r = r as usize;
                for s in (0..=top).rev() {
                    let half = p[s] / 2.0;
                    p[s] = half;
                    p[s + r] += half;
                }
                top += r;
            }
            Some(p)
        }
        Some(k) => {
            // counts[j][s]: ways to choose j items with sum s, kept as fractions of C(n, k)
            let mut counts = vec![vec![0.0; size]; k + 1];
            counts[0][0] = 1.0;
            let mut top = 0;
            for (i, &r) in ranks.iter().enumerate() {
                let r = r as usize;
                for j in (1..=k.min(i + 1)).rev() {
                    let (lower, upper) = counts.split_at_mut(j);
                    for s in (0..=top).rev() {
                        upper[0][s + r] += lower[j - 1][s];
                    }
                }
                top += r;
            }
            let ways = ln_choose(n as f64, k as f64).exp();
            Some(counts[k].iter().map(|c| c / ways).collect())
        }
    }
}

/// Exact two-tailed, one-tailed (the observed side) and point probability of `observed` under
/// a distribution symmetric about `mean`, by doubled sums.
pub(crate) fn exact_tails(p: &[f64], observed: usize, mean: f64) -> [f64; 3] {
    let lower: f64 = p[..=observed.min(p.len() - 1)].iter().sum();
    let upper: f64 = p[observed.min(p.len() - 1)..].iter().sum();
    let distance = (observed as f64 - mean).abs();
    let two: f64 = p
        .iter()
        .enumerate()
        .filter(|(s, _)| (*s as f64 - mean).abs() >= distance - 1e-7)
        .map(|(_, p)| p)
        .sum();
    [
        two.min(1.0),
        lower.min(upper).min(1.0),
        p[observed.min(p.len() - 1)],
    ]
}

fn wilcoxon(run: &mut Run, pairs: &[(Variable, Variable)]) -> Vec<Table> {
    let names = pair_names(pairs, " - ");
    let columns = ["", "", "N", "Mean Rank", "Sum of Ranks"]
        .map(String::from)
        .to_vec();
    let mut rank_rows = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "Z",
        "Asymp. Sig. (2-tailed)",
        "Exact Sig. (2-tailed)",
        "Exact Sig. (1-tailed)",
        "Point Probability",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    let mut bases = Vec::new();
    for ((x, y), name) in pairs.iter().zip(&names) {
        let cases = paired_values(run, x, y);
        let differences: Vec<(f64, f64)> = cases.iter().map(|(a, b, w)| (a - b, *w)).collect();
        let nonzero: Vec<(f64, f64)> = differences.iter().copied().filter(|d| d.0 != 0.0).collect();
        let ties: f64 = differences.iter().filter(|d| d.0 == 0.0).map(|d| d.1).sum();
        let absolute: Vec<(f64, f64)> = nonzero.iter().map(|(d, w)| (d.abs(), *w)).collect();
        let (ranks, t) = tied_ranks(&absolute);
        let (mut sp, mut sn, mut np, mut nn) = (0.0, 0.0, 0.0, 0.0);
        for ((d, w), r) in nonzero.iter().zip(&ranks) {
            if *d > 0.0 {
                sp += w * r;
                np += w;
            } else {
                sn += w * r;
                nn += w;
            }
        }
        let n = np + nn;
        let mean_rank = |s: f64, k: f64| {
            if k > 0.0 {
                Cell::Number(s / k)
            } else {
                Cell::Empty
            }
        };
        rank_rows.push(vec![
            text(name.clone()),
            text("Negative Ranks"),
            Cell::Number(nn),
            mean_rank(sn, nn),
            Cell::Number(sn),
        ]);
        rank_rows.push(vec![
            Cell::Empty,
            text("Positive Ranks"),
            Cell::Number(np),
            mean_rank(sp, np),
            Cell::Number(sp),
        ]);
        rank_rows.push(vec![
            Cell::Empty,
            text("Ties"),
            Cell::Number(ties),
            Cell::Empty,
            Cell::Empty,
        ]);
        rank_rows.push(vec![
            Cell::Empty,
            text("Total"),
            Cell::Number(n + ties),
            Cell::Empty,
            Cell::Empty,
        ]);
        let variance = n * (n + 1.0) * (2.0 * n + 1.0) / 24.0 - t / 48.0;
        let z = (variance > 0.0).then(|| (sp.min(sn) - n * (n + 1.0) / 4.0) / variance.sqrt());
        bases.push(if sp < sn { "positive" } else { "negative" });
        let exact = if run.exact && n > 0.0 {
            let doubled: Vec<u64> = nonzero
                .iter()
                .zip(&ranks)
                .flat_map(|((_, w), r)| std::iter::repeat_n((2.0 * r).round() as u64, *w as usize))
                .collect();
            let dist = rank_sum_distribution(&doubled, None);
            if dist.is_none() {
                run.warnings.push(format!(
                    "WILCOXON: {name} has too many cases for the exact test"
                ));
            }
            let mean = doubled.iter().sum::<u64>() as f64 / 2.0;
            dist.map(|p| exact_tails(&p, (2.0 * sp.min(sn)).round() as usize, mean))
        } else {
            None
        };
        let values = [
            z,
            z.map(normal_p),
            exact.map(|e| e[0]),
            exact.map(|e| e[1]),
            exact.map(|e| e[2]),
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
    }
    let mut test = statistics_table("Wilcoxon Signed Ranks Test", &names, used(stats));
    let notes: Vec<String> = names
        .iter()
        .zip(&bases)
        .map(|(name, base)| format!("{name}: Z is based on {base} ranks."))
        .collect();
    test.caption = (!notes.is_empty()).then(|| notes.join(" "));
    vec![table("Ranks", 2, columns, rank_rows), test]
}

fn sign(run: &mut Run, pairs: &[(Variable, Variable)]) -> Vec<Table> {
    let names = pair_names(pairs, " - ");
    let mut rows = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "Z",
        "Asymp. Sig. (2-tailed)",
        "Exact Sig. (2-tailed)",
        "Exact Sig. (1-tailed)",
        "Point Probability",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    for ((x, y), name) in pairs.iter().zip(&names) {
        let cases = paired_values(run, x, y);
        let (mut np, mut nn, mut ties) = (0.0, 0.0, 0.0);
        for (a, b, w) in &cases {
            match (a - b).partial_cmp(&0.0) {
                Some(std::cmp::Ordering::Greater) => np += w,
                Some(std::cmp::Ordering::Less) => nn += w,
                _ => ties += w,
            }
        }
        let n = np + nn;
        for (i, (label, value)) in [
            ("Negative Differences", nn),
            ("Positive Differences", np),
            ("Ties", ties),
            ("Total", n + ties),
        ]
        .into_iter()
        .enumerate()
        {
            let first = if i == 0 {
                text(name.clone())
            } else {
                Cell::Empty
            };
            rows.push(vec![first, text(label), Cell::Number(value)]);
        }
        let small = n <= 25.0;
        let z = (!small).then(|| (np.max(nn) - 0.5 * n - 0.5) / (0.5 * n.sqrt()));
        let exact = (small || run.exact).then(|| sign_exact(np, nn));
        let values = [
            z,
            z.map(normal_p),
            exact.map(|e| e[0]),
            exact.filter(|_| run.exact).map(|e| e[1]),
            exact.filter(|_| run.exact).map(|e| e[2]),
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
    }
    let columns = ["", "", "N"].map(String::from).to_vec();
    let mut test = statistics_table("Sign Test", &names, used(stats));
    test.caption = Some("Up to 25 differences the binomial distribution is used.".into());
    vec![table("Sign Test Frequencies", 2, columns, rows), test]
}

/// The McNemar test of pairs whose values are two values together.
fn mcnemar(run: &mut Run, pairs: &[(Variable, Variable)]) -> Vec<Table> {
    let mut out = Vec::new();
    let mut names = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "N",
        "Chi-Square",
        "Asymp. Sig.",
        "Exact Sig. (2-tailed)",
        "Exact Sig. (1-tailed)",
        "Point Probability",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    for (x, y) in pairs {
        let cases = paired_values(run, x, y);
        let both: Vec<(f64, f64)> = cases
            .iter()
            .flat_map(|(a, b, w)| [(*a, *w), (*b, *w)])
            .collect();
        let values: Vec<f64> = tally(&both).into_iter().map(|t| t.0).collect();
        if values.len() != 2 {
            run.warnings.push(format!(
                "MCNEMAR: {} and {} have {} values together, not two; the pair is not tested",
                x.name(),
                y.name(),
                values.len()
            ));
            continue;
        }
        let count = |a: f64, b: f64| -> f64 {
            cases
                .iter()
                .filter(|c| c.0 == a && c.1 == b)
                .map(|c| c.2)
                .sum()
        };
        let mut rows = Vec::new();
        for &a in &values {
            let mut row = vec![text(run.label(x, a))];
            row.extend(values.iter().map(|&b| Cell::Number(count(a, b))));
            rows.push(row);
        }
        let mut columns = vec![String::new()];
        columns.extend(values.iter().map(|&b| run.label(y, b)));
        let mut groups = vec![vec![x.name().to_string()]];
        groups.extend(values.iter().map(|_| vec![y.name().to_string()]));
        out.push(Table {
            title: format!("{} & {}", x.name(), y.name()),
            row_headers: 1,
            columns,
            column_groups: groups,
            rows,
            ..Table::default()
        });
        let n1 = count(values[0], values[1]);
        let n2 = count(values[1], values[0]);
        let n = n1 + n2;
        let total: f64 = cases.iter().map(|c| c.2).sum();
        let small = n <= 25.0;
        let chi = (!small).then(|| ((n1 - n2).abs() - 1.0).powi(2) / n);
        let exact = (small || run.exact).then(|| sign_exact(n1, n2));
        let values = [
            Some(total),
            chi,
            chi.map(|c| chi_square_p(c, 1.0)),
            exact.map(|e| e[0]),
            exact.filter(|_| run.exact).map(|e| e[1]),
            exact.filter(|_| run.exact).map(|e| e[2]),
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
        names.push(format!("{} & {}", x.name(), y.name()));
    }
    if !names.is_empty() {
        let mut test = statistics_table("McNemar Test", &names, used(stats));
        test.caption = Some(
            "Up to 25 changes the binomial distribution is used, else chi-square with a continuity correction."
                .into(),
        );
        out.push(test);
    }
    out
}

fn marginal_homogeneity(run: &mut Run, pairs: &[(Variable, Variable)]) -> Vec<Table> {
    run.no_exact("marginal homogeneity test");
    let mut names = Vec::new();
    let mut two_valued = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "Distinct Values",
        "Off-Diagonal Cases",
        "Observed MH Statistic",
        "Mean MH Statistic",
        "Std. Deviation of MH Statistic",
        "Std. MH Statistic",
        "Asymp. Sig. (2-tailed)",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    for (x, y) in pairs {
        let cases = paired_values(run, x, y);
        let both: Vec<(f64, f64)> = cases
            .iter()
            .flat_map(|(a, b, w)| [(*a, *w), (*b, *w)])
            .collect();
        let distinct = tally(&both).len();
        if distinct == 2 {
            // with two values the test is McNemar's
            two_valued.push((*x, *y));
            continue;
        }
        let (mut observed, mut mean, mut variance, mut off) = (0.0, 0.0, 0.0, 0.0);
        for (u, v, w) in &cases {
            if u != v {
                observed += w * u;
                mean += w * (u + v) / 2.0;
                variance += w * ((u - v) / 2.0).powi(2);
                off += w;
            }
        }
        let sd = variance.sqrt();
        let z = (sd > 0.0).then(|| (observed - mean) / sd);
        let values = [
            Some(distinct as f64),
            Some(off),
            Some(observed),
            Some(mean),
            Some(sd),
            z,
            z.map(normal_p),
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
        names.push(format!("{} & {}", x.name(), y.name()));
    }
    let mut out = Vec::new();
    if !names.is_empty() {
        out.push(statistics_table("Marginal Homogeneity Test", &names, stats));
    }
    if !two_valued.is_empty() {
        out.extend(mcnemar(run, &two_valued));
    }
    out
}

fn related(run: &mut Run, variables: &[Variable], test: RelatedTest) -> Vec<Table> {
    let refs: Vec<&Variable> = variables.iter().collect();
    let cases = run.cases(&refs);
    let k = variables.len() as f64;
    let n: f64 = cases.iter().map(|&c| run.weights[c]).sum();
    let title = match test {
        RelatedTest::Friedman => "Friedman Test",
        RelatedTest::Kendall => "Kendall's W Test",
        RelatedTest::Cochran => "Cochran Test",
    };
    let one_column = |rows: Vec<(&str, Option<f64>)>| {
        let rows = rows
            .into_iter()
            .map(|(name, value)| vec![text(name), number(value)])
            .collect();
        table(title, 1, vec![String::new(), "Value".into()], rows)
    };
    if test == RelatedTest::Cochran {
        let all: Vec<(f64, f64)> = cases
            .iter()
            .flat_map(|&c| variables.iter().map(move |v| (Run::value(v, c), 1.0)))
            .collect();
        let values: Vec<f64> = tally(&all).into_iter().map(|t| t.0).collect();
        if values.len() != 2 {
            run.warnings.push(format!(
                "COCHRAN: the variables have {} values together, not two; the test is not done",
                values.len()
            ));
            return Vec::new();
        }
        let success = values[1];
        let mut frequency_rows = Vec::new();
        let (mut sum_c, mut sum_c2) = (0.0, 0.0);
        for v in variables {
            let c: f64 = cases
                .iter()
                .filter(|&&i| Run::value(v, i) == success)
                .map(|&i| run.weights[i])
                .sum();
            sum_c += c;
            sum_c2 += c * c;
            frequency_rows.push(vec![text(v.name()), Cell::Number(n - c), Cell::Number(c)]);
        }
        let sum_r2: f64 = cases
            .iter()
            .map(|&i| {
                let r = variables
                    .iter()
                    .filter(|v| Run::value(v, i) == success)
                    .count() as f64;
                run.weights[i] * r * r
            })
            .sum();
        let denominator = k * sum_c - sum_r2;
        let q = (denominator > 0.0).then(|| (k - 1.0) * (k * sum_c2 - sum_c * sum_c) / denominator);
        let mut columns = vec![String::new()];
        columns.extend(values.iter().map(|&x| run.label(&variables[0], x)));
        let frequencies = Table {
            title: "Cochran Frequencies".into(),
            row_headers: 1,
            column_groups: vec![Vec::new(), vec!["Value".into()], vec!["Value".into()]],
            columns,
            rows: frequency_rows,
            ..Table::default()
        };
        let stats = one_column(vec![
            ("N", Some(n)),
            ("Cochran's Q", q),
            ("df", Some(k - 1.0)),
            ("Asymp. Sig.", q.map(|q| chi_square_p(q, k - 1.0))),
        ]);
        run.no_exact("Cochran test");
        return vec![frequencies, stats];
    }
    run.no_exact(if test == RelatedTest::Friedman {
        "Friedman test"
    } else {
        "Kendall's W test"
    });
    let mut sums = vec![0.0; variables.len()];
    let mut ties = 0.0;
    for &c in &cases {
        let row: Vec<(f64, f64)> = variables.iter().map(|v| (Run::value(v, c), 1.0)).collect();
        let (ranks, t) = tied_ranks(&row);
        for (sum, r) in sums.iter_mut().zip(ranks) {
            *sum += run.weights[c] * r;
        }
        ties += run.weights[c] * t;
    }
    let squares: f64 = sums.iter().map(|s| s * s).sum();
    let numerator = 12.0 / (n * k * (k + 1.0)) * squares - 3.0 * n * (k + 1.0);
    let denominator = 1.0 - ties / (n * k * (k * k - 1.0));
    let chi = (n > 0.0 && denominator > 1e-12).then(|| numerator / denominator);
    let ranks = Table {
        title: "Ranks".into(),
        row_headers: 1,
        columns: vec![String::new(), "Mean Rank".into()],
        column_groups: vec![Vec::new(); 2],
        rows: variables
            .iter()
            .zip(&sums)
            .map(|(v, s)| vec![text(v.name()), number(Some(s / n))])
            .collect(),
        ..Table::default()
    };
    let sig = chi.map(|c| chi_square_p(c, k - 1.0));
    let mut rows = vec![("N", Some(n))];
    if test == RelatedTest::Kendall {
        rows.push(("Kendall's W", chi.map(|c| c / (n * (k - 1.0)))));
    }
    rows.extend([
        ("Chi-Square", chi),
        ("df", Some(k - 1.0)),
        ("Asymp. Sig.", sig),
    ]);
    vec![ranks, one_column(rows)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binomial_tails_by_sums() {
        // P(X <= 3) for n = 10, p = 0.3, summed term by term
        let direct: f64 = (0..=3).map(|k| binomial_pmf(k as f64, 10.0, 0.3)).sum();
        assert!((binomial_cdf(3.0, 10.0, 0.3) - direct).abs() < 1e-12);
        assert!((binomial_upper(4.0, 10.0, 0.3) - (1.0 - direct)).abs() < 1e-12);
    }

    #[test]
    fn runs_distribution_sums_to_one() {
        let total: f64 = (2..=13).map(|r| runs_pmf(r, 6.0, 7.0)).sum();
        assert!((total - 1.0).abs() < 1e-12);
        // two runs of 6 and 7: 2 of the C(13, 6) = 1716 orders
        assert!((runs_pmf(2, 6.0, 7.0) - 2.0 / 1716.0).abs() < 1e-15);
    }

    #[test]
    fn signed_rank_distribution_by_hand() {
        // ranks 1, 2, 3 doubled: sums 0..12 in steps of 2, each sign pattern 1/8
        let p = rank_sum_distribution(&[2, 4, 6], None).unwrap();
        assert_eq!(p[0], 0.125);
        assert_eq!(p[6], 0.25);
        assert_eq!(p[12], 0.125);
        // choose 2 of ranks 1, 2, 3: sums 3, 4, 5 each 1/3
        let q = rank_sum_distribution(&[1, 2, 3], Some(2)).unwrap();
        assert!((q[3] - 1.0 / 3.0).abs() < 1e-12 && (q[5] - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn smirnov_is_continuous_enough() {
        assert_eq!(smirnov_p(0.2), 1.0);
        assert!((smirnov_p(0.999_999) - smirnov_p(1.0)).abs() < 1e-4);
        assert!((smirnov_p(1.36) - 0.0494).abs() < 1e-3);
    }
}
