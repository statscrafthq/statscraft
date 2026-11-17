//! PLS: partial least squares regression.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (PLS), in our words:
//! - A dependent is scale or categorical (MLEVEL, else its measurement level, else scale for a
//!   number, categorical for a string). A categorical one becomes a 0/1 column for every
//!   category but the reference (the last by default).
//! - The predictor columns are the design of a GLM model without an intercept: a 0/1 column
//!   for every level of a factor, one for each level combination that occurs in an
//!   interaction, times the covariates of the term.
//! - Cases with any missing value (user-missing too) are left out; the weight is a frequency
//!   rounded to a whole number, and a case below 1 after rounding is left out.
//! - Every column is centered and divided by its standard deviation. Each latent factor comes
//!   from one NIPALS pass: w = X'u normalized (u = y for one dependent, else Y times the
//!   dominant eigenvector of Y'XX'Y), t = Xw, c = Y't/t't, u = Yc/|c|, p = X't/t't,
//!   q = Y'u/u'u; then X loses tp' and Y loses tc'.
//! - B = W(P'W)^-1 C', taken back to the original units with a constant.
//! - The share of variance of factor k is t't c'c / trace(Y'Y) for Y and t't p'p / trace(X'X)
//!   for X. VIP of column j up to factor k is sqrt(n sum w_jl^2 SS_l / sum SS_l), SS_l the Y
//!   sum of squares of factor l.
//! - The distance to the X model of a case is sqrt(N / (N - d - 1) e'e), e its X residual.

use crate::crosstabs::{Key, categories};
use crate::descriptives::case_weights;
use crate::glm::{Data, Effect, design, effects_of, key_value_text, parameter_name};
use crate::matrix::{Matrix, inverse, symmetric_eigen};
use crate::regression::table;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{Pls, PlsLevel, PlsReference, Term, Value};

/// The fitted model in standardized units.
struct Fit {
    /// Columns of the latent factors: w, p (length n), c, c/|c|, q (length m).
    w: Vec<Vec<f64>>,
    p: Vec<Vec<f64>>,
    c: Vec<Vec<f64>>,
    cn: Vec<Vec<f64>>,
    q: Vec<Vec<f64>>,
    /// t't of each factor.
    tt: Vec<f64>,
    /// W(P'W)^-1, n rows of d.
    w_star: Matrix,
    /// n rows of m.
    b: Matrix,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A'v for the rows of `a`.
fn cross(a: &Matrix, v: &[f64], width: usize) -> Vec<f64> {
    let mut out = vec![0.0; width];
    for (row, &x) in a.iter().zip(v) {
        for (o, r) in out.iter_mut().zip(row) {
            *o += r * x;
        }
    }
    out
}

fn times(a: &Matrix, v: &[f64]) -> Vec<f64> {
    a.iter().map(|row| dot(row, v)).collect()
}

fn nipals(mut x: Matrix, mut y: Matrix, n: usize, m: usize, most: usize) -> Fit {
    let trace_x: f64 = x.iter().flatten().map(|v| v * v).sum();
    let mut fit = Fit {
        w: Vec::new(),
        p: Vec::new(),
        c: Vec::new(),
        cn: Vec::new(),
        q: Vec::new(),
        tt: Vec::new(),
        w_star: Vec::new(),
        b: Vec::new(),
    };
    let mut first = None;
    for _ in 0..most {
        // S = X'Y, n by m
        let s: Matrix = (0..m)
            .map(|r| cross(&x, &y.iter().map(|row| row[r]).collect::<Vec<_>>(), n))
            .collect();
        let mut w = if m == 1 {
            s[0].clone()
        } else {
            let ss: Matrix = (0..m)
                .map(|a| (0..m).map(|b| dot(&s[a], &s[b])).collect())
                .collect();
            let c0 = &symmetric_eigen(&ss)[0].1;
            (0..n)
                .map(|j| (0..m).map(|r| s[r][j] * c0[r]).sum())
                .collect()
        };
        let norm = dot(&w, &w).sqrt();
        let start = *first.get_or_insert(norm);
        if norm == 0.0 || norm <= 1e-10 * start {
            break;
        }
        w.iter_mut().for_each(|v| *v /= norm);
        let mut t = times(&x, &w);
        let tt = dot(&t, &t);
        if tt <= 1e-12 * trace_x {
            break;
        }
        let mut c: Vec<f64> = cross(&y, &t, m).iter().map(|v| v / tt).collect();
        // the sign of a factor is free: its Y weights add up to a positive number
        if c.iter().sum::<f64>() < 0.0 {
            for v in w.iter_mut().chain(t.iter_mut()).chain(c.iter_mut()) {
                *v = -*v;
            }
        }
        let c_norm = dot(&c, &c).sqrt();
        if c_norm == 0.0 {
            break;
        }
        let cn: Vec<f64> = c.iter().map(|v| v / c_norm).collect();
        let u = times(&y, &cn);
        let uu = dot(&u, &u);
        let p: Vec<f64> = cross(&x, &t, n).iter().map(|v| v / tt).collect();
        let q: Vec<f64> = cross(&y, &u, m)
            .iter()
            .map(|v| if uu > 0.0 { v / uu } else { 0.0 })
            .collect();
        for (i, &ti) in t.iter().enumerate() {
            x[i].iter_mut().zip(&p).for_each(|(v, pj)| *v -= ti * pj);
            y[i].iter_mut().zip(&c).for_each(|(v, cr)| *v -= ti * cr);
        }
        fit.w.push(w);
        fit.p.push(p);
        fit.c.push(c);
        fit.cn.push(cn);
        fit.q.push(q);
        fit.tt.push(tt);
    }
    let d = fit.w.len();
    if d == 0 {
        return fit;
    }
    let pw: Matrix = (0..d)
        .map(|a| (0..d).map(|b| dot(&fit.p[a], &fit.w[b])).collect())
        .collect();
    // P'W is upper triangular with a unit diagonal, never singular
    let (pw_inv, _) = inverse(&pw).expect("P'W has a unit diagonal");
    fit.w_star = (0..n)
        .map(|j| {
            (0..d)
                .map(|k| (0..d).map(|l| fit.w[l][j] * pw_inv[l][k]).sum())
                .collect()
        })
        .collect();
    fit.b = (0..n)
        .map(|j| {
            (0..m)
                .map(|r| (0..d).map(|k| fit.w_star[j][k] * fit.c[k][r]).sum())
                .collect()
        })
        .collect();
    fit
}

/// The weighted mean and standard deviation (N - 1) of each column; 1 for a constant one.
fn moments(rows: &Matrix, f: &[f64], width: usize) -> (Vec<f64>, Vec<f64>) {
    let n: f64 = f.iter().sum();
    let mean: Vec<f64> = (0..width)
        .map(|j| rows.iter().zip(f).map(|(r, w)| w * r[j]).sum::<f64>() / n)
        .collect();
    let sd = (0..width)
        .map(|j| {
            let ss: f64 = rows
                .iter()
                .zip(f)
                .map(|(r, w)| w * (r[j] - mean[j]).powi(2))
                .sum();
            let sd = (ss / (n - 1.0)).sqrt();
            if sd > 1e-12 * mean[j].abs().max(1.0) {
                sd
            } else {
                1.0
            }
        })
        .collect();
    (mean, sd)
}

/// The design row of a case from its factor levels and covariate values; None when a level
/// combination of the case did not occur among the cases of the fit.
fn design_row(effects: &[Effect], level: &[usize], cov: &[f64], width: usize) -> Option<Vec<f64>> {
    let mut row = vec![0.0; width];
    for e in effects {
        let mine: Vec<usize> = e.factors.iter().map(|&f| level[f]).collect();
        let at = e.cells.iter().position(|c| *c == mine)?;
        row[e.start + at] = e.covariates.iter().map(|&c| cov[c]).product();
    }
    Some(row)
}

struct Dependent<'a> {
    variable: Variable<'a>,
    keys: Vec<Option<Key>>,
    /// For a categorical one: its categories and the one left out.
    levels: Option<(Vec<Key>, usize)>,
}

impl Dependent<'_> {
    /// The model columns of case k; None when its value is missing or a category the fit did
    /// not see.
    fn columns(&self, k: usize) -> Option<Vec<f64>> {
        let key = self.keys[k].as_ref()?;
        match &self.levels {
            None => match key {
                Key::Number(v) => Some(vec![*v]),
                Key::Text(_) => None,
            },
            Some((levels, reference)) => {
                let at = levels.iter().position(|l| l.compare(key).is_eq())?;
                Some(
                    (0..levels.len())
                        .filter(|l| l != reference)
                        .map(|l| f64::from(u8::from(l == at)))
                        .collect(),
                )
            }
        }
    }
}

fn numbers(values: Vec<f64>) -> Vec<Option<f64>> {
    values
        .into_iter()
        .map(|v| v.is_finite().then_some(v))
        .collect()
}

fn named_column(
    out: &mut Dataset,
    name: &str,
    label: &str,
    values: Vec<Option<f64>>,
) -> Result<(), Error> {
    out.set_numeric_values(name, values)?;
    out.set_variable_label(name, Some(label))?;
    Ok(())
}

fn text_column(out: &mut Dataset, name: &str, values: Vec<String>) -> Result<(), Error> {
    let width = values
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(1)
        .clamp(8, 32767);
    out.set_string_values(name, width, values)?;
    Ok(())
}

impl Engine {
    pub(crate) fn pls(&mut self, command: &Pls) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        // the dependents, the last setting of a variable named twice counting
        let mut deps: Vec<(String, Option<PlsLevel>, Option<PlsReference>)> = Vec::new();
        for dep in &command.dependents {
            let names = self.expand(std::slice::from_ref(&dep.item))?;
            let last = names.len().saturating_sub(1);
            for (i, name) in names.into_iter().enumerate() {
                let mine = i == last;
                deps.retain(|(n, _, _)| !n.eq_ignore_ascii_case(&name));
                deps.push((
                    name,
                    dep.level.filter(|_| mine),
                    dep.reference.clone().filter(|_| mine),
                ));
            }
        }
        let is_dep = |n: &str| deps.iter().any(|(d, _, _)| d.eq_ignore_ascii_case(n));
        let list = |items| -> Result<Vec<String>, Error> {
            let mut out: Vec<String> = Vec::new();
            for n in self.expand(items)? {
                if !is_dep(&n) && !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
                    out.push(n);
                }
            }
            Ok(out)
        };
        let factor_names = list(&command.factors)?;
        let covariate_names = list(&command.covariates)?;
        if let Some(n) = factor_names
            .iter()
            .find(|f| covariate_names.iter().any(|c| c.eq_ignore_ascii_case(f)))
        {
            return Err(Error(format!(
                "PLS: {n} is both a factor (BY) and a covariate (WITH)"
            )));
        }
        if factor_names.is_empty() && covariate_names.is_empty() {
            return Err(Error(
                "PLS needs at least one predictor: factors after BY or covariates after WITH"
                    .into(),
            ));
        }
        if let Some(id) = &command.id
            && (is_dep(id)
                || factor_names
                    .iter()
                    .chain(&covariate_names)
                    .any(|n| n.eq_ignore_ascii_case(id)))
        {
            return Err(Error(format!(
                "PLS: the ID variable {id} is also a dependent variable or a predictor"
            )));
        }
        let id_var = command.id.as_ref().map(|n| data.variable(n)).transpose()?;
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = cov_vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "PLS needs numeric covariates, but {} is a string variable",
                v.name()
            )));
        }
        let mut dependents: Vec<Dependent> = Vec::new();
        let mut references: Vec<PlsReference> = Vec::new();
        for (name, level, reference) in &deps {
            let variable = data.variable(name)?;
            let categorical = match level.or(command.all_level) {
                Some(PlsLevel::Scale) if variable.is_string() => {
                    return Err(Error(format!(
                        "PLS: the string dependent {name} cannot be scale (MLEVEL=S)"
                    )));
                }
                Some(l) => l != PlsLevel::Scale,
                None => match variable.measure() {
                    Measure::Nominal | Measure::Ordinal => true,
                    Measure::Scale => variable.is_string(),
                    Measure::Unknown => variable.is_string(),
                },
            };
            references.push(
                reference
                    .clone()
                    .or_else(|| command.all_reference.clone())
                    .unwrap_or(PlsReference::Last),
            );
            dependents.push(Dependent {
                keys: categories(&variable, false),
                levels: categorical.then(|| (Vec::new(), 0)),
                variable,
            });
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let freq = |k: usize| weights.as_ref().map_or(1.0, |w| w[k].round());
        let factor_keys: Vec<Vec<Option<Key>>> =
            factor_vars.iter().map(|v| categories(v, false)).collect();
        let covariate = |v: &Variable, k: usize| -> Option<f64> {
            match (crate::means::is_valid(v, k, false), v.values()) {
                (true, Values::Numeric(values)) => values.get(k),
                _ => None,
            }
        };
        let x_valid: Vec<bool> = (0..cases)
            .map(|k| {
                factor_keys.iter().all(|col| col[k].is_some())
                    && cov_vars.iter().all(|v| covariate(v, k).is_some())
            })
            .collect();
        let rows: Vec<usize> = (0..cases)
            .filter(|&k| {
                x_valid[k] && freq(k) >= 1.0 && dependents.iter().all(|d| d.keys[k].is_some())
            })
            .collect();
        let sorted = |col: &[Option<Key>]| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(key.clone());
                }
            }
            found.sort_by(|a, b| a.compare(b));
            found
        };
        let n_total: f64 = rows.iter().map(|&k| freq(k)).sum();
        if n_total < 2.0 {
            return Err(Error(
                "PLS needs at least two cases with valid values on every variable and a weight of 1 or more"
                    .into(),
            ));
        }
        for (dep, reference) in dependents.iter_mut().zip(&references) {
            let Some(slot) = dep.levels.as_mut() else {
                continue;
            };
            let levels = sorted(&dep.keys);
            if levels.len() < 2 {
                return Err(Error(format!(
                    "PLS: the categorical dependent {} has {} category among the cases used; it needs two or more",
                    dep.variable.name(),
                    levels.len()
                )));
            }
            let at = match reference {
                PlsReference::First => Some(0),
                PlsReference::Last => levels.len().checked_sub(1),
                PlsReference::Value(value) => levels.iter().position(|l| match (l, value) {
                    (Key::Number(a), Value::Number(b)) => a == b,
                    (Key::Text(a), Value::String(b)) => a.trim_end() == b.trim_end(),
                    _ => false,
                }),
            };
            let Some(at) = at else {
                return Err(Error(format!(
                    "PLS: the REFERENCE value of {} does not occur among the cases used",
                    dep.variable.name()
                )));
            };
            *slot = (levels, at);
        }
        let levels: Vec<Vec<Key>> = factor_keys.iter().map(|c| sorted(c)).collect();
        let level_of = |f: usize, k: usize| {
            let key = factor_keys[f][k].as_ref()?;
            levels[f].iter().position(|l| l.compare(key).is_eq())
        };
        let d = Data {
            factors: factor_names.clone(),
            factor_texts: factor_vars.iter().map(|v| shown.variable(v)).collect(),
            values: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| key_value_text(v, k)).collect())
                .collect(),
            labels: Vec::new(),
            labelled: false,
            covariates: covariate_names.clone(),
            level: rows
                .iter()
                .map(|&k| {
                    (0..factor_vars.len())
                        .map(|f| level_of(f, k).expect("a level"))
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&k| {
                    cov_vars
                        .iter()
                        .map(|v| covariate(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            y: vec![Vec::new(); rows.len()],
            f: rows.iter().map(|&k| freq(k)).collect(),
            g: vec![1.0; rows.len()],
            case: rows.clone(),
        };
        let terms: Vec<Term> = match &command.model {
            Some(terms) => terms.clone(),
            None => factor_names
                .iter()
                .chain(&covariate_names)
                .map(|n| Term {
                    variables: vec![n.clone()],
                    label: n.clone(),
                })
                .collect(),
        };
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
        let x_raw = design(&d, &mut effects);
        let n = x_raw.first().map_or(0, Vec::len);
        let x_names: Vec<String> = effects
            .iter()
            .flat_map(|e| e.cells.iter().map(|cell| parameter_name(&d, e, cell)))
            .collect();
        let y_raw: Matrix = rows
            .iter()
            .map(|&k| {
                dependents
                    .iter()
                    .flat_map(|dep| dep.columns(k).expect("valid"))
                    .collect()
            })
            .collect();
        let m = y_raw.first().map_or(0, Vec::len);
        let y_names: Vec<String> = dependents
            .iter()
            .flat_map(|dep| match &dep.levels {
                None => vec![shown.variable(&dep.variable)],
                Some((levels, reference)) => (0..levels.len())
                    .filter(|l| l != reference)
                    .map(|l| {
                        format!(
                            "[{}={}]",
                            dep.variable.name(),
                            key_value_text(&dep.variable, &levels[l])
                        )
                    })
                    .collect(),
            })
            .collect();
        let (mx, sx) = moments(&x_raw, &d.f, n);
        let (my, sy) = moments(&y_raw, &d.f, m);
        // a categorical dependent takes one column per category but its reference
        let mut r = 0;
        for dep in &dependents {
            let width = dep
                .levels
                .as_ref()
                .map_or(1, |(levels, _)| levels.len() - 1);
            let at = r;
            r += width;
            let column = y_raw.iter().map(|row| row[at]);
            if dep.levels.is_none() && column.clone().all(|v| v == y_raw[0][at]) {
                return Err(Error(format!(
                    "PLS: the dependent {} has the same value in every case used",
                    dep.variable.name()
                )));
            }
        }
        let standard = |raw: &[f64], mean: &[f64], sd: &[f64]| -> Vec<f64> {
            raw.iter()
                .zip(mean.iter().zip(sd))
                .map(|(v, (mu, s))| (v - mu) / s)
                .collect()
        };
        let scaled = |raw: &Matrix, mean: &[f64], sd: &[f64]| -> Matrix {
            raw.iter()
                .zip(&d.f)
                .map(|(row, f)| {
                    let root = f.sqrt();
                    standard(row, mean, sd).iter().map(|v| v * root).collect()
                })
                .collect()
        };
        let x0 = scaled(&x_raw, &mx, &sx);
        let y0 = scaled(&y_raw, &my, &sy);
        let trace_x: f64 = x0.iter().flatten().map(|v| v * v).sum();
        let trace_y: f64 = y0.iter().flatten().map(|v| v * v).sum();
        // the factors are bounded by the distinct cases, whatever their weights
        if rows.len() < 2 {
            return Err(Error(
                "PLS needs at least two different cases with valid values".to_string(),
            ));
        }
        let most = command.latent_factors.min(n).min(rows.len() - 1);
        let fit = nipals(x0, y0, n, m, most);
        let factors = fit.w.len();
        if factors == 0 {
            return Err(Error(
                "PLS: no latent factor could be extracted; the predictors do not vary or do not relate to the dependents"
                    .into(),
            ));
        }
        let mut warnings = Vec::new();
        if factors < command.latent_factors {
            warnings.push(format!(
                "PLS extracted {factors} latent factors, fewer than the {} asked for: no more are possible with these predictors and cases",
                command.latent_factors
            ));
        }
        if command.empty_by {
            warnings.push("PLS: BY is followed by no factors and is ignored".to_string());
        }
        if command.empty_with {
            warnings.push("PLS: WITH is followed by no covariates and is ignored".to_string());
        }
        // coefficients in the original units
        let b: Matrix = (0..n)
            .map(|j| (0..m).map(|r| fit.b[j][r] * sy[r] / sx[j]).collect())
            .collect();
        let constant: Vec<f64> = (0..m)
            .map(|r| my[r] - (0..n).map(|j| mx[j] * b[j][r]).sum::<f64>())
            .collect();
        let ss_y: Vec<f64> = (0..factors)
            .map(|k| fit.tt[k] * dot(&fit.c[k], &fit.c[k]))
            .collect();
        let ss_x: Vec<f64> = (0..factors)
            .map(|k| fit.tt[k] * dot(&fit.p[k], &fit.p[k]))
            .collect();
        let vip: Matrix = (0..n)
            .map(|j| {
                (0..factors)
                    .map(|k| {
                        let top: f64 = (0..=k).map(|l| fit.w[l][j].powi(2) * ss_y[l]).sum();
                        let bottom: f64 = ss_y[..=k].iter().sum();
                        (n as f64 * top / bottom).sqrt()
                    })
                    .collect()
            })
            .collect();
        let factor_columns = |title: &str| -> Vec<(String, String)> {
            (1..=factors)
                .map(|k| (title.to_string(), k.to_string()))
                .collect()
        };
        let mut tables = Vec::new();
        let (mut cum_x, mut cum_y) = (0.0, 0.0);
        let variance_rows = (0..factors)
            .map(|k| {
                let (vx, vy) = (ss_x[k] / trace_x, ss_y[k] / trace_y);
                cum_x += vx;
                cum_y += vy;
                let df = n_total - k as f64 - 2.0;
                let adjusted = 1.0 - (1.0 - cum_y) * (n_total - 1.0) / df;
                vec![
                    Cell::Text((k + 1).to_string()),
                    Cell::Number(vx),
                    Cell::Number(cum_x),
                    Cell::Number(vy),
                    Cell::Number(cum_y),
                    if df > 0.0 {
                        Cell::Number(adjusted)
                    } else {
                        Cell::Empty
                    },
                ]
            })
            .collect();
        tables.push(table(
            "Proportion of Variance Explained",
            1,
            [
                "Latent Factors",
                "X Variance",
                "Cumulative X Variance",
                "Y Variance",
                "Cumulative Y Variance (R-square)",
                "Adjusted R-square",
            ]
            .iter()
            .map(|c| (String::new(), c.to_string()))
            .collect(),
            variance_rows,
            None,
        ));
        let mut columns = vec![(String::new(), "Independent Variables".to_string())];
        columns.extend(
            y_names
                .iter()
                .map(|y| ("Dependent Variables".into(), y.clone())),
        );
        let mut parameter_rows = vec![
            std::iter::once(Cell::Text("(Constant)".into()))
                .chain(constant.iter().map(|&v| Cell::Number(v)))
                .collect::<Vec<_>>(),
        ];
        parameter_rows.extend(x_names.iter().zip(&b).map(|(name, row)| {
            std::iter::once(Cell::Text(name.clone()))
                .chain(row.iter().map(|&v| Cell::Number(v)))
                .collect()
        }));
        tables.push(table("Parameters", 1, columns, parameter_rows, None));
        let by_factor = |title: &str, label: &str, rows: Vec<(String, Vec<f64>)>| {
            let mut columns = vec![(String::new(), label.to_string())];
            columns.extend(factor_columns("Latent Factors"));
            table(
                title,
                1,
                columns,
                rows.into_iter()
                    .map(|(name, values)| {
                        std::iter::once(Cell::Text(name))
                            .chain(values.into_iter().map(Cell::Number))
                            .collect()
                    })
                    .collect(),
                None,
            )
        };
        let column =
            |vectors: &[Vec<f64>], j: usize| -> Vec<f64> { vectors.iter().map(|v| v[j]).collect() };
        tables.push(by_factor(
            "Variable Importance in the Projection",
            "Variables",
            x_names.iter().cloned().zip(vip.iter().cloned()).collect(),
        ));
        let both = |xs: &[Vec<f64>], ys: &[Vec<f64>]| -> Vec<(String, Vec<f64>)> {
            (0..n)
                .map(|j| (x_names[j].clone(), column(xs, j)))
                .chain((0..m).map(|r| (y_names[r].clone(), column(ys, r))))
                .collect()
        };
        tables.push(by_factor("Weights", "Variables", both(&fit.w, &fit.c)));
        tables.push(by_factor("Loadings", "Variables", both(&fit.p, &fit.q)));
        if let Some(t) = tables.first_mut() {
            t.caption = Some(format!(
                "Latent factors extracted: {factors}. Cases used: {}.",
                n_total
            ));
        }

        // the output datasets
        let wanted = [
            &command.out_cases,
            &command.out_factors,
            &command.out_predictors,
        ];
        if wanted.iter().all(|w| w.is_none()) {
            warnings.into_iter().for_each(|w| self.warn(w));
            return Ok(tables);
        }
        if self.split_group.is_some() {
            return Err(Error(
                "PLS: OUTDATASET with SPLIT FILE is not supported yet".into(),
            ));
        }
        if let Some(name) = wanted
            .iter()
            .copied()
            .flatten()
            .find(|n| self.is_active_name(n))
        {
            return Err(Error(format!(
                "PLS: OUTDATASET cannot replace the active dataset {name}"
            )));
        }
        let mut made: Vec<(String, Dataset)> = Vec::new();
        if let Some(name) = &command.out_cases {
            let mut picked: Vec<(usize, Vec<f64>)> = Vec::new();
            for k in (0..cases).filter(|&k| x_valid[k]) {
                let level: Option<Vec<usize>> =
                    (0..factor_vars.len()).map(|f| level_of(f, k)).collect();
                let cov: Vec<f64> = cov_vars
                    .iter()
                    .map(|v| covariate(v, k).expect("valid"))
                    .collect();
                if let Some(row) = level.and_then(|l| design_row(&effects, &l, &cov, n)) {
                    picked.push((k, row));
                }
            }
            let mut out = Dataset::new(picked.len());
            match &id_var {
                Some(v) => {
                    let which: Vec<Option<usize>> = picked.iter().map(|(k, _)| Some(*k)).collect();
                    out.copy_variable(data, v.name(), &which)?;
                }
                None => {
                    let numbers = picked
                        .iter()
                        .map(|(k, _)| Some(self.case_number(*k) as f64))
                        .collect();
                    named_column(&mut out, "CaseNumber", "Case number", numbers)?;
                    out.set_format("CaseNumber", "F8.0")?;
                }
            }
            let mut predicted = vec![Vec::new(); m];
            let mut residual = vec![Vec::new(); m];
            let mut dmodx = Vec::new();
            let mut t_scores = vec![Vec::new(); factors];
            let mut u_scores = vec![Vec::new(); factors];
            let df = n_total - factors as f64 - 1.0;
            for (k, x) in &picked {
                let xs = standard(x, &mx, &sx);
                let t: Vec<f64> = (0..factors)
                    .map(|l| (0..n).map(|j| xs[j] * fit.w_star[j][l]).sum())
                    .collect();
                let e: f64 = (0..n)
                    .map(|j| xs[j] - (0..factors).map(|l| t[l] * fit.p[l][j]).sum::<f64>())
                    .map(|v| v * v)
                    .sum();
                dmodx.push((df > 0.0).then(|| (n_total / df * e).sqrt()));
                let y: Option<Vec<f64>> = dependents
                    .iter()
                    .map(|dep| dep.columns(*k))
                    .collect::<Option<Vec<Vec<f64>>>>()
                    .map(|v| v.concat());
                let mut ys = y.as_ref().map(|y| standard(y, &my, &sy));
                for l in 0..factors {
                    t_scores[l].push(Some(t[l]));
                    u_scores[l].push(ys.as_ref().map(|ys| dot(ys, &fit.cn[l])));
                    if let Some(ys) = ys.as_mut() {
                        ys.iter_mut()
                            .zip(&fit.c[l])
                            .for_each(|(v, c)| *v -= t[l] * c);
                    }
                }
                for r in 0..m {
                    let p = constant[r] + (0..n).map(|j| x[j] * b[j][r]).sum::<f64>();
                    predicted[r].push(Some(p));
                    residual[r].push(y.as_ref().map(|y| y[r] - p));
                }
            }
            for r in 0..m {
                let what = &y_names[r];
                named_column(
                    &mut out,
                    &format!("Predicted_{}", r + 1),
                    &format!("Predicted value of {what}"),
                    std::mem::take(&mut predicted[r]),
                )?;
                named_column(
                    &mut out,
                    &format!("Residual_{}", r + 1),
                    &format!("Residual of {what}"),
                    std::mem::take(&mut residual[r]),
                )?;
            }
            named_column(&mut out, "DModX", "Distance to the X model", dmodx)?;
            for (l, values) in t_scores.into_iter().enumerate() {
                named_column(
                    &mut out,
                    &format!("XScore_{}", l + 1),
                    &format!("X score of latent factor {}", l + 1),
                    values,
                )?;
            }
            for (l, values) in u_scores.into_iter().enumerate() {
                named_column(
                    &mut out,
                    &format!("YScore_{}", l + 1),
                    &format!("Y score of latent factor {}", l + 1),
                    values,
                )?;
            }
            made.push((name.clone(), out));
        }
        if let Some(name) = &command.out_factors {
            let rows = both(&fit.p, &fit.q);
            let weights = both(&fit.w, &fit.c);
            let mut out = Dataset::new(rows.len());
            text_column(
                &mut out,
                "Variable",
                rows.iter().map(|r| r.0.clone()).collect(),
            )?;
            for l in 0..factors {
                named_column(
                    &mut out,
                    &format!("Loading_{}", l + 1),
                    &format!("Loading on latent factor {}", l + 1),
                    numbers(rows.iter().map(|r| r.1[l]).collect()),
                )?;
            }
            for l in 0..factors {
                named_column(
                    &mut out,
                    &format!("Weight_{}", l + 1),
                    &format!("Weight on latent factor {}", l + 1),
                    numbers(weights.iter().map(|r| r.1[l]).collect()),
                )?;
            }
            made.push((name.clone(), out));
        }
        if let Some(name) = &command.out_predictors {
            let mut out = Dataset::new(n + 1);
            let labels = std::iter::once("(Constant)".to_string()).chain(x_names.iter().cloned());
            text_column(&mut out, "Variable", labels.collect())?;
            for r in 0..m {
                let values = std::iter::once(constant[r]).chain(b.iter().map(|row| row[r]));
                named_column(
                    &mut out,
                    &format!("Coefficient_{}", r + 1),
                    &format!("Coefficient for {}", y_names[r]),
                    numbers(values.collect()),
                )?;
            }
            for l in 0..factors {
                let values = std::iter::once(None).chain(vip.iter().map(|row| Some(row[l])));
                named_column(
                    &mut out,
                    &format!("VIP_{}", l + 1),
                    &format!("Variable importance up to latent factor {}", l + 1),
                    values.collect(),
                )?;
            }
            made.push((name.clone(), out));
        }
        for (name, out) in made {
            self.store_dataset(name, Some(out));
        }
        warnings.into_iter().for_each(|w| self.warn(w));
        Ok(tables)
    }
}
