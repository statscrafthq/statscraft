//! Parser of MANOVA, the older analysis of variance and covariance with its own design notation.
//!
//! Rules noted from the Command Syntax Reference (MANOVA, its univariate and multivariate parts):
//! - `dependents [BY factors (min,max) [factors (min,max) ...]] [WITH covariates]`; a range
//!   closes a group of factors and applies to each of them.
//! - Subcommands other than DESIGN stay in effect for every later DESIGN until replaced; a
//!   DESIGN uses those given before it. When no DESIGN ends the command, one more full
//!   factorial design is added. MISSING is read once for the whole procedure.
//! - DESIGN terms are separated by spaces or commas. A term is effects joined by `+` (pooled
//!   into one test); an effect is items joined by BY, then items after WITHIN (alias W) that
//!   nest it. An item is a factor or continuous variable with an optional `(n)` (a partition,
//!   or a level after WITHIN) or `POOL(varlist)`. `= n` makes the term error term n (1 to 10);
//!   `VS` (or AGAINST, VERSUS) names its error: WITHIN (W), RESIDUAL (R), WR (RW,
//!   WITHIN+RESIDUAL) or a number. CONSTANT is the constant term; MUPLUS joins the constant
//!   to the next term.
//! - ERROR, CONTRAST(f)=type, PARTITION(f)[=(n, m*k ...)], METHOD (UNIQUE, SEQUENTIAL,
//!   CONSTANT, NOCONSTANT, QR, CHOLESKY), PRINT and NOPRINT, OMEANS, PMEANS, ANALYSIS.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{Contrast, contrast, list_until};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// Factors sharing one `(min,max)` range.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaFactors {
    pub names: Vec<VarItem>,
    pub min: f64,
    pub max: f64,
}

/// The error term a test uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManovaError {
    Within,
    Residual,
    WithinResidual,
    Number(u32),
}

/// A factor or continuous variable of a design effect, with its `(n)`; POOL gives several.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaItem {
    pub names: Vec<VarItem>,
    pub pool: bool,
    pub number: Option<u32>,
}

/// One effect: the crossed items (joined by BY) and the items it is nested within. An empty
/// effect is the constant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManovaEffect {
    pub crossed: Vec<ManovaItem>,
    pub within: Vec<ManovaItem>,
}

/// A term of DESIGN: effects pooled with `+`, as error term `define`, tested against `against`.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaTerm {
    pub effects: Vec<ManovaEffect>,
    pub define: Option<u32>,
    pub against: Option<ManovaError>,
    pub muplus: bool,
    /// The term as written, for the tables.
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ManovaPrint {
    pub multiv: bool,
    pub univ: bool,
    pub eigen: bool,
    pub dimenr: bool,
    pub hypoth: bool,
    pub stepdown: bool,
    pub brief: bool,
    pub parameters: bool,
    pub negsum: bool,
    pub error_sscp: bool,
    pub error_cov: bool,
    pub error_cor: bool,
    pub error_stddev: bool,
}

/// The settings a DESIGN uses.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaDesign {
    /// None: the full factorial model.
    pub terms: Option<Vec<ManovaTerm>>,
    pub error: Option<ManovaError>,
    pub contrasts: Vec<(String, Contrast)>,
    pub partitions: Vec<(String, Vec<u32>)>,
    pub sequential: bool,
    pub constant: bool,
    pub print: ManovaPrint,
    /// ANALYSIS: dependents and covariates by name.
    pub analysis: Option<(Vec<VarItem>, Vec<VarItem>)>,
    pub pmeans: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManovaOmeans {
    pub variables: Option<Vec<VarItem>>,
    /// Each table: its factors; empty for CONSTANT.
    pub tables: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manova {
    pub dependents: Vec<VarItem>,
    pub factors: Vec<ManovaFactors>,
    pub covariates: Vec<VarItem>,
    pub designs: Vec<ManovaDesign>,
    pub implicit_design: bool,
    pub include: bool,
    /// CELLINFO: means, SSCP, covariance, correlation.
    pub cellinfo: [bool; 4],
    /// HOMOGENEITY: Bartlett-Box F, Cochran's C, Box's M.
    pub homogeneity: [bool; 3],
    pub omeans: Option<ManovaOmeans>,
}

const KNOWN: [&str; 12] = [
    "WSFACTORS",
    "WSDESIGN",
    "MEASURE",
    "TRANSFORM",
    "RENAME",
    "RESIDUALS",
    "POWER",
    "CINTERVAL",
    "PLOT",
    "PCOMPS",
    "DISCRIM",
    "MATRIX",
];

fn error_term(p: &mut Parser) -> PResult<ManovaError> {
    if let Some(n) = p.number() {
        return error_number(p, n);
    }
    let within = p.eat_keyword("WITHIN") || p.eat_word("W");
    if within && p.eat(Punct::Plus) {
        if p.eat_keyword("RESIDUAL") || p.eat_word("R") {
            return Ok(ManovaError::WithinResidual);
        }
        return Err(p.unexpected(&["RESIDUAL"]));
    }
    if within {
        return Ok(ManovaError::Within);
    }
    if p.eat_keyword("RESIDUAL") || p.eat_word("R") {
        return Ok(ManovaError::Residual);
    }
    if p.eat_word("WR") || p.eat_word("RW") {
        return Ok(ManovaError::WithinResidual);
    }
    Err(p.unexpected(&["WITHIN", "RESIDUAL", "WR", "an error term number"]))
}

fn error_number(p: &Parser, n: f64) -> PResult<ManovaError> {
    if n.fract() == 0.0 && (1.0..=10.0).contains(&n) {
        Ok(ManovaError::Number(n as u32))
    } else {
        Err(SyntaxError::new(
            p.span_from(p.pos - 1),
            "an error term number is a whole number from 1 to 10",
        ))
    }
}

fn whole(p: &mut Parser, what: &str) -> PResult<u32> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as u32),
        Some(_) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is a whole number of at least 1"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn item(p: &mut Parser) -> PResult<ManovaItem> {
    if p.eat_word("POOL") {
        p.expect(Punct::LParen)?;
        let names = list_until(p, &[])?;
        p.expect(Punct::RParen)?;
        return Ok(ManovaItem {
            names,
            pool: true,
            number: None,
        });
    }
    let name = p.name("a factor or variable name")?;
    let mut number = None;
    if p.eat(Punct::LParen) {
        number = Some(whole(p, "the partition or level number")?);
        p.expect(Punct::RParen)?;
    }
    Ok(ManovaItem {
        names: vec![VarItem::Name(name)],
        pool: false,
        number,
    })
}

fn is_within(p: &Parser) -> bool {
    p.at_keyword("WITHIN")
        || matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("W"))
}

fn effect(p: &mut Parser) -> PResult<ManovaEffect> {
    let mut e = ManovaEffect::default();
    if p.eat_keyword("CONSTANT") {
        return Ok(e);
    }
    e.crossed.push(item(p)?);
    while p.eat_word("BY") {
        e.crossed.push(item(p)?);
    }
    while is_within(p) {
        p.pos += 1;
        e.within.push(item(p)?);
        while p.eat_word("BY") {
            e.within.push(item(p)?);
        }
    }
    Ok(e)
}

fn design_terms(p: &mut Parser) -> PResult<Vec<ManovaTerm>> {
    let mut terms = Vec::new();
    let mut muplus = false;
    while !at_end(p) {
        if p.eat_keyword("MUPLUS") {
            muplus = true;
            continue;
        }
        if p.at_keyword("MWITHIN") {
            return Err(SyntaxError::new(
                p.span(),
                "MWITHIN (simple effects of within-subjects factors) is not supported yet",
            ));
        }
        let mut effects = vec![effect(p)?];
        while p.eat(Punct::Plus) {
            effects.push(effect(p)?);
        }
        let mut define = None;
        if p.eat(Punct::Eq) {
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["an error term number"]));
            };
            let ManovaError::Number(n) = error_number(p, n)? else {
                unreachable!()
            };
            define = Some(n);
        }
        let against = if p.eat_keyword("VS") || p.eat_keyword("AGAINST") || p.eat_keyword("VERSUS")
        {
            Some(error_term(p)?)
        } else {
            None
        };
        let label = effects
            .iter()
            .map(effect_label)
            .collect::<Vec<_>>()
            .join(" + ");
        terms.push(ManovaTerm {
            effects,
            define,
            against,
            muplus: std::mem::take(&mut muplus),
            label,
        });
        p.eat(Punct::Comma);
    }
    if muplus {
        return Err(p.unexpected(&["a term after MUPLUS"]));
    }
    Ok(terms)
}

/// Keywords in parentheses after a PRINT keyword; empty when there are none.
fn options<'k>(p: &mut Parser, allowed: &[&'k str]) -> PResult<Vec<&'k str>> {
    let mut out = Vec::new();
    if !p.eat(Punct::LParen) {
        return Ok(out);
    }
    while !p.eat(Punct::RParen) {
        let Some(k) = allowed.iter().find(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(allowed));
        };
        p.pos += 1;
        out.push(*k);
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn word(p: &Parser) -> String {
    match p.peek() {
        Some(TokenKind::Word(w)) => w.to_uppercase(),
        _ => String::new(),
    }
}

fn item_label(i: &ManovaItem) -> String {
    let names: Vec<String> = i
        .names
        .iter()
        .map(|n| match n {
            VarItem::Name(n) => n.to_uppercase(),
            VarItem::Range(a, b) => format!("{} TO {}", a.to_uppercase(), b.to_uppercase()),
            VarItem::All => "ALL".into(),
        })
        .collect();
    let mut s = if i.pool {
        format!("POOL({})", names.join(" "))
    } else {
        names.join(" ")
    };
    if let Some(n) = i.number {
        s.push_str(&format!("({n})"));
    }
    s
}

/// An effect as the tables name it, such as `A BY B` or `C WITHIN A(1)`.
pub fn effect_label(e: &ManovaEffect) -> String {
    if e.crossed.is_empty() {
        return "CONSTANT".into();
    }
    let join = |items: &[ManovaItem]| -> String {
        items
            .iter()
            .map(item_label)
            .collect::<Vec<_>>()
            .join(" BY ")
    };
    let mut s = join(&e.crossed);
    for w in &e.within {
        s.push_str(" WITHIN ");
        s.push_str(&item_label(w));
    }
    s
}

fn print(p: &mut Parser, on: bool, d: &mut ManovaDesign, m: &mut Manova) -> PResult<()> {
    p.eat(Punct::Eq);
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("CELLINFO") {
            let o = options(p, &["MEANS", "SSCP", "COV", "COR", "ALL"])?;
            let all = o.contains(&"ALL");
            for (i, k) in ["MEANS", "SSCP", "COV", "COR"].iter().enumerate() {
                if all || o.contains(k) || (o.is_empty() && i == 0) {
                    m.cellinfo[i] = on;
                }
            }
        } else if p.eat_keyword("HOMOGENEITY") {
            let o = options(p, &["BARTLETT", "COCHRAN", "BOXM", "ALL"])?;
            let all = o.is_empty() || o.contains(&"ALL");
            for (i, k) in ["BARTLETT", "COCHRAN", "BOXM"].iter().enumerate() {
                if all || o.contains(k) {
                    m.homogeneity[i] = on;
                }
            }
        } else if p.eat_keyword("PARAMETERS") {
            let o = options(
                p,
                &[
                    "ESTIM", "NEGSUM", "ALL", "ORTHO", "COR", "EFSIZE", "OPTIMAL",
                ],
            )?;
            if let Some(k) = o
                .iter()
                .find(|k| ["ORTHO", "COR", "EFSIZE", "OPTIMAL"].contains(k))
            {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("PARAMETERS({k}) of MANOVA is not supported yet; ESTIM and NEGSUM are"),
                ));
            }
            d.print.parameters = on;
            if o.contains(&"NEGSUM") || o.contains(&"ALL") {
                d.print.negsum = on;
            }
        } else if p.eat_keyword("SIGNIF") {
            let o = options(
                p,
                &[
                    "MULTIV", "UNIV", "EIGEN", "DIMENR", "HYPOTH", "STEPDOWN", "BRIEF", "SINGLEDF",
                    "AVERF", "AVONLY", "HF", "GG", "EFSIZE",
                ],
            )?;
            for k in &o {
                let flag = match *k {
                    "MULTIV" => &mut d.print.multiv,
                    "UNIV" => &mut d.print.univ,
                    "EIGEN" => &mut d.print.eigen,
                    "DIMENR" => &mut d.print.dimenr,
                    "HYPOTH" => &mut d.print.hypoth,
                    "STEPDOWN" => &mut d.print.stepdown,
                    "BRIEF" => &mut d.print.brief,
                    other => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            format!("SIGNIF({other}) of MANOVA is not supported yet"),
                        ));
                    }
                };
                *flag = on;
            }
        } else if p.eat_keyword("ERROR") {
            let o = options(p, &["SSCP", "COV", "COR", "STDDEV"])?;
            let flags = &mut d.print;
            if o.is_empty() {
                flags.error_cov = on;
                flags.error_cor = on;
                flags.error_stddev = on;
            }
            for k in &o {
                match *k {
                    "SSCP" => flags.error_sscp = on,
                    "COV" => flags.error_cov = on,
                    "COR" => flags.error_cor = on,
                    _ => flags.error_stddev = on,
                }
            }
        } else if p.at_keyword("DESIGN") || p.at_keyword("TRANSFORM") {
            return Err(SyntaxError::new(
                p.span(),
                format!("PRINT {} of MANOVA is not supported yet", word(p)),
            ));
        } else {
            return Err(p.unexpected(&[
                "CELLINFO",
                "HOMOGENEITY",
                "PARAMETERS",
                "SIGNIF",
                "ERROR",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn factor_in_parens(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let name = p.name("a factor name")?;
    p.expect(Punct::RParen)?;
    Ok(name)
}

fn means_tables(p: &mut Parser) -> PResult<Vec<Vec<String>>> {
    p.expect(Punct::LParen)?;
    let mut tables = Vec::new();
    while !p.eat(Punct::RParen) {
        if p.eat_keyword("CONSTANT") {
            tables.push(Vec::new());
        } else {
            let mut t = vec![p.name("a factor name")?];
            while p.eat_word("BY") {
                t.push(p.name("a factor name")?);
            }
            tables.push(t);
        }
        p.eat(Punct::Comma);
    }
    Ok(tables)
}

pub(crate) fn manova(p: &mut Parser) -> PResult<Command> {
    let dependents = list_until(p, &["BY", "WITH"])?;
    let mut factors = Vec::new();
    if p.eat_word("BY") {
        while matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH")) {
            let names = list_until(p, &["WITH"])?;
            let start = p.pos;
            p.expect(Punct::LParen)?;
            let (Some(min), _, Some(max)) = (p.number(), p.eat(Punct::Comma), p.number()) else {
                return Err(p.unexpected(&["the minimum and maximum values"]));
            };
            p.expect(Punct::RParen)?;
            if min.fract() != 0.0 || max.fract() != 0.0 || min > max {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "a factor range is two whole numbers, the minimum first",
                ));
            }
            factors.push(ManovaFactors { names, min, max });
            p.eat(Punct::Comma);
        }
        if factors.is_empty() {
            return Err(p.unexpected(&["a factor name"]));
        }
    }
    let covariates = if p.eat_word("WITH") {
        list_until(p, &[])?
    } else {
        Vec::new()
    };
    let mut m = Manova {
        dependents,
        factors,
        covariates,
        designs: Vec::new(),
        implicit_design: false,
        include: false,
        cellinfo: [false; 4],
        homogeneity: [false; 3],
        omeans: None,
    };
    let mut d = ManovaDesign {
        terms: None,
        error: None,
        contrasts: Vec::new(),
        partitions: Vec::new(),
        sequential: false,
        constant: true,
        print: ManovaPrint {
            multiv: true,
            univ: true,
            ..ManovaPrint::default()
        },
        analysis: None,
        pmeans: false,
    };
    let mut last_design = false;
    while p.eat(Punct::Slash) {
        last_design = false;
        let start = p.pos;
        if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = design_terms(p)?;
            d.terms = (!terms.is_empty()).then_some(terms);
            m.designs.push(d.clone());
            last_design = true;
        } else if p.eat_keyword("ERROR") {
            p.eat(Punct::Eq);
            d.error = Some(error_term(p)?);
        } else if p.eat_keyword("CONTRAST") {
            let name = factor_in_parens(p)?;
            p.expect(Punct::Eq)?;
            let c = contrast(p, false)?;
            d.contrasts.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            d.contrasts.push((name, c));
        } else if p.eat_keyword("PARTITION") {
            let name = factor_in_parens(p)?;
            let mut sizes = Vec::new();
            if p.eat(Punct::Eq) || p.at(Punct::LParen) {
                p.expect(Punct::LParen)?;
                while !p.eat(Punct::RParen) {
                    let n = whole(p, "a partition size")?;
                    if p.eat(Punct::Star) {
                        let size = whole(p, "a partition size")?;
                        sizes.extend(std::iter::repeat_n(size, n as usize));
                    } else {
                        sizes.push(n);
                    }
                    p.eat(Punct::Comma);
                }
            }
            d.partitions.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            d.partitions.push((name, sizes));
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("UNIQUE") {
                    d.sequential = false;
                } else if p.eat_keyword("SEQUENTIAL") {
                    d.sequential = true;
                } else if p.eat_keyword("CONSTANT") {
                    d.constant = true;
                } else if p.eat_keyword("NOCONSTANT") {
                    d.constant = false;
                } else if !(p.eat_word("QR") || p.eat_keyword("CHOLESKY")) {
                    return Err(p.unexpected(&[
                        "UNIQUE",
                        "SEQUENTIAL",
                        "CONSTANT",
                        "NOCONSTANT",
                        "QR",
                        "CHOLESKY",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            print(p, true, &mut d, &mut m)?;
        } else if p.eat_keyword("NOPRINT") {
            print(p, false, &mut d, &mut m)?;
        } else if p.eat_keyword("OMEANS") {
            p.eat(Punct::Eq);
            let mut o = ManovaOmeans::default();
            while !at_end(p) {
                if p.eat_keyword("VARIABLES") {
                    p.expect(Punct::LParen)?;
                    o.variables = Some(list_until(p, &[])?);
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("TABLES") {
                    o.tables.extend(means_tables(p)?);
                } else {
                    return Err(p.unexpected(&["VARIABLES", "TABLES"]));
                }
                p.eat(Punct::Comma);
            }
            if o.tables.is_empty() {
                if o.variables.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "OMEANS VARIABLES needs TABLES as well",
                    ));
                }
                m.cellinfo[0] = true;
            } else {
                m.omeans = Some(o);
            }
        } else if p.eat_keyword("PMEANS") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                return Err(SyntaxError::new(
                    p.span(),
                    "PMEANS takes no keywords yet: VARIABLES, TABLES and PLOT are not supported",
                ));
            }
            d.pmeans = true;
        } else if p.eat_keyword("ANALYSIS") {
            p.eat(Punct::Eq);
            if p.at(Punct::LParen) {
                return Err(SyntaxError::new(
                    p.span(),
                    "groups of analyses and CONDITIONAL in ANALYSIS are not supported yet",
                ));
            }
            let deps = list_until(p, &["WITH"])?;
            let covs = if p.eat_word("WITH") {
                list_until(p, &[])?
            } else {
                Vec::new()
            };
            d.analysis = Some((deps, covs));
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("INCLUDE") {
                    m.include = true;
                } else if p.eat_keyword("EXCLUDE") {
                    m.include = false;
                } else if !p.eat_keyword("LISTWISE") {
                    return Err(p.unexpected(&["LISTWISE", "EXCLUDE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            let mut error = p.subcommand_error("MANOVA", &KNOWN);
            if p.at_keyword("WSFACTORS") || p.at_keyword("WSDESIGN") || p.at_keyword("MEASURE") {
                error.message = format!(
                    "{} (repeated measures in MANOVA) is not supported yet; GLM with WSFACTOR runs repeated measures",
                    word(p)
                );
            }
            return Err(error);
        }
        if !last_design && !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !last_design {
        m.implicit_design = !m.designs.is_empty();
        d.terms = None;
        m.designs.push(d);
    }
    Ok(Command::Manova(Box::new(m)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn manova(source: &str) -> Manova {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match parsed.remove(0).command {
            Command::Manova(m) => *m,
            other => panic!("{other:?}"),
        }
    }

    fn error(source: &str) -> String {
        let (_, mut errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors.remove(0).message
    }

    #[test]
    fn variable_list_and_settings_per_design() {
        let m = manova(
            "MANOVA y1 y2 BY a b (1,3) c(0,1) WITH x /CONTR(a)=SIMPLE(1) /METH=SEQ NOCONSTANT \
             /DESIGN a, b W a = 1 VS 1, MUPLUS c + a BY c VS WR, CONSTANT /ERROR=R \
             /PRINT=SIGNIF(EIGEN) /NOPRINT=SIGNIF(UNIV).",
        );
        assert_eq!(m.factors.len(), 2);
        assert_eq!(
            (m.factors[0].min, m.factors[0].max, m.factors[1].max),
            (1.0, 3.0, 1.0)
        );
        assert_eq!(m.covariates, vec![VarItem::Name("x".into())]);
        assert_eq!(m.designs.len(), 2);
        assert!(m.implicit_design);
        let d = &m.designs[0];
        assert!(d.sequential && !d.constant);
        let t = d.terms.as_ref().unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[1].label, "B WITHIN A");
        assert_eq!(
            (t[1].define, t[1].against),
            (Some(1), Some(ManovaError::Number(1)))
        );
        assert!(t[2].muplus);
        assert_eq!(t[2].label, "C + A BY C");
        assert_eq!(t[2].against, Some(ManovaError::WithinResidual));
        assert_eq!(t[3].label, "CONSTANT");
        let last = &m.designs[1];
        assert!(last.terms.is_none());
        assert_eq!(last.error, Some(ManovaError::Residual));
        assert!(last.print.eigen && !last.print.univ && last.print.multiv);
    }

    #[test]
    fn partitions_and_unsupported_parts() {
        let m = manova("MANOVA y BY t(1,12) /PARTITION(t)=(3*2,4) /DESIGN t(2).");
        assert_eq!(m.designs[0].partitions[0].1, vec![2, 2, 2, 4]);
        assert!(error("MANOVA y BY t(1,3) /WSFACTORS=w(2).").contains("not supported yet"));
        assert!(
            error("MANOVA y BY t(1,3) /PRINT=PARAMETERS(ORTHO).").contains("not supported yet")
        );
        assert!(error("MANOVA y BY t(3,1).").contains("minimum first"));
        assert!(error("MANOVA y BY t(1,3) /DESIGN t = 11.").contains("1 to 10"));
    }
}
