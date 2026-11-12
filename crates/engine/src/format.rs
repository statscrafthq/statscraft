//! Display formats: how every output (result tables, LIST, the Data View, Excel export) turns a
//! number into text.
//!
//! Rules from the Command Syntax Reference (Universals: Variable Formats, Date and Time
//! Formats; SET: CCA to CCE; CTABLES: Formats for Summaries):
//! - Dates count seconds from 1582-10-14 00:00:00; TIME, DTIME and MTIME count seconds of a
//!   time interval, which may be negative; WKDAY is 1 (Sunday) to 7, MONTH 1 to 12.
//! - The width chooses the form: DATE11 shows 28-OCT-1990, DATE9 28-OCT-90 (the year in two
//!   digits whatever the century); TIME8 shows hh:mm:ss, TIME5 hh:mm; seconds get the
//!   decimals the format gives when the width has room. What does not fit is cut off, never
//!   rounded: 1:20:59 in TIME5 is 01:20. Hours, and the minutes of MTIME, have no upper limit.
//! - WKDAY and MONTH show the name cut to the width (WKDAY2: SU, MONTH3: JAN).
//! - A custom currency (SET CCA to CCE) has four parts: what goes before a negative number,
//!   before any number, after any number and after a negative number, separated by three commas or three periods; the other character is the
//!   decimal point and the separator groups thousands. A ' makes the next period or comma part
//!   of the text. A negative number shows as negative prefix, prefix, number, suffix, negative
//!   suffix. The default, "-,,,", looks like COMMA.
//!
//! Where the manual is silent: a negative date value, before the start of the scale, and a
//! WKDAY or MONTH outside its range show as asterisks; a fraction of a weekday or month is cut
//! off (3.7 is TUESDAY).

use crate::expr::{DAYS_TO_1970, civil_from_days, days_from_civil, format_number};

/// The SET CCA to CCE default.
pub(crate) const DEFAULT_CURRENCY: &str = "-,,,";

const MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

const WEEKDAYS: [&str; 7] = [
    "SUNDAY",
    "MONDAY",
    "TUESDAY",
    "WEDNESDAY",
    "THURSDAY",
    "FRIDAY",
    "SATURDAY",
];

/// `value` shown in a display format such as "F8.2", "PCT40.1", "COMMA10.0", "DOT8.2",
/// "DOLLAR8.2", "N5", "E10.3", "DATE11", "TIME11.2", "CCA9.2", or one of the CTABLES formats
/// NEGPAREN, NEQUAL, PAREN and PCTPAREN, without leading or trailing spaces. A custom
/// currency format may carry its SET specification after a space ("CCA9.2 (,$,,)"); without
/// one it has the default. With `fit` the width of a number format grows to 40 when the
/// number needs it, and a date or time that fits no form of its width shows in the shortest,
/// so the text never turns into asterisks; without it the result is what the reference
/// program shows in that width. The CTABLES formats show the number as F in the width and add
/// their parentheses, "N=" or percent sign outside it.
pub fn format_value(value: f64, format: &str, fit: bool) -> String {
    let (format, currency) = format.split_once(' ').unwrap_or((format, DEFAULT_CURRENCY));
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    let (kind, size) = format.split_at(letters.unwrap_or(format.len()));
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    let mut width: usize = width.parse().unwrap_or(8);
    let given = width;
    let decimals: usize = decimals.parse().unwrap_or(0);
    if fit {
        width = width.max(40);
    }
    let kind = kind.to_ascii_uppercase();
    match kind.as_str() {
        "PCT" => {
            let number = format_number(Some(value), width.saturating_sub(1), decimals);
            if number.contains('*') || width < 2 {
                "*".repeat(width)
            } else {
                format!("{}%", number.trim_start())
            }
        }
        "COMMA" | "DOLLAR" | "DOT" => {
            let plain = format_number(Some(value), 40, decimals);
            let plain = plain.trim_start();
            let (sign, digits) = match plain.strip_prefix('-') {
                Some(rest) => ("-", rest),
                None => ("", plain),
            };
            let dollar = if kind == "DOLLAR" { "$" } else { "" };
            let mut text = format!("{sign}{dollar}{}", grouped(digits, ','));
            if text.len() > width {
                // shortcut: a number too wide for its grouping falls back to F in the same
                // width; the reference program drops separators one at a time first, copy that
                // when someone needs narrow COMMA columns
                text = format_number(Some(value), width, decimals)
                    .trim_start()
                    .to_string();
            }
            if kind == "DOT" {
                text = swap_marks(&text);
            }
            text
        }
        "CCA" | "CCB" | "CCC" | "CCD" | "CCE" => {
            let currency = Currency::parse(currency).unwrap_or_else(|_| {
                Currency::parse(DEFAULT_CURRENCY).expect("the default specification")
            });
            currency.show(value, width, decimals)
        }
        // leading zeros fill the width the format gives; there is no sign
        "N" => {
            let number = format_number(Some(value), 40, decimals);
            let number = number.trim_start();
            if value < 0.0 || (!fit && number.len() > given) {
                "*".repeat(given)
            } else {
                format!("{number:0>given$}")
            }
        }
        "E" => {
            for decimals in (0..=decimals).rev() {
                let text = format!("{value:.decimals$e}");
                let (mantissa, exponent) = text.split_once('e').expect("exponent format");
                let exponent: i32 = exponent.parse().expect("exponent format");
                let text = format!("{mantissa}E{exponent:+04}");
                if text.len() <= width {
                    return text;
                }
            }
            "*".repeat(width)
        }
        "NEGPAREN" | "NEQUAL" | "PAREN" | "PCTPAREN" => {
            let number = format_number(Some(value), width, decimals);
            let number = number.trim_start();
            match kind.as_str() {
                _ if number.starts_with('*') => number.to_string(),
                "NEGPAREN" => match number.strip_prefix('-') {
                    Some(magnitude) => format!("({magnitude})"),
                    None => number.to_string(),
                },
                "NEQUAL" => format!("N={number}"),
                "PAREN" => format!("({number})"),
                _ => format!("({number}%)"),
            }
        }
        "WKDAY" | "MONTH" => {
            let names: &[&str] = if kind == "WKDAY" { &WEEKDAYS } else { &MONTHS };
            let index = value.floor();
            if index >= 1.0 && index <= names.len() as f64 {
                names[index as usize - 1].chars().take(given).collect()
            } else {
                "*".repeat(given)
            }
        }
        _ => match date_time_forms(value, &kind, decimals) {
            Some(Some(forms)) => match forms.iter().find(|f| f.len() <= given) {
                Some(form) => form.clone(),
                None if fit => forms.last().cloned().unwrap_or_default(),
                None => "*".repeat(given),
            },
            Some(None) => "*".repeat(given),
            None => format_number(Some(value), width, decimals)
                .trim_start()
                .to_string(),
        },
    }
}

/// The digits of a number (no sign, "." as decimal point) with `separator` between groups of
/// three digits before the point.
fn grouped(digits: &str, separator: char) -> String {
    let (whole, fraction) = digits.split_at(digits.find('.').unwrap_or(digits.len()));
    let mut text = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            text.push(separator);
        }
        text.push(digit);
    }
    text + fraction
}

/// The text with the roles of period and comma swapped.
fn swap_marks(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            ',' => '.',
            '.' => ',',
            c => c,
        })
        .collect()
}

/// The forms of `value` in a date or time format `kind`, from the most to the least complete,
/// with seconds to `decimals` places, cut off rather than rounded. None when `kind` is no date
/// or time format; Some(None) when the value cannot be shown in it.
fn date_time_forms(value: f64, kind: &str, decimals: usize) -> Option<Option<Vec<String>>> {
    let duration = matches!(kind, "TIME" | "DTIME" | "MTIME");
    let dated = matches!(
        kind,
        "DATE"
            | "ADATE"
            | "EDATE"
            | "JDATE"
            | "SDATE"
            | "QYR"
            | "MOYR"
            | "WKYR"
            | "DATETIME"
            | "YMDHMS"
    );
    if !duration && !dated {
        return None;
    }
    if !value.is_finite() || value.abs() >= 1e15 || (dated && value < 0.0) {
        return Some(None);
    }
    let decimals = decimals.min(16);
    let scale = 10i128.pow(decimals as u32);
    // the stored seconds are binary fractions: 0.3 can be 0.29999..., which must not lose a digit
    let units = (value.abs() * scale as f64 * (1.0 + 4.0 * f64::EPSILON)).floor() as i128;
    let (seconds, fraction) = (units / scale, units % scale);
    let sign = if value < 0.0 && units > 0 { "-" } else { "" };
    let (minute, second) = (seconds / 60 % 60, seconds % 60);
    let digits = format!("{fraction:0decimals$}");
    // hours, minutes and seconds with as many decimals as there are places, down to hh:mm
    let clock = |hours: i128| -> Vec<String> {
        let mut forms: Vec<String> = (1..=decimals)
            .rev()
            .map(|places| format!("{hours:02}:{minute:02}:{second:02}.{}", &digits[..places]))
            .collect();
        forms.push(format!("{hours:02}:{minute:02}:{second:02}"));
        forms.push(format!("{hours:02}:{minute:02}"));
        forms
    };
    let forms = match kind {
        "TIME" => clock(seconds / 3600)
            .into_iter()
            .map(|f| format!("{sign}{f}"))
            .collect(),
        "DTIME" => clock(seconds / 3600 % 24)
            .into_iter()
            .map(|f| format!("{sign}{} {f}", seconds / 86_400))
            .collect(),
        "MTIME" => {
            let minutes = seconds / 60;
            let mut forms: Vec<String> = (1..=decimals)
                .rev()
                .map(|places| format!("{sign}{minutes:02}:{second:02}.{}", &digits[..places]))
                .collect();
            forms.push(format!("{sign}{minutes:02}:{second:02}"));
            forms
        }
        _ => {
            let days = (seconds / 86_400) as i64 - DAYS_TO_1970;
            let (year, month, day) = civil_from_days(days);
            let short = year.rem_euclid(100);
            let name = &MONTHS[month as usize - 1][..3];
            let day_of_year = days - days_from_civil(year, 1, 1) + 1;
            let date = |full: String, two: String| vec![full, two];
            match kind {
                "DATE" => date(
                    format!("{day:02}-{name}-{year:04}"),
                    format!("{day:02}-{name}-{short:02}"),
                ),
                "ADATE" => date(
                    format!("{month:02}/{day:02}/{year:04}"),
                    format!("{month:02}/{day:02}/{short:02}"),
                ),
                "EDATE" => date(
                    format!("{day:02}.{month:02}.{year:04}"),
                    format!("{day:02}.{month:02}.{short:02}"),
                ),
                "SDATE" => date(
                    format!("{year:04}/{month:02}/{day:02}"),
                    format!("{short:02}/{month:02}/{day:02}"),
                ),
                "JDATE" => date(
                    format!("{year:04}{day_of_year:03}"),
                    format!("{short:02}{day_of_year:03}"),
                ),
                "QYR" => {
                    let quarter = (month - 1) / 3 + 1;
                    date(
                        format!("{quarter} Q {year:04}"),
                        format!("{quarter} Q {short:02}"),
                    )
                }
                "MOYR" => date(format!("{name} {year:04}"), format!("{name} {short:02}")),
                // week 1 starts on January 1, week 2 on January 8
                "WKYR" => {
                    let week = (day_of_year - 1) / 7 + 1;
                    date(
                        format!("{week} WK {year:04}"),
                        format!("{week} WK {short:02}"),
                    )
                }
                _ => {
                    let date = match kind {
                        "DATETIME" => format!("{day:02}-{name}-{year:04}"),
                        _ => format!("{year:04}-{month:02}-{day:02}"),
                    };
                    clock(seconds / 3600 % 24)
                        .into_iter()
                        .map(|time| format!("{date} {time}"))
                        .collect()
                }
            }
        }
    };
    Some(Some(forms))
}

/// A custom currency format as SET CCA to CCE defines it.
#[derive(Debug, PartialEq)]
pub(crate) struct Currency {
    negative_prefix: String,
    prefix: String,
    suffix: String,
    negative_suffix: String,
    /// The separator of thousands; the decimal point is the other of period and comma.
    grouping: char,
}

impl Currency {
    /// Reads a specification such as "-,$,," or "-/-.Dfl ..-".
    pub(crate) fn parse(spec: &str) -> Result<Currency, String> {
        // each character, and whether a ' made it plain text
        let mut chars: Vec<(char, bool)> = Vec::new();
        let mut rest = spec.chars().peekable();
        while let Some(c) = rest.next() {
            match rest.peek() {
                // PSPP keeps the ' as text; the manual makes it an escape character
                Some(&mark @ (',' | '.')) if c == '\'' => {
                    chars.push((mark, true));
                    rest.next();
                }
                _ => chars.push((c, false)),
            }
        }
        if chars.len() > 16 {
            return Err(format!(
                "a custom currency specification has at most 16 characters, got {spec:?}"
            ));
        }
        let count = |mark: char| chars.iter().filter(|&&(c, e)| c == mark && !e).count();
        let grouping = match (count(','), count('.')) {
            (3, _) => ',',
            (_, 3) => '.',
            _ => {
                return Err(format!(
                    "a custom currency specification needs three commas or three periods, \
                     such as '-,$,,', got {spec:?}"
                ));
            }
        };
        let mut parts = vec![String::new()];
        for (c, escaped) in chars {
            if c == grouping && !escaped {
                parts.push(String::new());
            } else {
                parts.last_mut().expect("one part").push(c);
            }
        }
        let [negative_prefix, prefix, suffix, negative_suffix] =
            <[String; 4]>::try_from(parts).expect("three separators make four parts");
        Ok(Currency {
            negative_prefix,
            prefix,
            suffix,
            negative_suffix,
            grouping,
        })
    }

    /// `value` in a width of `width` with `decimals` places: grouped when it fits, otherwise
    /// without separators and with fewer decimals, otherwise asterisks.
    fn show(&self, value: f64, width: usize, decimals: usize) -> String {
        let plain = format_number(Some(value.abs()), 40, decimals);
        let plain = plain.trim_start();
        let negative = value < 0.0 && plain.bytes().any(|d| (b'1'..=b'9').contains(&d));
        let (before, after) = if negative {
            (
                format!("{}{}", self.negative_prefix, self.prefix),
                format!("{}{}", self.suffix, self.negative_suffix),
            )
        } else {
            (self.prefix.clone(), self.suffix.clone())
        };
        let marks = |digits: &str| match self.grouping {
            '.' => swap_marks(digits),
            _ => digits.to_string(),
        };
        let text = format!("{before}{}{after}", marks(&grouped(plain, ',')));
        if text.chars().count() <= width {
            return text;
        }
        let room = width.saturating_sub(before.chars().count() + after.chars().count());
        let number = format_number(Some(value.abs()), room, decimals);
        if room == 0 || number.contains('*') {
            return "*".repeat(width);
        }
        format!("{before}{}{after}", marks(number.trim_start()))
    }
}

/// What the number of a display format stands for.
#[derive(Clone, Copy, PartialEq)]
enum Scale {
    /// A moment: seconds from 1582-10-14 (DATE, ADATE, DATETIME and the like).
    Moment,
    /// A length of time in seconds: TIME, DTIME, MTIME.
    Duration,
    /// A weekday or month number: WKDAY, MONTH.
    Index,
    /// A custom currency, CCA to CCE.
    Currency,
    /// Any other number.
    Plain,
}

fn scale(format: &str) -> Scale {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    match format[..letters.unwrap_or(format.len())]
        .to_ascii_uppercase()
        .as_str()
    {
        "DATE" | "ADATE" | "EDATE" | "JDATE" | "SDATE" | "QYR" | "MOYR" | "WKYR" | "DATETIME"
        | "YMDHMS" => Scale::Moment,
        "TIME" | "DTIME" | "MTIME" => Scale::Duration,
        "WKDAY" | "MONTH" => Scale::Index,
        "CCA" | "CCB" | "CCC" | "CCD" | "CCE" => Scale::Currency,
        _ => Scale::Plain,
    }
}

/// How a statistic relates to the values of its variable, which decides its format.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Stat {
    /// A point on the variable's scale: a value, mean, median, mode, minimum, maximum,
    /// percentile, confidence limit.
    Value,
    /// A distance on it: standard deviation, standard error, range.
    Spread,
    /// A total of values.
    Sum,
    /// Anything else: variance (squared units), a count of missing values.
    Other,
}

/// The display format of a statistic of a variable whose print format is `format`, as CTABLES
/// gives it by default (the manual's "General": the variable's print format), except where
/// that format cannot show the statistic: a spread of moments is a length of time, shown as
/// DTIME (the general rule would show it as a date); a sum or variance of moments, any
/// statistic of weekday or month numbers but a value, and a variance of durations are plain
/// numbers (""), as no date or time shows them.
pub(crate) fn statistic_format(format: &str, stat: Stat) -> String {
    match (scale(format), stat) {
        (_, Stat::Value) => format.to_string(),
        (Scale::Moment, Stat::Spread) => "DTIME40".to_string(),
        (Scale::Moment | Scale::Index, _) | (Scale::Duration, Stat::Other) => String::new(),
        _ => format.to_string(),
    }
}

/// The format the procedures other than CTABLES give a statistic of a variable: as
/// `statistic_format` for dates, times and custom currencies, whose numbers say little
/// without it; other variables keep plain numbers with the decimals the procedure chooses.
pub(crate) fn procedure_format(format: &str, stat: Stat) -> String {
    match scale(format) {
        Scale::Plain => String::new(),
        _ => statistic_format(format, stat),
    }
}

/// The formats of a table in the shape of `rows`, `of(row, column)` for each cell; empty when no
/// cell has one, as `Table::formats` wants.
pub(crate) fn formats_of(
    rows: &[Vec<crate::Cell>],
    of: impl Fn(usize, usize) -> String,
) -> Vec<Vec<String>> {
    let formats: Vec<Vec<String>> = (rows.iter().enumerate())
        .map(|(r, row)| (0..row.len()).map(|c| of(r, c)).collect())
        .collect();
    if formats.iter().flatten().all(String::is_empty) {
        Vec::new()
    } else {
        formats
    }
}

/// A category value as a label: in its date, time or custom currency format, else as a plain
/// number. `format` is the variable's format as `Currencies::resolve` gives it.
pub(crate) fn category_text(value: f64, format: &str) -> String {
    match scale(format) {
        Scale::Plain => value.to_string(),
        _ => format_value(value, format, true),
    }
}

/// The SET CCA to CCE specifications of a session, which every output that shows a custom
/// currency needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Currencies(pub(crate) [String; 5]);

impl Default for Currencies {
    fn default() -> Self {
        Self(std::array::from_fn(|_| DEFAULT_CURRENCY.to_string()))
    }
}

impl Currencies {
    /// `format` as `format_value` takes it: a custom currency format (CCA to CCE) gets its
    /// specification after a space when it is not the default; other formats stay as they are.
    pub fn resolve(&self, format: &str) -> String {
        let kind = format.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let letter = ["CCA", "CCB", "CCC", "CCD", "CCE"]
            .iter()
            .position(|cc| kind.eq_ignore_ascii_case(cc));
        match letter.map(|i| &self.0[i]) {
            Some(spec) if spec != DEFAULT_CURRENCY => format!("{format} {spec}"),
            _ => format.to_string(),
        }
    }
}

/// The negative prefix, prefix, suffix and negative suffix of a custom currency format as
/// `Currencies::resolve` gives it ("CCA8.2 -,$,,"), and whether a period groups thousands
/// (the decimal point is then a comma). None for other formats or a broken specification.
pub fn currency_parts(format: &str) -> Option<([String; 4], bool)> {
    if scale(format) != Scale::Currency {
        return None;
    }
    let spec = format
        .split_once(' ')
        .map_or(DEFAULT_CURRENCY, |(_, spec)| spec);
    let c = Currency::parse(spec).ok()?;
    let parts = [c.negative_prefix, c.prefix, c.suffix, c.negative_suffix];
    Some((parts, c.grouping == '.'))
}

/// Whether a format turns its number into a date, time or currency text, which a spreadsheet
/// cannot show with a number format of its own.
pub fn is_date_time_or_currency(format: &str) -> bool {
    scale(format) != Scale::Plain
}

#[cfg(test)]
mod tests {
    use super::{Currency, Stat, format_value, procedure_format, statistic_format};

    #[test]
    fn formats_like_the_reference_program() {
        assert_eq!(format_value(67.647, "PCT40.1", false), "67.6%");
        assert_eq!(format_value(0.0, "PCT40.1", false), ".0%");
        assert_eq!(format_value(5440.0, "F3.0", false), "***");
        assert_eq!(format_value(5440.0, "F3.0", true), "5440");
        assert_eq!(format_value(175.0444, "F8.2", false), "175.04");
        assert_eq!(format_value(1234567.5, "COMMA12.1", false), "1,234,567.5");
        assert_eq!(format_value(-1234.0, "DOLLAR9.0", false), "-$1,234");
        assert_eq!(format_value(0.5, "F40.0", false), "1");
        assert_eq!(
            format_value(207.045_000_000_000_02, "DOLLAR10.2", false),
            "$207.05"
        );
    }

    #[test]
    fn dot_n_and_e_formats() {
        assert_eq!(format_value(16978.75, "DOT12.1", false), "16.978,8");
        assert_eq!(format_value(-1234567.5, "DOT12.1", true), "-1.234.567,5");
        assert_eq!(format_value(175.04, "DOT10.2", false), "175,04");
        assert_eq!(format_value(97.0, "N5", false), "00097");
        assert_eq!(format_value(97.0, "N5", true), "00097");
        assert_eq!(format_value(123456.0, "N5", false), "*****");
        assert_eq!(format_value(123456.0, "N5", true), "123456");
        assert_eq!(format_value(-3.0, "N5", true), "*****");
        assert_eq!(format_value(175.0444, "E10.3", false), "1.750E+002");
        assert_eq!(format_value(-0.00125, "E10.2", true), "-1.25E-003");
        assert_eq!(format_value(175.0444, "E8.3", false), "1.8E+002");
    }

    #[test]
    fn ctables_formats_add_their_marks_outside_the_width() {
        // the manual's examples
        assert_eq!(format_value(-1234.567, "NEGPAREN9.2", false), "(1234.57)");
        assert_eq!(format_value(1234.567, "NEQUAL9.2", false), "N=1234.57");
        assert_eq!(format_value(1234.567, "PAREN8.2", false), "(1234.57)");
        assert_eq!(format_value(1234.567, "PCTPAREN10.2", false), "(1234.57%)");
        assert_eq!(format_value(1.31, "NEGPAREN9.2", false), "1.31");
        assert_eq!(format_value(-0.001, "NEGPAREN9.2", false), ".00");
        assert_eq!(format_value(-8.0, "PAREN9.1", false), "(-8.0)");
        assert_eq!(format_value(-32.0, "PCTPAREN9.1", true), "(-32.0%)");
        assert_eq!(format_value(11.5, "PCTPAREN5.1", false), "(11.5%)");
        assert_eq!(format_value(175.0, "PAREN4.0", false), "(175)");
        assert_eq!(format_value(54321.0, "PAREN3.0", false), "***");
    }

    /// Every form of `value` in each format, in pairs.
    fn check(value: f64, cases: &[(&str, &str)]) {
        for (format, expected) in cases {
            assert_eq!(
                format_value(value, format, false),
                *expected,
                "{value} {format}"
            );
        }
    }

    // 1995-02-24 10:23:46.75; PSPP shows the same text for all of these
    const MOMENT: f64 = 13_013_000_626.75;

    #[test]
    fn date_formats_take_their_form_from_the_width() {
        check(
            MOMENT,
            &[
                ("DATE9", "24-FEB-95"),
                ("DATE11", "24-FEB-1995"),
                ("ADATE8", "02/24/95"),
                ("ADATE10", "02/24/1995"),
                ("EDATE8", "24.02.95"),
                ("EDATE10", "24.02.1995"),
                ("JDATE5", "95055"),
                ("JDATE7", "1995055"),
                ("SDATE8", "95/02/24"),
                ("SDATE10", "1995/02/24"),
                ("QYR6", "1 Q 95"),
                ("QYR8", "1 Q 1995"),
                ("MOYR6", "FEB 95"),
                ("MOYR8", "FEB 1995"),
                ("WKYR8", "8 WK 95"),
                ("WKYR10", "8 WK 1995"),
                ("DATETIME17", "24-FEB-1995 10:23"),
                ("DATETIME20", "24-FEB-1995 10:23:46"),
                ("DATETIME23.2", "24-FEB-1995 10:23:46.75"),
                ("YMDHMS16", "1995-02-24 10:23"),
                ("YMDHMS19", "1995-02-24 10:23:46"),
                ("YMDHMS22.2", "1995-02-24 10:23:46.75"),
                ("date11", "24-FEB-1995"),
            ],
        );
        // 1935-10-14, Monday of week 41
        check(
            11_139_552_000.0,
            &[
                ("DATE11", "14-OCT-1935"),
                ("JDATE7", "1935287"),
                ("QYR8", "4 Q 1935"),
                ("WKYR10", "41 WK 1935"),
                // PSPP gives asterisks outside its century window; the manual lets the width decide
                ("DATE9", "14-OCT-35"),
            ],
        );
        check(86_400.0, &[("DATE11", "15-OCT-1582")]);
        check(1.0, &[("DATETIME20", "14-OCT-1582 00:00:01")]);
        // before the start of the scale
        check(
            -86_400.0,
            &[("DATE11", "***********"), ("ADATE8", "********")],
        );
        // a width too small for any form
        assert_eq!(format_value(MOMENT, "DATE7", false), "*******");
        assert_eq!(format_value(MOMENT, "DATE7", true), "24-FEB-95");
    }

    #[test]
    fn times_cut_off_what_the_width_cannot_show() {
        check(
            3754.75,
            &[
                ("TIME5", "01:02"),
                ("TIME8", "01:02:34"),
                ("TIME11.2", "01:02:34.75"),
                ("TIME40", "01:02:34"),
                ("MTIME5", "62:34"),
                ("MTIME8.2", "62:34.75"),
                ("DTIME9", "0 01:02"),
                ("DTIME12", "0 01:02:34"),
                ("DTIME15.2", "0 01:02:34.75"),
            ],
        );
        check(
            -3754.75,
            &[
                ("TIME5", "*****"),
                ("TIME8", "-01:02"),
                ("TIME40", "-01:02:34"),
                ("MTIME5", "*****"),
                ("DTIME9", "-0 01:02"),
                ("DTIME12", "-0 01:02:34"),
                ("DTIME15.2", "-0 01:02:34.75"),
                // PSPP rounds the last decimal (34.8); the manual cuts off
                ("TIME11.2", "-01:02:34.7"),
                ("MTIME8.2", "-62:34.7"),
            ],
        );
        check(
            400_000.5,
            &[
                ("TIME5", "*****"),
                ("TIME8", "111:06"),
                ("TIME11.2", "111:06:40.5"),
                ("TIME40", "111:06:40"),
                ("MTIME8.2", "6666:40"),
                ("DTIME9", "4 15:06"),
                ("DTIME12", "4 15:06:40"),
                ("DTIME15.2", "4 15:06:40.50"),
            ],
        );
        check(
            1_755_418.0,
            &[
                ("TIME8", "487:36"),
                ("TIME11.2", "487:36:58.0"),
                ("MTIME8.2", "29256:58"),
                ("DTIME9", "20 07:36"),
                ("DTIME15.2", "20 07:36:58.00"),
            ],
        );
        // PSPP rounds up to 00:00:60.00 and 23:59:60.00; the manual cuts off
        check(
            59.999,
            &[("TIME8", "00:00:59"), ("TIME11.2", "00:00:59.99")],
        );
        check(
            13_013_049_599.999,
            &[("DATETIME23.2", "24-FEB-1995 23:59:59.99")],
        );
        // a binary fraction just below the decimal one keeps its digit
        check(0.3, &[("TIME10.1", "00:00:00.3")]);
        assert_eq!(format_value(400_000.5, "TIME5", true), "111:06");
    }

    #[test]
    fn weekday_and_month_names() {
        check(
            5.0,
            &[("WKDAY2", "TH"), ("WKDAY9", "THURSDAY"), ("MONTH3", "MAY")],
        );
        check(
            3.7,
            &[("WKDAY2", "TU"), ("WKDAY9", "TUESDAY"), ("MONTH9", "MARCH")],
        );
        check(13.0, &[("MONTH3", "***"), ("WKDAY2", "**")]);
        check(0.0, &[("WKDAY2", "**")]);
    }

    #[test]
    fn custom_currencies_of_the_manual() {
        // the manual's examples; PSPP shows the same text
        let cca = "CCA9.0 (,,,-)";
        assert_eq!(format_value(23456.0, cca, false), "23,456");
        assert_eq!(format_value(-19423.0, cca, false), "(19,423-)");
        assert_eq!(format_value(-13.7, cca, false), "(14-)");
        assert_eq!(format_value(-0.004, cca, false), "0");
        let ccb = "CCB6.1 ,,%,";
        assert_eq!(format_value(13.7, ccb, false), "13.7%");
        assert_eq!(format_value(-13.7, ccb, false), "13.7%");
        assert_eq!(format_value(23456.0, ccb, false), "23456%");
        let ccc = "CCC8.0 (,$,,)";
        assert_eq!(format_value(352.0, ccc, false), "$352");
        assert_eq!(format_value(-189.0, ccc, false), "($189)");
        assert_eq!(format_value(23456.0, ccc, false), "$23,456");
        assert_eq!(format_value(-19423.0, ccc, false), "($19423)");
        let ccd = "CCD14.2 -/-.Dfl ..-";
        assert_eq!(format_value(37419.0, ccd, false), "Dfl 37.419,00");
        assert_eq!(format_value(-135.19, ccd, false), "-/-Dfl 135,19-");
        assert_eq!(format_value(0.5, ccd, false), "Dfl ,50");
        assert_eq!(format_value(-19423.0, ccd, false), "-/-Dfl 19423-");
        assert_eq!(
            format_value(37419.0, "CCD10.2 -/-.Dfl ..-", false),
            "Dfl 37419"
        );
        // PSPP keeps the ' in the text; the manual makes it an escape
        let cce = "CCE12.2 x'.y,z,w,";
        assert_eq!(format_value(23456.0, cce, false), "z23,456.00w");
        assert_eq!(format_value(-19423.0, cce, false), "x.yz19423.0w");
        // the default -,,, looks like COMMA
        assert_eq!(format_value(-1234.5, "CCA10.2", false), "-1,234.50");
        assert_eq!(format_value(-1234.5, "CCA8.2", false), "-1234.50");
        assert_eq!(format_value(-1234.5, "CCA8.2", true), "-1,234.50");
    }

    #[test]
    fn custom_currency_specifications() {
        let parsed = Currency::parse("-/-.Dfl ..-").unwrap();
        assert_eq!(
            (
                parsed.negative_prefix.as_str(),
                parsed.prefix.as_str(),
                parsed.suffix.as_str(),
                parsed.negative_suffix.as_str(),
                parsed.grouping
            ),
            ("-/-", "Dfl ", "", "-", '.')
        );
        assert!(Currency::parse("-,$,").is_err());
        assert!(Currency::parse("$").is_err());
        assert!(Currency::parse("-,12345678901234567,,").is_err());
        assert_eq!(Currency::parse("a'.b,,,").unwrap().negative_prefix, "a.b");
    }

    #[test]
    fn statistics_keep_the_format_they_can_show() {
        let cases = [
            ("DATE11", Stat::Value, "DATE11"),
            ("DATE11", Stat::Spread, "DTIME40"),
            ("DATE11", Stat::Sum, ""),
            ("DATE11", Stat::Other, ""),
            ("TIME8", Stat::Spread, "TIME8"),
            ("TIME8", Stat::Sum, "TIME8"),
            ("TIME8", Stat::Other, ""),
            ("WKDAY9", Stat::Value, "WKDAY9"),
            ("WKDAY9", Stat::Spread, ""),
            ("CCA9.2", Stat::Other, "CCA9.2"),
            ("DOLLAR8.2", Stat::Other, "DOLLAR8.2"),
            ("F8.2", Stat::Spread, "F8.2"),
        ];
        for (format, stat, expected) in cases {
            assert_eq!(
                statistic_format(format, stat),
                expected,
                "{format} {stat:?}"
            );
        }
        assert_eq!(procedure_format("F8.2", Stat::Value), "");
        assert_eq!(procedure_format("COMMA8.2", Stat::Value), "");
        assert_eq!(procedure_format("CCA9.2", Stat::Spread), "CCA9.2");
        assert_eq!(procedure_format("ADATE10", Stat::Value), "ADATE10");
        // a standard deviation of dates of 455 days and some hours
        assert_eq!(format_value(39_363_806.0, "DTIME40", false), "455 14:23:26");
    }
}
