//! Parser of UNIANOVA and GLM: the general linear model with one dependent variable, and GLM's
//! repeated measures form.
//!
//! Rules from the Command Syntax Reference (UNIANOVA, GLM, GLM: Repeated Measures):
//! - `dependent [BY factors] [WITH covariates]` comes first; the subcommands follow in any order.
//!   The last METHOD, INTERCEPT, MISSING, PRINT, SAVE and DESIGN apply; CRITERIA accumulates;
//!   EMMEANS, POSTHOC, CONTRAST and LMATRIX may repeat.
//! - DESIGN terms: INTERCEPT, a factor or covariate, `a*b` or `a BY b` for interactions, `a(b)`
//!   or `a WITHIN b` for nesting (nesting may nest: `a(b(c))`). Without DESIGN, or with an
//!   empty one: the intercept, the covariates, then the full factorial of the factors.
//! - METHOD=SSTYPE(1 to 4), default 3. CRITERIA: EPS(1E-8), ALPHA(0.05).
//! - GLM with several dependent variables is the multivariate model unless WSFACTOR turns them
//!   into the levels of within-subjects factors (the first factor's levels change slowest).
//! - CONTRAST(factor): DEVIATION (default), SIMPLE, DIFFERENCE, HELMERT, REPEATED, POLYNOMIAL,
//!   with the reference category or the metric in parentheses.

use crate::ast::{Command, Posthoc, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::stats::posthoc_test;

/// A contrast type of CONTRAST in UNIANOVA, GLM and LOGISTIC REGRESSION. The reference category
/// is the 1-based position among the categories in ascending order; None is the last.
#[derive(Debug, Clone, PartialEq)]
pub enum Contrast {
    Indicator(Option<u32>),
    Deviation(Option<u32>),
    Simple(Option<u32>),
    Difference,
    Helmert,
    Repeated,
    /// The metric, None for 1, 2, ..., k.
    Polynomial(Option<Vec<f64>>),
    /// The rows of the contrast matrix, read row by row.
    Special(Vec<f64>),
}

/// One effect of a design: its variables in the order written, factors and covariates alike
/// (a covariate may repeat), and its name as written, such as `a*b` or `b(a)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub variables: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlmPrint {
    pub descriptive: bool,
    pub homogeneity: bool,
    pub parameter: bool,
    pub etasq: bool,
    pub opower: bool,
    pub lof: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjustment {
    Lsd,
    Bonferroni,
    Sidak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Emmeans {
    /// The factors of TABLES; empty is OVERALL.
    pub factors: Vec<String>,
    /// `WITH(covariate=value|MEAN)`: None is MEAN.
    pub with: Vec<(String, Option<f64>)>,
    /// `COMPARE[(factor)]`: Some(None) when no factor is named.
    pub compare: Option<Option<String>>,
    pub adjustment: Adjustment,
}

/// One row of an L matrix: coefficients after an effect name, or after ALL (None).
pub type LRow = Vec<(Option<Term>, Vec<f64>)>;

#[derive(Debug, Clone, PartialEq)]
pub struct LMatrix {
    pub label: Option<String>,
    pub rows: Vec<LRow>,
}

/// The temporary variables of SAVE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlmTemp {
    Pred,
    Wpred,
    Resid,
    Wresid,
    Dresid,
    Zresid,
    Sresid,
    Sepred,
    Cook,
    Lever,
}

pub(crate) const GLM_TEMPS: [(&str, GlmTemp); 10] = [
    ("PRED", GlmTemp::Pred),
    ("WPRED", GlmTemp::Wpred),
    ("RESID", GlmTemp::Resid),
    ("WRESID", GlmTemp::Wresid),
    ("DRESID", GlmTemp::Dresid),
    ("ZRESID", GlmTemp::Zresid),
    ("SRESID", GlmTemp::Sresid),
    ("SEPRED", GlmTemp::Sepred),
    ("COOK", GlmTemp::Cook),
    ("LEVER", GlmTemp::Lever),
];

/// A within-subjects factor of WSFACTOR: its name, number of levels and contrast.
#[derive(Debug, Clone, PartialEq)]
pub struct WsFactor {
    pub name: String,
    pub levels: usize,
    pub contrast: Contrast,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Glm {
    /// The command name as run: UNIANOVA or GLM.
    pub command: &'static str,
    pub dependents: Vec<VarItem>,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub wsfactors: Vec<WsFactor>,
    /// WSDESIGN; None is the full factorial of the within-subjects factors.
    pub wsdesign: Option<Vec<Term>>,
    pub sstype: u8,
    /// `/INTERCEPT=INCLUDE` (default) or EXCLUDE.
    pub intercept: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    pub eps: f64,
    pub alpha: f64,
    pub print: GlmPrint,
    /// DESIGN terms; None is the default design. INTERCEPT among them is a term with no
    /// variables.
    pub design: Option<Vec<Term>>,
    pub regwgt: Option<String>,
    pub emmeans: Vec<Emmeans>,
    /// POSTHOC: the factors and the tests of each subcommand.
    pub posthoc: Vec<(Vec<String>, Vec<Posthoc>)>,
    pub contrasts: Vec<(String, Contrast)>,
    pub lmatrix: Vec<LMatrix>,
    pub kmatrix: Option<Vec<f64>>,
    pub save: Vec<(GlmTemp, Option<String>)>,
    /// Subcommands that ask for charts, which are not drawn yet.
    pub charts: Vec<String>,
}

/// A variable list that ends at one of the words of `stop`, such as BY and WITH.
pub(crate) fn list_until(p: &mut Parser, stop: &[&str]) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if stop.iter().any(|s| w.eq_ignore_ascii_case(s)) {
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

fn number_in_parens(p: &mut Parser) -> PResult<Option<f64>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(Some(n))
}

/// The contrast after `CONTRAST(var)=`. `indicator` allows INDICATOR (LOGISTIC REGRESSION).
pub(crate) fn contrast(p: &mut Parser, indicator: bool) -> PResult<Contrast> {
    let reference = |p: &mut Parser| -> PResult<Option<u32>> {
        let start = p.pos;
        match number_in_parens(p)? {
            None => Ok(None),
            Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(n as u32)),
            Some(n) => Err(SyntaxError::new(
                p.span_from(start),
                format!("the reference category is a whole number of at least 1, got {n}"),
            )),
        }
    };
    let numbers = |p: &mut Parser| -> PResult<Vec<f64>> {
        p.expect(Punct::LParen)?;
        let mut out = Vec::new();
        while let Some(n) = p.number() {
            out.push(n);
            p.eat(Punct::Comma);
        }
        p.expect(Punct::RParen)?;
        Ok(out)
    };
    if indicator && p.eat_keyword("INDICATOR") {
        return Ok(Contrast::Indicator(reference(p)?));
    }
    if p.eat_keyword("DEVIATION") {
        return Ok(Contrast::Deviation(reference(p)?));
    }
    if p.eat_keyword("SIMPLE") {
        return Ok(Contrast::Simple(reference(p)?));
    }
    if p.eat_keyword("DIFFERENCE") {
        return Ok(Contrast::Difference);
    }
    if p.eat_keyword("HELMERT") {
        return Ok(Contrast::Helmert);
    }
    if p.eat_keyword("REPEATED") {
        return Ok(Contrast::Repeated);
    }
    if p.eat_keyword("POLYNOMIAL") {
        if !p.at(Punct::LParen) {
            return Ok(Contrast::Polynomial(None));
        }
        let start = p.pos;
        let metric = numbers(p)?;
        if metric.windows(2).all(|w| w[0] == w[1]) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "the values of a POLYNOMIAL metric cannot all be equal",
            ));
        }
        return Ok(Contrast::Polynomial(Some(metric)));
    }
    if p.eat_keyword("SPECIAL") {
        let start = p.pos;
        let values = numbers(p)?;
        if values.is_empty() {
            return Err(SyntaxError::new(
                p.span_from(start),
                "SPECIAL needs the contrast matrix",
            ));
        }
        return Ok(Contrast::Special(values));
    }
    let mut expected = vec![
        "DEVIATION",
        "SIMPLE",
        "DIFFERENCE",
        "HELMERT",
        "REPEATED",
        "POLYNOMIAL",
        "SPECIAL",
    ];
    if indicator {
        expected.insert(0, "INDICATOR");
    }
    Err(p.unexpected(&expected))
}

/// `name`, `name(nested)` or `name WITHIN nested`, adding the names to `variables`; returns the
/// label.
fn nested(p: &mut Parser, variables: &mut Vec<String>) -> PResult<String> {
    let name = p.name("a factor or covariate")?;
    variables.push(name.clone());
    if p.eat(Punct::LParen) {
        let inner = joined(p, variables)?;
        p.expect(Punct::RParen)?;
        if p.at(Punct::LParen) {
            return Err(SyntaxError::new(
                p.span(),
                "each pair of parentheses must be nested in the one before, as in a(b(c))",
            ));
        }
        return Ok(format!("{name}({inner})"));
    }
    if p.eat_keyword("WITHIN") {
        let inner = nested(p, variables)?;
        return Ok(format!("{name}({inner})"));
    }
    Ok(name)
}

/// Parts joined by `*` or BY.
fn joined(p: &mut Parser, variables: &mut Vec<String>) -> PResult<String> {
    let mut label = nested(p, variables)?;
    while p.eat(Punct::Star) || p.eat_word("BY") {
        label.push('*');
        label.push_str(&nested(p, variables)?);
    }
    Ok(label)
}

pub(crate) fn term(p: &mut Parser) -> PResult<Term> {
    let mut variables = Vec::new();
    let label = joined(p, &mut variables)?;
    Ok(Term { variables, label })
}

/// Terms up to the end of the subcommand, INTERCEPT as a term with no variables.
fn terms(p: &mut Parser) -> PResult<Vec<Term>> {
    let mut out = Vec::new();
    while !at_end(p) {
        if p.eat_keyword("INTERCEPT") {
            out.push(Term {
                variables: Vec::new(),
                label: "Intercept".into(),
            });
        } else {
            out.push(term(p)?);
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// A factor list of EMMEANS TABLES or COMPARE: names joined by `*` or BY.
fn crossing(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = vec![p.name("a factor")?];
    while p.eat(Punct::Star) || p.eat_word("BY") {
        let start = p.pos;
        let name = p.name("a factor")?;
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} appears twice in a crossing of factors"),
            ));
        }
        names.push(name);
    }
    Ok(names)
}

fn emmeans(p: &mut Parser) -> PResult<Emmeans> {
    p.eat(Punct::Eq);
    if !p.eat_keyword("TABLES") {
        return Err(p.unexpected(&["TABLES"]));
    }
    p.expect(Punct::LParen)?;
    let factors = if p.eat_keyword("OVERALL") {
        Vec::new()
    } else {
        crossing(p)?
    };
    p.expect(Punct::RParen)?;
    let mut e = Emmeans {
        factors,
        with: Vec::new(),
        compare: None,
        adjustment: Adjustment::Lsd,
    };
    while !at_end(p) {
        if p.eat_word("WITH") {
            p.expect(Punct::LParen)?;
            while !p.at(Punct::RParen) {
                let name = p.name("a covariate")?;
                p.expect(Punct::Eq)?;
                let value = if p.eat_keyword("MEAN") {
                    None
                } else if let Some(n) = p.number() {
                    Some(n)
                } else {
                    return Err(p.unexpected(&["a number", "MEAN"]));
                };
                e.with.push((name, value));
                p.eat(Punct::Comma);
            }
            p.expect(Punct::RParen)?;
        } else if p.eat_keyword("COMPARE") {
            let start = p.pos;
            let factor = if p.eat(Punct::LParen) {
                let name = p.name("a factor")?;
                p.expect(Punct::RParen)?;
                Some(name)
            } else {
                None
            };
            if e.factors.is_empty() {
                return Err(SyntaxError::new(
                    p.span_from(start - 1),
                    "COMPARE cannot follow TABLES(OVERALL)",
                ));
            }
            if factor.is_none() && e.factors.len() > 1 {
                return Err(SyntaxError::new(
                    p.span_from(start - 1),
                    "COMPARE needs the factor to compare when TABLES crosses factors",
                ));
            }
            e.compare = Some(factor);
        } else if p.eat_keyword("ADJ") {
            p.expect(Punct::LParen)?;
            e.adjustment = if p.eat_keyword("LSD") {
                Adjustment::Lsd
            } else if p.eat_keyword("BONFERRONI") {
                Adjustment::Bonferroni
            } else if p.eat_keyword("SIDAK") {
                Adjustment::Sidak
            } else {
                return Err(p.unexpected(&["LSD", "BONFERRONI", "SIDAK"]));
            };
            p.expect(Punct::RParen)?;
        } else {
            return Err(p.unexpected(&["WITH", "COMPARE", "ADJ"]));
        }
    }
    Ok(e)
}

/// Numbers of an L or K matrix: `a`, `-a` or `a/b` with a positive b.
fn fraction(p: &mut Parser) -> PResult<Option<f64>> {
    let start = p.pos;
    let Some(n) = p.number() else {
        return Ok(None);
    };
    // a slash before a word starts the next subcommand
    if p.at(Punct::Slash) && !matches!(p.peek_at(1), Some(TokenKind::Word(_)) | None) {
        p.pos += 1;
        match p.peek() {
            Some(TokenKind::Number(d)) if *d > 0.0 => {
                let d = *d;
                p.pos += 1;
                return Ok(Some(n / d));
            }
            _ => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "a fraction needs a positive denominator, as in 1/3 or -1/3",
                ));
            }
        }
    }
    Ok(Some(n))
}

/// The end of an L or K matrix: the end of the command or a slash before a subcommand name.
fn matrix_end(p: &Parser) -> bool {
    match p.peek() {
        None => true,
        Some(TokenKind::Punct(Punct::Slash)) => {
            matches!(p.peek_at(1), Some(TokenKind::Word(_)) | None)
        }
        _ => false,
    }
}

fn lmatrix(p: &mut Parser) -> PResult<LMatrix> {
    p.eat(Punct::Eq);
    let label = match p.peek() {
        Some(TokenKind::Str(_)) => Some(p.string("a label")?),
        _ => None,
    };
    let mut rows = Vec::new();
    let mut row: LRow = Vec::new();
    while !matrix_end(p) {
        if p.eat(Punct::Semicolon) {
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
            }
            continue;
        }
        let effect = if p.eat_word("ALL") {
            None
        } else if p.eat_keyword("INTERCEPT") {
            Some(Term {
                variables: Vec::new(),
                label: "Intercept".into(),
            })
        } else {
            Some(term(p)?)
        };
        let mut values = Vec::new();
        while let Some(v) = fraction(p)? {
            values.push(v);
            p.eat(Punct::Comma);
        }
        if values.is_empty() {
            return Err(p.unexpected(&["a coefficient"]));
        }
        row.push((effect, values));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(p.unexpected(&["an effect name", "ALL"]));
    }
    Ok(LMatrix { label, rows })
}

fn kmatrix(p: &mut Parser) -> PResult<Vec<f64>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while !matrix_end(p) {
        if p.eat(Punct::Semicolon) {
            continue;
        }
        match fraction(p)? {
            Some(v) => out.push(v),
            None => return Err(p.unexpected(&["a number"])),
        }
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a number"]));
    }
    Ok(out)
}

fn print(p: &mut Parser) -> PResult<GlmPrint> {
    p.eat(Punct::Eq);
    let mut out = GlmPrint::default();
    const KEYWORDS: [&str; 7] = [
        "DESCRIPTIVE",
        "HOMOGENEITY",
        "PARAMETER",
        "ETASQ",
        "OPOWER",
        "LOF",
        "TEST",
    ];
    const LATER: [&str; 10] = [
        "GEF",
        "TEST",
        "MBP",
        "BP",
        "F",
        "WHITE",
        "RSSCP",
        "SSCP",
        "TRANSFORM",
        "ERROR",
    ];
    while !at_end(p) {
        let word = match p.peek() {
            Some(TokenKind::Word(w)) => w.to_uppercase(),
            _ => return Err(p.unexpected(&KEYWORDS[..6])),
        };
        if p.eat_keyword("DESCRIPTIVE") {
            out.descriptive = true;
        } else if p.eat_keyword("HOMOGENEITY") {
            out.homogeneity = true;
        } else if p.eat_keyword("PARAMETER") {
            out.parameter = true;
        } else if p.eat_keyword("ETASQ") {
            out.etasq = true;
        } else if p.eat_keyword("OPOWER") {
            out.opower = true;
        } else if p.eat_word("LOF") {
            out.lof = true;
        } else if let Some(name) = LATER
            .iter()
            .find(|k| word == **k || (k.len() > 3 && crate::table::matches_keyword(&word, k)))
        {
            return Err(SyntaxError::new(
                p.span(),
                format!("{name} in PRINT is not supported yet"),
            ));
        } else {
            return Err(p.unexpected(&KEYWORDS[..6]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn save(p: &mut Parser) -> PResult<Vec<(GlmTemp, Option<String>)>> {
    p.eat(Punct::Eq);
    let mut out: Vec<(GlmTemp, Option<String>)> = Vec::new();
    while !at_end(p) {
        let Some(&(_, temp)) = GLM_TEMPS.iter().find(|(n, _)| p.at_keyword(n)) else {
            let names: Vec<&str> = GLM_TEMPS.iter().map(|(n, _)| *n).collect();
            return Err(p.unexpected(&names));
        };
        let start = p.pos;
        p.pos += 1;
        let name = if p.eat(Punct::LParen) {
            let name = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            Some(name)
        } else {
            None
        };
        if out.iter().any(|(t, _)| *t == temp) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "a temporary variable is saved only once",
            ));
        }
        out.push((temp, name));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["PRED", "RESID", "a temporary variable"]));
    }
    Ok(out)
}

fn wsfactors(p: &mut Parser) -> PResult<Vec<WsFactor>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while !at_end(p) {
        let name = p.name("a within-subjects factor name")?;
        let start = p.pos;
        let levels = match p.number() {
            Some(n) if n >= 2.0 && n.fract() == 0.0 => n as usize,
            Some(n) => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!(
                        "a within-subjects factor has a whole number of at least 2 levels, got {n}"
                    ),
                ));
            }
            None => return Err(p.unexpected(&["the number of levels"])),
        };
        let contrast = if matches!(p.peek(), Some(TokenKind::Word(_)))
            && !matches!(p.peek_at(1), Some(TokenKind::Number(_)))
        {
            contrast(p, false)?
        } else {
            Contrast::Polynomial(None)
        };
        out.push(WsFactor {
            name,
            levels,
            contrast,
        });
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a within-subjects factor name"]));
    }
    Ok(out)
}

const SUBCOMMANDS_LATER: [&str; 12] = [
    "RANDOM",
    "TEST",
    "ROBUST",
    "OUTFILE",
    "MBPDESIGN",
    "BPDESIGN",
    "FDESIGN",
    "MMATRIX",
    "MEASURE",
    "SPLIT",
    "PLOT",
    "CONTRAST",
];

/// `UNIANOVA ...` and `GLM ...`.
pub(crate) fn glm_command(p: &mut Parser, name: &'static str) -> PResult<Command> {
    let dependents = list_until(p, &["BY", "WITH"])?;
    let factors = if p.eat_word("BY") {
        list_until(p, &["WITH"])?
    } else {
        Vec::new()
    };
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    let mut command = Glm {
        command: name,
        dependents,
        factors,
        covariates,
        wsfactors: Vec::new(),
        wsdesign: None,
        sstype: 3,
        intercept: true,
        include: false,
        eps: 1e-8,
        alpha: 0.05,
        print: GlmPrint::default(),
        design: None,
        regwgt: None,
        emmeans: Vec::new(),
        posthoc: Vec::new(),
        contrasts: Vec::new(),
        lmatrix: Vec::new(),
        kmatrix: None,
        save: Vec::new(),
        charts: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("SSTYPE") {
                return Err(p.unexpected(&["SSTYPE"]));
            }
            let at = p.pos;
            command.sstype = match number_in_parens(p)? {
                Some(n) if [1.0, 2.0, 3.0, 4.0].contains(&n) => n as u8,
                Some(n) => {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        format!("SSTYPE is 1, 2, 3 or 4, got {n}"),
                    ));
                }
                None => return Err(p.unexpected(&["(1)", "(2)", "(3)", "(4)"])),
            };
        } else if p.eat_keyword("INTERCEPT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("INCLUDE") {
                    command.intercept = true;
                } else if p.eat_keyword("EXCLUDE") {
                    command.intercept = false;
                } else {
                    return Err(p.unexpected(&["INCLUDE", "EXCLUDE"]));
                }
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else if p.eat_keyword("EXCLUDE") {
                    command.include = false;
                } else {
                    return Err(p.unexpected(&["INCLUDE", "EXCLUDE"]));
                }
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let eps = if p.eat_keyword("EPS") {
                    true
                } else if p.eat_keyword("ALPHA") {
                    false
                } else {
                    return Err(p.unexpected(&["EPS", "ALPHA"]));
                };
                let at = p.pos;
                let Some(value) = number_in_parens(p)? else {
                    return Err(p.unexpected(&["(value)"]));
                };
                if eps {
                    if value <= 0.0 {
                        return Err(SyntaxError::new(p.span_from(at), "EPS must be positive"));
                    }
                    command.eps = value;
                } else {
                    if !(value > 0.0 && value < 1.0) {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            format!("ALPHA must lie between 0 and 1, got {value}"),
                        ));
                    }
                    command.alpha = value;
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            command.print = print(p)?;
        } else if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = terms(p)?;
            command.design = (!terms.is_empty()).then_some(terms);
        } else if p.eat_keyword("WSFACTOR") {
            command.wsfactors = wsfactors(p)?;
        } else if p.eat_keyword("WSDESIGN") {
            p.eat(Punct::Eq);
            let terms = terms(p)?;
            command.wsdesign = (!terms.is_empty()).then_some(terms);
        } else if p.eat_keyword("REGWGT") {
            p.eat(Punct::Eq);
            command.regwgt = Some(p.name("a variable name")?);
        } else if p.eat_keyword("EMMEANS") {
            command.emmeans.push(emmeans(p)?);
        } else if p.eat_keyword("POSTHOC") {
            p.eat(Punct::Eq);
            let mut factors = Vec::new();
            while let Some(TokenKind::Word(_)) = p.peek() {
                factors.push(p.name("a factor")?);
                p.eat(Punct::Comma);
            }
            if factors.is_empty() {
                return Err(p.unexpected(&["a factor"]));
            }
            p.expect(Punct::LParen)?;
            let mut tests = Vec::new();
            while !p.at(Punct::RParen) {
                let test = posthoc_test(p, "POSTHOC", &[])?;
                if !tests.contains(&test) {
                    tests.push(test);
                }
                p.eat(Punct::Comma);
            }
            p.expect(Punct::RParen)?;
            if p.at_keyword("VS") {
                return Err(SyntaxError::new(
                    p.span(),
                    "VS in POSTHOC is not supported yet; the tests use the error of the model",
                ));
            }
            if tests.is_empty() {
                return Err(p.unexpected(&["a post hoc test"]));
            }
            command.posthoc.push((factors, tests));
        } else if p.eat_keyword("CONTRAST") {
            p.expect(Punct::LParen)?;
            let factor = p.name("a factor")?;
            p.expect(Punct::RParen)?;
            p.expect(Punct::Eq)?;
            if p.at_keyword("SPECIAL") {
                return Err(SyntaxError::new(
                    p.span(),
                    "SPECIAL in CONTRAST is not supported yet; LMATRIX tests any contrast",
                ));
            }
            let kind = contrast(p, false)?;
            command
                .contrasts
                .retain(|(f, _)| !f.eq_ignore_ascii_case(&factor));
            command.contrasts.push((factor, kind));
        } else if p.eat_keyword("LMATRIX") {
            command.lmatrix.push(lmatrix(p)?);
        } else if p.eat_keyword("KMATRIX") {
            command.kmatrix = Some(kmatrix(p)?);
        } else if p.eat_keyword("SAVE") {
            command.save = save(p)?;
        } else if p.eat_keyword("PLOT") {
            command.charts.push("PLOT".into());
            while !at_end(p) {
                p.pos += 1;
            }
        } else {
            let mut later: Vec<&str> = SUBCOMMANDS_LATER.to_vec();
            later.retain(|s| *s != "PLOT" && *s != "CONTRAST");
            return Err(p.subcommand_error(name, &later));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.factors.is_empty() && command.covariates.is_empty() && !command.intercept {
        let in_design = command
            .design
            .iter()
            .flatten()
            .any(|t| t.variables.is_empty());
        if !in_design {
            return Err(p.unexpected(&["BY factors", "WITH covariates", "INTERCEPT"]));
        }
    }
    Ok(Command::Glm(Box::new(command)))
}

pub(crate) fn unianova(p: &mut Parser) -> PResult<Command> {
    glm_command(p, "UNIANOVA")
}

pub(crate) fn glm(p: &mut Parser) -> PResult<Command> {
    glm_command(p, "GLM")
}
