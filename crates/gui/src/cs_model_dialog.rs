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

/// The reference category of the logistic dependent; the first is the default.
pub const CS_REFERENCES: [&str; 3] = ["Highest", "Lowest", "Value"];

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

/// One ODDSRATIOS subcommand: the odds ratios of one factor or covariate.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OddsRatio {
    pub variable: String,
    /// A factor: LOW, HIGH or a value (empty: the highest level). A covariate: its changes,
    /// separated by spaces (empty: 1).
    pub reference: String,
    /// The value other factors (LOW, HIGH or a value) and covariates (MEAN or a number) are
    /// held at; empty or missing: the highest level or the mean.
    pub control: Vec<(String, String)>,
}

/// CSGLM or CSLOGISTIC.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleModel {
    pub logistic: bool,
    pub plan: String,
    pub dependent: String,
    /// CSLOGISTIC: index into `CS_REFERENCES`.
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
        if !self.logistic {
            text.push_str(&self.means_syntax()?);
        } else {
            for (i, o) in self.odds.iter().enumerate() {
                text.push_str(&self.odds_syntax(o, i + 1, &quoted)?);
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
    fn means_syntax(&self) -> Result<String, String> {
        let mut text = String::new();
        // the values count only for tables of means
        let other: Vec<String> = match self.means.is_empty() {
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
    ) -> Result<String, String> {
        let variable = o.variable.trim();
        // LOW and HIGH, or a value of the factor
        let level = |name: &str, given: &str| -> Result<String, String> {
            match given.trim() {
                g if g.eq_ignore_ascii_case("LOW") || g.eq_ignore_ascii_case("HIGH") => {
                    Ok(g.to_uppercase())
                }
                _ => value(given, quoted(name)?),
            }
        };
        let mut text = if self.factors.iter().any(|f| f == variable) {
            match o.reference.trim() {
                "" => format!("FACTOR=[{variable}]"),
                given => format!("FACTOR=[{variable}({})]", level(variable, given)?),
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
            let Some((_, given)) = o.control.iter().find(|(c, _)| c == name) else {
                continue;
            };
            let given = given.trim();
            if given.is_empty() {
                continue;
            }
            let written = if self.factors.contains(name) {
                level(name, given)?
            } else if given.eq_ignore_ascii_case("MEAN") {
                "MEAN".into()
            } else {
                number(parse(
                    given,
                    &format!("the value of {name} in odds ratio {n}"),
                )?)
            };
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
        for &i in &names {
            let all = is(i, "ALL");
            if *all_form.get_or_insert(all) != all {
                return Err(format!(
                    "test {n}: ALL and effect names cannot be mixed in one test"
                ));
            }
        }
        if names.iter().any(|&i| i > 0 && is(i, "INTERCEPT")) {
            return Err(format!("test {n}: INTERCEPT comes first in a row"));
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
                reference: "low".into(),
                control: vec![("age".into(), "40".into())],
            },
            OddsRatio {
                variable: "age".into(),
                reference: "1 10".into(),
                control: vec![("gender".into(), "2".into())],
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
            reference: "1".into(),
            control: Vec::new(),
        }];
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
}
