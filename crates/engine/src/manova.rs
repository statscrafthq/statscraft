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
use crate::dist::{f_p, t_p, t_quantile};
use crate::glm::{Data, box_m_stats, multivariate, relative_eigenvalues, sweep_in_order};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Values;
use statscraft_syntax::{
    Contrast, Manova, ManovaDesign, ManovaEffect, ManovaError, ManovaItem, VarItem,
};

const EPS: f64 = 1e-8;
const MAX_CELLS: usize = 100_000;

struct Factor {
    name: String,
    min: f64,
    k: usize,
}

struct Case {
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

impl Engine {
    pub(crate) fn manova(&mut self, c: &Manova) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let deps = self.expand(&c.dependents)?;
        let covs = self.expand(&c.covariates)?;
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
            tables.extend(self.manova_design(&setup, d, deps.len(), &caption)?);
        }
        Ok(tables)
    }

    fn manova_design(
        &self,
        s: &Setup,
        d: &ManovaDesign,
        ndeps: usize,
        caption: &str,
    ) -> Result<Vec<Table>, Error> {
        // the continuous variables of this analysis: dependents, then covariates
        let (dv, cv): (Vec<usize>, Vec<usize>) = match &d.analysis {
            None => ((0..ndeps).collect(), (ndeps..s.cont.len()).collect()),
            Some((a, b)) => {
                let pick = |items: &[VarItem]| -> Result<Vec<usize>, Error> {
                    self.expand(items)?
                        .iter()
                        .map(|n| {
                            find(s.cont, n).ok_or_else(|| {
                                err(format!(
                                    "ANALYSIS names {n}, which is not a dependent variable or covariate of MANOVA"
                                ))
                            })
                        })
                        .collect()
                };
                (pick(a)?, pick(b)?)
            }
        };
        let used: Vec<usize> = dv.iter().chain(&cv).copied().collect();
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
        for (name, _) in &d.contrasts {
            if find(
                &s.factors.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
                name,
            )
            .is_none()
            {
                return Err(err(format!("CONTRAST names {name}, which is not a factor")));
            }
        }
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
            .any(|t| t.muplus || t.effects.iter().any(|e| e.crossed.is_empty()));
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
            let is_constant = !t.muplus && t.effects.iter().all(|e| e.crossed.is_empty());
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
        // every shown term with its test (None when its error term does not exist)
        let mut tests: Vec<(usize, ManovaError, Option<Test>)> = Vec::new();
        for (i, t) in terms.iter().enumerate() {
            if !t.shown {
                continue;
            }
            let e = t.against.unwrap_or(default);
            let test = match error_of(e)? {
                Some((se, dfe))
                    if t.define.is_none() || e != ManovaError::Number(t.define.unwrap_or(0)) =>
                {
                    Some(Test::new(&hyp[i].0, hyp[i].1, &se, dfe, p))
                }
                _ => None,
            };
            tests.push((i, e, test));
        }
        let default_error = error_of(default)?;
        let regression = default_error.as_ref().filter(|_| q > 0).map(|(se, dfe)| {
            let (ea, qq) = adjust(se, p);
            Test {
                h: sub(&block(se, 0..p), &ea),
                e: ea,
                dfh: qq as f64,
                dfe: dfe - qq as f64,
            }
        });
        let mut out = Vec::new();
        let label = |i: usize| terms[i].label.clone();
        if p == 1 {
            out.push(self.univariate_table(
                &terms,
                &tests,
                &regression,
                default,
                &zwz,
                nx,
                &x,
                &s_wr,
                rank,
                s.n,
                &cap(&format!("{} using {method} sums of squares", names[0])),
            ));
        } else {
            let mut groups: Vec<(String, &Test)> = Vec::new();
            if let Some(r) = &regression {
                groups.push((format!("{} Regression", error_name(default)), r));
            }
            for (i, e, t) in &tests {
                if let Some(t) = t {
                    let suffix = if *e == default {
                        String::new()
                    } else {
                        format!(" (vs {})", error_name(*e))
                    };
                    groups.push((format!("{}{suffix}", label(*i)), t));
                }
            }
            out.extend(multivariate_tables(&groups, &names, &d.print, &cap(method)));
        }
        if let (Some((se, dfe)), true) = (&default_error, q > 0) {
            let covs: Vec<String> = cv.iter().map(|&j| s.cont[j].clone()).collect();
            out.push(coefficients(
                se,
                *dfe,
                &names,
                &covs,
                &cap(&error_name(default)),
            ));
        }
        if let Some((se, dfe)) = &default_error {
            let (ea, qq) = adjust(se, p);
            let dfe = dfe - qq as f64;
            let pr = &d.print;
            let what = cap(&error_name(default));
            if pr.error_sscp {
                out.push(matrix_table("Error SSCP Matrix", &names, &ea, what.clone()));
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
                let cor: Matrix = (0..p)
                    .map(|i| {
                        (0..p)
                            .map(|j| {
                                if i == j {
                                    cov[i][i].sqrt()
                                } else {
                                    cov[i][j] / (cov[i][i] * cov[j][j]).sqrt()
                                }
                            })
                            .collect()
                    })
                    .collect();
                out.push(matrix_table(
                    "Error Correlation Matrix",
                    &names,
                    &cor,
                    format!("{what}; standard deviations on the diagonal"),
                ));
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
            if d.print.parameters || d.pmeans {
                let (full, swept) =
                    sweep_in_order(&zwz, &(0..nx).chain(nx + p..dim).collect::<Vec<_>>(), EPS);
                if d.print.parameters {
                    out.push(parameters(
                        &terms,
                        &tests,
                        (&ea, dfe),
                        &full,
                        &swept,
                        nx,
                        &names,
                        d.print.negsum,
                        &cap(method),
                    ));
                }
                if d.pmeans {
                    if continuous {
                        return Err(err(
                            "PMEANS is not available when DESIGN names a continuous variable",
                        ));
                    }
                    let (design_only, _) = sweep_in_order(&zwz, &all, EPS);
                    out.push(predicted_means(
                        s,
                        &x,
                        &full,
                        &design_only,
                        nx,
                        &dv,
                        &names,
                        &cov,
                        &cap(&error_name(default)),
                    ));
                }
            }
        }
        if d.print.hypoth && p > 1 {
            for (i, _, t) in &tests {
                if let Some(t) = t {
                    out.push(matrix_table(
                        "Hypothesis SSCP Matrix",
                        &names,
                        &t.h,
                        cap(&label(*i)),
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
        tests: &[(usize, ManovaError, Option<Test>)],
        regression: &Option<Test>,
        default: ManovaError,
        zwz: &Matrix,
        nx: usize,
        _x: &[Vec<f64>],
        s_wr: &Matrix,
        rank: usize,
        n: f64,
        caption: &str,
    ) -> Table {
        let mut rows = Vec::new();
        let row = |name: String, ss: f64, df: f64, f: Option<(f64, f64)>| -> Vec<Cell> {
            vec![
                text(name),
                number(Some(ss)),
                number(Some(df)),
                number((df > 0.0).then(|| ss / df)),
                number(f.map(|f| f.0)),
                number(f.map(|f| f.1)),
            ]
        };
        let mut errors: Vec<ManovaError> = vec![default];
        for (_, e, _) in tests {
            if !errors.contains(e) {
                errors.push(*e);
            }
        }
        for e in errors {
            let group: Vec<_> = tests
                .iter()
                .filter(|(_, ee, t)| *ee == e && t.is_some())
                .collect();
            let Some(first) = group
                .first()
                .and_then(|g| g.2.as_ref())
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
                rows.push(row(
                    "REGRESSION".into(),
                    r.h[0][0],
                    r.dfh,
                    r.ok().then(|| r.univariate(0)),
                ));
            }
            for (i, _, t) in group {
                let t = t.as_ref().expect("a test");
                rows.push(row(
                    terms[*i].label.clone(),
                    t.h[0][0],
                    t.dfh,
                    t.ok().then(|| t.univariate(0)),
                ));
            }
        }
        for (i, _, t) in tests {
            if t.is_none() {
                rows.push(row(
                    format!("{} (no error term)", terms[*i].label),
                    f64::NAN,
                    f64::NAN,
                    None,
                ));
            }
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
        let mut t = table(
            "Tests of Significance",
            1,
            plain(&["Source of Variation", "SS", "DF", "MS", "F", "Sig of F"]),
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

fn multivariate_tables(
    groups: &[(String, &Test)],
    names: &[String],
    pr: &statscraft_syntax::ManovaPrint,
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
        for (k, (_, value, f, d1, d2, _)) in stats.into_iter().enumerate() {
            let roy = k == 3;
            let ok = t.ok() && d1 > 0.0 && d2 > 0.0 && f.is_finite() && !roy;
            let value = if roy {
                l.first().map_or(0.0, |x| x / (1.0 + x))
            } else {
                value
            };
            mv.push(vec![
                text(if k == 0 { head.clone() } else { String::new() }),
                text(["Pillais", "Wilks", "Hotellings", "Roys"][k]),
                number(t.ok().then_some(value)),
                number(ok.then_some(f)),
                number(ok.then_some(d1)),
                number(ok.then_some(d2)),
                number(ok.then(|| f_p(f, d1, d2))),
            ]);
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
            uni.push(vec![
                text(if j == 0 { label.clone() } else { String::new() }),
                text(nm.clone()),
                number(Some(t.h[j][j])),
                number(Some(t.e[j][j])),
                number(Some(t.h[j][j] / t.dfh)),
                number(Some(t.e[j][j] / t.dfe)),
                number(t.ok().then_some(f)),
                number(t.ok().then_some(sig)),
            ]);
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
    if pr.multiv {
        out.push(table(
            "Multivariate Tests of Significance",
            2,
            plain(&[
                "Effect",
                "Test Name",
                "Value",
                "Approx. F",
                "Hypoth. DF",
                "Error DF",
                "Sig. of F",
            ]),
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
        out.push(table(
            "Univariate F-tests",
            2,
            plain(&[
                "Effect",
                "Variable",
                "Hypoth. SS",
                "Error SS",
                "Hypoth. MS",
                "Error MS",
                "F",
                "Sig. of F",
            ]),
            uni,
            note.clone(),
        ));
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

/// t, its two-sided significance and the 95% interval of an estimate.
fn t_cells(b: f64, se: f64, df: f64) -> Vec<Cell> {
    let ok = se > 0.0 && df > 0.0;
    let t = b / se;
    let half = if ok {
        t_quantile(0.025, df) * se
    } else {
        f64::NAN
    };
    vec![
        number(Some(b)),
        number(ok.then_some(se)),
        number(ok.then_some(t)),
        number(ok.then(|| t_p(t, df))),
        number(Some(b - half)),
        number(Some(b + half)),
    ]
}

const ESTIMATE_COLUMNS: [&str; 5] = ["Std. Err.", "t-Value", "Sig. t", "Lower -95%", "CL- Upper"];

/// The within-error regression of the dependents on the covariates. `se` is the error SSCP of
/// dependents and covariates.
fn coefficients(se: &Matrix, dfe: f64, names: &[String], covs: &[String], caption: &str) -> Table {
    let p = names.len();
    let (m, swept) = sweep_in_order(se, &(p..se.len()).collect::<Vec<_>>(), EPS);
    let dfe = dfe - swept.iter().filter(|&&s| s).count() as f64;
    let mut rows = Vec::new();
    for (j, name) in names.iter().enumerate() {
        for (k, cov) in covs.iter().enumerate() {
            let c = p + k;
            let mut row = vec![text(name.clone()), text(cov.clone())];
            if swept[c] {
                row.extend(t_cells(m[j][c], (m[j][j] / dfe * m[c][c]).sqrt(), dfe));
            }
            rows.push(row);
        }
    }
    let mut columns = plain(&["Dependent", "Covariate", "B"]);
    columns.extend(plain(&ESTIMATE_COLUMNS));
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
    tests: &[(usize, ManovaError, Option<Test>)],
    fallback: (&Matrix, f64),
    full: &Matrix,
    swept: &[bool],
    nx: usize,
    names: &[String],
    negsum: bool,
    caption: &str,
) -> Table {
    let p = names.len();
    let mut rows = Vec::new();
    for (j, name) in names.iter().enumerate() {
        for (i, term) in terms.iter().enumerate() {
            let (e, dfe) = tests
                .iter()
                .find(|t| t.0 == i)
                .and_then(|t| t.2.as_ref())
                .map_or(fallback, |t| (&t.e, t.dfe));
            let mse = e[j][j] / dfe;
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
                    row.extend(t_cells(full[nx + j][c], (mse * full[c][c]).sqrt(), dfe));
                }
                rows.push(row);
            }
            if negsum && !term.constant && term.cols.clone().all(|c| swept[c]) {
                let b: f64 = -term.cols.clone().map(|c| full[nx + j][c]).sum::<f64>();
                let v: f64 = term
                    .cols
                    .clone()
                    .flat_map(|a| term.cols.clone().map(move |c| (a, c)))
                    .map(|(a, c)| full[a][c])
                    .sum();
                let mut row = head("NEGSUM".into());
                row.extend(t_cells(b, (mse * v).sqrt(), dfe));
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
    columns.extend(plain(&ESTIMATE_COLUMNS));
    let heads = if p > 1 { 3 } else { 2 };
    table(
        "Parameter Estimates",
        heads,
        columns,
        rows,
        Some(caption.to_string()),
    )
}

/// Observed and predicted cell means of each dependent with the residuals.
#[allow(clippy::too_many_arguments)]
fn predicted_means(
    s: &Setup,
    x: &[Vec<f64>],
    full: &Matrix,
    design_only: &Matrix,
    nx: usize,
    dv: &[usize],
    names: &[String],
    cov: &Matrix,
    caption: &str,
) -> Table {
    let mut rows = Vec::new();
    for (j, (&v, name)) in dv.iter().zip(names).enumerate() {
        let mut cells: Vec<usize> = s.cases.iter().map(|c| c.cell).collect();
        cells.sort_unstable();
        cells.dedup();
        for cell in cells {
            let members: Vec<usize> = (0..s.cases.len())
                .filter(|&i| s.cases[i].cell == cell)
                .collect();
            let w: f64 = members.iter().map(|&i| s.cases[i].w).sum();
            let obs = members
                .iter()
                .map(|&i| s.cases[i].w * s.cases[i].v[v])
                .sum::<f64>()
                / w;
            let i = members[0];
            let fit = |m: &Matrix| (0..nx).map(|c| x[c][i] * m[nx + j][c]).sum::<f64>();
            let (adj, est) = (fit(full), fit(design_only));
            let mut row = vec![text(name.clone())];
            row.extend(levels_text(s.factors, &s.cases[i].levels));
            row.extend(
                [obs, adj, est, obs - est, (obs - est) / cov[j][j].sqrt()].map(|v| number(Some(v))),
            );
            rows.push(row);
        }
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
    table(
        "Observed and Predicted Means",
        1 + s.factors.len(),
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
