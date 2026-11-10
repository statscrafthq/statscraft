//! Which part of a large grid is visible, so only those cells are drawn, and where the keyboard
//! moves the selected cell.

use eframe::egui::Key;
use std::ops::Range;

/// Where `key` moves the selected cell `at` in a grid of (rows, columns); `page` is the number
/// of rows in view. With `ctrl`, Home and End go to the first and the last cell. Tab moves right
/// and on to the next row, Shift+Tab back.
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
