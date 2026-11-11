//! Result tables as an Excel workbook, for the command line and the window alike. The number
//! formats come from the engine's `excel` module, which SAVE TRANSLATE shares.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};
use statscraft_engine::{Cell, Table, cell_text, excel::excel_number, header_rows};
use std::path::Path;

/// Columns never grow wider than this many characters; longer labels wrap.
const MAX_WIDTH: usize = 50;

/// Writes `tables` to a new workbook at `path`: one worksheet per table named after its title,
/// or with `one_sheet` all of them on one sheet, one below the other with a blank row between.
pub fn write_xlsx<'a>(
    path: &Path,
    tables: impl IntoIterator<Item = &'a Table>,
    one_sheet: bool,
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    if one_sheet {
        // no frozen panes here: every table has its own header
        let sheet = workbook.add_worksheet().set_name("Tables")?;
        let (mut top, mut widths) = (0, Vec::new());
        for table in tables {
            top = put_table(sheet, table, top, &mut widths)? + 1;
        }
        set_widths(sheet, &widths)?;
    } else {
        let mut names = Vec::new();
        for table in tables {
            let sheet = workbook.add_worksheet();
            sheet.set_name(sheet_name(&table.title, &mut names))?;
            let mut widths = Vec::new();
            put_table(sheet, table, 0, &mut widths)?;
            set_widths(sheet, &widths)?;
            let depth = header_rows(table).len() as u32;
            sheet.set_freeze_panes(1 + depth, table.row_headers as u16)?;
        }
    }
    workbook.save(path)
}

/// Writes the title, header, body and caption of `table` from row `top` and returns the row
/// after the last one written. `widths` grows to the characters each column needs.
fn put_table(
    sheet: &mut Worksheet,
    table: &Table,
    top: u32,
    widths: &mut Vec<usize>,
) -> Result<u32, XlsxError> {
    let grey = Color::RGB(0xA6A6A6);
    let border = Format::new()
        .set_border(FormatBorder::Thin)
        .set_border_color(grey);
    let header = border
        .clone()
        .set_bold()
        .set_background_color(Color::RGB(0xF2F2F2))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let row_header = border.clone().set_align(FormatAlign::Top).set_text_wrap();
    let text = border.clone().set_align(FormatAlign::Right);
    if widths.len() < table.columns.len() {
        widths.resize(table.columns.len(), 0);
    }
    let fit = |widths: &mut Vec<usize>, column: usize, text: &str| {
        widths[column] = widths[column].max(text.chars().count().min(MAX_WIDTH));
    };

    sheet.write_string_with_format(top, 0, &table.title, &Format::new().set_bold())?;
    let mut row = top + 1;
    let mut spans = Vec::new();
    let headers = header_rows(table);
    // above each row label column the header levels form one cell holding its label, if any
    let depth = headers.len() as u32;
    for c in 0..table.row_headers {
        let label: Vec<&str> = headers
            .iter()
            .flatten()
            .filter(|cell| cell.first == c && !cell.label.is_empty())
            .map(|cell| cell.label.as_str())
            .collect();
        let label = label.join(" ");
        if depth > 1 {
            sheet.merge_range(row, c as u16, row + depth - 1, c as u16, &label, &header)?;
        } else {
            sheet.write_string_with_format(row, c as u16, &label, &header)?;
        }
        fit(widths, c, &label);
    }
    for cells in &headers {
        for cell in cells.iter().filter(|cell| cell.first >= table.row_headers) {
            let column = cell.first as u16;
            if cell.span > 1 {
                let last = column + cell.span as u16 - 1;
                sheet.merge_range(row, column, row, last, &cell.label, &header)?;
                spans.push(cell.clone());
            } else {
                sheet.write_string_with_format(row, column, &cell.label, &header)?;
                fit(widths, cell.first, &cell.label);
            }
        }
        row += 1;
    }
    let body_top = row;
    let format = |r: usize, c: usize| {
        let format = table.formats.get(r).and_then(|f| f.get(c));
        format.map_or("", String::as_str)
    };
    for (r, cells) in table.rows.iter().enumerate() {
        for (c, cell) in cells.iter().enumerate() {
            let column = c as u16;
            let shown = cell_text(table, r, c);
            fit(widths, c, &shown);
            match cell {
                // row labels are written below, merged over the rows that repeat them
                _ if c < table.row_headers => continue,
                Cell::Number(n) if n.is_finite() => match excel_number(format(r, c), *n, &shown) {
                    Some((value, num_format)) => {
                        let number = border.clone().set_num_format(num_format);
                        sheet.write_number_with_format(row, column, value, &number)?
                    }
                    None => sheet.write_string_with_format(row, column, &shown, &text)?,
                },
                Cell::Empty => sheet.write_blank(row, column, &text)?,
                _ => sheet.write_string_with_format(row, column, &shown, &text)?,
            };
        }
        row += 1;
    }
    put_row_labels(sheet, table, body_top, &row_header)?;
    // a merged label longer than its columns widens each of them by an equal share
    for cell in spans {
        let columns = cell.first..cell.first + cell.span;
        let room = widths[columns.clone()].iter().sum::<usize>() + cell.span - 1;
        let need = cell.label.chars().count().min(MAX_WIDTH);
        let share = need.saturating_sub(room).div_ceil(cell.span);
        widths[columns].iter_mut().for_each(|w| *w += share);
    }
    if let Some(caption) = &table.caption {
        sheet.write_string_with_format(row, 0, caption, &Format::new().set_italic())?;
        row += 1;
    }
    Ok(row)
}

/// Writes the row label columns, merging a label down over the rows that repeat it under the
/// same outer labels, so a nested variable's label appears once beside its categories.
fn put_row_labels(
    sheet: &mut Worksheet,
    table: &Table,
    top: u32,
    format: &Format,
) -> Result<(), XlsxError> {
    let labels: Vec<Vec<String>> = (0..table.rows.len())
        .map(|r| {
            (0..table.row_headers)
                .map(|c| cell_text(table, r, c))
                .collect()
        })
        .collect();
    for c in 0..table.row_headers {
        let mut first = 0;
        while first < labels.len() {
            let same = |r: usize| labels[r][..=c] == labels[first][..=c];
            let last = (first..labels.len())
                .take_while(|&r| same(r))
                .last()
                .unwrap_or(first);
            let (from, to, column) = (top + first as u32, top + last as u32, c as u16);
            if last > first && !labels[first][c].is_empty() {
                sheet.merge_range(from, column, to, column, &labels[first][c], format)?;
            } else {
                for (r, row) in labels.iter().enumerate().take(last + 1).skip(first) {
                    sheet.write_string_with_format(top + r as u32, column, &row[c], format)?;
                }
            }
            first = last + 1;
        }
    }
    Ok(())
}

fn set_widths(sheet: &mut Worksheet, widths: &[usize]) -> Result<(), XlsxError> {
    for (c, width) in widths.iter().enumerate() {
        // a little room for the cell padding and the bold header font
        sheet.set_column_width(c as u16, *width as f64 * 1.1 + 2.0)?;
    }
    Ok(())
}

/// A worksheet name from a table title under Excel's rules: at most 31 characters, none of
/// [ ] : * ? / \, no apostrophe at either end, not "History", and unique ignoring case among
/// `used`, to which it is added.
fn sheet_name(title: &str, used: &mut Vec<String>) -> String {
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
