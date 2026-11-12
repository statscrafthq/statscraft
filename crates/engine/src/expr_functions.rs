//! The date, time and string functions of expressions, one value at a time. `expr` evaluates the
//! arguments and calls these for each case.
//!
//! Rules from the Universals chapter (date and time functions, string functions, missing values
//! in arguments):
//! - Dates count seconds from 1582-10-14; the first valid date is 1582-10-15. A time interval
//!   counts seconds and may be negative.
//! - DATE.x, YRMODA and the other aggregations take whole numbers in their documented ranges;
//!   a day too large for its month carries into the next month, day 0 is the last day of the
//!   month before. Anything else gives system-missing.
//! - TIME.HMS: the arguments share one sign; minutes and seconds stay below 60 when a larger
//!   unit is not zero; only the last argument that is not zero may have a fraction.
//! - DATEDIFF counts whole units from the second date to the first, truncated toward zero; a
//!   month is the same day of the month (and time of day) one calendar month on, a year twelve
//!   months, a quarter three. DATESUM adds whole years, quarters and months (the value
//!   truncated), keeping the day ("closest", the default, moves it back to the last day of a
//!   shorter month; "rollover" carries the extra days into the next month), and any amount of
//!   the fixed units.
//! - A numeric argument of a string function that is missing or out of range gives the null
//!   string; one of INDEX or RINDEX gives system-missing.
//!
//! Choices where the manual is silent:
//! - XDATE.HOUR, MINUTE, SECOND, TDAY and TIME of a negative time interval are the parts of its
//!   size with its sign, as the time is shown (-1:30:00 has hour -1 and minute -30).
//! - Positions and lengths of SUBSTR, INDEX, RINDEX, LPAD and RPAD count characters like their
//!   CHAR. forms, so they never split a character; LENGTH, STRUNC and MBLEN.BYTE count bytes.
//! - An empty padding string, a REPLACE count or STRUNC length that is not a whole number of
//!   at least 0, or a trimming argument that is not one character give the null string.

use crate::expr::{DAYS_TO_1970, civil_from_days, days_from_civil};

pub(crate) const DAY: f64 = 86400.0;
/// The widest string a variable can hold.
const MAX_WIDTH: f64 = 32767.0;

/// (year, month, day) of a date value; dates before 1582-10-15 are missing.
pub(crate) fn date_parts(value: f64) -> Option<(i64, i64, i64)> {
    let days = (value / DAY).floor();
    (1.0..1e9)
        .contains(&days)
        .then(|| civil_from_days(days as i64 - DAYS_TO_1970))
}

/// The date value of a day: seconds from 1582-10-14. Day 0 to 31 and month 1 to 13 are
/// accepted and carry into the neighbouring month or year.
pub(crate) fn date(day: f64, month: f64, year: f64) -> Option<f64> {
    let whole = |v: f64| (v.fract() == 0.0 && v.abs() < 1e6).then_some(v as i64);
    let (day, month, year) = (whole(day)?, whole(month)?, whole(year)?);
    if !(0..=31).contains(&day) || !(1..=13).contains(&month) || year < 1582 {
        return None;
    }
    let days = days_from_civil(year, month, day) + DAYS_TO_1970;
    (days >= 1).then_some(days as f64 * DAY)
}

/// `value` when it is a whole number from `low` to `high`.
fn whole_in(value: f64, low: f64, high: f64) -> Option<f64> {
    (value.fract() == 0.0 && (low..=high).contains(&value)).then_some(value)
}

/// DATE.MOYR, DATE.QYR, DATE.WKYR and DATE.YRDAY; a two-digit year falls in the SET EPOCH
/// window.
pub(crate) fn date_of(function: &str, a: f64, b: f64, epoch: Option<f64>) -> Option<f64> {
    let year = |y: f64| crate::data_list::full_year(y, epoch);
    let (a, b) = match function {
        "DATE.YRDAY" => (year(a), b),
        _ => (a, year(b)),
    };
    match function {
        "DATE.MOYR" => date(1.0, a, b),
        "DATE.QYR" => date(1.0, whole_in(a, 1.0, 4.0)? * 3.0 - 2.0, b),
        "DATE.WKYR" => Some(date(1.0, 1.0, b)? + (whole_in(a, 1.0, 53.0)? - 1.0) * 7.0 * DAY),
        _ => Some(date(1.0, 1.0, a)? + (whole_in(b, 1.0, 366.0)? - 1.0) * DAY),
    }
}

/// YRMODA: days from 1582-10-14 to the date; a year from 0 to 99 is in the 1900s.
pub(crate) fn yrmoda(year: f64, month: f64, day: f64) -> Option<f64> {
    let year = if (0.0..=99.0).contains(&year) {
        year + 1900.0
    } else {
        year
    };
    if year > 47516.0 {
        return None;
    }
    Some(date(day, month, year)? / DAY)
}

/// TIME.HMS of one, two or three arguments.
pub(crate) fn time_hms(parts: &[f64]) -> Option<f64> {
    let positive = parts.iter().any(|p| *p > 0.0);
    let negative = parts.iter().any(|p| *p < 0.0);
    let last = parts.iter().rposition(|p| *p != 0.0).unwrap_or(0);
    for (i, part) in parts.iter().enumerate() {
        let larger = parts[..i].iter().any(|p| *p != 0.0);
        if (i < last && part.fract() != 0.0) || (larger && part.abs() >= 60.0) {
            return None;
        }
    }
    if positive && negative {
        return None;
    }
    Some(
        parts
            .iter()
            .zip([3600.0, 60.0, 1.0])
            .map(|(p, s)| p * s)
            .sum(),
    )
}

/// XDATE.x of a value; None for a calendar part of a value that is no valid date.
pub(crate) fn xdate(function: &str, value: f64) -> Option<f64> {
    // the parts of a negative time keep its sign; + 0.0 turns -0 into 0
    let part = |unit: f64, modulus: f64| (value / unit).trunc() % modulus + 0.0;
    let calendar = || {
        let (year, month, day) = date_parts(value)?;
        let days = (value / DAY).floor() as i64;
        let day_of_year = days - (days_from_civil(year, 1, 1) + DAYS_TO_1970) + 1;
        Some((year, month, day, days, day_of_year))
    };
    Some(match function {
        "XDATE.DATE" => (value / DAY).floor() * DAY,
        "XDATE.TIME" => value % DAY + 0.0,
        "XDATE.TDAY" => (value / DAY).trunc() + 0.0,
        "XDATE.HOUR" => part(3600.0, 24.0),
        "XDATE.MINUTE" => part(60.0, 60.0),
        "XDATE.SECOND" => value % 60.0 + 0.0,
        "XDATE.YEAR" => calendar()?.0 as f64,
        "XDATE.MONTH" => calendar()?.1 as f64,
        "XDATE.MDAY" => calendar()?.2 as f64,
        // 1582-10-14, day 0, was a Thursday (5)
        "XDATE.WKDAY" => ((calendar()?.3 + 4) % 7 + 1) as f64,
        "XDATE.JDAY" => calendar()?.4 as f64,
        "XDATE.WEEK" => ((calendar()?.4 - 1) / 7 + 1) as f64,
        _ => ((calendar()?.1 - 1) / 3 + 1) as f64,
    })
}

/// A unit of DATEDIFF and DATESUM: its length in seconds, or the number of months.
#[derive(Clone, Copy)]
pub(crate) enum Unit {
    Seconds(f64),
    Months(i64),
}

/// The unit a DATEDIFF or DATESUM argument names, in the plural or singular, any case.
pub(crate) fn unit(name: &str) -> Option<Unit> {
    let name = name.trim().to_lowercase();
    Some(match name.strip_suffix('s').unwrap_or(&name) {
        "year" => Unit::Months(12),
        "quarter" => Unit::Months(3),
        "month" => Unit::Months(1),
        "week" => Unit::Seconds(7.0 * DAY),
        "day" => Unit::Seconds(DAY),
        "hour" => Unit::Seconds(3600.0),
        "minute" => Unit::Seconds(60.0),
        "second" => Unit::Seconds(1.0),
        _ => return None,
    })
}

/// DATEDIFF(later, earlier, unit): whole units from `earlier` to `later`, toward zero.
pub(crate) fn date_diff(later: f64, earlier: f64, unit: Unit) -> Option<f64> {
    let size = match unit {
        Unit::Seconds(size) => return Some(((later - earlier) / size).trunc() + 0.0),
        Unit::Months(size) => size,
    };
    let ((y2, m2, d2), (y1, m1, d1)) = (date_parts(later)?, date_parts(earlier)?);
    let mut months = (y2 - y1) * 12 + (m2 - m1);
    // the day of the month and the time of day decide whether the last month is complete
    let (rest2, rest1) = ((d2, later.rem_euclid(DAY)), (d1, earlier.rem_euclid(DAY)));
    if later >= earlier && rest2 < rest1 {
        months -= 1;
    } else if later < earlier && rest2 > rest1 {
        months += 1;
    }
    Some((months / size) as f64)
}

/// DATESUM(date, value, unit, method).
pub(crate) fn date_sum(date: f64, value: f64, unit: Unit, rollover: bool) -> Option<f64> {
    let size = match unit {
        Unit::Seconds(size) => return Some(date + value * size),
        Unit::Months(size) => size,
    };
    let (year, month, day) = date_parts(date)?;
    let months = (value.trunc() as i64).checked_mul(size)?;
    let total = (month - 1).checked_add(months)?;
    let (year, month) = (year + total.div_euclid(12), total.rem_euclid(12) + 1);
    let last = days_from_civil(year, month + 1, 1) - days_from_civil(year, month, 1);
    let day = if rollover { day } else { day.min(last) };
    let days = days_from_civil(year, month, day) + DAYS_TO_1970;
    (days >= 1).then(|| days as f64 * DAY + date.rem_euclid(DAY))
}

/// A count argument of a string function: a whole number from `low` to the widest string.
pub(crate) fn count(value: Option<f64>, low: f64) -> Option<usize> {
    let valid = |v: &f64| v.fract() == 0.0 && (low..=MAX_WIDTH).contains(v);
    value.filter(valid).map(|v| v as usize)
}

/// LPAD and RPAD: `text` padded to `length` characters with as many whole copies of `pad` as
/// fit; the null string for a length that is missing or out of range, or an empty pad.
pub(crate) fn pad(text: &str, length: Option<f64>, pad: &str, left: bool) -> String {
    let (Some(length), false) = (count(length, 1.0), pad.is_empty()) else {
        return String::new();
    };
    let room = length.saturating_sub(text.chars().count());
    let padding = pad.repeat(room / pad.chars().count());
    if left {
        padding + text
    } else {
        format!("{text}{padding}")
    }
}

/// LTRIM and RTRIM with the character to remove.
pub(crate) fn trim(text: &str, what: &str, left: bool) -> String {
    let mut chars = what.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return String::new();
    };
    if left {
        text.trim_start_matches(c).to_string()
    } else {
        text.trim_end_matches(c).to_string()
    }
}

/// The character position (from 1) of the first or last occurrence of `needle`, or of any of
/// its pieces of `divisor` characters, in `haystack`; 0 when none occurs. Without a divisor
/// an empty needle is found at the start (or after the end). None for a divisor that is
/// missing, not a whole number from 1, or does not divide the needle's length.
pub(crate) fn find(
    haystack: &str,
    needle: &str,
    divisor: Option<Option<f64>>,
    last: bool,
) -> Option<f64> {
    let chars: Vec<char> = needle.chars().collect();
    let pieces: Vec<String> = match divisor {
        None => vec![needle.to_string()],
        Some(divisor) => {
            let size = divisor.filter(|d| d.fract() == 0.0 && *d >= 1.0)? as usize;
            if !chars.len().is_multiple_of(size) {
                return None;
            }
            chars.chunks(size).map(|c| c.iter().collect()).collect()
        }
    };
    let position = |at: usize| haystack[..at].chars().count() as f64 + 1.0;
    let found = pieces.iter().filter_map(|piece| {
        let at = if last {
            haystack.rfind(piece.as_str())
        } else {
            haystack.find(piece.as_str())
        };
        at.map(position)
    });
    let best = if last {
        found.fold(None, |best: Option<f64>, p| {
            Some(best.map_or(p, |b| b.max(p)))
        })
    } else {
        found.fold(None, |best: Option<f64>, p| {
            Some(best.map_or(p, |b| b.min(p)))
        })
    };
    Some(best.unwrap_or(0.0))
}

/// REPLACE: `count` occurrences (all when None) of `from` in `text` replaced by `to`. An empty
/// `from` changes nothing.
pub(crate) fn replace(text: &str, from: &str, to: &str, count: Option<usize>) -> String {
    match count {
        _ if from.is_empty() => text.to_string(),
        Some(n) => text.replacen(from, to, n),
        None => text.replace(from, to),
    }
}

/// STRUNC: at most `bytes` bytes of `text`, without a cut character, then without trailing
/// blanks.
pub(crate) fn strunc(text: &str, bytes: usize) -> String {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].trim_end_matches(' ').to_string()
}

/// CHAR.MBLEN (a character position) and MBLEN.BYTE (a byte position): the bytes of the
/// character there, from 1.
pub(crate) fn mblen(text: &str, position: f64, bytes: bool) -> Option<f64> {
    let index = whole_in(position, 1.0, text.len() as f64)? as usize - 1;
    let c = if bytes {
        let (_, c) = text
            .char_indices()
            .take_while(|(i, _)| *i <= index)
            .last()?;
        c
    } else {
        text.chars().nth(index)?
    };
    Some(c.len_utf8() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: f64, m: f64, d: f64) -> f64 {
        date(d, m, y).unwrap()
    }

    #[test]
    fn months_count_whole_calendar_months_toward_zero() {
        let diff = |a: f64, b: f64, unit| date_diff(a, b, Unit::Months(unit));
        assert_eq!(diff(ymd(2005., 2., 1.), ymd(2004., 1., 1.), 12), Some(1.0));
        assert_eq!(
            diff(ymd(2005., 1., 29.), ymd(2004., 1., 30.), 12),
            Some(0.0)
        );
        assert_eq!(diff(ymd(2004., 2., 29.), ymd(2004., 1., 31.), 1), Some(0.0));
        assert_eq!(
            diff(ymd(2004., 1., 31.), ymd(2004., 3., 31.), 1),
            Some(-2.0)
        );
        assert_eq!(diff(ymd(2004., 1., 1.), ymd(2005., 2., 1.), 1), Some(-13.0));
        // the time of day counts too
        let later = ymd(2005., 2., 1.) + 3600.0;
        assert_eq!(diff(later, ymd(2004., 1., 1.) + 7200.0, 1), Some(12.0));
    }

    #[test]
    fn datesum_keeps_the_day_or_rolls_it_over() {
        let sum = |d, v, rollover| date_sum(d, v, Unit::Months(1), rollover);
        assert_eq!(
            sum(ymd(2004., 1., 31.), 1.0, false),
            Some(ymd(2004., 2., 29.))
        );
        assert_eq!(
            sum(ymd(2004., 1., 31.), 1.0, true),
            Some(ymd(2004., 3., 2.))
        );
        assert_eq!(
            sum(ymd(2004., 3., 31.), -1.0, true),
            Some(ymd(2004., 3., 2.))
        );
        assert_eq!(
            sum(ymd(2004., 3., 31.), -1.9, false),
            Some(ymd(2004., 2., 29.))
        );
        assert_eq!(
            date_sum(ymd(2004., 2., 29.), 1.0, Unit::Months(12), true),
            Some(ymd(2005., 3., 1.))
        );
    }

    #[test]
    fn time_hms_checks_signs_ranges_and_fractions() {
        assert_eq!(time_hms(&[25.5]), Some(91800.0));
        assert_eq!(time_hms(&[0.0, 90.0, 25.5]), Some(5425.5));
        assert_eq!(time_hms(&[25.5, 30.0]), None);
        assert_eq!(time_hms(&[25.0, 90.0]), None);
        assert_eq!(time_hms(&[1.0, 0.0, 70.0]), None);
        assert_eq!(time_hms(&[1.0, 30.5, 0.0]), Some(5430.0));
        assert_eq!(time_hms(&[-1.0, -30.0, -5.0]), Some(-5405.0));
        assert_eq!(time_hms(&[-1.0, 30.0, 5.0]), None);
    }

    #[test]
    fn index_with_a_divisor_finds_any_piece() {
        assert_eq!(find("abcabc", "cb", Some(Some(1.0)), false), Some(2.0));
        assert_eq!(find("abcabc", "xbca", Some(Some(2.0)), false), Some(3.0));
        assert_eq!(find("abcabc", "abc", Some(Some(2.0)), false), None);
        assert_eq!(find("abcabc", "abc", Some(None), false), None);
        assert_eq!(find("abcabc", "ab", Some(Some(1.0)), true), Some(5.0));
        assert_eq!(find("abcabc", "x", None, true), Some(0.0));
        assert_eq!(find("abcabc", "", None, true), Some(7.0));
        assert_eq!(find("äbä", "ä", None, true), Some(3.0));
    }

    #[test]
    fn byte_and_character_widths() {
        assert_eq!(strunc("aä b", 2), "a");
        assert_eq!(strunc("ab  c", 4), "ab");
        assert_eq!(mblen("aäb", 2.0, false), Some(2.0));
        assert_eq!(mblen("aäb", 3.0, true), Some(2.0));
        assert_eq!(mblen("aäb", 4.0, true), Some(1.0));
        assert_eq!(mblen("ab", 3.0, false), None);
        assert_eq!(pad("ab", Some(7.0), "xy", true), "xyxyab");
        assert_eq!(pad("ab", Some(1.0), "*", false), "ab");
        assert_eq!(pad("ab", None, "*", false), "");
        assert_eq!(pad("ab", Some(4.0), "", false), "");
    }
}
