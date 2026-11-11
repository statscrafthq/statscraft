//! The output: result tables grouped by run and command, and their text forms: tab-separated
//! text and HTML.

use statscraft_engine::{Cell, Table};
pub use statscraft_engine::{cell_text, header_rows};

/// A table in the output, with an id that stays the same while other tables are deleted.
pub struct OutputTable {
    pub id: u64,
    pub table: Table,
}

/// The tables of one command.
pub struct OutputItem {
    pub heading: String,
    /// The syntax that made the tables, as it was written.
    pub syntax: String,
    pub tables: Vec<OutputTable>,
}

/// The commands of one run that made tables.
pub struct OutputRun {
    pub label: String,
    pub items: Vec<OutputItem>,
}

/// Every table of the output, in order.
pub fn tables(runs: &[OutputRun]) -> impl Iterator<Item = &OutputTable> {
    runs.iter()
        .flat_map(|run| &run.items)
        .flat_map(|item| &item.tables)
}

/// The command that made the table with that id.
pub fn item_of(runs: &[OutputRun], id: u64) -> Option<&OutputItem> {
    runs.iter()
        .flat_map(|run| &run.items)
        .find(|item| item.tables.iter().any(|t| t.id == id))
}

/// Lines `first` to `last` (from 1, inclusive) of a source: the syntax of one command.
pub fn source_lines(source: &str, first: u32, last: u32) -> String {
    let count = last.saturating_sub(first) as usize + 1;
    let lines: Vec<&str> = source
        .lines()
        .skip(first.saturating_sub(1) as usize)
        .take(count)
        .collect();
    lines.join("\n")
}

/// Keeps the tables `keep` takes, and the commands and runs that still have one.
pub fn retain(runs: &mut Vec<OutputRun>, keep: impl Fn(u64) -> bool) {
    for run in runs.iter_mut() {
        for item in &mut run.items {
            item.tables.retain(|t| keep(t.id));
        }
        run.items.retain(|item| !item.tables.is_empty());
    }
    runs.retain(|run| !run.items.is_empty());
}

/// For every body row, the number of rows each row label spans: a label repeated under the same
/// outer labels is shown once, over all its rows, and the rows below it get 0.
pub fn row_spans(table: &Table) -> Vec<Vec<usize>> {
    let labels: Vec<Vec<String>> = (0..table.rows.len())
        .map(|r| {
            (0..table.row_headers.min(table.rows[r].len()))
                .map(|c| cell_text(table, r, c))
                .collect()
        })
        .collect();
    let mut spans: Vec<Vec<usize>> = labels.iter().map(|l| vec![1; l.len()]).collect();
    for c in 0..table.row_headers {
        let mut first = 0;
        while first < labels.len() {
            let same = |r: usize| {
                labels[r].len() > c
                    && labels[first].len() > c
                    && labels[r][..=c] == labels[first][..=c]
            };
            let last = (first..labels.len())
                .take_while(|&r| same(r))
                .last()
                .unwrap_or(first);
            if last > first && !labels[first][c].is_empty() {
                spans[first][c] = last - first + 1;
                for row in &mut spans[first + 1..=last] {
                    row[c] = 0;
                }
            }
            first = last + 1;
        }
    }
    spans
}

/// Whether a cell's text belongs at the right of its column: numbers, and text that stands for
/// one, such as a hidden small count (<5) or a significance below a limit.
pub fn right_aligned(table: &Table, row: usize, column: usize) -> bool {
    match &table.rows[row][column] {
        Cell::Number(_) => true,
        Cell::Text(text) => text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || "-+.<>".contains(c)),
        Cell::Empty => false,
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
tbody th{text-align:left;font-weight:normal;vertical-align:top}td{text-align:right}\
td.text{text-align:left}p.note{font-size:.85rem;color:#4a5468}";

/// One table as HTML: a repeated row label spans its rows, and the caption follows as a note.
pub fn table_html(table: &Table) -> String {
    let mut html = format!(
        "<table>\n<caption>{}</caption>\n<thead>\n",
        escape(&table.title)
    );
    for cells in header_rows(table) {
        html.push_str("<tr>");
        for cell in cells {
            let span = match cell.span {
                1 => String::new(),
                n => format!(" colspan=\"{n}\""),
            };
            html.push_str(&format!("<th{span}>{}</th>", escape(&cell.label)));
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</thead>\n<tbody>\n");
    for (r, spans) in row_spans(table).iter().enumerate() {
        html.push_str("<tr>");
        for c in 0..table.rows[r].len() {
            let text = escape(&cell_text(table, r, c));
            if c < table.row_headers {
                match spans.get(c).copied().unwrap_or(1) {
                    0 => {}
                    1 => html.push_str(&format!("<th>{text}</th>")),
                    n => html.push_str(&format!("<th rowspan=\"{n}\">{text}</th>")),
                }
            } else if right_aligned(table, r, c) {
                html.push_str(&format!("<td>{text}</td>"));
            } else {
                html.push_str(&format!("<td class=\"text\">{text}</td>"));
            }
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</tbody>\n</table>\n");
    if let Some(caption) = &table.caption {
        html.push_str(&format!("<p class=\"note\">{}</p>\n", escape(caption)));
    }
    html
}

fn page(body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Statscraft output</title>\n<style>{STYLE}</style>\n</head>\n<body>\n{body}</body>\n</html>\n"
    )
}

/// The tables `keep` takes as one HTML page, under the heading of their command.
pub fn html(runs: &[OutputRun], keep: impl Fn(u64) -> bool) -> String {
    let mut body = String::new();
    for item in runs.iter().flat_map(|run| &run.items) {
        let tables: Vec<&OutputTable> = item.tables.iter().filter(|t| keep(t.id)).collect();
        if tables.is_empty() {
            continue;
        }
        body.push_str(&format!("<h2>{}</h2>\n", escape(&item.heading)));
        for t in tables {
            body.push_str(&table_html(&t.table));
        }
    }
    page(&body)
}

/// Tables as rich text for the clipboard: an HTML page that word processors and spreadsheets
/// paste with its layout.
pub fn clipboard_html(tables: &[&Table]) -> String {
    page(&tables.iter().map(|t| table_html(t)).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use statscraft_engine::Cell;

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

    /// Nested rows: Region > North, South, each with Male and Female, then a Total row.
    fn nested() -> Table {
        let row = |a: &str, b: &str, c: &str, n: f64| {
            vec![
                Cell::Text(s(a)),
                Cell::Text(s(b)),
                Cell::Text(s(c)),
                Cell::Number(n),
            ]
        };
        Table {
            title: s("Nested"),
            row_headers: 3,
            columns: vec![s(""), s(""), s(""), s("Count")],
            rows: vec![
                row("Region", "North", "Male", 1.0),
                row("Region", "North", "Female", 2.0),
                row("Region", "South", "Male", 3.0),
                row("Region", "South", "Female", 4.0),
                row("", "", "Total", 10.0),
                row("", "", "Other", 0.0),
            ],
            ..Table::default()
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
    fn repeated_row_labels_span_their_rows() {
        assert_eq!(
            row_spans(&nested()),
            [
                vec![4, 2, 1],
                vec![0, 0, 1],
                vec![0, 2, 1],
                vec![0, 0, 1],
                // empty labels are never merged
                vec![1, 1, 1],
                vec![1, 1, 1],
            ]
        );
        let html = table_html(&nested());
        assert!(html.contains(
            "<tr><th rowspan=\"4\">Region</th><th rowspan=\"2\">North</th><th>Male</th><td>1</td></tr>"
        ));
        assert!(html.contains("<tr><th>Female</th><td>2</td></tr>"));
    }

    #[test]
    fn numbers_and_number_like_text_align_right() {
        let mut t = nested();
        t.rows[0][3] = Cell::Text(s("<5"));
        t.rows[1][3] = Cell::Text(s("A B"));
        t.rows[2][3] = Cell::Empty;
        assert!(right_aligned(&t, 0, 3));
        assert!(!right_aligned(&t, 1, 3));
        assert!(!right_aligned(&t, 2, 3));
        assert!(right_aligned(&t, 3, 3));
        assert!(table_html(&t).contains("<td class=\"text\">A B</td>"));
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

    fn runs() -> Vec<OutputRun> {
        let t = |id| OutputTable { id, table: table() };
        vec![
            OutputRun {
                label: s("Run 1"),
                items: vec![OutputItem {
                    heading: s("CTABLES & more"),
                    syntax: s("CTABLES /TABLE a BY b."),
                    tables: vec![t(1), t(2)],
                }],
            },
            OutputRun {
                label: s("Run 2"),
                items: vec![
                    OutputItem {
                        heading: s("FREQUENCIES"),
                        syntax: s("FREQUENCIES x."),
                        tables: vec![t(3)],
                    },
                    OutputItem {
                        heading: s("CROSSTABS"),
                        syntax: s("CROSSTABS a BY b."),
                        tables: vec![t(4)],
                    },
                ],
            },
        ]
    }

    #[test]
    fn html_escapes_spans_and_keeps_only_the_selection() {
        let page = html(&runs(), |_| true);
        assert!(page.contains("<h2>CTABLES &amp; more</h2>"));
        assert!(page.contains("<th colspan=\"4\">Gender</th>"));
        assert!(page.contains("<th>North &lt;1&gt;</th><td>12</td><td>54.5%</td>"));
        assert!(page.contains("<p class=\"note\">Chi-square p = 0.512</p>"));
        assert!(page.ends_with("</html>\n"));
        assert_eq!(page.matches("<table>").count(), 4);
        let page = html(&runs(), |id| id == 4);
        assert_eq!(page.matches("<table>").count(), 1);
        assert!(page.contains("<h2>CROSSTABS</h2>") && !page.contains("FREQUENCIES"));
        assert!(clipboard_html(&[&table()]).starts_with("<!doctype html>"));
    }

    #[test]
    fn every_command_keeps_the_syntax_that_made_it() {
        let source = "GET FILE='a.sav'.\r\nFREQUENCIES\n  /VARIABLES=x.\nfre y.";
        assert_eq!(source_lines(source, 2, 3), "FREQUENCIES\n  /VARIABLES=x.");
        assert_eq!(source_lines(source, 4, 4), "fre y.");
        assert_eq!(source_lines(source, 1, 1), "GET FILE='a.sav'.");
        let output = runs();
        assert_eq!(
            item_of(&output, 2).unwrap().syntax,
            "CTABLES /TABLE a BY b."
        );
        assert_eq!(item_of(&output, 4).unwrap().syntax, "CROSSTABS a BY b.");
        assert!(item_of(&output, 9).is_none());
    }

    #[test]
    fn deleting_tables_drops_empty_commands_and_runs() {
        let mut output = runs();
        retain(&mut output, |id| id != 1);
        assert_eq!(tables(&output).map(|t| t.id).collect::<Vec<_>>(), [2, 3, 4]);
        retain(&mut output, |id| id == 4);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].label, "Run 2");
        assert_eq!(output[0].items.len(), 1);
        assert_eq!(output[0].items[0].heading, "CROSSTABS");
        retain(&mut output, |_| false);
        assert!(output.is_empty());
    }
}
