//! FACTOR options against base R on the 80 invented cases of factor_r.rs: the descriptive and
//! optional statistics (UNIVARIATE, CORRELATION with its significance, DET, INV, the anti-image
//! covariance and correlation matrices with the measures of sampling adequacy on the diagonal,
//! the reproduced and residual correlations with the count of residuals above 0.05), principal
//! axis factoring with the communalities iterated to 1e-14, image factoring from the manual's
//! formulas (loadings S Omega (Lambda - I) Lambda^-1/2 over the eigenvalues of S^-1 R S^-1,
//! S^2 = diag(1 / r^ii), and the communalities and sums of squares from them), the MINEIGEN rule
//! (components kept while the eigenvalue of the correlation matrix is above the limit), and the
//! analysis of a covariance matrix (raw and rescaled components, communalities, determinant and
//! inverse of the covariance matrix); and, on 100 more invented cases of six variables with
//! missing values, a user-missing code and frequency weights, principal components under
//! MISSING=LISTWISE, PAIRWISE (each pair's own cases), MEANSUB (the mean in place of a missing
//! value), LISTWISE INCLUDE (the user-missing -9 counts) and weights (the cases written out as
//! often as their weight): the descriptives with the analysis N, the correlation matrix, the
//! initial eigenvalues, the components and the communalities. Columns carry the signs R gives them (a positive column
//! sum; the off-diagonal anti-image correlations are the negatives of the partial correlations).
//! Scripts in lab/oracles/factor-r3.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /v1 v2 v3 v4 v5 v6.
BEGIN DATA
-0.63 -0.73 -0.85 1.01 0.49 0.83
0.74 0.52 1.25 -0.26 -0.31 1.58
0.28 0.83 1.07 -0.2 -0.11 0.47
0.37 0.95 -0.45 0.22 -0.12 -1.11
-0.6 0.64 0.09 -0.65 -0.84 0.14
0.89 -0.38 1.84 0.23 -0.27 -0.45
-1.27 -1.11 -0.89 1.1 0.32 1.03
-0.56 -0.96 0.45 0.52 0.86 -0.31
-0.21 0.15 -0.38 0.2 0.35 -0.79
0.34 0.34 1.41 -1.8 -1.52 -0.96
-2.62 -1.22 -1.43 -0.32 0.42 -0.06
0.24 0.84 0.89 0.54 1.56 0.93
-0.25 0.56 0.47 0.46 0.75 0.2
-0.59 -0.28 0.37 -2.06 -2.13 -2.95
-0.17 -0.09 -1.05 -1.83 -0.98 -0.07
-0.2 0.2 0.78 0.66 0.53 0.25
0.68 0.39 -0.32 0.41 -1.1 -0.19
0.94 1.14 1.56 1.39 0.95 0.09
-1.24 0.68 0.16 2.73 2.16 2.09
0 1.09 -1.1 -0.42 -0.15 0.43
-0.31 0.09 -0.05 -0.35 0.81 -0.37
1.84 1.35 -0.13 -0.92 -1.31 -0.66
0.09 1.44 1 1.04 -0.42 1.25
-0.79 -0.91 0.01 0.41 -0.57 -2.24
-1.13 -0.28 -0.17 -1.32 -1.74 0.5
0.98 -0.14 1.17 -0.37 -0.22 0.46
0.21 0.69 -0.87 -0.28 0.17 -0.21
-1.03 -0.98 -0.28 1.52 0.65 2.35
1.59 1.15 -0.7 -0.08 -0.11 -0.68
0.91 0.32 0.63 -0.13 -0.5 1.73
-1.55 -1.83 -0.24 0.76 1.49 0.84
-0.8 -1.39 -0.21 -0.07 0.86 2.26
0.09 1.2 -1.49 -0.72 -0.68 -1.16
-0.87 0.11 -0.75 0.11 -1.45 -1.34
0.53 1.41 -0.89 0.41 -0.16 -0.61
-0.48 0.08 -0.23 -0.53 0.05 0.13
-0.6 -0.2 0.22 -0.05 -1.24 -0.11
0.26 0.42 -0.74 0.81 0.4 0.39
-0.12 -0.3 -0.1 -0.29 0.46 -0.29
1.03 0.81 0.86 1 1.47 -0.01
-0.53 -0.55 -0.94 0.77 0.71 0.07
-0.58 -1.13 -0.73 0.82 1.51 0.73
-0.34 -2.23 -1.55 -0.48 -0.39 -0.86
-0.55 0.08 0.38 0.47 1.33 1.48
0.27 1.64 0.31 -0.12 -2.14 -1.03
0.76 0.73 -0.68 -0.01 0.02 -0.75
-0.41 -0.96 1.04 1.17 0.52 -1.38
1.17 0.93 0.61 0.75 0.93 0.31
0.81 0.72 2.03 0.32 0.05 0.99
0.59 1.17 0.39 0.91 1.12 0.6
-1.58 -0.67 -1.53 -0.55 -2.07 -0.44
0.35 0.73 0.71 0.13 -0.13 -0.72
2.15 1.65 1.48 -1.52 -0.43 0.8
1.24 0.71 -0.52 0.41 -0.03 -0.26
-2.16 -0.45 -2.82 -1.1 -1.11 0.76
-0.97 -0.45 -0.03 -0.36 -0.24 0.13
-0.2 0.79 1.14 1.21 1.32 0.98
-0.3 0 -0.65 0.37 0.96 0.54
0.75 1.47 1.07 1.24 1.78 -0.29
-1.6 -0.43 -1.08 -1.63 -1.84 0.59
1.1 -0.2 0.63 -0.41 -0.75 -0.26
-0.52 1.25 -0.91 -1.29 -1.1 -1.88
-0.17 -0.24 1.65 -0.11 -0.78 -0.2
0.06 0.74 0.86 0.36 1.32 -0.73
-0.37 -0.07 -0.19 -0.07 -0.49 0.9
-0.28 -0.47 -0.35 -1.39 -0.91 -0.67
-1.09 -0.19 -0.34 -0.21 1.21 -0.03
-0.88 0.33 -0.46 -1.15 -1.18 -1.39
0.05 -0.14 0.7 -0.61 -0.07 -0.04
0.95 1.13 0.48 1.81 1.14 0.29
-1.95 -0.59 -1.31 0.76 1.83 0.39
0.55 -0.13 -0.96 -0.06 -0.6 -0.89
0.81 0.13 1.46 0.86 0.66 0.08
-0.16 0 -0.02 1.15 0.84 0.01
0.47 -0.64 -0.42 0.23 1.9 1.03
1.02 1.97 2.14 1.65 1.36 2.82
-0.29 -0.32 0.23 0.33 -0.31 0.5
-0.37 1.16 -0.35 -1.2 -1.64 -0.44
-2.02 -1.73 -1.52 -0.61 -1.04 0.44
1.31 1.48 1.83 0.35 1.69 0.8
END DATA.
";

const DATA2: &str = "DATA LIST FREE /v1 v2 v3 v4 v5 v6 w.
BEGIN DATA.
-1 -0.93 0.07 0.68 -0.01 -2.22 2
0.9 0.24 0.83 1.67 -0.09 0.36 2
0.68 1.71 0.32 -0.48 0.58 0.38 1
0.57 0.49 0.88 . 0.48 0.24 2
-0.75 0.56 -0.8 0.62 1.65 -0.85 1
-1.03 -0.61 -0.04 . -9 -0.58 1
-0.04 . 0.46 0.38 0.51 0.54 1
-0.35 -1.68 -0.07 1.46 1.02 2.29 1
-0.02 -0.48 0.46 1.33 0.57 0.94 3
0.29 -0.6 0.15 -1.16 0.11 -1.2 1
0.84 0.92 1.29 1.59 0.26 0.94 1
0.83 -0.7 1.05 2.14 1.22 0.18 1
0.37 0.63 -0.56 -1.59 -1.36 -0.73 2
. -0.6 -1.03 -0.71 -0.61 -1.36 3
0.03 0.94 2.09 0.67 0.11 0.64 2
1.7 0.79 1.57 0.36 0.05 0.71 1
-1.72 -1.78 -0.16 1.13 . 1.5 1
0.18 0.74 -0.45 1.3 0.31 -0.26 3
0.21 -1.3 -0.21 0.13 . -0.92 3
-0.41 1.45 0.91 0.94 1.85 . 3
0.77 0.56 -0.61 0.22 0.89 0.21 1
-9 -0.1 -0.43 0.62 -0.12 0.78 2
. -0.79 0.9 -0.11 -0.46 -1.79 2
. 1.68 0.78 1.15 1.64 -1.15 1
1.36 1.45 0.39 -1.29 -1.09 0.25 1
-1.38 -0.43 -0.7 0.6 -0.11 -1.04 2
-0.96 -0.4 -1.44 -1.07 -0.83 . 1
0.8 -0.21 0.28 -0.22 0.18 1.41 3
-1.06 -0.53 2.01 0.73 0.69 -1.42 1
0.91 . . -0.19 2.01 -0.36 1
-0.11 0.9 0.4 0.81 -0.38 -1.13 1
-0.02 -0.31 -0.69 -1.05 -2.28 -0.9 2
1.75 1.81 1.12 -0.52 -1.09 0.35 1
-0.35 0.82 0.69 0.28 -0.52 -0.21 1
-0.75 0.73 -2.3 -0.46 -0.51 0.19 1
-0.06 0.92 -0.05 0.23 -0.82 -0.64 2
1.35 0.37 2.22 -1.46 -1.59 -0.21 2
0.4 1.77 -0.78 0.84 0.37 -0.06 1
-0.2 -0.6 -0.01 -0.04 0.67 0.07 3
1.73 1.11 1.09 -0.94 -1.42 0.33 2
-0.07 -9 -0.66 1.74 0.01 -0.39 1
0.77 0.06 -0.24 -1.49 -1.44 -1.86 3
-0.1 . -0.38 -0.69 -0.25 -0.4 1
-1.51 -0.96 -0.73 1.03 0.93 0.05 2
-0.67 -0.56 0.26 -1.25 -0.59 0.26 2
0.4 0.22 1.69 1.29 -9 0.94 1
-0.23 0.16 -0.5 -0.3 -0.93 -0.18 3
-1.63 -1.03 -0.9 -0.55 0.65 -1.51 3
1.11 0.8 0.47 1.34 1.99 1.62 1
-1.3 -1.03 1.32 0.86 0.48 0.48 1
-1.77 -2.15 -0.83 -1.48 -0.6 0.06 1
1.48 0.33 0.97 0.82 0.73 -0.36 1
1.24 0.89 1.84 0.37 0.85 -0.8 2
1.13 1.78 -0.54 -1.29 -0.63 -0.45 2
-0.22 0.25 0.39 0.39 0.77 0.12 2
-0.83 -1.47 0.11 -1.59 -0.25 -0.46 1
-1.9 0.28 -0.16 0.32 0.14 -0.41 1
-0.36 -0.67 0.11 0.74 0.14 0.07 2
0.8 0.66 . -0.39 0.32 1.16 3
1.07 1.28 1.12 0.99 -0.35 1.77 3
1.42 1 1.04 -1.07 0.2 0.29 1
-0.23 0.9 0.59 0.69 . -0.35 1
0.04 1.71 -0.49 -0.35 -0.04 -1 1
-0.15 0.79 -1.23 0.52 0.7 0.17 3
-0.59 -0.65 -1.1 -0.26 -1.02 -0.29 1
-0.8 -1.84 -2.09 -0.81 -0.74 -0.93 1
-1.37 -1.31 -1.04 -0.93 . -2.08 1
-0.08 0.51 1.08 0.97 0.82 -0.51 2
-1.12 -2.11 -1.77 -0.88 -0.14 -1.88 2
-0.06 -0.99 0.3 0.57 . 0.83 1
0.4 0.77 -0.27 -1.64 -1.25 -0.7 3
0.64 2.47 0.53 -0.81 -0.51 -0.27 2
0.73 0.85 -0.39 1.14 0.07 0.34 2
-0.7 -0.38 0.4 -0.8 -0.42 0.15 1
1.42 2.01 1.35 1.63 1.46 1.71 1
-0.17 -1.28 -0.08 1.06 1.64 -0.24 1
-0.79 -1.08 -0.23 0.33 0.32 0.11 1
-1.02 -0.97 -1.19 -1.2 -0.22 -2.28 3
0.63 0.75 -1.09 -1.02 -1.23 -0.2 3
0.22 0.23 -0.55 0.6 -0.17 -0.19 1
-1.07 -1.59 -1.23 -0.33 -0.46 -0.07 1
-1.85 -1.69 -1.7 0.07 -0.23 -0.46 1
-0.65 -1.81 0.37 -0.14 -0.98 0.61 3
0.15 -0.94 0.32 -0.96 -1.36 0.48 3
-1.75 -1.01 -0.84 -1.12 -0.59 -1.15 2
0.91 1.96 1.24 0.4 2.5 -0.79 2
-0.18 0.36 -0.22 . 2.23 -0.31 1
0.79 0.8 -0.48 . -1.14 -1.05 1
-1.67 -1.75 -2.17 -1.4 -1.72 0.14 3
-0.11 -1.2 1.68 2.51 2.49 1.72 3
0.47 0.12 0.53 . -0.03 0.26 1
-9 -0.41 1.36 -0.42 0.27 -0.1 1
-0.82 -0.81 1.49 0.17 -0.4 . 2
-0.45 1.49 1.68 0.38 0.21 -0.54 2
-0.82 0.66 1.3 1.43 0.22 1.82 3
1.14 0.27 -0.04 -1.93 0.11 -0.68 3
-1.47 0.67 -2.15 -0.16 -1.25 -0.99 2
0.66 -1.4 -0.51 -2.26 -0.36 2.34 1
0.65 -0.35 0.93 0.22 2.71 -0.06 1
0.87 . 1.33 -1.34 -0.4 -0.45 1
END DATA.
MISSING VALUES v1 TO v6 (-9).
";

fn run(syntax: &str) -> Vec<Table> {
    run_on(DATA, syntax)
}

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

/// The row headers joined with " / ", numbers as they print, an empty header taking the one above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value.
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let t = tables
            .iter()
            .find(|t| t.title == *title)
            .unwrap_or_else(|| panic!("no table {title}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-6 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn descriptive_statistics_matrices_reproduced_correlations_and_paf_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION SIG DET INV REPR AIC KMO /CRITERIA=FACTORS(2) ITERATE(1000) ECONVERGE(1E-12) /EXTRACTION=PAF /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.0865),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                0.9362260627,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 80.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.6536702618),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.5688233352),
            ("Correlation Matrix", "Correlation / v1", "v4", 0.1048089556),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                0.09889309006,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v6",
                -0.02425519033,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v1",
                "v2",
                2.458781954e-11,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v1",
                "v3",
                1.848122859e-08,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v1",
                "v4",
                0.177421194,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v1",
                "v5",
                0.1914003405,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v1",
                "v6",
                0.4154443105,
            ),
            ("Inverse of Correlation Matrix", "v1", "v1", 2.233892544),
            ("Inverse of Correlation Matrix", "v1", "v2", -1.124117506),
            ("Inverse of Correlation Matrix", "v1", "v3", -0.8594759624),
            ("Inverse of Correlation Matrix", "v1", "v4", 0.1423092444),
            ("Inverse of Correlation Matrix", "v1", "v5", -0.1973523045),
            ("Inverse of Correlation Matrix", "v1", "v6", 0.2308322691),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v1",
                0.4476491058,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v2",
                -0.2841924566,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v3",
                -0.2364175456,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v4",
                0.02765328538,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v5",
                -0.036845303,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v1",
                "v6",
                0.07717365167,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v1",
                0.5832839848,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v2",
                -0.5652129824,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v3",
                -0.4507717799,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v4",
                0.06273211417,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v5",
                -0.08527312271,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v1",
                "v6",
                0.1334697312,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v1",
                0.9333706523,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v2",
                0.6560156487,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v3",
                0.5672361195,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v4",
                0.1146262365,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v5",
                0.0750368772,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v1",
                "v6",
                0.001047073483,
            ),
            (
                "Reproduced Correlations",
                "Residual / v1",
                "v2",
                -0.002345386932,
            ),
            (
                "Reproduced Correlations",
                "Residual / v1",
                "v3",
                0.001587215753,
            ),
            (
                "Reproduced Correlations",
                "Residual / v1",
                "v4",
                -0.009817280882,
            ),
            (
                "Reproduced Correlations",
                "Residual / v1",
                "v5",
                0.02385621286,
            ),
            (
                "Reproduced Correlations",
                "Residual / v1",
                "v6",
                -0.02530226381,
            ),
            ("Factor Matrix", "v1", "Factor / 1", 0.6622291327),
            ("Factor Matrix", "v1", "Factor / 2", 0.7034367264),
            ("Communalities", "v1", "Initial", 0.5523508942),
            ("Communalities", "v1", "Extraction", 0.9333706523),
            ("Descriptive Statistics", "v2", "Mean", 0.17275),
            (
                "Descriptive Statistics",
                "v2",
                "Std. Deviation",
                0.8872386048,
            ),
            ("Descriptive Statistics", "v2", "Analysis N", 80.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.6536702618),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.3957363274),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v4",
                0.08484492534,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v5",
                0.02189341979,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v6",
                -0.01619902655,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v2",
                "v1",
                2.458781954e-11,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v2",
                "v3",
                0.0001398641825,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v2",
                "v4",
                0.2271446625,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v2",
                "v5",
                0.4235727418,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v2",
                "v6",
                0.4432961389,
            ),
            ("Inverse of Correlation Matrix", "v2", "v1", -1.124117506),
            ("Inverse of Correlation Matrix", "v2", "v2", 1.770666971),
            ("Inverse of Correlation Matrix", "v2", "v3", -0.06651406488),
            ("Inverse of Correlation Matrix", "v2", "v4", -0.1741466947),
            ("Inverse of Correlation Matrix", "v2", "v5", 0.2259361546),
            ("Inverse of Correlation Matrix", "v2", "v6", -0.01766732363),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v1",
                -0.2841924566,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v2",
                0.5647589391,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v3",
                -0.02308260669,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v4",
                -0.04269276188,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v5",
                0.05321708174,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v2",
                "v6",
                -0.007451928627,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v1",
                -0.5652129824,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v2",
                0.6347248657,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v3",
                -0.03918313411,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v4",
                -0.08622530585,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v5",
                0.1096524638,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v2",
                "v6",
                -0.01147412893,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v1",
                0.6560156487,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v2",
                0.4614261429,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v3",
                0.3939077283,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v4",
                0.0653230159,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v5",
                0.03626688976,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v2",
                "v6",
                -0.009213011494,
            ),
            (
                "Reproduced Correlations",
                "Residual / v2",
                "v1",
                -0.002345386932,
            ),
            (
                "Reproduced Correlations",
                "Residual / v2",
                "v3",
                0.001828599164,
            ),
            (
                "Reproduced Correlations",
                "Residual / v2",
                "v4",
                0.01952190944,
            ),
            (
                "Reproduced Correlations",
                "Residual / v2",
                "v5",
                -0.01437346998,
            ),
            (
                "Reproduced Correlations",
                "Residual / v2",
                "v6",
                -0.006986015058,
            ),
            ("Factor Matrix", "v2", "Factor / 1", 0.4518564608),
            ("Factor Matrix", "v2", "Factor / 2", 0.5072000412),
            ("Communalities", "v2", "Initial", 0.4352410609),
            ("Communalities", "v2", "Extraction", 0.4614261429),
            ("Descriptive Statistics", "v3", "Mean", 0.033625),
            (
                "Descriptive Statistics",
                "v3",
                "Std. Deviation",
                0.9856518036,
            ),
            ("Descriptive Statistics", "v3", "Analysis N", 80.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.5688233352),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.3957363274),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.2707468743),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.2466261018),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.1896716377),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v3",
                "v1",
                1.848122859e-08,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v3",
                "v2",
                0.0001398641825,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v3",
                "v4",
                0.007568699164,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v3",
                "v5",
                0.01371443639,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v3",
                "v6",
                0.04598382406,
            ),
            ("Inverse of Correlation Matrix", "v3", "v1", -0.8594759624),
            ("Inverse of Correlation Matrix", "v3", "v2", -0.06651406488),
            ("Inverse of Correlation Matrix", "v3", "v3", 1.627390408),
            ("Inverse of Correlation Matrix", "v3", "v4", -0.2104090029),
            ("Inverse of Correlation Matrix", "v3", "v5", -0.05942590441),
            ("Inverse of Correlation Matrix", "v3", "v6", -0.2138160154),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v1",
                -0.2364175456,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v2",
                -0.02308260669,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v3",
                0.6144807018,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v4",
                -0.05612398084,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v5",
                -0.01522952146,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v3",
                "v6",
                -0.09812581759,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v1",
                -0.4507717799,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v2",
                -0.03918313411,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v3",
                0.731713281,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v4",
                -0.1086691808,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v5",
                -0.03008368472,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v3",
                "v6",
                -0.1448477522,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v1",
                0.5672361195,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v2",
                0.3939077283,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v3",
                0.4101014249,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v4",
                0.278477677,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v5",
                0.2712821123,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v3",
                "v6",
                0.1369412504,
            ),
            (
                "Reproduced Correlations",
                "Residual / v3",
                "v1",
                0.001587215753,
            ),
            (
                "Reproduced Correlations",
                "Residual / v3",
                "v2",
                0.001828599164,
            ),
            (
                "Reproduced Correlations",
                "Residual / v3",
                "v4",
                -0.007730802751,
            ),
            (
                "Reproduced Correlations",
                "Residual / v3",
                "v5",
                -0.02465601054,
            ),
            (
                "Reproduced Correlations",
                "Residual / v3",
                "v6",
                0.05273038725,
            ),
            ("Factor Matrix", "v3", "Factor / 1", 0.5886242073),
            ("Factor Matrix", "v3", "Factor / 2", 0.2522359362),
            ("Communalities", "v3", "Initial", 0.3855192982),
            ("Communalities", "v3", "Extraction", 0.4101014249),
            ("Descriptive Statistics", "v4", "Mean", 0.075875),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                0.9073758153,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 80.0),
            ("Correlation Matrix", "Correlation / v4", "v1", 0.1048089556),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v2",
                0.08484492534,
            ),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.2707468743),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.7401125877),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.4232416565),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v4",
                "v1",
                0.177421194,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v4",
                "v2",
                0.2271446625,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v4",
                "v3",
                0.007568699164,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v4",
                "v5",
                2.182425729e-15,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v4",
                "v6",
                4.585735392e-05,
            ),
            ("Inverse of Correlation Matrix", "v4", "v1", 0.1423092444),
            ("Inverse of Correlation Matrix", "v4", "v2", -0.1741466947),
            ("Inverse of Correlation Matrix", "v4", "v3", -0.2104090029),
            ("Inverse of Correlation Matrix", "v4", "v4", 2.303690326),
            ("Inverse of Correlation Matrix", "v4", "v5", -1.568861003),
            ("Inverse of Correlation Matrix", "v4", "v6", -0.2025528173),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v1",
                0.02765328538,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v2",
                -0.04269276188,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v3",
                -0.05612398084,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v4",
                0.434086122,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v5",
                -0.2840289303,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v4",
                "v6",
                -0.06566727547,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v1",
                0.06273211417,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v2",
                -0.08622530585,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v3",
                -0.1086691808,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v4",
                0.6293015409,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v5",
                -0.6675342033,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v4",
                "v6",
                -0.1153303588,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v1",
                0.1146262365,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v2",
                0.0653230159,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v3",
                0.278477677,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v4",
                0.6810541475,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v5",
                0.7300574211,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v4",
                "v6",
                0.435498652,
            ),
            (
                "Reproduced Correlations",
                "Residual / v4",
                "v1",
                -0.009817280882,
            ),
            (
                "Reproduced Correlations",
                "Residual / v4",
                "v2",
                0.01952190944,
            ),
            (
                "Reproduced Correlations",
                "Residual / v4",
                "v3",
                -0.007730802751,
            ),
            (
                "Reproduced Correlations",
                "Residual / v4",
                "v5",
                0.01005516659,
            ),
            (
                "Reproduced Correlations",
                "Residual / v4",
                "v6",
                -0.01225699555,
            ),
            ("Factor Matrix", "v4", "Factor / 1", 0.6759666624),
            ("Factor Matrix", "v4", "Factor / 2", -0.4734165384),
            ("Communalities", "v4", "Initial", 0.565913878),
            ("Communalities", "v4", "Extraction", 0.6810541475),
            ("Descriptive Statistics", "v5", "Mean", 0.047125),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                1.063394605,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 80.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                0.09889309006,
            ),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v2",
                0.02189341979,
            ),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.2466261018),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.7401125877),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            ("Correlation Matrix", "Correlation / v5", "v6", 0.4665330708),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v5",
                "v1",
                0.1914003405,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v5",
                "v2",
                0.4235727418,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v5",
                "v3",
                0.01371443639,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v5",
                "v4",
                2.182425729e-15,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v5",
                "v6",
                6.4480619e-06,
            ),
            ("Inverse of Correlation Matrix", "v5", "v1", -0.1973523045),
            ("Inverse of Correlation Matrix", "v5", "v2", 0.2259361546),
            ("Inverse of Correlation Matrix", "v5", "v3", -0.05942590441),
            ("Inverse of Correlation Matrix", "v5", "v4", -1.568861003),
            ("Inverse of Correlation Matrix", "v5", "v5", 2.397716274),
            ("Inverse of Correlation Matrix", "v5", "v6", -0.4444620702),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v1",
                -0.036845303,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v2",
                0.05321708174,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v3",
                -0.01522952146,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v4",
                -0.2840289303,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v5",
                0.4170635244,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v5",
                "v6",
                -0.1384432296,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v1",
                -0.08527312271,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v2",
                0.1096524638,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v3",
                -0.03008368472,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v4",
                -0.6675342033,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v5",
                0.6133449957,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v5",
                "v6",
                -0.248057986,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v1",
                0.0750368772,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v2",
                0.03626688976,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v3",
                0.2712821123,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v4",
                0.7300574211,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v5",
                0.7850900606,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v5",
                "v6",
                0.4706148145,
            ),
            (
                "Reproduced Correlations",
                "Residual / v5",
                "v1",
                0.02385621286,
            ),
            (
                "Reproduced Correlations",
                "Residual / v5",
                "v2",
                -0.01437346998,
            ),
            (
                "Reproduced Correlations",
                "Residual / v5",
                "v3",
                -0.02465601054,
            ),
            (
                "Reproduced Correlations",
                "Residual / v5",
                "v4",
                0.01005516659,
            ),
            (
                "Reproduced Correlations",
                "Residual / v5",
                "v6",
                -0.004081743655,
            ),
            ("Factor Matrix", "v5", "Factor / 1", 0.6959010674),
            ("Factor Matrix", "v5", "Factor / 2", -0.5484630936),
            ("Communalities", "v5", "Initial", 0.5829364756),
            ("Communalities", "v5", "Extraction", 0.7850900606),
            ("Descriptive Statistics", "v6", "Mean", 0.0795),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                0.996098592,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 80.0),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v1",
                -0.02425519033,
            ),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v2",
                -0.01619902655,
            ),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.1896716377),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.4232416565),
            ("Correlation Matrix", "Correlation / v6", "v5", 0.4665330708),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v6",
                "v1",
                0.4154443105,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v6",
                "v2",
                0.4432961389,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v6",
                "v3",
                0.04598382406,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v6",
                "v4",
                4.585735392e-05,
            ),
            (
                "Correlation Matrix",
                "Sig. (1-tailed) / v6",
                "v5",
                6.4480619e-06,
            ),
            ("Inverse of Correlation Matrix", "v6", "v1", 0.2308322691),
            ("Inverse of Correlation Matrix", "v6", "v2", -0.01766732363),
            ("Inverse of Correlation Matrix", "v6", "v3", -0.2138160154),
            ("Inverse of Correlation Matrix", "v6", "v4", -0.2025528173),
            ("Inverse of Correlation Matrix", "v6", "v5", -0.4444620702),
            ("Inverse of Correlation Matrix", "v6", "v6", 1.338952565),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v1",
                0.07717365167,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v2",
                -0.007451928627,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v3",
                -0.09812581759,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v4",
                -0.06566727547,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v5",
                -0.1384432296,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Covariance / v6",
                "v6",
                0.7468524471,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v1",
                0.1334697312,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v2",
                -0.01147412893,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v3",
                -0.1448477522,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v4",
                -0.1153303588,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v5",
                -0.248057986,
            ),
            (
                "Anti-image Matrices",
                "Anti-image Correlation / v6",
                "v6",
                0.7921700407,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v1",
                0.001047073483,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v2",
                -0.009213011494,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v3",
                0.1369412504,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v4",
                0.435498652,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v5",
                0.4706148145,
            ),
            (
                "Reproduced Correlations",
                "Reproduced Correlation / v6",
                "v6",
                0.2841895112,
            ),
            (
                "Reproduced Correlations",
                "Residual / v6",
                "v1",
                -0.02530226381,
            ),
            (
                "Reproduced Correlations",
                "Residual / v6",
                "v2",
                -0.006986015058,
            ),
            (
                "Reproduced Correlations",
                "Residual / v6",
                "v3",
                0.05273038725,
            ),
            (
                "Reproduced Correlations",
                "Residual / v6",
                "v4",
                -0.01225699555,
            ),
            (
                "Reproduced Correlations",
                "Residual / v6",
                "v5",
                -0.004081743655,
            ),
            ("Factor Matrix", "v6", "Factor / 1", 0.3888942448),
            ("Factor Matrix", "v6", "Factor / 2", -0.3646241593),
            ("Communalities", "v6", "Initial", 0.2531475529),
            ("Communalities", "v6", "Extraction", 0.2841895112),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Correlation Matrix")
        .expect("the matrix");
    assert_eq!(table.caption.as_deref(), Some("Determinant = 0.119"));
    let reproduced = t
        .iter()
        .find(|t| t.title == "Reproduced Correlations")
        .expect("the reproduced matrix");
    let note = reproduced.caption.as_deref().expect("a note");
    assert!(
        note.contains(
            "There are 1 (7%) nonredundant residuals with absolute values greater than 0.05"
        ),
        "{note}"
    );
}

#[test]
fn image_factoring_agrees_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(2) /EXTRACTION=IMAGE /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Factor Matrix", "v1", "Factor / 1", 0.4164944712),
            ("Factor Matrix", "v1", "Factor / 2", 0.5557966254),
            ("Communalities", "v1", "Extraction", 0.4823775334),
            ("Factor Matrix", "v2", "Factor / 1", 0.336170195),
            ("Factor Matrix", "v2", "Factor / 2", 0.5194228229),
            ("Communalities", "v2", "Extraction", 0.382810469),
            ("Factor Matrix", "v3", "Factor / 1", 0.4939265998),
            ("Factor Matrix", "v3", "Factor / 2", 0.3194334118),
            ("Communalities", "v3", "Extraction", 0.3460011906),
            ("Factor Matrix", "v4", "Factor / 1", 0.650797052),
            ("Factor Matrix", "v4", "Factor / 2", -0.2914394063),
            ("Communalities", "v4", "Extraction", 0.5084737304),
            ("Factor Matrix", "v5", "Factor / 1", 0.6455299014),
            ("Factor Matrix", "v5", "Factor / 2", -0.3251098334),
            ("Communalities", "v5", "Extraction", 0.5224052574),
            ("Factor Matrix", "v6", "Factor / 1", 0.4057273235),
            ("Factor Matrix", "v6", "Factor / 2", -0.2630895776),
            ("Communalities", "v6", "Extraction", 0.2338307869),
            (
                "Total Variance Explained",
                "1",
                "Extraction Sums of Squared Loadings / Total",
                1.535301848,
            ),
            (
                "Total Variance Explained",
                "1",
                "Extraction Sums of Squared Loadings / % of Variance",
                25.58836413,
            ),
            (
                "Total Variance Explained",
                "1",
                "Extraction Sums of Squared Loadings / Cumulative %",
                25.58836413,
            ),
            (
                "Total Variance Explained",
                "2",
                "Extraction Sums of Squared Loadings / Total",
                0.9405971196,
            ),
            (
                "Total Variance Explained",
                "2",
                "Extraction Sums of Squared Loadings / % of Variance",
                15.67661866,
            ),
            (
                "Total Variance Explained",
                "2",
                "Extraction Sums of Squared Loadings / Cumulative %",
                41.26498279,
            ),
        ],
    );
}

#[test]
fn mineigen_decides_the_number_of_components_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION /CRITERIA=MINEIGEN(0.6) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Component Matrix", "v1", "Component / 1", 0.6067676186),
            ("Component Matrix", "v1", "Component / 2", 0.6735310367),
            ("Component Matrix", "v1", "Component / 3", -0.03452242699),
            ("Communalities", "v1", "Extraction", 0.8230027984),
            ("Component Matrix", "v2", "Component / 1", 0.5172027966),
            ("Component Matrix", "v2", "Component / 2", 0.663733427),
            ("Component Matrix", "v2", "Component / 3", -0.04960647059),
            ("Communalities", "v2", "Extraction", 0.7105015969),
            ("Component Matrix", "v3", "Component / 1", 0.7080295604),
            ("Component Matrix", "v3", "Component / 2", 0.3528190778),
            ("Component Matrix", "v3", "Component / 3", 0.2151022859),
            ("Communalities", "v3", "Extraction", 0.6720561534),
            ("Component Matrix", "v4", "Component / 1", 0.7258504429),
            ("Component Matrix", "v4", "Component / 2", -0.4857463403),
            ("Component Matrix", "v4", "Component / 3", -0.3377640061),
            ("Communalities", "v4", "Extraction", 0.8768928964),
            ("Component Matrix", "v5", "Component / 1", 0.7117837987),
            ("Component Matrix", "v5", "Component / 2", -0.5352605719),
            ("Component Matrix", "v5", "Component / 3", -0.2707802318),
            ("Communalities", "v5", "Extraction", 0.8664619898),
            ("Component Matrix", "v6", "Component / 1", 0.5206193209),
            ("Component Matrix", "v6", "Component / 2", -0.5151536532),
            ("Component Matrix", "v6", "Component / 3", 0.6381016633),
            ("Communalities", "v6", "Extraction", 0.9436014964),
        ],
    );
}

#[test]
fn the_analysis_of_a_covariance_matrix_agrees_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /METHOD=COVARIANCE /PRINT=INITIAL EXTRACTION DET INV /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Covariance Matrix", "", "Determinant", 0.07338276992),
            ("Inverse of Covariance Matrix", "v1", "v1", 2.548594989),
            ("Inverse of Covariance Matrix", "v1", "v2", -1.353289007),
            ("Inverse of Covariance Matrix", "v1", "v3", -0.9313855316),
            ("Inverse of Covariance Matrix", "v1", "v4", 0.1675194299),
            ("Inverse of Covariance Matrix", "v1", "v5", -0.1982289235),
            ("Inverse of Covariance Matrix", "v1", "v6", 0.2475218075),
            ("Component Matrix", "v1", "Raw Component / 1", 0.4528171132),
            ("Component Matrix", "v1", "Raw Component / 2", 0.7161044963),
            (
                "Component Matrix",
                "v1",
                "Rescaled Component / 1",
                0.483662153,
            ),
            (
                "Component Matrix",
                "v1",
                "Rescaled Component / 2",
                0.7648841715,
            ),
            ("Communalities", "v1", "Raw / Initial", 0.8765192405),
            ("Communalities", "v1", "Raw / Extraction", 0.7178489876),
            ("Communalities", "v1", "Rescaled / Initial", 1.0),
            ("Communalities", "v1", "Rescaled / Extraction", 0.818976874),
            ("Inverse of Covariance Matrix", "v2", "v1", -1.353289007),
            ("Inverse of Covariance Matrix", "v2", "v2", 2.249344762),
            ("Inverse of Covariance Matrix", "v2", "v3", -0.07605881217),
            ("Inverse of Covariance Matrix", "v2", "v4", -0.2163154833),
            ("Inverse of Covariance Matrix", "v2", "v5", 0.2394698544),
            ("Inverse of Covariance Matrix", "v2", "v6", -0.01999070029),
            ("Component Matrix", "v2", "Raw Component / 1", 0.3416944607),
            ("Component Matrix", "v2", "Raw Component / 2", 0.6334237522),
            (
                "Component Matrix",
                "v2",
                "Rescaled Component / 1",
                0.3851212728,
            ),
            (
                "Component Matrix",
                "v2",
                "Rescaled Component / 2",
                0.7139271768,
            ),
            ("Communalities", "v2", "Raw / Initial", 0.7871923418),
            ("Communalities", "v2", "Raw / Extraction", 0.5179807544),
            ("Communalities", "v2", "Rescaled / Initial", 1.0),
            ("Communalities", "v2", "Rescaled / Extraction", 0.6580104085),
            ("Inverse of Covariance Matrix", "v3", "v1", -0.9313855316),
            ("Inverse of Covariance Matrix", "v3", "v2", -0.07605881217),
            ("Inverse of Covariance Matrix", "v3", "v3", 1.675115318),
            ("Inverse of Covariance Matrix", "v3", "v4", -0.2352629821),
            ("Inverse of Covariance Matrix", "v3", "v5", -0.05669670585),
            ("Inverse of Covariance Matrix", "v3", "v6", -0.2177781904),
            ("Component Matrix", "v3", "Raw Component / 1", 0.6375920437),
            ("Component Matrix", "v3", "Raw Component / 2", 0.4931923978),
            (
                "Component Matrix",
                "v3",
                "Rescaled Component / 1",
                0.6468735119,
            ),
            (
                "Component Matrix",
                "v3",
                "Rescaled Component / 2",
                0.5003718311,
            ),
            ("Communalities", "v3", "Raw / Initial", 0.9715094778),
            ("Communalities", "v3", "Raw / Extraction", 0.6497623555),
            ("Communalities", "v3", "Rescaled / Initial", 1.0),
            ("Communalities", "v3", "Rescaled / Extraction", 0.6688173099),
            ("Inverse of Covariance Matrix", "v4", "v1", 0.1675194299),
            ("Inverse of Covariance Matrix", "v4", "v2", -0.2163154833),
            ("Inverse of Covariance Matrix", "v4", "v3", -0.2352629821),
            ("Inverse of Covariance Matrix", "v4", "v4", 2.798012816),
            ("Inverse of Covariance Matrix", "v4", "v5", -1.625933637),
            ("Inverse of Covariance Matrix", "v4", "v6", -0.2241035634),
            ("Component Matrix", "v4", "Raw Component / 1", 0.7071029642),
            ("Component Matrix", "v4", "Raw Component / 2", -0.3124274358),
            (
                "Component Matrix",
                "v4",
                "Rescaled Component / 1",
                0.7792834593,
            ),
            (
                "Component Matrix",
                "v4",
                "Rescaled Component / 2",
                -0.3443197741,
            ),
            ("Communalities", "v4", "Raw / Initial", 0.8233308703),
            ("Communalities", "v4", "Raw / Extraction", 0.5976055046),
            ("Communalities", "v4", "Rescaled / Initial", 1.0),
            ("Communalities", "v4", "Rescaled / Extraction", 0.7258388167),
            ("Inverse of Covariance Matrix", "v5", "v1", -0.1982289235),
            ("Inverse of Covariance Matrix", "v5", "v2", 0.2394698544),
            ("Inverse of Covariance Matrix", "v5", "v3", -0.05669670585),
            ("Inverse of Covariance Matrix", "v5", "v4", -1.625933637),
            ("Inverse of Covariance Matrix", "v5", "v5", 2.1203565),
            ("Inverse of Covariance Matrix", "v5", "v6", -0.4196023637),
            ("Component Matrix", "v5", "Raw Component / 1", 0.8556537092),
            ("Component Matrix", "v5", "Raw Component / 2", -0.4431675688),
            (
                "Component Matrix",
                "v5",
                "Rescaled Component / 1",
                0.8046436434,
            ),
            (
                "Component Matrix",
                "v5",
                "Rescaled Component / 2",
                -0.4167479944,
            ),
            ("Communalities", "v5", "Raw / Initial", 1.130808085),
            ("Communalities", "v5", "Raw / Extraction", 0.9285407641),
            ("Communalities", "v5", "Rescaled / Initial", 1.0),
            ("Communalities", "v5", "Rescaled / Extraction", 0.8211302837),
            ("Inverse of Covariance Matrix", "v6", "v1", 0.2475218075),
            ("Inverse of Covariance Matrix", "v6", "v2", -0.01999070029),
            ("Inverse of Covariance Matrix", "v6", "v3", -0.2177781904),
            ("Inverse of Covariance Matrix", "v6", "v4", -0.2241035634),
            ("Inverse of Covariance Matrix", "v6", "v5", -0.4196023637),
            ("Inverse of Covariance Matrix", "v6", "v6", 1.349461626),
            ("Component Matrix", "v6", "Raw Component / 1", 0.5999936417),
            ("Component Matrix", "v6", "Raw Component / 2", -0.425072783),
            (
                "Component Matrix",
                "v6",
                "Rescaled Component / 1",
                0.6023436299,
            ),
            (
                "Component Matrix",
                "v6",
                "Rescaled Component / 2",
                -0.4267376607,
            ),
            ("Communalities", "v6", "Raw / Initial", 0.9922124051),
            ("Communalities", "v6", "Raw / Extraction", 0.5406792409),
            ("Communalities", "v6", "Rescaled / Initial", 1.0),
            ("Communalities", "v6", "Rescaled / Extraction", 0.5449228796),
        ],
    );
}

#[test]
fn listwise_agree_with_r() {
    let t = run_on(
        DATA2,
        "FACTOR /VARIABLES=v1 TO v6 /MISSING=LISTWISE /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.02586666667),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                0.9862830762,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 75.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.6246148098),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.5144410492),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v4",
                0.02175090182,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                0.07457892911,
            ),
            ("Correlation Matrix", "Correlation / v1", "v6", 0.3208522775),
            ("Component Matrix", "v1", "Component / 1", 0.6769337173),
            ("Component Matrix", "v1", "Component / 2", -0.6121745209),
            ("Communalities", "v1", "Extraction", 0.8329969017),
            ("Descriptive Statistics", "v2", "Mean", 0.06066666667),
            (
                "Descriptive Statistics",
                "v2",
                "Std. Deviation",
                1.135502429,
            ),
            ("Descriptive Statistics", "v2", "Analysis N", 75.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.6246148098),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.3551792729),
            ("Correlation Matrix", "Correlation / v2", "v4", 0.1163166148),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v5",
                0.03740189589,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v6",
                0.07696779935,
            ),
            ("Component Matrix", "v2", "Component / 1", 0.5681612918),
            ("Component Matrix", "v2", "Component / 2", -0.5958153577),
            ("Communalities", "v2", "Extraction", 0.677803194),
            ("Descriptive Statistics", "v3", "Mean", 0.06),
            (
                "Descriptive Statistics",
                "v3",
                "Std. Deviation",
                1.067498971,
            ),
            ("Descriptive Statistics", "v3", "Analysis N", 75.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.5144410492),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.3551792729),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.3613006465),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.3275682958),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.3290649885),
            ("Component Matrix", "v3", "Component / 1", 0.7846672001),
            ("Component Matrix", "v3", "Component / 2", -0.1044751017),
            ("Communalities", "v3", "Extraction", 0.6266176618),
            ("Descriptive Statistics", "v4", "Mean", -0.008666666667),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                1.067463948,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 75.0),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v1",
                0.02175090182,
            ),
            ("Correlation Matrix", "Correlation / v4", "v2", 0.1163166148),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.3613006465),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.6445907844),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.3377166154),
            ("Component Matrix", "v4", "Component / 1", 0.6336228307),
            ("Component Matrix", "v4", "Component / 2", 0.6183018056),
            ("Communalities", "v4", "Extraction", 0.7837750144),
            ("Descriptive Statistics", "v5", "Mean", 0.0008),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                1.001490457,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 75.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                0.07457892911,
            ),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v2",
                0.03740189589,
            ),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.3275682958),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.6445907844),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            ("Correlation Matrix", "Correlation / v5", "v6", 0.2248164614),
            ("Component Matrix", "v5", "Component / 1", 0.5859301348),
            ("Component Matrix", "v5", "Component / 2", 0.6207223501),
            ("Communalities", "v5", "Extraction", 0.7286103588),
            ("Descriptive Statistics", "v6", "Mean", -0.0876),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                0.9564882842,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 75.0),
            ("Correlation Matrix", "Correlation / v6", "v1", 0.3208522775),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v2",
                0.07696779935,
            ),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.3290649885),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.3377166154),
            ("Correlation Matrix", "Correlation / v6", "v5", 0.2248164614),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            ("Component Matrix", "v6", "Component / 1", 0.5835412553),
            ("Component Matrix", "v6", "Component / 2", 0.136115333),
            ("Communalities", "v6", "Extraction", 0.3590477805),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.482061537,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                1.526789374,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.8257372969,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.5355004133,
            ),
            (
                "Total Variance Explained",
                "5",
                "Initial Eigenvalues / Total",
                0.4023536772,
            ),
            (
                "Total Variance Explained",
                "6",
                "Initial Eigenvalues / Total",
                0.2275577014,
            ),
        ],
    );
}

#[test]
fn pairwise_agree_with_r() {
    let t = run_on(
        DATA2,
        "FACTOR /VARIABLES=v1 TO v6 /MISSING=PAIRWISE /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.04115789474),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                0.9410031577,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 95.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.6248868196),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.4947014308),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v4",
                0.003004196812,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                0.07846247276,
            ),
            ("Correlation Matrix", "Correlation / v1", "v6", 0.2864802878),
            ("Component Matrix", "v1", "Component / 1", 0.6622305657),
            ("Component Matrix", "v1", "Component / 2", -0.6338674586),
            ("Communalities", "v1", "Extraction", 0.8403372773),
            ("Descriptive Statistics", "v2", "Mean", 0.02242105263),
            (
                "Descriptive Statistics",
                "v2",
                "Std. Deviation",
                1.095393367,
            ),
            ("Descriptive Statistics", "v2", "Analysis N", 95.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.6248868196),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.3406892286),
            ("Correlation Matrix", "Correlation / v2", "v4", 0.1341938031),
            ("Correlation Matrix", "Correlation / v2", "v5", 0.1040588784),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v6",
                0.07093300936,
            ),
            ("Component Matrix", "v2", "Component / 1", 0.5940268565),
            ("Component Matrix", "v2", "Component / 2", -0.5643264149),
            ("Communalities", "v2", "Extraction", 0.6713322088),
            ("Descriptive Statistics", "v3", "Mean", 0.09826530612),
            (
                "Descriptive Statistics",
                "v3",
                "Std. Deviation",
                1.023577741,
            ),
            ("Descriptive Statistics", "v3", "Analysis N", 98.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.4947014308),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.3406892286),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.3466436696),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.3239053032),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.3170110435),
            ("Component Matrix", "v3", "Component / 1", 0.7706740433),
            ("Component Matrix", "v3", "Component / 2", -0.1006194898),
            ("Communalities", "v3", "Extraction", 0.6040627628),
            ("Descriptive Statistics", "v4", "Mean", 0.02431578947),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                1.026165816,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 95.0),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v1",
                0.003004196812,
            ),
            ("Correlation Matrix", "Correlation / v4", "v2", 0.1341938031),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.3466436696),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.6221635972),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.3422571034),
            ("Component Matrix", "v4", "Component / 1", 0.6285556629),
            ("Component Matrix", "v4", "Component / 2", 0.6263407121),
            ("Communalities", "v4", "Extraction", 0.7873849089),
            ("Descriptive Statistics", "v5", "Mean", 0.0552688172),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                1.003784785,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 93.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                0.07846247276,
            ),
            ("Correlation Matrix", "Correlation / v5", "v2", 0.1040588784),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.3239053032),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.6221635972),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            ("Correlation Matrix", "Correlation / v5", "v6", 0.2045022035),
            ("Component Matrix", "v5", "Component / 1", 0.5983656076),
            ("Component Matrix", "v5", "Component / 2", 0.5802620113),
            ("Communalities", "v5", "Extraction", 0.6947454021),
            ("Descriptive Statistics", "v6", "Mean", -0.1196907216),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                0.9516449066,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 97.0),
            ("Correlation Matrix", "Correlation / v6", "v1", 0.2864802878),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v2",
                0.07093300936,
            ),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.3170110435),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.3422571034),
            ("Correlation Matrix", "Correlation / v6", "v5", 0.2045022035),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            ("Component Matrix", "v6", "Component / 1", 0.5602378245),
            ("Component Matrix", "v6", "Component / 2", 0.1635688356),
            ("Communalities", "v6", "Extraction", 0.340621184),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.452345751,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                1.486137993,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.8535917075,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.5615358563,
            ),
            (
                "Total Variance Explained",
                "5",
                "Initial Eigenvalues / Total",
                0.408512555,
            ),
            (
                "Total Variance Explained",
                "6",
                "Initial Eigenvalues / Total",
                0.2378761373,
            ),
        ],
    );
}

#[test]
fn mean_substitution_agree_with_r() {
    let t = run_on(
        DATA2,
        "FACTOR /VARIABLES=v1 TO v6 /MISSING=MEANSUB /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.04115789474),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                0.916932593,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 100.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.6070675011),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.4796256059),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v4",
                0.002967328507,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                0.07402506272,
            ),
            ("Correlation Matrix", "Correlation / v1", "v6", 0.2739846763),
            ("Component Matrix", "v1", "Component / 1", 0.6596442684),
            ("Component Matrix", "v1", "Component / 2", -0.6259664999),
            ("Communalities", "v1", "Extraction", 0.8269646198),
            ("Descriptive Statistics", "v2", "Mean", 0.02242105263),
            ("Descriptive Statistics", "v2", "Std. Deviation", 1.06737355),
            ("Descriptive Statistics", "v2", "Analysis N", 100.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.6070675011),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.3359614727),
            ("Correlation Matrix", "Correlation / v2", "v4", 0.1296080941),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v5",
                0.09641129592,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v6",
                0.06969827267,
            ),
            ("Component Matrix", "v2", "Component / 1", 0.594041046),
            ("Component Matrix", "v2", "Component / 2", -0.5609683365),
            ("Communalities", "v2", "Extraction", 0.6675702388),
            ("Descriptive Statistics", "v3", "Mean", 0.09826530612),
            ("Descriptive Statistics", "v3", "Std. Deviation", 1.01318582),
            ("Descriptive Statistics", "v3", "Analysis N", 100.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.4796256059),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.3359614727),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.3441024136),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.3102796861),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.3061952851),
            ("Component Matrix", "v3", "Component / 1", 0.7684730196),
            ("Component Matrix", "v3", "Component / 2", -0.0904922008),
            ("Communalities", "v3", "Extraction", 0.5987396202),
            ("Descriptive Statistics", "v4", "Mean", 0.02431578947),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                0.9999168174,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 100.0),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v1",
                0.002967328507,
            ),
            ("Correlation Matrix", "Correlation / v4", "v2", 0.1296080941),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.3441024136),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.5868171529),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.3359572108),
            ("Component Matrix", "v4", "Component / 1", 0.6193786788),
            ("Component Matrix", "v4", "Component / 2", 0.6234999659),
            ("Communalities", "v4", "Extraction", 0.7723821552),
            ("Descriptive Statistics", "v5", "Mean", 0.0552688172),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                0.9676469339,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 100.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                0.07402506272,
            ),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v2",
                0.09641129592,
            ),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.3102796861),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.5868171529),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            ("Correlation Matrix", "Correlation / v5", "v6", 0.1887032518),
            ("Component Matrix", "v5", "Component / 1", 0.5790766627),
            ("Component Matrix", "v5", "Component / 2", 0.5778595153),
            ("Communalities", "v5", "Extraction", 0.6692514007),
            ("Descriptive Statistics", "v6", "Mean", -0.1196907216),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                0.9371151235,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 100.0),
            ("Correlation Matrix", "Correlation / v6", "v1", 0.2739846763),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v2",
                0.06969827267,
            ),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.3061952851),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.3359572108),
            ("Correlation Matrix", "Correlation / v6", "v5", 0.1887032518),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            ("Component Matrix", "v6", "Component / 1", 0.552380856),
            ("Component Matrix", "v6", "Component / 2", 0.1717776788),
            ("Communalities", "v6", "Extraction", 0.334632181),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.402650446,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                1.46688977,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.858690684,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.5687722945,
            ),
            (
                "Total Variance Explained",
                "5",
                "Initial Eigenvalues / Total",
                0.4408900942,
            ),
            (
                "Total Variance Explained",
                "6",
                "Initial Eigenvalues / Total",
                0.2621067115,
            ),
        ],
    );
}

#[test]
fn user_missing_included_agree_with_r() {
    let t = run_on(
        DATA2,
        "FACTOR /VARIABLES=v1 TO v6 /MISSING=LISTWISE INCLUDE /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.2482278481),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                1.714684511,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 79.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.2771410235),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.2369025659),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v4",
                0.006603469544,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                -0.008932295126,
            ),
            ("Correlation Matrix", "Correlation / v1", "v6", 0.1229205822),
            ("Component Matrix", "v1", "Component / 1", 0.4152479154),
            ("Component Matrix", "v1", "Component / 2", 0.5800614874),
            ("Communalities", "v1", "Extraction", 0.5089021604),
            ("Descriptive Statistics", "v2", "Mean", -0.06),
            (
                "Descriptive Statistics",
                "v2",
                "Std. Deviation",
                1.504826423,
            ),
            ("Descriptive Statistics", "v2", "Analysis N", 79.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.2771410235),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.304930799),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v4",
                -0.03752130922,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v5",
                0.00275944317,
            ),
            ("Correlation Matrix", "Correlation / v2", "v6", 0.0825139654),
            ("Component Matrix", "v2", "Component / 1", 0.4167898745),
            ("Component Matrix", "v2", "Component / 2", 0.6339518749),
            ("Communalities", "v2", "Extraction", 0.5756087792),
            ("Descriptive Statistics", "v3", "Mean", 0.0817721519),
            (
                "Descriptive Statistics",
                "v3",
                "Std. Deviation",
                1.070555523,
            ),
            ("Descriptive Statistics", "v3", "Analysis N", 79.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.2369025659),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.304930799),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.3399886267),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.1007891568),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.3332487314),
            ("Component Matrix", "v3", "Component / 1", 0.7678907484),
            ("Component Matrix", "v3", "Component / 2", 0.1546180526),
            ("Communalities", "v3", "Extraction", 0.6135629436),
            ("Descriptive Statistics", "v4", "Mean", 0.03265822785),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                1.071156467,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 79.0),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v1",
                0.006603469544,
            ),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v2",
                -0.03752130922,
            ),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.3399886267),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.3360950864),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.3397372306),
            ("Component Matrix", "v4", "Component / 1", 0.6498894236),
            ("Component Matrix", "v4", "Component / 2", -0.5409409306),
            ("Communalities", "v4", "Extraction", 0.7149733533),
            ("Descriptive Statistics", "v5", "Mean", -0.1111392405),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                1.406634452,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 79.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                -0.008932295126,
            ),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v2",
                0.00275944317,
            ),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.1007891568),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.3360950864),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v6",
                0.06517202897,
            ),
            ("Component Matrix", "v5", "Component / 1", 0.3686457222),
            ("Component Matrix", "v5", "Component / 2", -0.5206209228),
            ("Communalities", "v5", "Extraction", 0.4069458138),
            ("Descriptive Statistics", "v6", "Mean", -0.06759493671),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                0.9444102965,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 79.0),
            ("Correlation Matrix", "Correlation / v6", "v1", 0.1229205822),
            ("Correlation Matrix", "Correlation / v6", "v2", 0.0825139654),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.3332487314),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.3397372306),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v5",
                0.06517202897,
            ),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            ("Component Matrix", "v6", "Component / 1", 0.6445097608),
            ("Component Matrix", "v6", "Component / 2", -0.1246642908),
            ("Communalities", "v6", "Extraction", 0.4309340172),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                1.909449595,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                1.341477472,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.9163265821,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.740389151,
            ),
            (
                "Total Variance Explained",
                "5",
                "Initial Eigenvalues / Total",
                0.6078038534,
            ),
            (
                "Total Variance Explained",
                "6",
                "Initial Eigenvalues / Total",
                0.484553346,
            ),
        ],
    );
}

#[test]
fn frequency_weights_agree_with_r() {
    let t = run_on(
        DATA2,
        "WEIGHT BY w.\nFACTOR /VARIABLES=v1 TO v6 /MISSING=LISTWISE /PRINT=UNIVARIATE INITIAL EXTRACTION CORRELATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "v1", "Mean", -0.04268656716),
            (
                "Descriptive Statistics",
                "v1",
                "Std. Deviation",
                0.9444600352,
            ),
            ("Descriptive Statistics", "v1", "Analysis N", 134.0),
            ("Correlation Matrix", "Correlation / v1", "v1", 1.0),
            ("Correlation Matrix", "Correlation / v1", "v2", 0.6246745932),
            ("Correlation Matrix", "Correlation / v1", "v3", 0.5036320686),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v4",
                -0.01764968596,
            ),
            (
                "Correlation Matrix",
                "Correlation / v1",
                "v5",
                0.02409870768,
            ),
            ("Correlation Matrix", "Correlation / v1", "v6", 0.2801200546),
            ("Component Matrix", "v1", "Component / 1", 0.599717339),
            ("Component Matrix", "v1", "Component / 2", 0.6920058453),
            ("Communalities", "v1", "Extraction", 0.8385329767),
            ("Descriptive Statistics", "v2", "Mean", 0.04201492537),
            (
                "Descriptive Statistics",
                "v2",
                "Std. Deviation",
                1.084853879,
            ),
            ("Descriptive Statistics", "v2", "Analysis N", 134.0),
            ("Correlation Matrix", "Correlation / v2", "v1", 0.6246745932),
            ("Correlation Matrix", "Correlation / v2", "v2", 1.0),
            ("Correlation Matrix", "Correlation / v2", "v3", 0.3407398113),
            ("Correlation Matrix", "Correlation / v2", "v4", 0.0957125379),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v5",
                0.03388142294,
            ),
            (
                "Correlation Matrix",
                "Correlation / v2",
                "v6",
                0.08233144291,
            ),
            ("Component Matrix", "v2", "Component / 1", 0.5184355438),
            ("Component Matrix", "v2", "Component / 2", 0.6421311078),
            ("Communalities", "v2", "Extraction", 0.6811077727),
            ("Descriptive Statistics", "v3", "Mean", 0.03276119403),
            (
                "Descriptive Statistics",
                "v3",
                "Std. Deviation",
                1.057766402,
            ),
            ("Descriptive Statistics", "v3", "Analysis N", 134.0),
            ("Correlation Matrix", "Correlation / v3", "v1", 0.5036320686),
            ("Correlation Matrix", "Correlation / v3", "v2", 0.3407398113),
            ("Correlation Matrix", "Correlation / v3", "v3", 1.0),
            ("Correlation Matrix", "Correlation / v3", "v4", 0.3992269228),
            ("Correlation Matrix", "Correlation / v3", "v5", 0.3470551065),
            ("Correlation Matrix", "Correlation / v3", "v6", 0.4038629549),
            ("Component Matrix", "v3", "Component / 1", 0.8036880824),
            ("Component Matrix", "v3", "Component / 2", 0.1209806501),
            ("Communalities", "v3", "Extraction", 0.6605508514),
            ("Descriptive Statistics", "v4", "Mean", -0.05634328358),
            (
                "Descriptive Statistics",
                "v4",
                "Std. Deviation",
                1.097044246,
            ),
            ("Descriptive Statistics", "v4", "Analysis N", 134.0),
            (
                "Correlation Matrix",
                "Correlation / v4",
                "v1",
                -0.01764968596,
            ),
            ("Correlation Matrix", "Correlation / v4", "v2", 0.0957125379),
            ("Correlation Matrix", "Correlation / v4", "v3", 0.3992269228),
            ("Correlation Matrix", "Correlation / v4", "v4", 1.0),
            ("Correlation Matrix", "Correlation / v4", "v5", 0.6648541763),
            ("Correlation Matrix", "Correlation / v4", "v6", 0.4366018083),
            ("Component Matrix", "v4", "Component / 1", 0.685258985),
            ("Component Matrix", "v4", "Component / 2", -0.5847299957),
            ("Communalities", "v4", "Extraction", 0.8114890444),
            ("Descriptive Statistics", "v5", "Mean", -0.0902238806),
            (
                "Descriptive Statistics",
                "v5",
                "Std. Deviation",
                1.017113316,
            ),
            ("Descriptive Statistics", "v5", "Analysis N", 134.0),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v1",
                0.02409870768,
            ),
            (
                "Correlation Matrix",
                "Correlation / v5",
                "v2",
                0.03388142294,
            ),
            ("Correlation Matrix", "Correlation / v5", "v3", 0.3470551065),
            ("Correlation Matrix", "Correlation / v5", "v4", 0.6648541763),
            ("Correlation Matrix", "Correlation / v5", "v5", 1.0),
            ("Correlation Matrix", "Correlation / v5", "v6", 0.2317598482),
            ("Component Matrix", "v5", "Component / 1", 0.6026626584),
            ("Component Matrix", "v5", "Component / 2", -0.5732890154),
            ("Communalities", "v5", "Extraction", 0.691862575),
            ("Descriptive Statistics", "v6", "Mean", -0.1089552239),
            (
                "Descriptive Statistics",
                "v6",
                "Std. Deviation",
                1.000739027,
            ),
            ("Descriptive Statistics", "v6", "Analysis N", 134.0),
            ("Correlation Matrix", "Correlation / v6", "v1", 0.2801200546),
            (
                "Correlation Matrix",
                "Correlation / v6",
                "v2",
                0.08233144291,
            ),
            ("Correlation Matrix", "Correlation / v6", "v3", 0.4038629549),
            ("Correlation Matrix", "Correlation / v6", "v4", 0.4366018083),
            ("Correlation Matrix", "Correlation / v6", "v5", 0.2317598482),
            ("Correlation Matrix", "Correlation / v6", "v6", 1.0),
            ("Component Matrix", "v6", "Component / 1", 0.6414561097),
            ("Component Matrix", "v6", "Component / 2", -0.1542597132),
            ("Communalities", "v6", "Extraction", 0.4352619998),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.518598931,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                1.600206289,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.7916317648,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.5105370819,
            ),
            (
                "Total Variance Explained",
                "5",
                "Initial Eigenvalues / Total",
                0.3684492655,
            ),
            (
                "Total Variance Explained",
                "6",
                "Initial Eigenvalues / Total",
                0.2105766678,
            ),
        ],
    );
}
