//! GENLOG: general loglinear and logit models of a table of counts, fitted by Newton-Raphson.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Factors are numeric; their categories are the values of the cases used, sorted, and the
//!   table has every combination. A case is used when every factor, covariate and CSTRUCTURE,
//!   GRESID and GLOR variable is valid (INCLUDE keeps user-missing values).
//! - A cell's count sums the case weights of its cases whose CSTRUCTURE value is above 0. Its
//!   cell weight, covariates and GRESID and GLOR coefficients are the means over those cases,
//!   weighted by the case weights (plain means when the weights sum to 0); a cell whose cases
//!   all have a cell weight of 0 or less is a structural zero, and a cell without cases has
//!   cell weight 1 and the other values 0. Structural zeros are left out of the fit and of the
//!   cell statistics.
//! - Dummy coding: a factor has a column for every category, `a(n)` one for category n only,
//!   an interaction the products, a covariate its values. The constant comes first; under
//!   MULTINOMIAL it, and in a logit model one column for every combination of the factors
//!   after BY, are fixed by the totals and are not parameters. Sweeping the cross-products of
//!   the columns in order finds those that repeat earlier ones (EPS); they are redundant and 0.
//!   A logit term of BY factors alone is dropped. Without DESIGN the model is saturated
//!   (every term of the factors); with covariates DESIGN is required.
//! - log m = log z + x'b. The start is m = count + DELTA; a saturated model (as many
//!   parameters as cells in use) is fitted to the counts plus DELTA. Each Newton-Raphson step
//!   solves (X'MX) b = X'(m log(m / z) + n - m). It stops when every fitted value changes by
//!   less than CONVERGE, either absolutely or relative to its value, and the root mean square
//!   of the score of the parameters other than the constant is below CONVERGE; at most ITERATE
//!   steps.
//! - The covariance of the estimates is (X'MX)^-1. Pearson chi-square; likelihood ratio
//!   2 sum(n log(n/m) - (n - m)) (Poisson) or 2 sum n log(n/m) (multinomial). df: cells less
//!   the non-redundant columns less the structural zeros and cells fitted as 0.
//! - Residuals: simple n - m; standardized (n - m) / sqrt(m), under MULTINOMIAL with m (1 - m /
//!   N_j) inside the root (N_j the total of the cell's BY combination); adjusted (n - m) /
//!   sqrt(m (1 - h)) with h the leverage m x'(X'MX)^-1 x; deviance sign(n - m) times the root
//!   of 2 (n log(n/m) - (n - m)), 2m for an empty cell.
//! - GRESID: for coefficients d, sum d n, sum d m, their difference, over sqrt(sum d^2 m)
//!   (multinomial: sum over the BY combinations of sum d^2 m - (sum d m)^2 / N_j) and over the
//!   root of its exact variance d'Md - (Md)'X(X'MX)^-1 X'(Md).
//! - GLOR: coefficients summing to 0 (in each BY combination of a logit model); estimate sum d
//!   log m, variance w'(X'MX)^-1 w with w = X'd, Wald chi-square on 1 df, CIN interval.
//! - Logit models add the analysis of dispersion (entropy and concentration).

use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_p, normal_quantile};
use crate::glm::{ginverse, sweep_in_order};
use crate::hiloglinear::{MAX_CELLS, cell_levels, hat_values, normal_plots, residual_by_counts};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Measure;
use statscraft_syntax::{Genlog, GenlogSaveKind, VarItem};

struct Column {
    label: String,
    /// The value in every cell.
    values: Vec<f64>,
    /// Fixed by the multinomial totals: not a parameter.
    nuisance: bool,
    /// The constant.
    constant: bool,
    /// A term of dependent factors alone.
    dependent_only: bool,
}

/// One Newton-Raphson step's change, for PRINT ITERATION.
struct Step {
    absolute: f64,
    relative: f64,
    score: f64,
}

struct Fitted {
    m: Vec<f64>,
    beta: Vec<f64>,
    /// (X'MX)^-1 over the non-redundant columns.
    cov: Matrix,
    steps: Vec<Step>,
    converged: bool,
}

/// A saved variable: its root, the name given, its label and the value of every case.
type Saved = (String, Option<String>, &'static str, Vec<Option<f64>>);

fn exp_capped(v: f64) -> f64 {
    v.min(700.0).exp()
}

impl Engine {
    pub(crate) fn genlog(&mut self, command: &Genlog) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let c = command.criteria;
        let expand = |items: &[VarItem]| -> Result<Vec<String>, Error> {
            if items.is_empty() {
                Ok(Vec::new())
            } else {
                self.expand(items)
            }
        };
        let dependents = expand(&command.dependents)?;
        let independents = expand(&command.independents)?;
        let covariates = expand(&command.covariates)?;
        let gresid = expand(&command.gresid)?;
        let glor = expand(&command.glor)?;
        let factors: Vec<String> = dependents.iter().chain(&independents).cloned().collect();
        let mut seen: Vec<&String> = Vec::new();
        for name in factors.iter().chain(&covariates) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                return Err(Error(format!(
                    "GENLOG names {name} more than once among the factors and covariates"
                )));
            }
            seen.push(name);
        }
        if factors.len() > 10 {
            return Err(Error("GENLOG takes at most 10 factors".into()));
        }
        if covariates.len() > 200 {
            return Err(Error("GENLOG takes at most 200 covariates".into()));
        }
        let extra: Vec<&String> = command
            .cstructure
            .iter()
            .chain(&gresid)
            .chain(&glor)
            .collect();
        for name in &extra {
            if factors.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                return Err(Error(format!(
                    "{name} is a factor and cannot also be on CSTRUCTURE, GRESID or GLOR"
                )));
            }
        }
        let all_names: Vec<&String> = factors
            .iter()
            .chain(&covariates)
            .chain(extra.iter().copied())
            .collect();
        let vars: Vec<_> = all_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "GENLOG variables are numeric, but {} is a string variable",
                v.name()
            )));
        }
        let columns_of: Vec<_> = vars.iter().map(|v| v.numeric()).collect::<Result<_, _>>()?;
        let nf = factors.len();
        let nc = covariates.len();
        let cs_at = nf + nc;
        let has_cs = command.cstructure.is_some();
        let gr_at = cs_at + usize::from(has_cs);
        let gl_at = gr_at + gresid.len();
        let weights = case_weights(data)?;
        let cases = data.case_count();
        // the cases used and their values
        let mut used: Vec<(usize, Vec<f64>)> = Vec::new();
        let mut missing = 0.0;
        for k in 0..cases {
            let values: Option<Vec<f64>> = vars
                .iter()
                .zip(&columns_of)
                .map(|(v, col)| {
                    col.get(k)
                        .filter(|x| command.include || !v.is_user_missing(*x))
                })
                .collect();
            match values {
                Some(values) => used.push((k, values)),
                None => missing += 1.0,
            }
        }
        if used.is_empty() {
            return Err(Error(
                "GENLOG: no case has valid values on every variable".into(),
            ));
        }
        let categories: Vec<Vec<f64>> = (0..nf)
            .map(|f| {
                let mut values: Vec<f64> = used.iter().map(|(_, v)| v[f]).collect();
                values.sort_by(f64::total_cmp);
                values.dedup();
                values
            })
            .collect();
        let levels: Vec<usize> = categories.iter().map(Vec::len).collect();
        let cells = levels
            .iter()
            .try_fold(1usize, |a, &l| a.checked_mul(l))
            .filter(|&c| c <= MAX_CELLS)
            .ok_or_else(|| {
                Error(format!(
                    "the GENLOG table would have more than {MAX_CELLS} cells"
                ))
            })?;
        // aggregation
        let extras = nc + gresid.len() + glor.len();
        let mut v_count = vec![0.0; cells];
        let mut v_plus = vec![0.0; cells];
        let mut n = vec![0.0; cells];
        let mut z_w = vec![0.0; cells];
        let mut z_plain = vec![0.0; cells];
        let mut x_w = vec![vec![0.0; extras]; cells];
        let mut x_plain = vec![vec![0.0; extras]; cells];
        let mut case_cell: Vec<Option<usize>> = vec![None; cases];
        let mut weighted_valid = 0.0;
        for (k, values) in &used {
            let index = (0..nf).fold(0, |a, f| {
                let at = categories[f]
                    .binary_search_by(|c| c.total_cmp(&values[f]))
                    .expect("a category of the cases used");
                a * levels[f] + at
            });
            case_cell[*k] = Some(index);
            v_count[index] += 1.0;
            let z = if has_cs { values[cs_at] } else { 1.0 };
            if z <= 0.0 {
                continue;
            }
            let w = weights.as_ref().map_or(1.0, |w| w[*k]).max(0.0);
            weighted_valid += w;
            v_plus[index] += 1.0;
            n[index] += w;
            z_w[index] += w * z;
            z_plain[index] += z;
            let others = values[nf..nf + nc]
                .iter()
                .chain(&values[gr_at..gl_at + glor.len()]);
            for (e, &x) in others.enumerate() {
                x_w[index][e] += w * x;
                x_plain[index][e] += x;
            }
        }
        let z: Vec<f64> = (0..cells)
            .map(|i| {
                if v_count[i] == 0.0 {
                    1.0
                } else if v_plus[i] == 0.0 {
                    0.0
                } else if n[i] > 0.0 {
                    z_w[i] / n[i]
                } else {
                    z_plain[i] / v_plus[i]
                }
            })
            .collect();
        let cell_extra = |i: usize, e: usize| {
            if v_count[i] == 0.0 || v_plus[i] == 0.0 {
                0.0
            } else if n[i] > 0.0 {
                x_w[i][e] / n[i]
            } else {
                x_plain[i][e] / v_plus[i]
            }
        };
        let in_use: Vec<bool> = z.iter().map(|&z| z > 0.0).collect();
        let used_cells = in_use.iter().filter(|&&u| u).count();
        if used_cells == 0 {
            return Err(Error("GENLOG: every cell is a structural zero".into()));
        }
        let factor_vars = &vars[..nf];
        let category_text = |f: usize, l: usize| shown.number(&factor_vars[f], categories[f][l]);
        let cell_text = |i: usize| -> Vec<String> {
            cell_levels(i, &levels)
                .iter()
                .enumerate()
                .map(|(f, &l)| category_text(f, l))
                .collect()
        };
        // the design
        let logit = !independents.is_empty();
        let n_dep = dependents.len();
        let multinomial = command.multinomial;
        let mut design_columns: Vec<Column> = vec![Column {
            label: "Constant".into(),
            values: vec![1.0; cells],
            nuisance: multinomial,
            constant: true,
            dependent_only: false,
        }];
        let cell_lv: Vec<Vec<usize>> = (0..cells).map(|i| cell_levels(i, &levels)).collect();
        if logit {
            let ind_levels: Vec<usize> = levels[n_dep..].to_vec();
            let combos: usize = ind_levels.iter().product();
            for combo in 0..combos {
                let lv = cell_levels(combo, &ind_levels);
                design_columns.push(Column {
                    label: String::new(),
                    values: cell_lv
                        .iter()
                        .map(|cl| f64::from(u8::from(cl[n_dep..] == lv[..])))
                        .collect(),
                    nuisance: true,
                    constant: false,
                    dependent_only: false,
                });
            }
        }
        // each part: a factor (with its category) or a covariate
        enum Part {
            Factor(usize, Option<usize>),
            Covariate(usize),
        }
        let terms: Vec<Vec<Part>> = match &command.design {
            None => {
                if nc > 0 {
                    return Err(Error(
                        "GENLOG with covariates needs DESIGN to name the model's terms".into(),
                    ));
                }
                let mut masks: Vec<u32> = (1..(1u32 << nf)).collect();
                masks.sort_by_key(|&t| (t.count_ones(), std::cmp::Reverse(t.reverse_bits())));
                masks
                    .into_iter()
                    .map(|t| {
                        (0..nf)
                            .filter(|f| t & (1 << f) != 0)
                            .map(|f| Part::Factor(f, None))
                            .collect()
                    })
                    .collect()
            }
            Some(terms) => {
                let mut out = Vec::new();
                for term in terms {
                    let mut parts = Vec::new();
                    for part in term {
                        if let Some(f) = factors
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(&part.name))
                        {
                            let category = match part.category {
                                None => None,
                                Some(c) if (c as usize) <= levels[f] => Some(c as usize - 1),
                                Some(c) => {
                                    return Err(Error(format!(
                                        "GENLOG DESIGN asks for category {c} of {}, which has {}",
                                        factors[f], levels[f]
                                    )));
                                }
                            };
                            parts.push(Part::Factor(f, category));
                        } else if let Some(v) = covariates
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(&part.name))
                        {
                            if part.category.is_some() {
                                return Err(Error(format!(
                                    "GENLOG DESIGN: the covariate {} takes no category",
                                    part.name
                                )));
                            }
                            parts.push(Part::Covariate(v));
                        } else {
                            return Err(Error(format!(
                                "GENLOG DESIGN names {}, which is neither a factor nor a covariate of the command",
                                part.name
                            )));
                        }
                    }
                    out.push(parts);
                }
                out
            }
        };
        let mut design_labels = vec![if multinomial {
            None
        } else {
            Some("Constant".to_string())
        }];
        for parts in &terms {
            let by_only = parts
                .iter()
                .all(|p| matches!(p, Part::Factor(f, _) if *f >= n_dep));
            if logit && by_only {
                continue;
            }
            let dependent_only = parts
                .iter()
                .all(|p| matches!(p, Part::Factor(f, _) if *f < n_dep));
            design_labels.push(Some(
                parts
                    .iter()
                    .map(|p| match p {
                        Part::Factor(f, None) => factors[*f].clone(),
                        Part::Factor(f, Some(c)) => format!("{}({})", factors[*f], c + 1),
                        Part::Covariate(v) => covariates[*v].clone(),
                    })
                    .collect::<Vec<_>>()
                    .join("*"),
            ));
            // the product of the parts' columns, the first part changing slowest
            let mut product: Vec<(String, Vec<f64>)> = vec![(String::new(), vec![1.0; cells])];
            for part in parts {
                let options: Vec<(String, Vec<f64>)> = match part {
                    Part::Factor(f, category) => {
                        let chosen: Vec<usize> = match category {
                            Some(c) => vec![*c],
                            None => (0..levels[*f]).collect(),
                        };
                        chosen
                            .into_iter()
                            .map(|l| {
                                (
                                    format!("[{} = {}]", factors[*f], category_text(*f, l)),
                                    cell_lv
                                        .iter()
                                        .map(|cl| f64::from(u8::from(cl[*f] == l)))
                                        .collect(),
                                )
                            })
                            .collect()
                    }
                    Part::Covariate(v) => vec![(
                        covariates[*v].clone(),
                        (0..cells).map(|i| cell_extra(i, *v)).collect(),
                    )],
                };
                product = product
                    .iter()
                    .flat_map(|(label, values)| {
                        options.iter().map(move |(l, o)| {
                            let label = if label.is_empty() {
                                l.clone()
                            } else {
                                format!("{label} * {l}")
                            };
                            (label, values.iter().zip(o).map(|(a, b)| a * b).collect())
                        })
                    })
                    .collect();
            }
            for (label, values) in product {
                design_columns.push(Column {
                    label,
                    values,
                    nuisance: false,
                    constant: false,
                    dependent_only,
                });
            }
        }
        let ncol = design_columns.len();
        // shortcut: dense linear algebra; a larger design would take minutes and gigabytes
        if ncol > 3000 {
            return Err(Error(format!(
                "GENLOG: the design has {ncol} columns, more than the 3000 this procedure handles"
            )));
        }
        // redundancy over the cells in use
        let mut gram = vec![vec![0.0; ncol]; ncol];
        for a in 0..ncol {
            for b in a..ncol {
                let s: f64 = (0..cells)
                    .filter(|&i| in_use[i])
                    .map(|i| design_columns[a].values[i] * design_columns[b].values[i])
                    .sum();
                gram[a][b] = s;
                gram[b][a] = s;
            }
        }
        let order: Vec<usize> = (0..ncol).collect();
        let (_, swept) = sweep_in_order(&gram, &order, c.eps);
        let kept: Vec<usize> = (0..ncol).filter(|&j| swept[j]).collect();
        let x: Vec<Vec<f64>> = (0..cells)
            .map(|i| kept.iter().map(|&j| design_columns[j].values[i]).collect())
            .collect();
        let saturated = kept.len() >= used_cells;
        let n_fit: Vec<f64> = (0..cells)
            .map(|i| {
                if saturated && in_use[i] {
                    n[i] + c.delta
                } else {
                    n[i]
                }
            })
            .collect();
        let scored: Vec<usize> = (0..kept.len())
            .filter(|&k| {
                let col = &design_columns[kept[k]];
                !col.nuisance && !col.constant
            })
            .collect();
        let fitted = newton(
            &x,
            &z,
            &in_use,
            &n_fit,
            if saturated { 0.0 } else { c.delta },
            c.converge,
            c.iterate as usize,
            &scored,
        );
        let m = &fitted.m;
        let mut warnings = Vec::new();
        if !fitted.converged {
            warnings.push(format!(
                "GENLOG: the estimates did not converge in {} iterations",
                c.iterate
            ));
        }
        // totals of each BY combination (the whole table without BY)
        let ind_levels: Vec<usize> = levels[n_dep..].to_vec();
        let settings: usize = ind_levels.iter().product::<usize>().max(1);
        let setting_of = |i: usize| -> usize { if logit { i % settings } else { 0 } };
        let mut setting_total = vec![0.0; settings];
        for i in (0..cells).filter(|&i| in_use[i]) {
            setting_total[setting_of(i)] += n_fit[i];
        }
        let zero_fitted = (0..cells).filter(|&i| !in_use[i] || m[i] <= 0.0).count();
        let df = cells as f64 - kept.len() as f64 - zero_fitted as f64;
        let mut pearson = Some(0.0);
        let mut lr = Some(0.0);
        for i in (0..cells).filter(|&i| in_use[i]) {
            let (o, e) = (n_fit[i], m[i]);
            if e > 0.0 {
                pearson = pearson.map(|p| p + (o - e) * (o - e) / e);
                let g = match (o > 0.0, multinomial) {
                    (true, false) => o * (o / e).ln() - (o - e),
                    (true, true) => o * (o / e).ln(),
                    (false, false) => e,
                    (false, true) => 0.0,
                };
                lr = lr.map(|l| l + 2.0 * g);
            } else if o > 0.0 {
                pearson = None;
                lr = None;
            }
        }
        // cell statistics
        let hat = hat_values(&x, m, &in_use);
        let resid: Vec<Option<f64>> = (0..cells)
            .map(|i| in_use[i].then(|| n_fit[i] - m[i]))
            .collect();
        let zresid: Vec<Option<f64>> = (0..cells)
            .map(|i| {
                let (o, e) = (n_fit[i], m[i]);
                if !in_use[i] {
                    None
                } else if o == e {
                    Some(0.0)
                } else if multinomial {
                    let nj = setting_total[setting_of(i)];
                    (e > 0.0 && e < nj).then(|| (o - e) / (e * (1.0 - e / nj)).sqrt())
                } else {
                    (e > 0.0).then(|| (o - e) / e.sqrt())
                }
            })
            .collect();
        let adjresid: Vec<Option<f64>> = (0..cells)
            .map(|i| {
                let (o, e) = (n_fit[i], m[i]);
                if !in_use[i] {
                    None
                } else if o == e {
                    Some(0.0)
                } else {
                    let v = e * (1.0 - hat[i]);
                    (e > 0.0 && v > 1e-12 * e).then(|| (o - e) / v.sqrt())
                }
            })
            .collect();
        let dev: Vec<Option<f64>> = (0..cells)
            .map(|i| {
                let (o, e) = (n_fit[i], m[i]);
                if !in_use[i] {
                    None
                } else if o == e {
                    Some(0.0)
                } else if o > 0.0 && e > 0.0 {
                    let d = (2.0 * (o * (o / e).ln() - (o - e))).max(0.0);
                    Some((o - e).signum() * d.sqrt())
                } else if o == 0.0 {
                    Some(-(2.0 * e).sqrt())
                } else {
                    None
                }
            })
            .collect();
        let mut tables = Vec::new();
        // data information
        let structural = (0..cells).filter(|&i| !in_use[i]).count();
        let sampling = (0..cells).filter(|&i| in_use[i] && n[i] == 0.0).count();
        let mut rows = vec![
            vec![
                text("Cases"),
                text("Valid"),
                Cell::Number(used.len() as f64),
            ],
            vec![text(""), text("Missing"), Cell::Number(missing)],
            vec![
                text(""),
                text("Weighted valid"),
                Cell::Number(weighted_valid),
            ],
            vec![
                text("Cells"),
                text("Defined cells"),
                Cell::Number(cells as f64),
            ],
            vec![
                text(""),
                text("Structural zeros"),
                Cell::Number(structural as f64),
            ],
            vec![
                text(""),
                text("Sampling zeros"),
                Cell::Number(sampling as f64),
            ],
        ];
        for (f, v) in factor_vars.iter().enumerate() {
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
        let design_text = design_labels
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(" + ");
        tables.push(table(
            "Model Information",
            1,
            plain(&["", "Value"]),
            vec![
                vec![
                    text("Model"),
                    text(match (multinomial, logit) {
                        (false, _) => "Poisson",
                        (true, false) => "Multinomial",
                        (true, true) => "Multinomial logit",
                    }),
                ],
                vec![text("Design"), text(design_text)],
            ],
            None,
        ));
        let last = fitted.steps.last();
        tables.push(table(
            "Convergence Information",
            1,
            plain(&["", "Value"]),
            vec![
                vec![
                    text("Maximum number of iterations"),
                    Cell::Number(f64::from(c.iterate)),
                ],
                vec![text("Convergence tolerance"), Cell::Number(c.converge)],
                vec![
                    text("Final maximum absolute difference"),
                    number(last.map(|s| s.absolute)),
                ],
                vec![
                    text("Final maximum relative difference"),
                    number(last.map(|s| s.relative)),
                ],
                vec![
                    text("Number of iterations"),
                    Cell::Number(fitted.steps.len() as f64),
                ],
            ],
            Some(if fitted.converged {
                "The iteration converged.".into()
            } else {
                "The iteration did not converge.".into()
            }),
        ));
        if command.print.iteration {
            tables.push(table(
                "Iteration History",
                1,
                plain(&[
                    "Iteration",
                    "Maximum absolute difference",
                    "Maximum relative difference",
                    "Root mean square score",
                ]),
                fitted
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(s, step)| {
                        vec![
                            Cell::Number(s as f64 + 1.0),
                            Cell::Number(step.absolute),
                            Cell::Number(step.relative),
                            Cell::Number(step.score),
                        ]
                    })
                    .collect(),
                None,
            ));
        }
        let sig = |v: Option<f64>| number(v.filter(|_| df > 0.0).map(|v| chi_square_p(v, df)));
        tables.push(table(
            "Goodness-of-Fit Tests",
            1,
            plain(&["", "Value", "df", "Sig."]),
            vec![
                vec![
                    text("Likelihood Ratio"),
                    number(lr),
                    Cell::Number(df),
                    sig(lr),
                ],
                vec![
                    text("Pearson Chi-Square"),
                    number(pearson),
                    Cell::Number(df),
                    sig(pearson),
                ],
            ],
            None,
        ));
        let p = command.print;
        let delta_note = (saturated && c.delta > 0.0).then(|| {
            format!(
                "The model is saturated, so it is fitted to the counts plus {}, which the observed counts show.",
                c.delta
            )
        });
        if p.freq || p.resid || p.zresid || p.adjresid || p.dev {
            let total_o: f64 = (0..cells).filter(|&i| in_use[i]).map(|i| n_fit[i]).sum();
            let total_e: f64 = (0..cells).filter(|&i| in_use[i]).map(|i| m[i]).sum();
            let mut columns: Vec<(String, String)> = factor_vars
                .iter()
                .map(|v| (String::new(), shown.variable(v)))
                .collect();
            if p.freq {
                for g in ["Observed", "Expected"] {
                    columns.push((g.into(), "Count".into()));
                    columns.push((g.into(), "%".into()));
                }
            }
            let picked: Vec<(&str, &Vec<Option<f64>>)> = [
                (p.resid, "Residual", &resid),
                (p.zresid, "Standardized Residual", &zresid),
                (p.adjresid, "Adjusted Residual", &adjresid),
                (p.dev, "Deviance", &dev),
            ]
            .into_iter()
            .filter(|(on, _, _)| *on)
            .map(|(_, name, values)| (name, values))
            .collect();
            columns.extend(
                picked
                    .iter()
                    .map(|(name, _)| (String::new(), name.to_string())),
            );
            let rows = (0..cells)
                .map(|i| {
                    let mut row: Vec<Cell> = cell_text(i).into_iter().map(text).collect();
                    if p.freq {
                        let o = if in_use[i] { n_fit[i] } else { n[i] };
                        row.push(Cell::Number(o));
                        row.push(number(
                            (in_use[i] && total_o > 0.0).then(|| 100.0 * o / total_o),
                        ));
                        row.push(number(in_use[i].then_some(m[i])));
                        row.push(number(
                            (in_use[i] && total_e > 0.0).then(|| 100.0 * m[i] / total_e),
                        ));
                    }
                    row.extend(picked.iter().map(|(_, values)| number(values[i])));
                    row
                })
                .collect();
            let mut notes: Vec<String> = delta_note.iter().cloned().collect();
            if structural > 0 {
                notes.push("Cells without an expected count are structural zeros.".into());
            }
            tables.push(table(
                "Cell Counts and Residuals",
                nf,
                columns,
                rows,
                (!notes.is_empty()).then(|| notes.join(" ")),
            ));
        }
        // the parameters shown: every column but the nuisance ones
        let shown_cols: Vec<usize> = (0..ncol).filter(|&j| !design_columns[j].nuisance).collect();
        let kept_at = |j: usize| kept.iter().position(|&k| k == j);
        if p.design {
            let mut columns: Vec<(String, String)> = factor_vars
                .iter()
                .map(|v| (String::new(), shown.variable(v)))
                .collect();
            let listed: Vec<usize> = shown_cols
                .iter()
                .copied()
                .filter(|&j| kept_at(j).is_some())
                .collect();
            columns.extend(
                listed
                    .iter()
                    .map(|&j| (String::new(), design_columns[j].label.clone())),
            );
            let rows = (0..cells)
                .map(|i| {
                    let mut row: Vec<Cell> = cell_text(i).into_iter().map(text).collect();
                    row.extend(
                        listed
                            .iter()
                            .map(|&j| Cell::Number(design_columns[j].values[i])),
                    );
                    row
                })
                .collect();
            tables.push(table("Design Matrix", nf, columns, rows, None));
        }
        let level = c.cin;
        let zq = normal_quantile(0.5 + level / 200.0);
        let ci = format!("{}% Confidence Interval", trim_number(level));
        if p.estim {
            let mut any_redundant = false;
            let rows = shown_cols
                .iter()
                .map(|&j| {
                    let label = text(design_columns[j].label.clone());
                    match kept_at(j) {
                        Some(k) => {
                            let b = fitted.beta[k];
                            let se = fitted.cov[k][k].max(0.0).sqrt();
                            let zv = b / se;
                            vec![
                                label,
                                Cell::Number(b),
                                Cell::Number(se),
                                number(Some(zv)),
                                number(Some(normal_p(zv))),
                                Cell::Number(b - zq * se),
                                Cell::Number(b + zq * se),
                            ]
                        }
                        None => {
                            any_redundant = true;
                            let mut row = vec![label, Cell::Number(0.0)];
                            row.extend(std::iter::repeat_n(Cell::Empty, 5));
                            row
                        }
                    }
                })
                .collect();
            let mut notes: Vec<String> = Vec::new();
            if any_redundant {
                notes.push("A parameter of 0 without a standard error is redundant.".into());
            }
            if multinomial {
                notes.push(
                    "Under the multinomial model the constant (and in a logit model the terms of the BY factors) only fix the totals and are not shown."
                        .into(),
                );
            }
            if let Some(d) = &delta_note {
                notes.push(d.clone());
            }
            tables.push(table(
                "Parameter Estimates",
                1,
                vec![
                    (String::new(), "Parameter".into()),
                    (String::new(), "Estimate".into()),
                    (String::new(), "Std. Error".into()),
                    (String::new(), "Z".into()),
                    (String::new(), "Sig.".into()),
                    (ci.clone(), "Lower Bound".into()),
                    (ci.clone(), "Upper Bound".into()),
                ],
                rows,
                (!notes.is_empty()).then(|| notes.join(" ")),
            ));
        }
        let listed: Vec<(usize, usize)> = shown_cols
            .iter()
            .filter_map(|&j| kept_at(j).map(|k| (j, k)))
            .collect();
        for (on, title, correlation) in [
            (p.cov, "Covariance of Parameter Estimates", false),
            (p.corr, "Correlations of Parameter Estimates", true),
        ] {
            if !on {
                continue;
            }
            let mut columns = plain(&[""]);
            columns.extend(
                listed
                    .iter()
                    .map(|&(j, _)| (String::new(), design_columns[j].label.clone())),
            );
            let rows = listed
                .iter()
                .map(|&(j, a)| {
                    let mut row = vec![text(design_columns[j].label.clone())];
                    row.extend(listed.iter().map(|&(_, b)| {
                        let v = fitted.cov[a][b];
                        if correlation {
                            let d = (fitted.cov[a][a] * fitted.cov[b][b]).sqrt();
                            number((d > 0.0).then(|| v / d))
                        } else {
                            Cell::Number(v)
                        }
                    }));
                    row
                })
                .collect();
            tables.push(table(title, 1, columns, rows, None));
        }
        // X'(M d) for a coefficient vector d
        let xm = |d: &[f64]| -> Vec<f64> {
            (0..kept.len())
                .map(|k| {
                    (0..cells)
                        .filter(|&i| in_use[i])
                        .map(|i| x[i][k] * m[i] * d[i])
                        .sum()
                })
                .collect()
        };
        let quadratic = |w: &[f64]| -> f64 {
            (0..w.len())
                .map(|a| w[a] * (0..w.len()).map(|b| fitted.cov[a][b] * w[b]).sum::<f64>())
                .sum()
        };
        if !gresid.is_empty() {
            let mut rows = Vec::new();
            for (g, name) in gresid.iter().enumerate() {
                let d: Vec<f64> = (0..cells)
                    .map(|i| {
                        if in_use[i] {
                            cell_extra(i, nc + g)
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let observed: f64 = (0..cells).map(|i| d[i] * n_fit[i]).sum();
                let expected: f64 = (0..cells).map(|i| d[i] * m[i]).sum();
                let simple = observed - expected;
                let var_std = if multinomial {
                    let mut by = vec![(0.0, 0.0); settings];
                    for i in (0..cells).filter(|&i| in_use[i]) {
                        by[setting_of(i)].0 += d[i] * d[i] * m[i];
                        by[setting_of(i)].1 += d[i] * m[i];
                    }
                    by.iter()
                        .zip(&setting_total)
                        .map(|(&(sq, s), &nj)| if nj > 0.0 { sq - s * s / nj } else { 0.0 })
                        .sum()
                } else {
                    (0..cells).map(|i| d[i] * d[i] * m[i]).sum::<f64>()
                };
                let v_adj =
                    (0..cells).map(|i| d[i] * d[i] * m[i]).sum::<f64>() - quadratic(&xm(&d));
                rows.push(vec![
                    text(name.clone()),
                    Cell::Number(observed),
                    Cell::Number(expected),
                    Cell::Number(simple),
                    number((var_std > 0.0).then(|| simple / var_std.sqrt())),
                    number(
                        (v_adj > 1e-12 * var_std.abs().max(1e-300)).then(|| simple / v_adj.sqrt()),
                    ),
                ]);
            }
            tables.push(table(
                "Generalized Residuals",
                1,
                plain(&[
                    "Coefficients",
                    "Observed",
                    "Expected",
                    "Residual",
                    "Standardized Residual",
                    "Adjusted Residual",
                ]),
                rows,
                None,
            ));
        }
        if !glor.is_empty() {
            let mut rows = Vec::new();
            for (g, name) in glor.iter().enumerate() {
                let e: Vec<f64> = (0..cells)
                    .map(|i| {
                        if in_use[i] {
                            cell_extra(i, nc + gresid.len() + g)
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let mut sums = vec![0.0; settings];
                for i in 0..cells {
                    sums[setting_of(i)] += e[i];
                }
                let scale = e.iter().map(|v| v.abs()).sum::<f64>().max(1.0);
                if sums.iter().any(|s| s.abs() > 1e-8 * scale) {
                    warnings.push(format!(
                        "GENLOG: the GLOR coefficients of {name} do not sum to 0{}, so they are not tested",
                        if logit { " in every BY combination" } else { "" }
                    ));
                    continue;
                }
                let estimate: Option<f64> = (0..cells)
                    .filter(|&i| e[i] != 0.0)
                    .map(|i| (m[i] > 0.0).then(|| e[i] * m[i].ln()))
                    .sum();
                let w: Vec<f64> = (0..kept.len())
                    .map(|k| (0..cells).map(|i| e[i] * x[i][k]).sum())
                    .collect();
                let var = quadratic(&w);
                let se = var.max(0.0).sqrt();
                let wald = estimate.filter(|_| var > 0.0).map(|est| est * est / var);
                rows.push(vec![
                    text(name.clone()),
                    number(estimate),
                    number(wald),
                    number(wald.map(|w| chi_square_p(w, 1.0))),
                    number(estimate.map(|est| est - zq * se)),
                    number(estimate.map(|est| est + zq * se)),
                ]);
            }
            if !rows.is_empty() {
                tables.push(table(
                    "Generalized Log-Odds Ratios",
                    1,
                    vec![
                        (String::new(), "Coefficients".into()),
                        (String::new(), "Estimate".into()),
                        (String::new(), "Wald".into()),
                        (String::new(), "Sig.".into()),
                        (ci.clone(), "Lower Bound".into()),
                        (ci.clone(), "Upper Bound".into()),
                    ],
                    rows,
                    None,
                ));
            }
        }
        if logit {
            let responses = cells / settings;
            let total: f64 = setting_total.iter().sum();
            let plogp = |p: f64| if p > 0.0 { p * p.ln() } else { 0.0 };
            let marginal: Vec<f64> = (0..responses)
                .map(|r| (0..settings).map(|s| m[r * settings + s]).sum::<f64>() / total)
                .collect();
            let entropy_total = -total * marginal.iter().map(|&p| plogp(p)).sum::<f64>();
            let concentration_total = total * (1.0 - marginal.iter().map(|p| p * p).sum::<f64>());
            let (mut entropy_resid, mut concentration_resid) = (0.0, 0.0);
            for (s, &nj) in setting_total.iter().enumerate() {
                if nj <= 0.0 {
                    continue;
                }
                let conditional: Vec<f64> =
                    (0..responses).map(|r| m[r * settings + s] / nj).collect();
                entropy_resid -= nj * conditional.iter().map(|&p| plogp(p)).sum::<f64>();
                concentration_resid += nj * (1.0 - conditional.iter().map(|p| p * p).sum::<f64>());
            }
            let model_df = listed
                .iter()
                .filter(|&&(j, _)| !design_columns[j].dependent_only)
                .count() as f64;
            let total_df = (total - 1.0) * (responses as f64 - 1.0);
            tables.push(table(
                "Analysis of Dispersion",
                1,
                plain(&["Source of Dispersion", "Entropy", "Concentration", "df"]),
                vec![
                    vec![
                        text("Due to model"),
                        Cell::Number(entropy_total - entropy_resid),
                        Cell::Number(concentration_total - concentration_resid),
                        Cell::Number(model_df),
                    ],
                    vec![
                        text("Due to residual"),
                        Cell::Number(entropy_resid),
                        Cell::Number(concentration_resid),
                        Cell::Number(total_df - model_df),
                    ],
                    vec![
                        text("Total"),
                        Cell::Number(entropy_total),
                        Cell::Number(concentration_total),
                        Cell::Number(total_df),
                    ],
                ],
                None,
            ));
            let ratio = |model: f64, all: f64| number((all > 0.0).then(|| model / all));
            tables.push(table(
                "Measures of Association",
                0,
                plain(&["Entropy", "Concentration"]),
                vec![vec![
                    ratio(entropy_total - entropy_resid, entropy_total),
                    ratio(
                        concentration_total - concentration_resid,
                        concentration_total,
                    ),
                ]],
                None,
            ));
        }
        // plots
        let mut charts = Vec::new();
        let kinds =
            |r: Option<statscraft_syntax::GenlogResiduals>| -> Vec<(&str, &Vec<Option<f64>>)> {
                let Some(r) = r else { return Vec::new() };
                let mut out = Vec::new();
                if r.adjusted {
                    out.push(("adjusted residual", &adjresid));
                }
                if r.deviance {
                    out.push(("deviance residual", &dev));
                }
                out
            };
        for (name, values) in kinds(command.plot.resid) {
            let points: Vec<(f64, f64, f64)> = (0..cells)
                .filter_map(|i| values[i].map(|r| (n_fit[i], m[i], r)))
                .collect();
            charts.extend(residual_by_counts("GENLOG", name, &points));
        }
        for (name, values) in kinds(command.plot.normprob) {
            let flat: Vec<f64> = values.iter().flatten().copied().collect();
            charts.extend(normal_plots("GENLOG", name, &flat, true));
        }
        // saved variables: the cell's value for each case of a cell in use
        let save: Vec<Saved> = command
            .save
            .iter()
            .map(|(kind, name)| {
                let (root, label, values) = match kind {
                    GenlogSaveKind::Resid => ("RES", "Residual", &resid),
                    GenlogSaveKind::Zresid => ("ZRE", "Standardized residual", &zresid),
                    GenlogSaveKind::Adjresid => ("ADJ", "Adjusted residual", &adjresid),
                    GenlogSaveKind::Dev => ("DEV", "Deviance residual", &dev),
                    GenlogSaveKind::Pred => ("PRE", "Predicted value", &resid),
                };
                let per_case = case_cell
                    .iter()
                    .map(|cell| {
                        cell.and_then(|i| match kind {
                            GenlogSaveKind::Pred => in_use[i].then_some(m[i]),
                            _ => values[i],
                        })
                    })
                    .collect();
                (root.to_string(), name.clone(), label, per_case)
            })
            .collect();
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        if !save.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (root, name, label, values) in save {
                    let name = match name {
                        Some(name) => name,
                        None => {
                            let mut k = 1;
                            while data.variable_index(&format!("{root}_{k}")).is_some() {
                                k += 1;
                            }
                            format!("{root}_{k}")
                        }
                    };
                    if data.variable_index(&name).is_some() {
                        return Err(Error(format!(
                            "GENLOG /SAVE: variable {name} already exists"
                        )));
                    }
                    data.set_numeric_values(&name, values)?;
                    data.set_format(&name, "F8.2")?;
                    data.set_measure(&name, Measure::Scale)?;
                    data.set_variable_label(&name, Some(&format!("{label} for GENLOG")))?;
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

/// A number such as 95 or 97.5 without trailing zeros.
fn trim_number(v: f64) -> String {
    let s = format!("{v}");
    s.trim_end_matches(".0").to_string()
}

/// Newton-Raphson for log m = log z + x'b over the cells in use.
#[allow(clippy::too_many_arguments)]
fn newton(
    x: &[Vec<f64>],
    z: &[f64],
    in_use: &[bool],
    n: &[f64],
    delta: f64,
    converge: f64,
    iterate: usize,
    scored: &[usize],
) -> Fitted {
    let cells = n.len();
    let p = x.first().map_or(0, Vec::len);
    let mut m: Vec<f64> = (0..cells)
        .map(|i| if in_use[i] { n[i] + delta } else { 0.0 })
        .collect();
    let information = |m: &[f64]| -> Matrix {
        let mut h = vec![vec![0.0; p]; p];
        for i in (0..cells).filter(|&i| in_use[i] && m[i] > 0.0) {
            for a in 0..p {
                if x[i][a] != 0.0 {
                    for b in 0..p {
                        h[a][b] += m[i] * x[i][a] * x[i][b];
                    }
                }
            }
        }
        h
    };
    let invert = |h: &Matrix| inverse(h).map_or_else(|| ginverse(h, 1e-12).0, |(inv, _)| inv);
    let mut beta = vec![0.0; p];
    let mut steps = Vec::new();
    let mut converged = false;
    for _ in 0..iterate {
        let h = information(&m);
        let q: Vec<f64> = (0..p)
            .map(|a| {
                (0..cells)
                    .filter(|&i| in_use[i] && m[i] > 0.0)
                    .map(|i| (m[i] * (m[i] / z[i]).ln() + n[i] - m[i]) * x[i][a])
                    .sum()
            })
            .collect();
        let inv = invert(&h);
        beta = (0..p)
            .map(|a| (0..p).map(|b| inv[a][b] * q[b]).sum())
            .collect();
        let next: Vec<f64> = (0..cells)
            .map(|i| {
                if in_use[i] {
                    z[i] * exp_capped((0..p).map(|a| x[i][a] * beta[a]).sum())
                } else {
                    0.0
                }
            })
            .collect();
        let absolute = (0..cells)
            .map(|i| (next[i] - m[i]).abs())
            .fold(0.0, f64::max);
        let relative = (0..cells)
            .filter(|&i| m[i] > 0.0)
            .map(|i| (next[i] - m[i]).abs() / m[i])
            .fold(0.0, f64::max);
        let score = if scored.is_empty() {
            0.0
        } else {
            let sum: f64 = scored
                .iter()
                .map(|&a| {
                    let g: f64 = (0..cells)
                        .filter(|&i| in_use[i])
                        .map(|i| (n[i] - next[i]) * x[i][a])
                        .sum();
                    g * g
                })
                .sum();
            (sum / scored.len() as f64).sqrt()
        };
        m = next;
        steps.push(Step {
            absolute,
            relative,
            score,
        });
        if (relative < converge || absolute < converge) && score < converge {
            converged = true;
            break;
        }
    }
    let cov = invert(&information(&m));
    Fitted {
        m,
        beta,
        cov,
        steps,
        converged,
    }
}
