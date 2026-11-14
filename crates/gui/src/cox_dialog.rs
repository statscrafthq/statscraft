//! The Cox Regression (COXREG) dialog, with TIME PROGRAM for one time-dependent covariate.
//! Options left as they are add nothing to the syntax, so the command runs with its own
//! defaults.

use crate::advanced_dialogs::ci_level;
use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::model_dialogs::{Categorical, LOGISTIC_METHODS, categorical_syntax};
use crate::stats_dialogs::{Block, numeric, parse};
use crate::survival_dialog::{event, saved};
use crate::syntax_text::{number, value};
use statscraft_data::Dataset;

/// The contrast of a new categorical covariate, an index into `LOGISTIC_CONTRASTS`: deviation,
/// the command's default.
pub const COX_DEFAULT_CONTRAST: usize = 6;

/// The name the dialog gives the time-dependent covariate.
pub const TIME_COVARIATE: &str = "T_COV_";

/// The charts of /PLOT.
pub const COX_PLOTS: [(&str, &str); 4] = [
    ("SURVIVAL", "Survival"),
    ("HAZARD", "Hazard"),
    ("LML", "Log minus log"),
    ("OMS", "One minus survival"),
];

/// The new variables of /SAVE; the index of DFBETA is `COX_DFBETA`.
pub const COX_SAVE: [(&str, &str); 7] = [
    ("SURVIVAL", "Survival function"),
    ("SE", "Standard error of survival"),
    ("HAZARD", "Cumulative hazard"),
    ("LML", "Log minus log of survival"),
    ("PRESID", "Partial residuals"),
    ("DFBETA", "DfBeta"),
    ("XBETA", "X*Beta"),
];

pub const COX_DFBETA: usize = 5;

/// COXREG: one time variable, a status with its event, and blocks of covariates.
#[derive(Clone, PartialEq, Debug)]
pub struct Cox {
    pub time: String,
    pub status: String,
    /// The event is a range of values rather than a list.
    pub event_range: bool,
    /// One value or several, separated by spaces or commas (commas only for a string status).
    pub event_values: String,
    pub event_from: String,
    pub event_to: String,
    /// Terms are variables or interactions written `a BY b`; each block's method is an index
    /// into `LOGISTIC_METHODS`.
    pub blocks: Vec<Block>,
    /// The block the dialog shows.
    pub block: usize,
    pub strata: String,
    pub categorical: Vec<Categorical>,
    /// One flag per entry of `COX_PLOTS`.
    pub plots: [bool; 4],
    /// The categorical covariate drawn as one line per category; may be empty.
    pub pattern_by: String,
    /// Covariates and the values to draw the curves at; an empty value means the mean.
    pub pattern: Vec<(String, String)>,
    /// One flag and one optional new name per entry of `COX_SAVE`.
    pub save: [bool; 7],
    pub save_names: [String; 7],
    pub ci: bool,
    pub ci_level: String,
    pub correlations: bool,
    pub baseline: bool,
    /// Only the last step of each block in full.
    pub last_step: bool,
    pub entry: String,
    pub removal: String,
    pub iterations: String,
    pub include: bool,
    /// An expression in T_ for the time-dependent covariate; empty for none.
    pub time_expression: String,
}

impl Default for Cox {
    fn default() -> Self {
        Self {
            time: String::new(),
            status: String::new(),
            event_range: false,
            event_values: String::new(),
            event_from: String::new(),
            event_to: String::new(),
            blocks: vec![Block::default()],
            block: 0,
            strata: String::new(),
            categorical: Vec::new(),
            plots: [false; 4],
            pattern_by: String::new(),
            pattern: Vec::new(),
            save: [false; 7],
            save_names: Default::default(),
            ci: false,
            ci_level: "95".into(),
            correlations: false,
            baseline: false,
            last_step: false,
            entry: "0.05".into(),
            removal: "0.1".into(),
            iterations: "20".into(),
            include: false,
            time_expression: String::new(),
        }
    }
}

impl Cox {
    /// Each variable named in the blocks once, in order, the parts of interactions included.
    pub fn covariates(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for term in self.blocks.iter().flat_map(|b| &b.variables) {
            for name in term.split(" BY ") {
                if !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
                    out.push(name.to_string());
                }
            }
        }
        out
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let time = self.time.trim();
        let status = self.status.trim();
        if time.is_empty() {
            return Err("choose the time variable".into());
        }
        if status.is_empty() {
            return Err("choose the status variable".into());
        }
        numeric(data, [&self.time])?;
        let (event, _) = event(
            data,
            status,
            self.event_range,
            &self.event_values,
            &self.event_from,
            &self.event_to,
        )?;
        let expression = self.time_expression.trim().trim_end_matches('.').trim();
        let mut blocks: Vec<Block> = self
            .blocks
            .iter()
            .filter(|b| !b.variables.is_empty())
            .cloned()
            .collect();
        if !expression.is_empty() {
            if data.variable("T_").is_ok() {
                return Err("the data has a variable T_, which stands for the time".into());
            }
            match blocks.first_mut() {
                Some(b) => b.variables.push(TIME_COVARIATE.into()),
                None => blocks.push(Block {
                    variables: vec![TIME_COVARIATE.into()],
                    method: 0,
                }),
            }
        }
        if blocks.is_empty() {
            return Err("add at least one covariate".into());
        }
        let covariates = self.covariates();
        let has = |name: &str| covariates.iter().any(|c| c.eq_ignore_ascii_case(name));
        let strata = self.strata.trim();
        for (name, role) in [(time, "time"), (status, "status"), (strata, "strata")] {
            if !name.is_empty() && has(name) {
                return Err(format!("{name} is the {role} variable"));
            }
        }
        if let Some(c) = self.categorical.iter().find(|c| !has(&c.name)) {
            return Err(format!("{} is categorical but in no block", c.name));
        }
        let mut text = String::new();
        if !expression.is_empty() {
            text.push_str(&format!(
                "TIME PROGRAM.\nCOMPUTE {TIME_COVARIATE} = {expression}.\n"
            ));
        }
        text.push_str(&format!("COXREG {time}"));
        // one block entered at once is the bare command's own method
        if let [b] = blocks.as_slice()
            && b.method == 0
        {
            text.push_str(&format!(" WITH {}", b.variables.join(" ")));
        }
        text.push_str(&format!("\n  /STATUS={status} EVENT({event})"));
        if !strata.is_empty() {
            text.push_str(&format!("\n  /STRATA={strata}"));
        }
        if blocks.len() > 1 || blocks[0].method != 0 {
            for b in &blocks {
                let method = LOGISTIC_METHODS[b.method].0;
                text.push_str(&format!("\n  /METHOD={method} {}", b.variables.join(" ")));
            }
        }
        text.push_str(&categorical_syntax(&self.categorical, COX_DEFAULT_CONTRAST));
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push_str(&self.print()?);
        text.push_str(&self.criteria()?);
        let plots = keywords(&COX_PLOTS, &self.plots);
        let pattern = self.pattern_syntax(data, &covariates)?;
        let save = saved(data, &COX_SAVE, &self.save, &self.save_names)?;
        if !expression.is_empty() {
            let other_save = self
                .save
                .iter()
                .enumerate()
                .any(|(i, on)| *on && i != COX_DFBETA);
            if !plots.is_empty() || !pattern.is_empty() || self.baseline || other_save {
                return Err(
                    "with a time-dependent covariate only DfBeta can be saved, and there are no plots, pattern or baseline table"
                        .into(),
                );
            }
        }
        if !plots.is_empty() {
            text.push_str(&format!("\n  /PLOT={}", plots.join(" ")));
        }
        text.push_str(&pattern);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// /PRINT with what was asked for beyond the default output.
    fn print(&self) -> Result<String, String> {
        let mut print = Vec::new();
        if self.ci {
            print.push(format!("CI({})", number(ci_level(&self.ci_level)?)));
        }
        for (on, keyword) in [
            (self.correlations, "CORR"),
            (self.baseline, "BASELINE"),
            (self.last_step, "SUMMARY"),
        ] {
            if on {
                print.push(keyword.into());
            }
        }
        Ok(match print.is_empty() {
            true => String::new(),
            false => format!("\n  /PRINT={}", print.join(" ")),
        })
    }

    /// /CRITERIA with the values that differ from the defaults.
    fn criteria(&self) -> Result<String, String> {
        let entry = parse(&self.entry, "the entry probability")?;
        let removal = parse(&self.removal, "the removal probability")?;
        if !(entry > 0.0 && entry < removal && removal <= 1.0) {
            return Err(
                "the probability to enter must be above 0 and below the probability to remove, which is at most 1"
                    .into(),
            );
        }
        let mut criteria = Vec::new();
        if entry != 0.05 {
            criteria.push(format!("PIN({})", number(entry)));
        }
        if removal != 0.1 {
            criteria.push(format!("POUT({})", number(removal)));
        }
        match whole(&self.iterations, 1, "the maximum number of iterations")? {
            Some(n) if n != 20 => criteria.push(format!("ITERATE({n})")),
            _ => {}
        }
        Ok(match criteria.is_empty() {
            true => String::new(),
            false => format!("\n  /CRITERIA={}", criteria.join(" ")),
        })
    }

    /// /PATTERN: values of covariates still in the model, and the variable after BY, which must
    /// be categorical (a string covariate always is).
    fn pattern_syntax(&self, data: &Dataset, covariates: &[String]) -> Result<String, String> {
        let mut parts = Vec::new();
        let by = self.pattern_by.trim();
        for (name, text) in &self.pattern {
            if text.trim().is_empty() || !covariates.contains(name) {
                continue;
            }
            if name.eq_ignore_ascii_case(by) {
                return Err(format!("{name} has separate lines, so it takes no value"));
            }
            let string = data.variable(name).is_ok_and(|v| v.is_string());
            parts.push(format!("{name}({})", value(text.trim(), string)?));
        }
        if !by.is_empty() {
            let categorical = self
                .categorical
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(by))
                || data.variable(by).is_ok_and(|v| v.is_string());
            if !covariates.iter().any(|c| c.eq_ignore_ascii_case(by)) || !categorical {
                return Err(format!(
                    "{by} gets separate lines only as a categorical covariate"
                ));
            }
            parts.push(format!("BY {by}"));
        }
        Ok(match parts.is_empty() {
            true => String::new(),
            false => format!("\n  /PATTERN={}", parts.join(" ")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn cox() -> Cox {
        let mut d = Cox {
            time: "age".into(),
            status: "q5_1".into(),
            event_values: "1".into(),
            ..Default::default()
        };
        d.blocks[0].variables = v(&["monthly_spend"]);
        d
    }

    #[test]
    fn cox_regression_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut Cox)| {
            let mut d = cox();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.time.clear()), "choose the time variable");
        assert_eq!(err(&|d| d.status.clear()), "choose the status variable");
        assert_eq!(
            err(&|d| d.event_values.clear()),
            "give the values that mark the event"
        );
        assert_eq!(
            err(&|d| d.blocks[0].variables.clear()),
            "add at least one covariate"
        );
        assert_eq!(
            err(&|d| d.blocks[0].variables.push("age".into())),
            "age is the time variable"
        );
        assert_eq!(
            err(&|d| d.blocks[0].variables.push("gender BY age".into())),
            "age is the time variable"
        );
        assert_eq!(
            err(&|d| d.blocks[0].variables.push("q5_1".into())),
            "q5_1 is the status variable"
        );
        assert_eq!(
            err(&|d| {
                d.strata = "gender".into();
                d.blocks[0].variables.push("gender BY monthly_spend".into());
            }),
            "gender is the strata variable"
        );
        assert_eq!(
            err(&|d| d.categorical.push(Categorical {
                name: "region".into(),
                contrast: COX_DEFAULT_CONTRAST,
                first: false,
            })),
            "region is categorical but in no block"
        );
        assert_eq!(
            err(&|d| d.removal = "0.05".into()),
            "the probability to enter must be above 0 and below the probability to remove, which is at most 1"
        );
        assert_eq!(
            err(&|d| d.iterations = "0".into()),
            "the maximum number of iterations must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.pattern_by = "monthly_spend".into()),
            "monthly_spend gets separate lines only as a categorical covariate"
        );
        assert_eq!(
            err(&|d| d.pattern_by = "gender".into()),
            "gender gets separate lines only as a categorical covariate"
        );
        assert_eq!(
            err(&|d| {
                d.blocks[0].variables.push("plan".into());
                d.pattern_by = "plan".into();
                d.pattern = vec![("plan".into(), "BASIC".into())];
            }),
            "plan has separate lines, so it takes no value"
        );
        assert_eq!(
            err(&|d| d.pattern = vec![("monthly_spend".into(), "lots".into())]),
            "\"lots\" is not a number"
        );
        assert_eq!(
            err(&|d| {
                d.time_expression = "monthly_spend * T_".into();
                d.plots[0] = true;
            }),
            "with a time-dependent covariate only DfBeta can be saved, and there are no plots, pattern or baseline table"
        );
        assert_eq!(
            err(&|d| {
                d.time_expression = "monthly_spend * T_".into();
                d.save[0] = true;
            }),
            "with a time-dependent covariate only DfBeta can be saved, and there are no plots, pattern or baseline table"
        );
    }

    #[test]
    fn cox_regression_writes_only_what_changed() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let mut d = cox();
        // an empty later block and pattern values of covariates no longer in the model add nothing
        d.blocks.push(Block::default());
        d.pattern = vec![("region".into(), "2".into())];
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "COXREG age WITH monthly_spend\n  /STATUS=q5_1 EVENT(1)."
        );
        assert!(run_in(&mut session, &syntax) >= 3);

        // a string status, a stepwise block and a categorical covariate with the default
        // contrast, which needs no CONTRAST
        let mut d = cox();
        d.status = "plan".into();
        d.event_values = "PREMIUM".into();
        d.blocks[0].method = 1;
        d.blocks[0].variables.push("region".into());
        d.categorical.push(Categorical {
            name: "region".into(),
            contrast: COX_DEFAULT_CONTRAST,
            first: false,
        });
        d.ci = true;
        d.ci_level = "90".into();
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "COXREG age\n  /STATUS=plan EVENT('PREMIUM')\n  \
             /METHOD=FSTEP(COND) monthly_spend region\n  /CATEGORICAL=region\n  /PRINT=CI(90)."
        );
        assert!(run_in(&mut session, &syntax) >= 3);

        // the time-dependent covariate alone forms the first block
        let mut d = cox();
        d.blocks[0].variables.clear();
        d.time_expression = "monthly_spend * LN(T_ + 1).".into();
        d.save[COX_DFBETA] = true;
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "TIME PROGRAM.\nCOMPUTE T_COV_ = monthly_spend * LN(T_ + 1).\n\
             COXREG age WITH T_COV_\n  /STATUS=q5_1 EVENT(1)\n  /SAVE=DFBETA."
        );
        assert!(run_in(&mut session, &syntax) >= 3);
    }
}
