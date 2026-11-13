//! The Ordinal Regression, Multinomial Logistic Regression, Linear Mixed Models and complex
//! samples dialogs (the analysis plan, Descriptives, Frequencies and Crosstabs). Options left as
//! they are add nothing to the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, parse, percent};
use crate::syntax_text::{names, number, quote, value};
use statscraft_data::Dataset;

/// The dialogs of this module, one variant of the window's dialogs.
#[derive(Clone, PartialEq, Debug)]
pub enum Advanced {
    Ordinal(Ordinal),
    Multinomial(Multinomial),
    Mixed(LinearMixed),
    Plan(SamplePlan),
    Descriptives(SampleDescriptives),
    Tables(SampleTables),
}

impl Advanced {
    pub fn title(&self) -> &'static str {
        match self {
            Advanced::Ordinal(_) => "Ordinal Regression",
            Advanced::Multinomial(_) => "Multinomial Logistic Regression",
            Advanced::Mixed(_) => "Linear Mixed Models",
            Advanced::Plan(_) => "Complex Samples Analysis Plan",
            Advanced::Descriptives(_) => "Complex Samples Descriptives",
            Advanced::Tables(d) if d.crosstabs => "Complex Samples Crosstabs",
            Advanced::Tables(_) => "Complex Samples Frequencies",
        }
    }

    /// The same dialog with its defaults.
    pub fn reset(&self) -> Self {
        match self {
            Advanced::Ordinal(_) => Advanced::Ordinal(Default::default()),
            Advanced::Multinomial(_) => Advanced::Multinomial(Default::default()),
            Advanced::Mixed(_) => Advanced::Mixed(Default::default()),
            Advanced::Plan(_) => Advanced::Plan(Default::default()),
            Advanced::Descriptives(_) => Advanced::Descriptives(Default::default()),
            Advanced::Tables(d) => Advanced::Tables(SampleTables {
                crosstabs: d.crosstabs,
                ..Default::default()
            }),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            // the dependent may be a string, the factors too
            Advanced::Ordinal(d) => numeric(data, &d.covariates).and_then(|()| d.syntax()),
            Advanced::Multinomial(d) => {
                let string = match data.variable(&d.dependent) {
                    Ok(v) => v.is_string(),
                    Err(_) if d.dependent.trim().is_empty() => false,
                    Err(_) => return Err(format!("{} is no longer a variable", d.dependent)),
                };
                numeric(data, &d.covariates).and_then(|()| d.syntax(string))
            }
            Advanced::Mixed(d) => numeric(data, std::iter::once(&d.dependent).chain(&d.covariates))
                .and_then(|()| d.syntax()),
            Advanced::Plan(d) => numeric(data, d.numeric()).and_then(|()| d.syntax()),
            Advanced::Descriptives(d) => numeric(
                data,
                d.summary.iter().chain(&d.numerators).chain(&d.denominators),
            )
            .and_then(|()| d.syntax()),
            Advanced::Tables(d) => d.syntax(),
        }
    }
}

/// The terms of a custom model: main effects and interactions of the model's variables.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Terms {
    /// Each term is one variable or an interaction of several.
    pub terms: Vec<Vec<String>>,
    /// The variables checked to make the next terms of.
    pub building: Vec<String>,
}

impl Terms {
    /// The checked variables each as a term of its own (`interaction` false) or together as one.
    pub fn add(&mut self, model: &[String], interaction: bool) {
        let checked: Vec<String> = model
            .iter()
            .filter(|n| self.building.contains(n))
            .cloned()
            .collect();
        // names taken out of the model since they were checked leave too few to combine
        let new: Vec<Vec<String>> = if interaction {
            match checked.len() {
                0 | 1 => Vec::new(),
                _ => vec![checked],
            }
        } else {
            checked.into_iter().map(|n| vec![n]).collect()
        };
        for term in new {
            if !self.terms.contains(&term) {
                self.terms.push(term);
            }
        }
        self.building.clear();
    }

    /// The terms as `a b a*b`, checked against the variables of the model.
    fn syntax(&self, model: &[String], what: &str) -> Result<String, String> {
        if self.terms.is_empty() {
            return Err(format!("add at least one term to the {what}"));
        }
        let mut out = Vec::new();
        for term in &self.terms {
            if term.is_empty() {
                return Err(format!("an empty term in the {what}: remove it"));
            }
            if let Some(n) = term.iter().find(|n| !model.contains(n)) {
                return Err(format!(
                    "{n} in the {what} is no longer a factor or covariate"
                ));
            }
            out.push(term.join("*"));
        }
        Ok(out.join(" "))
    }
}

/// `dependent BY factors WITH covariates`, checked: the dependent is chosen and is neither a
/// factor nor a covariate, and no variable is both.
fn model_variables(
    dependent: &str,
    factors: &[String],
    covariates: &[String],
) -> Result<String, String> {
    let dependent = dependent.trim();
    if dependent.is_empty() {
        return Err("choose the dependent variable".into());
    }
    if factors.iter().chain(covariates).any(|n| n == dependent) {
        return Err(format!("{dependent} is the dependent variable"));
    }
    if let Some(n) = factors.iter().find(|n| covariates.contains(n)) {
        return Err(format!("{n} is both a factor and a covariate"));
    }
    let mut text = dependent.to_string();
    if !factors.is_empty() {
        text.push_str(&format!(" BY {}", factors.join(" ")));
    }
    if !covariates.is_empty() {
        text.push_str(&format!(" WITH {}", covariates.join(" ")));
    }
    Ok(text)
}

/// `CIN(n) MXITER(n) MXSTEP(n)` where they differ from the defaults (95, 100 and `steps`).
fn iteration_criteria(
    ci_level: &str,
    iterations: &str,
    halvings: &str,
    steps: u32,
    least_iterations: u32,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    // an empty field means the default, as for the other fields here
    let level = match ci_level.trim() {
        "" => 95.0,
        given => percent(given)?,
    };
    if level != 95.0 {
        out.push(format!("CIN({})", number(level)));
    }
    match whole(
        iterations,
        least_iterations,
        "the maximum number of iterations",
    )? {
        Some(n) if n != 100 => out.push(format!("MXITER({n})")),
        _ => {}
    }
    match whole(halvings, 1, "the maximum step-halving")? {
        Some(n) if n != steps => out.push(format!("MXSTEP({n})")),
        _ => {}
    }
    Ok(out)
}

/// The variables saved by PLUM and NOMREG, keyword and label.
pub const PROBABILITY_SAVE: [(&str, &str); 4] = [
    ("ESTPROB", "Estimated response probabilities"),
    ("PREDCAT", "Predicted category"),
    ("PCPROB", "Predicted category probability"),
    ("ACPROB", "Actual category probability"),
];

pub const PLUM_LINKS: [(&str, &str); 5] = [
    ("LOGIT", "Logit"),
    ("PROBIT", "Probit"),
    ("CLOGLOG", "Complementary log-log"),
    ("NLOGLOG", "Negative log-log"),
    ("CAUCHIT", "Cauchit"),
];

/// Keyword and label of every table on /PRINT of PLUM.
pub const PLUM_PRINT: [(&str, &str); 9] = [
    ("FIT", "Goodness-of-fit statistics"),
    ("SUMMARY", "Summary statistics"),
    ("PARAMETER", "Parameter estimates"),
    ("TPARALLEL", "Test of parallel lines"),
    ("CELLINFO", "Cell information"),
    ("COVB", "Covariance of the estimates"),
    ("CORB", "Correlation of the estimates"),
    ("HISTORY", "Iteration history"),
    ("KERNEL", "Kernel of the log-likelihood"),
];

/// The tables PLUM shows without /PRINT.
const PLUM_DEFAULT_PRINT: [bool; 9] = [true, true, true, false, false, false, false, false, false];

/// PLUM: an ordinal dependent variable on factors and covariates.
#[derive(Clone, PartialEq, Debug)]
pub struct Ordinal {
    pub dependent: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `PLUM_LINKS`.
    pub link: usize,
    /// The location terms of `location` instead of the main effects.
    pub custom_location: bool,
    pub location: Terms,
    /// A scale part made of `scale_terms`.
    pub scale: bool,
    pub scale_terms: Terms,
    /// One flag per entry of `PLUM_PRINT`.
    pub print: [bool; 9],
    /// One flag per entry of `PROBABILITY_SAVE`.
    pub save: [bool; 4],
    pub ci_level: String,
    pub iterations: String,
    pub halvings: String,
    pub delta: String,
    /// User-missing values count as valid.
    pub include: bool,
}

impl Default for Ordinal {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            link: 0,
            custom_location: false,
            location: Terms::default(),
            scale: false,
            scale_terms: Terms::default(),
            print: PLUM_DEFAULT_PRINT,
            save: [false; 4],
            ci_level: "95".into(),
            iterations: "100".into(),
            halvings: "5".into(),
            delta: "0".into(),
            include: false,
        }
    }
}

impl Ordinal {
    /// The factors and covariates, the variables the terms are made of.
    pub fn model(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    pub fn syntax(&self) -> Result<String, String> {
        let variables = model_variables(&self.dependent, &self.factors, &self.covariates)?;
        let model = self.model();
        let mut text = format!("PLUM {variables}");
        if self.link > 0 {
            text.push_str(&format!("\n  /LINK={}", PLUM_LINKS[self.link].0));
        }
        if self.custom_location {
            let terms = self.location.syntax(&model, "location model")?;
            text.push_str(&format!("\n  /LOCATION={terms}"));
        }
        if self.scale {
            let terms = self.scale_terms.syntax(&model, "scale model")?;
            text.push_str(&format!("\n  /SCALE={terms}"));
        }
        // PLUM takes 0 iterations: the starting estimates only
        let mut criteria =
            iteration_criteria(&self.ci_level, &self.iterations, &self.halvings, 5, 0)?;
        // empty is the default, as for the iterations
        let delta = match self.delta.trim() {
            "" => 0.0,
            text => parse(text, "delta")?,
        };
        if !(0.0..1.0).contains(&delta) {
            return Err("delta must be at least 0 and below 1".into());
        }
        if delta != 0.0 {
            criteria.push(format!("DELTA({})", number(delta)));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.print != PLUM_DEFAULT_PRINT {
            let print = keywords(&PLUM_PRINT, &self.print);
            if print.is_empty() {
                return Err("choose at least one table of the output".into());
            }
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let save = keywords(&PROBABILITY_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

/// The reference category of NOMREG.
pub const NOMREG_BASES: [&str; 3] = ["Last", "First", "Custom"];

/// The models of NOMREG.
pub const NOMREG_MODELS: [&str; 3] = ["Main effects", "Full factorial", "Custom"];

/// Keyword and label of every table on /PRINT of NOMREG.
pub const NOMREG_PRINT: [(&str, &str); 11] = [
    ("CPS", "Case processing summary"),
    ("MFI", "Model fitting information"),
    ("IC", "Information criteria"),
    ("SUMMARY", "Pseudo R-square"),
    ("FIT", "Goodness of fit"),
    ("LRT", "Likelihood ratio tests"),
    ("PARAMETER", "Parameter estimates"),
    ("CLASSTABLE", "Classification table"),
    ("COVB", "Covariance of the estimates"),
    ("CORB", "Correlation of the estimates"),
    ("KERNEL", "Kernel of the log-likelihood"),
];

/// The tables NOMREG shows without /PRINT.
const NOMREG_DEFAULT_PRINT: [bool; 11] = [
    true, true, false, false, false, false, false, false, false, false, false,
];

/// NOMREG: a nominal dependent variable on factors and covariates.
#[derive(Clone, PartialEq, Debug)]
pub struct Multinomial {
    pub dependent: String,
    /// Index into `NOMREG_BASES`.
    pub base: usize,
    pub base_value: String,
    /// The categories in descending order, so First and Last swap.
    pub descending: bool,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `NOMREG_MODELS`.
    pub model: usize,
    pub terms: Terms,
    pub intercept: bool,
    /// One flag per entry of `NOMREG_PRINT`.
    pub print: [bool; 11],
    /// One flag per entry of `PROBABILITY_SAVE`.
    pub save: [bool; 4],
    pub ci_level: String,
    pub iterations: String,
    pub halvings: String,
    pub include: bool,
}

impl Default for Multinomial {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            base: 0,
            base_value: String::new(),
            descending: false,
            factors: Vec::new(),
            covariates: Vec::new(),
            model: 0,
            terms: Terms::default(),
            intercept: true,
            print: NOMREG_DEFAULT_PRINT,
            save: [false; 4],
            ci_level: "95".into(),
            iterations: "100".into(),
            halvings: "5".into(),
            include: false,
        }
    }
}

impl Multinomial {
    pub fn model_variables(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    /// `string`: the dependent variable is a string, so a custom reference value is quoted.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let variables = model_variables(&self.dependent, &self.factors, &self.covariates)?;
        let mut options = Vec::new();
        match self.base {
            0 => {}
            1 => options.push("BASE=FIRST".to_string()),
            _ => {
                let base = self.base_value.trim();
                if base.is_empty() {
                    return Err("give the value of the reference category".into());
                }
                options.push(format!("BASE={}", value(base, string)?));
            }
        }
        if self.descending {
            options.push("ORDER=DESCENDING".into());
        }
        let mut text = match variables.split_once(' ') {
            _ if options.is_empty() => format!("NOMREG {variables}"),
            Some((dependent, rest)) => {
                format!("NOMREG {dependent} ({}) {rest}", options.join(" "))
            }
            None => format!("NOMREG {variables} ({})", options.join(" ")),
        };
        match self.model {
            0 => {}
            1 => text.push_str("\n  /FULLFACTORIAL"),
            _ => {
                let terms = self.terms.syntax(&self.model_variables(), "model")?;
                text.push_str(&format!("\n  /MODEL={terms}"));
            }
        }
        if !self.intercept {
            text.push_str("\n  /INTERCEPT=EXCLUDE");
        }
        let criteria = iteration_criteria(&self.ci_level, &self.iterations, &self.halvings, 5, 1)?;
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.print != NOMREG_DEFAULT_PRINT {
            let print = keywords(&NOMREG_PRINT, &self.print);
            let print = if print.is_empty() {
                "NONE".to_string()
            } else {
                print.join(" ")
            };
            text.push_str(&format!("\n  /PRINT={print}"));
        }
        let save = keywords(&PROBABILITY_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

/// The covariance structures of random effects, keyword and label; the first is the default.
pub const RANDOM_COVARIANCES: [(&str, &str); 6] = [
    ("VC", "Variance components"),
    ("ID", "Scaled identity"),
    ("DIAG", "Diagonal"),
    ("UN", "Unstructured"),
    ("CS", "Compound symmetry"),
    ("AR1", "First-order autoregressive"),
];

/// The covariance structures of repeated measures; the first is the default.
pub const REPEATED_COVARIANCES: [(&str, &str); 5] = [
    ("DIAG", "Diagonal"),
    ("ID", "Scaled identity"),
    ("CS", "Compound symmetry"),
    ("AR1", "First-order autoregressive"),
    ("UN", "Unstructured"),
];

/// Keyword and label of every variable MIXED saves.
pub const MIXED_SAVE: [(&str, &str); 5] = [
    ("FIXPRED", "Fixed predicted values"),
    ("SEFIXP", "Their standard errors"),
    ("DFFIXP", "Their degrees of freedom"),
    ("PRED", "Predicted values"),
    ("RESID", "Residuals"),
];

/// MIXED: a linear mixed model of one dependent variable.
#[derive(Clone, PartialEq, Debug)]
pub struct LinearMixed {
    /// The variables that tell the subjects apart.
    pub subjects: Vec<String>,
    /// The variables that tell the repeated measurements of a subject apart.
    pub repeated: Vec<String>,
    /// Index into `REPEATED_COVARIANCES`.
    pub repeated_covariance: usize,
    pub dependent: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// The fixed terms of `fixed` instead of the main effects.
    pub custom_fixed: bool,
    pub fixed: Terms,
    pub intercept: bool,
    pub random: bool,
    pub random_intercept: bool,
    pub random_terms: Terms,
    /// The subject variables the random effects vary over, combined.
    pub random_subjects: Vec<String>,
    /// Index into `RANDOM_COVARIANCES`.
    pub random_covariance: usize,
    /// Maximum likelihood instead of restricted maximum likelihood.
    pub ml: bool,
    pub solution: bool,
    pub covariance_tests: bool,
    /// One flag per entry of `MIXED_SAVE`.
    pub save: [bool; 5],
    pub ci_level: String,
    pub iterations: String,
    pub halvings: String,
    /// The residual degrees of freedom instead of Satterthwaite's.
    pub residual_df: bool,
    pub include: bool,
}

impl Default for LinearMixed {
    fn default() -> Self {
        Self {
            subjects: Vec::new(),
            repeated: Vec::new(),
            repeated_covariance: 0,
            dependent: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            custom_fixed: false,
            fixed: Terms::default(),
            intercept: true,
            random: false,
            random_intercept: true,
            random_terms: Terms::default(),
            random_subjects: Vec::new(),
            random_covariance: 0,
            ml: false,
            solution: false,
            covariance_tests: false,
            save: [false; 5],
            ci_level: "95".into(),
            iterations: "100".into(),
            halvings: "10".into(),
            residual_df: false,
            include: false,
        }
    }
}

impl LinearMixed {
    /// Unchecks the random-effect subjects taken out of Subjects, so the dialog shows what the
    /// syntax uses.
    pub fn forget_old_subjects(&mut self) {
        let subjects = &self.subjects;
        self.random_subjects.retain(|s| subjects.contains(s));
    }

    pub fn model(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    pub fn syntax(&self) -> Result<String, String> {
        let variables = model_variables(&self.dependent, &self.factors, &self.covariates)?;
        let dependent = self.dependent.trim();
        let mut design = self.subjects.iter().chain(&self.repeated);
        if let Some(n) = design.find(|n| n.as_str() == dependent) {
            return Err(format!("{n} is the dependent variable"));
        }
        if let Some(n) = self.repeated.iter().find(|n| self.subjects.contains(n)) {
            return Err(format!("{n} is both a subject and a repeated variable"));
        }
        let model = self.model();
        let mut text = format!("MIXED {variables}");
        // custom with no terms is the intercept alone
        let fixed = if self.custom_fixed && !self.fixed.terms.is_empty() {
            self.fixed.syntax(&model, "fixed effects")?
        } else if self.custom_fixed {
            String::new()
        } else {
            model.join(" ")
        };
        match (fixed.is_empty(), self.intercept) {
            (true, false) => {
                return Err("the fixed effects need a term or the intercept".into());
            }
            (true, true) => {}
            (false, true) => text.push_str(&format!("\n  /FIXED={fixed}")),
            (false, false) => text.push_str(&format!("\n  /FIXED={fixed} | NOINT")),
        }
        if self.random {
            let mut effects = Vec::new();
            if self.random_intercept {
                effects.push("INTERCEPT".to_string());
            }
            if !self.random_terms.terms.is_empty() {
                effects.push(self.random_terms.syntax(&model, "random effects")?);
            }
            if effects.is_empty() {
                return Err("the random effects need the intercept or a term".into());
            }
            let mut options = Vec::new();
            if let Some(n) = self
                .random_subjects
                .iter()
                .find(|s| !self.subjects.contains(s))
            {
                return Err(format!("{n} in the random effects is no longer a subject"));
            }
            let subjects: Vec<&str> = self
                .subjects
                .iter()
                .filter(|s| self.random_subjects.contains(s))
                .map(String::as_str)
                .collect();
            if !subjects.is_empty() {
                options.push(format!("SUBJECT({})", subjects.join("*")));
            }
            if self.random_covariance > 0 {
                let covariance = RANDOM_COVARIANCES[self.random_covariance].0;
                options.push(format!("COVTYPE({covariance})"));
            }
            text.push_str(&format!("\n  /RANDOM={}", effects.join(" ")));
            if !options.is_empty() {
                text.push_str(&format!(" | {}", options.join(" ")));
            }
        }
        if !self.repeated.is_empty() {
            if self.subjects.is_empty() {
                return Err("repeated measures need the subject variables".into());
            }
            text.push_str(&format!(
                "\n  /REPEATED={} | SUBJECT({})",
                self.repeated.join("*"),
                self.subjects.join("*")
            ));
            if self.repeated_covariance > 0 {
                let covariance = REPEATED_COVARIANCES[self.repeated_covariance].0;
                text.push_str(&format!(" COVTYPE({covariance})"));
            }
        }
        let used = !self.repeated.is_empty()
            || (self.random
                && self
                    .subjects
                    .iter()
                    .all(|s| self.random_subjects.contains(s)));
        if !self.subjects.is_empty() && !used {
            return Err(
                "tick every subject under the random effects, or add repeated variables".into(),
            );
        }
        if self.ml {
            text.push_str("\n  /METHOD=ML");
        }
        let mut criteria =
            // MIXED takes 0 iterations: the starting estimates only
            iteration_criteria(&self.ci_level, &self.iterations, &self.halvings, 10, 0)?;
        if self.residual_df {
            criteria.push("DFMETHOD(RESIDUAL)".into());
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        let print: Vec<&str> = [
            (self.solution, "SOLUTION"),
            (self.covariance_tests, "TESTCOV"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let save = keywords(&MIXED_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

/// The estimators of a stage offered in the plan dialog, keyword and label.
pub const STAGE_ESTIMATORS: [(&str, &str); 2] = [
    ("WR", "With replacement"),
    ("EQUAL_WOR", "Equal probability, without replacement"),
];

/// What a stage drawn without replacement is given by.
pub const STAGE_RATES: [(&str, &str); 2] = [
    ("POPSIZE", "Population size"),
    ("INCLPROB", "Inclusion probability"),
];

/// One stage of a sample plan.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Stage {
    pub label: String,
    pub strata: Vec<String>,
    pub clusters: Vec<String>,
    /// Index into `STAGE_ESTIMATORS`.
    pub estimator: usize,
    /// Index into `STAGE_RATES`.
    pub rate: usize,
    /// The rate is in a variable instead of one value.
    pub rate_by_variable: bool,
    pub rate_value: String,
    pub rate_variable: String,
}

/// A plan has at most this many stages.
pub const MOST_STAGES: usize = 3;

/// CSPLAN ANALYSIS: the design of a complex sample, written to a plan file.
#[derive(Clone, PartialEq, Debug)]
pub struct SamplePlan {
    pub path: String,
    pub weight: String,
    pub stages: Vec<Stage>,
    /// Design effects compare with simple random sampling with replacement.
    pub srs_with_replacement: bool,
}

impl Default for SamplePlan {
    fn default() -> Self {
        Self {
            path: String::new(),
            weight: String::new(),
            stages: vec![Stage::default()],
            srs_with_replacement: false,
        }
    }
}

impl SamplePlan {
    /// The weight and the variables that hold population sizes or inclusion probabilities.
    pub fn numeric(&self) -> impl Iterator<Item = &String> {
        let rates = self
            .stages
            .iter()
            .filter(|s| s.estimator == 1 && s.rate_by_variable)
            .map(|s| &s.rate_variable);
        std::iter::once(&self.weight).chain(rates)
    }

    /// Another stage can follow the last one only when that one is drawn without replacement.
    pub fn can_add_stage(&self) -> bool {
        self.stages.len() < MOST_STAGES && self.stages.last().is_none_or(|s| s.estimator == 1)
    }

    pub fn syntax(&self) -> Result<String, String> {
        let path = self.path.trim();
        if path.is_empty() {
            return Err("give the plan file".into());
        }
        let weight = self.weight.trim();
        if weight.is_empty() {
            return Err("choose the sample weight variable".into());
        }
        // the other Complex Samples dialogs list .json files only
        let path = match std::path::Path::new(path).extension() {
            Some(_) => path.to_string(),
            None => format!("{path}.json"),
        };
        let mut text = format!(
            "CSPLAN ANALYSIS\n  /PLAN FILE={}\n  /PLANVARS ANALYSISWEIGHT={weight}",
            quote(&path)
        );
        if self.srs_with_replacement {
            text.push_str("\n  /SRSESTIMATOR TYPE=WR");
        }
        // a variable has one role in the plan
        let mut roles: Vec<&str> = vec![weight];
        let last = self.stages.len().saturating_sub(1);
        for (i, stage) in self.stages.iter().enumerate() {
            let stage_number = i + 1;
            // the analysis ignores every stage after one drawn with replacement
            if i < last && stage.estimator == 0 {
                return Err(format!(
                    "stage {stage_number} is drawn with replacement, which ends the design: remove the stages after it"
                ));
            }
            if i < last && stage.clusters.is_empty() {
                return Err(format!(
                    "stage {stage_number} is followed by another stage, so it needs clusters"
                ));
            }
            let rate_variable = stage.rate_variable.trim();
            let mut used: Vec<&str> = stage
                .strata
                .iter()
                .chain(&stage.clusters)
                .map(String::as_str)
                .collect();
            let without = stage.estimator == 1;
            if without && stage.rate_by_variable && !rate_variable.is_empty() {
                used.push(rate_variable);
            }
            for name in used {
                if roles.contains(&name) {
                    return Err(format!("{name} has more than one role in the plan"));
                }
                roles.push(name);
            }
            text.push_str("\n  /DESIGN");
            let label = stage.label.trim();
            if !label.is_empty() {
                text.push_str(&format!(" STAGELABEL={}", quote(label)));
            }
            if !stage.strata.is_empty() {
                text.push_str(&format!(" STRATA={}", stage.strata.join(" ")));
            }
            if !stage.clusters.is_empty() {
                text.push_str(&format!(" CLUSTER={}", stage.clusters.join(" ")));
            }
            text.push_str(&format!(
                "\n  /ESTIMATOR TYPE={}",
                STAGE_ESTIMATORS[stage.estimator].0
            ));
            if without {
                let (keyword, what) = STAGE_RATES[stage.rate];
                let what = what.to_lowercase();
                let rate = if stage.rate_by_variable {
                    if rate_variable.is_empty() {
                        return Err(format!(
                            "choose the {what} variable of stage {stage_number}"
                        ));
                    }
                    format!("VARIABLE={rate_variable}")
                } else {
                    let n = parse(
                        &stage.rate_value,
                        &format!("the {what} of stage {stage_number}"),
                    )?;
                    let ok = if stage.rate == 0 {
                        n >= 1.0 && n.fract() == 0.0
                    } else {
                        n > 0.0 && n <= 1.0
                    };
                    if !ok {
                        return Err(if stage.rate == 0 {
                            format!(
                                "the population size of stage {stage_number} must be a whole number of at least 1"
                            )
                        } else {
                            format!(
                                "the inclusion probability of stage {stage_number} must be above 0 and at most 1"
                            )
                        });
                    }
                    format!("VALUE={}", number(n))
                };
                text.push_str(&format!("\n  /{keyword} {rate}"));
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// The plan file a complex samples analysis reads, as `/PLAN FILE='...'`.
fn plan_file(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("choose the plan file".into());
    }
    // the same name as the Analysis Plan dialog writes it
    let path = match std::path::Path::new(path).extension() {
        Some(_) => path.to_string(),
        None => format!("{path}.json"),
    };
    Ok(format!("\n  /PLAN FILE={}", quote(&path)))
}

/// The statistics of both complex samples analyses, keyword and label; the first is the
/// default.
pub const SAMPLE_STATISTICS: [(&str, &str); 5] = [
    ("SE", "Standard error"),
    ("CV", "Coefficient of variation"),
    ("DEFF", "Design effect"),
    ("DEFFSQRT", "Square root of the design effect"),
    ("COUNT", "Unweighted count"),
];

/// The statistics, subpopulations and missing values both complex samples analyses share.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleOptions {
    /// One flag per entry of `SAMPLE_STATISTICS`.
    pub statistics: [bool; 5],
    pub ci: bool,
    pub ci_level: String,
    pub subpopulations: Vec<String>,
    /// Each subpopulation variable in a table of its own.
    pub separate: bool,
    /// The cases valid on every variable instead of each statistic's own.
    pub listwise: bool,
    /// User-missing values of the category variables count as valid.
    pub include: bool,
}

impl Default for SampleOptions {
    fn default() -> Self {
        Self {
            statistics: [true, false, false, false, false],
            ci: false,
            ci_level: "95".into(),
            subpopulations: Vec::new(),
            separate: false,
            listwise: false,
            include: false,
        }
    }
}

impl SampleOptions {
    /// `/STATISTICS` with `more`, unless it is the standard error alone; `/TEST` of `tests`;
    /// `/SUBPOP` and `/MISSING`.
    fn syntax(&self, text: &mut String, more: Vec<&str>, tests: Vec<&str>) -> Result<(), String> {
        let mut statistics: Vec<String> = keywords(&SAMPLE_STATISTICS, &self.statistics)
            .into_iter()
            .map(str::to_string)
            .collect();
        if self.ci {
            // an empty field means the default, as in the other dialogs here
            let level = match self.ci_level.trim() {
                "" => 95.0,
                given => percent(given)?,
            };
            statistics.push(if level == 95.0 {
                "CIN".into()
            } else {
                format!("CIN({})", number(level))
            });
        }
        statistics.extend(more.into_iter().map(str::to_string));
        if statistics.is_empty() {
            return Err("choose at least one statistic".into());
        }
        if statistics != ["SE"] {
            text.push_str(&format!("\n  /STATISTICS {}", statistics.join(" ")));
        }
        if !tests.is_empty() {
            text.push_str(&format!("\n  /TEST {}", tests.join(" ")));
        }
        if !self.subpopulations.is_empty() {
            text.push_str(&format!(
                "\n  /SUBPOP TABLE={}",
                self.subpopulations.join(" BY ")
            ));
            if self.separate {
                text.push_str(" DISPLAY=SEPARATE");
            }
        }
        let mut missing = Vec::new();
        if self.listwise {
            missing.push("SCOPE=LISTWISE");
        }
        if self.include {
            missing.push("CLASSMISSING=INCLUDE");
        }
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING {}", missing.join(" ")));
        }
        Ok(())
    }
}

/// CSDESCRIPTIVES: means, sums and ratios of a complex sample.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleDescriptives {
    pub plan: String,
    pub summary: Vec<String>,
    pub mean: bool,
    pub sum: bool,
    pub numerators: Vec<String>,
    pub denominators: Vec<String>,
    pub population_size: bool,
    pub options: SampleOptions,
}

impl Default for SampleDescriptives {
    fn default() -> Self {
        Self {
            plan: String::new(),
            summary: Vec::new(),
            mean: true,
            sum: false,
            numerators: Vec::new(),
            denominators: Vec::new(),
            population_size: false,
            options: SampleOptions::default(),
        }
    }
}

impl SampleDescriptives {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("CSDESCRIPTIVES{}", plan_file(&self.plan)?);
        let ratio = !self.numerators.is_empty() || !self.denominators.is_empty();
        if !self.summary.is_empty() {
            if !self.mean && !self.sum {
                return Err("choose Mean or Sum for the measures".into());
            }
            text.push_str(&format!(
                "\n  /SUMMARY VARIABLES={}",
                self.summary.join(" ")
            ));
            if self.mean {
                text.push_str("\n  /MEAN");
            }
            if self.sum {
                text.push_str("\n  /SUM");
            }
        } else if !ratio {
            return Err("add measures, or numerators and denominators of a ratio".into());
        }
        if ratio {
            let numerators = names(&self.numerators, "Numerators")?;
            let denominators = names(&self.denominators, "Denominators")?;
            text.push_str(&format!(
                "\n  /RATIO NUMERATOR={numerators} DENOMINATOR={denominators}"
            ));
        }
        let more = if self.population_size {
            vec!["POPSIZE"]
        } else {
            Vec::new()
        };
        self.options.syntax(&mut text, more, Vec::new())?;
        text.push('.');
        Ok(text)
    }
}

/// The cells of CSTABULATE, keyword and label: the first two apply to one-way tables too.
pub const SAMPLE_CELLS: [(&str, &str); 4] = [
    ("POPSIZE", "Population size"),
    ("TABLEPCT", "Table percent"),
    ("ROWPCT", "Row percent"),
    ("COLPCT", "Column percent"),
];

/// The statistics of two-way tables with table percents.
pub const SAMPLE_RESIDUALS: [(&str, &str); 3] = [
    ("EXPECTED", "Expected table percent"),
    ("RESID", "Residual"),
    ("ASRESID", "Adjusted residual"),
];

/// The tests of two-way tables.
pub const SAMPLE_TESTS: [(&str, &str); 4] = [
    ("INDEPENDENCE", "Independence"),
    ("ODDSRATIO", "Odds ratio"),
    ("RELRISK", "Relative risk"),
    ("RISKDIFF", "Risk difference"),
];

/// CSTABULATE: one-way tables (Frequencies) or two-way tables (Crosstabs) of a complex sample.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleTables {
    pub crosstabs: bool,
    pub plan: String,
    /// The variables of one-way tables, or the rows of two-way tables.
    pub variables: Vec<String>,
    pub column: String,
    /// One flag per entry of `SAMPLE_CELLS`.
    pub cells: [bool; 4],
    /// One-way: cumulative sizes and percents.
    pub cumulative: bool,
    /// Two-way: one flag per entry of `SAMPLE_RESIDUALS`.
    pub residuals: [bool; 3],
    /// One-way: the test of equal shares.
    pub homogeneity: bool,
    /// Two-way: one flag per entry of `SAMPLE_TESTS`.
    pub tests: [bool; 4],
    pub options: SampleOptions,
}

impl Default for SampleTables {
    fn default() -> Self {
        Self {
            crosstabs: false,
            plan: String::new(),
            variables: Vec::new(),
            column: String::new(),
            cells: [true, false, false, false],
            cumulative: false,
            residuals: [false; 3],
            homogeneity: false,
            tests: [false; 4],
            options: SampleOptions::default(),
        }
    }
}

impl SampleTables {
    /// The cells that apply to the kind of table.
    pub fn cell_options(&self) -> &'static [(&'static str, &'static str)] {
        if self.crosstabs {
            &SAMPLE_CELLS
        } else {
            &SAMPLE_CELLS[..2]
        }
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("CSTABULATE{}", plan_file(&self.plan)?);
        let what = if self.crosstabs { "Rows" } else { "Variables" };
        let mut tables = format!("\n  /TABLES VARIABLES={}", names(&self.variables, what)?);
        let column = self.column.trim();
        if self.crosstabs {
            if column.is_empty() {
                return Err("choose the column variable".into());
            }
            if self.variables.iter().any(|n| n == column) {
                return Err(format!("{column} is the column variable"));
            }
            tables.push_str(&format!(" BY {column}"));
        }
        text.push_str(&tables);
        let cells = &self.cells[..self.cell_options().len()];
        if cells != &[true, false, false, false][..cells.len()] {
            let cells = keywords(&SAMPLE_CELLS, cells);
            if cells.is_empty() {
                return Err("choose at least one cell".into());
            }
            text.push_str(&format!("\n  /CELLS {}", cells.join(" ")));
        }
        let mut more = Vec::new();
        let mut tests = Vec::new();
        if self.crosstabs {
            // the residuals compare table percents
            more = keywords(&SAMPLE_RESIDUALS, &self.residuals);
            if !more.is_empty() && !self.cells[1] {
                return Err("the residuals compare table percents: tick Table percent".into());
            }
            tests = keywords(&SAMPLE_TESTS, &self.tests);
        } else {
            if self.cumulative {
                more.push("CUMULATIVE");
            }
            if self.homogeneity {
                tests.push("HOMOGENEITY");
            }
        }
        self.options.syntax(&mut text, more, tests)?;
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, runs, v};

    /// Terms of single variables and of interactions.
    fn terms(list: &[&[&str]]) -> Terms {
        Terms {
            terms: list.iter().map(|t| v(t)).collect(),
            building: Vec::new(),
        }
    }

    #[test]
    fn terms_are_added_once_in_the_order_of_the_model() {
        let model = v(&["region", "gender", "age"]);
        let mut t = Terms {
            building: v(&["age", "region"]),
            ..Default::default()
        };
        t.add(&model, false);
        t.building = v(&["gender", "region"]);
        t.add(&model, true);
        t.building = v(&["age"]);
        t.add(&model, false);
        assert_eq!(
            t.terms,
            vec![v(&["region"]), v(&["age"]), v(&["region", "gender"])]
        );
        assert!(t.building.is_empty());
        assert_eq!(
            t.syntax(&model, "model").unwrap(),
            "region age region*gender"
        );
        assert_eq!(
            t.syntax(&model[..2], "model").unwrap_err(),
            "age in the model is no longer a factor or covariate"
        );
        assert_eq!(
            Terms::default().syntax(&model, "model").unwrap_err(),
            "add at least one term to the model"
        );
        // an interaction whose names left the model adds nothing, never an empty term
        t.building = v(&["gender", "weight"]);
        t.add(&model, true);
        t.building = v(&["weight", "income"]);
        t.add(&model, true);
        assert_eq!(t.terms.len(), 3);
        let empty = Terms {
            terms: vec![Vec::new()],
            ..Default::default()
        };
        assert_eq!(
            empty.syntax(&model, "model").unwrap_err(),
            "an empty term in the model: remove it"
        );
    }

    #[test]
    fn ordinal_regression() {
        let mut d = Ordinal::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the dependent variable");
        d.dependent = "q1_satisfaction".into();
        d.factors = v(&["gender"]);
        d.covariates = v(&["age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "PLUM q1_satisfaction BY gender WITH age.");
        assert!(runs(&syntax) >= 4);

        // a link, custom location and scale, every table and saved variable, the options
        d.link = 2;
        d.custom_location = true;
        d.location = terms(&[&["gender"], &["age"], &["gender", "age"]]);
        d.scale = true;
        d.scale_terms = terms(&[&["gender"]]);
        d.print = [true; 9];
        d.save = [true; 4];
        d.ci_level = "90".into();
        d.iterations = "200".into();
        d.halvings = "8".into();
        d.delta = "0.1".into();
        d.include = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PLUM q1_satisfaction BY gender WITH age\n  /LINK=CLOGLOG\n  \
             /LOCATION=gender age gender*age\n  /SCALE=gender\n  \
             /CRITERIA=CIN(90) MXITER(200) MXSTEP(8) DELTA(0.1)\n  \
             /PRINT=FIT SUMMARY PARAMETER TPARALLEL CELLINFO COVB CORB HISTORY KERNEL\n  \
             /SAVE=ESTPROB PREDCAT PCPROB ACPROB\n  /MISSING=INCLUDE."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 6);
        let data = session.dataset().unwrap();
        assert!(data.variable("EST1_1").is_ok());
        assert!(data.variable("PRE_1").is_ok());

        // fewer tables than the default; nothing left is an error
        d = Ordinal {
            dependent: "q1_satisfaction".into(),
            factors: v(&["gender"]),
            print: [false, false, true, false, false, false, false, false, false],
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PLUM q1_satisfaction BY gender\n  /PRINT=PARAMETER."
        );
        assert!(runs(&syntax) >= 2);
        d.print = [false; 9];
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose at least one table of the output"
        );
        d.print = PLUM_DEFAULT_PRINT;
        d.scale = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one term to the scale model"
        );
        d.scale = false;
        d.delta = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "delta must be at least 0 and below 1"
        );
        d.delta = "0".into();
        d.covariates = v(&["gender"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "gender is both a factor and a covariate"
        );
        d.covariates = v(&["q1_satisfaction"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "q1_satisfaction is the dependent variable"
        );
    }

    #[test]
    fn multinomial_logistic_regression() {
        let mut d = Multinomial {
            dependent: "region".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            ..Default::default()
        };
        let syntax = d.syntax(false).unwrap();
        assert_eq!(syntax, "NOMREG region BY gender WITH age.");
        assert!(runs(&syntax) >= 2);

        // the first category as reference in descending order, the full factorial, every
        // table, every saved variable, the options
        d.base = 1;
        d.descending = true;
        d.model = 1;
        d.print = [true; 11];
        d.save = [true; 4];
        d.ci_level = "99".into();
        d.iterations = "50".into();
        d.halvings = "10".into();
        d.include = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "NOMREG region (BASE=FIRST ORDER=DESCENDING) BY gender WITH age\n  \
             /FULLFACTORIAL\n  /CRITERIA=CIN(99) MXITER(50) MXSTEP(10)\n  \
             /PRINT=CPS MFI IC SUMMARY FIT LRT PARAMETER CLASSTABLE COVB CORB KERNEL\n  \
             /SAVE=ESTPROB PREDCAT PCPROB ACPROB\n  /MISSING=INCLUDE."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 7);
        let data = session.dataset().unwrap();
        assert!(data.variable("EST1_1").is_ok());

        // a string dependent with a custom reference value, a custom model without intercept,
        // no tables
        let mut d = Multinomial {
            dependent: "plan".into(),
            base: 2,
            base_value: "BASIC".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            model: 2,
            terms: terms(&[&["gender"], &["age"]]),
            intercept: false,
            print: [false; 11],
            ..Default::default()
        };
        let syntax = d.syntax(true).unwrap();
        assert_eq!(
            syntax,
            "NOMREG plan (BASE='BASIC') BY gender WITH age\n  /MODEL=gender age\n  \
             /INTERCEPT=EXCLUDE\n  /PRINT=NONE."
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{errors:?}");
        open("survey_basic.sav").run(&syntax, None).unwrap();
        // a dependent alone takes its options last
        d.factors.clear();
        d.covariates.clear();
        d.model = 0;
        d.intercept = true;
        assert_eq!(
            d.syntax(true).unwrap(),
            "NOMREG plan (BASE='BASIC')\n  /PRINT=NONE."
        );
        d.base_value.clear();
        assert_eq!(
            d.syntax(true).unwrap_err(),
            "give the value of the reference category"
        );
        d.base_value = "x".into();
        assert_eq!(d.syntax(false).unwrap_err(), "\"x\" is not a number");
    }

    #[test]
    fn linear_mixed_models() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MIXED monthly_spend.");
        assert!(runs(&syntax) >= 2);

        // main effects with a random intercept and slope per region, every option
        d.subjects = v(&["region"]);
        d.factors = v(&["gender"]);
        d.covariates = v(&["age"]);
        d.random = true;
        d.random_terms = terms(&[&["age"]]);
        d.random_subjects = v(&["region"]);
        d.random_covariance = 3;
        d.ml = true;
        d.solution = true;
        d.covariance_tests = true;
        d.save = [true; 5];
        d.ci_level = "90".into();
        d.iterations = "150".into();
        d.halvings = "5".into();
        d.residual_df = true;
        d.include = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MIXED monthly_spend BY gender WITH age\n  /FIXED=gender age\n  \
             /RANDOM=INTERCEPT age | SUBJECT(region) COVTYPE(UN)\n  /METHOD=ML\n  \
             /CRITERIA=CIN(90) MXITER(150) MXSTEP(5) DFMETHOD(RESIDUAL)\n  \
             /PRINT=SOLUTION TESTCOV\n  /SAVE=FIXPRED SEFIXP DFFIXP PRED RESID\n  \
             /MISSING=INCLUDE."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 5);
        let data = session.dataset().unwrap();
        assert!(data.variable("FXPRED_1").is_ok());

        // repeated measures in long form: the three mentions of each respondent
        let mut session = open("survey_basic.sav");
        session
            .run(
                "VARSTOCASES /MAKE mention FROM q6_mention1 q6_mention2 q6_mention3 \
                 /INDEX=which /KEEP=resp_id gender.",
                None,
            )
            .unwrap();
        let d = LinearMixed {
            subjects: v(&["resp_id"]),
            repeated: v(&["which"]),
            repeated_covariance: 2,
            dependent: "mention".into(),
            factors: v(&["which"]),
            custom_fixed: true,
            fixed: terms(&[&["which"]]),
            intercept: false,
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MIXED mention BY which\n  /FIXED=which | NOINT\n  \
             /REPEATED=which | SUBJECT(resp_id) COVTYPE(CS)."
        );
        assert!(run_in(&mut session, &syntax) >= 3);

        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            intercept: false,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "the fixed effects need a term or the intercept"
        );
        d.intercept = true;
        d.random = true;
        d.random_intercept = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the random effects need the intercept or a term"
        );
        d.random = false;
        d.repeated = v(&["region"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "repeated measures need the subject variables"
        );
        d.subjects = v(&["region"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "region is both a subject and a repeated variable"
        );
        d.subjects = v(&["monthly_spend"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "monthly_spend is the dependent variable"
        );
    }

    /// A folder of its own for the plan files a test writes.
    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("statscraft-gui-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn complex_samples() {
        let out = folder("complex-samples");
        let path = out.join("survey_plan.json").to_string_lossy().into_owned();
        let mut plan = SamplePlan::default();
        assert_eq!(plan.syntax().unwrap_err(), "give the plan file");
        plan.path = path.clone();
        assert_eq!(
            plan.syntax().unwrap_err(),
            "choose the sample weight variable"
        );
        plan.weight = "weight".into();
        plan.stages[0].strata = v(&["region"]);
        let syntax = plan.syntax().unwrap();
        assert_eq!(
            syntax,
            format!(
                "CSPLAN ANALYSIS\n  /PLAN FILE={}\n  /PLANVARS ANALYSISWEIGHT=weight\n  \
                 /DESIGN STRATA=region\n  /ESTIMATOR TYPE=WR.",
                quote(&path)
            )
        );
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);

        let mut d = SampleDescriptives::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the plan file");
        d.plan = path.clone();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add measures, or numerators and denominators of a ratio"
        );
        d.summary = v(&["monthly_spend", "age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            format!(
                "CSDESCRIPTIVES\n  /PLAN FILE={}\n  /SUMMARY VARIABLES=monthly_spend age\n  /MEAN.",
                quote(&path)
            )
        );
        assert!(run_in(&mut session, &syntax) >= 1);

        // sums and a ratio, every statistic, subpopulations in tables of their own, listwise
        d.sum = true;
        d.numerators = v(&["monthly_spend"]);
        d.denominators = v(&["age"]);
        d.population_size = true;
        d.options = SampleOptions {
            statistics: [true; 5],
            ci: true,
            ci_level: "90".into(),
            subpopulations: v(&["gender", "region"]),
            separate: true,
            listwise: true,
            include: true,
        };
        let syntax = d.syntax().unwrap();
        assert!(
            syntax.ends_with(
                "/SUMMARY VARIABLES=monthly_spend age\n  /MEAN\n  /SUM\n  \
                 /RATIO NUMERATOR=monthly_spend DENOMINATOR=age\n  \
                 /STATISTICS SE CV DEFF DEFFSQRT COUNT CIN(90) POPSIZE\n  \
                 /SUBPOP TABLE=gender BY region DISPLAY=SEPARATE\n  \
                 /MISSING SCOPE=LISTWISE CLASSMISSING=INCLUDE."
            ),
            "{syntax}"
        );
        assert!(run_in(&mut session, &syntax) >= 3);
        d.denominators.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Denominators"
        );
        d.mean = false;
        d.sum = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose Mean or Sum for the measures"
        );

        // one-way tables
        let mut t = SampleTables {
            plan: path.clone(),
            ..Default::default()
        };
        assert_eq!(
            t.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        t.variables = v(&["gender", "q5_1"]);
        let syntax = t.syntax().unwrap();
        assert_eq!(
            syntax,
            format!(
                "CSTABULATE\n  /PLAN FILE={}\n  /TABLES VARIABLES=gender q5_1.",
                quote(&path)
            )
        );
        assert!(run_in(&mut session, &syntax) >= 2);
        t.cells = [true, true, false, false];
        t.cumulative = true;
        t.homogeneity = true;
        t.options.statistics = [true, false, true, false, false];
        t.options.ci = true;
        let syntax = t.syntax().unwrap();
        assert!(
            syntax.ends_with(
                "/TABLES VARIABLES=gender q5_1\n  /CELLS POPSIZE TABLEPCT\n  \
                 /STATISTICS SE DEFF CIN CUMULATIVE\n  /TEST HOMOGENEITY."
            ),
            "{syntax}"
        );
        assert!(run_in(&mut session, &syntax) >= 4);
        t.cells = [false; 4];
        assert_eq!(t.syntax().unwrap_err(), "choose at least one cell");
        t.cells = [true, false, false, false];
        t.options.statistics = [false; 5];
        t.options.ci = false;
        t.cumulative = false;
        assert_eq!(t.syntax().unwrap_err(), "choose at least one statistic");

        // two-way tables: residuals only with table percents
        let mut t = SampleTables {
            crosstabs: true,
            plan: path.clone(),
            variables: v(&["q5_1", "q5_2"]),
            ..Default::default()
        };
        assert_eq!(t.syntax().unwrap_err(), "choose the column variable");
        t.column = "gender".into();
        t.cells = [true, false, true, true];
        t.residuals = [true; 3];
        t.tests = [true; 4];
        assert_eq!(
            t.syntax().unwrap_err(),
            "the residuals compare table percents: tick Table percent"
        );
        t.residuals = [false; 3];
        let syntax = t.syntax().unwrap();
        assert!(
            syntax.ends_with(
                "/TABLES VARIABLES=q5_1 q5_2 BY gender\n  /CELLS POPSIZE ROWPCT COLPCT\n  \
                 /TEST INDEPENDENCE ODDSRATIO RELRISK RISKDIFF."
            ),
            "{syntax}"
        );
        t.column = "q5_3".into();
        t.cells[1] = true;
        t.residuals = [true; 3];
        let syntax = t.syntax().unwrap();
        assert!(
            syntax.contains(
                "/CELLS POPSIZE TABLEPCT ROWPCT COLPCT\n  \
                 /STATISTICS SE EXPECTED RESID ASRESID\n"
            ),
            "{syntax}"
        );
        assert!(run_in(&mut session, &syntax) >= 4);
        t.column = "q5_1".into();
        assert_eq!(t.syntax().unwrap_err(), "q5_1 is the column variable");

        // two stages, the first without replacement by a population size
        plan.stages[0].clusters = v(&["gender"]);
        plan.stages[0].estimator = 1;
        plan.stages[0].rate_value = "10".into();
        plan.stages.push(Stage {
            label: "Respondents".into(),
            ..Default::default()
        });
        plan.srs_with_replacement = true;
        let syntax = plan.syntax().unwrap();
        assert!(
            syntax.ends_with(
                "/PLANVARS ANALYSISWEIGHT=weight\n  /SRSESTIMATOR TYPE=WR\n  \
                 /DESIGN STRATA=region CLUSTER=gender\n  /ESTIMATOR TYPE=EQUAL_WOR\n  \
                 /POPSIZE VALUE=10\n  /DESIGN STAGELABEL='Respondents'\n  /ESTIMATOR TYPE=WR."
            ),
            "{syntax}"
        );
        run_in(&mut session, &syntax);
        let d = SampleDescriptives {
            plan: path.clone(),
            summary: v(&["age"]),
            ..Default::default()
        };
        assert!(run_in(&mut session, &d.syntax().unwrap()) >= 1);

        plan.stages[0].rate = 1;
        plan.stages[0].rate_value = "1.5".into();
        assert_eq!(
            plan.syntax().unwrap_err(),
            "the inclusion probability of stage 1 must be above 0 and at most 1"
        );
        plan.stages[0].rate_by_variable = true;
        assert_eq!(
            plan.syntax().unwrap_err(),
            "choose the inclusion probability variable of stage 1"
        );
        plan.stages[0].rate_variable = "region".into();
        assert_eq!(
            plan.syntax().unwrap_err(),
            "region has more than one role in the plan"
        );
        plan.stages[0].clusters.clear();
        assert_eq!(
            plan.syntax().unwrap_err(),
            "stage 1 is followed by another stage, so it needs clusters"
        );
        std::fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn random_subjects_taken_out_of_subjects_are_not_lost_silently() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            subjects: v(&["region"]),
            random: true,
            random_subjects: v(&["region", "gender"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "gender in the random effects is no longer a subject"
        );
        // the dialog unchecks it when Subjects changes
        d.forget_old_subjects();
        assert_eq!(d.random_subjects, v(&["region"]));
        assert_eq!(
            d.syntax().unwrap(),
            "MIXED monthly_spend\n  /RANDOM=INTERCEPT | SUBJECT(region)."
        );
    }

    #[test]
    fn custom_fixed_effects_without_terms_are_the_intercept_alone() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            factors: v(&["gender"]),
            custom_fixed: true,
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MIXED monthly_spend BY gender.");
        assert!(runs(&syntax) >= 2);
        d.intercept = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the fixed effects need a term or the intercept"
        );
    }

    #[test]
    fn plan_variables_must_be_numeric() {
        let session = open("survey_basic.sav");
        let data = session.dataset().unwrap();
        let mut d = SamplePlan {
            path: "plan.json".into(),
            weight: "plan".into(),
            ..Default::default()
        };
        let syntax = |d: &SamplePlan| Advanced::Plan(d.clone()).syntax(data);
        assert_eq!(
            syntax(&d).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
        d.weight = "weight".into();
        d.stages[0].estimator = 1;
        d.stages[0].rate_by_variable = true;
        d.stages[0].rate_variable = "q3_comment".into();
        assert_eq!(
            syntax(&d).unwrap_err(),
            "q3_comment is a string; the command needs numbers"
        );
        d.stages[0].rate_variable = "age".into();
        assert!(syntax(&d).unwrap().ends_with("/POPSIZE VARIABLE=age."));
    }

    #[test]
    fn a_stage_with_replacement_ends_the_design() {
        let mut d = SamplePlan {
            path: "plan.json".into(),
            weight: "weight".into(),
            ..Default::default()
        };
        // the last stage needs no clusters, and nothing may follow it
        assert!(
            d.syntax()
                .unwrap()
                .ends_with("/DESIGN\n  /ESTIMATOR TYPE=WR.")
        );
        assert!(!d.can_add_stage());
        d.stages[0].clusters = v(&["region"]);
        d.stages.push(Stage::default());
        assert_eq!(
            d.syntax().unwrap_err(),
            "stage 1 is drawn with replacement, which ends the design: remove the stages after it"
        );
        d.stages[0].estimator = 1;
        d.stages[0].rate_value = "10".into();
        assert!(d.syntax().is_ok());
        // the second stage is drawn with replacement, so a third needs it changed
        assert!(!d.can_add_stage());
        d.stages[1].estimator = 1;
        assert!(d.can_add_stage());
        d.stages.push(Stage::default());
        assert!(!d.can_add_stage());
    }

    #[test]
    fn an_empty_delta_is_the_default() {
        let d = Ordinal {
            dependent: "q1_satisfaction".into(),
            delta: " ".into(),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "PLUM q1_satisfaction.");
    }

    #[test]
    fn the_reference_value_is_trimmed() {
        let d = Multinomial {
            dependent: "plan".into(),
            base: 2,
            base_value: " BASIC ".into(),
            ..Default::default()
        };
        assert_eq!(d.syntax(true).unwrap(), "NOMREG plan (BASE='BASIC').");
    }

    #[test]
    fn subjects_that_nothing_uses_are_an_error() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            subjects: v(&["region"]),
            random: true,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "tick every subject under the random effects, or add repeated variables"
        );
        d.random_subjects = v(&["region"]);
        assert!(d.syntax().is_ok());
        d.subjects = v(&["region", "gender"]);
        assert!(d.syntax().is_err());
    }

    #[test]
    fn mixed_and_plum_take_zero_iterations() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            iterations: "0".into(),
            ..Default::default()
        };
        assert!(d.syntax().unwrap().contains("MXITER(0)"));
        d.iterations = "100".into();
        let o = Ordinal {
            dependent: "q1_satisfaction".into(),
            iterations: "0".into(),
            ..Default::default()
        };
        assert!(o.syntax().unwrap().contains("MXITER(0)"));
    }

    #[test]
    fn a_plan_file_without_extension_gets_json() {
        let d = SamplePlan {
            path: "design".into(),
            weight: "weight".into(),
            ..Default::default()
        };
        assert!(d.syntax().unwrap().contains("/PLAN FILE='design.json'"));
    }
}
