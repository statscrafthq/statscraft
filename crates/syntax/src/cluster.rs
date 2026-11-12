//! Parser of CLUSTER (hierarchical clustering of cases).
//!
//! Rules from the Command Syntax Reference:
//! - The variable list comes first; with MATRIX=IN it may be left out, and then the command
//!   starts with a slash. The other subcommands come in any order.
//! - MEASURE takes one measure, SEUCLID (alias DEFAULT) when left out or empty. The binary
//!   measures take `(p[,np])`: the value of presence and of absence, 1 and 0 by default; with p
//!   alone every other value is absence.
//! - METHOD takes one or more methods, each with an optional `(rootname)` for SAVE; BAVERAGE
//!   (alias DEFAULT) when left out or empty.
//! - SAVE=CLUSTER(level) or CLUSTER(min,max); PRINT=CLUSTER(level | min,max) DISTANCE SCHEDULE
//!   (alias DEFAULT) NONE, SCHEDULE when left out or empty, NONE overriding the others.
//! - PLOT=VICICLE[(min[,max[,inc]])] HICICLE[(...)] DENDROGRAM NONE.
//! - MISSING=EXCLUDE (default) or INCLUDE. MATRIX=IN(file | dataset | *) and OUT(...).

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The binary measures, in the order of the manual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryMeasure {
    Rr,
    Sm,
    Jaccard,
    Dice,
    Ss1,
    Rt,
    Ss2,
    K1,
    Ss3,
    K2,
    Ss4,
    Hamann,
    Ochiai,
    Ss5,
    Phi,
    Lambda,
    D,
    Y,
    Q,
    Beuclid,
    Size,
    Pattern,
    Bseuclid,
    Bshape,
    Disper,
    Variance,
    Blwmn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Proximity {
    SEuclid,
    Euclid,
    Cosine,
    Correlation,
    Chebychev,
    Block,
    Minkowski(f64),
    /// POWER(p, r)
    Power(f64, f64),
    Chisq,
    Ph2,
    /// The measure with the value of presence and, if given, of absence.
    Binary(BinaryMeasure, f64, Option<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    BetweenGroups,
    WithinGroups,
    Single,
    Complete,
    Centroid,
    Median,
    Ward,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    /// Empty with matrix input and no list.
    pub variables: Vec<VarItem>,
    pub include: bool,
    pub measure: Proximity,
    /// Each method with its root name for SAVE.
    pub methods: Vec<(Linkage, Option<String>)>,
    /// `SAVE=CLUSTER(min,max)`
    pub save: Option<(usize, usize)>,
    pub id: Option<String>,
    pub schedule: bool,
    /// `PRINT=CLUSTER(min,max)`
    pub membership: Option<(usize, usize)>,
    pub distance: bool,
    /// `/PLOT`: a vertical icicle plot without the subcommand.
    pub plot: ClusterPlot,
    /// `MATRIX=IN(...)`: the file or dataset, None for `*`.
    pub matrix_in: Option<Option<String>>,
}

/// The plots of CLUSTER for each method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterPlot {
    /// `VICICLE`
    pub vertical: bool,
    /// `HICICLE`
    pub horizontal: bool,
    /// The solutions the icicle plots show: from, to (None: the number of items minus 1), step.
    pub range: (usize, Option<usize>, usize),
    pub dendrogram: bool,
}

impl ClusterPlot {
    pub const NONE: ClusterPlot = ClusterPlot {
        vertical: false,
        horizontal: false,
        range: (1, None, 1),
        dendrogram: false,
    };

    pub fn any(&self) -> bool {
        self.vertical || self.horizontal || self.dendrogram
    }
}

const BINARY: [(&str, BinaryMeasure); 27] = [
    ("RR", BinaryMeasure::Rr),
    ("SM", BinaryMeasure::Sm),
    ("JACCARD", BinaryMeasure::Jaccard),
    ("DICE", BinaryMeasure::Dice),
    ("SS1", BinaryMeasure::Ss1),
    ("RT", BinaryMeasure::Rt),
    ("SS2", BinaryMeasure::Ss2),
    ("K1", BinaryMeasure::K1),
    ("SS3", BinaryMeasure::Ss3),
    ("K2", BinaryMeasure::K2),
    ("SS4", BinaryMeasure::Ss4),
    ("HAMANN", BinaryMeasure::Hamann),
    ("OCHIAI", BinaryMeasure::Ochiai),
    ("SS5", BinaryMeasure::Ss5),
    ("PHI", BinaryMeasure::Phi),
    ("LAMBDA", BinaryMeasure::Lambda),
    ("D", BinaryMeasure::D),
    ("Y", BinaryMeasure::Y),
    ("Q", BinaryMeasure::Q),
    ("BEUCLID", BinaryMeasure::Beuclid),
    ("SIZE", BinaryMeasure::Size),
    ("PATTERN", BinaryMeasure::Pattern),
    ("BSEUCLID", BinaryMeasure::Bseuclid),
    ("BSHAPE", BinaryMeasure::Bshape),
    ("DISPER", BinaryMeasure::Disper),
    ("VARIANCE", BinaryMeasure::Variance),
    ("BLWMN", BinaryMeasure::Blwmn),
];

const MEASURES: [&str; 11] = [
    "SEUCLID",
    "EUCLID",
    "COSINE",
    "CORRELATION",
    "CHEBYCHEV",
    "BLOCK",
    "MINKOWSKI",
    "POWER",
    "CHISQ",
    "PH2",
    "DEFAULT",
];

/// A positive number in parentheses after a keyword, with the next one after a comma.
fn positive(p: &mut Parser, what: &str) -> PResult<f64> {
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    if n <= 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be above 0, got {n}"),
        ));
    }
    Ok(n)
}

fn measure(p: &mut Parser) -> PResult<Proximity> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(Proximity::SEuclid);
    }
    // the short binary names must be written in full; the others may be abbreviated
    let binary = BINARY.iter().find(|(k, _)| match k.len() {
        1..=3 => matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(k)),
        _ => p.at_keyword(k),
    });
    let result = if let Some((_, m)) = binary {
        p.pos += 1;
        let (mut present, mut absent) = (1.0, Some(0.0));
        if p.eat(Punct::LParen) {
            let Some(v) = p.number() else {
                return Err(p.unexpected(&["the value of presence"]));
            };
            present = v;
            absent = None;
            if p.eat(Punct::Comma) || !p.at(Punct::RParen) {
                let Some(v) = p.number() else {
                    return Err(p.unexpected(&["the value of absence"]));
                };
                absent = Some(v);
            }
            p.expect(Punct::RParen)?;
        }
        Proximity::Binary(*m, present, absent)
    } else if p.eat_word("PH2") {
        Proximity::Ph2
    } else if let Some(i) = MEASURES.iter().position(|k| p.at_keyword(k)) {
        p.pos += 1;
        match i {
            0 | 10 => Proximity::SEuclid,
            1 => Proximity::Euclid,
            2 => Proximity::Cosine,
            3 => Proximity::Correlation,
            4 => Proximity::Chebychev,
            5 => Proximity::Block,
            6 => {
                p.expect(Punct::LParen)?;
                let power = positive(p, "the power of MINKOWSKI")?;
                p.expect(Punct::RParen)?;
                Proximity::Minkowski(power)
            }
            7 => {
                p.expect(Punct::LParen)?;
                let power = positive(p, "the power p of POWER")?;
                p.eat(Punct::Comma);
                let root = positive(p, "the root r of POWER")?;
                p.expect(Punct::RParen)?;
                Proximity::Power(power, root)
            }
            _ => Proximity::Chisq,
        }
    } else {
        let mut names: Vec<&str> = MEASURES.to_vec();
        names.extend(BINARY.iter().map(|(k, _)| *k));
        return Err(p.unexpected(&names));
    };
    if !at_end(p) {
        return Err(SyntaxError::new(p.span(), "MEASURE takes one measure"));
    }
    Ok(result)
}

fn methods(p: &mut Parser) -> PResult<Vec<(Linkage, Option<String>)>> {
    p.eat(Punct::Eq);
    const KEYWORDS: [(&str, Linkage); 9] = [
        ("BAVERAGE", Linkage::BetweenGroups),
        ("DEFAULT", Linkage::BetweenGroups),
        ("WAVERAGE", Linkage::WithinGroups),
        ("SINGLE", Linkage::Single),
        ("COMPLETE", Linkage::Complete),
        ("CENTROID", Linkage::Centroid),
        ("MEDIAN", Linkage::Median),
        ("WARD", Linkage::Ward),
        // the manual's example writes WARDS
        ("WARDS", Linkage::Ward),
    ];
    let mut out = Vec::new();
    while !at_end(p) {
        let Some((_, method)) = KEYWORDS.iter().find(|(k, _)| p.at_keyword(k)) else {
            let names: Vec<&str> = KEYWORDS.iter().map(|(k, _)| *k).take(8).collect();
            return Err(p.unexpected(&names));
        };
        p.pos += 1;
        let root = if p.eat(Punct::LParen) {
            let name = p.name("a root name")?;
            p.expect(Punct::RParen)?;
            Some(name)
        } else {
            None
        };
        out.push((*method, root));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        out.push((Linkage::BetweenGroups, None));
    }
    Ok(out)
}

/// `(level)` or `(min, max)`.
fn levels(p: &mut Parser) -> PResult<(usize, usize)> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let whole = |p: &mut Parser| -> PResult<usize> {
        let at = p.pos;
        match p.number() {
            Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as usize),
            Some(n) => Err(SyntaxError::new(
                p.span_from(at),
                format!("a number of clusters must be a positive whole number, got {n}"),
            )),
            None => Err(p.unexpected(&["a number of clusters"])),
        }
    };
    let min = whole(p)?;
    p.eat(Punct::Comma);
    let max = if p.at(Punct::RParen) { min } else { whole(p)? };
    if min > max {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("the smallest number of clusters {min} is above the largest {max}"),
        ));
    }
    p.expect(Punct::RParen)?;
    Ok((min, max))
}

fn cluster_keyword(p: &mut Parser) -> bool {
    p.eat_keyword("CLUSTER") || p.eat_keyword("CLUSTERS")
}

fn matrix(p: &mut Parser, command: &mut Cluster) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut any = false;
    while !at_end(p) {
        if p.at_keyword("OUT") {
            return Err(SyntaxError::new(
                p.span(),
                "OUT in CLUSTER /MATRIX is not supported yet",
            ));
        }
        if !p.eat_keyword("IN") {
            return Err(p.unexpected(&["IN", "OUT"]));
        }
        p.expect(Punct::LParen)?;
        let source = if p.eat(Punct::Star) {
            None
        } else {
            Some(crate::files::dataset_or_file(p)?)
        };
        p.expect(Punct::RParen)?;
        command.matrix_in = Some(source);
        any = true;
    }
    if !any {
        return Err(p.unexpected(&["IN", "OUT"]));
    }
    Ok(())
}

pub(crate) fn cluster(p: &mut Parser) -> PResult<Command> {
    let mut command = Cluster {
        variables: Vec::new(),
        include: false,
        measure: Proximity::SEuclid,
        methods: vec![(Linkage::BetweenGroups, None)],
        save: None,
        id: None,
        schedule: true,
        membership: None,
        distance: false,
        plot: ClusterPlot {
            vertical: true,
            ..ClusterPlot::NONE
        },
        matrix_in: None,
    };
    if matches!(p.peek(), Some(TokenKind::Word(_))) {
        command.variables = p.var_list()?;
    } else if !p.at(Punct::Slash) {
        return Err(p.unexpected(&["a variable name", "'/'"]));
    }
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("MEASURE") {
            command.measure = measure(p)?;
        } else if p.eat_keyword("METHOD") {
            command.methods = methods(p)?;
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            if !cluster_keyword(p) {
                return Err(p.unexpected(&["CLUSTER"]));
            }
            command.save = Some(levels(p)?);
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            let empty = at_end(p);
            let (mut schedule, mut membership, mut distance, mut none) =
                (empty, None, false, false);
            while !at_end(p) {
                if p.eat_keyword("SCHEDULE") || p.eat_keyword("DEFAULT") {
                    schedule = true;
                } else if cluster_keyword(p) {
                    membership = Some(levels(p)?);
                } else if p.eat_keyword("DISTANCE") {
                    distance = true;
                } else if p.eat_keyword("NONE") {
                    none = true;
                } else {
                    return Err(p.unexpected(&["SCHEDULE", "CLUSTER", "DISTANCE", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
            if none {
                (schedule, membership, distance) = (false, None, false);
            }
            (command.schedule, command.membership, command.distance) =
                (schedule, membership, distance);
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            // PLOT alone keeps the default; keywords name every plot wanted
            let mut plot = match at_end(p) {
                true => command.plot,
                false => ClusterPlot::NONE,
            };
            while !at_end(p) {
                let vertical = p.eat_keyword("VICICLE");
                if vertical || p.eat_keyword("HICICLE") {
                    match vertical {
                        true => plot.vertical = true,
                        false => plot.horizontal = true,
                    }
                    // the last icicle keyword's range counts for both, its defaults included
                    plot.range = (1, None, 1);
                    if p.eat(Punct::LParen) {
                        let start = p.pos;
                        let mut numbers = Vec::new();
                        while let Some(n) = p.number() {
                            numbers.push(n);
                            p.eat(Punct::Comma);
                        }
                        p.expect(Punct::RParen)?;
                        if numbers.is_empty()
                            || numbers.len() > 3
                            || numbers.iter().any(|n| *n < 1.0 || n.fract() != 0.0)
                        {
                            let message =
                                "an icicle range is min[,max[,inc]], whole numbers from 1";
                            return Err(SyntaxError::new(p.span_from(start), message));
                        }
                        let n = |i: usize| numbers.get(i).map(|n| *n as usize);
                        plot.range = (n(0).unwrap_or(1), n(1), n(2).unwrap_or(1));
                    }
                } else if p.eat_keyword("DENDROGRAM") {
                    plot.dendrogram = true;
                } else if p.eat_keyword("NONE") {
                    plot = ClusterPlot::NONE;
                } else {
                    return Err(p.unexpected(&["VICICLE", "HICICLE", "DENDROGRAM", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
            command.plot = plot;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("EXCLUDE") {
                    command.include = false;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("MATRIX") {
            matrix(p, &mut command)?;
        } else {
            return Err(p.subcommand_error("CLUSTER", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.matrix_in.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "CLUSTER needs a variable list unless MATRIX=IN reads a proximity matrix",
        ));
    }
    Ok(Command::Cluster(Box::new(command)))
}
