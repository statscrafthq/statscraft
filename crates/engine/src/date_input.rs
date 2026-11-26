//! Reading dates and times from text, for the date and time formats of DATA LIST, GET DATA and
//! ALTER TYPE.
//!
//! Rules, from the table of date and time formats and the rules of its input data:
//! - A date is a string of numbers and words; blanks, dashes, slashes, periods and commas
//!   separate them, and a number and a word may touch (`28OCT1990`, `4Q90`, `43WK90`). The
//!   century of a two-digit year is the one in the 100 years from SET EPOCH.
//! - DATE and EDATE are day, month, year; ADATE month, day, year; SDATE year, month, day. A
//!   month is a number, a Roman numeral or at least three letters of its name. With no
//!   delimiter the text is six digits (two for the year) or eight (four).
//! - JDATE is five digits (yyddd) or seven (yyyyddd), with every leading zero and no delimiter.
//! - QYR is the quarter and the year with a Q between (`4Q90`, `4 Q 1990`, `Q4 1990`, `Q41990`);
//!   it is the first day of the quarter. MOYR is month and year; it is the first of the month.
//!   WKYR is the week, WK, and the year (`43WK90`, `43 1990`, `431990`); week 1 begins on
//!   January 1, week 2 on January 8, and it is the first day of the week.
//! - A time is hours and minutes with seconds after a colon, or hhmm and hhmmss with no colon.
//!   Hours have no upper limit, minutes are at most 59 and seconds below 60, a fraction of a
//!   second follows a period. MTIME is minutes and seconds, minutes without an upper limit. A
//!   sign before the interval makes all of it negative.
//! - A date with a time of day has a blank (or, for YMDHMS, an uppercase T or nothing) between
//!   them; with nothing between, the date and the time have no delimiters either.
//!
//! Choices where the table is silent: a day the month does not have rolls into the next month,
//! as in the DATE functions; a word other than Q and WK where those belong is no date.

use crate::data_list::{full_year, month};
use crate::expr_functions;
use statscraft_syntax::FormatType;

/// A piece of a date text: a run of digits or a run of letters.
enum Piece<'a> {
    Number(&'a str),
    Word(&'a str),
}

fn words<'a>(p: &Piece<'a>) -> &'a str {
    match p {
        Piece::Number(s) | Piece::Word(s) => s,
    }
}

const NOT_A_DATE: &str = "is not a date";

/// The numbers and words of `text`, none for text that holds anything else.
fn pieces(text: &str) -> Option<Vec<Piece<'_>>> {
    // the runs as (start, end, is a number)
    let mut runs: Vec<(usize, usize, bool)> = Vec::new();
    let mut open: Option<(usize, bool)> = None;
    for (i, c) in text.char_indices() {
        if matches!(c, ' ' | '-' | '/' | '.' | ',' | '\t') {
            if let Some((s, d)) = open.take() {
                runs.push((s, i, d));
            }
            continue;
        }
        let digit = c.is_ascii_digit();
        if !digit && !c.is_alphabetic() {
            return None;
        }
        match open {
            Some((_, d)) if d == digit => {}
            Some((s, d)) => {
                runs.push((s, i, d));
                open = Some((i, digit));
            }
            None => open = Some((i, digit)),
        }
    }
    if let Some((s, d)) = open {
        runs.push((s, text.len(), d));
    }
    Some(
        runs.into_iter()
            .map(|(s, e, d)| {
                if d {
                    Piece::Number(&text[s..e])
                } else {
                    Piece::Word(&text[s..e])
                }
            })
            .collect(),
    )
}

fn number(text: &str) -> Option<f64> {
    text.parse::<u32>().ok().map(f64::from)
}

/// A year of two or four digits, its century from `epoch`.
fn year(text: &str, epoch: Option<f64>) -> Option<f64> {
    matches!(text.len(), 2 | 4).then_some(())?;
    Some(full_year(number(text)?, epoch))
}

/// The first day of the month `month` of `year`, in seconds.
fn first_of(year: f64, month: f64) -> Option<f64> {
    expr_functions::date(1.0, month, year)
}

/// A date, by one of the date formats (and the date part of DATETIME and YMDHMS).
pub(crate) fn date(
    text: &str,
    kind: FormatType,
    epoch: Option<f64>,
) -> Result<Option<f64>, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let parts = pieces(text).ok_or(NOT_A_DATE)?;
    let day = |d: &str, m: &str, y: Option<f64>| -> Result<Option<f64>, &'static str> {
        let (Some(d), Some(m), Some(y)) = (number(d), month(m), y) else {
            return Err(NOT_A_DATE);
        };
        if !(1.0..=31.0).contains(&d) || !(1.0..=12.0).contains(&m) {
            return Err("is not a date: the day or the month is out of range");
        }
        expr_functions::date(d, m, y)
            .map(Some)
            .ok_or("is not a date the calendar holds")
    };

    match kind {
        FormatType::Date | FormatType::Adate | FormatType::Edate | FormatType::Sdate => {
            let texts: Vec<&str> = match parts.as_slice() {
                // no delimiter: six or eight digits
                [Piece::Number(all)] if matches!(all.len(), 6 | 8) => {
                    // the year has two digits in six, four in eight
                    let y = all.len() - 4;
                    if kind == FormatType::Sdate {
                        vec![&all[..y], &all[y..y + 2], &all[y + 2..]]
                    } else {
                        vec![&all[..2], &all[2..4], &all[4..]]
                    }
                }
                [a, b, c] => vec![words(a), words(b), words(c)],
                _ => return Err("is not a date: it needs a day, a month and a year"),
            };
            let [a, b, c] = texts[..] else {
                return Err(NOT_A_DATE);
            };
            let year_of = |t: &str| {
                (t.bytes().all(|b| b.is_ascii_digit()) && t.len() <= 4)
                    .then(|| number(t).map(|n| full_year(n, epoch)))
                    .flatten()
            };
            match kind {
                FormatType::Adate => day(b, a, year_of(c)),
                FormatType::Sdate => day(c, b, year_of(a)),
                _ => day(a, b, year_of(c)),
            }
        }
        FormatType::Jdate => {
            let [Piece::Number(all)] = parts.as_slice() else {
                return Err("is not a date: JDATE has no delimiter and keeps its leading zeros");
            };
            let split = match all.len() {
                5 => 2,
                7 => 4,
                _ => return Err("is not a date: JDATE is yyddd or yyyyddd"),
            };
            let (y, d) = (year(&all[..split], epoch), number(&all[split..]));
            let (Some(y), Some(d)) = (y, d) else {
                return Err(NOT_A_DATE);
            };
            let leap = (y % 4.0 == 0.0 && y % 100.0 != 0.0) || y % 400.0 == 0.0;
            if !(1.0..=if leap { 366.0 } else { 365.0 }).contains(&d) {
                return Err("is not a date: the day of the year is out of range");
            }
            Ok(first_of(y, 1.0).map(|s| s + (d - 1.0) * 86_400.0))
        }
        FormatType::Qyr => {
            let is_q = |p: &Piece| matches!(p, Piece::Word(w) if w.eq_ignore_ascii_case("Q"));
            let (q, y) = match parts.as_slice() {
                [Piece::Number(q), w, Piece::Number(y)] if is_q(w) => (*q, *y),
                [w, Piece::Number(q), Piece::Number(y)] if is_q(w) => (*q, *y),
                [w, Piece::Number(qy)] if is_q(w) && matches!(qy.len(), 3 | 5) => {
                    (&qy[..1], &qy[1..])
                }
                _ => return Err("is not a quarter and a year"),
            };
            let (Some(q), Some(y)) = (number(q), year(y, epoch)) else {
                return Err(NOT_A_DATE);
            };
            if !(1.0..=4.0).contains(&q) {
                return Err("is not a date: the quarter is out of range");
            }
            Ok(first_of(y, (q - 1.0) * 3.0 + 1.0))
        }
        FormatType::Moyr => {
            let [m, y] = parts.as_slice() else {
                return Err("is not a month and a year");
            };
            let (Some(m), Some(y)) = (month(words(m)), year(words(y), epoch)) else {
                return Err(NOT_A_DATE);
            };
            if !(1.0..=12.0).contains(&m) {
                return Err("is not a date: the month is out of range");
            }
            Ok(first_of(y, m))
        }
        FormatType::Wkyr => {
            let is_wk = |p: &Piece| matches!(p, Piece::Word(w) if w.eq_ignore_ascii_case("WK"));
            let (w, y) = match parts.as_slice() {
                [Piece::Number(w), k, Piece::Number(y)] if is_wk(k) => (*w, *y),
                [Piece::Number(w), Piece::Number(y)] => (*w, *y),
                // the week and the year with nothing between: two digits of week, then the year
                [Piece::Number(wy)] if matches!(wy.len(), 4 | 6) => (&wy[..2], &wy[2..]),
                _ => return Err("is not a week and a year"),
            };
            let (Some(w), Some(y)) = (number(w), year(y, epoch)) else {
                return Err(NOT_A_DATE);
            };
            if !(1.0..=53.0).contains(&w) {
                return Err("is not a date: the week is out of range");
            }
            Ok(first_of(y, 1.0).map(|s| s + (w - 1.0) * 7.0 * 86_400.0))
        }
        _ => Err(NOT_A_DATE),
    }
}

/// The seconds of `text` as hours, minutes and seconds (`mtime`: minutes and seconds), with a
/// colon between them or none, after an optional sign.
pub(crate) fn clock(text: &str, mtime: bool) -> Result<f64, &'static str> {
    let invalid = if mtime {
        "is not a time in minutes and seconds"
    } else {
        "is not a time"
    };
    let text = text.trim();
    let (sign, body) = match text.chars().next() {
        Some('-') => (-1.0, text[1..].trim_start()),
        Some('+') => (1.0, text[1..].trim_start()),
        _ => (1.0, text),
    };
    if body.is_empty() || body.contains(['-', '+']) {
        return Err(invalid);
    }
    let (whole, fraction) = match body.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (body, None),
    };
    if fraction.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit())) {
        return Err(invalid);
    }
    let digits_only = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    // a bare number of MTIME that is not mmss is minutes, as it always was
    if mtime && !whole.contains(':') && digits_only(whole) && whole.len() != 4 {
        return body
            .parse::<f64>()
            .map(|minutes| sign * minutes * 60.0)
            .map_err(|_| invalid);
    }
    // hh:mm and hhmm have no seconds to hold a fraction
    if !mtime && fraction.is_some() && (whole.matches(':').count() == 1 || whole.len() == 4) {
        return Err(invalid);
    }
    let parts: Vec<&str> = if whole.contains(':') {
        whole.split(':').collect()
    } else if digits_only(whole) {
        // no colon: hhmm, hhmmss, or for MTIME mmss
        match (mtime, whole.len()) {
            (true, 4) => vec![&whole[..2], &whole[2..]],
            (false, 4) => vec![&whole[..2], &whole[2..]],
            (false, 6) => vec![&whole[..2], &whole[2..4], &whole[4..]],
            _ => return Err(invalid),
        }
    } else {
        return Err(invalid);
    };
    let allowed = if mtime {
        parts.len() == 2
    } else {
        (2..=3).contains(&parts.len())
    };
    if !allowed || !parts.iter().all(|p| digits_only(p.trim())) {
        return Err(invalid);
    }
    let value = |i: usize| parts[i].trim().parse::<f64>().unwrap_or(0.0);
    let last = parts.len() - 1;
    // the fraction belongs to the seconds, which are the last part
    let seconds_part =
        value(last) + fraction.map_or(0.0, |f| format!("0.{f}").parse::<f64>().unwrap_or(0.0));
    if seconds_part >= 60.0 {
        return Err(invalid);
    }
    let total = if mtime {
        value(0) * 60.0 + seconds_part
    } else {
        let minutes = value(1);
        if minutes >= 60.0 {
            return Err(invalid);
        }
        let seconds = if parts.len() == 3 { seconds_part } else { 0.0 };
        value(0) * 3600.0 + minutes * 60.0 + seconds
    };
    Ok(sign * total)
}

/// A date with a time of day, DATETIME (the usual orders of the date) or YMDHMS (year, month,
/// day); the time may be left out.
pub(crate) fn datetime(
    text: &str,
    ymdhms: bool,
    epoch: Option<f64>,
) -> Result<Option<f64>, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    // YMDHMS with nothing between the date and the time: digits only
    let digits_only = text
        .split('.')
        .next()
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    if ymdhms && digits_only && matches!(text.split('.').next().map(str::len), Some(12 | 14)) {
        let (date_part, time_part) = text.split_at(8);
        let day = date(date_part, FormatType::Sdate, epoch)?.ok_or(NOT_A_DATE)?;
        return Ok(Some(day + clock(time_part, false)?));
    }
    // the time is the last blank-separated word with a colon; an uppercase T may stand for the
    // blank before it in YMDHMS
    let (date_text, time_text) = match text.rsplit_once(char::is_whitespace) {
        Some((d, t)) if t.contains(':') => (d.trim(), Some(t)),
        _ => match t_separator(text) {
            Some(at) if text[at + 1..].contains(':') => (&text[..at], Some(&text[at + 1..])),
            _ => (text, None),
        },
    };
    let day = if ymdhms {
        date(date_text, FormatType::Sdate, epoch)?
    } else {
        any_date(date_text, epoch)?
    }
    .ok_or(NOT_A_DATE)?;
    let time = match time_text {
        Some(t) => clock(t, false)?,
        None => 0.0,
    };
    Ok(Some(day + time))
}

/// The place of an uppercase T between the date and the time: one with a digit on each side, so
/// that the T of a month name (OCT) is never taken for it.
fn t_separator(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    (1..bytes.len().saturating_sub(1)).find(|&i| {
        bytes[i] == b'T' && bytes[i - 1].is_ascii_digit() && bytes[i + 1].is_ascii_digit()
    })
}

/// A date in any of the usual orders: year first, month first when slashes separate it, day
/// first otherwise.
fn any_date(text: &str, epoch: Option<f64>) -> Result<Option<f64>, &'static str> {
    let year_first = text
        .trim()
        .split(['-', '/', '.'])
        .next()
        .is_some_and(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit()));
    if year_first {
        return date(text, FormatType::Sdate, epoch);
    }
    if text.contains('/') {
        // 17/3/2011 and 3/17/2011: a first number above 12 can only be a day
        let first = text
            .split('/')
            .next()
            .and_then(|p| p.trim().parse::<u32>().ok());
        if first.is_some_and(|f| f > 12) {
            return date(text, FormatType::Date, epoch);
        }
        return date(text, FormatType::Adate, epoch);
    }
    date(text, FormatType::Date, epoch)
}
