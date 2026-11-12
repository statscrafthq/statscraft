//! Parser of OUTPUT MODIFY: changes to the items of an output document.
//!
//! Rules from the description of the command:
//! - NAME=* (the designated document, the default), ALL or a document name comes first, then
//!   blocks of subcommands. Every block starts with /SELECT; /REPORT may stand only before the
//!   first block. Inside a block the other subcommands come in any order.
//! - Lists go in square brackets. Keywords may be shortened to three or more letters, as long
//!   as the short form names one keyword only.
//! - /SELECT names item kinds, or ALL with EXCEPT (list) for all kinds but those.
//! - /IF narrows the items: COMMANDS (a name, a name with (n) or (LAST), or LAST), SUBTYPES,
//!   LABELS (EXACT, BEGINS, ENDS, CONTAINS), INSTANCES (n or LAST) and VISIBLEONLY. An item
//!   must meet every condition given.
//! - /INDEXING numbers the items with digits, letters or roman numerals from INDEXSTART.
//! - /TABLECELLS SELECTCONDITION holds ALL, TOP(n), BOTTOM(n) or comparisons such as "x<0.05"
//!   on the stand-in x, with ABS(x) and SYSMIS.
//!
//! Choices:
//! - Keywords that change how something looks (fonts, colors, styles, sizes, table looks,
//!   layers, pivoting, templates) are read and kept as `not_applied`, so the engine warns that
//!   they change nothing. PARENT and CHILD conditions are refused: without them the change
//!   would reach more cells than asked.

use crate::ast::{Command, Value};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::output_docs::OutputCommand;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

#[derive(Debug, Clone, PartialEq)]
pub struct OutputModify {
    pub target: ModifyTarget,
    /// /REPORT PRINTREPORT=YES: a table of what changed.
    pub report: bool,
    pub blocks: Vec<ModifyBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModifyTarget {
    Designated,
    All,
    Named(String),
}

/// The kinds of output items /SELECT names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Charts,
    Headings,
    Logs,
    Models,
    Tables,
    Texts,
    Trees,
    Warnings,
    OutlineHeaders,
    Unknown,
    Other,
    PageTitle,
    Notes,
}

const KINDS: [(&str, ItemKind); 13] = [
    ("CHARTS", ItemKind::Charts),
    ("HEADINGS", ItemKind::Headings),
    ("LOGS", ItemKind::Logs),
    ("MODELS", ItemKind::Models),
    ("TABLES", ItemKind::Tables),
    ("TEXTS", ItemKind::Texts),
    ("TREES", ItemKind::Trees),
    ("WARNINGS", ItemKind::Warnings),
    ("OUTLINEHEADERS", ItemKind::OutlineHeaders),
    ("UNKNOWN", ItemKind::Unknown),
    ("OTHER", ItemKind::Other),
    ("PAGETITLE", ItemKind::PageTitle),
    ("NOTES", ItemKind::Notes),
];

/// One block: the items it picks and what it does to them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModifyBlock {
    pub kinds: Vec<ItemKind>,
    /// Empty: any command.
    pub commands: Vec<CommandPick>,
    pub subtypes: Vec<String>,
    pub labels: Vec<(LabelTest, String)>,
    pub instances: Vec<Instance>,
    pub visible_only: bool,
    pub delete: bool,
    pub indexing: Option<Indexing>,
    /// VISIBLE=YES or NO; None is ASIS.
    pub visible: Option<bool>,
    pub outline_label: Option<String>,
    pub table_title: Option<String>,
    pub transpose: bool,
    pub sort: Option<SortColumn>,
    pub descending: bool,
    pub cells: Vec<TableCells>,
    /// /TEXTS TEXT.
    pub text: Option<String>,
    /// Keywords read that change nothing, each with the reason, such as ("/TABLECELLS FONT",
    /// "Statscraft tables have no fonts or colors").
    pub not_applied: Vec<(String, &'static str)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandPick {
    /// The last command in the document.
    Last,
    Named {
        name: String,
        instance: Option<Instance>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instance {
    /// From 1.
    Nth(usize),
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelTest {
    Exact,
    Begins,
    Ends,
    Contains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indexing {
    pub style: IndexStyle,
    /// The first index as a number from 1: 'c' is 3, 'iv' is 4.
    pub start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStyle {
    Number,
    LetterLow,
    LetterCap,
    RomanLow,
    RomanCap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortColumn {
    Label(String),
    /// From 1, counting the row label columns.
    Position(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCells {
    pub select: Vec<CellArea>,
    pub dimension: Dimension,
    /// Empty: all cells of the area.
    pub conditions: Vec<CellCondition>,
    pub apply_to: ApplyTo,
    pub format: Option<String>,
    pub replace: Option<Value>,
    pub hide: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellArea {
    /// Rows or columns with one of these labels.
    Labels(Vec<String>),
    /// The nth data row or column, from 1.
    Position(usize),
    Body,
    Headers,
    Footnotes,
    Title,
    Caption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Both,
    Columns,
    Rows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyTo {
    Cell,
    Column,
    Row,
    ColumnData,
    RowData,
    ColumnHeader,
    RowHeader,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellCondition {
    Compare {
        abs: bool,
        op: CompareOp,
        value: CompareValue,
    },
    Top(usize),
    Bottom(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompareValue {
    Number(f64),
    Text(String),
    Sysmis,
}

/// The labels the condition keywords of /TABLECELLS SELECT stand for.
const LABEL_KEYWORDS: [(&str, &[&str]); 12] = [
    ("COUNT", &["Frequency", "Count", "N"]),
    ("MEAN", &["Mean"]),
    ("MEDIAN", &["Median"]),
    ("RESIDUAL", &["Resid", "Residual", "Std. Residual"]),
    ("PERCENT", &["Percent"]),
    (
        "SIGNIFICANCE",
        &[
            "Approx Sig.",
            "Asymp. Sig (2-sided)",
            "Asymp. Sig. (2-sided)",
            "Exact Sig.",
            "Exact Sig. (1-sided)",
            "Exact Sig. (2-sided)",
            "Sig.",
            "Sig. (1-tailed)",
            "Sig. (2-tailed)",
        ],
    ),
    ("VALID", &["Valid"]),
    ("TOTAL", &["Total"]),
    ("MISSING", &["Missing"]),
    ("CUMULATIVEPERCENT", &["Cumulative Percent"]),
    ("VALIDPERCENT", &["Valid Percent"]),
    (
        "CORRELATION",
        &[
            "Correlation Coefficient",
            "Correlations",
            "Pearson Correlation",
        ],
    ),
];

const NO_STYLES: &str = "Statscraft tables have no fonts, colors or styles yet";
const NO_SIZES: &str = "Statscraft sizes tables and charts to their contents and the window";
const NO_LOOKS: &str =
    "Statscraft tables have one look, one layer and the layout their command gave";
const NO_TEMPLATES: &str = "Statscraft charts have no templates or style sheets";

/// The keyword at the parser among `list`: written in full, or shortened to three or more
/// letters that start no other keyword of the list. None when the word is none of them.
fn keyword_in(p: &Parser, list: &[&'static str]) -> PResult<Option<&'static str>> {
    let Some(TokenKind::Word(word)) = p.peek() else {
        return Ok(None);
    };
    if let Some(exact) = list.iter().find(|k| k.eq_ignore_ascii_case(word)) {
        return Ok(Some(exact));
    }
    let found: Vec<&'static str> = list
        .iter()
        .copied()
        .filter(|k| matches_keyword(word, k))
        .collect();
    match found[..] {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        _ => Err(SyntaxError::new(
            p.span(),
            format!(
                "{} is short for more than one keyword: {}",
                word.to_uppercase(),
                found.join(", ")
            ),
        )),
    }
}

/// Reads the keyword among `list` and moves past it, or fails listing them.
fn expect_keyword(p: &mut Parser, list: &[&'static str]) -> PResult<&'static str> {
    match keyword_in(p, list)? {
        Some(k) => {
            p.pos += 1;
            Ok(k)
        }
        None => Err(p.unexpected(list)),
    }
}

/// `keyword = value` pairs up to the next subcommand, each keyword at most once.
fn settings(
    p: &mut Parser,
    place: &str,
    known: &[&'static str],
    mut read: impl FnMut(&mut Parser, &'static str) -> PResult<()>,
) -> PResult<()> {
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(p.unexpected(known));
    }
    while !at_end(p) {
        let at = p.span();
        let keyword = expect_keyword(p, known)?;
        if seen.contains(&keyword) {
            return Err(SyntaxError::new(
                at,
                format!("{keyword} is given twice on {place}"),
            ));
        }
        seen.push(keyword);
        p.expect(Punct::Eq)?;
        read(p, keyword)?;
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    Ok(expect_keyword(p, &["YES", "NO"])? == "YES")
}

/// A whole number of at least `min`.
fn whole(p: &mut Parser, min: f64, what: &str) -> PResult<usize> {
    match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= min && *n <= 1e9 => {
            let n = *n as usize;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

/// `[ item item ... ]`, commas between items optional; at least one item.
fn bracket_list<T>(
    p: &mut Parser,
    mut item: impl FnMut(&mut Parser) -> PResult<T>,
) -> PResult<Vec<T>> {
    p.expect(Punct::LBracket)?;
    let mut items = Vec::new();
    while !p.eat(Punct::RBracket) {
        if p.peek().is_none() {
            return Err(p.unexpected(&["']'"]));
        }
        items.push(item(p)?);
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(p.pos - 1),
            "the list is empty",
        ));
    }
    Ok(items)
}

/// Passes over a value that changes nothing: a word, number or string, a word with
/// parentheses such as RGB(10, 20, 30), or a list in square brackets.
fn skip_value(p: &mut Parser) -> PResult<()> {
    let close = match p.peek() {
        Some(TokenKind::Word(_)) => {
            p.pos += 1;
            if !p.eat(Punct::LParen) {
                // STYLE takes several words
                while matches!(p.peek(), Some(TokenKind::Word(w)) if !is_keyword_like(w)) {
                    p.pos += 1;
                }
                return Ok(());
            }
            Punct::RParen
        }
        Some(TokenKind::Number(_) | TokenKind::Str(_)) => {
            p.pos += 1;
            return Ok(());
        }
        Some(TokenKind::Punct(Punct::LBracket)) => {
            p.pos += 1;
            Punct::RBracket
        }
        _ => return Err(p.unexpected(&["a value"])),
    };
    while !p.eat(close) {
        if p.peek().is_none() {
            return Err(p.unexpected(&[&format!("'{}'", close.as_str())]));
        }
        p.pos += 1;
    }
    Ok(())
}

/// The style words STYLE takes; another word starts the next keyword.
fn is_keyword_like(word: &str) -> bool {
    !["REGULAR", "BOLD", "ITALIC", "UNDERLINE"]
        .iter()
        .any(|s| s.eq_ignore_ascii_case(word))
}

pub(crate) fn output_modify(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut modify = OutputModify {
        target: ModifyTarget::Designated,
        report: false,
        blocks: Vec::new(),
    };
    if keyword_in(p, &["NAME"])?.is_some() {
        p.pos += 1;
        p.expect(Punct::Eq)?;
        modify.target = if p.eat(Punct::Star) {
            ModifyTarget::Designated
        } else if p.eat_word("ALL") {
            ModifyTarget::All
        } else {
            ModifyTarget::Named(p.name("a document name, * or ALL")?)
        };
    }
    const SUBCOMMANDS: [&str; 10] = [
        "REPORT",
        "SELECT",
        "IF",
        "DELETEOBJECT",
        "INDEXING",
        "OBJECTPROPERTIES",
        "TABLE",
        "TABLECELLS",
        "GRAPHS",
        "TEXTS",
    ];
    let mut seen: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let at = p.span();
        // IF has two letters: it is matched in full
        let name = if p.eat_word("IF") {
            "IF"
        } else {
            let Some(name) = keyword_in(p, &SUBCOMMANDS)? else {
                return Err(p.subcommand_error("OUTPUT MODIFY", &[]));
            };
            p.pos += 1;
            name
        };
        let twice = |what: &str| {
            SyntaxError::new(at, format!("/{what} is given twice in one /SELECT block"))
        };
        match name {
            "REPORT" => {
                if !modify.blocks.is_empty() || seen.contains(&"REPORT") {
                    return Err(SyntaxError::new(
                        at,
                        "/REPORT comes once, before the first /SELECT",
                    ));
                }
                seen.push("REPORT");
                settings(p, "/REPORT", &["PRINTREPORT"], |p, _| {
                    modify.report = yes_no(p)?;
                    Ok(())
                })?;
                continue;
            }
            "SELECT" => {
                seen.retain(|s| *s == "REPORT");
                modify.blocks.push(ModifyBlock {
                    kinds: select(p)?,
                    ..ModifyBlock::default()
                });
                continue;
            }
            _ => {}
        }
        let Some(block) = modify.blocks.last_mut() else {
            return Err(SyntaxError::new(
                at,
                format!("/{name} belongs to a block, and every block starts with /SELECT"),
            ));
        };
        // TABLECELLS may come several times in a block, each with its own area
        if name != "TABLECELLS" {
            if seen.contains(&name) {
                return Err(twice(name));
            }
            seen.push(name);
        }
        match name {
            "IF" => conditions(p, block)?,
            "DELETEOBJECT" => settings(p, "/DELETEOBJECT", &["DELETE"], |p, _| {
                block.delete = yes_no(p)?;
                Ok(())
            })?,
            "INDEXING" => block.indexing = Some(indexing(p)?),
            "OBJECTPROPERTIES" => settings(
                p,
                "/OBJECTPROPERTIES",
                &["VISIBLE", "OUTLINELABEL", "SIZE"],
                |p, k| {
                    match k {
                        "VISIBLE" => {
                            block.visible = match expect_keyword(p, &["ASIS", "YES", "NO"])? {
                                "YES" => Some(true),
                                "NO" => Some(false),
                                _ => None,
                            }
                        }
                        "OUTLINELABEL" => {
                            block.outline_label = Some(p.string("a label in quotes")?)
                        }
                        _ => {
                            skip_value(p)?;
                            block
                                .not_applied
                                .push(("/OBJECTPROPERTIES SIZE".into(), NO_SIZES));
                        }
                    }
                    Ok(())
                },
            )?,
            "TABLE" => table(p, block)?,
            "TABLECELLS" => {
                let cells = table_cells(p, &mut block.not_applied)?;
                block.cells.push(cells);
            }
            "GRAPHS" => settings(p, "/GRAPHS", &["CTEMPLATE", "VIZSTYLESHEET"], |p, k| {
                skip_value(p)?;
                block
                    .not_applied
                    .push((format!("/GRAPHS {k}"), NO_TEMPLATES));
                Ok(())
            })?,
            _ => settings(
                p,
                "/TEXTS",
                &["TEXT", "FONT", "FONTSIZE", "STYLE", "TEXTCOLOR"],
                |p, k| {
                    if k == "TEXT" {
                        block.text = Some(p.string("a text in quotes")?);
                    } else {
                        skip_value(p)?;
                        block.not_applied.push((format!("/TEXTS {k}"), NO_STYLES));
                    }
                    Ok(())
                },
            )?,
        }
    }
    p.finish(&["'/'"])?;
    if modify.blocks.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "OUTPUT MODIFY needs /SELECT with the kinds of items to change",
        ));
    }
    Ok(Command::Output(OutputCommand::Modify(Box::new(modify))))
}

/// The kinds of /SELECT: a list, or ALL [EXCEPT [=] (list)].
fn select(p: &mut Parser) -> PResult<Vec<ItemKind>> {
    let names: Vec<&str> = KINDS.iter().map(|(n, _)| *n).collect();
    let kind = |p: &mut Parser| -> PResult<ItemKind> {
        let name = expect_keyword(p, &names)?;
        Ok(KINDS.iter().find(|(n, _)| *n == name).expect("listed").1)
    };
    if p.eat_word("ALL") {
        let mut kinds: Vec<ItemKind> = KINDS.iter().map(|(_, k)| *k).collect();
        if keyword_in(p, &["EXCEPT"])?.is_some() {
            p.pos += 1;
            p.eat(Punct::Eq);
            let close = if p.eat(Punct::LBracket) {
                Punct::RBracket
            } else {
                p.expect(Punct::LParen)?;
                Punct::RParen
            };
            while !p.eat(close) {
                let except = kind(p)?;
                kinds.retain(|k| *k != except);
                p.eat(Punct::Comma);
            }
        }
        return Ok(kinds);
    }
    let mut kinds = Vec::new();
    while !at_end(p) {
        kinds.push(kind(p)?);
        p.eat(Punct::Comma);
    }
    if kinds.is_empty() {
        let mut expected = vec!["ALL"];
        expected.extend(&names);
        return Err(p.unexpected(&expected));
    }
    Ok(kinds)
}

/// `name`, `name(n)` or `name(LAST)` in a COMMANDS string.
fn command_pick(text: &str) -> Option<CommandPick> {
    let text = text.trim();
    let Some(open) = text
        .strip_suffix(')')
        .and_then(|t| t.rfind('(').map(|i| (t, i)))
    else {
        return (!text.is_empty()).then(|| CommandPick::Named {
            name: text.to_string(),
            instance: None,
        });
    };
    let (body, at) = open;
    let inside = body[at + 1..].trim();
    let instance = if inside.eq_ignore_ascii_case("LAST") {
        Instance::Last
    } else {
        Instance::Nth(inside.parse().ok().filter(|n| *n >= 1)?)
    };
    let name = body[..at].trim();
    (!name.is_empty()).then(|| CommandPick::Named {
        name: name.to_string(),
        instance: Some(instance),
    })
}

fn conditions(p: &mut Parser, block: &mut ModifyBlock) -> PResult<()> {
    settings(
        p,
        "/IF",
        &["COMMANDS", "SUBTYPES", "LABELS", "INSTANCES", "VISIBLEONLY"],
        |p, k| {
            match k {
                "COMMANDS" => {
                    block.commands = bracket_list(p, |p| {
                        if p.eat_word("LAST") {
                            return Ok(CommandPick::Last);
                        }
                        let at = p.pos;
                        let text = p.string("a command name in quotes, or LAST")?;
                        command_pick(&text).ok_or_else(|| {
                            SyntaxError::new(
                                p.span_from(at),
                                format!(
                                    "'{text}' is no command; write a name, a name with (n) for \
                                     its nth run or with (LAST)"
                                ),
                            )
                        })
                    })?
                }
                "SUBTYPES" => {
                    block.subtypes = bracket_list(p, |p| p.string("a subtype in quotes"))?
                }
                "LABELS" => {
                    block.labels = bracket_list(p, |p| {
                        let test =
                            match expect_keyword(p, &["EXACT", "BEGINS", "ENDS", "CONTAINS"])? {
                                "EXACT" => LabelTest::Exact,
                                "BEGINS" => LabelTest::Begins,
                                "ENDS" => LabelTest::Ends,
                                _ => LabelTest::Contains,
                            };
                        p.expect(Punct::LParen)?;
                        let text = p.string("a label in quotes")?;
                        p.expect(Punct::RParen)?;
                        Ok((test, text))
                    })?
                }
                "INSTANCES" => {
                    block.instances = bracket_list(p, |p| {
                        if p.eat_word("LAST") {
                            Ok(Instance::Last)
                        } else {
                            whole(p, 1.0, "a positive whole number or LAST").map(Instance::Nth)
                        }
                    })?
                }
                _ => block.visible_only = yes_no(p)?,
            }
            Ok(())
        },
    )
}

/// The value of a roman numeral written in the usual way, such as XIV; None for another text.
pub fn roman_value(text: &str) -> Option<usize> {
    let digit = |c: char| match c.to_ascii_uppercase() {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let digits: Vec<usize> = text.chars().map(digit).collect::<Option<_>>()?;
    let mut value = 0;
    for (i, d) in digits.iter().enumerate() {
        match digits.get(i + 1) {
            Some(next) if next > d => value -= *d as isize,
            _ => value += *d as isize,
        }
    }
    let value = usize::try_from(value).ok().filter(|v| *v > 0)?;
    // only the usual spelling: it must read back the same
    (roman(value).eq_ignore_ascii_case(text)).then_some(value)
}

/// `n` (at least 1) as a roman numeral in capitals.
pub fn roman(mut n: usize) -> String {
    const STEPS: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut text = String::new();
    for (value, letters) in STEPS {
        while n >= value {
            text.push_str(letters);
            n -= value;
        }
    }
    text
}

/// The value of letters counted a, b, ... z, aa, ab, ...; None for another text.
pub fn letters_value(text: &str) -> Option<usize> {
    if text.is_empty() || text.len() > 6 {
        return None;
    }
    text.chars().try_fold(0usize, |n, c| {
        c.is_ascii_alphabetic()
            .then(|| n * 26 + (c.to_ascii_lowercase() as usize - 'a' as usize + 1))
    })
}

/// `n` (at least 1) as lower-case letters: 1 is a, 27 is aa.
pub fn letters(mut n: usize) -> String {
    let mut text = Vec::new();
    while n > 0 {
        n -= 1;
        text.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    text.iter().rev().collect()
}

fn indexing(p: &mut Parser) -> PResult<Indexing> {
    let mut style = IndexStyle::Number;
    let mut start: Option<(usize, String, crate::error::Span)> = None;
    settings(p, "/INDEXING", &["INDEXTYPE", "INDEXSTART"], |p, k| {
        if k == "INDEXTYPE" {
            style = match expect_keyword(
                p,
                &["NUMBER", "LETTERLOW", "LETTERCAP", "ROMANLOW", "ROMANCAP"],
            )? {
                "NUMBER" => IndexStyle::Number,
                "LETTERLOW" => IndexStyle::LetterLow,
                "LETTERCAP" => IndexStyle::LetterCap,
                "ROMANLOW" => IndexStyle::RomanLow,
                _ => IndexStyle::RomanCap,
            };
            return Ok(());
        }
        let at = p.span();
        start = Some(match p.peek() {
            Some(TokenKind::Str(text)) => {
                let text = text.clone();
                p.pos += 1;
                (0, text, at)
            }
            _ => (
                whole(p, 0.0, "a whole number or letters in quotes")?,
                String::new(),
                at,
            ),
        });
        Ok(())
    })?;
    let Some((number, text, at)) = start else {
        return Ok(Indexing { style, start: 1 });
    };
    let is_text = !text.is_empty();
    let fits = |lower: bool| {
        text.chars().all(|c| {
            if lower {
                c.is_ascii_lowercase()
            } else {
                c.is_ascii_uppercase()
            }
        })
    };
    let value = match style {
        IndexStyle::Number if !is_text => Some(number),
        IndexStyle::LetterLow if is_text && fits(true) => letters_value(&text),
        IndexStyle::LetterCap if is_text && fits(false) => letters_value(&text),
        IndexStyle::RomanLow if is_text && fits(true) => roman_value(&text),
        IndexStyle::RomanCap if is_text && fits(false) => roman_value(&text),
        _ => None,
    };
    let want = match style {
        IndexStyle::Number => "a whole number",
        IndexStyle::LetterLow => "lower-case letters in quotes, such as 'a'",
        IndexStyle::LetterCap => "capital letters in quotes, such as 'A'",
        IndexStyle::RomanLow => "a lower-case roman numeral in quotes, such as 'iv'",
        IndexStyle::RomanCap => "a roman numeral in capitals in quotes, such as 'IV'",
    };
    match value {
        Some(start) => Ok(Indexing { style, start }),
        None => Err(SyntaxError::new(
            at,
            format!("INDEXSTART for this INDEXTYPE is {want}"),
        )),
    }
}

fn table(p: &mut Parser, block: &mut ModifyBlock) -> PResult<()> {
    settings(
        p,
        "/TABLE",
        &[
            "TABLETITLE",
            "TRANSPOSE",
            "TLOOK",
            "SELECTEDLAYER",
            "SELECTLAYER",
            "SORT",
            "SORTDIRECTION",
            "TABLESUMMARY",
            "PIVOT",
        ],
        |p, k| {
            match k {
                "TABLETITLE" => block.table_title = Some(p.string("a title in quotes")?),
                "TRANSPOSE" => block.transpose = yes_no(p)?,
                "SORT" => {
                    let by = expect_keyword(p, &["COLLABEL", "COLPOSITION"])?;
                    p.expect(Punct::LParen)?;
                    block.sort = Some(if by == "COLLABEL" {
                        SortColumn::Label(p.string("a column label in quotes")?)
                    } else {
                        SortColumn::Position(whole(p, 1.0, "a column number from 1")?)
                    });
                    p.expect(Punct::RParen)?;
                }
                "SORTDIRECTION" => {
                    block.descending =
                        expect_keyword(p, &["ASCENDING", "DESCENDING"])? == "DESCENDING"
                }
                "TABLESUMMARY" => {
                    p.string("a text in quotes")?;
                    block.not_applied.push((
                        "/TABLE TABLESUMMARY".into(),
                        "Statscraft tables have no summary text yet",
                    ));
                }
                other => {
                    skip_value(p)?;
                    let name = if other == "SELECTLAYER" {
                        "SELECTEDLAYER"
                    } else {
                        other
                    };
                    block.not_applied.push((format!("/TABLE {name}"), NO_LOOKS));
                }
            }
            Ok(())
        },
    )
}

fn table_cells(
    p: &mut Parser,
    not_applied: &mut Vec<(String, &'static str)>,
) -> PResult<TableCells> {
    let mut cells = TableCells {
        select: Vec::new(),
        dimension: Dimension::Both,
        conditions: Vec::new(),
        apply_to: ApplyTo::Cell,
        format: None,
        replace: None,
        hide: false,
    };
    settings(
        p,
        "/TABLECELLS",
        &[
            "SELECT",
            "SELECTDIMENSION",
            "SELECTCONDITION",
            "COLUMNWIDTH",
            "APPLYTO",
            "REVERTTODEFAULT",
            "FORMAT",
            "FONT",
            "FONTSIZE",
            "STYLE",
            "TEXTCOLOR",
            "BACKGROUNDCOLOR",
            "REPLACE",
            "HIDE",
            "UNGROUP",
        ],
        |p, k| {
            match k {
                "SELECT" => cells.select = bracket_list(p, cell_area)?,
                "SELECTDIMENSION" => {
                    cells.dimension = match expect_keyword(p, &["BOTH", "COLUMNS", "ROWS"])? {
                        "COLUMNS" => Dimension::Columns,
                        "ROWS" => Dimension::Rows,
                        _ => Dimension::Both,
                    }
                }
                "SELECTCONDITION" => {
                    let all = bracket_list(p, condition)?;
                    // ALL anywhere in the list means every cell
                    cells.conditions = if all.iter().any(Option::is_none) {
                        Vec::new()
                    } else {
                        all.into_iter().flatten().collect()
                    };
                }
                "APPLYTO" => {
                    cells.apply_to = match expect_keyword(
                        p,
                        &[
                            "CELL",
                            "COLUMN",
                            "ROW",
                            "COLUMNDATA",
                            "ROWDATA",
                            "COLUMNHEADER",
                            "ROWHEADER",
                        ],
                    )? {
                        "COLUMN" => ApplyTo::Column,
                        "ROW" => ApplyTo::Row,
                        "COLUMNDATA" => ApplyTo::ColumnData,
                        "ROWDATA" => ApplyTo::RowData,
                        "COLUMNHEADER" => ApplyTo::ColumnHeader,
                        "ROWHEADER" => ApplyTo::RowHeader,
                        _ => ApplyTo::Cell,
                    }
                }
                "REVERTTODEFAULT" => {
                    if yes_no(p)? {
                        not_applied.push((
                            "/TABLECELLS REVERTTODEFAULT".into(),
                            "Statscraft tables have no styles to remove, and a FORMAT or REPLACE \
                             change stays",
                        ));
                    }
                }
                "FORMAT" => {
                    let at = p.span();
                    let Some(TokenKind::Word(word)) = p.peek() else {
                        return Err(p.unexpected(&["a format such as F8.2"]));
                    };
                    crate::commands::format(word)
                        .map_err(|message| SyntaxError::new(at, message))?;
                    cells.format = Some(word.to_uppercase());
                    p.pos += 1;
                }
                "REPLACE" => {
                    cells.replace = Some(
                        p.value()
                            .ok_or_else(|| p.unexpected(&["a number or a text in quotes"]))?,
                    )
                }
                "HIDE" => cells.hide = yes_no(p)?,
                "UNGROUP" => {
                    if yes_no(p)? {
                        not_applied.push((
                            "/TABLECELLS UNGROUP".into(),
                            "Statscraft cannot take group labels out of a table yet",
                        ));
                    }
                }
                "COLUMNWIDTH" => {
                    skip_value(p)?;
                    not_applied.push(("/TABLECELLS COLUMNWIDTH".into(), NO_SIZES));
                }
                other => {
                    skip_value(p)?;
                    not_applied.push((format!("/TABLECELLS {other}"), NO_STYLES));
                }
            }
            Ok(())
        },
    )?;
    if cells.select.is_empty() {
        return Err(p.unexpected(&["SELECT=[...]"]));
    }
    Ok(cells)
}

fn cell_area(p: &mut Parser) -> PResult<CellArea> {
    if let Some(TokenKind::Str(text)) = p.peek() {
        let text = text.clone();
        p.pos += 1;
        return Ok(CellArea::Labels(vec![text]));
    }
    let mut known: Vec<&str> = LABEL_KEYWORDS.iter().map(|(k, _)| *k).collect();
    known.extend([
        "POSITION",
        "BODY",
        "HEADERS",
        "FOOTNOTES",
        "TITLE",
        "CAPTION",
    ]);
    let keyword = expect_keyword(p, &known)?;
    Ok(match keyword {
        "POSITION" => {
            p.expect(Punct::LParen)?;
            let n = whole(p, 1.0, "a position from 1")?;
            p.expect(Punct::RParen)?;
            CellArea::Position(n)
        }
        "BODY" => CellArea::Body,
        "HEADERS" => CellArea::Headers,
        "FOOTNOTES" => CellArea::Footnotes,
        "TITLE" => CellArea::Title,
        "CAPTION" => CellArea::Caption,
        k => {
            let labels = LABEL_KEYWORDS.iter().find(|(name, _)| *name == k);
            CellArea::Labels(
                labels
                    .expect("listed")
                    .1
                    .iter()
                    .map(|l| l.to_string())
                    .collect(),
            )
        }
    })
}

/// One SELECTCONDITION item; None is ALL.
fn condition(p: &mut Parser) -> PResult<Option<CellCondition>> {
    if let Some(TokenKind::Str(text)) = p.peek() {
        let at = p.span();
        let parsed = comparison(text).ok_or_else(|| {
            SyntaxError::new(
                at,
                format!(
                    "'{text}' is no condition; write x, or ABS(x), then <, <=, >, >=, = or <> and \
                     a number, SYSMIS or a text in quotes, as in 'x<0.05'"
                ),
            )
        })?;
        p.pos += 1;
        return Ok(Some(parsed));
    }
    let at = p.span();
    match expect_keyword(p, &["ALL", "TOP", "BOTTOM", "PARENT", "CHILD"])? {
        "ALL" => Ok(None),
        "PARENT" | "CHILD" => Err(SyntaxError::new(
            at,
            "PARENT and CHILD conditions are not supported yet; pick the cells by label, \
             position or comparison",
        )),
        which => {
            p.expect(Punct::LParen)?;
            let n = whole(p, 1.0, "a count from 1")?;
            p.expect(Punct::RParen)?;
            Ok(Some(if which == "TOP" {
                CellCondition::Top(n)
            } else {
                CellCondition::Bottom(n)
            }))
        }
    }
}

/// A comparison on the stand-in x, such as `x<=.05`, `ABS(x)>2`, `x=SYSMIS` or `x<>'n/a'`.
pub fn comparison(text: &str) -> Option<CellCondition> {
    let rest = text.trim_start();
    let (abs, rest) = match rest.get(..3) {
        Some(word) if word.eq_ignore_ascii_case("ABS") => {
            let inner = rest[3..].trim_start().strip_prefix('(')?.trim_start();
            let inner = inner.strip_prefix('x')?.trim_start();
            (true, inner.strip_prefix(')')?)
        }
        _ => (false, rest.strip_prefix('x')?),
    };
    let rest = rest.trim_start();
    const OPS: [(&str, CompareOp); 7] = [
        ("<=", CompareOp::Le),
        (">=", CompareOp::Ge),
        ("<>", CompareOp::Ne),
        ("~=", CompareOp::Ne),
        ("<", CompareOp::Lt),
        (">", CompareOp::Gt),
        ("=", CompareOp::Eq),
    ];
    let (sign, op) = OPS.iter().find(|(s, _)| rest.starts_with(s))?;
    let value = rest[sign.len()..].trim();
    let quoted = |q: char| value.strip_prefix(q).and_then(|v| v.strip_suffix(q));
    let value = if let Some(text) = quoted('\'').or_else(|| quoted('"')) {
        CompareValue::Text(text.to_string())
    } else if value.eq_ignore_ascii_case("SYSMIS") {
        CompareValue::Sysmis
    } else {
        CompareValue::Number(value.parse().ok()?)
    };
    Some(CellCondition::Compare {
        abs,
        op: *op,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_values_read_and_write() {
        assert_eq!(letters_value("a"), Some(1));
        assert_eq!(letters_value("AB"), Some(28));
        assert_eq!(letters(28), "ab");
        assert_eq!(letters(26), "z");
        assert_eq!(roman_value("xiv"), Some(14));
        assert_eq!(roman_value("IIII"), None);
        assert_eq!(roman_value("q"), None);
        assert_eq!(roman(1994), "MCMXCIV");
    }

    #[test]
    fn comparisons_on_x() {
        let compare = |abs, op, value| Some(CellCondition::Compare { abs, op, value });
        assert_eq!(
            comparison("x<=.05"),
            compare(false, CompareOp::Le, CompareValue::Number(0.05))
        );
        assert_eq!(
            comparison(" ABS( x ) > 2"),
            compare(true, CompareOp::Gt, CompareValue::Number(2.0))
        );
        assert_eq!(
            comparison("x<>'n/a'"),
            compare(false, CompareOp::Ne, CompareValue::Text("n/a".into()))
        );
        assert_eq!(
            comparison("x=sysmis"),
            compare(false, CompareOp::Eq, CompareValue::Sysmis)
        );
        assert_eq!(comparison("y<1"), None);
        assert_eq!(comparison("x<<1"), None);
    }

    #[test]
    fn command_picks() {
        let named = |name: &str, instance| CommandPick::Named {
            name: name.into(),
            instance,
        };
        assert_eq!(command_pick("Means"), Some(named("Means", None)));
        assert_eq!(
            command_pick("Means(3)"),
            Some(named("Means", Some(Instance::Nth(3))))
        );
        assert_eq!(
            command_pick("T-Test (last)"),
            Some(named("T-Test", Some(Instance::Last)))
        );
        assert_eq!(command_pick("Means(0)"), None);
        assert_eq!(command_pick(""), None);
    }
}
