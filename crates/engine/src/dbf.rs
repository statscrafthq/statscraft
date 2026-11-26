//! SAVE TRANSLATE /TYPE=DB3 and DB4: the active dataset as a dBASE III or dBASE IV file.
//!
//! The manual gives the rules in words: names, numbers by the width and decimals of the print
//! format, a missing number as asterisks, string variables as character fields, and the limits
//! on the variables. Choices where it is silent:
//! - A field name is the variable name in capitals, anything but a letter, digit or underscore
//!   an underscore, V in front of a name that does not start with a letter, at most 10 bytes,
//!   made unique with a number at the end.
//! - A numeric field is as wide as the print format and has its decimals, at most 19 wide
//!   (dBASE III) or 20 (dBASE IV) and with room for the point and a digit before it. A
//!   missing number, or one that does not fit, is asterisks over the whole field.
//! - A date, a date with a time of day, QYR, MOYR and WKYR are date fields (the time of day is
//!   not kept, with a warning); a time and every other number are numeric fields as above.
//! - A string variable is a character field of its width, at most 254 bytes (longer text is cut
//!   with a warning). Text is Windows Latin-1 (language driver 0x03); a character it cannot
//!   hold becomes ? with a warning.
//! - MISSING=RECODE makes user-missing values missing; a missing string is blanks.
//! - More variables than the type holds are left off, with a warning. A record longer than
//!   4000 bytes is an error.
//! - The date in the header is today's.

use crate::expr::{DAYS_TO_1970, civil_from_days};
use crate::save_translate::kind;
use statscraft_data::{Dataset, Values, cut_to};

/// What a dBASE file of one type can hold.
pub(crate) struct DbfLimits {
    pub version: u8,
    pub fields: usize,
    pub width: usize,
}

impl DbfLimits {
    pub(crate) fn new(four: bool) -> Self {
        match four {
            true => DbfLimits {
                version: 0x04,
                fields: 255,
                width: 20,
            },
            false => DbfLimits {
                version: 0x03,
                fields: 128,
                width: 19,
            },
        }
    }
}

/// The longest record dBASE files hold, with its deletion flag.
const RECORD: usize = 4000;

/// The names the variables get as fields.
pub(crate) fn dbf_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    for name in names {
        let mut clean: String = name
            .chars()
            .map(|c| match c.is_ascii_alphanumeric() || c == '_' {
                true => c.to_ascii_uppercase(),
                false => '_',
            })
            .collect();
        if !clean.starts_with(|c: char| c.is_ascii_alphabetic()) {
            clean.insert(0, 'V');
        }
        let mut short = cut_to(&clean, 10).to_string();
        let mut n = 2;
        while taken.contains(&short) {
            let suffix = n.to_string();
            short = format!("{}{suffix}", cut_to(&clean, 10 - suffix.len()));
            n += 1;
        }
        taken.push(short);
    }
    taken
}

/// The width and decimals of a print format such as "F8.2" (8 wide without a number).
fn size(format: &str) -> (usize, usize) {
    let rest = format.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let (width, decimals) = rest.split_once('.').unwrap_or((rest, ""));
    (width.parse().unwrap_or(8), decimals.parse().unwrap_or(0))
}

enum Kind {
    Text(usize),
    Date,
    Number(usize, usize),
}

/// Text in Windows Latin-1, cut to `width` bytes, padded with blanks; whether a character was
/// lost.
pub(crate) fn latin1(text: &str, width: usize) -> (Vec<u8>, bool) {
    let mut lost = false;
    let mut bytes: Vec<u8> = Vec::new();
    let mut one = [0u8; 4];
    for c in text.chars() {
        let (encoded, _, unmappable) = encoding_rs::WINDOWS_1252.encode(c.encode_utf8(&mut one));
        match unmappable {
            true => {
                lost = true;
                bytes.push(b'?');
            }
            false => bytes.extend_from_slice(&encoded),
        }
    }
    bytes.truncate(width);
    bytes.resize(width, b' ');
    (bytes, lost)
}

/// The file and the warnings.
pub(crate) fn write_dbf(
    data: &Dataset,
    limits: &DbfLimits,
    recode_missing: bool,
    today: (i64, i64, i64),
) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut warnings = Vec::new();
    let mut variables: Vec<_> = data.variables().collect();
    if variables.len() > limits.fields {
        warnings.push(format!(
            "the file holds {} variables; the last {} are left out",
            limits.fields,
            variables.len() - limits.fields
        ));
        variables.truncate(limits.fields);
    }
    let names = dbf_names(variables.iter().map(|v| v.name()));
    let mut kinds = Vec::new();
    for v in &variables {
        kinds.push(match v.string_width() {
            Some(width) => {
                if width > 254 {
                    warnings.push(format!("{}: text is cut to 254 bytes", v.name()));
                }
                Kind::Text(width.clamp(1, 254))
            }
            None => {
                let (width, decimals) = size(v.format());
                match kind(v.format()).as_str() {
                    "DATE" | "ADATE" | "EDATE" | "JDATE" | "SDATE" | "QYR" | "MOYR" | "WKYR" => {
                        Kind::Date
                    }
                    "DATETIME" | "YMDHMS" => {
                        warnings.push(format!(
                            "{}: only the date is kept, not the time of day",
                            v.name()
                        ));
                        Kind::Date
                    }
                    _ => {
                        let width = width.clamp(1, limits.width);
                        let decimals = decimals_for(width, decimals);
                        Kind::Number(width, decimals)
                    }
                }
            }
        });
    }
    let lengths: Vec<usize> = kinds
        .iter()
        .map(|k| match k {
            Kind::Text(w) | Kind::Number(w, _) => *w,
            Kind::Date => 8,
        })
        .collect();
    let record_length = 1 + lengths.iter().sum::<usize>();
    if record_length > RECORD {
        return Err(format!(
            "a record would be {record_length} bytes; dBASE files hold at most {RECORD}"
        ));
    }
    let header = 32 + 32 * variables.len() + 1;
    let cases = data.case_count();
    let mut out = vec![0u8; 32];
    out[0] = limits.version;
    out[1] = (today.0 - 1900).clamp(0, 255) as u8;
    out[2] = today.1 as u8;
    out[3] = today.2 as u8;
    out[4..8].copy_from_slice(&(cases as u32).to_le_bytes());
    out[8..10].copy_from_slice(&(header as u16).to_le_bytes());
    out[10..12].copy_from_slice(&(record_length as u16).to_le_bytes());
    out[29] = 0x03;
    for ((name, kind), length) in names.iter().zip(&kinds).zip(&lengths) {
        let mut field = [0u8; 32];
        field[..name.len()].copy_from_slice(name.as_bytes());
        field[11] = match kind {
            Kind::Text(_) => b'C',
            Kind::Date => b'D',
            Kind::Number(..) => b'N',
        };
        field[16] = *length as u8;
        if let Kind::Number(_, decimals) = kind {
            field[17] = *decimals as u8;
        }
        out.extend_from_slice(&field);
    }
    out.push(0x0D);
    let mut lost: Vec<&str> = Vec::new();
    let mut overflow: Vec<&str> = Vec::new();
    for case in 0..cases {
        out.push(b' ');
        for (v, kind) in variables.iter().zip(&kinds) {
            match (kind, v.values()) {
                (Kind::Text(width), Values::String(values)) => {
                    let text = values.get(case);
                    let text = match recode_missing && v.is_user_missing_str(text) {
                        true => "",
                        false => text.trim_end(),
                    };
                    let (bytes, was_lost) = latin1(text, *width);
                    if was_lost && !lost.contains(&v.name()) {
                        lost.push(v.name());
                    }
                    out.extend_from_slice(&bytes);
                }
                (kind, Values::Numeric(values)) => {
                    let x = values
                        .get(case)
                        .filter(|&x| !(recode_missing && v.is_user_missing(x)));
                    match kind {
                        Kind::Date => match x.and_then(date) {
                            Some(text) => out.extend_from_slice(text.as_bytes()),
                            None => out.extend_from_slice(b"        "),
                        },
                        Kind::Number(width, decimals) => {
                            // a value that rounds to zero keeps no minus sign
                            let text = x.filter(|x| x.is_finite()).map(|x| {
                                let t = format!("{x:.decimals$}");
                                match t.strip_prefix('-') {
                                    Some(rest) if rest.parse::<f64>() == Ok(0.0) => {
                                        rest.to_string()
                                    }
                                    _ => t,
                                }
                            });
                            let text = text.filter(|t| t.len() <= *width);
                            match text {
                                Some(t) => out.extend_from_slice(format!("{t:>width$}").as_bytes()),
                                None => {
                                    if x.is_some() && !overflow.contains(&v.name()) {
                                        overflow.push(v.name());
                                    }
                                    out.extend(std::iter::repeat_n(b'*', *width));
                                }
                            }
                        }
                        Kind::Text(_) => unreachable!("a string variable has string values"),
                    }
                }
                _ => unreachable!("a field has the kind of its variable"),
            }
        }
    }
    out.push(0x1A);
    for name in lost {
        warnings.push(format!(
            "{name}: characters outside Windows Latin-1 are written as ?"
        ));
    }
    for name in overflow {
        warnings.push(format!(
            "{name}: values that do not fit the width of the print format are missing; widen it \
             with PRINT FORMATS"
        ));
    }
    Ok((out, warnings))
}

/// The decimals a field of `width` can carry: the point and one digit before it take room.
fn decimals_for(width: usize, decimals: usize) -> usize {
    decimals.min(width.saturating_sub(2)).min(15)
}

/// A date value (seconds from 1582-10-14) as YYYYMMDD; None past the years 0 to 9999.
fn date(x: f64) -> Option<String> {
    let (y, m, d) = civil_from_days((x / 86_400.0).floor() as i64 - DAYS_TO_1970);
    (0..=9999)
        .contains(&y)
        .then(|| format!("{y:04}{m:02}{d:02}"))
}
