//! Stata data files (.dta): GET STATA reads them, SAVE TRANSLATE /TYPE=STATA writes them.
//!
//! The format, from the descriptions Stata publishes: formats 113 (Stata 8 and 9), 114 (10 and
//! 11) and 115 (12) are a fixed binary header and fixed-width dictionary arrays; formats 117
//! (13), 118 (14 to 19) and 119 (more than 32,767 variables) put each part between tags such as
//! `<data>` and add long strings (strL). Numbers are in the byte order the header names. Every
//! numeric type keeps its largest values for the missing values `.` and `.a` to `.z`.
//!
//! Choices where the manual is silent:
//! - A %tc or %tC variable (a date with a time of day) becomes DATETIME, not DATE, so the time
//!   survives; leap seconds of %tC are not counted. When writing, DATETIME and YMDHMS become
//!   %tc (milliseconds since 1960) from Stata 10 on; format 113 has no such format, so they are
//!   written as days, like dates.
//! - Values of a %g format that are all whole numbers get F with 0 decimals, else 2.
//! - A name that starts with _ gets V in front; a reserved word gets $ at the end.
//! - Text of files older than Stata 14 is UTF-8 when it decodes as UTF-8, else in the locale's
//!   code page, unless ENCODING names one. A long string marked binary is read as text when it
//!   holds no zero byte, else it is blank.
//! - Numbers are written as byte, int or long when every value is whole and fits, else as
//!   double. Names that are not ASCII become v1, v2, ... (by position) before Stata 14.
//! - The file label is cut to the 64 bytes a .sav file holds; value labels to 255 bytes.

use crate::expr::DAYS_TO_1970;
use crate::get_translate::column_letters;
use crate::save_translate::kind;
use crate::{Engine, Error, Table};
use encoding_rs::Encoding;
use statscraft_data::{Dataset, Value, Values, cut_to, is_valid_name};
use statscraft_syntax::{GetStata, encoding_for, legacy_encoding};
use std::collections::HashMap;

/// Days from 1582-10-14 (day 0 of our dates) to 1960-01-01 (day 0 of Stata's).
const DAYS_TO_1960: i64 = DAYS_TO_1970 - 3653;
pub(crate) const SECONDS_TO_1960: f64 = (DAYS_TO_1960 * 86_400) as f64;
/// The longest string a variable holds here; longer long strings are cut.
const MAX_STRING: usize = 32_767;
const MAX_FILE_LABEL: usize = 64;
const MAX_VALUE_LABEL: usize = 255;
/// Our reserved words; a Stata name equal to one gets $ at the end.
const RESERVED: [&str; 13] = [
    "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
];
/// Stata's reserved words that one of our names can spell.
const STATA_RESERVED: [&str; 9] = [
    "byte", "double", "float", "if", "in", "int", "long", "strL", "using",
];

/// The encoding ENCODING names: SYSTEM and LOCALE are the system's code page.
pub(crate) fn stata_encoding(name: &str) -> Option<&'static Encoding> {
    match name.trim().eq_ignore_ascii_case("system") {
        true => Some(legacy_encoding()),
        false => encoding_for(name),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Type {
    Str(usize),
    StrL,
    Byte,
    Int,
    Long,
    Float,
    Double,
}

impl Type {
    fn size(self) -> usize {
        match self {
            Type::Str(w) => w,
            Type::StrL | Type::Double => 8,
            Type::Byte => 1,
            Type::Int => 2,
            Type::Long | Type::Float => 4,
        }
    }

    fn is_string(self) -> bool {
        matches!(self, Type::Str(_) | Type::StrL)
    }
}

/// The sizes of the fields that differ between formats.
struct Layout {
    format: u16,
    tagged: bool,
    name: usize,
    fmt: usize,
    label: usize,
}

impl Layout {
    fn new(format: u16) -> Layout {
        let (name, fmt, label) = match format {
            113 => (33, 12, 81),
            114..=117 => (33, 49, 81),
            _ => (129, 57, 321),
        };
        Layout {
            format,
            tagged: format >= 117,
            name,
            fmt,
            label,
        }
    }

    /// Bytes of v in a strL reference; o takes the rest of the 8.
    fn v_bytes(&self) -> usize {
        match self.format {
            117 => 4,
            118 => 2,
            _ => 3,
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    big: bool,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.bytes.len());
        let Some(end) = end else {
            return Err(format!("the file ends early, at byte {}", self.bytes.len()));
        };
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn uint(&mut self, n: usize) -> Result<u64, String> {
        Ok(uint(self.take(n)?, self.big))
    }

    fn tag(&mut self, tag: &str) -> Result<(), String> {
        let at = self.pos;
        if self.take(tag.len())? == tag.as_bytes() {
            Ok(())
        } else {
            Err(format!("expected {tag} at byte {at}"))
        }
    }

    fn at(&self, tag: &str) -> bool {
        self.bytes[self.pos..].starts_with(tag.as_bytes())
    }
}

/// An unsigned integer of 1 to 8 bytes in the given byte order.
fn uint(bytes: &[u8], big: bool) -> u64 {
    let fold = |v: u64, &b: &u8| (v << 8) | u64::from(b);
    match big {
        true => bytes.iter().fold(0, fold),
        false => bytes.iter().rev().fold(0, fold),
    }
}

/// Text up to the first zero byte.
fn c_string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}

/// How text of the file becomes UTF-8.
struct Decoder {
    utf8: bool,
    encoding: Option<&'static Encoding>,
}

impl Decoder {
    fn decode(&self, bytes: &[u8]) -> String {
        if self.utf8 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        match (self.encoding, std::str::from_utf8(bytes)) {
            (None, Ok(text)) => text.to_string(),
            (encoding, _) => {
                let encoding = encoding.unwrap_or_else(legacy_encoding);
                encoding.decode_without_bom_handling(bytes).0.into_owned()
            }
        }
    }
}

/// A numeric value, or None for any of Stata's missing values.
fn number(t: Type, raw: &[u8], big: bool) -> Option<f64> {
    let u = uint(raw, big);
    match t {
        Type::Byte => Some(f64::from(u as u8 as i8)).filter(|&v| v <= 100.0),
        Type::Int => Some(f64::from(u as u16 as i16)).filter(|&v| v <= 32_740.0),
        Type::Long => Some(f64::from(u as u32 as i32)).filter(|&v| v <= 2_147_483_620.0),
        Type::Float => {
            let v = f32::from_bits(u as u32);
            (v <= f32::from_bits(0x7eff_ffff)).then_some(f64::from(v))
        }
        Type::Double => {
            let v = f64::from_bits(u);
            (v <= f64::from_bits(0x7fdf_ffff_ffff_ffff)).then_some(v)
        }
        Type::Str(_) | Type::StrL => None,
    }
}

/// A variable as the dictionary of the file declares it.
struct Declared {
    t: Type,
    name: String,
    format: String,
    value_labels: String,
    label: String,
}

/// Our format for a Stata display format, and how values convert: 1 for days to seconds,
/// 2 for milliseconds to seconds, 0 for none.
fn our_format(stata: &str, t: Type, whole: bool) -> (String, u8) {
    let f = stata.trim_start_matches('%').trim_start_matches('-');
    if f.starts_with("tc") || f.starts_with("tC") {
        return ("DATETIME20".into(), 2);
    }
    if f.starts_with("td") || f.starts_with('d') {
        return ("DATE11".into(), 1);
    }
    if f.starts_with('t') {
        // weeks, months, quarters, half years, years and business days stay counts since 1960
        return ("F8.0".into(), 0);
    }
    let digits = |s: &str| {
        s.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
    };
    let width_text = digits(f);
    let rest = &f[width_text.len()..];
    let (decimals_text, kind) = match rest.strip_prefix(['.', ',']) {
        Some(after) => {
            let d = digits(after);
            let kind = after[d.len()..].to_string();
            (d, kind)
        }
        None => (String::new(), rest.to_string()),
    };
    let width = width_text.parse::<usize>().unwrap_or(8).clamp(1, 40);
    let decimals = decimals_text.parse::<usize>().unwrap_or(0).min(16);
    let integer = matches!(t, Type::Byte | Type::Int | Type::Long) || whole;
    let comma = kind.ends_with('c');
    let format = match kind.trim_end_matches('c') {
        "f" => format!(
            "{}{width}.{}",
            if comma { "COMMA" } else { "F" },
            decimals.min(width - 1)
        ),
        "e" => format!("E{}.{decimals}", width.max(decimals + 7)),
        _ => {
            let decimals = if integer { 0 } else { 2.min(width - 1) };
            format!("{}{width}.{decimals}", if comma { "COMMA" } else { "F" })
        }
    };
    (format, 0)
}

/// Stata names made valid here: other characters become _, a leading _ gets V in front, a
/// reserved word gets $, and names equal but for case get _A, _B, ... as the manual says.
fn our_names(names: &[String]) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for name in names {
        let mut clean: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        if !clean.starts_with(char::is_alphabetic) {
            clean.insert(0, 'V');
        }
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&clean)) {
            clean.push('$');
        }
        let mut clean = cut_to(&clean, 60).to_string();
        if !is_valid_name(&clean) {
            clean = format!("V{}", used.len() + 1);
        }
        let mut unique = clean.clone();
        let mut n = 0;
        while used.iter().any(|u| u.eq_ignore_ascii_case(&unique)) {
            unique = format!("{clean}_{}", column_letters(n));
            n += 1;
        }
        used.push(unique);
    }
    used
}

/// Reads a .dta file: the dataset and warnings about what changed on the way.
pub(crate) fn read_dta(
    bytes: &[u8],
    encoding: Option<&'static Encoding>,
) -> Result<(Dataset, Vec<String>), String> {
    let mut r = Reader {
        bytes,
        pos: 0,
        big: false,
    };
    let mut warnings = Vec::new();
    let tagged = bytes.starts_with(b"<stata_dta>");
    let (layout, k, n, file_label) = if tagged {
        r.tag("<stata_dta><header><release>")?;
        let release = std::str::from_utf8(r.take(3)?).unwrap_or_default();
        let format = match release {
            "117" => 117,
            "118" => 118,
            "119" => 119,
            other => {
                return Err(format!(
                    "Stata format {other} is not supported; formats 113 to 119 are"
                ));
            }
        };
        r.tag("</release><byteorder>")?;
        r.big = match r.take(3)? {
            b"MSF" => true,
            b"LSF" => false,
            _ => return Err("the byte order is neither MSF nor LSF".into()),
        };
        r.tag("</byteorder><K>")?;
        let k = r.uint(if format == 119 { 4 } else { 2 })?;
        r.tag("</K><N>")?;
        let n = r.uint(if format == 117 { 4 } else { 8 })?;
        r.tag("</N><label>")?;
        let length = r.uint(if format == 117 { 1 } else { 2 })?;
        let label = r.take(length as usize)?;
        r.tag("</label><timestamp>")?;
        let length = r.uint(1)?;
        r.take(length as usize)?;
        r.tag("</timestamp></header><map>")?;
        r.take(14 * 8)?;
        r.tag("</map><variable_types>")?;
        (Layout::new(format), k, n, label)
    } else {
        let head = r.take(4)?;
        match head[0] {
            113..=115 => {}
            104..=112 => {
                return Err(format!(
                    "files of Stata 7 and older (format {}) are not supported; formats 113 to \
                     119 are",
                    head[0]
                ));
            }
            _ => return Err("not a Stata data file".into()),
        }
        r.big = match head[1] {
            1 => true,
            2 => false,
            _ => return Err("not a Stata data file: unknown byte order".into()),
        };
        let k = r.uint(2)?;
        let n = r.uint(4)?;
        let label = c_string(r.take(81)?);
        r.take(18)?;
        (Layout::new(u16::from(head[0])), k, n, label)
    };
    let decoder = Decoder {
        utf8: layout.format >= 118,
        encoding,
    };
    let k = usize::try_from(k).unwrap_or(usize::MAX);
    if k.saturating_mul(layout.name) > bytes.len() {
        return Err(format!(
            "the file ends before its {k} variables are described"
        ));
    }
    let mut types = Vec::with_capacity(k.min(1 << 16));
    for _ in 0..k {
        let code = r.uint(if tagged { 2 } else { 1 })?;
        types.push(match (tagged, code) {
            (true, 1..=2045) | (false, 1..=244) => Type::Str(code as usize),
            (true, 32768) => Type::StrL,
            (true, 65526) | (false, 255) => Type::Double,
            (true, 65527) | (false, 254) => Type::Float,
            (true, 65528) | (false, 253) => Type::Long,
            (true, 65529) | (false, 252) => Type::Int,
            (true, 65530) | (false, 251) => Type::Byte,
            _ => return Err(format!("unknown variable type {code}")),
        });
    }
    let section = |r: &mut Reader, close: &str, open: &str| match tagged {
        true => r.tag(&format!("</{close}><{open}>")),
        false => Ok(()),
    };
    let strings = |r: &mut Reader, size: usize| -> Result<Vec<String>, String> {
        (0..k)
            .map(|_| Ok(decoder.decode(c_string(r.take(size)?))))
            .collect()
    };
    section(&mut r, "variable_types", "varnames")?;
    let names = strings(&mut r, layout.name)?;
    section(&mut r, "varnames", "sortlist")?;
    r.take((k + 1) * if layout.format == 119 { 4 } else { 2 })?;
    section(&mut r, "sortlist", "formats")?;
    let formats = strings(&mut r, layout.fmt)?;
    section(&mut r, "formats", "value_label_names")?;
    let label_names = strings(&mut r, layout.name)?;
    section(&mut r, "value_label_names", "variable_labels")?;
    let labels = strings(&mut r, layout.label)?;
    if tagged {
        r.tag("</variable_labels><characteristics>")?;
        while r.at("<ch>") {
            r.tag("<ch>")?;
            let length = r.uint(4)?;
            r.take(length as usize)?;
            r.tag("</ch>")?;
        }
        r.tag("</characteristics><data>")?;
    } else {
        // expansion fields: a type byte and a length until both are zero
        loop {
            let kind = r.uint(1)?;
            let length = r.uint(4)?;
            if kind == 0 && length == 0 {
                break;
            }
            r.take(length as usize)?;
        }
    }
    let declared: Vec<Declared> = (0..k)
        .map(|i| Declared {
            t: types[i],
            name: names[i].clone(),
            format: formats[i].clone(),
            value_labels: label_names[i].clone(),
            label: labels[i].clone(),
        })
        .collect();

    // the cases: fixed-width rows
    let row: usize = declared.iter().map(|d| d.t.size()).sum();
    let n = usize::try_from(n).map_err(|_| "too many cases".to_string())?;
    let data_bytes = row
        .checked_mul(n)
        .filter(|&b| b <= bytes.len() - r.pos)
        .ok_or_else(|| format!("the file ends before its {n} cases"))?;
    let data = r.take(data_bytes)?;
    let v_bytes = layout.v_bytes();
    let mut numbers: Vec<Vec<Option<f64>>> = vec![Vec::new(); k];
    let mut texts: Vec<Vec<String>> = vec![Vec::new(); k];
    // strL references (v, o) by variable, filled in from the strLs
    let mut refs: Vec<Vec<(u64, u64)>> = vec![Vec::new(); k];
    let mut offset = 0;
    // without variables there is nothing to read per case
    for _ in 0..if row == 0 { 0 } else { n } {
        for (i, d) in declared.iter().enumerate() {
            let raw = &data[offset..offset + d.t.size()];
            offset += d.t.size();
            match d.t {
                Type::Str(_) => texts[i].push(decoder.decode(c_string(raw))),
                Type::StrL => {
                    let v = uint(&raw[..v_bytes], r.big);
                    let o = uint(&raw[v_bytes..], r.big);
                    refs[i].push((v, o));
                }
                t => numbers[i].push(number(t, raw, r.big)),
            }
        }
    }
    let mut long_strings: HashMap<(u64, u64), String> = HashMap::new();
    if tagged {
        r.tag("</data><strls>")?;
        while r.at("GSO") {
            r.tag("GSO")?;
            let v = r.uint(4)?;
            let o = r.uint(if layout.format == 117 { 4 } else { 8 })?;
            let kind = r.uint(1)?;
            let length = r.uint(4)?;
            let contents = r.take(length as usize)?;
            let text = match kind {
                130 => decoder.decode(c_string(contents)),
                // binary: blank, unless it is text that was only marked binary
                _ if contents.contains(&0) => String::new(),
                _ => decoder.decode(contents),
            };
            long_strings.insert((v, o), text);
        }
        r.tag("</strls><value_labels>")?;
    }

    // value labels: name, then n values and their texts
    let mut tables: HashMap<String, Vec<(f64, String)>> = HashMap::new();
    loop {
        if tagged {
            if !r.at("<lbl>") {
                r.tag("</value_labels></stata_dta>")?;
                break;
            }
            r.tag("<lbl>")?;
        } else if r.pos >= bytes.len() {
            break;
        }
        let length = r.uint(4)? as usize;
        let name = decoder.decode(c_string(r.take(layout.name)?));
        r.take(3)?;
        let table = r.take(length)?;
        let mut t = Reader {
            bytes: table,
            pos: 0,
            big: r.big,
        };
        let count = t.uint(4)? as usize;
        let text_length = t.uint(4)? as usize;
        if count.saturating_mul(8) > table.len() {
            return Err(format!("value labels {name} do not fit their table"));
        }
        let offsets = (0..count)
            .map(|_| t.uint(4))
            .collect::<Result<Vec<_>, _>>()?;
        let values = (0..count)
            .map(|_| t.uint(4))
            .collect::<Result<Vec<_>, _>>()?;
        let text = t.take(text_length)?;
        let mut entries = Vec::new();
        for (off, value) in offsets.into_iter().zip(values) {
            let value = f64::from(value as u32 as i32);
            // labels of .a to .z are left out, as their values become system-missing
            if value > 2_147_483_620.0 {
                continue;
            }
            let label = text.get(off as usize..).map(c_string).unwrap_or_default();
            let label = decoder.decode(label);
            entries.push((value, cut_to(&label, MAX_VALUE_LABEL).to_string()));
        }
        tables.insert(name, entries);
        if tagged {
            r.tag("</lbl>")?;
        }
    }

    // the dataset
    let file_names: Vec<String> = declared.iter().map(|d| d.name.clone()).collect();
    let names = our_names(&file_names);
    let renamed: Vec<String> = file_names
        .iter()
        .zip(&names)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("{a} as {b}"))
        .collect();
    if !renamed.is_empty() {
        warnings.push(format!("variables renamed: {}", renamed.join(", ")));
    }
    let mut out = Dataset::new(n);
    let mut cut = Vec::new();
    let fail = |e: statscraft_data::Error| e.to_string();
    for (i, (d, name)) in declared.iter().zip(&names).enumerate() {
        if d.t.is_string() {
            let mut values = std::mem::take(&mut texts[i]);
            if d.t == Type::StrL {
                for (case, &(v, o)) in refs[i].iter().enumerate() {
                    let text = match (v, o) {
                        (0, 0) => String::new(),
                        vo => long_strings.get(&vo).cloned().ok_or_else(|| {
                            format!(
                                "case {}: {name} refers to a long string the file lacks",
                                case + 1
                            )
                        })?,
                    };
                    values.push(text);
                }
            }
            if values.iter().any(|v| v.len() > MAX_STRING) {
                cut.push(name.clone());
                for v in &mut values {
                    *v = cut_to(v, MAX_STRING).to_string();
                }
            }
            let declared_width = if let Type::Str(w) = d.t { w } else { 1 };
            let longest = values
                .iter()
                .map(|v| v.trim_end_matches(' ').len())
                .max()
                .unwrap_or(0);
            out.set_string_values(name, longest.max(declared_width), values)
                .map_err(fail)?;
        } else {
            let mut values = std::mem::take(&mut numbers[i]);
            let whole = values.iter().flatten().all(|v| v.fract() == 0.0);
            let (mut format, convert) = our_format(&d.format, d.t, whole);
            if convert == 2 && values.iter().flatten().any(|v| (v / 1000.0).fract() != 0.0) {
                format = "DATETIME23.3".into();
            }
            for v in values.iter_mut().flatten() {
                match convert {
                    1 => *v = *v * 86_400.0 + SECONDS_TO_1960,
                    2 => *v = *v / 1000.0 + SECONDS_TO_1960,
                    _ => {}
                }
            }
            out.set_numeric_values(name, values).map_err(fail)?;
            if out.set_format(name, &format).is_err() {
                out.set_format(name, if whole { "F8.0" } else { "F8.2" })
                    .map_err(fail)?;
            }
            if let Some(entries) = tables
                .get(&d.value_labels)
                .filter(|_| !d.value_labels.is_empty())
            {
                let labels = entries
                    .iter()
                    .map(|(v, l)| (Value::Number(*v), l.clone()))
                    .collect();
                out.set_value_labels(name, labels).map_err(fail)?;
            }
        }
        out.set_variable_label(name, Some(&d.label)).map_err(fail)?;
    }
    if !cut.is_empty() {
        warnings.push(format!(
            "strings longer than {MAX_STRING} bytes are cut: {}",
            cut.join(", ")
        ));
    }
    let label = decoder.decode(file_label);
    if label.len() > MAX_FILE_LABEL {
        warnings.push(format!(
            "the file label is cut to {MAX_FILE_LABEL} bytes, as a .sav file holds no more"
        ));
    }
    out.set_file_label(Some(cut_to(&label, MAX_FILE_LABEL)));
    Ok((out, warnings))
}

/// How SAVE TRANSLATE /TYPE=STATA writes.
pub(crate) struct DtaOptions {
    /// The Stata release of VERSION: 8 or later.
    pub version: u32,
    /// EDITION=SE
    pub se: bool,
    /// ENCODING for files before Stata 14; None is the locale's.
    pub encoding: Option<&'static Encoding>,
    /// MISSING=RECODE
    pub recode_missing: bool,
    /// MSF byte order instead of LSF (tests use it).
    pub big: bool,
}

impl DtaOptions {
    fn format(&self) -> u16 {
        match self.version {
            ..=9 => 113,
            10 | 11 => 114,
            12 => 115,
            13 => 117,
            _ => 118,
        }
    }
}

/// The names the variables get in a Stata file: letters, digits and _ only, at most 32 bytes,
/// no Stata reserved word, unique; before Stata 14 a name that is not ASCII becomes v and its
/// position.
pub(crate) fn stata_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
    unicode: bool,
) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    for (position, name) in names.into_iter().enumerate() {
        let mut clean: String = match unicode || name.is_ascii() {
            true => name
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect(),
            false => format!("v{}", position + 1),
        };
        let typed = clean
            .strip_prefix("str")
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
        if typed || STATA_RESERVED.contains(&clean.as_str()) {
            clean.push('_');
        }
        let mut short = cut_to(&clean, 32).to_string();
        let mut n = 2;
        while taken.contains(&short) {
            let suffix = format!("_{n}");
            short = format!("{}{suffix}", cut_to(&clean, 32 - suffix.len()));
            n += 1;
        }
        taken.push(short);
    }
    taken
}

struct Writer {
    out: Vec<u8>,
    big: bool,
}

impl Writer {
    fn uint(&mut self, v: u64, n: usize) {
        let bytes = v.to_be_bytes();
        let low = &bytes[8 - n..];
        match self.big {
            true => self.out.extend(low),
            false => self.out.extend(low.iter().rev()),
        }
    }

    fn tag(&mut self, tag: &str) {
        self.out.extend(tag.as_bytes());
    }

    /// `text` in a field of `size` bytes ending in a zero byte.
    fn fixed(&mut self, text: &[u8], size: usize) {
        let text = &text[..text.len().min(size - 1)];
        self.out.extend(text);
        self.out.extend(std::iter::repeat_n(0, size - text.len()));
    }
}

/// One variable as the file receives it.
struct Column {
    name: String,
    t: Type,
    format: String,
    label: Vec<u8>,
    numbers: Vec<Option<f64>>,
    texts: Vec<Vec<u8>>,
    value_labels: Vec<(i32, Vec<u8>)>,
}

/// The smallest Stata type that holds every value exactly.
fn storage(values: &[Option<f64>]) -> Type {
    let (mut low, mut high) = (0.0f64, 0.0f64);
    for v in values.iter().flatten() {
        if v.fract() != 0.0 || !v.is_finite() {
            return Type::Double;
        }
        low = low.min(*v);
        high = high.max(*v);
    }
    match (low, high) {
        (l, h) if l >= -127.0 && h <= 100.0 => Type::Byte,
        (l, h) if l >= -32_767.0 && h <= 32_740.0 => Type::Int,
        (l, h) if l >= -2_147_483_647.0 && h <= 2_147_483_620.0 => Type::Long,
        _ => Type::Double,
    }
}

/// Writes `data` as a .dta file: the bytes and warnings about what the file cannot hold.
pub(crate) fn write_dta(data: &Dataset, o: &DtaOptions) -> Result<(Vec<u8>, Vec<String>), String> {
    let format = o.format();
    let layout = Layout::new(format);
    let unicode = format >= 118;
    let encoding = match unicode {
        true => encoding_rs::UTF_8,
        false => o.encoding.unwrap_or_else(legacy_encoding),
    };
    let encode = |text: &str| -> Result<Vec<u8>, String> {
        let (bytes, _, lost) = encoding.encode(text);
        if lost {
            return Err(format!(
                "'{text}' has characters that {} cannot hold; use /VERSION=14 or another ENCODING",
                encoding.name()
            ));
        }
        Ok(bytes.into_owned())
    };
    // at most `limit` bytes, not splitting a character
    let fit = |text: &str, limit: usize| -> Result<Vec<u8>, String> {
        if unicode {
            return Ok(cut_to(text, limit).as_bytes().to_vec());
        }
        // every character takes at least one byte
        let mut text = text
            .char_indices()
            .nth(limit)
            .map_or(text, |(i, _)| &text[..i]);
        let mut bytes = encode(text)?;
        while bytes.len() > limit {
            text = &text[..text.char_indices().last().map_or(0, |(i, _)| i)];
            bytes = encode(text)?;
        }
        Ok(bytes)
    };
    let mut warnings = Vec::new();
    let max_variables = if o.se { 32_767 } else { 2_047 };
    let max_string = match (o.se, format >= 117) {
        (false, _) => Some(80),
        (true, false) => Some(244),
        (true, true) => None,
    };
    let mut variables: Vec<_> = data.variables().collect();
    if variables.len() > max_variables {
        warnings.push(format!(
            "the file holds {max_variables} variables: the last {} are left out{}",
            variables.len() - max_variables,
            if o.se {
                ""
            } else {
                " (EDITION=SE holds 32767)"
            }
        ));
        variables.truncate(max_variables);
    }
    let names = stata_names(variables.iter().map(|v| v.name()), unicode);
    let renamed: Vec<String> = variables
        .iter()
        .zip(&names)
        .filter(|(v, n)| v.name() != n.as_str())
        .map(|(v, n)| format!("{} as {n}", v.name()))
        .collect();
    if !renamed.is_empty() {
        warnings.push(format!("names in the Stata file: {}", renamed.join(", ")));
    }
    let label_limit = if o.version <= 8 { 80 } else { 32_000 };
    let mut cut_strings = Vec::new();
    let mut dropped_labels = Vec::new();
    let mut columns = Vec::with_capacity(variables.len());
    for (v, name) in variables.iter().zip(names) {
        let label = fit(v.label().unwrap_or_default(), 80)?;
        let column = match v.values() {
            Values::String(values) => {
                let declared = v.string_width().unwrap_or(1);
                let width = max_string.map_or(declared, |m| declared.min(m));
                if width < declared {
                    cut_strings.push(v.name().to_string());
                }
                let texts = (0..data.case_count())
                    .map(|case| {
                        let text = values.get(case).trim_end_matches(' ');
                        match o.recode_missing && v.is_user_missing_str(text) {
                            true => Ok(Vec::new()),
                            false => fit(text, width),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !v.value_labels().is_empty() {
                    dropped_labels.push(v.name().to_string());
                }
                let t = match width > 2045 {
                    true => Type::StrL,
                    false => Type::Str(width.max(1)),
                };
                Column {
                    name,
                    t,
                    format: format!("%{}s", width.clamp(1, 2045)),
                    label,
                    numbers: Vec::new(),
                    texts,
                    value_labels: Vec::new(),
                }
            }
            Values::Numeric(values) => {
                let (stata_format, scale) = match kind(v.format()).as_str() {
                    "DATE" | "ADATE" | "EDATE" | "SDATE" | "JDATE" | "QYR" | "MOYR" | "WKYR" => {
                        (Some("%tdD_m_Y"), 1)
                    }
                    "DATETIME" | "YMDHMS" if format > 113 => (Some("%tc"), 2),
                    "DATETIME" | "YMDHMS" => (Some("%tdD_m_Y"), 1),
                    _ => (None, 0),
                };
                let numbers: Vec<Option<f64>> = values
                    .iter()
                    .map(|x| {
                        let x = x.filter(|&x| !(o.recode_missing && v.is_user_missing(x)))?;
                        Some(match scale {
                            1 => (x - SECONDS_TO_1960) / 86_400.0,
                            2 => (x - SECONDS_TO_1960) * 1000.0,
                            _ => x,
                        })
                    })
                    .collect();
                let t = storage(&numbers);
                let format = match stata_format {
                    // format 113 knows dates only by the old %d
                    Some(f) if format == 113 => f.replace("%td", "%d"),
                    Some(f) => f.to_string(),
                    None => match t {
                        Type::Byte | Type::Int => "%8.0g",
                        Type::Long => "%12.0g",
                        _ => "%10.0g",
                    }
                    .to_string(),
                };
                let mut value_labels = Vec::new();
                for (value, text) in v.value_labels() {
                    match value {
                        Value::Number(x)
                            if x.fract() == 0.0
                                && (-2_147_483_647.0..=2_147_483_620.0).contains(x) =>
                        {
                            value_labels.push((*x as i32, fit(text, label_limit)?));
                        }
                        _ => {
                            if dropped_labels.last().map(String::as_str) != Some(v.name()) {
                                dropped_labels.push(v.name().to_string());
                            }
                        }
                    }
                }
                value_labels.sort_by_key(|(x, _)| *x);
                Column {
                    name,
                    t,
                    format,
                    label,
                    numbers,
                    texts: Vec::new(),
                    value_labels,
                }
            }
        };
        columns.push(column);
    }
    if !cut_strings.is_empty() {
        warnings.push(format!(
            "strings are cut to {} bytes{}: {}",
            max_string.unwrap_or_default(),
            if o.se { "" } else { " (EDITION=SE keeps more)" },
            cut_strings.join(", ")
        ));
    }
    if !dropped_labels.is_empty() {
        warnings.push(format!(
            "a Stata file holds value labels only for whole numbers: labels of strings and other \
             values are left out for {}",
            dropped_labels.join(", ")
        ));
    }
    let cases = data.case_count();
    if format < 118 && u32::try_from(cases).is_err() {
        return Err("too many cases for this Stata version; use /VERSION=14".into());
    }

    let file_label = fit(data.file_label().unwrap_or_default(), 80)?;
    let stamp = chrono::Local::now().format("%d %b %Y %H:%M").to_string();
    let k = columns.len() as u64;
    let mut w = Writer {
        out: Vec::new(),
        big: o.big,
    };
    let mut map = [0u64; 14];
    if layout.tagged {
        w.tag("<stata_dta><header><release>");
        w.tag(&format.to_string());
        w.tag("</release><byteorder>");
        w.tag(if o.big { "MSF" } else { "LSF" });
        w.tag("</byteorder><K>");
        w.uint(k, 2);
        w.tag("</K><N>");
        w.uint(cases as u64, if format == 117 { 4 } else { 8 });
        w.tag("</N><label>");
        w.uint(file_label.len() as u64, if format == 117 { 1 } else { 2 });
        w.out.extend(&file_label);
        w.tag("</label><timestamp>");
        w.uint(stamp.len() as u64, 1);
        w.tag(&stamp);
        w.tag("</timestamp></header>");
        map[1] = w.out.len() as u64;
        w.tag("<map>");
        w.out.extend([0; 14 * 8]);
        w.tag("</map>");
    } else {
        w.out
            .extend([format as u8, if o.big { 1 } else { 2 }, 1, 0]);
        w.uint(k, 2);
        w.uint(cases as u64, 4);
        w.fixed(&file_label, 81);
        w.fixed(stamp.as_bytes(), 18);
    }
    // each dictionary part, with its tag and its place in the map
    let mut part = |w: &mut Writer, slot: usize, tag: &str, body: &dyn Fn(&mut Writer)| {
        if layout.tagged {
            map[slot] = w.out.len() as u64;
            w.tag(&format!("<{tag}>"));
        }
        body(w);
        if layout.tagged {
            w.tag(&format!("</{tag}>"));
        }
    };
    part(&mut w, 2, "variable_types", &|w| {
        for c in &columns {
            let code = match (layout.tagged, c.t) {
                (true, Type::Str(n)) => n as u64,
                (true, Type::StrL) => 32768,
                (true, Type::Double) => 65526,
                (true, Type::Long) => 65528,
                (true, Type::Int) => 65529,
                (true, _) => 65530,
                (false, Type::Str(n)) => n as u64,
                (false, Type::Double) => 255,
                (false, Type::Long) => 253,
                (false, Type::Int) => 252,
                (false, _) => 251,
            };
            w.uint(code, if layout.tagged { 2 } else { 1 });
        }
    });
    part(&mut w, 3, "varnames", &|w| {
        for c in &columns {
            w.fixed(c.name.as_bytes(), layout.name);
        }
    });
    part(&mut w, 4, "sortlist", &|w| {
        w.out.extend(vec![0; 2 * (columns.len() + 1)])
    });
    part(&mut w, 5, "formats", &|w| {
        for c in &columns {
            w.fixed(c.format.as_bytes(), layout.fmt);
        }
    });
    part(&mut w, 6, "value_label_names", &|w| {
        for c in &columns {
            let name = if c.value_labels.is_empty() {
                ""
            } else {
                &c.name
            };
            w.fixed(name.as_bytes(), layout.name);
        }
    });
    part(&mut w, 7, "variable_labels", &|w| {
        for c in &columns {
            w.fixed(&c.label, layout.label);
        }
    });
    part(&mut w, 8, "characteristics", &|w| {
        if !layout.tagged {
            w.out.extend([0; 5]);
        }
    });
    let v_bytes = layout.v_bytes();
    part(&mut w, 9, "data", &|w| {
        for case in 0..cases {
            for (i, c) in columns.iter().enumerate() {
                match c.t {
                    Type::Str(width) => w.out.extend(
                        c.texts[case]
                            .iter()
                            .copied()
                            .chain(std::iter::repeat(0))
                            .take(width),
                    ),
                    Type::StrL if c.texts[case].is_empty() => w.uint(0, 8),
                    Type::StrL => {
                        w.uint(i as u64 + 1, v_bytes);
                        w.uint(case as u64 + 1, 8 - v_bytes);
                    }
                    t => {
                        let x = c.numbers[case];
                        match t {
                            Type::Byte => w.uint(x.map_or(101, |x| x as i8 as u8 as u64), 1),
                            Type::Int => w.uint(x.map_or(32_741, |x| x as i16 as u16 as u64), 2),
                            Type::Long => {
                                w.uint(x.map_or(2_147_483_621, |x| x as i32 as u32 as u64), 4)
                            }
                            _ => w.uint(x.map_or(0x7fe0_0000_0000_0000, f64::to_bits), 8),
                        }
                    }
                }
            }
        }
    });
    if layout.tagged {
        part(&mut w, 10, "strls", &|w| {
            for case in 0..cases {
                for (i, c) in columns.iter().enumerate() {
                    if c.t != Type::StrL || c.texts[case].is_empty() {
                        continue;
                    }
                    w.tag("GSO");
                    w.uint(i as u64 + 1, 4);
                    w.uint(case as u64 + 1, if format == 117 { 4 } else { 8 });
                    w.uint(130, 1);
                    w.uint(c.texts[case].len() as u64 + 1, 4);
                    w.out.extend(&c.texts[case]);
                    w.out.push(0);
                }
            }
        });
    }
    part(&mut w, 11, "value_labels", &|w| {
        for c in columns.iter().filter(|c| !c.value_labels.is_empty()) {
            if layout.tagged {
                w.tag("<lbl>");
            }
            let n = c.value_labels.len() as u64;
            let text_length: usize = c.value_labels.iter().map(|(_, t)| t.len() + 1).sum();
            w.uint(8 + 8 * n + text_length as u64, 4);
            w.fixed(c.name.as_bytes(), layout.name);
            w.out.extend([0; 3]);
            w.uint(n, 4);
            w.uint(text_length as u64, 4);
            let mut offset = 0;
            for (_, text) in &c.value_labels {
                w.uint(offset as u64, 4);
                offset += text.len() + 1;
            }
            for (value, _) in &c.value_labels {
                w.uint(*value as u32 as u64, 4);
            }
            for (_, text) in &c.value_labels {
                w.out.extend(text);
                w.out.push(0);
            }
            if layout.tagged {
                w.tag("</lbl>");
            }
        }
    });
    if layout.tagged {
        map[12] = w.out.len() as u64;
        w.tag("</stata_dta>");
        map[13] = w.out.len() as u64;
        let at = map[1] as usize + "<map>".len();
        let mut entries = Writer {
            out: Vec::new(),
            big: o.big,
        };
        for position in map {
            entries.uint(position, 8);
        }
        w.out[at..at + 14 * 8].copy_from_slice(&entries.out);
    }
    Ok((w.out, warnings))
}

impl Engine {
    pub(crate) fn get_stata(&mut self, command: &GetStata) -> Result<Vec<Table>, Error> {
        let fail = |e: String| Error(format!("GET STATA: {}: {e}", command.file));
        let encoding = match command.encoding.as_deref() {
            None => None,
            Some(name) => {
                Some(stata_encoding(name).ok_or_else(|| fail(format!("unknown encoding {name}")))?)
            }
        };
        let bytes = std::fs::read(self.path(&command.file)).map_err(|e| fail(e.to_string()))?;
        let (data, warnings) = read_dta(&bytes, encoding).map_err(fail)?;
        self.warn_some(warnings);
        self.make_active(data);
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(version: u32) -> DtaOptions {
        DtaOptions {
            version,
            se: true,
            encoding: Some(encoding_rs::WINDOWS_1252),
            recode_missing: false,
            big: false,
        }
    }

    #[test]
    fn names_for_stata_are_valid_and_unique() {
        let names = [
            "income.net",
            "@weight",
            "Int",
            "int",
            "str12",
            "pikkuse_mõõt",
        ];
        assert_eq!(
            stata_names(names, true),
            [
                "income_net",
                "_weight",
                "Int",
                "int_",
                "str12_",
                "pikkuse_mõõt"
            ]
        );
        assert_eq!(stata_names(names, false)[5], "v6");
        let long = "a".repeat(40);
        let longer = format!("{long}b");
        assert_eq!(
            stata_names([long.as_str(), longer.as_str()], true),
            ["a".repeat(32), format!("{}_2", "a".repeat(30))]
        );
    }

    #[test]
    fn stata_names_are_made_valid_here() {
        let names: Vec<String> = ["_merge", "to", "Region", "region", "REGION", "ok"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            our_names(&names),
            ["V_merge", "to$", "Region", "region_A", "REGION_B", "ok"]
        );
    }

    #[test]
    fn display_formats_map_to_ours() {
        assert_eq!(our_format("%td", Type::Long, true), ("DATE11".into(), 1));
        assert_eq!(
            our_format("%dD_m_Y", Type::Long, true),
            ("DATE11".into(), 1)
        );
        assert_eq!(our_format("%tcDDmonCCYY", Type::Double, true).1, 2);
        assert_eq!(our_format("%tm", Type::Int, true), ("F8.0".into(), 0));
        assert_eq!(our_format("%9.2f", Type::Double, false).0, "F9.2");
        assert_eq!(our_format("%12,0fc", Type::Double, true).0, "COMMA12.0");
        assert_eq!(our_format("%10.0g", Type::Double, false).0, "F10.2");
        assert_eq!(our_format("%8.0g", Type::Byte, true).0, "F8.0");
        assert_eq!(our_format("%10.3e", Type::Double, false).0, "E10.3");
    }

    #[test]
    fn missing_codes_of_every_type() {
        assert_eq!(number(Type::Byte, &[100], false), Some(100.0));
        assert_eq!(number(Type::Byte, &[101], false), None);
        assert_eq!(number(Type::Byte, &[0x81], false), Some(-127.0));
        assert_eq!(number(Type::Int, &[0x7f, 0xe4], true), Some(32_740.0));
        assert_eq!(number(Type::Int, &[0xe6, 0x7f], false), None);
        assert_eq!(number(Type::Long, &[0x7f, 0xff, 0xff, 0xe5], true), None);
        assert_eq!(number(Type::Float, &[0x7f, 0x00, 0x08, 0x00], true), None);
        assert_eq!(
            number(Type::Float, &[0x40, 0x49, 0x0f, 0xdb], true),
            Some(f64::from(std::f32::consts::PI))
        );
        let dot_b = 0x7fe0_0200_0000_0000u64.to_le_bytes();
        assert_eq!(number(Type::Double, &dot_b, false), None);
        assert_eq!(
            number(Type::Double, &(-2.5f64).to_bits().to_be_bytes(), true),
            Some(-2.5)
        );
    }

    fn sample() -> Dataset {
        let mut d = Dataset::new(4);
        d.set_numeric_values("id", vec![Some(1.0), Some(2.0), Some(3.0), Some(40_000.0)])
            .unwrap();
        d.set_numeric_values("score", vec![Some(2.5), None, Some(-1.25), Some(7.0)])
            .unwrap();
        d.set_numeric_values("rating", vec![Some(1.0), Some(2.0), Some(9.0), None])
            .unwrap();
        d.set_value_labels(
            "rating",
            vec![
                (Value::Number(1.0), "poor".into()),
                (Value::Number(2.0), "fine".into()),
                (Value::Number(1.5), "between".into()),
            ],
        )
        .unwrap();
        d.set_variable_label("rating", Some("Rating of the course"))
            .unwrap();
        // 2021-06-30 and 1959-12-31, as seconds since 1582-10-14
        let day = |days: i64| Some(((days + DAYS_TO_1960) * 86_400) as f64);
        d.set_numeric_values("visit", vec![day(22_461), day(-1), None, day(0)])
            .unwrap();
        d.set_format("visit", "ADATE10").unwrap();
        let moment = Some(SECONDS_TO_1960 + 86_400.0 * 400.0 + 3_723.0);
        d.set_numeric_values("stamp", vec![moment, None, None, moment])
            .unwrap();
        d.set_format("stamp", "DATETIME20").unwrap();
        let texts = ["Tartu", "", "Pärnu", "Võru"].map(String::from).to_vec();
        d.set_string_values("town", 12, texts).unwrap();
        d.set_file_label(Some("Course feedback, spring wave"));
        d
    }

    fn round_trip(data: &Dataset, o: &DtaOptions) -> Dataset {
        let (bytes, _) = write_dta(data, o).unwrap();
        read_dta(&bytes, o.encoding).unwrap().0
    }

    #[test]
    fn every_version_and_byte_order_round_trips() {
        let data = sample();
        for version in [8, 10, 12, 13, 14] {
            for big in [false, true] {
                let o = DtaOptions {
                    big,
                    ..options(version)
                };
                let back = round_trip(&data, &o);
                let numbers = |d: &Dataset, name: &str| -> Vec<Option<f64>> {
                    d.variable(name)
                        .unwrap()
                        .numeric()
                        .unwrap()
                        .iter()
                        .collect()
                };
                for name in ["id", "score", "rating", "visit"] {
                    assert_eq!(
                        numbers(&back, name),
                        numbers(&data, name),
                        "{name} {version}"
                    );
                }
                let stamp = numbers(&back, "stamp");
                if version > 9 {
                    assert_eq!(stamp, numbers(&data, "stamp"));
                    assert_eq!(back.variable("stamp").unwrap().format(), "DATETIME20");
                } else {
                    // format 113 keeps a date with a time of day as days
                    assert_eq!(back.variable("stamp").unwrap().format(), "DATE11");
                }
                let town: Vec<&str> = back
                    .variable("town")
                    .unwrap()
                    .string()
                    .unwrap()
                    .iter()
                    .collect();
                assert_eq!(town, ["Tartu", "", "Pärnu", "Võru"]);
                assert_eq!(back.variable("visit").unwrap().format(), "DATE11");
                let rating = back.variable("rating").unwrap();
                assert_eq!(rating.label(), Some("Rating of the course"));
                assert_eq!(
                    rating.value_labels(),
                    [
                        (Value::Number(1.0), "poor".into()),
                        (Value::Number(2.0), "fine".into())
                    ]
                );
                assert_eq!(back.file_label(), Some("Course feedback, spring wave"));
            }
        }
    }

    #[test]
    fn storage_types_hold_their_values() {
        let (bytes, _) = write_dta(&sample(), &options(14)).unwrap();
        let start = bytes
            .windows(16)
            .position(|w| w == b"<variable_types>")
            .unwrap()
            + 16;
        let codes: Vec<u16> = bytes[start..start + 12]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        // id needs long, score double, rating byte, visit int (days), stamp double, town str12
        assert_eq!(codes, [65528, 65526, 65530, 65529, 65526, 12]);
    }

    #[test]
    fn editions_limit_strings_and_long_strings_use_strl() {
        let mut d = Dataset::new(2);
        let long = "x".repeat(3000);
        d.set_string_values("note", 3000, vec![long.clone(), "short".into()])
            .unwrap();
        let back = round_trip(&d, &options(14));
        let values: Vec<&str> = back
            .variable("note")
            .unwrap()
            .string()
            .unwrap()
            .iter()
            .collect();
        assert_eq!(values, [long.as_str(), "short"]);
        assert_eq!(back.variable("note").unwrap().string_width(), Some(3000));
        let o = DtaOptions {
            se: false,
            ..options(14)
        };
        let (bytes, warnings) = write_dta(&d, &o).unwrap();
        assert!(warnings[0].contains("cut to 80 bytes"), "{warnings:?}");
        let back = read_dta(&bytes, None).unwrap().0;
        assert_eq!(
            back.variable("note").unwrap().string().unwrap().get(0),
            "x".repeat(80)
        );
        let back = round_trip(&d, &options(12));
        assert_eq!(
            back.variable("note")
                .unwrap()
                .string()
                .unwrap()
                .get(0)
                .len(),
            244
        );
    }

    #[test]
    fn user_missing_values_are_kept_or_recoded() {
        let mut d = Dataset::new(3);
        d.set_numeric_values("answer", vec![Some(1.0), Some(99.0), None])
            .unwrap();
        d.set_missing_values(
            "answer",
            statscraft_data::MissingValues::Numbers(vec![99.0]),
        )
        .unwrap();
        let get = |d: &Dataset| -> Vec<Option<f64>> {
            d.variable("answer")
                .unwrap()
                .numeric()
                .unwrap()
                .iter()
                .collect()
        };
        assert_eq!(
            get(&round_trip(&d, &options(14))),
            [Some(1.0), Some(99.0), None]
        );
        let o = DtaOptions {
            recode_missing: true,
            ..options(14)
        };
        assert_eq!(get(&round_trip(&d, &o)), [Some(1.0), None, None]);
    }

    #[test]
    fn legacy_text_needs_its_encoding() {
        let mut d = Dataset::new(1);
        d.set_string_values("word", 6, vec!["õun".into()]).unwrap();
        let (bytes, _) = write_dta(&d, &options(12)).unwrap();
        assert!(bytes.windows(3).any(|w| w == b"\xf5un"));
        let o = DtaOptions {
            encoding: Some(encoding_rs::WINDOWS_1251),
            ..options(12)
        };
        let error = write_dta(&d, &o).err().unwrap();
        assert!(error.contains("windows-1251"), "{error}");
    }

    /// A format 118 file built byte by byte: a strL shared by two cases, a binary strL, an
    /// extended missing value and a label of it, and a characteristic to skip.
    #[test]
    fn hand_built_118_file_reads() {
        let mut w = Writer {
            out: Vec::new(),
            big: true,
        };
        w.tag("<stata_dta><header><release>118</release><byteorder>MSF</byteorder><K>");
        w.uint(2, 2);
        w.tag("</K><N>");
        w.uint(3, 8);
        w.tag("</N><label>");
        w.uint(0, 2);
        w.tag("</label><timestamp>");
        w.uint(0, 1);
        w.tag("</timestamp></header><map>");
        w.out.extend([0; 112]);
        w.tag("</map><variable_types>");
        w.uint(32768, 2);
        w.uint(65528, 2);
        w.tag("</variable_types><varnames>");
        w.fixed(b"memo", 129);
        w.fixed(b"grade", 129);
        w.tag("</varnames><sortlist>");
        w.out.extend([0; 6]);
        w.tag("</sortlist><formats>");
        w.fixed(b"%9s", 57);
        w.fixed(b"%8.0g", 57);
        w.tag("</formats><value_label_names>");
        w.fixed(b"", 129);
        w.fixed(b"grades", 129);
        w.tag("</value_label_names><variable_labels>");
        w.fixed(b"", 321);
        w.fixed("Hinne".as_bytes(), 321);
        w.tag("</variable_labels><characteristics><ch>");
        w.uint(5, 4);
        w.out.extend(b"</ch>");
        w.tag("</ch></characteristics><data>");
        // case 1: its own strL, grade 4; case 2: case 1's strL, grade .b; case 3: binary, grade 5
        w.uint(1, 2);
        w.uint(1, 6);
        w.uint(4, 4);
        w.uint(1, 2);
        w.uint(1, 6);
        w.uint(2_147_483_623, 4);
        w.uint(1, 2);
        w.uint(3, 6);
        w.uint(5, 4);
        w.tag("</data><strls>GSO");
        w.uint(1, 4);
        w.uint(1, 8);
        w.uint(130, 1);
        w.uint(6, 4);
        w.out.extend(b"kiire\0");
        w.tag("GSO");
        w.uint(1, 4);
        w.uint(3, 8);
        w.uint(129, 1);
        w.uint(3, 4);
        w.out.extend([1, 0, 2]);
        w.tag("</strls><value_labels><lbl>");
        w.uint(8 + 16 + 8, 4);
        w.fixed(b"grades", 129);
        w.out.extend([0; 3]);
        w.uint(2, 4);
        w.uint(8, 4);
        w.uint(0, 4);
        w.uint(5, 4);
        w.uint(4, 4);
        w.uint(2_147_483_623, 4);
        w.out.extend(b"good\0.b\0");
        w.tag("</lbl></value_labels></stata_dta>");
        let (data, warnings) = read_dta(&w.out, None).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let memo: Vec<&str> = data
            .variable("memo")
            .unwrap()
            .string()
            .unwrap()
            .iter()
            .collect();
        assert_eq!(memo, ["kiire", "kiire", ""]);
        let grade = data.variable("grade").unwrap();
        let values: Vec<Option<f64>> = grade.numeric().unwrap().iter().collect();
        assert_eq!(values, [Some(4.0), None, Some(5.0)]);
        assert_eq!(grade.value_labels(), [(Value::Number(4.0), "good".into())]);
        assert_eq!(grade.label(), Some("Hinne"));
        assert_eq!(grade.format(), "F8.0");
    }

    #[test]
    fn damaged_files_give_errors() {
        let (bytes, _) = write_dta(&sample(), &options(14)).unwrap();
        for end in (0..bytes.len()).step_by(7) {
            assert!(read_dta(&bytes[..end], None).is_err(), "cut at {end}");
        }
        let (bytes, _) = write_dta(&sample(), &options(10)).unwrap();
        for end in (0..bytes.len() - 1).step_by(5) {
            // format 114 may end after the data: that keeps all cases, or fails
            let _ = read_dta(&bytes[..end], None);
        }
        assert!(
            read_dta(b"Hello world", None)
                .err()
                .unwrap()
                .contains("not a Stata")
        );
    }
}
