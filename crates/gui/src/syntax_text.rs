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

/// A probability the user typed, such as a significance level: a number between 0 and 1.
pub fn probability(text: &str) -> Result<String, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|p| *p > 0.0 && *p < 1.0)
        .map(number)
        .ok_or_else(|| "the significance level must be a number between 0 and 1".into())
}

/// A confidence level the user typed in percent, as the fraction the syntax takes: 95 is 0.95.
/// `None` for the default of 95 percent.
pub fn confidence(text: &str) -> Result<Option<String>, String> {
    let percent = text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|p| *p > 0.0 && *p < 100.0)
        .ok_or("the confidence level must be a percentage between 0 and 100")?;
    // rounded, so 99.9 gives 0.999 and not 0.9990000000000001
    let fraction = (percent * 1e6).round() / 1e8;
    Ok((fraction != 0.95).then(|| number(fraction)))
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
        assert_eq!(probability(" 0.01 ").unwrap(), "0.01");
        assert!(probability("1").is_err());
        assert_eq!(confidence("95").unwrap(), None);
        assert_eq!(confidence("99.9").unwrap().unwrap(), "0.999");
        assert_eq!(confidence("90").unwrap().unwrap(), "0.9");
        assert!(confidence("0.95x").is_err());
        assert_eq!(
            names(&[], "Variables").unwrap_err(),
            "add at least one variable to Variables"
        );
    }
}
