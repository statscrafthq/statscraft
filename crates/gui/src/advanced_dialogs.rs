//! The Ordinal Regression, Multinomial Logistic Regression, Linear Mixed Models, Generalized
//! Linear Models (in `genlin_dialog`), Correspondence Analysis (in `correspondence_dialog`) and
//! complex samples dialogs (the analysis plan, Descriptives, Frequencies and Crosstabs). Options
//! left as they are add nothing to the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::correspondence_dialog::Correspondence;
use crate::dialogs::whole;
use crate::genlin_dialog::{GeneralizedLinear, data_checks};
use crate::model_dialogs::GLM_ADJUSTMENTS;
use crate::stats_dialogs::{numeric, parse, percent};
use crate::syntax_text::{names, number, quote, value};
use statscraft_data::Dataset;

/// The dialogs of this module, one variant of the window's dialogs.
#[derive(Clone, PartialEq, Debug)]
pub enum Advanced {
    Ordinal(Ordinal),
    Multinomial(Multinomial),
    Mixed(LinearMixed),
    // boxed: the largest dialog by far
    Genlin(Box<GeneralizedLinear>),
    Plan(SamplePlan),
    Descriptives(SampleDescriptives),
    Tables(SampleTables),
    Correspondence(Correspondence),
}

impl Advanced {
    pub fn title(&self) -> &'static str {
        match self {
            Advanced::Ordinal(_) => "Ordinal Regression",
            Advanced::Multinomial(_) => "Multinomial Logistic Regression",
            Advanced::Mixed(_) => "Linear Mixed Models",
            Advanced::Genlin(_) => "Generalized Linear Models",
            Advanced::Plan(_) => "Complex Samples Analysis Plan",
            Advanced::Descriptives(_) => "Complex Samples Descriptives",
            Advanced::Tables(d) if d.crosstabs => "Complex Samples Crosstabs",
            Advanced::Tables(_) => "Complex Samples Frequencies",
            Advanced::Correspondence(_) => "Correspondence Analysis",
        }
    }

    /// The same dialog with its defaults.
    pub fn reset(&self) -> Self {
        match self {
            Advanced::Ordinal(_) => Advanced::Ordinal(Default::default()),
            Advanced::Multinomial(_) => Advanced::Multinomial(Default::default()),
            Advanced::Mixed(_) => Advanced::Mixed(Default::default()),
            Advanced::Genlin(_) => Advanced::Genlin(Default::default()),
            Advanced::Plan(_) => Advanced::Plan(Default::default()),
            Advanced::Descriptives(_) => Advanced::Descriptives(Default::default()),
            Advanced::Tables(d) => Advanced::Tables(SampleTables {
                crosstabs: d.crosstabs,
                ..Default::default()
            }),
            Advanced::Correspondence(_) => Advanced::Correspondence(Default::default()),
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
            Advanced::Genlin(d) => data_checks(d, data).and_then(|string| d.syntax(string)),
            Advanced::Plan(d) => numeric(data, d.numeric()).and_then(|()| d.syntax()),
            Advanced::Descriptives(d) => numeric(
                data,
                d.summary.iter().chain(&d.numerators).chain(&d.denominators),
            )
            .and_then(|()| d.syntax()),
            Advanced::Tables(d) => d.syntax(),
            Advanced::Correspondence(d) => d.syntax(data),
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
    pub(crate) fn syntax(&self, model: &[String], what: &str) -> Result<String, String> {
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

/// One table of estimated marginal means: the crossed factors, none for the overall mean, and
/// the factor whose levels are compared.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MeansTable {
    pub factors: Vec<String>,
    /// Empty: no comparison.
    pub compare: String,
    /// Index into the comparisons of the command (`MIXED_COMPARISONS`, `GENLIN_CONTRASTS`).
    pub contrast: usize,
    /// Index into the adjustments of the command.
    pub adjustment: usize,
}

impl MeansTable {
    /// The factors of table `n` in the order of the model, and the compared one, checked.
    pub(crate) fn checked<'a>(
        &self,
        factors: &'a [String],
        n: usize,
    ) -> Result<(Vec<&'a str>, Option<&str>), String> {
        if let Some(f) = self.factors.iter().find(|f| !factors.contains(f)) {
            return Err(format!(
                "{f} in estimated marginal means table {n} is no longer a factor"
            ));
        }
        let crossed: Vec<&str> = factors
            .iter()
            .filter(|f| self.factors.contains(f))
            .map(String::as_str)
            .collect();
        let compare = match self.compare.as_str() {
            "" => None,
            c if crossed.contains(&c) => Some(c),
            c => {
                return Err(format!(
                    "estimated marginal means table {n} compares {c}, which is not in the table"
                ));
            }
        };
        Ok((crossed, compare))
    }
}

/// Unchecks the factors taken out of the dialog's Factors, and a comparison of one of them, so
/// each table shows what its syntax uses.
pub fn forget_old_factors(factors: &[String], tables: &mut [MeansTable]) {
    for t in tables {
        t.factors.retain(|f| factors.contains(f));
        if !t.factors.contains(&t.compare) {
            t.compare.clear();
        }
    }
}

/// Forgets the values of covariates taken out of the dialog's Covariates.
pub fn forget_old_covariates(covariates: &[String], at: &mut Vec<(String, String)>) {
    at.retain(|(n, _)| covariates.contains(n));
}

/// The covariates given a value in `at`, as (name, value), in the order of `covariates`; the
/// others stay at their means.
pub(crate) fn covariate_values<'a>(
    covariates: &'a [String],
    at: &[(String, String)],
) -> Result<Vec<(&'a str, String)>, String> {
    let mut out = Vec::new();
    for c in covariates {
        let Some((_, text)) = at.iter().find(|(n, _)| n == c) else {
            continue;
        };
        if !text.trim().is_empty() {
            let v = parse(text, &format!("the value of {c} for the means"))?;
            out.push((c.as_str(), number(v)));
        }
    }
    Ok(out)
}

/// A custom test: a label and rows of coefficients, one row per line, each an effect followed by
/// its coefficients, such as `gender 1 -1` or `INTERCEPT 1 age 0.5`.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ContrastTest {
    pub label: String,
    pub rows: String,
}

impl ContrastTest {
    /// `/TEST='label' row; row`, every effect checked against `terms`, the effects of the
    /// model. ALL is taken too, and INTERCEPT when the model has one (`intercept`). With
    /// `random` Ok (the random terms and whether the random intercept is in), a row may go on
    /// after `|` with coefficients of random effects, checked the same way; Err says why not.
    pub(crate) fn syntax(
        &self,
        terms: &[Vec<String>],
        intercept: bool,
        random: Result<(&[Vec<String>], bool), &str>,
        n: usize,
    ) -> Result<String, String> {
        let mut rows = Vec::new();
        for line in self.rows.lines() {
            let mut parts = line.split('|');
            let fixed = parts.next().unwrap_or_default();
            let after = parts.next();
            if parts.next().is_some() {
                return Err(format!("test {n}: a row has one | at most"));
            }
            // `ALL |` with no coefficients gives those of the random effects only
            let bare_all = after.is_some();
            let mut row = row_part(fixed, n, false, bare_all, "an effect of the model", &|t| {
                (intercept && t.eq_ignore_ascii_case("INTERCEPT"))
                    || terms.iter().any(|term| same_term(term, t))
            })?;
            if let Some(after) = after {
                let (random_terms, random_intercept) =
                    random.map_err(|why| format!("test {n}: | is for random effects, {why}"))?;
                // values straight after `|` are for every random parameter, as after ALL
                let part = row_part(after, n, true, false, "a random effect", &|t| {
                    (random_intercept && t.eq_ignore_ascii_case("INTERCEPT"))
                        || random_terms.iter().any(|term| same_term(term, t))
                })?;
                if part.is_empty() {
                    return Err(format!("test {n}: | needs coefficients after it"));
                }
                row = format!("{row} | {part}").trim_start().to_string();
            }
            if !row.is_empty() {
                rows.push(row);
            }
        }
        if rows.is_empty() {
            return Err(format!("give the coefficients of test {n}"));
        }
        let label = match self.label.trim() {
            "" => String::new(),
            label => format!("{} ", quote(label)),
        };
        Ok(format!("\n  /TEST={label}{}", rows.join("; ")))
    }
}

/// One part of a test row, effects each followed by its coefficients, as the syntax writes it;
/// empty when there is nothing. `leading` lets coefficients come before the first effect,
/// `bare_all` lets a last ALL go without them. ALL is an effect, the others are those `known`
/// takes, `what` names them in the error.
fn row_part(
    text: &str,
    n: usize,
    leading: bool,
    bare_all: bool,
    what: &str,
    known: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    // coefficients written plainly: `1.` would end the command
    let tokens: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .map(|t| coefficient(t).unwrap_or_else(|| t.to_string()))
        .collect();
    let mut after_effect = 0;
    for (i, token) in tokens.iter().enumerate() {
        if coefficient(token).is_some() {
            if i == 0 && !leading {
                return Err(format!(
                    "test {n}: a row starts with an effect, then its coefficients"
                ));
            }
            after_effect += 1;
            continue;
        }
        if i > 0 && after_effect == 0 {
            return Err(format!("test {n}: {} needs coefficients", tokens[i - 1]));
        }
        after_effect = 0;
        if !token.eq_ignore_ascii_case("ALL") && !known(token) {
            return Err(format!("test {n}: {token} is not {what}"));
        }
    }
    if let Some(last) = tokens.last()
        && after_effect == 0
        && !(bare_all && last.eq_ignore_ascii_case("ALL"))
    {
        return Err(format!("test {n}: {last} needs coefficients"));
    }
    Ok(tokens.join(" "))
}

/// A number of an L matrix, `1`, `-0.5` or a fraction such as `1/3`, as the syntax writes it;
/// None for anything else.
fn coefficient(token: &str) -> Option<String> {
    let finite = |t: &str| t.parse::<f64>().ok().filter(|v| v.is_finite());
    match token.split_once('/') {
        Some((a, b)) => {
            let (a, b) = (finite(a)?, finite(b).filter(|d| *d > 0.0)?);
            Some(format!("{}/{}", number(a), number(b)))
        }
        None => finite(token).map(number),
    }
}

/// `a*b` written as `b*a` or in other case is the same term.
fn same_term(term: &[String], written: &str) -> bool {
    let parts: Vec<&str> = written.split('*').collect();
    parts.len() == term.len()
        && term
            .iter()
            .all(|t| parts.iter().any(|p| p.eq_ignore_ascii_case(t)))
}

/// Each name of `model` as a term of its own.
fn main_effects(model: &[String]) -> Vec<Vec<String>> {
    model.iter().map(|n| vec![n.clone()]).collect()
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
    pub tests: Vec<ContrastTest>,
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
            tests: Vec::new(),
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
        // the thresholds are INTERCEPT; the scale terms have parameters of their own
        let mut terms = if self.custom_location {
            self.location.terms.clone()
        } else {
            main_effects(&model)
        };
        let scale: &[Vec<String>] = match self.scale {
            true => &self.scale_terms.terms,
            false => &[],
        };
        for (i, test) in self.tests.iter().enumerate() {
            // an effect of both parts is ambiguous by name
            let both = test
                .rows
                .split(|c: char| c.is_whitespace() || c == ',')
                .find(|t| {
                    [&terms[..], scale]
                        .iter()
                        .all(|part| part.iter().any(|term| same_term(term, t)))
                });
            if let Some(t) = both {
                return Err(format!(
                    "test {}: {t} is in both the location and the scale part, so give its coefficients with ALL",
                    i + 1
                ));
            }
        }
        terms.extend(scale.iter().cloned());
        for (i, test) in self.tests.iter().enumerate() {
            // the thresholds are always there
            text.push_str(&test.syntax(
                &terms,
                true,
                Err("which ordinal regression has none of"),
                i + 1,
            )?);
        }
        text.push('.');
        Ok(text)
    }
}

/// The reference category of NOMREG.
pub const NOMREG_BASES: [&str; 3] = ["Last", "First", "Custom"];

/// The models of NOMREG.
pub const NOMREG_MODELS: [&str; 3] = ["Main effects", "Full factorial", "Custom"];

/// The stepwise methods of NOMREG, keyword and label.
pub const NOMREG_STEP_METHODS: [(&str, &str); 4] = [
    ("FORWARD", "Forward entry"),
    ("BACKWARD", "Backward elimination"),
    ("FSTEP", "Forward stepwise"),
    ("BSTEP", "Backward stepwise"),
];

/// Which effects may enter or leave together, keyword and label; the first is the default.
pub const NOMREG_STEP_RULES: [(&str, &str); 4] = [
    ("SINGLE", "Hierarchy of all effects"),
    ("SFACTOR", "Hierarchy of factor effects"),
    ("CONTAINMENT", "Containment"),
    ("NONE", "None"),
];

/// Keyword and label of every table on /PRINT of NOMREG.
pub const NOMREG_PRINT: [(&str, &str); 12] = [
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
    ("STEP", "Step summary"),
];

/// The tables NOMREG shows without /PRINT.
const NOMREG_DEFAULT_PRINT: [bool; 12] = [
    true, true, false, false, false, false, false, false, false, false, false, true,
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
    /// The effects a stepwise method may enter or remove, after the custom terms.
    pub step_terms: Terms,
    /// Index into `NOMREG_STEP_METHODS`.
    pub step_method: usize,
    /// Index into `NOMREG_STEP_RULES`.
    pub step_rule: usize,
    pub min_effects: String,
    /// Empty: every effect.
    pub max_effects: String,
    pub entry: String,
    pub removal: String,
    /// The score test for entry instead of the likelihood ratio.
    pub score_entry: bool,
    /// The Wald test for removal instead of the likelihood ratio.
    pub wald_removal: bool,
    pub intercept: bool,
    /// One flag per entry of `NOMREG_PRINT`.
    pub print: [bool; 12],
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
            step_terms: Terms::default(),
            step_method: 0,
            step_rule: 0,
            min_effects: "0".into(),
            max_effects: String::new(),
            entry: "0.05".into(),
            removal: "0.1".into(),
            score_entry: false,
            wald_removal: false,
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
        let mut stepwise = None;
        match self.model {
            0 => {}
            1 => text.push_str("\n  /FULLFACTORIAL"),
            _ if self.step_terms.terms.is_empty() => {
                let terms = self.terms.syntax(&self.model_variables(), "model")?;
                text.push_str(&format!("\n  /MODEL={terms}"));
            }
            _ => {
                let model = self.model_variables();
                let steps = self.step_terms.syntax(&model, "stepwise terms")?;
                if let Some(t) = self
                    .step_terms
                    .terms
                    .iter()
                    .find(|t| self.terms.terms.iter().any(|m| same_term(m, &t.join("*"))))
                {
                    return Err(format!(
                        "{} is both a model term and a stepwise term",
                        t.join("*")
                    ));
                }
                // the model may be empty: the stepwise method then starts from the intercept
                let fixed = match self.terms.terms.is_empty() {
                    true => String::new(),
                    false => format!("{} ", self.terms.syntax(&model, "model")?),
                };
                let method = NOMREG_STEP_METHODS[self.step_method].0;
                // forward methods start from the model alone, which must not be empty
                if fixed.is_empty() && !self.intercept && matches!(method, "FORWARD" | "FSTEP") {
                    return Err(
                        "forward stepping needs a model term or the intercept to start from".into(),
                    );
                }
                text.push_str(&format!("\n  /MODEL={fixed}| {method}={steps}"));
                stepwise = Some(self.stepwise_criteria()?);
            }
        }
        if !self.intercept {
            text.push_str("\n  /INTERCEPT=EXCLUDE");
        }
        if let Some(criteria) = stepwise.filter(|c| !c.is_empty()) {
            text.push_str(&format!("\n  /STEPWISE={}", criteria.join(" ")));
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

    /// Which stepping settings the chosen method uses: the least effects (the backward
    /// methods), the most effects (the forward ones), entry (all but backward elimination) and
    /// removal (all but forward entry).
    pub fn stepping_fields(&self) -> [bool; 4] {
        let method = NOMREG_STEP_METHODS[self.step_method].0;
        let backward = matches!(method, "BACKWARD" | "BSTEP");
        [
            backward,
            !backward,
            method != "BACKWARD",
            method != "FORWARD",
        ]
    }

    /// The STEPWISE settings the method uses that differ from the defaults.
    fn stepwise_criteria(&self) -> Result<Vec<String>, String> {
        let [least, most, entry, removal] = self.stepping_fields();
        let mut out = Vec::new();
        if self.step_rule > 0 {
            out.push(format!("RULE({})", NOMREG_STEP_RULES[self.step_rule].0));
        }
        if least
            && let Some(n) = whole(&self.min_effects, 0, "the least number of effects")?
            && n > 0
        {
            out.push(format!("MINEFFECT({n})"));
        }
        if most && let Some(n) = whole(&self.max_effects, 0, "the most effects")? {
            out.push(format!("MAXEFFECT({n})"));
        }
        let probabilities = [
            (entry, &self.entry, "PIN", 0.05),
            (removal, &self.removal, "POUT", 0.1),
        ];
        for (_, text, keyword, default) in probabilities.into_iter().filter(|p| p.0) {
            let p = match text.trim() {
                "" => default,
                given => parse(given, "the probability for entry and removal")?,
            };
            if !(p > 0.0 && p <= 1.0) {
                return Err(
                    "the probabilities for entry and removal are above 0 and at most 1".into(),
                );
            }
            if p != default {
                out.push(format!("{keyword}({})", number(p)));
            }
        }
        if entry && self.score_entry {
            out.push("ENTRYMETHOD(SCORE)".into());
        }
        if removal && self.wald_removal {
            out.push("REMOVALMETHOD(WALD)".into());
        }
        Ok(out)
    }
}

/// The covariance structures MIXED takes on both RANDOM and REPEATED, after the first few.
const MORE_COVARIANCES: [(&str, &str); 11] = [
    ("AD1", "Ante-dependence, first order (AD1)"),
    ("ARH1", "Heterogeneous first-order autoregressive (ARH1)"),
    (
        "ARMA11",
        "First-order autoregressive moving average (ARMA11)",
    ),
    ("CSH", "Heterogeneous compound symmetry (CSH)"),
    ("CSR", "Compound symmetry, correlation form (CSR)"),
    ("FA1", "Factor-analytic, first order (FA1)"),
    ("FAH1", "Factor-analytic, first order, heterogeneous (FAH1)"),
    ("HF", "Huynh-Feldt (HF)"),
    ("TP", "Toeplitz (TP)"),
    ("TPH", "Heterogeneous Toeplitz (TPH)"),
    ("UNR", "Unstructured, correlation form (UNR)"),
];

/// `first` followed by `MORE_COVARIANCES`.
const fn with_more<const N: usize, const M: usize>(
    first: [(&'static str, &'static str); N],
) -> [(&'static str, &'static str); M] {
    assert!(M == N + MORE_COVARIANCES.len());
    let mut out = [("", ""); M];
    let mut i = 0;
    while i < M {
        out[i] = if i < N {
            first[i]
        } else {
            MORE_COVARIANCES[i - N]
        };
        i += 1;
    }
    out
}

/// The covariance structures of random effects, keyword and label; the first is the default.
pub const RANDOM_COVARIANCES: [(&str, &str); 17] = with_more([
    ("VC", "Variance components (VC)"),
    ("ID", "Scaled identity (ID)"),
    ("DIAG", "Diagonal (DIAG)"),
    ("UN", "Unstructured (UN)"),
    ("CS", "Compound symmetry (CS)"),
    ("AR1", "First-order autoregressive (AR1)"),
]);

/// The covariance structures of repeated measures; the first is the default. VC is left out:
/// on REPEATED it is DIAG.
pub const REPEATED_COVARIANCES: [(&str, &str); 16] = with_more([
    ("DIAG", "Diagonal (DIAG)"),
    ("ID", "Scaled identity (ID)"),
    ("CS", "Compound symmetry (CS)"),
    ("AR1", "First-order autoregressive (AR1)"),
    ("UN", "Unstructured (UN)"),
]);

/// Keyword and label of every variable MIXED saves.
pub const MIXED_SAVE: [(&str, &str); 7] = [
    ("FIXPRED", "Fixed predicted values"),
    ("SEFIXP", "Their standard errors"),
    ("DFFIXP", "Their degrees of freedom"),
    ("PRED", "Predicted values"),
    ("SEPRED", "Standard errors of the predicted values"),
    ("DFPRED", "Degrees of freedom of the predicted values"),
    ("RESID", "Residuals"),
];

/// Keyword and label of every table on /PRINT of MIXED.
pub const MIXED_PRINT: [(&str, &str); 10] = [
    ("SOLUTION", "Parameter estimates of the fixed effects"),
    ("TESTCOV", "Tests of the covariance parameters"),
    ("G", "Covariance of the random effects (G)"),
    ("R", "Covariance of the residuals (R)"),
    ("COVB", "Covariance of the fixed estimates"),
    ("CORB", "Correlation of the fixed estimates"),
    ("CPS", "Case processing summary"),
    ("DESCRIPTIVES", "Descriptive statistics"),
    ("HISTORY", "Iteration history"),
    ("LMATRIX", "Coefficients of the tests (L matrix)"),
];

/// The place of a /PRINT keyword of MIXED in `MIXED_PRINT`.
pub fn mixed_print(keyword: &str) -> usize {
    MIXED_PRINT
        .iter()
        .position(|(k, _)| *k == keyword)
        .unwrap_or_else(|| panic!("{keyword} is not in MIXED_PRINT"))
}

/// How MIXED finds the denominator degrees of freedom, DFMETHOD keyword and label.
pub const MIXED_DF_METHODS: [(&str, &str); 3] = [
    ("SATTERTHWAITE", "Satterthwaite"),
    ("RESIDUAL", "Residual"),
    ("KENWARDROGER", "Kenward-Roger"),
];

/// How the levels of a factor are compared in MIXED: REFCAT and label; the first is every pair.
pub const MIXED_COMPARISONS: [(&str, &str); 3] = [
    ("", "Every pair"),
    ("FIRST", "Each with the first"),
    ("LAST", "Each with the last"),
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
    /// Type I sums of squares instead of Type III.
    pub type_one: bool,
    pub random: bool,
    pub random_intercept: bool,
    pub random_terms: Terms,
    /// The subject variables the random effects vary over, combined.
    pub random_subjects: Vec<String>,
    /// Index into `RANDOM_COVARIANCES`.
    pub random_covariance: usize,
    /// The predicted random effects of every subject (SOLUTION on RANDOM).
    pub random_solution: bool,
    /// Maximum likelihood instead of restricted maximum likelihood.
    pub ml: bool,
    /// One flag per entry of `MIXED_PRINT`.
    pub print: [bool; 10],
    /// The iteration history shows every n-th iteration.
    pub history: String,
    pub means: Vec<MeansTable>,
    /// The values of covariates for the means, by name; empty is the mean.
    pub means_at: Vec<(String, String)>,
    pub tests: Vec<ContrastTest>,
    /// One flag per entry of `MIXED_SAVE`.
    pub save: [bool; 7],
    pub ci_level: String,
    pub iterations: String,
    pub halvings: String,
    /// Index into `MIXED_DF_METHODS`.
    pub df_method: usize,
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
            type_one: false,
            random: false,
            random_intercept: true,
            random_terms: Terms::default(),
            random_subjects: Vec::new(),
            random_covariance: 0,
            random_solution: false,
            ml: false,
            print: [false; 10],
            history: "1".into(),
            means: Vec::new(),
            means_at: Vec::new(),
            tests: Vec::new(),
            save: [false; 7],
            ci_level: "95".into(),
            iterations: "100".into(),
            halvings: "10".into(),
            df_method: 0,
            include: false,
        }
    }
}

impl LinearMixed {
    /// The index into `MIXED_DF_METHODS` the syntax uses: Kenward-Roger is made for restricted
    /// maximum likelihood, so with ML a choice of it falls back to Satterthwaite.
    pub fn df_used(&self) -> usize {
        match self.ml && MIXED_DF_METHODS[self.df_method].0 == "KENWARDROGER" {
            true => 0,
            false => self.df_method,
        }
    }

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
        if fixed.is_empty() && !self.intercept {
            return Err("the fixed effects need a term or the intercept".into());
        }
        // the intercept alone has the same test of either type
        if !fixed.is_empty() {
            text.push_str(&format!("\n  /FIXED={fixed}"));
            let options: Vec<&str> = [(!self.intercept, "NOINT"), (self.type_one, "SSTYPE(1)")]
                .into_iter()
                .filter_map(|(on, keyword)| on.then_some(keyword))
                .collect();
            if !options.is_empty() {
                text.push_str(&format!(" | {}", options.join(" ")));
            }
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
            if self.random_solution {
                options.push("SOLUTION".into());
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
        if self.df_used() > 0 {
            criteria.push(format!("DFMETHOD({})", MIXED_DF_METHODS[self.df_used()].0));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.print[mixed_print("G")] && !self.random {
            return Err("the covariance of the random effects needs random effects".into());
        }
        if self.print[mixed_print("R")] && self.repeated.is_empty() {
            return Err("the covariance of the residuals needs repeated variables".into());
        }
        let mut print: Vec<String> = keywords(&MIXED_PRINT, &self.print)
            .into_iter()
            .map(String::from)
            .collect();
        if let Some(k) = print.iter_mut().find(|k| *k == "HISTORY")
            && let Some(n) = whole(&self.history, 1, "the step of the iteration history")?
            && n != 1
        {
            *k = format!("HISTORY({n})");
        }
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
        // the values count only for tables of means
        let at = match self.means.is_empty() {
            true => Vec::new(),
            false => covariate_values(&self.covariates, &self.means_at)?,
        };
        let with = match at.is_empty() {
            true => String::new(),
            false => {
                let values: Vec<String> = at.iter().map(|(n, v)| format!("{n}={v}")).collect();
                format!(" WITH({})", values.join(" "))
            }
        };
        for (i, table) in self.means.iter().enumerate() {
            let (crossed, compare) = table.checked(&self.factors, i + 1)?;
            let tables = match crossed.is_empty() {
                true => "OVERALL".to_string(),
                false => crossed.join("*"),
            };
            text.push_str(&format!("\n  /EMMEANS=TABLES({tables}){with}"));
            if let Some(c) = compare {
                text.push_str(&format!(" COMPARE({c})"));
                if table.contrast > 0 {
                    let refcat = MIXED_COMPARISONS[table.contrast].0;
                    text.push_str(&format!(" REFCAT({refcat})"));
                }
                if table.adjustment > 0 {
                    let adjustment = GLM_ADJUSTMENTS[table.adjustment].0;
                    text.push_str(&format!(" ADJ({adjustment})"));
                }
            }
        }
        let terms = match self.custom_fixed {
            true => self.fixed.terms.clone(),
            false => main_effects(&model),
        };
        let random = match self.random {
            true => Ok((self.random_terms.terms.as_slice(), self.random_intercept)),
            false => Err("and the model has none"),
        };
        for (i, test) in self.tests.iter().enumerate() {
            text.push_str(&test.syntax(&terms, self.intercept, random, i + 1)?);
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
    use crate::dialogs::testing::{open, run_in, runs, v, waves};

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
        d.print = [true; 12];
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
             /PRINT=CPS MFI IC SUMMARY FIT LRT PARAMETER CLASSTABLE COVB CORB KERNEL STEP\n  \
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
            print: [false; 12],
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
        d.print = [
            true, true, true, false, true, true, false, false, false, false,
        ];
        d.save = [true, true, true, true, false, false, true];
        d.ci_level = "90".into();
        d.iterations = "150".into();
        d.halvings = "5".into();
        d.df_method = 1;
        d.include = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MIXED monthly_spend BY gender WITH age\n  /FIXED=gender age\n  \
             /RANDOM=INTERCEPT age | SUBJECT(region) COVTYPE(UN)\n  /METHOD=ML\n  \
             /CRITERIA=CIN(90) MXITER(150) MXSTEP(5) DFMETHOD(RESIDUAL)\n  \
             /PRINT=SOLUTION TESTCOV G COVB CORB\n  /SAVE=FIXPRED SEFIXP DFFIXP PRED RESID\n  \
             /MISSING=INCLUDE."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 5);
        let data = session.dataset().unwrap();
        assert!(data.variable("FXPRED_1").is_ok());
        d.print[mixed_print("R")] = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the covariance of the residuals needs repeated variables"
        );
        d.random = false;
        d.subjects.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the covariance of the random effects needs random effects"
        );

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

    #[test]
    fn mixed_kenward_roger_type_one_random_solution_and_more_tables() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            subjects: v(&["region"]),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            type_one: true,
            random: true,
            random_terms: terms(&[&["age"]]),
            random_subjects: v(&["region"]),
            random_covariance: 9,
            random_solution: true,
            df_method: 2,
            print: std::array::from_fn(|i| i >= 6),
            history: "2".into(),
            save: std::array::from_fn(|i| matches!(i, 4 | 5)),
            // each matrix names effects in every row or starts every row with ALL
            tests: vec![
                ContrastTest {
                    label: "Women in a region".into(),
                    rows: "gender 1 -1 0 | INTERCEPT 1 age 40\nINTERCEPT 1 | age 1".into(),
                },
                ContrastTest {
                    label: String::new(),
                    rows: "ALL | ALL 0 1\nALL | ALL 1 0".into(),
                },
            ],
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MIXED monthly_spend BY gender WITH age\n  /FIXED=gender age | SSTYPE(1)\n  \
             /RANDOM=INTERCEPT age | SUBJECT(region) COVTYPE(CSH) SOLUTION\n  \
             /CRITERIA=DFMETHOD(KENWARDROGER)\n  /PRINT=CPS DESCRIPTIVES HISTORY(2) LMATRIX\n  \
             /SAVE=SEPRED DFPRED\n  \
             /TEST='Women in a region' gender 1 -1 0 | INTERCEPT 1 age 40; INTERCEPT 1 | age 1\n  \
             /TEST=ALL | ALL 0 1; ALL | ALL 1 0."
        );
        assert!(statscraft_syntax::parse(&syntax).1.is_empty());
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 8);
        let data = session.dataset().unwrap();
        assert!(data.variable("SEPRED_1").is_ok());

        // Kenward-Roger is for REML: with ML the syntax falls back to Satterthwaite
        d.ml = true;
        assert_eq!(d.df_used(), 0);
        let ml = d.syntax().unwrap();
        assert!(ml.contains("/METHOD=ML") && !ml.contains("DFMETHOD"));
        d.df_method = 1;
        assert!(d.syntax().unwrap().contains("DFMETHOD(RESIDUAL)"));
        d.ml = false;
        d.df_method = 2;

        // the defaults write nothing, HISTORY every iteration has no count
        d.history = " 1 ".into();
        assert!(
            d.syntax()
                .unwrap()
                .contains("/PRINT=CPS DESCRIPTIVES HISTORY LMATRIX\n")
        );
        d.history = "0".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the step of the iteration history must be a whole number of at least 1"
        );
        // the count is checked only when the history is shown
        d.print[mixed_print("HISTORY")] = false;
        assert!(d.syntax().is_ok());

        // a random effect of a test is one of the random terms or the random intercept
        for (rows, error) in [
            ("| gender 1 -1 0", "test 1: gender is not a random effect"),
            ("age 1 |", "test 1: | needs coefficients after it"),
            ("age 1 | age 1 | age 1", "test 1: a row has one | at most"),
            ("| age", "test 1: age needs coefficients"),
            ("ALL", "test 1: ALL needs coefficients"),
            ("ALL | ALL", "test 1: ALL needs coefficients"),
            ("| age INTERCEPT 1", "test 1: age needs coefficients"),
        ] {
            d.tests[0].rows = rows.into();
            assert_eq!(d.syntax().unwrap_err(), error, "{rows}");
        }
        d.tests[0].rows = "| INTERCEPT 1".into();
        d.random_intercept = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: INTERCEPT is not a random effect"
        );
        d.random = false;
        d.subjects.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: | is for random effects, and the model has none"
        );
    }

    #[test]
    fn mixed_every_covariance_type_runs_on_the_waves() {
        // random intercept and slope of each person, then four waves of each person
        let mut d = LinearMixed {
            dependent: "level".into(),
            subjects: v(&["person"]),
            covariates: v(&["dose"]),
            random: true,
            random_terms: terms(&[&["dose"]]),
            random_subjects: v(&["person"]),
            ..Default::default()
        };
        for (i, (keyword, _)) in RANDOM_COVARIANCES.iter().enumerate() {
            d.random_covariance = i;
            let syntax = d.syntax().unwrap();
            assert!(i == 0 || syntax.contains(&format!("COVTYPE({keyword})")));
            // over two random parameters these have one parameter too many: the command says so
            if matches!(*keyword, "ARMA11" | "FAH1") {
                assert!(waves().run(&syntax, None).is_err(), "{keyword}");
            } else {
                assert!(run_in(&mut waves(), &syntax) >= 4, "{keyword}");
            }
        }
        d.random = false;
        d.repeated = v(&["wave"]);
        for (i, (covariance, _)) in REPEATED_COVARIANCES.iter().enumerate() {
            d.repeated_covariance = i;
            let syntax = d.syntax().unwrap();
            let expected = match i {
                0 => String::new(),
                _ => format!(" COVTYPE({covariance})"),
            };
            assert_eq!(
                syntax,
                format!(
                    "MIXED level WITH dose\n  /FIXED=dose\n  \
                     /REPEATED=wave | SUBJECT(person){expected}."
                )
            );
            assert!(run_in(&mut waves(), &syntax) >= 4, "{covariance}");
        }
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
    fn mixed_means_and_custom_tests() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            factors: v(&["gender", "region"]),
            covariates: v(&["age"]),
            means: vec![
                MeansTable::default(),
                MeansTable {
                    factors: v(&["region", "gender"]),
                    compare: "gender".into(),
                    contrast: 1,
                    adjustment: 1,
                },
                MeansTable {
                    factors: v(&["region"]),
                    compare: "region".into(),
                    contrast: 2,
                    adjustment: 2,
                },
            ],
            means_at: vec![("age".into(), "40".into())],
            tests: vec![ContrastTest {
                label: "Men and women".into(),
                rows: "gender 1 -1 0\n\nINTERCEPT 0, age 1".into(),
            }],
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MIXED monthly_spend BY gender region WITH age\n  /FIXED=gender region age\n  \
             /EMMEANS=TABLES(OVERALL) WITH(age=40)\n  \
             /EMMEANS=TABLES(gender*region) WITH(age=40) COMPARE(gender) REFCAT(FIRST) ADJ(BONFERRONI)\n  \
             /EMMEANS=TABLES(region) WITH(age=40) COMPARE(region) REFCAT(LAST) ADJ(SIDAK)\n  \
             /TEST='Men and women' gender 1 -1 0; INTERCEPT 0 age 1."
        );
        assert!(runs(&syntax) >= 8);

        for (rows, error) in [
            (
                "1 -1",
                "test 1: a row starts with an effect, then its coefficients",
            ),
            ("gender", "test 1: gender needs coefficients"),
            ("gender age 1", "test 1: gender needs coefficients"),
            ("weight 1", "test 1: weight is not an effect of the model"),
            (
                "gender 1 -1/0",
                "test 1: -1/0 is not an effect of the model",
            ),
            (" \n ", "give the coefficients of test 1"),
        ] {
            d.tests[0].rows = rows.into();
            assert_eq!(d.syntax().unwrap_err(), error, "{rows}");
        }
        // custom fixed terms are the effects a test may name, in either order
        d.custom_fixed = true;
        d.fixed = terms(&[&["gender", "age"]]);
        d.tests[0].rows = "age*GENDER 1 -1 0".into();
        assert!(
            d.syntax()
                .unwrap()
                .ends_with("/TEST='Men and women' age*GENDER 1 -1 0.")
        );
        d.tests[0].rows = "gender 1 -1 0".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: gender is not an effect of the model"
        );
        d.tests.clear();
        d.means_at[0].1 = "x".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the value of age for the means must be a number"
        );
        d.means_at[0].1 = " ".into();
        d.means[1].factors = v(&["income"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "income in estimated marginal means table 2 is no longer a factor"
        );
    }

    #[test]
    fn ordinal_custom_tests() {
        let mut d = Ordinal {
            dependent: "q1_satisfaction".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            scale: true,
            scale_terms: terms(&[&["age"]]),
            tests: vec![
                ContrastTest {
                    label: String::new(),
                    rows: "gender 1/2 -1/2 0".into(),
                },
                ContrastTest {
                    label: "First threshold".into(),
                    rows: "INTERCEPT 1 0 0 0".into(),
                },
            ],
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PLUM q1_satisfaction BY gender WITH age\n  /SCALE=age\n  \
             /TEST=gender 1/2 -1/2 0\n  /TEST='First threshold' INTERCEPT 1 0 0 0."
        );
        assert!(runs(&syntax) >= 6);
        d.tests[0].rows = "age 1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: age is in both the location and the scale part, so give its coefficients with ALL"
        );
        d.tests[0].rows = "gender 1 -1 0 | INTERCEPT 1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: | is for random effects, which ordinal regression has none of"
        );
    }

    #[test]
    fn multinomial_stepwise() {
        let mut d = Multinomial {
            dependent: "region".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            model: 2,
            step_terms: terms(&[&["gender"], &["age"]]),
            ..Default::default()
        };
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "NOMREG region BY gender WITH age\n  /MODEL=| FORWARD=gender age."
        );
        assert!(runs(&syntax) >= 3);

        d.terms = terms(&[&["age"]]);
        d.step_terms = terms(&[&["gender"], &["gender", "age"]]);
        d.step_method = 3;
        d.step_rule = 2;
        d.min_effects = "1".into();
        d.max_effects = "2".into();
        d.entry = "0.1".into();
        d.removal = "0.2".into();
        d.score_entry = true;
        d.wald_removal = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "NOMREG region BY gender WITH age\n  /MODEL=age | BSTEP=gender gender*age\n  \
             /STEPWISE=RULE(CONTAINMENT) MINEFFECT(1) PIN(0.1) POUT(0.2) \
             ENTRYMETHOD(SCORE) REMOVALMETHOD(WALD)."
        );
        assert!(runs(&syntax) >= 3);

        d.step_terms = terms(&[&["age"]]);
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "age is both a model term and a stepwise term"
        );
        d.step_terms = terms(&[&["gender"]]);
        d.entry = "0".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the probabilities for entry and removal are above 0 and at most 1"
        );
        // the stepping settings count only with stepwise terms
        d.step_terms = Terms::default();
        assert_eq!(
            d.syntax(false).unwrap(),
            "NOMREG region BY gender WITH age\n  /MODEL=age."
        );
    }

    #[test]
    fn stepping_writes_only_what_the_method_uses() {
        let mut d = Multinomial {
            dependent: "region".into(),
            covariates: v(&["age"]),
            model: 2,
            step_terms: terms(&[&["age"]]),
            min_effects: "1".into(),
            max_effects: "1".into(),
            entry: "0.2".into(),
            removal: "0.3".into(),
            score_entry: true,
            wald_removal: true,
            ..Default::default()
        };
        let stepwise = |d: &Multinomial| {
            let syntax = d.syntax(false).unwrap();
            syntax.split("/STEPWISE=").nth(1).unwrap_or("").to_string()
        };
        assert_eq!(stepwise(&d), "MAXEFFECT(1) PIN(0.2) ENTRYMETHOD(SCORE).");
        d.step_method = 1;
        assert_eq!(stepwise(&d), "MINEFFECT(1) POUT(0.3) REMOVALMETHOD(WALD).");
        d.step_method = 2;
        assert_eq!(
            stepwise(&d),
            "MAXEFFECT(1) PIN(0.2) POUT(0.3) ENTRYMETHOD(SCORE) REMOVALMETHOD(WALD)."
        );
        d.step_method = 3;
        assert_eq!(
            stepwise(&d),
            "MINEFFECT(1) PIN(0.2) POUT(0.3) ENTRYMETHOD(SCORE) REMOVALMETHOD(WALD)."
        );
        // a field the method does not use is not checked either
        d.step_method = 0;
        d.min_effects = "x".into();
        d.removal = "2".into();
        assert!(d.syntax(false).is_ok());
    }

    #[test]
    fn means_tables_forget_factors_taken_out() {
        let mut tables = vec![MeansTable {
            factors: v(&["gender", "region"]),
            compare: "region".into(),
            ..Default::default()
        }];
        forget_old_factors(&v(&["gender"]), &mut tables);
        assert_eq!(tables[0].factors, v(&["gender"]));
        assert_eq!(tables[0].compare, "");
    }

    #[test]
    fn covariate_values_count_only_with_tables_of_means() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            covariates: v(&["age"]),
            means_at: vec![("age".into(), "x".into())],
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "MIXED monthly_spend WITH age\n  /FIXED=age."
        );
        d.means.push(MeansTable::default());
        assert_eq!(
            d.syntax().unwrap_err(),
            "the value of age for the means must be a number"
        );
    }

    #[test]
    fn a_test_names_the_intercept_only_when_the_model_has_one() {
        let mut d = LinearMixed {
            dependent: "monthly_spend".into(),
            covariates: v(&["age"]),
            tests: vec![ContrastTest {
                label: String::new(),
                rows: "INTERCEPT 1".into(),
            }],
            ..Default::default()
        };
        assert!(d.syntax().unwrap().ends_with("/TEST=INTERCEPT 1."));
        d.intercept = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "test 1: INTERCEPT is not an effect of the model"
        );
    }

    #[test]
    fn a_stepwise_term_may_not_repeat_a_model_term_in_another_order() {
        let d = Multinomial {
            dependent: "region".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            model: 2,
            terms: terms(&[&["age", "gender"]]),
            step_terms: terms(&[&["gender", "age"]]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "gender*age is both a model term and a stepwise term"
        );
    }

    #[test]
    fn coefficients_are_written_plainly() {
        let t = ContrastTest {
            label: String::new(),
            rows: "gender 1. -1. 0\nage .5 2./4.".into(),
        };
        let terms = vec![v(&["gender"]), v(&["age"])];
        assert_eq!(
            t.syntax(&terms, true, Err("which ordinal regression has none of"), 1)
                .unwrap(),
            "\n  /TEST=gender 1 -1 0; age 0.5 2/4"
        );
    }

    #[test]
    fn forward_stepping_needs_something_to_start_from() {
        let mut d = Multinomial {
            dependent: "region".into(),
            covariates: v(&["age"]),
            model: 2,
            step_terms: terms(&[&["age"]]),
            intercept: false,
            ..Default::default()
        };
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "forward stepping needs a model term or the intercept to start from"
        );
        // backward methods start with every term in
        d.step_method = 1;
        assert!(d.syntax(false).is_ok());
    }

    #[test]
    fn values_of_covariates_taken_out_are_forgotten() {
        let mut at = vec![("age".into(), "40".into()), ("weight".into(), "2".into())];
        forget_old_covariates(&v(&["age"]), &mut at);
        assert_eq!(at, vec![("age".to_string(), "40".to_string())]);
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
