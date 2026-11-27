//! FACTOR rotation tables against R on the 80 cases of factor_r.rs, two principal axis factors
//! tied to 1e-14: the factor transformation matrix and the rotation sums of squared loadings of
//! varimax with Kaiser normalization (`stats::varimax`, the columns reflected to a positive sum and
//! ordered by their sums of squares); the pattern and structure matrices, the factor correlations
//! and the rotation sums of direct oblimin with delta -0.5 (`GPArotation::oblimin`, gam = delta;
//! the rotation sums of correlated factors are the column sums of the squared structure loadings,
//! with no percentages); and the factor score coefficient and covariance matrices of the
//! regression, Bartlett and Anderson-Rubin scores (the last for the orthogonal rotation only),
//! from the manual's matrix formulas written out in base R. Scripts in lab/oracles/factor-r3.

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

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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
fn varimax_transformation_rotation_sums_and_regression_scores_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) ECONVERGE(1E-12) ITERATE(1000) RCONVERGE(1E-10) /EXTRACTION=PAF /ROTATION=VARIMAX /SAVE=REG(ALL).",
    );
    check(
        &t,
        &[
            ("Factor Transformation Matrix", "1", "1", 0.7249480312),
            ("Factor Transformation Matrix", "1", "2", 0.6888035657),
            ("Factor Transformation Matrix", "2", "1", -0.6888035657),
            ("Factor Transformation Matrix", "2", "2", 0.7249480312),
            (
                "Total Variance Explained",
                "1",
                "Rotation Sums of Squared Loadings / Total",
                1.793151797,
            ),
            (
                "Total Variance Explained",
                "1",
                "Rotation Sums of Squared Loadings / % of Variance",
                29.88586329,
            ),
            (
                "Total Variance Explained",
                "1",
                "Rotation Sums of Squared Loadings / Cumulative %",
                29.88586329,
            ),
            (
                "Total Variance Explained",
                "2",
                "Rotation Sums of Squared Loadings / Total",
                1.762080142,
            ),
            (
                "Total Variance Explained",
                "2",
                "Rotation Sums of Squared Loadings / % of Variance",
                29.36800237,
            ),
            (
                "Total Variance Explained",
                "2",
                "Rotation Sums of Squared Loadings / Cumulative %",
                59.25386566,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 1",
                -0.1377976856,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 2",
                0.8914366381,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 1",
                -0.002614165087,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 2",
                0.07411173254,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 1",
                0.07883793421,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 2",
                0.05053300352,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 1",
                0.3379017721,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 2",
                0.0485089693,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 1",
                0.5790353616,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 2",
                -0.06986747772,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 1",
                0.101591311,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 2",
                0.02884047427,
            ),
            ("Factor Score Covariance Matrix", "1", "1", 0.8614119636),
            ("Factor Score Covariance Matrix", "1", "2", 0.0005256588905),
            ("Factor Score Covariance Matrix", "2", "1", 0.0005256588905),
            ("Factor Score Covariance Matrix", "2", "2", 0.9415928882),
        ],
    );
}

#[test]
fn varimax_bartlett_scores_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) ECONVERGE(1E-12) ITERATE(1000) RCONVERGE(1E-10) /EXTRACTION=PAF /ROTATION=VARIMAX /SAVE=BART(ALL).",
    );
    check(
        &t,
        &[
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 1",
                -0.1335605675,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 2",
                0.9407082955,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 1",
                -0.01722397724,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 2",
                0.08207961849,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 1",
                0.06101169091,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 2",
                0.06103334916,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 1",
                0.4111637151,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 2",
                0.003221843164,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 1",
                0.6616515284,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 2",
                -0.01009548815,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 1",
                0.120568943,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 2",
                -0.005984103039,
            ),
            ("Factor Score Covariance Matrix", "1", "1", 1.161671398),
            ("Factor Score Covariance Matrix", "1", "2", -0.00109496161),
            ("Factor Score Covariance Matrix", "2", "1", -0.00109496161),
            ("Factor Score Covariance Matrix", "2", "2", 1.064478905),
        ],
    );
}

#[test]
fn varimax_anderson_rubin_scores_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) ECONVERGE(1E-12) ITERATE(1000) RCONVERGE(1E-10) /EXTRACTION=PAF /ROTATION=VARIMAX /SAVE=AR(ALL).",
    );
    check(
        &t,
        &[
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 1",
                -0.1247861419,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 2",
                0.9115324896,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 1",
                -0.0160562384,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 2",
                0.07952393458,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 1",
                0.05655079191,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 2",
                0.05926559348,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 1",
                0.381477692,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 2",
                0.003861405984,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 1",
                0.6138943773,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 2",
                -0.008596281401,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 1",
                0.1118702555,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 2",
                -0.00558342907,
            ),
            ("Factor Score Covariance Matrix", "1", "1", 1.0),
            ("Factor Score Covariance Matrix", "1", "2", 0.0),
            ("Factor Score Covariance Matrix", "2", "1", 1.474514955e-17),
            ("Factor Score Covariance Matrix", "2", "2", 1.0),
        ],
    );
}

#[test]
fn oblimin_structure_correlations_rotation_sums_and_regression_scores_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) ECONVERGE(1E-12) ITERATE(1000) RCONVERGE(1E-10) DELTA(-0.5) /EXTRACTION=PAF /ROTATION=OBLIMIN /SAVE=REG(ALL).",
    );
    check(
        &t,
        &[
            ("Pattern Matrix", "v1", "Factor / 1", -0.06751550771),
            ("Pattern Matrix", "v1", "Factor / 2", 0.9728048817),
            ("Structure Matrix", "v1", "Factor / 1", 0.06235857526),
            ("Structure Matrix", "v1", "Factor / 2", 0.96379124),
            ("Pattern Matrix", "v2", "Factor / 1", -0.06623901853),
            ("Pattern Matrix", "v2", "Factor / 2", 0.6849473124),
            ("Structure Matrix", "v2", "Factor / 1", 0.02520470854),
            ("Structure Matrix", "v2", "Factor / 2", 0.6761040881),
            ("Pattern Matrix", "v3", "Factor / 1", 0.2163457662),
            ("Pattern Matrix", "v3", "Factor / 2", 0.5745488056),
            ("Structure Matrix", "v3", "Factor / 1", 0.293050767),
            ("Structure Matrix", "v3", "Factor / 2", 0.6034319953),
            ("Pattern Matrix", "v4", "Factor / 1", 0.8137932136),
            ("Pattern Matrix", "v4", "Factor / 2", 0.0662791365),
            ("Structure Matrix", "v4", "Factor / 1", 0.8226417938),
            ("Structure Matrix", "v4", "Factor / 2", 0.1749244048),
            ("Pattern Matrix", "v5", "Factor / 1", 0.8830486447),
            ("Pattern Matrix", "v5", "Factor / 2", 0.02072152247),
            ("Structure Matrix", "v5", "Factor / 1", 0.8858150666),
            ("Structure Matrix", "v5", "Factor / 2", 0.1386127205),
            ("Pattern Matrix", "v6", "Factor / 1", 0.5365410587),
            ("Pattern Matrix", "v6", "Factor / 2", -0.03362850912),
            ("Structure Matrix", "v6", "Factor / 1", 0.5320514926),
            ("Structure Matrix", "v6", "Factor / 2", 0.03800227654),
            ("Factor Correlation Matrix", "1", "1", 1.0),
            ("Factor Correlation Matrix", "1", "2", 0.1335047607),
            ("Factor Correlation Matrix", "2", "1", 0.1335047607),
            ("Factor Correlation Matrix", "2", "2", 1.0),
            (
                "Total Variance Explained",
                "1",
                "Rotation Sums of Squared Loadings / Total",
                1.834889265,
            ),
            (
                "Total Variance Explained",
                "2",
                "Rotation Sums of Squared Loadings / Total",
                1.801396672,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 1",
                -0.07583424705,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 2",
                0.8806603009,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 1",
                0.002516155682,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 2",
                0.07378757767,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 1",
                0.08214311203,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 2",
                0.05552525518,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 1",
                0.3404470653,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 2",
                0.07025768572,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 1",
                0.5728191033,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 2",
                -0.0322782284,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 1",
                0.1033422241,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 2",
                0.03534946233,
            ),
            ("Factor Score Covariance Matrix", "1", "1", 0.8618677665),
            ("Factor Score Covariance Matrix", "1", "2", 0.1210603364),
            ("Factor Score Covariance Matrix", "2", "1", 0.1210603364),
            ("Factor Score Covariance Matrix", "2", "2", 0.9413254527),
        ],
    );
}

#[test]
fn oblimin_bartlett_scores_agree_with_r() {
    let t = run(
        "FACTOR /VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) ECONVERGE(1E-12) ITERATE(1000) RCONVERGE(1E-10) DELTA(-0.5) /EXTRACTION=PAF /ROTATION=OBLIMIN /SAVE=BART(ALL).",
    );
    check(
        &t,
        &[
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 1",
                -0.06820063938,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v1",
                "Factor / 2",
                0.9301028269,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 1",
                -0.01150779752,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v2",
                "Factor / 2",
                0.08079405142,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 1",
                0.06508551823,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v3",
                "Factor / 2",
                0.06485089948,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 1",
                0.4104025518,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v4",
                "Factor / 2",
                0.02980278823,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 1",
                0.6593701875,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v5",
                "Factor / 2",
                0.03271099365,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 1",
                0.1198666806,
            ),
            (
                "Factor Score Coefficient Matrix",
                "v6",
                "Factor / 2",
                0.001824949423,
            ),
            ("Factor Score Covariance Matrix", "1", "1", 1.161055741),
            ("Factor Score Covariance Matrix", "1", "2", 0.1472879132),
            ("Factor Score Covariance Matrix", "2", "1", 0.1472879132),
            ("Factor Score Covariance Matrix", "2", "2", 1.064744001),
        ],
    );
}
