//! MANOVA: analysis of variance and covariance for one or several dependents, with a full-rank
//! design built from contrasts.
//!
//! Rules from the Command Syntax Reference (MANOVA) and the Algorithms manual (MANOVA):
//! - Factors take the whole numbers of their range (values are cut to whole numbers); a case
//!   outside a range, or missing on any variable of the list, is left out. The cells are the
//!   combinations of all factors, the last factor varying fastest. Case weights are used as
//!   they are; N is their sum.
//! - Each factor gets the k x k matrix whose first row is the mean (1/k each, or the first
//!   row of a square SPECIAL) and whose other rows are the contrasts (DEVIATION by default).
//!   Its inverse is the one-way basis: column 1 the mean column, the others the effect
//!   columns. An effect's design columns are products of the basis columns of its factors
//!   (only the partition named, for `a(n)`), the indicators of the levels of the factors it is
//!   nested in (only level n for `WITHIN b(n)`), the mean column of every other factor, and
//!   the values of continuous variables. The constant is the product of the mean columns.
//! - UNIQUE: an effect's hypothesis SSCP is the residual SSCP without its columns minus the
//!   residual SSCP of the whole design; SEQUENTIAL: the drop as its columns enter after the
//!   terms before it. The constant comes first unless NOCONSTANT.
//! - Error terms: WITHIN, the pooled within-cell SSCP (df N minus the non-empty cells);
//!   RESIDUAL, the cell model the design leaves out; WR, both; or a numbered term. Default:
//!   WITHIN, or RESIDUAL when no cell has two cases, or WR when DESIGN names a continuous
//!   variable.
//! - Covariates are removed from both the error E and E + H by regression; the adjusted H is
//!   the difference and the error loses one df per covariate. The covariates are tested as a
//!   regression against the adjusted error.
//! - Multivariate tests from the eigenvalues of H E^-1: Pillai, Hotelling, Wilks (Rao's F)
//!   and Roy's largest root as lambda / (1 + lambda) without an F.

use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, f_quantile, t_p, t_quantile};
use crate::glm::{
    Data, box_m_stats, multivariate, power_f, power_lambda, power_t, relative_eigenvalues,
    sweep_in_order,
};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{
    Contrast, FactorCriteria, Manova, ManovaAnalysis, ManovaCriterion, ManovaDesign, ManovaDiscrim,
    ManovaEffect, ManovaError, ManovaInterval, ManovaItem, ManovaPcomps, ManovaPower, ManovaPrint,
    ManovaTransform, Rotation, VarItem,
};

const EPS: f64 = 1e-8;
const MAX_CELLS: usize = 100_000;

struct Factor {
    name: String,
    min: f64,
    k: usize,
}

struct Case {
    /// The case number in the data, from 0.
    index: usize,
    levels: Vec<usize>,
    cell: usize,
    /// The continuous variables, dependents then covariates.
    v: Vec<f64>,
    w: f64,
}

/// What a design needs from the procedure.
struct Setup<'a> {
    factors: &'a [Factor],
    cont: &'a [String],
    cases: &'a [Case],
    n: f64,
}

/// A term: its columns in X, its error number and the error it is tested against.
struct Term {
    label: String,
    cols: std::ops::Range<usize>,
    define: Option<u32>,
    against: Option<ManovaError>,
    shown: bool,
    constant: bool,
}

fn err(s: impl Into<String>) -> Error {
    Error(s.into())
}

fn sub(a: &Matrix, b: &Matrix) -> Matrix {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.iter().zip(y).map(|(u, v)| u - v).collect())
        .collect()
}

fn add(a: &Matrix, b: &Matrix) -> Matrix {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.iter().zip(y).map(|(u, v)| u + v).collect())
        .collect()
}

fn block(m: &Matrix, r: std::ops::Range<usize>) -> Matrix {
    m[r.clone()]
        .iter()
        .map(|row| row[r.clone()].to_vec())
        .collect()
}

/// The p x p part of a (p + q) SSCP after regression on the last q variables, with the number
/// of those that are not redundant.
fn adjust(s: &Matrix, p: usize) -> (Matrix, usize) {
    let (m, swept) = sweep_in_order(s, &(p..s.len()).collect::<Vec<_>>(), EPS);
    (block(&m, 0..p), swept.iter().filter(|&&x| x).count())
}

/// The one-way basis of a factor: column 0 the mean column, the others the contrasts.
fn basis(kind: &Contrast, k: usize) -> Result<Matrix, String> {
    if k == 1 {
        return Ok(vec![vec![1.0]]);
    }
    let first = match kind {
        Contrast::Special(v) if v.len() == k * k => v[..k].to_vec(),
        _ => vec![1.0 / k as f64; k],
    };
    let mut c = vec![first];
    c.extend(
        crate::contrast::contrast_rows(kind, k)?
            .into_iter()
            .map(|(_, r)| r),
    );
    inverse(&c)
        .filter(|(_, det)| det.abs() > 1e-12)
        .map(|(inv, _)| inv)
        .ok_or_else(|| "the contrasts are linearly dependent".to_string())
}

/// The value of each factor at `levels`, as text.
fn levels_text(factors: &[Factor], levels: &[usize]) -> Vec<Cell> {
    factors
        .iter()
        .zip(levels)
        .map(|(f, l)| text(format!("{}", f.min + *l as f64)))
        .collect()
}

/// One group of ANALYSIS: the indices of its dependents and of its covariates.
type AnalysisGroup = (Vec<usize>, Vec<usize>);

/// The caption of one analysis among several groups of ANALYSIS: its number and dependents.
fn analysis_caption<'a>(
    groups: usize,
    index: usize,
    deps: impl Iterator<Item = &'a String>,
    caption: &str,
) -> String {
    if groups < 2 {
        return caption.to_string();
    }
    let what = format!(
        "Analysis {}: {}",
        index + 1,
        deps.map(String::as_str).collect::<Vec<_>>().join(" ")
    );
    match caption.is_empty() {
        true => what,
        false => format!("{what}; {caption}"),
    }
}

fn find(names: &[String], name: &str) -> Option<usize> {
    names.iter().position(|n| n.eq_ignore_ascii_case(name))
}

fn matrix_table(title: &str, names: &[String], m: &Matrix, caption: String) -> Table {
    let mut columns = plain(&[""]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    let rows = names
        .iter()
        .zip(m)
        .map(|(n, r)| {
            let mut row = vec![text(n.clone())];
            row.extend(r.iter().map(|v| number(Some(*v))));
            row
        })
        .collect();
    table(title, 1, columns, rows, Some(caption))
}

fn error_name(e: ManovaError) -> String {
    match e {
        ManovaError::Within => "WITHIN CELLS".into(),
        ManovaError::Residual => "RESIDUAL".into(),
        ManovaError::WithinResidual => "WITHIN+RESIDUAL".into(),
        ManovaError::Number(n) => format!("ERROR {n}"),
    }
}

/// The adjusted hypothesis and error of a test, with their df.
struct Test {
    h: Matrix,
    e: Matrix,
    dfh: f64,
    dfe: f64,
}

impl Test {
    fn new(h: &Matrix, dfh: f64, e: &Matrix, dfe: f64, p: usize) -> Test {
        let (ea, q) = adjust(e, p);
        let (ta, _) = adjust(&add(e, h), p);
        Test {
            h: sub(&ta, &ea),
            e: ea,
            dfh,
            dfe: dfe - q as f64,
        }
    }

    fn ok(&self) -> bool {
        self.dfh > 0.0 && self.dfe > 0.0
    }

    fn univariate(&self, j: usize) -> (f64, f64) {
        let f = (self.h[j][j] / self.dfh) / (self.e[j][j] / self.dfe);
        (f, f_p(f, self.dfh, self.dfe))
    }

    /// The eigenvalues of H E^-1, largest first, and the number of variables that count.
    fn roots(&self) -> (Vec<f64>, usize) {
        let reference: Vec<f64> = (0..self.e.len())
            .map(|i| self.e[i][i] + self.h[i][i])
            .collect();
        let (mut l, p) = relative_eigenvalues(&self.h, &self.e, &reference);
        l.truncate(p.min(self.dfh as usize));
        (l, p)
    }
}

/// The continuous variables a design analyzes: each a combination of the original ones.
struct Space {
    names: Vec<String>,
    /// One row per variable: its coefficients on the original variables.
    rows: Matrix,
    ndeps: usize,
}

impl Space {
    fn identity(cont: &[String], ndeps: usize) -> Space {
        let k = cont.len();
        Space {
            names: cont.to_vec(),
            rows: (0..k).map(|i| unit(k, i)).collect(),
            ndeps,
        }
    }

    fn rename(&mut self, d: &ManovaDesign) -> Result<(), Error> {
        let Some(new) = &d.rename else {
            return Ok(());
        };
        if new.len() != self.names.len() {
            return Err(err(format!(
                "RENAME gives {} names for {} continuous variables",
                new.len(),
                self.names.len()
            )));
        }
        for (name, n) in self.names.iter_mut().zip(new) {
            if let Some(n) = n {
                name.clone_from(n);
            }
        }
        for (i, n) in self.names.iter().enumerate() {
            if find(&self.names[..i], n).is_some() {
                return Err(err(format!("RENAME gives the name {n} twice")));
            }
        }
        Ok(())
    }

    fn apply(&self, cases: &[Case]) -> Vec<Case> {
        cases
            .iter()
            .map(|c| Case {
                v: self
                    .rows
                    .iter()
                    .map(|r| r.iter().zip(&c.v).map(|(a, b)| a * b).sum())
                    .collect(),
                levels: c.levels.clone(),
                ..*c
            })
            .collect()
    }

    /// The transposed transformation matrix: original variables down, new ones across.
    fn table(&self, cont: &[String], caption: &str) -> Table {
        let mut columns = plain(&[""]);
        columns.extend(self.names.iter().map(|n| (String::new(), n.clone())));
        let rows = cont
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut row = vec![text(n.clone())];
                row.extend(self.rows.iter().map(|r| number(Some(r[i]))));
                row
            })
            .collect();
        table(
            "Transformation Matrix",
            1,
            columns,
            rows,
            (!caption.is_empty()).then(|| caption.to_string()),
        )
    }
}

fn unit(k: usize, i: usize) -> Vec<f64> {
    (0..k).map(|j| f64::from(u8::from(i == j))).collect()
}

/// Gram-Schmidt on the rows, in order, leaving out a row that the earlier ones span.
fn orthonormal_rows(m: &Matrix) -> Matrix {
    let mut out: Matrix = Vec::new();
    for row in m {
        let norm0 = row.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mut v = row.clone();
        for q in &out {
            let dot: f64 = v.iter().zip(q).map(|(a, b)| a * b).sum();
            v.iter_mut().zip(q).for_each(|(a, b)| *a -= dot * b);
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-10 * norm0.max(f64::MIN_POSITIVE) {
            out.push(v.iter().map(|x| x / norm).collect());
        }
    }
    out
}

/// The rows of a TRANSFORM matrix for k variables: the mean and the contrasts (SPECIAL: the
/// matrix itself), BASIS taking the transposed inverse, ORTHONORM orthonormalizing the rows.
fn transform_matrix(t: &ManovaTransform, k: usize) -> Result<Matrix, String> {
    let c: Matrix = match &t.kind {
        Contrast::Special(v) if v.len() == k * k => v.chunks(k).map(<[f64]>::to_vec).collect(),
        Contrast::Special(v) => {
            return Err(format!(
                "SPECIAL has {} values; {k} variables need a square matrix of {}",
                v.len(),
                k * k
            ));
        }
        kind => {
            let mut c = vec![vec![1.0 / k as f64; k]];
            c.extend(
                crate::contrast::contrast_rows(kind, k)?
                    .into_iter()
                    .map(|(_, r)| r),
            );
            c
        }
    };
    let m = if t.basis {
        let (inv, det) = inverse(&c).ok_or("the matrix is singular, so BASIS has no inverse")?;
        if det.abs() <= 1e-12 {
            return Err("the matrix is singular, so BASIS has no inverse".into());
        }
        crate::matrix::transpose(&inv)
    } else {
        c
    };
    Ok(if t.orthonorm { orthonormal_rows(&m) } else { m })
}

/// Rao's F for Wilks' lambda with p variables and q hypothesis df: (F, df1, df2).
fn rao(lambda: f64, p: f64, q: f64, ne: f64) -> (f64, f64, f64) {
    let t = if p * p + q * q - 5.0 > 0.0 {
        ((p * p * q * q - 4.0) / (p * p + q * q - 5.0)).sqrt()
    } else {
        1.0
    };
    let m = ne - (p + 1.0 - q) / 2.0;
    let df2 = m * t + 1.0 - p * q / 2.0;
    let root = lambda.powf(1.0 / t);
    ((1.0 - root) * df2 / (root * p * q), p * q, df2)
}

mod matrix;

impl Engine {
    pub(crate) fn manova(&mut self, c: &Manova) -> Result<Vec<Table>, Error> {
        let mut part = None;
        let result = self.manova_inner(c, &mut part);
        if let (Ok(_), Some(part)) = (&result, part) {
            self.matrix_parts.1.push(part);
        }
        result
    }

    fn manova_inner(
        &mut self,
        c: &Manova,
        matrix_part: &mut Option<Dataset>,
    ) -> Result<Vec<Table>, Error> {
        if let Some(source) = &c.matrix.input {
            return self.manova_from_matrix(c, source.as_ref(), matrix_part);
        }
        let data = self.active()?;
        let deps = self.expand(&c.dependents)?;
        let mut covs = self.expand(&c.covariates)?;
        let nvary = covs.len();
        if !c.constant_covariates.is_empty() {
            covs.extend(self.expand(&c.constant_covariates)?);
        }
        let mut factors = Vec::new();
        for g in &c.factors {
            for name in self.expand(&g.names)? {
                let k = (g.max - g.min) as usize + 1;
                factors.push(Factor {
                    name,
                    min: g.min,
                    k,
                });
            }
        }
        let cont: Vec<String> = deps.iter().chain(&covs).cloned().collect();
        let mut seen: Vec<&String> = Vec::new();
        for n in cont.iter().chain(factors.iter().map(|f| &f.name)) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(err(format!("MANOVA names {n} more than once")));
            }
            seen.push(n);
        }
        factors
            .iter()
            .try_fold(1usize, |a, f| a.checked_mul(f.k))
            .filter(|&c| c <= MAX_CELLS)
            .ok_or_else(|| {
                err(format!(
                    "MANOVA: more than {MAX_CELLS} cells; narrow the ranges"
                ))
            })?;
        let vars: Vec<_> = cont
            .iter()
            .chain(factors.iter().map(|f| &f.name))
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = vars.iter().find(|v| v.is_string()) {
            return Err(err(format!(
                "MANOVA needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let weights = case_weights(data)?;
        let (mut out_of_range, mut missing) = (0.0, 0.0);
        let mut cases = Vec::new();
        'cases: for k in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[k]);
            if w <= 0.0 {
                continue;
            }
            let mut values = Vec::with_capacity(vars.len());
            for v in &vars {
                let x = match v.values() {
                    Values::Numeric(x) => x.get(k),
                    Values::String(_) => None,
                };
                match x.filter(|x| c.include || !v.is_user_missing(*x)) {
                    Some(x) => values.push(x),
                    None => {
                        missing += w;
                        continue 'cases;
                    }
                }
            }
            let mut levels = Vec::new();
            let mut cell = 0;
            for (f, x) in factors.iter().zip(&values[cont.len()..]) {
                let l = x.trunc() - f.min;
                if l < 0.0 || l >= f.k as f64 {
                    out_of_range += w;
                    continue 'cases;
                }
                levels.push(l as usize);
                cell = cell * f.k + l as usize;
            }
            values.truncate(cont.len());
            cases.push(Case {
                index: k,
                levels,
                cell,
                v: values,
                w,
            });
        }
        let n: f64 = cases.iter().map(|c| c.w).sum();
        if cases.is_empty() {
            return Err(err(
                "MANOVA: no case has valid values within the factor ranges",
            ));
        }
        let mut occupied: Vec<usize> = cases.iter().map(|c| c.cell).collect();
        occupied.sort_unstable();
        occupied.dedup();
        let mut tables = vec![table(
            "Cases",
            1,
            plain(&["", "N"]),
            vec![
                vec![text("Accepted"), number(Some(n))],
                vec![
                    text("Rejected: factor value out of range"),
                    number(Some(out_of_range)),
                ],
                vec![text("Rejected: missing value"), number(Some(missing))],
                vec![text("Non-empty cells"), number(Some(occupied.len() as f64))],
            ],
            None,
        )];
        let setup = Setup {
            factors: &factors,
            cont: &cont,
            cases: &cases,
            n,
        };
        tables.extend(setup.cell_tables(c, deps.len())?);
        if c.matrix.output.is_some() {
            *matrix_part = Some(self.manova_matrix_out(&setup)?);
        }
        self.manova_designs(c, &setup, tables, deps.len(), nvary)
    }

    /// The designs of the command on the cases `setup` holds.
    fn manova_designs(
        &mut self,
        c: &Manova,
        setup: &Setup,
        mut tables: Vec<Table>,
        ndeps: usize,
        nvary: usize,
    ) -> Result<Vec<Table>, Error> {
        if c.implicit_design {
            self.warn(
                "MANOVA: the command does not end with DESIGN, so a full factorial design is added",
            );
        }
        for (i, d) in c.designs.iter().enumerate() {
            let caption = if c.designs.len() > 1 {
                format!("Design {}", i + 1)
            } else {
                String::new()
            };
            if c.wsfactors.is_empty() {
                tables.extend(self.transformed_design(setup, d, ndeps, &caption)?);
            } else {
                tables.extend(self.repeated_design(setup, c, d, ndeps, nvary, &caption)?);
            }
        }
        Ok(tables)
    }

    /// Every factor CONTRAST names must be one of `factors`.
    fn check_contrasts(d: &ManovaDesign, factors: &[&str]) -> Result<(), Error> {
        for (name, _) in &d.contrasts {
            if !factors.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                return Err(err(format!("CONTRAST names {name}, which is not a factor")));
            }
        }
        Ok(())
    }

    /// A design on the continuous variables after TRANSFORM and RENAME.
    fn transformed_design(
        &self,
        s: &Setup,
        d: &ManovaDesign,
        ndeps: usize,
        caption: &str,
    ) -> Result<Vec<Table>, Error> {
        let factors: Vec<&str> = s.factors.iter().map(|f| f.name.as_str()).collect();
        Self::check_contrasts(d, &factors)?;
        let mut space = Space::identity(s.cont, ndeps);
        if let Some(t) = &d.transform {
            space = self.transform_space(s.cont, t, ndeps)?;
        }
        space.rename(d)?;
        let mut out = Vec::new();
        if d.print.transform && d.transform.is_some() {
            out.push(space.table(s.cont, caption));
        }
        let cases = space.apply(s.cases);
        let s2 = Setup {
            cont: &space.names,
            cases: &cases,
            ..*s
        };
        let groups = match &d.analysis {
            None => vec![(
                (0..space.ndeps).collect(),
                (space.ndeps..space.names.len()).collect(),
            )],
            Some(a) => self.analysis_groups(a, &space.names)?,
        };
        for (g, (dv, cv)) in groups.iter().enumerate() {
            let cap = analysis_caption(
                groups.len(),
                g,
                dv.iter().map(|&v| &space.names[v]),
                caption,
            );
            out.extend(self.manova_design(&s2, d, dv, cv, None, &cap)?);
        }
        Ok(out)
    }

    /// The groups of ANALYSIS as indices into `names`: each group's dependents and covariates.
    /// The covariates after the parentheses go to every group, and with CONDITIONAL the
    /// variables of the earlier groups follow as covariates of a later one.
    fn analysis_groups(
        &self,
        a: &ManovaAnalysis,
        names: &[String],
    ) -> Result<Vec<AnalysisGroup>, Error> {
        let pick = |items: &[VarItem]| -> Result<Vec<usize>, Error> {
            self.expand_among(items, names)?
                .iter()
                .map(|n| {
                    find(names, n).ok_or_else(|| {
                        err(format!(
                            "ANALYSIS names {n}, which is not a dependent variable or covariate of MANOVA"
                        ))
                    })
                })
                .collect()
        };
        let globals = pick(&a.covariates)?;
        let mut groups = Vec::new();
        let mut seen: Vec<usize> = Vec::new();
        for (deps, covs) in &a.groups {
            let (dv, cv) = (pick(deps)?, pick(covs)?);
            groups.push((dv, cv));
        }
        // a variable is named once on the whole subcommand: in one group, or among the
        // covariates after the groups
        for &v in groups
            .iter()
            .flat_map(|(d, c)| d.iter().chain(c))
            .chain(&globals)
        {
            if seen.contains(&v) {
                return Err(err(format!(
                    "ANALYSIS names {} more than once; a variable belongs to one group, \
                     or to the covariates after the groups",
                    names[v]
                )));
            }
            seen.push(v);
        }
        let mut out = Vec::new();
        let mut earlier: Vec<usize> = Vec::new();
        for (dv, cv) in groups {
            let mut covs = cv.clone();
            covs.extend(&globals);
            if a.conditional {
                covs.extend(&earlier);
            }
            earlier.extend(dv.iter().chain(&cv));
            out.push((dv, covs));
        }
        Ok(out)
    }

    /// Names of a variable list, with TO ranges taken over `names` when they are not in the
    /// data (transformed or renamed variables).
    fn expand_among(&self, items: &[VarItem], names: &[String]) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        for item in items {
            match item {
                VarItem::Name(n) => out.push(n.clone()),
                VarItem::Range(a, b) => match (find(names, a), find(names, b)) {
                    (Some(i), Some(j)) if i <= j => out.extend(names[i..=j].iter().cloned()),
                    _ => out.extend(self.expand(std::slice::from_ref(item))?),
                },
                VarItem::All => out.extend(names.iter().cloned()),
            }
        }
        Ok(out)
    }

    /// A repeated measures design: the between-subjects analysis of the constant of the
    /// orthonormal within-subjects transformation, then one analysis for each within-subjects
    /// effect of its transformed variables (the constant of that analysis is the effect).
    fn repeated_design(
        &self,
        s: &Setup,
        c: &Manova,
        d: &ManovaDesign,
        ndeps: usize,
        nvary: usize,
        caption: &str,
    ) -> Result<Vec<Table>, Error> {
        if d.transform.is_some() {
            return Err(err(
                "TRANSFORM cannot be used with WSFACTORS, which transforms the dependents itself",
            ));
        }
        let Some(a) = &d.analysis else {
            return self.repeated_on(s, c, d, ndeps, nvary, caption);
        };
        // each group is a repeated measures analysis of its own: its dependents are the cells,
        // its covariates the varying ones (those not in the parentheses of the variable list) in
        // sets of one per cell, and the constant ones
        let groups = self.analysis_groups(a, s.cont)?;
        let mut out = Vec::new();
        for (g, (dv, cv)) in groups.iter().enumerate() {
            let varying: Vec<usize> = cv.iter().copied().filter(|&v| v < ndeps + nvary).collect();
            let constant: Vec<usize> = cv.iter().copied().filter(|&v| v >= ndeps + nvary).collect();
            let order: Vec<usize> = dv
                .iter()
                .chain(&varying)
                .chain(&constant)
                .copied()
                .collect();
            let names: Vec<String> = order.iter().map(|&v| s.cont[v].clone()).collect();
            let cases: Vec<Case> = s
                .cases
                .iter()
                .map(|case| Case {
                    v: order.iter().map(|&v| case.v[v]).collect(),
                    levels: case.levels.clone(),
                    ..*case
                })
                .collect();
            let s2 = Setup {
                cont: &names,
                cases: &cases,
                ..*s
            };
            let cap = analysis_caption(groups.len(), g, dv.iter().map(|&v| &s.cont[v]), caption);
            out.extend(self.repeated_on(&s2, c, d, dv.len(), varying.len(), &cap)?);
        }
        Ok(out)
    }

    /// The repeated measures design of the variables `s` holds: the `ndeps` dependents, the
    /// `nvary` varying covariates, then the constant ones.
    fn repeated_on(
        &self,
        s: &Setup,
        c: &Manova,
        d: &ManovaDesign,
        ndeps: usize,
        nvary: usize,
        caption: &str,
    ) -> Result<Vec<Table>, Error> {
        let wsf: Vec<Factor> = c
            .wsfactors
            .iter()
            .map(|(name, k)| Factor {
                name: name.clone(),
                min: 1.0,
                k: *k,
            })
            .collect();
        for (i, f) in wsf.iter().enumerate() {
            let clash = find(s.cont, &f.name).is_some()
                || s.factors
                    .iter()
                    .any(|g| g.name.eq_ignore_ascii_case(&f.name))
                || wsf[..i]
                    .iter()
                    .any(|g| g.name.eq_ignore_ascii_case(&f.name));
            if clash {
                return Err(err(format!(
                    "the within-subjects factor {} has the name of another factor or variable",
                    f.name
                )));
            }
        }
        let factors: Vec<&str> = s
            .factors
            .iter()
            .chain(&wsf)
            .map(|f| f.name.as_str())
            .collect();
        Self::check_contrasts(d, &factors)?;
        let r: usize = wsf.iter().map(|f| f.k).product();
        if !ndeps.is_multiple_of(r) {
            return Err(err(format!(
                "WSFACTORS gives {r} within-subjects cells, which do not divide the {ndeps} dependent variables"
            )));
        }
        let nmeas = ndeps / r;
        if !c.measures.is_empty() && c.measures.len() != nmeas {
            return Err(err(format!(
                "MEASURE names {} measures, but the dependents make {nmeas}",
                c.measures.len()
            )));
        }
        if !nvary.is_multiple_of(r) {
            return Err(err(format!(
                "the {nvary} covariates outside parentheses are not a whole number of sets of the {r} within-subjects cells"
            )));
        }
        let measures: Vec<String> = if c.measures.is_empty() {
            (1..=nmeas).map(|m| format!("MEAS.{m}")).collect()
        } else {
            c.measures.clone()
        };
        // each factor's orthonormal basis: column 0 the constant, the others its contrasts
        let mut bases = Vec::new();
        for f in &wsf {
            let kind = d
                .contrasts
                .iter()
                .rev()
                .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                .map_or(Contrast::Polynomial(None), |(_, k)| k.clone());
            let mut rows = vec![vec![1.0; f.k]];
            rows.extend(
                crate::contrast::contrast_rows(&kind, f.k)
                    .map_err(|e| err(format!("CONTRAST({}): {e}", f.name)))?
                    .into_iter()
                    .map(|(_, r)| r),
            );
            let q = orthonormal_rows(&rows);
            if q.len() < f.k {
                return Err(err(format!(
                    "CONTRAST({}): the contrasts are linearly dependent",
                    f.name
                )));
            }
            bases.push(crate::matrix::transpose(&q));
        }
        let full = crate::glm::kronecker(&bases);
        // the transformed variables: each measure and each varying covariate set by `full`
        let sets = nmeas + nvary / r;
        let k = s.cont.len();
        let mut space = Space {
            names: Vec::new(),
            rows: Vec::new(),
            ndeps,
        };
        for set in 0..sets {
            for j in 0..r {
                let mut row = vec![0.0; k];
                for (i, f) in full.iter().enumerate() {
                    row[set * r + i] = f[j];
                }
                space.rows.push(row);
                space.names.push(format!("T{}", set * r + j + 1));
            }
        }
        for (v, name) in s.cont.iter().enumerate().take(k).skip(sets * r) {
            space.rows.push(unit(k, v));
            space.names.push(name.clone());
        }
        space.rename(d)?;
        let mut out = Vec::new();
        if d.print.transform {
            out.push(space.table(s.cont, caption));
        }
        let terms = match &d.wsdesign {
            Some(t) => t.clone(),
            None => full_factorial(&wsf),
        };
        let mut analyses: Vec<(Option<String>, Matrix)> =
            vec![(None, vec![full.iter().map(|row| row[0]).collect()])];
        for t in &terms {
            let mut cols = Vec::new();
            for e in &t.effects {
                cols.extend(within_columns(e, &wsf, &bases, d)?);
            }
            analyses.push((Some(t.label.clone()), cols));
        }
        for (label, cols) in analyses {
            // the name of a column: the transformed variable it equals, else the effect's own
            let name = |set: usize, ci: usize, col: &[f64]| -> String {
                let at = (0..r).find(|&j| {
                    (full
                        .iter()
                        .zip(col)
                        .map(|(f, x)| f[j] * x)
                        .sum::<f64>()
                        .abs()
                        - 1.0)
                        .abs()
                        < 1e-9
                });
                if let Some(j) = at {
                    return space.names[set * r + j].clone();
                }
                let own = format!("{} {}", label.as_deref().unwrap_or("T"), ci + 1);
                if set >= nmeas {
                    format!("{} {own}", s.cont[set * r])
                } else if nmeas > 1 {
                    format!("{} {own}", measures[set])
                } else {
                    own
                }
            };
            let mut names = Vec::new();
            for set in 0..sets {
                for (ci, col) in cols.iter().enumerate() {
                    names.push(name(set, ci, col));
                }
            }
            names.extend(space.names[sets * r..].iter().cloned());
            let cases: Vec<Case> = s
                .cases
                .iter()
                .map(|case| {
                    let mut v = Vec::with_capacity(names.len());
                    for set in 0..sets {
                        for col in &cols {
                            v.push(col.iter().zip(&case.v[set * r..]).map(|(a, b)| a * b).sum());
                        }
                    }
                    v.extend_from_slice(&case.v[sets * r..]);
                    Case {
                        v,
                        levels: case.levels.clone(),
                        ..*case
                    }
                })
                .collect();
            let s2 = Setup {
                cont: &names,
                cases: &cases,
                ..*s
            };
            let p = nmeas * cols.len();
            let dv: Vec<usize> = (0..p).collect();
            let cv: Vec<usize> = (p..names.len()).collect();
            let what = match &label {
                None => "Between-subjects effects".to_string(),
                Some(l) => format!("Within-subjects effect {l}"),
            };
            let cap = if caption.is_empty() {
                what
            } else {
                format!("{what}; {caption}")
            };
            let within = label.map(|label| Within {
                label,
                per: cols.len(),
                measures: measures.clone(),
            });
            out.extend(self.manova_design(&s2, d, &dv, &cv, within.as_ref(), &cap)?);
        }
        Ok(out)
    }

    /// The variables TRANSFORM makes: each list transformed by the same matrix, the others kept.
    fn transform_space(
        &self,
        cont: &[String],
        t: &ManovaTransform,
        ndeps: usize,
    ) -> Result<Space, Error> {
        let groups: Vec<Vec<usize>> = if t.lists.is_empty() {
            vec![(0..cont.len()).collect()]
        } else {
            let mut groups = Vec::new();
            for list in &t.lists {
                let g: Vec<usize> = self
                    .expand_among(list, cont)?
                    .iter()
                    .map(|n| {
                        find(cont, n).ok_or_else(|| {
                            err(format!(
                                "TRANSFORM names {n}, which is not a dependent variable or covariate of MANOVA"
                            ))
                        })
                    })
                    .collect::<Result<_, _>>()?;
                groups.push(g);
            }
            groups
        };
        let k = groups[0].len();
        if groups.iter().any(|g| g.len() != k) {
            return Err(err(
                "the lists of TRANSFORM must have the same number of variables",
            ));
        }
        let mut slot: Vec<Option<(usize, usize)>> = vec![None; cont.len()];
        for (g, list) in groups.iter().enumerate() {
            for (j, &v) in list.iter().enumerate() {
                if slot[v].replace((g, j)).is_some() {
                    return Err(err(format!("TRANSFORM names {} twice", cont[v])));
                }
            }
        }
        let m = transform_matrix(t, k).map_err(|e| err(format!("TRANSFORM: {e}")))?;
        let mut space = Space {
            names: Vec::new(),
            rows: Vec::new(),
            ndeps: 0,
        };
        let mut counter = 0;
        for v in 0..cont.len() {
            let mut row = vec![0.0; cont.len()];
            match slot[v] {
                None => {
                    row[v] = 1.0;
                    space.names.push(cont[v].clone());
                }
                Some((g, j)) if j < m.len() => {
                    for (i, &u) in groups[g].iter().enumerate() {
                        row[u] = m[j][i];
                    }
                    counter += 1;
                    space.names.push(format!("T{counter}"));
                }
                Some(_) => continue,
            }
            space.rows.push(row);
            if v < ndeps {
                space.ndeps += 1;
            }
        }
        Ok(space)
    }

    fn manova_design(
        &self,
        s: &Setup,
        d: &ManovaDesign,
        dv: &[usize],
        cv: &[usize],
        within: Option<&Within>,
        caption: &str,
    ) -> Result<Vec<Table>, Error> {
        let used: Vec<usize> = dv.iter().chain(cv).copied().collect();
        let (p, q) = (dv.len(), cv.len());
        let mut x: Vec<Vec<f64>> = Vec::new();
        let mut terms: Vec<Term> = Vec::new();
        let mut continuous = false;
        let push = |x: &mut Vec<Vec<f64>>, terms: &mut Vec<Term>, cols: Vec<Vec<f64>>, t: Term| {
            let start = x.len();
            x.extend(cols);
            terms.push(Term {
                cols: start..x.len(),
                ..t
            });
        };
        let bases: Vec<Matrix> = s
            .factors
            .iter()
            .map(|f| {
                let kind = d
                    .contrasts
                    .iter()
                    .rev()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                    .map_or(Contrast::Deviation(None), |(_, k)| k.clone());
                basis(&kind, f.k).map_err(|e| err(format!("CONTRAST({}): {e}", f.name)))
            })
            .collect::<Result<_, _>>()?;
        let constant = |case: &Case| -> f64 {
            bases
                .iter()
                .zip(&case.levels)
                .map(|(b, &l)| b[l][0])
                .product()
        };
        let written: Vec<statscraft_syntax::ManovaTerm> = match &d.terms {
            Some(t) => t.clone(),
            None => full_factorial(s.factors),
        };
        let explicit_constant = written
            .iter()
            .any(|t| t.muplus || t.effects.iter().any(ManovaEffect::is_constant));
        if d.constant && !explicit_constant {
            let col = s.cases.iter().map(constant).collect();
            let t = Term {
                label: "CONSTANT".into(),
                cols: 0..0,
                define: None,
                against: None,
                shown: s.factors.is_empty() && d.terms.is_none(),
                constant: true,
            };
            push(&mut x, &mut terms, vec![col], t);
        }
        for t in &written {
            let mut cols = Vec::new();
            if t.muplus {
                cols.push(s.cases.iter().map(constant).collect());
            }
            for e in &t.effects {
                let (more, cont) = self.effect_columns(s, d, e, &bases, &used)?;
                continuous |= cont;
                cols.extend(more);
            }
            let label = if t.muplus {
                format!("MUPLUS {}", t.label)
            } else {
                t.label.clone()
            };
            let is_constant = !t.muplus && t.effects.iter().all(ManovaEffect::is_constant);
            push(
                &mut x,
                &mut terms,
                cols,
                Term {
                    label,
                    cols: 0..0,
                    define: t.define,
                    against: t.against,
                    shown: true,
                    constant: is_constant,
                },
            );
        }
        // in a within-subjects analysis the constant is the within-subjects effect
        if let Some(w) = within {
            for t in &mut terms {
                if t.constant {
                    t.label.clone_from(&w.label);
                    t.shown = true;
                } else {
                    t.label = format!("{} BY {}", t.label, w.label);
                }
            }
        }
        // Z'WZ over the design columns, the dependents and the covariates
        let nx = x.len();
        let dim = nx + p + q;
        let mut zwz = vec![vec![0.0; dim]; dim];
        for (i, case) in s.cases.iter().enumerate() {
            let z: Vec<f64> = x
                .iter()
                .map(|col| col[i])
                .chain(used.iter().map(|&j| case.v[j]))
                .collect();
            for a in 0..dim {
                if z[a] != 0.0 {
                    for b in 0..dim {
                        zwz[a][b] += case.w * z[a] * z[b];
                    }
                }
            }
        }
        let after = |order: &[usize]| -> (Matrix, usize) {
            let (m, swept) = sweep_in_order(&zwz, order, EPS);
            (block(&m, nx..dim), swept.iter().filter(|&&s| s).count())
        };
        let all: Vec<usize> = (0..nx).collect();
        let (s_wr, rank) = after(&all);
        let mut hyp: Vec<(Matrix, f64)> = Vec::new();
        let mut before: Vec<usize> = Vec::new();
        let (mut s_before, mut r_before) = after(&[]);
        for t in &terms {
            if d.sequential {
                before.extend(t.cols.clone());
                let (s_after, r_after) = after(&before);
                hyp.push((sub(&s_before, &s_after), (r_after - r_before) as f64));
                (s_before, r_before) = (s_after, r_after);
            } else {
                let others: Vec<usize> = all
                    .iter()
                    .filter(|c| !t.cols.contains(c))
                    .copied()
                    .collect();
                let (s_minus, r_minus) = after(&others);
                hyp.push((sub(&s_minus, &s_wr), (rank - r_minus) as f64));
            }
        }
        // the pooled within-cell SSCP
        let mut sums: std::collections::BTreeMap<usize, (f64, Vec<f64>, Matrix)> =
            Default::default();
        for case in s.cases {
            let v: Vec<f64> = used.iter().map(|&j| case.v[j]).collect();
            let e = sums
                .entry(case.cell)
                .or_insert_with(|| (0.0, vec![0.0; p + q], vec![vec![0.0; p + q]; p + q]));
            e.0 += case.w;
            for a in 0..p + q {
                e.1[a] += case.w * v[a];
                for b in 0..p + q {
                    e.2[a][b] += case.w * v[a] * v[b];
                }
            }
        }
        let mut s_w = vec![vec![0.0; p + q]; p + q];
        for (w, sv, ss) in sums.values() {
            for a in 0..p + q {
                for b in 0..p + q {
                    s_w[a][b] += ss[a][b] - sv[a] * sv[b] / w;
                }
            }
        }
        let df_w = s.n - sums.len() as f64;
        let df_wr = s.n - rank as f64;
        let default = match d.error {
            Some(e) => e,
            None if continuous => ManovaError::WithinResidual,
            None if df_w > 0.0 => ManovaError::Within,
            None => ManovaError::Residual,
        };
        let error_of = |e: ManovaError| -> Result<Option<(Matrix, f64)>, Error> {
            if continuous && matches!(e, ManovaError::Within | ManovaError::Residual) {
                return Err(err(format!(
                    "MANOVA: the {} error term is not available when DESIGN names a continuous variable; use WR",
                    error_name(e)
                )));
            }
            Ok(match e {
                ManovaError::Within => Some((s_w.clone(), df_w)),
                ManovaError::Residual => Some((sub(&s_wr, &s_w), df_wr - df_w)),
                ManovaError::WithinResidual => Some((s_wr.clone(), df_wr)),
                ManovaError::Number(n) => terms
                    .iter()
                    .position(|t| t.define == Some(n))
                    .map(|i| hyp[i].clone()),
            })
        };
        for n in 1..=10 {
            if terms.iter().filter(|t| t.define == Some(n)).count() > 1 {
                return Err(err(format!("DESIGN defines error term {n} more than once")));
            }
        }
        let names: Vec<String> = dv.iter().map(|&j| s.cont[j].clone()).collect();
        let method = if d.sequential { "SEQUENTIAL" } else { "UNIQUE" };
        let cap = |what: &str| -> String {
            let mut parts = vec![what.to_string()];
            if !caption.is_empty() {
                parts.push(caption.to_string());
            }
            parts.join("; ")
        };
        // every shown term with its test (None when its error term does not exist), and with
        // SINGLEDF one test for each of its degrees of freedom, orthogonal within the term
        let mut tests: Vec<Row> = Vec::new();
        for (i, t) in terms.iter().enumerate() {
            if !t.shown {
                continue;
            }
            let e = t.against.unwrap_or(default);
            let error = error_of(e)?
                .filter(|_| t.define.is_none() || e != ManovaError::Number(t.define.unwrap_or(0)));
            tests.push(Row {
                term: i,
                label: t.label.clone(),
                error: e,
                test: error
                    .as_ref()
                    .map(|(se, dfe)| Test::new(&hyp[i].0, hyp[i].1, se, *dfe, p)),
                single: false,
            });
            if let (true, Some((se, dfe)), true) = (d.print.singledf, &error, t.cols.len() > 1) {
                let mut order: Vec<usize> = if d.sequential {
                    terms[..i].iter().flat_map(|t| t.cols.clone()).collect()
                } else {
                    all.iter()
                        .filter(|c| !t.cols.contains(c))
                        .copied()
                        .collect()
                };
                let (mut s0, mut r0) = after(&order);
                let mut j = 0;
                for c in t.cols.clone() {
                    order.push(c);
                    let (s1, r1) = after(&order);
                    if r1 > r0 {
                        j += 1;
                        tests.push(Row {
                            term: i,
                            label: format!("{} ({j})", t.label),
                            error: e,
                            test: Some(Test::new(&sub(&s0, &s1), 1.0, se, *dfe, p)),
                            single: true,
                        });
                    }
                    (s0, r0) = (s1, r1);
                }
            }
        }
        let default_error = error_of(default)?;
        // the default error after the covariates, with its df
        let adjusted = default_error.as_ref().map(|(se, dfe)| {
            let (ea, qq) = adjust(se, p);
            (ea, dfe - qq as f64)
        });
        let regression = default_error.as_ref().filter(|_| q > 0).map(|(se, dfe)| {
            let (ea, qq) = adjust(se, p);
            Test {
                h: sub(&block(se, 0..p), &ea),
                e: ea,
                dfh: qq as f64,
                dfe: dfe - qq as f64,
            }
        });
        let pr = &d.print;
        let iv = &d.cinterval;
        let wide = (pr.parameters && terms.iter().any(|t| t.cols.len() > 1)) || q > 1;
        if iv.multivariate == Some(ManovaCriterion::Roy) && iv.joint && p > 1 && wide {
            return Err(err(
                "CINTERVAL JOINT MULTIVARIATE(ROY) for a term of more than one df needs the distribution of the largest root, which is not supported yet; use INDIVIDUAL or another type",
            ));
        }
        let mut out = Vec::new();
        let averaged = within.filter(|w| w.per > 1);
        if p == 1 {
            out.push(self.univariate_table(
                &terms,
                &tests,
                &regression,
                default,
                &zwz,
                nx,
                &s_wr,
                rank,
                s.n,
                pr.efsize,
                d.power,
                &cap(&format!("{} using {method} sums of squares", names[0])),
            ));
        } else if !(averaged.is_some() && pr.avonly) {
            let mut groups: Vec<(String, &Test)> = Vec::new();
            if let Some(r) = &regression {
                groups.push((format!("{} Regression", error_name(default)), r));
            }
            for row in &tests {
                if let Some(t) = &row.test {
                    let suffix = if row.error == default {
                        String::new()
                    } else {
                        format!(" (vs {})", error_name(row.error))
                    };
                    groups.push((format!("{}{suffix}", row.label), t));
                }
            }
            out.extend(multivariate_tables(
                &groups,
                &names,
                pr,
                d.power,
                &cap(method),
            ));
            if let Some(dc) = &d.discrim {
                // the estimates of each effect's parameters (the covariates' coefficients for
                // the regression), dependents across
                let order: Vec<usize> = (0..nx).chain(nx + p..dim).collect();
                let (full, swept) = sweep_in_order(&zwz, &order, EPS);
                let mut estimates: Vec<Vec<(String, Vec<f64>)>> = Vec::new();
                if let (Some(_), Some((se, _))) = (&regression, &default_error) {
                    let (m, swept) = sweep_in_order(se, &(p..p + q).collect::<Vec<_>>(), EPS);
                    estimates.push(
                        (0..q)
                            .filter(|&k| swept[p + k])
                            .map(|k| (s.cont[cv[k]].clone(), (0..p).map(|j| m[j][p + k]).collect()))
                            .collect(),
                    );
                }
                for row in tests.iter().filter(|r| r.test.is_some()) {
                    estimates.push(
                        terms[row.term]
                            .cols
                            .clone()
                            .filter(|&c| swept[c] && !row.single)
                            .map(|c| {
                                (
                                    format!("{}", c + 1),
                                    (0..p).map(|j| full[nx + j][c]).collect(),
                                )
                            })
                            .collect(),
                    );
                }
                out.extend(discriminant_tables(
                    dc,
                    &groups,
                    &estimates,
                    &names,
                    &cap(method),
                ));
            }
        }
        if let Some(w) = averaged {
            out.extend(averaged_tables(
                w,
                &tests,
                adjusted.as_ref(),
                default,
                s.n,
                pr,
                d.power,
                &cap(method),
            ));
        }
        if let (Some((se, dfe)), true) = (&default_error, q > 0) {
            let covs: Vec<String> = cv.iter().map(|&j| s.cont[j].clone()).collect();
            out.push(coefficients(
                se,
                *dfe,
                &names,
                &covs,
                Est::new(d, p),
                &cap(&error_name(default)),
            ));
        }
        if pr.overall || pr.oneway {
            out.extend(design_tables(s, &x, &bases, pr, continuous, &cap(method))?);
        }
        if let Some((ea, dfe)) = &adjusted {
            let dfe = *dfe;
            let what = cap(&error_name(default));
            if pr.error_sscp {
                out.push(matrix_table("Error SSCP Matrix", &names, ea, what.clone()));
            }
            let cov: Matrix = ea
                .iter()
                .map(|r| r.iter().map(|v| v / dfe).collect())
                .collect();
            if pr.error_cov {
                out.push(matrix_table(
                    "Error Variance-Covariance Matrix",
                    &names,
                    &cov,
                    what.clone(),
                ));
            }
            if pr.error_cor {
                out.push(matrix_table(
                    "Error Correlation Matrix",
                    &names,
                    &correlations(&cov),
                    format!("{what}; standard deviations on the diagonal"),
                ));
            }
            if let (Some(pc), true) = (&d.pcomps, p > 1) {
                out.extend(principal_components(pc, &names, &cov, &what));
            }
            if pr.error_stddev {
                out.push(table(
                    "Error Standard Deviations",
                    1,
                    plain(&["", "Std. Dev."]),
                    names
                        .iter()
                        .enumerate()
                        .map(|(j, nm)| vec![text(nm.clone()), number(Some(cov[j][j].sqrt()))])
                        .collect(),
                    Some(what),
                ));
            }
            if pr.parameters || pr.param_cor || d.pmeans.is_some() || d.residuals {
                let order: Vec<usize> = (0..nx).chain(nx + p..dim).collect();
                let (full, swept) = sweep_in_order(&zwz, &order, EPS);
                if pr.parameters {
                    out.push(parameters(
                        &terms,
                        &tests,
                        (ea, dfe),
                        &full,
                        &swept,
                        nx,
                        &names,
                        d,
                        &cap(method),
                    ));
                }
                if pr.param_cor {
                    out.extend(parameter_correlations(
                        &zwz,
                        &order,
                        &swept,
                        nx,
                        &cap(method),
                    ));
                }
                if let Some(pm) = &d.pmeans {
                    if continuous {
                        return Err(err(
                            "PMEANS is not available when DESIGN names a continuous variable",
                        ));
                    }
                    let keep: Vec<usize> = match &pm.variables {
                        None => (0..p).collect(),
                        Some(items) => self
                            .expand_among(items, &names)?
                            .iter()
                            .map(|n| {
                                find(&names, n).ok_or_else(|| {
                                    err(format!(
                                        "PMEANS VARIABLES names {n}, which is not a dependent variable of the analysis"
                                    ))
                                })
                            })
                            .collect::<Result<_, _>>()?,
                    };
                    let (design_only, design_swept) = sweep_in_order(&zwz, &all, EPS);
                    out.extend(predicted_means(
                        s,
                        &x,
                        (&full, &swept),
                        (&design_only, &design_swept),
                        nx,
                        dv,
                        &names,
                        &keep,
                        &pm.tables,
                        &cov,
                        &cap(&error_name(default)),
                    )?);
                }
                if d.residuals {
                    out.push(casewise(
                        s,
                        &x,
                        (&full, &swept),
                        (nx, p),
                        dv,
                        cv,
                        &names,
                        &cov,
                        &cap(&error_name(default)),
                    ));
                }
            }
        }
        if pr.ortho {
            out.push(orthogonal_estimates(&terms, &zwz, nx, &names, &cap(method)));
        }
        if pr.hypoth && p > 1 {
            for row in tests.iter().filter(|r| !r.single) {
                if let Some(t) = &row.test {
                    out.push(matrix_table(
                        "Hypothesis SSCP Matrix",
                        &names,
                        &t.h,
                        cap(&row.label),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// The design columns of one effect and whether it has a continuous variable.
    fn effect_columns(
        &self,
        s: &Setup,
        d: &ManovaDesign,
        e: &ManovaEffect,
        bases: &[Matrix],
        used: &[usize],
    ) -> Result<(Vec<Vec<f64>>, bool), Error> {
        let factor_names: Vec<String> = s.factors.iter().map(|f| f.name.clone()).collect();
        let mut cols: Vec<Vec<f64>> = vec![vec![1.0; s.cases.len()]];
        let mut involved: Vec<usize> = Vec::new();
        let mut continuous = false;
        let items = e
            .crossed
            .iter()
            .map(|i| (i, false))
            .chain(e.within.iter().map(|i| (i, true)));
        for (item, nested) in items {
            let part: Vec<Vec<f64>> = match self.item_kind(s, item, &factor_names)? {
                Kind::Factor(f) => {
                    if involved.contains(&f) {
                        return Err(err(format!(
                            "the effect {} names {} twice",
                            statscraft_syntax::manova_effect_label(e),
                            s.factors[f].name
                        )));
                    }
                    involved.push(f);
                    let k = s.factors[f].k;
                    let chosen: Vec<usize> = if nested {
                        match item.number {
                            Some(l) if l as usize > k => {
                                return Err(err(format!(
                                    "{} has {k} levels, not {l}",
                                    s.factors[f].name
                                )));
                            }
                            Some(l) => vec![l as usize - 1],
                            None => (0..k).collect(),
                        }
                    } else {
                        partition(d, &s.factors[f], item.number)?
                    };
                    chosen
                        .iter()
                        .map(|&j| {
                            s.cases
                                .iter()
                                .map(|c| {
                                    if nested {
                                        f64::from(u8::from(c.levels[f] == j))
                                    } else {
                                        bases[f][c.levels[f]][j]
                                    }
                                })
                                .collect()
                        })
                        .collect()
                }
                Kind::Continuous(vs) => {
                    if nested || item.number.is_some() {
                        return Err(err(
                            "a continuous variable on DESIGN takes no number and cannot follow WITHIN",
                        ));
                    }
                    if let Some(&j) = vs.iter().find(|j| used.contains(j)) {
                        return Err(err(format!(
                            "{} is a dependent variable or covariate of this analysis; leave it out of ANALYSIS to name it on DESIGN",
                            s.cont[j]
                        )));
                    }
                    if continuous {
                        return Err(err(
                            "an effect cannot join two continuous variables; compute their product first",
                        ));
                    }
                    continuous = true;
                    vs.iter()
                        .map(|&j| s.cases.iter().map(|c| c.v[j]).collect())
                        .collect()
                }
            };
            cols = cols
                .iter()
                .flat_map(|a| {
                    part.iter()
                        .map(move |b| a.iter().zip(b).map(|(u, v)| u * v).collect())
                })
                .collect();
        }
        for (f, b) in bases.iter().enumerate() {
            if !involved.contains(&f) {
                for col in &mut cols {
                    for (v, c) in col.iter_mut().zip(s.cases) {
                        *v *= b[c.levels[f]][0];
                    }
                }
            }
        }
        Ok((cols, continuous))
    }

    fn item_kind(&self, s: &Setup, item: &ManovaItem, factors: &[String]) -> Result<Kind, Error> {
        if !item.pool
            && let [VarItem::Name(n)] = item.names.as_slice()
            && let Some(f) = find(factors, n)
        {
            return Ok(Kind::Factor(f));
        }
        let mut out = Vec::new();
        for v in &item.names {
            let range = match v {
                VarItem::Name(n) => (n, n),
                VarItem::Range(a, b) => (a, b),
                VarItem::All => return Err(err("ALL is not allowed on DESIGN")),
            };
            let at = |n: &String| {
                find(s.cont, n).ok_or_else(|| {
                    err(format!(
                        "{n} on DESIGN is not a factor, dependent variable or covariate of MANOVA"
                    ))
                })
            };
            let (a, b) = (at(range.0)?, at(range.1)?);
            if a > b {
                return Err(err(format!(
                    "{} comes after {} on the MANOVA variable list",
                    range.0, range.1
                )));
            }
            out.extend(a..=b);
        }
        Ok(Kind::Continuous(out))
    }

    #[allow(clippy::too_many_arguments)]
    fn univariate_table(
        &self,
        terms: &[Term],
        tests: &[Row],
        regression: &Option<Test>,
        default: ManovaError,
        zwz: &Matrix,
        nx: usize,
        s_wr: &Matrix,
        rank: usize,
        n: f64,
        efsize: bool,
        power: Option<ManovaPower>,
        caption: &str,
    ) -> Table {
        let mut rows = Vec::new();
        let row = |name: String, ss: f64, df: f64, t: Option<&Test>| -> Vec<Cell> {
            let f = t.filter(|t| t.ok()).map(|t| t.univariate(0));
            let mut r = vec![
                text(name),
                number(Some(ss)),
                number(Some(df)),
                number((df > 0.0).then(|| ss / df)),
                number(f.map(|f| f.0)),
                number(f.map(|f| f.1)),
            ];
            if efsize {
                r.push(number(t.map(|t| t.h[0][0] / (t.h[0][0] + t.e[0][0]))));
            }
            if let Some(w) = power {
                let lp = t
                    .zip(f)
                    .map(|(t, (f, _))| power_f(f, t.dfh, t.dfe, w.f_alpha));
                r.push(number(lp.map(|x| x.0)));
                r.push(number(lp.map(|x| x.1)));
            }
            r
        };
        let mut errors: Vec<ManovaError> = vec![default];
        for t in tests {
            if !errors.contains(&t.error) {
                errors.push(t.error);
            }
        }
        for e in errors {
            let group: Vec<&Row> = tests
                .iter()
                .filter(|t| t.error == e && t.test.is_some())
                .collect();
            let Some(first) = group
                .first()
                .and_then(|g| g.test.as_ref())
                .or(if e == default {
                    regression.as_ref()
                } else {
                    None
                })
            else {
                continue;
            };
            rows.push(row(error_name(e), first.e[0][0], first.dfe, None));
            if e == default
                && let Some(r) = regression
            {
                rows.push(row("REGRESSION".into(), r.h[0][0], r.dfh, Some(r)));
            }
            for r in group {
                let t = r.test.as_ref().expect("a test");
                rows.push(row(r.label.clone(), t.h[0][0], t.dfh, Some(t)));
            }
        }
        for r in tests.iter().filter(|r| r.test.is_none()) {
            rows.push(row(
                format!("{} (no error term)", r.label),
                f64::NAN,
                f64::NAN,
                None,
            ));
        }
        // (Model) and (Total): the design after the constant, both after the covariates
        let base: Vec<usize> = terms
            .iter()
            .filter(|t| t.constant)
            .flat_map(|t| t.cols.clone())
            .collect();
        let (m, swept) = sweep_in_order(zwz, &base, EPS);
        let r_base = swept.iter().filter(|&&s| s).count();
        let dim = zwz.len();
        let (total, q) = adjust(&block(&m, nx..dim), 1);
        let (resid, _) = adjust(s_wr, 1);
        let model = total[0][0] - resid[0][0];
        let df_model = (rank - r_base) as f64;
        let df_total = n - r_base as f64 - q as f64;
        rows.push(row("(Model)".into(), model, df_model, None));
        rows.push(row("(Total)".into(), total[0][0], df_total, None));
        let r2 = model / total[0][0];
        let adj = 1.0 - (1.0 - r2) * df_total / (n - rank as f64 - q as f64);
        let mut columns = plain(&["Source of Variation", "SS", "DF", "MS", "F", "Sig of F"]);
        if efsize {
            columns.extend(plain(&["Partial Eta Sq."]));
        }
        if power.is_some() {
            columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
        }
        let mut t = table(
            "Tests of Significance",
            1,
            columns,
            rows,
            Some(format!(
                "{caption}. R-Squared = {r2:.3}, Adjusted R-Squared = {adj:.3}"
            )),
        );
        t.rows.iter_mut().for_each(|r| {
            for c in r.iter_mut() {
                if matches!(c, Cell::Number(v) if v.is_nan()) {
                    *c = Cell::Empty;
                }
            }
        });
        t
    }
}

enum Kind {
    Factor(usize),
    Continuous(Vec<usize>),
}

/// The contrast columns (1-based in the basis) of partition `n` of a factor, or all of them.
fn partition(d: &ManovaDesign, f: &Factor, n: Option<u32>) -> Result<Vec<usize>, Error> {
    let df = f.k - 1;
    let mut sizes: Vec<usize> = d
        .partitions
        .iter()
        .rev()
        .find(|(name, _)| name.eq_ignore_ascii_case(&f.name))
        .map_or_else(
            || vec![1; df],
            |(_, s)| s.iter().map(|&x| x as usize).collect(),
        );
    let sum: usize = sizes.iter().sum();
    if sum > df {
        return Err(err(format!(
            "PARTITION({}) has more than its {df} degrees of freedom",
            f.name
        )));
    }
    if sum < df {
        sizes.push(df - sum);
    }
    let Some(n) = n else {
        return Ok((1..=df).collect());
    };
    let n = n as usize;
    if n > sizes.len() {
        return Err(err(format!(
            "{}({n}): the factor has {} partitions",
            f.name,
            sizes.len()
        )));
    }
    let start: usize = sizes[..n - 1].iter().sum::<usize>() + 1;
    Ok((start..start + sizes[n - 1]).collect())
}

/// A within-subjects effect being analyzed: its label, its transformed variables per measure
/// and the names of the measures.
struct Within {
    label: String,
    per: usize,
    measures: Vec<String>,
}

/// The columns of a within-subjects effect over the r cells: a factor it crosses gives its
/// contrast columns (partition n for f(n)), a factor it is nested in all its columns (or the
/// unit vector of level n), any other factor its constant column.
fn within_columns(
    e: &ManovaEffect,
    wsf: &[Factor],
    bases: &[Matrix],
    d: &ManovaDesign,
) -> Result<Matrix, Error> {
    let names: Vec<String> = wsf.iter().map(|f| f.name.clone()).collect();
    let mut parts: Vec<Option<Matrix>> = vec![None; wsf.len()];
    let items = e
        .crossed
        .iter()
        .map(|i| (i, false))
        .chain(e.within.iter().map(|i| (i, true)));
    for (item, nested) in items {
        let f = match item.names.as_slice() {
            [VarItem::Name(n)] if !item.pool => find(&names, n),
            _ => None,
        }
        .ok_or_else(|| {
            err(format!(
                "WSDESIGN names {}, which is not a within-subjects factor",
                statscraft_syntax::manova_effect_label(e)
            ))
        })?;
        let k = wsf[f].k;
        let chosen: Matrix = if nested {
            match item.number {
                Some(l) if l as usize > k => {
                    return Err(err(format!("{} has {k} levels, not {l}", wsf[f].name)));
                }
                Some(l) => vec![unit(k, l as usize - 1)],
                None => (0..k)
                    .map(|j| bases[f].iter().map(|r| r[j]).collect())
                    .collect(),
            }
        } else {
            partition(d, &wsf[f], item.number)?
                .into_iter()
                .map(|j| bases[f].iter().map(|r| r[j]).collect())
                .collect()
        };
        if parts[f]
            .replace(crate::matrix::transpose(&chosen))
            .is_some()
        {
            return Err(err(format!(
                "the effect {} names {} twice",
                statscraft_syntax::manova_effect_label(e),
                wsf[f].name
            )));
        }
    }
    let parts: Vec<Matrix> = parts
        .into_iter()
        .zip(bases)
        .map(|(p, b)| p.unwrap_or_else(|| b.iter().map(|r| vec![r[0]]).collect()))
        .collect();
    Ok(crate::matrix::transpose(&crate::glm::kronecker(&parts)))
}

/// All main effects and interactions of the factors, lower orders first.
fn full_factorial(factors: &[Factor]) -> Vec<statscraft_syntax::ManovaTerm> {
    let k = factors.len();
    let mut sets: Vec<Vec<usize>> = (1..1usize << k)
        .map(|m| (0..k).filter(|i| m & (1 << i) != 0).collect())
        .collect();
    sets.sort_by_key(|s| s.len());
    sets.into_iter()
        .map(|set| {
            let effect = ManovaEffect {
                crossed: set
                    .iter()
                    .map(|&i| ManovaItem {
                        names: vec![VarItem::Name(factors[i].name.clone())],
                        pool: false,
                        number: None,
                    })
                    .collect(),
                within: Vec::new(),
            };
            statscraft_syntax::ManovaTerm {
                label: statscraft_syntax::manova_effect_label(&effect),
                effects: vec![effect],
                define: None,
                against: None,
                muplus: false,
            }
        })
        .collect()
}

/// A shown test: its term, label and error, the test (None without an error term) and whether
/// it is one degree of freedom of a term (SINGLEDF).
struct Row {
    term: usize,
    label: String,
    error: ManovaError,
    test: Option<Test>,
    single: bool,
}

fn trace(m: &Matrix) -> f64 {
    (0..m.len()).map(|i| m[i][i]).sum()
}

/// A matrix with each off-diagonal element over the roots of its diagonal elements, and those
/// roots on the diagonal.
fn correlations(cov: &Matrix) -> Matrix {
    let k = cov.len();
    (0..k)
        .map(|i| {
            (0..k)
                .map(|j| {
                    if i == j {
                        cov[i][i].sqrt()
                    } else {
                        cov[i][j] / (cov[i][i] * cov[j][j]).sqrt()
                    }
                })
                .collect()
        })
        .collect()
}

/// Mauchly's test and the averaged tests of a within-subjects effect with several transformed
/// variables: for each measure the sums over its variables of the hypothesis and error sums of
/// squares, with the df times their number; with several measures also the multivariate tests
/// of those summed matrices.
#[allow(clippy::too_many_arguments)]
fn averaged_tables(
    w: &Within,
    tests: &[Row],
    error: Option<&(Matrix, f64)>,
    default: ManovaError,
    n: f64,
    pr: &ManovaPrint,
    power: Option<ManovaPower>,
    caption: &str,
) -> Vec<Table> {
    let k = w.per;
    let kf = k as f64;
    let nm = w.measures.len();
    let block = |m: &Matrix, a: usize| -> Matrix {
        (0..k)
            .map(|i| (0..k).map(|j| m[a * k + i][a * k + j]).collect())
            .collect()
    };
    // the Greenhouse-Geisser and Huynh-Feldt epsilons of measure `a`
    let epsilons = |e: &Matrix, dfe: f64, a: usize| -> (f64, f64) {
        let b = block(e, a);
        let tr2: f64 = b.iter().flatten().map(|x| x * x).sum();
        let gg = trace(&b).powi(2) / (kf * tr2);
        let hf = ((n * kf * gg - 2.0) / (kf * dfe - kf * kf * gg)).min(1.0);
        (gg, hf)
    };
    let label = |a: usize| {
        if nm > 1 {
            w.measures[a].clone()
        } else {
            w.label.clone()
        }
    };
    let mut out = Vec::new();
    if let Some((e, dfe)) = error {
        let rows = (0..nm)
            .map(|a| {
                let b = block(e, a);
                let reference: Vec<f64> = (0..k).map(|i| b[i][i]).collect();
                let (wv, chi, df, sig) = crate::glm::mauchly(&b, *dfe, &reference);
                let (gg, hf) = epsilons(e, *dfe, a);
                let mut row = vec![text(label(a))];
                row.extend(
                    [wv, chi, df, sig, gg, hf, 1.0 / kf]
                        .map(|v| number(v.is_finite().then_some(v))),
                );
                row
            })
            .collect();
        out.push(table(
            "Mauchly Sphericity Test",
            1,
            plain(&[
                "",
                "Mauchly's W",
                "Chi-Square",
                "DF",
                "Sig.",
                "Greenhouse-Geisser Epsilon",
                "Huynh-Feldt Epsilon",
                "Lower-bound Epsilon",
            ]),
            rows,
            Some(format!(
                "{caption}. From the error matrix of the orthonormal transformed variables"
            )),
        ));
    }
    if !pr.averf {
        return out;
    }
    let mut columns = plain(&["Source of Variation", "SS", "DF", "MS", "F", "Sig of F"]);
    if pr.gg {
        columns.extend(plain(&["Sig. (Greenhouse-Geisser)"]));
    }
    if pr.hf {
        columns.extend(plain(&["Sig. (Huynh-Feldt)"]));
    }
    if pr.efsize {
        columns.extend(plain(&["Partial Eta Sq."]));
    }
    if power.is_some() {
        columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
    }
    let mut errors: Vec<ManovaError> = vec![default];
    for t in tests {
        if !errors.contains(&t.error) {
            errors.push(t.error);
        }
    }
    for a in 0..nm {
        let mut rows = Vec::new();
        for &e in &errors {
            let group: Vec<(&Row, &Test)> = tests
                .iter()
                .filter(|r| !r.single && r.error == e)
                .filter_map(|r| r.test.as_ref().map(|t| (r, t)))
                .collect();
            let Some((_, first)) = group.first() else {
                continue;
            };
            let (sse, dfe) = (trace(&block(&first.e, a)), first.dfe * kf);
            let mut row = vec![
                text(error_name(e)),
                number(Some(sse)),
                number(Some(dfe)),
                number(Some(sse / dfe)),
            ];
            row.extend((4..columns.len()).map(|_| Cell::Empty));
            rows.push(row);
            for (r, t) in group {
                let (ssh, dfh) = (trace(&block(&t.h, a)), t.dfh * kf);
                let ok = t.ok();
                let f = (ssh / dfh) / (sse / dfe);
                let mut row = vec![
                    text(r.label.clone()),
                    number(Some(ssh)),
                    number(Some(dfh)),
                    number((dfh > 0.0).then(|| ssh / dfh)),
                    number(ok.then_some(f)),
                    number(ok.then(|| f_p(f, dfh, dfe))),
                ];
                let (gg, hf) = epsilons(&t.e, t.dfe, a);
                if pr.gg {
                    row.push(number(ok.then(|| f_p(f, dfh * gg, dfe * gg))));
                }
                if pr.hf {
                    row.push(number(ok.then(|| f_p(f, dfh * hf, dfe * hf))));
                }
                if pr.efsize {
                    row.push(number(Some(ssh / (ssh + sse))));
                }
                if let Some(w) = power {
                    let lp = ok.then(|| power_f(f, dfh, dfe, w.f_alpha));
                    row.push(number(lp.map(|x| x.0)));
                    row.push(number(lp.map(|x| x.1)));
                }
                rows.push(row);
            }
        }
        let what = if nm > 1 {
            format!("{caption}; {}", w.measures[a])
        } else {
            caption.to_string()
        };
        out.push(table(
            "Averaged Tests of Significance",
            1,
            columns.clone(),
            rows,
            Some(what),
        ));
    }
    if nm > 1 {
        let sum = |m: &Matrix| -> Matrix {
            (0..nm)
                .map(|a| {
                    (0..nm)
                        .map(|b| (0..k).map(|i| m[a * k + i][b * k + i]).sum())
                        .collect()
                })
                .collect()
        };
        let summed: Vec<(String, Test)> = tests
            .iter()
            .filter(|r| !r.single)
            .filter_map(|r| {
                r.test.as_ref().map(|t| {
                    (
                        r.label.clone(),
                        Test {
                            h: sum(&t.h),
                            e: sum(&t.e),
                            dfh: t.dfh * kf,
                            dfe: t.dfe * kf,
                        },
                    )
                })
            })
            .collect();
        let groups: Vec<(String, &Test)> = summed.iter().map(|(l, t)| (l.clone(), t)).collect();
        let only = ManovaPrint {
            multiv: true,
            efsize: pr.efsize,
            ..ManovaPrint::default()
        };
        for mut t in multivariate_tables(&groups, &w.measures, &only, power, caption) {
            t.title = format!("Averaged {}", t.title);
            out.push(t);
        }
    }
    out
}

fn multivariate_tables(
    groups: &[(String, &Test)],
    names: &[String],
    pr: &ManovaPrint,
    power: Option<ManovaPower>,
    caption: &str,
) -> Vec<Table> {
    let mut out = Vec::new();
    let p = names.len() as f64;
    if pr.brief {
        let rows = groups
            .iter()
            .map(|(label, t)| {
                let (l, _) = t.roots();
                let wilks: f64 = l.iter().map(|x| 1.0 / (1.0 + x)).product();
                let (f, d1, d2) = rao(wilks, p, t.dfh, t.dfe);
                let ok = t.ok();
                vec![
                    text(label.clone()),
                    number(Some(wilks)),
                    number(ok.then_some(f)),
                    number(Some(d1)),
                    number(Some(d2)),
                    number(ok.then(|| f_p(f, d1, d2))),
                ]
            })
            .collect();
        out.push(table(
            "Multivariate Tests of Significance",
            1,
            plain(&[
                "Source of Variation",
                "Wilks Lambda",
                "Approx. F",
                "Hypoth. DF",
                "Error DF",
                "Sig. of F",
            ]),
            rows,
            Some(caption.to_string()),
        ));
        return out;
    }
    let (mut mv, mut eig, mut dimr, mut uni, mut step) = (vec![], vec![], vec![], vec![], vec![]);
    for (label, t) in groups {
        let (l, re) = t.roots();
        let s = (re as f64).min(t.dfh);
        let m = ((re as f64 - t.dfh).abs() - 1.0) / 2.0;
        let nn = (t.dfe - re as f64 - 1.0) / 2.0;
        let head = format!("{label} (S = {s}, M = {m}, N = {nn})");
        let stats = multivariate(
            &t.h,
            &t.e,
            &(0..t.e.len())
                .map(|i| t.e[i][i] + t.h[i][i])
                .collect::<Vec<_>>(),
            t.dfh,
            t.dfe,
        );
        for (k, (_, value, f, d1, d2, eta)) in stats.into_iter().enumerate() {
            let roy = k == 3;
            let ok = t.ok() && d1 > 0.0 && d2 > 0.0 && f.is_finite() && !roy;
            let value = if roy {
                l.first().map_or(0.0, |x| x / (1.0 + x))
            } else {
                value
            };
            let mut row = vec![
                text(if k == 0 { head.clone() } else { String::new() }),
                text(["Pillais", "Wilks", "Hotellings", "Roys"][k]),
                number(t.ok().then_some(value)),
                number(ok.then_some(f)),
                number(ok.then_some(d1)),
                number(ok.then_some(d2)),
                number(ok.then(|| f_p(f, d1, d2))),
            ];
            if pr.efsize {
                row.push(number((t.ok() && eta.is_finite()).then_some(eta)));
            }
            if let Some(w) = power {
                // EXACT: with one hypothesis df every criterion is Hotelling's T^2 with its
                // exact F; APPROXIMATE: each criterion's eta squared over one less it, times
                // the error df of its F (Roy's root has no F)
                let lp = if w.exact {
                    let (q, total) = (re as f64, l.iter().sum::<f64>());
                    (t.ok() && t.dfh == 1.0 && t.dfe > q - 1.0).then(|| {
                        let lambda = total * t.dfe;
                        (lambda, power_lambda(lambda, q, t.dfe - q + 1.0, w.f_alpha))
                    })
                } else {
                    ok.then(|| {
                        let lambda = d2 * eta / (1.0 - eta);
                        (lambda, power_lambda(lambda, d1, d2, w.f_alpha))
                    })
                };
                row.push(number(lp.map(|x| x.0)));
                row.push(number(lp.map(|x| x.1)));
            }
            mv.push(row);
        }
        let total: f64 = l.iter().sum();
        let mut cum = 0.0;
        for (i, x) in l.iter().enumerate() {
            cum += x;
            eig.push(vec![
                text(if i == 0 { label.clone() } else { String::new() }),
                text(format!("{}", i + 1)),
                number(Some(*x)),
                number(Some(100.0 * x / total)),
                number(Some(100.0 * cum / total)),
                number(Some((x / (1.0 + x)).sqrt())),
            ]);
        }
        for k in 0..l.len() {
            let wilks: f64 = l[k..].iter().map(|x| 1.0 / (1.0 + x)).product();
            let (f, d1, d2) = rao(wilks, re as f64 - k as f64, t.dfh - k as f64, t.dfe);
            dimr.push(vec![
                text(if k == 0 { label.clone() } else { String::new() }),
                text(format!("{} TO {}", k + 1, l.len())),
                number(Some(wilks)),
                number(Some(f)),
                number(Some(d1)),
                number(Some(d2)),
                number(Some(f_p(f, d1, d2))),
            ]);
        }
        for (j, nm) in names.iter().enumerate() {
            let (f, sig) = t.univariate(j);
            let mut row = vec![
                text(if j == 0 { label.clone() } else { String::new() }),
                text(nm.clone()),
                number(Some(t.h[j][j])),
                number(Some(t.e[j][j])),
                number(Some(t.h[j][j] / t.dfh)),
                number(Some(t.e[j][j] / t.dfe)),
                number(t.ok().then_some(f)),
                number(t.ok().then_some(sig)),
            ];
            if pr.efsize {
                row.push(number(Some(t.h[j][j] / (t.h[j][j] + t.e[j][j]))));
            }
            if let Some(w) = power {
                let lp = t.ok().then(|| power_f(f, t.dfh, t.dfe, w.f_alpha));
                row.push(number(lp.map(|x| x.0)));
                row.push(number(lp.map(|x| x.1)));
            }
            uni.push(row);
        }
        // Roy-Bargmann stepdown: Cholesky factors of E and E + H
        if let (Some(le), Some(lt)) = (
            crate::matrix::cholesky(&t.e),
            crate::matrix::cholesky(&add(&t.e, &t.h)),
        ) {
            for (j, nm) in names.iter().enumerate() {
                let (te, tt) = (le[j][j] * le[j][j], lt[j][j] * lt[j][j]);
                let d2 = t.dfe - j as f64;
                let (hm, em) = ((tt - te) / t.dfh, te / d2);
                let f = hm / em;
                step.push(vec![
                    text(if j == 0 { label.clone() } else { String::new() }),
                    text(nm.clone()),
                    number(Some(hm)),
                    number(Some(em)),
                    number(Some(f)),
                    number(Some(t.dfh)),
                    number(Some(d2)),
                    number(Some(f_p(f, t.dfh, d2))),
                ]);
            }
        }
    }
    let note = Some(caption.to_string());
    let mut eta = if pr.efsize {
        plain(&["Partial Eta Sq."])
    } else {
        Vec::new()
    };
    if power.is_some() {
        eta.extend(plain(&["Noncent. Parameter", "Observed Power"]));
    }
    if pr.multiv {
        let mut columns = plain(&[
            "Effect",
            "Test Name",
            "Value",
            "Approx. F",
            "Hypoth. DF",
            "Error DF",
            "Sig. of F",
        ]);
        columns.extend(eta.clone());
        out.push(table(
            "Multivariate Tests of Significance",
            2,
            columns,
            mv,
            Some(format!(
                "{caption}. Roys is the largest root as lambda / (1 + lambda)"
            )),
        ));
    }
    if pr.eigen {
        out.push(table(
            "Eigenvalues and Canonical Correlations",
            2,
            plain(&[
                "Effect",
                "Root No.",
                "Eigenvalue",
                "Pct.",
                "Cum. Pct.",
                "Canon Cor.",
            ]),
            eig,
            note.clone(),
        ));
    }
    if pr.dimenr {
        out.push(table(
            "Dimension Reduction Analysis",
            2,
            plain(&[
                "Effect",
                "Roots",
                "Wilks L.",
                "F",
                "Hypoth. DF",
                "Error DF",
                "Sig. of F",
            ]),
            dimr,
            note.clone(),
        ));
    }
    if pr.univ {
        let mut columns = plain(&[
            "Effect",
            "Variable",
            "Hypoth. SS",
            "Error SS",
            "Hypoth. MS",
            "Error MS",
            "F",
            "Sig. of F",
        ]);
        columns.extend(eta);
        out.push(table("Univariate F-tests", 2, columns, uni, note.clone()));
    }
    if pr.stepdown {
        out.push(table(
            "Roy-Bargmann Stepdown F-tests",
            2,
            plain(&[
                "Effect",
                "Variable",
                "Hypoth. MS",
                "Error MS",
                "StepDown F",
                "Hypoth. DF",
                "Error DF",
                "Sig. of F",
            ]),
            step,
            note,
        ));
    }
    out
}

/// How the estimates are shown: the interval, EFSIZE, the significance level of POWER for t
/// tests and the number of dependents (for multivariate intervals).
#[derive(Clone, Copy)]
struct Est {
    iv: ManovaInterval,
    efsize: bool,
    power: Option<f64>,
    p: usize,
}

impl Est {
    fn new(d: &ManovaDesign, p: usize) -> Est {
        Est {
            iv: d.cinterval,
            efsize: d.print.efsize,
            power: d.power.map(|w| w.t_alpha),
            p,
        }
    }

    fn columns(&self) -> Vec<(String, String)> {
        let pct = format!("{}", (self.iv.level * 1e8).round() / 1e6);
        let mut columns = plain(&["Std. Err.", "t-Value", "Sig. t"]);
        columns.push((String::new(), format!("Lower -{pct}%")));
        columns.push((String::new(), "CL- Upper".into()));
        if self.efsize {
            columns.extend(plain(&["Partial Eta Sq."]));
        }
        if self.power.is_some() {
            columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
        }
        columns
    }
}

/// The multiplier of the standard error for the interval of an estimate in a family of `nh`
/// (the hypothesis df of a joint interval) with `ne` error df and `p` dependents.
fn multiplier(iv: &ManovaInterval, p: usize, nh: f64, ne: f64) -> f64 {
    let alpha = 1.0 - iv.level;
    // the multivariate types mean nothing for one dependent
    let Some(kind) = iv.multivariate.filter(|_| p > 1) else {
        return if !iv.joint {
            t_quantile(alpha / 2.0, ne)
        } else if iv.bonferroni {
            t_quantile(alpha / (2.0 * nh), ne)
        } else {
            (nh * f_quantile(alpha, nh, ne)).sqrt()
        };
    };
    // an individual multivariate interval is a family with one hypothesis df
    let nh = if iv.joint { nh } else { 1.0 };
    let p = p as f64;
    let s = p.min(nh);
    // each criterion's critical value of the largest root over one less it, from its F
    let c = match kind {
        ManovaCriterion::Bonferroni => return t_quantile(alpha / (2.0 * p * nh), ne),
        ManovaCriterion::Roy if s == 1.0 => {
            let (d1, d2) = (p.max(nh), ne - p.max(nh) + 1.0);
            d1 * f_quantile(alpha, d1, d2) / d2
        }
        // the largest-root distribution for s > 1 is refused before the tables are made
        ManovaCriterion::Roy => f64::NAN,
        ManovaCriterion::Wilks => {
            let b = p * p + nh * nh - 5.0;
            let r = if b > 0.0 {
                ((p * p * nh * nh - 4.0) / b).sqrt()
            } else {
                1.0
            };
            let d1 = p * nh;
            let d2 = (ne - (p - nh + 1.0) / 2.0) * r - (d1 - 2.0) / 2.0;
            let w = (1.0 + d1 * f_quantile(alpha, d1, d2) / d2).powf(-r);
            (1.0 - w) / w
        }
        ManovaCriterion::Hotelling => {
            let (m, n) = (((nh - p).abs() - 1.0) / 2.0, (ne - p - 1.0) / 2.0);
            let (d1, d2) = (s * (2.0 * m + s + 1.0), 2.0 * (s * n + 1.0));
            s * d1 * f_quantile(alpha, d1, d2) / d2
        }
        ManovaCriterion::Pillai => {
            let (d1, d2) = (s * p.max(nh), s * (ne - p + s));
            let d = f_quantile(alpha, d1, d2) * d1 / d2;
            let v = s * d / (d + 1.0);
            v / (1.0 - v)
        }
    };
    (ne * c).sqrt()
}

/// An orthogonal rotation of `loadings` (variables down), with Kaiser normalization, and a
/// note when it did not converge.
fn rotated(loadings: &Matrix, kind: Rotation) -> (Matrix, Option<String>) {
    let criteria = FactorCriteria {
        iterate: 1000,
        rconverge: 1e-10,
        ..FactorCriteria::default()
    };
    let mut warnings = Vec::new();
    let r = crate::factor::orthogonal(loadings, kind, &criteria, &mut warnings);
    let note = (!warnings.is_empty()).then(|| "the rotation did not converge".to_string());
    (r.pattern, note)
}

/// A table of variables down and `m` columns named `word 1`, `word 2` and so on.
fn columns_table(
    title: &str,
    word: &str,
    rows: Vec<Vec<Cell>>,
    heads: usize,
    m: usize,
    caption: String,
) -> Table {
    let mut columns = plain(&vec![""; heads]);
    columns.extend((1..=m).map(|k| (String::new(), format!("{word} {k}"))));
    table(title, heads, columns, rows, Some(caption))
}

/// PCOMPS: the principal components of the error correlation and covariance matrices after
/// the covariates. The loadings are each eigenvector times the root of its eigenvalue, with
/// a positive column sum; MINEIGEN drops the smaller components, NCOMP (at least 2) limits
/// the ones rotated.
fn principal_components(
    pc: &ManovaPcomps,
    names: &[String],
    cov: &Matrix,
    caption: &str,
) -> Vec<Table> {
    let p = names.len();
    let cor: Matrix = (0..p)
        .map(|i| {
            (0..p)
                .map(|j| cov[i][j] / (cov[i][i] * cov[j][j]).sqrt())
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    for (on, m, what) in [
        (pc.cor, &cor, "error correlation matrix"),
        (pc.cov, cov, "error variance-covariance matrix"),
    ] {
        if !on {
            continue;
        }
        let pairs = crate::matrix::symmetric_eigen(m);
        let total: f64 = pairs.iter().map(|x| x.0).sum();
        let mut cum = 0.0;
        let rows = pairs
            .iter()
            .enumerate()
            .map(|(i, (value, _))| {
                cum += value;
                vec![
                    text(format!("{}", i + 1)),
                    number(Some(*value)),
                    number(Some(100.0 * value / total)),
                    number(Some(100.0 * cum / total)),
                ]
            })
            .collect();
        let cap = format!("Of the {what}; {caption}");
        out.push(table(
            "Principal Components",
            1,
            plain(&["Component", "Eigenvalue", "Pct of Var", "Cum Pct"]),
            rows,
            Some(cap.clone()),
        ));
        let kept = pairs
            .iter()
            .take_while(|(v, _)| *v > 0.0 && *v >= pc.min_eigen)
            .count();
        if kept == 0 {
            continue;
        }
        let mut loadings = crate::factor::eigen_loadings(&pairs, kept);
        crate::factor::reflect(&mut loadings, kept);
        let rows = |l: &Matrix| -> Vec<Vec<Cell>> {
            names
                .iter()
                .zip(l)
                .map(|(n, r)| {
                    let mut row = vec![text(n.clone())];
                    row.extend(r.iter().map(|v| number(Some(*v))));
                    row
                })
                .collect()
        };
        out.push(columns_table(
            "Component Loadings",
            "Component",
            rows(&loadings),
            1,
            kept,
            cap.clone(),
        ));
        let r = pc.ncomp.unwrap_or(p).max(2).min(kept);
        if pc.rotate != Rotation::None && r >= 2 {
            let first: Matrix = loadings.iter().map(|row| row[..r].to_vec()).collect();
            let (l, note) = rotated(&first, pc.rotate);
            let cap = match note {
                Some(n) => format!("{cap}; {n}"),
                None => cap,
            };
            out.push(columns_table(
                "Rotated Component Loadings",
                "Component",
                rows(&l),
                1,
                r,
                format!(
                    "{cap}; {} rotation",
                    format!("{:?}", pc.rotate).to_uppercase()
                ),
            ));
        }
    }
    out
}

/// DISCRIM: for each multivariate effect (and the covariates), the discriminant functions
/// H v = lambda E v that the dimension reduction tests keep at ALPHA, each scaled so that its
/// variance within the error is 1 (v' E v / dfe = 1) and signed so that its correlations with
/// the dependents add up to more than 0. Standardized coefficients multiply by the error
/// standard deviations; ESTIM gives the effect's parameter estimates times the coefficients.
fn discriminant_tables(
    dc: &ManovaDiscrim,
    groups: &[(String, &Test)],
    estimates: &[Vec<(String, Vec<f64>)>],
    names: &[String],
    caption: &str,
) -> Vec<Table> {
    let p = names.len();
    let (mut raw, mut stan, mut est, mut cor, mut rot) = (vec![], vec![], vec![], vec![], vec![]);
    let (mut most, mut most_rotated) = (0, 0);
    let mut notes: Vec<String> = Vec::new();
    for ((label, t), estimates) in groups.iter().zip(estimates) {
        let Some(l) = crate::matrix::cholesky(&t.e).filter(|_| t.ok()) else {
            continue;
        };
        let (roots, re) = t.roots();
        let mut k = 0;
        while k < roots.len() && dc.alpha > 0.0 {
            let wilks: f64 = roots[k..].iter().map(|x| 1.0 / (1.0 + x)).product();
            let (f, d1, d2) = rao(wilks, re as f64 - k as f64, t.dfh - k as f64, t.dfe);
            let sig = f_p(f, d1, d2);
            if sig.is_nan() || sig > dc.alpha {
                break;
            }
            k += 1;
        }
        if k == 0 {
            continue;
        }
        // with E = L L', the eigenvectors u of L^-1 H L^-T give v = L^-T u
        let Some((li, _)) = inverse(&l) else {
            continue;
        };
        let lt = crate::matrix::transpose(&li);
        let m = crate::matrix::multiply(&crate::matrix::multiply(&li, &t.h), &lt);
        let pairs = crate::matrix::symmetric_eigen(&m);
        let mut v: Matrix = (0..p)
            .map(|i| {
                (0..k)
                    .map(|f| (0..p).map(|a| lt[i][a] * pairs[f].1[a]).sum::<f64>() * t.dfe.sqrt())
                    .collect()
            })
            .collect();
        let sd: Vec<f64> = (0..p).map(|i| (t.e[i][i] / t.dfe).sqrt()).collect();
        let mut c: Matrix = (0..p)
            .map(|i| {
                (0..k)
                    .map(|f| (0..p).map(|a| t.e[i][a] * v[a][f]).sum::<f64>() / t.dfe / sd[i])
                    .collect()
            })
            .collect();
        let signs = crate::factor::reflect(&mut c, k);
        v.iter_mut()
            .for_each(|row| row.iter_mut().zip(&signs).for_each(|(x, s)| *x *= s));
        most = most.max(k);
        let push = |to: &mut Vec<Vec<Cell>>, rows: Vec<(String, Vec<f64>)>| {
            for (i, (n, r)) in rows.into_iter().enumerate() {
                let mut row = vec![
                    text(if i == 0 { label.clone() } else { String::new() }),
                    text(n),
                ];
                row.extend(r.iter().map(|x| number(Some(*x))));
                to.push(row);
            }
        };
        let by_name = |m: &Matrix| -> Vec<(String, Vec<f64>)> {
            names.iter().cloned().zip(m.iter().cloned()).collect()
        };
        push(&mut raw, by_name(&v));
        let s: Matrix = (0..p)
            .map(|i| v[i].iter().map(|x| x * sd[i]).collect())
            .collect();
        push(&mut stan, by_name(&s));
        push(
            &mut est,
            estimates
                .iter()
                .map(|(n, b)| {
                    let row = (0..k)
                        .map(|f| (0..p).map(|j| b[j] * v[j][f]).sum())
                        .collect();
                    (n.clone(), row)
                })
                .collect(),
        );
        push(&mut cor, by_name(&c));
        if dc.rotate != Rotation::None && k >= 2 {
            let (r, note) = rotated(&c, dc.rotate);
            notes.extend(note.map(|n| format!("{label}: {n}")));
            most_rotated = most_rotated.max(k);
            push(&mut rot, by_name(&r));
        }
    }
    let pad = |rows: Vec<Vec<Cell>>, m: usize| -> Vec<Vec<Cell>> {
        rows.into_iter()
            .map(|mut r| {
                r.resize(m + 2, Cell::Empty);
                r
            })
            .collect()
    };
    let mut out = Vec::new();
    let note = format!("{caption}; functions kept at ALPHA {}", dc.alpha);
    for (on, title, rows, m) in [
        (dc.raw, "Raw Discriminant Function Coefficients", raw, most),
        (
            dc.stan,
            "Standardized Discriminant Function Coefficients",
            stan,
            most,
        ),
        (
            dc.estim,
            "Estimates of Effects for Canonical Variables",
            est,
            most,
        ),
        (
            dc.cor,
            "Correlations between Dependent and Canonical Variables",
            cor,
            most,
        ),
        (
            true,
            "Rotated Correlations between Dependent and Canonical Variables",
            rot,
            most_rotated,
        ),
    ] {
        if on && !rows.is_empty() {
            let mut cap = note.clone();
            if title.starts_with("Rotated") {
                cap = format!(
                    "{cap}; {} rotation",
                    format!("{:?}", dc.rotate).to_uppercase()
                );
                for n in &notes {
                    cap = format!("{cap}; {n}");
                }
            }
            out.push(columns_table(title, "Function", pad(rows, m), 2, m, cap));
        }
    }
    out
}

/// t, its two-sided significance, the confidence interval CINTERVAL asks for (`nh` estimates
/// in the joint family), with EFSIZE the partial eta squared t^2 / (t^2 + df) and with POWER
/// the noncentrality |t| and the power of the two-sided test.
fn t_cells(b: f64, se: f64, df: f64, nh: f64, est: Est) -> Vec<Cell> {
    let ok = se > 0.0 && df > 0.0;
    let t = b / se;
    let half = if ok {
        multiplier(&est.iv, est.p, nh, df) * se
    } else {
        f64::NAN
    };
    let half = half.is_finite().then_some(half);
    let mut cells = vec![
        number(Some(b)),
        number(ok.then_some(se)),
        number(ok.then_some(t)),
        number(ok.then(|| t_p(t, df))),
        number(half.map(|h| b - h)),
        number(half.map(|h| b + h)),
    ];
    if est.efsize {
        cells.push(number(ok.then(|| t * t / (t * t + df))));
    }
    if let Some(alpha) = est.power {
        let (c, power) = power_t(t, df, alpha);
        cells.push(number(ok.then_some(c)));
        cells.push(number(ok.then_some(power)));
    }
    cells
}

/// The within-error regression of the dependents on the covariates. `se` is the error SSCP of
/// dependents and covariates.
fn coefficients(
    se: &Matrix,
    dfe: f64,
    names: &[String],
    covs: &[String],
    est: Est,
    caption: &str,
) -> Table {
    let p = names.len();
    let (m, swept) = sweep_in_order(se, &(p..se.len()).collect::<Vec<_>>(), EPS);
    let nq = swept.iter().filter(|&&s| s).count() as f64;
    let dfe = dfe - nq;
    let mut rows = Vec::new();
    for (j, name) in names.iter().enumerate() {
        for (k, cov) in covs.iter().enumerate() {
            let c = p + k;
            let mut row = vec![text(name.clone()), text(cov.clone())];
            if swept[c] {
                row.extend(t_cells(
                    m[j][c],
                    (m[j][j] / dfe * m[c][c]).sqrt(),
                    dfe,
                    nq,
                    est,
                ));
            }
            rows.push(row);
        }
    }
    let mut columns = plain(&["Dependent", "Covariate", "B"]);
    columns.extend(est.columns());
    table(
        "Regression Coefficients",
        2,
        columns,
        rows,
        Some(caption.to_string()),
    )
}

/// The estimates of every column of the design for each dependent, with the error of the
/// term's test (or `fallback`) for their standard errors.
#[allow(clippy::too_many_arguments)]
fn parameters(
    terms: &[Term],
    tests: &[Row],
    fallback: (&Matrix, f64),
    full: &Matrix,
    swept: &[bool],
    nx: usize,
    names: &[String],
    d: &ManovaDesign,
    caption: &str,
) -> Table {
    let p = names.len();
    let est = Est::new(d, p);
    let mut rows = Vec::new();
    for (j, name) in names.iter().enumerate() {
        for (i, term) in terms.iter().enumerate() {
            let (e, dfe) = tests
                .iter()
                .find(|t| t.term == i && !t.single)
                .and_then(|t| t.test.as_ref())
                .map_or(fallback, |t| (&t.e, t.dfe));
            let mse = e[j][j] / dfe;
            let nh = term.cols.clone().filter(|&c| swept[c]).count() as f64;
            let head = |label: String| -> Vec<Cell> {
                let mut row = Vec::new();
                if p > 1 {
                    row.push(text(name.clone()));
                }
                row.push(text(term.label.clone()));
                row.push(text(label));
                row
            };
            for c in term.cols.clone() {
                let mut row = head(format!("{}", c + 1));
                if swept[c] {
                    row.extend(t_cells(
                        full[nx + j][c],
                        (mse * full[c][c]).sqrt(),
                        dfe,
                        nh,
                        est,
                    ));
                }
                rows.push(row);
            }
            if d.print.negsum && !term.constant && term.cols.clone().all(|c| swept[c]) {
                let b: f64 = -term.cols.clone().map(|c| full[nx + j][c]).sum::<f64>();
                let v: f64 = term
                    .cols
                    .clone()
                    .flat_map(|a| term.cols.clone().map(move |c| (a, c)))
                    .map(|(a, c)| full[a][c])
                    .sum();
                let mut row = head("NEGSUM".into());
                row.extend(t_cells(b, (mse * v).sqrt(), dfe, nh, est));
                rows.push(row);
            }
        }
    }
    let mut columns = if p > 1 {
        plain(&["Dependent"])
    } else {
        Vec::new()
    };
    columns.extend(plain(&["Term", "Parameter", "Coeff."]));
    columns.extend(est.columns());
    let heads = if p > 1 { 3 } else { 2 };
    table(
        "Parameter Estimates",
        heads,
        columns,
        rows,
        Some(caption.to_string()),
    )
}

/// PARAMETERS(COR): the covariance factors of the estimates (the inverse of the cross products
/// of the design with the covariates, design part) and their correlations.
fn parameter_correlations(
    zwz: &Matrix,
    order: &[usize],
    swept: &[bool],
    nx: usize,
    caption: &str,
) -> Vec<Table> {
    let kept: Vec<usize> = order.iter().copied().filter(|&c| swept[c]).collect();
    let sub: Matrix = kept
        .iter()
        .map(|&a| kept.iter().map(|&b| zwz[a][b]).collect())
        .collect();
    let Some((inv, _)) = inverse(&sub) else {
        return Vec::new();
    };
    let design: Vec<usize> = (0..kept.len()).filter(|&i| kept[i] < nx).collect();
    let factors: Matrix = design
        .iter()
        .map(|&a| design.iter().map(|&b| inv[a][b]).collect())
        .collect();
    let names: Vec<String> = design.iter().map(|&i| format!("{}", kept[i] + 1)).collect();
    vec![
        matrix_table(
            "Covariance Factors of Parameter Estimates",
            &names,
            &factors,
            caption.to_string(),
        ),
        matrix_table(
            "Correlations of Parameter Estimates",
            &names,
            &correlations(&factors),
            format!("{caption}; the roots of the covariance factors on the diagonal"),
        ),
    ]
}

/// PARAMETERS(ORTHO): the orthogonal estimates of the design columns in order, without the
/// covariates; the squares of a term's estimates add up to its sequential sum of squares.
fn orthogonal_estimates(
    terms: &[Term],
    zwz: &Matrix,
    nx: usize,
    names: &[String],
    caption: &str,
) -> Table {
    let p = names.len();
    let mut m = zwz.clone();
    let mut est: Vec<Option<Vec<f64>>> = vec![None; nx];
    for (c, e) in est.iter_mut().enumerate() {
        if zwz[c][c] > 0.0 && m[c][c] > EPS * zwz[c][c] {
            *e = Some((0..p).map(|j| m[c][nx + j] / m[c][c].sqrt()).collect());
            crate::matrix::sweep(&mut m, c);
        }
    }
    let mut rows = Vec::new();
    for (j, name) in names.iter().enumerate() {
        for term in terms {
            for c in term.cols.clone() {
                let mut row = Vec::new();
                if p > 1 {
                    row.push(text(name.clone()));
                }
                row.push(text(term.label.clone()));
                row.push(text(format!("{}", c + 1)));
                row.push(number(est[c].as_ref().map(|e| e[j])));
                rows.push(row);
            }
        }
    }
    let mut columns = if p > 1 {
        plain(&["Dependent"])
    } else {
        Vec::new()
    };
    columns.extend(plain(&["Term", "Parameter", "Estimate"]));
    table(
        "Orthogonal Estimates",
        if p > 1 { 3 } else { 2 },
        columns,
        rows,
        Some(caption.to_string()),
    )
}

/// PRINT DESIGN: the design matrix of the cells (OVERALL) and each factor's one-way basis.
fn design_tables(
    s: &Setup,
    x: &[Vec<f64>],
    bases: &[Matrix],
    pr: &ManovaPrint,
    continuous: bool,
    caption: &str,
) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    if pr.overall {
        if continuous {
            return Err(err(
                "PRINT DESIGN is not available when DESIGN names a continuous variable",
            ));
        }
        let mut columns = plain(&["Cell"]);
        columns.extend(s.factors.iter().map(|f| (String::new(), f.name.clone())));
        columns.extend((1..=x.len()).map(|c| (String::new(), format!("{c}"))));
        let mut seen = Vec::new();
        let mut rows = Vec::new();
        let mut order: Vec<usize> = (0..s.cases.len()).collect();
        order.sort_by_key(|&i| s.cases[i].cell);
        for i in order {
            let c = &s.cases[i];
            if seen.contains(&c.cell) {
                continue;
            }
            seen.push(c.cell);
            let mut row = vec![text(format!("{}", seen.len()))];
            row.extend(levels_text(s.factors, &c.levels));
            row.extend(x.iter().map(|col| number(Some(col[i]))));
            rows.push(row);
        }
        out.push(table(
            "Design Matrix",
            1,
            columns,
            rows,
            Some(caption.to_string()),
        ));
    }
    if pr.oneway {
        for (f, b) in s.factors.iter().zip(bases) {
            let mut columns = plain(&[&f.name]);
            columns.extend((1..=f.k).map(|c| (String::new(), format!("{c}"))));
            let rows = b
                .iter()
                .enumerate()
                .map(|(l, r)| {
                    let mut row = vec![text(format!("{}", f.min + l as f64))];
                    row.extend(r.iter().map(|v| number(Some(*v))));
                    row
                })
                .collect();
            out.push(table(
                "One-Way Basis Matrix",
                1,
                columns,
                rows,
                Some(f.name.clone()),
            ));
        }
    }
    Ok(out)
}

/// The design part of the fit of dependent row `row` for case `i`, from the swept columns.
fn fit(x: &[Vec<f64>], m: (&Matrix, &[bool]), row: usize, i: usize) -> f64 {
    (0..x.len())
        .filter(|&c| m.1[c])
        .map(|c| x[c][i] * m.0[row][c])
        .sum()
}

/// Observed and predicted cell means of each dependent with the residuals, and with TABLES the
/// adjusted means collapsed over the other factors.
#[allow(clippy::too_many_arguments)]
fn predicted_means(
    s: &Setup,
    x: &[Vec<f64>],
    full: (&Matrix, &[bool]),
    design_only: (&Matrix, &[bool]),
    nx: usize,
    dv: &[usize],
    names: &[String],
    keep: &[usize],
    specs: &[Vec<String>],
    cov: &Matrix,
    caption: &str,
) -> Result<Vec<Table>, Error> {
    let groups = s.groups();
    let mut rows = Vec::new();
    // each kept dependent's adjusted mean in each cell, with the cell's weight
    let mut adjusted: Vec<Vec<(f64, f64)>> = Vec::new();
    for &j in keep {
        let mut cells = Vec::new();
        for g in &groups {
            let w: f64 = g.iter().map(|c| c.w).sum();
            let obs = g.iter().map(|c| c.w * c.v[dv[j]]).sum::<f64>() / w;
            let i = s
                .cases
                .iter()
                .position(|c| std::ptr::eq(c, g[0]))
                .expect("a case of the cell");
            let (adj, est) = (fit(x, full, nx + j, i), fit(x, design_only, nx + j, i));
            let mut row = vec![text(names[j].clone())];
            row.extend(levels_text(s.factors, &g[0].levels));
            row.extend(
                [obs, adj, est, obs - est, (obs - est) / cov[j][j].sqrt()].map(|v| number(Some(v))),
            );
            rows.push(row);
            cells.push((adj, w));
        }
        adjusted.push(cells);
    }
    let mut columns = plain(&["Variable"]);
    columns.extend(s.factors.iter().map(|f| (String::new(), f.name.clone())));
    columns.extend(plain(&[
        "Obs. Mean",
        "Adj. Mean",
        "Est. Mean",
        "Raw Resid.",
        "Std. Resid.",
    ]));
    let mut out = vec![table(
        "Observed and Predicted Means",
        1 + s.factors.len(),
        columns,
        rows,
        Some(caption.to_string()),
    )];
    for spec in specs {
        let fs: Vec<usize> = spec
            .iter()
            .map(|n| {
                s.factors
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(n))
                    .ok_or_else(|| err(format!("PMEANS TABLES names {n}, which is not a factor")))
            })
            .collect::<Result<_, _>>()?;
        let key = |c: &Case| -> Vec<usize> { fs.iter().map(|&f| c.levels[f]).collect() };
        let mut keys: Vec<Vec<usize>> = groups.iter().map(|g| key(g[0])).collect();
        keys.sort();
        keys.dedup();
        let mut rows = Vec::new();
        for (cells, &j) in adjusted.iter().zip(keep) {
            for k in &keys {
                let mine: Vec<(f64, f64)> = groups
                    .iter()
                    .zip(cells)
                    .filter(|(g, _)| key(g[0]) == *k)
                    .map(|(_, c)| *c)
                    .collect();
                let w: f64 = mine.iter().map(|c| c.1).sum();
                let mut row = vec![text(names[j].clone())];
                row.extend(
                    fs.iter()
                        .zip(k)
                        .map(|(&f, l)| text(format!("{}", s.factors[f].min + *l as f64))),
                );
                row.push(number(Some(
                    mine.iter().map(|c| c.0 * c.1).sum::<f64>() / w,
                )));
                row.push(number(Some(
                    mine.iter().map(|c| c.0).sum::<f64>() / mine.len() as f64,
                )));
                rows.push(row);
            }
        }
        let mut columns = plain(&["Variable"]);
        columns.extend(
            fs.iter()
                .map(|&f| (String::new(), s.factors[f].name.clone())),
        );
        columns.extend(plain(&["Weighted Mean", "Unweighted Mean"]));
        let what = if spec.is_empty() {
            "CONSTANT".to_string()
        } else {
            spec.join(" BY ").to_uppercase()
        };
        out.push(table(
            "Combined Adjusted Means",
            1 + fs.len(),
            columns,
            rows,
            Some(format!("{what}; {caption}")),
        ));
    }
    Ok(out)
}

/// RESIDUALS: each case's observed and predicted value (design and covariates), the residual
/// and the residual over the error standard deviation.
#[allow(clippy::too_many_arguments)]
fn casewise(
    s: &Setup,
    x: &[Vec<f64>],
    full: (&Matrix, &[bool]),
    (nx, p): (usize, usize),
    dv: &[usize],
    cv: &[usize],
    names: &[String],
    cov: &Matrix,
    caption: &str,
) -> Table {
    let mut rows = Vec::new();
    for (i, c) in s.cases.iter().enumerate() {
        for (j, name) in names.iter().enumerate() {
            let covs: f64 = cv
                .iter()
                .enumerate()
                .filter(|(k, _)| full.1[nx + p + k])
                .map(|(k, &v)| c.v[v] * full.0[nx + j][nx + p + k])
                .sum();
            let pred = fit(x, full, nx + j, i) + covs;
            let obs = c.v[dv[j]];
            let mut row = vec![text(format!("{}", c.index + 1))];
            if p > 1 {
                row.push(text(name.clone()));
            }
            row.extend(
                [obs, pred, obs - pred, (obs - pred) / cov[j][j].sqrt()].map(|v| number(Some(v))),
            );
            rows.push(row);
        }
    }
    let mut columns = plain(&["Case No."]);
    if p > 1 {
        columns.extend(plain(&["Variable"]));
    }
    columns.extend(plain(&[
        "Observed",
        "Predicted",
        "Raw Resid.",
        "Std. Resid.",
    ]));
    table(
        "Observed and Predicted Values for Each Case",
        if p > 1 { 2 } else { 1 },
        columns,
        rows,
        Some(caption.to_string()),
    )
}

/// The weighted mean, standard deviation and N of variable `v` over `cases`.
fn moments(cases: &[&Case], v: usize) -> (f64, f64, f64) {
    let w: f64 = cases.iter().map(|c| c.w).sum();
    let mean = cases.iter().map(|c| c.w * c.v[v]).sum::<f64>() / w;
    let ss: f64 = cases.iter().map(|c| c.w * (c.v[v] - mean).powi(2)).sum();
    (mean, (ss / (w - 1.0)).sqrt(), w)
}

impl Setup<'_> {
    fn glm_data(&self, y: &[usize]) -> Data {
        Data {
            factors: Vec::new(),
            factor_texts: Vec::new(),
            values: Vec::new(),
            labels: Vec::new(),
            labelled: false,
            covariates: Vec::new(),
            case: (0..self.cases.len()).collect(),
            level: self.cases.iter().map(|c| vec![c.cell]).collect(),
            cov: vec![Vec::new(); self.cases.len()],
            y: self
                .cases
                .iter()
                .map(|c| y.iter().map(|&j| c.v[j]).collect())
                .collect(),
            f: self.cases.iter().map(|c| c.w).collect(),
            g: vec![1.0; self.cases.len()],
        }
    }

    /// The non-empty cells with their cases, in cell order.
    fn groups(&self) -> Vec<Vec<&Case>> {
        let mut cells: std::collections::BTreeMap<usize, Vec<&Case>> = Default::default();
        for c in self.cases {
            cells.entry(c.cell).or_default().push(c);
        }
        cells.into_values().collect()
    }

    /// The tables shown once before the designs: CELLINFO, OMEANS and HOMOGENEITY.
    fn cell_tables(&self, c: &Manova, ndeps: usize) -> Result<Vec<Table>, Error> {
        let mut out = Vec::new();
        let groups = self.groups();
        let nf = self.factors.len();
        let factor_columns = || -> Vec<(String, String)> {
            self.factors
                .iter()
                .map(|f| (String::new(), f.name.clone()))
                .collect()
        };
        if c.cellinfo[0] {
            let mut rows = Vec::new();
            for (v, name) in self.cont.iter().enumerate() {
                let mut add = |levels: Vec<Cell>, cases: &[&Case]| {
                    let (mean, sd, n) = moments(cases, v);
                    let half = t_quantile(0.025, n - 1.0) * sd / n.sqrt();
                    let mut row = vec![text(name.clone())];
                    row.extend(levels);
                    row.extend([mean, sd, n, mean - half, mean + half].map(|x| number(Some(x))));
                    rows.push(row);
                };
                for g in &groups {
                    add(levels_text(self.factors, &g[0].levels), g);
                }
                if nf > 0 {
                    let mut levels = vec![text("For entire sample")];
                    levels.extend((1..nf).map(|_| Cell::Empty));
                    add(levels, &self.cases.iter().collect::<Vec<_>>());
                }
            }
            let mut columns = plain(&["Variable"]);
            columns.extend(factor_columns());
            columns.extend(plain(&[
                "Mean",
                "Std. Dev.",
                "N",
                "Lower 95% CI",
                "Upper 95% CI",
            ]));
            out.push(table(
                "Cell Means and Standard Deviations",
                1 + nf,
                columns,
                rows,
                None,
            ));
        }
        if c.cellinfo[1..].iter().any(|&b| b) {
            let mut columns = plain(&["Cell"]);
            columns.extend(factor_columns());
            let rows = groups
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    let mut row = vec![text(format!("{}", i + 1))];
                    row.extend(levels_text(self.factors, &g[0].levels));
                    row
                })
                .collect();
            out.push(table("Cell Numbers", 1, columns, rows, None));
            let k = self.cont.len();
            for (i, g) in groups.iter().enumerate() {
                let means: Vec<f64> = (0..k).map(|v| moments(g, v).0).collect();
                let w: f64 = g.iter().map(|c| c.w).sum();
                let sscp: Matrix = (0..k)
                    .map(|a| {
                        (0..k)
                            .map(|b| {
                                g.iter()
                                    .map(|c| c.w * (c.v[a] - means[a]) * (c.v[b] - means[b]))
                                    .sum()
                            })
                            .collect()
                    })
                    .collect();
                let cov: Matrix = sscp
                    .iter()
                    .map(|r| r.iter().map(|x| x / (w - 1.0)).collect())
                    .collect();
                let cap = format!("Cell {}", i + 1);
                if c.cellinfo[1] {
                    out.push(matrix_table(
                        "Within-Cell SSCP Matrix",
                        self.cont,
                        &sscp,
                        cap.clone(),
                    ));
                }
                if c.cellinfo[2] {
                    out.push(matrix_table(
                        "Within-Cell Variance-Covariance Matrix",
                        self.cont,
                        &cov,
                        cap.clone(),
                    ));
                }
                if c.cellinfo[3] {
                    let cor: Matrix = (0..k)
                        .map(|a| {
                            (0..k)
                                .map(|b| {
                                    if a == b {
                                        cov[a][a].sqrt()
                                    } else {
                                        cov[a][b] / (cov[a][a] * cov[b][b]).sqrt()
                                    }
                                })
                                .collect()
                        })
                        .collect();
                    out.push(matrix_table(
                        "Within-Cell Correlation Matrix",
                        self.cont,
                        &cor,
                        format!("{cap}; standard deviations on the diagonal"),
                    ));
                }
            }
        }
        if let Some(o) = &c.omeans {
            let vars: Vec<usize> = match &o.variables {
                None => (0..self.cont.len()).collect(),
                Some(items) => items
                    .iter()
                    .map(|i| match i {
                        VarItem::Name(n) => find(self.cont, n)
                            .ok_or_else(|| err(format!("OMEANS VARIABLES names {n}, which is not a dependent variable or covariate"))),
                        _ => Err(err("OMEANS VARIABLES takes variable names")),
                    })
                    .collect::<Result<_, _>>()?,
            };
            for spec in &o.tables {
                let fs: Vec<usize> = spec
                    .iter()
                    .map(|n| {
                        self.factors
                            .iter()
                            .position(|f| f.name.eq_ignore_ascii_case(n))
                            .ok_or_else(|| {
                                err(format!("OMEANS TABLES names {n}, which is not a factor"))
                            })
                    })
                    .collect::<Result<_, _>>()?;
                let key = |c: &Case| -> Vec<usize> { fs.iter().map(|&f| c.levels[f]).collect() };
                let mut keys: Vec<Vec<usize>> = self.cases.iter().map(key).collect();
                keys.sort();
                keys.dedup();
                let mut rows = Vec::new();
                for &v in &vars {
                    for k in &keys {
                        let cases: Vec<&Case> =
                            self.cases.iter().filter(|c| key(c) == *k).collect();
                        let cell_means: Vec<f64> = groups
                            .iter()
                            .filter(|g| key(g[0]) == *k)
                            .map(|g| moments(g, v).0)
                            .collect();
                        let mut row = vec![text(self.cont[v].clone())];
                        row.extend(
                            fs.iter().zip(k).map(|(&f, l)| {
                                text(format!("{}", self.factors[f].min + *l as f64))
                            }),
                        );
                        row.push(number(Some(moments(&cases, v).0)));
                        row.push(number(Some(
                            cell_means.iter().sum::<f64>() / cell_means.len() as f64,
                        )));
                        rows.push(row);
                    }
                }
                let mut columns = plain(&["Variable"]);
                columns.extend(
                    fs.iter()
                        .map(|&f| (String::new(), self.factors[f].name.clone())),
                );
                columns.extend(plain(&["Weighted Mean", "Unweighted Mean"]));
                let what = if spec.is_empty() {
                    "CONSTANT".to_string()
                } else {
                    spec.join(" BY ").to_uppercase()
                };
                out.push(table(
                    "Combined Observed Means",
                    1 + fs.len(),
                    columns,
                    rows,
                    Some(what),
                ));
            }
        }
        let [bartlett, cochran, boxm] = c.homogeneity;
        if bartlett || cochran {
            let mut rows = Vec::new();
            for v in 0..ndeps {
                let usable: Vec<(f64, f64)> = groups
                    .iter()
                    .map(|g| {
                        let (_, sd, n) = moments(g, v);
                        (sd * sd, n)
                    })
                    .filter(|(var, n)| *n > 1.0 && var.is_finite())
                    .collect();
                if cochran && usable.len() > 1 {
                    let k = usable.len() as f64;
                    let total: f64 = usable.iter().map(|u| u.0).sum();
                    let big = usable.iter().map(|u| u.0).fold(0.0, f64::max);
                    let cc = big / total;
                    let nu = usable.iter().map(|u| u.1).sum::<f64>() / k - 1.0;
                    let p = (k * f_p((k - 1.0) * cc / (1.0 - cc), nu, (k - 1.0) * nu)).min(1.0);
                    rows.push(vec![
                        text(self.cont[v].clone()),
                        text("Cochrans C"),
                        number(Some(cc)),
                        number(Some(nu)),
                        number(Some(k)),
                        number(Some(p)),
                    ]);
                }
                if bartlett && let Some((_, f, f1, f2)) = box_m_stats(&self.glm_data(&[v])) {
                    rows.push(vec![
                        text(self.cont[v].clone()),
                        text("Bartlett-Box F"),
                        number(Some(f)),
                        number(Some(f1)),
                        number(f2.is_finite().then_some(f2)),
                        number((f.is_finite() && f2.is_finite()).then(|| f_p(f, f1, f2))),
                    ]);
                }
            }
            out.push(table(
                "Univariate Homogeneity of Variance Tests",
                2,
                plain(&["Variable", "Test", "Value", "df1", "df2", "Sig."]),
                rows,
                Some(
                    "Cochrans C has df1 the mean cell size less 1 and df2 the number of cells"
                        .into(),
                ),
            ));
        }
        if boxm && ndeps > 1 {
            let deps: Vec<usize> = (0..ndeps).collect();
            if let Some((m, f, f1, f2)) = box_m_stats(&self.glm_data(&deps)) {
                let ok = f.is_finite() && f2.is_finite();
                out.push(table(
                    "Box's M Test of Homogeneity of Dispersion Matrices",
                    1,
                    plain(&["", "Value"]),
                    vec![
                        vec![text("Box's M"), number(Some(m))],
                        vec![text("F"), number(ok.then_some(f))],
                        vec![text("df1"), number(Some(f1))],
                        vec![text("df2"), number(ok.then_some(f2))],
                        vec![text("Sig."), number(ok.then(|| f_p(f, f1, f2)))],
                    ],
                    None,
                ));
            }
        }
        Ok(out)
    }
}
