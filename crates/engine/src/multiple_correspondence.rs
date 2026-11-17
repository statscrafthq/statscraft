//! MULTIPLE CORRESPONDENCE: homogeneity analysis of categorical variables, with its tables,
//! charts and saved variables.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Every analysis variable is a set of categories coded by positive whole numbers. Values
//!   below 1, system-missing and user-missing values are missing. A string variable is first
//!   coded by the sorted order of its values. A numeric variable with fractions is grouped into
//!   7 categories (fewer when it has fewer values) shaped like a normal distribution, unless
//!   DISCRETIZATION says otherwise. Discretizing ignores the case weights and sees values below
//!   1 as valid.
//! - The analysis gives every object (case) a point and every category a point so that each
//!   category sits at the weighted mean (centroid) of its objects and the objects lie as close
//!   as possible to their own categories. Each variable counts with its weight. A missing value
//!   that is passive simply leaves that variable out for that object. Objects are centered and
//!   normalized so that, weighted by the case weight times the summed weights of the variables
//!   they have, each dimension has mean 0, the dimensions are uncorrelated and their sum of
//!   squares is n times that summed weight.
//! - The discrimination measure of a variable in a dimension is the weighted variance of its
//!   category centroids (the squared correlation ratio); the eigenvalue of a dimension is the
//!   weighted sum of these over the active variables, its inertia the eigenvalue over the summed
//!   variable weights. Cronbach's alpha of eigenvalue e with m weighted variables is
//!   m(e - 1) / ((m - 1)e).
//! - Supplementary objects get scores from their categories after the fit; supplementary
//!   variables get centroids from the fitted objects. An object without any active value
//!   becomes supplementary.
//! - NORMALIZATION spreads the singular value of each dimension: the objects get the power
//!   (1 + q)/2, the categories (1 - q)/2, both from the variable principal solution in which the
//!   objects are standard and the categories are centroids.

use crate::matrix::Matrix;
use crate::npar_tests::text;
use crate::optimal_scaling::{
    AVar, centroids, coded_index, correlation_table, correlations, cut, eigen_sorted, free_set,
    num, object_groups, orthonormalize, point_chart, resolve, scatter, transformation_chart,
};
use crate::plot::Point;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{
    Impute, McaMissing, McaNormalization, McaPlot, MultipleCorrespondence, VarItem,
};
use std::collections::BTreeMap;

const WHO: &str = "MULTIPLE CORRESPONDENCE";

/// The fitted solution, in variable principal form.
struct Fit {
    /// Per active object: the standard scores.
    x: Matrix,
    /// The start, then every iteration: (variance accounted for, loss).
    history: Vec<(f64, f64)>,
    converged: bool,
}

/// Alternating least squares from `x`: centroids, objects as the weighted mean of their
/// categories, then centering and orthonormalization, until the fit gains less than
/// `criterion`. Ends with the dimensions turned to principal axes.
fn iterate(
    vars: &[&AVar],
    w: &[f64],
    mstar: &[f64],
    mut x: Matrix,
    max_iterations: usize,
    criterion: f64,
) -> Fit {
    let p = x[0].len();
    let n_w: f64 = w.iter().sum();
    let m_w: f64 = vars.iter().map(|v| v.weight).sum();
    let mw: Vec<f64> = w.iter().zip(mstar).map(|(a, b)| a * b).collect();
    let fit_of = |x: &Matrix| -> f64 {
        vars.iter()
            .map(|v| {
                let y = centroids(v, x, w, p);
                v.weight
                    * y.iter()
                        .zip(&v.d)
                        .map(|(r, d)| d * r.iter().map(|e| e * e).sum::<f64>())
                        .sum::<f64>()
            })
            .sum::<f64>()
            / (n_w * p as f64)
    };
    let mut fit = fit_of(&x);
    let mut history = vec![(fit, m_w - fit)];
    let mut converged = false;
    for _ in 0..max_iterations {
        let mut z = vec![vec![0.0; p]; w.len()];
        for v in vars {
            let y = centroids(v, &x, w, p);
            for (i, h) in v.h.iter().enumerate() {
                if let Some(r) = h {
                    for s in 0..p {
                        z[i][s] += v.weight * y[*r][s] / mstar[i];
                    }
                }
            }
        }
        if orthonormalize(&mut z, &mw, n_w * m_w).is_none() {
            break;
        }
        x = z;
        let next = fit_of(&x);
        history.push((next, m_w - next));
        let gain = next - fit;
        fit = next;
        if gain < criterion {
            converged = true;
            break;
        }
    }
    // principal axes: the eigenvectors of the summed weighted category cross products
    let mut t = vec![vec![0.0; p]; p];
    for v in vars {
        let y = centroids(v, &x, w, p);
        for (r, row) in y.iter().enumerate() {
            for a in 0..p {
                for b in 0..p {
                    t[a][b] += v.weight * v.d[r] * row[a] * row[b];
                }
            }
        }
    }
    let (_, l) = eigen_sorted(&t);
    for row in x.iter_mut() {
        let new: Vec<f64> = (0..p)
            .map(|b| (0..p).map(|a| row[a] * l[a][b]).sum())
            .collect();
        *row = new;
    }
    // the object with the largest absolute score is positive in every dimension
    for s in 0..p {
        let big = x.iter().map(|r| r[s].abs()).fold(0.0, f64::max);
        if let Some(r) = x.iter().find(|r| r[s].abs() >= big * (1.0 - 1e-9))
            && r[s] < 0.0
        {
            x.iter_mut().for_each(|r| r[s] = -r[s]);
        }
    }
    Fit {
        x,
        history,
        converged,
    }
}

impl Engine {
    pub(crate) fn multiple_correspondence(
        &mut self,
        command: &MultipleCorrespondence,
    ) -> Result<Vec<Table>, Error> {
        let lists: Vec<(&[VarItem], f64)> = command
            .analysis
            .iter()
            .map(|(items, weight)| (items.as_slice(), *weight))
            .collect();
        let prep = self.prepare_scaling(WHO, &command.common, &lists)?;
        let data = self.active()?;
        let n_tot = prep.n_tot;
        let fixed = prep.fixed;
        let mut warnings = prep.warnings.clone();
        let variables = &prep.variables;
        let analysis_names = &prep.analysis_names;
        let coded = &prep.coded;
        let avars = &prep.avars;
        let active_vars = &prep.active_vars;
        let active = &prep.active;
        let excluded = &prep.excluded;
        let (w, n_w, m_w, mstar) = (&prep.w, prep.n_w, prep.m_w, &prep.mstar);
        let passive_missing = prep.passive_missing;
        let n = active.len();

        // dimensions
        let mut p = command.common.dimension;
        if !fixed {
            let k: usize = active_vars.iter().map(|&j| avars[j].cats.len()).sum();
            let m1 = active_vars
                .iter()
                .filter(|&&j| avars[j].h.iter().all(Option::is_some))
                .count();
            let less = match passive_missing {
                true => m1.max(1),
                false => active_vars.len(),
            };
            let pmax = (n - 1).min(k.saturating_sub(less));
            if pmax < 1 {
                return Err(Error(format!(
                    "{WHO}: the data leave no dimension to compute (every variable has one category)"
                )));
            }
            if p > pmax {
                warnings.push(format!(
                    "{WHO}: DIMENSION is {p}, but these data allow at most {pmax}; {pmax} are computed"
                ));
                p = pmax;
            }
        }

        // the fit
        let act: Vec<&AVar> = active_vars.iter().map(|&j| &avars[j]).collect();
        let indicators = vec![None; act.len()];
        let start = prep.start(self, WHO, &command.common.configuration, &indicators, p)?;
        let fit = if fixed {
            Fit {
                x: start,
                history: Vec::new(),
                converged: true,
            }
        } else {
            iterate(
                &act,
                w,
                mstar,
                start,
                command.common.max_iterations,
                command.common.criterion,
            )
        };
        if !fit.converged {
            warnings.push(format!(
                "{WHO}: the fit did not converge within {} iterations",
                command.common.max_iterations
            ));
        }
        let x = fit.x;

        // centroids, discrimination measures, eigenvalues
        let ys: Vec<Matrix> = avars.iter().map(|v| centroids(v, &x, w, p)).collect();
        let discr: Vec<Vec<f64>> = avars
            .iter()
            .zip(&ys)
            .map(|(v, y)| {
                (0..p)
                    .map(|s| {
                        y.iter()
                            .zip(&v.d)
                            .map(|(r, d)| d * r[s] * r[s])
                            .sum::<f64>()
                            / n_w
                    })
                    .collect()
            })
            .collect();
        let eigen: Vec<f64> = (0..p)
            .map(|s| {
                active_vars
                    .iter()
                    .map(|&j| avars[j].weight * discr[j][s])
                    .sum()
            })
            .collect();
        let inertia: Vec<f64> = eigen.iter().map(|e| e / m_w).collect();
        // the normalization powers of the singular value sqrt(inertia)
        let (a_pow, b_pow) = match (fixed, command.common.normalization) {
            (true, _) => (0.0, 1.0),
            (false, McaNormalization::Independent) => (1.0, 1.0),
            (false, McaNormalization::Q(q)) => ((1.0 + q) / 2.0, (1.0 - q) / 2.0),
        };
        if fixed && command.common.normalization != McaNormalization::Q(-1.0) {
            warnings.push(format!(
                "{WHO}: with a FIXED configuration the scores keep their own scale; NORMALIZATION is ignored"
            ));
        }
        let sigma: Vec<f64> = inertia.iter().map(|l| l.max(0.0).sqrt()).collect();
        let x_factor: Vec<f64> = sigma.iter().map(|s| s.powf(a_pow)).collect();
        let y_factor: Vec<f64> = sigma
            .iter()
            .map(|s| {
                if b_pow == 1.0 {
                    1.0
                } else {
                    s.powf(b_pow - 1.0)
                }
            })
            .collect();

        // scores of every case: active from the fit, others from their categories
        let mut scores: Vec<Option<Vec<f64>>> = vec![None; n_tot];
        for (i, &c) in active.iter().enumerate() {
            scores[c] = Some(x[i].clone());
        }
        for c in 0..n_tot {
            if excluded[c] || scores[c].is_some() {
                continue;
            }
            let (mut z, mut m) = (vec![0.0; p], 0.0);
            let fitting: Vec<usize> = match fixed {
                true => Vec::new(),
                false => active_vars.clone(),
            };
            for &j in &fitting {
                let v = &avars[j];
                let Some(r) = v.codes[c].and_then(|x| v.cats.binary_search(&(x as i64)).ok())
                else {
                    continue;
                };
                m += v.weight;
                for s in 0..p {
                    z[s] += v.weight * ys[j][r][s];
                }
            }
            if m > 0.0 {
                scores[c] = Some(
                    (0..p)
                        .map(|s| match inertia[s] > 0.0 {
                            true => z[s] / (m * inertia[s]),
                            false => 0.0,
                        })
                        .collect(),
                );
            }
        }
        // categories that only supplementary objects have: the mean of their scores
        let mut extra_cats: Vec<BTreeMap<i64, (f64, Vec<f64>)>> =
            vec![BTreeMap::new(); avars.len()];
        for (j, v) in avars.iter().enumerate() {
            for (c, score) in scores.iter().enumerate() {
                let (Some(code), Some(score)) = (v.codes[c], score) else {
                    continue;
                };
                if active.binary_search(&c).is_ok() || v.cats.binary_search(&(code as i64)).is_ok()
                {
                    continue;
                }
                let e = extra_cats[j]
                    .entry(code as i64)
                    .or_insert((0.0, vec![0.0; p]));
                e.0 += 1.0;
                e.1.iter_mut().zip(score).for_each(|(a, b)| *a += b);
            }
            for (count, sum) in extra_cats[j].values_mut() {
                sum.iter_mut().for_each(|x| *x /= *count);
            }
        }
        // the quantification of each code, normalized
        let quantification = |j: usize, code: f64| -> Option<Vec<f64>> {
            let v = &avars[j];
            let y = match v.cats.binary_search(&(code as i64)) {
                Ok(r) => ys[j][r].clone(),
                Err(_) => extra_cats[j].get(&(code as i64))?.1.clone(),
            };
            Some((0..p).map(|s| y[s] * y_factor[s]).collect())
        };
        let normalized = |score: &Vec<f64>| -> Vec<f64> {
            score.iter().zip(&x_factor).map(|(a, b)| a * b).collect()
        };

        // tables
        let print = &command.print;
        let mut tables = Vec::new();
        let analysis_titles: Vec<String> =
            avars.iter().map(|v| coded[v.coded].title.clone()).collect();
        let dims: Vec<String> = (1..=p).map(|s| format!("Dimension {s}")).collect();
        tables.push(prep.case_summary());
        if let Some(items) = &print.descrip {
            tables.extend(prep.descriptives(WHO, items)?);
        }
        if !fixed {
            let shown_history: Vec<(usize, &(f64, f64))> = match print.history {
                true => fit.history.iter().enumerate().collect(),
                false => fit
                    .history
                    .iter()
                    .enumerate()
                    .next_back()
                    .into_iter()
                    .collect(),
            };
            let rows = shown_history
                .into_iter()
                .map(|(k, (fit_k, loss))| {
                    let before = match k {
                        0 => Cell::Empty,
                        k => Cell::Number(fit_k - fit.history[k - 1].0),
                    };
                    vec![
                        Cell::Number(k as f64),
                        Cell::Number(*fit_k),
                        before,
                        Cell::Number(*loss),
                    ]
                })
                .collect();
            tables.push(Table {
                title: "Iteration History".into(),
                columns: vec![
                    "Iteration Number".into(),
                    "Total".into(),
                    "Increase".into(),
                    "Loss".into(),
                ],
                column_groups: vec![
                    Vec::new(),
                    vec!["Variance Accounted For".into()],
                    vec!["Variance Accounted For".into()],
                    Vec::new(),
                ],
                rows,
                row_headers: 1,
                caption: Some(match fit.converged {
                    true => "The fit stopped gaining more than the convergence criterion.".into(),
                    false => "The most iterations were reached.".into(),
                }),
                ..Table::default()
            });
            let alpha = |e: f64| match m_w > 1.0 && e != 0.0 {
                true => num(m_w * (e - 1.0) / ((m_w - 1.0) * e)),
                false => Cell::Empty,
            };
            let total_e: f64 = eigen.iter().sum();
            let mut rows: Vec<Vec<Cell>> = (0..p)
                .map(|s| {
                    vec![
                        text((s + 1).to_string()),
                        alpha(eigen[s]),
                        Cell::Number(eigen[s]),
                        Cell::Number(inertia[s]),
                        Cell::Number(100.0 * inertia[s]),
                    ]
                })
                .collect();
            rows.push(vec![
                text("Total"),
                alpha(total_e),
                Cell::Number(total_e),
                Cell::Number(total_e / m_w),
                Cell::Number(100.0 * total_e / m_w),
            ]);
            let mean = total_e / p as f64;
            rows.push(vec![
                text("Mean"),
                alpha(mean),
                Cell::Number(mean),
                Cell::Number(mean / m_w),
                Cell::Number(100.0 * mean / m_w),
            ]);
            let group = vec!["Variance Accounted For".to_string()];
            tables.push(Table {
                title: "Model Summary".into(),
                columns: vec![
                    "Dimension".into(),
                    "Cronbach's Alpha".into(),
                    "Total (Eigenvalue)".into(),
                    "Inertia".into(),
                    "% of Variance".into(),
                ],
                column_groups: vec![Vec::new(), Vec::new(), group.clone(), group.clone(), group],
                rows,
                row_headers: 1,
                caption: Some(
                    "The Total row's alpha comes from the summed eigenvalue, the Mean row's from the mean."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let active_positions: Vec<(usize, f64)> =
            active_vars.iter().map(|&j| (j, avars[j].weight)).collect();
        // the code of each active object on each analysis variable, passive gaps filled
        let filled = |j: usize, i: usize, how: Impute, quantified: Option<usize>| -> f64 {
            let v = &avars[j];
            let pick = |r: usize| match quantified {
                Some(s) => ys[j][r][s],
                None => v.cats[r] as f64,
            };
            match v.h[i] {
                Some(r) => pick(r),
                None => {
                    let mode = (0..v.cats.len()).fold(None, |best: Option<usize>, r| match best {
                        Some(b) if v.d[b] >= v.d[r] => best,
                        _ => Some(r),
                    });
                    match (how, quantified) {
                        (Impute::ExtraCategory, Some(s)) => {
                            let (mut sw, mut sx) = (0.0, 0.0);
                            for (k, h) in v.h.iter().enumerate() {
                                if h.is_none() {
                                    sw += w[k];
                                    sx += w[k] * x[k][s];
                                }
                            }
                            sx / sw
                        }
                        _ => mode.map_or(f64::NAN, pick),
                    }
                }
            }
        };
        let impute_of = |j: usize| match avars[j].missing {
            McaMissing::Passive(how) => how,
            _ => Impute::Mode,
        };
        if print.ocorr {
            let columns: Vec<Vec<f64>> = (0..avars.len())
                .map(|j| (0..n).map(|i| filled(j, i, Impute::Mode, None)).collect())
                .collect();
            tables.push(correlation_table(
                "Correlations Original Variables".into(),
                &analysis_titles,
                &correlations(&columns, w),
                &active_positions,
            ));
        }
        if print.corr {
            for s in 0..p {
                let columns: Vec<Vec<f64>> = (0..avars.len())
                    .map(|j| {
                        (0..n)
                            .map(|i| filled(j, i, impute_of(j), Some(s)))
                            .collect()
                    })
                    .collect();
                tables.push(correlation_table(
                    format!("Correlations Transformed Variables: Dimension {}", s + 1),
                    &analysis_titles,
                    &correlations(&columns, w),
                    &active_positions,
                ));
            }
        }
        if print.discrim {
            let mut columns = vec![String::new()];
            columns.extend(dims.iter().cloned());
            columns.push("Mean".into());
            let mut rows: Vec<Vec<Cell>> = avars
                .iter()
                .enumerate()
                .map(|(j, v)| {
                    let mut row = vec![text(analysis_titles[j].clone())];
                    row.extend(discr[j].iter().map(|d| Cell::Number(*d)));
                    row.push(Cell::Number(discr[j].iter().sum::<f64>() / p as f64));
                    if v.supplementary {
                        row[0] = text(format!("{} (supplementary)", analysis_titles[j]));
                    }
                    row
                })
                .collect();
            if !fixed {
                let mut total = vec![text("Active Total")];
                total.extend(eigen.iter().map(|e| Cell::Number(*e)));
                total.push(Cell::Number(eigen.iter().sum::<f64>() / p as f64));
                rows.push(total);
                let mut share = vec![text("% of Variance")];
                share.extend(inertia.iter().map(|l| Cell::Number(100.0 * l)));
                share.push(Cell::Number(100.0 * inertia.iter().sum::<f64>() / p as f64));
                rows.push(share);
            }
            tables.push(Table {
                title: "Discrimination Measures".into(),
                columns,
                rows,
                row_headers: 1,
                caption: Some(
                    "The Active Total weights each variable by its weight; supplementary variables take no part in it."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let tr_mw: f64 = w.iter().zip(mstar).map(|(a, b)| a * b).sum();
        if let Some(items) = &print.quant {
            let names = match items.is_empty() {
                true => analysis_names.clone(),
                false => resolve(WHO, items, analysis_names, analysis_names)?,
            };
            for name in names {
                let j = analysis_names
                    .iter()
                    .position(|a| a.eq_ignore_ascii_case(&name))
                    .expect("resolved");
                let v = &avars[j];
                let c = &coded[v.coded];
                let mut columns = vec![String::new(), "Frequency".into()];
                let mut groups: Vec<Vec<String>> = vec![Vec::new(), Vec::new()];
                let label_group = |g: &str| vec![g.to_string()];
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(label_group("Centroid Coordinates"));
                }
                columns.push("Mass".into());
                groups.push(Vec::new());
                columns.push("Inertia".into());
                groups.push(Vec::new());
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(label_group("Contribution of Point to Inertia of Dimension"));
                }
                for d in dims.iter().cloned().chain(["Total".to_string()]) {
                    columns.push(d);
                    groups.push(label_group("Contribution of Dimension to Inertia of Point"));
                }
                let mut rows = Vec::new();
                for (r, code) in v.cats.iter().enumerate() {
                    let mut row = vec![text(c.label(*code, None)), Cell::Number(v.d[r])];
                    let q = quantification(j, *code as f64).expect("an active category");
                    row.extend(q.iter().map(|x| Cell::Number(*x)));
                    if v.supplementary || fixed {
                        row.resize(columns.len(), Cell::Empty);
                    } else {
                        let mass = v.weight * v.d[r] / tr_mw;
                        let spread: f64 = (0..n)
                            .filter(|&i| v.h[i] == Some(r))
                            .map(|i| w[i] / mstar[i])
                            .sum();
                        let cat_inertia = v.weight * spread / v.d[r] - mass;
                        let part: Vec<f64> = (0..p)
                            .map(|s| v.weight * v.d[r] * ys[j][r][s].powi(2) / (n_w * m_w))
                            .collect();
                        row.push(Cell::Number(mass));
                        row.push(Cell::Number(cat_inertia));
                        row.extend((0..p).map(|s| num(part[s] / inertia[s])));
                        row.extend(part.iter().map(|x| num(x / cat_inertia)));
                        row.push(num(part.iter().sum::<f64>() / cat_inertia));
                    }
                    rows.push(row);
                }
                for (code, (count, _)) in &extra_cats[j] {
                    let mut row = vec![
                        text(format!("{} (supplementary objects)", c.label(*code, None))),
                        Cell::Number(*count),
                    ];
                    let q = quantification(j, *code as f64).expect("a supplementary category");
                    row.extend(q.iter().map(|x| Cell::Number(*x)));
                    row.resize(columns.len(), Cell::Empty);
                    rows.push(row);
                }
                tables.push(Table {
                    title: format!("Quantifications: {}", c.title),
                    columns,
                    column_groups: groups,
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
        }
        let object_label = |case: usize, label: &Option<usize>| match label {
            Some(l) => coded[*l]
                .codes
                .get(case)
                .copied()
                .flatten()
                .map(|code| coded[*l].label(code as i64, None))
                .unwrap_or_else(|| (case + 1).to_string()),
            None => (case + 1).to_string(),
        };
        if let Some((label, items)) = &print.object {
            let label = match label {
                Some(name) => {
                    let found = resolve(WHO, &[VarItem::Name(name.clone())], variables, variables)?;
                    Some(coded_index(coded, &found[0]))
                }
                None => None,
            };
            let shown_vars: Vec<usize> = resolve(WHO, items, analysis_names, variables)?
                .iter()
                .map(|name| coded_index(coded, name))
                .collect();
            let mut columns = vec![String::new()];
            columns.extend(shown_vars.iter().map(|&k| coded[k].title.clone()));
            columns.extend(dims.iter().cloned());
            let mut rows = Vec::new();
            let mut sups = Vec::new();
            for (case, score) in scores.iter().enumerate() {
                let Some(score) = score else { continue };
                if active.binary_search(&case).is_err() {
                    sups.push((case + 1).to_string());
                }
                let mut row = vec![text(object_label(case, &label))];
                row.extend(shown_vars.iter().map(|&k| match coded[k].codes[case] {
                    Some(code) => Cell::Number(code),
                    None => Cell::Empty,
                }));
                row.extend(normalized(score).into_iter().map(Cell::Number));
                rows.push(row);
            }
            tables.push(Table {
                title: "Object Scores".into(),
                columns,
                rows,
                row_headers: 1,
                caption: (!sups.is_empty())
                    .then(|| format!("Supplementary objects: {}.", sups.join(", "))),
                ..Table::default()
            });
            if !fixed {
                let mut columns = vec![String::new(), "Mass".into(), "Inertia".into()];
                let mut groups: Vec<Vec<String>> = vec![Vec::new(); 3];
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(vec!["Contribution of Point to Inertia of Dimension".into()]);
                }
                for d in dims.iter().cloned().chain(["Total".to_string()]) {
                    columns.push(d);
                    groups.push(vec!["Contribution of Dimension to Inertia of Point".into()]);
                }
                let rows = active
                    .iter()
                    .enumerate()
                    .map(|(i, &case)| {
                        let mass = mstar[i] * w[i] / tr_mw;
                        let spread: f64 = active_vars
                            .iter()
                            .filter_map(|&j| avars[j].h[i].map(|r| avars[j].weight / avars[j].d[r]))
                            .sum();
                        let point_inertia = w[i] * spread / mstar[i] - mass;
                        let part: Vec<f64> = (0..p)
                            .map(|s| mstar[i] * w[i] * x[i][s].powi(2) / (n_w * m_w))
                            .collect();
                        let mut row = vec![
                            text(object_label(case, &label)),
                            Cell::Number(mass),
                            Cell::Number(point_inertia),
                        ];
                        row.extend(part.iter().map(|c| Cell::Number(*c)));
                        let of_point: Vec<f64> = (0..p)
                            .map(|s| part[s] * inertia[s] / point_inertia)
                            .collect();
                        row.extend(of_point.iter().map(|c| num(*c)));
                        row.push(num(of_point.iter().sum()));
                        row
                    })
                    .collect();
                tables.push(Table {
                    title: "Object Contributions".into(),
                    columns,
                    column_groups: groups,
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
        }

        // charts
        let mut charts = Vec::new();
        let ndim = command.plot.ndim;
        if let Some((a, b)) = ndim
            && b > p
        {
            warnings.push(format!(
                "{WHO}: NDIM({a},{b}) goes beyond the {p} dimensions of the solution"
            ));
        }
        let plot_vars = |items: &[VarItem], allowed: &[String]| -> Result<Vec<usize>, Error> {
            Ok(resolve(WHO, items, analysis_names, allowed)?
                .iter()
                .map(|name| coded_index(coded, name))
                .collect())
        };
        let analysis_of = |k: usize| avars.iter().position(|v| v.coded == k);
        let shown_scores: Vec<Option<Vec<f64>>> =
            scores.iter().map(|s| s.as_ref().map(normalized)).collect();
        let object_points = |by: Option<usize>, length: Option<usize>| {
            object_groups(coded, &shown_scores, by, length)
        };
        let category_points = |j: usize, length: Option<usize>| {
            let v = &avars[j];
            let c = &coded[v.coded];
            v.cats
                .iter()
                .copied()
                .chain(extra_cats[j].keys().copied())
                .filter_map(|code| Some((c.label(code, length), quantification(j, code as f64)?)))
                .collect::<Vec<_>>()
        };
        let var_title = |j: usize, length: Option<usize>| {
            let c = &coded[avars[j].coded];
            cut(c.title.clone(), c.name.clone(), length)
        };
        for (chart, length) in &command.plot.charts {
            let length = *length;
            match chart {
                McaPlot::Object(items) => {
                    let by = plot_vars(items, variables)?;
                    if by.is_empty() {
                        charts.extend(scatter(
                            "Object Points",
                            &object_points(None, length),
                            p,
                            ndim,
                        ));
                    }
                    for k in by {
                        let title = format!(
                            "Object Points Labeled by {}",
                            cut(coded[k].title.clone(), coded[k].name.clone(), length)
                        );
                        charts.extend(scatter(&title, &object_points(Some(k), length), p, ndim));
                    }
                }
                McaPlot::Category(items) => {
                    for k in plot_vars(items, analysis_names)? {
                        let j = analysis_of(k).expect("an analysis variable");
                        let title = format!("Category Points: {}", var_title(j, length));
                        let groups = vec![(var_title(j, length), category_points(j, length))];
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                McaPlot::Jointcat(items) => {
                    let ks = match items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(items, analysis_names)?,
                    };
                    let groups: Vec<_> = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), category_points(j, length)))
                        .collect();
                    charts.extend(scatter("Joint Plot of Category Points", &groups, p, ndim));
                }
                McaPlot::Discrim(items) => {
                    let ks: Vec<usize> = match items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(items, analysis_names)?,
                    };
                    let points = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), discr[j].clone()))
                        .collect();
                    let groups = vec![("Variables".to_string(), points)];
                    charts.extend(scatter("Discrimination Measures", &groups, p, ndim));
                }
                McaPlot::Biplot(centroid_items, label_items) => {
                    if command.common.normalization == McaNormalization::Independent {
                        warnings.push(format!(
                            "{WHO}: a biplot is not drawn with NORMALIZATION=INDEPENDENT"
                        ));
                        continue;
                    }
                    let ks: Vec<usize> = match centroid_items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(centroid_items, analysis_names)?,
                    };
                    let centroid_groups: Vec<_> = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), category_points(j, length)))
                        .collect();
                    let by = plot_vars(label_items, variables)?;
                    let labels: Vec<Option<usize>> = match by.is_empty() {
                        true => vec![None],
                        false => by.into_iter().map(Some).collect(),
                    };
                    for k in labels {
                        let title = match k {
                            None => "Biplot".to_string(),
                            Some(k) => format!(
                                "Biplot Labeled by {}",
                                cut(coded[k].title.clone(), coded[k].name.clone(), length)
                            ),
                        };
                        let mut groups = object_points(k, length);
                        groups.extend(centroid_groups.iter().cloned());
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                McaPlot::Trans(items) | McaPlot::Resid(items) => {
                    let trans = matches!(chart, McaPlot::Trans(_));
                    for (item, count) in items {
                        for name in resolve(
                            WHO,
                            std::slice::from_ref(item),
                            analysis_names,
                            analysis_names,
                        )? {
                            let j = analysis_names
                                .iter()
                                .position(|a| a.eq_ignore_ascii_case(&name))
                                .expect("resolved");
                            let v = &avars[j];
                            let c = &coded[v.coded];
                            if *count > p {
                                warnings.push(format!(
                                    "{WHO}: {name} asks for {count} dimensions in a plot, but the solution has {p}"
                                ));
                            }
                            for s in 0..(*count).min(p) {
                                let title = format!(
                                    "{}: {}, Dimension {}",
                                    if trans { "Transformation" } else { "Residuals" },
                                    var_title(j, if trans { None } else { length }),
                                    s + 1
                                );
                                if trans {
                                    let codes: Vec<i64> = v
                                        .cats
                                        .iter()
                                        .copied()
                                        .chain(extra_cats[j].keys().copied())
                                        .collect::<std::collections::BTreeSet<_>>()
                                        .into_iter()
                                        .collect();
                                    let values = codes
                                        .iter()
                                        .map(|code| {
                                            quantification(j, *code as f64)
                                                .map_or(f64::NAN, |q| q[s])
                                        })
                                        .collect();
                                    charts.push(transformation_chart(
                                        title,
                                        c,
                                        &codes,
                                        &format!("Dimension {}", s + 1),
                                        values,
                                    ));
                                } else {
                                    let points = active
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(i, &case)| {
                                            let r = v.h[i]?;
                                            Some(Point {
                                                x: x[i][s] * x_factor[s],
                                                y: ys[j][r][s] * y_factor[s],
                                                z: None,
                                                label: Some((case + 1).to_string()),
                                            })
                                        })
                                        .collect();
                                    charts.push(point_chart(
                                        title,
                                        "Approximation",
                                        "Quantification",
                                        c.title.clone(),
                                        points,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // the values of each case for SAVE and OUTFILE
        let transformed = |j: usize, s: usize| -> Vec<Option<f64>> {
            (0..n_tot)
                .map(|c| {
                    if excluded[c] {
                        return None;
                    }
                    let code = avars[j].codes[c]?;
                    quantification(j, code).map(|q| q[s])
                })
                .collect()
        };
        let object_column = |s: usize| -> Vec<Option<f64>> {
            scores
                .iter()
                .map(|score| score.as_ref().map(|sc| sc[s] * x_factor[s]))
                .collect()
        };
        let dims_of = |asked: Option<usize>, what: &str, warnings: &mut Vec<String>| match asked {
            Some(k) if k > p => {
                warnings.push(format!(
                    "{WHO}: SAVE {what} asks for {k} dimensions, but the solution has {p}"
                ));
                p
            }
            Some(k) => k,
            None => p,
        };
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if let Some((root, asked)) = &command.save_trdata {
            let count = dims_of(*asked, "TRDATA", &mut warnings);
            let names = |set: usize| -> Vec<String> {
                (1..=avars.len())
                    .flat_map(|k| (1..=count).map(move |s| format!("{root}{k}_{s}_{set}")))
                    .collect()
            };
            let set = free_set(data, &names);
            for (j, v) in avars.iter().enumerate() {
                for s in 0..count {
                    saves.push((
                        format!("{root}{}_{}_{set}", j + 1, s + 1),
                        format!(
                            "Quantified {} in dimension {} ({WHO} {set})",
                            coded[v.coded].name,
                            s + 1
                        ),
                        transformed(j, s),
                    ));
                }
            }
        }
        if let Some((root, asked)) = &command.save_object {
            let count = dims_of(*asked, "OBJECT", &mut warnings);
            let names = |set: usize| -> Vec<String> {
                (1..=count).map(|s| format!("{root}{s}_{set}")).collect()
            };
            let set = free_set(data, &names);
            for s in 0..count {
                saves.push((
                    format!("{root}{}_{set}", s + 1),
                    format!("Object score in dimension {} ({WHO} {set})", s + 1),
                    object_column(s),
                ));
            }
        }
        let mut files: Vec<(String, Dataset)> = Vec::new();
        if let Some(file) = &command.outfile_discrdata {
            let mut out = Dataset::new(n_tot);
            for v in avars {
                let c = &coded[v.coded];
                let values = (0..n_tot)
                    .map(|case| c.codes[case].filter(|_| !excluded[case]))
                    .collect();
                out.set_numeric_values(&c.name, values)?;
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_trdata {
            let mut out = Dataset::new(n_tot);
            for j in 0..avars.len() {
                for s in 0..p {
                    out.set_numeric_values(&format!("TRA{}_{}", j + 1, s + 1), transformed(j, s))?;
                }
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_object {
            let mut out = Dataset::new(n_tot);
            for s in 0..p {
                out.set_numeric_values(&format!("OBSCO{}", s + 1), object_column(s))?;
            }
            files.push((file.clone(), out));
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
        for (file, out) in files {
            self.write_data_file(&file, out)?;
        }
        // HOMALS MATRIX=OUT: the quantification of every category
        if let Some(target) = command
            .common
            .legacy
            .as_ref()
            .and_then(|l| l.matrix_out.as_ref())
        {
            let mut rows = Vec::new();
            for (j, v) in avars.iter().enumerate() {
                for &code in &v.cats {
                    rows.push(crate::legacy_scaling::MatrixRow {
                        rowtype: "QUANT",
                        level: coded[v.coded].label(code, None),
                        varname: coded[v.coded].name.clone(),
                        vartype: None,
                        set: None,
                        dims: quantification(j, code as f64).unwrap_or_default(),
                    });
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
