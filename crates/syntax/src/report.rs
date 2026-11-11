//! Parser of REPORT: case listings and summary statistics laid out as a report.
//!
//! Rules from the command's description:
//! - FORMAT comes first; STRING before VARIABLES; VARIABLES (required, once) before BREAK and
//!   MISSING; MISSING before the first BREAK; each SUMMARY follows its BREAK, and several
//!   SUMMARY subcommands of one BREAK follow each other. TITLE and FOOTNOTE come anywhere
//!   after FORMAT, but not between a BREAK and its SUMMARY. FORMAT, STRING, VARIABLES and
//!   MISSING at most once.
//! - Options in parentheses follow a variable (or a TO list) and apply to it; headings are
//!   quoted strings, one per line. Stacked columns join variables with +.
//! - BREAK=(TOTAL) without variables, on the first BREAK, gives totals for a report without
//!   break groups.
//! - SUMMARY takes aggregate functions, each with optional (variables (format)(decimals)),
//!   and composite functions with their arguments and an optional (column (format)(decimals));
//!   then an optional title, (break column number) and SKIP(n). PREVIOUS[(n)] stands alone.
//! - Function names and aliases: VALIDN, SUM, MIN, MAX, MEAN, STDDEV (SD, STDEV), VARIANCE,
//!   KURTOSIS, SKEWNESS, MEDIAN(min,max), MODE(min,max), PGT(n) (PCGT), PLT(n) (PCLT),
//!   PIN(min,max) (PCIN), FREQUENCY(min,max), PERCENT(min,max); DIVIDE, MULTIPLY, PCT,
//!   SUBTRACT, ADD, GREAT, LEAST, AVERAGE. FREQUENCY and PERCENT stand alone on their line.

use crate::ast::{Command, Format, FormatType, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportOffset {
    Columns(u32),
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportFormat {
    pub automatic: bool,
    /// None is NOLIST; Some(n) lists the cases with a blank line after every n (0: none).
    pub list: Option<u32>,
    pub align: ReportAlign,
    pub tspace: u32,
    pub chdspace: u32,
    pub ftspace: u32,
    pub sumspace: u32,
    pub colspace: Option<u32>,
    /// None takes the default of MANUAL (1) or AUTOMATIC (-1).
    pub brkspace: Option<i32>,
    pub length: (Option<u32>, Option<u32>),
    pub margins: (Option<u32>, Option<u32>),
    /// None takes the default: TOP for MANUAL, BOTTOM for AUTOMATIC.
    pub chalign_top: Option<bool>,
    pub underscore: Option<bool>,
    pub page1: u32,
    pub missing: char,
    pub one_break_column: bool,
    pub indent: u32,
    pub chwrap: bool,
    pub preview: bool,
}

impl Default for ReportFormat {
    fn default() -> Self {
        Self {
            automatic: false,
            list: None,
            align: ReportAlign::Left,
            tspace: 1,
            chdspace: 1,
            ftspace: 1,
            sumspace: 1,
            colspace: None,
            brkspace: None,
            length: (None, None),
            margins: (None, None),
            chalign_top: None,
            underscore: None,
            page1: 1,
            missing: '.',
            one_break_column: false,
            indent: 2,
            chwrap: false,
            preview: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnContents {
    Value,
    Label,
    Dummy,
}

/// One report column, or with a TO list one per variable, each with the same options.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportColumn {
    /// The variable and those stacked below it with +, each with what it shows.
    pub variables: Vec<(VarItem, ColumnContents)>,
    pub heading: Option<Vec<String>>,
    pub width: Option<u32>,
    pub offset: Option<ReportOffset>,
    pub align: Option<ReportAlign>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Variable {
        name: String,
        width: Option<u32>,
        blank: bool,
    },
    Literal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportString {
    pub name: String,
    pub parts: Vec<StringPart>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReportMissing {
    Var,
    None,
    /// Cases missing on at least n of the variables (all report variables when empty) are
    /// left out.
    List(Vec<VarItem>, u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportFunction {
    ValidN,
    Sum,
    Min,
    Max,
    Mean,
    StdDev,
    Variance,
    Kurtosis,
    Skewness,
    Median(f64, f64),
    Mode(f64, f64),
    Pgt(f64),
    Plt(f64),
    Pin(f64, f64),
    Frequency(f64, f64),
    Percent(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Composite {
    Divide,
    Multiply,
    Pct,
    Subtract,
    Add,
    Great,
    Least,
    Average,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompositeArg {
    Aggregate(ReportFunction, String),
    Constant(f64),
}

/// `(format)(decimals)` after a variable of a summary. A format without width is the type.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SummaryFormat {
    pub plain: bool,
    pub format: Option<Format>,
    /// A format given without width, such as (DOLLAR).
    pub kind: Option<FormatType>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryItem {
    /// No variables: every report variable.
    Aggregate {
        function: ReportFunction,
        variables: Vec<(String, SummaryFormat)>,
    },
    Composite {
        function: Composite,
        args: Vec<CompositeArg>,
        column: Option<(String, SummaryFormat)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReportSummary {
    Line {
        items: Vec<SummaryItem>,
        title: Option<String>,
        /// The break column (from 1) the title goes in.
        title_column: Option<u32>,
        skip: u32,
    },
    /// The summaries of BREAK n (from 1); None: of the BREAK before.
    Previous(Option<u32>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportBreak {
    /// Empty for `BREAK=(TOTAL)` without variables.
    pub variables: Vec<VarItem>,
    pub heading: Option<Vec<String>>,
    pub width: Option<u32>,
    /// None takes the default: LABEL for AUTOMATIC, VALUE for MANUAL.
    pub label: Option<bool>,
    pub total: bool,
    pub skip: u32,
    pub page: bool,
    pub offset: Option<ReportOffset>,
    /// UNDERSCORE: Some(empty) for every column.
    pub underscore: Option<Vec<String>>,
    pub align: Option<ReportAlign>,
    pub name: bool,
    pub summaries: Vec<ReportSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub format: ReportFormat,
    pub outfile: Option<String>,
    pub strings: Vec<ReportString>,
    pub columns: Vec<ReportColumn>,
    pub missing: ReportMissing,
    pub titles: Vec<(ReportAlign, Vec<String>)>,
    pub footnotes: Vec<(ReportAlign, Vec<String>)>,
    pub breaks: Vec<ReportBreak>,
}

/// The functions by name with their aliases.
const FUNCTIONS: [&str; 19] = [
    "VALIDN", "SUM", "MIN", "MAX", "MEAN", "STDDEV", "SD", "STDEV", "VARIANCE", "KURTOSIS",
    "SKEWNESS", "MEDIAN", "MODE", "PGT", "PCGT", "PLT", "PCLT", "PIN", "PCIN",
];
const MORE_FUNCTIONS: [&str; 2] = ["FREQUENCY", "PERCENT"];
const COMPOSITES: [(&str, Composite); 8] = [
    ("DIVIDE", Composite::Divide),
    ("MULTIPLY", Composite::Multiply),
    ("PCT", Composite::Pct),
    ("SUBTRACT", Composite::Subtract),
    ("ADD", Composite::Add),
    ("GREAT", Composite::Great),
    ("LEAST", Composite::Least),
    ("AVERAGE", Composite::Average),
];

/// The name `word` stands for among `names`: written in full, or the one name it abbreviates.
fn lookup<'a>(word: &str, names: impl Iterator<Item = &'a str> + Clone) -> Option<&'a str> {
    if let Some(name) = names.clone().find(|n| n.eq_ignore_ascii_case(word)) {
        return Some(name);
    }
    let mut found = names.filter(|n| matches_keyword(word, n));
    match (found.next(), found.next()) {
        (Some(name), None) => Some(name),
        _ => None,
    }
}

fn whole(p: &mut Parser, what: &str) -> PResult<u32> {
    match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 0.0 && *n <= 1e6 => {
            let n = *n as u32;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

/// `(n)` after a FORMAT keyword.
fn paren_whole(p: &mut Parser, what: &str) -> PResult<u32> {
    p.expect(Punct::LParen)?;
    let n = whole(p, what)?;
    p.expect(Punct::RParen)?;
    Ok(n)
}

/// `(n)` or `(*)`, as in LENGTH and MARGINS; None is *.
fn pair(p: &mut Parser) -> PResult<(Option<u32>, Option<u32>)> {
    p.expect(Punct::LParen)?;
    let one = |p: &mut Parser| -> PResult<Option<u32>> {
        if p.eat(Punct::Star) {
            Ok(None)
        } else {
            whole(p, "a line or column number or *").map(Some)
        }
    };
    let first = one(p)?;
    p.eat(Punct::Comma);
    let second = one(p)?;
    p.expect(Punct::RParen)?;
    Ok((first, second))
}

/// ON or OFF, in parentheses or after a blank.
fn on_off(p: &mut Parser) -> PResult<bool> {
    let paren = p.eat(Punct::LParen);
    let on = if p.eat_keyword("ON") {
        true
    } else if p.eat_keyword("OFF") {
        false
    } else {
        return Err(p.unexpected(&["ON", "OFF"]));
    };
    if paren {
        p.expect(Punct::RParen)?;
    }
    Ok(on)
}

fn align(p: &mut Parser) -> Option<ReportAlign> {
    if p.eat_keyword("LEFT") {
        Some(ReportAlign::Left)
    } else if p.eat_keyword("CENTER") {
        Some(ReportAlign::Center)
    } else if p.eat_keyword("RIGHT") {
        Some(ReportAlign::Right)
    } else {
        None
    }
}

fn format(p: &mut Parser) -> PResult<ReportFormat> {
    let mut f = ReportFormat::default();
    p.eat(Punct::Eq);
    while !at_end(p) {
        if p.eat_keyword("MANUAL") {
            f.automatic = false;
        } else if p.eat_keyword("AUTOMATIC") {
            f.automatic = true;
        } else if p.eat_keyword("NOLIST") {
            f.list = None;
        } else if p.eat_keyword("LIST") {
            f.list = Some(if p.at(Punct::LParen) {
                paren_whole(p, "a number of cases")?
            } else {
                0
            });
        } else if p.eat_keyword("ALIGN") {
            p.expect(Punct::LParen)?;
            f.align = align(p).ok_or_else(|| p.unexpected(&["LEFT", "CENTER", "RIGHT"]))?;
            p.expect(Punct::RParen)?;
        } else if p.eat_keyword("TSPACE") {
            f.tspace = paren_whole(p, "a number of lines")?;
        } else if p.eat_keyword("CHDSPACE") {
            f.chdspace = paren_whole(p, "a number of lines")?;
        } else if p.eat_keyword("FTSPACE") {
            f.ftspace = paren_whole(p, "a number of lines")?;
        } else if p.eat_keyword("SUMSPACE") {
            f.sumspace = paren_whole(p, "a number of lines")?;
        } else if p.eat_keyword("COLSPACE") {
            f.colspace = Some(paren_whole(p, "a number of columns")?);
        } else if p.eat_keyword("BRKSPACE") {
            p.expect(Punct::LParen)?;
            let at = p.pos;
            f.brkspace = match p.number() {
                Some(n) if n.fract() == 0.0 && (-1.0..=1e6).contains(&n) => Some(n as i32),
                _ => {
                    p.pos = at;
                    return Err(p.unexpected(&["a number of lines, or -1"]));
                }
            };
            p.expect(Punct::RParen)?;
        } else if p.eat_keyword("LENGTH") {
            f.length = pair(p)?;
        } else if p.eat_keyword("MARGINS") {
            f.margins = pair(p)?;
        } else if p.eat_keyword("CHALIGN") {
            p.expect(Punct::LParen)?;
            f.chalign_top = Some(if p.eat_keyword("TOP") {
                true
            } else if p.eat_keyword("BOTTOM") {
                false
            } else {
                return Err(p.unexpected(&["TOP", "BOTTOM"]));
            });
            p.expect(Punct::RParen)?;
        } else if p.eat_keyword("UNDERSCORE") {
            f.underscore = Some(on_off(p)?);
        } else if p.eat_keyword("PAGE1") || p.eat_word("PAGE") {
            f.page1 = paren_whole(p, "a page number")?;
        } else if p.eat_keyword("MISSING") {
            let paren = p.eat(Punct::LParen);
            let at = p.pos;
            let text = p.string("one character in quotes")?;
            let mut chars = text.chars();
            f.missing = match (chars.next(), chars.next()) {
                (Some(c), None) => c,
                _ => {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "the missing-value symbol is one character",
                    ));
                }
            };
            if paren {
                p.expect(Punct::RParen)?;
            }
        } else if p.eat_keyword("ONEBREAKCOLUMN") {
            f.one_break_column = on_off(p)?;
        } else if p.eat_keyword("INDENT") {
            f.indent = if p.at(Punct::LParen) {
                paren_whole(p, "a number of columns")?
            } else {
                whole(p, "a number of columns")?
            };
        } else if p.eat_keyword("CHWRAP") {
            f.chwrap = on_off(p)?;
        } else if p.eat_keyword("PREVIEW") {
            f.preview = on_off(p)?;
        } else {
            return Err(p.unexpected(&[
                "MANUAL",
                "AUTOMATIC",
                "LIST",
                "NOLIST",
                "ALIGN",
                "TSPACE",
                "CHDSPACE",
                "FTSPACE",
                "SUMSPACE",
                "COLSPACE",
                "BRKSPACE",
                "LENGTH",
                "MARGINS",
                "CHALIGN",
                "UNDERSCORE",
                "PAGE1",
                "MISSING",
                "ONEBREAKCOLUMN",
                "INDENT",
                "CHWRAP",
                "PREVIEW",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(f)
}

/// Quoted heading lines, if any.
fn heading(p: &mut Parser) -> PResult<Option<Vec<String>>> {
    let mut lines = Vec::new();
    while let Some(TokenKind::Str(_)) = p.peek() {
        lines.push(p.string("a heading")?);
    }
    Ok((!lines.is_empty()).then_some(lines))
}

/// `(OFFSET(n | CENTER))` after the keyword.
fn offset(p: &mut Parser) -> PResult<ReportOffset> {
    p.expect(Punct::LParen)?;
    let offset = if p.eat_keyword("CENTER") {
        ReportOffset::Center
    } else {
        ReportOffset::Columns(whole(p, "a number of columns or CENTER")?)
    };
    p.expect(Punct::RParen)?;
    Ok(offset)
}

fn variables(p: &mut Parser) -> PResult<Vec<ReportColumn>> {
    p.eat(Punct::Eq);
    let mut columns = Vec::new();
    while !at_end(p) {
        let mut stacked = Vec::new();
        loop {
            let first = p.name("a variable name")?;
            let item = if p.eat_word("TO") {
                VarItem::Range(first, p.name("a variable name")?)
            } else {
                VarItem::Name(first)
            };
            let mut contents = ColumnContents::Value;
            if p.at(Punct::LParen)
                && let Some(TokenKind::Word(w)) = p.peek_at(1)
                && p.peek_at(2) == Some(&TokenKind::Punct(Punct::RParen))
            {
                let found = [
                    ("VALUE", ColumnContents::Value),
                    ("LABEL", ColumnContents::Label),
                    ("DUMMY", ColumnContents::Dummy),
                ]
                .into_iter()
                .find(|(k, _)| matches_keyword(w, k));
                if let Some((_, c)) = found {
                    contents = c;
                    p.pos += 3;
                }
            }
            stacked.push((item, contents));
            if !p.eat(Punct::Plus) {
                break;
            }
        }
        let mut column = ReportColumn {
            variables: stacked,
            heading: None,
            width: None,
            offset: None,
            align: None,
        };
        loop {
            if let Some(lines) = heading(p)? {
                column.heading = Some(lines);
            } else if p.eat(Punct::LParen) {
                if let Some(TokenKind::Number(_)) = p.peek() {
                    column.width = Some(whole(p, "a column width")?);
                } else if p.eat_keyword("OFFSET") {
                    column.offset = Some(offset(p)?);
                } else if let Some(a) = align(p) {
                    column.align = Some(a);
                } else if p.at_keyword("VALUE") || p.at_keyword("LABEL") || p.at_keyword("DUMMY") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "VALUE, LABEL or DUMMY comes right after the variable name",
                    ));
                } else {
                    return Err(p.unexpected(&["a width", "OFFSET", "LEFT", "CENTER", "RIGHT"]));
                }
                p.expect(Punct::RParen)?;
            } else {
                break;
            }
        }
        columns.push(column);
        p.eat(Punct::Comma);
    }
    if columns.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(columns)
}

fn strings(p: &mut Parser) -> PResult<Vec<ReportString>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while !at_end(p) {
        let name = p.name("a name for the string")?;
        p.expect(Punct::LParen)?;
        let mut parts = Vec::new();
        while !p.eat(Punct::RParen) {
            if let Some(TokenKind::Str(_)) = p.peek() {
                parts.push(StringPart::Literal(p.string("a literal")?));
                continue;
            }
            let variable = p.name("a variable name or a literal in quotes")?;
            let (mut width, mut blank) = (None, false);
            while p.at(Punct::LParen) {
                p.pos += 1;
                if p.eat_keyword("BLANK") {
                    blank = true;
                } else {
                    let n = whole(p, "a width or BLANK")?;
                    if n == 0 {
                        return Err(SyntaxError::new(
                            p.span_from(p.pos - 1),
                            "a width is at least 1",
                        ));
                    }
                    width = Some(n);
                }
                p.expect(Punct::RParen)?;
            }
            parts.push(StringPart::Variable {
                name: variable,
                width,
                blank,
            });
        }
        if parts.is_empty() {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                format!("the string {name} needs a variable or a literal"),
            ));
        }
        out.push(ReportString { name, parts });
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a name for the string"]));
    }
    Ok(out)
}

fn missing(p: &mut Parser) -> PResult<ReportMissing> {
    p.eat(Punct::Eq);
    let missing = if p.eat_keyword("VAR") {
        ReportMissing::Var
    } else if p.eat_keyword("NONE") {
        ReportMissing::None
    } else if p.eat_keyword("LIST") {
        let (mut vars, mut n) = (Vec::new(), 1);
        if p.eat(Punct::LParen) {
            if matches!(p.peek(), Some(TokenKind::Word(_))) {
                vars = p.var_list()?;
            }
            if let Some(TokenKind::Number(_)) = p.peek() {
                n = whole(p, "a number of missing values")?.max(1);
            }
            p.expect(Punct::RParen)?;
        }
        ReportMissing::List(vars, n)
    } else {
        return Err(p.unexpected(&["VAR", "NONE", "LIST"]));
    };
    if !at_end(p) {
        return Err(p.unexpected(&["'/'", "end of command"]));
    }
    Ok(missing)
}

/// TITLE or FOOTNOTE: groups of lines, each after an optional LEFT, CENTER or RIGHT.
fn titles(p: &mut Parser) -> PResult<Vec<(ReportAlign, Vec<String>)>> {
    p.eat(Punct::Eq);
    let mut out: Vec<(ReportAlign, Vec<String>)> = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        let place = align(p).unwrap_or(ReportAlign::Center);
        if out.iter().any(|(a, _)| *a == place) {
            return Err(SyntaxError::new(
                p.span_from(at),
                "LEFT, CENTER and RIGHT can each be given once",
            ));
        }
        match heading(p)? {
            Some(lines) => out.push((place, lines)),
            None => return Err(p.unexpected(&["a line in quotes"])),
        }
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a line in quotes"]));
    }
    Ok(out)
}

fn break_options(p: &mut Parser, b: &mut ReportBreak) -> PResult<()> {
    loop {
        if let Some(lines) = heading(p)? {
            b.heading = Some(lines);
            continue;
        }
        if !p.eat(Punct::LParen) {
            return Ok(());
        }
        if let Some(TokenKind::Number(_)) = p.peek() {
            b.width = Some(whole(p, "a column width")?);
        } else if p.eat_keyword("VALUE") {
            b.label = Some(false);
        } else if p.eat_keyword("LABEL") {
            b.label = Some(true);
        } else if p.eat_keyword("NOTOTAL") {
            b.total = false;
        } else if p.eat_keyword("TOTAL") {
            b.total = true;
        } else if p.eat_keyword("SKIP") {
            b.skip = paren_whole(p, "a number of lines")?;
        } else if p.eat_keyword("PAGE") {
            b.page = true;
            if p.eat(Punct::LParen) {
                if !p.eat_keyword("RESET") {
                    return Err(p.unexpected(&["RESET"]));
                }
                p.expect(Punct::RParen)?;
            }
        } else if p.eat_keyword("OFFSET") {
            b.offset = Some(offset(p)?);
        } else if p.eat_keyword("UNDERSCORE") {
            let mut names = Vec::new();
            if p.eat(Punct::LParen) {
                while !p.eat(Punct::RParen) {
                    names.push(p.name("a report variable")?);
                    p.eat(Punct::Comma);
                }
            }
            b.underscore = Some(names);
        } else if p.eat_keyword("NONAME") {
            b.name = false;
        } else if p.eat_keyword("NAME") {
            b.name = true;
        } else if let Some(a) = align(p) {
            b.align = Some(a);
        } else {
            return Err(p.unexpected(&[
                "a width",
                "VALUE",
                "LABEL",
                "TOTAL",
                "NOTOTAL",
                "SKIP",
                "PAGE",
                "OFFSET",
                "UNDERSCORE",
                "LEFT",
                "CENTER",
                "RIGHT",
                "NAME",
                "NONAME",
            ]));
        }
        p.expect(Punct::RParen)?;
    }
}

fn break_command(p: &mut Parser, first: bool) -> PResult<ReportBreak> {
    p.eat(Punct::Eq);
    let mut b = ReportBreak {
        variables: Vec::new(),
        heading: None,
        width: None,
        label: None,
        total: false,
        skip: 1,
        page: false,
        offset: None,
        underscore: None,
        align: None,
        name: false,
        summaries: Vec::new(),
    };
    if matches!(p.peek(), Some(TokenKind::Word(_))) {
        b.variables = p.var_list()?;
    }
    let at = p.pos;
    break_options(p, &mut b)?;
    if !at_end(p) {
        return Err(p.unexpected(&["a heading in quotes", "'('", "'/'"]));
    }
    if b.total && !first {
        return Err(SyntaxError::new(
            p.span_from(at),
            "TOTAL goes on the first BREAK",
        ));
    }
    if b.variables.is_empty() && !(first && b.total) {
        return Err(SyntaxError::new(
            p.span_from(at),
            "BREAK needs variables, or (TOTAL) alone on the first BREAK",
        ));
    }
    Ok(b)
}

/// `(min, max)` or `(n)` arguments of a function.
fn arguments(p: &mut Parser, count: usize, function: &str) -> PResult<Vec<f64>> {
    p.expect(Punct::LParen)?;
    let mut out = Vec::new();
    for i in 0..count {
        if i > 0 {
            p.eat(Punct::Comma);
        }
        out.push(
            p.number()
                .ok_or_else(|| p.unexpected(&[&format!("a number for {function}")]))?,
        );
    }
    p.expect(Punct::RParen)?;
    if count == 2 && out[0] > out[1] {
        return Err(SyntaxError::new(
            p.span_from(p.pos - 5),
            format!("{function}: the minimum is above the maximum"),
        ));
    }
    Ok(out)
}

/// An aggregate function name and its arguments, None when the next word is no aggregate.
fn aggregate(p: &mut Parser) -> PResult<Option<ReportFunction>> {
    let Some(TokenKind::Word(word)) = p.peek() else {
        return Ok(None);
    };
    let names = FUNCTIONS.iter().chain(&MORE_FUNCTIONS).copied();
    let Some(name) = lookup(word, names) else {
        return Ok(None);
    };
    // PCT is a composite function, not an abbreviation
    if word.eq_ignore_ascii_case("PCT") {
        return Ok(None);
    }
    p.pos += 1;
    Ok(Some(match name {
        "VALIDN" => ReportFunction::ValidN,
        "SUM" => ReportFunction::Sum,
        "MIN" => ReportFunction::Min,
        "MAX" => ReportFunction::Max,
        "MEAN" => ReportFunction::Mean,
        "STDDEV" | "SD" | "STDEV" => ReportFunction::StdDev,
        "VARIANCE" => ReportFunction::Variance,
        "KURTOSIS" => ReportFunction::Kurtosis,
        "SKEWNESS" => ReportFunction::Skewness,
        "PGT" | "PCGT" => ReportFunction::Pgt(arguments(p, 1, name)?[0]),
        "PLT" | "PCLT" => ReportFunction::Plt(arguments(p, 1, name)?[0]),
        _ => {
            let a = arguments(p, 2, name)?;
            match name {
                "MEDIAN" => ReportFunction::Median(a[0], a[1]),
                "MODE" => ReportFunction::Mode(a[0], a[1]),
                "PIN" | "PCIN" => ReportFunction::Pin(a[0], a[1]),
                "FREQUENCY" => ReportFunction::Frequency(a[0], a[1]),
                _ => ReportFunction::Percent(a[0], a[1]),
            }
        }
    }))
}

/// `(format)` and `(decimals)` after a variable, each optional.
fn summary_format(p: &mut Parser) -> PResult<SummaryFormat> {
    let mut f = SummaryFormat::default();
    while p.at(Punct::LParen) {
        match p.peek_at(1) {
            Some(TokenKind::Number(_)) => {
                p.pos += 1;
                let n = whole(p, "a number of decimals")?;
                f.decimals = Some(n.min(16) as u8);
            }
            Some(TokenKind::Word(w)) => {
                let w = w.clone();
                p.pos += 1;
                if w.eq_ignore_ascii_case("PLAIN") {
                    f.plain = true;
                } else if let Some(kind) = FormatType::from_name(&w) {
                    f.kind = Some(kind);
                } else {
                    let format = crate::commands::format(&w)
                        .map_err(|message| SyntaxError::new(p.span(), message))?;
                    f.format = Some(format);
                }
                p.pos += 1;
            }
            _ => return Ok(f),
        }
        p.expect(Punct::RParen)?;
    }
    Ok(f)
}

fn summary(p: &mut Parser) -> PResult<ReportSummary> {
    p.eat(Punct::Eq);
    if p.eat_keyword("PREVIOUS") {
        let n = if p.at(Punct::LParen) {
            Some(paren_whole(p, "a BREAK number")?.max(1))
        } else {
            None
        };
        if !at_end(p) {
            return Err(SyntaxError::new(
                p.span(),
                "PREVIOUS stands alone on its SUMMARY",
            ));
        }
        return Ok(ReportSummary::Previous(n));
    }
    let (mut items, mut title, mut title_column, mut skip) = (Vec::new(), None, None, 0);
    while !at_end(p) {
        let at = p.pos;
        if let Some(TokenKind::Str(_)) = p.peek() {
            if items.is_empty() {
                return Err(p.unexpected(&["a function"]));
            }
            title = Some(p.string("a title")?);
            if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Number(_))) {
                title_column = Some(paren_whole(p, "a break column number")?.max(1));
            }
            continue;
        }
        if p.eat_keyword("SKIP") {
            skip = paren_whole(p, "a number of lines")?;
            continue;
        }
        if let Some(function) = aggregate(p)? {
            let mut variables = Vec::new();
            if p.eat(Punct::LParen) {
                while !p.eat(Punct::RParen) {
                    let name = p.name("a report variable")?;
                    let format = summary_format(p)?;
                    variables.push((name, format));
                    p.eat(Punct::Comma);
                }
            }
            items.push(SummaryItem::Aggregate {
                function,
                variables,
            });
            continue;
        }
        let composite = match p.peek() {
            Some(TokenKind::Word(w)) => lookup(w, COMPOSITES.iter().map(|(n, _)| *n)),
            _ => None,
        };
        let Some(name) = composite else {
            let mut names: Vec<&str> = FUNCTIONS.to_vec();
            names.extend(MORE_FUNCTIONS);
            names.extend(COMPOSITES.iter().map(|(n, _)| *n));
            return Err(p.unexpected(&names));
        };
        p.pos += 1;
        let function = COMPOSITES.iter().find(|(n, _)| *n == name).unwrap().1;
        p.expect(Punct::LParen)?;
        let mut args = Vec::new();
        while !p.eat(Punct::RParen) {
            if let Some(n) = p.number() {
                args.push(CompositeArg::Constant(n));
            } else if let Some(f) = aggregate(p)? {
                if matches!(
                    f,
                    ReportFunction::Median(..)
                        | ReportFunction::Mode(..)
                        | ReportFunction::Frequency(..)
                        | ReportFunction::Percent(..)
                ) {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "a composite function takes VALIDN, SUM, MIN, MAX, MEAN, STDDEV, \
                         VARIANCE, KURTOSIS, SKEWNESS, PGT, PLT, PIN and numbers",
                    ));
                }
                p.expect(Punct::LParen)?;
                let name = p.name("one variable")?;
                p.expect(Punct::RParen)?;
                args.push(CompositeArg::Aggregate(f, name));
            } else {
                return Err(p.unexpected(&["an aggregate function such as SUM(var)", "a number"]));
            }
            p.eat(Punct::Comma);
        }
        let wanted = match function {
            Composite::Pct | Composite::Subtract => 2..=2,
            Composite::Divide => 2..=3,
            _ => 1..=usize::MAX,
        };
        if !wanted.contains(&args.len()) {
            return Err(SyntaxError::new(
                p.span_from(at),
                format!(
                    "{name} takes {} arguments",
                    match function {
                        Composite::Divide => "two or three",
                        Composite::Pct | Composite::Subtract => "two",
                        _ => "one or more",
                    }
                ),
            ));
        }
        let column = if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Word(_))) {
            p.pos += 1;
            let name = p.name("a report column")?;
            let format = summary_format(p)?;
            p.expect(Punct::RParen)?;
            Some((name, format))
        } else {
            None
        };
        items.push(SummaryItem::Composite {
            function,
            args,
            column,
        });
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a function"]));
    }
    let alone = items.iter().any(|i| {
        matches!(
            i,
            SummaryItem::Aggregate {
                function: ReportFunction::Frequency(..) | ReportFunction::Percent(..),
                ..
            }
        )
    });
    if alone && items.len() > 1 {
        return Err(SyntaxError::new(
            p.span(),
            "FREQUENCY and PERCENT stand alone on their summary line",
        ));
    }
    Ok(ReportSummary::Line {
        items,
        title,
        title_column,
        skip,
    })
}

fn order<T>(p: &Parser, message: &str) -> PResult<T> {
    Err(SyntaxError::new(p.span(), message.to_string()))
}

pub(crate) fn report(p: &mut Parser) -> PResult<Command> {
    let mut report = Report {
        format: ReportFormat::default(),
        outfile: None,
        strings: Vec::new(),
        columns: Vec::new(),
        missing: ReportMissing::Var,
        titles: Vec::new(),
        footnotes: Vec::new(),
        breaks: Vec::new(),
    };
    let start = p.pos;
    let mut first = true;
    let mut seen: Vec<&str> = Vec::new();
    loop {
        if first {
            p.eat(Punct::Slash);
        } else if !p.eat(Punct::Slash) {
            break;
        }
        if p.peek().is_none() {
            break;
        }
        let at = p.pos;
        let mut once = |name: &'static str, p: &Parser| -> PResult<()> {
            if seen.contains(&name) {
                return Err(SyntaxError::new(
                    p.span_from(at),
                    format!("{name} can be given only once"),
                ));
            }
            seen.push(name);
            Ok(())
        };
        if p.at_keyword("FORMAT") {
            if !first {
                return order(p, "FORMAT must come first");
            }
            p.pos += 1;
            once("FORMAT", p)?;
            report.format = format(p)?;
        } else if p.at_keyword("OUTFILE") {
            if !report.breaks.is_empty() {
                return order(p, "OUTFILE must come before BREAK");
            }
            p.pos += 1;
            p.eat(Punct::Eq);
            report.outfile = Some(p.file("a file")?);
        } else if p.at_keyword("STRING") {
            if !report.columns.is_empty() {
                return order(p, "STRING must come before VARIABLES");
            }
            p.pos += 1;
            once("STRING", p)?;
            report.strings = strings(p)?;
        } else if p.at_keyword("VARIABLES") || p.eat_word("VARS") {
            if p.at_keyword("VARIABLES") {
                p.pos += 1;
            }
            once("VARIABLES", p)?;
            report.columns = variables(p)?;
        } else if p.at_keyword("MISSING") {
            if report.columns.is_empty() || !report.breaks.is_empty() {
                return order(
                    p,
                    "MISSING must come after VARIABLES and before the first BREAK",
                );
            }
            p.pos += 1;
            once("MISSING", p)?;
            report.missing = missing(p)?;
        } else if p.at_keyword("TITLE") || p.at_keyword("FOOTNOTE") {
            let footnote = p.at_keyword("FOOTNOTE");
            p.pos += 1;
            let lines = titles(p)?;
            if footnote {
                report.footnotes = lines;
            } else {
                report.titles = lines;
            }
        } else if p.at_keyword("BREAK") {
            if report.columns.is_empty() {
                return order(p, "BREAK must come after VARIABLES");
            }
            p.pos += 1;
            let b = break_command(p, report.breaks.is_empty())?;
            report.breaks.push(b);
        } else if p.at_keyword("SUMMARY") {
            let Some(last) = report.breaks.last_mut() else {
                return order(p, "SUMMARY must follow a BREAK");
            };
            p.pos += 1;
            let s = summary(p)?;
            last.summaries.push(s);
        } else {
            return Err(p.subcommand_error("REPORT", &[]));
        }
        first = false;
    }
    p.finish(&["'/'"])?;
    if report.columns.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "REPORT needs VARIABLES",
        ));
    }
    let summaries = report.breaks.iter().any(|b| !b.summaries.is_empty());
    if report.format.list.is_none() && !summaries {
        return Err(SyntaxError::new(
            p.span_from(start),
            "REPORT without FORMAT=LIST needs BREAK and SUMMARY",
        ));
    }
    // PREVIOUS(n) names an earlier BREAK
    for (i, b) in report.breaks.iter().enumerate() {
        for s in &b.summaries {
            if let ReportSummary::Previous(n) = s {
                let target = n.map_or(i, |n| n as usize);
                if target == 0 || target > i {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!(
                            "SUMMARY=PREVIOUS on BREAK {} must name an earlier BREAK",
                            i + 1
                        ),
                    ));
                }
            }
        }
    }
    Ok(Command::Report(Box::new(report)))
}
