//! PREFSCAL: reading the rectangular proximities, running the unfolding of `unfolding` and
//! its tables, charts and files.
//!
//! Rules from the Command Syntax Reference (PREFSCAL), in our words:
//! - The variables are the column objects; each case is a row object of a source. ROWS(n) and
//!   SOURCES(n) split the cases evenly (both: the first rows times sources cases); a row
//!   variable numbers the rows and a smaller value starts the next source; a source variable
//!   numbers the sources, whose cases follow each other. Identifiers are whole numbers from 1
//!   and are not among the variables. A cell given twice keeps the last value.
//! - WEIGHTS names one nonnegative weight variable per variable; the weight variable of the
//!   data weights whole rows. A missing proximity is left out. SPLIT FILE is not used.
//! - At least two rows and two columns. With one source the model is the identity model (a
//!   warning for WEIGHTED and GENERALIZED).
//! - A start from a file has one case per object, rows then columns, and no missing value.
//!   RESTRICTIONS fixes the coordinates of the rows or the columns to those of a file with one
//!   case per object of the set; a missing value leaves that coordinate free. The file's
//!   variables are the ones named, else the first MAXDIM numeric ones, and the first p of them
//!   serve in p dimensions. Restrictions go with the identity model only.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::descriptives::{case_weights, number};
use crate::matrix::{Matrix, multiply};
use crate::mds::{self, Criteria, Model, zeros};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::plot::{CaseGroup, CaseValues, Plot, Point, PointGroup};
use crate::proxscal::{coordinates, label_of, space_weights, titled};
use crate::unfolding::{Entry, Fit, Kind, MEASURES, Unfolding};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    InputCount, Prefscal, PrefscalCondition, PrefscalFile, PrefscalStart, PrefscalTransformation,
    ScalingModel, ValueList,
};

/// The names shown for a value of an identifier: its value label or the number.
fn shown(v: &Variable, value: f64) -> String {
    label_of(v, value).unwrap_or_else(|| match value.fract() == 0.0 {
        true => format!("{value:.0}"),
        false => value.to_string(),
    })
}

/// Whether `value` (or the 1-based position `place`) is in a value list; empty means all.
fn picked(list: &ValueList, value: Option<f64>, place: usize) -> bool {
    let v = value.unwrap_or(place as f64);
    list.is_empty() || list.iter().any(|(lo, hi)| *lo <= v && v <= *hi)
}

/// Rows and sources as read.
struct Layout {
    rows: Vec<String>,
    row_ids: Vec<Option<f64>>,
    sources: Vec<String>,
    source_ids: Vec<Option<f64>>,
    /// The first case of every row object, for OPTIONS.
    first_case: Vec<usize>,
}

impl Engine {
    /// The coordinates a data file or dataset holds, one case per object: the variables named,
    /// else the first numeric ones, `p` of them. A user-missing value counts as missing.
    pub(crate) fn file_coordinates(
        &self,
        who: &str,
        spec: &PrefscalFile,
        objects: usize,
        p: usize,
        what: &str,
    ) -> Result<Vec<Vec<Option<f64>>>, Error> {
        let data = self.data_file(&spec.file)?;
        let names: Vec<String> = match spec.variables.is_empty() {
            true => data
                .variable_names()
                .filter(|n| data.variable(n).is_ok_and(|v| !v.is_string()))
                .map(str::to_string)
                .collect(),
            false => crate::expand_in(&data, &spec.variables)?,
        };
        if names.len() < p {
            return Err(Error(format!(
                "{who}: {} holds {} numeric variables for the {what} coordinates, but {p} dimensions \
                 (the largest of CRITERIA DIMENSIONS) need {p}",
                spec.file,
                names.len()
            )));
        }
        if data.case_count() != objects {
            return Err(Error(format!(
                "{who}: {} has {} cases for the {what} coordinates, but there are {objects} objects",
                spec.file,
                data.case_count()
            )));
        }
        let mut out = vec![Vec::with_capacity(p); objects];
        for name in &names[..p] {
            let variable = data.variable(name)?;
            let Values::Numeric(values) = variable.values() else {
                return Err(Error(format!(
                    "{who}: {name} in {} is a string variable, not coordinates",
                    spec.file
                )));
            };
            for (row, v) in out.iter_mut().zip(values.iter()) {
                row.push(v.filter(|v| !variable.is_user_missing(*v)));
            }
        }
        Ok(out)
    }

    pub(crate) fn prefscal(&mut self, command: &Prefscal) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut warnings = Vec::new();
        let variables: Vec<Variable> = self
            .expand(&command.variables)?
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "PREFSCAL needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let m = variables.len();
        if m < 2 {
            return Err(Error(
                "PREFSCAL needs at least two variables (columns)".into(),
            ));
        }
        let weights: Vec<Variable> = self
            .expand(&command.weights)?
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if !weights.is_empty() && weights.len() != m {
            return Err(Error(format!(
                "PREFSCAL: WEIGHTS names {} variables, one for each of the {m} variables is needed",
                weights.len()
            )));
        }
        let value = |v: &Variable, case: usize| match (is_valid(v, case, false), v.values()) {
            (true, Values::Numeric(values)) => values.get(case),
            _ => None,
        };
        let id_var = |count: &Option<InputCount>| -> Result<Option<Variable>, Error> {
            match count {
                Some(InputCount::Variable(name)) => {
                    let v = data.variable(name)?;
                    if variables.iter().any(|x| x.name() == v.name()) {
                        return Err(Error(format!(
                            "PREFSCAL: the identifier {} cannot be one of the proximity variables",
                            v.name()
                        )));
                    }
                    Ok(Some(v))
                }
                _ => Ok(None),
            }
        };
        let number_of = |count: &Option<InputCount>| match count {
            Some(InputCount::Number(n)) => Some(*n),
            _ => None,
        };
        let (row_var, source_var) = (id_var(&command.rows)?, id_var(&command.sources)?);
        let (rows_n, sources_n) = (number_of(&command.rows), number_of(&command.sources));
        let mut cases = data.case_count();
        if let (Some(r), Some(s)) = (rows_n, sources_n) {
            if r * s > cases {
                return Err(Error(format!(
                    "PREFSCAL: {r} rows in {s} sources need {} cases, but there are {cases}",
                    r * s
                )));
            }
            cases = r * s;
        }
        let per_source = match (rows_n, sources_n) {
            (Some(r), _) => r,
            (None, Some(s)) => {
                if cases % s != 0 {
                    return Err(Error(format!(
                        "PREFSCAL: the {cases} cases do not divide into {s} sources"
                    )));
                }
                cases / s
            }
            (None, None) => cases,
        };
        if per_source == 0 || (source_var.is_none() && row_var.is_none() && cases % per_source != 0)
        {
            return Err(Error(format!(
                "PREFSCAL: the {cases} cases do not divide into sources of {per_source} rows"
            )));
        }
        let id = |v: &Variable, case: usize, what: &str| -> Result<f64, Error> {
            match value(v, case) {
                Some(x) if x >= 1.0 && x.fract() == 0.0 => Ok(x),
                other => Err(Error(format!(
                    "PREFSCAL: the {what} identifier {} must be a whole number from 1, but case {} has {}",
                    v.name(),
                    self.case_number(case),
                    other.map_or("a missing value".to_string(), |x| x.to_string())
                ))),
            }
        };
        let sorted_ids = |v: &Option<Variable>, what: &str| -> Result<Vec<f64>, Error> {
            let Some(v) = v else { return Ok(Vec::new()) };
            let mut ids = (0..cases)
                .map(|c| id(v, c, what))
                .collect::<Result<Vec<f64>, Error>>()?;
            ids.sort_by(f64::total_cmp);
            ids.dedup();
            Ok(ids)
        };
        let (row_codes, source_codes) = (
            sorted_ids(&row_var, "row")?,
            sorted_ids(&source_var, "source")?,
        );
        // the (row, source) of every case
        let mut place = Vec::with_capacity(cases);
        let (mut source, mut previous_row) = (0usize, f64::INFINITY);
        let mut count_in = std::collections::HashMap::new();
        let rank = |codes: &[f64], x: f64| codes.partition_point(|c| *c < x);
        for case in 0..cases {
            let k = match (&source_var, &row_var) {
                (Some(v), _) => rank(&source_codes, id(v, case, "source")?),
                (None, Some(v)) => {
                    let r = id(v, case, "row")?;
                    if r < previous_row && case > 0 {
                        source += 1;
                    }
                    previous_row = r;
                    source
                }
                (None, None) => case / per_source,
            };
            let i = match &row_var {
                Some(v) => rank(&row_codes, id(v, case, "row")?),
                None => {
                    let next = count_in.entry(k).or_insert(0usize);
                    *next += 1;
                    *next - 1
                }
            };
            place.push((case, i, k));
        }
        let n = place.iter().map(|p| p.1 + 1).max().unwrap_or(0);
        let sources = place.iter().map(|p| p.2 + 1).max().unwrap_or(0);
        if n < 2 {
            return Err(Error(format!(
                "PREFSCAL needs at least two rows, but there are {n}"
            )));
        }
        let case_w = case_weights(data)?;
        let mut first_case = vec![usize::MAX; n];
        let mut cells = vec![vec![vec![None::<(f64, f64)>; m]; n]; sources];
        for &(case, i, k) in &place {
            first_case[i] = first_case[i].min(case);
            let row_w = case_w.as_ref().map_or(1.0, |w| w[case]);
            for (j, v) in variables.iter().enumerate() {
                let w = match weights.get(j) {
                    None => 1.0,
                    Some(wv) => match value(wv, case) {
                        Some(x) if x < 0.0 => {
                            return Err(Error(format!(
                                "PREFSCAL: weights cannot be negative, but {} is {x} in case {}",
                                wv.name(),
                                self.case_number(case)
                            )));
                        }
                        Some(x) => x,
                        None => 0.0,
                    },
                } * row_w;
                cells[k][i][j] = value(v, case).map(|x| (x, w));
            }
        }
        let layout = Layout {
            rows: (0..n)
                .map(|i| match &row_var {
                    Some(v) => shown(v, row_codes[i]),
                    None => format!("Row {}", i + 1),
                })
                .collect(),
            row_ids: (0..n).map(|i| row_codes.get(i).copied()).collect(),
            sources: (0..sources)
                .map(|k| match &source_var {
                    Some(v) => shown(v, source_codes[k]),
                    None => format!("Source {}", k + 1),
                })
                .collect(),
            source_ids: (0..sources).map(|k| source_codes.get(k).copied()).collect(),
            first_case,
        };
        let columns: Vec<String> = variables.iter().map(|v| v.name().to_string()).collect();
        let mut entries = Vec::new();
        for (k, block) in cells.iter().enumerate() {
            for (i, row) in block.iter().enumerate() {
                for (j, cell) in row.iter().enumerate() {
                    if let Some((delta, w)) = cell
                        && *w > 0.0
                    {
                        entries.push(Entry {
                            k,
                            i,
                            j,
                            delta: *delta,
                            w: *w,
                        });
                    }
                }
            }
        }
        if entries.is_empty() {
            return Err(Error("PREFSCAL: every proximity is missing".into()));
        }
        let kind = match command.transformation {
            PrefscalTransformation::None { intercept: false } => Kind::None,
            PrefscalTransformation::None { intercept: true } => Kind::NoneIntercept,
            PrefscalTransformation::Linear { intercept: false } => Kind::Linear,
            PrefscalTransformation::Linear { intercept: true } => Kind::LinearIntercept,
            PrefscalTransformation::Ordinal { untie } => Kind::Ordinal { untie },
            PrefscalTransformation::Smooth { untie } => {
                warnings.push(
                    "PREFSCAL: TRANSFORMATION=SMOOTH is read but the smoothing is not applied; \
                     the run uses the ordinal transformation with the same tie handling"
                        .to_string(),
                );
                Kind::Ordinal { untie }
            }
            PrefscalTransformation::Spline {
                intercept,
                degree,
                inknot,
            } => Kind::Spline {
                intercept,
                degree,
                inknot,
            },
        };
        let groups: Vec<Vec<usize>> = match command.condition {
            PrefscalCondition::Row => {
                let mut g = vec![Vec::new(); sources * n];
                entries
                    .iter()
                    .enumerate()
                    .for_each(|(t, e)| g[e.k * n + e.i].push(t));
                g
            }
            PrefscalCondition::Matrix => {
                let mut g = vec![Vec::new(); sources];
                entries.iter().enumerate().for_each(|(t, e)| g[e.k].push(t));
                g
            }
            PrefscalCondition::Unconditional => vec![(0..entries.len()).collect()],
        };
        let problem = Unfolding::new(
            (n, m, sources),
            entries,
            groups,
            kind,
            (command.lambda, command.omega),
            command.similarities,
        );
        let negative = problem.entries.iter().any(|e| e.delta < 0.0);
        let no_intercept = matches!(
            kind,
            Kind::None
                | Kind::Linear
                | Kind::Spline {
                    intercept: false,
                    ..
                }
        );
        if negative && no_intercept {
            return Err(Error(
                "PREFSCAL: this transformation needs proximities that are not negative; add INTERCEPT"
                    .into(),
            ));
        }
        if !problem.connected() {
            return Err(Error(
                "PREFSCAL: the proximities that are not missing leave the objects in separate groups"
                    .into(),
            ));
        }
        let (min_dim, max_dim) = command.dimensions;
        if max_dim > n + m - 1 {
            return Err(Error(format!(
                "PREFSCAL: {max_dim} dimensions need at least {} objects, but there are {}",
                max_dim + 1,
                n + m
            )));
        }
        let mut model = match command.model {
            ScalingModel::Identity => Model::Identity,
            ScalingModel::Weighted => Model::Weighted,
            _ => Model::Generalized,
        };
        if sources == 1 && model != Model::Identity {
            warnings.push("PREFSCAL: one source only, so the identity model is used".to_string());
            model = Model::Identity;
        }
        if data.split_file().is_some() {
            warnings
                .push("PREFSCAL does not use SPLIT FILE: it analyzes all cases together".into());
        }
        let restricted: Option<Vec<Vec<Option<f64>>>> =
            match (&command.restrict_rows, &command.restrict_columns) {
                (None, None) => None,
                (rows, columns) => {
                    if model != Model::Identity {
                        return Err(Error(
                            "PREFSCAL: RESTRICTIONS go with MODEL=IDENTITY only".into(),
                        ));
                    }
                    let mut fixed = match rows {
                        Some(f) => self.file_coordinates("PREFSCAL", f, n, max_dim, "row")?,
                        None => vec![vec![None; max_dim]; n],
                    };
                    fixed.extend(match columns {
                        Some(f) => self.file_coordinates("PREFSCAL", f, m, max_dim, "column")?,
                        None => vec![vec![None; max_dim]; m],
                    });
                    Some(fixed)
                }
            };
        let file_start: Option<Matrix> = match &command.initial {
            PrefscalStart::File(f) => {
                let given = self.file_coordinates("PREFSCAL", f, n + m, max_dim, "initial")?;
                if given.iter().flatten().any(Option::is_none) {
                    return Err(Error(format!(
                        "PREFSCAL: the initial configuration in {} has missing values",
                        f.file
                    )));
                }
                Some(
                    given
                        .into_iter()
                        .map(|r| r.into_iter().flatten().collect())
                        .collect(),
                )
            }
            _ => None,
        };
        let criteria = Criteria {
            max_iterations: command.max_iterations,
            diff_stress: command.diff_stress,
            min_stress: command.min_stress,
            acceleration: false,
        };
        let start_gamma = problem.start_gamma();
        let mut dims: Vec<usize> = (min_dim..=max_dim).rev().collect();
        if let PrefscalStart::Random(_) = command.initial
            && min_dim < max_dim
        {
            warnings.push(format!(
                "PREFSCAL: random starts do not reduce dimensions; only {min_dim} dimensions are computed"
            ));
            dims = vec![min_dim];
        }
        let mut random: Vec<[f64; 3]> = Vec::new();
        let mut initial: Option<Matrix> = None;
        let mut solutions: Vec<(usize, Fit)> = Vec::new();
        for &p in &dims {
            // the fixed coordinates of the first p dimensions
            let pinned: Option<Vec<Vec<Option<f64>>>> = restricted
                .as_ref()
                .map(|f| f.iter().map(|r| r[..p].to_vec()).collect());
            let fit = match solutions.last() {
                Some((_, previous)) => {
                    let (z, a) = match restricted {
                        // a restricted space keeps its axes: the last dimension goes
                        Some(_) => (
                            previous.z.iter().map(|r| r[..p].to_vec()).collect(),
                            vec![mds::identity(p); sources],
                        ),
                        None => {
                            let as_solution = mds::Solution {
                                z: previous.z.clone(),
                                a: previous.a.clone(),
                                dhat: Vec::new(),
                                history: Vec::new(),
                                variables: None,
                            };
                            mds::reduce(model, &as_solution)
                        }
                    };
                    problem.iterate(
                        model,
                        &criteria,
                        z,
                        a,
                        start_gamma.clone(),
                        pinned.as_deref(),
                    )
                }
                None => {
                    let a = vec![mds::identity(p); sources];
                    let mut starts: Vec<Matrix> = match &command.initial {
                        PrefscalStart::Classical { spearman } => {
                            vec![problem.classical(*spearman, p)]
                        }
                        PrefscalStart::RossCliff => vec![problem.ross_cliff(p)],
                        PrefscalStart::Correspondence => vec![problem.correspondence(p)],
                        PrefscalStart::Centroids(c) => vec![problem.centroids((*c).min(m), p)],
                        PrefscalStart::Random(count) => (0..*count)
                            .map(|_| {
                                (0..n + m)
                                    .map(|_| (0..p).map(|_| self.random()).collect())
                                    .collect()
                            })
                            .collect(),
                        PrefscalStart::File(_) => file_start.clone().into_iter().collect(),
                    };
                    let mut best: Option<Fit> = None;
                    for z in starts.drain(..) {
                        let fit = problem.iterate(
                            model,
                            &criteria,
                            z,
                            a.clone(),
                            start_gamma.clone(),
                            pinned.as_deref(),
                        );
                        if matches!(command.initial, PrefscalStart::Random(_)) {
                            random.push(fit.last());
                        }
                        if best.as_ref().is_none_or(|b| fit.last()[0] < b.last()[0]) {
                            best = Some(fit);
                        }
                    }
                    let best = best.expect("at least one start");
                    initial = Some(best.start.clone());
                    best
                }
            };
            let solution = mds::Solution {
                z: fit.z.clone(),
                a: fit.a.clone(),
                dhat: Vec::new(),
                history: Vec::new(),
                variables: None,
            };
            // principal axes would move the fixed coordinates
            let done = match restricted {
                Some(_) => solution,
                None => mds::finish(model, solution),
            };
            solutions.push((
                p,
                Fit {
                    z: done.z,
                    a: done.a,
                    ..fit
                },
            ));
        }
        let several = solutions.len() > 1;
        let print = &command.print;
        let mut tables = vec![summary(&problem)];
        let objects: Vec<String> = layout.rows.iter().chain(&columns).cloned().collect();
        if print.input {
            for k in 0..sources {
                let values = rectangle(&problem, k, &|t| problem.entries[t].delta);
                tables.push(rect(
                    titled("Proximities", &layout.sources[k], sources),
                    &layout.rows,
                    &columns,
                    &values,
                ));
            }
        }
        if print.multiple && !random.is_empty() {
            tables.push(Table {
                title: "Random Starts".into(),
                columns: ["Start", "Penalized Stress", "Stress Part", "Penalty Part"]
                    .map(String::from)
                    .to_vec(),
                rows: random
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let mut row = vec![Cell::Number((i + 1) as f64)];
                        row.extend(r.iter().map(|v| Cell::Number(*v)));
                        row
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        let dims_cols =
            |p: usize| -> Vec<String> { (1..=p).map(|d| format!("Dimension {d}")).collect() };
        if print.initial
            && let Some(z) = &initial
        {
            tables.push(coordinates(
                "Initial Common Space".into(),
                &objects,
                z,
                &dims_cols(dims[0]),
            ));
        }
        for (p, fit) in &solutions {
            let suffix = |title: &str| match several {
                true => format!("{title} ({p} Dimensions)"),
                false => title.to_string(),
            };
            let dims = dims_cols(*p);
            let x: Vec<Matrix> = fit.a.iter().map(|a| multiply(&fit.z, a)).collect();
            if print.history {
                tables.push(Table {
                    title: suffix("Iteration History"),
                    columns: [
                        "Iteration",
                        "Penalized Stress",
                        "Stress Part",
                        "Penalty Part",
                    ]
                    .map(String::from)
                    .to_vec(),
                    rows: fit
                        .history
                        .iter()
                        .enumerate()
                        .map(|(i, h)| {
                            vec![
                                Cell::Number(i as f64),
                                Cell::Number(h[0]),
                                Cell::Number(h[0] / h[2]),
                                Cell::Number(h[2]),
                            ]
                        })
                        .collect(),
                    row_headers: 1,
                    ..Table::default()
                });
            }
            let measures = problem.measures(fit);
            let shown_rows = if print.measures { MEASURES.len() } else { 3 };
            let mut rows = vec![vec![
                text("Iterations"),
                Cell::Number((fit.history.len() - 1) as f64),
            ]];
            rows.extend(
                MEASURES
                    .iter()
                    .zip(measures)
                    .take(shown_rows)
                    .map(|(name, v)| vec![text(*name), number(Some(v))]),
            );
            tables.push(Table {
                title: suffix("Measures"),
                columns: vec![String::new(), "Value".into()],
                rows,
                row_headers: 1,
                ..Table::default()
            });
            if print.decomposition {
                tables.push(decomposition(
                    &problem,
                    fit,
                    &objects,
                    &layout.sources,
                    suffix("Decomposition of Normalized Stress"),
                ));
            }
            if print.common {
                tables.push(coordinates(
                    suffix("Final Common Space"),
                    &objects,
                    &fit.z,
                    &dims,
                ));
            }
            if print.weights && model != Model::Identity {
                let after = match several {
                    true => format!(" ({p} Dimensions)"),
                    false => String::new(),
                };
                tables.push(space_weights(model, &after, &layout.sources, &fit.a, &dims));
            }
            if print.individual && model != Model::Identity {
                for (k, name) in layout.sources.iter().enumerate() {
                    tables.push(coordinates(
                        suffix(&format!("Individual Space Coordinates: {name}")),
                        &objects,
                        &x[k],
                        &dims,
                    ));
                }
            }
            if print.distances {
                let mut spaces: Vec<(String, &Matrix)> = Vec::new();
                if print.common {
                    spaces.push((suffix("Distances"), &fit.z));
                }
                if print.individual && model != Model::Identity {
                    for (k, name) in layout.sources.iter().enumerate() {
                        spaces.push((suffix(&format!("Distances: {name}")), &x[k]));
                    }
                }
                for (title, space) in spaces {
                    let d = mds::distances(space);
                    let values: Matrix = (0..n)
                        .map(|i| (0..m).map(|j| d[i][n + j]).collect())
                        .collect();
                    let values: Vec<Vec<Option<f64>>> = values
                        .into_iter()
                        .map(|r| r.into_iter().map(Some).collect())
                        .collect();
                    tables.push(rect(title, &layout.rows, &columns, &values));
                }
            }
            if print.transformations {
                for k in 0..sources {
                    let values = rectangle(&problem, k, &|t| fit.gamma[t]);
                    tables.push(rect(
                        suffix(&titled(
                            "Transformed Proximities",
                            &layout.sources[k],
                            sources,
                        )),
                        &layout.rows,
                        &columns,
                        &values,
                    ));
                }
            }
        }
        let (p, fit) = solutions.last().expect("at least one dimensionality");
        let p = *p;
        let x: Vec<Matrix> = fit.a.iter().map(|a| multiply(&fit.z, a)).collect();
        let d = problem.distances(&x);
        let plot = &command.plot;
        let mut charts: Vec<Chart> = Vec::new();
        let row_groups = self.row_groups(command, &layout)?;
        let joint = |title: String, z: &Matrix| {
            joint_chart(title, &row_groups, &layout.rows, &columns, z, p)
        };
        if plot.multiple && random.len() > 1 {
            let mut chart = Chart::lines(
                "Random Starts".into(),
                Axis::titled("Start"),
                Axis::titled("Penalized Stress"),
                (1..=random.len()).map(|s| category(s as f64)).collect(),
                vec![
                    series("Stress Part", random.iter().map(|r| r[0] / r[2]).collect()),
                    series("Penalized Stress", random.iter().map(|r| r[0]).collect()),
                ],
            );
            chart.kind = ChartKind::Bar;
            charts.push(chart);
        }
        if plot.initial
            && let Some(z) = &initial
        {
            charts.push(joint("Initial Common Space".into(), z));
        }
        if plot.stress && several {
            let mut ordered: Vec<(usize, f64)> =
                solutions.iter().map(|(p, f)| (*p, f.last()[0])).collect();
            ordered.sort_by_key(|(p, _)| *p);
            charts.push(Chart::lines(
                "Scree Plot".into(),
                Axis::titled("Dimensionality"),
                Axis::titled("Penalized Stress"),
                ordered.iter().map(|(p, _)| category(*p as f64)).collect(),
                vec![series(
                    "Penalized Stress",
                    ordered.iter().map(|(_, s)| *s).collect(),
                )],
            ));
        }
        if plot.common {
            charts.push(joint("Common Space".into(), &fit.z));
        }
        if plot.weights && model == Model::Weighted && p >= 2 {
            let points = layout
                .sources
                .iter()
                .zip(&fit.a)
                .map(|(name, a)| point(a[0][0], a[1][1], Some(name.clone())))
                .collect();
            charts.push(scatter(
                "Dimension Weights",
                "Dimension 1",
                "Dimension 2",
                vec![PointGroup {
                    name: "Sources".into(),
                    points,
                }],
            ));
        }
        let pick_sources = |list: &ValueList| -> Vec<usize> {
            (0..sources)
                .filter(|&k| picked(list, layout.source_ids[k], k + 1))
                .collect()
        };
        if model != Model::Identity {
            for list in plot.individual.iter().flatten() {
                for k in pick_sources(list) {
                    charts.push(joint(
                        format!("Individual Space: {}", layout.sources[k]),
                        &x[k],
                    ));
                }
            }
        }
        // the transformation lists a value list picks: rows, sources or all
        let pick_lists = |list: &ValueList| -> Vec<(String, Vec<usize>)> {
            let in_list = |e: &Entry| match command.condition {
                PrefscalCondition::Row => picked(list, layout.row_ids[e.i], e.i + 1),
                PrefscalCondition::Matrix => picked(list, layout.source_ids[e.k], e.k + 1),
                PrefscalCondition::Unconditional => true,
            };
            problem
                .lists
                .iter()
                .filter(|l| in_list(&problem.entries[l.members[0]]))
                .map(|l| {
                    let e = problem.entries[l.members[0]];
                    let name = match command.condition {
                        PrefscalCondition::Row if sources > 1 => {
                            format!("{}, {}", layout.rows[e.i], layout.sources[e.k])
                        }
                        PrefscalCondition::Row => layout.rows[e.i].clone(),
                        PrefscalCondition::Matrix => layout.sources[e.k].clone(),
                        PrefscalCondition::Unconditional => "All".into(),
                    };
                    (name, l.members.clone())
                })
                .collect()
        };
        let group = |name: String, members: &[usize], f: &dyn Fn(usize) -> (f64, f64)| PointGroup {
            name,
            points: members
                .iter()
                .map(|&t| {
                    let (a, b) = f(t);
                    point(a, b, None)
                })
                .collect(),
        };
        let delta = |t: usize| problem.entries[t].delta;
        for list in plot.transformations.iter().flatten() {
            let groups = pick_lists(list)
                .into_iter()
                .map(|(name, members)| group(name, &members, &|t| (delta(t), fit.gamma[t])))
                .collect();
            charts.push(scatter(
                "Transformation Plot",
                "Proximities",
                "Transformed Proximities",
                groups,
            ));
        }
        for list in plot.shepard.iter().flatten() {
            let members: Vec<usize> = pick_lists(list).into_iter().flat_map(|(_, m)| m).collect();
            charts.push(scatter(
                "Shepard Plot",
                "Proximities",
                "Distances and Transformed Proximities",
                vec![
                    group("Distances".into(), &members, &|t| (delta(t), d[t])),
                    group("Transformed Proximities".into(), &members, &|t| {
                        (delta(t), fit.gamma[t])
                    }),
                ],
            ));
        }
        for (asked, title, y_title, residual) in [
            (&plot.fit, "Fit Plot", "Distances", false),
            (&plot.residuals, "Residuals Plot", "Residuals", true),
        ] {
            for list in asked.iter().flatten() {
                let groups = pick_sources(list)
                    .into_iter()
                    .map(|k| {
                        let members: Vec<usize> = (0..problem.entries.len())
                            .filter(|&t| problem.entries[t].k == k)
                            .collect();
                        group(layout.sources[k].clone(), &members, &|t| {
                            let g = fit.gamma[t];
                            (g, if residual { g - d[t] } else { d[t] })
                        })
                    })
                    .collect();
                charts.push(scatter(title, "Transformed Proximities", y_title, groups));
            }
        }
        self.prefscal_outfiles(command, &problem, fit, model, p, &x, &mut warnings)?;
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    /// The row objects in groups by the values of the OPTIONS variables.
    fn row_groups(
        &self,
        command: &Prefscal,
        layout: &Layout,
    ) -> Result<Vec<(String, Vec<usize>)>, Error> {
        let data = self.active()?;
        let vars: Vec<Variable> = [&command.markers, &command.colors]
            .into_iter()
            .flatten()
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        if vars.is_empty() {
            return Ok(vec![("Rows".into(), (0..layout.rows.len()).collect())]);
        }
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, &case) in layout.first_case.iter().enumerate() {
            let key = vars
                .iter()
                .map(|v| match (v.values(), is_valid(v, case, false)) {
                    (Values::Numeric(values), true) => {
                        values.get(case).map_or("Missing".into(), |x| shown(v, x))
                    }
                    (Values::String(values), true) => values.get(case).trim_end().to_string(),
                    _ => "Missing".into(),
                })
                .collect::<Vec<String>>()
                .join(", ");
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(i),
                None => groups.push((key, vec![i])),
            }
        }
        Ok(groups)
    }

    #[allow(clippy::too_many_arguments)]
    fn prefscal_outfiles(
        &mut self,
        command: &Prefscal,
        problem: &Unfolding,
        fit: &Fit,
        model: Model,
        p: usize,
        x: &[Matrix],
        warnings: &mut Vec<String>,
    ) -> Result<(), Error> {
        let (n, m) = (problem.n, problem.m);
        let big = n + m;
        let out = &command.outfile;
        let columns = |rows: &[Vec<Option<f64>>], names: &[String]| -> Result<Dataset, Error> {
            let mut data = Dataset::new(rows.len());
            for (d, name) in names.iter().enumerate() {
                data.set_numeric_values(name, rows.iter().map(|r| r[d]).collect())?;
            }
            Ok(data)
        };
        let dim_names: Vec<String> = (1..=p).map(|d| format!("DIM_{d}")).collect();
        if let Some(file) = &out.common {
            let rows: Vec<Vec<Option<f64>>> = fit
                .z
                .iter()
                .map(|r| r.iter().map(|v| Some(*v)).collect())
                .collect();
            let data = columns(&rows, &dim_names)?;
            self.write_data_file(file, data)?;
        }
        if let Some(file) = &out.weights {
            if model == Model::Identity {
                warnings.push("PREFSCAL: the identity model has no space weights to save".into());
            } else {
                let rows: Vec<Vec<Option<f64>>> = match model {
                    Model::Weighted => fit
                        .a
                        .iter()
                        .map(|a| (0..p).map(|d| Some(a[d][d])).collect())
                        .collect(),
                    _ => fit
                        .a
                        .iter()
                        .flat_map(|a| a.iter().map(|r| r.iter().map(|v| Some(*v)).collect()))
                        .collect(),
                };
                let data = columns(&rows, &dim_names)?;
                self.write_data_file(file, data)?;
            }
        }
        let object_names: Vec<String> = (1..=n)
            .map(|i| format!("ROW_{i}"))
            .chain((1..=m).map(|j| format!("COL_{j}")))
            .collect();
        if let Some(file) = &out.distances {
            let rows: Vec<Vec<Option<f64>>> = x
                .iter()
                .flat_map(|xk| {
                    mds::distances(xk)
                        .into_iter()
                        .map(|r| r.into_iter().map(Some).collect())
                })
                .collect();
            let data = columns(&rows, &object_names)?;
            self.write_data_file(file, data)?;
        }
        if let Some(file) = &out.transformations {
            let mut rows = vec![vec![None; big]; big * problem.sources];
            for (e, g) in problem.entries.iter().zip(&fit.gamma) {
                rows[e.k * big + e.i][n + e.j] = Some(*g);
                rows[e.k * big + n + e.j][e.i] = Some(*g);
            }
            let data = columns(&rows, &object_names)?;
            self.write_data_file(file, data)?;
        }
        Ok(())
    }
}

fn summary(problem: &Unfolding) -> Table {
    let total = problem.n * problem.m * problem.sources;
    let active = problem.entries.len();
    let row = |label: &str, v: usize| vec![text(label), Cell::Number(v as f64)];
    Table {
        title: "Case Processing Summary".into(),
        columns: vec![String::new(), "N".into()],
        rows: vec![
            row("Rows", problem.n),
            row("Columns", problem.m),
            row("Sources", problem.sources),
            row("Proximities", total),
            row("Missing Proximities", total - active),
            row("Active Proximities", active),
        ],
        row_headers: 1,
        ..Table::default()
    }
}

/// The values `f` gives each entry of source k, rows by columns, None where missing.
fn rectangle(problem: &Unfolding, k: usize, f: &dyn Fn(usize) -> f64) -> Vec<Vec<Option<f64>>> {
    let mut out = vec![vec![None; problem.m]; problem.n];
    for (t, e) in problem.entries.iter().enumerate().filter(|(_, e)| e.k == k) {
        out[e.i][e.j] = Some(f(t));
    }
    out
}

/// A rows by columns table.
fn rect(title: String, rows: &[String], columns: &[String], values: &[Vec<Option<f64>>]) -> Table {
    let mut header = vec![String::new()];
    header.extend(columns.iter().cloned());
    Table {
        title,
        columns: header,
        rows: rows
            .iter()
            .zip(values)
            .map(|(name, r)| {
                let mut row = vec![text(name.clone())];
                row.extend(r.iter().map(|v| v.map_or(Cell::Empty, |x| number(Some(x)))));
                row
            })
            .collect(),
        row_headers: 1,
        ..Table::default()
    }
}

/// Every object's share of the normalized stress by source: half of each of its
/// proximities' shares, so that the totals add up to the normalized stress.
fn decomposition(
    problem: &Unfolding,
    fit: &Fit,
    objects: &[String],
    sources: &[String],
    title: String,
) -> Table {
    let shares = problem.shares(fit);
    let mut cells = zeros(objects.len(), problem.sources);
    for (e, s) in problem.entries.iter().zip(&shares) {
        cells[e.i][e.k] += s / 2.0;
        cells[problem.n + e.j][e.k] += s / 2.0;
    }
    let mut columns = vec![String::new()];
    columns.extend(sources.iter().cloned());
    columns.push("Total".into());
    let mut rows: Vec<Vec<Cell>> = objects
        .iter()
        .zip(&cells)
        .map(|(name, r)| {
            let mut row = vec![text(name.clone())];
            row.extend(r.iter().map(|v| Cell::Number(*v)));
            row.push(Cell::Number(r.iter().sum()));
            row
        })
        .collect();
    let mut total = vec![text("Total")];
    total.extend((0..problem.sources).map(|k| Cell::Number(cells.iter().map(|r| r[k]).sum())));
    total.push(Cell::Number(cells.iter().flatten().sum()));
    rows.push(total);
    Table {
        title,
        columns,
        rows,
        row_headers: 1,
        ..Table::default()
    }
}

fn point(x: f64, y: f64, label: Option<String>) -> Point {
    Point {
        x,
        y,
        z: None,
        label,
    }
}

fn category(v: f64) -> Category {
    Category {
        label: v.to_string(),
        value: Cell::Number(v),
        missing: false,
        bin: None,
    }
}

fn series(name: &str, values: Vec<f64>) -> Series {
    Series {
        name: name.into(),
        statistic: Statistic::Value,
        variable: None,
        value: Cell::Empty,
        values,
    }
}

fn scatter(title: &str, x: &str, y: &str, groups: Vec<PointGroup>) -> Chart {
    Chart::plot(
        ChartKind::Scatter,
        title.into(),
        Axis::titled(x),
        Axis::titled(y),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    )
}

/// The joint plot of rows and columns: a scatter in one or two dimensions, a scatterplot
/// matrix in more.
fn joint_chart(
    title: String,
    row_groups: &[(String, Vec<usize>)],
    rows: &[String],
    columns: &[String],
    z: &Matrix,
    p: usize,
) -> Chart {
    let mut sets: Vec<(String, Vec<(String, usize)>)> = row_groups
        .iter()
        .map(|(name, members)| {
            (
                name.clone(),
                members.iter().map(|&i| (rows[i].clone(), i)).collect(),
            )
        })
        .collect();
    let n = rows.len();
    sets.push((
        "Columns".into(),
        columns
            .iter()
            .enumerate()
            .map(|(j, c)| (c.clone(), n + j))
            .collect(),
    ));
    let axis = |d: usize| Axis::titled(format!("Dimension {d}"));
    if p >= 3 {
        return Chart::plot(
            ChartKind::Scatter,
            title,
            Axis::titled(String::new()),
            Axis::titled(String::new()),
            Plot::Matrix {
                variables: (1..=p).map(axis).collect(),
                groups: sets
                    .into_iter()
                    .map(|(name, members)| CaseGroup {
                        name,
                        cases: members
                            .into_iter()
                            .map(|(label, o)| CaseValues {
                                values: z[o].clone(),
                                label: Some(label),
                            })
                            .collect(),
                    })
                    .collect(),
            },
        );
    }
    let groups = sets
        .into_iter()
        .map(|(name, members)| PointGroup {
            name,
            points: members
                .into_iter()
                .map(|(label, o)| point(z[o][0], z[o].get(1).copied().unwrap_or(0.0), Some(label)))
                .collect(),
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
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    )
}
