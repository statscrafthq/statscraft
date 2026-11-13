//! Parsers of the complex samples commands: CSPLAN (analysis plans), CSDESCRIPTIVES and
//! CSTABULATE.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - CSPLAN ANALYSIS writes a plan: PLAN FILE, PLANVARS ANALYSISWEIGHT (required), an optional
//!   SRSESTIMATOR TYPE=WOR|WR and PRINT PLAN|MATRIX, then one to three stages, each a DESIGN
//!   (STAGELABEL, STRATA, CLUSTER; it may be empty) followed by ESTIMATOR TYPE=EQUAL_WOR |
//!   UNEQUAL_WOR | WR and, for EQUAL_WOR, POPSIZE or INCLPROB (one of them, not both) as
//!   VALUE=, VARIABLE= or MATRIX=vars; categories value; ... CSPLAN VIEW shows a plan.
//! - PLAN, PLANVARS, SRSESTIMATOR and PRINT are global and come at most once; the stage
//!   subcommands belong to the DESIGN before them and come at most once per stage. Every stage but
//!   the last needs CLUSTER, and so does UNEQUAL_WOR, which only the first stage may use. POPSIZE
//!   and INCLPROB with WR or UNEQUAL_WOR are ignored with a warning, as are SAMPLEWEIGHT and
//!   PREVIOUSWEIGHT on PLANVARS. A variable has one role in a plan.
//! - CSDESCRIPTIVES: PLAN FILE, SUMMARY VARIABLES, MEAN and SUM (each with an optional TTEST of
//!   one value or one per variable), any number of RATIO NUMERATOR= DENOMINATOR= [TTEST=],
//!   STATISTICS, SUBPOP TABLE=a [BY b ...] [DISPLAY=LAYERED|SEPARATE] and MISSING
//!   SCOPE=ANALYSIS|LISTWISE CLASSMISSING=EXCLUDE|INCLUDE. MEAN or SUM needs SUMMARY; one of
//!   MEAN, SUM and RATIO is required. Only RATIO may repeat.
//! - CSTABULATE: PLAN FILE, TABLES VARIABLES=list [BY var], CELLS, STATISTICS, TEST, SUBPOP and
//!   MISSING SCOPE=TABLE|LISTWISE. Each subcommand once, none empty.
//! - The equals signs of the syntax charts are required. Keywords may be shortened as elsewhere.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsEstimator {
    EqualWor,
    UnequalWor,
    Wr,
}

impl CsEstimator {
    pub fn keyword(self) -> &'static str {
        match self {
            CsEstimator::EqualWor => "EQUAL_WOR",
            CsEstimator::UnequalWor => "UNEQUAL_WOR",
            CsEstimator::Wr => "WR",
        }
    }
}

/// A population size or an inclusion probability of a stage.
#[derive(Debug, Clone, PartialEq)]
pub enum CsRate {
    Value(f64),
    Variable(String),
    /// One value per combination of categories of the variables; the last row for a
    /// combination counts.
    Matrix {
        variables: Vec<String>,
        rows: Vec<(Vec<Value>, f64)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsStage {
    pub label: Option<String>,
    pub strata: Vec<String>,
    pub clusters: Vec<String>,
    pub estimator: CsEstimator,
    pub popsize: Option<CsRate>,
    pub inclprob: Option<CsRate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsplanMode {
    Analysis,
    View,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Csplan {
    pub mode: CsplanMode,
    pub file: String,
    /// ANALYSISWEIGHT; None only for VIEW.
    pub weight: Option<String>,
    /// SRSESTIMATOR TYPE=WR: design effects compare with sampling with replacement.
    pub srs_wr: bool,
    pub print_plan: bool,
    pub print_matrix: bool,
    pub stages: Vec<CsStage>,
    /// Specifications that are ignored, for the engine to warn about.
    pub ignored: Vec<String>,
}

/// The statistics of CSDESCRIPTIVES and CSTABULATE.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CsStatistics {
    pub se: bool,
    pub cv: bool,
    pub deff: bool,
    pub deffsqrt: bool,
    /// The confidence level in percent.
    pub cin: Option<f64>,
    pub count: bool,
    pub popsize: bool,
    pub cumulative: bool,
    pub expected: bool,
    pub resid: bool,
    pub asresid: bool,
}

impl CsStatistics {
    fn standard_error() -> Self {
        Self {
            se: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsSubpop {
    pub variables: Vec<String>,
    pub separate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsRatio {
    pub numerators: Vec<VarItem>,
    pub denominators: Vec<VarItem>,
    pub test: Option<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Csdescriptives {
    pub plan: String,
    pub summary: Vec<VarItem>,
    /// MEAN given, with its TTEST values (an empty list without TTEST).
    pub mean: Option<Vec<f64>>,
    pub sum: Option<Vec<f64>>,
    pub ratios: Vec<CsRatio>,
    pub statistics: CsStatistics,
    pub subpop: Option<CsSubpop>,
    pub listwise: bool,
    /// CLASSMISSING=INCLUDE
    pub include: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsCells {
    pub popsize: bool,
    pub rowpct: bool,
    pub colpct: bool,
    pub tablepct: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CsTests {
    pub homogeneity: bool,
    pub independence: bool,
    pub oddsratio: bool,
    pub relrisk: bool,
    pub riskdiff: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cstabulate {
    pub plan: String,
    pub variables: Vec<VarItem>,
    pub by: Option<String>,
    pub cells: CsCells,
    pub statistics: CsStatistics,
    pub tests: CsTests,
    pub subpop: Option<CsSubpop>,
    pub listwise: bool,
    pub include: bool,
}

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message)
}

/// `KEYWORD =`: the keyword, then the required equals sign.
fn eat_assignment(p: &mut Parser, keyword: &str) -> PResult<bool> {
    if !p.at_keyword(keyword) || p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq)) {
        return Ok(false);
    }
    p.pos += 2;
    Ok(true)
}

fn require_assignment(p: &mut Parser, keyword: &str) -> PResult<()> {
    if eat_assignment(p, keyword)? {
        return Ok(());
    }
    if p.eat_keyword(keyword) {
        return Err(p.unexpected(&["'='"]));
    }
    Err(p.unexpected(&[keyword]))
}

/// Variable names up to the next `KEYWORD =`, BY or the end of the subcommand.
fn names(p: &mut Parser) -> PResult<Vec<String>> {
    let mut out = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) || w.eq_ignore_ascii_case("BY") {
            break;
        }
        out.push(p.name("a variable name")?);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(out)
}

/// `PLAN FILE = file`, after the subcommand name.
fn plan_file(p: &mut Parser) -> PResult<String> {
    require_assignment(p, "FILE")?;
    p.file("a plan file name")
}

/// Marks a subcommand as seen; a second one is an error.
fn once(p: &Parser, start: usize, seen: &mut Vec<&'static str>, name: &'static str) -> PResult<()> {
    if seen.contains(&name) {
        return Err(error(p, start, format!("{name} may be given only once")));
    }
    seen.push(name);
    Ok(())
}

fn rate(p: &mut Parser, what: &str) -> PResult<CsRate> {
    let start = p.pos;
    let check = |p: &Parser, v: f64| -> PResult<f64> {
        let ok = if what == "POPSIZE" {
            v >= 1.0 && v.fract() == 0.0
        } else {
            v > 0.0 && v <= 1.0
        };
        if ok {
            Ok(v)
        } else if what == "POPSIZE" {
            Err(error(
                p,
                start,
                format!("a population size is a positive whole number, got {v}"),
            ))
        } else {
            Err(error(
                p,
                start,
                format!("an inclusion probability is above 0 and at most 1, got {v}"),
            ))
        }
    };
    if eat_assignment(p, "VALUE")? {
        let Some(v) = p.number() else {
            return Err(p.unexpected(&["a number"]));
        };
        return Ok(CsRate::Value(check(p, v)?));
    }
    if eat_assignment(p, "VARIABLE")? {
        return Ok(CsRate::Variable(p.name("a variable name")?));
    }
    if eat_assignment(p, "MATRIX")? {
        let mut variables = Vec::new();
        while !p.at(Punct::Semicolon) {
            variables.push(p.name("a variable name")?);
        }
        let mut rows = Vec::new();
        while p.eat(Punct::Semicolon) {
            let row_start = p.pos;
            let mut values = Vec::new();
            while let Some(v) = p.value() {
                values.push(v);
            }
            let Some(Value::Number(size)) = values.pop() else {
                return Err(p.unexpected(&["a category value", "a number"]));
            };
            if values.len() != variables.len() {
                return Err(error(
                    p,
                    row_start,
                    format!(
                        "each {what} MATRIX row gives {} categories and then the value",
                        variables.len()
                    ),
                ));
            }
            rows.push((values, check(p, size)?));
        }
        if rows.is_empty() {
            return Err(p.unexpected(&["';'"]));
        }
        return Ok(CsRate::Matrix { variables, rows });
    }
    Err(p.unexpected(&["VALUE =", "VARIABLE =", "MATRIX ="]))
}

fn design(p: &mut Parser) -> PResult<CsStage> {
    let mut stage = CsStage {
        label: None,
        strata: Vec::new(),
        clusters: Vec::new(),
        estimator: CsEstimator::Wr,
        popsize: None,
        inclprob: None,
    };
    let mut seen = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        if eat_assignment(p, "STAGELABEL")? {
            once(p, start, &mut seen, "STAGELABEL")?;
            stage.label = Some(p.string("a quoted label")?);
        } else if eat_assignment(p, "STRATA")? {
            once(p, start, &mut seen, "STRATA")?;
            stage.strata = names(p)?;
        } else if eat_assignment(p, "CLUSTER")? {
            once(p, start, &mut seen, "CLUSTER")?;
            stage.clusters = names(p)?;
        } else {
            return Err(p.unexpected(&["STAGELABEL =", "STRATA =", "CLUSTER ="]));
        }
    }
    Ok(stage)
}

/// Checks a finished plan: estimators, clusters, roles and matrix variables.
fn check_plan(p: &Parser, plan: &mut Csplan, estimators: &[bool]) -> PResult<()> {
    let end = p.pos;
    if plan.stages.is_empty() {
        return Err(error(p, end, "CSPLAN ANALYSIS needs a DESIGN subcommand"));
    }
    let Some(weight) = &plan.weight else {
        return Err(error(
            p,
            end,
            "CSPLAN ANALYSIS needs PLANVARS ANALYSISWEIGHT=variable",
        ));
    };
    let mut roles: Vec<String> = vec![weight.to_uppercase()];
    // the strata and clusters of the earlier stages, which a MATRIX may name
    let mut earlier: Vec<String> = Vec::new();
    let last = plan.stages.len() - 1;
    for (s, stage) in plan.stages.iter_mut().enumerate() {
        let number = s + 1;
        if !estimators[s] {
            return Err(error(
                p,
                end,
                format!("stage {number} needs an ESTIMATOR subcommand"),
            ));
        }
        if s < last && stage.clusters.is_empty() {
            return Err(error(
                p,
                end,
                format!("stage {number} is followed by another stage, so it needs CLUSTER"),
            ));
        }
        match stage.estimator {
            CsEstimator::UnequalWor if s > 0 => {
                return Err(error(
                    p,
                    end,
                    "UNEQUAL_WOR is allowed only in the first stage",
                ));
            }
            CsEstimator::UnequalWor if stage.clusters.is_empty() => {
                return Err(error(p, end, "UNEQUAL_WOR needs CLUSTER"));
            }
            CsEstimator::EqualWor if stage.popsize.is_none() && stage.inclprob.is_none() => {
                return Err(error(
                    p,
                    end,
                    format!("stage {number} uses EQUAL_WOR, which needs POPSIZE or INCLPROB"),
                ));
            }
            CsEstimator::Wr | CsEstimator::UnequalWor
                if stage.popsize.is_some() || stage.inclprob.is_some() =>
            {
                plan.ignored.push(format!(
                    "POPSIZE and INCLPROB are ignored in stage {number}, whose estimator is {}",
                    stage.estimator.keyword()
                ));
                stage.popsize = None;
                stage.inclprob = None;
            }
            _ => {}
        }
        // a matrix may use the strata of this and earlier stages and the clusters of earlier ones
        let mut allowed: Vec<String> = stage.strata.iter().map(|v| v.to_uppercase()).collect();
        allowed.extend(earlier.iter().cloned());
        for name in stage.strata.iter().chain(&stage.clusters) {
            let upper = name.to_uppercase();
            if roles.contains(&upper) {
                return Err(error(
                    p,
                    end,
                    format!("{name} has more than one role in the plan"),
                ));
            }
            roles.push(upper);
        }
        earlier.extend(
            stage
                .strata
                .iter()
                .chain(&stage.clusters)
                .map(|v| v.to_uppercase()),
        );
        for rate in stage.popsize.iter().chain(&stage.inclprob) {
            match rate {
                CsRate::Matrix { variables, .. } => {
                    if let Some(v) = variables
                        .iter()
                        .find(|v| !allowed.contains(&v.to_uppercase()))
                    {
                        return Err(error(
                            p,
                            end,
                            format!(
                                "the MATRIX of stage {number} names {v}, which is not a stratum variable of this or an earlier stage or a cluster variable of an earlier stage"
                            ),
                        ));
                    }
                }
                CsRate::Variable(v) => {
                    let upper = v.to_uppercase();
                    if roles.contains(&upper) {
                        return Err(error(
                            p,
                            end,
                            format!("{v} has more than one role in the plan"),
                        ));
                    }
                    roles.push(upper);
                }
                CsRate::Value(_) => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn csplan(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mode = if p.eat_keyword("ANALYSIS") {
        CsplanMode::Analysis
    } else if p.eat_keyword("VIEW") {
        CsplanMode::View
    } else if p.eat_keyword("SAMPLE") {
        return Err(error(
            p,
            start,
            "CSPLAN SAMPLE (sample designs for drawing samples) is not supported yet; CSPLAN ANALYSIS and VIEW are",
        ));
    } else {
        return Err(p.unexpected(&["ANALYSIS", "VIEW", "SAMPLE"]));
    };
    let mut plan = Csplan {
        mode,
        file: String::new(),
        weight: None,
        srs_wr: false,
        print_plan: true,
        print_matrix: false,
        stages: Vec::new(),
        ignored: Vec::new(),
    };
    let mut seen: Vec<&'static str> = Vec::new();
    let mut stage_seen: Vec<&'static str> = Vec::new();
    // whether each stage got its ESTIMATOR
    let mut estimators: Vec<bool> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let stage_only = |p: &Parser, plan: &Csplan, name: &str| -> PResult<()> {
            if plan.mode == CsplanMode::View {
                return Err(error(
                    p,
                    start,
                    format!("CSPLAN VIEW takes only PLAN and PRINT, not {name}"),
                ));
            }
            if plan.stages.is_empty() {
                return Err(error(
                    p,
                    start,
                    format!("{name} belongs to a stage: give DESIGN first"),
                ));
            }
            Ok(())
        };
        if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            plan.file = plan_file(p)?;
        } else if p.eat_keyword("PLANVARS") {
            once(p, start, &mut seen, "PLANVARS")?;
            if mode == CsplanMode::View {
                return Err(error(p, start, "CSPLAN VIEW takes only PLAN and PRINT"));
            }
            while !at_end(p) {
                if eat_assignment(p, "ANALYSISWEIGHT")? {
                    plan.weight = Some(p.name("the weight variable")?);
                } else if eat_assignment(p, "SAMPLEWEIGHT")? || eat_assignment(p, "PREVIOUSWEIGHT")?
                {
                    p.name("a variable name")?;
                    plan.ignored.push(
                        "SAMPLEWEIGHT and PREVIOUSWEIGHT belong to sample designs and are ignored"
                            .into(),
                    );
                } else {
                    return Err(p.unexpected(&["ANALYSISWEIGHT ="]));
                }
            }
        } else if p.eat_keyword("SRSESTIMATOR") {
            once(p, start, &mut seen, "SRSESTIMATOR")?;
            if mode == CsplanMode::View {
                return Err(error(p, start, "CSPLAN VIEW takes only PLAN and PRINT"));
            }
            require_assignment(p, "TYPE")?;
            if p.eat_keyword("WOR") {
                plan.srs_wr = false;
            } else if p.eat_keyword("WR") {
                plan.srs_wr = true;
            } else {
                return Err(p.unexpected(&["WOR", "WR"]));
            }
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            plan.print_plan = false;
            if at_end(p) {
                return Err(error(p, start, "PRINT needs PLAN or MATRIX"));
            }
            while !at_end(p) {
                if p.eat_keyword("PLAN") {
                    plan.print_plan = true;
                } else if p.eat_keyword("MATRIX") {
                    plan.print_matrix = true;
                } else {
                    return Err(p.unexpected(&["PLAN", "MATRIX"]));
                }
            }
        } else if p.eat_keyword("DESIGN") {
            if mode == CsplanMode::View {
                return Err(error(
                    p,
                    start,
                    "CSPLAN VIEW takes only PLAN and PRINT, not DESIGN",
                ));
            }
            if plan.stages.len() == 3 {
                return Err(error(
                    p,
                    start,
                    "a plan has at most three stages (DESIGN subcommands)",
                ));
            }
            plan.stages.push(design(p)?);
            estimators.push(false);
            stage_seen.clear();
        } else if p.eat_keyword("ESTIMATOR") {
            stage_only(p, &plan, "ESTIMATOR")?;
            once(p, start, &mut stage_seen, "ESTIMATOR")?;
            require_assignment(p, "TYPE")?;
            let estimator = if p.eat_keyword("EQUAL_WOR") {
                CsEstimator::EqualWor
            } else if p.eat_keyword("UNEQUAL_WOR") {
                CsEstimator::UnequalWor
            } else if p.eat_word("WR") {
                CsEstimator::Wr
            } else {
                return Err(p.unexpected(&["EQUAL_WOR", "UNEQUAL_WOR", "WR"]));
            };
            plan.stages.last_mut().expect("checked").estimator = estimator;
            *estimators.last_mut().expect("checked") = true;
        } else if let Some(name) = ["POPSIZE", "INCLPROB"]
            .into_iter()
            .find(|k| p.at_keyword(k))
        {
            p.pos += 1;
            stage_only(p, &plan, name)?;
            once(p, start, &mut stage_seen, name)?;
            let value = rate(p, name)?;
            let stage = plan.stages.last_mut().expect("checked");
            if name == "POPSIZE" {
                stage.popsize = Some(value);
            } else {
                stage.inclprob = Some(value);
            }
            if stage.popsize.is_some() && stage.inclprob.is_some() {
                return Err(error(
                    p,
                    start,
                    "POPSIZE and INCLPROB cannot both be given for one stage",
                ));
            }
        } else if let Some(name) = ["METHOD", "MOS", "SIZE", "RATE", "STAGEVARS"]
            .into_iter()
            .find(|k| p.at_keyword(k))
        {
            p.pos += 1;
            return Err(error(
                p,
                start,
                format!(
                    "{name} belongs to sample designs (CSPLAN SAMPLE), which are not supported yet"
                ),
            ));
        } else {
            return Err(p.subcommand_error("CSPLAN", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if plan.file.is_empty() {
        return Err(error(p, p.pos, "CSPLAN needs PLAN FILE='name'"));
    }
    if mode == CsplanMode::Analysis {
        check_plan(p, &mut plan, &estimators)?;
    }
    Ok(Command::Csplan(Box::new(plan)))
}

/// `TTEST = value [value ...]`, values separated by spaces or commas.
fn ttest(p: &mut Parser) -> PResult<Vec<f64>> {
    let mut values = Vec::new();
    while let Some(v) = p.number() {
        values.push(v);
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a number"]));
    }
    Ok(values)
}

/// The optional `TTEST = values` of MEAN and SUM.
fn mean_or_sum(p: &mut Parser) -> PResult<Vec<f64>> {
    if at_end(p) {
        return Ok(Vec::new());
    }
    require_assignment(p, "TTEST")?;
    ttest(p)
}

fn statistics(p: &mut Parser, tabulate: bool) -> PResult<CsStatistics> {
    let start = p.pos;
    let mut s = CsStatistics::default();
    if at_end(p) {
        return Err(error(p, start, "STATISTICS needs at least one keyword"));
    }
    while !at_end(p) {
        if p.eat_word("SE") {
            s.se = true;
        } else if p.eat_word("CV") {
            s.cv = true;
        } else if p.eat_word("DEFF") {
            // before DEFFSQRT, which DEFF would abbreviate
            s.deff = true;
        } else if p.at_keyword("DEFFSQRT") && !p.at_keyword("DEFF") {
            p.pos += 1;
            s.deffsqrt = true;
        } else if p.eat_keyword("COUNT") {
            s.count = true;
        } else if p.eat_keyword("CIN") {
            let mut level = 95.0;
            if p.eat(Punct::LParen) {
                let at = p.pos;
                let Some(v) = p.number() else {
                    return Err(p.unexpected(&["a number"]));
                };
                if !(0.0..100.0).contains(&v) {
                    return Err(error(
                        p,
                        at,
                        format!("CIN is at least 0 and below 100, got {v}"),
                    ));
                }
                level = v;
                p.expect(Punct::RParen)?;
            }
            s.cin = Some(level);
        } else if !tabulate && p.eat_keyword("POPSIZE") {
            s.popsize = true;
        } else if tabulate && p.eat_keyword("CUMULATIVE") {
            s.cumulative = true;
        } else if tabulate && p.eat_keyword("EXPECTED") {
            s.expected = true;
        } else if tabulate && p.eat_keyword("RESID") {
            s.resid = true;
        } else if tabulate && p.eat_keyword("ASRESID") {
            s.asresid = true;
        } else if tabulate {
            return Err(p.unexpected(&[
                "SE",
                "CV",
                "DEFF",
                "DEFFSQRT",
                "CIN",
                "COUNT",
                "CUMULATIVE",
                "EXPECTED",
                "RESID",
                "ASRESID",
            ]));
        } else {
            return Err(p.unexpected(&["COUNT", "POPSIZE", "SE", "CV", "DEFF", "DEFFSQRT", "CIN"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(s)
}

fn subpop(p: &mut Parser, limit: usize) -> PResult<CsSubpop> {
    let start = p.pos;
    require_assignment(p, "TABLE")?;
    let mut variables = vec![p.name("a variable name")?];
    while p.eat_word("BY") {
        variables.push(p.name("a variable name")?);
    }
    for (i, v) in variables.iter().enumerate() {
        if variables[..i].iter().any(|u| u.eq_ignore_ascii_case(v)) {
            return Err(error(p, start, format!("{v} appears twice on SUBPOP")));
        }
    }
    if variables.len() > limit {
        return Err(error(
            p,
            start,
            format!("SUBPOP takes at most {limit} variables"),
        ));
    }
    let mut separate = false;
    if eat_assignment(p, "DISPLAY")? {
        if p.eat_keyword("SEPARATE") {
            separate = true;
        } else if !p.eat_keyword("LAYERED") {
            return Err(p.unexpected(&["LAYERED", "SEPARATE"]));
        }
    }
    Ok(CsSubpop {
        variables,
        separate,
    })
}

/// `MISSING SCOPE=... CLASSMISSING=...`: (listwise, include).
fn missing(p: &mut Parser, scope: &'static str) -> PResult<(bool, bool)> {
    let start = p.pos;
    let (mut listwise, mut include) = (false, false);
    if at_end(p) {
        return Err(error(p, start, "MISSING needs SCOPE or CLASSMISSING"));
    }
    while !at_end(p) {
        if eat_assignment(p, "SCOPE")? {
            if p.eat_keyword("LISTWISE") {
                listwise = true;
            } else if p.eat_keyword(scope) {
                listwise = false;
            } else {
                return Err(p.unexpected(&[scope, "LISTWISE"]));
            }
        } else if eat_assignment(p, "CLASSMISSING")? {
            if p.eat_keyword("INCLUDE") {
                include = true;
            } else if p.eat_keyword("EXCLUDE") {
                include = false;
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else {
            return Err(p.unexpected(&["SCOPE =", "CLASSMISSING ="]));
        }
    }
    Ok((listwise, include))
}

pub(crate) fn csdescriptives(p: &mut Parser) -> PResult<Command> {
    let mut c = Csdescriptives {
        plan: String::new(),
        summary: Vec::new(),
        mean: None,
        sum: None,
        ratios: Vec::new(),
        statistics: CsStatistics::standard_error(),
        subpop: None,
        listwise: false,
        include: false,
    };
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            c.plan = plan_file(p)?;
        } else if p.eat_word("SUM") {
            // before SUMMARY, which SUM would abbreviate
            once(p, start, &mut seen, "SUM")?;
            c.sum = Some(mean_or_sum(p)?);
        } else if p.eat_keyword("SUMMARY") {
            once(p, start, &mut seen, "SUMMARY")?;
            require_assignment(p, "VARIABLES")?;
            c.summary = p.var_list()?;
        } else if p.eat_keyword("MEAN") {
            once(p, start, &mut seen, "MEAN")?;
            c.mean = Some(mean_or_sum(p)?);
        } else if p.eat_keyword("RATIO") {
            let mut ratio = CsRatio {
                numerators: Vec::new(),
                denominators: Vec::new(),
                test: None,
            };
            while !at_end(p) {
                if eat_assignment(p, "NUMERATOR")? {
                    ratio.numerators = list_until(p, &["DENOMINATOR", "TTEST"])?;
                } else if eat_assignment(p, "DENOMINATOR")? {
                    ratio.denominators = list_until(p, &["NUMERATOR", "TTEST"])?;
                } else if eat_assignment(p, "TTEST")? {
                    ratio.test = Some(ttest(p)?);
                } else {
                    return Err(p.unexpected(&["NUMERATOR =", "DENOMINATOR =", "TTEST ="]));
                }
            }
            if ratio.numerators.is_empty() || ratio.denominators.is_empty() {
                return Err(error(p, start, "RATIO needs NUMERATOR= and DENOMINATOR="));
            }
            c.ratios.push(ratio);
        } else if p.eat_keyword("STATISTICS") {
            once(p, start, &mut seen, "STATISTICS")?;
            c.statistics = statistics(p, false)?;
        } else if p.eat_keyword("SUBPOP") {
            once(p, start, &mut seen, "SUBPOP")?;
            c.subpop = Some(subpop(p, 17)?);
        } else if p.eat_keyword("MISSING") {
            once(p, start, &mut seen, "MISSING")?;
            (c.listwise, c.include) = missing(p, "ANALYSIS")?;
        } else {
            return Err(p.subcommand_error("CSDESCRIPTIVES", &["JOINTPROB"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let end = p.pos;
    if c.plan.is_empty() {
        return Err(error(p, end, "CSDESCRIPTIVES needs PLAN FILE='name'"));
    }
    if c.mean.is_none() && c.sum.is_none() && c.ratios.is_empty() {
        return Err(error(p, end, "CSDESCRIPTIVES needs MEAN, SUM or RATIO"));
    }
    if (c.mean.is_some() || c.sum.is_some()) && c.summary.is_empty() {
        return Err(error(p, end, "MEAN and SUM need SUMMARY VARIABLES=list"));
    }
    Ok(Command::Csdescriptives(Box::new(c)))
}

pub(crate) fn cstabulate(p: &mut Parser) -> PResult<Command> {
    let mut c = Cstabulate {
        plan: String::new(),
        variables: Vec::new(),
        by: None,
        cells: CsCells {
            popsize: true,
            rowpct: false,
            colpct: false,
            tablepct: false,
        },
        statistics: CsStatistics::standard_error(),
        tests: CsTests::default(),
        subpop: None,
        listwise: false,
        include: false,
    };
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            c.plan = plan_file(p)?;
        } else if p.eat_keyword("TABLES") {
            once(p, start, &mut seen, "TABLES")?;
            require_assignment(p, "VARIABLES")?;
            c.variables = list_until(p, &["BY"])?;
            if p.eat_word("BY") {
                c.by = Some(p.name("a variable name")?);
            }
        } else if p.eat_keyword("CELLS") {
            once(p, start, &mut seen, "CELLS")?;
            if at_end(p) {
                return Err(error(p, start, "CELLS needs at least one keyword"));
            }
            c.cells = CsCells {
                popsize: false,
                rowpct: false,
                colpct: false,
                tablepct: false,
            };
            while !at_end(p) {
                if p.eat_keyword("POPSIZE") {
                    c.cells.popsize = true;
                } else if p.eat_keyword("ROWPCT") {
                    c.cells.rowpct = true;
                } else if p.eat_keyword("COLPCT") {
                    c.cells.colpct = true;
                } else if p.eat_keyword("TABLEPCT") {
                    c.cells.tablepct = true;
                } else {
                    return Err(p.unexpected(&["POPSIZE", "ROWPCT", "COLPCT", "TABLEPCT"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("STATISTICS") {
            once(p, start, &mut seen, "STATISTICS")?;
            c.statistics = statistics(p, true)?;
        } else if p.eat_keyword("TEST") {
            once(p, start, &mut seen, "TEST")?;
            if at_end(p) {
                return Err(error(p, start, "TEST needs at least one keyword"));
            }
            while !at_end(p) {
                if p.eat_keyword("HOMOGENEITY") {
                    c.tests.homogeneity = true;
                } else if p.eat_keyword("INDEPENDENCE") {
                    c.tests.independence = true;
                } else if p.eat_keyword("ODDSRATIO") {
                    c.tests.oddsratio = true;
                } else if p.eat_keyword("RELRISK") {
                    c.tests.relrisk = true;
                } else if p.eat_keyword("RISKDIFF") {
                    c.tests.riskdiff = true;
                } else {
                    return Err(p.unexpected(&[
                        "HOMOGENEITY",
                        "INDEPENDENCE",
                        "ODDSRATIO",
                        "RELRISK",
                        "RISKDIFF",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("SUBPOP") {
            once(p, start, &mut seen, "SUBPOP")?;
            c.subpop = Some(subpop(p, 16)?);
        } else if p.eat_keyword("MISSING") {
            once(p, start, &mut seen, "MISSING")?;
            (c.listwise, c.include) = missing(p, "TABLE")?;
        } else {
            return Err(p.subcommand_error("CSTABULATE", &["JOINTPROB"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let end = p.pos;
    if c.plan.is_empty() {
        return Err(error(p, end, "CSTABULATE needs PLAN FILE='name'"));
    }
    if c.variables.is_empty() {
        return Err(error(p, end, "CSTABULATE needs TABLES VARIABLES=list"));
    }
    Ok(Command::Cstabulate(Box::new(c)))
}
