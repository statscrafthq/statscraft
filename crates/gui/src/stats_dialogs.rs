//! The Linear Regression, Reliability Analysis, Nonparametric Tests and Partial Correlations
//! dialogs. Options left as they are add nothing to the syntax, so the command runs with its own
//! defaults.

use crate::analyze_dialogs::keywords;
use crate::syntax_text::{names, number, quote, value};
use statscraft_data::Dataset;

/// An error naming the first string variable among `list`: these commands take numbers only.
pub fn numeric<'a>(
    data: &Dataset,
    list: impl IntoIterator<Item = &'a String>,
) -> Result<(), String> {
    let mut list = list.into_iter();
    match list.find(|n| data.variable(n).is_ok_and(|v| v.is_string())) {
        Some(name) => Err(format!("{name} is a string; the command needs numbers")),
        None => Ok(()),
    }
}

/// A percentage the user typed, above 0 and below 100.
fn percent(text: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|p| *p > 0.0 && *p < 100.0)
        .ok_or_else(|| "the confidence level must be a percentage between 0 and 100".into())
}

/// A number the user typed, or an error naming the field.
fn parse(text: &str, what: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{what} must be a number"))
}

/// Keyword and label of every method of a block.
pub const REGRESSION_METHODS: [(&str, &str); 5] = [
    ("ENTER", "Enter"),
    ("STEPWISE", "Stepwise"),
    ("REMOVE", "Remove"),
    ("BACKWARD", "Backward"),
    ("FORWARD", "Forward"),
];

/// Keywords and label of every statistic on /STATISTICS besides the confidence intervals.
pub const REGRESSION_STATISTICS: [(&str, &str); 5] = [
    ("COEFF OUTS", "Estimates"),
    ("R ANOVA", "Model fit"),
    ("CHANGE", "R squared change"),
    ("ZPP", "Part and partial correlations"),
    ("COLLIN TOL", "Collinearity diagnostics"),
];

/// Estimates and model fit, what REGRESSION shows without /STATISTICS.
const REGRESSION_DEFAULT_STATISTICS: [bool; 5] = [true, true, false, false, false];

/// Keyword and label of every variable REGRESSION saves.
pub const REGRESSION_SAVE: [(&str, &str); 17] = [
    ("PRED", "Unstandardized"),
    ("ZPRED", "Standardized"),
    ("ADJPRED", "Adjusted"),
    ("SEPRED", "Standard error of the mean"),
    ("RESID", "Unstandardized"),
    ("ZRESID", "Standardized"),
    ("SRESID", "Studentized"),
    ("DRESID", "Deleted"),
    ("SDRESID", "Studentized deleted"),
    ("MAHAL", "Mahalanobis"),
    ("COOK", "Cook's"),
    ("LEVER", "Leverage values"),
    ("DFBETA", "DfBeta"),
    ("SDBETA", "Standardized DfBeta"),
    ("DFFIT", "DfFit"),
    ("SDFIT", "Standardized DfFit"),
    ("COVRATIO", "Covariance ratio"),
];

/// The groups of `REGRESSION_SAVE`: title and the range of its entries.
pub const REGRESSION_SAVE_GROUPS: [(&str, std::ops::Range<usize>); 4] = [
    ("Predicted values", 0..4),
    ("Residuals", 4..9),
    ("Distances", 9..12),
    ("Influence statistics", 12..17),
];

/// Keyword and label of every way REGRESSION treats missing values.
pub const REGRESSION_MISSING: [(&str, &str); 3] = [
    ("LISTWISE", "Leave out a case missing on any variable"),
    ("PAIRWISE", "Use the valid cases of each pair of variables"),
    ("MEANSUBSTITUTION", "Replace a missing value with the mean"),
];

/// The independent variables of one block and the method that enters or removes them.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Block {
    pub variables: Vec<String>,
    /// Index into `REGRESSION_METHODS`.
    pub method: usize,
}

/// REGRESSION: one dependent variable and blocks of independent variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Regression {
    pub dependent: String,
    pub blocks: Vec<Block>,
    /// The block the dialog shows.
    pub block: usize,
    /// One flag per entry of `REGRESSION_STATISTICS`.
    pub statistics: [bool; 5],
    pub ci: bool,
    /// Confidence level of the coefficients in percent.
    pub ci_level: String,
    pub descriptives: bool,
    /// One flag per entry of `REGRESSION_SAVE`.
    pub save: [bool; 17],
    /// The stepping criteria are values of F instead of their probabilities.
    pub use_f: bool,
    pub entry: String,
    pub removal: String,
    pub constant: bool,
    /// Index into `REGRESSION_MISSING`.
    pub missing: usize,
}

impl Default for Regression {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            blocks: vec![Block::default()],
            block: 0,
            statistics: REGRESSION_DEFAULT_STATISTICS,
            ci: false,
            ci_level: "95".into(),
            descriptives: false,
            save: [false; 17],
            use_f: false,
            entry: "0.05".into(),
            removal: "0.10".into(),
            constant: true,
            missing: 0,
        }
    }
}

impl Regression {
    /// Switches the stepping criteria between probabilities and values of F, at their defaults.
    pub fn set_use_f(&mut self, use_f: bool) {
        self.use_f = use_f;
        let (entry, removal) = if use_f {
            ("3.84", "2.71")
        } else {
            ("0.05", "0.10")
        };
        self.entry = entry.into();
        self.removal = removal.into();
    }

    pub fn variables(&self) -> Vec<&String> {
        let blocks = self.blocks.iter().flat_map(|b| &b.variables);
        std::iter::once(&self.dependent).chain(blocks).collect()
    }

    pub fn syntax(&self) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        let blocks: Vec<&Block> = self
            .blocks
            .iter()
            .filter(|b| !b.variables.is_empty())
            .collect();
        let Some(first) = blocks.first() else {
            return Err("add at least one variable to Independent variables".into());
        };
        if REGRESSION_METHODS[first.method].0 == "REMOVE" {
            return Err("Remove takes out variables entered in an earlier block".into());
        }
        if blocks
            .iter()
            .any(|b| b.variables.iter().any(|v| v == dependent))
        {
            return Err(format!("{dependent} is the dependent variable"));
        }
        let mut text = String::from("REGRESSION");
        if self.descriptives {
            text.push_str("\n  /DESCRIPTIVES=MEAN STDDEV CORR SIG N");
        }
        if self.missing > 0 {
            text.push_str(&format!(
                "\n  /MISSING={}",
                REGRESSION_MISSING[self.missing].0
            ));
        }
        if self.statistics != REGRESSION_DEFAULT_STATISTICS || self.ci {
            let mut statistics = Vec::new();
            for (i, ((keyword, _), on)) in REGRESSION_STATISTICS
                .iter()
                .zip(self.statistics)
                .enumerate()
            {
                if on {
                    statistics.push(keyword.to_string());
                }
                if i == 0 && self.ci {
                    statistics.push(format!("CI({})", number(percent(&self.ci_level)?)));
                }
            }
            if statistics.is_empty() {
                return Err("choose at least one statistic".into());
            }
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        let entry = parse(&self.entry, "the entry value")?;
        let removal = parse(&self.removal, "the removal value")?;
        if self.use_f {
            if !(removal >= 0.0 && entry > removal) {
                return Err(
                    "the F to enter must be above the F to remove, which is 0 or more".into(),
                );
            }
            text.push_str(&format!(
                "\n  /CRITERIA=FIN({}) FOUT({})",
                number(entry),
                number(removal)
            ));
        } else {
            if !(entry > 0.0 && entry < removal && removal <= 1.0) {
                return Err(
                    "the probability to enter must be above 0 and below the probability to remove, which is at most 1"
                        .into(),
                );
            }
            if (entry, removal) != (0.05, 0.10) {
                text.push_str(&format!(
                    "\n  /CRITERIA=PIN({}) POUT({})",
                    number(entry),
                    number(removal)
                ));
            }
        }
        if !self.constant {
            text.push_str("\n  /ORIGIN");
        }
        text.push_str(&format!("\n  /DEPENDENT={dependent}"));
        for b in blocks {
            let method = REGRESSION_METHODS[b.method].0;
            text.push_str(&format!("\n  /METHOD={method} {}", b.variables.join(" ")));
        }
        let save = keywords(&REGRESSION_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of every model of a scale.
pub const RELIABILITY_MODELS: [(&str, &str); 5] = [
    ("ALPHA", "Alpha"),
    ("SPLIT", "Split-half"),
    ("GUTTMAN", "Guttman"),
    ("PARALLEL", "Parallel"),
    ("STRICTPARALLEL", "Strict parallel"),
];

/// Keyword and label of the descriptives: the first two on /STATISTICS, the last on /SUMMARY.
pub const RELIABILITY_DESCRIPTIVES: [(&str, &str); 3] = [
    ("DESCRIPTIVE", "Item"),
    ("SCALE", "Scale"),
    ("TOTAL", "Scale if item deleted"),
];

pub const RELIABILITY_INTER_ITEM: [(&str, &str); 2] = [
    ("CORRELATIONS", "Correlations"),
    ("COVARIANCES", "Covariances"),
];

pub const RELIABILITY_SUMMARIES: [(&str, &str); 4] = [
    ("MEANS", "Means"),
    ("VARIANCE", "Variances"),
    ("COVARIANCES", "Covariances"),
    ("CORRELATIONS", "Correlations"),
];

/// Keywords and label of every test of the ANOVA table.
pub const RELIABILITY_ANOVA: [(&str, &str); 4] = [
    ("", "None"),
    ("ANOVA", "F test"),
    ("ANOVA FRIEDMAN", "Friedman chi-square"),
    ("ANOVA COCHRAN", "Cochran chi-square"),
];

/// Keyword and label of every model of the intraclass correlation.
pub const ICC_MODELS: [(&str, &str); 3] = [
    ("MIXED", "Two-way mixed"),
    ("RANDOM", "Two-way random"),
    ("ONEWAY", "One-way random"),
];

/// RELIABILITY of one scale made of all the items.
#[derive(Clone, PartialEq, Debug)]
pub struct Reliability {
    pub items: Vec<String>,
    /// Index into `RELIABILITY_MODELS`.
    pub model: usize,
    pub label: String,
    /// One flag per entry of `RELIABILITY_DESCRIPTIVES`.
    pub descriptives: [bool; 3],
    /// One flag per entry of `RELIABILITY_INTER_ITEM`.
    pub inter_item: [bool; 2],
    /// One flag per entry of `RELIABILITY_SUMMARIES`.
    pub summaries: [bool; 4],
    /// Index into `RELIABILITY_ANOVA`.
    pub anova: usize,
    pub hotelling: bool,
    pub tukey: bool,
    pub icc: bool,
    /// Index into `ICC_MODELS`.
    pub icc_model: usize,
    /// Absolute agreement instead of consistency; not for the one-way model.
    pub icc_absolute: bool,
    /// Confidence level in percent.
    pub icc_level: String,
    pub icc_test_value: String,
}

impl Default for Reliability {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            model: 0,
            label: String::new(),
            descriptives: [false; 3],
            inter_item: [false; 2],
            summaries: [false; 4],
            anova: 0,
            hotelling: false,
            tukey: false,
            icc: false,
            icc_model: 0,
            icc_absolute: false,
            icc_level: "95".into(),
            icc_test_value: "0".into(),
        }
    }
}

impl Reliability {
    pub fn syntax(&self) -> Result<String, String> {
        if self.items.len() < 2 {
            return Err("add at least two items".into());
        }
        let mut text = format!("RELIABILITY /VARIABLES={}", self.items.join(" "));
        let label = self.label.trim();
        if !label.is_empty() {
            text.push_str(&format!("\n  /SCALE({})=ALL", quote(label)));
        }
        if self.model > 0 {
            text.push_str(&format!("\n  /MODEL={}", RELIABILITY_MODELS[self.model].0));
        }
        let mut statistics = keywords(&RELIABILITY_DESCRIPTIVES[..2], &self.descriptives[..2]);
        statistics.extend(keywords(&RELIABILITY_INTER_ITEM, &self.inter_item));
        if self.anova > 0 {
            statistics.push(RELIABILITY_ANOVA[self.anova].0);
        }
        statistics.extend(self.tukey.then_some("TUKEY"));
        statistics.extend(self.hotelling.then_some("HOTELLING"));
        if !statistics.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        let mut summary: Vec<&str> = self.descriptives[2]
            .then_some("TOTAL")
            .into_iter()
            .collect();
        summary.extend(keywords(&RELIABILITY_SUMMARIES, &self.summaries));
        if !summary.is_empty() {
            text.push_str(&format!("\n  /SUMMARY={}", summary.join(" ")));
        }
        if self.icc {
            let mut icc = String::from("\n  /ICC");
            let model = ICC_MODELS[self.icc_model].0;
            let mut parts = Vec::new();
            if model != "MIXED" {
                parts.push(format!("MODEL({model})"));
            }
            if self.icc_absolute && model != "ONEWAY" {
                parts.push("TYPE(ABSOLUTE)".into());
            }
            let level = percent(&self.icc_level)?;
            if level != 95.0 {
                parts.push(format!("CIN={}", number(level)));
            }
            let test = parse(&self.icc_test_value, "the test value")?;
            if !(0.0..1.0).contains(&test) {
                return Err("the test value must be from 0 up to 1".into());
            }
            if test != 0.0 {
                parts.push(format!("TESTVAL={}", number(test)));
            }
            if !parts.is_empty() {
                icc.push_str(&format!("={}", parts.join(" ")));
            }
            text.push_str(&icc);
        }
        text.push('.');
        Ok(text)
    }
}

/// The legacy dialogs of the nonparametric tests.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NparKind {
    ChiSquare,
    Binomial,
    Runs,
    OneSampleKs,
    TwoIndependent,
    KIndependent,
    TwoRelated,
    KRelated,
}

impl NparKind {
    pub const ALL: [NparKind; 8] = [
        NparKind::ChiSquare,
        NparKind::Binomial,
        NparKind::Runs,
        NparKind::OneSampleKs,
        NparKind::TwoIndependent,
        NparKind::KIndependent,
        NparKind::TwoRelated,
        NparKind::KRelated,
    ];

    pub fn menu(self) -> &'static str {
        match self {
            NparKind::ChiSquare => "Chi-square...",
            NparKind::Binomial => "Binomial...",
            NparKind::Runs => "Runs...",
            NparKind::OneSampleKs => "1-Sample K-S...",
            NparKind::TwoIndependent => "2 Independent Samples...",
            NparKind::KIndependent => "K Independent Samples...",
            NparKind::TwoRelated => "2 Related Samples...",
            NparKind::KRelated => "K Related Samples...",
        }
    }

    /// Subcommand and label of every test of the dialog that has a choice of tests.
    pub fn tests(self) -> &'static [(&'static str, &'static str)] {
        match self {
            NparKind::Runs => &[
                ("RUNS(MEDIAN)", "Median"),
                ("RUNS(MODE)", "Mode"),
                ("RUNS(MEAN)", "Mean"),
            ],
            NparKind::OneSampleKs => &[
                ("K-S(NORMAL)", "Normal"),
                ("K-S(UNIFORM)", "Uniform"),
                ("K-S(POISSON)", "Poisson"),
                ("K-S(EXPONENTIAL)", "Exponential"),
            ],
            NparKind::TwoIndependent => &[
                ("M-W", "Mann-Whitney U"),
                ("K-S", "Kolmogorov-Smirnov Z"),
                ("MOSES", "Moses extreme reactions"),
                ("W-W", "Wald-Wolfowitz runs"),
            ],
            NparKind::KIndependent => &[
                ("K-W", "Kruskal-Wallis H"),
                ("MEDIAN", "Median"),
                ("J-T", "Jonckheere-Terpstra"),
            ],
            NparKind::TwoRelated => &[
                ("WILCOXON", "Wilcoxon"),
                ("SIGN", "Sign"),
                ("MCNEMAR", "McNemar"),
                ("MH", "Marginal homogeneity"),
            ],
            NparKind::KRelated => &[
                ("FRIEDMAN", "Friedman"),
                ("KENDALL", "Kendall's W"),
                ("COCHRAN", "Cochran's Q"),
            ],
            NparKind::ChiSquare | NparKind::Binomial => &[],
        }
    }
}

/// NPAR TESTS from one of the legacy dialogs.
#[derive(Clone, PartialEq, Debug)]
pub struct NparTests {
    pub kind: NparKind,
    pub variables: Vec<String>,
    /// Two related samples.
    pub pairs: Vec<(String, String)>,
    pub grouping: String,
    /// The two groups, or the lowest and highest group of k samples, or the range of the
    /// chi-square categories.
    pub low: String,
    pub high: String,
    /// One flag per entry of `kind.tests()`.
    pub tests: [bool; 4],
    /// Chi-square: the categories are the whole numbers of `low` to `high`.
    pub range: bool,
    /// Chi-square: the expected frequencies, all equal when false.
    pub expected_given: bool,
    pub expected: String,
    /// Binomial: the test proportion of the first category.
    pub proportion: String,
    /// Binomial: the cut point that makes the two categories; Runs: a cut point of its own.
    pub cut: bool,
    pub cut_value: String,
    pub descriptives: bool,
    pub quartiles: bool,
    pub listwise: bool,
}

impl NparTests {
    pub fn new(kind: NparKind) -> Self {
        Self {
            kind,
            variables: Vec::new(),
            pairs: Vec::new(),
            grouping: String::new(),
            low: String::new(),
            high: String::new(),
            tests: [true, false, false, false],
            range: false,
            expected_given: false,
            expected: String::new(),
            proportion: "0.5".into(),
            cut: false,
            cut_value: String::new(),
            descriptives: false,
            quartiles: false,
            listwise: false,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            NparKind::ChiSquare => "Chi-square Test",
            NparKind::Binomial => "Binomial Test",
            NparKind::Runs => "Runs Test",
            NparKind::OneSampleKs => "One-Sample Kolmogorov-Smirnov Test",
            NparKind::TwoIndependent => "Two Independent Samples Tests",
            NparKind::KIndependent => "Several Independent Samples Tests",
            NparKind::TwoRelated => "Two Related Samples Tests",
            NparKind::KRelated => "Several Related Samples Tests",
        }
    }

    pub fn variables(&self) -> Vec<&String> {
        let pairs = self.pairs.iter().flat_map(|(a, b)| [a, b]);
        self.variables
            .iter()
            .chain(pairs)
            .chain(std::iter::once(&self.grouping))
            .collect()
    }

    pub fn syntax(&self) -> Result<String, String> {
        let chosen = keywords(self.kind.tests(), &self.tests);
        let mut tests: Vec<String> = Vec::new();
        match self.kind {
            NparKind::TwoRelated => {
                if self.pairs.is_empty() {
                    return Err("add at least one pair".into());
                }
                let (left, right): (Vec<&str>, Vec<&str>) = self
                    .pairs
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .unzip();
                let (left, right) = (left.join(" "), right.join(" "));
                for test in chosen {
                    tests.push(format!("/{test}={left} WITH {right} (PAIRED)"));
                }
            }
            NparKind::KRelated => {
                if self.variables.len() < 2 {
                    return Err("add at least two variables".into());
                }
                let list = self.variables.join(" ");
                for test in chosen {
                    tests.push(format!("/{test}={list}"));
                }
            }
            kind => {
                let list = names(&self.variables, "Test variables")?;
                match kind {
                    NparKind::ChiSquare => {
                        let mut range = String::new();
                        if self.range {
                            let (low, high) = (
                                parse(&self.low, "the lower bound")?,
                                parse(&self.high, "the upper bound")?,
                            );
                            if low.trunc() >= high.trunc() {
                                return Err("the lower bound must be below the upper bound".into());
                            }
                            range = format!(" ({}, {})", number(low), number(high));
                        }
                        tests.push(format!("/CHISQUARE={list}{range}"));
                        if self.expected_given {
                            let values = self
                                .expected
                                .split([' ', ','])
                                .filter(|t| !t.is_empty())
                                .map(|t| value(t, false))
                                .collect::<Result<Vec<_>, _>>()?;
                            if values.is_empty() {
                                return Err("give the expected values".into());
                            }
                            tests.push(format!("/EXPECTED={}", values.join(" ")));
                        }
                    }
                    NparKind::Binomial => {
                        let p = parse(&self.proportion, "the test proportion")?;
                        if !(p > 0.0 && p < 1.0) {
                            return Err("the test proportion must lie between 0 and 1".into());
                        }
                        let p = if p == 0.5 {
                            String::new()
                        } else {
                            format!("({})", number(p))
                        };
                        let cut = if self.cut {
                            format!(" ({})", value(&self.cut_value, false)?)
                        } else {
                            String::new()
                        };
                        tests.push(format!("/BINOMIAL{p}={list}{cut}"));
                    }
                    NparKind::TwoIndependent | NparKind::KIndependent => {
                        let grouping = self.grouping.trim();
                        if grouping.is_empty() {
                            return Err("choose a grouping variable".into());
                        }
                        let two = kind == NparKind::TwoIndependent;
                        let missing = if two {
                            "give the values of both groups"
                        } else {
                            "give the lowest and the highest group"
                        };
                        if self.low.trim().is_empty() || self.high.trim().is_empty() {
                            return Err(missing.into());
                        }
                        let (a, b) = (parse(&self.low, "a group")?, parse(&self.high, "a group")?);
                        if two && a == b {
                            return Err("the two groups need different values".into());
                        }
                        if !two && a >= b {
                            return Err("the lowest group must be below the highest".into());
                        }
                        let (low, high) = (number(a), number(b));
                        for test in chosen {
                            tests.push(format!("/{test}={list} BY {grouping}({low}, {high})"));
                        }
                    }
                    _ => {
                        // Runs and one-sample K-S: one subcommand per cut point or distribution
                        for test in chosen {
                            tests.push(format!("/{test}={list}"));
                        }
                        if kind == NparKind::Runs && self.cut {
                            let cut = value(&self.cut_value, false)?;
                            tests.push(format!("/RUNS({cut})={list}"));
                        }
                    }
                }
            }
        }
        if tests.is_empty() {
            return Err("choose at least one test".into());
        }
        let mut text = format!("NPAR TESTS {}", tests.join("\n  "));
        let statistics: Vec<&str> = [
            (self.descriptives, "DESCRIPTIVES"),
            (self.quartiles, "QUARTILES"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !statistics.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        if self.listwise {
            text.push_str("\n  /MISSING=LISTWISE");
        }
        text.push('.');
        Ok(text)
    }
}

/// PARTIAL CORR of the variables controlling for others.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PartialCorr {
    pub variables: Vec<String>,
    pub controls: Vec<String>,
    pub one_tailed: bool,
    /// The coefficients alone, the significant ones marked with stars.
    pub condensed: bool,
    pub descriptives: bool,
    pub zero_order: bool,
    /// Each zero-order correlation from the cases valid on its pair.
    pub pairwise: bool,
}

impl PartialCorr {
    pub fn syntax(&self) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        let controls = names(&self.controls, "Controlling for")?;
        if let Some(name) = self.controls.iter().find(|c| self.variables.contains(c)) {
            return Err(format!("{name} is both a variable and a control"));
        }
        let mut text = format!(
            "PARTIAL CORR /VARIABLES={} BY {controls}",
            self.variables.join(" ")
        );
        if self.one_tailed {
            text.push_str("\n  /SIGNIFICANCE=ONETAIL");
        }
        let statistics: Vec<&str> = [
            (self.descriptives, "DESCRIPTIVES"),
            (self.zero_order, "CORR"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !statistics.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        if self.condensed {
            text.push_str("\n  /FORMAT=CONDENSED");
        }
        if self.pairwise {
            text.push_str("\n  /MISSING=ANALYSIS");
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, runs, v};

    #[test]
    fn regression() {
        let mut d = Regression::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the dependent variable");
        d.dependent = "monthly_spend".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Independent variables"
        );
        d.blocks[0].variables = v(&["age", "q2_recommend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "REGRESSION\n  /DEPENDENT=monthly_spend\n  /METHOD=ENTER age q2_recommend."
        );
        assert!(runs(&syntax) >= 3);

        // a second block, stepwise, an empty third block left out, every statistic
        d.blocks.push(Block {
            variables: v(&["q1_satisfaction", "gender"]),
            method: 1,
        });
        d.blocks.push(Block::default());
        d.statistics = [true; 5];
        d.ci = true;
        d.ci_level = "90".into();
        d.descriptives = true;
        d.entry = "0.1".into();
        d.removal = "0.2".into();
        d.missing = 1;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "REGRESSION\n  /DESCRIPTIVES=MEAN STDDEV CORR SIG N\n  /MISSING=PAIRWISE\n  \
             /STATISTICS=COEFF OUTS CI(90) R ANOVA CHANGE ZPP COLLIN TOL\n  \
             /CRITERIA=PIN(0.1) POUT(0.2)\n  /DEPENDENT=monthly_spend\n  \
             /METHOD=ENTER age q2_recommend\n  /METHOD=STEPWISE q1_satisfaction gender."
        );
        assert!(runs(&syntax) >= 5);

        // values of F, no constant, removal in a later block, and every saved variable
        d.statistics = REGRESSION_DEFAULT_STATISTICS;
        d.ci = false;
        d.descriptives = false;
        d.missing = 0;
        d.set_use_f(true);
        d.constant = false;
        d.blocks[1] = Block {
            variables: v(&["age"]),
            method: 2,
        };
        d.save = [true; 17];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "REGRESSION\n  /CRITERIA=FIN(3.84) FOUT(2.71)\n  /ORIGIN\n  /DEPENDENT=monthly_spend\n  \
             /METHOD=ENTER age q2_recommend\n  /METHOD=REMOVE age\n  \
             /SAVE=PRED ZPRED ADJPRED SEPRED RESID ZRESID SRESID DRESID SDRESID MAHAL COOK LEVER \
             DFBETA SDBETA DFFIT SDFIT COVRATIO."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 3);
        let data = session.dataset().unwrap();
        assert!(data.variable("PRE_1").is_ok());
        assert!(data.variable("COO_1").is_ok());

        for (use_f, entry, removal, error) in [
            (
                false,
                "0.2",
                "0.1",
                "the probability to enter must be above 0",
            ),
            (
                true,
                "2",
                "3",
                "the F to enter must be above the F to remove",
            ),
            (false, "x", "0.1", "the entry value must be a number"),
        ] {
            d.use_f = use_f;
            d.entry = entry.into();
            d.removal = removal.into();
            assert!(d.syntax().unwrap_err().starts_with(error));
        }
        d.set_use_f(false);
        d.blocks[0].method = 2;
        assert_eq!(
            d.syntax().unwrap_err(),
            "Remove takes out variables entered in an earlier block"
        );
        d.blocks[0].method = 0;
        d.blocks[1].variables.push("monthly_spend".into());
        assert_eq!(
            d.syntax().unwrap_err(),
            "monthly_spend is the dependent variable"
        );
        d.blocks[1].variables.pop();
        d.statistics = [false; 5];
        assert_eq!(d.syntax().unwrap_err(), "choose at least one statistic");
    }

    #[test]
    fn reliability() {
        let mut d = Reliability {
            items: v(&["q5_1"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "add at least two items");
        d.items = v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "RELIABILITY /VARIABLES=q5_1 q5_2 q5_3 q5_4 q5_5.");
        assert!(runs(&syntax) >= 2);

        d.label = "Channel's use".into();
        d.model = 1;
        d.descriptives = [true; 3];
        d.inter_item = [true; 2];
        d.summaries = [true; 4];
        d.anova = 3;
        d.tukey = true;
        d.hotelling = true;
        d.icc = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "RELIABILITY /VARIABLES=q5_1 q5_2 q5_3 q5_4 q5_5\n  /SCALE('Channel''s use')=ALL\n  \
             /MODEL=SPLIT\n  /STATISTICS=DESCRIPTIVE SCALE CORRELATIONS COVARIANCES ANOVA COCHRAN \
             TUKEY HOTELLING\n  /SUMMARY=TOTAL MEANS VARIANCE COVARIANCES CORRELATIONS\n  /ICC."
        );
        assert!(runs(&syntax) >= 10);

        d.label.clear();
        d.model = 4;
        d.descriptives = [false; 3];
        d.inter_item = [false; 2];
        d.summaries = [false; 4];
        d.anova = 2;
        d.tukey = false;
        d.hotelling = false;
        d.icc_model = 1;
        d.icc_absolute = true;
        d.icc_level = "90".into();
        d.icc_test_value = "0.5".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "RELIABILITY /VARIABLES=q5_1 q5_2 q5_3 q5_4 q5_5\n  /MODEL=STRICTPARALLEL\n  \
             /STATISTICS=ANOVA FRIEDMAN\n  /ICC=MODEL(RANDOM) TYPE(ABSOLUTE) CIN=90 TESTVAL=0.5."
        );
        assert!(runs(&syntax) >= 4);
        // the one-way model has no type
        d.icc_model = 2;
        d.model = 2;
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("/ICC=MODEL(ONEWAY) CIN=90 TESTVAL=0.5."));
        assert!(runs(&syntax) >= 4);
        d.icc_test_value = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the test value must be from 0 up to 1"
        );
        d.icc_level = "100".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the confidence level must be a percentage between 0 and 100"
        );
    }

    #[test]
    fn one_sample_nonparametric_tests() {
        let mut d = NparTests::new(NparKind::ChiSquare);
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Test variables"
        );
        d.variables = v(&["region"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "NPAR TESTS /CHISQUARE=region.");
        assert!(runs(&syntax) >= 2);
        d.variables = v(&["q1_satisfaction"]);
        d.range = true;
        d.low = "1".into();
        d.high = "5".into();
        d.expected_given = true;
        d.expected = "1, 2 3 3 1".into();
        d.descriptives = true;
        d.quartiles = true;
        d.listwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /CHISQUARE=q1_satisfaction (1, 5)\n  /EXPECTED=1 2 3 3 1\n  \
             /STATISTICS=DESCRIPTIVES QUARTILES\n  /MISSING=LISTWISE."
        );
        assert!(runs(&syntax) >= 3);
        d.expected.clear();
        assert_eq!(d.syntax().unwrap_err(), "give the expected values");
        d.high = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the lower bound must be below the upper bound"
        );

        let mut d = NparTests::new(NparKind::Binomial);
        d.variables = v(&["q5_1", "q5_2"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "NPAR TESTS /BINOMIAL=q5_1 q5_2.");
        assert!(runs(&syntax) >= 1);
        d.variables = v(&["age"]);
        d.proportion = "0.3".into();
        d.cut = true;
        d.cut_value = "40".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "NPAR TESTS /BINOMIAL(0.3)=age (40).");
        assert!(runs(&syntax) >= 1);
        d.proportion = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the test proportion must lie between 0 and 1"
        );

        let mut d = NparTests::new(NparKind::Runs);
        d.variables = v(&["age"]);
        d.tests = [true, true, true, false];
        d.cut = true;
        d.cut_value = "50".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /RUNS(MEDIAN)=age\n  /RUNS(MODE)=age\n  /RUNS(MEAN)=age\n  /RUNS(50)=age."
        );
        assert!(runs(&syntax) >= 4);
        d.tests = [false; 4];
        d.cut = false;
        assert_eq!(d.syntax().unwrap_err(), "choose at least one test");

        let mut d = NparTests::new(NparKind::OneSampleKs);
        d.variables = v(&["age", "monthly_spend"]);
        d.tests = [true; 4];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /K-S(NORMAL)=age monthly_spend\n  /K-S(UNIFORM)=age monthly_spend\n  \
             /K-S(POISSON)=age monthly_spend\n  /K-S(EXPONENTIAL)=age monthly_spend."
        );
        assert!(runs(&syntax) >= 4);
    }

    #[test]
    fn independent_samples_nonparametric_tests() {
        let mut d = NparTests::new(NparKind::TwoIndependent);
        d.variables = v(&["age", "monthly_spend"]);
        assert_eq!(d.syntax().unwrap_err(), "choose a grouping variable");
        d.grouping = "gender".into();
        assert_eq!(d.syntax().unwrap_err(), "give the values of both groups");
        d.low = "1".into();
        d.high = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the two groups need different values"
        );
        d.high = "2".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "NPAR TESTS /M-W=age monthly_spend BY gender(1, 2).");
        assert!(runs(&syntax) >= 2);
        d.tests = [true; 4];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /M-W=age monthly_spend BY gender(1, 2)\n  \
             /K-S=age monthly_spend BY gender(1, 2)\n  \
             /MOSES=age monthly_spend BY gender(1, 2)\n  \
             /W-W=age monthly_spend BY gender(1, 2)."
        );
        assert!(runs(&syntax) >= 8);

        let mut d = NparTests::new(NparKind::KIndependent);
        d.variables = v(&["age"]);
        d.grouping = "region".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the lowest and the highest group"
        );
        d.low = "5".into();
        d.high = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the lowest group must be below the highest"
        );
        d.low = "1".into();
        d.high = "5".into();
        d.tests = [true, true, true, false];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /K-W=age BY region(1, 5)\n  /MEDIAN=age BY region(1, 5)\n  \
             /J-T=age BY region(1, 5)."
        );
        assert!(runs(&syntax) >= 5);
    }

    #[test]
    fn related_samples_nonparametric_tests() {
        let mut d = NparTests::new(NparKind::TwoRelated);
        assert_eq!(d.syntax().unwrap_err(), "add at least one pair");
        d.pairs = vec![
            ("q5_1".into(), "q5_2".into()),
            ("q5_3".into(), "q5_4".into()),
        ];
        d.tests = [true; 4];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /WILCOXON=q5_1 q5_3 WITH q5_2 q5_4 (PAIRED)\n  \
             /SIGN=q5_1 q5_3 WITH q5_2 q5_4 (PAIRED)\n  \
             /MCNEMAR=q5_1 q5_3 WITH q5_2 q5_4 (PAIRED)\n  \
             /MH=q5_1 q5_3 WITH q5_2 q5_4 (PAIRED)."
        );
        assert!(runs(&syntax) >= 4);

        let mut d = NparTests::new(NparKind::KRelated);
        d.variables = v(&["q5_1"]);
        assert_eq!(d.syntax().unwrap_err(), "add at least two variables");
        d.variables = v(&["q5_1", "q5_2", "q5_3"]);
        d.tests = [true, true, true, false];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NPAR TESTS /FRIEDMAN=q5_1 q5_2 q5_3\n  /KENDALL=q5_1 q5_2 q5_3\n  \
             /COCHRAN=q5_1 q5_2 q5_3."
        );
        assert!(runs(&syntax) >= 3);
    }

    #[test]
    fn partial_correlations() {
        let mut d = PartialCorr {
            variables: v(&["q1_satisfaction"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "add at least two variables");
        d.variables.push("q2_recommend".into());
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Controlling for"
        );
        d.controls = v(&["age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PARTIAL CORR /VARIABLES=q1_satisfaction q2_recommend BY age."
        );
        assert!(runs(&syntax) >= 1);
        d.controls.push("monthly_spend".into());
        d.one_tailed = true;
        d.descriptives = true;
        d.zero_order = true;
        d.condensed = true;
        d.pairwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PARTIAL CORR /VARIABLES=q1_satisfaction q2_recommend BY age monthly_spend\n  \
             /SIGNIFICANCE=ONETAIL\n  /STATISTICS=DESCRIPTIVES CORR\n  /FORMAT=CONDENSED\n  \
             /MISSING=ANALYSIS."
        );
        // the descriptives, and the zero-order and partial correlations in one table
        assert!(runs(&syntax) >= 2);
        d.controls.push("q2_recommend".into());
        assert_eq!(
            d.syntax().unwrap_err(),
            "q2_recommend is both a variable and a control"
        );
    }

    #[test]
    fn string_variables_are_named() {
        let session = open("survey_basic.sav");
        let data = session.dataset().unwrap();
        let mut d = Regression {
            dependent: "age".into(),
            ..Default::default()
        };
        assert!(numeric(data, d.variables()).is_ok());
        d.blocks[0].variables = v(&["plan"]);
        assert_eq!(
            numeric(data, d.variables()).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
        let d = NparTests {
            grouping: "plan".into(),
            ..NparTests::new(NparKind::TwoIndependent)
        };
        assert!(numeric(data, d.variables()).is_err());
    }
}
