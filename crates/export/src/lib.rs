//! Result tables as an Excel workbook, for the command line and the window alike. A crate of its
//! own so the engine stays free of file formats it does not read.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};
use statscraft_engine::{
    Cell, Table, cell_text, currency_parts, header_rows, is_date_time_or_currency,
};
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

/// The Excel number format that shows `value` as our display format `format` does ("F8.2",
/// "PCT40.1", "COMMA12.1", "DOLLAR8.2", "E10.3", the CTABLES formats such as "PCTPAREN8.1").
/// The value is never scaled: a percentage cell holds 45.2 for 45.2 percent, so the % sign is
/// a literal; a negative section writes its own minus where the text output has one. Without a format, whole numbers show
/// without decimals and others with two, as in the text output. Other formats show like F, as
/// `format_value` does. Excel adds a leading zero (0.50 where the text output has .50).
fn number_format(format: &str, value: f64) -> String {
    let letters = format
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(format.len());
    let (kind, size) = format.split_at(letters);
    let decimals = match size.split_once('.') {
        _ if format.is_empty() => usize::from(value.fract() != 0.0) * 2,
        Some((_, d)) => d.parse().unwrap_or(0),
        None => 0,
    };
    let fixed = match decimals {
        0 => "0".to_string(),
        d => format!("0.{}", "0".repeat(d.min(30))),
    };
    match kind.to_ascii_uppercase().as_str() {
        "PCT" => format!("{fixed}\"%\""),
        "COMMA" => format!("#,##{fixed}"),
        "DOLLAR" => format!("\"$\"#,##{fixed}"),
        "E" => format!("{fixed}E+000"),
        "NEGPAREN" => format!("{fixed};\"(\"{fixed}\")\""),
        "NEQUAL" => format!("\"N=\"{fixed};\"N=-\"{fixed}"),
        "PAREN" => format!("\"(\"{fixed}\")\";\"(-\"{fixed}\")\""),
        "PCTPAREN" => format!("\"(\"{fixed}\"%)\";\"(-\"{fixed}\"%)\""),
        _ => fixed,
    }
}

/// A number cell as Excel holds it: the number and the number format that shows it as `shown`,
/// the cell's text, does. None where no Excel format can show it; the text is written then.
fn excel_number(format: &str, value: f64, shown: &str) -> Option<(f64, String)> {
    if !is_date_time_or_currency(format) {
        return Some((value, number_format(format, value)));
    }
    if let Some((parts, _)) = currency_parts(format) {
        return Some((value, currency_format(format, &parts)));
    }
    excel_date(format, value, shown)
}

/// `text` as literal characters of an Excel number format.
fn literal(text: &str) -> String {
    text.chars().map(|c| format!("\\{c}")).collect()
}

/// The Excel format of a custom currency format ("CCA8.2 -,$,,"), from its negative prefix,
/// prefix, suffix and negative suffix. The thousands are grouped and the decimal point placed
/// as the reader's Excel does, which for a specification with periods (-.Dfl ..-) may differ
/// from the text output.
fn currency_format(
    format: &str,
    [negative_prefix, prefix, suffix, negative_suffix]: &[String; 4],
) -> String {
    let name = format.split(' ').next().unwrap_or(format);
    let decimals = name
        .split_once('.')
        .map_or(0, |(_, d)| d.parse().unwrap_or(0));
    let number = match decimals {
        0 => "#,##0".to_string(),
        d => format!("#,##0.{}", "0".repeat(d.min(30))),
    };
    let positive = format!("{}{number}{}", literal(prefix), literal(suffix));
    let negative = format!(
        "{}{number}{}",
        literal(&format!("{negative_prefix}{prefix}")),
        literal(&format!("{suffix}{negative_suffix}"))
    );
    format!("{positive};{negative}")
}

/// Days from 1582-10-14, where the date numbers start, to 1899-12-30, where Excel's start.
const EXCEL_EPOCH: f64 = 115_859.0;

/// A date or time cell as Excel holds one: days (from 1899-12-30 for a date, the time of day as
/// the fraction) and the number format that shows it in the form `shown` took (a two or four
/// digit year, hh:mm or hh:mm:ss, decimals of seconds, up to three). Excel writes month names
/// as Jan, not JAN, and rounds the last digit where the text cuts it off. None where Excel
/// cannot show it: a date before March 1900 (Excel's calendar holds a false 29 February 1900),
/// a negative time, a day of the year (JDATE), a quarter (QYR), a week (WKYR), a day count
/// before the clock (DTIME), the names of WKDAY and MONTH, and asterisks.
fn excel_date(format: &str, value: f64, shown: &str) -> Option<(f64, String)> {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    let kind = format[..letters.unwrap_or(format.len())].to_ascii_uppercase();
    if shown.contains('*') || value < 0.0 {
        return None;
    }
    // hh:mm, hh:mm:ss or mm:ss with the decimals of the seconds, the first field as `first`
    let clock = |text: &str, first: &str| {
        let decimals = text.split_once('.').map_or(0, |(_, d)| d.len().min(3));
        let mut out = match (first, text.matches(':').count()) {
            ("[mm]", _) => "[mm]:ss".to_string(),
            (hours, 2) => format!("{hours}:mm:ss"),
            (hours, _) => format!("{hours}:mm"),
        };
        if decimals > 0 {
            out.push_str(&format!(".{}", "0".repeat(decimals)));
        }
        out
    };
    let days = value / 86_400.0;
    match kind.as_str() {
        "TIME" => return Some((days, clock(shown, "[hh]"))),
        "MTIME" => return Some((days, clock(shown, "[mm]"))),
        _ => {}
    }
    let (date, time) = match kind.as_str() {
        "DATETIME" | "YMDHMS" => shown.split_once(' ')?,
        _ => (shown, ""),
    };
    // the four-digit year form and its length; the other form has a two-digit year
    let (long, length) = match kind.as_str() {
        "DATE" | "DATETIME" => ("dd-mmm-yyyy", 11),
        "ADATE" => ("mm/dd/yyyy", 10),
        "EDATE" => ("dd.mm.yyyy", 10),
        "SDATE" => ("yyyy/mm/dd", 10),
        "YMDHMS" => ("yyyy-mm-dd", 10),
        "MOYR" => ("mmm yyyy", 8),
        _ => return None,
    };
    let pattern = if date.len() >= length {
        long.to_string()
    } else {
        long.replacen("yyyy", "yy", 1)
    };
    // the separators as literals, so no Excel locale swaps them for its own
    let mut out: String = pattern
        .chars()
        .map(|c| match c {
            '/' | '.' => format!("\\{c}"),
            c => c.to_string(),
        })
        .collect();
    if !time.is_empty() {
        out.push(' ');
        out.push_str(&clock(time, "hh"));
    }
    let serial = days - EXCEL_EPOCH;
    (serial >= 61.0).then_some((serial, out))
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
    fn every_display_format_maps_to_an_excel_format() {
        let cases = [
            ("F40.0", 12.0, "0"),
            ("F8.2", 175.04, "0.00"),
            ("F3.0", 5440.0, "0"),
            ("F7.0", 3.0, "0"),
            ("f8.3", 1.5, "0.000"),
            ("PCT40.1", 45.2, "0.0\"%\""),
            ("PCT8.1", 0.0, "0.0\"%\""),
            ("PCT40.0", 50.0, "0\"%\""),
            ("COMMA10.0", 1234.0, "#,##0"),
            ("COMMA12.1", 1234567.5, "#,##0.0"),
            ("DOLLAR10.2", 207.045, "\"$\"#,##0.00"),
            ("DOLLAR9.0", -1234.0, "\"$\"#,##0"),
            ("N8", 7.0, "0"),
            ("E10.3", 175.04, "0.000E+000"),
            ("NEGPAREN9.2", -1.5, "0.00;\"(\"0.00\")\""),
            ("NEQUAL5", 394.0, "\"N=\"0;\"N=-\"0"),
            ("PAREN6.1", -8.0, "\"(\"0.0\")\";\"(-\"0.0\")\""),
            ("PCTPAREN8.1", 67.6, "\"(\"0.0\"%)\";\"(-\"0.0\"%)\""),
            ("", 22.0, "0"),
            ("", 0.25, "0.00"),
        ];
        for (format, value, expected) in cases {
            assert_eq!(number_format(format, value), expected, "{format}");
        }
    }

    #[test]
    fn dates_times_and_currencies_become_excel_numbers() {
        // 2020-01-01 is day 159 690 of the date scale and day 43 831 of Excel's
        let day = 159_690.0 * 86_400.0;
        let cases = [
            ("ADATE10", day, "01/01/2020", 43_831.0, "mm\\/dd\\/yyyy"),
            ("ADATE8", day, "01/01/20", 43_831.0, "mm\\/dd\\/yy"),
            ("DATE11", day, "01-JAN-2020", 43_831.0, "dd-mmm-yyyy"),
            ("DATE9", day, "01-JAN-20", 43_831.0, "dd-mmm-yy"),
            ("EDATE10", day, "01.01.2020", 43_831.0, "dd\\.mm\\.yyyy"),
            ("SDATE10", day, "2020/01/01", 43_831.0, "yyyy\\/mm\\/dd"),
            ("MOYR8", day, "JAN 2020", 43_831.0, "mmm yyyy"),
            (
                "DATETIME20",
                day + 43_200.0,
                "01-JAN-2020 12:00:00",
                43_831.5,
                "dd-mmm-yyyy hh:mm:ss",
            ),
            (
                "YMDHMS16",
                day + 43_200.0,
                "2020-01-01 12:00",
                43_831.5,
                "yyyy-mm-dd hh:mm",
            ),
            ("TIME8", 21_600.0, "06:00:00", 0.25, "[hh]:mm:ss"),
            ("TIME5", 21_600.0, "06:00", 0.25, "[hh]:mm"),
            ("TIME11.2", 21_600.0, "06:00:00.00", 0.25, "[hh]:mm:ss.00"),
            ("MTIME5", 21_600.0, "360:00", 0.25, "[mm]:ss"),
            ("CCA8.2", -1.5, "-1.50", -1.5, "#,##0.00;\\-#,##0.00"),
            (
                "CCA8.2 (,$,,)",
                -1.5,
                "($1.50)",
                -1.5,
                "\\$#,##0.00;\\(\\$#,##0.00\\)",
            ),
            (
                "CCB6.0 -,, EUR,",
                2.0,
                "2 EUR",
                2.0,
                "#,##0\\ \\E\\U\\R;\\-#,##0\\ \\E\\U\\R",
            ),
        ];
        for (format, value, shown, number, code) in cases {
            let expected = Some((number, code.to_string()));
            assert_eq!(excel_number(format, value, shown), expected, "{format}");
        }
        // what Excel cannot show stays text
        let text = [
            ("DTIME10", 90_000.0, "1 01:00:00"),
            ("JDATE7", day, "2020001"),
            ("QYR8", day, "1 Q 2020"),
            ("WKYR10", day, "1 WK 2020"),
            ("WKDAY9", 1.0, "SUNDAY"),
            ("MONTH3", 1.0, "JAN"),
            ("TIME8", -60.0, "-00:01:00"),
            // 1900-01-01, before Excel's false 29 February 1900
            ("ADATE10", 115_860.0 * 86_400.0, "01/01/1900"),
            ("DATE11", day, "***********"),
        ];
        for (format, value, shown) in text {
            assert_eq!(excel_number(format, value, shown), None, "{format}");
        }
    }

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
