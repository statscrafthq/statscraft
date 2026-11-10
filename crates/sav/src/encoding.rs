use std::borrow::Cow;

use encoding_rs::Encoding;

use crate::error::{Result, SavError};

/// Windows code page numbers (the character code of the integer info record)
/// and their encodings. Where one code page stands for another (an approximation),
/// the entry says so; such entries are skipped when looking up a number for writing.
static CODE_PAGES: &[(i32, &Encoding, bool)] = &[
    (437, encoding_rs::CP866, true),       // approximate: DOS US
    (850, encoding_rs::WINDOWS_1252, true), // approximate: DOS Western European
    (874, encoding_rs::WINDOWS_874, false),
    (932, encoding_rs::SHIFT_JIS, false),
    (936, encoding_rs::GBK, false),
    (949, encoding_rs::EUC_KR, false),
    (950, encoding_rs::BIG5, false),
    (1200, encoding_rs::UTF_16LE, false),
    (1201, encoding_rs::UTF_16BE, false),
    (1250, encoding_rs::WINDOWS_1250, false),
    (1251, encoding_rs::WINDOWS_1251, false),
    (1252, encoding_rs::WINDOWS_1252, false),
    (1253, encoding_rs::WINDOWS_1253, false),
    (1254, encoding_rs::WINDOWS_1254, false),
    (1255, encoding_rs::WINDOWS_1255, false),
    (1256, encoding_rs::WINDOWS_1256, false),
    (1257, encoding_rs::WINDOWS_1257, false),
    (1258, encoding_rs::WINDOWS_1258, false),
    (20127, encoding_rs::WINDOWS_1252, true), // US-ASCII -> treat as 1252
    (20936, encoding_rs::GBK, false),         // simplified Chinese
    (28591, encoding_rs::WINDOWS_1252, true), // ISO-8859-1 mapped to windows-1252 per WHATWG
    (28592, encoding_rs::ISO_8859_2, false),
    (28593, encoding_rs::ISO_8859_3, false),
    (28594, encoding_rs::ISO_8859_4, false),
    (28595, encoding_rs::ISO_8859_5, false),
    (28596, encoding_rs::ISO_8859_6, false),
    (28597, encoding_rs::ISO_8859_7, false),
    (28598, encoding_rs::ISO_8859_8, false),
    (28599, encoding_rs::WINDOWS_1254, true), // ISO-8859-9 ≈ windows-1254
    (28603, encoding_rs::ISO_8859_13, false),
    (28605, encoding_rs::ISO_8859_15, false),
    (50220, encoding_rs::ISO_2022_JP, false),
    (51932, encoding_rs::EUC_JP, false),
    (51949, encoding_rs::EUC_KR, false),
    (52936, encoding_rs::GBK, true), // HZ-GB-2312
    (54936, encoding_rs::GB18030, false),
    (65001, encoding_rs::UTF_8, false),
];

/// Map a SAV IANA code page number to an encoding_rs Encoding.
pub fn encoding_from_code_page(code_page: i32) -> &'static Encoding {
    CODE_PAGES
        .iter()
        .find(|(number, _, _)| *number == code_page)
        .map_or(encoding_rs::WINDOWS_1252, |(_, encoding, _)| encoding) // safe default
}

/// The code page number a writer stores for `encoding`, 0 when it has none.
pub fn code_page_of(encoding: &'static Encoding) -> i32 {
    CODE_PAGES
        .iter()
        .find(|(_, known, approximate)| *known == encoding && !approximate)
        .map_or(0, |(number, _, _)| *number)
}

/// The encoding to write a file in: an encoding name ("windows-1252", "UTF-8") or a
/// Windows code page number ("1252"). Empty means UTF-8. UTF-16 is refused: text records
/// of a system file are byte strings.
pub fn encoding_for_writing(name: &str) -> Result<&'static Encoding> {
    let name = name.trim();
    let encoding = if name.is_empty() {
        Some(encoding_rs::UTF_8)
    } else if let Ok(number) = name.parse::<i32>() {
        CODE_PAGES
            .iter()
            .find(|(known, _, _)| *known == number)
            .map(|(_, encoding, _)| *encoding)
    } else {
        Encoding::for_label(name.as_bytes())
    };
    match encoding {
        Some(encoding) if encoding.output_encoding() == encoding => Ok(encoding),
        _ => Err(SavError::Encoding(format!(
            "cannot write a file in the encoding {name}"
        ))),
    }
}

/// Text as bytes of `encoding`; an error names the text when a character has no code there.
pub fn encode<'a>(text: &'a str, encoding: &'static Encoding) -> Result<Cow<'a, [u8]>> {
    if encoding == encoding_rs::UTF_8 {
        return Ok(Cow::Borrowed(text.as_bytes()));
    }
    let (bytes, _, unmappable) = encoding.encode(text);
    if unmappable {
        return Err(SavError::Encoding(format!(
            "{text:?} has characters that {} cannot hold; write the file as UTF-8",
            encoding.name()
        )));
    }
    Ok(bytes)
}

/// Map an encoding name string (e.g., "UTF-8", "windows-1252") to an encoding_rs Encoding.
pub fn encoding_from_name(name: &str) -> &'static Encoding {
    let normalized = name.trim().to_ascii_lowercase();
    Encoding::for_label(normalized.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252)
}

/// Decode a byte slice using the given encoding, returning a UTF-8 String.
#[allow(dead_code)]
pub fn decode_str(bytes: &[u8], encoding: &'static Encoding) -> Result<String> {
    if encoding == encoding_rs::UTF_8 {
        // Fast path: check if it's valid UTF-8
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => {
                let (decoded, _, had_errors) = encoding.decode(bytes);
                if had_errors {
                    // Still return the lossy result but note it
                    Ok(decoded.into_owned())
                } else {
                    Ok(decoded.into_owned())
                }
            }
        }
    } else {
        let (decoded, _, had_errors) = encoding.decode(bytes);
        if had_errors {
            Err(SavError::Encoding(format!(
                "failed to decode bytes with encoding {}",
                encoding.name()
            )))
        } else {
            Ok(decoded.into_owned())
        }
    }
}

/// Decode a byte slice using the given encoding, never failing (lossy).
/// Returns `Cow::Borrowed` for valid UTF-8, avoiding heap allocation.
#[inline]
pub fn decode_str_lossy<'a>(bytes: &'a [u8], encoding: &'static Encoding) -> Cow<'a, str> {
    if encoding == encoding_rs::UTF_8 {
        // Fast path: just validate UTF-8, zero-copy borrow
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Cow::Borrowed(s);
        }
    }
    let (decoded, _, _) = encoding.decode(bytes);
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_passthrough() {
        let s = "Hello, world!";
        let result = decode_str(s.as_bytes(), encoding_rs::UTF_8).unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_windows_1252_decode() {
        // "café" in windows-1252: 63 61 66 e9
        let bytes = [0x63, 0x61, 0x66, 0xe9];
        let result = decode_str(&bytes, encoding_rs::WINDOWS_1252).unwrap();
        assert_eq!(result, "café");
    }

    #[test]
    fn test_code_page_mapping() {
        assert_eq!(encoding_from_code_page(65001), encoding_rs::UTF_8);
        assert_eq!(encoding_from_code_page(1252), encoding_rs::WINDOWS_1252);
        assert_eq!(encoding_from_code_page(932), encoding_rs::SHIFT_JIS);
        // Unknown code page defaults to windows-1252
        assert_eq!(encoding_from_code_page(99999), encoding_rs::WINDOWS_1252);
    }

    #[test]
    fn test_encoding_from_name() {
        assert_eq!(encoding_from_name("UTF-8"), encoding_rs::UTF_8);
        assert_eq!(encoding_from_name("utf-8"), encoding_rs::UTF_8);
        assert_eq!(
            encoding_from_name("windows-1252"),
            encoding_rs::WINDOWS_1252
        );
        assert_eq!(encoding_from_name("ISO-8859-1"), encoding_rs::WINDOWS_1252); // encoding_rs maps this
    }
}
