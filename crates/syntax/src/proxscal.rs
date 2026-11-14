//! Parser of PROXSCAL (multidimensional scaling by majorization).
//!
//! Rules from the Command Syntax Reference:
//! - The variable list (optionally after VARIABLES=) comes first: the columns of one or more
//!   square proximity matrices, stacked case after case, or with TABLE the variables that hold
//!   the proximities. With MATRIX=IN the list may be left out.
//! - TABLE=rowid BY columnid [BY sourceid] reads one proximity per case; TABLE=sourceid tells
//!   the stacked square matrices apart.
//! - SHAPE=LOWER (default), UPPER or BOTH. INITIAL=SIMPLEX (default), TORGERSON or
//!   RANDOM[(n)]. WEIGHTS=varlist, one weight variable per proximity variable.
//! - CONDITION=MATRIX (default) or UNCONDITIONAL. TRANSFORMATION=RATIO (default), INTERVAL,
//!   ORDINAL[(KEEPTIES | UNTIE)] or SPLINE [DEGREE=2] [INKNOT=1]; DEGREE from 1 to 3, INKNOT
//!   from 1. PROXIMITIES=DISSIMILARITIES (default) or SIMILARITIES.
//! - MODEL=IDENTITY (default), WEIGHTED, GENERALIZED or REDUCED[(2)]. ACCELERATION=NONE.
//! - CRITERIA=DIMENSIONS(min[,max]) (2), MAXITER(n) (100), DIFFSTRESS(v) and MINSTRESS(v)
//!   (0.0001, from 0 to 1).
//! - PRINT: without the subcommand or keywords COMMON, WEIGHTS, STRESS (and VARIABLES,
//!   CORRELATIONS); keywords name exactly what is shown. PLOT: COMMON and WEIGHTS by default;
//!   INDIVIDUAL, TRANSFORMATIONS and RESIDUALS take source names or ALL in parentheses.
//! - OUTFILE=COMMON(file) WEIGHTS(file) DISTANCES(file) TRANSFORMATIONS(file).
//! - MATRIX=IN(file | dataset | *).

use crate::ast::{Command, VarItem};
use crate::cluster::matrix_source;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxShape {
    Lower,
    Upper,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingStart {
    Simplex,
    Torgerson,
    /// The number of random starts.
    Random(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxTransformation {
    Ratio,
    Interval,
    /// `UNTIE`: tied proximities may get different values.
    Ordinal {
        untie: bool,
    },
    Spline {
        degree: usize,
        inknot: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingModel {
    Identity,
    Weighted,
    Generalized,
    /// The rank of the individual spaces.
    Reduced(usize),
}

/// TABLE: the variables that say where each proximity goes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProxscalTable {
    pub row: Option<String>,
    pub column: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProxscalPrint {
    pub input: bool,
    pub random: bool,
    pub history: bool,
    pub stress: bool,
    pub decomposition: bool,
    pub common: bool,
    pub distances: bool,
    pub weights: bool,
    pub individual: bool,
    pub transformations: bool,
}

/// Plots of PROXSCAL; the source lists are None when not asked for and empty for ALL.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProxscalPlot {
    pub stress: bool,
    pub common: bool,
    pub weights: bool,
    pub individual: Option<Vec<String>>,
    pub transformations: Option<Vec<String>>,
    pub residuals: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProxscalOutfile {
    pub common: Option<String>,
    pub weights: Option<String>,
    pub distances: Option<String>,
    pub transformations: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Proxscal {
    pub variables: Vec<VarItem>,
    pub table: Option<ProxscalTable>,
    pub shape: ProxShape,
    pub initial: ScalingStart,
    pub weights: Vec<VarItem>,
    pub unconditional: bool,
    pub transformation: ProxTransformation,
    pub similarities: bool,
    pub model: ScalingModel,
    pub acceleration: bool,
    pub dimensions: (usize, usize),
    pub max_iterations: usize,
    pub diff_stress: f64,
    pub min_stress: f64,
    pub print: ProxscalPrint,
    pub plot: ProxscalPlot,
    pub outfile: ProxscalOutfile,
    pub matrix_in: Option<Option<String>>,
}

/// A whole number from `low` in parentheses.
fn whole_in_parens(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    p.expect(Punct::LParen)?;
    let n = whole(p, what, low)?;
    p.expect(Punct::RParen)?;
    Ok(n)
}

fn whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low as f64 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number from {low}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// A number from 0 to 1 in parentheses.
fn fraction(p: &mut Parser, what: &str) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let Some(v) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    if !(0.0..=1.0).contains(&v) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be from 0 to 1, got {v}"),
        ));
    }
    p.expect(Punct::RParen)?;
    Ok(v)
}

/// `(name name ...)` or `(ALL)` after a plot keyword: empty for ALL; several lists add up.
fn sources(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = Vec::new();
    let mut all = false;
    while p.eat(Punct::LParen) {
        while !p.eat(Punct::RParen) {
            if p.eat_word("ALL") {
                all = true;
            } else {
                names.push(p.name("a source name")?);
            }
            p.eat(Punct::Comma);
        }
    }
    Ok(if all { Vec::new() } else { names })
}

/// TRANSFORMATION=... after the equals sign.
pub(crate) fn transformation(p: &mut Parser) -> PResult<ProxTransformation> {
    if p.eat_keyword("RATIO") {
        Ok(ProxTransformation::Ratio)
    } else if p.eat_keyword("INTERVAL") {
        Ok(ProxTransformation::Interval)
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
        Ok(ProxTransformation::Ordinal { untie })
    } else if p.eat_keyword("SPLINE") {
        let (mut degree, mut inknot) = (2, 1);
        loop {
            if p.eat_keyword("DEGREE") {
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
                inknot = whole(p, "the number of interior knots", 1)?;
            } else {
                break;
            }
        }
        Ok(ProxTransformation::Spline { degree, inknot })
    } else {
        Err(p.unexpected(&["RATIO", "INTERVAL", "ORDINAL", "SPLINE"]))
    }
}

fn print(p: &mut Parser) -> PResult<ProxscalPrint> {
    let mut print = ProxscalPrint::default();
    if at_end(p) {
        return Ok(defaults());
    }
    while !at_end(p) {
        if p.eat_keyword("NONE") {
            print = ProxscalPrint::default();
        } else if p.eat_keyword("INPUT") {
            print.input = true;
        } else if p.eat_keyword("RANDOM") {
            print.random = true;
        } else if p.eat_keyword("HISTORY") {
            print.history = true;
        } else if p.eat_keyword("STRESS") {
            print.stress = true;
        } else if p.eat_keyword("DECOMPOSITION") {
            print.decomposition = true;
        } else if p.eat_keyword("COMMON") {
            print.common = true;
        } else if p.eat_keyword("DISTANCES") {
            print.distances = true;
        } else if p.eat_keyword("WEIGHTS") {
            print.weights = true;
        } else if p.eat_keyword("INDIVIDUAL") {
            print.individual = true;
        } else if p.eat_keyword("TRANSFORMATIONS") || p.eat_keyword("TRANSFORMATION") {
            print.transformations = true;
        } else if p.eat_keyword("VARIABLES") || p.eat_keyword("CORRELATIONS") {
            // they need RESTRICTIONS, which is not supported: nothing to show
        } else {
            return Err(p.unexpected(&[
                "NONE",
                "INPUT",
                "RANDOM",
                "HISTORY",
                "STRESS",
                "DECOMPOSITION",
                "COMMON",
                "DISTANCES",
                "WEIGHTS",
                "INDIVIDUAL",
                "TRANSFORMATIONS",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(print)
}

fn defaults() -> ProxscalPrint {
    ProxscalPrint {
        stress: true,
        common: true,
        weights: true,
        ..ProxscalPrint::default()
    }
}

fn plot(p: &mut Parser) -> PResult<ProxscalPlot> {
    let mut plot = ProxscalPlot::default();
    if at_end(p) {
        return Ok(ProxscalPlot {
            common: true,
            weights: true,
            ..ProxscalPlot::default()
        });
    }
    while !at_end(p) {
        if p.eat_keyword("NONE") {
            plot = ProxscalPlot::default();
        } else if p.eat_keyword("STRESS") {
            plot.stress = true;
        } else if p.eat_keyword("COMMON") {
            plot.common = true;
        } else if p.eat_keyword("WEIGHTS") {
            plot.weights = true;
        } else if p.eat_keyword("INDIVIDUAL") {
            plot.individual = Some(sources(p)?);
        } else if p.eat_keyword("TRANSFORMATIONS") || p.eat_keyword("TRANSFORMATION") {
            plot.transformations = Some(sources(p)?);
        } else if p.eat_keyword("RESIDUALS") {
            plot.residuals = Some(sources(p)?);
        } else if p.eat_keyword("CORRELATIONS") || p.eat_keyword("VARIABLES") {
            // they need RESTRICTIONS, which is not supported: nothing to plot
            sources(p)?;
        } else {
            return Err(p.unexpected(&[
                "NONE",
                "STRESS",
                "COMMON",
                "WEIGHTS",
                "INDIVIDUAL",
                "TRANSFORMATIONS",
                "RESIDUALS",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(plot)
}

/// `(file | dataset)` of OUTFILE.
pub(crate) fn out_target(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let target = crate::files::dataset_or_file(p)?;
    p.expect(Punct::RParen)?;
    Ok(target)
}

pub(crate) fn proxscal(p: &mut Parser) -> PResult<Command> {
    let mut command = Proxscal {
        variables: Vec::new(),
        table: None,
        shape: ProxShape::Lower,
        initial: ScalingStart::Simplex,
        weights: Vec::new(),
        unconditional: false,
        transformation: ProxTransformation::Ratio,
        similarities: false,
        model: ScalingModel::Identity,
        acceleration: true,
        dimensions: (2, 2),
        max_iterations: 100,
        diff_stress: 0.0001,
        min_stress: 0.0001,
        print: defaults(),
        plot: ProxscalPlot {
            common: true,
            weights: true,
            ..ProxscalPlot::default()
        },
        outfile: ProxscalOutfile::default(),
        matrix_in: None,
    };
    // VARIABLES= before the list, as the manual's examples write it
    if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        p.pos += 2;
    }
    if matches!(p.peek(), Some(TokenKind::Word(_))) {
        command.variables = p.var_list()?;
    } else if !p.at(Punct::Slash) {
        return Err(p.unexpected(&["a variable name", "'/'"]));
    }
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("TABLE") {
            p.eat(Punct::Eq);
            let first = p.name("a variable name")?;
            let mut table = ProxscalTable::default();
            if p.eat_word("BY") {
                table.row = Some(first);
                table.column = Some(p.name("the column variable")?);
                if p.eat_word("BY") {
                    table.source = Some(p.name("the source variable")?);
                }
            } else {
                table.source = Some(first);
            }
            command.table = Some(table);
        } else if p.eat_keyword("SHAPE") {
            p.eat(Punct::Eq);
            command.shape = if p.eat_keyword("LOWER") {
                ProxShape::Lower
            } else if p.eat_keyword("UPPER") {
                ProxShape::Upper
            } else if p.eat_keyword("BOTH") {
                ProxShape::Both
            } else {
                return Err(p.unexpected(&["LOWER", "UPPER", "BOTH"]));
            };
        } else if p.eat_keyword("INITIAL") {
            p.eat(Punct::Eq);
            command.initial = if p.eat_keyword("SIMPLEX") {
                ScalingStart::Simplex
            } else if p.eat_keyword("TORGERSON") {
                ScalingStart::Torgerson
            } else if p.eat_keyword("RANDOM") {
                match p.at(Punct::LParen) {
                    true => ScalingStart::Random(whole_in_parens(p, "the number of starts", 1)?),
                    false => ScalingStart::Random(1),
                }
            } else if p.at(Punct::LParen) || matches!(p.peek(), Some(TokenKind::Word(_))) {
                return Err(SyntaxError::new(
                    p.span(),
                    "an initial configuration from a file is not supported yet",
                ));
            } else {
                return Err(p.unexpected(&["SIMPLEX", "TORGERSON", "RANDOM"]));
            };
        } else if p.eat_keyword("WEIGHTS") {
            p.eat(Punct::Eq);
            command.weights = p.var_list()?;
        } else if p.eat_keyword("CONDITION") {
            p.eat(Punct::Eq);
            command.unconditional = if p.eat_keyword("MATRIX") {
                false
            } else if p.eat_keyword("UNCONDITIONAL") {
                true
            } else {
                return Err(p.unexpected(&["MATRIX", "UNCONDITIONAL"]));
            };
        } else if p.eat_keyword("TRANSFORMATION") || p.eat_keyword("TRANSFORMATIONS") {
            p.eat(Punct::Eq);
            command.transformation = transformation(p)?;
        } else if p.eat_keyword("PROXIMITIES") {
            p.eat(Punct::Eq);
            command.similarities = if p.eat_keyword("DISSIMILARITIES") {
                false
            } else if p.eat_keyword("SIMILARITIES") {
                true
            } else {
                return Err(p.unexpected(&["DISSIMILARITIES", "SIMILARITIES"]));
            };
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            command.model = if p.eat_keyword("IDENTITY") {
                ScalingModel::Identity
            } else if p.eat_keyword("WEIGHTED") {
                ScalingModel::Weighted
            } else if p.eat_keyword("GENERALIZED") {
                ScalingModel::Generalized
            } else if p.eat_keyword("REDUCED") {
                match p.at(Punct::LParen) {
                    true => ScalingModel::Reduced(whole_in_parens(p, "the rank", 1)?),
                    false => ScalingModel::Reduced(2),
                }
            } else {
                return Err(p.unexpected(&["IDENTITY", "WEIGHTED", "GENERALIZED", "REDUCED"]));
            };
        } else if p.eat_keyword("ACCELERATION") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("NONE") {
                return Err(p.unexpected(&["NONE"]));
            }
            command.acceleration = false;
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
                    command.dimensions = (min, max);
                } else if p.eat_keyword("MAXITER") {
                    command.max_iterations = whole_in_parens(p, "MAXITER", 1)?;
                } else if p.eat_keyword("DIFFSTRESS") {
                    command.diff_stress = fraction(p, "DIFFSTRESS")?;
                } else if p.eat_keyword("MINSTRESS") {
                    command.min_stress = fraction(p, "MINSTRESS")?;
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
            command.print = print(p)?;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            command.plot = plot(p)?;
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let slot = if p.eat_keyword("COMMON") {
                    &mut command.outfile.common
                } else if p.eat_keyword("WEIGHTS") {
                    &mut command.outfile.weights
                } else if p.eat_keyword("DISTANCES") {
                    &mut command.outfile.distances
                } else if p.eat_keyword("TRANSFORMATIONS") || p.eat_keyword("TRANSFORMATION") {
                    &mut command.outfile.transformations
                } else if p.at_keyword("VARIABLES") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "OUTFILE VARIABLES needs RESTRICTIONS, which is not supported yet",
                    ));
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
        } else if p.eat_keyword("MATRIX") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("IN") {
                return Err(p.unexpected(&["IN"]));
            }
            command.matrix_in = Some(matrix_source(p)?);
        } else {
            return Err(p.subcommand_error("PROXSCAL", &["RESTRICTIONS"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.matrix_in.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "PROXSCAL needs a variable list unless MATRIX=IN reads a proximity matrix",
        ));
    }
    if let ScalingModel::Reduced(rank) = command.model
        && rank >= command.dimensions.1
    {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "REDUCED({rank}) needs a rank below the largest number of dimensions, {}",
                command.dimensions.1
            ),
        ));
    }
    Ok(Command::Proxscal(Box::new(command)))
}
