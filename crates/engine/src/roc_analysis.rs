//! ROC ANALYSIS: ROC curves of several test variables against a state variable, with
//! comparisons of their areas between two independent groups of cases or between the test
//! variables measured on the same cases, precision-recall curves and classifier metrics.
//!
//! Rules from the Command Syntax Reference (ROC ANALYSIS) and the Algorithms manual (ROC
//! Analysis Algorithms):
//! - A case counts when its state, every test variable and (in the independent-group
//!   design) the grouping variable are valid; USERMISSING=INCLUDE makes user-missing values
//!   valid. Weights are rounded to whole numbers and count cases in every statistic; a case
//!   whose weight is missing or at most 0.5 counts as missing.
//! - Designs: one sample (no PAIR, no GROUP); paired (PAIR=TRUE with at least two test
//!   variables; GROUP without SELECT is then ignored); independent groups (GROUP with
//!   SELECT, which wins over PAIR). SELECT LEVEL(a b) puts the cases with value a in the
//!   first group and b in the second; MIDPOINT and CUTPOINT(c) put first the cases from
//!   the middle of the smallest and largest group value (or from c) upward, the lower cases
//!   second.
//! - Cutoffs as ROC; when every test value lies strictly between 0 and 1 the outer cutoffs
//!   are 0 and 1. Areas and their nonparametric (or bi-negative exponential) standard errors
//!   as ROC, per group in the independent-group design; NEGEXPO there needs equally many
//!   positive and negative cases in both groups, else both groups fall back. The
//!   significance tests area 0.5 with the null error sqrt((n+ + n- + 1) / (12 n+ n-)).
//!   Independent difference: z = (W1 - W2) / sqrt(SE1^2 + SE2^2).
//! - Paired design (DeLong): each positive case's share of the negative cases it beats (a tie
//!   as half) and each negative case's share of the positive cases that beat it; the
//!   variance of an area is the variance of the first over m plus that of the second over n
//!   (divisors m - 1 and n - 1), the covariance of two areas likewise from the products. The
//!   significance of an area uses its own error; the pairs of test variables are compared by
//!   z = (W1 - W2) / sqrt(V1 + V2 - 2 C). NEGEXPO does not apply.
//! - TESTPOS=SMALL reports 1 - W; errors and covariances stay.
//! - Precision TP / (TP + FP), undefined (shown empty) when nothing tests positive; recall is
//!   the sensitivity. On the curve the point where nothing tests positive takes the next
//!   point's precision when that point has true positives, else it is left out; between two
//!   points whose precisions differ, the counts are filled in one true positive (TP) or one
//!   false positive (FP) at a time, the other count growing in proportion.
//! - Classifier metrics: Gini index 2 A - 1; Max K-S, the largest sensitivity minus
//!   1 - specificity over the cutoffs, at the largest such cutoff (the smallest under
//!   TESTPOS=SMALL).

use crate::chart::{Axis, Chart, ChartKind};
use crate::crosstabs::Key;
use crate::descriptives::{case_weights, number};
use crate::dist::{normal_p, normal_quantile};
use crate::means::is_valid;
use crate::plot::{Plot, Point, PointGroup, ReferenceLine};
use crate::roc::{area, cutoffs, model_quality_chart, short};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{RocAnalysis, RocSelect, Value};

/// The test values of one curve's positive and negative cases, in case order, with their
/// (whole) weights.
pub(crate) struct Curve {
    pub pos: Vec<f64>,
    pub neg: Vec<f64>,
    pub pos_w: Vec<f64>,
    pub neg_w: Vec<f64>,
}

impl Curve {
    fn n_pos(&self) -> f64 {
        self.pos_w.iter().sum()
    }
    fn n_neg(&self) -> f64 {
        self.neg_w.iter().sum()
    }
}

/// One area with its statistics, as reported (after TESTPOS).
#[derive(Debug, Clone, Copy)]
struct Estimate {
    area: f64,
    se: f64,
    sig: f64,
    fell_back: bool,
}

/// The DeLong components with the case weights: for each positive case the share of
/// negative cases below it and for each negative case the share of positive cases above it,
/// ties as half.
pub(crate) struct Parts<'a> {
    tx: Vec<f64>,
    ty: Vec<f64>,
    curve: &'a Curve,
}

pub(crate) fn placements(curve: &Curve) -> Parts<'_> {
    // values sorted with the running sum of their weights
    let sorted = |v: &[f64], w: &[f64]| {
        let mut pairs: Vec<(f64, f64)> = v.iter().copied().zip(w.iter().copied()).collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut sums = vec![0.0];
        for p in &pairs {
            sums.push(sums.last().unwrap() + p.1);
        }
        (pairs, sums)
    };
    let (pos, neg) = (
        sorted(&curve.pos, &curve.pos_w),
        sorted(&curve.neg, &curve.neg_w),
    );
    let share = |(pairs, sums): &(Vec<(f64, f64)>, Vec<f64>), x: f64, above: bool| {
        let below = sums[pairs.partition_point(|p| p.0 < x)];
        let up_to = sums[pairs.partition_point(|p| p.0 <= x)];
        let total = sums[pairs.len()];
        let beaten = if above { total - up_to } else { below };
        (beaten + (up_to - below) / 2.0) / total
    };
    Parts {
        tx: curve.pos.iter().map(|&x| share(&neg, x, false)).collect(),
        ty: curve.neg.iter().map(|&y| share(&pos, y, true)).collect(),
        curve,
    }
}

/// The weighted sample covariance of two lists around their weighted means, over the
/// weight total minus 1, as when each case were repeated as often as its weight says.
fn covariance(a: &[f64], b: &[f64], w: &[f64]) -> f64 {
    let total: f64 = w.iter().sum();
    let mean = |v: &[f64]| v.iter().zip(w).map(|(x, w)| x * w).sum::<f64>() / total;
    let (ma, mb) = (mean(a), mean(b));
    let sum: f64 = (a.iter().zip(b).zip(w))
        .map(|((x, y), w)| w * (x - ma) * (y - mb))
        .sum();
    sum / (total - 1.0)
}

/// The DeLong covariance of the areas of two curves on the same cases (a variance when both
/// are the same).
pub(crate) fn delong_covariance(a: &Parts, b: &Parts) -> f64 {
    let c = a.curve;
    covariance(&a.tx, &b.tx, &c.pos_w) / c.n_pos() + covariance(&a.ty, &b.ty, &c.neg_w) / c.n_neg()
}

/// The coordinates at every cutoff, ascending: (cutoff, true positives, false positives),
/// counted with the weights.
pub(crate) fn coordinates(curve: &Curve, small: bool, exclude: bool) -> Vec<(f64, f64, f64)> {
    let values: Vec<f64> = curve.pos.iter().chain(&curve.neg).copied().collect();
    let mut cuts = cutoffs(&values);
    // test values that all lie strictly between 0 and 1 get the outer cutoffs 0 and 1
    if values.iter().all(|&v| v > 0.0 && v < 1.0) {
        let last = cuts.len() - 1;
        (cuts[0], cuts[last]) = (0.0, 1.0);
    }
    cuts.iter()
        .map(|&cut| {
            let positive = |x: f64| match (small, exclude) {
                (false, false) => x >= cut,
                (false, true) => x > cut,
                (true, false) => x <= cut,
                (true, true) => x < cut,
            };
            let count = |v: &[f64], w: &[f64]| -> f64 {
                v.iter()
                    .zip(w)
                    .filter(|p| positive(*p.0))
                    .map(|p| p.1)
                    .sum()
            };
            (
                cut,
                count(&curve.pos, &curve.pos_w),
                count(&curve.neg, &curve.neg_w),
            )
        })
        .collect()
}

/// The precision-recall curve from (true positives, false positives) pairs ordered from no
/// case testing positive to every case testing positive, filled in between neighbors.
pub(crate) fn pr_points(counts: &[(f64, f64)], n_pos: f64, along_fp: bool) -> Vec<Point> {
    let point = |tp: f64, fp: f64| Point {
        x: tp / n_pos,
        y: tp / (tp + fp),
        z: None,
        label: None,
    };
    let mut out = Vec::new();
    let mut previous: Option<(f64, f64, f64)> = None;
    for (k, &(tp, fp)) in counts.iter().enumerate() {
        let precision = if tp + fp > 0.0 {
            tp / (tp + fp)
        } else {
            // nothing tests positive: the next point's precision if it has true positives
            match counts.get(k + 1) {
                Some(&(next_tp, next_fp)) if next_tp > 0.0 => next_tp / (next_tp + next_fp),
                _ => continue,
            }
        };
        if let Some((tp_a, fp_a, precision_a)) = previous
            && precision_a != precision
        {
            let (d_tp, d_fp) = (tp - tp_a, fp - fp_a);
            let steps = if along_fp { d_fp } else { d_tp };
            for delta in 1..steps.max(0.0) as usize {
                let delta = delta as f64;
                out.push(match along_fp {
                    false => point(tp_a + delta, fp_a + d_fp / d_tp * delta),
                    true => point(tp_a + d_tp / d_fp * delta, fp_a + delta),
                });
            }
        }
        out.push(Point {
            y: precision,
            ..point(tp, fp)
        });
        previous = Some((tp, fp, precision));
    }
    out
}

/// The largest sensitivity minus 1 - specificity and its cutoff: of several, the largest
/// cutoff, or the smallest when `small`.
pub(crate) fn max_ks(
    coordinates: &[(f64, f64, f64)],
    n_pos: f64,
    n_neg: f64,
    small: bool,
) -> (f64, f64) {
    let mut best: Option<(f64, f64)> = None;
    for &(cut, tp, fp) in coordinates {
        // counts are whole numbers, so tp n- - fp n+ compares the differences exactly
        let key = tp * n_neg - fp * n_pos;
        let better = best.is_none_or(|(b, _)| if small { key > b } else { key >= b });
        if better {
            best = Some((key, cut));
        }
    }
    let (key, cut) = best.expect("a curve has cutoffs");
    (key / (n_pos * n_neg), cut)
}

/// The cases of one group of the independent-group design, or all cases.
struct Unit {
    label: Option<String>,
    cases: Vec<usize>,
}

enum Design {
    One,
    Paired,
    Independent,
}

/// A group value as the tables show it: its label, or `= value`.
fn level_label(shown: &crate::shown::Shown, variable: &Variable, key: &Key) -> String {
    let labelled = variable
        .value_labels()
        .iter()
        .any(|(value, _)| match (value, key) {
            (statscraft_data::Value::Number(a), Key::Number(b)) => a == b,
            (statscraft_data::Value::String(a), Key::Text(b)) => a.trim_end() == b,
            _ => false,
        });
    let text = shown.value(variable, key);
    match (labelled, key) {
        (true, _) => text,
        (false, Key::Text(_)) => format!("= '{text}'"),
        (false, Key::Number(_)) => format!("= {text}"),
    }
}

fn key_of(value: &Value, variable: &Variable, what: &str) -> Result<Key, Error> {
    match (value, variable.values()) {
        (Value::Number(n), Values::Numeric(_)) => Ok(Key::Number(*n)),
        (Value::String(s), Values::String(_)) => Ok(Key::Text(s.trim_end().to_string())),
        _ => Err(Error(format!(
            "the {what} must have the type of {}",
            variable.name()
        ))),
    }
}

fn key_at(variable: &Variable, case: usize) -> Option<Key> {
    match variable.values() {
        Values::Numeric(v) => v.get(case).map(Key::Number),
        Values::String(v) => Some(Key::Text(v.get(case).trim_end().to_string())),
    }
}

impl Engine {
    pub(crate) fn roc_analysis(&mut self, command: &RocAnalysis) -> Result<Vec<Table>, Error> {
        for warning in &command.warnings {
            self.warn(format!("ROC ANALYSIS: {warning}"));
        }
        let d = &command.design;
        let paired = d.pair && !(d.group.is_some() && d.select.is_some());
        if command.criteria.negexpo && paired {
            self.warn(
                "ROC ANALYSIS: DISTRIBUTION=NEGEXPO does not apply to the paired-sample design; \
                 the standard errors are nonparametric",
            );
        }
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&command.tests)?;
        let state = data.variable(&command.state)?;
        let mut tests = Vec::new();
        for name in &names {
            let variable = data.variable(name)?;
            if variable.is_string() {
                return Err(Error(format!(
                    "ROC ANALYSIS needs numeric test variables, but {name} is a string variable"
                )));
            }
            tests.push(variable);
        }
        let (criteria, design_spec, print) = (&command.criteria, &command.design, &command.print);
        let small = criteria.small_positive;
        let design = match (&design_spec.group, &design_spec.select, design_spec.pair) {
            (Some(_), Some(_), _) => Design::Independent,
            (_, _, true) => Design::Paired,
            (Some(_), None, false) => {
                return Err(Error(
                    "DESIGN GROUP needs SELECT to define the two groups".into(),
                ));
            }
            (None, Some(_), false) => {
                return Err(Error("DESIGN SELECT needs GROUP".into()));
            }
            (None, None, false) => Design::One,
        };
        if matches!(design, Design::Paired) && tests.len() < 2 {
            return Err(Error(
                "the paired-sample design (PAIR=TRUE) needs at least two test variables".into(),
            ));
        }
        let negexpo = criteria.negexpo && !matches!(design, Design::Paired);
        let positive_key = key_of(&command.positive, &state, "positive state value")?;
        let is_positive = |c: usize| key_at(&state, c).as_ref() == Some(&positive_key);
        let group = match (&design, &design_spec.group) {
            (Design::Independent, Some(name)) => Some(data.variable(name)?),
            _ => None,
        };
        let weights = case_weights(data)?;
        // weights round to whole numbers; at most 0.5 (or missing) makes the case missing
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let include = command.include;
        let (mut valid, mut missing) = (Vec::new(), 0.0);
        for c in 0..data.case_count() {
            let w = weight(c);
            let ok = w > 0.5
                && is_valid(&state, c, include)
                && tests.iter().all(|t| is_valid(t, c, include))
                && group.as_ref().is_none_or(|g| is_valid(g, c, include));
            match (ok, w > 0.5) {
                (true, _) => valid.push(c),
                (false, true) => missing += w.round(),
                (false, false) => missing += 1.0,
            }
        }
        let value = |t: &Variable, c: usize| match t.values() {
            Values::Numeric(v) => v.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };

        // the units: all valid cases, or the two groups
        let mut not_selected = 0.0;
        let units: Vec<Unit> = match (&group, &design_spec.select) {
            (Some(g), Some(select)) => {
                // the group of each valid case, in the order of `valid`
                let (labels, member): ([String; 2], Vec<Option<usize>>) = match select {
                    RocSelect::Levels(a, b) => {
                        let keys = [key_of(a, g, "LEVEL value")?, key_of(b, g, "LEVEL value")?];
                        let member: Vec<Option<usize>> = valid
                            .iter()
                            .map(|&c| {
                                let key = key_at(g, c);
                                keys.iter().position(|k| Some(k) == key.as_ref())
                            })
                            .collect();
                        for (k, key) in keys.iter().enumerate() {
                            if !member.contains(&Some(k)) {
                                return Err(Error(format!(
                                    "the LEVEL value {} does not occur among the valid cases of {}",
                                    shown.value(g, key),
                                    g.name()
                                )));
                            }
                        }
                        (keys.map(|k| level_label(&shown, g, &k)), member)
                    }
                    RocSelect::Midpoint | RocSelect::Cutpoint(_) => {
                        if g.is_string() {
                            return Err(Error(format!(
                                "SELECT MIDPOINT and CUTPOINT need a numeric grouping variable, \
                                 but {} is a string variable",
                                g.name()
                            )));
                        }
                        let cut = match select {
                            RocSelect::Cutpoint(c) => *c,
                            _ => {
                                let (lo, hi) = valid
                                    .iter()
                                    .map(|&c| value(g, c))
                                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                                        (lo.min(v), hi.max(v))
                                    });
                                (lo + hi) / 2.0
                            }
                        };
                        let member = valid
                            .iter()
                            .map(|&c| Some(usize::from(value(g, c) < cut)))
                            .collect();
                        ([format!("≥ {cut}"), format!("< {cut}")], member)
                    }
                };
                let mut units = labels.map(|label| Unit {
                    label: Some(label),
                    cases: Vec::new(),
                });
                for (&c, k) in valid.iter().zip(member) {
                    match k {
                        Some(k) => units[k].cases.push(c),
                        None => not_selected += weight(c).round(),
                    }
                }
                units.into()
            }
            _ => vec![Unit {
                label: None,
                cases: valid.clone(),
            }],
        };
        let count = |cases: &[usize], positive: bool| -> f64 {
            cases
                .iter()
                .filter(|&&c| is_positive(c) == positive)
                .map(|&c| weight(c).round())
                .sum()
        };
        for unit in &units {
            let (n_pos, n_neg) = (
                unit.cases.iter().filter(|&&c| is_positive(c)).count(),
                unit.cases.iter().filter(|&&c| !is_positive(c)).count(),
            );
            if n_pos == 0 || n_neg == 0 {
                let place = unit
                    .label
                    .as_ref()
                    .map_or(String::new(), |l| format!(" in the group {l}"));
                return Err(Error(format!(
                    "ROC ANALYSIS needs valid cases in both states{place}, but there are {n_pos} \
                     positive and {n_neg} negative cases"
                )));
            }
        }

        // the case processing summary
        let state_text = shown.value(&state, &positive_key);
        let direction = if small { "Smaller" } else { "Larger" };
        let mut caption = format!(
            "{direction} test values point to the positive state, which is {} = {state_text}.",
            shown.variable(&state)
        );
        if weights.is_some() {
            caption.push_str(
                " Weights are rounded to whole numbers; a case whose weight is missing or at most \
                 0.5 counts as missing.",
            );
        }
        let text = |s: &str| Cell::Text(s.into());
        let mut summary_rows = Vec::new();
        let mut columns = vec![shown.variable(&state), "Valid N (listwise)".into()];
        let valid_n: f64 = units
            .iter()
            .map(|u| count(&u.cases, true) + count(&u.cases, false))
            .sum();
        match &group {
            Some(g) => {
                columns.insert(0, shown.variable(g));
                for unit in &units {
                    let label = unit.label.clone().unwrap_or_default();
                    summary_rows.push(vec![
                        Cell::Text(label),
                        text("Positive"),
                        Cell::Number(count(&unit.cases, true)),
                    ]);
                    summary_rows.push(vec![
                        text(""),
                        text("Negative"),
                        Cell::Number(count(&unit.cases, false)),
                    ]);
                }
                summary_rows.push(vec![text("Valid"), text(""), Cell::Number(valid_n)]);
                if not_selected > 0.0 {
                    summary_rows.push(vec![
                        text("In neither group"),
                        text(""),
                        Cell::Number(not_selected),
                    ]);
                }
                summary_rows.push(vec![text("Missing"), text(""), Cell::Number(missing)]);
                summary_rows.push(vec![
                    text("Total"),
                    text(""),
                    Cell::Number(valid_n + not_selected + missing),
                ]);
            }
            None => {
                let cases = &units[0].cases;
                summary_rows.push(vec![text("Positive"), Cell::Number(count(cases, true))]);
                summary_rows.push(vec![text("Negative"), Cell::Number(count(cases, false))]);
                summary_rows.push(vec![text("Missing"), Cell::Number(missing)]);
                summary_rows.push(vec![text("Total"), Cell::Number(valid_n + missing)]);
            }
        }
        let grouped = group.is_some();
        let mut tables = vec![Table {
            title: "Case Processing Summary".into(),
            row_headers: columns.len() - 1,
            columns,
            rows: summary_rows,
            caption: Some(caption),
            ..Table::default()
        }];

        // the curves: per test variable, per unit
        let balanced = units
            .iter()
            .all(|u| count(&u.cases, true) == count(&u.cases, false));
        let z = normal_quantile(0.5 + criteria.ci / 200.0);
        let labels: Vec<String> = tests.iter().map(|t| shown.variable(t)).collect();
        // [test][unit]
        let mut curves: Vec<Vec<Curve>> = Vec::new();
        let mut estimates: Vec<Vec<Estimate>> = Vec::new();
        let mut tied = Vec::new();
        for test in &tests {
            let mut test_curves = Vec::new();
            let mut test_estimates = Vec::new();
            for unit in &units {
                let split = |positive: bool, of: &dyn Fn(usize) -> f64| -> Vec<f64> {
                    unit.cases
                        .iter()
                        .filter(|&&c| is_positive(c) == positive)
                        .map(|&c| of(c))
                        .collect()
                };
                let rounded = |c: usize| weight(c).round();
                let curve = Curve {
                    pos: split(true, &|c| value(test, c)),
                    neg: split(false, &|c| value(test, c)),
                    pos_w: split(true, &rounded),
                    neg_w: split(false, &rounded),
                };
                let tagged = |v: &[f64], w: &[f64], positive: bool| {
                    v.iter()
                        .zip(w)
                        .map(move |(&v, &w)| (v, w, positive))
                        .collect::<Vec<_>>()
                };
                let mut cases = tagged(&curve.pos, &curve.pos_w, true);
                cases.extend(tagged(&curve.neg, &curve.neg_w, false));
                let a = area(&cases, negexpo && balanced);
                let mut estimate = Estimate {
                    area: a.area,
                    se: a.se,
                    sig: a.sig,
                    fell_back: negexpo && !balanced,
                };
                if small {
                    estimate.area = 1.0 - estimate.area;
                }
                test_curves.push(curve);
                test_estimates.push(estimate);
            }
            let ties = test_curves
                .iter()
                .any(|c| c.pos.iter().any(|p| c.neg.contains(p)));
            if ties {
                tied.push(shown.variable(test));
            }
            curves.push(test_curves);
            estimates.push(test_estimates);
        }
        // the paired design: DeLong errors, and the significance of each area with its own
        let components: Vec<Parts> = match design {
            Design::Paired => curves.iter().map(|c| placements(&c[0])).collect(),
            _ => Vec::new(),
        };
        for (parts, e) in components.iter().zip(&mut estimates) {
            e[0].se = delong_covariance(parts, parts).max(0.0).sqrt();
            e[0].sig = normal_p((e[0].area - 0.5) / e[0].se);
        }
        let fell_back = estimates.iter().flatten().any(|e| e.fell_back);
        let tie_note = (!tied.is_empty()).then(|| {
            format!(
                "{}: some test values occur in both the positive and the negative group, which \
                 may bias the statistics.",
                tied.join(", ")
            )
        });
        let method = match (&design, negexpo, fell_back) {
            (Design::Paired, ..) => "The standard errors follow DeLong's nonparametric method.",
            (_, false, _) => "The standard errors assume no distribution.",
            (_, true, false) => "The standard errors assume bi-negative exponential distributions.",
            (Design::Independent, true, true) => {
                "Bi-negative exponential standard errors need as many positive as negative cases \
                 in both groups, which is not so here: the standard errors assume no distribution."
            }
            (_, true, true) => {
                "Bi-negative exponential standard errors need as many positive as negative \
                 cases, which is not so here: the standard errors assume no distribution."
            }
        };
        let level = format!("Asymptotic {}% Confidence Interval", short(criteria.ci));
        // the row labels of a [test][unit] table: the test on its first row, then the group
        let row_start = |t: usize, u: usize| -> Vec<Cell> {
            let mut row = vec![Cell::Text(if u == 0 {
                labels[t].clone()
            } else {
                String::new()
            })];
            if let Some(label) = &units[u].label {
                row.push(Cell::Text(label.clone()));
            }
            row
        };
        let mut head: Vec<String> = vec!["Test Result Variable(s)".into()];
        if grouped {
            head.push("Group".into());
        }

        // area under the curve
        let mut area_rows = Vec::new();
        let mut lower_bounds = Vec::new();
        let mut bar_labels = Vec::new();
        for (t, test_estimates) in estimates.iter().enumerate() {
            for (u, e) in test_estimates.iter().enumerate() {
                let mut row = row_start(t, u);
                row.push(Cell::Number(e.area));
                let (lo, hi) = (e.area - z * e.se, e.area + z * e.se);
                if print.se {
                    row.extend([
                        number(Some(e.se)),
                        number(Some(e.sig)),
                        number(Some(lo)),
                        number(Some(hi)),
                    ]);
                }
                area_rows.push(row);
                lower_bounds.push(lo);
                bar_labels.push(match &units[u].label {
                    Some(label) => format!("{} ({label})", labels[t]),
                    None => labels[t].clone(),
                });
            }
        }
        let mut columns = head.clone();
        columns.push("Area".into());
        let mut column_groups = vec![Vec::new(); columns.len()];
        let mut notes: Vec<String> = tie_note.iter().cloned().collect();
        if print.se {
            columns.extend(
                [
                    "Std. Error",
                    "Asymptotic Sig.",
                    "Lower Bound",
                    "Upper Bound",
                ]
                .map(String::from),
            );
            column_groups.extend([vec![], vec![], vec![level.clone()], vec![level.clone()]]);
            notes.push(method.into());
            notes.push("The significance tests the null hypothesis that the area is 0.5.".into());
        }
        tables.push(Table {
            title: "Area Under the ROC Curve".into(),
            row_headers: head.len(),
            columns,
            column_groups,
            rows: area_rows,
            caption: (!notes.is_empty()).then(|| notes.join(" ")),
            ..Table::default()
        });

        // the coordinates of every curve, [test][unit]
        let coords: Vec<Vec<Vec<(f64, f64, f64)>>> = curves
            .iter()
            .map(|cs| {
                cs.iter()
                    .map(|c| coordinates(c, small, criteria.exclude_cutoff))
                    .collect()
            })
            .collect();

        if print.classifier {
            let mut rows = Vec::new();
            for (t, test_curves) in curves.iter().enumerate() {
                for (u, curve) in test_curves.iter().enumerate() {
                    let (n_pos, n_neg) = (curve.n_pos(), curve.n_neg());
                    let (ks, cut) = max_ks(&coords[t][u], n_pos, n_neg, small);
                    let mut row = row_start(t, u);
                    row.extend([
                        Cell::Number(2.0 * estimates[t][u].area - 1.0),
                        Cell::Number(ks),
                        Cell::Number(cut),
                    ]);
                    rows.push(row);
                }
            }
            let ks_group = "K-S Statistic".to_string();
            let mut columns = head.clone();
            columns.extend(["Gini Index", "Max K-S", "Cutoff"].map(String::from));
            let mut column_groups = vec![Vec::new(); head.len() + 1];
            column_groups.extend([vec![ks_group.clone()], vec![ks_group]]);
            let which = if small { "smallest" } else { "largest" };
            tables.push(Table {
                title: "Classifier Evaluation Metrics".into(),
                row_headers: head.len(),
                columns,
                column_groups,
                rows,
                caption: Some(format!(
                    "The Gini index is 2 x area - 1. Max K-S is the largest difference between \
                     sensitivity and 1 - specificity over the cutoffs; where several cutoffs \
                     reach it, the {which} is shown."
                )),
                ..Table::default()
            });
        }

        // the comparison of areas
        let difference_columns = |first: &str| -> (Vec<String>, Vec<Vec<String>>) {
            let columns = [
                first,
                "Asymptotic z",
                "Asymptotic Sig. (2-tail)",
                "AUC Difference",
                "Std. Error Difference",
                "Lower Bound",
                "Upper Bound",
            ]
            .map(String::from)
            .into();
            let mut groups = vec![Vec::new(); 5];
            groups.extend([vec![level.clone()], vec![level.clone()]]);
            (columns, groups)
        };
        let difference_row = |label: String, diff: f64, se: f64| -> Vec<Cell> {
            let z_value = diff / se;
            vec![
                Cell::Text(label),
                number(Some(z_value)),
                number(Some(normal_p(z_value))),
                Cell::Number(diff),
                number(Some(se)),
                number(Some(diff - z * se)),
                number(Some(diff + z * se)),
            ]
        };
        let difference_note = format!(
            "The significance tests the null hypothesis that the difference is 0. {method}"
        );
        match design {
            Design::Independent => {
                let rows = estimates
                    .iter()
                    .enumerate()
                    .map(|(t, e)| {
                        difference_row(
                            labels[t].clone(),
                            e[0].area - e[1].area,
                            e[0].se.hypot(e[1].se),
                        )
                    })
                    .collect();
                let (columns, column_groups) = difference_columns("Test Result Variable(s)");
                tables.push(Table {
                    title: "Independent-Group Area Difference Under the ROC Curves".into(),
                    row_headers: 1,
                    columns,
                    column_groups,
                    rows,
                    caption: Some(format!(
                        "Each difference is the area of the group {} minus that of the group {}. \
                         {difference_note}",
                        units[0].label.as_deref().unwrap_or_default(),
                        units[1].label.as_deref().unwrap_or_default()
                    )),
                    ..Table::default()
                });
            }
            Design::Paired => {
                let mut rows = Vec::new();
                for i in 0..tests.len() {
                    for j in i + 1..tests.len() {
                        let variance = delong_covariance(&components[i], &components[i])
                            + delong_covariance(&components[j], &components[j])
                            - 2.0 * delong_covariance(&components[i], &components[j]);
                        let diff = estimates[i][0].area - estimates[j][0].area;
                        rows.push(difference_row(
                            format!("{} - {}", labels[i], labels[j]),
                            diff,
                            variance.max(0.0).sqrt(),
                        ));
                    }
                }
                let (columns, column_groups) = difference_columns("Test Result Pair(s)");
                tables.push(Table {
                    title: "Paired-Sample Area Difference Under the ROC Curves".into(),
                    row_headers: 1,
                    columns,
                    column_groups,
                    rows,
                    caption: Some(difference_note),
                    ..Table::default()
                });
            }
            Design::One => {}
        }

        let rule = match (small, criteria.exclude_cutoff) {
            (false, false) => "Positive if Greater Than or Equal To",
            (false, true) => "Positive if Greater Than",
            (true, false) => "Positive if Less Than or Equal To",
            (true, true) => "Positive if Less Than",
        };
        let coordinate_table = |title: &str, measures: [&str; 2], pr: bool| -> Table {
            let mut rows = Vec::new();
            let mut unit_interval = Vec::new();
            for (t, test_coords) in coords.iter().enumerate() {
                for (u, points) in test_coords.iter().enumerate() {
                    let curve = &curves[t][u];
                    let (n_pos, n_neg) = (curve.n_pos(), curve.n_neg());
                    let inside = (curve.pos.iter().chain(&curve.neg)).all(|&v| v > 0.0 && v < 1.0);
                    if inside && !unit_interval.contains(&labels[t]) {
                        unit_interval.push(labels[t].clone());
                    }
                    for (k, &(cut, tp, fp)) in points.iter().enumerate() {
                        let mut row = if k == 0 {
                            row_start(t, u)
                        } else {
                            vec![text(""); head.len()]
                        };
                        let (a, b) = match pr {
                            false => (Cell::Number(tp / n_pos), Cell::Number(fp / n_neg)),
                            true => (number(Some(tp / (tp + fp))), Cell::Number(tp / n_pos)),
                        };
                        row.extend([Cell::Number(cut), a, b]);
                        rows.push(row);
                    }
                }
            }
            let mut notes: Vec<String> = tie_note.iter().cloned().collect();
            notes.push(
                "The cutoffs lie halfway between neighboring test values, plus one cutoff 1 below \
                 the smallest value and one 1 above the largest."
                    .into(),
            );
            if !unit_interval.is_empty() {
                notes.push(format!(
                    "{}: every test value lies between 0 and 1, so the outer cutoffs are 0 and 1.",
                    unit_interval.join(", ")
                ));
            }
            if pr {
                notes.push("Precision is empty where no case tests positive.".into());
            }
            let mut columns = head.clone();
            columns.push(rule.into());
            columns.extend(measures.map(String::from));
            Table {
                title: title.into(),
                row_headers: head.len(),
                columns,
                rows,
                caption: Some(notes.join(" ")),
                ..Table::default()
            }
        };
        if print.coordinates_roc {
            tables.push(coordinate_table(
                "Coordinates of the ROC Curve",
                ["Sensitivity", "1 - Specificity"],
                false,
            ));
        }
        if print.coordinates_pr {
            tables.push(coordinate_table(
                "Coordinates of the Precision-Recall Curve",
                ["Precision", "Recall"],
                true,
            ));
        }

        // charts: one with every test variable, or one per test variable with its two groups
        let mut charts = Vec::new();
        if command.plot.model_quality {
            charts.push(model_quality_chart(bar_labels, &lower_bounds));
        }
        // from the strictest cutoff (nothing tests positive) on
        let ordered = |points: &[(f64, f64, f64)]| -> Vec<(f64, f64)> {
            let mut counts: Vec<(f64, f64)> = points.iter().map(|&(_, tp, fp)| (tp, fp)).collect();
            if !small {
                counts.reverse();
            }
            counts
        };
        let plots: Vec<(Option<String>, Vec<usize>)> = match grouped {
            true => (0..tests.len())
                .map(|t| (Some(labels[t].clone()), vec![t]))
                .collect(),
            false => vec![(None, (0..tests.len()).collect())],
        };
        let legend = match &group {
            Some(g) => Some(format!("Grouped by {}", shown.variable(g))),
            None if tests.len() > 1 => Some("Test variable".into()),
            None => None,
        };
        let group_name =
            |t: usize, u: usize| units[u].label.clone().unwrap_or_else(|| labels[t].clone());
        for (subtitle, members) in &plots {
            if !command.plot.roc {
                break;
            }
            let mut groups = Vec::new();
            for &t in members {
                for (u, points) in coords[t].iter().enumerate() {
                    let curve = &curves[t][u];
                    let (n_pos, n_neg) = (curve.n_pos(), curve.n_neg());
                    let points = ordered(points)
                        .iter()
                        .map(|&(tp, fp)| Point {
                            x: fp / n_neg,
                            y: tp / n_pos,
                            z: None,
                            label: None,
                        })
                        .collect();
                    groups.push(PointGroup {
                        name: group_name(t, u),
                        points,
                    });
                }
            }
            let lines = match command.plot.reference {
                true => vec![ReferenceLine {
                    intercept: 0.0,
                    slope: 1.0,
                }],
                false => Vec::new(),
            };
            let mut chart = Chart::plot(
                ChartKind::Roc,
                "ROC curve".into(),
                Axis::titled("1 - Specificity"),
                Axis::titled("Sensitivity"),
                Plot::Points {
                    groups,
                    lines,
                    z_axis: None,
                },
            );
            chart.subtitle = subtitle.clone();
            chart.legend = legend.clone();
            if members.iter().any(|&t| tied.contains(&labels[t])) {
                chart.footnote =
                    Some("A diagonal step comes from values tied between the groups".into());
            }
            charts.push(chart);
        }
        for (subtitle, members) in &plots {
            if !command.plot.pr {
                break;
            }
            let mut groups = Vec::new();
            let mut lines = Vec::new();
            for &t in members {
                for (u, points) in coords[t].iter().enumerate() {
                    let n_pos = curves[t][u].n_pos();
                    let points = pr_points(&ordered(points), n_pos, command.plot.interpolate_fp);
                    groups.push(PointGroup {
                        name: group_name(t, u),
                        points,
                    });
                    let n = n_pos + curves[t][u].n_neg();
                    let chance = ReferenceLine {
                        intercept: n_pos / n,
                        slope: 0.0,
                    };
                    if command.plot.pr_reference && !lines.contains(&chance) {
                        lines.push(chance);
                    }
                }
            }
            let mut chart = Chart::plot(
                ChartKind::Line,
                "Precision-recall curve".into(),
                Axis::titled("Recall"),
                Axis::titled("Precision"),
                Plot::Points {
                    groups,
                    lines,
                    z_axis: None,
                },
            );
            chart.subtitle = subtitle.clone();
            chart.legend = legend.clone();
            if command.plot.pr_reference {
                chart.footnote = Some(
                    "The horizontal line is the share of positive cases, the precision of guessing"
                        .into(),
                );
            }
            charts.push(chart);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_and_classifier_parts() {
        // positives 2, 4, 4; negatives 1, 4: hand counts
        let ones = |n: usize| vec![1.0; n];
        let curve = Curve {
            pos: vec![2.0, 4.0, 4.0],
            neg: vec![1.0, 4.0],
            pos_w: ones(3),
            neg_w: ones(2),
        };
        let parts = placements(&curve);
        assert_eq!(parts.tx, [0.5, 0.75, 0.75]);
        assert_eq!(parts.ty, [1.0, 1.0 / 3.0]);
        // a weight of 2 is the same as the case twice
        let weighted = Curve {
            pos: vec![2.0, 4.0],
            neg: vec![1.0, 4.0],
            pos_w: vec![1.0, 2.0],
            neg_w: ones(2),
        };
        let w = placements(&weighted);
        assert_eq!(
            (w.tx.as_slice(), w.ty.as_slice()),
            ([0.5, 0.75].as_slice(), parts.ty.as_slice())
        );
        let (v1, v2) = (delong_covariance(&parts, &parts), delong_covariance(&w, &w));
        assert!((v1 - v2).abs() < 1e-15, "{v1} {v2}");
        let coords = coordinates(&curve, false, false);
        assert_eq!(coords, coordinates(&weighted, false, false));
        let cuts: Vec<f64> = coords.iter().map(|c| c.0).collect();
        assert_eq!(cuts, [0.0, 1.5, 3.0, 5.0]);
        // (tp, fp): 3 2, 3 1, 2 1, 0 0; tp n- - fp n+: 0, 3, 1, 0
        assert_eq!(max_ks(&coords, 3.0, 2.0, false), (0.5, 1.5));
        // a tie for the largest gap: the largest cutoff, or the smallest one under SMALL
        let tied = [(1.0, 2.0, 1.0), (2.0, 1.0, 0.0), (3.0, 0.0, 0.0)];
        assert_eq!(max_ks(&tied, 2.0, 2.0, false), (0.5, 2.0));
        assert_eq!(max_ks(&tied, 2.0, 2.0, true), (0.5, 1.0));
        // values inside (0, 1) get the outer cutoffs 0 and 1
        let unit = Curve {
            pos: vec![0.8],
            neg: vec![0.2],
            pos_w: ones(1),
            neg_w: ones(1),
        };
        let cuts: Vec<f64> = coordinates(&unit, false, false)
            .iter()
            .map(|c| c.0)
            .collect();
        assert_eq!(cuts, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn precision_recall_is_filled_in_between_points() {
        // from nothing positive to 4 true and 2 false positives in one step, 5 positives:
        // along TP the false positives grow by 2/3 per true positive
        let points = pr_points(&[(0.0, 0.0), (1.0, 0.0), (4.0, 2.0)], 5.0, false);
        let xy: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
        assert_eq!(
            xy,
            [
                (0.0, 1.0),
                (0.2, 1.0),
                (0.4, 2.0 / (2.0 + 2.0 / 3.0)),
                (0.6, 3.0 / (3.0 + 4.0 / 3.0)),
                (0.8, 4.0 / 6.0)
            ]
        );
        // along FP: one false positive at a time, true positives growing by 3/2 each
        let points = pr_points(&[(1.0, 0.0), (4.0, 2.0)], 5.0, true);
        let xy: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
        assert_eq!(xy, [(0.2, 1.0), (0.5, 2.5 / 3.5), (0.8, 4.0 / 6.0)]);
    }
}
