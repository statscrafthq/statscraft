//! Parser of ALSCAL (multidimensional scaling by alternating least squares on squared
//! distances).
//!
//! Rules from the Command Syntax Reference:
//! - VARIABLES=varlist is required unless MATRIX=IN reads a matrix: the columns of one or more
//!   square matrices, one row per case. INPUT=ROWS(ALL | n) gives the rows per matrix.
//! - SHAPE=SYMMETRIC (default; the triangle below the diagonal), ASYMMETRIC or RECTANGULAR.
//! - LEVEL=ORDINAL (default) [(UNTIE SIMILAR)], INTERVAL[(1..4)], RATIO[(1..4)] or NOMINAL.
//! - CONDITION=MATRIX (default), ROW or UNCONDITIONAL. MODEL (alias METHOD)=EUCLID (default),
//!   INDSCAL, ASCAL, AINDS or GEMSCAL.
//! - CRITERIA: NEGATIVE, CUTOFF(0), CONVERGE(.001), ITER(30), STRESSMIN(.005), NOULB,
//!   DIMENS(2 | min,max) from 1 (EUCLID only) or 2 to 6, DIRECTIONS(n), CONSTRAIN, TIESTORE(n).
//! - PRINT=DATA HEADER; PLOT[=DEFAULT | ALL], no plots without the subcommand.
//! - OUTFILE=file; MATRIX=IN(file | dataset | *).

use crate::ast::{Command, VarItem};
use crate::cluster::matrix_source;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlscalLevel {
    Ordinal {
        untie: bool,
        similar: bool,
    },
    /// The degree of the polynomial.
    Interval(usize),
    Ratio(usize),
    Nominal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alscal {
    pub variables: Vec<VarItem>,
    /// INPUT=ROWS(n); None for ALL.
    pub rows: Option<usize>,
    pub level: AlscalLevel,
    pub unconditional: bool,
    /// MODEL=INDSCAL.
    pub indscal: bool,
    pub negative: bool,
    pub cutoff: f64,
    pub converge: f64,
    pub iterations: usize,
    pub stress_min: f64,
    pub dimensions: (usize, usize),
    pub print_data: bool,
    pub print_header: bool,
    /// None: no plots; Some(false): the default plots; Some(true): ALL.
    pub plot: Option<bool>,
    pub outfile: Option<String>,
    pub matrix_in: Option<Option<String>>,
    /// `/FILE`: the configuration and weights file the iterations start from.
    pub file: Option<AlscalFile>,
}

/// `/FILE=file [CONFIG | ROWCONF] [SUBJWGHT]`: what is read from the configuration/weights file.
#[derive(Debug, Clone, PartialEq)]
pub struct AlscalFile {
    pub name: String,
    /// The stimulus coordinates (TYPE_ CON or ROW) start the iterations.
    pub config: bool,
    /// The subject weights (TYPE_ SUB) start the weights of MODEL=INDSCAL.
    pub weights: bool,
    /// What to read was listed; if not, the model's default: the configuration and, for
    /// INDSCAL, the subject weights.
    pub listed: bool,
}

fn number_in_parens(p: &mut Parser, what: &str) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(v) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    p.expect(Punct::RParen)?;
    Ok(v)
}

fn whole(p: &mut Parser, what: &str, low: usize, high: usize) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low as f64 && n <= high as f64 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number from {low} to {high}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn not_yet(p: &Parser, what: &str) -> SyntaxError {
    SyntaxError::new(p.span(), format!("{what} is not supported yet"))
}

pub(crate) fn alscal(p: &mut Parser) -> PResult<Command> {
    let mut command = Alscal {
        variables: Vec::new(),
        rows: None,
        level: AlscalLevel::Ordinal {
            untie: false,
            similar: false,
        },
        unconditional: false,
        indscal: false,
        negative: false,
        cutoff: 0.0,
        converge: 0.001,
        iterations: 30,
        stress_min: 0.005,
        dimensions: (2, 2),
        print_data: false,
        print_header: false,
        plot: None,
        outfile: None,
        matrix_in: None,
        file: None,
    };
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if p.peek().is_none() {
            break;
        }
        // the first subcommand comes without a slash
        if first && p.at(Punct::Slash) {
            p.pos += 1;
        }
        first = false;
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            command.variables = p.var_list()?;
        } else if p.eat_keyword("INPUT") {
            p.eat(Punct::Eq);
            if p.eat_keyword("ROWS") && p.eat(Punct::LParen) {
                if !p.eat_word("ALL") {
                    command.rows = Some(whole(p, "the number of rows", 4, usize::MAX)?);
                }
                p.expect(Punct::RParen)?;
            }
        } else if p.eat_keyword("SHAPE") {
            p.eat(Punct::Eq);
            if p.at_keyword("ASYMMETRIC") || p.at_keyword("RECTANGULAR") {
                return Err(not_yet(p, "SHAPE=ASYMMETRIC or RECTANGULAR"));
            }
            if !p.eat_keyword("SYMMETRIC") {
                return Err(p.unexpected(&["SYMMETRIC", "ASYMMETRIC", "RECTANGULAR"]));
            }
        } else if p.eat_keyword("LEVEL") {
            p.eat(Punct::Eq);
            command.level = if p.eat_keyword("ORDINAL") {
                let (mut untie, mut similar) = (false, false);
                if p.eat(Punct::LParen) {
                    while !p.eat(Punct::RParen) {
                        if p.eat_keyword("UNTIE") {
                            untie = true;
                        } else if p.eat_keyword("SIMILAR") {
                            similar = true;
                        } else {
                            return Err(p.unexpected(&["UNTIE", "SIMILAR", "')'"]));
                        }
                        p.eat(Punct::Comma);
                    }
                }
                AlscalLevel::Ordinal { untie, similar }
            } else if p.eat_keyword("INTERVAL") || p.at_keyword("RATIO") {
                let interval = !p.eat_keyword("RATIO");
                let degree = match p.eat(Punct::LParen) {
                    true => {
                        let n = whole(p, "the degree", 1, 4)?;
                        p.expect(Punct::RParen)?;
                        n
                    }
                    false => 1,
                };
                match interval {
                    true => AlscalLevel::Interval(degree),
                    false => AlscalLevel::Ratio(degree),
                }
            } else if p.eat_keyword("NOMINAL") {
                AlscalLevel::Nominal
            } else {
                return Err(p.unexpected(&["ORDINAL", "INTERVAL", "RATIO", "NOMINAL"]));
            };
        } else if p.eat_keyword("CONDITION") {
            p.eat(Punct::Eq);
            if p.at_keyword("ROW") {
                return Err(not_yet(p, "CONDITION=ROW"));
            }
            command.unconditional = if p.eat_keyword("MATRIX") {
                false
            } else if p.eat_keyword("UNCONDITIONAL") {
                true
            } else {
                return Err(p.unexpected(&["MATRIX", "ROW", "UNCONDITIONAL"]));
            };
        } else if p.eat_keyword("MODEL") || p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            if p.at_keyword("ASCAL") || p.at_keyword("AINDS") || p.at_keyword("GEMSCAL") {
                return Err(not_yet(p, "MODEL=ASCAL, AINDS or GEMSCAL"));
            }
            command.indscal = if p.eat_keyword("EUCLID") {
                false
            } else if p.eat_keyword("INDSCAL") {
                true
            } else {
                return Err(p.unexpected(&["EUCLID", "INDSCAL", "ASCAL", "AINDS", "GEMSCAL"]));
            };
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("NEGATIVE") {
                    command.negative = true;
                } else if p.eat_keyword("CUTOFF") {
                    command.cutoff = number_in_parens(p, "a cutoff")?;
                } else if p.eat_keyword("CONVERGE") {
                    let at = p.pos;
                    command.converge = number_in_parens(p, "a convergence criterion")?;
                    if command.converge < 0.0 {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "CONVERGE cannot be negative",
                        ));
                    }
                } else if p.eat_keyword("ITER") {
                    p.expect(Punct::LParen)?;
                    command.iterations = whole(p, "the number of iterations", 1, usize::MAX)?;
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("STRESSMIN") {
                    let at = p.pos;
                    command.stress_min = number_in_parens(p, "the smallest S-stress")?;
                    if !(0.0..=1.0).contains(&command.stress_min) {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "STRESSMIN must be from 0 to 1",
                        ));
                    }
                } else if p.eat_keyword("NOULB") || p.eat_keyword("CONSTRAIN") {
                    // missing values never enter the start here, and there is no unfolding
                } else if p.eat_keyword("DIMENS") {
                    p.expect(Punct::LParen)?;
                    let start = p.pos;
                    let min = whole(p, "the number of dimensions", 1, 6)?;
                    p.eat(Punct::Comma);
                    let max = match p.at(Punct::RParen) {
                        true => min,
                        false => whole(p, "the number of dimensions", 1, 6)?,
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
                } else if p.eat_keyword("DIRECTIONS") || p.eat_keyword("TIESTORE") {
                    // DIRECTIONS only counts for GEMSCAL; ties need no storage here
                    number_in_parens(p, "a number")?;
                } else {
                    return Err(p.unexpected(&[
                        "NEGATIVE",
                        "CUTOFF",
                        "CONVERGE",
                        "ITER",
                        "STRESSMIN",
                        "NOULB",
                        "DIMENS",
                        "DIRECTIONS",
                        "CONSTRAIN",
                        "TIESTORE",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("DATA") {
                    command.print_data = true;
                } else if p.eat_keyword("HEADER") {
                    command.print_header = true;
                } else {
                    return Err(p.unexpected(&["DATA", "HEADER"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            let mut all = false;
            while !at_end(p) {
                if p.eat_keyword("ALL") {
                    all = true;
                } else if !p.eat_keyword("DEFAULT") {
                    return Err(p.unexpected(&["DEFAULT", "ALL"]));
                }
                p.eat(Punct::Comma);
            }
            command.plot = Some(all);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = Some(crate::files::dataset_or_file(p)?);
        } else if p.eat_keyword("MATRIX") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("IN") {
                return Err(p.unexpected(&["IN"]));
            }
            command.matrix_in = Some(matrix_source(p)?);
        } else if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            let name = crate::files::dataset_or_file(p)?;
            let (mut config, mut weights, mut given) = (false, false, false);
            while !at_end(p) {
                let start = p.pos;
                if p.eat_keyword("CONFIG") || p.eat_keyword("ROWCONF") {
                    config = true;
                } else if p.eat_keyword("SUBJWGHT") {
                    weights = true;
                } else {
                    return Err(p.unexpected(&["CONFIG", "ROWCONF", "SUBJWGHT"]));
                }
                given = true;
                if p.eat(Punct::LParen) {
                    if p.eat_keyword("FIXED") {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "FIXED coordinates and weights in ALSCAL /FILE are not supported yet; INITIAL is",
                        ));
                    }
                    if !p.eat_keyword("INITIAL") {
                        return Err(p.unexpected(&["INITIAL", "FIXED"]));
                    }
                    p.expect(Punct::RParen)?;
                }
                p.eat(Punct::Comma);
            }
            command.file = Some(AlscalFile {
                name,
                config: config || !given,
                weights,
                listed: given,
            });
        } else if matches!(p.peek(), Some(TokenKind::Word(_))) {
            return Err(p.subcommand_error("ALSCAL", &[]));
        } else {
            return Err(p.unexpected(&["VARIABLES"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.matrix_in.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "ALSCAL needs VARIABLES unless MATRIX=IN reads a proximity matrix",
        ));
    }
    if command.indscal && command.dimensions.0 < 2 {
        return Err(SyntaxError::new(
            p.span(),
            "MODEL=INDSCAL needs at least 2 dimensions",
        ));
    }
    Ok(Command::Alscal(Box::new(command)))
}
