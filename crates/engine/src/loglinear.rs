//! LOGLINEAR: general loglinear and logit models of a table of counts with contrasts, fitted by
//! Newton-Raphson (multinomial response).
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Every integer of a factor's range is a category (the table has every combination, the last
//!   factor changing fastest); a case with a value outside a range is left out, other values are
//!   cut to integers. A case missing on a factor or covariate is left out (INCLUDE keeps
//!   user-missing values, which must then be inside the range). A cell's count sums the case
//!   weights; a covariate takes the weighted mean of its cases in the cell (0 in an empty cell).
//! - The table is I x J: the factors after BY (or one row without BY) are the rows, the others
//!   the columns. The model is log(m / w) = lambda_i + sum b_k x_k with cell weights w (1 by
//!   default, 0 a structural zero left out of the fit), and the lambda_i fix the row totals.
//! - A factor of K categories contributes the K - 1 non-constant columns of the basis matrix of
//!   its contrast, the inverse of the matrix whose first row is the mean and whose other rows
//!   are the contrast, so that each estimate is its contrast of the log expected counts. The
//!   default is DEVIATION (the last category is the reference). DIFFERENCE, HELMERT, SIMPLE,
//!   REPEATED and POLYNOMIAL are the contrasts of the other procedures. SPECIAL(matrix) takes
//!   the transpose of the K x K matrix as the basis (so its first row is the constant and its
//!   other rows are the columns), BASIS SPECIAL takes the inverse (the matrix holds the
//!   contrasts). `factor(n)` is the n-th non-constant column alone. Effects are the products of
//!   the columns of their parts; a covariate is its cell value.
//! - Without DESIGN, or with an empty one, the model is the saturated one: every effect of the
//!   factors. A column that repeats earlier ones (the sweep of the cross products finds them,
//!   the row indicators first) is redundant and left out; the others are the parameters.
//! - The fit starts at m = n + DELTA; a saturated model (as many parameters as cells in use) is
//!   fitted to the counts plus DELTA, which stay in the cells, so its residuals are 0; other
//!   models drop DELTA after the first step. The row totals of the fit are those of the data.
//!   The loop of the GENLOG procedure does the Newton-Raphson steps; it stops when no expected
//!   count moves by CONVERGE (absolutely or relatively) and the score is below CONVERGE, or
//!   after ITERATE steps (the manual states a test on the change of the linear predictor; the
//!   fit then differs from the manual's stop only in the last digits).
//! - Parameter estimates have standard errors from the inverse information, z values and 95%
//!   intervals; COR is their correlation matrix. Neither is shown when an expected count is 0.
//! - Pearson and likelihood-ratio chi-square, df = I (J - 1) - p - E with p the parameters
//!   and E the cells whose count times weight is 0 or less (as the manual states, so a sampling
//!   zero counts in E when the model is not saturated). Residuals: raw, standardized
//!   (n - m) / sqrt(m), adjusted (n - m) / sqrt(m (1 - h)) with h the leverage of the cell.
//! - GRESID: for coefficients d: sum d n, sum d m, their difference, and over the root of
//!   sum d^2 m - (the part the fit explains) the adjusted value.
//! - A logit model (BY) adds the analysis of dispersion and the measures of association
//!   (entropy and concentration).
//! - PLOT draws the adjusted residuals against the observed and expected counts (RESID), and a
//!   normal and a detrended normal plot of them (NORMPROB).

use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_quantile};
use crate::genlog::newton;
use crate::glm::sweep_in_order;
use crate::hiloglinear::{MAX_CELLS, cell_levels, hat_values, normal_plots, residual_by_counts};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_syntax::{
    CellWeights, Contrast, GenlogPart, Loglinear, LoglinearContrastKind, LoglinearModel,
};

/// The values a variable takes in the cells: the weighted mean of its cases there.
struct CellMeans {
    sum: Vec<f64>,
    weight: Vec<f64>,
}

impl CellMeans {
    fn new(cells: usize) -> Self {
        Self {
            sum: vec![0.0; cells],
            weight: vec![0.0; cells],
        }
    }

    /// The mean in `cell`, or `empty` where no case contributed.
    fn at(&self, cell: usize, empty: f64) -> f64 {
        if self.weight[cell] > 0.0 {
            self.sum[cell] / self.weight[cell]
        } else {
            empty
        }
    }
}

struct Setup {
    /// Factor names, dependents first.
    names: Vec<String>,
    headers: Vec<String>,
    levels: Vec<usize>,
    value_text: Vec<Vec<String>>,
    n_dep: usize,
    covariates: Vec<String>,
    cells: usize,
    n: Vec<f64>,
    covariate_means: Vec<CellMeans>,
    /// The CWEIGHT variable and its cell means.
    cweight_variable: Option<CellMeans>,
    /// GRESID variables by name.
    gresid_variables: Vec<(String, CellMeans)>,
}

/// A column of the design: the effect it belongs to, and its values in every cell.
struct Column {
    effect: String,
    values: Vec<f64>,
    /// The effect names factors of the columns of the table only (no factor after BY).
    dependent_only: bool,
}

enum Part {
    Factor(usize, Option<usize>),
    Covariate(usize),
}

fn name_matches(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The k x (k - 1) basis columns of a factor's contrast.
fn basis(kind: &LoglinearContrastKind, k: usize, factor: &str) -> Result<Matrix, Error> {
    basis_raw(kind, k, factor).map(snapped)
}

fn basis_raw(kind: &LoglinearContrastKind, k: usize, factor: &str) -> Result<Matrix, Error> {
    let own = |e: String| Error(format!("LOGLINEAR CONTRAST({factor}): {e}"));
    match kind {
        LoglinearContrastKind::Named(c) => crate::contrast::coding(c, k).map_err(own),
        LoglinearContrastKind::Special { values, basis } => {
            if values.len() != k * k {
                return Err(own(format!(
                    "SPECIAL has {} values, but {k} categories need {} (the matrix is {k} by {k})",
                    values.len(),
                    k * k
                )));
            }
            let m: Matrix = values.chunks(k).map(<[f64]>::to_vec).collect();
            let b: Matrix = if *basis {
                inverse(&m)
                    .map(|(inv, _)| inv)
                    .ok_or_else(|| own("the BASIS SPECIAL matrix cannot be inverted".into()))?
            } else {
                (0..k).map(|i| (0..k).map(|j| m[j][i]).collect()).collect()
            };
            Ok(b.iter().map(|row| row[1..].to_vec()).collect())
        }
    }
}

/// Values of a basis rounded to 12 decimals, so that 1 - 1/3 - 2/3 shows as 0.
fn snapped(b: Matrix) -> Matrix {
    b.into_iter()
        .map(|row| row.into_iter().map(|v| (v * 1e12).round() / 1e12).collect())
        .collect()
}

impl Engine {
    pub(crate) fn loglinear(&mut self, command: &Loglinear) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let mut names: Vec<String> = Vec::new();
        let mut bounds: Vec<(f64, f64)> = Vec::new();
        for range in command.dependents.iter().chain(&command.independents) {
            for name in self.expand(&range.variables)? {
                if names.iter().any(|n| name_matches(n, &name)) {
                    return Err(Error(format!("LOGLINEAR names {name} more than once")));
                }
                names.push(name);
                bounds.push((range.min, range.max));
            }
        }
        let n_dep: usize = {
            let mut k = 0;
            for range in &command.dependents {
                k += self.expand(&range.variables)?.len();
            }
            k
        };
        let covariates = if command.covariates.is_empty() {
            Vec::new()
        } else {
            self.expand(&command.covariates)?
        };
        for c in &covariates {
            if names.iter().any(|n| name_matches(n, c)) {
                return Err(Error(format!(
                    "LOGLINEAR: {c} is a factor and cannot also be a cell covariate"
                )));
            }
        }
        if names.len() > 10 {
            return Err(Error("LOGLINEAR takes at most 10 factors".into()));
        }
        if covariates.len() > 200 {
            return Err(Error("LOGLINEAR takes at most 200 covariates".into()));
        }
        let cweight_name = command.models.iter().find_map(|m| match &m.cweight {
            Some(CellWeights::Variable(v)) => Some(v.clone()),
            _ => None,
        });
        let mut gresid_names: Vec<String> = Vec::new();
        for model in &command.models {
            for g in &model.gresid {
                if let CellWeights::Variable(v) = g
                    && !gresid_names.iter().any(|n| name_matches(n, v))
                {
                    gresid_names.push(v.clone());
                }
            }
        }
        let factor_vars: Vec<_> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let covariate_vars: Vec<_> = covariates
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let extra_names: Vec<&String> = cweight_name.iter().chain(&gresid_names).collect();
        let extra_vars: Vec<_> = extra_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        for v in factor_vars.iter().chain(&covariate_vars).chain(&extra_vars) {
            if v.is_string() {
                return Err(Error(format!(
                    "LOGLINEAR variables are numeric, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let levels: Vec<usize> = bounds
            .iter()
            // the cast saturates for a huge range, so the + 1 must not overflow either
            .map(|(lo, hi)| ((hi - lo) as usize).saturating_add(1))
            .collect();
        let cells = levels
            .iter()
            .try_fold(1usize, |a, &l| a.checked_mul(l))
            .filter(|&c| c <= MAX_CELLS)
            .ok_or_else(|| {
                Error(format!(
                    "the LOGLINEAR table would have more than {MAX_CELLS} cells; narrow the ranges"
                ))
            })?;
        let factor_columns: Vec<_> = factor_vars
            .iter()
            .map(|v| v.numeric())
            .collect::<Result<_, _>>()?;
        let covariate_columns: Vec<_> = covariate_vars
            .iter()
            .map(|v| v.numeric())
            .collect::<Result<_, _>>()?;
        let extra_columns: Vec<_> = extra_vars
            .iter()
            .map(|v| v.numeric())
            .collect::<Result<_, _>>()?;
        let weights = case_weights(data)?;
        let mut n = vec![0.0; cells];
        let mut covariate_means: Vec<CellMeans> =
            covariates.iter().map(|_| CellMeans::new(cells)).collect();
        let mut extra_means: Vec<CellMeans> =
            extra_names.iter().map(|_| CellMeans::new(cells)).collect();
        let (mut valid, mut out_of_range, mut missing, mut weighted) = (0.0, 0.0, 0.0, 0.0);
        'cases: for k in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[k]);
            if w <= 0.0 {
                continue;
            }
            let mut index = 0;
            let mut outside = false;
            for (f, column) in factor_columns.iter().enumerate() {
                let Some(v) = column
                    .get(k)
                    .filter(|v| command.include || !factor_vars[f].is_user_missing(*v))
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
            let mut covariate_values = Vec::with_capacity(covariates.len());
            for (c, column) in covariate_columns.iter().enumerate() {
                match column
                    .get(k)
                    .filter(|v| command.include || !covariate_vars[c].is_user_missing(*v))
                {
                    Some(v) => covariate_values.push(v),
                    None => {
                        missing += 1.0;
                        continue 'cases;
                    }
                }
            }
            if outside {
                out_of_range += 1.0;
                continue;
            }
            valid += 1.0;
            weighted += w;
            n[index] += w;
            for (means, v) in covariate_means.iter_mut().zip(&covariate_values) {
                means.sum[index] += w * v;
                means.weight[index] += w;
            }
            for (e, column) in extra_columns.iter().enumerate() {
                if let Some(x) = column.get(k).filter(|x| !extra_vars[e].is_user_missing(*x)) {
                    extra_means[e].sum[index] += w * x;
                    extra_means[e].weight[index] += w;
                }
            }
        }
        if weighted <= 0.0 {
            return Err(Error(
                "LOGLINEAR: no case has valid values within the ranges".into(),
            ));
        }
        let mut extras = extra_means.into_iter();
        let cweight_variable = cweight_name.as_ref().and_then(|_| extras.next());
        let gresid_variables: Vec<(String, CellMeans)> =
            gresid_names.iter().cloned().zip(extras).collect();
        let value_text: Vec<Vec<String>> = factor_vars
            .iter()
            .enumerate()
            .map(|(f, v)| {
                (0..levels[f])
                    .map(|l| shown.number(v, bounds[f].0 + l as f64))
                    .collect()
            })
            .collect();
        let headers: Vec<String> = factor_vars.iter().map(|v| shown.variable(v)).collect();
        let mut tables = Vec::new();
        let mut rows = vec![
            vec![text("Cases"), text("Valid"), Cell::Number(valid)],
            vec![text(""), text("Out of range"), Cell::Number(out_of_range)],
            vec![text(""), text("Missing"), Cell::Number(missing)],
            vec![text(""), text("Weighted valid"), Cell::Number(weighted)],
        ];
        for (f, header) in headers.iter().enumerate() {
            rows.push(vec![
                text(if f == 0 { "Categories" } else { "" }),
                text(header),
                Cell::Number(levels[f] as f64),
            ]);
        }
        for (c, v) in covariate_vars.iter().enumerate() {
            rows.push(vec![
                text(if c == 0 { "Cell covariates" } else { "" }),
                text(shown.variable(v)),
                Cell::Empty,
            ]);
        }
        tables.push(table(
            "Data Information",
            2,
            plain(&["", "", "N"]),
            rows,
            None,
        ));
        let setup = Setup {
            names,
            headers,
            levels,
            value_text,
            n_dep,
            covariates,
            cells,
            n,
            covariate_means,
            cweight_variable,
            gresid_variables,
        };
        let mut warnings = Vec::new();
        let mut charts = Vec::new();
        if command.implicit_design {
            warnings.push(
                "LOGLINEAR: the command does not end with DESIGN, so the saturated model is fitted as well"
                    .to_string(),
            );
        }
        let many = command.models.len() > 1;
        for (d, model) in command.models.iter().enumerate() {
            let label = many.then(|| format!("Design {}", d + 1));
            setup.fit(model, label, &mut tables, &mut charts, &mut warnings)?;
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

impl Setup {
    fn cell_text(&self, cell: usize) -> Vec<String> {
        cell_levels(cell, &self.levels)
            .iter()
            .enumerate()
            .map(|(f, &l)| self.value_text[f][l].clone())
            .collect()
    }

    /// The per-cell values of CWEIGHT, GRESID entries and the like, checked for the cell count.
    fn cell_values(&self, spec: &CellWeights, what: &str, empty: f64) -> Result<Vec<f64>, Error> {
        match spec {
            CellWeights::Values(v) => {
                if v.len() == self.cells {
                    Ok(v.clone())
                } else {
                    Err(Error(format!(
                        "LOGLINEAR {what} has {} values, but the table has {} cells",
                        v.len(),
                        self.cells
                    )))
                }
            }
            CellWeights::Variable(name) => {
                let means = if what == "CWEIGHT" {
                    self.cweight_variable.as_ref()
                } else {
                    self.gresid_variables
                        .iter()
                        .find(|(n, _)| name_matches(n, name))
                        .map(|(_, m)| m)
                };
                let means =
                    means.ok_or_else(|| Error(format!("LOGLINEAR: no values for {name}")))?;
                Ok((0..self.cells).map(|i| means.at(i, empty)).collect())
            }
        }
    }

    /// The effects of the model as lists of parts.
    fn effects(&self, model: &LoglinearModel) -> Result<Vec<(String, Vec<Part>)>, Error> {
        let nf = self.names.len();
        let Some(terms) = &model.terms else {
            let mut masks: Vec<u32> = (1..(1u32 << nf)).collect();
            masks.sort_by_key(|&t| (t.count_ones(), std::cmp::Reverse(t.reverse_bits())));
            return Ok(masks
                .into_iter()
                .map(|t| {
                    let parts: Vec<usize> = (0..nf).filter(|f| t & (1 << f) != 0).collect();
                    (
                        parts
                            .iter()
                            .map(|&f| self.names[f].as_str())
                            .collect::<Vec<_>>()
                            .join(" BY "),
                        parts.into_iter().map(|f| Part::Factor(f, None)).collect(),
                    )
                })
                .collect());
        };
        let mut out = Vec::new();
        for term in terms {
            let mut parts: Vec<Part> = Vec::new();
            let mut label: Vec<String> = Vec::new();
            for GenlogPart { name, category } in term {
                if let Some(f) = self.names.iter().position(|n| name_matches(n, name)) {
                    if parts
                        .iter()
                        .any(|p| matches!(p, Part::Factor(g, _) if *g == f))
                    {
                        return Err(Error(format!(
                            "LOGLINEAR DESIGN names {name} twice in one effect"
                        )));
                    }
                    let partition = match category {
                        Some(c) => {
                            let k = self.levels[f];
                            if (*c as usize) > k.saturating_sub(1) {
                                return Err(Error(format!(
                                    "LOGLINEAR DESIGN: {name}({c}) is beyond the {} contrast columns of {name}",
                                    k.saturating_sub(1)
                                )));
                            }
                            Some(*c as usize - 1)
                        }
                        None => None,
                    };
                    label.push(match category {
                        Some(c) => format!("{}({c})", self.names[f]),
                        None => self.names[f].clone(),
                    });
                    parts.push(Part::Factor(f, partition));
                } else if let Some(c) = self.covariates.iter().position(|n| name_matches(n, name)) {
                    if category.is_some() {
                        return Err(Error(format!(
                            "LOGLINEAR DESIGN: the covariate {name} has no partitions"
                        )));
                    }
                    label.push(self.covariates[c].clone());
                    parts.push(Part::Covariate(c));
                } else {
                    return Err(Error(format!(
                        "LOGLINEAR DESIGN names {name}, which is neither a factor nor a cell covariate of the variable list"
                    )));
                }
            }
            out.push((label.join(" BY "), parts));
        }
        Ok(out)
    }

    fn fit(
        &self,
        model: &LoglinearModel,
        label: Option<String>,
        tables: &mut Vec<Table>,
        charts: &mut Vec<Chart>,
        warnings: &mut Vec<String>,
    ) -> Result<(), Error> {
        let cells = self.cells;
        let c = model.criteria;
        let print = {
            let mut p = model.print;
            // a saturated model that DESIGN did not name shows its estimates
            if model.terms.is_none() && !p.none {
                p.estim = true;
            }
            p
        };
        let caption = |extra: Option<&str>| match (&label, extra) {
            (Some(l), Some(e)) => Some(format!("{l}. {e}")),
            (Some(l), None) => Some(l.clone()),
            (None, Some(e)) => Some(e.to_string()),
            (None, None) => None,
        };
        // cell weights
        let z: Vec<f64> = match &model.cweight {
            Some(spec) => self.cell_values(spec, "CWEIGHT", 1.0)?,
            None => vec![1.0; cells],
        };
        let in_use: Vec<bool> = z.iter().map(|&w| w > 0.0).collect();
        let used_cells = in_use.iter().filter(|&&u| u).count();
        if used_cells == 0 {
            return Err(Error("LOGLINEAR: every cell is a structural zero".into()));
        }
        // the design columns
        let nf = self.names.len();
        let cell_lv: Vec<Vec<usize>> = (0..cells).map(|i| cell_levels(i, &self.levels)).collect();
        let mut bases: Vec<Matrix> = Vec::with_capacity(nf);
        for f in 0..nf {
            let kind = model
                .contrasts
                .iter()
                .rev()
                .find(|c| name_matches(&c.variable, &self.names[f]))
                .map_or(
                    LoglinearContrastKind::Named(Contrast::Deviation(None)),
                    |c| c.kind.clone(),
                );
            bases.push(if self.levels[f] < 2 {
                vec![Vec::new(); self.levels[f]]
            } else {
                basis(&kind, self.levels[f], &self.names[f])?
            });
        }
        for c in &model.contrasts {
            if !self.names.iter().any(|n| name_matches(n, &c.variable)) {
                return Err(Error(format!(
                    "LOGLINEAR CONTRAST names {}, which is not a factor",
                    c.variable
                )));
            }
        }
        let mut columns: Vec<Column> = Vec::new();
        for (effect, parts) in self.effects(model)? {
            let dependent_only = parts
                .iter()
                .all(|p| matches!(p, Part::Factor(f, _) if *f < self.n_dep));
            let mut product: Vec<Vec<f64>> = vec![vec![1.0; cells]];
            for part in &parts {
                let options: Vec<Vec<f64>> = match part {
                    Part::Factor(f, partition) => {
                        let k = self.levels[*f];
                        let chosen: Vec<usize> = match partition {
                            Some(p) => vec![*p],
                            None => (0..k.saturating_sub(1)).collect(),
                        };
                        chosen
                            .into_iter()
                            .map(|j| (0..cells).map(|i| bases[*f][cell_lv[i][*f]][j]).collect())
                            .collect()
                    }
                    Part::Covariate(v) => vec![
                        (0..cells)
                            .map(|i| self.covariate_means[*v].at(i, 0.0))
                            .collect(),
                    ],
                };
                product = product
                    .iter()
                    .flat_map(|base| {
                        options
                            .iter()
                            .map(move |o| base.iter().zip(o).map(|(a, b)| a * b).collect())
                    })
                    .collect();
            }
            for values in product {
                columns.push(Column {
                    effect: effect.clone(),
                    values,
                    dependent_only,
                });
            }
        }
        // the rows of the table: one indicator per combination of the factors after BY
        let ind_levels: Vec<usize> = self.levels[self.n_dep..].to_vec();
        let rows_count: usize = ind_levels.iter().product::<usize>().max(1);
        let row_of = |i: usize| -> usize {
            let lv = &cell_lv[i][self.n_dep..];
            lv.iter().zip(&ind_levels).fold(0, |a, (&l, &k)| a * k + l)
        };
        let ncol = columns.len();
        let total_cols = rows_count + ncol;
        // shortcut: dense linear algebra; a larger design would take minutes and gigabytes
        if total_cols > 3000 {
            return Err(Error(format!(
                "LOGLINEAR: the design has {total_cols} columns, more than the 3000 this procedure handles"
            )));
        }
        let value_of = |j: usize, i: usize| -> f64 {
            if j < rows_count {
                f64::from(u8::from(row_of(i) == j))
            } else {
                columns[j - rows_count].values[i]
            }
        };
        let mut gram = vec![vec![0.0; total_cols]; total_cols];
        for a in 0..total_cols {
            for b in a..total_cols {
                let s: f64 = (0..cells)
                    .filter(|&i| in_use[i])
                    .map(|i| value_of(a, i) * value_of(b, i))
                    .sum();
                gram[a][b] = s;
                gram[b][a] = s;
            }
        }
        let order: Vec<usize> = (0..total_cols).collect();
        let (_, swept) = sweep_in_order(&gram, &order, 1e-8);
        let kept: Vec<usize> = (0..total_cols).filter(|&j| swept[j]).collect();
        let kept_rows = kept.iter().filter(|&&j| j < rows_count).count();
        let model_columns: Vec<usize> = kept
            .iter()
            .copied()
            .filter(|&j| j >= rows_count)
            .map(|j| j - rows_count)
            .collect();
        let redundant = ncol - model_columns.len();
        if redundant > 0 {
            warnings.push(format!(
                "LOGLINEAR{}: {redundant} column(s) of the design repeat others and are left out of the model",
                label.as_ref().map_or(String::new(), |l| format!(" ({l})"))
            ));
        }
        let x: Vec<Vec<f64>> = (0..cells)
            .map(|i| kept.iter().map(|&j| value_of(j, i)).collect())
            .collect();
        let saturated = kept.len() >= used_cells;
        let n_fit: Vec<f64> = (0..cells)
            .map(|i| {
                if saturated && in_use[i] {
                    self.n[i] + c.delta
                } else {
                    self.n[i]
                }
            })
            .collect();
        let scored: Vec<usize> = (kept_rows..kept.len()).collect();
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
        if !fitted.converged {
            warnings.push(format!(
                "LOGLINEAR{}: the estimates did not converge in {} iterations",
                label.as_ref().map_or(String::new(), |l| format!(" ({l})")),
                c.iterate
            ));
        }
        // goodness of fit
        let p = model_columns.len();
        let e_cells = (0..cells).filter(|&i| n_fit[i] * z[i] <= 0.0).count();
        let j_count: usize = self.levels[..self.n_dep].iter().product::<usize>().max(1);
        let df = (rows_count * (j_count - 1)).saturating_sub(p + e_cells);
        let mut pearson = 0.0;
        let mut lr = 0.0;
        for i in (0..cells).filter(|&i| in_use[i]) {
            let (o, e) = (n_fit[i], m[i]);
            if e > 0.0 {
                pearson += (o - e) * (o - e) / e;
                if o > 0.0 {
                    lr += 2.0 * o * (o / e).ln();
                }
            }
        }
        // cell statistics
        let hat = hat_values(&x, m, &in_use);
        let total_fit: f64 = (0..cells).filter(|&i| in_use[i]).map(|i| n_fit[i]).sum();
        let total_expected: f64 = (0..cells).filter(|&i| in_use[i]).map(|i| m[i]).sum();
        let residual = |i: usize| n_fit[i] - m[i];
        let standardized = |i: usize| (in_use[i] && m[i] > 0.0).then(|| residual(i) / m[i].sqrt());
        let adjusted = |i: usize| -> Option<f64> {
            if !in_use[i] || m[i] <= 0.0 {
                return None;
            }
            if residual(i).abs() <= 1e-12 * m[i].max(1.0) {
                return Some(0.0);
            }
            let v = m[i] * (1.0 - hat[i]);
            (v > 1e-12 * m[i]).then(|| residual(i) / v.sqrt())
        };
        // the tables
        let mut convergence_rows = vec![
            vec![text("Iterations"), Cell::Number(fitted.steps.len() as f64)],
            vec![
                text("Converged"),
                text(if fitted.converged { "Yes" } else { "No" }),
            ],
        ];
        if let Some(last) = fitted.steps.last() {
            convergence_rows.push(vec![
                text("Largest change in an expected count"),
                Cell::Number(last.absolute),
            ]);
        }
        convergence_rows.push(vec![text("Parameters"), Cell::Number(p as f64)]);
        tables.push(table(
            "Convergence Information",
            1,
            plain(&["", "Value"]),
            convergence_rows,
            caption(None),
        ));
        let sig = |stat: f64| number((df > 0).then(|| chi_square_p(stat, df as f64)));
        let delta_note = saturated.then(|| {
            format!(
                "Saturated model: {} was added to every cell count and stays in it.",
                trim(c.delta)
            )
        });
        if print.freq || print.resid {
            let mut columns = vec![(String::new(), String::new()); 0];
            for h in &self.headers {
                columns.push((String::new(), h.clone()));
            }
            if print.freq {
                for s in [
                    "Observed Count",
                    "Observed %",
                    "Expected Count",
                    "Expected %",
                ] {
                    columns.push((String::new(), s.into()));
                }
            }
            if print.resid {
                for s in ["Residual", "Std. Residual", "Adj. Residual"] {
                    columns.push((String::new(), s.into()));
                }
            }
            let rows = (0..cells)
                .filter(|&i| in_use[i])
                .map(|i| {
                    let mut row: Vec<Cell> = self.cell_text(i).iter().map(|s| text(s)).collect();
                    if print.freq {
                        row.push(Cell::Number(n_fit[i]));
                        row.push(Cell::Number(100.0 * n_fit[i] / total_fit));
                        row.push(Cell::Number(m[i]));
                        row.push(Cell::Number(100.0 * m[i] / total_expected));
                    }
                    if print.resid {
                        row.push(Cell::Number(residual(i)));
                        row.push(number(standardized(i)));
                        row.push(number(adjusted(i)));
                    }
                    row
                })
                .collect();
            tables.push(table(
                "Cell Counts and Residuals",
                nf,
                columns,
                rows,
                caption(delta_note.as_deref()),
            ));
        }
        tables.push(table(
            "Goodness-of-Fit Tests",
            1,
            plain(&["", "Chi-Square", "df", "Sig."]),
            vec![
                vec![
                    text("Likelihood Ratio"),
                    Cell::Number(lr),
                    Cell::Number(df as f64),
                    sig(lr),
                ],
                vec![
                    text("Pearson"),
                    Cell::Number(pearson),
                    Cell::Number(df as f64),
                    sig(pearson),
                ],
            ],
            caption(None),
        ));
        // parameter numbers run over every column the effects generate
        if print.design {
            let mut head: Vec<(String, String)> = self
                .headers
                .iter()
                .map(|h| (String::new(), h.clone()))
                .collect();
            for k in 0..ncol {
                head.push((columns[k].effect.clone(), format!("Parameter {}", k + 1)));
            }
            let rows = (0..cells)
                .filter(|&i| in_use[i])
                .map(|i| {
                    let mut row: Vec<Cell> = self.cell_text(i).iter().map(|s| text(s)).collect();
                    row.extend((0..ncol).map(|k| Cell::Number(columns[k].values[i])));
                    row
                })
                .collect();
            tables.push(table("Design Matrix", nf, head, rows, caption(None)));
        }
        let hidden = (0..cells).any(|i| in_use[i] && m[i] <= 0.0);
        if (print.estim || print.cor) && hidden {
            warnings.push(
                "LOGLINEAR: no parameter estimates or correlations are shown because an expected count is 0"
                    .into(),
            );
        }
        if (print.estim || print.cor) && !hidden && p > 0 {
            let at = |j: usize| rows_count_index(&kept, rows_count + j);
            let se: Vec<f64> = model_columns
                .iter()
                .map(|&j| fitted.cov[at(j)][at(j)].max(0.0).sqrt())
                .collect();
            if print.estim {
                let zq = normal_quantile(0.975);
                let mut last_effect = String::new();
                let rows = model_columns
                    .iter()
                    .enumerate()
                    .map(|(k, &j)| {
                        let b = fitted.beta[at(j)];
                        let effect = if columns[j].effect == last_effect {
                            String::new()
                        } else {
                            last_effect = columns[j].effect.clone();
                            last_effect.clone()
                        };
                        let zvalue = (se[k] > 0.0).then(|| b / se[k]);
                        vec![
                            text(effect),
                            Cell::Number((j + 1) as f64),
                            Cell::Number(b),
                            Cell::Number(se[k]),
                            number(zvalue),
                            Cell::Number(b - zq * se[k]),
                            Cell::Number(b + zq * se[k]),
                        ]
                    })
                    .collect();
                let ci = "95% Confidence Interval".to_string();
                tables.push(table(
                    "Parameter Estimates",
                    2,
                    vec![
                        (String::new(), "Effect".into()),
                        (String::new(), "Parameter".into()),
                        (String::new(), "Coefficient".into()),
                        (String::new(), "Std. Error".into()),
                        (String::new(), "Z".into()),
                        (ci.clone(), "Lower Bound".into()),
                        (ci, "Upper Bound".into()),
                    ],
                    rows,
                    caption(None),
                ));
            }
            if print.cor {
                let mut head = vec![(String::new(), String::new())];
                head.extend(
                    model_columns
                        .iter()
                        .map(|&j| (String::new(), format!("Parameter {}", j + 1))),
                );
                let rows = model_columns
                    .iter()
                    .enumerate()
                    .map(|(a, &ja)| {
                        let mut row = vec![text(format!("Parameter {}", ja + 1))];
                        row.extend(model_columns.iter().enumerate().map(|(b, &jb)| {
                            let d = se[a] * se[b];
                            number((d > 0.0).then(|| fitted.cov[at(ja)][at(jb)] / d))
                        }));
                        row
                    })
                    .collect();
                tables.push(table(
                    "Correlation Matrix of the Parameter Estimates",
                    1,
                    head,
                    rows,
                    caption(None),
                ));
            }
        }
        // generalized residuals
        if !model.gresid.is_empty() {
            let mut rows = Vec::new();
            for (g, spec) in model.gresid.iter().enumerate() {
                let d = self.cell_values(spec, "GRESID", 0.0)?;
                let mut observed = 0.0;
                let mut expected = 0.0;
                let mut dmd = 0.0;
                let mut q = vec![0.0; kept.len()];
                for i in (0..cells).filter(|&i| in_use[i]) {
                    observed += d[i] * n_fit[i];
                    expected += d[i] * m[i];
                    dmd += d[i] * d[i] * m[i];
                    for (a, qa) in q.iter_mut().enumerate() {
                        *qa += x[i][a] * m[i] * d[i];
                    }
                }
                let explained: f64 = (0..kept.len())
                    .map(|a| {
                        q[a] * (0..kept.len())
                            .map(|b| fitted.cov[a][b] * q[b])
                            .sum::<f64>()
                    })
                    .sum();
                let variance = dmd - explained;
                let diff = observed - expected;
                rows.push(vec![
                    text(format!("GRESID {}", g + 1)),
                    Cell::Number(observed),
                    Cell::Number(expected),
                    Cell::Number(diff),
                    number((variance > 1e-12 * dmd.abs().max(1.0)).then(|| diff / variance.sqrt())),
                ]);
            }
            tables.push(table(
                "Generalized Residuals",
                1,
                plain(&["", "Observed", "Expected", "Residual", "Adj. Residual"]),
                rows,
                caption(None),
            ));
        }
        // analysis of dispersion of a logit model
        if self.n_dep < nf {
            let settings = rows_count;
            let responses = cells / settings;
            let mut setting_total = vec![0.0; settings];
            for i in (0..cells).filter(|&i| in_use[i]) {
                setting_total[row_of(i)] += n_fit[i];
            }
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
            let model_df = model_columns
                .iter()
                .filter(|&&j| !columns[j].dependent_only)
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
                caption(None),
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
                caption(None),
            ));
        }
        // plots
        if model.plot_resid || model.plot_normal {
            let points: Vec<(f64, f64, f64)> = (0..cells)
                .filter_map(|i| adjusted(i).map(|a| (n_fit[i], m[i], a)))
                .collect();
            if model.plot_resid {
                charts.extend(residual_by_counts(
                    "LOGLINEAR",
                    "adjusted residual",
                    &points,
                ));
            }
            if model.plot_normal {
                let values: Vec<f64> = points.iter().map(|t| t.2).collect();
                charts.extend(normal_plots(
                    "LOGLINEAR",
                    "adjusted residual",
                    &values,
                    true,
                ));
            }
        }
        Ok(())
    }
}

/// The position of column `j` among the kept columns.
fn rows_count_index(kept: &[usize], j: usize) -> usize {
    kept.iter()
        .position(|&k| k == j)
        .expect("a parameter column is kept")
}

/// A number without a trailing `.0`.
fn trim(v: f64) -> String {
    let s = format!("{v}");
    s.trim_end_matches(".0").to_string()
}
