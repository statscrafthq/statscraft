//! Result tables as an Excel workbook, for the command line, the window and OUTPUT EXPORT.
//! The number formats come from the `excel` module, which SAVE TRANSLATE shares.

use crate::excel::excel_number;
use crate::office::{Item, put_chart};
use crate::{Cell, CellLook, Decoration, Table, cell_text, header_rows, parse_colour};
use rust_xlsxwriter::{
    Color, Format, FormatAlign, FormatBorder, Note, Workbook, Worksheet, XlsxError,
};
use std::collections::HashMap;
use std::path::Path;

/// Columns never grow wider than this many characters; longer labels wrap.
const MAX_WIDTH: usize = 50;

/// Writes `items` to a new workbook at `path`: one worksheet per item named after its title,
/// or with `one_sheet` all of them on one sheet, one below the other with a blank row between.
/// A chart goes out as its data table with the office chart beside it.
pub fn write_xlsx<'a>(
    path: &Path,
    items: impl IntoIterator<Item = Item<'a>>,
    one_sheet: bool,
) -> Result<(), XlsxError> {
    write_xlsx_parts(path, items.into_iter().map(|item| (item, true)), one_sheet)
}

/// `write_xlsx` with each item's notes left out when its bool is false. A table with an empty
/// title has no title row either.
pub fn write_xlsx_parts<'a>(
    path: &Path,
    items: impl IntoIterator<Item = (Item<'a>, bool)>,
    one_sheet: bool,
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    if one_sheet {
        // no frozen panes here: every table has its own header
        let sheet = workbook.add_worksheet().set_name("Tables")?;
        let (mut top, mut widths) = (0, Vec::new());
        for (item, notes) in items {
            top = put_item(sheet, item, (top, 0), notes, &mut widths)? + 1;
        }
        set_widths(sheet, &widths)?;
    } else {
        let mut names = Vec::new();
        for (item, notes) in items {
            let sheet = workbook.add_worksheet();
            let owned;
            let table = match item {
                Item::Table(table) => table,
                Item::Chart(chart) => {
                    owned = chart.data_table();
                    &owned
                }
            };
            // a table without its title is named by its outline label, when it has one
            let name = match (&table.outline, table.title.as_str()) {
                (Some(outline), "") => outline,
                _ => &table.title,
            };
            sheet.set_name(sheet_name(name, &mut names))?;
            let mut widths = Vec::new();
            put_item(sheet, item, (0, 0), notes, &mut widths)?;
            set_widths(sheet, &widths)?;
            let depth = header_rows(table).len() as u32;
            // stacked block labels run on to the right, where a frozen column would cut them
            let columns = if table.stacked() {
                0
            } else {
                table.row_headers
            };
            sheet.set_freeze_panes(title_rows(table) + depth, columns as u16)?;
        }
    }
    workbook.save(path)
}

/// A table, or a chart with its data table, with its notes when `notes`, from `at`; returns the
/// row after it.
fn put_item(
    sheet: &mut Worksheet,
    item: Item,
    at: (u32, u16),
    notes: bool,
    widths: &mut Vec<f64>,
) -> Result<u32, XlsxError> {
    match item {
        Item::Table(table) => put_table(sheet, table, at, notes, widths),
        Item::Chart(chart) => put_chart(sheet, &chart.data_table(), chart, at, notes, widths, true),
    }
}

/// The rows a table's title takes: none for an empty title.
pub(crate) fn title_rows(table: &Table) -> u32 {
    u32::from(!table.title.is_empty())
}

/// The base formats of a table's cells, by their place in `Formats::bases`.
const HEADER: usize = 0;
const ROW_LABEL: usize = 1;
const TEXT: usize = 2;
const NUMBER: usize = 3;

/// The formats of a table's cells: each base with a number format and a plugin's look is made
/// once, so thousands of cells that look alike share one format.
struct Formats<'a> {
    bases: [Format; 4],
    made: HashMap<(usize, String, Option<CellLook<'a>>), Format>,
}

impl<'a> Formats<'a> {
    fn get(&mut self, base: usize, num_format: String, look: Option<&'a Decoration>) -> &Format {
        let bases = &self.bases;
        let key = (base, num_format, look.map(Decoration::look));
        self.made
            .entry(key)
            .or_insert_with_key(|(_, num_format, _)| {
                let mut format = bases[base].clone();
                if !num_format.is_empty() {
                    format = format.set_num_format(num_format);
                }
                match look {
                    Some(d) => paint(format, d),
                    None => format,
                }
            })
    }
}

/// A plugin's look over a cell's own format: fill, text colour, bold, italic, alignment and
/// borders.
fn paint(format: Format, d: &Decoration) -> Format {
    let colour = |c: &Option<String>| {
        let [r, g, b] = c.as_deref().and_then(parse_colour)?;
        Some(Color::RGB(u32::from_be_bytes([0, r, g, b])))
    };
    let mut format = format;
    if let Some(fill) = colour(&d.fill) {
        format = format.set_background_color(fill);
    }
    if let Some(text) = colour(&d.text) {
        format = format.set_font_color(text);
    }
    if d.bold {
        format = format.set_bold();
    }
    if d.italic {
        format = format.set_italic();
    }
    match d.align.as_deref() {
        Some("left") => format = format.set_align(FormatAlign::Left),
        Some("center") => format = format.set_align(FormatAlign::Center),
        Some("right") => format = format.set_align(FormatAlign::Right),
        _ => {}
    }
    if let Some(b) = &d.border {
        if let Some(c) = colour(&b.top) {
            format = format
                .set_border_top(FormatBorder::Thin)
                .set_border_top_color(c);
        }
        if let Some(c) = colour(&b.bottom) {
            format = format
                .set_border_bottom(FormatBorder::Thin)
                .set_border_bottom_color(c);
        }
        if let Some(c) = colour(&b.left) {
            format = format
                .set_border_left(FormatBorder::Thin)
                .set_border_left_color(c);
        }
        if let Some(c) = colour(&b.right) {
            format = format
                .set_border_right(FormatBorder::Thin)
                .set_border_right_color(c);
        }
    }
    format
}

/// Writes the title, header, body and, with `notes`, the caption of `table` from the cell
/// `(top, left)` and returns the row after the last one written. `widths` grows to the
/// Excel width each column of the sheet needs.
///
/// With stacked row labels (`Table::stacked`) the outer row label column is left out, every
/// other column moves one to the left, and each block of rows gets a row above it holding its
/// outer label in the first column, styled and commented as the block's first row in column 0
/// was; the label is not wrapped, so it runs on over the empty cells to its right.
pub(crate) fn put_table(
    sheet: &mut Worksheet,
    table: &Table,
    (top, left): (u32, u16),
    notes: bool,
    widths: &mut Vec<f64>,
) -> Result<u32, XlsxError> {
    // the plugin's font, if any, for every cell of the table
    let font = |format: Format| {
        let format = match &table.style.font {
            Some(name) => format.set_font_name(name),
            None => format,
        };
        match table.style.font_size {
            Some(size) => format.set_font_size(size),
            None => format,
        }
    };
    let plain = table.style.plain;
    let border = match plain {
        true => font(Format::new()),
        false => font(Format::new())
            .set_border(FormatBorder::Thin)
            .set_border_color(Color::RGB(0xA6A6A6)),
    };
    let header = border
        .clone()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let header = match plain {
        true => header,
        false => header.set_bold().set_background_color(Color::RGB(0xF2F2F2)),
    };
    let row_header = border.clone().set_align(FormatAlign::Top).set_text_wrap();
    let text = border.clone().set_align(FormatAlign::Right);
    let mut formats = Formats {
        bases: [header, row_header, text, border],
        made: HashMap::new(),
    };
    // table column `c` from `shift` on goes to the sheet column `left + c - shift`
    let shift = usize::from(table.stacked());
    let column_of = |c: usize| left + (c - shift) as u16;
    // the characters each column of the table needs
    let mut own = vec![0; table.columns.len() - shift];
    let fit = |own: &mut Vec<usize>, c: usize, text: &str| {
        own[c - shift] = own[c - shift].max(text.chars().count().min(MAX_WIDTH));
    };

    if !table.title.is_empty() {
        sheet.write_string_with_format(top, left, &table.title, &font(Format::new().set_bold()))?;
    }
    let mut row = top + title_rows(table);
    let mut spans = Vec::new();
    let headers = header_rows(table);
    let style = |line: usize, c: usize| table.header_decoration(line, c);
    // above each row label column the header levels form one cell holding its label, if any
    let depth = headers.len() as u32;
    for c in shift..table.row_headers {
        let label: Vec<&str> = headers
            .iter()
            .flatten()
            .filter(|cell| cell.first == c && !cell.label.is_empty())
            .map(|cell| cell.label.as_str())
            .collect();
        let label = label.join(" ");
        let column = column_of(c);
        let format = formats.get(HEADER, String::new(), style(0, c));
        if depth > 1 {
            sheet.merge_range(row, column, row + depth - 1, column, &label, format)?;
            // each line keeps its own borders under the merged cell
            for line in 1..headers.len() {
                if let Some(d) = style(line, c) {
                    let format = formats.get(HEADER, String::new(), Some(d));
                    sheet.write_blank(row + line as u32, column, format)?;
                }
            }
        } else {
            sheet.write_string_with_format(row, column, &label, format)?;
        }
        fit(&mut own, c, &label);
    }
    for (line, cells) in headers.iter().enumerate() {
        for cell in cells.iter().filter(|cell| cell.first >= table.row_headers) {
            let column = column_of(cell.first);
            let format = formats.get(HEADER, String::new(), style(line, cell.first));
            if cell.span > 1 {
                let last = column + cell.span as u16 - 1;
                sheet.merge_range(row, column, row, last, &cell.label, format)?;
                for c in cell.first + 1..cell.first + cell.span {
                    if let Some(d) = style(line, c) {
                        let format = formats.get(HEADER, String::new(), Some(d));
                        sheet.write_blank(row, column_of(c), format)?;
                    }
                }
                spans.push(cell.clone());
            } else {
                sheet.write_string_with_format(row, column, &cell.label, format)?;
                fit(&mut own, cell.first, &cell.label);
            }
        }
        row += 1;
    }
    // the sheet row of every body row, below the label row of its block when stacked
    let starts = table.block_starts();
    let mut body_rows = Vec::with_capacity(table.rows.len());
    for r in 0..table.rows.len() {
        if starts.binary_search(&r).is_ok() {
            row += 1;
        }
        body_rows.push(row);
        row += 1;
    }
    let format = |r: usize, c: usize| {
        let format = table.formats.get(r).and_then(|f| f.get(c));
        format.map_or("", String::as_str)
    };
    // one note reused for every plugin comment, its text replaced
    let mut note = Note::new("").add_author_prefix(false).set_width(260);
    let mut comment = |sheet: &mut Worksheet, at: u32, column: u16, d: Option<&Decoration>| {
        let Some(comment) = d.and_then(|d| d.comment.as_deref()) else {
            return Ok(());
        };
        let lines = comment.lines().count().max(1) as u32;
        note.reset_text(comment);
        let sized = note.clone().set_height(16 * lines + 12);
        sheet.insert_note(at, column, &sized).map(|_| ())
    };
    for &r in &starts {
        // the base without wrap, so the label runs on over the empty cells to its right
        let decoration = table.decoration(r, 0);
        let label = formats.get(NUMBER, String::new(), decoration);
        let at = body_rows[r] - 1;
        sheet.write_string_with_format(at, left, cell_text(table, r, 0), label)?;
        comment(sheet, at, left, decoration)?;
    }
    for (r, cells) in table.rows.iter().enumerate() {
        let at = body_rows[r];
        for (c, cell) in cells.iter().enumerate().skip(shift) {
            let column = column_of(c);
            let shown = cell_text(table, r, c);
            fit(&mut own, c, &shown);
            let decoration = table.decoration(r, c);
            comment(sheet, at, column, decoration)?;
            if c < table.row_headers {
                // row labels are written below, merged over the rows that repeat them
                continue;
            }
            match cell {
                Cell::Number(n) if n.is_finite() => match excel_number(format(r, c), *n, &shown) {
                    Some((value, num_format)) => {
                        let number = formats.get(NUMBER, num_format, decoration);
                        sheet.write_number_with_format(at, column, value, number)?
                    }
                    None => {
                        let text = formats.get(TEXT, String::new(), decoration);
                        sheet.write_string_with_format(at, column, &shown, text)?
                    }
                },
                Cell::Empty => {
                    sheet.write_blank(at, column, formats.get(TEXT, String::new(), decoration))?
                }
                _ => {
                    let text = formats.get(TEXT, String::new(), decoration);
                    sheet.write_string_with_format(at, column, &shown, text)?
                }
            };
        }
    }
    put_row_labels(sheet, table, &body_rows, column_of, &mut formats)?;
    // a merged label longer than its columns widens each of them by an equal share
    for cell in spans {
        let columns = cell.first - shift..cell.first - shift + cell.span;
        let room = own[columns.clone()].iter().sum::<usize>() + cell.span - 1;
        let need = cell.label.chars().count().min(MAX_WIDTH);
        let share = need.saturating_sub(room).div_ceil(cell.span);
        own[columns].iter_mut().for_each(|w| *w += share);
    }
    let at = usize::from(left);
    if widths.len() < at + own.len() {
        widths.resize(at + own.len(), 0.0);
    }
    // a little room for the cell padding and the bold header font
    let mut own: Vec<f64> = own.iter().map(|&chars| chars as f64 * 1.1 + 2.0).collect();
    // the plugin's widths win over what the cells need
    for w in table.style.column_widths.iter().filter(|w| w.col >= shift) {
        own[w.col - shift] = w.width;
    }
    for (c, width) in own.into_iter().enumerate() {
        widths[at + c] = widths[at + c].max(width);
    }
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        sheet.write_string_with_format(row, left, caption, &font(Format::new().set_italic()))?;
        row += 1;
    }
    Ok(row)
}

/// Writes the row label columns, merging a label down over the rows that repeat it under the
/// same outer labels, so a nested variable's label appears once beside its categories. Body
/// row `r` goes to the sheet row `rows[r]` and table column `c` to `column_of(c)`; with
/// stacked labels the outer column is left out.
fn put_row_labels<'a>(
    sheet: &mut Worksheet,
    table: &'a Table,
    rows: &[u32],
    column_of: impl Fn(usize) -> u16,
    formats: &mut Formats<'a>,
) -> Result<(), XlsxError> {
    let labels: Vec<Vec<String>> = (0..table.rows.len())
        .map(|r| {
            (0..table.row_headers)
                .map(|c| cell_text(table, r, c))
                .collect()
        })
        .collect();
    for c in usize::from(table.stacked())..table.row_headers {
        let mut first = 0;
        while first < labels.len() {
            let same = |r: usize| labels[r][..=c] == labels[first][..=c];
            let last = (first..labels.len())
                .take_while(|&r| same(r))
                .last()
                .unwrap_or(first);
            // a run shares its outer label, so no block label row comes between its rows
            let (from, to, column) = (rows[first], rows[last], column_of(c));
            if last > first && !labels[first][c].is_empty() {
                let format = formats.get(ROW_LABEL, String::new(), table.decoration(first, c));
                sheet.merge_range(from, column, to, column, &labels[first][c], format)?;
                // each row keeps its own borders under the merged cell
                for (r, &at) in rows.iter().enumerate().take(last + 1).skip(first + 1) {
                    if let Some(d) = table.decoration(r, c) {
                        let format = formats.get(ROW_LABEL, String::new(), Some(d));
                        sheet.write_blank(at, column, format)?;
                    }
                }
            } else {
                for (r, row) in labels.iter().enumerate().take(last + 1).skip(first) {
                    let format = formats.get(ROW_LABEL, String::new(), table.decoration(r, c));
                    sheet.write_string_with_format(rows[r], column, &row[c], format)?;
                }
            }
            first = last + 1;
        }
    }
    Ok(())
}

/// Sets every column of the sheet to the width `put_table` worked out for it.
pub(crate) fn set_widths(sheet: &mut Worksheet, widths: &[f64]) -> Result<(), XlsxError> {
    for (c, width) in widths.iter().enumerate() {
        sheet.set_column_width(c as u16, *width)?;
    }
    Ok(())
}

/// A worksheet name from a table title under Excel's rules: at most 31 characters, none of
/// [ ] : * ? / \, no apostrophe at either end, not "History", and unique ignoring case among
/// `used`, to which it is added.
pub(crate) fn sheet_name(title: &str, used: &mut Vec<String>) -> String {
    let clean: String = title
        .chars()
        .map(|c| match c {
            '[' | ']' | ':' | '*' | '?' | '/' | '\\' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_matches(|c| c == '\'' || c == ' ');
    let base = if clean.is_empty() { "Table" } else { clean };
    for n in 1.. {
        let suffix = if n == 1 {
            String::new()
        } else {
            format!(" ({n})")
        };
        let mut room = 31 - suffix.len();
        let prefix: String = base
            .chars()
            .take_while(|c| match room.checked_sub(c.len_utf16()) {
                Some(left) => {
                    room = left;
                    true
                }
                None => false,
            })
            .collect();
        let name = format!("{}{suffix}", prefix.trim_end_matches([' ', '\'']));
        let taken = |other: &String| other.to_lowercase() == name.to_lowercase();
        if !name.eq_ignore_ascii_case("history") && !used.iter().any(taken) {
            used.push(name.clone());
            return name;
        }
    }
    unreachable!("some suffix is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_names_follow_excel_rules() {
        let mut used = Vec::new();
        let mut name = |title: &str| sheet_name(title, &mut used);
        assert_eq!(name("Gender"), "Gender");
        assert_eq!(name("gender"), "gender (2)");
        assert_eq!(name("GENDER"), "GENDER (3)");
        assert_eq!(
            name("Chi-Square Tests: a/b [c]*?\\"),
            "Chi-Square Tests a b c"
        );
        assert_eq!(name("'quoted'"), "quoted");
        assert_eq!(name(" :: "), "Table");
        assert_eq!(name(""), "Table (2)");
        assert_eq!(name("History"), "History (2)");
        let long = "Q1. Overall, how satisfied are you with our service?";
        assert_eq!(name(long), "Q1. Overall, how satisfied are");
        assert_eq!(name(long), "Q1. Overall, how satisfied (2)");
        assert_eq!(name(long), "Q1. Overall, how satisfied (3)");
        // Excel counts UTF-16 units, so a character outside the basic plane counts twice
        let wide = name(&"\u{1F600}".repeat(20));
        assert_eq!(wide.encode_utf16().count(), 30);
        for name in &used {
            assert!(name.encode_utf16().count() <= 31, "{name}");
        }
    }
}
