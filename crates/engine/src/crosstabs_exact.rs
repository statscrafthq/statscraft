//! CROSSTABS /METHOD: exact and Monte Carlo significance of the tests and measures, given the
//! row and column totals of the table.
//!
//! Rules, in our words:
//! - The counts are made whole first (rounded, or truncated with COUNT TRUNCATE); empty rows and
//!   columns are left out.
//! - With both margins fixed, a table's probability under independence is the multivariate
//!   hypergeometric one. Filling the table cell by cell, column by column, the cell of row i
//!   takes x of the n cases the column still needs with probability C(m_i, x) C(M, n - x) /
//!   C(m_i + M, n), m_i what is left of row i's total and M what is left of the rows after it;
//!   the product over the cells is the probability of the table.
//! - Every statistic used here adds up over the cells once that state is known (lambda also
//!   keeps the largest cell of the column so far), so all tables share one network: a node per
//!   cell and state, holding the distinct partial values that reach it with their probability.
//!   Rows a statistic treats alike may swap, which merges states, and the table is turned so
//!   that the columns are walked over the fewer rows. The smallest and largest value the rest
//!   of the table can add are worked out per node; a partial value whose every completion
//!   falls on one side of the observed value is settled at once, as the completions of a node
//!   have probability 1 together.
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
    /// The cases of the columns after each one.
    after: Vec<u64>,
    ln_factorial: Vec<f64>,
    /// The base of the packed states, None when a state does not fit in 128 bits.
    base: Option<u128>,
}

/// How many bytes the nodes and partial values of one exact computation may take, as counted
/// here, when the memory of the computer cannot be read; the process peaks at about twice that.
const MOST_BYTES: usize = 256 << 20;

/// What one exact computation may count when the memory of the computer is known: a quarter of
/// what is free, at least `MOST_BYTES` and at most 8 GiB.
const MOST_SHARE: u64 = 4;
const LARGEST_BYTES: u64 = 8 << 30;

/// The memory free for new work, None when this system does not tell.
#[cfg(target_os = "linux")]
fn free_memory() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kilobytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes * 1024)
}

#[cfg(windows)]
fn free_memory() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_physical: u64,
        free_physical: u64,
        total_page_file: u64,
        free_page_file: u64,
        total_virtual: u64,
        free_virtual: u64,
        free_extended: u64,
    }
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        load: 0,
        total_physical: 0,
        free_physical: 0,
        total_page_file: 0,
        free_page_file: 0,
        total_virtual: 0,
        free_virtual: 0,
        free_extended: 0,
    };
    // SAFETY: `status` is a MEMORYSTATUSEX with its length set, as the call requires.
    let done = unsafe { GlobalMemoryStatusEx(&mut status) };
    (done != 0).then_some(status.free_physical)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn free_memory() -> Option<u64> {
    None
}

/// The bytes one exact computation may count on this computer.
fn memory_budget() -> usize {
    let share = free_memory().map(|free| free / MOST_SHARE);
    let bytes = share.map_or(MOST_BYTES as u64, |b| {
        b.clamp(MOST_BYTES as u64, LARGEST_BYTES)
    });
    usize::try_from(bytes).unwrap_or(MOST_BYTES)
}

/// Stops the work when the time of TIMER has run out or too much is held in memory.
struct Clock {
    deadline: Option<Instant>,
    ticks: u32,
    /// Bytes of the known ranges, and of the partial values of the next cell.
    held: [usize; 2],
    full: bool,
    /// How many bytes it may hold.
    most: usize,
}

impl Clock {
    fn new(deadline: Option<Instant>) -> Clock {
        Clock {
            deadline,
            ticks: 0,
            held: [0, 0],
            full: false,
            most: MOST_BYTES,
        }
    }

    /// A clock that lets the computation use what this computer has free.
    fn sized(deadline: Option<Instant>) -> Clock {
        Clock {
            most: memory_budget(),
            ..Clock::new(deadline)
        }
    }

    fn tick(&mut self) -> Option<()> {
        self.ticks = self.ticks.wrapping_add(1);
        let late =
            self.ticks.is_multiple_of(1024) && self.deadline.is_some_and(|d| Instant::now() > d);
        (!late).then_some(())
    }

    /// Counts `bytes` more of `kind` (0 ranges, 1 partial values).
    fn hold(&mut self, kind: usize, bytes: usize) -> Option<()> {
        self.held[kind] += bytes;
        self.full = self.held[0] + self.held[1] > self.most;
        (!self.full).then_some(())
    }
}

/// The smallest and largest value still to come from each state of a cell.
type Ranges = HashMap<u128, (f64, f64)>;

/// The significances found for one table.
pub(crate) type Found = Vec<(Stat, Outcome)>;

/// The partial values of a state with their probability, and how many were merged last.
type Partials = (Vec<(f64, f64)>, usize);

/// Sorts partial values and adds up those closer than `grid`.
fn merge(parts: &mut Vec<(f64, f64)>, grid: f64) {
    parts.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let mut kept = 0;
    for k in 0..parts.len() {
        if kept > 0 && parts[k].0 - parts[kept - 1].0 <= grid {
            parts[kept - 1].1 += parts[k].1;
        } else {
            parts[kept] = parts[k];
            kept += 1;
        }
    }
    parts.truncate(kept);
}

/// The totals left with those of interchangeable rows (`twins`) in ascending order, apart for
/// the rows the current column has passed (before `visited`) and the others: states that differ
/// only by swapping such rows have the same completions, so they merge.
fn canonical(twins: &[Vec<usize>], left: &mut [u64], visited: usize) {
    for group in twins {
        for side in [true, false] {
            let members: Vec<usize> = (group.iter())
                .copied()
                .filter(|&i| (i < visited) == side)
                .collect();
            let mut values: Vec<u64> = members.iter().map(|&i| left[i]).collect();
            values.sort_unstable();
            for (&i, v) in members.iter().zip(values) {
                left[i] = v;
            }
        }
    }
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

/// A cell of the network: column j, row i, the totals left (rows before i already took their
/// share of column j), the cases column j still needs and its largest cell so far.
struct Spot<'a> {
    j: usize,
    i: usize,
    left: &'a [u64],
    need: u64,
    largest: u64,
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
        let mut after = vec![0; columns.len()];
        for j in (0..columns.len().saturating_sub(1)).rev() {
            after[j] = after[j + 1] + columns[j + 1];
        }
        let n: u64 = rows.iter().sum();
        let mut ln_factorial = vec![0.0; n as usize + 1];
        for k in 1..ln_factorial.len() {
            ln_factorial[k] = ln_factorial[k - 1] + (k as f64).ln();
        }
        // a state packs the totals left, each below `base`, and the column's largest cell
        let base = u128::from(rows.iter().copied().max().unwrap_or(0)) + 1;
        let top = u128::from(columns.iter().copied().max().unwrap_or(0)) + 1;
        let base = (0..rows.len())
            .try_fold(top, |room, _| room.checked_mul(base))
            .map(|_| base);
        let table = Whole {
            columns_cells,
            rows,
            columns,
            after,
            ln_factorial,
            base,
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

    /// The log probability that the cell takes x, given the cells before it.
    fn ln_cell(&self, at: &Spot, x: u64) -> f64 {
        let rest: u64 = at.left[at.i + 1..].iter().sum();
        let mine = at.left[at.i];
        self.ln_choose(mine, x) + self.ln_choose(rest, at.need - x)
            - self.ln_choose(mine + rest, at.need)
    }

    /// What the cell adds to the statistic when it takes x (`ln_p` its log probability), and
    /// the column's largest cell after it.
    fn add(&self, kind: &Kind, at: &Spot, x: u64, ln_p: f64) -> (f64, u64) {
        let (i, j) = (at.i, at.j);
        let n = x as f64;
        let added = match kind {
            Kind::Squares(w) => w[i][j] * n * n,
            Kind::Ratio if x > 0 => n * n.ln(),
            Kind::Ratio => 0.0,
            Kind::Probability => -ln_p,
            Kind::Linear(a) => a[i][j] * n,
            Kind::Concordance => {
                // the cases of the other rows that lie left of the column; those of the rows
                // before i in this column are not
                let placed = |k: usize| (self.rows[k] - at.left[k]) as f64;
                let taken = (self.columns[j] - at.need) as f64;
                let below = (0..i).map(placed).sum::<f64>() - taken;
                let above: f64 = (i + 1..self.rows.len()).map(placed).sum();
                n * (below - above)
            }
            Kind::Largest => x.saturating_sub(at.largest) as f64,
        };
        let largest = match kind {
            Kind::Largest => at.largest.max(x),
            _ => 0,
        };
        (added, largest)
    }

    /// The statistic of a table given by its columns.
    fn value(&self, kind: &Kind, columns: &[Vec<u64>]) -> f64 {
        let mut left = self.rows.clone();
        let mut sum = 0.0;
        for (j, x) in columns.iter().enumerate() {
            let mut largest = 0;
            for i in 0..x.len() {
                let total: u64 = left.iter().sum();
                let at = Spot {
                    j,
                    i,
                    left: &left,
                    need: total - self.after[j],
                    largest,
                };
                let (added, top) = self.add(kind, &at, x[i], self.ln_cell(&at, x[i]));
                sum += added;
                largest = top;
                left[i] -= x[i];
            }
        }
        sum
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

    fn pack(&self, base: u128, left: &[u64], largest: u64) -> u128 {
        (left.iter().rev()).fold(u128::from(largest), |key, &m| key * base + u128::from(m))
    }

    /// The ways to fill the cell of step `s` (columns times rows, column by column) from
    /// state `key`: probability, what the statistic gains, and the state after.
    fn edges(
        &self,
        kind: &Kind,
        (s, key): (usize, u128),
        (base, twins): (u128, &[Vec<usize>]),
        out: &mut Vec<(f64, f64, u128)>,
    ) {
        out.clear();
        let r = self.rows.len();
        let (j, i) = (s / r, s % r);
        let mut rest = key;
        let mut left = vec![0; r];
        for m in left.iter_mut() {
            *m = (rest % base) as u64;
            rest /= base;
        }
        let total: u64 = left.iter().sum();
        let at = Spot {
            j,
            i,
            left: &left,
            need: total - self.after[j],
            largest: rest as u64,
        };
        let others: u64 = left[i + 1..].iter().sum();
        let mut child = left.clone();
        for x in at.need.saturating_sub(others)..=left[i].min(at.need) {
            let ln_p = self.ln_cell(&at, x);
            let (added, largest) = self.add(kind, &at, x, ln_p);
            child.copy_from_slice(&left);
            child[i] -= x;
            // the column ends after its last row: the next one starts afresh
            let (visited, largest) = if i + 1 < r { (i + 1, largest) } else { (0, 0) };
            canonical(twins, &mut child, visited);
            out.push((ln_p.exp(), added, self.pack(base, &child, largest)));
        }
    }

    /// The smallest and largest value the cells from step `s` on can add, from state `key`.
    fn range(
        &self,
        kind: &Kind,
        (s, key): (usize, u128),
        (memo, base, twins): (&mut [Ranges], u128, &[Vec<usize>]),
        clock: &mut Clock,
    ) -> Option<(f64, f64)> {
        if s == memo.len() {
            return Some((0.0, 0.0));
        }
        if let Some(found) = memo[s].get(&key) {
            return Some(*found);
        }
        let mut edges = Vec::new();
        self.edges(kind, (s, key), (base, twins), &mut edges);
        let mut range = (f64::INFINITY, f64::NEG_INFINITY);
        for (_, added, child) in edges {
            clock.tick()?;
            let (low, high) = self.range(kind, (s + 1, child), (memo, base, twins), clock)?;
            range = (range.0.min(added + low), range.1.max(added + high));
        }
        clock.hold(0, 80)?;
        memo[s].insert(key, range);
        Some(range)
    }

    /// The exact significance, None when the clock runs out or too much would be held.
    fn exact(&self, kind: &Kind, centre: Option<f64>, clock: &mut Clock) -> Option<Outcome> {
        let bounds = Bounds::new(self.value(kind, &self.columns_cells), centre, kind);
        let Bounds { lo, hi, .. } = bounds;
        let tol = bounds.tolerance;
        let grid = tol / 64.0;
        let Some(base) = self.base else {
            clock.full = true;
            return None;
        };
        let steps = self.rows.len() * self.columns.len();
        let mut memo = vec![Ranges::new(); steps];
        let twins = self.twins(kind);
        let mut start = self.rows.clone();
        canonical(&twins, &mut start, 0);
        let (mut upper, mut lower, mut point) = (0.0, 0.0, 0.0);
        let mut stage: HashMap<u128, Partials> =
            HashMap::from([(self.pack(base, &start, 0), (vec![(0.0, 1.0)], 1))]);
        let mut edges = Vec::new();
        let mut open = Vec::new();
        for s in 0..steps {
            let mut next: HashMap<u128, Partials> = HashMap::new();
            clock.held[1] = 0;
            for (key, (mut partials, _)) in stage {
                merge(&mut partials, grid);
                self.edges(kind, (s, key), (base, &twins), &mut edges);
                for &(p_step, added, child) in &edges {
                    let ranges = (&mut memo[..], base, &twins[..]);
                    let (low, high) = self.range(kind, (s + 1, child), ranges, clock)?;
                    open.clear();
                    for &(v, p) in &partials {
                        clock.tick()?;
                        let (v, p) = (v + added, p * p_step);
                        if v + low > hi + tol {
                            upper += p;
                        } else if v + high < lo - tol {
                            lower += p;
                        } else if !(v + low > lo + tol && v + high < hi - tol) {
                            open.push((v, p));
                        }
                    }
                    if open.is_empty() {
                        continue;
                    }
                    let node = next.entry(child).or_insert_with(|| (Vec::new(), 0));
                    let before = node.0.capacity();
                    node.0.extend_from_slice(&open);
                    // merge now and then, so that a state holds about its distinct values
                    if node.0.len() > 2 * node.1 + 256 {
                        merge(&mut node.0, grid);
                        node.1 = node.0.len();
                    }
                    clock.held[1] -= 16 * before;
                    clock.hold(1, 16 * node.0.capacity() + if before == 0 { 96 } else { 0 })?;
                }
            }
            stage = next;
        }
        for (v, p) in stage.into_values().flat_map(|(partials, _)| partials) {
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
    // the network walks the rows of each column, so fewer rows make fewer states
    let found = match found {
        (0, kind, centre) if r > c && !matches!(kind, Kind::Largest) => (1, flip(kind), centre),
        found => found,
    };
    tables[found.0]?.0.usable().then_some(found)
}

/// The statistic of the transposed table.
fn flip(kind: Kind) -> Kind {
    let t = |m: Vec<Vec<f64>>| -> Vec<Vec<f64>> {
        (0..m.first().map_or(0, Vec::len))
            .map(|j| m.iter().map(|row| row[j]).collect())
            .collect()
    };
    match kind {
        Kind::Squares(w) => Kind::Squares(t(w)),
        Kind::Linear(a) => Kind::Linear(t(a)),
        other => other,
    }
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
                let mut clock = Clock::sized(
                    (minutes > 0.0)
                        .then(|| Instant::now() + Duration::from_secs_f64(minutes * 60.0)),
                );
                let found = table(at).exact(&kind, centre, &mut clock);
                if found.is_none() {
                    let why = match clock.full {
                        true => format!(
                            "needs more memory than the {} MB it may use here (a quarter of what is free)",
                            clock.most >> 20
                        ),
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

    /// Every table with the margins of `f`, by column, with its probability.
    fn every_table(t: &Whole) -> Vec<(Vec<Vec<u64>>, f64)> {
        let (r, c) = (t.rows.len(), t.columns.len());
        let mut out = Vec::new();
        let mut cells = vec![vec![0; r]; c];
        fn fill(
            t: &Whole,
            k: usize,
            cells: &mut Vec<Vec<u64>>,
            out: &mut Vec<(Vec<Vec<u64>>, f64)>,
        ) {
            let (r, c) = (t.rows.len(), t.columns.len());
            if k == r * c {
                let ok = (0..r).all(|i| cells.iter().map(|col| col[i]).sum::<u64>() == t.rows[i])
                    && (0..c).all(|j| cells[j].iter().sum::<u64>() == t.columns[j]);
                if ok {
                    let f = |n: u64| t.ln_factorial[n as usize];
                    let margins: f64 = t.rows.iter().chain(&t.columns).map(|&n| f(n)).sum();
                    let inner: f64 = cells.iter().flatten().map(|&n| f(n)).sum();
                    out.push((cells.clone(), (margins - f(t.total()) - inner).exp()));
                }
                return;
            }
            let (j, i) = (k / r, k % r);
            for x in 0..=t.rows[i].min(t.columns[j]) {
                cells[j][i] = x;
                fill(t, k + 1, cells, out);
            }
        }
        fill(t, 0, &mut cells, &mut out);
        out
    }

    #[test]
    fn the_network_agrees_with_listing_every_table() {
        // invented tables: 2 x 4, its transpose, and 3 x 3
        let f = vec![vec![2.0, 0.0, 1.0, 3.0], vec![1.0, 2.0, 1.0, 1.0]];
        let flipped: Vec<Vec<f64>> = (0..4)
            .map(|j| f.iter().map(|row| row[j]).collect())
            .collect();
        let square = vec![
            vec![2.0, 0.0, 1.0],
            vec![1.0, 2.0, 0.0],
            vec![0.0, 1.0, 2.0],
        ];
        for f in [f, flipped, square] {
            let (t, ..) = Whole::new(&f, false);
            let tables = every_table(&t);
            let (r, c) = (t.rows.len(), t.columns.len());
            let a: Vec<Vec<f64>> = (0..r)
                .map(|i| (0..c).map(|j| ((i + 1) * (j * j + 1)) as f64).collect())
                .collect();
            let mean: f64 = tables
                .iter()
                .map(|(cells, p)| p * t.value(&Kind::Linear(a.clone()), cells))
                .sum();
            let w = (0..r)
                .map(|i| {
                    (0..c)
                        .map(|j| 1.0 / (t.rows[i] * t.columns[j]) as f64)
                        .collect()
                })
                .collect();
            let kinds = [
                (Kind::Squares(w), None),
                (Kind::Ratio, None),
                (Kind::Probability, None),
                (Kind::Largest, None),
                (Kind::Concordance, Some(0.0)),
                (Kind::Linear(a), Some(mean)),
            ];
            for (kind, centre) in kinds {
                let b = Bounds::new(t.value(&kind, &t.columns_cells), centre, &kind);
                let (mut two, mut one) = (0.0, 0.0);
                for (cells, p) in &tables {
                    let (upper, lower) = b.tail(t.value(&kind, cells));
                    two += if upper || lower { *p } else { 0.0 };
                    one += if (b.upper_side && upper) || (!b.upper_side && lower) {
                        *p
                    } else {
                        0.0
                    };
                }
                let Some(Outcome::Exact {
                    two: got,
                    one: side,
                    ..
                }) = t.exact(&kind, centre, &mut Clock::new(None))
                else {
                    panic!()
                };
                assert!((got - two).abs() < 1e-12, "{got} {two}");
                if let Some(side) = side {
                    assert!((side - one).abs() < 1e-12, "{side} {one}");
                }
            }
        }
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

    #[test]
    fn a_small_budget_stops_the_work_and_the_computers_budget_is_in_range() {
        let f = vec![
            vec![5.0, 4.0, 3.0],
            vec![2.0, 6.0, 4.0],
            vec![3.0, 3.0, 7.0],
        ];
        let (table, ..) = Whole::new(&f, false);
        let mut clock = Clock::new(None);
        clock.most = 1 << 10;
        assert_eq!(table.exact(&Kind::Probability, None, &mut clock), None);
        assert!(clock.full);
        // the budget a computer gets is between the two limits, whatever it has free
        let budget = memory_budget() as u64;
        assert!((MOST_BYTES as u64..=LARGEST_BYTES).contains(&budget));
    }
}
