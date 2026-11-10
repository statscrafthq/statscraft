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
//! counts bytes (as widths do); SUBSTR, INDEX and CHAR.LENGTH count characters.

use crate::Error;
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{BinaryOp, Expr, UnaryOp};
use std::cmp::Ordering;

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
    ("SUM", "SUM(number, ...)"),
    ("MEAN", "MEAN(number, ...)"),
    ("MIN", "MIN(number, ...)"),
    ("MAX", "MAX(number, ...)"),
    ("SD", "SD(number, number, ...)"),
    ("VARIANCE", "VARIANCE(number, number, ...)"),
    ("CFVAR", "CFVAR(number, number, ...)"),
    ("NVALID", "NVALID(number, ...)"),
    ("NMISS", "NMISS(number, ...)"),
    ("MISSING", "MISSING(variable)"),
    ("SYSMIS", "SYSMIS(number)"),
    ("VALUE", "VALUE(variable)"),
    ("ANY", "ANY(value, candidate, ...)"),
    ("RANGE", "RANGE(value, low, high [, low, high ...])"),
    ("CONCAT", "CONCAT(string, ...)"),
    ("SUBSTR", "SUBSTR(string, position [, length])"),
    ("CHAR.SUBSTR", "CHAR.SUBSTR(string, position [, length])"),
    ("LENGTH", "LENGTH(string)"),
    ("CHAR.LENGTH", "CHAR.LENGTH(string)"),
    ("UPCASE", "UPCASE(string)"),
    ("LOWER", "LOWER(string)"),
    ("LTRIM", "LTRIM(string)"),
    ("RTRIM", "RTRIM(string)"),
    ("INDEX", "INDEX(string, text to find)"),
    ("CHAR.INDEX", "CHAR.INDEX(string, text to find)"),
    ("NUMBER", "NUMBER(string, Fw.d)"),
    ("STRING", "STRING(number, Fw.d)"),
    ("DATE.DMY", "DATE.DMY(day, month, year)"),
    ("DATE.MDY", "DATE.MDY(month, day, year)"),
    ("XDATE.YEAR", "XDATE.YEAR(date)"),
    ("XDATE.MONTH", "XDATE.MONTH(date)"),
    ("XDATE.MDAY", "XDATE.MDAY(date)"),
];

/// Functions that take a `.n` suffix, with the number of valid arguments they need without one.
const STATISTICS: &[(&str, usize)] = &[
    ("SUM", 1),
    ("MEAN", 1),
    ("MIN", 1),
    ("MAX", 1),
    ("SD", 2),
    ("VARIANCE", 2),
    ("CFVAR", 2),
];

/// Functions of the language that are not evaluated yet. A name ending in a period stands for
/// every function with that prefix.
const UNSUPPORTED: &[&str] = &[
    "ARCOS",
    "ARSIN",
    "ARTAN",
    "COS",
    "SIN",
    "TAN",
    "MEDIAN",
    "LAG",
    "LNGAMMA",
    "NORMAL",
    "UNIFORM",
    "CDFNORM",
    "PROBIT",
    "YRMODA",
    "DATEDIFF",
    "DATESUM",
    "REPLACE",
    "LPAD",
    "RPAD",
    "STRUNC",
    "RINDEX",
    "NTRIM",
    "NORMALIZE",
    "VALUELABEL",
    "MBLEN.BYTE",
    "CHAR.",
    "DATE.",
    "XDATE.",
    "TIME.",
    "CTIME.",
    "RV.",
    "CDF.",
    "PDF.",
    "IDF.",
    "SIG.",
    "NCDF.",
    "NPDF.",
];

const SECONDS_PER_DAY: f64 = 86400.0;
/// Days from 1582-10-14, where date values start, to 1970-01-01.
const DAYS_TO_1970: i64 = 141_428;

pub(crate) struct Evaluator<'a> {
    pub data: &'a Dataset,
    /// The cases the result is used for; None means all. A value that is an error (an operand
    /// of AND that is not 0 or 1) is reported only for these cases.
    pub active: Option<&'a [bool]>,
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

/// The `Fw.d` argument of NUMBER and STRING. The parser hands it over as a variable name.
// shortcut: only F formats, add the others (N, COMMA, DATE ...) when a real script needs one
fn f_format(function: &str, expr: &Expr) -> Result<(usize, usize), Error> {
    let parsed = match expr {
        Expr::Variable(text) if text.starts_with(['F', 'f']) => {
            let (width, decimals) = text[1..].split_once('.').unwrap_or((&text[1..], "0"));
            width.parse().ok().zip(decimals.parse().ok())
        }
        _ => None,
    };
    match parsed {
        Some((width, decimals)) if (1..=40).contains(&width) && decimals < width => {
            Ok((width, decimals))
        }
        _ => Err(Error(format!(
            "argument 2 of {function} must be an F format such as F8.2 (width 1 to 40, \
             fewer decimals than the width); other formats are not supported yet"
        ))),
    }
}

/// Reads the first `width` characters of `text` as a number. Without a decimal point or an
/// exponent the last `decimals` digits are decimals, as in a fixed-column data file.
fn parse_number(text: &str, width: usize, decimals: usize) -> Option<f64> {
    let field: String = text.chars().take(width).collect();
    let field = field.trim();
    let unsigned = field.strip_prefix(['+', '-']).unwrap_or(field).trim_start();
    let end = unsigned
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(unsigned.len());
    let (mantissa, rest) = unsigned.split_at(end);
    let exponent = match rest.strip_prefix(['e', 'E', 'd', 'D']) {
        _ if rest.is_empty() && mantissa.contains('.') => 0,
        _ if rest.is_empty() => -(decimals as i32),
        // `1+2` is 100: the exponent may come without its letter
        None if rest.starts_with(['+', '-']) => rest.parse().ok()?,
        None => return None,
        Some(digits) => digits.parse().ok()?,
    };
    let value: f64 = mantissa.parse().ok()?;
    let value: f64 = format!("{value}e{exponent}").parse().ok()?;
    finite(if field.starts_with('-') {
        -value
    } else {
        value
    })
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
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
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

/// The date value of a day: seconds from 1582-10-14. Day 0 to 31 and month 1 to 13 are
/// accepted and carry into the neighbouring month or year.
pub(crate) fn date(day: Option<f64>, month: Option<f64>, year: Option<f64>) -> Option<f64> {
    let whole = |v: Option<f64>| v.filter(|v| v.fract() == 0.0 && v.abs() < 1e6);
    let (day, month, year) = (
        whole(day)? as i64,
        whole(month)? as i64,
        whole(year)? as i64,
    );
    // shortcut: a two-digit year is missing instead of being moved into a century window,
    // add the window when SET EPOCH is supported
    if !(0..=31).contains(&day) || !(1..=13).contains(&month) || year < 1582 {
        return None;
    }
    let days = days_from_civil(year, month, day) + DAYS_TO_1970;
    (days >= 1).then_some(days as f64 * SECONDS_PER_DAY)
}

/// (year, month, day) of a date value; dates before 1582-10-15 are missing.
fn date_parts(value: f64) -> Option<(i64, i64, i64)> {
    let days = (value / SECONDS_PER_DAY).floor();
    (1.0..1e9)
        .contains(&days)
        .then(|| civil_from_days(days as i64 - DAYS_TO_1970))
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
            let cases = 1..=self.data.case_count();
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
        match self.eval(expr)? {
            Column::Number(values) => Ok(values),
            Column::String(_) => Err(Error(format!(
                "{what} must be a number, but {} is a string",
                describe(expr)
            ))),
        }
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

    fn call(&self, name: &str, args: &[Expr]) -> Result<Column, Error> {
        let upper = name.to_uppercase();
        // `MEAN.2`: the suffix is the number of valid arguments the result needs
        let suffix = upper.rsplit_once('.').and_then(|(base, n)| {
            let needed = STATISTICS.iter().find(|(known, _)| *known == base)?.1;
            n.parse::<usize>().ok().map(|n| (base, n.max(needed)))
        });
        let (function, needed) = match suffix {
            Some((base, n)) => (base, n),
            None => {
                let needed = STATISTICS.iter().find(|(known, _)| *known == upper);
                (upper.as_str(), needed.map_or(0, |(_, n)| *n))
            }
        };
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
            let (x, multiple, fuzzbits) = (number(0)?, optional(1, 1.0)?, optional(2, 6.0)?);
            let results = x.into_iter().zip(multiple).zip(fuzzbits);
            let results = results.map(|((x, m), f)| round(x, m, f, half).and_then(finite));
            Ok(Column::Number(results.collect()))
        };
        let date_part = |part: fn((i64, i64, i64)) -> i64| -> Result<Column, Error> {
            let part = |v: Option<f64>| date_parts(v?).map(|parts| part(parts) as f64);
            Ok(Column::Number(number(0)?.into_iter().map(part).collect()))
        };

        Ok(match (function, args.len()) {
            ("ABS", 1) => one(f64::abs)?,
            ("SQRT", 1) => one(f64::sqrt)?,
            ("EXP", 1) => one(f64::exp)?,
            ("LN", 1) => one(f64::ln)?,
            ("LG10", 1) => one(f64::log10)?,
            ("RND", 1..=3) => rounded(0.5)?,
            ("TRUNC", 1..=3) => rounded(0.0)?,
            ("MOD", 2) => zip(number(0)?, number(1)?, modulo),

            ("SUM" | "MEAN" | "MIN" | "MAX" | "SD" | "VARIANCE" | "CFVAR", count)
                if count >= 1 && count >= needed =>
            {
                across(&numbers()?, cases, |valid| {
                    (valid.len() >= needed).then(|| statistic(function, valid))
                })
            }
            ("SUM" | "MEAN" | "MIN" | "MAX" | "SD" | "VARIANCE" | "CFVAR", count) if count >= 1 => {
                return Err(Error(format!(
                    "{upper} needs at least {needed} arguments, got {count}"
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
            ("CHAR.LENGTH", 1) => {
                let length = |v: &String| Some(v.chars().count() as f64);
                Column::Number(text(0)?.iter().map(length).collect())
            }
            ("UPCASE", 1) => map_text(text(0)?, str::to_uppercase),
            ("LOWER", 1) => map_text(text(0)?, str::to_lowercase),
            ("LTRIM", 1) => map_text(text(0)?, |v| v.trim_start_matches(' ').to_string()),
            ("RTRIM", 1) => map_text(text(0)?, |v| v.trim_end_matches(' ').to_string()),
            ("INDEX" | "CHAR.INDEX", 2) => {
                let (values, needles) = (text(0)?, text(1)?);
                let position = |(value, needle): (&String, &String)| {
                    let found = value.find(needle.as_str());
                    Some(found.map_or(0.0, |at| value[..at].chars().count() as f64 + 1.0))
                };
                Column::Number(values.iter().zip(&needles).map(position).collect())
            }
            ("NUMBER", 2) => {
                let (width, decimals) = f_format(function, &args[1])?;
                let parse = |v: &String| parse_number(v, width, decimals);
                Column::Number(text(0)?.iter().map(parse).collect())
            }
            ("STRING", 2) => {
                let (width, decimals) = f_format(function, &args[1])?;
                let format = |v: Option<f64>| format_number(v, width, decimals);
                Column::String(number(0)?.into_iter().map(format).collect())
            }

            ("DATE.DMY" | "DATE.MDY", 3) => {
                let (day, month) = if function == "DATE.DMY" {
                    (0, 1)
                } else {
                    (1, 0)
                };
                let (day, month, year) = (number(day)?, number(month)?, number(2)?);
                let dates = day.into_iter().zip(month).zip(year);
                Column::Number(dates.map(|((d, m), y)| date(d, m, y)).collect())
            }
            ("XDATE.YEAR", 1) => date_part(|(year, _, _)| year)?,
            ("XDATE.MONTH", 1) => date_part(|(_, month, _)| month)?,
            ("XDATE.MDAY", 1) => date_part(|(_, _, day)| day)?,

            (_, count) => {
                let known = FUNCTIONS.iter().find(|(known, _)| *known == function);
                let unsupported = UNSUPPORTED.iter().any(|u| match u.strip_suffix('.') {
                    Some(_) => upper.starts_with(u),
                    None => *u == upper,
                });
                return Err(Error(match known {
                    Some((_, usage)) => format!(
                        "wrong number of arguments for {function}: expected {usage}, got {count}"
                    ),
                    None if unsupported => format!("function {upper} is not supported yet"),
                    None => format!("unknown function {upper}"),
                }));
            }
        })
    }
}
