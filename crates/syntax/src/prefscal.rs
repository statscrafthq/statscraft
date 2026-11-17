//! Parser of PREFSCAL (multidimensional unfolding with penalized stress).
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The variable list (optionally after VARIABLES=) names the column objects; every case is a
//!   row of one source.
//! - INPUT=ROWS(n | rowid) SOURCES(n | sourceid): how many rows a source has, how many sources
//!   there are, or the variables that number them.
//! - PROXIMITIES=DISSIMILARITIES (default) or SIMILARITIES. WEIGHTS=varlist, one per variable.
//! - INITIAL=CLASSICAL[(TRIANGLE | SPEARMAN)] (default TRIANGLE), CORRESPONDENCE, ROSSCLIFF,
//!   CENTROIDS[(n)] or RANDOM[(n)]; a start from a file is not supported yet.
//! - CONDITION=ROW (default), MATRIX or UNCONDITIONAL.
//! - TRANSFORMATION=NONE[(INTERCEPT)], LINEAR[(INTERCEPT)], ORDINAL[(KEEPTIES | UNTIE)]
//!   (default ORDINAL with ties kept), SMOOTH (not supported yet) or
//!   SPLINE[([INTERCEPT] [ORDER | DEGREE=2] [INKNOT=1])].
//! - MODEL=IDENTITY (default), WEIGHTED or GENERALIZED. RESTRICTIONS is not supported yet.
//! - PENALTY=LAMBDA(v) (0 < v <= 1) OMEGA(v) (v >= 0).
//! - CRITERIA=DIMENSIONS(min[,max]) (2) MAXITER(n) (5000) DIFFSTRESS(v) (0.000001)
//!   MINSTRESS(v) (0.0001).
//! - PRINT: MEASURES, COMMON and WEIGHTS without the subcommand or keywords; otherwise just
//!   the keywords named. PLOT: COMMON and WEIGHTS by default; INDIVIDUAL, TRANSFORMATIONS,
//!   SHEPARD, FIT and RESIDUALS take value lists in parentheses, one plot per list.
//! - OPTIONS=MARKERS(var) COLORS(var). OUTFILE=COMMON WEIGHTS DISTANCES TRANSFORMATIONS.
//! - A later subcommand replaces an earlier one.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::proxscal::{ProxscalOutfile, ScalingModel, out_target, whole, whole_in_parens};

/// A count or the variable that numbers the rows or sources.
#[derive(Debug, Clone, PartialEq)]
pub enum InputCount {
    Number(usize),
    Variable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefscalStart {
    /// Classical scaling of the completed matrix; `spearman` fills it with rank distances.
    Classical {
        spearman: bool,
    },
    Correspondence,
    RossCliff,
    /// The columns at the centroid of the rows that name them among their first n choices.
    Centroids(usize),
    Random(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefscalCondition {
    Row,
    Matrix,
    Unconditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefscalTransformation {
    None {
        intercept: bool,
    },
    Linear {
        intercept: bool,
    },
    Ordinal {
        untie: bool,
    },
    Spline {
        intercept: bool,
        degree: usize,
        inknot: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrefscalPrint {
    pub input: bool,
    pub multiple: bool,
    pub initial: bool,
    pub history: bool,
    pub measures: bool,
    pub decomposition: bool,
    pub common: bool,
    pub distances: bool,
    pub weights: bool,
    pub individual: bool,
    pub transformations: bool,
}

/// A list of values and ranges `lo TO hi` in parentheses; empty for ALL.
pub type ValueList = Vec<(f64, f64)>;

/// Plots; the lists are None when not asked for, one plot per list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrefscalPlot {
    pub multiple: bool,
    pub initial: bool,
    pub stress: bool,
    pub common: bool,
    pub weights: bool,
    pub individual: Option<Vec<ValueList>>,
    pub transformations: Option<Vec<ValueList>>,
    pub shepard: Option<Vec<ValueList>>,
    pub fit: Option<Vec<ValueList>>,
    pub residuals: Option<Vec<ValueList>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prefscal {
    pub variables: Vec<VarItem>,
    pub rows: Option<InputCount>,
    pub sources: Option<InputCount>,
    pub similarities: bool,
    pub weights: Vec<VarItem>,
    pub initial: PrefscalStart,
    pub condition: PrefscalCondition,
    pub transformation: PrefscalTransformation,
    pub model: ScalingModel,
    pub lambda: f64,
    pub omega: f64,
    pub dimensions: (usize, usize),
    pub max_iterations: usize,
    pub diff_stress: f64,
    pub min_stress: f64,
    pub print: PrefscalPrint,
    pub plot: PrefscalPlot,
    pub markers: Option<String>,
    pub colors: Option<String>,
    pub outfile: ProxscalOutfile,
}

/// A number in parentheses that must lie in the range `ok` describes.
fn value(p: &mut Parser, what: &str, ok: fn(f64) -> bool, range: &str) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let Some(v) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    if !ok(v) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be {range}, got {v}"),
        ));
    }
    p.expect(Punct::RParen)?;
    Ok(v)
}

/// A count or a variable name in parentheses.
fn count(p: &mut Parser, what: &str) -> PResult<InputCount> {
    p.expect(Punct::LParen)?;
    let out = match p.peek() {
        Some(TokenKind::Word(_)) => InputCount::Variable(p.name("a variable name")?),
        _ => InputCount::Number(whole(p, what, 1)?),
    };
    p.expect(Punct::RParen)?;
    Ok(out)
}

/// The value lists after a plot keyword; one list of ALL without parentheses.
fn value_lists(p: &mut Parser) -> PResult<Vec<ValueList>> {
    let mut lists = Vec::new();
    while p.eat(Punct::LParen) {
        let (mut list, mut all) = (ValueList::new(), false);
        while !p.eat(Punct::RParen) {
            if p.eat_word("ALL") {
                all = true;
            } else {
                let Some(lo) = p.number() else {
                    return Err(p.unexpected(&["a value", "ALL", "')'"]));
                };
                let hi = match p.eat_word("TO") {
                    true => p.number().ok_or_else(|| p.unexpected(&["a value"]))?,
                    false => lo,
                };
                list.push((lo.min(hi), lo.max(hi)));
            }
            p.eat(Punct::Comma);
        }
        lists.push(if all { ValueList::new() } else { list });
    }
    if lists.is_empty() {
        lists.push(ValueList::new());
    }
    Ok(lists)
}

fn transformation(p: &mut Parser) -> PResult<PrefscalTransformation> {
    let intercept_only = |p: &mut Parser| -> PResult<bool> {
        if !p.eat(Punct::LParen) {
            return Ok(false);
        }
        if !p.eat_keyword("INTERCEPT") {
            return Err(p.unexpected(&["INTERCEPT"]));
        }
        p.expect(Punct::RParen)?;
        Ok(true)
    };
    if p.eat_keyword("NONE") {
        Ok(PrefscalTransformation::None {
            intercept: intercept_only(p)?,
        })
    } else if p.eat_keyword("LINEAR") {
        Ok(PrefscalTransformation::Linear {
            intercept: intercept_only(p)?,
        })
    } else if p.eat_keyword("ORDINAL") {
        let mut untie = false;
        if p.eat(Punct::LParen) {
            if p.eat_keyword("UNTIE") {
                untie = true;
            } else if !p.eat_keyword("KEEPTIES") {
                return Err(p.unexpected(&["KEEPTIES", "UNTIE"]));
            }
            p.expect(Punct::RParen)?;
        }
        Ok(PrefscalTransformation::Ordinal { untie })
    } else if p.at_keyword("SMOOTH") {
        Err(SyntaxError::new(
            p.span(),
            "the SMOOTH transformation is not supported yet",
        ))
    } else if p.eat_keyword("SPLINE") {
        let (mut intercept, mut degree, mut inknot) = (false, 2, 1);
        if p.eat(Punct::LParen) {
            while !p.eat(Punct::RParen) {
                if p.eat_keyword("INTERCEPT") {
                    intercept = true;
                } else if p.eat_keyword("ORDER") || p.eat_keyword("DEGREE") {
                    p.eat(Punct::Eq);
                    let at = p.pos;
                    degree = whole(p, "the degree", 1)?;
                    if degree > 3 {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "the degree of a spline goes from 1 to 3",
                        ));
                    }
                } else if p.eat_keyword("INKNOT") {
                    p.eat(Punct::Eq);
                    inknot = whole(p, "the number of interior knots", 0)?;
                } else {
                    return Err(p.unexpected(&["INTERCEPT", "ORDER", "INKNOT", "')'"]));
                }
                p.eat(Punct::Comma);
            }
        }
        Ok(PrefscalTransformation::Spline {
            intercept,
            degree,
            inknot,
        })
    } else {
        Err(p.unexpected(&["NONE", "LINEAR", "ORDINAL", "SMOOTH", "SPLINE"]))
    }
}

fn print_defaults() -> PrefscalPrint {
    PrefscalPrint {
        measures: true,
        common: true,
        weights: true,
        ..PrefscalPrint::default()
    }
}

fn plot_defaults() -> PrefscalPlot {
    PrefscalPlot {
        common: true,
        weights: true,
        ..PrefscalPlot::default()
    }
}

fn print(p: &mut Parser) -> PResult<PrefscalPrint> {
    if at_end(p) {
        return Ok(print_defaults());
    }
    let mut print = PrefscalPrint::default();
    while !at_end(p) {
        let flag = if p.eat_keyword("NONE") {
            print = PrefscalPrint::default();
            p.eat(Punct::Comma);
            continue;
        } else if p.eat_keyword("INPUT") {
            &mut print.input
        } else if p.eat_keyword("MULTIPLE") {
            &mut print.multiple
        } else if p.eat_keyword("INITIAL") {
            &mut print.initial
        } else if p.eat_keyword("HISTORY") {
            &mut print.history
        } else if p.eat_keyword("MEASURES") {
            &mut print.measures
        } else if p.eat_keyword("DECOMPOSITION") {
            &mut print.decomposition
        } else if p.eat_keyword("COMMON") {
            &mut print.common
        } else if p.eat_keyword("DISTANCES") {
            &mut print.distances
        } else if p.eat_keyword("WEIGHTS") {
            &mut print.weights
        } else if p.eat_keyword("INDIVIDUAL") {
            &mut print.individual
        } else if p.eat_keyword("TRANSFORMATIONS") {
            &mut print.transformations
        } else {
            return Err(p.unexpected(&[
                "NONE",
                "INPUT",
                "MULTIPLE",
                "INITIAL",
                "HISTORY",
                "MEASURES",
                "DECOMPOSITION",
                "COMMON",
                "DISTANCES",
                "WEIGHTS",
                "INDIVIDUAL",
                "TRANSFORMATION",
            ]));
        };
        *flag = true;
        p.eat(Punct::Comma);
    }
    Ok(print)
}

fn plot(p: &mut Parser) -> PResult<PrefscalPlot> {
    if at_end(p) {
        return Ok(plot_defaults());
    }
    let mut plot = PrefscalPlot::default();
    while !at_end(p) {
        if p.eat_keyword("NONE") {
            plot = PrefscalPlot::default();
        } else if p.eat_keyword("MULTIPLE") {
            plot.multiple = true;
        } else if p.eat_keyword("INITIAL") {
            plot.initial = true;
        } else if p.eat_keyword("STRESS") {
            plot.stress = true;
        } else if p.eat_keyword("COMMON") {
            plot.common = true;
        } else if p.eat_keyword("WEIGHTS") {
            plot.weights = true;
        } else if p.eat_keyword("INDIVIDUAL") {
            plot.individual = Some(value_lists(p)?);
        } else if p.eat_keyword("TRANSFORMATIONS") {
            plot.transformations = Some(value_lists(p)?);
        } else if p.eat_keyword("SHEPARD") {
            plot.shepard = Some(value_lists(p)?);
        } else if p.eat_keyword("FIT") {
            plot.fit = Some(value_lists(p)?);
        } else if p.eat_keyword("RESIDUALS") {
            plot.residuals = Some(value_lists(p)?);
        } else {
            return Err(p.unexpected(&[
                "NONE",
                "MULTIPLE",
                "INITIAL",
                "STRESS",
                "COMMON",
                "WEIGHTS",
                "INDIVIDUAL",
                "TRANSFORMATIONS",
                "SHEPARD",
                "FIT",
                "RESIDUALS",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(plot)
}

/// INITIAL=... after the equals sign.
fn initial(p: &mut Parser) -> PResult<PrefscalStart> {
    let optional = |p: &mut Parser, what: &str| -> PResult<usize> {
        match p.at(Punct::LParen) {
            true => whole_in_parens(p, what, 1),
            false => Ok(1),
        }
    };
    if p.eat_keyword("CLASSICAL") {
        let mut spearman = false;
        if p.eat(Punct::LParen) {
            if p.eat_keyword("SPEARMAN") {
                spearman = true;
            } else if !p.eat_keyword("TRIANGLE") {
                return Err(p.unexpected(&["TRIANGLE", "SPEARMAN"]));
            }
            p.expect(Punct::RParen)?;
        }
        Ok(PrefscalStart::Classical { spearman })
    } else if p.eat_keyword("CORRESPONDENCE") {
        Ok(PrefscalStart::Correspondence)
    } else if p.eat_keyword("ROSSCLIFF") {
        Ok(PrefscalStart::RossCliff)
    } else if p.eat_keyword("CENTROIDS") {
        Ok(PrefscalStart::Centroids(optional(
            p,
            "the number of choices",
        )?))
    } else if p.eat_keyword("RANDOM") {
        Ok(PrefscalStart::Random(optional(p, "the number of starts")?))
    } else if p.at(Punct::LParen) || matches!(p.peek(), Some(TokenKind::Str(_))) {
        Err(SyntaxError::new(
            p.span(),
            "an initial configuration from a file is not supported yet",
        ))
    } else {
        Err(p.unexpected(&[
            "CLASSICAL",
            "CORRESPONDENCE",
            "ROSSCLIFF",
            "CENTROIDS",
            "RANDOM",
        ]))
    }
}

pub(crate) fn prefscal(p: &mut Parser) -> PResult<Command> {
    let mut c = Prefscal {
        variables: Vec::new(),
        rows: None,
        sources: None,
        similarities: false,
        weights: Vec::new(),
        initial: PrefscalStart::Classical { spearman: false },
        condition: PrefscalCondition::Row,
        transformation: PrefscalTransformation::Ordinal { untie: false },
        model: ScalingModel::Identity,
        lambda: 0.5,
        omega: 1.0,
        dimensions: (2, 2),
        max_iterations: 5000,
        diff_stress: 0.000001,
        min_stress: 0.0001,
        print: print_defaults(),
        plot: plot_defaults(),
        markers: None,
        colors: None,
        outfile: ProxscalOutfile::default(),
    };
    if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        p.pos += 2;
    }
    c.variables = p.var_list()?;
    if c.variables.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("INPUT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("ROWS") {
                    c.rows = Some(count(p, "the number of rows")?);
                } else if p.eat_keyword("SOURCES") {
                    c.sources = Some(count(p, "the number of sources")?);
                } else {
                    return Err(p.unexpected(&["ROWS", "SOURCES"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PROXIMITIES") {
            p.eat(Punct::Eq);
            c.similarities = if p.eat_keyword("DISSIMILARITIES") {
                false
            } else if p.eat_keyword("SIMILARITIES") {
                true
            } else {
                return Err(p.unexpected(&["DISSIMILARITIES", "SIMILARITIES"]));
            };
        } else if p.eat_keyword("WEIGHTS") {
            p.eat(Punct::Eq);
            c.weights = p.var_list()?;
        } else if p.eat_keyword("INITIAL") {
            p.eat(Punct::Eq);
            c.initial = initial(p)?;
        } else if p.eat_keyword("CONDITION") {
            p.eat(Punct::Eq);
            c.condition = if p.eat_keyword("ROW") {
                PrefscalCondition::Row
            } else if p.eat_keyword("MATRIX") {
                PrefscalCondition::Matrix
            } else if p.eat_keyword("UNCONDITIONAL") {
                PrefscalCondition::Unconditional
            } else {
                return Err(p.unexpected(&["ROW", "MATRIX", "UNCONDITIONAL"]));
            };
        } else if p.eat_keyword("TRANSFORMATIONS") {
            p.eat(Punct::Eq);
            c.transformation = transformation(p)?;
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            c.model = if p.eat_keyword("IDENTITY") {
                ScalingModel::Identity
            } else if p.eat_keyword("WEIGHTED") {
                ScalingModel::Weighted
            } else if p.eat_keyword("GENERALIZED") {
                ScalingModel::Generalized
            } else {
                return Err(p.unexpected(&["IDENTITY", "WEIGHTED", "GENERALIZED"]));
            };
        } else if p.eat_keyword("PENALTY") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("LAMBDA") {
                    c.lambda = value(
                        p,
                        "LAMBDA",
                        |v| v > 0.0 && v <= 1.0,
                        "above 0 and at most 1",
                    )?;
                } else if p.eat_keyword("OMEGA") {
                    c.omega = value(p, "OMEGA", |v| v >= 0.0, "0 or more")?;
                } else {
                    return Err(p.unexpected(&["LAMBDA", "OMEGA"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("DIMENSIONS") {
                    p.expect(Punct::LParen)?;
                    let start = p.pos;
                    let min = whole(p, "the number of dimensions", 1)?;
                    p.eat(Punct::Comma);
                    let max = match p.at(Punct::RParen) {
                        true => min,
                        false => whole(p, "the number of dimensions", 1)?,
                    };
                    if min > max {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            format!(
                                "the smallest number of dimensions {min} is above the largest {max}"
                            ),
                        ));
                    }
                    p.expect(Punct::RParen)?;
                    c.dimensions = (min, max);
                } else if p.eat_keyword("MAXITER") {
                    c.max_iterations = whole_in_parens(p, "MAXITER", 0)?;
                } else if p.eat_keyword("DIFFSTRESS") {
                    c.diff_stress = value(p, "DIFFSTRESS", |v| v >= 0.0, "0 or more")?;
                } else if p.eat_keyword("MINSTRESS") {
                    c.min_stress = value(p, "MINSTRESS", |v| v >= 0.0, "0 or more")?;
                } else {
                    return Err(p.unexpected(&[
                        "DIMENSIONS",
                        "MAXITER",
                        "DIFFSTRESS",
                        "MINSTRESS",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            c.print = print(p)?;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            c.plot = plot(p)?;
        } else if p.eat_keyword("OPTIONS") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let slot = if p.eat_keyword("MARKERS") {
                    &mut c.markers
                } else if p.eat_keyword("COLORS") {
                    &mut c.colors
                } else {
                    return Err(p.unexpected(&["MARKERS", "COLORS"]));
                };
                p.expect(Punct::LParen)?;
                *slot = Some(p.name("a variable name")?);
                p.expect(Punct::RParen)?;
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let slot = if p.eat_keyword("COMMON") {
                    &mut c.outfile.common
                } else if p.eat_keyword("WEIGHTS") {
                    &mut c.outfile.weights
                } else if p.eat_keyword("DISTANCES") {
                    &mut c.outfile.distances
                } else if p.eat_keyword("TRANSFORMATIONS") {
                    &mut c.outfile.transformations
                } else {
                    return Err(p.unexpected(&[
                        "COMMON",
                        "WEIGHTS",
                        "DISTANCES",
                        "TRANSFORMATIONS",
                    ]));
                };
                *slot = Some(out_target(p)?);
            }
        } else {
            return Err(p.subcommand_error("PREFSCAL", &["RESTRICTIONS"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Prefscal(Box::new(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(source: &str) -> Prefscal {
        let (parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match &parsed[0].command {
            Command::Prefscal(p) => (**p).clone(),
            other => panic!("{other:?}"),
        }
    }

    fn err(source: &str) -> String {
        let (_, errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors[0].message.clone()
    }

    #[test]
    fn defaults() {
        let p = one("PREFSCAL VARIABLES=a TO e.");
        assert_eq!(p.initial, PrefscalStart::Classical { spearman: false });
        assert_eq!(p.condition, PrefscalCondition::Row);
        assert_eq!(
            p.transformation,
            PrefscalTransformation::Ordinal { untie: false }
        );
        assert_eq!((p.lambda, p.omega, p.max_iterations), (0.5, 1.0, 5000));
        assert_eq!((p.diff_stress, p.min_stress), (0.000001, 0.0001));
        assert!(p.print.measures && p.print.common && p.print.weights && !p.print.history);
        assert!(p.plot.common && p.plot.weights && p.plot.individual.is_none());
    }

    #[test]
    fn keywords_and_abbreviations() {
        let p = one(
            "PREFSCAL a b c /INP=ROW(id) SOURCE(4) /PROX=SIM /WEIGHTS=w1 TO w3 \
             /INITIAL=CLASSICAL(SPEARMAN) /COND=UNCOND /TRANS=SPLINE(INTERCEPT ORDER=3 INKNOT=2) \
             /MODEL=GENERALIZED /PENALTY=LAMBDA(0.8) OMEGA(2) \
             /CRITERIA=DIMENSIONS(1,3) MAXITER(0) DIFFSTRESS(0) MINSTRESS(0) \
             /PRINT=HISTORY DECOMPOSITION TRANSFORMATION \
             /PLOT=INDIVIDUAL(ALL) TRANSFORMATIONS(1 TO 4)(2 7) SHEPARD FIT(1) RESIDUALS(2) \
             /OPTIONS=MARKERS(g) COLORS(age) /OUTFILE=COMMON(space) TRANSFORMATION('t.sav').",
        );
        assert_eq!(p.rows, Some(InputCount::Variable("id".into())));
        assert_eq!(p.sources, Some(InputCount::Number(4)));
        assert!(p.similarities);
        assert_eq!(p.initial, PrefscalStart::Classical { spearman: true });
        assert_eq!(p.condition, PrefscalCondition::Unconditional);
        assert_eq!(
            p.transformation,
            PrefscalTransformation::Spline {
                intercept: true,
                degree: 3,
                inknot: 2
            }
        );
        assert_eq!(p.model, ScalingModel::Generalized);
        assert_eq!((p.lambda, p.omega), (0.8, 2.0));
        assert_eq!((p.dimensions, p.max_iterations), ((1, 3), 0));
        assert!(p.print.history && p.print.decomposition && p.print.transformations);
        assert!(!p.print.measures && !p.print.common);
        assert_eq!(p.plot.individual, Some(vec![Vec::new()]));
        assert_eq!(
            p.plot.transformations,
            Some(vec![vec![(1.0, 4.0)], vec![(2.0, 2.0), (7.0, 7.0)]])
        );
        assert_eq!(p.plot.shepard, Some(vec![Vec::new()]));
        assert_eq!(p.plot.fit, Some(vec![vec![(1.0, 1.0)]]));
        assert!(!p.plot.common);
        assert_eq!(
            (p.markers.as_deref(), p.colors.as_deref()),
            (Some("g"), Some("age"))
        );
        assert_eq!(p.outfile.common.as_deref(), Some("space"));
        assert_eq!(p.outfile.transformations.as_deref(), Some("t.sav"));
        for (start, want) in [
            ("CORRESPONDENCE", PrefscalStart::Correspondence),
            ("ROSSCLIFF", PrefscalStart::RossCliff),
            ("CENTROIDS(2)", PrefscalStart::Centroids(2)),
            ("RANDOM", PrefscalStart::Random(1)),
        ] {
            assert_eq!(
                one(&format!("PREFSCAL a b /INITIAL={start}.")).initial,
                want
            );
        }
        assert_eq!(
            one("PREFSCAL a b /TRANSFORMATION=NONE(INTERCEPT).").transformation,
            PrefscalTransformation::None { intercept: true }
        );
        assert_eq!(
            one("PREFSCAL a b /TRANSFORMATION=LINEAR.").transformation,
            PrefscalTransformation::Linear { intercept: false }
        );
    }

    #[test]
    fn errors() {
        assert!(err("PREFSCAL a b /RESTRICTIONS=ROW(NONE).").contains("not supported yet"));
        assert!(err("PREFSCAL a b /TRANSFORMATION=SMOOTH.").contains("not supported yet"));
        assert!(err("PREFSCAL a b /INITIAL=('start.sav').").contains("not supported yet"));
        assert!(err("PREFSCAL a b /PENALTY=LAMBDA(0).").contains("LAMBDA"));
        assert!(err("PREFSCAL a b /PENALTY=OMEGA(-1).").contains("OMEGA"));
        assert!(err("PREFSCAL a b /CRITERIA=DIMENSIONS(3,2).").contains("above"));
        assert!(err("PREFSCAL a b /TRANSFORMATION=SPLINE(ORDER=4).").contains("1 to 3"));
        assert!(err("PREFSCAL a b /MODEL=REDUCED.").contains("REDUCED"));
    }
}
