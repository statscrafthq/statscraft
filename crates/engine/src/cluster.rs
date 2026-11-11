//! CLUSTER: hierarchical clustering of cases.
//!
//! Rules from the Command Syntax Reference (CLUSTER) and the Algorithms manual (CLUSTER and the
//! proximity measures of PROXIMITIES):
//! - A case missing on any clustering variable is left out (INCLUDE keeps user-missing values).
//!   The weight variable is not used. With MATRIX=IN the items are the rows of type PROX of a
//!   matrix file, a dissimilarity matrix unless the value label of PROX says SIMILARITY.
//! - Every step joins the most similar pair of clusters q < p into q (smallest dissimilarity or
//!   largest similarity) and updates the proximities of the new cluster by the formulas of the
//!   method. BAVERAGE chooses by the mean proximity between the two clusters, WAVERAGE by the
//!   mean proximity of all pairs within the joined cluster. The coefficient of the schedule is
//!   the value the pair was chosen by; for WARD it is the running within-cluster sum of squares,
//!   W + s_pq / 2 at each step.
//! - The clusters of a solution are numbered 1, 2, ... in the order of their first item.

use crate::descriptives::number;
use crate::matrix::Matrix;
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Value, Values, Variable};
use statscraft_syntax::{BinaryMeasure, Cluster, Linkage, Proximity, VarItem};

/// The items to cluster with their proximities.
struct Items {
    labels: Vec<String>,
    /// The case of every item in the active dataset; empty with matrix input.
    cases: Vec<usize>,
    prox: Matrix,
    similarity: bool,
    /// Cases left out for missing values.
    missing: usize,
}

/// One step of the agglomeration: clusters q < p joined (0-based) at `coefficient`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stage {
    pub q: usize,
    pub p: usize,
    pub coefficient: f64,
}

pub(crate) fn measure_name(m: &Proximity) -> String {
    use BinaryMeasure as B;
    match m {
        Proximity::SEuclid => "Squared Euclidean Distance".into(),
        Proximity::Euclid => "Euclidean Distance".into(),
        Proximity::Cosine => "Cosine of Vectors of Values".into(),
        Proximity::Correlation => "Correlation between Vectors of Values".into(),
        Proximity::Chebychev => "Chebychev Distance".into(),
        Proximity::Block => "Block Distance".into(),
        Proximity::Minkowski(p) => format!("Minkowski({p}) Distance"),
        Proximity::Power(p, r) => format!("Power({p},{r}) Distance"),
        Proximity::Chisq => "Chi-square between Sets of Frequencies".into(),
        Proximity::Ph2 => "Phi-square between Sets of Frequencies".into(),
        Proximity::Binary(b, _, _) => match b {
            B::Rr => "Russell and Rao Measure",
            B::Sm => "Simple Matching Measure",
            B::Jaccard => "Jaccard Measure",
            B::Dice => "Dice Measure",
            B::Ss1 => "Sokal and Sneath Measure 1",
            B::Rt => "Rogers and Tanimoto Measure",
            B::Ss2 => "Sokal and Sneath Measure 2",
            B::K1 => "Kulczynski Measure 1",
            B::Ss3 => "Sokal and Sneath Measure 3",
            B::K2 => "Kulczynski Measure 2",
            B::Ss4 => "Sokal and Sneath Measure 4",
            B::Hamann => "Hamann Measure",
            B::Ochiai => "Ochiai Measure",
            B::Ss5 => "Sokal and Sneath Measure 5",
            B::Phi => "Phi 4-Point Correlation",
            B::Lambda => "Lambda",
            B::D => "Anderberg's D",
            B::Y => "Yule's Y",
            B::Q => "Yule's Q",
            B::Beuclid => "Binary Euclidean Distance",
            B::Size => "Size Difference",
            B::Pattern => "Pattern Difference",
            B::Bseuclid => "Binary Squared Euclidean Distance",
            B::Bshape => "Binary Shape Difference",
            B::Disper => "Dispersion Similarity",
            B::Variance => "Binary Variance",
            B::Blwmn => "Binary Lance and Williams Nonmetric",
        }
        .into(),
    }
}

pub(crate) fn is_similarity(m: &Proximity) -> bool {
    use BinaryMeasure as B;
    match m {
        Proximity::Cosine | Proximity::Correlation => true,
        Proximity::Binary(b, _, _) => !matches!(
            b,
            B::Beuclid | B::Size | B::Pattern | B::Bseuclid | B::Bshape | B::Variance | B::Blwmn
        ),
        _ => false,
    }
}

/// The binary measure from the 2 x 2 table; K1 and SS3 are limited to 9999.999 as the
/// manual says, the others are NaN where undefined.
fn binary(m: BinaryMeasure, a: f64, b: f64, c: f64, d: f64) -> f64 {
    use BinaryMeasure as B;
    let n = a + b + c + d;
    let limited = |v: f64| {
        if v.is_finite() {
            v.min(9999.999)
        } else {
            9999.999
        }
    };
    let t1 = a.max(b) + c.max(d) + a.max(c) + b.max(d);
    let t2 = (a + c).max(b + d) + (a + b).max(c + d);
    let margins = ((a + b) * (a + c) * (b + d) * (c + d)).sqrt();
    match m {
        B::Rr => a / n,
        B::Sm => (a + d) / n,
        B::Jaccard => a / (a + b + c),
        B::Dice => 2.0 * a / (2.0 * a + b + c),
        B::Ss1 => 2.0 * (a + d) / (2.0 * (a + d) + b + c),
        B::Rt => (a + d) / (a + d + 2.0 * (b + c)),
        B::Ss2 => a / (a + 2.0 * (b + c)),
        B::K1 => limited(a / (b + c)),
        B::Ss3 => limited((a + d) / (b + c)),
        B::K2 => (a / (a + b) + a / (a + c)) / 2.0,
        B::Ss4 => (a / (a + b) + a / (a + c) + d / (b + d) + d / (c + d)) / 4.0,
        B::Hamann => ((a + d) - (b + c)) / n,
        B::Lambda => (t1 - t2) / (2.0 * n - t2),
        B::D => (t1 - t2) / (2.0 * n),
        B::Y => {
            let (x, y) = ((a * d).sqrt(), (b * c).sqrt());
            (x - y) / (x + y)
        }
        B::Q => (a * d - b * c) / (a * d + b * c),
        B::Ochiai => (a / (a + b) * (a / (a + c))).sqrt(),
        B::Ss5 => a * d / margins,
        B::Phi => (a * d - b * c) / margins,
        B::Beuclid => (b + c).sqrt(),
        B::Bseuclid => b + c,
        B::Size => (b - c).powi(2) / (n * n),
        B::Pattern => b * c / (n * n),
        B::Bshape => (n * (b + c) - (b - c).powi(2)) / (n * n),
        B::Disper => (a * d - b * c) / (n * n),
        B::Variance => (b + c) / (4.0 * n),
        B::Blwmn => (b + c) / (2.0 * a + b + c),
    }
}

/// The proximity of two items over their values; NaN where the measure is undefined.
pub(crate) fn proximity(m: &Proximity, x: &[f64], y: &[f64]) -> f64 {
    let pairs = x.iter().zip(y);
    match *m {
        Proximity::SEuclid => pairs.map(|(a, b)| (a - b).powi(2)).sum(),
        Proximity::Euclid => pairs.map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt(),
        Proximity::Block => pairs.map(|(a, b)| (a - b).abs()).sum(),
        Proximity::Chebychev => pairs.map(|(a, b)| (a - b).abs()).fold(0.0, f64::max),
        Proximity::Minkowski(p) => pairs
            .map(|(a, b)| (a - b).abs().powf(p))
            .sum::<f64>()
            .powf(1.0 / p),
        Proximity::Power(p, r) => pairs
            .map(|(a, b)| (a - b).abs().powf(p))
            .sum::<f64>()
            .powf(1.0 / r),
        Proximity::Cosine => {
            let xy: f64 = pairs.map(|(a, b)| a * b).sum();
            let xx: f64 = x.iter().map(|a| a * a).sum();
            let yy: f64 = y.iter().map(|b| b * b).sum();
            xy / (xx * yy).sqrt()
        }
        Proximity::Correlation => {
            let n = x.len() as f64;
            let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
            let sxy: f64 = pairs.map(|(a, b)| (a - mx) * (b - my)).sum();
            let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
            let syy: f64 = y.iter().map(|b| (b - my).powi(2)).sum();
            sxy / (sxx * syy).sqrt()
        }
        Proximity::Chisq | Proximity::Ph2 => {
            let (sx, sy): (f64, f64) = (x.iter().sum(), y.iter().sum());
            let total = sx + sy;
            let chi: f64 = pairs
                .filter(|(a, b)| *a + *b != 0.0)
                .map(|(a, b)| {
                    let (ex, ey) = (sx * (a + b) / total, sy * (a + b) / total);
                    (a - ex).powi(2) / ex + (b - ey).powi(2) / ey
                })
                .sum::<f64>()
                .sqrt();
            match m {
                Proximity::Ph2 => chi / total.sqrt(),
                _ => chi,
            }
        }
        Proximity::Binary(kind, present, absent) => {
            let state = |v: f64| {
                if v == present {
                    Some(true)
                } else if absent.is_none_or(|a| v == a) {
                    Some(false)
                } else {
                    None
                }
            };
            let (mut a, mut b, mut c, mut d) = (0.0, 0.0, 0.0, 0.0);
            for (u, v) in pairs {
                match (state(*u), state(*v)) {
                    (Some(true), Some(true)) => a += 1.0,
                    (Some(true), Some(false)) => b += 1.0,
                    (Some(false), Some(true)) => c += 1.0,
                    (Some(false), Some(false)) => d += 1.0,
                    _ => {}
                }
            }
            binary(kind, a, b, c, d)
        }
    }
}

/// The agglomeration schedule of the method on the proximities `s`.
pub(crate) fn agglomerate(s: &Matrix, similarity: bool, method: Linkage) -> Vec<Stage> {
    let n = s.len();
    let mut s = s.clone();
    let mut size = vec![1.0; n];
    let mut within = vec![0.0; n];
    let mut active = vec![true; n];
    let mut ward = 0.0;
    let mut stages = Vec::with_capacity(n.saturating_sub(1));
    let better = |c: f64, best: f64| {
        let margin = 1e-10 * best.abs().max(1.0);
        match similarity {
            true => c > best + margin,
            false => c < best - margin,
        }
    };
    for _ in 1..n {
        let mut best: Option<(usize, usize, f64)> = None;
        for i in (0..n).filter(|&i| active[i]) {
            for j in (i + 1..n).filter(|&j| active[j]) {
                let c = match method {
                    Linkage::BetweenGroups => s[i][j] / (size[i] * size[j]),
                    Linkage::WithinGroups => {
                        let t = size[i] + size[j];
                        (within[i] + within[j] + s[i][j]) / (t * (t - 1.0) / 2.0)
                    }
                    _ => s[i][j],
                };
                if best.is_none_or(|(_, _, b)| better(c, b)) {
                    best = Some((i, j, c));
                }
            }
        }
        let Some((q, p, chosen)) = best else { break };
        let spq = s[q][p];
        let (np, nq) = (size[p], size[q]);
        let coefficient = match method {
            Linkage::Ward => {
                ward += 0.5 * spq;
                ward
            }
            _ => chosen,
        };
        stages.push(Stage { q, p, coefficient });
        for r in (0..n).filter(|&r| active[r] && r != p && r != q) {
            let (spr, sqr, nr) = (s[p][r], s[q][r], size[r]);
            let pick = |a: f64, b: f64, low: bool| if low { a.min(b) } else { a.max(b) };
            let t = match method {
                Linkage::BetweenGroups | Linkage::WithinGroups => spr + sqr,
                Linkage::Single => pick(spr, sqr, !similarity),
                Linkage::Complete => pick(spr, sqr, similarity),
                Linkage::Centroid => {
                    let m = np + nq;
                    np / m * spr + nq / m * sqr - np * nq / (m * m) * spq
                }
                Linkage::Median => (spr + sqr) / 2.0 - spq / 4.0,
                Linkage::Ward => ((nr + np) * spr + (nr + nq) * sqr - nr * spq) / (np + nq + nr),
            };
            s[q][r] = t;
            s[r][q] = t;
        }
        within[q] += within[p] + spq;
        size[q] += np;
        active[p] = false;
    }
    stages
}

/// The cluster of every item in the solution with `k` clusters, numbered from 1 in the order
/// of the first item of each cluster.
pub(crate) fn membership(n: usize, stages: &[Stage], k: usize) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    for stage in stages.iter().take(n.saturating_sub(k)) {
        for v in parent.iter_mut() {
            if *v == stage.p {
                *v = stage.q;
            }
        }
    }
    let mut numbers: Vec<usize> = Vec::new();
    parent
        .iter()
        .map(|root| match numbers.iter().position(|r| r == root) {
            Some(i) => i + 1,
            None => {
                numbers.push(*root);
                numbers.len()
            }
        })
        .collect()
}

pub(crate) fn method_name(m: Linkage) -> &'static str {
    match m {
        Linkage::BetweenGroups => "Average Linkage (Between Groups)",
        Linkage::WithinGroups => "Average Linkage (Within Group)",
        Linkage::Single => "Single Linkage",
        Linkage::Complete => "Complete Linkage",
        Linkage::Centroid => "Centroid Linkage",
        Linkage::Median => "Median Linkage",
        Linkage::Ward => "Ward Linkage",
    }
}

fn schedule_table(stages: &[Stage], method: Linkage) -> Table {
    // the stage that last formed each cluster
    let mut formed = vec![0usize; stages.len() + 1];
    let mut rows = Vec::new();
    for (k, stage) in stages.iter().enumerate() {
        let next = stages[k + 1..]
            .iter()
            .position(|s| s.q == stage.q || s.p == stage.q)
            .map_or(0, |i| k + 2 + i);
        rows.push(vec![
            Cell::Number((k + 1) as f64),
            Cell::Number((stage.q + 1) as f64),
            Cell::Number((stage.p + 1) as f64),
            number(Some(stage.coefficient)),
            Cell::Number(formed.get(stage.q).copied().unwrap_or(0) as f64),
            Cell::Number(formed.get(stage.p).copied().unwrap_or(0) as f64),
            Cell::Number(next as f64),
        ]);
        if let Some(f) = formed.get_mut(stage.q) {
            *f = k + 1;
        }
    }
    let group = |g: &str| vec![g.to_string()];
    Table {
        title: "Agglomeration Schedule".into(),
        row_headers: 1,
        columns: [
            "Stage",
            "Cluster 1",
            "Cluster 2",
            "Coefficients",
            "Cluster 1",
            "Cluster 2",
            "Next Stage",
        ]
        .map(String::from)
        .to_vec(),
        column_groups: vec![
            Vec::new(),
            group("Cluster Combined"),
            group("Cluster Combined"),
            Vec::new(),
            group("Stage Cluster First Appears"),
            group("Stage Cluster First Appears"),
            Vec::new(),
        ],
        rows,
        caption: Some(method_name(method).into()),
        ..Table::default()
    }
}

/// The solutions from `min` to `max` clusters that the items allow, largest first.
fn solutions(n: usize, (min, max): (usize, usize)) -> Vec<usize> {
    let top = n.saturating_sub(1).max(1);
    let (min, max) = (min.min(top), max.min(top));
    (min..=max).rev().collect()
}

fn value_text(var: &Variable, case: usize) -> String {
    match var.values() {
        Values::Numeric(values) => values.get(case).map_or(String::new(), |v| {
            crate::format::category_text(v, var.format())
        }),
        Values::String(values) => values.get(case).trim_end().to_string(),
    }
}

impl Engine {
    fn cluster_items(&self, command: &Cluster) -> Result<Items, Error> {
        let data = self.active()?;
        let mut names = self.expand(&command.variables)?;
        if command.variables.contains(&VarItem::All) {
            names.retain(|n| data.variable(n).is_ok_and(|v| !v.is_string()));
        }
        let variables: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "CLUSTER needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let id = match &command.id {
            Some(name) => Some(data.variable(name)?),
            None => None,
        };
        let value = |var: &Variable, case: usize| -> Option<f64> {
            match (is_valid(var, case, command.include), var.values()) {
                (true, Values::Numeric(values)) => values.get(case),
                _ => None,
            }
        };
        let mut cases = Vec::new();
        let mut rows = Vec::new();
        for case in 0..data.case_count() {
            let row: Option<Vec<f64>> = variables.iter().map(|v| value(v, case)).collect();
            if let Some(row) = row {
                cases.push(case);
                rows.push(row);
            }
        }
        let missing = data.case_count() - cases.len();
        let n = rows.len();
        let mut prox = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in i + 1..n {
                let v = proximity(&command.measure, &rows[i], &rows[j]);
                if !v.is_finite() {
                    let label = |k: usize| self.case_number(cases[k]);
                    return Err(Error(format!(
                        "CLUSTER: the {} of cases {} and {} is undefined",
                        measure_name(&command.measure),
                        label(i),
                        label(j)
                    )));
                }
                prox[i][j] = v;
                prox[j][i] = v;
            }
        }
        let labels = cases
            .iter()
            .map(|&case| {
                let number = self.case_number(case);
                match &id {
                    Some(var) => format!("{number}:{}", value_text(var, case)),
                    None => number.to_string(),
                }
            })
            .collect();
        Ok(Items {
            labels,
            cases,
            prox,
            similarity: is_similarity(&command.measure),
            missing,
        })
    }

    /// The case number in the file, counting from 1.
    fn case_number(&self, case: usize) -> usize {
        self.case_numbers.as_ref().map_or(case, |n| n[case]) + 1
    }

    /// The items of a proximity matrix file: the rows of type PROX.
    fn cluster_matrix(&self, command: &Cluster, source: Option<&String>) -> Result<Items, Error> {
        let file;
        let data: &Dataset = match source {
            None => self.active()?,
            Some(name) => {
                file = self.data_file(name)?;
                &file
            }
        };
        let find = |name: &str| {
            data.variables()
                .find(|v| v.name().eq_ignore_ascii_case(name) && v.is_string())
                .ok_or_else(|| {
                    Error(format!(
                        "CLUSTER /MATRIX=IN needs the string variable {name} in the matrix file"
                    ))
                })
        };
        let (rowtype, varname) = (find("ROWTYPE_")?, find("VARNAME_")?);
        let all: Vec<Variable> = data
            .variables()
            .skip_while(|v| v.name() != varname.name())
            .skip(1)
            .filter(|v| !v.is_string())
            .collect();
        let chosen: Vec<Variable> = match command.variables.is_empty() {
            true => all,
            false => {
                let mut out = Vec::new();
                for item in &command.variables {
                    let VarItem::Name(name) = item else {
                        return Err(Error(
                            "CLUSTER /MATRIX=IN takes item names, without TO or ALL".into(),
                        ));
                    };
                    let v = all
                        .iter()
                        .find(|v| v.name().eq_ignore_ascii_case(name))
                        .ok_or_else(|| Error(format!("CLUSTER: the matrix has no item {name}")))?;
                    out.push(*v);
                }
                out
            }
        };
        let similarity = rowtype.value_labels().iter().any(|(value, label)| {
            matches!(value, Value::String(s) if s.trim().eq_ignore_ascii_case("PROX"))
                && label.to_uppercase().contains("SIMILARITY")
                && !label.to_uppercase().contains("DISSIMILARITY")
        });
        let label_var = match &command.id {
            Some(name) => Some(data.variable(name)?),
            None => data
                .variables()
                .find(|v| v.name().eq_ignore_ascii_case("ID") && v.is_string()),
        };
        let n = chosen.len();
        let mut prox = vec![vec![f64::NAN; n]; n];
        let mut labels = vec![String::new(); n];
        for case in 0..data.case_count() {
            if value_text(&rowtype, case).to_uppercase() != "PROX" {
                continue;
            }
            let of = value_text(&varname, case);
            let Some(i) = chosen
                .iter()
                .position(|v| v.name().eq_ignore_ascii_case(&of))
            else {
                continue;
            };
            labels[i] = match &label_var {
                Some(v) => value_text(v, case),
                None => chosen[i].name().to_string(),
            };
            for (j, var) in chosen.iter().enumerate() {
                if let Values::Numeric(values) = var.values()
                    && let Some(v) = values.get(case)
                {
                    prox[i][j] = v;
                }
            }
        }
        // a triangle fills the other half
        for (i, j) in (0..n).flat_map(|i| (0..n).map(move |j| (i, j))) {
            if i != j && prox[i][j].is_nan() {
                prox[i][j] = prox[j][i];
            }
        }
        if let Some((i, j)) = (0..n)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .find(|&(i, j)| i != j && prox[i][j].is_nan())
        {
            return Err(Error(format!(
                "CLUSTER /MATRIX=IN: the matrix has no proximity of {} and {}",
                chosen[i].name(),
                chosen[j].name()
            )));
        }
        Ok(Items {
            labels,
            cases: Vec::new(),
            prox,
            similarity,
            missing: 0,
        })
    }

    pub(crate) fn cluster(&mut self, command: &Cluster) -> Result<Vec<Table>, Error> {
        let items = match &command.matrix_in {
            Some(source) => {
                if command.save.is_some() {
                    return Err(Error(
                        "CLUSTER: SAVE with MATRIX=IN is not supported yet".into(),
                    ));
                }
                self.cluster_matrix(command, source.as_ref())?
            }
            None => {
                if self.active()?.weight_variable().is_some() {
                    self.warn("CLUSTER does not use the weight variable");
                }
                self.cluster_items(command)?
            }
        };
        let n = items.labels.len();
        if n < 2 {
            return Err(Error(format!(
                "CLUSTER needs at least 2 items to cluster, but there are {n}"
            )));
        }
        if command.plot {
            self.warn("CLUSTER: the icicle plot and the dendrogram are not drawn yet");
        }
        let mut tables = Vec::new();
        let measure = match command.matrix_in {
            Some(_) => match items.similarity {
                true => "a similarity matrix".to_string(),
                false => "a dissimilarity matrix".to_string(),
            },
            None => measure_name(&command.measure),
        };
        let total = n + items.missing;
        let percent = |k: usize| Cell::Number(100.0 * k as f64 / total as f64);
        tables.push(Table {
            title: "Case Processing Summary".into(),
            columns: ["N", "Percent", "N", "Percent", "N", "Percent"]
                .map(String::from)
                .to_vec(),
            column_groups: ["Valid", "Valid", "Missing", "Missing", "Total", "Total"]
                .map(|g| vec!["Cases".to_string(), g.to_string()])
                .to_vec(),
            rows: vec![vec![
                Cell::Number(n as f64),
                percent(n),
                Cell::Number(items.missing as f64),
                percent(items.missing),
                Cell::Number(total as f64),
                percent(total),
            ]],
            caption: Some(format!("{measure} used")),
            ..Table::default()
        });
        if command.distance {
            let rows = (0..n)
                .map(|i| {
                    let mut row = vec![text(items.labels[i].clone())];
                    row.extend((0..n).map(|j| match i == j {
                        true => Cell::Empty,
                        false => number(Some(items.prox[i][j])),
                    }));
                    row
                })
                .collect();
            let mut columns = vec!["Case".to_string()];
            columns.extend(items.labels.iter().cloned());
            let kind = match items.similarity {
                true => "similarity",
                false => "dissimilarity",
            };
            tables.push(Table {
                title: "Proximity Matrix".into(),
                row_headers: 1,
                column_groups: vec![Vec::new(); n + 1],
                columns,
                rows,
                caption: Some(format!("{measure}. This is a {kind} matrix.")),
                ..Table::default()
            });
        }
        // the saved variables of every method, solutions from the largest down
        let mut saved: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        let mut taken: Vec<String> = Vec::new();
        for (method, root) in &command.methods {
            let stages = agglomerate(&items.prox, items.similarity, *method);
            if command.schedule {
                tables.push(schedule_table(&stages, *method));
            }
            if let Some(range) = command.membership {
                let levels = solutions(n, range);
                let members: Vec<Vec<usize>> =
                    levels.iter().map(|&k| membership(n, &stages, k)).collect();
                let rows = (0..n)
                    .map(|i| {
                        let mut row = vec![text(items.labels[i].clone())];
                        row.extend(members.iter().map(|m| Cell::Number(m[i] as f64)));
                        row
                    })
                    .collect();
                let mut columns = vec!["Case".to_string()];
                columns.extend(levels.iter().map(|k| format!("{k} Clusters")));
                tables.push(Table {
                    title: "Cluster Membership".into(),
                    row_headers: 1,
                    column_groups: vec![Vec::new(); columns.len()],
                    columns,
                    rows,
                    caption: Some(method_name(*method).into()),
                    ..Table::default()
                });
            }
            if let Some(range) = command.save {
                let levels = solutions(n, range);
                let data = self.active()?;
                let free = |name: &String| {
                    data.variable_index(name).is_none()
                        && !taken.iter().any(|t| t.eq_ignore_ascii_case(name))
                };
                let names: Vec<String> = match root {
                    Some(root) => levels.iter().map(|k| format!("{root}{k}")).collect(),
                    None => (1..)
                        .map(|m| {
                            levels
                                .iter()
                                .map(|k| format!("CLU{k}_{m}"))
                                .collect::<Vec<_>>()
                        })
                        .find(|names| names.iter().all(free))
                        .expect("a free name"),
                };
                if let Some(name) = names.iter().find(|n| !free(n)) {
                    return Err(Error(format!(
                        "CLUSTER /SAVE: variable {name} already exists"
                    )));
                }
                for (k, name) in levels.iter().zip(names) {
                    let member = membership(n, &stages, *k);
                    let mut values = vec![None; data.case_count()];
                    for (i, &case) in items.cases.iter().enumerate() {
                        values[case] = Some(member[i] as f64);
                    }
                    taken.push(name.clone());
                    saved.push((
                        name,
                        format!("{} {k} clusters", method_name(*method)),
                        values,
                    ));
                }
            }
        }
        if !saved.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saved {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F8.0")?;
                    data.set_measure(&name, Measure::Nominal)?;
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}
