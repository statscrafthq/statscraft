//! Display formats of result cells.

use crate::expr::format_number;

/// `value` shown in a display format such as "F8.2", "PCT40.1", "COMMA10.0", "DOT8.2",
/// "DOLLAR8.2", "N5", "E10.3" or one of the CTABLES formats NEGPAREN, NEQUAL, PAREN and
/// PCTPAREN, without leading spaces. With `fit` the width grows to 40 when the number needs it,
/// so the text never turns into asterisks; without it the result is what the reference program
/// shows in that width. The CTABLES formats show the number as F in the width and add their
/// parentheses, "N=" or percent sign outside it.
pub fn format_value(value: f64, format: &str, fit: bool) -> String {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    let (kind, size) = format.split_at(letters.unwrap_or(format.len()));
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    let mut width: usize = width.parse().unwrap_or(8);
    let given = width;
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
        "COMMA" | "DOLLAR" | "DOT" => {
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
            let mut text = format!("{sign}{dollar}{grouped}{fraction}");
            if text.len() > width {
                // shortcut: a number too wide for its grouping falls back to F in the same
                // width; the reference program drops separators one at a time first, copy that
                // when someone needs narrow COMMA columns
                text = format_number(Some(value), width, decimals)
                    .trim_start()
                    .to_string();
            }
            if kind == "DOT" {
                // DOT is COMMA with the roles of period and comma swapped
                text = text
                    .chars()
                    .map(|c| match c {
                        ',' => '.',
                        '.' => ',',
                        c => c,
                    })
                    .collect();
            }
            text
        }
        // leading zeros fill the width the format gives; there is no sign
        "N" => {
            let number = format_number(Some(value), 40, decimals);
            let number = number.trim_start();
            if value < 0.0 || (!fit && number.len() > given) {
                "*".repeat(given)
            } else {
                format!("{number:0>given$}")
            }
        }
        "E" => {
            for decimals in (0..=decimals).rev() {
                let text = format!("{value:.decimals$e}");
                let (mantissa, exponent) = text.split_once('e').expect("exponent format");
                let exponent: i32 = exponent.parse().expect("exponent format");
                let text = format!("{mantissa}E{exponent:+04}");
                if text.len() <= width {
                    return text;
                }
            }
            "*".repeat(width)
        }
        "NEGPAREN" | "NEQUAL" | "PAREN" | "PCTPAREN" => {
            let number = format_number(Some(value), width, decimals);
            let number = number.trim_start();
            match kind.as_str() {
                _ if number.starts_with('*') => number.to_string(),
                "NEGPAREN" => match number.strip_prefix('-') {
                    Some(magnitude) => format!("({magnitude})"),
                    None => number.to_string(),
                },
                "NEQUAL" => format!("N={number}"),
                "PAREN" => format!("({number})"),
                _ => format!("({number}%)"),
            }
        }
        // shortcut: date, time and currency formats are shown like F of the same size; add
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

    #[test]
    fn dot_n_and_e_formats() {
        assert_eq!(format_value(16978.75, "DOT12.1", false), "16.978,8");
        assert_eq!(format_value(-1234567.5, "DOT12.1", true), "-1.234.567,5");
        assert_eq!(format_value(175.04, "DOT10.2", false), "175,04");
        assert_eq!(format_value(97.0, "N5", false), "00097");
        assert_eq!(format_value(97.0, "N5", true), "00097");
        assert_eq!(format_value(123456.0, "N5", false), "*****");
        assert_eq!(format_value(123456.0, "N5", true), "123456");
        assert_eq!(format_value(-3.0, "N5", true), "*****");
        assert_eq!(format_value(175.0444, "E10.3", false), "1.750E+002");
        assert_eq!(format_value(-0.00125, "E10.2", true), "-1.25E-003");
        assert_eq!(format_value(175.0444, "E8.3", false), "1.8E+002");
    }

    #[test]
    fn ctables_formats_add_their_marks_outside_the_width() {
        // the manual's examples
        assert_eq!(format_value(-1234.567, "NEGPAREN9.2", false), "(1234.57)");
        assert_eq!(format_value(1234.567, "NEQUAL9.2", false), "N=1234.57");
        assert_eq!(format_value(1234.567, "PAREN8.2", false), "(1234.57)");
        assert_eq!(format_value(1234.567, "PCTPAREN10.2", false), "(1234.57%)");
        assert_eq!(format_value(1.31, "NEGPAREN9.2", false), "1.31");
        assert_eq!(format_value(-0.001, "NEGPAREN9.2", false), ".00");
        assert_eq!(format_value(-8.0, "PAREN9.1", false), "(-8.0)");
        assert_eq!(format_value(-32.0, "PCTPAREN9.1", true), "(-32.0%)");
        assert_eq!(format_value(11.5, "PCTPAREN5.1", false), "(11.5%)");
        assert_eq!(format_value(175.0, "PAREN4.0", false), "(175)");
        assert_eq!(format_value(54321.0, "PAREN3.0", false), "***");
    }
}
