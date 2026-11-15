//! NPTESTS: one-sample, independent-samples and related-samples nonparametric tests, chosen by
//! measurement level or named, with a hypothesis summary, detail tables, confidence intervals
//! and multiple comparisons. The test statistics come from the NPAR TESTS code.
//!
//! Rules from the command's description and the Nonparametric Tests chapter of the algorithms:
//! - Nominal and ordinal fields are categorical, scale fields continuous; an unknown level is
//!   found from the data. USERMISSING=INCLUDE makes user-missing values valid on categorical
//!   fields only. SCOPE=ANALYSIS drops a case from a test missing on a field of that test;
//!   LISTWISE from every test when missing on any field named.
//! - Without test keywords: one-sample tests are binomial (two values), chi-square (more) or
//!   Kolmogorov-Smirnov against a normal with the sample's mean and SD (continuous);
//!   independent samples are Mann-Whitney (two groups) or Kruskal-Wallis; related fields are
//!   split by level, and two or more continuous fields get Wilcoxon or Friedman, categorical
//!   fields with two values together McNemar or Cochran.
//! - Binomial: exact up to 25 cases, p1 = min of the two tails; else the normal approximation
//!   with a continuity correction. Two-sided (2 p1) when the test value is 0.5, else p1 decides.
//!   Clopper-Pearson and Jeffreys limits are beta quantiles; likelihood limits solve
//!   2 (l(p^) - l(p)) = the chi-square quantile with one degree of freedom.
//! - K-S with estimated parameters takes a Monte Carlo significance (Lilliefors' idea: samples
//!   from the fitted family, parameters estimated again each time); NORMAL with
//!   SIMULATION=FALSE uses the tabled Lilliefors significance. Given parameters use Smirnov's.
//! - Runs: categorical fields split into two groups by their two values or a list; continuous
//!   fields at a cut point, values at or below it in the first group.
//! - Mann-Whitney U counts the pairs with the first group's case below (ties half); exact when
//!   n1 n2 <= 400 and n1 n2 / 2 + min(n1, n2) <= 220. Wald-Wolfowitz decides on the most runs
//!   ties allow, exact up to 30 cases. Moses trims h records from each end (5% of the control
//!   group, at least 1, at most (nc - 2) / 2) and rejects at p <= alpha.
//! - Hodges-Lehmann: the weighted median of the pairwise differences (independent) or of the
//!   Walsh averages (related), with limits at the ranks the normal approximation of the
//!   Mann-Whitney or signed-rank statistic gives.
//! - Jonckheere-Terpstra decides one-sided in the direction ORDER gives.
//! - Multiple comparisons follow a rejected k-sample test with three samples or more:
//!   pairwise (Dunn's statistics, Bonferroni-adjusted) or stepwise step-down (homogeneous
//!   subsets of the sorted samples).

use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::dist::{beta_i, chi_square_p, normal_cdf, normal_p, normal_quantile};
use crate::examine::lilliefors;
use crate::means::is_valid;
use crate::npar_groups::{Grouped, moses_p, run_bounds};
use crate::npar_tests::{
    SignedRanks, cochran, friedman, ks_cdf, ks_extremes, ks_problem, marginal_statistic, middle,
    runs_exact, runs_z, sign_exact, smirnov_p, tally, text,
};
use crate::random::{Random, time_seed};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{
    KsDistribution, NpCompare, NpCut, NpIndependent, NpOneSample, NpParameters, NpRelated, Nptests,
    Value, VarItem,
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Level {
    Nominal,
    Ordinal,
    Scale,
}

/// The rows of one kind of detail table, a column per tested field or pair.
struct Detail {
    title: String,
    names: Vec<String>,
    rows: Vec<(String, Vec<Cell>)>,
}

/// One NPTESTS run.
struct Np<'d> {
    data: &'d Dataset,
    weights: Vec<f64>,
    include: bool,
    complete: Option<Vec<bool>>,
    scale_min: usize,
    shown: Shown,
    alpha: f64,
    cilevel: f64,
    seed: u64,
    summary: Vec<Vec<Cell>>,
    intervals: Vec<Vec<Cell>>,
    details: Vec<Detail>,
    comparisons: Vec<Table>,
    warnings: Vec<String>,
}

/// What a test gives the summary: its significance, the kind, and whether p <= alpha rejects.
struct Sig {
    p: Option<f64>,
    shown: Cell,
    kind: &'static str,
    inclusive: bool,
}

impl Sig {
    fn new(p: Option<f64>, kind: &'static str) -> Self {
        let shown = crate::descriptives::number(p);
        Sig {
            p,
            shown,
            kind,
            inclusive: false,
        }
    }
}

fn key_matches(key: &Key, value: &Value) -> bool {
    match (key, value) {
        (Key::Number(a), Value::Number(b)) => a == b,
        (Key::Text(a), Value::String(b)) => a.trim_end() == b.trim_end(),
        _ => false,
    }
}

fn number_value(variable: &Variable, case: usize) -> f64 {
    match variable.values() {
        Values::Numeric(values) => values.get(case).unwrap_or(f64::NAN),
        Values::String(_) => f64::NAN,
    }
}

fn key_of(variable: &Variable, case: usize) -> Key {
    match variable.values() {
        Values::Numeric(values) => Key::Number(values.get(case).unwrap_or(f64::NAN)),
        Values::String(values) => Key::Text(values.get(case).trim_end().to_string()),
    }
}

/// Distinct keys in the order first met.
fn first_found(keys: &[Key]) -> Vec<Key> {
    let mut out: Vec<Key> = Vec::new();
    for k in keys {
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    out
}

fn sorted(mut keys: Vec<Key>) -> Vec<Key> {
    keys.sort_by(|a, b| a.compare(b));
    keys
}

/// Values sorted with their weights, and the weighted median as the algorithms define it.
fn weighted_sorted(mut values: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    values
}

/// The value at 1-based position `i` of weighted sorted values.
fn at_position(sorted: &[(f64, f64)], i: f64) -> f64 {
    let mut seen = 0.0;
    for (x, w) in sorted {
        seen += w;
        if seen >= i - 1e-9 {
            return *x;
        }
    }
    sorted.last().map_or(f64::NAN, |v| v.0)
}

fn weighted_median(sorted: &[(f64, f64)]) -> f64 {
    let total: f64 = sorted.iter().map(|v| v.1).sum();
    let half = (total + 1.0) / 2.0;
    if total % 2.0 == 0.0 {
        (at_position(sorted, half.floor()) + at_position(sorted, half.ceil())) / 2.0
    } else {
        at_position(sorted, half)
    }
}

/// The root of a monotone `f` between `low` and `high` by halving.
fn bisect(f: impl Fn(f64) -> f64, mut low: f64, mut high: f64) -> f64 {
    let rising = f(high) > f(low);
    for _ in 0..200 {
        let mid = (low + high) / 2.0;
        if (f(mid) < 0.0) == rising {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.0
}

/// The `q` quantile of the beta distribution.
fn beta_quantile(q: f64, a: f64, b: f64) -> f64 {
    bisect(|x| beta_i(a, b, x) - q, 0.0, 1.0)
}

/// The share of draws at or above `observed` and its confidence interval at `level` percent.
fn monte_carlo(at_or_above: usize, samples: usize, level: f64) -> [f64; 3] {
    let n = samples as f64;
    let p = at_or_above as f64 / n;
    let z = normal_quantile(1.0 - (1.0 - level / 100.0) / 2.0);
    let half = z * (p * (1.0 - p) / n).sqrt();
    [p, (p - half).max(0.0), (p + half).min(1.0)]
}

impl Engine {
    pub(crate) fn nptests(&mut self, command: &Nptests) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut named: Vec<String> = Vec::new();
        let mut add = |names: Vec<String>| {
            for name in names {
                if !named.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    named.push(name);
                }
            }
        };
        let lists: Vec<&[VarItem]> = [
            command.one_sample.as_ref().map(|s| &s.fields[..]),
            command.independent.as_ref().map(|s| &s.fields[..]),
            command.related.as_ref().map(|s| &s.fields[..]),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut expanded = Vec::new();
        for list in &lists {
            let names = self.npar_names(list)?;
            add(names.clone());
            expanded.push(names);
        }
        if let Some(independent) = &command.independent {
            add(vec![
                self.expand(&[VarItem::Name(independent.group.clone())])?
                    .remove(0),
            ]);
        }
        if let Some(weight) = data.weight_variable()
            && named.iter().any(|n| n.eq_ignore_ascii_case(weight))
        {
            return Err(Error(format!(
                "NPTESTS cannot test the weight variable {weight}"
            )));
        }
        let variables: Vec<Variable> = named
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let weights: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; data.case_count()],
        };
        let mut np = Np {
            data,
            weights,
            include: command.include,
            complete: None,
            scale_min: self.scale_min(),
            shown: self.shown(),
            alpha: command.alpha,
            cilevel: command.cilevel,
            seed: command.seed.unwrap_or_else(time_seed),
            summary: Vec::new(),
            intervals: Vec::new(),
            details: Vec::new(),
            comparisons: Vec::new(),
            warnings: Vec::new(),
        };
        if command.listwise {
            let complete = (0..data.case_count())
                .map(|c| variables.iter().all(|v| np.valid(v, c)))
                .collect();
            np.complete = Some(complete);
        }
        let mut lists = expanded.into_iter();
        if let Some(one) = &command.one_sample {
            np.one_sample(one, &lists.next().unwrap_or_default())?;
        }
        if let Some(independent) = &command.independent {
            np.independent(independent, &lists.next().unwrap_or_default())?;
        }
        if let Some(related) = &command.related {
            np.related(related, &lists.next().unwrap_or_default())?;
        }
        let tables = np.tables();
        for warning in std::mem::take(&mut np.warnings) {
            self.warn(warning);
        }
        Ok(tables)
    }

    fn npar_names(&self, items: &[VarItem]) -> Result<Vec<String>, Error> {
        let mut out: Vec<String> = Vec::new();
        for name in self.expand(items)? {
            if !out.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                out.push(name);
            }
        }
        Ok(out)
    }
}

impl<'d> Np<'d> {
    fn level(&self, variable: &Variable) -> Level {
        let measure = match (
            variable.measure(),
            self.data.variable_index(variable.name()),
        ) {
            (Measure::Unknown, Some(index)) => {
                crate::pass::default_measure(self.data, index, self.scale_min)
            }
            (known, _) => known,
        };
        match measure {
            Measure::Scale if !variable.is_string() => Level::Scale,
            Measure::Ordinal => Level::Ordinal,
            _ => Level::Nominal,
        }
    }

    fn valid(&self, variable: &Variable, case: usize) -> bool {
        let categorical = self.level(variable) != Level::Scale;
        is_valid(variable, case, self.include && categorical)
    }

    /// The cases with a weight valid on every one of `variables`, in file order.
    fn cases(&self, variables: &[&Variable]) -> Vec<usize> {
        (0..self.data.case_count())
            .filter(|&c| self.weights[c] > 0.0)
            .filter(|&c| self.complete.as_ref().is_none_or(|m| m[c]))
            .filter(|&c| variables.iter().all(|v| self.valid(v, c)))
            .collect()
    }

    fn name(&self, variable: &Variable) -> String {
        self.shown.variable(variable)
    }

    fn value(&self, variable: &Variable, key: &Key) -> String {
        self.shown.value(variable, key)
    }

    fn values(&self, variable: &Variable, values: &[Value]) -> String {
        values
            .iter()
            .map(|v| match v {
                Value::Number(n) => self.value(variable, &Key::Number(*n)),
                Value::String(s) => self.value(variable, &Key::Text(s.trim_end().to_string())),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn summarize(&mut self, null: String, test: &str, sig: Sig) {
        let reject = sig
            .p
            .is_some_and(|p| p < self.alpha || (sig.inclusive && p <= self.alpha));
        let decision = match sig.p {
            Some(_) if reject => "Reject the null hypothesis.",
            Some(_) => "Retain the null hypothesis.",
            None => "",
        };
        self.summary.push(vec![
            text(null),
            text(test),
            sig.shown,
            text(sig.kind),
            text(decision),
        ]);
    }

    fn detail(&mut self, title: &str, name: String, rows: Vec<(&str, Cell)>) {
        let at = match self.details.iter().position(|d| d.title == title) {
            Some(at) => at,
            None => {
                self.details.push(Detail {
                    title: title.to_string(),
                    names: Vec::new(),
                    rows: Vec::new(),
                });
                self.details.len() - 1
            }
        };
        let detail = &mut self.details[at];
        let before = detail.names.len();
        detail.names.push(name);
        for row in detail.rows.iter_mut() {
            row.1.push(Cell::Empty);
        }
        // a new row goes after the row this column filled last
        let mut after = 0;
        for (label, cell) in rows {
            match detail.rows.iter().position(|r| r.0 == label) {
                Some(at) => {
                    detail.rows[at].1[before] = cell;
                    after = at + 1;
                }
                None => {
                    let mut cells = vec![Cell::Empty; before];
                    cells.push(cell);
                    detail.rows.insert(after, (label.to_string(), cells));
                    after += 1;
                }
            }
        }
    }

    fn interval(&mut self, kind: &str, parameter: String, estimate: f64, limits: [f64; 2]) {
        self.intervals.push(vec![
            text(kind),
            text(parameter),
            Cell::Number(estimate),
            Cell::Number(limits[0]),
            Cell::Number(limits[1]),
        ]);
    }

    fn tables(&mut self) -> Vec<Table> {
        let mut out = Vec::new();
        let columns = ["Null Hypothesis", "Test", "Sig.", "Sig. Type", "Decision"];
        out.push(Table {
            title: "Hypothesis Test Summary".into(),
            row_headers: 0,
            columns: columns.map(String::from).to_vec(),
            column_groups: vec![Vec::new(); 5],
            rows: std::mem::take(&mut self.summary),
            caption: Some(format!("The significance level is {}.", self.alpha)),
            ..Table::default()
        });
        if !self.intervals.is_empty() {
            let level = format!("{}% Confidence Interval", self.cilevel);
            let columns = [
                "Confidence Interval Type",
                "Parameter",
                "Estimate",
                "Lower",
                "Upper",
            ];
            out.push(Table {
                title: "Confidence Interval Summary".into(),
                row_headers: 0,
                columns: columns.map(String::from).to_vec(),
                column_groups: vec![
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    vec![level.clone()],
                    vec![level],
                ],
                rows: std::mem::take(&mut self.intervals),
                ..Table::default()
            });
        }
        for detail in std::mem::take(&mut self.details) {
            let mut columns = vec![String::new()];
            columns.extend(detail.names);
            let rows = detail
                .rows
                .into_iter()
                .map(|(label, cells)| {
                    let mut row = vec![text(label)];
                    row.extend(cells);
                    row
                })
                .collect();
            out.push(crate::npar_tests::table(detail.title, 1, columns, rows));
        }
        out.append(&mut self.comparisons);
        out
    }

    // ---------------------------------------------------------------- one sample

    fn one_sample(&mut self, spec: &NpOneSample, names: &[String]) -> Result<(), Error> {
        let data = self.data;
        for name in names {
            let v = data.variable(name)?;
            let level = self.level(&v);
            let cases = self.cases(&[&v]);
            let numeric = !v.is_string();
            if spec.automatic() {
                if level == Level::Scale {
                    let default = statscraft_syntax::NpKs {
                        samples: 1000,
                        mc_cilevel: 99.0,
                        normal: Some(NpParameters::Sample { simulation: true }),
                        uniform: None,
                        exponential: None,
                        poisson: None,
                    };
                    self.ks(&v, &cases, &default);
                } else {
                    let keys: Vec<Key> = cases.iter().map(|&c| key_of(&v, c)).collect();
                    match first_found(&keys).len() {
                        2 => self.binomial(&v, &cases, level, &default_binomial()),
                        k if k > 2 => self.chi_square(&v, &cases, None),
                        k => self.warnings.push(format!(
                            "NPTESTS: {name} has {k} values among its valid cases; no one-sample test"
                        )),
                    }
                }
                continue;
            }
            if let Some(custom) = &spec.chisquare {
                if level == Level::Scale {
                    self.skip("CHISQUARE", name, "a continuous field");
                } else {
                    self.chi_square(&v, &cases, custom.as_deref());
                }
            }
            if let Some(binomial) = &spec.binomial {
                self.binomial(&v, &cases, level, binomial);
            }
            if let Some(ks) = &spec.ks {
                if level == Level::Nominal || !numeric {
                    self.skip(
                        "KOLMOGOROV_SMIRNOV",
                        name,
                        "not a numeric continuous or ordinal field",
                    );
                } else {
                    self.ks(&v, &cases, ks);
                }
            }
            if let Some(runs) = &spec.runs {
                self.runs(&v, &cases, level, runs);
            }
            if let Some(theta) = spec.wilcoxon {
                if level == Level::Nominal || !numeric {
                    self.skip(
                        "WILCOXON",
                        name,
                        "not a numeric continuous or ordinal field",
                    );
                } else {
                    self.wilcoxon_one(&v, &cases, theta);
                }
            }
        }
        Ok(())
    }

    fn skip(&mut self, test: &str, name: &str, why: &str) {
        self.warnings
            .push(format!("NPTESTS: {test} does not test {name}: {why}"));
    }

    fn total(&self, cases: &[usize]) -> f64 {
        cases.iter().map(|&c| self.weights[c]).sum()
    }

    fn chi_square(&mut self, v: &Variable, cases: &[usize], custom: Option<&[(Value, f64)]>) {
        let name = self.name(v);
        let n = self.total(cases);
        let keys: Vec<(Key, f64)> = cases
            .iter()
            .map(|&c| (key_of(v, c), self.weights[c]))
            .collect();
        // (category, observed, expected share)
        let cells: Vec<(f64, f64)> = match custom {
            None => {
                let categories = sorted(first_found(
                    &keys.iter().map(|k| k.0.clone()).collect::<Vec<_>>(),
                ));
                let k = categories.len() as f64;
                categories
                    .iter()
                    .map(|cat| {
                        let o = keys.iter().filter(|k| &k.0 == cat).map(|k| k.1).sum();
                        (o, 1.0 / k)
                    })
                    .collect()
            }
            Some(list) => {
                if keys
                    .iter()
                    .any(|(key, _)| !list.iter().any(|(value, _)| key_matches(key, value)))
                {
                    self.warnings.push(format!(
                        "NPTESTS: {name} has values outside CATEGORIES; no chi-square test"
                    ));
                    return;
                }
                let sum: f64 = list.iter().map(|c| c.1).sum();
                list.iter()
                    .map(|(value, f)| {
                        let o = keys
                            .iter()
                            .filter(|k| key_matches(&k.0, value))
                            .map(|k| k.1)
                            .sum();
                        (o, f / sum)
                    })
                    .collect()
            }
        };
        if cells.len() < 2 || n <= 0.0 {
            self.warnings.push(format!(
                "NPTESTS: {name} needs two categories and cases for the chi-square test"
            ));
            return;
        }
        let chi: f64 = cells
            .iter()
            .map(|(o, share)| (o - share * n).powi(2) / (share * n))
            .sum();
        let df = cells.len() as f64 - 1.0;
        let p = chi_square_p(chi, df);
        let null = match custom {
            None => format!("The categories of {name} are equally likely."),
            Some(_) => format!("The categories of {name} have the proportions EXPECTED gives."),
        };
        self.summarize(
            null,
            "One-Sample Chi-Square Test",
            Sig::new(Some(p), "Asymptotic"),
        );
        self.detail(
            "One-Sample Chi-Square Test",
            name,
            vec![
                ("Total N", Cell::Number(n)),
                ("Test Statistic", Cell::Number(chi)),
                ("Degrees of Freedom", Cell::Number(df)),
                ("Asymptotic Sig.", Cell::Number(p)),
            ],
        );
    }

    fn binomial(
        &mut self,
        v: &Variable,
        cases: &[usize],
        level: Level,
        spec: &statscraft_syntax::NpBinomial,
    ) {
        let name = self.name(v);
        let (success, what): (Vec<bool>, String) = if level == Level::Scale {
            let xs: Vec<f64> = cases.iter().map(|&c| number_value(v, c)).collect();
            let cut = spec.cut.unwrap_or_else(|| {
                let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (lo + hi) / 2.0
            });
            let shown = self.value(v, &Key::Number(cut));
            (
                xs.iter().map(|x| *x <= cut).collect(),
                format!("<= {shown}"),
            )
        } else {
            let keys: Vec<Key> = cases.iter().map(|&c| key_of(v, c)).collect();
            match &spec.success {
                Some(list) => (
                    keys.iter()
                        .map(|k| list.iter().any(|value| key_matches(k, value)))
                        .collect(),
                    format!("in ({})", self.values(v, list)),
                ),
                None => {
                    let found = first_found(&keys);
                    if found.len() != 2 {
                        self.warnings.push(format!(
                            "NPTESTS: {name} has {} values; the binomial test with the first value as success needs two",
                            found.len()
                        ));
                        return;
                    }
                    let shown = self.value(v, &found[0]);
                    (
                        keys.iter().map(|k| *k == found[0]).collect(),
                        format!("= {shown}"),
                    )
                }
            }
        };
        let n = self.total(cases);
        let n1: f64 = cases
            .iter()
            .zip(&success)
            .filter(|(_, s)| **s)
            .map(|(&c, _)| self.weights[c])
            .sum();
        if n <= 0.0 {
            self.warnings.push(format!(
                "NPTESTS: {name} has no cases for the binomial test"
            ));
            return;
        }
        let p0 = spec.test_value;
        let (p1, kind) = if n <= 25.0 {
            let lower = crate::npar_tests::binomial_cdf(n1, n, p0);
            let upper = 1.0 - crate::npar_tests::binomial_cdf(n1 - 1.0, n, p0);
            (lower.min(upper), "Exact")
        } else {
            let sd = (n * p0 * (1.0 - p0)).sqrt();
            let z1 = (n1 + 0.5 - n * p0) / sd;
            let z2 = (n1 - 0.5 - n * p0) / sd;
            (normal_cdf(z1).min(1.0 - normal_cdf(z2)), "Asymptotic")
        };
        let p2 = (2.0 * p1).min(1.0);
        let decide = if p0 == 0.5 { p2 } else { p1 };
        self.summarize(
            format!("The proportion of {name} {what} is {p0}."),
            "One-Sample Binomial Test",
            Sig::new(Some(decide), kind),
        );
        let (one, two) = if kind == "Exact" {
            ("Exact Sig. (1-sided test)", "Exact Sig. (2-sided test)")
        } else {
            (
                "Asymptotic Sig. (1-sided test)",
                "Asymptotic Sig. (2-sided test)",
            )
        };
        self.detail(
            "One-Sample Binomial Test",
            name.clone(),
            vec![
                ("Total N", Cell::Number(n)),
                ("Successes", Cell::Number(n1)),
                ("Observed Proportion", Cell::Number(n1 / n)),
                ("Test Proportion", Cell::Number(p0)),
                (one, Cell::Number(p1)),
                (two, Cell::Number(p2)),
            ],
        );
        let alpha = 1.0 - self.cilevel / 100.0;
        let parameter = format!("Probability of {name} {what}");
        let estimate = n1 / n;
        let t = n1;
        if spec.clopper_pearson {
            let lower = if t == 0.0 {
                0.0
            } else {
                beta_quantile(alpha / 2.0, t, n - t + 1.0)
            };
            let upper = if t == n {
                1.0
            } else {
                beta_quantile(1.0 - alpha / 2.0, t + 1.0, n - t)
            };
            self.interval(
                "One-Sample Binomial Success Rate (Clopper-Pearson)",
                parameter.clone(),
                estimate,
                [lower, upper],
            );
        }
        if spec.jeffreys {
            let lower = if t == 0.0 {
                0.0
            } else {
                beta_quantile(alpha / 2.0, t + 0.5, n - t + 0.5)
            };
            let upper = if t == n {
                1.0
            } else {
                beta_quantile(1.0 - alpha / 2.0, t + 0.5, n - t + 0.5)
            };
            self.interval(
                "One-Sample Binomial Success Rate (Jeffreys)",
                parameter.clone(),
                estimate,
                [lower, upper],
            );
        }
        if spec.likelihood {
            let chi = normal_quantile(1.0 - alpha / 2.0).powi(2);
            let log_lik = |p: f64| {
                let a = if t > 0.0 { t * p.ln() } else { 0.0 };
                let b = if n - t > 0.0 {
                    (n - t) * (1.0 - p).ln()
                } else {
                    0.0
                };
                a + b
            };
            let top = log_lik(estimate);
            let h = |p: f64| log_lik(p) - top + chi / 2.0;
            let lower = if t == 0.0 {
                0.0
            } else if t == n {
                (-chi / (2.0 * n)).exp()
            } else {
                bisect(h, 1e-300, estimate)
            };
            let upper = if t == n {
                1.0
            } else if t == 0.0 {
                1.0 - (-chi / (2.0 * n)).exp()
            } else {
                bisect(h, estimate, 1.0 - 1e-16)
            };
            self.interval(
                "One-Sample Binomial Success Rate (Likelihood Ratio)",
                parameter,
                estimate,
                [lower, upper],
            );
        }
    }

    fn ks(&mut self, v: &Variable, cases: &[usize], spec: &statscraft_syntax::NpKs) {
        let pairs: Vec<(f64, f64)> = cases
            .iter()
            .map(|&c| (number_value(v, c), self.weights[c]))
            .collect();
        let tests = [
            (KsDistribution::Normal, spec.normal.clone()),
            (KsDistribution::Uniform, spec.uniform.clone()),
            (KsDistribution::Exponential, spec.exponential.clone()),
            (
                KsDistribution::Poisson,
                spec.poisson.map(|m| NpParameters::Custom(vec![m])),
            ),
        ];
        for (distribution, parameters) in tests {
            if let Some(parameters) = parameters {
                self.ks_one(v, &pairs, distribution, &parameters, spec);
            }
        }
    }

    fn ks_one(
        &mut self,
        v: &Variable,
        pairs: &[(f64, f64)],
        distribution: KsDistribution,
        parameters: &NpParameters,
        spec: &statscraft_syntax::NpKs,
    ) {
        let name = self.name(v);
        let n: f64 = pairs.iter().map(|p| p.1).sum();
        let values = tally(pairs);
        let (family, labels): (&str, &[&str]) = match distribution {
            KsDistribution::Normal => ("Normal", &["Mean", "Std. Deviation"]),
            KsDistribution::Uniform => ("Uniform", &["Minimum", "Maximum"]),
            KsDistribution::Exponential => ("Exponential", &["Mean"]),
            KsDistribution::Poisson => ("Poisson", &["Mean"]),
        };
        let params = match parameters {
            NpParameters::Custom(given) => given.clone(),
            NpParameters::Sample { .. } => estimate(distribution, &values),
        };
        let problem = if n < 1.0 {
            Some("has no valid cases")
        } else {
            ks_problem(distribution, &params, &values)
        };
        if let Some(problem) = problem {
            self.warnings.push(format!(
                "NPTESTS: {name} {problem}; no {family} Kolmogorov-Smirnov test"
            ));
            return;
        }
        let [absolute, positive, negative] = ks_extremes(
            &values,
            n,
            |x| ks_cdf(distribution, &params, x),
            distribution == KsDistribution::Poisson,
        );
        let title = format!("One-Sample Kolmogorov-Smirnov Test ({family})");
        let mut rows: Vec<(&str, Cell)> = vec![("Total N", Cell::Number(n))];
        for (label, value) in labels.iter().zip(&params) {
            rows.push((label, Cell::Number(*value)));
        }
        rows.extend([
            ("Most Extreme Differences: Absolute", Cell::Number(absolute)),
            ("Most Extreme Differences: Positive", Cell::Number(positive)),
            ("Most Extreme Differences: Negative", Cell::Number(negative)),
            ("Test Statistic", Cell::Number(absolute)),
        ]);
        let simulate = match parameters {
            NpParameters::Sample { simulation } => {
                *simulation || distribution != KsDistribution::Normal
            }
            NpParameters::Custom(_) => false,
        };
        let size = n.round() as usize;
        let sig = if simulate && size.saturating_mul(spec.samples) <= 200_000_000 {
            let [p, lower, upper] = monte_carlo(
                self.simulate(distribution, size, spec.samples, absolute),
                spec.samples,
                spec.mc_cilevel,
            );
            rows.extend([
                ("Monte Carlo Sig. (2-sided test)", Cell::Number(p)),
                (
                    "Monte Carlo Confidence Level (%)",
                    Cell::Number(spec.mc_cilevel),
                ),
                ("Monte Carlo Sig. Lower Limit", Cell::Number(lower)),
                ("Monte Carlo Sig. Upper Limit", Cell::Number(upper)),
            ]);
            Sig::new(Some(p), "Monte Carlo")
        } else if matches!(parameters, NpParameters::Sample { .. })
            && distribution == KsDistribution::Normal
        {
            if simulate {
                self.warnings.push(format!(
                    "NPTESTS: {name} has too many cases to simulate; the Lilliefors table gives the significance"
                ));
            }
            let cell = lilliefors(absolute, n);
            rows.push(("Lilliefors Sig. (2-sided test)", cell.clone()));
            let p = match &cell {
                Cell::Number(p) => Some(*p),
                // above the table's range: retained at any alpha below 0.2
                Cell::Text(_) => Some(1.0),
                _ => None,
            };
            Sig {
                p,
                shown: cell,
                kind: "Lilliefors",
                inclusive: false,
            }
        } else {
            if simulate {
                self.warnings.push(format!(
                    "NPTESTS: {name} has too many cases to simulate; Smirnov's significance is shown"
                ));
            }
            let p = smirnov_p(n.sqrt() * absolute);
            rows.push(("Asymptotic Sig. (2-sided test)", Cell::Number(p)));
            Sig::new(Some(p), "Asymptotic")
        };
        let shown: Vec<String> = params.iter().map(|p| short(*p)).collect();
        let null = format!(
            "{name} follows the {family} distribution ({}).",
            labels
                .iter()
                .zip(&shown)
                .map(|(l, v)| format!("{} {v}", l.to_lowercase()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.summarize(null, "One-Sample Kolmogorov-Smirnov Test", sig);
        self.detail(&title, name, rows);
    }

    /// The number of `samples` simulated samples of `size` from the standard member of the
    /// family whose statistic, with the parameters estimated again, reaches `observed`.
    fn simulate(
        &self,
        distribution: KsDistribution,
        size: usize,
        samples: usize,
        observed: f64,
    ) -> usize {
        let mut random = Random::default();
        random.seed = self.seed;
        let mut count = 0;
        let mut draws = vec![0.0; size];
        for _ in 0..samples {
            for d in draws.iter_mut() {
                *d = match distribution {
                    KsDistribution::Normal => random.normal(),
                    KsDistribution::Exponential => -(1.0 - random.uniform()).max(1e-300).ln(),
                    _ => random.uniform(),
                };
            }
            draws.sort_by(f64::total_cmp);
            let values = tally(&draws.iter().map(|d| (*d, 1.0)).collect::<Vec<_>>());
            let params = estimate(distribution, &values);
            if ks_problem(distribution, &params, &values).is_some() {
                continue;
            }
            let [d, _, _] = ks_extremes(
                &values,
                size as f64,
                |x| ks_cdf(distribution, &params, x),
                false,
            );
            if d >= observed - 1e-12 {
                count += 1;
            }
        }
        count
    }

    fn runs(
        &mut self,
        v: &Variable,
        cases: &[usize],
        level: Level,
        spec: &statscraft_syntax::NpRuns,
    ) {
        let name = self.name(v);
        let (first, cut): (Vec<bool>, Option<f64>) = if level == Level::Scale {
            let pairs: Vec<(f64, f64)> = cases
                .iter()
                .map(|&c| (number_value(v, c), self.weights[c]))
                .collect();
            let n: f64 = pairs.iter().map(|p| p.1).sum();
            let cut = match spec.cut {
                NpCut::Value(x) => Some(x),
                NpCut::Mean => (n > 0.0).then(|| pairs.iter().map(|(x, w)| x * w).sum::<f64>() / n),
                NpCut::Median => middle(&pairs),
            };
            let Some(cut) = cut else {
                self.warnings
                    .push(format!("NPTESTS: {name} has no cases for the runs test"));
                return;
            };
            (pairs.iter().map(|p| p.0 <= cut).collect(), Some(cut))
        } else {
            let keys: Vec<Key> = cases.iter().map(|&c| key_of(v, c)).collect();
            match &spec.group {
                Some(list) => (
                    keys.iter()
                        .map(|k| list.iter().any(|value| key_matches(k, value)))
                        .collect(),
                    None,
                ),
                None => {
                    let found = first_found(&keys);
                    if found.len() != 2 {
                        self.warnings.push(format!(
                            "NPTESTS: {name} has {} values; the runs test with the values found needs two",
                            found.len()
                        ));
                        return;
                    }
                    (keys.iter().map(|k| *k == found[0]).collect(), None)
                }
            }
        };
        let (mut n1, mut n2, mut count, mut last) = (0.0, 0.0, 0.0, None);
        for (&c, side) in cases.iter().zip(&first) {
            if *side {
                n1 += self.weights[c];
            } else {
                n2 += self.weights[c];
            }
            if last != Some(*side) {
                count += 1.0;
                last = Some(*side);
            }
        }
        if n1 == 0.0 || n2 == 0.0 {
            self.warnings.push(format!(
                "NPTESTS: {name} needs cases in both groups for the runs test"
            ));
            return;
        }
        let n = n1 + n2;
        let sd = (2.0 * n1 * n2 * (2.0 * n1 * n2 - n) / (n * n * (n - 1.0))).sqrt();
        let z = runs_z(count, n1, n2);
        let p = z.map(normal_p);
        self.summarize(
            format!("The order of the values of {name} is random."),
            "One-Sample Runs Test",
            Sig::new(p, "Asymptotic"),
        );
        let mut rows = vec![("Total N", Cell::Number(n))];
        if let Some(cut) = cut {
            rows.push(("Test Value", Cell::Number(cut)));
        }
        rows.extend([
            ("Number of Runs", Cell::Number(count)),
            ("Standard Error", Cell::Number(sd)),
            (
                "Standardized Test Statistic",
                crate::descriptives::number(z),
            ),
            (
                "Asymptotic Sig. (2-sided test)",
                crate::descriptives::number(p),
            ),
        ]);
        self.detail("One-Sample Runs Test", name, rows);
    }

    fn wilcoxon_one(&mut self, v: &Variable, cases: &[usize], theta: f64) {
        let name = self.name(v);
        let differences: Vec<(f64, f64)> = cases
            .iter()
            .map(|&c| (number_value(v, c) - theta, self.weights[c]))
            .collect();
        let signed = SignedRanks::new(&differences);
        let n = signed.np + signed.nn;
        let sd = signed.variance().sqrt();
        let z = (sd > 0.0).then(|| (signed.sp - n * (n + 1.0) / 4.0) / sd);
        let p = z.map(normal_p);
        self.summarize(
            format!("The median of {name} is {theta}."),
            "One-Sample Wilcoxon Signed Rank Test",
            Sig::new(p, "Asymptotic"),
        );
        self.detail(
            "One-Sample Wilcoxon Signed Rank Test",
            name,
            vec![
                ("Total N", Cell::Number(n + signed.ties)),
                ("Test Statistic", Cell::Number(signed.sp)),
                ("Standard Error", Cell::Number(sd)),
                (
                    "Standardized Test Statistic",
                    crate::descriptives::number(z),
                ),
                (
                    "Asymptotic Sig. (2-sided test)",
                    crate::descriptives::number(p),
                ),
            ],
        );
    }

    // ---------------------------------------------------------------- independent samples

    fn independent(&mut self, spec: &NpIndependent, names: &[String]) -> Result<(), Error> {
        let data = self.data;
        let group = data.variable(&spec.group)?;
        if self.level(&group) == Level::Scale {
            return Err(Error(format!(
                "NPTESTS: the grouping field {} must be nominal or ordinal, not scale",
                group.name()
            )));
        }
        for name in names {
            let v = data.variable(name)?;
            if v.name().eq_ignore_ascii_case(group.name()) {
                continue;
            }
            if self.level(&v) == Level::Nominal || v.is_string() {
                self.skip(
                    "INDEPENDENT",
                    name,
                    "not a numeric continuous or ordinal field",
                );
                continue;
            }
            let cases = self.cases(&[&v, &group]);
            let keys = sorted(first_found(
                &cases.iter().map(|&c| key_of(&group, c)).collect::<Vec<_>>(),
            ));
            let groups: Vec<Vec<(f64, f64)>> = keys
                .iter()
                .map(|k| {
                    cases
                        .iter()
                        .filter(|&&c| key_of(&group, c) == *k)
                        .map(|&c| (number_value(&v, c), self.weights[c]))
                        .collect()
                })
                .collect();
            let grouped = Grouped {
                keys: (0..keys.len()).map(|i| i as f64).collect(),
                groups,
            };
            let labels: Vec<String> = keys.iter().map(|k| self.value(&group, k)).collect();
            let sample = Sample {
                field: self.name(&v),
                group: self.name(&group),
                labels,
                grouped,
            };
            if keys.len() < 2 {
                self.warnings.push(format!(
                    "NPTESTS: {name} has fewer than two groups of {}; no test",
                    group.name()
                ));
                continue;
            }
            let two = keys.len() == 2;
            let two_only = |np: &mut Self, test: &str| {
                if !two {
                    np.skip(test, name, "it needs a grouping field with two groups only");
                }
                two
            };
            if spec.automatic() {
                if two {
                    self.mann_whitney(&sample);
                } else {
                    self.kruskal_wallis(&sample, NpCompare::Pairwise);
                }
                continue;
            }
            if spec.mann_whitney && two_only(self, "MANN_WHITNEY") {
                self.mann_whitney(&sample);
            }
            if spec.ks && two_only(self, "KOLMOGOROV_SMIRNOV") {
                self.smirnov(&sample);
            }
            if spec.wald_wolfowitz && two_only(self, "WALD_WOLFOWITZ") {
                self.wald_wolfowitz(&sample);
            }
            if let Some(trim) = spec.moses
                && two_only(self, "MOSES")
            {
                self.moses(&sample, trim);
            }
            if spec.hodges_lehmann && two_only(self, "HODGES_LEHMAN") {
                self.hodges_lehmann_independent(&sample);
            }
            if let Some(how) = spec.kruskal_wallis {
                self.kruskal_wallis(&sample, how);
            }
            if let Some((value, how)) = spec.median {
                self.median(&sample, value, how);
            }
            if let Some((descending, how)) = spec.jonckheere {
                self.jonckheere(&sample, descending, how);
            }
        }
        Ok(())
    }

    fn mann_whitney(&mut self, s: &Sample) {
        let mw = s.grouped.mann_whitney();
        let sd = mw.variance.sqrt();
        let mean = mw.n[0] * mw.n[1] / 2.0;
        let z = (sd > 0.0).then(|| (mw.u1 - mean) / sd);
        let asymptotic = z.map(normal_p);
        let sig = match mw.exact {
            Some(p) => Sig::new(Some(p), "Exact"),
            None => Sig::new(asymptotic, "Asymptotic"),
        };
        self.summarize(
            s.same("distribution"),
            "Independent-Samples Mann-Whitney U Test",
            sig,
        );
        let mut rows = vec![
            ("Total N", Cell::Number(mw.n[0] + mw.n[1])),
            ("Mann-Whitney U", Cell::Number(mw.u1)),
            ("Wilcoxon W", Cell::Number(mw.s[1])),
            ("Test Statistic", Cell::Number(mw.u1)),
            ("Standard Error", Cell::Number(sd)),
            (
                "Standardized Test Statistic",
                crate::descriptives::number(z),
            ),
            (
                "Asymptotic Sig. (2-sided test)",
                crate::descriptives::number(asymptotic),
            ),
        ];
        if let Some(p) = mw.exact {
            rows.push(("Exact Sig. (2-sided test)", Cell::Number(p)));
        }
        self.detail("Independent-Samples Mann-Whitney U Test", s.title(), rows);
    }

    fn smirnov(&mut self, s: &Sample) {
        let n = s.grouped.sizes();
        let [absolute, positive, negative, z, p] = s.grouped.smirnov();
        self.summarize(
            s.same("distribution"),
            "Independent-Samples Kolmogorov-Smirnov Test",
            Sig::new(Some(p), "Asymptotic"),
        );
        self.detail(
            "Independent-Samples Kolmogorov-Smirnov Test",
            s.title(),
            vec![
                ("Total N", Cell::Number(n[0] + n[1])),
                ("Most Extreme Differences: Absolute", Cell::Number(absolute)),
                ("Most Extreme Differences: Positive", Cell::Number(positive)),
                ("Most Extreme Differences: Negative", Cell::Number(negative)),
                ("Test Statistic", Cell::Number(z)),
                ("Asymptotic Sig. (2-sided test)", Cell::Number(p)),
            ],
        );
    }

    fn wald_wolfowitz(&mut self, s: &Sample) {
        let n = s.grouped.sizes();
        let (n1, n2) = (n[0], n[1]);
        let total = n1 + n2;
        let (fewest, most) = run_bounds(&s.grouped.blocks());
        let mean = 2.0 * n1 * n2 / total + 1.0;
        let sd = (2.0 * n1 * n2 * (2.0 * n1 * n2 - total) / (total * total * (total - 1.0))).sqrt();
        // below 50 cases a half run toward the lower tail, which the one-sided test looks at
        let z = |r: f64| {
            let d = r - mean;
            if total >= 50.0 {
                d / sd
            } else if d.abs() >= 0.5 {
                (d + 0.5) / sd
            } else {
                0.0
            }
        };
        let exact = total <= 30.0;
        let p = |r: f64| {
            if exact {
                runs_exact(r, n1, n2)[0]
            } else {
                normal_cdf(z(r))
            }
        };
        let kind = if exact { "Exact" } else { "Asymptotic" };
        self.summarize(
            s.same("distribution"),
            "Independent-Samples Wald-Wolfowitz Runs Test",
            Sig::new(Some(p(most)), kind),
        );
        let label = if exact {
            "Exact Sig. (1-sided test)"
        } else {
            "Asymptotic Sig. (1-sided test)"
        };
        let mut rows = vec![("Total N", Cell::Number(total))];
        if fewest == most {
            rows.extend([
                ("Number of Runs", Cell::Number(most)),
                ("Standardized Test Statistic", Cell::Number(z(most))),
                (label, Cell::Number(p(most))),
            ]);
        } else {
            let (low_label, high_label) = if exact {
                (
                    "Exact Sig. (1-sided test), Fewest Runs",
                    "Exact Sig. (1-sided test), Most Runs",
                )
            } else {
                (
                    "Asymptotic Sig. (1-sided test), Fewest Runs",
                    "Asymptotic Sig. (1-sided test), Most Runs",
                )
            };
            rows.extend([
                ("Fewest Possible Runs", Cell::Number(fewest)),
                ("Most Possible Runs", Cell::Number(most)),
                (
                    "Standardized Test Statistic, Fewest Runs",
                    Cell::Number(z(fewest)),
                ),
                (
                    "Standardized Test Statistic, Most Runs",
                    Cell::Number(z(most)),
                ),
                (low_label, Cell::Number(p(fewest))),
                (high_label, Cell::Number(p(most))),
            ]);
        }
        self.detail(
            "Independent-Samples Wald-Wolfowitz Runs Test",
            s.title(),
            rows,
        );
    }

    fn moses(&mut self, s: &Sample, trim: Option<f64>) {
        let n = s.grouped.sizes();
        let (nc, ne) = (n[0], n[1]);
        let control = s.grouped.control_ranks();
        let span_of =
            |ranks: &[f64]| -> Option<f64> { Some((ranks.last()? - ranks.first()? + 1.0).round()) };
        let Some(span) = span_of(&control) else {
            return;
        };
        let p = moses_p(span, nc, ne, 0.0);
        let mut h = trim.unwrap_or_else(|| (0.05 * nc).floor().max(1.0));
        if 2.0 * h > nc - 2.0 {
            h = ((nc - 2.0) / 2.0).floor().max(0.0);
        }
        let kept = h as usize;
        let trimmed = span_of(&control[kept..control.len() - kept]);
        let trimmed_p = trimmed.map(|t| moses_p(t, nc, ne, h));
        let decide = if h > 0.0 { trimmed_p } else { Some(p) };
        self.summarize(
            s.same("range"),
            "Independent-Samples Moses Test of Extreme Reaction",
            Sig {
                inclusive: true,
                ..Sig::new(decide, "Exact")
            },
        );
        self.detail(
            "Independent-Samples Moses Test of Extreme Reaction",
            s.title(),
            vec![
                ("Total N", Cell::Number(nc + ne)),
                ("Observed Control Group Span", Cell::Number(span)),
                ("Exact Sig. (1-sided test), Observed", Cell::Number(p)),
                (
                    "Trimmed Control Group Span",
                    crate::descriptives::number(trimmed),
                ),
                (
                    "Exact Sig. (1-sided test), Trimmed",
                    crate::descriptives::number(trimmed_p),
                ),
                ("Records Trimmed from Each End", Cell::Number(h)),
            ],
        );
    }

    fn hodges_lehmann_independent(&mut self, s: &Sample) {
        let a = tally(&s.grouped.groups[0]);
        let b = tally(&s.grouped.groups[1]);
        if a.len().saturating_mul(b.len()) > MAX_PAIRS {
            self.too_many(&s.field);
            return;
        }
        let differences: Vec<(f64, f64)> = a
            .iter()
            .flat_map(|(x, f)| b.iter().map(move |(y, g)| (x - y, f * g)))
            .collect();
        let sorted = weighted_sorted(differences);
        let mw = s.grouped.mann_whitney();
        let mean = mw.n[0] * mw.n[1] / 2.0;
        self.hodges_lehmann_interval(
            &sorted,
            mean,
            mw.variance.sqrt(),
            "Independent-Samples Hodges-Lehmann Median Difference",
            format!(
                "Difference between the medians of {} for {} and {}",
                s.field, s.labels[0], s.labels[1]
            ),
        );
    }

    fn too_many(&mut self, name: &str) {
        self.warnings.push(format!(
            "NPTESTS: {name} has too many distinct values for the Hodges-Lehmann estimate"
        ));
    }

    /// The weighted median of `sorted` with the limits at the ranks mean -+ z sd.
    fn hodges_lehmann_interval(
        &mut self,
        sorted: &[(f64, f64)],
        mean: f64,
        sd: f64,
        kind: &str,
        parameter: String,
    ) {
        if sorted.is_empty() {
            return;
        }
        let total: f64 = sorted.iter().map(|v| v.1).sum();
        let z = normal_quantile(1.0 - (1.0 - self.cilevel / 100.0) / 2.0);
        let i1 = (1.0 + (mean - z * sd).floor()).clamp(1.0, total);
        let i2 = (mean + z * sd).ceil().clamp(1.0, total);
        self.interval(
            kind,
            parameter,
            weighted_median(sorted),
            [at_position(sorted, i1), at_position(sorted, i2)],
        );
    }

    fn kruskal_wallis(&mut self, s: &Sample, how: NpCompare) {
        let n = s.grouped.sizes();
        let total: f64 = n.iter().sum();
        let (sums, h) = s.grouped.kruskal_wallis();
        let df = s.labels.len() as f64 - 1.0;
        let p = h.map(|h| chi_square_p(h, df));
        self.summarize(
            s.same("distribution"),
            "Independent-Samples Kruskal-Wallis Test",
            Sig::new(p, "Asymptotic"),
        );
        self.detail(
            "Independent-Samples Kruskal-Wallis Test",
            s.title(),
            vec![
                ("Total N", Cell::Number(total)),
                ("Test Statistic", crate::descriptives::number(h)),
                ("Degrees of Freedom", Cell::Number(df)),
                ("Asymptotic Sig.", crate::descriptives::number(p)),
            ],
        );
        if !self.compare_now(p, how, s.labels.len()) {
            return;
        }
        let means: Vec<f64> = sums.iter().zip(&n).map(|(s, n)| s / n).collect();
        match how {
            NpCompare::Pairwise => {
                let (_, ties) = s.grouped.ranked();
                let a = total * (total + 1.0) / 12.0 - ties / (12.0 * (total - 1.0));
                let rows = pairs(s.labels.len())
                    .map(|(j, k)| {
                        let se = (a * (1.0 / n[j] + 1.0 / n[k])).sqrt();
                        Pair::z(j, k, means[j] - means[k], se)
                    })
                    .collect();
                self.pairwise(&s.title(), &s.labels, rows, "Kruskal-Wallis");
            }
            _ => {
                let medians: Vec<f64> = s
                    .grouped
                    .groups
                    .iter()
                    .map(|g| middle(g).unwrap_or(f64::NAN))
                    .collect();
                let mut order: Vec<usize> = (0..s.labels.len()).collect();
                order.sort_by(|&a, &b| {
                    means[a]
                        .total_cmp(&means[b])
                        .then(medians[a].total_cmp(&medians[b]))
                });
                let test = |members: &[usize]| {
                    let sub = s.subset(members);
                    let (_, h) = sub.kruskal_wallis();
                    h.map(|h| (h, chi_square_p(h, members.len() as f64 - 1.0)))
                };
                self.stepwise(&s.title(), &s.labels, &order, &means, "Mean Rank", test);
            }
        }
    }

    fn median(&mut self, s: &Sample, given: Option<f64>, how: NpCompare) {
        let n = s.grouped.sizes();
        let total: f64 = n.iter().sum();
        let pooled: Vec<(f64, f64)> = s.grouped.pooled().iter().map(|p| (p.0, p.1)).collect();
        let md = given.or_else(|| middle(&pooled));
        let k = s.labels.len();
        let test = s.grouped.median_test(md, k == 2);
        let df = test.groups - 1.0;
        let (p, kind) = match (test.chi, test.exact) {
            (_, Some(p)) => (Some(p), "Exact"),
            (Some(chi), None) => (Some(chi_square_p(chi, df)), "Asymptotic"),
            _ => (None, "Asymptotic"),
        };
        if p.is_none() {
            self.warnings.push(format!(
                "NPTESTS: {} needs two groups with cases on both sides of the median; no median test",
                s.field
            ));
        }
        self.summarize(
            s.same("median"),
            "Independent-Samples Median Test",
            Sig::new(p, kind),
        );
        let mut rows = vec![
            ("Total N", Cell::Number(total)),
            ("Median", crate::descriptives::number(md)),
            ("Test Statistic", crate::descriptives::number(test.chi)),
            ("Degrees of Freedom", Cell::Number(df)),
        ];
        match test.exact {
            Some(p) => rows.push(("Exact Sig. (2-sided test)", Cell::Number(p))),
            None => rows.push((
                "Asymptotic Sig.",
                crate::descriptives::number(p.filter(|_| test.chi.is_some())),
            )),
        }
        self.detail("Independent-Samples Median Test", s.title(), rows);
        if !self.compare_now(p, how, k) {
            return;
        }
        // the median test of two samples with their own median
        let pair_test = |members: &[usize]| {
            let sub = s.subset(members);
            let pooled: Vec<(f64, f64)> = sub.pooled().iter().map(|p| (p.0, p.1)).collect();
            let test = sub.median_test(middle(&pooled), members.len() == 2);
            let df = members.len() as f64 - 1.0;
            match (test.chi, test.exact) {
                (chi, Some(p)) => Some((chi.unwrap_or(f64::NAN), p)),
                (Some(chi), None) => Some((chi, chi_square_p(chi, df))),
                _ => None,
            }
        };
        match how {
            NpCompare::Pairwise => {
                let rows = pairs(k)
                    .map(|(j, l)| match pair_test(&[j, l]) {
                        Some((chi, p)) => Pair {
                            j,
                            k: l,
                            statistic: chi.is_finite().then_some(chi),
                            se: None,
                            z: None,
                            p: Some(p),
                        },
                        None => Pair {
                            j,
                            k: l,
                            statistic: None,
                            se: None,
                            z: None,
                            p: None,
                        },
                    })
                    .collect();
                self.pairwise(&s.title(), &s.labels, rows, "median");
            }
            _ => {
                let medians: Vec<f64> = s
                    .grouped
                    .groups
                    .iter()
                    .map(|g| middle(g).unwrap_or(f64::NAN))
                    .collect();
                let mut order: Vec<usize> = (0..k).collect();
                order.sort_by(|&a, &b| medians[a].total_cmp(&medians[b]));
                self.stepwise(&s.title(), &s.labels, &order, &medians, "Median", pair_test);
            }
        }
    }

    fn jonckheere(&mut self, s: &Sample, descending: bool, how: NpCompare) {
        let n = s.grouped.sizes();
        let total: f64 = n.iter().sum();
        let [t, mean, sd] = s.grouped.jonckheere();
        let z = (sd > 0.0).then(|| (t - mean) / sd);
        let two = z.map(normal_p);
        // ascending locations make the statistic large
        let one_sided = |z: f64| {
            if descending {
                normal_cdf(z)
            } else {
                1.0 - normal_cdf(z)
            }
        };
        let one = z.map(one_sided);
        let order = if descending {
            "decreasing"
        } else {
            "increasing"
        };
        self.summarize(
            format!(
                "The distribution of {} is the same across the categories of {} (alternative: {order} with the category order).",
                s.field, s.group
            ),
            "Independent-Samples Jonckheere-Terpstra Test for Ordered Alternatives",
            Sig::new(one, "Asymptotic"),
        );
        self.detail(
            "Independent-Samples Jonckheere-Terpstra Test",
            s.title(),
            vec![
                ("Total N", Cell::Number(total)),
                ("Test Statistic", Cell::Number(t)),
                ("Standard Error", Cell::Number(sd)),
                (
                    "Standardized Test Statistic",
                    crate::descriptives::number(z),
                ),
                (
                    "Asymptotic Sig. (1-sided test)",
                    crate::descriptives::number(one),
                ),
                (
                    "Asymptotic Sig. (2-sided test)",
                    crate::descriptives::number(two),
                ),
            ],
        );
        let k = s.labels.len();
        if !self.compare_now(one, how, k) {
            return;
        }
        let (sums, _) = s.grouped.kruskal_wallis();
        let means: Vec<f64> = sums.iter().zip(&n).map(|(s, n)| s / n).collect();
        match how {
            NpCompare::Pairwise => {
                let rows = pairs(k)
                    .map(|(j, l)| {
                        let mw = s.subset(&[j, l]).mann_whitney();
                        let se = mw.variance.sqrt();
                        let mean = mw.n[0] * mw.n[1] / 2.0;
                        let z = (se > 0.0).then(|| (mw.u1 - mean) / se);
                        Pair {
                            j,
                            k: l,
                            statistic: Some(mw.u1),
                            se: Some(se),
                            z,
                            p: z.map(one_sided),
                        }
                    })
                    .collect();
                self.pairwise(
                    &s.title(),
                    &s.labels,
                    rows,
                    "Jonckheere-Terpstra (one-sided)",
                );
            }
            _ => {
                let mut order: Vec<usize> = (0..k).collect();
                if descending {
                    order.reverse();
                }
                let test = |members: &[usize]| {
                    // the members in the sorted order, tested for increasing locations
                    let sub = s.subset(members);
                    let [t, mean, sd] = sub.jonckheere();
                    (sd > 0.0).then(|| {
                        let z = (t - mean) / sd;
                        (t, 1.0 - normal_cdf(z))
                    })
                };
                self.stepwise(&s.title(), &s.labels, &order, &means, "Mean Rank", test);
            }
        }
    }

    // ---------------------------------------------------------------- multiple comparisons

    fn compare_now(&mut self, p: Option<f64>, how: NpCompare, k: usize) -> bool {
        how != NpCompare::None && k >= 3 && p.is_some_and(|p| p < self.alpha)
    }

    fn pairwise(&mut self, title: &str, labels: &[String], rows: Vec<Pair>, test: &str) {
        let k = labels.len() as f64;
        let factor = k * (k - 1.0) / 2.0;
        let rows = rows
            .into_iter()
            .map(|r| {
                vec![
                    text(format!("{}-{}", labels[r.j], labels[r.k])),
                    crate::descriptives::number(r.statistic),
                    crate::descriptives::number(r.se),
                    crate::descriptives::number(r.z),
                    crate::descriptives::number(r.p),
                    crate::descriptives::number(r.p.map(|p| (p * factor).min(1.0))),
                ]
            })
            .collect();
        let columns = [
            "Sample 1-Sample 2",
            "Test Statistic",
            "Std. Error",
            "Std. Test Statistic",
            "Sig.",
            "Adj. Sig.",
        ];
        let mut table = crate::npar_tests::table(
            format!("Pairwise Comparisons of {title}"),
            1,
            columns.map(String::from).to_vec(),
            rows,
        );
        table.caption = Some(format!(
            "{test}: each row tests that the two samples are the same. Adj. Sig. multiplies Sig. by the {factor} comparisons (Bonferroni), at most 1."
        ));
        self.comparisons.push(table);
    }

    /// Homogeneous subsets of the samples sorted in `order`: from each start, samples join
    /// while `test` of the start to the new sample keeps the null hypothesis.
    fn stepwise(
        &mut self,
        title: &str,
        labels: &[String],
        order: &[usize],
        criterion: &[f64],
        criterion_name: &str,
        test: impl Fn(&[usize]) -> Option<(f64, f64)>,
    ) {
        let mut subsets: Vec<Vec<usize>> = Vec::new();
        let mut start = 0;
        while start < order.len() {
            let mut end = start + 1;
            while end < order.len() {
                let rejected = test(&order[start..=end]).is_some_and(|(_, p)| p < self.alpha);
                if rejected {
                    break;
                }
                end += 1;
            }
            subsets.push(order[start..end].to_vec());
            start = end;
        }
        let mut columns = vec!["Sample".to_string()];
        columns.extend((1..=subsets.len()).map(|i| format!("Subset {i}")));
        let mut rows: Vec<Vec<Cell>> = order
            .iter()
            .map(|&i| {
                let mut row = vec![text(labels[i].clone())];
                row.extend(subsets.iter().map(|s| {
                    if s.contains(&i) {
                        Cell::Number(criterion[i])
                    } else {
                        Cell::Empty
                    }
                }));
                row
            })
            .collect();
        let results: Vec<Option<(f64, f64)>> = subsets
            .iter()
            .map(|s| if s.len() > 1 { test(s) } else { None })
            .collect();
        let mut statistic = vec![text("Test Statistic")];
        let mut sig = vec![text("Sig.")];
        for r in &results {
            statistic.push(crate::descriptives::number(r.map(|r| r.0)));
            sig.push(crate::descriptives::number(r.map(|r| r.1)));
        }
        rows.push(statistic);
        rows.push(sig);
        let mut table =
            crate::npar_tests::table(format!("Homogeneous Subsets of {title}"), 1, columns, rows);
        table.caption = Some(format!(
            "The cells show the {}; a subset holds samples the step-down tests do not separate.",
            criterion_name.to_lowercase()
        ));
        self.comparisons.push(table);
    }

    // ---------------------------------------------------------------- related samples

    fn related(&mut self, spec: &NpRelated, names: &[String]) -> Result<(), Error> {
        let data = self.data;
        let mut by_level: [Vec<Variable<'d>>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for name in names {
            let v = data.variable(name)?;
            let slot = match self.level(&v) {
                Level::Nominal => 0,
                Level::Ordinal => 1,
                Level::Scale => 2,
            };
            by_level[slot].push(v);
        }
        let [nominal, ordinal, scale] = by_level;
        let numeric = |list: &[Variable<'d>]| list.iter().all(|v| !v.is_string());
        if spec.automatic() {
            match scale.len() {
                0 => {}
                1 => self.warnings.push(format!(
                    "NPTESTS: {} is the only continuous field; no related-samples test",
                    scale[0].name()
                )),
                2 => self.paired(&scale, Paired::Wilcoxon),
                _ => self.k_related(&scale, Ranked::Friedman, NpCompare::Pairwise),
            }
            for set in [&nominal, &ordinal] {
                if set.len() < 2 {
                    if let Some(v) = set.first() {
                        self.warnings.push(format!(
                            "NPTESTS: {} is the only field of its level; no related-samples test",
                            v.name()
                        ));
                    }
                    continue;
                }
                let refs: Vec<&Variable> = set.iter().collect();
                let cases = self.cases(&refs);
                let keys: Vec<Key> = cases
                    .iter()
                    .flat_map(|&c| set.iter().map(move |v| key_of(v, c)))
                    .collect();
                if first_found(&keys).len() != 2 {
                    self.warnings.push(format!(
                        "NPTESTS: {} do not have two values together; no related-samples test",
                        set.iter().map(|v| v.name()).collect::<Vec<_>>().join(", ")
                    ));
                } else if set.len() == 2 {
                    self.mcnemar(set, None);
                } else {
                    self.cochran(set, None, NpCompare::Pairwise);
                }
            }
            return Ok(());
        }
        let mut both: Vec<Variable<'d>> = Vec::new();
        for name in names {
            if let Some(v) = ordinal.iter().chain(&scale).find(|v| v.name() == name) {
                both.push(*v);
            }
        }
        let two = |np: &mut Self, test: &str, list: &[Variable<'d>]| {
            let ok = list.len() == 2 && numeric(list);
            if !ok {
                np.warnings.push(format!(
                    "NPTESTS: {test} needs exactly two numeric continuous or ordinal fields, got {}",
                    list.len()
                ));
            }
            ok
        };
        if let Some(success) = &spec.mcnemar {
            for set in [&nominal, &ordinal] {
                if set.is_empty() {
                    continue;
                }
                if set.len() != 2 {
                    self.warnings.push(format!(
                        "NPTESTS: MCNEMAR needs two fields of one level, got {}",
                        set.len()
                    ));
                    continue;
                }
                self.mcnemar(set, success.as_deref());
            }
        }
        if spec.wilcoxon && two(self, "WILCOXON", &both) {
            self.paired(&both, Paired::Wilcoxon);
        }
        if spec.sign && two(self, "SIGN", &both) {
            self.paired(&both, Paired::Sign);
        }
        if spec.marginal_homogeneity && two(self, "MARGINAL_HOMOGENEITY", &ordinal) {
            self.paired(&ordinal, Paired::Marginal);
        }
        if spec.hodges_lehmann && two(self, "HODGES_LEHMAN", &both) {
            self.paired(&both, Paired::HodgesLehmann);
        }
        if let Some((success, how)) = &spec.cochran {
            for set in [&nominal, &ordinal] {
                if set.len() >= 2 {
                    self.cochran(set, success.as_deref(), *how);
                } else if let Some(v) = set.first() {
                    self.warnings.push(format!(
                        "NPTESTS: {} is the only field of its level; no Cochran test",
                        v.name()
                    ));
                }
            }
        }
        for (wanted, test) in [
            (spec.friedman, Ranked::Friedman),
            (spec.kendall, Ranked::Kendall),
        ] {
            if let Some(how) = wanted {
                if both.len() >= 2 && numeric(&both) {
                    self.k_related(&both, test, how);
                } else {
                    self.warnings.push(format!(
                        "NPTESTS: {} needs two or more numeric continuous or ordinal fields",
                        test.keyword()
                    ));
                }
            }
        }
        Ok(())
    }

    fn joined(&self, fields: &[Variable]) -> String {
        fields
            .iter()
            .map(|v| self.name(v))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Whether each value of every case (rows of the fields) is a success: the values in
    /// `success`, else the first value found when the fields have two values together.
    fn successes(
        &mut self,
        fields: &[Variable],
        cases: &[usize],
        success: Option<&[Value]>,
        test: &str,
    ) -> Option<(Vec<Vec<bool>>, String)> {
        let keys: Vec<Vec<Key>> = cases
            .iter()
            .map(|&c| fields.iter().map(|v| key_of(v, c)).collect())
            .collect();
        match success {
            Some(list) => Some((
                keys.iter()
                    .map(|row| {
                        row.iter()
                            .map(|k| list.iter().any(|value| key_matches(k, value)))
                            .collect()
                    })
                    .collect(),
                format!("in ({})", self.values(&fields[0], list)),
            )),
            None => {
                let found = first_found(&keys.concat());
                if found.len() != 2 {
                    self.warnings.push(format!(
                        "NPTESTS: {} have {} values together; {test} with the first value as success needs two",
                        self.joined(fields),
                        found.len()
                    ));
                    return None;
                }
                let shown = self.value(&fields[0], &found[0]);
                Some((
                    keys.iter()
                        .map(|row| row.iter().map(|k| *k == found[0]).collect())
                        .collect(),
                    format!("= {shown}"),
                ))
            }
        }
    }

    fn mcnemar(&mut self, fields: &[Variable], success: Option<&[Value]>) {
        let refs: Vec<&Variable> = fields.iter().collect();
        let cases = self.cases(&refs);
        let Some((hits, what)) = self.successes(fields, &cases, success, "MCNEMAR") else {
            return;
        };
        let (mut n1, mut n2) = (0.0, 0.0);
        for (&c, row) in cases.iter().zip(&hits) {
            match (row[0], row[1]) {
                (true, false) => n1 += self.weights[c],
                (false, true) => n2 += self.weights[c],
                _ => {}
            }
        }
        let changes = n1 + n2;
        let total = self.total(&cases);
        let (chi, p, kind) = if changes <= 25.0 {
            (None, sign_exact(n1, n2)[0], "Exact")
        } else {
            let chi = ((n1 - n2).abs() - 1.0).powi(2) / changes;
            (Some(chi), chi_square_p(chi, 1.0), "Asymptotic")
        };
        let joined = self.joined(fields);
        self.summarize(
            format!("{joined} have the same proportion of values {what}."),
            "Related-Samples McNemar Change Test",
            Sig::new(Some(p), kind),
        );
        let mut rows = vec![
            ("Total N", Cell::Number(total)),
            ("Success Then Failure", Cell::Number(n1)),
            ("Failure Then Success", Cell::Number(n2)),
            ("Test Statistic", crate::descriptives::number(chi)),
        ];
        if chi.is_some() {
            rows.push(("Degrees of Freedom", Cell::Number(1.0)));
            rows.push(("Asymptotic Sig.", Cell::Number(p)));
        } else {
            rows.push(("Exact Sig. (2-sided test)", Cell::Number(p)));
        }
        self.detail("Related-Samples McNemar Change Test", joined, rows);
    }

    fn paired(&mut self, fields: &[Variable], test: Paired) {
        let (x, y) = (&fields[0], &fields[1]);
        let cases = self.cases(&[x, y]);
        let triples: Vec<(f64, f64, f64)> = cases
            .iter()
            .map(|&c| (number_value(x, c), number_value(y, c), self.weights[c]))
            .collect();
        let differences: Vec<(f64, f64)> = triples.iter().map(|(a, b, w)| (a - b, *w)).collect();
        let total = self.total(&cases);
        let joined = self.joined(fields);
        let median_null = format!(
            "The median of the differences between {} and {} is 0.",
            self.name(x),
            self.name(y)
        );
        match test {
            Paired::Wilcoxon => {
                let signed = SignedRanks::new(&differences);
                let n = signed.np + signed.nn;
                let sd = signed.variance().sqrt();
                let z = (sd > 0.0).then(|| (signed.sp - n * (n + 1.0) / 4.0) / sd);
                let p = z.map(normal_p);
                self.summarize(
                    median_null,
                    "Related-Samples Wilcoxon Signed Rank Test",
                    Sig::new(p, "Asymptotic"),
                );
                self.detail(
                    "Related-Samples Wilcoxon Signed Rank Test",
                    joined,
                    vec![
                        ("Total N", Cell::Number(total)),
                        ("Test Statistic", Cell::Number(signed.sp)),
                        ("Standard Error", Cell::Number(sd)),
                        (
                            "Standardized Test Statistic",
                            crate::descriptives::number(z),
                        ),
                        (
                            "Asymptotic Sig. (2-sided test)",
                            crate::descriptives::number(p),
                        ),
                    ],
                );
            }
            Paired::Sign => {
                let np: f64 = differences.iter().filter(|d| d.0 > 0.0).map(|d| d.1).sum();
                let nn: f64 = differences.iter().filter(|d| d.0 < 0.0).map(|d| d.1).sum();
                let n = np + nn;
                let (z, p, kind) = if n <= 25.0 {
                    (None, sign_exact(np, nn)[0], "Exact")
                } else {
                    let z = (np.max(nn) - 0.5 * n - 0.5) / (0.5 * n.sqrt());
                    (Some(z), normal_p(z), "Asymptotic")
                };
                self.summarize(
                    median_null,
                    "Related-Samples Sign Test",
                    Sig::new(Some(p), kind),
                );
                let label = if z.is_some() {
                    "Asymptotic Sig. (2-sided test)"
                } else {
                    "Exact Sig. (2-sided test)"
                };
                self.detail(
                    "Related-Samples Sign Test",
                    joined,
                    vec![
                        ("Total N", Cell::Number(total)),
                        ("Positive Differences", Cell::Number(np)),
                        ("Negative Differences", Cell::Number(nn)),
                        ("Ties", Cell::Number(total - n)),
                        (
                            "Standardized Test Statistic",
                            crate::descriptives::number(z),
                        ),
                        (label, Cell::Number(p)),
                    ],
                );
            }
            Paired::Marginal => {
                let [observed, mean, sd, off] = marginal_statistic(&triples);
                let z = (sd > 0.0).then(|| (observed - mean) / sd);
                let p = z.map(normal_p);
                self.summarize(
                    format!(
                        "{} and {} have the same distribution of values.",
                        self.name(x),
                        self.name(y)
                    ),
                    "Related-Samples Marginal Homogeneity Test",
                    Sig::new(p, "Asymptotic"),
                );
                self.detail(
                    "Related-Samples Marginal Homogeneity Test",
                    joined,
                    vec![
                        ("Total N", Cell::Number(total)),
                        ("Off-Diagonal Cases", Cell::Number(off)),
                        ("Test Statistic", Cell::Number(observed)),
                        ("Mean of the Statistic", Cell::Number(mean)),
                        ("Standard Error", Cell::Number(sd)),
                        (
                            "Standardized Test Statistic",
                            crate::descriptives::number(z),
                        ),
                        (
                            "Asymptotic Sig. (2-sided test)",
                            crate::descriptives::number(p),
                        ),
                    ],
                );
            }
            Paired::HodgesLehmann => {
                let d = tally(&differences);
                if d.len().saturating_mul(d.len() + 1) / 2 > MAX_PAIRS {
                    self.too_many(&joined);
                    return;
                }
                let mut walsh = Vec::new();
                for (i, (a, f)) in d.iter().enumerate() {
                    walsh.push((*a, f * (f + 1.0) / 2.0));
                    for (b, g) in &d[i + 1..] {
                        walsh.push(((a + b) / 2.0, f * g));
                    }
                }
                let sorted = weighted_sorted(walsh);
                let n = total;
                let sd = (n * (n + 1.0) * (2.0 * n + 1.0) / 24.0).sqrt();
                self.hodges_lehmann_interval(
                    &sorted,
                    n * (n + 1.0) / 4.0,
                    sd,
                    "Related-Samples Hodges-Lehmann Median Difference",
                    format!(
                        "Median of the differences between {} and {}",
                        self.name(x),
                        self.name(y)
                    ),
                );
            }
        }
    }

    fn cochran(&mut self, fields: &[Variable], success: Option<&[Value]>, how: NpCompare) {
        let refs: Vec<&Variable> = fields.iter().collect();
        let cases = self.cases(&refs);
        let Some((hits, what)) = self.successes(fields, &cases, success, "COCHRAN") else {
            return;
        };
        let rows: Vec<(Vec<bool>, f64)> = hits
            .into_iter()
            .zip(&cases)
            .map(|(h, &c)| (h, self.weights[c]))
            .collect();
        let (counts, q) = cochran(&rows);
        let k = fields.len();
        let df = k as f64 - 1.0;
        let p = q.map(|q| chi_square_p(q, df));
        let n = self.total(&cases);
        let joined = self.joined(fields);
        self.summarize(
            format!("{joined} have the same proportion of values {what}."),
            "Related-Samples Cochran's Q Test",
            Sig::new(p, "Asymptotic"),
        );
        self.detail(
            "Related-Samples Cochran's Q Test",
            joined.clone(),
            vec![
                ("Total N", Cell::Number(n)),
                ("Test Statistic", crate::descriptives::number(q)),
                ("Degrees of Freedom", Cell::Number(df)),
                ("Asymptotic Sig.", crate::descriptives::number(p)),
            ],
        );
        if !self.compare_now(p, how, k) {
            return;
        }
        let labels: Vec<String> = fields.iter().map(|v| self.name(v)).collect();
        let shares: Vec<f64> = counts.iter().map(|c| c / n).collect();
        match how {
            NpCompare::Pairwise => {
                let kf = k as f64;
                let (mut sum_r, mut sum_r2) = (0.0, 0.0);
                for (h, w) in &rows {
                    let r = h.iter().filter(|x| **x).count() as f64;
                    sum_r += w * r;
                    sum_r2 += w * r * r;
                }
                let se = (2.0 * (kf * sum_r - sum_r2) / (n * n * kf * (kf - 1.0))).sqrt();
                let list = pairs(k)
                    .map(|(j, l)| Pair::z(j, l, shares[j] - shares[l], se))
                    .collect();
                self.pairwise(&joined, &labels, list, "Cochran's Q");
            }
            _ => {
                let mut order: Vec<usize> = (0..k).collect();
                order.sort_by(|&a, &b| counts[a].total_cmp(&counts[b]));
                let test = |members: &[usize]| {
                    let sub: Vec<(Vec<bool>, f64)> = rows
                        .iter()
                        .map(|(h, w)| (members.iter().map(|&m| h[m]).collect(), *w))
                        .collect();
                    let (_, q) = cochran(&sub);
                    q.map(|q| (q, chi_square_p(q, members.len() as f64 - 1.0)))
                };
                self.stepwise(
                    &joined,
                    &labels,
                    &order,
                    &shares,
                    "Proportion of successes",
                    test,
                );
            }
        }
    }

    fn k_related(&mut self, fields: &[Variable], test: Ranked, how: NpCompare) {
        let refs: Vec<&Variable> = fields.iter().collect();
        let cases = self.cases(&refs);
        let rows: Vec<(Vec<f64>, f64)> = cases
            .iter()
            .map(|&c| {
                (
                    fields.iter().map(|v| number_value(v, c)).collect(),
                    self.weights[c],
                )
            })
            .collect();
        let (sums, chi) = friedman(&rows);
        let k = fields.len();
        let df = k as f64 - 1.0;
        let n = self.total(&cases);
        let p = chi.map(|c| chi_square_p(c, df));
        let joined = self.joined(fields);
        let (title, null) = match test {
            Ranked::Friedman => (
                "Related-Samples Friedman's Two-Way Analysis of Variance by Ranks",
                format!("{joined} have the same distribution."),
            ),
            Ranked::Kendall => (
                "Related-Samples Kendall's Coefficient of Concordance",
                format!("{joined} have the same distribution (no concordance)."),
            ),
        };
        self.summarize(null, title, Sig::new(p, "Asymptotic"));
        let mut detail = vec![("Total N", Cell::Number(n))];
        if test == Ranked::Kendall {
            detail.push((
                "Kendall's W",
                crate::descriptives::number(chi.map(|c| c / (n * df))),
            ));
        }
        detail.extend([
            ("Test Statistic", crate::descriptives::number(chi)),
            ("Degrees of Freedom", Cell::Number(df)),
            ("Asymptotic Sig.", crate::descriptives::number(p)),
        ]);
        self.detail(title, joined.clone(), detail);
        if !self.compare_now(p, how, k) {
            return;
        }
        let labels: Vec<String> = fields.iter().map(|v| self.name(v)).collect();
        let means: Vec<f64> = sums.iter().map(|s| s / n).collect();
        match how {
            NpCompare::Pairwise => {
                let kf = k as f64;
                let se = (kf * (kf + 1.0) / (6.0 * n)).sqrt();
                let list = pairs(k)
                    .map(|(j, l)| Pair::z(j, l, means[j] - means[l], se))
                    .collect();
                self.pairwise(&joined, &labels, list, test.keyword_name());
            }
            _ => {
                let mut order: Vec<usize> = (0..k).collect();
                order.sort_by(|&a, &b| means[a].total_cmp(&means[b]));
                let sub_test = |members: &[usize]| {
                    let sub: Vec<(Vec<f64>, f64)> = rows
                        .iter()
                        .map(|(r, w)| (members.iter().map(|&m| r[m]).collect(), *w))
                        .collect();
                    let (_, chi) = friedman(&sub);
                    chi.map(|c| (c, chi_square_p(c, members.len() as f64 - 1.0)))
                };
                self.stepwise(&joined, &labels, &order, &means, "Mean Rank", sub_test);
            }
        }
    }
}

/// The most pairwise differences or Walsh averages the Hodges-Lehmann estimate builds.
// shortcut: the pairs are built in memory; past this many, use a selection algorithm instead
const MAX_PAIRS: usize = 5_000_000;

#[derive(Clone, Copy, PartialEq)]
enum Paired {
    Wilcoxon,
    Sign,
    Marginal,
    HodgesLehmann,
}

#[derive(Clone, Copy, PartialEq)]
enum Ranked {
    Friedman,
    Kendall,
}

impl Ranked {
    fn keyword(self) -> &'static str {
        match self {
            Ranked::Friedman => "FRIEDMAN",
            Ranked::Kendall => "KENDALL",
        }
    }

    fn keyword_name(self) -> &'static str {
        match self {
            Ranked::Friedman => "Friedman",
            Ranked::Kendall => "Kendall's W",
        }
    }
}

/// One row of a pairwise comparisons table, samples `j` and `k`.
struct Pair {
    j: usize,
    k: usize,
    statistic: Option<f64>,
    se: Option<f64>,
    z: Option<f64>,
    p: Option<f64>,
}

impl Pair {
    /// A difference over its standard error with a two-sided normal significance.
    fn z(j: usize, k: usize, statistic: f64, se: f64) -> Pair {
        let z = (se > 0.0).then(|| statistic / se);
        Pair {
            j,
            k,
            statistic: Some(statistic),
            se: Some(se),
            z,
            p: z.map(normal_p),
        }
    }
}

fn pairs(k: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..k).flat_map(move |j| (j + 1..k).map(move |l| (j, l)))
}

/// A test field split by the groups of the grouping field.
struct Sample {
    field: String,
    group: String,
    labels: Vec<String>,
    grouped: Grouped,
}

impl Sample {
    fn title(&self) -> String {
        format!("{} by {}", self.field, self.group)
    }

    fn same(&self, what: &str) -> String {
        format!(
            "The {what} of {} is the same across the categories of {}.",
            self.field, self.group
        )
    }

    fn subset(&self, members: &[usize]) -> Grouped {
        Grouped {
            keys: members.iter().map(|&m| m as f64).collect(),
            groups: members
                .iter()
                .map(|&m| self.grouped.groups[m].clone())
                .collect(),
        }
    }
}

/// The parameters estimated from distinct weighted values: normal mean and SD, uniform minimum
/// and maximum, exponential or Poisson mean.
fn estimate(distribution: KsDistribution, values: &[(f64, f64)]) -> Vec<f64> {
    let n: f64 = values.iter().map(|v| v.1).sum();
    let mean = values.iter().map(|(x, w)| x * w).sum::<f64>() / n;
    match distribution {
        KsDistribution::Normal => {
            let squares: f64 = values.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
            vec![mean, (squares / (n - 1.0)).sqrt()]
        }
        KsDistribution::Uniform => vec![
            values.first().map_or(f64::NAN, |v| v.0),
            values.last().map_or(f64::NAN, |v| v.0),
        ],
        _ => vec![mean],
    }
}

fn short(x: f64) -> String {
    crate::examine::short(x)
}

fn default_binomial() -> statscraft_syntax::NpBinomial {
    statscraft_syntax::NpBinomial {
        test_value: 0.5,
        success: None,
        cut: None,
        clopper_pearson: false,
        jeffreys: false,
        likelihood: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_median_by_hand() {
        // 1, 2, 2, 3: the middle pair 2 and 2
        assert_eq!(weighted_median(&[(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)]), 2.0);
        // 1, 4: between the two
        assert_eq!(weighted_median(&[(1.0, 1.0), (4.0, 1.0)]), 2.5);
        // 1, 4, 4: the middle case
        assert_eq!(weighted_median(&[(1.0, 1.0), (4.0, 2.0)]), 4.0);
    }

    #[test]
    fn beta_quantile_inverts() {
        // Beta(1, 1) is uniform; Beta(2, 1) has cdf x^2
        assert!((beta_quantile(0.3, 1.0, 1.0) - 0.3).abs() < 1e-12);
        assert!((beta_quantile(0.25, 2.0, 1.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn pairs_in_order() {
        let all: Vec<(usize, usize)> = pairs(3).collect();
        assert_eq!(all, vec![(0, 1), (0, 2), (1, 2)]);
    }
}
