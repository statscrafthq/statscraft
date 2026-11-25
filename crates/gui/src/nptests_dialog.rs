//! The One Sample, Independent Samples and Related Samples dialogs of Nonparametric Tests
//! (NPTESTS). Left to the data, the subcommand names only its fields (and the grouping field), so
//! the command picks the tests by measurement level. Options left as they are add nothing to the
//! syntax.

use crate::advanced_dialogs::ci_level;
use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::i18n::{t, t_args};
use crate::stats_dialogs::parse;
use crate::survival_dialog::{items, numbers, values};
use crate::syntax_text::{names, number, probability};
use statscraft_data::{Dataset, Measure};

/// Which of the three dialogs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NpKind {
    OneSample,
    Independent,
    Related,
}

/// How a k-sample test follows up a rejection; the first writes nothing.
pub const NP_COMPARE: [(&str, &str); 3] = [
    ("PAIRWISE", "stats-nptests-all-pairs"),
    ("STEPWISE", "stats-nptests-stepwise-step-down"),
    ("NONE", "stats-nptests-none"),
];

/// The confidence intervals of the binomial test.
pub const BINOMIAL_INTERVALS: [(&str, &str); 3] = [
    ("CLOPPERPEARSON", "stats-nptests-clopper-pearson-exact"),
    ("JEFFREYS", "Jeffreys"),
    ("LIKELIHOOD", "stats-nptests-likelihood-ratio"),
];

/// The distributions of the one-sample Kolmogorov-Smirnov test.
pub const KS_DISTRIBUTIONS: [&str; 4] = [
    "stats-nptests-normal",
    "stats-nptests-uniform",
    "stats-nptests-exponential",
    "Poisson",
];

/// Where the runs test cuts a continuous field; the first writes nothing.
pub const RUNS_CUTS: [(&str, &str); 3] = [
    ("SAMPLEMEDIAN", "stats-nptests-median"),
    ("SAMPLEMEAN", "stats-nptests-mean"),
    ("", "stats-nptests-a-value"),
];

/// The tests of two groups that take no setting.
pub const INDEPENDENT_TWO: [(&str, &str); 4] = [
    ("MANN_WHITNEY", "Mann-Whitney U"),
    ("WALD_WOLFOWITZ", "stats-nptests-wald-wolfowitz-runs"),
    ("KOLMOGOROV_SMIRNOV", "Kolmogorov-Smirnov"),
    ("HODGES_LEHMAN", "stats-nptests-hodges-lehmann-estimate"),
];

/// The k-sample tests of independent groups, each with its comparisons.
pub const INDEPENDENT_K: [(&str, &str); 3] = [
    ("KRUSKAL_WALLIS", "Kruskal-Wallis"),
    ("MEDIAN", "stats-nptests-median-test"),
    ("JONCKHEERE_TERPSTRA", "Jonckheere-Terpstra"),
];

/// The tests of two continuous or ordinal fields.
pub const RELATED_TWO: [(&str, &str); 4] = [
    ("WILCOXON", "stats-nptests-wilcoxon-signed-rank"),
    ("SIGN", "stats-nptests-sign"),
    ("MARGINAL_HOMOGENEITY", "stats-nptests-marginal-homogeneity"),
    ("HODGES_LEHMAN", "stats-nptests-hodges-lehmann-estimate"),
];

/// The k-sample tests of related fields, each with its comparisons.
pub const RELATED_K: [(&str, &str); 3] = [
    ("COCHRAN", "stats-nptests-cochrans-q"),
    ("FRIEDMAN", "Friedman"),
    ("KENDALL", "stats-nptests-kendalls-w"),
];

/// NPTESTS with one test subcommand.
#[derive(Clone, PartialEq, Debug)]
pub struct Nptests {
    pub kind: NpKind,
    pub fields: Vec<String>,
    /// GROUP of the independent samples.
    pub group: String,
    /// The tests are chosen here; off, the command chooses them from the data.
    pub customize: bool,

    // one sample
    pub binomial: bool,
    /// Empty for 0.5.
    pub proportion: String,
    /// The binomial, McNemar and Cochran tests count these values as success, not the first
    /// value found.
    pub success_list: bool,
    pub success: String,
    /// The binomial test cuts continuous fields at `cut`, not halfway between the extremes.
    pub cut_value: bool,
    pub cut: String,
    /// One flag per entry of `BINOMIAL_INTERVALS`.
    pub intervals: [bool; 3],
    pub chisquare: bool,
    /// Expected frequencies of the given categories, not equal ones.
    pub expected_custom: bool,
    pub categories: String,
    pub frequencies: String,
    pub ks: bool,
    /// One flag per entry of `KS_DISTRIBUTIONS`.
    pub distributions: [bool; 4],
    pub normal_custom: bool,
    pub normal_mean: String,
    pub normal_sd: String,
    /// SIMULATION=FALSE: the Lilliefors table instead of Monte Carlo samples.
    pub lilliefors: bool,
    pub uniform_custom: bool,
    pub uniform_min: String,
    pub uniform_max: String,
    pub exponential_custom: bool,
    pub exponential_mean: String,
    pub poisson_mean: String,
    /// Empty for 1000.
    pub samples: String,
    /// Empty for 99.
    pub mc_level: String,
    pub runs: bool,
    /// The runs test makes one group of these values of a categorical field.
    pub runs_list: bool,
    pub runs_values: String,
    /// Index into `RUNS_CUTS`.
    pub runs_cut: usize,
    pub runs_cut_value: String,
    pub wilcoxon: bool,
    /// The hypothesized median of the one-sample Wilcoxon test; required.
    pub test_median: String,

    // independent and related samples
    /// One flag per entry of `INDEPENDENT_TWO` or `RELATED_TWO`.
    pub two_tests: [bool; 4],
    /// One flag per entry of `INDEPENDENT_K` or `RELATED_K`.
    pub k_tests: [bool; 3],
    /// Index into `NP_COMPARE` for each of `k_tests`.
    pub compare: [usize; 3],
    pub moses: bool,
    /// Cases trimmed from each end of the control group; empty for 5 percent.
    pub trim: String,
    /// The value the median test compares with; empty for the pooled median.
    pub median_value: String,
    /// ORDER=DESCENDING of the Jonckheere-Terpstra test.
    pub descending: bool,
    pub mcnemar: bool,

    /// SCOPE=LISTWISE: a case missing on any field named is left out of every test.
    pub listwise: bool,
    /// USERMISSING=INCLUDE: user-missing values of categorical fields are valid.
    pub include: bool,
    /// Empty for 0.05.
    pub alpha: String,
    /// Empty for 95.
    pub ci_level: String,
}

impl Nptests {
    pub fn new(kind: NpKind) -> Self {
        Self {
            kind,
            fields: Vec::new(),
            group: String::new(),
            customize: false,
            binomial: false,
            proportion: String::new(),
            success_list: false,
            success: String::new(),
            cut_value: false,
            cut: String::new(),
            intervals: [false; 3],
            chisquare: false,
            expected_custom: false,
            categories: String::new(),
            frequencies: String::new(),
            ks: false,
            distributions: [true, false, false, false],
            normal_custom: false,
            normal_mean: String::new(),
            normal_sd: String::new(),
            lilliefors: false,
            uniform_custom: false,
            uniform_min: String::new(),
            uniform_max: String::new(),
            exponential_custom: false,
            exponential_mean: String::new(),
            poisson_mean: String::new(),
            samples: String::new(),
            mc_level: String::new(),
            runs: false,
            runs_list: false,
            runs_values: String::new(),
            runs_cut: 0,
            runs_cut_value: String::new(),
            wilcoxon: false,
            test_median: String::new(),
            two_tests: [false; 4],
            k_tests: [false; 3],
            compare: [0; 3],
            moses: false,
            trim: String::new(),
            median_value: String::new(),
            descending: false,
            mcnemar: false,
            listwise: false,
            include: false,
            alpha: String::new(),
            ci_level: String::new(),
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            NpKind::OneSample => t("stats-nptests-title-one-sample"),
            NpKind::Independent => t("stats-nptests-title-independent"),
            NpKind::Related => t("stats-nptests-title-related"),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let fields = names(&self.fields, t("stats-nptests-test-fields"))?;
        let group = self.group.trim();
        if let Some(w) = data.weight_variable()
            && let Some(name) = self.fields.iter().map(String::as_str).find(|n| *n == w)
        {
            return Err(t_args(
                "stats-nptests-weight-cannot-be-tested",
                &[("name", name.into())],
            ));
        }
        // typed values are quoted only when every field holds text
        let string = self
            .fields
            .iter()
            .all(|n| data.variable(n).is_ok_and(|v| v.is_string()));
        let mut text = match self.kind {
            NpKind::OneSample => format!("NPTESTS /ONESAMPLE TEST({fields})"),
            NpKind::Independent => {
                if group.is_empty() {
                    return Err(t("stats-nptests-set-grouping-variable").into());
                }
                if data.weight_variable() == Some(group) {
                    return Err(t_args(
                        "stats-nptests-weight-cannot-group",
                        &[("name", group.into())],
                    ));
                }
                if data
                    .variable(group)
                    .is_ok_and(|v| v.measure() == Measure::Scale && !v.is_string())
                {
                    return Err(t_args(
                        "stats-nptests-scale-cannot-group",
                        &[("name", group.into())],
                    ));
                }
                format!("NPTESTS /INDEPENDENT TEST({fields}) GROUP({group})")
            }
            NpKind::Related => {
                if self.fields.len() < 2 {
                    return Err(t("stats-nptests-two-test-fields").into());
                }
                format!("NPTESTS /RELATED TEST({fields})")
            }
        };
        if self.customize {
            let tests = match self.kind {
                NpKind::OneSample => self.one_sample(string)?,
                NpKind::Independent => self.independent()?,
                NpKind::Related => self.related(string)?,
            };
            if tests.is_empty() {
                return Err(t("stats-nptests-choose-a-test").into());
            }
            for test in tests {
                text.push_str("\n    ");
                text.push_str(&test);
            }
        }
        let mut missing = Vec::new();
        if self.listwise {
            missing.push("SCOPE=LISTWISE");
        }
        if self.include {
            missing.push("USERMISSING=INCLUDE");
        }
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING {}", missing.join(" ")));
        }
        let mut criteria = Vec::new();
        if !self.alpha.trim().is_empty() {
            let alpha = probability(&self.alpha)?;
            if alpha != "0.05" {
                criteria.push(format!("ALPHA={alpha}"));
            }
        }
        let level = ci_level(&self.ci_level)?;
        if level != 95.0 {
            criteria.push(format!("CILEVEL={}", number(level)));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// `LIST(values)` of the success values.
    fn success_list(&self, string: bool) -> Result<String, String> {
        let list = values(&self.success, string)?;
        if list.is_empty() {
            return Err(t("stats-nptests-type-success-values").into());
        }
        Ok(format!("LIST({list})"))
    }

    fn one_sample(&self, string: bool) -> Result<Vec<String>, String> {
        let mut tests = Vec::new();
        if self.binomial {
            let mut options = Vec::new();
            if !self.proportion.trim().is_empty() {
                let p = parse(&self.proportion, t("stats-nptests-what-test-proportion"))?;
                if !(p > 0.0 && p < 1.0) {
                    return Err(t("stats-nptests-proportion-between").into());
                }
                if p != 0.5 {
                    options.push(format!("TESTVALUE={}", number(p)));
                }
            }
            if self.success_list {
                options.push(format!("SUCCESSCATEGORICAL={}", self.success_list(string)?));
            }
            if self.cut_value {
                let cut = parse(&self.cut, t("stats-nptests-what-binomial-cut"))?;
                options.push(format!("SUCCESSCONTINUOUS=CUTPOINT({})", number(cut)));
            }
            options.extend(
                keywords(&BINOMIAL_INTERVALS, &self.intervals)
                    .into_iter()
                    .map(String::from),
            );
            tests.push(with_options("BINOMIAL", &options));
        }
        if self.chisquare {
            let mut options = Vec::new();
            if self.expected_custom {
                let categories = values(&self.categories, string)?;
                let count = items(&self.categories, string).count();
                let frequencies = numbers(
                    &self.frequencies,
                    t("stats-nptests-what-expected-frequency"),
                )?;
                if count == 0 || frequencies.is_empty() {
                    return Err(t("stats-nptests-type-categories-frequencies").into());
                }
                if frequencies.iter().any(|f| *f <= 0.0) {
                    return Err(t("stats-nptests-frequencies-above-zero").into());
                }
                if frequencies.len() != count {
                    return Err(t_args(
                        "stats-nptests-frequencies-for-categories",
                        &[
                            ("frequencies", frequencies.len().to_string().into()),
                            ("categories", count.to_string().into()),
                        ],
                    ));
                }
                let frequencies: Vec<String> = frequencies.into_iter().map(number).collect();
                options.push(format!(
                    "EXPECTED=CUSTOM(FREQUENCIES={} CATEGORIES={categories})",
                    frequencies.join(" ")
                ));
            }
            tests.push(with_options("CHISQUARE", &options));
        }
        if self.ks {
            tests.push(self.kolmogorov_smirnov()?);
        }
        if self.runs {
            let mut options = Vec::new();
            if self.runs_list {
                let list = values(&self.runs_values, string)?;
                if list.is_empty() {
                    return Err(t("stats-nptests-type-runs-values").into());
                }
                options.push(format!("GROUPCATEGORICAL=LIST({list})"));
            }
            match self.runs_cut {
                0 => {}
                2 => {
                    let cut = parse(&self.runs_cut_value, t("stats-nptests-what-runs-cut"))?;
                    options.push(format!("GROUPCONTINUOUS=CUTPOINT({})", number(cut)));
                }
                i => options.push(format!("GROUPCONTINUOUS=CUTPOINT({})", RUNS_CUTS[i].0)),
            }
            tests.push(with_options("RUNS", &options));
        }
        if self.wilcoxon {
            let median = parse(
                &self.test_median,
                t("stats-nptests-what-hypothesized-median"),
            )?;
            tests.push(format!("WILCOXON(TESTVALUE={})", number(median)));
        }
        Ok(tests)
    }

    fn kolmogorov_smirnov(&self) -> Result<String, String> {
        let [normal, uniform, exponential, poisson] = self.distributions;
        if !self.distributions.contains(&true) {
            return Err(t("stats-nptests-choose-distribution").into());
        }
        let mut options = Vec::new();
        if let Some(n) = whole(&self.samples, 1, t("stats-nptests-what-mc-samples"))? {
            if n > 1_000_000_000 {
                return Err(t("stats-nptests-mc-samples-at-most").into());
            }
            if n != 1000 {
                options.push(format!("NSAMPLES={n}"));
            }
        }
        if !self.mc_level.trim().is_empty() {
            let level = parse(&self.mc_level, t("stats-nptests-what-mc-level"))?;
            if !(level > 0.0 && level < 100.0) {
                return Err(t("stats-nptests-mc-level-between").into());
            }
            if level != 99.0 {
                options.push(format!("MC_CILEVEL={}", number(level)));
            }
        }
        let positive = |text: &str, what: &str| -> Result<String, String> {
            let n = parse(text, what)?;
            if n <= 0.0 {
                return Err(t_args(
                    "stats-nptests-must-be-above-zero",
                    &[("what", what.into())],
                ));
            }
            Ok(number(n))
        };
        if normal {
            if self.normal_custom {
                let mean = parse(&self.normal_mean, t("stats-nptests-what-normal-mean"))?;
                let sd = positive(&self.normal_sd, t("stats-nptests-what-standard-deviation"))?;
                options.push(format!("NORMAL=CUSTOM(MEAN={} SD={sd})", number(mean)));
            } else if self.lilliefors {
                options.push("NORMAL=SAMPLE(SIMULATION=FALSE)".into());
            } else if uniform || exponential || poisson {
                // alone, the normal distribution with estimated parameters is the default
                options.push("NORMAL=SAMPLE".into());
            }
        }
        if uniform {
            if self.uniform_custom {
                let min = parse(&self.uniform_min, t("stats-nptests-what-uniform-min"))?;
                let max = parse(&self.uniform_max, t("stats-nptests-what-uniform-max"))?;
                if min >= max {
                    return Err(t("stats-nptests-uniform-min-below-max").into());
                }
                options.push(format!(
                    "UNIFORM=CUSTOM(MIN={} MAX={})",
                    number(min),
                    number(max)
                ));
            } else {
                options.push("UNIFORM=SAMPLE".into());
            }
        }
        if exponential {
            if self.exponential_custom {
                let mean = positive(
                    &self.exponential_mean,
                    t("stats-nptests-what-exponential-mean"),
                )?;
                options.push(format!("EXPONENTIAL=CUSTOM(MEAN={mean})"));
            } else {
                options.push("EXPONENTIAL=SAMPLE".into());
            }
        }
        if poisson {
            let mean = positive(&self.poisson_mean, t("stats-nptests-what-poisson-mean"))?;
            options.push(format!("POISSON=CUSTOM(MEAN={mean})"));
        }
        Ok(with_options("KOLMOGOROV_SMIRNOV", &options))
    }

    /// `COMPARE=...` of the k-sample test `i` when it is not the default.
    fn compare(&self, i: usize) -> Option<String> {
        (self.compare[i] > 0).then(|| format!("COMPARE={}", NP_COMPARE[self.compare[i]].0))
    }

    fn independent(&self) -> Result<Vec<String>, String> {
        let mut tests: Vec<String> = keywords(&INDEPENDENT_TWO, &self.two_tests)
            .into_iter()
            .map(String::from)
            .collect();
        if self.moses {
            let mut options = Vec::new();
            if let Some(n) = whole(&self.trim, 0, t("stats-nptests-what-cases-to-trim"))? {
                options.push(format!("TRIMOUTLIERS={n}"));
            }
            tests.push(with_options("MOSES", &options));
        }
        for (i, (keyword, _)) in INDEPENDENT_K.iter().enumerate() {
            if !self.k_tests[i] {
                continue;
            }
            let mut options = Vec::new();
            if *keyword == "MEDIAN" && !self.median_value.trim().is_empty() {
                let median = parse(
                    &self.median_value,
                    t("stats-nptests-what-median-to-compare"),
                )?;
                options.push(format!("TESTVALUE={}", number(median)));
            }
            if *keyword == "JONCKHEERE_TERPSTRA" && self.descending {
                options.push("ORDER=DESCENDING".into());
            }
            options.extend(self.compare(i));
            tests.push(with_options(keyword, &options));
        }
        Ok(tests)
    }

    fn related(&self, string: bool) -> Result<Vec<String>, String> {
        let success = if self.success_list && (self.mcnemar || self.k_tests[0]) {
            Some(format!("SUCCESS={}", self.success_list(string)?))
        } else {
            None
        };
        if (self.mcnemar || self.two_tests.contains(&true)) && self.fields.len() != 2 {
            return Err(t("stats-nptests-exactly-two-fields").into());
        }
        let mut tests = Vec::new();
        if self.mcnemar {
            tests.push(with_options("MCNEMAR", success.as_slice()));
        }
        tests.extend(
            keywords(&RELATED_TWO, &self.two_tests)
                .into_iter()
                .map(String::from),
        );
        for (i, (keyword, _)) in RELATED_K.iter().enumerate() {
            if !self.k_tests[i] {
                continue;
            }
            let mut options = Vec::new();
            if i == 0 {
                options.extend(success.clone());
            }
            options.extend(self.compare(i));
            tests.push(with_options(keyword, &options));
        }
        Ok(tests)
    }
}

/// A test keyword with its settings in parentheses, or alone without any.
fn with_options(keyword: &str, options: &[String]) -> String {
    if options.is_empty() {
        keyword.to_string()
    } else {
        format!("{keyword}({})", options.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    fn base(kind: NpKind, fields: &[&str]) -> Nptests {
        Nptests {
            fields: v(fields),
            group: "region".into(),
            ..Nptests::new(kind)
        }
    }

    #[test]
    fn defaults_write_the_bare_subcommand() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let mut d = base(NpKind::OneSample, &["age"]);
        assert_eq!(d.syntax(&data).unwrap(), "NPTESTS /ONESAMPLE TEST(age).");
        // typed defaults write nothing, and settings of tests left off are not checked
        d.alpha = "0.05".into();
        d.ci_level = "95".into();
        d.proportion = "2".into();
        d.test_median = "x".into();
        assert_eq!(d.syntax(&data).unwrap(), "NPTESTS /ONESAMPLE TEST(age).");
        d.customize = true;
        d.ks = true;
        d.samples = "1000".into();
        d.mc_level = "99".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "NPTESTS /ONESAMPLE TEST(age)\n    KOLMOGOROV_SMIRNOV."
        );
        let d = base(NpKind::Independent, &["age"]);
        assert_eq!(
            d.syntax(&data).unwrap(),
            "NPTESTS /INDEPENDENT TEST(age) GROUP(region)."
        );
    }

    #[test]
    fn the_dialog_refuses_what_the_command_would() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let e = |d: &Nptests| d.syntax(&data).unwrap_err();
        let mut d = base(NpKind::OneSample, &[]);
        assert_eq!(e(&d), "add at least one variable to Test fields");
        d.fields = v(&["age"]);
        d.alpha = "1".into();
        assert_eq!(
            e(&d),
            "the significance level must be a number between 0 and 1"
        );
        d.alpha.clear();
        d.ci_level = "100".into();
        assert_eq!(
            e(&d),
            "the confidence level must be a percentage between 0 and 100"
        );
        d.ci_level.clear();
        d.customize = true;
        assert_eq!(e(&d), "choose a test, or let the data choose them");
        d.binomial = true;
        d.proportion = "1.5".into();
        assert_eq!(e(&d), "the test proportion must lie between 0 and 1");
        d.proportion.clear();
        d.success_list = true;
        assert_eq!(e(&d), "type the values that count as a success");
        d.success_list = false;
        d.binomial = false;
        d.wilcoxon = true;
        assert_eq!(e(&d), "the hypothesized median must be a number");
        d.wilcoxon = false;
        d.chisquare = true;
        d.expected_custom = true;
        d.categories = "1 2 3".into();
        d.frequencies = "1 2".into();
        assert_eq!(e(&d), "2 expected frequencies for 3 categories");
        d.frequencies = "1 0 2".into();
        assert_eq!(e(&d), "expected frequencies must be above 0");
        d.chisquare = false;
        d.ks = true;
        d.distributions = [false; 4];
        assert_eq!(
            e(&d),
            "choose a distribution for the Kolmogorov-Smirnov test"
        );
        d.distributions = [false, true, false, true];
        d.uniform_custom = true;
        d.uniform_min = "5".into();
        d.uniform_max = "1".into();
        assert_eq!(e(&d), "the uniform minimum must be below its maximum");
        d.uniform_custom = false;
        assert_eq!(e(&d), "the Poisson mean must be a number");
        d.poisson_mean = "-1".into();
        assert_eq!(e(&d), "the Poisson mean must be above 0");

        let mut d = base(NpKind::Independent, &["age"]);
        d.group.clear();
        assert_eq!(e(&d), "set the grouping variable");
        d.group = "monthly_spend".into();
        let scale = data
            .variable("monthly_spend")
            .is_ok_and(|v| v.measure() == Measure::Scale);
        if scale {
            assert!(e(&d).contains("is a scale variable"));
        }
        d.group = "region".into();
        d.customize = true;
        d.moses = true;
        d.trim = "-1".into();
        assert_eq!(
            e(&d),
            "the cases to trim must be a whole number of at least 0"
        );

        let mut d = base(NpKind::Related, &["q5_1"]);
        assert_eq!(e(&d), "add at least two variables to Test fields");
        d.fields = v(&["q5_1", "q5_2", "q5_3"]);
        d.customize = true;
        d.two_tests[1] = true;
        assert_eq!(
            e(&d),
            "McNemar and the tests of two fields need exactly two fields"
        );
        d.two_tests[1] = false;
        d.k_tests[0] = true;
        d.success_list = true;
        assert_eq!(e(&d), "type the values that count as a success");
        // the success values count only for McNemar and Cochran
        d.k_tests = [false, true, false];
        assert!(d.syntax(&data).is_ok());
    }

    #[test]
    fn string_fields_take_quoted_values() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let mut d = base(NpKind::OneSample, &["plan"]);
        d.customize = true;
        d.chisquare = true;
        d.expected_custom = true;
        d.categories = "basic, pro".into();
        d.frequencies = "3 1".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "NPTESTS /ONESAMPLE TEST(plan)\n    \
             CHISQUARE(EXPECTED=CUSTOM(FREQUENCIES=3 1 CATEGORIES='basic' 'pro'))."
        );
    }
}
