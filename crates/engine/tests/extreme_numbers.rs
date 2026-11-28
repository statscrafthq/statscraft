//! Numbers of extreme size. A sum of squares over values near 1e200 overflows, and near 1e-200
//! vanishes; a command must then give the result its scale-free statistics have, or an empty
//! cell with a warning, never a panic, a hang or a NaN. DISCRIMINANT stopped with a panic on
//! a predictor of 1e150 (the inverse of a Cholesky factor whose columns differ by 1e150),
//! TWOSTEP CLUSTER never returned on one of 1e200 (its standard deviation overflowed), and
//! VARCOMP printed NaN for a response of 1e200. Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x z y.
BEGIN DATA
1 3.92 3.25 8.21
2 8.5 1.18 13.21
3 3.81 2.43 10.03
1 7.26 0.06 14.37
2 1.55 3.81 7.14
3 8.57 1.05 15.87
1 3.18 2.83 9.39
2 8.74 0.19 16.01
3 6.85 4.16 13.18
1 5.05 4.67 7.97
2 7.17 3.95 13.0
3 2.87 4.56 8.86
1 7.09 3.09 15.41
2 5.09 4.67 8.7
3 1.86 2.56 5.92
1 2.11 2.7 7.38
2 4.39 0.97 7.72
3 5.54 0.03 11.91
1 5.36 4.56 8.56
2 6.86 0.54 13.03
3 3.48 3.96 8.73
1 7.71 4.91 11.21
2 5.93 1.84 12.48
3 4.52 1.54 9.13
1 4.83 4.96 10.13
2 4.31 0.86 8.47
3 2.64 0.62 10.02
1 2.46 0.35 4.75
2 6.83 2.83 12.56
3 2.98 4.98 8.22
1 1.04 2.88 5.71
2 1.75 2.53 6.81
3 5.81 0.89 10.7
1 8.22 3.28 11.99
2 2.19 2.46 8.65
3 5.87 1.63 12.18
END DATA.
";

fn run(transform: &str, command: &str) -> (Vec<Table>, Vec<String>) {
    let (commands, errors) = parse(&format!("{DATA}{transform}{command}\n"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    let mut warnings = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
        warnings = engine.warnings().iter().map(|w| w.to_string()).collect();
    }
    (tables, warnings)
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn numbers(t: &Table) -> Vec<f64> {
    t.rows
        .iter()
        .flatten()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

#[test]
fn discriminant_with_a_predictor_of_1e150_gives_the_scale_free_results() {
    let command = "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x z y.";
    let (base, _) = run("", command);
    let (big, _) = run("COMPUTE y = y * 1E150.\n", command);
    for title in ["Eigenvalues", "Wilks' Lambda"] {
        let (a, b) = (numbers(table(&base, title)), numbers(table(&big, title)));
        assert_eq!(a.len(), b.len(), "{title}");
        for (x, y) in a.iter().zip(&b) {
            assert!(
                (x - y).abs() <= 1e-8 * x.abs().max(1e-6),
                "{title}: {x} {y}"
            );
        }
    }
}

#[test]
fn twostep_with_a_variable_of_1e200_or_1e_minus_200_returns_the_clusters_of_the_same_data() {
    let command = "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x z y.";
    let clusters = |transform: &str| {
        let (tables, _) = run(transform, command);
        numbers(table(&tables, "Model Summary"))
    };
    let base = clusters("");
    assert_eq!(clusters("COMPUTE x = x * 1E200.\n"), base);
    assert_eq!(clusters("COMPUTE x = x * 1E-200.\n"), base);
}

#[test]
fn varcomp_shows_an_estimate_beyond_the_range_empty_with_a_warning() {
    let command = "VARCOMP y BY g /RANDOM=g.";
    let (base, _) = run("", command);
    let (large, warnings) = run("COMPUTE y = y * 1E150.\n", command);
    let (a, b) = (
        numbers(table(&base, "Variance Estimates")),
        numbers(table(&large, "Variance Estimates")),
    );
    for (x, y) in a.iter().zip(&b) {
        assert!((x * 1e300 - y).abs() <= 1e-6 * y.abs(), "{x} {y}");
    }
    assert!(warnings.iter().all(|w| !w.contains("beyond the range")));
    let (huge, warnings) = run("COMPUTE y = y * 1E200.\n", command);
    assert!(numbers(table(&huge, "Variance Estimates")).is_empty());
    assert!(
        warnings.iter().any(|w| w.contains("beyond the range")),
        "{warnings:?}"
    );
}

#[test]
fn values_beyond_1e100_get_a_warning_and_ordinary_data_do_not() {
    let command = "DESCRIPTIVES x y.";
    let (_, warnings) = run("", command);
    assert!(
        warnings.iter().all(|w| !w.contains("1E100")),
        "{warnings:?}"
    );
    let (_, warnings) = run("COMPUTE y = y * 1E-150.\n", command);
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("y has values beyond")),
        "{warnings:?}"
    );
}
