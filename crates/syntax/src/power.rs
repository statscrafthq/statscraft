//! Parser of the POWER commands for means and proportions: the power of a planned t-test or
//! binomial test for given sample sizes, or the sample sizes that reach given powers.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - Every command has a PARAMETERS subcommand and an optional PLOT; either order.
//! - Common keywords: TEST=NONDIRECTIONAL (default, two-sided) or DIRECTIONAL; SIGNIFICANCE
//!   strictly between 0 and 1, 0.05 by default; the sample size (N or NPAIRS) or POWER, never
//!   both and never neither.
//! - POWER takes one or more values strictly between 0 and 1 and at most one grid
//!   `low TO high BY step` (low <= high, step from 0.01 up to 1, equal ends give one value),
//!   in any place in the list. Repeated values are used once, with a warning.
//! - MEANS ONESAMPLE: N (whole, above 1), SD (above 0), MEAN, NULL (0). MEANS INDEPENDENT:
//!   N (two whole numbers above 1), NRATIO (n2 / n1, from 0.01 to 100, default 1, used only
//!   with POWER), SD (one common value or one per group, above 0) and MEAN (the difference, or
//!   one per group). MEANS RELATED: NPAIRS (whole, above 1), SD (of the differences, or one per
//!   member of the pair), RHO (the correlation that turns two SDs into one, from -1 to 1 but
//!   not 0, default 0.5, not 1 when the two SDs are equal) and MEAN.
//! - PROPORTIONS ONESAMPLE: N (whole, at least 1), NULL (0.5) and ALTERNATIVE, both strictly
//!   between 0 and 1 and different with POWER; ESTIMATE=NORMAL (default) or BINOMIAL; TIMER
//!   (whole minutes, for the BINOMIAL sample size search); CONTINUITY=FALSE (default) or TRUE.
//! - PROPORTIONS INDEPENDENT: N (two whole numbers, at least 1, more than 2 together with
//!   METHOD=T), NRATIO (strictly between 0.01 and 100), PROPORTIONS (two, strictly between 0
//!   and 1, different with POWER), METHOD=CHISQ (default), T, LRT or FISHER, ESTIMATE, TIMER,
//!   CONTINUITY and POOLED=TRUE (default) or FALSE.
//! - PROPORTIONS RELATED: NPAIRS (whole, at least 2), VALUES=PROPORTIONS(a b) or COUNTS(a b)
//!   (counts only with NPAIRS), MARGINAL=FALSE (default: the values are the two discordant
//!   cells) or TRUE (the two marginal success rates), CORRELATION strictly between -1 and 1,
//!   used with MARGINAL=TRUE.
//! - PLOT names charts: a keyword alone gives power against that quantity, with an optional
//!   `(low high)` range where the command allows one; an `_XAXIS` keyword and the matching
//!   `_YAXIS` keyword together give a surface of power over both. VROTATE and HROTATE take a
//!   whole number of degrees from 0 to 359.
//!
//! Choices where the manual is silent: CORRELATION may be left out and is then 0.5, the value
//! the Algorithms manual names as the default; a lone `_XAXIS` or `_YAXIS` keyword is an error;
//! PROPORTIONS and VALUES may be written with or without parentheses.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

/// Which of the POWER commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerKind {
    MeansOneSample,
    MeansIndependent,
    MeansRelated,
    ProportionsOneSample,
    ProportionsIndependent,
    ProportionsRelated,
}

impl PowerKind {
    pub fn name(self) -> &'static str {
        match self {
            PowerKind::MeansOneSample => "POWER MEANS ONESAMPLE",
            PowerKind::MeansIndependent => "POWER MEANS INDEPENDENT",
            PowerKind::MeansRelated => "POWER MEANS RELATED",
            PowerKind::ProportionsOneSample => "POWER PROPORTIONS ONESAMPLE",
            PowerKind::ProportionsIndependent => "POWER PROPORTIONS INDEPENDENT",
            PowerKind::ProportionsRelated => "POWER PROPORTIONS RELATED",
        }
    }
}

/// The test the power of POWER PROPORTIONS INDEPENDENT is for (METHOD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProportionTest {
    ChiSquare,
    T,
    LikelihoodRatio,
    Fisher,
}

/// One requested chart. Axes are named by their keyword without `_XAXIS` or `_YAXIS`.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerPlot {
    /// The quantity across: for a line chart the only one; DISCORDANT and MARGINAL name
    /// surfaces over two proportions.
    pub x: String,
    pub x_range: Option<(f64, f64)>,
    /// A surface: the second quantity.
    pub y: Option<String>,
    pub y_range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Power {
    pub kind: PowerKind,
    /// TEST=DIRECTIONAL: a one-sided test.
    pub directional: bool,
    pub significance: f64,
    /// The powers to find sample sizes for; empty when the sample size is given.
    pub power: Vec<f64>,
    /// Powers given more than once, dropped from `power`.
    pub repeated: Vec<f64>,
    /// N or NPAIRS: one size, or two for the independent groups; empty with POWER.
    pub n: Vec<f64>,
    /// NRATIO: n2 / n1 of the sample sizes to find.
    pub ratio: f64,
    pub sd: Vec<f64>,
    pub mean: Vec<f64>,
    /// NULL; None for the default (0 for a mean, 0.5 for a proportion).
    pub null: Option<f64>,
    pub alternative: Option<f64>,
    /// RHO of MEANS RELATED.
    pub rho: f64,
    /// PROPORTIONS of INDEPENDENT, or the values of VALUES of RELATED.
    pub proportions: Vec<f64>,
    /// VALUES=COUNTS.
    pub counts: bool,
    pub marginal: bool,
    pub correlation: f64,
    pub method: ProportionTest,
    /// ESTIMATE=BINOMIAL: binomial enumeration instead of the normal approximation.
    pub binomial: bool,
    /// TIMER in minutes.
    pub timer: Option<f64>,
    pub continuity: bool,
    pub pooled: bool,
    pub plots: Vec<PowerPlot>,
}

/// A PLOT keyword and how it takes a range.
type Keyword = (&'static str, Range);
/// A chart axis as given: its keyword and range.
type Given = Option<(String, Option<(f64, f64)>)>;

/// How a PLOT keyword takes a range.
#[derive(Clone, Copy, PartialEq)]
enum Range {
    None,
    Whole,
    Real,
}

/// The line charts and the axes of the surfaces a command allows.
fn plot_keywords(kind: PowerKind) -> (&'static [Keyword], &'static [[Keyword; 2]]) {
    use Range::*;
    match kind {
        PowerKind::MeansOneSample => (
            &[("N", Whole), ("ES", Real)],
            &[[("N", Whole), ("ES", Real)]],
        ),
        PowerKind::MeansIndependent => (
            &[("NRATIO", Real), ("ES", Real)],
            &[[("NRATIO", Real), ("ES", Real)]],
        ),
        PowerKind::MeansRelated => (
            &[("NPAIRS", Whole), ("ES", Real)],
            &[[("NPAIRS", Whole), ("ES", Real)]],
        ),
        PowerKind::ProportionsOneSample => (
            &[
                ("N", Whole),
                ("NULL", None),
                ("ALTERNATIVE", None),
                ("DIFF", None),
            ],
            &[
                [("N", Whole), ("DIFF", None)],
                [("NULL", None), ("ALTERNATIVE", None)],
            ],
        ),
        PowerKind::ProportionsIndependent => (
            &[
                ("NRATIO", Real),
                ("RISK_DIFF", None),
                ("RISK_RATIO", Real),
                ("ODDS_RATIO", Real),
            ],
            &[[("N1", Whole), ("N2", Whole)], [("P1", None), ("P2", None)]],
        ),
        PowerKind::ProportionsRelated => (
            &[
                ("NPAIRS", Whole),
                ("RISK_DIFF", None),
                ("RISK_RATIO", Real),
                ("ODDS_RATIO", Real),
                ("CORRELATION", None),
                ("DISCORDANT", None),
                ("MARGINAL", None),
            ],
            &[],
        ),
    }
}

/// The keyword of `options` that the next word names: written in full first, else the first
/// abbreviation in list order (shorter keywords come first, so NUL is NULL, not NULL_XAXIS).
fn pick(p: &Parser, options: &[String]) -> Option<usize> {
    let Some(TokenKind::Word(w)) = p.peek() else {
        return None;
    };
    options
        .iter()
        .position(|k| w.eq_ignore_ascii_case(k))
        .or_else(|| options.iter().position(|k| matches_keyword(w, k)))
}

fn number(p: &mut Parser, what: &str) -> PResult<(f64, usize)> {
    let start = p.pos;
    match p.number() {
        Some(value) => Ok((value, start)),
        None => Err(p.unexpected(&[what])),
    }
}

fn fail<T>(p: &Parser, start: usize, message: String) -> PResult<T> {
    Err(SyntaxError::new(p.span_from(start), message))
}

/// The numbers up to the next keyword or the end of the subcommand, in optional parentheses.
fn numbers(p: &mut Parser, keyword: &str, low: usize, high: usize) -> PResult<Vec<f64>> {
    let start = p.pos;
    let paren = p.eat(Punct::LParen);
    let mut values = Vec::new();
    while let Some(value) = p.number() {
        values.push(value);
        p.eat(Punct::Comma);
    }
    if paren {
        p.expect(Punct::RParen)?;
    }
    if values.len() < low || values.len() > high {
        let count = if low == high {
            format!("{low}")
        } else {
            format!("{low} or {high}")
        };
        return fail(
            p,
            start,
            format!("{keyword} takes {count} values, got {}", values.len()),
        );
    }
    Ok(values)
}

fn whole(p: &Parser, start: usize, keyword: &str, values: &[f64], least: f64) -> PResult<()> {
    for &v in values {
        if v.fract() != 0.0 || v < least {
            return fail(
                p,
                start,
                format!("{keyword} must be a whole number of at least {least}, got {v}"),
            );
        }
    }
    Ok(())
}

fn inside(p: &Parser, start: usize, keyword: &str, v: f64, low: f64, high: f64) -> PResult<()> {
    if v > low && v < high {
        Ok(())
    } else {
        fail(
            p,
            start,
            format!("{keyword} must lie strictly between {low} and {high}, got {v}"),
        )
    }
}

fn boolean(p: &mut Parser, keyword: &str) -> PResult<bool> {
    if p.eat_keyword("TRUE") {
        Ok(true)
    } else if p.eat_keyword("FALSE") {
        Ok(false)
    } else {
        Err(p
            .unexpected(&["TRUE", "FALSE"])
            .hint(format!("{keyword} is TRUE or FALSE")))
    }
}

/// POWER: values and at most one grid `low TO high BY step`.
fn powers(p: &mut Parser, command: &mut Power) -> PResult<()> {
    let mut values = Vec::new();
    let mut grid = false;
    while let Some((value, at)) = p.number().map(|v| (v, p.pos - 1)) {
        if p.eat_word("TO") {
            if grid {
                return fail(
                    p,
                    at,
                    "POWER takes one grid low TO high BY step at most".into(),
                );
            }
            grid = true;
            let (high, _) = number(p, "the end of the grid")?;
            if !p.eat_word("BY") {
                return Err(p
                    .unexpected(&["BY"])
                    .hint("a POWER grid is written low TO high BY step"));
            }
            let (step, _) = number(p, "the step of the grid")?;
            if !(value > 0.0 && value <= high && high < 1.0) {
                return fail(
                    p,
                    at,
                    format!("a POWER grid needs 0 < low <= high < 1, got {value} TO {high}"),
                );
            }
            if !(0.01..1.0).contains(&step) {
                return fail(
                    p,
                    at,
                    format!("the step of a POWER grid must lie from 0.01 up to 1, got {step}"),
                );
            }
            let mut k = 0.0;
            loop {
                // rounded so that 0.1 steps give 0.3, not 0.30000000000000004
                let v = ((value + k * step) * 1e10).round() / 1e10;
                if v > high + 1e-12 {
                    break;
                }
                values.push(v);
                k += 1.0;
            }
        } else {
            if !(value > 0.0 && value < 1.0) {
                return fail(
                    p,
                    at,
                    format!("POWER values must lie strictly between 0 and 1, got {value}"),
                );
            }
            values.push(value);
        }
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a power between 0 and 1"]));
    }
    for v in values {
        if command.power.contains(&v) {
            if !command.repeated.contains(&v) {
                command.repeated.push(v);
            }
        } else {
            command.power.push(v);
        }
    }
    Ok(())
}

const COMMON: [&str; 3] = ["TEST", "SIGNIFICANCE", "POWER"];

fn parameter_keywords(kind: PowerKind) -> &'static [&'static str] {
    match kind {
        PowerKind::MeansOneSample => &["N", "SD", "MEAN", "NULL"],
        PowerKind::MeansIndependent => &["N", "NRATIO", "SD", "MEAN"],
        PowerKind::MeansRelated => &["NPAIRS", "SD", "RHO", "MEAN"],
        PowerKind::ProportionsOneSample => &[
            "N",
            "NULL",
            "ALTERNATIVE",
            "ESTIMATE",
            "TIMER",
            "CONTINUITY",
        ],
        PowerKind::ProportionsIndependent => &[
            "N",
            "NRATIO",
            "PROPORTIONS",
            "METHOD",
            "ESTIMATE",
            "TIMER",
            "CONTINUITY",
            "POOLED",
        ],
        PowerKind::ProportionsRelated => &[
            "NPAIRS",
            "VALUES",
            "MARGINAL",
            "CORRELATION",
            "ESTIMATE",
            "TIMER",
        ],
    }
}

fn parameters(p: &mut Parser, c: &mut Power, seen: &mut Vec<&'static str>) -> PResult<()> {
    let mut keywords: Vec<&'static str> = parameter_keywords(c.kind).to_vec();
    keywords.extend(COMMON);
    // short keywords first, so that an abbreviation prefers N over NULL and so on
    keywords.sort_by_key(|k| k.len());
    let owned: Vec<String> = keywords.iter().map(|k| k.to_string()).collect();
    let proportions = matches!(
        c.kind,
        PowerKind::ProportionsOneSample
            | PowerKind::ProportionsIndependent
            | PowerKind::ProportionsRelated
    );
    while !at_end(p) {
        let Some(i) = pick(p, &owned) else {
            return Err(p.unexpected(&keywords));
        };
        let keyword = keywords[i];
        p.pos += 1;
        p.eat(Punct::Eq);
        let start = p.pos;
        seen.push(keyword);
        match keyword {
            "TEST" => {
                c.directional = if p.eat_keyword("NONDIRECTIONAL") {
                    false
                } else if p.eat_keyword("DIRECTIONAL") {
                    true
                } else {
                    return Err(p.unexpected(&["NONDIRECTIONAL", "DIRECTIONAL"]));
                }
            }
            "SIGNIFICANCE" => {
                let (v, at) = number(p, "a significance level")?;
                inside(p, at, keyword, v, 0.0, 1.0)?;
                c.significance = v;
            }
            "POWER" => powers(p, c)?,
            "N" | "NPAIRS" => {
                let two = matches!(
                    c.kind,
                    PowerKind::MeansIndependent | PowerKind::ProportionsIndependent
                );
                let count = if two { 2 } else { 1 };
                let values = numbers(p, keyword, count, count)?;
                let least = match c.kind {
                    PowerKind::ProportionsOneSample | PowerKind::ProportionsIndependent => 1.0,
                    _ => 2.0,
                };
                whole(p, start, keyword, &values, least)?;
                c.n = values;
            }
            "NRATIO" => {
                let (v, at) = number(p, "a ratio")?;
                let open = c.kind == PowerKind::ProportionsIndependent;
                if (open && !(v > 0.01 && v < 100.0)) || !(0.01..=100.0).contains(&v) {
                    return fail(p, at, format!("NRATIO must lie from 0.01 to 100, got {v}"));
                }
                c.ratio = v;
            }
            "SD" => {
                let one = c.kind == PowerKind::MeansOneSample;
                let values = numbers(p, keyword, 1, if one { 1 } else { 2 })?;
                if let Some(v) = values.iter().find(|v| **v <= 0.0) {
                    return fail(p, start, format!("SD must be above 0, got {v}"));
                }
                c.sd = values;
            }
            "MEAN" => {
                let one = c.kind == PowerKind::MeansOneSample;
                c.mean = numbers(p, keyword, 1, if one { 1 } else { 2 })?;
            }
            "NULL" => {
                let (v, at) = number(p, "a null hypothesis value")?;
                if proportions {
                    inside(p, at, keyword, v, 0.0, 1.0)?;
                }
                c.null = Some(v);
            }
            "ALTERNATIVE" => {
                let (v, at) = number(p, "an alternative hypothesis value")?;
                inside(p, at, keyword, v, 0.0, 1.0)?;
                c.alternative = Some(v);
            }
            "RHO" => {
                let (v, at) = number(p, "a correlation")?;
                if !(-1.0..=1.0).contains(&v) || v == 0.0 {
                    return fail(
                        p,
                        at,
                        format!("RHO must lie from -1 to 1 and not be 0, got {v}"),
                    );
                }
                c.rho = v;
            }
            "PROPORTIONS" => {
                c.proportions = numbers(p, keyword, 2, 2)?;
                for &v in &c.proportions {
                    inside(p, start, keyword, v, 0.0, 1.0)?;
                }
            }
            "VALUES" => {
                if p.eat_keyword("PROPORTIONS") {
                    c.counts = false;
                    let at = p.pos;
                    c.proportions = numbers(p, "PROPORTIONS", 2, 2)?;
                    for &v in &c.proportions {
                        if !(0.0..=1.0).contains(&v) {
                            return fail(
                                p,
                                at,
                                format!("PROPORTIONS must lie from 0 to 1, got {v}"),
                            );
                        }
                    }
                } else if p.eat_keyword("COUNTS") {
                    c.counts = true;
                    let at = p.pos;
                    c.proportions = numbers(p, "COUNTS", 2, 2)?;
                    whole(p, at, "COUNTS", &c.proportions, 0.0)?;
                } else {
                    return Err(p.unexpected(&["PROPORTIONS", "COUNTS"]));
                }
            }
            "MARGINAL" => c.marginal = boolean(p, keyword)?,
            "CORRELATION" => {
                let (v, at) = number(p, "a correlation")?;
                inside(p, at, keyword, v, -1.0, 1.0)?;
                c.correlation = v;
            }
            "METHOD" => {
                c.method = if p.eat_keyword("CHISQ") {
                    ProportionTest::ChiSquare
                } else if p.eat_word("T") {
                    ProportionTest::T
                } else if p.eat_keyword("LRT") {
                    ProportionTest::LikelihoodRatio
                } else if p.eat_keyword("FISHER") {
                    ProportionTest::Fisher
                } else {
                    return Err(p.unexpected(&["CHISQ", "T", "LRT", "FISHER"]));
                }
            }
            "ESTIMATE" => {
                c.binomial = if p.eat_keyword("NORMAL") {
                    false
                } else if p.eat_keyword("BINOMIAL") {
                    true
                } else {
                    return Err(p.unexpected(&["NORMAL", "BINOMIAL"]));
                }
            }
            "TIMER" => {
                let (v, at) = number(p, "a number of minutes")?;
                whole(p, at, keyword, &[v], 1.0)?;
                c.timer = Some(v);
            }
            "CONTINUITY" => c.continuity = boolean(p, keyword)?,
            _ => c.pooled = boolean(p, keyword)?,
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

/// `(low high)` after a PLOT keyword, if any.
fn plot_range(p: &mut Parser, keyword: &str, range: Range) -> PResult<Option<(f64, f64)>> {
    if range == Range::None || !p.at(Punct::LParen) {
        return Ok(None);
    }
    let start = p.pos;
    let values = numbers(p, keyword, 2, 2)?;
    if range == Range::Whole {
        whole(p, start + 1, keyword, &values, 0.0)?;
    }
    if values[0] >= values[1] {
        return fail(
            p,
            start,
            format!(
                "the range of {keyword} must run upwards, got {} to {}",
                values[0], values[1]
            ),
        );
    }
    Ok(Some((values[0], values[1])))
}

fn plot(p: &mut Parser, c: &mut Power) -> PResult<()> {
    let (lines, surfaces) = plot_keywords(c.kind);
    let mut options: Vec<(String, usize, Range)> = Vec::new();
    for (k, r) in lines {
        options.push((k.to_string(), usize::MAX, *r));
    }
    for (i, pair) in surfaces.iter().enumerate() {
        for (k, r) in pair {
            options.push((format!("{k}_XAXIS"), i, *r));
            options.push((format!("{k}_YAXIS"), i, *r));
        }
    }
    options.push(("VROTATE".into(), usize::MAX, Range::None));
    options.push(("HROTATE".into(), usize::MAX, Range::None));
    options.sort_by_key(|o| o.0.len());
    let names: Vec<String> = options.iter().map(|o| o.0.clone()).collect();
    // per surface: (name, range) of the x and the y axis, and where it was named
    let mut axes: Vec<[Given; 2]> = vec![[None, None]; surfaces.len()];
    let mut at: Vec<usize> = vec![0; surfaces.len()];
    while !at_end(p) {
        let Some(i) = pick(p, &names) else {
            let expected: Vec<&str> = names.iter().map(String::as_str).collect();
            return Err(p.unexpected(&expected));
        };
        let start = p.pos;
        p.pos += 1;
        let (name, surface, range) = options[i].clone();
        if name == "VROTATE" || name == "HROTATE" {
            p.expect(Punct::LParen)?;
            let (v, at) = number(p, "degrees")?;
            if v.fract() != 0.0 || !(0.0..=359.0).contains(&v) {
                return fail(
                    p,
                    at,
                    format!("{name} takes whole degrees from 0 to 359, got {v}"),
                );
            }
            p.expect(Punct::RParen)?;
        } else if surface == usize::MAX {
            let range = plot_range(p, &name, range)?;
            c.plots.push(PowerPlot {
                x: name,
                x_range: range,
                y: None,
                y_range: None,
            });
        } else {
            let (base, side) = name.split_at(name.len() - 6);
            let range = plot_range(p, base, range)?;
            let slot = usize::from(side == "_YAXIS");
            if axes[surface][slot].is_some() {
                return fail(p, start, format!("{name} is given twice"));
            }
            axes[surface][slot] = Some((base.to_string(), range));
            at[surface] = start;
        }
        p.eat(Punct::Comma);
    }
    for (surface, [x, y]) in axes.into_iter().enumerate() {
        match (x, y) {
            (Some((x, x_range)), Some((y, y_range))) if x != y => c.plots.push(PowerPlot {
                x,
                x_range,
                y: Some(y),
                y_range,
            }),
            (None, None) => {}
            (x, y) => {
                let [a, b] = &surfaces[surface];
                let message = match (x, y) {
                    (Some((x, _)), Some(_)) => {
                        format!("{x}_XAXIS and {x}_YAXIS cannot form one chart")
                    }
                    _ => format!(
                        "a surface chart needs both axes: {}_XAXIS with {}_YAXIS, or {}_YAXIS with {}_XAXIS",
                        a.0, b.0, a.0, b.0
                    ),
                };
                return fail(p, at[surface], message);
            }
        }
    }
    Ok(())
}

fn power(p: &mut Parser, kind: PowerKind) -> PResult<Command> {
    let mut c = Power {
        kind,
        directional: false,
        significance: 0.05,
        power: Vec::new(),
        repeated: Vec::new(),
        n: Vec::new(),
        ratio: 1.0,
        sd: Vec::new(),
        mean: Vec::new(),
        null: None,
        alternative: None,
        rho: 0.5,
        proportions: Vec::new(),
        counts: false,
        marginal: false,
        correlation: 0.5,
        method: ProportionTest::ChiSquare,
        binomial: false,
        timer: None,
        continuity: false,
        pooled: true,
        plots: Vec::new(),
    };
    let mut seen = Vec::new();
    let mut had_parameters = false;
    let start = p.pos;
    while p.eat(Punct::Slash) {
        if p.eat_keyword("PARAMETERS") {
            had_parameters = true;
            parameters(p, &mut c, &mut seen)?;
        } else if p.eat_keyword("PLOT") {
            plot(p, &mut c)?;
        } else {
            return Err(p.subcommand_error(kind.name(), &[]));
        }
    }
    p.finish(&["'/'"])?;
    let name = kind.name();
    if !had_parameters {
        return fail(p, start, format!("{name} needs a PARAMETERS subcommand"));
    }
    let has = |k: &str| seen.contains(&k);
    let size = if has("NPAIRS") { "NPAIRS" } else { "N" };
    let size_word = match kind {
        PowerKind::MeansRelated | PowerKind::ProportionsRelated => "NPAIRS",
        _ => "N",
    };
    if !c.n.is_empty() && !c.power.is_empty() {
        return fail(
            p,
            start,
            format!("{name}: give either {size} or POWER, not both"),
        );
    }
    if c.n.is_empty() && c.power.is_empty() {
        return fail(
            p,
            start,
            format!(
                "{name} needs {size_word} (to find the power) or POWER (to find the sample size)"
            ),
        );
    }
    let required: &[&str] = match kind {
        PowerKind::MeansOneSample | PowerKind::MeansIndependent | PowerKind::MeansRelated => {
            &["SD", "MEAN"]
        }
        PowerKind::ProportionsOneSample => &["ALTERNATIVE"],
        PowerKind::ProportionsIndependent => &["PROPORTIONS"],
        PowerKind::ProportionsRelated => &["VALUES"],
    };
    for k in required {
        if !has(k) {
            return fail(p, start, format!("{name} needs {k} in PARAMETERS"));
        }
    }
    let finding_size = !c.power.is_empty();
    match kind {
        PowerKind::MeansRelated => {
            if c.sd.len() == 2 && c.sd[0] == c.sd[1] && c.rho == 1.0 {
                return fail(
                    p,
                    start,
                    "RHO cannot be 1 when both SD values are equal: the differences would not vary"
                        .into(),
                );
            }
        }
        PowerKind::ProportionsOneSample => {
            if finding_size && c.alternative == Some(c.null.unwrap_or(0.5)) {
                return fail(
                    p,
                    start,
                    "to find a sample size, ALTERNATIVE must differ from NULL".into(),
                );
            }
        }
        PowerKind::ProportionsIndependent => {
            if finding_size && c.proportions[0] == c.proportions[1] {
                return fail(
                    p,
                    start,
                    "to find a sample size, the two PROPORTIONS must differ".into(),
                );
            }
            if c.method == ProportionTest::T && c.n.len() == 2 && c.n[0] + c.n[1] <= 2.0 {
                return fail(
                    p,
                    start,
                    "METHOD=T needs more than 2 trials in the two groups together".into(),
                );
            }
        }
        PowerKind::ProportionsRelated => {
            if c.counts && finding_size {
                return fail(
                    p,
                    start,
                    "VALUES=COUNTS needs NPAIRS; to find a sample size give PROPORTIONS".into(),
                );
            }
            if c.counts && c.proportions[0] + c.proportions[1] > c.n[0] {
                return fail(p, start, "the two COUNTS add up to more than NPAIRS".into());
            }
        }
        _ => {}
    }
    Ok(Command::Power(Box::new(c)))
}

pub(crate) fn means_onesample(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::MeansOneSample)
}

pub(crate) fn means_independent(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::MeansIndependent)
}

pub(crate) fn means_related(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::MeansRelated)
}

pub(crate) fn proportions_onesample(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::ProportionsOneSample)
}

pub(crate) fn proportions_independent(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::ProportionsIndependent)
}

pub(crate) fn proportions_related(p: &mut Parser) -> PResult<Command> {
    power(p, PowerKind::ProportionsRelated)
}
