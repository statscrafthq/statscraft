//! MATRIX=OUT and MATRIX=IN of MANOVA.
//!
//! The file: ROWTYPE_, the factors, VARNAME_ and the continuous variables (dependents, then
//! covariates). The records: N, the total number of cases, with no factor value; for every
//! non-empty cell N and MEAN with the cell's factor values; STDDEV, the square root of the
//! within-cells mean square of each variable, and CORR, the within-cells correlations, both with
//! no factor value. With no factors the one cell is the sample: N and MEAN stand alone (the
//! layout of a file CORRELATIONS writes, which MANOVA reads as well).
//!
//! Reading needs only the cell sizes and means and the pooled within-cells moments, which is all
//! the analyses use. The reader builds cases that have exactly those: in each cell the rows are
//! the cell mean plus deviations that sum to zero (weighted) and whose cross products are the
//! pooled ones. The analyses and their tables are then those of the data. What needs the
//! variances of the cells themselves (CELLINFO, HOMOGENEITY) or the cases (RESIDUALS) is an
//! error with IN.

use super::*;
use crate::matrix::cholesky;
use crate::matrix_file::MatrixRows;

/// A non-empty cell: its levels, its weighted size and its means.
struct CellStats {
    levels: Vec<usize>,
    n: f64,
    mean: Vec<f64>,
}

/// The non-empty cells in the order of their cell numbers, with the pooled within-cells sums of
/// squares and cross products and their degrees of freedom (cases less cells).
fn cells_and_pooled(s: &Setup) -> (Vec<CellStats>, Matrix, f64) {
    let k = s.cont.len();
    let mut cells: Vec<(usize, CellStats)> = Vec::new();
    for case in s.cases {
        let at = match cells.iter().position(|(c, _)| *c == case.cell) {
            Some(i) => i,
            None => {
                cells.push((
                    case.cell,
                    CellStats {
                        levels: case.levels.clone(),
                        n: 0.0,
                        mean: vec![0.0; k],
                    },
                ));
                cells.len() - 1
            }
        };
        let cell = &mut cells[at].1;
        cell.n += case.w;
        for (m, x) in cell.mean.iter_mut().zip(&case.v) {
            *m += case.w * x;
        }
    }
    cells.sort_by_key(|(c, _)| *c);
    let mut cells: Vec<CellStats> = cells.into_iter().map(|(_, c)| c).collect();
    for cell in &mut cells {
        cell.mean.iter_mut().for_each(|m| *m /= cell.n);
    }
    let mut w = vec![vec![0.0; k]; k];
    for case in s.cases {
        let cell = cells
            .iter()
            .find(|c| c.levels == case.levels)
            .expect("every case is in a cell");
        let d: Vec<f64> = case.v.iter().zip(&cell.mean).map(|(x, m)| x - m).collect();
        for i in 0..k {
            for j in 0..k {
                w[i][j] += case.w * d[i] * d[j];
            }
        }
    }
    let df = s.cases.iter().map(|c| c.w).sum::<f64>() - cells.len() as f64;
    (cells, w, df)
}

impl Engine {
    /// The matrix file MATRIX=OUT asks for.
    pub(super) fn manova_matrix_out(
        &self,
        s: &Setup,
        labels: Vec<Option<String>>,
    ) -> Result<Dataset, Error> {
        let (cells, w, df) = cells_and_pooled(s);
        if df <= 0.0 {
            return Err(err(
                "MATRIX=OUT: no case shares its cell with another, so there is no within-cells variance to write",
            ));
        }
        let k = s.cont.len();
        let mut rows = MatrixRows::new(s.cont.to_vec(), labels);
        let mut factor_values: Vec<Vec<Option<f64>>> = vec![Vec::new(); s.factors.len()];
        let mut push = |rows: &mut MatrixRows,
                        kind: &str,
                        varname: &str,
                        v: Vec<f64>,
                        at: Option<&[usize]>| {
            rows.push(kind, varname, v);
            for (f, column) in factor_values.iter_mut().enumerate() {
                column.push(at.map(|l| s.factors[f].min + l[f] as f64));
            }
        };
        push(&mut rows, "N", "", vec![s.n; k], None);
        if s.factors.is_empty() {
            push(&mut rows, "MEAN", "", cells[0].mean.clone(), None);
        } else {
            for cell in &cells {
                push(&mut rows, "N", "", vec![cell.n; k], Some(&cell.levels));
                push(&mut rows, "MEAN", "", cell.mean.clone(), Some(&cell.levels));
            }
        }
        let sd: Vec<f64> = (0..k).map(|j| (w[j][j] / df).sqrt()).collect();
        push(&mut rows, "STDDEV", "", sd.clone(), None);
        for (i, name) in s.cont.iter().enumerate() {
            let row = (0..k)
                .map(|j| match i == j && sd[i] > 0.0 {
                    true => 1.0,
                    false => w[i][j] / (w[i][i] * w[j][j]).sqrt(),
                })
                .collect();
            push(&mut rows, "CORR", name, row, None);
        }
        rows.factors = s
            .factors
            .iter()
            .zip(factor_values)
            .map(|(f, v)| (f.name.clone(), v))
            .collect();
        self.matrix_part("MANOVA", &rows)
    }

    /// The analysis of a matrix file: the cell sizes and means and the pooled within-cells
    /// moments of its records, as cases that have exactly those.
    pub(super) fn manova_from_matrix(
        &mut self,
        c: &Manova,
        source: Option<&String>,
        matrix_part: &mut Option<Dataset>,
    ) -> Result<Vec<Table>, Error> {
        if c.cellinfo.iter().any(|&b| b) || c.homogeneity.iter().any(|&b| b) {
            return Err(err(
                "MATRIX=IN holds the means and the pooled variances of the cells, not their own variances, so CELLINFO and HOMOGENEITY are not available",
            ));
        }
        if c.designs.iter().any(|d| d.residuals) {
            return Err(err("MATRIX=IN has no cases, so RESIDUALS is not available"));
        }
        let m = self.matrix_materials("MANOVA", source)?;
        let groupings = m.grouping_names();
        let deps = self.expand_among(&c.dependents, &m.all)?;
        let mut covs = self.expand_among(&c.covariates, &m.all)?;
        let nvary = covs.len();
        covs.extend(self.expand_among(&c.constant_covariates, &m.all)?);
        let mut factors = Vec::new();
        for g in &c.factors {
            for name in self.expand_among(&g.names, &groupings)? {
                if !groupings.iter().any(|v| v.eq_ignore_ascii_case(&name)) {
                    return Err(err(format!(
                        "MANOVA names the factor {name}, which is not between ROWTYPE_ and VARNAME_ in the matrix file"
                    )));
                }
                factors.push(Factor {
                    name,
                    min: g.min,
                    k: (g.max - g.min) as usize + 1,
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
        if let Some(n) = cont
            .iter()
            .find(|n| !m.all.iter().any(|v| v.eq_ignore_ascii_case(n)))
        {
            return Err(err(format!(
                "the matrix file has no variable {n} among its columns after VARNAME_"
            )));
        }
        let k = cont.len();
        // the factors that are in the file but not in the command would mix cells
        if let Some(extra) = groupings
            .iter()
            .find(|g| !factors.iter().any(|f| f.name.eq_ignore_ascii_case(g)))
        {
            return Err(err(format!(
                "the matrix file has the variable {extra} between ROWTYPE_ and VARNAME_, which MANOVA does not name as a factor"
            )));
        }
        let names: Vec<String> = factors.iter().map(|f| f.name.clone()).collect();
        // the cells: N and MEAN records with the same, whole and in-range factor values
        let in_range = |values: &[Option<f64>]| -> Option<Vec<usize>> {
            values
                .iter()
                .zip(&factors)
                .map(|(v, f)| {
                    let l = (*v)? - f.min;
                    (l >= 0.0 && l.fract() == 0.0 && l < f.k as f64).then_some(l as usize)
                })
                .collect()
        };
        let counts = m.records("N", &names, &cont);
        let means = m.records("MEAN", &names, &cont);
        let mut cells: Vec<(Vec<usize>, f64, Vec<f64>)> = Vec::new();
        if factors.is_empty() {
            let (Some(n), Some(mean)) = (counts.first(), means.first()) else {
                return Err(err("the matrix file has no N or no MEAN record"));
            };
            cells.push((Vec::new(), n.1[0], mean.1.clone()));
        } else {
            for (at, n) in &counts {
                let Some(levels) = in_range(at) else { continue };
                let Some(mean) = means.iter().find(|(a, _)| a == at) else {
                    return Err(err(format!(
                        "the matrix file has an N record for the cell {levels:?} but no MEAN record for it"
                    )));
                };
                cells.push((levels, n[0], mean.1.clone()));
            }
        }
        if cells.iter().any(|c| !c.1.is_finite()) {
            return Err(err("an N record of the matrix file has a missing count"));
        }
        cells.retain(|c| c.1 > 0.0);
        if cells.is_empty() {
            return Err(err(
                "the matrix file has no cell of the factor ranges with cases",
            ));
        }
        if cells.iter().any(|c| c.2.iter().any(|v| !v.is_finite())) {
            return Err(err("a MEAN record of the matrix file has a missing value"));
        }
        let pooled = |kind: &str| -> Option<Vec<f64>> {
            m.records(kind, &names, &cont)
                .into_iter()
                .find(|(a, _)| a.iter().all(Option::is_none))
                .map(|(_, v)| v)
        };
        let Some(sd) = pooled("STDDEV") else {
            return Err(err("the matrix file has no STDDEV record"));
        };
        let corr = m
            .matrix("CORR", &cont)
            .map_err(|e| err(format!("MATRIX=IN: {e}")))?
            .ok_or_else(|| err("the matrix file has no CORR records"))?;
        if sd.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(err(
                "a STDDEV of the matrix file is missing or not above 0, so there is no within-cells variance to analyze",
            ));
        }
        let total: f64 = cells.iter().map(|c| c.1).sum();
        let df = total - cells.len() as f64;
        if df < k as f64 {
            return Err(err(format!(
                "the matrix file leaves {df} degrees of freedom within cells for {k} variables"
            )));
        }
        let sscp: Matrix = (0..k)
            .map(|i| (0..k).map(|j| df * sd[i] * sd[j] * corr[i][j]).collect())
            .collect();
        let l = cholesky(&sscp)
            .ok_or_else(|| err("the correlations of the matrix file are not positive definite"))?;
        // rows: m_c = ceil(n_c) of weight n_c / m_c in a cell; the weighted orthonormal
        // within-cell contrasts (Helmert, divided by the root of the weight), each in its cell
        // only, spread over the cells by taking every (M / k)-th of all of them
        let sizes: Vec<usize> = cells
            .iter()
            .map(|c| (c.1 - 1e-9).ceil().max(1.0) as usize)
            .collect();
        let contrasts: usize = sizes.iter().map(|s| s - 1).sum();
        if contrasts < k {
            return Err(err(format!(
                "the cells of the matrix file have {contrasts} within-cell contrasts for {k} variables"
            )));
        }
        let mut basis: Vec<(usize, usize)> = Vec::new();
        for (cell, &s) in sizes.iter().enumerate() {
            basis.extend((1..s).map(|j| (cell, j)));
        }
        let picked: Vec<(usize, usize)> = (0..k).map(|i| basis[i * contrasts / k]).collect();
        let mut cases = Vec::new();
        for (cell_at, ((levels, n, mean), &rows)) in cells.iter().zip(&sizes).enumerate() {
            let w = n / rows as f64;
            let cell_number = levels.iter().zip(&factors).fold(0, |a, (l, f)| a * f.k + l);
            for r in 0..rows {
                // Helmert contrast j of `rows`: 1 on the first j rows, -j on row j, scaled
                let u: Vec<f64> = picked
                    .iter()
                    .map(|&(cell, j)| {
                        if cell != cell_at {
                            return 0.0;
                        }
                        let scale = 1.0 / ((j * (j + 1)) as f64).sqrt() / w.sqrt();
                        match r.cmp(&j) {
                            std::cmp::Ordering::Less => scale,
                            std::cmp::Ordering::Equal => -(j as f64) * scale,
                            std::cmp::Ordering::Greater => 0.0,
                        }
                    })
                    .collect();
                let v: Vec<f64> = (0..k)
                    .map(|a| mean[a] + (0..k).map(|b| l[a][b] * u[b]).sum::<f64>())
                    .collect();
                cases.push(Case {
                    index: cases.len(),
                    levels: levels.clone(),
                    cell: cell_number,
                    v,
                    w,
                });
            }
        }
        let n: f64 = cases.iter().map(|c| c.w).sum();
        let tables = vec![table(
            "Cases",
            1,
            plain(&["", "N"]),
            vec![
                vec![text("Accepted"), number(Some(n))],
                vec![text("Non-empty cells"), number(Some(cells.len() as f64))],
            ],
            None,
        )];
        let setup = Setup {
            factors: &factors,
            cont: &cont,
            cases: &cases,
            n,
        };
        if c.matrix.output.is_some() {
            *matrix_part = Some(self.manova_matrix_out(&setup, vec![None; cont.len()])?);
        }
        self.manova_designs(c, &setup, tables, deps.len(), nvary)
    }
}
