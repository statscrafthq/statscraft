//! Complex samples: CSPLAN analysis plans, CSDESCRIPTIVES and CSTABULATE.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - The plan is a JSON file of our own (format "statscraft-analysis-plan", version 1) with the
//!   analysis weight, the simple random sampling estimator for design effects and one to three
//!   stages of strata, clusters, estimator and population size or inclusion probability.
//! - A case belongs to the design when its weight, strata, clusters and rate variables are all
//!   valid (user-missing strata and clusters count as valid under CLASSMISSING=INCLUDE).
//! - Every estimate is a total or a ratio of totals. With z_i = w_i u_i, the variance of the
//!   total of z adds over the strata h of each stage mult_h fpc_h n_h S_h^2, where S_h^2 is the
//!   variance of the unit totals of z in h, n_h the number of units, fpc_h = 1 - f_h under
//!   EQUAL_WOR (f_h the inclusion probability, or n_h over the population size) and 1 under WR.
//!   A stratum with one unit adds 0. mult is 1 in the first stage and, in a later stage, the
//!   mult of the parent stratum times its f. A WR stage ends the variance: later stages add
//!   nothing.
//! - Linearized values: a mean u = (y - mean) / N, a ratio u = (y - R x) / X, a proportion
//!   u = (d_cell - d_base P) / N_base. Domains (subpopulations, cases without valid analysis
//!   values) keep the whole design and set u = 0 outside.
//! - df = first-stage units minus first-stage strata. Intervals: estimate +- t SE; proportions on
//!   the logit scale; odds ratios and relative risks on the log scale.
//! - Design effect = V / V_srs with V_srs = fpc N / (n - 1) sum w (u - u_bar)^2 over the design
//!   cases, u_bar the weighted mean of u, fpc = 1 - n / N (none when n / N >= 1) or 1 under
//!   SRSESTIMATOR WR.
//! - Tests of tables: Pearson and likelihood ratio chi-squares divided by the trace of the
//!   generalized design effect matrix, against F with d = tr^2 / tr(D^2) and d df numerator and
//!   d times the design df denominator.

use crate::crosstabs::{Key, categories};
use crate::descriptives::number;
use crate::dist::{f_p, t_p, t_quantile};
use crate::matrix::{Matrix, inverse, multiply, transpose};
use crate::npar_tests::text;
use crate::regression::table;
use crate::shown::Shown;
use crate::variables::duplicate;
use crate::{Cell, Engine, Error, Table};
use serde::{Deserialize, Serialize};
use statscraft_data::{Dataset, Variable};
use statscraft_syntax::{
    CsRate, CsStatistics, CsSubpop, Csdescriptives, Csplan, CsplanMode, Cstabulate, Value,
};
use std::collections::HashMap;

const FORMAT: &str = "statscraft-analysis-plan";

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlanFile {
    format: String,
    version: u32,
    weight: String,
    /// "WOR" or "WR".
    srs_estimator: String,
    stages: Vec<PlanStage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlanStage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default)]
    strata: Vec<String>,
    #[serde(default)]
    clusters: Vec<String>,
    /// "EQUAL_WOR", "UNEQUAL_WOR" or "WR".
    estimator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    popsize: Option<PlanRate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inclprob: Option<PlanRate>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
enum PlanRate {
    Value(f64),
    Variable(String),
    Matrix {
        variables: Vec<String>,
        rows: Vec<PlanRow>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlanRow {
    categories: Vec<PlanKey>,
    value: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum PlanKey {
    Number(f64),
    Text(String),
}

impl PlanKey {
    fn matches(&self, key: &Key) -> bool {
        match (self, key) {
            (PlanKey::Number(a), Key::Number(b)) => a == b,
            (PlanKey::Text(a), Key::Text(b)) => a.trim_end() == b.trim_end(),
            _ => false,
        }
    }

    fn show(&self) -> String {
        match self {
            PlanKey::Number(n) => n.to_string(),
            PlanKey::Text(t) => format!("'{t}'"),
        }
    }
}

fn plan_rate(rate: &CsRate) -> PlanRate {
    match rate {
        CsRate::Value(v) => PlanRate::Value(*v),
        CsRate::Variable(v) => PlanRate::Variable(v.clone()),
        CsRate::Matrix { variables, rows } => PlanRate::Matrix {
            variables: variables.clone(),
            rows: rows
                .iter()
                .map(|(cats, value)| PlanRow {
                    categories: cats
                        .iter()
                        .map(|c| match c {
                            Value::Number(n) => PlanKey::Number(*n),
                            Value::String(s) => PlanKey::Text(s.clone()),
                        })
                        .collect(),
                    value: *value,
                })
                .collect(),
        },
    }
}

fn plan_of(command: &Csplan) -> PlanFile {
    PlanFile {
        format: FORMAT.into(),
        version: 1,
        weight: command.weight.clone().unwrap_or_default(),
        srs_estimator: if command.srs_wr { "WR" } else { "WOR" }.into(),
        stages: command
            .stages
            .iter()
            .map(|s| PlanStage {
                label: s.label.clone(),
                strata: s.strata.clone(),
                clusters: s.clusters.clone(),
                estimator: s.estimator.keyword().into(),
                popsize: s.popsize.as_ref().map(plan_rate),
                inclprob: s.inclprob.as_ref().map(plan_rate),
            })
            .collect(),
    }
}

fn estimator_text(estimator: &str) -> &'static str {
    match estimator {
        "EQUAL_WOR" => "Equal probability, without replacement",
        "UNEQUAL_WOR" => "Unequal probability, without replacement",
        _ => "With replacement",
    }
}

fn rate_text(rate: &PlanRate) -> String {
    match rate {
        PlanRate::Value(v) => v.to_string(),
        PlanRate::Variable(v) => format!("variable {v}"),
        PlanRate::Matrix { variables, rows } => {
            format!("matrix by {} ({} rows)", variables.join(", "), rows.len())
        }
    }
}

/// The tables PRINT asks for.
fn plan_tables(plan: &PlanFile, file: &str, print_plan: bool, print_matrix: bool) -> Vec<Table> {
    let mut out = Vec::new();
    if print_plan {
        let mut rows = vec![
            vec![text("Plan"), text("File"), text(file)],
            vec![text(""), text("Analysis weight"), text(&plan.weight)],
            vec![
                text(""),
                text("Simple random sampling estimator"),
                text(if plan.srs_estimator == "WR" {
                    "With replacement"
                } else {
                    "Without replacement"
                }),
            ],
        ];
        for (s, stage) in plan.stages.iter().enumerate() {
            let none = |list: &[String]| {
                if list.is_empty() {
                    "None".to_string()
                } else {
                    list.join(", ")
                }
            };
            let name = format!("Stage {}", s + 1);
            rows.push(vec![
                text(&name),
                text("Label"),
                text(stage.label.clone().unwrap_or_else(|| name.clone())),
            ]);
            rows.push(vec![text(""), text("Strata"), text(none(&stage.strata))]);
            rows.push(vec![
                text(""),
                text("Clusters"),
                text(none(&stage.clusters)),
            ]);
            rows.push(vec![
                text(""),
                text("Estimator"),
                text(estimator_text(&stage.estimator)),
            ]);
            if let Some(rate) = &stage.popsize {
                rows.push(vec![
                    text(""),
                    text("Population size"),
                    text(rate_text(rate)),
                ]);
            }
            if let Some(rate) = &stage.inclprob {
                rows.push(vec![
                    text(""),
                    text("Inclusion probability"),
                    text(rate_text(rate)),
                ]);
            }
        }
        out.push(table(
            "Analysis Plan",
            2,
            vec![
                (String::new(), String::new()),
                (String::new(), String::new()),
                (String::new(), "Value".into()),
            ],
            rows,
            None,
        ));
    }
    if print_matrix {
        let mut rows = Vec::new();
        for (s, stage) in plan.stages.iter().enumerate() {
            for (kind, rate) in [
                ("Population size", &stage.popsize),
                ("Inclusion probability", &stage.inclprob),
            ] {
                if let Some(PlanRate::Matrix {
                    variables,
                    rows: specs,
                }) = rate
                {
                    for spec in specs {
                        let cats: Vec<String> = variables
                            .iter()
                            .zip(&spec.categories)
                            .map(|(v, c)| format!("{v} = {}", c.show()))
                            .collect();
                        rows.push(vec![
                            text(format!("Stage {}", s + 1)),
                            text(kind),
                            text(cats.join(", ")),
                            Cell::Number(spec.value),
                        ]);
                    }
                }
            }
        }
        if !rows.is_empty() {
            out.push(table(
                "Plan Matrix",
                3,
                vec![
                    (String::new(), "Stage".into()),
                    (String::new(), "Specification".into()),
                    (String::new(), "Categories".into()),
                    (String::new(), "Value".into()),
                ],
                rows,
                None,
            ));
        }
    }
    out
}

/// One stage of the design as the variance needs it.
struct StageIndex {
    /// The unit of every design case.
    unit: Vec<usize>,
    /// The stratum of every unit.
    unit_stratum: Vec<usize>,
    /// Per stratum: units, fpc (1 - f, or 1 under WR), f and the multiplier from earlier stages.
    n_units: Vec<usize>,
    fpc: Vec<f64>,
    rate: Vec<f64>,
    mult: Vec<f64>,
}

/// The design of the cases that have valid design variables.
pub(crate) struct Design {
    /// The dataset case of every design case.
    rows: Vec<usize>,
    w: Vec<f64>,
    stages: Vec<StageIndex>,
    df: f64,
    total_w: f64,
    srs_wr: bool,
    /// Whether design effects are asked for; without them V_srs is not computed.
    deff: bool,
    /// Upper-case names of every plan variable, and of the first-stage strata.
    plan_vars: Vec<String>,
    strata_clusters: Vec<String>,
    first_strata: Vec<String>,
}

/// The category of every dataset case, string values without trailing blanks.
fn keys_of(variable: &Variable, include: bool) -> Vec<Option<Key>> {
    categories(variable, include)
        .into_iter()
        .map(|k| {
            k.map(|k| match k {
                Key::Text(t) => Key::Text(t.trim_end().to_string()),
                other => other,
            })
        })
        .collect()
}

/// A key as part of a hash key.
fn key_part(key: &Key) -> String {
    match key {
        Key::Number(n) => format!("n{}", n.to_bits()),
        Key::Text(t) => format!("s{t}"),
    }
}

/// Valid numbers of every dataset case: None for system- and user-missing.
fn numbers(variable: &Variable) -> Result<Vec<Option<f64>>, Error> {
    let values = variable.numeric()?;
    Ok(values
        .iter()
        .map(|v| v.filter(|v| !variable.is_user_missing(*v)))
        .collect())
}

/// Where a per-case rate value comes from.
enum RateSource {
    Value(f64),
    Variable(Vec<Option<f64>>),
    Matrix {
        keys: Vec<Vec<Option<Key>>>,
        rows: Vec<PlanRow>,
        variables: Vec<String>,
    },
}

impl RateSource {
    fn at(&self, case: usize) -> Result<Option<f64>, Error> {
        Ok(match self {
            RateSource::Value(v) => Some(*v),
            RateSource::Variable(values) => values[case],
            RateSource::Matrix {
                keys,
                rows,
                variables,
            } => {
                let case_keys: Vec<&Key> = match keys.iter().map(|k| k[case].as_ref()).collect() {
                    Some(k) => k,
                    None => return Ok(None),
                };
                let found = rows.iter().rev().find(|r| {
                    r.categories
                        .iter()
                        .zip(&case_keys)
                        .all(|(c, k)| c.matches(k))
                });
                match found {
                    Some(r) => Some(r.value),
                    None => {
                        let shown: Vec<String> = variables
                            .iter()
                            .zip(&case_keys)
                            .map(|(v, k)| match k {
                                Key::Number(n) => format!("{v} = {n}"),
                                Key::Text(t) => format!("{v} = '{t}'"),
                            })
                            .collect();
                        return Err(Error(format!(
                            "the plan's MATRIX has no row for {}",
                            shown.join(", ")
                        )));
                    }
                }
            }
        })
    }
}

impl Design {
    fn new(data: &Dataset, plan: &PlanFile, include: bool, deff: bool) -> Result<Design, Error> {
        let weight_var = data.variable(&plan.weight)?;
        let weights = numbers(&weight_var)?;
        let mut used: Vec<&PlanStage> = Vec::new();
        for stage in &plan.stages {
            if stage.estimator == "UNEQUAL_WOR" {
                return Err(Error(
                    "the UNEQUAL_WOR estimator needs joint inclusion probabilities and is not supported yet"
                        .into(),
                ));
            }
            used.push(stage);
            if stage.estimator == "WR" {
                break;
            }
        }
        let mut plan_vars = vec![plan.weight.to_uppercase()];
        let mut strata_clusters = Vec::new();
        struct StageData {
            strata: Vec<Vec<Option<Key>>>,
            clusters: Vec<Vec<Option<Key>>>,
            rate: Option<(RateSource, bool)>,
        }
        let mut stage_data = Vec::new();
        for stage in &plan.stages {
            for name in stage.strata.iter().chain(&stage.clusters) {
                plan_vars.push(name.to_uppercase());
                strata_clusters.push(name.to_uppercase());
            }
            for rate in stage.popsize.iter().chain(&stage.inclprob) {
                if let PlanRate::Variable(v) = rate {
                    plan_vars.push(v.to_uppercase());
                }
            }
        }
        for stage in &used {
            let keys = |names: &[String]| -> Result<Vec<Vec<Option<Key>>>, Error> {
                names
                    .iter()
                    .map(|n| Ok(keys_of(&data.variable(n)?, include)))
                    .collect()
            };
            let wor = stage.estimator == "EQUAL_WOR";
            let rate = match (&stage.popsize, &stage.inclprob) {
                _ if !wor => None,
                (Some(r), _) | (None, Some(r)) => {
                    let is_popsize = stage.popsize.is_some();
                    let source = match r {
                        PlanRate::Value(v) => RateSource::Value(*v),
                        PlanRate::Variable(v) => RateSource::Variable(numbers(&data.variable(v)?)?),
                        PlanRate::Matrix { variables, rows } => RateSource::Matrix {
                            keys: keys(variables)?,
                            rows: rows.clone(),
                            variables: variables.clone(),
                        },
                    };
                    Some((source, is_popsize))
                }
                (None, None) => {
                    return Err(Error(
                        "the plan gives EQUAL_WOR without a population size or inclusion probability"
                            .into(),
                    ));
                }
            };
            stage_data.push(StageData {
                strata: keys(&stage.strata)?,
                clusters: keys(&stage.clusters)?,
                rate,
            });
        }
        // the design cases
        let mut rows = Vec::new();
        let mut w = Vec::new();
        let mut rate_values: Vec<Vec<f64>> = vec![Vec::new(); stage_data.len()];
        'cases: for case in 0..data.case_count() {
            // a weight that is missing or not above 0 is not a valid design value
            let Some(wt) = weights[case].filter(|w| *w > 0.0) else {
                continue;
            };
            for sd in &stage_data {
                if sd
                    .strata
                    .iter()
                    .chain(&sd.clusters)
                    .any(|k| k[case].is_none())
                {
                    continue 'cases;
                }
            }
            let mut rates = Vec::new();
            for sd in &stage_data {
                match &sd.rate {
                    Some((source, _)) => match source.at(case)? {
                        Some(v) => rates.push(v),
                        None => continue 'cases,
                    },
                    None => rates.push(0.0),
                }
            }
            for (s, r) in rates.into_iter().enumerate() {
                rate_values[s].push(r);
            }
            rows.push(case);
            w.push(wt);
        }
        if rows.is_empty() {
            return Err(Error(
                "no case has valid values on every design variable".into(),
            ));
        }
        // the stages
        let mut stages: Vec<StageIndex> = Vec::new();
        let n = rows.len();
        let mut parent: Vec<usize> = vec![0; n];
        let last = stage_data.len() - 1;
        for (s, sd) in stage_data.iter().enumerate() {
            let mut stratum_ids: HashMap<String, usize> = HashMap::new();
            let mut stratum_parent: Vec<usize> = Vec::new();
            let mut unit_ids: HashMap<String, usize> = HashMap::new();
            let mut unit = Vec::with_capacity(n);
            let mut unit_stratum = Vec::new();
            let mut stratum_of_case = Vec::with_capacity(n);
            for (c, &case) in rows.iter().enumerate() {
                let mut key = format!("{}", parent[c]);
                for k in &sd.strata {
                    key.push('\u{1}');
                    key.push_str(&key_part(k[case].as_ref().expect("design case")));
                }
                let next = stratum_ids.len();
                let h = *stratum_ids.entry(key.clone()).or_insert(next);
                if h == stratum_parent.len() {
                    stratum_parent.push(parent[c]);
                }
                let mut ukey = format!("{h}\u{2}");
                if sd.clusters.is_empty() {
                    ukey.push_str(&format!("case{c}"));
                } else {
                    for k in &sd.clusters {
                        ukey.push('\u{1}');
                        ukey.push_str(&key_part(k[case].as_ref().expect("design case")));
                    }
                }
                let next = unit_ids.len();
                let u = *unit_ids.entry(ukey).or_insert(next);
                if u == unit_stratum.len() {
                    unit_stratum.push(h);
                }
                unit.push(u);
                stratum_of_case.push(h);
            }
            let strata = stratum_parent.len();
            let mut n_units = vec![0usize; strata];
            for &h in &unit_stratum {
                n_units[h] += 1;
            }
            let mut fpc = vec![1.0; strata];
            let mut rate = vec![0.0; strata];
            if let Some((_, is_popsize)) = &sd.rate {
                let mut given: Vec<Option<f64>> = vec![None; strata];
                for (c, &h) in stratum_of_case.iter().enumerate() {
                    let v = rate_values[s][c];
                    match given[h] {
                        None => given[h] = Some(v),
                        Some(g) if (g - v).abs() > 1e-9 * g.abs().max(1.0) => {
                            let what = if *is_popsize {
                                "population size"
                            } else {
                                "inclusion probability"
                            };
                            return Err(Error(format!(
                                "the {what} of stage {} differs within a stratum ({g} and {v})",
                                s + 1
                            )));
                        }
                        Some(_) => {}
                    }
                }
                for h in 0..strata {
                    let v = given[h].expect("every stratum has a case");
                    let f = if *is_popsize {
                        if v <= 0.0 {
                            return Err(Error(format!("a population size of {v} is not above 0")));
                        }
                        n_units[h] as f64 / v
                    } else {
                        v
                    };
                    if !(f > 0.0 && f <= 1.0 + 1e-12) {
                        return Err(Error(format!(
                            "stage {}: the sampled units are {} of the population, which is not above 0 and at most 1 (a population size below the number of units drawn?)",
                            s + 1,
                            f
                        )));
                    }
                    rate[h] = f.min(1.0);
                    fpc[h] = 1.0 - rate[h];
                }
            }
            let mult: Vec<f64> = match stages.last() {
                None => vec![1.0; strata],
                Some(prev) => stratum_parent
                    .iter()
                    .map(|&pu| {
                        let g = prev.unit_stratum[pu];
                        prev.mult[g] * prev.rate[g]
                    })
                    .collect(),
            };
            if s < last && sd.clusters.is_empty() {
                return Err(Error(format!(
                    "stage {} of the plan has a later stage but no CLUSTER",
                    s + 1
                )));
            }
            parent = unit.clone();
            stages.push(StageIndex {
                unit,
                unit_stratum,
                n_units,
                fpc,
                rate,
                mult,
            });
        }
        let first = &stages[0];
        let df = first.unit_stratum.len() as f64 - first.n_units.len() as f64;
        let total_w = w.iter().sum();
        Ok(Design {
            rows,
            w,
            stages,
            df,
            total_w,
            srs_wr: plan.srs_estimator == "WR",
            deff,
            plan_vars,
            strata_clusters,
            first_strata: plan
                .stages
                .first()
                .map(|s| s.strata.iter().map(|v| v.to_uppercase()).collect())
                .unwrap_or_default(),
        })
    }

    fn cases(&self) -> usize {
        self.rows.len()
    }

    /// The covariance matrix of the totals of w u for each u.
    fn covariance(&self, us: &[Vec<f64>]) -> Matrix {
        let k = us.len();
        let mut v = vec![vec![0.0; k]; k];
        for stage in &self.stages {
            let units = stage.unit_stratum.len();
            let strata = stage.n_units.len();
            let mut totals = vec![vec![0.0; k]; units];
            for (c, &u) in stage.unit.iter().enumerate() {
                for (j, values) in us.iter().enumerate() {
                    totals[u][j] += self.w[c] * values[c];
                }
            }
            let mut sums = vec![vec![0.0; k]; strata];
            for (u, t) in totals.iter().enumerate() {
                let h = stage.unit_stratum[u];
                for j in 0..k {
                    sums[h][j] += t[j];
                }
            }
            for (u, t) in totals.iter().enumerate() {
                let h = stage.unit_stratum[u];
                let nh = stage.n_units[h] as f64;
                if nh < 2.0 {
                    continue;
                }
                let factor = stage.mult[h] * stage.fpc[h] * nh / (nh - 1.0);
                let d: Vec<f64> = (0..k).map(|j| t[j] - sums[h][j] / nh).collect();
                for a in 0..k {
                    for b in 0..k {
                        v[a][b] += factor * d[a] * d[b];
                    }
                }
            }
        }
        v
    }

    fn variance(&self, u: &[f64]) -> f64 {
        self.covariance(&[u.to_vec()])[0][0]
    }

    /// The variance under simple random sampling of the same linearized values, if defined.
    fn srs_variance(&self, u: &[f64]) -> Option<f64> {
        let n = self.cases() as f64;
        let big_n = self.total_w;
        let fpc = if self.srs_wr {
            1.0
        } else if n / big_n < 1.0 {
            1.0 - n / big_n
        } else {
            return None;
        };
        if n < 2.0 {
            return None;
        }
        let mean = self.w.iter().zip(u).map(|(w, u)| w * u).sum::<f64>() / big_n;
        let ss: f64 = self
            .w
            .iter()
            .zip(u)
            .map(|(w, u)| w * (u - mean).powi(2))
            .sum();
        Some(fpc * big_n / (n - 1.0) * ss)
    }

    fn estimate(&self, value: f64, u: &[f64], count: f64, popsize: f64) -> Estimate {
        let variance = self.variance(u);
        Estimate {
            value,
            variance,
            srs: if self.deff {
                self.srs_variance(u)
            } else {
                None
            },
            count,
            popsize,
        }
    }
}

/// One estimate with what its statistics need.
struct Estimate {
    value: f64,
    variance: f64,
    srs: Option<f64>,
    count: f64,
    popsize: f64,
}

/// How the interval of an estimate is formed.
#[derive(Clone, Copy, PartialEq)]
enum Scale {
    Plain,
    /// A proportion shown as a percent: logit interval, values times 100.
    Percent,
}

/// The statistic columns: (group, leaf).
fn stat_columns(s: &CsStatistics, test: bool, descriptives: bool) -> Vec<(String, String)> {
    let mut out = vec![(String::new(), "Estimate".to_string())];
    if s.se {
        out.push((String::new(), "Standard Error".into()));
    }
    if let Some(level) = s.cin {
        let group = format!("{level}% Confidence Interval");
        out.push((group.clone(), "Lower".into()));
        out.push((group, "Upper".into()));
    }
    if test {
        for leaf in ["Test Value", "t", "df", "Sig."] {
            out.push(("Hypothesis Test".into(), leaf.into()));
        }
    }
    if s.deff {
        out.push((String::new(), "Design Effect".into()));
    }
    if s.deffsqrt {
        out.push((String::new(), "Square Root of Design Effect".into()));
    }
    if s.cv {
        out.push((String::new(), "Coefficient of Variation".into()));
    }
    if descriptives && s.count {
        out.push((String::new(), "Unweighted Count".into()));
    }
    if descriptives && s.popsize {
        out.push((String::new(), "Population Size".into()));
    }
    out
}

/// The cells of `stat_columns` for one estimate; `test` is the hypothesis value when the
/// table has a test column group.
fn stat_cells(
    e: Option<&Estimate>,
    s: &CsStatistics,
    df: f64,
    scale: Scale,
    test: Option<Option<f64>>,
    descriptives: bool,
) -> Vec<Cell> {
    let mut out = Vec::new();
    let factor = if scale == Scale::Percent { 100.0 } else { 1.0 };
    let value = e.map(|e| e.value);
    let se = e.map(|e| e.variance.max(0.0).sqrt());
    out.push(number(value.map(|v| v * factor)));
    if s.se {
        out.push(number(se.map(|v| v * factor)));
    }
    if let Some(level) = s.cin {
        let bounds = match (value, se) {
            (Some(v), Some(se)) if df > 0.0 => {
                let t = t_quantile((1.0 - level / 100.0) / 2.0, df);
                match scale {
                    Scale::Plain => Some((v - t * se, v + t * se)),
                    Scale::Percent if v > 0.0 && v < 1.0 => {
                        let logit = (v / (1.0 - v)).ln();
                        let half = t * se / (v * (1.0 - v));
                        let back = |x: f64| 1.0 / (1.0 + (-x).exp());
                        Some((back(logit - half), back(logit + half)))
                    }
                    Scale::Percent => None,
                }
            }
            _ => None,
        };
        out.push(number(bounds.map(|b| b.0 * factor)));
        out.push(number(bounds.map(|b| b.1 * factor)));
    }
    if let Some(theta) = test {
        match (theta, value, se) {
            (Some(theta), Some(v), Some(se)) if se > 0.0 && df > 0.0 => {
                let t = (v - theta) / se;
                out.extend([
                    Cell::Number(theta),
                    Cell::Number(t),
                    Cell::Number(df),
                    Cell::Number(t_p(t, df)),
                ]);
            }
            (theta, ..) => {
                out.extend([number(theta), Cell::Empty, Cell::Empty, Cell::Empty]);
            }
        }
    }
    let deff = e.and_then(|e| e.srs.filter(|v| *v > 0.0).map(|v| e.variance / v));
    if s.deff {
        out.push(number(deff));
    }
    if s.deffsqrt {
        out.push(number(deff.map(f64::sqrt)));
    }
    if s.cv {
        out.push(number(match (value, se) {
            (Some(v), Some(se)) if v != 0.0 => Some(se / v),
            _ => None,
        }));
    }
    if descriptives && s.count {
        out.push(number(e.map(|e| e.count)));
    }
    if descriptives && s.popsize {
        out.push(number(e.map(|e| e.popsize)));
    }
    out
}

/// A subpopulation: its category texts and which design cases are in it.
struct Domain {
    labels: Vec<String>,
    inside: Vec<bool>,
}

/// The subpopulations of SUBPOP in category order, or the whole sample.
fn domains(
    data: &Dataset,
    design: &Design,
    subpop: Option<&CsSubpop>,
    include: bool,
    shown: &Shown,
) -> Result<(Vec<String>, Vec<Domain>), Error> {
    let Some(subpop) = subpop else {
        return Ok((
            Vec::new(),
            vec![Domain {
                labels: Vec::new(),
                inside: vec![true; design.cases()],
            }],
        ));
    };
    let vars: Vec<Variable> = subpop
        .variables
        .iter()
        .map(|n| data.variable(n))
        .collect::<Result<_, _>>()?;
    for v in &vars {
        let upper = v.name().to_uppercase();
        if design.plan_vars.contains(&upper) && !design.strata_clusters.contains(&upper) {
            return Err(Error(format!(
                "{} is a plan variable other than a stratum or cluster and cannot be on SUBPOP",
                v.name()
            )));
        }
    }
    let keys: Vec<Vec<Option<Key>>> = vars.iter().map(|v| keys_of(v, include)).collect();
    let mut combos: Vec<Vec<Key>> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut of_case: Vec<Option<usize>> = Vec::with_capacity(design.cases());
    for &case in &design.rows {
        let combo: Option<Vec<Key>> = keys.iter().map(|k| k[case].clone()).collect();
        of_case.push(combo.map(|combo| {
            let hash: Vec<String> = combo.iter().map(key_part).collect();
            *index.entry(hash.join("\u{1}")).or_insert_with(|| {
                combos.push(combo);
                combos.len() - 1
            })
        }));
    }
    if combos.is_empty() {
        return Err(Error(
            "SUBPOP: no case in the design has valid values on every SUBPOP variable".into(),
        ));
    }
    let mut order: Vec<usize> = (0..combos.len()).collect();
    order.sort_by(|&a, &b| {
        combos[a]
            .iter()
            .zip(&combos[b])
            .map(|(x, y)| x.compare(y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let names = vars.iter().map(|v| shown.variable(v)).collect();
    let out = order
        .into_iter()
        .map(|i| Domain {
            labels: vars
                .iter()
                .zip(&combos[i])
                .map(|(v, k)| shown.value(v, k))
                .collect(),
            inside: of_case.iter().map(|c| *c == Some(i)).collect(),
        })
        .collect();
    Ok((names, out))
}

/// The design cases' valid numbers of a variable.
fn design_numbers(data: &Dataset, design: &Design, name: &str) -> Result<Vec<Option<f64>>, Error> {
    let variable = data.variable(name)?;
    if variable.is_string() {
        return Err(Error(format!(
            "{name} is a string variable; complex samples statistics need numbers"
        )));
    }
    let values = numbers(&variable)?;
    Ok(design.rows.iter().map(|&c| values[c]).collect())
}

/// The TTEST value of item `i` of `n` from a list of one value or one per item.
fn test_value(values: &[f64], i: usize, n: usize, what: &str) -> Result<Option<f64>, Error> {
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(values[0])),
        k if k == n => Ok(Some(values[i])),
        k => Err(Error(format!(
            "TTEST of {what} gives {k} values for {n} estimates; give one value or one per estimate"
        ))),
    }
}

impl Engine {
    fn read_plan(&self, file: &str) -> Result<PlanFile, Error> {
        let path = self.path(file);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| Error(format!("cannot read the plan {}: {e}", path.display())))?;
        let plan: PlanFile = serde_json::from_str(&content).map_err(|e| {
            Error(format!(
                "{} is not a Statscraft analysis plan (written by CSPLAN ANALYSIS): {e}",
                path.display()
            ))
        })?;
        if plan.format != FORMAT || plan.version != 1 {
            return Err(Error(format!(
                "{} is not a Statscraft analysis plan of version 1",
                path.display()
            )));
        }
        if plan.stages.is_empty() || plan.stages.len() > 3 {
            return Err(Error(format!(
                "the plan {} needs one to three stages",
                path.display()
            )));
        }
        for stage in &plan.stages {
            if !["EQUAL_WOR", "UNEQUAL_WOR", "WR"].contains(&stage.estimator.as_str()) {
                return Err(Error(format!(
                    "the plan {} has an unknown estimator {}",
                    path.display(),
                    stage.estimator
                )));
            }
        }
        Ok(plan)
    }

    pub(crate) fn csplan(&mut self, command: &Csplan) -> Result<Vec<Table>, Error> {
        let plan = match command.mode {
            CsplanMode::View => self.read_plan(&command.file)?,
            CsplanMode::Analysis => {
                let plan = plan_of(command);
                let path = self.path(&command.file);
                let json = serde_json::to_string_pretty(&plan).expect("a plan serializes");
                std::fs::write(&path, json + "\n")
                    .map_err(|e| Error(format!("cannot write the plan {}: {e}", path.display())))?;
                for note in &command.ignored {
                    self.warn(format!("CSPLAN: {note}"));
                }
                if let Some(data) = &self.dataset
                    && (data.weight_variable().is_some() || data.split_file().is_some())
                {
                    self.warn("CSPLAN ignores WEIGHT and SPLIT FILE: the plan names its own weight, and the complex samples procedures do not split");
                }
                plan
            }
        };
        Ok(plan_tables(
            &plan,
            &command.file,
            command.print_plan,
            command.print_matrix,
        ))
    }

    pub(crate) fn csdescriptives(&mut self, c: &Csdescriptives) -> Result<Vec<Table>, Error> {
        let plan = self.read_plan(&c.plan)?;
        let data = self.active()?;
        let shown = self.shown();
        let deff = c.statistics.deff || c.statistics.deffsqrt;
        let design = Design::new(data, &plan, c.include, deff)?;
        let summary = if c.mean.is_some() || c.sum.is_some() {
            self.expand(&c.summary)?
        } else {
            Vec::new()
        };
        if let Some(name) = duplicate(&summary) {
            return Err(Error(format!("{name} appears twice on SUMMARY")));
        }
        // (numerator, denominator, test value) per ratio, denominators fastest
        let mut ratios: Vec<(String, String, Option<f64>)> = Vec::new();
        for r in &c.ratios {
            let numerators = self.expand(&r.numerators)?;
            let denominators = self.expand(&r.denominators)?;
            if let Some(name) = duplicate(&numerators).or(duplicate(&denominators)) {
                return Err(Error(format!("{name} appears twice in a RATIO list")));
            }
            let n = numerators.len() * denominators.len();
            let mut i = 0;
            for num in &numerators {
                for den in &denominators {
                    let test = test_value(r.test.as_deref().unwrap_or(&[]), i, n, "RATIO")?;
                    ratios.push((num.clone(), den.clone(), test));
                    i += 1;
                }
            }
        }
        let subpop_names: Vec<String> = c
            .subpop
            .iter()
            .flat_map(|s| &s.variables)
            .map(|v| v.to_uppercase())
            .collect();
        let analysis: Vec<String> = summary
            .iter()
            .chain(ratios.iter().flat_map(|(a, b, _)| [a, b]))
            .cloned()
            .collect();
        for name in &analysis {
            let upper = name.to_uppercase();
            if design.plan_vars.contains(&upper) {
                return Err(Error(format!(
                    "{name} is a plan variable and cannot be analyzed"
                )));
            }
            if subpop_names.contains(&upper) {
                return Err(Error(format!("{name} is on SUBPOP and cannot be analyzed")));
            }
        }
        let mut values: HashMap<String, Vec<Option<f64>>> = HashMap::new();
        for name in &analysis {
            if !values.contains_key(name) {
                values.insert(name.clone(), design_numbers(data, &design, name)?);
            }
        }
        let n = design.cases();
        let listwise: Vec<bool> = (0..n)
            .map(|i| values.values().all(|v| v[i].is_some()))
            .collect();
        let (subpop_labels, mut domains) =
            domains(data, &design, c.subpop.as_ref(), c.include, &shown)?;
        if c.subpop.is_some() {
            // the whole population too, as the manual's t tests imply
            domains.push(Domain {
                labels: vec!["Total".to_string(); subpop_labels.len()],
                inside: vec![true; n],
            });
        }
        let layout = Layout {
            separate: c.subpop.as_ref().is_some_and(|s| s.separate),
            subpop_labels: &subpop_labels,
        };
        let s = &c.statistics;
        let df = design.df;
        let w = &design.w;
        let valid = |names: &[&String], i: usize| -> bool {
            if c.listwise {
                listwise[i]
            } else {
                names.iter().all(|n| values[*n][i].is_some())
            }
        };
        let label_of = |name: &str| -> String {
            data.variable(name)
                .map(|v| shown.variable(&v))
                .unwrap_or_else(|_| name.to_string())
        };
        let mut tables = Vec::new();
        let groups = layout.groups(&domains);
        // univariate statistics
        let kinds: Vec<(&str, &Vec<f64>)> = [("Mean", &c.mean), ("Sum", &c.sum)]
            .into_iter()
            .filter_map(|(k, t)| t.as_ref().map(|t| (k, t)))
            .collect();
        if !kinds.is_empty() {
            let has_test = kinds.iter().any(|(_, t)| !t.is_empty());
            for group in &groups {
                let mut rows = Vec::new();
                for d in group {
                    for (kind, tests) in &kinds {
                        for (k, name) in summary.iter().enumerate() {
                            let y = &values[name];
                            let delta: Vec<bool> =
                                (0..n).map(|i| d.inside[i] && valid(&[name], i)).collect();
                            let big_n: f64 = (0..n).filter(|&i| delta[i]).map(|i| w[i]).sum();
                            let total: f64 = (0..n)
                                .filter(|&i| delta[i])
                                .map(|i| w[i] * y[i].expect("valid"))
                                .sum();
                            let count = delta.iter().filter(|d| **d).count() as f64;
                            let e = if *kind == "Mean" {
                                (big_n != 0.0).then(|| {
                                    let mean = total / big_n;
                                    let u: Vec<f64> = (0..n)
                                        .map(|i| match delta[i] {
                                            true => (y[i].expect("valid") - mean) / big_n,
                                            false => 0.0,
                                        })
                                        .collect();
                                    design.estimate(mean, &u, count, big_n)
                                })
                            } else {
                                let u: Vec<f64> = (0..n)
                                    .map(|i| if delta[i] { y[i].expect("valid") } else { 0.0 })
                                    .collect();
                                Some(design.estimate(total, &u, count, big_n))
                            };
                            let test = test_value(tests, k, summary.len(), kind)?;
                            let mut row = layout.layer_cells(d);
                            row.push(text(label_of(name)));
                            row.push(text(*kind));
                            row.extend(stat_cells(
                                e.as_ref(),
                                s,
                                df,
                                Scale::Plain,
                                has_test.then_some(test),
                                true,
                            ));
                            rows.push(row);
                        }
                    }
                }
                let mut cols = layout.layer_columns();
                cols.push((String::new(), "Variable".into()));
                cols.push((String::new(), "Statistic".into()));
                cols.extend(stat_columns(s, has_test, true));
                tables.push(table(
                    "Univariate Statistics",
                    layout.layers() + 2,
                    cols,
                    rows,
                    layout.caption(group[0]),
                ));
            }
        }
        if !ratios.is_empty() {
            let has_test = ratios.iter().any(|r| r.2.is_some());
            for group in &groups {
                let mut rows = Vec::new();
                for d in group {
                    for (num, den, test) in &ratios {
                        let (y, x) = (&values[num], &values[den]);
                        let delta: Vec<bool> = (0..n)
                            .map(|i| d.inside[i] && valid(&[num, den], i))
                            .collect();
                        let sum = |v: &Vec<Option<f64>>| -> f64 {
                            (0..n)
                                .filter(|&i| delta[i])
                                .map(|i| w[i] * v[i].expect("valid"))
                                .sum()
                        };
                        let (ty, tx) = (sum(y), sum(x));
                        let big_n: f64 = (0..n).filter(|&i| delta[i]).map(|i| w[i]).sum();
                        let count = delta.iter().filter(|d| **d).count() as f64;
                        let e = (tx != 0.0).then(|| {
                            let r = ty / tx;
                            let u: Vec<f64> = (0..n)
                                .map(|i| match delta[i] {
                                    true => (y[i].expect("valid") - r * x[i].expect("valid")) / tx,
                                    false => 0.0,
                                })
                                .collect();
                            design.estimate(r, &u, count, big_n)
                        });
                        let mut row = layout.layer_cells(d);
                        row.push(text(label_of(num)));
                        row.push(text(label_of(den)));
                        row.extend(stat_cells(
                            e.as_ref(),
                            s,
                            df,
                            Scale::Plain,
                            has_test.then_some(*test),
                            true,
                        ));
                        rows.push(row);
                    }
                }
                let mut cols = layout.layer_columns();
                cols.push((String::new(), "Numerator".into()));
                cols.push((String::new(), "Denominator".into()));
                cols.extend(stat_columns(s, has_test, true));
                tables.push(table(
                    "Ratios",
                    layout.layers() + 2,
                    cols,
                    rows,
                    layout.caption(group[0]),
                ));
            }
        }
        Ok(tables)
    }

    pub(crate) fn cstabulate(&mut self, c: &Cstabulate) -> Result<Vec<Table>, Error> {
        let plan = self.read_plan(&c.plan)?;
        let data = self.active()?;
        let shown = self.shown();
        let deff = c.statistics.deff || c.statistics.deffsqrt;
        let design = Design::new(data, &plan, c.include, deff)?;
        let rows_vars = self.expand(&c.variables)?;
        let mut with_by = rows_vars.clone();
        with_by.extend(c.by.clone());
        if let Some(name) = duplicate(&with_by) {
            return Err(Error(format!("{name} appears twice on TABLES")));
        }
        let subpop_names: Vec<String> = c
            .subpop
            .iter()
            .flat_map(|s| &s.variables)
            .map(|v| v.to_uppercase())
            .collect();
        let all_vars: Vec<&String> = rows_vars.iter().chain(c.by.iter()).collect();
        let mut keys: HashMap<String, (Variable, Vec<Option<Key>>)> = HashMap::new();
        for name in &all_vars {
            let upper = name.to_uppercase();
            if design.plan_vars.contains(&upper) {
                return Err(Error(format!(
                    "{name} is a plan variable and cannot be tabulated"
                )));
            }
            if subpop_names.contains(&upper) {
                return Err(Error(format!(
                    "{name} is on SUBPOP and cannot be tabulated"
                )));
            }
            let variable = data.variable(name)?;
            let all = keys_of(&variable, c.include);
            let mine = design.rows.iter().map(|&r| all[r].clone()).collect();
            keys.insert((*name).clone(), (variable, mine));
        }
        let n = design.cases();
        let listwise: Vec<bool> = (0..n)
            .map(|i| keys.values().all(|(_, k)| k[i].is_some()))
            .collect();
        let (subpop_labels, domains) =
            domains(data, &design, c.subpop.as_ref(), c.include, &shown)?;
        let separate = c.subpop.as_ref().is_some_and(|s| s.separate);
        let strata_only = c.subpop.as_ref().is_some_and(|s| {
            s.variables
                .iter()
                .all(|v| design.first_strata.contains(&v.to_uppercase()))
        });
        let mut test_domains: Vec<Domain> = vec![Domain {
            labels: vec!["Total".to_string(); subpop_labels.len()],
            inside: vec![true; n],
        }];
        if strata_only {
            test_domains.extend(domains.iter().map(|d| Domain {
                labels: d.labels.clone(),
                inside: d.inside.clone(),
            }));
        }
        let s = &c.statistics;
        let mut tables = Vec::new();
        let mut warnings = Vec::new();
        let two_way = c.by.is_some();
        if !two_way && (s.expected || s.resid || s.asresid) {
            warnings.push(
                "CSTABULATE: EXPECTED, RESID and ASRESID apply to two-way tables only".to_string(),
            );
        }
        if two_way && s.cumulative {
            warnings.push("CSTABULATE: CUMULATIVE applies to one-way tables only".to_string());
        }
        if two_way && c.tests.homogeneity {
            warnings.push("CSTABULATE: HOMOGENEITY applies to one-way tables only".to_string());
        }
        if !two_way
            && (c.tests.independence || c.tests.oddsratio || c.tests.relrisk || c.tests.riskdiff)
        {
            warnings.push(
                "CSTABULATE: INDEPENDENCE, ODDSRATIO, RELRISK and RISKDIFF apply to two-way tables only"
                    .to_string(),
            );
        }
        if two_way && (s.expected || s.resid || s.asresid) && !c.cells.tablepct {
            warnings.push(
                "CSTABULATE: EXPECTED, RESID and ASRESID are shown only with CELLS TABLEPCT"
                    .to_string(),
            );
        }
        if c.subpop.is_some() && !strata_only && c.tests != Default::default() {
            warnings.push(
                "CSTABULATE: SUBPOP is not made of first-stage strata, so the tests are for the whole population only"
                    .to_string(),
            );
        }
        let ctx = Tabulation {
            design: &design,
            s,
            layout: Layout {
                separate,
                subpop_labels: &subpop_labels,
            },
            subpop_labels: &subpop_labels,
        };
        for row_name in &rows_vars {
            let (row_var, row_keys) = &keys[row_name];
            let col = c.by.as_ref().map(|b| &keys[b]);
            let valid: Vec<bool> = (0..n)
                .map(|i| {
                    if c.listwise {
                        listwise[i]
                    } else {
                        row_keys[i].is_some() && col.is_none_or(|(_, k)| k[i].is_some())
                    }
                })
                .collect();
            let cats = |k: &Vec<Option<Key>>| -> (Vec<Key>, Vec<Option<usize>>) {
                let mut seen: HashMap<String, Key> = HashMap::new();
                for (i, key) in k.iter().enumerate() {
                    if let Some(key) = key
                        && valid[i]
                    {
                        seen.entry(key_part(key)).or_insert_with(|| key.clone());
                    }
                }
                let mut found: Vec<Key> = seen.into_values().collect();
                found.sort_by(|a, b| a.compare(b));
                let position: HashMap<String, usize> = found
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (key_part(f), i))
                    .collect();
                let index = k
                    .iter()
                    .enumerate()
                    .map(|(i, key)| {
                        key.as_ref()
                            .filter(|_| valid[i])
                            .map(|key| position[&key_part(key)])
                    })
                    .collect();
                (found, index)
            };
            let (row_cats, row_index) = cats(row_keys);
            let row_texts: Vec<String> = row_cats.iter().map(|k| shown.value(row_var, k)).collect();
            match col {
                None => {
                    tables.extend(ctx.one_way(
                        &shown.variable(row_var),
                        &row_texts,
                        &row_index,
                        &domains,
                        &c.cells,
                    ));
                    if c.tests.homogeneity {
                        tables.push(ctx.homogeneity(
                            &shown.variable(row_var),
                            row_texts.len(),
                            &row_index,
                            &test_domains,
                            &mut warnings,
                        ));
                    }
                }
                Some((col_var, col_keys)) => {
                    let (col_cats, col_index) = cats(col_keys);
                    let col_texts: Vec<String> =
                        col_cats.iter().map(|k| shown.value(col_var, k)).collect();
                    let names = [shown.variable(row_var), shown.variable(col_var)];
                    let cells: Vec<Option<(usize, usize)>> =
                        (0..n).map(|i| row_index[i].zip(col_index[i])).collect();
                    tables.extend(ctx.two_way(
                        &names,
                        [&row_texts, &col_texts],
                        &cells,
                        &domains,
                        &c.cells,
                    ));
                    if c.tests.independence {
                        tables.push(ctx.independence(
                            &names,
                            [row_texts.len(), col_texts.len()],
                            &cells,
                            &test_domains,
                            &mut warnings,
                        ));
                    }
                    if c.tests.oddsratio || c.tests.relrisk || c.tests.riskdiff {
                        if row_texts.len() == 2 && col_texts.len() == 2 {
                            tables.push(ctx.risks(
                                &names,
                                &col_texts,
                                &cells,
                                &test_domains,
                                &c.tests,
                                &mut warnings,
                            ));
                        } else {
                            warnings.push(format!(
                                "CSTABULATE: {} by {} is not a 2 x 2 table, so it has no odds ratios or risks",
                                names[0], names[1]
                            ));
                        }
                    }
                }
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }
}

/// How subpopulations are laid out: as leading row headers (LAYERED) or one table each
/// (SEPARATE).
struct Layout<'a> {
    separate: bool,
    subpop_labels: &'a [String],
}

impl Layout<'_> {
    fn layer_cells(&self, d: &Domain) -> Vec<Cell> {
        if self.separate {
            Vec::new()
        } else {
            d.labels.iter().map(text).collect()
        }
    }

    fn layer_columns(&self) -> Vec<(String, String)> {
        if self.separate {
            Vec::new()
        } else {
            self.subpop_labels
                .iter()
                .map(|l| (String::new(), l.clone()))
                .collect()
        }
    }

    fn caption(&self, d: &Domain) -> Option<String> {
        self.separate.then(|| {
            let parts: Vec<String> = self
                .subpop_labels
                .iter()
                .zip(&d.labels)
                .map(|(n, v)| format!("{n} = {v}"))
                .collect();
            format!("Subpopulation: {}", parts.join(", "))
        })
    }

    fn layers(&self) -> usize {
        if self.separate {
            0
        } else {
            self.subpop_labels.len()
        }
    }

    fn groups<'d>(&self, domains: &'d [Domain]) -> Vec<Vec<&'d Domain>> {
        if self.separate {
            domains.iter().map(|d| vec![d]).collect()
        } else {
            vec![domains.iter().collect()]
        }
    }
}

/// What every CSTABULATE table shares.
struct Tabulation<'a> {
    design: &'a Design,
    s: &'a CsStatistics,
    layout: Layout<'a>,
    subpop_labels: &'a [String],
}

/// The cell kinds of CELLS that a table shows: (column group, base of the percent).
#[derive(Clone, Copy, PartialEq)]
enum Base {
    /// A population size.
    Size,
    Row,
    Column,
    Table,
}

impl Tabulation<'_> {
    fn n(&self) -> usize {
        self.design.cases()
    }

    /// A population size in a set of cases.
    fn size(&self, inside: &[bool]) -> Estimate {
        let u: Vec<f64> = inside.iter().map(|&b| f64::from(u8::from(b))).collect();
        let total: f64 = (0..self.n())
            .filter(|&i| inside[i])
            .map(|i| self.design.w[i])
            .sum();
        let count = inside.iter().filter(|b| **b).count() as f64;
        self.design.estimate(total, &u, count, total)
    }

    /// The proportion of `base` that is in `part` (part inside base); None for an empty base.
    fn proportion(&self, part: &[bool], base: &[bool]) -> Option<Estimate> {
        let w = &self.design.w;
        let nb: f64 = (0..self.n()).filter(|&i| base[i]).map(|i| w[i]).sum();
        if nb == 0.0 {
            return None;
        }
        let np: f64 = (0..self.n()).filter(|&i| part[i]).map(|i| w[i]).sum();
        let p = np / nb;
        let u: Vec<f64> = (0..self.n())
            .map(|i| (f64::from(u8::from(part[i])) - f64::from(u8::from(base[i])) * p) / nb)
            .collect();
        let count = part.iter().filter(|b| **b).count() as f64;
        Some(self.design.estimate(p, &u, count, np))
    }

    /// The columns of one cell kind.
    fn kind_columns(&self, group: &str, cumulative: bool, extra: &[&str]) -> Vec<(String, String)> {
        let mut cols: Vec<(String, String)> = stat_columns(self.s, false, false)
            .into_iter()
            .map(|(g, leaf)| {
                let full = if g.is_empty() {
                    group.to_string()
                } else {
                    format!("{group} / {g}")
                };
                (full, leaf)
            })
            .collect();
        if cumulative {
            cols.push((group.to_string(), "Cumulative".into()));
        }
        for leaf in extra {
            cols.push((group.to_string(), leaf.to_string()));
        }
        cols
    }

    fn one_way(
        &self,
        title: &str,
        cats: &[String],
        index: &[Option<usize>],
        domains: &[Domain],
        cells: &statscraft_syntax::CsCells,
    ) -> Vec<Table> {
        let n = self.n();
        let mut kinds = Vec::new();
        if cells.popsize {
            kinds.push((Base::Size, "Population Size"));
        }
        if cells.rowpct || cells.colpct || cells.tablepct {
            kinds.push((Base::Table, "Table Percent"));
        }
        let mut out = Vec::new();
        for group in self.layout.groups(domains) {
            let mut rows = Vec::new();
            for d in &group {
                let base: Vec<bool> = (0..n).map(|i| d.inside[i] && index[i].is_some()).collect();
                let mut running = vec![0.0; kinds.len()];
                for r in 0..=cats.len() {
                    let part: Vec<bool> = if r < cats.len() {
                        (0..n).map(|i| base[i] && index[i] == Some(r)).collect()
                    } else {
                        base.clone()
                    };
                    let mut row = self.layout.layer_cells(d);
                    row.push(text(cats.get(r).map_or("Total", String::as_str)));
                    for (k, (kind, _)) in kinds.iter().enumerate() {
                        let (e, scale) = match kind {
                            Base::Size => (Some(self.size(&part)), Scale::Plain),
                            _ => (self.proportion(&part, &base), Scale::Percent),
                        };
                        row.extend(stat_cells(
                            e.as_ref(),
                            self.s,
                            self.design.df,
                            scale,
                            None,
                            false,
                        ));
                        if self.s.cumulative {
                            let factor = if scale == Scale::Percent { 100.0 } else { 1.0 };
                            if r < cats.len() {
                                running[k] += e.as_ref().map_or(0.0, |e| e.value * factor);
                                row.push(Cell::Number(running[k]));
                            } else {
                                row.push(Cell::Empty);
                            }
                        }
                    }
                    if self.s.count {
                        row.push(Cell::Number(part.iter().filter(|b| **b).count() as f64));
                    }
                    rows.push(row);
                }
            }
            let mut cols = self.layout.layer_columns();
            cols.push((String::new(), title.to_string()));
            for (_, name) in &kinds {
                cols.extend(self.kind_columns(name, self.s.cumulative, &[]));
            }
            if self.s.count {
                cols.push((String::new(), "Unweighted Count".into()));
            }
            out.push(table(
                title,
                self.layout.layers() + 1,
                cols,
                rows,
                self.layout.caption(group[0]),
            ));
        }
        out
    }

    fn two_way(
        &self,
        names: &[String; 2],
        cats: [&Vec<String>; 2],
        cell_of: &[Option<(usize, usize)>],
        domains: &[Domain],
        cells: &statscraft_syntax::CsCells,
    ) -> Vec<Table> {
        let n = self.n();
        let (nr, nc) = (cats[0].len(), cats[1].len());
        let mut kinds = Vec::new();
        if cells.popsize {
            kinds.push((Base::Size, "Population Size"));
        }
        if cells.rowpct {
            kinds.push((Base::Row, "Row Percent"));
        }
        if cells.colpct {
            kinds.push((Base::Column, "Column Percent"));
        }
        if cells.tablepct {
            kinds.push((Base::Table, "Table Percent"));
        }
        let mut extra: Vec<&str> = Vec::new();
        if self.s.expected {
            extra.push("Expected");
        }
        if self.s.resid {
            extra.push("Residual");
        }
        if self.s.asresid {
            extra.push("Adjusted Residual");
        }
        let in_cell = |r: Option<usize>, c: Option<usize>, d: &Domain| -> Vec<bool> {
            (0..n)
                .map(|i| {
                    d.inside[i]
                        && cell_of[i].is_some_and(|(a, b)| {
                            r.is_none_or(|r| r == a) && c.is_none_or(|c| c == b)
                        })
                })
                .collect()
        };
        let mut out = Vec::new();
        for group in self.layout.groups(domains) {
            let mut rows = Vec::new();
            for d in &group {
                let residuals = (!extra.is_empty() && cells.tablepct)
                    .then(|| self.residuals(nr, nc, cell_of, d));
                for r in 0..=nr {
                    for c in 0..=nc {
                        let (ro, co) = ((r < nr).then_some(r), (c < nc).then_some(c));
                        let part = in_cell(ro, co, d);
                        let mut row = self.layout.layer_cells(d);
                        row.push(text(cats[0].get(r).map_or("Total", String::as_str)));
                        row.push(text(cats[1].get(c).map_or("Total", String::as_str)));
                        for (kind, _) in &kinds {
                            let (e, scale) = match kind {
                                Base::Size => (Some(self.size(&part)), Scale::Plain),
                                Base::Row => (
                                    self.proportion(&part, &in_cell(ro, None, d)),
                                    Scale::Percent,
                                ),
                                Base::Column => (
                                    self.proportion(&part, &in_cell(None, co, d)),
                                    Scale::Percent,
                                ),
                                Base::Table => (
                                    self.proportion(&part, &in_cell(None, None, d)),
                                    Scale::Percent,
                                ),
                            };
                            row.extend(stat_cells(
                                e.as_ref(),
                                self.s,
                                self.design.df,
                                scale,
                                None,
                                false,
                            ));
                            if *kind == Base::Table && !extra.is_empty() {
                                let at = ro.zip(co).map(|(a, b)| a * nc + b);
                                let res = residuals.as_ref().and_then(|v| v.as_ref());
                                let value = |k: usize| -> Cell {
                                    number(at.and_then(|i| res.map(|v| v[k][i])))
                                };
                                if self.s.expected {
                                    row.push(value(0));
                                }
                                if self.s.resid {
                                    row.push(value(1));
                                }
                                if self.s.asresid {
                                    row.push(value(2));
                                }
                            }
                        }
                        if self.s.count {
                            row.push(Cell::Number(part.iter().filter(|b| **b).count() as f64));
                        }
                        rows.push(row);
                    }
                }
            }
            let mut cols = self.layout.layer_columns();
            cols.push((String::new(), names[0].clone()));
            cols.push((String::new(), names[1].clone()));
            for (kind, name) in &kinds {
                let e: &[&str] = if *kind == Base::Table { &extra } else { &[] };
                cols.extend(self.kind_columns(name, false, e));
            }
            if self.s.count {
                cols.push((String::new(), "Unweighted Count".into()));
            }
            let title = format!("{} * {}", names[0], names[1]);
            out.push(table(
                &title,
                self.layout.layers() + 2,
                cols,
                rows,
                self.layout.caption(group[0]),
            ));
        }
        out
    }

    /// The table proportions of a two-way table in a domain, row-major, with their design
    /// covariance and the unweighted count; None when the domain is empty.
    fn table_proportions(
        &self,
        nr: usize,
        nc: usize,
        cell_of: &[Option<(usize, usize)>],
        d: &Domain,
    ) -> Option<(Vec<f64>, Matrix, f64)> {
        let n = self.n();
        let w = &self.design.w;
        let mut totals = vec![0.0; nr * nc];
        let mut base = 0.0;
        let mut count = 0.0;
        for i in 0..n {
            if let (true, Some((r, c))) = (d.inside[i], cell_of[i]) {
                totals[r * nc + c] += w[i];
                base += w[i];
                count += 1.0;
            }
        }
        if base == 0.0 {
            return None;
        }
        let p: Vec<f64> = totals.iter().map(|t| t / base).collect();
        let us: Vec<Vec<f64>> = (0..nr * nc)
            .map(|k| {
                (0..n)
                    .map(|i| match (d.inside[i], cell_of[i]) {
                        (true, Some((r, c))) => {
                            (f64::from(u8::from(r * nc + c == k)) - p[k]) / base
                        }
                        _ => 0.0,
                    })
                    .collect()
            })
            .collect();
        Some((p, self.design.covariance(&us), count))
    }

    /// Expected table proportions, residuals and adjusted residuals of every inner cell.
    fn residuals(
        &self,
        nr: usize,
        nc: usize,
        cell_of: &[Option<(usize, usize)>],
        d: &Domain,
    ) -> Option<[Vec<f64>; 3]> {
        let (p, v, _) = self.table_proportions(nr, nc, cell_of, d)?;
        let k = nr * nc;
        let row: Vec<f64> = (0..nr)
            .map(|r| (0..nc).map(|c| p[r * nc + c]).sum())
            .collect();
        let col: Vec<f64> = (0..nc)
            .map(|c| (0..nr).map(|r| p[r * nc + c]).sum())
            .collect();
        let expected: Vec<f64> = (0..k).map(|i| row[i / nc] * col[i % nc]).collect();
        let resid: Vec<f64> = (0..k).map(|i| p[i] - expected[i]).collect();
        let m = multinomial(&p);
        let x = independence_design(nr, nc);
        let xt = transpose(&x);
        let adjusted = inverse(&multiply(&multiply(&xt, &m), &x)).map(|(inv, _)| {
            // I - M X (X'MX)^-1 X'
            let mx = multiply(&m, &x);
            let proj = multiply(&multiply(&mx, &inv), &xt);
            let left: Matrix = (0..k)
                .map(|a| {
                    (0..k)
                        .map(|b| f64::from(u8::from(a == b)) - proj[a][b])
                        .collect()
                })
                .collect();
            let var = multiply(&multiply(&left, &v), &transpose(&left));
            (0..k)
                .map(|i| match var[i][i] {
                    vi if vi > 0.0 => resid[i] / vi.sqrt(),
                    _ => f64::NAN,
                })
                .collect::<Vec<f64>>()
        });
        let adjusted = adjusted.unwrap_or_else(|| vec![f64::NAN; k]);
        Some([
            expected.iter().map(|e| e * 100.0).collect(),
            resid.iter().map(|r| r * 100.0).collect(),
            adjusted,
        ])
    }

    fn test_table(&self, title: &str, caption: String, rows: Vec<Vec<Cell>>) -> Table {
        let mut cols: Vec<(String, String)> = self
            .subpop_labels
            .iter()
            .map(|l| (String::new(), l.clone()))
            .collect();
        cols.push((String::new(), String::new()));
        for leaf in ["Chi-Square", "Adjusted F", "df1", "df2", "Sig."] {
            cols.push((String::new(), leaf.into()));
        }
        table(
            title,
            self.subpop_labels.len() + 1,
            cols,
            rows,
            Some(caption),
        )
    }

    /// The Pearson and likelihood ratio rows of one test from the chi-squares and the
    /// generalized design effect matrix.
    fn test_rows(&self, d: &Domain, chi: [f64; 2], delta: &Matrix) -> Vec<Vec<Cell>> {
        let k = delta.len();
        let trace: f64 = (0..k).map(|i| delta[i][i]).sum();
        let trace2: f64 = (0..k)
            .flat_map(|i| (0..k).map(move |j| (i, j)))
            .map(|(i, j)| delta[i][j] * delta[j][i])
            .sum();
        let df1 = trace * trace / trace2;
        let df2 = df1 * self.design.df;
        ["Pearson", "Likelihood Ratio"]
            .iter()
            .zip(chi)
            .map(|(name, x)| {
                let f = x / trace;
                let mut row: Vec<Cell> = d.labels.iter().map(text).collect();
                row.push(text(*name));
                let ok = trace > 0.0 && df2 > 0.0;
                row.extend([
                    Cell::Number(x),
                    number(ok.then_some(f)),
                    number(ok.then_some(df1)),
                    number(ok.then_some(df2)),
                    number(ok.then(|| f_p(f, df1, df2))),
                ]);
                row
            })
            .collect()
    }

    fn empty_test_rows(&self, d: &Domain) -> Vec<Vec<Cell>> {
        ["Pearson", "Likelihood Ratio"]
            .iter()
            .map(|name| {
                let mut row: Vec<Cell> = d.labels.iter().map(text).collect();
                row.push(text(*name));
                row.extend(std::iter::repeat_n(Cell::Empty, 5));
                row
            })
            .collect()
    }

    fn independence(
        &self,
        names: &[String; 2],
        dims: [usize; 2],
        cell_of: &[Option<(usize, usize)>],
        domains: &[Domain],
        warnings: &mut Vec<String>,
    ) -> Table {
        let [nr, nc] = dims;
        let mut rows = Vec::new();
        for d in domains {
            let computed = (nr >= 2 && nc >= 2)
                .then(|| self.table_proportions(nr, nc, cell_of, d))
                .flatten()
                .filter(|(p, ..)| p.iter().all(|&x| x > 0.0))
                .and_then(|(p, v, n)| {
                    let row: Vec<f64> = (0..nr)
                        .map(|r| (0..nc).map(|c| p[r * nc + c]).sum())
                        .collect();
                    let col: Vec<f64> = (0..nc)
                        .map(|c| (0..nr).map(|r| p[r * nc + c]).sum())
                        .collect();
                    let mut x2 = 0.0;
                    let mut g2 = 0.0;
                    for r in 0..nr {
                        for c in 0..nc {
                            let e = row[r] * col[c];
                            let o = p[r * nc + c];
                            x2 += (o - e).powi(2) / e;
                            g2 += o * (o / e).ln();
                        }
                    }
                    let contrast = kronecker(&contrasts(nr), &contrasts(nc));
                    let delta = generalized_deffs(&contrast, &p, &v, n)?;
                    Some(([n * x2, 2.0 * n * g2], delta))
                });
            match computed {
                Some((chi, delta)) => rows.extend(self.test_rows(d, chi, &delta)),
                None => {
                    warnings.push(format!(
                        "CSTABULATE: the test of independence of {} by {} needs at least two rows and columns and no empty cell",
                        names[0], names[1]
                    ));
                    rows.extend(self.empty_test_rows(d));
                }
            }
        }
        self.test_table(
            "Tests of Independence",
            format!("{} * {}", names[0], names[1]),
            rows,
        )
    }

    fn homogeneity(
        &self,
        name: &str,
        nr: usize,
        index: &[Option<usize>],
        domains: &[Domain],
        warnings: &mut Vec<String>,
    ) -> Table {
        let cell_of: Vec<Option<(usize, usize)>> =
            index.iter().map(|r| r.map(|r| (r, 0))).collect();
        let mut rows = Vec::new();
        for d in domains {
            let computed = (nr >= 2)
                .then(|| self.table_proportions(nr, 1, &cell_of, d))
                .flatten()
                .filter(|(p, ..)| p.iter().all(|&x| x > 0.0))
                .and_then(|(p, v, n)| {
                    let r = nr as f64;
                    let x2 = n * r * p.iter().map(|x| (x - 1.0 / r).powi(2)).sum::<f64>();
                    let g2 = 2.0 * n * p.iter().map(|x| x * (r * x).ln()).sum::<f64>();
                    // the first R - 1 proportions
                    let contrast: Matrix = (0..nr)
                        .map(|i| (0..nr - 1).map(|j| f64::from(u8::from(i == j))).collect())
                        .collect();
                    let p0: Vec<f64> = p[..nr - 1].to_vec();
                    let m: Matrix = (0..nr - 1)
                        .map(|a| {
                            (0..nr - 1)
                                .map(|b| f64::from(u8::from(a == b)) * p0[a] - p0[a] * p0[b])
                                .collect()
                        })
                        .collect();
                    let ct = transpose(&contrast);
                    let v0 = multiply(&multiply(&ct, &v), &contrast);
                    let (minv, _) = inverse(&m)?;
                    let delta: Matrix = multiply(&minv, &v0)
                        .into_iter()
                        .map(|row| row.into_iter().map(|x| x * n).collect())
                        .collect();
                    Some(([x2, g2], delta))
                });
            match computed {
                Some((chi, delta)) => rows.extend(self.test_rows(d, chi, &delta)),
                None => {
                    warnings.push(format!(
                        "CSTABULATE: the test of homogeneity of {name} needs at least two categories and no empty one"
                    ));
                    rows.extend(self.empty_test_rows(d));
                }
            }
        }
        self.test_table("Tests of Homogeneity", name.to_string(), rows)
    }

    fn risks(
        &self,
        names: &[String; 2],
        col_texts: &[String],
        cell_of: &[Option<(usize, usize)>],
        domains: &[Domain],
        tests: &statscraft_syntax::CsTests,
        warnings: &mut Vec<String>,
    ) -> Table {
        let n = self.n();
        let w = &self.design.w;
        let mut rows = Vec::new();
        let stats = CsStatistics {
            se: true,
            cin: Some(self.s.cin.unwrap_or(95.0)),
            ..CsStatistics::default()
        };
        for d in domains {
            let mut sizes = [[0.0; 2]; 2];
            for i in 0..n {
                if let (true, Some((r, c))) = (d.inside[i], cell_of[i]) {
                    sizes[r][c] += w[i];
                }
            }
            let row_total = [sizes[0][0] + sizes[0][1], sizes[1][0] + sizes[1][1]];
            let usable = sizes.iter().flatten().all(|&s| s > 0.0);
            if !usable {
                warnings.push(format!(
                    "CSTABULATE: {} by {} has an empty cell, so the odds ratio and risks are not computed",
                    names[0], names[1]
                ));
            }
            // the indicator of cell (r, c) of case i
            let at = |i: usize, r: usize, c: usize| -> f64 {
                f64::from(u8::from(d.inside[i] && cell_of[i] == Some((r, c))))
            };
            let mut measures: Vec<(String, Option<Estimate>, bool)> = Vec::new();
            let make = |value: f64, u: Vec<f64>| self.design.estimate(value, &u, 0.0, 0.0);
            if tests.oddsratio {
                let e = usable.then(|| {
                    let or = sizes[0][0] * sizes[1][1] / (sizes[0][1] * sizes[1][0]);
                    let u = (0..n)
                        .map(|i| {
                            (at(i, 0, 0) / sizes[0][0]
                                - at(i, 0, 1) / sizes[0][1]
                                - at(i, 1, 0) / sizes[1][0]
                                + at(i, 1, 1) / sizes[1][1])
                                * or
                        })
                        .collect();
                    make(or, u)
                });
                measures.push(("Odds Ratio".into(), e, true));
            }
            for c in 0..2 {
                let o = 1 - c;
                if tests.relrisk {
                    let e = usable.then(|| {
                        let rr = (sizes[0][c] / row_total[0]) / (sizes[1][c] / row_total[1]);
                        let u = (0..n)
                            .map(|i| {
                                (at(i, 0, c) * sizes[0][o] / (sizes[0][c] * row_total[0])
                                    - at(i, 0, o) / row_total[0]
                                    - at(i, 1, c) * sizes[1][o] / (sizes[1][c] * row_total[1])
                                    + at(i, 1, o) / row_total[1])
                                    * rr
                            })
                            .collect();
                        make(rr, u)
                    });
                    measures.push((
                        format!("Relative Risk ({} = {})", names[1], col_texts[c]),
                        e,
                        true,
                    ));
                }
            }
            for c in 0..2 {
                let o = 1 - c;
                if tests.riskdiff {
                    let e = usable.then(|| {
                        let rd = sizes[0][c] / row_total[0] - sizes[1][c] / row_total[1];
                        let u = (0..n)
                            .map(|i| {
                                (at(i, 0, c) * sizes[0][o] - at(i, 0, o) * sizes[0][c])
                                    / row_total[0].powi(2)
                                    - (at(i, 1, c) * sizes[1][o] - at(i, 1, o) * sizes[1][c])
                                        / row_total[1].powi(2)
                            })
                            .collect();
                        make(rd, u)
                    });
                    measures.push((
                        format!("Risk Difference ({} = {})", names[1], col_texts[c]),
                        e,
                        false,
                    ));
                }
            }
            for (label, e, log_scale) in measures {
                let mut row: Vec<Cell> = d.labels.iter().map(text).collect();
                row.push(text(label));
                let mut cells = stat_cells(
                    e.as_ref(),
                    &stats,
                    self.design.df,
                    Scale::Plain,
                    None,
                    false,
                );
                if log_scale && let Some(e) = &e {
                    // the interval of a ratio on the log scale
                    let se = e.variance.max(0.0).sqrt();
                    let level = stats.cin.expect("set");
                    if self.design.df > 0.0 && e.value > 0.0 {
                        let t = t_quantile((1.0 - level / 100.0) / 2.0, self.design.df);
                        let half = t * se / e.value;
                        cells[2] = Cell::Number((e.value.ln() - half).exp());
                        cells[3] = Cell::Number((e.value.ln() + half).exp());
                    }
                }
                row.extend(cells);
                rows.push(row);
            }
        }
        let mut cols: Vec<(String, String)> = self
            .subpop_labels
            .iter()
            .map(|l| (String::new(), l.clone()))
            .collect();
        cols.push((String::new(), String::new()));
        cols.extend(stat_columns(&stats, false, false));
        table(
            "Odds Ratios and Risks",
            self.subpop_labels.len() + 1,
            cols,
            rows,
            Some(format!("{} * {}", names[0], names[1])),
        )
    }
}

/// The contrasts of k categories: the identity of k - 1 over a row of -1 (k x (k - 1)).
fn contrasts(k: usize) -> Matrix {
    (0..k)
        .map(|i| {
            (0..k - 1)
                .map(|j| {
                    if i == k - 1 {
                        -1.0
                    } else {
                        f64::from(u8::from(i == j))
                    }
                })
                .collect()
        })
        .collect()
}

fn kronecker(a: &Matrix, b: &Matrix) -> Matrix {
    let (ar, ac) = (a.len(), a.first().map_or(0, Vec::len));
    let (br, bc) = (b.len(), b.first().map_or(0, Vec::len));
    (0..ar * br)
        .map(|i| {
            (0..ac * bc)
                .map(|j| a[i / br][j / bc] * b[i % br][j % bc])
                .collect()
        })
        .collect()
}

/// D_p - p p': the covariance of one multinomial draw.
fn multinomial(p: &[f64]) -> Matrix {
    (0..p.len())
        .map(|a| {
            (0..p.len())
                .map(|b| f64::from(u8::from(a == b)) * p[a] - p[a] * p[b])
                .collect()
        })
        .collect()
}

/// [A_R x 1_C | 1_R x A_C]: the main effects of rows and columns.
fn independence_design(nr: usize, nc: usize) -> Matrix {
    let ones = |k: usize| -> Matrix { vec![vec![1.0]; k] };
    let left = kronecker(&contrasts(nr), &ones(nc));
    let right = kronecker(&ones(nr), &contrasts(nc));
    left.into_iter()
        .zip(right)
        .map(|(mut l, r)| {
            l.extend(r);
            l
        })
        .collect()
}

/// n (C' D^-1 M D^-1 C)^-1 (C' D^-1 V D^-1 C): the generalized design effects of contrasts C of
/// cell proportions p with design covariance V.
fn generalized_deffs(c: &Matrix, p: &[f64], v: &Matrix, n: f64) -> Option<Matrix> {
    let dc: Matrix = c
        .iter()
        .zip(p)
        .map(|(row, pi)| row.iter().map(|x| x / pi).collect())
        .collect();
    let dct = transpose(&dc);
    let left = multiply(&multiply(&dct, &multinomial(p)), &dc);
    let right = multiply(&multiply(&dct, v), &dc);
    let (inv, _) = inverse(&left)?;
    Some(
        multiply(&inv, &right)
            .into_iter()
            .map(|row| row.into_iter().map(|x| x * n).collect())
            .collect(),
    )
}
