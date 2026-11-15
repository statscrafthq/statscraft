//! The Power Analysis dialogs: the power of a planned test for given sample sizes, or the
//! sample sizes that reach given powers (POWER MEANS and POWER PROPORTIONS, each ONESAMPLE,
//! INDEPENDENT and RELATED, POWER ONEWAY ANOVA, the tests of one correlation and POWER UNIVARIATE
//! LINEAR). They take typed numbers and no variables, so they need no data.
//! One struct serves every kind; a new POWER command is a new kind with its fields and charts.
//! Options left as they are add nothing to the syntax, so the command runs with its own
//! defaults.

use crate::dialogs::whole;
use crate::loglinear_dialog::optional;
use crate::survival_dialog::numbers;
use crate::syntax_text::number;
pub use statscraft_syntax::PowerKind;

/// The submenus of Analyze > Power Analysis and their items.
pub const POWER_MENU: [(&str, &[(&str, PowerKind)]); 4] = [
    (
        "Means",
        &[
            ("One-Sample T Test...", PowerKind::MeansOneSample),
            ("Independent-Samples T Test...", PowerKind::MeansIndependent),
            ("Paired-Samples T Test...", PowerKind::MeansRelated),
            ("One-Way ANOVA...", PowerKind::OnewayAnova),
        ],
    ),
    (
        "Proportions",
        &[
            (
                "One-Sample Binomial Test...",
                PowerKind::ProportionsOneSample,
            ),
            (
                "Independent-Samples Binomial Test...",
                PowerKind::ProportionsIndependent,
            ),
            (
                "Related-Samples Binomial Test...",
                PowerKind::ProportionsRelated,
            ),
        ],
    ),
    (
        "Correlations",
        &[
            ("Pearson Product-Moment...", PowerKind::PearsonOneSample),
            ("Spearman Rank-Order...", PowerKind::SpearmanOneSample),
            ("Partial Correlation...", PowerKind::PartialCorr),
        ],
    ),
    (
        "Regression",
        &[("Univariate Linear...", PowerKind::UnivariateLinear)],
    ),
];

/// The tests of two independent proportions (METHOD), the default first.
pub const POWER_METHODS: [(&str, &str); 4] = [
    ("CHISQ", "Chi-square test"),
    ("T", "t-test"),
    ("LRT", "Likelihood ratio test"),
    ("FISHER", "Fisher's exact test"),
];

/// How the pairwise comparisons of the group means share the significance level (ADJUST), the
/// default first.
pub const POWER_ADJUSTS: [(&str, &str); 3] = [
    ("BONFERRONI", "Bonferroni"),
    ("SIDAK", "Sidak"),
    ("LSD", "None (LSD)"),
];

/// The variances of Fisher's z of a rank correlation (VARIANCE), the default first.
pub const POWER_VARIANCES: [(&str, &str); 3] = [
    ("BW", "Bonett and Wright"),
    ("FHP", "Fieller, Hartley and Pearson"),
    ("CC", "Caruso and Cliff"),
];

/// A chart of PLOT: its keywords, its label, and whether it takes a range (the line charts that
/// allow one).
pub type PowerChart = (&'static str, &'static str, bool);

/// The most charts one kind offers.
pub const MOST_CHARTS: usize = 7;

/// The charts each kind offers, the line charts first.
pub fn power_charts(kind: PowerKind) -> &'static [PowerChart] {
    match kind {
        PowerKind::MeansOneSample => &[
            ("N", "Power by sample size", true),
            ("ES", "Power by effect size", true),
            (
                "N_XAXIS ES_YAXIS",
                "Power by sample size and effect size",
                false,
            ),
        ],
        PowerKind::MeansIndependent => &[
            ("NRATIO", "Power by ratio of group sizes", true),
            (
                "ES",
                "Power by effect size (mean difference with two SDs)",
                true,
            ),
            (
                "NRATIO_XAXIS ES_YAXIS",
                "Power by size ratio and effect size",
                false,
            ),
        ],
        PowerKind::MeansRelated => &[
            ("NPAIRS", "Power by number of pairs", true),
            ("ES", "Power by effect size", true),
            (
                "NPAIRS_XAXIS ES_YAXIS",
                "Power by number of pairs and effect size",
                false,
            ),
        ],
        PowerKind::ProportionsOneSample => &[
            ("N", "Power by sample size", true),
            ("NULL", "Power by null proportion", false),
            ("ALTERNATIVE", "Power by alternative proportion", false),
            ("DIFF", "Power by alternative minus null", false),
            (
                "N_XAXIS DIFF_YAXIS",
                "Power by sample size and difference",
                false,
            ),
            (
                "NULL_XAXIS ALTERNATIVE_YAXIS",
                "Power by null and alternative proportions",
                false,
            ),
        ],
        PowerKind::ProportionsIndependent => &[
            ("NRATIO", "Power by ratio of group sizes", true),
            ("RISK_DIFF", "Power by risk difference", false),
            ("RISK_RATIO", "Power by risk ratio", true),
            ("ODDS_RATIO", "Power by odds ratio", true),
            ("N1_XAXIS N2_YAXIS", "Power by the two group sizes", false),
            ("P1_XAXIS P2_YAXIS", "Power by the two proportions", false),
        ],
        PowerKind::ProportionsRelated => &[
            ("NPAIRS", "Power by number of pairs", true),
            ("RISK_DIFF", "Power by risk difference", false),
            ("RISK_RATIO", "Power by risk ratio", true),
            ("ODDS_RATIO", "Power by odds ratio", true),
            ("CORRELATION", "Power by correlation (success rates)", false),
            ("DISCORDANT", "Power by the two discordant shares", false),
            ("MARGINAL", "Power by the two success rates", false),
        ],
        PowerKind::OnewayAnova => &[
            ("TOTAL_N", "Power by total sample size", true),
            ("POOLED_SD", "Power by pooled standard deviation", true),
            (
                "TOTAL_N_XAXIS POOLED_SD_YAXIS",
                "Power by total size and standard deviation",
                false,
            ),
        ],
        PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => &[
            ("N", "Power by sample size", true),
            ("NULL", "Power by null correlation", false),
            ("ALTERNATIVE", "Power by alternative correlation", false),
            ("DIFF", "Power by alternative minus null", false),
            (
                "N_XAXIS DIFF_YAXIS",
                "Power by sample size and difference",
                false,
            ),
            (
                "NULL_XAXIS ALTERNATIVE_YAXIS",
                "Power by null and alternative correlations",
                false,
            ),
        ],
        PowerKind::PartialCorr => &[
            ("N", "Power by sample size", true),
            (
                "PARTIALOUT",
                "Power by number of variables held fixed",
                false,
            ),
            ("NULL", "Power by null correlation", false),
            ("ALTERNATIVE", "Power by alternative correlation", false),
            ("DIFF", "Power by alternative minus null", false),
            (
                "N_XAXIS DIFF_YAXIS",
                "Power by sample size and difference",
                false,
            ),
            (
                "NULL_XAXIS ALTERNATIVE_YAXIS",
                "Power by null and alternative correlations",
                false,
            ),
        ],
        PowerKind::UnivariateLinear => &[
            ("N", "Power by sample size", true),
            ("PARTIAL_CORR", "Power by partial correlation", false),
            (
                "N_XAXIS PARTIAL_CORR_YAXIS",
                "Power by sample size and partial correlation",
                false,
            ),
        ],
    }
}

/// A chart drawn only from marginal success rates.
pub fn needs_marginal(kind: PowerKind, keywords: &str) -> bool {
    kind == PowerKind::ProportionsRelated && matches!(keywords, "CORRELATION" | "MARGINAL")
}

/// Every POWER command: the fields of every kind; each kind uses its own.
#[derive(Clone, PartialEq, Debug)]
pub struct PowerAnalysis {
    pub kind: PowerKind,
    /// The sample sizes that reach `powers`, instead of the power of the sizes given.
    pub find_size: bool,
    /// TEST=DIRECTIONAL: a one-sided test.
    pub directional: bool,
    pub significance: String,
    /// N or NPAIRS; with two groups the size of the first; GROUP_SIZES of ONEWAY ANOVA, a typed
    /// list.
    pub n1: String,
    pub n2: String,
    /// The powers to reach, a typed list.
    pub powers: String,
    /// NRATIO: n2 / n1 of the sizes to find.
    pub ratio: String,
    /// Two standard deviations (one per group or measure) instead of one.
    pub two_sds: bool,
    /// SD, or POOLED_SD of ONEWAY ANOVA.
    pub sd1: String,
    pub sd2: String,
    /// Two means (one per group or measure) instead of their difference.
    pub two_means: bool,
    /// MEAN, or GROUP_MEANS of ONEWAY ANOVA, a typed list.
    pub mean1: String,
    pub mean2: String,
    /// NULL: the mean, proportion or correlation under the null hypothesis.
    pub null: String,
    pub alternative: String,
    /// RHO: the correlation of the two measures of a pair, with two SDs.
    pub rho: String,
    /// PROPORTIONS of the two groups, the two values of VALUES, or FULL_MODEL and NESTED_MODEL.
    pub p1: String,
    pub p2: String,
    /// VALUES=COUNTS, only with a given number of pairs.
    pub counts: bool,
    /// MARGINAL=TRUE: the values are the success rates of the two occasions.
    pub marginal: bool,
    pub correlation: String,
    /// Index into `POWER_METHODS`.
    pub method: usize,
    /// ESTIMATE=BINOMIAL.
    pub binomial: bool,
    pub continuity: bool,
    pub pooled: bool,
    /// TIMER in minutes, for the exact sample size search.
    pub timer: String,
    /// GROUP_WEIGHTS of the group sizes to find, a typed list; empty for equal groups.
    pub weights: String,
    /// CONTRAST COEFFICIENTS, a typed list; empty for no contrast test.
    pub coefficients: String,
    pub pairwise: bool,
    /// Index into `POWER_ADJUSTS`.
    pub adjust: usize,
    /// PARTIALOUT: the number of variables held fixed.
    pub partialout: String,
    pub adjust_bias: bool,
    /// Index into `POWER_VARIANCES`.
    pub variance: usize,
    /// MODEL=RANDOM.
    pub random: bool,
    pub total_predictors: String,
    pub test_predictors: String,
    /// FULL_MODEL and NESTED_MODEL (in `p1` and `p2`) instead of PARTIAL_CORR.
    pub models: bool,
    /// PARTIAL_CORR.
    pub partial: String,
    pub intercept: bool,
    /// One flag and one range per entry of `power_charts`.
    pub charts: [bool; MOST_CHARTS],
    pub from: [String; MOST_CHARTS],
    pub to: [String; MOST_CHARTS],
}

/// A required number.
fn required(text: &str, what: &str) -> Result<f64, String> {
    optional(text, what)?.ok_or_else(|| format!("give {what}"))
}

/// A required number strictly between 0 and 1.
fn proportion(text: &str, what: &str) -> Result<f64, String> {
    let v = required(text, what)?;
    between(v, what)
}

fn between(v: f64, what: &str) -> Result<f64, String> {
    match v > 0.0 && v < 1.0 {
        true => Ok(v),
        false => Err(format!("{what} must lie above 0 and below 1")),
    }
}

fn positive(text: &str, what: &str) -> Result<f64, String> {
    match required(text, what)? {
        v if v > 0.0 => Ok(v),
        _ => Err(format!("{what} must be above 0")),
    }
}

fn join(values: &[f64]) -> String {
    let words: Vec<String> = values.iter().map(|v| number(*v)).collect();
    words.join(" ")
}

/// The two shares of discordant pairs from the success rates of the two occasions and their
/// correlation, as the engine forms them.
fn discordant(first: f64, second: f64, rho: f64) -> [f64; 2] {
    let a = first * (1.0 - second) - rho * (first * (1.0 - first) * second * (1.0 - second)).sqrt();
    [a, a + second - first]
}

impl PowerAnalysis {
    pub fn new(kind: PowerKind) -> Self {
        Self {
            kind,
            find_size: false,
            directional: false,
            significance: "0.05".into(),
            n1: String::new(),
            n2: String::new(),
            powers: String::new(),
            ratio: "1".into(),
            two_sds: false,
            sd1: String::new(),
            sd2: String::new(),
            two_means: false,
            mean1: String::new(),
            mean2: String::new(),
            null: match kind {
                PowerKind::ProportionsOneSample => "0.5",
                _ => "0",
            }
            .into(),
            alternative: String::new(),
            rho: "0.5".into(),
            p1: String::new(),
            p2: String::new(),
            counts: false,
            marginal: false,
            correlation: "0.5".into(),
            method: 0,
            binomial: false,
            continuity: false,
            pooled: true,
            timer: String::new(),
            weights: String::new(),
            coefficients: String::new(),
            pairwise: false,
            adjust: 0,
            partialout: String::new(),
            adjust_bias: true,
            variance: 0,
            random: false,
            total_predictors: String::new(),
            test_predictors: String::new(),
            models: false,
            partial: String::new(),
            intercept: true,
            charts: [false; MOST_CHARTS],
            from: Default::default(),
            to: Default::default(),
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            PowerKind::MeansOneSample => "Power Analysis: One-Sample T Test",
            PowerKind::MeansIndependent => "Power Analysis: Independent-Samples T Test",
            PowerKind::MeansRelated => "Power Analysis: Paired-Samples T Test",
            PowerKind::ProportionsOneSample => "Power Analysis: One-Sample Binomial Test",
            PowerKind::ProportionsIndependent => {
                "Power Analysis: Independent-Samples Binomial Test"
            }
            PowerKind::ProportionsRelated => "Power Analysis: Related-Samples Binomial Test",
            PowerKind::OnewayAnova => "Power Analysis: One-Way ANOVA",
            PowerKind::PearsonOneSample => "Power Analysis: Pearson Correlation",
            PowerKind::SpearmanOneSample => "Power Analysis: Spearman Correlation",
            PowerKind::PartialCorr => "Power Analysis: Partial Correlation",
            PowerKind::UnivariateLinear => "Power Analysis: Univariate Linear Regression",
        }
    }

    /// The smallest sample size the test allows, from the typed counts it depends on (a count
    /// not yet typed as a whole number counts as its least).
    pub fn least_size(&self) -> u32 {
        let count = |text: &str| text.trim().parse::<u32>().unwrap_or(0);
        match self.kind {
            PowerKind::ProportionsOneSample | PowerKind::ProportionsIndependent => 1,
            PowerKind::MeansOneSample
            | PowerKind::MeansIndependent
            | PowerKind::MeansRelated
            | PowerKind::ProportionsRelated
            | PowerKind::OnewayAnova => 2,
            PowerKind::PartialCorr => count(&self.partialout).saturating_add(4),
            PowerKind::PearsonOneSample => 4,
            PowerKind::SpearmanOneSample if self.variance == 2 => 3,
            PowerKind::SpearmanOneSample => 4,
            PowerKind::UnivariateLinear => count(&self.total_predictors)
                .max(1)
                .saturating_add(if self.intercept { 2 } else { 1 }),
        }
    }

    /// Two groups of their own sizes.
    pub fn two_groups(&self) -> bool {
        matches!(
            self.kind,
            PowerKind::MeansIndependent | PowerKind::ProportionsIndependent
        )
    }

    /// Pairs, counted by NPAIRS.
    pub fn pairs(&self) -> bool {
        matches!(
            self.kind,
            PowerKind::MeansRelated | PowerKind::ProportionsRelated
        )
    }

    pub fn proportions(&self) -> bool {
        matches!(
            self.kind,
            PowerKind::ProportionsOneSample
                | PowerKind::ProportionsIndependent
                | PowerKind::ProportionsRelated
        )
    }

    /// CONTINUITY applies: the normal test of one proportion, or the chi-square test of two.
    pub fn has_continuity(&self) -> bool {
        self.kind == PowerKind::ProportionsOneSample
            || (self.kind == PowerKind::ProportionsIndependent && self.method == 0)
    }

    /// POOLED applies: the chi-square test or the t-test of two proportions.
    pub fn has_pooled(&self) -> bool {
        self.kind == PowerKind::ProportionsIndependent && self.method <= 1
    }

    /// TIMER applies: the exact sample size search.
    pub fn has_timer(&self) -> bool {
        self.proportions() && self.binomial && self.find_size
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut words: Vec<String> = Vec::new();
        // ONEWAY ANOVA gives the side of its contrast test on CONTRAST; the F test of the
        // regression has none
        let sided = !matches!(
            self.kind,
            PowerKind::OnewayAnova | PowerKind::UnivariateLinear
        );
        if sided && self.directional {
            words.push("TEST=DIRECTIONAL".into());
        }
        if let Some(alpha) = optional(&self.significance, "the significance level")? {
            between(alpha, "the significance level")?;
            if alpha != 0.05 {
                words.push(format!("SIGNIFICANCE={}", number(alpha)));
            }
        }
        let mut sizes = Vec::new();
        if self.find_size {
            let powers = numbers(&self.powers, "each power")?;
            if powers.is_empty() {
                return Err("give at least one power".into());
            }
            for &v in &powers {
                between(v, "each power")?;
            }
            words.push(format!("POWER={}", join(&powers)));
            if self.two_groups()
                && let Some(r) = optional(&self.ratio, "the ratio of the group sizes")?
            {
                if self.kind == PowerKind::MeansIndependent && !(0.01..=100.0).contains(&r) {
                    return Err("the ratio of the group sizes must lie from 0.01 to 100".into());
                }
                if self.kind == PowerKind::ProportionsIndependent && !(r > 0.01 && r < 100.0) {
                    return Err(
                        "the ratio of the group sizes must lie above 0.01 and below 100".into(),
                    );
                }
                if r != 1.0 {
                    words.push(format!("NRATIO={}", number(r)));
                }
            }
        } else if self.kind == PowerKind::OnewayAnova {
            sizes = numbers(&self.n1, "each group size")?;
            if sizes.is_empty() {
                return Err("give the group sizes".into());
            }
            if sizes.iter().any(|n| n.fract() != 0.0 || *n < 2.0) {
                return Err("each group size must be a whole number of at least 2".into());
            }
            words.push(format!("GROUP_SIZES={}", join(&sizes)));
        } else {
            let least = self.least_size();
            let size = |text: &str, what: &str| {
                whole(text, least, what)?
                    .map(f64::from)
                    .ok_or_else(|| format!("give {what}"))
            };
            if self.two_groups() {
                sizes.push(size(&self.n1, "the size of group 1")?);
                sizes.push(size(&self.n2, "the size of group 2")?);
            } else if self.pairs() {
                sizes.push(size(&self.n1, "the number of pairs")?);
            } else {
                sizes.push(size(&self.n1, "the sample size")?);
            }
            let keyword = if self.pairs() { "NPAIRS" } else { "N" };
            words.push(format!("{keyword}={}", join(&sizes)));
        }
        let mut contrast = None;
        let unequal = match self.kind {
            PowerKind::MeansOneSample | PowerKind::MeansIndependent | PowerKind::MeansRelated => {
                self.mean_parameters(&mut words)?
            }
            PowerKind::ProportionsOneSample
            | PowerKind::ProportionsIndependent
            | PowerKind::ProportionsRelated => {
                self.proportion_parameters(&sizes, &mut words)?;
                false
            }
            PowerKind::OnewayAnova => {
                contrast = self.anova_parameters(sizes.len(), &mut words)?;
                false
            }
            PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
                self.correlation_parameters(&mut words)?;
                false
            }
            PowerKind::UnivariateLinear => {
                self.regression_parameters(&mut words)?;
                false
            }
        };
        let mut text = format!("{}\n  /PARAMETERS {}", self.kind.name(), words.join(" "));
        if let Some(contrast) = contrast {
            text.push_str(&format!("\n  /CONTRAST {contrast}"));
        }
        let plots = self.plots(unequal)?;
        if !plots.is_empty() {
            text.push_str(&format!("\n  /PLOT {}", plots.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// SD, RHO, MEAN and NULL; true for two different SDs of two groups (the separate-variances
    /// test, whose effect is the mean difference).
    fn mean_parameters(&self, words: &mut Vec<String>) -> Result<bool, String> {
        let one = self.kind == PowerKind::MeansOneSample;
        let sds = match !one && self.two_sds {
            true => vec![
                positive(&self.sd1, "the first standard deviation")?,
                positive(&self.sd2, "the second standard deviation")?,
            ],
            false => vec![positive(&self.sd1, "the standard deviation")?],
        };
        words.push(format!("SD={}", join(&sds)));
        if self.kind == PowerKind::MeansRelated
            && sds.len() == 2
            && let Some(rho) = optional(&self.rho, "the correlation of the measures")?
        {
            if !(-1.0..=1.0).contains(&rho) || rho == 0.0 {
                return Err(
                    "the correlation of the measures must lie from -1 to 1 and not be 0".into(),
                );
            }
            if rho == 1.0 && sds[0] == sds[1] {
                return Err(
                    "with equal standard deviations the correlation of the measures cannot be 1"
                        .into(),
                );
            }
            if rho != 0.5 {
                words.push(format!("RHO={}", number(rho)));
            }
        }
        let means = match !one && self.two_means {
            true => vec![
                required(&self.mean1, "the first mean")?,
                required(&self.mean2, "the second mean")?,
            ],
            false if one => vec![required(&self.mean1, "the mean")?],
            false => vec![required(&self.mean1, "the mean difference")?],
        };
        words.push(format!("MEAN={}", join(&means)));
        let null = match one {
            true => optional(&self.null, "the null hypothesis mean")?.unwrap_or(0.0),
            false => 0.0,
        };
        if null != 0.0 {
            words.push(format!("NULL={}", number(null)));
        }
        let difference = match means.as_slice() {
            [m] => m - null,
            [m1, m2] => m1 - m2,
            _ => 0.0,
        };
        if self.find_size && difference == 0.0 {
            return Err(match one {
                true => "to find a sample size, the mean must differ from the null mean".into(),
                false => "to find a sample size, the mean difference must not be 0".into(),
            });
        }
        Ok(self.kind == PowerKind::MeansIndependent && sds.len() == 2 && sds[0] != sds[1])
    }

    /// POOLED_SD and GROUP_MEANS, checked against the number of `sizes` given (0 when they are
    /// searched), and the words of CONTRAST, if any.
    fn anova_parameters(
        &self,
        sizes: usize,
        words: &mut Vec<String>,
    ) -> Result<Option<String>, String> {
        let sd = positive(&self.sd1, "the pooled standard deviation")?;
        words.push(format!("POOLED_SD={}", number(sd)));
        let means = numbers(&self.mean1, "each group mean")?;
        if means.len() < 2 {
            return Err("give the means of at least two groups".into());
        }
        let groups = means.len();
        if sizes != 0 && sizes != groups {
            return Err(format!("give {groups} group sizes, one per mean"));
        }
        if self.find_size && means.iter().all(|m| *m == means[0]) {
            return Err("to find the group sizes, the group means must not all be equal".into());
        }
        words.push(format!("GROUP_MEANS={}", join(&means)));
        // the weights only shape the sizes searched for
        if self.find_size {
            let weights = numbers(&self.weights, "each group weight")?;
            if weights.iter().any(|w| !(1.0..=100.0).contains(w)) {
                return Err("each group weight must lie from 1 to 100".into());
            }
            if !weights.is_empty() && weights.len() != groups {
                return Err(format!(
                    "give {groups} group weights, one per mean, or none"
                ));
            }
            if !weights.is_empty() {
                words.push(format!("GROUP_WEIGHTS={}", join(&weights)));
            }
        }
        let coefficients = numbers(&self.coefficients, "each contrast coefficient")?;
        let mut contrast = Vec::new();
        if !coefficients.is_empty() {
            if coefficients.len() != groups {
                return Err(format!("give {groups} contrast coefficients, one per mean"));
            }
            if coefficients.iter().all(|c| *c == 0.0) {
                return Err("the contrast coefficients cannot all be 0".into());
            }
            contrast.push(format!("COEFFICIENTS={}", join(&coefficients)));
            if self.directional {
                contrast.push("TEST=DIRECTIONAL".into());
            }
        }
        if self.pairwise {
            contrast.push("PAIRWISE=TRUE".into());
            if self.adjust != 0 {
                contrast.push(format!("ADJUST={}", POWER_ADJUSTS[self.adjust].0));
            }
        }
        Ok((!contrast.is_empty()).then(|| contrast.join(" ")))
    }

    /// PARTIALOUT, NULL, ALTERNATIVE and the options of the tests of one correlation.
    fn correlation_parameters(&self, words: &mut Vec<String>) -> Result<(), String> {
        if self.kind == PowerKind::PartialCorr {
            let what = "the number of variables held fixed";
            let k = whole(&self.partialout, 0, what)?.ok_or_else(|| format!("give {what}"))?;
            words.push(format!("PARTIALOUT={k}"));
        }
        let null = optional(&self.null, "the null correlation")?.unwrap_or(0.0);
        let alternative = required(&self.alternative, "the alternative correlation")?;
        for (v, what) in [
            (null, "the null correlation"),
            (alternative, "the alternative correlation"),
        ] {
            if !(-1.0..=1.0).contains(&v) {
                return Err(format!("{what} must lie from -1 to 1"));
            }
        }
        if self.find_size && (null.abs() == 1.0 || alternative.abs() == 1.0) {
            return Err(
                "to find a sample size, the correlations must lie above -1 and below 1".into(),
            );
        }
        if self.find_size && null == alternative {
            return Err(
                "to find a sample size, the alternative correlation must differ from the null one"
                    .into(),
            );
        }
        if null != 0.0 {
            words.push(format!("NULL={}", number(null)));
        }
        words.push(format!("ALTERNATIVE={}", number(alternative)));
        if self.kind == PowerKind::PearsonOneSample && !self.adjust_bias {
            words.push("ADJUST_BIAS=FALSE".into());
        }
        if self.kind == PowerKind::SpearmanOneSample && self.variance != 0 {
            words.push(format!("VARIANCE={}", POWER_VARIANCES[self.variance].0));
        }
        Ok(())
    }

    /// MODEL, the predictors, the effect and INTERCEPT of a linear regression.
    fn regression_parameters(&self, words: &mut Vec<String>) -> Result<(), String> {
        if self.random {
            words.push("MODEL=RANDOM".into());
        }
        let count =
            |text: &str, what: &str| whole(text, 1, what)?.ok_or_else(|| format!("give {what}"));
        let total = count(&self.total_predictors, "the number of predictors")?;
        let tested = count(&self.test_predictors, "the number of tested predictors")?;
        words.push(format!("TOTAL_PREDICTORS={total} TEST_PREDICTORS={tested}"));
        let finding = "to find a sample size";
        if self.models {
            let full = proportion(&self.p1, "the R squared of the full model")?;
            let nested = proportion(&self.p2, "the R squared of the nested model")?;
            if tested >= total {
                return Err(
                    "with two models, fewer predictors must be tested than the model has".into(),
                );
            }
            if full < nested || (self.find_size && full == nested) {
                return Err(match self.find_size {
                    true => format!("{finding}, the full model must have the larger R squared"),
                    false => "the full model cannot have the smaller R squared".into(),
                });
            }
            words.push(format!(
                "FULL_MODEL={} NESTED_MODEL={}",
                number(full),
                number(nested)
            ));
        } else {
            let r = required(&self.partial, "the partial correlation")?;
            if !(r > -1.0 && r < 1.0) {
                return Err("the partial correlation must lie above -1 and below 1".into());
            }
            if tested > total {
                return Err("more predictors cannot be tested than the model has".into());
            }
            if self.find_size && r == 0.0 {
                return Err(format!("{finding}, the partial correlation must not be 0"));
            }
            words.push(format!("PARTIAL_CORR={}", number(r)));
        }
        if !self.intercept {
            words.push("INTERCEPT=FALSE".into());
        }
        Ok(())
    }

    /// The values and options of the proportions; `sizes` are those given, if any.
    fn proportion_parameters(&self, sizes: &[f64], words: &mut Vec<String>) -> Result<(), String> {
        let finding = "to find a sample size";
        match self.kind {
            PowerKind::ProportionsOneSample => {
                let null = optional(&self.null, "the null proportion")?
                    .map(|v| between(v, "the null proportion"))
                    .transpose()?
                    .unwrap_or(0.5);
                let alternative = proportion(&self.alternative, "the alternative proportion")?;
                if self.find_size && alternative == null {
                    return Err(format!(
                        "{finding}, the alternative proportion must differ from the null one"
                    ));
                }
                if null != 0.5 {
                    words.push(format!("NULL={}", number(null)));
                }
                words.push(format!("ALTERNATIVE={}", number(alternative)));
            }
            PowerKind::ProportionsIndependent => {
                let p1 = proportion(&self.p1, "the proportion of group 1")?;
                let p2 = proportion(&self.p2, "the proportion of group 2")?;
                if self.find_size && p1 == p2 {
                    return Err(format!("{finding}, the two proportions must differ"));
                }
                words.push(format!("PROPORTIONS={}", join(&[p1, p2])));
                if self.method != 0 {
                    words.push(format!("METHOD={}", POWER_METHODS[self.method].0));
                }
                if self.method == 1 && !sizes.is_empty() && sizes.iter().sum::<f64>() <= 2.0 {
                    return Err("the t-test needs more than 2 cases in the two groups".into());
                }
            }
            _ => self.related_values(sizes, words)?,
        }
        if self.binomial {
            words.push("ESTIMATE=BINOMIAL".into());
        }
        if self.has_continuity() && self.continuity {
            words.push("CONTINUITY=TRUE".into());
        }
        if self.has_pooled() && !self.pooled {
            words.push("POOLED=FALSE".into());
        }
        if self.has_timer()
            && let Some(minutes) = whole(&self.timer, 1, "the time limit in minutes")?
        {
            words.push(format!("TIMER={minutes}"));
        }
        Ok(())
    }

    /// VALUES, MARGINAL and CORRELATION of related proportions, checked as the engine does: the
    /// two shares of discordant pairs must be possible and, to find a size, differ.
    fn related_values(&self, sizes: &[f64], words: &mut Vec<String>) -> Result<(), String> {
        let counts = self.counts && !self.find_size;
        let (first, second) = match self.marginal {
            true => ("the first success rate", "the second success rate"),
            false => ("the first share", "the second share"),
        };
        let values = match counts {
            true => {
                let count = |text: &str, what: &str| {
                    whole(text, 0, what)?
                        .map(f64::from)
                        .ok_or_else(|| format!("give {what}"))
                };
                let (a, b) = (
                    count(&self.p1, "the first count")?,
                    count(&self.p2, "the second count")?,
                );
                if a + b > sizes[0] {
                    return Err("the two counts add up to more than the number of pairs".into());
                }
                words.push(format!("VALUES=COUNTS({})", join(&[a, b])));
                [a / sizes[0], b / sizes[0]]
            }
            false => {
                let mut values = [required(&self.p1, first)?, required(&self.p2, second)?];
                for (v, what) in values.iter_mut().zip([first, second]) {
                    if !(0.0..=1.0).contains(v) {
                        return Err(format!("{what} must lie from 0 to 1"));
                    }
                }
                words.push(format!("VALUES=PROPORTIONS({})", join(&values)));
                values
            }
        };
        let shares = match self.marginal {
            true => {
                words.push("MARGINAL=TRUE".into());
                if values.iter().any(|v| *v <= 0.0 || *v >= 1.0) {
                    return Err("the success rates must lie above 0 and below 1".into());
                }
                let rho =
                    optional(&self.correlation, "the correlation of the occasions")?.unwrap_or(0.5);
                if !(rho > -1.0 && rho < 1.0) {
                    return Err(
                        "the correlation of the occasions must lie above -1 and below 1".into(),
                    );
                }
                if rho != 0.5 {
                    words.push(format!("CORRELATION={}", number(rho)));
                }
                discordant(values[0], values[1], rho)
            }
            false => values,
        };
        let [a, b] = shares;
        if a < 0.0 || b < 0.0 || a + b > 1.0 + 1e-12 || a + b == 0.0 {
            return Err(match self.marginal {
                true => "these success rates cannot have this correlation: a share of \
                         discordant pairs would fall outside 0 to 1"
                    .into(),
                false => "the two shares must add up to at most 1 and not both be 0".into(),
            });
        }
        if self.find_size && a == b {
            return Err(match self.marginal {
                true => "to find a number of pairs, the two success rates must differ".into(),
                false => "to find a number of pairs, the two shares must differ".into(),
            });
        }
        Ok(())
    }

    /// The charts checked, each with its range when both ends are typed.
    fn plots(&self, unequal: bool) -> Result<Vec<String>, String> {
        let mut plots = Vec::new();
        for (i, &(keywords, label, ranged)) in power_charts(self.kind).iter().enumerate() {
            if !self.charts[i] || (needs_marginal(self.kind, keywords) && !self.marginal) {
                continue;
            }
            let ends = (
                optional(&self.from[i], label)?,
                optional(&self.to[i], label)?,
            );
            match (ranged, ends) {
                (true, (Some(low), Some(high))) => {
                    self.check_range(keywords, label, low, high, unequal)?;
                    plots.push(format!("{keywords}({} {})", number(low), number(high)));
                }
                (true, (None, Some(_)) | (Some(_), None)) => {
                    return Err(format!("{label}: give both ends of the range or neither"));
                }
                _ => plots.push(keywords.to_string()),
            }
        }
        Ok(plots)
    }

    /// The limits the parser and the engine put on a chart range.
    fn check_range(
        &self,
        keyword: &str,
        label: &str,
        low: f64,
        high: f64,
        unequal: bool,
    ) -> Result<(), String> {
        if low >= high {
            return Err(format!("{label}: the range must run upwards"));
        }
        let independent_means = self.kind == PowerKind::MeansIndependent;
        // the lowest start, the highest end, and whether the ends are whole numbers
        let (least, most, whole) = match keyword {
            "N" => (f64::from(self.least_size()), f64::INFINITY, true),
            "TOTAL_N" => {
                let (least, most) = self.total_range();
                (least, most, true)
            }
            "NPAIRS" if self.kind == PowerKind::MeansRelated => (2.0, 5000.0, true),
            "NPAIRS" => (2.0, 2500.0, true),
            "ES" if unequal => (0.0, 5000.0, false),
            "ES" if self.directional => (-5.0, 5.0, false),
            "ES" => (0.0, 5.0, false),
            "NRATIO" if independent_means => (0.2, 5.0, false),
            // the ratios of proportions and the pooled standard deviation: only above 0
            _ => {
                return match low > 0.0 {
                    true => Ok(()),
                    false => Err(format!("{label}: the range must lie above 0")),
                };
            }
        };
        if whole && (low.fract() != 0.0 || high.fract() != 0.0) {
            return Err(format!("{label}: the range takes whole numbers"));
        }
        if low < least || high > most {
            return Err(match most.is_finite() {
                true => format!(
                    "{label}: the range must lie from {} to {}",
                    number(least),
                    number(most)
                ),
                false => format!("{label}: the range must start at {} or more", number(least)),
            });
        }
        Ok(())
    }

    /// The totals a chart over the total size of ONEWAY ANOVA may span: every group at least 2
    /// and at most 5000, the groups kept in proportion.
    fn total_range(&self) -> (f64, f64) {
        let list = |text: &str| numbers(text, "").unwrap_or_default();
        // shortcut: a search takes the proportions of the weights, while the engine takes those
        // of the sizes it finds (each rounded up); exact for equal groups, close otherwise
        let groups = match (self.find_size, list(&self.weights)) {
            (false, _) => list(&self.n1),
            (true, weights) if !weights.is_empty() => weights,
            (true, _) => vec![1.0; list(&self.mean1).len()],
        };
        let total: f64 = groups.iter().sum();
        let smallest = groups.iter().copied().fold(f64::INFINITY, f64::min);
        let largest = groups.iter().copied().fold(0.0, f64::max);
        (
            (2.0 * total / smallest).ceil(),
            (5000.0 / largest * total).floor().min(2_147_483_647.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn power_checks_what_the_command_would_reject() {
        let err = |kind: PowerKind, change: &dyn Fn(&mut PowerAnalysis)| {
            let mut d = PowerAnalysis::new(kind);
            change(&mut d);
            d.syntax().unwrap_err()
        };
        use PowerKind::*;
        // the empty dialog asks for the first value it needs
        assert_eq!(err(MeansOneSample, &|_| {}), "give the sample size");
        assert_eq!(
            err(MeansOneSample, &|d| d.n1 = s("1")),
            "the sample size must be a whole number of at least 2"
        );
        assert_eq!(
            err(MeansOneSample, &|d| {
                d.find_size = true;
                d.powers = s("0.8 1");
            }),
            "each power must lie above 0 and below 1"
        );
        assert_eq!(
            err(MeansOneSample, &|d| {
                d.find_size = true;
                d.powers = s("0.8");
                d.sd1 = s("3");
                d.mean1 = s("0");
            }),
            "to find a sample size, the mean must differ from the null mean"
        );
        assert_eq!(
            err(MeansIndependent, &|d| {
                d.n1 = s("20");
                d.n2 = s("20");
                d.two_sds = true;
                d.sd1 = s("2");
                d.sd2 = s("0");
            }),
            "the second standard deviation must be above 0"
        );
        assert_eq!(
            err(MeansRelated, &|d| {
                d.n1 = s("20");
                d.two_sds = true;
                d.sd1 = s("2");
                d.sd2 = s("2");
                d.rho = s("1");
                d.mean1 = s("1");
            }),
            "with equal standard deviations the correlation of the measures cannot be 1"
        );
        assert_eq!(
            err(ProportionsOneSample, &|d| {
                d.n1 = s("20");
                d.alternative = s("1.2");
            }),
            "the alternative proportion must lie above 0 and below 1"
        );
        assert_eq!(
            err(ProportionsIndependent, &|d| {
                d.n1 = s("1");
                d.n2 = s("1");
                d.p1 = s("0.2");
                d.p2 = s("0.4");
                d.method = 1;
            }),
            "the t-test needs more than 2 cases in the two groups"
        );
        assert_eq!(
            err(ProportionsRelated, &|d| {
                d.n1 = s("30");
                d.counts = true;
                d.p1 = s("20");
                d.p2 = s("15");
            }),
            "the two counts add up to more than the number of pairs"
        );
        assert_eq!(
            err(ProportionsRelated, &|d| {
                d.n1 = s("30");
                d.marginal = true;
                d.p1 = s("0.9");
                d.p2 = s("0.1");
                d.correlation = s("0.9");
            }),
            "these success rates cannot have this correlation: a share of discordant pairs \
             would fall outside 0 to 1"
        );
        assert_eq!(
            err(MeansOneSample, &|d| {
                d.n1 = s("20");
                d.sd1 = s("3");
                d.mean1 = s("1");
                d.charts[0] = true;
                d.from[0] = s("1");
                d.to[0] = s("40");
            }),
            "Power by sample size: the range must start at 2 or more"
        );
        assert_eq!(
            err(MeansOneSample, &|d| {
                d.n1 = s("20");
                d.sd1 = s("3");
                d.mean1 = s("1");
                d.charts[1] = true;
                d.from[1] = s("0.5");
            }),
            "Power by effect size: give both ends of the range or neither"
        );
        assert_eq!(
            err(OnewayAnova, &|d| {
                d.n1 = s("8 8");
                d.sd1 = s("2");
                d.mean1 = s("3 4 5");
            }),
            "give 3 group sizes, one per mean"
        );
        assert_eq!(
            err(OnewayAnova, &|d| {
                d.find_size = true;
                d.powers = s("0.8");
                d.sd1 = s("2");
                d.mean1 = s("4 4");
            }),
            "to find the group sizes, the group means must not all be equal"
        );
        assert_eq!(
            err(OnewayAnova, &|d| {
                d.n1 = s("6 6 6");
                d.sd1 = s("2");
                d.mean1 = s("3 4 5");
                d.charts[0] = true;
                d.from[0] = s("4");
                d.to[0] = s("30");
            }),
            "Power by total sample size: the range must lie from 6 to 15000"
        );
        // PARTIALOUT raises the least sample size
        assert_eq!(
            err(PartialCorr, &|d| {
                d.n1 = s("8");
                d.partialout = s("6");
            }),
            "the sample size must be a whole number of at least 10"
        );
        assert_eq!(
            err(PearsonOneSample, &|d| {
                d.find_size = true;
                d.powers = s("0.8");
                d.alternative = s("1");
            }),
            "to find a sample size, the correlations must lie above -1 and below 1"
        );
        assert_eq!(
            err(UnivariateLinear, &|d| {
                d.n1 = s("50");
                d.total_predictors = s("3");
                d.test_predictors = s("3");
                d.models = true;
                d.p1 = s("0.3");
                d.p2 = s("0.2");
            }),
            "with two models, fewer predictors must be tested than the model has"
        );
        // settings that do not apply are not written: a side without coefficients, an
        // adjustment without pairs, weights of given sizes
        let mut d = PowerAnalysis::new(OnewayAnova);
        d.n1 = s("5 5");
        d.sd1 = s("1");
        d.mean1 = s("2 3");
        d.directional = true;
        d.adjust = 2;
        d.weights = s("0");
        assert_eq!(
            d.syntax().unwrap(),
            "POWER ONEWAY ANOVA\n  /PARAMETERS GROUP_SIZES=5 5 POOLED_SD=1 GROUP_MEANS=2 3."
        );
        // a range of a chart left unchecked, and a timer of the normal approximation, are not
        // checked
        let mut d = PowerAnalysis::new(ProportionsOneSample);
        d.n1 = s("40");
        d.alternative = s("0.7");
        d.from[0] = s("0");
        d.timer = s("0");
        assert_eq!(
            d.syntax().unwrap(),
            "POWER PROPORTIONS ONESAMPLE\n  /PARAMETERS N=40 ALTERNATIVE=0.7."
        );
    }
}
