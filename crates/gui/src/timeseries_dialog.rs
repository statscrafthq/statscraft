//! The time series dialogs: Autocorrelations (ACF or PACF), Cross-Correlations (CCF), Seasonal
//! Decomposition (SEASON), Create Time Series (CREATE), the time series options (TSET), Define
//! Dates (DATE), Exponential Smoothing (EXSMOOTH), ARIMA, the historical and forecast periods
//! (USE and PREDICT) and the sequence and case charts (TSPLOT and CASEPLOT). Options left as
//! they are add nothing to the syntax, so the command runs with its own defaults and the
//! session's TSET settings.

use crate::dialogs::whole;
use crate::forecast_more_dialog::{Areg, Spectra, TwoSls};
use crate::stats_dialogs::{numeric, parse, percent};
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};
use statscraft_syntax::DateUnit;

/// One dialog of this module.
#[derive(Clone, PartialEq, Debug)]
pub enum TimeSeries {
    Correlogram(Correlogram),
    Season(Season),
    Create(CreateSeries),
    Options(SeriesOptions),
    Dates(Dates),
    Smoothing(ExpSmoothing),
    Arima(ArimaModel),
    Range(TimeRange),
    Chart(SeriesChart),
    Spectra(Spectra),
    Areg(Areg),
    TwoSls(TwoSls),
}

/// The Analyze > Forecasting menu, in order.
pub fn forecasting_menu() -> [(&'static str, TimeSeries); 10] {
    [
        (
            "Exponential Smoothing...",
            TimeSeries::Smoothing(Default::default()),
        ),
        ("ARIMA...", TimeSeries::Arima(Default::default())),
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
        (
            "Sequence Charts...",
            TimeSeries::Chart(SeriesChart::new(false)),
        ),
        (
            "Forecast Period...",
            TimeSeries::Range(TimeRange::new(true)),
        ),
        (
            "Spectral Analysis...",
            TimeSeries::Spectra(Default::default()),
        ),
        ("Autoregression...", TimeSeries::Areg(Default::default())),
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
            TimeSeries::Smoothing(_) => "Exponential Smoothing",
            TimeSeries::Arima(_) => "ARIMA",
            TimeSeries::Range(d) if d.predict => "Forecast Period",
            TimeSeries::Range(_) => "Select Cases by Time Range",
            TimeSeries::Chart(d) if d.casewise => "Case Plots",
            TimeSeries::Chart(_) => "Sequence Charts",
            TimeSeries::Spectra(_) => "Spectral Analysis",
            TimeSeries::Areg(_) => "Autoregression",
            TimeSeries::TwoSls(_) => "2-Stage Least Squares",
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
            TimeSeries::Smoothing(_) => TimeSeries::Smoothing(Default::default()),
            TimeSeries::Arima(_) => TimeSeries::Arima(Default::default()),
            TimeSeries::Range(d) => TimeSeries::Range(TimeRange::new(d.predict)),
            TimeSeries::Chart(d) => TimeSeries::Chart(SeriesChart::new(d.casewise)),
            TimeSeries::Spectra(_) => TimeSeries::Spectra(Default::default()),
            TimeSeries::Areg(_) => TimeSeries::Areg(Default::default()),
            TimeSeries::TwoSls(_) => TimeSeries::TwoSls(Default::default()),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            TimeSeries::Correlogram(d) => d.syntax(data),
            TimeSeries::Season(d) => d.syntax(data),
            TimeSeries::Create(d) => d.syntax(data),
            TimeSeries::Options(d) => d.syntax(),
            TimeSeries::Dates(d) => d.syntax(),
            TimeSeries::Smoothing(d) => d.syntax(data),
            TimeSeries::Arima(d) => d.syntax(data),
            TimeSeries::Range(d) => d.syntax(data),
            TimeSeries::Chart(d) => d.syntax(data),
            TimeSeries::Spectra(d) => d.syntax(data),
            TimeSeries::Areg(d) => d.syntax(data),
            TimeSeries::TwoSls(d) => d.syntax(data),
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
        transforms(
            &mut text,
            self.ln,
            self.diff.then_some(&self.diff_order),
            self.sdiff.then_some(&self.sdiff_order),
            &self.period,
        )?;
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

/// LN, DIFF, SDIFF and PERIOD of the correlograms and the charts, added to `text`. The order of
/// a transform left off (None) is not checked, nor the period without SDIFF.
fn transforms(
    text: &mut String,
    ln: bool,
    diff: Option<&String>,
    sdiff: Option<&String>,
    period: &str,
) -> Result<(), String> {
    if ln {
        text.push_str("\n  /LN");
    }
    if let Some(order) = diff {
        let n = whole(order, 1, "the degree of differencing")?.unwrap_or(1);
        text.push_str(&format!("\n  /DIFF={n}"));
    }
    if let Some(order) = sdiff {
        let n = whole(order, 1, "the degree of seasonal differencing")?.unwrap_or(1);
        text.push_str(&format!("\n  /SDIFF={n}"));
        if let Some(p) = whole(period, 2, "the period")? {
            text.push_str(&format!("\n  /PERIOD={p}"));
        }
    }
    Ok(())
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

/// The date variables DATE made in the data, highest first.
pub fn date_units(data: &Dataset) -> Vec<DateUnit> {
    let mut units = DateUnit::ALL;
    units.sort();
    units
        .into_iter()
        .filter(|u| data.variable(&format!("{}_", u.name())).is_ok())
        .collect()
}

/// The period of a seasonal model; empty takes TSET PERIOD or the date variables. The window
/// cannot see TSET, so without date variables the period has to be typed.
fn seasonal_period(data: &Dataset, period: &str) -> Result<Option<u32>, String> {
    let p = whole(period, 2, "the period")?;
    if p.is_none() && date_units(data).is_empty() {
        return Err("a seasonal model needs a period: type it, or define dates first".into());
    }
    Ok(p)
}

/// Subcommands after a command and its variables, one per line.
fn command(head: String, subcommands: Vec<String>) -> String {
    let mut text = head;
    for s in subcommands {
        text.push_str(&format!("\n  /{s}"));
    }
    text.push('.');
    text
}

/// EXSMOOTH models, keyword and label. The trend is the index divided by 3 (none, linear,
/// exponential, damped), the season the rest (none, additive, multiplicative).
pub const SMOOTHING_MODELS: [(&str, &str); 12] = [
    ("NN", "Simple: no trend, no season"),
    ("NA", "No trend, additive season"),
    ("NM", "No trend, multiplicative season"),
    ("LN", "Holt: linear trend, no season"),
    ("LA", "Linear trend, additive season"),
    ("LM", "Winters: linear trend, multiplicative season"),
    ("EN", "Exponential trend, no season"),
    ("EA", "Exponential trend, additive season"),
    ("EM", "Exponential trend, multiplicative season"),
    ("DN", "Damped trend, no season"),
    ("DA", "Damped trend, additive season"),
    ("DM", "Damped trend, multiplicative season"),
];

/// The index of EM, whose ALPHA cannot be 0 alone.
const EM: usize = 8;

/// The smoothing parameters: keyword and what each one smooths.
pub const SMOOTHING_PARAMETERS: [(&str, &str); 4] = [
    ("ALPHA", "Level"),
    ("GAMMA", "Trend"),
    ("DELTA", "Season"),
    ("PHI", "Trend damping"),
];

/// One smoothing parameter: a value, or a grid of values to try.
#[derive(Clone, PartialEq, Debug)]
pub struct Parameter {
    pub grid: bool,
    /// 0.1 or empty writes nothing.
    pub value: String,
    /// All three empty: the parameter's own grid.
    pub from: String,
    pub to: String,
    pub step: String,
}

impl Default for Parameter {
    fn default() -> Self {
        Self {
            grid: false,
            value: "0.1".into(),
            from: String::new(),
            to: String::new(),
            step: String::new(),
        }
    }
}

impl Parameter {
    /// `NAME=value` or `NAME=GRID...`; None at the default 0.1. PHI lies strictly between 0
    /// and 1, the others from 0 to 1.
    fn syntax(&self, name: &str) -> Result<Option<String>, String> {
        let open = name == "PHI";
        let inside = |v: f64| {
            if open {
                v > 0.0 && v < 1.0
            } else {
                (0.0..=1.0).contains(&v)
            }
        };
        let bounds = if open {
            "above 0 and below 1"
        } else {
            "from 0 to 1"
        };
        if !self.grid {
            if self.value.trim().is_empty() {
                return Ok(None);
            }
            let v = parse(&self.value, name)?;
            if !inside(v) {
                return Err(format!("{name} must be {bounds}"));
            }
            return Ok((v != 0.1).then(|| format!("{name}={}", number(v))));
        }
        let fields = [&self.from, &self.to, &self.step];
        if fields.iter().all(|f| f.trim().is_empty()) {
            return Ok(Some(format!("{name}=GRID")));
        }
        let what = format!("the {name} grid");
        if fields.iter().any(|f| f.trim().is_empty()) {
            return Err(format!(
                "give the start, end and step of {what}, or none of them"
            ));
        }
        let from = parse(&self.from, &format!("the start of {what}"))?;
        let to = parse(&self.to, &format!("the end of {what}"))?;
        let step = parse(&self.step, &format!("the step of {what}"))?;
        if !inside(from) || !inside(to) {
            return Err(format!("{what} must start and end {bounds}"));
        }
        if to < from {
            return Err(format!("{what} must end at or above its start"));
        }
        if step <= 0.0 || (to > from && step > to - from) {
            return Err(format!(
                "the step of {what} must be above 0 and at most its end less its start"
            ));
        }
        Ok(Some(format!(
            "{name}=GRID({} {} {})",
            number(from),
            number(to),
            number(step)
        )))
    }

    /// The highest value tried; None when it cannot be read (refused by `syntax`).
    fn highest(&self) -> Option<f64> {
        let (text, empty) = match self.grid {
            true => (&self.to, 1.0),
            false => (&self.value, 0.1),
        };
        match text.trim() {
            "" => Some(empty),
            t => t.parse().ok(),
        }
    }
}

/// EXSMOOTH: exponential smoothing of one or more series.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ExpSmoothing {
    pub variables: Vec<String>,
    /// Index into `SMOOTHING_MODELS`.
    pub model: usize,
    /// Seasonal models only; empty: TSET PERIOD or the date variables.
    pub period: String,
    /// The initial seasonal factors, one number per case of the period or the name of a
    /// variable holding them; empty: computed. Seasonal models only.
    pub factors: String,
    /// In the order of `SMOOTHING_PARAMETERS`.
    pub parameters: [Parameter; 4],
    /// INITIAL: the start level and trend given.
    pub initial: bool,
    pub start: String,
    pub trend: String,
    /// APPLY: the previous EXSMOOTH, with what is set here replacing its settings.
    pub apply: bool,
}

impl ExpSmoothing {
    pub fn seasonal(&self) -> bool {
        !self.model.is_multiple_of(3)
    }

    /// Which of ALPHA, GAMMA, DELTA and PHI the model uses: GAMMA with a trend, except a
    /// damped one with a season, DELTA with a season, PHI with a damped trend.
    pub fn uses(&self) -> [bool; 4] {
        let (trend, season) = (self.model / 3, self.model % 3);
        [
            true,
            trend > 0 && !(trend == 3 && season > 0),
            season > 0,
            trend == 3,
        ]
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.is_empty() && !self.apply {
            return Err("add at least one series".into());
        }
        numeric(data, &self.variables)?;
        let mut subs = Vec::new();
        if self.model > 0 {
            subs.push(format!("MODEL={}", SMOOTHING_MODELS[self.model].0));
        }
        if self.seasonal() {
            let period = seasonal_period(data, &self.period)?;
            if let Some(p) = period {
                subs.push(format!("PERIOD={p}"));
            }
            if let Some(f) = self.seasonal_factors(data, period)? {
                subs.push(f);
            }
        }
        for (((name, _), p), used) in SMOOTHING_PARAMETERS
            .iter()
            .zip(&self.parameters)
            .zip(self.uses())
        {
            if used && let Some(s) = p.syntax(name)? {
                subs.push(s);
            }
        }
        if self.model == EM && self.parameters[0].highest() == Some(0.0) {
            return Err("ALPHA must go above 0 in the exponential multiplicative model".into());
        }
        if self.initial {
            let start = parse(&self.start, "the start level")?;
            let trend = parse(&self.trend, "the start trend")?;
            subs.push(format!("INITIAL=({}, {})", number(start), number(trend)));
        }
        if self.apply {
            subs.push("APPLY".into());
        }
        let mut head = "EXSMOOTH".to_string();
        if !self.variables.is_empty() {
            head.push_str(&format!(" VARIABLES={}", self.variables.join(" ")));
        }
        Ok(command(head, subs))
    }

    /// SEASFACT: a list as long as the period (when typed), above 0 for a multiplicative
    /// season, or one numeric variable.
    fn seasonal_factors(
        &self,
        data: &Dataset,
        period: Option<u32>,
    ) -> Result<Option<String>, String> {
        let words: Vec<&str> = self
            .factors
            .split([' ', ','])
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return Ok(None);
        }
        let values: Option<Vec<f64>> = words
            .iter()
            .map(|w| w.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect();
        if let Some(values) = values {
            if let Some(p) = period
                && values.len() != p as usize
            {
                return Err(format!(
                    "give {p} seasonal factors, one for each case of the period"
                ));
            }
            if self.model % 3 == 2 && values.iter().any(|v| *v <= 0.0) {
                return Err("multiplicative seasonal factors must be above 0".into());
            }
            let list: Vec<String> = values.into_iter().map(number).collect();
            return Ok(Some(format!("SEASFACT=({})", list.join(" "))));
        }
        let [name] = words[..] else {
            return Err("the seasonal factors are numbers, or the name of one variable".into());
        };
        if data.variable(name).is_err() {
            return Err(format!("{name} is not a variable"));
        }
        numeric(data, [&name.to_string()])?;
        Ok(Some(format!("SEASFACT={name}")))
    }
}

/// CONSTANT on MODEL; the first writes nothing.
pub const ARIMA_CONSTANT: [(&str, &str); 3] = [
    ("", "As TSET says (at first included)"),
    ("CONSTANT", "Included"),
    ("NOCONSTANT", "Left out"),
];

/// The log transformation on MODEL; the first writes nothing.
pub const ARIMA_LOG: [(&str, &str); 3] = [
    ("NOLOG", "None"),
    ("LG10", "Base-10 log"),
    ("LN", "Natural log"),
];

/// FORECAST; the first writes nothing.
pub const ARIMA_FORECAST: [(&str, &str); 3] = [
    ("EXACT", "Exact, by the Kalman filter"),
    ("CLS", "Conditional least squares"),
    (
        "AUTOINIT",
        "Conditional least squares, first values fit themselves",
    ),
];

/// APPLY and what it takes from the previous ARIMA; the first writes nothing.
pub const ARIMA_APPLY: [(&str, &str); 4] = [
    ("", "No, a new model"),
    ("", "Its model, estimated again"),
    ("INITIAL", "Its estimates as starting values"),
    ("FIT", "Its estimates as they are"),
];

/// The lag lists: keyword and label.
pub const ARIMA_LAGS: [(&str, &str); 4] = [
    ("P", "Autoregressive"),
    ("Q", "Moving average"),
    ("SP", "Seasonal autoregressive"),
    ("SQ", "Seasonal moving average"),
];

/// ARIMA: one dependent series, with regressors.
#[derive(Clone, PartialEq, Debug)]
pub struct ArimaModel {
    pub dependent: String,
    pub regressors: Vec<String>,
    /// p, d and q of MODEL.
    pub orders: [String; 3],
    /// sp, sd and sq of MODEL.
    pub seasonal: [String; 3],
    /// Read only for a seasonal model; empty: TSET PERIOD or the date variables.
    pub period: String,
    /// Index into `ARIMA_CONSTANT`.
    pub constant: usize,
    /// Index into `ARIMA_LOG`.
    pub log: usize,
    /// P, Q, SP and SQ: only these lags, separated by blanks; empty: 1 to the order.
    pub lags: [String; 4],
    pub mxiter: String,
    pub mxlamb: String,
    pub ssqpct: String,
    /// Empty: the TSET setting.
    pub pareps: String,
    /// Empty: the TSET setting.
    pub cinpct: String,
    /// Index into `ARIMA_FORECAST`.
    pub forecast: usize,
    /// Index into `ARIMA_APPLY`.
    pub apply: usize,
}

impl Default for ArimaModel {
    fn default() -> Self {
        let zeros = || ["0".to_string(), "0".to_string(), "0".to_string()];
        Self {
            dependent: String::new(),
            regressors: Vec::new(),
            orders: zeros(),
            seasonal: zeros(),
            period: String::new(),
            constant: 0,
            log: 0,
            lags: Default::default(),
            mxiter: "10".into(),
            mxlamb: "1000000000".into(),
            ssqpct: "0.001".into(),
            pareps: String::new(),
            cinpct: String::new(),
            forecast: 0,
            apply: 0,
        }
    }
}

impl ArimaModel {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.dependent.is_empty() {
            if self.apply == 0 {
                return Err("set the dependent series".into());
            }
            if !self.regressors.is_empty() {
                return Err("set the dependent series to go with the regressors".into());
            }
        }
        numeric(
            data,
            std::iter::once(&self.dependent).chain(&self.regressors),
        )?;
        if self.regressors.contains(&self.dependent) {
            return Err(format!(
                "{} is both the dependent series and a regressor",
                self.dependent
            ));
        }
        let orders = |texts: &[String; 3], names: [&str; 3]| -> Result<[u32; 3], String> {
            let mut out = [0; 3];
            for ((slot, text), what) in out.iter_mut().zip(texts).zip(names) {
                *slot = whole(text, 0, what)?.unwrap_or(0);
                if *slot > 1000 {
                    return Err(format!("{what} must be at most 1000"));
                }
            }
            Ok(out)
        };
        let o = orders(
            &self.orders,
            [
                "the autoregressive order",
                "the degree of differencing",
                "the moving average order",
            ],
        )?;
        let s = orders(
            &self.seasonal,
            [
                "the seasonal autoregressive order",
                "the degree of seasonal differencing",
                "the seasonal moving average order",
            ],
        )?;
        let mut lags = Vec::new();
        for ((keyword, _), text) in ARIMA_LAGS.iter().zip(&self.lags) {
            let mut list = Vec::new();
            for word in text.split([' ', ',']).filter(|w| !w.is_empty()) {
                match word.parse::<u32>() {
                    Ok(n) if (1..=1000).contains(&n) => list.push(n.to_string()),
                    _ => {
                        return Err(format!(
                            "the lags of {keyword} must be whole numbers from 1 to 1000"
                        ));
                    }
                }
            }
            lags.push((*keyword, list));
        }
        let seasonal = s.iter().any(|n| *n > 0) || lags[2..].iter().any(|(_, l)| !l.is_empty());
        let mut model = Vec::new();
        if seasonal || o.iter().any(|n| *n > 0) {
            model.push(format!("({},{},{})", o[0], o[1], o[2]));
        }
        if seasonal {
            model.push(format!("({},{},{})", s[0], s[1], s[2]));
            if let Some(p) = seasonal_period(data, &self.period)? {
                model.push(p.to_string());
            }
        }
        if self.constant > 0 {
            model.push(ARIMA_CONSTANT[self.constant].0.into());
        }
        if self.log > 0 {
            model.push(ARIMA_LOG[self.log].0.into());
        }
        let mut subs = Vec::new();
        if !model.is_empty() {
            subs.push(format!("MODEL={}", model.join(" ")));
        }
        for (keyword, list) in lags {
            if !list.is_empty() {
                subs.push(format!("{keyword}=({})", list.join(", ")));
            }
        }
        if let Some(n) = whole(&self.mxiter, 0, "the most iterations")?
            && n != 10
        {
            subs.push(format!("MXITER={n}"));
        }
        // the limits: above 0 and at most the bound, and the value that writes nothing
        let limits = [
            (
                "MXLAMB",
                &self.mxlamb,
                "the largest Marquardt constant",
                None,
                Some(1e9),
            ),
            (
                "SSQPCT",
                &self.ssqpct,
                "the change in the sum of squares (%)",
                Some(100.0),
                Some(0.001),
            ),
            (
                "PAREPS",
                &self.pareps,
                "the change in the estimates",
                None,
                None,
            ),
        ];
        for (keyword, text, what, most, default) in limits {
            if text.trim().is_empty() {
                continue;
            }
            let v = parse(text, what)?;
            match most {
                Some(m) if v <= 0.0 || v > m => {
                    return Err(format!("{what} must be above 0 and at most {m}"));
                }
                None if v <= 0.0 => return Err(format!("{what} must be above 0")),
                _ => {}
            }
            if Some(v) != default {
                subs.push(format!("{keyword}={}", number(v)));
            }
        }
        if !self.cinpct.trim().is_empty() {
            subs.push(format!("CINPCT={}", number(percent(&self.cinpct)?)));
        }
        if self.forecast > 0 {
            subs.push(format!("FORECAST={}", ARIMA_FORECAST[self.forecast].0));
        }
        match self.apply {
            0 => {}
            1 => subs.push("APPLY".into()),
            n => subs.push(format!("APPLY {}", ARIMA_APPLY[n].0)),
        }
        let mut head = "ARIMA".to_string();
        if !self.dependent.is_empty() {
            head.push_str(&format!(" VARIABLES={}", self.dependent));
            if !self.regressors.is_empty() {
                head.push_str(&format!(" WITH {}", self.regressors.join(" ")));
            }
        }
        Ok(command(head, subs))
    }
}

/// USE (the historical period) or, with `predict`, PREDICT (the forecast period): from a
/// start through an end, both case numbers or both dates; an empty end is open.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TimeRange {
    pub predict: bool,
    pub by_date: bool,
    pub start: String,
    pub end: String,
    /// The values of the date variables of the start and the end, highest first, as
    /// `date_units` lists them; empty: not given.
    pub start_date: [String; 4],
    pub end_date: [String; 4],
    /// USE /PERMANENT: delete the cases outside the range.
    pub permanent: bool,
}

impl TimeRange {
    pub fn new(predict: bool) -> Self {
        Self {
            predict,
            ..Default::default()
        }
    }

    /// A start or end as DATE keywords and values, and the values for comparing.
    fn date(
        units: &[DateUnit],
        texts: &[String; 4],
        what: &str,
    ) -> Result<Option<(String, Vec<u32>)>, String> {
        let mut words = Vec::new();
        let mut values = Vec::new();
        for (unit, text) in units.iter().zip(texts) {
            let label = unit.name().to_lowercase();
            if let Some(v) = whole(text, 0, &format!("the {label} of the {what}"))? {
                words.push(format!("{} {v}", unit.name()));
                values.push(v);
            }
        }
        if words.is_empty() {
            return Ok(None);
        }
        // lower parts may be left out, the highest not
        if texts[0].trim().is_empty() {
            return Err(format!(
                "the {what} needs the {}",
                units[0].name().to_lowercase()
            ));
        }
        Ok(Some((words.join(" "), values)))
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let (start, end) = if self.by_date {
            let units = date_units(data);
            if units.is_empty() {
                return Err("the data has no date variables: define dates first".into());
            }
            let start = Self::date(&units, &self.start_date, "start")?;
            let end = Self::date(&units, &self.end_date, "end")?;
            // dates naming the same parts compare part by part, the highest first
            let same_parts = |a: &[String; 4], b: &[String; 4]| {
                a.iter()
                    .zip(b)
                    .all(|(x, y)| x.trim().is_empty() == y.trim().is_empty())
            };
            if let (Some((_, a)), Some((_, b))) = (&start, &end)
                && same_parts(&self.start_date, &self.end_date)
                && b < a
            {
                return Err("the end comes before the start".into());
            }
            (start.map(|s| s.0), end.map(|e| e.0))
        } else {
            let start = whole(&self.start, 1, "the first case")?;
            let end = whole(&self.end, 1, "the last case")?;
            if let (Some(a), Some(b)) = (start, end)
                && b < a
            {
                return Err("the last case comes before the first".into());
            }
            let cases = data.case_count();
            if !self.predict
                && let Some(a) = start
                && a as usize > cases
            {
                return Err(format!("the first case must be at most {cases}"));
            }
            (start.map(|n| n.to_string()), end.map(|n| n.to_string()))
        };
        if self.predict {
            return Ok(match (start, end) {
                (Some(s), Some(e)) => format!("PREDICT {s} THRU {e}."),
                (Some(s), None) => format!("PREDICT {s}."),
                (None, Some(e)) => format!("PREDICT THRU {e}."),
                (None, None) => "PREDICT THRU.".into(),
            });
        }
        if start.is_none() && end.is_none() {
            return Ok("USE ALL.".into());
        }
        let mut text = format!(
            "USE {} THRU {}",
            start.as_deref().unwrap_or("FIRST"),
            end.as_deref().unwrap_or("LAST")
        );
        if self.permanent {
            text.push_str("\n  /PERMANENT");
        }
        text.push('.');
        Ok(text)
    }
}

/// How several series are drawn: FORMAT keyword and label; the first writes nothing.
pub const CHART_JOIN: [(&str, &str); 3] = [
    ("NOJOIN", "Each series on its own"),
    ("JOIN", "Join the values of each case"),
    ("HILO", "Only the highest and lowest of each case"),
];

/// TSPLOT or, `casewise`, CASEPLOT: series over the cases, which may be logged and
/// differenced first.
#[derive(Clone, PartialEq, Debug)]
pub struct SeriesChart {
    pub casewise: bool,
    pub variables: Vec<String>,
    pub ln: bool,
    pub diff: bool,
    pub diff_order: String,
    pub sdiff: bool,
    pub sdiff_order: String,
    /// Empty: the TSET or DATE period. Read only with `sdiff`.
    pub period: String,
    /// Labels the cases; empty: TSET ID, the date or the case number.
    pub id: String,
    /// A line where this variable changes value.
    pub mark: String,
    /// One series: fill the area under (TSPLOT) or beside (CASEPLOT) the line.
    pub fill: bool,
    /// One series: 0 no reference line, 1 at the mean, 2 at `reference_value`.
    pub reference: usize,
    pub reference_value: String,
    /// Several series: index into `CHART_JOIN`.
    pub join: usize,
    /// APPLY: the previous chart of this kind, with what is set here replacing its settings.
    pub apply: bool,
}

impl SeriesChart {
    pub fn new(casewise: bool) -> Self {
        Self {
            casewise,
            variables: Vec::new(),
            ln: false,
            diff: false,
            diff_order: "1".into(),
            sdiff: false,
            sdiff_order: "1".into(),
            period: String::new(),
            id: String::new(),
            mark: String::new(),
            fill: false,
            reference: 0,
            reference_value: String::new(),
            join: 0,
            apply: false,
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.is_empty() && !self.apply {
            return Err("add at least one series".into());
        }
        numeric(
            data,
            self.variables
                .iter()
                .chain(Some(&self.mark).filter(|m| !m.is_empty())),
        )?;
        let name = if self.casewise { "CASEPLOT" } else { "TSPLOT" };
        let mut text = name.to_string();
        if !self.variables.is_empty() {
            text.push_str(&format!(" VARIABLES={}", self.variables.join(" ")));
        }
        transforms(
            &mut text,
            self.ln,
            self.diff.then_some(&self.diff_order),
            self.sdiff.then_some(&self.sdiff_order),
            &self.period,
        )?;
        let mut subs = Vec::new();
        if !self.id.is_empty() {
            subs.push(format!("ID={}", self.id));
        }
        if !self.mark.is_empty() {
            subs.push(format!("MARK={}", self.mark));
        }
        let mut format = Vec::new();
        match self.variables.len() {
            1 => {
                if self.fill {
                    format.push(if self.casewise { "LEFT" } else { "BOTTOM" }.to_string());
                }
                match self.reference {
                    1 => format.push("REFERENCE".into()),
                    2 => {
                        let v = parse(&self.reference_value, "the reference value")?;
                        format.push(format!("REFERENCE({})", number(v)));
                    }
                    _ => {}
                }
            }
            n if n > 1 && self.join > 0 => {
                if self.join == 2 && n > 3 {
                    return Err("the highest and lowest are shown for two or three series".into());
                }
                format.push(CHART_JOIN[self.join].0.into());
            }
            _ => {}
        }
        if !format.is_empty() {
            subs.push(format!("FORMAT={}", format.join(" ")));
        }
        if self.apply {
            subs.push("APPLY".into());
        }
        Ok(command(text, subs))
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
    fn smoothing_checks_its_parameters_and_season() {
        let data = data();
        let mut d = ExpSmoothing {
            variables: v(&["age"]),
            model: 4,
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "a seasonal model needs a period: type it, or define dates first"
        );
        d.period = "4".into();
        d.factors = "1 2 3".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give 4 seasonal factors, one for each case of the period"
        );
        d.factors = "age".into();
        d.parameters[2] = Parameter {
            grid: true,
            from: "0.5".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "give the start, end and step of the DELTA grid, or none of them"
        );
        d.parameters[2].to = "0.2".into();
        d.parameters[2].step = "0.1".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the DELTA grid must end at or above its start"
        );
        d.parameters[2].to = "0.6".into();
        d.parameters[2].step = "0.2".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the step of the DELTA grid must be above 0 and at most its end less its start"
        );
        // a parameter the model does not use is neither checked nor written
        d.parameters[3].value = "1".into();
        d.parameters[2] = Parameter::default();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "EXSMOOTH VARIABLES=age\n  /MODEL=LA\n  /PERIOD=4\n  /SEASFACT=age."
        );
        d.model = 9;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "PHI must be above 0 and below 1"
        );
        d.model = EM;
        d.parameters[0].value = "0".into();
        d.factors = "1 1 -1 1".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "multiplicative seasonal factors must be above 0"
        );
        d.factors.clear();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "ALPHA must go above 0 in the exponential multiplicative model"
        );
    }

    #[test]
    fn arima_checks_orders_lags_and_roles() {
        let data = data();
        let mut d = ArimaModel::default();
        assert_eq!(d.syntax(&data).unwrap_err(), "set the dependent series");
        d.apply = 3;
        assert_eq!(d.syntax(&data).unwrap(), "ARIMA\n  /APPLY FIT.");
        d.dependent = "age".into();
        d.regressors = v(&["age"]);
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "age is both the dependent series and a regressor"
        );
        d.regressors.clear();
        d.orders[0] = "1001".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the autoregressive order must be at most 1000"
        );
        d.orders[0] = "1".into();
        d.lags[3] = "0".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the lags of SQ must be whole numbers from 1 to 1000"
        );
        // seasonal lags alone make the model seasonal, which needs a period here
        d.lags[3] = "4".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "a seasonal model needs a period: type it, or define dates first"
        );
        d.period = "4".into();
        d.ssqpct = "0".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the change in the sum of squares (%) must be above 0 and at most 100"
        );
        d.ssqpct = "".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ARIMA VARIABLES=age\n  /MODEL=(1,0,0) (0,0,0) 4\n  /SQ=(4)\n  /APPLY FIT."
        );
    }

    #[test]
    fn ranges_check_their_ends() {
        let data = data();
        let mut d = TimeRange::new(false);
        assert_eq!(d.syntax(&data).unwrap(), "USE ALL.");
        d.start = "5".into();
        d.end = "3".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the last case comes before the first"
        );
        d.start = "100000".into();
        d.end.clear();
        assert!(
            d.syntax(&data)
                .unwrap_err()
                .starts_with("the first case must be at most")
        );
        // a forecast may start past the end of the file
        d.predict = true;
        assert_eq!(d.syntax(&data).unwrap(), "PREDICT 100000.");
        d.by_date = true;
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the data has no date variables: define dates first"
        );
        let units = [DateUnit::Year, DateUnit::Month];
        let date = |texts: [&str; 4]| TimeRange::date(&units, &texts.map(String::from), "start");
        assert_eq!(
            date(["", "3", "", ""]).unwrap_err(),
            "the start needs the year"
        );
        assert_eq!(
            date(["2020", "", "", ""]).unwrap(),
            Some(("YEAR 2020".to_string(), vec![2020]))
        );
    }

    #[test]
    fn charts_write_formats_that_fit_the_series() {
        let data = data();
        let mut d = SeriesChart {
            variables: v(&["age"]),
            fill: true,
            reference: 2,
            join: 2,
            ..SeriesChart::new(true)
        };
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the reference value must be a number"
        );
        d.reference = 1;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CASEPLOT VARIABLES=age\n  /FORMAT=LEFT REFERENCE."
        );
        // with several series the fill and reference line do not apply
        d.variables = v(&["age", "monthly_spend"]);
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CASEPLOT VARIABLES=age monthly_spend\n  /FORMAT=HILO."
        );
        d.mark = "plan".into();
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "plan is a string; the command needs numbers"
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
