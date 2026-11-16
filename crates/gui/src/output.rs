//! The output: result tables grouped by run and command, and their text forms: tab-separated
//! text and HTML.

use statscraft_engine::{Cell, Chart, Table};
pub use statscraft_engine::{cell_text, header_rows};
use std::collections::HashSet;

/// A table in the output, with an id that stays the same while other tables are deleted. A
/// chart shows as its data table and keeps the chart beside it.
pub struct OutputTable {
    pub id: u64,
    pub table: Table,
    pub chart: Option<Chart>,
}

/// The tables of one command, with an id of its own for its heading.
pub struct OutputItem {
    pub id: u64,
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

/// The kinds of element the output shows: a command's heading, and a table's title, the table
/// itself and its notes. A chart takes the place of the table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Part {
    Heading,
    Title,
    Table,
    Notes,
    Chart,
}

impl Part {
    pub const ALL: [Part; 5] = [
        Part::Heading,
        Part::Title,
        Part::Table,
        Part::Notes,
        Part::Chart,
    ];

    /// Its word in the settings file.
    pub fn word(self) -> &'static str {
        match self {
            Part::Heading => "headings",
            Part::Title => "titles",
            Part::Table => "tables",
            Part::Notes => "notes",
            Part::Chart => "charts",
        }
    }

    /// Its name in Edit > Settings.
    pub fn label(self) -> &'static str {
        match self {
            Part::Heading => "Command headings",
            Part::Title => "Titles",
            Part::Table => "Tables",
            Part::Notes => "Notes under tables",
            Part::Chart => "Charts",
        }
    }
}

/// Which kinds of element the output shows, and whether exports take hidden ones too.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Show {
    /// By `Part::ALL`.
    pub kinds: [bool; 5],
    pub export_hidden: bool,
}

impl Default for Show {
    /// Everything but the command headings.
    fn default() -> Self {
        Show {
            kinds: [false, true, true, true, true],
            export_hidden: false,
        }
    }
}

impl Show {
    pub fn kind(&self, part: Part) -> bool {
        self.kinds[part as usize]
    }
}

/// A table, or a chart, as shown: without its title or notes when they do not show, and
/// whether its notes show.
pub type Exported = (Table, Option<Chart>, bool);

/// What of the output is shown: an element shows when its kind is on and it is not hidden
/// itself. A table's own visibility is `Table::hidden`; headings, titles and notes are hidden
/// one by one in `hidden`, by the id of their command or table. A table or chart not shown
/// takes its title and notes with it. With `all`, everything shows, as exports with hidden
/// items take it.
pub struct Visibility<'a> {
    pub show: Show,
    pub hidden: &'a HashSet<(u64, Part)>,
    pub all: bool,
}

impl Visibility<'_> {
    /// The kind of the table itself: a chart or a table.
    pub fn body(t: &OutputTable) -> Part {
        match t.chart {
            Some(_) => Part::Chart,
            None => Part::Table,
        }
    }

    /// The table or chart itself.
    pub fn table(&self, t: &OutputTable) -> bool {
        self.all || !t.table.hidden && self.show.kind(Self::body(t))
    }

    /// The title or the notes of a table: shown only with the table, and notes only when
    /// there are some.
    pub fn part(&self, t: &OutputTable, part: Part) -> bool {
        let own = self.all || self.show.kind(part) && !self.hidden.contains(&(t.id, part));
        own && self.table(t) && (part != Part::Notes || t.table.caption.is_some())
    }

    /// The command's heading, above its first table shown.
    pub fn heading(&self, item: &OutputItem) -> bool {
        let hidden = self.hidden.contains(&(item.id, Part::Heading));
        let own = self.all || self.show.kind(Part::Heading) && !hidden;
        own && item.tables.iter().any(|t| self.table(t))
    }

    /// The table and chart without the title, and without the notes (with a chart, its
    /// subtitle and footnote), when those do not show.
    pub fn shown(&self, t: &OutputTable) -> Exported {
        let (title, notes) = (self.part(t, Part::Title), self.part(t, Part::Notes));
        let mut table = t.table.clone();
        let mut chart = t.chart.clone();
        if !title {
            // still the name of its bookmark or sheet
            let title = std::mem::take(&mut table.title);
            table.outline.get_or_insert(title);
            chart.iter_mut().for_each(|c| c.title.clear());
        }
        if !notes {
            table.caption = None;
            chart
                .iter_mut()
                .for_each(|c| (c.subtitle, c.footnote) = (None, None));
        }
        (table, chart, notes)
    }

    /// Each command with tables that `keep` takes and that show: its heading when that shows,
    /// and those tables as shown. What an export holds.
    pub fn exported(
        &self,
        runs: &[OutputRun],
        keep: impl Fn(u64) -> bool,
    ) -> Vec<(Option<String>, Vec<Exported>)> {
        let mut out = Vec::new();
        for item in runs.iter().flat_map(|run| &run.items) {
            let tables = item.tables.iter().filter(|t| keep(t.id) && self.table(t));
            let tables: Vec<Exported> = tables.map(|t| self.shown(t)).collect();
            if !tables.is_empty() {
                out.push((self.heading(item).then(|| item.heading.clone()), tables));
            }
        }
        out
    }
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

/// The syntax of one command, as the engine's output documents record it.
pub use statscraft_engine::session::source_lines;

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
    // no title line for a hidden title
    let mut lines: Vec<String> = Some(clean(&table.title))
        .filter(|t| !t.is_empty())
        .into_iter()
        .collect();
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
    let mut html = match table.title.as_str() {
        "" => "<table>\n<thead>\n".to_string(),
        title => format!("<table>\n<caption>{}</caption>\n<thead>\n", escape(title)),
    };
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

/// The tables `keep` takes as one HTML page, under the heading of their command; a drawn chart
/// as its PNG picture inside the page, on light paper. Only what `shown` shows goes in.
pub fn html(runs: &[OutputRun], keep: impl Fn(u64) -> bool, shown: &Visibility) -> String {
    use statscraft_engine::picture::{FILE_WIDTH, Paper, chart_png, figure};
    let mut body = String::new();
    for (heading, tables) in shown.exported(runs, keep) {
        if let Some(heading) = heading {
            body.push_str(&format!("<h2>{}</h2>\n", escape(&heading)));
        }
        for (table, chart, _) in tables {
            let png = chart
                .as_ref()
                .and_then(|c| chart_png(c, Paper::Light)?.ok());
            match (png, &chart) {
                (Some(png), Some(chart)) => body.push_str(&figure(
                    &png,
                    "image/png",
                    f64::from(FILE_WIDTH),
                    &chart.title,
                )),
                _ => body.push_str(&table_html(&table)),
            }
        }
    }
    page(&body)
}

/// The output as a PDF file, as OUTPUT EXPORT /PDF writes it: bookmarks and the font embedded.
/// Only what `shown` shows goes in.
pub fn pdf(runs: &[OutputRun], keep: impl Fn(u64) -> bool, shown: &Visibility) -> Vec<u8> {
    use statscraft_engine::office::Item;
    let exported = shown.exported(runs, keep);
    // shortcut: a chart with no drawing goes in as its own data table, notes and all
    let items: Vec<(&str, Item)> = exported
        .iter()
        .flat_map(|(heading, tables)| {
            let heading = heading.as_deref().unwrap_or("");
            tables.iter().map(move |(table, chart, _)| match chart {
                Some(chart) => (heading, Item::Chart(chart)),
                None => (heading, Item::Table(table)),
            })
        })
        .collect();
    let options = statscraft_engine::pdf::Options {
        bookmarks: true,
        embed_fonts: true,
        title: None,
    };
    statscraft_engine::pdf::document(&items, &options)
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
            outline: None,
            hidden: false,
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
        let t = |id| OutputTable {
            id,
            table: table(),
            chart: None,
        };
        vec![
            OutputRun {
                label: s("Run 1"),
                items: vec![OutputItem {
                    id: 11,
                    heading: s("CTABLES & more"),
                    syntax: s("CTABLES /TABLE a BY b."),
                    tables: vec![t(1), t(2)],
                }],
            },
            OutputRun {
                label: s("Run 2"),
                items: vec![
                    OutputItem {
                        id: 12,
                        heading: s("FREQUENCIES"),
                        syntax: s("FREQUENCIES x."),
                        tables: vec![t(3)],
                    },
                    OutputItem {
                        id: 13,
                        heading: s("CROSSTABS"),
                        syntax: s("CROSSTABS a BY b."),
                        tables: vec![t(4)],
                    },
                ],
            },
        ]
    }

    /// Everything shown, as with "Export hidden items".
    fn all() -> Visibility<'static> {
        static NONE: std::sync::LazyLock<HashSet<(u64, Part)>> =
            std::sync::LazyLock::new(HashSet::new);
        Visibility {
            show: Show::default(),
            hidden: &NONE,
            all: true,
        }
    }

    #[test]
    fn html_escapes_spans_and_keeps_only_the_selection() {
        let page = html(&runs(), |_| true, &all());
        assert!(page.contains("<h2>CTABLES &amp; more</h2>"));
        assert!(page.contains("<th colspan=\"4\">Gender</th>"));
        assert!(page.contains("<th>North &lt;1&gt;</th><td>12</td><td>54.5%</td>"));
        assert!(page.contains("<p class=\"note\">Chi-square p = 0.512</p>"));
        assert!(page.ends_with("</html>\n"));
        assert_eq!(page.matches("<table>").count(), 4);
        let page = html(&runs(), |id| id == 4, &all());
        assert_eq!(page.matches("<table>").count(), 1);
        assert!(page.contains("<h2>CROSSTABS</h2>") && !page.contains("FREQUENCIES"));
        assert!(clipboard_html(&[&table()]).starts_with("<!doctype html>"));
    }

    #[test]
    fn pdf_holds_the_output_with_its_outline() {
        let everything = pdf(&runs(), |_| true, &all());
        assert!(everything.starts_with(b"%PDF-1.") && everything.ends_with(b"%%EOF"));
        let outline = everything.windows(9).any(|w| w == b"/Outlines");
        assert!(outline);
        assert!(pdf(&runs(), |id| id == 4, &all()).len() < everything.len());
    }

    #[test]
    fn an_element_shows_when_its_kind_is_on_and_it_is_not_hidden() {
        let mut output = runs();
        let mut hidden = HashSet::from([(2, Part::Title), (12, Part::Heading)]);
        let shown = |output: &[OutputRun], hidden: &HashSet<(u64, Part)>, show: Show| {
            let v = Visibility {
                show,
                hidden,
                all: false,
            };
            let items: Vec<&OutputItem> = output.iter().flat_map(|r| &r.items).collect();
            let tables: Vec<&OutputTable> = tables(output).collect();
            let parts = |part| tables.iter().map(|t| v.part(t, part)).collect::<Vec<_>>();
            (
                items.iter().map(|i| v.heading(i)).collect::<Vec<_>>(),
                tables.iter().map(|t| v.table(t)).collect::<Vec<_>>(),
                parts(Part::Title),
                parts(Part::Notes),
            )
        };
        // headings are off by default; one title is hidden itself
        let (headings, bodies, titles, notes) = shown(&output, &hidden, Show::default());
        assert_eq!(headings, [false, false, false]);
        assert_eq!(bodies, [true; 4]);
        assert_eq!(titles, [true, false, true, true]);
        assert_eq!(notes, [true; 4]);
        // headings on: the one hidden itself stays away
        let mut show = Show::default();
        show.kinds[Part::Heading as usize] = true;
        assert_eq!(shown(&output, &hidden, show).0, [true, false, true]);
        // notes off, and a table hidden itself takes its title and notes with it
        show.kinds[Part::Notes as usize] = false;
        output[1].items[1].tables[0].table.hidden = true;
        hidden.clear();
        let (headings, bodies, titles, notes) = shown(&output, &hidden, show);
        // the command whose only table is hidden loses its heading too
        assert_eq!(headings, [true, true, false]);
        assert_eq!(bodies, [true, true, true, false]);
        assert_eq!(titles, [true, true, true, false]);
        assert_eq!(notes, [false; 4]);
        // tables off leaves nothing of a table
        show.kinds[Part::Table as usize] = false;
        let (headings, bodies, ..) = shown(&output, &hidden, show);
        assert_eq!((headings, bodies), (vec![false; 3], vec![false; 4]));
    }

    #[test]
    fn exports_leave_out_what_does_not_show_unless_asked() {
        let mut output = runs();
        output[1].items[0].tables[0].table.hidden = true;
        output[0].items[0].tables[1].table.title = s("Second");
        let hidden = HashSet::from([(1, Part::Title)]);
        let shown = Visibility {
            show: Show::default(),
            hidden: &hidden,
            all: false,
        };
        let page = html(&output, |_| true, &shown);
        // no headings, the hidden table and the hidden title left out
        assert!(!page.contains("<h2>"));
        assert_eq!(page.matches("<table>").count(), 3);
        assert_eq!(page.matches("<caption>").count(), 2);
        assert!(page.contains("<caption>Second</caption>"));
        // the same with "Export hidden items": everything
        let page = html(&output, |_| true, &all());
        assert_eq!(page.matches("<h2>").count(), 3);
        assert_eq!(
            page.matches("<caption>Region by gender</caption>").count(),
            3
        );
        // what a workbook and a copy get: no hidden table, no title where it is hidden (its
        // bookmark and sheet keep the name); the workbook writer leaves out an empty title
        let exported = shown.exported(&output, |_| true);
        let tables: Vec<&Exported> = exported.iter().flat_map(|(_, t)| t).collect();
        assert_eq!(tables.len(), 3);
        assert!(exported.iter().all(|(heading, _)| heading.is_none()));
        assert_eq!(tables[0].0.title, "");
        assert_eq!(tables[0].0.outline.as_deref(), Some("Region by gender"));
        assert!(tsv(&tables[0].0).starts_with("\t\tGender"));
        // notes off: none in the copy, and the workbook is told to leave them out
        let mut show = Show::default();
        show.kinds[Part::Notes as usize] = false;
        let shown = Visibility { show, ..shown };
        let (table, _, notes) = shown.shown(&output[0].items[0].tables[1]);
        assert!(table.caption.is_none() && !notes && table.title == "Second");
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
