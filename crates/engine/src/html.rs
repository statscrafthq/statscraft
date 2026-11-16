//! Result tables as a web page, for OUTPUT EXPORT /HTML: the same layout as the window's
//! export (crates/gui/src/output.rs).

use crate::{Cell, Decoration, Table, cell_text, header_rows};

/// For every body row, the number of rows each row label spans: a label repeated under the same
/// outer labels is shown once, over all its rows, and the rows below it get 0.
fn row_spans(table: &Table) -> Vec<Vec<usize>> {
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

/// Numbers, and text that stands for one, such as a hidden small count (<5), go right.
pub(crate) fn right_aligned(table: &Table, row: usize, column: usize) -> bool {
    match &table.rows[row][column] {
        Cell::Number(_) => true,
        Cell::Text(text) => text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || "-+.<>".contains(c)),
        Cell::Empty => false,
    }
}

pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The attributes a plugin's decoration gives a body cell: its colours, bold, italic and
/// borders as a style, its comment as a title that shows on hover. Empty without one.
pub fn cell_attributes(table: &Table, row: usize, column: usize) -> String {
    let d = table.decoration(row, column);
    attributes(d, d)
}

/// `cell_attributes` of a row label spanning `rows` rows: its bottom border is the last row's.
pub fn label_attributes(table: &Table, row: usize, column: usize, rows: usize) -> String {
    let last = table.decoration(row + rows.max(1) - 1, column);
    attributes(table.decoration(row, column), last)
}

/// The attributes of a header cell on `line` of `header_rows` spanning `span` columns from
/// `first`: its right border is the last column's.
pub fn header_attributes(table: &Table, line: usize, first: usize, span: usize) -> String {
    let last = table.header_decoration(line, first + span.max(1) - 1);
    attributes(table.header_decoration(line, first), last)
}

/// The font a plugin gave the table, as a style of the whole table. Empty without one.
pub fn table_attributes(table: &Table) -> String {
    let mut style = String::new();
    if let Some(font) = &table.style.font {
        // set_decorations lets only letters, digits, spaces and hyphens through
        style.push_str(&format!("font-family:'{font}';"));
    }
    if let Some(size) = table.style.font_size {
        style.push_str(&format!("font-size:{size}pt;"));
    }
    match style.is_empty() {
        true => style,
        false => format!(" style=\"{style}\""),
    }
}

/// The style of a cell merged from `first` to `last` (its top left and bottom right places):
/// colours, bold, italic, alignment, the top and left border and the comment from the first, the bottom
/// and right border from the last.
fn attributes(first: Option<&Decoration>, last: Option<&Decoration>) -> String {
    let mut style = String::new();
    if let Some(d) = first {
        for (property, colour) in [("background", &d.fill), ("color", &d.text)] {
            if let Some(colour) = colour {
                style.push_str(&format!("{property}:{colour};"));
            }
        }
        if d.bold {
            style.push_str("font-weight:bold;");
        }
        if d.italic {
            style.push_str("font-style:italic;");
        }
        if let Some(align) = &d.align {
            // set_decorations keeps only left, center and right
            style.push_str(&format!("text-align:{align};"));
        }
    }
    let border = |d: Option<&Decoration>| d.and_then(|d| d.border.clone()).unwrap_or_default();
    let (top, bottom) = (border(first), border(last));
    let sides = [
        ("top", top.top),
        ("bottom", bottom.bottom),
        ("left", top.left),
        ("right", bottom.right),
    ];
    for (side, colour) in sides {
        if let Some(colour) = colour {
            style.push_str(&format!("border-{side}:1px solid {colour};"));
        }
    }
    let mut attributes = String::new();
    if !style.is_empty() {
        attributes.push_str(&format!(" style=\"{style}\""));
    }
    if let Some(comment) = first.and_then(|d| d.comment.as_ref()) {
        attributes.push_str(&format!(" title=\"{}\"", escape(comment)));
    }
    attributes
}

/// The style sheet of a page of tables; a plain table keeps only its plugin's lines and fills.
pub const STYLE: &str = "body{font-family:system-ui,sans-serif;color:#1d2433;margin:2rem;line-height:1.4}\
h2{font-size:1rem;color:#4a5468;font-weight:600;margin:2rem 0 .5rem}\
table{border-collapse:collapse;margin:.5rem 0 1.5rem;font-variant-numeric:tabular-nums}\
caption{text-align:left;font-weight:600;padding:.25rem 0}\
th,td{padding:.25rem .6rem;border-bottom:1px solid #dde2ea}\
thead th{background:#f3f5f8;text-align:center}\
tbody th{text-align:left;font-weight:normal;vertical-align:top}td{text-align:right}\
td.text{text-align:left}p.note{font-size:.85rem;color:#4a5468}\
table.plain th,table.plain td{border:none}table.plain thead th{background:none;font-weight:normal}";

/// One table: a repeated row label spans its rows; with `notes` the caption follows. With
/// stacked row labels (`Table::stacked`) the outer row label column is left out and each
/// block's label is a row of its own above the block, one cell across the table, styled and
/// commented as the block's first row in column 0 was.
pub fn table_html(table: &Table, notes: bool) -> String {
    let class = match table.style.plain {
        true => " class=\"plain\"",
        false => "",
    };
    let mut html = format!("<table{class}{}>\n", table_attributes(table));
    if !table.title.is_empty() {
        html.push_str(&format!("<caption>{}</caption>\n", escape(&table.title)));
    }
    html.push_str("<thead>\n");
    let shift = usize::from(table.stacked());
    for (line, cells) in header_rows(table).into_iter().enumerate() {
        html.push_str("<tr>");
        for cell in cells.into_iter().filter(|cell| cell.first >= shift) {
            let span = match cell.span {
                1 => String::new(),
                n => format!(" colspan=\"{n}\""),
            };
            let more = header_attributes(table, line, cell.first, cell.span);
            html.push_str(&format!("<th{span}{more}>{}</th>", escape(&cell.label)));
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</thead>\n<tbody>\n");
    let starts = table.block_starts();
    let width = table.columns.len() - shift;
    for (r, spans) in row_spans(table).iter().enumerate() {
        if starts.binary_search(&r).is_ok() {
            let more = cell_attributes(table, r, 0);
            let label = escape(&cell_text(table, r, 0));
            html.push_str(&format!(
                "<tr><th colspan=\"{width}\"{more}>{label}</th></tr>\n"
            ));
        }
        html.push_str("<tr>");
        for c in shift..table.rows[r].len() {
            let text = escape(&cell_text(table, r, c));
            if c < table.row_headers {
                let more = |n| label_attributes(table, r, c, n);
                match spans.get(c).copied().unwrap_or(1) {
                    0 => {}
                    1 => html.push_str(&format!("<th{}>{text}</th>", more(1))),
                    n => html.push_str(&format!("<th rowspan=\"{n}\"{}>{text}</th>", more(n))),
                }
            } else {
                let more = cell_attributes(table, r, c);
                match right_aligned(table, r, c) {
                    true => html.push_str(&format!("<td{more}>{text}</td>")),
                    false => html.push_str(&format!("<td class=\"text\"{more}>{text}</td>")),
                }
            }
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</tbody>\n</table>\n");
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        html.push_str(&format!("<p class=\"note\">{}</p>\n", escape(caption)));
    }
    html
}

/// A picture file inside the page as a data URL, `width` CSS pixels wide.
pub fn figure(bytes: &[u8], media: &str, width: f64, title: &str) -> String {
    format!(
        "<figure><img src=\"data:{media};base64,{}\" width=\"{width}\" alt=\"{}\"></figure>\n",
        base64(bytes),
        escape(title)
    )
}

/// Bytes in base64 with padding, as data URLs take them.
fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, b| n << 8 | u32::from(*b)) << (8 * (3 - chunk.len()));
        for i in 0..4 {
            out.push(match i <= chunk.len() {
                true => DIGITS[(n >> (18 - 6 * i) & 63) as usize] as char,
                false => '=',
            });
        }
    }
    out
}

/// The pieces (a table or a figure) as one page, each run of them under the heading of the
/// command that made them. Without `styling` the page has no style sheet.
pub(crate) fn page<'a>(
    pieces: impl IntoIterator<Item = (&'a str, String)>,
    styling: bool,
) -> String {
    let mut body = String::new();
    let mut heading = None;
    for (command, piece) in pieces {
        if heading != Some(command) {
            body.push_str(&format!("<h2>{}</h2>\n", escape(command)));
            heading = Some(command);
        }
        body.push_str(&piece);
    }
    let style = if styling {
        format!("<style>{STYLE}</style>\n")
    } else {
        String::new()
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Statscraft output</title>\n{style}</head>\n<body>\n{body}</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_pads_the_last_group() {
        let cases = [
            ("", ""),
            ("a", "YQ=="),
            ("ab", "YWI="),
            ("abc", "YWJj"),
            ("chart", "Y2hhcnQ="),
        ];
        for (bytes, text) in cases {
            assert_eq!(super::base64(bytes.as_bytes()), text);
        }
        assert_eq!(super::base64(&[0xFF, 0xFE, 0x00]), "//4A");
    }
}
