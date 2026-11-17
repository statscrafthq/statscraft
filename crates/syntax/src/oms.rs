//! OMS, OMSEND, OMSINFO and OMSLOG: requests that route the output of later commands to files
//! and keep chosen output out of the results.
//!
//! Rules, from the descriptions of these commands:
//! - OMS needs /DESTINATION with FORMAT (and OUTFILE) or VIEWER, or both. Every other
//!   subcommand is optional, comes at most once and may stand in any order; the slash before
//!   the first one may be left out.
//! - /SELECT names output kinds, or ALL with EXCEPT=[kinds]; without it every kind is chosen.
//! - /IF and /EXCEPTIF take COMMANDS, SUBTYPES and LABELS lists of quoted texts in square
//!   brackets; /IF also takes INSTANCES=[n ... LAST]. A list keeps one copy of a repeated item.
//! - /COLUMNS takes DIMNAMES=['name' ...] or SEQUENCE=[R1 C2 RALL ...]; a repeated item in
//!   SEQUENCE is an error, and ALL with a position of the same dimension is an error.
//! - /TAG='id' names the request; the name cannot start with a dollar sign.
//! - OMSEND ends requests by TAG or FILE, or all of them; LOG ends the log. With nothing
//!   given it ends every request and the log.
//! - OMSLOG FILE='file' [/APPEND=NO | YES] [/FORMAT=XML | TEXT].
//!
//! Choices where the manual is silent:
//! - The equals signs of DESTINATION may be left out (FORMAT SAV), and `/DESTINATION = SAV`
//!   names the format, as the manual's own examples write them.

use crate::SyntaxError;
use crate::ast::Command;
use crate::lexer::{Punct, TokenKind};
use crate::output_docs::ImageFormat;
use crate::output_modify::{Instance, bracket_list, expect_keyword, keyword_in, yes_no};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The kinds of output an OMS request can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmsKind {
    Charts,
    Headings,
    Logs,
    Models,
    Tables,
    Texts,
    Trees,
    Warnings,
}

const KINDS: [(&str, OmsKind); 8] = [
    ("CHARTS", OmsKind::Charts),
    ("HEADINGS", OmsKind::Headings),
    ("LOGS", OmsKind::Logs),
    ("MODELS", OmsKind::Models),
    ("TABLES", OmsKind::Tables),
    ("TEXTS", OmsKind::Texts),
    ("TREES", OmsKind::Trees),
    ("WARNINGS", OmsKind::Warnings),
];

impl OmsKind {
    pub fn name(self) -> &'static str {
        KINDS.iter().find(|(_, k)| *k == self).expect("listed").0
    }
}

/// The conditions of /IF or /EXCEPTIF; an empty list sets no condition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OmsCondition {
    pub commands: Vec<String>,
    pub subtypes: Vec<String>,
    pub labels: Vec<String>,
    pub instances: Vec<Instance>,
}

impl OmsCondition {
    pub fn is_empty(&self) -> bool {
        *self == OmsCondition::default()
    }
}

/// /DESTINATION.
#[derive(Debug, Clone, PartialEq)]
pub struct OmsDestination {
    /// FORMAT, as its keyword: SAV, HTML, TEXT, TABTEXT, XLSX, XLS, DOC, PDF, OXML, SPV, ...
    pub format: Option<&'static str>,
    /// OUTFILE: a file in quotes, a file handle or a dataset name.
    pub outfile: Option<String>,
    /// XMLWORKSPACE or OUTPUTSET, given instead of OUTFILE.
    pub other_target: Option<&'static str>,
    pub viewer: bool,
    /// NUMBERED: the name of the table number variable.
    pub numbered: Option<String>,
    pub images: bool,
    pub image_format: ImageFormat,
    /// CHARTSIZE, in percent.
    pub chart_size: f64,
    /// Keywords accepted that change nothing in Statscraft.
    pub ignored: Vec<&'static str>,
}

/// /COLUMNS.
#[derive(Debug, Clone, PartialEq)]
pub enum OmsColumns {
    Dimnames(Vec<String>),
    /// Each item as its dimension letter (R, C or L) and its position from 1; None for ALL.
    Sequence(Vec<(char, Option<usize>)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Oms {
    pub kinds: Vec<OmsKind>,
    pub condition: OmsCondition,
    pub exception: OmsCondition,
    pub destination: OmsDestination,
    pub columns: Option<OmsColumns>,
    pub tag: Option<String>,
    pub nowarn: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Omsend {
    pub tags: Vec<String>,
    /// TAG=ALL.
    pub all_tags: bool,
    pub files: Vec<String>,
    pub log: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Omslog {
    pub file: String,
    pub append: bool,
    /// FORMAT=TEXT; else XML.
    pub text: bool,
}

const FORMATS: [&str; 13] = [
    "DOC",
    "HTML",
    "OXML",
    "PDF",
    "TABTEXT",
    "TEXT",
    "SAV",
    "SPV",
    "SPW",
    "XLS",
    "XLSX",
    "REPORTHTML",
    "REPORTMHT",
];

fn kind(p: &mut Parser) -> PResult<OmsKind> {
    let names: Vec<&'static str> = KINDS.iter().map(|(n, _)| *n).collect();
    let name = expect_keyword(p, &names)?;
    Ok(KINDS.iter().find(|(n, _)| *n == name).expect("listed").1)
}

fn deduped<T: PartialEq>(items: Vec<T>) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn select(p: &mut Parser) -> PResult<Vec<OmsKind>> {
    let all: Vec<OmsKind> = KINDS.iter().map(|(_, k)| *k).collect();
    if p.eat_word("ALL") {
        let mut kinds = all;
        if p.eat_keyword("EXCEPT") {
            p.expect(Punct::Eq)?;
            let except = bracket_list(p, kind)?;
            kinds.retain(|k| !except.contains(k));
        }
        return Ok(kinds);
    }
    let mut kinds = vec![kind(p)?];
    while !at_end(p) {
        kinds.push(kind(p)?);
        p.eat(Punct::Comma);
    }
    Ok(deduped(kinds))
}

fn condition(p: &mut Parser, except: bool) -> PResult<OmsCondition> {
    const KEYS: [&str; 4] = ["COMMANDS", "SUBTYPES", "LABELS", "INSTANCES"];
    let mut c = OmsCondition::default();
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(p.unexpected(&KEYS));
    }
    while !at_end(p) {
        let at = p.span();
        let key = expect_keyword(p, &KEYS)?;
        if seen.contains(&key) {
            return Err(SyntaxError::new(at, format!("{key} is given twice")));
        }
        seen.push(key);
        if except && key == "INSTANCES" {
            return Err(SyntaxError::new(
                at,
                "INSTANCES belongs on /IF; /EXCEPTIF does not take it",
            ));
        }
        p.expect(Punct::Eq)?;
        if key == "INSTANCES" {
            c.instances = deduped(bracket_list(p, |p| {
                if p.eat_word("LAST") {
                    return Ok(Instance::Last);
                }
                match p.peek() {
                    Some(TokenKind::Number(n)) if n.fract() == 0.0 && (1.0..=1e9).contains(n) => {
                        let n = *n as usize;
                        p.pos += 1;
                        Ok(Instance::Nth(n))
                    }
                    _ => Err(p.unexpected(&["a whole number from 1", "LAST"])),
                }
            })?);
        } else {
            let texts = deduped(bracket_list(p, |p| p.string("a text in quotes"))?);
            match key {
                "COMMANDS" => c.commands = texts,
                "SUBTYPES" => c.subtypes = texts,
                _ => c.labels = texts,
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(c)
}

fn destination(p: &mut Parser, start: usize) -> PResult<OmsDestination> {
    const KEYS: [&str; 18] = [
        "FORMAT",
        "OUTFILE",
        "XMLWORKSPACE",
        "OUTPUTSET",
        "FOLDER",
        "VIEWER",
        "NUMBERED",
        "IMAGES",
        "IMAGEFORMAT",
        "IMAGEROOT",
        "CHARTSIZE",
        "IMAGEMAP",
        "TREEFORMAT",
        "CHARTFORMAT",
        "MODELFORMAT",
        "TABLES",
        "REPORTTITLE",
        "SHOWOUTLINE",
    ];
    let mut d = OmsDestination {
        format: None,
        outfile: None,
        other_target: None,
        viewer: true,
        numbered: None,
        images: true,
        image_format: ImageFormat::Png,
        chart_size: 100.0,
        ignored: Vec::new(),
    };
    let mut seen = Vec::new();
    if p.eat(Punct::Eq) {
        d.format = Some(expect_keyword(p, &FORMATS)?);
        seen.push("FORMAT");
    }
    while !at_end(p) {
        let at = p.span();
        let key = expect_keyword(p, &KEYS)?;
        if seen.contains(&key) {
            return Err(SyntaxError::new(at, format!("{key} is given twice")));
        }
        seen.push(key);
        let equals = p.eat(Punct::Eq);
        match key {
            "FORMAT" => d.format = Some(expect_keyword(p, &FORMATS)?),
            "OUTFILE" => d.outfile = Some(p.file("a file in quotes or a dataset name")?),
            "XMLWORKSPACE" => {
                p.string("a name in quotes")?;
                d.other_target = Some(key);
            }
            "OUTPUTSET" => {
                expect_keyword(p, &["SUBTYPES", "LABELS"])?;
                d.other_target = Some(key);
            }
            "VIEWER" => d.viewer = yes_no(p)?,
            "NUMBERED" => {
                let name = if equals {
                    p.string("a variable name in quotes")?
                } else {
                    "TableNumber_".into()
                };
                d.numbered = Some(name);
            }
            "IMAGES" => d.images = yes_no(p)?,
            "IMAGEFORMAT" => {
                d.image_format = match expect_keyword(p, &["PNG", "JPG", "BMP", "VML"])? {
                    "JPG" => ImageFormat::Jpg,
                    "BMP" => ImageFormat::Bmp,
                    "VML" => {
                        d.ignored.push("IMAGEFORMAT=VML");
                        ImageFormat::Png
                    }
                    _ => ImageFormat::Png,
                }
            }
            "CHARTSIZE" => match p.peek() {
                Some(TokenKind::Number(n)) if (10.0..=200.0).contains(n) => {
                    d.chart_size = *n;
                    p.pos += 1;
                }
                _ => return Err(p.unexpected(&["a percentage from 10 to 200"])),
            },
            "FOLDER" | "IMAGEROOT" | "REPORTTITLE" => {
                p.string("a text in quotes")?;
                d.ignored.push(key);
            }
            _ => {
                match p.peek() {
                    Some(TokenKind::Word(_) | TokenKind::Str(_)) => p.pos += 1,
                    _ => return Err(p.unexpected(&["a value"])),
                }
                d.ignored.push(key);
            }
        }
        p.eat(Punct::Comma);
    }
    let span = p.span_from(start);
    if d.format.is_none() && !seen.contains(&"VIEWER") {
        return Err(SyntaxError::new(
            span,
            "/DESTINATION needs FORMAT with OUTFILE, or VIEWER, or both",
        ));
    }
    if d.format.is_some() && d.outfile.is_none() && d.other_target.is_none() {
        return Err(SyntaxError::new(
            span,
            "FORMAT needs OUTFILE='file' on /DESTINATION",
        ));
    }
    if d.format.is_none() && (d.outfile.is_some() || d.other_target.is_some()) {
        return Err(SyntaxError::new(
            span,
            "OUTFILE needs FORMAT on /DESTINATION, such as FORMAT=SAV",
        ));
    }
    Ok(d)
}

fn columns(p: &mut Parser) -> PResult<OmsColumns> {
    let key = expect_keyword(p, &["DIMNAMES", "SEQUENCE"])?;
    p.expect(Punct::Eq)?;
    if key == "DIMNAMES" {
        let names = bracket_list(p, |p| p.string("a dimension name in quotes"))?;
        return Ok(OmsColumns::Dimnames(deduped(names)));
    }
    let items = bracket_list(p, |p| {
        let at = p.span();
        let word = match p.peek() {
            Some(TokenKind::Word(w)) => w.to_uppercase(),
            _ => return Err(p.unexpected(&["R1, C1, L1, RALL, CALL or LALL"])),
        };
        let mut chars = word.chars();
        let place = chars.next().filter(|c| "RCL".contains(*c));
        let rest = chars.as_str();
        let position = match rest {
            "ALL" => Some(None),
            _ => rest.parse::<usize>().ok().filter(|n| *n > 0).map(Some),
        };
        match (place, position) {
            (Some(place), Some(position)) => {
                p.pos += 1;
                Ok((place, position, at))
            }
            _ => Err(SyntaxError::new(
                at,
                format!("{word} is no position: write R, C or L with a number from 1, or with ALL"),
            )),
        }
    })?;
    let mut out: Vec<(char, Option<usize>)> = Vec::new();
    for (place, position, at) in items {
        if out.contains(&(place, position)) {
            return Err(SyntaxError::new(at, "SEQUENCE names this position twice"));
        }
        let mixed = out
            .iter()
            .any(|(p, n)| *p == place && (n.is_none() != position.is_none()));
        if mixed {
            return Err(SyntaxError::new(
                at,
                format!("{place}ALL and a numbered {place} position cannot stand together"),
            ));
        }
        out.push((place, position));
    }
    Ok(OmsColumns::Sequence(out))
}

pub(crate) fn oms(p: &mut Parser) -> PResult<Command> {
    const SUBCOMMANDS: [&str; 6] = [
        "SELECT",
        "EXCEPTIF",
        "DESTINATION",
        "COLUMNS",
        "TAG",
        "NOWARN",
    ];
    let start = p.pos;
    let mut kinds = None;
    let mut when = OmsCondition::default();
    let mut unless = OmsCondition::default();
    let mut destination_spec = None;
    let mut columns_spec = None;
    let mut tag = None;
    let mut nowarn = false;
    let mut seen: Vec<&str> = Vec::new();
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() && !slash {
            break;
        }
        if !slash && !first {
            p.finish(&["'/'"])?;
        }
        first = false;
        let at = p.span();
        let name = if p.eat_word("IF") {
            "IF"
        } else {
            let Some(name) = keyword_in(p, &SUBCOMMANDS)? else {
                return Err(p.subcommand_error("OMS", &[]));
            };
            p.pos += 1;
            name
        };
        if seen.contains(&name) {
            return Err(SyntaxError::new(at, format!("/{name} is given twice")));
        }
        seen.push(name);
        let sub = p.pos;
        match name {
            "SELECT" => kinds = Some(select(p)?),
            "IF" => when = condition(p, false)?,
            "EXCEPTIF" => unless = condition(p, true)?,
            "DESTINATION" => destination_spec = Some(destination(p, sub)?),
            "COLUMNS" => columns_spec = Some(columns(p)?),
            "TAG" => {
                p.eat(Punct::Eq);
                let at = p.span();
                let text = p.string("a name in quotes")?;
                if text.starts_with('$') || text.trim().is_empty() {
                    return Err(SyntaxError::new(
                        at,
                        "a TAG cannot be empty or start with a dollar sign",
                    ));
                }
                tag = Some(text);
            }
            _ => nowarn = true,
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    let destination = destination_spec.ok_or_else(|| {
        SyntaxError::new(
            p.span_from(start),
            "OMS needs /DESTINATION with FORMAT and OUTFILE, or VIEWER",
        )
    })?;
    Ok(Command::Oms(Box::new(Oms {
        kinds: kinds.unwrap_or_else(|| KINDS.iter().map(|(_, k)| *k).collect()),
        condition: when,
        exception: unless,
        destination,
        columns: columns_spec,
        tag,
        nowarn,
    })))
}

pub(crate) fn omsend(p: &mut Parser) -> PResult<Command> {
    let mut end = Omsend::default();
    let mut seen = Vec::new();
    while p.peek().is_some() {
        p.eat(Punct::Slash);
        let at = p.span();
        let key = expect_keyword(p, &["TAG", "FILE", "LOG"])?;
        if seen.contains(&key) {
            return Err(SyntaxError::new(at, format!("{key} is given twice")));
        }
        seen.push(key);
        match key {
            "TAG" => {
                p.expect(Punct::Eq)?;
                if p.eat_word("ALL") {
                    end.all_tags = true;
                } else {
                    end.tags = deduped(bracket_list(p, |p| p.string("a tag in quotes"))?);
                }
            }
            "FILE" => {
                p.expect(Punct::Eq)?;
                end.files = deduped(bracket_list(p, |p| p.string("a file in quotes"))?);
            }
            _ => end.log = true,
        }
    }
    Ok(Command::Omsend(end))
}

pub(crate) fn omsinfo(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Omsinfo)
}

pub(crate) fn omslog(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let (mut file, mut append, mut text) = (None, false, false);
    let mut seen = Vec::new();
    while p.peek().is_some() {
        p.eat(Punct::Slash);
        let at = p.span();
        let key = expect_keyword(p, &["FILE", "APPEND", "FORMAT"])?;
        if seen.contains(&key) {
            return Err(SyntaxError::new(at, format!("{key} is given twice")));
        }
        seen.push(key);
        p.expect(Punct::Eq)?;
        match key {
            "FILE" => file = Some(p.file("a file in quotes")?),
            "APPEND" => append = yes_no(p)?,
            _ => text = expect_keyword(p, &["XML", "TEXT"])? == "TEXT",
        }
    }
    let file =
        file.ok_or_else(|| SyntaxError::new(p.span_from(start), "OMSLOG needs FILE='file'"))?;
    Ok(Command::Omslog(Omslog { file, append, text }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_command(source: &str) -> Result<Command, SyntaxError> {
        let (mut parsed, mut errors) = crate::parse(source);
        match errors.pop() {
            Some(e) => Err(e),
            None => Ok(parsed.remove(0).command),
        }
    }

    fn oms(source: &str) -> Oms {
        match parse_command(source) {
            Ok(Command::Oms(oms)) => *oms,
            other => panic!("{source}: {other:?}"),
        }
    }

    fn error(source: &str) -> String {
        match parse_command(source) {
            Err(e) => e.message,
            other => panic!("{source}: {other:?}"),
        }
    }

    #[test]
    fn a_full_request_reads_every_subcommand() {
        let o = oms(
            "OMS /SELECT TABLES CHARTS /IF COMMANDS=['Frequencies' 'Frequencies'] \
             SUBTYPES=['Stat*'] INSTANCES=[1 LAST] /EXCEPTIF LABELS=['Notes'] \
             /DESTINATION FORMAT=SAV OUTFILE='out.sav' NUMBERED='nr' VIEWER=NO \
             /COLUMNS SEQUENCE=[R2 CALL] /TAG='first' /NOWARN.",
        );
        assert_eq!(o.kinds, [OmsKind::Tables, OmsKind::Charts]);
        assert_eq!(o.condition.commands, ["Frequencies"]);
        assert_eq!(o.condition.instances, [Instance::Nth(1), Instance::Last]);
        assert_eq!(o.exception.labels, ["Notes"]);
        assert_eq!(o.destination.format, Some("SAV"));
        assert_eq!(o.destination.outfile.as_deref(), Some("out.sav"));
        assert_eq!(o.destination.numbered.as_deref(), Some("nr"));
        assert!(!o.destination.viewer);
        assert_eq!(
            o.columns,
            Some(OmsColumns::Sequence(vec![('R', Some(2)), ('C', None)]))
        );
        assert_eq!(o.tag.as_deref(), Some("first"));
        assert!(o.nowarn);
    }

    #[test]
    fn short_forms_and_defaults() {
        let o =
            oms("OMS SELECT ALL EXCEPT=[CHARTS] /DESTINATION FORMAT SAV OUTFILE='a.sav' NUMBERED.");
        assert!(!o.kinds.contains(&OmsKind::Charts) && o.kinds.len() == 7);
        assert_eq!(o.destination.numbered.as_deref(), Some("TableNumber_"));
        let o = oms("OMS /DESTINATION = HTML OUTFILE='a.htm' IMAGEMAP=YES.");
        assert_eq!(o.destination.format, Some("HTML"));
        assert_eq!(o.destination.ignored, ["IMAGEMAP"]);
        assert_eq!(o.kinds.len(), 8);
        assert!(
            oms("OMS /DESTINATION VIEWER=NO.")
                .destination
                .format
                .is_none()
        );
    }

    #[test]
    fn mistakes_are_named() {
        assert!(error("OMS /SELECT TABLES.").contains("DESTINATION"));
        assert!(error("OMS /DESTINATION FORMAT=SAV.").contains("OUTFILE"));
        assert!(error("OMS /DESTINATION VIEWER=NO /TAG='$x'.").contains("dollar"));
        assert!(error("OMS /EXCEPTIF INSTANCES=[1] /DESTINATION VIEWER=NO.").contains("INSTANCES"));
        assert!(error("OMS /COLUMNS SEQUENCE=[R1 R1] /DESTINATION VIEWER=NO.").contains("twice"));
        assert!(error("OMS /COLUMNS SEQUENCE=[RALL R1] /DESTINATION VIEWER=NO.").contains("RALL"));
        assert!(error("OMS /TAG='a' /TAG='b' /DESTINATION VIEWER=NO.").contains("twice"));
    }

    #[test]
    fn omsend_omsinfo_and_omslog() {
        let end = |s: &str| match parse_command(s) {
            Ok(Command::Omsend(e)) => e,
            other => panic!("{other:?}"),
        };
        assert_eq!(end("OMSEND."), Omsend::default());
        assert_eq!(end("OMSEND TAG=['a' 'b'] LOG.").tags, ["a", "b"]);
        assert!(end("OMSEND TAG=ALL.").all_tags);
        assert_eq!(end("OMSEND FILE=['x.sav'].").files, ["x.sav"]);
        assert!(matches!(parse_command("OMSINFO."), Ok(Command::Omsinfo)));
        match parse_command("OMSLOG FILE='log.txt' /APPEND=YES /FORMAT=TEXT.") {
            Ok(Command::Omslog(log)) => assert!(log.append && log.text && log.file == "log.txt"),
            other => panic!("{other:?}"),
        }
        assert!(parse_command("OMSLOG /FORMAT=TEXT.").is_err());
    }
}
