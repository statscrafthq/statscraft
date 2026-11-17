//! The Categorical Regression dialog (CATREG): a response and its predictors, each with its
//! scaling level, recoding and missing value handling, fitted by optimal scaling, with ridge,
//! lasso and elastic net penalties, resampling, tables, charts and saved variables. What it
//! shares with CATPCA is in `scaling_dialog`. Options left as they are add nothing to the
//! syntax, so the command runs with its own defaults.

use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::scaling_dialog::{
    ScalingVariable, bounded, label_length, paren, save_syntax, setting, still,
};
use crate::stats_dialogs::parse;
use crate::syntax_text::number;

/// The scaling levels; the first is the default, the first two are splines.
pub const CATREG_LEVELS: [(&str, &str); 5] = [
    ("SPORD", "Spline ordinal"),
    ("SPNOM", "Spline nominal"),
    ("ORDI", "Ordinal"),
    ("NOMI", "Nominal"),
    ("NUME", "Numeric"),
];

/// What a variable with missing values does; the first is the command's own.
pub const CATREG_MISSING: [(&str, &str); 3] = [
    ("LISTWISE", "Leave the case out"),
    ("MODEIMPU", "Fill in the mode"),
    ("EXTRACAT", "Fill in an extra category"),
];

/// The highest spline degree the dialog takes; higher ones only cost time.
const MOST_DEGREE: u32 = 100;

/// The scaling level of a variable with a spline's degree and knots, and its missing values.
#[derive(Clone, PartialEq, Debug)]
pub struct CatregSetting {
    /// Index into `CATREG_LEVELS`.
    pub level: usize,
    pub degree: String,
    pub knots: String,
    /// Index into `CATREG_MISSING`.
    pub missing: usize,
}

impl Default for CatregSetting {
    fn default() -> Self {
        Self {
            level: 0,
            degree: "2".into(),
            knots: "2".into(),
            missing: 0,
        }
    }
}

impl CatregSetting {
    pub fn spline(&self) -> bool {
        self.level < 2
    }

    /// The words in the parentheses after the variable; empty for a spline ordinal of the
    /// command's own shape.
    fn spec(&self, name: &str) -> Result<String, String> {
        let keyword = CATREG_LEVELS[self.level].0;
        if !self.spline() {
            return Ok(format!("LEVEL={keyword}"));
        }
        let what = format!("the spline degree of {name}");
        let degree = bounded(&self.degree, 1, &what)?.unwrap_or(2);
        if degree > MOST_DEGREE {
            return Err(format!("{what} goes from 1 to {MOST_DEGREE}"));
        }
        let knots = bounded(&self.knots, 0, &format!("the interior knots of {name}"))?.unwrap_or(2);
        let mut words = Vec::new();
        if self.level != 0 {
            words.push(format!("LEVEL={keyword}"));
        }
        if degree != 2 {
            words.push(format!("DEGREE={degree}"));
        }
        if knots != 2 {
            words.push(format!("INKNOT={knots}"));
        }
        Ok(words.join(" "))
    }
}

/// The settings of one variable of the analysis.
pub type CatregVariable = ScalingVariable<CatregSetting>;

/// How the iterations start, the first being the command's own.
pub const CATREG_INITIALS: [(&str, &str); 2] = [
    ("NUMERICAL", "Fit every variable as numeric first"),
    ("RANDOM", "From the standardized codes"),
];

/// The penalties; the first is none.
pub const CATREG_PENALTIES: [(&str, &str); 4] = [
    ("NONE", "None"),
    ("RIDGE", "Ridge"),
    ("LASSO", "Lasso"),
    ("ENET", "Elastic net"),
];
pub const ENET: usize = 3;

pub const CATREG_RESAMPLES: [(&str, &str); 3] = [
    ("NONE", "None"),
    ("CROSSVAL", "Cross validation"),
    ("BOOTSTRAP", "Bootstrap"),
];

/// The tables of /PRINT; QUANT takes a variable list.
pub const CATREG_PRINT: [(&str, &str); 9] = [
    ("R", "Model summary"),
    ("COEFF", "Coefficients"),
    ("DESCRIP", "Descriptives"),
    ("HISTORY", "Iteration history"),
    ("ANOVA", "Analysis of variance"),
    ("CORR", "Correlations of quantified predictors"),
    ("OCORR", "Correlations of original predictors"),
    ("QUANT", "Category quantifications"),
    ("REGU", "Penalty table (with a penalty)"),
];
const CATREG_DEFAULT_PRINT: [bool; 9] = [true, true, true, false, true, false, false, false, false];
pub const PRINT_QUANT: usize = 7;
pub const PRINT_REGU: usize = 8;

/// The variables /SAVE adds, with the longest root the command keeps.
pub const CATREG_SAVE: [(&str, &str, usize); 3] = [
    ("TRDATA", "Quantified variables", 5),
    ("PRED", "Predicted values", 5),
    ("RES", "Residuals", 5),
];

/// CATREG: regression of a quantified response on quantified predictors.
#[derive(Clone, PartialEq, Debug)]
pub struct CategoricalRegression {
    pub dependent: String,
    pub independents: Vec<String>,
    /// The settings of the variables, found by name; a variable without one has the defaults.
    pub settings: Vec<CatregVariable>,
    pub max_iterations: String,
    pub convergence: String,
    /// Index into `CATREG_INITIALS`.
    pub initial: usize,
    /// Index into `CATREG_PENALTIES`, with the start, stop and step of the ridge (the elastic
    /// net's own, which starts coarser) and lasso penalties.
    pub penalty: usize,
    pub ridge: [String; 3],
    pub enet_ridge: [String; 3],
    pub lasso: [String; 3],
    /// Index into `CATREG_RESAMPLES`, with the folds or samples.
    pub resample: usize,
    pub resample_count: String,
    /// One flag per entry of `CATREG_PRINT`.
    pub print: [bool; 9],
    /// The variables of the quantification table; none means all.
    pub quant: Vec<String>,
    pub plot_trans: bool,
    pub plot_trans_variables: Vec<String>,
    pub plot_resid: bool,
    pub plot_resid_variables: Vec<String>,
    pub plot_regu: bool,
    /// The longest label in the plots; empty for labels in full.
    pub label_length: String,
    pub save: [bool; 3],
    pub save_roots: [String; 3],
}

fn range(start: &str, stop: &str, step: &str) -> [String; 3] {
    [start.into(), stop.into(), step.into()]
}

impl Default for CategoricalRegression {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            independents: Vec::new(),
            settings: Vec::new(),
            max_iterations: "100".into(),
            convergence: "0.00001".into(),
            initial: 0,
            penalty: 0,
            ridge: range("0", "1", "0.02"),
            enet_ridge: range("0", "1", "0.1"),
            lasso: range("0", "1", "0.02"),
            resample: 0,
            resample_count: String::new(),
            print: CATREG_DEFAULT_PRINT,
            quant: Vec::new(),
            plot_trans: false,
            plot_trans_variables: Vec::new(),
            plot_resid: false,
            plot_resid_variables: Vec::new(),
            plot_regu: false,
            label_length: String::new(),
            save: [false; 3],
            save_roots: Default::default(),
        }
    }
}

/// `(start, stop, step)` of a penalty, checked as the command does.
fn penalty_range(values: &[String; 3], what: &str) -> Result<(f64, f64, f64), String> {
    let get =
        |i: usize, part: &str| parse(&values[i], &format!("the {part} of the {what} penalty"));
    let (a, b, c) = (get(0, "start")?, get(1, "stop")?, get(2, "step")?);
    if a < 0.0 || b < a || c < 0.0 || (c == 0.0 && b != a) {
        return Err(format!(
            "the {what} penalties need 0 <= start <= stop and a positive step (0 only when stop equals start)"
        ));
    }
    if c > 0.0 && (b - a) / c > 100_000.0 {
        return Err(format!("the {what} penalties have too many values"));
    }
    Ok((a, b, c))
}

impl CategoricalRegression {
    /// The response, then the predictors.
    pub fn variables(&self) -> Vec<String> {
        let mut all = Vec::new();
        if !self.dependent.is_empty() {
            all.push(self.dependent.clone());
        }
        all.extend(self.independents.iter().cloned());
        all
    }

    /// The settings of a variable, the defaults when it has none yet.
    pub fn setting(&self, name: &str) -> CatregVariable {
        setting(&self.settings, name)
    }

    /// Whether a penalty is chosen: the penalty table and chart need one.
    pub fn penalized(&self) -> bool {
        self.penalty > 0
    }

    /// The ANALYSIS predictors: runs of variables with the same setting, each run but a last
    /// default one closed by its setting, since parentheses cover every variable since the
    /// previous ones.
    fn analysis_list(entries: &[(&str, String)]) -> String {
        let mut runs: Vec<(Vec<&str>, &str)> = Vec::new();
        for (name, spec) in entries {
            match runs.last_mut() {
                Some((names, s)) if *s == spec.as_str() => names.push(name),
                _ => runs.push((vec![name], spec.as_str())),
            }
        }
        let last = runs.len().saturating_sub(1);
        runs.iter()
            .enumerate()
            .map(|(i, (names, spec))| match (i == last, spec.is_empty()) {
                (true, true) => names.join(" "),
                (false, true) => format!("{}(LEVEL=SPORD)", names.join(" ")),
                _ => format!("{}({spec})", names.join(" ")),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn syntax(&self) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err("set the dependent variable".into());
        }
        if self.independents.is_empty() {
            return Err("add at least one independent variable".into());
        }
        if self
            .independents
            .iter()
            .any(|n| n.eq_ignore_ascii_case(dependent))
        {
            return Err(format!(
                "{dependent} cannot also be an independent variable"
            ));
        }
        let all = self.variables();
        let mut specs = Vec::new();
        let mut discretization = Vec::new();
        let mut missing = Vec::new();
        for name in &all {
            let s = self.setting(name);
            specs.push((name.as_str(), s.level.spec(name)?));
            if let Some(how) = s.discretization()? {
                discretization.push(format!("{name}({how})"));
            }
            if s.level.missing > 0 {
                missing.push(format!("{name}({})", CATREG_MISSING[s.level.missing].0));
            }
        }
        let response = match specs[0].1.is_empty() {
            true => dependent.to_string(),
            false => format!("{dependent}({})", specs[0].1),
        };
        let mut text = format!(
            "CATREG VARIABLES={}\n  /ANALYSIS={response} WITH {}",
            all.join(" "),
            Self::analysis_list(&specs[1..])
        );
        if !discretization.is_empty() {
            text.push_str(&format!("\n  /DISCRETIZATION={}", discretization.join(" ")));
        }
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
        }
        if self.initial > 0 {
            text.push_str(&format!("\n  /INITIAL={}", CATREG_INITIALS[self.initial].0));
        }
        let iterations = bounded(&self.max_iterations, 1, "the most iterations")?.unwrap_or(100);
        if iterations != 100 {
            text.push_str(&format!("\n  /MAXITER={iterations}"));
        }
        if let Some(c) = optional(&self.convergence, "the convergence criterion")? {
            if !(0.000001..=0.1).contains(&c) {
                return Err("the convergence criterion lies from 0.000001 to 0.1".into());
            }
            if c != 0.00001 {
                text.push_str(&format!("\n  /CRITITER={}", number(c)));
            }
        }
        text.push_str(&self.regularization_syntax()?);
        match self.resample {
            0 => {}
            i => {
                let default = if i == 1 { 10 } else { 50 };
                let what = if i == 1 {
                    "the number of folds"
                } else {
                    "the number of samples"
                };
                let n = whole(&self.resample_count, 2, what)?.unwrap_or(default);
                let keyword = CATREG_RESAMPLES[i].0;
                match n == default {
                    true => text.push_str(&format!("\n  /RESAMPLE={keyword}")),
                    false => text.push_str(&format!("\n  /RESAMPLE={keyword}({n})")),
                }
            }
        }
        text.push_str(&self.print_syntax(&all));
        text.push_str(&self.plot_syntax(&all)?);
        text.push_str(&save_syntax(&CATREG_SAVE, &self.save, &self.save_roots)?);
        text.push('.');
        Ok(text)
    }

    /// /REGULARIZATION with the ranges that differ from the command's own.
    fn regularization_syntax(&self) -> Result<String, String> {
        if !self.penalized() {
            return Ok(String::new());
        }
        let keyword = CATREG_PENALTIES[self.penalty].0;
        let show =
            |r: (f64, f64, f64)| format!("({}, {}, {})", number(r.0), number(r.1), number(r.2));
        let lasso = (self.penalty >= 2)
            .then(|| penalty_range(&self.lasso, "lasso"))
            .transpose()?;
        let fields = if self.penalty == ENET {
            &self.enet_ridge
        } else {
            &self.ridge
        };
        let ridge = (self.penalty != 2)
            .then(|| penalty_range(fields, "ridge"))
            .transpose()?;
        // the elastic net's own ridge range has a coarser step
        let own_ridge = if self.penalty == ENET {
            (0.0, 1.0, 0.1)
        } else {
            (0.0, 1.0, 0.02)
        };
        let own_lasso = (0.0, 1.0, 0.02);
        let ranges = match (self.penalty, ridge, lasso) {
            (ENET, Some(r), Some(l)) if r != own_ridge || l != own_lasso => {
                format!("{}{}", show(r), show(l))
            }
            (1, Some(r), _) if r != own_ridge => show(r),
            (2, _, Some(l)) if l != own_lasso => show(l),
            _ => String::new(),
        };
        Ok(format!("\n  /REGULARIZATION={keyword}{ranges}"))
    }

    /// /PRINT when the tables or the quantification list differ from the defaults.
    fn print_syntax(&self, all: &[String]) -> String {
        let mut flags = self.print;
        flags[PRINT_REGU] &= self.penalized();
        let quant = still(all, &self.quant);
        if flags == CATREG_DEFAULT_PRINT && !(flags[PRINT_QUANT] && !quant.is_empty()) {
            return String::new();
        }
        let words: Vec<String> = CATREG_PRINT
            .iter()
            .zip(flags)
            .filter(|(_, on)| *on)
            .map(|((k, _), _)| match *k {
                "QUANT" => format!("QUANT{}", paren(&quant)),
                k => k.to_string(),
            })
            .collect();
        match words.is_empty() {
            true => "\n  /PRINT=NONE".into(),
            false => format!("\n  /PRINT={}", words.join(" ")),
        }
    }

    /// /PLOT: transformation and residual charts of the chosen variables (all when none is
    /// chosen), and the penalty paths.
    fn plot_syntax(&self, all: &[String]) -> Result<String, String> {
        let regu = self.plot_regu && self.penalized();
        if !(self.plot_trans || self.plot_resid || regu) {
            return Ok(String::new());
        }
        let length = label_length(&self.label_length, self.plot_trans || self.plot_resid)?;
        let mut words = Vec::new();
        if self.plot_trans {
            let named = still(all, &self.plot_trans_variables);
            let named = if named.is_empty() {
                all.to_vec()
            } else {
                named
            };
            words.push(format!("TRANS({}){length}", named.join(" ")));
        }
        if self.plot_resid {
            let named = still(&self.independents, &self.plot_resid_variables);
            let named = if named.is_empty() {
                self.independents.clone()
            } else {
                named
            };
            words.push(format!("RESID({}){length}", named.join(" ")));
        }
        if regu {
            words.push("REGU".into());
        }
        Ok(format!("\n  /PLOT={}", words.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::v;

    fn base() -> CategoricalRegression {
        CategoricalRegression {
            dependent: "rating".into(),
            independents: v(&["taste", "price", "area"]),
            ..Default::default()
        }
    }

    fn with(name: &str, change: &dyn Fn(&mut CatregVariable)) -> CatregVariable {
        let mut s = CatregVariable::new(name);
        change(&mut s);
        s
    }

    #[test]
    fn the_least_and_per_variable_settings() {
        assert_eq!(
            base().syntax().unwrap(),
            "CATREG VARIABLES=rating taste price area\n  /ANALYSIS=rating WITH taste price area."
        );
        let d = CategoricalRegression {
            settings: vec![
                with("rating", &|s| s.level.level = 2),
                with("price", &|s| s.level.level = 4),
                with("area", &|s| {
                    s.level.level = 1;
                    s.level.degree = "3".into();
                    s.level.knots = "1".into();
                    s.discretize = 1;
                    s.groups = "4".into();
                    s.uniform = true;
                    s.level.missing = 2;
                }),
            ],
            ..base()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "CATREG VARIABLES=rating taste price area\n  \
             /ANALYSIS=rating(LEVEL=ORDI) WITH taste(LEVEL=SPORD) price(LEVEL=NUME) \
             area(LEVEL=SPNOM DEGREE=3 INKNOT=1)\n  \
             /DISCRETIZATION=area(GROUPING NCAT=4 DISTR=UNIFORM)\n  /MISSING=area(EXTRACAT)."
        );
        assert!(
            CategoricalRegression {
                independents: v(&["rating"]),
                ..base()
            }
            .syntax()
            .is_err()
        );
    }

    #[test]
    fn model_penalty_tables_charts_and_saved_variables() {
        let d = CategoricalRegression {
            max_iterations: "50".into(),
            convergence: "0.001".into(),
            initial: 1,
            penalty: 3,
            enet_ridge: range("0", "2", "0.5"),
            resample: 1,
            resample_count: "5".into(),
            print: [true, false, false, true, true, true, true, true, true],
            quant: v(&["area", "gone"]),
            plot_trans: true,
            plot_trans_variables: v(&["area"]),
            plot_resid: true,
            plot_regu: true,
            label_length: "8".into(),
            save: [true, true, true],
            save_roots: [String::new(), "pr".into(), String::new()],
            ..base()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "CATREG VARIABLES=rating taste price area\n  /ANALYSIS=rating WITH taste price area\n  \
             /INITIAL=RANDOM\n  /MAXITER=50\n  /CRITITER=0.001\n  \
             /REGULARIZATION=ENET(0, 2, 0.5)(0, 1, 0.02)\n  /RESAMPLE=CROSSVAL(5)\n  \
             /PRINT=R HISTORY ANOVA CORR OCORR QUANT(area) REGU\n  \
             /PLOT=TRANS(area)(8) RESID(taste price area)(8) REGU\n  /SAVE=TRDATA PRED(pr) RES."
        );
        let none = CategoricalRegression {
            print: [false; 9],
            ..base()
        };
        assert!(none.syntax().unwrap().ends_with("/PRINT=NONE."));
        let bad = CategoricalRegression {
            penalty: 2,
            lasso: range("1", "0", "0.1"),
            ..base()
        };
        assert!(bad.syntax().unwrap_err().contains("lasso"));
    }
}
