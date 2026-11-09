//! Display formats of result cells.

use crate::expr::format_number;

/// `value` shown in a display format such as "F8.2", "PCT40.1", "COMMA10.0" or "DOLLAR8.2",
/// without leading spaces. With `fit` the width grows to 40 when the number needs it, so the
/// text never turns into asterisks; without it the result is what the reference program shows
/// in that width.
pub fn format_value(value: f64, format: &str, fit: bool) -> String {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    let (kind, size) = format.split_at(letters.unwrap_or(format.len()));
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    let mut width: usize = width.parse().unwrap_or(8);
    let decimals: usize = decimals.parse().unwrap_or(0);
    if fit {
        width = width.max(40);
    }
    let kind = kind.to_ascii_uppercase();
    match kind.as_str() {
        "PCT" => {
            let number = format_number(Some(value), width.saturating_sub(1), decimals);
            if number.contains('*') || width < 2 {
                "*".repeat(width)
            } else {
                format!("{}%", number.trim_start())
            }
        }
        "COMMA" | "DOLLAR" => {
            let plain = format_number(Some(value), 40, decimals);
            let plain = plain.trim_start();
            let (sign, digits) = match plain.strip_prefix('-') {
                Some(rest) => ("-", rest),
                None => ("", plain),
            };
            let (whole, fraction) = digits.split_at(digits.find('.').unwrap_or(digits.len()));
            let mut grouped = String::new();
            for (i, digit) in whole.chars().enumerate() {
                if i > 0 && (whole.len() - i) % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(digit);
            }
            let dollar = if kind == "DOLLAR" { "$" } else { "" };
            let text = format!("{sign}{dollar}{grouped}{fraction}");
            if text.len() <= width {
                text
            } else {
                // shortcut: a number too wide for its grouping falls back to F in the same
                // width; the reference program drops separators one at a time first, copy that
                // when someone needs narrow COMMA columns
                format_number(Some(value), width, decimals)
                    .trim_start()
                    .to_string()
            }
        }
        // shortcut: date, time and the other formats are shown like F of the same size; add
        // them when a table summarizes a date variable
        _ => format_number(Some(value), width, decimals)
            .trim_start()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_value;

    #[test]
    fn formats_like_the_reference_program() {
        assert_eq!(format_value(67.647, "PCT40.1", false), "67.6%");
        assert_eq!(format_value(0.0, "PCT40.1", false), ".0%");
        assert_eq!(format_value(5440.0, "F3.0", false), "***");
        assert_eq!(format_value(5440.0, "F3.0", true), "5440");
        assert_eq!(format_value(175.0444, "F8.2", false), "175.04");
        assert_eq!(format_value(1234567.5, "COMMA12.1", false), "1,234,567.5");
        assert_eq!(format_value(-1234.0, "DOLLAR9.0", false), "-$1,234");
        assert_eq!(format_value(0.5, "F40.0", false), "1");
        assert_eq!(
            format_value(207.045_000_000_000_02, "DOLLAR10.2", false),
            "$207.05"
        );
    }
}
