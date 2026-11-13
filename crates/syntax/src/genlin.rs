//! Parser of GENLIN: generalized linear models.
//!
//! Rules from the Command Syntax Reference (GENLIN), in our words:
//! - The command starts with the response: one dependent variable, optionally followed by
//!   `(REFERENCE=LAST|FIRST|value)` and `(ORDER=ASCENDING|DESCENDING|DATA)`, or `events OF
//!   trials`, where trials is a variable or a whole number. Then `BY factors [(ORDER=...)]` and
//!   `WITH covariates`. TO is not allowed in these lists.
//! - Every subcommand, keyword and value name is written in full; no abbreviations.
//! - Each subcommand may appear once, except EMMEANS; each keyword once within its subcommand.
//!   Only EMMEANS may be empty.
//! - MODEL: effects (`a`, `a*b` or `a BY b`, `a(b)`, `x*x`, `a*x`), INTERCEPT=YES|NO,
//!   OFFSET=variable|number, SCALEWEIGHT=variable, DISTRIBUTION and LINK. Without DISTRIBUTION
//!   the distribution is BINOMIAL for events/trials and NORMAL otherwise. Without LINK the link
//!   is the distribution's own: IDENTITY, LOGIT, POWER(-1) for GAMMA, POWER(-2) for IGAUSS,
//!   CUMLOGIT, LOG for NEGBIN and POISSON. Only some pairs of distribution and link are allowed;
//!   MULTINOMIAL takes only the cumulative links and the others never take them.
//! - CRITERIA: ANALYSISTYPE=3|1|ALL [(WALD|LR)], CHECKSEP, CILEVEL (0 to below 100),
//!   CITYPE=WALD, COVB=MODEL|ROBUST, HCONVERGE, LCONVERGE and PCONVERGE with ABSOLUTE or
//!   RELATIVE, LIKELIHOOD=FULL|KERNEL, MAXITERATIONS, MAXSTEPHALVING (above 0),
//!   METHOD=FISHER[(n)]|NEWTON, SCALE=MLE|DEVIANCE|PEARSON|number, SINGULAR. At least one of the
//!   three convergence criteria is above 0. SCALE=MLE is only for NORMAL, GAMMA and IGAUSS.
//! - EMMEANS: TABLES=factor[*factor...], CONTROL=covariate(value|MEAN) ..., SCALE=ORIGINAL|
//!   TRANSFORMED, COMPARE (factors of TABLES; needs TABLES), CONTRAST (needs COMPARE; crossed
//!   factors only PAIRWISE), PADJUST=LSD|BONFERRONI|SEQBONFERRONI|SIDAK|SEQSIDAK.
//! - MISSING: CLASSMISSING=EXCLUDE|INCLUDE.
//! - PRINT: the keywords named, or NONE alone. Without PRINT: CPS, DESCRIPTIVES, FIT,
//!   MODELINFO, SOLUTION and SUMMARY.
//! - SAVE: each keyword with an optional new name in parentheses; the multinomial ones also take
//!   `root:n` or `:n`.

use crate::ast::{Command, Value};
use crate::error::SyntaxError;
use crate::glm::{Term, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser, is_reserved};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenlinOrder {
    Ascending,
    Descending,
    Data,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenlinReference {
    Last,
    First,
    Value(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenlinTrials {
    Variable(String),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenlinResponse {
    Dependent {
        name: String,
        reference: GenlinReference,
        order: GenlinOrder,
    },
    Events {
        events: String,
        trials: GenlinTrials,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenlinDistribution {
    Normal,
    Binomial,
    Gamma,
    InverseGaussian,
    /// The ancillary parameter k, None for its maximum likelihood estimate.
    NegativeBinomial(Option<f64>),
    Poisson,
    Multinomial,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenlinLink {
    Identity,
    Cloglog,
    Log,
    Logc,
    Logit,
    NegativeBinomial,
    Nloglog,
    OddsPower(f64),
    Power(f64),
    Probit,
    CumCauchit,
    CumCloglog,
    CumLogit,
    CumNloglog,
    CumProbit,
}

impl GenlinLink {
    pub fn is_cumulative(self) -> bool {
        matches!(
            self,
            GenlinLink::CumCauchit
                | GenlinLink::CumCloglog
                | GenlinLink::CumLogit
                | GenlinLink::CumNloglog
                | GenlinLink::CumProbit
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenlinOffset {
    Variable(String),
    Value(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenlinScale {
    Mle,
    Deviance,
    Pearson,
    Fixed(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenlinMethod {
    /// Fisher scoring for the first n iterations, Newton-Raphson after.
    Fisher(u32),
    /// Fisher scoring throughout.
    FisherOnly,
    Newton,
}

/// A convergence tolerance and whether the change is relative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenlinTolerance {
    pub value: f64,
    pub relative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenlinCriteria {
    pub type1: bool,
    pub type3: bool,
    /// Likelihood ratio instead of Wald statistics for the effect tests.
    pub lr: bool,
    pub checksep: u32,
    pub cilevel: f64,
    pub robust: bool,
    pub hconverge: GenlinTolerance,
    pub lconverge: GenlinTolerance,
    pub pconverge: GenlinTolerance,
    pub kernel: bool,
    pub maxiterations: u32,
    pub maxstephalving: u32,
    pub method: GenlinMethod,
    /// None: MLE for the continuous distributions, 1 for the others.
    pub scale: Option<GenlinScale>,
    pub singular: f64,
}

impl Default for GenlinCriteria {
    fn default() -> Self {
        let tolerance = |value| GenlinTolerance {
            value,
            relative: false,
        };
        Self {
            type1: false,
            type3: true,
            lr: false,
            checksep: 20,
            cilevel: 95.0,
            robust: false,
            hconverge: tolerance(0.0),
            lconverge: tolerance(0.0),
            pconverge: tolerance(1e-6),
            kernel: false,
            maxiterations: 100,
            maxstephalving: 5,
            method: GenlinMethod::Fisher(1),
            scale: None,
            singular: 1e-12,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenlinContrast {
    Pairwise,
    /// The omitted level as a data value, None for the last.
    Deviation(Option<Value>),
    Difference,
    Helmert,
    /// The metric, None for 1, 2, ..., k.
    Polynomial(Option<Vec<f64>>),
    Repeated,
    Simple(Option<Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenlinAdjust {
    Lsd,
    Bonferroni,
    SeqBonferroni,
    Sidak,
    SeqSidak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenlinEmmeans {
    /// Empty: the overall mean.
    pub tables: Vec<String>,
    /// Covariates at a value; None for the mean.
    pub control: Vec<(String, Option<f64>)>,
    pub transformed: bool,
    pub compare: Option<Vec<String>>,
    /// None: PAIRWISE when COMPARE is given.
    pub contrast: Option<GenlinContrast>,
    pub padjust: GenlinAdjust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenlinPrint {
    pub corb: bool,
    pub covb: bool,
    pub cps: bool,
    pub descriptives: bool,
    pub fit: bool,
    pub modelinfo: bool,
    pub solution: bool,
    pub exponentiated: bool,
    pub summary: bool,
}

impl Default for GenlinPrint {
    fn default() -> Self {
        Self {
            corb: false,
            covb: false,
            cps: true,
            descriptives: true,
            fit: true,
            modelinfo: true,
            solution: true,
            exponentiated: false,
            summary: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenlinSaveKind {
    XbPred,
    XbStdError,
    MeanPred,
    CiMeanPredL,
    CiMeanPredU,
    PredVal,
    Leverage,
    Resid,
    PearsonResid,
    DevianceResid,
    StdPearsonResid,
    StdDevianceResid,
    LikelihoodResid,
    Cook,
}

/// One SAVE keyword: the new name (a root for the multinomial ones) and, for those, the number
/// of categories (25 by default).
#[derive(Debug, Clone, PartialEq)]
pub struct GenlinSave {
    pub kind: GenlinSaveKind,
    pub name: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Genlin {
    pub response: GenlinResponse,
    pub factors: Vec<String>,
    pub factor_order: GenlinOrder,
    pub covariates: Vec<String>,
    pub effects: Vec<Term>,
    pub intercept: bool,
    pub offset: Option<GenlinOffset>,
    pub scale_weight: Option<String>,
    pub distribution: GenlinDistribution,
    pub link: GenlinLink,
    pub criteria: GenlinCriteria,
    pub emmeans: Vec<GenlinEmmeans>,
    /// `/MISSING CLASSMISSING=INCLUDE`
    pub include: bool,
    pub print: GenlinPrint,
    pub save: Vec<GenlinSave>,
}

/// The default link of a distribution.
pub fn default_link(distribution: GenlinDistribution) -> GenlinLink {
    match distribution {
        GenlinDistribution::Normal => GenlinLink::Identity,
        GenlinDistribution::Binomial => GenlinLink::Logit,
        GenlinDistribution::Gamma => GenlinLink::Power(-1.0),
        GenlinDistribution::InverseGaussian => GenlinLink::Power(-2.0),
        GenlinDistribution::NegativeBinomial(_) | GenlinDistribution::Poisson => GenlinLink::Log,
        GenlinDistribution::Multinomial => GenlinLink::CumLogit,
    }
}

/// Whether the manual allows `link` with `distribution`.
pub fn link_allowed(distribution: GenlinDistribution, link: GenlinLink) -> bool {
    use GenlinLink as L;
    match distribution {
        GenlinDistribution::Multinomial => link.is_cumulative(),
        GenlinDistribution::Binomial => !link.is_cumulative() && link != L::NegativeBinomial,
        GenlinDistribution::NegativeBinomial(k) => {
            matches!(link, L::Identity | L::Log | L::Power(_))
                || (link == L::NegativeBinomial && k != Some(0.0))
        }
        _ => matches!(link, L::Identity | L::Log | L::Power(_)),
    }
}

fn word_is(p: &Parser, word: &str) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(word))
}

/// `KEYWORD =` ahead: the start of a keyword setting rather than an effect.
fn at_setting(p: &Parser) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(_)))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
}

fn number(p: &mut Parser, what: &str) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

fn whole(p: &mut Parser, what: &str, min: f64) -> PResult<u32> {
    let start = p.pos;
    let v = number(p, "a whole number")?;
    if v < min || v.fract() != 0.0 || v > f64::from(u32::MAX) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is a whole number, {min} or more"),
        ));
    }
    Ok(v as u32)
}

/// One of the words of `table`, written in full.
fn choice<T: Copy>(p: &mut Parser, table: &[(&str, T)]) -> PResult<T> {
    for (name, value) in table {
        if p.eat_word(name) {
            return Ok(*value);
        }
    }
    Err(p.unexpected(&table.iter().map(|(n, _)| *n).collect::<Vec<_>>()))
}

/// Variable names up to a reserved word, a parenthesis or the end of the subcommand.
fn names(p: &mut Parser, what: &str) -> PResult<Vec<String>> {
    let mut out = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if is_reserved(w) || w.eq_ignore_ascii_case("OF") {
            break;
        }
        out.push(p.name(what)?);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&[what]));
    }
    if word_is(p, "TO") {
        return Err(SyntaxError::new(
            p.span(),
            "GENLIN does not take TO in its variable lists; name every variable",
        ));
    }
    Ok(out)
}

fn order(p: &mut Parser) -> PResult<GenlinOrder> {
    choice(
        p,
        &[
            ("ASCENDING", GenlinOrder::Ascending),
            ("DESCENDING", GenlinOrder::Descending),
            ("DATA", GenlinOrder::Data),
        ],
    )
}

/// `(KEY=value ...)` groups after the dependent variable.
fn dependent_options(p: &mut Parser) -> PResult<(GenlinReference, GenlinOrder)> {
    let mut reference = None;
    let mut ord = None;
    while p.eat(Punct::LParen) {
        while !p.eat(Punct::RParen) {
            let start = p.pos;
            if p.eat_word("REFERENCE") {
                p.expect(Punct::Eq)?;
                let r = if p.eat_word("LAST") {
                    GenlinReference::Last
                } else if p.eat_word("FIRST") {
                    GenlinReference::First
                } else if let Some(v) = p.value() {
                    GenlinReference::Value(v)
                } else {
                    return Err(p.unexpected(&["LAST", "FIRST", "a value"]));
                };
                if reference.replace(r).is_some() {
                    return Err(once(p, start, "REFERENCE"));
                }
            } else if p.eat_word("ORDER") {
                p.expect(Punct::Eq)?;
                if ord.replace(order(p)?).is_some() {
                    return Err(once(p, start, "ORDER"));
                }
            } else {
                return Err(p.unexpected(&["REFERENCE", "ORDER", "')'"]));
            }
            p.eat(Punct::Comma);
        }
    }
    Ok((
        reference.unwrap_or(GenlinReference::Last),
        ord.unwrap_or(GenlinOrder::Ascending),
    ))
}

fn once(p: &Parser, start: usize, keyword: &str) -> SyntaxError {
    SyntaxError::new(
        p.span_from(start),
        format!("{keyword} may be given only once"),
    )
}

fn tolerance(p: &mut Parser, what: &str) -> PResult<GenlinTolerance> {
    let start = p.pos;
    let value = number(p, "a number")?;
    if value < 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is 0 or more"),
        ));
    }
    p.eat(Punct::Comma);
    let mut relative = false;
    let paren = p.eat(Punct::LParen);
    if paren || word_is(p, "ABSOLUTE") || word_is(p, "RELATIVE") {
        relative = choice(p, &[("ABSOLUTE", false), ("RELATIVE", true)])?;
        if paren {
            p.expect(Punct::RParen)?;
        }
    }
    Ok(GenlinTolerance { value, relative })
}

/// Records `keyword` as seen; an error when it was seen before.
fn first_time(
    p: &Parser,
    seen: &mut Vec<&'static str>,
    keyword: &'static str,
    start: usize,
) -> PResult<()> {
    if seen.contains(&keyword) {
        return Err(once(p, start, keyword));
    }
    seen.push(keyword);
    Ok(())
}

fn criteria(p: &mut Parser, c: &mut GenlinCriteria) -> PResult<()> {
    let mut seen = Vec::new();
    let mut any = false;
    while !at_end(p) {
        any = true;
        let start = p.pos;
        let check = |p: &Parser, ok: bool, rule: &str| {
            if ok {
                Ok(())
            } else {
                Err(SyntaxError::new(p.span_from(start), rule.to_string()))
            }
        };
        const KEYWORDS: [&str; 15] = [
            "ANALYSISTYPE",
            "CHECKSEP",
            "CILEVEL",
            "CITYPE",
            "COVB",
            "HCONVERGE",
            "INITIAL",
            "LCONVERGE",
            "LIKELIHOOD",
            "MAXITERATIONS",
            "MAXSTEPHALVING",
            "METHOD",
            "PCONVERGE",
            "SCALE",
            "SINGULAR",
        ];
        let Some(&keyword) = KEYWORDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        first_time(p, &mut seen, keyword, start)?;
        p.expect(Punct::Eq)?;
        match keyword {
            "ANALYSISTYPE" => {
                if p.eat_word("ALL") {
                    c.type1 = true;
                    c.type3 = true;
                } else {
                    let v = number(p, "1, 3 or ALL")?;
                    check(p, v == 1.0 || v == 3.0, "ANALYSISTYPE is 1, 3 or ALL")?;
                    c.type1 = v == 1.0;
                    c.type3 = v == 3.0;
                }
                if p.eat(Punct::LParen) {
                    c.lr = choice(p, &[("WALD", false), ("LR", true)])?;
                    p.expect(Punct::RParen)?;
                }
            }
            "CHECKSEP" => c.checksep = whole(p, "CHECKSEP", 0.0)?,
            "CILEVEL" => {
                let v = number(p, "a number")?;
                check(
                    p,
                    (0.0..100.0).contains(&v),
                    "CILEVEL is at least 0 and below 100",
                )?;
                c.cilevel = v;
            }
            "CITYPE" => {
                if word_is(p, "PROFILE") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "CITYPE=PROFILE of GENLIN is not supported yet; the intervals are Wald intervals",
                    ));
                }
                choice(p, &[("WALD", ())])?;
            }
            "COVB" => c.robust = choice(p, &[("MODEL", false), ("ROBUST", true)])?,
            "HCONVERGE" => c.hconverge = tolerance(p, "HCONVERGE")?,
            "LCONVERGE" => c.lconverge = tolerance(p, "LCONVERGE")?,
            "PCONVERGE" => c.pconverge = tolerance(p, "PCONVERGE")?,
            "INITIAL" => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "INITIAL of GENLIN is not supported yet; the initial values are computed",
                ));
            }
            "LIKELIHOOD" => c.kernel = choice(p, &[("FULL", false), ("KERNEL", true)])?,
            "MAXITERATIONS" => c.maxiterations = whole(p, "MAXITERATIONS", 0.0)?,
            "MAXSTEPHALVING" => c.maxstephalving = whole(p, "MAXSTEPHALVING", 1.0)?,
            "METHOD" => {
                if p.eat_word("NEWTON") {
                    c.method = GenlinMethod::Newton;
                } else if p.eat_word("FISHER") {
                    c.method = GenlinMethod::FisherOnly;
                    if p.eat(Punct::LParen) {
                        c.method = GenlinMethod::Fisher(whole(p, "FISHER(n)", 1.0)?);
                        p.expect(Punct::RParen)?;
                    }
                } else {
                    return Err(p.unexpected(&["FISHER", "NEWTON"]));
                }
            }
            "SCALE" => {
                c.scale = Some(if p.eat_word("MLE") {
                    GenlinScale::Mle
                } else if p.eat_word("DEVIANCE") {
                    GenlinScale::Deviance
                } else if p.eat_word("PEARSON") {
                    GenlinScale::Pearson
                } else {
                    let v = number(p, "MLE, DEVIANCE, PEARSON or a number")?;
                    check(p, v > 0.0, "a fixed SCALE is above 0")?;
                    GenlinScale::Fixed(v)
                });
            }
            "SINGULAR" => {
                let v = number(p, "a number")?;
                check(p, v > 0.0, "SINGULAR is above 0")?;
                c.singular = v;
            }
            _ => unreachable!("listed"),
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["a CRITERIA keyword"]));
    }
    Ok(())
}

/// Factors joined by `*` or BY.
fn crossing(p: &mut Parser) -> PResult<Vec<String>> {
    let mut out = vec![p.name("a factor")?];
    while p.eat(Punct::Star) || p.eat_word("BY") {
        let start = p.pos;
        let name = p.name("a factor")?;
        if out.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} appears twice in a crossing of factors"),
            ));
        }
        out.push(name);
    }
    Ok(out)
}

fn level_value(p: &mut Parser) -> PResult<Option<Value>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let Some(v) = p.value() else {
        return Err(p.unexpected(&["a value"]));
    };
    p.expect(Punct::RParen)?;
    Ok(Some(v))
}

fn emmeans(p: &mut Parser) -> PResult<GenlinEmmeans> {
    let mut e = GenlinEmmeans {
        tables: Vec::new(),
        control: Vec::new(),
        transformed: false,
        compare: None,
        contrast: None,
        padjust: GenlinAdjust::Lsd,
    };
    let mut seen = Vec::new();
    let begin = p.pos;
    while !at_end(p) {
        let start = p.pos;
        const KEYWORDS: [&str; 6] = [
            "TABLES", "CONTROL", "SCALE", "COMPARE", "CONTRAST", "PADJUST",
        ];
        let Some(&keyword) = KEYWORDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        first_time(p, &mut seen, keyword, start)?;
        p.expect(Punct::Eq)?;
        match keyword {
            "TABLES" => e.tables = crossing(p)?,
            "CONTROL" => {
                while matches!(p.peek(), Some(TokenKind::Word(_))) && !at_setting(p) {
                    let at = p.pos;
                    let name = p.name("a covariate")?;
                    if e.control.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            format!("{name} is given twice on CONTROL"),
                        ));
                    }
                    p.expect(Punct::LParen)?;
                    let value = if p.eat_word("MEAN") {
                        None
                    } else {
                        Some(number(p, "a number or MEAN")?)
                    };
                    p.expect(Punct::RParen)?;
                    e.control.push((name, value));
                    p.eat(Punct::Comma);
                }
                if e.control.is_empty() {
                    return Err(p.unexpected(&["a covariate"]));
                }
            }
            "SCALE" => {
                e.transformed = choice(p, &[("ORIGINAL", false), ("TRANSFORMED", true)])?;
            }
            "COMPARE" => e.compare = Some(crossing(p)?),
            "CONTRAST" => {
                e.contrast = Some(if p.eat_word("PAIRWISE") {
                    GenlinContrast::Pairwise
                } else if p.eat_word("DEVIATION") {
                    GenlinContrast::Deviation(level_value(p)?)
                } else if p.eat_word("DIFFERENCE") {
                    GenlinContrast::Difference
                } else if p.eat_word("HELMERT") {
                    GenlinContrast::Helmert
                } else if p.eat_word("POLYNOMIAL") {
                    let mut metric = None;
                    if p.eat(Punct::LParen) {
                        let mut list = Vec::new();
                        while !p.eat(Punct::RParen) {
                            list.push(number(p, "a number")?);
                            p.eat(Punct::Comma);
                        }
                        let mut sorted = list.clone();
                        sorted.sort_by(f64::total_cmp);
                        if list.is_empty() || sorted.windows(2).any(|w| w[0] == w[1]) {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                "a POLYNOMIAL metric needs different numbers",
                            ));
                        }
                        metric = Some(list);
                    }
                    GenlinContrast::Polynomial(metric)
                } else if p.eat_word("REPEATED") {
                    GenlinContrast::Repeated
                } else if p.eat_word("SIMPLE") {
                    GenlinContrast::Simple(level_value(p)?)
                } else {
                    return Err(p.unexpected(&[
                        "PAIRWISE",
                        "DEVIATION",
                        "DIFFERENCE",
                        "HELMERT",
                        "POLYNOMIAL",
                        "REPEATED",
                        "SIMPLE",
                    ]));
                });
            }
            "PADJUST" => {
                e.padjust = choice(
                    p,
                    &[
                        ("LSD", GenlinAdjust::Lsd),
                        ("BONFERRONI", GenlinAdjust::Bonferroni),
                        ("SEQBONFERRONI", GenlinAdjust::SeqBonferroni),
                        ("SIDAK", GenlinAdjust::Sidak),
                        ("SEQSIDAK", GenlinAdjust::SeqSidak),
                    ],
                )?;
            }
            _ => unreachable!("listed"),
        }
        p.eat(Punct::Comma);
    }
    let fail = |message: &str| Err(SyntaxError::new(p.span_from(begin), message.to_string()));
    match &e.compare {
        Some(compare) => {
            if e.tables.is_empty() {
                return fail("COMPARE needs TABLES");
            }
            if let Some(f) = compare
                .iter()
                .find(|c| !e.tables.iter().any(|t| t.eq_ignore_ascii_case(c)))
            {
                return fail(&format!("COMPARE names {f}, which is not on TABLES"));
            }
            if compare.len() > 1 && !matches!(e.contrast, None | Some(GenlinContrast::Pairwise)) {
                return fail("crossed factors on COMPARE take only CONTRAST=PAIRWISE");
            }
        }
        None if e.contrast.is_some() => return fail("CONTRAST needs COMPARE"),
        None => {}
    }
    Ok(e)
}

fn print(p: &mut Parser) -> PResult<GenlinPrint> {
    let mut out = GenlinPrint {
        cps: false,
        descriptives: false,
        fit: false,
        modelinfo: false,
        solution: false,
        summary: false,
        ..GenlinPrint::default()
    };
    let mut seen = Vec::new();
    let mut none = false;
    while !at_end(p) {
        let start = p.pos;
        const KEYWORDS: [&str; 14] = [
            "CORB",
            "COVB",
            "CPS",
            "DESCRIPTIVES",
            "FIT",
            "GEF",
            "HISTORY",
            "LAGRANGE",
            "LMATRIX",
            "MODELINFO",
            "SOLUTION",
            "SUMMARY",
            "WORKINGCORR",
            "NONE",
        ];
        let Some(&keyword) = KEYWORDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        first_time(p, &mut seen, keyword, start)?;
        match keyword {
            "CORB" => out.corb = true,
            "COVB" => out.covb = true,
            "CPS" => out.cps = true,
            "DESCRIPTIVES" => out.descriptives = true,
            "FIT" => out.fit = true,
            "MODELINFO" => out.modelinfo = true,
            "SUMMARY" => out.summary = true,
            "SOLUTION" => {
                out.solution = true;
                if p.eat(Punct::LParen) {
                    choice(p, &[("EXPONENTIATED", ())])?;
                    p.expect(Punct::RParen)?;
                    out.exponentiated = true;
                }
            }
            // a working correlation matrix exists only with REPEATED, which is not supported
            "WORKINGCORR" => {}
            "NONE" => none = true,
            _ => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("PRINT {keyword} of GENLIN is not supported yet"),
                ));
            }
        }
        p.eat(Punct::Comma);
    }
    if seen.is_empty() {
        return Err(p.unexpected(&["a PRINT keyword"]));
    }
    if none && seen.len() > 1 {
        return Err(SyntaxError::new(
            p.span(),
            "PRINT NONE takes no other keyword",
        ));
    }
    Ok(out)
}

fn save(p: &mut Parser) -> PResult<Vec<GenlinSave>> {
    use GenlinSaveKind as K;
    const KEYWORDS: [(&str, GenlinSaveKind); 14] = [
        ("XBPRED", K::XbPred),
        ("XBSTDERROR", K::XbStdError),
        ("MEANPRED", K::MeanPred),
        ("CIMEANPREDL", K::CiMeanPredL),
        ("CIMEANPREDU", K::CiMeanPredU),
        ("PREDVAL", K::PredVal),
        ("LEVERAGE", K::Leverage),
        ("RESID", K::Resid),
        ("PEARSONRESID", K::PearsonResid),
        ("DEVIANCERESID", K::DevianceResid),
        ("STDPEARSONRESID", K::StdPearsonResid),
        ("STDDEVIANCERESID", K::StdDevianceResid),
        ("LIKELIHOODRESID", K::LikelihoodResid),
        ("COOK", K::Cook),
    ];
    let mut out: Vec<GenlinSave> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let kind = choice(p, &KEYWORDS)?;
        if out.iter().any(|s| s.kind == kind) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "each SAVE keyword is given only once",
            ));
        }
        let rooted = matches!(
            kind,
            K::XbPred | K::XbStdError | K::MeanPred | K::CiMeanPredL | K::CiMeanPredU
        );
        let mut name = None;
        let mut count = 25;
        if p.eat(Punct::LParen) {
            if matches!(p.peek(), Some(TokenKind::Word(_))) {
                name = Some(p.name("a new variable name")?);
            }
            if rooted && p.eat(Punct::Colon) {
                let at = p.pos;
                match p.number() {
                    Some(n) if n >= 1.0 && n.fract() == 0.0 => count = n as usize,
                    _ => {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "the number of categories after ':' is a whole number, 1 or more",
                        ));
                    }
                }
            }
            if name.is_none() && count == 25 && !rooted {
                return Err(p.unexpected(&["a new variable name"]));
            }
            p.expect(Punct::RParen)?;
        }
        out.push(GenlinSave { kind, name, count });
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a SAVE keyword"]));
    }
    Ok(out)
}

/// What MODEL says.
#[derive(Default)]
struct ModelSpec {
    effects: Vec<Term>,
    intercept: Option<bool>,
    offset: Option<GenlinOffset>,
    scale_weight: Option<String>,
    distribution: Option<GenlinDistribution>,
    link: Option<GenlinLink>,
}

fn model(p: &mut Parser) -> PResult<ModelSpec> {
    let mut m = ModelSpec::default();
    let mut seen = Vec::new();
    while !at_end(p) {
        if !at_setting(p) {
            m.effects.push(term(p)?);
            p.eat(Punct::Comma);
            continue;
        }
        let start = p.pos;
        const KEYWORDS: [&str; 5] = ["INTERCEPT", "OFFSET", "SCALEWEIGHT", "DISTRIBUTION", "LINK"];
        let Some(&keyword) = KEYWORDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        first_time(p, &mut seen, keyword, start)?;
        p.expect(Punct::Eq)?;
        match keyword {
            "INTERCEPT" => m.intercept = Some(choice(p, &[("YES", true), ("NO", false)])?),
            "OFFSET" => {
                m.offset = Some(match p.number() {
                    Some(v) => GenlinOffset::Value(v),
                    None => GenlinOffset::Variable(p.name("a variable or a number")?),
                });
            }
            "SCALEWEIGHT" => m.scale_weight = Some(p.name("a variable")?),
            "DISTRIBUTION" => {
                let d = if p.eat_word("NEGBIN") {
                    let mut k = Some(1.0);
                    if p.eat(Punct::LParen) {
                        k = if p.eat_word("MLE") {
                            None
                        } else {
                            let at = p.pos;
                            let v = number(p, "a number or MLE")?;
                            if v < 0.0 {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "the NEGBIN parameter is 0 or more",
                                ));
                            }
                            Some(v)
                        };
                        p.expect(Punct::RParen)?;
                    }
                    GenlinDistribution::NegativeBinomial(k)
                } else if word_is(p, "TWEEDIE") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "DISTRIBUTION=TWEEDIE of GENLIN is not supported yet",
                    ));
                } else {
                    choice(
                        p,
                        &[
                            ("NORMAL", GenlinDistribution::Normal),
                            ("BINOMIAL", GenlinDistribution::Binomial),
                            ("GAMMA", GenlinDistribution::Gamma),
                            ("IGAUSS", GenlinDistribution::InverseGaussian),
                            ("POISSON", GenlinDistribution::Poisson),
                            ("MULTINOMIAL", GenlinDistribution::Multinomial),
                        ],
                    )?
                };
                m.distribution = Some(d);
            }
            "LINK" => {
                let parameter = |p: &mut Parser| -> PResult<f64> {
                    p.expect(Punct::LParen)?;
                    let v = number(p, "a number")?;
                    p.expect(Punct::RParen)?;
                    Ok(v)
                };
                m.link = Some(if p.eat_word("POWER") {
                    GenlinLink::Power(parameter(p)?)
                } else if p.eat_word("ODDSPOWER") {
                    GenlinLink::OddsPower(parameter(p)?)
                } else {
                    choice(
                        p,
                        &[
                            ("IDENTITY", GenlinLink::Identity),
                            ("CLOGLOG", GenlinLink::Cloglog),
                            ("LOGC", GenlinLink::Logc),
                            ("LOGIT", GenlinLink::Logit),
                            ("LOG", GenlinLink::Log),
                            ("NEGBIN", GenlinLink::NegativeBinomial),
                            ("NLOGLOG", GenlinLink::Nloglog),
                            ("PROBIT", GenlinLink::Probit),
                            ("CUMCAUCHIT", GenlinLink::CumCauchit),
                            ("CUMCLOGLOG", GenlinLink::CumCloglog),
                            ("CUMLOGIT", GenlinLink::CumLogit),
                            ("CUMNLOGLOG", GenlinLink::CumNloglog),
                            ("CUMPROBIT", GenlinLink::CumProbit),
                        ],
                    )?
                });
            }
            _ => unreachable!("listed"),
        }
        p.eat(Punct::Comma);
    }
    if m.effects.is_empty() && seen.is_empty() {
        return Err(p.unexpected(&["an effect", "a MODEL keyword"]));
    }
    Ok(m)
}

pub(crate) fn genlin(p: &mut Parser) -> PResult<Command> {
    let first = p.name("the dependent variable or the events variable")?;
    let response = if p.eat_word("OF") {
        let trials = match p.number() {
            Some(n) => {
                if n < 1.0 || n.fract() != 0.0 {
                    return Err(SyntaxError::new(
                        p.span_from(p.pos - 1),
                        "the number of trials is a whole number, 1 or more",
                    ));
                }
                GenlinTrials::Number(n)
            }
            None => GenlinTrials::Variable(p.name("the trials variable or a number")?),
        };
        GenlinResponse::Events {
            events: first,
            trials,
        }
    } else {
        let (reference, order) = dependent_options(p)?;
        GenlinResponse::Dependent {
            name: first,
            reference,
            order,
        }
    };
    if !(at_end(p) || word_is(p, "BY") || word_is(p, "WITH")) {
        return Err(SyntaxError::new(
            p.span(),
            "GENLIN takes one dependent variable (or events OF trials); factors follow BY and covariates WITH",
        ));
    }
    let mut factors = Vec::new();
    let mut factor_order = GenlinOrder::Ascending;
    if p.eat_word("BY") {
        factors = names(p, "a factor")?;
        if p.eat(Punct::LParen) {
            if !p.eat_word("ORDER") {
                return Err(p.unexpected(&["ORDER"]));
            }
            p.expect(Punct::Eq)?;
            factor_order = order(p)?;
            p.expect(Punct::RParen)?;
        }
    }
    let covariates = if p.eat_word("WITH") {
        names(p, "a covariate")?
    } else {
        Vec::new()
    };
    let mut spec = ModelSpec::default();
    let mut model_span = p.span();
    let mut criteria_spec = GenlinCriteria::default();
    let mut emmeans_list = Vec::new();
    let mut include = false;
    let mut print_spec = None;
    let mut save_list = Vec::new();
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        const SUBCOMMANDS: [&str; 8] = [
            "MODEL", "CRITERIA", "REPEATED", "EMMEANS", "MISSING", "PRINT", "SAVE", "OUTFILE",
        ];
        let Some(&name) = SUBCOMMANDS.iter().find(|k| word_is(p, k)) else {
            return Err(p.unexpected(&SUBCOMMANDS));
        };
        p.pos += 1;
        if name != "EMMEANS" {
            first_time(p, &mut seen, name, start)?;
        }
        match name {
            "MODEL" => {
                spec = model(p)?;
                model_span = p.span_from(start);
            }
            "CRITERIA" => criteria(p, &mut criteria_spec)?,
            "REPEATED" | "OUTFILE" => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!(
                        "subcommand {name} of GENLIN is not supported yet{}",
                        if name == "REPEATED" {
                            " (generalized estimating equations)"
                        } else {
                            ""
                        }
                    ),
                ));
            }
            "EMMEANS" => emmeans_list.push(emmeans(p)?),
            "MISSING" => {
                if !p.eat_word("CLASSMISSING") {
                    return Err(p.unexpected(&["CLASSMISSING"]));
                }
                p.expect(Punct::Eq)?;
                include = choice(p, &[("EXCLUDE", false), ("INCLUDE", true)])?;
            }
            "PRINT" => print_spec = Some(print(p)?),
            "SAVE" => save_list = save(p)?,
            _ => unreachable!("listed"),
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let events = matches!(response, GenlinResponse::Events { .. });
    let distribution = spec.distribution.unwrap_or(if events {
        GenlinDistribution::Binomial
    } else {
        GenlinDistribution::Normal
    });
    let link = spec.link.unwrap_or(default_link(distribution));
    let model_error = |message: String| Err(SyntaxError::new(model_span, message));
    if !link_allowed(distribution, link) {
        return model_error(format!(
            "LINK={} cannot be used with DISTRIBUTION={}",
            link_keyword(link),
            distribution_keyword(distribution)
        ));
    }
    if events && distribution != GenlinDistribution::Binomial {
        return model_error("an events/trials response needs DISTRIBUTION=BINOMIAL".into());
    }
    let continuous = matches!(
        distribution,
        GenlinDistribution::Normal
            | GenlinDistribution::Gamma
            | GenlinDistribution::InverseGaussian
    );
    if criteria_spec.scale == Some(GenlinScale::Mle) && !continuous {
        return model_error(format!(
            "SCALE=MLE is for the NORMAL, GAMMA and IGAUSS distributions, not {}",
            distribution_keyword(distribution)
        ));
    }
    let c = &criteria_spec;
    if c.hconverge.value == 0.0 && c.lconverge.value == 0.0 && c.pconverge.value == 0.0 {
        return model_error(
            "at least one of HCONVERGE, LCONVERGE and PCONVERGE on CRITERIA must be above 0".into(),
        );
    }
    Ok(Command::Genlin(Box::new(Genlin {
        response,
        factors,
        factor_order,
        covariates,
        effects: spec.effects,
        intercept: spec.intercept.unwrap_or(true),
        offset: spec.offset,
        scale_weight: spec.scale_weight,
        distribution,
        link,
        criteria: criteria_spec,
        emmeans: emmeans_list,
        include,
        print: print_spec.unwrap_or_default(),
        save: save_list,
    })))
}

pub fn distribution_keyword(d: GenlinDistribution) -> &'static str {
    match d {
        GenlinDistribution::Normal => "NORMAL",
        GenlinDistribution::Binomial => "BINOMIAL",
        GenlinDistribution::Gamma => "GAMMA",
        GenlinDistribution::InverseGaussian => "IGAUSS",
        GenlinDistribution::NegativeBinomial(_) => "NEGBIN",
        GenlinDistribution::Poisson => "POISSON",
        GenlinDistribution::Multinomial => "MULTINOMIAL",
    }
}

pub fn link_keyword(l: GenlinLink) -> &'static str {
    match l {
        GenlinLink::Identity => "IDENTITY",
        GenlinLink::Cloglog => "CLOGLOG",
        GenlinLink::Log => "LOG",
        GenlinLink::Logc => "LOGC",
        GenlinLink::Logit => "LOGIT",
        GenlinLink::NegativeBinomial => "NEGBIN",
        GenlinLink::Nloglog => "NLOGLOG",
        GenlinLink::OddsPower(_) => "ODDSPOWER",
        GenlinLink::Power(_) => "POWER",
        GenlinLink::Probit => "PROBIT",
        GenlinLink::CumCauchit => "CUMCAUCHIT",
        GenlinLink::CumCloglog => "CUMCLOGLOG",
        GenlinLink::CumLogit => "CUMLOGIT",
        GenlinLink::CumNloglog => "CUMNLOGLOG",
        GenlinLink::CumProbit => "CUMPROBIT",
    }
}
