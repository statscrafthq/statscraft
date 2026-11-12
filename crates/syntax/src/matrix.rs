//! MATRIX ... END MATRIX: the statements of a matrix program and their expressions.
//!
//! Rules, from the description of the matrix language:
//! - The statements are BREAK, CALL, COMPUTE, DISPLAY, DO IF, ELSE IF, ELSE, END IF, LOOP,
//!   END LOOP, GET, MGET, MSAVE, PRINT, READ, RELEASE, SAVE and WRITE. Their names and keywords
//!   may be shortened to three or more letters, as command names can.
//! - A matrix is written in braces: commas separate the elements of a row, semicolons the rows.
//!   `a:b` is the integers from a to b, `a:b:c` those with the increment c.
//! - Operators from the tightest: unary + and -; `:`; `**` and `&**`; `*`, `/`, `&*` and `&/`;
//!   binary + and -; the relational operators (also written GT, LT, NE (or ~=), LE, GE, EQ);
//!   NOT; AND; OR and XOR. Equal operators run from left to right. `&`, `|` and `~` are AND,
//!   OR and NOT.
//! - `name(...)` is a function call when the name is one of the matrix functions, else an
//!   index into a matrix: one or two indexes, each a vector of positions or `:` for all. The
//!   names of functions and of the CALL procedures are not variable names; T stands for
//!   TRANSPOS.
//! - PRINT, WRITE, SAVE and MSAVE take a name, a call, a matrix in braces or a power of one of
//!   these; any other expression goes in parentheses.
//! - The first READ, WRITE and SAVE in the program (in the order written) names its file; a
//!   later one without a file uses the file named last before it. The same holds for the
//!   specifications of MSAVE other than FACTOR; all MSAVE statements write one file.
//! - DO IF and ELSE IF take any expression that gives a scalar, true when positive. LOOP takes
//!   an optional index `var = first TO last [BY step]` (a comma may stand for TO, as in the
//!   example of the manual) and an optional IF condition; END LOOP an optional IF condition.
//!   BREAK leaves the innermost loop and is only allowed inside one.
//! - RELEASE separates its names with commas.

use crate::ast::VarItem;
use crate::data_list::numbered_names;
use crate::error::{Span, SyntaxError};
use crate::lexer::{Punct, Token, TokenKind};
use crate::parser::{PResult, Parser, is_reserved};
use crate::split::RawCommand;
use crate::table::matches_keyword;

/// The statements of a matrix program, blocks nested.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MatrixProgram {
    pub statements: Vec<MatrixStatement>,
}

/// One statement with the place of its name, which errors at run time point at.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixStatement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Compute {
        target: Target,
        expr: MExpr,
    },
    Call(CallStatement),
    Print(Box<MatrixPrint>),
    /// Each condition with its statements, then the statements of ELSE.
    DoIf {
        branches: Vec<(MExpr, Vec<MatrixStatement>)>,
        otherwise: Option<Vec<MatrixStatement>>,
    },
    Loop(Box<MatrixLoop>),
    Break,
    Read(Box<MatrixRead>),
    Write(Box<MatrixWrite>),
    Get(Box<MatrixGet>),
    Save(Box<MatrixSave>),
    Mget(MatrixMget),
    Msave(Box<MatrixMsave>),
    Display {
        dictionary: bool,
        status: bool,
    },
    Release(Vec<String>),
}

/// A matrix variable, or a part of one when it has indexes.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub name: String,
    pub index: Option<Vec<Index>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    /// `:`, every row or column.
    All,
    Expr(MExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MExpr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(f64),
    String(String),
    Variable(String),
    /// Rows of elements, each element a matrix pasted beside the others.
    Matrix(Vec<Vec<MExpr>>),
    Index {
        name: String,
        indexes: Vec<Index>,
    },
    /// A matrix function, its name in upper case and in full.
    Call {
        name: String,
        args: Vec<MExpr>,
    },
    /// EOF(file)
    Eof(String),
    Neg(Box<MExpr>),
    Not(Box<MExpr>),
    Binary {
        op: MatrixOp,
        left: Box<MExpr>,
        right: Box<MExpr>,
    },
    /// `first:last[:step]`
    Sequence {
        first: Box<MExpr>,
        last: Box<MExpr>,
        step: Option<Box<MExpr>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixOp {
    Add,
    Sub,
    /// `*`: matrix product, or scalar times matrix.
    Mul,
    /// `/`: elementwise division.
    Div,
    /// `**`: matrix power.
    Pow,
    ElemMul,
    ElemDiv,
    ElemPow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallStatement {
    Eigen {
        matrix: MExpr,
        vectors: String,
        values: String,
    },
    Setdiag {
        matrix: String,
        values: MExpr,
    },
    Svd {
        matrix: MExpr,
        u: String,
        q: String,
        v: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixPrint {
    pub expr: Option<MExpr>,
    /// The expression as written, the title when TITLE is not given.
    pub text: String,
    /// Such as "F8.2", "E13" or "A8".
    pub format: Option<String>,
    pub title: Option<String>,
    pub rlabels: Option<Labels>,
    pub clabels: Option<Labels>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Labels {
    /// RLABELS or CLABELS: the labels as given; unquoted ones in upper case.
    List(Vec<String>),
    /// RNAMES or CNAMES: a vector of strings.
    Names(MExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixLoop {
    pub index: Option<LoopIndex>,
    pub condition: Option<MExpr>,
    pub body: Vec<MatrixStatement>,
    pub until: Option<MExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopIndex {
    pub name: String,
    pub first: MExpr,
    pub last: MExpr,
    pub step: Option<MExpr>,
}

/// FIELD = first TO last [BY width], in columns from 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    pub first: usize,
    pub last: usize,
    pub width: Option<usize>,
}

/// A data format of READ or WRITE: an optional repetition factor, the type, an optional width
/// and the decimals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataFormat {
    pub repeat: Option<usize>,
    pub kind: crate::ast::FormatType,
    pub width: Option<usize>,
    pub decimals: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixRead {
    pub target: Target,
    pub file: String,
    pub field: Field,
    pub size: Option<MExpr>,
    pub symmetric: bool,
    pub reread: bool,
    pub format: Option<DataFormat>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixWrite {
    pub expr: MExpr,
    pub outfile: String,
    pub field: Field,
    pub triangular: bool,
    pub hold: bool,
    pub format: Option<DataFormat>,
}

/// GET /MISSING and /SYSMIS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MissingAction {
    Accept,
    Omit,
    Value(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixGet {
    pub target: Target,
    /// None: the active dataset.
    pub file: Option<String>,
    /// None: all variables.
    pub variables: Option<Vec<VarItem>>,
    pub names: Option<String>,
    pub missing: Option<MissingAction>,
    pub sysmis: Option<MissingAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixSave {
    pub expr: MExpr,
    /// None: the active dataset.
    pub outfile: Option<String>,
    pub variables: Option<Vec<String>>,
    pub names: Option<MExpr>,
    pub strings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixMget {
    /// None: the active dataset.
    pub file: Option<String>,
    /// COV, CORR, MEAN, STDDEV, N or COUNT; empty: all.
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixMsave {
    pub expr: MExpr,
    /// COV, CORR, MEAN, STDDEV, N or COUNT.
    pub kind: String,
    pub outfile: String,
    pub variables: Option<Vec<String>>,
    pub snames: Option<Vec<String>>,
    pub split: Option<MExpr>,
    pub fnames: Option<Vec<String>>,
    pub factor: Option<MExpr>,
}

const STATEMENTS: [&str; 19] = [
    "BREAK",
    "CALL",
    "COMPUTE",
    "DISPLAY",
    "DO IF",
    "ELSE",
    "ELSE IF",
    "END IF",
    "END LOOP",
    "END MATRIX",
    "GET",
    "LOOP",
    "MGET",
    "MSAVE",
    "PRINT",
    "READ",
    "RELEASE",
    "SAVE",
    "WRITE",
];

/// The matrix statement the tokens start, with the number of tokens its name takes. The longest
/// name wins, and a name written in full wins over an abbreviation.
pub(crate) fn statement_name(tokens: &[Token]) -> Option<(&'static str, usize)> {
    let word = |i: usize| match tokens.get(i).map(|t| &t.kind) {
        Some(TokenKind::Word(w)) => Some(w.as_str()),
        _ => None,
    };
    let mut best: Option<(&'static str, (usize, bool))> = None;
    for name in STATEMENTS {
        let parts: Vec<&str> = name.split(' ').collect();
        let words: Option<Vec<&str>> = (0..parts.len()).map(word).collect();
        let Some(words) = words else { continue };
        if !parts.iter().zip(&words).all(|(p, w)| matches_keyword(w, p)) {
            continue;
        }
        let exact = parts
            .iter()
            .zip(&words)
            .all(|(p, w)| w.eq_ignore_ascii_case(p));
        let rank = (parts.len(), exact);
        if best.is_none_or(|(_, b)| rank > b) {
            best = Some((name, rank));
        }
    }
    best.map(|(name, (used, _))| (name, used))
}

/// The matrix functions with the number of arguments they take (least, most).
const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("ABS", 1, 1),
    ("ALL", 1, 1),
    ("ANY", 1, 1),
    ("ARSIN", 1, 1),
    ("ARTAN", 1, 1),
    ("BLOCK", 1, usize::MAX),
    ("CHOL", 1, 1),
    ("CMAX", 1, 1),
    ("CMIN", 1, 1),
    ("COS", 1, 1),
    ("CSSQ", 1, 1),
    ("CSUM", 1, 1),
    ("DESIGN", 1, 1),
    ("DET", 1, 1),
    ("DIAG", 1, 1),
    ("EOF", 1, 1),
    ("EVAL", 1, 1),
    ("EXP", 1, 1),
    ("GINV", 1, 1),
    ("GRADE", 1, 1),
    ("GSCH", 1, 1),
    ("IDENT", 1, 2),
    ("INV", 1, 1),
    ("KRONEKER", 2, 2),
    ("LG10", 1, 1),
    ("LN", 1, 1),
    ("MAGIC", 1, 1),
    ("MAKE", 3, 3),
    ("MDIAG", 1, 1),
    ("MMAX", 1, 1),
    ("MMIN", 1, 1),
    ("MOD", 2, 2),
    ("MSSQ", 1, 1),
    ("MSUM", 1, 1),
    ("NCOL", 1, 1),
    ("NROW", 1, 1),
    ("RANK", 1, 1),
    ("RESHAPE", 3, 3),
    ("RMAX", 1, 1),
    ("RMIN", 1, 1),
    ("RND", 1, 1),
    ("RNKORDER", 1, 1),
    ("RSSQ", 1, 1),
    ("RSUM", 1, 1),
    ("SIN", 1, 1),
    ("SOLVE", 2, 2),
    ("SQRT", 1, 1),
    ("SSCP", 1, 1),
    ("SVAL", 1, 1),
    ("SWEEP", 2, 2),
    ("TRACE", 1, 1),
    ("TRANSPOS", 1, 1),
    ("TRUNC", 1, 1),
    ("UNIFORM", 1, 2),
    ("NORMAL", 1, 2),
    ("CDFNORM", 1, 1),
    ("CHICDF", 2, 2),
    ("FCDF", 3, 3),
    ("TCDF", 2, 2),
    ("PROBIT", 1, 1),
];

/// The distribution functions, each family with its distributions and their numbers of
/// arguments.
const DISTRIBUTIONS: &[(&str, &[(&str, usize)])] = &[
    (
        "PDF",
        &[
            ("BERNOULLI", 2),
            ("BETA", 3),
            ("BINOM", 3),
            ("BVNOR", 3),
            ("CAUCHY", 3),
            ("CHISQ", 2),
            ("EXP", 2),
            ("F", 3),
            ("GAMMA", 3),
            ("GEOM", 2),
            ("HALFNRM", 3),
            ("HYPER", 4),
            ("IGAUSS", 3),
            ("LAPLACE", 3),
            ("LOGISTIC", 3),
            ("LNORMAL", 3),
            ("NEGBIN", 3),
            ("NORMAL", 3),
            ("PARETO", 3),
            ("POISSON", 2),
            ("T", 2),
            ("UNIFORM", 3),
            ("WEIBULL", 3),
        ],
    ),
    ("NPDF", &[("BETA", 4), ("CHISQ", 3), ("F", 4), ("T", 3)]),
    ("SIG", &[("CHISQ", 2), ("F", 3)]),
    (
        "CDF",
        &[
            ("BERNOULLI", 2),
            ("BETA", 3),
            ("BINOM", 3),
            ("BVNOR", 3),
            ("CAUCHY", 3),
            ("CHISQ", 2),
            ("EXP", 2),
            ("F", 3),
            ("GAMMA", 3),
            ("GEOM", 2),
            ("HALFNRM", 3),
            ("HYPER", 4),
            ("IGAUSS", 3),
            ("LAPLACE", 3),
            ("LNORMAL", 3),
            ("LOGISTIC", 3),
            ("NEGBIN", 3),
            ("NORMAL", 3),
            ("PARETO", 3),
            ("POISSON", 2),
            ("SMOD", 3),
            ("SRANGE", 3),
            ("T", 2),
            ("UNIFORM", 3),
            ("WEIBULL", 3),
        ],
    ),
    ("NCDF", &[("BETA", 4), ("CHISQ", 3), ("F", 4), ("T", 3)]),
    (
        "IDF",
        &[
            ("BETA", 3),
            ("CAUCHY", 3),
            ("CHISQ", 2),
            ("EXP", 2),
            ("F", 3),
            ("GAMMA", 3),
            ("HALFNRM", 3),
            ("IGAUSS", 3),
            ("LAPLACE", 3),
            ("LNORMAL", 3),
            ("LOGISTIC", 3),
            ("NORMAL", 3),
            ("PARETO", 3),
            ("SMOD", 3),
            ("SRANGE", 3),
            ("T", 2),
            ("UNIFORM", 3),
            ("WEIBULL", 3),
        ],
    ),
    (
        "RV",
        &[
            ("BERNOULLI", 1),
            ("BETA", 2),
            ("BINOM", 2),
            ("CAUCHY", 2),
            ("CHISQ", 1),
            ("EXP", 1),
            ("F", 2),
            ("GAMMA", 2),
            ("GEOM", 1),
            ("HALFNRM", 2),
            ("HYPER", 3),
            ("IGAUSS", 2),
            ("LAPLACE", 2),
            ("LOGISTIC", 2),
            ("LNORMAL", 2),
            ("NEGBIN", 2),
            ("NORMAL", 2),
            ("PARETO", 2),
            ("POISSON", 1),
            ("T", 1),
            ("UNIFORM", 2),
            ("WEIBULL", 2),
        ],
    ),
];

/// The procedures of CALL; their names are no variable names either.
const PROCEDURES: [&str; 3] = ["EIGEN", "SETDIAG", "SVD"];

/// The matrix function `name` (any case) stands for, in full, with its least and most numbers of
/// arguments. T is TRANSPOS.
pub fn matrix_function(name: &str) -> Option<(String, usize, usize)> {
    let upper = name.to_uppercase();
    let upper = if upper == "T" {
        "TRANSPOS".into()
    } else {
        upper
    };
    if let Some(&(f, low, high)) = FUNCTIONS.iter().find(|(f, _, _)| *f == upper) {
        return Some((f.to_string(), low, high));
    }
    let (family, distribution) = upper.split_once('.')?;
    let (_, list) = DISTRIBUTIONS.iter().find(|(f, _)| *f == family)?;
    let &(_, count) = list.iter().find(|(d, _)| *d == distribution)?;
    Some((upper, count, count))
}

/// Every matrix function name, distributions included, for help and completion.
pub fn matrix_function_names() -> Vec<String> {
    let mut names: Vec<String> = FUNCTIONS.iter().map(|(f, _, _)| f.to_string()).collect();
    for (family, list) in DISTRIBUTIONS {
        names.extend(list.iter().map(|(d, _)| format!("{family}.{d}")));
    }
    names
}

fn reserved_name(name: &str) -> bool {
    matrix_function(name).is_some() || PROCEDURES.iter().any(|p| p.eq_ignore_ascii_case(name))
}

/// The expression as written: the tokens with a blank where the source had one.
fn text_of(tokens: &[Token]) -> String {
    let mut text = String::new();
    let mut last: Option<Span> = None;
    for token in tokens {
        if let Some(last) = last
            && (token.span.line != last.end_line || token.span.column > last.end_column)
        {
            text.push(' ');
        }
        match &token.kind {
            TokenKind::Word(w) => text.push_str(w),
            TokenKind::Number(n) => text.push_str(&n.to_string()),
            TokenKind::Str(s) => text.push_str(&format!("\"{}\"", s.replace('"', "\"\""))),
            TokenKind::Punct(p) => text.push_str(p.as_str()),
        }
        last = Some(token.span);
    }
    text
}

/// A statement before blocks are put together.
enum Flat {
    Statement(StatementKind),
    DoIf(MExpr),
    ElseIf(MExpr),
    Else,
    EndIf,
    Loop(Option<LoopIndex>, Option<MExpr>),
    EndLoop(Option<MExpr>),
}

/// The files that READ, WRITE, SAVE and MSAVE take from the statements before them.
#[derive(Default)]
struct Carried {
    read: Option<String>,
    write: Option<String>,
    /// Some(None): the active dataset.
    save: Option<Option<String>>,
    msave: Option<MsaveCarried>,
}

#[derive(Clone)]
struct MsaveCarried {
    outfile: String,
    variables: Option<Vec<String>>,
    snames: Option<Vec<String>>,
    split: Option<MExpr>,
    fnames: Option<Vec<String>>,
}

/// The program of the statements between MATRIX and END MATRIX. Every bad statement gives
/// one error.
pub(crate) fn program(raws: &[RawCommand]) -> Result<MatrixProgram, Vec<SyntaxError>> {
    let mut errors = Vec::new();
    let mut flat = Vec::new();
    let mut carried = Carried::default();
    for raw in raws {
        match statement(raw, &mut carried) {
            Ok(item) => flat.push((item, raw.tokens[0].span)),
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut items = flat.into_iter().peekable();
    let statements = block(&mut items, &mut errors, 0, 0, None)?;
    if let Some((item, span)) = items.next() {
        let message = match item {
            Flat::EndLoop(_) => "END LOOP without LOOP",
            Flat::EndIf => "END IF without DO IF",
            _ => "ELSE IF or ELSE without DO IF",
        };
        errors.push(SyntaxError::new(span, message));
    }
    if errors.is_empty() {
        Ok(MatrixProgram { statements })
    } else {
        Err(errors)
    }
}

type Items = std::iter::Peekable<std::vec::IntoIter<(Flat, Span)>>;

/// Statements up to the end of the block that `opened` (DO IF or LOOP, with its place)
/// started, which the caller consumes. `loops` counts the loops around, `depth` all blocks.
fn block(
    items: &mut Items,
    errors: &mut Vec<SyntaxError>,
    loops: usize,
    depth: usize,
    opened: Option<(&str, Span)>,
) -> Result<Vec<MatrixStatement>, Vec<SyntaxError>> {
    // each level parses and runs one level deeper on the stack, which has 2 MiB
    if let Some((name, span)) = opened.filter(|_| depth > 100) {
        let message = format!("{name} is nested more than 100 levels deep");
        errors.push(SyntaxError::new(span, message));
        return Err(std::mem::take(errors));
    }
    let mut out = Vec::new();
    loop {
        let Some((item, _)) = items.peek() else {
            let Some((name, span)) = opened else {
                return Ok(out);
            };
            let end = if name == "DO IF" {
                "END IF"
            } else {
                "END LOOP"
            };
            errors.push(
                SyntaxError::new(span, format!("{name} without {end}"))
                    .hint(format!("end the block with {end} before END MATRIX")),
            );
            return Err(std::mem::take(errors));
        };
        if matches!(
            item,
            Flat::ElseIf(_) | Flat::Else | Flat::EndIf | Flat::EndLoop(_)
        ) {
            return Ok(out);
        }
        let (item, span) = items.next().expect("peeked");
        let kind = match item {
            Flat::Statement(StatementKind::Break) if loops == 0 => {
                errors.push(
                    SyntaxError::new(span, "BREAK outside a loop")
                        .hint("BREAK leaves the LOOP it stands in"),
                );
                continue;
            }
            Flat::Statement(kind) => kind,
            Flat::DoIf(condition) => {
                let mut branches = Vec::new();
                let mut otherwise = None;
                let mut condition = Some(condition);
                loop {
                    let body = block(items, errors, loops, depth + 1, Some(("DO IF", span)))?;
                    match condition.take() {
                        Some(c) => branches.push((c, body)),
                        None => otherwise = Some(body),
                    }
                    let (next, at) = items.next().expect("block stops at an item");
                    match next {
                        Flat::ElseIf(c) if otherwise.is_none() => condition = Some(c),
                        Flat::Else if otherwise.is_none() => {}
                        Flat::EndIf => break,
                        Flat::ElseIf(_) | Flat::Else => {
                            errors.push(
                                SyntaxError::new(at, "ELSE IF or ELSE after ELSE")
                                    .hint("ELSE comes last in a DO IF block, before END IF"),
                            );
                        }
                        _ => {
                            errors.push(
                                SyntaxError::new(at, "END LOOP inside a DO IF block").hint(
                                    format!(
                                        "close the DO IF on line {} with END IF first",
                                        span.line
                                    ),
                                ),
                            );
                            return Err(std::mem::take(errors));
                        }
                    }
                }
                StatementKind::DoIf {
                    branches,
                    otherwise,
                }
            }
            Flat::Loop(index, condition) => {
                let body = block(items, errors, loops + 1, depth + 1, Some(("LOOP", span)))?;
                let (next, at) = items.next().expect("block stops at an item");
                let Flat::EndLoop(until) = next else {
                    errors.push(
                        SyntaxError::new(at, "END IF, ELSE IF or ELSE inside a LOOP").hint(
                            format!("close the LOOP on line {} with END LOOP first", span.line),
                        ),
                    );
                    return Err(std::mem::take(errors));
                };
                StatementKind::Loop(Box::new(MatrixLoop {
                    index,
                    condition,
                    body,
                    until,
                }))
            }
            _ => unreachable!("handled above"),
        };
        out.push(MatrixStatement { kind, span });
    }
}

fn statement(raw: &RawCommand, carried: &mut Carried) -> PResult<Flat> {
    let first = raw.tokens[0].span;
    let Some((name, used)) = statement_name(&raw.tokens) else {
        let word = match &raw.tokens[0].kind {
            TokenKind::Word(w) => w.to_uppercase(),
            _ => "this".into(),
        };
        let mut error = SyntaxError::new(
            first,
            format!("{word} is not a statement of a matrix program"),
        );
        error.expected = STATEMENTS.iter().map(|s| s.to_string()).collect();
        return Err(error.hint("a MATRIX program ends with END MATRIX"));
    };
    let mut p = Parser::new(&raw.tokens, used, raw.end);
    let p = &mut p;
    let item = match name {
        "COMPUTE" => {
            let target = target(p)?;
            p.expect(Punct::Eq)?;
            let expr = expr(p)?;
            Flat::Statement(StatementKind::Compute { target, expr })
        }
        "CALL" => Flat::Statement(StatementKind::Call(call(p)?)),
        "PRINT" => Flat::Statement(StatementKind::Print(Box::new(print(p, &raw.tokens)?))),
        "DO IF" => Flat::DoIf(expr(p)?),
        "ELSE IF" => Flat::ElseIf(expr(p)?),
        "ELSE" => Flat::Else,
        "END IF" => Flat::EndIf,
        "LOOP" => loop_header(p)?,
        "END LOOP" => Flat::EndLoop(if p.eat_word("IF") {
            Some(expr(p)?)
        } else {
            None
        }),
        "BREAK" => Flat::Statement(StatementKind::Break),
        "READ" => Flat::Statement(StatementKind::Read(Box::new(read(p, carried)?))),
        "WRITE" => Flat::Statement(StatementKind::Write(Box::new(write(p, carried)?))),
        "GET" => Flat::Statement(StatementKind::Get(Box::new(get(p)?))),
        "SAVE" => Flat::Statement(StatementKind::Save(Box::new(save(p, carried)?))),
        "MGET" => Flat::Statement(StatementKind::Mget(mget(p)?)),
        "MSAVE" => Flat::Statement(StatementKind::Msave(Box::new(msave(p, carried)?))),
        "DISPLAY" => {
            let (mut dictionary, mut status) = (false, false);
            loop {
                if p.eat_keyword("DICTIONARY") {
                    dictionary = true;
                } else if p.eat_keyword("STATUS") {
                    status = true;
                } else {
                    break;
                }
            }
            if !dictionary && !status {
                (dictionary, status) = (true, true);
            }
            p.finish(&["DICTIONARY", "STATUS"])?;
            Flat::Statement(StatementKind::Display { dictionary, status })
        }
        "RELEASE" => {
            let mut names = vec![p.name("a matrix variable")?];
            while p.eat(Punct::Comma) {
                names.push(p.name("a matrix variable")?);
            }
            if matches!(p.peek(), Some(TokenKind::Word(_))) {
                return Err(p
                    .unexpected(&["','", "end of command"])
                    .hint("RELEASE separates its names with commas"));
            }
            Flat::Statement(StatementKind::Release(names))
        }
        _ => {
            let error = SyntaxError::new(first, "END MATRIX inside the program");
            return Err(error);
        }
    };
    p.finish(&["an operator"])?;
    Ok(item)
}

/// A matrix variable name: no reserved word and no function or procedure name.
fn variable_name(p: &mut Parser, what: &str) -> PResult<String> {
    let span = p.span();
    let name = p.name(what)?;
    if reserved_name(&name) {
        let message = format!(
            "{} is the name of a matrix function or procedure and cannot name a matrix",
            name.to_uppercase()
        );
        return Err(SyntaxError::new(span, message));
    }
    Ok(name)
}

fn target(p: &mut Parser) -> PResult<Target> {
    let start = p.pos;
    let name = variable_name(p, "a matrix variable")?;
    let index = if p.eat(Punct::LParen) {
        Some(indexes(p)?)
    } else {
        None
    };
    Ok(Target {
        name,
        index,
        span: p.span_from(start),
    })
}

/// One or two indexes after the opening parenthesis, and the closing one.
fn indexes(p: &mut Parser) -> PResult<Vec<Index>> {
    let mut out = Vec::new();
    loop {
        if p.at(Punct::Colon)
            && matches!(
                p.peek_at(1),
                Some(TokenKind::Punct(Punct::Comma | Punct::RParen))
            )
        {
            p.pos += 1;
            out.push(Index::All);
        } else {
            out.push(Index::Expr(expr(p)?));
        }
        if p.eat(Punct::RParen) {
            break;
        }
        if out.len() == 2 {
            return Err(p
                .unexpected(&["')'"])
                .hint("a matrix takes at most two indexes"));
        }
        p.expect(Punct::Comma)?;
    }
    Ok(out)
}

fn call(p: &mut Parser) -> PResult<CallStatement> {
    let span = p.span();
    let Some(TokenKind::Word(word)) = p.peek() else {
        return Err(p.unexpected(&["EIGEN", "SETDIAG", "SVD"]));
    };
    let name = word.to_uppercase();
    p.pos += 1;
    p.expect(Punct::LParen)?;
    let call = match name.as_str() {
        "EIGEN" => {
            let matrix = expr(p)?;
            p.expect(Punct::Comma)?;
            let vectors = variable_name(p, "a name for the eigenvectors")?;
            p.expect(Punct::Comma)?;
            let values = variable_name(p, "a name for the eigenvalues")?;
            CallStatement::Eigen {
                matrix,
                vectors,
                values,
            }
        }
        "SETDIAG" => {
            let matrix = variable_name(p, "the matrix whose diagonal is set")?;
            p.expect(Punct::Comma)?;
            CallStatement::Setdiag {
                matrix,
                values: expr(p)?,
            }
        }
        "SVD" => {
            let matrix = expr(p)?;
            let mut names = Vec::new();
            for _ in 0..3 {
                p.expect(Punct::Comma)?;
                names.push(variable_name(p, "a name for a result")?);
            }
            let [u, q, v] = <[String; 3]>::try_from(names).expect("three names");
            CallStatement::Svd { matrix, u, q, v }
        }
        _ => {
            let mut error = SyntaxError::new(span, format!("unknown matrix procedure {name}"));
            error.expected = PROCEDURES.iter().map(|s| s.to_string()).collect();
            return Err(error);
        }
    };
    p.expect(Punct::RParen)?;
    Ok(call)
}

/// The matrix of PRINT, WRITE, SAVE and MSAVE: a name, a call, a matrix in braces or a power of
/// one, or any expression in parentheses.
fn operand(p: &mut Parser, statement: &str) -> PResult<MExpr> {
    let expr = power(p)?;
    if p.peek().is_some() && !p.at(Punct::Slash) {
        return Err(p.unexpected(&["'/'", "end of command"]).hint(format!(
            "an expression other than a name, a call or a power goes in parentheses: \
             {statement} (A+B)"
        )));
    }
    Ok(expr)
}

fn print(p: &mut Parser, tokens: &[Token]) -> PResult<MatrixPrint> {
    let start = p.pos;
    let expr = if p.peek().is_none() || p.at(Punct::Slash) {
        None
    } else {
        Some(operand(p, "PRINT")?)
    };
    let mut print = MatrixPrint {
        expr,
        text: text_of(&tokens[start..p.pos]),
        format: None,
        title: None,
        rlabels: None,
        clabels: None,
    };
    while p.eat(Punct::Slash) {
        let at = p.span();
        if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            let (text, span) = word_or_string(p, "a format such as F8.2")?;
            let format = data_format(&text).map_err(|m| SyntaxError::new(span, m))?;
            if format.repeat.is_some() || format.width.is_none() {
                return Err(SyntaxError::new(
                    span,
                    "PRINT /FORMAT needs a format with a width, such as F8.2 or A8",
                ));
            }
            print.format = Some(text.to_uppercase());
        } else if p.eat_keyword("TITLE") {
            p.eat(Punct::Eq);
            print.title = Some(p.string("a quoted title")?);
        } else if p.eat_keyword("SPACE") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("NEWPAGE") && p.number().is_none() {
                return Err(p.unexpected(&["NEWPAGE", "a number of lines"]));
            }
        } else if p.eat_keyword("RLABELS") {
            print.rlabels = Some(Labels::List(labels(p)?));
        } else if p.eat_keyword("CLABELS") {
            print.clabels = Some(Labels::List(labels(p)?));
        } else if p.eat_keyword("RNAMES") {
            p.eat(Punct::Eq);
            print.rlabels = Some(Labels::Names(sub_expr(p)?));
        } else if p.eat_keyword("CNAMES") {
            p.eat(Punct::Eq);
            print.clabels = Some(Labels::Names(sub_expr(p)?));
        } else {
            let mut error = p.unexpected(&[
                "FORMAT", "TITLE", "SPACE", "RLABELS", "RNAMES", "CLABELS", "CNAMES",
            ]);
            error.span = at;
            return Err(error);
        }
    }
    Ok(print)
}

/// A list of labels separated by commas: names, numbers or quoted strings.
fn labels(p: &mut Parser) -> PResult<Vec<String>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    loop {
        match p.peek() {
            Some(TokenKind::Word(w)) => out.push(w.to_uppercase()),
            Some(TokenKind::Str(s)) => out.push(s.clone()),
            Some(TokenKind::Number(n)) => out.push(n.to_string()),
            _ => return Err(p.unexpected(&["a label"])),
        }
        p.pos += 1;
        if !p.eat(Punct::Comma) {
            return Ok(out);
        }
    }
}

fn word_or_string(p: &mut Parser, what: &str) -> PResult<(String, Span)> {
    let span = p.span();
    let text = match p.peek() {
        Some(TokenKind::Word(w)) => w.clone(),
        Some(TokenKind::Str(s)) => s.clone(),
        _ => return Err(p.unexpected(&[what])),
    };
    p.pos += 1;
    Ok((text, span))
}

/// `[repeat]type[width[.decimals]]`, such as F8.2, 10F, E12.2, A6 or F.
pub(crate) fn data_format(text: &str) -> Result<DataFormat, String> {
    let invalid = || {
        format!(
            "invalid format {}: write such as F8.2, E12.2, A8 or 10F",
            text.to_uppercase()
        )
    };
    let digits = text
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(invalid)?;
    let (repeat, rest) = text.split_at(digits);
    let letters = rest
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let (kind, size) = rest.split_at(letters);
    let kind = crate::ast::FormatType::from_name(kind)
        .ok_or_else(|| format!("unknown format type in {}", text.to_uppercase()))?;
    let repeat = match repeat {
        "" => None,
        r => Some(
            r.parse::<usize>()
                .ok()
                .filter(|&r| r > 0)
                .ok_or_else(invalid)?,
        ),
    };
    let (width, decimals) = size.split_once('.').unwrap_or((size, ""));
    let width = match width {
        "" => None,
        w => Some(
            w.parse::<usize>()
                .ok()
                .filter(|&w| w > 0)
                .ok_or_else(invalid)?,
        ),
    };
    let decimals = match decimals {
        "" => 0,
        d => d.parse::<usize>().map_err(|_| invalid())?,
    };
    if repeat.is_some() && width.is_some() {
        return Err(format!(
            "invalid format {}: give a repetition factor or a width, not both",
            text.to_uppercase()
        ));
    }
    Ok(DataFormat {
        repeat,
        kind,
        width,
        decimals,
    })
}

fn loop_header(p: &mut Parser) -> PResult<Flat> {
    let index = if matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("IF")) {
        let name = variable_name(p, "an index variable")?;
        p.expect(Punct::Eq)?;
        let first = expr(p)?;
        if !p.eat_word("TO") && !p.eat(Punct::Comma) {
            return Err(p.unexpected(&["TO"]));
        }
        let last = expr(p)?;
        let step = if p.eat_word("BY") {
            Some(expr(p)?)
        } else {
            None
        };
        Some(LoopIndex {
            name,
            first,
            last,
            step,
        })
    } else {
        None
    };
    let condition = if p.eat_word("IF") {
        Some(expr(p)?)
    } else {
        None
    };
    Ok(Flat::Loop(index, condition))
}

fn field(p: &mut Parser) -> PResult<Field> {
    p.eat(Punct::Eq);
    let column = |p: &mut Parser, what: &str| -> PResult<usize> {
        match p.peek() {
            Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= 1e6 => {
                let n = *n as usize;
                p.pos += 1;
                Ok(n)
            }
            _ => Err(p.unexpected(&[what])),
        }
    };
    let span = p.span();
    let first = column(p, "a first column")?;
    if !p.eat_word("TO") {
        return Err(p.unexpected(&["TO"]));
    }
    let last = column(p, "a last column")?;
    if last < first {
        return Err(SyntaxError::new(
            span,
            "the last column comes before the first",
        ));
    }
    let width = if p.eat_word("BY") {
        let width = column(p, "a field width")?;
        if (last - first + 1) % width != 0 {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                format!(
                    "the width {width} does not divide the {} columns of the field evenly",
                    last - first + 1
                ),
            ));
        }
        Some(width)
    } else {
        None
    };
    Ok(Field { first, last, width })
}

/// FORMAT on READ and WRITE, checked against the width of the field.
fn io_format(p: &mut Parser, field: Option<&Field>) -> PResult<DataFormat> {
    p.eat(Punct::Eq);
    let (text, span) = word_or_string(p, "a format such as F8.2")?;
    let format = data_format(&text).map_err(|m| SyntaxError::new(span, m))?;
    if let (Some(field), Some(repeat)) = (field, format.repeat)
        && (field.last - field.first + 1) % repeat != 0
    {
        return Err(SyntaxError::new(
            span,
            format!(
                "{repeat} fields do not divide the {} columns of FIELD evenly",
                field.last - field.first + 1
            ),
        ));
    }
    Ok(format)
}

/// The field width that FIELD BY and FORMAT give, which must agree.
fn check_width(field: &Field, format: Option<&DataFormat>, span: Span) -> PResult<()> {
    let columns = field.last - field.first + 1;
    let from_format = format.and_then(|f| f.width.or(f.repeat.map(|r| columns / r)));
    if let (Some(by), Some(width)) = (field.width, from_format)
        && by != width
    {
        return Err(SyntaxError::new(
            span,
            format!("FIELD BY {by} and FORMAT give different widths ({width})"),
        ));
    }
    Ok(())
}

fn file_or_handle(p: &mut Parser) -> PResult<String> {
    p.eat(Punct::Eq);
    p.file("a quoted file name or a file handle")
}

fn read(p: &mut Parser, carried: &mut Carried) -> PResult<MatrixRead> {
    let start = p.span();
    let target = target(p)?;
    let (mut file, mut field, mut size, mut symmetric, mut reread, mut format) =
        (None, None, None, false, false, None);
    while p.eat(Punct::Slash) {
        if p.eat_keyword("FILE") {
            file = Some(file_or_handle(p)?);
        } else if p.eat_keyword("FIELD") {
            field = Some(field_spec(p)?);
        } else if p.eat_keyword("SIZE") {
            p.eat(Punct::Eq);
            size = Some(sub_expr(p)?);
        } else if p.eat_keyword("MODE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("SYMMETRIC") {
                symmetric = true;
            } else if p.eat_keyword("RECTANGULAR") {
                symmetric = false;
            } else {
                return Err(p.unexpected(&["RECTANGULAR", "SYMMETRIC"]));
            }
        } else if p.eat_keyword("REREAD") {
            reread = true;
        } else if p.eat_keyword("FORMAT") {
            format = Some(io_format(p, field.as_ref())?);
        } else {
            return Err(p.unexpected(&["FILE", "FIELD", "SIZE", "MODE", "REREAD", "FORMAT"]));
        }
    }
    let Some(field) = field else {
        return Err(SyntaxError::new(start, "READ needs FIELD = first TO last"));
    };
    check_width(&field, format.as_ref(), start)?;
    if target.index.is_none() && size.is_none() {
        return Err(
            SyntaxError::new(target.span, "READ of a whole matrix needs SIZE")
                .hint("give the rows and columns, such as /SIZE={3, 4}, or name a part: M(:,:)"),
        );
    }
    let file = match file {
        Some(file) => {
            carried.read = Some(file.clone());
            file
        }
        None => carried
            .read
            .clone()
            .ok_or_else(|| SyntaxError::new(start, "the first READ of the program needs FILE"))?,
    };
    Ok(MatrixRead {
        target,
        file,
        field,
        size,
        symmetric,
        reread,
        format,
    })
}

fn field_spec(p: &mut Parser) -> PResult<Field> {
    field(p)
}

fn write(p: &mut Parser, carried: &mut Carried) -> PResult<MatrixWrite> {
    let start = p.span();
    let expr = operand(p, "WRITE")?;
    let (mut outfile, mut field, mut triangular, mut hold, mut format) =
        (None, None, false, false, None);
    while p.eat(Punct::Slash) {
        if p.eat_keyword("OUTFILE") {
            outfile = Some(file_or_handle(p)?);
        } else if p.eat_keyword("FIELD") {
            field = Some(field_spec(p)?);
        } else if p.eat_keyword("MODE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("TRIANGULAR") {
                triangular = true;
            } else if p.eat_keyword("RECTANGULAR") {
                triangular = false;
            } else {
                return Err(p.unexpected(&["RECTANGULAR", "TRIANGULAR"]));
            }
        } else if p.eat_keyword("HOLD") {
            hold = true;
        } else if p.eat_keyword("FORMAT") {
            format = Some(io_format(p, field.as_ref())?);
        } else {
            return Err(p.unexpected(&["OUTFILE", "FIELD", "MODE", "HOLD", "FORMAT"]));
        }
    }
    let Some(field) = field else {
        return Err(SyntaxError::new(start, "WRITE needs FIELD = first TO last"));
    };
    check_width(&field, format.as_ref(), start)?;
    let outfile = match outfile {
        Some(file) => {
            carried.write = Some(file.clone());
            file
        }
        None => carried.write.clone().ok_or_else(|| {
            SyntaxError::new(start, "the first WRITE of the program needs OUTFILE")
        })?,
    };
    Ok(MatrixWrite {
        expr,
        outfile,
        field,
        triangular,
        hold,
        format,
    })
}

/// A file or `*` for the active dataset (None).
fn file_or_active(p: &mut Parser) -> PResult<Option<String>> {
    p.eat(Punct::Eq);
    if p.eat(Punct::Star) {
        Ok(None)
    } else {
        p.file("a quoted file name, a file handle, a dataset or *")
            .map(Some)
    }
}

fn missing_action(p: &mut Parser, accept: bool) -> PResult<MissingAction> {
    p.eat(Punct::Eq);
    if accept && p.eat_keyword("ACCEPT") {
        Ok(MissingAction::Accept)
    } else if p.eat_keyword("OMIT") {
        Ok(MissingAction::Omit)
    } else if let Some(n) = p.number() {
        Ok(MissingAction::Value(n))
    } else if accept {
        Err(p.unexpected(&["ACCEPT", "OMIT", "a number"]))
    } else {
        Err(p.unexpected(&["OMIT", "a number"]))
    }
}

fn get(p: &mut Parser) -> PResult<MatrixGet> {
    let target = target(p)?;
    let mut get = MatrixGet {
        target,
        file: None,
        variables: None,
        names: None,
        missing: None,
        sysmis: None,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("FILE") {
            get.file = file_or_active(p)?;
        } else if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            get.variables = Some(p.var_list()?);
        } else if p.eat_keyword("NAMES") {
            p.eat(Punct::Eq);
            get.names = Some(variable_name(p, "a name for the vector of names")?);
        } else if p.eat_keyword("MISSING") {
            get.missing = Some(missing_action(p, true)?);
        } else if p.eat_keyword("SYSMIS") {
            get.sysmis = Some(missing_action(p, false)?);
        } else {
            return Err(p.unexpected(&["FILE", "VARIABLES", "NAMES", "MISSING", "SYSMIS"]));
        }
    }
    Ok(get)
}

/// New variable names separated by commas or blanks, `X1 TO X5` as on the manual's example.
fn new_names(p: &mut Parser) -> PResult<Vec<String>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        let span = p.span();
        let first = p.name("a variable name")?;
        if p.eat_word("TO") {
            let last = p.name("a variable name")?;
            let names = numbered_names(&first, &last).map_err(|m| SyntaxError::new(span, m))?;
            out.extend(names);
        } else {
            out.push(first);
        }
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(out)
}

fn save(p: &mut Parser, carried: &mut Carried) -> PResult<MatrixSave> {
    let start = p.span();
    let expr = operand(p, "SAVE")?;
    let (mut outfile, mut variables, mut names, mut strings) = (None, None, None, Vec::new());
    while p.eat(Punct::Slash) {
        if p.eat_keyword("OUTFILE") {
            outfile = Some(file_or_active(p)?);
        } else if p.eat_keyword("VARIABLES") {
            variables = Some(new_names(p)?);
        } else if p.eat_keyword("NAMES") {
            p.eat(Punct::Eq);
            names = Some(sub_expr(p)?);
        } else if p.eat_keyword("STRINGS") {
            strings = new_names(p)?;
        } else {
            return Err(p.unexpected(&["OUTFILE", "VARIABLES", "NAMES", "STRINGS"]));
        }
    }
    let outfile = match outfile {
        Some(file) => {
            carried.save = Some(file.clone());
            file
        }
        None => carried.save.clone().ok_or_else(|| {
            SyntaxError::new(start, "the first SAVE of the program needs OUTFILE")
                .hint("OUTFILE=* replaces the active dataset")
        })?,
    };
    Ok(MatrixSave {
        expr,
        outfile,
        variables,
        names,
        strings,
    })
}

const ROW_TYPES: [&str; 6] = ["COV", "CORR", "MEAN", "STDDEV", "N", "COUNT"];

fn row_type(p: &mut Parser) -> Option<String> {
    let found = ROW_TYPES
        .iter()
        .find(|t| matches!(p.peek(), Some(TokenKind::Word(w)) if matches_keyword(w, t)))?;
    p.pos += 1;
    Some(found.to_string())
}

fn mget(p: &mut Parser) -> PResult<MatrixMget> {
    let mut mget = MatrixMget {
        file: None,
        types: Vec::new(),
    };
    loop {
        let slash = p.eat(Punct::Slash);
        if p.eat_keyword("FILE") {
            mget.file = file_or_active(p)?;
        } else if slash && p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            while let Some(kind) = row_type(p) {
                mget.types.push(kind);
                p.eat(Punct::Comma);
            }
            if mget.types.is_empty() {
                return Err(p.unexpected(&ROW_TYPES));
            }
        } else if slash {
            return Err(p.unexpected(&["FILE", "TYPE"]));
        } else {
            return Ok(mget);
        }
    }
}

fn msave(p: &mut Parser, carried: &mut Carried) -> PResult<MatrixMsave> {
    let start = p.span();
    let expr = operand(p, "MSAVE")?;
    let mut kind = None;
    let mut outfile = None;
    let mut state = carried.msave.clone();
    let (mut variables, mut snames, mut split, mut fnames, mut factor) =
        (None, None, None, None, None);
    while p.eat(Punct::Slash) {
        if p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            kind = Some(row_type(p).ok_or_else(|| p.unexpected(&ROW_TYPES))?);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            let span = p.span();
            let file = file_or_handle(p)?;
            if let Some(state) = &state
                && !state.outfile.eq_ignore_ascii_case(&file)
            {
                return Err(SyntaxError::new(
                    span,
                    format!(
                        "a matrix program writes one matrix file; an earlier MSAVE writes {}",
                        state.outfile
                    ),
                ));
            }
            outfile = Some(file);
        } else if p.eat_keyword("VARIABLES") {
            variables = Some(new_names(p)?);
        } else if p.eat_keyword("SNAMES") {
            snames = Some(new_names(p)?);
        } else if p.eat_keyword("FNAMES") {
            fnames = Some(new_names(p)?);
        } else if p.eat_keyword("SPLIT") {
            p.eat(Punct::Eq);
            split = Some(sub_expr(p)?);
        } else if p.eat_keyword("FACTOR") {
            p.eat(Punct::Eq);
            factor = Some(sub_expr(p)?);
        } else {
            return Err(p.unexpected(&[
                "TYPE",
                "OUTFILE",
                "VARIABLES",
                "SNAMES",
                "SPLIT",
                "FNAMES",
                "FACTOR",
            ]));
        }
    }
    let Some(kind) = kind else {
        return Err(SyntaxError::new(start, "MSAVE needs TYPE")
            .hint("TYPE = COV, CORR, MEAN, STDDEV, N or COUNT"));
    };
    let state = match (state.take(), outfile) {
        (Some(mut state), _) => {
            // the other specifications carry over to the next MSAVE
            state.variables = variables.or(state.variables);
            state.snames = snames.or(state.snames);
            state.split = split.or(state.split);
            state.fnames = fnames.or(state.fnames);
            state
        }
        (None, Some(outfile)) => MsaveCarried {
            outfile,
            variables,
            snames,
            split,
            fnames,
        },
        (None, None) => {
            return Err(SyntaxError::new(
                start,
                "the first MSAVE of the program needs OUTFILE",
            ));
        }
    };
    carried.msave = Some(state.clone());
    Ok(MatrixMsave {
        expr,
        kind,
        outfile: state.outfile,
        variables: state.variables,
        snames: state.snames,
        split: state.split,
        fnames: state.fnames,
        factor,
    })
}

// ---- expressions ----

fn node(p: &Parser, start: usize, kind: ExprKind) -> MExpr {
    MExpr {
        kind,
        span: p.span_from(start),
    }
}

fn binary(p: &Parser, start: usize, op: MatrixOp, left: MExpr, right: MExpr) -> MExpr {
    node(
        p,
        start,
        ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
    )
}

pub(crate) fn expr(p: &mut Parser) -> PResult<MExpr> {
    p.nested(or)
}

fn or(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = and(p)?;
    loop {
        let op = if p.eat_word("OR") || p.eat(Punct::Or) {
            MatrixOp::Or
        } else if p.eat_word("XOR") {
            MatrixOp::Xor
        } else {
            return Ok(left);
        };
        p.count_operator()?;
        let right = and(p)?;
        left = binary(p, start, op, left, right);
    }
}

fn and(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = not(p)?;
    while p.eat_word("AND") || p.eat(Punct::And) {
        p.count_operator()?;
        let right = not(p)?;
        left = binary(p, start, MatrixOp::And, left, right);
    }
    Ok(left)
}

fn not(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    if p.eat_word("NOT") || p.eat(Punct::Not) {
        let operand = p.nested(not)?;
        return Ok(node(p, start, ExprKind::Not(Box::new(operand))));
    }
    relation(p)
}

fn relation(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = additive(p)?;
    loop {
        let op = match p.peek() {
            Some(TokenKind::Punct(Punct::Eq)) => MatrixOp::Eq,
            Some(TokenKind::Punct(Punct::Ne)) => MatrixOp::Ne,
            Some(TokenKind::Punct(Punct::Lt)) => MatrixOp::Lt,
            Some(TokenKind::Punct(Punct::Le)) => MatrixOp::Le,
            Some(TokenKind::Punct(Punct::Gt)) => MatrixOp::Gt,
            Some(TokenKind::Punct(Punct::Ge)) => MatrixOp::Ge,
            Some(TokenKind::Word(w)) => match w.to_uppercase().as_str() {
                "EQ" => MatrixOp::Eq,
                "NE" => MatrixOp::Ne,
                "LT" => MatrixOp::Lt,
                "LE" => MatrixOp::Le,
                "GT" => MatrixOp::Gt,
                "GE" => MatrixOp::Ge,
                _ => return Ok(left),
            },
            _ => return Ok(left),
        };
        p.pos += 1;
        p.count_operator()?;
        let right = additive(p)?;
        left = binary(p, start, op, left, right);
    }
}

fn additive(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = multiplicative(p)?;
    loop {
        let op = if p.eat(Punct::Plus) {
            MatrixOp::Add
        } else if p.eat(Punct::Minus) {
            MatrixOp::Sub
        } else {
            return Ok(left);
        };
        p.count_operator()?;
        let right = multiplicative(p)?;
        left = binary(p, start, op, left, right);
    }
}

fn multiplicative(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = power(p)?;
    loop {
        let op = if p.eat(Punct::Star) {
            MatrixOp::Mul
        } else if !p.slash_ends && p.eat(Punct::Slash) {
            MatrixOp::Div
        } else if p.eat(Punct::ElemMul) {
            MatrixOp::ElemMul
        } else if p.eat(Punct::ElemDiv) {
            MatrixOp::ElemDiv
        } else {
            return Ok(left);
        };
        p.count_operator()?;
        let right = power(p)?;
        left = binary(p, start, op, left, right);
    }
}

fn power(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let mut left = sequence(p)?;
    loop {
        let op = if p.eat(Punct::Pow) {
            MatrixOp::Pow
        } else if p.eat(Punct::ElemPow) {
            MatrixOp::ElemPow
        } else {
            return Ok(left);
        };
        p.count_operator()?;
        let right = sequence(p)?;
        left = binary(p, start, op, left, right);
    }
}

fn sequence(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    let first = unary(p)?;
    if !p.eat(Punct::Colon) {
        return Ok(first);
    }
    let last = unary(p)?;
    let step = if p.eat(Punct::Colon) {
        Some(Box::new(unary(p)?))
    } else {
        None
    };
    Ok(node(
        p,
        start,
        ExprKind::Sequence {
            first: Box::new(first),
            last: Box::new(last),
            step,
        },
    ))
}

fn unary(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    if p.eat(Punct::Minus) {
        let operand = p.nested(unary)?;
        return Ok(node(p, start, ExprKind::Neg(Box::new(operand))));
    }
    if p.eat(Punct::Plus) {
        return p.nested(unary);
    }
    primary(p)
}

/// An expression that a subcommand gives: a slash ends it, unless it is in brackets.
fn sub_expr(p: &mut Parser) -> PResult<MExpr> {
    let saved = std::mem::replace(&mut p.slash_ends, true);
    let result = expr(p);
    p.slash_ends = saved;
    result
}

fn primary(p: &mut Parser) -> PResult<MExpr> {
    // inside brackets a slash divides again
    let saved = std::mem::replace(&mut p.slash_ends, false);
    let result = bracketed(p);
    p.slash_ends = saved;
    result
}

fn bracketed(p: &mut Parser) -> PResult<MExpr> {
    let start = p.pos;
    match p.peek() {
        Some(TokenKind::Number(n)) => {
            let n = *n;
            p.pos += 1;
            Ok(node(p, start, ExprKind::Number(n)))
        }
        Some(TokenKind::Str(s)) => {
            let s = s.clone();
            p.pos += 1;
            Ok(node(p, start, ExprKind::String(s)))
        }
        Some(TokenKind::Punct(Punct::LParen)) => {
            p.pos += 1;
            let inner = expr(p)?;
            p.expect(Punct::RParen)?;
            Ok(MExpr {
                span: p.span_from(start),
                ..inner
            })
        }
        Some(TokenKind::Punct(Punct::LBrace)) => {
            p.pos += 1;
            let mut rows = Vec::new();
            if !p.eat(Punct::RBrace) {
                let mut row = Vec::new();
                loop {
                    row.push(expr(p)?);
                    if p.eat(Punct::Comma) {
                        continue;
                    }
                    rows.push(std::mem::take(&mut row));
                    if p.eat(Punct::Semicolon) {
                        continue;
                    }
                    if p.eat(Punct::RBrace) {
                        break;
                    }
                    return Err(p.unexpected(&["','", "';'", "'}'"]));
                }
            }
            Ok(node(p, start, ExprKind::Matrix(rows)))
        }
        // ALL is a reserved word and a matrix function
        Some(TokenKind::Word(w))
            if is_reserved(w)
                && !(w.eq_ignore_ascii_case("ALL")
                    && p.peek_at(1) == Some(&TokenKind::Punct(Punct::LParen))) =>
        {
            Err(p.unexpected(&["an operand"]))
        }
        Some(TokenKind::Word(w)) => {
            let name = w.clone();
            let span = p.span();
            p.pos += 1;
            let function = matrix_function(&name);
            if !p.at(Punct::LParen) {
                if function.is_some() {
                    return Err(SyntaxError::new(
                        span,
                        format!(
                            "{} is a matrix function and needs its arguments in parentheses",
                            name.to_uppercase()
                        ),
                    ));
                }
                return Ok(node(p, start, ExprKind::Variable(name)));
            }
            p.pos += 1;
            let Some((function, low, high)) = function else {
                let indexes = indexes(p)?;
                return Ok(node(p, start, ExprKind::Index { name, indexes }));
            };
            if function == "EOF" {
                let file = p.file("a quoted file name or a file handle")?;
                p.expect(Punct::RParen)?;
                return Ok(node(p, start, ExprKind::Eof(file)));
            }
            let mut args = Vec::new();
            if !p.eat(Punct::RParen) {
                loop {
                    args.push(expr(p)?);
                    if p.eat(Punct::RParen) {
                        break;
                    }
                    p.expect(Punct::Comma)?;
                }
            }
            if args.len() < low || args.len() > high {
                let wanted = match (low, high) {
                    (l, h) if l == h => format!("{l}"),
                    (l, usize::MAX) => format!("at least {l}"),
                    (l, h) => format!("{l} or {h}"),
                };
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!(
                        "{function} takes {wanted} argument{}, got {}",
                        if wanted == "1" { "" } else { "s" },
                        args.len()
                    ),
                ));
            }
            Ok(node(
                p,
                start,
                ExprKind::Call {
                    name: function,
                    args,
                },
            ))
        }
        _ => Err(p.unexpected(&["an operand"])),
    }
}

/// MATRIX itself takes nothing; `parse_raws` gives it the statements up to END MATRIX.
pub(crate) fn matrix_command(p: &mut Parser) -> PResult<crate::ast::Command> {
    p.finish(&[])?;
    Ok(crate::ast::Command::Matrix(Box::default()))
}
