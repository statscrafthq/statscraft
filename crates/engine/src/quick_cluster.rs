//! QUICK CLUSTER: k-means clustering of cases.
//!
//! Rules from the Command Syntax Reference (QUICK CLUSTER) and the Algorithms manual:
//! - Distances are Euclidean. LISTWISE leaves out a case missing on any clustering variable;
//!   PAIRWISE measures a case on the variables it has and leaves out only a case missing on all.
//!   Cases with a zero, negative or missing weight are left out; means use the weights.
//! - Initial centers: the first k cases with no missing value; then each later such case x
//!   replaces the nearer of the two closest centers M_m and M_n when x lies farther from every
//!   center than d(M_m, M_n), else it replaces its closest center M_q when its second closest
//!   center lies farther from x than M_q lies from every other center. NOINITIAL keeps the
//!   first k cases; INITIAL and FILE give the centers.
//! - KMEANS(NOUPDATE): every pass assigns all cases to their nearest center and moves each center
//!   to the mean of its cases. KMEANS(UPDATE): every pass moves the nearest center after each
//!   case, the center counting as one case of the mean. Passes stop when no center moves more
//!   than CONVERGE times the smallest distance between the initial centers, or after MXITER.
//!   CLASSIFY keeps the initial centers.
//! - The cases then go to their nearest classification center; the final centers are the means
//!   of their cases, and the saved distance is the one to the classification center.

use crate::descriptives::{case_weights, number};
use crate::dist::f_p;
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{ClusterMethod, ClusterSave, QuickCluster, VarItem};

/// The squared distance over the coordinates both have.
fn distance(x: &[f64], c: &[f64]) -> f64 {
    x.iter()
        .zip(c)
        .filter(|(x, c)| !x.is_nan() && !c.is_nan())
        .map(|(x, c)| (x - c).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// The nearest center, the first on a tie.
fn nearest(x: &[f64], centers: &[Vec<f64>]) -> (usize, f64) {
    centers
        .iter()
        .enumerate()
        .map(|(j, c)| (j, distance(x, c)))
        .fold(
            (0, f64::INFINITY),
            |best, d| if d.1 < best.1 { d } else { best },
        )
}

fn closest_pair(centers: &[Vec<f64>]) -> (usize, usize, f64) {
    let mut best = (0, 0, f64::INFINITY);
    for i in 0..centers.len() {
        for j in i + 1..centers.len() {
            let d = distance(&centers[i], &centers[j]);
            if d < best.2 {
                best = (i, j, d);
            }
        }
    }
    best
}

/// Step 1 of the algorithm: the first k complete cases, then replacements.
fn select_initial(cases: &[&Vec<f64>], k: usize, replace: bool) -> Vec<Vec<f64>> {
    let mut centers: Vec<Vec<f64>> = cases.iter().take(k).map(|x| x.to_vec()).collect();
    if !replace || k < 2 {
        return centers;
    }
    for x in &cases[k..] {
        let d: Vec<f64> = centers.iter().map(|c| distance(x, c)).collect();
        let (m, n, dmn) = closest_pair(&centers);
        let closest = d.iter().copied().fold(f64::INFINITY, f64::min);
        if closest > dmn && d[m] != d[n] {
            let replaced = if d[m] > d[n] { n } else { m };
            centers[replaced] = x.to_vec();
            continue;
        }
        let mut order: Vec<usize> = (0..k).collect();
        order.sort_by(|&a, &b| d[a].total_cmp(&d[b]));
        let (q, p) = (order[0], order[1]);
        let from_q = (0..k)
            .filter(|&i| i != q)
            .map(|i| distance(&centers[q], &centers[i]))
            .fold(f64::INFINITY, f64::min);
        if d[p] > from_q {
            centers[q] = x.to_vec();
        }
    }
    centers
}

/// The weighted mean of each cluster's cases, variable by variable; None for a cluster or a
/// variable without cases.
fn means(rows: &[(Vec<f64>, f64)], member: &[usize], k: usize, v: usize) -> Vec<Vec<Option<f64>>> {
    let mut sum = vec![vec![0.0; v]; k];
    let mut n = vec![vec![0.0; v]; k];
    for ((x, w), &j) in rows.iter().zip(member) {
        for i in 0..v {
            if !x[i].is_nan() {
                sum[j][i] += w * x[i];
                n[j][i] += w;
            }
        }
    }
    (0..k)
        .map(|j| {
            (0..v)
                .map(|i| (n[j][i] > 0.0).then(|| sum[j][i] / n[j][i]))
                .collect()
        })
        .collect()
}

fn centers_table(title: &str, names: &[String], centers: &[Vec<f64>]) -> Table {
    let k = centers.len();
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut row = vec![text(name.clone())];
            row.extend(centers.iter().map(|c| number(Some(c[i]))));
            row
        })
        .collect();
    let mut columns = vec![String::new()];
    columns.extend((1..=k).map(|j| j.to_string()));
    let mut groups = vec![Vec::new()];
    groups.extend((0..k).map(|_| vec!["Cluster".to_string()]));
    Table {
        title: title.into(),
        row_headers: 1,
        columns,
        column_groups: groups,
        rows,
        ..Table::default()
    }
}

fn plain_table(title: &str, row_headers: usize, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers,
        columns: columns.iter().map(|c| c.to_string()).collect(),
        column_groups: vec![Vec::new(); columns.len()],
        rows,
        ..Table::default()
    }
}

impl Engine {
    pub(crate) fn quick_cluster(&mut self, command: &QuickCluster) -> Result<Vec<Table>, Error> {
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
                "QUICK CLUSTER needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let (k, v) = (command.clusters, names.len());
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let value = |var: &Variable, case: usize| -> f64 {
            if !is_valid(var, case, command.include) {
                return f64::NAN;
            }
            match var.values() {
                Values::Numeric(values) => values.get(case).unwrap_or(f64::NAN),
                Values::String(_) => f64::NAN,
            }
        };
        let x: Vec<Vec<f64>> = (0..cases)
            .map(|case| variables.iter().map(|var| value(var, case)).collect())
            .collect();
        let w: Vec<f64> = (0..cases)
            .map(|case| weights.as_ref().map_or(1.0, |w| w[case]))
            .collect();
        let usable: Vec<usize> = (0..cases)
            .filter(|&case| {
                w[case] > 0.0
                    && match command.pairwise {
                        true => x[case].iter().any(|v| !v.is_nan()),
                        false => x[case].iter().all(|v| !v.is_nan()),
                    }
            })
            .collect();
        let complete: Vec<&Vec<f64>> = usable
            .iter()
            .map(|&case| &x[case])
            .filter(|x| x.iter().all(|v| !v.is_nan()))
            .collect();
        let missing: f64 = (0..cases)
            .filter(|&case| w[case] > 0.0 && !usable.contains(&case))
            .map(|case| w[case])
            .sum();

        let initial: Vec<Vec<f64>> = if let Some(values) = &command.initial {
            if values.len() != k * v {
                return Err(Error(format!(
                    "QUICK CLUSTER /INITIAL needs {} values ({k} clusters of {v} variables), got {}",
                    k * v,
                    values.len()
                )));
            }
            values.chunks(v).map(|c| c.to_vec()).collect()
        } else if let Some(file) = &command.file {
            let source = self.data_file(file)?;
            if source.case_count() < k {
                return Err(Error(format!(
                    "QUICK CLUSTER /FILE: {file} has {} cases, fewer than the {k} clusters",
                    source.case_count()
                )));
            }
            let mut centers = vec![vec![0.0; v]; k];
            for (i, name) in names.iter().enumerate() {
                let var = source.variable(name).map_err(|_| {
                    Error(format!(
                        "QUICK CLUSTER /FILE: {file} has no variable {name}"
                    ))
                })?;
                for (j, center) in centers.iter_mut().enumerate() {
                    center[i] = match var.values() {
                        Values::Numeric(values) => values.get(j).unwrap_or(f64::NAN),
                        Values::String(_) => f64::NAN,
                    };
                }
            }
            if centers.iter().flatten().any(|c| c.is_nan()) {
                return Err(Error(format!(
                    "QUICK CLUSTER /FILE: a center in {file} has a missing value"
                )));
            }
            centers
        } else {
            if complete.len() < k {
                return Err(Error(format!(
                    "QUICK CLUSTER: {k} clusters need at least {k} cases without missing values, but there are {}",
                    complete.len()
                )));
            }
            select_initial(&complete, k, !command.no_initial)
        };
        let mut tables = Vec::new();
        if command.print_initial {
            tables.push(centers_table("Initial Cluster Centers", &names, &initial));
        }
        let rows: Vec<(Vec<f64>, f64)> = usable.iter().map(|&c| (x[c].clone(), w[c])).collect();
        let (_, _, initial_gap) = closest_pair(&initial);
        let initial_gap = if initial_gap.is_finite() {
            initial_gap
        } else {
            0.0
        };

        let mut centers = initial.clone();
        if let ClusterMethod::KMeans { update } = command.method {
            let limit = command.converge * initial_gap;
            let mut history: Vec<Vec<f64>> = Vec::new();
            let mut converged = false;
            while history.len() < command.max_iterations {
                let start = centers.clone();
                if update {
                    let mut counts = vec![1.0; k];
                    for (row, weight) in &rows {
                        let (j, _) = nearest(row, &centers);
                        counts[j] += weight;
                        for i in (0..v).filter(|&i| !row[i].is_nan()) {
                            centers[j][i] += weight * (row[i] - centers[j][i]) / counts[j];
                        }
                    }
                } else {
                    let member: Vec<usize> =
                        rows.iter().map(|(r, _)| nearest(r, &centers).0).collect();
                    let means = means(&rows, &member, k, v);
                    for (center, mean) in centers.iter_mut().zip(means) {
                        for (c, m) in center.iter_mut().zip(mean) {
                            if let Some(m) = m {
                                *c = m;
                            }
                        }
                    }
                }
                let change: Vec<f64> = centers
                    .iter()
                    .zip(&start)
                    .map(|(a, b)| distance(a, b))
                    .collect();
                let largest = change.iter().copied().fold(0.0, f64::max);
                history.push(change);
                if largest <= limit {
                    converged = true;
                    break;
                }
            }
            let largest = history
                .last()
                .map_or(0.0, |c| c.iter().copied().fold(0.0, f64::max));
            let rows: Vec<Vec<Cell>> = history
                .iter()
                .enumerate()
                .map(|(i, change)| {
                    let mut row = vec![Cell::Number((i + 1) as f64)];
                    row.extend(change.iter().map(|c| number(Some(*c))));
                    row
                })
                .collect();
            let mut columns = vec!["Iteration".to_string()];
            columns.extend((1..=k).map(|j| j.to_string()));
            let mut groups = vec![Vec::new()];
            groups.extend((0..k).map(|_| vec!["Change in Cluster Centers".to_string()]));
            let reason = match converged {
                true => "Convergence achieved due to no or small change in cluster centers.",
                false => {
                    "Iterations stopped because the maximum number of iterations was performed. Iterations failed to converge."
                }
            };
            tables.push(Table {
                title: "Iteration History".into(),
                row_headers: 1,
                columns,
                column_groups: groups,
                rows,
                caption: Some(format!(
                    "{reason} The largest change of a center in the last iteration is {largest:.3}. The current iteration is {}. The minimum distance between initial centers is {initial_gap:.3}.",
                    history.len()
                )),
                ..Table::default()
            });
            if !converged {
                self.warn(format!(
                    "QUICK CLUSTER: the centers did not converge in {} iterations",
                    command.max_iterations
                ));
            }
        }

        // classification
        let assigned: Vec<(usize, f64)> = rows.iter().map(|(r, _)| nearest(r, &centers)).collect();
        let member: Vec<usize> = assigned.iter().map(|a| a.0).collect();
        let final_means = means(&rows, &member, k, v);
        let finals: Vec<Vec<f64>> = final_means
            .iter()
            .map(|c| c.iter().map(|m| m.unwrap_or(f64::NAN)).collect())
            .collect();
        let counts: Vec<f64> = (0..k)
            .map(|j| {
                rows.iter()
                    .zip(&member)
                    .filter(|(_, m)| **m == j)
                    .map(|((_, w), _)| w)
                    .sum()
            })
            .collect();
        if let Some(j) = (0..k).find(|&j| counts[j] == 0.0) {
            self.warn(format!(
                "QUICK CLUSTER: cluster {} has no cases, so its final center is empty",
                j + 1
            ));
        }

        if command.print_cluster {
            let data = self.active()?;
            let id = match &command.id {
                Some(name) => Some(data.variable(name)?),
                None => None,
            };
            let mut columns = vec!["Case Number"];
            if let Some(var) = &id {
                columns.push(var.name());
            }
            columns.extend(["Cluster", "Distance"]);
            let table_rows = usable
                .iter()
                .zip(&assigned)
                .map(|(&case, (j, d))| {
                    let number_of = self.case_numbers.as_ref().map_or(case, |n| n[case]) + 1;
                    let mut row = vec![Cell::Number(number_of as f64)];
                    if let Some(var) = &id {
                        row.push(match var.values() {
                            Values::Numeric(values) => number(values.get(case)),
                            Values::String(values) => text(values.get(case).trim_end()),
                        });
                    }
                    row.extend([Cell::Number((j + 1) as f64), number(Some(*d))]);
                    row
                })
                .collect();
            tables.push(plain_table("Cluster Membership", 0, &columns, table_rows));
        }
        tables.push(centers_table("Final Cluster Centers", &names, &finals));
        if command.print_distance {
            let rows = (0..k)
                .map(|i| {
                    let mut row = vec![Cell::Number((i + 1) as f64)];
                    row.extend((0..k).map(|j| match i == j {
                        true => Cell::Empty,
                        false => number(Some(distance(&finals[i], &finals[j]))),
                    }));
                    row
                })
                .collect();
            let mut columns = vec!["Cluster".to_string()];
            columns.extend((1..=k).map(|j| j.to_string()));
            tables.push(Table {
                title: "Distances between Final Cluster Centers".into(),
                row_headers: 1,
                column_groups: vec![Vec::new(); k + 1],
                columns,
                rows,
                ..Table::default()
            });
        }
        if command.print_anova {
            let rows = (0..v)
                .map(|i| {
                    let valid: Vec<(f64, f64, usize)> = rows
                        .iter()
                        .zip(&member)
                        .filter(|((x, _), _)| !x[i].is_nan())
                        .map(|((x, w), &j)| (x[i], *w, j))
                        .collect();
                    let n: f64 = valid.iter().map(|t| t.1).sum();
                    let grand = valid.iter().map(|t| t.0 * t.1).sum::<f64>() / n;
                    let groups = (0..k).filter(|&j| valid.iter().any(|t| t.2 == j)).count() as f64;
                    let between: f64 = (0..k)
                        .filter_map(|j| {
                            let nj: f64 = valid.iter().filter(|t| t.2 == j).map(|t| t.1).sum();
                            final_means[j][i].map(|m| nj * (m - grand).powi(2))
                        })
                        .sum();
                    let within: f64 = valid
                        .iter()
                        .map(|(x, w, j)| w * (x - final_means[*j][i].unwrap_or(f64::NAN)).powi(2))
                        .sum();
                    let (df1, df2) = (groups - 1.0, n - groups);
                    let (ms1, ms2) = (between / df1, within / df2);
                    let f = ms1 / ms2;
                    vec![
                        text(names[i].clone()),
                        number(Some(ms1)),
                        Cell::Number(df1),
                        number(Some(ms2)),
                        Cell::Number(df2),
                        number(Some(f)),
                        number(Some(f_p(f, df1, df2))),
                    ]
                })
                .collect();
            let columns = ["", "Mean Square", "df", "Mean Square", "df", "F", "Sig."];
            let mut t = plain_table("ANOVA", 1, &columns, rows);
            t.column_groups = vec![
                Vec::new(),
                vec!["Cluster".into()],
                vec!["Cluster".into()],
                vec!["Error".into()],
                vec!["Error".into()],
                Vec::new(),
                Vec::new(),
            ];
            t.caption = Some(
                "The F tests should be used only for descriptive purposes because the clusters have been chosen to maximize the differences among cases in different clusters. The observed significance levels are not corrected for this and thus cannot be interpreted as tests of the hypothesis that the cluster means are equal."
                    .into(),
            );
            tables.push(t);
        }
        let valid: f64 = counts.iter().sum();
        let mut count_rows: Vec<Vec<Cell>> = counts
            .iter()
            .enumerate()
            .map(|(j, n)| {
                vec![
                    if j == 0 { text("Cluster") } else { Cell::Empty },
                    Cell::Number((j + 1) as f64),
                    Cell::Number(*n),
                ]
            })
            .collect();
        count_rows.push(vec![text("Valid"), Cell::Empty, Cell::Number(valid)]);
        count_rows.push(vec![text("Missing"), Cell::Empty, Cell::Number(missing)]);
        tables.push(plain_table(
            "Number of Cases in each Cluster",
            2,
            &["", "", "N"],
            count_rows,
        ));

        if let Some(file) = &command.outfile {
            let mut out = Dataset::new(k);
            out.set_numeric_values("CLUSTER_", (1..=k).map(|j| Some(j as f64)).collect())?;
            out.set_format("CLUSTER_", "F8.0")?;
            for (i, name) in names.iter().enumerate() {
                let values = finals
                    .iter()
                    .map(|c| Some(c[i]).filter(|v| v.is_finite()))
                    .collect();
                out.set_numeric_values(name, values)?;
            }
            self.write_data_file(file, out)?;
        }
        if !command.save.is_empty() {
            let mut cluster = vec![None; cases];
            let mut distance = vec![None; cases];
            for (&case, (j, d)) in usable.iter().zip(&assigned) {
                cluster[case] = Some((j + 1) as f64);
                distance[case] = Some(*d);
            }
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (what, name) in &command.save {
                    let name = match name {
                        Some(name) => {
                            if data.variable_index(name).is_some() {
                                return Err(Error(format!(
                                    "QUICK CLUSTER /SAVE: variable {name} already exists"
                                )));
                            }
                            name.clone()
                        }
                        None => (1..)
                            .map(|n| format!("QCL_{n}"))
                            .find(|n| data.variable_index(n).is_none())
                            .expect("a free name"),
                    };
                    let (values, label, format) = match what {
                        ClusterSave::Cluster => (&cluster, "Cluster Number of Case", "F8.0"),
                        ClusterSave::Distance => (
                            &distance,
                            "Distance of Case from its Classification Cluster Center",
                            "F11.5",
                        ),
                    };
                    data.set_numeric_values(&name, values.clone())?;
                    data.set_variable_label(&name, Some(label))?;
                    data.set_format(&name, format)?;
                    let measure = match what {
                        ClusterSave::Cluster => Measure::Nominal,
                        ClusterSave::Distance => Measure::Scale,
                    };
                    data.set_measure(&name, measure)?;
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}
