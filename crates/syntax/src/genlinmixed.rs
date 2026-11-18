//! Parser of GENLINMIXED: generalized linear mixed models.
//!
//! Rules (our notes from the Command Syntax Reference, GENLINMIXED):
//! - Subcommands in any order; FIELDS is required. RANDOM may repeat (one covariance block
//!   each); every other subcommand appears once. A subcommand is never empty and a keyword
//!   appears once within it. Names and keywords are written in full.
//! - DATA_STRUCTURE: SUBJECTS=a*b (or a BY b) is required there. REPEATED_MEASURES=a*b gives
//!   the residual block of each subject, COVARIANCE_TYPE its structure (DIAGONAL by default;
//!   the spatial and Kronecker ones are not supported yet, nor KRONECKER_MEASURES, GROUPING and
//!   SPCOORDS).
//! - FIELDS: TARGET=name, TRIALS=NONE | VALUE(n) | FIELD(name), ANALYSIS_WEIGHT=name,
//!   OFFSET=NONE | VALUE(x) | FIELD(name). We also take a bare name or number for TRIALS and
//!   OFFSET and the spelling ANALYSISWEIGHT.
//! - TARGET_OPTIONS: REFERENCE=value, DISTRIBUTION, LINK (POWER takes LINK_PARAMETER, or
//!   POWER(x)). Without DISTRIBUTION the engine picks one from the target's measurement level.
//! - FIXED: EFFECTS=terms, USE_INTERCEPT=TRUE (default) | FALSE.
//! - RANDOM: EFFECTS, USE_INTERCEPT (FALSE by default; a block needs an effect or the
//!   intercept), SUBJECTS, COVARIANCE_TYPE (VARIANCE_COMPONENTS by default), SOLUTION.
//! - BUILD_OPTIONS: TARGET_CATEGORY_ORDER, INPUTS_CATEGORY_ORDER (ASCENDING only for now),
//!   MAX_ITERATIONS (100), CONFIDENCE_LEVEL (95), DF_METHOD (RESIDUAL), COVB (MODEL | ROBUST),
//!   PCONVERGE (1E-6), LCONVERGE and HCONVERGE (0), each with (ABSOLUTE | RELATIVE),
//!   SCORING (0), SINGULAR (1E-12).
//! - EMMEANS may repeat and may be empty: TABLES=a*b, COMPARE=a (one of TABLES), CONTRAST=NONE |
//!   PAIRWISE | DEVIATION | SIMPLE; COMPARE without CONTRAST compares pairwise, CONTRAST other
//!   than NONE needs COMPARE. EMMEANS_OPTIONS: CONTROL=x(value) ..., SCALE=ORIGINAL |
//!   TRANSFORMED, PADJUST=LSD | SEQBONFERRONI | SEQSIDAK.
//! - OUTFILE MODEL='file'. SAVE: PREDICTED_VALUES, PREDICTED_PROBABILITY, CONFIDENCE,
//!   CONFIDENCE_INTERVALS and PEARSON_RESIDUALS, each with an optional (name);
//!   MAX_CATEGORIES(n) and CONFIDENCE_METHOD(PREDICTEDPROBABILITY | PROBABILITYIMPROVEMENT).

use crate::ast::{Command, Value};
use crate::error::SyntaxError;
use crate::genlin::{
    GenlinAdjust, GenlinDistribution, GenlinLink, GenlinOffset, GenlinOrder, GenlinTrials,
};
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

/// The contrast of one EMMEANS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlmmContrast {
    None,
    Pairwise,
    Deviation,
    Simple,
}

/// One EMMEANS subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct GlmmEmmeans {
    /// The factors of TABLES; empty for the overall mean.
    pub tables: Vec<String>,
    pub compare: Option<String>,
    pub contrast: GlmmContrast,
}

/// SAVE: the names given, or None for a field not saved.
#[derive(Debug, Clone, PartialEq)]
pub struct GlmmSave {
    pub predicted: Option<String>,
    pub probability: Option<String>,
    pub max_categories: usize,
    pub confidence: Option<String>,
    /// CONFIDENCE_METHOD(PROBABILITYIMPROVEMENT).
    pub improvement: bool,
    /// The root of the _Lower and _Upper fields.
    pub intervals: Option<String>,
    pub pearson: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenLinMixed {
    /// DATA_STRUCTURE SUBJECTS.
    pub subjects: Vec<String>,
    /// DATA_STRUCTURE REPEATED_MEASURES and the structure of their residual block.
    pub repeated: Vec<String>,
    pub residual_covtype: CovType,
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
    /// COVB=ROBUST.
    pub robust: bool,
    pub emmeans: Vec<GlmmEmmeans>,
    /// EMMEANS_OPTIONS CONTROL, SCALE=TRANSFORMED and PADJUST.
    pub control: Vec<(String, f64)>,
    pub transformed: bool,
    pub padjust: GenlinAdjust,
    pub outfile: Option<String>,
    pub save: Option<GlmmSave>,
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

/// COVARIANCE_TYPE; the spatial and Kronecker types belong to the residuals (`residual`).
fn covariance_type(p: &mut Parser, residual: bool) -> PResult<CovType> {
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
        None if residual => Err(not_yet(p, start, "this COVARIANCE_TYPE")),
        None => Err(SyntaxError::new(
            p.span_from(start),
            "this COVARIANCE_TYPE belongs to repeated measures on DATA_STRUCTURE, not to RANDOM",
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
                "COVARIANCE_TYPE" => r.covtype = covariance_type(p, false)?,
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

fn emmeans_options(p: &mut Parser, g: &mut GenLinMixed) -> PResult<()> {
    settings(p, &["CONTROL", "SCALE", "PADJUST"], |p, keyword, _| {
        match keyword {
            "CONTROL" => loop {
                let start = p.pos;
                let name = p.name("a covariate")?;
                if g.control.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!("{name} is on CONTROL twice"),
                    ));
                }
                p.expect(Punct::LParen)?;
                let v = number(p)?;
                p.expect(Punct::RParen)?;
                g.control.push((name, v));
                p.eat(Punct::Comma);
                if at_end(p) || at_setting(p) {
                    break;
                }
            },
            "SCALE" => g.transformed = choice(p, &[("ORIGINAL", false), ("TRANSFORMED", true)])?,
            _ => {
                g.padjust = choice(
                    p,
                    &[
                        ("LSD", GenlinAdjust::Lsd),
                        ("SEQBONFERRONI", GenlinAdjust::SeqBonferroni),
                        ("SEQSIDAK", GenlinAdjust::SeqSidak),
                    ],
                )?
            }
        }
        Ok(())
    })
}

/// One EMMEANS; it may be empty (the overall mean).
fn emmeans(p: &mut Parser, span_start: usize) -> PResult<GlmmEmmeans> {
    let mut e = GlmmEmmeans {
        tables: Vec::new(),
        compare: None,
        contrast: GlmmContrast::None,
    };
    let mut contrast = None;
    if !at_end(p) {
        settings(p, &["TABLES", "COMPARE", "CONTRAST"], |p, keyword, _| {
            match keyword {
                "TABLES" => e.tables = crossed(p)?,
                "COMPARE" => e.compare = Some(p.name("a factor")?),
                _ => {
                    contrast = Some(choice(
                        p,
                        &[
                            ("NONE", GlmmContrast::None),
                            ("PAIRWISE", GlmmContrast::Pairwise),
                            ("DEVIATION", GlmmContrast::Deviation),
                            ("SIMPLE", GlmmContrast::Simple),
                        ],
                    )?)
                }
            }
            Ok(())
        })?;
    }
    if let Some(c) = &e.compare
        && !e.tables.iter().any(|t| t.eq_ignore_ascii_case(c))
    {
        return Err(SyntaxError::new(
            p.span_from(span_start),
            format!("COMPARE names {c}, which is not on TABLES"),
        ));
    }
    e.contrast = match (contrast, &e.compare) {
        (Some(c), Some(_)) => c,
        (None, Some(_)) => GlmmContrast::Pairwise,
        (Some(GlmmContrast::None) | None, None) => GlmmContrast::None,
        (Some(_), None) => {
            return Err(SyntaxError::new(
                p.span_from(span_start),
                "CONTRAST needs COMPARE",
            ));
        }
    };
    Ok(e)
}

/// SAVE: keywords with an optional name in parentheses.
fn save(p: &mut Parser) -> PResult<GlmmSave> {
    let mut s = GlmmSave {
        predicted: None,
        probability: None,
        max_categories: 25,
        confidence: None,
        improvement: false,
        intervals: None,
        pearson: None,
    };
    let keywords = [
        "PREDICTED_VALUES",
        "PREDICTED_PROBABILITY",
        "MAX_CATEGORIES",
        "CONFIDENCE_METHOD",
        "CONFIDENCE_INTERVALS",
        "CONFIDENCE",
        "PEARSON_RESIDUALS",
    ];
    let mut seen: Vec<&str> = Vec::new();
    if at_end(p) {
        return Err(p.unexpected(&keywords));
    }
    while !at_end(p) {
        let start = p.pos;
        let Some(&keyword) = keywords.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&keywords));
        };
        p.pos += 1;
        if seen.contains(&keyword) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{keyword} may be given only once"),
            ));
        }
        seen.push(keyword);
        let given = p.eat(Punct::LParen);
        let name = |p: &mut Parser, default: &str| -> PResult<Option<String>> {
            Ok(Some(if given {
                p.name("a field name")?
            } else {
                default.to_string()
            }))
        };
        match keyword {
            "PREDICTED_VALUES" => s.predicted = name(p, "PredictedValue")?,
            "PREDICTED_PROBABILITY" => s.probability = name(p, "PredictedProbability")?,
            "CONFIDENCE" => s.confidence = name(p, "Confidence")?,
            "CONFIDENCE_INTERVALS" => s.intervals = name(p, "CI")?,
            "PEARSON_RESIDUALS" => s.pearson = name(p, "PearsonResidual")?,
            "MAX_CATEGORIES" => {
                if !given {
                    return Err(p.unexpected(&["'('"]));
                }
                let at = p.pos;
                let n = whole(p, keyword)?;
                if n == 0 {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "MAX_CATEGORIES is 1 or more",
                    ));
                }
                s.max_categories = n;
            }
            _ => {
                if !given {
                    return Err(p.unexpected(&["'('"]));
                }
                s.improvement = choice(
                    p,
                    &[
                        ("PREDICTEDPROBABILITY", false),
                        ("PROBABILITYIMPROVEMENT", true),
                    ],
                )?;
            }
        }
        if given {
            p.expect(Punct::RParen)?;
        }
        p.eat(Punct::Comma);
    }
    Ok(s)
}

pub(crate) fn genlinmixed(p: &mut Parser) -> PResult<Command> {
    let off = Convergence {
        value: 0.0,
        relative: false,
    };
    let mut g = GenLinMixed {
        subjects: Vec::new(),
        repeated: Vec::new(),
        residual_covtype: CovType::Diag,
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
        robust: false,
        emmeans: Vec::new(),
        control: Vec::new(),
        transformed: false,
        padjust: GenlinAdjust::Lsd,
        outfile: None,
        save: None,
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
        if name != "RANDOM" && name != "EMMEANS" {
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
                            "REPEATED_MEASURES" => g.repeated = crossed(p)?,
                            "COVARIANCE_TYPE" => g.residual_covtype = covariance_type(p, true)?,
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
                                g.distribution = Some(choice(
                                    p,
                                    &[
                                        ("NORMAL", GenlinDistribution::Normal),
                                        ("BINOMIAL", GenlinDistribution::Binomial),
                                        ("GAMMA", GenlinDistribution::Gamma),
                                        ("INVERSE_GAUSSIAN", GenlinDistribution::InverseGaussian),
                                        (
                                            "NEGATIVE_BINOMIAL",
                                            GenlinDistribution::NegativeBinomial(None),
                                        ),
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
                            "COVB" => g.robust = choice(p, &[("MODEL", false), ("ROBUST", true)])?,
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
            "EMMEANS_OPTIONS" => emmeans_options(p, &mut g)?,
            "EMMEANS" => g.emmeans.push(emmeans(p, start)?),
            "OUTFILE" => {
                settings(p, &["MODEL"], |p, _, _| {
                    g.outfile = Some(p.file("a file name")?);
                    Ok(())
                })?;
            }
            _ => g.save = Some(save(p)?),
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
