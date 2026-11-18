//! CROSSTABS /METHOD: exact and Monte Carlo significance of the tests and measures, given the
//! row and column totals of the table.
//!
//! Rules, in our words:
//! - The counts are made whole first (rounded, or truncated with COUNT TRUNCATE); empty rows and
//!   columns are left out.
//! - With both margins fixed, a table's probability under independence is the multivariate
//!   hypergeometric one. Filling the table one column at a time, column j takes x_i from what
//!   is left of the row totals, m_i, with probability prod_i C(m_i, x_i) / C(sum m, c_j); the
//!   product over the columns is the probability of the table.
//! - Every statistic used here adds up over the columns once the totals left before a column
//!   are known, so all tables share one network: a node per column and vector of totals left,
//!   holding the distinct partial values that reach it with their probability. The smallest and
//!   largest value the rest of the table can add are worked out per node; a partial value whose
//!   every completion falls on one side of the observed value is settled at once, as the
//!   completions of a node have probability 1 together.
//! - Statistics that grow with departure from independence (Pearson's and the likelihood-ratio
//!   chi-square, minus the log probability of the table for Fisher's test, lambda, Goodman and
//!   Kruskal's tau): the significance is the probability of a value at least the observed one.
//!   Signed ones (the sum of score products of linear-by-linear, Pearson's r and Spearman's
//!   correlation, Kendall's S, the diagonal of kappa): two-sided, a distance from the mean at
//!   least the observed one; one-sided, the tail on the observed side. The point probability is
//!   that of the observed value; for Fisher's test that of the observed table. Values closer
//!   than 1e-7 (relative) count as equal.
//! - Monte Carlo: tables with the same margins drawn by shuffling the column categories of the
//!   cases against their rows; the significance is the share of draws as extreme as the table,
//!   with the interval p +- z root(p (1 - p) / samples) at the asked level, cut at 0 and 1.

use crate::crosstabs::whole;
use crate::nptests::monte_carlo;
use crate::random::Random;
use statscraft_syntax::CrosstabsMethod;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A statistic that gets an exact or Monte Carlo significance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stat {
    Pearson,
    Ratio,
    Fisher,
    /// The linear-by-linear association and Pearson's r: products of the category values.
    Linear,
    /// Kendall's S, behind tau-b, tau-c and Somers' d.
    Concordance,
    Spearman,
    Kappa,
    LambdaRows,
    LambdaColumns,
    TauRows,
    TauColumns,
}

impl Stat {
    /// The statistic whose significance a line of the chi-square or a measures table shows;
    /// `dependent` is 0 for a symmetric value, 1 with the rows dependent, 2 with the columns.
    pub(crate) fn of(name: &str, dependent: u8) -> Option<Stat> {
        Some(match (name, dependent) {
            ("Pearson Chi-Square" | "Phi" | "Cramer's V" | "Contingency Coefficient", _) => {
                Stat::Pearson
            }
            ("Likelihood Ratio" | "Uncertainty Coefficient", _) => Stat::Ratio,
            ("Fisher's Exact Test", _) => Stat::Fisher,
            ("Linear-by-Linear Association" | "Pearson's R", _) => Stat::Linear,
            ("Kendall's tau-b" | "Kendall's tau-c" | "Somers' d", _) => Stat::Concordance,
            ("Spearman Correlation", _) => Stat::Spearman,
            ("Kappa", _) => Stat::Kappa,
            ("Lambda", 1) => Stat::LambdaRows,
            ("Lambda", 2) => Stat::LambdaColumns,
            ("Goodman and Kruskal tau", 1) => Stat::TauRows,
            ("Goodman and Kruskal tau", 2) => Stat::TauColumns,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Stat::Pearson => "Pearson chi-square",
            Stat::Ratio => "likelihood ratio",
            Stat::Fisher => "Fisher's test",
            Stat::Linear => "linear-by-linear association",
            Stat::Concordance => "Kendall's S",
            Stat::Spearman => "Spearman correlation",
            Stat::Kappa => "kappa",
            Stat::LambdaRows | Stat::LambdaColumns => "lambda",
            Stat::TauRows | Stat::TauColumns => "Goodman and Kruskal tau",
        }
    }
}

/// An exact or a Monte Carlo result: the two-sided significance, the one-sided one of a signed
/// statistic and, exact only, the point probability. Monte Carlo values come with their
/// interval as [estimate, lower, upper].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Outcome {
    Exact {
        two: f64,
        one: Option<f64>,
        point: f64,
    },
    MonteCarlo {
        two: [f64; 3],
        one: Option<[f64; 3]>,
    },
}

/// One table of a CROSSTABS run as the significance needs it: the counts with rows and columns
/// ascending, the category values when both variables are numeric, and the square table of
/// kappa when it has one.
pub(crate) struct Input<'a> {
    pub(crate) f: &'a [Vec<f64>],
    pub(crate) values: Option<(&'a [f64], &'a [f64])>,
    pub(crate) square: Option<&'a [Vec<f64>]>,
    pub(crate) truncate: bool,
}

/// What a statistic adds for a column.
enum Kind {
    /// sum w_ij x_ij^2: Pearson (w = 1 / (r_i c_j)) and Goodman and Kruskal's tau.
    Squares(Vec<Vec<f64>>),
    /// sum x ln x: the likelihood ratio.
    Ratio,
    /// Minus the log of the column's probability: Fisher's test.
    Probability,
    /// sum a_ij x_ij.
    Linear(Vec<Vec<f64>>),
    /// The concordant less the discordant pairs the column makes with those left of it.
    Concordance,
    /// The largest cell of the column.
    Largest,
}

/// A table of whole counts without empty rows or columns.
struct Whole {
    /// By column, then row.
    columns_cells: Vec<Vec<u64>>,
    rows: Vec<u64>,
    columns: Vec<u64>,
    ln_factorial: Vec<f64>,
}

/// How many nodes and partial values the exact computation may hold at once, some hundreds
/// of megabytes.
const MOST_HELD: usize = 4_000_000;

/// Stops the work when the time of TIMER has run out or too much is held in memory.
struct Clock {
    deadline: Option<Instant>,
    ticks: u32,
    /// Nodes whose range is known, and partial values of the next column.
    held: [usize; 2],
    full: bool,
}

impl Clock {
    fn new(deadline: Option<Instant>) -> Clock {
        Clock {
            deadline,
            ticks: 0,
            held: [0, 0],
            full: false,
        }
    }

    fn tick(&mut self) -> Option<()> {
        self.ticks = self.ticks.wrapping_add(1);
        let late =
            self.ticks.is_multiple_of(1024) && self.deadline.is_some_and(|d| Instant::now() > d);
        (!late).then_some(())
    }

    /// Counts one more held item of `kind` (0 nodes, 1 partial values).
    fn hold(&mut self, kind: usize) -> Option<()> {
        self.held[kind] += 1;
        self.full = self.held[0] + self.held[1] > MOST_HELD;
        (!self.full).then_some(())
    }
}

/// Calls `visit` with every column that takes `need` cases from rows with `left` cases left.
fn each_column(left: &[u64], need: u64, visit: &mut dyn FnMut(&[u64])) {
    let mut room = vec![0; left.len() + 1];
    for i in (0..left.len()).rev() {
        room[i] = room[i + 1] + left[i];
    }
    fn walk(
        i: usize,
        rest: u64,
        left: &[u64],
        room: &[u64],
        x: &mut [u64],
        visit: &mut dyn FnMut(&[u64]),
    ) {
        if i == left.len() {
            visit(x);
            return;
        }
        for v in rest.saturating_sub(room[i + 1])..=rest.min(left[i]) {
            x[i] = v;
            walk(i + 1, rest - v, left, room, x, visit);
        }
    }
    walk(0, need, left, &room, &mut vec![0; left.len()], visit);
}

fn minus(left: &[u64], x: &[u64]) -> Vec<u64> {
    left.iter().zip(x).map(|(m, x)| m - x).collect()
}

/// The smallest and largest value still to come from each node of a column.
type Ranges = HashMap<Vec<u64>, (f64, f64)>;

/// The significances found for one table.
pub(crate) type Found = Vec<(Stat, Outcome)>;

/// The totals left with those of interchangeable rows (`twins`) in ascending order: nodes that
/// differ only by swapping such rows have the same completions, so they merge.
fn canonical(twins: &[Vec<usize>], mut left: Vec<u64>) -> Vec<u64> {
    for group in twins {
        let mut values: Vec<u64> = group.iter().map(|&i| left[i]).collect();
        values.sort_unstable();
        for (&i, v) in group.iter().zip(values) {
            left[i] = v;
        }
    }
    left
}

/// The thresholds of a statistic: values from `hi` up and from `lo` down are extreme.
struct Bounds {
    observed: f64,
    /// Whether the observed value is on the upper side of the mean (always when unsigned).
    upper_side: bool,
    lo: f64,
    hi: f64,
    tolerance: f64,
}

impl Bounds {
    fn new(observed: f64, centre: Option<f64>, kind: &Kind) -> Bounds {
        let tolerance = match kind {
            Kind::Probability => 1e-7,
            _ => 1e-7 * observed.abs().max(1.0),
        };
        let (lo, hi) = match centre {
            Some(mean) => {
                let d = (observed - mean).abs();
                (mean - d, mean + d)
            }
            None => (f64::NEG_INFINITY, observed),
        };
        Bounds {
            observed,
            upper_side: centre.is_none_or(|mean| observed >= mean),
            lo,
            hi,
            tolerance,
        }
    }

    /// Whether a value is in the upper tail, else in the lower one.
    fn tail(&self, v: f64) -> (bool, bool) {
        let upper = v >= self.hi - self.tolerance;
        (upper, !upper && v <= self.lo + self.tolerance)
    }
}

impl Whole {
    /// The whole counts of `f` (rows by columns) and the rows and columns kept.
    fn new(f: &[Vec<f64>], truncate: bool) -> (Whole, Vec<usize>, Vec<usize>) {
        let cells: Vec<Vec<u64>> = (f.iter())
            .map(|row| {
                row.iter()
                    .map(|&n| whole(n.max(0.0), truncate) as u64)
                    .collect()
            })
            .collect();
        let width = cells.first().map_or(0, Vec::len);
        let kept_rows: Vec<usize> = (0..cells.len())
            .filter(|&i| cells[i].iter().sum::<u64>() > 0)
            .collect();
        let kept_columns: Vec<usize> = (0..width)
            .filter(|&j| cells.iter().map(|row| row[j]).sum::<u64>() > 0)
            .collect();
        let columns_cells: Vec<Vec<u64>> = (kept_columns.iter())
            .map(|&j| kept_rows.iter().map(|&i| cells[i][j]).collect())
            .collect();
        let rows: Vec<u64> = (0..kept_rows.len())
            .map(|i| columns_cells.iter().map(|c| c[i]).sum())
            .collect();
        let columns: Vec<u64> = columns_cells.iter().map(|c| c.iter().sum()).collect();
        let n: u64 = rows.iter().sum();
        let mut ln_factorial = vec![0.0; n as usize + 1];
        for k in 1..ln_factorial.len() {
            ln_factorial[k] = ln_factorial[k - 1] + (k as f64).ln();
        }
        let table = Whole {
            columns_cells,
            rows,
            columns,
            ln_factorial,
        };
        (table, kept_rows, kept_columns)
    }

    fn usable(&self) -> bool {
        self.rows.len() >= 2 && self.columns.len() >= 2
    }

    fn total(&self) -> u64 {
        self.rows.iter().sum()
    }

    fn ln_choose(&self, n: u64, k: u64) -> f64 {
        let f = |k: u64| self.ln_factorial[k as usize];
        f(n) - f(k) - f(n - k)
    }

    /// The log probability that column j takes `x` from the totals `left`.
    fn ln_step(&self, j: usize, x: &[u64], left: &[u64]) -> f64 {
        let ways: f64 = left
            .iter()
            .zip(x)
            .map(|(&m, &x)| self.ln_choose(m, x))
            .sum();
        ways - self.ln_choose(left.iter().sum(), self.columns[j])
    }

    /// What column j with cells `x` adds to the statistic, `left` the row totals before it.
    fn step(&self, kind: &Kind, j: usize, x: &[u64], left: &[u64]) -> f64 {
        match kind {
            Kind::Squares(w) => (0..x.len()).map(|i| w[i][j] * (x[i] * x[i]) as f64).sum(),
            Kind::Ratio => (x.iter())
                .filter(|&&x| x > 0)
                .map(|&x| x as f64 * (x as f64).ln())
                .sum(),
            Kind::Probability => -self.ln_step(j, x, left),
            Kind::Linear(a) => (0..x.len()).map(|i| a[i][j] * x[i] as f64).sum(),
            Kind::Concordance => {
                // the cases of row i already placed lie left of the column
                let before: Vec<f64> = (0..x.len())
                    .map(|i| (self.rows[i] - left[i]) as f64)
                    .collect();
                let all: f64 = before.iter().sum();
                let mut below = 0.0;
                let mut s = 0.0;
                for i in 0..x.len() {
                    let above = all - below - before[i];
                    s += x[i] as f64 * (below - above);
                    below += before[i];
                }
                s
            }
            Kind::Largest => x.iter().copied().max().unwrap_or(0) as f64,
        }
    }

    /// The groups of rows a statistic treats alike, so that they may trade places.
    fn twins(&self, kind: &Kind) -> Vec<Vec<usize>> {
        let all = || vec![(0..self.rows.len()).collect()];
        match kind {
            Kind::Ratio | Kind::Probability | Kind::Largest => all(),
            Kind::Squares(w) => {
                let mut groups: Vec<Vec<usize>> = Vec::new();
                for i in 0..w.len() {
                    match groups.iter_mut().find(|g| w[g[0]] == w[i]) {
                        Some(g) => g.push(i),
                        None => groups.push(vec![i]),
                    }
                }
                groups
            }
            Kind::Linear(_) | Kind::Concordance => Vec::new(),
        }
    }

    /// The statistic of a table given by its columns.
    fn value(&self, kind: &Kind, columns: &[Vec<u64>]) -> f64 {
        let mut left = self.rows.clone();
        let mut sum = 0.0;
        for (j, x) in columns.iter().enumerate() {
            sum += self.step(kind, j, x, &left);
            left = minus(&left, x);
        }
        sum
    }

    /// The smallest and largest value columns j on can add, given the totals `left`.
    fn range(
        &self,
        kind: &Kind,
        (j, left): (usize, &[u64]),
        (memo, twins): (&mut [Ranges], &[Vec<usize>]),
        clock: &mut Clock,
    ) -> Option<(f64, f64)> {
        if j == self.columns.len() {
            return Some((0.0, 0.0));
        }
        if let Some(found) = memo[j].get(left) {
            return Some(*found);
        }
        clock.tick()?;
        let mut xs = Vec::new();
        each_column(left, self.columns[j], &mut |x| xs.push(x.to_vec()));
        let mut range = (f64::INFINITY, f64::NEG_INFINITY);
        for x in xs {
            let s = self.step(kind, j, &x, left);
            let child = canonical(twins, minus(left, &x));
            let (low, high) = self.range(kind, (j + 1, &child), (memo, twins), clock)?;
            range = (range.0.min(s + low), range.1.max(s + high));
        }
        clock.hold(0)?;
        memo[j].insert(left.to_vec(), range);
        Some(range)
    }

    /// The exact significance, None when the clock runs out or too much is held.
    fn exact(&self, kind: &Kind, centre: Option<f64>, clock: &mut Clock) -> Option<Outcome> {
        let bounds = Bounds::new(self.value(kind, &self.columns_cells), centre, kind);
        let Bounds { lo, hi, .. } = bounds;
        let tol = bounds.tolerance;
        let grid = tol / 64.0;
        let mut memo = vec![HashMap::new(); self.columns.len() + 1];
        let twins = self.twins(kind);
        let (mut upper, mut lower, mut point) = (0.0, 0.0, 0.0);
        type Partials = HashMap<i128, (f64, f64)>;
        let mut stage: HashMap<Vec<u64>, Partials> = HashMap::from([(
            canonical(&twins, self.rows.clone()),
            HashMap::from([(0, (0.0, 1.0))]),
        )]);
        for j in 0..self.columns.len() {
            let mut next: HashMap<Vec<u64>, Partials> = HashMap::new();
            for (left, partials) in stage {
                let mut xs = Vec::new();
                each_column(&left, self.columns[j], &mut |x| xs.push(x.to_vec()));
                for x in xs {
                    clock.tick()?;
                    let child = canonical(&twins, minus(&left, &x));
                    let (low, high) =
                        self.range(kind, (j + 1, &child), (&mut memo, &twins), clock)?;
                    let s = self.step(kind, j, &x, &left);
                    let p_step = self.ln_step(j, &x, &left).exp();
                    let mut open = Vec::new();
                    for &(v, p) in partials.values() {
                        let (v, p) = (v + s, p * p_step);
                        if v + low > hi + tol {
                            upper += p;
                        } else if v + high < lo - tol {
                            lower += p;
                        } else if !(v + low > lo + tol && v + high < hi - tol) {
                            open.push((v, p));
                        }
                    }
                    if !open.is_empty() {
                        let slot = next.entry(child).or_default();
                        for (v, p) in open {
                            let key = (v / grid).round() as i128;
                            if !slot.contains_key(&key) {
                                clock.hold(1)?;
                            }
                            slot.entry(key).or_insert((v, 0.0)).1 += p;
                        }
                    }
                }
            }
            stage = next;
            clock.held[1] = 0;
        }
        for &(v, p) in stage.values().flat_map(HashMap::values) {
            match bounds.tail(v) {
                (true, _) => upper += p,
                (_, true) => lower += p,
                _ => {}
            }
            if (v - bounds.observed).abs() <= tol {
                point += p;
            }
        }
        // Fisher's test: the probability of the observed table itself, not of all as likely
        if matches!(kind, Kind::Probability) {
            point = (-bounds.observed).exp();
        }
        let side = centre.map(|_| if bounds.upper_side { upper } else { lower });
        Some(Outcome::Exact {
            two: (upper + lower).min(1.0),
            one: side.map(|p| p.min(1.0)),
            point: point.min(1.0),
        })
    }

    /// A table with these margins drawn at random, by column.
    fn draw(&self, labels: &mut [usize], random: &mut Random) -> Vec<Vec<u64>> {
        for i in (1..labels.len()).rev() {
            let k = ((random.uniform() * (i + 1) as f64) as usize).min(i);
            labels.swap(i, k);
        }
        let mut cells = vec![vec![0; self.rows.len()]; self.columns.len()];
        let mut at = 0;
        for (i, &r) in self.rows.iter().enumerate() {
            for &j in &labels[at..at + r as usize] {
                cells[j][i] += 1;
            }
            at += r as usize;
        }
        cells
    }
}

/// The table a statistic works on (0 the table, 1 its transpose, 2 the square table of kappa),
/// what it adds up and its mean when it is signed.
fn test(
    stat: Stat,
    input: &Input,
    tables: [Option<&Kept>; 3],
) -> Option<(usize, Kind, Option<f64>)> {
    let (table, kept_rows, kept_columns) = tables[0]?;
    let (r, c) = (table.rows.len(), table.columns.len());
    let grid = |cell: &dyn Fn(usize, usize) -> f64| -> Vec<Vec<f64>> {
        (0..r)
            .map(|i| (0..c).map(|j| cell(i, j)).collect())
            .collect()
    };
    let linear = |at: usize, table: &Whole, a: Vec<Vec<f64>>| {
        let n = table.total() as f64;
        let mut mean = 0.0;
        for (i, &ri) in table.rows.iter().enumerate() {
            for (j, &cj) in table.columns.iter().enumerate() {
                mean += a[i][j] * (ri * cj) as f64 / n;
            }
        }
        (at, Kind::Linear(a), Some(mean))
    };
    let (rows, columns) = (&table.rows, &table.columns);
    let found = match stat {
        Stat::Pearson => (
            0,
            Kind::Squares(grid(&|i, j| 1.0 / (rows[i] * columns[j]) as f64)),
            None,
        ),
        Stat::Ratio => (0, Kind::Ratio, None),
        Stat::Fisher => (0, Kind::Probability, None),
        Stat::Concordance => (0, Kind::Concordance, Some(0.0)),
        Stat::LambdaRows => (0, Kind::Largest, None),
        Stat::LambdaColumns => (1, Kind::Largest, None),
        Stat::TauRows => (
            0,
            Kind::Squares(grid(&|_, j| 1.0 / columns[j] as f64)),
            None,
        ),
        Stat::TauColumns => (0, Kind::Squares(grid(&|i, _| 1.0 / rows[i] as f64)), None),
        Stat::Linear => {
            let (x, y) = input.values?;
            linear(0, table, grid(&|i, j| x[kept_rows[i]] * y[kept_columns[j]]))
        }
        Stat::Spearman => {
            input.values?;
            let ranks = |totals: &[u64]| -> Vec<f64> {
                let mut below = 0.0;
                (totals.iter())
                    .map(|&n| {
                        let rank = below + (n as f64 + 1.0) / 2.0;
                        below += n as f64;
                        rank
                    })
                    .collect()
            };
            let (x, y) = (ranks(rows), ranks(columns));
            linear(0, table, grid(&|i, j| x[i] * y[j]))
        }
        Stat::Kappa => {
            let (square, kept_rows, kept_columns) = tables[2]?;
            let a = (kept_rows.iter())
                .map(|i| {
                    kept_columns
                        .iter()
                        .map(|j| f64::from(u8::from(i == j)))
                        .collect()
                })
                .collect();
            linear(2, square, a)
        }
    };
    tables[found.0]?.0.usable().then_some(found)
}

/// A table of whole counts with the rows and columns of the counts it keeps.
type Kept = (Whole, Vec<usize>, Vec<usize>);

/// The significance of `stats` on one table by `method`; a statistic without one (too few
/// rows or columns, no values, or out of time) is left out, the last with a warning.
pub(crate) fn significances(
    input: &Input,
    stats: &[Stat],
    method: CrosstabsMethod,
    random: &mut Random,
    warnings: &mut Vec<String>,
) -> Found {
    let transposed: Vec<Vec<f64>> = (0..input.f.first().map_or(0, Vec::len))
        .map(|j| input.f.iter().map(|row| row[j]).collect())
        .collect();
    let kept: [Option<Kept>; 3] = [
        Some(Whole::new(input.f, input.truncate)),
        Some(Whole::new(&transposed, input.truncate)),
        input.square.map(|f| Whole::new(f, input.truncate)),
    ];
    let tables = [kept[0].as_ref(), kept[1].as_ref(), kept[2].as_ref()];
    let tests: Vec<(Stat, usize, Kind, Option<f64>)> = (stats.iter())
        .filter_map(|&s| {
            let (at, kind, centre) = test(s, input, tables)?;
            Some((s, at, kind, centre))
        })
        .collect();
    let table = |at: usize| &tables[at].expect("tested").0;
    match method {
        CrosstabsMethod::Exact { minutes } => (tests.into_iter())
            .filter_map(|(stat, at, kind, centre)| {
                let mut clock = Clock::new(
                    (minutes > 0.0)
                        .then(|| Instant::now() + Duration::from_secs_f64(minutes * 60.0)),
                );
                let found = table(at).exact(&kind, centre, &mut clock);
                if found.is_none() {
                    let why = match clock.full {
                        true => "is too large to hold in memory".to_string(),
                        false => format!("ran past TIMER ({minutes} minutes)"),
                    };
                    warnings.push(format!(
                        "CROSSTABS: the exact test of the {} {why}; only its asymptotic significance is shown, and METHOD=MC estimates it",
                        stat.label()
                    ));
                }
                Some((stat, found?))
            })
            .collect(),
        CrosstabsMethod::MonteCarlo { level, samples } => {
            let bounds: Vec<Bounds> = (tests.iter())
                .map(|(_, at, kind, centre)| {
                    let t = table(*at);
                    Bounds::new(t.value(kind, &t.columns_cells), *centre, kind)
                })
                .collect();
            // each table a statistic works on is drawn once per sample
            let mut labels: Vec<Vec<usize>> = (0..3)
                .map(|at| match tables[at] {
                    Some((t, ..)) => (t.columns.iter().enumerate())
                        .flat_map(|(j, &c)| std::iter::repeat_n(j, c as usize))
                        .collect(),
                    None => Vec::new(),
                })
                .collect();
            let used: Vec<usize> = (0..3).filter(|at| tests.iter().any(|t| t.1 == *at)).collect();
            let mut drawn: Vec<Vec<Vec<u64>>> = vec![Vec::new(); 3];
            let mut hits = vec![(0u64, 0u64); tests.len()];
            for _ in 0..samples {
                for &at in &used {
                    drawn[at] = table(at).draw(&mut labels[at], random);
                }
                for (k, (_, at, kind, _)) in tests.iter().enumerate() {
                    let b = &bounds[k];
                    let (upper, lower) = b.tail(table(*at).value(kind, &drawn[*at]));
                    hits[k].0 += u64::from(upper || lower);
                    hits[k].1 += u64::from(if b.upper_side { upper } else { lower });
                }
            }
            let n = samples as usize;
            (tests.iter().zip(hits))
                .map(|((stat, _, _, centre), (two, one))| {
                    let one = centre.map(|_| monte_carlo(one as usize, n, level));
                    let two = monte_carlo(two as usize, n, level);
                    (*stat, Outcome::MonteCarlo { two, one })
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(f: &[Vec<f64>]) -> Input<'_> {
        Input {
            f,
            values: None,
            square: None,
            truncate: false,
        }
    }

    #[test]
    fn columns_cover_every_split() {
        let mut found = Vec::new();
        each_column(&[2, 1, 3], 3, &mut |x| found.push(x.to_vec()));
        // the splits of 3 cases over rows with room for 2, 1 and 3
        assert_eq!(found.len(), 6);
        assert!(found.iter().all(|x| x.iter().sum::<u64>() == 3));
        assert!(found.iter().all(|x| x[0] <= 2 && x[1] <= 1));
    }

    #[test]
    fn a_two_by_two_matches_fisher() {
        // invented counts; the two-sided value is the classic hypergeometric sum
        let f = vec![vec![3.0, 1.0], vec![1.0, 3.0]];
        let (two, _) = crate::crosstabs::fisher(3, 1, 1, 3);
        let mut random = Random::seeded(1);
        let found = significances(
            &input(&f),
            &[Stat::Fisher],
            CrosstabsMethod::Exact { minutes: 0.0 },
            &mut random,
            &mut Vec::new(),
        );
        let Outcome::Exact {
            two: got, point, ..
        } = found[0].1
        else {
            panic!("{found:?}")
        };
        assert!((got - two).abs() < 1e-12, "{got} {two}");
        // the observed table alone: 16/70
        assert!((point - 16.0 / 70.0).abs() < 1e-12);
    }

    #[test]
    fn a_clock_that_ran_out_gives_nothing() {
        let f = vec![
            vec![5.0, 4.0, 3.0],
            vec![2.0, 6.0, 4.0],
            vec![3.0, 3.0, 7.0],
        ];
        let (table, ..) = Whole::new(&f, false);
        let mut clock = Clock::new(Some(Instant::now() - Duration::from_secs(1)));
        clock.ticks = 1023;
        assert_eq!(table.exact(&Kind::Probability, None, &mut clock), None);
    }
}
