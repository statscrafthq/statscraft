//! Portable (.por) files, for EXPORT and IMPORT.
//!
//! The format, from its public descriptions and from files examined as black boxes: text in
//! lines of 80 characters, a 200-byte banner, a 256-byte table that says which byte of the file
//! stands for each character of the format's own character set, an 8-character tag, then records
//! that each start with a one-character tag. Numbers are written in base 30 (digits 0-9 and A-T)
//! and end with a slash; a string is its length as such a number, then its bytes.
//!
//! Choices:
//! - Text is written as UTF-8. Only control characters are replaced (by `0`, as the manual's
//!   TYPE=COMM does), since line ends in the file carry no meaning. A byte the table does not
//!   name is read as itself, and text that is not valid UTF-8 is read as windows-1252.
//! - Numbers are written with the fewest base-30 digits (at most 12) that give back exactly the
//!   same value, and read with exact arithmetic, so values survive a round trip unchanged.
//! - Names become upper case and at most 8 bytes, unique as the manual shows (MYLONGRO,
//!   MYLONG_2); strings are at most 255 bytes wide. Measurement levels, display settings,
//!   attributes, the file label and multiple response sets have no place in the format.

use crate::sav::{VariableExtras, canonical_format};
use crate::{
    Dataset, Dictionary, Error, Measure, MissingValues, Value, Values, VariableDef, is_valid_name,
};
use arrow::array::{ArrayRef, Float64Array, StringViewArray};
use statscraft_sav::constants::{FormatType, SavFormat};
use std::path::Path;
use std::sync::Arc;

const LINE: usize = 80;
const MAX_STRING: usize = 255;
const MAX_NAME: usize = 8;
/// Base-30 digits a number is written with at most: enough for every double to come back.
const MAX_DIGITS: usize = 12;
/// The tag that follows the table, as codes of the format's character set.
const TAG: [u8; 8] = [92, 89, 92, 92, 89, 88, 91, 93];

/// The ASCII character of each code of the format's character set from 64 on; `0` where ASCII
/// has none (unused codes are written as the character of code 64, which is `0`).
const CODES: &[u8; 125] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz \
.<(+|&[]!$*);^-/0,%_>?`:0@'=\"000000~000000000000000000000{}\\00";

/// The 256-byte table this writer puts in the file: ASCII for every code that has a character.
fn table() -> [u8; 256] {
    let mut table = [b'0'; 256];
    table[64..64 + CODES.len()].copy_from_slice(CODES);
    table
}

fn digit(d: u8) -> u8 {
    if d < 10 { b'0' + d } else { b'A' + d - 10 }
}

fn digit_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'T' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// An unsigned integer of any size, little-endian in 32-bit limbs: what exact conversion
/// between base 30 and binary numbers needs, nothing more.
#[derive(Clone)]
struct Big(Vec<u32>);

impl Big {
    fn new(v: u64) -> Big {
        let mut big = Big(vec![v as u32, (v >> 32) as u32]);
        big.trim();
        big
    }

    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    /// self = self * m + a
    fn mul_add(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for limb in &mut self.0 {
            let v = u64::from(*limb) * u64::from(m) + carry;
            *limb = v as u32;
            carry = v >> 32;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }

    /// self = self / d, giving the remainder.
    fn div_rem(&mut self, d: u32) -> u32 {
        let mut rem = 0u64;
        for limb in self.0.iter_mut().rev() {
            let v = (rem << 32) | u64::from(*limb);
            *limb = (v / u64::from(d)) as u32;
            rem = v % u64::from(d);
        }
        self.trim();
        rem as u32
    }

    fn shl(&mut self, bits: u32) {
        let (words, bits) = ((bits / 32) as usize, bits % 32);
        if bits > 0 {
            let mut carry = 0;
            for limb in &mut self.0 {
                let v = (*limb << bits) | carry;
                carry = *limb >> (32 - bits);
                *limb = v;
            }
            if carry > 0 {
                self.0.push(carry);
            }
        }
        if !self.is_zero() {
            self.0.splice(0..0, std::iter::repeat_n(0, words));
        }
    }

    /// Shifts right by `bits`, telling whether a bit that fell out was set.
    fn shr(&mut self, bits: u32) -> bool {
        let (words, bits) = ((bits / 32) as usize, bits % 32);
        if words >= self.0.len() {
            let lost = !self.is_zero();
            self.0.clear();
            return lost;
        }
        let mut lost = self.0[..words].iter().any(|&l| l != 0);
        self.0.drain(..words);
        if bits > 0 {
            lost |= self.0[0] & ((1 << bits) - 1) != 0;
            for i in 0..self.0.len() {
                let next = self.0.get(i + 1).map_or(0, |&n| n << (32 - bits));
                self.0[i] = (self.0[i] >> bits) | next;
            }
            self.trim();
        }
        lost
    }

    fn bits(&self) -> u32 {
        self.0.last().map_or(0, |&top| {
            32 * (self.0.len() as u32 - 1) + 32 - top.leading_zeros()
        })
    }

    /// The bits from `bits` up, which must be fewer than 32, leaving the bits below it.
    fn split(&mut self, bits: u32) -> u32 {
        let word = (bits / 32) as usize;
        let mut high = self.clone();
        high.shr(bits);
        self.0.truncate(word + 1);
        if let Some(limb) = self.0.get_mut(word) {
            *limb &= (1u32 << (bits % 32)) - 1;
        }
        self.trim();
        high.0.first().copied().unwrap_or(0)
    }

    fn to_u64(&self) -> u64 {
        self.0
            .iter()
            .take(2)
            .enumerate()
            .map(|(i, &l)| u64::from(l) << (32 * i))
            .sum()
    }
}

/// `x * 2^e`, exact while the result is a normal number.
fn ldexp(mut x: f64, mut e: i64) -> f64 {
    while e > 1000 {
        x *= 2f64.powi(1000);
        e -= 1000;
    }
    while e < -1000 {
        x *= 2f64.powi(-1000);
        e += 1000;
    }
    x * 2f64.powi(e as i32)
}

/// The double nearest to `(m + d) * 2^e`, where `d` is in [0, 1) and nonzero only when
/// `sticky`.
fn to_double(mut m: Big, mut sticky: bool, e: i64) -> f64 {
    let n = m.bits();
    if n == 0 {
        return 0.0;
    }
    // 54 bits: the 53 of a double and one to round with
    let shift = i64::from(n) - 54;
    if shift > 0 {
        sticky |= m.shr(shift as u32);
    } else {
        m.shl((-shift) as u32);
    }
    let q = m.to_u64();
    let mut mantissa = q >> 1;
    if q & 1 == 1 && (sticky || mantissa & 1 == 1) {
        mantissa += 1;
    }
    ldexp(mantissa as f64, e + shift + 1)
}

/// The value of base-30 `digits` times 30 to the power `exponent`, correctly rounded.
fn from_base30(digits: &[u8], exponent: i64) -> f64 {
    let mut m = Big::new(0);
    for &d in digits {
        m.mul_add(30, u32::from(d));
    }
    if m.is_zero() {
        return 0.0;
    }
    let significant = digits.iter().skip_while(|&&d| d == 0).count() as i64;
    if exponent + significant > 215 {
        return f64::INFINITY;
    }
    if exponent + significant < -230 {
        return 0.0;
    }
    if exponent >= 0 {
        for _ in 0..exponent {
            m.mul_add(30, 0);
        }
        return to_double(m, false, 0);
    }
    let k = -exponent;
    // enough bits that the quotient keeps more than 54 of them
    let bits = 5 * k + 64;
    m.shl(bits as u32);
    let mut sticky = false;
    for _ in 0..k {
        sticky |= m.div_rem(30) != 0;
    }
    to_double(m, sticky, -bits)
}

/// The first `limit` significant base-30 digits of `x` (positive, finite), the power of 30 of
/// the first one (the value is 0.d1d2... times 30^point), and whether any digit after them is
/// not zero.
fn base30_digits(x: f64, limit: usize) -> (Vec<u8>, i64, bool) {
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (m, e) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    let (mut whole, mut part, k) = if e >= 0 {
        let mut whole = Big::new(m);
        whole.shl(e as u32);
        (whole, Big::new(0), 0)
    } else {
        let k = (-e) as u32;
        let mut whole = Big::new(m);
        whole.shr(k);
        // what is left below the point, over 2^k
        let low = if k < 64 { m & ((1u64 << k) - 1) } else { m };
        (whole, Big::new(low), k)
    };
    let mut reversed = Vec::new();
    while !whole.is_zero() {
        reversed.push(whole.div_rem(30) as u8);
    }
    let mut point = reversed.len() as i64;
    let mut digits: Vec<u8> = reversed.into_iter().rev().collect();
    let mut sticky = digits.len() > limit && digits[limit..].iter().any(|&d| d != 0);
    digits.truncate(limit);
    while digits.len() < limit && !part.is_zero() {
        part.mul_add(30, 0);
        let d = part.split(k) as u8;
        if digits.is_empty() && d == 0 {
            point -= 1;
        } else {
            digits.push(d);
        }
    }
    sticky |= !part.is_zero();
    (digits, point, sticky)
}

/// `digits` rounded to `n` significant digits, without trailing zeros, and the new point.
fn round_digits(digits: &[u8], point: i64, sticky: bool, n: usize) -> (Vec<u8>, i64) {
    let mut kept = digits[..n.min(digits.len())].to_vec();
    let mut point = point;
    let beyond = sticky || digits.iter().skip(n + 1).any(|&d| d > 0);
    // half way rounds to even, which in base 30 is the parity of the last digit
    let up = match digits.get(n) {
        Some(&d) if d > 15 => true,
        Some(15) => beyond || kept.last().is_some_and(|d| d % 2 == 1),
        _ => false,
    };
    if up {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, 1);
                point += 1;
                break;
            }
            i -= 1;
            if kept[i] == 29 {
                kept[i] = 0;
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    while kept.last() == Some(&0) {
        kept.pop();
    }
    (kept, point)
}

fn push_integer(out: &mut Vec<u8>, mut n: u64) {
    let mut reversed = Vec::new();
    loop {
        reversed.push(digit((n % 30) as u8));
        n /= 30;
        if n == 0 {
            break;
        }
    }
    out.extend(reversed.iter().rev());
}

/// A number field: base 30, as short as gives `x` back exactly (or `decimals` significant
/// decimal digits of a value that is not whole), ending with a slash.
fn push_number(out: &mut Vec<u8>, x: f64, decimals: Option<u32>) {
    if !x.is_finite() {
        out.extend(b"*.");
        return;
    }
    if x < 0.0 {
        out.push(b'-');
    }
    let mut x = x.abs();
    if x.fract() == 0.0 && x < 9.0e15 {
        push_integer(out, x as u64);
        out.push(b'/');
        return;
    }
    if let Some(n) = decimals.filter(|_| x.fract() != 0.0) {
        x = format!("{:.*e}", n.saturating_sub(1) as usize, x)
            .parse()
            .unwrap_or(x);
    }
    let (digits, point, sticky) = base30_digits(x, MAX_DIGITS + 1);
    let (digits, point) = (1..=MAX_DIGITS)
        .map(|n| round_digits(&digits, point, sticky, n))
        .find(|(d, p)| from_base30(d, p - d.len() as i64) == x)
        .unwrap_or_else(|| round_digits(&digits, point, sticky, MAX_DIGITS));
    let n = digits.len() as i64;
    let text = |ds: &[u8]| ds.iter().map(|&d| digit(d)).collect::<Vec<u8>>();
    if point >= n && point - n <= 3 {
        out.extend(text(&digits));
        out.extend(std::iter::repeat_n(b'0', (point - n) as usize));
    } else if point > 0 && point < n {
        out.extend(text(&digits[..point as usize]));
        out.push(b'.');
        out.extend(text(&digits[point as usize..]));
    } else if (-3..=0).contains(&point) {
        out.push(b'.');
        out.extend(std::iter::repeat_n(b'0', (-point) as usize));
        out.extend(text(&digits));
    } else {
        // the digits as a whole number times a power of 30
        out.extend(text(&digits));
        out.push(if point > n { b'+' } else { b'-' });
        push_integer(out, (point - n).unsigned_abs());
    }
    out.push(b'/');
}

/// A string field: the length, then the bytes, control characters replaced by `0`. An empty
/// text is one blank, since GNU PSPP misreads a string of length 0.
fn push_string(out: &mut Vec<u8>, text: &str) {
    let text = if text.is_empty() { " " } else { text };
    push_integer(out, text.len() as u64);
    out.push(b'/');
    out.extend(
        text.bytes()
            .map(|b| if b < 0x20 || b == 0x7f { b'0' } else { b }),
    );
}

/// The names the variables get in a portable file: upper case, at most 8 bytes and unique.
/// Characters the format has no letter for become `_`, a name that would not start with a
/// letter is V and its position, and a name cut short gets `_2`, `_3` and so on when its first
/// 8 bytes are taken already.
pub fn portable_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    for (position, name) in names.into_iter().enumerate() {
        let mut clean: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "@$_.#".contains(c) {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        if !clean.starts_with(|c: char| c.is_ascii_alphabetic() || c == '@') {
            clean = format!("V{}", position + 1);
        }
        let cut = |n: usize| {
            clean[..n.min(clean.len())]
                .trim_end_matches('.')
                .to_string()
        };
        let mut short = cut(MAX_NAME);
        let mut n = 2;
        while taken.contains(&short) {
            let suffix = format!("_{n}");
            short = format!("{}{suffix}", cut(MAX_NAME - suffix.len()));
            n += 1;
        }
        taken.push(short);
    }
    taken
}

impl Dataset {
    /// Writes a portable file. `digits` limits values that are not whole to that many
    /// significant decimal digits (EXPORT /DIGITS). Returns warnings about what the format
    /// cannot hold: shortened names and strings cut to 255 bytes. An existing file is replaced
    /// only after the new one is complete.
    pub fn write_por(
        &self,
        path: impl AsRef<Path>,
        digits: Option<u32>,
    ) -> Result<Vec<String>, Error> {
        if let Some(scratch) = self.variable_names().find(|name| name.starts_with('#')) {
            return Err(Error::Invalid(format!(
                "{scratch} is a scratch variable, which a data file cannot hold"
            )));
        }
        let path = path.as_ref();
        let names = portable_names(self.variable_names());
        let mut warnings = Vec::new();
        let renamed: Vec<String> = self
            .variable_names()
            .zip(&names)
            .filter(|(old, new)| old.len() > MAX_NAME && !old.eq_ignore_ascii_case(new))
            .map(|(old, new)| format!("{old} as {new}"))
            .collect();
        if !renamed.is_empty() {
            warnings.push(format!(
                "a portable file holds names of at most 8 bytes: {} in the file",
                renamed.join(", ")
            ));
        }
        // strings are padded to the width, as GNU PSPP misreads shorter ones
        let padded = |s: &str, width: usize| {
            let width = width.min(MAX_STRING);
            let s = crate::cut_to(s, width);
            s.to_string() + &" ".repeat(width - s.len())
        };

        let mut body = Vec::new();
        let now = chrono::Local::now();
        body.push(b'A');
        push_string(&mut body, &now.format("%Y%m%d").to_string());
        push_string(&mut body, &now.format("%H%M%S").to_string());
        body.push(b'1');
        push_string(
            &mut body,
            &format!("Statscraft {}", env!("CARGO_PKG_VERSION")),
        );
        body.push(b'4');
        push_integer(&mut body, self.variable_count() as u64);
        body.push(b'/');
        body.push(b'5');
        push_integer(&mut body, MAX_DIGITS as u64);
        body.push(b'/');
        if let Some(weight) = self.weight_variable().and_then(|w| self.variable_index(w)) {
            body.push(b'6');
            push_string(&mut body, &names[weight]);
        }
        for (variable, name) in self.variables().zip(&names) {
            let def = variable.def;
            body.push(b'7');
            let width = def.width.map_or(0, |w| w.min(MAX_STRING));
            if def.width.is_some_and(|w| w > MAX_STRING) {
                warnings.push(format!(
                    "{}: a portable file holds strings of at most {MAX_STRING} bytes; longer \
                     values are cut",
                    def.name
                ));
            }
            push_integer(&mut body, width as u64);
            body.push(b'/');
            push_string(&mut body, name);
            for format in [def.format.as_str(), variable.write_format()] {
                let parsed = SavFormat::from_string(format).unwrap_or(SavFormat {
                    format_type: FormatType::F,
                    width: 8,
                    decimals: 2,
                });
                let (kind, width, decimals) = match def.width {
                    Some(_) => (FormatType::A as u64, width as u64, 0),
                    None => (
                        parsed.format_type as u64,
                        u64::from(parsed.width),
                        u64::from(parsed.decimals),
                    ),
                };
                for n in [kind, width, decimals] {
                    push_integer(&mut body, n);
                    body.push(b'/');
                }
            }
            match &def.missing {
                MissingValues::None => {}
                MissingValues::Numbers(values) => {
                    for v in values {
                        body.push(b'8');
                        push_number(&mut body, *v, None);
                    }
                }
                MissingValues::Range { low, high, extra } => {
                    if *low < -1e308 {
                        body.push(b'9');
                        push_number(&mut body, *high, None);
                    } else if *high > 1e308 {
                        body.push(b'A');
                        push_number(&mut body, *low, None);
                    } else {
                        body.push(b'B');
                        push_number(&mut body, *low, None);
                        push_number(&mut body, *high, None);
                    }
                    if let Some(v) = extra {
                        body.push(b'8');
                        push_number(&mut body, *v, None);
                    }
                }
                MissingValues::Strings(values) => {
                    for v in values {
                        body.push(b'8');
                        push_string(&mut body, &padded(v, width.min(8)));
                    }
                }
            }
            if let Some(label) = &def.label {
                body.push(b'C');
                push_string(&mut body, label);
            }
        }
        for (def, name) in self.dict.variables.iter().zip(&names) {
            if def.value_labels.is_empty() {
                continue;
            }
            body.push(b'D');
            push_integer(&mut body, 1);
            body.push(b'/');
            push_string(&mut body, name);
            push_integer(&mut body, def.value_labels.len() as u64);
            body.push(b'/');
            for (value, label) in &def.value_labels {
                match value {
                    Value::Number(v) => push_number(&mut body, *v, None),
                    Value::String(s) => push_string(&mut body, &padded(s, def.width.unwrap_or(1))),
                }
                push_string(&mut body, label);
            }
        }
        if !self.dict.documents.is_empty() {
            body.push(b'E');
            push_integer(&mut body, self.dict.documents.len() as u64);
            body.push(b'/');
            for line in &self.dict.documents {
                push_string(&mut body, crate::cut_to(line, LINE));
            }
        }
        body.push(b'F');
        let columns: Vec<(Values, usize)> = self
            .variables()
            .map(|v| (v.values(), v.string_width().unwrap_or(0)))
            .collect();
        for case in 0..self.case_count {
            for (values, width) in &columns {
                match values {
                    Values::Numeric(v) => match v.get(case) {
                        Some(x) => push_number(&mut body, x, digits),
                        None => body.extend(b"*."),
                    },
                    Values::String(v) => push_string(&mut body, &padded(v.get(case), *width)),
                }
            }
        }
        body.push(b'Z');

        let table = table();
        let mut stream = Vec::with_capacity(464 + body.len());
        for _ in 0..5 {
            stream.extend(format!("{:<40}", "ASCII STATSCRAFT PORT FILE").bytes());
        }
        stream.extend(table);
        stream.extend(TAG.iter().map(|&code| table[usize::from(code)]));
        stream.extend(body);
        let padding = (LINE - stream.len() % LINE) % LINE;
        stream.extend(std::iter::repeat_n(b'Z', padding));
        let mut bytes = Vec::with_capacity(stream.len() / LINE * (LINE + 2));
        for line in stream.chunks(LINE) {
            bytes.extend(line);
            bytes.extend(b"\r\n");
        }

        let file_error = |e: String| Error::File(format!("{}: {e}", path.display()));
        let mut partial = path.as_os_str().to_owned();
        partial.push(format!(".{}.partial", std::process::id()));
        std::fs::write(&partial, bytes)
            .and_then(|()| std::fs::rename(&partial, path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&partial);
                file_error(e.to_string())
            })?;
        Ok(warnings)
    }

    /// Reads a portable file: its variables, labels, missing values, formats, weight and
    /// documents, and its cases.
    pub fn read_por(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes =
            std::fs::read(path).map_err(|e| Error::File(format!("{}: {e}", path.display())))?;
        parse(&bytes).map_err(|e| Error::File(format!("{}: {e}", path.display())))
    }
}

struct Reader {
    bytes: Vec<u8>,
    pos: usize,
}

impl Reader {
    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!(
            "not a valid portable file: {what} at byte {}",
            self.pos
        ))
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek() == Some(b' ') {
            self.pos += 1;
        }
    }

    /// A number field; None is the missing value.
    fn number(&mut self) -> Result<Option<f64>, String> {
        self.skip_spaces();
        if self.peek() == Some(b'*') {
            self.pos += 2;
            return Ok(None);
        }
        let negative = self.peek() == Some(b'-');
        if negative {
            self.pos += 1;
        }
        let mut digits = Vec::new();
        let mut fraction = 0i64;
        let mut point = false;
        while let Some(c) = self.peek() {
            match (c, digit_value(c)) {
                (_, Some(d)) => {
                    digits.push(d);
                    fraction += i64::from(point);
                }
                (b'.', None) if !point => point = true,
                _ => break,
            }
            self.pos += 1;
            if digits.len() > 200 {
                return self.fail("a number with too many digits");
            }
        }
        if digits.is_empty() {
            return self.fail("a number expected");
        }
        let mut exponent = 0i64;
        if let Some(sign @ (b'+' | b'-')) = self.peek() {
            self.pos += 1;
            let start = self.pos;
            while let Some(d) = self.peek().and_then(digit_value) {
                exponent = (exponent * 30 + i64::from(d)).min(100_000);
                self.pos += 1;
            }
            if self.pos == start {
                return self.fail("an exponent expected");
            }
            if sign == b'-' {
                exponent = -exponent;
            }
        }
        if self.peek() != Some(b'/') {
            return self.fail("a number must end with '/'");
        }
        self.pos += 1;
        let value = from_base30(&digits, exponent - fraction);
        if !value.is_finite() {
            return self.fail("a number too large");
        }
        Ok(Some(if negative { -value } else { value }))
    }

    fn integer(&mut self) -> Result<usize, String> {
        match self.number()? {
            Some(v) if v.fract() == 0.0 && (0.0..=1e9).contains(&v) => Ok(v as usize),
            _ => self.fail("a whole number expected"),
        }
    }

    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.integer()?;
        if self.pos + n > self.bytes.len() {
            return self.fail("a string runs past the end of the file");
        }
        let text = self.bytes[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(text)
    }

    fn string(&mut self) -> Result<String, String> {
        let bytes = self.bytes()?;
        Ok(decode(bytes))
    }

    fn value(&mut self, string: bool) -> Result<Option<Value>, String> {
        Ok(if string {
            Some(Value::String(self.string()?.trim_end().to_string()))
        } else {
            self.number()?.map(Value::Number)
        })
    }

    fn format(&mut self) -> Result<(usize, usize, usize), String> {
        Ok((self.integer()?, self.integer()?, self.integer()?))
    }
}

fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|e| {
        let bytes = e.into_bytes();
        let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
        text.into_owned()
    })
}

/// A variable as the file declares it, before its values are read.
struct Declared {
    def: VariableDef,
    file_name: String,
}

fn parse(raw: &[u8]) -> Result<Dataset, String> {
    // lines are 80 characters; a shorter one stands for one padded with spaces
    let mut stream = Vec::with_capacity(raw.len());
    let lines: Vec<&[u8]> = raw.split(|&b| b == b'\n').collect();
    for (i, line) in lines.iter().enumerate() {
        let mut line = *line;
        while let [rest @ .., b'\r'] = line {
            line = rest;
        }
        if i + 1 == lines.len() && line.is_empty() {
            break;
        }
        stream.extend(line);
        stream.extend(std::iter::repeat_n(b' ', LINE.saturating_sub(line.len())));
    }
    if stream.len() < 464 {
        return Err("not a portable file: too short".into());
    }
    // the file's byte for each code, read back: digits win over the codes that also show as 0
    let mut code_of = [None; 256];
    for code in (64..256).chain(0..64) {
        let byte = usize::from(stream[200 + code]);
        code_of[byte] = code_of[byte].or(Some(code));
    }
    let tag = &stream[456..464];
    if tag
        .iter()
        .zip(TAG)
        .any(|(&b, code)| code_of[usize::from(b)] != Some(usize::from(code)))
    {
        return Err("not a portable file: its tag is missing".into());
    }
    // the rest in ASCII; a byte the table does not name stays as it is
    let bytes = stream[464..]
        .iter()
        .map(|&b| match code_of[usize::from(b)] {
            Some(code) if code >= 64 && code - 64 < CODES.len() && CODES[code - 64] != b'0' => {
                CODES[code - 64]
            }
            Some(64) => b'0',
            _ => b,
        })
        .collect();
    let mut r = Reader { bytes, pos: 0 };
    if r.peek() != Some(b'A') {
        return r.fail("unknown version");
    }
    r.pos += 1;
    r.bytes()?;
    r.bytes()?;

    let mut variables: Vec<Declared> = Vec::new();
    let mut weight = None;
    let mut documents = Vec::new();
    let mut warnings = Vec::new();
    loop {
        let Some(tag) = r.peek() else {
            return r.fail("the data are missing");
        };
        r.pos += 1;
        match tag {
            b'1' | b'2' | b'3' => {
                r.bytes()?;
            }
            b'4' | b'5' => {
                r.integer()?;
            }
            b'6' => weight = Some(r.string()?),
            b'7' => {
                let width = r.integer()?;
                if width > 32767 {
                    return r.fail("a string wider than 32767 bytes");
                }
                let file_name = r.string()?;
                let print = r.format()?;
                let write = r.format()?;
                let string = width > 0;
                let mut def = VariableDef {
                    name: String::new(),
                    label: None,
                    format: String::new(),
                    width: string.then_some(width),
                    measure: Measure::Unknown,
                    value_labels: Vec::new(),
                    missing: MissingValues::None,
                    extras: VariableExtras::default(),
                };
                def.format = file_format(print, &def);
                let write = file_format(write, &def);
                def.extras.write_format = (write != def.format).then_some(write);
                variables.push(Declared { def, file_name });
            }
            b'8' | b'9' | b'A' | b'B' => {
                let Some(last) = variables.last_mut() else {
                    return r.fail("a missing value before the first variable");
                };
                let string = last.def.width.is_some();
                let first = r.value(string)?;
                let second = if tag == b'B' { r.value(string)? } else { None };
                add_missing(&mut last.def, tag, first, second, &mut warnings);
            }
            b'C' => {
                let label = r.string()?;
                match variables.last_mut() {
                    Some(last) => last.def.label = Some(label).filter(|l| !l.is_empty()),
                    None => return r.fail("a variable label before the first variable"),
                }
            }
            b'D' => {
                let count = r.integer()?;
                let mut targets = Vec::with_capacity(count.min(1000));
                for _ in 0..count {
                    let name = r.string()?;
                    match variables.iter().position(|v| v.file_name == name) {
                        Some(i) => targets.push(i),
                        None => {
                            return r.fail(&format!("value labels for unknown variable {name}"));
                        }
                    }
                }
                let string = targets
                    .first()
                    .is_some_and(|&i| variables[i].def.width.is_some());
                let labels = r.integer()?;
                for _ in 0..labels {
                    let value = r.value(string)?;
                    let label = r.string()?;
                    let Some(value) = value else { continue };
                    for &i in &targets {
                        let def = &mut variables[i].def;
                        if def.width.is_some() != string {
                            continue;
                        }
                        // a value labelled twice keeps the later label
                        match def.value_labels.iter_mut().find(|(v, _)| *v == value) {
                            Some(entry) => entry.1 = label.clone(),
                            None => def.value_labels.push((value.clone(), label.clone())),
                        }
                    }
                }
            }
            b'E' => {
                let count = r.integer()?;
                for _ in 0..count {
                    documents.push(r.string()?.trim_end().to_string());
                }
            }
            b'F' => break,
            other => {
                r.pos -= 1;
                return r.fail(&format!("unknown record '{}'", char::from(other)));
            }
        }
    }

    // the values, case after case, until Z
    let mut numbers: Vec<Vec<Option<f64>>> = variables.iter().map(|_| Vec::new()).collect();
    let mut strings: Vec<Vec<String>> = variables.iter().map(|_| Vec::new()).collect();
    let mut cases = 0;
    'cases: loop {
        r.skip_spaces();
        if matches!(r.peek(), None | Some(b'Z')) || variables.is_empty() {
            break;
        }
        for (i, variable) in variables.iter().enumerate() {
            r.skip_spaces();
            if matches!(r.peek(), None | Some(b'Z')) {
                warnings.push(format!(
                    "the file ends inside case {}, which is left out",
                    cases + 1
                ));
                for column in &mut numbers {
                    column.truncate(cases);
                }
                for column in &mut strings {
                    column.truncate(cases);
                }
                break 'cases;
            }
            if variable.def.width.is_some() {
                strings[i].push(r.string()?);
            } else {
                numbers[i].push(r.number()?);
            }
        }
        cases += 1;
    }

    // names the dictionary can use, unique
    let mut defs: Vec<VariableDef> = Vec::with_capacity(variables.len());
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(variables.len());
    for (i, mut variable) in variables.into_iter().enumerate() {
        let mut name = variable.file_name.clone();
        if !is_valid_name(&name) {
            name = format!("V{}", i + 1);
        }
        let base = name.clone();
        let mut n = 1;
        while defs.iter().any(|d| crate::same_name(&d.name, &name)) {
            name = format!("{base}_{n}");
            n += 1;
        }
        if name != variable.file_name {
            warnings.push(format!("variable {} is read as {name}", variable.file_name));
        }
        if weight.as_deref() == Some(variable.file_name.as_str()) {
            weight = Some(name.clone());
        }
        variable.def.name = name;
        let def = &mut variable.def;
        let column: ArrayRef = match def.width {
            None => Arc::new(Float64Array::from(std::mem::take(&mut numbers[i]))),
            Some(width) => {
                let values: Vec<String> = std::mem::take(&mut strings[i])
                    .into_iter()
                    .map(|s| s.trim_end_matches(' ').to_string())
                    .collect();
                // text read as windows-1252 may need more bytes than the file gave it
                let longest = values.iter().map(String::len).max().unwrap_or(0);
                if longest > width {
                    def.width = Some(longest);
                    def.format = format!("A{longest}");
                    def.extras.write_format = None;
                }
                Arc::new(StringViewArray::from_iter_values(values))
            }
        };
        defs.push(variable.def);
        columns.push(column);
    }
    let weight = weight.filter(|w| defs.iter().any(|d| d.name == *w && d.width.is_none()));
    Ok(Dataset {
        columns,
        case_count: cases,
        dict: Arc::new(Dictionary {
            encoding: "UTF-8".to_string(),
            compression: "none",
            file_label: None,
            weight_variable: weight,
            split: None,
            filter_variable: None,
            variables: defs,
            multiple_response_sets: Vec::new(),
            variable_sets: Vec::new(),
            attributes: Vec::new(),
            warnings,
            documents,
        }),
    })
}

/// The format a file declares as text, or the usual format of the type when it does not fit.
fn file_format((kind, width, decimals): (usize, usize, usize), def: &VariableDef) -> String {
    if let Some(width) = def.width {
        return format!("A{width}");
    }
    let format = u8::try_from(kind)
        .ok()
        .and_then(FormatType::from_u8)
        .filter(|kind| !kind.is_string())
        .zip(u8::try_from(width).ok())
        .zip(u8::try_from(decimals).ok())
        .map(|((format_type, width), decimals)| {
            SavFormat {
                format_type,
                width,
                decimals,
            }
            .to_format_string()
        });
    format
        .and_then(|f| canonical_format(&f, def).ok())
        .unwrap_or_else(|| "F8.2".to_string())
}

fn add_missing(
    def: &mut VariableDef,
    tag: u8,
    first: Option<Value>,
    second: Option<Value>,
    warnings: &mut Vec<String>,
) {
    let number = |v: &Option<Value>| match v {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    };
    let missing = std::mem::take(&mut def.missing);
    def.missing = match (tag, missing, &first) {
        (b'8', MissingValues::None, Some(Value::String(s))) => {
            MissingValues::Strings(vec![s.clone()])
        }
        (b'8', MissingValues::Strings(mut v), Some(Value::String(s))) if v.len() < 3 => {
            v.push(s.clone());
            MissingValues::Strings(v)
        }
        (b'8', MissingValues::None, Some(Value::Number(n))) => MissingValues::Numbers(vec![*n]),
        (b'8', MissingValues::Numbers(mut v), Some(Value::Number(n))) if v.len() < 3 => {
            v.push(*n);
            MissingValues::Numbers(v)
        }
        (
            b'8',
            MissingValues::Range {
                low,
                high,
                extra: None,
            },
            Some(Value::Number(n)),
        ) => MissingValues::Range {
            low,
            high,
            extra: Some(*n),
        },
        (
            b'9' | b'A' | b'B',
            MissingValues::None | MissingValues::Numbers(_),
            Some(Value::Number(n)),
        ) => {
            let (low, high) = match tag {
                b'9' => (f64::MIN, *n),
                b'A' => (*n, f64::MAX),
                _ => match number(&second) {
                    Some(high) => (*n, high),
                    None => (*n, *n),
                },
            };
            MissingValues::Range {
                low,
                high,
                extra: None,
            }
        }
        (_, missing, _) => {
            warnings.push(format!(
                "a missing value of {} that the file cannot combine is left out",
                def.name
            ));
            missing
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(x: f64) -> String {
        let mut out = Vec::new();
        push_number(&mut out, x, None);
        String::from_utf8(out).unwrap()
    }

    fn read_number(text: &str) -> Option<f64> {
        let mut r = Reader {
            bytes: text.as_bytes().to_vec(),
            pos: 0,
        };
        r.number().unwrap()
    }

    #[test]
    fn numbers_are_short_base_30_and_come_back_exactly() {
        assert_eq!(number(0.0), "0/");
        assert_eq!(number(3.0), "3/");
        assert_eq!(number(-30.0), "-10/");
        assert_eq!(number(3.25), "3.7F/");
        assert_eq!(number(0.1), ".3/");
        assert_eq!(number(-7.5), "-7.F/");
        // exact, where a rounded "6850QA888H+4" would read back as another number
        assert_eq!(number(1e20), "6850QA888GO000/");
        assert_eq!(read_number("IRNI+3/"), Some(13_797_216_000.0));
        assert_eq!(read_number(" *."), None);
        let mut x = 1.0e-300_f64;
        let samples = [
            1.0 / 3.0,
            0.333333333,
            12.34567890876,
            f64::MAX,
            f64::MIN_POSITIVE,
            5e-324,
            123456789012345680.0,
            2f64.powi(53) + 2.0,
            1e-5,
            -987.654321,
        ];
        for value in samples {
            let text = number(value);
            assert_eq!(read_number(&text), Some(value), "{value} as {text}");
        }
        while x < 1e300 {
            let text = number(x);
            assert_eq!(read_number(&text), Some(x), "{x} as {text}");
            x *= 7.389;
        }
    }

    #[test]
    fn digits_round_values_that_are_not_whole() {
        let mut out = Vec::new();
        push_number(&mut out, 12.34567890876, Some(4));
        push_number(&mut out, 123456.0, Some(4));
        let text = String::from_utf8(out).unwrap();
        let mut r = Reader {
            bytes: text.into_bytes(),
            pos: 0,
        };
        assert_eq!(r.number().unwrap(), Some(12.35));
        assert_eq!(r.number().unwrap(), Some(123456.0));
    }

    #[test]
    fn names_are_short_upper_case_and_unique() {
        let names = portable_names([
            "mylongrootname1",
            "mylongrootname2",
            "mylongrootname3",
            "id",
            "ID",
            "vanus_õ",
            "õun",
        ]);
        assert_eq!(
            names,
            [
                "MYLONGRO", "MYLONG_2", "MYLONG_3", "ID", "ID_2", "VANUS__", "V7"
            ]
        );
    }
}
