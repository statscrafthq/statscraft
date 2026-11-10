//! The Compare Means and Correlate dialogs. Options left as they are add nothing to the syntax,
//! so the command runs with its own defaults.

use crate::syntax_text::{confidence, names, probability, value};

/// Keyword and label of every cell statistic Means offers, in the order shown.
pub const MEANS_CELLS: [(&str, &str); 16] = [
    ("MEAN", "Mean"),
    ("COUNT", "Number of cases"),
    ("STDDEV", "Standard deviation"),
    ("MEDIAN", "Median"),
    ("SEMEAN", "Standard error of the mean"),
    ("SUM", "Sum"),
    ("MIN", "Minimum"),
    ("MAX", "Maximum"),
    ("RANGE", "Range"),
    ("VARIANCE", "Variance"),
    ("SKEW", "Skewness"),
    ("KURT", "Kurtosis"),
    ("NPCT", "Percent of cases"),
    ("SPCT", "Percent of sum"),
    ("HARMONIC", "Harmonic mean"),
    ("GEOMETRIC", "Geometric mean"),
];

/// MEAN COUNT STDDEV, what MEANS shows without /CELLS.
const MEANS_DEFAULT_CELLS: [bool; 16] = {
    let mut cells = [false; 16];
    cells[0] = true;
    cells[1] = true;
    cells[2] = true;
    cells
};

#[derive(Clone, PartialEq, Debug)]
pub struct Means {
    pub dependents: Vec<String>,
    /// The control variables of every layer; each layer is nested in the one before.
    pub layers: Vec<Vec<String>>,
    /// The layer the dialog shows.
    pub layer: usize,
    /// One flag per entry of `MEANS_CELLS`.
    pub cells: [bool; 16],
    pub anova: bool,
    pub linearity: bool,
}

impl Default for Means {
    fn default() -> Self {
        Self {
            dependents: Vec::new(),
            layers: vec![Vec::new()],
            layer: 0,
            cells: MEANS_DEFAULT_CELLS,
            anova: false,
            linearity: false,
        }
    }
}

impl Means {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!(
            "MEANS TABLES={}",
            names(&self.dependents, "Dependent variables")?
        );
        let layers: Vec<&Vec<String>> = self.layers.iter().filter(|l| !l.is_empty()).collect();
        for layer in &layers {
            text.push_str(&format!(" BY {}", layer.join(" ")));
        }
        if self.cells != MEANS_DEFAULT_CELLS {
            let cells: Vec<&str> = MEANS_CELLS
                .iter()
                .zip(self.cells)
                .filter_map(|((keyword, _), on)| on.then_some(*keyword))
                .collect();
            if cells.is_empty() {
                return Err("choose at least one statistic".into());
            }
            text.push_str(&format!("\n  /CELLS={}", cells.join(" ")));
        }
        if self.anova || self.linearity {
            if layers.is_empty() {
                return Err("the analysis of variance needs a variable in Layer 1".into());
            }
            let keyword = if self.linearity { "LINEARITY" } else { "ANOVA" };
            text.push_str(&format!("\n  /STATISTICS={keyword}"));
        }
        text.push('.');
        Ok(text)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum TTestKind {
    #[default]
    OneSample,
    Independent,
    Paired,
}

/// How a paired test scales Cohen's d.
pub const STANDARDIZERS: [(&str, &str); 3] = [
    ("SD", "Standard deviation of the differences"),
    ("CORRECTED_SD", "Corrected standard deviation"),
    ("AVERAGE", "Average of the variances"),
];

/// One of the three t tests: its variables and the options they share.
#[derive(Clone, PartialEq, Debug)]
pub struct TTest {
    pub kind: TTestKind,
    /// Test variables of the one-sample and the independent-samples test.
    pub variables: Vec<String>,
    pub test_value: String,
    pub grouping: String,
    /// Split the grouping variable at `group1` instead of comparing two values.
    pub cut_point: bool,
    /// Empty for both: 1 and 2, the default of a numeric grouping variable.
    pub group1: String,
    pub group2: String,
    pub pairs: Vec<(String, String)>,
    /// Confidence level in percent.
    pub confidence: String,
    pub listwise: bool,
    pub effect_sizes: bool,
    /// Index into `STANDARDIZERS`, for the paired test.
    pub standardizer: usize,
}

impl TTest {
    pub fn new(kind: TTestKind) -> Self {
        Self {
            kind,
            variables: Vec::new(),
            test_value: "0".into(),
            grouping: String::new(),
            cut_point: false,
            group1: String::new(),
            group2: String::new(),
            pairs: Vec::new(),
            confidence: "95".into(),
            listwise: false,
            effect_sizes: true,
            standardizer: 0,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            TTestKind::OneSample => "One-Sample T Test",
            TTestKind::Independent => "Independent-Samples T Test",
            TTestKind::Paired => "Paired-Samples T Test",
        }
    }

    /// `string` tells whether the grouping variable is a string variable.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let mut text = match self.kind {
            TTestKind::OneSample => format!(
                "T-TEST /TESTVAL={} /VARIABLES={}",
                value(&self.test_value, false)?,
                names(&self.variables, "Test variables")?
            ),
            TTestKind::Independent => {
                let variables = names(&self.variables, "Test variables")?;
                let grouping = self.grouping.trim();
                if grouping.is_empty() {
                    return Err("choose a grouping variable".into());
                }
                let given = [&self.group1, &self.group2].map(|g| !g.trim().is_empty());
                let groups = if self.cut_point {
                    if !given[0] {
                        return Err("give the cut point".into());
                    }
                    format!("({})", value(&self.group1, string)?)
                } else if given == [false, false] && !string {
                    String::new()
                } else if given != [true, true] {
                    return Err("give the values of both groups".into());
                } else {
                    format!(
                        "({} {})",
                        value(&self.group1, string)?,
                        value(&self.group2, string)?
                    )
                };
                format!("T-TEST GROUPS={grouping}{groups}\n  /VARIABLES={variables}")
            }
            TTestKind::Paired => {
                if self.pairs.is_empty() {
                    return Err("add at least one pair".into());
                }
                let (left, right): (Vec<&str>, Vec<&str>) = self
                    .pairs
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .unzip();
                format!(
                    "T-TEST PAIRS={} WITH {} (PAIRED)",
                    left.join(" "),
                    right.join(" ")
                )
            }
        };
        if let Some(level) = confidence(&self.confidence)? {
            text.push_str(&format!("\n  /CRITERIA=CI({level})"));
        }
        if self.listwise {
            text.push_str("\n  /MISSING=LISTWISE");
        }
        if !self.effect_sizes {
            text.push_str("\n  /ES DISPLAY(FALSE)");
        } else if self.kind == TTestKind::Paired && self.standardizer > 0 {
            let keyword = STANDARDIZERS[self.standardizer].0;
            text.push_str(&format!("\n  /ES STANDARDIZER({keyword})"));
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of every statistic of the One-Way ANOVA options.
pub const ONEWAY_STATISTICS: [(&str, &str); 4] = [
    ("DESCRIPTIVES", "Descriptives"),
    ("HOMOGENEITY", "Test of equal variances"),
    ("WELCH", "Welch test"),
    ("BROWNFORSYTHE", "Brown-Forsythe test"),
];

/// Keyword and label of every post hoc test One-Way ANOVA offers.
pub const POST_HOC_TESTS: [(&str, &str); 7] = [
    ("LSD", "LSD"),
    ("BONFERRONI", "Bonferroni"),
    ("SIDAK", "Sidak"),
    ("TUKEY", "Tukey"),
    ("SCHEFFE", "Scheffe"),
    ("GH", "Games-Howell"),
    ("T2", "Tamhane's T2"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Oneway {
    pub dependents: Vec<String>,
    pub factor: String,
    /// One flag per entry of `ONEWAY_STATISTICS`.
    pub statistics: [bool; 4],
    /// One flag per entry of `POST_HOC_TESTS`.
    pub post_hoc: [bool; 7],
    pub alpha: String,
    pub listwise: bool,
    /// Confidence level in percent.
    pub confidence: String,
}

impl Default for Oneway {
    fn default() -> Self {
        Self {
            dependents: Vec::new(),
            factor: String::new(),
            statistics: [false; 4],
            post_hoc: [false; 7],
            alpha: "0.05".into(),
            listwise: false,
            confidence: "95".into(),
        }
    }
}

fn keywords<'a>(options: &[(&'a str, &str)], flags: &[bool]) -> Vec<&'a str> {
    options
        .iter()
        .zip(flags)
        .filter_map(|((keyword, _), on)| on.then_some(*keyword))
        .collect()
}

impl Oneway {
    pub fn syntax(&self) -> Result<String, String> {
        let dependents = names(&self.dependents, "Dependent variables")?;
        let factor = self.factor.trim();
        if factor.is_empty() {
            return Err("choose a factor".into());
        }
        let mut text = format!("ONEWAY {dependents} BY {factor}");
        let statistics = keywords(&ONEWAY_STATISTICS, &self.statistics);
        if !statistics.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        let tests = keywords(&POST_HOC_TESTS, &self.post_hoc);
        if !tests.is_empty() {
            text.push_str(&format!("\n  /POSTHOC={}", tests.join(" ")));
            let alpha = probability(&self.alpha)?;
            if alpha != "0.05" {
                text.push_str(&format!(" ALPHA({alpha})"));
            }
        }
        if self.listwise {
            text.push_str("\n  /MISSING=LISTWISE");
        }
        if let Some(level) = confidence(&self.confidence)? {
            text.push_str(&format!("\n  /CRITERIA=CILEVEL({level})"));
        }
        text.push('.');
        Ok(text)
    }
}

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Correlations {
    pub variables: Vec<String>,
    pub one_tailed: bool,
    pub flag: bool,
    pub descriptives: bool,
    pub cross_products: bool,
    pub listwise: bool,
}

impl Correlations {
    pub fn syntax(&self) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        let mut text = format!("CORRELATIONS /VARIABLES={}", self.variables.join(" "));
        let print: Vec<&str> = [(self.one_tailed, "ONETAIL"), (self.flag, "NOSIG")]
            .into_iter()
            .filter_map(|(on, keyword)| on.then_some(keyword))
            .collect();
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let statistics: Vec<&str> = [
            (self.descriptives, "DESCRIPTIVES"),
            (self.cross_products, "XPROD"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{runs, v};

    #[test]
    fn means() {
        let mut d = Means::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Dependent variables"
        );
        d.dependents = v(&["age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "MEANS TABLES=age.");
        assert!(runs(&syntax) >= 1);
        d.anova = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the analysis of variance needs a variable in Layer 1"
        );
        d.dependents = v(&["age", "monthly_spend"]);
        d.layers = vec![v(&["region"]), Vec::new(), v(&["gender"])];
        d.cells[2] = false;
        d.cells[3] = true;
        d.cells[15] = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MEANS TABLES=age monthly_spend BY region BY gender\n  \
             /CELLS=MEAN COUNT MEDIAN GEOMETRIC\n  /STATISTICS=ANOVA."
        );
        assert!(runs(&syntax) >= 3);
        d.linearity = true;
        d.layers = vec![v(&["q1_satisfaction"])];
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with(
            "BY q1_satisfaction\n  /CELLS=MEAN COUNT MEDIAN GEOMETRIC\n  /STATISTICS=LINEARITY."
        ));
        assert!(runs(&syntax) >= 3);
        d.cells = [false; 16];
        assert_eq!(d.syntax().unwrap_err(), "choose at least one statistic");
    }

    #[test]
    fn one_sample_t_test() {
        let mut d = TTest::new(TTestKind::OneSample);
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "add at least one variable to Test variables"
        );
        d.variables = v(&["age"]);
        let syntax = d.syntax(false).unwrap();
        assert_eq!(syntax, "T-TEST /TESTVAL=0 /VARIABLES=age.");
        assert!(runs(&syntax) >= 2);
        d.test_value = "40.5".into();
        d.confidence = "90".into();
        d.listwise = true;
        d.effect_sizes = false;
        d.standardizer = 2; // only for paired tests
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST /TESTVAL=40.5 /VARIABLES=age\n  /CRITERIA=CI(0.9)\n  /MISSING=LISTWISE\n  /ES DISPLAY(FALSE)."
        );
        assert!(runs(&syntax) >= 2);
        d.test_value = "forty".into();
        assert_eq!(d.syntax(false).unwrap_err(), "\"forty\" is not a number");
        d.test_value = "40".into();
        d.confidence = "100".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the confidence level must be a percentage between 0 and 100"
        );
    }

    #[test]
    fn independent_samples_t_test() {
        let mut d = TTest::new(TTestKind::Independent);
        d.variables = v(&["age", "monthly_spend"]);
        assert_eq!(d.syntax(false).unwrap_err(), "choose a grouping variable");
        d.grouping = "gender".into();
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST GROUPS=gender\n  /VARIABLES=age monthly_spend."
        );
        assert!(runs(&syntax) >= 2);
        d.group1 = "1".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "give the values of both groups"
        );
        d.group2 = "3".into();
        d.confidence = "99".into();
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST GROUPS=gender(1 3)\n  /VARIABLES=age monthly_spend\n  /CRITERIA=CI(0.99)."
        );
        assert!(runs(&syntax) >= 2);
        d.grouping = "age".into();
        d.variables = v(&["monthly_spend"]);
        d.cut_point = true;
        d.group1.clear();
        assert_eq!(d.syntax(false).unwrap_err(), "give the cut point");
        d.group1 = "40".into();
        d.effect_sizes = false;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST GROUPS=age(40)\n  /VARIABLES=monthly_spend\n  /CRITERIA=CI(0.99)\n  /ES DISPLAY(FALSE)."
        );
        assert!(runs(&syntax) >= 2);
        // a string grouping variable has no default groups, and its values get quotes
        d.grouping = "plan".into();
        d.cut_point = false;
        d.group1.clear();
        d.group2.clear();
        assert_eq!(
            d.syntax(true).unwrap_err(),
            "give the values of both groups"
        );
        d.group1 = "BASIC".into();
        d.group2 = "PREMIUM".into();
        let syntax = d.syntax(true).unwrap();
        assert!(syntax.starts_with("T-TEST GROUPS=plan('BASIC' 'PREMIUM')\n"));
        assert!(runs(&syntax) >= 2);
    }

    #[test]
    fn paired_samples_t_test() {
        let mut d = TTest::new(TTestKind::Paired);
        assert_eq!(d.syntax(false).unwrap_err(), "add at least one pair");
        d.pairs = vec![("q1_satisfaction".into(), "q2_recommend".into())];
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST PAIRS=q1_satisfaction WITH q2_recommend (PAIRED)."
        );
        assert!(runs(&syntax) >= 3);
        d.pairs.push(("age".into(), "monthly_spend".into()));
        d.standardizer = 2;
        d.listwise = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "T-TEST PAIRS=q1_satisfaction age WITH q2_recommend monthly_spend (PAIRED)\n  \
             /MISSING=LISTWISE\n  /ES STANDARDIZER(AVERAGE)."
        );
        assert!(runs(&syntax) >= 3);
        d.effect_sizes = false;
        assert!(
            d.syntax(false)
                .unwrap()
                .ends_with("\n  /ES DISPLAY(FALSE).")
        );
    }

    #[test]
    fn oneway() {
        let mut d = Oneway {
            dependents: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "choose a factor");
        d.factor = "region".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "ONEWAY age BY region.");
        assert!(runs(&syntax) >= 1);
        d.dependents = v(&["age", "monthly_spend"]);
        d.statistics = [true; 4];
        d.post_hoc = [true; 7];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "ONEWAY age monthly_spend BY region\n  \
             /STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH BROWNFORSYTHE\n  \
             /POSTHOC=LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2."
        );
        assert!(runs(&syntax) >= 4);
        d.statistics = [false; 4];
        d.post_hoc = [false, false, false, true, false, true, false];
        d.alpha = "0.01".into();
        d.listwise = true;
        d.confidence = "90".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "ONEWAY age monthly_spend BY region\n  /POSTHOC=TUKEY GH ALPHA(0.01)\n  \
             /MISSING=LISTWISE\n  /CRITERIA=CILEVEL(0.9)."
        );
        assert!(runs(&syntax) >= 2);
        d.alpha = "0".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the significance level must be a number between 0 and 1"
        );
    }

    #[test]
    fn correlations() {
        let mut d = Correlations {
            variables: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "add at least two variables");
        d.variables.push("monthly_spend".into());
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "CORRELATIONS /VARIABLES=age monthly_spend.");
        assert_eq!(runs(&syntax), 1);
        d.variables.push("q2_recommend".into());
        d.one_tailed = true;
        d.flag = true;
        d.descriptives = true;
        d.cross_products = true;
        d.listwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CORRELATIONS /VARIABLES=age monthly_spend q2_recommend\n  /PRINT=ONETAIL NOSIG\n  \
             /STATISTICS=DESCRIPTIVES XPROD\n  /MISSING=LISTWISE."
        );
        assert_eq!(runs(&syntax), 2);
    }
}
