//! Parser of RESPONSE RATE: response rates of a numerator over a denominator in each group of
//! a grouping variable, with cumulative rates and limits.
//!
//! Rules from the Command Syntax Reference (RESPONSE RATE), in our words:
//! - VARIABLES is required with NUMERATOR=, DENOMINATOR= and GROUP=, one variable each, in any
//!   order.
//! - MINRATE RATE=NONE (default) | value (above 0, below 100) | FORMULA=a, b (both above 0;
//!   the rate a / b * 100 above 0 and below 100).
//! - MAXCOUNT COUNT=ALL (default) | value (a positive whole number) | PERCENT=value (above 0,
//!   below 100).
//! - Subcommands in any order; every word in full; the equals signs are required.
//!
//! Choices where the manual is silent: `/MINRATE=value` and
//! `/MAXCOUNT=value`, as the manual's own examples write them, stand for RATE= and COUNT=;
//! each subcommand is given once.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::twostep::keyword;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaxCount {
    /// The largest cumulative sum of the denominator.
    Count(f64),
    /// That sum as a percentage of the denominator's total.
    Percent(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResponseRate {
    pub numerator: String,
    pub denominator: String,
    pub group: String,
    /// The lowest cumulative response rate in percent, from RATE= or FORMULA=.
    pub min_rate: Option<f64>,
    pub max_count: Option<MaxCount>,
}

/// A number strictly between `low` and `high`.
fn between(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(n) if n > low && n < high => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be above {low} and below {high}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn min_rate(p: &mut Parser) -> PResult<Option<f64>> {
    let mut seen = Vec::new();
    // `/MINRATE=5` is RATE=5
    let word = match p.eat(Punct::Eq) {
        true => "RATE",
        false => {
            let word = keyword(p, &mut seen, &["RATE", "FORMULA"])?;
            p.expect(Punct::Eq)?;
            word
        }
    };
    if word == "RATE" {
        if p.eat_word("NONE") {
            return Ok(None);
        }
        return between(p, "RATE", 0.0, 100.0).map(Some);
    }
    let start = p.pos;
    let cost = between(p, "the first FORMULA value", 0.0, f64::INFINITY)?;
    p.eat(Punct::Comma);
    let revenue = between(p, "the second FORMULA value", 0.0, f64::INFINITY)?;
    let rate = cost / revenue * 100.0;
    if rate >= 100.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("FORMULA gives a minimum rate of {rate}%, which must be below 100"),
        ));
    }
    Ok(Some(rate))
}

fn max_count(p: &mut Parser) -> PResult<Option<MaxCount>> {
    let mut seen = Vec::new();
    // `/MAXCOUNT=6000` is COUNT=6000
    let word = match p.eat(Punct::Eq) {
        true => "COUNT",
        false => {
            let word = keyword(p, &mut seen, &["COUNT", "PERCENT"])?;
            p.expect(Punct::Eq)?;
            word
        }
    };
    if word == "PERCENT" {
        return between(p, "PERCENT", 0.0, 100.0).map(|n| Some(MaxCount::Percent(n)));
    }
    if p.eat_word("ALL") {
        return Ok(None);
    }
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(MaxCount::Count(n))),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("COUNT must be a positive whole number, got {n}"),
        )),
        None => Err(p.unexpected(&["ALL", "a positive whole number"])),
    }
}

pub(crate) fn response_rate(p: &mut Parser) -> PResult<Command> {
    let mut numerator = None;
    let mut denominator = None;
    let mut group = None;
    let mut command = ResponseRate {
        numerator: String::new(),
        denominator: String::new(),
        group: String::new(),
        min_rate: None,
        max_count: None,
    };
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let start = p.pos;
        let names = ["VARIABLES", "MINRATE", "MAXCOUNT"];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("RESPONSE RATE", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("RESPONSE RATE takes /{sub} once"),
            ));
        }
        subcommands.push(sub);
        match sub {
            "VARIABLES" => {
                let mut seen = Vec::new();
                while !at_end(p) {
                    let word = keyword(p, &mut seen, &["NUMERATOR", "DENOMINATOR", "GROUP"])?;
                    p.expect(Punct::Eq)?;
                    let name = Some(p.name("a variable name")?);
                    match word {
                        "NUMERATOR" => numerator = name,
                        "DENOMINATOR" => denominator = name,
                        _ => group = name,
                    }
                }
            }
            "MINRATE" => command.min_rate = min_rate(p)?,
            _ => command.max_count = max_count(p)?,
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    match (numerator, denominator, group) {
        (Some(n), Some(d), Some(g)) => {
            command.numerator = n;
            command.denominator = d;
            command.group = g;
        }
        _ => {
            return Err(SyntaxError::new(
                p.span(),
                "RESPONSE RATE needs /VARIABLES with NUMERATOR=, DENOMINATOR= and GROUP=",
            ));
        }
    }
    Ok(Command::ResponseRate(Box::new(command)))
}

#[cfg(test)]
mod tests {
    use crate::{Command, MaxCount, parse};

    fn one(source: &str) -> super::ResponseRate {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match parsed.remove(0).command {
            Command::ResponseRate(r) => *r,
            other => panic!("{other:?}"),
        }
    }

    fn err(source: &str) -> String {
        let (_, mut errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors.remove(0).message
    }

    #[test]
    fn variables_and_limits() {
        let r = one("RESPONSE RATE /VARIABLES GROUP=zone NUMERATOR=buys DENOMINATOR=mailed.");
        assert_eq!(
            (r.numerator.as_str(), r.denominator.as_str()),
            ("buys", "mailed")
        );
        assert_eq!(
            (r.group.as_str(), r.min_rate, r.max_count),
            ("zone", None, None)
        );
        let r = one(
            "RESPONSE RATE /MAXCOUNT PERCENT=40 /VARIABLES NUMERATOR=b DENOMINATOR=m \
                     GROUP=z /MINRATE FORMULA=0.5, 20.",
        );
        assert_eq!(
            (r.min_rate, r.max_count),
            (Some(2.5), Some(MaxCount::Percent(40.0)))
        );
        let r = one(
            "RESPONSE RATE /VARIABLES NUMERATOR=b DENOMINATOR=m GROUP=z /MINRATE=4 /MAXCOUNT=300.",
        );
        assert_eq!(
            (r.min_rate, r.max_count),
            (Some(4.0), Some(MaxCount::Count(300.0)))
        );
        let r = one(
            "RESPONSE RATE /VARIABLES NUMERATOR=b DENOMINATOR=m GROUP=z \
                     /MINRATE RATE=NONE /MAXCOUNT COUNT=ALL.",
        );
        assert_eq!((r.min_rate, r.max_count), (None, None));
    }

    #[test]
    fn errors() {
        let base = "RESPONSE RATE /VARIABLES NUMERATOR=b DENOMINATOR=m GROUP=z";
        assert!(err("RESPONSE RATE /VARIABLES NUMERATOR=b GROUP=z.").contains("DENOMINATOR="));
        assert!(err("RESPONSE RATE /MINRATE=5.").contains("needs /VARIABLES"));
        assert!(err(&format!("{base} /MINRATE RATE=100.")).contains("below 100"));
        assert!(err(&format!("{base} /MINRATE RATE=0.")).contains("above 0"));
        assert!(err(&format!("{base} /MINRATE FORMULA=3 2.")).contains("below 100"));
        assert!(err(&format!("{base} /MINRATE FORMULA=-1, 2.")).contains("above 0"));
        assert!(err(&format!("{base} /MAXCOUNT COUNT=2.5.")).contains("whole number"));
        assert!(err(&format!("{base} /MAXCOUNT PERCENT=100.")).contains("below 100"));
        assert!(err(&format!("{base} /MINRATE=5 /MINRATE=6.")).contains("once"));
        assert!(err(&format!("{base} GROUP=y.")).contains("twice"));
        // every word in full
        assert!(!err(&format!("{base} /MINR=5.")).is_empty());
        assert!(!err("RESPONSE RAT /VARIABLES NUMERATOR=b DENOMINATOR=m GROUP=z.").is_empty());
        assert!(!err("RESP RATE /VARIABLES NUMERATOR=b DENOMINATOR=m GROUP=z.").is_empty());
    }
}
