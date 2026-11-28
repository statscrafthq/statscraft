//! SET DECIMAL=COMMA for the texts a procedure writes with a number in them: a caption such as
//! "R Squared = 0.720", a column header "Subset for alpha = 0.05", a cell "G (S = 2, M = -0.5)",
//! a warning. A number there is written with a period; here it gets the comma, and nothing
//! that came from the user's data changes: a value label "2.5 kg", a string value "3.14", a
//! variable named v1.2 are looked up in the dictionary and the data and kept as they are.

use crate::{Cell, Table};
use statscraft_data::Dataset;
use std::collections::HashSet;

/// How many cases of a string variable are looked at for values that hold a number.
const CASES_LOOKED_AT: usize = 100_000;

/// Whether `text` has a digit, a period and a digit together.
fn has_decimal_point(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit())
}

/// The names, labels, value labels and string values of the data that have a decimal point in
/// them, longest first.
pub(crate) fn user_texts(dataset: Option<&Dataset>) -> Vec<String> {
    let Some(dataset) = dataset else {
        return Vec::new();
    };
    let mut found: HashSet<String> = HashSet::new();
    let mut keep = |text: &str| {
        if has_decimal_point(text) {
            found.insert(text.to_string());
        }
    };
    for variable in dataset.variables() {
        keep(variable.name());
        if let Some(label) = variable.label() {
            keep(label);
        }
        for (_, label) in variable.value_labels() {
            keep(label);
        }
        if let Ok(values) = variable.string() {
            for value in values.iter().take(CASES_LOOKED_AT) {
                keep(value.trim_end());
            }
        }
    }
    let mut texts: Vec<String> = found.into_iter().collect();
    texts.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    texts
}

/// `text` with the period of each number a comma. A number is digits, one period and digits not
/// after a letter or an underscore (v1.2 is a name) and not part of a longer run of digits and
/// periods (2026.11.28 and 1.2.3 are dates and versions). The `protected` texts stay as they are.
pub(crate) fn comma_text(text: &str, protected: &[String]) -> String {
    if !has_decimal_point(text) {
        return text.to_string();
    }
    // the user's texts are put aside under marks and put back
    let mut work = text.to_string();
    let mut put_aside: Vec<&String> = Vec::new();
    for kept in protected {
        if work.contains(kept.as_str()) {
            let mark = format!("\u{1}{}\u{1}", put_aside.len());
            work = work.replace(kept.as_str(), &mark);
            put_aside.push(kept);
        }
    }
    let chars: Vec<char> = work.chars().collect();
    let mut out = String::with_capacity(work.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() || chars[i] == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // the periods that end a sentence are not part of the number
            let mut end = i;
            while end > start && chars[end - 1] == '.' {
                end -= 1;
            }
            let run: String = chars[start..end].iter().collect();
            let before = start.checked_sub(1).map(|b| chars[b]);
            let named = before.is_some_and(|c| c.is_alphabetic() || c == '_');
            let one_point = run.matches('.').count() == 1 && !run.starts_with('.');
            out.push_str(&if one_point && !named {
                run.replace('.', ",")
            } else {
                run
            });
            out.extend(std::iter::repeat_n('.', i - end));
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    for (n, kept) in put_aside.iter().enumerate() {
        out = out.replace(&format!("\u{1}{n}\u{1}"), kept);
    }
    out
}

/// The texts of a table, its title, header, caption and text cells, with the comma.
pub(crate) fn comma_table(table: &mut Table, protected: &[String]) {
    let fix = |text: &mut String| *text = comma_text(text, protected);
    fix(&mut table.title);
    table.columns.iter_mut().for_each(fix);
    table.column_groups.iter_mut().flatten().for_each(fix);
    if let Some(caption) = table.caption.as_mut() {
        fix(caption);
    }
    // LIST shows the values of the data through their own formats, which already follow the
    // setting (and a currency format or DOT keeps its own mark): its cells are the data's
    if table.title == "List" {
        return;
    }
    for cell in table.rows.iter_mut().flatten() {
        if let Cell::Text(text) = cell {
            fix(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::comma_text;

    #[test]
    fn a_number_in_a_sentence_gets_the_comma() {
        let none: &[String] = &[];
        assert_eq!(
            comma_text("R Squared = 0.720 (Adjusted R Squared = -0.006).", none),
            "R Squared = 0,720 (Adjusted R Squared = -0,006)."
        );
        assert_eq!(
            comma_text("G (S = 2, M = -0.5, N = 12)", none),
            "G (S = 2, M = -0,5, N = 12)"
        );
        assert_eq!(
            comma_text("a significance shown as >0.200", none),
            "a significance shown as >0,200"
        );
        assert_eq!(
            comma_text("Subset for alpha = 0.05", none),
            "Subset for alpha = 0,05"
        );
        assert_eq!(
            comma_text("(0.0%) of 3 cases. The end.", none),
            "(0,0%) of 3 cases. The end."
        );
    }

    #[test]
    fn names_dates_and_versions_are_left_alone() {
        let none: &[String] = &[];
        for text in [
            "v1.2",
            "x_2.5",
            "2026.11.28",
            "version 1.2.3",
            "file data.sav",
            "5.",
            ".5",
        ] {
            assert_eq!(comma_text(text, none), text);
        }
    }

    #[test]
    fn the_users_texts_are_kept() {
        let kept = vec!["Under 2.5 kg".to_string(), "3.14".to_string()];
        assert_eq!(
            comma_text("Under 2.5 kg: 0.5 of 3.14 (3.14 is pi), p = 0.05", &kept),
            "Under 2.5 kg: 0,5 of 3.14 (3.14 is pi), p = 0,05"
        );
    }
}
