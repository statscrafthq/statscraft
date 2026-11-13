//! The Generalized Linear Models dialog: GENLIN of one response on factors and covariates.
//! Options left as they are add nothing to the syntax, except the distribution and the link,
//! which are always written so the model reads plainly.

use crate::advanced_dialogs::{MeansTable, Terms, covariate_values};
use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::{parse, percent};
use crate::syntax_text::{number, value};
use statscraft_data::Dataset;
use statscraft_syntax::{
    GenlinCriteria, GenlinDistribution as D, GenlinLink as L, GenlinMethod, GenlinPrint,
    default_link, distribution_keyword, link_allowed, link_keyword,
};
use std::mem::discriminant;

/// The distributions with their labels; the negative binomial's k comes from the dialog.
pub const GENLIN_DISTRIBUTIONS: [(D, &str); 7] = [
    (D::Normal, "Normal"),
    (D::Binomial, "Binomial"),
    (D::Gamma, "Gamma"),
    (D::InverseGaussian, "Inverse Gaussian"),
    (D::NegativeBinomial(Some(1.0)), "Negative binomial"),
    (D::Poisson, "Poisson"),
    (D::Multinomial, "Ordinal multinomial"),
];

/// The links with their labels; the exponent of the power links comes from the dialog.
pub const GENLIN_LINKS: [(L, &str); 15] = [
    (L::Identity, "Identity"),
    (L::Log, "Log"),
    (L::Logit, "Logit"),
    (L::Probit, "Probit"),
    (L::Cloglog, "Complementary log-log"),
    (L::Nloglog, "Negative log-log"),
    (L::Logc, "Log complement"),
    (L::Power(1.0), "Power"),
    (L::OddsPower(1.0), "Odds power"),
    (L::NegativeBinomial, "Negative binomial"),
    (L::CumLogit, "Cumulative logit"),
    (L::CumProbit, "Cumulative probit"),
    (L::CumCloglog, "Cumulative complementary log-log"),
    (L::CumNloglog, "Cumulative negative log-log"),
    (L::CumCauchit, "Cumulative Cauchit"),
];

/// The order of the categories of an ordinal dependent; the first is the default.
pub const GENLIN_ORDERS: [(&str, &str); 3] = [
    ("ASCENDING", "Ascending"),
    ("DESCENDING", "Descending"),
    ("DATA", "As they come in the data"),
];

/// The models offered.
pub const GENLIN_MODELS: [&str; 3] = ["Main effects", "Full factorial", "Custom"];

/// The estimation methods; the first, Fisher scoring for some iterations and Newton-Raphson
/// after them, is the default.
pub const GENLIN_METHODS: [(&str, &str); 3] = [
    ("", "Hybrid"),
    ("FISHER", "Fisher scoring"),
    ("NEWTON", "Newton-Raphson"),
];

/// How the scale parameter is found.
pub const GENLIN_SCALES: [(&str, &str); 4] = [
    ("MLE", "Maximum likelihood estimate"),
    ("DEVIANCE", "Deviance"),
    ("PEARSON", "Pearson chi-square"),
    ("", "Fixed value"),
];

/// The tests of model effects; the first is the default.
pub const GENLIN_ANALYSES: [(&str, &str); 3] = [
    ("3", "Type III"),
    ("1", "Type I"),
    ("ALL", "Type I and Type III"),
];

/// Keyword and label of every table on /PRINT.
pub const GENLIN_PRINT: [(&str, &str); 8] = [
    ("CPS", "Case processing summary"),
    ("DESCRIPTIVES", "Descriptive statistics"),
    ("MODELINFO", "Model information"),
    ("FIT", "Goodness of fit"),
    ("SUMMARY", "Omnibus test and tests of effects"),
    ("SOLUTION", "Parameter estimates"),
    ("COVB", "Covariance of the estimates"),
    ("CORB", "Correlation of the estimates"),
];

/// The tables GENLIN shows without /PRINT, in the order of `GENLIN_PRINT`.
fn default_print() -> [bool; 8] {
    let p = GenlinPrint::default();
    [
        p.cps,
        p.descriptives,
        p.modelinfo,
        p.fit,
        p.summary,
        p.solution,
        p.covb,
        p.corb,
    ]
}

/// How the levels of a compared factor are contrasted; the first is the default.
pub const GENLIN_CONTRASTS: [(&str, &str); 7] = [
    ("PAIRWISE", "Every pair"),
    ("SIMPLE", "Each with the last"),
    ("DEVIATION", "Each with the overall mean"),
    ("DIFFERENCE", "Each with the mean of those before"),
    ("HELMERT", "Each with the mean of those after"),
    ("REPEATED", "Each with the next"),
    ("POLYNOMIAL", "Polynomial trends"),
];

/// The adjustments of the p-values of comparisons; the first is the default.
pub const GENLIN_ADJUSTMENTS: [(&str, &str); 5] = [
    ("LSD", "LSD (none)"),
    ("BONFERRONI", "Bonferroni"),
    ("SEQBONFERRONI", "Sequential Bonferroni"),
    ("SIDAK", "Sidak"),
    ("SEQSIDAK", "Sequential Sidak"),
];

/// Keyword and label of every saved variable. The first six are the ones the ordinal
/// multinomial distribution saves.
pub const GENLIN_SAVE: [(&str, &str); 14] = [
    ("XBPRED", "Linear predictor"),
    ("XBSTDERROR", "Its standard error"),
    ("MEANPRED", "Predicted mean of the response"),
    ("CIMEANPREDL", "Lower bound of its interval"),
    ("CIMEANPREDU", "Upper bound of its interval"),
    ("PREDVAL", "Predicted category"),
    ("LEVERAGE", "Leverage"),
    ("RESID", "Raw residual"),
    ("PEARSONRESID", "Pearson residual"),
    ("DEVIANCERESID", "Deviance residual"),
    ("STDPEARSONRESID", "Standardized Pearson residual"),
    ("STDDEVIANCERESID", "Standardized deviance residual"),
    ("LIKELIHOODRESID", "Likelihood residual"),
    ("COOK", "Cook's distance"),
];

/// The saved variables of the ordinal multinomial distribution.
const MULTINOMIAL_SAVES: usize = 6;

/// GENLIN: a generalized linear model of one response.
#[derive(Clone, PartialEq, Debug)]
pub struct GeneralizedLinear {
    /// The response is a number of events out of trials, not one dependent variable.
    pub events: bool,
    /// The dependent variable, or the events variable.
    pub dependent: String,
    /// Index into `NOMREG_BASES`: the reference category of a binary dependent.
    pub reference: usize,
    pub reference_value: String,
    /// Index into `GENLIN_ORDERS`: the order of the categories of an ordinal dependent.
    pub order: usize,
    pub trials_by_variable: bool,
    pub trials_variable: String,
    pub trials: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `GENLIN_DISTRIBUTIONS`.
    pub distribution: usize,
    /// The negative binomial's k is estimated instead of `k`.
    pub estimate_k: bool,
    pub k: String,
    /// Index into `GENLIN_LINKS`.
    pub link: usize,
    /// The exponent of the power and odds power links.
    pub power: String,
    /// Index into `GENLIN_MODELS`.
    pub model: usize,
    pub terms: Terms,
    pub intercept: bool,
    pub offset: String,
    pub scale_weight: String,
    /// Index into `GENLIN_METHODS`.
    pub method: usize,
    /// The Fisher scoring iterations of the hybrid method.
    pub fisher_iterations: String,
    /// Index into `GENLIN_SCALES`.
    pub scale: usize,
    pub scale_value: String,
    pub robust: bool,
    pub ci_level: String,
    pub iterations: String,
    pub halvings: String,
    /// The parameter convergence criterion.
    pub convergence: String,
    pub kernel: bool,
    /// Index into `GENLIN_ANALYSES`.
    pub analysis: usize,
    /// Likelihood ratio tests of the effects instead of Wald tests.
    pub likelihood_ratio: bool,
    /// One flag per entry of `GENLIN_PRINT`.
    pub print: [bool; 8],
    pub exponentiated: bool,
    pub means: Vec<MeansTable>,
    /// The values of covariates for the means, by name; empty is the mean.
    pub means_at: Vec<(String, String)>,
    /// The means on the scale of the linear predictor instead of the response.
    pub transformed: bool,
    /// One flag per entry of `GENLIN_SAVE`.
    pub save: [bool; 14],
    /// User-missing values of the factors are valid.
    pub include: bool,
}

impl Default for GeneralizedLinear {
    fn default() -> Self {
        // the fields start at the command's own defaults
        let c = GenlinCriteria::default();
        Self {
            events: false,
            dependent: String::new(),
            reference: 0,
            reference_value: String::new(),
            order: 0,
            trials_by_variable: true,
            trials_variable: String::new(),
            trials: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            distribution: 0,
            estimate_k: false,
            k: "1".into(),
            link: 0,
            power: "1".into(),
            model: 0,
            terms: Terms::default(),
            intercept: true,
            offset: String::new(),
            scale_weight: String::new(),
            method: 0,
            fisher_iterations: "1".into(),
            scale: 0,
            scale_value: "1".into(),
            robust: false,
            ci_level: number(c.cilevel),
            iterations: c.maxiterations.to_string(),
            halvings: c.maxstephalving.to_string(),
            convergence: number(c.pconverge.value),
            kernel: false,
            analysis: 0,
            likelihood_ratio: false,
            print: default_print(),
            exponentiated: false,
            means: Vec::new(),
            means_at: Vec::new(),
            transformed: false,
            save: [false; 14],
            include: false,
        }
    }
}

impl GeneralizedLinear {
    pub fn model_variables(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    fn kind(&self) -> D {
        GENLIN_DISTRIBUTIONS[self.distribution].0
    }

    /// Normal, gamma and inverse Gaussian, whose scale is estimated by default.
    fn continuous(&self) -> bool {
        matches!(self.kind(), D::Normal | D::Gamma | D::InverseGaussian)
    }

    /// The dependent is categorical: binary, or ordinal, so it may be a string.
    pub fn categorical(&self) -> bool {
        !self.events && matches!(self.kind(), D::Multinomial | D::Binomial)
    }

    /// The intercept is a choice, except for the ordinal multinomial distribution, whose
    /// thresholds are always fitted.
    pub fn intercept_optional(&self) -> bool {
        self.kind() != D::Multinomial
    }

    /// The distribution with its k, checked.
    fn distribution_value(&self) -> Result<D, String> {
        match self.kind() {
            D::NegativeBinomial(_) if self.estimate_k => Ok(D::NegativeBinomial(None)),
            D::NegativeBinomial(_) => match parse(&self.k, "the negative binomial k")? {
                k if k < 0.0 => Err("the negative binomial k must be 0 or more".into()),
                k => Ok(D::NegativeBinomial(Some(k))),
            },
            other => Ok(other),
        }
    }

    /// The link with its exponent, checked.
    fn link_value(&self) -> Result<L, String> {
        Ok(match GENLIN_LINKS[self.link].0 {
            L::Power(_) => L::Power(parse(&self.power, "the exponent of the link")?),
            L::OddsPower(_) => L::OddsPower(parse(&self.power, "the exponent of the link")?),
            other => other,
        })
    }

    /// The links that go with the chosen distribution, as indexes into `GENLIN_LINKS`.
    pub fn links(&self) -> Vec<usize> {
        let d = self.distribution_value().unwrap_or(self.kind());
        (0..GENLIN_LINKS.len())
            .filter(|&i| link_allowed(d, GENLIN_LINKS[i].0))
            .collect()
    }

    /// Events out of trials or one dependent; events take the binomial distribution.
    pub fn set_events(&mut self, events: bool) {
        self.events = events;
        // an already binomial model keeps its link and scale
        if events && self.kind() != D::Binomial {
            let binomial = GENLIN_DISTRIBUTIONS
                .iter()
                .position(|(d, _)| *d == D::Binomial)
                .unwrap_or(1);
            self.set_distribution(binomial);
        }
    }

    /// Estimated marginal means exist for every distribution but the ordinal multinomial.
    pub fn means_allowed(&self) -> bool {
        self.kind() != D::Multinomial
    }

    /// Which saved variables the response and distribution give, one flag per `GENLIN_SAVE`
    /// entry: the ordinal multinomial gives the first six only, and the predicted category needs
    /// a categorical dependent.
    pub fn saves_allowed(&self) -> [bool; 14] {
        let multinomial = self.kind() == D::Multinomial;
        std::array::from_fn(|i| match i {
            5 => self.categorical(),
            i => !multinomial || i < MULTINOMIAL_SAVES,
        })
    }

    /// A new distribution with its own link and scale.
    pub fn set_distribution(&mut self, distribution: usize) {
        self.distribution = distribution;
        let link = default_link(self.kind());
        self.link = GENLIN_LINKS
            .iter()
            .position(|(l, _)| discriminant(l) == discriminant(&link))
            .unwrap_or(0);
        self.power = match link {
            L::Power(p) => number(p),
            _ => "1".into(),
        };
        self.scale = if self.continuous() { 0 } else { 3 };
        self.scale_value = "1".into();
    }

    /// The effects of the model, `a b a*b`; empty for the intercept alone.
    fn effects(&self) -> Result<String, String> {
        Ok(match self.model {
            0 => self.model_variables().join(" "),
            1 => {
                // every crossing of the factors, smallest first, then the covariates
                let n = self.factors.len();
                if n > 10 {
                    return Err(
                        "the full factorial of more than 10 factors is too large: choose custom terms"
                            .into(),
                    );
                }
                let mut masks: Vec<u32> = (1..1u32 << n).collect();
                masks.sort_by_key(|m| m.count_ones());
                masks
                    .iter()
                    .map(|m| {
                        let crossed: Vec<&str> = (0..n)
                            .filter(|i| m & (1 << i) != 0)
                            .map(|i| self.factors[i].as_str())
                            .collect();
                        crossed.join("*")
                    })
                    .chain(self.covariates.iter().cloned())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            _ if self.terms.terms.is_empty() => String::new(),
            _ => self.terms.syntax(&self.model_variables(), "model")?,
        })
    }

    /// `quoted`: the dependent is a string or shows dates or times, so a reference value is
    /// quoted.
    pub fn syntax(&self, quoted: bool) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err(match self.events {
                true => "choose the events variable".into(),
                false => "choose the dependent variable".into(),
            });
        }
        let distribution = self.distribution_value()?;
        let mut roles = vec![dependent];
        let mut text = format!("GENLIN {dependent}");
        if self.events {
            if distribution != D::Binomial {
                return Err("events out of trials need the binomial distribution".into());
            }
            let trials = if self.trials_by_variable {
                let trials = self.trials_variable.trim();
                if trials.is_empty() {
                    return Err("choose the trials variable".into());
                }
                roles.push(trials);
                trials.to_string()
            } else {
                whole(&self.trials, 1, "the number of trials")?
                    .ok_or("give the number of trials")?
                    .to_string()
            };
            text.push_str(&format!(" OF {trials}"));
        } else if distribution == D::Binomial && self.reference > 0 {
            let reference = match self.reference {
                1 => "FIRST".to_string(),
                _ => match self.reference_value.trim() {
                    "" => return Err("give the value of the reference category".into()),
                    given => value(given, quoted)?,
                },
            };
            text.push_str(&format!(" (REFERENCE={reference})"));
        } else if distribution == D::Multinomial && self.order > 0 {
            text.push_str(&format!(" (ORDER={})", GENLIN_ORDERS[self.order].0));
        }
        let offset = self.offset.trim();
        let scale_weight = self.scale_weight.trim();
        roles.extend(
            self.factors
                .iter()
                .chain(&self.covariates)
                .map(String::as_str),
        );
        roles.extend([offset, scale_weight].into_iter().filter(|n| !n.is_empty()));
        if let Some(n) =
            (1..roles.len()).find_map(|i| roles[..i].contains(&roles[i]).then_some(roles[i]))
        {
            return Err(format!("{n} has more than one role in the model"));
        }
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }

        // no effects and no intercept is a null model, such as counts fitted by an offset alone
        let mut model = Vec::new();
        let effects = self.effects()?;
        if !effects.is_empty() {
            model.push(effects);
        }
        if !self.intercept && self.intercept_optional() {
            model.push("INTERCEPT=NO".into());
        }
        if !offset.is_empty() {
            model.push(format!("OFFSET={offset}"));
        }
        if !scale_weight.is_empty() {
            model.push(format!("SCALEWEIGHT={scale_weight}"));
        }
        let mut written = distribution_keyword(distribution).to_string();
        match distribution {
            D::NegativeBinomial(None) => written.push_str("(MLE)"),
            D::NegativeBinomial(Some(k)) => written.push_str(&format!("({})", number(k))),
            _ => {}
        }
        model.push(format!("DISTRIBUTION={written}"));
        let link = self.link_value()?;
        if !link_allowed(distribution, link) {
            return Err(format!(
                "the {} link does not go with the {} distribution",
                GENLIN_LINKS[self.link].1.to_lowercase(),
                GENLIN_DISTRIBUTIONS[self.distribution].1.to_lowercase()
            ));
        }
        let mut written = link_keyword(link).to_string();
        if let L::Power(p) | L::OddsPower(p) = link {
            written.push_str(&format!("({})", number(p)));
        }
        model.push(format!("LINK={written}"));
        text.push_str(&format!("\n  /MODEL {}", model.join(" ")));

        let criteria = self.criteria()?;
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }

        if !self.means.is_empty() && distribution == D::Multinomial {
            return Err(
                "the ordinal multinomial distribution has no estimated marginal means".into(),
            );
        }
        // the values count only for tables of means
        let control: Vec<String> = match self.means.is_empty() {
            true => Vec::new(),
            false => covariate_values(&self.covariates, &self.means_at)?
                .into_iter()
                .map(|(n, v)| format!("{n}({v})"))
                .collect(),
        };
        for (i, table) in self.means.iter().enumerate() {
            let (crossed, compare) = table.checked(&self.factors, i + 1)?;
            let mut parts = Vec::new();
            if !crossed.is_empty() {
                parts.push(format!("TABLES={}", crossed.join("*")));
            }
            if !control.is_empty() {
                parts.push(format!("CONTROL={}", control.join(" ")));
            }
            if self.transformed {
                parts.push("SCALE=TRANSFORMED".into());
            }
            if let Some(c) = compare {
                parts.push(format!("COMPARE={c}"));
                if table.contrast > 0 {
                    parts.push(format!("CONTRAST={}", GENLIN_CONTRASTS[table.contrast].0));
                }
                if table.adjustment > 0 {
                    parts.push(format!(
                        "PADJUST={}",
                        GENLIN_ADJUSTMENTS[table.adjustment].0
                    ));
                }
            }
            text.push_str("\n  /EMMEANS");
            if !parts.is_empty() {
                text.push_str(&format!(" {}", parts.join(" ")));
            }
        }

        if self.include {
            text.push_str("\n  /MISSING CLASSMISSING=INCLUDE");
        }
        let exponentiated = self.exponentiated && self.print[5];
        if self.print != default_print() || exponentiated {
            let print: Vec<&str> = keywords(&GENLIN_PRINT, &self.print)
                .into_iter()
                .map(|k| match k {
                    "SOLUTION" if exponentiated => "SOLUTION(EXPONENTIATED)",
                    k => k,
                })
                .collect();
            let print = match print.is_empty() {
                true => "NONE".to_string(),
                false => print.join(" "),
            };
            text.push_str(&format!("\n  /PRINT {print}"));
        }
        let save = keywords(&GENLIN_SAVE, &self.save);
        if distribution == D::Multinomial && self.save[MULTINOMIAL_SAVES..].contains(&true) {
            return Err(
                "the ordinal multinomial distribution saves only the predictions, their intervals and the predicted category"
                    .into(),
            );
        }
        if self.save[5] && !self.categorical() {
            return Err(
                "the predicted category needs a binary dependent with the binomial distribution, or the ordinal multinomial one"
                    .into(),
            );
        }
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// The CRITERIA settings that differ from the defaults.
    fn criteria(&self) -> Result<Vec<String>, String> {
        let c = GenlinCriteria::default();
        let mut out = Vec::new();
        if self.analysis > 0 || self.likelihood_ratio {
            let lr = if self.likelihood_ratio { "(LR)" } else { "" };
            out.push(format!(
                "ANALYSISTYPE={}{lr}",
                GENLIN_ANALYSES[self.analysis].0
            ));
        }
        // an empty field means the default, as in the other dialogs
        let level = match self.ci_level.trim() {
            "" => c.cilevel,
            given => percent(given)?,
        };
        if level != c.cilevel {
            out.push(format!("CILEVEL={}", number(level)));
        }
        if self.robust {
            out.push("COVB=ROBUST".into());
        }
        if self.kernel {
            out.push("LIKELIHOOD=KERNEL".into());
        }
        match whole(&self.iterations, 0, "the maximum number of iterations")? {
            Some(n) if n != c.maxiterations => out.push(format!("MAXITERATIONS={n}")),
            _ => {}
        }
        match whole(&self.halvings, 1, "the maximum step-halving")? {
            Some(n) if n != c.maxstephalving => out.push(format!("MAXSTEPHALVING={n}")),
            _ => {}
        }
        match self.method {
            0 => match whole(&self.fisher_iterations, 1, "the Fisher scoring iterations")? {
                Some(n) if GenlinMethod::Fisher(n) != c.method => {
                    out.push(format!("METHOD=FISHER({n})"))
                }
                _ => {}
            },
            m => out.push(format!("METHOD={}", GENLIN_METHODS[m].0)),
        }
        let convergence = match self.convergence.trim() {
            "" => c.pconverge.value,
            given => parse(given, "the parameter convergence")?,
        };
        if convergence <= 0.0 {
            return Err("the parameter convergence must be above 0".into());
        }
        if convergence != c.pconverge.value {
            out.push(format!("PCONVERGE={}", number(convergence)));
        }
        match self.scale {
            0 if !self.continuous() => {
                return Err(
                    "the maximum likelihood scale is for the normal, gamma and inverse Gaussian distributions"
                        .into(),
                );
            }
            0 => {}
            3 => {
                let v = parse(&self.scale_value, "the scale value")?;
                if v <= 0.0 {
                    return Err("the scale value must be above 0".into());
                }
                if self.continuous() || v != 1.0 {
                    out.push(format!("SCALE={}", number(v)));
                }
            }
            s => out.push(format!("SCALE={}", GENLIN_SCALES[s].0)),
        }
        Ok(out)
    }
}

/// The checks that need the data: numbers where the command needs them, and no role for the
/// frequency weight. True when a reference value is quoted: the dependent is a string or shows
/// dates or times.
pub fn data_checks(d: &GeneralizedLinear, data: &Dataset) -> Result<bool, String> {
    // only the fields the syntax uses
    let trials = (d.events && d.trials_by_variable).then_some(&d.trials_variable);
    let mut numeric: Vec<&String> = d.covariates.iter().collect();
    numeric.extend([&d.offset, &d.scale_weight].into_iter().chain(trials));
    if !d.categorical() {
        numeric.push(&d.dependent);
    }
    crate::stats_dialogs::numeric(data, numeric)?;
    if let Some(w) = data.weight_variable()
        && let Some(n) = d
            .model_variables()
            .iter()
            .chain(
                [&d.dependent, &d.offset, &d.scale_weight]
                    .into_iter()
                    .chain(trials),
            )
            .find(|n| n.trim() == w)
    {
        return Err(format!(
            "{n} weights the cases, so it cannot be in the model as well"
        ));
    }
    let dependent = d.dependent.trim();
    match data.variable(dependent) {
        Ok(v) => Ok(v.is_string() || crate::edits::shows_date(v.format())),
        Err(_) if dependent.is_empty() => Ok(false),
        Err(_) => Err(format!("{dependent} is no longer a variable")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, runs, v};

    fn terms(list: &[&[&str]]) -> Terms {
        Terms {
            terms: list.iter().map(|t| v(t)).collect(),
            building: Vec::new(),
        }
    }

    #[test]
    fn a_normal_model_with_its_defaults() {
        let mut d = GeneralizedLinear::default();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "choose the dependent variable"
        );
        d.dependent = "monthly_spend".into();
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN monthly_spend\n  /MODEL DISTRIBUTION=NORMAL LINK=IDENTITY."
        );
        assert!(runs(&syntax) >= 3);
        d.factors = v(&["gender", "region"]);
        d.covariates = v(&["age"]);
        d.model = 1;
        assert_eq!(
            d.syntax(false).unwrap(),
            "GENLIN monthly_spend BY gender region WITH age\n  \
             /MODEL gender region gender*region age DISTRIBUTION=NORMAL LINK=IDENTITY."
        );
    }

    #[test]
    fn a_binary_dependent_with_every_option() {
        let mut d = GeneralizedLinear {
            dependent: "q5_1".into(),
            factors: v(&["gender", "region"]),
            covariates: v(&["age"]),
            ..Default::default()
        };
        d.set_distribution(1);
        assert_eq!(GENLIN_LINKS[d.link].1, "Logit");
        assert_eq!(d.scale, 3);
        d.link = 3;
        d.reference = 1;
        d.model = 2;
        d.terms = terms(&[&["gender"], &["age"], &["gender", "age"]]);
        d.method = 2;
        d.robust = true;
        d.ci_level = "90".into();
        d.iterations = "50".into();
        d.halvings = "10".into();
        d.convergence = "0.0001".into();
        d.kernel = true;
        d.analysis = 2;
        d.likelihood_ratio = true;
        d.scale = 2;
        d.print = [true; 8];
        d.exponentiated = true;
        d.means = vec![
            MeansTable::default(),
            MeansTable {
                factors: v(&["region", "gender"]),
                compare: "gender".into(),
                contrast: 1,
                adjustment: 3,
            },
        ];
        d.means_at = vec![("age".into(), "40".into())];
        d.transformed = true;
        d.save = [true; 14];
        d.include = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q5_1 (REFERENCE=FIRST) BY gender region WITH age\n  \
             /MODEL gender age gender*age DISTRIBUTION=BINOMIAL LINK=PROBIT\n  \
             /CRITERIA ANALYSISTYPE=ALL(LR) CILEVEL=90 COVB=ROBUST LIKELIHOOD=KERNEL \
             MAXITERATIONS=50 MAXSTEPHALVING=10 METHOD=NEWTON PCONVERGE=0.0001 SCALE=PEARSON\n  \
             /EMMEANS CONTROL=age(40) SCALE=TRANSFORMED\n  \
             /EMMEANS TABLES=gender*region CONTROL=age(40) SCALE=TRANSFORMED COMPARE=gender \
             CONTRAST=SIMPLE PADJUST=SIDAK\n  \
             /MISSING CLASSMISSING=INCLUDE\n  \
             /PRINT CPS DESCRIPTIVES MODELINFO FIT SUMMARY SOLUTION(EXPONENTIATED) COVB CORB\n  \
             /SAVE XBPRED XBSTDERROR MEANPRED CIMEANPREDL CIMEANPREDU PREDVAL LEVERAGE RESID \
             PEARSONRESID DEVIANCERESID STDPEARSONRESID STDDEVIANCERESID LIKELIHOODRESID COOK."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 8);
        let data = session.dataset().unwrap();
        assert!(data.variable("CooksDistance").is_ok());

        // a custom reference value, quoted for a string dependent
        d.reference = 2;
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "give the value of the reference category"
        );
        d.reference_value = "1".into();
        assert!(
            d.syntax(false)
                .unwrap()
                .starts_with("GENLIN q5_1 (REFERENCE=1) BY")
        );
        assert!(
            d.syntax(true)
                .unwrap()
                .starts_with("GENLIN q5_1 (REFERENCE='1') BY")
        );
        d.means[1].compare = "age".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "estimated marginal means table 2 compares age, which is not in the table"
        );
    }

    #[test]
    fn events_out_of_trials() {
        let mut d = GeneralizedLinear {
            events: true,
            dependent: "q5_1".into(),
            trials_by_variable: false,
            trials: "2".into(),
            factors: v(&["gender"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "events out of trials need the binomial distribution"
        );
        d.set_distribution(1);
        d.link = 4;
        // the reference category belongs to a binary dependent, not to events
        d.reference = 1;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q5_1 OF 2 BY gender\n  /MODEL gender DISTRIBUTION=BINOMIAL LINK=CLOGLOG."
        );
        assert!(runs(&syntax) >= 3);
        d.trials = "0".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the number of trials must be a whole number of at least 1"
        );
        d.trials_by_variable = true;
        assert_eq!(d.syntax(false).unwrap_err(), "choose the trials variable");
        d.trials_variable = "gender".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "gender has more than one role in the model"
        );
        d.save[5] = true;
        d.trials_variable = "q2_recommend".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the predicted category needs a binary dependent with the binomial distribution, or the ordinal multinomial one"
        );
    }

    #[test]
    fn counts_and_ordinal_responses() {
        // a negative binomial with k estimated, an offset and a power link
        let mut d = GeneralizedLinear {
            dependent: "q2_recommend".into(),
            covariates: v(&["age"]),
            offset: "q5_2".into(),
            ..Default::default()
        };
        d.set_distribution(4);
        assert_eq!(GENLIN_LINKS[d.link].1, "Log");
        d.estimate_k = true;
        d.link = 7;
        d.power = "0.5".into();
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q2_recommend WITH age\n  \
             /MODEL age OFFSET=q5_2 DISTRIBUTION=NEGBIN(MLE) LINK=POWER(0.5)."
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{errors:?}");
        // k fixed at 0 takes no negative binomial link
        d.estimate_k = false;
        d.k = "0".into();
        d.link = 9;
        assert!(!d.links().contains(&9));
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the negative binomial link does not go with the negative binomial distribution"
        );
        d.set_distribution(5);
        d.scale = 0;
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the maximum likelihood scale is for the normal, gamma and inverse Gaussian distributions"
        );
        d.scale = 3;
        d.scale_value = "2".into();
        d.offset.clear();
        let syntax = d.syntax(false).unwrap();
        assert!(syntax.ends_with("/CRITERIA SCALE=2."), "{syntax}");
        assert!(runs(&syntax) >= 3);

        // an ordinal dependent: only cumulative links, the category order, no means
        let mut d = GeneralizedLinear {
            dependent: "q1_satisfaction".into(),
            factors: v(&["gender"]),
            order: 1,
            ..Default::default()
        };
        d.set_distribution(6);
        assert_eq!(GENLIN_LINKS[d.link].1, "Cumulative logit");
        assert!(d.links().iter().all(|&l| GENLIN_LINKS[l].0.is_cumulative()));
        d.save[0] = true;
        d.save[5] = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q1_satisfaction (ORDER=DESCENDING) BY gender\n  \
             /MODEL gender DISTRIBUTION=MULTINOMIAL LINK=CUMLOGIT\n  /SAVE XBPRED PREDVAL."
        );
        assert!(runs(&syntax) >= 3);
        d.save[7] = true;
        assert!(d.syntax(false).is_err());
        d.save[7] = false;
        d.means.push(MeansTable::default());
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the ordinal multinomial distribution has no estimated marginal means"
        );
    }

    #[test]
    fn the_intercept_alone_or_nothing() {
        let mut d = GeneralizedLinear {
            dependent: "monthly_spend".into(),
            factors: v(&["gender"]),
            model: 2,
            ..Default::default()
        };
        assert_eq!(
            d.syntax(false).unwrap(),
            "GENLIN monthly_spend BY gender\n  /MODEL DISTRIBUTION=NORMAL LINK=IDENTITY."
        );
        d.intercept = false;
        assert_eq!(
            d.syntax(false).unwrap(),
            "GENLIN monthly_spend BY gender\n  /MODEL INTERCEPT=NO DISTRIBUTION=NORMAL LINK=IDENTITY."
        );
        d.model = 0;
        d.method = 0;
        d.fisher_iterations = "3".into();
        d.convergence = "0".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the parameter convergence must be above 0"
        );
        d.convergence = "1E-6".into();
        assert_eq!(
            d.syntax(false).unwrap(),
            "GENLIN monthly_spend BY gender\n  \
             /MODEL gender INTERCEPT=NO DISTRIBUTION=NORMAL LINK=IDENTITY\n  \
             /CRITERIA METHOD=FISHER(3)."
        );
    }

    #[test]
    fn a_null_model_with_an_offset_runs() {
        let mut d = GeneralizedLinear {
            dependent: "q2_recommend".into(),
            model: 2,
            intercept: false,
            offset: "q5_2".into(),
            ..Default::default()
        };
        d.set_distribution(5);
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q2_recommend\n  /MODEL INTERCEPT=NO OFFSET=q5_2 DISTRIBUTION=POISSON LINK=LOG."
        );
        assert!(runs(&syntax) >= 3);
    }

    #[test]
    fn a_reference_value_of_a_date_is_quoted() {
        let session = open("survey_basic.sav");
        let data = session.dataset().unwrap();
        let mut d = GeneralizedLinear {
            dependent: "signup_date".into(),
            reference: 2,
            reference_value: "01-JAN-2024".into(),
            ..Default::default()
        };
        d.set_distribution(1);
        let quoted = data_checks(&d, data).unwrap();
        assert!(quoted);
        assert!(
            d.syntax(quoted)
                .unwrap()
                .starts_with("GENLIN signup_date (REFERENCE='01-JAN-2024')")
        );
    }

    #[test]
    fn events_take_the_binomial_distribution() {
        let mut d = GeneralizedLinear::default();
        d.set_events(true);
        assert_eq!(GENLIN_DISTRIBUTIONS[d.distribution].0, D::Binomial);
        assert_eq!(GENLIN_LINKS[d.link].1, "Logit");
        assert_eq!(d.scale, 3);
        d.set_events(false);
        assert!(!d.events);
        assert_eq!(GENLIN_DISTRIBUTIONS[d.distribution].0, D::Binomial);
    }

    #[test]
    fn events_keep_a_binomial_link_and_scale() {
        let mut d = GeneralizedLinear::default();
        d.set_distribution(1);
        d.link = 3;
        d.scale = 2;
        d.set_events(true);
        assert_eq!(GENLIN_LINKS[d.link].1, "Probit");
        assert_eq!(d.scale, 2);
    }

    #[test]
    fn the_dialog_offers_only_what_the_model_gives() {
        let mut d = GeneralizedLinear::default();
        assert!(d.means_allowed());
        // a normal response has no predicted category
        assert_eq!(d.saves_allowed().iter().filter(|a| !**a).count(), 1);
        assert!(!d.saves_allowed()[5]);
        d.set_distribution(6);
        assert!(!d.means_allowed());
        assert_eq!(d.saves_allowed(), std::array::from_fn(|i| i < 6));
        d.set_distribution(1);
        assert_eq!(d.saves_allowed(), [true; 14]);
        d.set_events(true);
        assert!(!d.saves_allowed()[5]);
    }

    #[test]
    fn the_ordinal_multinomial_always_has_its_thresholds() {
        let mut d = GeneralizedLinear {
            dependent: "q1_satisfaction".into(),
            factors: v(&["gender"]),
            model: 2,
            intercept: false,
            ..Default::default()
        };
        d.set_distribution(6);
        assert!(!d.intercept_optional());
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "GENLIN q1_satisfaction BY gender\n  /MODEL DISTRIBUTION=MULTINOMIAL LINK=CUMLOGIT."
        );
        assert!(runs(&syntax) >= 3);
    }

    #[test]
    fn an_events_response_is_never_categorical() {
        let mut d = GeneralizedLinear {
            events: true,
            dependent: "plan".into(),
            trials_by_variable: false,
            trials: "3".into(),
            ..Default::default()
        };
        d.set_distribution(6);
        assert!(!d.categorical());
        assert!(!d.saves_allowed()[5]);
        let session = open("survey_basic.sav");
        assert_eq!(
            data_checks(&d, session.dataset().unwrap()).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
    }

    #[test]
    fn covariate_values_count_only_with_tables_of_means() {
        let mut d = GeneralizedLinear {
            dependent: "monthly_spend".into(),
            covariates: v(&["age"]),
            means_at: vec![("age".into(), "x".into())],
            ..Default::default()
        };
        assert!(d.syntax(false).is_ok());
        d.means.push(MeansTable::default());
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the value of age for the means must be a number"
        );
    }

    #[test]
    fn unused_fields_are_not_checked() {
        let mut session = open("survey_basic.sav");
        session.run("WEIGHT BY weight.", None).unwrap();
        let data = session.dataset().unwrap();
        // a trials variable left over from events out of trials
        let mut d = GeneralizedLinear {
            dependent: "monthly_spend".into(),
            trials_variable: "weight".into(),
            ..Default::default()
        };
        assert_eq!(data_checks(&d, data), Ok(false));
        d.trials_variable = "plan".into();
        assert_eq!(data_checks(&d, data), Ok(false));
        d.dependent = "q5_1".into();
        d.set_events(true);
        d.trials_by_variable = false;
        assert_eq!(data_checks(&d, data), Ok(false));
        d.trials_by_variable = true;
        assert_eq!(
            data_checks(&d, data).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
    }

    #[test]
    fn the_defaults_are_the_command_s_own() {
        let d = GeneralizedLinear {
            dependent: "monthly_spend".into(),
            ..Default::default()
        };
        assert_eq!(d.ci_level, "95");
        assert_eq!(d.convergence, "0.000001");
        assert!(!d.syntax(false).unwrap().contains("CRITERIA"));
        assert_eq!(d.print, [true, true, true, true, true, true, false, false]);
    }

    #[test]
    fn a_dependent_taken_out_of_the_data_is_reported() {
        let session = open("survey_basic.sav");
        let mut d = GeneralizedLinear {
            dependent: "gone".into(),
            ..Default::default()
        };
        d.set_distribution(1);
        assert_eq!(
            data_checks(&d, session.dataset().unwrap()).unwrap_err(),
            "gone is no longer a variable"
        );
    }

    #[test]
    fn variables_are_checked_against_the_data() {
        let session = open("survey_basic.sav");
        let data = session.dataset().unwrap();
        let mut d = GeneralizedLinear {
            dependent: "plan".into(),
            ..Default::default()
        };
        assert_eq!(
            data_checks(&d, data).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
        d.set_distribution(1);
        assert_eq!(data_checks(&d, data), Ok(true));
        d.covariates = v(&["q3_comment"]);
        assert!(data_checks(&d, data).is_err());
        let mut session = open("survey_basic.sav");
        session.run("WEIGHT BY weight.", None).unwrap();
        d.covariates = v(&["weight"]);
        assert_eq!(
            data_checks(&d, session.dataset().unwrap()).unwrap_err(),
            "weight weights the cases, so it cannot be in the model as well"
        );
    }
}
