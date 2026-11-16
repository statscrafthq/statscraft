//! Parsers of the data checking commands: COMPARE DATASETS and VALIDATEDATA.
//!
//! Rules from the commands' descriptions:
//! - Both: every subcommand and keyword is written in full (no abbreviations), each subcommand
//!   at most once, a keyword at most once within its subcommand.
//! - COMPARE DATASETS: COMPDATASET (a quoted .sav file, or the unquoted name of an open dataset,
//!   with an optional PASSWORD that is read and dropped) and VARIABLES (a list or ALL) are
//!   required. CASEID names the key variables. SAVE: FLAGMISMATCHES=YES|NO (YES) with
//!   VARNAME (CasesCompare); MATCHDATASET and MISMATCHDATASET =NO|YES with MATCHNAME and
//!   MISMATCHNAME (a dataset name, or a quoted file with MATCHPASS or MISMATCHPASS, NONE by
//!   default); ENCRYPTEDPW=NO|YES. OUTPUT: VARPROPERTIES=NONE|ALL|properties and
//!   CASETABLE=YES|NO with TABLELIMIT=n|NONE (100).
//! - VALIDATEDATA: VARIABLES=, ID= and CROSSVARRULES= before the first slash, at least one of
//!   them; the equals signs are required. CROSSVARRULES takes `$VD.CRule[n]`, plain numbers n
//!   or ALL. Subcommands VARCHECKS, IDCHECKS, CASECHECKS, RULESUMMARIES, CASEREPORT and SAVE;
//!   an empty one is an error.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The dictionary properties COMPARE DATASETS /OUTPUT VARPROPERTIES can compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareProperty {
    Measure,
    Label,
    ValueLabels,
    Columns,
    Missing,
    Align,
    Role,
    Attributes,
    Width,
}

const PROPERTIES: [(&str, CompareProperty); 9] = [
    ("MEASURE", CompareProperty::Measure),
    ("LABEL", CompareProperty::Label),
    ("VALUELABELS", CompareProperty::ValueLabels),
    ("COLUMNS", CompareProperty::Columns),
    ("MISSING", CompareProperty::Missing),
    ("ALIGN", CompareProperty::Align),
    ("ROLE", CompareProperty::Role),
    ("ATTRIBUTES", CompareProperty::Attributes),
    ("WIDTH", CompareProperty::Width),
];

/// Where MATCHDATASET or MISMATCHDATASET writes its cases.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareOutput {
    pub name: String,
    /// A quoted file name; else the name of a dataset.
    pub file: bool,
    /// A password other than NONE was given.
    pub password: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareDatasets {
    /// The other dataset: a file when `other_file`, else a dataset name.
    pub other: String,
    pub other_file: bool,
    pub variables: Vec<VarItem>,
    pub case_id: Vec<VarItem>,
    /// The flag variable's name; None for FLAGMISMATCHES=NO.
    pub flag: Option<String>,
    pub matches: Option<CompareOutput>,
    pub mismatches: Option<CompareOutput>,
    pub properties: Vec<CompareProperty>,
    /// None for CASETABLE=NO; Some(None) for TABLELIMIT=NONE.
    pub case_table: Option<Option<usize>>,
}

/// Error with the span from `start` to the current token.
fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message.into())
}

/// `YES` or `NO` (or `ON` and `OFF` when `on_off`).
fn yes_no(p: &mut Parser, on_off: bool) -> PResult<bool> {
    let words: &[(&str, bool)] = match on_off {
        true => &[("ON", true), ("OFF", false), ("YES", true), ("NO", false)],
        false => &[("YES", true), ("NO", false)],
    };
    for (word, value) in words {
        if p.eat_word(word) {
            return Ok(*value);
        }
    }
    Err(p.unexpected(&words.iter().map(|w| w.0).collect::<Vec<_>>()))
}

/// The keyword at the parser, if it is one of `keywords`; it must not have come before.
fn keyword<'k>(
    p: &mut Parser,
    keywords: &[&'k str],
    seen: &mut Vec<&'k str>,
    place: &str,
) -> PResult<&'k str> {
    let start = p.pos;
    let Some(&word) = keywords.iter().find(|k| p.eat_word(k)) else {
        return Err(p.unexpected(keywords));
    };
    if seen.contains(&word) {
        return Err(error(p, start, format!("{place} allows {word} once")));
    }
    seen.push(word);
    Ok(word)
}

/// The subcommand after a slash, once each.
fn subcommand<'k>(
    p: &mut Parser,
    command: &str,
    subcommands: &[&'k str],
    seen: &mut Vec<&'k str>,
) -> PResult<&'k str> {
    let start = p.pos;
    let Some(&name) = subcommands.iter().find(|s| p.eat_word(s)) else {
        return Err(p.subcommand_error(command, &[]));
    };
    if seen.contains(&name) {
        return Err(error(p, start, format!("{command} allows {name} once")));
    }
    seen.push(name);
    Ok(name)
}

/// Variables up to a slash, the end, or a word followed by '=' (the next keyword).
fn names_until_keyword(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
            break;
        }
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            items.push(match p.eat_word("TO") {
                true => VarItem::Range(first, p.name("a variable name")?),
                false => VarItem::Name(first),
            });
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

fn whole(p: &mut Parser, what: &str) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 && n < 1e15 => Ok(n as usize),
        Some(_) => Err(error(
            p,
            start,
            format!("{what} must be a positive whole number"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// `name` or `'file'` after MATCHNAME or MISMATCHNAME.
fn output_target(p: &mut Parser) -> PResult<(String, bool)> {
    match p.peek() {
        Some(TokenKind::Str(_)) => Ok((p.string("a quoted file name")?, true)),
        _ => Ok((p.name("a dataset name or a quoted file name")?, false)),
    }
}

/// `'password'` or NONE; true for a password.
fn password(p: &mut Parser) -> PResult<bool> {
    if p.eat_word("NONE") {
        return Ok(false);
    }
    p.string("a quoted password or NONE")?;
    Ok(true)
}

const COMPARE_SUBCOMMANDS: [&str; 5] = ["COMPDATASET", "VARIABLES", "CASEID", "SAVE", "OUTPUT"];

pub(crate) fn compare_datasets(p: &mut Parser) -> PResult<Command> {
    let begin = p.pos;
    let mut command = CompareDatasets {
        other: String::new(),
        other_file: false,
        variables: Vec::new(),
        case_id: Vec::new(),
        flag: Some("CasesCompare".into()),
        matches: None,
        mismatches: None,
        properties: Vec::new(),
        case_table: Some(Some(100)),
    };
    let mut seen = Vec::new();
    while p.eat(Punct::Slash) {
        let name = subcommand(p, "COMPARE DATASETS", &COMPARE_SUBCOMMANDS, &mut seen)?;
        p.eat(Punct::Eq);
        match name {
            "COMPDATASET" => {
                (command.other, command.other_file) = match p.peek() {
                    Some(TokenKind::Str(_)) => (p.string("a quoted file name")?, true),
                    _ => (p.name("a quoted .sav file or a dataset name")?, false),
                };
                if p.eat_word("PASSWORD") {
                    p.eat(Punct::Eq);
                    p.string("a quoted password")?;
                }
            }
            "VARIABLES" => command.variables = p.var_list()?,
            "CASEID" => command.case_id = p.var_list()?,
            "SAVE" => compare_save(p, &mut command)?,
            _ => compare_output(p, &mut command)?,
        }
    }
    p.finish(&["'/'"])?;
    for (required, what) in [
        ("COMPDATASET", "the other dataset"),
        ("VARIABLES", "the variables"),
    ] {
        if !seen.contains(&required) {
            return Err(error(
                p,
                begin,
                format!("COMPARE DATASETS needs /{required} with {what} to compare"),
            ));
        }
    }
    Ok(Command::CompareDatasets(Box::new(command)))
}

fn compare_save(p: &mut Parser, command: &mut CompareDatasets) -> PResult<()> {
    const KEYWORDS: [&str; 9] = [
        "FLAGMISMATCHES",
        "VARNAME",
        "MATCHDATASET",
        "MATCHNAME",
        "MATCHPASS",
        "MISMATCHDATASET",
        "MISMATCHNAME",
        "MISMATCHPASS",
        "ENCRYPTEDPW",
    ];
    let start = p.pos;
    let mut seen = Vec::new();
    let (mut flag, mut varname) = (true, None);
    let (mut matched, mut mismatched) = (false, false);
    let mut targets: [Option<(String, bool)>; 2] = [None, None];
    let mut passwords = [false, false];
    while !at_end(p) {
        let word = keyword(p, &KEYWORDS, &mut seen, "/SAVE")?;
        p.expect(Punct::Eq)?;
        match word {
            "FLAGMISMATCHES" => flag = yes_no(p, false)?,
            "VARNAME" => varname = Some(p.name("a new variable name")?),
            "MATCHDATASET" => matched = yes_no(p, false)?,
            "MISMATCHDATASET" => mismatched = yes_no(p, false)?,
            "MATCHNAME" => targets[0] = Some(output_target(p)?),
            "MISMATCHNAME" => targets[1] = Some(output_target(p)?),
            "MATCHPASS" => passwords[0] = password(p)?,
            "MISMATCHPASS" => passwords[1] = password(p)?,
            // the passwords are never used, so whether they are encrypted does not matter
            _ => _ = yes_no(p, false)?,
        }
        p.eat(Punct::Comma);
    }
    if seen.is_empty() {
        return Err(p.unexpected(&KEYWORDS));
    }
    command.flag = flag.then(|| varname.unwrap_or_else(|| "CasesCompare".into()));
    let [match_target, mismatch_target] = targets;
    for (wanted, target, password, keyword, slot) in [
        (
            matched,
            match_target,
            passwords[0],
            "MATCHDATASET=YES needs MATCHNAME",
            &mut command.matches,
        ),
        (
            mismatched,
            mismatch_target,
            passwords[1],
            "MISMATCHDATASET=YES needs MISMATCHNAME",
            &mut command.mismatches,
        ),
    ] {
        if !wanted {
            continue;
        }
        let Some((name, file)) = target else {
            return Err(error(
                p,
                start,
                format!("{keyword}: a dataset name or a quoted file name"),
            ));
        };
        *slot = Some(CompareOutput {
            name,
            file,
            password,
        });
    }
    Ok(())
}

fn compare_output(p: &mut Parser, command: &mut CompareDatasets) -> PResult<()> {
    const KEYWORDS: [&str; 3] = ["VARPROPERTIES", "CASETABLE", "TABLELIMIT"];
    let mut seen = Vec::new();
    let mut table = true;
    let mut limit = Some(100);
    while !at_end(p) {
        let word = keyword(p, &KEYWORDS, &mut seen, "/OUTPUT")?;
        p.expect(Punct::Eq)?;
        match word {
            "VARPROPERTIES" => {
                if p.eat_word("NONE") {
                    command.properties.clear();
                } else if p.eat_word("ALL") {
                    command.properties = PROPERTIES.iter().map(|e| e.1).collect();
                } else {
                    let mut any = false;
                    while let Some(&(_, property)) = PROPERTIES.iter().find(|e| p.eat_word(e.0)) {
                        if !command.properties.contains(&property) {
                            command.properties.push(property);
                        }
                        any = true;
                        p.eat(Punct::Comma);
                    }
                    if !any {
                        let mut expected = vec!["NONE", "ALL"];
                        expected.extend(PROPERTIES.iter().map(|e| e.0));
                        return Err(p.unexpected(&expected));
                    }
                }
            }
            "CASETABLE" => table = yes_no(p, false)?,
            _ => {
                limit = match p.eat_word("NONE") {
                    true => None,
                    false => Some(whole(p, "a number of cases or NONE")?),
                }
            }
        }
        p.eat(Punct::Comma);
    }
    if seen.is_empty() {
        return Err(p.unexpected(&KEYWORDS));
    }
    command.case_table = table.then_some(limit);
    Ok(())
}

/// A cross-variable rule on CROSSVARRULES: its number, or all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleItem {
    All,
    Number(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyScope {
    AnalysisVariables,
    AllVariables,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidateSave {
    EmptyCase,
    IncompleteId,
    DuplicateId,
    RuleViolations,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Validatedata {
    pub variables: Vec<VarItem>,
    pub id: Vec<VarItem>,
    pub rules: Vec<RuleItem>,
    /// None for VARCHECKS STATUS=OFF.
    pub var_checks: Option<VarChecks>,
    pub incomplete_ids: bool,
    pub duplicate_ids: bool,
    pub report_empty: bool,
    /// None: analysis variables when there are some, else all variables.
    pub empty_scope: Option<EmptyScope>,
    /// RULESUMMARIES: (by variable, by rule).
    pub by_variable: bool,
    pub by_rule: bool,
    /// RULESUMMARIES was given.
    pub summaries_given: bool,
    pub case_report: bool,
    pub min_violations: usize,
    /// None for CASELIMIT=NONE.
    pub case_limit: Option<usize>,
    /// Each with its name, if one was given.
    pub save: Vec<(ValidateSave, Option<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarChecks {
    pub pct_missing: f64,
    pub pct_equal: f64,
    pub pct_unequal: f64,
    pub cv: f64,
    pub std_dev: f64,
}

/// `[n]` after `$VD.CRule`.
fn rule_index(p: &mut Parser) -> PResult<usize> {
    p.expect(Punct::LBracket)?;
    let n = whole(p, "a rule number")?;
    p.expect(Punct::RBracket)?;
    Ok(n)
}

fn rule_list(p: &mut Parser) -> PResult<Vec<RuleItem>> {
    let mut rules = Vec::new();
    loop {
        if matches!(p.peek(), Some(TokenKind::Word(_)))
            && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
        {
            break;
        }
        if p.eat_word("ALL") {
            rules.push(RuleItem::All);
        } else if p.eat_word("$VD.CRule") {
            rules.push(RuleItem::Number(rule_index(p)?));
        } else if matches!(p.peek(), Some(TokenKind::Number(_))) {
            rules.push(RuleItem::Number(whole(p, "a rule number")?));
        } else {
            break;
        }
        p.eat(Punct::Comma);
    }
    if rules.is_empty() {
        return Err(p.unexpected(&["$VD.CRule[n]", "a rule number", "ALL"]));
    }
    Ok(rules)
}

/// A percentage above 0 and at most 100, or a number of at least 0 when `percent` is false.
fn threshold(p: &mut Parser, name: &str, percent: bool) -> PResult<f64> {
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    let fits = match percent {
        true => n > 0.0 && n <= 100.0,
        false => n >= 0.0,
    };
    if !fits {
        let range = match percent {
            true => "above 0 and at most 100",
            false => "0 or more",
        };
        return Err(error(p, start, format!("{name} must be {range}")));
    }
    Ok(n)
}

const VALIDATE_SUBCOMMANDS: [&str; 6] = [
    "VARCHECKS",
    "IDCHECKS",
    "CASECHECKS",
    "RULESUMMARIES",
    "CASEREPORT",
    "SAVE",
];

pub(crate) fn validatedata(p: &mut Parser) -> PResult<Command> {
    let begin = p.pos;
    let mut command = Validatedata {
        variables: Vec::new(),
        id: Vec::new(),
        rules: Vec::new(),
        var_checks: Some(VarChecks {
            pct_missing: 70.0,
            pct_equal: 95.0,
            pct_unequal: 90.0,
            cv: 0.001,
            std_dev: 0.0,
        }),
        incomplete_ids: true,
        duplicate_ids: true,
        report_empty: true,
        empty_scope: None,
        by_variable: true,
        by_rule: false,
        summaries_given: false,
        case_report: true,
        min_violations: 1,
        case_limit: Some(500),
        save: Vec::new(),
    };
    let mut seen = Vec::new();
    while !at_end(p) {
        let word = keyword(
            p,
            &["VARIABLES", "ID", "CROSSVARRULES"],
            &mut seen,
            "VALIDATEDATA",
        )?;
        p.expect(Punct::Eq)?;
        match word {
            "VARIABLES" => command.variables = names_until_keyword(p)?,
            "ID" => command.id = names_until_keyword(p)?,
            _ => command.rules = rule_list(p)?,
        }
    }
    if seen.is_empty() {
        return Err(error(
            p,
            begin,
            "VALIDATEDATA needs VARIABLES=, ID= or CROSSVARRULES=",
        ));
    }
    let mut subcommands = Vec::new();
    while p.eat(Punct::Slash) {
        let name = subcommand(p, "VALIDATEDATA", &VALIDATE_SUBCOMMANDS, &mut subcommands)?;
        if at_end(p) {
            return Err(error(p, p.pos - 1, format!("/{name} is empty")));
        }
        let place = format!("/{name}");
        let mut keywords = Vec::new();
        match name {
            "VARCHECKS" => var_checks(p, &mut command, &place)?,
            "IDCHECKS" => {
                let start = p.pos;
                command.incomplete_ids = false;
                command.duplicate_ids = false;
                while !at_end(p) {
                    match keyword(
                        p,
                        &["INCOMPLETE", "DUPLICATE", "NONE"],
                        &mut keywords,
                        &place,
                    )? {
                        "INCOMPLETE" => command.incomplete_ids = true,
                        "DUPLICATE" => command.duplicate_ids = true,
                        _ => {}
                    }
                    p.eat(Punct::Comma);
                }
                if keywords.contains(&"NONE") && keywords.len() > 1 {
                    return Err(error(p, start, "IDCHECKS NONE stands alone"));
                }
            }
            "CASECHECKS" => {
                while !at_end(p) {
                    let word = keyword(p, &["REPORTEMPTY", "SCOPE"], &mut keywords, &place)?;
                    p.expect(Punct::Eq)?;
                    if word == "REPORTEMPTY" {
                        command.report_empty = yes_no(p, false)?;
                    } else if p.eat_word("ANALYSISVARS") {
                        command.empty_scope = Some(EmptyScope::AnalysisVariables);
                    } else if p.eat_word("ALLVARS") {
                        command.empty_scope = Some(EmptyScope::AllVariables);
                    } else {
                        return Err(p.unexpected(&["ANALYSISVARS", "ALLVARS"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            "RULESUMMARIES" => {
                let start = p.pos;
                command.summaries_given = true;
                command.by_variable = false;
                while !at_end(p) {
                    match keyword(p, &["BYVARIABLE", "BYRULE", "NONE"], &mut keywords, &place)? {
                        "BYVARIABLE" => command.by_variable = true,
                        "BYRULE" => command.by_rule = true,
                        _ => {}
                    }
                    p.eat(Punct::Comma);
                }
                if keywords.contains(&"NONE") && keywords.len() > 1 {
                    return Err(error(p, start, "RULESUMMARIES NONE stands alone"));
                }
            }
            "CASEREPORT" => {
                const WORDS: [&str; 3] = ["DISPLAY", "MINVIOLATIONS", "CASELIMIT"];
                while !at_end(p) {
                    let word = keyword(p, &WORDS, &mut keywords, &place)?;
                    p.expect(Punct::Eq)?;
                    match word {
                        "DISPLAY" => command.case_report = yes_no(p, false)?,
                        "MINVIOLATIONS" => {
                            command.min_violations = whole(p, "a number of violations")?
                        }
                        _ if p.eat_word("NONE") => command.case_limit = None,
                        _ if p.eat_word("FIRSTN") => {
                            p.expect(Punct::LParen)?;
                            command.case_limit = Some(whole(p, "a number of cases")?);
                            p.expect(Punct::RParen)?;
                        }
                        _ => return Err(p.unexpected(&["FIRSTN(n)", "NONE"])),
                    }
                    p.eat(Punct::Comma);
                }
            }
            _ => {
                const WORDS: [(&str, ValidateSave); 4] = [
                    ("EMPTYCASE", ValidateSave::EmptyCase),
                    ("INCOMPLETEID", ValidateSave::IncompleteId),
                    ("DUPLICATEID", ValidateSave::DuplicateId),
                    ("RULEVIOLATIONS", ValidateSave::RuleViolations),
                ];
                let names: Vec<&str> = WORDS.iter().map(|w| w.0).collect();
                while !at_end(p) {
                    let word = keyword(p, &names, &mut keywords, &place)?;
                    let what = WORDS.iter().find(|w| w.0 == word).expect("listed").1;
                    let name = match p.eat(Punct::LParen) {
                        true => {
                            let name = p.name("a new variable name")?;
                            p.expect(Punct::RParen)?;
                            Some(name)
                        }
                        false => None,
                    };
                    command.save.push((what, name));
                    p.eat(Punct::Comma);
                }
            }
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Validatedata(Box::new(command)))
}

fn var_checks(p: &mut Parser, command: &mut Validatedata, place: &str) -> PResult<()> {
    const WORDS: [&str; 6] = [
        "STATUS",
        "PCTMISSING",
        "PCTEQUAL",
        "PCTUNEQUAL",
        "CV",
        "STDDEV",
    ];
    let mut keywords = Vec::new();
    let mut checks = command.var_checks.expect("set by default");
    let mut on = true;
    while !at_end(p) {
        let word = keyword(p, &WORDS, &mut keywords, place)?;
        p.expect(Punct::Eq)?;
        match word {
            // the description says NO while the syntax chart says OFF: both are taken
            "STATUS" => on = yes_no(p, true)?,
            "PCTMISSING" => checks.pct_missing = threshold(p, word, true)?,
            "PCTEQUAL" => checks.pct_equal = threshold(p, word, true)?,
            "PCTUNEQUAL" => checks.pct_unequal = threshold(p, word, true)?,
            "CV" => checks.cv = threshold(p, word, false)?,
            _ => checks.std_dev = threshold(p, word, false)?,
        }
        p.eat(Punct::Comma);
    }
    command.var_checks = on.then_some(checks);
    Ok(())
}
