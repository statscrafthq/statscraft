//! The time series dialogs: Autocorrelations (ACF or PACF), Cross-Correlations (CCF), Seasonal
//! Decomposition (SEASON), Create Time Series (CREATE), the time series options (TSET) and
//! Define Dates (DATE). Options left as they are add nothing to the syntax, so the command runs
//! with its own defaults and the session's TSET settings.

use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, percent};
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};

/// One dialog of this module.
#[derive(Clone, PartialEq, Debug)]
pub enum TimeSeries {
    Correlogram(Correlogram),
    Season(Season),
    Create(CreateSeries),
    Options(SeriesOptions),
    Dates(Dates),
}

/// The Analyze > Forecasting menu, in order.
pub fn forecasting_menu() -> [(&'static str, TimeSeries); 4] {
    [
        (
            "Autocorrelations...",
            TimeSeries::Correlogram(Default::default()),
        ),
        (
            "Cross-Correlations...",
            TimeSeries::Correlogram(Correlogram::new(true)),
        ),
        (
            "Seasonal Decomposition...",
            TimeSeries::Season(Default::default()),
        ),
        ("Options...", TimeSeries::Options(Default::default())),
    ]
}

impl TimeSeries {
    pub fn title(&self) -> &'static str {
        match self {
            TimeSeries::Correlogram(d) if d.cross => "Cross-Correlations",
            TimeSeries::Correlogram(_) => "Autocorrelations",
            TimeSeries::Season(_) => "Seasonal Decomposition",
            TimeSeries::Create(_) => "Create Time Series",
            TimeSeries::Options(_) => "Time Series Options",
            TimeSeries::Dates(_) => "Define Dates",
        }
    }

    /// The same dialog with its defaults.
    pub fn reset(&self) -> Self {
        match self {
            TimeSeries::Correlogram(d) => TimeSeries::Correlogram(Correlogram::new(d.cross)),
            TimeSeries::Season(_) => TimeSeries::Season(Default::default()),
            TimeSeries::Create(_) => TimeSeries::Create(Default::default()),
            TimeSeries::Options(_) => TimeSeries::Options(Default::default()),
            TimeSeries::Dates(_) => TimeSeries::Dates(Default::default()),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            TimeSeries::Correlogram(d) => d.syntax(data),
            TimeSeries::Season(d) => d.syntax(data),
            TimeSeries::Create(d) => d.syntax(data),
            TimeSeries::Options(d) => d.syntax(),
            TimeSeries::Dates(d) => d.syntax(),
        }
    }
}

/// What the Autocorrelations dialog shows; the second runs PACF instead of ACF.
pub const CORRELOGRAM_DISPLAY: [&str; 3] = ["Autocorrelations", "Partial autocorrelations", "Both"];

/// ACF, PACF or (`cross`) CCF, on series that may be logged and differenced first.
#[derive(Clone, PartialEq, Debug)]
pub struct Correlogram {
    pub cross: bool,
    pub variables: Vec<String>,
    /// LN: the natural log before any differencing.
    pub ln: bool,
    pub diff: bool,
    pub diff_order: String,
    pub sdiff: bool,
    pub sdiff_order: String,
    /// Empty: the TSET or DATE period. Read only with `sdiff`.
    pub period: String,
    /// MXAUTO or MXCROSS; empty: the TSET setting.
    pub max_lag: String,
    /// Index into `CORRELOGRAM_DISPLAY` (ACF only).
    pub display: usize,
    /// SERROR=MA, Bartlett's standard errors (ACF only, not with partials alone).
    pub ma_errors: bool,
}

impl Default for Correlogram {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Correlogram {
    pub fn new(cross: bool) -> Self {
        Self {
            cross,
            variables: Vec::new(),
            ln: false,
            diff: false,
            diff_order: "1".into(),
            sdiff: false,
            sdiff_order: "1".into(),
            period: String::new(),
            max_lag: String::new(),
            display: 0,
            ma_errors: false,
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.cross && self.variables.len() < 2 {
            return Err("add at least two series".into());
        }
        if self.variables.is_empty() {
            return Err("add at least one series".into());
        }
        numeric(data, &self.variables)?;
        let command = match (self.cross, self.display) {
            (true, _) => "CCF",
            (false, 1) => "PACF",
            _ => "ACF",
        };
        let mut text = format!("{command} VARIABLES={}", self.variables.join(" "));
        if self.ln {
            text.push_str("\n  /LN");
        }
        if self.diff {
            let n = whole(&self.diff_order, 1, "the degree of differencing")?.unwrap_or(1);
            text.push_str(&format!("\n  /DIFF={n}"));
        }
        if self.sdiff {
            let n =
                whole(&self.sdiff_order, 1, "the degree of seasonal differencing")?.unwrap_or(1);
            text.push_str(&format!("\n  /SDIFF={n}"));
            if let Some(p) = whole(&self.period, 2, "the period")? {
                text.push_str(&format!("\n  /PERIOD={p}"));
            }
        }
        if let Some(n) = whole(&self.max_lag, 1, "the maximum lag")? {
            let keyword = if self.cross { "MXCROSS" } else { "MXAUTO" };
            text.push_str(&format!("\n  /{keyword}={n}"));
        }
        if command == "ACF" {
            if self.ma_errors {
                text.push_str("\n  /SERROR=MA");
            }
            if self.display == 2 {
                text.push_str("\n  /PACF");
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// SEASON: seasonal factors, adjusted series, trend-cycle and irregular parts.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Season {
    pub variables: Vec<String>,
    pub additive: bool,
    /// MA=EQUAL: every case of the span weighs the same, also for an even period.
    pub equal: bool,
    /// Empty: the TSET or DATE period.
    pub period: String,
}

impl Season {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.is_empty() {
            return Err("add at least one series".into());
        }
        numeric(data, &self.variables)?;
        let mut text = format!("SEASON VARIABLES={}", self.variables.join(" "));
        if self.additive {
            text.push_str("\n  /MODEL=ADDITIVE");
        }
        if self.equal {
            text.push_str("\n  /MA=EQUAL");
        }
        if let Some(p) = whole(&self.period, 2, "the period")? {
            text.push_str(&format!("\n  /PERIOD={p}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// A function of CREATE: keyword, label, the label of its first and second number (empty for
/// none; the second is optional) and the first number a new row starts with.
pub struct SeriesFunction {
    pub keyword: &'static str,
    pub label: &'static str,
    pub first: &'static str,
    pub second: &'static str,
    pub start: &'static str,
}

const fn function(
    keyword: &'static str,
    label: &'static str,
    first: &'static str,
    second: &'static str,
    start: &'static str,
) -> SeriesFunction {
    SeriesFunction {
        keyword,
        label,
        first,
        second,
        start,
    }
}

pub const SERIES_FUNCTIONS: [SeriesFunction; 11] = [
    function("CSUM", "Running total", "", "", ""),
    function("DIFF", "Difference", "Order", "", "1"),
    function("SDIFF", "Seasonal difference", "Order", "Period", "1"),
    function("LAG", "Lag", "Order", "Last order", "1"),
    function("LEAD", "Lead", "Order", "Last order", "1"),
    function("MA", "Centered moving average", "Span", "Minimum span", "3"),
    function("RMED", "Running median", "Span", "Minimum span", "3"),
    function("PMA", "Prior moving average", "Span", "", "3"),
    function("T4253H", "Smoothing (T4253H)", "", "", ""),
    function("FFT", "Fast Fourier transform", "", "", ""),
    function("IFFT", "Inverse Fourier transform", "", "", ""),
];

/// The index of IFFT, which reads a real and an imaginary series.
pub const IFFT: usize = 10;

/// One equation of CREATE: new names = FUNCTION(sources, numbers).
#[derive(Clone, PartialEq, Debug)]
pub struct CreateRow {
    /// Separated by blanks or commas.
    pub names: String,
    /// Index into `SERIES_FUNCTIONS`.
    pub function: usize,
    /// One series; IFFT a real and an imaginary part.
    pub sources: Vec<String>,
    pub first: String,
    pub second: String,
}

impl CreateRow {
    /// A row for `sources`, named after the first: sales_lag; FFT names its two parts.
    pub fn new(function: usize, sources: Vec<String>) -> Self {
        let f = &SERIES_FUNCTIONS[function];
        let root = &sources[0];
        let names = match f.keyword {
            "FFT" => format!("{root}_re {root}_im"),
            k => format!("{root}_{}", k.to_lowercase()),
        };
        Self {
            names,
            function,
            sources,
            first: f.start.into(),
            second: String::new(),
        }
    }

    /// The equation, checked; `taken` gathers the new names of the rows before.
    fn equation(&self, data: &Dataset, taken: &mut Vec<String>) -> Result<String, String> {
        let f = &SERIES_FUNCTIONS[self.function];
        numeric(data, &self.sources)?;
        let mut numbers = Vec::new();
        let mut made = 1;
        if !f.first.is_empty() {
            let lag = matches!(f.keyword, "LAG" | "LEAD");
            let what = format!("the {} of {}", f.first.to_lowercase(), f.keyword);
            let first = whole(&self.first, u32::from(!lag), &what)?
                .ok_or_else(|| format!("type {what}"))?;
            numbers.push(first);
            let what = format!("the {} of {}", f.second.to_lowercase(), f.keyword);
            if let Some(second) = whole(&self.second, u32::from(!lag), &what)? {
                if lag && second < first {
                    return Err(format!("{what} must be at least the order"));
                }
                if matches!(f.keyword, "MA" | "RMED") && second > first {
                    return Err(format!("{what} must be at most the span"));
                }
                if lag {
                    made = second - first + 1;
                }
                numbers.push(second);
            }
        } else if f.keyword == "FFT" {
            made = 2;
        }
        let names: Vec<&str> = self
            .names
            .split([' ', ','])
            .filter(|n| !n.is_empty())
            .collect();
        if names.len() as u32 != made {
            let plural = if made == 1 { "" } else { "s" };
            return Err(format!(
                "{} of {} makes {made} series: give {made} new name{plural}",
                f.keyword, self.sources[0]
            ));
        }
        for name in &names {
            // the command makes no scratch or system variables
            if !is_valid_name(name) || name.starts_with(['#', '$']) {
                return Err(format!("{name} cannot name a variable"));
            }
            if data.variable(name).is_ok() {
                return Err(format!("{name} is already a variable"));
            }
            if taken.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                return Err(format!("{name} is given twice"));
            }
            taken.push(name.to_string());
        }
        let arguments: Vec<String> = self
            .sources
            .iter()
            .cloned()
            .chain(numbers.iter().map(u32::to_string))
            .collect();
        Ok(format!(
            "{} = {}({})",
            names.join(" "),
            f.keyword,
            arguments.join(", ")
        ))
    }
}

/// CREATE: new series, one row each.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct CreateSeries {
    pub rows: Vec<CreateRow>,
    /// The function of the next rows, an index into `SERIES_FUNCTIONS`.
    pub function: usize,
}

impl CreateSeries {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.rows.is_empty() {
            return Err("add at least one new series".into());
        }
        let mut taken = Vec::new();
        let equations = self
            .rows
            .iter()
            .map(|row| row.equation(data, &mut taken))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("CREATE {}.", equations.join("\n  /")))
    }
}

/// NEWVAR choices, the starting one first.
pub const TSET_NEWVAR: [(&str, &str); 3] = [
    (
        "CURRENT",
        "Save them, replacing those of the last procedure",
    ),
    ("NONE", "Do not save them"),
    ("ALL", "Save them, keeping the earlier ones"),
];

/// PRINT choices, the starting one first.
pub const TSET_PRINT: [(&str, &str); 3] = [
    ("DEFAULT", "Default"),
    ("BRIEF", "Brief"),
    ("DETAILED", "Detailed"),
];

/// TSET: the settings the time series procedures read. The session does not show its current
/// values, so the dialog starts from the starting values and writes only what differs.
#[derive(Clone, PartialEq, Debug)]
pub struct SeriesOptions {
    /// DEFAULT first: every other setting goes back to its starting value.
    pub reset: bool,
    /// Index into `TSET_NEWVAR`.
    pub newvar: usize,
    pub mxnewvars: String,
    pub mxauto: String,
    pub mxcross: String,
    /// Empty: none set.
    pub period: String,
    pub cin: String,
    /// MISSING=INCLUDE
    pub include: bool,
    /// ACFSE=MA
    pub acfse_ma: bool,
    /// Index into `TSET_PRINT`.
    pub print: usize,
}

impl Default for SeriesOptions {
    fn default() -> Self {
        Self {
            reset: false,
            newvar: 0,
            mxnewvars: "60".into(),
            mxauto: "16".into(),
            mxcross: "7".into(),
            period: String::new(),
            cin: "95".into(),
            include: false,
            acfse_ma: false,
            print: 0,
        }
    }
}

impl SeriesOptions {
    pub fn syntax(&self) -> Result<String, String> {
        let mut items = Vec::new();
        if self.reset {
            items.push("DEFAULT".to_string());
        }
        if self.newvar > 0 {
            items.push(format!("NEWVAR={}", TSET_NEWVAR[self.newvar].0));
        }
        for (keyword, text, start, what) in [
            ("MXNEWVARS", &self.mxnewvars, 60, "the most new variables"),
            ("MXAUTO", &self.mxauto, 16, "the most autocorrelation lags"),
            (
                "MXCROSS",
                &self.mxcross,
                7,
                "the most cross-correlation lags",
            ),
        ] {
            match whole(text, 1, what)? {
                Some(n) if n != start => items.push(format!("{keyword}={n}")),
                _ => {}
            }
        }
        if let Some(p) = whole(&self.period, 1, "the period")? {
            items.push(format!("PERIOD={p}"));
        }
        if !self.cin.trim().is_empty() {
            let level = percent(&self.cin)?;
            if level != 95.0 {
                items.push(format!("CIN={}", number(level)));
            }
        }
        if self.include {
            items.push("MISSING=INCLUDE".into());
        }
        if self.acfse_ma {
            items.push("ACFSE=MA".into());
        }
        if self.print > 0 {
            items.push(format!("PRINT={}", TSET_PRINT[self.print].0));
        }
        if items.is_empty() {
            return Err("change a setting, or start from the defaults".into());
        }
        Ok(format!("TSET {}.", items.join(" ")))
    }
}

/// One date variable of a case structure: keyword, the label of its first value (empty: not
/// asked, as QUARTER beside MONTH, which follows the month), the period written after the
/// first value and the period in effect, which bounds the first value.
pub type DatePart = (&'static str, &'static str, Option<u32>, Option<u32>);

const YEAR: DatePart = ("YEAR", "Year", None, None);
const WEEK: DatePart = ("WEEK", "Week", None, None);
const DAY: DatePart = ("DAY", "Day", None, None);
const DAY_OF_WEEK: DatePart = ("DAY", "Day", None, Some(7));
const WORK_DAY: DatePart = ("DAY", "Day", Some(5), Some(5));
const HOUR: DatePart = ("HOUR", "Hour", None, None);
const HOUR_OF_DAY: DatePart = ("HOUR", "Hour", None, Some(24));
const MINUTE: DatePart = ("MINUTE", "Minute", None, None);
const MINUTE_OF_HOUR: DatePart = ("MINUTE", "Minute", None, Some(60));
const SECOND_OF_MINUTE: DatePart = ("SECOND", "Second", None, Some(60));

/// The case structures Define Dates offers, each a label and its date variables, highest
/// first. The last one has a period of the user's own.
pub const DATE_CASES: [(&str, &[DatePart]); 20] = [
    ("Years", &[YEAR]),
    (
        "Years, quarters",
        &[YEAR, ("QUARTER", "Quarter", None, Some(4))],
    ),
    ("Years, months", &[YEAR, ("MONTH", "Month", None, Some(12))]),
    (
        "Years, quarters, months",
        &[
            YEAR,
            ("QUARTER", "", None, Some(4)),
            ("MONTH", "Month", None, Some(12)),
        ],
    ),
    ("Days", &[DAY]),
    ("Weeks, days", &[WEEK, DAY_OF_WEEK]),
    ("Weeks, work days (5)", &[WEEK, WORK_DAY]),
    (
        "Weeks, work days (6)",
        &[WEEK, ("DAY", "Day", Some(6), Some(6))],
    ),
    ("Hours", &[HOUR]),
    ("Days, hours", &[DAY, HOUR_OF_DAY]),
    (
        "Days, work hours (8)",
        &[DAY, ("HOUR", "Hour", Some(8), Some(8))],
    ),
    ("Weeks, days, hours", &[WEEK, DAY_OF_WEEK, HOUR_OF_DAY]),
    (
        "Weeks, work days (5), hours (8)",
        &[WEEK, WORK_DAY, ("HOUR", "Hour", Some(8), Some(8))],
    ),
    ("Minutes", &[MINUTE]),
    ("Hours, minutes", &[HOUR, MINUTE_OF_HOUR]),
    ("Days, hours, minutes", &[DAY, HOUR_OF_DAY, MINUTE_OF_HOUR]),
    ("Seconds", &[("SECOND", "Second", None, None)]),
    ("Minutes, seconds", &[MINUTE, SECOND_OF_MINUTE]),
    (
        "Hours, minutes, seconds",
        &[HOUR, MINUTE_OF_HOUR, SECOND_OF_MINUTE],
    ),
    (
        "Cycles of a period you choose",
        &[
            ("CYCLE", "Cycle", None, None),
            ("OBS", "Observation", None, None),
        ],
    ),
];

/// DATE: date variables that count the cases from a first case.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Dates {
    /// Index into `DATE_CASES`.
    pub case: usize,
    /// The first value of each date variable; empty: its starting value.
    pub starts: [String; 3],
    /// The period of OBS.
    pub period: String,
}

impl Dates {
    pub fn syntax(&self) -> Result<String, String> {
        let mut words = Vec::new();
        for (i, &(keyword, label, written, effect)) in DATE_CASES[self.case].1.iter().enumerate() {
            let (written, effect) = if keyword == "OBS" {
                let p = whole(&self.period, 1, "the period of the cycle")?
                    .ok_or("type the period of the cycle")?;
                (Some(p), Some(p))
            } else {
                (written, effect)
            };
            let what = format!("the first {}", label.to_lowercase());
            let start = match label {
                "" => None,
                _ => whole(&self.starts[i], 0, &what)?,
            };
            let base = u32::from(!matches!(keyword, "HOUR" | "MINUTE" | "SECOND"));
            if let (Some(s), Some(p)) = (start, effect)
                && (s < base || s > base + p - 1)
            {
                return Err(format!("{what} must be from {base} to {}", base + p - 1));
            }
            words.push(keyword.to_string());
            // a period follows a first value
            match (start, written) {
                (s, Some(p)) => words.push(format!("{} {p}", s.unwrap_or(base))),
                (Some(s), None) => words.push(s.to_string()),
                (None, None) => {}
            }
        }
        Ok(format!("DATE {}.", words.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    fn data() -> Dataset {
        open("survey_basic.sav").dataset().unwrap().clone()
    }

    #[test]
    fn correlograms_write_only_what_changed_and_check_their_fields() {
        let data = data();
        let mut d = Correlogram {
            variables: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax(&data).unwrap(), "ACF VARIABLES=age.");
        // the settings of a transform left off are not checked
        d.diff_order = "x".into();
        d.sdiff_order = "x".into();
        d.period = "1".into();
        assert_eq!(d.syntax(&data).unwrap(), "ACF VARIABLES=age.");
        d.sdiff = true;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the degree of seasonal differencing must be a whole number of at least 1"
        );
        d.sdiff_order = "".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the period must be a whole number of at least 2"
        );
        d.period = "".into();
        assert_eq!(d.syntax(&data).unwrap(), "ACF VARIABLES=age\n  /SDIFF=1.");
        // partial autocorrelations alone run PACF, which has no standard error choice
        d.display = 1;
        d.ma_errors = true;
        assert_eq!(d.syntax(&data).unwrap(), "PACF VARIABLES=age\n  /SDIFF=1.");
        d.variables.push("plan".into());
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "plan is a string; the command needs numbers"
        );
        let mut c = Correlogram {
            variables: v(&["age"]),
            ..Correlogram::new(true)
        };
        assert_eq!(c.syntax(&data).unwrap_err(), "add at least two series");
        c.variables.push("monthly_spend".into());
        c.max_lag = "0".into();
        assert_eq!(
            c.syntax(&data).unwrap_err(),
            "the maximum lag must be a whole number of at least 1"
        );
    }

    #[test]
    fn create_checks_names_numbers_and_counts() {
        let data = data();
        let row = |function: usize| CreateRow::new(function, v(&["age"]));
        let e = |rows: Vec<CreateRow>| {
            CreateSeries { rows, function: 0 }
                .syntax(&data)
                .unwrap_err()
        };
        assert_eq!(e(vec![]), "add at least one new series");
        assert_eq!(row(9).names, "age_re age_im");
        let mut lag = row(3);
        lag.second = "3".into();
        assert_eq!(
            e(vec![lag.clone()]),
            "LAG of age makes 3 series: give 3 new names"
        );
        lag.names = "a1 a2 a3".into();
        lag.first = "4".into();
        assert_eq!(
            e(vec![lag.clone()]),
            "the last order of LAG must be at least the order"
        );
        let mut ma = row(5);
        ma.second = "4".into();
        assert_eq!(
            e(vec![ma]),
            "the minimum span of MA must be at most the span"
        );
        let mut diff = row(1);
        diff.first = "".into();
        assert_eq!(e(vec![diff]), "type the order of DIFF");
        let mut taken = row(0);
        taken.names = "age".into();
        assert_eq!(e(vec![taken]), "age is already a variable");
        let mut scratch = row(0);
        scratch.names = "#t".into();
        assert_eq!(e(vec![scratch]), "#t cannot name a variable");
        assert_eq!(e(vec![row(0), row(0)]), "age_csum is given twice");
        // a lag of 0 orders is a copy, which the command takes
        lag.first = "0".into();
        lag.second = "2".into();
        let mut d = CreateSeries {
            rows: vec![lag, row(IFFT)],
            function: 0,
        };
        d.rows[1].sources = v(&["age", "monthly_spend"]);
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CREATE a1 a2 a3 = LAG(age, 0, 2)\n  /age_ifft = IFFT(age, monthly_spend)."
        );
    }

    #[test]
    fn options_write_only_changed_settings() {
        let mut d = SeriesOptions::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "change a setting, or start from the defaults"
        );
        d.reset = true;
        assert_eq!(d.syntax().unwrap(), "TSET DEFAULT.");
        d.cin = "100".into();
        assert!(d.syntax().is_err());
        d.cin = "".into();
        d.mxauto = "0".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the most autocorrelation lags must be a whole number of at least 1"
        );
    }

    #[test]
    fn dates_check_the_first_values() {
        let d = |case: usize, starts: [&str; 3]| Dates {
            case,
            starts: starts.map(String::from),
            period: String::new(),
        };
        assert_eq!(d(0, ["", "", ""]).syntax().unwrap(), "DATE YEAR.");
        assert_eq!(
            d(2, ["2019", "13", ""]).syntax().unwrap_err(),
            "the first month must be from 1 to 12"
        );
        assert_eq!(
            d(3, ["2019", "", "5"]).syntax().unwrap(),
            "DATE YEAR 2019 QUARTER MONTH 5."
        );
        assert_eq!(
            d(10, ["3", "", ""]).syntax().unwrap(),
            "DATE DAY 3 HOUR 0 8."
        );
        assert_eq!(
            d(10, ["", "8", ""]).syntax().unwrap_err(),
            "the first hour must be from 0 to 7"
        );
        assert_eq!(
            d(19, ["", "", ""]).syntax().unwrap_err(),
            "type the period of the cycle"
        );
    }
}
