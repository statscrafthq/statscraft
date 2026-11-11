//! The Explore, Compare Means, Correlate, Multiple Response and Tree dialogs. Options left as
//! they are add nothing to the syntax, so the command runs with its own defaults.

use crate::dialogs::whole;
use crate::syntax_text::{confidence, names, number, probability, quote, value};

/// Keyword and label of every cell statistic Means offers, in the order shown.
pub const MEANS_CELLS: [(&str, &str); 21] = [
    ("MEAN", "Mean"),
    ("COUNT", "Number of cases"),
    ("STDDEV", "Standard deviation"),
    ("MEDIAN", "Median"),
    ("GMEDIAN", "Grouped median"),
    ("SEMEAN", "Standard error of the mean"),
    ("SUM", "Sum"),
    ("MIN", "Minimum"),
    ("MAX", "Maximum"),
    ("RANGE", "Range"),
    ("VARIANCE", "Variance"),
    ("SKEW", "Skewness"),
    ("SESKEW", "Standard error of skewness"),
    ("KURT", "Kurtosis"),
    ("SEKURT", "Standard error of kurtosis"),
    ("FIRST", "First value"),
    ("LAST", "Last value"),
    ("NPCT", "Percent of all cases"),
    ("SPCT", "Percent of the total sum"),
    ("HARMONIC", "Harmonic mean"),
    ("GEOMETRIC", "Geometric mean"),
];

/// MEAN COUNT STDDEV, what MEANS shows without /CELLS.
const MEANS_DEFAULT_CELLS: [bool; 21] = {
    let mut cells = [false; 21];
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
    pub cells: [bool; 21],
    /// The control variable of NPCT(var) and SPCT(var); empty for none.
    pub within: String,
    /// Percent of the cases, and of the sum, within the category of `within`.
    pub npct_within: bool,
    pub spct_within: bool,
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
            within: String::new(),
            npct_within: false,
            spct_within: false,
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
        let within = self.npct_within || self.spct_within;
        if self.cells != MEANS_DEFAULT_CELLS || within {
            let mut cells: Vec<String> = keywords(&MEANS_CELLS, &self.cells)
                .into_iter()
                .map(str::to_string)
                .collect();
            if within {
                let control = self.within.trim();
                if !layers
                    .iter()
                    .any(|layer| layer.iter().any(|v| v == control))
                {
                    return Err("choose a control variable for the percentages within".into());
                }
                for (on, keyword) in [(self.npct_within, "NPCT"), (self.spct_within, "SPCT")] {
                    if on {
                        cells.push(format!("{keyword}({control})"));
                    }
                }
            }
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

/// Keyword and label of every post hoc test One-Way ANOVA offers besides Dunnett's: the
/// first `EQUAL_VARIANCE_TESTS` assume equal variances, the rest do not.
pub const POST_HOC_TESTS: [(&str, &str); 16] = [
    ("LSD", "LSD"),
    ("BONFERRONI", "Bonferroni"),
    ("SIDAK", "Sidak"),
    ("SCHEFFE", "Scheffe"),
    ("FREGW", "Ryan-Einot-Gabriel-Welsch F"),
    ("QREGW", "Ryan-Einot-Gabriel-Welsch range"),
    ("SNK", "Student-Newman-Keuls"),
    ("TUKEY", "Tukey"),
    ("BTUKEY", "Tukey's b"),
    ("DUNCAN", "Duncan"),
    ("GT2", "Hochberg's GT2"),
    ("GABRIEL", "Gabriel"),
    ("T2", "Tamhane's T2"),
    ("T3", "Dunnett's T3"),
    ("GH", "Games-Howell"),
    ("C", "Dunnett's C"),
];

pub const EQUAL_VARIANCE_TESTS: usize = 12;

/// Keyword and label of each side of Dunnett's test against a control group.
pub const DUNNETT_SIDES: [(&str, &str); 3] = [
    ("DUNNETT", "Two-sided"),
    ("DUNNETTL", "Below the control"),
    ("DUNNETTR", "Above the control"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Oneway {
    pub dependents: Vec<String>,
    pub factor: String,
    /// One flag per entry of `ONEWAY_STATISTICS`.
    pub statistics: [bool; 4],
    /// One flag per entry of `POST_HOC_TESTS`.
    pub post_hoc: [bool; 16],
    /// Dunnett's test: each group against a control group.
    pub dunnett: bool,
    /// The control is the first group instead of the last.
    pub dunnett_first: bool,
    /// Index into `DUNNETT_SIDES`.
    pub dunnett_side: usize,
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
            post_hoc: [false; 16],
            dunnett: false,
            dunnett_first: false,
            dunnett_side: 0,
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
        let mut tests: Vec<String> = keywords(&POST_HOC_TESTS, &self.post_hoc)
            .into_iter()
            .map(str::to_string)
            .collect();
        if self.dunnett {
            let keyword = DUNNETT_SIDES[self.dunnett_side].0;
            // the control is the last group unless its position is given
            let control = if self.dunnett_first { "(1)" } else { "" };
            tests.push(format!("{keyword}{control}"));
        }
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

/// Bivariate correlations: Pearson's r by CORRELATIONS, Spearman's rho and Kendall's tau-b by
/// NONPAR CORR, both commands when both kinds are chosen.
#[derive(Clone, PartialEq, Debug)]
pub struct Correlations {
    pub variables: Vec<String>,
    pub pearson: bool,
    pub kendall: bool,
    pub spearman: bool,
    pub one_tailed: bool,
    pub flag: bool,
    /// Pearson only.
    pub descriptives: bool,
    /// Pearson only.
    pub cross_products: bool,
    pub listwise: bool,
}

impl Default for Correlations {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            pearson: true,
            kendall: false,
            spearman: false,
            one_tailed: false,
            flag: false,
            descriptives: false,
            cross_products: false,
            listwise: false,
        }
    }
}

impl Correlations {
    pub fn syntax(&self) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        if !(self.pearson || self.kendall || self.spearman) {
            return Err("choose at least one correlation coefficient".into());
        }
        let list = self.variables.join(" ");
        let print = |first: Option<&str>| {
            let print: Vec<&str> = first
                .into_iter()
                .chain(self.one_tailed.then_some("ONETAIL"))
                .chain(self.flag.then_some("NOSIG"))
                .collect();
            if print.is_empty() {
                String::new()
            } else {
                format!("\n  /PRINT={}", print.join(" "))
            }
        };
        let missing = if self.listwise {
            "\n  /MISSING=LISTWISE"
        } else {
            ""
        };
        let mut commands = Vec::new();
        if self.pearson {
            let mut text = format!("CORRELATIONS /VARIABLES={list}{}", print(None));
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
            commands.push(format!("{text}{missing}."));
        }
        if self.kendall || self.spearman {
            // Spearman alone is what NONPAR CORR gives without /PRINT
            let which = match (self.spearman, self.kendall) {
                (true, true) => Some("BOTH"),
                (false, true) => Some("KENDALL"),
                _ => None,
            };
            commands.push(format!(
                "NONPAR CORR /VARIABLES={list}{}{missing}.",
                print(which)
            ));
        }
        Ok(commands.join("\n"))
    }
}

/// How Explore treats missing values.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ExploreMissing {
    #[default]
    Listwise,
    Pairwise,
    Report,
}

impl ExploreMissing {
    pub const ALL: [ExploreMissing; 3] = [
        ExploreMissing::Listwise,
        ExploreMissing::Pairwise,
        ExploreMissing::Report,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExploreMissing::Listwise => "Leave out a case missing on any listed variable",
            ExploreMissing::Pairwise => "Leave out a case only where its value is missing",
            ExploreMissing::Report => "Show missing factor values as groups of their own",
        }
    }
}

/// EXAMINE: dependent variables over all cases and within the groups of each factor.
#[derive(Clone, PartialEq, Debug)]
pub struct Explore {
    pub dependents: Vec<String>,
    pub factors: Vec<String>,
    /// The variable that labels extreme values; empty for case numbers.
    pub id: String,
    pub descriptives: bool,
    /// Confidence level of the mean in percent.
    pub confidence: String,
    pub mestimators: bool,
    /// The five highest and lowest values.
    pub outliers: bool,
    pub percentiles: bool,
    /// Tests of normality (NPPLOT; the plots themselves are not drawn).
    pub normality: bool,
    pub missing: ExploreMissing,
}

impl Default for Explore {
    fn default() -> Self {
        Self {
            dependents: Vec::new(),
            factors: Vec::new(),
            id: String::new(),
            descriptives: true,
            confidence: "95".into(),
            mestimators: false,
            outliers: false,
            percentiles: false,
            normality: false,
            missing: ExploreMissing::Listwise,
        }
    }
}

impl Explore {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!(
            "EXAMINE VARIABLES={}",
            names(&self.dependents, "Dependent variables")?
        );
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        let id = self.id.trim();
        if !id.is_empty() {
            text.push_str(&format!("\n  /ID={id}"));
        }
        if !(self.descriptives
            || self.outliers
            || self.percentiles
            || self.mestimators
            || self.normality)
        {
            return Err("choose at least one statistic or test".into());
        }
        if self.percentiles {
            text.push_str("\n  /PERCENTILES");
        }
        match (self.descriptives, self.outliers) {
            (true, false) => {}
            (true, true) => text.push_str("\n  /STATISTICS=DESCRIPTIVES EXTREME"),
            (false, true) => text.push_str("\n  /STATISTICS=EXTREME"),
            (false, false) => text.push_str("\n  /STATISTICS=NONE"),
        }
        if self.descriptives {
            let percent = self
                .confidence
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|p| (50.0..=99.99).contains(p))
                .ok_or("the confidence level must be a percentage from 50 to 99.99")?;
            if percent != 95.0 {
                text.push_str(&format!("\n  /CINTERVAL={}", number(percent)));
            }
        }
        if self.mestimators {
            text.push_str("\n  /MESTIMATORS");
        }
        if self.normality {
            text.push_str("\n  /PLOT=NPPLOT");
        }
        match self.missing {
            ExploreMissing::Listwise => {}
            ExploreMissing::Pairwise => text.push_str("\n  /MISSING=PAIRWISE"),
            ExploreMissing::Report => text.push_str("\n  /MISSING=REPORT"),
        }
        text.push('.');
        Ok(text)
    }
}

/// One group of MULT RESPONSE: a dichotomy group counts one value of its variables, a category
/// group the values from `value` to `max`.
#[derive(Clone, PartialEq, Debug)]
pub struct MrGroup {
    pub name: String,
    pub label: String,
    pub variables: Vec<String>,
    pub dichotomy: bool,
    pub value: String,
    pub max: String,
}

impl Default for MrGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            label: String::new(),
            variables: Vec::new(),
            dichotomy: true,
            value: "1".into(),
            max: String::new(),
        }
    }
}

/// Keyword and label of every percentage of a multiple response crosstabulation.
pub const MULT_RESPONSE_CELLS: [(&str, &str); 3] = [
    ("ROW", "Row percentages"),
    ("COLUMN", "Column percentages"),
    ("TOTAL", "Total percentages"),
];

/// MULT RESPONSE: frequency tables of the groups, or crosstabulations of groups and variables.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct MultResponse {
    pub crosstabs: bool,
    pub groups: Vec<MrGroup>,
    /// Crosstabs: the groups and variables of each dimension.
    pub rows: Vec<String>,
    pub columns: Vec<String>,
    pub layers: Vec<String>,
    /// The range of every variable of the tables that is not a group: name, min and max.
    pub ranges: Vec<(String, String, String)>,
    /// One flag per entry of `MULT_RESPONSE_CELLS`; counts are always shown.
    pub cells: [bool; 3],
    pub base_responses: bool,
    /// Leave out a case missing on any variable of a dichotomy group (MDGROUP) or of a
    /// category group (MRGROUP).
    pub missing_dichotomies: bool,
    pub missing_categories: bool,
}

impl MultResponse {
    pub fn title(&self) -> &'static str {
        if self.crosstabs {
            "Multiple Response Crosstabs"
        } else {
            "Multiple Response Frequencies"
        }
    }

    pub fn is_group(&self, name: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.name.trim().eq_ignore_ascii_case(name))
    }

    /// The variables of the tables that are not groups, each once, in order.
    pub fn table_variables(&self) -> Vec<&String> {
        let mut out: Vec<&String> = Vec::new();
        for name in self.rows.iter().chain(&self.columns).chain(&self.layers) {
            if !self.is_group(name) && !out.contains(&name) {
                out.push(name);
            }
        }
        out
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut text = String::from("MULT RESPONSE");
        for (i, g) in self.groups.iter().enumerate() {
            let name = g.name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err("give every group a name of one word".into());
            }
            let variables = names(&g.variables, &format!("the group {name}"))?;
            text.push_str(if i == 0 { " GROUPS=" } else { "\n  " });
            text.push_str(name);
            if !g.label.trim().is_empty() {
                text.push_str(&format!(" {}", quote(g.label.trim())));
            }
            let first = value(&g.value, false)?;
            let values = match g.dichotomy {
                true => first,
                false => format!("{first}, {}", value(&g.max, false)?),
            };
            text.push_str(&format!(" ({variables} ({values}))"));
        }
        if self.crosstabs {
            for (i, name) in self.table_variables().into_iter().enumerate() {
                let Some((_, min, max)) = self.ranges.iter().find(|(n, ..)| n == name) else {
                    return Err(format!("give the range of {name}"));
                };
                let (min, max) = (value(min, false)?, value(max, false)?);
                text.push_str(if i == 0 { "\n  /VARIABLES=" } else { " " });
                text.push_str(&format!("{name} ({min}, {max})"));
            }
            text.push_str(&format!(
                "\n  /TABLES={} BY {}",
                names(&self.rows, "Rows")?,
                names(&self.columns, "Columns")?
            ));
            if !self.layers.is_empty() {
                text.push_str(&format!(" BY {}", self.layers.join(" ")));
            }
        } else {
            if self.groups.is_empty() {
                return Err("define at least one group".into());
            }
            let groups: Vec<&str> = self.groups.iter().map(|g| g.name.trim()).collect();
            text.push_str(&format!("\n  /FREQUENCIES={}", groups.join(" ")));
        }
        let missing: Vec<&str> = [
            (self.missing_dichotomies, "MDGROUP"),
            (self.missing_categories, "MRGROUP"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
        }
        if self.crosstabs && self.cells.contains(&true) {
            if self.base_responses {
                text.push_str("\n  /BASE=RESPONSES");
            }
            let cells: Vec<&str> = MULT_RESPONSE_CELLS
                .iter()
                .zip(self.cells)
                .filter_map(|((keyword, _), on)| on.then_some(*keyword))
                .collect();
            text.push_str(&format!("\n  /CELLS=COUNT {}", cells.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// The level written after a variable of TREE, the measure of the dictionary first.
pub const TREE_LEVELS: [(&str, &str); 4] = [
    ("", "Its measure"),
    ("N", "Nominal"),
    ("O", "Ordinal"),
    ("S", "Scale"),
];

/// Keyword and label of every variable TREE saves.
pub const TREE_SAVE: [(&str, &str); 3] = [
    ("NODEID", "Terminal node"),
    ("PREDVAL", "Predicted category"),
    ("PREDPROB", "Predicted probabilities"),
];

/// TREE by CHAID or exhaustive CHAID.
#[derive(Clone, PartialEq, Debug)]
pub struct Tree {
    pub dependent: String,
    /// Index into `TREE_LEVELS`; the dependent is never scale.
    pub dependent_level: usize,
    /// Each predictor with an index into `TREE_LEVELS`.
    pub predictors: Vec<(String, usize)>,
    pub exhaustive: bool,
    /// Empty for the automatic depth.
    pub max_depth: String,
    pub min_parent: String,
    pub min_child: String,
    /// The target categories of the gain tables, separated by spaces.
    pub targets: String,
    /// One flag per entry of `TREE_SAVE`.
    pub save: [bool; 3],
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            dependent_level: 0,
            predictors: Vec::new(),
            exhaustive: false,
            max_depth: String::new(),
            min_parent: "100".into(),
            min_child: "50".into(),
            targets: String::new(),
            save: [false; 3],
        }
    }
}

impl Tree {
    /// `string` tells whether the dependent is a string variable, so the targets get quotes.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let level = |i: usize| match TREE_LEVELS[i].0 {
            "" => String::new(),
            level => format!(" [{level}]"),
        };
        if self.dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        if self.predictors.is_empty() {
            return Err("add at least one variable to Predictors".into());
        }
        let mut text = format!("TREE {}{} BY", self.dependent, level(self.dependent_level));
        for (name, l) in &self.predictors {
            text.push_str(&format!(" {name}{}", level(*l)));
        }
        if self.exhaustive {
            text.push_str("\n  /METHOD TYPE=EXHAUSTIVECHAID");
        }
        let depth = whole(&self.max_depth, 1, "the maximum depth")?;
        let parent = whole(&self.min_parent, 1, "the parent node size")?.unwrap_or(100);
        let child = whole(&self.min_child, 1, "the child node size")?.unwrap_or(50);
        if child >= parent {
            return Err("the child node size must be smaller than the parent node size".into());
        }
        let mut limits = String::new();
        if let Some(depth) = depth {
            limits.push_str(&format!(" MAXDEPTH={depth}"));
        }
        if parent != 100 {
            limits.push_str(&format!(" MINPARENTSIZE={parent}"));
        }
        if child != 50 {
            limits.push_str(&format!(" MINCHILDSIZE={child}"));
        }
        if !limits.is_empty() {
            text.push_str(&format!("\n  /GROWTHLIMIT{limits}"));
        }
        let targets = self
            .targets
            .split([' ', ','])
            .filter(|t| !t.is_empty())
            .map(|t| value(t, string))
            .collect::<Result<Vec<_>, _>>()?;
        if !targets.is_empty() {
            text.push_str(&format!(
                "\n  /DEPCATEGORIES TARGET=[{}]",
                targets.join(" ")
            ));
        }
        let save: Vec<&str> = TREE_SAVE
            .iter()
            .zip(self.save)
            .filter_map(|((keyword, _), on)| on.then_some(*keyword))
            .collect();
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
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
        d.cells[20] = true;
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
        d.cells = [false; 21];
        assert_eq!(d.syntax().unwrap_err(), "choose at least one statistic");
    }

    #[test]
    fn means_grouped_median_and_percentages_within() {
        let mut d = Means {
            dependents: v(&["monthly_spend"]),
            layers: vec![v(&["region"]), v(&["gender"])],
            ..Default::default()
        };
        d.cells[4] = true;
        d.cells[17] = true;
        d.cells[18] = true;
        d.npct_within = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose a control variable for the percentages within"
        );
        d.within = "age".into();
        assert!(d.syntax().is_err());
        d.within = "region".into();
        d.spct_within = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MEANS TABLES=monthly_spend BY region BY gender\n  \
             /CELLS=MEAN COUNT STDDEV GMEDIAN NPCT SPCT NPCT(region) SPCT(region)."
        );
        assert!(runs(&syntax) >= 2);
        // a control is chosen but no percentage within: nothing added
        d.npct_within = false;
        d.spct_within = false;
        d.cells = MEANS_DEFAULT_CELLS;
        d.cells[15] = true;
        d.cells[16] = true;
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("/CELLS=MEAN COUNT STDDEV FIRST LAST."));
        assert!(runs(&syntax) >= 2);
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
        d.post_hoc = [true; 16];
        d.dunnett = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "ONEWAY age monthly_spend BY region\n  \
             /STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH BROWNFORSYTHE\n  \
             /POSTHOC=LSD BONFERRONI SIDAK SCHEFFE FREGW QREGW SNK TUKEY BTUKEY DUNCAN GT2 \
             GABRIEL T2 T3 GH C DUNNETT."
        );
        // multiple comparisons and homogeneous subsets for both dependents
        assert!(runs(&syntax) >= 8);
        d.statistics = [false; 4];
        d.post_hoc = [false; 16];
        d.post_hoc[7] = true;
        d.post_hoc[14] = true;
        d.dunnett = false;
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
    fn oneway_dunnett_control_and_side() {
        let mut d = Oneway {
            dependents: v(&["monthly_spend"]),
            factor: "region".into(),
            dunnett: true,
            ..Default::default()
        };
        for (side, first, keyword) in [
            (0, false, "DUNNETT"),
            (1, false, "DUNNETTL"),
            (2, true, "DUNNETTR(1)"),
            (0, true, "DUNNETT(1)"),
        ] {
            d.dunnett_side = side;
            d.dunnett_first = first;
            let syntax = d.syntax().unwrap();
            assert_eq!(
                syntax,
                format!("ONEWAY monthly_spend BY region\n  /POSTHOC={keyword}.")
            );
            assert!(runs(&syntax) >= 2);
        }
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
        d.pearson = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose at least one correlation coefficient"
        );
    }

    #[test]
    fn rank_correlations_switch_the_command() {
        let mut d = Correlations {
            variables: v(&["q1_satisfaction", "q2_recommend"]),
            pearson: false,
            spearman: true,
            ..Default::default()
        };
        // Spearman alone: NONPAR CORR with its own default
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NONPAR CORR /VARIABLES=q1_satisfaction q2_recommend."
        );
        assert_eq!(runs(&syntax), 1);
        d.spearman = false;
        d.kendall = true;
        d.flag = true;
        // the Pearson statistics are left out when Pearson is not chosen
        d.descriptives = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NONPAR CORR /VARIABLES=q1_satisfaction q2_recommend\n  /PRINT=KENDALL NOSIG."
        );
        assert_eq!(runs(&syntax), 1);
        d.spearman = true;
        d.one_tailed = true;
        d.listwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "NONPAR CORR /VARIABLES=q1_satisfaction q2_recommend\n  \
             /PRINT=BOTH ONETAIL NOSIG\n  /MISSING=LISTWISE."
        );
        assert_eq!(runs(&syntax), 1);
        // Pearson and a rank correlation: both commands
        d.pearson = true;
        d.kendall = false;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CORRELATIONS /VARIABLES=q1_satisfaction q2_recommend\n  /PRINT=ONETAIL NOSIG\n  \
             /STATISTICS=DESCRIPTIVES\n  /MISSING=LISTWISE.\n\
             NONPAR CORR /VARIABLES=q1_satisfaction q2_recommend\n  \
             /PRINT=ONETAIL NOSIG\n  /MISSING=LISTWISE."
        );
        assert_eq!(runs(&syntax), 3);
    }

    #[test]
    fn explore() {
        let mut d = Explore::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Dependent variables"
        );
        d.dependents = v(&["age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "EXAMINE VARIABLES=age.");
        assert!(runs(&syntax) >= 1);

        d.dependents = v(&["age", "monthly_spend"]);
        d.factors = v(&["region", "plan"]);
        d.id = "resp_id".into();
        d.percentiles = true;
        d.outliers = true;
        d.confidence = "90".into();
        d.mestimators = true;
        d.normality = true;
        d.missing = ExploreMissing::Pairwise;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "EXAMINE VARIABLES=age monthly_spend BY region plan\n  /ID=resp_id\n  \
             /PERCENTILES\n  /STATISTICS=DESCRIPTIVES EXTREME\n  /CINTERVAL=90\n  \
             /MESTIMATORS\n  /PLOT=NPPLOT\n  /MISSING=PAIRWISE."
        );
        assert!(runs(&syntax) >= 5);

        // without descriptives: only what is chosen, and no confidence level
        d.descriptives = false;
        d.confidence = "x".into();
        d.percentiles = false;
        d.mestimators = false;
        d.normality = false;
        d.missing = ExploreMissing::Report;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "EXAMINE VARIABLES=age monthly_spend BY region plan\n  /ID=resp_id\n  \
             /STATISTICS=EXTREME\n  /MISSING=REPORT."
        );
        assert!(runs(&syntax) >= 1);
        d.outliers = false;
        d.normality = true;
        let syntax = d.syntax().unwrap();
        assert!(syntax.contains("/STATISTICS=NONE\n  /PLOT=NPPLOT"));
        assert!(runs(&syntax) >= 1);
        d.normality = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose at least one statistic or test"
        );
        d.descriptives = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the confidence level must be a percentage from 50 to 99.99"
        );
        d.confidence = "40".into();
        assert!(d.syntax().is_err());
    }

    #[test]
    fn mult_response_frequencies() {
        let mut d = MultResponse::default();
        assert_eq!(d.syntax().unwrap_err(), "define at least one group");
        d.groups.push(MrGroup::default());
        assert_eq!(
            d.syntax().unwrap_err(),
            "give every group a name of one word"
        );
        d.groups[0].name = "channels".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to the group channels"
        );
        d.groups[0].variables = v(&["q5_1", "q5_2", "q5_3"]);
        d.groups[0].label = "Contact channels".into();
        d.groups.push(MrGroup {
            name: "mentions".into(),
            variables: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
            dichotomy: false,
            max: "6".into(),
            ..Default::default()
        });
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MULT RESPONSE GROUPS=channels 'Contact channels' (q5_1 q5_2 q5_3 (1))\n  \
             mentions (q6_mention1 q6_mention2 q6_mention3 (1, 6))\n  \
             /FREQUENCIES=channels mentions."
        );
        // a case summary and a table for each group
        assert_eq!(runs(&syntax), 3);
        d.missing_categories = true;
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("/FREQUENCIES=channels mentions\n  /MISSING=MRGROUP."));
        assert_eq!(runs(&syntax), 3);
        d.groups[1].max = "x".into();
        assert_eq!(d.syntax().unwrap_err(), "\"x\" is not a number");
    }

    #[test]
    fn mult_response_crosstabs() {
        let mut d = MultResponse {
            crosstabs: true,
            groups: vec![MrGroup {
                name: "channels".into(),
                variables: v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "add at least one variable to Rows");
        d.rows = v(&["channels"]);
        d.columns = v(&["gender"]);
        assert_eq!(d.syntax().unwrap_err(), "give the range of gender");
        d.ranges.push(("gender".into(), "1".into(), "3".into()));
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MULT RESPONSE GROUPS=channels (q5_1 q5_2 q5_3 q5_4 q5_5 (1))\n  \
             /VARIABLES=gender (1, 3)\n  /TABLES=channels BY gender."
        );
        assert_eq!(runs(&syntax), 2);
        d.layers = v(&["region"]);
        d.ranges.push(("region".into(), "1".into(), "5".into()));
        d.cells = [false, true, false];
        d.base_responses = true;
        d.missing_dichotomies = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "MULT RESPONSE GROUPS=channels (q5_1 q5_2 q5_3 q5_4 q5_5 (1))\n  \
             /VARIABLES=gender (1, 3) region (1, 5)\n  \
             /TABLES=channels BY gender BY region\n  /MISSING=MDGROUP\n  \
             /BASE=RESPONSES\n  /CELLS=COUNT COLUMN."
        );
        assert!(runs(&syntax) >= 2);
        // no groups: variables only
        d.groups.clear();
        d.rows = v(&["region"]);
        d.layers.clear();
        d.missing_dichotomies = false;
        let syntax = d.syntax().unwrap();
        assert!(syntax.starts_with("MULT RESPONSE\n  /VARIABLES=region (1, 5) gender (1, 3)"));
        assert_eq!(runs(&syntax), 2);
    }

    #[test]
    fn tree() {
        let mut d = Tree::default();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "choose the dependent variable"
        );
        d.dependent = "gender".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "add at least one variable to Predictors"
        );
        d.predictors = vec![("region".into(), 0), ("age".into(), 3)];
        let syntax = d.syntax(false).unwrap();
        assert_eq!(syntax, "TREE gender BY region age [S].");
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 1);

        d.dependent = "q1_satisfaction".into();
        d.dependent_level = 2;
        d.predictors[0].1 = 1;
        d.exhaustive = true;
        d.max_depth = "2".into();
        d.min_parent = "40".into();
        d.min_child = "20".into();
        d.targets = "4, 5".into();
        d.save = [true, true, true];
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "TREE q1_satisfaction [O] BY region [N] age [S]\n  /METHOD TYPE=EXHAUSTIVECHAID\n  \
             /GROWTHLIMIT MAXDEPTH=2 MINPARENTSIZE=40 MINCHILDSIZE=20\n  \
             /DEPCATEGORIES TARGET=[4 5]\n  /SAVE NODEID PREDVAL PREDPROB."
        );
        assert!(run_in(&mut session, &syntax) >= 1);
        let data = session.dataset().unwrap();
        assert!(data.variable("NodeID").is_ok());
        assert!(data.variable("PredictedValue").is_ok());

        d.min_child = "40".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the child node size must be smaller than the parent node size"
        );
        d.min_child = "20".into();
        d.max_depth = "0".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the maximum depth must be a whole number of at least 1"
        );
        d.max_depth.clear();
        d.dependent = "plan".into();
        d.targets = "BASIC".into();
        assert!(d.syntax(true).unwrap().contains("TARGET=['BASIC']"));
    }
}
