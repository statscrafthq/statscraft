//! ORTHOPLAN: an orthogonal main-effects plan of profiles for a conjoint study; PLANCARDS: the
//! profiles of a plan as a list or as cards.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (ORTHOPLAN), in our words:
//! - The plan is the smallest orthogonal main-effects plan with at least MINIMUM profiles; when
//!   no plan is that large, the largest one that fits. At most 9 levels per factor, 90 levels in
//!   all and 81 profiles. A factor with fewer levels than its column folds the column's
//!   symbols onto its levels, which keeps the columns orthogonal.
//! - Which columns the factors take is random (SET SEED repeats a plan). STATUS_ is 0 for
//!   the plan, 1 for HOLDOUT profiles (random profiles that repeat neither the plan nor each
//!   other, after the plan or mixed in with MIXHOLD=YES) and 2 for simulation profiles, the
//!   cases already in the active dataset, which follow. CARD_ numbers the profiles.
//! - Without FACTORS the variables of the active dataset (but STATUS_ and CARD_) are the
//!   factors and their value labels the levels. OUTFILE writes the plan to a file or dataset;
//!   REPLACE replaces the active dataset; otherwise the plan is added to it. SPLIT FILE and
//!   WEIGHT are not used.
//! - PLANCARDS lists the profiles (holdouts marked, simulation profiles in a table of their own,
//!   one listing per split group) or shows each experimental or holdout profile as a card, with
//!   the TITLE and FOOTER lines (")CARD" becomes the profile's number on a card). OUTFILE writes
//!   the cards as text instead.
//!
//! Choices where the manual is silent:
//! - The plans are built, not taken from a printed library: the runs are all vectors of
//!   GF(p)^k (p = 2, 3, 5 or 7, at most 81 runs) and a factor of l levels reads a subspace of
//!   dimension r (p^r >= l) of linear forms; factors with subspaces that meet only in 0 are
//!   orthogonal. The full factorial is a candidate too, and wins a tie. Sizes can differ from
//!   other programs' libraries (no 12, 18 or 20 run plans).
//! - Folded symbols are spread as evenly as the counts allow; the runs are in random order.
//! - Texts without values in FACTORS become levels 1, 2, ... with the texts as labels.

use crate::descriptives::number;
use crate::npar_tests::text;
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Value};
use statscraft_syntax::{CardFormat, Orthoplan, Plancards, VarItem};

const MAX_CASES: usize = 81;

/// Factor titles, then per profile STATUS_, CARD_ and the shown levels.
type Profiles = (Vec<String>, Vec<f64>, Vec<f64>, Vec<Vec<String>>);

struct Factor {
    name: String,
    label: Option<String>,
    values: Vec<f64>,
    labels: Vec<Option<String>>,
}

fn pow(p: usize, k: usize) -> usize {
    p.pow(k as u32)
}

/// The digit-wise combination a + c b of two vectors of GF(p)^k coded in base p.
fn add_scaled(a: usize, c: usize, b: usize, p: usize, k: usize) -> usize {
    let (mut out, mut place) = (0, 1);
    for _ in 0..k {
        out += ((a / place % p + c * (b / place % p)) % p) * place;
        place *= p;
    }
    out
}

fn dot(a: usize, x: usize, p: usize, k: usize) -> usize {
    let (mut sum, mut place) = (0, 1);
    for _ in 0..k {
        sum += (a / place % p) * (x / place % p);
        place *= p;
    }
    sum % p
}

/// Every combination of the basis vectors, the coefficients read as base-p digits.
fn span(basis: &[usize], p: usize, k: usize) -> Vec<usize> {
    (0..pow(p, basis.len()))
        .map(|c| {
            let mut v = 0;
            for (i, &b) in basis.iter().enumerate() {
                v = add_scaled(v, c / pow(p, i) % p, b, p, k);
            }
            v
        })
        .collect()
}

fn below(m: usize, draw: &mut dyn FnMut() -> f64) -> usize {
    ((draw() * m as f64) as usize).min(m - 1)
}

fn shuffle<T>(items: &mut [T], draw: &mut dyn FnMut() -> f64) {
    for i in (1..items.len()).rev() {
        items.swap(i, below(i + 1, draw));
    }
}

/// A plan of p^k runs: per run the level (0-based) of every factor, or None when the factors
/// do not fit. Each factor reads a subspace of linear forms that meets the others only in 0.
fn linear_plan(
    levels: &[usize],
    p: usize,
    k: usize,
    draw: &mut dyn FnMut() -> f64,
) -> Option<Vec<Vec<usize>>> {
    let n = pow(p, k);
    let dims: Vec<usize> = levels
        .iter()
        .map(|&l| (1..).find(|&r| pow(p, r) >= l).expect("levels are few"))
        .collect();
    let needed: usize = dims.iter().map(|&r| pow(p, r) - 1).sum();
    if dims.iter().any(|&r| r > k) || needed > n - 1 {
        return None;
    }
    let mut order: Vec<usize> = (0..levels.len()).collect();
    order.sort_by_key(|&f| std::cmp::Reverse(dims[f]));
    // a random search: tight packings may need several starts
    'attempt: for _ in 0..200 {
        let mut used = vec![false; n];
        let mut bases = vec![Vec::new(); levels.len()];
        for &f in &order {
            let mut placed = false;
            for _ in 0..200 {
                let free: Vec<usize> = (1..n).filter(|&v| !used[v]).collect();
                if free.len() < dims[f] {
                    break;
                }
                let basis: Vec<usize> = (0..dims[f])
                    .map(|_| free[below(free.len(), draw)])
                    .collect();
                let s = span(&basis, p, k);
                let mut distinct = s.clone();
                distinct.sort_unstable();
                distinct.dedup();
                if distinct.len() < s.len() || s.iter().any(|&v| used[v]) {
                    continue;
                }
                for &v in &s {
                    used[v] = v != 0;
                }
                bases[f] = basis;
                placed = true;
                break;
            }
            if !placed {
                continue 'attempt;
            }
        }
        // each factor's symbols, shuffled, then folded onto its levels
        let maps: Vec<Vec<usize>> = levels
            .iter()
            .zip(&dims)
            .map(|(&l, &r)| {
                let mut symbols: Vec<usize> = (0..pow(p, r)).collect();
                shuffle(&mut symbols, draw);
                symbols.into_iter().map(|s| s % l).collect()
            })
            .collect();
        let runs = (0..n)
            .map(|x| {
                (0..levels.len())
                    .map(|f| {
                        let symbol: usize = bases[f]
                            .iter()
                            .enumerate()
                            .map(|(i, &b)| dot(b, x, p, k) * pow(p, i))
                            .sum();
                        maps[f][symbol]
                    })
                    .collect()
            })
            .collect();
        return Some(runs);
    }
    None
}

/// Every combination of the levels, the last factor changing fastest.
fn full_factorial(levels: &[usize]) -> Vec<Vec<usize>> {
    let total: usize = levels.iter().product();
    (0..total)
        .map(|mut i| {
            let mut run = vec![0; levels.len()];
            for f in (0..levels.len()).rev() {
                run[f] = i % levels[f];
                i /= levels[f];
            }
            run
        })
        .collect()
}

/// The smallest orthogonal plan with at least `minimum` runs, else the largest there is; the
/// runs in random order. Also says whether it is smaller than asked.
pub(crate) fn plan(
    levels: &[usize],
    minimum: usize,
    draw: &mut dyn FnMut() -> f64,
) -> Option<(Vec<Vec<usize>>, bool)> {
    let full: f64 = levels.iter().map(|&l| l as f64).product();
    // (runs, p, k) with p = 0 for the full factorial, which comes first on a tie
    let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
    if full <= MAX_CASES as f64 {
        candidates.push((full as usize, 0, 0));
    }
    for p in [2, 3, 5, 7] {
        for k in (1..).take_while(|&k| pow(p, k) <= MAX_CASES) {
            candidates.push((pow(p, k), p, k));
        }
    }
    candidates.sort_unstable();
    let mut build = |&(_, p, k): &(usize, usize, usize)| match p {
        0 => Some(full_factorial(levels)),
        _ => linear_plan(levels, p, k, draw),
    };
    let large = candidates.iter().filter(|c| c.0 >= minimum);
    let small = candidates.iter().rev().filter(|c| c.0 < minimum);
    let (mut runs, short) = large
        .filter_map(&mut build)
        .map(|r| (r, false))
        .next()
        .or_else(|| small.filter_map(&mut build).map(|r| (r, true)).next())?;
    shuffle(&mut runs, draw);
    Some((runs, short))
}

/// The value of every level of a factor, or the factor's error.
fn factor_levels(
    factor: &statscraft_syntax::PlanFactor,
    active: Option<&Dataset>,
) -> Result<Factor, Error> {
    let name = &factor.name;
    let (values, labels): (Vec<f64>, Vec<Option<String>>) = match &factor.levels {
        Some(levels) if levels.iter().all(|l| l.value.is_none()) => levels
            .iter()
            .enumerate()
            .map(|(i, l)| ((i + 1) as f64, l.label.clone()))
            .unzip(),
        Some(levels) if levels.iter().all(|l| l.value.is_some()) => levels
            .iter()
            .map(|l| (l.value.expect("checked"), l.label.clone()))
            .unzip(),
        Some(_) => {
            return Err(Error(format!(
                "ORTHOPLAN: the levels of {name} mix values and texts without values; give \
                 either values (each with an optional label) or only texts"
            )));
        }
        None => labelled_levels(name, active)?,
    };
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    sorted.dedup();
    if sorted.len() < values.len() {
        return Err(Error(format!("ORTHOPLAN: {name} has a level twice")));
    }
    if !(2..=9).contains(&values.len()) {
        return Err(Error(format!(
            "ORTHOPLAN: {name} has {} levels; a factor needs 2 to 9",
            values.len()
        )));
    }
    let label = factor
        .label
        .clone()
        .or_else(|| active.and_then(|d| d.variable(name).ok()?.label().map(str::to_string)));
    Ok(Factor {
        name: name.clone(),
        label,
        values,
        labels,
    })
}

/// The levels of a variable of the active dataset: its labelled values.
fn labelled_levels(
    name: &str,
    active: Option<&Dataset>,
) -> Result<(Vec<f64>, Vec<Option<String>>), Error> {
    let variable = active
        .and_then(|d| d.variable(name).ok())
        .ok_or_else(|| Error(format!("ORTHOPLAN: give the levels of {name} on FACTORS")))?;
    if variable.is_string() {
        return Err(Error(format!(
            "ORTHOPLAN: {name} is a string variable; factors are numeric"
        )));
    }
    let mut levels: Vec<(f64, String)> = variable
        .value_labels()
        .iter()
        .filter_map(|(v, l)| match v {
            Value::Number(v) => Some((*v, l.clone())),
            Value::String(_) => None,
        })
        .collect();
    levels.sort_by(|a, b| a.0.total_cmp(&b.0));
    if levels.is_empty() {
        return Err(Error(format!(
            "ORTHOPLAN: {name} has no value labels to take its levels from; give them on FACTORS \
             or with VALUE LABELS"
        )));
    }
    Ok(levels.into_iter().map(|(v, l)| (v, Some(l))).unzip())
}

fn is_plan_variable(name: &str) -> bool {
    name.eq_ignore_ascii_case("STATUS_") || name.eq_ignore_ascii_case("CARD_")
}

/// The factors of a plan file: every variable but STATUS_ and CARD_.
pub(crate) fn plan_factors(data: &Dataset) -> Vec<String> {
    data.variable_names()
        .filter(|n| !is_plan_variable(n))
        .map(str::to_string)
        .collect()
}

fn status_name(status: f64) -> &'static str {
    match status as i64 {
        0 => "Design",
        1 => "Holdout",
        _ => "Simulation",
    }
}

impl Engine {
    pub(crate) fn orthoplan(&mut self, c: &Orthoplan) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let active = self
            .dataset
            .as_ref()
            .filter(|d| d.variable_count() > 0)
            .cloned();
        let factors: Vec<Factor> = match c.factors.is_empty() {
            false => c
                .factors
                .iter()
                .map(|f| factor_levels(f, active.as_ref()))
                .collect::<Result<_, _>>()?,
            true => {
                let Some(data) = &active else {
                    return Err(Error(
                        "ORTHOPLAN needs FACTORS when there is no active dataset".into(),
                    ));
                };
                plan_factors(data)
                    .into_iter()
                    .map(|name| {
                        let f = statscraft_syntax::PlanFactor {
                            name,
                            label: None,
                            levels: None,
                        };
                        factor_levels(&f, Some(data))
                    })
                    .collect::<Result<_, _>>()?
            }
        };
        if factors.is_empty() {
            return Err(Error("ORTHOPLAN: there are no factors".into()));
        }
        let levels: Vec<usize> = factors.iter().map(|f| f.values.len()).collect();
        if levels.iter().sum::<usize>() > 90 {
            return Err(Error(
                "ORTHOPLAN: the factors have more than 90 levels in all".into(),
            ));
        }
        let full: f64 = levels.iter().map(|&l| l as f64).product();
        for (n, what) in [(c.minimum.unwrap_or(1), "MINIMUM"), (c.holdout, "HOLDOUT")] {
            if n as f64 > full {
                return Err(Error(format!(
                    "ORTHOPLAN: {what}={n} is more than the {full} combinations of the levels"
                )));
            }
        }
        // the cases already there are simulation profiles when their variables are the factors
        let mut theirs: Vec<String> = active.as_ref().map(plan_factors).unwrap_or_default();
        let mut ours: Vec<String> = factors.iter().map(|f| f.name.to_uppercase()).collect();
        theirs.iter_mut().for_each(|n| *n = n.to_uppercase());
        theirs.sort();
        ours.sort();
        let matching = active.is_some() && theirs == ours;
        if active.is_some() && !matching && !c.replace && c.outfile.is_none() {
            return Err(Error(
                "ORTHOPLAN: the variables of the active dataset are not the factors; use REPLACE \
                 or OUTFILE, or name the same factors"
                    .into(),
            ));
        }
        let keep_existing = matching && (c.outfile.is_some() || !c.replace);

        let mut draw = || self.random.get_mut().uniform();
        let (runs, short) = plan(&levels, c.minimum.unwrap_or(1), &mut draw).ok_or_else(|| {
            Error("ORTHOPLAN: no orthogonal plan of at most 81 profiles fits these factors".into())
        })?;
        let mut warnings = Vec::new();
        if short {
            warnings.push(format!(
                "ORTHOPLAN: the largest plan for these factors has {} profiles, fewer than MINIMUM",
                runs.len()
            ));
        }
        let mut distinct = runs.clone();
        distinct.sort();
        distinct.dedup();
        if distinct.len() < runs.len() {
            warnings.push(format!(
                "ORTHOPLAN: the plan repeats {} profiles; another SET SEED may give one without",
                runs.len() - distinct.len()
            ));
        }
        // holdouts: random combinations outside the plan and unlike each other
        let available = full - distinct.len() as f64;
        if c.holdout as f64 > available {
            return Err(Error(format!(
                "ORTHOPLAN: HOLDOUT={} but only {available} combinations are not in the plan",
                c.holdout
            )));
        }
        let mut holdouts: Vec<Vec<usize>> = Vec::new();
        while holdouts.len() < c.holdout {
            let run: Vec<usize> = levels.iter().map(|&l| below(l, &mut draw)).collect();
            if !distinct.contains(&run) && !holdouts.contains(&run) {
                holdouts.push(run);
            }
        }
        let mut profiles: Vec<(Vec<usize>, f64)> = runs.iter().map(|r| (r.clone(), 0.0)).collect();
        profiles.extend(holdouts.into_iter().map(|r| (r, 1.0)));
        if c.mixhold {
            shuffle(&mut profiles, &mut draw);
        }

        let n = profiles.len();
        let mut out = Dataset::new(n);
        for (f, factor) in factors.iter().enumerate() {
            let values = profiles.iter().map(|(r, _)| Some(factor.values[r[f]]));
            out.set_numeric_values(&factor.name, values.collect())?;
            let whole = factor.values.iter().all(|v| v.fract() == 0.0);
            out.set_format(&factor.name, if whole { "F8.0" } else { "F8.2" })?;
            out.set_variable_label(&factor.name, factor.label.as_deref())?;
            let labels = factor.values.iter().zip(&factor.labels);
            let labels = labels.filter_map(|(v, l)| Some((Value::Number(*v), l.clone()?)));
            out.set_value_labels(&factor.name, labels.collect())?;
        }
        out.set_numeric_values("STATUS_", profiles.iter().map(|p| Some(p.1)).collect())?;
        out.set_numeric_values("CARD_", (1..=n).map(|i| Some(i as f64)).collect())?;
        out.set_format("STATUS_", "F1.0")?;
        out.set_format("CARD_", "F8.0")?;
        let status_labels = [0.0, 1.0, 2.0].map(|s| (Value::Number(s), status_name(s).to_string()));
        out.set_value_labels("STATUS_", status_labels.to_vec())?;
        out.set_variable_label("STATUS_", Some("Status of the profile"))?;
        out.set_variable_label("CARD_", Some("Profile number"))?;

        let mut simulations = 0;
        if keep_existing && let Some(mut existing) = active {
            let count = existing.case_count();
            let read = |data: &Dataset, name: &str| -> Vec<Option<f64>> {
                match data.variable(name).ok().and_then(|v| v.numeric().ok()) {
                    Some(values) => values.iter().collect(),
                    None => vec![None; count],
                }
            };
            let status: Vec<f64> = read(&existing, "STATUS_")
                .into_iter()
                .map(|s| s.unwrap_or(2.0))
                .collect();
            let card = read(&existing, "CARD_");
            let card: Vec<Option<f64>> = card
                .into_iter()
                .enumerate()
                .map(|(i, c)| c.or(Some((n + i + 1) as f64)))
                .collect();
            let old: Vec<String> = existing
                .variable_names()
                .filter(|v| is_plan_variable(v))
                .map(str::to_string)
                .collect();
            existing.delete_variables(&old.iter().map(String::as_str).collect::<Vec<_>>())?;
            existing.set_numeric_values("STATUS_", status.iter().map(|&s| Some(s)).collect())?;
            existing.set_numeric_values("CARD_", card)?;
            // a profile of the plan entered again as experimental is kept once
            let columns: Vec<Vec<Option<f64>>> =
                factors.iter().map(|f| read(&existing, &f.name)).collect();
            let row_of = |i: usize| -> Option<Vec<usize>> {
                factors
                    .iter()
                    .zip(&columns)
                    .map(|(f, col)| f.values.iter().position(|&v| Some(v) == col[i]))
                    .collect()
            };
            let mut keep = vec![true; count];
            let mut repeated = 0;
            for (i, kept) in keep.iter_mut().enumerate() {
                let in_plan = row_of(i).is_some_and(|r| distinct.contains(&r));
                if in_plan && status[i] == 0.0 {
                    *kept = false;
                } else if in_plan && status[i] == 2.0 {
                    repeated += 1;
                }
            }
            if repeated > 0 {
                warnings.push(format!(
                    "ORTHOPLAN: {repeated} simulation profiles are also in the plan"
                ));
            }
            existing.select_cases(&keep)?;
            simulations = status
                .iter()
                .zip(&keep)
                .filter(|(s, k)| **k && **s == 2.0)
                .count();
            out = Dataset::from_parts(&out, &[existing])?;
        }
        let summary = Table {
            title: "Orthogonal Plan".into(),
            columns: vec!["Profiles".into(), "N".into()],
            rows: [
                ("Design", runs.len()),
                ("Holdout", c.holdout),
                ("Simulation", simulations),
            ]
            .into_iter()
            .filter(|(what, count)| *count > 0 || *what == "Design")
            .map(|(what, count)| vec![text(what), number(Some(count as f64))])
            .collect(),
            row_headers: 1,
            ..Table::default()
        };
        match &c.outfile {
            Some(file) => self.write_data_file(file, out)?,
            None => {
                if !keep_existing {
                    self.park_active();
                }
                self.temporary = None;
                self.dataset = Some(out);
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        Ok(vec![summary])
    }

    /// PLANCARDS: the listing runs per split group, the cards once over all profiles.
    pub(crate) fn plancards(&mut self, c: &Plancards) -> Result<Vec<Table>, Error> {
        // OUTFILE turns BOTH into CARD and is ignored with LIST
        let list = match c.format {
            CardFormat::List => true,
            CardFormat::Both => c.outfile.is_none(),
            CardFormat::Card => false,
        };
        if list {
            let command = statscraft_syntax::Command::Plancards(Box::new(c.clone()));
            let mut tables = self.procedure(&command)?;
            if c.format == CardFormat::Both {
                tables.extend(self.plan_cards(c)?);
            }
            return Ok(tables);
        }
        self.begin_pass()?;
        let before = self.dataset.clone();
        let result = self.plan_cards(c);
        self.end_pass(before.as_ref());
        result
    }

    /// The factors and the profiles of the active dataset, as shown.
    fn plan_profiles(&self, factors: &Option<Vec<VarItem>>) -> Result<Profiles, Error> {
        let data = self.active()?;
        let names = match factors {
            Some(items) => self.expand(items)?,
            None => plan_factors(data),
        };
        let shown = self.shown();
        let n = data.case_count();
        let column = |name: &str| -> Vec<f64> {
            match data.variable(name).ok().and_then(|v| v.numeric().ok()) {
                Some(values) => values.iter().map(|v| v.unwrap_or(f64::NAN)).collect(),
                None => vec![f64::NAN; n],
            }
        };
        let status: Vec<f64> = column("STATUS_")
            .into_iter()
            .map(|s| if s.is_nan() { 0.0 } else { s })
            .collect();
        let card: Vec<f64> = column("CARD_")
            .into_iter()
            .enumerate()
            .map(|(i, c)| if c.is_nan() { (i + 1) as f64 } else { c })
            .collect();
        let mut texts = vec![Vec::new(); n];
        let mut titles = Vec::new();
        for name in &names {
            let v = data.variable(name)?;
            titles.push(shown.variable(&v));
            for (case, row) in texts.iter_mut().enumerate() {
                row.push(match v.values() {
                    statscraft_data::Values::Numeric(values) => match values.get(case) {
                        Some(x) => shown.number(&v, x),
                        None => ".".into(),
                    },
                    statscraft_data::Values::String(values) => values.get(case).to_string(),
                });
            }
        }
        Ok((titles, status, card, texts))
    }

    /// The listing of one split group.
    pub(crate) fn plan_list(&mut self, c: &Plancards) -> Result<Vec<Table>, Error> {
        let (titles, status, card, texts) = self.plan_profiles(&c.factors)?;
        let mut columns = vec!["Card ID".to_string()];
        columns.extend(titles);
        let title = match c.titles.iter().any(|t| !t.trim().is_empty()) {
            true => c
                .titles
                .iter()
                .map(|t| t.trim())
                .collect::<Vec<_>>()
                .join(" "),
            false => "Plan Cards".into(),
        };
        let caption = (!c.footers.is_empty()).then(|| c.footers.join("\n"));
        let table = |rows: Vec<usize>, title: String, caption: Option<String>| Table {
            title,
            columns: columns.clone(),
            rows: rows
                .into_iter()
                .map(|i| {
                    let id = match status[i] == 1.0 {
                        true => format!("{} (Holdout)", card[i]),
                        false => card[i].to_string(),
                    };
                    let mut row = vec![text(id)];
                    row.extend(texts[i].iter().map(|t| text(t.clone())));
                    row
                })
                .collect(),
            row_headers: 1,
            caption,
            ..Table::default()
        };
        let rated: Vec<usize> = (0..status.len()).filter(|&i| status[i] != 2.0).collect();
        let simulated: Vec<usize> = (0..status.len()).filter(|&i| status[i] == 2.0).collect();
        let mut tables = vec![table(rated, title.trim().to_string(), caption)];
        if !simulated.is_empty() {
            tables.push(table(simulated, "Simulation Profiles".into(), None));
        }
        Ok(tables)
    }

    /// One card per experimental or holdout profile, as tables or into the text file.
    fn plan_cards(&mut self, c: &Plancards) -> Result<Vec<Table>, Error> {
        let (titles, status, _, texts) = self.plan_profiles(&c.factors)?;
        let rated: Vec<usize> = (0..status.len()).filter(|&i| status[i] != 2.0).collect();
        let lines = |list: &[String], number: usize| -> Vec<String> {
            list.iter()
                .map(|l| l.replace(")CARD", &number.to_string()))
                .collect()
        };
        if let Some(file) = &c.outfile {
            let mut out = String::new();
            for (k, &i) in rated.iter().enumerate() {
                for line in lines(&c.titles, k + 1) {
                    out.push_str(&format!("{line}\n"));
                }
                out.push('\n');
                for (title, value) in titles.iter().zip(&texts[i]) {
                    out.push_str(&format!("{title}: {value}\n"));
                }
                out.push('\n');
                for line in lines(&c.footers, k + 1) {
                    out.push_str(&format!("{line}\n"));
                }
                out.push_str("\n\n");
            }
            if !self.dry_run {
                let path = self.path(file);
                std::fs::write(&path, out)
                    .map_err(|e| Error(format!("PLANCARDS: {}: {e}", path.display())))?;
            }
            return Ok(Vec::new());
        }
        Ok(rated
            .iter()
            .enumerate()
            .map(|(k, &i)| {
                let title = lines(&c.titles, k + 1);
                let title = title.iter().map(|t| t.trim()).collect::<Vec<_>>().join(" ");
                let footer = lines(&c.footers, k + 1);
                Table {
                    title: match title.trim().is_empty() {
                        true => format!("Profile Number {}", k + 1),
                        false => title.trim().to_string(),
                    },
                    columns: vec!["Factor".into(), "Level".into()],
                    rows: titles
                        .iter()
                        .zip(&texts[i])
                        .map(|(t, v)| vec![text(t.clone()), text(v.clone())])
                        .collect(),
                    row_headers: 1,
                    caption: (!footer.is_empty()).then(|| footer.join("\n")),
                    ..Table::default()
                }
            })
            .collect())
    }
}

/// Whether every pair of factors of a plan has proportional cell counts.
#[cfg(test)]
pub(crate) fn orthogonal(runs: &[Vec<usize>], levels: &[usize]) -> bool {
    let n = runs.len() as f64;
    let count = |f: usize, l: usize| runs.iter().filter(|r| r[f] == l).count() as f64;
    (0..levels.len()).all(|a| {
        (0..a).all(|b| {
            (0..levels[a]).all(|i| {
                (0..levels[b]).all(|j| {
                    let both = runs.iter().filter(|r| r[a] == i && r[b] == j).count() as f64;
                    (both * n - count(a, i) * count(b, j)).abs() < 1e-9
                })
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw_from(seed: u64) -> impl FnMut() -> f64 {
        let mut random = crate::random::Random::seeded(seed);
        move || random.uniform()
    }

    #[test]
    fn plans_are_orthogonal_and_small() {
        // (levels, minimum, expected runs)
        let cases: &[(&[usize], usize, usize)] = &[
            (&[2, 2, 2], 1, 4),
            (&[3, 3, 3, 3], 1, 9),
            (&[3, 3, 2], 1, 9),
            (&[2, 3], 1, 6),
            (&[4, 2, 2, 2, 2], 1, 8),
            (&[3, 2, 2, 2, 2], 1, 8),
            (&[4, 4, 4, 4, 4], 1, 16),
            (&[3, 3, 3, 3, 3, 3], 1, 25),
            (&[2; 7], 1, 8),
            (&[2; 8], 1, 16),
            (&[5, 5, 5], 1, 25),
            (&[9, 3, 3, 3], 1, 27),
            (&[3, 3, 3], 12, 16),
            (&[2, 2], 3, 4),
            (&[6, 2, 2], 1, 16),
        ];
        for (seed, &(levels, minimum, size)) in cases.iter().enumerate() {
            let (runs, short) = plan(levels, minimum, &mut draw_from(seed as u64 + 7)).unwrap();
            assert_eq!(runs.len(), size, "{levels:?}");
            assert!(!short);
            assert!(orthogonal(&runs, levels), "{levels:?}");
            for (f, &l) in levels.iter().enumerate() {
                assert!((0..l).all(|v| runs.iter().any(|r| r[f] == v)), "{levels:?}");
            }
        }
        // more than any plan holds: the largest that fits
        let (runs, short) = plan(&[2, 2, 2, 2, 2, 2, 2], 100, &mut draw_from(3)).unwrap();
        assert!(short && runs.len() == 81 && orthogonal(&runs, &[2; 7]));
        // nine levels in ten factors: the 81-run plan
        let (runs, _) = plan(&[9; 10], 1, &mut draw_from(5)).unwrap();
        assert!(runs.len() == 81 && orthogonal(&runs, &[9; 10]));
        assert!(plan(&[2; 81], 1, &mut draw_from(5)).is_none());
    }
}
