//! Parsers of the older optimal scaling commands: HOMALS, PRINCALS, OVERALS and ANACOR.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - HOMALS, PRINCALS and OVERALS start with the variables, each list followed by its largest
//!   category in parentheses (`a b(3) c(5)`); the word VARIABLES may be left out. Every whole
//!   value from 1 to that largest one is a category; other values are missing.
//! - ANALYSIS names the variables the fit uses (all of VARIABLES without it, except OVERALS,
//!   which needs it). PRINCALS and OVERALS put a level after a list: MNOM, SNOM, ORDI (the
//!   default) or NUME; the level covers the variables since the previous parenthesis. ANALYSIS
//!   given twice is an error; for every other subcommand the last one counts.
//! - OVERALS also needs SETS=n(count, ...): n sets of at least 2, taken in order from ANALYSIS.
//! - NOBSERVATIONS=n keeps the first n cases. DIMENSION (2), MAXITER (100) and CONVERGENCE
//!   (0.00001) tune the fit; OVERALS has INITIAL=NUMERICAL (the default) or RANDOM.
//! - PRINT and PLOT take the keywords of each command; DEFAULT, ALL and NONE stand for groups.
//!   PLOT keywords take a variable list where the command allows one and a label length from 1
//!   to 20 (12 by default) in parentheses; NDIM(first | ALL, last | MAX) picks the dimension
//!   pairs.
//! - SAVE=[root][(n)] adds the object scores; MATRIX=OUT(* | file | dataset) writes the
//!   quantifications.
//! - ANACOR reads TABLE as CORRESPONDENCE does and has DIMENSION, NORMALIZATION (CANONICAL or
//!   DEFAULT, PRINCIPAL, RPRINCIPAL, CPRINCIPAL or a value from -2 to 2), VARIANCES (SINGULAR,
//!   ROWS, COLUMNS), PRINT (TABLE, PROFILES, SCORES, CONTRIBUTIONS, PERMUTATION, DEFAULT, NONE),
//!   PLOT (ROWS, COLUMNS, JOINT, TRROWS, TRCOLUMNS, DEFAULT, NONE, NDIM; label length 3 by
//!   default) and MATRIX OUT=SCORE(file) VARIANCE(file).

use crate::ast::{Command, VarItem};
use crate::catpca::{Catpca, CatpcaPlot, CatpcaPlots, CatpcaPrint, CatpcaVariable, ScalingLevel};
use crate::cluster::matrix_source;
use crate::correspondence::{
    Correspondence, CorrespondenceMeasure, CorrespondenceNormalization, CorrespondencePlot,
    CorrespondencePrint, CorrespondenceStandardize, table,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::multiple_correspondence::{McaPlot, McaPlots, McaPrint, MultipleCorrespondence};
use crate::optimal_scaling::{ScalingCommon, specified_lists, whole};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The legacy reading of the data: categories 1 to a stated largest value.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyCoding {
    /// Each list with its largest category; values outside 1..=max are missing, fractions are
    /// truncated and user-missing values count as values.
    pub ranges: Vec<(Vec<VarItem>, usize)>,
    /// NOBSERVATIONS: only the first n cases take part.
    pub first_cases: Option<usize>,
    /// MATRIX OUT: None for the active dataset, else a file or dataset.
    pub matrix_out: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyLevel {
    MultipleNominal,
    SingleNominal,
    Ordinal,
    Numerical,
}

/// One chart: its keyword, the variables in parentheses and the label length.
pub type LegacyChart = (&'static str, Vec<VarItem>, usize);

#[derive(Debug, Clone, PartialEq)]
pub struct Overals {
    pub coding: LegacyCoding,
    pub analysis: Vec<(Vec<VarItem>, LegacyLevel)>,
    /// The number of variables in each set.
    pub sets: Vec<usize>,
    pub dimension: usize,
    pub random_start: bool,
    pub max_iterations: usize,
    pub convergence: f64,
    /// The PRINT keywords in effect (FREQ, HISTORY, FIT, CENTROID, OBJECT, QUANT, WEIGHTS).
    pub print: Vec<&'static str>,
    pub plots: Vec<LegacyChart>,
    /// NDIM: the first dimension (None for ALL) and the last (None for MAX); None without it.
    pub ndim: Option<(Option<usize>, Option<usize>)>,
    /// SAVE: the root and the number of dimensions (None for all).
    pub save: Option<(String, Option<usize>)>,
}

/// What the three variable commands read, before each is turned into its own form.
struct Legacy {
    coding: LegacyCoding,
    analysis: Option<Vec<(Vec<VarItem>, Option<LegacyLevel>)>>,
    sets: Option<Vec<usize>>,
    dimension: usize,
    random_start: bool,
    max_iterations: usize,
    convergence: f64,
    print: Option<Vec<&'static str>>,
    plots: Option<Vec<LegacyChart>>,
    ndim: Option<(Option<usize>, Option<usize>)>,
    save: Option<(Option<String>, Option<usize>)>,
}

/// The PRINT and PLOT keywords of a command and its groups.
struct Keywords {
    print: &'static [&'static str],
    print_default: &'static [&'static str],
    plot: &'static [&'static str],
    plot_default: &'static [&'static str],
    /// The plot keywords that take a variable list.
    plot_lists: &'static [&'static str],
}

const HOMALS: Keywords = Keywords {
    print: &["FREQ", "EIGEN", "DISCRIM", "QUANT", "OBJECT", "HISTORY"],
    print_default: &["FREQ", "EIGEN", "DISCRIM", "QUANT"],
    plot: &["QUANT", "OBJECT", "DISCRIM"],
    plot_default: &["QUANT", "OBJECT"],
    plot_lists: &["QUANT", "OBJECT"],
};

const PRINCALS: Keywords = Keywords {
    print: &[
        "FREQ",
        "EIGEN",
        "LOADINGS",
        "QUANT",
        "HISTORY",
        "CORRELATION",
        "OBJECT",
    ],
    print_default: &["FREQ", "EIGEN", "LOADINGS", "QUANT"],
    plot: &["QUANT", "OBJECT", "LOADINGS"],
    plot_default: &["QUANT", "OBJECT"],
    plot_lists: &["QUANT", "OBJECT"],
};

const OVERALS: Keywords = Keywords {
    print: &[
        "FREQ", "QUANT", "CENTROID", "HISTORY", "WEIGHTS", "OBJECT", "FIT",
    ],
    print_default: &["FREQ", "CENTROID", "WEIGHTS"],
    plot: &["OBJECT", "QUANT", "LOADINGS", "TRANS", "CENTROID"],
    plot_default: &["OBJECT", "LOADINGS"],
    plot_lists: &["OBJECT", "QUANT", "TRANS", "CENTROID"],
};

fn keyword(p: &mut Parser, list: &[&'static str]) -> Option<&'static str> {
    let found = list.iter().find(|k| p.at_keyword(k)).copied();
    p.pos += usize::from(found.is_some());
    found
}

fn print(p: &mut Parser, k: &Keywords) -> PResult<Vec<&'static str>> {
    let mut out: Vec<&'static str> = Vec::new();
    while !at_end(p) {
        if p.eat_keyword("DEFAULT") {
            out.extend(k.print_default);
        } else if p.eat_keyword("ALL") {
            out.extend(k.print);
        } else if p.eat_keyword("NONE") {
            out.clear();
        } else if let Some(word) = keyword(p, k.print) {
            out.push(word);
        } else {
            let mut all: Vec<&str> = k.print.to_vec();
            all.extend(["DEFAULT", "ALL", "NONE"]);
            return Err(p.unexpected(&all));
        }
        p.eat(Punct::Comma);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// `(n)` with a label length from 1 to 20; `default` without it.
fn label_length(p: &mut Parser, default: usize) -> PResult<usize> {
    if !(p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Number(_)))) {
        return Ok(default);
    }
    p.pos += 1;
    let start = p.pos;
    let n = whole(p, "a label length", 1)?;
    if n > 20 {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a label length lies from 1 to 20",
        ));
    }
    p.expect(Punct::RParen)?;
    Ok(n)
}

/// NDIM's `(first | ALL, last | MAX)`.
fn ndim(p: &mut Parser) -> PResult<(Option<usize>, Option<usize>)> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let first = match p.eat_keyword("ALL") {
        true => None,
        false => Some(whole(p, "the first dimension or ALL", 1)?),
    };
    p.eat(Punct::Comma);
    let last = match p.eat_keyword("MAX") {
        true => None,
        false => Some(whole(p, "the last dimension or MAX", 2)?),
    };
    if let (Some(a), Some(b)) = (first, last)
        && b <= a
    {
        return Err(SyntaxError::new(
            p.span_from(start),
            "NDIM needs the first dimension below the last",
        ));
    }
    p.expect(Punct::RParen)?;
    Ok((first, last))
}

type Plots = (Vec<LegacyChart>, Option<(Option<usize>, Option<usize>)>);

fn plot(p: &mut Parser, k: &Keywords, default_length: usize) -> PResult<Plots> {
    let (mut charts, mut dims): (Vec<LegacyChart>, _) = (Vec::new(), None);
    let mut any = false;
    while !at_end(p) {
        if p.eat_keyword("NDIM") {
            p.eat(Punct::Eq);
            dims = Some(ndim(p)?);
        } else if p.eat_keyword("NONE") {
            charts.clear();
            any = true;
        } else if let Some(word) = keyword(p, &["DEFAULT", "ALL"]) {
            let group = match word {
                "DEFAULT" => k.plot_default,
                _ => k.plot,
            };
            let length = label_length(p, default_length)?;
            charts.extend(group.iter().map(|w| (*w, Vec::new(), length)));
            any = true;
        } else if let Some(word) = keyword(p, k.plot) {
            let mut items = Vec::new();
            if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Word(_))) {
                if !k.plot_lists.contains(&word) {
                    return Err(SyntaxError::new(
                        p.span(),
                        format!("PLOT {word} takes no variable list"),
                    ));
                }
                p.pos += 1;
                items = p.var_list()?;
                p.expect(Punct::RParen)?;
            }
            charts.retain(|(w, _, _)| *w != word);
            charts.push((word, items, label_length(p, default_length)?));
            any = true;
        } else {
            let mut all: Vec<&str> = k.plot.to_vec();
            all.extend(["DEFAULT", "ALL", "NONE", "NDIM"]);
            return Err(p.unexpected(&all));
        }
        p.eat(Punct::Comma);
    }
    if !any {
        charts = k
            .plot_default
            .iter()
            .map(|w| (*w, Vec::new(), default_length))
            .collect();
    }
    Ok((charts, dims))
}

fn level(p: &mut Parser) -> PResult<Option<LegacyLevel>> {
    let found = [
        ("MNOM", LegacyLevel::MultipleNominal),
        ("SNOM", LegacyLevel::SingleNominal),
        ("ORDI", LegacyLevel::Ordinal),
        ("NUME", LegacyLevel::Numerical),
    ]
    .into_iter()
    .find(|(k, _)| p.at_keyword(k));
    match found {
        Some((_, level)) => {
            p.pos += 1;
            Ok(Some(level))
        }
        None => Err(p.unexpected(&["MNOM", "SNOM", "ORDI", "NUME"])),
    }
}

/// HOMALS, PRINCALS and OVERALS: the variables first, then the subcommands in any order.
fn legacy(p: &mut Parser, who: &str, k: &Keywords, levels: bool) -> PResult<Legacy> {
    let mut out = Legacy {
        coding: LegacyCoding {
            ranges: Vec::new(),
            first_cases: None,
            matrix_out: None,
        },
        analysis: None,
        sets: None,
        dimension: 2,
        random_start: false,
        max_iterations: 100,
        convergence: 0.00001,
        print: None,
        plots: None,
        ndim: None,
        save: None,
    };
    let mut own = vec![
        "VARIABLES",
        "ANALYSIS",
        "NOBSERVATIONS",
        "DIMENSIONS",
        "DIMENSION",
        "MAXITER",
        "CONVERGENCE",
        "PRINT",
        "PLOT",
        "SAVE",
        "MATRIX",
    ];
    if who == "OVERALS" {
        own.extend(["SETS", "INITIAL"]);
    }
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if !slash && !first {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        let start = p.pos;
        let name = match keyword(p, &own) {
            Some(name) => name,
            None if first => "VARIABLES",
            None => return Err(p.subcommand_error(who, &[])),
        };
        first = false;
        p.eat(Punct::Eq);
        match name {
            "VARIABLES" => {
                let lists =
                    specified_lists(p, None, |p| Ok(Some(whole(p, "the largest category", 1)?)))?;
                out.coding.ranges = Vec::new();
                for (items, max) in lists {
                    let Some(max) = max else {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "every list on VARIABLES needs its largest category in parentheses",
                        ));
                    };
                    out.coding.ranges.push((items, max));
                }
            }
            "ANALYSIS" => {
                if out.analysis.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!("ANALYSIS appears twice; {who} is not run"),
                    ));
                }
                let lists = match levels {
                    true => specified_lists(p, None, level)?,
                    false => vec![(p.var_list()?, None)],
                };
                out.analysis = Some(lists);
            }
            "SETS" => {
                let count = whole(p, "the number of sets", 2)?;
                p.expect(Punct::LParen)?;
                let mut sizes = Vec::new();
                while !p.at(Punct::RParen) {
                    sizes.push(whole(p, "the number of variables in a set", 1)?);
                    p.eat(Punct::Comma);
                }
                p.expect(Punct::RParen)?;
                if sizes.len() != count {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!("SETS names {count} sets but gives {} sizes", sizes.len()),
                    ));
                }
                out.sets = Some(sizes);
            }
            "NOBSERVATIONS" => out.coding.first_cases = Some(whole(p, "the number of cases", 1)?),
            "DIMENSION" | "DIMENSIONS" => out.dimension = whole(p, "the number of dimensions", 1)?,
            "MAXITER" => out.max_iterations = whole(p, "the most iterations", 0)?,
            "CONVERGENCE" => {
                out.convergence = match p.number() {
                    Some(v) if v > 0.0 => v,
                    Some(_) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "CONVERGENCE must be positive",
                        ));
                    }
                    None => return Err(p.unexpected(&["the convergence criterion"])),
                }
            }
            "INITIAL" => {
                out.random_start = if p.eat_keyword("RANDOM") {
                    true
                } else if p.eat_keyword("NUMERICAL") {
                    false
                } else {
                    return Err(p.unexpected(&["NUMERICAL", "RANDOM"]));
                }
            }
            "PRINT" => out.print = Some(print(p, k)?),
            "PLOT" => {
                let (charts, dims) = plot(p, k, 12)?;
                out.plots = Some(charts);
                out.ndim = dims;
            }
            "SAVE" => {
                let root = match p.peek() {
                    Some(TokenKind::Word(w)) => {
                        let w: String = w.chars().take(6).collect();
                        p.pos += 1;
                        Some(w)
                    }
                    _ => None,
                };
                let dims = match p.eat(Punct::LParen) {
                    true => {
                        let n = whole(p, "a number of dimensions", 1)?;
                        p.expect(Punct::RParen)?;
                        Some(n)
                    }
                    false => None,
                };
                out.save = Some((root, dims));
            }
            _ => {
                if !p.eat_keyword("OUT") {
                    return Err(p.unexpected(&["OUT"]));
                }
                p.eat(Punct::Eq);
                out.coding.matrix_out = Some(matrix_source(p)?);
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if out.coding.ranges.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    if let Some((_, Some(n))) = out.save
        && n > out.dimension
    {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "SAVE asks for {n} dimensions, but DIMENSION is {}",
                out.dimension
            ),
        ));
    }
    Ok(out)
}

impl Legacy {
    /// The analysis lists, every variable of VARIABLES without ANALYSIS.
    fn analysis_lists(&self) -> Vec<(Vec<VarItem>, Option<LegacyLevel>)> {
        self.analysis.clone().unwrap_or_else(|| {
            vec![(
                self.coding
                    .ranges
                    .iter()
                    .flat_map(|(items, _)| items.clone())
                    .collect(),
                None,
            )]
        })
    }

    fn common(&self) -> ScalingCommon {
        ScalingCommon {
            variables: self
                .coding
                .ranges
                .iter()
                .flat_map(|(items, _)| items.clone())
                .collect(),
            dimension: self.dimension,
            max_iterations: self.max_iterations,
            criterion: self.convergence,
            legacy: Some(self.coding.clone()),
            ..ScalingCommon::default()
        }
    }

    /// NDIM as the newer commands take it: None for every pair, MAX as the largest number.
    fn ndim(&self) -> Option<(usize, usize)> {
        match self.ndim {
            None if self.dimension > 2 => Some((1, 2)),
            None | Some((None, _)) => None,
            Some((Some(a), b)) => Some((a, b.unwrap_or(usize::MAX))),
        }
    }

    fn save_root(&self, default: &str) -> Option<(String, Option<usize>)> {
        self.save
            .as_ref()
            .map(|(root, n)| (root.clone().unwrap_or_else(|| default.to_string()), *n))
    }
}

pub(crate) fn homals(p: &mut Parser) -> PResult<Command> {
    let legacy = legacy(p, "HOMALS", &HOMALS, false)?;
    let print = legacy
        .print
        .clone()
        .unwrap_or(HOMALS.print_default.to_vec());
    let has = |k: &str| print.contains(&k);
    let mut charts: Vec<(McaPlot, Option<usize>)> = Vec::new();
    for (word, items, length) in legacy.plots.clone().unwrap_or_else(|| {
        HOMALS
            .plot_default
            .iter()
            .map(|w| (*w, Vec::new(), 12))
            .collect()
    }) {
        let chart = match word {
            "QUANT" => McaPlot::Jointcat(items),
            "OBJECT" => McaPlot::Object(items),
            _ => McaPlot::Discrim(Vec::new()),
        };
        charts.push((chart, Some(length)));
    }
    // no charts of a one-dimensional solution
    if legacy.dimension == 1 {
        charts.clear();
    }
    let analysis = legacy
        .analysis_lists()
        .into_iter()
        .map(|(items, _)| (items, 1.0))
        .collect();
    Ok(Command::Homals(Box::new(MultipleCorrespondence {
        common: legacy.common(),
        analysis,
        print: McaPrint {
            corr: false,
            descrip: has("FREQ").then(Vec::new),
            discrim: has("DISCRIM"),
            history: has("HISTORY"),
            object: has("OBJECT").then(|| (None, Vec::new())),
            ocorr: false,
            quant: has("QUANT").then(Vec::new),
        },
        plot: McaPlots {
            charts,
            ndim: legacy.ndim(),
        },
        save_trdata: None,
        save_object: legacy.save_root("HOM"),
        outfile_discrdata: None,
        outfile_trdata: None,
        outfile_object: None,
    })))
}

pub(crate) fn princals(p: &mut Parser) -> PResult<Command> {
    let legacy = legacy(p, "PRINCALS", &PRINCALS, true)?;
    let print = legacy
        .print
        .clone()
        .unwrap_or(vec!["FREQ", "EIGEN", "LOADINGS"]);
    let has = |k: &str| print.contains(&k);
    let lists = legacy.analysis_lists();
    let all: Vec<VarItem> = lists.iter().flat_map(|(items, _)| items.clone()).collect();
    let mut charts: Vec<(CatpcaPlot, Option<usize>)> = Vec::new();
    for (word, items, length) in legacy.plots.clone().unwrap_or_else(|| {
        PRINCALS
            .plot_default
            .iter()
            .map(|w| (*w, Vec::new(), 12))
            .collect()
    }) {
        let chart = match word {
            "QUANT" if items.is_empty() => CatpcaPlot::Category(all.clone()),
            "QUANT" => CatpcaPlot::Category(items),
            "OBJECT" => CatpcaPlot::Object(items),
            _ => CatpcaPlot::Loading(Vec::new(), None),
        };
        charts.push((chart, Some(length)));
    }
    if legacy.dimension == 1 {
        charts.clear();
    }
    let analysis = lists
        .into_iter()
        .map(|(items, level)| {
            let level = match level.unwrap_or(LegacyLevel::Ordinal) {
                LegacyLevel::MultipleNominal => ScalingLevel::MultipleNominal,
                LegacyLevel::SingleNominal => ScalingLevel::Nominal,
                LegacyLevel::Ordinal => ScalingLevel::Ordinal,
                LegacyLevel::Numerical => ScalingLevel::Numeric,
            };
            (items, CatpcaVariable { weight: 1.0, level })
        })
        .collect();
    Ok(Command::Princals(Box::new(Catpca {
        common: legacy.common(),
        analysis,
        rotation: None,
        resample: None,
        print: CatpcaPrint {
            descrip: has("FREQ").then(Vec::new),
            loading: has("LOADINGS").then_some(false),
            corr: has("CORRELATION"),
            vaf: false,
            ocorr: false,
            quant: has("QUANT").then(Vec::new),
            history: has("HISTORY"),
            object: has("OBJECT").then(|| (None, Vec::new())),
        },
        plot: CatpcaPlots {
            charts,
            ndim: legacy.ndim(),
            ellipse: Default::default(),
        },
        save_trdata: None,
        save_object: legacy.save_root("PRI"),
        save_approx: None,
        save_areas: Default::default(),
        ellipses: Vec::new(),
        outfile_discrdata: None,
        outfile_trdata: None,
        outfile_object: None,
        outfile_approx: None,
        outfile_ellcoord: None,
    })))
}

pub(crate) fn overals(p: &mut Parser) -> PResult<Command> {
    let legacy = legacy(p, "OVERALS", &OVERALS, true)?;
    let Some(analysis) = legacy.analysis.clone() else {
        return Err(SyntaxError::new(
            p.span(),
            "OVERALS needs the ANALYSIS subcommand",
        ));
    };
    let Some(sets) = legacy.sets.clone() else {
        return Err(SyntaxError::new(
            p.span(),
            "OVERALS needs the SETS subcommand",
        ));
    };
    let plots = legacy.plots.clone().unwrap_or_else(|| {
        OVERALS
            .plot_default
            .iter()
            .map(|w| (*w, Vec::new(), 12))
            .collect()
    });
    Ok(Command::Overals(Box::new(Overals {
        analysis: analysis
            .into_iter()
            .map(|(items, level)| (items, level.unwrap_or(LegacyLevel::Ordinal)))
            .collect(),
        sets,
        dimension: legacy.dimension,
        random_start: legacy.random_start,
        max_iterations: legacy.max_iterations,
        convergence: legacy.convergence,
        print: legacy
            .print
            .clone()
            .unwrap_or(OVERALS.print_default.to_vec()),
        plots,
        ndim: legacy.ndim,
        save: legacy.save_root("OVE"),
        coding: legacy.coding,
    })))
}

pub(crate) fn anacor(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if !p.eat_keyword("TABLE") {
        return Err(SyntaxError::new(
            p.span(),
            "ANACOR starts with the TABLE subcommand",
        ));
    }
    let none = CorrespondencePrint {
        table: false,
        rpoints: false,
        cpoints: false,
        ..CorrespondencePrint::default()
    };
    let mut command = Correspondence {
        table: table(p)?,
        supplementary_rows: Vec::new(),
        supplementary_columns: Vec::new(),
        equal_rows: Vec::new(),
        equal_columns: Vec::new(),
        measure: CorrespondenceMeasure::Chisq,
        standardize: CorrespondenceStandardize::Rcmean,
        dimension: None,
        normalization: CorrespondenceNormalization::Q(0.0),
        print: CorrespondencePrint::default(),
        plot: CorrespondencePlot {
            rpoints: Some(Some(3)),
            cpoints: Some(Some(3)),
            biplot: None,
            ndim: Some((1, Some(2))),
            ..CorrespondencePlot::default()
        },
        outfile_score: None,
        outfile_variance: None,
    };
    let (mut rconf, mut cconf) = (false, false);
    if !at_end(p) {
        return Err(p.unexpected(&["'/'", "end of command"]));
    }
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("TABLE") {
            return Err(SyntaxError::new(
                p.span_from(start),
                "TABLE is given only once, as the first subcommand",
            ));
        } else if p.eat_keyword("DIMENSION") {
            p.eat(Punct::Eq);
            command.dimension = Some(whole(p, "the number of dimensions", 1)?);
        } else if p.eat_keyword("NORMALIZATION") {
            p.eat(Punct::Eq);
            let at = p.pos;
            command.normalization = if p.eat_keyword("CANONICAL") || p.eat_keyword("DEFAULT") {
                CorrespondenceNormalization::Q(0.0)
            } else if p.eat_keyword("PRINCIPAL") {
                CorrespondenceNormalization::Principal
            } else if p.eat_keyword("RPRINCIPAL") {
                CorrespondenceNormalization::Q(1.0)
            } else if p.eat_keyword("CPRINCIPAL") {
                CorrespondenceNormalization::Q(-1.0)
            } else if let Some(q) = p.number() {
                if !(-2.0..=2.0).contains(&q) {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "a NORMALIZATION value lies from -2 to 2",
                    ));
                }
                CorrespondenceNormalization::Q(q)
            } else {
                return Err(p.unexpected(&[
                    "CANONICAL",
                    "PRINCIPAL",
                    "RPRINCIPAL",
                    "CPRINCIPAL",
                    "a value from -2 to 2",
                ]));
            };
        } else if p.eat_keyword("VARIANCES") {
            p.eat(Punct::Eq);
            (rconf, cconf) = (false, false);
            while !at_end(p) {
                if p.eat_keyword("ROWS") {
                    rconf = true;
                } else if p.eat_keyword("COLUMNS") {
                    cconf = true;
                } else if !p.eat_keyword("SINGULAR") {
                    // the summary always shows the spread of the singular values
                    return Err(p.unexpected(&["SINGULAR", "ROWS", "COLUMNS"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            let mut print = none;
            while !at_end(p) {
                if p.eat_keyword("TABLE") {
                    print.table = true;
                } else if p.eat_keyword("PROFILES") {
                    (print.rprofiles, print.cprofiles) = (true, true);
                } else if p.eat_keyword("SCORES") || p.eat_keyword("CONTRIBUTIONS") {
                    (print.rpoints, print.cpoints) = (true, true);
                } else if p.eat_keyword("PERMUTATION") {
                    print.permutation = Some(usize::MAX);
                } else if p.eat_keyword("DEFAULT") {
                    (print.table, print.rpoints, print.cpoints) = (true, true, true);
                } else if p.eat_keyword("NONE") {
                    print = none;
                } else {
                    return Err(p.unexpected(&[
                        "TABLE",
                        "PROFILES",
                        "SCORES",
                        "CONTRIBUTIONS",
                        "PERMUTATION",
                        "DEFAULT",
                        "NONE",
                    ]));
                }
                p.eat(Punct::Comma);
            }
            command.print = print;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            let mut plot = CorrespondencePlot {
                biplot: None,
                ndim: Some((1, Some(2))),
                given: true,
                ..CorrespondencePlot::default()
            };
            let mut any = false;
            while !at_end(p) {
                if p.eat_keyword("NDIM") {
                    p.eat(Punct::Eq);
                    plot.ndim = match ndim(p)? {
                        (None, _) => None,
                        (Some(a), b) => Some((a, b)),
                    };
                    p.eat(Punct::Comma);
                    continue;
                }
                any = true;
                if p.eat_keyword("ROWS") {
                    plot.rpoints = Some(Some(label_length(p, 3)?));
                } else if p.eat_keyword("COLUMNS") {
                    plot.cpoints = Some(Some(label_length(p, 3)?));
                } else if p.eat_keyword("DEFAULT") {
                    let n = label_length(p, 3)?;
                    (plot.rpoints, plot.cpoints) = (Some(Some(n)), Some(Some(n)));
                } else if p.eat_keyword("JOINT") {
                    plot.biplot = Some(Some(label_length(p, 3)?));
                } else if p.eat_keyword("TRROWS") {
                    plot.trrows = Some(Some(20));
                } else if p.eat_keyword("TRCOLUMNS") {
                    plot.trcolumns = Some(Some(20));
                } else if p.eat_keyword("NONE") {
                    plot = CorrespondencePlot {
                        biplot: None,
                        ndim: plot.ndim,
                        given: true,
                        ..CorrespondencePlot::default()
                    };
                } else {
                    return Err(p.unexpected(&[
                        "ROWS",
                        "COLUMNS",
                        "JOINT",
                        "TRROWS",
                        "TRCOLUMNS",
                        "DEFAULT",
                        "NONE",
                        "NDIM",
                    ]));
                }
                p.eat(Punct::Comma);
            }
            if !any {
                (plot.rpoints, plot.cpoints) = (Some(Some(3)), Some(Some(3)));
            }
            command.plot = plot;
        } else if p.eat_keyword("MATRIX") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("OUT") {
                return Err(p.unexpected(&["OUT"]));
            }
            p.eat(Punct::Eq);
            let (mut score, mut variance) = (None, None);
            while !at_end(p) {
                let which = if p.eat_keyword("SCORE") {
                    &mut score
                } else if p.eat_keyword("VARIANCE") {
                    &mut variance
                } else {
                    return Err(p.unexpected(&["SCORE", "VARIANCE"]));
                };
                // '*' stands for the active dataset
                *which = Some(matrix_source(p)?.unwrap_or_else(|| "*".into()));
                p.eat(Punct::Comma);
            }
            if score.is_none() && variance.is_none() {
                return Err(p.unexpected(&["SCORE", "VARIANCE"]));
            }
            if let (Some(a), Some(b)) = (&score, &variance)
                && a.eq_ignore_ascii_case(b)
            {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "MATRIX OUT needs different files for SCORE and VARIANCE",
                ));
            }
            command.outfile_score = score;
            command.outfile_variance = variance;
        } else {
            return Err(p.subcommand_error("ANACOR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    (command.print.rconf, command.print.cconf) = (rconf, cconf);
    p.finish(&["'/'"])?;
    Ok(Command::Anacor(Box::new(command)))
}
