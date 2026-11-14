//! The complex samples General Linear Model (CSGLM) and Logistic Regression (CSLOGISTIC)
//! dialogs: one model of a dependent on factors and covariates, fitted on the design of a plan
//! file. Options left as they are add nothing to the syntax.

use crate::advanced_dialogs::{
    ContrastTest, MeansTable, Terms, ci_level, coefficient, covariate_values, full_factorial,
    main_effects, model_variables, plan_file, row_part, same_term,
};
use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{number, quote, value};
use statscraft_data::{Dataset, is_valid_name};

/// INCLUDE of INTERCEPT, keyword and label; the first is the default.
pub const CS_INTERCEPTS: [(&str, &str); 3] = [
    ("YES", "Include"),
    ("NO", "Leave out"),
    ("ONLY", "The intercept alone"),
];

/// The statistics of the estimates, keyword and label; the last is for CSLOGISTIC only.
pub const CS_STATISTICS: [(&str, &str); 7] = [
    ("PARAMETER", "Estimates"),
    ("SE", "Standard error"),
    ("TTEST", "t test"),
    ("CINTERVAL", "Confidence interval"),
    ("DEFF", "Design effect"),
    ("DEFFSQRT", "Square root of the design effect"),
    ("EXP", "Exponentiated estimates"),
];

/// The statistics each command shows without STATISTICS: the engine's choice for CSGLM, none
/// for CSLOGISTIC.
const GLM_STATISTICS: [bool; 7] = [true, true, true, true, false, false, false];

/// TYPE of TEST; the first is the default.
pub const CS_TESTS: [(&str, &str); 4] = [
    ("F", "Wald F"),
    ("ADJF", "Adjusted Wald F"),
    ("CHISQUARE", "Wald chi-square"),
    ("ADJCHISQUARE", "Adjusted Wald chi-square"),
];

/// PADJUST of TEST; the first is the default.
pub const CS_ADJUSTMENTS: [(&str, &str); 5] = [
    ("LSD", "None (least significant difference)"),
    ("BONFERRONI", "Bonferroni"),
    ("SEQBONFERRONI", "Sequential Bonferroni"),
    ("SIDAK", "Sidak"),
    ("SEQSIDAK", "Sequential Sidak"),
];

/// CONTRAST of EMMEANS; the first is the default.
pub const CS_CONTRASTS: [(&str, &str); 6] = [
    ("SIMPLE", "Each with the last"),
    ("DEVIATION", "Each with the overall mean"),
    ("DIFFERENCE", "Each with the mean of those before"),
    ("HELMERT", "Each with the mean of those after"),
    ("REPEATED", "Each with the next"),
    ("POLYNOMIAL", "Polynomial trends"),
];

/// The tables of PRINT, keyword and label; the last two are for CSLOGISTIC only.
pub const CS_PRINT: [(&str, &str); 8] = [
    ("SAMPLEINFO", "Sample information"),
    ("VARIABLEINFO", "Variable information"),
    ("SUMMARY", "Model summary"),
    ("LMATRIX", "Coefficients of the tests"),
    ("COVB", "Covariance of the estimates"),
    ("CORB", "Correlation of the estimates"),
    ("CLASSTABLE", "Classification table"),
    ("HISTORY", "Iteration history"),
];

/// The tables shown without PRINT.
const DEFAULT_PRINT: [bool; 8] = [true, true, true, false, false, false, false, false];

/// The variables of SAVE, keyword and label, for CSGLM and for CSLOGISTIC.
pub const GLM_SAVE: [(&str, &str); 2] = [("PRED", "Predicted values"), ("RESID", "Residuals")];
pub const LOGISTIC_SAVE: [(&str, &str); 2] = [
    ("PREDPROB", "Predicted probabilities"),
    ("PREDVAL", "Predicted category"),
];

/// A category or level: the reference of the logistic dependent and of a factor in an odds
/// ratio, and the level a factor is held at; the first is the default. For a covariate held at
/// a value the first stands for its mean and Lowest is not offered.
pub const CS_LEVELS: [&str; 3] = ["Highest", "Lowest", "Value"];

/// One ODDSRATIOS subcommand: the odds ratios of one factor or covariate.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OddsRatio {
    pub variable: String,
    /// A factor: index into `CS_LEVELS` for its reference level.
    pub level: usize,
    /// A factor: the reference value when `level` is Value. A covariate: its changes,
    /// separated by spaces (empty: 1).
    pub reference: String,
    /// What the other factors and covariates are held at.
    pub control: Vec<Held>,
    /// The variable was a covariate when its level or changes were given.
    pub covariate: bool,
}

/// A factor or covariate held at a level in an odds ratio.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Held {
    pub name: String,
    /// Index into `CS_LEVELS`: 0 is the highest level of a factor or the mean of a covariate.
    pub level: usize,
    pub value: String,
    /// The variable was a covariate when its level and value were given.
    pub covariate: bool,
}

/// CSGLM or CSLOGISTIC.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleModel {
    pub logistic: bool,
    pub plan: String,
    pub dependent: String,
    /// CSLOGISTIC: index into `CS_LEVELS`.
    pub reference: usize,
    pub reference_value: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `GENLIN_MODELS`: main effects, full factorial or custom terms.
    pub model: usize,
    pub terms: Terms,
    /// Index into `CS_INTERCEPTS`.
    pub intercept: usize,
    pub show_intercept: bool,
    /// One flag per entry of `CS_STATISTICS`.
    pub statistics: [bool; 7],
    /// Index into `CS_TESTS`.
    pub test_type: usize,
    /// Index into `CS_ADJUSTMENTS`.
    pub adjustment: usize,
    /// Custom tests; a row may end with `= value`, its hypothesized value.
    pub tests: Vec<ContrastTest>,
    /// CSGLM: tables of estimated marginal means and the values the covariates are held at.
    pub means: Vec<MeansTable>,
    pub means_at: Vec<(String, String)>,
    /// CSLOGISTIC.
    pub odds: Vec<OddsRatio>,
    pub ci_level: String,
    /// Empty: the design's own degrees of freedom.
    pub df: String,
    /// CSLOGISTIC: MXITER, MXSTEP, PCONVERGE, LCONVERGE and CHKSEP.
    pub iterations: String,
    pub halvings: String,
    pub pconverge: String,
    pub lconverge: String,
    pub separation: String,
    /// The subpopulation: a variable and its value.
    pub domain: String,
    pub domain_value: String,
    /// User-missing values of factors and design variables are valid.
    pub include: bool,
    /// One flag per entry of `CS_PRINT`.
    pub print: [bool; 8],
    /// CSLOGISTIC: every n-th iteration in the history.
    pub history: String,
    /// One flag per entry of `GLM_SAVE` or `LOGISTIC_SAVE`, and the names (a root for
    /// PREDPROB); empty: the command's own.
    pub save: [bool; 2],
    pub save_names: [String; 2],
    /// CSLOGISTIC: how many categories get a predicted probability.
    pub probabilities: String,
}

impl Default for SampleModel {
    fn default() -> Self {
        Self::new(false)
    }
}

impl SampleModel {
    pub fn new(logistic: bool) -> Self {
        Self {
            logistic,
            plan: String::new(),
            dependent: String::new(),
            reference: 0,
            reference_value: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            model: 0,
            terms: Terms::default(),
            intercept: 0,
            show_intercept: true,
            statistics: if logistic { [false; 7] } else { GLM_STATISTICS },
            test_type: 0,
            adjustment: 0,
            tests: Vec::new(),
            means: Vec::new(),
            means_at: Vec::new(),
            odds: Vec::new(),
            ci_level: "95".into(),
            df: String::new(),
            iterations: "100".into(),
            halvings: "5".into(),
            pconverge: "0.000001".into(),
            lconverge: "0".into(),
            separation: "20".into(),
            domain: String::new(),
            domain_value: String::new(),
            include: false,
            print: DEFAULT_PRINT,
            history: "1".into(),
            save: [false; 2],
            save_names: Default::default(),
            probabilities: "25".into(),
        }
    }

    /// The statistics and tables this command offers: CSGLM has neither EXP nor the last two
    /// tables.
    pub fn offered(&self) -> (usize, usize) {
        if self.logistic { (7, 8) } else { (6, 6) }
    }

    /// Drops the odds ratios and held values of variables taken out of Factors and Covariates,
    /// and starts over the level and value of one that moved between the two lists, so a
    /// factor's value is never read as a covariate's change.
    pub fn forget_old_odds(&mut self) {
        let (factors, covariates) = (&self.factors, &self.covariates);
        // Some(true) for a covariate, Some(false) for a factor
        let role = |n: &str| {
            if factors.iter().any(|f| f == n) {
                Some(false)
            } else {
                covariates.iter().any(|c| c == n).then_some(true)
            }
        };
        self.odds
            .retain(|o| o.variable.is_empty() || role(&o.variable).is_some());
        for o in &mut self.odds {
            if let Some(covariate) = role(&o.variable)
                && covariate != o.covariate
            {
                o.covariate = covariate;
                o.level = 0;
                o.reference.clear();
            }
            o.control.retain(|h| role(&h.name).is_some());
            for h in &mut o.control {
                let covariate = role(&h.name) == Some(true);
                if covariate != h.covariate {
                    h.covariate = covariate;
                    h.level = 0;
                    h.value.clear();
                }
            }
        }
    }

    pub fn model_variables(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let command = if self.logistic { "CSLOGISTIC" } else { "CSGLM" };
        let plan = plan_file(&self.plan)?;
        let variables = model_variables(&self.dependent, &self.factors, &self.covariates)?;
        let dependent = self.dependent.trim();
        let mut numbers: Vec<&String> = self.covariates.iter().collect();
        if !self.logistic {
            numbers.push(&self.dependent);
        }
        numeric(data, numbers)?;
        // values are quoted for strings and for dates and times
        let quoted = |name: &str| match data.variable(name) {
            Ok(v) => Ok(v.is_string() || crate::edits::shows_date(v.format())),
            Err(_) => Err(format!("{name} is no longer a variable")),
        };

        let mut text = format!("{command} {dependent}");
        if self.logistic && self.reference > 0 {
            let reference = match self.reference {
                1 => "LOW".to_string(),
                _ => match self.reference_value.trim() {
                    "" => return Err("give the value of the reference category".into()),
                    given => value(given, quoted(dependent)?)?,
                },
            };
            text.push_str(&format!("({reference})"));
        }
        text.push_str(&variables[dependent.len()..]);
        text.push_str(&plan);

        let model = self.model_variables();
        let only = self.intercept == 2;
        let intercept = self.intercept != 1;
        if !intercept && model.is_empty() {
            return Err("without the intercept the model needs factors or covariates".into());
        }
        let main = main_effects(&model);
        let terms = match self.model {
            _ if only => Vec::new(),
            0 => main.clone(),
            1 => full_factorial(&self.factors, &self.covariates)?,
            _ => {
                self.terms.syntax(&model, "model")?;
                self.terms.terms.clone()
            }
        };
        if !only && terms != main {
            let written: Vec<String> = terms.iter().map(|t| t.join("*")).collect();
            text.push_str(&format!("\n  /MODEL {}", written.join(" ")));
        }
        let mut parts = Vec::new();
        if self.intercept > 0 {
            parts.push(format!("INCLUDE={}", CS_INTERCEPTS[self.intercept].0));
        } else if !self.show_intercept {
            parts.push("SHOW=NO".into());
        }
        if !parts.is_empty() {
            text.push_str(&format!("\n  /INTERCEPT {}", parts.join(" ")));
        }
        for (i, test) in self.tests.iter().enumerate() {
            text.push_str(&custom(test, &terms, intercept, i + 1)?);
        }
        // a variable the terms leave out has no means or odds ratios
        let in_model = |name: &str| terms.iter().any(|t| t.iter().any(|v| v == name));
        if !self.logistic {
            text.push_str(&self.means_syntax(&in_model)?);
        } else {
            for (i, o) in self.odds.iter().enumerate() {
                text.push_str(&self.odds_syntax(o, i + 1, &quoted, &in_model)?);
            }
        }

        let criteria = self.criteria()?;
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        let (statistics, tables) = self.offered();
        let flags = &self.statistics[..statistics];
        let defaults = if self.logistic {
            &[false; 7][..statistics]
        } else {
            &GLM_STATISTICS[..statistics]
        };
        if flags != defaults {
            let chosen = keywords(&CS_STATISTICS, flags);
            if chosen.is_empty() {
                return Err("choose at least one statistic of the estimates".into());
            }
            text.push_str(&format!("\n  /STATISTICS {}", chosen.join(" ")));
        }
        let mut parts = Vec::new();
        if self.test_type > 0 {
            parts.push(format!("TYPE={}", CS_TESTS[self.test_type].0));
        }
        if self.adjustment > 0 {
            parts.push(format!("PADJUST={}", CS_ADJUSTMENTS[self.adjustment].0));
        }
        if !parts.is_empty() {
            text.push_str(&format!("\n  /TEST {}", parts.join(" ")));
        }
        let domain = self.domain.trim();
        if !domain.is_empty() {
            if domain == dependent || model.iter().any(|n| n == domain) {
                return Err(format!(
                    "{domain} is in the model, so it cannot define the subpopulation"
                ));
            }
            let given = match self.domain_value.trim() {
                "" => return Err(format!("give the value of {domain} for the subpopulation")),
                given => value(given, quoted(domain)?)?,
            };
            text.push_str(&format!("\n  /DOMAIN VARIABLE={domain}({given})"));
        }
        if self.include {
            text.push_str("\n  /MISSING CLASSMISSING=INCLUDE");
        }
        let print = &self.print[..tables];
        if print != &DEFAULT_PRINT[..tables] {
            let every = match self.logistic && print.last() == Some(&true) {
                true => whole(&self.history, 1, "the iteration history step")?,
                false => None,
            };
            let print: Vec<String> = keywords(&CS_PRINT, print)
                .into_iter()
                .map(|k| match every {
                    Some(n) if k == "HISTORY" && n != 1 => format!("HISTORY({n})"),
                    _ => k.to_string(),
                })
                .collect();
            let print = match print.is_empty() {
                true => "NONE".to_string(),
                false => print.join(" "),
            };
            text.push_str(&format!("\n  /PRINT {print}"));
        }
        text.push_str(&self.save_syntax(data)?);
        text.push('.');
        Ok(text)
    }

    /// One `/EMMEANS` per table.
    fn means_syntax(&self, in_model: &dyn Fn(&str) -> bool) -> Result<String, String> {
        let mut text = String::new();
        // the values count only for tables of means
        let mut other = Vec::new();
        if !self.means.is_empty() {
            for (n, v) in covariate_values(&self.covariates, &self.means_at)? {
                if !in_model(n) {
                    return Err(format!(
                        "the means hold {n} at a value, but {n} is not in the model"
                    ));
                }
                other.push(format!("{n}({v})"));
            }
        }
        for (i, table) in self.means.iter().enumerate() {
            let (crossed, compare) = table.checked(&self.factors, i + 1)?;
            if let Some(f) = crossed.iter().find(|f| !in_model(f)) {
                return Err(format!(
                    "{f} in estimated marginal means table {} is not in the model",
                    i + 1
                ));
            }
            let mut parts = Vec::new();
            if !crossed.is_empty() {
                parts.push(format!("TABLES={}", crossed.join("*")));
            }
            if !other.is_empty() {
                parts.push(format!("OTHER=[{}]", other.join(" ")));
            }
            if let Some(c) = compare {
                parts.push(format!("COMPARE={c}"));
                if table.contrast > 0 {
                    parts.push(format!("CONTRAST={}", CS_CONTRASTS[table.contrast].0));
                }
            }
            text.push_str("\n  /EMMEANS");
            if !parts.is_empty() {
                text.push_str(&format!(" {}", parts.join(" ")));
            }
        }
        Ok(text)
    }

    /// `/ODDSRATIOS FACTOR=[...]` or `COVARIATE=[...]`, with `CONTROL=[...]` when the others are
    /// held at given values.
    fn odds_syntax(
        &self,
        o: &OddsRatio,
        n: usize,
        quoted: &dyn Fn(&str) -> Result<bool, String>,
        in_model: &dyn Fn(&str) -> bool,
    ) -> Result<String, String> {
        let variable = o.variable.trim();
        // the level of a factor: nothing for the highest (the default), LOW, or a value typed,
        // always quoted for strings and formatted values, so a category "Low" stays a value
        let level = |name: &str, level: usize, given: &str| -> Result<Option<String>, String> {
            match level {
                0 => Ok(None),
                1 => Ok(Some("LOW".into())),
                _ => match given.trim() {
                    "" => Err(format!("give the value of {name} in odds ratio {n}")),
                    given => value(given, quoted(name)?).map(Some),
                },
            }
        };
        let listed = |name: &str| {
            self.factors
                .iter()
                .chain(&self.covariates)
                .any(|m| m == name)
        };
        if listed(variable) && !in_model(variable) {
            return Err(format!("{variable} in odds ratio {n} is not in the model"));
        }
        let mut text = if self.factors.iter().any(|f| f == variable) {
            match level(variable, o.level, &o.reference)? {
                None => format!("FACTOR=[{variable}]"),
                Some(at) => format!("FACTOR=[{variable}({at})]"),
            }
        } else if self.covariates.iter().any(|c| c == variable) {
            let mut changes = Vec::new();
            for change in o.reference.split_whitespace() {
                let c = parse(change, &format!("a change of {variable} in odds ratio {n}"))?;
                if c == 0.0 {
                    return Err(format!(
                        "odds ratio {n}: a change of {variable} cannot be 0"
                    ));
                }
                changes.push(number(c));
            }
            match changes.is_empty() {
                true => format!("COVARIATE=[{variable}]"),
                false => format!("COVARIATE=[{variable}({})]", changes.join(" ")),
            }
        } else if variable.is_empty() {
            return Err(format!("choose the factor or covariate of odds ratio {n}"));
        } else {
            return Err(format!(
                "{variable} in odds ratio {n} is no longer a factor or covariate"
            ));
        };
        let mut control = Vec::new();
        for name in self.model_variables().iter().filter(|m| *m != variable) {
            let Some(held) = o.control.iter().find(|h| &h.name == name) else {
                continue;
            };
            let written = if self.factors.contains(name) {
                level(name, held.level, &held.value)?
            } else if held.level == 1 {
                return Err(format!(
                    "odds ratio {n}: {name} is a covariate, held at its mean or a value"
                ));
            } else if held.level == 2 {
                Some(number(parse(
                    &held.value,
                    &format!("the value of {name} in odds ratio {n}"),
                )?))
            } else {
                None
            };
            let Some(written) = written else {
                continue;
            };
            if !in_model(name) {
                return Err(format!(
                    "odds ratio {n} holds {name} at a value, but {name} is not in the model"
                ));
            }
            control.push(format!("{name}({written})"));
        }
        if !control.is_empty() {
            text.push_str(&format!(" CONTROL=[{}]", control.join(" ")));
        }
        Ok(format!("\n  /ODDSRATIOS {text}"))
    }

    /// The criteria that differ from their defaults.
    fn criteria(&self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let level = ci_level(&self.ci_level)?;
        if level != 95.0 {
            out.push(format!("CILEVEL={}", number(level)));
        }
        if !self.df.trim().is_empty() {
            let df = parse(&self.df, "the degrees of freedom")?;
            if df <= 0.0 {
                return Err("the degrees of freedom must be above 0".into());
            }
            out.push(format!("DF={}", number(df)));
        }
        if !self.logistic {
            return Ok(out);
        }
        let numbers = [
            (
                "CHKSEP",
                &self.separation,
                0,
                20,
                "the start of the separation check",
            ),
            (
                "MXITER",
                &self.iterations,
                0,
                100,
                "the maximum number of iterations",
            ),
            ("MXSTEP", &self.halvings, 1, 5, "the maximum step-halving"),
        ];
        for (keyword, text, least, default, what) in numbers {
            match whole(text, least, what)? {
                Some(n) if n != default => out.push(format!("{keyword}={n}")),
                _ => {}
            }
        }
        for (keyword, text, default, what) in [
            (
                "LCONVERGE",
                &self.lconverge,
                0.0,
                "the log-likelihood convergence",
            ),
            (
                "PCONVERGE",
                &self.pconverge,
                1e-6,
                "the parameter convergence",
            ),
        ] {
            if text.trim().is_empty() {
                continue;
            }
            let v = parse(text, what)?;
            if v < 0.0 {
                return Err(format!("{what} must be at least 0"));
            }
            if v != default {
                out.push(format!("{keyword}=[{}]", number(v)));
            }
        }
        Ok(out)
    }

    /// `/SAVE` with the names given, checked: valid, not taken and not given twice.
    fn save_syntax(&self, data: &Dataset) -> Result<String, String> {
        let keywords = if self.logistic {
            LOGISTIC_SAVE
        } else {
            GLM_SAVE
        };
        let mut out = Vec::new();
        let mut given: Vec<String> = Vec::new();
        for (i, (keyword, label)) in keywords.iter().enumerate() {
            if !self.save[i] {
                continue;
            }
            let name = self.save_names[i].trim();
            if !name.is_empty() && !is_valid_name(name) {
                return Err(format!("{name} cannot name a variable"));
            }
            let root = *keyword == "PREDPROB";
            // a root gets suffixes, so only full names must be free
            if !name.is_empty() && !root {
                if data.variable(name).is_ok() {
                    return Err(format!("{name} is already a variable"));
                }
                if given.iter().any(|g| g.eq_ignore_ascii_case(name)) {
                    return Err(format!("{name} names two saved variables"));
                }
                given.push(name.to_string());
            }
            let count = match root {
                true => whole(
                    &self.probabilities,
                    1,
                    &format!("the number of {}", label.to_lowercase()),
                )?
                .filter(|n| *n != 25),
                false => None,
            };
            if root && !name.is_empty() {
                // the command names the variables root_1, root_2, ... up to the count
                let most = count.unwrap_or(25);
                if !is_valid_name(&format!("{name}_{most}")) {
                    return Err(format!(
                        "{name} is too long for a root: {name}_{most} cannot name a variable"
                    ));
                }
                let prefix = format!("{}_", name.to_uppercase());
                let derived = |n: &str| {
                    n.to_uppercase()
                        .strip_prefix(&prefix)
                        .and_then(|k| k.parse::<u32>().ok().filter(|v| v.to_string() == k))
                        .is_some_and(|k| (1..=most).contains(&k))
                };
                let other = self.save_names[1].trim();
                let taken = data
                    .variable_names()
                    .find(|n| derived(n))
                    .or((self.save[1] && derived(other)).then_some(other));
                if let Some(n) = taken {
                    return Err(format!(
                        "the root {name} would name a predicted probability {n}, which is taken"
                    ));
                }
            }
            out.push(match (name, count) {
                ("", None) => keyword.to_string(),
                (name, None) => format!("{keyword}({name})"),
                (name, Some(n)) => format!("{keyword}({name}:{n})"),
            });
        }
        Ok(match out.is_empty() {
            true => String::new(),
            false => format!("\n  /SAVE {}", out.join(" ")),
        })
    }
}

/// `/CUSTOM` of test `n`: rows typed as in the other dialogs, an effect then its coefficients,
/// checked against `terms`. INTERCEPT, when the model has one, comes first in a row and is
/// written as its bare coefficient; a row may end with `= value`, the value it is tested
/// against instead of 0.
fn custom(
    t: &ContrastTest,
    terms: &[Vec<String>],
    intercept: bool,
    n: usize,
) -> Result<String, String> {
    let mut rows = Vec::new();
    let mut k = Vec::new();
    // whether the test uses ALL rather than effect names: never both, as in the other dialogs
    let mut all_form = None;
    for line in t.rows.lines() {
        let (l, against) = match line.split_once('=') {
            Some((l, v)) => (l, Some(v)),
            None => (line, None),
        };
        let row = row_part(l, n, false, false, "an effect of the model", &|e| {
            (intercept && e.eq_ignore_ascii_case("INTERCEPT"))
                || terms.iter().any(|t| same_term(t, e))
        })?;
        if row.is_empty() {
            if against.is_some() {
                return Err(format!("test {n}: = needs a row before it"));
            }
            continue;
        }
        let tokens: Vec<&str> = row.split_whitespace().collect();
        let names: Vec<usize> = (0..tokens.len())
            .filter(|&i| coefficient(tokens[i]).is_none())
            .collect();
        let is = |i: usize, word: &str| tokens[i].eq_ignore_ascii_case(word);
        if names.iter().any(|&i| i > 0 && is(i, "INTERCEPT")) {
            return Err(format!("test {n}: INTERCEPT comes first in a row"));
        }
        if is(0, "INTERCEPT") && names.iter().any(|&i| is(i, "ALL")) {
            return Err(format!(
                "test {n}: ALL gives the intercept its coefficient, so INTERCEPT goes in a row of its own"
            ));
        }
        // a leading INTERCEPT is written as a bare number, so it goes with either form
        for &i in names.iter().filter(|&&i| i > 0 || !is(i, "INTERCEPT")) {
            let all = is(i, "ALL");
            if *all_form.get_or_insert(all) != all {
                return Err(format!(
                    "test {n}: ALL and effect names cannot be mixed in one test"
                ));
            }
        }
        let row = if is(0, "INTERCEPT") {
            if names.get(1).copied().unwrap_or(tokens.len()) != 2 {
                return Err(format!("test {n}: INTERCEPT takes one coefficient"));
            }
            tokens[1..].join(" ")
        } else {
            row
        };
        k.push(match against {
            None => "0".to_string(),
            Some(v) => coefficient(v.trim())
                .ok_or_else(|| format!("test {n}: the value after = must be a number"))?,
        });
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(format!("give the coefficients of test {n}"));
    }
    let label = match t.label.trim() {
        "" => String::new(),
        label => format!(" LABEL={}", quote(label)),
    };
    let mut text = format!("\n  /CUSTOM{label} LMATRIX={}", rows.join("; "));
    if k.iter().any(|v| v != "0") {
        text.push_str(&format!(" KMATRIX={}", k.join("; ")));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};
    use statscraft_engine::session::Session;

    /// The survey file with a one-stage plan, stratified by region, written in a folder of its
    /// own; the session, the plan's path and the folder.
    fn survey(name: &str) -> (Session, String, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("statscraft-gui-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plan.json").to_string_lossy().into_owned();
        let mut session = open("survey_basic.sav");
        run_in(
            &mut session,
            &format!(
                "CSPLAN ANALYSIS /PLAN FILE={} /PLANVARS ANALYSISWEIGHT=weight\n  \
                 /DESIGN STRATA=region /ESTIMATOR TYPE=WR.",
                quote(&path)
            ),
        );
        (session, path, dir)
    }

    fn held(name: &str, level: usize, value: &str) -> Held {
        Held {
            name: name.into(),
            level,
            value: value.into(),
            covariate: false,
        }
    }

    fn data(session: &Session) -> Dataset {
        session.dataset().unwrap().clone()
    }

    /// The syntax parses without an error and runs; the number of tables.
    fn runs_in(session: &mut Session, syntax: &str) -> usize {
        let (_, errors) = statscraft_syntax::parse(syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        run_in(session, syntax)
    }

    #[test]
    fn general_linear_model() {
        let (mut session, path, dir) = survey("csglm");
        let data = data(&session);
        let mut d = SampleModel::default();
        assert_eq!(d.syntax(&data).unwrap_err(), "choose the plan file");
        d.plan = path.clone();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "choose the dependent variable"
        );
        d.dependent = "plan".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
        d.dependent = "monthly_spend".into();
        d.factors = v(&["gender", "plan"]);
        d.covariates = v(&["age"]);
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            format!(
                "CSGLM monthly_spend BY gender plan WITH age\n  /PLAN FILE={}.",
                quote(&path)
            )
        );
        assert!(runs_in(&mut session, &syntax) >= 4);

        // every option
        d.model = 1;
        d.show_intercept = false;
        d.statistics = [true, true, false, true, true, true, false];
        d.test_type = 1;
        d.adjustment = 2;
        // a contrast of gender alone is not estimable next to gender*plan
        d.tests = vec![ContrastTest {
            label: "Ten years".into(),
            rows: "age 10 = 5".into(),
        }];
        d.means = vec![
            MeansTable::default(),
            MeansTable {
                factors: v(&["plan", "gender"]),
                compare: "gender".into(),
                contrast: 4,
                adjustment: 0,
            },
        ];
        d.means_at = vec![("age".into(), "40".into())];
        d.ci_level = "90".into();
        d.df = "150".into();
        d.domain = "region".into();
        d.domain_value = "2".into();
        d.include = true;
        d.print = [true, false, true, true, true, true, false, false];
        d.save = [true, true];
        d.save_names = ["fitted".into(), String::new()];
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.ends_with(
                "\n  /MODEL gender plan gender*plan age\n  /INTERCEPT SHOW=NO\n  \
                 /CUSTOM LABEL='Ten years' LMATRIX=age 10 KMATRIX=5\n  /EMMEANS OTHER=[age(40)]\n  \
                 /EMMEANS TABLES=gender*plan OTHER=[age(40)] COMPARE=gender CONTRAST=REPEATED\n  \
                 /CRITERIA CILEVEL=90 DF=150\n  \
                 /STATISTICS PARAMETER SE CINTERVAL DEFF DEFFSQRT\n  \
                 /TEST TYPE=ADJF PADJUST=SEQBONFERRONI\n  /DOMAIN VARIABLE=region(2)\n  \
                 /MISSING CLASSMISSING=INCLUDE\n  /PRINT SAMPLEINFO SUMMARY LMATRIX COVB CORB\n  \
                 /SAVE PRED(fitted) RESID."
            ),
            "{syntax}"
        );
        assert!(runs_in(&mut session, &syntax) >= 8);

        // custom terms, the intercept left out or alone, nothing shown
        d.model = 2;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "add at least one term to the model"
        );
        d.terms = Terms {
            terms: vec![v(&["gender"]), v(&["gender", "age"])],
            building: Vec::new(),
        };
        d.intercept = 1;
        d.tests.clear();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "plan in estimated marginal means table 2 is not in the model"
        );
        d.means.clear();
        d.domain.clear();
        d.domain_value.clear();
        d.print = [false; 8];
        d.save = [false; 2];
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.contains(
                "/MODEL gender gender*age\n  /INTERCEPT INCLUDE=NO\n  /CRITERIA CILEVEL=90 DF=150"
            ),
            "{syntax}"
        );
        assert!(syntax.contains("/PRINT NONE."), "{syntax}");
        runs_in(&mut session, &syntax);
        d.intercept = 2;
        let syntax = d.syntax(&data).unwrap();
        assert!(!syntax.contains("/MODEL"), "{syntax}");
        assert!(syntax.contains("/INTERCEPT INCLUDE=ONLY"), "{syntax}");
        runs_in(&mut session, &syntax);
        d.means = vec![MeansTable::default()];
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the means hold age at a value, but age is not in the model"
        );
        d.means.clear();

        // the checks
        d.intercept = 1;
        d.factors.clear();
        d.covariates.clear();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "without the intercept the model needs factors or covariates"
        );
        d.intercept = 0;
        d.model = 0;
        d.statistics = [false; 7];
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "choose at least one statistic of the estimates"
        );
        d.statistics = GLM_STATISTICS;
        d.covariates = v(&["age"]);
        d.domain = "age".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "age is in the model, so it cannot define the subpopulation"
        );
        d.domain = "plan".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give the value of plan for the subpopulation"
        );
        d.domain_value = "BASIC".into();
        d.tests = vec![ContrastTest {
            label: String::new(),
            rows: "INTERCEPT 1 age 40".into(),
        }];
        let syntax = d.syntax(&data).unwrap();
        assert!(syntax.contains("/CUSTOM LMATRIX=1 age 40\n"), "{syntax}");
        assert!(
            syntax.contains("/DOMAIN VARIABLE=plan('BASIC')"),
            "{syntax}"
        );
        runs_in(&mut session, &syntax);
        // a leading INTERCEPT is a bare number, so it goes with rows of ALL
        d.tests[0].rows = "INTERCEPT 1\nALL 0 1".into();
        let syntax = d.syntax(&data).unwrap();
        assert!(syntax.contains("/CUSTOM LMATRIX=1; ALL 0 1\n"), "{syntax}");
        runs_in(&mut session, &syntax);
        // in one row the command refuses them: ALL already covers the intercept
        let raw = d
            .syntax(&data)
            .unwrap()
            .replace("LMATRIX=1; ALL", "LMATRIX=1 ALL");
        assert!(session.run(&raw, None).is_err(), "{raw}");
        d.tests[0].rows = "INTERCEPT 1 ALL 0 1".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "test 1: ALL gives the intercept its coefficient, so INTERCEPT goes in a row of its own"
        );
        d.tests.clear();
        d.save = [true, false];
        d.save_names[0] = "age".into();
        assert_eq!(d.syntax(&data).unwrap_err(), "age is already a variable");
        d.save_names[0] = "2nd".into();
        assert_eq!(d.syntax(&data).unwrap_err(), "2nd cannot name a variable");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn logistic_regression() {
        let (mut session, path, dir) = survey("cslogistic");
        let data = data(&session);
        let mut d = SampleModel {
            plan: path.clone(),
            dependent: "q5_1".into(),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            ..SampleModel::new(true)
        };
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            format!(
                "CSLOGISTIC q5_1 BY gender WITH age\n  /PLAN FILE={}.",
                quote(&path)
            )
        );
        assert!(runs_in(&mut session, &syntax) >= 4);

        d.reference = 1;
        d.statistics = [true, true, true, true, false, false, true];
        d.tests = vec![ContrastTest {
            label: "Age".into(),
            rows: "age 1".into(),
        }];
        d.odds = vec![
            OddsRatio {
                variable: "gender".into(),
                level: 1,
                control: vec![held("age", 2, "40")],
                ..Default::default()
            },
            OddsRatio {
                variable: "age".into(),
                reference: "1 10".into(),
                control: vec![held("gender", 2, "2")],
                ..Default::default()
            },
        ];
        d.iterations = "50".into();
        d.halvings = "10".into();
        d.pconverge = "1e-8".into();
        d.lconverge = "0.0001".into();
        d.separation = "0".into();
        d.print = [true, true, true, false, false, false, true, true];
        d.history = "2".into();
        d.save = [true, true];
        d.save_names = ["prob".into(), "guess".into()];
        d.probabilities = "2".into();
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.starts_with("CSLOGISTIC q5_1(LOW) BY gender WITH age\n"),
            "{syntax}"
        );
        assert!(
            syntax.ends_with(
                "\n  /CUSTOM LABEL='Age' LMATRIX=age 1\n  \
                 /ODDSRATIOS FACTOR=[gender(LOW)] CONTROL=[age(40)]\n  \
                 /ODDSRATIOS COVARIATE=[age(1 10)] CONTROL=[gender(2)]\n  \
                 /CRITERIA CHKSEP=0 MXITER=50 MXSTEP=10 LCONVERGE=[0.0001] PCONVERGE=[0.00000001]\n  \
                 /STATISTICS PARAMETER SE TTEST CINTERVAL EXP\n  \
                 /PRINT SAMPLEINFO VARIABLEINFO SUMMARY CLASSTABLE HISTORY(2)\n  \
                 /SAVE PREDPROB(prob:2) PREDVAL(guess)."
            ),
            "{syntax}"
        );
        assert!(runs_in(&mut session, &syntax) >= 8);

        // the root leaves room for its suffixes, and the names it makes are free
        d.save_names[0] = "r".repeat(63);
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            format!(
                "{0} is too long for a root: {0}_2 cannot name a variable",
                "r".repeat(63)
            )
        );
        d.save_names[0] = "q5".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the root q5 would name a predicted probability q5_1, which is taken"
        );
        d.save_names = ["p".into(), "P_2".into()];
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the root p would name a predicted probability P_2, which is taken"
        );
        d.save_names[1] = "p_3".into();
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("PREDPROB(p:2) PREDVAL(p_3).")
        );

        // a covariate is held at its mean or a value, never its lowest level
        d.odds[0].control[0].level = 1;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "odds ratio 1: age is a covariate, held at its mean or a value"
        );

        // a string dependent with a value as its reference, the factor at a value too
        let mut d = SampleModel {
            plan: path.clone(),
            dependent: "plan".into(),
            reference: 2,
            factors: v(&["gender"]),
            ..SampleModel::new(true)
        };
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give the value of the reference category"
        );
        d.reference_value = "BASIC".into();
        d.odds = vec![OddsRatio {
            variable: "gender".into(),
            level: 2,
            ..Default::default()
        }];
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give the value of gender in odds ratio 1"
        );
        d.odds[0].reference = "1".into();
        d.save = [true, false];
        d.probabilities = String::new();
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.starts_with("CSLOGISTIC plan('BASIC') BY gender\n"),
            "{syntax}"
        );
        assert!(
            syntax.ends_with("/ODDSRATIOS FACTOR=[gender(1)]\n  /SAVE PREDPROB."),
            "{syntax}"
        );
        runs_in(&mut session, &syntax);

        // the checks
        d.odds[0].variable = "age".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "age in odds ratio 1 is no longer a factor or covariate"
        );
        d.covariates = v(&["age"]);
        d.odds[0].reference = "0".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "odds ratio 1: a change of age cannot be 0"
        );
        d.odds[0].reference.clear();
        d.intercept = 2;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "age in odds ratio 1 is not in the model"
        );
        // a category typed as a value stays a value, even one named like a keyword
        d.intercept = 0;
        d.factors = v(&["gender", "q5_2"]);
        d.model = 2;
        d.terms = Terms {
            terms: vec![v(&["gender"]), v(&["age"])],
            building: Vec::new(),
        };
        d.odds = vec![OddsRatio {
            variable: "gender".into(),
            level: 0,
            reference: "Low".into(),
            control: vec![held("q5_2", 1, "")],
            covariate: false,
        }];
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "odds ratio 1 holds q5_2 at a value, but q5_2 is not in the model"
        );
        d.odds[0].control[0].level = 0;
        d.odds[0].level = 2;
        assert_eq!(d.syntax(&data).unwrap_err(), "\"Low\" is not a number");
        d.dependent = "q5_1".into();
        d.reference = 0;
        d.factors = v(&["plan"]);
        d.terms.terms[0] = v(&["plan"]);
        d.odds[0].variable = "plan".into();
        d.odds[0].control.clear();
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.contains("/ODDSRATIOS FACTOR=[plan('Low')]\n"),
            "{syntax}"
        );
        d.odds.clear();
        d.covariates = v(&["q3_comment"]);
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "q3_comment is a string; the command needs numbers"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn custom_rows() {
        let terms = vec![v(&["gender"]), v(&["gender", "age"])];
        let test = |rows: &str, intercept: bool| {
            let t = ContrastTest {
                label: String::new(),
                rows: rows.into(),
            };
            custom(&t, &terms, intercept, 1)
        };
        assert_eq!(
            test("INTERCEPT 1/2 age*gender 1 -1\ngender 1 0 -1 = -1/4", true).unwrap(),
            "\n  /CUSTOM LMATRIX=1/2 age*gender 1 -1; gender 1 0 -1 KMATRIX=0; -1/4"
        );
        assert_eq!(
            test("ALL 1 0 0\nALL 0 1 0 = 2", true).unwrap(),
            "\n  /CUSTOM LMATRIX=ALL 1 0 0; ALL 0 1 0 KMATRIX=0; 2"
        );
        assert_eq!(
            test("gender 1 0 -1\nALL 1 0 0", true).unwrap_err(),
            "test 1: ALL and effect names cannot be mixed in one test"
        );
        assert_eq!(
            test("INTERCEPT 1", false).unwrap_err(),
            "test 1: INTERCEPT is not an effect of the model"
        );
        assert_eq!(
            test("ALL 1 0 INTERCEPT 1", true).unwrap_err(),
            "test 1: INTERCEPT comes first in a row"
        );
        assert_eq!(
            test("gender 1 INTERCEPT 1", true).unwrap_err(),
            "test 1: INTERCEPT comes first in a row"
        );
        assert_eq!(
            test("INTERCEPT 1 2 gender 1", true).unwrap_err(),
            "test 1: INTERCEPT takes one coefficient"
        );
        assert_eq!(
            test("ALL 1 gender 1", true).unwrap_err(),
            "test 1: ALL and effect names cannot be mixed in one test"
        );
        assert_eq!(
            test("gender 1 = x", true).unwrap_err(),
            "test 1: the value after = must be a number"
        );
        assert_eq!(
            test("\n", true).unwrap_err(),
            "give the coefficients of test 1"
        );
    }

    #[test]
    fn odds_ratios_forget_variables_taken_out_or_moved() {
        let mut d = SampleModel {
            factors: v(&["gender", "q5_2"]),
            covariates: v(&["age"]),
            odds: vec![
                OddsRatio {
                    variable: "gender".into(),
                    level: 2,
                    reference: "1".into(),
                    control: vec![
                        Held {
                            covariate: true,
                            ..held("age", 2, "40")
                        },
                        held("q5_2", 1, ""),
                    ],
                    covariate: false,
                },
                OddsRatio {
                    variable: "age".into(),
                    reference: "1 10".into(),
                    covariate: true,
                    ..Default::default()
                },
            ],
            ..SampleModel::new(true)
        };
        let before = d.clone();
        d.forget_old_odds();
        assert_eq!(d, before);
        // age leaves the model, q5_2 becomes a covariate
        d.factors = v(&["gender"]);
        d.covariates = v(&["q5_2"]);
        d.forget_old_odds();
        assert_eq!(d.odds.len(), 1);
        assert_eq!(
            d.odds[0].control,
            vec![Held {
                covariate: true,
                ..held("q5_2", 0, "")
            }]
        );
        // gender becomes a covariate: its value is no change
        d.factors.clear();
        d.covariates = v(&["gender", "q5_2"]);
        d.forget_old_odds();
        assert_eq!((d.odds[0].level, d.odds[0].reference.as_str()), (0, ""));
        assert!(d.odds[0].covariate);
    }
}
