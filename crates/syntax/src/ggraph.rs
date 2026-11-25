//! Parser of GGRAPH and of the chart program between BEGIN GPL and END GPL.
//!
//! Rules taken from the manual:
//! - Subcommands and keywords are written in full and come in any order; GRAPHDATASET and
//!   GRAPHSPEC repeat. The equals signs, brackets and parentheses are required.
//! - GRAPHDATASET: NAME (required), DATASET, VARIABLES, DATAFILTER, TRANSFORM, MISSING,
//!   REPORTMISSING and CASELIMIT. The same NAME given twice: the last one counts.
//! - VARIABLES: variables (ALL and TO allowed) and functions in any order. A variable may carry
//!   [NAME="new" LEVEL=SCALE|NOMINAL|ORDINAL]; a function [NAME="new"]. A function's default
//!   name is its name in capitals followed by its arguments, joined by underscores.
//! - A function among the variables makes the data a summary by the other variables;
//!   CASEVALUE keeps one row per case and does not mix with the summary functions.
//! - GRAPHSPEC SOURCE=INLINE takes its chart program from the BEGIN GPL block that follows.
//!
//! The chart program is read into statements of a kind (SOURCE, DATA, ELEMENT and so on) with
//! one expression each; what the statements mean is up to the engine. A statement that cannot
//! be read is kept without its expression, so the engine can name it in a warning.

use crate::ast::{Command, VarItem};
use crate::error::{Span, SyntaxError};
use crate::graph::GraphFunction as F;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

#[derive(Debug, Clone, PartialEq)]
pub struct Ggraph {
    pub datasets: Vec<GraphDataset>,
    /// One per GRAPHSPEC, each an inline chart program, in order.
    pub specs: Vec<GraphSpec>,
    /// Keywords and subcommands read but not applied, for a warning.
    pub skipped: Vec<String>,
    /// The programs of the BEGIN GPL blocks after the command, one per spec; the engine adds
    /// them as the blocks arrive.
    pub programs: Vec<GplProgram>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDataset {
    pub name: String,
    pub items: Vec<GraphItem>,
    /// TRANSFORM=VARSTOCASES: the names of the summary and the index columns.
    pub to_cases: Option<(String, String)>,
    pub listwise: bool,
    pub report_missing: bool,
    pub case_limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphItem {
    /// Variables; `name` and `scale` (LEVEL=SCALE: Some(true), NOMINAL or ORDINAL: Some(false))
    /// come from the qualifier.
    Variables {
        item: VarItem,
        name: Option<String>,
        scale: Option<bool>,
    },
    Function {
        stat: GraphStat,
        variable: Option<String>,
        /// The name the chart program uses for its column.
        name: String,
    },
}

/// A function of VARIABLES.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphStat {
    /// One row per case with the variable's value.
    CaseValue,
    /// COUNT and the summary functions that GRAPH shares.
    Graph(F),
    Nle(f64),
    Ple(f64),
    Neq(f64),
    Peq(f64),
    Nge(f64),
    Pge(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphSpec {
    /// LABEL: the name of the chart in the outline.
    pub label: Option<String>,
}

/// The statements between BEGIN GPL and END GPL.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GplProgram {
    pub statements: Vec<GplStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GplStatement {
    /// In capitals: SOURCE, DATA, ELEMENT and so on.
    pub kind: String,
    pub line: u32,
    /// None when the statement could not be read.
    pub body: Option<GplExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GplExpr {
    /// A function with its arguments; the name in lower case, as `unit.category`.
    Call(String, Vec<GplExpr>),
    /// A name without parentheses: a variable of the program or a constant like `shape.square`.
    Name(String),
    Str(String),
    Number(f64),
    /// `name = expression`.
    Assign(String, Box<GplExpr>),
    /// The graph algebra: `*` crosses, `+` blends, `/` nests.
    Op(char, Box<GplExpr>, Box<GplExpr>),
}

impl GplExpr {
    /// The name of a call or a plain name.
    pub fn head(&self) -> &str {
        match self {
            GplExpr::Call(name, _) | GplExpr::Name(name) | GplExpr::Assign(name, _) => name,
            GplExpr::Str(text) => text,
            _ => "",
        }
    }
}

/// (name, takes a variable, how many numbers follow)
const FUNCTIONS: [(&str, bool, usize); 26] = [
    ("COUNT", false, 0),
    ("CASEVALUE", true, 0),
    ("MINIMUM", true, 0),
    ("MAXIMUM", true, 0),
    ("VALIDN", true, 0),
    ("SUM", true, 0),
    ("MEAN", true, 0),
    ("STDDEV", true, 0),
    ("VARIANCE", true, 0),
    ("MEDIAN", true, 0),
    ("GMEDIAN", true, 0),
    ("MODE", true, 0),
    ("PTILE", true, 1),
    ("GPTILE", true, 1),
    ("PLT", true, 1),
    ("PGT", true, 1),
    ("NLT", true, 1),
    ("NGT", true, 1),
    ("NLE", true, 1),
    ("PLE", true, 1),
    ("NEQ", true, 1),
    ("PEQ", true, 1),
    ("NGE", true, 1),
    ("PGE", true, 1),
    ("PIN", true, 2),
    ("NIN", true, 2),
];

/// Functions of the manual this version does not compute.
const NOT_YET: [&str; 6] = [
    "RESPONSES",
    "COUNTCI",
    "MEDIANCI",
    "MEANCI",
    "MEANSD",
    "MEANSE",
];

fn word<'a>(p: &Parser<'a>) -> Option<&'a str> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    }
}

/// `KEYWORD=` written in full.
fn keyword(p: &mut Parser, name: &str) -> PResult<bool> {
    if !p.eat_word(name) {
        return Ok(false);
    }
    p.expect(Punct::Eq)?;
    Ok(true)
}

fn not_yet(span: Span, what: &str) -> SyntaxError {
    SyntaxError::new(span, format!("{what} of GGRAPH is not supported yet"))
}

/// Skips one value: a word, string or number with any bracketed or parenthesized groups
/// right after it.
fn skip_value(p: &mut Parser) -> PResult<()> {
    if !p.at(Punct::LBracket) && !p.at(Punct::LParen) {
        if p.peek().is_none() || p.at(Punct::Slash) {
            return Err(p.unexpected(&["a value"]));
        }
        p.pos += 1;
    }
    while p.at(Punct::LBracket) || p.at(Punct::LParen) {
        let mut depth = 0;
        loop {
            match p.peek() {
                None => return Err(p.unexpected(&["')'", "']'"])),
                Some(TokenKind::Punct(Punct::LBracket | Punct::LParen)) => depth += 1,
                Some(TokenKind::Punct(Punct::RBracket | Punct::RParen)) => depth -= 1,
                _ => {}
            }
            p.pos += 1;
            if depth == 0 {
                break;
            }
        }
    }
    Ok(())
}

/// A number as it appears in a default function name: 90, 2.5.
fn plain(x: f64) -> String {
    format!("{x}")
}

/// `[NAME="x" LEVEL=SCALE]` after a variable, or `[NAME="x"]` after a function.
fn qualifier(p: &mut Parser, level: bool) -> PResult<(Option<String>, Option<bool>)> {
    let (mut name, mut scale) = (None, None);
    if !p.eat(Punct::LBracket) {
        return Ok((name, scale));
    }
    while !p.eat(Punct::RBracket) {
        if keyword(p, "NAME")? {
            name = Some(p.string("a name in quotes")?);
        } else if level && keyword(p, "LEVEL")? {
            let levels = ["SCALE", "NOMINAL", "ORDINAL"];
            let Some(k) = levels.iter().position(|l| p.eat_word(l)) else {
                return Err(p.unexpected(&levels));
            };
            scale = Some(k == 0);
        } else {
            let mut expected = vec!["NAME", "']'"];
            if level {
                expected.insert(1, "LEVEL");
            }
            return Err(p.unexpected(&expected));
        }
    }
    Ok((name, scale))
}

/// The items of VARIABLES, up to the next keyword of GRAPHDATASET or the next subcommand.
fn items(p: &mut Parser) -> PResult<Vec<GraphItem>> {
    const KEYWORDS: [&str; 7] = [
        "NAME",
        "DATASET",
        "DATAFILTER",
        "TRANSFORM",
        "MISSING",
        "REPORTMISSING",
        "CASELIMIT",
    ];
    let mut items = Vec::new();
    while let Some(w) = word(p) {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
            && KEYWORDS.iter().any(|k| w.eq_ignore_ascii_case(k))
        {
            break;
        }
        let start = p.span();
        if p.peek_at(1) != Some(&TokenKind::Punct(Punct::LParen)) {
            if w.starts_with('$') {
                let what = "a multiple response set in VARIABLES";
                return Err(not_yet(start, what));
            }
            let item = match p.eat_word("ALL") {
                true => VarItem::All,
                false => {
                    let first = p.name("a variable")?;
                    match p.eat_word("TO") {
                        true => VarItem::Range(first, p.name("a variable")?),
                        false => VarItem::Name(first),
                    }
                }
            };
            let (name, scale) = qualifier(p, true)?;
            if name.is_some() && !matches!(item, VarItem::Name(_)) {
                let message = "NAME renames a single variable, not a list";
                return Err(SyntaxError::new(start, message));
            }
            items.push(GraphItem::Variables { item, name, scale });
            continue;
        }
        let upper = w.to_uppercase();
        if NOT_YET.contains(&upper.as_str()) {
            return Err(not_yet(start, &format!("the function {upper}")));
        }
        let Some(&(function, takes_variable, numbers)) = FUNCTIONS.iter().find(|f| f.0 == upper)
        else {
            let message = format!("unknown function {upper} in VARIABLES");
            return Err(SyntaxError::new(start, message));
        };
        let first = p.pos;
        p.pos += 2;
        let mut variable = None;
        let mut args = Vec::new();
        if takes_variable {
            variable = Some(p.name("a variable")?);
        }
        for _ in 0..numbers {
            p.eat(Punct::Comma);
            let Some(x) = p.number() else {
                return Err(p.unexpected(&["a number"]));
            };
            args.push(x);
        }
        p.expect(Punct::RParen)?;
        let span = p.span_from(first);
        let x = args.first().copied().unwrap_or(0.0);
        let stat = match function {
            "COUNT" => GraphStat::Graph(F::Count),
            "CASEVALUE" => GraphStat::CaseValue,
            "MINIMUM" => GraphStat::Graph(F::Minimum),
            "MAXIMUM" => GraphStat::Graph(F::Maximum),
            "VALIDN" => GraphStat::Graph(F::N),
            "SUM" => GraphStat::Graph(F::Sum),
            "MEAN" => GraphStat::Graph(F::Mean),
            "STDDEV" => GraphStat::Graph(F::StdDev),
            "VARIANCE" => GraphStat::Graph(F::Variance),
            "MEDIAN" => GraphStat::Graph(F::Median),
            "GMEDIAN" => GraphStat::Graph(F::Gmedian),
            "MODE" => GraphStat::Graph(F::Mode),
            "PTILE" if x <= 0.0 || x >= 100.0 => {
                let message = "the percentile of PTILE lies above 0 and below 100";
                return Err(SyntaxError::new(span, message));
            }
            "PTILE" => GraphStat::Graph(F::Ptile(x)),
            "GPTILE" if x <= 0.0 || x >= 100.0 => {
                let message = "the percentile of GPTILE lies above 0 and below 100";
                return Err(SyntaxError::new(span, message));
            }
            "GPTILE" => GraphStat::Graph(F::Gptile(x)),
            "PLT" => GraphStat::Graph(F::Plt(x)),
            "PGT" => GraphStat::Graph(F::Pgt(x)),
            "NLT" => GraphStat::Graph(F::Nlt(x)),
            "NGT" => GraphStat::Graph(F::Ngt(x)),
            "NLE" => GraphStat::Nle(x),
            "PLE" => GraphStat::Ple(x),
            "NEQ" => GraphStat::Neq(x),
            "PEQ" => GraphStat::Peq(x),
            "NGE" => GraphStat::Nge(x),
            "PGE" => GraphStat::Pge(x),
            _ if args[0] > args[1] => {
                let message = format!("the low end of {function} cannot exceed its high end");
                return Err(SyntaxError::new(span, message));
            }
            "PIN" => GraphStat::Graph(F::Pin(args[0], args[1])),
            _ => GraphStat::Graph(F::Nin(args[0], args[1])),
        };
        let default = std::iter::once(function.to_string())
            .chain(variable.clone())
            .chain(args.iter().map(|&a| plain(a)))
            .collect::<Vec<_>>()
            .join("_");
        let (name, _) = qualifier(p, false)?;
        items.push(GraphItem::Function {
            stat,
            variable,
            name: name.unwrap_or(default),
        });
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable", "a function"]));
    }
    let casevalue = items.iter().any(|i| {
        matches!(
            i,
            GraphItem::Function {
                stat: GraphStat::CaseValue,
                ..
            }
        )
    });
    let summary = items
        .iter()
        .any(|i| matches!(i, GraphItem::Function { stat, .. } if *stat != GraphStat::CaseValue));
    if casevalue && summary {
        let message = "CASEVALUE does not mix with the summary functions in VARIABLES";
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(items)
}

fn graph_dataset(p: &mut Parser) -> PResult<GraphDataset> {
    let mut set = GraphDataset {
        name: String::new(),
        items: Vec::new(),
        to_cases: None,
        listwise: true,
        report_missing: false,
        case_limit: 1_000_000,
    };
    let start = p.span();
    while !crate::procedures::at_end(p) {
        if keyword(p, "NAME")? {
            set.name = p.string("the name of the graph dataset in quotes")?;
        } else if keyword(p, "DATASET")? {
            if !p.eat(Punct::Star) {
                return Err(not_yet(p.span(), "DATASET with a name"));
            }
        } else if keyword(p, "VARIABLES")? {
            set.items = items(p)?;
        } else if p.eat_word("DATAFILTER") {
            return Err(not_yet(p.span(), "DATAFILTER"));
        } else if keyword(p, "TRANSFORM")? {
            if p.eat_word("NO") {
                set.to_cases = None;
                continue;
            }
            if !p.eat_word("VARSTOCASES") {
                return Err(p.unexpected(&["NO", "VARSTOCASES"]));
            }
            let (mut summary, mut index) = ("#SUMMARY".to_string(), "#INDEX".to_string());
            p.expect(Punct::LParen)?;
            while !p.eat(Punct::RParen) {
                if keyword(p, "SUMMARY")? {
                    summary = p.string("a name in quotes")?;
                } else if keyword(p, "INDEX")? {
                    index = p.string("a name in quotes")?;
                } else if p.eat_word("HIGH") || p.eat_word("LOW") {
                    p.expect(Punct::Eq)?;
                    p.string("a name in quotes")?;
                } else {
                    return Err(p.unexpected(&["SUMMARY", "INDEX", "')'"]));
                }
            }
            set.to_cases = Some((summary, index));
        } else if keyword(p, "MISSING")? {
            let options = ["LISTWISE", "VARIABLEWISE"];
            let Some(k) = options.iter().position(|o| p.eat_word(o)) else {
                return Err(p.unexpected(&options));
            };
            set.listwise = k == 0;
        } else if keyword(p, "REPORTMISSING")? {
            let options = ["NO", "YES"];
            let Some(k) = options.iter().position(|o| p.eat_word(o)) else {
                return Err(p.unexpected(&options));
            };
            set.report_missing = k == 1;
        } else if keyword(p, "CASELIMIT")? {
            match p.number() {
                Some(n) if n >= 1.0 && n.fract() == 0.0 => set.case_limit = n as usize,
                _ => return Err(p.unexpected(&["a whole number of cases, 1 or more"])),
            }
        } else {
            let known = [
                "NAME",
                "DATASET",
                "VARIABLES",
                "DATAFILTER",
                "TRANSFORM",
                "MISSING",
                "REPORTMISSING",
                "CASELIMIT",
            ];
            return Err(p.unexpected(&known));
        }
    }
    if set.name.is_empty() {
        return Err(SyntaxError::new(start, "GRAPHDATASET needs NAME=\"name\""));
    }
    if set.items.is_empty() {
        // without VARIABLES the chart has no data to show
        return Err(SyntaxError::new(start, "GRAPHDATASET needs VARIABLES"));
    }
    if set.to_cases.is_some()
        && !set
            .items
            .iter()
            .any(|i| matches!(i, GraphItem::Function { stat, .. } if *stat != GraphStat::CaseValue))
    {
        let message = "TRANSFORM=VARSTOCASES turns summary functions into rows; give some";
        return Err(SyntaxError::new(start, message));
    }
    Ok(set)
}

fn graph_spec(p: &mut Parser, skipped: &mut Vec<String>) -> PResult<GraphSpec> {
    let mut spec = GraphSpec { label: None };
    let mut inline = false;
    let start = p.span();
    while !crate::procedures::at_end(p) {
        if keyword(p, "SOURCE")? {
            let at = p.span();
            if p.eat_word("INLINE") {
                inline = true;
                continue;
            }
            let Some(w) = word(p) else {
                return Err(p.unexpected(&["INLINE"]));
            };
            return Err(not_yet(at, &format!("SOURCE={}", w.to_uppercase())));
        } else if keyword(p, "LABEL")? {
            spec.label = Some(p.string("a label in quotes")?);
        } else if let Some(w) = word(p).map(str::to_uppercase).filter(|w| {
            [
                "EDITABLE",
                "DEFAULTTEMPLATE",
                "TEMPLATE",
                "VIZSTYLESHEET",
                "VIZMAP",
            ]
            .contains(&w.as_str())
        }) {
            p.pos += 1;
            p.expect(Punct::Eq)?;
            skip_value(p)?;
            skipped.push(w);
        } else {
            let known = ["SOURCE", "LABEL", "EDITABLE", "DEFAULTTEMPLATE", "TEMPLATE"];
            return Err(p.unexpected(&known));
        }
    }
    if !inline {
        return Err(SyntaxError::new(start, "GRAPHSPEC needs SOURCE=INLINE"));
    }
    Ok(spec)
}

pub(crate) fn ggraph(p: &mut Parser) -> PResult<Command> {
    let mut command = Ggraph {
        datasets: Vec::new(),
        specs: Vec::new(),
        skipped: Vec::new(),
        programs: Vec::new(),
    };
    p.eat(Punct::Slash);
    loop {
        let Some(w) = word(p).map(str::to_uppercase) else {
            return Err(p.unexpected(&["GRAPHDATASET", "GRAPHSPEC"]));
        };
        match w.as_str() {
            "GRAPHDATASET" => {
                p.pos += 1;
                let set = graph_dataset(p)?;
                // the same name again: the last one counts
                command
                    .datasets
                    .retain(|d| !d.name.eq_ignore_ascii_case(&set.name));
                command.datasets.push(set);
            }
            "GRAPHSPEC" => {
                p.pos += 1;
                let spec = graph_spec(p, &mut command.skipped)?;
                command.specs.push(spec);
            }
            "COLORCYCLE" | "FRAME" | "GRIDLINES" | "FITLINES" => {
                p.pos += 1;
                while !crate::procedures::at_end(p) {
                    p.pos += 1;
                }
                command.skipped.push(format!("subcommand {w}"));
            }
            _ => return Err(p.subcommand_error("GGRAPH", &[])),
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.specs.is_empty() {
        let message = "GGRAPH needs GRAPHSPEC SOURCE=INLINE with its BEGIN GPL block";
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(Command::Ggraph(Box::new(command)))
}

/// BEGIN GPL: the splitter hands over each line of the block as one string token.
pub(crate) fn begin_gpl(p: &mut Parser) -> PResult<Command> {
    let mut lines = Vec::new();
    while let Some(TokenKind::Str(text)) = p.peek() {
        lines.push((p.span().line, text.clone()));
        p.pos += 1;
    }
    p.finish(&[])?;
    Ok(Command::BeginGpl(Box::new(read_gpl(&lines))))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    Str(String),
    Number(f64),
    Punct(char),
}

fn tokens(lines: &[(u32, String)]) -> Vec<(u32, Tok)> {
    let mut out = Vec::new();
    for (line, text) in lines {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let name_char = |c: char| c.is_alphanumeric() || "_.$#@".contains(c);
        while i < chars.len() {
            let c = chars[i];
            let start = i;
            i += 1;
            let tok = if c.is_whitespace() {
                continue;
            } else if c == '"' {
                let mut s = String::new();
                while i < chars.len() {
                    if chars[i] == '"' {
                        // a doubled quote stands for one
                        if chars.get(i + 1) == Some(&'"') {
                            s.push('"');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                Tok::Str(s)
            } else if c.is_ascii_digit()
                || (c == '.' && chars.get(i).is_some_and(char::is_ascii_digit))
            {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                if i < chars.len() && name_char(chars[i]) {
                    // a size such as 400px
                    while i < chars.len() && name_char(chars[i]) {
                        i += 1;
                    }
                    Tok::Name(chars[start..i].iter().collect())
                } else {
                    let text: String = chars[start..i].iter().collect();
                    match text.parse() {
                        Ok(x) => Tok::Number(x),
                        Err(_) => Tok::Name(text),
                    }
                }
            } else if name_char(c) {
                while i < chars.len() && name_char(chars[i]) {
                    i += 1;
                }
                Tok::Name(chars[start..i].iter().collect())
            } else {
                Tok::Punct(c)
            };
            out.push((*line, tok));
        }
    }
    out
}

struct Gpl {
    toks: Vec<(u32, Tok)>,
    pos: usize,
    depth: u32,
}

impl Gpl {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.1)
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(&Tok::Punct(c));
        self.pos += usize::from(found);
        found
    }

    /// A name followed by a colon starts a statement.
    fn at_statement(&self) -> bool {
        matches!(self.peek(), Some(Tok::Name(_)))
            && self.toks.get(self.pos + 1).map(|t| &t.1) == Some(&Tok::Punct(':'))
    }

    fn expr(&mut self) -> Option<GplExpr> {
        if let (Some(Tok::Name(name)), Some((_, Tok::Punct('=')))) =
            (self.peek().cloned(), self.toks.get(self.pos + 1))
        {
            self.pos += 2;
            return Some(GplExpr::Assign(name, Box::new(self.binary(0)?)));
        }
        self.binary(0)
    }

    /// Blend (+) binds loosest, then cross (*), then nest (/).
    fn binary(&mut self, level: usize) -> Option<GplExpr> {
        const OPS: [char; 3] = ['+', '*', '/'];
        if level == OPS.len() {
            return self.primary();
        }
        let mut left = self.binary(level + 1)?;
        while self.eat(OPS[level]) {
            let right = self.binary(level + 1)?;
            left = GplExpr::Op(OPS[level], Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn primary(&mut self) -> Option<GplExpr> {
        // runaway nesting would overflow the stack
        if self.depth > 64 {
            return None;
        }
        let tok = self.peek()?.clone();
        self.pos += 1;
        match tok {
            Tok::Str(s) => Some(GplExpr::Str(s)),
            Tok::Number(x) => Some(GplExpr::Number(x)),
            Tok::Punct('-') => match self.primary()? {
                GplExpr::Number(x) => Some(GplExpr::Number(-x)),
                _ => None,
            },
            Tok::Punct('(') => {
                self.depth += 1;
                let inner = self.expr();
                self.depth -= 1;
                self.eat(')').then_some(inner?)
            }
            Tok::Name(name) if self.eat('(') => {
                let mut args = Vec::new();
                self.depth += 1;
                while !self.eat(')') {
                    args.push(self.expr()?);
                    if !self.eat(',') && self.peek() != Some(&Tok::Punct(')')) {
                        return None;
                    }
                }
                self.depth -= 1;
                Some(GplExpr::Call(name.to_lowercase(), args))
            }
            Tok::Name(name) => Some(GplExpr::Name(name)),
            Tok::Punct(_) => None,
        }
    }
}

/// The statements of a chart program; a statement that cannot be read keeps no body, and
/// anything before the first statement is a statement of kind "?".
pub fn read_gpl(lines: &[(u32, String)]) -> GplProgram {
    let mut gpl = Gpl {
        toks: tokens(lines),
        pos: 0,
        depth: 0,
    };
    let mut statements = Vec::new();
    while gpl.pos < gpl.toks.len() {
        let line = gpl.toks[gpl.pos].0;
        let (kind, body) = match gpl.at_statement() {
            true => {
                let kind = match gpl.peek() {
                    Some(Tok::Name(name)) => name.to_uppercase(),
                    _ => unreachable!("at a statement"),
                };
                gpl.pos += 2;
                let body = gpl.expr();
                (
                    kind,
                    body.filter(|_| gpl.pos >= gpl.toks.len() || gpl.at_statement()),
                )
            }
            false => ("?".to_string(), None),
        };
        // a statement that was not read to its end: skip to the next one
        while gpl.pos < gpl.toks.len() && !gpl.at_statement() {
            gpl.pos += 1;
        }
        statements.push(GplStatement { kind, line, body });
    }
    GplProgram { statements }
}
