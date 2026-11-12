//! Result tables as a web page, for OUTPUT EXPORT /HTML: the same layout as the window's
//! export (crates/gui/src/output.rs).

use crate::{Cell, Table, cell_text, header_rows};

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

const STYLE: &str = "body{font-family:system-ui,sans-serif;color:#1d2433;margin:2rem;line-height:1.4}\
h2{font-size:1rem;color:#4a5468;font-weight:600;margin:2rem 0 .5rem}\
table{border-collapse:collapse;margin:.5rem 0 1.5rem;font-variant-numeric:tabular-nums}\
caption{text-align:left;font-weight:600;padding:.25rem 0}\
th,td{padding:.25rem .6rem;border-bottom:1px solid #dde2ea}\
thead th{background:#f3f5f8;text-align:center}\
tbody th{text-align:left;font-weight:normal;vertical-align:top}td{text-align:right}\
td.text{text-align:left}p.note{font-size:.85rem;color:#4a5468}";

/// One table: a repeated row label spans its rows; with `notes` the caption follows.
pub(crate) fn table_html(table: &Table, notes: bool) -> String {
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
