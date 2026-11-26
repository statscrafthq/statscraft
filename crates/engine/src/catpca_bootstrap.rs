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
use crate::matrix::{Matrix, multiply, transpose};
use crate::mds::polar;
use crate::npar_tests::text;
use crate::optimal_scaling::Prepared;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{Catpca, CatpcaBootstrap, VarItem};

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
    /// Analyses one resample of the active dataset; None when it cannot be analysed.
    #[allow(clippy::too_many_arguments)]
    fn bootstrap_sample(
        &mut self,
        command: &Catpca,
        spec: &CatpcaBootstrap,
        cases: &[usize],
        original: &Solution,
        prep: &Prepared,
        singles: &[usize],
        p: usize,
    ) -> Result<Result<Aligned, String>, Error> {
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
        let run = (|| -> Result<(Solution, Prepared), Error> {
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
        let (solution, sample_prep) = match run {
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
    ) -> Result<Vec<Table>, Error> {
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
            match self.bootstrap_sample(command, spec, &cases, original, prep, &singles, p)? {
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
        let rows: Vec<Vec<Cell>> = (0..p)
            .map(|s| {
                let values: Vec<f64> = kept.iter().map(|a| a.eigen[s]).collect();
                let sp = spread(values, spec.percent);
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
        Ok(tables)
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
