//! The Multiple Imputation dialogs: Impute Missing Data Values, and Analyze Patterns, which only
//! summarizes the missing values (METHOD=NONE). Options left as they are add nothing to the
//! syntax, so the command runs with its own defaults.

use crate::mds_dialog::count;
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::parse;
use crate::syntax_text::{number, quote, value};
use statscraft_data::Dataset;

/// The ways to impute; the first writes nothing and lets the command pick by the pattern.
pub const IMPUTE_METHODS: [(&str, &str); 3] = [
    ("", "Automatic"),
    ("FCS", "Fully conditional specification (iterative)"),
    ("MONOTONE", "Monotone (one pass)"),
];

/// What a variable does in the models; the first writes nothing.
pub const IMPUTE_ROLES: [(&str, &str); 3] = [
    ("", "Impute and predict"),
    ("DEP", "Impute only"),
    ("IND", "Predict only"),
];

/// Where the imputed data go; there is no choice of saving nothing, so the first is blank.
pub const IMPUTED_OUTPUT: [&str; 3] = ["", "A data file", "A new dataset"];

/// Where the iteration history goes; the first saves nothing.
pub const HISTORY_OUTPUT: [&str; 3] = ["None", "A data file", "A new dataset"];

/// The names the command adds to the data it writes.
const IMPUTATION: &str = "Imputation_";
const HISTORY_NAMES: [&str; 2] = ["Iteration_", "SummaryStatistic_"];

/// The settings of one variable of the model, kept by name while the variable is in the list.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Limits {
    pub name: String,
    /// An index into `IMPUTE_ROLES`.
    pub role: usize,
    /// Lowest and highest imputed value, and the step values are rounded to; empty for none.
    pub min: String,
    pub max: String,
    pub rnd: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Imputation {
    /// Analyze Patterns: summaries of the missing values and nothing imputed.
    pub patterns_only: bool,
    pub variables: Vec<String>,
    pub nimputations: String,
    /// The imputed data: 1 a data file, 2 a new dataset.
    pub output: usize,
    pub file: String,
    pub dataset: String,
    /// An index into `IMPUTE_METHODS`.
    pub method: usize,
    pub maxiter: String,
    /// Predictive mean matching for scale variables, from the `pmm_k` nearest predictions.
    pub pmm: bool,
    pub pmm_k: String,
    pub twoway: bool,
    pub singular: String,
    /// Variables missing in more than this percentage of cases are left out; empty for no limit.
    pub maxpctmissing: String,
    pub maxcasedraws: String,
    pub maxparamdraws: String,
    pub maxmodelparam: String,
    pub limits: Vec<Limits>,
    pub overall: bool,
    /// The table of the most often missing variables: how many at most, and from what share.
    pub variable_summary: bool,
    pub maxvars: String,
    pub minpct: String,
    pub patterns: bool,
    pub models: bool,
    pub descriptives: bool,
    /// The variables described; none checked means every imputed one.
    pub described: Vec<String>,
    pub weight: String,
    /// The iteration history: an index into `HISTORY_OUTPUT`.
    pub history: usize,
    pub history_file: String,
    pub history_dataset: String,
}

impl Default for Imputation {
    fn default() -> Self {
        Self {
            patterns_only: false,
            variables: Vec::new(),
            nimputations: "5".into(),
            output: 2,
            file: String::new(),
            dataset: String::new(),
            method: 0,
            maxiter: "10".into(),
            pmm: false,
            pmm_k: "5".into(),
            twoway: false,
            singular: "1E-12".into(),
            maxpctmissing: String::new(),
            maxcasedraws: "50".into(),
            maxparamdraws: "2".into(),
            maxmodelparam: "100".into(),
            limits: Vec::new(),
            overall: true,
            variable_summary: false,
            maxvars: "25".into(),
            minpct: "10".into(),
            patterns: false,
            models: true,
            descriptives: false,
            described: Vec::new(),
            weight: String::new(),
            history: 0,
            history_file: String::new(),
            history_dataset: String::new(),
        }
    }
}

impl Imputation {
    /// The custom methods take the settings of the models; automatic keeps its own.
    pub fn custom(&self) -> bool {
        self.method != 0
    }

    /// Predictive mean matching takes observed values, so MIN and MAX do not apply.
    pub fn bounded(&self) -> bool {
        !(self.custom() && self.pmm)
    }

    /// Only the iterative method has a history; automatic may pick it.
    pub fn has_history(&self) -> bool {
        self.method != 2
    }

    /// MISSINGSUMMARIES, empty at its default of the overall summary alone.
    fn missing_summaries(&self) -> Result<String, String> {
        let mut words = Vec::new();
        if self.overall {
            words.push("OVERALL".to_string());
        }
        if self.variable_summary {
            let mut word = "VARIABLES".to_string();
            let mut inner = Vec::new();
            let maxvars = count(&self.maxvars, 1, "the most variables listed", 25)?;
            if maxvars != 25 {
                inner.push(format!("MAXVARS={maxvars}"));
            }
            let minpct = match self.minpct.trim() {
                "" => 10.0,
                t => parse(t, "the least percentage missing")?,
            };
            if !(0.0..100.0).contains(&minpct) {
                return Err("the least percentage missing must be from 0 to below 100".into());
            }
            if minpct != 10.0 {
                inner.push(format!("MINPCTMISSING={}", number(minpct)));
            }
            if !inner.is_empty() {
                word.push_str(&format!("({})", inner.join(" ")));
            }
            words.push(word);
        }
        if self.patterns {
            words.push("PATTERNS".into());
        }
        Ok(match words.as_slice() {
            [] => "\n  /MISSINGSUMMARIES NONE".into(),
            [only] if only == "OVERALL" => String::new(),
            _ => format!("\n  /MISSINGSUMMARIES {}", words.join(" ")),
        })
    }

    /// The IMPUTE keywords that differ from the command's defaults.
    fn impute(&self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        if self.custom() {
            out.push(format!("METHOD={}", IMPUTE_METHODS[self.method].0));
        }
        let n = count(&self.nimputations, 1, "the number of imputations", 5)?;
        if n != 5 {
            out.push(format!("NIMPUTATIONS={n}"));
        }
        // automatic ignores these and warns
        if self.custom() {
            if self.pmm {
                match count(&self.pmm_k, 1, "the nearest predictions", 5)? {
                    5 => out.push("SCALEMODEL=PMM".into()),
                    k => out.push(format!("SCALEMODEL=PMM({k})")),
                }
            }
            if self.twoway {
                out.push("INTERACTIONS=TWOWAY".into());
            }
            if self.method == 1 {
                let maxiter = count(&self.maxiter, 1, "the number of iterations", 10)?;
                if maxiter != 10 {
                    out.push(format!("MAXITER={maxiter}"));
                }
            }
            let singular = match self.singular.trim() {
                "" => 1e-12,
                t => parse(t, "the singularity tolerance")?,
            };
            if singular <= 0.0 {
                return Err("the singularity tolerance must be above 0".into());
            }
            if singular != 1e-12 {
                out.push(format!("SINGULAR={}", number(singular)));
            }
        }
        if !self.maxpctmissing.trim().is_empty() {
            let p = parse(&self.maxpctmissing, "the most percentage missing")?;
            if !(p > 0.0 && p < 100.0) {
                return Err("the most percentage missing must be above 0 and below 100".into());
            }
            out.push(format!("MAXPCTMISSING={}", number(p)));
        }
        for (text, what, keyword, default) in [
            (&self.maxcasedraws, "the draws per case", "MAXCASEDRAWS", 50),
            (
                &self.maxparamdraws,
                "the parameter draws",
                "MAXPARAMDRAWS",
                2,
            ),
            (
                &self.maxmodelparam,
                "the most model parameters",
                "MAXMODELPARAM",
                100,
            ),
        ] {
            let n = count(text, 1, what, default)?;
            if n != default {
                out.push(format!("{keyword}={n}"));
            }
        }
        Ok(out)
    }

    /// One CONSTRAINTS subcommand per variable with settings of its own.
    fn constraints(&self, data: &Dataset) -> Result<String, String> {
        let mut text = String::new();
        for name in &self.variables {
            let Some(l) = self.limits.iter().find(|l| &l.name == name) else {
                continue;
            };
            let mut parts = Vec::new();
            if self.bounded() {
                // a date variable also takes its limits as dates, written quoted
                let date = data.variable(name).is_ok_and(|v| {
                    let kind = v
                        .format()
                        .trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
                    ["DATE", "ADATE", "EDATE", "SDATE"].contains(&kind.to_uppercase().as_str())
                });
                let bound = |text: &str, which: &str| -> Result<Option<String>, String> {
                    match text.trim() {
                        "" => Ok(None),
                        t => match value(t, false) {
                            Ok(n) => Ok(Some(n)),
                            Err(_) if date => Ok(Some(quote(t))),
                            Err(_) => Err(format!("the {which} of {name} must be a number")),
                        },
                    }
                };
                let min = bound(&l.min, "minimum")?;
                let max = bound(&l.max, "maximum")?;
                if let (Some(a), Some(b)) = (&min, &max)
                    && let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>())
                    && a >= b
                {
                    return Err(format!("the minimum of {name} must be below its maximum"));
                }
                if let Some(min) = min {
                    parts.push(format!("MIN={min}"));
                }
                if let Some(max) = max {
                    parts.push(format!("MAX={max}"));
                }
            }
            if !l.rnd.trim().is_empty() {
                let rnd = parse(&l.rnd, &format!("the rounding of {name}"))?;
                if rnd <= 0.0 {
                    return Err(format!("the rounding of {name} must be above 0"));
                }
                parts.push(format!("RND={}", number(rnd)));
            }
            if l.role != 0 {
                parts.push(format!("ROLE={}", IMPUTE_ROLES[l.role].0));
            }
            if !parts.is_empty() {
                text.push_str(&format!("\n  /CONSTRAINTS {name} ({})", parts.join(" ")));
            }
        }
        Ok(text)
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        let weight = self.weight.trim();
        if !weight.is_empty() && data.variable(weight).is_ok_and(|v| v.is_string()) {
            return Err(format!(
                "{weight} is a string; the analysis weight must be numeric"
            ));
        }
        let mut text = format!("MULTIPLE IMPUTATION {}", self.variables.join(" "));
        if self.patterns_only {
            if !(self.overall || self.variable_summary || self.patterns) {
                return Err("choose at least one summary".into());
            }
            text.push_str("\n  /IMPUTE METHOD=NONE");
            text.push_str(&self.missing_summaries()?);
            if !weight.is_empty() {
                text.push_str(&format!("\n  /ANALYSISWEIGHT {weight}"));
            }
            text.push('.');
            return Ok(text);
        }

        if data.variable_index(IMPUTATION).is_some() {
            return Err(format!(
                "the data already have a variable named {IMPUTATION}; rename it first"
            ));
        }
        if data.split_file().is_some_and(|s| s.variables.len() >= 8) {
            return Err(
                "imputing needs a split variable of its own; split by fewer than eight".into(),
            );
        }
        let impute = self.impute()?;
        if !impute.is_empty() {
            text.push_str(&format!("\n  /IMPUTE {}", impute.join(" ")));
        }
        text.push_str(&self.constraints(data)?);
        text.push_str(&self.missing_summaries()?);
        let mut summaries = Vec::new();
        if self.models {
            summaries.push("MODELS".to_string());
        }
        if self.descriptives {
            let described: Vec<&str> = self
                .variables
                .iter()
                .filter(|n| self.described.contains(n))
                .map(String::as_str)
                .collect();
            summaries.push(match described.is_empty() {
                true => "DESCRIPTIVES".into(),
                false => format!("DESCRIPTIVES({})", described.join(" ")),
            });
        }
        match summaries.as_slice() {
            [] => text.push_str("\n  /IMPUTATIONSUMMARIES NONE"),
            [only] if only == "MODELS" => {}
            _ => text.push_str(&format!("\n  /IMPUTATIONSUMMARIES {}", summaries.join(" "))),
        }
        if !weight.is_empty() {
            text.push_str(&format!("\n  /ANALYSISWEIGHT {weight}"));
        }

        let imputations =
            output_target(self.output, &self.file, &self.dataset, "the imputed data")?
                .ok_or("choose where to save the imputed data")?;
        let mut declare = Vec::new();
        if self.output == 2 {
            declare.push(imputations.clone());
        }
        text.push_str(&format!("\n  /OUTFILE IMPUTATIONS={imputations}"));
        if self.has_history()
            && let Some(history) = output_target(
                self.history,
                &self.history_file,
                &self.history_dataset,
                "the iteration history",
            )?
        {
            if let Some(name) = HISTORY_NAMES
                .iter()
                .find(|n| data.variable_index(n).is_some())
            {
                return Err(format!(
                    "the iteration history needs the name {name}, which the data already use"
                ));
            }
            // dataset names ignore case, file paths may not
            let same = match self.output {
                2 => history.eq_ignore_ascii_case(&imputations),
                _ => history == imputations,
            };
            if self.history == self.output && same {
                return Err(
                    "the imputed data and the iteration history need different places".into(),
                );
            }
            if self.history == 2 {
                declare.push(history.clone());
            }
            text.push_str(&format!(" FCSITERATIONS={history}"));
        }
        text.push('.');
        // the command writes into a dataset only when one of that name exists
        let declare: String = declare
            .iter()
            .map(|name| format!("DATASET DECLARE {name}.\n"))
            .collect();
        Ok(declare + &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn dialog() -> Imputation {
        Imputation {
            variables: v(&["age", "monthly_spend", "q2_recommend"]),
            dataset: "filled".into(),
            ..Default::default()
        }
    }

    #[test]
    fn imputation_writes_only_what_changed() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let mut d = dialog();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "DATASET DECLARE filled.\nMULTIPLE IMPUTATION age monthly_spend q2_recommend\n  \
             /OUTFILE IMPUTATIONS=filled."
        );
        // automatic ignores the model settings, so they are not written
        d.pmm = true;
        d.twoway = true;
        d.maxiter = "20".into();
        d.singular = "0.001".into();
        // a role or limit of a variable no longer in the list is dropped
        d.limits.push(Limits {
            name: "region".into(),
            role: 2,
            ..Default::default()
        });
        // the history of a monotone imputation is not written
        d.history = 2;
        d.history_dataset = "trace".into();
        d.method = 2;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "DATASET DECLARE filled.\nMULTIPLE IMPUTATION age monthly_spend q2_recommend\n  \
             /IMPUTE METHOD=MONOTONE SCALEMODEL=PMM INTERACTIONS=TWOWAY SINGULAR=0.001\n  \
             /OUTFILE IMPUTATIONS=filled."
        );
        // limits do not apply to matching, rounding does
        d.limits.push(Limits {
            name: "age".into(),
            min: "18".into(),
            max: "90".into(),
            rnd: "1".into(),
            ..Default::default()
        });
        d.models = false;
        d.overall = false;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "DATASET DECLARE filled.\nMULTIPLE IMPUTATION age monthly_spend q2_recommend\n  \
             /IMPUTE METHOD=MONOTONE SCALEMODEL=PMM INTERACTIONS=TWOWAY SINGULAR=0.001\n  \
             /CONSTRAINTS age (RND=1)\n  /MISSINGSUMMARIES NONE\n  /IMPUTATIONSUMMARIES NONE\n  \
             /OUTFILE IMPUTATIONS=filled."
        );
        // a date variable takes quoted limits
        let mut d = dialog();
        d.variables.push("signup_date".into());
        d.limits.push(Limits {
            name: "signup_date".into(),
            min: "01-JAN-2020".into(),
            ..Default::default()
        });
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/CONSTRAINTS signup_date (MIN='01-JAN-2020')")
        );
    }

    #[test]
    fn imputation_checks_what_the_command_would_reject() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let e = |change: &dyn Fn(&mut Imputation)| {
            let mut d = dialog();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            e(&|d| d.variables.truncate(1)),
            "add at least two variables"
        );
        assert_eq!(e(&|d| d.dataset.clear()), "name the new dataset");
        assert_eq!(e(&|d| d.output = 1), "choose the file for the imputed data");
        assert_eq!(
            e(&|d| d.nimputations = "0".into()),
            "the number of imputations must be a whole number of at least 1"
        );
        assert_eq!(
            e(&|d| d.maxpctmissing = "100".into()),
            "the most percentage missing must be above 0 and below 100"
        );
        assert_eq!(
            e(&|d| {
                d.method = 1;
                d.singular = "0".into();
            }),
            "the singularity tolerance must be above 0"
        );
        assert_eq!(
            e(&|d| d.limits.push(Limits {
                name: "age".into(),
                min: "50".into(),
                max: "20".into(),
                ..Default::default()
            })),
            "the minimum of age must be below its maximum"
        );
        assert_eq!(
            e(&|d| d.limits.push(Limits {
                name: "age".into(),
                rnd: "-1".into(),
                ..Default::default()
            })),
            "the rounding of age must be above 0"
        );
        assert_eq!(
            e(&|d| d.weight = "plan".into()),
            "plan is a string; the analysis weight must be numeric"
        );
        assert_eq!(
            e(&|d| {
                d.history = 2;
                d.history_dataset = "FILLED".into();
            }),
            "the imputed data and the iteration history need different places"
        );
        assert_eq!(
            e(&|d| {
                d.patterns_only = true;
                d.overall = false;
            }),
            "choose at least one summary"
        );
        // the settings of a switched-off part are not checked
        let mut d = dialog();
        d.maxiter = "x".into();
        d.history = 1;
        d.method = 2;
        d.limits.push(Limits {
            name: "age".into(),
            min: "x".into(),
            ..Default::default()
        });
        d.pmm = true;
        assert!(d.syntax(&data).is_ok());
        // automatic keeps its own model settings
        let mut d = dialog();
        d.pmm = true;
        d.pmm_k = "0".into();
        d.singular = "-1".into();
        assert!(d.syntax(&data).is_ok());

        run_in(&mut session, "COMPUTE Imputation_ = 1.\nEXECUTE.");
        let renamed = session.dataset().unwrap().clone();
        assert_eq!(
            dialog().syntax(&renamed).unwrap_err(),
            "the data already have a variable named Imputation_; rename it first"
        );
        // analyzing patterns writes no data, so the name does not matter
        let mut d = dialog();
        d.patterns_only = true;
        d.patterns = true;
        assert_eq!(
            d.syntax(&renamed).unwrap(),
            "MULTIPLE IMPUTATION age monthly_spend q2_recommend\n  /IMPUTE METHOD=NONE\n  \
             /MISSINGSUMMARIES OVERALL PATTERNS."
        );
    }
}
