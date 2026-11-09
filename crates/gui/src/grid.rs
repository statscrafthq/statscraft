//! Which part of a large grid is visible, so only those cells are drawn.

use std::ops::Range;

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
    fn column_widths_stay_within_limits() {
        assert_eq!(column_width("a", 1, 8.0), 64.0);
        assert_eq!(column_width("monthly_spend", 8, 8.0), 13.0 * 8.0 + 16.0);
        assert_eq!(column_width("comment", 200, 8.0), 260.0);
    }
}
