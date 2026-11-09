//! Output items and their text forms: header layout, tab-separated text and HTML.

use statscraft_engine::{Cell, Table, format_value};

/// The tables of one command as they appear in the output, with a heading for the outline.
pub struct OutputItem {
    pub heading: String,
    pub tables: Vec<Table>,
}

/// One header cell spanning `span` columns from `first`.
#[derive(Debug, PartialEq)]
pub struct HeaderCell {
    pub first: usize,
    pub span: usize,
    pub label: String,
}

/// The header rows of a table, outermost first. Every level covers all columns; neighbouring
/// columns that share a label and everything above it are merged into one cell.
pub fn header_rows(table: &Table) -> Vec<Vec<HeaderCell>> {
    let depth = 1 + table.column_groups.iter().map(Vec::len).max().unwrap_or(0);
    let label = |column: usize, level: usize| -> &str {
        let groups = table
            .column_groups
            .get(column)
            .map_or(&[][..], Vec::as_slice);
        if level == depth - 1 {
            &table.columns[column]
        } else {
            groups.get(level).map_or("", String::as_str)
        }
    };
    let path = |column: usize, level: usize| (0..=level).map(move |l| label(column, l));
    (0..depth)
        .map(|level| {
            let mut cells: Vec<HeaderCell> = Vec::new();
            for column in 0..table.columns.len() {
                match cells.last_mut() {
                    Some(cell)
                        if column >= table.row_headers
                            && cell.first >= table.row_headers
                            && path(cell.first, level).eq(path(column, level)) =>
                    {
                        cell.span += 1
                    }
                    _ => cells.push(HeaderCell {
                        first: column,
                        span: 1,
                        label: label(column, level).to_string(),
                    }),
                }
            }
            cells
        })
        .collect()
}

/// A result cell as text, in its display format. The same rules as the command line output.
pub fn cell_text(table: &Table, row: usize, column: usize) -> String {
    let format = table
        .formats
        .get(row)
        .and_then(|f| f.get(column))
        .filter(|f| !f.is_empty());
    match (&table.rows[row][column], format) {
        (Cell::Text(t), _) => t.clone(),
        (Cell::Number(n), Some(format)) => format_value(*n, format, true),
        (Cell::Number(n), None) if n.fract() == 0.0 => format!("{n:.0}"),
        (Cell::Number(n), None) => format!("{n:.2}"),
        (Cell::Empty, _) => String::new(),
    }
}

/// The table as tab-separated text for pasting into a spreadsheet: title, header rows, body
/// and caption, with a spanning label in its first column.
pub fn tsv(table: &Table) -> String {
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let mut lines = vec![clean(&table.title)];
    for cells in header_rows(table) {
        let mut row = vec![String::new(); table.columns.len()];
        for cell in cells {
            row[cell.first] = clean(&cell.label);
        }
        lines.push(row.join("\t"));
    }
    for r in 0..table.rows.len() {
        let row: Vec<String> = (0..table.rows[r].len())
            .map(|c| clean(&cell_text(table, r, c)))
            .collect();
        lines.push(row.join("\t"));
    }
    if let Some(caption) = &table.caption {
        lines.push(clean(caption));
    }
    lines.join("\n") + "\n"
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;color:#1d2433;margin:2rem;line-height:1.4}\
h2{font-size:1rem;color:#4a5468;font-weight:600;margin:2rem 0 .5rem}\
table{border-collapse:collapse;margin:.5rem 0 1.5rem;font-variant-numeric:tabular-nums}\
caption{text-align:left;font-weight:600;padding:.25rem 0}\
th,td{padding:.25rem .6rem;border-bottom:1px solid #dde2ea}\
thead th{background:#f3f5f8;text-align:center}\
tbody th{text-align:left;font-weight:normal}td{text-align:right}\
p.note{font-size:.85rem;color:#4a5468}";

/// The whole output as one HTML page.
pub fn html(items: &[OutputItem]) -> String {
    let mut page = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>StatsCraft output</title>\n<style>{STYLE}</style>\n</head>\n<body>\n"
    );
    for item in items {
        page.push_str(&format!("<h2>{}</h2>\n", escape(&item.heading)));
        for table in &item.tables {
            page.push_str(&format!(
                "<table>\n<caption>{}</caption>\n<thead>\n",
                escape(&table.title)
            ));
            for cells in header_rows(table) {
                page.push_str("<tr>");
                for cell in cells {
                    let span = match cell.span {
                        1 => String::new(),
                        n => format!(" colspan=\"{n}\""),
                    };
                    page.push_str(&format!("<th{span}>{}</th>", escape(&cell.label)));
                }
                page.push_str("</tr>\n");
            }
            page.push_str("</thead>\n<tbody>\n");
            for r in 0..table.rows.len() {
                page.push_str("<tr>");
                for c in 0..table.rows[r].len() {
                    let tag = if c < table.row_headers { "th" } else { "td" };
                    page.push_str(&format!(
                        "<{tag}>{}</{tag}>",
                        escape(&cell_text(table, r, c))
                    ));
                }
                page.push_str("</tr>\n");
            }
            page.push_str("</tbody>\n</table>\n");
            if let Some(caption) = &table.caption {
                page.push_str(&format!("<p class=\"note\">{}</p>\n", escape(caption)));
            }
        }
    }
    page.push_str("</body>\n</html>\n");
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    /// Two row header columns, then Gender (Male, Female) with Count and %, then Total.
    fn table() -> Table {
        let group = |a: &str, b: &str| vec![s(a), s(b)];
        Table {
            title: s("Region by gender"),
            row_headers: 2,
            columns: vec![
                s(""),
                s(""),
                s("Count"),
                s("%"),
                s("Count"),
                s("%"),
                s("Total"),
            ],
            column_groups: vec![
                vec![],
                vec![],
                group("Gender", "Male"),
                group("Gender", "Male"),
                group("Gender", "Female"),
                group("Gender", "Female"),
                vec![],
            ],
            rows: vec![vec![
                Cell::Text(s("Region")),
                Cell::Text(s("North <1>")),
                Cell::Number(12.0),
                Cell::Number(54.5454),
                Cell::Number(10.0),
                Cell::Number(45.4545),
                Cell::Number(22.0),
            ]],
            formats: vec![vec![
                s(""),
                s(""),
                s("F40.0"),
                s("PCT40.1"),
                s(""),
                s("PCT40.1"),
                s(""),
            ]],
            caption: Some(s("Chi-square p = 0.512")),
        }
    }

    #[test]
    fn header_levels_merge_shared_labels() {
        let rows = header_rows(&table());
        let labels: Vec<Vec<(usize, usize, &str)>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|c| (c.first, c.span, c.label.as_str()))
                    .collect()
            })
            .collect();
        assert_eq!(
            labels,
            [
                vec![(0, 1, ""), (1, 1, ""), (2, 4, "Gender"), (6, 1, "")],
                vec![
                    (0, 1, ""),
                    (1, 1, ""),
                    (2, 2, "Male"),
                    (4, 2, "Female"),
                    (6, 1, "")
                ],
                vec![
                    (0, 1, ""),
                    (1, 1, ""),
                    (2, 1, "Count"),
                    (3, 1, "%"),
                    (4, 1, "Count"),
                    (5, 1, "%"),
                    (6, 1, "Total")
                ],
            ]
        );
        // a one-level header is the column titles
        let flat = Table {
            columns: vec![s("Value"), s("Count")],
            row_headers: 1,
            ..Table::default()
        };
        assert_eq!(header_rows(&flat).len(), 1);
    }

    #[test]
    fn cells_use_their_display_format() {
        let t = table();
        let row: Vec<String> = (0..7).map(|c| cell_text(&t, 0, c)).collect();
        assert_eq!(
            row,
            ["Region", "North <1>", "12", "54.5%", "10", "45.5%", "22"]
        );
    }

    #[test]
    fn tab_separated_text_keeps_the_layout() {
        assert_eq!(
            tsv(&table()),
            "Region by gender\n\
             \t\tGender\t\t\t\t\n\
             \t\tMale\t\tFemale\t\t\n\
             \t\tCount\t%\tCount\t%\tTotal\n\
             Region\tNorth <1>\t12\t54.5%\t10\t45.5%\t22\n\
             Chi-square p = 0.512\n"
        );
    }

    #[test]
    fn html_escapes_and_spans() {
        let page = html(&[OutputItem {
            heading: s("CTABLES & more"),
            tables: vec![table()],
        }]);
        assert!(page.contains("<h2>CTABLES &amp; more</h2>"));
        assert!(page.contains("<th colspan=\"4\">Gender</th>"));
        assert!(page.contains("<th>North &lt;1&gt;</th><td>12</td><td>54.5%</td>"));
        assert!(page.contains("<p class=\"note\">Chi-square p = 0.512</p>"));
        assert!(page.ends_with("</html>\n"));
    }
}
