//! HILOGLINEAR: hierarchical loglinear models of a multiway table, fitted by iterative
//! proportional fitting, with backward elimination; and helpers GENLOG shares.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Each variable has an integer range; a value is truncated to an integer, and a case with a
//!   value outside its range is left out (and counted as out of range). Every integer of a
//!   range is a category, so the table has every combination. A case missing on any variable
//!   is left out (INCLUDE keeps user-missing values); weights are used as they are, and a case
//!   with a weight of 0 or less does not count.
//! - The generating class lists the highest terms; every subset of a term is in the model.
//!   MAXORDER k replaces a term of more than k variables by its k-variable subsets; without a
//!   DESIGN the model is the saturated one, or every k-variable term under MAXORDER k.
//! - Fitting starts from 1 in every cell, or from the cell weight cut to [0, 1] under CWEIGHT
//!   (a 0 is a structural zero), and scales the fitted table to each margin of the generating
//!   class in turn. A cycle that moves no cell by CONVERGE or more stops it; the default is the
//!   larger of 0.25 and a thousandth of the largest count times its cell weight. At most
//!   ITERATE cycles.
//! - Pearson chi-square over cells fitted above 0; likelihood ratio over cells with a count
//!   and a fit above 0. Unadjusted df: cells minus parameters (1 plus the product of
//!   (levels - 1) over every term in the model); the adjusted df also subtracts the cells fitted
//!   as 0 and gives the significance of the goodness-of-fit tests.
//! - A saturated model is fitted to the counts plus DELTA, which the cell table shows. Its
//!   parameters (effect coding, last levels left out) are sums of coefficients times log
//!   (count + DELTA): the product over the term's variables of (levels - 1) where the cell's
//!   level matches and -1 where it does not, over the number of cells; the variance is the sum
//!   of squared coefficients over (count + DELTA). They need every count + DELTA and cell weight
//!   above 0.
//! - Tests of k-way and higher effects: the model of every (k - 1)-way term against the counts
//!   (k = 1: equal cells), with unadjusted df; the k-way effects are the differences between
//!   neighbouring rows. Partial association of a k-way term: the model of all k-way terms less
//!   that one, minus the model of all of them.
//! - Backward elimination: from the generating class, each term in turn is replaced by its
//!   subsets one order lower; the term whose removal raises the likelihood ratio chi-square with
//!   the largest p is dropped when that p exceeds P (the first term on a tie), at most MAXSTEPS
//!   times.

use crate::chart::{Axis, ChartKind};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_p, normal_quantile};
use crate::glm::ginverse;
use crate::npar_tests::text;
use crate::plot::{Plot, Point, PointGroup, ReferenceLine};
use crate::regression::{plain, table};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_syntax::{CellWeights, HiloDesign, HiloPrint, Hiloglinear};

/// The largest table the procedures build.
pub(crate) const MAX_CELLS: usize = 1_000_000;

/// The level of every factor in cell `index`; the last factor changes fastest.
pub(crate) fn cell_levels(mut index: usize, levels: &[usize]) -> Vec<usize> {
    let mut out = vec![0; levels.len()];
    for f in (0..levels.len()).rev() {
        out[f] = index % levels[f];
        index /= levels[f];
    }
    out
}

/// The diagonal of the hat matrix, m_i x_i' (X' M X)^- x_i, over the rows in use.
pub(crate) fn hat_values(x: &[Vec<f64>], m: &[f64], used: &[bool]) -> Vec<f64> {
    let p = x.first().map_or(0, Vec::len);
    let mut xmx = vec![vec![0.0; p]; p];
    for (i, row) in x.iter().enumerate() {
        if !used[i] || m[i] <= 0.0 {
            continue;
        }
        for a in 0..p {
            if row[a] != 0.0 {
                for b in 0..p {
                    xmx[a][b] += m[i] * row[a] * row[b];
                }
            }
        }
    }
    let (inv, _) = ginverse(&xmx, 1e-10);
    x.iter()
        .enumerate()
        .map(|(i, row)| {
            if !used[i] || m[i] <= 0.0 {
                return f64::NAN;
            }
            let q: f64 = (0..p)
                .map(|a| row[a] * (0..p).map(|b| inv[a][b] * row[b]).sum::<f64>())
                .sum();
            m[i] * q
        })
        .collect()
}

/// Scatter plots of a residual against the observed and the expected counts.
pub(crate) fn residual_by_counts(
    procedure: &str,
    residual: &str,
    points: &[(f64, f64, f64)],
) -> Vec<Chart> {
    let make = |against: &str, pick: fn(&(f64, f64, f64)) -> f64| {
        Chart::plot(
            ChartKind::Scatter,
            format!("{procedure}: {residual} by {against} count"),
            Axis::titled(format!("{} count", capitalized(against))),
            Axis::titled(capitalized(residual)),
            Plot::Points {
                groups: vec![PointGroup {
                    name: capitalized(residual),
                    points: points
                        .iter()
                        .map(|t| Point {
                            x: pick(t),
                            y: t.2,
                            z: None,
                            label: None,
                        })
                        .collect(),
                }],
                lines: Vec::new(),
                z_axis: None,
            },
        )
    };
    vec![make("observed", |t| t.0), make("expected", |t| t.1)]
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// The normal Q-Q plot of residuals, and the detrended one when asked. The i-th smallest of n
/// residuals is set against the normal quantile of i / (n + 1).
pub(crate) fn normal_plots(
    procedure: &str,
    residual: &str,
    values: &[f64],
    detrended: bool,
) -> Vec<Chart> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return Vec::new();
    }
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len() as f64;
    let scores: Vec<f64> = (1..=sorted.len())
        .map(|i| normal_quantile(i as f64 / (n + 1.0)))
        .collect();
    let group = |points: Vec<Point>| {
        vec![PointGroup {
            name: capitalized(residual),
            points,
        }]
    };
    let point = |x: f64, y: f64| Point {
        x,
        y,
        z: None,
        label: None,
    };
    let mut out = vec![Chart::plot(
        ChartKind::NormalQq,
        format!("{procedure}: normal Q-Q plot of {residual}s"),
        Axis::titled(capitalized(residual)),
        Axis::titled("Expected normal"),
        Plot::Points {
            groups: group(
                sorted
                    .iter()
                    .zip(&scores)
                    .map(|(&v, &s)| point(v, s))
                    .collect(),
            ),
            lines: vec![ReferenceLine {
                intercept: 0.0,
                slope: 1.0,
            }],
            z_axis: None,
        },
    )];
    if detrended {
        out.push(Chart::plot(
            ChartKind::DetrendedNormalQq,
            format!("{procedure}: detrended normal Q-Q plot of {residual}s"),
            Axis::titled(capitalized(residual)),
            Axis::titled("Deviation from normal"),
            Plot::Points {
                groups: group(
                    sorted
                        .iter()
                        .zip(&scores)
                        .map(|(&v, &s)| point(v, v - s))
                        .collect(),
                ),
                lines: vec![ReferenceLine {
                    intercept: 0.0,
                    slope: 0.0,
                }],
                z_axis: None,
            },
        ));
    }
    out
}

/// A term as a set of factors; bit f is factor f.
type Term = u32;

fn order(t: Term) -> u32 {
    t.count_ones()
}

/// Drops the terms contained in others and repeats, keeping the first-seen order.
fn reduce(terms: &[Term]) -> Vec<Term> {
    let mut out: Vec<Term> = Vec::new();
    for (i, &t) in terms.iter().enumerate() {
        let covered = terms
            .iter()
            .enumerate()
            .any(|(j, &u)| u != t && u & t == t || u == t && j < i);
        if !covered && t != 0 {
            out.push(t);
        }
    }
    out
}

/// Every non-empty subset of a term of the generating class.
fn implied(generating: &[Term]) -> Vec<Term> {
    let mut out: Vec<Term> = Vec::new();
    for &g in generating {
        let mut s = g;
        while s != 0 {
            if !out.contains(&s) {
                out.push(s);
            }
            s = (s - 1) & g;
        }
    }
    out.sort_by_key(|&t| (order(t), std::cmp::Reverse(t.reverse_bits())));
    out
}

/// The subsets of `t` with exactly `k` factors, in factor order.
fn subsets_of_order(t: Term, k: u32) -> Vec<Term> {
    let mut out: Vec<Term> = (1..=t).filter(|&s| s & t == s && order(s) == k).collect();
    out.sort_by_key(|&s| std::cmp::Reverse(s.reverse_bits()));
    out
}

struct Fit {
    m: Vec<f64>,
    iterations: usize,
    converged: bool,
    max_difference: f64,
    lr: f64,
    pearson: f64,
    /// Unadjusted df.
    df: f64,
    /// Cells fitted as 0.
    zero_fitted: usize,
}

impl Fit {
    fn adjusted_df(&self) -> f64 {
        self.df - self.zero_fitted as f64
    }
}

/// The table, its start values and the fitting criteria.
struct Problem<'a> {
    levels: &'a [usize],
    /// Counts, 0 in structural zeros.
    n: Vec<f64>,
    m0: Vec<f64>,
    eps: f64,
    iterate: u32,
}

impl Problem<'_> {
    fn margin_codes(&self, t: Term) -> (Vec<usize>, usize) {
        let mut size = 1;
        let mut codes = vec![0; self.n.len()];
        let mut stride = 1;
        for f in (0..self.levels.len()).rev() {
            if t & (1 << f) == 0 {
                continue;
            }
            let block: usize = self.levels[f + 1..].iter().product();
            for (i, code) in codes.iter_mut().enumerate() {
                *code += (i / block % self.levels[f]) * stride;
            }
            stride *= self.levels[f];
            size *= self.levels[f];
        }
        (codes, size)
    }

    fn margins(values: &[f64], codes: &[usize], size: usize) -> Vec<f64> {
        let mut out = vec![0.0; size];
        for (v, &c) in values.iter().zip(codes) {
            out[c] += v;
        }
        out
    }

    /// Fits the hierarchical model of a generating class to `n`.
    fn fit(&self, generating: &[Term], n: &[f64]) -> Fit {
        let mut configs: Vec<Term> = vec![0];
        configs.extend_from_slice(generating);
        let prepared: Vec<(Vec<usize>, usize, Vec<f64>)> = configs
            .iter()
            .map(|&t| {
                let (codes, size) = self.margin_codes(t);
                let observed = Self::margins(n, &codes, size);
                (codes, size, observed)
            })
            .collect();
        let mut m = self.m0.clone();
        let (mut iterations, mut converged) = (0, false);
        while iterations < self.iterate as usize {
            let before = m.clone();
            for (codes, size, observed) in &prepared {
                let fitted = Self::margins(&m, codes, *size);
                for (v, &c) in m.iter_mut().zip(codes) {
                    if fitted[c] > 0.0 {
                        *v *= observed[c] / fitted[c];
                    }
                }
            }
            iterations += 1;
            let change = m
                .iter()
                .zip(&before)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            if change < self.eps {
                converged = true;
                break;
            }
        }
        let max_difference = prepared
            .iter()
            .flat_map(|(codes, size, observed)| {
                let fitted = Self::margins(&m, codes, *size);
                observed
                    .iter()
                    .zip(fitted)
                    .map(|(o, f)| (o - f).abs())
                    .collect::<Vec<_>>()
            })
            .fold(0.0, f64::max);
        let (mut lr, mut pearson) = (0.0, 0.0);
        for (&o, &e) in n.iter().zip(&m) {
            if e > 0.0 {
                pearson += (o - e) * (o - e) / e;
                if o > 0.0 {
                    lr += 2.0 * o * (o / e).ln();
                }
            }
        }
        let parameters: usize = 1 + implied(generating)
            .iter()
            .map(|&t| self.term_df(t))
            .sum::<usize>();
        Fit {
            zero_fitted: m.iter().filter(|&&v| v <= 0.0).count(),
            m,
            iterations,
            converged,
            max_difference,
            lr,
            pearson,
            df: n.len() as f64 - parameters as f64,
        }
    }

    fn term_df(&self, t: Term) -> usize {
        (0..self.levels.len())
            .filter(|f| t & (1 << f) != 0)
            .map(|f| self.levels[f] - 1)
            .product()
    }
}

/// The cell weights of a design, one per cell; None without CWEIGHT.
struct CellWeightSums {
    /// Weighted sum of the variable and the weights, plain sum and count, per cell.
    weighted: Vec<f64>,
    weights: Vec<f64>,
    plain: Vec<f64>,
    count: Vec<f64>,
}

impl Engine {
    pub(crate) fn hiloglinear(&mut self, command: &Hiloglinear) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let mut names: Vec<String> = Vec::new();
        let mut bounds: Vec<(f64, f64)> = Vec::new();
        for range in &command.ranges {
            for name in self.expand(&range.variables)? {
                if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    return Err(Error(format!("HILOGLINEAR names {name} more than once")));
                }
                names.push(name);
                bounds.push((range.min, range.max));
            }
        }
        if names.len() < 2 {
            return Err(Error("HILOGLINEAR needs at least two variables".into()));
        }
        if names.len() > 10 {
            return Err(Error("HILOGLINEAR takes at most 10 variables".into()));
        }
        let vars: Vec<_> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "HILOGLINEAR variables are numeric, but {} is a string variable",
                v.name()
            )));
        }
        let levels: Vec<usize> = bounds
            .iter()
            .map(|(lo, hi)| (hi - lo) as usize + 1)
            .collect();
        let cells = levels
            .iter()
            .try_fold(1usize, |a, &l| a.checked_mul(l))
            .filter(|&c| c <= MAX_CELLS)
            .ok_or_else(|| {
                Error(format!(
                    "the HILOGLINEAR table would have more than {MAX_CELLS} cells; narrow the ranges"
                ))
            })?;
        let weights = case_weights(data)?;
        let columns: Vec<_> = vars.iter().map(|v| v.numeric()).collect::<Result<_, _>>()?;
        // the variable of every CWEIGHT subcommand: there is at most one
        let cweight_var = command.designs.iter().find_map(|d| match &d.cweight {
            Some(CellWeights::Variable(name)) => Some(name.clone()),
            _ => None,
        });
        let cweight_column = match &cweight_var {
            Some(name) => {
                let v = data.variable(name)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "the CWEIGHT variable {name} must be numeric"
                    )));
                }
                Some((v.numeric()?, v))
            }
            None => None,
        };
        let mut n = vec![0.0; cells];
        let mut sums = CellWeightSums {
            weighted: vec![0.0; cells],
            weights: vec![0.0; cells],
            plain: vec![0.0; cells],
            count: vec![0.0; cells],
        };
        let (mut valid, mut out_of_range, mut missing, mut weighted) = (0.0, 0.0, 0.0, 0.0);
        'cases: for k in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[k]);
            if w <= 0.0 {
                continue;
            }
            let mut index = 0;
            let mut outside = false;
            for (f, column) in columns.iter().enumerate() {
                let Some(v) = column
                    .get(k)
                    .filter(|v| command.include || !vars[f].is_user_missing(*v))
                else {
                    missing += 1.0;
                    continue 'cases;
                };
                let v = v.trunc();
                let (lo, hi) = bounds[f];
                if v < lo || v > hi {
                    outside = true;
                }
                index = index * levels[f] + (v - lo).max(0.0).min(hi - lo) as usize;
            }
            if outside {
                out_of_range += 1.0;
                continue;
            }
            valid += 1.0;
            weighted += w;
            n[index] += w;
            if let Some((values, v)) = &cweight_column
                && let Some(x) = values.get(k).filter(|x| !v.is_user_missing(*x))
            {
                sums.weighted[index] += w * x;
                sums.weights[index] += w;
                sums.plain[index] += x;
                sums.count[index] += 1.0;
            }
        }
        if weighted <= 0.0 {
            return Err(Error(
                "HILOGLINEAR: no case has valid values within the ranges".into(),
            ));
        }
        let mut tables = Vec::new();
        let mut rows = vec![
            vec![text("Cases"), text("Valid"), Cell::Number(valid)],
            vec![text(""), text("Out of range"), Cell::Number(out_of_range)],
            vec![text(""), text("Missing"), Cell::Number(missing)],
            vec![text(""), text("Weighted valid"), Cell::Number(weighted)],
        ];
        for (f, v) in vars.iter().enumerate() {
            rows.push(vec![
                text(if f == 0 { "Categories" } else { "" }),
                text(shown.variable(v)),
                Cell::Number(levels[f] as f64),
            ]);
        }
        tables.push(table(
            "Data Information",
            2,
            plain(&["", "", "N"]),
            rows,
            None,
        ));
        let mut warnings = Vec::new();
        let mut charts = Vec::new();
        if command.implicit_design {
            warnings.push(
                "HILOGLINEAR: the command does not end with DESIGN, so the default model is fitted as well"
                    .to_string(),
            );
        }
        let value_text: Vec<Vec<String>> = vars
            .iter()
            .enumerate()
            .map(|(f, v)| {
                (0..levels[f])
                    .map(|l| shown.number(v, bounds[f].0 + l as f64))
                    .collect()
            })
            .collect();
        let headers: Vec<String> = vars.iter().map(|v| shown.variable(v)).collect();
        let context = Context {
            names: &names,
            levels: &levels,
            n: &n,
            sums: &sums,
            value_text: &value_text,
            headers: &headers,
        };
        for (d, design) in command.designs.iter().enumerate() {
            let label = (command.designs.len() > 1).then(|| format!("Design {}", d + 1));
            context.design(design, label, &mut tables, &mut charts, &mut warnings)?;
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

struct Context<'a> {
    names: &'a [String],
    levels: &'a [usize],
    n: &'a [f64],
    sums: &'a CellWeightSums,
    value_text: &'a [Vec<String>],
    headers: &'a [String],
}

impl Context<'_> {
    fn label(&self, t: Term) -> String {
        (0..self.names.len())
            .filter(|f| t & (1 << f) != 0)
            .map(|f| self.names[f].as_str())
            .collect::<Vec<_>>()
            .join("*")
    }

    fn class_label(&self, generating: &[Term]) -> String {
        generating
            .iter()
            .map(|&t| self.label(t))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn design(
        &self,
        design: &HiloDesign,
        label: Option<String>,
        tables: &mut Vec<Table>,
        charts: &mut Vec<Chart>,
        warnings: &mut Vec<String>,
    ) -> Result<(), Error> {
        let k = self.names.len();
        let full: Term = (1 << k) - 1;
        let maxorder = design
            .maxorder
            .map_or(k as u32, |m| (m as u32).min(k as u32));
        let mut generating: Vec<Term> = match &design.terms {
            None => subsets_of_order(full, maxorder),
            Some(terms) => {
                let mut out = Vec::new();
                for term in terms {
                    let mut t: Term = 0;
                    for name in term {
                        let Some(f) = self.names.iter().position(|n| n.eq_ignore_ascii_case(name))
                        else {
                            return Err(Error(format!(
                                "HILOGLINEAR DESIGN names {name}, which is not on the variable list"
                            )));
                        };
                        if t & (1 << f) != 0 {
                            return Err(Error(format!(
                                "HILOGLINEAR DESIGN repeats {name} within a term"
                            )));
                        }
                        t |= 1 << f;
                    }
                    if order(t) > maxorder {
                        out.extend(subsets_of_order(t, maxorder));
                    } else {
                        out.push(t);
                    }
                }
                out
            }
        };
        generating = reduce(&generating);
        let cells = self.n.len();
        // cell weights: from the variable (the weighted mean of the cell, 1 without cases) or
        // the list
        let cweights: Option<Vec<f64>> = match &design.cweight {
            None => None,
            Some(CellWeights::Values(values)) => {
                if values.len() != cells {
                    return Err(Error(format!(
                        "CWEIGHT gives {} cell weights, but the table has {cells} cells",
                        values.len()
                    )));
                }
                Some(values.clone())
            }
            Some(CellWeights::Variable(_)) => Some(
                (0..cells)
                    .map(|i| {
                        let s = self.sums;
                        if s.weights[i] > 0.0 {
                            s.weighted[i] / s.weights[i]
                        } else if s.count[i] > 0.0 {
                            s.plain[i] / s.count[i]
                        } else {
                            1.0
                        }
                    })
                    .collect(),
            ),
        };
        let m0: Vec<f64> = match &cweights {
            None => vec![1.0; cells],
            Some(w) => w.iter().map(|&w| w.clamp(0.0, 1.0)).collect(),
        };
        let n: Vec<f64> = self
            .n
            .iter()
            .zip(&m0)
            .map(|(&n, &m)| if m > 0.0 { n } else { 0.0 })
            .collect();
        if self.n.iter().zip(&m0).any(|(&n, &m)| m <= 0.0 && n > 0.0) {
            warnings.push(
                "HILOGLINEAR: cases in cells with a cell weight of 0 are not used".to_string(),
            );
        }
        let largest = n
            .iter()
            .zip(cweights.iter().flatten().chain(std::iter::repeat(&1.0)))
            .map(|(n, w)| n * w)
            .fold(0.0, f64::max);
        let problem = Problem {
            levels: self.levels,
            n: n.clone(),
            m0,
            eps: design
                .criteria
                .converge
                .unwrap_or_else(|| (largest / 1000.0).max(0.25)),
            iterate: design.criteria.iterate,
        };
        let caption_of = |extra: Option<String>| match (&label, extra) {
            (Some(l), Some(e)) => Some(format!("{l}. {e}")),
            (Some(l), None) => Some(l.clone()),
            (None, e) => e,
        };
        if design.backward {
            generating = self.backward(
                &problem,
                generating,
                &design.criteria,
                tables,
                caption_of(None),
            );
        }
        let saturated = generating == [full];
        let delta = if saturated {
            design.criteria.delta
        } else {
            0.0
        };
        let observed: Vec<f64> = n
            .iter()
            .zip(&problem.m0)
            .map(|(&n, &m)| if m > 0.0 { n + delta } else { n })
            .collect();
        let fit = problem.fit(&generating, &observed);
        if !fit.converged {
            warnings.push(format!(
                "HILOGLINEAR: the fit of {} did not converge in {} iterations",
                self.class_label(&generating),
                design.criteria.iterate
            ));
        }
        let class = self.class_label(&generating);
        tables.push(table(
            "Convergence Information",
            1,
            plain(&["", "Value"]),
            vec![
                vec![text("Generating class"), text(class.clone())],
                vec![
                    text("Number of iterations"),
                    Cell::Number(fit.iterations as f64),
                ],
                vec![
                    text("Largest difference between observed and fitted margins"),
                    Cell::Number(fit.max_difference),
                ],
                vec![text("Convergence criterion"), Cell::Number(problem.eps)],
            ],
            caption_of(None),
        ));
        let print = design.print.unwrap_or(HiloPrint {
            freq: true,
            resid: true,
            estim: saturated,
            association: saturated,
        });
        let standardized: Vec<Option<f64>> = observed
            .iter()
            .zip(&fit.m)
            .map(|(&o, &e)| (e > 0.0).then(|| (o - e) / e.sqrt()))
            .collect();
        if print.freq || print.resid {
            let total_o: f64 = observed.iter().sum();
            let total_e: f64 = fit.m.iter().sum();
            let mut columns: Vec<(String, String)> = self
                .headers
                .iter()
                .map(|h| (String::new(), h.clone()))
                .collect();
            if print.freq {
                for g in ["Observed", "Expected"] {
                    columns.push((g.into(), "Count".into()));
                    columns.push((g.into(), "%".into()));
                }
            }
            if print.resid {
                columns.push((String::new(), "Residual".into()));
                columns.push((String::new(), "Standardized residual".into()));
            }
            let rows = (0..observed.len())
                .map(|i| {
                    let lv = cell_levels(i, self.levels);
                    let mut row: Vec<Cell> = lv
                        .iter()
                        .enumerate()
                        .map(|(f, &l)| text(self.value_text[f][l].clone()))
                        .collect();
                    if print.freq {
                        row.push(Cell::Number(observed[i]));
                        row.push(number(
                            (total_o > 0.0).then(|| 100.0 * observed[i] / total_o),
                        ));
                        row.push(Cell::Number(fit.m[i]));
                        row.push(number((total_e > 0.0).then(|| 100.0 * fit.m[i] / total_e)));
                    }
                    if print.resid {
                        row.push(number((fit.m[i] > 0.0).then(|| observed[i] - fit.m[i])));
                        row.push(number(standardized[i]));
                    }
                    row
                })
                .collect();
            let note = (saturated && delta > 0.0).then(|| {
                format!("The saturated model is fitted to the counts plus {delta}, as shown.")
            });
            tables.push(table(
                "Cell Counts and Residuals",
                k,
                columns,
                rows,
                caption_of(note),
            ));
        }
        let adjusted = fit.adjusted_df();
        let sig = |chi: f64| number((adjusted > 0.0).then(|| chi_square_p(chi, adjusted)));
        let note = (fit.zero_fitted > 0).then(|| {
            format!(
                "{} cell(s) are fitted as 0; df is adjusted for them (unadjusted df {}).",
                fit.zero_fitted, fit.df
            )
        });
        tables.push(table(
            "Goodness-of-Fit Tests",
            1,
            plain(&["", "Chi-Square", "df", "Sig."]),
            vec![
                vec![
                    text("Likelihood Ratio"),
                    Cell::Number(fit.lr),
                    Cell::Number(adjusted),
                    sig(fit.lr),
                ],
                vec![
                    text("Pearson"),
                    Cell::Number(fit.pearson),
                    Cell::Number(adjusted),
                    sig(fit.pearson),
                ],
            ],
            caption_of(note),
        ));
        if saturated {
            tables.push(self.k_way(&problem, caption_of(None)));
        }
        if (design.print.is_some_and(|p| p.estim || p.association)) && !saturated {
            warnings.push(
                "HILOGLINEAR: parameter estimates and partial associations need the saturated model"
                    .to_string(),
            );
        }
        if saturated && print.association {
            tables.push(self.partial_associations(&problem, caption_of(None)));
        }
        if saturated && print.estim {
            let positive = observed
                .iter()
                .zip(&problem.m0)
                .all(|(&o, &m)| o > 0.0 && m > 0.0);
            if positive {
                tables.push(self.estimates(&observed, caption_of(None)));
            } else {
                warnings.push(
                    "HILOGLINEAR: no parameter estimates, because a cell is 0 after DELTA or a structural zero"
                        .to_string(),
                );
            }
        }
        if !saturated && (design.plot_resid || design.plot_normal) {
            let suffix = label.map_or(String::new(), |l| format!(" ({l})"));
            let name = format!("HILOGLINEAR{suffix}");
            if design.plot_resid {
                let points: Vec<(f64, f64, f64)> = (0..cells)
                    .filter_map(|i| standardized[i].map(|r| (observed[i], fit.m[i], r)))
                    .collect();
                charts.extend(residual_by_counts(&name, "standardized residual", &points));
            }
            if design.plot_normal {
                let adjusted = self.adjusted_residuals(&generating, &observed, &fit.m);
                charts.extend(normal_plots(&name, "adjusted residual", &adjusted, false));
            }
        }
        Ok(())
    }

    /// Adjusted residuals of a hierarchical model: the indicator design of every term in it.
    fn adjusted_residuals(&self, generating: &[Term], observed: &[f64], m: &[f64]) -> Vec<f64> {
        let terms = implied(generating);
        let x: Vec<Vec<f64>> = (0..m.len())
            .map(|i| {
                let lv = cell_levels(i, self.levels);
                let mut row = vec![1.0];
                for &t in &terms {
                    let factors: Vec<usize> = (0..self.levels.len())
                        .filter(|f| t & (1 << f) != 0)
                        .collect();
                    let size: usize = factors.iter().map(|&f| self.levels[f]).product();
                    let at = factors.iter().fold(0, |a, &f| a * self.levels[f] + lv[f]);
                    row.extend((0..size).map(|c| f64::from(u8::from(c == at))));
                }
                row
            })
            .collect();
        let used = vec![true; m.len()];
        let h = hat_values(&x, m, &used);
        (0..m.len())
            .map(|i| {
                let v = m[i] * (1.0 - h[i]);
                if m[i] > 0.0 && v > 1e-12 * m[i] {
                    (observed[i] - m[i]) / v.sqrt()
                } else {
                    f64::NAN
                }
            })
            .collect()
    }

    fn backward(
        &self,
        problem: &Problem,
        mut generating: Vec<Term>,
        criteria: &statscraft_syntax::HiloCriteria,
        tables: &mut Vec<Table>,
        caption: Option<String>,
    ) -> Vec<Term> {
        let n = &problem.n;
        let mut rows = Vec::new();
        let mut current = problem.fit(&generating, n);
        let mut step = 0;
        loop {
            rows.push(vec![
                text(step.to_string()),
                text("Generating class"),
                text(self.class_label(&generating)),
                Cell::Number(current.lr),
                Cell::Number(current.df),
                number((current.df > 0.0).then(|| chi_square_p(current.lr, current.df))),
                Cell::Number(current.iterations as f64),
            ]);
            if step >= criteria.maxsteps as usize {
                break;
            }
            let mut best: Option<(usize, f64, Vec<Term>, Fit)> = None;
            for (i, &t) in generating.iter().enumerate() {
                let mut reduced: Vec<Term> =
                    generating.iter().copied().filter(|&u| u != t).collect();
                reduced.extend(subsets_of_order(t, order(t) - 1));
                let reduced = reduce(&reduced);
                let fit = problem.fit(&reduced, n);
                let change = (fit.lr - current.lr).max(0.0);
                let df = fit.df - current.df;
                let p = if change <= 1e-10 {
                    1.0
                } else {
                    chi_square_p(change, df)
                };
                rows.push(vec![
                    text(""),
                    text(format!("Deleted effect {}", i + 1)),
                    text(self.label(t)),
                    Cell::Number(change),
                    Cell::Number(df),
                    Cell::Number(p),
                    Cell::Number(fit.iterations as f64),
                ]);
                if best.as_ref().is_none_or(|b| p > b.1) {
                    best = Some((i, p, reduced, fit));
                }
            }
            match best {
                Some((_, p, reduced, fit)) if p > criteria.p => {
                    generating = reduced;
                    current = fit;
                    step += 1;
                    if generating.is_empty() {
                        rows.push(vec![
                            text(step.to_string()),
                            text("Generating class"),
                            text("(constant only)"),
                            Cell::Number(current.lr),
                            Cell::Number(current.df),
                            number(
                                (current.df > 0.0).then(|| chi_square_p(current.lr, current.df)),
                            ),
                            Cell::Number(current.iterations as f64),
                        ]);
                        break;
                    }
                }
                _ => break,
            }
        }
        let note = format!(
            "Each step drops the term whose removal raises the likelihood-ratio chi-square with the largest p, while that p is above {}.",
            criteria.p
        );
        let caption = Some(match caption {
            Some(c) => format!("{c}. {note}"),
            None => note,
        });
        tables.push(table(
            "Step Summary",
            3,
            plain(&[
                "Step",
                "",
                "Effects",
                "Chi-Square",
                "df",
                "Sig.",
                "Number of Iterations",
            ]),
            rows,
            caption,
        ));
        generating
    }

    fn k_way(&self, problem: &Problem, caption: Option<String>) -> Table {
        let k = self.names.len() as u32;
        let full: Term = (1 << k) - 1;
        let all = implied(&[full]);
        let higher: Vec<(f64, Fit)> = (1..=k)
            .map(|order_k| {
                let model = subsets_of_order(full, order_k - 1);
                let model: Vec<Term> = model.into_iter().filter(|&t| t != 0).collect();
                let df: usize = all
                    .iter()
                    .filter(|&&t| order(t) >= order_k)
                    .map(|&t| problem.term_df(t))
                    .sum();
                (df as f64, problem.fit(&model, &problem.n))
            })
            .collect();
        let mut rows = Vec::new();
        for (i, (df, fit)) in higher.iter().enumerate() {
            rows.push(vec![
                text(if i == 0 {
                    "K-way and higher-order effects"
                } else {
                    ""
                }),
                Cell::Number(i as f64 + 1.0),
                Cell::Number(*df),
                Cell::Number(fit.lr),
                number((*df > 0.0).then(|| chi_square_p(fit.lr, *df))),
                Cell::Number(fit.pearson),
                number((*df > 0.0).then(|| chi_square_p(fit.pearson, *df))),
                Cell::Number(fit.iterations as f64),
            ]);
        }
        for i in 0..higher.len() {
            let (df, fit) = &higher[i];
            let (df_next, lr_next, pearson_next) = higher
                .get(i + 1)
                .map_or((0.0, 0.0, 0.0), |(d, f)| (*d, f.lr, f.pearson));
            let df = df - df_next;
            let lr = fit.lr - lr_next;
            let pearson = fit.pearson - pearson_next;
            rows.push(vec![
                text(if i == 0 { "K-way effects" } else { "" }),
                Cell::Number(i as f64 + 1.0),
                Cell::Number(df),
                Cell::Number(lr),
                number((df > 0.0).then(|| chi_square_p(lr.max(0.0), df))),
                Cell::Number(pearson),
                number((df > 0.0).then(|| chi_square_p(pearson.max(0.0), df))),
                Cell::Empty,
            ]);
        }
        table(
            "K-Way and Higher-Order Effects",
            2,
            vec![
                (String::new(), String::new()),
                (String::new(), "K".into()),
                (String::new(), "df".into()),
                ("Likelihood Ratio".into(), "Chi-Square".into()),
                ("Likelihood Ratio".into(), "Sig.".into()),
                ("Pearson".into(), "Chi-Square".into()),
                ("Pearson".into(), "Sig.".into()),
                (String::new(), "Number of Iterations".into()),
            ],
            rows,
            caption,
        )
    }

    fn partial_associations(&self, problem: &Problem, caption: Option<String>) -> Table {
        let k = self.names.len() as u32;
        let full: Term = (1 << k) - 1;
        let mut rows = Vec::new();
        for order_k in (1..k).rev() {
            let all = subsets_of_order(full, order_k);
            let with_all = problem.fit(&all, &problem.n);
            for &t in &all {
                let without: Vec<Term> = all.iter().copied().filter(|&u| u != t).collect();
                let fit = problem.fit(&without, &problem.n);
                let df = problem.term_df(t) as f64;
                let chi = (fit.lr - with_all.lr).max(0.0);
                rows.push(vec![
                    text(self.label(t)),
                    Cell::Number(df),
                    Cell::Number(chi),
                    number((df > 0.0).then(|| chi_square_p(chi, df))),
                    Cell::Number(fit.iterations as f64),
                ]);
            }
        }
        table(
            "Partial Associations",
            1,
            plain(&[
                "Effect",
                "df",
                "Partial Chi-Square",
                "Sig.",
                "Number of Iterations",
            ]),
            rows,
            caption,
        )
    }

    /// Effect-coded parameters of the saturated model from log(count + DELTA).
    fn estimates(&self, observed: &[f64], caption: Option<String>) -> Table {
        let k = self.names.len() as u32;
        let full: Term = (1 << k) - 1;
        let cells = observed.len() as f64;
        let logs: Vec<f64> = observed.iter().map(|o| o.ln()).collect();
        let z = normal_quantile(0.975);
        let mut rows = Vec::new();
        for order_k in (1..=k).rev() {
            for t in subsets_of_order(full, order_k) {
                let factors: Vec<usize> = (0..self.levels.len())
                    .filter(|f| t & (1 << f) != 0)
                    .collect();
                let reduced: Vec<usize> = factors.iter().map(|&f| self.levels[f] - 1).collect();
                let count: usize = reduced.iter().product();
                for (p, combo) in (0..count).map(|c| (c, cell_levels(c, &reduced))) {
                    let (mut estimate, mut variance) = (0.0, 0.0);
                    for i in 0..observed.len() {
                        let lv = cell_levels(i, self.levels);
                        let mut c = 1.0 / cells;
                        for (j, &f) in factors.iter().enumerate() {
                            c *= if lv[f] == combo[j] {
                                (self.levels[f] - 1) as f64
                            } else {
                                -1.0
                            };
                        }
                        estimate += c * logs[i];
                        variance += c * c / observed[i];
                    }
                    let se = variance.sqrt();
                    let zvalue = estimate / se;
                    rows.push(vec![
                        text(if p == 0 { self.label(t) } else { String::new() }),
                        Cell::Number(p as f64 + 1.0),
                        Cell::Number(estimate),
                        Cell::Number(se),
                        Cell::Number(zvalue),
                        Cell::Number(normal_p(zvalue)),
                        Cell::Number(estimate - z * se),
                        Cell::Number(estimate + z * se),
                    ]);
                }
            }
        }
        table(
            "Parameter Estimates",
            2,
            vec![
                (String::new(), "Effect".into()),
                (String::new(), "Parameter".into()),
                (String::new(), "Estimate".into()),
                (String::new(), "Std. Error".into()),
                (String::new(), "Z".into()),
                (String::new(), "Sig.".into()),
                ("95% Confidence Interval".into(), "Lower Bound".into()),
                ("95% Confidence Interval".into(), "Upper Bound".into()),
            ],
            rows,
            caption,
        )
    }
}
