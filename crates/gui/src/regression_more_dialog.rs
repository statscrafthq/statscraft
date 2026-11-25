//! The Curve Estimation (CURVEFIT), Probit Analysis (PROBIT) and Weight Estimation (WLS)
//! dialogs. Options left as they are add nothing to the syntax, so the command runs with its own
//! defaults.

use crate::advanced_dialogs::ci_level;
use crate::analyze_dialogs::keywords;
use crate::correspondence_dialog::TableVariable;
use crate::i18n::{t, t_args};
use crate::loglinear_dialog::{Criterion, ITERATIONS, POSITIVE, criteria, optional};
use crate::stats_dialogs::{numeric, parse};
use crate::survival_dialog::numbers;
use crate::syntax_text::number;
use statscraft_data::Dataset;

/// The eleven curves of /MODEL, in the command's order; the index of the logistic one is
/// `CURVE_LOGISTIC`.
pub const CURVE_MODELS: [(&str, &str); 11] = [
    ("LINEAR", "stats-regmore-linear"),
    ("LOGARITHMIC", "stats-regmore-logarithmic"),
    ("INVERSE", "stats-regmore-inverse"),
    ("QUADRATIC", "stats-regmore-quadratic"),
    ("CUBIC", "stats-regmore-cubic"),
    ("COMPOUND", "stats-regmore-compound"),
    ("POWER", "stats-regmore-power"),
    ("S", "S"),
    ("GROWTH", "stats-regmore-growth"),
    ("EXPONENTIAL", "stats-regmore-exponential"),
    ("LGSTIC", "stats-regmore-logistic"),
];

pub const CURVE_LOGISTIC: usize = 10;

/// The new variables of CURVEFIT /SAVE; the index of the intervals is `CURVE_CIN`.
pub const CURVE_SAVE: [(&str, &str); 3] = [
    ("PRED", "stats-regmore-predicted-values"),
    ("RESID", "stats-regmore-residuals"),
    ("CIN", "stats-regmore-prediction-intervals"),
];

pub const CURVE_CIN: usize = 2;

/// CURVEFIT: curves of one or more dependents on one independent or on the case order.
#[derive(Clone, PartialEq, Debug)]
pub struct CurveEstimation {
    pub dependents: Vec<String>,
    pub independent: String,
    /// The case sequence is the independent; `independent` is then not used.
    pub sequence: bool,
    /// The variable that labels the points of the chart; may be empty.
    pub id: String,
    /// One flag per entry of `CURVE_MODELS`.
    pub models: [bool; 11],
    /// The upper bound of the logistic curve; empty for none.
    pub upper_bound: String,
    pub constant: bool,
    pub anova: bool,
    pub plot: bool,
    /// One flag per entry of `CURVE_SAVE`.
    pub save: [bool; 3],
    /// The level of the saved prediction intervals in percent.
    pub ci_level: String,
}

impl Default for CurveEstimation {
    fn default() -> Self {
        let mut models = [false; 11];
        models[0] = true;
        Self {
            dependents: Vec::new(),
            independent: String::new(),
            sequence: false,
            id: String::new(),
            models,
            upper_bound: String::new(),
            constant: true,
            anova: false,
            plot: true,
            save: [false; 3],
            ci_level: "95".into(),
        }
    }
}

/// An error naming the first variable given in two roles.
fn one_role_each(names: &[&str]) -> Result<(), String> {
    match names
        .iter()
        .enumerate()
        .find(|(i, n)| names[..*i].iter().any(|m| m.eq_ignore_ascii_case(n)))
    {
        Some((_, name)) => Err(t_args(
            "stats-regmore-two-roles",
            &[("name", (*name).into())],
        )),
        None => Ok(()),
    }
}

/// `/KEYWORD=value` when there is a value.
fn sub(keyword: &str, value: &str) -> String {
    match value.is_empty() {
        true => String::new(),
        false => format!("\n  /{keyword}={value}"),
    }
}

impl CurveEstimation {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.dependents.is_empty() {
            return Err(t("stats-regmore-add-dependent").into());
        }
        let independent = match self.sequence {
            true => "",
            false => self.independent.trim(),
        };
        if !self.sequence && independent.is_empty() {
            return Err(t("stats-regmore-choose-independent").into());
        }
        let mut variables: Vec<String> = self.dependents.clone();
        if !independent.is_empty() {
            variables.push(independent.to_string());
        }
        one_role_each(&variables.iter().map(String::as_str).collect::<Vec<_>>())?;
        numeric(data, &variables)?;
        if !self.models.contains(&true) {
            return Err(t("stats-regmore-choose-model").into());
        }
        let mut text = format!("CURVEFIT VARIABLES={}", self.dependents.join(" "));
        if !independent.is_empty() {
            text.push_str(&format!(" WITH {independent}"));
        }
        let models = keywords(&CURVE_MODELS, &self.models);
        if models != ["LINEAR"] {
            text.push_str(&sub("MODEL", &models.join(" ")));
        }
        // the bound counts only for the logistic curve
        if self.models[CURVE_LOGISTIC]
            && let Some(bound) = optional(&self.upper_bound, t("stats-regmore-upper-bound"))?
        {
            let largest = self.largest(data);
            if bound <= 0.0 || largest.is_some_and(|m| bound <= m) {
                return Err(match largest {
                    Some(m) => t_args(
                        "stats-regmore-upper-bound-above-largest",
                        &[("value", number(m.max(0.0)).into())],
                    ),
                    None => t("stats-regmore-upper-bound-above-0").into(),
                });
            }
            text.push_str(&sub("UPPERBOUND", &number(bound)));
        }
        if !self.constant {
            text.push_str("\n  /NOCONSTANT");
        }
        if self.anova {
            text.push_str("\n  /PRINT=ANOVA");
        }
        if !self.plot {
            text.push_str("\n  /PLOT=NONE");
        }
        // the level counts only for saved intervals
        if self.save[CURVE_CIN] {
            let level = ci_level(&self.ci_level)?;
            if level != 95.0 {
                text.push_str(&sub("CIN", &number(level)));
            }
        }
        text.push_str(&sub("ID", self.id.trim()));
        text.push_str(&sub("SAVE", &keywords(&CURVE_SAVE, &self.save).join(" ")));
        text.push('.');
        Ok(text)
    }

    /// The largest valid value of the dependents, user-missing values left out.
    fn largest(&self, data: &Dataset) -> Option<f64> {
        let mut out: Option<f64> = None;
        for name in &self.dependents {
            let Ok(v) = data.variable(name) else { continue };
            let Ok(values) = v.numeric() else { continue };
            for x in values.iter().flatten() {
                if !v.is_user_missing(x) {
                    out = Some(out.map_or(x, |m| m.max(x)));
                }
            }
        }
        out
    }
}

/// The response models of PROBIT /MODEL; the first is the default.
pub const PROBIT_MODELS: [(&str, &str); 3] = [
    ("PROBIT", "Probit"),
    ("LOGIT", "Logit"),
    ("BOTH", "stats-regmore-both-logit-first"),
];

/// The transformations of the covariates on /LOG; the first is the default, the index of the
/// typed base is `PROBIT_LOG_BASE`.
pub const PROBIT_LOGS: [(&str, &str); 4] = [
    ("10", "stats-regmore-log-base-10"),
    ("E", "stats-regmore-log-natural"),
    ("BASE", "stats-regmore-log-this-base"),
    ("NONE", "stats-regmore-none"),
];

pub const PROBIT_LOG_BASE: usize = 2;

/// The tables of /PRINT; the first three are the default.
pub const PROBIT_PRINT: [(&str, &str); 4] = [
    ("FREQ", "stats-regmore-observed-expected"),
    ("CI", "stats-regmore-effective-levels"),
    ("RMP", "stats-regmore-relative-median-potency"),
    ("PARALL", "stats-regmore-parallelism-test"),
];

/// PROBIT: responses out of subjects, on covariates and an optional factor with its range.
#[derive(Clone, PartialEq, Debug)]
pub struct Probit {
    pub response: String,
    pub total: String,
    /// The factor and its smallest and largest level; an empty name for none.
    pub factor: TableVariable,
    pub covariates: Vec<String>,
    /// An index into `PROBIT_MODELS`.
    pub model: usize,
    /// An index into `PROBIT_LOGS`.
    pub log: usize,
    pub log_base: String,
    /// The natural response rate: 0 none, 1 estimated, 2 fixed at `natres_value`.
    pub natres: usize,
    pub natres_value: String,
    /// Empty: the command's own criterion.
    pub converge: String,
    /// Empty: the command's own limit, which depends on the number of parameters.
    pub iterations: String,
    /// The significance of the goodness-of-fit test below which limits use the heterogeneity
    /// factor.
    pub heterogeneity: String,
    /// One flag per entry of `PROBIT_PRINT`.
    pub print: [bool; 4],
    pub include: bool,
}

impl Default for Probit {
    fn default() -> Self {
        Self {
            response: String::new(),
            total: String::new(),
            factor: TableVariable::default(),
            covariates: Vec::new(),
            model: 0,
            log: 0,
            log_base: String::new(),
            natres: 0,
            natres_value: String::new(),
            converge: String::new(),
            iterations: String::new(),
            heterogeneity: "0.15".into(),
            print: [true, true, true, false],
            include: false,
        }
    }
}

impl Probit {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let response = self.response.trim();
        let total = self.total.trim();
        let factor = self.factor.name.trim();
        if response.is_empty() {
            return Err(t("stats-regmore-choose-response").into());
        }
        if total.is_empty() {
            return Err(t("stats-regmore-choose-total").into());
        }
        if self.covariates.is_empty() {
            return Err(t("stats-regmore-add-covariate").into());
        }
        let mut variables = vec![response.to_string(), total.to_string()];
        if !factor.is_empty() {
            variables.push(factor.to_string());
        }
        variables.extend(self.covariates.iter().cloned());
        one_role_each(&variables.iter().map(String::as_str).collect::<Vec<_>>())?;
        numeric(data, &variables)?;
        let mut text = format!(
            "PROBIT {response} OF {total} WITH {}",
            self.covariates.join(" ")
        );
        if !factor.is_empty() {
            let (min, max) = self.factor.range()?;
            text.push_str(&format!(" BY {factor}({min},{max})"));
        }
        if self.model != 0 {
            text.push_str(&sub("MODEL", PROBIT_MODELS[self.model].0));
        }
        match self.log {
            1 => text.push_str("\n  /LOG"),
            PROBIT_LOG_BASE => {
                let base = parse(&self.log_base, t("stats-regmore-log-base"))?;
                if base <= 0.0 || base == 1.0 {
                    return Err(t("stats-regmore-log-base-invalid").into());
                }
                text.push_str(&sub("LOG", &number(base)));
            }
            3 => text.push_str("\n  /LOG=NONE"),
            _ => {}
        }
        let fields: [Criterion; 3] = [
            // no default to compare with: the command's own is far below what anyone types
            (
                "CONVERGE",
                &self.converge,
                f64::NAN,
                t("stats-regmore-convergence-criterion"),
                POSITIVE,
            ),
            // nor here: the default grows with the number of parameters
            (
                "ITERATE",
                &self.iterations,
                f64::NAN,
                t("stats-regmore-maximum-iterations"),
                ITERATIONS,
            ),
            (
                "P",
                &self.heterogeneity,
                0.15,
                t("stats-regmore-heterogeneity-significance"),
                (|v| (0.0..=1.0).contains(&v), t("stats-regmore-from-0-to-1")),
            ),
        ];
        text.push_str(&criteria(&fields)?);
        match self.natres {
            1 => text.push_str("\n  /NATRES"),
            2 => {
                let rate = parse(&self.natres_value, t("stats-regmore-natural-rate"))?;
                if !(0.0..1.0).contains(&rate) {
                    return Err(t("stats-regmore-natural-rate-invalid").into());
                }
                text.push_str(&sub("NATRES", &number(rate)));
            }
            _ => {}
        }
        if self.print != Probit::default().print {
            let print = keywords(&PROBIT_PRINT, &self.print);
            text.push_str(&match print.is_empty() {
                true => sub("PRINT", "NONE"),
                false => sub("PRINT", &print.join(" ")),
            });
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

/// The most powers the command takes.
const MOST_POWERS: usize = 150;

/// WLS: one dependent on independents, weighted by a power of a source variable.
#[derive(Clone, PartialEq, Debug)]
pub struct WeightEstimation {
    pub dependent: String,
    pub independents: Vec<String>,
    /// Empty: the first independent.
    pub source: String,
    /// The powers are a list of values rather than a range.
    pub power_list: bool,
    pub from: String,
    pub to: String,
    pub by: String,
    /// Values separated by spaces or commas.
    pub powers: String,
    pub constant: bool,
    /// Coefficients for every power, not only the best.
    pub print_all: bool,
    pub save_weight: bool,
}

impl Default for WeightEstimation {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            independents: Vec::new(),
            source: String::new(),
            power_list: false,
            from: String::new(),
            to: String::new(),
            by: String::new(),
            powers: String::new(),
            constant: true,
            print_all: false,
            save_weight: false,
        }
    }
}

impl WeightEstimation {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let dependent = self.dependent.trim();
        let source = self.source.trim();
        if dependent.is_empty() {
            return Err(t("stats-regmore-choose-dependent").into());
        }
        if self.independents.is_empty() {
            return Err(t("stats-regmore-add-independent").into());
        }
        let mut variables = vec![dependent.to_string()];
        variables.extend(self.independents.iter().cloned());
        one_role_each(&variables.iter().map(String::as_str).collect::<Vec<_>>())?;
        if source.eq_ignore_ascii_case(dependent) {
            return Err(t_args(
                "stats-regmore-is-dependent",
                &[("name", dependent.into())],
            ));
        }
        variables.push(source.to_string());
        numeric(data, &variables)?;
        let mut text = format!(
            "WLS VARIABLES={dependent} WITH {}",
            self.independents.join(" ")
        );
        text.push_str(&sub("SOURCE", source));
        text.push_str(&sub("POWER", &self.power_syntax()?));
        if !self.constant {
            text.push_str("\n  /NOCONSTANT");
        }
        if self.print_all {
            text.push_str("\n  /PRINT=ALL");
        }
        if self.save_weight {
            text.push_str("\n  /SAVE=WEIGHT");
        }
        text.push('.');
        Ok(text)
    }

    /// The powers as /POWER takes them; empty for the command's own power of 1.
    fn power_syntax(&self) -> Result<String, String> {
        if self.power_list {
            let list = numbers(&self.powers, t("stats-regmore-each-power"))?;
            if list.len() > MOST_POWERS {
                return Err(t_args(
                    "stats-regmore-most-powers",
                    &[("count", MOST_POWERS.to_string().into())],
                ));
            }
            return Ok(list.into_iter().map(number).collect::<Vec<_>>().join(" "));
        }
        let fields = [&self.from, &self.to, &self.by];
        if fields.iter().all(|f| f.trim().is_empty()) {
            return Ok(String::new());
        }
        let from = parse(&self.from, t("stats-regmore-first-power"))?;
        let to = parse(&self.to, t("stats-regmore-last-power"))?;
        let by = parse(&self.by, t("stats-regmore-step"))?;
        if by <= 0.0 {
            return Err(t("stats-regmore-step-above-0").into());
        }
        if to < from {
            return Err(t("stats-regmore-last-below-first").into());
        }
        // counted as the parser counts them, allowing for rounding in the step
        if ((to - from) / by + 1e-9).floor() >= MOST_POWERS as f64 {
            return Err(t_args(
                "stats-regmore-range-too-many",
                &[("count", MOST_POWERS.to_string().into())],
            ));
        }
        Ok(format!(
            "{} TO {} BY {}",
            number(from),
            number(to),
            number(by)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    #[test]
    fn curve_estimation_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut CurveEstimation)| {
            let mut d = CurveEstimation {
                dependents: v(&["monthly_spend"]),
                independent: "age".into(),
                ..Default::default()
            };
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.dependents.clear()),
            "add at least one dependent variable"
        );
        assert_eq!(
            err(&|d| d.independent.clear()),
            "choose the independent variable, or use the case sequence"
        );
        assert_eq!(
            err(&|d| d.dependents.push("age".into())),
            "age has two roles; keep it in one"
        );
        assert_eq!(
            err(&|d| d.dependents.push("plan".into())),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| d.models = [false; 11]),
            "choose at least one model"
        );
        let largest = |d: &mut CurveEstimation| {
            d.models[CURVE_LOGISTIC] = true;
            d.upper_bound = "-1".into();
        };
        assert!(err(&largest).starts_with("the upper bound must be above "));
        assert_eq!(
            err(&|d| {
                d.save[CURVE_CIN] = true;
                d.ci_level = "100".into();
            }),
            "the confidence level must be a percentage between 0 and 100"
        );
        // settings of a part that is off are not checked
        let mut d = CurveEstimation {
            dependents: v(&["monthly_spend"]),
            independent: "monthly_spend".into(),
            sequence: true,
            upper_bound: "-1".into(),
            ci_level: "x".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CURVEFIT VARIABLES=monthly_spend."
        );
        d.models[CURVE_LOGISTIC] = true;
        d.upper_bound = "100000".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CURVEFIT VARIABLES=monthly_spend\n  /MODEL=LINEAR LGSTIC\n  /UPPERBOUND=100000."
        );
    }

    #[test]
    fn probit_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut Probit)| {
            let mut d = Probit {
                response: "q5_1".into(),
                total: "q5_2".into(),
                covariates: v(&["age"]),
                ..Default::default()
            };
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.response.clear()),
            "choose the response frequency variable"
        );
        assert_eq!(
            err(&|d| d.total.clear()),
            "choose the total observed variable"
        );
        assert_eq!(err(&|d| d.covariates.clear()), "add at least one covariate");
        assert_eq!(
            err(&|d| d.covariates.push("q5_1".into())),
            "q5_1 has two roles; keep it in one"
        );
        assert_eq!(
            err(&|d| d.covariates.push("plan".into())),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| {
                d.factor.name = "region".into();
                d.factor.min = "3".into();
                d.factor.max = "1".into();
            }),
            "the smallest category value of region is above the largest"
        );
        assert_eq!(
            err(&|d| {
                d.log = PROBIT_LOG_BASE;
                d.log_base = "1".into();
            }),
            "the base of the logarithm must be above 0 and not 1"
        );
        assert_eq!(
            err(&|d| {
                d.natres = 2;
                d.natres_value = "1".into();
            }),
            "the natural response rate must be at least 0 and below 1"
        );
        assert_eq!(
            err(&|d| d.iterations = "2.5".into()),
            "the maximum iterations must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.heterogeneity = "2".into()),
            "the heterogeneity significance must be from 0 to 1"
        );
        // a typed base and a rate that are switched off are not checked
        let d = Probit {
            response: "q5_1".into(),
            total: "q5_2".into(),
            covariates: v(&["age"]),
            log_base: "x".into(),
            natres_value: "x".into(),
            print: [false; 4],
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "PROBIT q5_1 OF q5_2 WITH age\n  /PRINT=NONE."
        );
    }

    #[test]
    fn weight_estimation_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut WeightEstimation)| {
            let mut d = WeightEstimation {
                dependent: "monthly_spend".into(),
                independents: v(&["age"]),
                ..Default::default()
            };
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.dependent.clear()),
            "choose the dependent variable"
        );
        assert_eq!(
            err(&|d| d.independents.clear()),
            "add at least one independent variable"
        );
        assert_eq!(
            err(&|d| d.independents.push("monthly_spend".into())),
            "monthly_spend has two roles; keep it in one"
        );
        assert_eq!(
            err(&|d| d.source = "monthly_spend".into()),
            "monthly_spend is the dependent variable"
        );
        assert_eq!(
            err(&|d| d.source = "plan".into()),
            "plan is a string; the command needs numbers"
        );
        let range = |from: &str, to: &str, by: &str| {
            err(&|d| {
                d.from = from.into();
                d.to = to.into();
                d.by = by.into();
            })
        };
        assert_eq!(range("0", "", "1"), "the last power must be a number");
        assert_eq!(range("0", "2", "0"), "the step must be above 0");
        assert_eq!(
            range("2", "0", "0.5"),
            "the last power must not be below the first"
        );
        assert_eq!(
            range("0", "150", "1"),
            "the range gives more than 150 powers"
        );
        assert_eq!(
            err(&|d| {
                d.power_list = true;
                d.powers = "1 two".into();
            }),
            "each power must be a number"
        );
        // a range that is switched off is not checked, an empty list is the default power, and
        // 150 powers are allowed
        let d = WeightEstimation {
            dependent: "monthly_spend".into(),
            independents: v(&["age"]),
            power_list: true,
            from: "x".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "WLS VARIABLES=monthly_spend WITH age."
        );
        let d = WeightEstimation {
            power_list: false,
            from: "0".into(),
            to: "149".into(),
            by: "1".into(),
            ..d
        };
        assert!(d.syntax(&data).unwrap().ends_with("/POWER=0 TO 149 BY 1."));
    }
}
