//! The Multiple Correspondence dialog: MULTIPLE CORRESPONDENCE of nominal variables, each with
//! its weight, recoding, missing value handling and role. Options left as they are add nothing to
//! the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::parse;
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};

/// How a variable is recoded before the analysis; the first leaves the command's own coding.
pub const MCA_DISCRETIZE: [(&str, &str); 5] = [
    ("", "As coded"),
    ("GROUPING", "Groups"),
    ("EQINTV", "Equal intervals"),
    ("RANKING", "Ranks"),
    ("MULTIPLYING", "Scaled z-scores"),
];

/// How the missing values of a variable are handled; the first is the default.
pub const MCA_MISSING: [(&str, &str); 6] = [
    ("PASSIVE MODEIMPU", "Passive"),
    ("PASSIVE EXTRACAT", "Passive, own category in correlations"),
    ("ACTIVE MODEIMPU", "Fill in the mode"),
    ("ACTIVE EXTRACAT", "Fill in an extra category"),
    ("ACTIVE RANDIMPU", "Fill in a random category"),
    ("LISTWISE", "Leave the case out"),
];

const LISTWISE: usize = 5;

pub const MCA_NORMALIZATIONS: [(&str, &str); 5] = [
    ("VPRINCIPAL", "Variable principal"),
    ("OPRINCIPAL", "Object principal"),
    ("SYMMETRICAL", "Symmetrical"),
    ("INDEPENDENT", "Independent"),
    ("", "Custom value"),
];

const INDEPENDENT: usize = 3;

/// The tables of /PRINT; DESCRIP, OBJECT and QUANT take variable lists.
pub const MCA_PRINT: [(&str, &str); 7] = [
    ("CORR", "Correlations of quantified variables"),
    ("DESCRIP", "Descriptives"),
    ("DISCRIM", "Discrimination measures"),
    ("HISTORY", "Iteration history"),
    ("OBJECT", "Object scores"),
    ("OCORR", "Correlations of original variables"),
    ("QUANT", "Category quantifications"),
];

const MCA_DEFAULT_PRINT: [bool; 7] = [true, true, true, false, false, false, false];

/// The charts of /PLOT; every one but the biplot takes one list of variables.
pub const MCA_PLOTS: [(&str, &str); 7] = [
    ("OBJECT", "Object points"),
    ("CATEGORY", "Category points"),
    ("JOINTCAT", "Joint category points"),
    ("DISCRIM", "Discrimination measures"),
    ("BIPLOT", "Objects and centroids"),
    ("TRANS", "Transformations"),
    ("RESID", "Residuals"),
];

const MCA_DEFAULT_PLOTS: [bool; 7] = [true, false, false, true, false, false, false];
pub const BIPLOT: usize = 4;

/// The variables /SAVE adds, with the longest root the command keeps.
pub const MCA_SAVE: [(&str, &str, usize); 2] = [
    ("TRDATA", "Quantified variables", 3),
    ("OBJECT", "Object scores", 5),
];

/// The data /OUTFILE writes.
pub const MCA_OUTFILE: [(&str, &str); 3] = [
    ("DISCRDATA", "Recoded data"),
    ("TRDATA", "Quantified variables"),
    ("OBJECT", "Object scores"),
];

/// The settings of one analysis variable.
#[derive(Clone, PartialEq, Debug)]
pub struct McaVariable {
    pub name: String,
    pub weight: String,
    /// Index into `MCA_DISCRETIZE`.
    pub discretize: usize,
    /// The number of groups.
    pub groups: String,
    /// Groups of about equal counts instead of the shape of a normal curve.
    pub uniform: bool,
    /// The width of an equal interval.
    pub width: String,
    /// Index into `MCA_MISSING`.
    pub missing: usize,
    pub supplementary: bool,
}

impl McaVariable {
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
            i => Some(MCA_DISCRETIZE[i].0.to_string()),
        })
    }
}

/// MULTIPLE CORRESPONDENCE: homogeneity analysis of nominal variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Mca {
    pub variables: Vec<String>,
    /// The settings of the analysis variables, found by name; a variable without one has the
    /// defaults.
    pub settings: Vec<McaVariable>,
    /// Variables outside the analysis that label objects or are shown beside them.
    pub labels: Vec<String>,
    /// The supplementary cases by number, with ranges such as 4-9.
    pub objects: String,
    pub dimensions: String,
    /// Index into `MCA_NORMALIZATIONS`.
    pub normalization: usize,
    /// The custom normalization value.
    pub q: String,
    pub max_iterations: String,
    pub convergence: String,
    /// One flag per entry of `MCA_PRINT`.
    pub print: [bool; 7],
    /// The variables of the descriptives, quantifications and object score tables; none means
    /// the command's own choice.
    pub descrip: Vec<String>,
    pub quant: Vec<String>,
    pub object_shown: Vec<String>,
    /// Labels the object scores; empty for case numbers.
    pub object_label: String,
    /// One flag per entry of `MCA_PLOTS`.
    pub plots: [bool; 7],
    /// The variables of each plot (for the biplot, those with centroids); none means all, or
    /// for object points the case numbers.
    pub plot_variables: [Vec<String>; 7],
    /// Labels the objects of the biplot; none for case numbers.
    pub biplot_labels: Vec<String>,
    /// The dimensions plotted for each variable in transformation and residual plots.
    pub plot_dimensions: String,
    /// The longest label in the plots; empty for labels in full.
    pub label_length: String,
    pub ndim: bool,
    pub ndim_first: String,
    pub ndim_last: String,
    /// One flag per entry of `MCA_SAVE`, with its root; an empty root is the command's own.
    pub save: [bool; 2],
    pub save_roots: [String; 2],
    /// Per entry of `MCA_OUTFILE`: nowhere (0), a data file (1) or a new dataset (2).
    pub outfile: [usize; 3],
    pub outfile_files: [String; 3],
    pub outfile_datasets: [String; 3],
}

impl Default for Mca {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            settings: Vec::new(),
            labels: Vec::new(),
            objects: String::new(),
            dimensions: "2".into(),
            normalization: 0,
            q: "0".into(),
            max_iterations: "100".into(),
            convergence: "0.00001".into(),
            print: MCA_DEFAULT_PRINT,
            descrip: Vec::new(),
            quant: Vec::new(),
            object_shown: Vec::new(),
            object_label: String::new(),
            plots: MCA_DEFAULT_PLOTS,
            plot_variables: Default::default(),
            biplot_labels: Vec::new(),
            plot_dimensions: "1".into(),
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: "2".into(),
            save: [false; 2],
            save_roots: Default::default(),
            outfile: [0; 3],
            outfile_files: Default::default(),
            outfile_datasets: Default::default(),
        }
    }
}

/// The names of `chosen` still among `offered`, in the order offered.
fn still(offered: &[String], chosen: &[String]) -> Vec<String> {
    offered
        .iter()
        .filter(|n| chosen.contains(n))
        .cloned()
        .collect()
}

/// Case numbers and ranges such as `3 8 10-12`, as the syntax writes them, checked against the
/// number of cases; also the number of cases they name.
fn case_numbers(text: &str, cases: usize) -> Result<(String, usize), String> {
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

impl Mca {
    /// The settings of an analysis variable, the defaults when it has none yet.
    pub fn setting(&self, name: &str) -> McaVariable {
        self.settings
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .unwrap_or_else(|| McaVariable::new(name))
    }

    /// Every variable on VARIABLES: the analysis ones, then the labelling ones not among them.
    pub fn all_variables(&self) -> Vec<String> {
        let mut all = self.variables.clone();
        for name in &self.labels {
            if !all.iter().any(|a| a.eq_ignore_ascii_case(name)) {
                all.push(name.clone());
            }
        }
        all
    }

    /// The biplot is not drawn with independent normalization, so the dialog offers it only
    /// with the others.
    pub fn biplot_allowed(&self) -> bool {
        self.normalization != INDEPENDENT
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two analysis variables".into());
        }
        let settings: Vec<McaVariable> = self.variables.iter().map(|n| self.setting(n)).collect();
        if settings.iter().all(|s| s.supplementary) {
            return Err("at least one analysis variable must not be supplementary".into());
        }
        let all = self.all_variables();
        let mut text = format!(
            "MULTIPLE CORRESPONDENCE VARIABLES={}\n  /ANALYSIS={}",
            all.join(" "),
            analysis_list(&settings)?
        );
        let mut discretization = Vec::new();
        let mut missing = Vec::new();
        for s in &settings {
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
                missing.push(format!("{}({})", s.name, MCA_MISSING[s.missing].0));
            }
        }
        if !discretization.is_empty() {
            text.push_str(&format!("\n  /DISCRETIZATION={}", discretization.join(" ")));
        }
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
        }
        let mut supplementary = Vec::new();
        if !self.objects.trim().is_empty() {
            let cases = data.case_count();
            let (list, count) = case_numbers(&self.objects, cases)?;
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
        // the command lowers more dimensions than the data allow, with a warning
        let dims = bounded(&self.dimensions, 1, "the number of dimensions")?.unwrap_or(2);
        if dims != 2 {
            text.push_str(&format!("\n  /DIMENSION={dims}"));
        }
        match self.normalization {
            0 => {}
            4 => {
                let q = parse(&self.q, "the normalization value")?;
                if !(-1.0..=1.0).contains(&q) {
                    return Err("the normalization value lies from -1 to 1".into());
                }
                if q != -1.0 {
                    text.push_str(&format!("\n  /NORMALIZATION={}", number(q)));
                }
            }
            i => text.push_str(&format!("\n  /NORMALIZATION={}", MCA_NORMALIZATIONS[i].0)),
        }
        let iterations = bounded(&self.max_iterations, 1, "the most iterations")?.unwrap_or(100);
        if iterations != 100 {
            text.push_str(&format!("\n  /MAXITER={iterations}"));
        }
        if let Some(c) = optional(&self.convergence, "the convergence criterion")? {
            if c <= 0.0 {
                return Err("the convergence criterion must be above 0".into());
            }
            if c != 0.00001 {
                text.push_str(&format!("\n  /CRITITER={}", number(c)));
            }
        }
        text.push_str(&self.print_syntax(&all));
        text.push_str(&self.plot_syntax(&all, dims)?);
        let mut saves = Vec::new();
        for (i, (keyword, label, longest)) in MCA_SAVE.iter().enumerate() {
            if !self.save[i] {
                continue;
            }
            match self.save_roots[i].trim() {
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
        if !saves.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", saves.join(" ")));
        }
        let mut outfiles = Vec::new();
        let mut places: Vec<String> = Vec::new();
        let mut declare = String::new();
        for (i, (keyword, label)) in MCA_OUTFILE.iter().enumerate() {
            let what = label.to_lowercase();
            let Some(target) = output_target(
                self.outfile[i],
                &self.outfile_files[i],
                &self.outfile_datasets[i],
                &what,
            )?
            else {
                continue;
            };
            // the command compares the places without case, a file and a dataset alike
            let place = match self.outfile[i] {
                1 => self.outfile_files[i].trim().to_lowercase(),
                _ => self.outfile_datasets[i].trim().to_lowercase(),
            };
            if places.contains(&place) {
                return Err(format!("the {what} need a place of their own"));
            }
            places.push(place);
            // the command writes into a dataset only when one of that name exists
            if self.outfile[i] == 2 {
                declare.push_str(&format!("DATASET DECLARE {target}.\n"));
            }
            outfiles.push(format!("{keyword}({target})"));
        }
        if !outfiles.is_empty() {
            text.push_str(&format!("\n  /OUTFILE={}", outfiles.join(" ")));
        }
        text.push('.');
        Ok(declare + &text)
    }

    /// /PRINT when the tables or their lists differ from the default.
    fn print_syntax(&self, all: &[String]) -> String {
        let descrip = still(all, &self.descrip);
        let quant = still(&self.variables, &self.quant);
        let shown = still(all, &self.object_shown);
        let label = all
            .iter()
            .find(|n| **n == self.object_label)
            .filter(|_| self.print[4]);
        // a list counts only while its table is checked
        let lists = [(1, &descrip), (6, &quant), (4, &shown)]
            .iter()
            .any(|(i, l)| self.print[*i] && !l.is_empty());
        if self.print == MCA_DEFAULT_PRINT && !lists && label.is_none() {
            return String::new();
        }
        let list = |l: &[String]| match l.is_empty() {
            true => String::new(),
            false => format!("({})", l.join(" ")),
        };
        let words: Vec<String> = keywords(&MCA_PRINT, &self.print)
            .into_iter()
            .map(|k| match k {
                "DESCRIP" => format!("DESCRIP{}", list(&descrip)),
                "QUANT" => format!("QUANT{}", list(&quant)),
                "OBJECT" => match label {
                    Some(l) => format!(
                        "OBJECT(({l}){})",
                        shown.iter().map(|n| format!(" {n}")).collect::<String>()
                    ),
                    None => format!("OBJECT{}", list(&shown)),
                },
                k => k.to_string(),
            })
            .collect();
        match words.is_empty() {
            true => "\n  /PRINT=NONE".into(),
            false => format!("\n  /PRINT={}", words.join(" ")),
        }
    }

    /// /PLOT when the charts, their lists or their labels differ from the default.
    fn plot_syntax(&self, all: &[String], dims: u32) -> Result<String, String> {
        let mut plots = self.plots;
        plots[BIPLOT] &= self.biplot_allowed();
        // object points may be labeled by any variable, the other plots show analysis ones
        let chosen: Vec<Vec<String>> = (0..MCA_PLOTS.len())
            .map(|i| match i {
                0 => still(all, &self.plot_variables[i]),
                _ => still(&self.variables, &self.plot_variables[i]),
            })
            .collect();
        let biplot_labels = still(all, &self.biplot_labels);
        if !plots.contains(&true) {
            return Ok("\n  /PLOT=NONE".into());
        }
        // every plot but the transformations takes the label length
        let labeled = plots.iter().enumerate().any(|(i, on)| *on && i != 5);
        let length = match labeled {
            true => whole(&self.label_length, 0, "the longest label")?,
            false => None,
        };
        // the command lowers a longer length to its own limit
        let length = length.map_or(String::new(), |n| format!("({})", n.min(255)));
        let per_variable = match plots[5] || plots[6] {
            true => bounded(&self.plot_dimensions, 1, "the plotted dimensions")?,
            false => None,
        };
        let per_variable = match per_variable {
            Some(n) if n > dims => {
                return Err(format!(
                    "the plotted dimensions per variable must be at most {dims}"
                ));
            }
            Some(n) if n > 1 => format!("({n})"),
            _ => String::new(),
        };
        let paren = |l: &[String]| match l.is_empty() {
            true => String::new(),
            false => format!("({})", l.join(" ")),
        };
        let mut words = Vec::new();
        for (i, (keyword, _)) in MCA_PLOTS.iter().enumerate() {
            if !plots[i] {
                continue;
            }
            // a plot that needs a list gets every analysis variable when none is checked
            let every = |l: &[String]| match l.is_empty() {
                true => self.variables.clone(),
                false => l.to_vec(),
            };
            words.push(match *keyword {
                "CATEGORY" => format!("CATEGORY{}{length}", paren(&every(&chosen[i]))),
                "BIPLOT" => {
                    let centroids = match chosen[i].is_empty() {
                        true => String::new(),
                        false => format!("({})", paren(&chosen[i])),
                    };
                    format!("BIPLOT{centroids}{}{length}", paren(&biplot_labels))
                }
                "TRANS" | "RESID" => {
                    let list: Vec<String> = every(&chosen[i])
                        .iter()
                        .map(|n| format!("{n}{per_variable}"))
                        .collect();
                    match *keyword {
                        // the transformation plots take no label length
                        "TRANS" => format!("TRANS{}", paren(&list)),
                        _ => format!("RESID{}{length}", paren(&list)),
                    }
                }
                k => format!("{k}{}{length}", paren(&chosen[i])),
            });
        }
        if self.ndim {
            if dims < 2 {
                return Err("plotting a range of dimensions needs at least two".into());
            }
            let first = whole(&self.ndim_first, 1, "the first plotted dimension")?.unwrap_or(1);
            if first >= dims {
                return Err(format!(
                    "the first plotted dimension must be below the number of dimensions ({dims})"
                ));
            }
            let last = whole(&self.ndim_last, 1, "the last plotted dimension")?.unwrap_or(dims);
            if last <= first || last > dims {
                return Err(format!(
                    "the last plotted dimension must be above the first and at most {dims}"
                ));
            }
            words.push(format!("NDIM({first},{last})"));
        } else if plots == MCA_DEFAULT_PLOTS
            && length.is_empty()
            && plots.iter().zip(&chosen).all(|(on, l)| !on || l.is_empty())
        {
            return Ok(String::new());
        }
        Ok(format!("\n  /PLOT={}", words.join(" ")))
    }
}

/// A whole number of at least `least` and below the syntax's limit, or None for an empty field.
fn bounded(text: &str, least: u32, what: &str) -> Result<Option<u32>, String> {
    match whole(text, least, what)? {
        Some(n) if n >= 1_000_000_000 => Err(format!("{what} must be below 1000000000")),
        n => Ok(n),
    }
}

/// The ANALYSIS list: runs of variables with the same weight, each run but a last one of weight
/// 1 closed by its weight, since a weight covers every variable since the previous one.
fn analysis_list(settings: &[McaVariable]) -> Result<String, String> {
    let mut runs: Vec<(Vec<&str>, f64)> = Vec::new();
    for s in settings {
        // the command ignores the weight of a supplementary variable
        let weight = match s.supplementary {
            true => 1.0,
            false => {
                let what = format!("the weight of {}", s.name);
                match optional(&s.weight, &what)? {
                    None => 1.0,
                    Some(w) if w > 0.0 => w,
                    Some(_) => return Err(format!("{what} must be above 0")),
                }
            }
        };
        match runs.last_mut() {
            Some((names, w)) if *w == weight => names.push(&s.name),
            _ => runs.push((vec![s.name.as_str()], weight)),
        }
    }
    let last = runs.len() - 1;
    let parts: Vec<String> = runs
        .iter()
        .enumerate()
        .map(|(i, (names, w))| match i == last && *w == 1.0 {
            true => names.join(" "),
            false => format!("{}(WEIGHT={})", names.join(" "), number(*w)),
        })
        .collect();
    Ok(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::v;
    use statscraft_engine::session::Session;

    /// Six invented cases of three nominal answers.
    fn data() -> Dataset {
        let mut session = Session::default();
        session
            .run(
                "DATA LIST LIST /tea snack seat note (3F1 A4).
BEGIN DATA
1 2 1 aa
2 1 3 bb
3 3 2 cc
1 1 1 dd
2 3 2 ee
3 2 3 ff
END DATA.",
                None,
            )
            .unwrap();
        session.dataset().unwrap().clone()
    }

    fn base() -> Mca {
        Mca {
            variables: v(&["tea", "snack", "seat"]),
            ..Default::default()
        }
    }

    #[test]
    fn weights_cover_the_variables_since_the_previous_one() {
        let data = data();
        let mut d = base();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MULTIPLE CORRESPONDENCE VARIABLES=tea snack seat\n  /ANALYSIS=tea snack seat."
        );
        let mut snack = McaVariable::new("snack");
        snack.weight = "2".into();
        d.settings.push(snack);
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea(WEIGHT=1) snack(WEIGHT=2) seat.")
        );
        let mut seat = McaVariable::new("seat");
        seat.weight = "2".into();
        d.settings.push(seat);
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea(WEIGHT=1) snack seat(WEIGHT=2).")
        );
        // a supplementary variable keeps weight 1
        d.settings[0].supplementary = true;
        d.settings[0].weight = "0".into();
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea snack(WEIGHT=1) seat(WEIGHT=2)")
        );
    }

    #[test]
    fn supplementary_cases_take_numbers_and_ranges() {
        assert_eq!(
            case_numbers("1, 3 4-5", 9).unwrap(),
            ("1 3 4 TO 5".into(), 4)
        );
        assert_eq!(case_numbers("2 to 3 3", 9).unwrap(), ("2 TO 3 3".into(), 2));
        assert!(case_numbers("0", 9).is_err());
        assert!(case_numbers("1.5", 9).is_err());
        assert!(case_numbers("4-", 9).is_err());
        assert_eq!(
            case_numbers("5-3", 9).unwrap_err(),
            "the supplementary range 5-3 runs backward"
        );
        assert_eq!(
            case_numbers("10", 9).unwrap_err(),
            "supplementary case 10 is past the last case (9)"
        );
    }

    #[test]
    fn mca_checks_what_the_command_would_reject() {
        let data = data();
        let err = |change: &dyn Fn(&mut Mca)| {
            let mut d = base();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        let tea_err = |change: &dyn Fn(&mut McaVariable)| {
            let mut d = base();
            let mut s = McaVariable::new("tea");
            change(&mut s);
            d.settings.push(s);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.variables.truncate(1)),
            "add at least two analysis variables"
        );
        assert_eq!(
            tea_err(&|s| s.weight = "-1".into()),
            "the weight of tea must be above 0"
        );
        assert_eq!(
            tea_err(&|s| {
                s.discretize = 1;
                s.groups = "0".into();
            }),
            "the number of groups of tea must be a whole number of at least 1"
        );
        assert_eq!(
            tea_err(&|s| s.discretize = 2),
            "the interval width of tea must be a number"
        );
        assert_eq!(
            tea_err(&|s| {
                s.discretize = 2;
                s.width = "0".into();
            }),
            "the interval width of tea must be above 0"
        );
        assert_eq!(
            tea_err(&|s| {
                s.supplementary = true;
                s.missing = LISTWISE;
            }),
            "supplementary variable tea cannot leave cases out for missing values"
        );
        assert_eq!(
            err(&|d| {
                for name in ["tea", "snack", "seat"] {
                    let mut s = McaVariable::new(name);
                    s.supplementary = true;
                    d.settings.push(s);
                }
            }),
            "at least one analysis variable must not be supplementary"
        );
        assert_eq!(
            err(&|d| d.objects = "1-4".into()),
            "at least three cases must stay out of the supplementary ones"
        );
        assert_eq!(
            err(&|d| d.dimensions = "0".into()),
            "the number of dimensions must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.dimensions = "1000000000".into()),
            "the number of dimensions must be below 1000000000"
        );
        assert_eq!(
            err(&|d| {
                d.normalization = 4;
                d.q = "2".into();
            }),
            "the normalization value lies from -1 to 1"
        );
        assert_eq!(
            err(&|d| d.convergence = "0".into()),
            "the convergence criterion must be above 0"
        );
        assert_eq!(
            err(&|d| {
                d.plots[5] = true;
                d.plot_dimensions = "3".into();
            }),
            "the plotted dimensions per variable must be at most 2"
        );
        assert_eq!(
            err(&|d| {
                d.ndim = true;
                d.ndim_last = "3".into();
            }),
            "the last plotted dimension must be above the first and at most 2"
        );
        assert_eq!(
            err(&|d| {
                d.save[1] = true;
                d.save_roots[1] = "scores".into();
            }),
            "the root of the object scores has at most 5 characters"
        );
        assert_eq!(
            err(&|d| {
                d.save[0] = true;
                d.save_roots[0] = "#q".into();
            }),
            "#q cannot start variable names"
        );
        assert_eq!(
            err(&|d| {
                d.outfile = [1, 2, 0];
                d.outfile_files[0] = "Same".into();
                d.outfile_datasets[1] = "same".into();
            }),
            "the quantified variables need a place of their own"
        );
        assert_eq!(err(&|d| d.outfile[2] = 2), "name the new dataset");
        // settings that are switched off are not checked
        let mut d = base();
        d.settings.push(McaVariable {
            groups: "x".into(),
            width: "-1".into(),
            ..McaVariable::new("tea")
        });
        d.q = "5".into();
        d.plots = [false; 7];
        d.label_length = "x".into();
        d.plot_dimensions = "9".into();
        d.ndim = true;
        d.ndim_last = "9".into();
        d.save_roots = ["#bad".into(), "toolong".into()];
        d.outfile_files[0] = "a".into();
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
        // lists of tables and plots left unchecked leave the defaults alone
        let mut d = base();
        d.quant = v(&["tea"]);
        d.plot_variables[2] = v(&["tea"]);
        d.biplot_labels = v(&["tea"]);
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/ANALYSIS=tea snack seat.")
        );
    }

    #[test]
    fn lists_keep_to_the_variables_still_chosen() {
        let data = data();
        let mut d = base();
        d.labels = v(&["note", "Tea"]);
        d.print[4] = true;
        d.object_label = "note".into();
        d.object_shown = v(&["seat", "gone"]);
        d.descrip = v(&["note"]);
        d.plot_variables[0] = v(&["note"]);
        d.plot_variables[1] = v(&["note", "snack"]);
        d.plots[1] = true;
        d.plots[BIPLOT] = true;
        d.biplot_labels = v(&["note"]);
        d.normalization = INDEPENDENT;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MULTIPLE CORRESPONDENCE VARIABLES=tea snack seat note\n  /ANALYSIS=tea snack seat\n  \
             /NORMALIZATION=INDEPENDENT\n  \
             /PRINT=CORR DESCRIP(note) DISCRIM OBJECT((note) seat)\n  \
             /PLOT=OBJECT(note) CATEGORY(snack) DISCRIM."
        );
    }
}
