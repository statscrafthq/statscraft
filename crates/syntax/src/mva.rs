//! Parser of MVA, missing value analysis.
//!
//! Rules from the Command Syntax Reference (MVA), in our words:
//! - `[VARIABLES=] varlist | ALL` comes first. CATEGORICAL=varlist, MAXCAT=n (default 25) and
//!   ID=var follow it in any order, before every other subcommand.
//! - NOUNIVARIATE turns the univariate table off.
//! - TTEST [PERCENT=n] [T | NOT] [DF | NODF] [PROB | NOPROB] [COUNTS | NOCOUNTS]
//!   [MEANS | NOMEANS]: PERCENT defaults to 5; PROB is off by default, the rest on.
//! - CROSSTAB [PERCENT=n] (default 5). MISMATCH [PERCENT=n] (default 5) [NOSORT].
//! - DPATTERN [SORT=var[(ASCENDING | DESCENDING)] ...] [DESCRIBE=varlist].
//!   MPATTERN [NOSORT] [DESCRIBE=varlist]. TPATTERN [NOSORT] [DESCRIBE=varlist]
//!   [PERCENT=n] (default 1). Every PERCENT lies from 0 to 100.
//! - LISTWISE, PAIRWISE.
//! - EM [predicted] [WITH predictors] [(TOLERANCE=0.001 CONVERGENCE=0.0001 ITERATIONS=25
//!   TDF=n | LAMBDA=a PROPORTION=b, OUTFILE='file')]: TDF a whole number of at least 2,
//!   LAMBDA positive, PROPORTION between 0 and 1, CONVERGENCE between 0 and 1.
//! - REGRESSION [predicted] [WITH predictors] [(TOLERANCE=0.001 FLIMIT=4 NPREDICTORS=n
//!   ADDTYPE=RESIDUAL | NORMAL | T[(5)] | NONE, OUTFILE='file')].
//! - A repeated subcommand: the last one counts. VARIABLES, SORT, NOSORT, DESCRIBE and WITH
//!   are reserved inside MVA and cannot name variables there.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

/// TTEST: which rows the table shows and which indicator variables it keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct MvaTtest {
    pub percent: f64,
    pub t: bool,
    pub df: bool,
    pub prob: bool,
    pub counts: bool,
    pub means: bool,
}

impl Default for MvaTtest {
    fn default() -> Self {
        Self {
            percent: 5.0,
            t: true,
            df: true,
            prob: false,
            counts: true,
            means: true,
        }
    }
}

/// DPATTERN, MPATTERN or TPATTERN.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MvaPattern {
    /// DPATTERN SORT: variables, true for DESCENDING.
    pub sort: Vec<(String, bool)>,
    pub nosort: bool,
    pub describe: Vec<VarItem>,
    /// TPATTERN PERCENT.
    pub percent: f64,
}

/// The random term REGRESSION adds to each estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddType {
    Residual,
    Normal,
    /// Student's t with these degrees of freedom.
    T(f64),
    None,
}

/// EM or REGRESSION.
#[derive(Debug, Clone, PartialEq)]
pub struct MvaEstimate {
    /// The variables to estimate; empty for every quantitative one.
    pub predicted: Vec<VarItem>,
    /// After WITH; empty for every quantitative one.
    pub predictors: Vec<VarItem>,
    pub tolerance: f64,
    /// EM only.
    pub convergence: f64,
    pub iterations: usize,
    pub tdf: Option<f64>,
    pub lambda: Option<f64>,
    pub proportion: Option<f64>,
    /// REGRESSION only.
    pub flimit: f64,
    pub npredictors: Option<usize>,
    /// None: RESIDUAL, or NORMAL when fewer than half the cases are complete.
    pub addtype: Option<AddType>,
    pub outfile: Option<String>,
}

impl Default for MvaEstimate {
    fn default() -> Self {
        Self {
            predicted: Vec::new(),
            predictors: Vec::new(),
            tolerance: 0.001,
            convergence: 0.0001,
            iterations: 25,
            tdf: None,
            lambda: None,
            proportion: None,
            flimit: 4.0,
            npredictors: None,
            addtype: None,
            outfile: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mva {
    pub variables: Vec<VarItem>,
    pub categorical: Vec<VarItem>,
    pub maxcat: usize,
    pub id: Option<String>,
    pub univariate: bool,
    pub ttest: Option<MvaTtest>,
    /// CROSSTAB PERCENT.
    pub crosstab: Option<f64>,
    /// MISMATCH PERCENT and NOSORT.
    pub mismatch: Option<(f64, bool)>,
    pub dpattern: Option<MvaPattern>,
    pub mpattern: Option<MvaPattern>,
    pub tpattern: Option<MvaPattern>,
    pub listwise: bool,
    pub pairwise: bool,
    pub em: Option<MvaEstimate>,
    pub regression: Option<MvaEstimate>,
}

const RESERVED: [&str; 5] = ["VARIABLES", "SORT", "NOSORT", "DESCRIBE", "WITH"];

/// True at a word that ends a variable list inside MVA: a reserved word, or PERCENT=.
fn at_list_end(p: &Parser) -> bool {
    match p.peek() {
        Some(TokenKind::Word(w)) => {
            let eq = matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)));
            let number = matches!(p.peek_at(1), Some(TokenKind::Number(_)));
            RESERVED.iter().any(|r| w.eq_ignore_ascii_case(r))
                || ((eq || number) && matches_keyword(w, "PERCENT"))
                || (eq && ["SORT", "DESCRIBE"].iter().any(|k| matches_keyword(w, k)))
        }
        _ => true,
    }
}

/// A variable list that stops at the reserved words of MVA.
fn list(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while !at_list_end(p) {
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            if p.eat_word("TO") {
                items.push(VarItem::Range(first, p.name("a variable name")?));
            } else {
                items.push(VarItem::Name(first));
            }
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

/// `[=] n` with 0 <= n <= 100.
fn percent(p: &mut Parser) -> PResult<f64> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a percentage"]));
    };
    if !(0.0..=100.0).contains(&n) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("PERCENT takes a percentage from 0 to 100, got {n}"),
        ));
    }
    Ok(n)
}

/// `[=] number` checked by `ok`, which names the allowed range.
fn number(p: &mut Parser, what: &str, ok: fn(f64) -> bool, range: &str) -> PResult<f64> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    if !ok(n) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes {range}, got {n}"),
        ));
    }
    Ok(n)
}

fn pattern(p: &mut Parser, dpattern: bool, percent_default: f64) -> PResult<MvaPattern> {
    let mut pattern = MvaPattern {
        percent: percent_default,
        ..MvaPattern::default()
    };
    while !at_end(p) {
        if dpattern && p.eat_keyword("SORT") {
            p.eat(Punct::Eq);
            pattern.sort.clear();
            while matches!(p.peek(), Some(TokenKind::Word(_))) && !at_list_end(p) {
                let name = p.name("a variable name")?;
                let mut descending = false;
                if p.eat(Punct::LParen) {
                    if p.eat_keyword("DESCENDING") {
                        descending = true;
                    } else if !p.eat_keyword("ASCENDING") {
                        return Err(p.unexpected(&["ASCENDING", "DESCENDING"]));
                    }
                    p.expect(Punct::RParen)?;
                }
                pattern.sort.push((name, descending));
                p.eat(Punct::Comma);
            }
            if pattern.sort.is_empty() {
                return Err(p.unexpected(&["a variable name"]));
            }
        } else if !dpattern && p.eat_keyword("NOSORT") {
            pattern.nosort = true;
        } else if p.eat_keyword("DESCRIBE") {
            p.eat(Punct::Eq);
            pattern.describe = list(p)?;
        } else if percent_default > 0.0 && p.eat_keyword("PERCENT") {
            pattern.percent = percent(p)?;
        } else {
            let mut expected = vec!["DESCRIBE"];
            expected.insert(0, if dpattern { "SORT" } else { "NOSORT" });
            if percent_default > 0.0 {
                expected.push("PERCENT");
            }
            return Err(p.unexpected(&expected));
        }
    }
    Ok(pattern)
}

fn estimate(p: &mut Parser, em: bool) -> PResult<MvaEstimate> {
    let mut estimate = MvaEstimate::default();
    if !at_list_end(p) {
        estimate.predicted = list(p)?;
    }
    if p.eat_word("WITH") {
        estimate.predictors = list(p)?;
    }
    let open = p.pos;
    if !p.eat(Punct::LParen) {
        return Ok(estimate);
    }
    let positive = |n: f64| n > 0.0;
    let unit = |n: f64| n > 0.0 && n < 1.0;
    while !p.eat(Punct::RParen) {
        if p.peek().is_none() {
            return Err(p.unexpected(&["')'"]));
        }
        if p.eat_keyword("TOLERANCE") {
            estimate.tolerance = number(p, "TOLERANCE", unit, "a number between 0 and 1")?;
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            estimate.outfile = Some(p.file("a file name")?);
        } else if em && p.eat_keyword("CONVERGENCE") {
            estimate.convergence = number(p, "CONVERGENCE", unit, "a number between 0 and 1")?;
        } else if em && p.eat_keyword("ITERATIONS") {
            let whole = |n: f64| n >= 1.0 && n.fract() == 0.0;
            estimate.iterations =
                number(p, "ITERATIONS", whole, "a whole number of at least 1")? as usize;
        } else if em && p.eat_word("TDF") {
            let df = |n: f64| n >= 2.0 && n.fract() == 0.0;
            estimate.tdf = Some(number(p, "TDF", df, "a whole number of at least 2")?);
        } else if em && p.eat_keyword("LAMBDA") {
            estimate.lambda = Some(number(p, "LAMBDA", positive, "a positive number")?);
        } else if em && p.eat_keyword("PROPORTION") {
            estimate.proportion = Some(number(p, "PROPORTION", unit, "a number between 0 and 1")?);
        } else if !em && p.eat_keyword("FLIMIT") {
            estimate.flimit = number(p, "FLIMIT", |n| n >= 0.0, "a number of at least 0")?;
        } else if !em && p.eat_keyword("NPREDICTORS") {
            let whole = |n: f64| n >= 0.0 && n.fract() == 0.0;
            estimate.npredictors =
                Some(number(p, "NPREDICTORS", whole, "a whole number of at least 0")? as usize);
        } else if !em && p.eat_keyword("ADDTYPE") {
            p.eat(Punct::Eq);
            estimate.addtype = Some(if p.eat_keyword("RESIDUAL") {
                AddType::Residual
            } else if p.eat_keyword("NORMAL") {
                AddType::Normal
            } else if p.eat_keyword("NONE") {
                AddType::None
            } else if p.eat_word("T") {
                let mut df = 5.0;
                if p.eat(Punct::LParen) {
                    df = number(p, "T", positive, "positive degrees of freedom")?;
                    p.expect(Punct::RParen)?;
                }
                AddType::T(df)
            } else {
                return Err(p.unexpected(&["RESIDUAL", "NORMAL", "T", "NONE"]));
            });
        } else {
            let expected: &[&str] = match em {
                true => &[
                    "TOLERANCE",
                    "CONVERGENCE",
                    "ITERATIONS",
                    "TDF",
                    "LAMBDA",
                    "PROPORTION",
                    "OUTFILE",
                ],
                false => &["TOLERANCE", "FLIMIT", "NPREDICTORS", "ADDTYPE", "OUTFILE"],
            };
            return Err(p.unexpected(expected));
        }
        p.eat(Punct::Comma);
    }
    if em && estimate.tdf.is_some() && estimate.lambda.is_some() {
        return Err(SyntaxError::new(
            p.span_from(open),
            "EM takes either TDF or LAMBDA with PROPORTION, not both",
        ));
    }
    if em && estimate.lambda.is_some() != estimate.proportion.is_some() {
        return Err(SyntaxError::new(
            p.span_from(open),
            "the mixed normal distribution of EM needs both LAMBDA and PROPORTION",
        ));
    }
    Ok(estimate)
}

pub(crate) fn mva(p: &mut Parser) -> PResult<Command> {
    // a shortened VARIABLES only before '=', so a variable named VAR stays a variable
    let keyword = matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq)));
    if p.eat_word("VARIABLES") || (keyword && p.eat_keyword("VARIABLES")) {
        p.eat(Punct::Eq);
    }
    let variables = list(p)?;
    let mut command = Mva {
        variables,
        categorical: Vec::new(),
        maxcat: 25,
        id: None,
        univariate: true,
        ttest: None,
        crosstab: None,
        mismatch: None,
        dpattern: None,
        mpattern: None,
        tpattern: None,
        listwise: false,
        pairwise: false,
        em: None,
        regression: None,
    };
    // CATEGORICAL, MAXCAT and ID must come before the other subcommands
    let mut others = false;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let leading = ["CATEGORICAL", "MAXCAT", "ID"]
            .iter()
            .any(|k| matches!(p.peek(), Some(TokenKind::Word(w)) if matches_keyword(w, k)));
        if leading && others {
            return Err(SyntaxError::new(
                p.span(),
                "CATEGORICAL, MAXCAT and ID must come before the other subcommands of MVA",
            ));
        }
        others |= !leading;
        if p.eat_keyword("CATEGORICAL") {
            p.eat(Punct::Eq);
            command.categorical = list(p)?;
        } else if p.eat_keyword("MAXCAT") {
            let whole = |n: f64| n >= 1.0 && n.fract() == 0.0;
            command.maxcat = number(p, "MAXCAT", whole, "a whole number of at least 1")? as usize;
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("NOUNIVARIATE") {
            command.univariate = false;
        } else if p.eat_keyword("TTEST") {
            let mut ttest = MvaTtest::default();
            while !at_end(p) {
                if p.eat_keyword("PERCENT") {
                    ttest.percent = percent(p)?;
                } else if p.eat_word("T") {
                    ttest.t = true;
                } else if p.eat_word("NOT") {
                    ttest.t = false;
                } else if p.eat_word("DF") {
                    ttest.df = true;
                } else if p.eat_keyword("NODF") {
                    ttest.df = false;
                } else if p.eat_keyword("PROB") {
                    ttest.prob = true;
                } else if p.eat_keyword("NOPROB") {
                    ttest.prob = false;
                } else if p.eat_keyword("COUNTS") {
                    ttest.counts = true;
                } else if p.eat_keyword("NOCOUNTS") {
                    ttest.counts = false;
                } else if p.eat_keyword("MEANS") {
                    ttest.means = true;
                } else if p.eat_keyword("NOMEANS") {
                    ttest.means = false;
                } else {
                    return Err(p.unexpected(&[
                        "PERCENT", "T", "NOT", "DF", "NODF", "PROB", "NOPROB", "COUNTS",
                        "NOCOUNTS", "MEANS", "NOMEANS",
                    ]));
                }
            }
            command.ttest = Some(ttest);
        } else if p.eat_keyword("CROSSTAB") {
            let mut value = 5.0;
            if p.eat_keyword("PERCENT") {
                value = percent(p)?;
            }
            command.crosstab = Some(value);
        } else if p.eat_keyword("MISMATCH") {
            let (mut value, mut nosort) = (5.0, false);
            while !at_end(p) {
                if p.eat_keyword("PERCENT") {
                    value = percent(p)?;
                } else if p.eat_keyword("NOSORT") {
                    nosort = true;
                } else {
                    return Err(p.unexpected(&["PERCENT", "NOSORT"]));
                }
            }
            command.mismatch = Some((value, nosort));
        } else if p.eat_keyword("DPATTERN") {
            command.dpattern = Some(pattern(p, true, 0.0)?);
        } else if p.eat_keyword("MPATTERN") {
            command.mpattern = Some(pattern(p, false, 0.0)?);
        } else if p.eat_keyword("TPATTERN") {
            command.tpattern = Some(pattern(p, false, 1.0)?);
        } else if p.eat_keyword("LISTWISE") {
            command.listwise = true;
        } else if p.eat_keyword("PAIRWISE") {
            command.pairwise = true;
        } else if p.eat_word("EM") {
            command.em = Some(estimate(p, true)?);
        } else if p.eat_keyword("REGRESSION") {
            command.regression = Some(estimate(p, false)?);
        } else {
            p.pos = start;
            return Err(p.subcommand_error("MVA", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Mva(Box::new(command)))
}
