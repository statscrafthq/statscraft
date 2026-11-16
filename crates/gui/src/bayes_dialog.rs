//! The Bayesian Statistics dialogs: one sample of a normal, binomial or Poisson variable, related
//! samples, independent samples, Pearson correlation, one-way ANOVA, linear regression, repeated
//! measures and the independence of a two-way table (BAYES ONESAMPLE, RELATED, INDEPENDENT,
//! CORRELATION, ANOVA, REGRESSION, REPEATED and LOGLINEAR). One struct serves every kind; a new
//! BAYES command is a new kind with its fields.
//! The test settings of a one-sample or related dialog apply to every variable or pair. Options
//! left as they are add nothing to the syntax, so the command runs with its own defaults, and the
//! settings of a part the analysis does not use (the prior without a posterior, the Bayes factor
//! without a factor) are neither checked nor written.

use crate::advanced_dialogs::ci_level;
use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::stats_dialogs::numeric;
use crate::survival_dialog::{numbers, saved, values};
use crate::syntax_text::{names, number, quote, value};
use statscraft_data::Dataset;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BayesKind {
    Normal,
    Binomial,
    Poisson,
    Related,
    Independent,
    Correlation,
    Anova,
    Regression,
    Repeated,
    Loglinear,
}

/// The items of Analyze > Bayesian Statistics.
pub const BAYES_MENU: [(&str, BayesKind); 10] = [
    ("One Sample Normal...", BayesKind::Normal),
    ("One Sample Binomial...", BayesKind::Binomial),
    ("One Sample Poisson...", BayesKind::Poisson),
    ("Related Samples Normal...", BayesKind::Related),
    ("Independent Samples Normal...", BayesKind::Independent),
    ("Pearson Correlation...", BayesKind::Correlation),
    ("One-way ANOVA...", BayesKind::Anova),
    ("Linear Regression...", BayesKind::Regression),
    ("Repeated Measures...", BayesKind::Repeated),
    ("Log-Linear Regression Models...", BayesKind::Loglinear),
];

/// ANALYSIS, the default first.
pub const BAYES_ANALYSES: [(&str, &str); 3] = [
    ("POSTERIOR", "Posterior distribution"),
    ("BAYESFACTOR", "Bayes factor"),
    ("BOTH", "Both"),
];

/// The prior of the variance (VARDIST) or of the precision (PRECDIST) of a normal variable.
pub const BAYES_VARIANCE_PRIORS: [(&str, &str); 7] = [
    ("VARDIST=DIFFUSE", "Diffuse"),
    ("VARDIST=INVCHISQ", "Inverse chi-square (df, scale)"),
    ("VARDIST=INVGAMMA", "Inverse gamma (shape, scale)"),
    ("VARDIST=JEFFREYS2", "Jeffreys, 1 / variance"),
    ("VARDIST=JEFFREYS4", "Jeffreys, 1 / variance squared"),
    ("PRECDIST=GAMMA", "Gamma on the precision (shape, rate)"),
    ("PRECDIST=CHISQ", "Chi-square on the precision (df)"),
];

/// How many numbers each entry of `BAYES_VARIANCE_PRIORS` takes; the conjugate ones take some.
pub fn variance_parameters(prior: usize) -> usize {
    match prior {
        1 | 2 | 5 => 2,
        6 => 1,
        _ => 0,
    }
}

/// Which values of a binomial variable are successes (SUCCESS), the default first.
pub const BAYES_SUCCESSES: [(&str, &str); 5] = [
    ("LAST", "The highest value"),
    ("FIRST", "The lowest value"),
    ("MIDPOINT", "From the middle of the range up"),
    ("CUTPOINT", "From a cut point up"),
    ("LEVEL", "The values listed"),
];

pub const SUCCESS_CUTPOINT: usize = 3;
pub const SUCCESS_LEVEL: usize = 4;

/// How the grouping variable forms two groups (SELECT).
pub const BAYES_SELECTS: [(&str, &str); 3] = [
    ("LEVEL", "Two values"),
    ("MIDPOINT", "Below the middle of the range, and the rest"),
    ("CUTPOINT", "Below a cut point, and the rest"),
];

pub const SELECT_CUTPOINT: usize = 2;

/// The Bayes factor of two independent means (COMPUTATION), the default first.
pub const BAYES_FACTORS: [(&str, &str); 3] = [
    ("ROUDER", "Rouder: Cauchy prior on the effect size"),
    ("GONEN", "Gonen: normal prior on the effect size"),
    ("HYPER", "Hyper-prior"),
];

pub const GONEN: usize = 1;
pub const HYPER: usize = 2;

/// The prior of the common variance of two groups (VARDIST), the default first.
pub const BAYES_GROUP_PRIORS: [(&str, &str); 3] = [
    ("DIFFUSE", "Diffuse"),
    ("JEFFREYS", "Jeffreys"),
    ("INVCHISQ", "Inverse chi-square (df, scale)"),
];

pub const GROUP_INVCHISQ: usize = 2;

/// The prior on g behind the Bayes factor of a one-way ANOVA or a regression (COMPUTATION).
pub const BAYES_G_PRIORS: [(&str, &str); 4] = [
    ("JZS", "JZS: g mixed over an inverse gamma"),
    ("ZELLNER", "Zellner: a fixed g"),
    ("HYPER", "Hyper-g"),
    ("ROUDER", "Rouder: inverse gamma with a scale"),
];

pub const G_ZELLNER: usize = 1;
pub const G_HYPER: usize = 2;
pub const G_ROUDER: usize = 3;

/// The prior of the parameters and error variance of a linear model (TYPE).
pub const BAYES_LINEAR_PRIORS: [(&str, &str); 2] = [
    ("REFERENCE", "Reference: flat"),
    ("CONJUGATE", "Conjugate: normal and inverse gamma"),
];

/// The Bayes factor of repeated measures (COMPUTATION), the default first.
pub const BAYES_REPEATED_FACTORS: [(&str, &str); 2] = [
    ("BIC", "BIC approximation"),
    ("ROUDER", "Rouder: mixed model by Monte Carlo draws"),
];

pub const REPEATED_ROUDER: usize = 1;

/// What a regression saves of the predictive distribution of every case (SAVE).
pub const BAYES_SAVE: [(&str, &str); 5] = [
    ("MEANPRED", "Predicted mean"),
    ("VARIANCE", "Predictive variance"),
    ("MODE", "Predictive mode"),
    ("CIMEANPREDL", "Lower credible bound"),
    ("CIMEANPREDU", "Upper credible bound"),
];

/// How the counts of a two-way table arose (MODEL), the default first.
pub const BAYES_COUNT_MODELS: [(&str, &str); 3] = [
    ("MULTINOMIAL", "Multinomial"),
    ("POISSON", "Poisson"),
    ("NONPARAMETRIC", "Nonparametric"),
];

pub const COUNT_MULTINOMIAL: usize = 0;
pub const COUNT_POISSON: usize = 1;
pub const COUNT_NONPARAMETRIC: usize = 2;

/// The totals of a two-way table fixed by the design (FIX_MARGIN), the default first.
pub const BAYES_MARGINS: [(&str, &str); 3] = [
    ("TOTAL", "The grand total"),
    ("ROW", "The row totals"),
    ("COLUMN", "The column totals"),
];

/// The prior of the Bayes factor of a multinomial table (PRIOR_TYPE), the default first.
pub const BAYES_TABLE_PRIORS: [(&str, &str); 3] = [
    ("CONJUGATE", "Conjugate"),
    ("MIXTURE_DIRICHLET", "Mixture of Dirichlet priors"),
    ("INTRINSIC", "Intrinsic (2 x 2 tables)"),
];

/// The contents of the crosstab of a two-way table (PRINT CELLS).
pub const BAYES_CELLS: [(&str, &str); 5] = [
    ("OBSERVED", "Observed counts"),
    ("EXPECTED", "Expected counts"),
    ("ROW", "Row percentages"),
    ("COLUMN", "Column percentages"),
    ("TOTAL", "Total percentages"),
];

/// One F test of a regression: its parameters equal the test values (0 when empty).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FTestRow {
    pub label: String,
    pub variables: Vec<String>,
    pub intercept: bool,
    pub values: String,
}

/// Every BAYES command: the fields of every kind; each kind uses its own.
#[derive(Clone, PartialEq, Debug)]
pub struct Bayesian {
    pub kind: BayesKind,
    pub variables: Vec<String>,
    /// The pairs of a related-samples test, first minus second.
    pub pairs: Vec<(String, String)>,
    pub group: String,
    /// Index into `BAYES_ANALYSES`.
    pub analysis: usize,
    pub ci_level: String,
    pub tolerance: String,
    pub max_iterations: String,
    /// METHOD=MCA: the Bayes factor by Monte Carlo draws.
    pub monte_carlo: bool,
    /// Empty for a random seed.
    pub seed: String,
    pub mc_samples: String,
    pub listwise: bool,
    /// NULLVALUE, the known VARIANCE and ALTERGVALUE (empty: g integrated out) of a mean; `g`
    /// is also the fixed g of a Zellner prior.
    pub null: String,
    pub known_variance: String,
    pub g: String,
    /// Index into `BAYES_VARIANCE_PRIORS`, `BAYES_GROUP_PRIORS` or `BAYES_LINEAR_PRIORS`, with
    /// its numbers.
    pub variance_prior: usize,
    pub prior_a: String,
    pub prior_b: String,
    /// MEANDIST=NORMAL instead of DIFFUSE.
    pub normal_mean: bool,
    pub prior_mean: String,
    pub prior_mean_variance: String,
    pub kappa: String,
    /// Index into `BAYES_SUCCESSES`, with the cut point or the values.
    pub success: usize,
    pub success_value: String,
    /// NULLPRIORPARAM, ALTERPRIORPARAM and TYPE=CONJUGATE: two numbers each.
    pub null_prior: String,
    pub alter_prior: String,
    pub conjugate: String,
    /// Index into `BAYES_SELECTS`, with the two values or the cut point.
    pub select: usize,
    pub level1: String,
    pub level2: String,
    pub cutpoint: String,
    /// DATAVAR: the two known group variances.
    pub known_variances: bool,
    pub variance1: String,
    pub variance2: String,
    /// Index into `BAYES_FACTORS`, `BAYES_G_PRIORS` or `BAYES_REPEATED_FACTORS`: the Rouder
    /// scale, the Gonen mean and variance, the hyper a.
    pub factor: usize,
    pub rouder: String,
    pub gonen_mean: String,
    pub gonen_variance: String,
    pub hyper: String,
    pub equal_variances: bool,
    /// MEANDIST=NORMAL of two groups: the two means, then the two variances.
    pub group_means: [String; 4],
    /// CVALUE: the power of 1 - rho squared in the prior.
    pub cvalue: String,
    /// TYPE=FRACTION instead of JZS, with the fraction and the null correlation.
    pub fraction: bool,
    pub fraction_value: String,
    pub fraction_null: String,
    /// The dependent of a one-way ANOVA (whose factor is `group`) or a regression.
    pub dependent: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// REGWGT; empty for none.
    pub weight: String,
    /// COMPARE=FULL of a regression.
    pub compare_full: bool,
    /// VMATRIX: the rows of the lower triangle separated by `;`, empty for the identity. The
    /// conjugate prior of a linear model (`variance_prior` 1) takes its shape, scale and means
    /// from `prior_a`, `prior_b` and `prior_mean`.
    pub vmatrix: String,
    /// PLOT MEANS of a one-way ANOVA: values of the factor.
    pub plot_values: String,
    /// The plotted variables: model variables of a regression, or repeated variables.
    pub plotted: Vec<String>,
    pub plot_error: bool,
    pub plot_intercept: bool,
    pub plot_prediction: bool,
    pub max_category: String,
    /// PREDICT REGRESSORS: the value typed for each factor and covariate.
    pub predict: BTreeMap<String, String>,
    /// One flag and one optional new name per entry of `BAYES_SAVE`.
    pub save: [bool; 5],
    pub save_names: [String; 5],
    pub ftests: Vec<FTestRow>,
    /// The two variables of a log-linear table; the prior of its Bayes factor is `factor`, an
    /// index into `BAYES_TABLE_PRIORS`, and `mc_samples` its posterior draws (POSTSAMPLES).
    pub row: String,
    pub column: String,
    /// Indexes into `BAYES_COUNT_MODELS` and `BAYES_MARGINS`.
    pub count_model: usize,
    pub margin: usize,
    /// SHAPEPARAMS (one value, or rows separated by `;`), SCALEPARAM (empty: from the data) and
    /// LAMBDAS.
    pub shapes: String,
    pub scale: String,
    pub lambdas: String,
    /// ORDER=DESCENDING.
    pub descending: bool,
    pub suppress_table: bool,
    /// One flag per entry of `BAYES_CELLS`.
    pub cells: [bool; 5],
    pub chisq: bool,
    pub lrt: bool,
}

/// A required number.
fn required(text: &str, what: &str) -> Result<f64, String> {
    optional(text, what)?.ok_or_else(|| format!("give {what}"))
}

/// A number that must pass `ok`; None when empty or the default (NaN for a field without one).
fn setting(
    text: &str,
    what: &str,
    rule: &str,
    default: f64,
    ok: impl Fn(f64) -> bool,
) -> Result<Option<f64>, String> {
    match optional(text, what)? {
        Some(v) if !ok(v) => Err(format!("{what} must be {rule}")),
        Some(v) if v != default => Ok(Some(v)),
        _ => Ok(None),
    }
}

/// Two typed numbers that must pass `ok`.
fn two(
    text: &str,
    what: &str,
    rule: &str,
    ok: impl Fn(f64, f64) -> bool,
) -> Result<String, String> {
    match *numbers(text, what)?.as_slice() {
        [a, b] if ok(a, b) => Ok(format!("{} {}", number(a), number(b))),
        [_, _] => Err(format!("{what} must be {rule}")),
        _ => Err(format!("give two numbers for {what}")),
    }
}

fn positive(text: &str, what: &str) -> Result<f64, String> {
    match required(text, what)? {
        v if v > 0.0 => Ok(v),
        _ => Err(format!("{what} must be above 0")),
    }
}

impl Bayesian {
    pub fn new(kind: BayesKind) -> Self {
        Self {
            kind,
            variables: Vec::new(),
            pairs: Vec::new(),
            group: String::new(),
            analysis: 0,
            ci_level: "95".into(),
            tolerance: match kind {
                BayesKind::Correlation => "0.0001",
                _ => "0.000001",
            }
            .into(),
            max_iterations: "2000".into(),
            monte_carlo: false,
            seed: String::new(),
            mc_samples: match kind {
                BayesKind::Repeated => "30000",
                BayesKind::Normal
                | BayesKind::Binomial
                | BayesKind::Poisson
                | BayesKind::Related
                | BayesKind::Independent
                | BayesKind::Correlation
                | BayesKind::Anova
                | BayesKind::Regression
                | BayesKind::Loglinear => "10000",
            }
            .into(),
            listwise: false,
            null: "0".into(),
            known_variance: String::new(),
            g: String::new(),
            variance_prior: 0,
            prior_a: String::new(),
            prior_b: String::new(),
            normal_mean: false,
            prior_mean: String::new(),
            prior_mean_variance: String::new(),
            kappa: "1".into(),
            success: 0,
            success_value: String::new(),
            null_prior: "2 2".into(),
            alter_prior: "1 1".into(),
            // a uniform prior: beta(1, 1) of a proportion, gamma(1, 0) of a rate
            conjugate: match kind {
                BayesKind::Poisson => "1 0",
                _ => "1 1",
            }
            .into(),
            select: 0,
            level1: String::new(),
            level2: String::new(),
            cutpoint: String::new(),
            known_variances: false,
            variance1: String::new(),
            variance2: String::new(),
            factor: 0,
            rouder: "1".into(),
            gonen_mean: "0".into(),
            gonen_variance: String::new(),
            hyper: match kind {
                BayesKind::Anova | BayesKind::Regression => "3",
                BayesKind::Normal
                | BayesKind::Binomial
                | BayesKind::Poisson
                | BayesKind::Related
                | BayesKind::Independent
                | BayesKind::Correlation
                | BayesKind::Repeated
                | BayesKind::Loglinear => "-0.75",
            }
            .into(),
            equal_variances: false,
            group_means: Default::default(),
            cvalue: "0".into(),
            fraction: false,
            fraction_value: "0.5".into(),
            fraction_null: "0".into(),
            dependent: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            weight: String::new(),
            compare_full: false,
            vmatrix: String::new(),
            plot_values: String::new(),
            plotted: Vec::new(),
            plot_error: false,
            plot_intercept: false,
            plot_prediction: false,
            max_category: "2".into(),
            predict: BTreeMap::new(),
            save: [false; 5],
            save_names: Default::default(),
            ftests: Vec::new(),
            row: String::new(),
            column: String::new(),
            count_model: 0,
            margin: 0,
            shapes: String::new(),
            scale: String::new(),
            lambdas: String::new(),
            descending: false,
            suppress_table: false,
            cells: [true, false, false, false, false],
            chisq: true,
            lrt: false,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            BayesKind::Normal => "Bayesian One Sample Normal",
            BayesKind::Binomial => "Bayesian One Sample Binomial",
            BayesKind::Poisson => "Bayesian One Sample Poisson",
            BayesKind::Related => "Bayesian Related Samples",
            BayesKind::Independent => "Bayesian Independent Samples",
            BayesKind::Correlation => "Bayesian Pearson Correlation",
            BayesKind::Anova => "Bayesian One-way ANOVA",
            BayesKind::Regression => "Bayesian Linear Regression",
            BayesKind::Repeated => "Bayesian Repeated Measures",
            BayesKind::Loglinear => "Bayesian Log-Linear Models",
        }
    }

    pub fn posterior(&self) -> bool {
        self.analysis != 1
    }

    pub fn factor(&self) -> bool {
        self.analysis != 0
    }

    /// A mean tested against a normal model: one sample or the differences of pairs.
    pub fn normal(&self) -> bool {
        matches!(self.kind, BayesKind::Normal | BayesKind::Related)
    }

    /// The normal mean prior is used: with a known variance, or with a conjugate prior on the
    /// variance or precision.
    pub fn mean_prior_used(&self) -> bool {
        !self.known_variance.trim().is_empty() || variance_parameters(self.variance_prior) > 0
    }

    /// KAPPA is used: a normal mean prior with a conjugate prior on an unknown variance.
    pub fn kappa_used(&self) -> bool {
        self.normal_mean
            && self.known_variance.trim().is_empty()
            && variance_parameters(self.variance_prior) > 0
    }

    /// Monte Carlo settings apply: the Bayes factor of a normal mean.
    pub fn monte_carlo_offered(&self) -> bool {
        self.normal() && self.factor()
    }

    /// The criteria take the tolerance and iterations of the numerical integration.
    pub fn numerical_offered(&self) -> bool {
        match self.kind {
            BayesKind::Binomial | BayesKind::Poisson | BayesKind::Repeated => false,
            BayesKind::Normal
            | BayesKind::Related
            | BayesKind::Independent
            | BayesKind::Correlation
            | BayesKind::Anova
            | BayesKind::Regression
            | BayesKind::Loglinear => true,
        }
    }

    /// MISSING SCOPE=LISTWISE exists; the models always leave out incomplete cases.
    pub fn missing_offered(&self) -> bool {
        match self.kind {
            BayesKind::Anova
            | BayesKind::Regression
            | BayesKind::Repeated
            | BayesKind::Loglinear => false,
            BayesKind::Normal
            | BayesKind::Binomial
            | BayesKind::Poisson
            | BayesKind::Related
            | BayesKind::Independent
            | BayesKind::Correlation => true,
        }
    }

    /// Monte Carlo draws are made: by choice for a normal mean, with Rouder's method for
    /// repeated measures, for the posterior of a log-linear table.
    pub fn draws_used(&self) -> bool {
        match self.kind {
            BayesKind::Repeated => self.factor() && self.factor == REPEATED_ROUDER,
            BayesKind::Loglinear => self.posterior(),
            BayesKind::Normal
            | BayesKind::Binomial
            | BayesKind::Poisson
            | BayesKind::Related
            | BayesKind::Independent
            | BayesKind::Correlation
            | BayesKind::Anova
            | BayesKind::Regression => self.monte_carlo_offered() && self.monte_carlo,
        }
    }

    /// The prior of a linear model is used: by a one-way ANOVA for the posterior or its plots,
    /// always by a regression, whose posterior also feeds the predictions, F tests and plots.
    pub fn linear_prior_used(&self) -> bool {
        match self.kind {
            BayesKind::Anova => {
                self.posterior() || !self.plot_values.trim().is_empty() || self.plot_error
            }
            BayesKind::Regression => true,
            BayesKind::Normal
            | BayesKind::Binomial
            | BayesKind::Poisson
            | BayesKind::Related
            | BayesKind::Independent
            | BayesKind::Correlation
            | BayesKind::Repeated
            | BayesKind::Loglinear => false,
        }
    }

    /// The factors and then the covariates of a regression.
    pub fn model(&self) -> impl Iterator<Item = &String> {
        self.factors.iter().chain(&self.covariates)
    }

    /// A prediction value is typed for some variable of the regression model.
    pub fn predicting(&self) -> bool {
        self.model()
            .any(|n| self.predict.get(n).is_some_and(|v| !v.trim().is_empty()))
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let analysis =
            (self.analysis != 0).then(|| format!(" ANALYSIS={}", BAYES_ANALYSES[self.analysis].0));
        let analysis = analysis.unwrap_or_default();
        let mut text = match self.kind {
            BayesKind::Normal | BayesKind::Binomial | BayesKind::Poisson => {
                self.onesample(data, &analysis)?
            }
            BayesKind::Related => self.related(data, &analysis)?,
            BayesKind::Independent => self.independent(data, &analysis)?,
            BayesKind::Correlation => self.correlation(data, &analysis)?,
            BayesKind::Anova => self.anova(data, &analysis)?,
            BayesKind::Regression => self.regression(data, &analysis)?,
            BayesKind::Repeated => self.repeated(data, &analysis)?,
            BayesKind::Loglinear => self.loglinear(&analysis)?,
        };
        text.push_str(&self.criteria()?);
        if self.missing_offered() && self.listwise {
            text.push_str("\n  /MISSING SCOPE=LISTWISE");
        }
        text.push('.');
        Ok(text)
    }

    fn onesample(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        let list = names(&self.variables, "the test variables")?;
        let distribution = match self.kind {
            BayesKind::Binomial => "BINOMIAL",
            BayesKind::Poisson => "POISSON",
            _ => "NORMAL",
        };
        if self.kind != BayesKind::Binomial {
            numeric(data, &self.variables)?;
        }
        let mut text = format!(
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION={distribution} VARIABLES={list}{analysis}"
        );
        let mut words = Vec::new();
        let mut prior = Vec::new();
        if self.kind == BayesKind::Normal {
            self.mean_test(&mut words)?;
            prior = self.normal_prior()?;
        } else {
            let binomial = self.kind == BayesKind::Binomial;
            if binomial && let Some(success) = self.success(data)? {
                words.push(format!("SUCCESS={success}"));
            }
            // a beta prior may be Haldane's (0 0); the gamma priors of the factor need a rate
            let (ok, rule): (fn(f64, f64) -> bool, &str) = match binomial {
                true => (
                    |a, b| (a > 0.0 && b > 0.0) || (a == 0.0 && b == 0.0),
                    "both above 0, or both 0",
                ),
                false => (|a, b| a > 0.0 && b > 0.0, "both above 0"),
            };
            if self.factor() {
                let null = two(&self.null_prior, "the null hypothesis prior", rule, ok)?;
                if null != "2 2" {
                    words.push(format!("NULLPRIORPARAM={null}"));
                }
                let alter = two(&self.alter_prior, "the alternative prior", rule, ok)?;
                words.push(format!("ALTERPRIORPARAM={alter}"));
            }
            if self.posterior() {
                let conjugate = match binomial {
                    true => two(&self.conjugate, "the conjugate prior", rule, ok)?,
                    false => two(
                        &self.conjugate,
                        "the conjugate prior",
                        "a shape above 0 and a rate of at least 0",
                        |a, b| a > 0.0 && b >= 0.0,
                    )?,
                };
                prior.push(format!("TYPE=CONJUGATE({conjugate})"));
            }
        }
        if !words.is_empty() {
            text.push_str(&format!("\n  /DATA VARIABLES={list} {}", words.join(" ")));
        }
        if !prior.is_empty() {
            text.push_str(&format!("\n  /PRIOR {}", prior.join(" ")));
        }
        Ok(text)
    }

    /// SUCCESS of a binomial variable; None for the default.
    fn success(&self, data: &Dataset) -> Result<Option<String>, String> {
        let string = |n: &String| data.variable(n).is_ok_and(|v| v.is_string());
        let strings = self.variables.iter().any(string);
        let keyword = BAYES_SUCCESSES[self.success].0;
        match self.success {
            0 | 1 => Ok((self.success != 0).then(|| keyword.to_string())),
            SUCCESS_LEVEL => {
                if self.success_value.trim().is_empty() {
                    return Err("give the values that count as a success".into());
                }
                // a quoted number also matches a numeric variable
                if strings && self.variables.iter().any(|n| !string(n)) {
                    numbers(
                        &self.success_value,
                        "each success value of a numeric variable",
                    )?;
                }
                Ok(Some(format!(
                    "LEVEL({})",
                    values(&self.success_value, strings)?
                )))
            }
            _ => {
                if let Some(n) = self.variables.iter().find(|n| string(n)) {
                    return Err(format!("{n} is a string; list its success values instead"));
                }
                Ok(Some(match self.success {
                    SUCCESS_CUTPOINT => {
                        let cut = required(&self.success_value, "the cut point")?;
                        format!("CUTPOINT({})", number(cut))
                    }
                    _ => keyword.to_string(),
                }))
            }
        }
    }

    /// VARIANCE, NULLVALUE and ALTERGVALUE of a mean; the last two only for a Bayes factor.
    fn mean_test(&self, words: &mut Vec<String>) -> Result<(), String> {
        let above = |v: f64| v > 0.0;
        if let Some(v) = setting(
            &self.known_variance,
            "the known variance",
            "above 0",
            f64::NAN,
            above,
        )? {
            words.push(format!("VARIANCE={}", number(v)));
        }
        if self.factor() {
            if let Some(v) = setting(&self.null, "the test value", "a number", 0.0, |_| true)? {
                words.push(format!("NULLVALUE={}", number(v)));
            }
            if let Some(v) = setting(&self.g, "the g value", "above 0", f64::NAN, above)? {
                words.push(format!("ALTERGVALUE={}", number(v)));
            }
        }
        Ok(())
    }

    /// The PRIOR words of a normal mean, only for a posterior.
    fn normal_prior(&self) -> Result<Vec<String>, String> {
        let mut words = Vec::new();
        if !self.posterior() {
            return Ok(words);
        }
        let known = !self.known_variance.trim().is_empty();
        let count = variance_parameters(self.variance_prior);
        // a known variance makes the variance prior idle
        if !known && self.variance_prior != 0 {
            let keyword = BAYES_VARIANCE_PRIORS[self.variance_prior].0;
            words.push(match count {
                2 => format!(
                    "{keyword}({} {})",
                    number(positive(&self.prior_a, "the first prior parameter")?),
                    number(positive(&self.prior_b, "the second prior parameter")?)
                ),
                1 => format!(
                    "{keyword}({})",
                    number(positive(&self.prior_a, "the prior degrees of freedom")?)
                ),
                _ => keyword.to_string(),
            });
        }
        if self.mean_prior_used() && self.normal_mean {
            let mean = required(&self.prior_mean, "the prior mean")?;
            words.push(match known {
                true => format!(
                    "MEANDIST=NORMAL({} {})",
                    number(mean),
                    number(positive(
                        &self.prior_mean_variance,
                        "the prior variance of the mean"
                    )?)
                ),
                false => format!("MEANDIST=NORMAL({})", number(mean)),
            });
        }
        if self.kappa_used()
            && let Some(k) = setting(&self.kappa, "kappa", "above 0", 1.0, |v| v > 0.0)?
        {
            words.push(format!("KAPPA={}", number(k)));
        }
        Ok(words)
    }

    fn related(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        if self.pairs.is_empty() {
            return Err("add at least one pair of variables".into());
        }
        numeric(data, self.pairs.iter().flat_map(|(a, b)| [a, b]))?;
        let mut words = Vec::new();
        self.mean_test(&mut words)?;
        let test: String = words.iter().map(|w| format!(" {w}")).collect();
        let pairs: Vec<String> = self
            .pairs
            .iter()
            .map(|(a, b)| format!("/PAIR VARIABLES={a} {b}{test}"))
            .collect();
        let mut text = format!("BAYES RELATED {}", pairs.join("\n  "));
        if !analysis.is_empty() {
            text.push_str(&format!("\n  /INFERENCE{analysis}"));
        }
        let prior = self.normal_prior()?;
        if !prior.is_empty() {
            text.push_str(&format!("\n  /PRIOR {}", prior.join(" ")));
        }
        Ok(text)
    }

    fn independent(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        let list = names(&self.variables, "the test variables")?;
        numeric(data, &self.variables)?;
        if self.group.trim().is_empty() {
            return Err("choose the grouping variable".into());
        }
        let string = match data.variable(&self.group) {
            Ok(v) => v.is_string(),
            Err(_) => return Err(format!("{} is no longer a variable", self.group)),
        };
        if self.variables.contains(&self.group) {
            return Err(format!(
                "{} cannot both be tested and form the groups",
                self.group
            ));
        }
        let select = match self.select {
            0 => {
                if self.level1.trim().is_empty() || self.level2.trim().is_empty() {
                    return Err("give the values of both groups".into());
                }
                let (a, b) = (value(&self.level1, string)?, value(&self.level2, string)?);
                if a == b {
                    return Err("the two groups need different values".into());
                }
                format!("LEVEL({a} {b})")
            }
            _ if string => {
                return Err(format!(
                    "{} is a string; give the values of the two groups",
                    self.group
                ));
            }
            SELECT_CUTPOINT => format!(
                "CUTPOINT({})",
                number(required(&self.cutpoint, "the cut point")?)
            ),
            _ => "MIDPOINT".into(),
        };
        let mut text = format!(
            "BAYES INDEPENDENT /INFERENCE VARIABLES={list} GROUP={} SELECT={select}{analysis}",
            self.group
        );
        let mut prior = Vec::new();
        if self.posterior() {
            if self.known_variances {
                text.push_str(&format!(
                    " DATAVAR={} {}",
                    number(positive(&self.variance1, "the variance of group 1")?),
                    number(positive(&self.variance2, "the variance of group 2")?)
                ));
                // the group mean priors are used only with known variances
                if self.normal_mean {
                    let mut v = Vec::new();
                    for (i, what) in ["the prior mean of group 1", "the prior mean of group 2"]
                        .into_iter()
                        .enumerate()
                    {
                        v.push(number(required(&self.group_means[i], what)?));
                    }
                    for (i, what) in [
                        "the prior variance of group 1",
                        "the prior variance of group 2",
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        v.push(number(positive(&self.group_means[i + 2], what)?));
                    }
                    prior.push(format!("MEANDIST=NORMAL({})", v.join(" ")));
                }
            } else if self.equal_variances {
                prior.push("EQUALDATAVAR=TRUE".into());
                match self.variance_prior {
                    GROUP_INVCHISQ => prior.push(format!(
                        "VARDIST=INVCHISQ({} {})",
                        number(positive(&self.prior_a, "the prior degrees of freedom")?),
                        number(positive(&self.prior_b, "the prior scale")?)
                    )),
                    0 => {}
                    i => prior.push(format!("VARDIST={}", BAYES_GROUP_PRIORS[i].0)),
                }
            }
        }
        if self.factor() {
            let computation = match self.factor {
                GONEN => {
                    let mean = required(&self.gonen_mean, "the prior mean of the effect size")?;
                    let variance = positive(
                        &self.gonen_variance,
                        "the prior variance of the effect size",
                    )?;
                    Some(format!("GONEN({} {})", number(mean), number(variance)))
                }
                HYPER => {
                    let a = setting(
                        &self.hyper,
                        "the hyper-prior value",
                        "above -1 and at most -0.5",
                        -0.75,
                        |v| v > -1.0 && v <= -0.5,
                    )?;
                    Some(match a {
                        Some(a) => format!("HYPER({})", number(a)),
                        None => "HYPER".into(),
                    })
                }
                _ => setting(&self.rouder, "the Rouder scale", "above 0", 1.0, |v| {
                    v > 0.0
                })?
                .map(|s| format!("ROUDER({})", number(s))),
            };
            if let Some(c) = computation {
                text.push_str(&format!("\n  /ESTBF COMPUTATION={c}"));
            }
        }
        if !prior.is_empty() {
            text.push_str(&format!("\n  /PRIOR {}", prior.join(" ")));
        }
        Ok(text)
    }

    fn correlation(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        numeric(data, &self.variables)?;
        let mut text = format!(
            "BAYES CORRELATION /INFERENCE VARIABLES={}{analysis}",
            self.variables.join(" ")
        );
        if self.posterior()
            && let Some(c) = setting(&self.cvalue, "the prior power", "a number", 0.0, |_| true)?
        {
            text.push_str(&format!(" CVALUE={}", number(c)));
        }
        if self.factor() && self.fraction {
            let b = required(&self.fraction_value, "the fraction")?;
            if !(b > 0.0 && b < 1.0) {
                return Err("the fraction must lie above 0 and below 1".into());
            }
            text.push_str(&format!("\n  /ESTBF TYPE=FRACTION({})", number(b)));
            let between = |v: f64| v > -1.0 && v < 1.0;
            if let Some(v) = setting(
                &self.fraction_null,
                "the null correlation",
                "above -1 and below 1",
                0.0,
                between,
            )? {
                text.push_str(&format!(" NULLVALUE={}", number(v)));
            }
        }
        Ok(text)
    }

    fn anova(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        let (dependent, factor) = (self.dependent.trim(), self.group.trim());
        if dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        if factor.is_empty() {
            return Err("choose the factor".into());
        }
        if dependent == factor {
            return Err(format!(
                "{factor} cannot both be the dependent and form the groups"
            ));
        }
        numeric(data, [&self.dependent])?;
        let string = match data.variable(factor) {
            Ok(v) => v.is_string(),
            Err(_) => return Err(format!("{factor} is no longer a variable")),
        };
        let mut text = format!("BAYES ANOVA {dependent} BY {factor}");
        self.linear_model(data, analysis, &mut text)?;
        if self.linear_prior_used() {
            text.push_str(&self.linear_prior()?);
        }
        let mut plot = Vec::new();
        if !self.plot_values.trim().is_empty() {
            plot.push(format!("MEANS={}", values(&self.plot_values, string)?));
        }
        if self.plot_error {
            plot.push("ERRORVAR=TRUE".into());
        }
        if !plot.is_empty() {
            text.push_str(&format!("\n  /PLOT {}", plot.join(" ")));
        }
        Ok(text)
    }

    fn regression(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        if self.factors.is_empty() && self.covariates.is_empty() {
            return Err("add at least one factor or covariate".into());
        }
        if self.model().any(|n| n == dependent) {
            return Err(format!(
                "{dependent} cannot both be the dependent and in the model"
            ));
        }
        if let Some(n) = self.factors.iter().find(|n| self.covariates.contains(n)) {
            return Err(format!("{n} cannot be both a factor and a covariate"));
        }
        numeric(
            data,
            std::iter::once(&self.dependent).chain(&self.covariates),
        )?;
        let mut text = format!("BAYES REGRESSION {dependent}");
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        self.linear_model(data, analysis, &mut text)?;
        text.push_str(&self.linear_prior()?);
        if self.predicting() {
            let mut regressors = Vec::new();
            for n in self.model() {
                let typed = self.predict.get(n).map_or("", |v| v.trim());
                if typed.is_empty() {
                    return Err(format!("give a prediction value for {n}, or for none"));
                }
                // a factor takes one of its categories, a covariate a number
                let string =
                    self.factors.contains(n) && data.variable(n).is_ok_and(|v| v.is_string());
                regressors.push(value(typed, string)?);
            }
            text.push_str(&format!("\n  /PREDICT REGRESSORS={}", regressors.join(" ")));
        }
        let save = saved(data, &BAYES_SAVE, &self.save, &self.save_names)?;
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        if let Some(n) = self.plotted.iter().find(|n| !self.model().any(|m| m == *n)) {
            return Err(format!("{n} is not in the model; remove it from the plots"));
        }
        let pick = |list: &[String]| -> Vec<String> {
            let chosen = self.plotted.iter().filter(|n| list.contains(n));
            chosen.cloned().collect()
        };
        let (covariates, factors) = (pick(&self.covariates), pick(&self.factors));
        let mut plot = Vec::new();
        if !covariates.is_empty() {
            plot.push(format!("COVARIATES={}", covariates.join(" ")));
        }
        if !factors.is_empty() {
            plot.push(format!("FACTORS={}", factors.join(" ")));
            if let Some(n) = whole(&self.max_category, 1, "the categories plotted")?
                && n != 2
            {
                plot.push(format!("MAX_CATEGORY={n}"));
            }
        }
        for (on, keyword) in [
            (self.plot_intercept, "INTERCEPT"),
            (self.plot_error, "ERRORVAR"),
            (self.plot_prediction && self.predicting(), "BAYESPRED"),
        ] {
            if on {
                plot.push(format!("{keyword}=TRUE"));
            }
        }
        if !plot.is_empty() {
            text.push_str(&format!("\n  /PLOT {}", plot.join(" ")));
        }
        for (i, test) in self.ftests.iter().enumerate() {
            let what = format!("F test {}", i + 1);
            if test.variables.is_empty() && !test.intercept {
                return Err(format!("{what} needs variables or the intercept"));
            }
            if let Some(n) = test
                .variables
                .iter()
                .find(|n| !self.model().any(|m| m == *n))
            {
                return Err(format!("{what} names {n}, which is not in the model"));
            }
            let mut words = Vec::new();
            if !test.label.trim().is_empty() {
                words.push(format!("LABEL={}", quote(test.label.trim())));
            }
            if !test.variables.is_empty() {
                words.push(format!("VARIABLES={}", test.variables.join(" ")));
            }
            if test.intercept {
                words.push("INTERCEPT=TRUE".into());
            }
            let tested = numbers(&test.values, &format!("each test value of {what}"))?;
            if !tested.is_empty() {
                let tested: Vec<String> = tested.into_iter().map(number).collect();
                words.push(format!("TESTVALUES={}", tested.join(" ")));
            }
            text.push_str(&format!("\n  /FTEST {}", words.join(" ")));
        }
        Ok(text)
    }

    fn repeated(&self, data: &Dataset, analysis: &str) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two repeated variables".into());
        }
        numeric(data, &self.variables)?;
        let mut text = format!("BAYES REPEATED {}", self.variables.join(" "));
        self.weight_and_analysis(data, analysis, &mut text)?;
        if self.draws_used() {
            text.push_str("\n  /ESTBF COMPUTATION=ROUDER");
        }
        if let Some(n) = self.plotted.iter().find(|n| !self.variables.contains(n)) {
            return Err(format!(
                "{n} is not a repeated variable; remove it from the plots"
            ));
        }
        if !self.plotted.is_empty() {
            text.push_str(&format!("\n  /PLOT MEANS={}", self.plotted.join(" ")));
        }
        Ok(text)
    }

    /// The gamma shapes of the cells are used: by a Poisson model or a conjugate multinomial one.
    pub fn shapes_used(&self) -> bool {
        self.count_model == COUNT_POISSON
            || (self.count_model == COUNT_MULTINOMIAL && self.factor == 0)
    }

    fn loglinear(&self, analysis: &str) -> Result<String, String> {
        let (row, column) = (self.row.trim(), self.column.trim());
        if row.is_empty() || column.is_empty() {
            return Err("choose the row and the column variable".into());
        }
        if row.eq_ignore_ascii_case(column) {
            return Err("the rows and the columns need two different variables".into());
        }
        let mut text = format!("BAYES LOGLINEAR /TABLE {row} BY {column}");
        if !analysis.is_empty() {
            text.push_str(&format!("\n  /INFERENCE{analysis}"));
        }
        let mut words = Vec::new();
        if self.factor() {
            let model = self.count_model;
            if model != COUNT_MULTINOMIAL {
                words.push(format!("MODEL={}", BAYES_COUNT_MODELS[model].0));
            }
            // a Poisson model fixes no total
            if model == COUNT_NONPARAMETRIC && self.margin == 0 {
                return Err("the nonparametric model needs fixed row or column totals".into());
            }
            if model != COUNT_POISSON && self.margin != 0 {
                words.push(format!("FIX_MARGIN={}", BAYES_MARGINS[self.margin].0));
            }
            if model == COUNT_MULTINOMIAL && self.factor != 0 {
                words.push(format!("PRIOR_TYPE={}", BAYES_TABLE_PRIORS[self.factor].0));
            }
            // the gamma shapes belong to the conjugate priors only
            if self.shapes_used() && !self.shapes.trim().is_empty() {
                let mut rows = Vec::new();
                let given = self
                    .shapes
                    .split(';')
                    .map(str::trim)
                    .filter(|r| !r.is_empty());
                for (i, row) in given.enumerate() {
                    let what = format!("each shape of row {}", i + 1);
                    let row = numbers(row, &what)?;
                    if row.is_empty() || row.iter().any(|&v| v <= 0.0) {
                        return Err(format!("{what} must be above 0"));
                    }
                    rows.push(row.into_iter().map(number).collect::<Vec<_>>().join(" "));
                }
                words.push(format!("SHAPEPARAMS={}", rows.join("; ")));
            }
            if model == COUNT_POISSON
                && let Some(b) =
                    setting(&self.scale, "the scale", "above 0", f64::NAN, |v| v > 0.0)?
            {
                words.push(format!("SCALEPARAM={}", number(b)));
            }
            if model == COUNT_NONPARAMETRIC && !self.lambdas.trim().is_empty() {
                let lambdas = numbers(&self.lambdas, "each Dirichlet parameter")?;
                if lambdas.iter().any(|&v| v <= 0.0) {
                    return Err("each Dirichlet parameter must be above 0".into());
                }
                let lambdas: Vec<String> = lambdas.into_iter().map(number).collect();
                words.push(format!("LAMBDAS={}", lambdas.join(" ")));
            }
        }
        if !words.is_empty() {
            text.push_str(&format!("\n  /ESTBF {}", words.join(" ")));
        }
        let mut print = Vec::new();
        if self.suppress_table {
            print.push("SUPPRESS_TABLE=TRUE".to_string());
        } else if self.cells != [true, false, false, false, false] {
            let cells: Vec<&str> = BAYES_CELLS
                .iter()
                .zip(self.cells)
                .filter(|(_, on)| *on)
                .map(|((keyword, _), _)| *keyword)
                .collect();
            if cells.is_empty() {
                return Err("choose what the crosstab shows, or hide it".into());
            }
            print.push(format!("CELLS={}", cells.join(" ")));
        }
        match (self.chisq, self.lrt) {
            (true, false) => {}
            (true, true) => print.push("STATISTICS=CHISQ LRT".into()),
            (false, true) => print.push("STATISTICS=LRT".into()),
            (false, false) => {
                return Err("choose the chi-square, the likelihood ratio or both".into());
            }
        }
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT {}", print.join(" ")));
        }
        Ok(text)
    }

    /// /DESIGN REGWGT and /INFERENCE ANALYSIS of the linear models and repeated measures.
    fn weight_and_analysis(
        &self,
        data: &Dataset,
        analysis: &str,
        text: &mut String,
    ) -> Result<(), String> {
        let weight = self.weight.trim();
        if !weight.is_empty() {
            numeric(data, [&self.weight])?;
            text.push_str(&format!("\n  /DESIGN REGWGT={weight}"));
        }
        if !analysis.is_empty() {
            text.push_str(&format!("\n  /INFERENCE{analysis}"));
        }
        Ok(())
    }

    /// The weight, the analysis and the Bayes factor of a one-way ANOVA or a regression.
    fn linear_model(
        &self,
        data: &Dataset,
        analysis: &str,
        text: &mut String,
    ) -> Result<(), String> {
        self.weight_and_analysis(data, analysis, text)?;
        if !self.factor() {
            return Ok(());
        }
        let computation = match self.factor {
            G_ZELLNER => Some(format!(
                "ZELLNER({})",
                number(positive(&self.g, "g of the Zellner prior")?)
            )),
            G_HYPER => Some(
                match setting(&self.hyper, "the hyper-g value", "above 2", 3.0, |v| {
                    v > 2.0
                })? {
                    Some(a) => format!("HYPER({})", number(a)),
                    None => "HYPER".into(),
                },
            ),
            G_ROUDER => Some(
                match setting(&self.rouder, "the Rouder scale", "above 0", 1.0, |v| {
                    v > 0.0
                })? {
                    Some(s) => format!("ROUDER({})", number(s)),
                    None => "ROUDER".into(),
                },
            ),
            _ => None,
        };
        let mut words: Vec<String> = computation
            .into_iter()
            .map(|c| format!("COMPUTATION={c}"))
            .collect();
        if self.kind == BayesKind::Regression && self.compare_full {
            words.push("COMPARE=FULL".into());
        }
        if !words.is_empty() {
            text.push_str(&format!("\n  /ESTBF {}", words.join(" ")));
        }
        Ok(())
    }

    /// /PRIOR of a linear model; empty for the reference prior.
    fn linear_prior(&self) -> Result<String, String> {
        if self.variance_prior == 0 {
            return Ok(String::new());
        }
        let shape = positive(&self.prior_a, "the shape of the variance prior")?;
        let scale = positive(&self.prior_b, "the scale of the variance prior")?;
        let means = numbers(&self.prior_mean, "each prior mean")?;
        if means.is_empty() {
            return Err("give the prior means".into());
        }
        let means: Vec<String> = means.into_iter().map(number).collect();
        let mut text = format!(
            "\n  /PRIOR TYPE=CONJUGATE SHAPEPARAM={} SCALEPARAM={} MEANVECTOR={}",
            number(shape),
            number(scale),
            means.join(" ")
        );
        let rows: Vec<&str> = self
            .vmatrix
            .split(';')
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .collect();
        if !rows.is_empty() {
            if rows.len() != means.len() {
                return Err("the prior matrix needs one row per prior mean".into());
            }
            let mut matrix = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let what = format!("row {} of the prior matrix", i + 1);
                let row = numbers(row, &format!("each value of {what}"))?;
                if row.len() != i + 1 || row[i] <= 0.0 {
                    return Err(format!("{what} needs {} values, the last above 0", i + 1));
                }
                let row: Vec<String> = row.into_iter().map(number).collect();
                matrix.push(format!("{};", row.join(" ")));
            }
            text.push_str(&format!(" VMATRIX={}", matrix.join(" ")));
        }
        Ok(text)
    }

    /// /CRITERIA with the settings that differ from the defaults.
    fn criteria(&self) -> Result<String, String> {
        let mut words = Vec::new();
        let level = ci_level(&self.ci_level)?;
        if level != 95.0 {
            words.push(format!("CILEVEL={}", number(level)));
        }
        if self.numerical_offered() {
            let default = match self.kind {
                BayesKind::Correlation => 0.0001,
                _ => 0.000001,
            };
            let tol = setting(
                &self.tolerance,
                "the tolerance",
                "above 0 and at most 0.001",
                default,
                |v| v > 0.0 && v <= 0.001,
            )?;
            if let Some(t) = tol {
                words.push(format!("TOL={}", number(t)));
            }
            if let Some(n) = whole(&self.max_iterations, 1, "the most iterations")?
                && n != 2000
            {
                words.push(format!("MAXITER={n}"));
            }
        }
        if self.draws_used() {
            // only a normal mean chooses its method; the others draw when they need to
            if self.monte_carlo_offered() {
                words.push("METHOD=MCA".into());
            }
            if let Some(seed) = whole(&self.seed, 1, "the seed")? {
                if seed > 2_147_483_647 {
                    return Err("the seed must be at most 2147483647".into());
                }
                words.push(format!("SEED={seed}"));
            }
            let samples = whole(&self.mc_samples, 1000, "the Monte Carlo draws")?;
            let default = match self.kind {
                BayesKind::Repeated => 30_000,
                BayesKind::Normal
                | BayesKind::Binomial
                | BayesKind::Poisson
                | BayesKind::Related
                | BayesKind::Independent
                | BayesKind::Correlation
                | BayesKind::Anova
                | BayesKind::Regression
                | BayesKind::Loglinear => 10_000,
            };
            let keyword = match self.kind {
                BayesKind::Loglinear => "POSTSAMPLES",
                BayesKind::Normal
                | BayesKind::Binomial
                | BayesKind::Poisson
                | BayesKind::Related
                | BayesKind::Independent
                | BayesKind::Correlation
                | BayesKind::Anova
                | BayesKind::Regression
                | BayesKind::Repeated => "MCSAMPLES",
            };
            match samples {
                Some(n) if n > 1_000_000 => {
                    return Err("the Monte Carlo draws must be at most 1000000".into());
                }
                Some(n) if n != default => words.push(format!("{keyword}={n}")),
                _ => {}
            }
        }
        if self.kind == BayesKind::Loglinear && self.descending {
            words.push("ORDER=DESCENDING".into());
        }
        Ok(match words.is_empty() {
            true => String::new(),
            false => format!("\n  /CRITERIA {}", words.join(" ")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn bayes_checks_what_the_command_would_reject() {
        let data = Dataset::new(0);
        let normal = Bayesian {
            variables: vec![s("x")],
            ..Bayesian::new(BayesKind::Normal)
        };
        let error = |d: Bayesian| d.syntax(&data).unwrap_err();
        assert_eq!(
            error(Bayesian::new(BayesKind::Normal)),
            "add at least one variable to the test variables"
        );
        assert!(
            error(Bayesian {
                ci_level: s("100"),
                ..normal.clone()
            })
            .contains("percentage")
        );
        assert!(
            error(Bayesian {
                tolerance: s("0.01"),
                ..normal.clone()
            })
            .contains("at most 0.001")
        );
        // a known variance needs the prior variance of the mean
        let known = Bayesian {
            known_variance: s("4"),
            normal_mean: true,
            prior_mean: s("10"),
            ..normal.clone()
        };
        assert_eq!(error(known.clone()), "give the prior variance of the mean");
        // the prior is idle without a posterior, and the mean prior with a flat variance prior
        let idle = Bayesian {
            analysis: 1,
            ..known
        };
        assert_eq!(
            idle.syntax(&data).unwrap(),
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x ANALYSIS=BAYESFACTOR\n  /DATA VARIABLES=x VARIANCE=4."
        );
        let flat = Bayesian {
            normal_mean: true,
            ..normal.clone()
        };
        assert!(flat.syntax(&data).unwrap().ends_with("VARIABLES=x."));
        let gamma = Bayesian {
            variance_prior: 5,
            prior_a: s("2"),
            ..normal.clone()
        };
        assert_eq!(error(gamma), "give the second prior parameter");
        let binomial = Bayesian {
            variables: vec![s("x")],
            conjugate: s("0 1"),
            ..Bayesian::new(BayesKind::Binomial)
        };
        assert!(error(binomial).contains("both above 0, or both 0"));
        let independent = Bayesian {
            variables: vec![s("x")],
            group: s("x"),
            ..Bayesian::new(BayesKind::Independent)
        };
        assert!(error(independent).contains("no longer a variable"));
        let correlation = Bayesian {
            variables: vec![s("x")],
            ..Bayesian::new(BayesKind::Correlation)
        };
        assert_eq!(error(correlation), "add at least two variables");
    }

    #[test]
    fn bayes_models_check_what_the_command_would_reject() {
        let data = Dataset::new(0);
        let error = |d: Bayesian| d.syntax(&data).unwrap_err();
        let anova = Bayesian {
            dependent: s("y"),
            group: s("y"),
            ..Bayesian::new(BayesKind::Anova)
        };
        assert!(error(anova).contains("cannot both be the dependent"));
        let regression = Bayesian {
            dependent: s("y"),
            covariates: vec![s("x")],
            ..Bayesian::new(BayesKind::Regression)
        };
        assert_eq!(
            error(Bayesian::new(BayesKind::Regression)),
            "choose the dependent variable"
        );
        // a conjugate prior needs its parameters, and the matrix one row per mean
        let conjugate = Bayesian {
            variance_prior: 1,
            prior_a: s("2"),
            prior_b: s("3"),
            prior_mean: s("0 1"),
            vmatrix: s("1"),
            ..regression.clone()
        };
        assert_eq!(
            error(conjugate.clone()),
            "the prior matrix needs one row per prior mean"
        );
        let triangle = Bayesian {
            vmatrix: s("1; 2 0"),
            ..conjugate
        };
        assert_eq!(
            error(triangle),
            "row 2 of the prior matrix needs 2 values, the last above 0"
        );
        let zellner = Bayesian {
            analysis: 1,
            factor: G_ZELLNER,
            ..regression.clone()
        };
        assert_eq!(error(zellner.clone()), "give g of the Zellner prior");
        // the Bayes factor settings are idle without a Bayes factor
        let idle = Bayesian {
            analysis: 0,
            ..zellner
        };
        assert_eq!(idle.syntax(&data).unwrap(), "BAYES REGRESSION y WITH x.");
        let partial = Bayesian {
            factors: vec![s("g")],
            predict: [(s("x"), s("4"))].into_iter().collect(),
            ..regression.clone()
        };
        assert_eq!(error(partial), "give a prediction value for g, or for none");
        let test = Bayesian {
            ftests: vec![FTestRow::default()],
            ..regression.clone()
        };
        assert_eq!(error(test), "F test 1 needs variables or the intercept");
        let plot = Bayesian {
            plotted: vec![s("z")],
            ..regression
        };
        assert!(error(plot).contains("not in the model"));
        let repeated = Bayesian {
            variables: vec![s("a")],
            ..Bayesian::new(BayesKind::Repeated)
        };
        assert_eq!(error(repeated), "add at least two repeated variables");
        let table = Bayesian {
            row: s("a"),
            column: s("b"),
            analysis: 1,
            count_model: COUNT_NONPARAMETRIC,
            ..Bayesian::new(BayesKind::Loglinear)
        };
        assert_eq!(
            error(table.clone()),
            "the nonparametric model needs fixed row or column totals"
        );
        let bare = Bayesian {
            chisq: false,
            cells: [false; 5],
            ..Bayesian::new(BayesKind::Loglinear)
        };
        assert_eq!(
            error(Bayesian {
                row: s("a"),
                column: s("b"),
                ..bare.clone()
            }),
            "choose what the crosstab shows, or hide it"
        );
        // without a Bayes factor the model is idle; the shapes apply to conjugate priors only
        let idle = Bayesian {
            analysis: 0,
            shapes: s("0"),
            ..table
        };
        assert_eq!(
            Bayesian {
                analysis: 1,
                count_model: COUNT_POISSON,
                shapes: s(""),
                lambdas: s("x"),
                ..idle.clone()
            }
            .syntax(&data)
            .unwrap(),
            "BAYES LOGLINEAR /TABLE a BY b\n  /INFERENCE ANALYSIS=BAYESFACTOR\n  /ESTBF MODEL=POISSON."
        );
        assert_eq!(
            idle.syntax(&data).unwrap(),
            "BAYES LOGLINEAR /TABLE a BY b."
        );
    }
}
