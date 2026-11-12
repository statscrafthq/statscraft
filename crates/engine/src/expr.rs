//! Expression evaluation, one column at a time.
//!
//! Every node of an expression yields a whole column, so the types of an expression are checked
//! even when the dataset has no cases, and before a command changes anything.
//!
//! Missing values: a user-missing value of a numeric variable reads as missing. A missing
//! operand makes the result missing, with the exceptions the language defines:
//! `0 * missing`, `0 / missing` and `MOD(0, missing)` are 0, and AND / OR use three-valued
//! logic. Dividing by 0 is missing, also 0 / 0. A string is never missing in an expression,
//! except to MISSING.
//!
//! Strings: a string variable reads with its padding spaces, as wide as it is declared. LENGTH
//! counts bytes (as widths do); SUBSTR, INDEX and the other positions count characters, and
//! CHAR.LENGTH leaves out the trailing blanks. `expr_functions` holds the date, time and string
//! functions of one value with their rules.
//!
//! Functions with a parameter out of range give system-missing, also the distribution functions
//! with an x the distribution does not take. Scoring (APPLYMODEL) is not supported.

use crate::expr_functions as f;
use crate::format::format_value;
use crate::{Error, dist, distributions};
use statscraft_data::{Dataset, Value, Values};
use statscraft_syntax::{BinaryOp, Expr, FormatType, UnaryOp};
use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;

pub(crate) type Numbers = Vec<Option<f64>>;

pub(crate) enum Column {
    /// None is system-missing.
    Number(Numbers),
    String(Vec<String>),
}

/// Functions that are evaluated, each with the call forms it accepts.
const FUNCTIONS: &[(&str, &str)] = &[
    ("ABS", "ABS(number)"),
    ("RND", "RND(number [, multiple [, fuzzbits]])"),
    ("TRUNC", "TRUNC(number [, multiple [, fuzzbits]])"),
    ("MOD", "MOD(number, divisor)"),
    ("SQRT", "SQRT(number)"),
    ("EXP", "EXP(number)"),
    ("LN", "LN(number)"),
    ("LG10", "LG10(number)"),
    ("LNGAMMA", "LNGAMMA(number)"),
    ("SIN", "SIN(radians)"),
    ("COS", "COS(radians)"),
    ("TAN", "TAN(radians)"),
    ("ARSIN", "ARSIN(number)"),
    ("ARCOS", "ARCOS(number)"),
    ("ARTAN", "ARTAN(number)"),
    ("SUM", "SUM(number, ...)"),
    ("MEAN", "MEAN(number, ...)"),
    ("MEDIAN", "MEDIAN(number, ...)"),
    ("MIN", "MIN(value, ...)"),
    ("MAX", "MAX(value, ...)"),
    ("SD", "SD(number, number, ...)"),
    ("VARIANCE", "VARIANCE(number, number, ...)"),
    ("CFVAR", "CFVAR(number, number, ...)"),
    ("NVALID", "NVALID(number, ...)"),
    ("NMISS", "NMISS(number, ...)"),
    ("MISSING", "MISSING(variable)"),
    ("SYSMIS", "SYSMIS(number)"),
    ("VALUE", "VALUE(variable)"),
    ("LAG", "LAG(variable [, n])"),
    ("VALUELABEL", "VALUELABEL(variable)"),
    ("ANY", "ANY(value, candidate, ...)"),
    ("RANGE", "RANGE(value, low, high [, low, high ...])"),
    ("CONCAT", "CONCAT(string, ...)"),
    ("SUBSTR", "SUBSTR(string, position [, length])"),
    ("CHAR.SUBSTR", "CHAR.SUBSTR(string, position [, length])"),
    ("LENGTH", "LENGTH(string)"),
    ("CHAR.LENGTH", "CHAR.LENGTH(string)"),
    ("UPCASE", "UPCASE(string)"),
    ("LOWER", "LOWER(string)"),
    ("LTRIM", "LTRIM(string [, character])"),
    ("RTRIM", "RTRIM(string [, character])"),
    ("NTRIM", "NTRIM(variable)"),
    ("INDEX", "INDEX(string, text to find [, piece length])"),
    (
        "CHAR.INDEX",
        "CHAR.INDEX(string, text to find [, piece length])",
    ),
    ("RINDEX", "RINDEX(string, text to find [, piece length])"),
    (
        "CHAR.RINDEX",
        "CHAR.RINDEX(string, text to find [, piece length])",
    ),
    ("LPAD", "LPAD(string, length [, padding])"),
    ("CHAR.LPAD", "CHAR.LPAD(string, length [, padding])"),
    ("RPAD", "RPAD(string, length [, padding])"),
    ("CHAR.RPAD", "CHAR.RPAD(string, length [, padding])"),
    ("REPLACE", "REPLACE(string, old, new [, count])"),
    ("STRUNC", "STRUNC(string, bytes)"),
    ("CHAR.MBLEN", "CHAR.MBLEN(string, character position)"),
    ("MBLEN.BYTE", "MBLEN.BYTE(string, byte position)"),
    ("NORMALIZE", "NORMALIZE(string)"),
    ("NUMBER", "NUMBER(string, format)"),
    ("STRING", "STRING(number, format)"),
    ("DATE.DMY", "DATE.DMY(day, month, year)"),
    ("DATE.MDY", "DATE.MDY(month, day, year)"),
    ("DATE.MOYR", "DATE.MOYR(month, year)"),
    ("DATE.QYR", "DATE.QYR(quarter, year)"),
    ("DATE.WKYR", "DATE.WKYR(week, year)"),
    ("DATE.YRDAY", "DATE.YRDAY(year, day of year)"),
    ("YRMODA", "YRMODA(year, month, day)"),
    ("TIME.DAYS", "TIME.DAYS(days)"),
    ("TIME.HMS", "TIME.HMS(hours [, minutes [, seconds]])"),
    ("CTIME.DAYS", "CTIME.DAYS(time)"),
    ("CTIME.HOURS", "CTIME.HOURS(time)"),
    ("CTIME.MINUTES", "CTIME.MINUTES(time)"),
    ("CTIME.SECONDS", "CTIME.SECONDS(time)"),
    ("XDATE.DATE", "XDATE.DATE(date)"),
    ("XDATE.HOUR", "XDATE.HOUR(time)"),
    ("XDATE.JDAY", "XDATE.JDAY(date)"),
    ("XDATE.MDAY", "XDATE.MDAY(date)"),
    ("XDATE.MINUTE", "XDATE.MINUTE(time)"),
    ("XDATE.MONTH", "XDATE.MONTH(date)"),
    ("XDATE.QUARTER", "XDATE.QUARTER(date)"),
    ("XDATE.SECOND", "XDATE.SECOND(time)"),
    ("XDATE.TDAY", "XDATE.TDAY(time)"),
    ("XDATE.TIME", "XDATE.TIME(time)"),
    ("XDATE.WEEK", "XDATE.WEEK(date)"),
    ("XDATE.WKDAY", "XDATE.WKDAY(date)"),
    ("XDATE.YEAR", "XDATE.YEAR(date)"),
    ("DATEDIFF", "DATEDIFF(date2, date1, 'unit')"),
    (
        "DATESUM",
        "DATESUM(date, value, 'unit' [, 'rollover' | 'closest'])",
    ),
    ("CDFNORM", "CDFNORM(z)"),
    ("PROBIT", "PROBIT(p)"),
    ("CDF.BERNOULLI", "CDF.BERNOULLI(x, prob)"),
    ("CDF.BETA", "CDF.BETA(x, shape1, shape2)"),
    ("CDF.BINOM", "CDF.BINOM(x, n, prob)"),
    ("CDF.BVNOR", "CDF.BVNOR(x1, x2, corr)"),
    ("CDF.CAUCHY", "CDF.CAUCHY(x, location, scale)"),
    ("CDF.CHISQ", "CDF.CHISQ(x, df)"),
    ("CDF.EXP", "CDF.EXP(x, rate)"),
    ("CDF.F", "CDF.F(x, df1, df2)"),
    ("CDF.GAMMA", "CDF.GAMMA(x, shape, rate)"),
    ("CDF.GEOM", "CDF.GEOM(x, prob)"),
    ("CDF.HALFNRM", "CDF.HALFNRM(x, location, scale)"),
    ("CDF.HYPER", "CDF.HYPER(x, total, sample, hits)"),
    ("CDF.IGAUSS", "CDF.IGAUSS(x, mean, shape)"),
    ("CDF.LAPLACE", "CDF.LAPLACE(x, mean, scale)"),
    ("CDF.LOGISTIC", "CDF.LOGISTIC(x, mean, scale)"),
    ("CDF.LNORMAL", "CDF.LNORMAL(x, a, b)"),
    ("CDF.NEGBIN", "CDF.NEGBIN(x, threshold, prob)"),
    ("CDF.NORMAL", "CDF.NORMAL(x, mean, sd)"),
    ("CDF.PARETO", "CDF.PARETO(x, threshold, shape)"),
    ("CDF.POISSON", "CDF.POISSON(x, mean)"),
    ("CDF.SMOD", "CDF.SMOD(x, comparisons, df)"),
    ("CDF.SRANGE", "CDF.SRANGE(x, means, df)"),
    ("CDF.T", "CDF.T(x, df)"),
    ("CDF.UNIFORM", "CDF.UNIFORM(x, min, max)"),
    ("CDF.WEIBULL", "CDF.WEIBULL(x, scale, shape)"),
    ("IDF.BETA", "IDF.BETA(p, shape1, shape2)"),
    ("IDF.CAUCHY", "IDF.CAUCHY(p, location, scale)"),
    ("IDF.CHISQ", "IDF.CHISQ(p, df)"),
    ("IDF.EXP", "IDF.EXP(p, rate)"),
    ("IDF.F", "IDF.F(p, df1, df2)"),
    ("IDF.GAMMA", "IDF.GAMMA(p, shape, rate)"),
    ("IDF.HALFNRM", "IDF.HALFNRM(p, location, scale)"),
    ("IDF.IGAUSS", "IDF.IGAUSS(p, mean, shape)"),
    ("IDF.LAPLACE", "IDF.LAPLACE(p, mean, scale)"),
    ("IDF.LOGISTIC", "IDF.LOGISTIC(p, mean, scale)"),
    ("IDF.LNORMAL", "IDF.LNORMAL(p, a, b)"),
    ("IDF.NORMAL", "IDF.NORMAL(p, mean, sd)"),
    ("IDF.PARETO", "IDF.PARETO(p, threshold, shape)"),
    ("IDF.SMOD", "IDF.SMOD(p, comparisons, df)"),
    ("IDF.SRANGE", "IDF.SRANGE(p, means, df)"),
    ("IDF.T", "IDF.T(p, df)"),
    ("IDF.UNIFORM", "IDF.UNIFORM(p, min, max)"),
    ("IDF.WEIBULL", "IDF.WEIBULL(p, scale, shape)"),
    ("PDF.BERNOULLI", "PDF.BERNOULLI(x, prob)"),
    ("PDF.BETA", "PDF.BETA(x, shape1, shape2)"),
    ("PDF.BINOM", "PDF.BINOM(x, n, prob)"),
    ("PDF.BVNOR", "PDF.BVNOR(x1, x2, corr)"),
    ("PDF.CAUCHY", "PDF.CAUCHY(x, location, scale)"),
    ("PDF.CHISQ", "PDF.CHISQ(x, df)"),
    ("PDF.EXP", "PDF.EXP(x, rate)"),
    ("PDF.F", "PDF.F(x, df1, df2)"),
    ("PDF.GAMMA", "PDF.GAMMA(x, shape, rate)"),
    ("PDF.GEOM", "PDF.GEOM(x, prob)"),
    ("PDF.HALFNRM", "PDF.HALFNRM(x, location, scale)"),
    ("PDF.HYPER", "PDF.HYPER(x, total, sample, hits)"),
    ("PDF.IGAUSS", "PDF.IGAUSS(x, mean, shape)"),
    ("PDF.LAPLACE", "PDF.LAPLACE(x, mean, scale)"),
    ("PDF.LOGISTIC", "PDF.LOGISTIC(x, mean, scale)"),
    ("PDF.LNORMAL", "PDF.LNORMAL(x, a, b)"),
    ("PDF.NEGBIN", "PDF.NEGBIN(x, threshold, prob)"),
    ("PDF.NORMAL", "PDF.NORMAL(x, mean, sd)"),
    ("PDF.PARETO", "PDF.PARETO(x, threshold, shape)"),
    ("PDF.POISSON", "PDF.POISSON(x, mean)"),
    ("PDF.T", "PDF.T(x, df)"),
    ("PDF.UNIFORM", "PDF.UNIFORM(x, min, max)"),
    ("PDF.WEIBULL", "PDF.WEIBULL(x, scale, shape)"),
    ("NCDF.BETA", "NCDF.BETA(x, shape1, shape2, lambda)"),
    ("NCDF.CHISQ", "NCDF.CHISQ(x, df, lambda)"),
    ("NCDF.F", "NCDF.F(x, df1, df2, lambda)"),
    ("NCDF.T", "NCDF.T(x, df, lambda)"),
    ("NPDF.BETA", "NPDF.BETA(x, shape1, shape2, lambda)"),
    ("NPDF.CHISQ", "NPDF.CHISQ(x, df, lambda)"),
    ("NPDF.F", "NPDF.F(x, df1, df2, lambda)"),
    ("NPDF.T", "NPDF.T(x, df, lambda)"),
    ("SIG.CHISQ", "SIG.CHISQ(x, df)"),
    ("SIG.F", "SIG.F(x, df1, df2)"),
    ("UNIFORM", "UNIFORM(max)"),
    ("NORMAL", "NORMAL(sd)"),
    ("RV.BERNOULLI", "RV.BERNOULLI(prob)"),
    ("RV.BETA", "RV.BETA(shape1, shape2)"),
    ("RV.BINOM", "RV.BINOM(n, prob)"),
    ("RV.CAUCHY", "RV.CAUCHY(location, scale)"),
    ("RV.CHISQ", "RV.CHISQ(df)"),
    ("RV.EXP", "RV.EXP(rate)"),
    ("RV.F", "RV.F(df1, df2)"),
    ("RV.GAMMA", "RV.GAMMA(shape, rate)"),
    ("RV.GEOM", "RV.GEOM(prob)"),
    ("RV.HALFNRM", "RV.HALFNRM(mean, sd)"),
    ("RV.HYPER", "RV.HYPER(total, sample, hits)"),
    ("RV.IGAUSS", "RV.IGAUSS(location, scale)"),
    ("RV.LAPLACE", "RV.LAPLACE(mean, scale)"),
    ("RV.LOGISTIC", "RV.LOGISTIC(mean, scale)"),
    ("RV.LNORMAL", "RV.LNORMAL(a, b)"),
    ("RV.NEGBIN", "RV.NEGBIN(threshold, prob)"),
    ("RV.NORMAL", "RV.NORMAL(mean, sd)"),
    ("RV.PARETO", "RV.PARETO(threshold, shape)"),
    ("RV.POISSON", "RV.POISSON(mean)"),
    ("RV.T", "RV.T(df)"),
    ("RV.UNIFORM", "RV.UNIFORM(min, max)"),
    ("RV.WEIBULL", "RV.WEIBULL(a, b)"),
];

/// The functions expressions evaluate, each with its call form, for completion and help.
pub fn functions() -> &'static [(&'static str, &'static str)] {
    FUNCTIONS
}

/// Functions that take a `.n` suffix, with the number of valid arguments they need without one.
const STATISTICS: &[(&str, usize)] = &[
    ("SUM", 1),
    ("MEAN", 1),
    ("MEDIAN", 1),
    ("MIN", 1),
    ("MAX", 1),
    ("SD", 2),
    ("VARIANCE", 2),
    ("CFVAR", 2),
];

/// Functions of the language that are not evaluated: scoring needs MODEL HANDLE.
const UNSUPPORTED: &[&str] = &["APPLYMODEL", "STRAPPLYMODEL"];

/// The distribution families whose functions `distributions` evaluates.
const DISTRIBUTION_FAMILIES: &[&str] = &["CDF", "IDF", "PDF", "NCDF", "NPDF", "SIG"];

/// The number of arguments of a distribution function such as CDF.NORMAL, None for any other
/// name. The matrix language has the same functions.
fn distribution_arity(function: &str) -> Option<usize> {
    let (family, _) = function.split_once('.')?;
    DISTRIBUTION_FAMILIES.contains(&family).then_some(())?;
    statscraft_syntax::matrix_function(function).map(|(_, count, _)| count)
}

/// A function name shortened to three or more letters that start only one function, such as
/// SYS for SYSMIS, as the full name; any other name as it is.
fn full_name(name: String) -> String {
    let names = FUNCTIONS.iter().map(|(f, _)| *f);
    let all = names.clone().chain(UNSUPPORTED.iter().copied());
    if name.len() < 3 || name.contains('.') || names.clone().any(|f| f == name) {
        return name;
    }
    let mut starting = all.filter(|f| f.starts_with(&name));
    match (starting.next(), starting.next()) {
        (Some(only), None) if names.clone().any(|f| f == only) => only.to_string(),
        _ => name,
    }
}

/// Days from 1582-10-14, where date values start, to 1970-01-01.
pub(crate) const DAYS_TO_1970: i64 = 141_428;

pub(crate) struct Evaluator<'a> {
    pub data: &'a Dataset,
    /// The cases the result is used for; None means all. A value that is an error (an operand
    /// of AND that is not 0 or 1) is reported only for these cases.
    pub active: Option<&'a [bool]>,
    /// The number of cases before the first one here, for $CASENUM.
    pub first_case: usize,
    /// The cases before the first one here as they ended the pass, the latest last, for LAG.
    pub previous: &'a [Dataset],
    /// The vectors by lower-case name, with their variables.
    pub vectors: &'a std::collections::HashMap<String, Vec<String>>,
    /// The generator that UNIFORM, NORMAL and the RV functions draw from.
    pub random: &'a std::cell::RefCell<crate::random::Random>,
    /// SET FUZZBITS: the third argument of RND and TRUNC when it is left out.
    pub fuzzbits: f64,
    /// SET BLANKS and EPOCH: NUMBER reads a blank string as BLANKS, and NUMBER and the DATE
    /// functions put a two-digit year in the EPOCH window.
    pub reading: crate::data_list::Reading,
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn boolean(value: bool) -> Option<f64> {
    Some(f64::from(u8::from(value)))
}

fn map(values: Numbers, f: impl Fn(f64) -> f64) -> Column {
    Column::Number(
        values
            .into_iter()
            .map(|v| v.and_then(|v| finite(f(v))))
            .collect(),
    )
}

fn zip(a: Numbers, b: Numbers, f: impl Fn(Option<f64>, Option<f64>) -> Option<f64>) -> Column {
    let results = a.into_iter().zip(b).map(|(a, b)| f(a, b).and_then(finite));
    Column::Number(results.collect())
}

fn map_text(values: Vec<String>, f: impl Fn(&str) -> String) -> Column {
    Column::String(values.iter().map(|v| f(v)).collect())
}

/// How an operand is named in a type error.
fn describe(expr: &Expr) -> String {
    match expr {
        Expr::Variable(name) => format!("variable {name}"),
        Expr::String(text) => format!("'{text}'"),
        Expr::Number(n) => n.to_string(),
        Expr::Call { name, .. } => format!("the result of {}", name.to_uppercase()),
        _ => "the expression".to_string(),
    }
}

/// Compares as the language does: the shorter string counts as padded with spaces.
fn compare_text(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let at = |s: &[u8], i: usize| s.get(i).copied().unwrap_or(b' ');
    (0..a.len().max(b.len()))
        .map(|i| at(a, i).cmp(&at(b, i)))
        .find(|order| order.is_ne())
        .unwrap_or(Ordering::Equal)
}

fn holds(op: BinaryOp, order: Ordering) -> bool {
    match op {
        BinaryOp::Eq => order.is_eq(),
        BinaryOp::Ne => order.is_ne(),
        BinaryOp::Lt => order.is_lt(),
        BinaryOp::Le => order.is_le(),
        BinaryOp::Gt => order.is_gt(),
        _ => order.is_ge(),
    }
}

fn symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Pow => "**",
        BinaryOp::Eq => "=",
        BinaryOp::Ne => "<>",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "AND",
        BinaryOp::Or => "OR",
    }
}

fn arithmetic(op: BinaryOp, a: Option<f64>, b: Option<f64>) -> Option<f64> {
    let zero = Some(0.0);
    match op {
        BinaryOp::Add => Some(a? + b?),
        BinaryOp::Sub => Some(a? - b?),
        BinaryOp::Mul if a == zero || b == zero => zero,
        BinaryOp::Mul => Some(a? * b?),
        // dividing by 0 is a domain error, even 0 / 0
        BinaryOp::Div if b == zero => None,
        BinaryOp::Div if a == zero => zero,
        BinaryOp::Div => Some(a? / b?),
        // the manual's list of operations that survive a missing operand has no power
        _ if a == zero => (b? > 0.0).then_some(0.0),
        // a negative base with a fractional exponent gives NaN, which `finite` turns to missing
        _ => Some(a?.powf(b?)),
    }
}

/// Rounds `x / multiple` half away from zero (`half` 0.5) or towards zero (`half` 0.0). A value
/// that falls short of the next step by less than its last `fuzzbits` bits (the step times
/// 2^(fuzzbits - 53)) still reaches it, so that the error of earlier arithmetic does not turn
/// 2.5 into 2. The slack grows with the size of the value; fuzzbits 0 rounds exactly.
fn round(x: Option<f64>, multiple: Option<f64>, fuzzbits: Option<f64>, half: f64) -> Option<f64> {
    let (x, multiple) = (x?, multiple.filter(|m| *m != 0.0)?);
    let steps = x / multiple;
    let mut whole = (steps.abs() + half).floor();
    let next = whole + 1.0;
    if next - (steps.abs() + half) < next * (fuzzbits? - 53.0).exp2() {
        whole = next;
    }
    Some(steps.signum() * whole * multiple)
}

fn modulo(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (_, Some(0.0)) => None,
        (Some(0.0), _) => Some(0.0),
        _ => Some(a? % b?),
    }
}

fn variance(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)
}

fn statistic(name: &str, values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let sum: f64 = values.iter().sum();
    match name {
        "SUM" => sum,
        "MEAN" => sum / n,
        "MEDIAN" => {
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            let half = sorted.len() / 2;
            if sorted.len() % 2 == 1 {
                sorted[half]
            } else {
                (sorted[half - 1] + sorted[half]) / 2.0
            }
        }
        "MIN" => values.iter().copied().fold(f64::INFINITY, f64::min),
        "MAX" => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "VARIANCE" => variance(values),
        "SD" => variance(values).sqrt(),
        // a mean of zero gives a result that is not finite, which becomes missing
        _ => variance(values).sqrt() / (sum / n),
    }
}

/// Applies `f` to the valid values of each case across the columns.
fn across(columns: &[Numbers], cases: usize, f: impl Fn(&[f64]) -> Option<f64>) -> Column {
    let mut valid = Vec::with_capacity(columns.len());
    let results = (0..cases).map(|case| {
        valid.clear();
        valid.extend(columns.iter().filter_map(|column| column[case]));
        f(&valid).and_then(finite)
    });
    Column::Number(results.collect())
}

/// 1 when a candidate matches, 0 when none does but one could be compared, else missing.
/// Each candidate is Some(matches), or None when it is missing.
fn any_match(candidates: impl Iterator<Item = Option<bool>>) -> Option<f64> {
    let mut comparable = false;
    for matches in candidates.flatten() {
        if matches {
            return Some(1.0);
        }
        comparable = true;
    }
    comparable.then_some(0.0)
}

/// The formats NUMBER reads with, as the data input formats read them.
const NUMBER_FORMATS: &[(&str, FormatType)] = &[
    ("F", FormatType::F),
    ("N", FormatType::N),
    ("E", FormatType::E),
    ("COMMA", FormatType::Comma),
    ("DOT", FormatType::Dot),
    ("DOLLAR", FormatType::Dollar),
    ("PCT", FormatType::Pct),
    ("DATE", FormatType::Date),
    ("ADATE", FormatType::Adate),
    ("EDATE", FormatType::Edate),
    ("SDATE", FormatType::Sdate),
];

/// The formats STRING writes with: those every output shows.
const STRING_FORMATS: &[&str] = &[
    "F", "N", "E", "COMMA", "DOT", "DOLLAR", "PCT", "DATE", "ADATE", "EDATE", "JDATE", "SDATE",
    "QYR", "MOYR", "WKYR", "DATETIME", "YMDHMS", "TIME", "MTIME", "DTIME", "WKDAY", "MONTH",
];

/// The format argument of NUMBER and STRING (such as F8.2 or DATE11): its type in upper case,
/// width and decimals. The parser hands it over as a variable name.
// shortcut: NUMBER reads the formats DATA LIST reads (no TIME, DATETIME ...); add them there
fn format_arg(function: &str, expr: &Expr) -> Result<(String, usize, usize), Error> {
    let parsed = match expr {
        Expr::Variable(text) => {
            let letters = text.find(|c: char| !c.is_ascii_alphabetic());
            let (kind, size) = text.split_at(letters.unwrap_or(text.len()));
            let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
            let kind = kind.to_uppercase();
            let known = if function == "NUMBER" {
                NUMBER_FORMATS.iter().any(|(k, _)| *k == kind)
            } else {
                STRING_FORMATS.contains(&kind.as_str())
            };
            width
                .parse::<usize>()
                .ok()
                .zip(decimals.parse::<usize>().ok())
                .filter(|_| known)
                .map(|(width, decimals)| (kind, width, decimals))
        }
        _ => None,
    };
    let names = if function == "NUMBER" {
        NUMBER_FORMATS.iter().map(|(k, _)| *k).collect::<Vec<_>>()
    } else {
        STRING_FORMATS.to_vec()
    };
    match parsed {
        Some((kind, width, decimals)) if (1..=40).contains(&width) && decimals < width => {
            Ok((kind, width, decimals))
        }
        _ => Err(Error(format!(
            "argument 2 of {function} must be a format such as F8.2 (width 1 to 40, fewer \
             decimals than the width) of type {}",
            names.join(", ")
        ))),
    }
}

/// The first `width` characters of `text` read in the format `kind`; the SET BLANKS value when
/// they are blank, None when they cannot be read.
fn read_number(
    text: &str,
    kind: &str,
    width: usize,
    decimals: usize,
    reading: crate::data_list::Reading,
) -> Option<f64> {
    let field: String = text.chars().take(width).collect();
    let (_, kind) = NUMBER_FORMATS.iter().find(|(k, _)| *k == kind)?;
    if field.trim().is_empty() {
        return reading.blanks;
    }
    let read = match kind {
        FormatType::Date | FormatType::Adate | FormatType::Edate | FormatType::Sdate => {
            crate::data_list::date(&field, *kind, reading.epoch)
        }
        _ => crate::data_list::number(&field, *kind, decimals as u8),
    };
    read.ok().flatten()
}

/// The digits of `value` (not negative) with two more decimals than its format has. Every
/// output of the value is rounded half up from this text, also one with fewer decimals.
fn working_digits(value: f64, decimals: usize) -> String {
    let places = decimals + 2;
    let mut text = format!("{value:.places$}");
    // Ending in 50 the text sits on a half, but the value may be stored a little below it. The
    // reference program then decides by the decimals a number of this size holds reliably: 16
    // below 8, one fewer for each decimal digit of 2^exponent. So 0.285 becomes .29 and 1.005
    // stays 1.00. (The rule was found by comparing 3000 outputs.)
    let exponent = ((value.to_bits() >> 52) & 0x7ff) as i32 - 1022;
    let reliable = 16 - (f64::from(exponent) * std::f64::consts::LOG10_2).floor() as i32;
    let reliable = reliable.clamp(0, 16) as usize;
    if text.ends_with("50") && reliable > places {
        let precise = format!("{value:.reliable$}");
        if precise[..text.len()] < *text {
            text.replace_range(text.len() - 2.., "49");
        }
    }
    text
}

/// `working`, which has `places` decimals, rounded half up to `decimals` places.
fn round_digits(working: &str, places: usize, decimals: usize) -> String {
    let cut = working.len() - (places - decimals);
    let mut carry = working.as_bytes()[cut] >= b'5';
    let mut digits = working.as_bytes()[..cut - usize::from(decimals == 0)].to_vec();
    for digit in digits.iter_mut().rev().filter(|d| **d != b'.') {
        if !carry {
            break;
        }
        carry = *digit == b'9';
        *digit = if carry { b'0' } else { *digit + 1 };
    }
    if carry {
        digits.insert(0, b'1');
    }
    String::from_utf8(digits).expect("ASCII digits")
}

/// A number in F format: right-aligned, no zero before the decimal point, fewer decimals when
/// the width is too small, then scientific notation, then asterisks.
pub(crate) fn format_number(value: Option<f64>, width: usize, decimals: usize) -> String {
    let Some(value) = value else {
        return format!("{:>width$}", format!(".{:decimals$}", ""));
    };
    let negative = value < 0.0;
    let working = working_digits(value.abs(), decimals);
    for fewer in (0..=decimals).rev() {
        let digits = round_digits(&working, decimals + 2, fewer);
        let is_zero = !digits.bytes().any(|d| (b'1'..=b'9').contains(&d));
        let digits = match digits.strip_prefix('0') {
            Some(rest) if fewer > 0 => rest,
            _ => &digits,
        };
        let sign = if negative && !is_zero { "-" } else { "" };
        if sign.len() + digits.len() <= width {
            return format!("{:>width$}", format!("{sign}{digits}"));
        }
    }
    let sign = if negative { "-" } else { "" };
    // "1E+012" is the shortest form; a decimal point with digits needs at least two more
    let Some(room) = width.checked_sub(sign.len() + 6) else {
        return "*".repeat(width);
    };
    let decimals = decimals.min(room.saturating_sub(1));
    let text = format!("{:.decimals$e}", value.abs());
    let (mantissa, exponent) = text.split_once('e').expect("exponent format");
    let exponent: i32 = exponent.parse().expect("exponent format");
    format!("{:>width$}", format!("{sign}{mantissa}E{exponent:+04}"))
}

/// Days from 1970-01-01 to a date of the Gregorian calendar; the month may run past 1 to 12.
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year + (month - 1).div_euclid(12);
    let month = (month - 1).rem_euclid(12) + 1;
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// (year, month, day) of a number of days from 1970-01-01.
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The text of a numeric column as a function argument that must be a number.
fn numeric(column: Column, expr: &Expr, what: &str) -> Result<Numbers, Error> {
    match column {
        Column::Number(values) => Ok(values),
        Column::String(_) => Err(Error(format!(
            "{what} must be a number, but {} is a string",
            describe(expr)
        ))),
    }
}

/// `f` of the arguments of each case; a missing argument makes the result missing.
fn each_case(columns: &[Numbers], cases: usize, f: impl Fn(&[f64]) -> Option<f64>) -> Column {
    let value = |case: usize| {
        let row: Option<Vec<f64>> = columns.iter().map(|c| c[case]).collect();
        f(&row?).and_then(finite)
    };
    Column::Number((0..cases).map(value).collect())
}

/// The literal text of a DATEDIFF or DATESUM keyword argument such as "days".
fn keyword<'e>(function: &str, expr: Option<&'e Expr>, what: &str) -> Result<&'e str, Error> {
    match expr {
        Some(Expr::String(text)) => Ok(text),
        _ => Err(Error(format!(
            "the {what} of {function} must be a quoted word such as {}",
            if what == "unit" {
                "'days'"
            } else {
                "'closest'"
            }
        ))),
    }
}

impl Evaluator<'_> {
    pub fn eval(&self, expr: &Expr) -> Result<Column, Error> {
        let cases = self.data.case_count();
        Ok(match expr {
            // shortcut: a literal becomes a whole column, add a scalar kind of column when
            // large datasets make that show up in a profile
            Expr::Number(n) => Column::Number(vec![Some(*n); cases]),
            Expr::String(text) => Column::String(vec![text.clone(); cases]),
            Expr::Variable(name) => self.variable(name, true)?,
            Expr::Unary { op, operand } => {
                let operator = if *op == UnaryOp::Neg { "-" } else { "NOT" };
                let values = self.number(operand, &format!("the operand of {operator}"))?;
                match op {
                    UnaryOp::Neg => map(values, |v| -v),
                    UnaryOp::Not => {
                        let truth = self.truth(values, "the operand of NOT")?;
                        Column::Number(truth.into_iter().map(|v| boolean(!v?)).collect())
                    }
                }
            }
            Expr::Binary { op, left, right } => self.binary(*op, left, right)?,
            Expr::Call { name, args } => self.call(name, args)?,
            Expr::Range(first, last) => {
                return Err(Error(format!(
                    "{first} TO {last} can only be an argument of a function"
                )));
            }
        })
    }

    /// A condition: for each case true, false or missing.
    pub fn condition(&self, expr: &Expr) -> Result<Vec<Option<bool>>, Error> {
        let values = self.number(expr, "a condition")?;
        self.truth(values, "a condition")
    }

    fn variable(&self, name: &str, user_missing: bool) -> Result<Column, Error> {
        if name.eq_ignore_ascii_case("$SYSMIS") {
            return Ok(Column::Number(vec![None; self.data.case_count()]));
        }
        if name.eq_ignore_ascii_case("$CASENUM") {
            let first = self.first_case + 1;
            let cases = first..first + self.data.case_count();
            return Ok(Column::Number(
                cases.map(|case| Some(case as f64)).collect(),
            ));
        }
        if name.starts_with('$') {
            let name = name.to_uppercase();
            return Err(Error(format!(
                "system variable {name} is not supported yet"
            )));
        }
        let Ok(variable) = self.data.variable(name) else {
            return Err(Error(format!("unknown variable {name}")));
        };
        Ok(match variable.values() {
            Values::Numeric(values) => {
                let missing = |v: &f64| user_missing && variable.is_user_missing(*v);
                Column::Number(values.iter().map(|v| v.filter(|v| !missing(v))).collect())
            }
            Values::String(values) => {
                let width = variable.string_width().unwrap_or(0);
                let padded = |v: &str| format!("{v}{:1$}", "", width.saturating_sub(v.len()));
                Column::String(values.iter().map(padded).collect())
            }
        })
    }

    fn number(&self, expr: &Expr, what: &str) -> Result<Numbers, Error> {
        numeric(self.eval(expr)?, expr, what)
    }

    fn text(&self, expr: &Expr, what: &str) -> Result<Vec<String>, Error> {
        match self.eval(expr)? {
            Column::String(values) => Ok(values),
            Column::Number(_) => Err(Error(format!(
                "{what} must be a string, but {} is a number",
                describe(expr)
            ))),
        }
    }

    /// 0 and 1 as false and true. Any other value is an error in an active case.
    fn truth(&self, values: Numbers, what: &str) -> Result<Vec<Option<bool>>, Error> {
        let check = |(case, value): (usize, Option<f64>)| match value {
            Some(v) if v == 0.0 || v == 1.0 => Ok(Some(v == 1.0)),
            Some(v) if self.active.is_none_or(|active| active[case]) => Err(Error(format!(
                "{what} must be 0 or 1, but it is {v} in case {}; compare the value instead, \
                 for example with > 0",
                case + 1
            ))),
            _ => Ok(None),
        };
        values.into_iter().enumerate().map(check).collect()
    }

    fn binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Column, Error> {
        let name = symbol(op);
        let side = |side: &str| format!("the {side} operand of {name}");
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let a = self.number(left, &side("left"))?;
                let b = self.number(right, &side("right"))?;
                let a = self.truth(a, &side("left"))?;
                let b = self.truth(b, &side("right"))?;
                // the value that decides the result whatever the other operand is
                let decisive = op == BinaryOp::Or;
                let results = a.into_iter().zip(b).map(|(a, b)| {
                    if a == Some(decisive) || b == Some(decisive) {
                        boolean(decisive)
                    } else {
                        a.and(b).map(|_| f64::from(u8::from(!decisive)))
                    }
                });
                Ok(Column::Number(results.collect()))
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
                let a = self.number(left, &side("left"))?;
                let b = self.number(right, &side("right"))?;
                Ok(zip(a, b, |a, b| arithmetic(op, a, b)))
            }
            _ => match (self.eval(left)?, self.eval(right)?) {
                (Column::Number(a), Column::Number(b)) => {
                    Ok(zip(a, b, |a, b| boolean(holds(op, a?.partial_cmp(&b?)?))))
                }
                (Column::String(a), Column::String(b)) => {
                    let results = a.iter().zip(&b);
                    let results = results.map(|(a, b)| boolean(holds(op, compare_text(a, b))));
                    Ok(Column::Number(results.collect()))
                }
                (Column::Number(_), _) => Err(Error(format!(
                    "{name} cannot compare a number with a string: {} is a number, {} is a string",
                    describe(left),
                    describe(right)
                ))),
                _ => Err(Error(format!(
                    "{name} cannot compare a string with a number: {} is a string, {} is a number",
                    describe(left),
                    describe(right)
                ))),
            },
        }
    }

    /// `a TO b` arguments become one argument per variable, in dictionary order.
    fn expand(&self, args: &[Expr]) -> Result<Vec<Expr>, Error> {
        let mut expanded = Vec::new();
        for arg in args {
            let Expr::Range(first, last) = arg else {
                expanded.push(arg.clone());
                continue;
            };
            let index = |name: &str| {
                let found = self.data.variable_index(name);
                found.ok_or_else(|| Error(format!("unknown variable {name}")))
            };
            let (from, to) = (index(first)?, index(last)?);
            if from > to {
                return Err(Error(format!(
                    "in {first} TO {last} the first variable must come before the second in the file"
                )));
            }
            let names = (from..=to).map(|i| self.data.variable_at(i).name().to_string());
            expanded.extend(names.map(Expr::Variable));
        }
        Ok(expanded)
    }

    /// LAG(variable, n): the value n cases back, as that case ended the pass; system-missing
    /// (also for a user-missing value) or blank for the first n cases.
    fn lag(&self, args: &[Expr]) -> Result<Column, Error> {
        let usage =
            "LAG needs a variable and an optional whole number of cases, as LAG(x) or LAG(x, 2)";
        let (Some(Expr::Variable(name)), true) = (args.first(), args.len() <= 2) else {
            return Err(Error(usage.into()));
        };
        let n = match args.get(1) {
            None => 1,
            Some(Expr::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= 1e6 => *n as usize,
            Some(_) => return Err(Error(usage.into())),
        };
        let variable = self
            .data
            .variable(name)
            .map_err(|_| Error(format!("unknown variable {name}")))?;
        let cases = self.data.case_count();
        // the value of case `case - n`: from the cases here, else from those before
        let back = |case: usize| -> Option<(&Dataset, usize)> {
            match case.checked_sub(n) {
                Some(earlier) => Some((self.data, earlier)),
                None => {
                    let index = self.previous.len().checked_sub(n - case)?;
                    Some((&self.previous[index], 0))
                }
            }
        };
        Ok(match variable.values() {
            Values::Numeric(_) => Column::Number(
                (0..cases)
                    .map(|case| {
                        let (data, at) = back(case)?;
                        // a user-missing value reads as missing, as in any expression
                        let value = data.variable(name).ok()?.numeric().ok()?.get(at)?;
                        (!variable.is_user_missing(value)).then_some(value)
                    })
                    .collect(),
            ),
            Values::String(_) => {
                let width = variable.string_width().unwrap_or(0);
                Column::String(
                    (0..cases)
                        .map(|case| {
                            let text = back(case).and_then(|(data, at)| {
                                Some(data.variable(name).ok()?.string().ok()?.get(at).to_string())
                            });
                            let text = text.unwrap_or_default();
                            format!("{text}{:1$}", "", width.saturating_sub(text.len()))
                        })
                        .collect(),
                )
            }
        })
    }

    fn call(&self, name: &str, args: &[Expr]) -> Result<Column, Error> {
        let upper = full_name(name.to_uppercase());
        // `MEAN.2`: the suffix is the number of valid arguments the result needs
        let suffix = upper.rsplit_once('.').and_then(|(base, n)| {
            STATISTICS.iter().find(|(known, _)| *known == base)?;
            n.parse::<usize>().ok().map(|n| (base, n))
        });
        let function = suffix.map_or(upper.as_str(), |(base, _)| base);
        // the fewest arguments a statistical function takes, and the valid values it needs
        let least = STATISTICS.iter().find(|(known, _)| *known == function);
        let least = least.map_or(0, |(_, n)| *n);
        let needed = suffix.map_or(least, |(_, n)| n.max(least));
        if let (Some(variables), [index]) = (self.vectors.get(&name.to_lowercase()), args) {
            let Column::Number(indexes) = self.eval(index)? else {
                return Err(Error(format!("the index of {name} must be a number")));
            };
            return crate::loops::element_values(self.data, variables, &indexes);
        }
        if upper == "LAG" {
            return self.lag(args);
        }
        let args = &self.expand(args)?[..];
        let cases = self.data.case_count();
        let argument = |i: usize| format!("argument {} of {function}", i + 1);
        let number = |i: usize| self.number(&args[i], &argument(i));
        let text = |i: usize| self.text(&args[i], &argument(i));
        let numbers = || (0..args.len()).map(number).collect::<Result<Vec<_>, _>>();
        let one = |f: fn(f64) -> f64| number(0).map(|values| map(values, f));
        let constant = |value: f64| vec![Some(value); cases];
        // an argument that was left out, with its default value
        let optional = |i: usize, default: f64| match args.get(i) {
            Some(_) => number(i),
            None => Ok(constant(default)),
        };
        let rounded = |half: f64| -> Result<Column, Error> {
            let (x, multiple, fuzzbits) =
                (number(0)?, optional(1, 1.0)?, optional(2, self.fuzzbits)?);
            let results = x.into_iter().zip(multiple).zip(fuzzbits);
            let results = results.map(|((x, m), f)| round(x, m, f, half).and_then(finite));
            Ok(Column::Number(results.collect()))
        };
        // an optional numeric argument: None when it is left out
        let given = |i: usize| args.get(i).map(|_| number(i)).transpose();
        let known = FUNCTIONS.iter().any(|(f, _)| *f == function);

        Ok(match (function, args.len()) {
            ("ABS", 1) => one(f64::abs)?,
            ("SQRT", 1) => one(f64::sqrt)?,
            ("EXP", 1) => one(f64::exp)?,
            ("LN", 1) => one(f64::ln)?,
            ("LG10", 1) => one(f64::log10)?,
            // the manual allows x > 0 only; Γ(1) = Γ(2) = 1 exactly, where the series is a
            // hair off
            ("LNGAMMA", 1) => one(|x| match x {
                1.0 | 2.0 => 0.0,
                _ if x > 0.0 => dist::ln_gamma(x),
                _ => f64::NAN,
            })?,
            ("SIN", 1) => one(f64::sin)?,
            ("COS", 1) => one(f64::cos)?,
            ("TAN", 1) => one(f64::tan)?,
            ("ARSIN", 1) => one(f64::asin)?,
            ("ARCOS", 1) => one(f64::acos)?,
            ("ARTAN", 1) => one(f64::atan)?,
            ("RND", 1..=3) => rounded(0.5)?,
            ("TRUNC", 1..=3) => rounded(0.0)?,
            ("MOD", 2) => zip(number(0)?, number(1)?, modulo),

            (_, count) if least > 0 && count >= least => {
                let columns: Vec<Column> = args
                    .iter()
                    .map(|a| self.eval(a))
                    .collect::<Result<_, _>>()?;
                let strings = columns.iter().all(|c| matches!(c, Column::String(_)));
                if strings && suffix.is_none() && matches!(function, "MIN" | "MAX") {
                    let texts: Vec<Vec<String>> = columns
                        .into_iter()
                        .filter_map(|c| match c {
                            Column::String(values) => Some(values),
                            Column::Number(_) => None,
                        })
                        .collect();
                    let pick = |case: usize| {
                        let values = texts.iter().map(|t| &t[case]);
                        let order = |a: &&String, b: &&String| compare_text(a, b);
                        let best = if function == "MIN" {
                            values.min_by(order)
                        } else {
                            values.max_by(order)
                        };
                        best.cloned().unwrap_or_default()
                    };
                    Column::String((0..cases).map(pick).collect())
                } else {
                    let columns = columns.into_iter().zip(args).enumerate();
                    let columns =
                        columns.map(|(i, (column, arg))| numeric(column, arg, &argument(i)));
                    // MEAN.4 of three arguments is missing, as the manual says
                    across(&columns.collect::<Result<Vec<_>, _>>()?, cases, |valid| {
                        (valid.len() >= needed).then(|| statistic(function, valid))
                    })
                }
            }
            (_, count) if least > 0 && count >= 1 => {
                return Err(Error(format!(
                    "{upper} needs at least {least} arguments, got {count}"
                )));
            }
            ("NVALID", 1..) => across(&numbers()?, cases, |valid| Some(valid.len() as f64)),
            ("NMISS", count @ 1..) => across(&numbers()?, cases, |valid| {
                Some((count - valid.len()) as f64)
            }),
            ("MISSING", 1) => match self.eval(&args[0])? {
                Column::Number(values) => {
                    Column::Number(values.iter().map(|v| boolean(v.is_none())).collect())
                }
                // MISSING is the one function that sees user-missing values of a string variable
                Column::String(values) => {
                    let variable = match &args[0] {
                        Expr::Variable(name) => self.data.variable(name).ok(),
                        _ => None,
                    };
                    let missing = |v: &String| {
                        variable
                            .as_ref()
                            .is_some_and(|var| var.is_user_missing_str(v))
                    };
                    Column::Number(values.iter().map(|v| boolean(missing(v))).collect())
                }
            },
            ("SYSMIS", 1) => {
                let mut values = number(0)?;
                // a user-missing value of a variable is not system-missing
                if let Expr::Variable(name) = &args[0]
                    && let Column::Number(raw) = self.variable(name, false)?
                {
                    values = raw;
                }
                Column::Number(values.iter().map(|v| boolean(v.is_none())).collect())
            }
            ("VALUE", 1) => match &args[0] {
                Expr::Variable(name) => self.variable(name, false)?,
                other => {
                    return Err(Error(format!(
                        "the argument of VALUE must be a variable name, but it is {}",
                        describe(other)
                    )));
                }
            },

            ("ANY", count @ 2..) => match self.eval(&args[0])? {
                Column::Number(values) => {
                    let candidates = (1..count).map(number).collect::<Result<Vec<_>, _>>()?;
                    let any = |(case, value): (usize, Option<f64>)| {
                        let value = value?;
                        any_match(candidates.iter().map(|c| c[case].map(|c| c == value)))
                    };
                    Column::Number(values.into_iter().enumerate().map(any).collect())
                }
                Column::String(values) => {
                    let candidates = (1..count).map(text).collect::<Result<Vec<_>, _>>()?;
                    let any = |(case, value): (usize, &String)| {
                        let equal = |c: &Vec<String>| compare_text(&c[case], value).is_eq();
                        boolean(candidates.iter().any(equal))
                    };
                    Column::Number(values.iter().enumerate().map(any).collect())
                }
            },
            ("RANGE", count @ 3..) if count % 2 == 1 => match self.eval(&args[0])? {
                Column::Number(values) => {
                    let bounds = (1..count).map(number).collect::<Result<Vec<_>, _>>()?;
                    let within = |(case, value): (usize, Option<f64>)| {
                        let value = value?;
                        let pairs = || {
                            bounds
                                .chunks(2)
                                .map(|pair| pair[0][case].zip(pair[1][case]))
                        };
                        // a range that runs backwards makes the result missing, even next to
                        // one that matches
                        if pairs().flatten().any(|(low, high)| low > high) {
                            return None;
                        }
                        any_match(pairs().map(|pair| pair.map(|(l, h)| l <= value && value <= h)))
                    };
                    Column::Number(values.into_iter().enumerate().map(within).collect())
                }
                Column::String(values) => {
                    let bounds = (1..count).map(text).collect::<Result<Vec<_>, _>>()?;
                    let within = |(case, value): (usize, &String)| {
                        let pairs = || {
                            bounds
                                .chunks(2)
                                .map(|pair| (&pair[0][case], &pair[1][case]))
                        };
                        if pairs().any(|(low, high)| compare_text(low, high).is_gt()) {
                            return None;
                        }
                        boolean(pairs().any(|(low, high)| {
                            compare_text(low, value).is_le() && compare_text(value, high).is_le()
                        }))
                    };
                    Column::Number(values.iter().enumerate().map(within).collect())
                }
            },

            ("CONCAT", count @ 1..) => {
                let parts = (0..count).map(text).collect::<Result<Vec<_>, _>>()?;
                let joined = |case: usize| parts.iter().map(|p| p[case].as_str()).collect();
                Column::String((0..cases).map(joined).collect())
            }
            ("SUBSTR" | "CHAR.SUBSTR", 2 | 3) => {
                let (values, start) = (text(0)?, number(1)?);
                let length = optional(2, f64::MAX)?;
                let part = |((value, start), length): ((&String, Option<f64>), Option<f64>)| {
                    let (Some(start), Some(length)) = (start, length) else {
                        return String::new();
                    };
                    if start < 1.0 || length < 1.0 {
                        return String::new();
                    }
                    let skipped = value.chars().skip(start as usize - 1);
                    skipped.take(length as usize).collect()
                };
                Column::String(values.iter().zip(start).zip(length).map(part).collect())
            }
            ("LENGTH", 1) => {
                Column::Number(text(0)?.iter().map(|v| Some(v.len() as f64)).collect())
            }
            // without trailing blanks, as the manual says
            ("CHAR.LENGTH", 1) => {
                let length = |v: &String| Some(v.trim_end_matches(' ').chars().count() as f64);
                Column::Number(text(0)?.iter().map(length).collect())
            }
            ("UPCASE", 1) => map_text(text(0)?, str::to_uppercase),
            ("LOWER", 1) => map_text(text(0)?, str::to_lowercase),
            ("LTRIM", 1) => map_text(text(0)?, |v| v.trim_start_matches(' ').to_string()),
            ("RTRIM", 1) => map_text(text(0)?, |v| v.trim_end_matches(' ').to_string()),
            ("LTRIM" | "RTRIM", 2) => {
                let (values, what) = (text(0)?, text(1)?);
                let left = function == "LTRIM";
                let trimmed = |(value, what): (&String, &String)| f::trim(value, what, left);
                Column::String(values.iter().zip(&what).map(trimmed).collect())
            }
            ("NTRIM", 1) => match &args[0] {
                Expr::Variable(name) => self.variable(name, true)?,
                other => {
                    return Err(Error(format!(
                        "the argument of NTRIM must be a variable name, but it is {}",
                        describe(other)
                    )));
                }
            },
            ("INDEX" | "CHAR.INDEX" | "RINDEX" | "CHAR.RINDEX", 2 | 3) => {
                let (values, needles, divisors) = (text(0)?, text(1)?, given(2)?);
                let last = function.ends_with("RINDEX");
                let position = |case: usize| {
                    let divisor = divisors.as_ref().map(|d| d[case]);
                    f::find(&values[case], &needles[case], divisor, last)
                };
                Column::Number((0..cases).map(position).collect())
            }
            ("LPAD" | "CHAR.LPAD" | "RPAD" | "CHAR.RPAD", 2 | 3) => {
                let (values, lengths) = (text(0)?, number(1)?);
                let pads = match args.get(2) {
                    Some(_) => text(2)?,
                    None => vec![" ".to_string(); cases],
                };
                let left = function.ends_with("LPAD");
                let padded = |case: usize| f::pad(&values[case], lengths[case], &pads[case], left);
                Column::String((0..cases).map(padded).collect())
            }
            ("REPLACE", 3 | 4) => {
                let (values, old, new, counts) = (text(0)?, text(1)?, text(2)?, given(3)?);
                let replaced = |case: usize| {
                    let count = match &counts {
                        None => None,
                        Some(counts) => match f::count(counts[case], 0.0) {
                            Some(count) => Some(count),
                            None => return String::new(),
                        },
                    };
                    f::replace(&values[case], &old[case], &new[case], count)
                };
                Column::String((0..cases).map(replaced).collect())
            }
            ("STRUNC", 2) => {
                let (values, lengths) = (text(0)?, number(1)?);
                let cut = |(value, length): (&String, Option<f64>)| {
                    f::count(length, 0.0).map_or(String::new(), |bytes| f::strunc(value, bytes))
                };
                Column::String(values.iter().zip(lengths).map(cut).collect())
            }
            ("CHAR.MBLEN" | "MBLEN.BYTE", 2) => {
                let (values, positions) = (text(0)?, number(1)?);
                let bytes = function == "MBLEN.BYTE";
                let length =
                    |(value, position): (&String, Option<f64>)| f::mblen(value, position?, bytes);
                Column::Number(values.iter().zip(positions).map(length).collect())
            }
            ("NORMALIZE", 1) => map_text(text(0)?, |v| v.nfc().collect()),
            ("VALUELABEL", 1) => self.value_label(&args[0])?,
            ("NUMBER", 2) => {
                let (kind, width, decimals) = format_arg(function, &args[1])?;
                let read = |v: &String| read_number(v, &kind, width, decimals, self.reading);
                Column::Number(text(0)?.iter().map(read).collect())
            }
            ("STRING", 2) => {
                let (kind, width, decimals) = format_arg(function, &args[1])?;
                let spec = format!("{kind}{width}.{decimals}");
                let write = |v: Option<f64>| match v {
                    Some(v) => format!("{:>width$}", format_value(v, &spec, false)),
                    None => format_number(None, width, decimals),
                };
                Column::String(number(0)?.into_iter().map(write).collect())
            }

            ("DATE.DMY" | "DATE.MDY", 3) => {
                let dmy = function == "DATE.DMY";
                each_case(&numbers()?, cases, |p| {
                    let (day, month) = if dmy { (p[0], p[1]) } else { (p[1], p[0]) };
                    f::date(
                        day,
                        month,
                        crate::data_list::full_year(p[2], self.reading.epoch),
                    )
                })
            }
            ("DATE.MOYR" | "DATE.QYR" | "DATE.WKYR" | "DATE.YRDAY", 2) => {
                each_case(&numbers()?, cases, |p| {
                    f::date_of(function, p[0], p[1], self.reading.epoch)
                })
            }
            ("YRMODA", 3) => each_case(&numbers()?, cases, |p| f::yrmoda(p[0], p[1], p[2])),
            ("TIME.DAYS", 1) => one(|days| days * f::DAY)?,
            ("TIME.HMS", 1..=3) => each_case(&numbers()?, cases, f::time_hms),
            ("CTIME.DAYS", 1) => one(|time| time / f::DAY)?,
            ("CTIME.HOURS", 1) => one(|time| time / 3600.0)?,
            ("CTIME.MINUTES", 1) => one(|time| time / 60.0)?,
            ("CTIME.SECONDS", 1) => one(|time| time)?,
            (_, 1) if known && function.starts_with("XDATE.") => {
                each_case(&numbers()?, cases, |p| f::xdate(function, p[0]))
            }
            ("DATEDIFF", 3) => {
                let unit = self.unit(function, args.get(2))?;
                let dates = [number(0)?, number(1)?];
                each_case(&dates, cases, |p| f::date_diff(p[0], p[1], unit))
            }
            ("DATESUM", 3 | 4) => {
                let unit = self.unit(function, args.get(2))?;
                let rollover = match args.get(3) {
                    None => false,
                    method => match keyword(function, method, "method")?.trim().to_lowercase() {
                        m if m == "rollover" => true,
                        m if m == "closest" => false,
                        m => {
                            return Err(Error(format!(
                                "the method of DATESUM must be 'rollover' or 'closest', not '{m}'"
                            )));
                        }
                    },
                };
                let values = [number(0)?, number(1)?];
                each_case(&values, cases, |p| f::date_sum(p[0], p[1], unit, rollover))
            }

            ("CDFNORM", 1) => each_case(&numbers()?, cases, |p| {
                distributions::evaluate("CDF.NORMAL", &[p[0], 0.0, 1.0])
            }),
            ("PROBIT", 1) => each_case(&numbers()?, cases, |p| {
                distributions::evaluate("IDF.NORMAL", &[p[0], 0.0, 1.0])
            }),
            // a parameter or value out of range gives system-missing
            (_, count) if distribution_arity(function) == Some(count) => {
                each_case(&numbers()?, cases, |p| {
                    let valid = distributions::x_in_range(function, p);
                    valid
                        .then(|| distributions::evaluate(function, p))
                        .flatten()
                })
            }
            (_, count) if crate::random::arity(function) == Some(count) => {
                let parameters = numbers()?;
                let random = &mut *self.random.borrow_mut();
                let value = |case: usize| {
                    if self.active.is_some_and(|active| !active[case]) {
                        return None;
                    }
                    let p: Option<Vec<f64>> = parameters.iter().map(|c| c[case]).collect();
                    crate::random::draw(random, function, &p?).and_then(finite)
                };
                Column::Number((0..cases).map(value).collect())
            }

            (_, count) => {
                let usage = FUNCTIONS.iter().find(|(known, _)| *known == function);
                return Err(Error(match usage {
                    Some((_, usage)) => format!(
                        "wrong number of arguments for {function}: expected {usage}, got {count}"
                    ),
                    None if UNSUPPORTED.contains(&upper.as_str()) => {
                        format!("function {upper} is not supported yet")
                    }
                    None => format!("unknown function {upper}"),
                }));
            }
        })
    }

    /// The unit argument of DATEDIFF and DATESUM.
    fn unit(&self, function: &str, expr: Option<&Expr>) -> Result<f::Unit, Error> {
        let name = keyword(function, expr, "unit")?;
        f::unit(name).ok_or_else(|| {
            Error(format!(
                "the unit of {function} must be years, quarters, months, weeks, days, hours, \
                 minutes or seconds, not '{name}'"
            ))
        })
    }

    /// VALUELABEL(variable): the label of each value, empty where it has none.
    fn value_label(&self, arg: &Expr) -> Result<Column, Error> {
        let Expr::Variable(name) = arg else {
            return Err(Error(format!(
                "the argument of VALUELABEL must be a variable name, but it is {}",
                describe(arg)
            )));
        };
        let variable = self
            .data
            .variable(name)
            .map_err(|_| Error(format!("unknown variable {name}")))?;
        let labels = variable.value_labels();
        let label = |value: Value| {
            let found = labels.iter().find(|(v, _)| *v == value);
            found.map_or(String::new(), |(_, label)| label.clone())
        };
        Ok(Column::String(match variable.values() {
            Values::Numeric(values) => values
                .iter()
                .map(|v| v.map_or(String::new(), |v| label(Value::Number(v))))
                .collect(),
            Values::String(values) => values
                .iter()
                .map(|v| label(Value::String(v.trim_end().to_string())))
                .collect(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_distribution_and_random_function_is_listed_with_its_arguments() {
        let listed = |name: &str| FUNCTIONS.iter().find(|(f, _)| *f == name);
        let mut distributions = 0;
        for name in statscraft_syntax::matrix_function_names() {
            if let Some(arity) = distribution_arity(&name) {
                let (_, form) = listed(&name).unwrap_or_else(|| panic!("{name} is not listed"));
                assert_eq!(form.matches(',').count() + 1, arity, "{form}");
                distributions += 1;
            } else if name.starts_with("RV.") {
                assert!(listed(&name).is_some(), "{name} is not listed");
            }
        }
        assert_eq!(distributions, 76);
    }

    #[test]
    fn the_help_names_every_function() {
        let help = statscraft_syntax::help(None).expect("the general help");
        let words: Vec<&str> = help
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
            .map(|w| w.trim_end_matches('.'))
            .collect();
        for (name, _) in FUNCTIONS {
            let (family, distribution) = name.split_once('.').unwrap_or((name, ""));
            let listed = if DISTRIBUTION_FAMILIES.contains(&family) || family == "RV" {
                words.contains(&format!("{family}.d").as_str()) && words.contains(&distribution)
            } else {
                words.contains(name)
            };
            assert!(listed, "{name} is not in the help");
        }
    }
}
