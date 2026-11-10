//! Reading syntax files that are not UTF-8.
//!
//! A syntax file is UTF-8 when it decodes as UTF-8, with or without a byte order mark; a file
//! that starts with a UTF-16 byte order mark is UTF-16. Anything else was written in a legacy
//! code page: by default the one of the system (on Windows its ANSI code page, elsewhere the
//! character set of the locale, else Windows-1252), or the one the caller names.

use encoding_rs::Encoding;

/// The encoding a name stands for: an IANA or WHATWG label such as `windows-1257`, `cp1257`
/// or `iso-8859-15`, a Windows code page number such as `1257`, `UTF8`, or `LOCALE` for the
/// system's legacy code page.
pub fn encoding_for(name: &str) -> Option<&'static Encoding> {
    let name = name.trim();
    if name.eq_ignore_ascii_case("locale") {
        return Some(legacy_encoding());
    }
    if name.eq_ignore_ascii_case("utf8") {
        return Some(encoding_rs::UTF_8);
    }
    if let Ok(page) = name.parse::<u32>() {
        return code_page(page);
    }
    let digits = name
        .strip_prefix("cp")
        .or_else(|| name.strip_prefix("CP"))
        .and_then(|d| d.parse::<u32>().ok());
    digits
        .and_then(code_page)
        .or_else(|| Encoding::for_label(name.as_bytes()))
}

fn code_page(page: u32) -> Option<&'static Encoding> {
    let label = match page {
        65001 => "utf-8",
        1200 => "utf-16le",
        1201 => "utf-16be",
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        28591..=28606 => {
            return Encoding::for_label(format!("iso-8859-{}", page - 28590).as_bytes());
        }
        _ => return Encoding::for_label(format!("windows-{page}").as_bytes()),
    };
    Encoding::for_label(label.as_bytes())
}

/// The legacy code page of this system, used for syntax files that are not UTF-8.
pub fn legacy_encoding() -> &'static Encoding {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetACP() -> u32;
        }
        // SAFETY: GetACP takes no arguments and only reads a process setting.
        if let Some(encoding) = code_page(unsafe { GetACP() }) {
            return encoding;
        }
    }
    // LANG=et_EE.ISO-8859-15 names a character set; a UTF-8 locale has no legacy one
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
        .unwrap_or_default();
    locale
        .split_once('.')
        .map(|(_, charset)| charset.split('@').next().unwrap_or(charset))
        .and_then(|charset| Encoding::for_label(charset.as_bytes()))
        .filter(|e| *e != encoding_rs::UTF_8)
        .unwrap_or(encoding_rs::WINDOWS_1252)
}

/// The text of a syntax file. With `encoding` the bytes are read in that encoding (a byte
/// order mark still wins); without, UTF-8 when they are valid UTF-8, else the system's legacy
/// code page. The byte order mark is removed. Fails only on an unknown encoding name.
pub fn decode_syntax(bytes: &[u8], encoding: Option<&str>) -> Result<String, String> {
    let named = match encoding {
        Some(name) => Some(encoding_for(name).ok_or_else(|| format!("unknown encoding {name}"))?),
        None => None,
    };
    if let Some((bom, _)) = Encoding::for_bom(bytes) {
        return Ok(bom.decode_with_bom_removal(bytes).0.into_owned());
    }
    let encoding = match (named, std::str::from_utf8(bytes)) {
        (Some(encoding), _) => encoding,
        (None, Ok(text)) => return Ok(text.to_string()),
        (None, Err(_)) => legacy_encoding(),
    };
    Ok(encoding.decode_without_bom_handling(bytes).0.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_with_and_without_bom_and_legacy_code_pages() {
        assert_eq!(decode_syntax("ÕÄ.".as_bytes(), None).unwrap(), "ÕÄ.");
        assert_eq!(decode_syntax(b"\xEF\xBB\xBFGET.", None).unwrap(), "GET.");
        // 0xF0 is s with caron in Windows-1257 (Baltic) and eth in Windows-1252
        assert_eq!(decode_syntax(b"\xF0", Some("1257")).unwrap(), "š");
        assert_eq!(decode_syntax(b"\xF0", Some("cp1257")).unwrap(), "š");
        assert_eq!(decode_syntax(b"\xF0", Some("windows-1252")).unwrap(), "ð");
        assert_eq!(decode_syntax(b"\xF0", Some("iso-8859-15")).unwrap(), "ð");
        assert_eq!(decode_syntax(b"\xF0", Some("UTF8")).unwrap(), "\u{FFFD}");
        // not UTF-8 and no encoding named: the legacy code page, whatever it is, decodes it
        assert_eq!(decode_syntax(b"a\xE4", None).unwrap().chars().count(), 2);
        assert_eq!(decode_syntax(b"\xFF\xFEa\x00", None).unwrap(), "a");
        assert!(decode_syntax(b"x", Some("klingon")).is_err());
    }
}
