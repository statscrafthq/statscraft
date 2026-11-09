//! A number literal too large for a double is an error at its position, not infinity.

use statscraft_syntax::{TokenKind, parse, split_commands};

#[test]
fn overflowing_numbers_are_positioned_errors_and_parsing_recovers() {
    for literal in ["1e999".to_string(), "-1e999".to_string(), "9".repeat(400)] {
        let source = format!("COMPUTE value = {literal}.\nCOMPUTE next = 1.");
        let (commands, errors) = parse(&source);
        assert_eq!(errors.len(), 1, "{literal}");
        assert_eq!(errors[0].span.line, 1);
        // the sign is a token of its own, so the error starts at the digits
        let column = 17 + u32::from(literal.starts_with('-'));
        assert_eq!(errors[0].span.column, column, "{literal}");
        assert!(
            errors[0].message.contains("number"),
            "{}",
            errors[0].message
        );
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].first_line, 2);
    }
}

#[test]
fn finite_numbers_and_underflow_keep_their_values() {
    for (literal, expected) in [
        ("1.7976931348623157e308", f64::MAX),
        ("1e-999", 0.0),
        ("2.5e+3", 2500.0),
    ] {
        let commands = split_commands(&format!("COMPUTE value = {literal}.")).unwrap();
        let number = commands[0]
            .tokens
            .iter()
            .find_map(|token| match token.kind {
                TokenKind::Number(value) => Some(value),
                _ => None,
            });
        assert_eq!(number, Some(expected), "{literal}");
    }
}
