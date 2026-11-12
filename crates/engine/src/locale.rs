//! What SET LOCALE changes in Statscraft: the decimal mark for reading and writing text data
//! (SET DECIMAL), which a later LOCALE sets anew. Files are always read and written in Unicode,
//! so the code page part of a locale (`.1257`, `.UTF-8`) changes nothing.

/// Languages that write a decimal comma, by ISO 639-1 code and English name.
const COMMA: &[(&str, &str)] = &[
    ("af", "AFRIKAANS"),
    ("az", "AZERBAIJANI"),
    ("be", "BELARUSIAN"),
    ("bg", "BULGARIAN"),
    ("bs", "BOSNIAN"),
    ("ca", "CATALAN"),
    ("cs", "CZECH"),
    ("da", "DANISH"),
    ("de", "GERMAN"),
    ("el", "GREEK"),
    ("es", "SPANISH"),
    ("et", "ESTONIAN"),
    ("eu", "BASQUE"),
    ("fi", "FINNISH"),
    ("fo", "FAROESE"),
    ("fr", "FRENCH"),
    ("gl", "GALICIAN"),
    ("hr", "CROATIAN"),
    ("hu", "HUNGARIAN"),
    ("hy", "ARMENIAN"),
    ("id", "INDONESIAN"),
    ("is", "ICELANDIC"),
    ("it", "ITALIAN"),
    ("ka", "GEORGIAN"),
    ("kk", "KAZAKH"),
    ("ky", "KYRGYZ"),
    ("lt", "LITHUANIAN"),
    ("lv", "LATVIAN"),
    ("mk", "MACEDONIAN"),
    ("nb", "NORWEGIAN"),
    ("nl", "DUTCH"),
    ("nn", "NYNORSK"),
    ("no", "NORWEGIAN"),
    ("pl", "POLISH"),
    ("pt", "PORTUGUESE"),
    ("ro", "ROMANIAN"),
    ("ru", "RUSSIAN"),
    ("sk", "SLOVAK"),
    ("sl", "SLOVENIAN"),
    ("sq", "ALBANIAN"),
    ("sr", "SERBIAN"),
    ("sv", "SWEDISH"),
    ("tr", "TURKISH"),
    ("uk", "UKRAINIAN"),
    ("uz", "UZBEK"),
    ("vi", "VIETNAMESE"),
];

/// Languages that write a decimal point.
const DOT: &[(&str, &str)] = &[
    ("ar", "ARABIC"),
    ("bn", "BENGALI"),
    ("cy", "WELSH"),
    ("en", "ENGLISH"),
    ("ga", "IRISH"),
    ("he", "HEBREW"),
    ("hi", "HINDI"),
    ("ja", "JAPANESE"),
    ("ko", "KOREAN"),
    ("ms", "MALAY"),
    ("mt", "MALTESE"),
    ("th", "THAI"),
    ("tl", "TAGALOG"),
    ("zh", "CHINESE"),
];

/// Countries where a comma language writes a decimal point.
const DOT_COUNTRIES: &[(&str, &[&str])] = &[
    ("de", &["CH", "LI", "SWITZERLAND", "LIECHTENSTEIN"]),
    ("it", &["CH", "SWITZERLAND"]),
    (
        "es",
        &[
            "MX", "US", "PR", "GT", "HN", "NI", "SV", "DO", "PA", "PE", "MEXICO",
        ],
    ),
];

/// The SET DECIMAL value a locale brings: `DOT` or `COMMA`; None for a locale not known.
/// The locale is `language[_COUNTRY][.codepage][@variant]`, with a code or an English name
/// (`et_EE`, `en-GB.UTF-8`, `Estonian_Estonia.1257`, `Japanese`).
pub(crate) fn decimal(locale: &str) -> Option<&'static str> {
    let base = locale.split(['.', '@']).next().unwrap_or("").trim();
    let mut parts = base.split(['_', '-']);
    let language = parts.next().unwrap_or("").to_uppercase();
    let country = parts.next().unwrap_or("").to_uppercase();
    let find = |table: &[(&'static str, &str)]| {
        table
            .iter()
            .find(|(code, name)| code.eq_ignore_ascii_case(&language) || *name == language)
            .map(|(code, _)| *code)
    };
    if let Some(code) = find(COMMA) {
        let dot = DOT_COUNTRIES
            .iter()
            .any(|(c, countries)| *c == code && countries.contains(&country.as_str()));
        return Some(if dot { "DOT" } else { "COMMA" });
    }
    find(DOT).map(|_| "DOT")
}

#[cfg(test)]
mod tests {
    use super::decimal;

    #[test]
    fn a_locale_gives_the_decimal_mark_of_its_language_and_country() {
        assert_eq!(decimal("et_EE"), Some("COMMA"));
        assert_eq!(decimal("en_GB"), Some("DOT"));
        assert_eq!(decimal("en_UK"), Some("DOT"));
        assert_eq!(decimal("en-GB.UTF-8"), Some("DOT"));
        assert_eq!(decimal("Estonian_Estonia.1257"), Some("COMMA"));
        assert_eq!(decimal("Japanese"), Some("DOT"));
        assert_eq!(decimal("ja_JP.Shift_JIS"), Some("DOT"));
        assert_eq!(decimal("de_CH"), Some("DOT"));
        assert_eq!(decimal("de_DE@euro"), Some("COMMA"));
        assert_eq!(decimal("es_MX"), Some("DOT"));
        assert_eq!(decimal("es_ES"), Some("COMMA"));
        assert_eq!(decimal("klingon"), None);
        assert_eq!(decimal(""), None);
    }
}
