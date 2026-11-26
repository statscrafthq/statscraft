//! RESAMPLE=BOOTSTRAP of CATPCA: how stable the solution is under resampling of the cases.
//!
//! Rules from the Command Syntax Reference, in our words (the Algorithms chapter gives the
//! Procrustes rotation and the reflection rule, not the bootstrap statistics):
//! - BOOTSTRAP(samples, percent, BALANCED or UNBALANCED, PROCRU or REFLEC): 1000 samples, 95
//!   percent, balanced and Procrustes by default. A sample draws as many cases as the data have,
//!   with replacement; the balanced draw gives every case exactly `samples` appearances over all
//!   samples (the cases, each `samples` times, are shuffled and cut into samples), the
//!   unbalanced draw takes each case independently.
//! - Every sample is analysed like the data (same variables, levels, weights, dimensions and
//!   settings); categories a sample lacks are not in its solution. A sample that cannot be
//!   analysed (a variable without variation, too few cases for the dimensions) is left out and
//!   counted in a warning.
//! - Each sample's component loadings, as the loadings table shows them, are aligned to those
//!   of the data: PROCRU rotates them with the orthogonal matrix closest to the data's loadings
//!   in the least squares sense (the polar factor of the cross product, Cliff 1966), REFLEC
//!   only reflects the dimensions whose loadings correlate negatively with the data's.
//! - The tables give, for the eigenvalues, the aligned loadings of the single variables and
//!   the category values of the single variables, the value of the data, the mean and the
//!   standard deviation (with n - 1) of the samples and the percentile interval at the
//!   confidence percent: from the sorted values by linear interpolation at position
//!   (k - 1) p, as `quantile(type = 7)` of R does. The eigenvalues are those of each
//!   sample's own principal axes, not rotated. A category value is taken over the samples that
//!   have the category, with its sign set so that it agrees with the data's over the categories
//!   both have; the table says in how many samples the category occurred. Multiple nominal
//!   variables have no single value and no loadings and are left out of the tables.

use crate::catpca::{Solution, fit_quants_for_sample};
use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::matrix::{Matrix, multiply, transpose};
use crate::mds::polar;
use crate::npar_tests::text;
use crate::optimal_scaling::Prepared;
use crate::plot::{Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{Catpca, CatpcaBootstrap, CatpcaPlot, EllipseLimit, VarItem};

/// The pool of a balanced bootstrap: each of the `n` cases `samples` times, shuffled; sample `b`
/// is the `b`-th run of `n` entries.
pub(crate) fn balanced_pool(
    n: usize,
    samples: usize,
    mut uniform: impl FnMut() -> f64,
) -> Vec<u32> {
    let mut pool: Vec<u32> = (0..samples).flat_map(|_| 0..n as u32).collect();
    // Fisher-Yates
    for i in (1..pool.len()).rev() {
        let j = ((uniform() * (i + 1) as f64) as usize).min(i);
        pool.swap(i, j);
    }
    pool
}

/// The cases of sample `b` of a balanced pool, in case order.
pub(crate) fn balanced_sample(pool: &[u32], n: usize, b: usize) -> Vec<usize> {
    let mut s: Vec<usize> = pool[b * n..(b + 1) * n]
        .iter()
        .map(|&c| c as usize)
        .collect();
    s.sort_unstable();
    s
}

/// One unbalanced sample: `n` cases drawn with replacement.
pub(crate) fn unbalanced_sample(n: usize, mut uniform: impl FnMut() -> f64) -> Vec<usize> {
    let mut s: Vec<usize> = (0..n)
        .map(|_| ((uniform() * n as f64) as usize).min(n - 1))
        .collect();
    s.sort_unstable();
    s
}

/// The matrix that turns `sample` loadings (variables by dimensions) towards `target`: the polar
/// factor of their cross product for Procrustes, else the reflections of the dimensions.
pub(crate) fn alignment(target: &Matrix, sample: &Matrix, procrustes: bool) -> Matrix {
    let cross = multiply(&transpose(sample), target);
    let p = cross.len();
    if procrustes {
        return polar(&cross);
    }
    (0..p)
        .map(|a| {
            (0..p)
                .map(|b| match (a == b, cross[a][a] < 0.0) {
                    (true, true) => -1.0,
                    (true, false) => 1.0,
                    _ => 0.0,
                })
                .collect()
        })
        .collect()
}

/// The percentile at fraction `q` of sorted values, interpolated linearly at (k - 1) q.
pub(crate) fn percentile(sorted: &[f64], q: f64) -> f64 {
    let position = (sorted.len() - 1) as f64 * q;
    let (lo, hi) = (position.floor() as usize, position.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (position - lo as f64)
}

fn mean_sd(values: &[f64]) -> (f64, Option<f64>) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sd = (values.len() > 1)
        .then(|| (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)).sqrt());
    (mean, sd)
}

/// The summary of the values of one statistic over the samples.
struct Spread {
    mean: f64,
    sd: Option<f64>,
    lower: f64,
    upper: f64,
}

fn spread(mut values: Vec<f64>, percent: f64) -> Option<Spread> {
    if values.is_empty() {
        return None;
    }
    let (mean, sd) = mean_sd(&values);
    values.sort_by(f64::total_cmp);
    let tail = (1.0 - percent / 100.0) / 2.0;
    Some(Spread {
        mean,
        sd,
        lower: percentile(&values, tail),
        upper: percentile(&values, 1.0 - tail),
    })
}

fn spread_cells(value: f64, s: Option<&Spread>) -> Vec<Cell> {
    let get = |f: fn(&Spread) -> Option<f64>| s.and_then(f).map_or(Cell::Empty, Cell::Number);
    vec![
        Cell::Number(value),
        get(|s| Some(s.mean)),
        get(|s| s.sd),
        get(|s| Some(s.lower)),
        get(|s| Some(s.upper)),
    ]
}

/// One analysed sample: its summary, its prepared categories and the aligned loadings.
struct Aligned {
    converged: bool,
    eigen: Vec<f64>,
    /// Per analysis variable: the aligned loadings of a single variable.
    loadings: Vec<Vec<f64>>,
    /// Per analysis variable: (category code, value with the data's sign) of a single variable.
    values: Vec<Vec<(i64, f64)>>,
}

impl Engine {
    /// Analyses one resample of the active dataset; Err inside when it cannot be analysed. The
    /// aligned points of a two-dimensional solution go into `clouds`.
    fn bootstrap_sample(
        &mut self,
        run: &Run,
        cases: &[usize],
        clouds: &mut Option<Clouds>,
    ) -> Result<Result<Aligned, String>, Error> {
        let Run {
            command,
            spec,
            original,
            prep,
            singles,
            p,
        } = *run;
        let data = self.active()?;
        let mut sample: Dataset = data.clone();
        let picked: Vec<Option<usize>> = cases.iter().map(|&c| Some(c)).collect();
        sample.take_cases(&picked)?;
        let held = self.dataset.replace(sample);
        let lists: Vec<(&[VarItem], f64)> = command
            .analysis
            .iter()
            .map(|(items, s)| (items.as_slice(), s.weight))
            .collect();
        let fitted = (|| -> Result<(Solution, Prepared), Error> {
            let sample_prep = self.prepare_scaling("CATPCA", &command.common, &lists)?;
            if sample_prep.active.len() <= p {
                return Err(Error(format!(
                    "the sample has {} active cases, too few for {p} dimensions",
                    sample_prep.active.len()
                )));
            }
            let solution = fit_quants_for_sample(self, command, &sample_prep, p)?;
            Ok((solution, sample_prep))
        })();
        self.dataset = held;
        let (solution, sample_prep) = match fitted {
            Ok(done) => done,
            Err(e) => return Ok(Err(e.0)),
        };
        if solution
            .eigen
            .iter()
            .chain(solution.loadings.iter().flatten())
            .chain(solution.values.iter().flatten())
            .any(|v| !v.is_finite())
        {
            return Ok(Err("a variable has no variation in the sample".into()));
        }
        let target: Matrix = singles
            .iter()
            .map(|&j| original.loadings[j].clone())
            .collect();
        let found: Matrix = singles
            .iter()
            .map(|&j| solution.loadings[j].clone())
            .collect();
        let turn = alignment(&target, &found, spec.procrustes);
        let aligned = multiply(&found, &turn);
        if let Some(clouds) = clouds {
            clouds.add_sample(run, &solution, &sample_prep, cases, &turn, &aligned);
        }
        let mut loadings = vec![Vec::new(); original.loadings.len()];
        for (row, &j) in aligned.into_iter().zip(singles) {
            loadings[j] = row;
        }
        let values = (0..original.values.len())
            .map(|j| {
                if solution.values[j].is_empty() {
                    return Vec::new();
                }
                let mine = &sample_prep.avars[j].cats;
                let theirs = &prep.avars[j].cats;
                let pairs: Vec<(i64, f64)> = mine
                    .iter()
                    .copied()
                    .zip(solution.values[j].iter().copied())
                    .collect();
                // the sign that agrees with the data's values over the shared categories
                let agree: f64 = pairs
                    .iter()
                    .filter_map(|(code, v)| {
                        theirs
                            .iter()
                            .position(|c| c == code)
                            .map(|r| v * original.values[j][r])
                    })
                    .sum();
                let sign = if agree < 0.0 { -1.0 } else { 1.0 };
                pairs.into_iter().map(|(c, v)| (c, sign * v)).collect()
            })
            .collect();
        Ok(Ok(Aligned {
            converged: solution.converged,
            eigen: solution.eigen,
            loadings,
            values,
        }))
    }

    /// The bootstrap tables of the analysis `prep` fitted as `original`.
    pub(crate) fn catpca_bootstrap(
        &mut self,
        command: &Catpca,
        spec: &CatpcaBootstrap,
        prep: &Prepared,
        original: &Solution,
        p: usize,
        warnings: &mut Vec<String>,
    ) -> Result<BootResult, Error> {
        let singles: Vec<usize> = prep
            .active_vars
            .iter()
            .copied()
            .filter(|&j| !original.loadings[j].is_empty())
            .collect();
        if singles.is_empty() {
            return Err(Error(
                "CATPCA RESAMPLE needs at least one active variable that is not multiple nominal: the samples are aligned by the loadings of those".into(),
            ));
        }
        let n = prep.n_tot;
        if n.saturating_mul(spec.samples) > 100_000_000 {
            return Err(Error(format!(
                "CATPCA RESAMPLE: {} samples of {n} cases are too many to draw; lower the number of samples",
                spec.samples
            )));
        }
        let run = Run {
            command,
            spec,
            original,
            prep,
            singles: &singles,
            p,
        };
        // the points of a two-dimensional solution, for the ellipses
        let mut clouds =
            (p == 2 && !command.plot.charts.is_empty()).then(|| Clouds::new(prep, original));
        let pool = spec
            .balanced
            .then(|| balanced_pool(n, spec.samples, || self.random()));
        let mut kept: Vec<Aligned> = Vec::new();
        let mut first_reason: Option<String> = None;
        for b in 0..spec.samples {
            let cases = match &pool {
                Some(pool) => balanced_sample(pool, n, b),
                None => unbalanced_sample(n, || self.random()),
            };
            match self.bootstrap_sample(&run, &cases, &mut clouds)? {
                Ok(a) => kept.push(a),
                Err(reason) => {
                    first_reason.get_or_insert(reason);
                }
            }
        }
        if kept.is_empty() {
            return Err(Error(format!(
                "CATPCA RESAMPLE: none of the samples could be analysed ({})",
                first_reason.unwrap_or_default()
            )));
        }
        if kept.len() < spec.samples {
            warnings.push(format!(
                "CATPCA RESAMPLE: {} of {} samples could not be analysed and are left out (first reason: {})",
                spec.samples - kept.len(),
                spec.samples,
                first_reason.unwrap_or_default()
            ));
        }
        let unconverged = kept.iter().filter(|a| !a.converged).count();
        if unconverged > 0 {
            warnings.push(format!(
                "CATPCA RESAMPLE: {unconverged} of the samples did not converge within {} iterations; they are used as they ended",
                command.common.max_iterations
            ));
        }
        let data = self.active()?;
        let shown = self.shown();
        let interval = format!(
            "{}% Percentile Interval",
            crate::catpca_bootstrap::trim(spec.percent)
        );
        let columns = |first: &[&str]| -> (Vec<String>, Vec<Vec<String>>) {
            let mut names: Vec<String> = first.iter().map(|s| s.to_string()).collect();
            let mut groups: Vec<Vec<String>> = vec![Vec::new(); names.len()];
            for stat in ["Mean", "Std. Deviation"] {
                names.push(stat.into());
                groups.push(Vec::new());
            }
            for stat in ["Lower", "Upper"] {
                names.push(stat.into());
                groups.push(vec![interval.clone()]);
            }
            (names, groups)
        };
        let note = format!(
            "{} bootstrap samples ({}, {}); percentile intervals.",
            kept.len(),
            if spec.balanced {
                "balanced"
            } else {
                "unbalanced"
            },
            if spec.procrustes {
                "Procrustes"
            } else {
                "reflection"
            }
        );
        let mut tables = Vec::new();

        // eigenvalues
        let mut eigen_bands: Vec<Band> = Vec::new();
        let rows: Vec<Vec<Cell>> = (0..p)
            .map(|s| {
                let values: Vec<f64> = kept.iter().map(|a| a.eigen[s]).collect();
                let sp = spread(values, spec.percent);
                eigen_bands.push(sp.as_ref().map(|s| (s.lower, s.upper)));
                let mut row = vec![Cell::Number((s + 1) as f64)];
                let mut rest = spread_cells(original.eigen[s], sp.as_ref());
                // the value of the data comes first, then the statistics of the samples
                row.append(&mut rest);
                row
            })
            .collect();
        let (names, groups) = columns(&["Dimension", "Data"]);
        tables.push(boot_table(
            "Bootstrap Eigenvalues",
            names,
            groups,
            1,
            rows,
            &note,
        ));

        // loadings
        let mut rows = Vec::new();
        for &j in &singles {
            let var = data.variable(&prep.analysis_names[j])?;
            for s in 0..p {
                let values: Vec<f64> = kept.iter().map(|a| a.loadings[j][s]).collect();
                let sp = spread(values, spec.percent);
                let mut row = vec![text(shown.variable(&var)), Cell::Number((s + 1) as f64)];
                row.extend(spread_cells(original.loadings[j][s], sp.as_ref()));
                rows.push(row);
            }
        }
        let (names, groups) = columns(&["Variable", "Dimension", "Data"]);
        tables.push(boot_table(
            "Bootstrap Component Loadings",
            names,
            groups,
            2,
            rows,
            &note,
        ));

        // category values
        let mut value_bands: Vec<Vec<Band>> = vec![Vec::new(); original.values.len()];
        let mut rows = Vec::new();
        for j in prep
            .active_vars
            .iter()
            .copied()
            .filter(|&j| !original.values[j].is_empty())
        {
            let var = data.variable(&prep.analysis_names[j])?;
            for (r, &code) in prep.avars[j].cats.iter().enumerate() {
                let values: Vec<f64> = kept
                    .iter()
                    .filter_map(|a| {
                        a.values[j]
                            .iter()
                            .find(|(c, _)| *c == code)
                            .map(|(_, v)| *v)
                    })
                    .collect();
                let count = values.len();
                let sp = spread(values, spec.percent);
                value_bands[j].push(sp.as_ref().map(|s| (s.lower, s.upper)));
                let mut row = vec![
                    text(shown.variable(&var)),
                    text(shown.number(&var, code as f64)),
                ];
                row.extend(spread_cells(original.values[j][r], sp.as_ref()));
                row.push(Cell::Number(count as f64));
                rows.push(row);
            }
        }
        let (mut names, mut groups) = columns(&["Variable", "Category", "Data"]);
        names.push("Samples".into());
        groups.push(Vec::new());
        tables.push(boot_table(
            "Bootstrap Quantifications",
            names,
            groups,
            2,
            rows,
            &note,
        ));
        let ellipses =
            clouds.map(|c| c.ellipses(original, prep, spec.percent, command.plot.ellipse.points));
        Ok(BootResult {
            tables,
            eigen_bands,
            value_bands,
            ellipses,
        })
    }
}

/// A percentile interval, where a sample had the value.
pub(crate) type Band = Option<(f64, f64)>;

/// What the bootstrap hands to the charts of the analysis.
pub(crate) struct BootResult {
    pub(crate) tables: Vec<Table>,
    /// The percentile interval of each eigenvalue.
    pub(crate) eigen_bands: Vec<Band>,
    /// Per analysis variable the interval of each category's value (single levels).
    pub(crate) value_bands: Vec<Vec<Band>>,
    /// The confidence ellipses of a two-dimensional solution.
    pub(crate) ellipses: Option<Ellipses>,
}

/// The inputs every sample needs.
pub(crate) struct Run<'a> {
    command: &'a Catpca,
    spec: &'a CatpcaBootstrap,
    original: &'a Solution,
    prep: &'a Prepared,
    singles: &'a [usize],
    p: usize,
}

impl Clone for Run<'_> {
    fn clone(&self) -> Self {
        *self
    }
}
impl Copy for Run<'_> {}

/// Running sums of the aligned two-dimensional points of one entity over the samples.
#[derive(Clone, Default)]
pub(crate) struct Cloud {
    n: f64,
    sx: f64,
    sy: f64,
    sxx: f64,
    sxy: f64,
    syy: f64,
}

impl Cloud {
    fn add(&mut self, point: &[f64]) {
        let (x, y) = (point[0], point[1]);
        self.n += 1.0;
        self.sx += x;
        self.sy += y;
        self.sxx += x * x;
        self.sxy += x * y;
        self.syy += y * y;
    }

    /// Variances and covariance (n - 1); None with fewer than three points.
    fn covariance(&self) -> Option<[f64; 3]> {
        (self.n >= 3.0).then(|| {
            let d = self.n - 1.0;
            [
                (self.sxx - self.sx * self.sx / self.n) / d,
                (self.sxy - self.sx * self.sy / self.n) / d,
                (self.syy - self.sy * self.sy / self.n) / d,
            ]
        })
    }
}

/// The points of the samples, per entity.
pub(crate) struct Clouds {
    eigen: Cloud,
    loadings: Vec<Cloud>,
    /// Per analysis variable the clouds of the categories of the data.
    categories: Vec<Vec<Cloud>>,
    /// Per case of the data.
    objects: Vec<Cloud>,
}

impl Clouds {
    fn new(prep: &Prepared, original: &Solution) -> Clouds {
        Clouds {
            eigen: Cloud::default(),
            loadings: vec![Cloud::default(); original.loadings.len()],
            categories: prep
                .avars
                .iter()
                .map(|v| vec![Cloud::default(); v.cats.len()])
                .collect(),
            objects: vec![Cloud::default(); prep.n_tot],
        }
    }

    /// Adds a sample's points, turned by `turn` as its loadings were.
    fn add_sample(
        &mut self,
        run: &Run,
        solution: &Solution,
        sample_prep: &Prepared,
        cases: &[usize],
        turn: &Matrix,
        aligned: &Matrix,
    ) {
        self.eigen.add(&solution.eigen);
        for (row, &j) in aligned.iter().zip(run.singles) {
            self.loadings[j].add(row);
        }
        let turned = |point: &[f64], factor: &[f64]| -> Vec<f64> {
            let scaled: Vec<f64> = point.iter().zip(factor).map(|(a, f)| a * f).collect();
            (0..run.p)
                .map(|s| scaled.iter().enumerate().map(|(r, v)| v * turn[r][s]).sum())
                .collect()
        };
        for &j in &run.prep.active_vars {
            let factor = match solution.loadings[j].is_empty() {
                true => &solution.vector_factor,
                false => &solution.score_factor,
            };
            for (r, code) in sample_prep.avars[j].cats.iter().enumerate() {
                if let Ok(at) = run.prep.avars[j].cats.binary_search(code) {
                    self.categories[j][at].add(&turned(&solution.centroids[j][r], factor));
                }
            }
        }
        let mut last = usize::MAX;
        for (i, &row) in sample_prep.active.iter().enumerate() {
            // a case drawn twice has the same scores both times: once is enough
            let case = cases[row];
            if case == last {
                continue;
            }
            last = case;
            self.objects[case].add(&turned(&solution.scores[i], &solution.score_factor));
        }
    }

    /// The confidence ellipses around the points of the data.
    fn ellipses(
        &self,
        original: &Solution,
        prep: &Prepared,
        percent: f64,
        points: usize,
    ) -> Ellipses {
        let make = |center: [f64; 2], cloud: &Cloud| ellipse(center, cloud, percent, points);
        let scaled = |point: &[f64], factor: &[f64]| -> [f64; 2] {
            [point[0] * factor[0], point[1] * factor[1]]
        };
        Ellipses {
            eigen: make([original.eigen[0], original.eigen[1]], &self.eigen),
            loadings: self
                .loadings
                .iter()
                .enumerate()
                .map(|(j, c)| {
                    let l = &original.loadings[j];
                    (!l.is_empty()).then(|| make([l[0], l[1]], c)).flatten()
                })
                .collect(),
            categories: self
                .categories
                .iter()
                .enumerate()
                .map(|(j, clouds)| {
                    let factor = match original.loadings[j].is_empty() {
                        true => &original.vector_factor,
                        false => &original.score_factor,
                    };
                    clouds
                        .iter()
                        .enumerate()
                        .map(|(r, c)| make(scaled(&original.centroids[j][r], factor), c))
                        .collect()
                })
                .collect(),
            objects: {
                let mut out: Vec<Option<Ellipse>> = vec![None; self.objects.len()];
                for (i, &case) in prep.active.iter().enumerate() {
                    out[case] = make(
                        scaled(&original.scores[i], &original.score_factor),
                        &self.objects[case],
                    );
                }
                out
            },
        }
    }
}

/// A confidence ellipse: its center (the data's point), its contour and its area.
#[derive(Clone)]
pub(crate) struct Ellipse {
    pub(crate) center: [f64; 2],
    pub(crate) contour: Vec<[f64; 2]>,
    pub(crate) area: f64,
}

/// The ellipses of every entity of a two-dimensional solution.
pub(crate) struct Ellipses {
    pub(crate) eigen: Option<Ellipse>,
    pub(crate) loadings: Vec<Option<Ellipse>>,
    pub(crate) categories: Vec<Vec<Option<Ellipse>>>,
    pub(crate) objects: Vec<Option<Ellipse>>,
}

/// The normal-theory region (x - c)' S^-1 (x - c) <= k around `center`, k the chi-square
/// quantile of two degrees of freedom, -2 ln(1 - percent / 100), with S the covariance of the
/// points; `points` contour points. None when there are too few samples.
fn ellipse(center: [f64; 2], cloud: &Cloud, percent: f64, points: usize) -> Option<Ellipse> {
    let [a, b, c] = cloud.covariance()?;
    let k = -2.0 * (1.0 - percent / 100.0).ln();
    let mean = (a + c) / 2.0;
    let half = (((a - c) / 2.0).powi(2) + b * b).sqrt();
    let (l1, l2) = ((mean + half).max(0.0), (mean - half).max(0.0));
    // the unit vector of the larger eigenvalue
    let axis = if b != 0.0 {
        let v = [l1 - c, b];
        let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
        [v[0] / norm, v[1] / norm]
    } else if a >= c {
        [1.0, 0.0]
    } else {
        [0.0, 1.0]
    };
    let (r1, r2) = ((k * l1).sqrt(), (k * l2).sqrt());
    let mut contour: Vec<[f64; 2]> = (0..points)
        .map(|t| {
            let angle = std::f64::consts::TAU * t as f64 / points as f64;
            let (u, v) = (r1 * angle.cos(), r2 * angle.sin());
            [
                center[0] + u * axis[0] - v * axis[1],
                center[1] + u * axis[1] + v * axis[0],
            ]
        })
        .collect();
    // closed: the last point is the first
    contour.push(contour[0]);
    Some(Ellipse {
        center,
        contour,
        area: std::f64::consts::PI * k * (l1 * l2).sqrt(),
    })
}

/// The entities whose ellipse area passes the limit: above or below the area given, or the mean
/// area plus that many standard deviations of the areas.
fn passing(ellipses: &[Option<&Ellipse>], limit: &EllipseLimit) -> Vec<bool> {
    let areas: Vec<f64> = ellipses.iter().flatten().map(|e| e.area).collect();
    let threshold = match limit.stdev {
        false => limit.value,
        true => {
            let n = areas.len() as f64;
            let mean = areas.iter().sum::<f64>() / n.max(1.0);
            let sd = match areas.len() > 1 {
                true => (areas.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt(),
                false => 0.0,
            };
            mean + limit.value * sd
        }
    };
    ellipses
        .iter()
        .map(|e| {
            e.is_some_and(|e| match limit.greater {
                true => e.area > threshold,
                false => e.area < threshold,
            })
        })
        .collect()
}

/// A chart of confidence ellipses: one group per entity, its point first and then its closed
/// contour.
fn ellipse_chart(title: String, shown: &[(String, &Ellipse)]) -> Option<Chart> {
    if shown.is_empty() {
        return None;
    }
    let groups = shown
        .iter()
        .map(|(name, e)| {
            let mut points = vec![Point {
                x: e.center[0],
                y: e.center[1],
                z: None,
                label: Some(name.clone()),
            }];
            points.extend(e.contour.iter().map(|c| Point {
                x: c[0],
                y: c[1],
                z: None,
                label: None,
            }));
            PointGroup {
                name: name.clone(),
                points,
            }
        })
        .collect();
    Some(Chart::plot(
        ChartKind::ConfidenceEllipse,
        title,
        Axis::titled("Dimension 1"),
        Axis::titled("Dimension 2"),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    ))
}

impl Engine {
    /// The charts of the bootstrap of a two-dimensional solution: the eigenvalues with their
    /// intervals, and the confidence ellipses of the loadings, the categories and the objects
    /// as the plots of the command ask for them.
    pub(crate) fn catpca_bootstrap_charts(
        &self,
        command: &Catpca,
        prep: &Prepared,
        result: &BootResult,
        original: &Solution,
        object_labels: &[String],
    ) -> Result<Vec<Chart>, Error> {
        let mut charts = Vec::new();
        if command.plot.charts.is_empty() {
            return Ok(charts);
        }
        let p = original.eigen.len();
        // the eigenvalues of the data with the interval of the samples
        let categories = (0..p)
            .map(|s| Category {
                label: format!("Dimension {}", s + 1),
                value: Cell::Number((s + 1) as f64),
                missing: false,
                bin: None,
            })
            .collect();
        let series = |name: &str, values: Vec<f64>| Series {
            name: name.to_string(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values,
        };
        let band = |pick: fn(&(f64, f64)) -> f64| -> Vec<f64> {
            result
                .eigen_bands
                .iter()
                .map(|b| b.as_ref().map_or(f64::NAN, pick))
                .collect()
        };
        charts.push(Chart::lines(
            "Bootstrap Eigenvalues".into(),
            Axis::titled("Dimension"),
            Axis::titled("Eigenvalue"),
            categories,
            vec![
                series("Data", original.eigen.clone()),
                series("Lower", band(|b| b.0)),
                series("Upper", band(|b| b.1)),
            ],
        ));
        let Some(e) = &result.ellipses else {
            return Ok(charts);
        };
        let data = self.active()?;
        let shown = self.shown();
        let limits = &command.plot.ellipse;
        let wants = |is: fn(&CatpcaPlot) -> bool| command.plot.charts.iter().any(|(c, _)| is(c));
        if let Some(ellipse) = &e.eigen {
            charts.extend(ellipse_chart(
                "Confidence Ellipse of the Eigenvalues".into(),
                &[("Eigenvalues".to_string(), ellipse)],
            ));
        }
        let name_of = |j: usize| -> Result<String, Error> {
            Ok(shown.variable(&data.variable(&prep.analysis_names[j])?))
        };
        if wants(|c| matches!(c, CatpcaPlot::Loading(..))) {
            let all: Vec<Option<&Ellipse>> = e.loadings.iter().map(Option::as_ref).collect();
            let keep = passing(&all, &limits.loadings);
            let mut picked = Vec::new();
            for (j, ellipse) in all.iter().enumerate() {
                if let (true, Some(ellipse)) = (keep[j], ellipse) {
                    picked.push((name_of(j)?, *ellipse));
                }
            }
            charts.extend(ellipse_chart(
                "Confidence Ellipses of the Component Loadings".into(),
                &picked,
            ));
        }
        if wants(|c| matches!(c, CatpcaPlot::Object(..))) {
            let all: Vec<Option<&Ellipse>> = e.objects.iter().map(Option::as_ref).collect();
            let keep = passing(&all, &limits.objects);
            let picked: Vec<(String, &Ellipse)> = all
                .iter()
                .enumerate()
                .filter_map(|(c, ellipse)| {
                    ellipse
                        .filter(|_| keep[c])
                        .map(|el| (object_labels[c].clone(), el))
                })
                .collect();
            charts.extend(ellipse_chart(
                "Confidence Ellipses of the Object Points".into(),
                &picked,
            ));
        }
        // the category points of the variables a CATEGORY plot names, and of all of them together
        // for JOINTCAT
        let analysis_of = |name: &str| {
            prep.analysis_names
                .iter()
                .position(|a| a.eq_ignore_ascii_case(name))
        };
        let mut requested: Vec<usize> = Vec::new();
        let mut joint = false;
        for (chart, _) in &command.plot.charts {
            match chart {
                CatpcaPlot::Category(items) | CatpcaPlot::Jointcat(items) => {
                    joint |= matches!(chart, CatpcaPlot::Jointcat(_));
                    let names = match items.is_empty() {
                        true => prep.analysis_names.clone(),
                        false => crate::optimal_scaling::resolve(
                            "CATPCA",
                            items,
                            &prep.analysis_names,
                            &prep.analysis_names,
                        )?,
                    };
                    for name in names {
                        if let Some(j) = analysis_of(&name)
                            && !requested.contains(&j)
                        {
                            requested.push(j);
                        }
                    }
                }
                _ => {}
            }
        }
        let category_label = |j: usize, r: usize| -> Result<String, Error> {
            let var = data.variable(&prep.analysis_names[j])?;
            Ok(shown.number(&var, prep.avars[j].cats[r] as f64))
        };
        let mut joint_picked: Vec<(String, &Ellipse)> = Vec::new();
        for &j in &requested {
            let all: Vec<Option<&Ellipse>> = e.categories[j].iter().map(Option::as_ref).collect();
            let keep = passing(&all, &limits.categories);
            let mut picked = Vec::new();
            for (r, ellipse) in all.iter().enumerate() {
                if let (true, Some(ellipse)) = (keep[r], ellipse) {
                    picked.push((category_label(j, r)?, *ellipse));
                    joint_picked.push((
                        format!("{}: {}", name_of(j)?, category_label(j, r)?),
                        *ellipse,
                    ));
                }
            }
            charts.extend(ellipse_chart(
                format!(
                    "Confidence Ellipses of the Category Points: {}",
                    name_of(j)?
                ),
                &picked,
            ));
        }
        if joint {
            charts.extend(ellipse_chart(
                "Confidence Ellipses of the Category Points".into(),
                &joint_picked,
            ));
        }
        Ok(charts)
    }
}

/// A number without a trailing `.0`.
pub(crate) fn trim(v: f64) -> String {
    format!("{v}").trim_end_matches(".0").to_string()
}

fn boot_table(
    title: &str,
    columns: Vec<String>,
    column_groups: Vec<Vec<String>>,
    row_headers: usize,
    rows: Vec<Vec<Cell>>,
    caption: &str,
) -> Table {
    Table {
        title: title.into(),
        row_headers,
        columns,
        column_groups,
        rows,
        caption: Some(caption.to_string()),
        ..Table::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: u64) -> impl FnMut() -> f64 {
        let mut state = seed;
        move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    #[test]
    fn a_balanced_bootstrap_draws_every_case_exactly_samples_times() {
        let (n, samples) = (7, 50);
        let pool = balanced_pool(n, samples, lcg(1));
        let drawn: Vec<Vec<usize>> = (0..samples).map(|b| balanced_sample(&pool, n, b)).collect();
        assert_eq!(drawn.len(), samples);
        assert!(drawn.iter().all(|s| s.len() == n));
        let mut count = vec![0; n];
        for case in drawn.iter().flatten() {
            count[*case] += 1;
        }
        assert!(count.iter().all(|&c| c == samples), "{count:?}");
        // the samples differ from one another
        assert!(drawn.iter().any(|s| s != &drawn[0]));
        // the same generator gives the same samples; another one does not
        assert_eq!(pool, balanced_pool(n, samples, lcg(1)));
        assert_ne!(pool, balanced_pool(n, samples, lcg(2)));
    }

    #[test]
    fn an_unbalanced_sample_has_the_size_of_the_data_and_repeats_cases() {
        let s = unbalanced_sample(30, lcg(5));
        assert_eq!(s.len(), 30);
        assert!(s.windows(2).any(|w| w[0] == w[1]));
        assert!(s.iter().all(|&c| c < 30));
    }

    fn rotate(m: &Matrix, t: &Matrix) -> Matrix {
        multiply(m, t)
    }

    #[test]
    fn procrustes_and_reflection_recover_a_turned_solution() {
        let target: Matrix = vec![
            vec![0.9, 0.1],
            vec![0.7, -0.4],
            vec![-0.2, 0.8],
            vec![0.3, 0.6],
        ];
        // a rotation by 0.6 radians, then a reflection of the second dimension
        let (c, s) = (0.6f64.cos(), 0.6f64.sin());
        let turn: Matrix = vec![vec![c, s], vec![-s, c]];
        let turned = rotate(&target, &turn);
        let back = rotate(&turned, &alignment(&target, &turned, true));
        for (a, b) in back.iter().zip(&target) {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() < 1e-9, "{back:?}");
            }
        }
        let flip: Matrix = vec![vec![-1.0, 0.0], vec![0.0, 1.0]];
        let reflected = rotate(&target, &flip);
        let restored = rotate(&reflected, &alignment(&target, &reflected, false));
        for (a, b) in restored.iter().zip(&target) {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() < 1e-12);
            }
        }
        // a reflection alone is also recovered by the Procrustes rotation
        let restored = rotate(&reflected, &alignment(&target, &reflected, true));
        for (a, b) in restored.iter().zip(&target) {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() < 1e-9);
            }
        }
    }

    fn cloud_of(points: &[[f64; 2]]) -> Cloud {
        let mut cloud = Cloud::default();
        for p in points {
            cloud.add(p);
        }
        cloud
    }

    #[test]
    fn an_ellipse_has_the_covariance_the_area_and_the_contour_of_r() {
        // cov(cbind(x, y)) in R: 3.70, 3.15, 4.30; the chi-square level of two degrees of
        // freedom at 95% is 5.99146454710798 and the area pi k sqrt(det S) is 46.0580587846453
        let cloud = cloud_of(&[[1.0, 2.0], [2.0, 1.0], [3.0, 5.0], [4.0, 3.0], [6.0, 6.0]]);
        let [a, b, c] = cloud.covariance().unwrap();
        assert!((a - 3.70).abs() < 1e-12 && (b - 3.15).abs() < 1e-12 && (c - 4.30).abs() < 1e-12);
        let e = ellipse([10.0, -2.0], &cloud, 95.0, 24).unwrap();
        assert!((e.area - 46.0580587846453).abs() < 1e-9);
        // closed contour of 24 steps; every point lies on (x - c)' S^-1 (x - c) = k
        assert_eq!(e.contour.len(), 25);
        assert_eq!(e.contour[0], e.contour[24]);
        let det = a * c - b * b;
        let k = 5.99146454710798;
        for p in &e.contour {
            let (u, v) = (p[0] - 10.0, p[1] + 2.0);
            let form = (c * u * u - 2.0 * b * u * v + a * v * v) / det;
            assert!((form - k).abs() < 1e-9, "{form}");
        }
        // a cloud of fewer than three points has no ellipse
        assert!(ellipse([0.0, 0.0], &cloud_of(&[[1.0, 1.0], [2.0, 3.0]]), 95.0, 8).is_none());
    }

    #[test]
    fn a_limit_keeps_the_ellipses_above_or_below_an_area_or_mean_plus_deviations() {
        let make = |area: f64| Ellipse {
            center: [0.0, 0.0],
            contour: Vec::new(),
            area,
        };
        let all = [0.5, 1.0, 1.5, 4.0, 9.0].map(make);
        let refs: Vec<Option<&Ellipse>> = all.iter().map(Some).collect();
        let limit = |greater, stdev, value| EllipseLimit {
            greater,
            stdev,
            value,
        };
        // mean 3.2, sd 3.41...: mean + 2 sd = 10.2213958726168 keeps none, mean + 0.5 sd =
        // 4.9553489681542 keeps the 9 (R: mean(areas) + k * sd(areas))
        assert_eq!(passing(&refs, &limit(true, true, 2.0)), [false; 5]);
        assert_eq!(
            passing(&refs, &limit(true, true, 0.5)),
            [false, false, false, false, true]
        );
        assert_eq!(
            passing(&refs, &limit(false, false, 1.5)),
            [true, true, false, false, false]
        );
        assert_eq!(
            passing(&refs, &limit(true, false, 0.0)),
            [true, true, true, true, true]
        );
    }

    #[test]
    fn percentiles_interpolate_like_r_type_7() {
        // quantile(c(1, 2, 4, 8, 16, 32), c(0.025, 0.5, 0.975)) in R
        let v = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
        let want = [1.125, 6.0, 30.0];
        for (q, w) in [0.025, 0.5, 0.975].into_iter().zip(want) {
            assert!((percentile(&v, q) - w).abs() < 1e-12);
        }
        assert_eq!(percentile(&[5.0], 0.975), 5.0);
    }
}
