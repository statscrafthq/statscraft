//! Parsers of FACTOR (factor analysis) and QUICK CLUSTER (k-means clustering).
//!
//! FACTOR, rules from the Command Syntax Reference:
//! - VARIABLES (abbreviated VAR or VARS) comes first, or MATRIX=IN(COR=...) or IN(COV=...)
//!   in its place; METHOD and SELECT may stand anywhere; MISSING must come before ANALYSIS.
//! - A later ANALYSIS cancels the PRINT, PLOT, DIAGONAL, EXTRACTION, ROTATION and SAVE given
//!   before it; a later EXTRACTION cancels ROTATION and SAVE before it; a later ROTATION
//!   cancels SAVE before it. The last of each subcommand counts.
//! - CRITERIA changes only the criteria it names and applies to the EXTRACTION and ROTATION
//!   after it. Without EXTRACTION the extraction is PC; without ROTATION the factors are
//!   rotated by VARIMAX unless EXTRACTION was given, then they are not rotated.
//! - PRINT without keywords, or not given, is INITIAL EXTRACTION ROTATION.
//! - Keyword aliases: PA1 and DEFAULT for PC, PA2 for PAF, DEFAULT for VARIMAX and for REG,
//!   ROTATE for ROTATION.
//!
//! QUICK CLUSTER: the variable list first, then subcommands in any order. CRITERIA
//! CLUSTER(2) MXITER(10) CONVERGE(0) are the defaults, METHOD=KMEANS(NOUPDATE), PRINT=INITIAL;
//! PRINT=NONE leaves only the tables that are always shown.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactorMissing {
    Listwise,
    Pairwise,
    MeanSubstitution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extraction {
    PrincipalComponents,
    PrincipalAxis,
    Alpha,
    Image,
    UnweightedLeastSquares,
    GeneralizedLeastSquares,
    MaximumLikelihood,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotation {
    Varimax,
    Equamax,
    Quartimax,
    /// Direct oblimin with its delta.
    Oblimin(f64),
    /// Promax with its power kappa.
    Promax(f64),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FactorCriteria {
    /// `FACTORS(n)`; None counts the eigenvalues above MINEIGEN.
    pub factors: Option<usize>,
    pub min_eigen: f64,
    pub iterate: usize,
    pub econverge: f64,
    pub rconverge: f64,
    pub kaiser: bool,
    /// The delta of OBLIMIN and the kappa of PROMAX when the rotation gives none.
    pub delta: f64,
    pub kappa: f64,
}

impl Default for FactorCriteria {
    fn default() -> Self {
        FactorCriteria {
            factors: None,
            min_eigen: 1.0,
            iterate: 25,
            econverge: 0.001,
            rconverge: 0.0001,
            kaiser: true,
            delta: 0.0,
            kappa: 4.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FactorPrint {
    pub initial: bool,
    pub extraction: bool,
    pub rotation: bool,
    pub univariate: bool,
    pub correlation: bool,
    pub covariance: bool,
    pub det: bool,
    pub inv: bool,
    pub repr: bool,
    pub aic: bool,
    pub kmo: bool,
    pub fscore: bool,
    pub sig: bool,
}

impl FactorPrint {
    pub const DEFAULT: FactorPrint = FactorPrint {
        initial: true,
        extraction: true,
        rotation: true,
        univariate: false,
        correlation: false,
        covariance: false,
        det: false,
        inv: false,
        repr: false,
        aic: false,
        kmo: false,
        fscore: false,
        sig: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreMethod {
    Regression,
    Bartlett,
    AndersonRubin,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactorSave {
    pub method: ScoreMethod,
    /// The number of scores; None is ALL.
    pub count: Option<usize>,
    pub root: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixKind {
    Correlation,
    Covariance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Factor {
    /// Empty with matrix input.
    pub variables: Vec<VarItem>,
    /// `/MATRIX=IN(COR=...)` or `IN(COV=...)`: the kind and the file or dataset, None for `*`.
    pub matrix_in: Option<(MatrixKind, Option<String>)>,
    pub missing: FactorMissing,
    pub include: bool,
    /// `/SELECT=var(value)`
    pub select: Option<(String, Value)>,
    pub analysis: Option<Vec<VarItem>>,
    pub print: FactorPrint,
    /// PLOT was given; charts are not drawn.
    pub plot: bool,
    pub diagonal: Option<Vec<f64>>,
    pub sort: bool,
    pub blank: Option<f64>,
    pub extraction: Extraction,
    pub extraction_criteria: FactorCriteria,
    pub rotation: Rotation,
    pub rotation_criteria: FactorCriteria,
    pub save: Option<FactorSave>,
    /// Subcommands that a later subcommand cancelled, for a warning.
    pub ignored: Vec<String>,
}

/// Notes a subcommand that a later one cancelled.
fn cancel(ignored: &mut Vec<String>, what: &str, set: bool) {
    if set {
        ignored.push(what.to_string());
    }
}

/// `(n)` after a keyword.
fn paren_number(p: &mut Parser) -> PResult<(f64, usize)> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok((n, start))
}

fn check(p: &Parser, start: usize, ok: bool, message: String) -> PResult<()> {
    if ok {
        Ok(())
    } else {
        Err(SyntaxError::new(p.span_from(start), message))
    }
}

fn whole(p: &Parser, start: usize, n: f64, what: &str) -> PResult<usize> {
    check(
        p,
        start,
        n >= 1.0 && n.fract() == 0.0,
        format!("{what} must be a positive whole number, got {n}"),
    )?;
    Ok(n as usize)
}

fn criteria(p: &mut Parser, c: &mut FactorCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 10] = [
        "FACTORS",
        "MINEIGEN",
        "ITERATE",
        "ECONVERGE",
        "RCONVERGE",
        "KAISER",
        "NOKAISER",
        "DELTA",
        "KAPPA",
        "DEFAULT",
    ];
    let mut any = false;
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        any = true;
        match i {
            5 => c.kaiser = true,
            6 => c.kaiser = false,
            9 => *c = FactorCriteria::default(),
            _ => {
                let (n, start) = paren_number(p)?;
                match i {
                    0 => c.factors = Some(whole(p, start, n, "FACTORS")?),
                    1 => {
                        check(
                            p,
                            start,
                            n >= 0.0,
                            format!("MINEIGEN cannot be negative, got {n}"),
                        )?;
                        c.min_eigen = n;
                    }
                    2 => c.iterate = whole(p, start, n, "ITERATE")?,
                    3 | 4 => {
                        let what = KEYWORDS[i];
                        check(
                            p,
                            start,
                            n > 0.0,
                            format!("{what} must be above 0, got {n}"),
                        )?;
                        if i == 3 {
                            c.econverge = n;
                        } else {
                            c.rconverge = n;
                        }
                    }
                    7 => c.delta = delta(p, start, n)?,
                    _ => c.kappa = kappa(p, start, n)?,
                }
            }
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&KEYWORDS));
    }
    Ok(())
}

fn delta(p: &Parser, start: usize, n: f64) -> PResult<f64> {
    check(
        p,
        start,
        n <= 0.8,
        format!("the delta of OBLIMIN must be at most 0.8, got {n}"),
    )?;
    Ok(n)
}

fn kappa(p: &Parser, start: usize, n: f64) -> PResult<f64> {
    check(
        p,
        start,
        n > 1.0,
        format!("the kappa of PROMAX must be above 1, got {n}"),
    )?;
    Ok(n)
}

fn print(p: &mut Parser) -> PResult<FactorPrint> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 15] = [
        "DEFAULT",
        "INITIAL",
        "EXTRACTION",
        "ROTATION",
        "UNIVARIATE",
        "CORRELATION",
        "COVARIANCE",
        "DET",
        "INV",
        "REPR",
        "AIC",
        "KMO",
        "FSCORE",
        "SIG",
        "ALL",
    ];
    if at_end(p) {
        return Ok(FactorPrint::DEFAULT);
    }
    let mut out = FactorPrint::default();
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        match i {
            0 => {
                out.initial = true;
                out.extraction = true;
                out.rotation = true;
            }
            1 => out.initial = true,
            2 => out.extraction = true,
            3 => out.rotation = true,
            4 => out.univariate = true,
            5 => out.correlation = true,
            6 => out.covariance = true,
            7 => out.det = true,
            8 => out.inv = true,
            9 => out.repr = true,
            10 => out.aic = true,
            11 => out.kmo = true,
            12 => out.fscore = true,
            13 => out.sig = true,
            _ => {
                out = FactorPrint {
                    initial: true,
                    extraction: true,
                    rotation: true,
                    univariate: true,
                    correlation: true,
                    covariance: true,
                    det: true,
                    inv: true,
                    repr: true,
                    aic: true,
                    kmo: true,
                    fscore: true,
                    sig: true,
                }
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn extraction(p: &mut Parser) -> PResult<Extraction> {
    p.eat(Punct::Eq);
    use Extraction as E;
    const KEYWORDS: [(&str, Extraction); 10] = [
        ("PC", E::PrincipalComponents),
        ("PA1", E::PrincipalComponents),
        ("DEFAULT", E::PrincipalComponents),
        ("PAF", E::PrincipalAxis),
        ("PA2", E::PrincipalAxis),
        ("ALPHA", E::Alpha),
        ("IMAGE", E::Image),
        ("ULS", E::UnweightedLeastSquares),
        ("GLS", E::GeneralizedLeastSquares),
        ("ML", E::MaximumLikelihood),
    ];
    if at_end(p) {
        return Ok(E::PrincipalComponents);
    }
    let Some((_, method)) = KEYWORDS.iter().find(|(k, _)| p.at_keyword(k)) else {
        let names: Vec<&str> = KEYWORDS.iter().map(|(k, _)| *k).collect();
        return Err(p.unexpected(&names));
    };
    p.pos += 1;
    Ok(*method)
}

fn rotation(p: &mut Parser, c: &FactorCriteria) -> PResult<Rotation> {
    p.eat(Punct::Eq);
    if at_end(p) || p.eat_keyword("VARIMAX") || p.eat_keyword("DEFAULT") {
        return Ok(Rotation::Varimax);
    }
    Ok(if p.eat_keyword("EQUAMAX") {
        Rotation::Equamax
    } else if p.eat_keyword("QUARTIMAX") {
        Rotation::Quartimax
    } else if p.eat_keyword("NOROTATE") {
        Rotation::None
    } else if p.eat_keyword("OBLIMIN") {
        match p.at(Punct::LParen) {
            true => {
                let (n, start) = paren_number(p)?;
                Rotation::Oblimin(delta(p, start, n)?)
            }
            false => Rotation::Oblimin(c.delta),
        }
    } else if p.eat_keyword("PROMAX") {
        match p.at(Punct::LParen) {
            true => {
                let (n, start) = paren_number(p)?;
                Rotation::Promax(kappa(p, start, n)?)
            }
            false => Rotation::Promax(c.kappa),
        }
    } else {
        return Err(p.unexpected(&[
            "VARIMAX",
            "EQUAMAX",
            "QUARTIMAX",
            "OBLIMIN",
            "PROMAX",
            "NOROTATE",
        ]));
    })
}

fn save(p: &mut Parser) -> PResult<FactorSave> {
    p.eat(Punct::Eq);
    let mut save = FactorSave {
        method: ScoreMethod::Regression,
        count: None,
        root: None,
    };
    if p.eat_keyword("REG") || p.eat_keyword("DEFAULT") {
    } else if p.eat_keyword("BART") {
        save.method = ScoreMethod::Bartlett;
    } else if p.eat_word("AR") {
        save.method = ScoreMethod::AndersonRubin;
    } else if !p.at(Punct::LParen) && !at_end(p) {
        return Err(p.unexpected(&["REG", "BART", "AR", "'('"]));
    }
    if p.eat(Punct::LParen) {
        if !p.eat_word("ALL") {
            let start = p.pos;
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["ALL", "a number of scores"]));
            };
            save.count = Some(whole(p, start, n, "the number of scores")?);
        }
        p.eat(Punct::Comma);
        if !p.at(Punct::RParen) {
            save.root = Some(p.name("a root name")?);
        }
        p.expect(Punct::RParen)?;
    }
    Ok(save)
}

/// `n*value` repeats a value n times.
fn diagonal(p: &mut Parser) -> PResult<Option<Vec<f64>>> {
    p.eat(Punct::Eq);
    if at_end(p) || p.eat_keyword("DEFAULT") {
        return Ok(None);
    }
    let mut values = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let Some(value) = p.number() else {
            return Err(p.unexpected(&["a number", "DEFAULT"]));
        };
        if p.eat(Punct::Star) {
            let n = whole(p, start, value, "a repeat count")?;
            let Some(v) = p.number() else {
                return Err(p.unexpected(&["a number"]));
            };
            values.extend(std::iter::repeat_n(v, n));
        } else {
            values.push(value);
        }
        p.eat(Punct::Comma);
    }
    Ok(Some(values))
}

fn matrix_in(p: &mut Parser) -> PResult<(MatrixKind, Option<String>)> {
    p.eat(Punct::Eq);
    if p.at_keyword("OUT") {
        return Err(SyntaxError::new(
            p.span(),
            "OUT in FACTOR /MATRIX is not supported yet",
        ));
    }
    if !p.eat_keyword("IN") {
        return Err(p.unexpected(&["IN"]));
    }
    p.expect(Punct::LParen)?;
    let kind = if p.eat_keyword("COR") {
        MatrixKind::Correlation
    } else if p.eat_keyword("COV") {
        MatrixKind::Covariance
    } else {
        return Err(keyword_error(
            p,
            "FACTOR /MATRIX=IN",
            &["FAC"],
            &["COR", "COV"],
        ));
    };
    p.expect(Punct::Eq)?;
    let source = if p.eat(Punct::Star) {
        None
    } else {
        Some(crate::files::dataset_or_file(p)?)
    };
    p.expect(Punct::RParen)?;
    if !at_end(p) {
        return Err(SyntaxError::new(
            p.span(),
            "OUT in FACTOR /MATRIX is not supported yet",
        ));
    }
    Ok((kind, source))
}

pub(crate) fn factor(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    let mut command = Factor {
        variables: Vec::new(),
        matrix_in: None,
        missing: FactorMissing::Listwise,
        include: false,
        select: None,
        analysis: None,
        print: FactorPrint::DEFAULT,
        plot: false,
        diagonal: None,
        sort: false,
        blank: None,
        extraction: Extraction::PrincipalComponents,
        extraction_criteria: FactorCriteria::default(),
        rotation: Rotation::Varimax,
        rotation_criteria: FactorCriteria::default(),
        save: None,
        ignored: Vec::new(),
    };
    if p.eat_keyword("MATRIX") {
        command.matrix_in = Some(matrix_in(p)?);
    } else if p.eat_keyword("VARIABLES") || p.eat_word("VARS") {
        p.eat(Punct::Eq);
        command.variables = p.var_list()?;
    } else {
        return Err(p
            .unexpected(&["VARIABLES", "MATRIX"])
            .hint("FACTOR starts with VARIABLES=varlist"));
    }
    let mut criteria_now = FactorCriteria::default();
    // what the order of the subcommands left in effect
    let mut print: Option<FactorPrint> = None;
    let mut extraction: Option<(Extraction, FactorCriteria)> = None;
    let mut rotation: Option<(Rotation, FactorCriteria)> = None;
    let mut analysis_seen = false;
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        if p.eat_keyword("MISSING") {
            if analysis_seen {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "MISSING must come before ANALYSIS",
                ));
            }
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("LISTWISE") || p.eat_keyword("DEFAULT") {
                    command.missing = FactorMissing::Listwise;
                } else if p.eat_keyword("PAIRWISE") {
                    command.missing = FactorMissing::Pairwise;
                } else if p.eat_keyword("MEANSUB") {
                    command.missing = FactorMissing::MeanSubstitution;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["LISTWISE", "PAIRWISE", "MEANSUB", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("CORRELATION") {
                return Err(keyword_error(
                    p,
                    "FACTOR /METHOD",
                    &["COVARIANCE"],
                    &["CORRELATION"],
                ));
            }
        } else if p.eat_keyword("SELECT") {
            p.eat(Punct::Eq);
            let name = p.name("a variable name")?;
            p.expect(Punct::LParen)?;
            let Some(value) = p.value() else {
                return Err(p.unexpected(&["a value"]));
            };
            p.expect(Punct::RParen)?;
            command.select = Some((name, value));
        } else if p.eat_keyword("ANALYSIS") {
            p.eat(Punct::Eq);
            command.analysis = Some(p.var_list()?);
            analysis_seen = true;
            cancel(&mut command.ignored, "PRINT", print.take().is_some());
            cancel(
                &mut command.ignored,
                "PLOT",
                std::mem::take(&mut command.plot),
            );
            cancel(
                &mut command.ignored,
                "DIAGONAL",
                command.diagonal.take().is_some(),
            );
            cancel(
                &mut command.ignored,
                "EXTRACTION",
                extraction.take().is_some(),
            );
            cancel(&mut command.ignored, "ROTATION", rotation.take().is_some());
            cancel(&mut command.ignored, "SAVE", command.save.take().is_some());
        } else if p.eat_keyword("PRINT") {
            print = Some(self::print(p)?);
        } else if p.eat_keyword("PLOT") {
            while !at_end(p) {
                p.pos += 1;
            }
            command.plot = true;
        } else if p.eat_keyword("DIAGONAL") {
            command.diagonal = diagonal(p)?;
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            command.sort = false;
            command.blank = None;
            while !at_end(p) {
                if p.eat_keyword("SORT") {
                    command.sort = true;
                } else if p.eat_keyword("BLANK") {
                    let (n, start) = paren_number(p)?;
                    check(
                        p,
                        start,
                        n >= 0.0,
                        format!("BLANK cannot be negative, got {n}"),
                    )?;
                    command.blank = Some(n);
                } else if p.eat_keyword("DEFAULT") {
                    command.sort = false;
                    command.blank = None;
                } else {
                    return Err(p.unexpected(&["SORT", "BLANK", "DEFAULT"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut criteria_now)?;
        } else if p.eat_keyword("EXTRACTION") {
            extraction = Some((self::extraction(p)?, criteria_now));
            cancel(&mut command.ignored, "ROTATION", rotation.take().is_some());
            cancel(&mut command.ignored, "SAVE", command.save.take().is_some());
        } else if p.eat_keyword("ROTATION") || p.eat_word("ROTATE") {
            rotation = Some((self::rotation(p, &criteria_now)?, criteria_now));
            cancel(&mut command.ignored, "SAVE", command.save.take().is_some());
        } else if p.eat_keyword("SAVE") {
            command.save = Some(save(p)?);
        } else if p.at_keyword("VARIABLES") || p.at_keyword("MATRIX") {
            return Err(SyntaxError::new(
                p.span(),
                "VARIABLES or MATRIX comes first and only once",
            ));
        } else {
            return Err(p.subcommand_error("FACTOR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    command.print = print.unwrap_or(FactorPrint::DEFAULT);
    let explicit_extraction = extraction.is_some();
    (command.extraction, command.extraction_criteria) =
        extraction.unwrap_or((Extraction::PrincipalComponents, criteria_now));
    // the manual: without ROTATION the factors of an explicit EXTRACTION are not rotated
    (command.rotation, command.rotation_criteria) = rotation.unwrap_or(if explicit_extraction {
        (Rotation::None, criteria_now)
    } else {
        (Rotation::Varimax, criteria_now)
    });
    Ok(Command::Factor(Box::new(command)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterMethod {
    /// KMEANS(NOUPDATE), or KMEANS(UPDATE) with `update`.
    KMeans {
        update: bool,
    },
    Classify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterSave {
    Cluster,
    Distance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuickCluster {
    pub variables: Vec<VarItem>,
    pub pairwise: bool,
    pub include: bool,
    /// `/FILE`: initial centers from a data file or dataset.
    pub file: Option<String>,
    /// `/INITIAL=(values)`, cluster by cluster.
    pub initial: Option<Vec<f64>>,
    pub clusters: usize,
    pub no_initial: bool,
    pub max_iterations: usize,
    pub converge: f64,
    pub method: ClusterMethod,
    pub print_initial: bool,
    pub print_cluster: bool,
    pub id: Option<String>,
    pub print_distance: bool,
    pub print_anova: bool,
    pub outfile: Option<String>,
    /// In the order written, each with its name if given.
    pub save: Vec<(ClusterSave, Option<String>)>,
}

pub(crate) fn quick_cluster(p: &mut Parser) -> PResult<Command> {
    let mut command = QuickCluster {
        variables: p.var_list()?,
        pairwise: false,
        include: false,
        file: None,
        initial: None,
        clusters: 2,
        no_initial: false,
        max_iterations: 10,
        converge: 0.0,
        method: ClusterMethod::KMeans { update: false },
        print_initial: true,
        print_cluster: false,
        id: None,
        print_distance: false,
        print_anova: false,
        outfile: None,
        save: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("LISTWISE") || p.eat_keyword("DEFAULT") {
                    command.pairwise = false;
                } else if p.eat_keyword("PAIRWISE") {
                    command.pairwise = true;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["LISTWISE", "PAIRWISE", "DEFAULT", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = Some(crate::files::dataset_or_file(p)?);
        } else if p.eat_keyword("INITIAL") {
            p.eat(Punct::Eq);
            p.expect(Punct::LParen)?;
            let mut values = Vec::new();
            while let Some(v) = p.number() {
                values.push(v);
                p.eat(Punct::Comma);
            }
            p.expect(Punct::RParen)?;
            command.initial = Some(values);
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("NOINITIAL") {
                    command.no_initial = true;
                } else if p.eat_keyword("CLUSTER") || p.eat_keyword("CLUSTERS") {
                    let (n, start) = paren_number(p)?;
                    command.clusters = whole(p, start, n, "CLUSTER")?;
                } else if p.eat_keyword("MXITER") {
                    let (n, start) = paren_number(p)?;
                    command.max_iterations = whole(p, start, n, "MXITER")?;
                } else if p.eat_keyword("CONVERGE") {
                    let (n, start) = paren_number(p)?;
                    check(
                        p,
                        start,
                        n >= 0.0,
                        format!("CONVERGE cannot be negative, got {n}"),
                    )?;
                    command.converge = n;
                } else {
                    return Err(p.unexpected(&["CLUSTER", "NOINITIAL", "MXITER", "CONVERGE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            command.method = if p.eat_keyword("KMEANS") {
                let mut update = false;
                if p.eat(Punct::LParen) {
                    if p.eat_keyword("UPDATE") {
                        update = true;
                    } else if !p.eat_keyword("NOUPDATE") {
                        return Err(p.unexpected(&["NOUPDATE", "UPDATE"]));
                    }
                    p.expect(Punct::RParen)?;
                }
                ClusterMethod::KMeans { update }
            } else if p.eat_keyword("CLASSIFY") {
                ClusterMethod::Classify
            } else {
                return Err(p.unexpected(&["KMEANS", "CLASSIFY"]));
            };
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            command.print_initial = at_end(p);
            let mut none = false;
            while !at_end(p) {
                if p.eat_keyword("INITIAL") {
                    command.print_initial = true;
                } else if p.eat_keyword("CLUSTER") {
                    command.print_cluster = true;
                } else if p.eat_word("ID") {
                    p.expect(Punct::LParen)?;
                    command.id = Some(p.name("a variable name")?);
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("DISTANCE") {
                    command.print_distance = true;
                } else if p.eat_keyword("ANOVA") {
                    command.print_anova = true;
                } else if p.eat_keyword("NONE") {
                    none = true;
                } else {
                    return Err(
                        p.unexpected(&["INITIAL", "CLUSTER", "ID", "DISTANCE", "ANOVA", "NONE"])
                    );
                }
                p.eat(Punct::Comma);
            }
            if none {
                command.print_initial = false;
                command.print_cluster = false;
                command.print_distance = false;
                command.print_anova = false;
            }
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = Some(crate::files::dataset_or_file(p)?);
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            let mut any = false;
            while !at_end(p) {
                let start = p.pos;
                let what = if p.eat_keyword("CLUSTER") {
                    ClusterSave::Cluster
                } else if p.eat_keyword("DISTANCE") {
                    ClusterSave::Distance
                } else {
                    return Err(p.unexpected(&["CLUSTER", "DISTANCE"]));
                };
                let name = if p.eat(Punct::LParen) {
                    let name = p.name("a variable name")?;
                    p.expect(Punct::RParen)?;
                    Some(name)
                } else {
                    None
                };
                check(
                    p,
                    start,
                    !command.save.iter().any(|(w, _)| *w == what),
                    "CLUSTER and DISTANCE are saved once each".into(),
                )?;
                command.save.push((what, name));
                any = true;
                p.eat(Punct::Comma);
            }
            if !any {
                return Err(p.unexpected(&["CLUSTER", "DISTANCE"]));
            }
        } else if p.at_keyword("VARIABLES") {
            return Err(SyntaxError::new(
                p.span(),
                "QUICK CLUSTER takes its variables first, without VARIABLES=",
            ));
        } else {
            return Err(p.subcommand_error("QUICK CLUSTER", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::QuickCluster(Box::new(command)))
}
