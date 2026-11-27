//! DATA LIST, BEGIN DATA and END DATA: text data, inline or from a file.

use crate::ast::{Command, DataField, DataLayout, DataList, Format, FormatType, KeyedDataList};
use crate::commands::format;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// The display format of a numeric input format: room for the decimal point, grouping
/// commas and the dollar or percent sign that the data need not contain.
pub(crate) fn display_format(format: Format) -> Format {
    let (w, d) = (u32::from(format.width), u32::from(format.decimals));
    let point = u32::from(d > 0);
    let groups = w.saturating_sub(d).saturating_sub(1) / 3;
    let (kind, width, decimals) = match format.kind {
        FormatType::F | FormatType::N => (FormatType::F, w + point, d),
        FormatType::E => (FormatType::E, w.max(10), d.max(3)),
        FormatType::Comma | FormatType::Dot => (format.kind, w + groups + point, d),
        FormatType::Dollar => (format.kind, w + groups + point + 1, d),
        FormatType::Pct => (format.kind, w + point + 1, d),
        _ => return format,
    };
    Format {
        kind,
        width: width.min(40) as u16,
        decimals: decimals as u8,
    }
}

/// The input formats DATA LIST reads.
const INPUT_FORMATS: [FormatType; 23] = [
    FormatType::F,
    FormatType::N,
    FormatType::E,
    FormatType::Comma,
    FormatType::Dot,
    FormatType::Dollar,
    FormatType::Pct,
    FormatType::Z,
    FormatType::Pibhex,
    FormatType::A,
    FormatType::Date,
    FormatType::Adate,
    FormatType::Edate,
    FormatType::Sdate,
    FormatType::Jdate,
    FormatType::Qyr,
    FormatType::Moyr,
    FormatType::Wkyr,
    FormatType::Datetime,
    FormatType::Ymdhms,
    FormatType::Time,
    FormatType::Mtime,
    FormatType::Dtime,
];

/// Names for new variables: `v1 TO v3` counts the number at the end of the name up, keeping
/// leading zeros (`v01 TO v10`). At most 10000 names.
pub fn numbered_names(first: &str, last: &str) -> Result<Vec<String>, String> {
    const MAX: u64 = 10_000;
    fn split(name: &str) -> (&str, &str) {
        let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
        name.split_at(stem.len())
    }
    let ((stem, from), (last_stem, to)) = (split(first), split(last));
    match (from.parse::<u64>(), to.parse::<u64>()) {
        (Ok(low), Ok(high))
            if stem.eq_ignore_ascii_case(last_stem) && low <= high && high - low < MAX =>
        {
            let width = from.len();
            Ok((low..=high).map(|n| format!("{stem}{n:0width$}")).collect())
        }
        _ => Err(format!(
            "{first} TO {last} cannot name new variables: both names need the same stem and a \
             number at the end, as in v1 TO v5 (at most {MAX} names)"
        )),
    }
}

/// A positive whole number.
pub(crate) fn count(p: &mut Parser, what: &str) -> PResult<u32> {
    match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= 1e9 => {
            let n = *n as u32;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

/// `name [TO name] ...` up to something that is not a name.
fn names(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        let start = p.pos;
        let first = p.name("a variable name")?;
        if p.eat_word("TO") {
            let last = p.name("a variable name")?;
            let numbered = numbered_names(&first, &last)
                .map_err(|message| SyntaxError::new(p.span_from(start), message))?;
            names.extend(numbered);
        } else {
            names.push(first);
        }
        p.eat(Punct::Comma);
    }
    if names.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(names)
}

/// A format DATA LIST can read, from its text.
fn input_format(p: &Parser, text: &str) -> PResult<Format> {
    let parsed = format(text).map_err(|message| SyntaxError::new(p.span(), message))?;
    if !INPUT_FORMATS.contains(&parsed.kind) {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "DATA LIST cannot read format {} yet",
                parsed.kind.name().to_uppercase()
            ),
        )
        .hint(
            "supported: F, N, E, COMMA, DOT, DOLLAR, PCT, Z, PIBHEX, A, DATE, ADATE, EDATE, SDATE, JDATE, QYR, \
             MOYR, WKYR, DATETIME, YMDHMS, TIME, MTIME, DTIME",
        ));
    }
    if parsed.width == 0 || parsed.width > 32767 {
        return Err(SyntaxError::new(
            p.span(),
            "a format needs a width of 1 or more",
        ));
    }
    if parsed.kind == FormatType::A && parsed.decimals > 0 {
        return Err(SyntaxError::new(p.span(), "an A format has no decimals"));
    }
    if u16::from(parsed.decimals) > parsed.width.min(16) {
        return Err(SyntaxError::new(
            p.span(),
            format!("format {text} has more decimals than its width allows"),
        ));
    }
    Ok(parsed)
}

/// One item of a parenthesized format list.
#[derive(Clone)]
enum Item {
    /// A format, repeated.
    Format(u32, Format),
    /// `Tn`: go to column n.
    Tab(u32),
    /// `nX`: skip n columns.
    Skip(u32),
    /// `/`: the next record.
    Record,
}

/// Groups inside groups that a format list may have, and formats it may expand to: a repeat of a
/// repeat of a repeat would otherwise fill the memory, and a long run of parentheses the stack.
/// PRINT and WRITE keep their lists within the same limits.
pub(crate) const MAX_DEPTH: usize = 32;
pub(crate) const MAX_ITEMS: usize = 1_000_000;

pub(crate) fn too_deep(p: &Parser) -> SyntaxError {
    SyntaxError::new(
        p.span(),
        format!("format groups are nested more than {MAX_DEPTH} deep"),
    )
}

pub(crate) fn too_long(p: &Parser) -> SyntaxError {
    SyntaxError::new(
        p.span(),
        format!("a format list holds at most {MAX_ITEMS} formats"),
    )
}

/// The formats an item stands for once its repeat is expanded.
fn weight(item: &Item) -> usize {
    match item {
        Item::Format(n, _) => *n as usize,
        _ => 1,
    }
}

/// `(item, item ...)` after the opening parenthesis, up to and including the closing one.
fn format_items(p: &mut Parser, fixed: bool, depth: usize) -> PResult<Vec<Item>> {
    if depth > MAX_DEPTH {
        return Err(too_deep(p));
    }
    let mut items = Vec::new();
    // the formats the list expands to so far
    let mut total = 0_usize;
    while !p.eat(Punct::RParen) {
        if p.eat(Punct::Comma) {
            continue;
        }
        if fixed && p.eat(Punct::Slash) {
            items.push(Item::Record);
            continue;
        }
        let expected = ["a format such as F8.2", "')'"];
        let repeat = match p.peek() {
            Some(TokenKind::Number(_)) => Some(count(p, "a number")?),
            _ => None,
        };
        // n(...) repeats a group of items
        if p.eat(Punct::LParen) {
            let group = format_items(p, fixed, depth + 1)?;
            let group_weight: usize = group.iter().map(weight).sum();
            // an empty group, however often it is repeated, adds nothing
            if group_weight == 0 {
                continue;
            }
            for _ in 0..repeat.unwrap_or(1) {
                total = total.saturating_add(group_weight);
                if total > MAX_ITEMS {
                    return Err(too_long(p));
                }
                items.extend(group.iter().cloned());
            }
            continue;
        }
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&expected));
        };
        let upper = word.to_uppercase();
        let digits = upper.len() - upper.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let (glued, rest) = upper.split_at(digits);
        let number = |text: &str| text.parse::<u32>().ok().filter(|n| *n >= 1);
        let item = match (rest, number(glued).or(repeat)) {
            ("X", Some(n)) if fixed => Item::Skip(n),
            (tab, None) if fixed && tab.starts_with('T') && number(&tab[1..]).is_some() => {
                Item::Tab(number(&tab[1..]).expect("checked"))
            }
            (text, n) => {
                let format = input_format(p, text)?;
                pibhex_width(p, format.kind, format.width)?;
                Item::Format(n.unwrap_or(1), format)
            }
        };
        p.pos += 1;
        total = total.saturating_add(weight(&item));
        if total > MAX_ITEMS {
            return Err(too_long(p));
        }
        items.push(item);
    }
    Ok(items)
}

/// PIBHEX is read in whole bytes, two hexadecimal digits each, at most eight of them.
fn pibhex_width(p: &Parser, kind: FormatType, width: u16) -> PResult<()> {
    if kind == FormatType::Pibhex && (!width.is_multiple_of(2) || width > 16) {
        return Err(SyntaxError::new(
            p.span(),
            "PIBHEX needs an even width of at most 16",
        ));
    }
    Ok(())
}

/// Column-style format after the columns: `(A)`, `(F,2)`, `(DOLLAR)`, `(2)`.
pub(crate) fn column_format(p: &mut Parser, width: u16) -> PResult<Format> {
    let mut kind = FormatType::F;
    let mut decimals = 0;
    if p.eat(Punct::LParen) {
        if let Some(TokenKind::Word(word)) = p.peek() {
            kind = input_format(p, &format!("{word}1"))?.kind;
            p.pos += 1;
            p.eat(Punct::Comma);
        }
        if let Some(TokenKind::Number(n)) = p.peek() {
            if kind == FormatType::A || n.fract() != 0.0 || *n > 16.0 {
                return Err(p.unexpected(&["')'"]));
            }
            decimals = *n as u8;
            p.pos += 1;
        }
        p.expect(Punct::RParen)?;
    }
    if u16::from(decimals) > width {
        return Err(SyntaxError::new(
            p.span(),
            "more decimals than the columns hold",
        ));
    }
    pibhex_width(p, kind, width)?;
    Ok(Format {
        kind,
        width,
        decimals,
    })
}

/// The variables of fixed-format data: per record, names with columns or with formats.
/// `record` is 0 when a slash must open the first record. Also returns the last column any
/// variable or skip reaches.
pub(crate) fn fixed_fields(
    p: &mut Parser,
    records: u32,
    mut record: u32,
) -> PResult<(Vec<DataField>, u32)> {
    let mut fields = Vec::new();
    // where a format without a column starts: after the previous field of the record
    let mut cursor = 1;
    let mut reach = 0;
    let too_many = |p: &Parser, record: u32| {
        SyntaxError::new(
            p.span(),
            format!("record {record} is past RECORDS={records}"),
        )
        .hint("raise RECORDS to the number of lines that make one case")
    };
    while p.peek().is_some() {
        if p.eat(Punct::Slash) {
            record = match p.peek() {
                Some(TokenKind::Number(_)) => {
                    let start = p.pos;
                    let number = count(p, "a record number")?;
                    if number <= record {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "records must be given in increasing order",
                        ));
                    }
                    number
                }
                _ => record + 1,
            };
            cursor = 1;
            if record > records {
                return Err(too_many(p, record));
            }
            continue;
        }
        if record == 0 {
            return Err(p.unexpected(&["'/'"]));
        }
        let names = names(p)?;
        if let Some(TokenKind::Number(_)) = p.peek() {
            let start = p.pos;
            let first = count(p, "a column")?;
            let last = if p.eat(Punct::Minus) {
                count(p, "a column")?
            } else {
                first
            };
            let columns = (last + 1).saturating_sub(first);
            let share = columns / names.len() as u32;
            if last < first || share == 0 || columns % names.len() as u32 != 0 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!(
                        "columns {first}-{last} cannot be shared equally by {} variables",
                        names.len()
                    ),
                ));
            }
            let format = column_format(p, share.min(32767) as u16)?;
            for (i, name) in names.into_iter().enumerate() {
                fields.push(DataField {
                    name,
                    format,
                    display: display_format(format),
                    record,
                    column: first + i as u32 * share,
                });
            }
            cursor = last + 1;
            reach = reach.max(last);
        } else if p.eat(Punct::LParen) {
            let start = p.pos;
            let items = format_items(p, true, 0)?;
            let mut names = names.into_iter();
            for item in items {
                match item {
                    Item::Tab(column) => cursor = column,
                    Item::Skip(n) => cursor += n,
                    Item::Record => {
                        record += 1;
                        cursor = 1;
                        if record > records {
                            return Err(too_many(p, record));
                        }
                    }
                    Item::Format(repeat, format) => {
                        for _ in 0..repeat {
                            let Some(name) = names.next() else {
                                return Err(SyntaxError::new(
                                    p.span_from(start),
                                    "more formats than variables",
                                ));
                            };
                            fields.push(DataField {
                                name,
                                format,
                                display: display_format(format),
                                record,
                                column: cursor,
                            });
                            cursor += u32::from(format.width);
                        }
                    }
                }
                reach = reach.max(cursor - 1);
            }
            if names.next().is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "fewer formats than variables: give one format per variable",
                ));
            }
        } else {
            return Err(p.unexpected(&["a column such as 1-3", "'(' and formats"]));
        }
    }
    Ok((fields, reach))
}

/// The variables of freefield data: names, each group followed by formats, `*` or nothing.
fn free_fields(p: &mut Parser) -> PResult<Vec<DataField>> {
    let default = Format {
        kind: FormatType::F,
        width: 8,
        decimals: 2,
    };
    let mut fields = Vec::new();
    // the default is a display format already
    let mut add = |name: String, format: Format, given: bool| {
        let display = match given {
            true => display_format(format),
            false => format,
        };
        fields.push(DataField {
            name,
            format,
            display,
            record: 1,
            column: 0,
        })
    };
    while p.peek().is_some() {
        let names = names(p)?;
        if p.eat(Punct::LParen) {
            let start = p.pos;
            let mut formats = Vec::new();
            for item in format_items(p, false, 0)? {
                if let Item::Format(repeat, format) = item {
                    formats.extend(std::iter::repeat_n(format, repeat as usize));
                }
            }
            match formats.len() {
                1 => names
                    .into_iter()
                    .for_each(|name| add(name, formats[0], true)),
                n if n == names.len() => names
                    .into_iter()
                    .zip(formats)
                    .for_each(|(n, f)| add(n, f, true)),
                _ => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!(
                            "{} variables but {} formats: give one format, or one per variable; \
                             end a run of default formats with *",
                            names.len(),
                            formats.len()
                        ),
                    ));
                }
            }
        } else {
            p.eat(Punct::Star);
            names.into_iter().for_each(|name| add(name, default, false));
        }
    }
    Ok(fields)
}

pub(crate) fn data_list(p: &mut Parser) -> PResult<Command> {
    let mut file = None;
    let mut encoding = None;
    let mut layout = None;
    let mut delimiters = Vec::new();
    let mut records = None;
    let mut skip = 0;
    let mut end = None;
    while !p.at(Punct::Slash) {
        if p.eat(Punct::Comma) {
            continue;
        }
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            file = Some(p.file("a quoted file path or a file handle")?);
        } else if p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            encoding = Some(p.string("a quoted encoding")?);
        } else if p.eat_keyword("FIXED") {
            layout = Some(DataLayout::Fixed { records: 1 });
        } else if p.eat_keyword("FREE") || p.at_keyword("LIST") {
            layout = Some(if p.eat_keyword("LIST") {
                DataLayout::List
            } else {
                DataLayout::Free
            });
            if p.eat(Punct::LParen) {
                while !p.eat(Punct::RParen) {
                    if p.eat(Punct::Comma) {
                        continue;
                    }
                    if p.eat_keyword("TAB") {
                        delimiters.push('\t');
                    } else {
                        delimiters.extend(p.string("a quoted delimiter or TAB")?.chars());
                    }
                }
            }
        } else if p.eat_keyword("RECORDS") {
            p.eat(Punct::Eq);
            records = Some(count(p, "the number of records per case")?);
        } else if p.eat_keyword("SKIP") {
            p.eat(Punct::Eq);
            skip = count(p, "the number of records to skip")?;
        } else if p.eat_keyword("TABLE") || p.eat_keyword("NOTABLE") {
        } else if p.eat_word("END") {
            p.eat(Punct::Eq);
            end = Some(p.name("a variable name")?);
        } else {
            return Err(p.unexpected(&[
                "FILE", "FIXED", "FREE", "LIST", "RECORDS", "SKIP", "END", "TABLE", "NOTABLE",
                "'/'",
            ]));
        }
    }
    let layout = match layout.unwrap_or(DataLayout::Fixed { records: 1 }) {
        DataLayout::Fixed { .. } => DataLayout::Fixed {
            records: records.unwrap_or(1),
        },
        _ if records.is_some() => {
            return Err(SyntaxError::new(
                p.span(),
                "RECORDS is for fixed-format data only",
            ));
        }
        free => free,
    };
    if end.is_some() && !matches!(layout, DataLayout::Fixed { .. }) {
        return Err(SyntaxError::new(
            p.span(),
            "END is for fixed-format data only",
        ));
    }
    let fields = match layout {
        DataLayout::Fixed { records } => fixed_fields(p, records, 0)?.0,
        _ => {
            p.expect(Punct::Slash)?;
            free_fields(p)?
        }
    };
    let mut seen: Vec<String> = Vec::new();
    for field in &fields {
        let lower = field.name.to_lowercase();
        if seen.contains(&lower) {
            return Err(SyntaxError::new(
                p.span(),
                format!("variable {} is defined twice", field.name),
            ));
        }
        if field.name.starts_with('$') {
            return Err(SyntaxError::new(
                p.span(),
                format!(
                    "{} cannot be defined on DATA LIST: it is a system variable",
                    field.name
                ),
            ));
        }
        seen.push(lower);
    }
    // fixed data without variables only passes over records, in an input program
    if fields.is_empty() && !matches!(layout, DataLayout::Fixed { .. }) {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(Command::DataList(DataList {
        file,
        layout,
        delimiters,
        skip,
        end,
        encoding,
        fields,
    }))
}

/// The splitter hands over the data lines as one string token each.
pub(crate) fn begin_data(p: &mut Parser) -> PResult<Command> {
    let mut lines = Vec::new();
    while let Some(TokenKind::Str(text)) = p.peek() {
        lines.push((p.span().line, text.clone()));
        p.pos += 1;
    }
    p.finish(&[])?;
    Ok(Command::BeginData(lines))
}

/// KEYED DATA LIST: FILE, KEY and IN (and TABLE, NOTABLE, ENCODING) in any order, then the
/// definitions of the variables of the one record.
pub(crate) fn keyed_data_list(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let (mut file, mut key, mut found, mut encoding) = (None, None, None, None);
    while !p.at(Punct::Slash) && p.peek().is_some() {
        if p.eat(Punct::Comma) {
            continue;
        }
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            file = Some(p.file("a quoted file path or a file handle")?);
        } else if p.eat_keyword("KEY") {
            p.eat(Punct::Eq);
            key = Some(p.name("the key variable")?);
        } else if p.eat_keyword("IN") {
            p.eat(Punct::Eq);
            found = Some(p.name("a variable name")?);
        } else if p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            encoding = Some(p.string("a quoted encoding")?);
        } else if p.eat_keyword("TABLE") || p.eat_keyword("NOTABLE") {
        } else {
            return Err(p.unexpected(&["FILE", "KEY", "IN", "TABLE", "NOTABLE", "ENCODING", "'/'"]));
        }
    }
    let span = p.span_from(start);
    let missing = |what: &str| SyntaxError::new(span, format!("KEYED DATA LIST needs {what}"));
    let Some(file) = file else {
        return Err(missing("FILE=handle"));
    };
    let Some(key) = key else {
        return Err(missing("KEY=variable"));
    };
    let Some(found) = found else {
        return Err(missing("IN=variable"));
    };
    if !p.at(Punct::Slash) {
        return Err(p.unexpected(&["'/'"]));
    }
    let (fields, _) = fixed_fields(p, 1, 0)?;
    if fields.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    let mut seen: Vec<String> = vec![key.to_lowercase(), found.to_lowercase()];
    if seen[0] == seen[1] {
        return Err(missing("a KEY variable other than the IN variable"));
    }
    for field in &fields {
        let lower = field.name.to_lowercase();
        if seen[1..].contains(&lower) || lower == seen[0] {
            return Err(SyntaxError::new(
                p.span(),
                format!("variable {} is named twice", field.name),
            ));
        }
        seen.push(lower);
    }
    Ok(Command::KeyedDataList(KeyedDataList {
        file,
        key,
        found,
        encoding,
        fields,
    }))
}
