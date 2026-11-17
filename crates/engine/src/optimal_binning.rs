//! OPTIMAL BINNING: bins of scale variables, by MDLP against a guide variable or by equal
//! frequency.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual:
//! - MDLP (Fayyad and Irani): within a set of cases, the candidate cuts lie between neighbouring
//!   distinct values unless both values are pure in the same guide class; the cut of least class
//!   information entropy is kept when its gain exceeds (log2(N - 1) + delta) / N, with
//!   delta = log2(3^k - 2) - (k Ent(S) - k1 Ent(S1) - k2 Ent(S2)) and k, k1, k2 the classes
//!   present. Both sides are then cut again the same way. Entropies in bits.
//! - Equal frequency into n bins: cuts at the i/n percentiles (i < n) by the empirical
//!   distribution with averaging; a variable with at most n distinct values gets one bin per
//!   value. PREPROCESS=EQUALFREQ(BINS=n) first groups the distinct values so, and MDLP then
//!   cuts only between the groups.
//! - FORCEMERGE: one pass over the inner bins; a bin smaller than the threshold times its
//!   smaller neighbour joins the neighbour that leaves the lower class information entropy.
//! - Weights rounded to whole numbers; a case of missing or rounded zero weight is left out.
//!   User-missing values are missing. PAIRWISE uses the cases valid on the variable (and the
//!   guide for MDLP), LISTWISE the cases valid on all of them.
//! - Choices where the manuals are silent are in docs/differences.md: the end points are data
//!   values (the lowest value of the upper bin, or with LOWERLIMIT=EXCLUSIVE the highest value
//!   of the lower bin), an observed end belongs to its bin, every case with a valid value is
//!   binned, and SPLIT FILE is not used.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::examine::Distribution;
use crate::npar_tests::text;
use crate::twostep::{free_name, numeric_value, plain_table};
use crate::variables::{duplicate, new_names};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Value};
use statscraft_syntax::{BinningMethod, OptimalBinning, PercentileMethod};

/// Neighbouring distinct values taken together: the lowest and highest of them and the weight
/// of each guide class (one class without a guide).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Unit {
    pub low: f64,
    pub high: f64,
    pub counts: Vec<f64>,
}

fn total(counts: &[f64]) -> f64 {
    counts.iter().sum()
}

/// Class entropy in bits.
fn entropy(counts: &[f64]) -> f64 {
    let n = total(counts);
    counts
        .iter()
        .filter(|&&c| c > 0.0)
        .map(|&c| -(c / n) * (c / n).log2())
        .sum()
}

fn present(counts: &[f64]) -> f64 {
    counts.iter().filter(|&&c| c > 0.0).count() as f64
}

/// n Ent(S): the class information entropy of a partition is the sum over its parts over N.
fn weighted(counts: &[f64]) -> f64 {
    total(counts) * entropy(counts)
}

fn join(a: &Unit, b: &Unit) -> Unit {
    Unit {
        low: a.low,
        high: b.high,
        counts: a.counts.iter().zip(&b.counts).map(|(x, y)| x + y).collect(),
    }
}

/// The units grouped at the cuts, a cut c falling between units c - 1 and c.
pub(crate) fn group(units: &[Unit], cuts: &[usize]) -> Vec<Unit> {
    let mut bounds = vec![0];
    bounds.extend_from_slice(cuts);
    bounds.push(units.len());
    bounds
        .windows(2)
        .map(|w| {
            units[w[0] + 1..w[1]]
                .iter()
                .fold(units[w[0]].clone(), |a, u| join(&a, u))
        })
        .collect()
}

/// The single class of a unit whose cases all share one, else None.
fn pure(unit: &Unit) -> Option<usize> {
    let mut classes = unit.counts.iter().enumerate().filter(|(_, c)| **c > 0.0);
    match (classes.next(), classes.next()) {
        (Some((j, _)), None) => Some(j),
        _ => None,
    }
}

/// MDLP cuts between the units, ascending.
pub(crate) fn mdlp(units: &[Unit]) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut stack = vec![(0, units.len())];
    while let Some((a, b)) = stack.pop() {
        if b - a < 2 {
            continue;
        }
        let all = group(&units[a..b], &[])[0].counts.clone();
        let n = total(&all);
        let mut left = vec![0.0; all.len()];
        // (n E(A, T; S), cut, left counts) of the best candidate so far
        let mut best: Option<(f64, usize, Vec<f64>)> = None;
        for i in a..b - 1 {
            for (l, c) in left.iter_mut().zip(&units[i].counts) {
                *l += c;
            }
            if pure(&units[i]).is_some() && pure(&units[i]) == pure(&units[i + 1]) {
                continue;
            }
            let right: Vec<f64> = all.iter().zip(&left).map(|(s, l)| s - l).collect();
            let e = weighted(&left) + weighted(&right);
            if best.as_ref().is_none_or(|(b, _, _)| e < *b) {
                best = Some((e, i + 1, left.clone()));
            }
        }
        let Some((e, cut, left)) = best else {
            continue;
        };
        let right: Vec<f64> = all.iter().zip(&left).map(|(s, l)| s - l).collect();
        let ent = entropy(&all);
        let gain = ent - e / n;
        let k = present(&all);
        let delta = (3f64.powf(k) - 2.0).log2()
            - (k * ent - present(&left) * entropy(&left) - present(&right) * entropy(&right));
        if gain > ((n - 1.0).log2() + delta) / n {
            cuts.push(cut);
            stack.push((a, cut));
            stack.push((cut, b));
        }
    }
    cuts.sort_unstable();
    cuts
}

/// Equal frequency cuts between distinct values (one unit each) into at most `bins` bins.
pub(crate) fn equal_frequency(units: &[Unit], bins: usize) -> Vec<usize> {
    if units.len() <= bins {
        return (1..units.len()).collect();
    }
    let pairs: Vec<(f64, f64)> = units.iter().map(|u| (u.low, total(&u.counts))).collect();
    let distribution = Distribution::new(&pairs);
    let mut cuts: Vec<usize> = Vec::new();
    for i in 1..bins {
        let percent = 100.0 * i as f64 / bins as f64;
        let Some(p) = distribution.percentile(percent, PercentileMethod::Aempirical) else {
            break;
        };
        // the bins below the cut hold the values up to the percentile
        let cut = units.partition_point(|u| u.high <= p);
        if cut > 0 && cut < units.len() && cuts.last() != Some(&cut) {
            cuts.push(cut);
        }
    }
    cuts
}

/// One pass of forced merging over the inner bins.
pub(crate) fn force_merge(mut bins: Vec<Unit>, threshold: f64) -> Vec<Unit> {
    let mut i = 1;
    while threshold > 0.0 && i + 1 < bins.len() {
        let (left, size, right) = (
            total(&bins[i - 1].counts),
            total(&bins[i].counts),
            total(&bins[i + 1].counts),
        );
        if size / left.min(right) >= threshold {
            i += 1;
            continue;
        }
        let with_left =
            weighted(&join(&bins[i - 1], &bins[i]).counts) + weighted(&bins[i + 1].counts);
        let with_right =
            weighted(&bins[i - 1].counts) + weighted(&join(&bins[i], &bins[i + 1]).counts);
        // on equal entropy (always without a guide) the smaller neighbour, then the lower one
        if with_left < with_right || (with_left == with_right && left <= right) {
            bins[i - 1] = join(&bins[i - 1], &bins[i]);
            bins.remove(i);
        } else {
            bins[i] = join(&bins[i], &bins[i + 1]);
            bins.remove(i + 1);
            i += 1;
        }
    }
    bins
}

/// The model entropy: the class entropy of each bin weighted by its share of the cases.
pub(crate) fn model_entropy(bins: &[Unit]) -> f64 {
    let n: f64 = bins.iter().map(|b| total(&b.counts)).sum();
    bins.iter().map(|b| weighted(&b.counts)).sum::<f64>() / n
}

/// The bins of one variable and what the tables show about it.
struct Binned {
    /// Its place on the BIN list.
    index: usize,
    name: String,
    shown: String,
    /// The saved and rule variable.
    target: String,
    bins: Vec<Unit>,
    /// The end points between the bins.
    ends: Vec<f64>,
    valid: f64,
    missing: f64,
    distinct: usize,
}

/// A number as syntax: the shortest text that reads back as the same number.
fn literal(x: f64) -> String {
    format!("{x}")
}

impl Binned {
    fn min(&self) -> f64 {
        self.bins[0].low
    }

    fn max(&self) -> f64 {
        self.bins[self.bins.len() - 1].high
    }

    /// The lower and upper end of bin `b`, None where unbounded.
    fn range(&self, b: usize, command: &OptimalBinning) -> (Option<f64>, Option<f64>) {
        let low = match b {
            0 => command.lower_observed.then(|| self.min()),
            b => Some(self.ends[b - 1]),
        };
        let high = match self.ends.get(b) {
            Some(&e) => Some(e),
            None => command.upper_observed.then(|| self.max()),
        };
        (low, high)
    }

    /// The bin number of a value; None outside an observed end.
    fn bin_of(&self, x: f64, command: &OptimalBinning) -> Option<f64> {
        if (command.lower_observed && x < self.min()) || (command.upper_observed && x > self.max())
        {
            return None;
        }
        let above = match command.lower_exclusive {
            false => self.ends.iter().filter(|&&e| e <= x).count(),
            true => self.ends.iter().filter(|&&e| e < x).count(),
        };
        Some((above + 1) as f64)
    }

    /// The conditions on the variable of each bin, as syntax (">= 2 AND x < 5") or as a label.
    fn conditions(&self, command: &OptimalBinning, var: &str) -> Vec<(String, String)> {
        let last = self.bins.len() - 1;
        (0..self.bins.len())
            .map(|b| {
                let (low, high) = self.range(b, command);
                // an observed end belongs to its bin
                let low_op = if command.lower_exclusive && b > 0 {
                    ">"
                } else {
                    ">="
                };
                let high_op = if !command.lower_exclusive && b < last {
                    "<"
                } else {
                    "<="
                };
                let parts: Vec<(&str, f64)> =
                    [low.map(|v| (low_op, v)), high.map(|v| (high_op, v))]
                        .into_iter()
                        .flatten()
                        .collect();
                let rule = parts
                    .iter()
                    .map(|(op, v)| format!("{var} {op} {}", literal(*v)))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                let label = match parts.is_empty() {
                    true => "All values".to_string(),
                    false => parts
                        .iter()
                        .map(|(op, v)| format!("{op} {v}"))
                        .collect::<Vec<_>>()
                        .join(" and "),
                };
                (rule, label)
            })
            .collect()
    }

    /// The rule syntax that bins the variable into the target variable.
    fn rules(&self, command: &OptimalBinning) -> String {
        let (var, target) = (&self.name, &self.target);
        let mut out = format!("* Bins of {var} into {target}.\nCOMPUTE {target} = $SYSMIS.\n");
        let conditions = self.conditions(command, var);
        for (b, (rule, _)) in conditions.iter().enumerate() {
            let rule = match rule.is_empty() {
                true => String::new(),
                false => format!(" AND {rule}"),
            };
            out += &format!("IF (NOT MISSING({var}){rule}) {target} = {}.\n", b + 1);
        }
        out += &format!("VARIABLE LABELS {target} '{var} (binned)'.\nVALUE LABELS {target}");
        for (b, (_, label)) in conditions.iter().enumerate() {
            out += &format!("\n  {} '{label}'", b + 1);
        }
        out += &format!(".\nFORMATS {target} (F8.0).\nVARIABLE LEVEL {target} (ORDINAL).\n");
        out
    }
}

impl Engine {
    pub(crate) fn optimal_binning(
        &mut self,
        command: &OptimalBinning,
    ) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let names = self.expand(&command.bin)?;
        if let Some(twice) = duplicate(&names) {
            return Err(Error(format!(
                "OPTIMAL BINNING: {twice} is given twice on BIN"
            )));
        }
        let guide = command
            .guide
            .as_ref()
            .map(|g| data.variable(g))
            .transpose()?;
        if let Some(g) = &guide
            && names.iter().any(|n| same(n, g.name()))
        {
            return Err(Error(format!(
                "OPTIMAL BINNING: the guide variable {} cannot also be binned",
                g.name()
            )));
        }
        if let Some(w) = data.weight_variable()
            && (names.iter().any(|n| same(n, w))
                || guide.as_ref().is_some_and(|g| same(g.name(), w)))
        {
            return Err(Error(format!(
                "OPTIMAL BINNING cannot use the weight variable {w} on a subcommand"
            )));
        }
        let cases = data.case_count();
        let weights: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; cases],
        };
        let mut variables = Vec::new();
        for name in &names {
            let variable = data.variable(name)?;
            if variable.is_string() {
                return Err(Error(format!(
                    "OPTIMAL BINNING: {name} is a string variable; only numeric variables are binned"
                )));
            }
            variables.push(variable);
        }
        let guide_keys = guide.as_ref().map(|g| categories(g, false));
        let class_of = |c: usize| guide_keys.as_ref().map(|k| k[c].as_ref());
        // the classes: the guide values of the cases that count
        let mut levels: Vec<Key> = Vec::new();
        if let Some(keys) = &guide_keys {
            for (c, key) in keys.iter().enumerate() {
                if let Some(key) = key
                    && weights[c] > 0.0
                    && !levels.iter().any(|l| l.compare(key).is_eq())
                {
                    levels.push(key.clone());
                }
            }
            levels.sort_by(|a, b| a.compare(b));
            if levels.len() > 256 {
                return Err(Error(format!(
                    "OPTIMAL BINNING: the guide variable has {} distinct values; at most 256 are allowed",
                    levels.len()
                )));
            }
        }
        let classes = levels.len().max(1);
        let values: Vec<Vec<Option<f64>>> = variables
            .iter()
            .map(|v| (0..cases).map(|c| numeric_value(v, c, false)).collect())
            .collect();
        let listwise_ok = |c: usize| {
            values.iter().all(|v| v[c].is_some()) && class_of(c).is_none_or(|k| k.is_some())
        };
        let counted: f64 = weights.iter().filter(|&&w| w > 0.0).sum();

        let mut warnings: Vec<String> = Vec::new();
        let mut binned: Vec<Binned> = Vec::new();
        let mut taken: Vec<String> = Vec::new();
        let wanted = match command.into.is_empty() {
            true => None,
            false => {
                let into = new_names(&command.into)?;
                if into.len() != names.len() {
                    return Err(Error(format!(
                        "OPTIMAL BINNING: INTO gives {} names for {} variables to bin",
                        into.len(),
                        names.len()
                    )));
                }
                if let Some(twice) = duplicate(&into) {
                    return Err(Error(format!(
                        "OPTIMAL BINNING: INTO gives the name {twice} twice"
                    )));
                }
                Some(into)
            }
        };
        let shown = self.shown();
        for (v, variable) in variables.iter().enumerate() {
            let name = variable.name().to_string();
            // (value, class, weight) of the cases this variable uses
            let mut cells: Vec<(f64, usize, f64)> = Vec::new();
            for c in 0..cases {
                let class = match class_of(c) {
                    None => Some(0),
                    Some(key) => key.and_then(|k| levels.iter().position(|l| l.compare(k).is_eq())),
                };
                if let (Some(x), Some(class)) = (values[v][c], class)
                    && weights[c] > 0.0
                    && (!command.listwise || listwise_ok(c))
                {
                    cells.push((x, class, weights[c]));
                }
            }
            if cells.is_empty() {
                warnings.push(format!(
                    "OPTIMAL BINNING: {name} has no valid cases, so it is not binned"
                ));
                continue;
            }
            cells.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut units: Vec<Unit> = Vec::new();
            for (x, class, w) in cells {
                if units.last().is_none_or(|u| u.low != x) {
                    units.push(Unit {
                        low: x,
                        high: x,
                        counts: vec![0.0; classes],
                    });
                }
                units.last_mut().expect("just pushed").counts[class] += w;
            }
            let distinct = units.len();
            let bins = match command.method {
                BinningMethod::EqualFreq(n) => {
                    let cuts = equal_frequency(&units, n);
                    group(&units, &cuts)
                }
                BinningMethod::Mdlp => {
                    let pre = match command.preprocess {
                        Some(n) if units.len() > n => group(&units, &equal_frequency(&units, n)),
                        _ => units,
                    };
                    let cuts = mdlp(&pre);
                    group(&pre, &cuts)
                }
            };
            let bins = force_merge(bins, command.force_merge);
            let ends = bins
                .windows(2)
                .map(|w| match command.lower_exclusive {
                    false => w[1].low,
                    true => w[0].high,
                })
                .collect();
            let valid: f64 = bins.iter().map(|b| total(&b.counts)).sum();
            let default = match &guide {
                Some(g) => format!("{}_{name}_bin", g.name()),
                None => format!("{name}_bin"),
            };
            let target = match &wanted {
                Some(into) => {
                    let want = &into[v];
                    if data.variable_index(want).is_some() {
                        return Err(Error(format!(
                            "OPTIMAL BINNING: INTO names {want}, a variable that already exists; choose another name"
                        )));
                    }
                    want.clone()
                }
                None => free_name(&default, |n| {
                    data.variable_index(n).is_some() || taken.iter().any(|t| same(t, n))
                }),
            };
            taken.push(target.clone());
            binned.push(Binned {
                index: v,
                shown: shown.variable(variable),
                name,
                target,
                bins,
                ends,
                valid,
                missing: counted - valid,
                distinct,
            });
        }

        let mut tables = Vec::new();
        if command.print_descriptives {
            let rows = binned
                .iter()
                .map(|b| {
                    vec![
                        text(&b.shown),
                        Cell::Number(b.valid),
                        Cell::Number(b.missing),
                        Cell::Number(b.distinct as f64),
                        Cell::Number(b.min()),
                        Cell::Number(b.max()),
                    ]
                })
                .collect();
            let columns = ["", "N", "Missing", "Distinct Values", "Minimum", "Maximum"];
            tables.push(plain_table("Descriptive Statistics", 1, &columns, rows));
            if let Some(g) = &guide {
                let rows = binned
                    .iter()
                    .map(|b| {
                        let all = group(&b.bins, &[])[0].counts.clone();
                        let mut row = vec![text(&b.shown)];
                        row.extend(all.iter().map(|&c| Cell::Number(c)));
                        row.push(Cell::Number(total(&all)));
                        row
                    })
                    .collect();
                let mut columns = vec![String::new()];
                columns.extend(levels.iter().map(|l| shown.value(g, l)));
                columns.push("Total".into());
                let mut table = plain_table("Guide Variable Distribution", 1, &columns, rows);
                for group in &mut table.column_groups[1..=levels.len()] {
                    group.push(shown.variable(g));
                }
                tables.push(table);
            }
        }
        if command.print_entropy {
            match command.method {
                BinningMethod::Mdlp => {
                    let rows = binned
                        .iter()
                        .map(|b| vec![text(&b.shown), number(Some(model_entropy(&b.bins)))])
                        .collect();
                    tables.push(plain_table(
                        "Model Entropy",
                        1,
                        &["", "Model Entropy"],
                        rows,
                    ));
                }
                BinningMethod::EqualFreq(_) => warnings.push(
                    "OPTIMAL BINNING: PRINT ENTROPY needs MDLP binning; no model entropy is shown"
                        .into(),
                ),
            }
        }
        if command.print_endpoints && !binned.is_empty() {
            let mut rows = Vec::new();
            for b in &binned {
                for (i, bin) in b.bins.iter().enumerate() {
                    let (low, high) = b.range(i, command);
                    let mut row = vec![
                        if i == 0 { text(&b.shown) } else { Cell::Empty },
                        Cell::Number((i + 1) as f64),
                        number(low),
                        number(high),
                    ];
                    if guide.is_some() {
                        row.extend(bin.counts.iter().map(|&c| Cell::Number(c)));
                    }
                    row.push(Cell::Number(total(&bin.counts)));
                    rows.push(row);
                }
            }
            let mut columns = vec![String::new(), "Bin".into(), "Lower".into(), "Upper".into()];
            if let Some(g) = &guide {
                columns.extend(levels.iter().map(|l| shown.value(g, l)));
            }
            columns.push("Total".into());
            let mut table = plain_table("End Points", 2, &columns, rows);
            for group in &mut table.column_groups[2..4] {
                group.push("End Point".into());
            }
            for group in &mut table.column_groups[4..] {
                group.push("Number of Cases".into());
            }
            if let Some(g) = &guide {
                for group in &mut table.column_groups[4..4 + levels.len()] {
                    group.push(shown.variable(g));
                }
            }
            table.caption = Some(match command.lower_exclusive {
                false => "A bin holds the values from its lower end point up to, but not including, its upper end point. An empty end point is unbounded.",
                true => "A bin holds the values above its lower end point up to and including its upper end point. An empty end point is unbounded.",
            }.into());
            tables.push(table);
        }

        if let Some(file) = &command.rules
            && !self.dry_run
        {
            let text: String = binned.iter().map(|b| b.rules(command)).collect();
            let path = self.path(file);
            std::fs::write(&path, text).map_err(|e| {
                Error(format!(
                    "OPTIMAL BINNING cannot write the RULES file {}: {e}",
                    path.display()
                ))
            })?;
        }
        for warning in warnings {
            self.warn(warning);
        }
        if !command.save {
            return Ok(tables);
        }
        let saved: Vec<Vec<Option<f64>>> = binned
            .iter()
            .map(|b| {
                values[b.index]
                    .iter()
                    .map(|x| x.and_then(|x| b.bin_of(x, command)))
                    .collect()
            })
            .collect();
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (b, values) in binned.iter().zip(saved) {
                data.set_numeric_values(&b.target, values)?;
                data.set_format(&b.target, "F8.0")?;
                data.set_measure(&b.target, Measure::Ordinal)?;
                data.set_variable_label(&b.target, Some(&format!("{} (binned)", b.name)))?;
                let labels = b
                    .conditions(command, &b.name)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (_, label))| (Value::Number((i + 1) as f64), label))
                    .collect();
                data.set_value_labels(&b.target, labels)?;
            }
            Ok(())
        })?;
        Ok(tables)
    }
}
