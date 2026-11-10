//! RANK: ranks, fractional ranks, proportion estimates, normal and Savage scores, ntiles and
//! the sum of weights of numeric variables, saved as new variables.
//!
//! Rules from the Command Syntax Reference (RANK) and the Algorithms manual (RANK):
//! - Within each group of equal BY values, the distinct values y_1 < ... < y_m (in the order
//!   asked for: D ranks the largest first) have summed weights C_i, cumulative weights CC_i
//!   and the total W.
//! - Ranks: LOW CC_i-1 + 1, HIGH CC_i, MEAN CC_i-1 + (C_i + 1) / 2, CONDENSE i; when C_i < 1,
//!   LOW is CC_i-1 and MEAN CC_i-1 + C_i / 2.
//! - RFRACTION R / W, PERCENT 100 R / W, N W, NTILES(k) the integer part of R k / (W + 1) plus 1,
//!   PROPORTION by BLOM (R - 3/8) / (W + 1/4), RANKIT (R - 1/2) / W, TUKEY (R - 1/3) / (W + 1/3)
//!   or VW R / (W + 1), missing when negative; NORMAL the standard normal quantile of it.
//! - SAVAGE from the expected exponential order statistics l_j, whatever TIES says.
//! - Missing values (user-missing ones unless INCLUDE) get no rank and system-missing results.
//! - Default names: the function's first letter and the first seven bytes of the name, then
//!   the first three letters of the function and 001, 002, ..., then RNK, two letters and 01,
//!   02, ...; labels are the function, "of" and the variable, as in RANK of salary.
//!
//! Where the manual is silent: a case with a missing, zero or negative weight gets no rank, as
//! in the other procedures; missing values of BY variables form groups of their own, as SPLIT
//! FILE does; a label adds "by" and the BY variables; the new variables get formats that show
//! their precision (rank F9.3, fractions F6.4, percent F6.2, scores F8.4 and F6.4, N F6.0 or
//! F8.2 when weighted); a variable named twice is ranked once, with a warning.

use crate::descriptives::case_weights;
use crate::dist::normal_quantile;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values};
use statscraft_syntax::{Rank, RankFraction, RankFunction as F, RankTies};
use std::collections::HashMap;

const MAX_DEFAULT_NAME: usize = 8;

fn function_name(function: F) -> &'static str {
    match function {
        F::Rank => "RANK",
        F::Rfraction => "RFRACTION",
        F::Percent => "PERCENT",
        F::Proportion => "PROPORTION",
        F::N => "N",
        F::Ntiles(_) => "NTILES",
        F::Savage => "SAVAGE",
        F::Normal => "NORMAL",
    }
}

/// The first `bytes` bytes of `name`, cut back to a character boundary.
fn prefix(name: &str, bytes: usize) -> &str {
    let mut end = bytes.min(name.len());
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

/// The harmonic number H(n) = 1 + 1/2 + ... + 1/n for a whole n >= 0.
fn harmonic(n: f64) -> f64 {
    if n <= 1000.0 {
        return (1..=n as u64).map(|k| 1.0 / k as f64).sum();
    }
    // the asymptotic series is exact to double precision this far out
    let n2 = n * n;
    n.ln() + std::f64::consts::EULER_GAMMA + 1.0 / (2.0 * n) - 1.0 / (12.0 * n2)
        + 1.0 / (120.0 * n2 * n2)
}

/// The Savage scores of the distinct values with cumulative weights `cc` (cc[0] = 0).
fn savage(cc: &[f64]) -> Vec<f64> {
    let w = cc[cc.len() - 1];
    let fuzz = 1e-9 * w.max(1.0);
    let whole = (w + fuzz).floor();
    let w_star = if (w - whole).abs() <= fuzz {
        whole
    } else {
        whole + 1.0
    };
    let h_star = harmonic(w_star);
    // l_j = 1 / W* + 1 / (W* - 1) + ... + 1 / (W* - j + 1)
    let l = |j: f64| h_star - harmonic(w_star - j);
    // a coefficient of 0 leaves its term out, also where l_j is not defined
    let term = |coefficient: f64, j: f64| {
        if coefficient.abs() <= fuzz {
            0.0
        } else {
            coefficient * l(j)
        }
    };
    cc.windows(2)
        .map(|pair| {
            let (before, upto) = (pair[0], pair[1]);
            let i1 = (before + fuzz).floor();
            let i2 = (upto + fuzz).floor();
            let (g1, g2) = ((before - i1).max(0.0), (upto - i2).max(0.0));
            let c = upto - before;
            if i1 == i2 {
                l(i1 + 1.0) - 1.0
            } else {
                let inner: f64 = (i1 as u64 + 2..=i2 as u64).map(|j| l(j as f64)).sum();
                (term(1.0 - g1, i1 + 1.0) + term(g2, i2 + 1.0) + inner) / c - 1.0
            }
        })
        .collect()
}

/// The result of `function` for every distinct value of one group, from the ranks `r`, the
/// cumulative weights `cc` (cc[0] = 0) and the total `w`.
fn results(function: F, fraction: RankFraction, r: &[f64], cc: &[f64]) -> Vec<Option<f64>> {
    let w = cc[cc.len() - 1];
    let proportion = |r: f64| {
        let p = match fraction {
            RankFraction::Blom => (r - 3.0 / 8.0) / (w + 0.25),
            RankFraction::Rankit => (r - 0.5) / w,
            RankFraction::Tukey => (r - 1.0 / 3.0) / (w + 1.0 / 3.0),
            RankFraction::Vw => r / (w + 1.0),
        };
        (p >= 0.0).then_some(p)
    };
    match function {
        F::Savage => savage(cc).into_iter().map(Some).collect(),
        _ => r
            .iter()
            .map(|&r| match function {
                F::Rank => Some(r),
                F::Rfraction => Some(r / w),
                F::Percent => Some(100.0 * r / w),
                F::N => Some(w),
                F::Ntiles(k) => Some((r * f64::from(k) / (w + 1.0)).floor() + 1.0),
                F::Proportion => proportion(r),
                F::Normal => proportion(r)
                    .filter(|p| *p > 0.0 && *p < 1.0)
                    .map(normal_quantile),
                F::Savage => unreachable!("handled above"),
            })
            .collect(),
    }
}

/// The ranks of distinct values with weights `c`, and their cumulative weights from 0.
fn ranks(c: &[f64], ties: RankTies) -> (Vec<f64>, Vec<f64>) {
    let mut cc = vec![0.0];
    for w in c {
        cc.push(cc[cc.len() - 1] + w);
    }
    let r = c
        .iter()
        .enumerate()
        .map(|(i, &ci)| {
            let before = cc[i];
            match (ties, ci >= 1.0) {
                (RankTies::Low, true) => before + 1.0,
                (RankTies::Low, false) => before,
                (RankTies::High, _) => cc[i + 1],
                (RankTies::Mean, true) => before + (ci + 1.0) / 2.0,
                (RankTies::Mean, false) => before + ci / 2.0,
                (RankTies::Condense, _) => (i + 1) as f64,
            }
        })
        .collect();
    (r, cc)
}

/// A value of a BY variable as a key for grouping; None is system-missing.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Number(Option<u64>),
    Text(String),
}

impl Engine {
    pub(crate) fn rank(&mut self, command: &Rank) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut sources: Vec<(String, bool)> = Vec::new();
        let mut twice = Vec::new();
        for (item, descending) in &command.variables {
            for name in self.expand(std::slice::from_ref(item))? {
                if data.variable(&name)?.is_string() {
                    return Err(Error(format!(
                        "RANK ranks numeric variables only, and {name} is a string variable; \
                         AUTORECODE turns it into numbers"
                    )));
                }
                if sources.iter().any(|(s, _)| s.eq_ignore_ascii_case(&name)) {
                    twice.push(name);
                } else {
                    sources.push((name, *descending));
                }
            }
        }
        let by = self.expand(&command.by)?;

        // the names of the new variables, function by function in the order written
        let mut taken: Vec<String> = data.variable_names().map(str::to_lowercase).collect();
        for (_, into) in &command.functions {
            if into.len() > sources.len() {
                return Err(Error(format!(
                    "INTO gives {} names for {} ranked variables",
                    into.len(),
                    sources.len()
                )));
            }
            for name in into {
                if taken.contains(&name.to_lowercase()) {
                    return Err(Error(format!(
                        "the new variable name {name} is already in use; choose another name"
                    )));
                }
                taken.push(name.to_lowercase());
            }
        }
        let mut names: Vec<Vec<String>> = Vec::new();
        for (function, into) in &command.functions {
            let full = function_name(*function);
            let mut counter = 1;
            let mut short = 1;
            let mut chosen = Vec::new();
            for (index, (source, _)) in sources.iter().enumerate() {
                let name = match into.get(index) {
                    Some(name) => name.clone(),
                    None => {
                        let free = |name: &String| !taken.contains(&name.to_lowercase());
                        let natural =
                            format!("{}{}", &full[..1], prefix(source, MAX_DEFAULT_NAME - 1));
                        let name = if free(&natural) {
                            Some(natural)
                        } else {
                            let stem = prefix(full, 3);
                            let mut found = None;
                            while found.is_none() && counter <= 999 {
                                let name = format!("{stem}{counter:03}");
                                counter += 1;
                                found = Some(name).filter(free);
                            }
                            let stem = format!("RNK{}", prefix(full, 2));
                            while found.is_none() && short <= 99 {
                                let name = format!("{stem}{short:02}");
                                short += 1;
                                found = Some(name).filter(free);
                            }
                            found
                        };
                        let Some(name) = name else {
                            return Err(Error(format!(
                                "no free name for the {full} of {source}; name it with INTO"
                            )));
                        };
                        taken.push(name.to_lowercase());
                        name
                    }
                };
                chosen.push(name);
            }
            names.push(chosen);
        }
        let used_fraction = command
            .functions
            .iter()
            .any(|(f, _)| matches!(f, F::Normal | F::Proportion));
        for name in twice {
            self.warn(format!(
                "{name} is ranked once; the second mention is ignored"
            ));
        }
        if command.fraction.is_some() && !used_fraction {
            self.warn("FRACTION applies to NORMAL and PROPORTION only; it is ignored here");
        }
        let fraction = command.fraction.unwrap_or(RankFraction::Blom);

        // the cases of every group of BY values
        let data = self.active()?;
        let weights = case_weights(data)?;
        let cases = data.case_count();
        let mut groups: HashMap<Vec<Key>, Vec<usize>> = HashMap::new();
        let mut by_columns = Vec::new();
        for name in &by {
            by_columns.push(data.variable(name)?);
        }
        for case in 0..cases {
            if weights.as_ref().is_some_and(|w| w[case] <= 0.0) {
                continue;
            }
            let key: Vec<Key> = by_columns
                .iter()
                .map(|v| match v.values() {
                    Values::Numeric(values) => Key::Number(values.get(case).map(f64::to_bits)),
                    Values::String(values) => Key::Text(values.get(case).to_string()),
                })
                .collect();
            groups.entry(key).or_default().push(case);
        }

        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let mut columns: Vec<(String, F, Vec<Option<f64>>)> = Vec::new();
        for (index, (source, descending)) in sources.iter().enumerate() {
            let variable = data.variable(source)?;
            let values = variable.numeric()?;
            let mut out: Vec<Vec<Option<f64>>> = vec![vec![None; cases]; command.functions.len()];
            for members in groups.values() {
                let mut valid: Vec<(f64, usize)> = members
                    .iter()
                    .filter_map(|&case| {
                        let v = values.get(case)?;
                        (command.include || !variable.is_user_missing(v)).then_some((v, case))
                    })
                    .collect();
                if valid.is_empty() {
                    continue;
                }
                valid.sort_by(|a, b| {
                    let order = a.0.total_cmp(&b.0);
                    if *descending { order.reverse() } else { order }
                });
                // distinct values, each with its weight and its cases
                let mut c: Vec<f64> = Vec::new();
                let mut of_value: Vec<usize> = Vec::new();
                let mut last = None;
                for &(v, case) in &valid {
                    if last != Some(v) {
                        c.push(0.0);
                        last = Some(v);
                    }
                    *c.last_mut().expect("pushed") += weight(case);
                    of_value.push(c.len() - 1);
                }
                let (r, cc) = ranks(&c, command.ties);
                for (f, (function, _)) in command.functions.iter().enumerate() {
                    let result = results(*function, fraction, &r, &cc);
                    for (&(_, case), &i) in valid.iter().zip(&of_value) {
                        out[f][case] = result[i];
                    }
                }
            }
            for (f, (function, _)) in command.functions.iter().enumerate() {
                columns.push((
                    names[f][index].clone(),
                    *function,
                    std::mem::take(&mut out[f]),
                ));
            }
        }

        let by_text = if by.is_empty() {
            String::new()
        } else {
            format!(" by {}", by.join(" "))
        };
        let weighted = weights.is_some();
        let mut rows = Vec::new();
        let created: Vec<(String, String, String, F)> = sources
            .iter()
            .enumerate()
            .flat_map(|(index, (source, _))| {
                let names = &names;
                command
                    .functions
                    .iter()
                    .enumerate()
                    .map(move |(f, (function, _))| {
                        let label = format!("{} of {source}", function_name(*function));
                        (source.clone(), names[f][index].clone(), label, *function)
                    })
            })
            .map(|(source, name, label, function)| {
                (source, name, format!("{label}{by_text}"), function)
            })
            .collect();
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for ((name, function, values), (_, _, label, _)) in columns.into_iter().zip(&created) {
                data.set_numeric_values(&name, values)?;
                data.set_variable_label(&name, Some(label))?;
                let format = match function {
                    F::Rank => "F9.3".to_string(),
                    F::Rfraction | F::Proportion | F::Normal => "F6.4".to_string(),
                    F::Percent => "F6.2".to_string(),
                    F::N if weighted => "F8.2".to_string(),
                    F::N => "F6.0".to_string(),
                    F::Ntiles(k) => format!("F{}.0", k.to_string().len().max(3)),
                    F::Savage => "F8.4".to_string(),
                };
                data.set_format(&name, &format)?;
                let measure = if function == F::N {
                    Measure::Scale
                } else {
                    Measure::Ordinal
                };
                data.set_measure(&name, measure)?;
            }
            Ok(())
        })?;
        if !command.print {
            return Ok(Vec::new());
        }
        for (source, name, label, function) in &created {
            let function = match function {
                F::Ntiles(k) => format!("NTILES({k})"),
                F::Proportion | F::Normal => {
                    let formula = match fraction {
                        RankFraction::Blom => "BLOM",
                        RankFraction::Rankit => "RANKIT",
                        RankFraction::Tukey => "TUKEY",
                        RankFraction::Vw => "VW",
                    };
                    format!("{} ({formula})", function_name(*function))
                }
                other => function_name(*other).to_string(),
            };
            rows.push(vec![
                Cell::Text(source.clone()),
                Cell::Text(function),
                Cell::Text(name.clone()),
                Cell::Text(label.clone()),
            ]);
        }
        let ties = match command.ties {
            RankTies::Mean => "the mean rank",
            RankTies::Low => "the lowest rank",
            RankTies::High => "the highest rank",
            RankTies::Condense => "one rank, the next value the next rank",
        };
        let columns = ["Source Variable", "Function", "New Variable", "Label"];
        Ok(vec![Table {
            title: "Created Variables".into(),
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns: columns.map(String::from).to_vec(),
            rows,
            caption: Some(format!("Tied values get {ties}.")),
            ..Table::default()
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harmonic_numbers_join_smoothly() {
        let direct: f64 = (1..=5000u64).map(|k| 1.0 / k as f64).sum();
        assert!((harmonic(5000.0) - direct).abs() < 1e-12);
        assert_eq!(harmonic(0.0), 0.0);
        assert!((harmonic(3.0) - 11.0 / 6.0).abs() < 1e-15);
    }

    #[test]
    fn savage_scores_by_hand() {
        // six cases, two tied at the second place: l = 1/6, +1/5, +1/4, ...
        let (_, cc) = ranks(&[1.0, 2.0, 1.0, 1.0, 1.0], RankTies::Mean);
        let s = savage(&cc);
        let l1 = 1.0 / 6.0;
        let l2 = l1 + 0.2;
        let l3 = l2 + 0.25;
        assert!((s[0] - (l1 - 1.0)).abs() < 1e-12);
        assert!((s[1] - ((l2 + l3) / 2.0 - 1.0)).abs() < 1e-12);
        // the scores of all cases add up to 0
        let total: f64 = s
            .iter()
            .zip([1.0, 2.0, 1.0, 1.0, 1.0])
            .map(|(s, c)| s * c)
            .sum();
        assert!(total.abs() < 1e-12);
    }

    #[test]
    fn ranks_with_small_weights() {
        // a weight below 1 takes C_i / 2 for the mean and CC_i-1 for the lowest rank
        let close = |got: Vec<f64>, want: [f64; 3]| {
            assert!(
                got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-12),
                "{got:?}"
            );
        };
        close(ranks(&[1.0, 0.4, 2.0], RankTies::Mean).0, [1.0, 1.2, 2.9]);
        close(ranks(&[1.0, 0.4, 2.0], RankTies::Low).0, [1.0, 1.0, 2.4]);
        close(ranks(&[1.0, 0.4, 2.0], RankTies::High).0, [1.0, 1.4, 3.4]);
    }
}
