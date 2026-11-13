//! Parser of MIXED: the linear mixed model.
//!
//! Rules (our notes from the Command Syntax Reference, MIXED):
//! - `dependent [BY factors] [WITH covariates]` first, then subcommands in any order. RANDOM may
//!   repeat; every other subcommand counts once, the last one winning. An empty subcommand is
//!   ignored.
//! - FIXED: terms as in UNIANOVA (`a*b`, `a BY b`, `a(b)`, `a WITHIN b`), INTERCEPT first when
//!   written; after `|`, NOINT drops the intercept and SSTYPE(1 | 3) picks the tests (3 by default).
//!   Without FIXED the fixed part is the intercept alone.
//! - RANDOM: terms, INTERCEPT first when written (never implied); after `|`, SUBJECT(a*b*...),
//!   COVTYPE(type) (VC by default) and SOLUTION. Effects on one RANDOM share one covariance block.
//! - REPEATED: variables joined by `*`, then `|` with SUBJECT (required) and COVTYPE (DIAG by
//!   default; VC there means DIAG).
//! - METHOD: REML (default) or ML. MISSING: EXCLUDE (default) or INCLUDE.
//! - CRITERIA: CIN(95), MXITER(100), MXSTEP(10), SCORING(1), SINGULAR(1E-12), PCONVERGE(1E-6),
//!   LCONVERGE(0) and HCONVERGE(0), the three with ABSOLUTE (default) or RELATIVE, and
//!   DFMETHOD(SATTERTHWAITE | RESIDUAL | KENWARDROGER).
//! - PRINT adds tables; SAVE adds casewise values; REGWGT names regression weights.
//! - EMMEANS may repeat (an identical one counts once): TABLES(OVERALL | factor[*factor...]),
//!   WITH(covariate=value | MEAN ...), COMPARE[(factor)] with REFCAT(value | FIRST | LAST) and
//!   ADJ(LSD | BONFERRONI | SIDAK), as in UNIANOVA plus REFCAT.
//! - TEST may repeat: `TEST[(constants)] = ['label'] effect values ... [DIVISOR=v]; ...`, or ALL
//!   with values for every fixed parameter. The constants, one per row, default to 0; DIVISOR
//!   divides the coefficients of its row. Coefficients for random effects after `|` are not
//!   supported yet.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{CustomTest, Emmeans, Term, custom_test, emmeans, list_until, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// A covariance structure of RANDOM or REPEATED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovType {
    /// Variance components: a scaled identity for each effect (DIAG on REPEATED).
    Vc,
    /// One variance for the whole block.
    Id,
    /// One variance per level.
    Diag,
    /// Every variance and covariance free.
    Un,
    /// A common variance plus a common covariance.
    Cs,
    /// A variance and a correlation that falls by a power of the distance.
    Ar1,
}

/// One RANDOM subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct RandomEffects {
    /// The effects; INTERCEPT is a term with no variables.
    pub terms: Vec<Term>,
    pub subject: Vec<String>,
    pub covtype: CovType,
}

/// The REPEATED subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct Repeated {
    pub variables: Vec<String>,
    pub subject: Vec<String>,
    pub covtype: CovType,
}

/// The temporary variables of SAVE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixedTemp {
    Fixpred,
    Pred,
    Resid,
    Sefixp,
    Dffixp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MixedPrint {
    pub solution: bool,
    pub testcov: bool,
    pub g: bool,
    pub r: bool,
    pub covb: bool,
    pub corb: bool,
}

/// A convergence criterion: the value (0 turns it off) and whether it is relative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Convergence {
    pub value: f64,
    pub relative: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mixed {
    pub dependent: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    /// FIXED terms; INTERCEPT is a term with no variables.
    pub fixed: Vec<Term>,
    /// False after `| NOINT`.
    pub intercept: bool,
    pub sstype: u8,
    pub ml: bool,
    pub include: bool,
    pub cin: f64,
    pub mxiter: usize,
    pub mxstep: usize,
    pub scoring: usize,
    pub singular: f64,
    pub pconverge: Convergence,
    pub lconverge: Convergence,
    pub hconverge: Convergence,
    /// DFMETHOD(RESIDUAL); Satterthwaite otherwise.
    pub residual_df: bool,
    pub print: MixedPrint,
    pub random: Vec<RandomEffects>,
    pub repeated: Option<Repeated>,
    pub regwgt: Option<String>,
    pub save: Vec<(MixedTemp, Option<String>)>,
    pub emmeans: Vec<Emmeans>,
    pub tests: Vec<CustomTest>,
}

const COVTYPES: [(&str, CovType); 6] = [
    ("VC", CovType::Vc),
    ("ID", CovType::Id),
    ("DIAG", CovType::Diag),
    ("UN", CovType::Un),
    ("CS", CovType::Cs),
    ("AR1", CovType::Ar1),
];

const COVTYPES_LATER: [&str; 20] = [
    "AD1",
    "ARH1",
    "ARMA11",
    "CSH",
    "CSR",
    "FA1",
    "FAH1",
    "HF",
    "TP",
    "TPH",
    "UNR",
    "SP_POWER",
    "SP_EXPONENTIAL",
    "SP_GAUSSIAN",
    "SP_LINEAR",
    "SP_LINEARLOG",
    "SP_SPHERICAL",
    "UN_AR1",
    "UN_CS",
    "UN_UN",
];

fn not_yet(p: &Parser, what: String) -> SyntaxError {
    SyntaxError::new(p.span(), format!("{what} is not supported yet"))
}

/// The word at the cursor in capitals, if any.
fn word(p: &Parser) -> Option<String> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w.to_uppercase()),
        _ => None,
    }
}

fn in_parens<T>(p: &mut Parser, rule: impl FnOnce(&mut Parser) -> PResult<T>) -> PResult<T> {
    p.expect(Punct::LParen)?;
    let out = rule(p)?;
    p.expect(Punct::RParen)?;
    Ok(out)
}

fn covtype(p: &mut Parser) -> PResult<CovType> {
    in_parens(p, |p| {
        let w = word(p).unwrap_or_default();
        if let Some((_, t)) = COVTYPES.iter().find(|(n, _)| w == *n) {
            p.pos += 1;
            return Ok(*t);
        }
        if let Some(n) = COVTYPES_LATER.iter().find(|n| w == **n) {
            return Err(not_yet(p, format!("COVTYPE({n})")));
        }
        Err(p.unexpected(&["VC", "ID", "DIAG", "UN", "CS", "AR1"]))
    })
}

/// `name*name*...` inside SUBJECT( ) or after REPEATED.
fn starred(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = vec![p.name("a variable name")?];
    while p.eat(Punct::Star) {
        let start = p.pos;
        let name = p.name("a variable name")?;
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} is named twice"),
            ));
        }
        names.push(name);
    }
    Ok(names)
}

/// Terms up to `|` or the end of the subcommand.
fn effects(p: &mut Parser) -> PResult<Vec<Term>> {
    let mut out: Vec<Term> = Vec::new();
    while !at_end(p) && !p.at(Punct::Or) {
        let start = p.pos;
        if p.eat_keyword("INTERCEPT") {
            if !out.is_empty() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "INTERCEPT comes first among the effects",
                ));
            }
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

fn positive_whole(p: &mut Parser, what: &str, zero: bool) -> PResult<usize> {
    in_parens(p, |p| {
        let start = p.pos;
        match p.number() {
            Some(n) if n.fract() == 0.0 && (n > 0.0 || (zero && n == 0.0)) => Ok(n as usize),
            _ => Err(SyntaxError::new(
                p.span_from(start),
                format!(
                    "{what} needs a whole number of at least {}",
                    u8::from(!zero)
                ),
            )),
        }
    })
}

fn convergence(p: &mut Parser, what: &str) -> PResult<Convergence> {
    in_parens(p, |p| {
        let start = p.pos;
        let Some(value) = p.number().filter(|v| *v >= 0.0) else {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{what} needs a value of at least 0"),
            ));
        };
        p.eat(Punct::Comma);
        let relative = if p.eat_keyword("RELATIVE") {
            true
        } else {
            p.eat_keyword("ABSOLUTE");
            false
        };
        Ok(Convergence { value, relative })
    })
}

fn criteria(p: &mut Parser, m: &mut Mixed) -> PResult<()> {
    while !at_end(p) {
        if p.eat_keyword("CIN") {
            let start = p.pos;
            let v = in_parens(p, |p| Ok(p.number()))?;
            match v {
                Some(v) if (0.0..100.0).contains(&v) => m.cin = v,
                _ => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "CIN needs a value of at least 0 and below 100",
                    ));
                }
            }
        } else if p.eat_keyword("MXITER") {
            m.mxiter = positive_whole(p, "MXITER", true)?;
        } else if p.eat_keyword("MXSTEP") {
            m.mxstep = positive_whole(p, "MXSTEP", false)?;
        } else if p.eat_keyword("SCORING") {
            m.scoring = positive_whole(p, "SCORING", true)?;
        } else if p.eat_keyword("SINGULAR") {
            let start = p.pos;
            match in_parens(p, |p| Ok(p.number()))? {
                Some(v) if v > 0.0 => m.singular = v,
                _ => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "SINGULAR needs a positive value",
                    ));
                }
            }
        } else if p.eat_keyword("PCONVERGE") {
            m.pconverge = convergence(p, "PCONVERGE")?;
        } else if p.eat_keyword("LCONVERGE") {
            m.lconverge = convergence(p, "LCONVERGE")?;
        } else if p.eat_keyword("HCONVERGE") {
            m.hconverge = convergence(p, "HCONVERGE")?;
        } else if p.eat_keyword("DFMETHOD") {
            m.residual_df = in_parens(p, |p| {
                if p.eat_keyword("SATTERTHWAITE") {
                    Ok(false)
                } else if p.eat_keyword("RESIDUAL") {
                    Ok(true)
                } else if p.at_keyword("KENWARDROGER") {
                    Err(not_yet(p, "DFMETHOD(KENWARDROGER)".into()))
                } else {
                    Err(p.unexpected(&["SATTERTHWAITE", "RESIDUAL", "KENWARDROGER"]))
                }
            })?;
        } else {
            return Err(p.unexpected(&[
                "CIN",
                "DFMETHOD",
                "HCONVERGE",
                "LCONVERGE",
                "MXITER",
                "MXSTEP",
                "PCONVERGE",
                "SCORING",
                "SINGULAR",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn print(p: &mut Parser) -> PResult<MixedPrint> {
    const LATER: [&str; 4] = ["CPS", "DESCRIPTIVES", "HISTORY", "LMATRIX"];
    let mut out = MixedPrint::default();
    while !at_end(p) {
        if p.eat_keyword("SOLUTION") {
            out.solution = true;
        } else if p.eat_keyword("TESTCOV") {
            out.testcov = true;
        } else if p.eat_word("G") {
            out.g = true;
        } else if p.eat_word("R") {
            out.r = true;
        } else if p.eat_keyword("COVB") {
            out.covb = true;
        } else if p.eat_keyword("CORB") {
            out.corb = true;
        } else if let Some(w) = word(p)
            && let Some(name) = LATER
                .iter()
                .find(|k| w == **k || (w.len() >= 3 && k.starts_with(w.as_str())))
        {
            return Err(not_yet(p, format!("PRINT {name}")));
        } else {
            return Err(p.unexpected(&["CORB", "COVB", "G", "R", "SOLUTION", "TESTCOV"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn save(p: &mut Parser) -> PResult<Vec<(MixedTemp, Option<String>)>> {
    const TEMPS: [(&str, MixedTemp); 5] = [
        ("FIXPRED", MixedTemp::Fixpred),
        ("PRED", MixedTemp::Pred),
        ("RESID", MixedTemp::Resid),
        ("SEFIXP", MixedTemp::Sefixp),
        ("DFFIXP", MixedTemp::Dffixp),
    ];
    let mut out: Vec<(MixedTemp, Option<String>)> = Vec::new();
    while !at_end(p) {
        let w = word(p).unwrap_or_default();
        let Some(&(_, temp)) = TEMPS.iter().find(|(n, _)| w == *n) else {
            if w == "SEPRED" || w == "DFPRED" {
                return Err(not_yet(p, format!("SAVE {w}")));
            }
            return Err(p.unexpected(&["FIXPRED", "PRED", "RESID", "SEFIXP", "DFFIXP"]));
        };
        let start = p.pos;
        p.pos += 1;
        let name = if p.at(Punct::LParen) {
            Some(in_parens(p, |p| p.name("a variable name"))?)
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
    Ok(out)
}

fn random(p: &mut Parser) -> PResult<Option<RandomEffects>> {
    let terms = effects(p)?;
    let mut r = RandomEffects {
        terms,
        subject: Vec::new(),
        covtype: CovType::Vc,
    };
    if p.eat(Punct::Or) {
        while !at_end(p) {
            if p.eat_keyword("SUBJECT") {
                r.subject = in_parens(p, starred)?;
            } else if p.eat_keyword("COVTYPE") {
                r.covtype = covtype(p)?;
            } else if p.at_keyword("SOLUTION") {
                return Err(not_yet(p, "SOLUTION on RANDOM".into()));
            } else {
                return Err(p.unexpected(&["SUBJECT", "COVTYPE", "SOLUTION"]));
            }
        }
    }
    Ok((!r.terms.is_empty()).then_some(r))
}

fn repeated(p: &mut Parser) -> PResult<Option<Repeated>> {
    if at_end(p) {
        return Ok(None);
    }
    let variables = starred(p)?;
    let mut r = Repeated {
        variables,
        subject: Vec::new(),
        covtype: CovType::Diag,
    };
    if !p.eat(Punct::Or) && !at_end(p) {
        return Err(p.unexpected(&["'|'"]));
    }
    while !at_end(p) {
        if p.eat_keyword("SUBJECT") {
            r.subject = in_parens(p, starred)?;
        } else if p.eat_keyword("COVTYPE") {
            // VC stays VC here; the engine runs it as DIAG and says so
            r.covtype = covtype(p)?;
        } else if p.at_keyword("KRONECKER") || p.at_keyword("SPCOORDS") {
            let w = word(p).unwrap_or_default();
            return Err(not_yet(p, format!("{w} on REPEATED")));
        } else {
            return Err(p.unexpected(&["SUBJECT", "COVTYPE"]));
        }
    }
    if r.subject.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "REPEATED needs SUBJECT to say which cases belong to one subject",
        ));
    }
    if let Some(v) = r
        .variables
        .iter()
        .find(|v| r.subject.iter().any(|s| s.eq_ignore_ascii_case(v)))
    {
        return Err(SyntaxError::new(
            p.span(),
            format!("{v} cannot be both a repeated variable and a subject variable"),
        ));
    }
    Ok(Some(r))
}

pub(crate) fn mixed(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
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
    let off = Convergence {
        value: 0.0,
        relative: false,
    };
    let mut m = Mixed {
        dependent,
        factors,
        covariates,
        fixed: Vec::new(),
        intercept: true,
        sstype: 3,
        ml: false,
        include: false,
        cin: 95.0,
        mxiter: 100,
        mxstep: 10,
        scoring: 1,
        singular: 1e-12,
        pconverge: Convergence {
            value: 1e-6,
            relative: false,
        },
        lconverge: off,
        hconverge: off,
        residual_df: false,
        print: MixedPrint::default(),
        random: Vec::new(),
        repeated: None,
        regwgt: None,
        save: Vec::new(),
        emmeans: Vec::new(),
        tests: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("FIXED") {
            p.eat(Punct::Eq);
            m.fixed = effects(p)?;
            m.intercept = true;
            m.sstype = 3;
            if p.eat(Punct::Or) {
                while !at_end(p) {
                    if p.eat_keyword("NOINT") {
                        m.intercept = false;
                    } else if p.eat_keyword("SSTYPE") {
                        let start = p.pos;
                        match in_parens(p, |p| Ok(p.number()))? {
                            Some(3.0) => m.sstype = 3,
                            Some(1.0) => {
                                return Err(SyntaxError::new(
                                    p.span_from(start),
                                    "SSTYPE(1) is not supported yet",
                                ));
                            }
                            _ => {
                                return Err(SyntaxError::new(
                                    p.span_from(start),
                                    "SSTYPE is 1 or 3",
                                ));
                            }
                        }
                    } else {
                        return Err(p.unexpected(&["NOINT", "SSTYPE"]));
                    }
                }
            }
        } else if p.eat_keyword("RANDOM") {
            p.eat(Punct::Eq);
            if let Some(r) = random(p)? {
                // the same effects with the same subjects: the later one wins
                m.random.retain(|old| {
                    let same = |a: &[String], b: &[String]| {
                        a.len() == b.len()
                            && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
                    };
                    !(same(&old.subject, &r.subject)
                        && old.terms.len() == r.terms.len()
                        && old
                            .terms
                            .iter()
                            .zip(&r.terms)
                            .all(|(a, b)| a.label.eq_ignore_ascii_case(&b.label)))
                });
                m.random.push(r);
            }
        } else if p.eat_keyword("REPEATED") {
            p.eat(Punct::Eq);
            if let Some(r) = repeated(p)? {
                m.repeated = Some(r);
            }
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            if p.eat_keyword("ML") {
                m.ml = true;
            } else if p.eat_keyword("REML") {
                m.ml = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["ML", "REML"]));
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                m.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                m.include = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            criteria(p, &mut m)?;
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            m.print = print(p)?;
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            m.save = save(p)?;
        } else if p.eat_keyword("REGWGT") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                m.regwgt = Some(p.name("a variable name")?);
            }
        } else if p.eat_keyword("EMMEANS") {
            let e = emmeans(p, true)?;
            // an identical EMMEANS counts once, where it was first written
            if !m.emmeans.contains(&e) {
                m.emmeans.push(e);
            }
        } else if p.eat_keyword("TEST") {
            m.tests.push(custom_test(p, true)?);
        } else {
            return Err(p.subcommand_error("MIXED", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Mixed(Box::new(m)))
}
