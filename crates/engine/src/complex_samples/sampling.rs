//! CSSELECT: draws a sample from the active dataset by a sample plan of CSPLAN SAMPLE.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - The frame is the active dataset. A case leaves it when a stratum or cluster variable of the
//!   stages that nest the run is missing (user-missing counts as valid under CLASSMISSING=INCLUDE),
//!   or a size, rate, measure of size or starting weight is missing (user-missing always counts
//!   as missing).
//! - Stage by stage, a unit is a combination of the stage's cluster values (a case without
//!   CLUSTER) inside a stratum, and a stratum is a combination of the stage's strata values inside
//!   the unit of the stage before. Only units the earlier stage drew go on. Strata are drawn
//!   independently, units in the order they first appear in the data.
//! - n is SIZE, or round(RATE * N) raised to MINSIZE and lowered to MAXSIZE. Without replacement
//!   n is at most N.
//! - SIMPLE_WOR: unit k of N joins when (n - i) / (N - k + 1) beats a uniform number, i the units
//!   already taken. SIMPLE_WR: n uniform picks among the N units, a unit counting every pick.
//!   SIMPLE_SYSTEMATIC: units trunc((U + k) N / n) + 1 for k = 0 .. n - 1. SIMPLE_CHROMY: Chromy's
//!   sequential method with equal sizes: a random unit starts the order, which wraps around, and
//!   the running number of hits follows i n / N, its fractional part decided at random. The PPS
//!   methods (first stage only) are in `pps`; Brewer and Murthy draw two units.
//! - Inclusion probability n / N (with replacement 1 - (1 - 1/N)^n); stage weight N / n per draw;
//!   the cumulative weight multiplies the stage weights and the starting weight (PREVIOUSWEIGHT,
//!   or the cumulative weight of the stage before the first one run). The final weight is the
//!   cumulative weight of the last stage run.
//! - Output to the active dataset needs a plan without replacement; SAMPLEFILE writes the drawn
//!   cases only, one row per draw, with the duplicate index.
//! - A first stage drawn now by a method analyzed as UNEQUAL_WOR (without ESTIMATION=WR) writes
//!   the joint probabilities file: the first-stage strata and clusters, Unit_No_ and
//!   Joint_Prob_1_, Joint_Prob_2_, ..., one row per selected unit, the inclusion probabilities
//!   on the diagonal.
//!
//! Choices where the manual is silent:
//! - A case that the first stage drew but a later stage did not keeps the variables of the stages
//!   that drew it; only the final weight is missing.
//! - A size or rate given by a variable must be the same in every case of a stratum.
//! - A stratum with fewer units than the size asks for gives all its units, with a warning.
//! - Without CRITERIA SEED, the seed comes from the session's generator after SET SEED, else
//!   from the clock.
//! - The measure of size must be the same in every case of a unit; MIN and MAX bound it after it
//!   is read or counted, and it must then be above 0.
//! - The joint probabilities file is named after the plan with the extension joint.sav unless
//!   JOINTPROB names one.

use super::{PlanFile, PlanRate, PlanSampling, RateSource, key_part, keys_of, numbers};
use crate::crosstabs::Key;
use crate::npar_tests::text;
use crate::random::{Random, time_seed};
use crate::regression::table;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{CsEstimator, CsMethod, Csselect};
use std::collections::HashMap;
use std::ops::Range;

mod pps;

/// The variables CSSELECT writes for one stage.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Inclprob,
    Cumweight,
    Popsize,
    Sampsize,
    Rate,
    Weight,
    Index,
}

impl Kind {
    fn format(self) -> &'static str {
        match self {
            Kind::Inclprob | Kind::Rate => "F10.6",
            Kind::Cumweight | Kind::Weight => "F12.4",
            Kind::Popsize | Kind::Sampsize | Kind::Index => "F8.0",
        }
    }
}

fn method_of(d: &PlanSampling) -> CsMethod {
    CsMethod::from_keyword(&d.method).expect("checked by sample_plan")
}

/// The variables of stage `s` (from 0) in the order they are written.
fn stage_vars(d: &PlanSampling, s: usize) -> Vec<(Kind, String)> {
    let v = &d.variables;
    let mut out = vec![
        (Kind::Inclprob, v.inclprob.clone()),
        (Kind::Cumweight, v.cumweight.clone()),
    ];
    for (kind, name) in [
        (Kind::Popsize, &v.popsize),
        (Kind::Sampsize, &v.sampsize),
        (Kind::Rate, &v.rate),
        (Kind::Weight, &v.weight),
    ] {
        if let Some(name) = name {
            out.push((kind, name.clone()));
        }
    }
    if method_of(d).with_replacement() {
        let name = v
            .index
            .clone()
            .unwrap_or_else(|| format!("Index_{}_", s + 1));
        out.push((Kind::Index, name));
    }
    out
}

/// Every variable a run writes: the stage variables, then the final weight.
fn new_names(plan: &PlanFile, range: &Range<usize>) -> Vec<String> {
    let mut out: Vec<String> = range
        .clone()
        .flat_map(|s| stage_vars(plan.stages[s].sample.as_ref().expect("checked"), s))
        .map(|(_, name)| name)
        .collect();
    out.push(plan.weight.clone());
    out
}

/// `name_1`, `name_2`, ...: the first that neither the data nor `taken` has.
fn free_name(data: &Dataset, name: &str, taken: &[String]) -> String {
    (1..)
        .map(|k| format!("{name}_{k}"))
        .find(|n| {
            data.variable_index(n).is_none() && !taken.iter().any(|t| t.eq_ignore_ascii_case(n))
        })
        .expect("a free name")
}

/// round(f N), with f N first snapped to 9 decimals so that a product such as 0.3 * 5 that
/// lands just below a half in binary still rounds up.
fn rounded_size(f: f64, big_n: usize) -> usize {
    let x = f * big_n as f64;
    ((x * 1e9).round() / 1e9).round() as usize
}

/// Chromy's sequential method: the hits of `big_n` units in the order that starts at unit
/// `first` and wraps around; `expected(i)` is the whole and the fractional part of the running
/// expected hits of the first i units in that order.
fn chromy(
    first: usize,
    big_n: usize,
    random: &mut Random,
    expected: impl Fn(usize) -> (usize, f64),
) -> Vec<u32> {
    let mut hits = vec![0u32; big_n];
    let (mut t_prev, mut i_prev, mut f_prev) = (0usize, 0usize, 0.0);
    for i in 1..=big_n {
        let (whole, frac) = expected(i);
        let mut t = whole;
        if t_prev == i_prev {
            if frac > f_prev && random.uniform() < (frac - f_prev) / (1.0 - f_prev) {
                t += 1;
            }
        } else if frac > f_prev || (f_prev > 0.0 && random.uniform() < frac / f_prev) {
            // the fraction grew: the extra hit is sure; it shrank: it stays with chance
            // F_i / F_(i-1)
            t += 1;
        }
        hits[(first + i - 1) % big_n] = (t - t_prev) as u32;
        (t_prev, i_prev, f_prev) = (t, whole, frac);
    }
    hits
}

/// The number of hits of each of `big_n` units in a draw of `n` by an equal-chance method; a
/// method without replacement takes every unit when `n` is `big_n` or more.
fn draw(method: CsMethod, big_n: usize, n: usize, random: &mut Random) -> Vec<u32> {
    let mut hits = vec![0u32; big_n];
    if n == 0 || big_n == 0 {
        return hits;
    }
    if !method.with_replacement() && n >= big_n {
        return vec![1; big_n];
    }
    match method {
        CsMethod::SimpleWor => {
            let mut taken = 0;
            for (k, hit) in hits.iter_mut().enumerate() {
                if taken == n {
                    break;
                }
                // (n - i) / (N - k + 1) with k counted from 1
                if (n - taken) as f64 / (big_n - k) as f64 > random.uniform() {
                    *hit = 1;
                    taken += 1;
                }
            }
        }
        CsMethod::SimpleWr => {
            for _ in 0..n {
                let k = ((random.uniform() * big_n as f64) as usize).min(big_n - 1);
                hits[k] += 1;
            }
        }
        CsMethod::SimpleChromy => {
            let first = ((random.uniform() * big_n as f64) as usize).min(big_n - 1);
            // the expected hits up to unit i are i n / N, the whole part found in whole
            // numbers so that the last unit ends on n exactly
            return chromy(first, big_n, random, |i| {
                (i * n / big_n, (i * n % big_n) as f64 / big_n as f64)
            });
        }
        // SIMPLE_SYSTEMATIC; a PPS method reaches here only when it takes every unit
        _ => {
            let u = random.uniform();
            for k in 0..n {
                let at = ((u + k as f64) * big_n as f64 / n as f64) as usize;
                hits[at.min(big_n - 1)] = 1;
            }
        }
    }
    hits
}

impl Engine {
    /// The sample plan of a CSSELECT and the stages it runs, checked.
    fn sample_plan(&self, c: &Csselect) -> Result<(PlanFile, Range<usize>), Error> {
        let plan = self.read_plan(&c.plan)?;
        if plan.kind != "sample" {
            return Err(Error(format!(
                "{} is an analysis plan; CSSELECT needs a sample plan written by CSPLAN SAMPLE",
                c.plan
            )));
        }
        let count = plan.stages.len();
        let range = match c.stages.as_slice() {
            [] => 0..count,
            list => list[0] - 1..list[list.len() - 1],
        };
        if range.end > count {
            return Err(Error(format!(
                "STAGES names stage {}, but the plan has {count}",
                range.end
            )));
        }
        for s in range.clone() {
            let stage = &plan.stages[s];
            let Some(d) = &stage.sample else {
                return Err(Error(format!(
                    "stage {} of the plan says nothing about how to draw it",
                    s + 1
                )));
            };
            if CsMethod::from_keyword(&d.method).is_none() {
                return Err(Error(format!(
                    "stage {} of the plan uses the method {}, which is not supported yet",
                    s + 1,
                    d.method
                )));
            }
            if method_of(d).pps() && d.mos.is_none() {
                return Err(Error(format!(
                    "stage {} of the plan draws with {}, but gives no measure of size",
                    s + 1,
                    d.method
                )));
            }
            if d.size.is_none() && d.rate.is_none() && !method_of(d).draws_two() {
                return Err(Error(format!(
                    "stage {} of the plan has neither a sample size nor a rate",
                    s + 1
                )));
            }
        }
        Ok((plan, range))
    }

    /// Before the data pass: a new variable whose name the active dataset already has is an
    /// error, or under RENAMEVARS the existing one is renamed.
    pub(crate) fn csselect_names(&mut self, c: &Csselect) -> Result<(), Error> {
        if c.samplefile.is_some() {
            return Ok(());
        }
        let (plan, range) = self.sample_plan(c)?;
        let names = new_names(&plan, &range);
        let mut renamed = Vec::new();
        let data = self.active_mut()?;
        for name in &names {
            let Some(index) = data.variable_index(name) else {
                continue;
            };
            let existing = data.variable_at(index).name().to_string();
            if !c.renamevars {
                return Err(Error(format!(
                    "CSSELECT writes {name}, which the active dataset already has; rename it or give /DATA RENAMEVARS"
                )));
            }
            let free = free_name(data, &existing, &names);
            data.rename_variable(&existing, &free)?;
            renamed.push(format!(
                "CSSELECT renamed the existing variable {existing} to {free}"
            ));
        }
        for warning in renamed {
            self.warn(warning);
        }
        Ok(())
    }

    pub(crate) fn csselect(&mut self, c: &Csselect) -> Result<Vec<Table>, Error> {
        let (plan, range) = self.sample_plan(c)?;
        let last = range.end - 1;
        let stage_sampling = |s: usize| plan.stages[s].sample.as_ref().expect("checked");
        if c.samplefile.is_none()
            && range
                .clone()
                .any(|s| method_of(stage_sampling(s)).with_replacement())
        {
            return Err(Error(
                "the plan draws with replacement, so CSSELECT needs /SAMPLEFILE OUTFILE=: the active dataset cannot hold a case twice"
                    .into(),
            ));
        }
        let mut notes = Vec::new();
        // joint inclusion probabilities: first stage, drawn now, analyzed as UNEQUAL_WOR
        let first = stage_sampling(range.start);
        let joint_file = (range.start == 0
            && method_of(first).estimator() == CsEstimator::UnequalWor
            && !first.estimation_wr)
            .then(|| {
                c.jointprob
                    .clone()
                    .unwrap_or_else(|| self.default_joint_file(&c.plan))
            });
        if c.jointprob.is_some() && joint_file.is_none() {
            notes.push(
                "CSSELECT ignores JOINTPROB: joint inclusion probabilities belong to a first stage drawn now by PPS_WOR, PPS_BREWER, PPS_MURTHY or PPS_SAMPFORD without ESTIMATION=WR"
                    .to_string(),
            );
        }
        self.warn_weight_and_split("CSSELECT");
        let seed = match c.seed {
            Some(seed) => seed,
            // after SET SEED the same syntax draws the same sample again
            None if self.seed_was_set() => (self.random() * 2_147_483_646.0) as u64 + 1,
            None => time_seed(),
        };
        let mut random = Random::seeded(seed);
        let data = self.active()?;
        let shown = self.shown();
        let cases = data.case_count();
        let key_columns = |names: &[String]| -> Result<Vec<Vec<Option<Key>>>, Error> {
            names
                .iter()
                .map(|n| Ok(keys_of(&data.variable(n)?, c.include)))
                .collect()
        };
        // the frame: valid strata and clusters of every stage up to the last one run
        let mut alive = vec![true; cases];
        let mut keys = Vec::new();
        for stage in &plan.stages[..range.end] {
            let pair = (key_columns(&stage.strata)?, key_columns(&stage.clusters)?);
            for column in pair.0.iter().chain(&pair.1) {
                for (case, key) in column.iter().enumerate() {
                    alive[case] &= key.is_some();
                }
            }
            keys.push(pair);
        }
        // the starting weight
        let mut cumw = vec![1.0; cases];
        let start = match range.start {
            0 => plan.previous_weight.clone(),
            s => Some(stage_sampling(s - 1).variables.cumweight.clone()),
        };
        if let Some(name) = &start {
            let variable = data.variable(name).map_err(|_| {
                Error(match range.start {
                    0 => format!("the plan's previous weight {name} is not in the active dataset"),
                    s => format!(
                        "CSSELECT starts at stage {}, so it needs {name}, the cumulative weight of stage {s}, in the active dataset",
                        s + 1
                    ),
                })
            })?;
            for (case, w) in numbers(&variable)?.into_iter().enumerate() {
                match w {
                    Some(w) if w > 0.0 => cumw[case] = w,
                    _ => alive[case] = false,
                }
            }
        }
        // the sizes and rates; a case without one leaves the frame
        let mut sources = Vec::new();
        // the measure of size of a PPS first stage, when it comes from a variable
        let mut mos: Option<Vec<Option<f64>>> = None;
        if let Some(name) = first.mos.as_ref().and_then(|m| m.variable.as_ref())
            && range.start == 0
        {
            let values = numbers(&data.variable(name)?)?;
            for (case, alive) in alive.iter_mut().enumerate() {
                *alive &= values[case].is_some();
            }
            mos = Some(values);
        }
        for s in range.clone() {
            let d = stage_sampling(s);
            let (rate, is_size) = match (&d.size, &d.rate) {
                // Brewer and Murthy draw two units, as if SIZE VALUE=2
                _ if method_of(d).draws_two() => (&PlanRate::Value(2.0), true),
                (Some(r), _) => (r, true),
                (None, Some(r)) => (r, false),
                (None, None) => unreachable!("checked by sample_plan"),
            };
            let source = match rate {
                PlanRate::Value(v) => RateSource::Value(*v),
                PlanRate::Variable(v) => RateSource::Variable(numbers(&data.variable(v)?)?),
                PlanRate::Matrix { variables, rows } => RateSource::Matrix {
                    keys: key_columns(variables)?,
                    rows: rows.clone(),
                    variables: variables.clone(),
                },
            };
            for (case, alive) in alive.iter_mut().enumerate() {
                if *alive && source.at(case)?.is_none() {
                    *alive = false;
                }
            }
            sources.push((source, is_size));
        }
        let frame = alive.iter().filter(|a| **a).count();
        // the unit of the stage before the first one run
        let mut parent = vec![String::new(); cases];
        for (strata, clusters) in &keys[..range.start] {
            for column in strata.iter().chain(clusters) {
                for (case, key) in column.iter().enumerate() {
                    if let Some(key) = key {
                        parent[case].push('\u{1}');
                        parent[case].push_str(&key_part(key));
                    }
                }
            }
        }
        // per selected first-stage unit: a case of it, its number in the stratum, its joint
        // probabilities with the selected units of the stratum
        let mut joint_rows: Vec<(usize, usize, Vec<f64>)> = Vec::new();
        let mut columns: Vec<(String, Vec<Option<f64>>, &'static str)> = Vec::new();
        let mut tables = Vec::new();
        let mut hits_last = vec![1u32; cases];
        for (i, s) in range.clone().enumerate() {
            let d = stage_sampling(s);
            let method = method_of(d);
            let wr = method.with_replacement();
            let (strata_keys, cluster_keys) = &keys[s];
            // strata (first case, units) and units (cases) in order of first appearance
            let mut stratum_ids: HashMap<String, usize> = HashMap::new();
            let mut strata: Vec<(usize, Vec<usize>)> = Vec::new();
            let mut unit_ids: HashMap<String, usize> = HashMap::new();
            let mut unit_cases: Vec<Vec<usize>> = Vec::new();
            let mut unit_key = vec![String::new(); cases];
            for case in (0..cases).filter(|&c| alive[c]) {
                let mut skey = parent[case].clone();
                skey.push('\u{2}');
                for column in strata_keys {
                    skey.push('\u{1}');
                    skey.push_str(&key_part(column[case].as_ref().expect("in the frame")));
                }
                let next = strata.len();
                let h = *stratum_ids.entry(skey.clone()).or_insert(next);
                if h == next {
                    strata.push((case, Vec::new()));
                }
                let mut ukey = skey;
                ukey.push('\u{3}');
                if cluster_keys.is_empty() {
                    ukey.push_str(&case.to_string());
                } else {
                    for column in cluster_keys {
                        ukey.push('\u{1}');
                        ukey.push_str(&key_part(column[case].as_ref().expect("in the frame")));
                    }
                }
                let next = unit_cases.len();
                let u = *unit_ids.entry(ukey.clone()).or_insert(next);
                if u == next {
                    unit_cases.push(Vec::new());
                    strata[h].1.push(u);
                }
                unit_cases[u].push(case);
                unit_key[case] = ukey;
            }
            // the label of a stratum: the strata and clusters that nest it
            let mut label_vars: Vec<(String, &Vec<Option<Key>>)> = Vec::new();
            for (t, stage) in plan.stages[..=s].iter().enumerate() {
                let names = stage.strata.iter().zip(&keys[t].0);
                let clusters = stage.clusters.iter().zip(&keys[t].1);
                label_vars.extend(names.map(|(n, k)| (n.clone(), k)));
                if t < s {
                    label_vars.extend(clusters.map(|(n, k)| (n.clone(), k)));
                }
            }
            let mut values: Vec<Vec<Option<f64>>> = vec![vec![None; cases]; 7];
            let mut hits_case = vec![0u32; cases];
            let mut rows = Vec::new();
            let mut capped = 0;
            let (source, is_size) = &sources[i];
            for (first, units) in &strata {
                let big_n = units.len();
                let mut given = None;
                for &case in units.iter().flat_map(|&u| &unit_cases[u]) {
                    let v = source.at(case)?.expect("in the frame");
                    match given {
                        Some(g) if g != v => {
                            return Err(Error(format!(
                                "the {} of stage {} differs within a stratum ({g} and {v})",
                                if *is_size {
                                    "sample size"
                                } else {
                                    "sampling rate"
                                },
                                s + 1
                            )));
                        }
                        _ => given = Some(v),
                    }
                }
                let v = given.expect("a stratum has a case");
                let mut n = if *is_size {
                    if !(v >= 1.0 && v.fract() == 0.0) {
                        return Err(Error(format!(
                            "a sample size is a positive whole number, got {v} in stage {}",
                            s + 1
                        )));
                    }
                    v as usize
                } else {
                    if !(v > 0.0 && (wr || v <= 1.0)) {
                        return Err(Error(format!(
                            "a sampling rate is above 0{}, got {v} in stage {}",
                            if wr { "" } else { " and at most 1" },
                            s + 1
                        )));
                    }
                    let mut n = rounded_size(v, big_n);
                    if let Some(min) = d.minsize {
                        n = n.max(min as usize);
                    }
                    if let Some(max) = d.maxsize {
                        n = n.min(max as usize);
                    }
                    n
                };
                if !wr && n > big_n {
                    n = big_n;
                    capped += 1;
                }
                let (nf, bn) = (n as f64, big_n as f64);
                let joint_wanted = s == 0 && joint_file.is_some();
                let (hits, prob, weight) = if method.pps() && (wr || n < big_n) {
                    let m = d.mos.as_ref().expect("checked by sample_plan");
                    let mut sizes = Vec::with_capacity(big_n);
                    for &u in units {
                        let cases_u = &unit_cases[u];
                        let mut size = match &mos {
                            Some(values) => {
                                let v = values[cases_u[0]].expect("in the frame");
                                if let Some(w) =
                                    cases_u.iter().find_map(|&c| values[c].filter(|w| *w != v))
                                {
                                    return Err(Error(format!(
                                        "the measure of size differs within a unit ({v} and {w})"
                                    )));
                                }
                                v
                            }
                            None => cases_u.len() as f64,
                        };
                        if let Some(min) = m.min {
                            size = size.max(min);
                        }
                        if let Some(max) = m.max {
                            size = size.min(max);
                        }
                        if size.is_nan() || size <= 0.0 {
                            return Err(Error(format!(
                                "a measure of size must be above 0, got {size}"
                            )));
                        }
                        sizes.push(size);
                    }
                    let drawn = pps::draw(method, &sizes, n, &mut random)?;
                    if joint_wanted {
                        let chosen: Vec<usize> =
                            (0..big_n).filter(|&k| drawn.hits[k] > 0).collect();
                        let matrix = pps::joint(method, &sizes, n, &chosen);
                        for (no, (&k, row)) in chosen.iter().zip(matrix).enumerate() {
                            joint_rows.push((unit_cases[units[k]][0], no + 1, row));
                        }
                    }
                    (drawn.hits, drawn.prob, drawn.weight)
                } else {
                    let hits = draw(method, big_n, n, &mut random);
                    let inclprob = if wr {
                        1.0 - (1.0 - 1.0 / bn).powi(n as i32)
                    } else {
                        nf / bn
                    };
                    if joint_wanted {
                        // every unit is taken: all joint probabilities are 1
                        for (no, &u) in units.iter().enumerate() {
                            joint_rows.push((unit_cases[u][0], no + 1, vec![1.0; big_n]));
                        }
                    }
                    (hits, vec![inclprob; big_n], vec![bn / nf; big_n])
                };
                for (k, (&u, &h)) in units.iter().zip(&hits).enumerate() {
                    for &case in &unit_cases[u] {
                        if h == 0 {
                            alive[case] = false;
                            continue;
                        }
                        hits_case[case] = h;
                        cumw[case] *= weight[k];
                        for (kind, value) in [
                            (Kind::Inclprob, prob[k]),
                            (Kind::Cumweight, cumw[case]),
                            (Kind::Popsize, bn),
                            (Kind::Sampsize, nf),
                            (Kind::Rate, nf / bn),
                            (Kind::Weight, weight[k]),
                        ] {
                            values[kind as usize][case] = Some(value);
                        }
                    }
                }
                let parts: Vec<String> = label_vars
                    .iter()
                    .map(|(name, column)| {
                        let variable = data.variable(name).expect("read above");
                        let key = column[*first].as_ref().expect("in the frame");
                        format!(
                            "{} = {}",
                            shown.variable(&variable),
                            shown.value(&variable, key)
                        )
                    })
                    .collect();
                let label = if parts.is_empty() {
                    "All".to_string()
                } else {
                    parts.join(", ")
                };
                rows.push(vec![
                    text(label),
                    Cell::Number(bn),
                    Cell::Number(nf),
                    Cell::Number(nf / bn),
                ]);
            }
            if capped > 0 {
                notes.push(format!(
                    "stage {}: {capped} strata have fewer units than the sample size asks for, so all their units are drawn",
                    s + 1
                ));
            }
            for (kind, name) in stage_vars(d, s) {
                if kind != Kind::Index {
                    columns.push((
                        name,
                        std::mem::take(&mut values[kind as usize]),
                        kind.format(),
                    ));
                }
            }
            parent = unit_key;
            if s == last {
                hits_last = hits_case;
            }
            if c.print_selection {
                let stage_name = format!("Stage {}", s + 1);
                let label = plan.stages[s].label.clone().unwrap_or(stage_name);
                tables.push(table(
                    &format!("Selection Summary: {label}"),
                    1,
                    vec![
                        (String::new(), "Stratum".into()),
                        (String::new(), "Population Size".into()),
                        (String::new(), "Sample Size".into()),
                        (String::new(), "Sampling Rate".into()),
                    ],
                    rows,
                    None,
                ));
            }
        }
        let final_weight: Vec<Option<f64>> = (0..cases)
            .map(|case| alive[case].then_some(cumw[case]))
            .collect();
        columns.push((plan.weight.clone(), final_weight, Kind::Weight.format()));
        // the joint probabilities file: the first-stage strata and clusters, Unit_No_ and
        // Joint_Prob_1_, Joint_Prob_2_, ..., one row per selected unit
        let joint_out = match &joint_file {
            Some(file) => {
                let mut out = data.clone();
                out.set_filter_variable(None)?;
                out.set_split_file(None)?;
                out.set_weight_variable(None)?;
                let rows: Vec<Option<usize>> = joint_rows.iter().map(|r| Some(r.0)).collect();
                out.take_cases(&rows)?;
                let stage = &plan.stages[0];
                let ids = stage
                    .strata
                    .iter()
                    .chain(&stage.clusters)
                    .map(|n| Ok(data.variable(n)?.name().to_string()))
                    .collect::<Result<Vec<String>, Error>>()?;
                out.keep_variables(&ids.iter().map(String::as_str).collect::<Vec<_>>())?;
                let numbers = joint_rows.iter().map(|r| Some(r.1 as f64)).collect();
                out.set_numeric_values("Unit_No_", numbers)?;
                out.set_format("Unit_No_", "F8.0")?;
                let width = joint_rows.iter().map(|r| r.2.len()).max().unwrap_or(0);
                for k in 0..width {
                    let name = format!("Joint_Prob_{}_", k + 1);
                    out.set_numeric_values(
                        &name,
                        joint_rows.iter().map(|r| r.2.get(k).copied()).collect(),
                    )?;
                    out.set_format(&name, "F10.6")?;
                }
                Some((file.clone(), out))
            }
            None => None,
        };
        let selected = alive.iter().filter(|a| **a).count();
        let mut written = selected;
        match &c.samplefile {
            None => {
                self.all_or_nothing(|engine| {
                    let data = engine.active_mut()?;
                    for (name, values, format) in columns {
                        data.set_numeric_values(&name, values)?;
                        data.set_format(&name, format)?;
                    }
                    Ok(())
                })?;
            }
            Some(file) => {
                // the variables KEEP and DROP leave, in their order
                let mut kept: Vec<String> = data.variable_names().map(String::from).collect();
                for (keep, items) in &c.subsets {
                    let mut names = self.expand(items)?;
                    let mut seen: Vec<String> = Vec::new();
                    names.retain(|n| {
                        let fresh = !seen.contains(n);
                        seen.push(n.clone());
                        fresh
                    });
                    if let Some(gone) = names.iter().find(|n| !kept.contains(n)) {
                        return Err(Error(format!(
                            "SAMPLEFILE {}: {gone} is no longer among the variables to copy",
                            if *keep { "KEEP" } else { "DROP" }
                        )));
                    }
                    if *keep {
                        kept = names;
                    } else {
                        kept.retain(|k| !names.contains(k));
                    }
                }
                let mut out = data.clone();
                let new = new_names(&plan, &range);
                for name in &new {
                    let Some(index) = out.variable_index(name) else {
                        continue;
                    };
                    let existing = out.variable_at(index).name().to_string();
                    let place = kept.iter().position(|k| *k == existing);
                    if place.is_some() && !c.renamevars {
                        return Err(Error(format!(
                            "CSSELECT writes {name}, which is among the variables copied to the sample file; drop it, rename it or give /DATA RENAMEVARS"
                        )));
                    }
                    let free = free_name(&out, &existing, &new);
                    out.rename_variable(&existing, &free)?;
                    if let Some(place) = place {
                        notes.push(format!(
                            "CSSELECT renamed the copied variable {existing} to {free} in the sample file"
                        ));
                        kept[place] = free;
                    }
                }
                for (name, values, format) in columns {
                    out.set_numeric_values(&name, values)?;
                    out.set_format(&name, format)?;
                }
                let mut rows: Vec<Option<usize>> = Vec::new();
                let mut index: Vec<Option<f64>> = Vec::new();
                for case in (0..cases).filter(|&c| alive[c]) {
                    for k in 1..=hits_last[case] {
                        rows.push(Some(case));
                        index.push(Some(f64::from(k)));
                    }
                }
                written = rows.len();
                out.take_cases(&rows)?;
                if let Some((_, name)) = stage_vars(stage_sampling(last), last)
                    .into_iter()
                    .find(|(k, _)| *k == Kind::Index)
                {
                    out.set_numeric_values(&name, index)?;
                    out.set_format(&name, Kind::Index.format())?;
                }
                out.set_filter_variable(None)?;
                out.set_split_file(None)?;
                out.set_weight_variable(None)?;
                let order: Vec<&str> = kept.iter().chain(&new).map(String::as_str).collect();
                out.keep_variables(&order)?;
                self.write_data_file(file, out)?;
            }
        }
        if let Some((file, out)) = joint_out {
            self.write_data_file(&file, out)?;
        }
        if c.print_cps {
            let mut rows = vec![
                vec![
                    text("Cases in the active dataset"),
                    Cell::Number(cases as f64),
                ],
                vec![
                    text("Cases left out (missing design values)"),
                    Cell::Number((cases - frame) as f64),
                ],
                vec![text("Cases in the frame"), Cell::Number(frame as f64)],
                vec![text("Cases selected"), Cell::Number(selected as f64)],
            ];
            if c.samplefile.is_some() {
                rows.push(vec![
                    text("Rows written to the sample file"),
                    Cell::Number(written as f64),
                ]);
            }
            tables.push(table(
                "Case Processing Summary",
                1,
                vec![(String::new(), String::new()), (String::new(), "N".into())],
                rows,
                None,
            ));
        }
        for note in notes {
            self.warn(note);
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(method: CsMethod, big_n: usize, n: usize, seed: u64) -> Vec<u32> {
        draw(method, big_n, n, &mut Random::seeded(seed))
    }

    #[test]
    fn without_replacement_draws_exactly_n_distinct_units() {
        for method in [
            CsMethod::SimpleWor,
            CsMethod::SimpleSystematic,
            CsMethod::SimpleChromy,
        ] {
            for seed in 1..200 {
                for (big_n, n) in [(10, 3), (7, 7), (9, 1), (12, 5), (5, 0)] {
                    let h = hits(method, big_n, n, seed);
                    assert!(h.iter().all(|&x| x <= 1), "{method:?} {h:?}");
                    assert_eq!(h.iter().sum::<u32>() as usize, n, "{method:?} {h:?}");
                }
            }
        }
    }

    #[test]
    fn with_replacement_counts_every_pick_and_repeats_units() {
        let mut repeated = false;
        for seed in 1..50 {
            let h = hits(CsMethod::SimpleWr, 4, 6, seed);
            assert_eq!(h.iter().sum::<u32>(), 6);
            repeated |= h.iter().any(|&x| x > 1);
        }
        // six picks among four units always repeat one
        assert!(repeated);
    }

    #[test]
    fn systematic_draws_are_evenly_spaced_from_a_random_start() {
        for seed in 1..100 {
            // interval 12 / 4 = 3: one unit in every block of three, at the same place
            let h = hits(CsMethod::SimpleSystematic, 12, 4, seed);
            let at: Vec<usize> = (0..12).filter(|&k| h[k] == 1).collect();
            assert_eq!(at.len(), 4);
            assert!(at[0] < 3);
            assert!(at.windows(2).all(|w| w[1] - w[0] == 3), "{at:?}");
        }
    }

    #[test]
    fn every_unit_has_the_same_chance_under_each_method() {
        // 6000 draws of 2 of 5 units: each unit is expected 2400 times; the binomial standard
        // deviation is sqrt(6000 * 0.4 * 0.6) = 38, so 5 of them allow 190
        for method in [
            CsMethod::SimpleWor,
            CsMethod::SimpleSystematic,
            CsMethod::SimpleChromy,
        ] {
            let mut random = Random::seeded(77);
            let mut count = [0u32; 5];
            for _ in 0..6000 {
                for (c, h) in count.iter_mut().zip(draw(method, 5, 2, &mut random)) {
                    *c += h;
                }
            }
            for c in count {
                assert!(
                    (f64::from(c) - 2400.0).abs() < 190.0,
                    "{method:?} {count:?}"
                );
            }
        }
    }

    #[test]
    fn rate_times_population_rounds_half_up() {
        assert_eq!(rounded_size(0.3, 5), 2);
        assert_eq!(rounded_size(0.1, 25), 3);
        assert_eq!(rounded_size(0.24, 10), 2);
        assert_eq!(rounded_size(0.01, 20), 0);
    }
}
