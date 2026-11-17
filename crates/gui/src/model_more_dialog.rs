//! The Quantile Regression, Variance Components and Optimal Binning dialogs (QUANTILE
//! REGRESSION, VARCOMP and OPTIMAL BINNING). Options left as they are add nothing to the
//! syntax, so the commands run with their own defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::{names, number, quote};
use statscraft_data::is_valid_name;

/// The tables QUANTILE REGRESSION prints; the estimates are the default.
pub const QR_PRINT: [(&str, &str); 3] = [
    ("PARAMETER", "Parameter estimates"),
    ("COVB", "Covariances of the estimates"),
    ("CORB", "Correlations of the estimates"),
];

/// The variables QUANTILE REGRESSION saves.
pub const QR_SAVES: [(&str, &str); 4] = [
    ("PRED", "Predicted value"),
    ("RESID", "Residual"),
    ("CIPREDL", "Lower limit of the predicted value"),
    ("CIPREDU", "Upper limit of the predicted value"),
];

pub const QR_METHODS: [(&str, &str); 3] = [
    ("AUTO", "Automatic"),
    ("SIMPLEX", "Simplex"),
    ("INTERIOR_POINT", "Interior point"),
];

pub const QR_BANDWIDTHS: [(&str, &str); 2] =
    [("BOFINGER", "Bofinger"), ("HALL_SHEATHER", "Hall-Sheather")];

/// The SAVE keywords that are on, each with its name when one is given.
fn save_names(list: &[(bool, String)]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for ((key, label), (on, name)) in QR_SAVES.iter().zip(list) {
        if !on {
            continue;
        }
        let name = name.trim();
        if name.is_empty() {
            out.push(key.to_string());
        } else if is_valid_name(name) {
            out.push(format!("{key}({name})"));
        } else {
            return Err(format!("{name:?} is not a valid name for the {label}"));
        }
    }
    Ok(out)
}

/// QUANTILE REGRESSION: a linear model of one or several quantiles of the dependent variable.
#[derive(Clone, PartialEq, Debug)]
pub struct QuantileRegression {
    pub dependent: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Quantiles between 0 and 1 separated by spaces.
    pub quantiles: String,
    pub intercept: bool,
    /// Standard errors from the spread of the residuals (identical errors), else from the
    /// density at each case.
    pub iid: bool,
    /// Index into `QR_BANDWIDTHS`.
    pub bandwidth: usize,
    pub level: String,
    /// Index into `QR_METHODS`.
    pub method: usize,
    pub include: bool,
    pub print: [bool; 3],
    /// A flag and a name of each of `QR_SAVES`; an empty name takes the command's own.
    pub saves: [(bool, String); 4],
}

impl Default for QuantileRegression {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            quantiles: "0.5".into(),
            intercept: true,
            iid: true,
            bandwidth: 0,
            level: "95".into(),
            method: 0,
            include: false,
            print: [true, false, false],
            saves: QR_SAVES.map(|_| (false, String::new())),
        }
    }
}

impl QuantileRegression {
    pub fn syntax(&self) -> Result<String, String> {
        if self.dependent.is_empty() {
            return Err("set the dependent variable".into());
        }
        let mut text = format!("QUANTILE REGRESSION {}", self.dependent);
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING CLASSMISSING=INCLUDE");
        }
        let mut criteria = String::new();
        let mut quantiles = Vec::new();
        for word in self.quantiles.split_whitespace() {
            let q = parse(word, "a quantile")?;
            if !(0.0..=1.0).contains(&q) {
                return Err(format!("the quantile {word} must be from 0 to 1"));
            }
            quantiles.push(number(q));
        }
        if quantiles.is_empty() {
            return Err("give at least one quantile".into());
        }
        if quantiles != ["0.5"] {
            criteria.push_str(&format!(" QUANTILE={}", quantiles.join(" ")));
        }
        if !self.iid {
            criteria.push_str(" IID=FALSE");
        }
        if self.bandwidth != 0 {
            criteria.push_str(&format!(" BANDWIDTH={}", QR_BANDWIDTHS[self.bandwidth].0));
        }
        let level = parse(&self.level, "the confidence level")?;
        if level <= 0.0 || level >= 100.0 {
            return Err("the confidence level must be above 0 and below 100".into());
        }
        if level != 95.0 {
            criteria.push_str(&format!(" CILEVEL={}", number(level)));
        }
        if self.method != 0 {
            criteria.push_str(&format!(" METHOD={}", QR_METHODS[self.method].0));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA{criteria}"));
        }
        if !self.intercept {
            text.push_str("\n  /MODEL INTERCEPT=FALSE");
        }
        if self.print != [true, false, false] {
            let print = keywords(&QR_PRINT, &self.print);
            if print.is_empty() {
                return Err("tick at least one table to print".into());
            }
            text.push_str(&format!("\n  /PRINT {}", print.join(" ")));
        }
        let saves = save_names(&self.saves)?;
        if !saves.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", saves.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// The estimators of VARCOMP: the keyword text and the label.
pub const VARCOMP_METHODS: [(&str, &str); 6] = [
    ("MINQUE(1)", "MINQUE, weight 1 on every random effect"),
    ("MINQUE(0)", "MINQUE, weight 0 on the random effects"),
    ("ML", "Maximum likelihood"),
    ("REML", "Restricted maximum likelihood"),
    ("SSTYPE(3)", "ANOVA, Type III sums of squares"),
    ("SSTYPE(1)", "ANOVA, Type I sums of squares"),
];
/// The estimators that iterate (ML and REML) and the ANOVA ones, as indexes of the list.
pub const VARCOMP_ML: std::ops::Range<usize> = 2..4;
pub const VARCOMP_ANOVA: std::ops::Range<usize> = 4..6;

/// VARCOMP: how much each random effect adds to the variance of the dependent variable.
#[derive(Clone, PartialEq, Debug)]
pub struct VarianceComponents {
    pub dependent: String,
    pub fixed: Vec<String>,
    pub random: Vec<String>,
    pub covariates: Vec<String>,
    pub weight: String,
    /// Index into `VARCOMP_METHODS`.
    pub method: usize,
    pub intercept: bool,
    pub include: bool,
    /// Empty: the command's own value.
    pub converge: String,
    pub eps: String,
    pub iterate: String,
    pub sums: bool,
    pub expected: bool,
    pub history: bool,
    pub every: String,
}

impl Default for VarianceComponents {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            fixed: Vec::new(),
            random: Vec::new(),
            covariates: Vec::new(),
            weight: String::new(),
            method: 0,
            intercept: true,
            include: false,
            converge: String::new(),
            eps: String::new(),
            iterate: String::new(),
            sums: false,
            expected: false,
            history: false,
            every: "1".into(),
        }
    }
}

/// A positive number the user typed, or None for an empty field.
fn positive(text: &str, what: &str) -> Result<Option<f64>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    match parse(text, what)? {
        n if n > 0.0 => Ok(Some(n)),
        _ => Err(format!("{what} must be above 0")),
    }
}

impl VarianceComponents {
    pub fn syntax(&self) -> Result<String, String> {
        if self.dependent.is_empty() {
            return Err("set the dependent variable".into());
        }
        if self.random.is_empty() {
            return Err("add at least one random factor".into());
        }
        if let Some(name) = self.random.iter().find(|n| self.fixed.contains(n)) {
            return Err(format!("{name} is both a fixed and a random factor"));
        }
        let factors: Vec<&str> = self
            .fixed
            .iter()
            .chain(&self.random)
            .map(String::as_str)
            .collect();
        let mut text = format!("VARCOMP {} BY {}", self.dependent, factors.join(" "));
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        text.push_str(&format!("\n  /RANDOM={}", self.random.join(" ")));
        if self.method != 0 {
            text.push_str(&format!("\n  /METHOD={}", VARCOMP_METHODS[self.method].0));
        }
        if !self.intercept {
            text.push_str("\n  /INTERCEPT=EXCLUDE");
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        if !self.weight.is_empty() {
            text.push_str(&format!("\n  /REGWGT={}", self.weight));
        }
        let mut criteria = Vec::new();
        // the iteration settings only serve ML and REML
        if VARCOMP_ML.contains(&self.method) {
            if let Some(n) = positive(&self.converge, "the convergence criterion")? {
                criteria.push(format!("CONVERGE({})", number(n)));
            }
            if let Some(n) = whole(&self.iterate, 1, "the largest number of iterations")? {
                criteria.push(format!("ITERATE({n})"));
            }
        }
        if let Some(n) = positive(&self.eps, "the tolerance for redundant columns")? {
            criteria.push(format!("EPS({})", number(n)));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        let mut print = Vec::new();
        if self.sums {
            print.push("SS".to_string());
        }
        // expected mean squares belong to the ANOVA estimators, the history to the iterating ones
        if self.expected && VARCOMP_ANOVA.contains(&self.method) {
            print.push("EMS".into());
        }
        if self.history && VARCOMP_ML.contains(&self.method) {
            let every = whole(&self.every, 1, "the history interval")?.unwrap_or(1);
            print.push(if every == 1 {
                "HISTORY".into()
            } else {
                format!("HISTORY({every})")
            });
        }
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// The tables OPTIMAL BINNING prints; the end points are the default.
pub const BINNING_PRINT: [(&str, &str); 3] = [
    ("ENDPOINTS", "End points of the bins"),
    ("DESCRIPTIVES", "Descriptive statistics"),
    ("ENTROPY", "Model entropy (guided binning only)"),
];

/// OPTIMAL BINNING: cuts for scale variables, guided by a variable (MDLP) or equal frequency.
#[derive(Clone, PartialEq, Debug)]
pub struct OptimalBinning {
    pub variables: Vec<String>,
    pub guide: String,
    /// Equal frequency bins even with a guide; without a guide the bins are always equal
    /// frequency.
    pub equal: bool,
    pub bins: String,
    /// Group the values into equal frequency classes before the guided search.
    pub preprocess: bool,
    pub classes: String,
    pub merge: String,
    pub lower_observed: bool,
    pub upper_observed: bool,
    pub lower_exclusive: bool,
    pub listwise: bool,
    pub save: bool,
    /// New variable names, one per variable; empty for the command's own.
    pub into: String,
    pub rules: String,
    pub print: [bool; 3],
}

impl Default for OptimalBinning {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            guide: String::new(),
            equal: false,
            bins: "10".into(),
            preprocess: true,
            classes: "1000".into(),
            merge: String::new(),
            lower_observed: false,
            upper_observed: false,
            lower_exclusive: false,
            listwise: false,
            save: false,
            into: String::new(),
            rules: String::new(),
            print: [true, false, false],
        }
    }
}

impl OptimalBinning {
    /// Equal frequency bins: chosen, or forced by the lack of a guide.
    pub fn is_equal(&self) -> bool {
        self.equal || self.guide.is_empty()
    }

    pub fn syntax(&self) -> Result<String, String> {
        let list = names(&self.variables, "Variables to bin")?;
        if self.variables.contains(&self.guide) {
            return Err(format!("{} is both guide and variable to bin", self.guide));
        }
        let equal = self.is_equal();
        let mut text = String::from("OPTIMAL BINNING\n  /VARIABLES");
        if !equal {
            text.push_str(&format!(" GUIDE={}", self.guide));
        }
        text.push_str(&format!(" BIN={list}"));
        if self.save {
            let into: Vec<&str> = self.into.split_whitespace().collect();
            if into.is_empty() {
                text.push_str(" SAVE=YES");
            } else if into.len() != self.variables.len() {
                return Err("give one new name for each variable to bin, or none".into());
            } else if let Some(name) = into.iter().find(|n| !is_valid_name(n)) {
                return Err(format!("{name:?} is not a valid variable name"));
            } else {
                text.push_str(&format!(" SAVE=YES (INTO={})", into.join(" ")));
            }
        }
        let mut criteria = String::new();
        if equal {
            let bins = whole(&self.bins, 2, "the number of bins")?.unwrap_or(10);
            if bins != 10 || !self.guide.is_empty() {
                criteria.push_str(&format!(" METHOD=EQUALFREQ(BINS={bins})"));
            }
        } else if !self.preprocess {
            criteria.push_str(" PREPROCESS=NONE");
        } else if let Some(n) = whole(&self.classes, 2, "the number of classes")?
            && n != 1000
        {
            criteria.push_str(&format!(" PREPROCESS=EQUALFREQ(BINS={n})"));
        }
        if self.lower_observed {
            criteria.push_str(" LOWEREND=OBSERVED");
        }
        if self.upper_observed {
            criteria.push_str(" UPPEREND=OBSERVED");
        }
        if self.lower_exclusive {
            criteria.push_str(" LOWERLIMIT=EXCLUSIVE");
        }
        if !self.merge.trim().is_empty() {
            let r = parse(&self.merge, "the merge ratio")?;
            if r < 0.0 {
                return Err("the merge ratio must not be negative".into());
            }
            if r != 0.0 {
                criteria.push_str(&format!(" FORCEMERGE={}", number(r)));
            }
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA{criteria}"));
        }
        if self.listwise {
            text.push_str("\n  /MISSING SCOPE=LISTWISE");
        }
        if !self.rules.trim().is_empty() {
            text.push_str(&format!("\n  /OUTFILE RULES={}", quote(self.rules.trim())));
        }
        // the entropy table only exists for guided binning
        let flags = [self.print[0], self.print[1], self.print[2] && !equal];
        if flags != [true, false, false] {
            let print = keywords(&BINNING_PRINT, &flags);
            text.push_str(&if print.is_empty() {
                "\n  /PRINT NONE".to_string()
            } else {
                format!("\n  /PRINT {}", print.join(" "))
            });
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn quantile_regression_writes_only_what_differs_from_the_defaults() {
        let mut d = QuantileRegression {
            dependent: "rent".into(),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "QUANTILE REGRESSION rent.");
        d.factors = list(&["district"]);
        d.covariates = list(&["area", "age"]);
        d.quantiles = "0.25 0.5 0.75".into();
        d.iid = false;
        d.bandwidth = 1;
        d.level = "90".into();
        d.method = 1;
        d.intercept = false;
        d.print = [true, true, false];
        d.saves[0] = (true, "fit".into());
        d.saves[1] = (true, String::new());
        assert_eq!(
            d.syntax().unwrap(),
            "QUANTILE REGRESSION rent BY district WITH area age\n  /CRITERIA QUANTILE=0.25 0.5 0.75 \
             IID=FALSE BANDWIDTH=HALL_SHEATHER CILEVEL=90 METHOD=SIMPLEX\n  /MODEL INTERCEPT=FALSE\n  \
             /PRINT PARAMETER COVB\n  /SAVE PRED(fit) RESID."
        );
        d.quantiles = "1.5".into();
        assert!(d.syntax().is_err());
        d.quantiles = "0.5".into();
        d.saves[0].1 = "1x".into();
        assert!(d.syntax().is_err());
    }

    #[test]
    fn variance_components_need_a_random_factor_and_drop_options_of_other_methods() {
        let mut d = VarianceComponents {
            dependent: "strength".into(),
            fixed: list(&["mixture"]),
            ..Default::default()
        };
        assert!(d.syntax().is_err());
        d.random = list(&["batch"]);
        assert_eq!(
            d.syntax().unwrap(),
            "VARCOMP strength BY mixture batch\n  /RANDOM=batch."
        );
        d.method = 3;
        d.covariates = list(&["temp"]);
        d.intercept = false;
        d.converge = "0.000001".into();
        d.iterate = "30".into();
        d.history = true;
        d.every = "5".into();
        d.expected = true;
        assert_eq!(
            d.syntax().unwrap(),
            "VARCOMP strength BY mixture batch WITH temp\n  /RANDOM=batch\n  /METHOD=REML\n  \
             /INTERCEPT=EXCLUDE\n  /CRITERIA=CONVERGE(0.000001) ITERATE(30)\n  /PRINT=HISTORY(5)."
        );
        d.method = 4;
        d.sums = true;
        assert_eq!(
            d.syntax().unwrap(),
            "VARCOMP strength BY mixture batch WITH temp\n  /RANDOM=batch\n  /METHOD=SSTYPE(3)\n  \
             /INTERCEPT=EXCLUDE\n  /PRINT=SS EMS."
        );
        d.fixed = list(&["batch"]);
        assert!(d.syntax().is_err());
    }

    #[test]
    fn optimal_binning_is_equal_frequency_without_a_guide() {
        let mut d = OptimalBinning {
            variables: list(&["tenure", "spend"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "OPTIMAL BINNING\n  /VARIABLES BIN=tenure spend."
        );
        d.bins = "4".into();
        d.print = [true, true, true];
        assert_eq!(
            d.syntax().unwrap(),
            "OPTIMAL BINNING\n  /VARIABLES BIN=tenure spend\n  /CRITERIA METHOD=EQUALFREQ(BINS=4)\n  \
             /PRINT ENDPOINTS DESCRIPTIVES."
        );
        d.guide = "churned".into();
        d.equal = false;
        d.save = true;
        d.into = "t_bin s_bin".into();
        d.preprocess = false;
        d.merge = "0.05".into();
        d.lower_observed = true;
        d.lower_exclusive = true;
        d.listwise = true;
        d.rules = "bins.sps".into();
        d.print = [false; 3];
        assert_eq!(
            d.syntax().unwrap(),
            "OPTIMAL BINNING\n  /VARIABLES GUIDE=churned BIN=tenure spend SAVE=YES (INTO=t_bin s_bin)\n  \
             /CRITERIA PREPROCESS=NONE LOWEREND=OBSERVED LOWERLIMIT=EXCLUSIVE FORCEMERGE=0.05\n  \
             /MISSING SCOPE=LISTWISE\n  /OUTFILE RULES='bins.sps'\n  /PRINT NONE."
        );
        d.into = "one".into();
        assert!(d.syntax().is_err());
    }
}
