//! SAVE TRANSLATE /TYPE=SAS /VERSION=X: the active dataset as a SAS transport file (the
//! version 5 layout that SAS's XPORT engine and other software read).
//!
//! The file is records of 80 bytes: library and member headers, one 140-byte descriptor per
//! variable, then the observations, each variable after the other (a number is 8 bytes in
//! hexadecimal floating point, text is blank padded), the whole padded with blanks to a full record.
//!
//! The manual gives the rules in words. Choices where it is silent:
//! - Names have at most 8 bytes in this layout. A name is made of letters, digits and _ (anything
//!   else becomes _, a name that is not ASCII becomes V and its position, one that does not
//!   start with a letter or _ gets a V in front), cut to 8 bytes and made unique with a number.
//!   Case is kept. The data set is named after the file (capitals, the same rules, DATA when
//!   nothing is left).
//! - A label is the variable label, else the variable name, cut to 40 bytes.
//! - A date (and QYR, MOYR, WKYR) is SAS days since 1960-01-01 with the format DATE; a date with
//!   a time of day is seconds since then with DATETIME; a time is seconds with TIME; every other
//!   number has no format name and width 12, as the manual's table gives.
//! - Every missing value is the one SAS missing value, `.`; a user-missing value too. Text is
//!   Windows Latin-1, with ? for a character it cannot hold (a warning); a string of more than
//!   200 bytes, the most this layout holds for text, is cut (a warning).
//! - Value labels are not part of the file.

use crate::dbf::latin1;
use crate::expr::civil_from_days;
use crate::save_translate::kind;
use crate::stata::SECONDS_TO_1960;
use statscraft_data::{Dataset, Values, cut_to};

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The names the variables get: 8 bytes at most, unique.
pub(crate) fn xport_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    for (position, name) in names.into_iter().enumerate() {
        let mut clean: String = match name.is_ascii() {
            true => name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect(),
            false => format!("V{}", position + 1),
        };
        if !clean.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            clean.insert(0, 'V');
        }
        let mut short = cut_to(&clean, 8).to_string();
        let mut n = 2;
        while taken.iter().any(|t| t.eq_ignore_ascii_case(&short)) {
            let suffix = n.to_string();
            short = format!("{}{suffix}", cut_to(&clean, 8 - suffix.len()));
            n += 1;
        }
        taken.push(short);
    }
    taken
}

/// The name of the data set: the file's, in capitals.
pub(crate) fn member_name(file: &str) -> String {
    let stem = std::path::Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let clean: String = stem
        .chars()
        .filter(char::is_ascii)
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_uppercase(),
            false => '_',
        })
        .collect();
    let clean = match clean.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        true => clean,
        false if clean.is_empty() => String::new(),
        false => format!("V{clean}"),
    };
    match cut_to(&clean, 8) {
        "" => "DATA".to_string(),
        name => name.to_string(),
    }
}

/// A number in hexadecimal floating point (8 bytes, big endian); None when it is too large for it.
fn hex_float(x: f64) -> Option<[u8; 8]> {
    if x == 0.0 {
        return Some([0; 8]);
    }
    let sign = u8::from(x < 0.0) << 7;
    let (mut fraction, mut exponent) = (x.abs(), 0i32);
    while fraction >= 1.0 {
        fraction /= 16.0;
        exponent += 1;
    }
    while fraction < 1.0 / 16.0 {
        fraction *= 16.0;
        exponent -= 1;
    }
    let mut mantissa = (fraction * (1u64 << 56) as f64).round() as u64;
    if mantissa >= 1 << 56 {
        mantissa >>= 4;
        exponent += 1;
    }
    match exponent + 64 {
        e if e > 127 => None,
        e if e < 0 => Some([0; 8]),
        e => {
            let mut bytes = mantissa.to_be_bytes();
            bytes[0] = sign | e as u8;
            Some(bytes)
        }
    }
}

/// The SAS missing value `.`.
const MISSING: [u8; 8] = [0x2E, 0, 0, 0, 0, 0, 0, 0];

/// `text` blank padded (or cut) to `width` bytes.
fn padded(text: &str, width: usize) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.resize(width, b' ');
    bytes.truncate(width);
    bytes
}

/// A date and time as SAS writes them in a header: 26NOV26:07:30:00.
fn stamp(seconds: u64) -> String {
    let (y, m, d) = civil_from_days((seconds / 86_400) as i64);
    let day = seconds % 86_400;
    format!(
        "{d:02}{}{:02}:{:02}:{:02}:{:02}",
        MONTHS[(m - 1) as usize],
        y.rem_euclid(100),
        day / 3600,
        day % 3600 / 60,
        day % 60
    )
}

fn record(text: &str) -> Vec<u8> {
    padded(text, 80)
}

/// How a numeric variable is described: format name, format width, and what to do with a value.
enum Number {
    Plain,
    Date,
    Datetime,
    Time,
}

/// The file and the warnings; `now` is the clock in seconds since 1970 and `member` the name
/// of the data set.
pub(crate) fn write_xport(
    data: &Dataset,
    member: &str,
    now: u64,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut warnings = Vec::new();
    let variables: Vec<_> = data.variables().collect();
    if variables.is_empty() {
        return Err("the dataset has no variables".into());
    }
    let names = xport_names(variables.iter().map(|v| v.name()));
    let created = stamp(now);
    // the descriptors, and per variable its length and kind
    let mut namestrs: Vec<u8> = Vec::new();
    let mut layout: Vec<(usize, Option<Number>)> = Vec::new();
    let mut position = 0usize;
    for (n, (v, name)) in variables.iter().zip(&names).enumerate() {
        let (ntype, length, format, width, number) = match v.string_width() {
            Some(w) => {
                if w > 200 {
                    warnings.push(format!("{}: text is cut to 200 bytes", v.name()));
                }
                (2i16, w.clamp(1, 200), "", 0i16, None)
            }
            None => {
                let (format, width, number) = match kind(v.format()).as_str() {
                    "DATE" | "ADATE" | "EDATE" | "JDATE" | "SDATE" | "QYR" | "MOYR" | "WKYR" => {
                        ("DATE", 9, Number::Date)
                    }
                    "DATETIME" | "YMDHMS" => ("DATETIME", 16, Number::Datetime),
                    "TIME" | "DTIME" | "MTIME" => ("TIME", 8, Number::Time),
                    _ => ("", 12, Number::Plain),
                };
                (1i16, 8, format, width, Some(number))
            }
        };
        let label = v.label().filter(|l| !l.is_empty()).unwrap_or(v.name());
        let mut d = Vec::with_capacity(140);
        d.extend_from_slice(&ntype.to_be_bytes());
        d.extend_from_slice(&0i16.to_be_bytes());
        d.extend_from_slice(&(length as i16).to_be_bytes());
        d.extend_from_slice(&((n + 1) as i16).to_be_bytes());
        d.extend(padded(name, 8));
        d.extend(padded(cut_to(label, 40), 40));
        d.extend(padded(format, 8));
        d.extend_from_slice(&width.to_be_bytes());
        d.extend_from_slice(&0i16.to_be_bytes());
        d.extend_from_slice(&0i16.to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend(padded("", 8));
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(&(position as i32).to_be_bytes());
        d.extend_from_slice(&[0; 52]);
        namestrs.extend(d);
        layout.push((length, number));
        position += length;
    }
    let mut out = Vec::new();
    out.extend(record(
        "HEADER RECORD*******LIBRARY HEADER RECORD!!!!!!!000000000000000000000000000000",
    ));
    out.extend(record(&format!(
        "SAS     SAS     SASLIB  6.06    Statscft{}{created}",
        " ".repeat(24)
    )));
    out.extend(record(&created));
    out.extend(record(
        "HEADER RECORD*******MEMBER  HEADER RECORD!!!!!!!000000000000000001600000000140",
    ));
    out.extend(record(
        "HEADER RECORD*******DSCRPTR HEADER RECORD!!!!!!!000000000000000000000000000000",
    ));
    out.extend(record(&format!(
        "SAS     {:<8}SASDATA 6.06    Statscft{}{created}",
        member,
        " ".repeat(24)
    )));
    out.extend(record(&created));
    out.extend(record(&format!(
        "HEADER RECORD*******NAMESTR HEADER RECORD!!!!!!!000000{:04}00000000000000000000",
        variables.len()
    )));
    out.extend(&namestrs);
    out.resize(out.len().next_multiple_of(80), b' ');
    out.extend(record(
        "HEADER RECORD*******OBS     HEADER RECORD!!!!!!!000000000000000000000000000000",
    ));
    let mut lost: Vec<&str> = Vec::new();
    let mut large: Vec<&str> = Vec::new();
    for case in 0..data.case_count() {
        for (v, (length, number)) in variables.iter().zip(&layout) {
            match (v.values(), number) {
                (Values::String(values), _) => {
                    let text = values.get(case);
                    let (bytes, was_lost) = latin1(text.trim_end(), *length);
                    if was_lost && !lost.contains(&v.name()) {
                        lost.push(v.name());
                    }
                    out.extend(bytes);
                }
                (Values::Numeric(values), Some(number)) => {
                    let x = values
                        .get(case)
                        .filter(|x| x.is_finite() && !v.is_user_missing(*x));
                    let x = x.map(|x| match number {
                        Number::Plain | Number::Time => x,
                        Number::Date => (x / 86_400.0).floor() - (SECONDS_TO_1960 / 86_400.0),
                        Number::Datetime => x - SECONDS_TO_1960,
                    });
                    match x.map(hex_float) {
                        Some(Some(bytes)) => out.extend(bytes),
                        Some(None) => {
                            if !large.contains(&v.name()) {
                                large.push(v.name());
                            }
                            out.extend(MISSING);
                        }
                        None => out.extend(MISSING),
                    }
                }
                _ => unreachable!("a field has the kind of its variable"),
            }
        }
    }
    out.resize(out.len().next_multiple_of(80), b' ');
    for name in lost {
        warnings.push(format!(
            "{name}: characters outside Windows Latin-1 are written as ?"
        ));
    }
    for name in large {
        warnings.push(format!(
            "{name}: values too large for a SAS number are written as missing"
        ));
    }
    Ok((out, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_floats_are_the_hexadecimal_form_of_the_number() {
        // 1 = 0x41 10 00.., 0.5 = 0x40 80 00.., -118.625 = 0xC2 76 A0 00..
        assert_eq!(hex_float(1.0), Some([0x41, 0x10, 0, 0, 0, 0, 0, 0]));
        assert_eq!(hex_float(0.5), Some([0x40, 0x80, 0, 0, 0, 0, 0, 0]));
        assert_eq!(hex_float(-118.625), Some([0xC2, 0x76, 0xA0, 0, 0, 0, 0, 0]));
        assert_eq!(hex_float(0.0), Some([0; 8]));
        assert_eq!(hex_float(1e80), None);
    }
}
