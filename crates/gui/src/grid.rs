//! Which part of a large grid is visible, so only those cells are drawn, where the keyboard
//! moves the selected cell, what is selected, and the text a selection copies.

use eframe::egui::Key;
use std::collections::BTreeSet;
use std::ops::Range;

/// What is selected in a grid: a block of cells between the anchor and the cursor, or whole
/// rows picked by their numbers, or whole columns picked by their headers. The cursor is the
/// cell the keyboard moves and Enter edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub cursor: (usize, usize),
    pub anchor: (usize, usize),
    pub rows: BTreeSet<usize>,
    pub columns: BTreeSet<usize>,
}

impl Selection {
    /// One cell.
    pub fn cell(at: (usize, usize)) -> Self {
        Self {
            cursor: at,
            anchor: at,
            ..Default::default()
        }
    }

    /// The block from the anchor to `at` (Shift+click, Shift+arrow).
    pub fn extend_to(&mut self, at: (usize, usize)) {
        self.cursor = at;
        self.rows.clear();
        self.columns.clear();
    }

    /// A click on a row number: that row alone, with Shift the rows from the anchor's to it,
    /// with Ctrl that row added or taken out.
    pub fn click_row(&mut self, row: usize, shift: bool, ctrl: bool) {
        let rows_before = std::mem::take(&mut self.rows);
        self.columns.clear();
        if shift {
            let from = self.anchor.0;
            self.rows = (from.min(row)..=from.max(row)).collect();
        } else if ctrl && !rows_before.is_empty() {
            self.rows = rows_before;
            if !self.rows.remove(&row) {
                self.rows.insert(row);
            }
            self.anchor = (row, 0);
        } else {
            self.rows.insert(row);
            self.anchor = (row, 0);
        }
        self.cursor = (row, self.cursor.1);
    }

    /// A click on a column header, as `click_row` for columns.
    pub fn click_column(&mut self, column: usize, shift: bool, ctrl: bool) {
        let columns_before = std::mem::take(&mut self.columns);
        self.rows.clear();
        if shift {
            let from = self.anchor.1;
            self.columns = (from.min(column)..=from.max(column)).collect();
        } else if ctrl && !columns_before.is_empty() {
            self.columns = columns_before;
            if !self.columns.remove(&column) {
                self.columns.insert(column);
            }
            self.anchor = (0, column);
        } else {
            self.columns.insert(column);
            self.anchor = (0, column);
        }
        self.cursor = (self.cursor.0, column);
    }

    fn block(&self) -> (Range<usize>, Range<usize>) {
        let (a, c) = (self.anchor, self.cursor);
        (
            a.0.min(c.0)..a.0.max(c.0) + 1,
            a.1.min(c.1)..a.1.max(c.1) + 1,
        )
    }

    pub fn contains(&self, row: usize, column: usize) -> bool {
        if !self.rows.is_empty() {
            return self.rows.contains(&row);
        }
        if !self.columns.is_empty() {
            return self.columns.contains(&column);
        }
        let (rows, columns) = self.block();
        rows.contains(&row) && columns.contains(&column)
    }

    /// More than the cursor's cell is selected.
    pub fn is_wide(&self) -> bool {
        !self.rows.is_empty() || !self.columns.is_empty() || self.anchor != self.cursor
    }

    /// The selected rows and columns of a grid of (rows, columns), in order.
    pub fn span(&self, (rows, columns): (usize, usize)) -> (Vec<usize>, Vec<usize>) {
        if !self.rows.is_empty() {
            let picked = self.rows.iter().copied().filter(|&r| r < rows).collect();
            return (picked, (0..columns).collect());
        }
        if !self.columns.is_empty() {
            let picked = self.columns.iter().copied().filter(|&c| c < columns);
            return ((0..rows).collect(), picked.collect());
        }
        let (r, c) = self.block();
        (
            r.filter(|&r| r < rows).collect(),
            c.filter(|&c| c < columns).collect(),
        )
    }
}

/// The selected part of a grid as tab-separated text with a header row, for a spreadsheet or
/// a text editor. Tabs and line breaks in a cell become blanks.
pub fn copy_text(
    headers: &[String],
    (rows, columns): &(Vec<usize>, Vec<usize>),
    cell: impl Fn(usize, usize) -> String,
) -> String {
    let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
    let line = |texts: Vec<String>| texts.join("\t") + "\n";
    let mut out = line(columns.iter().map(|&c| clean(&headers[c])).collect());
    for &r in rows {
        out.push_str(&line(columns.iter().map(|&c| clean(&cell(r, c))).collect()));
    }
    out
}

/// The next variable after `after` (or the first, from the start) whose name or label holds
/// `needle`, ignoring case, wrapping around at the end. `items` holds name and label.
pub fn find(items: &[(String, String)], needle: &str, after: Option<usize>) -> Option<usize> {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() || items.is_empty() {
        return None;
    }
    let start = after.map_or(0, |a| a + 1);
    (0..items.len())
        .map(|i| (start + i) % items.len())
        .find(|&i| {
            let (name, label) = &items[i];
            name.to_lowercase().contains(&needle) || label.to_lowercase().contains(&needle)
        })
}

/// Where `key` moves the selected cell `at` in a grid of (rows, columns); `page` is the number
/// of rows in view. With `ctrl`, the arrows go to the first or last row or column, Home and End
/// to the first and the last cell. Tab moves right and on to the next row, Shift+Tab back.
pub fn moved(
    key: Key,
    ctrl: bool,
    shift: bool,
    at: (usize, usize),
    (rows, columns): (usize, usize),
    page: usize,
) -> (usize, usize) {
    let (last_row, last_column) = (rows.saturating_sub(1), columns.saturating_sub(1));
    // the data may have shrunk since the cell was selected
    let (r, c) = (at.0.min(last_row), at.1.min(last_column));
    match key {
        Key::ArrowUp if ctrl => (0, c),
        Key::ArrowDown if ctrl => (last_row, c),
        Key::ArrowLeft if ctrl => (r, 0),
        Key::ArrowRight if ctrl => (r, last_column),
        Key::ArrowUp => (r.saturating_sub(1), c),
        Key::ArrowDown => ((r + 1).min(last_row), c),
        Key::ArrowLeft => (r, c.saturating_sub(1)),
        Key::ArrowRight => (r, (c + 1).min(last_column)),
        Key::PageUp => (r.saturating_sub(page), c),
        Key::PageDown => ((r + page).min(last_row), c),
        Key::Home if ctrl => (0, 0),
        Key::End if ctrl => (last_row, last_column),
        Key::Home => (r, 0),
        Key::End => (r, last_column),
        Key::Tab if shift => match (r, c) {
            (0, 0) => (0, 0),
            (r, 0) => (r - 1, last_column),
            (r, c) => (r, c - 1),
        },
        Key::Tab if c < last_column => (r, c + 1),
        Key::Tab if r < last_row => (r + 1, 0),
        _ => (r, c),
    }
}

/// Left edge of every column, plus the total width as the last entry.
pub fn offsets(widths: &[f32]) -> Vec<f32> {
    let mut x = 0.0;
    let mut offsets = Vec::with_capacity(widths.len() + 1);
    offsets.push(0.0);
    for w in widths {
        x += w;
        offsets.push(x);
    }
    offsets
}

/// The rows of equal height that intersect the band from `top` to `bottom`.
pub fn visible_rows(top: f32, bottom: f32, row_height: f32, count: usize) -> Range<usize> {
    let first = ((top / row_height).floor().max(0.0) as usize).min(count);
    let last = ((bottom / row_height).ceil().max(0.0) as usize).min(count);
    first..last.max(first)
}

/// The columns, given by `offsets`, that intersect the band from `left` to `right`.
pub fn visible_columns(offsets: &[f32], left: f32, right: f32) -> Range<usize> {
    let count = offsets.len().saturating_sub(1);
    // a column is visible when its right edge is past `left` and its left edge before `right`
    let first = offsets[1..].partition_point(|&end| end <= left).min(count);
    let last = offsets[..count].partition_point(|&start| start < right);
    first..last.max(first)
}

/// Width of a Data View column: room for the name or the format width, within limits.
pub fn column_width(name: &str, format_width: usize, char_width: f32) -> f32 {
    let chars = name.chars().count().max(format_width) as f32;
    (chars * char_width + 16.0).clamp(64.0, 260.0)
}

/// Room beside the text of a column, in points.
const PADDING: f32 = 12.0;

/// Width in points of a column `chars` characters wide: the display width of a variable.
pub fn width_of(chars: u32, char_width: f32) -> f32 {
    chars as f32 * char_width + PADDING
}

/// The display width in characters that a column dragged to `width` points stands for.
pub fn chars_of(width: f32, char_width: f32) -> u32 {
    ((width - PADDING) / char_width).round().max(1.0) as u32
}

/// The syntax that gives a variable the width of a dragged column.
pub fn width_command(name: &str, width: f32, char_width: f32) -> String {
    format!("VARIABLE WIDTH {name} ({}).", chars_of(width, char_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_rows_in_view_are_visible() {
        assert_eq!(visible_rows(0.0, 100.0, 20.0, 1000), 0..5);
        assert_eq!(visible_rows(30.0, 100.0, 20.0, 1000), 1..5);
        assert_eq!(visible_rows(30.0, 101.0, 20.0, 1000), 1..6);
        assert_eq!(visible_rows(19_990.0, 20_100.0, 20.0, 1000), 999..1000);
        assert_eq!(visible_rows(0.0, 100.0, 20.0, 0), 0..0);
        // 100 000 cases: the view near the end draws a screenful, not the whole column
        let rows = visible_rows(1_999_000.0, 1_999_600.0, 20.0, 100_000);
        assert_eq!(rows, 99_950..99_980);
    }

    #[test]
    fn only_the_columns_in_view_are_visible() {
        let offsets = offsets(&[50.0, 100.0, 50.0, 80.0]);
        assert_eq!(offsets, [0.0, 50.0, 150.0, 200.0, 280.0]);
        assert_eq!(visible_columns(&offsets, 0.0, 50.0), 0..1);
        assert_eq!(visible_columns(&offsets, 49.0, 151.0), 0..3);
        assert_eq!(visible_columns(&offsets, 50.0, 150.0), 1..2);
        assert_eq!(visible_columns(&offsets, 260.0, 900.0), 3..4);
        assert_eq!(visible_columns(&offsets, 300.0, 900.0), 4..4);
        assert_eq!(visible_columns(&[0.0], 0.0, 100.0), 0..0);
        let wide = super::offsets(&[90.0; 500]);
        assert_eq!(visible_columns(&wide, 9_000.0, 10_000.0), 100..112);
    }

    #[test]
    fn keys_move_the_selection_within_the_grid() {
        let size = (100, 5);
        let go = |key, ctrl, shift, at| moved(key, ctrl, shift, at, size, 20);
        assert_eq!(go(Key::ArrowDown, false, false, (3, 2)), (4, 2));
        assert_eq!(go(Key::ArrowUp, false, false, (0, 2)), (0, 2));
        assert_eq!(go(Key::ArrowRight, false, false, (3, 4)), (3, 4));
        assert_eq!(go(Key::ArrowLeft, false, false, (3, 0)), (3, 0));
        assert_eq!(go(Key::PageDown, false, false, (90, 1)), (99, 1));
        assert_eq!(go(Key::PageUp, false, false, (30, 1)), (10, 1));
        assert_eq!(go(Key::Home, false, false, (30, 3)), (30, 0));
        assert_eq!(go(Key::End, false, false, (30, 1)), (30, 4));
        assert_eq!(go(Key::Home, true, false, (30, 3)), (0, 0));
        assert_eq!(go(Key::End, true, false, (30, 1)), (99, 4));
        assert_eq!(go(Key::Tab, false, false, (30, 3)), (30, 4));
        assert_eq!(go(Key::Tab, false, false, (30, 4)), (31, 0));
        assert_eq!(go(Key::Tab, false, false, (99, 4)), (99, 4));
        assert_eq!(go(Key::Tab, false, true, (31, 0)), (30, 4));
        assert_eq!(go(Key::Tab, false, true, (0, 0)), (0, 0));
        // a selection left outside a grid that shrank comes back inside
        assert_eq!(go(Key::ArrowUp, false, false, (500, 9)), (98, 4));
        // Ctrl with an arrow goes to the edge
        assert_eq!(go(Key::ArrowDown, true, false, (3, 2)), (99, 2));
        assert_eq!(go(Key::ArrowUp, true, false, (30, 2)), (0, 2));
        assert_eq!(go(Key::ArrowRight, true, false, (3, 1)), (3, 4));
        assert_eq!(go(Key::ArrowLeft, true, false, (3, 3)), (3, 0));
    }

    #[test]
    fn a_block_grows_from_its_anchor() {
        let mut s = Selection::cell((2, 1));
        assert!(!s.is_wide() && s.contains(2, 1) && !s.contains(2, 2));
        // Shift+arrows or Shift+click: the block between the anchor and the cursor
        s.extend_to((4, 3));
        assert!(s.is_wide());
        assert!(s.contains(3, 2) && s.contains(4, 3) && !s.contains(5, 3) && !s.contains(1, 1));
        assert_eq!(s.span((100, 10)), (vec![2, 3, 4], vec![1, 2, 3]));
        // up and to the left of the anchor too
        s.extend_to((0, 0));
        assert_eq!(s.span((100, 10)), (vec![0, 1, 2], vec![0, 1]));
        // a grid that shrank keeps only what is still there
        assert_eq!(s.span((1, 1)), (vec![0], vec![0]));
    }

    #[test]
    fn row_numbers_and_headers_select_whole_rows_and_columns() {
        let mut s = Selection::cell((5, 2));
        s.click_row(3, false, false);
        assert_eq!(s.rows, BTreeSet::from([3]));
        assert!(s.contains(3, 9) && !s.contains(5, 2));
        assert_eq!(s.cursor, (3, 2));
        // Shift: a range from the row clicked first
        s.click_row(6, true, false);
        assert_eq!(s.rows, BTreeSet::from([3, 4, 5, 6]));
        s.click_row(1, true, false);
        assert_eq!(s.rows, BTreeSet::from([1, 2, 3]));
        // Ctrl: one row more, or one less
        s.click_row(8, false, true);
        s.click_row(2, false, true);
        assert_eq!(s.rows, BTreeSet::from([1, 3, 8]));
        assert_eq!(s.span((5, 3)), (vec![1, 3], vec![0, 1, 2]));
        // a plain click starts again
        s.click_row(0, false, false);
        assert_eq!(s.rows, BTreeSet::from([0]));

        s.click_column(4, false, false);
        assert!(s.rows.is_empty() && s.columns == BTreeSet::from([4]));
        s.click_column(2, true, false);
        assert_eq!(s.columns, BTreeSet::from([2, 3, 4]));
        s.click_column(3, false, true);
        assert_eq!(s.span((2, 9)), (vec![0, 1], vec![2, 4]));
        assert!(s.contains(1, 4) && !s.contains(1, 3));
        // a cell click or Shift+arrow ends the whole rows or columns
        s.extend_to((1, 1));
        assert!(s.columns.is_empty());
    }

    #[test]
    fn a_selection_copies_as_tab_separated_text_with_its_headers() {
        let headers: Vec<String> = ["Name", "Label", "Measure"].map(String::from).to_vec();
        let cell = |r: usize, c: usize| match c {
            1 if r == 1 => "two\twords\nand a line".to_string(),
            _ => format!("r{r}c{c}"),
        };
        let mut s = Selection::cell((0, 0));
        s.extend_to((1, 1));
        assert_eq!(
            copy_text(&headers, &s.span((5, 3)), cell),
            "Name\tLabel\nr0c0\tr0c1\nr1c0\ttwo words and a line\n"
        );
        s.click_column(2, false, false);
        assert_eq!(
            copy_text(&headers, &s.span((2, 3)), cell),
            "Measure\nr0c2\nr1c2\n"
        );
    }

    #[test]
    fn find_goes_from_match_to_match_by_name_or_label() {
        let items: Vec<(String, String)> = [
            ("resp_id", "Respondent"),
            ("age", "Age in years"),
            ("q1", "Satisfaction"),
            ("q2", "Recommend to a friend"),
        ]
        .map(|(n, l)| (n.to_string(), l.to_string()))
        .to_vec();
        assert_eq!(find(&items, "AGE", None), Some(1));
        assert_eq!(find(&items, "re", None), Some(0));
        assert_eq!(find(&items, "re", Some(0)), Some(3));
        // past the last match it starts again from the top
        assert_eq!(find(&items, "re", Some(3)), Some(0));
        assert_eq!(find(&items, "satis", Some(2)), Some(2));
        assert_eq!(find(&items, "nothing", None), None);
        assert_eq!(find(&items, "  ", None), None);
    }

    #[test]
    fn a_dragged_column_becomes_a_display_width() {
        assert_eq!(width_of(10, 8.0), 92.0);
        assert_eq!(chars_of(92.0, 8.0), 10);
        assert_eq!(chars_of(95.9, 8.0), 10);
        assert_eq!(chars_of(96.1, 8.0), 11);
        // never narrower than one character
        assert_eq!(chars_of(2.0, 8.0), 1);
        assert_eq!(width_command("q1", 140.0, 8.0), "VARIABLE WIDTH q1 (16).");
    }

    #[test]
    fn column_widths_stay_within_limits() {
        assert_eq!(column_width("a", 1, 8.0), 64.0);
        assert_eq!(column_width("monthly_spend", 8, 8.0), 13.0 * 8.0 + 16.0);
        assert_eq!(column_width("comment", 200, 8.0), 260.0);
    }
}
