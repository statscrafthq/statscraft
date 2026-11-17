//! Parser of CSCOXREG: Cox proportional hazards regression on a complex sample.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The variable list comes first: the end time, or a start time and an end time, then factors
//!   after BY and covariates after WITH. VARIABLES STATUS=var(values) and PLAN FILE= are
//!   required. The event values are numbers, THRU ranges or quoted strings.
//! - VARIABLES may also name BASELINESTRATA=var (a baseline hazard per value) and ID=var.
//! - MODEL, CUSTOM, STATISTICS, TEST, DOMAIN and MISSING work as for the other complex samples
//!   models; there is no intercept (the baseline hazard absorbs it), so an L row has no
//!   intercept coefficient. CRITERIA: CILEVEL, DF, LCONVERGE, MXITER, MXSTEP, PCONVERGE,
//!   SINGULAR and TIES=EFRON (default) | BRESLOW.
//! - PRINT SAMPLEINFO EVENTINFO RISKINFO HISTORY[(n)] GEF LMATRIX COVB CORB BASELINE NONE; a
//!   PRINT shows only what it names, and NONE gives way to any other keyword.
//! - SURVIVALMETHOD BASELINE=EFRON | BRESLOW | PRODUCTLIMIT (default: as TIES) CI=ORIGINAL |
//!   LOG (default) | LML.
//! - SAVE keyword[(name)] for one variable, keyword[(root[:n])] for one per parameter (25 at
//!   most by default); the AGG keywords need VARIABLES ID.
//! - PLOT SURVIVAL CUMHAZARD (or HAZARD) LML OMS CI=NO | YES.
//! - PATTERN var(value) ... [BY factor]; a value list for the end time is a piecewise path.
//! - OUTFILE COVB | CORB = file, MODEL = file, SURVIVAL = file.
//! - CUSTOM and PATTERN may repeat; every other subcommand comes at most once and none may be
//!   empty.

use super::{CsCriteria, CsCustom, CsModelStatistics, CsPadjust, CsTestType};
use super::{checked, eat_assignment, error, joint_file, once, plan_file, require_assignment};
use super::{convergence, custom, flag, pick, statistics, whole};
use crate::ast::{Command, MissingValue, Value, VarItem};
use crate::glm::{Term, list_until, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsCoxPrint {
    pub sampleinfo: bool,
    pub eventinfo: bool,
    pub riskinfo: bool,
    /// HISTORY(n): every nth iteration.
    pub history: Option<u32>,
    pub lmatrix: bool,
    pub covb: bool,
    pub corb: bool,
    pub baseline: bool,
}

/// SURVIVALMETHOD BASELINE: how the baseline functions are estimated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsCoxBaseline {
    Efron,
    Breslow,
    ProductLimit,
}

/// SURVIVALMETHOD CI: the scale of the survival interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsCoxCi {
    Original,
    Log,
    Lml,
}

/// A SAVE keyword of CSCOXREG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsCoxSave {
    Schoenfeld,
    Martingale,
    Deviance,
    CoxSnell,
    Score,
    Dfbeta,
    AggMartingale,
    AggDeviance,
    AggCoxSnell,
    AggScore,
    AggDfbeta,
    Survival,
    LclSurvival,
    UclSurvival,
    CumHazard,
    LclCumHazard,
    UclCumHazard,
    Xbeta,
}

/// The SAVE keywords, and whether each takes a root for one variable per parameter.
const SAVES: [(&str, CsCoxSave, bool); 18] = [
    ("SCHOENFELD", CsCoxSave::Schoenfeld, true),
    ("MARTINGALE", CsCoxSave::Martingale, false),
    ("DEVIANCE", CsCoxSave::Deviance, false),
    ("COXSNELL", CsCoxSave::CoxSnell, false),
    ("SCORE", CsCoxSave::Score, true),
    ("DFBETA", CsCoxSave::Dfbeta, true),
    ("AGGMARTINGALE", CsCoxSave::AggMartingale, false),
    ("AGGDEVIANCE", CsCoxSave::AggDeviance, false),
    ("AGGCOXSNELL", CsCoxSave::AggCoxSnell, false),
    ("AGGSCORE", CsCoxSave::AggScore, true),
    ("AGGDFBETA", CsCoxSave::AggDfbeta, true),
    ("SURVIVAL", CsCoxSave::Survival, false),
    ("LCL_SURVIVAL", CsCoxSave::LclSurvival, false),
    ("UCL_SURVIVAL", CsCoxSave::UclSurvival, false),
    ("CUMHAZARD", CsCoxSave::CumHazard, false),
    ("LCL_CUMHAZARD", CsCoxSave::LclCumHazard, false),
    ("UCL_CUMHAZARD", CsCoxSave::UclCumHazard, false),
    ("XBETA", CsCoxSave::Xbeta, false),
];

/// PLOT: the functions to draw and whether with their intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CsCoxPlot {
    pub survival: bool,
    pub cumhazard: bool,
    pub lml: bool,
    pub oms: bool,
    pub ci: bool,
}

/// PATTERN: values of model variables, and a factor whose every level gets a line.
#[derive(Debug, Clone, PartialEq)]
pub struct CsCoxPattern {
    pub values: Vec<(String, Value)>,
    pub by: Option<String>,
}

/// OUTFILE of CSCOXREG.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CsCoxOutfile {
    /// COVB or CORB: the file and whether it holds correlations (CORB).
    pub matrix: Option<(String, bool)>,
    pub model: Option<String>,
    pub survival: Option<String>,
}

/// CSCOXREG.
#[derive(Debug, Clone, PartialEq)]
pub struct CsCox {
    pub start: Option<String>,
    pub end: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub status: String,
    pub events: Vec<MissingValue>,
    /// BASELINESTRATA
    pub strata: Option<String>,
    pub plan: String,
    pub model: Option<Vec<Term>>,
    pub custom: Vec<CsCustom>,
    pub criteria: CsCriteria,
    /// TIES=BRESLOW; Efron otherwise.
    pub breslow: bool,
    /// None when STATISTICS is not given.
    pub statistics: Option<CsModelStatistics>,
    pub test: CsTestType,
    pub padjust: CsPadjust,
    pub domain: Option<(String, Value)>,
    /// CLASSMISSING=INCLUDE
    pub include: bool,
    pub print: CsCoxPrint,
    /// SURVIVALMETHOD BASELINE; None: as TIES.
    pub baseline: Option<CsCoxBaseline>,
    pub ci: CsCoxCi,
    /// (keyword, name or root, at most this many variables of a root)
    pub save: Vec<(CsCoxSave, Option<String>, u32)>,
    pub plot: Option<CsCoxPlot>,
    pub patterns: Vec<CsCoxPattern>,
    pub outfile: CsCoxOutfile,
    /// JOINTPROB FILE; None for the file CSSELECT names after the plan.
    pub jointprob: Option<String>,
}

const COMMAND: &str = "CSCOXREG";

fn not_yet(p: &Parser, start: usize, what: &str) -> crate::error::SyntaxError {
    error(
        p,
        start,
        format!("{what} of {COMMAND} is not supported yet"),
    )
}

fn variables(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, p.pos, "VARIABLES needs STATUS="));
    }
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "STATUS")? {
            once(p, at, &mut seen, "STATUS")?;
            c.status = p.name("the status variable")?;
            p.expect(Punct::LParen)?;
            while let Some(v) = p.value_or_range()? {
                c.events.push(v);
                p.eat(Punct::Comma);
            }
            if c.events.is_empty() {
                return Err(p.unexpected(&["an event value"]));
            }
            p.expect(Punct::RParen)?;
        } else if eat_assignment(p, "BASELINESTRATA")? {
            once(p, at, &mut seen, "BASELINESTRATA")?;
            c.strata = Some(p.name("the baseline strata variable")?);
        } else if p.at_keyword("ID") {
            return Err(not_yet(p, at, "VARIABLES ID (several records per subject)"));
        } else {
            return Err(p.unexpected(&["STATUS =", "BASELINESTRATA =", "ID ="]));
        }
    }
    Ok(())
}

fn criteria(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let start = p.pos;
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "CRITERIA needs at least one keyword"));
    }
    let k = &mut c.criteria;
    let assign = eat_assignment;
    while !at_end(p) {
        let at = p.pos;
        if assign(p, "CILEVEL")? {
            once(p, at, &mut seen, "CILEVEL")?;
            k.cilevel = checked(p, "CILEVEL", "is at least 0 and below 100", |v| {
                (0.0..100.0).contains(&v)
            })?;
        } else if assign(p, "DF")? {
            once(p, at, &mut seen, "DF")?;
            k.df = Some(checked(p, "DF", "is above 0", |v| v > 0.0)?);
        } else if assign(p, "LCONVERGE")? {
            once(p, at, &mut seen, "LCONVERGE")?;
            k.lconverge = convergence(p, "LCONVERGE")?;
        } else if assign(p, "MXITER")? {
            once(p, at, &mut seen, "MXITER")?;
            k.mxiter = whole(p, "MXITER", 0.0)?;
        } else if assign(p, "MXSTEP")? {
            once(p, at, &mut seen, "MXSTEP")?;
            k.mxstep = whole(p, "MXSTEP", 1.0)?;
        } else if assign(p, "PCONVERGE")? {
            once(p, at, &mut seen, "PCONVERGE")?;
            k.pconverge = convergence(p, "PCONVERGE")?;
        } else if assign(p, "SINGULAR")? {
            once(p, at, &mut seen, "SINGULAR")?;
            k.singular = checked(p, "SINGULAR", "is above 0", |v| v > 0.0)?;
        } else if assign(p, "TIES")? {
            once(p, at, &mut seen, "TIES")?;
            let Some(breslow) = pick(p, &[("EFRON", false), ("BRESLOW", true)])? else {
                return Err(p.unexpected(&["EFRON", "BRESLOW"]));
            };
            c.breslow = breslow;
        } else {
            return Err(p.unexpected(&[
                "CILEVEL =",
                "DF =",
                "LCONVERGE =",
                "MXITER =",
                "MXSTEP =",
                "PCONVERGE =",
                "SINGULAR =",
                "TIES =",
            ]));
        }
    }
    Ok(())
}

fn print(p: &mut Parser) -> PResult<CsCoxPrint> {
    let start = p.pos;
    let mut out = CsCoxPrint {
        sampleinfo: false,
        eventinfo: false,
        riskinfo: false,
        history: None,
        lmatrix: false,
        covb: false,
        corb: false,
        baseline: false,
    };
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "PRINT needs at least one keyword"));
    }
    while !at_end(p) {
        let at = p.pos;
        if flag(p, &mut seen, "SAMPLEINFO")? {
            out.sampleinfo = true;
        } else if flag(p, &mut seen, "EVENTINFO")? {
            out.eventinfo = true;
        } else if flag(p, &mut seen, "RISKINFO")? {
            out.riskinfo = true;
        } else if flag(p, &mut seen, "LMATRIX")? {
            out.lmatrix = true;
        } else if flag(p, &mut seen, "COVB")? {
            out.covb = true;
        } else if flag(p, &mut seen, "CORB")? {
            out.corb = true;
        } else if flag(p, &mut seen, "NONE")? {
        } else if flag(p, &mut seen, "HISTORY")? {
            let mut every = 1;
            if p.eat(Punct::LParen) {
                every = whole(p, "HISTORY", 1.0)?;
                p.expect(Punct::RParen)?;
            }
            out.history = Some(every);
        } else if p.at_keyword("GEF") {
            return Err(not_yet(p, at, "PRINT GEF (the general estimable function)"));
        } else if flag(p, &mut seen, "BASELINE")? {
            out.baseline = true;
        } else {
            return Err(p.unexpected(&[
                "SAMPLEINFO",
                "EVENTINFO",
                "RISKINFO",
                "HISTORY",
                "GEF",
                "LMATRIX",
                "COVB",
                "CORB",
                "BASELINE",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn survival_method(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let start = p.pos;
    if at_end(p) {
        return Err(error(p, start, "SURVIVALMETHOD needs BASELINE= or CI="));
    }
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "BASELINE")? {
            once(p, at, &mut seen, "BASELINE")?;
            const METHODS: [(&str, CsCoxBaseline); 3] = [
                ("EFRON", CsCoxBaseline::Efron),
                ("BRESLOW", CsCoxBaseline::Breslow),
                ("PRODUCTLIMIT", CsCoxBaseline::ProductLimit),
            ];
            let Some(m) = pick(p, &METHODS)? else {
                return Err(p.unexpected(&["EFRON", "BRESLOW", "PRODUCTLIMIT"]));
            };
            c.baseline = Some(m);
        } else if eat_assignment(p, "CI")? {
            once(p, at, &mut seen, "CI")?;
            const SCALES: [(&str, CsCoxCi); 3] = [
                ("ORIGINAL", CsCoxCi::Original),
                ("LOG", CsCoxCi::Log),
                ("LML", CsCoxCi::Lml),
            ];
            let Some(s) = pick(p, &SCALES)? else {
                return Err(p.unexpected(&["ORIGINAL", "LOG", "LML"]));
            };
            c.ci = s;
        } else {
            return Err(p.unexpected(&["BASELINE =", "CI ="]));
        }
    }
    Ok(())
}

fn save(p: &mut Parser) -> PResult<Vec<(CsCoxSave, Option<String>, u32)>> {
    let start = p.pos;
    if at_end(p) {
        return Err(error(p, start, "SAVE needs at least one keyword"));
    }
    let table: Vec<(&str, usize)> = SAVES.iter().enumerate().map(|(k, s)| (s.0, k)).collect();
    let mut out: Vec<(CsCoxSave, Option<String>, u32)> = Vec::new();
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        let Some(k) = pick(p, &table)? else {
            let names: Vec<&str> = SAVES.iter().map(|s| s.0).collect();
            return Err(p.unexpected(&names));
        };
        let (keyword, kind, rooted) = SAVES[k];
        once(p, at, &mut seen, keyword)?;
        let (mut name, mut count) = (None, 25);
        if p.eat(Punct::LParen) {
            if !(rooted && p.at(Punct::Colon)) {
                name = Some(p.name(if rooted {
                    "a root name"
                } else {
                    "a variable name"
                })?);
            }
            if rooted && p.eat(Punct::Colon) {
                count = whole(p, "the number of variables", 1.0)?;
            }
            p.expect(Punct::RParen)?;
        }
        out.push((kind, name, count));
        p.eat(Punct::Comma);
    }
    let names: Vec<String> = out
        .iter()
        .filter_map(|(_, n, _)| n.as_ref().map(|n| n.to_uppercase()))
        .collect();
    if let Some(i) = (1..names.len()).find(|&i| names[..i].contains(&names[i])) {
        return Err(error(p, start, format!("SAVE names {} twice", names[i])));
    }
    Ok(out)
}

fn plot(p: &mut Parser) -> PResult<CsCoxPlot> {
    let start = p.pos;
    if at_end(p) {
        return Err(error(p, start, "PLOT needs at least one keyword"));
    }
    let mut out = CsCoxPlot::default();
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "CI")? {
            once(p, at, &mut seen, "CI")?;
            let Some(yes) = pick(p, &[("YES", true), ("NO", false)])? else {
                return Err(p.unexpected(&["YES", "NO"]));
            };
            out.ci = yes;
        } else if flag(p, &mut seen, "SURVIVAL")? {
            out.survival = true;
        } else if flag(p, &mut seen, "CUMHAZARD")? || flag(p, &mut seen, "HAZARD")? {
            out.cumhazard = true;
        } else if flag(p, &mut seen, "LML")? {
            out.lml = true;
        } else if flag(p, &mut seen, "OMS")? {
            out.oms = true;
        } else {
            return Err(p.unexpected(&["SURVIVAL", "CUMHAZARD", "LML", "OMS", "CI ="]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn pattern(p: &mut Parser, end: &str) -> PResult<CsCoxPattern> {
    let start = p.pos;
    let mut out = CsCoxPattern {
        values: Vec::new(),
        by: None,
    };
    while !at_end(p) && !p.at_keyword("BY") {
        let at = p.pos;
        let name = p.name("a variable name")?;
        p.expect(Punct::LParen)?;
        let mut values = Vec::new();
        while let Some(v) = p.value() {
            values.push(v);
            p.eat(Punct::Comma);
        }
        p.expect(Punct::RParen)?;
        if name.eq_ignore_ascii_case(end) || values.len() > 1 {
            return Err(not_yet(p, at, "PATTERN with a piecewise path"));
        }
        let Some(value) = values.pop() else {
            return Err(error(p, at, format!("PATTERN needs a value for {name}")));
        };
        if out
            .values
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(&name))
        {
            return Err(error(p, at, format!("PATTERN names {name} twice")));
        }
        out.values.push((name, value));
    }
    if p.eat_keyword("BY") {
        out.by = Some(p.name("a factor")?);
    }
    if out.values.is_empty() && out.by.is_none() {
        return Err(error(p, start, "PATTERN needs var(value) or BY factor"));
    }
    Ok(out)
}

fn outfile(p: &mut Parser) -> PResult<CsCoxOutfile> {
    let mut out = CsCoxOutfile::default();
    let expected = ["COVB =", "CORB =", "MODEL =", "SURVIVAL ="];
    while !at_end(p) {
        let at = p.pos;
        let corb = p.at_keyword("CORB");
        let (taken, what) = if eat_assignment(p, "COVB")? || eat_assignment(p, "CORB")? {
            (out.matrix.is_some(), "one of COVB and CORB")
        } else if eat_assignment(p, "MODEL")? {
            (out.model.is_some(), "MODEL")
        } else if eat_assignment(p, "SURVIVAL")? {
            (out.survival.is_some(), "SURVIVAL")
        } else {
            return Err(p.unexpected(&expected));
        };
        if taken {
            return Err(error(p, at, format!("OUTFILE takes {what} once")));
        }
        let file = p.file("a quoted file name")?;
        match what {
            "MODEL" => out.model = Some(file),
            "SURVIVAL" => out.survival = Some(file),
            _ => out.matrix = Some((file, corb)),
        }
    }
    if out == CsCoxOutfile::default() {
        return Err(p.unexpected(&expected));
    }
    Ok(out)
}

fn test(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let start = p.pos;
    if at_end(p) {
        return Err(error(p, start, "TEST needs TYPE= or PADJUST="));
    }
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "TYPE")? {
            once(p, at, &mut seen, "TYPE")?;
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
            once(p, at, &mut seen, "PADJUST")?;
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
    Ok(())
}

pub(crate) fn cscoxreg(p: &mut Parser) -> PResult<Command> {
    let end = p.name("the end time variable")?;
    let mut c = CsCox {
        start: None,
        end,
        factors: Vec::new(),
        covariates: Vec::new(),
        status: String::new(),
        events: Vec::new(),
        strata: None,
        plan: String::new(),
        model: None,
        custom: Vec::new(),
        criteria: CsCriteria::default(),
        breslow: false,
        statistics: None,
        test: CsTestType::F,
        padjust: CsPadjust::Lsd,
        domain: None,
        include: false,
        print: CsCoxPrint {
            sampleinfo: true,
            eventinfo: true,
            riskinfo: false,
            history: None,
            lmatrix: false,
            covb: false,
            corb: false,
            baseline: false,
        },
        baseline: None,
        ci: CsCoxCi::Log,
        save: Vec::new(),
        plot: None,
        patterns: Vec::new(),
        outfile: CsCoxOutfile::default(),
        jointprob: None,
    };
    let not_by_with = |p: &Parser| {
        matches!(p.peek(), Some(TokenKind::Word(w))
            if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    };
    if not_by_with(p) {
        c.start = Some(std::mem::replace(
            &mut c.end,
            p.name("the end time variable")?,
        ));
    }
    if not_by_with(p) {
        return Err(error(
            p,
            p.pos,
            "CSCOXREG takes one or two time variables; factors follow BY and covariates WITH",
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
        if p.eat_keyword("VARIABLES") {
            once(p, start, &mut seen, "VARIABLES")?;
            variables(p, &mut c)?;
        } else if p.eat_keyword("PLAN") {
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
        } else if p.eat_keyword("CUSTOM") {
            c.custom.push(custom(p, false)?);
        } else if p.eat_keyword("CRITERIA") {
            once(p, start, &mut seen, "CRITERIA")?;
            criteria(p, &mut c)?;
        } else if p.eat_keyword("STATISTICS") {
            once(p, start, &mut seen, "STATISTICS")?;
            c.statistics = Some(statistics(p, true)?);
        } else if p.eat_keyword("TEST") {
            once(p, start, &mut seen, "TEST")?;
            test(p, &mut c)?;
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
            c.include = match pick(p, &[("EXCLUDE", false), ("INCLUDE", true)])? {
                Some(include) => include,
                None => return Err(p.unexpected(&["EXCLUDE", "INCLUDE"])),
            };
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            c.print = print(p)?;
        } else if p.eat_keyword("SURVIVALMETHOD") {
            once(p, start, &mut seen, "SURVIVALMETHOD")?;
            survival_method(p, &mut c)?;
        } else if p.eat_keyword("SAVE") {
            once(p, start, &mut seen, "SAVE")?;
            c.save = save(p)?;
        } else if p.eat_keyword("PLOT") {
            once(p, start, &mut seen, "PLOT")?;
            c.plot = Some(plot(p)?);
        } else if p.eat_keyword("PATTERN") {
            let end = c.end.clone();
            c.patterns.push(pattern(p, &end)?);
        } else if p.eat_keyword("OUTFILE") {
            once(p, start, &mut seen, "OUTFILE")?;
            c.outfile = outfile(p)?;
        } else if p.eat_keyword("JOINTPROB") {
            once(p, start, &mut seen, "JOINTPROB")?;
            c.jointprob = Some(joint_file(p)?);
        } else {
            return Err(p.subcommand_error(COMMAND, &["TESTASSUMPTIONS"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'", "BY", "WITH"])?;
    let end = p.pos;
    if c.status.is_empty() {
        return Err(error(
            p,
            end,
            "CSCOXREG needs VARIABLES STATUS=var(values): the status variable and its event values",
        ));
    }
    if c.plan.is_empty() {
        return Err(error(p, end, "CSCOXREG needs PLAN FILE='name'"));
    }
    let intercept_entry = c
        .custom
        .iter()
        .flat_map(|k| &k.rows)
        .flatten()
        .any(|(e, _)| e.as_ref().is_some_and(|t| t.variables.is_empty()));
    if intercept_entry {
        return Err(error(
            p,
            end,
            "CSCOXREG has no intercept, so an LMATRIX row cannot start with a number; name an effect or ALL",
        ));
    }
    if c.custom.iter().any(|k| k.rows.is_empty()) {
        return Err(error(
            p,
            end,
            "CSCOXREG has no intercept, so CUSTOM needs LMATRIX=",
        ));
    }
    Ok(Command::CsCox(Box::new(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn cox(source: &str) -> CsCox {
        let (commands, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        match &commands[0].command {
            Command::CsCox(c) => (**c).clone(),
            other => panic!("{other:?}"),
        }
    }

    fn fails(source: &str) -> String {
        let (_, errors) = parse(source);
        assert!(!errors.is_empty(), "no error for {source}");
        errors[0].message.clone()
    }

    #[test]
    fn one_or_two_times_status_and_defaults() {
        let c = cox(
            "CSCOXREG months BY arm WITH age /VARIABLES STATUS=died(1 3 THRU 5) /PLAN FILE='p.json'.",
        );
        assert_eq!(c.start, None);
        assert_eq!(c.end, "months");
        assert_eq!(c.status, "died");
        assert_eq!(c.events.len(), 2);
        assert!(!c.breslow && c.print.sampleinfo && c.print.eventinfo && !c.print.riskinfo);
        assert_eq!(c.criteria, CsCriteria::default());
        let c = cox(
            "CSCOXREG entry exit /VARIABLES STATUS=s('y') BASELINESTRATA=site /PLAN FILE='p.json' \
             /CRITERIA TIES=BRESLOW MXITER=20 PCONVERGE=[1e-8 ABSOLUTE] /PRINT RISKINFO HISTORY(2) COVB \
             /TEST TYPE=ADJF PADJUST=SIDAK /MISSING CLASSMISSING=INCLUDE /DOMAIN VARIABLE=zone(2) \
             /STATISTICS PARAMETER EXP SE.",
        );
        assert_eq!(c.start.as_deref(), Some("entry"));
        assert_eq!(c.end, "exit");
        assert_eq!(c.strata.as_deref(), Some("site"));
        assert!(c.breslow && c.include);
        assert_eq!(c.criteria.mxiter, 20);
        assert_eq!(c.criteria.pconverge, (1e-8, false));
        assert_eq!(c.print.history, Some(2));
        assert!(c.print.riskinfo && c.print.covb && !c.print.sampleinfo);
        assert_eq!((c.test, c.padjust), (CsTestType::AdjF, CsPadjust::Sidak));
        assert!(c.statistics.is_some_and(|s| s.exp && s.se && !s.ttest));
        assert_eq!(c.jointprob, None);
        let c = cox(
            "CSCOXREG t WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json' /JOINTPROB FILE='j.sav'.",
        );
        assert_eq!(c.jointprob.as_deref(), Some("j.sav"));
    }

    #[test]
    fn errors_name_what_is_missing_or_not_supported() {
        let base = "CSCOXREG t WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json'";
        assert!(fails("CSCOXREG t WITH x /PLAN FILE='p.json'.").contains("STATUS"));
        assert!(fails("CSCOXREG t WITH x /VARIABLES STATUS=s(1).").contains("PLAN"));
        assert!(
            fails("CSCOXREG a b c WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json'.")
                .contains("one or two time variables")
        );
        for (sub, what) in [
            (
                "TESTASSUMPTIONS PROPHAZARD=KM",
                "subcommand TESTASSUMPTIONS",
            ),
            ("PRINT GEF", "PRINT GEF"),
            ("PATTERN t(1 2) x(1 2)", "piecewise path"),
        ] {
            let message = fails(&format!("{base} /{sub}."));
            assert!(
                message.contains(what) && message.contains("not supported yet"),
                "{message}"
            );
        }
        let message =
            fails("CSCOXREG t WITH x /VARIABLES STATUS=s(1) ID=person /PLAN FILE='p.json'.");
        assert!(message.contains("ID") && message.contains("not supported yet"));
        assert!(fails(&format!("{base} /CUSTOM LMATRIX=1 x 1.")).contains("no intercept"));
        assert!(fails(&format!("{base} /CUSTOM KMATRIX=1.")).contains("no intercept"));
        assert!(fails(&format!("{base} /CRITERIA CHKSEP=3.")).contains("CHKSEP"));
        assert!(fails(&format!("{base} /PLAN FILE='q.json'.")).contains("only once"));
        assert!(fails(&format!("{base} /SAVE XBETA(a) MARTINGALE(a).")).contains("twice"));
        assert!(fails(&format!("{base} /OUTFILE COVB='a' CORB='b'.")).contains("once"));
        assert!(fails(&format!("{base} /PATTERN.")).contains("PATTERN needs"));
    }

    #[test]
    fn survival_save_plot_pattern_and_outfile() {
        let c = cox(
            "CSCOXREG t BY g WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json' \
             /SURVIVALMETHOD BASELINE=PRODUCTLIMIT CI=LML /PRINT BASELINE \
             /SAVE SCHOENFELD(sch:2) SCORE DFBETA(:3) XBETA(xb) SURVIVAL \
             /PLOT SURVIVAL HAZARD CI=YES /PATTERN x(2.5) g(1) /PATTERN BY g \
             /OUTFILE CORB='c.sav' MODEL='m.json' SURVIVAL='s.sav'.",
        );
        assert_eq!(c.baseline, Some(CsCoxBaseline::ProductLimit));
        assert_eq!(c.ci, CsCoxCi::Lml);
        assert!(c.print.baseline && !c.print.sampleinfo);
        assert_eq!(
            c.save,
            [
                (CsCoxSave::Schoenfeld, Some("sch".into()), 2),
                (CsCoxSave::Score, None, 25),
                (CsCoxSave::Dfbeta, None, 3),
                (CsCoxSave::Xbeta, Some("xb".into()), 25),
                (CsCoxSave::Survival, None, 25),
            ]
        );
        let plot = c.plot.unwrap();
        assert!(plot.survival && plot.cumhazard && plot.ci && !plot.lml);
        assert_eq!(c.patterns.len(), 2);
        assert_eq!(c.patterns[0].values[1], ("g".into(), Value::Number(1.0)));
        assert_eq!(c.patterns[1].by.as_deref(), Some("g"));
        assert_eq!(c.outfile.matrix, Some(("c.sav".into(), true)));
        assert_eq!(c.outfile.model.as_deref(), Some("m.json"));
        assert_eq!(c.outfile.survival.as_deref(), Some("s.sav"));
    }
}
