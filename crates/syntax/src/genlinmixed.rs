//! Parser of GENLINMIXED: generalized linear mixed models.
//!
//! Rules (our notes from the Command Syntax Reference, GENLINMIXED):
//! - Subcommands in any order; FIELDS is required. RANDOM may repeat (one covariance block
//!   each); every other subcommand appears once. A subcommand is never empty and a keyword
//!   appears once within it. Names and keywords are written in full.
//! - DATA_STRUCTURE: SUBJECTS=a*b (or a BY b) is required there. REPEATED_MEASURES,
//!   KRONECKER_MEASURES, GROUPING and SPCOORDS are not supported yet; COVARIANCE_TYPE only
//!   shapes repeated measures, so it is read and has no effect.
//! - FIELDS: TARGET=name, TRIALS=NONE | VALUE(n) | FIELD(name), ANALYSIS_WEIGHT=name,
//!   OFFSET=NONE | VALUE(x) | FIELD(name). We also take a bare name or number for TRIALS and
//!   OFFSET and the spelling ANALYSISWEIGHT.
//! - TARGET_OPTIONS: REFERENCE=value, DISTRIBUTION, LINK (POWER takes LINK_PARAMETER, or
//!   POWER(x)). Without DISTRIBUTION the engine picks one from the target's measurement level.
//! - FIXED: EFFECTS=terms, USE_INTERCEPT=TRUE (default) | FALSE.
//! - RANDOM: EFFECTS, USE_INTERCEPT (FALSE by default; a block needs an effect or the
//!   intercept), SUBJECTS, COVARIANCE_TYPE (VARIANCE_COMPONENTS by default), SOLUTION.
//! - BUILD_OPTIONS: TARGET_CATEGORY_ORDER, INPUTS_CATEGORY_ORDER (ASCENDING only for now),
//!   MAX_ITERATIONS (100), CONFIDENCE_LEVEL (95), DF_METHOD (RESIDUAL), COVB (MODEL only for
//!   now), PCONVERGE (1E-6), LCONVERGE and HCONVERGE (0), each with (ABSOLUTE | RELATIVE),
//!   SCORING (0), SINGULAR (1E-12).
//! - EMMEANS_OPTIONS is checked and has no effect, since EMMEANS is not supported yet.

use crate::ast::{Command, Value};
use crate::error::SyntaxError;
use crate::genlin::{GenlinDistribution, GenlinLink, GenlinOffset, GenlinOrder, GenlinTrials};
use crate::glm::{Term, term};
use crate::lexer::{Punct, TokenKind};
use crate::mixed::{Convergence, CovType, DfMethod};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// One RANDOM subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct GlmmRandom {
    /// The effects, the intercept first when USE_INTERCEPT=TRUE.
    pub terms: Vec<Term>,
    pub subjects: Vec<String>,
    pub covtype: CovType,
    pub solution: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenLinMixed {
    /// DATA_STRUCTURE SUBJECTS.
    pub subjects: Vec<String>,
    pub target: String,
    pub trials: Option<GenlinTrials>,
    pub analysis_weight: Option<String>,
    pub offset: Option<GenlinOffset>,
    pub reference: Option<Value>,
    /// None: from the measurement level of the target.
    pub distribution: Option<GenlinDistribution>,
    pub link: Option<GenlinLink>,
    pub fixed: Vec<Term>,
    pub intercept: bool,
    pub random: Vec<GlmmRandom>,
    pub target_order: GenlinOrder,
    pub max_iterations: usize,
    pub confidence: f64,
    pub df_method: DfMethod,
    pub pconverge: Convergence,
    pub lconverge: Convergence,
    pub hconverge: Convergence,
    pub scoring: usize,
    pub singular: f64,
}

fn word_is(p: &Parser, word: &str) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(word))
}

/// `KEYWORD =` ahead: a keyword rather than an effect.
fn at_setting(p: &Parser) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(_)))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
}

fn not_yet(p: &Parser, start: usize, what: &str) -> SyntaxError {
    SyntaxError::new(
        p.span_from(start),
        format!("{what} of GENLINMIXED is not supported yet"),
    )
}

fn choice<T: Copy>(p: &mut Parser, table: &[(&str, T)]) -> PResult<T> {
    for (name, value) in table {
        if p.eat_word(name) {
            return Ok(*value);
        }
    }
    Err(p.unexpected(&table.iter().map(|(n, _)| *n).collect::<Vec<_>>()))
}

/// The structures of the residuals under DATA_STRUCTURE.
const RESIDUAL_TYPES: [&str; 19] = [
    "DIAGONAL",
    "AR1",
    "ARH1",
    "ARMA11",
    "COMPOUND_SYMMETRY",
    "CSH",
    "IDENTITY",
    "SP_POWER",
    "SP_EXPONENTIAL",
    "SP_GAUSSIAN",
    "SP_LINEAR",
    "SP_LINEARLOG",
    "SP_SPHERICAL",
    "TOEPLITZ",
    "UN_AR1",
    "UN_CS",
    "UN_UN",
    "UNSTRUCTURED",
    "VARIANCE_COMPONENTS",
];

fn number(p: &mut Parser) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&["a number"]))
}

/// The keywords of one subcommand: each `KEYWORD =` once, from `keywords`; `rule` reads the
/// value. Effects come through `rule` with the keyword EFFECTS.
fn settings(
    p: &mut Parser,
    keywords: &[&'static str],
    mut rule: impl FnMut(&mut Parser, &'static str, usize) -> PResult<()>,
) -> PResult<()> {
    let mut seen: Vec<&str> = Vec::new();
    if at_end(p) {
        return Err(SyntaxError::new(
            p.span(),
            format!("the subcommand is empty; it takes {}", keywords.join(", ")),
        ));
    }
    while !at_end(p) {
        let start = p.pos;
        let Some(&keyword) = keywords.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(keywords));
        };
        p.pos += 1;
        if seen.contains(&keyword) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{keyword} may be given only once"),
            ));
        }
        seen.push(keyword);
        p.expect(Punct::Eq)?;
        rule(p, keyword, start)?;
        p.eat(Punct::Comma);
    }
    Ok(())
}

/// Fields joined by `*` or BY.
fn crossed(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = vec![p.name("a field")?];
    while p.eat(Punct::Star) || p.eat_word("BY") {
        let start = p.pos;
        let name = p.name("a field")?;
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

fn effects(p: &mut Parser) -> PResult<Vec<Term>> {
    let mut out = Vec::new();
    while !at_end(p) && !at_setting(p) {
        out.push(term(p)?);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["an effect"]));
    }
    Ok(out)
}

fn boolean(p: &mut Parser) -> PResult<bool> {
    choice(p, &[("TRUE", true), ("FALSE", false)])
}

/// NONE, VALUE(x), FIELD(name), or (our addition) a bare number or name.
fn value_or_field(p: &mut Parser) -> PResult<Option<Result<f64, String>>> {
    if p.eat_word("NONE") {
        return Ok(None);
    }
    if p.eat_word("VALUE") {
        p.expect(Punct::LParen)?;
        let v = number(p)?;
        p.expect(Punct::RParen)?;
        return Ok(Some(Ok(v)));
    }
    if p.eat_word("FIELD") {
        p.expect(Punct::LParen)?;
        let n = p.name("a field")?;
        p.expect(Punct::RParen)?;
        return Ok(Some(Err(n)));
    }
    match p.number() {
        Some(v) => Ok(Some(Ok(v))),
        None => Ok(Some(Err(p.name("NONE, VALUE(number) or FIELD(name)")?))),
    }
}

fn covariance_type(p: &mut Parser) -> PResult<CovType> {
    let start = p.pos;
    let types = [
        ("DIAGONAL", Some(CovType::Diag)),
        ("AR1", Some(CovType::Ar1)),
        ("ARH1", Some(CovType::Arh1)),
        ("ARMA11", Some(CovType::Arma11)),
        ("COMPOUND_SYMMETRY", Some(CovType::Cs)),
        ("CSH", Some(CovType::Csh)),
        ("IDENTITY", Some(CovType::Id)),
        ("TOEPLITZ", Some(CovType::Tp)),
        ("UNSTRUCTURED", Some(CovType::Un)),
        ("VARIANCE_COMPONENTS", Some(CovType::Vc)),
        ("SP_POWER", None),
        ("SP_EXPONENTIAL", None),
        ("SP_GAUSSIAN", None),
        ("SP_LINEAR", None),
        ("SP_LINEARLOG", None),
        ("SP_SPHERICAL", None),
        ("UN_AR1", None),
        ("UN_CS", None),
        ("UN_UN", None),
    ];
    match choice(p, &types)? {
        Some(t) => Ok(t),
        None => Err(SyntaxError::new(
            p.span_from(start),
            "this COVARIANCE_TYPE belongs to repeated measures, which GENLINMIXED does not support yet",
        )),
    }
}

fn convergence(p: &mut Parser, what: &str) -> PResult<Convergence> {
    let start = p.pos;
    let value = number(p)?;
    if value < 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is 0 or more"),
        ));
    }
    let mut relative = false;
    if p.eat(Punct::LParen) {
        relative = choice(p, &[("ABSOLUTE", false), ("RELATIVE", true)])?;
        p.expect(Punct::RParen)?;
    }
    Ok(Convergence { value, relative })
}

fn whole(p: &mut Parser, what: &str) -> PResult<usize> {
    let start = p.pos;
    let v = number(p)?;
    if v < 0.0 || v.fract() != 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is a whole number, 0 or more"),
        ));
    }
    Ok(v as usize)
}

fn random(p: &mut Parser, span_start: usize) -> PResult<GlmmRandom> {
    let mut terms = Vec::new();
    let mut intercept = false;
    let mut r = GlmmRandom {
        terms: Vec::new(),
        subjects: Vec::new(),
        covtype: CovType::Vc,
        solution: false,
    };
    settings(
        p,
        &[
            "EFFECTS",
            "USE_INTERCEPT",
            "SUBJECTS",
            "GROUPING",
            "COVARIANCE_TYPE",
            "SOLUTION",
        ],
        |p, keyword, start| {
            match keyword {
                "EFFECTS" => terms = effects(p)?,
                "USE_INTERCEPT" => intercept = boolean(p)?,
                "SUBJECTS" => r.subjects = crossed(p)?,
                "GROUPING" => return Err(not_yet(p, start, "GROUPING on RANDOM")),
                "COVARIANCE_TYPE" => r.covtype = covariance_type(p)?,
                _ => r.solution = boolean(p)?,
            }
            Ok(())
        },
    )?;
    if terms.is_empty() && !intercept {
        return Err(SyntaxError::new(
            p.span_from(span_start),
            "RANDOM needs EFFECTS or USE_INTERCEPT=TRUE",
        ));
    }
    if intercept {
        terms.insert(
            0,
            Term {
                variables: Vec::new(),
                label: "Intercept".into(),
            },
        );
    }
    r.terms = terms;
    Ok(r)
}

fn link(p: &mut Parser) -> PResult<Option<GenlinLink>> {
    let start = p.pos;
    if p.eat_word("POWER") {
        if p.eat(Punct::LParen) {
            let v = number(p)?;
            p.expect(Punct::RParen)?;
            return Ok(Some(GenlinLink::Power(v)));
        }
        // the exponent comes from LINK_PARAMETER
        return Ok(None);
    }
    if p.eat_word("CAUCHIT") {
        return Err(SyntaxError::new(
            p.span_from(start),
            "LINK=CAUCHIT is for an ordinal multinomial target, which GENLINMIXED does not support yet",
        ));
    }
    Ok(Some(choice(
        p,
        &[
            ("IDENTITY", GenlinLink::Identity),
            ("CLOGLOG", GenlinLink::Cloglog),
            ("LOGC", GenlinLink::Logc),
            ("LOGIT", GenlinLink::Logit),
            ("LOG", GenlinLink::Log),
            ("NLOGLOG", GenlinLink::Nloglog),
            ("PROBIT", GenlinLink::Probit),
        ],
    )?))
}

fn emmeans_options(p: &mut Parser) -> PResult<()> {
    settings(p, &["CONTROL", "SCALE", "PADJUST"], |p, keyword, _| {
        match keyword {
            "CONTROL" => loop {
                p.name("a covariate")?;
                p.expect(Punct::LParen)?;
                number(p)?;
                p.expect(Punct::RParen)?;
                p.eat(Punct::Comma);
                if at_end(p) || at_setting(p) {
                    break;
                }
            },
            "SCALE" => choice(p, &[("ORIGINAL", ()), ("TRANSFORMED", ())])?,
            _ => choice(p, &[("LSD", ()), ("SEQBONFERRONI", ()), ("SEQSIDAK", ())])?,
        }
        Ok(())
    })
}

pub(crate) fn genlinmixed(p: &mut Parser) -> PResult<Command> {
    let off = Convergence {
        value: 0.0,
        relative: false,
    };
    let mut g = GenLinMixed {
        subjects: Vec::new(),
        target: String::new(),
        trials: None,
        analysis_weight: None,
        offset: None,
        reference: None,
        distribution: None,
        link: None,
        fixed: Vec::new(),
        intercept: true,
        random: Vec::new(),
        target_order: GenlinOrder::Ascending,
        max_iterations: 100,
        confidence: 95.0,
        df_method: DfMethod::Residual,
        pconverge: Convergence {
            value: 1e-6,
            relative: false,
        },
        lconverge: off,
        hconverge: off,
        scoring: 0,
        singular: 1e-12,
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut power = false;
    let mut link_parameter = None;
    let mut target_options = None;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        const SUBCOMMANDS: [&str; 10] = [
            "DATA_STRUCTURE",
            "FIELDS",
            "TARGET_OPTIONS",
            "FIXED",
            "RANDOM",
            "BUILD_OPTIONS",
            "EMMEANS",
            "EMMEANS_OPTIONS",
            "OUTFILE",
            "SAVE",
        ];
        let Some(&name) = SUBCOMMANDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.subcommand_error("GENLINMIXED", &[]));
        };
        p.pos += 1;
        if name != "RANDOM" {
            if seen.contains(&name) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("subcommand {name} may be given only once"),
                ));
            }
            seen.push(name);
        }
        match name {
            "DATA_STRUCTURE" => {
                settings(
                    p,
                    &[
                        "SUBJECTS",
                        "REPEATED_MEASURES",
                        "KRONECKER_MEASURES",
                        "GROUPING",
                        "COVARIANCE_TYPE",
                        "SPCOORDS",
                    ],
                    |p, keyword, start| {
                        match keyword {
                            "SUBJECTS" => g.subjects = crossed(p)?,
                            // it shapes the repeated measures only
                            "COVARIANCE_TYPE" => {
                                let names: Vec<(&str, ())> =
                                    RESIDUAL_TYPES.iter().map(|n| (*n, ())).collect();
                                choice(p, &names)?;
                            }
                            other => return Err(not_yet(p, start, other)),
                        }
                        Ok(())
                    },
                )?;
                if g.subjects.is_empty() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "DATA_STRUCTURE needs SUBJECTS",
                    ));
                }
            }
            "FIELDS" => {
                settings(
                    p,
                    &[
                        "TARGET",
                        "TRIALS",
                        "ANALYSIS_WEIGHT",
                        "ANALYSISWEIGHT",
                        "OFFSET",
                    ],
                    |p, keyword, _| {
                        match keyword {
                            "TARGET" => g.target = p.name("the target field")?,
                            "TRIALS" => {
                                let at = p.pos;
                                g.trials = match value_or_field(p)? {
                                    None => None,
                                    Some(Ok(n)) if n >= 1.0 && n.fract() == 0.0 => {
                                        Some(GenlinTrials::Number(n))
                                    }
                                    Some(Ok(_)) => {
                                        return Err(SyntaxError::new(
                                            p.span_from(at),
                                            "the number of trials is a whole number, 1 or more",
                                        ));
                                    }
                                    Some(Err(n)) => Some(GenlinTrials::Variable(n)),
                                }
                            }
                            "OFFSET" => {
                                g.offset = value_or_field(p)?.map(|v| match v {
                                    Ok(x) => GenlinOffset::Value(x),
                                    Err(n) => GenlinOffset::Variable(n),
                                })
                            }
                            _ => {
                                if g.analysis_weight.is_some() {
                                    return Err(p.unexpected(&["one analysis weight"]));
                                }
                                g.analysis_weight = Some(p.name("a field")?);
                            }
                        }
                        Ok(())
                    },
                )?;
                if g.target.is_empty() {
                    return Err(SyntaxError::new(p.span_from(start), "FIELDS needs TARGET"));
                }
            }
            "TARGET_OPTIONS" => {
                target_options = Some(p.span_from(start));
                settings(
                    p,
                    &[
                        "REFERENCE",
                        "DISTRIBUTION",
                        "LINK",
                        "LINK_PARAMETER",
                        "LINKPARAMETER",
                    ],
                    |p, keyword, start| {
                        match keyword {
                            "REFERENCE" => {
                                g.reference =
                                    Some(p.value().ok_or_else(|| p.unexpected(&["a value"]))?)
                            }
                            "DISTRIBUTION" => {
                                if p.eat_word("MULTINOMIAL") {
                                    return Err(not_yet(p, start, "DISTRIBUTION=MULTINOMIAL"));
                                }
                                if p.eat_word("NEGATIVE_BINOMIAL") {
                                    return Err(not_yet(
                                        p,
                                        start,
                                        "DISTRIBUTION=NEGATIVE_BINOMIAL",
                                    ));
                                }
                                g.distribution = Some(choice(
                                    p,
                                    &[
                                        ("NORMAL", GenlinDistribution::Normal),
                                        ("BINOMIAL", GenlinDistribution::Binomial),
                                        ("GAMMA", GenlinDistribution::Gamma),
                                        ("INVERSE_GAUSSIAN", GenlinDistribution::InverseGaussian),
                                        ("POISSON", GenlinDistribution::Poisson),
                                    ],
                                )?);
                            }
                            "LINK" => {
                                let l = link(p)?;
                                power = l.is_none();
                                g.link = l;
                            }
                            _ => {
                                if link_parameter.is_some() {
                                    return Err(p.unexpected(&["one link parameter"]));
                                }
                                link_parameter = Some(number(p)?);
                            }
                        }
                        Ok(())
                    },
                )?;
            }
            "FIXED" => {
                settings(p, &["EFFECTS", "USE_INTERCEPT"], |p, keyword, _| {
                    match keyword {
                        "EFFECTS" => g.fixed = effects(p)?,
                        _ => g.intercept = boolean(p)?,
                    }
                    Ok(())
                })?;
            }
            "RANDOM" => g.random.push(random(p, start)?),
            "BUILD_OPTIONS" => {
                settings(
                    p,
                    &[
                        "TARGET_CATEGORY_ORDER",
                        "INPUTS_CATEGORY_ORDER",
                        "MAX_ITERATIONS",
                        "CONFIDENCE_LEVEL",
                        "DF_METHOD",
                        "COVB",
                        "PCONVERGE",
                        "LCONVERGE",
                        "HCONVERGE",
                        "SCORING",
                        "SINGULAR",
                    ],
                    |p, keyword, start| {
                        let order = [
                            ("ASCENDING", GenlinOrder::Ascending),
                            ("DESCENDING", GenlinOrder::Descending),
                            ("DATA", GenlinOrder::Data),
                        ];
                        match keyword {
                            "TARGET_CATEGORY_ORDER" => g.target_order = choice(p, &order)?,
                            "INPUTS_CATEGORY_ORDER" => {
                                if choice(p, &order)? != GenlinOrder::Ascending {
                                    return Err(not_yet(
                                        p,
                                        start,
                                        "INPUTS_CATEGORY_ORDER other than ASCENDING",
                                    ));
                                }
                            }
                            "MAX_ITERATIONS" => g.max_iterations = whole(p, keyword)?,
                            "CONFIDENCE_LEVEL" => {
                                let at = p.pos;
                                let v = number(p)?;
                                if !(v > 0.0 && v < 100.0) {
                                    return Err(SyntaxError::new(
                                        p.span_from(at),
                                        "CONFIDENCE_LEVEL is above 0 and below 100",
                                    ));
                                }
                                g.confidence = v;
                            }
                            "DF_METHOD" => {
                                g.df_method = choice(
                                    p,
                                    &[
                                        ("RESIDUAL", DfMethod::Residual),
                                        ("SATTERTHWAITE", DfMethod::Satterthwaite),
                                        ("KENWARD_ROGER", DfMethod::KenwardRoger),
                                    ],
                                )?
                            }
                            "COVB" => {
                                if choice(p, &[("MODEL", false), ("ROBUST", true)])? {
                                    return Err(not_yet(p, start, "COVB=ROBUST"));
                                }
                            }
                            "PCONVERGE" => g.pconverge = convergence(p, keyword)?,
                            "LCONVERGE" => g.lconverge = convergence(p, keyword)?,
                            "HCONVERGE" => g.hconverge = convergence(p, keyword)?,
                            "SCORING" => g.scoring = whole(p, keyword)?,
                            _ => {
                                let at = p.pos;
                                let v = number(p)?;
                                if v <= 0.0 {
                                    return Err(SyntaxError::new(
                                        p.span_from(at),
                                        "SINGULAR is above 0",
                                    ));
                                }
                                g.singular = v;
                            }
                        }
                        Ok(())
                    },
                )?;
            }
            "EMMEANS_OPTIONS" => emmeans_options(p)?,
            other => return Err(not_yet(p, start, &format!("subcommand {other}"))),
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if g.target.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "GENLINMIXED needs /FIELDS with TARGET",
        ));
    }
    match (power, link_parameter) {
        (true, Some(a)) => g.link = Some(GenlinLink::Power(a)),
        (true, None) => {
            return Err(SyntaxError::new(
                target_options.unwrap_or(p.span()),
                "LINK=POWER needs LINK_PARAMETER=number",
            ));
        }
        _ => {}
    }
    Ok(Command::GenLinMixed(Box::new(g)))
}
