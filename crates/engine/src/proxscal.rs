//! PROXSCAL: reading the proximities, running the scaling of `mds` and its tables, charts and
//! files.
//!
//! Rules from the Command Syntax Reference (PROXSCAL), in our words:
//! - Without TABLE the cases hold one or more square matrices one after the other, a column
//!   per variable, so the cases divide evenly into matrices as tall as they are wide. TABLE
//!   row BY column reads one proximity per case and variable (each variable a source), or with
//!   BY source one variable and the source in a third; TABLE=source tells the stacked square
//!   matrices apart. The objects are the variables, or the rows of TABLE.
//! - SHAPE=LOWER uses the triangle below the diagonal, UPPER the one above, BOTH the weighted
//!   mean of the two; the diagonal is ignored. With TABLE a pair given only in the other
//!   triangle is used as well.
//! - WEIGHTS names one weight variable per proximity variable; a negative weight is an error,
//!   a missing one counts as 0. The weight variable of the data is not used.
//! - RATIO needs dissimilarities that are not negative; otherwise INTERVAL is used with a
//!   warning. One source runs the identity model. At least three objects, and at most one
//!   dimension fewer than the objects.
//! - MATRIX=IN reads a proximity matrix file as one source; a similarity matrix (by the value
//!   label of PROX) is scaled as similarities.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::descriptives::number;
use crate::matrix::{Matrix, multiply};
use crate::mds::{self, Criteria, Model, Problem, Solution, Transform, distances, identity, zeros};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::plot::{CaseGroup, CaseValues, Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{
    ProxShape, ProxTransformation, Proxscal, ProxscalTable, ScalingModel, ScalingStart, VarItem,
};

/// What to read the proximities from.
pub(crate) struct Reading<'a> {
    pub who: &'a str,
    pub variables: &'a [VarItem],
    pub weights: &'a [VarItem],
    pub table: Option<&'a ProxscalTable>,
    pub shape: ProxShape,
    pub matrix_in: &'a Option<Option<String>>,
    /// With MATRIX=IN the variable list picks the items (ALSCAL); otherwise all are used.
    pub matrix_items: bool,
}

/// The proximities as read, before the scaling turns them.
pub(crate) struct Input {
    pub objects: Vec<String>,
    pub sources: Vec<String>,
    /// Per source: symmetric proximities, NaN where missing.
    pub delta: Vec<Matrix>,
    pub w: Vec<Matrix>,
    /// The file said the proximities are similarities.
    pub similarity: bool,
    pub warnings: Vec<String>,
}

/// One full square matrix from raw cells: the triangle the shape names, BOTH the weighted
/// mean of the two; with `fallback` a pair missing there comes from the other triangle.
fn symmetric(raw: &Matrix, wraw: &Matrix, shape: ProxShape, fallback: bool) -> (Matrix, Matrix) {
    let n = raw.len();
    let mut d = vec![vec![f64::NAN; n]; n];
    let mut w = zeros(n, n);
    for i in 0..n {
        for j in 0..i {
            let lower = (raw[i][j], wraw[i][j]);
            let upper = (raw[j][i], wraw[j][i]);
            let ok = |c: (f64, f64)| c.0.is_finite() && c.1 > 0.0;
            let pick = match shape {
                ProxShape::Lower if ok(lower) => Some(lower),
                ProxShape::Lower if fallback && ok(upper) => Some(upper),
                ProxShape::Upper if ok(upper) => Some(upper),
                ProxShape::Upper if fallback && ok(lower) => Some(lower),
                ProxShape::Both => match (ok(lower), ok(upper)) {
                    (true, true) => Some((
                        (lower.0 * lower.1 + upper.0 * upper.1) / (lower.1 + upper.1),
                        lower.1 + upper.1,
                    )),
                    (true, false) => Some(lower),
                    (false, true) => Some(upper),
                    (false, false) => None,
                },
                _ => None,
            };
            if let Some((v, weight)) = pick {
                (d[i][j], d[j][i], w[i][j], w[j][i]) = (v, v, weight, weight);
            }
        }
    }
    (d, w)
}

/// A row, column or source number; `case` is the case number shown in the message.
fn whole_value(v: Option<f64>, what: &str, case: usize) -> Result<Option<usize>, Error> {
    match v {
        None => Ok(None),
        Some(v) if v >= 1.0 && v.fract() == 0.0 => Ok(Some(v as usize)),
        Some(v) => Err(Error(format!(
            "PROXSCAL: the {what} must be a whole number from 1, but case {case} has {v}"
        ))),
    }
}

impl Engine {
    pub(crate) fn proximity_input(&self, command: &Reading) -> Result<Input, Error> {
        let who = command.who;
        let mut warnings = Vec::new();
        if let Some(source) = &command.matrix_in {
            let chosen = match command.matrix_items {
                true => command.variables,
                false => &[],
            };
            if command.table.is_some() {
                warnings.push(format!(
                    "{who}: TABLE is ignored with MATRIX=IN; under SPLIT FILE each group is scaled on its own"
                ));
            }
            let items = self.proximity_matrix(who, chosen, None, source.as_ref())?;
            let n = items.labels.len();
            let w: Matrix = (0..n)
                .map(|i| (0..n).map(|j| f64::from(u8::from(i != j))).collect())
                .collect();
            let raw = items.prox.clone();
            let (delta, w) = symmetric(&raw, &w, ProxShape::Both, false);
            // both triangles counted: back to one weight per pair
            let w = w
                .into_iter()
                .map(|r| r.into_iter().map(|v| v / 2.0).collect())
                .collect();
            return Ok(Input {
                // ALSCAL has no case labels: the items keep their names
                objects: match command.matrix_items {
                    true => items.names,
                    false => items.labels,
                },
                sources: vec!["SRC_1".into()],
                delta: vec![delta],
                w: vec![w],
                similarity: items.similarity,
                warnings,
            });
        }
        let data = self.active()?;
        let variables: Vec<Variable> = self
            .expand(command.variables)?
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "{who} needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let weights: Vec<Variable> = match command.weights.is_empty() {
            true => Vec::new(),
            false => self
                .expand(command.weights)?
                .iter()
                .map(|n| data.variable(n))
                .collect::<Result<_, _>>()?,
        };
        if !weights.is_empty() && weights.len() != variables.len() {
            return Err(Error(format!(
                "PROXSCAL: WEIGHTS names {} variables, one for each of the {} proximity variables is needed",
                weights.len(),
                variables.len()
            )));
        }
        if data.weight_variable().is_some() {
            warnings.push(format!("{who} does not use the weight variable"));
        }
        let value = |v: &Variable, case: usize| match (is_valid(v, case, false), v.values()) {
            (true, Values::Numeric(values)) => values.get(case),
            _ => None,
        };
        let weight = |j: usize, case: usize| -> Result<f64, Error> {
            match weights.get(j) {
                None => Ok(1.0),
                Some(v) => match value(v, case) {
                    Some(w) if w < 0.0 => Err(Error(format!(
                        "PROXSCAL: weights cannot be negative, but {} is {w} in case {}",
                        v.name(),
                        self.case_number(case)
                    ))),
                    Some(w) => Ok(w),
                    None => Ok(0.0),
                },
            }
        };
        let cases = data.case_count();
        let table = command.table;
        if let Some(t) = table
            && let (Some(row), Some(column)) = (&t.row, &t.column)
        {
            let (row, column) = (data.variable(row)?, data.variable(column)?);
            let source = match &t.source {
                Some(s) if variables.len() == 1 => Some(data.variable(s)?),
                Some(_) => {
                    warnings.push(
                        "PROXSCAL: with several proximity variables the source variable of TABLE is ignored"
                            .to_string(),
                    );
                    None
                }
                None => None,
            };
            let mut cells = Vec::new();
            let (mut n, mut m) = (0, if source.is_some() { 0 } else { variables.len() });
            for case in 0..cases {
                let (Some(r), Some(c)) = (
                    whole_value(value(&row, case), "row", self.case_number(case))?,
                    whole_value(value(&column, case), "column", self.case_number(case))?,
                ) else {
                    continue;
                };
                let k = match &source {
                    Some(s) => match whole_value(value(s, case), "source", self.case_number(case))?
                    {
                        Some(k) => Some(k),
                        None => continue,
                    },
                    None => None,
                };
                n = n.max(r).max(c);
                cells.push((case, r - 1, c - 1, k));
            }
            // the source codes that occur, numbered in order: a gap makes no empty source
            let mut codes: Vec<usize> = cells.iter().filter_map(|c| c.3).collect();
            codes.sort_unstable();
            codes.dedup();
            if source.is_some() {
                m = codes.len();
            }
            let cells: Vec<(usize, usize, usize, Option<usize>)> = cells
                .into_iter()
                .map(|(case, r, c, k)| (case, r, c, k.and_then(|k| codes.binary_search(&k).ok())))
                .collect();
            let mut raw = vec![vec![vec![f64::NAN; n]; n]; m];
            let mut wraw = vec![zeros(n, n); m];
            for (case, r, c, k) in cells {
                for (j, v) in variables.iter().enumerate() {
                    let k = k.unwrap_or(j);
                    raw[k][r][c] = value(v, case).unwrap_or(f64::NAN);
                    wraw[k][r][c] = weight(j, case)?;
                }
            }
            let (delta, w): (Vec<Matrix>, Vec<Matrix>) = raw
                .iter()
                .zip(&wraw)
                .map(|(r, w)| symmetric(r, w, command.shape, true))
                .unzip();
            let objects = (1..=n)
                .map(|i| label_of(&row, i as f64).unwrap_or_else(|| i.to_string()))
                .collect();
            let sources = match &source {
                Some(s) => codes
                    .iter()
                    .map(|&k| label_of(s, k as f64).unwrap_or_else(|| format!("SRC_{k}")))
                    .collect(),
                None => variables.iter().map(|v| v.name().to_string()).collect(),
            };
            return Ok(Input {
                objects,
                sources,
                delta,
                w,
                similarity: false,
                warnings,
            });
        }
        // square matrices, case after case, told apart by TABLE=source when given
        let n = variables.len();
        let groups: Vec<(String, Vec<usize>)> = match table.and_then(|t| t.source.as_ref()) {
            Some(name) => {
                let s = data.variable(name)?;
                let mut keys: Vec<(f64, Vec<usize>)> = Vec::new();
                for case in 0..cases {
                    let Some(k) = value(&s, case) else { continue };
                    match keys.iter_mut().find(|(v, _)| *v == k) {
                        Some((_, list)) => list.push(case),
                        None => keys.push((k, vec![case])),
                    }
                }
                keys.sort_by(|a, b| a.0.total_cmp(&b.0));
                keys.into_iter()
                    .map(|(k, list)| {
                        let name = label_of(&s, k).unwrap_or_else(|| format!("SRC_{k}"));
                        (name, list)
                    })
                    .collect()
            }
            None => {
                if cases % n.max(1) != 0 {
                    return Err(Error(format!(
                        "{who}: the number of cases, {cases}, is not a multiple of the number of variables, {n}"
                    )));
                }
                (0..cases / n.max(1))
                    .map(|k| (format!("SRC_{}", k + 1), (k * n..(k + 1) * n).collect()))
                    .collect()
            }
        };
        let mut delta = Vec::new();
        let mut w = Vec::new();
        for (name, rows) in &groups {
            if rows.len() != n {
                return Err(Error(format!(
                    "{who}: source {name} has {} rows, but a square matrix of {n} variables needs {n}",
                    rows.len()
                )));
            }
            let mut raw = zeros(n, n);
            let mut wraw = zeros(n, n);
            for (i, &case) in rows.iter().enumerate() {
                for (j, v) in variables.iter().enumerate() {
                    raw[i][j] = value(v, case).unwrap_or(f64::NAN);
                    wraw[i][j] = weight(j, case)?;
                }
            }
            let (d, wk) = symmetric(&raw, &wraw, command.shape, false);
            delta.push(d);
            w.push(wk);
        }
        Ok(Input {
            objects: variables.iter().map(|v| v.name().to_string()).collect(),
            sources: groups.into_iter().map(|(name, _)| name).collect(),
            delta,
            w,
            similarity: false,
            warnings,
        })
    }

    pub(crate) fn proxscal(&mut self, command: &Proxscal) -> Result<Vec<Table>, Error> {
        let input = self.proximity_input(&Reading {
            who: "PROXSCAL",
            variables: &command.variables,
            weights: &command.weights,
            table: command.table.as_ref(),
            shape: command.shape,
            matrix_in: &command.matrix_in,
            matrix_items: false,
        })?;
        let n = input.objects.len();
        let m = input.sources.len();
        if n < 3 {
            return Err(Error(format!(
                "PROXSCAL needs at least 3 objects, but there are {n}"
            )));
        }
        if m == 0 {
            return Err(Error("PROXSCAL has no proximities".into()));
        }
        let (min_dim, max_dim) = command.dimensions;
        if max_dim > n - 1 {
            return Err(Error(format!(
                "PROXSCAL: {max_dim} dimensions need at least {} objects, but there are {n}",
                max_dim + 1
            )));
        }
        let mut warnings = input.warnings.clone();
        let similarities = command.similarities || input.similarity;
        let mut transform = match command.transformation {
            ProxTransformation::Ratio => Transform::Ratio,
            ProxTransformation::Interval => Transform::Interval,
            ProxTransformation::Ordinal { untie } => Transform::Ordinal { untie },
            ProxTransformation::Spline { degree, inknot } => Transform::Spline { degree, inknot },
        };
        let negative = (0..m).any(|k| {
            (0..n).any(|i| (0..n).any(|j| input.w[k][i][j] > 0.0 && input.delta[k][i][j] < 0.0))
        });
        if transform == Transform::Ratio && (similarities || negative) {
            warnings.push(
                "PROXSCAL: RATIO needs dissimilarities that are not negative; INTERVAL is used"
                    .to_string(),
            );
            transform = Transform::Interval;
        }
        let mut model = match command.model {
            ScalingModel::Identity => Model::Identity,
            ScalingModel::Weighted => Model::Weighted,
            ScalingModel::Generalized => Model::Generalized,
            ScalingModel::Reduced(r) => Model::Reduced(r),
        };
        if m == 1 && model != Model::Identity {
            warnings.push("PROXSCAL: one source only, so the identity model is used".to_string());
            model = Model::Identity;
        }
        let groups = match command.unconditional {
            true => vec![(0..m).collect()],
            false => (0..m).map(|k| vec![k]).collect(),
        };
        let mut problem = Problem {
            n,
            delta: input
                .delta
                .iter()
                .map(|d| {
                    d.iter()
                        .map(|r| {
                            r.iter()
                                .map(|v| if v.is_finite() { *v } else { 0.0 })
                                .collect()
                        })
                        .collect()
                })
                .collect(),
            w: input.w.clone(),
            transform,
            groups,
        };
        if (0..m).all(|k| problem.w[k].iter().flatten().all(|w| *w == 0.0)) {
            return Err(Error("PROXSCAL: every proximity is missing".into()));
        }
        let Some((_, vinv)) = mds::mean_laplacian(&problem) else {
            return Err(Error(
                "PROXSCAL: the proximities that are not missing leave the objects in separate groups"
                    .into(),
            ));
        };
        mds::prepare(&mut problem, similarities);
        let start_dhat = mds::initial_dhat(&problem);
        let criteria = Criteria {
            max_iterations: command.max_iterations,
            diff_stress: command.diff_stress,
            min_stress: command.min_stress,
            acceleration: command.acceleration,
        };
        let mut dims: Vec<usize> = (min_dim..=max_dim).rev().collect();
        let mut random_stress = Vec::new();
        if let ScalingStart::Random(_) = command.initial
            && min_dim < max_dim
        {
            warnings.push(format!(
                "PROXSCAL: random starts do not reduce dimensions; only {min_dim} dimensions are computed"
            ));
            dims = vec![min_dim];
        }
        let mut solutions: Vec<(usize, Solution, Solution)> = Vec::new();
        for &p in &dims {
            let raw = match solutions.last() {
                Some((_, raw, _)) => {
                    let (z, a) = mds::reduce(model, raw);
                    mds::iterate(&problem, model, &criteria, z, a, start_dhat.clone())
                }
                None => match command.initial {
                    ScalingStart::Simplex | ScalingStart::Torgerson => {
                        let z = match command.initial {
                            ScalingStart::Simplex => mds::simplex(&problem, &start_dhat, &vinv, p),
                            _ => mds::torgerson(&problem, &start_dhat, p),
                        };
                        mds::iterate(
                            &problem,
                            model,
                            &criteria,
                            z,
                            vec![identity(p); m],
                            start_dhat.clone(),
                        )
                    }
                    ScalingStart::Random(starts) => {
                        let mut best: Option<Solution> = None;
                        for _ in 0..starts {
                            let z: Matrix = (0..n)
                                .map(|_| (0..p).map(|_| self.random()).collect())
                                .collect();
                            let s = mds::iterate(
                                &problem,
                                model,
                                &criteria,
                                z,
                                vec![identity(p); m],
                                start_dhat.clone(),
                            );
                            let stress = s.history.last().copied().unwrap_or(1.0);
                            random_stress.push(stress);
                            if best
                                .as_ref()
                                .is_none_or(|b| stress < b.history.last().copied().unwrap_or(1.0))
                            {
                                best = Some(s);
                            }
                        }
                        best.expect("at least one start")
                    }
                },
            };
            let done = mds::finish(model, raw.clone());
            solutions.push((p, raw, done));
        }
        let several = solutions.len() > 1;
        let mut tables = vec![summary(&problem, m)];
        let print = &command.print;
        if print.input {
            for (k, name) in input.sources.iter().enumerate() {
                tables.push(square(
                    titled("Proximities", name, m),
                    &input.objects,
                    &input.delta[k],
                    Some(&input.w[k]),
                ));
            }
        }
        if print.random && !random_stress.is_empty() {
            tables.push(Table {
                title: "Random Starts".into(),
                columns: vec!["Start".into(), "Normalized Raw Stress".into()],
                rows: random_stress
                    .iter()
                    .enumerate()
                    .map(|(i, s)| vec![Cell::Number((i + 1) as f64), Cell::Number(*s)])
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        let mut charts: Vec<Chart> = Vec::new();
        for (p, raw, done) in &solutions {
            let suffix = |title: &str| match several {
                true => format!("{title} ({p} Dimensions)"),
                false => title.to_string(),
            };
            let x: Vec<Matrix> = done.a.iter().map(|a| multiply(&done.z, a)).collect();
            let sums = mds::sums(&problem, &done.dhat, &x);
            if print.history {
                let rows = raw
                    .history
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let gain = match i {
                            0 => Cell::Empty,
                            _ => Cell::Number(raw.history[i - 1] - s),
                        };
                        vec![Cell::Number(i as f64), Cell::Number(*s), gain]
                    })
                    .collect();
                tables.push(Table {
                    title: suffix("Iteration History"),
                    columns: ["Iteration", "Normalized Raw Stress", "Improvement"]
                        .map(String::from)
                        .to_vec(),
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
            let fit = mds::fit_measures(&sums);
            let names = [
                "Normalized Raw Stress",
                "Stress-I",
                "Stress-II",
                "S-Stress",
                "Dispersion Accounted For (D.A.F.)",
                "Tucker's Coefficient of Congruence",
            ];
            let shown: Vec<usize> = match print.stress {
                true => (0..6).collect(),
                false => vec![0, 4, 5],
            };
            tables.push(Table {
                title: suffix("Stress and Fit Measures"),
                columns: vec![String::new(), "Value".into()],
                rows: shown
                    .iter()
                    .map(|&i| vec![text(names[i]), number(Some(fit[i]))])
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
            if print.decomposition {
                let mut table = decomposition(&problem, &done.dhat, &x, &sums, &input);
                table.title = suffix(&table.title);
                tables.push(table);
            }
            let dims_cols: Vec<String> = (1..=*p).map(|d| format!("Dimension {d}")).collect();
            if print.common {
                tables.push(coordinates(
                    suffix("Final Coordinates"),
                    &input.objects,
                    &done.z,
                    &dims_cols,
                ));
            }
            if print.weights && model != Model::Identity {
                let after = match several {
                    true => format!(" ({p} Dimensions)"),
                    false => String::new(),
                };
                tables.push(space_weights(
                    model,
                    &after,
                    &input.sources,
                    &done.a,
                    &dims_cols,
                ));
            }
            if print.individual && model != Model::Identity {
                for (k, name) in input.sources.iter().enumerate() {
                    tables.push(coordinates(
                        suffix(&format!("Individual Space Coordinates: {name}")),
                        &input.objects,
                        &x[k],
                        &dims_cols,
                    ));
                }
            }
            if print.distances {
                let count = if model == Model::Identity { 1 } else { m };
                for (k, xk) in x.iter().enumerate().take(count) {
                    let title = match count {
                        1 => suffix("Distances"),
                        _ => suffix(&format!("Distances: {}", input.sources[k])),
                    };
                    tables.push(square(title, &input.objects, &distances(xk), None));
                }
            }
            if print.transformations {
                for (k, name) in input.sources.iter().enumerate() {
                    tables.push(square(
                        suffix(&titled("Transformed Proximities", name, m)),
                        &input.objects,
                        &done.dhat[k],
                        Some(&problem.w[k]),
                    ));
                }
            }
        }
        let (p, _, done) = solutions.last().expect("at least one dimensionality");
        let x: Vec<Matrix> = done.a.iter().map(|a| multiply(&done.z, a)).collect();
        let plot = &command.plot;
        if plot.stress && several {
            let mut ordered: Vec<(usize, f64)> = solutions
                .iter()
                .map(|(p, raw, _)| (*p, raw.history.last().copied().unwrap_or(1.0)))
                .collect();
            ordered.sort_by_key(|(p, _)| *p);
            charts.push(Chart::lines(
                "Scree Plot".into(),
                Axis::titled("Dimensionality"),
                Axis::titled("Normalized Raw Stress"),
                ordered
                    .iter()
                    .map(|(p, _)| Category {
                        label: p.to_string(),
                        value: Cell::Number(*p as f64),
                        missing: false,
                        bin: None,
                    })
                    .collect(),
                vec![Series {
                    name: "Normalized Raw Stress".into(),
                    statistic: Statistic::Value,
                    variable: None,
                    value: Cell::Empty,
                    values: ordered.iter().map(|(_, s)| *s).collect(),
                }],
            ));
        }
        if plot.common {
            charts.push(space_chart(
                "Common Space".into(),
                &input.objects,
                &done.z,
                *p,
            ));
        }
        if plot.weights && model == Model::Weighted && *p >= 2 {
            let points = input
                .sources
                .iter()
                .zip(&done.a)
                .map(|(name, a)| Point {
                    x: a[0][0],
                    y: a[1][1],
                    z: None,
                    label: Some(name.clone()),
                })
                .collect();
            charts.push(Chart::plot(
                ChartKind::Scatter,
                "Dimension Weights".into(),
                Axis::titled("Dimension 1"),
                Axis::titled("Dimension 2"),
                Plot::Points {
                    groups: vec![PointGroup {
                        name: "Sources".into(),
                        points,
                    }],
                    lines: Vec::new(),
                    z_axis: None,
                },
            ));
        }
        let chosen = |list: &Option<Vec<String>>, warnings: &mut Vec<String>| -> Vec<usize> {
            let Some(list) = list else { return Vec::new() };
            if list.is_empty() {
                return (0..m).collect();
            }
            list.iter()
                .filter_map(|name| {
                    let found = input
                        .sources
                        .iter()
                        .position(|s| s.eq_ignore_ascii_case(name));
                    if found.is_none() {
                        warnings.push(format!("PROXSCAL: there is no source {name} to plot"));
                    }
                    found
                })
                .collect()
        };
        if model != Model::Identity {
            for k in chosen(&plot.individual, &mut warnings) {
                charts.push(space_chart(
                    format!("Individual Space: {}", input.sources[k]),
                    &input.objects,
                    &x[k],
                    *p,
                ));
            }
        }
        let pair_points = |k: usize, f: &dyn Fn(usize, usize) -> (f64, f64)| -> PointGroup {
            PointGroup {
                name: input.sources[k].clone(),
                points: (0..n)
                    .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
                    .filter(|&(i, j)| problem.w[k][i][j] > 0.0)
                    .map(|(i, j)| {
                        let (x, y) = f(i, j);
                        Point {
                            x,
                            y,
                            z: None,
                            label: None,
                        }
                    })
                    .collect(),
            }
        };
        let d: Vec<Matrix> = x.iter().map(distances).collect();
        // one plot per list of sources
        let lists = [
            (
                &plot.transformations,
                false,
                "Transformation Plot",
                "Proximities",
                "Transformed Proximities",
            ),
            (
                &plot.residuals,
                true,
                "Residuals Plot",
                "Transformed Proximities",
                "Distances",
            ),
        ];
        for (asked, residuals, title, x_title, y_title) in lists {
            for list in asked.iter().flatten() {
                let sources = chosen(&Some(list.clone()), &mut warnings);
                if sources.is_empty() {
                    continue;
                }
                let groups = sources
                    .iter()
                    .map(|&k| match residuals {
                        true => pair_points(k, &|i, j| (done.dhat[k][i][j], d[k][i][j])),
                        false => pair_points(k, &|i, j| (input.delta[k][i][j], done.dhat[k][i][j])),
                    })
                    .collect();
                charts.push(Chart::plot(
                    ChartKind::Scatter,
                    title.into(),
                    Axis::titled(x_title),
                    Axis::titled(y_title),
                    Plot::Points {
                        groups,
                        lines: Vec::new(),
                        z_axis: None,
                    },
                ));
            }
        }
        let out = &command.outfile;
        if let Some(file) = &out.common {
            let mut data = Dataset::new(n);
            for d in 0..*p {
                data.set_numeric_values(
                    &format!("DIM_{}", d + 1),
                    done.z.iter().map(|r| Some(r[d])).collect(),
                )?;
            }
            self.write_data_file(file, data)?;
        }
        if let Some(file) = &out.weights {
            if model == Model::Identity {
                warnings.push("PROXSCAL: the identity model has no space weights to save".into());
            } else {
                let rows: Vec<Vec<f64>> = match model {
                    Model::Weighted => done
                        .a
                        .iter()
                        .map(|a| (0..*p).map(|d| a[d][d]).collect())
                        .collect(),
                    _ => done.a.iter().flat_map(|a| a.iter().cloned()).collect(),
                };
                let mut data = Dataset::new(rows.len());
                for d in 0..*p {
                    data.set_numeric_values(
                        &format!("DIM_{}", d + 1),
                        rows.iter().map(|r| Some(r[d])).collect(),
                    )?;
                }
                self.write_data_file(file, data)?;
            }
        }
        for (target, values, masked) in [
            (&out.distances, d, false),
            (&out.transformations, done.dhat.clone(), true),
        ] {
            let Some(file) = target else { continue };
            let mut data = Dataset::new(n * m);
            for j in 0..n {
                // a missing proximity has no transformed value
                let column = values
                    .iter()
                    .enumerate()
                    .flat_map(|(k, v)| {
                        let w = &problem.w[k];
                        v.iter()
                            .enumerate()
                            .map(move |(i, r)| (!masked || i == j || w[i][j] > 0.0).then_some(r[j]))
                    })
                    .collect();
                data.set_numeric_values(&format!("OBJ_{}", j + 1), column)?;
            }
            self.write_data_file(file, data)?;
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

/// The value label of `value`, or None.
fn label_of(v: &Variable, value: f64) -> Option<String> {
    v.value_labels()
        .iter()
        .find(|(k, _)| matches!(k, Value::Number(x) if *x == value))
        .map(|(_, label)| label.clone())
}

fn titled(title: &str, source: &str, m: usize) -> String {
    match m {
        1 => title.to_string(),
        _ => format!("{title}: {source}"),
    }
}

fn summary(problem: &Problem, m: usize) -> Table {
    let n = problem.n;
    let total = m * n * (n - 1) / 2;
    let active: usize = (0..m)
        .map(|k| {
            (0..n)
                .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
                .filter(|&(i, j)| problem.w[k][i][j] > 0.0)
                .count()
        })
        .sum();
    let row = |label: &str, v: usize| vec![text(label), Cell::Number(v as f64)];
    Table {
        title: "Case Processing Summary".into(),
        columns: vec![String::new(), "N".into()],
        rows: vec![
            row("Sources", m),
            row("Objects", n),
            row("Proximities", total),
            row("Missing Proximities", total - active),
            row("Active Proximities", active),
        ],
        row_headers: 1,
        ..Table::default()
    }
}

/// An object by object matrix; with `w` the pairs of weight 0 and the diagonal are empty.
pub(crate) fn square(
    title: String,
    objects: &[String],
    values: &Matrix,
    w: Option<&Matrix>,
) -> Table {
    let n = objects.len();
    let mut columns = vec![String::new()];
    columns.extend(objects.iter().cloned());
    Table {
        title,
        columns,
        rows: (0..n)
            .map(|i| {
                let mut row = vec![text(objects[i].clone())];
                row.extend((0..n).map(|j| match w {
                    Some(w) if i == j || w[i][j] <= 0.0 => Cell::Empty,
                    _ => number(Some(values[i][j])),
                }));
                row
            })
            .collect(),
        row_headers: 1,
        ..Table::default()
    }
}

pub(crate) fn coordinates(title: String, objects: &[String], z: &Matrix, dims: &[String]) -> Table {
    let mut columns = vec![String::new()];
    columns.extend(dims.iter().cloned());
    Table {
        title,
        columns,
        rows: objects
            .iter()
            .zip(z)
            .map(|(name, r)| {
                let mut row = vec![text(name.clone())];
                row.extend(r.iter().map(|v| Cell::Number(*v)));
                row
            })
            .collect(),
        row_headers: 1,
        ..Table::default()
    }
}

fn space_weights(
    model: Model,
    after: &str,
    sources: &[String],
    a: &[Matrix],
    dims: &[String],
) -> Table {
    let p = dims.len();
    match model {
        Model::Weighted => {
            let mut columns = vec!["Source".to_string()];
            columns.extend(dims.iter().cloned());
            Table {
                title: format!("Dimension Weights{after}"),
                columns,
                rows: sources
                    .iter()
                    .zip(a)
                    .map(|(name, a)| {
                        let mut row = vec![text(name.clone())];
                        row.extend((0..p).map(|d| Cell::Number(a[d][d])));
                        row
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            }
        }
        _ => {
            let mut columns = vec!["Source".to_string(), "Dimension".to_string()];
            columns.extend(dims.iter().cloned());
            let mut rows = Vec::new();
            for (name, a) in sources.iter().zip(a) {
                for (d, r) in a.iter().enumerate() {
                    let mut row = vec![
                        if d == 0 {
                            text(name.clone())
                        } else {
                            Cell::Empty
                        },
                        Cell::Number((d + 1) as f64),
                    ];
                    row.extend(r.iter().map(|v| Cell::Number(*v)));
                    rows.push(row);
                }
            }
            Table {
                title: format!("Space Weights{after}"),
                columns,
                rows,
                row_headers: 2,
                ..Table::default()
            }
        }
    }
}

/// The share of the normalized raw stress of every object in every source.
fn decomposition(
    problem: &Problem,
    dhat: &[Matrix],
    x: &[Matrix],
    s: &mds::Sums,
    input: &Input,
) -> Table {
    let n = problem.n;
    let m = dhat.len();
    let alpha = s.rho / s.eta_x;
    let mut cells = zeros(n, m);
    for k in 0..m {
        let d = distances(&x[k]);
        for i in 0..n {
            for j in 0..n {
                if i != j && problem.w[k][i][j] > 0.0 {
                    cells[i][k] +=
                        0.5 * problem.w[k][i][j] * (dhat[k][i][j] - alpha * d[i][j]).powi(2)
                            / s.eta_dhat;
                }
            }
        }
    }
    let mut columns = vec![String::new()];
    columns.extend(input.sources.iter().cloned());
    columns.push("Total".into());
    let mut rows: Vec<Vec<Cell>> = (0..n)
        .map(|i| {
            let mut row = vec![text(input.objects[i].clone())];
            row.extend(cells[i].iter().map(|v| Cell::Number(*v)));
            row.push(Cell::Number(cells[i].iter().sum()));
            row
        })
        .collect();
    let mut total = vec![text("Total")];
    total.extend((0..m).map(|k| Cell::Number(cells.iter().map(|r| r[k]).sum())));
    total.push(Cell::Number(cells.iter().flatten().sum()));
    rows.push(total);
    Table {
        title: "Decomposition of Normalized Raw Stress".into(),
        columns,
        rows,
        row_headers: 1,
        ..Table::default()
    }
}

/// The objects in a space: a scatter in two dimensions, a scatterplot matrix in more, the
/// points on a line in one.
pub(crate) fn space_chart(title: String, objects: &[String], z: &Matrix, p: usize) -> Chart {
    let axis = |d: usize| Axis::titled(format!("Dimension {d}"));
    if p >= 3 {
        return Chart::plot(
            ChartKind::Scatter,
            title,
            Axis::titled(String::new()),
            Axis::titled(String::new()),
            Plot::Matrix {
                variables: (1..=p).map(axis).collect(),
                groups: vec![CaseGroup {
                    name: "Objects".into(),
                    cases: objects
                        .iter()
                        .zip(z)
                        .map(|(name, r)| CaseValues {
                            values: r.clone(),
                            label: Some(name.clone()),
                        })
                        .collect(),
                }],
            },
        );
    }
    let points = objects
        .iter()
        .zip(z)
        .map(|(name, r)| Point {
            x: r[0],
            y: r.get(1).copied().unwrap_or(0.0),
            z: None,
            label: Some(name.clone()),
        })
        .collect();
    Chart::plot(
        ChartKind::Scatter,
        title,
        axis(1),
        match p {
            1 => Axis::titled(String::new()),
            _ => axis(2),
        },
        Plot::Points {
            groups: vec![PointGroup {
                name: "Objects".into(),
                points,
            }],
            lines: Vec::new(),
            z_axis: None,
        },
    )
}
