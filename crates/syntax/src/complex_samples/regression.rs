//! Parsers of the complex samples regression commands: CSGLM (linear model) and CSLOGISTIC
//! (binary and multinomial logistic model).
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The variable list comes first: the dependent variable, for CSLOGISTIC optionally followed by
//!   its reference category in parentheses (LOW, HIGH, the default, or a value), then factors
//!   after BY and covariates after WITH. PLAN FILE= is required; everything else is optional.
//! - CUSTOM may repeat, and so may EMMEANS (CSGLM only) and ODDSRATIOS (CSLOGISTIC only); every
//!   other subcommand comes at most once, and a keyword at most once within its subcommand.
//!   Only EMMEANS may be empty. The equals signs and square brackets of the syntax charts are
//!   required.
//! - MODEL lists effects as in UNIANOVA: factors crossed with `*` or BY, nesting in parentheses,
//!   covariates crossed with themselves or with factors but never nested. INTERCEPT INCLUDE=YES
//!   (default), NO or ONLY, and SHOW=YES or NO; ONLY cannot go with MODEL.
//! - CUSTOM: LABEL='text', LMATRIX= rows separated by `;`, each an optional intercept
//!   coefficient followed by effects with their coefficients, or ALL with a coefficient for
//!   every parameter; KMATRIX= one value per row, separated by `;`. One of the two matrices is
//!   needed. Coefficients may be fractions such as 1/3.
//! - EMMEANS TABLES=factor[*factor...] OTHER=[covariate(value|MEAN) ...] COMPARE=factor
//!   CONTRAST=SIMPLE[(value)] | DEVIATION[(value)] | DIFFERENCE | HELMERT | REPEATED |
//!   POLYNOMIAL[(metric)].
//! - ODDSRATIOS FACTOR=[factor(LOW|HIGH|value) ...] or COVARIATE=[covariate(changes) ...] (one
//!   of the two), and CONTROL=[factor(LOW|HIGH|value) covariate(MEAN|value) ...].
//! - CRITERIA CILEVEL (0 to below 100), DF (above 0), SINGULAR (above 0); CSLOGISTIC also
//!   CHKSEP, LCONVERGE=[value RELATIVE|ABSOLUTE], MXITER, MXSTEP and PCONVERGE=[...].
//! - STATISTICS PARAMETER EXP (CSLOGISTIC) SE TTEST CINTERVAL DEFF DEFFSQRT. TEST TYPE=F | ADJF |
//!   CHISQUARE | ADJCHISQUARE and PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK.
//! - DOMAIN VARIABLE=name(value). MISSING CLASSMISSING=EXCLUDE | INCLUDE.
//! - PRINT SAMPLEINFO VARIABLEINFO SUMMARY GEF LMATRIX COVB CORB NONE, CSLOGISTIC also
//!   HISTORY[(n)] and CLASSTABLE; a PRINT shows only what it names, and NONE gives way to any
//!   other keyword.
//! - SAVE: CSGLM PRED[(name)] RESID[(name)]; CSLOGISTIC PREDPROB[(root[:n])] PREDVAL[(name)].

use super::{eat_assignment, error, once, plan_file, require_assignment};
use crate::ast::{Command, Value, VarItem};
use crate::glm::{LRow, Term, fraction, list_until, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// A reference category: the lowest, the highest or a given value.
#[derive(Debug, Clone, PartialEq)]
pub enum CsReference {
    Low,
    High,
    Value(Value),
}

/// INTERCEPT INCLUDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsIntercept {
    Yes,
    No,
    Only,
}

/// One CUSTOM subcommand. A row's leading intercept coefficient is an entry for an effect with
/// no variables; ALL is an entry without an effect.
#[derive(Debug, Clone, PartialEq)]
pub struct CsCustom {
    pub label: Option<String>,
    pub rows: Vec<LRow>,
    pub k: Option<Vec<f64>>,
}

/// The contrast of EMMEANS COMPARE; a reference value picks the omitted level.
#[derive(Debug, Clone, PartialEq)]
pub enum CsContrast {
    Simple(Option<Value>),
    Deviation(Option<Value>),
    Difference,
    Helmert,
    Repeated,
    Polynomial(Option<Vec<f64>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsEmmeans {
    /// The factors of TABLES; empty is the overall mean.
    pub tables: Vec<String>,
    /// OTHER: each covariate with its value, None for MEAN.
    pub other: Vec<(String, Option<f64>)>,
    /// COMPARE: the factor, or an empty name when COMPARE stands alone.
    pub compare: Option<String>,
    pub contrast: CsContrast,
}

/// A value of ODDSRATIOS CONTROL: LOW, HIGH or a value for a factor, MEAN or a number for a
/// covariate.
#[derive(Debug, Clone, PartialEq)]
pub enum CsControl {
    Low,
    High,
    Mean,
    Value(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsOddsRatios {
    pub factors: Vec<(String, CsReference)>,
    /// Each covariate with its changes (1 when none are given).
    pub covariates: Vec<(String, Vec<f64>)>,
    pub control: Vec<(String, CsControl)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsTestType {
    F,
    AdjF,
    ChiSquare,
    AdjChiSquare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsPadjust {
    Lsd,
    Bonferroni,
    SeqBonferroni,
    Sidak,
    SeqSidak,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CsModelStatistics {
    pub parameter: bool,
    pub exp: bool,
    pub se: bool,
    pub ttest: bool,
    pub cinterval: bool,
    pub deff: bool,
    pub deffsqrt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CsCriteria {
    pub cilevel: f64,
    pub df: Option<f64>,
    pub singular: f64,
    pub chksep: u32,
    /// (value, relative)
    pub lconverge: (f64, bool),
    pub mxiter: u32,
    pub mxstep: u32,
    pub pconverge: (f64, bool),
}

impl Default for CsCriteria {
    fn default() -> Self {
        Self {
            cilevel: 95.0,
            df: None,
            singular: 1e-12,
            chksep: 20,
            lconverge: (0.0, true),
            mxiter: 100,
            mxstep: 5,
            pconverge: (1e-6, true),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CsModelPrint {
    pub sampleinfo: bool,
    pub variableinfo: bool,
    pub summary: bool,
    /// HISTORY(n): every nth iteration.
    pub history: Option<u32>,
    pub lmatrix: bool,
    pub covb: bool,
    pub corb: bool,
    pub classtable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CsModelSave {
    Pred(Option<String>),
    Resid(Option<String>),
    /// The root name and how many categories get a variable.
    PredProb(Option<String>, u32),
    PredVal(Option<String>),
}

/// CSGLM or CSLOGISTIC.
#[derive(Debug, Clone, PartialEq)]
pub struct CsModel {
    pub logistic: bool,
    pub dependent: String,
    /// The reference category of the dependent variable (CSLOGISTIC).
    pub reference: CsReference,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub plan: String,
    pub model: Option<Vec<Term>>,
    pub intercept: CsIntercept,
    pub show_intercept: bool,
    pub custom: Vec<CsCustom>,
    pub emmeans: Vec<CsEmmeans>,
    pub oddsratios: Vec<CsOddsRatios>,
    pub criteria: CsCriteria,
    /// None when STATISTICS is not given.
    pub statistics: Option<CsModelStatistics>,
    pub test: CsTestType,
    pub padjust: CsPadjust,
    pub domain: Option<(String, Value)>,
    /// CLASSMISSING=INCLUDE
    pub include: bool,
    pub print: CsModelPrint,
    pub save: Vec<CsModelSave>,
}

impl CsModel {
    pub fn name(&self) -> &'static str {
        if self.logistic { "CSLOGISTIC" } else { "CSGLM" }
    }
}

/// `(LOW | HIGH | value)` after a variable name, when there is one.
fn reference(p: &mut Parser) -> PResult<Option<CsReference>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let out = if p.eat_keyword("LOW") {
        CsReference::Low
    } else if p.eat_keyword("HIGH") {
        CsReference::High
    } else if let Some(v) = p.value() {
        CsReference::Value(v)
    } else {
        return Err(p.unexpected(&["LOW", "HIGH", "a value"]));
    };
    p.expect(Punct::RParen)?;
    Ok(Some(out))
}

/// `[ name(...) name(...) ... ]`, each element read by `element`.
fn bracketed<T>(
    p: &mut Parser,
    mut element: impl FnMut(&mut Parser, String) -> PResult<T>,
) -> PResult<Vec<T>> {
    p.expect(Punct::LBracket)?;
    let mut out = Vec::new();
    let mut names: Vec<String> = Vec::new();
    while !p.at(Punct::RBracket) {
        let start = p.pos;
        let name = p.name("a variable name")?;
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            return Err(error(p, start, format!("{name} appears twice in the list")));
        }
        names.push(name.clone());
        out.push(element(p, name)?);
        p.eat(Punct::Comma);
    }
    p.expect(Punct::RBracket)?;
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(out)
}

/// A number that must satisfy `ok`, described by `rule` in the error.
fn checked(p: &mut Parser, what: &str, rule: &str, ok: impl Fn(f64) -> bool) -> PResult<f64> {
    let start = p.pos;
    let Some(v) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    if !ok(v) {
        return Err(error(p, start, format!("{what} {rule}, got {v}")));
    }
    Ok(v)
}

fn whole(p: &mut Parser, what: &str, min: f64) -> PResult<u32> {
    let rule = if min > 0.0 {
        "is a positive whole number"
    } else {
        "is a whole number of at least 0"
    };
    let v = checked(p, what, rule, |v| v >= min && v.fract() == 0.0 && v < 1e9)?;
    Ok(v as u32)
}

/// `[value RELATIVE | ABSOLUTE]` of LCONVERGE and PCONVERGE.
fn convergence(p: &mut Parser, what: &str) -> PResult<(f64, bool)> {
    p.expect(Punct::LBracket)?;
    let v = checked(p, what, "is at least 0", |v| v >= 0.0)?;
    let relative = if p.eat_keyword("ABSOLUTE") {
        false
    } else {
        p.eat_keyword("RELATIVE");
        true
    };
    p.expect(Punct::RBracket)?;
    Ok((v, relative))
}

/// The LMATRIX rows, up to the next `KEYWORD =` or the end of the subcommand.
fn lmatrix(p: &mut Parser) -> PResult<Vec<LRow>> {
    let mut rows = Vec::new();
    let mut row: LRow = Vec::new();
    let at_keyword = |p: &Parser| {
        matches!(p.peek(), Some(TokenKind::Word(_)))
            && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
    };
    while !at_end(p) && !at_keyword(p) {
        if p.eat(Punct::Semicolon) {
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
            }
            continue;
        }
        let leading_number = matches!(p.peek(), Some(TokenKind::Number(_))) || p.at(Punct::Minus);
        let effect = if row.is_empty() && leading_number {
            // the coefficient of the intercept, first in the row
            let start = p.pos;
            let Some(v) = fraction(p)? else {
                return Err(p.unexpected(&["a coefficient"]));
            };
            if matches!(p.peek(), Some(TokenKind::Number(_))) || p.at(Punct::Minus) {
                return Err(error(
                    p,
                    start,
                    "a row starts with one coefficient for the intercept, then effect names with their coefficients, or ALL",
                ));
            }
            row.push((
                Some(Term {
                    variables: Vec::new(),
                    label: "Intercept".into(),
                }),
                vec![v],
            ));
            continue;
        } else if p.eat_word("ALL") {
            None
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
        return Err(p.unexpected(&["a coefficient", "an effect name", "ALL"]));
    }
    Ok(rows)
}

fn custom(p: &mut Parser) -> PResult<CsCustom> {
    let start = p.pos;
    let mut c = CsCustom {
        label: None,
        rows: Vec::new(),
        k: None,
    };
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "LABEL")? {
            once(p, at, &mut seen, "LABEL")?;
            c.label = Some(p.string("a quoted label")?);
        } else if eat_assignment(p, "LMATRIX")? {
            once(p, at, &mut seen, "LMATRIX")?;
            c.rows = lmatrix(p)?;
        } else if eat_assignment(p, "KMATRIX")? {
            once(p, at, &mut seen, "KMATRIX")?;
            let mut k = Vec::new();
            loop {
                match fraction(p)? {
                    Some(v) => k.push(v),
                    None => return Err(p.unexpected(&["a number"])),
                }
                if !p.eat(Punct::Semicolon) {
                    break;
                }
            }
            c.k = Some(k);
        } else {
            return Err(p.unexpected(&["LABEL =", "LMATRIX =", "KMATRIX ="]));
        }
    }
    if c.rows.is_empty() && c.k.is_none() {
        return Err(error(p, start, "CUSTOM needs LMATRIX= or KMATRIX="));
    }
    if let Some(k) = &c.k
        && !c.rows.is_empty()
        && k.len() != c.rows.len()
    {
        return Err(error(
            p,
            start,
            format!(
                "KMATRIX gives {} values for {} rows of LMATRIX",
                k.len(),
                c.rows.len()
            ),
        ));
    }
    Ok(c)
}

/// The value of SIMPLE or DEVIATION, when given.
fn contrast_value(p: &mut Parser) -> PResult<Option<Value>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let Some(v) = p.value() else {
        return Err(p.unexpected(&["a value"]));
    };
    p.expect(Punct::RParen)?;
    Ok(Some(v))
}

fn emmeans(p: &mut Parser) -> PResult<CsEmmeans> {
    let mut e = CsEmmeans {
        tables: Vec::new(),
        other: Vec::new(),
        compare: None,
        contrast: CsContrast::Simple(None),
    };
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "TABLES")? {
            once(p, at, &mut seen, "TABLES")?;
            e.tables.push(p.name("a factor")?);
            while p.eat(Punct::Star) || p.eat_word("BY") {
                let start = p.pos;
                let name = p.name("a factor")?;
                if e.tables.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    return Err(error(p, start, format!("{name} appears twice in TABLES")));
                }
                e.tables.push(name);
            }
        } else if eat_assignment(p, "OTHER")? {
            once(p, at, &mut seen, "OTHER")?;
            e.other = bracketed(p, |p, name| {
                p.expect(Punct::LParen)?;
                let value = if p.eat_keyword("MEAN") {
                    None
                } else if let Some(v) = p.number() {
                    Some(v)
                } else {
                    return Err(p.unexpected(&["a number", "MEAN"]));
                };
                p.expect(Punct::RParen)?;
                Ok((name, value))
            })?;
        } else if eat_assignment(p, "COMPARE")? {
            once(p, at, &mut seen, "COMPARE")?;
            e.compare = Some(p.name("a factor")?);
        } else if p.eat_keyword("COMPARE") {
            // COMPARE alone: the only factor of TABLES (an empty name here)
            once(p, at, &mut seen, "COMPARE")?;
            e.compare = Some(String::new());
        } else if eat_assignment(p, "CONTRAST")? {
            once(p, at, &mut seen, "CONTRAST")?;
            e.contrast = if p.eat_keyword("SIMPLE") {
                CsContrast::Simple(contrast_value(p)?)
            } else if p.eat_keyword("DEVIATION") {
                CsContrast::Deviation(contrast_value(p)?)
            } else if p.eat_keyword("DIFFERENCE") {
                CsContrast::Difference
            } else if p.eat_keyword("HELMERT") {
                CsContrast::Helmert
            } else if p.eat_keyword("REPEATED") {
                CsContrast::Repeated
            } else if p.eat_keyword("POLYNOMIAL") {
                let mut metric = None;
                if p.eat(Punct::LParen) {
                    let start = p.pos;
                    let mut values: Vec<f64> = Vec::new();
                    while let Some(v) = p.number() {
                        values.push(v);
                        p.eat(Punct::Comma);
                    }
                    p.expect(Punct::RParen)?;
                    let mut sorted = values.clone();
                    sorted.sort_by(f64::total_cmp);
                    if values.is_empty() || sorted.windows(2).any(|w| w[0] == w[1]) {
                        return Err(error(
                            p,
                            start,
                            "a POLYNOMIAL metric gives one distinct number per level",
                        ));
                    }
                    metric = Some(values);
                }
                CsContrast::Polynomial(metric)
            } else {
                return Err(p.unexpected(&[
                    "SIMPLE",
                    "DEVIATION",
                    "DIFFERENCE",
                    "HELMERT",
                    "REPEATED",
                    "POLYNOMIAL",
                ]));
            };
        } else {
            return Err(p.unexpected(&["TABLES =", "OTHER =", "COMPARE =", "CONTRAST ="]));
        }
    }
    if e.compare.is_some() && e.tables.is_empty() {
        return Err(error(p, p.pos, "EMMEANS COMPARE needs TABLES"));
    }
    Ok(e)
}

fn oddsratios(p: &mut Parser) -> PResult<CsOddsRatios> {
    let start = p.pos;
    let mut o = CsOddsRatios {
        factors: Vec::new(),
        covariates: Vec::new(),
        control: Vec::new(),
    };
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "FACTOR")? {
            once(p, at, &mut seen, "FACTOR")?;
            o.factors = bracketed(p, |p, name| {
                Ok((name, reference(p)?.unwrap_or(CsReference::High)))
            })?;
        } else if eat_assignment(p, "COVARIATE")? {
            once(p, at, &mut seen, "COVARIATE")?;
            o.covariates = bracketed(p, |p, name| {
                let mut changes = Vec::new();
                if p.eat(Punct::LParen) {
                    while !p.at(Punct::RParen) {
                        let v = checked(p, "a change of a covariate", "is not 0", |v| v != 0.0)?;
                        if !changes.contains(&v) {
                            changes.push(v);
                        }
                        p.eat(Punct::Comma);
                    }
                    p.expect(Punct::RParen)?;
                }
                if changes.is_empty() {
                    changes.push(1.0);
                }
                Ok((name, changes))
            })?;
        } else if eat_assignment(p, "CONTROL")? {
            once(p, at, &mut seen, "CONTROL")?;
            o.control = bracketed(p, |p, name| {
                p.expect(Punct::LParen)?;
                let value = if p.eat_keyword("LOW") {
                    CsControl::Low
                } else if p.eat_keyword("HIGH") {
                    CsControl::High
                } else if p.eat_keyword("MEAN") {
                    CsControl::Mean
                } else if let Some(v) = p.value() {
                    CsControl::Value(v)
                } else {
                    return Err(p.unexpected(&["LOW", "HIGH", "MEAN", "a value"]));
                };
                p.expect(Punct::RParen)?;
                Ok((name, value))
            })?;
        } else {
            return Err(p.unexpected(&["FACTOR =", "COVARIATE =", "CONTROL ="]));
        }
    }
    if o.factors.is_empty() == o.covariates.is_empty() {
        return Err(error(
            p,
            start,
            "ODDSRATIOS needs FACTOR=[...] or COVARIATE=[...], not both",
        ));
    }
    Ok(o)
}

fn criteria(p: &mut Parser, c: &mut CsCriteria, logistic: bool) -> PResult<()> {
    let start = p.pos;
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "CRITERIA needs at least one keyword"));
    }
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "CILEVEL")? {
            once(p, at, &mut seen, "CILEVEL")?;
            c.cilevel = checked(p, "CILEVEL", "is at least 0 and below 100", |v| {
                (0.0..100.0).contains(&v)
            })?;
        } else if eat_assignment(p, "DF")? {
            once(p, at, &mut seen, "DF")?;
            c.df = Some(checked(p, "DF", "is above 0", |v| v > 0.0)?);
        } else if eat_assignment(p, "SINGULAR")? {
            once(p, at, &mut seen, "SINGULAR")?;
            c.singular = checked(p, "SINGULAR", "is above 0", |v| v > 0.0)?;
        } else if logistic && eat_assignment(p, "CHKSEP")? {
            once(p, at, &mut seen, "CHKSEP")?;
            c.chksep = whole(p, "CHKSEP", 0.0)?;
        } else if logistic && eat_assignment(p, "LCONVERGE")? {
            once(p, at, &mut seen, "LCONVERGE")?;
            c.lconverge = convergence(p, "LCONVERGE")?;
        } else if logistic && eat_assignment(p, "MXITER")? {
            once(p, at, &mut seen, "MXITER")?;
            c.mxiter = whole(p, "MXITER", 0.0)?;
        } else if logistic && eat_assignment(p, "MXSTEP")? {
            once(p, at, &mut seen, "MXSTEP")?;
            c.mxstep = whole(p, "MXSTEP", 1.0)?;
        } else if logistic && eat_assignment(p, "PCONVERGE")? {
            once(p, at, &mut seen, "PCONVERGE")?;
            c.pconverge = convergence(p, "PCONVERGE")?;
        } else if logistic {
            return Err(p.unexpected(&[
                "CHKSEP =",
                "CILEVEL =",
                "DF =",
                "LCONVERGE =",
                "MXITER =",
                "MXSTEP =",
                "PCONVERGE =",
                "SINGULAR =",
            ]));
        } else {
            return Err(p.unexpected(&["CILEVEL =", "DF =", "SINGULAR ="]));
        }
    }
    Ok(())
}

/// One keyword of `table`: written in full, or shortened to a prefix that fits only one of them.
fn pick<T: Copy>(p: &mut Parser, table: &[(&str, T)]) -> PResult<Option<T>> {
    let Some(TokenKind::Word(w)) = p.peek() else {
        return Ok(None);
    };
    if let Some((_, t)) = table.iter().find(|(k, _)| w.eq_ignore_ascii_case(k)) {
        p.pos += 1;
        return Ok(Some(*t));
    }
    let hits: Vec<&(&str, T)> = table.iter().filter(|(k, _)| p.at_keyword(k)).collect();
    match hits.as_slice() {
        [] => Ok(None),
        [(_, t)] => {
            p.pos += 1;
            Ok(Some(*t))
        }
        _ => {
            let names: Vec<&str> = hits.iter().map(|(k, _)| *k).collect();
            Err(error(
                p,
                p.pos,
                format!(
                    "{} is short for more than one keyword ({}); write more of it",
                    w.to_uppercase(),
                    names.join(", ")
                ),
            ))
        }
    }
}

/// A keyword of a list subcommand, given at most once.
fn flag(p: &mut Parser, seen: &mut Vec<&'static str>, keyword: &'static str) -> PResult<bool> {
    let at = p.pos;
    if !p.eat_keyword(keyword) {
        return Ok(false);
    }
    once(p, at, seen, keyword)?;
    Ok(true)
}

fn statistics(p: &mut Parser, logistic: bool) -> PResult<CsModelStatistics> {
    let start = p.pos;
    let mut s = CsModelStatistics::default();
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "STATISTICS needs at least one keyword"));
    }
    let mut keywords = vec!["PARAMETER", "SE", "TTEST", "CINTERVAL", "DEFF", "DEFFSQRT"];
    if logistic {
        keywords.insert(1, "EXP");
    }
    let table: Vec<(&str, &'static str)> = keywords.iter().map(|k| (*k, *k)).collect();
    while !at_end(p) {
        let at = p.pos;
        let Some(keyword) = pick(p, &table)? else {
            return Err(p.unexpected(&keywords));
        };
        once(p, at, &mut seen, keyword)?;
        match keyword {
            "PARAMETER" => s.parameter = true,
            "EXP" => s.exp = true,
            "SE" => s.se = true,
            "TTEST" => s.ttest = true,
            "CINTERVAL" => s.cinterval = true,
            "DEFF" => s.deff = true,
            _ => s.deffsqrt = true,
        }
        p.eat(Punct::Comma);
    }
    Ok(s)
}

fn print(p: &mut Parser, logistic: bool) -> PResult<CsModelPrint> {
    let start = p.pos;
    let mut out = CsModelPrint::default();
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "PRINT needs at least one keyword"));
    }
    while !at_end(p) {
        let at = p.pos;
        if flag(p, &mut seen, "SAMPLEINFO")? {
            out.sampleinfo = true;
        } else if flag(p, &mut seen, "VARIABLEINFO")? {
            out.variableinfo = true;
        } else if flag(p, &mut seen, "SUMMARY")? {
            out.summary = true;
        } else if flag(p, &mut seen, "LMATRIX")? {
            out.lmatrix = true;
        } else if flag(p, &mut seen, "COVB")? {
            out.covb = true;
        } else if flag(p, &mut seen, "CORB")? {
            out.corb = true;
        } else if flag(p, &mut seen, "NONE")? {
        } else if logistic && flag(p, &mut seen, "CLASSTABLE")? {
            out.classtable = true;
        } else if logistic && flag(p, &mut seen, "HISTORY")? {
            let mut every = 1;
            if p.eat(Punct::LParen) {
                every = whole(p, "HISTORY", 1.0)?;
                p.expect(Punct::RParen)?;
            }
            out.history = Some(every);
        } else if p.eat_keyword("GEF") {
            return Err(error(
                p,
                at,
                "PRINT GEF (the general estimable function) is not supported yet",
            ));
        } else if logistic {
            return Err(p.unexpected(&[
                "SAMPLEINFO",
                "VARIABLEINFO",
                "SUMMARY",
                "HISTORY",
                "GEF",
                "LMATRIX",
                "COVB",
                "CORB",
                "CLASSTABLE",
                "NONE",
            ]));
        } else {
            return Err(p.unexpected(&[
                "SAMPLEINFO",
                "VARIABLEINFO",
                "SUMMARY",
                "GEF",
                "LMATRIX",
                "COVB",
                "CORB",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// `(name)` after a SAVE keyword, when given.
fn save_name(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

fn save(p: &mut Parser, logistic: bool) -> PResult<Vec<CsModelSave>> {
    let start = p.pos;
    let mut out = Vec::new();
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "SAVE needs at least one keyword"));
    }
    let keywords: &[&'static str] = if logistic {
        &["PREDPROB", "PREDVAL"]
    } else {
        &["PRED", "RESID"]
    };
    let table: Vec<(&str, &'static str)> = keywords.iter().map(|k| (*k, *k)).collect();
    while !at_end(p) {
        let at = p.pos;
        let Some(keyword) = pick(p, &table)? else {
            return Err(p.unexpected(keywords));
        };
        once(p, at, &mut seen, keyword)?;
        if keyword == "PRED" {
            out.push(CsModelSave::Pred(save_name(p)?));
        } else if keyword == "RESID" {
            out.push(CsModelSave::Resid(save_name(p)?));
        } else if keyword == "PREDPROB" {
            let (mut root, mut count) = (None, 25);
            if p.eat(Punct::LParen) {
                if !p.at(Punct::Colon) {
                    root = Some(p.name("a root name")?);
                }
                if p.eat(Punct::Colon) {
                    count = whole(p, "the number of predicted probabilities", 1.0)?;
                }
                p.expect(Punct::RParen)?;
            }
            out.push(CsModelSave::PredProb(root, count));
        } else {
            out.push(CsModelSave::PredVal(save_name(p)?));
        }
        p.eat(Punct::Comma);
    }
    let names: Vec<String> = out
        .iter()
        .filter_map(|s| match s {
            // a PREDPROB root gets suffixes, so it cannot clash
            CsModelSave::Pred(n) | CsModelSave::Resid(n) | CsModelSave::PredVal(n) => {
                n.as_ref().map(|n| n.to_uppercase())
            }
            CsModelSave::PredProb(..) => None,
        })
        .collect();
    if let Some(i) = (1..names.len()).find(|&i| names[..i].contains(&names[i])) {
        return Err(error(p, start, format!("SAVE names {} twice", names[i])));
    }
    Ok(out)
}

fn model(p: &mut Parser, logistic: bool) -> PResult<Command> {
    let command = if logistic { "CSLOGISTIC" } else { "CSGLM" };
    let dependent = p.name("the dependent variable")?;
    let mut c = CsModel {
        logistic,
        dependent,
        reference: CsReference::High,
        factors: Vec::new(),
        covariates: Vec::new(),
        plan: String::new(),
        model: None,
        intercept: CsIntercept::Yes,
        show_intercept: true,
        custom: Vec::new(),
        emmeans: Vec::new(),
        oddsratios: Vec::new(),
        criteria: CsCriteria::default(),
        statistics: None,
        test: CsTestType::F,
        padjust: CsPadjust::Lsd,
        domain: None,
        include: false,
        print: CsModelPrint {
            sampleinfo: true,
            variableinfo: true,
            summary: true,
            ..CsModelPrint::default()
        },
        save: Vec::new(),
    };
    if logistic && let Some(r) = reference(p)? {
        c.reference = r;
    }
    if matches!(p.peek(), Some(TokenKind::Word(w))
        if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    {
        return Err(error(
            p,
            p.pos,
            format!(
                "{command} takes one dependent variable; factors follow BY and covariates WITH"
            ),
        ));
    }
    if p.eat_word("BY") {
        c.factors = list_until(p, &["WITH"])?;
    }
    if p.eat_word("WITH") {
        c.covariates = p.var_list()?;
    }
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            c.plan = plan_file(p)?;
        } else if p.eat_keyword("MODEL") {
            once(p, start, &mut seen, "MODEL")?;
            let mut terms = Vec::new();
            while !at_end(p) {
                terms.push(term(p)?);
                p.eat(Punct::Comma);
            }
            if terms.is_empty() {
                return Err(error(p, start, "MODEL needs at least one effect"));
            }
            c.model = Some(terms);
        } else if p.eat_keyword("INTERCEPT") {
            once(p, start, &mut seen, "INTERCEPT")?;
            if at_end(p) {
                return Err(error(p, start, "INTERCEPT needs INCLUDE= or SHOW="));
            }
            let mut inner = Vec::new();
            while !at_end(p) {
                let at = p.pos;
                if eat_assignment(p, "INCLUDE")? {
                    once(p, at, &mut inner, "INCLUDE")?;
                    c.intercept = if p.eat_keyword("YES") {
                        CsIntercept::Yes
                    } else if p.eat_keyword("NO") {
                        CsIntercept::No
                    } else if p.eat_keyword("ONLY") {
                        CsIntercept::Only
                    } else {
                        return Err(p.unexpected(&["YES", "NO", "ONLY"]));
                    };
                } else if eat_assignment(p, "SHOW")? {
                    once(p, at, &mut inner, "SHOW")?;
                    c.show_intercept = if p.eat_keyword("YES") {
                        true
                    } else if p.eat_keyword("NO") {
                        false
                    } else {
                        return Err(p.unexpected(&["YES", "NO"]));
                    };
                } else {
                    return Err(p.unexpected(&["INCLUDE =", "SHOW ="]));
                }
            }
        } else if p.eat_keyword("CUSTOM") {
            c.custom.push(custom(p)?);
        } else if !logistic && p.eat_keyword("EMMEANS") {
            c.emmeans.push(emmeans(p)?);
        } else if logistic && p.eat_keyword("ODDSRATIOS") {
            c.oddsratios.push(oddsratios(p)?);
        } else if p.eat_keyword("CRITERIA") {
            once(p, start, &mut seen, "CRITERIA")?;
            criteria(p, &mut c.criteria, logistic)?;
        } else if p.eat_keyword("STATISTICS") {
            once(p, start, &mut seen, "STATISTICS")?;
            c.statistics = Some(statistics(p, logistic)?);
        } else if p.eat_keyword("TEST") {
            once(p, start, &mut seen, "TEST")?;
            if at_end(p) {
                return Err(error(p, start, "TEST needs TYPE= or PADJUST="));
            }
            let mut inner = Vec::new();
            while !at_end(p) {
                let at = p.pos;
                if eat_assignment(p, "TYPE")? {
                    once(p, at, &mut inner, "TYPE")?;
                    const TYPES: [(&str, CsTestType); 4] = [
                        ("F", CsTestType::F),
                        ("ADJF", CsTestType::AdjF),
                        ("CHISQUARE", CsTestType::ChiSquare),
                        ("ADJCHISQUARE", CsTestType::AdjChiSquare),
                    ];
                    let Some(t) = pick(p, &TYPES)? else {
                        return Err(p.unexpected(&["F", "ADJF", "CHISQUARE", "ADJCHISQUARE"]));
                    };
                    c.test = t;
                } else if eat_assignment(p, "PADJUST")? {
                    once(p, at, &mut inner, "PADJUST")?;
                    const METHODS: [(&str, CsPadjust); 5] = [
                        ("LSD", CsPadjust::Lsd),
                        ("BONFERRONI", CsPadjust::Bonferroni),
                        ("SEQBONFERRONI", CsPadjust::SeqBonferroni),
                        ("SIDAK", CsPadjust::Sidak),
                        ("SEQSIDAK", CsPadjust::SeqSidak),
                    ];
                    let Some(m) = pick(p, &METHODS)? else {
                        return Err(p.unexpected(&[
                            "LSD",
                            "BONFERRONI",
                            "SEQBONFERRONI",
                            "SIDAK",
                            "SEQSIDAK",
                        ]));
                    };
                    c.padjust = m;
                } else {
                    return Err(p.unexpected(&["TYPE =", "PADJUST ="]));
                }
            }
        } else if p.eat_keyword("DOMAIN") {
            once(p, start, &mut seen, "DOMAIN")?;
            require_assignment(p, "VARIABLE")?;
            let name = p.name("a variable name")?;
            p.expect(Punct::LParen)?;
            let Some(value) = p.value() else {
                return Err(p.unexpected(&["a value"]));
            };
            p.expect(Punct::RParen)?;
            c.domain = Some((name, value));
        } else if p.eat_keyword("MISSING") {
            once(p, start, &mut seen, "MISSING")?;
            require_assignment(p, "CLASSMISSING")?;
            if p.eat_keyword("INCLUDE") {
                c.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                c.include = false;
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            c.print = print(p, logistic)?;
        } else if p.eat_keyword("SAVE") {
            once(p, start, &mut seen, "SAVE")?;
            c.save = save(p, logistic)?;
        } else if logistic && p.at_keyword("EMMEANS") {
            return Err(p.subcommand_error(command, &[]));
        } else {
            return Err(p.subcommand_error(command, &["JOINTPROB", "OUTFILE"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'", "BY", "WITH"])?;
    let end = p.pos;
    if c.plan.is_empty() {
        return Err(error(p, end, format!("{command} needs PLAN FILE='name'")));
    }
    if c.intercept == CsIntercept::Only && c.model.is_some() {
        return Err(error(
            p,
            end,
            "INTERCEPT INCLUDE=ONLY fits the intercept alone and cannot go with MODEL",
        ));
    }
    if c.intercept == CsIntercept::No && c.factors.is_empty() && c.covariates.is_empty() {
        return Err(error(
            p,
            end,
            "INTERCEPT INCLUDE=NO needs factors or covariates",
        ));
    }
    Ok(Command::CsModel(Box::new(c)))
}

pub(crate) fn csglm(p: &mut Parser) -> PResult<Command> {
    model(p, false)
}

pub(crate) fn cslogistic(p: &mut Parser) -> PResult<Command> {
    model(p, true)
}
