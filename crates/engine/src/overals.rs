//! OVERALS: nonlinear canonical correlation analysis of two or more sets of variables.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - The variables are read as categories 1 to the largest value named on VARIABLES (see
//!   `legacy_scaling`). An object (case) with a missing value on any variable of a set leaves
//!   that whole set out; an object left out of every set takes no part. Each set needs at
//!   least three objects.
//! - Each variable is quantified at its level: MNOM gives a free point to every category in
//!   every dimension; SNOM, ORDI and NUME give one value per category (any values, values in
//!   the order of the codes, or values in equal steps) times a weight per dimension.
//! - The fit makes every set's sum of quantified variables as close as possible to the object
//!   scores, summed over the sets: alternating least squares over the variables of each set
//!   (with the other variables of the set held fixed), then the objects as the mean of what
//!   their sets give, centered and made orthonormal with sum of squares K times n in the
//!   metric of the number of sets each object takes part in. Case weights count as
//!   replications.
//! - The default start fits every single variable as numerical first and continues from that
//!   solution; RANDOM starts from random objects. The fit stops when it gains less than the
//!   criterion; the solution is then turned to its principal axes.
//! - Loss per set and dimension is the weighted sum of squares of the objects minus the set's
//!   sum, over n; the eigenvalue of a dimension is 1 minus the mean loss, the fit their sum.
//!   Multiple fit is the weighted sum of squares of the free category points over n, single
//!   fit the squared weights, single loss their difference. Loadings are the inner products of
//!   the quantified variable and the objects over n; centroids are the mean objects of each
//!   category, projected centroids the quantifications times the loadings.

use crate::legacy_scaling::{MatrixRow, vartype};
use crate::matrix::Matrix;
use crate::npar_tests::text;
use crate::optimal_scaling::{
    Coded, cut, eigen_sorted, free_set, num, object_groups, orthonormalize, pseudo_random_start,
    scatter, transformation_chart,
};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::Measure;
use statscraft_syntax::{LegacyLevel, Overals, ScalingCommon, VarItem};

const WHO: &str = "OVERALS";

/// One analysis variable during the fit.
struct Var {
    /// Index into the prepared analysis variables.
    j: usize,
    set: usize,
    level: LegacyLevel,
    /// Per object: its category, None when the object leaves this variable's set out.
    h: Vec<Option<usize>>,
    /// Weighted count of each category.
    d: Vec<f64>,
    codes: Vec<f64>,
    /// Single levels: the category values and the weight per dimension.
    y: Vec<f64>,
    a: Vec<f64>,
    /// The current category points (y times a for single levels).
    points: Matrix,
    /// The free category points of the last update.
    free: Matrix,
}

impl Var {
    fn single(&self) -> bool {
        self.level != LegacyLevel::MultipleNominal
    }

    /// The codes centered and scaled with the category weights to sum of squares `n_w`.
    fn numeric_values(&self, n_w: f64) -> Vec<f64> {
        let total: f64 = self.d.iter().sum();
        let mean = self
            .codes
            .iter()
            .zip(&self.d)
            .map(|(c, d)| c * d)
            .sum::<f64>()
            / total;
        let ss: f64 = (self.codes.iter().zip(&self.d))
            .map(|(c, d)| d * (c - mean).powi(2))
            .sum();
        (self.codes.iter().zip(&self.d))
            .map(|(c, d)| match *d > 0.0 && ss > 0.0 {
                true => (c - mean) * (n_w / ss).sqrt(),
                false => 0.0,
            })
            .collect()
    }

    /// The free category points: the weighted mean of `target` over each category.
    fn free_points(&self, target: &Matrix, w: &[f64], p: usize) -> Matrix {
        let mut out = vec![vec![0.0; p]; self.d.len()];
        for (i, h) in self.h.iter().enumerate() {
            if let Some(r) = h {
                for s in 0..p {
                    out[*r][s] += w[i] * target[i][s];
                }
            }
        }
        for (r, row) in out.iter_mut().enumerate() {
            row.iter_mut()
                .for_each(|e| *e = if self.d[r] > 0.0 { *e / self.d[r] } else { 0.0 });
        }
        out
    }

    /// One update from the free points: kept as they are for MNOM, else a rank-one fit.
    fn update(&mut self, free: Matrix, as_numeric: bool, n_w: f64) {
        let p = free.first().map_or(0, Vec::len);
        if !self.single() {
            self.points = free.clone();
            self.free = free;
            return;
        }
        let dy: f64 = self.y.iter().zip(&self.d).map(|(y, d)| d * y * y).sum();
        self.a = (0..p)
            .map(|s| {
                (free.iter().zip(&self.y).zip(&self.d))
                    .map(|((f, y), d)| d * y * f[s])
                    .sum::<f64>()
                    / dy
            })
            .collect();
        if !as_numeric && self.level != LegacyLevel::Numerical {
            let mut y: Vec<f64> = free
                .iter()
                .map(|f| f.iter().zip(&self.a).map(|(f, a)| f * a).sum())
                .collect();
            if self.level == LegacyLevel::Ordinal {
                let keep: Vec<usize> = (0..y.len()).filter(|&r| self.d[r] > 0.0).collect();
                let values: Vec<f64> = keep.iter().map(|&r| y[r]).collect();
                let weights: Vec<f64> = keep.iter().map(|&r| self.d[r]).collect();
                for (&r, v) in keep.iter().zip(crate::mds::monotone(&values, &weights)) {
                    y[r] = v;
                }
            }
            let ss: f64 = y.iter().zip(&self.d).map(|(y, d)| d * y * y).sum();
            if ss > 1e-12 * n_w {
                self.y = y.iter().map(|v| v * (n_w / ss).sqrt()).collect();
            }
        }
        self.points = self
            .y
            .iter()
            .map(|y| self.a.iter().map(|a| y * a).collect())
            .collect();
        self.free = free;
    }
}

/// The sum of the category points of `vars` for every object.
fn set_sum(vars: &[&Var], n: usize, p: usize) -> Matrix {
    let mut out = vec![vec![0.0; p]; n];
    for v in vars {
        for (i, h) in v.h.iter().enumerate() {
            if let Some(r) = h {
                out[i]
                    .iter_mut()
                    .zip(&v.points[*r])
                    .for_each(|(o, c)| *o += c);
            }
        }
    }
    out
}

struct State {
    vars: Vec<Var>,
    sets: usize,
    /// Per object: the case weight and the number of sets it takes part in.
    w: Vec<f64>,
    mstar: Vec<f64>,
    n_w: f64,
    p: usize,
}

impl State {
    /// One sweep over the sets with the objects `x`: new category points, then the objects'
    /// target, the mean of their sets' sums. Returns the target and each set's sum.
    fn sweep(&mut self, x: &Matrix, as_numeric: bool) -> (Matrix, Vec<Matrix>) {
        let (n, p) = (x.len(), self.p);
        let mut target = vec![vec![0.0; p]; n];
        let mut sums = Vec::new();
        for k in 0..self.sets {
            let members: Vec<usize> = (0..self.vars.len())
                .filter(|&j| self.vars[j].set == k)
                .collect();
            let mut v = set_sum(
                &members.iter().map(|&j| &self.vars[j]).collect::<Vec<_>>(),
                n,
                p,
            );
            for &j in &members {
                let var = &self.vars[j];
                // the objects without this variable's contribution to the set
                let mut rest = x.clone();
                for (i, h) in var.h.iter().enumerate() {
                    if let Some(r) = h {
                        for s in 0..p {
                            rest[i][s] -= v[i][s] - var.points[*r][s];
                        }
                    }
                }
                let free = var.free_points(&rest, &self.w, p);
                let old = var.points.clone();
                self.vars[j].update(free, as_numeric, self.n_w);
                let var = &self.vars[j];
                for (i, h) in var.h.iter().enumerate() {
                    if let Some(r) = h {
                        for s in 0..p {
                            v[i][s] += var.points[*r][s] - old[*r][s];
                        }
                    }
                }
            }
            for (t, row) in target.iter_mut().zip(&v) {
                t.iter_mut().zip(row).for_each(|(a, b)| *a += b);
            }
            sums.push(v);
        }
        for (row, m) in target.iter_mut().zip(&self.mstar) {
            row.iter_mut().for_each(|e| *e /= m);
        }
        (target, sums)
    }

    /// Whether object i takes part in set k.
    fn in_set(&self, k: usize, i: usize) -> bool {
        self.vars
            .iter()
            .find(|v| v.set == k)
            .is_some_and(|v| v.h[i].is_some())
    }

    /// Loss of each set in each dimension over n.
    fn losses(&self, x: &Matrix, sums: &[Matrix]) -> Matrix {
        (0..self.sets)
            .map(|k| {
                (0..self.p)
                    .map(|s| {
                        (0..x.len())
                            .filter(|&i| self.in_set(k, i))
                            .map(|i| self.w[i] * (x[i][s] - sums[k][i][s]).powi(2))
                            .sum::<f64>()
                            / self.n_w
                    })
                    .collect()
            })
            .collect()
    }

    fn fit(&self, losses: &Matrix) -> f64 {
        (0..self.p)
            .map(|s| 1.0 - losses.iter().map(|l| l[s]).sum::<f64>() / self.sets as f64)
            .sum()
    }

    /// Alternating least squares from `x`; returns the objects turned to principal axes, the
    /// fit history and whether it converged.
    fn iterate(
        &mut self,
        mut x: Matrix,
        as_numeric: bool,
        max_iterations: usize,
        criterion: f64,
    ) -> Result<(Matrix, Vec<f64>, bool), Error> {
        let p = self.p;
        let mw: Vec<f64> = self.w.iter().zip(&self.mstar).map(|(a, b)| a * b).collect();
        let scale = self.sets as f64 * self.n_w;
        let mut history: Vec<f64> = Vec::new();
        let mut converged = false;
        let mut turn = vec![vec![0.0; p]; p];
        for _ in 0..max_iterations.max(1) {
            let (mut z, sums) = self.sweep(&x, as_numeric);
            // centered, then the nearest orthonormal matrix z L diag(1/l) L'; the last L turns
            // the solution to its principal axes
            let total: f64 = mw.iter().sum();
            for s in 0..p {
                let mean = z.iter().zip(&mw).map(|(r, m)| m * r[s]).sum::<f64>() / total;
                z.iter_mut().for_each(|r| r[s] -= mean);
            }
            let cross: Matrix = (0..p)
                .map(|a| {
                    (0..p)
                        .map(|b| z.iter().zip(&mw).map(|(r, m)| m * r[a] * r[b]).sum())
                        .collect()
                })
                .collect();
            let (values, l) = eigen_sorted(&cross);
            if values
                .iter()
                .any(|&e| e <= 1e-12 * values[0].abs().max(1e-300))
            {
                return Err(Error(format!(
                    "{WHO}: the data support fewer than {p} dimensions; lower DIMENSION"
                )));
            }
            let t: Matrix = (0..p)
                .map(|a| {
                    (0..p)
                        .map(|b| {
                            (0..p)
                                .map(|k| l[a][k] * l[b][k] * (scale / values[k]).sqrt())
                                .sum()
                        })
                        .collect()
                })
                .collect();
            x = z
                .iter()
                .map(|r| {
                    (0..p)
                        .map(|b| (0..p).map(|a| r[a] * t[a][b]).sum())
                        .collect()
                })
                .collect();
            turn = l;
            let fit = self.fit(&self.losses(&x, &sums));
            let gain = history.last().map(|f| fit - f);
            history.push(fit);
            if gain.is_some_and(|g| g.abs() < criterion) {
                converged = true;
                break;
            }
        }
        // principal axes: objects and category points turned by the last L
        let rotate = |m: &mut Matrix| {
            for row in m.iter_mut() {
                *row = (0..p)
                    .map(|b| (0..p).map(|a| row[a] * turn[a][b]).sum())
                    .collect();
            }
        };
        rotate(&mut x);
        for v in &mut self.vars {
            rotate(&mut v.points);
            rotate(&mut v.free);
            if v.single() {
                let mut a = vec![v.a.clone()];
                rotate(&mut a);
                v.a = a.remove(0);
            }
        }
        Ok((x, history, converged))
    }
}

fn level_name(level: LegacyLevel) -> &'static str {
    match level {
        LegacyLevel::MultipleNominal => "Multiple Nominal",
        LegacyLevel::SingleNominal => "Single Nominal",
        LegacyLevel::Ordinal => "Ordinal",
        LegacyLevel::Numerical => "Numerical",
    }
}

fn dims(p: usize) -> Vec<String> {
    (1..=p).map(|s| format!("Dimension {s}")).collect()
}

impl Engine {
    pub(crate) fn overals(&mut self, command: &Overals) -> Result<Vec<Table>, Error> {
        let common = ScalingCommon {
            variables: (command.coding.ranges.iter())
                .flat_map(|(items, _)| items.clone())
                .collect(),
            dimension: command.dimension,
            legacy: Some(command.coding.clone()),
            ..ScalingCommon::default()
        };
        let lists: Vec<(&[VarItem], f64)> = (command.analysis.iter())
            .map(|(items, _)| (items.as_slice(), 1.0))
            .collect();
        let prep = self.prepare_scaling(WHO, &common, &lists)?;
        let mut warnings = prep.warnings.clone();
        let (avars, coded) = (&prep.avars, &prep.coded);
        let sets = command.sets.len();
        if command.sets.iter().sum::<usize>() != avars.len() {
            return Err(Error(format!(
                "{WHO}: SETS counts {} variables, but ANALYSIS names {}",
                command.sets.iter().sum::<usize>(),
                avars.len()
            )));
        }
        let set_of: Vec<usize> = (command.sets.iter().enumerate())
            .flat_map(|(k, &m)| std::iter::repeat_n(k, m))
            .collect();
        // objects: those that take part in at least one set
        let in_set = |k: usize, i: usize| {
            (0..avars.len()).all(|j| set_of[j] != k || avars[j].h[i].is_some())
        };
        let objects: Vec<usize> = (0..prep.active.len())
            .filter(|&i| (0..sets).any(|k| in_set(k, i)))
            .collect();
        let w: Vec<f64> = objects.iter().map(|&i| prep.w[i]).collect();
        let n_w: f64 = w.iter().sum();
        let mstar: Vec<f64> = (objects.iter())
            .map(|&i| (0..sets).filter(|&k| in_set(k, i)).count() as f64)
            .collect();
        for k in 0..sets {
            let count = objects.iter().filter(|&&i| in_set(k, i)).count();
            if count < 3 {
                return Err(Error(format!(
                    "{WHO}: set {} has {count} cases without missing values; each set needs at least three",
                    k + 1
                )));
            }
        }
        let mut vars: Vec<Var> = Vec::new();
        for (j, v) in avars.iter().enumerate() {
            let h: Vec<Option<usize>> = (objects.iter())
                .map(|&i| v.h[i].filter(|_| in_set(set_of[j], i)))
                .collect();
            let mut d = vec![0.0; v.cats.len()];
            for (i, r) in h.iter().enumerate() {
                if let Some(r) = r {
                    d[*r] += w[i];
                }
            }
            let mut var = Var {
                j,
                set: set_of[j],
                level: command.analysis[v.list].1,
                h,
                d,
                codes: v.cats.iter().map(|c| *c as f64).collect(),
                y: Vec::new(),
                a: Vec::new(),
                points: Vec::new(),
                free: Vec::new(),
            };
            var.y = var.numeric_values(n_w);
            vars.push(var);
        }

        // dimensions
        let rank = |k: usize| -> usize {
            let members: Vec<&Var> = vars.iter().filter(|v| v.set == k).collect();
            let multiple = members.iter().filter(|v| !v.single());
            let cats: usize = multiple
                .clone()
                .map(|v| v.d.iter().filter(|d| **d > 0.0).count())
                .sum();
            let complete = multiple.filter(|v| v.h.iter().all(Option::is_some)).count();
            (cats + members.iter().filter(|v| v.single()).count()).saturating_sub(complete)
        };
        let ranks: Vec<usize> = (0..sets).map(rank).collect();
        // two sets: the smaller rank; more: the summed ranks, as DIMENSION describes it
        let by_sets = match sets {
            2 => ranks[0].min(ranks[1]),
            _ => ranks.iter().sum(),
        };
        let pmax = by_sets.min(objects.len().saturating_sub(1));
        if pmax < 1 {
            return Err(Error(format!(
                "{WHO}: the data leave no dimension to compute"
            )));
        }
        let mut p = command.dimension;
        if p > pmax {
            warnings.push(format!(
                "{WHO}: DIMENSION is {p}, but these data allow at most {pmax}; {pmax} are computed"
            ));
            p = pmax;
        }
        for v in &mut vars {
            v.a = vec![0.0; p];
            v.points = vec![vec![0.0; p]; v.d.len()];
            v.free = v.points.clone();
        }

        // the fit
        let mut state = State {
            vars,
            sets,
            w,
            mstar,
            n_w,
            p,
        };
        let scale = sets as f64 * n_w;
        let start = match command.random_start {
            false => pseudo_random_start(&state.w, &state.mstar, p, scale),
            true => {
                let mut x: Matrix = (0..objects.len())
                    .map(|_| {
                        (0..p)
                            .map(|_| self.random.borrow_mut().uniform() - 0.5)
                            .collect()
                    })
                    .collect();
                let mw: Vec<f64> = (state.w.iter().zip(&state.mstar))
                    .map(|(a, b)| a * b)
                    .collect();
                orthonormalize(&mut x, &mw, scale).map(|_| x)
            }
        };
        let Some(mut x) = start else {
            return Err(Error(format!(
                "{WHO}: the data support fewer than {p} dimensions; lower DIMENSION"
            )));
        };
        let (maxit, eps) = (command.max_iterations, command.convergence);
        let restricted = state
            .vars
            .iter()
            .any(|v| matches!(v.level, LegacyLevel::SingleNominal | LegacyLevel::Ordinal));
        if !command.random_start && restricted {
            // the nested start: every single variable numerical first
            x = state.iterate(x, true, maxit, eps)?.0;
        }
        let (mut x, history, converged) = state.iterate(x, false, maxit, eps)?;
        if !converged {
            warnings.push(format!(
                "{WHO}: the fit did not converge within {maxit} iterations"
            ));
        }
        // the object with the largest absolute score is positive in every dimension
        for s in 0..p {
            let big = x.iter().map(|r| r[s].abs()).fold(0.0, f64::max);
            if let Some(r) = x.iter().find(|r| r[s].abs() >= big * (1.0 - 1e-9))
                && r[s] < 0.0
            {
                x.iter_mut().for_each(|r| r[s] = -r[s]);
                for v in &mut state.vars {
                    v.points.iter_mut().for_each(|r| r[s] = -r[s]);
                    if v.single() {
                        v.a[s] = -v.a[s];
                    }
                }
            }
        }
        let n = x.len();
        // the free points and losses at the final objects
        let sums: Vec<Matrix> = (0..sets)
            .map(|k| {
                set_sum(
                    &state.vars.iter().filter(|v| v.set == k).collect::<Vec<_>>(),
                    n,
                    p,
                )
            })
            .collect();
        for v in &mut state.vars {
            let target: Matrix = (0..n)
                .map(|i| match v.h[i] {
                    Some(r) => (0..p)
                        .map(|s| x[i][s] - sums[v.set][i][s] + v.points[r][s])
                        .collect(),
                    None => vec![0.0; p],
                })
                .collect();
            v.free = v.free_points(&target, &state.w, p);
        }
        let losses = state.losses(&x, &sums);
        let vars = &state.vars;
        let mean_loss: Vec<f64> = (0..p)
            .map(|s| losses.iter().map(|l| l[s]).sum::<f64>() / sets as f64)
            .collect();
        let eigen: Vec<f64> = mean_loss.iter().map(|l| 1.0 - l).collect();
        let centroids: Vec<Matrix> = vars
            .iter()
            .map(|v| v.free_points(&x, &state.w, p))
            .collect();
        let loadings: Vec<Vec<f64>> = vars
            .iter()
            .map(|v| match v.single() {
                true => (0..p)
                    .map(|s| {
                        (0..n)
                            .filter_map(|i| v.h[i].map(|r| state.w[i] * v.y[r] * x[i][s]))
                            .sum::<f64>()
                            / n_w
                    })
                    .collect(),
                false => Vec::new(),
            })
            .collect();
        let multiple_fit: Vec<Vec<f64>> = vars
            .iter()
            .map(|v| {
                (0..p)
                    .map(|s| {
                        v.free
                            .iter()
                            .zip(&v.d)
                            .map(|(f, d)| d * f[s] * f[s])
                            .sum::<f64>()
                            / n_w
                    })
                    .collect()
            })
            .collect();
        let coded_of = |v: &Var| -> &Coded { &coded[avars[v.j].coded] };
        let name_of = |v: &Var| coded_of(v).title.clone();
        let print = |k: &str| command.print.contains(&k);
        let with_sum = |row: &[f64]| -> Vec<Cell> {
            let mut out: Vec<Cell> = row.iter().map(|x| num(*x)).collect();
            out.push(num(row.iter().sum()));
            out
        };

        // tables
        let largest =
            crate::legacy_scaling::largest_categories(WHO, self.active()?, &command.coding.ranges)?;
        let mut tables = Vec::new();
        let mut columns = vec![
            "Set".to_string(),
            "Variable".into(),
            "Optimal Scaling Level".into(),
        ];
        columns.push("Categories".into());
        tables.push(Table {
            title: "Variables by Set".into(),
            columns,
            rows: vars
                .iter()
                .map(|v| {
                    let max = largest[&coded_of(v).name.to_uppercase()];
                    vec![
                        Cell::Number((v.set + 1) as f64),
                        text(name_of(v)),
                        text(level_name(v.level)),
                        Cell::Number(max as f64),
                    ]
                })
                .collect(),
            row_headers: 2,
            ..Table::default()
        });
        if print("FREQ") {
            for v in vars {
                let c = coded_of(v);
                let mut rows: Vec<Vec<Cell>> = (avars[v.j].cats.iter().zip(&v.d))
                    .map(|(code, d)| vec![text(c.label(*code, None)), Cell::Number(*d)])
                    .collect();
                let valid: f64 = v.d.iter().sum();
                rows.push(vec![text("Missing"), Cell::Number(n_w - valid)]);
                rows.push(vec![text("Total"), Cell::Number(n_w)]);
                tables.push(Table {
                    title: format!("Marginal Frequencies: {}", name_of(v)),
                    columns: vec![String::new(), "Frequency".into()],
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
        }
        if print("HISTORY") {
            tables.push(Table {
                title: "Iteration History".into(),
                columns: vec!["Iteration".into(), "Fit".into(), "Difference".into()],
                rows: (history.iter().enumerate())
                    .map(|(t, f)| {
                        vec![
                            Cell::Number((t + 1) as f64),
                            num(*f),
                            match t {
                                0 => Cell::Empty,
                                _ => num(f - history[t - 1]),
                            },
                        ]
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        let mut columns = vec![String::new()];
        columns.extend(dims(p));
        columns.push("Sum".into());
        let mut rows: Vec<Vec<Cell>> = (losses.iter().enumerate())
            .map(|(k, l)| {
                let mut row = vec![text(format!("Loss Set {}", k + 1))];
                row.extend(with_sum(l));
                row
            })
            .collect();
        let mut row = vec![text("Mean Loss")];
        row.extend(with_sum(&mean_loss));
        rows.push(row);
        let mut row = vec![text("Eigenvalue")];
        row.extend(with_sum(&eigen));
        rows.push(row);
        tables.push(Table {
            title: "Summary of Analysis".into(),
            columns,
            rows,
            row_headers: 1,
            caption: Some(format!(
                "The fit is the sum of the eigenvalues; {} iterations{}.",
                history.len(),
                if converged { "" } else { ", not converged" }
            )),
            ..Table::default()
        });
        let singles: Vec<&Var> = vars.iter().filter(|v| v.single()).collect();
        let per_variable = |title: &str, values: &dyn Fn(&Var) -> Vec<f64>| {
            let mut columns = vec!["Set".to_string(), "Variable".into()];
            columns.extend(dims(p));
            Table {
                title: title.into(),
                columns,
                rows: singles
                    .iter()
                    .map(|v| {
                        let mut row = vec![Cell::Number((v.set + 1) as f64), text(name_of(v))];
                        row.extend(values(v).iter().map(|x| num(*x)));
                        row
                    })
                    .collect(),
                row_headers: 2,
                ..Table::default()
            }
        };
        let index = |v: &Var| vars.iter().position(|u| u.j == v.j).expect("a variable");
        if print("WEIGHTS") && !singles.is_empty() {
            tables.push(per_variable("Weights", &|v| v.a.clone()));
            tables.push(per_variable("Component Loadings", &|v| {
                loadings[index(v)].clone()
            }));
        }
        if print("FIT") {
            let mut columns = vec!["Set".to_string(), "Variable".into()];
            let mut groups = vec![String::new(), String::new()];
            for what in ["Multiple Fit", "Single Fit", "Single Loss"] {
                for d in dims(p).into_iter().chain(["Sum".to_string()]) {
                    columns.push(d);
                    groups.push(what.to_string());
                }
            }
            let rows = vars
                .iter()
                .enumerate()
                .map(|(j, v)| {
                    let mut row = vec![Cell::Number((v.set + 1) as f64), text(name_of(v))];
                    row.extend(with_sum(&multiple_fit[j]));
                    match v.single() {
                        true => {
                            let single: Vec<f64> = v.a.iter().map(|a| a * a).collect();
                            let loss: Vec<f64> =
                                (0..p).map(|s| multiple_fit[j][s] - single[s]).collect();
                            row.extend(with_sum(&single));
                            row.extend(with_sum(&loss));
                        }
                        false => row.extend(vec![Cell::Empty; 2 * (p + 1)]),
                    }
                    row
                })
                .collect();
            tables.push(Table {
                title: "Fit".into(),
                columns,
                column_groups: groups.into_iter().map(|g| vec![g]).collect(),
                rows,
                row_headers: 2,
                ..Table::default()
            });
        }
        // per variable: categories with their frequency and the asked columns
        let category_table = |title: String, v: &Var, parts: &[(&str, Matrix)]| {
            let c = coded_of(v);
            let mut columns = vec![String::new(), "Marginal Frequency".to_string()];
            let mut groups = vec![String::new(), String::new()];
            for (what, m) in parts {
                let width = m.first().map_or(0, Vec::len);
                for s in 0..width {
                    columns.push(match width {
                        1 => (*what).to_string(),
                        _ => format!("{}", s + 1),
                    });
                    groups.push(match width {
                        1 => String::new(),
                        _ => (*what).to_string(),
                    });
                }
            }
            Table {
                title,
                columns,
                column_groups: groups.into_iter().map(|g| vec![g]).collect(),
                rows: (avars[v.j].cats.iter().enumerate())
                    .map(|(r, code)| {
                        let mut row = vec![text(c.label(*code, None)), Cell::Number(v.d[r])];
                        for (_, m) in parts {
                            row.extend(m[r].iter().map(|x| num(*x)));
                        }
                        row
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            }
        };
        let column = |y: &[f64]| -> Matrix { y.iter().map(|v| vec![*v]).collect() };
        let outer = |y: &[f64], b: &[f64]| -> Matrix {
            y.iter()
                .map(|y| b.iter().map(|b| y * b).collect())
                .collect()
        };
        for (j, v) in vars.iter().enumerate() {
            if print("CENTROID") {
                let mut parts = Vec::new();
                if v.single() {
                    parts.push(("Quantification", column(&v.y)));
                    parts.push(("Projected Centroids", outer(&v.y, &loadings[j])));
                }
                parts.push(("Centroids", centroids[j].clone()));
                tables.push(category_table(
                    format!("Centroids: {}", name_of(v)),
                    v,
                    &parts,
                ));
            }
            if print("QUANT") {
                let mut parts = Vec::new();
                if v.single() {
                    parts.push(("Quantification", column(&v.y)));
                    parts.push(("Single Coordinates", v.points.clone()));
                }
                parts.push(("Multiple Coordinates", v.free.clone()));
                tables.push(category_table(
                    format!("Quantifications: {}", name_of(v)),
                    v,
                    &parts,
                ));
            }
        }
        // every case's scores, None for the cases that take no part
        let mut scores: Vec<Option<Vec<f64>>> = vec![None; prep.n_tot];
        for (i, &o) in objects.iter().enumerate() {
            scores[prep.active[o]] = Some(x[i].clone());
        }
        if print("OBJECT") {
            let mut columns = vec!["Case".to_string()];
            columns.extend(dims(p));
            tables.push(Table {
                title: "Object Scores".into(),
                columns,
                rows: (scores.iter().enumerate())
                    .filter_map(|(c, s)| {
                        let s = s.as_ref()?;
                        let mut row = vec![text((c + 1).to_string())];
                        row.extend(s.iter().map(|x| num(*x)));
                        Some(row)
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }

        // charts
        let ndim = match command.ndim {
            None if p > 2 => Some((1, 2)),
            None | Some((None, _)) => None,
            Some((Some(a), b)) => Some((a, b.unwrap_or(usize::MAX))),
        };
        let pick = |items: &[VarItem]| -> Result<Vec<usize>, Error> {
            if items.is_empty() {
                return Ok((0..vars.len()).collect());
            }
            let names = crate::expand_in(self.active()?, items)?;
            names
                .iter()
                .map(|name| {
                    vars.iter()
                        .position(|v| coded_of(v).name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| {
                            Error(format!(
                                "{WHO}: {name} on PLOT must be an analysis variable"
                            ))
                        })
                })
                .collect()
        };
        let mut charts: Vec<Chart> = Vec::new();
        let category_points = |j: usize, m: &Matrix, length: usize| {
            let v = &vars[j];
            (avars[v.j].cats.iter().zip(m))
                .map(|(code, point)| (coded_of(v).label(*code, Some(length)), point.clone()))
                .collect::<Vec<_>>()
        };
        for (word, items, length) in &command.plots {
            let length = *length;
            match *word {
                "OBJECT" => {
                    if items.is_empty() {
                        let groups = object_groups(coded, &scores, None, Some(length));
                        charts.extend(scatter("Object Scores", &groups, p, ndim));
                    }
                    for name in crate::expand_in(self.active()?, items)? {
                        let k = coded
                            .iter()
                            .position(|c| c.name.eq_ignore_ascii_case(&name))
                            .ok_or_else(|| {
                                Error(format!("{WHO}: {name} on PLOT OBJECT must be on VARIABLES"))
                            })?;
                        let groups = object_groups(coded, &scores, Some(k), Some(length));
                        let title = format!(
                            "Object Scores Labeled by {}",
                            cut(coded[k].title.clone(), coded[k].name.clone(), Some(length))
                        );
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                "LOADINGS" => {
                    let points: Vec<(String, Vec<f64>)> = (vars.iter().enumerate())
                        .filter(|(_, v)| v.single())
                        .map(|(j, v)| {
                            let c = coded_of(v);
                            (
                                cut(c.title.clone(), c.name.clone(), Some(length)),
                                loadings[j].clone(),
                            )
                        })
                        .collect();
                    if points.is_empty() {
                        warnings.push(format!(
                            "{WHO}: the loadings plot needs a variable that is not multiple nominal"
                        ));
                    } else {
                        let groups = vec![("Variables".to_string(), points)];
                        charts.extend(scatter("Component Loadings", &groups, p, ndim));
                    }
                }
                "TRANS" => {
                    for j in pick(items)? {
                        let v = &vars[j];
                        let cats = &avars[v.j].cats;
                        let series: Vec<(String, Vec<f64>)> = match v.single() {
                            true => vec![(String::new(), v.y.clone())],
                            false => (0..p)
                                .map(|s| {
                                    (
                                        format!(", Dimension {}", s + 1),
                                        v.free.iter().map(|r| r[s]).collect(),
                                    )
                                })
                                .collect(),
                        };
                        for (suffix, values) in series {
                            charts.push(transformation_chart(
                                format!("Transformation: {}{suffix}", name_of(v)),
                                coded_of(v),
                                cats,
                                "Quantification",
                                values,
                            ));
                        }
                    }
                }
                "QUANT" | "CENTROID" => {
                    let groups: crate::optimal_scaling::GroupList = pick(items)?
                        .into_iter()
                        .map(|j| {
                            let v = &vars[j];
                            let m = match (*word, v.single()) {
                                ("CENTROID", _) => &centroids[j],
                                (_, true) => &v.points,
                                (_, false) => &v.free,
                            };
                            let c = coded_of(v);
                            (
                                cut(c.title.clone(), c.name.clone(), Some(length)),
                                category_points(j, m, length),
                            )
                        })
                        .collect();
                    let title = match *word {
                        "QUANT" => "Category Coordinates",
                        _ => "Category Centroids",
                    };
                    charts.extend(scatter(title, &groups, p, ndim));
                }
                _ => {}
            }
        }

        // SAVE and MATRIX
        let data = self.active()?;
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if let Some((root, asked)) = &command.save {
            let count = asked.unwrap_or(p).min(p);
            let names = |set: usize| -> Vec<String> {
                (1..=count).map(|s| format!("{root}{s}_{set}")).collect()
            };
            let set = free_set(data, &names);
            for s in 0..count {
                saves.push((
                    format!("{root}{}_{set}", s + 1),
                    format!("Object score in dimension {} ({WHO} {set})", s + 1),
                    scores.iter().map(|x| x.as_ref().map(|x| x[s])).collect(),
                ));
            }
        }
        if !saves.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saves {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F8.4")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
                Ok(())
            })?;
        }
        if let Some(target) = &command.coding.matrix_out {
            let mut rows = Vec::new();
            for (j, v) in vars.iter().enumerate() {
                let c = coded_of(v);
                let row = |rowtype, level: String, dims: Vec<f64>| MatrixRow {
                    rowtype,
                    level,
                    varname: c.name.clone(),
                    vartype: Some(vartype(
                        !v.single(),
                        v.level == LegacyLevel::SingleNominal,
                        v.level == LegacyLevel::Ordinal,
                    )),
                    set: Some(v.set + 1),
                    dims,
                };
                for (r, &code) in avars[v.j].cats.iter().enumerate() {
                    let label = c.label(code, None);
                    match v.single() {
                        true => {
                            let mut quant = vec![0.0; p];
                            quant[0] = v.y[r];
                            rows.push(row("QUANT", label.clone(), quant));
                            rows.push(row("SCOOR_", label.clone(), v.points[r].clone()));
                            let projected = loadings[j].iter().map(|b| v.y[r] * b).collect();
                            rows.push(row("PCENTRO_", label.clone(), projected));
                        }
                        false => rows.push(row("QUANT", label.clone(), v.free[r].clone())),
                    }
                    rows.push(row("MCOOR_", label.clone(), v.free[r].clone()));
                    rows.push(row("CENTRO_", label, centroids[j][r].clone()));
                }
                if v.single() {
                    rows.push(row("WEIGHT_", String::new(), v.a.clone()));
                    rows.push(row("LOADING_", String::new(), loadings[j].clone()));
                }
            }
            self.write_matrix(target, &rows, p)?;
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}
