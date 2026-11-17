//! Parser of PROPORTIONS: tests and confidence intervals for one proportion, for the difference
//! of two related proportions and for the difference of two independent proportions.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - Exactly one of ONESAMPLE varlist, PAIREDSAMPLES varlist [WITH varlist [(PAIRED)]] and
//!   INDEPENDENTSAMPLES varlist BY var, given once.
//! - TESTVAL (ONESAMPLE only, strictly between 0 and 1, default 0.5), SELECT (INDEPENDENTSAMPLES
//!   only), TESTTYPES and CITYPES follow the variable list, and may also stand as subcommands of
//!   their own, as the manual's examples write them. NONE turns tests or intervals off.
//! - The keywords of TESTTYPES and CITYPES depend on the analysis, and each analysis has its own
//!   defaults (see `DEFAULTS`).
//! - SELECT is LEVEL(value1, value2) (default LEVEL(1, 2)), MIDPOINT or CUTPOINT(value).
//! - SUCCESSVALUE (SUCCESS for short) is LAST (default), FIRST, LEVEL(values), MIDPOINT or
//!   CUTPOINT(value).
//! - MISSING SCOPE=ANALYSIS | LISTWISE and USERMISSING=EXCLUDE | INCLUDE.
//! - CRITERIA CILEVEL in (0, 100), default 95. A value below 1 is read as a fraction, since the
//!   manual also calls the default 0.95.
//! - Subcommands come in any order.

use crate::ast::{Command, TTestPairs, Value, VarItem};
use crate::error::{Span, SyntaxError};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

#[derive(Debug, Clone, PartialEq)]
pub struct Proportions {
    pub analysis: ProportionsAnalysis,
    /// Canonical TESTTYPES keywords in the order given; empty for NONE.
    pub tests: Vec<String>,
    /// Canonical CITYPES keywords in the order given; empty for NONE.
    pub intervals: Vec<String>,
    pub success: ProportionsSuccess,
    /// `SCOPE=LISTWISE`
    pub listwise: bool,
    /// `USERMISSING=INCLUDE`
    pub include: bool,
    /// The confidence level in percent.
    pub level: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProportionsAnalysis {
    OneSample {
        variables: Vec<VarItem>,
        test_value: f64,
    },
    Paired(TTestPairs),
    Independent {
        variables: Vec<VarItem>,
        group: String,
        select: ProportionsGroups,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProportionsSuccess {
    Last,
    First,
    Level(Vec<Value>),
    Midpoint,
    Cutpoint(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProportionsGroups {
    Levels(Value, Value),
    Midpoint,
    Cutpoint(f64),
}

/// Per analysis: the TESTTYPES keywords, the CITYPES keywords and the defaults of each.
const ONE_TESTS: [&str; 6] = ["EXACT", "MIDP", "SCORE", "SCORECC", "WALD", "WALDCC"];
const ONE_INTERVALS: [&str; 9] = [
    "AGRESTI_COULL",
    "ANSCOMBE",
    "CLOPPER_PEARSON",
    "JEFFREYS",
    "LOGIT",
    "WALD",
    "WALDCC",
    "WILSON_SCORE",
    "WILSON_SCORECC",
];
const PAIRED_TESTS: [&str; 6] = ["EXACT", "MIDP", "MCNEMAR", "MCNEMARCC", "WALD", "WALDCC"];
const PAIRED_INTERVALS: [&str; 5] = ["AGRESTI_MIN", "BONETT_PRICE", "NEWCOMBE", "WALD", "WALDCC"];
const INDEPENDENT_TESTS: [&str; 5] = ["HAUCK_ANDERSON", "WALD", "WALDCC", "WALDH0", "WALDH0CC"];
const INDEPENDENT_INTERVALS: [&str; 7] = [
    "AGRESTI_CAFFO",
    "BROWN_LI_JEFFREYS",
    "HAUCK_ANDERSON",
    "NEWCOMBE",
    "NEWCOMBECC",
    "WALD",
    "WALDCC",
];

/// The keywords of one analysis and its defaults: (tests, default tests, intervals, default
/// intervals).
type Methods = (
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
);

fn methods(analysis: &ProportionsAnalysis) -> Methods {
    match analysis {
        ProportionsAnalysis::OneSample { .. } => (
            &ONE_TESTS,
            &["MIDP", "SCORE"],
            &ONE_INTERVALS,
            &["AGRESTI_COULL", "JEFFREYS", "WILSON_SCORE"],
        ),
        ProportionsAnalysis::Paired(_) => (
            &PAIRED_TESTS,
            &["MIDP", "MCNEMAR"],
            &PAIRED_INTERVALS,
            &["BONETT_PRICE", "NEWCOMBE"],
        ),
        ProportionsAnalysis::Independent { .. } => (
            &INDEPENDENT_TESTS,
            &["WALDH0"],
            &INDEPENDENT_INTERVALS,
            &["AGRESTI_CAFFO", "NEWCOMBE"],
        ),
    }
}

/// True at a word followed by '=': a keyword of the subcommand, not a variable.
fn at_assignment(p: &Parser) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(_)))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
}

/// A variable list that stops at BY, WITH and at `KEYWORD=`.
fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("BY") || w.eq_ignore_ascii_case("WITH") || at_assignment(p) {
            break;
        }
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            if p.eat_word("TO") {
                items.push(VarItem::Range(first, p.name("a variable name")?));
            } else {
                items.push(VarItem::Name(first));
            }
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

fn number(p: &mut Parser) -> PResult<(f64, usize)> {
    let start = p.pos;
    match p.number() {
        Some(value) => Ok((value, start)),
        None => Err(p.unexpected(&["a number"])),
    }
}

/// `(value)` after CUTPOINT.
fn cutpoint(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let (value, _) = number(p)?;
    p.expect(Punct::RParen)?;
    Ok(value)
}

/// `(value ...)`: numbers or quoted strings, with optional commas.
fn levels(p: &mut Parser) -> PResult<Vec<Value>> {
    p.expect(Punct::LParen)?;
    let mut out = Vec::new();
    while let Some(value) = p.value() {
        out.push(value);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a number", "a quoted string"]));
    }
    p.expect(Punct::RParen)?;
    Ok(out)
}

/// The keywords of TESTTYPES or CITYPES up to the next keyword or subcommand, as written (upper
/// case) with their positions; checked once the analysis is known.
fn method_words(p: &mut Parser) -> PResult<Vec<(String, Span)>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if at_assignment(p) {
            break;
        }
        out.push((w.to_uppercase(), p.span()));
        p.pos += 1;
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a test or interval type", "NONE"]));
    }
    Ok(out)
}

/// The canonical keywords of `words`: an exact match first, else the one keyword the word
/// abbreviates. NONE alone gives an empty list.
fn resolve(what: &str, words: &[(String, Span)], known: &[&'static str]) -> PResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for (word, span) in words {
        let span = *span;
        if matches_keyword(word, "NONE") && word.len() >= 4 {
            if words.len() > 1 {
                let message = format!("NONE on {what} cannot be combined with other types");
                return Err(SyntaxError::new(span, message));
            }
            return Ok(Vec::new());
        }
        let found: Vec<&str> = match known.iter().find(|k| *k == word) {
            Some(exact) => vec![*exact],
            None => known
                .iter()
                .copied()
                .filter(|k| matches_keyword(word, k))
                .collect(),
        };
        match found[..] {
            [one] => {
                if !out.iter().any(|o| o == one) {
                    out.push(one.to_string());
                }
            }
            [] => {
                let message = format!(
                    "{word} is not a {what} type of this analysis; use one of {} or NONE",
                    known.join(", ")
                );
                return Err(SyntaxError::new(span, message));
            }
            _ => {
                let message = format!("{word} could mean {}", found.join(" or "));
                return Err(SyntaxError::new(span, message));
            }
        }
    }
    Ok(out)
}

/// What SELECT, TESTVAL, TESTTYPES and CITYPES set, wherever they were written.
#[derive(Default)]
struct Options {
    test_value: Option<(f64, Span)>,
    select: Option<(ProportionsGroups, Span)>,
    tests: Option<Vec<(String, Span)>>,
    intervals: Option<Vec<(String, Span)>>,
}

/// One of the keywords that may follow the variable list or stand as a subcommand. False when
/// the next word is none of them.
fn option(p: &mut Parser, options: &mut Options) -> PResult<bool> {
    let start = p.pos;
    if p.eat_keyword("TESTVAL") {
        p.eat(Punct::Eq);
        let (value, at) = number(p)?;
        if !(value > 0.0 && value < 1.0) {
            let message = format!("TESTVAL must lie between 0 and 1, got {value}");
            return Err(SyntaxError::new(p.span_from(at), message));
        }
        options.test_value = Some((value, p.span_from(start)));
    } else if p.eat_keyword("SELECT") {
        p.eat(Punct::Eq);
        let select = if p.eat_keyword("LEVEL") {
            let values = levels(p)?;
            let [a, b] = <[Value; 2]>::try_from(values).map_err(|_| {
                SyntaxError::new(p.span_from(start), "SELECT=LEVEL takes two values")
            })?;
            ProportionsGroups::Levels(a, b)
        } else if p.eat_keyword("MIDPOINT") {
            ProportionsGroups::Midpoint
        } else if p.eat_keyword("CUTPOINT") {
            ProportionsGroups::Cutpoint(cutpoint(p)?)
        } else {
            return Err(p.unexpected(&["LEVEL", "MIDPOINT", "CUTPOINT"]));
        };
        options.select = Some((select, p.span_from(start)));
    } else if p.eat_keyword("TESTTYPES") || p.eat_keyword("TESTSTYPES") {
        // TESTSTYPES: the spelling of the manual's own example
        options.tests = Some(method_words(p)?);
    } else if p.eat_keyword("CITYPES") {
        options.intervals = Some(method_words(p)?);
    } else {
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn proportions(p: &mut Parser) -> PResult<Command> {
    // the analysis, its options filled in at the end
    let mut analysis: Option<ProportionsAnalysis> = None;
    let mut options = Options::default();
    let mut success = ProportionsSuccess::Last;
    let (mut listwise, mut include, mut level) = (false, false, 95.0);
    const SUBCOMMANDS: [&str; 10] = [
        "ONESAMPLE",
        "PAIREDSAMPLES",
        "INDEPENDENTSAMPLES",
        "SUCCESSVALUE",
        "MISSING",
        "CRITERIA",
        "TESTVAL",
        "SELECT",
        "TESTTYPES",
        "CITYPES",
    ];
    p.eat(Punct::Slash);
    loop {
        let is_analysis = ["ONESAMPLE", "PAIREDSAMPLES", "INDEPENDENTSAMPLES"]
            .iter()
            .any(|k| p.at_keyword(k));
        if is_analysis && analysis.is_some() {
            let message = "PROPORTIONS runs one analysis: give only one ONESAMPLE, PAIREDSAMPLES or \
                           INDEPENDENTSAMPLES subcommand";
            return Err(SyntaxError::new(p.span(), message));
        }
        if p.eat_keyword("ONESAMPLE") {
            p.eat(Punct::Eq);
            let variables = variables(p)?;
            analysis = Some(ProportionsAnalysis::OneSample {
                variables,
                test_value: 0.5,
            });
            while option(p, &mut options)? {}
        } else if p.eat_keyword("PAIREDSAMPLES") {
            p.eat(Punct::Eq);
            let left = variables(p)?;
            let mut pairs = TTestPairs {
                left,
                right: None,
                paired: false,
            };
            if p.eat_word("WITH") {
                pairs.right = Some(variables(p)?);
                if p.eat(Punct::LParen) {
                    if !p.eat_keyword("PAIRED") {
                        return Err(p.unexpected(&["PAIRED"]));
                    }
                    p.expect(Punct::RParen)?;
                    pairs.paired = true;
                }
            }
            analysis = Some(ProportionsAnalysis::Paired(pairs));
            while option(p, &mut options)? {}
        } else if p.eat_keyword("INDEPENDENTSAMPLES") {
            p.eat(Punct::Eq);
            let variables = variables(p)?;
            if !p.eat_word("BY") {
                return Err(p
                    .unexpected(&["BY"])
                    .hint("INDEPENDENTSAMPLES needs BY and the variable that forms the groups"));
            }
            let group = p.name("the grouping variable")?;
            analysis = Some(ProportionsAnalysis::Independent {
                variables,
                group,
                select: ProportionsGroups::Levels(Value::Number(1.0), Value::Number(2.0)),
            });
            while option(p, &mut options)? {}
        } else if p.eat_keyword("SUCCESSVALUE") {
            p.eat(Punct::Eq);
            success = if p.eat_keyword("LAST") {
                ProportionsSuccess::Last
            } else if p.eat_keyword("FIRST") {
                ProportionsSuccess::First
            } else if p.eat_keyword("LEVEL") {
                ProportionsSuccess::Level(levels(p)?)
            } else if p.eat_keyword("MIDPOINT") {
                ProportionsSuccess::Midpoint
            } else if p.eat_keyword("CUTPOINT") {
                ProportionsSuccess::Cutpoint(cutpoint(p)?)
            } else {
                return Err(p.unexpected(&["LAST", "FIRST", "LEVEL", "MIDPOINT", "CUTPOINT"]));
            };
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("SCOPE") {
                    p.eat(Punct::Eq);
                    listwise = if p.eat_keyword("LISTWISE") {
                        true
                    } else if p.eat_keyword("ANALYSIS") {
                        false
                    } else {
                        return Err(p.unexpected(&["ANALYSIS", "LISTWISE"]));
                    };
                } else if p.eat_keyword("USERMISSING") {
                    p.eat(Punct::Eq);
                    include = if p.eat_keyword("INCLUDE") {
                        true
                    } else if p.eat_keyword("EXCLUDE") {
                        false
                    } else {
                        return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                    };
                } else {
                    return Err(p.unexpected(&["SCOPE", "USERMISSING"]));
                }
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if !p.eat_keyword("CILEVEL") {
                    return Err(p.unexpected(&["CILEVEL"]));
                }
                p.eat(Punct::Eq);
                let parenthesized = p.eat(Punct::LParen);
                let (value, at) = number(p)?;
                if parenthesized {
                    p.expect(Punct::RParen)?;
                }
                if !(value > 0.0 && value < 100.0) {
                    let message = format!("CILEVEL must lie between 0 and 100, got {value}");
                    return Err(SyntaxError::new(p.span_from(at), message));
                }
                level = if value < 1.0 { value * 100.0 } else { value };
            }
        } else if !option(p, &mut options)? {
            return Err(p.subcommand_error("PROPORTIONS", &[]).hint(format!(
                "PROPORTIONS has the subcommands {}",
                SUBCOMMANDS.join(", ")
            )));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    let Some(mut analysis) = analysis else {
        return Err(SyntaxError::new(
            p.span(),
            "PROPORTIONS needs one of ONESAMPLE, PAIREDSAMPLES or INDEPENDENTSAMPLES",
        ));
    };
    if let Some((value, at)) = options.test_value {
        match &mut analysis {
            ProportionsAnalysis::OneSample { test_value, .. } => *test_value = value,
            _ => {
                let message = "TESTVAL belongs to ONESAMPLE";
                return Err(SyntaxError::new(at, message));
            }
        }
    }
    if let Some((choice, at)) = options.select {
        match &mut analysis {
            ProportionsAnalysis::Independent { select, .. } => *select = choice,
            _ => {
                let message = "SELECT belongs to INDEPENDENTSAMPLES";
                return Err(SyntaxError::new(at, message));
            }
        }
    }
    let (tests, default_tests, intervals, default_intervals) = methods(&analysis);
    let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let tests = match &options.tests {
        Some(words) => resolve("TESTTYPES", words, tests)?,
        None => owned(default_tests),
    };
    let intervals = match &options.intervals {
        Some(words) => resolve("CITYPES", words, intervals)?,
        None => owned(default_intervals),
    };
    Ok(Command::Proportions(Box::new(Proportions {
        analysis,
        tests,
        intervals,
        success,
        listwise,
        include,
        level,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(source: &str) -> Proportions {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match parsed.remove(0).command {
            Command::Proportions(c) => *c,
            other => panic!("{other:?}"),
        }
    }

    fn err(source: &str) -> String {
        let (_, errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        format!("{:?}", errors[0])
    }

    fn names(list: &[&str]) -> Vec<VarItem> {
        list.iter().map(|n| VarItem::Name(n.to_string())).collect()
    }

    #[test]
    fn one_sample_defaults_and_keywords() {
        let c = one("PROPORTIONS /ONESAMPLE a b.");
        assert_eq!(
            c.analysis,
            ProportionsAnalysis::OneSample {
                variables: names(&["a", "b"]),
                test_value: 0.5
            }
        );
        assert_eq!(c.tests, ["MIDP", "SCORE"]);
        assert_eq!(c.intervals, ["AGRESTI_COULL", "JEFFREYS", "WILSON_SCORE"]);
        assert_eq!(
            (c.success, c.listwise, c.include, c.level),
            (ProportionsSuccess::Last, false, false, 95.0)
        );

        let c = one(
            "PROPORTIONS /ONESAMPLE x TESTVAL=0.3 TESTTYPES=exact waldcc CITYPES=clo wilson_scorecc \
             /SUCCESS=LEVEL(2 3) /MISSING SCOPE=LISTWISE USERMISSING=INCLUDE /CRITERIA CILEVEL=90.",
        );
        assert!(
            matches!(c.analysis, ProportionsAnalysis::OneSample { test_value, .. } if test_value == 0.3)
        );
        assert_eq!(c.tests, ["EXACT", "WALDCC"]);
        assert_eq!(c.intervals, ["CLOPPER_PEARSON", "WILSON_SCORECC"]);
        assert_eq!(
            c.success,
            ProportionsSuccess::Level(vec![Value::Number(2.0), Value::Number(3.0)])
        );
        assert_eq!((c.listwise, c.include, c.level), (true, true, 90.0));
    }

    #[test]
    fn options_as_subcommands_and_none() {
        // the layout of the manual's examples: types as subcommands, in any order
        let c = one(
            "PROPORTIONS /TESTSTYPES=EXACT /ONESAMPLE m1 m2 /CITYPES=NONE /CRITERIA CILEVEL=0.99.",
        );
        assert_eq!(c.tests, ["EXACT"]);
        assert!(c.intervals.is_empty());
        assert!((c.level - 99.0).abs() < 1e-12);
        let c = one(
            "PROPORTIONS /INDEPENDENTSAMPLES t1 TO t4 BY g /SELECT=LEVEL('a','b') \
             /TESTTYPES=HAUCK_ANDERSON WALDH0 /SUCCESS=CUTPOINT(5).",
        );
        assert_eq!(
            c.analysis,
            ProportionsAnalysis::Independent {
                variables: vec![VarItem::Range("t1".into(), "t4".into())],
                group: "g".into(),
                select: ProportionsGroups::Levels(
                    Value::String("a".into()),
                    Value::String("b".into())
                ),
            }
        );
        assert_eq!(c.tests, ["HAUCK_ANDERSON", "WALDH0"]);
        assert_eq!(c.intervals, ["AGRESTI_CAFFO", "NEWCOMBE"]);
        assert_eq!(c.success, ProportionsSuccess::Cutpoint(5.0));
    }

    #[test]
    fn paired_lists() {
        let c = one(
            "PROPORTIONS /PAIREDSAMPLES p1 p2 WITH q1 q2 (PAIRED) CITYPES=AGRESTI_MIN WALD /SUCCESS=FIRST.",
        );
        assert_eq!(
            c.analysis,
            ProportionsAnalysis::Paired(TTestPairs {
                left: names(&["p1", "p2"]),
                right: Some(names(&["q1", "q2"])),
                paired: true
            })
        );
        assert_eq!(c.tests, ["MIDP", "MCNEMAR"]);
        assert_eq!(c.intervals, ["AGRESTI_MIN", "WALD"]);
        assert_eq!(c.success, ProportionsSuccess::First);
    }

    #[test]
    fn errors_name_the_problem() {
        assert!(err("PROPORTIONS /SUCCESS=FIRST.").contains("needs one of ONESAMPLE"));
        assert!(err("PROPORTIONS /ONESAMPLE a /ONESAMPLE b.").contains("one analysis"));
        assert!(err("PROPORTIONS /ONESAMPLE a TESTVAL=1.").contains("between 0 and 1"));
        assert!(err("PROPORTIONS /PAIREDSAMPLES a b /TESTVAL=0.2.").contains("TESTVAL belongs"));
        assert!(err("PROPORTIONS /ONESAMPLE a /SELECT=MIDPOINT.").contains("SELECT belongs"));
        assert!(
            err("PROPORTIONS /ONESAMPLE a TESTTYPES=MCNEMAR.").contains("not a TESTTYPES type")
        );
        assert!(err("PROPORTIONS /PAIREDSAMPLES a b TESTTYPES=MCN.").contains("could mean"));
        assert!(err("PROPORTIONS /ONESAMPLE a CITYPES=NONE WALD.").contains("cannot be combined"));
        assert!(err("PROPORTIONS /INDEPENDENTSAMPLES a /SELECT=MIDPOINT.").contains("BY"));
        assert!(
            err("PROPORTIONS /INDEPENDENTSAMPLES a BY g SELECT=LEVEL(1).").contains("two values")
        );
        assert!(
            err("PROPORTIONS /ONESAMPLE a /CRITERIA CILEVEL=100.").contains("between 0 and 100")
        );
        assert!(err("PROPORTIONS /ONESAMPLE a /FOO.").contains("unknown subcommand"));
    }
}
