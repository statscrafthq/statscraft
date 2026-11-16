//! DETECTANOMALY: unusual cases measured against peer groups found by two-step clustering.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - The variables are CATEGORICAL and SCALE when either is given, else every variable but ID
//!   and EXCEPT, a string, nominal or ordinal one categorical. EXCEPT is applied first; a
//!   variable then on both lists, or the ID on a list, is an error.
//! - Training cases: those without a continuous value beyond 1e150 in size and, unless
//!   HANDLEMISSING APPLY=YES, without missing values. Variables constant or empty over them
//!   are dropped. With APPLY=YES a missing continuous value becomes the variable's mean and
//!   the missing values of a categorical one form one more category; CREATEMISPROPVAR=YES
//!   (only with APPLY=YES) adds the share of missing values of each case as a continuous
//!   variable.
//! - Peer groups: two-step clustering with the likelihood distance, each variance adjusted by
//!   the variable's variance over the training cases divided by MLWEIGHT, noise handling at
//!   25 percent and the count chosen by BIC between MINNUMPEERS and MAXNUMPEERS (equal: fixed).
//!   Each case goes to its closest peer group.
//! - A case's deviation of variable k is the variable's share in the likelihood distance
//!   between its peer group and the case; the group deviation is their sum. The anomaly index
//!   divides it by the mean group deviation of the peer group, a variable's impact divides its
//!   deviation by the group deviation. When both are 0 the index is the mean group size over
//!   the size of the group and each impact 1 over the number of variables; when only the mean
//!   is 0, the divisor is the size-weighted mean of the other groups' means, or if that is 0
//!   too, the index is again mean size over size.
//! - The anomaly list holds the cases of largest index, the first PCTANOMALOUSCASES percent
//!   (rounded down) or NUMANOMALOUSCASES of them, and of those only the ones with an index of
//!   at least ANOMALYCUTPOINT. A case's reasons are its NUMREASONS variables of largest impact.
//! - WEIGHT and SPLIT FILE are not used, with a warning.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::twostep::{
    Cf, Model, Settings, agglomerate, auto_count, category_codes, cf_json, clusters_at,
    distribution, free_name, key_json, mean_sd, number_by_first, numeric_value, plain_table,
    precluster, variance,
};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{AnomalySave, Detectanomaly, VarItem};

/// One analysis variable after missing value handling.
enum Input<'a> {
    Scale {
        variable: Variable<'a>,
        /// Over the cases of the active dataset; None where missing.
        values: Vec<Option<f64>>,
    },
    Categorical {
        variable: Variable<'a>,
        keys: Vec<Option<Key>>,
    },
    /// The share of missing values of each case.
    MissingShare(Vec<Option<f64>>),
}

impl Input<'_> {
    fn name(&self) -> String {
        match self {
            Input::Scale { variable, .. } | Input::Categorical { variable, .. } => {
                variable.name().to_string()
            }
            Input::MissingShare(_) => "Missing Proportion".into(),
        }
    }

    fn missing(&self, case: usize) -> bool {
        match self {
            Input::Scale { values, .. } | Input::MissingShare(values) => values[case].is_none(),
            Input::Categorical { keys, .. } => keys[case].is_none(),
        }
    }
}

/// A number as saved text: whole numbers without decimals.
fn plain_number(x: f64) -> String {
    format!("{x}")
}

impl Engine {
    pub(crate) fn detectanomaly(&mut self, command: &Detectanomaly) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut warnings: Vec<String> = Vec::new();
        if case_weights(data)?.is_some() {
            warnings.push("DETECTANOMALY does not use the weight variable".into());
        }
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let id = command.id.as_ref().map(|n| data.variable(n)).transpose()?;
        let id_name = id.as_ref().map(|v| v.name().to_string());
        let except = self.expand(&command.except)?;
        let excepted = |n: &String| except.iter().any(|e| same(e, n));
        let unique = |names: Vec<String>| {
            let mut out: Vec<String> = Vec::new();
            for n in names {
                if !out.iter().any(|o| same(o, &n)) && !excepted(&n) {
                    out.push(n);
                }
            }
            out
        };
        let (categorical, scale) = if command.categorical.is_some() || command.scale.is_some() {
            let list = |items: &Option<Vec<VarItem>>| -> Result<Vec<String>, Error> {
                Ok(unique(self.expand(items.as_deref().unwrap_or(&[]))?))
            };
            let (categorical, scale) = (list(&command.categorical)?, list(&command.scale)?);
            if let Some(both) = categorical
                .iter()
                .find(|c| scale.iter().any(|s| same(s, c)))
            {
                return Err(Error(format!(
                    "DETECTANOMALY: {both} is both categorical and scale"
                )));
            }
            if let Some(id) = &id_name
                && categorical.iter().chain(&scale).any(|n| same(n, id))
            {
                return Err(Error(format!(
                    "DETECTANOMALY: the ID variable {id} cannot also be an analysis variable"
                )));
            }
            (categorical, scale)
        } else {
            let scale_min = self.scale_min();
            let (mut categorical, mut scale) = (Vec::new(), Vec::new());
            for index in 0..data.variable_count() {
                let variable = data.variable_at(index);
                let name = variable.name().to_string();
                if excepted(&name) || id_name.as_ref().is_some_and(|i| same(i, &name)) {
                    continue;
                }
                let measure = match variable.measure() {
                    Measure::Unknown => default_measure(data, index, scale_min),
                    known => known,
                };
                match (variable.is_string(), measure) {
                    (false, Measure::Scale) => scale.push(name),
                    _ => categorical.push(name),
                }
            }
            (categorical, scale)
        };
        let cases = data.case_count();
        let mut inputs: Vec<Input<'_>> = Vec::new();
        for name in &scale {
            let variable = data.variable(name)?;
            if variable.is_string() {
                return Err(Error(format!(
                    "DETECTANOMALY: scale variable {name} is a string variable"
                )));
            }
            let values = (0..cases)
                .map(|c| numeric_value(&variable, c, false))
                .collect();
            inputs.push(Input::Scale { variable, values });
        }
        for name in &categorical {
            let variable = data.variable(name)?;
            let keys = categories(&variable, false);
            inputs.push(Input::Categorical { variable, keys });
        }
        if inputs.is_empty() {
            return Err(Error("DETECTANOMALY has no variables to analyze".into()));
        }

        // the training cases
        let huge = |case: usize| {
            inputs.iter().any(|i| match i {
                Input::Scale { values, .. } => values[case].is_some_and(|v| v.abs() > 1e150),
                _ => false,
            })
        };
        let training: Vec<usize> = (0..cases)
            .filter(|&c| !huge(c) && (command.apply || inputs.iter().all(|i| !i.missing(c))))
            .collect();
        inputs.retain(|input| {
            let distinct = match input {
                Input::Scale { values, .. } => {
                    let mut v: Vec<f64> = training.iter().filter_map(|&c| values[c]).collect();
                    v.sort_by(f64::total_cmp);
                    v.dedup();
                    v.len()
                }
                Input::Categorical { keys, .. } => {
                    let picked: Vec<Option<Key>> =
                        training.iter().map(|&c| keys[c].clone()).collect();
                    category_codes(&picked).0.len()
                }
                Input::MissingShare(_) => 2,
            };
            if distinct < 2 {
                warnings.push(format!(
                    "DETECTANOMALY leaves out {}: it is constant or missing in every case",
                    input.name()
                ));
            }
            distinct >= 2
        });
        if inputs.is_empty() {
            return Err(Error(
                "DETECTANOMALY has no variable left that varies".into(),
            ));
        }
        if training.len() < 2 {
            return Err(Error(format!(
                "DETECTANOMALY needs at least 2 cases to analyze, found {}",
                training.len()
            )));
        }
        if command.missing_proportion {
            if !command.apply {
                warnings.push(
                    "DETECTANOMALY: CREATEMISPROPVAR needs HANDLEMISSING APPLY=YES; it is turned off"
                        .into(),
                );
            } else {
                let k = inputs.len() as f64;
                let share: Vec<Option<f64>> = (0..cases)
                    .map(|c| Some(inputs.iter().filter(|i| i.missing(c)).count() as f64 / k))
                    .collect();
                let first = share[training[0]];
                if training.iter().all(|&c| share[c] == first) {
                    warnings.push(
                        "DETECTANOMALY leaves out the missing proportion: it is the same in every case"
                            .into(),
                    );
                } else {
                    // continuous inputs come first
                    let at = inputs
                        .iter()
                        .position(|i| matches!(i, Input::Categorical { .. }))
                        .unwrap_or(inputs.len());
                    inputs.insert(at, Input::MissingShare(share));
                }
            }
        }

        // the processed values of the training cases
        let n = training.len();
        let mut continuous: Vec<Vec<f64>> = Vec::new();
        let mut coded: Vec<(Vec<Key>, Vec<usize>, usize)> = Vec::new();
        // the mean of each continuous input, which stands in for its missing values
        let mut means: Vec<f64> = Vec::new();
        for input in &inputs {
            match input {
                Input::Scale { values, .. } | Input::MissingShare(values) => {
                    let valid: Vec<f64> = training.iter().filter_map(|&c| values[c]).collect();
                    let mean = valid.iter().sum::<f64>() / valid.len() as f64;
                    means.push(mean);
                    continuous.push(
                        training
                            .iter()
                            .map(|&c| values[c].unwrap_or(mean))
                            .collect(),
                    );
                }
                Input::Categorical { keys, .. } => {
                    let picked: Vec<Option<Key>> =
                        training.iter().map(|&c| keys[c].clone()).collect();
                    let (levels, codes) = category_codes(&picked);
                    let missing = levels.len();
                    let count = levels.len() + usize::from(codes.iter().any(Option::is_none));
                    let codes = codes.iter().map(|c| c.unwrap_or(missing)).collect();
                    coded.push((levels, codes, count));
                }
            }
        }
        let variances: Vec<f64> = continuous.iter().map(|v| variance(v)).collect();
        let model = Model {
            adjust: variances.iter().map(|v| v / command.ml_weight).collect(),
            euclidean: false,
        };
        let counts: Vec<usize> = coded.iter().map(|c| c.2).collect();
        let records: Vec<Cf> = (0..n)
            .map(|r| {
                let x: Vec<f64> = continuous.iter().map(|v| v[r]).collect();
                let c: Vec<usize> = coded.iter().map(|c| c.1[r]).collect();
                Cf::record(&x, &c, &counts)
            })
            .collect();
        let settings = Settings {
            threshold: 0.0,
            branch: 8,
            levels: 3,
            noise: 0.25,
        };
        let pre = precluster(&model, records.clone(), &settings);
        let subs = &pre.subclusters;
        let joins = agglomerate(&model, subs);
        let count = if command.min_peers == command.max_peers {
            command.min_peers
        } else {
            auto_count(&model, subs, &joins, command.max_peers, false)
                .1
                .max(command.min_peers)
        }
        .min(subs.len());
        if count < command.min_peers {
            warnings.push(format!(
                "DETECTANOMALY: the data support only {count} peer groups, fewer than MINNUMPEERS"
            ));
        }
        let clusters = clusters_at(subs, &joins, count);
        let closest: Vec<Option<usize>> = records
            .iter()
            .map(|r| {
                let mut best = (0, f64::INFINITY);
                for (j, c) in clusters.iter().enumerate() {
                    let d = model.distance(c, r);
                    if d < best.1 {
                        best = (j, d);
                    }
                }
                Some(best.0)
            })
            .collect();
        let (numbers, order) = number_by_first(&closest);
        let peer: Vec<usize> = numbers.iter().map(|p| p.expect("no noise") - 1).collect();
        let h = order.len();
        let mut groups: Vec<Option<Cf>> = vec![None; h];
        for (r, record) in records.iter().enumerate() {
            match &mut groups[peer[r]] {
                Some(cf) => cf.add(record),
                slot => *slot = Some(record.clone()),
            }
        }
        let groups: Vec<Cf> = groups.into_iter().map(|g| g.expect("members")).collect();
        let sizes: Vec<f64> = groups.iter().map(|g| g.n).collect();

        // deviations
        let v = inputs.len();
        let vdi: Vec<Vec<f64>> = (0..n)
            .map(|r| {
                (0..v)
                    .map(|k| model.term(k, &groups[peer[r]], &records[r]).max(0.0))
                    .collect()
            })
            .collect();
        let gdi: Vec<f64> = vdi.iter().map(|d| d.iter().sum()).collect();
        let mean_gdi: Vec<f64> = (0..h)
            .map(|g| {
                let members: Vec<f64> = (0..n).filter(|&r| peer[r] == g).map(|r| gdi[r]).collect();
                members.iter().sum::<f64>() / members.len() as f64
            })
            .collect();
        let mut identical = false;
        let mut index = vec![0.0; n];
        let mut impact = vec![vec![0.0; v]; n];
        for r in 0..n {
            let g = peer[r];
            let size_ratio = (n as f64 / h as f64) / sizes[g];
            if mean_gdi[g] > 0.0 {
                index[r] = gdi[r] / mean_gdi[g];
            } else if gdi[r] == 0.0 {
                index[r] = size_ratio;
            } else {
                identical = true;
                let others: f64 = (0..h)
                    .filter(|&s| s != g)
                    .map(|s| sizes[s] * mean_gdi[s])
                    .sum::<f64>()
                    / (n as f64 - sizes[g]);
                index[r] = match others > 0.0 {
                    true => gdi[r] / others,
                    false => size_ratio,
                };
            }
            impact[r] = match gdi[r] > 0.0 {
                true => vdi[r].iter().map(|d| d / gdi[r]).collect(),
                false => vec![1.0 / v as f64; v],
            };
        }
        if identical {
            warnings.push(
                "DETECTANOMALY: a case differs from a peer group made of identical cases".into(),
            );
        }
        let reasons = command.reasons.min(v);
        let ranked: Vec<Vec<usize>> = (0..n)
            .map(|r| {
                let mut ks: Vec<usize> = (0..v).collect();
                ks.sort_by(|&a, &b| vdi[r][b].total_cmp(&vdi[r][a]));
                ks.truncate(reasons);
                ks
            })
            .collect();
        let reason_of = |r: usize, reason: usize| ranked[r][reason];
        let mut by_index: Vec<usize> = (0..n).collect();
        by_index.sort_by(|&a, &b| index[b].total_cmp(&index[a]));
        let listed = command
            .count
            .unwrap_or((command.percent * n as f64 / 100.0 + 1e-9).floor() as usize)
            .min(n);
        let anomalies: Vec<usize> = by_index
            .into_iter()
            .take(listed)
            .filter(|&r| command.cutpoint.is_none_or(|cut| index[r] >= cut))
            .collect();

        // how values and norms show
        let shown = self.shown();
        let mut continuous_at = 0;
        let mut categorical_at = 0;
        // for each input: its column among continuous or coded
        let slots: Vec<usize> = inputs
            .iter()
            .map(|i| match i {
                Input::Categorical { .. } => {
                    categorical_at += 1;
                    categorical_at - 1
                }
                _ => {
                    continuous_at += 1;
                    continuous_at - 1
                }
            })
            .collect();
        let category_text = |k: usize, code: usize| -> Cell {
            let (levels, _, _) = &coded[slots[k]];
            match (&inputs[k], levels.get(code)) {
                (Input::Categorical { variable, .. }, Some(key)) => {
                    text(shown.value(variable, key))
                }
                _ => text("Missing"),
            }
        };
        let mode = |k: usize, cf: &Cf| -> (usize, f64) {
            let counts = &cf.counts[slots[k]];
            let mut best = (0, f64::NEG_INFINITY);
            for (c, &m) in counts.iter().enumerate() {
                if m > best.1 {
                    best = (c, m);
                }
            }
            best
        };
        let value_cell = |k: usize, r: usize| -> Cell {
            let case = training[r];
            match &inputs[k] {
                Input::Scale { values, .. } | Input::MissingShare(values) => number(values[case]),
                Input::Categorical { keys, variable } => match &keys[case] {
                    Some(key) => text(shown.value(variable, key)),
                    None => Cell::Empty,
                },
            }
        };
        let norm_cell = |k: usize, g: usize| -> Cell {
            match &inputs[k] {
                Input::Categorical { .. } => category_text(k, mode(k, &groups[g]).0),
                _ => number(Some(groups[g].mean[slots[k]])),
            }
        };
        let case_number = |r: usize| {
            let case = training[r];
            self.case_numbers.as_ref().map_or(case, |c| c[case]) + 1
        };
        let id_cell = |r: usize| -> Cell {
            let variable = id.as_ref().expect("ID given");
            match variable.values() {
                Values::Numeric(values) => number(values.get(training[r])),
                Values::String(values) => text(values.get(training[r]).trim_end()),
            }
        };
        let mut head = vec!["Case".to_string()];
        if let Some(id) = &id {
            head.push(shown.variable(id));
        }
        let lead = |r: usize| -> Vec<Cell> {
            let mut row = vec![Cell::Number(case_number(r) as f64)];
            if id.is_some() {
                row.push(id_cell(r));
            }
            row
        };
        let none_caption = |table: &mut Table| {
            if anomalies.is_empty() {
                table.caption = Some("No case meets the criteria of the anomaly list.".into());
            }
        };

        let mut tables = Vec::new();
        if command.print_cps {
            tables.push(distribution(
                "Case Processing Summary",
                "Peer ID",
                &sizes,
                0.0,
                cases as f64,
            ));
        }
        if command.print_list {
            let rows = anomalies
                .iter()
                .map(|&r| {
                    let mut row = lead(r);
                    row.push(Cell::Number(index[r]));
                    row
                })
                .collect();
            let mut columns = head.clone();
            columns.push("Anomaly Index".into());
            let mut table = plain_table("Anomaly Case Index List", 0, &columns, rows);
            none_caption(&mut table);
            tables.push(table);
            let rows = anomalies
                .iter()
                .map(|&r| {
                    let mut row = lead(r);
                    let size = sizes[peer[r]];
                    row.extend([
                        Cell::Number((peer[r] + 1) as f64),
                        Cell::Number(size),
                        Cell::Number(100.0 * size / n as f64),
                    ]);
                    row
                })
                .collect();
            let mut columns = head.clone();
            columns.extend(["Peer ID", "Peer Size", "Peer Size Percent"].map(String::from));
            let mut table = plain_table("Anomaly Case Peer ID List", 0, &columns, rows);
            none_caption(&mut table);
            tables.push(table);
            for reason in 0..reasons {
                let rows = anomalies
                    .iter()
                    .map(|&r| {
                        let k = reason_of(r, reason);
                        let mut row = lead(r);
                        row.extend([
                            text(input_text(&shown, &inputs[k])),
                            Cell::Number(impact[r][k]),
                            value_cell(k, r),
                            norm_cell(k, peer[r]),
                        ]);
                        row
                    })
                    .collect();
                let mut columns = head.clone();
                columns.extend(
                    [
                        "Reason Variable",
                        "Variable Impact",
                        "Variable Value",
                        "Variable Norm",
                    ]
                    .map(String::from),
                );
                let title = format!("Anomaly Case Reason List: Reason {}", reason + 1);
                let mut table = plain_table(&title, 0, &columns, rows);
                none_caption(&mut table);
                tables.push(table);
            }
        }
        if command.print_norms {
            let mut columns: Vec<String> = vec![String::new(), String::new()];
            columns.extend((1..=h).map(|g| g.to_string()));
            columns.push("Combined".into());
            let mut column_groups = vec![Vec::new(), Vec::new()];
            column_groups.extend((0..h).map(|_| vec!["Peer ID".to_string()]));
            column_groups.push(Vec::new());
            let mut all = groups[0].clone();
            for g in &groups[1..] {
                all.add(g);
            }
            let mut scale_rows = Vec::new();
            let mut category_rows = Vec::new();
            for (k, input) in inputs.iter().enumerate() {
                let name = text(input_text(&shown, input));
                let members = |g: Option<usize>| -> Vec<f64> {
                    (0..n)
                        .filter(|&r| g.is_none_or(|g| peer[r] == g))
                        .map(|r| continuous[slots[k]][r])
                        .collect()
                };
                if let Input::Categorical { .. } = input {
                    let mut modes = vec![name, text("Mode")];
                    let mut freq = vec![Cell::Empty, text("Frequency")];
                    let mut pct = vec![Cell::Empty, text("Percent")];
                    for cf in groups.iter().chain(std::iter::once(&all)) {
                        let (c, m) = mode(k, cf);
                        modes.push(category_text(k, c));
                        freq.push(Cell::Number(m));
                        pct.push(Cell::Number(100.0 * m / cf.n));
                    }
                    category_rows.extend([modes, freq, pct]);
                } else {
                    let mut means = vec![name, text("Mean")];
                    let mut sds = vec![Cell::Empty, text("Std. Deviation")];
                    for g in (0..h).map(Some).chain(std::iter::once(None)) {
                        let (mean, sd) = mean_sd(&members(g));
                        means.push(number(mean));
                        sds.push(number(sd));
                    }
                    scale_rows.extend([means, sds]);
                }
            }
            for (title, rows) in [
                ("Scale Variable Norms", scale_rows),
                ("Categorical Variable Norms", category_rows),
            ] {
                if !rows.is_empty() {
                    tables.push(Table {
                        title: title.into(),
                        row_headers: 2,
                        columns: columns.clone(),
                        column_groups: column_groups.clone(),
                        rows,
                        ..Table::default()
                    });
                }
            }
        }
        if command.print_summary {
            let values: Vec<f64> = anomalies.iter().map(|&r| index[r]).collect();
            let (mean, sd) = mean_sd(&values);
            let min = values.iter().copied().reduce(f64::min);
            let max = values.iter().copied().reduce(f64::max);
            let row = vec![
                text("Anomaly Index"),
                Cell::Number(values.len() as f64),
                number(min),
                number(max),
                number(mean),
                number(sd),
            ];
            let columns = ["", "N", "Minimum", "Maximum", "Mean", "Std. Deviation"];
            tables.push(plain_table("Anomaly Index Summary", 1, &columns, vec![row]));
        }
        if command.print_reasons {
            if reasons == 0 {
                warnings.push(
                    "DETECTANOMALY: REASONSUMMARY needs NUMREASONS above 0; it is skipped".into(),
                );
            }
            for reason in 0..reasons {
                let mut rows = Vec::new();
                for (k, input) in inputs.iter().enumerate() {
                    let impacts: Vec<f64> = anomalies
                        .iter()
                        .filter(|&&r| reason_of(r, reason) == k)
                        .map(|&r| impact[r][k])
                        .collect();
                    if impacts.is_empty() {
                        continue;
                    }
                    let (mean, sd) = mean_sd(&impacts);
                    rows.push(vec![
                        text(input_text(&shown, input)),
                        Cell::Number(impacts.len() as f64),
                        Cell::Number(100.0 * impacts.len() as f64 / anomalies.len() as f64),
                        number(impacts.iter().copied().reduce(f64::min)),
                        number(impacts.iter().copied().reduce(f64::max)),
                        number(mean),
                        number(sd),
                    ]);
                }
                let columns = [
                    "",
                    "Frequency",
                    "Percent",
                    "Minimum",
                    "Maximum",
                    "Mean",
                    "Std. Deviation",
                ];
                let mut table = plain_table(
                    &format!("Reason Summary: Reason {}", reason + 1),
                    1,
                    &columns,
                    rows,
                );
                for group in &mut table.column_groups[3..] {
                    group.push("Variable Impact".into());
                }
                none_caption(&mut table);
                tables.push(table);
            }
        }

        if let Some(file) = &command.outfile {
            let (mut at_scale, mut at_category) = (0, 0);
            let variables: Vec<_> = inputs
                .iter()
                .map(|input| match input {
                    Input::Categorical { .. } => {
                        let (levels, _, count) = &coded[at_category];
                        at_category += 1;
                        let levels: Vec<_> = levels.iter().map(key_json).collect();
                        json!({
                            "name": input.name(),
                            "kind": "categorical",
                            "categories": levels,
                            "missing_category": *count > levels.len(),
                        })
                    }
                    _ => {
                        let k = at_scale;
                        at_scale += 1;
                        json!({
                            "name": input.name(),
                            "kind": if matches!(input, Input::Scale { .. }) { "scale" } else { "missing proportion" },
                            "mean": means[k],
                            "variance": variances[k],
                        })
                    }
                })
                .collect();
            let peers: Vec<_> = (0..h)
                .map(|g| {
                    json!({
                        "number": g + 1,
                        "mean_deviation": mean_gdi[g],
                        "summary": cf_json(&groups[g]),
                    })
                })
                .collect();
            let model = json!({
                "format": "statscraft-anomaly-model",
                "ml_weight": command.ml_weight,
                "missing_applied": command.apply,
                "variables": variables,
                "peer_groups": peers,
            });
            self.write_json("DETECTANOMALY", file, "model", &model)?;
        }

        // saved variables
        let mut saves: Vec<(AnomalySave, Option<String>)> = Vec::new();
        for (what, name) in &command.save {
            let per_reason = matches!(
                what,
                AnomalySave::ReasonVar
                    | AnomalySave::ReasonMeasure
                    | AnomalySave::ReasonValue
                    | AnomalySave::ReasonNorm
            );
            if per_reason && reasons == 0 {
                warnings.push(
                    "DETECTANOMALY: reason variables need NUMREASONS above 0; they are not saved"
                        .into(),
                );
            } else {
                saves.push((*what, name.clone()));
            }
        }
        if saves.is_empty() {
            for warning in warnings {
                self.warn(warning);
            }
            return Ok(tables);
        }
        let textual = inputs.iter().any(|i| match i {
            Input::Categorical { variable, .. } => variable.is_string(),
            _ => false,
        });
        // the saved value of input k for training row r, and its norm in group g
        let raw_value = |k: usize, r: usize| -> Option<String> {
            let case = training[r];
            match &inputs[k] {
                Input::Scale { values, .. } | Input::MissingShare(values) => {
                    values[case].map(plain_number)
                }
                Input::Categorical { keys, .. } => keys[case].as_ref().map(key_text),
            }
        };
        let raw_norm = |k: usize, g: usize| -> Option<String> {
            match &inputs[k] {
                Input::Categorical { .. } => {
                    coded[slots[k]].0.get(mode(k, &groups[g]).0).map(key_text)
                }
                _ => Some(plain_number(groups[g].mean[slots[k]])),
            }
        };
        let data = self.active()?;
        let mut names: Vec<String> = Vec::new();
        let mut columns: Vec<(String, Saved, &str)> = Vec::new();
        for (what, given) in &saves {
            let (root, label) = match what {
                AnomalySave::Anomaly => ("AnomalyIndex", "Anomaly index"),
                AnomalySave::PeerId => ("PeerId", "Peer group"),
                AnomalySave::PeerSize => ("PeerSize", "Peer group size"),
                AnomalySave::PeerPctSize => ("PeerPctSize", "Peer group size in percent"),
                AnomalySave::ReasonVar => ("ReasonVar", "Reason variable"),
                AnomalySave::ReasonMeasure => ("ReasonMeasure", "Reason variable impact"),
                AnomalySave::ReasonValue => ("ReasonValue", "Reason variable value"),
                AnomalySave::ReasonNorm => ("ReasonNorm", "Reason variable norm"),
            };
            let per_reason = !matches!(
                what,
                AnomalySave::Anomaly
                    | AnomalySave::PeerId
                    | AnomalySave::PeerSize
                    | AnomalySave::PeerPctSize
            );
            let wanted: Vec<(String, usize)> = match per_reason {
                true => (0..reasons)
                    .map(|i| (format!("{}_{}", given.as_deref().unwrap_or(root), i + 1), i))
                    .collect(),
                false => vec![(given.clone().unwrap_or(root.to_string()), 0)],
            };
            for (want, reason) in wanted {
                let taken =
                    |n: &str| data.variable_index(n).is_some() || names.iter().any(|m| same(m, n));
                let name = match given {
                    Some(_) if data.variable_index(&want).is_some() => {
                        return Err(Error(format!(
                            "DETECTANOMALY /SAVE names {want}, a variable that already exists; choose another name"
                        )));
                    }
                    Some(_) if taken(&want) => {
                        return Err(Error(format!(
                            "DETECTANOMALY /SAVE gives the name {want} to two saved variables; choose another name"
                        )));
                    }
                    Some(_) => want,
                    None => free_name(&want, taken),
                };
                names.push(name.clone());
                let at = |r: usize| reason_of(r, reason);
                let saved = match what {
                    AnomalySave::Anomaly => Saved::Number(
                        (0..n).map(|r| Some(index[r])).collect(),
                        "F8.3",
                        Measure::Scale,
                    ),
                    AnomalySave::PeerId => Saved::Number(
                        (0..n).map(|r| Some((peer[r] + 1) as f64)).collect(),
                        "F8.0",
                        Measure::Nominal,
                    ),
                    AnomalySave::PeerSize => Saved::Number(
                        (0..n).map(|r| Some(sizes[peer[r]])).collect(),
                        "F8.0",
                        Measure::Scale,
                    ),
                    AnomalySave::PeerPctSize => Saved::Number(
                        (0..n)
                            .map(|r| Some(100.0 * sizes[peer[r]] / n as f64))
                            .collect(),
                        "F8.2",
                        Measure::Scale,
                    ),
                    AnomalySave::ReasonVar => {
                        Saved::Text((0..n).map(|r| inputs[at(r)].name()).collect())
                    }
                    AnomalySave::ReasonMeasure => Saved::Number(
                        (0..n).map(|r| Some(impact[r][at(r)])).collect(),
                        "F8.3",
                        Measure::Scale,
                    ),
                    AnomalySave::ReasonValue | AnomalySave::ReasonNorm => {
                        let value = |r: usize| match what {
                            AnomalySave::ReasonValue => raw_value(at(r), r),
                            _ => raw_norm(at(r), peer[r]),
                        };
                        match textual {
                            true => {
                                Saved::Text((0..n).map(|r| value(r).unwrap_or_default()).collect())
                            }
                            false => Saved::Number(
                                (0..n)
                                    .map(|r| value(r).and_then(|v| v.parse().ok()))
                                    .collect(),
                                "F8.2",
                                Measure::Scale,
                            ),
                        }
                    }
                };
                columns.push((name, saved, label));
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (name, saved, label) in columns {
                match saved {
                    Saved::Number(values, format, measure) => {
                        let mut all = vec![None; cases];
                        for (r, v) in values.into_iter().enumerate() {
                            all[training[r]] = v;
                        }
                        data.set_numeric_values(&name, all)?;
                        data.set_format(&name, format)?;
                        data.set_measure(&name, measure)?;
                    }
                    Saved::Text(values) => {
                        let width = values.iter().map(String::len).max().unwrap_or(1).max(8);
                        let mut all = vec![String::new(); cases];
                        for (r, v) in values.into_iter().enumerate() {
                            all[training[r]] = v;
                        }
                        data.set_string_values(&name, width, all)?;
                        data.set_measure(&name, Measure::Nominal)?;
                    }
                }
                data.set_variable_label(&name, Some(label))?;
            }
            Ok(())
        })?;
        Ok(tables)
    }
}

/// A variable to save: numbers with their format and level, or text.
enum Saved {
    Number(Vec<Option<f64>>, &'static str, Measure),
    Text(Vec<String>),
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Number(x) => plain_number(*x),
        Key::Text(t) => t.trim_end().to_string(),
    }
}

fn input_text(shown: &crate::shown::Shown, input: &Input) -> String {
    match input {
        Input::Scale { variable, .. } | Input::Categorical { variable, .. } => {
            shown.variable(variable)
        }
        Input::MissingShare(_) => input.name(),
    }
}
