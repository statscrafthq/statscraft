//! Parsers of SIMPLAN, SIMRUN and the BEGIN EXPR and END EXPR block.
//!
//! Rules, from the command descriptions:
//! - Command, subcommand and keyword names are written in full; equals signs and parentheses
//!   as shown are required; empty subcommands are not allowed.
//! - SIMPLAN CREATE needs PLAN and at least one SIMINPUT; SIMINPUT, FIXEDINPUT, TARGETOPTS and
//!   VALUELABELS may repeat, every other subcommand comes once. SIMPLAN VIEW takes PLAN only.
//! - SIMINPUT TYPE is AUTOFIT unless MANUAL; DISTRIBUTION goes with MANUAL only. A parameter
//!   with several values (several value sets for PROBS) asks for one simulation per value, the
//!   sensitivity analysis: for one SIMINPUT with one input, or one FIXEDINPUT with ITERATIONS
//!   and one input, never both.
//! - SIMRUN needs PLAN; the other subcommands only shape the output.
//! - BEGIN EXPR takes /OUTFILE PREPXML='file'; END EXPR stands alone.

use crate::ast::{Command, Value};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// A field named on INPUT or TARGET, with its MAPTO field and FORMAT such as "F,4".
#[derive(Debug, Clone, PartialEq)]
pub struct SimField {
    pub name: String,
    pub mapto: Option<String>,
    pub format: Option<String>,
}

/// The valid range of a target; None is LO or HI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetFilter {
    pub min: Option<f64>,
    pub min_inclusive: bool,
    pub max: Option<f64>,
    pub max_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetOptions {
    pub targets: Vec<SimField>,
    pub comment: Option<String>,
    pub output: bool,
    pub filter: Option<TargetFilter>,
}

/// A distribution as written: its name and each parameter's value sets (`;` separates sets).
#[derive(Debug, Clone, PartialEq)]
pub struct SimDistribution {
    pub family: String,
    pub params: Vec<(String, Vec<Vec<f64>>)>,
    /// NEGBIN TYPE=TRIALS
    pub trials: bool,
    /// CATEGORICAL CONTINGENCY=YES
    pub contingency: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimInput {
    pub inputs: Vec<SimField>,
    pub output: bool,
    /// TYPE=AUTOFIT with the distributions to try (empty: the default set); None is MANUAL.
    pub autofit: Option<Vec<String>>,
    pub distribution: Option<SimDistribution>,
    pub minval: Option<f64>,
    pub maxval: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixedInput {
    pub inputs: Vec<SimField>,
    /// VALUE, or the values of ITERATIONS.
    pub values: Vec<f64>,
    pub iterations: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StopRule {
    /// Until the width of the confidence interval of the mean is at most `thresh`, a value or a
    /// percent of the mean.
    Mean {
        thresh: f64,
        percent: bool,
        level: f64,
    },
    /// Until `frequency` cases lie beyond `thresh`, a value or a percentile.
    Tail {
        thresh: f64,
        percentile: bool,
        left: bool,
        frequency: usize,
    },
}

/// VALUELABELS: the variables and their (value, label) pairs.
pub type LabelGroup = (Vec<String>, Vec<(Value, String)>);

#[derive(Debug, Clone, PartialEq)]
pub struct Simplan {
    pub view: bool,
    /// SOURCE='file'
    pub source: Option<String>,
    /// MODEL FILE and CUSTOM=YES.
    pub model: Option<(String, bool)>,
    pub targets: Vec<TargetOptions>,
    pub inputs: Vec<SimInput>,
    pub fixed: Vec<FixedInput>,
    /// CORRELATIONS VARORDER and the full symmetric matrix.
    pub correlations: Option<(Vec<String>, Vec<Vec<f64>>)>,
    pub multiway: bool,
    pub ncases: Option<usize>,
    /// AUTOFIT FIT=KS
    pub ks: bool,
    pub max_iterations: usize,
    pub bins: usize,
    pub max_cases: usize,
    pub stop_target: Option<String>,
    pub stop_rule: Option<StopRule>,
    /// MISSING CLASSMISSING=INCLUDE
    pub include_missing: bool,
    pub value_labels: Vec<LabelGroup>,
    pub plan: String,
    pub display: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Simrun {
    pub plan: String,
    /// None with REPRESULTS=FALSE.
    pub seed: Option<f64>,
    pub pdf: bool,
    pub cdf: bool,
    pub cdf_ascending: bool,
    pub cdf_descending: bool,
    pub scatter: bool,
    pub boxplot: bool,
    pub tornado_corr: bool,
    pub tornado_delta: Option<f64>,
    pub tornado_variance: bool,
    /// DESCRIPTIVES=YES with its CILEVEL.
    pub descriptives: Option<f64>,
    pub ntiles: Option<usize>,
    pub ptiles: Vec<f64>,
    pub percentiles: bool,
    pub associations: bool,
    pub outfile: Option<String>,
}

fn whole(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<usize> {
    let n = number(p, what, low, high)?;
    if n.fract() != 0.0 {
        return Err(SyntaxError::new(
            p.span_from(p.pos - 1),
            format!("{what} must be a whole number"),
        ));
    }
    Ok(n as usize)
}

fn number(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(n) if (low..=high).contains(&n) => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be from {low} to {high}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// `KEYWORD =` with the equals sign required.
fn key(p: &mut Parser, keyword: &str) -> PResult<bool> {
    if !p.eat_word(keyword) {
        return Ok(false);
    }
    p.expect(Punct::Eq)?;
    Ok(true)
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    if p.eat_word("YES") || p.eat_word("TRUE") {
        Ok(true)
    } else if p.eat_word("NO") || p.eat_word("FALSE") {
        Ok(false)
    } else {
        Err(p.unexpected(&["YES", "NO"]))
    }
}

fn numbers(p: &mut Parser) -> Vec<f64> {
    let mut out = Vec::new();
    while let Some(n) = p.number() {
        out.push(n);
        p.eat(Punct::Comma);
    }
    out
}

/// Field names, each with an optional `(MAPTO=name FORMAT=type[,d])`, up to the next keyword.
fn fields(p: &mut Parser) -> PResult<Vec<SimField>> {
    let mut out = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_)))
        && !matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)))
    {
        let mut field = SimField {
            name: p.name("a field name")?,
            mapto: None,
            format: None,
        };
        if p.eat(Punct::LParen) {
            while !p.eat(Punct::RParen) {
                if key(p, "MAPTO")? {
                    field.mapto = Some(p.name("a field name")?);
                } else if key(p, "FORMAT")? {
                    let mut format = p.name("a format such as F or DOLLAR")?.to_uppercase();
                    if p.eat(Punct::Comma) {
                        let d = whole(p, "the number of decimals", 0.0, 16.0)?;
                        format.push_str(&format!(",{d}"));
                    }
                    field.format = Some(format);
                } else {
                    return Err(p.unexpected(&["MAPTO", "FORMAT", "')'"]));
                }
            }
        }
        out.push(field);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a field name"]));
    }
    Ok(out)
}

const FAMILIES: [&str; 15] = [
    "BERNOULLI",
    "BETA",
    "BINOM",
    "CATEGORICAL",
    "EMPIRICAL",
    "EXP",
    "GAMMA",
    "LNORMAL",
    "NEGBIN",
    "NORMAL",
    "POISSON",
    "TRIANGULAR",
    "UNIFORM",
    "USER_RANGES",
    "WEIBULL",
];

fn distribution(p: &mut Parser) -> PResult<SimDistribution> {
    let Some(family) = FAMILIES.into_iter().find(|f| p.eat_word(f)) else {
        return Err(p.unexpected(&FAMILIES));
    };
    let mut d = SimDistribution {
        family: family.into(),
        params: Vec::new(),
        trials: false,
        contingency: false,
    };
    if !p.eat(Punct::LParen) {
        return Ok(d);
    }
    while !p.eat(Punct::RParen) {
        let name = p.name("a parameter name")?.to_uppercase();
        p.expect(Punct::Eq)?;
        match name.as_str() {
            "TYPE" if family == "NEGBIN" => {
                d.trials = p.eat_word("TRIALS");
                if !d.trials && !p.eat_word("FAILURES") {
                    return Err(p.unexpected(&["FAILURES", "TRIALS"]));
                }
            }
            "CONTINGENCY" if family == "CATEGORICAL" => d.contingency = yes_no(p)?,
            "SOURCE" if family == "EMPIRICAL" => {
                if !p.eat_word("AUTOFIT") {
                    p.string("a quoted plan file")?;
                    return Err(SyntaxError::new(
                        p.span_from(p.pos - 1),
                        "EMPIRICAL(SOURCE='file') is not supported yet: the empirical \
                         distribution is fitted to the active dataset",
                    ));
                }
            }
            _ => {
                if matches!(p.peek(), Some(TokenKind::Str(_))) {
                    return Err(SyntaxError::new(
                        p.span(),
                        "string categories are not supported yet; give numbers",
                    ));
                }
                let mut sets = vec![numbers(p)];
                while p.eat(Punct::Semicolon) {
                    sets.push(numbers(p));
                }
                if sets.iter().any(Vec::is_empty) {
                    return Err(p.unexpected(&["a number"]));
                }
                d.params.push((name, sets));
            }
        }
    }
    Ok(d)
}

const AUTOFIT_GROUPS: [&[&str]; 3] = [
    &["CATEGORICAL"],
    &["BINOM", "NEGBIN", "POISSON"],
    &[
        "BETA",
        "EXP",
        "GAMMA",
        "LNORMAL",
        "NORMAL",
        "TRIANGULAR",
        "UNIFORM",
        "WEIBULL",
    ],
];

fn siminput(p: &mut Parser) -> PResult<SimInput> {
    let mut input = SimInput {
        inputs: Vec::new(),
        output: true,
        autofit: Some(Vec::new()),
        distribution: None,
        minval: None,
        maxval: None,
    };
    while !at_end(p) {
        if key(p, "INPUT")? {
            input.inputs = fields(p)?;
        } else if key(p, "OUTPUT")? {
            input.output = yes_no(p)?;
        } else if key(p, "TYPE")? {
            if p.eat_word("MANUAL") {
                input.autofit = None;
                // LOCK and SAVEASFITTED keep the state of a dialog; they change no result
                if p.eat(Punct::LParen) {
                    while !p.eat(Punct::RParen) {
                        if !(key(p, "LOCK")? || key(p, "SAVEASFITTED")?) {
                            return Err(p.unexpected(&["LOCK", "SAVEASFITTED"]));
                        }
                        yes_no(p)?;
                    }
                }
            } else if p.eat_word("AUTOFIT") {
                let mut names = Vec::new();
                if p.eat(Punct::LParen) {
                    while !p.eat(Punct::RParen) {
                        if p.eat_word("DEFAULT") {
                            continue;
                        }
                        let all = AUTOFIT_GROUPS.concat();
                        let Some(name) = all.into_iter().find(|n| p.eat_word(n)) else {
                            return Err(p.unexpected(&["DEFAULT", "a distribution name"]));
                        };
                        names.push(name.to_string());
                    }
                }
                let groups = AUTOFIT_GROUPS
                    .iter()
                    .filter(|g| names.iter().any(|n| g.contains(&n.as_str())))
                    .count();
                if groups > 1 {
                    return Err(SyntaxError::new(
                        p.span_from(p.pos - 1),
                        "AUTOFIT cannot mix distributions of different measurement levels \
                         (CATEGORICAL; BINOM NEGBIN POISSON; the continuous ones)",
                    ));
                }
                input.autofit = Some(names);
            } else {
                return Err(p.unexpected(&["MANUAL", "AUTOFIT"]));
            }
        } else if key(p, "DISTRIBUTION")? {
            input.distribution = Some(distribution(p)?);
        } else if key(p, "MINVAL")? {
            input.minval = Some(number(p, "MINVAL", f64::MIN, f64::MAX)?);
        } else if key(p, "MAXVAL")? {
            input.maxval = Some(number(p, "MAXVAL", f64::MIN, f64::MAX)?);
        } else {
            return Err(p.unexpected(&[
                "INPUT",
                "OUTPUT",
                "TYPE",
                "DISTRIBUTION",
                "MINVAL",
                "MAXVAL",
            ]));
        }
    }
    let span = p.span_from(p.pos - 1);
    if input.inputs.is_empty() {
        return Err(SyntaxError::new(span, "SIMINPUT needs INPUT="));
    }
    match (&input.autofit, &input.distribution) {
        (Some(_), Some(_)) => Err(SyntaxError::new(
            span,
            "DISTRIBUTION goes with TYPE=MANUAL; add TYPE=MANUAL",
        )),
        (None, None) => Err(SyntaxError::new(
            span,
            "TYPE=MANUAL needs DISTRIBUTION= (taking the input from a plan file with SOURCE is \
             not supported yet)",
        )),
        _ => Ok(input),
    }
}

fn filter_bound(p: &mut Parser, keyword: &str, open: &str) -> PResult<(Option<f64>, bool)> {
    p.expect(Punct::Eq)?;
    let value = match p.eat_word(open) || p.eat_word(&format!("{open}WEST")) {
        true => None,
        false => Some(number(p, keyword, f64::MIN, f64::MAX)?),
    };
    let mut inclusive = true;
    if p.eat(Punct::LParen) {
        inclusive = p.eat_word("INCLUSIVE");
        if !inclusive && !p.eat_word("EXCLUSIVE") {
            return Err(p.unexpected(&["INCLUSIVE", "EXCLUSIVE"]));
        }
        p.expect(Punct::RParen)?;
    }
    Ok((value, inclusive))
}

fn targetopts(p: &mut Parser) -> PResult<TargetOptions> {
    let mut t = TargetOptions {
        targets: Vec::new(),
        comment: None,
        output: true,
        filter: None,
    };
    while !at_end(p) {
        if key(p, "TARGET")? {
            t.targets = fields(p)?;
        } else if key(p, "COMMENT")? {
            t.comment = Some(p.string("a quoted comment")?);
        } else if key(p, "OUTPUT")? {
            t.output = yes_no(p)?;
        } else if key(p, "TARGETFILTER")? {
            if !yes_no(p)? {
                t.filter = None;
                continue;
            }
            let mut f = TargetFilter {
                min: None,
                min_inclusive: true,
                max: None,
                max_inclusive: true,
            };
            p.expect(Punct::LParen)?;
            while !p.eat(Punct::RParen) {
                if p.eat_word("MIN") {
                    (f.min, f.min_inclusive) = filter_bound(p, "MIN", "LO")?;
                } else if p.eat_word("MAX") {
                    // HI or HIGHEST
                    (f.max, f.max_inclusive) = match p.eat_word("HIGHEST") {
                        true => (None, true),
                        false => filter_bound(p, "MAX", "HI")?,
                    };
                } else {
                    return Err(p.unexpected(&["MIN", "MAX", "')'"]));
                }
            }
            t.filter = Some(f);
        } else {
            return Err(p.unexpected(&["TARGET", "COMMENT", "OUTPUT", "TARGETFILTER"]));
        }
    }
    if t.targets.is_empty() {
        return Err(SyntaxError::new(p.span(), "TARGETOPTS needs TARGET="));
    }
    Ok(t)
}

fn fixedinput(p: &mut Parser) -> PResult<FixedInput> {
    let mut f = FixedInput {
        inputs: Vec::new(),
        values: Vec::new(),
        iterations: false,
    };
    while !at_end(p) {
        if key(p, "INPUT")? {
            f.inputs = fields(p)?;
        } else if key(p, "VALUE")? {
            if matches!(p.peek(), Some(TokenKind::Str(_))) {
                return Err(SyntaxError::new(
                    p.span(),
                    "string fixed inputs are not supported yet",
                ));
            }
            f.values = vec![number(p, "VALUE", f64::MIN, f64::MAX)?];
        } else if key(p, "ITERATIONS")? {
            f.iterations = true;
            if p.eat_word("VALUES") {
                p.expect(Punct::LParen)?;
                f.values = numbers(p);
                p.expect(Punct::RParen)?;
            } else if p.eat_word("RANGE") {
                let (mut min, mut max, mut n) = (None, None, None);
                p.expect(Punct::LParen)?;
                while !p.eat(Punct::RParen) {
                    if key(p, "MIN")? {
                        min = Some(number(p, "MIN", f64::MIN, f64::MAX)?);
                    } else if key(p, "MAX")? {
                        max = Some(number(p, "MAX", f64::MIN, f64::MAX)?);
                    } else if key(p, "N")? {
                        n = Some(whole(p, "N", 2.0, 1e6)?);
                    } else {
                        return Err(p.unexpected(&["MIN", "MAX", "N"]));
                    }
                }
                let (Some(min), Some(max), Some(n)) = (min, max, n) else {
                    return Err(SyntaxError::new(p.span(), "RANGE needs MIN, MAX and N"));
                };
                let step = (max - min) / (n - 1) as f64;
                f.values = (0..n).map(|i| min + step * i as f64).collect();
            } else {
                return Err(p.unexpected(&["VALUES", "RANGE"]));
            }
            if f.values.is_empty() {
                return Err(p.unexpected(&["a number"]));
            }
        } else if key(p, "LOCK")? {
            yes_no(p)?;
        } else {
            return Err(p.unexpected(&["INPUT", "VALUE", "ITERATIONS", "LOCK"]));
        }
    }
    let span = p.span();
    if f.inputs.is_empty() || f.values.is_empty() {
        return Err(SyntaxError::new(
            span,
            "FIXEDINPUT needs INPUT= and VALUE= or ITERATIONS=",
        ));
    }
    if f.iterations && f.inputs.len() > 1 {
        return Err(SyntaxError::new(
            span,
            "ITERATIONS takes a single input on INPUT=",
        ));
    }
    Ok(f)
}

/// CORRMATRIX: the lower triangle row by row, rows ended by `;`, made symmetric.
fn corrmatrix(p: &mut Parser) -> PResult<Vec<Vec<f64>>> {
    let mut rows = vec![numbers(p)];
    while p.eat(Punct::Semicolon) {
        rows.push(numbers(p));
    }
    let k = rows.len();
    for (i, row) in rows.iter().enumerate() {
        if row.len() != i + 1 || row[i] != 1.0 || row.iter().any(|r| !(-1.0..=1.0).contains(r)) {
            return Err(SyntaxError::new(
                p.span(),
                format!(
                    "row {} of CORRMATRIX must hold {} values from -1 to 1 ending with the 1 of \
                     the diagonal",
                    i + 1,
                    i + 1
                ),
            ));
        }
    }
    Ok((0..k)
        .map(|i| {
            (0..k)
                .map(|j| rows[i.max(j)][i.min(j)])
                .collect::<Vec<f64>>()
        })
        .collect())
}

fn stopcriteria(p: &mut Parser, plan: &mut Simplan) -> PResult<()> {
    while !at_end(p) {
        if key(p, "MAXCASES")? {
            plan.max_cases = whole(p, "MAXCASES", 1.0, 1e9)?;
        } else if key(p, "TARGET")? {
            plan.stop_target = Some(p.name("a target name")?);
        } else if key(p, "TYPE")?
            || matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("TAIL"))
        {
            let mean = p.eat_word("MEAN");
            if !mean && !p.eat_word("TAIL") {
                return Err(p.unexpected(&["MEAN", "TAIL"]));
            }
            let (mut thresh, mut unit, mut level, mut left, mut frequency) =
                (None, None, 95.0, false, None);
            p.expect(Punct::LParen)?;
            while !p.eat(Punct::RParen) {
                if key(p, "THRESH")? {
                    thresh = Some(number(p, "THRESH", f64::MIN, f64::MAX)?);
                } else if key(p, "UNIT")? {
                    let words: &[&str] = match mean {
                        true => &["PERCENT", "VALUE"],
                        false => &["PERCENTILE", "VALUE"],
                    };
                    let Some(w) = words.iter().find(|w| p.eat_word(w)) else {
                        return Err(p.unexpected(words));
                    };
                    unit = Some(*w != "VALUE");
                } else if mean && key(p, "CILEVEL")? {
                    level = number(p, "CILEVEL", 50.0, 99.9999)?;
                } else if !mean && key(p, "SIDE")? {
                    left = p.eat_word("LEFT");
                    if !left && !p.eat_word("RIGHT") {
                        return Err(p.unexpected(&["RIGHT", "LEFT"]));
                    }
                } else if !mean && key(p, "FREQUENCY")? {
                    frequency = Some(whole(p, "FREQUENCY", 1.0, 1e9)?);
                } else {
                    return Err(p.unexpected(&["THRESH", "UNIT", "CILEVEL", "SIDE", "FREQUENCY"]));
                }
            }
            let span = p.span_from(p.pos - 1);
            let (Some(thresh), Some(percent)) = (thresh, unit) else {
                return Err(SyntaxError::new(
                    span,
                    "the stopping rule needs THRESH and UNIT",
                ));
            };
            plan.stop_rule = Some(match mean {
                true => StopRule::Mean {
                    thresh,
                    percent,
                    level,
                },
                false => StopRule::Tail {
                    thresh,
                    percentile: percent,
                    left,
                    frequency: frequency
                        .ok_or_else(|| SyntaxError::new(span, "TAIL needs FREQUENCY"))?,
                },
            });
        } else {
            return Err(p.unexpected(&["MAXCASES", "TARGET", "TYPE"]));
        }
    }
    Ok(())
}

pub(crate) fn simplan(p: &mut Parser) -> PResult<Command> {
    let view = p.eat_word("VIEW");
    if !view && !p.eat_word("CREATE") {
        return Err(p.unexpected(&["CREATE", "VIEW"]));
    }
    let mut plan = Simplan {
        view,
        source: None,
        model: None,
        targets: Vec::new(),
        inputs: Vec::new(),
        fixed: Vec::new(),
        correlations: None,
        multiway: false,
        ncases: None,
        ks: false,
        max_iterations: 100,
        bins: 100,
        max_cases: 100_000,
        stop_target: None,
        stop_rule: None,
        include_missing: false,
        value_labels: Vec::new(),
        plan: String::new(),
        display: true,
    };
    if !view && key(p, "SOURCE")? && !p.eat_word("AUTOFIT") {
        plan.source = Some(p.string("a quoted plan file")?);
    }
    let mut seen: Vec<String> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let name = p.name("a subcommand")?.to_uppercase();
        let repeats = ["SIMINPUT", "FIXEDINPUT", "TARGETOPTS", "VALUELABELS"];
        if seen.contains(&name) && !repeats.contains(&name.as_str()) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} may be given once"),
            ));
        }
        if at_end(p) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} is empty"),
            ));
        }
        if view && name != "PLAN" {
            return Err(SyntaxError::new(
                p.span_from(start),
                "SIMPLAN VIEW takes PLAN only",
            ));
        }
        seen.push(name.clone());
        match name.as_str() {
            "MODEL" => {
                let (mut file, mut custom) = (None, false);
                while !at_end(p) {
                    if key(p, "FILE")? {
                        file = Some(p.file("a quoted model file")?);
                    } else if key(p, "CUSTOM")? {
                        custom = yes_no(p)?;
                    } else {
                        return Err(p.unexpected(&["FILE", "CUSTOM"]));
                    }
                }
                let file = file.ok_or_else(|| SyntaxError::new(p.span(), "MODEL needs FILE="))?;
                plan.model = Some((file, custom));
            }
            "TARGETOPTS" => plan.targets.push(targetopts(p)?),
            "SIMINPUT" => plan.inputs.push(siminput(p)?),
            "FIXEDINPUT" => plan.fixed.push(fixedinput(p)?),
            "CORRELATIONS" => {
                let (mut order, mut matrix) = (Vec::new(), Vec::new());
                while !at_end(p) {
                    if key(p, "VARORDER")? {
                        while matches!(p.peek(), Some(TokenKind::Word(_)))
                            && !matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)))
                        {
                            order.push(p.name("a field name")?);
                        }
                    } else if key(p, "CORRMATRIX")? {
                        matrix = corrmatrix(p)?;
                    } else if key(p, "LOCK")? {
                        yes_no(p)?;
                    } else {
                        return Err(p.unexpected(&["VARORDER", "CORRMATRIX", "LOCK"]));
                    }
                }
                if order.is_empty() || order.len() != matrix.len() {
                    return Err(SyntaxError::new(
                        p.span(),
                        "CORRELATIONS needs VARORDER and a CORRMATRIX with one row per field",
                    ));
                }
                plan.correlations = Some((order, matrix));
            }
            "CONTINGENCY" => {
                if !key(p, "MULTIWAY")? {
                    return Err(p.unexpected(&["MULTIWAY"]));
                }
                plan.multiway = yes_no(p)?;
            }
            "AUTOFIT" => {
                while !at_end(p) {
                    if key(p, "NCASES")? {
                        plan.ncases = match p.eat_word("ALL") {
                            true => None,
                            false => Some(whole(p, "NCASES", 1.0, 1e12)?),
                        };
                    } else if key(p, "FIT")? {
                        plan.ks = p.eat_word("KS");
                        if !plan.ks && !p.eat_word("AD") {
                            return Err(p.unexpected(&["AD", "KS"]));
                        }
                    } else if key(p, "MAXITERATIONS")? {
                        plan.max_iterations = whole(p, "MAXITERATIONS", 1.0, 1e6)?;
                    } else if key(p, "BINS")? {
                        plan.bins = whole(p, "BINS", 2.0, 1000.0)?;
                    } else {
                        return Err(p.unexpected(&["NCASES", "FIT", "MAXITERATIONS", "BINS"]));
                    }
                }
            }
            "STOPCRITERIA" => stopcriteria(p, &mut plan)?,
            "MISSING" => {
                if !key(p, "CLASSMISSING")? {
                    return Err(p.unexpected(&["CLASSMISSING"]));
                }
                plan.include_missing = p.eat_word("INCLUDE");
                if !plan.include_missing && !p.eat_word("EXCLUDE") {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                }
            }
            "VALUELABELS" => {
                let (mut vars, mut labels) = (Vec::new(), Vec::new());
                while !at_end(p) {
                    if key(p, "VARIABLES")? {
                        while matches!(p.peek(), Some(TokenKind::Word(_)))
                            && !matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)))
                        {
                            vars.push(p.name("a variable name")?);
                        }
                    } else if key(p, "VALUES")? {
                        while let Some(value) = p.value() {
                            labels.push((value, p.string("a quoted label")?));
                        }
                    } else {
                        return Err(p.unexpected(&["VARIABLES", "VALUES"]));
                    }
                }
                plan.value_labels.push((vars, labels));
            }
            "PLAN" => {
                while !at_end(p) {
                    if key(p, "FILE")? {
                        plan.plan = p.file("a quoted plan file")?;
                    } else if !view && key(p, "DISPLAY")? {
                        plan.display = yes_no(p)?;
                    } else {
                        return Err(p.unexpected(&["FILE", "DISPLAY"]));
                    }
                }
            }
            _ => {
                return Err(p.subcommand_error(
                    "SIMPLAN",
                    &[
                        "MODEL",
                        "TARGETOPTS",
                        "SIMINPUT",
                        "FIXEDINPUT",
                        "CORRELATIONS",
                        "CONTINGENCY",
                        "AUTOFIT",
                        "STOPCRITERIA",
                        "MISSING",
                        "VALUELABELS",
                        "PLAN",
                    ],
                ));
            }
        }
    }
    p.finish(&["'/'"])?;
    let span = p.span();
    if plan.plan.is_empty() {
        return Err(SyntaxError::new(span, "SIMPLAN needs /PLAN FILE='file'"));
    }
    if view {
        return Ok(Command::Simplan(Box::new(plan)));
    }
    if plan.inputs.is_empty() {
        return Err(SyntaxError::new(
            span,
            "SIMPLAN CREATE needs a SIMINPUT subcommand",
        ));
    }
    let iterating = plan
        .inputs
        .iter()
        .filter(|i| i.distribution.as_ref().is_some_and(|d| iterations(d) > 0))
        .collect::<Vec<_>>();
    let fixed = plan.fixed.iter().filter(|f| f.iterations).count();
    let rule = if iterating.len() + fixed > 1 {
        Some("only one SIMINPUT or FIXEDINPUT may vary a value (sensitivity analysis)")
    } else if iterating.iter().any(|i| i.inputs.len() > 1) {
        Some("a SIMINPUT that varies a parameter takes a single input")
    } else if iterating
        .iter()
        .any(|i| iterations(i.distribution.as_ref().expect("filtered")) > 1)
    {
        Some("only one parameter of a distribution may take several values")
    } else {
        None
    };
    match rule {
        Some(rule) => Err(SyntaxError::new(span, rule)),
        None => Ok(Command::Simplan(Box::new(plan))),
    }
}

/// Whether a parameter of the distribution holds the values of a sensitivity analysis: several
/// values of a single-valued parameter, or several sets of PROBS.
pub fn is_iterated(family: &str, name: &str, sets: &[Vec<f64>]) -> bool {
    match family {
        "CATEGORICAL" | "USER_RANGES" => sets.len() > 1 && name == "PROBS",
        _ => sets.len() > 1 || sets[0].len() > 1,
    }
}

/// How many parameters of `d` take several values.
fn iterations(d: &SimDistribution) -> usize {
    let iterated = |(name, sets): &&(String, Vec<Vec<f64>>)| is_iterated(&d.family, name, sets);
    d.params.iter().filter(iterated).count()
}

pub(crate) fn simrun(p: &mut Parser) -> PResult<Command> {
    let mut run = Simrun {
        plan: String::new(),
        seed: Some(629_111_597.0),
        pdf: true,
        cdf: false,
        cdf_ascending: true,
        cdf_descending: false,
        scatter: false,
        boxplot: false,
        tornado_corr: true,
        tornado_delta: None,
        tornado_variance: false,
        descriptives: Some(95.0),
        ntiles: None,
        ptiles: Vec::new(),
        percentiles: false,
        associations: false,
        outfile: None,
    };
    let mut seen: Vec<String> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let name = p.name("a subcommand")?.to_uppercase();
        if seen.contains(&name) || at_end(p) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} is empty or given twice"),
            ));
        }
        seen.push(name.clone());
        while !at_end(p) {
            match name.as_str() {
                "PLAN" if key(p, "FILE")? => run.plan = p.file("a quoted plan file")?,
                "CRITERIA" if key(p, "REPRESULTS")? => {
                    if !yes_no(p)? {
                        run.seed = None;
                    }
                }
                "CRITERIA" if key(p, "SEED")? => {
                    let seed = number(p, "SEED", 1.0, 2_147_483_646.0)?;
                    if run.seed.is_some() {
                        run.seed = Some(seed);
                    }
                }
                "DISTRIBUTION" if key(p, "DISPLAY")? => {
                    (run.pdf, run.cdf) = (false, false);
                    loop {
                        if p.eat_word("PDF") {
                            run.pdf = true;
                        } else if p.eat_word("CDF") {
                            run.cdf = true;
                        } else if !p.eat_word("NONE") {
                            break;
                        }
                    }
                }
                "DISTRIBUTION" if key(p, "CDFORDER")? => {
                    (run.cdf_ascending, run.cdf_descending) = (false, false);
                    loop {
                        if p.eat_word("ASCENDING") {
                            run.cdf_ascending = true;
                        } else if p.eat_word("DESCENDING") {
                            run.cdf_descending = true;
                        } else {
                            break;
                        }
                    }
                }
                // reference lines, overlays and the categorical views only arrange charts
                "DISTRIBUTION" if key(p, "SCALE")? || key(p, "CATEGORICAL")? => skip_value(p),
                "SCATTERPLOT" | "BOXPLOT" if key(p, "DISPLAY")? => {
                    let on = yes_no(p)?;
                    match name.as_str() {
                        "SCATTERPLOT" => run.scatter = on,
                        _ => run.boxplot = on,
                    }
                }
                "SCATTERPLOT" if key(p, "ROWSIZE")? => skip_value(p),
                "BOXPLOT" if key(p, "OVERLAYTARGETS")? => skip_value(p),
                "TORNADO" if key(p, "DISPLAY")? => {
                    if !yes_no(p)? {
                        run.tornado_corr = false;
                        run.tornado_delta = None;
                        run.tornado_variance = false;
                        skip_value(p);
                    }
                }
                "TORNADO" if key(p, "TYPE")? => {
                    run.tornado_corr = false;
                    loop {
                        if p.eat_word("CORR") {
                            run.tornado_corr = true;
                        } else if p.eat_word("VARIANCE") {
                            run.tornado_variance = true;
                        } else if p.eat_word("DELTASTDDEV") {
                            p.expect(Punct::LParen)?;
                            let k = number(p, "DELTASTDDEV", f64::MIN_POSITIVE, 10.0)?;
                            p.expect(Punct::RParen)?;
                            run.tornado_delta = Some(k);
                        } else {
                            break;
                        }
                    }
                }
                "PRINT" if key(p, "DESCRIPTIVES")? => {
                    run.descriptives = yes_no(p)?.then_some(95.0);
                    if run.descriptives.is_some() && p.eat(Punct::LParen) {
                        if !key(p, "CILEVEL")? {
                            return Err(p.unexpected(&["CILEVEL"]));
                        }
                        run.descriptives = Some(number(p, "CILEVEL", 50.0, 99.9999)?);
                        p.expect(Punct::RParen)?;
                    }
                }
                "PRINT" if key(p, "PERCENTILES")? => {
                    run.percentiles = yes_no(p)?;
                    if run.percentiles && p.eat(Punct::LParen) {
                        while !p.eat(Punct::RParen) {
                            if key(p, "NTILES")? {
                                run.ntiles = Some(whole(p, "NTILES", 2.0, 100.0)?);
                            } else if key(p, "PTILES")? {
                                run.ptiles = numbers(p);
                                if run.ptiles.iter().any(|v| !(0.0..=100.0).contains(v)) {
                                    return Err(SyntaxError::new(
                                        p.span(),
                                        "PTILES are percentages from 0 to 100",
                                    ));
                                }
                            } else {
                                return Err(p.unexpected(&["NTILES", "PTILES"]));
                            }
                        }
                    }
                }
                "PRINT" if key(p, "ASSOCIATIONS")? => run.associations = yes_no(p)?,
                "OPTIONS" if key(p, "VIZSTYLESHEET")? => skip_value(p),
                "OUTFILE" if key(p, "FILE")? => {
                    run.outfile = Some(match p.eat(Punct::Star) {
                        true => "*".into(),
                        false => p.file("a quoted file or a dataset name")?,
                    });
                }
                "PLAN" | "CRITERIA" | "DISTRIBUTION" | "SCATTERPLOT" | "BOXPLOT" | "TORNADO"
                | "PRINT" | "OPTIONS" | "OUTFILE" => {
                    return Err(p.unexpected(&["a keyword of the subcommand"]));
                }
                _ => {
                    return Err(p.subcommand_error(
                        "SIMRUN",
                        &[
                            "PLAN",
                            "CRITERIA",
                            "DISTRIBUTION",
                            "SCATTERPLOT",
                            "BOXPLOT",
                            "TORNADO",
                            "PRINT",
                            "OPTIONS",
                            "OUTFILE",
                        ],
                    ));
                }
            }
        }
    }
    p.finish(&["'/'"])?;
    if run.plan.is_empty() {
        return Err(SyntaxError::new(p.span(), "SIMRUN needs /PLAN FILE='file'"));
    }
    Ok(Command::Simrun(Box::new(run)))
}

/// The value of a display keyword that does not change any number: up to the next keyword.
fn skip_value(p: &mut Parser) {
    let mut depth = 0usize;
    while !at_end(p) || depth > 0 {
        if depth == 0
            && matches!(p.peek(), Some(TokenKind::Word(_)))
            && matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)))
        {
            return;
        }
        match p.peek() {
            Some(TokenKind::Punct(Punct::LParen | Punct::LBracket)) => depth += 1,
            Some(TokenKind::Punct(Punct::RParen | Punct::RBracket)) => {
                depth = depth.saturating_sub(1)
            }
            None => return,
            _ => {}
        }
        p.pos += 1;
    }
}

pub(crate) fn begin_expr(p: &mut Parser) -> PResult<Command> {
    p.expect(Punct::Slash)?;
    if !p.eat_word("OUTFILE") {
        return Err(p.unexpected(&["OUTFILE"]));
    }
    if !key(p, "PREPXML")? {
        return Err(p.unexpected(&["PREPXML"]));
    }
    let file = p.file("a quoted file")?;
    p.finish(&[])?;
    Ok(Command::BeginExpr(file))
}

pub(crate) fn end_expr(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndExpr)
}
