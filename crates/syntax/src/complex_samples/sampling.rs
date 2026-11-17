//! Parsers of the sampling side of the complex samples commands: the stage subcommands of
//! CSPLAN SAMPLE (METHOD, MOS, SIZE, RATE, STAGEVARS) and CSSELECT.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - A sample plan names its file (PLAN), may name the final weight to create (PLANVARS
//!   SAMPLEWEIGHT, default SampleWeight_Final_) and a weight from an earlier selection to
//!   multiply in (PREVIOUSWEIGHT). Each DESIGN block needs METHOD and one of SIZE or RATE.
//! - Methods with equal chances: SIMPLE_WOR (analyzed as EQUAL_WOR), SIMPLE_WR, SIMPLE_SYSTEMATIC
//!   and SIMPLE_CHROMY (all three analyzed as WR). Methods proportional to size, first stage
//!   only: PPS_WOR, PPS_BREWER, PPS_MURTHY and PPS_SAMPFORD (analyzed as UNEQUAL_WOR, so they
//!   need CLUSTER), PPS_WR, PPS_SYSTEMATIC and PPS_CHROMY (analyzed as WR). Drawing with
//!   replacement is for the last stage only. ESTIMATION=WR, first stage only, makes the analysis
//!   use WR. Method names are spelled in full.
//! - SIZE is a count per stratum (positive whole numbers), RATE a fraction (above 0, at most 1
//!   without replacement) with MINSIZE and MAXSIZE bounds; either one as VALUE=, VARIABLE= or
//!   MATRIX= like POPSIZE of an analysis plan. Brewer and Murthy always draw two units, so SIZE
//!   and RATE are ignored with a warning there.
//! - MOS (first stage only) is VARIABLE=name or SOURCE=FROMDATA (the cases in each cluster), with
//!   optional MIN and MAX bounds above 0. A PPS method needs it; with an equal-chance method it is
//!   ignored with a warning.
//! - STAGEVARS names the stage's output variables: INCLPROB and CUMWEIGHT always exist, INDEX
//!   exists when the stage draws with replacement (asked for otherwise it is ignored), POPSIZE,
//!   SAMPSIZE, RATE and WEIGHT on request. A keyword without a name takes the default root and
//!   the stage number, such as InclusionProbability_1_.
//! - CSSELECT: PLAN FILE (required), CRITERIA STAGES=consecutive stages SEED=RANDOM|n,
//!   CLASSMISSING, DATA RENAMEVARS PRESORTED, SAMPLEFILE OUTFILE= KEEP= DROP=, JOINTPROB
//!   OUTFILE=, SELECTRULE OUTFILE= (not supported yet) and PRINT SELECTION CPS. Each subcommand
//!   once, none empty, a keyword once within its subcommand; two subcommands cannot write the
//!   same file.

use super::{
    CsEstimator, CsRate, CsStage, Csplan, eat_assignment, error, once, plan_file,
    require_assignment,
};
use crate::ast::{Command, VarItem};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// How a stage of a sample plan draws its units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsMethod {
    SimpleWor,
    SimpleWr,
    SimpleSystematic,
    SimpleChromy,
    PpsWor,
    PpsWr,
    PpsSystematic,
    PpsBrewer,
    PpsMurthy,
    PpsSampford,
    PpsChromy,
}

const METHODS: [(&str, CsMethod); 11] = [
    ("SIMPLE_WOR", CsMethod::SimpleWor),
    ("SIMPLE_WR", CsMethod::SimpleWr),
    ("SIMPLE_SYSTEMATIC", CsMethod::SimpleSystematic),
    ("SIMPLE_CHROMY", CsMethod::SimpleChromy),
    ("PPS_WOR", CsMethod::PpsWor),
    ("PPS_WR", CsMethod::PpsWr),
    ("PPS_SYSTEMATIC", CsMethod::PpsSystematic),
    ("PPS_BREWER", CsMethod::PpsBrewer),
    ("PPS_MURTHY", CsMethod::PpsMurthy),
    ("PPS_SAMPFORD", CsMethod::PpsSampford),
    ("PPS_CHROMY", CsMethod::PpsChromy),
];

/// The measure of size of a PPS stage: a variable, or None for the number of cases in each
/// cluster (SOURCE=FROMDATA); MIN and MAX bound it.
#[derive(Debug, Clone, PartialEq)]
pub struct CsMos {
    pub variable: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl CsMethod {
    pub fn keyword(self) -> &'static str {
        METHODS.iter().find(|(_, m)| *m == self).expect("listed").0
    }

    pub fn from_keyword(keyword: &str) -> Option<CsMethod> {
        METHODS
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(keyword))
            .map(|(_, m)| *m)
    }

    pub fn with_replacement(self) -> bool {
        matches!(self, CsMethod::SimpleWr | CsMethod::PpsWr)
    }

    /// Drawn with probability proportional to a measure of size.
    pub fn pps(self) -> bool {
        self.keyword().starts_with("PPS")
    }

    /// Brewer and Murthy always draw two units per stratum.
    pub fn draws_two(self) -> bool {
        matches!(self, CsMethod::PpsBrewer | CsMethod::PpsMurthy)
    }

    /// The estimator an analysis of the drawn sample uses by default. UNEQUAL_WOR also means
    /// that the stage needs CLUSTER and that CSSELECT writes joint inclusion probabilities.
    pub fn estimator(self) -> CsEstimator {
        match self {
            CsMethod::SimpleWor => CsEstimator::EqualWor,
            CsMethod::PpsWor
            | CsMethod::PpsBrewer
            | CsMethod::PpsMurthy
            | CsMethod::PpsSampford => CsEstimator::UnequalWor,
            _ => CsEstimator::Wr,
        }
    }
}

/// The names of the variables CSSELECT writes for a stage; None is not written.
#[derive(Debug, Clone, PartialEq)]
pub struct CsStageVars {
    pub inclprob: String,
    pub cumweight: String,
    pub index: Option<String>,
    pub popsize: Option<String>,
    pub sampsize: Option<String>,
    pub rate: Option<String>,
    pub weight: Option<String>,
}

impl CsStageVars {
    /// Every name, the always-present ones first.
    pub fn names(&self) -> Vec<&String> {
        let mut out = vec![&self.inclprob, &self.cumweight];
        out.extend(
            [
                &self.index,
                &self.popsize,
                &self.sampsize,
                &self.rate,
                &self.weight,
            ]
            .into_iter()
            .flatten(),
        );
        out
    }
}

/// The sampling part of a stage of a sample plan.
#[derive(Debug, Clone, PartialEq)]
pub struct CsSampling {
    pub method: CsMethod,
    /// ESTIMATION=WR
    pub estimation_wr: bool,
    pub size: Option<CsRate>,
    pub rate: Option<CsRate>,
    pub minsize: Option<f64>,
    pub maxsize: Option<f64>,
    pub mos: Option<CsMos>,
    pub vars: CsStageVars,
}

fn default_name(root: &str, stage: usize) -> String {
    format!("{root}_{stage}_")
}

/// The sampling part of stage `stage` (from 1) before its subcommands.
pub(super) fn new_sampling(stage: usize) -> CsSampling {
    CsSampling {
        method: CsMethod::SimpleWor,
        estimation_wr: false,
        size: None,
        rate: None,
        minsize: None,
        maxsize: None,
        mos: None,
        vars: CsStageVars {
            inclprob: default_name("InclusionProbability", stage),
            cumweight: default_name("SampleWeightCumulative", stage),
            index: None,
            popsize: None,
            sampsize: None,
            rate: None,
            weight: None,
        },
    }
}

/// A whole positive number after `KEYWORD =`.
fn count(p: &mut Parser, what: &str) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(v) if v >= 1.0 && v.fract() == 0.0 => Ok(v),
        Some(v) => Err(error(
            p,
            start,
            format!("{what} is a positive whole number, got {v}"),
        )),
        None => Err(p.unexpected(&["a number"])),
    }
}

/// `(name)` after a STAGEVARS keyword, when given.
fn paren_name(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

/// A stage subcommand of CSPLAN SAMPLE, its name already read. `got_method` is set by METHOD.
pub(super) fn stage_subcommand(
    p: &mut Parser,
    start: usize,
    name: &'static str,
    stage: &mut CsStage,
    number: usize,
    got_method: &mut bool,
) -> PResult<()> {
    let no_clusters = stage.clusters.is_empty();
    let sample = stage.sample.as_mut().expect("a sample stage");
    match name {
        "METHOD" => {
            require_assignment(p, "TYPE")?;
            let type_start = p.pos;
            if let Some(&(keyword, method)) = METHODS.iter().find(|(k, _)| p.eat_word(k)) {
                if method.pps() && number > 1 {
                    return Err(error(
                        p,
                        type_start,
                        format!(
                            "{keyword} is allowed only in the first stage, like every PPS method"
                        ),
                    ));
                }
                sample.method = method;
            } else {
                let expected: Vec<&str> = METHODS.iter().map(|(k, _)| *k).collect();
                return Err(p.unexpected(&expected));
            }
            *got_method = true;
            if eat_assignment(p, "ESTIMATION")? {
                if p.eat_word("WR") {
                    if number > 1 {
                        return Err(error(
                            p,
                            start,
                            "ESTIMATION=WR is allowed only in the first stage",
                        ));
                    }
                    sample.estimation_wr = true;
                } else if !p.eat_keyword("DEFAULT") {
                    return Err(p.unexpected(&["DEFAULT", "WR"]));
                }
            }
        }
        "MOS" => {
            if number > 1 {
                return Err(error(p, start, "MOS is allowed only in the first stage"));
            }
            let mut seen = Vec::new();
            let mut mos = CsMos {
                variable: None,
                min: None,
                max: None,
            };
            if at_end(p) {
                return Err(error(p, start, "MOS needs VARIABLE= or SOURCE=FROMDATA"));
            }
            while !at_end(p) {
                let at = p.pos;
                if eat_assignment(p, "VARIABLE")? {
                    once(p, at, &mut seen, "VARIABLE")?;
                    mos.variable = Some(p.name("a variable name")?);
                } else if eat_assignment(p, "SOURCE")? {
                    once(p, at, &mut seen, "SOURCE")?;
                    if !p.eat_keyword("FROMDATA") {
                        return Err(p.unexpected(&["FROMDATA"]));
                    }
                    if no_clusters {
                        return Err(error(p, at, "SOURCE=FROMDATA needs CLUSTER in DESIGN"));
                    }
                } else if let Some(key) = ["MIN", "MAX"]
                    .into_iter()
                    .find(|k| matches!(eat_assignment(p, k), Ok(true)))
                {
                    once(p, at, &mut seen, key)?;
                    let slot = if key == "MIN" {
                        &mut mos.min
                    } else {
                        &mut mos.max
                    };
                    match p.number() {
                        Some(v) if v > 0.0 => *slot = Some(v),
                        _ => return Err(error(p, at, "MIN and MAX of MOS are above 0")),
                    }
                } else {
                    return Err(p.unexpected(&["VARIABLE =", "SOURCE =", "MIN =", "MAX ="]));
                }
            }
            if seen.contains(&"VARIABLE") == seen.contains(&"SOURCE") {
                return Err(error(
                    p,
                    start,
                    "MOS needs one of VARIABLE= and SOURCE=FROMDATA",
                ));
            }
            if let (Some(min), Some(max)) = (mos.min, mos.max)
                && min > max
            {
                return Err(error(p, start, "MIN of MOS is above its MAX"));
            }
            sample.mos = Some(mos);
        }
        "SIZE" | "RATE" => {
            if sample.size.is_some() || sample.rate.is_some() {
                return Err(error(
                    p,
                    start,
                    "SIZE and RATE cannot both be given for one stage",
                ));
            }
            let value = super::rate(p, name)?;
            if name == "SIZE" {
                sample.size = Some(value);
                return Ok(());
            }
            sample.rate = Some(value);
            let mut seen = Vec::new();
            while !at_end(p) {
                let at = p.pos;
                if eat_assignment(p, "MINSIZE")? {
                    once(p, at, &mut seen, "MINSIZE")?;
                    sample.minsize = Some(count(p, "MINSIZE")?);
                } else if eat_assignment(p, "MAXSIZE")? {
                    once(p, at, &mut seen, "MAXSIZE")?;
                    sample.maxsize = Some(count(p, "MAXSIZE")?);
                } else {
                    return Err(p.unexpected(&["MINSIZE =", "MAXSIZE =", "'/'"]));
                }
            }
            if let (Some(min), Some(max)) = (sample.minsize, sample.maxsize)
                && min > max
            {
                return Err(error(p, start, "MINSIZE is above MAXSIZE"));
            }
        }
        _ => {
            // STAGEVARS
            if at_end(p) {
                return Err(error(p, start, "STAGEVARS needs at least one keyword"));
            }
            let mut seen = Vec::new();
            let vars = &mut sample.vars;
            while !at_end(p) {
                let at = p.pos;
                let (keyword, root, slot): (&'static str, &str, &mut Option<String>) =
                    if p.eat_keyword("INCLPROB") {
                        once(p, at, &mut seen, "INCLPROB")?;
                        if let Some(n) = paren_name(p)? {
                            vars.inclprob = n;
                        }
                        continue;
                    } else if p.eat_keyword("CUMWEIGHT") {
                        once(p, at, &mut seen, "CUMWEIGHT")?;
                        if let Some(n) = paren_name(p)? {
                            vars.cumweight = n;
                        }
                        continue;
                    } else if p.eat_keyword("INDEX") {
                        ("INDEX", "Index", &mut vars.index)
                    } else if p.eat_keyword("POPSIZE") {
                        ("POPSIZE", "PopulationSize", &mut vars.popsize)
                    } else if p.eat_keyword("SAMPSIZE") {
                        ("SAMPSIZE", "SampleSize", &mut vars.sampsize)
                    } else if p.eat_keyword("RATE") {
                        ("RATE", "SamplingRate", &mut vars.rate)
                    } else if p.eat_keyword("WEIGHT") {
                        ("WEIGHT", "SampleWeight", &mut vars.weight)
                    } else {
                        return Err(p.unexpected(&[
                            "INCLPROB",
                            "CUMWEIGHT",
                            "INDEX",
                            "POPSIZE",
                            "SAMPSIZE",
                            "RATE",
                            "WEIGHT",
                        ]));
                    };
                once(p, at, &mut seen, keyword)?;
                *slot = Some(paren_name(p)?.unwrap_or_else(|| default_name(root, number)));
            }
        }
    }
    Ok(())
}

/// Checks the sample stages of a finished plan before the checks it shares with analysis
/// plans: METHOD, SIZE or RATE, where drawing with replacement may go, rates and the INDEX.
pub(super) fn check_sample(p: &Parser, plan: &mut Csplan, methods: &[bool]) -> PResult<()> {
    let end = p.pos;
    let last = plan.stages.len().saturating_sub(1);
    for (s, stage) in plan.stages.iter_mut().enumerate() {
        let number = s + 1;
        let sample = stage.sample.as_mut().expect("a sample stage");
        if !methods[s] {
            return Err(error(
                p,
                end,
                format!("stage {number} needs a METHOD subcommand"),
            ));
        }
        let method = sample.method;
        if method.draws_two() {
            if sample.size.take().is_some() || sample.rate.take().is_some() {
                plan.ignored.push(format!(
                    "SIZE and RATE are ignored in stage {number}: {} draws two units per stratum",
                    method.keyword()
                ));
            }
        } else if sample.size.is_none() && sample.rate.is_none() {
            return Err(error(p, end, format!("stage {number} needs SIZE or RATE")));
        }
        if !method.pps() && sample.mos.take().is_some() {
            plan.ignored.push(format!(
                "MOS is ignored in stage {number}: only the PPS methods use it"
            ));
        }
        if method.pps() && sample.mos.is_none() {
            return Err(error(
                p,
                end,
                format!(
                    "stage {number} draws with {}, which needs MOS",
                    method.keyword()
                ),
            ));
        }
        if method.estimator() == CsEstimator::UnequalWor && stage.clusters.is_empty() {
            return Err(error(
                p,
                end,
                format!(
                    "stage {number} draws with {}, which needs CLUSTER",
                    method.keyword()
                ),
            ));
        }
        let wr = method.with_replacement();
        if wr && s < last {
            return Err(error(
                p,
                end,
                format!(
                    "stage {number} draws with replacement ({}), which only the last stage may do",
                    sample.method.keyword()
                ),
            ));
        }
        if !wr {
            let too_high = match &sample.rate {
                Some(CsRate::Value(v)) => *v > 1.0,
                Some(CsRate::Matrix { rows, .. }) => rows.iter().any(|(_, v)| *v > 1.0),
                _ => false,
            };
            if too_high {
                return Err(error(
                    p,
                    end,
                    format!("stage {number} draws without replacement, so its RATE is at most 1"),
                ));
            }
            if sample.vars.index.take().is_some() {
                plan.ignored.push(format!(
                    "STAGEVARS INDEX is ignored in stage {number}, which draws without replacement"
                ));
            }
        } else if sample.vars.index.is_none() {
            sample.vars.index = Some(default_name("Index", number));
        }
    }
    Ok(())
}

/// After the shared checks: the estimator each method implies, and the inclusion probability
/// variable that EQUAL_WOR reads in the analysis.
pub(super) fn analysis_side(plan: &mut Csplan) {
    for stage in &mut plan.stages {
        let sample = stage.sample.as_ref().expect("a sample stage");
        stage.estimator = if sample.estimation_wr {
            CsEstimator::Wr
        } else {
            sample.method.estimator()
        };
        if stage.estimator == CsEstimator::EqualWor {
            stage.inclprob = Some(CsRate::Variable(sample.vars.inclprob.clone()));
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Csselect {
    pub plan: String,
    /// CRITERIA STAGES; empty for every stage of the plan.
    pub stages: Vec<usize>,
    /// CRITERIA SEED; None for RANDOM.
    pub seed: Option<u64>,
    /// CLASSMISSING=INCLUDE
    pub include: bool,
    pub renamevars: bool,
    /// SAMPLEFILE OUTFILE; None writes to the active dataset.
    pub samplefile: Option<String>,
    /// SAMPLEFILE KEEP (true) and DROP (false) in their order.
    pub subsets: Vec<(bool, Vec<VarItem>)>,
    pub jointprob: Option<String>,
    pub print_selection: bool,
    pub print_cps: bool,
}

fn outfile(p: &mut Parser) -> PResult<String> {
    require_assignment(p, "OUTFILE")?;
    p.file("a file or dataset name")
}

pub(crate) fn csselect(p: &mut Parser) -> PResult<Command> {
    let mut c = Csselect {
        plan: String::new(),
        stages: Vec::new(),
        seed: None,
        include: false,
        renamevars: false,
        samplefile: None,
        subsets: Vec::new(),
        jointprob: None,
        print_selection: true,
        print_cps: false,
    };
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let mut keys = Vec::new();
        if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            c.plan = plan_file(p)?;
        } else if p.eat_keyword("CRITERIA") {
            once(p, start, &mut seen, "CRITERIA")?;
            if at_end(p) {
                return Err(error(p, start, "CRITERIA needs STAGES= or SEED="));
            }
            while !at_end(p) {
                let at = p.pos;
                if eat_assignment(p, "STAGES")? {
                    once(p, at, &mut keys, "STAGES")?;
                    while let Some(TokenKind::Number(_)) = p.peek() {
                        let v = count(p, "a stage")?;
                        c.stages.push(v as usize);
                    }
                    if c.stages.is_empty() {
                        return Err(p.unexpected(&["a stage number"]));
                    }
                    let consecutive = c.stages.windows(2).all(|w| w[1] == w[0] + 1);
                    if c.stages.len() > 3 || !consecutive || c.stages[0] > 3 {
                        return Err(error(
                            p,
                            at,
                            "STAGES lists one to three consecutive stages, such as 2 3",
                        ));
                    }
                } else if eat_assignment(p, "SEED")? {
                    once(p, at, &mut keys, "SEED")?;
                    if !p.eat_keyword("RANDOM") {
                        let v = count(p, "SEED")?;
                        c.seed = Some(v as u64);
                    }
                } else {
                    return Err(p.unexpected(&["STAGES =", "SEED ="]));
                }
            }
        } else if p.eat_keyword("CLASSMISSING") {
            once(p, start, &mut seen, "CLASSMISSING")?;
            if p.eat_keyword("INCLUDE") {
                c.include = true;
            } else if !p.eat_keyword("EXCLUDE") {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("DATA") {
            once(p, start, &mut seen, "DATA")?;
            if at_end(p) {
                return Err(error(p, start, "DATA needs RENAMEVARS or PRESORTED"));
            }
            while !at_end(p) {
                let at = p.pos;
                if p.eat_keyword("RENAMEVARS") {
                    once(p, at, &mut keys, "RENAMEVARS")?;
                    c.renamevars = true;
                } else if p.eat_keyword("PRESORTED") {
                    // the draw does not need sorted data, so the hint changes nothing
                    once(p, at, &mut keys, "PRESORTED")?;
                } else {
                    return Err(p.unexpected(&["RENAMEVARS", "PRESORTED"]));
                }
            }
        } else if p.eat_keyword("SAMPLEFILE") {
            once(p, start, &mut seen, "SAMPLEFILE")?;
            while !at_end(p) {
                let at = p.pos;
                if p.at_keyword("OUTFILE") {
                    once(p, at, &mut keys, "OUTFILE")?;
                    c.samplefile = Some(outfile(p)?);
                } else if eat_assignment(p, "KEEP")? {
                    once(p, at, &mut keys, "KEEP")?;
                    c.subsets.push((true, super::ratio_list(p)?));
                } else if eat_assignment(p, "DROP")? {
                    once(p, at, &mut keys, "DROP")?;
                    c.subsets.push((false, super::ratio_list(p)?));
                } else {
                    return Err(p.unexpected(&["OUTFILE =", "KEEP =", "DROP ="]));
                }
            }
            if c.samplefile.is_none() {
                return Err(error(p, start, "SAMPLEFILE needs OUTFILE='file'"));
            }
        } else if p.eat_keyword("JOINTPROB") {
            once(p, start, &mut seen, "JOINTPROB")?;
            c.jointprob = Some(outfile(p)?);
        } else if p.eat_keyword("SELECTRULE") {
            return Err(error(
                p,
                start,
                "SELECTRULE (a text rule describing the selected units) is not supported yet",
            ));
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            if at_end(p) {
                return Err(error(p, start, "PRINT needs SELECTION or CPS"));
            }
            c.print_selection = false;
            while !at_end(p) {
                if p.eat_keyword("SELECTION") {
                    c.print_selection = true;
                } else if p.eat_keyword("CPS") {
                    c.print_cps = true;
                } else {
                    return Err(p.unexpected(&["SELECTION", "CPS"]));
                }
            }
        } else {
            return Err(p.subcommand_error("CSSELECT", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let end = p.pos;
    if c.plan.is_empty() {
        return Err(error(p, end, "CSSELECT needs PLAN FILE='name'"));
    }
    if let (Some(a), Some(b)) = (&c.samplefile, &c.jointprob)
        && a.eq_ignore_ascii_case(b)
    {
        return Err(error(
            p,
            end,
            "SAMPLEFILE and JOINTPROB cannot write the same file",
        ));
    }
    Ok(Command::Csselect(Box::new(c)))
}
