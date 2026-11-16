//! What the Multiple Correspondence and Categorical Principal Components dialogs share: the
//! settings of each analysis variable (weight, recoding, missing values, role), supplementary
//! cases, the model and its criteria, the lists of the tables, the plot options, and the saved
//! variables and written data. Each dialog adds its own tables, plots and options.

use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::parse;
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};

/// How a variable is recoded before the analysis; the first leaves the command's own coding.
pub const SCALING_DISCRETIZE: [(&str, &str); 5] = [
    ("", "As coded"),
    ("GROUPING", "Groups"),
    ("EQINTV", "Equal intervals"),
    ("RANKING", "Ranks"),
    ("MULTIPLYING", "Scaled z-scores"),
];

/// How the missing values of a variable are handled; the first is the default.
pub const SCALING_MISSING: [(&str, &str); 6] = [
    ("PASSIVE MODEIMPU", "Passive"),
    ("PASSIVE EXTRACAT", "Passive, own category in correlations"),
    ("ACTIVE MODEIMPU", "Fill in the mode"),
    ("ACTIVE EXTRACAT", "Fill in an extra category"),
    ("ACTIVE RANDIMPU", "Fill in a random category"),
    ("LISTWISE", "Leave the case out"),
];

pub const LISTWISE: usize = 5;

pub const SCALING_NORMALIZATIONS: [(&str, &str); 5] = [
    ("VPRINCIPAL", "Variable principal"),
    ("OPRINCIPAL", "Object principal"),
    ("SYMMETRICAL", "Symmetrical"),
    ("INDEPENDENT", "Independent"),
    ("", "Custom value"),
];

pub const INDEPENDENT: usize = 3;

/// The settings of one analysis variable; `level` holds what only one dialog asks (nothing for
/// Multiple Correspondence, the scaling level for Categorical Principal Components).
#[derive(Clone, PartialEq, Debug)]
pub struct ScalingVariable<L = ()> {
    pub name: String,
    pub weight: String,
    /// Index into `SCALING_DISCRETIZE`.
    pub discretize: usize,
    /// The number of groups.
    pub groups: String,
    /// Groups of about equal counts instead of the shape of a normal curve.
    pub uniform: bool,
    /// The width of an equal interval.
    pub width: String,
    /// Index into `SCALING_MISSING`.
    pub missing: usize,
    pub supplementary: bool,
    pub level: L,
}

impl<L: Default> ScalingVariable<L> {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            weight: "1".into(),
            discretize: 0,
            groups: "7".into(),
            uniform: false,
            width: String::new(),
            missing: 0,
            supplementary: false,
            level: L::default(),
        }
    }
}

impl<L> ScalingVariable<L> {
    /// `WEIGHT=w` on ANALYSIS; None for weight 1.
    pub fn weight_spec(&self) -> Result<Option<String>, String> {
        // the command ignores the weight of a supplementary variable
        if self.supplementary {
            return Ok(None);
        }
        let what = format!("the weight of {}", self.name);
        match optional(&self.weight, &what)? {
            None => Ok(None),
            Some(w) if w <= 0.0 => Err(format!("{what} must be above 0")),
            Some(1.0) => Ok(None),
            Some(w) => Ok(Some(format!("WEIGHT={}", number(w)))),
        }
    }

    /// The recoding in parentheses; None for the command's own coding.
    fn discretization(&self) -> Result<Option<String>, String> {
        let name = &self.name;
        Ok(match self.discretize {
            0 => None,
            1 => {
                let n = whole(&self.groups, 1, &format!("the number of groups of {name}"))?
                    .ok_or_else(|| format!("give the number of groups of {name}"))?;
                if n >= 1_000_000_000 {
                    return Err(format!("the number of groups of {name} is too large"));
                }
                Some(match self.uniform {
                    true => format!("GROUPING NCAT={n} DISTR=UNIFORM"),
                    false => format!("GROUPING NCAT={n}"),
                })
            }
            2 => {
                let width = parse(&self.width, &format!("the interval width of {name}"))?;
                if width <= 0.0 {
                    return Err(format!("the interval width of {name} must be above 0"));
                }
                Some(format!("GROUPING EQINTV={}", number(width)))
            }
            i => Some(SCALING_DISCRETIZE[i].0.to_string()),
        })
    }
}

/// The settings of an analysis variable, the defaults when it has none yet.
pub fn setting<L: Clone + Default>(
    settings: &[ScalingVariable<L>],
    name: &str,
) -> ScalingVariable<L> {
    settings
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .unwrap_or_else(|| ScalingVariable::new(name))
}

/// The settings of every analysis variable, once there are two and one of them is active.
pub fn analysis_settings<L: Clone + Default>(
    variables: &[String],
    settings: &[ScalingVariable<L>],
) -> Result<Vec<ScalingVariable<L>>, String> {
    if variables.len() < 2 {
        return Err("add at least two analysis variables".into());
    }
    let out: Vec<ScalingVariable<L>> = variables.iter().map(|n| setting(settings, n)).collect();
    if out.iter().all(|s| s.supplementary) {
        return Err("at least one analysis variable must not be supplementary".into());
    }
    Ok(out)
}

/// Every variable on VARIABLES: the analysis ones, then the labelling ones not among them.
pub fn all_variables(variables: &[String], labels: &[String]) -> Vec<String> {
    let mut all = variables.to_vec();
    for name in labels {
        if !all.iter().any(|a| a.eq_ignore_ascii_case(name)) {
            all.push(name.clone());
        }
    }
    all
}

/// The names of `chosen` still among `offered`, in the order offered.
pub fn still(offered: &[String], chosen: &[String]) -> Vec<String> {
    offered
        .iter()
        .filter(|n| chosen.contains(n))
        .cloned()
        .collect()
}

/// A whole number of at least `least` and below the syntax's limit, or None for an empty field.
pub fn bounded(text: &str, least: u32, what: &str) -> Result<Option<u32>, String> {
    match whole(text, least, what)? {
        Some(n) if n >= 1_000_000_000 => Err(format!("{what} must be below 1000000000")),
        n => Ok(n),
    }
}

/// Case numbers and ranges such as `3 8 10-12`, as the syntax writes them, checked against the
/// number of cases; also the number of cases they name.
pub fn case_numbers(text: &str, cases: usize) -> Result<(String, usize), String> {
    let wrong = || "give supplementary cases as whole numbers from 1, ranges as 4-9".to_string();
    let spaced = text.replace(',', " ").replace('-', " - ");
    let mut tokens = spaced.split_whitespace().peekable();
    let mut named = vec![false; cases];
    let mut out = Vec::new();
    let case = |t: Option<&str>| -> Result<usize, String> {
        t.and_then(|t| t.parse::<usize>().ok())
            .filter(|n| *n >= 1)
            .ok_or_else(wrong)
    };
    while let Some(t) = tokens.next() {
        let first = case(Some(t))?;
        let last = match tokens.next_if(|t| *t == "-" || t.eq_ignore_ascii_case("TO")) {
            Some(_) => case(tokens.next())?,
            None => first,
        };
        if last < first {
            return Err(format!(
                "the supplementary range {first}-{last} runs backward"
            ));
        }
        if last > cases {
            return Err(format!(
                "supplementary case {last} is past the last case ({cases})"
            ));
        }
        named[first - 1..last].iter_mut().for_each(|n| *n = true);
        out.push(match first == last {
            true => first.to_string(),
            false => format!("{first} TO {last}"),
        });
    }
    let count = named.iter().filter(|n| **n).count();
    Ok((out.join(" "), count))
}

/// The ANALYSIS list from each variable and what its parentheses say (empty for the defaults):
/// runs of variables with the same specification, each run but a last default one closed by
/// it, since parentheses cover every variable since the previous ones. A default run before
/// others says WEIGHT=1.
pub fn analysis_list(entries: &[(&str, String)]) -> String {
    let mut runs: Vec<(Vec<&str>, &str)> = Vec::new();
    for (name, spec) in entries {
        match runs.last_mut() {
            Some((names, s)) if *s == spec.as_str() => names.push(name),
            _ => runs.push((vec![name], spec.as_str())),
        }
    }
    let last = runs.len().saturating_sub(1);
    let parts: Vec<String> = runs
        .iter()
        .enumerate()
        .map(|(i, (names, spec))| match (i == last, spec.is_empty()) {
            (true, true) => names.join(" "),
            (false, true) => format!("{}(WEIGHT=1)", names.join(" ")),
            _ => format!("{}({spec})", names.join(" ")),
        })
        .collect();
    parts.join(" ")
}

/// DISCRETIZATION, MISSING and SUPPLEMENTARY for the analysis variables and the supplementary
/// cases, each when it says anything.
pub fn data_subcommands<L>(
    settings: &[ScalingVariable<L>],
    objects: &str,
    data: &Dataset,
) -> Result<String, String> {
    let mut text = String::new();
    let mut discretization = Vec::new();
    let mut missing = Vec::new();
    for s in settings {
        if let Some(how) = s.discretization()? {
            discretization.push(format!("{}({how})", s.name));
        }
        if s.supplementary && s.missing == LISTWISE {
            return Err(format!(
                "supplementary variable {} cannot leave cases out for missing values",
                s.name
            ));
        }
        if s.missing > 0 {
            missing.push(format!("{}({})", s.name, SCALING_MISSING[s.missing].0));
        }
    }
    if !discretization.is_empty() {
        text.push_str(&format!("\n  /DISCRETIZATION={}", discretization.join(" ")));
    }
    if !missing.is_empty() {
        text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
    }
    let mut supplementary = Vec::new();
    if !objects.trim().is_empty() {
        let cases = data.case_count();
        let (list, count) = case_numbers(objects, cases)?;
        // the command needs three active cases
        if cases.saturating_sub(count) < 3 {
            return Err("at least three cases must stay out of the supplementary ones".into());
        }
        supplementary.push(format!("OBJECT({list})"));
    }
    let sup_vars: Vec<&str> = settings
        .iter()
        .filter(|s| s.supplementary)
        .map(|s| s.name.as_str())
        .collect();
    if !sup_vars.is_empty() {
        supplementary.push(format!("VARIABLE({})", sup_vars.join(" ")));
    }
    if !supplementary.is_empty() {
        text.push_str(&format!("\n  /SUPPLEMENTARY={}", supplementary.join(" ")));
    }
    Ok(text)
}

/// DIMENSION, NORMALIZATION, MAXITER and CRITITER when they differ from the defaults, and the
/// number of dimensions.
pub fn model_subcommands(
    dimensions: &str,
    normalization: usize,
    q: &str,
    max_iterations: &str,
    convergence: &str,
) -> Result<(String, u32), String> {
    let mut text = String::new();
    // the command lowers more dimensions than the data allow, with a warning
    let dims = bounded(dimensions, 1, "the number of dimensions")?.unwrap_or(2);
    if dims != 2 {
        text.push_str(&format!("\n  /DIMENSION={dims}"));
    }
    match normalization {
        0 => {}
        4 => {
            let q = parse(q, "the normalization value")?;
            if !(-1.0..=1.0).contains(&q) {
                return Err("the normalization value lies from -1 to 1".into());
            }
            if q != -1.0 {
                text.push_str(&format!("\n  /NORMALIZATION={}", number(q)));
            }
        }
        i => text.push_str(&format!(
            "\n  /NORMALIZATION={}",
            SCALING_NORMALIZATIONS[i].0
        )),
    }
    let iterations = bounded(max_iterations, 1, "the most iterations")?.unwrap_or(100);
    if iterations != 100 {
        text.push_str(&format!("\n  /MAXITER={iterations}"));
    }
    if let Some(c) = optional(convergence, "the convergence criterion")? {
        if c <= 0.0 {
            return Err("the convergence criterion must be above 0".into());
        }
        if c != 0.00001 {
            text.push_str(&format!("\n  /CRITITER={}", number(c)));
        }
    }
    Ok((text, dims))
}

/// The variable lists of the descriptives, quantifications and object score tables, and the
/// variable that labels the object scores; empty lists mean the command's own choice.
pub struct PrintLists<'a> {
    pub descrip: &'a [String],
    pub quant: &'a [String],
    pub shown: &'a [String],
    pub label: &'a str,
}

/// /PRINT when the tables, their lists or `changed` (a keyword written another way, counted
/// while its table is checked) differ from the defaults.
pub fn print_syntax(
    table: &[(&str, &str)],
    flags: &[bool],
    defaults: &[bool],
    (all, analysis): (&[String], &[String]),
    lists: PrintLists,
    changed: Option<(&str, &str)>,
) -> String {
    let on = |k: &str| table.iter().zip(flags).any(|((w, _), f)| *w == k && *f);
    let descrip = still(all, lists.descrip);
    let quant = still(analysis, lists.quant);
    let shown = still(all, lists.shown);
    let label = all
        .iter()
        .find(|n| **n == lists.label)
        .filter(|_| on("OBJECT"));
    // a list counts only while its table is checked
    let listed = [("DESCRIP", &descrip), ("QUANT", &quant), ("OBJECT", &shown)]
        .iter()
        .any(|(k, l)| on(k) && !l.is_empty());
    let changed = changed.filter(|(k, _)| on(k));
    if flags == defaults && !listed && label.is_none() && changed.is_none() {
        return String::new();
    }
    let list = |l: &[String]| match l.is_empty() {
        true => String::new(),
        false => format!("({})", l.join(" ")),
    };
    let words: Vec<String> = table
        .iter()
        .zip(flags)
        .filter(|(_, on)| **on)
        .map(|((k, _), _)| match *k {
            "DESCRIP" => format!("DESCRIP{}", list(&descrip)),
            "QUANT" => format!("QUANT{}", list(&quant)),
            "OBJECT" => match label {
                Some(l) => format!(
                    "OBJECT(({l}){})",
                    shown.iter().map(|n| format!(" {n}")).collect::<String>()
                ),
                None => format!("OBJECT{}", list(&shown)),
            },
            k => match changed {
                Some((c, word)) if c == k => word.to_string(),
                _ => k.to_string(),
            },
        })
        .collect();
    match words.is_empty() {
        true => "\n  /PRINT=NONE".into(),
        false => format!("\n  /PRINT={}", words.join(" ")),
    }
}

/// `(varlist)`, or nothing for an empty list.
pub fn paren(list: &[String]) -> String {
    match list.is_empty() {
        true => String::new(),
        false => format!("({})", list.join(" ")),
    }
}

/// The label length after a plot keyword, `(n)`, when a plot that takes one is on; empty for
/// labels in full.
pub fn label_length(text: &str, labeled: bool) -> Result<String, String> {
    let length = match labeled {
        true => whole(text, 0, "the longest label")?,
        false => None,
    };
    // the command lowers a longer length to its own limit
    Ok(length.map_or(String::new(), |n| format!("({})", n.min(255))))
}

/// The dimensions a transformation or residual plot shows per variable, `(n)` above 1, checked
/// while such a plot is on.
pub fn per_variable(text: &str, on: bool, dims: u32) -> Result<String, String> {
    let n = match on {
        true => bounded(text, 1, "the plotted dimensions")?,
        false => None,
    };
    Ok(match n {
        Some(n) if n > dims => {
            return Err(format!(
                "the plotted dimensions per variable must be at most {dims}"
            ));
        }
        Some(n) if n > 1 => format!("({n})"),
        _ => String::new(),
    })
}

/// NDIM(first,last), checked against the number of dimensions.
pub fn ndim(first: &str, last: &str, dims: u32) -> Result<String, String> {
    if dims < 2 {
        return Err("plotting a range of dimensions needs at least two".into());
    }
    let first = whole(first, 1, "the first plotted dimension")?.unwrap_or(1);
    if first >= dims {
        return Err(format!(
            "the first plotted dimension must be below the number of dimensions ({dims})"
        ));
    }
    let last = whole(last, 1, "the last plotted dimension")?.unwrap_or(dims);
    if last <= first || last > dims {
        return Err(format!(
            "the last plotted dimension must be above the first and at most {dims}"
        ));
    }
    Ok(format!("NDIM({first},{last})"))
}

/// /SAVE of the checked entries of `table` (keyword, label, longest root), each with its root;
/// an empty root is the command's own.
pub fn save_syntax(
    table: &[(&str, &str, usize)],
    flags: &[bool],
    roots: &[String],
) -> Result<String, String> {
    let mut saves = Vec::new();
    for (i, (keyword, label, longest)) in table.iter().enumerate() {
        if !flags[i] {
            continue;
        }
        match roots[i].trim() {
            "" => saves.push(keyword.to_string()),
            root if !is_valid_name(root) || root.starts_with('#') => {
                return Err(format!("{root} cannot start variable names"));
            }
            root if root.chars().count() > *longest => {
                return Err(format!(
                    "the root of the {} has at most {longest} characters",
                    label.to_lowercase()
                ));
            }
            root => saves.push(format!("{keyword}({root})")),
        }
    }
    Ok(match saves.is_empty() {
        true => String::new(),
        false => format!("\n  /SAVE={}", saves.join(" ")),
    })
}

/// /OUTFILE of the entries of `table` given a place: nowhere (0), a data file (1) or a new
/// dataset (2); also the DATASET DECLARE commands that go before the command.
pub fn outfile_syntax(
    table: &[(&str, &str)],
    choices: &[usize],
    files: &[String],
    datasets: &[String],
) -> Result<(String, String), String> {
    let mut outfiles = Vec::new();
    let mut places: Vec<String> = Vec::new();
    let mut declare = String::new();
    for (i, (keyword, label)) in table.iter().enumerate() {
        let what = label.to_lowercase();
        let Some(target) = output_target(choices[i], &files[i], &datasets[i], &what)? else {
            continue;
        };
        // the command compares the places without case, a file and a dataset alike
        let place = match choices[i] {
            1 => files[i].trim().to_lowercase(),
            _ => datasets[i].trim().to_lowercase(),
        };
        if places.contains(&place) {
            return Err(format!("the {what} need a place of their own"));
        }
        places.push(place);
        // the command writes into a dataset only when one of that name exists
        if choices[i] == 2 {
            declare.push_str(&format!("DATASET DECLARE {target}.\n"));
        }
        outfiles.push(format!("{keyword}({target})"));
    }
    let text = match outfiles.is_empty() {
        true => String::new(),
        false => format!("\n  /OUTFILE={}", outfiles.join(" ")),
    };
    Ok((declare, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_runs_close_with_their_specification() {
        let list = |entries: &[(&str, &str)]| {
            let owned: Vec<(&str, String)> =
                entries.iter().map(|(n, s)| (*n, s.to_string())).collect();
            analysis_list(&owned)
        };
        assert_eq!(list(&[("a", ""), ("b", "")]), "a b");
        assert_eq!(
            list(&[("a", ""), ("b", "LEVEL=NOMI"), ("c", "LEVEL=NOMI")]),
            "a(WEIGHT=1) b c(LEVEL=NOMI)"
        );
        assert_eq!(
            list(&[("a", "WEIGHT=2"), ("b", ""), ("c", "WEIGHT=2")]),
            "a(WEIGHT=2) b(WEIGHT=1) c(WEIGHT=2)"
        );
    }
}
