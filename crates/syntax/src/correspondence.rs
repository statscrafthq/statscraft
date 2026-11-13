//! Parser of CORRESPONDENCE: simple correspondence analysis of a two-way table.
//!
//! Rules from the Command Syntax Reference (CORRESPONDENCE), in our words:
//! - TABLE comes first and is required: `rowvar(min,max) BY colvar(min,max)` with whole-number
//!   ranges, or `ALL(rows,columns)` when the cases are the rows of a table and the variables its
//!   columns. The other subcommands follow in any order; a repeated one replaces the earlier one.
//! - SUPPLEMENTARY and EQUAL name the row or the column variable (ROW or COLUMN after ALL) with
//!   values in parentheses: category values for variables, positions for ALL. EQUAL takes one or
//!   more groups of at least two values each; SUPPLEMENTARY one or more values.
//! - MEASURE: CHISQ (default) or EUCLID. STANDARDIZE: RMEAN, CMEAN, RCMEAN (default), RSUM or
//!   CSUM; CHISQ allows only RCMEAN. DIMENSION: a whole number, 2 by default.
//! - NORMALIZATION: SYMMETRICAL (default, 0), PRINCIPAL, RPRINCIPAL (1), CPRINCIPAL (-1) or a
//!   value from -1 to 1.
//! - PRINT: TABLE, RPROFILES, CPROFILES, RPOINTS, CPOINTS, RCONF, CCONF, PERMUTATION[(n)],
//!   DEFAULT (TABLE RPOINTS CPOINTS) and NONE. The summary always shows.
//! - PLOT: TRROWS, TRCOLUMNS, RPOINTS, CPOINTS, BIPLOT (the default), each with an optional
//!   label length up to 60, NONE, and NDIM(first, last | MAX).
//! - OUTFILE: SCORE('file') and VARIANCE('file'), each a file or a dataset name.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub enum CorrespondenceTable {
    Variables {
        row: String,
        row_range: (f64, f64),
        column: String,
        column_range: (f64, f64),
    },
    /// The cases are the rows, the variables the columns.
    All { rows: usize, columns: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrespondenceMeasure {
    Chisq,
    Euclid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrespondenceStandardize {
    Rmean,
    Cmean,
    Rcmean,
    Rsum,
    Csum,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CorrespondenceNormalization {
    /// Both rows and columns in principal coordinates.
    Principal,
    /// The share of each singular value given to the rows: 1 row principal, 0 symmetrical,
    /// -1 column principal.
    Q(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrespondencePrint {
    pub table: bool,
    pub rprofiles: bool,
    pub cprofiles: bool,
    pub rpoints: bool,
    pub cpoints: bool,
    pub rconf: bool,
    pub cconf: bool,
    /// Permuted tables for the first n dimensions.
    pub permutation: Option<usize>,
}

impl Default for CorrespondencePrint {
    fn default() -> Self {
        Self {
            table: true,
            rprofiles: false,
            cprofiles: false,
            rpoints: true,
            cpoints: true,
            rconf: false,
            cconf: false,
            permutation: None,
        }
    }
}

/// A requested plot with its label length: None shows labels in full.
pub type PlotLabels = Option<Option<usize>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrespondencePlot {
    pub trrows: PlotLabels,
    pub trcolumns: PlotLabels,
    pub rpoints: PlotLabels,
    pub cpoints: PlotLabels,
    pub biplot: PlotLabels,
    /// NDIM: the first dimension and the last (None for MAX); None without NDIM.
    pub ndim: Option<(usize, Option<usize>)>,
}

impl Default for CorrespondencePlot {
    fn default() -> Self {
        Self {
            trrows: None,
            trcolumns: None,
            rpoints: None,
            cpoints: None,
            biplot: Some(None),
            ndim: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Correspondence {
    pub table: CorrespondenceTable,
    /// Values (or positions with ALL) of supplementary rows and columns.
    pub supplementary_rows: Vec<f64>,
    pub supplementary_columns: Vec<f64>,
    /// Groups of rows and of columns restricted to equal scores.
    pub equal_rows: Vec<Vec<f64>>,
    pub equal_columns: Vec<Vec<f64>>,
    pub measure: CorrespondenceMeasure,
    pub standardize: CorrespondenceStandardize,
    pub dimension: usize,
    pub normalization: CorrespondenceNormalization,
    pub print: CorrespondencePrint,
    pub plot: CorrespondencePlot,
    pub outfile_score: Option<String>,
    pub outfile_variance: Option<String>,
}

/// A whole number in parentheses, or None without the parenthesis.
fn optional_count(p: &mut Parser, what: &str) -> PResult<Option<usize>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let n = whole(p, what)?;
    p.expect(Punct::RParen)?;
    Ok(Some(n))
}

fn whole(p: &mut Parser, what: &str) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 0.0 && n.fract() == 0.0 && n < 1e9 => Ok(n as usize),
        Some(_) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number of 0 or more"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// `(min, max)` after a TABLE variable.
fn range(p: &mut Parser) -> PResult<(f64, f64)> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let min = p.number();
    p.eat(Punct::Comma);
    let max = p.number();
    let (Some(min), Some(max)) = (min, max) else {
        return Err(p.unexpected(&["the smallest and the largest category value"]));
    };
    if min.fract() != 0.0 || max.fract() != 0.0 || min > max {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("a TABLE range is two whole numbers, the smaller first; got {min} and {max}"),
        ));
    }
    if max - min >= 10_000.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a TABLE range holds at most 10000 categories",
        ));
    }
    p.expect(Punct::RParen)?;
    Ok((min, max))
}

fn table(p: &mut Parser) -> PResult<CorrespondenceTable> {
    p.eat(Punct::Eq);
    if p.eat_word("ALL") {
        p.expect(Punct::LParen)?;
        let rows = whole(p, "the number of rows")?;
        p.eat(Punct::Comma);
        let columns = whole(p, "the number of columns")?;
        p.expect(Punct::RParen)?;
        return Ok(CorrespondenceTable::All { rows, columns });
    }
    let row = p.name("the row variable")?;
    let row_range = range(p)?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let column = p.name("the column variable")?;
    let column_range = range(p)?;
    if row.eq_ignore_ascii_case(&column) {
        return Err(SyntaxError::new(
            p.span(),
            "TABLE needs two different variables",
        ));
    }
    Ok(CorrespondenceTable::Variables {
        row,
        row_range,
        column,
        column_range,
    })
}

/// One or more parenthesized value lists.
fn value_groups(p: &mut Parser) -> PResult<Vec<Vec<f64>>> {
    let mut groups = Vec::new();
    while p.eat(Punct::LParen) {
        let mut values = Vec::new();
        while let Some(v) = p.number() {
            values.push(v);
            p.eat(Punct::Comma);
        }
        if values.is_empty() {
            return Err(p.unexpected(&["a value"]));
        }
        p.expect(Punct::RParen)?;
        groups.push(values);
    }
    if groups.is_empty() {
        return Err(p.unexpected(&["'('"]));
    }
    Ok(groups)
}

/// SUPPLEMENTARY and EQUAL: the row and column groups, named by the TABLE variables or by
/// ROW and COLUMN after ALL.
type Groups = (Vec<Vec<f64>>, Vec<Vec<f64>>);

fn sides(p: &mut Parser, table: &CorrespondenceTable, sub: &str) -> PResult<Groups> {
    p.eat(Punct::Eq);
    let (mut rows, mut columns) = (Vec::new(), Vec::new());
    while !at_end(p) {
        let start = p.pos;
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&["the row or the column variable"]));
        };
        let is_row = match table {
            CorrespondenceTable::All { .. } => {
                if word.eq_ignore_ascii_case("ROW") {
                    true
                } else if word.eq_ignore_ascii_case("COLUMN") {
                    false
                } else {
                    return Err(p.unexpected(&["ROW", "COLUMN"]));
                }
            }
            CorrespondenceTable::Variables { row, column, .. } => {
                if word.eq_ignore_ascii_case(row) {
                    true
                } else if word.eq_ignore_ascii_case(column) {
                    false
                } else {
                    return Err(p.unexpected(&[row.as_str(), column.as_str()]));
                }
            }
        };
        p.pos += 1;
        let groups = value_groups(p)?;
        if sub == "EQUAL" && groups.iter().any(|g| g.len() < 2) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "each EQUAL group needs at least two values",
            ));
        }
        if is_row {
            rows.extend(groups);
        } else {
            columns.extend(groups);
        }
    }
    Ok((rows, columns))
}

fn print(p: &mut Parser) -> PResult<CorrespondencePrint> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(CorrespondencePrint::default());
    }
    let mut out = CorrespondencePrint {
        table: false,
        rpoints: false,
        cpoints: false,
        ..CorrespondencePrint::default()
    };
    while !at_end(p) {
        if p.eat_keyword("TABLE") {
            out.table = true;
        } else if p.eat_keyword("RPROFILES") {
            out.rprofiles = true;
        } else if p.eat_keyword("CPROFILES") {
            out.cprofiles = true;
        } else if p.eat_keyword("RPOINTS") {
            out.rpoints = true;
        } else if p.eat_keyword("CPOINTS") {
            out.cpoints = true;
        } else if p.eat_keyword("RCONF") {
            out.rconf = true;
        } else if p.eat_keyword("CCONF") {
            out.cconf = true;
        } else if p.eat_keyword("PERMUTATION") {
            let n = optional_count(p, "the number of dimensions")?.unwrap_or(1);
            out.permutation = Some(n.max(1));
        } else if p.eat_keyword("DEFAULT") {
            out.table = true;
            out.rpoints = true;
            out.cpoints = true;
        } else if p.eat_keyword("NONE") {
            out = CorrespondencePrint {
                table: false,
                rpoints: false,
                cpoints: false,
                ..CorrespondencePrint::default()
            };
        } else {
            return Err(p.unexpected(&[
                "TABLE",
                "RPROFILES",
                "CPROFILES",
                "RPOINTS",
                "CPOINTS",
                "RCONF",
                "CCONF",
                "PERMUTATION",
                "DEFAULT",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn plot(p: &mut Parser) -> PResult<CorrespondencePlot> {
    p.eat(Punct::Eq);
    let mut out = CorrespondencePlot {
        biplot: None,
        ..CorrespondencePlot::default()
    };
    let mut any = false;
    let length = |p: &mut Parser| -> PResult<PlotLabels> {
        Ok(Some(
            optional_count(p, "a label length")?.map(|n| n.min(60)),
        ))
    };
    while !at_end(p) {
        if p.eat_keyword("NDIM") {
            out.ndim = Some((1, None));
            if p.eat(Punct::LParen) {
                if p.eat(Punct::RParen) {
                    continue;
                }
                let start = p.pos;
                let first = whole(p, "the first dimension")?;
                p.eat(Punct::Comma);
                let last = if p.eat_keyword("MAX") {
                    None
                } else {
                    Some(whole(p, "the last dimension or MAX")?)
                };
                if first < 1 || last.is_some_and(|l| l <= first) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "NDIM needs a first dimension of 1 or more and a larger last one",
                    ));
                }
                p.expect(Punct::RParen)?;
                out.ndim = Some((first, last));
            }
            p.eat(Punct::Comma);
            continue;
        }
        any = true;
        if p.eat_keyword("TRROWS") {
            out.trrows = length(p)?;
        } else if p.eat_keyword("TRCOLUMNS") {
            out.trcolumns = length(p)?;
        } else if p.eat_keyword("RPOINTS") {
            out.rpoints = length(p)?;
        } else if p.eat_keyword("CPOINTS") {
            out.cpoints = length(p)?;
        } else if p.eat_keyword("BIPLOT") {
            out.biplot = length(p)?;
        } else if p.eat_keyword("NONE") {
            out = CorrespondencePlot {
                biplot: None,
                ndim: out.ndim,
                ..CorrespondencePlot::default()
            };
        } else {
            return Err(p.unexpected(&[
                "TRROWS",
                "TRCOLUMNS",
                "RPOINTS",
                "CPOINTS",
                "BIPLOT",
                "NONE",
                "NDIM",
            ]));
        }
        p.eat(Punct::Comma);
    }
    if !any {
        out.biplot = Some(None);
    }
    Ok(out)
}

pub(crate) fn correspondence(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if !p.eat_keyword("TABLE") {
        return Err(SyntaxError::new(
            p.span(),
            "CORRESPONDENCE starts with the TABLE subcommand",
        ));
    }
    let mut command = Correspondence {
        table: table(p)?,
        supplementary_rows: Vec::new(),
        supplementary_columns: Vec::new(),
        equal_rows: Vec::new(),
        equal_columns: Vec::new(),
        measure: CorrespondenceMeasure::Chisq,
        standardize: CorrespondenceStandardize::Rcmean,
        dimension: 2,
        normalization: CorrespondenceNormalization::Q(0.0),
        print: CorrespondencePrint::default(),
        plot: CorrespondencePlot::default(),
        outfile_score: None,
        outfile_variance: None,
    };
    let mut standardize_at = None;
    if !at_end(p) {
        return Err(p.unexpected(&["'/'", "end of command"]));
    }
    while p.eat(Punct::Slash) {
        if p.eat_keyword("TABLE") {
            return Err(SyntaxError::new(
                p.span(),
                "TABLE is given only once, as the first subcommand",
            ));
        } else if p.eat_keyword("SUPPLEMENTARY") {
            let (rows, columns) = sides(p, &command.table, "SUPPLEMENTARY")?;
            command.supplementary_rows = rows.concat();
            command.supplementary_columns = columns.concat();
        } else if p.eat_keyword("EQUAL") {
            let (rows, columns) = sides(p, &command.table, "EQUAL")?;
            command.equal_rows = rows;
            command.equal_columns = columns;
        } else if p.eat_keyword("MEASURE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("CHISQ") {
                command.measure = CorrespondenceMeasure::Chisq;
            } else if p.eat_keyword("EUCLID") {
                command.measure = CorrespondenceMeasure::Euclid;
            } else {
                return Err(p.unexpected(&["CHISQ", "EUCLID"]));
            }
        } else if p.eat_keyword("STANDARDIZE") {
            p.eat(Punct::Eq);
            standardize_at = Some(p.span());
            const KINDS: [(&str, CorrespondenceStandardize); 5] = [
                ("RMEAN", CorrespondenceStandardize::Rmean),
                ("CMEAN", CorrespondenceStandardize::Cmean),
                ("RCMEAN", CorrespondenceStandardize::Rcmean),
                ("RSUM", CorrespondenceStandardize::Rsum),
                ("CSUM", CorrespondenceStandardize::Csum),
            ];
            match KINDS.iter().find(
                |(n, _)| matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(n)),
            ) {
                Some((_, kind)) => {
                    p.pos += 1;
                    command.standardize = *kind;
                }
                None => return Err(p.unexpected(&KINDS.map(|(n, _)| n))),
            }
        } else if p.eat_keyword("DIMENSION") {
            p.eat(Punct::Eq);
            let start = p.pos;
            let n = whole(p, "the number of dimensions")?;
            if n < 1 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "DIMENSION is at least 1",
                ));
            }
            command.dimension = n;
        } else if p.eat_keyword("NORMALIZATION") {
            p.eat(Punct::Eq);
            let start = p.pos;
            command.normalization = if p.eat_keyword("SYMMETRICAL") {
                CorrespondenceNormalization::Q(0.0)
            } else if p.eat_keyword("PRINCIPAL") {
                CorrespondenceNormalization::Principal
            } else if p.eat_keyword("RPRINCIPAL") {
                CorrespondenceNormalization::Q(1.0)
            } else if p.eat_keyword("CPRINCIPAL") {
                CorrespondenceNormalization::Q(-1.0)
            } else if let Some(q) = p.number() {
                if !(-1.0..=1.0).contains(&q) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "a NORMALIZATION value lies from -1 to 1",
                    ));
                }
                CorrespondenceNormalization::Q(q)
            } else {
                return Err(p.unexpected(&[
                    "SYMMETRICAL",
                    "PRINCIPAL",
                    "RPRINCIPAL",
                    "CPRINCIPAL",
                    "a value from -1 to 1",
                ]));
            };
        } else if p.eat_keyword("PRINT") {
            command.print = print(p)?;
        } else if p.eat_keyword("PLOT") {
            command.plot = plot(p)?;
        } else if p.eat_keyword("OUTFILE") {
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
                p.expect(Punct::LParen)?;
                *which = Some(p.file("a file or dataset name")?);
                p.expect(Punct::RParen)?;
                p.eat(Punct::Comma);
            }
            if score.is_none() && variance.is_none() {
                return Err(p.unexpected(&["SCORE", "VARIANCE"]));
            }
            if score.is_some() && score == variance {
                return Err(SyntaxError::new(
                    p.span(),
                    "OUTFILE needs different names for SCORE and VARIANCE",
                ));
            }
            command.outfile_score = score;
            command.outfile_variance = variance;
        } else {
            return Err(p.subcommand_error("CORRESPONDENCE", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if let Some(span) = standardize_at
        && command.measure == CorrespondenceMeasure::Chisq
        && command.standardize != CorrespondenceStandardize::Rcmean
    {
        return Err(SyntaxError::new(
            span,
            "with MEASURE=CHISQ only STANDARDIZE=RCMEAN applies; use MEASURE=EUCLID for the others",
        ));
    }
    p.finish(&["'/'"])?;
    Ok(Command::Correspondence(Box::new(command)))
}
