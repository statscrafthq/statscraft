//! The dialogs of the Graphs menu: Chart Builder (GRAPH), P-P Plots and Q-Q Plots (PPLOT) and
//! ROC Curve (ROC). Options left as they are add nothing to the syntax, so the command runs with
//! its own defaults.

use crate::dialogs::whole;
use crate::stats_dialogs::{parse, percent};
use crate::syntax_text::{names, number, quote, value};

/// The charts of the Chart Builder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChartKind {
    SimpleBar,
    ClusteredBar,
    StackedBar,
    SimpleLine,
    MultipleLine,
    Area,
    Pie,
    Histogram,
    SimpleScatter,
    GroupedScatter,
    OverlayScatter,
    MatrixScatter,
    ErrorBar,
    Pareto,
    HighLow,
}

impl ChartKind {
    pub const ALL: [ChartKind; 15] = [
        ChartKind::SimpleBar,
        ChartKind::ClusteredBar,
        ChartKind::StackedBar,
        ChartKind::SimpleLine,
        ChartKind::MultipleLine,
        ChartKind::Area,
        ChartKind::Pie,
        ChartKind::Histogram,
        ChartKind::SimpleScatter,
        ChartKind::GroupedScatter,
        ChartKind::OverlayScatter,
        ChartKind::MatrixScatter,
        ChartKind::ErrorBar,
        ChartKind::Pareto,
        ChartKind::HighLow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChartKind::SimpleBar => "Bar: simple",
            ChartKind::ClusteredBar => "Bar: clustered",
            ChartKind::StackedBar => "Bar: stacked",
            ChartKind::SimpleLine => "Line: simple",
            ChartKind::MultipleLine => "Line: multiple",
            ChartKind::Area => "Area",
            ChartKind::Pie => "Pie",
            ChartKind::Histogram => "Histogram",
            ChartKind::SimpleScatter => "Scatter: simple",
            ChartKind::GroupedScatter => "Scatter: grouped",
            ChartKind::OverlayScatter => "Scatter: overlay",
            ChartKind::MatrixScatter => "Scatter: matrix",
            ChartKind::ErrorBar => "Error bar",
            ChartKind::Pareto => "Pareto",
            ChartKind::HighLow => "High-low",
        }
    }

    /// A chart of a function by categories.
    pub fn summarizes(self) -> bool {
        matches!(
            self,
            ChartKind::SimpleBar
                | ChartKind::ClusteredBar
                | ChartKind::StackedBar
                | ChartKind::SimpleLine
                | ChartKind::MultipleLine
                | ChartKind::Area
                | ChartKind::Pie
                | ChartKind::Pareto
                | ChartKind::HighLow
        )
    }

    /// The indices into `GRAPH_FUNCTIONS` the chart takes.
    pub fn functions(self) -> Vec<usize> {
        match self {
            ChartKind::Pareto => vec![0, SUM],
            ChartKind::HighLow => (COUNT_FUNCTIONS..GRAPH_FUNCTIONS.len()).collect(),
            _ => (0..GRAPH_FUNCTIONS.len()).collect(),
        }
    }

    /// The title of the variable whose values make the series or groups of points, if the
    /// chart has one.
    pub fn group(self) -> Option<&'static str> {
        match self {
            ChartKind::ClusteredBar | ChartKind::ErrorBar => Some("Clusters by"),
            ChartKind::StackedBar | ChartKind::Area | ChartKind::Pareto => Some("Stacks by"),
            ChartKind::MultipleLine => Some("Lines by"),
            ChartKind::GroupedScatter | ChartKind::MatrixScatter => Some("Groups of points by"),
            _ => None,
        }
    }

    /// Several series are what the chart is for: from the group variable or from several
    /// summarized variables.
    fn needs_series(self) -> bool {
        matches!(
            self,
            ChartKind::ClusteredBar | ChartKind::StackedBar | ChartKind::MultipleLine
        )
    }

    pub fn scatter(self) -> bool {
        matches!(
            self,
            ChartKind::SimpleScatter
                | ChartKind::GroupedScatter
                | ChartKind::OverlayScatter
                | ChartKind::MatrixScatter
        )
    }
}

/// Keyword, label and the number of numbers each function of GRAPH takes. The count functions
/// come first.
pub const GRAPH_FUNCTIONS: [(&str, &str, usize); 22] = [
    ("COUNT", "Number of cases", 0),
    ("PCT", "Percentage of cases", 0),
    ("CUFREQ", "Cumulative number of cases", 0),
    ("CUPCT", "Cumulative percentage of cases", 0),
    ("MEAN", "Mean", 0),
    ("MEDIAN", "Median", 0),
    ("MODE", "Mode", 0),
    ("N", "Number of valid values", 0),
    ("SUM", "Sum", 0),
    ("CUSUM", "Cumulative sum", 0),
    ("MINIMUM", "Minimum", 0),
    ("MAXIMUM", "Maximum", 0),
    ("STDDEV", "Standard deviation", 0),
    ("VARIANCE", "Variance", 0),
    ("GMEDIAN", "Grouped median", 0),
    ("PTILE", "Percentile", 1),
    ("PLT", "Percentage below a value", 1),
    ("PGT", "Percentage above a value", 1),
    ("NLT", "Number below a value", 1),
    ("NGT", "Number above a value", 1),
    ("PIN", "Percentage within a range", 2),
    ("NIN", "Number within a range", 2),
];

/// COUNT, PCT, CUFREQ and CUPCT count cases and summarize no variable.
pub const COUNT_FUNCTIONS: usize = 4;
const SUM: usize = 8;

/// Keyword and label of what an error bar spans.
pub const ERROR_BARS: [(&str, &str); 3] = [
    ("CI", "Confidence interval of the mean, level %"),
    ("STERROR", "Standard errors of the mean, how many"),
    ("STDDEV", "Standard deviations, how many"),
];

/// GRAPH: one chart of the active dataset.
#[derive(Clone, PartialEq, Debug)]
pub struct ChartBuilder {
    pub kind: ChartKind,
    /// Index into `GRAPH_FUNCTIONS`.
    pub function: usize,
    /// The numbers of the function: a percentile, a value, or the ends of a range.
    pub low: String,
    pub high: String,
    /// The summarized variables; the variables of an error bar chart, a matrix, and the X axis
    /// of an overlay.
    pub variables: Vec<String>,
    /// The Y axis of an overlay.
    pub y_variables: Vec<String>,
    /// Pairs the overlay's lists in order instead of every combination.
    pub pair: bool,
    pub category: String,
    pub group: String,
    /// The variable of a histogram, the X axis of a scatterplot.
    pub x: String,
    pub y: String,
    /// The variable whose values label the points.
    pub label: String,
    pub normal: bool,
    /// Index into `ERROR_BARS`.
    pub error_bar: usize,
    pub level: String,
    pub multiplier: String,
    /// How many standard deviations, apart from the standard errors' `multiplier`.
    pub deviations: String,
    /// The cumulative line of a Pareto chart.
    pub cumulative: bool,
    pub title: String,
    pub subtitle: String,
    pub footnote: String,
    /// Leave a case out only of the summaries of the variable it is missing on.
    pub by_variable: bool,
    /// Categories for missing values.
    pub report: bool,
    pub include: bool,
    pub panel_columns: Vec<String>,
    pub panel_rows: Vec<String>,
}

impl Default for ChartBuilder {
    fn default() -> Self {
        Self {
            kind: ChartKind::SimpleBar,
            function: 0,
            low: String::new(),
            high: String::new(),
            variables: Vec::new(),
            y_variables: Vec::new(),
            pair: false,
            category: String::new(),
            group: String::new(),
            x: String::new(),
            y: String::new(),
            label: String::new(),
            normal: false,
            error_bar: 0,
            level: "95".into(),
            multiplier: "2".into(),
            deviations: "2".into(),
            cumulative: true,
            title: String::new(),
            subtitle: String::new(),
            footnote: String::new(),
            by_variable: false,
            report: false,
            include: false,
            panel_columns: Vec::new(),
            panel_rows: Vec::new(),
        }
    }
}

/// One variable from a slot, or an error naming the slot.
fn one(slot: &str, what: &str) -> Result<String, String> {
    match slot.trim() {
        "" => Err(format!("set {what}")),
        name => Ok(name.to_string()),
    }
}

/// A quoted line of TITLE, SUBTITLE or FOOTNOTE, None when empty.
fn line(text: &str, what: &str) -> Result<Option<String>, String> {
    let text = text.trim();
    if text.chars().count() > 72 {
        return Err(format!("the {what} has at most 72 characters"));
    }
    Ok((!text.is_empty()).then(|| quote(text)))
}

impl ChartBuilder {
    /// The variables whose values the chart measures, which must be numbers.
    pub fn measured(&self) -> Vec<&String> {
        match self.kind {
            k if k.summarizes() && self.function < COUNT_FUNCTIONS => Vec::new(),
            ChartKind::Histogram => vec![&self.x],
            ChartKind::SimpleScatter | ChartKind::GroupedScatter => vec![&self.x, &self.y],
            ChartKind::OverlayScatter => self.variables.iter().chain(&self.y_variables).collect(),
            _ => self.variables.iter().collect(),
        }
    }

    /// ` BY label (NAME)` when points are labeled.
    fn labels(&self) -> String {
        match self.label.trim() {
            "" => String::new(),
            name => format!(" BY {name} (NAME)"),
        }
    }

    /// The function by the category and group variables.
    fn summary(&self) -> Result<String, String> {
        let kind = self.kind;
        if !kind.functions().contains(&self.function) {
            return Err(match kind {
                ChartKind::Pareto => "a Pareto chart shows the number of cases or a sum".into(),
                _ => "a high-low chart summarizes variables; choose a function other than a count"
                    .into(),
            });
        }
        let (keyword, _, numbers) = GRAPH_FUNCTIONS[self.function];
        let counts = self.function < COUNT_FUNCTIONS;
        let mut spec = keyword.to_string();
        if !counts {
            let mut inside = names(&self.variables, "Variables")?;
            if numbers == 1 {
                let x = parse(&self.low, "the value of the function")?;
                if keyword == "PTILE" && !(x > 0.0 && x < 100.0) {
                    return Err("the percentile must be between 0 and 100".into());
                }
                inside.push_str(&format!(",{}", number(x)));
            } else if numbers == 2 {
                let low = parse(&self.low, "the lower end of the range")?;
                let high = parse(&self.high, "the upper end of the range")?;
                if low > high {
                    return Err("the lower end of the range comes first".into());
                }
                inside.push_str(&format!(",{},{}", number(low), number(high)));
            }
            spec.push_str(&format!("({inside})"));
        }
        let many = !counts && self.variables.len() > 1;
        match kind {
            ChartKind::Pie if many => return Err("a pie summarizes one variable".into()),
            ChartKind::SimpleBar | ChartKind::SimpleLine if many => {
                return Err(
                    "a simple chart summarizes one variable; pick clustered bars or multiple lines for more"
                        .into(),
                );
            }
            ChartKind::HighLow if !(2..=3).contains(&self.variables.len()) => {
                return Err(
                    "a high-low chart takes two or three variables: high, low and close".into(),
                );
            }
            _ => {}
        }
        let what = match kind {
            ChartKind::Pie => "the variable of the slices",
            _ => "the variable of the category axis",
        };
        spec.push_str(&format!(" BY {}", one(&self.category, what)?));
        let group = self.group.trim();
        match kind.group() {
            Some(title) if !group.is_empty() => {
                if many {
                    return Err(format!("with {title} set, summarize one variable"));
                }
                spec.push_str(&format!(" BY {group}"));
            }
            Some(title) if kind.needs_series() && !many => {
                return Err(format!("set {title}, or summarize two variables or more"));
            }
            _ => {}
        }
        Ok(spec)
    }

    pub fn syntax(&self) -> Result<String, String> {
        let group = self.group.trim();
        let chart = match self.kind {
            ChartKind::SimpleBar => format!("BAR(SIMPLE)={}", self.summary()?),
            ChartKind::ClusteredBar => format!("BAR(GROUPED)={}", self.summary()?),
            ChartKind::StackedBar => format!("BAR(STACKED)={}", self.summary()?),
            ChartKind::SimpleLine => format!("LINE(SIMPLE)={}", self.summary()?),
            ChartKind::MultipleLine => format!("LINE(MULTIPLE)={}", self.summary()?),
            ChartKind::Area => format!("LINE(AREA)={}", self.summary()?),
            ChartKind::Pie => format!("PIE={}", self.summary()?),
            ChartKind::HighLow => format!("HILO={}", self.summary()?),
            ChartKind::Pareto => {
                // several summed variables are stacked series, as a group variable is
                let many = self.function >= COUNT_FUNCTIONS && self.variables.len() > 1;
                let stacked = if group.is_empty() && !many {
                    "SIMPLE"
                } else {
                    "STACKED"
                };
                let line = if self.cumulative { "" } else { "(NOCUM)" };
                format!("PARETO{line}({stacked})={}", self.summary()?)
            }
            ChartKind::Histogram => {
                let normal = if self.normal { "(NORMAL)" } else { "" };
                format!("HISTOGRAM{normal}={}", one(&self.x, "the variable")?)
            }
            ChartKind::SimpleScatter | ChartKind::GroupedScatter => {
                let mut spec = format!(
                    "SCATTERPLOT(BIVARIATE)={} WITH {}",
                    one(&self.x, "the X axis")?,
                    one(&self.y, "the Y axis")?
                );
                if self.kind == ChartKind::GroupedScatter {
                    spec.push_str(&format!(" BY {}", one(group, "Groups of points by")?));
                }
                spec + &self.labels()
            }
            ChartKind::OverlayScatter => {
                let x = names(&self.variables, "X axis variables")?;
                let y = names(&self.y_variables, "Y axis variables")?;
                let (nx, ny) = (self.variables.len(), self.y_variables.len());
                if nx == 1 && ny == 1 {
                    return Err("an overlay needs a second variable on one of the axes".into());
                }
                let pair = if self.pair { " (PAIR)" } else { "" };
                if self.pair && nx != ny {
                    return Err("pairs in order need as many Y as X variables".into());
                }
                format!("SCATTERPLOT(OVERLAY)={x} WITH {y}{pair}{}", self.labels())
            }
            ChartKind::MatrixScatter => {
                if self.variables.len() < 2 {
                    return Err("a scatterplot matrix needs two variables or more".into());
                }
                let mut spec = format!(
                    "SCATTERPLOT(MATRIX)={}",
                    names(&self.variables, "Variables")?
                );
                if !group.is_empty() {
                    spec.push_str(&format!(" BY {group}"));
                }
                spec + &self.labels()
            }
            ChartKind::ErrorBar => {
                let mut spec = names(&self.variables, "Variables")?;
                let category = self.category.trim();
                if !category.is_empty() {
                    spec.push_str(&format!(" BY {category}"));
                }
                if !group.is_empty() {
                    if category.is_empty() {
                        return Err("set the variable of the category axis first".into());
                    }
                    if self.variables.len() > 1 {
                        return Err("with Clusters by set, add one variable".into());
                    }
                    spec.push_str(&format!(" BY {group}"));
                }
                let bars = match ERROR_BARS[self.error_bar].0 {
                    "CI" => {
                        let level = parse(&self.level, "the confidence level")?;
                        if !(50.0..=99.9).contains(&level) {
                            return Err("the confidence level must be from 50 to 99.9".into());
                        }
                        match level == 95.0 {
                            true => String::new(),
                            false => format!("(CI {})", number(level)),
                        }
                    }
                    keyword => {
                        let given = match keyword {
                            "STDDEV" => &self.deviations,
                            _ => &self.multiplier,
                        };
                        let n = parse(given, "the multiplier")?;
                        if n <= 0.0 {
                            return Err("the multiplier must be above 0".into());
                        }
                        format!("({keyword} {})", number(n))
                    }
                };
                format!("ERRORBAR{bars}={spec}")
            }
        };
        let mut text = format!("GRAPH /{chart}");
        for (keyword, given, what) in [
            ("TITLE", &self.title, "title"),
            ("SUBTITLE", &self.subtitle, "subtitle"),
            ("FOOTNOTE", &self.footnote, "footnote"),
        ] {
            if let Some(quoted) = line(given, what)? {
                text.push_str(&format!("\n  /{keyword}={quoted}"));
            }
        }
        if !self.panel_columns.is_empty() || !self.panel_rows.is_empty() {
            text.push_str("\n  /PANEL");
            if !self.panel_columns.is_empty() {
                text.push_str(&format!(" COLVAR={}", self.panel_columns.join(" ")));
            }
            if !self.panel_rows.is_empty() {
                text.push_str(&format!(" ROWVAR={}", self.panel_rows.join(" ")));
            }
        }
        let missing: Vec<&str> = [
            (self.by_variable, "VARIABLE"),
            (self.report, "REPORT"),
            (self.include, "INCLUDE"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword, label and the names of the parameters of each test distribution of PPLOT; an
/// empty name for a distribution with one parameter.
pub const PPLOT_DISTRIBUTIONS: [(&str, &str, [&str; 2]); 13] = [
    ("NORMAL", "Normal", ["Location", "Scale"]),
    ("EXPONENTIAL", "Exponential", ["Scale", ""]),
    ("WEIBULL", "Weibull", ["Scale", "Shape"]),
    ("PARETO", "Pareto", ["Threshold", "Shape"]),
    ("LNORMAL", "Lognormal", ["Scale", "Shape"]),
    ("BETA", "Beta", ["Shape 1", "Shape 2"]),
    ("GAMMA", "Gamma", ["Shape", "Scale"]),
    ("LOGISTIC", "Logistic", ["Location", "Scale"]),
    ("LAPLACE", "Laplace", ["Location", "Scale"]),
    ("UNIFORM", "Uniform", ["Minimum", "Maximum"]),
    ("HNORMAL", "Half-normal", ["Scale", ""]),
    ("CHI", "Chi-square", ["Degrees of freedom", ""]),
    ("STUDENT", "Student t", ["Degrees of freedom", ""]),
];

pub const PPLOT_FRACTIONS: [(&str, &str); 4] = [
    ("BLOM", "Blom"),
    ("RANKIT", "Rankit"),
    ("TUKEY", "Tukey"),
    ("VW", "Van der Waerden"),
];

pub const PPLOT_TIES: [(&str, &str); 4] = [
    ("MEAN", "Mean rank"),
    ("HIGH", "Highest rank"),
    ("LOW", "Lowest rank"),
    ("BREAK", "Consecutive ranks, ties share one"),
];

pub const PPLOT_PLOTS: [(&str, &str); 3] = [
    ("BOTH", "Plot and detrended plot"),
    ("NORMAL", "Plot only"),
    ("DETRENDED", "Detrended plot only"),
];

/// PPLOT of the variables, P-P or Q-Q.
#[derive(Clone, PartialEq, Debug)]
pub struct ProbabilityPlot {
    pub pp: bool,
    pub variables: Vec<String>,
    /// Index into `PPLOT_DISTRIBUTIONS`.
    pub distribution: usize,
    /// Estimate the parameters from the data instead of the given ones.
    pub estimate: bool,
    pub parameters: [String; 2],
    /// Index into `PPLOT_FRACTIONS`.
    pub fraction: usize,
    /// Index into `PPLOT_TIES`.
    pub ties: usize,
    /// Index into `PPLOT_PLOTS`.
    pub plot: usize,
    pub ln: bool,
    pub standardize: bool,
    pub diff: bool,
    pub diff_degree: String,
    pub sdiff: bool,
    pub sdiff_degree: String,
    pub period: String,
}

impl ProbabilityPlot {
    pub fn new(pp: bool) -> Self {
        Self {
            pp,
            variables: Vec::new(),
            distribution: 0,
            estimate: true,
            parameters: Default::default(),
            fraction: 0,
            ties: 0,
            plot: 0,
            ln: false,
            standardize: false,
            diff: false,
            diff_degree: "1".into(),
            sdiff: false,
            sdiff_degree: "1".into(),
            period: String::new(),
        }
    }

    pub fn title(&self) -> &'static str {
        if self.pp { "P-P Plots" } else { "Q-Q Plots" }
    }

    /// The degrees of freedom of chi-square and Student t cannot be estimated.
    pub fn must_give(&self) -> bool {
        matches!(PPLOT_DISTRIBUTIONS[self.distribution].0, "CHI" | "STUDENT")
    }

    fn distribution(&self) -> Result<Option<String>, String> {
        let (keyword, _, labels) = PPLOT_DISTRIBUTIONS[self.distribution];
        if self.estimate && !self.must_give() {
            return Ok((self.distribution > 0).then(|| keyword.to_string()));
        }
        let mut given = Vec::new();
        for (text, label) in self.parameters.iter().zip(labels) {
            if !label.is_empty() {
                given.push(parse(text, &label.to_lowercase())?);
            }
        }
        let ok = match (keyword, given.as_slice()) {
            ("NORMAL" | "LOGISTIC" | "LAPLACE", [_, scale]) => *scale > 0.0,
            ("UNIFORM", [low, high]) => low < high,
            _ => given.iter().all(|v| *v > 0.0),
        };
        if !ok {
            return Err(match keyword {
                "UNIFORM" => "the minimum must be below the maximum".into(),
                "NORMAL" | "LOGISTIC" | "LAPLACE" => "the scale must be positive".into(),
                _ => "the parameters must be positive".into(),
            });
        }
        let given: Vec<String> = given.into_iter().map(number).collect();
        Ok(Some(format!("{keyword}({})", given.join(","))))
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("PPLOT /VARIABLES={}", names(&self.variables, "Variables")?);
        if self.pp {
            text.push_str("\n  /TYPE=P-P");
        }
        if let Some(distribution) = self.distribution()? {
            text.push_str(&format!("\n  /DISTRIBUTION={distribution}"));
        }
        for (index, options, keyword) in [
            (self.fraction, &PPLOT_FRACTIONS, "FRACTION"),
            (self.ties, &PPLOT_TIES, "TIES"),
        ] {
            if index > 0 {
                text.push_str(&format!("\n  /{keyword}={}", options[index].0));
            }
        }
        if self.plot > 0 {
            text.push_str(&format!("\n  /PLOT={}", PPLOT_PLOTS[self.plot].0));
        }
        if self.ln {
            text.push_str("\n  /LN");
        }
        if self.standardize {
            text.push_str("\n  /STANDARDIZE");
        }
        if self.diff {
            let n = whole(&self.diff_degree, 1, "the degree of differencing")?.unwrap_or(1);
            text.push_str(&format!("\n  /DIFF={n}"));
        }
        if self.sdiff {
            let n = whole(&self.sdiff_degree, 1, "the degree of seasonal differencing")?;
            let Some(period) = whole(&self.period, 1, "the period")? else {
                return Err("give the period of the seasonal differencing".into());
            };
            text.push_str(&format!(
                "\n  /SDIFF={}\n  /PERIOD={period}",
                n.unwrap_or(1)
            ));
        }
        text.push('.');
        Ok(text)
    }
}

/// ROC of test variables against the positive value of a state variable.
#[derive(Clone, PartialEq, Debug)]
pub struct RocCurve {
    pub tests: Vec<String>,
    pub state: String,
    pub value: String,
    pub curve: bool,
    pub reference: bool,
    /// The standard error, interval and test of the area.
    pub se: bool,
    pub model_quality: bool,
    pub coordinates: bool,
    /// Smaller test values point to the positive state.
    pub small_positive: bool,
    /// A value equal to the cutoff counts as negative.
    pub exclude_cutoff: bool,
    /// The standard error under the bi-negative exponential distribution.
    pub negexpo: bool,
    pub level: String,
    pub include: bool,
}

impl Default for RocCurve {
    fn default() -> Self {
        Self {
            tests: Vec::new(),
            state: String::new(),
            value: String::new(),
            curve: true,
            reference: false,
            se: false,
            model_quality: false,
            coordinates: false,
            small_positive: false,
            exclude_cutoff: false,
            negexpo: false,
            level: "95".into(),
            include: false,
        }
    }
}

impl RocCurve {
    /// `string` tells whether the state variable is a string variable.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let tests = names(&self.tests, "Test variables")?;
        let state = one(&self.state, "the state variable")?;
        if self.value.trim().is_empty() {
            return Err("give the value of the positive state".into());
        }
        let positive = value(self.value.trim(), string)?;
        let mut text = format!("ROC {tests} BY {state} ({positive})");
        if !self.curve {
            text.push_str("\n  /PLOT=NONE");
        } else if self.reference {
            text.push_str("\n  /PLOT=CURVE(REFERENCE)");
        }
        let mut print = Vec::new();
        if self.se {
            print.push(if self.model_quality {
                "SE(MODELQUALITY)"
            } else {
                "SE"
            });
        }
        if self.coordinates {
            print.push("COORDINATES");
        }
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let mut criteria = Vec::new();
        if self.exclude_cutoff {
            criteria.push("CUTOFF(EXCLUDE)".to_string());
        }
        if self.small_positive {
            criteria.push("TESTPOS(SMALL)".into());
        }
        let level = percent(&self.level)?;
        if level != 95.0 {
            criteria.push(format!("CI({})", number(level)));
        }
        if self.negexpo {
            criteria.push("DISTRIBUTION(NEGEXPO)".into());
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    const MEAN: usize = 4;

    /// Parses the syntax, runs it on the survey file and returns the number of charts it drew.
    fn charts(syntax: &str) -> usize {
        let (_, errors) = statscraft_syntax::parse(syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let mut session = open("survey_basic.sav");
        let result = session
            .run(syntax, None)
            .unwrap_or_else(|e| panic!("{syntax}\n{e:?}"));
        result.commands.iter().map(|o| o.charts.len()).sum()
    }

    fn s(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn bar_line_area_and_pie_charts() {
        let mut d = ChartBuilder::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "set the variable of the category axis"
        );
        d.category = s("region");
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /BAR(SIMPLE)=COUNT BY region.");
        assert_eq!(charts(&syntax), 1);

        d.variables = v(&["age"]);
        d.function = MEAN;
        d.title = s("Age by region");
        d.footnote = s("Invented survey");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /BAR(SIMPLE)=MEAN(age) BY region\n  /TITLE='Age by region'\n  \
             /FOOTNOTE='Invented survey'."
        );
        assert_eq!(charts(&syntax), 1);

        // a simple bar summarizes one variable
        d.variables = v(&["age", "monthly_spend"]);
        assert!(
            d.syntax()
                .unwrap_err()
                .starts_with("a simple chart summarizes one variable")
        );
        d.variables = v(&["age"]);

        // clustered: a group variable or several variables
        d.kind = ChartKind::ClusteredBar;
        d.title.clear();
        d.footnote.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "set Clusters by, or summarize two variables or more"
        );
        d.group = s("gender");
        d.function = 15;
        d.low = s("75");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /BAR(GROUPED)=PTILE(age,75) BY region BY gender."
        );
        assert_eq!(charts(&syntax), 1);
        d.variables = v(&["age", "monthly_spend"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "with Clusters by set, summarize one variable"
        );
        d.group.clear();
        d.function = 20;
        d.low = s("30");
        d.high = s("20");
        assert_eq!(
            d.syntax().unwrap_err(),
            "the lower end of the range comes first"
        );
        d.high = s("50");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /BAR(GROUPED)=PIN(age monthly_spend,30,50) BY region."
        );
        assert_eq!(charts(&syntax), 1);

        // stacked counts, with panels and every missing-value option
        d.kind = ChartKind::StackedBar;
        d.function = 1;
        d.group = s("gender");
        d.panel_columns = v(&["q5_1"]);
        d.panel_rows = v(&["q5_2"]);
        d.by_variable = true;
        d.report = true;
        d.include = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /BAR(STACKED)=PCT BY region BY gender\n  /PANEL COLVAR=q5_1 ROWVAR=q5_2\n  \
             /MISSING=VARIABLE REPORT INCLUDE."
        );
        assert!(charts(&syntax) >= 4);

        // lines and areas
        d.panel_columns.clear();
        d.panel_rows.clear();
        d.by_variable = false;
        d.report = false;
        d.include = false;
        d.kind = ChartKind::MultipleLine;
        d.function = SUM;
        d.variables = v(&["monthly_spend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /LINE(MULTIPLE)=SUM(monthly_spend) BY region BY gender."
        );
        assert_eq!(charts(&syntax), 1);
        d.kind = ChartKind::SimpleLine;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /LINE(SIMPLE)=SUM(monthly_spend) BY region.");
        assert_eq!(charts(&syntax), 1);
        d.kind = ChartKind::Area;
        d.subtitle = s("Spend, it's monthly");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /LINE(AREA)=SUM(monthly_spend) BY region BY gender\n  \
             /SUBTITLE='Spend, it''s monthly'."
        );
        assert_eq!(charts(&syntax), 1);
        d.subtitle = "x".repeat(73);
        assert_eq!(
            d.syntax().unwrap_err(),
            "the subtitle has at most 72 characters"
        );
        d.subtitle.clear();

        // a pie of one series
        d.kind = ChartKind::Pie;
        d.variables = v(&["age", "monthly_spend"]);
        assert_eq!(d.syntax().unwrap_err(), "a pie summarizes one variable");
        d.function = 0;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /PIE=COUNT BY region.");
        assert_eq!(charts(&syntax), 1);
    }

    #[test]
    fn histogram_error_bar_pareto_and_high_low() {
        let mut d = ChartBuilder {
            kind: ChartKind::Histogram,
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "set the variable");
        d.x = s("age");
        assert_eq!(d.syntax().unwrap(), "GRAPH /HISTOGRAM=age.");
        d.normal = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /HISTOGRAM(NORMAL)=age.");
        assert_eq!(charts(&syntax), 1);

        d.kind = ChartKind::ErrorBar;
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["age", "monthly_spend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /ERRORBAR=age monthly_spend.");
        assert_eq!(charts(&syntax), 1);
        d.category = s("region");
        d.level = s("90");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /ERRORBAR(CI 90)=age monthly_spend BY region."
        );
        assert_eq!(charts(&syntax), 1);
        d.group = s("gender");
        assert_eq!(
            d.syntax().unwrap_err(),
            "with Clusters by set, add one variable"
        );
        d.variables = v(&["age"]);
        d.error_bar = 1;
        d.multiplier = s("1.5");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /ERRORBAR(STERROR 1.5)=age BY region BY gender."
        );
        assert_eq!(charts(&syntax), 1);
        d.error_bar = 2;
        d.deviations = s("0");
        assert_eq!(d.syntax().unwrap_err(), "the multiplier must be above 0");
        // standard errors and standard deviations keep their own counts
        d.deviations = s("3");
        assert!(d.syntax().unwrap().contains("(STDDEV 3)"));
        d.error_bar = 1;
        assert!(d.syntax().unwrap().contains("(STERROR 1.5)"));

        // Pareto: counts or sums, simple or stacked, with or without the line
        d.kind = ChartKind::Pareto;
        d.group.clear();
        d.function = MEAN;
        assert_eq!(
            d.syntax().unwrap_err(),
            "a Pareto chart shows the number of cases or a sum"
        );
        d.function = 0;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GRAPH /PARETO(SIMPLE)=COUNT BY region.");
        assert_eq!(charts(&syntax), 1);
        d.function = SUM;
        d.variables = v(&["monthly_spend"]);
        d.group = s("gender");
        d.cumulative = false;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /PARETO(NOCUM)(STACKED)=SUM(monthly_spend) BY region BY gender."
        );
        assert_eq!(charts(&syntax), 1);
        d.group.clear();
        d.variables = v(&["monthly_spend", "age"]);
        assert_eq!(
            d.syntax().unwrap(),
            "GRAPH /PARETO(NOCUM)(STACKED)=SUM(monthly_spend age) BY region."
        );
        d.variables = v(&["monthly_spend"]);

        // high-low of two or three variables
        d.kind = ChartKind::HighLow;
        d.function = 0;
        assert!(
            d.syntax()
                .unwrap_err()
                .starts_with("a high-low chart summarizes")
        );
        d.function = 11;
        assert_eq!(
            d.syntax().unwrap_err(),
            "a high-low chart takes two or three variables: high, low and close"
        );
        d.variables = v(&["age", "monthly_spend", "q2_recommend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /HILO=MAXIMUM(age monthly_spend q2_recommend) BY region."
        );
        assert_eq!(charts(&syntax), 1);
    }

    #[test]
    fn scatterplots() {
        let mut d = ChartBuilder {
            kind: ChartKind::SimpleScatter,
            x: s("age"),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "set the Y axis");
        d.y = s("monthly_spend");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /SCATTERPLOT(BIVARIATE)=age WITH monthly_spend."
        );
        assert_eq!(charts(&syntax), 1);
        assert_eq!(d.measured(), vec!["age", "monthly_spend"]);

        d.kind = ChartKind::GroupedScatter;
        assert_eq!(d.syntax().unwrap_err(), "set Groups of points by");
        d.group = s("gender");
        d.label = s("resp_id");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /SCATTERPLOT(BIVARIATE)=age WITH monthly_spend BY gender BY resp_id (NAME)."
        );
        assert_eq!(charts(&syntax), 1);

        d.kind = ChartKind::OverlayScatter;
        d.variables = v(&["age"]);
        d.y_variables = v(&["monthly_spend"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "an overlay needs a second variable on one of the axes"
        );
        d.y_variables = v(&["monthly_spend", "q2_recommend"]);
        d.label.clear();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /SCATTERPLOT(OVERLAY)=age WITH monthly_spend q2_recommend."
        );
        assert_eq!(charts(&syntax), 1);
        d.pair = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "pairs in order need as many Y as X variables"
        );
        d.variables = v(&["age", "q1_satisfaction"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /SCATTERPLOT(OVERLAY)=age q1_satisfaction WITH monthly_spend q2_recommend \
             (PAIR)."
        );
        assert_eq!(charts(&syntax), 1);

        d.kind = ChartKind::MatrixScatter;
        d.variables = v(&["age"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "a scatterplot matrix needs two variables or more"
        );
        d.variables = v(&["age", "monthly_spend", "q2_recommend"]);
        d.label = s("resp_id");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GRAPH /SCATTERPLOT(MATRIX)=age monthly_spend q2_recommend BY gender BY resp_id \
             (NAME)."
        );
        assert_eq!(charts(&syntax), 1);
    }

    #[test]
    fn probability_plots() {
        let mut d = ProbabilityPlot::new(false);
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["age"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "PPLOT /VARIABLES=age.");
        assert_eq!(charts(&syntax), 2);

        // a given normal, every option of the ranks, the plot and the changes of the values
        d.estimate = false;
        d.parameters = [s("40"), s("-1")];
        assert_eq!(d.syntax().unwrap_err(), "the scale must be positive");
        d.parameters[1] = s("12.5");
        d.fraction = 3;
        d.ties = 3;
        d.plot = 1;
        d.ln = true;
        d.standardize = true;
        d.diff = true;
        d.diff_degree = s("2");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PPLOT /VARIABLES=age\n  /DISTRIBUTION=NORMAL(40,12.5)\n  /FRACTION=VW\n  \
             /TIES=BREAK\n  /PLOT=NORMAL\n  /LN\n  /STANDARDIZE\n  /DIFF=2."
        );
        assert_eq!(charts(&syntax), 1);

        // P-P against an estimated logistic, detrended only, seasonal differencing
        let mut d = ProbabilityPlot::new(true);
        d.variables = v(&["age", "monthly_spend"]);
        d.distribution = 7;
        d.plot = 2;
        d.sdiff = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the period of the seasonal differencing"
        );
        d.period = s("4");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PPLOT /VARIABLES=age monthly_spend\n  /TYPE=P-P\n  /DISTRIBUTION=LOGISTIC\n  \
             /PLOT=DETRENDED\n  /SDIFF=1\n  /PERIOD=4."
        );
        assert_eq!(charts(&syntax), 2);

        // chi-square and Student t always need their degrees of freedom
        d.sdiff = false;
        d.plot = 0;
        d.distribution = 11;
        assert!(d.must_give());
        assert_eq!(
            d.syntax().unwrap_err(),
            "degrees of freedom must be a number"
        );
        d.parameters[0] = s("30");
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PPLOT /VARIABLES=age monthly_spend\n  /TYPE=P-P\n  /DISTRIBUTION=CHI(30)."
        );
        assert_eq!(charts(&syntax), 4);
        d.distribution = 9;
        d.estimate = false;
        d.parameters = [s("100"), s("0")];
        assert_eq!(
            d.syntax().unwrap_err(),
            "the minimum must be below the maximum"
        );
    }

    #[test]
    fn roc_curves() {
        let mut d = RocCurve::default();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "add at least one variable to Test variables"
        );
        d.tests = v(&["age"]);
        d.state = s("q5_1");
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "give the value of the positive state"
        );
        d.value = s("1");
        let syntax = d.syntax(false).unwrap();
        assert_eq!(syntax, "ROC age BY q5_1 (1).");
        assert_eq!(charts(&syntax), 1);

        // every option
        d.tests = v(&["age", "monthly_spend"]);
        d.reference = true;
        d.se = true;
        d.model_quality = true;
        d.coordinates = true;
        d.small_positive = true;
        d.exclude_cutoff = true;
        d.negexpo = true;
        d.level = s("90");
        d.include = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "ROC age monthly_spend BY q5_1 (1)\n  /PLOT=CURVE(REFERENCE)\n  \
             /PRINT=SE(MODELQUALITY) COORDINATES\n  \
             /CRITERIA=CUTOFF(EXCLUDE) TESTPOS(SMALL) CI(90) DISTRIBUTION(NEGEXPO)\n  \
             /MISSING=INCLUDE."
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty());

        // no chart; a string state value is quoted
        d.curve = false;
        d.se = false;
        d.coordinates = false;
        d.small_positive = false;
        d.exclude_cutoff = false;
        d.negexpo = false;
        d.level = s("100");
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the confidence level must be a percentage between 0 and 100"
        );
        d.level = s("95");
        d.include = false;
        d.state = s("plan");
        d.value = s(" basic ");
        let syntax = d.syntax(true).unwrap();
        assert_eq!(
            syntax,
            "ROC age monthly_spend BY plan ('basic')\n  /PLOT=NONE."
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty());
    }
}
