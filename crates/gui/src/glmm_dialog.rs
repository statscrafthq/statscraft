//! The Generalized Linear Mixed Models dialog: GENLINMIXED of one target on fixed effects with
//! random effects of subjects. Options left as they are add nothing to the syntax. Whether a
//! field is a factor or a covariate follows its measurement level, so the dialog has one list.

use crate::advanced_dialogs::{Terms, ci_level};
use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::{number, value};
use statscraft_data::Dataset;
use statscraft_syntax::{GenlinDistribution as D, GenlinLink as L, link_allowed};

/// The distributions, keyword and label; the first, none written, lets the target decide.
pub const GLMM_DISTRIBUTIONS: [(&str, &str); 6] = [
    ("", "Automatic"),
    ("NORMAL", "Normal"),
    ("BINOMIAL", "Binomial"),
    ("GAMMA", "Gamma"),
    ("INVERSE_GAUSSIAN", "Inverse Gaussian"),
    ("POISSON", "Poisson"),
];

/// The links, keyword and label; the first, none written, is the distribution's own.
pub const GLMM_LINKS: [(&str, &str); 9] = [
    ("", "Default of the distribution"),
    ("IDENTITY", "Identity"),
    ("LOG", "Log"),
    ("LOGIT", "Logit"),
    ("PROBIT", "Probit"),
    ("CLOGLOG", "Complementary log-log"),
    ("NLOGLOG", "Negative log-log"),
    ("LOGC", "Log complement"),
    ("POWER", "Power"),
];

/// The structures of the random effects of a block; the first is the default.
pub const GLMM_COVARIANCES: [(&str, &str); 10] = [
    ("", "Variance components"),
    ("IDENTITY", "Scaled identity"),
    ("DIAGONAL", "Diagonal"),
    ("UNSTRUCTURED", "Unstructured"),
    ("COMPOUND_SYMMETRY", "Compound symmetry"),
    ("AR1", "First-order autoregressive"),
    ("ARH1", "Heterogeneous AR(1)"),
    ("ARMA11", "ARMA(1,1)"),
    ("CSH", "Heterogeneous compound symmetry"),
    ("TOEPLITZ", "Toeplitz"),
];

/// How the denominator degrees of freedom are found; the first is the default.
pub const GLMM_DF_METHODS: [(&str, &str); 3] = [
    ("", "Residual"),
    ("SATTERTHWAITE", "Satterthwaite"),
    ("KENWARD_ROGER", "Kenward-Roger"),
];

/// The order of the two target values of a binomial target; the first is the default.
pub const GLMM_ORDERS: [(&str, &str); 3] = [
    ("", "Ascending"),
    ("DESCENDING", "Descending"),
    ("DATA", "As they come in the data"),
];

/// Not given, a number, or a field: the form of trials and offset.
pub const GLMM_FORMS: [&str; 3] = ["None", "Value", "Field"];

/// The convergence limit of the covariance parameters the command starts from.
const PCONVERGE: f64 = 1e-6;

/// One RANDOM block.
#[derive(Clone, PartialEq, Debug)]
pub struct RandomBlock {
    pub intercept: bool,
    pub terms: Terms,
    /// The subjects the effects vary over, a selection of the data structure's.
    pub subjects: Vec<String>,
    /// Index into `GLMM_COVARIANCES`.
    pub covariance: usize,
    /// The predicted random effects of every subject.
    pub solution: bool,
}

impl Default for RandomBlock {
    fn default() -> Self {
        Self {
            intercept: true,
            terms: Terms::default(),
            subjects: Vec::new(),
            covariance: 0,
            solution: false,
        }
    }
}

/// GENLINMIXED: a generalized linear mixed model of one target.
#[derive(Clone, PartialEq, Debug)]
pub struct GenlinMixed {
    /// The fields that tell the subjects apart.
    pub subjects: Vec<String>,
    pub target: String,
    /// Index into `GLMM_DISTRIBUTIONS`.
    pub distribution: usize,
    /// Index into `GLMM_LINKS`.
    pub link: usize,
    /// The exponent of the power link.
    pub power: String,
    /// The value of a binomial target the model is not for; empty for the order.
    pub reference: String,
    /// Index into `GLMM_FORMS`: events out of this many trials.
    pub trials_form: usize,
    pub trials: String,
    pub weight: String,
    /// Index into `GLMM_FORMS`.
    pub offset_form: usize,
    pub offset: String,
    /// The fields of the fixed effects.
    pub inputs: Vec<String>,
    /// The fixed terms of `fixed` instead of the main effects.
    pub custom_fixed: bool,
    pub fixed: Terms,
    pub intercept: bool,
    pub random: Vec<RandomBlock>,
    /// Index into `GLMM_ORDERS`.
    pub order: usize,
    pub iterations: String,
    pub ci_level: String,
    /// Index into `GLMM_DF_METHODS`.
    pub df_method: usize,
    pub pconverge: String,
}

impl Default for GenlinMixed {
    fn default() -> Self {
        Self {
            subjects: Vec::new(),
            target: String::new(),
            distribution: 0,
            link: 0,
            power: "1".into(),
            reference: String::new(),
            trials_form: 0,
            trials: String::new(),
            weight: String::new(),
            offset_form: 0,
            offset: String::new(),
            inputs: Vec::new(),
            custom_fixed: false,
            fixed: Terms::default(),
            intercept: true,
            random: vec![RandomBlock::default()],
            order: 0,
            iterations: "100".into(),
            ci_level: "95".into(),
            df_method: 0,
            pconverge: number(PCONVERGE),
        }
    }
}

/// The value of TRIALS or OFFSET for a form and its text; None when not given.
fn form_text(
    form: usize,
    text: &str,
    whole_number: bool,
    what: &str,
) -> Result<Option<String>, String> {
    match form {
        0 => Ok(None),
        1 if whole_number => Ok(Some(format!(
            "VALUE({})",
            whole(text, 1, what)?.ok_or_else(|| format!("give {what}"))?
        ))),
        1 => Ok(Some(format!("VALUE({})", number(parse(text, what)?)))),
        _ => match text.trim() {
            "" => Err(format!("choose the field of {what}")),
            field => Ok(Some(format!("FIELD({field})"))),
        },
    }
}

impl GenlinMixed {
    /// The distribution chosen, None for automatic.
    fn kind(&self) -> Option<D> {
        match GLMM_DISTRIBUTIONS[self.distribution].0 {
            "NORMAL" => Some(D::Normal),
            "BINOMIAL" => Some(D::Binomial),
            "GAMMA" => Some(D::Gamma),
            "INVERSE_GAUSSIAN" => Some(D::InverseGaussian),
            "POISSON" => Some(D::Poisson),
            _ => None,
        }
    }

    /// Link `i` of `GLMM_LINKS` goes with the distribution chosen; any does with automatic.
    pub fn link_allowed(&self, i: usize) -> bool {
        let link = match GLMM_LINKS[i].0 {
            "IDENTITY" => L::Identity,
            "LOG" => L::Log,
            "LOGIT" => L::Logit,
            "PROBIT" => L::Probit,
            "CLOGLOG" => L::Cloglog,
            "NLOGLOG" => L::Nloglog,
            "LOGC" => L::Logc,
            "POWER" => L::Power(1.0),
            _ => return true,
        };
        self.kind().is_none_or(|d| link_allowed(d, link))
    }

    /// Unchecks the random-effect subjects taken out of Subjects, so the dialog shows what the
    /// syntax uses.
    pub fn forget_old_subjects(&mut self) {
        for b in &mut self.random {
            b.subjects.retain(|s| self.subjects.contains(s));
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err("choose the target".into());
        }
        if self.subjects.is_empty() {
            return Err("add the subject fields".into());
        }
        let weight = self.weight.trim();
        let trials = form_text(self.trials_form, &self.trials, true, "the number of trials")?;
        let offset = form_text(self.offset_form, &self.offset, false, "the offset")?;
        let mut roles = vec![target];
        roles.extend(self.inputs.iter().map(String::as_str));
        roles.extend(Some(weight).filter(|w| !w.is_empty()));
        for (form, text) in [
            (self.trials_form, &self.trials),
            (self.offset_form, &self.offset),
        ] {
            roles.extend(Some(text.trim()).filter(|t| form == 2 && !t.is_empty()));
        }
        if let Some(n) =
            (1..roles.len()).find_map(|i| roles[..i].contains(&roles[i]).then_some(roles[i]))
        {
            return Err(format!("{n} has more than one role in the model"));
        }
        if !self.link_allowed(self.link) {
            return Err(format!(
                "the {} link does not go with the {} distribution",
                GLMM_LINKS[self.link].1, GLMM_DISTRIBUTIONS[self.distribution].1
            ));
        }
        let mut text = format!(
            "GENLINMIXED\n  /DATA_STRUCTURE SUBJECTS={}\n  /FIELDS TARGET={target}",
            self.subjects.join("*")
        );
        if let Some(t) = trials {
            text.push_str(&format!(" TRIALS={t}"));
        }
        if !weight.is_empty() {
            text.push_str(&format!(" ANALYSIS_WEIGHT={weight}"));
        }
        if let Some(o) = offset {
            text.push_str(&format!(" OFFSET={o}"));
        }

        let mut options = Vec::new();
        if self.distribution > 0 {
            options.push(format!(
                "DISTRIBUTION={}",
                GLMM_DISTRIBUTIONS[self.distribution].0
            ));
        }
        if self.link > 0 {
            options.push(format!("LINK={}", GLMM_LINKS[self.link].0));
            if GLMM_LINKS[self.link].0 == "POWER" {
                options.push(format!(
                    "LINK_PARAMETER={}",
                    number(parse(&self.power, "the exponent of the link")?)
                ));
            }
        }
        let reference = self.reference.trim();
        if !reference.is_empty() {
            let string = data.variable(target).is_ok_and(|v| v.is_string());
            options.push(format!("REFERENCE={}", value(reference, string)?));
        }
        if !options.is_empty() {
            text.push_str(&format!("\n  /TARGET_OPTIONS {}", options.join(" ")));
        }

        let effects = match (self.custom_fixed, self.fixed.terms.is_empty()) {
            (false, _) => self.inputs.join(" "),
            (true, true) => String::new(),
            (true, false) => self.fixed.syntax(&self.inputs, "fixed effects")?,
        };
        if !effects.is_empty() || !self.intercept {
            text.push_str("\n  /FIXED");
            if !effects.is_empty() {
                text.push_str(&format!(" EFFECTS={effects}"));
            }
            if !self.intercept {
                text.push_str(" USE_INTERCEPT=FALSE");
            }
        }

        if self.random.is_empty() {
            return Err("add a random effects block".into());
        }
        for (i, b) in self.random.iter().enumerate() {
            let n = i + 1;
            if let Some(s) = b.subjects.iter().find(|s| !self.subjects.contains(s)) {
                return Err(format!("{s} in random block {n} is no longer a subject"));
            }
            let subjects: Vec<&str> = self
                .subjects
                .iter()
                .filter(|s| b.subjects.contains(s))
                .map(String::as_str)
                .collect();
            if subjects.is_empty() {
                return Err(format!("tick the subjects of random block {n}"));
            }
            let mut line = String::from("\n  /RANDOM");
            if !b.terms.terms.is_empty() {
                let what = format!("random effects of block {n}");
                line.push_str(&format!(
                    " EFFECTS={}",
                    b.terms.syntax(&self.inputs, &what)?
                ));
            } else if !b.intercept {
                return Err(format!("random block {n} needs the intercept or a term"));
            }
            if b.intercept {
                line.push_str(" USE_INTERCEPT=TRUE");
            }
            line.push_str(&format!(" SUBJECTS={}", subjects.join("*")));
            if b.covariance > 0 {
                line.push_str(&format!(
                    " COVARIANCE_TYPE={}",
                    GLMM_COVARIANCES[b.covariance].0
                ));
            }
            if b.solution {
                line.push_str(" SOLUTION=TRUE");
            }
            text.push_str(&line);
        }

        let mut build = Vec::new();
        if self.order > 0 {
            build.push(format!(
                "TARGET_CATEGORY_ORDER={}",
                GLMM_ORDERS[self.order].0
            ));
        }
        match whole(&self.iterations, 1, "the maximum iterations")? {
            Some(n) if n != 100 => build.push(format!("MAX_ITERATIONS={n}")),
            _ => {}
        }
        let level = ci_level(&self.ci_level)?;
        if level != 95.0 {
            build.push(format!("CONFIDENCE_LEVEL={}", number(level)));
        }
        if self.df_method > 0 {
            build.push(format!("DF_METHOD={}", GLMM_DF_METHODS[self.df_method].0));
        }
        let limit = parse(&self.pconverge, "the convergence limit")?;
        if limit < 0.0 {
            return Err("the convergence limit must be 0 or more".into());
        }
        if limit != PCONVERGE {
            build.push(format!("PCONVERGE={}", number(limit)));
        }
        if !build.is_empty() {
            text.push_str(&format!("\n  /BUILD_OPTIONS {}", build.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> GenlinMixed {
        GenlinMixed {
            subjects: vec!["clinic".into()],
            target: "cured".into(),
            inputs: vec!["dose".into(), "age".into()],
            random: vec![RandomBlock {
                subjects: vec!["clinic".into()],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn syntax(d: &GenlinMixed) -> Result<String, String> {
        d.syntax(&Dataset::new(0))
    }

    #[test]
    fn the_plainest_model_writes_the_required_parts_only() {
        assert_eq!(
            syntax(&model()).unwrap(),
            "GENLINMIXED\n  /DATA_STRUCTURE SUBJECTS=clinic\n  /FIELDS TARGET=cured\n  \
             /FIXED EFFECTS=dose age\n  /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic."
        );
    }

    #[test]
    fn every_option_is_written() {
        let mut d = model();
        d.subjects = vec!["clinic".into(), "ward".into()];
        d.distribution = 2;
        d.link = 4;
        d.reference = "1".into();
        d.trials_form = 2;
        d.trials = "n".into();
        d.weight = "w".into();
        d.offset_form = 1;
        d.offset = "0.5".into();
        d.custom_fixed = true;
        d.fixed.terms = vec![vec!["dose".into()], vec!["dose".into(), "age".into()]];
        d.intercept = false;
        d.random[0].subjects = vec!["clinic".into(), "ward".into()];
        d.random[0].terms.terms = vec![vec!["age".into()]];
        d.random[0].covariance = 3;
        d.random[0].solution = true;
        d.random.push(RandomBlock {
            subjects: vec!["ward".into()],
            ..Default::default()
        });
        d.order = 1;
        d.iterations = "40".into();
        d.ci_level = "90".into();
        d.df_method = 1;
        d.pconverge = "0.0001".into();
        assert_eq!(
            syntax(&d).unwrap(),
            "GENLINMIXED\n  /DATA_STRUCTURE SUBJECTS=clinic*ward\n  \
             /FIELDS TARGET=cured TRIALS=FIELD(n) ANALYSIS_WEIGHT=w OFFSET=VALUE(0.5)\n  \
             /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=PROBIT REFERENCE=1\n  \
             /FIXED EFFECTS=dose dose*age USE_INTERCEPT=FALSE\n  \
             /RANDOM EFFECTS=age USE_INTERCEPT=TRUE SUBJECTS=clinic*ward \
             COVARIANCE_TYPE=UNSTRUCTURED SOLUTION=TRUE\n  \
             /RANDOM USE_INTERCEPT=TRUE SUBJECTS=ward\n  \
             /BUILD_OPTIONS TARGET_CATEGORY_ORDER=DESCENDING MAX_ITERATIONS=40 \
             CONFIDENCE_LEVEL=90 DF_METHOD=SATTERTHWAITE PCONVERGE=0.0001."
        );
    }

    #[test]
    fn the_power_link_carries_its_exponent() {
        let mut d = model();
        d.distribution = 3;
        d.link = 8;
        d.power = "-1".into();
        assert!(
            syntax(&d)
                .unwrap()
                .contains("DISTRIBUTION=GAMMA LINK=POWER LINK_PARAMETER=-1")
        );
    }

    #[test]
    fn mistakes_are_named() {
        let mut d = model();
        d.random[0].subjects.clear();
        assert!(
            syntax(&d)
                .unwrap_err()
                .contains("subjects of random block 1")
        );
        let mut d = model();
        d.inputs.push("cured".into());
        assert!(syntax(&d).unwrap_err().contains("more than one role"));
        let mut d = model();
        d.distribution = 5;
        d.link = 3;
        assert!(syntax(&d).unwrap_err().contains("does not go with"));
        let mut d = model();
        d.random.clear();
        assert!(syntax(&d).is_err());
    }
}
