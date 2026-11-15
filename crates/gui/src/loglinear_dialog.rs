//! The Loglinear dialogs: Model Selection (HILOGLINEAR), and General and Logit (GENLOG). Options
//! left as they are add nothing to the syntax, so the command runs with its own defaults.

use crate::advanced_dialogs::Terms;
use crate::analyze_dialogs::keywords;
use crate::correspondence_dialog::TableVariable;
use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, parse};
use crate::survival_dialog::saved;
use crate::syntax_text::number;
use statscraft_data::Dataset;

/// The tables of HILOGLINEAR /PRINT.
pub const HILO_PRINT: [(&str, &str); 4] = [
    ("FREQ", "Observed and expected counts"),
    ("RESID", "Residuals"),
    ("ESTIM", "Parameter estimates (saturated model)"),
    ("ASSOCIATION", "Partial associations (saturated model)"),
];

/// The charts of HILOGLINEAR /PLOT.
pub const HILO_PLOTS: [(&str, &str); 2] = [
    ("RESID", "Residuals against observed and expected counts"),
    ("NORMPROB", "Normal plot of adjusted residuals"),
];

/// The most cells the engine fits.
const MOST_CELLS: i64 = 1_000_000;

/// HILOGLINEAR: a hierarchical model of the table of factors, each with its range.
#[derive(Clone, PartialEq, Debug)]
pub struct ModelSelection {
    pub factors: Vec<String>,
    /// The range of each factor, found by name; `roles` is not used.
    pub ranges: Vec<TableVariable>,
    /// Backward elimination instead of entering the model in one step.
    pub backward: bool,
    /// The highest order of the terms; empty for every order.
    pub maxorder: String,
    pub maxsteps: String,
    pub probability: String,
    pub iterations: String,
    /// Empty: the command's own criterion, which depends on the counts.
    pub converge: String,
    pub delta: String,
    pub cweight: String,
    /// PRINT names the tables checked; otherwise the default display.
    pub print_chosen: bool,
    /// One flag per entry of `HILO_PRINT`.
    pub print: [bool; 4],
    /// One flag per entry of `HILO_PLOTS`.
    pub plots: [bool; 2],
    pub include: bool,
    /// A generating class of custom terms instead of the saturated model.
    pub custom: bool,
    pub terms: Terms,
}

impl Default for ModelSelection {
    fn default() -> Self {
        Self {
            factors: Vec::new(),
            ranges: Vec::new(),
            backward: false,
            maxorder: String::new(),
            maxsteps: "10".into(),
            probability: "0.05".into(),
            iterations: "20".into(),
            converge: String::new(),
            delta: "0.5".into(),
            cweight: String::new(),
            print_chosen: false,
            print: [true, true, false, false],
            plots: [false; 2],
            include: false,
            custom: false,
            terms: Terms::default(),
        }
    }
}

/// A number typed in a field that may be left empty for the default.
fn optional(text: &str, what: &str) -> Result<Option<f64>, String> {
    match text.trim() {
        "" => Ok(None),
        t => parse(t, what).map(Some),
    }
}

/// A check of a criterion's value, and the end of the sentence that says what is valid.
type Rule = (fn(f64) -> bool, &'static str);

const POSITIVE: Rule = (|v| v > 0.0, "must be above 0");
const ITERATIONS: Rule = (
    |v| v >= 1.0 && v.fract() == 0.0,
    "must be a whole number of at least 1",
);

/// A criterion field: keyword, text, default, what it is called and its rule.
type Criterion<'a> = (&'a str, &'a str, f64, &'a str, Rule);

/// Each criterion checked, and added as `KEYWORD(value)` when it holds a value other than the
/// default; an empty field is the default.
fn criteria(fields: &[Criterion]) -> Result<String, String> {
    let mut out = Vec::new();
    for &(keyword, text, default, what, (valid, rule)) in fields {
        let Some(v) = optional(text, what)? else {
            continue;
        };
        if !valid(v) {
            return Err(format!("{what} {rule}"));
        }
        if v != default {
            out.push(format!("{keyword}({})", number(v)));
        }
    }
    Ok(list("CRITERIA", &out))
}

/// `/KEYWORD=a b` for a non-empty list.
fn list(keyword: &str, items: &[impl AsRef<str>]) -> String {
    match items.is_empty() {
        true => String::new(),
        false => {
            let items: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
            format!("\n  /{keyword}={}", items.join(" "))
        }
    }
}

impl ModelSelection {
    /// The range of a factor, as the view keeps it.
    pub fn range_of(&self, name: &str) -> Option<&TableVariable> {
        self.ranges.iter().find(|r| r.name == name)
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.factors.len() < 2 {
            return Err("add at least two factors".into());
        }
        if self.factors.len() > 10 {
            return Err("the command takes at most 10 factors".into());
        }
        let cweight = self.cweight.trim();
        let weight = [cweight.to_string()];
        numeric(data, self.factors.iter().chain(&weight))?;
        let mut ranges = Vec::new();
        for name in &self.factors {
            let range = match self.range_of(name) {
                Some(r) => r.range()?,
                None => return Err(format!("give the range of {name}")),
            };
            ranges.push(range);
        }
        let cells = ranges
            .iter()
            .try_fold(1i64, |a, (lo, hi)| a.checked_mul(hi - lo + 1))
            .filter(|c| *c <= MOST_CELLS);
        if cells.is_none() {
            return Err(format!(
                "the table would have more than {MOST_CELLS} cells; narrow the ranges"
            ));
        }
        let mut text = String::from("HILOGLINEAR");
        for (i, name) in self.factors.iter().enumerate() {
            text.push_str(&format!(" {name}"));
            // factors in a row with the same range share it
            let (lo, hi) = ranges[i];
            if ranges.get(i + 1) != Some(&(lo, hi)) {
                text.push_str(&format!("({lo},{hi})"));
            }
        }
        if self.backward {
            text.push_str("\n  /METHOD=BACKWARD");
        }
        if let Some(k) = whole(&self.maxorder, 1, "the highest order")? {
            text.push_str(&format!("\n  /MAXORDER={k}"));
        }
        let mut fields: Vec<Criterion> = Vec::new();
        // the stepping fields count only for backward elimination
        if self.backward {
            fields.push((
                "MAXSTEPS",
                &self.maxsteps,
                10.0,
                "the most steps",
                (
                    |v| (1.0..=99.0).contains(&v) && v.fract() == 0.0,
                    "must be a whole number from 1 to 99",
                ),
            ));
            fields.push((
                "P",
                &self.probability,
                0.05,
                "the probability to remove",
                (|v| v > 0.0 && v < 1.0, "must be above 0 and below 1"),
            ));
        }
        fields.extend([
            (
                "ITERATE",
                self.iterations.as_str(),
                20.0,
                "the maximum iterations",
                ITERATIONS,
            ),
            // no default to compare with: the command's own depends on the counts
            (
                "CONVERGE",
                &self.converge,
                f64::NAN,
                "the convergence criterion",
                POSITIVE,
            ),
            (
                "DELTA",
                &self.delta,
                0.5,
                "the value added to the cells",
                (|v| (0.0..=1.0).contains(&v), "must be from 0 to 1"),
            ),
        ]);
        text.push_str(&criteria(&fields)?);
        if !cweight.is_empty() {
            text.push_str(&format!("\n  /CWEIGHT={cweight}"));
        }
        if self.print_chosen {
            let print = keywords(&HILO_PRINT, &self.print);
            text.push_str(&match print.is_empty() {
                true => "\n  /PRINT=NONE".to_string(),
                false => list("PRINT", &print),
            });
        }
        text.push_str(&list("PLOT", &keywords(&HILO_PLOTS, &self.plots)));
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        // always given: without it the command adds the saturated design with a warning
        text.push_str("\n  /DESIGN");
        if self.custom {
            let terms = self.terms.syntax(&self.factors, "generating class")?;
            text.push_str(&format!("={terms}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// The tables of GENLOG /PRINT.
pub const GENLOG_PRINT: [(&str, &str); 10] = [
    ("FREQ", "Observed and expected counts"),
    ("RESID", "Residuals"),
    ("ZRESID", "Standardized residuals"),
    ("ADJRESID", "Adjusted residuals"),
    ("DEV", "Deviance residuals"),
    ("ITERATION", "Iteration history"),
    ("COV", "Covariances of the estimates"),
    ("DESIGN", "Design matrix"),
    ("ESTIM", "Parameter estimates"),
    ("CORR", "Correlations of the estimates"),
];

/// The tables GENLOG shows without /PRINT.
const GENLOG_DEFAULT_PRINT: [bool; 10] = [
    true, true, false, true, true, false, false, false, false, false,
];

/// The residuals the GENLOG plots draw.
pub const GENLOG_PLOTTED: [(&str, &str); 3] = [
    ("ADJRESID", "Adjusted"),
    ("DEV", "Deviance"),
    ("ADJRESID DEV", "Both"),
];

/// The new variables of GENLOG /SAVE.
pub const GENLOG_SAVE: [(&str, &str); 5] = [
    ("PRED", "Expected counts"),
    ("RESID", "Residuals"),
    ("ZRESID", "Standardized residuals"),
    ("ADJRESID", "Adjusted residuals"),
    ("DEV", "Deviance residuals"),
];

/// GENLOG: a general loglinear model of the factors, or a logit model of dependent factors by
/// the others.
#[derive(Clone, PartialEq, Debug)]
pub struct GeneralLoglinear {
    /// The Logit dialog: dependents BY factors, always multinomial.
    pub logit: bool,
    /// The dependent factors of a logit model.
    pub dependents: Vec<String>,
    pub factors: Vec<String>,
    /// Cell covariates, which enter only through custom terms.
    pub covariates: Vec<String>,
    pub cstructure: String,
    pub gresid: Vec<String>,
    pub glor: Vec<String>,
    /// The General dialog's choice of the multinomial instead of the Poisson model.
    pub multinomial: bool,
    pub converge: String,
    pub iterations: String,
    pub delta: String,
    pub ci_level: String,
    pub eps: String,
    /// One flag per entry of `GENLOG_PRINT`.
    pub print: [bool; 10],
    /// Residuals against the observed and the expected counts.
    pub residual_plots: bool,
    /// Normal and detrended normal plots.
    pub normal_plots: bool,
    /// Index into `GENLOG_PLOTTED`.
    pub plotted: usize,
    /// One flag and one optional new name per entry of `GENLOG_SAVE`.
    pub save: [bool; 5],
    pub save_names: [String; 5],
    pub include: bool,
    /// Custom terms instead of the saturated model.
    pub custom: bool,
    pub terms: Terms,
}

impl Default for GeneralLoglinear {
    fn default() -> Self {
        Self {
            logit: false,
            dependents: Vec::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            cstructure: String::new(),
            gresid: Vec::new(),
            glor: Vec::new(),
            multinomial: false,
            converge: "0.001".into(),
            iterations: "20".into(),
            delta: "0.5".into(),
            ci_level: "95".into(),
            eps: "1E-8".into(),
            print: GENLOG_DEFAULT_PRINT,
            residual_plots: true,
            normal_plots: true,
            plotted: 0,
            save: [false; 5],
            save_names: Default::default(),
            include: false,
            custom: false,
            terms: Terms::default(),
        }
    }
}

impl GeneralLoglinear {
    pub fn new(logit: bool) -> Self {
        Self {
            logit,
            ..Default::default()
        }
    }

    /// The names the custom terms are made of: the factors, then the covariates.
    pub fn model_variables(&self) -> Vec<String> {
        let dependents = if self.logit {
            &self.dependents[..]
        } else {
            &[]
        };
        dependents
            .iter()
            .chain(&self.factors)
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.logit && self.dependents.is_empty() {
            return Err("add at least one dependent variable".into());
        }
        if self.factors.is_empty() {
            return Err("add at least one factor".into());
        }
        let model = self.model_variables();
        let factors = model.len() - self.covariates.len();
        if factors > 10 {
            return Err("the command takes at most 10 factors".into());
        }
        if let Some((_, name)) = model
            .iter()
            .enumerate()
            .find(|(i, n)| model[..*i].iter().any(|m| m.eq_ignore_ascii_case(n)))
        {
            return Err(format!("{name} is in two lists"));
        }
        let cstructure = self.cstructure.trim();
        let structure = [cstructure.to_string()];
        let coefficients: Vec<&String> = structure
            .iter()
            .filter(|n| !n.is_empty())
            .chain(&self.gresid)
            .chain(&self.glor)
            .collect();
        if let Some(name) = coefficients
            .iter()
            .find(|n| model[..factors].iter().any(|f| f.eq_ignore_ascii_case(n)))
        {
            return Err(format!(
                "{name} is a factor, so it cannot give cell weights or coefficients"
            ));
        }
        numeric(data, model.iter().chain(coefficients.iter().copied()))?;
        let mut text = String::from("GENLOG ");
        if self.logit {
            text.push_str(&format!(
                "{} BY {}",
                self.dependents.join(" "),
                self.factors.join(" ")
            ));
        } else {
            text.push_str(&self.factors.join(" "));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        if !cstructure.is_empty() {
            text.push_str(&format!("\n  /CSTRUCTURE={cstructure}"));
        }
        text.push_str(&list("GRESID", &self.gresid));
        text.push_str(&list("GLOR", &self.glor));
        // a logit model needs the multinomial model
        if self.logit || self.multinomial {
            text.push_str("\n  /MODEL=MULTINOMIAL");
        }
        text.push_str(&criteria(&[
            (
                "CONVERGE",
                &self.converge,
                0.001,
                "the convergence criterion",
                POSITIVE,
            ),
            (
                "ITERATE",
                &self.iterations,
                20.0,
                "the maximum iterations",
                ITERATIONS,
            ),
            (
                "DELTA",
                &self.delta,
                0.5,
                "the value added to the cells",
                (|v| v >= 0.0, "cannot be negative"),
            ),
            (
                "CIN",
                &self.ci_level,
                95.0,
                "the confidence level",
                (|v| (50.0..=99.99).contains(&v), "must be from 50 to 99.99"),
            ),
            ("EPS", &self.eps, 1e-8, "the redundancy tolerance", POSITIVE),
        ])?);
        if self.print != GENLOG_DEFAULT_PRINT {
            let print = keywords(&GENLOG_PRINT, &self.print);
            text.push_str(&match print.is_empty() {
                true => "\n  /PRINT=NONE".to_string(),
                false => list("PRINT", &print),
            });
        }
        text.push_str(&self.plot());
        text.push_str(&list(
            "SAVE",
            &saved(data, &GENLOG_SAVE, &self.save, &self.save_names)?,
        ));
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        if self.custom {
            let terms = self.terms.syntax(&model, "model")?;
            text.push_str(&format!("\n  /DESIGN={terms}"));
        } else if !self.covariates.is_empty() {
            return Err("covariates enter the model only through custom terms".into());
        }
        text.push('.');
        Ok(text)
    }

    /// /PLOT when it differs from the default of both plots of adjusted residuals.
    fn plot(&self) -> String {
        let kind = match self.plotted {
            0 => String::new(),
            i => format!("({})", GENLOG_PLOTTED[i].0),
        };
        let plots: Vec<String> = [
            (self.residual_plots, "RESID"),
            (self.normal_plots, "NORMPROB"),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, k)| format!("{k}{kind}"))
        .collect();
        match plots.len() {
            0 => "\n  /PLOT=NONE".into(),
            2 if self.plotted == 0 => String::new(),
            _ => list("PLOT", &plots),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    fn range(name: &str, min: &str, max: &str) -> TableVariable {
        TableVariable {
            name: name.into(),
            min: min.into(),
            max: max.into(),
            ..Default::default()
        }
    }

    fn selection() -> ModelSelection {
        ModelSelection {
            factors: v(&["gender", "region"]),
            ranges: vec![range("gender", "1", "2"), range("region", "1", "4")],
            ..Default::default()
        }
    }

    #[test]
    fn model_selection_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut ModelSelection)| {
            let mut d = selection();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.factors.truncate(1)), "add at least two factors");
        assert_eq!(
            err(&|d| d.factors.push("plan".into())),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| d.factors.push("age".into())),
            "give the range of age"
        );
        assert_eq!(
            err(&|d| d.ranges[1].max = "0".into()),
            "the smallest category value of region is above the largest"
        );
        assert_eq!(
            err(&|d| {
                d.factors.push("age".into());
                d.ranges.push(range("age", "1", "9999"));
                d.factors.push("q1_satisfaction".into());
                d.ranges.push(range("q1_satisfaction", "1", "9999"));
            }),
            "the table would have more than 1000000 cells; narrow the ranges"
        );
        assert_eq!(
            err(&|d| d.maxorder = "0".into()),
            "the highest order must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.delta = "2".into()),
            "the value added to the cells must be from 0 to 1"
        );
        assert_eq!(
            err(&|d| d.iterations = "2.5".into()),
            "the maximum iterations must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| {
                d.backward = true;
                d.probability = "1".into();
            }),
            "the probability to remove must be above 0 and below 1"
        );
        assert_eq!(
            err(&|d| d.custom = true),
            "add at least one term to the generating class"
        );
        assert_eq!(
            err(&|d| {
                d.custom = true;
                d.terms.terms = vec![v(&["gender", "age"])];
            }),
            "age in the generating class is no longer a factor or covariate"
        );
        // the stepping fields count only for backward elimination
        let mut d = selection();
        d.maxsteps = "0".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "HILOGLINEAR gender(1,2) region(1,4)\n  /DESIGN."
        );
    }

    #[test]
    fn general_loglinear_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut GeneralLoglinear)| {
            let mut d = GeneralLoglinear::new(false);
            d.factors = v(&["gender", "region"]);
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.factors.clear()), "add at least one factor");
        assert_eq!(
            err(&|d| d.logit = true),
            "add at least one dependent variable"
        );
        assert_eq!(
            err(&|d| {
                d.logit = true;
                d.dependents = v(&["gender"]);
            }),
            "gender is in two lists"
        );
        assert_eq!(
            err(&|d| d.covariates = v(&["region"])),
            "region is in two lists"
        );
        assert_eq!(
            err(&|d| d.glor = v(&["region"])),
            "region is a factor, so it cannot give cell weights or coefficients"
        );
        assert_eq!(
            err(&|d| d.cstructure = "plan".into()),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| d.covariates = v(&["age"])),
            "covariates enter the model only through custom terms"
        );
        assert_eq!(
            err(&|d| d.ci_level = "40".into()),
            "the confidence level must be from 50 to 99.99"
        );
        assert_eq!(
            err(&|d| d.eps = "0".into()),
            "the redundancy tolerance must be above 0"
        );
        assert_eq!(
            err(&|d| {
                d.save[0] = true;
                d.save_names[0] = "age".into();
            }),
            "age is already a variable"
        );
        assert_eq!(
            err(&|d| {
                d.custom = true;
                d.terms.terms = vec![v(&["gender", "plan"])];
            }),
            "plan in the model is no longer a factor or covariate"
        );
        // a save name of a variable left unchecked is not checked
        let mut d = GeneralLoglinear::new(false);
        d.factors = v(&["gender", "region"]);
        d.save_names[1] = "age".into();
        d.residual_plots = false;
        d.plotted = 1;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "GENLOG gender region\n  /PLOT=NORMPROB(DEV)."
        );
        d.normal_plots = false;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "GENLOG gender region\n  /PLOT=NONE."
        );
    }
}
