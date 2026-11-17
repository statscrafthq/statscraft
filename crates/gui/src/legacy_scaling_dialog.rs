//! The dialogs of the older optimal scaling commands: HOMALS, PRINCALS and OVERALS (one model,
//! `LegacyScaling`, since they read the same VARIABLES, ANALYSIS and options) and ANACOR.
//! Options left as they are add nothing to the syntax, so the command runs with its own
//! defaults.

use crate::analyze_dialogs::keywords;
use crate::correspondence_dialog::TableVariable;
use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::scaling_dialog::{bounded, ndim, still};
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};

/// Which of the three commands a `LegacyScaling` writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalingKind {
    Homals,
    Princals,
    Overals,
}

/// What differs between the commands: their tables and charts with the defaults.
pub struct Spec {
    pub name: &'static str,
    pub title: &'static str,
    pub print: &'static [(&'static str, &'static str)],
    pub print_default: &'static [bool],
    pub plots: &'static [(&'static str, &'static str)],
    pub plots_default: &'static [bool],
    /// The charts that take a list of variables.
    pub plot_lists: &'static [&'static str],
    /// The charts that take no label length.
    pub plot_plain: &'static [&'static str],
    /// The variables take a scaling level.
    pub levels: bool,
}

const HOMALS: Spec = Spec {
    name: "HOMALS",
    title: "Homogeneity Analysis (HOMALS)",
    print: &[
        ("FREQ", "Frequencies"),
        ("DISCRIM", "Discrimination measures"),
        ("QUANT", "Category quantifications"),
        ("OBJECT", "Object scores"),
        ("HISTORY", "Iteration history"),
    ],
    print_default: &[true, true, true, false, false],
    plots: &[
        ("QUANT", "Category points"),
        ("OBJECT", "Object points"),
        ("DISCRIM", "Discrimination measures"),
    ],
    plots_default: &[true, true, false],
    plot_lists: &["QUANT", "OBJECT"],
    plot_plain: &[],
    levels: false,
};

const PRINCALS: Spec = Spec {
    name: "PRINCALS",
    title: "Nonlinear Principal Components (PRINCALS)",
    print: &[
        ("FREQ", "Frequencies"),
        ("LOADINGS", "Component loadings"),
        ("QUANT", "Category quantifications"),
        ("CORRELATION", "Correlations of quantified variables"),
        ("OBJECT", "Object scores"),
        ("HISTORY", "Iteration history"),
    ],
    print_default: &[true, true, false, false, false, false],
    plots: &[
        ("QUANT", "Category points"),
        ("OBJECT", "Object points"),
        ("LOADINGS", "Component loadings"),
    ],
    plots_default: &[true, true, false],
    plot_lists: &["QUANT", "OBJECT"],
    plot_plain: &[],
    levels: true,
};

const OVERALS: Spec = Spec {
    name: "OVERALS",
    title: "Nonlinear Canonical Correlation (OVERALS)",
    print: &[
        ("FREQ", "Frequencies"),
        ("CENTROID", "Centroids"),
        ("WEIGHTS", "Weights and loadings"),
        ("FIT", "Fit measures"),
        ("QUANT", "Category quantifications"),
        ("OBJECT", "Object scores"),
        ("HISTORY", "Iteration history"),
    ],
    print_default: &[true, true, true, false, false, false, false],
    plots: &[
        ("OBJECT", "Object points"),
        ("LOADINGS", "Loadings"),
        ("QUANT", "Category coordinates"),
        ("TRANS", "Transformations"),
        ("CENTROID", "Centroids"),
    ],
    plots_default: &[true, true, false, false, false],
    plot_lists: &["OBJECT", "QUANT", "TRANS", "CENTROID"],
    plot_plain: &["TRANS"],
    levels: true,
};

impl ScalingKind {
    pub fn spec(self) -> &'static Spec {
        match self {
            ScalingKind::Homals => &HOMALS,
            ScalingKind::Princals => &PRINCALS,
            ScalingKind::Overals => &OVERALS,
        }
    }
}

/// The scaling levels of the variables; the first is the default.
pub const LEGACY_LEVELS: [(&str, &str); 4] = [
    ("ORDI", "Ordinal"),
    ("SNOM", "Single nominal"),
    ("MNOM", "Multiple nominal"),
    ("NUME", "Numerical"),
];

/// The longest label a chart of these commands takes.
const LONGEST_LABEL: u32 = 20;

/// The settings of one variable.
#[derive(Clone, PartialEq, Debug)]
pub struct LegacyVariable {
    pub name: String,
    /// The largest category: the categories are the whole values from 1 up to it.
    pub max: String,
    /// Index into `LEGACY_LEVELS`.
    pub level: usize,
    /// The set it belongs to, from 1 (OVERALS).
    pub set: usize,
}

impl LegacyVariable {
    pub fn new(name: &str, max: &str) -> Self {
        Self {
            name: name.to_string(),
            max: max.to_string(),
            level: 0,
            set: 1,
        }
    }
}

/// HOMALS, PRINCALS or OVERALS.
#[derive(Clone, PartialEq, Debug)]
pub struct LegacyScaling {
    pub kind: ScalingKind,
    pub variables: Vec<String>,
    /// The settings of the variables, found by name.
    pub settings: Vec<LegacyVariable>,
    /// The number of sets (OVERALS).
    pub sets: String,
    /// Start OVERALS from random scores instead of numerical fits.
    pub random_start: bool,
    pub dimensions: String,
    pub max_iterations: String,
    pub convergence: String,
    /// One flag per entry of the command's `print` table.
    pub print: Vec<bool>,
    /// One flag per entry of the command's `plots` table.
    pub plots: Vec<bool>,
    /// The variables of each plot that takes a list; none means the command's own choice.
    pub plot_variables: Vec<Vec<String>>,
    /// The longest label in the plots; empty for the command's own.
    pub label_length: String,
    pub ndim: bool,
    pub ndim_first: String,
    /// Empty: up to the last dimension.
    pub ndim_last: String,
    pub save: bool,
    /// The root of the saved object scores, empty for the command's own.
    pub save_root: String,
    /// The dimensions saved, empty for all.
    pub save_dims: String,
}

impl Default for LegacyScaling {
    fn default() -> Self {
        Self::new(ScalingKind::Homals)
    }
}

impl LegacyScaling {
    pub fn new(kind: ScalingKind) -> Self {
        let spec = kind.spec();
        Self {
            kind,
            variables: Vec::new(),
            settings: Vec::new(),
            sets: "2".into(),
            random_start: false,
            dimensions: "2".into(),
            max_iterations: "100".into(),
            convergence: "0.00001".into(),
            print: spec.print_default.to_vec(),
            plots: spec.plots_default.to_vec(),
            plot_variables: vec![Vec::new(); spec.plots.len()],
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: String::new(),
            save: false,
            save_root: String::new(),
            save_dims: String::new(),
        }
    }

    pub fn spec(&self) -> &'static Spec {
        self.kind.spec()
    }

    pub fn reset(&self) -> Self {
        Self::new(self.kind)
    }

    /// The settings of a variable, the defaults when it has none yet.
    pub fn setting(&self, name: &str) -> LegacyVariable {
        self.settings
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .unwrap_or_else(|| LegacyVariable::new(name, ""))
    }

    /// The ANALYSIS list: runs of variables with the same level, each closed by it (the
    /// parentheses cover every variable since the previous ones).
    fn analysis(order: &[&LegacyVariable]) -> String {
        let mut runs: Vec<(Vec<&str>, usize)> = Vec::new();
        for s in order {
            match runs.last_mut() {
                Some((names, level)) if *level == s.level => names.push(&s.name),
                _ => runs.push((vec![&s.name], s.level)),
            }
        }
        runs.iter()
            .map(|(names, level)| format!("{}({})", names.join(" "), LEGACY_LEVELS[*level].0))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let spec = self.spec();
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        numeric(data, &self.variables)?;
        let settings: Vec<LegacyVariable> =
            self.variables.iter().map(|n| self.setting(n)).collect();
        let mut entries = Vec::new();
        for s in &settings {
            let what = format!("the largest category of {}", s.name);
            let max = bounded(&s.max, 1, &what)?.ok_or_else(|| format!("give {what}"))?;
            entries.push(format!("{}({max})", s.name));
        }
        let mut text = format!("{} VARIABLES={}", spec.name, entries.join(" "));
        let mut ordered: Vec<&LegacyVariable> = settings.iter().collect();
        if self.kind == ScalingKind::Overals {
            let sets = bounded(&self.sets, 2, "the number of sets")?
                .ok_or("give the number of sets")? as usize;
            if let Some(s) = settings.iter().find(|s| s.set > sets) {
                return Err(format!(
                    "{} is in set {}, but there are only {sets} sets",
                    s.name, s.set
                ));
            }
            // the sets are consecutive variables of ANALYSIS
            ordered.sort_by_key(|s| s.set);
            let sizes: Vec<usize> = (1..=sets)
                .map(|k| settings.iter().filter(|s| s.set == k).count())
                .collect();
            if let Some(k) = sizes.iter().position(|n| *n == 0) {
                return Err(format!("set {} has no variable", k + 1));
            }
            let sizes: Vec<String> = sizes.iter().map(usize::to_string).collect();
            text.push_str(&format!(
                "\n  /ANALYSIS={}\n  /SETS={sets}({})",
                Self::analysis(&ordered),
                sizes.join(",")
            ));
            if self.random_start {
                text.push_str("\n  /INITIAL=RANDOM");
            }
        } else if spec.levels && settings.iter().any(|s| s.level != 0) {
            text.push_str(&format!("\n  /ANALYSIS={}", Self::analysis(&ordered)));
        }
        let dims = bounded(&self.dimensions, 1, "the number of dimensions")?.unwrap_or(2);
        if dims != 2 {
            text.push_str(&format!("\n  /DIMENSION={dims}"));
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
                text.push_str(&format!("\n  /CONVERGENCE={}", number(c)));
            }
        }
        if self.print != spec.print_default {
            text.push_str(&print_syntax(spec.print, &self.print));
        }
        text.push_str(&self.plot(dims)?);
        if self.save {
            text.push_str(&self.save_syntax(dims)?);
        }
        text.push('.');
        Ok(text)
    }

    fn plot(&self, dims: u32) -> Result<String, String> {
        let spec = self.spec();
        let length = label_length(&self.label_length)?;
        let mut words = Vec::new();
        let mut listed = false;
        for (i, (key, _)) in spec.plots.iter().enumerate() {
            if !self.plots[i] {
                continue;
            }
            let mut word = key.to_string();
            if spec.plot_lists.contains(key) {
                let list = still(&self.variables, &self.plot_variables[i]);
                if !list.is_empty() {
                    listed = true;
                    word.push_str(&format!("({})", list.join(" ")));
                }
            }
            if let (Some(n), false) = (length, spec.plot_plain.contains(key)) {
                word.push_str(&format!("({n})"));
            }
            words.push(word);
        }
        if self.ndim {
            words.push(ndim(&self.ndim_first, &self.ndim_last, dims)?);
        } else if self.plots == spec.plots_default && !listed && length.is_none() {
            return Ok(String::new());
        }
        Ok(plot_syntax(self.plots.iter().any(|p| *p), &words))
    }

    fn save_syntax(&self, dims: u32) -> Result<String, String> {
        let root = self.save_root.trim();
        if !root.is_empty()
            && (!is_valid_name(root) || root.starts_with('#') || root.chars().count() > 6)
        {
            return Err("the root of the saved scores is a name of up to 6 characters".into());
        }
        let count = match bounded(&self.save_dims, 1, "the saved dimensions")? {
            Some(n) if n > dims => {
                return Err(format!("the saved dimensions must be at most {dims}"));
            }
            Some(n) => format!("({n})"),
            None => String::new(),
        };
        Ok(format!("\n  /SAVE={root}{count}"))
    }
}

/// The label length from 1 to 20, None when empty.
fn label_length(text: &str) -> Result<Option<u32>, String> {
    match whole(text, 1, "the longest label")? {
        Some(n) if n > LONGEST_LABEL => {
            Err(format!("the longest label lies from 1 to {LONGEST_LABEL}"))
        }
        n => Ok(n),
    }
}

/// /PRINT of the checked tables, NONE when there are none.
fn print_syntax(table: &[(&str, &str)], flags: &[bool]) -> String {
    let words = keywords(table, flags);
    match words.is_empty() {
        true => "\n  /PRINT=NONE".to_string(),
        false => format!("\n  /PRINT={}", words.join(" ")),
    }
}

/// /PLOT of the words; with no chart on, NONE keeps the default charts away from NDIM alone.
fn plot_syntax(any: bool, words: &[String]) -> String {
    match any {
        true => format!("\n  /PLOT={}", words.join(" ")),
        false => format!("\n  /PLOT=NONE {}", words.join(" "))
            .trim_end()
            .to_string(),
    }
}

/// The normalizations of ANACOR; the last takes a value from -2 to 2.
pub const ANACOR_NORMALIZATIONS: [(&str, &str); 5] = [
    ("CANONICAL", "Canonical"),
    ("PRINCIPAL", "Principal"),
    ("RPRINCIPAL", "Row principal"),
    ("CPRINCIPAL", "Column principal"),
    ("", "Custom value"),
];

pub const ANACOR_PRINT: [(&str, &str); 5] = [
    ("TABLE", "Correspondence table"),
    ("PROFILES", "Row and column profiles"),
    ("SCORES", "Overview of row and column points"),
    ("CONTRIBUTIONS", "Contributions of the points"),
    ("PERMUTATION", "Permuted table"),
];

const ANACOR_DEFAULT_PRINT: [bool; 5] = [true, false, true, true, false];

pub const ANACOR_PLOTS: [(&str, &str); 5] = [
    ("ROWS", "Row points"),
    ("COLUMNS", "Column points"),
    ("JOINT", "Row and column points together"),
    ("TRROWS", "Transformed row categories"),
    ("TRCOLUMNS", "Transformed column categories"),
];

const ANACOR_DEFAULT_PLOTS: [bool; 5] = [true, true, false, false, false];
pub const JOINT: usize = 2;

/// ANACOR: correspondence analysis of a two-way table, the older command.
#[derive(Clone, PartialEq, Debug)]
pub struct Anacor {
    pub row: TableVariable,
    pub column: TableVariable,
    pub dimensions: String,
    /// Index into `ANACOR_NORMALIZATIONS`.
    pub normalization: usize,
    pub q: String,
    /// The spread and correlations of the row and column scores.
    pub row_variances: bool,
    pub column_variances: bool,
    /// One flag per entry of `ANACOR_PRINT`.
    pub print: [bool; 5],
    /// One flag per entry of `ANACOR_PLOTS`.
    pub plots: [bool; 5],
    /// The longest label in the point plots; empty for the command's own.
    pub label_length: String,
    pub ndim: bool,
    pub ndim_first: String,
    /// Empty: up to the last dimension.
    pub ndim_last: String,
}

impl Default for Anacor {
    fn default() -> Self {
        Self {
            row: TableVariable::default(),
            column: TableVariable::default(),
            dimensions: "2".into(),
            normalization: 0,
            q: "0".into(),
            row_variances: false,
            column_variances: false,
            print: ANACOR_DEFAULT_PRINT,
            plots: ANACOR_DEFAULT_PLOTS,
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: String::new(),
        }
    }
}

impl Anacor {
    /// The joint plot is not drawn with principal normalization.
    pub fn joint_allowed(&self) -> bool {
        ANACOR_NORMALIZATIONS[self.normalization].0 != "PRINCIPAL"
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        for (v, what) in [(&self.row, "row"), (&self.column, "column")] {
            let name = v.name.trim();
            if name.is_empty() {
                return Err(format!("choose the {what} variable"));
            }
            match data.variable(name) {
                Err(_) => return Err(format!("{name} is no longer a variable")),
                Ok(var) if var.is_string() => {
                    return Err(format!("{name} is a string; the command needs numbers"));
                }
                Ok(_) => {}
            }
        }
        let (row, column) = (self.row.name.trim(), self.column.name.trim());
        if row.eq_ignore_ascii_case(column) {
            return Err(format!(
                "{row} cannot be both the row and the column variable"
            ));
        }
        let (r, c) = (self.row.range()?, self.column.range()?);
        for ((min, max), name) in [(r, row), (c, column)] {
            if min == max {
                return Err(format!("{name} needs at least two categories"));
            }
        }
        let mut text = format!(
            "ANACOR TABLE={row}({},{}) BY {column}({},{})",
            r.0, r.1, c.0, c.1
        );
        let dims = bounded(&self.dimensions, 1, "the number of dimensions")?.unwrap_or(2);
        if dims != 2 {
            text.push_str(&format!("\n  /DIMENSION={dims}"));
        }
        match ANACOR_NORMALIZATIONS[self.normalization].0 {
            "CANONICAL" => {}
            "" => {
                let q = parse(&self.q, "the normalization value")?;
                if !(-2.0..=2.0).contains(&q) {
                    return Err("the normalization value lies from -2 to 2".into());
                }
                if q != 0.0 {
                    text.push_str(&format!("\n  /NORMALIZATION={}", number(q)));
                }
            }
            k => text.push_str(&format!("\n  /NORMALIZATION={k}")),
        }
        let variances: Vec<&str> = [
            (self.row_variances, "ROWS"),
            (self.column_variances, "COLUMNS"),
        ]
        .into_iter()
        .filter_map(|(on, k)| on.then_some(k))
        .collect();
        if !variances.is_empty() {
            text.push_str(&format!("\n  /VARIANCES={}", variances.join(" ")));
        }
        if self.print != ANACOR_DEFAULT_PRINT {
            text.push_str(&print_syntax(&ANACOR_PRINT, &self.print));
        }
        text.push_str(&self.plot(dims)?);
        text.push('.');
        Ok(text)
    }

    fn plot(&self, dims: u32) -> Result<String, String> {
        let mut plots = self.plots;
        plots[JOINT] &= self.joint_allowed();
        let length = label_length(&self.label_length)?;
        let mut words: Vec<String> = keywords(&ANACOR_PLOTS, &plots)
            .into_iter()
            .map(|k| match (length, k.starts_with("TR")) {
                (Some(n), false) => format!("{k}({n})"),
                _ => k.to_string(),
            })
            .collect();
        if self.ndim {
            words.push(ndim(&self.ndim_first, &self.ndim_last, dims)?);
        } else if plots == ANACOR_DEFAULT_PLOTS && length.is_none() {
            return Ok(String::new());
        }
        Ok(plot_syntax(plots.iter().any(|p| *p), &words))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    fn data() -> Dataset {
        open("survey_basic.sav").dataset().unwrap().clone()
    }

    fn var(name: &str, max: &str) -> LegacyVariable {
        LegacyVariable::new(name, max)
    }

    #[test]
    fn homals_writes_the_largest_categories() {
        let data = data();
        let mut d = LegacyScaling::default();
        assert_eq!(d.syntax(&data).unwrap_err(), "add at least two variables");
        d.variables = v(&["gender", "region"]);
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give the largest category of gender"
        );
        d.settings = vec![var("gender", "2"), var("region", "5")];
        assert_eq!(
            d.syntax(&data).unwrap(),
            "HOMALS VARIABLES=gender(2) region(5)."
        );
        d.dimensions = "3".into();
        d.print = vec![true, false, true, true, false];
        d.plots = vec![true, false, true];
        d.plot_variables[0] = v(&["region"]);
        d.label_length = "6".into();
        d.save = true;
        d.save_root = "hs".into();
        d.save_dims = "2".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "HOMALS VARIABLES=gender(2) region(5)\n  /DIMENSION=3\n  /PRINT=FREQ QUANT OBJECT\
             \n  /PLOT=QUANT(region)(6) DISCRIM(6)\n  /SAVE=hs(2)."
        );
        d.print = vec![false; 5];
        d.plots = vec![false; 3];
        d.label_length = String::new();
        d.save_dims = "4".into();
        assert!(d.syntax(&data).unwrap_err().contains("at most 3"));
        d.save_dims = String::new();
        d.ndim = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "HOMALS VARIABLES=gender(2) region(5)\n  /DIMENSION=3\n  /PRINT=NONE\
             \n  /PLOT=NONE NDIM(1,3)\n  /SAVE=hs."
        );
        d.label_length = "21".into();
        assert!(d.syntax(&data).unwrap_err().contains("1 to 20"));
    }

    #[test]
    fn princals_writes_the_level_runs() {
        let data = data();
        let mut d = LegacyScaling::new(ScalingKind::Princals);
        d.variables = v(&["gender", "region", "q6_mention1"]);
        d.settings = vec![
            var("gender", "2"),
            var("region", "5"),
            var("q6_mention1", "6"),
        ];
        assert_eq!(
            d.syntax(&data).unwrap(),
            "PRINCALS VARIABLES=gender(2) region(5) q6_mention1(6)."
        );
        d.settings[1].level = 1;
        d.settings[2].level = 3;
        d.print = vec![true, true, true, false, false, false];
        d.plots = vec![false, false, true];
        assert_eq!(
            d.syntax(&data).unwrap(),
            "PRINCALS VARIABLES=gender(2) region(5) q6_mention1(6)\n  \
             /ANALYSIS=gender(ORDI) region(SNOM) q6_mention1(NUME)\n  /PRINT=FREQ LOADINGS QUANT\
             \n  /PLOT=LOADINGS."
        );
    }

    #[test]
    fn overals_orders_the_variables_by_set() {
        let data = data();
        let mut d = LegacyScaling::new(ScalingKind::Overals);
        d.variables = v(&["gender", "region", "q6_mention1"]);
        d.settings = vec![
            var("gender", "2"),
            var("region", "5"),
            var("q6_mention1", "6"),
        ];
        assert_eq!(d.syntax(&data).unwrap_err(), "set 2 has no variable");
        d.settings[0].set = 2;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "OVERALS VARIABLES=gender(2) region(5) q6_mention1(6)\n  \
             /ANALYSIS=region q6_mention1 gender(ORDI)\n  /SETS=2(2,1)."
        );
        d.settings[1].level = 2;
        d.random_start = true;
        d.print = vec![true, true, true, true, false, false, false];
        d.plots = vec![true, false, false, true, true];
        d.plot_variables[4] = v(&["region"]);
        d.sets = "3".into();
        assert_eq!(d.syntax(&data).unwrap_err(), "set 3 has no variable");
        d.sets = "2".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "OVERALS VARIABLES=gender(2) region(5) q6_mention1(6)\n  \
             /ANALYSIS=region(MNOM) q6_mention1 gender(ORDI)\n  /SETS=2(2,1)\n  /INITIAL=RANDOM\
             \n  /PRINT=FREQ CENTROID WEIGHTS FIT\n  /PLOT=OBJECT TRANS CENTROID(region)."
        );
    }

    #[test]
    fn anacor_writes_the_table_and_its_options() {
        let data = data();
        let side = |name: &str, max: &str| TableVariable {
            name: name.into(),
            min: "1".into(),
            max: max.into(),
            ..Default::default()
        };
        let mut d = Anacor::default();
        assert_eq!(d.syntax(&data).unwrap_err(), "choose the row variable");
        d.row = side("region", "5");
        d.column = side("q6_mention1", "6");
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ANACOR TABLE=region(1,5) BY q6_mention1(1,6)."
        );
        d.dimensions = "3".into();
        d.normalization = 2;
        d.row_variances = true;
        d.column_variances = true;
        d.print = [true, true, false, false, true];
        d.plots = [true, false, true, true, false];
        d.label_length = "5".into();
        d.ndim = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ANACOR TABLE=region(1,5) BY q6_mention1(1,6)\n  /DIMENSION=3\n  /NORMALIZATION=RPRINCIPAL\
             \n  /VARIANCES=ROWS COLUMNS\n  /PRINT=TABLE PROFILES PERMUTATION\
             \n  /PLOT=ROWS(5) JOINT(5) TRROWS NDIM(1,3)."
        );
        d.normalization = 4;
        d.q = "2.5".into();
        assert!(d.syntax(&data).unwrap_err().contains("-2 to 2"));
        d.normalization = 1;
        d.plots = [false; 5];
        d.ndim = false;
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
    }
}
