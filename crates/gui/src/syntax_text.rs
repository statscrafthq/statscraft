//! Small pieces of syntax text shared by the dialogs and the editors.

/// A string literal: single quotes, a quote inside doubled.
pub fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// A number literal as the user would type it: 1, 0.5, -3.
pub fn number(n: f64) -> String {
    format!("{n}")
}

/// A value the user typed, as a literal: quoted for a string variable, otherwise a number.
pub fn value(text: &str, string: bool) -> Result<String, String> {
    if string {
        return Ok(quote(text));
    }
    let text = text.trim();
    text.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(number)
        .ok_or_else(|| format!("{text:?} is not a number"))
}

/// A list of variable names, or an error naming what is missing.
pub fn names(names: &[String], what: &str) -> Result<String, String> {
    if names.is_empty() {
        return Err(format!("add at least one variable to {what}"));
    }
    Ok(names.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        assert_eq!(quote("it's"), "'it''s'");
        assert_eq!(number(1.0), "1");
        assert_eq!(number(-0.25), "-0.25");
        assert_eq!(value(" 3.50 ", false).unwrap(), "3.5");
        assert_eq!(value(" a b ", true).unwrap(), "' a b '");
        assert_eq!(value("abc", false).unwrap_err(), "\"abc\" is not a number");
        assert!(value("inf", false).is_err());
        assert_eq!(
            names(&[], "Variables").unwrap_err(),
            "add at least one variable to Variables"
        );
    }
}
