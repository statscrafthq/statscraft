//! DISCRIMINANT options against base R on the 60 cases of discriminant_r.rs (three groups, four
//! predictors): the group means, standard deviations and counts, the univariate tests of equality
//! of group means (Wilks' lambda and F), the pooled within-groups covariance and correlation
//! matrices, the group and total covariance matrices, the log determinants and Box's M with its F
//! approximation; and the stepwise methods WILKS, MAHAL, MAXMINF, MINRESID and RAO with the
//! probabilities PIN 0.15 and POUT 0.20, written out in base R from the manual's formulas: the
//! variable of each step (the five criteria enter x3 first and then differ: Wilks, minimum
//! residual and Rao's V take x2 then x1, the Mahalanobis and smallest F criteria x1 then x2), the
//! criterion after the step, the tolerances, F to enter and to remove, the criterion with each
//! variable out or in, and the overall Wilks' lambda with its F (exact for q = 1 or three groups,
//! else Rao's approximation). Scripts in lab/oracles/discriminant-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x1 x2 x3 x4.
BEGIN DATA
1 3.63 10.95 0.98 7.84
1 5.3 11.14 2.65 7.95
1 4.52 10.47 3.7 9.47
1 4.8 9.9 1.58 6.79
1 5.07 10.55 3.01 9.14
1 4.6 11.42 2.11 8.33
1 5.04 10.91 2.32 6.62
1 6.07 11.77 2.83 7.59
1 7.24 9.83 1.2 5.03
1 5.28 11.55 3.67 6.17
1 5.23 9.75 2.66 7.42
1 4.82 8.88 2.09 7.52
1 5.51 10.35 1.91 5.83
1 5.8 11.36 3.13 6.1
1 5.07 10.71 3.92 6.95
1 5.45 10.9 4.01 6.93
1 4.82 10.25 3.56 7.95
1 4.47 9.67 1.02 9.14
2 4.5 10.58 5.02 9.43
2 5.18 9.69 4.22 9.38
2 6.11 10.49 3.08 7.68
2 5.28 10.73 2.6 6.82
2 6.93 10.84 2.67 8.47
2 6.03 9.15 2.94 6.47
2 7.11 11.36 3.53 6.53
2 7.06 11.57 2.01 8.31
2 4.88 9.84 3.12 7.4
2 5.38 10.63 0.72 7.68
2 6.02 9.04 2.92 7.89
2 7.15 9.91 2.82 8.68
2 7.36 9.64 3.96 7.15
2 6.45 10.83 2.07 5.79
2 6.69 11.23 3.69 10.42
2 7.81 11.68 2.7 6.18
2 6.78 11.49 4.07 5.84
2 4.73 10.6 4.7 6.37
2 3.57 12.26 2.92 6.73
2 7.41 10.76 3.48 4.97
2 8.52 10.32 3.14 5.92
2 7.26 10.93 3.91 7.5
3 4.56 8.31 4.06 7.53
3 6.44 9.58 4.91 7.54
3 7.13 8.65 3.92 6.19
3 5.02 8.94 3.55 8.76
3 5.83 10.92 3.6 7.95
3 4.39 7.49 5.28 5.24
3 5.87 9.28 4.5 8.57
3 5.75 9.59 3.04 8.08
3 7.33 11.88 7.16 7.42
3 6.96 8.95 4.2 6.61
3 4.97 10.7 5.69 10.32
3 4.67 9.32 4.45 7.75
3 5.29 8.79 6.06 9.07
3 5.81 7.33 5.22 7.16
3 4.91 9.55 2.96 7.15
3 6.49 9.85 3.23 6.09
3 5.7 10.22 3.35 8.08
3 5.68 8.22 2.41 8.04
3 7 8.83 4.61 9.01
3 5.14 10.73 3.09 6.99
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

fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{n}"),
        Cell::Empty => String::new(),
    }
}

/// Each expectation: the table title, the row, the column (a title, or #n for the column at
/// position n) and the value.
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
        let c = match column.strip_prefix('#') {
            Some(n) => n.parse::<usize>().expect("a position"),
            None => t
                .column_titles()
                .iter()
                .position(|k| k == column)
                .unwrap_or_else(|| {
                    panic!("{title}: no column {column} in {:?}", t.column_titles())
                }),
        };
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
fn group_statistics_equality_of_means_matrices_log_determinants_and_boxs_m_agree_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /STATISTICS=MEAN STDDEV UNIVF BOXM GCOV COV CORR TCOV.",
    );
    check(
        &t,
        &[
            ("Group Statistics", "1 / x1", "Mean", 5.151111111),
            ("Group Statistics", "1 / x1", "Std. Deviation", 0.7569201441),
            (
                "Group Statistics",
                "1 / x1",
                "Valid N (listwise) / Unweighted",
                18.0,
            ),
            (
                "Group Statistics",
                "1 / x1",
                "Valid N (listwise) / Weighted",
                18.0,
            ),
            ("Group Statistics", "1 / x2", "Mean", 10.57555556),
            ("Group Statistics", "1 / x2", "Std. Deviation", 0.7652775138),
            (
                "Group Statistics",
                "1 / x2",
                "Valid N (listwise) / Unweighted",
                18.0,
            ),
            (
                "Group Statistics",
                "1 / x2",
                "Valid N (listwise) / Weighted",
                18.0,
            ),
            ("Group Statistics", "1 / x3", "Mean", 2.575),
            ("Group Statistics", "1 / x3", "Std. Deviation", 0.9905331304),
            (
                "Group Statistics",
                "1 / x3",
                "Valid N (listwise) / Unweighted",
                18.0,
            ),
            (
                "Group Statistics",
                "1 / x3",
                "Valid N (listwise) / Weighted",
                18.0,
            ),
            ("Group Statistics", "1 / x4", "Mean", 7.376111111),
            ("Group Statistics", "1 / x4", "Std. Deviation", 1.206180841),
            (
                "Group Statistics",
                "1 / x4",
                "Valid N (listwise) / Unweighted",
                18.0,
            ),
            (
                "Group Statistics",
                "1 / x4",
                "Valid N (listwise) / Weighted",
                18.0,
            ),
            ("Group Statistics", "2 / x1", "Mean", 6.282272727),
            ("Group Statistics", "2 / x1", "Std. Deviation", 1.229397575),
            (
                "Group Statistics",
                "2 / x1",
                "Valid N (listwise) / Unweighted",
                22.0,
            ),
            (
                "Group Statistics",
                "2 / x1",
                "Valid N (listwise) / Weighted",
                22.0,
            ),
            ("Group Statistics", "2 / x2", "Mean", 10.61681818),
            ("Group Statistics", "2 / x2", "Std. Deviation", 0.8256736722),
            (
                "Group Statistics",
                "2 / x2",
                "Valid N (listwise) / Unweighted",
                22.0,
            ),
            (
                "Group Statistics",
                "2 / x2",
                "Valid N (listwise) / Weighted",
                22.0,
            ),
            ("Group Statistics", "2 / x3", "Mean", 3.195),
            ("Group Statistics", "2 / x3", "Std. Deviation", 0.9510833422),
            (
                "Group Statistics",
                "2 / x3",
                "Valid N (listwise) / Unweighted",
                22.0,
            ),
            (
                "Group Statistics",
                "2 / x3",
                "Valid N (listwise) / Weighted",
                22.0,
            ),
            ("Group Statistics", "2 / x4", "Mean", 7.345909091),
            ("Group Statistics", "2 / x4", "Std. Deviation", 1.367227501),
            (
                "Group Statistics",
                "2 / x4",
                "Valid N (listwise) / Unweighted",
                22.0,
            ),
            (
                "Group Statistics",
                "2 / x4",
                "Valid N (listwise) / Weighted",
                22.0,
            ),
            ("Group Statistics", "3 / x1", "Mean", 5.747),
            ("Group Statistics", "3 / x1", "Std. Deviation", 0.8986955459),
            (
                "Group Statistics",
                "3 / x1",
                "Valid N (listwise) / Unweighted",
                20.0,
            ),
            (
                "Group Statistics",
                "3 / x1",
                "Valid N (listwise) / Weighted",
                20.0,
            ),
            ("Group Statistics", "3 / x2", "Mean", 9.3565),
            ("Group Statistics", "3 / x2", "Std. Deviation", 1.148354773),
            (
                "Group Statistics",
                "3 / x2",
                "Valid N (listwise) / Unweighted",
                20.0,
            ),
            (
                "Group Statistics",
                "3 / x2",
                "Valid N (listwise) / Weighted",
                20.0,
            ),
            ("Group Statistics", "3 / x3", "Mean", 4.2645),
            ("Group Statistics", "3 / x3", "Std. Deviation", 1.197648903),
            (
                "Group Statistics",
                "3 / x3",
                "Valid N (listwise) / Unweighted",
                20.0,
            ),
            (
                "Group Statistics",
                "3 / x3",
                "Valid N (listwise) / Weighted",
                20.0,
            ),
            ("Group Statistics", "3 / x4", "Mean", 7.6775),
            ("Group Statistics", "3 / x4", "Std. Deviation", 1.172944451),
            (
                "Group Statistics",
                "3 / x4",
                "Valid N (listwise) / Unweighted",
                20.0,
            ),
            (
                "Group Statistics",
                "3 / x4",
                "Valid N (listwise) / Weighted",
                20.0,
            ),
            ("Group Statistics", "Total / x1", "Mean", 5.7645),
            (
                "Group Statistics",
                "Total / x1",
                "Std. Deviation",
                1.085353063,
            ),
            (
                "Group Statistics",
                "Total / x1",
                "Valid N (listwise) / Unweighted",
                60.0,
            ),
            (
                "Group Statistics",
                "Total / x1",
                "Valid N (listwise) / Weighted",
                60.0,
            ),
            ("Group Statistics", "Total / x2", "Mean", 10.18433333),
            (
                "Group Statistics",
                "Total / x2",
                "Std. Deviation",
                1.088493543,
            ),
            (
                "Group Statistics",
                "Total / x2",
                "Valid N (listwise) / Unweighted",
                60.0,
            ),
            (
                "Group Statistics",
                "Total / x2",
                "Valid N (listwise) / Weighted",
                60.0,
            ),
            ("Group Statistics", "Total / x3", "Mean", 3.3655),
            (
                "Group Statistics",
                "Total / x3",
                "Std. Deviation",
                1.24178579,
            ),
            (
                "Group Statistics",
                "Total / x3",
                "Valid N (listwise) / Unweighted",
                60.0,
            ),
            (
                "Group Statistics",
                "Total / x3",
                "Valid N (listwise) / Weighted",
                60.0,
            ),
            ("Group Statistics", "Total / x4", "Mean", 7.4655),
            (
                "Group Statistics",
                "Total / x4",
                "Std. Deviation",
                1.245234203,
            ),
            (
                "Group Statistics",
                "Total / x4",
                "Valid N (listwise) / Unweighted",
                60.0,
            ),
            (
                "Group Statistics",
                "Total / x4",
                "Valid N (listwise) / Weighted",
                60.0,
            ),
            (
                "Tests of Equality of Group Means",
                "x1",
                "Wilks' Lambda",
                0.8176082013,
            ),
            ("Tests of Equality of Group Means", "x1", "F", 6.357771673),
            ("Tests of Equality of Group Means", "x1", "df1", 2.0),
            ("Tests of Equality of Group Means", "x1", "df2", 57.0),
            (
                "Tests of Equality of Group Means",
                "x1",
                "Sig.",
                0.003217653812,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x1",
                "x1",
                0.9969295463,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x1",
                "x2",
                0.1031480347,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x1",
                "x3",
                0.0314477193,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x1",
                "x4",
                -0.3636345206,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x1",
                "x1",
                1.0,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x1",
                "x2",
                0.1110500147,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x1",
                "x3",
                0.02997584027,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x1",
                "x4",
                -0.2896270656,
            ),
            ("Covariance Matrices", "1 / x1", "x1", 0.5729281046),
            ("Covariance Matrices", "1 / x1", "x2", 0.04226993464),
            ("Covariance Matrices", "1 / x1", "x3", 0.06887058824),
            ("Covariance Matrices", "1 / x1", "x4", -0.5939777778),
            ("Covariance Matrices", "2 / x1", "x1", 1.511418398),
            ("Covariance Matrices", "2 / x1", "x2", 0.01947900433),
            ("Covariance Matrices", "2 / x1", "x3", -0.1322547619),
            ("Covariance Matrices", "2 / x1", "x4", -0.3861616883),
            ("Covariance Matrices", "3 / x1", "x1", 0.8076536842),
            ("Covariance Matrices", "3 / x1", "x2", 0.2500942105),
            ("Covariance Matrices", "3 / x1", "x3", 0.1788984211),
            ("Covariance Matrices", "3 / x1", "x4", -0.1326394737),
            ("Covariance Matrices", "Total / x1", "x1", 1.177991271),
            ("Covariance Matrices", "Total / x1", "x2", 0.114849661),
            ("Covariance Matrices", "Total / x1", "x3", 0.1400612712),
            ("Covariance Matrices", "Total / x1", "x4", -0.3589268644),
            (
                "Tests of Equality of Group Means",
                "x2",
                "Wilks' Lambda",
                0.7056532364,
            ),
            ("Tests of Equality of Group Means", "x2", "F", 11.88810924),
            ("Tests of Equality of Group Means", "x2", "df1", 2.0),
            ("Tests of Equality of Group Means", "x2", "df2", 57.0),
            (
                "Tests of Equality of Group Means",
                "x2",
                "Sig.",
                4.840086523e-05,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x2",
                "x1",
                0.1031480347,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x2",
                "x2",
                0.8654066091,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x2",
                "x3",
                0.1065818421,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x2",
                "x4",
                0.03654960571,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x2",
                "x1",
                0.1110500147,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x2",
                "x2",
                1.0,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x2",
                "x3",
                0.1090404321,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x2",
                "x4",
                0.03124488031,
            ),
            ("Covariance Matrices", "1 / x2", "x1", 0.04226993464),
            ("Covariance Matrices", "1 / x2", "x2", 0.5856496732),
            ("Covariance Matrices", "1 / x2", "x3", 0.2928),
            ("Covariance Matrices", "1 / x2", "x4", -0.05348300654),
            ("Covariance Matrices", "2 / x2", "x1", 0.01947900433),
            ("Covariance Matrices", "2 / x2", "x2", 0.681737013),
            ("Covariance Matrices", "2 / x2", "x3", -0.07672619048),
            ("Covariance Matrices", "2 / x2", "x4", -0.1775374459),
            ("Covariance Matrices", "3 / x2", "x1", 0.2500942105),
            ("Covariance Matrices", "3 / x2", "x2", 1.318718684),
            ("Covariance Matrices", "3 / x2", "x3", 0.1425692105),
            ("Covariance Matrices", "3 / x2", "x4", 0.3537276316),
            ("Covariance Matrices", "Total / x2", "x1", 0.114849661),
            ("Covariance Matrices", "Total / x2", "x2", 1.184818192),
            ("Covariance Matrices", "Total / x2", "x3", -0.2711564407),
            ("Covariance Matrices", "Total / x2", "x4", -0.05413610169),
            (
                "Tests of Equality of Group Means",
                "x3",
                "Wilks' Lambda",
                0.6916726153,
            ),
            ("Tests of Equality of Group Means", "x3", "F", 12.7044649),
            ("Tests of Equality of Group Means", "x3", "df1", 2.0),
            ("Tests of Equality of Group Means", "x3", "df2", 57.0),
            (
                "Tests of Equality of Group Means",
                "x3",
                "Sig.",
                2.736318025e-05,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x3",
                "x1",
                0.0314477193,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x3",
                "x2",
                0.1065818421,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x3",
                "x3",
                1.104005175,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x3",
                "x4",
                0.166370614,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x3",
                "x1",
                0.02997584027,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x3",
                "x2",
                0.1090404321,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x3",
                "x3",
                1.0,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x3",
                "x4",
                0.1259207604,
            ),
            ("Covariance Matrices", "1 / x3", "x1", 0.06887058824),
            ("Covariance Matrices", "1 / x3", "x2", 0.2928),
            ("Covariance Matrices", "1 / x3", "x3", 0.9811558824),
            ("Covariance Matrices", "1 / x3", "x4", 0.09301470588),
            ("Covariance Matrices", "2 / x3", "x1", -0.1322547619),
            ("Covariance Matrices", "2 / x3", "x2", -0.07672619048),
            ("Covariance Matrices", "2 / x3", "x3", 0.9045595238),
            ("Covariance Matrices", "2 / x3", "x4", 0.1663452381),
            ("Covariance Matrices", "3 / x3", "x1", 0.1788984211),
            ("Covariance Matrices", "3 / x3", "x2", 0.1425692105),
            ("Covariance Matrices", "3 / x3", "x3", 1.434362895),
            ("Covariance Matrices", "3 / x3", "x4", 0.2320328947),
            ("Covariance Matrices", "Total / x3", "x1", 0.1400612712),
            ("Covariance Matrices", "Total / x3", "x2", -0.2711564407),
            ("Covariance Matrices", "Total / x3", "x3", 1.542031949),
            ("Covariance Matrices", "Total / x3", "x4", 0.2544980508),
            (
                "Tests of Equality of Group Means",
                "x4",
                "Wilks' Lambda",
                0.9851632806,
            ),
            ("Tests of Equality of Group Means", "x4", "F", 0.4292146398),
            ("Tests of Equality of Group Means", "x4", "df1", 2.0),
            ("Tests of Equality of Group Means", "x4", "df2", 57.0),
            (
                "Tests of Equality of Group Means",
                "x4",
                "Sig.",
                0.6531067372,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x4",
                "x1",
                -0.3636345206,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x4",
                "x2",
                0.03654960571,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x4",
                "x3",
                0.166370614,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Covariance / x4",
                "x4",
                1.581202361,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x4",
                "x1",
                -0.2896270656,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x4",
                "x2",
                0.03124488031,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x4",
                "x3",
                0.1259207604,
            ),
            (
                "Pooled Within-Groups Matrices",
                "Correlation / x4",
                "x4",
                1.0,
            ),
            ("Covariance Matrices", "1 / x4", "x1", -0.5939777778),
            ("Covariance Matrices", "1 / x4", "x2", -0.05348300654),
            ("Covariance Matrices", "1 / x4", "x3", 0.09301470588),
            ("Covariance Matrices", "1 / x4", "x4", 1.454872222),
            ("Covariance Matrices", "2 / x4", "x1", -0.3861616883),
            ("Covariance Matrices", "2 / x4", "x2", -0.1775374459),
            ("Covariance Matrices", "2 / x4", "x3", 0.1663452381),
            ("Covariance Matrices", "2 / x4", "x4", 1.869311039),
            ("Covariance Matrices", "3 / x4", "x1", -0.1326394737),
            ("Covariance Matrices", "3 / x4", "x2", 0.3537276316),
            ("Covariance Matrices", "3 / x4", "x3", 0.2320328947),
            ("Covariance Matrices", "3 / x4", "x4", 1.375798684),
            ("Covariance Matrices", "Total / x4", "x1", -0.3589268644),
            ("Covariance Matrices", "Total / x4", "x2", -0.05413610169),
            ("Covariance Matrices", "Total / x4", "x3", 0.2544980508),
            ("Covariance Matrices", "Total / x4", "x4", 1.55060822),
            ("Log Determinants", "1", "Log Determinant", -1.500359604),
            ("Log Determinants", "2", "Log Determinant", 0.4449769272),
            ("Log Determinants", "3", "Log Determinant", 0.5040364867),
            (
                "Log Determinants",
                "Pooled within-groups",
                "Log Determinant",
                0.2744363842,
            ),
            ("Test Results", "Box's M", "#2", 22.22777845),
            ("Test Results", "F / Approx.", "#2", 0.9966429546),
            ("Test Results", "F / df1", "#2", 20.0),
            ("Test Results", "F / df2", "#2", 11016.18317),
            ("Test Results", "F / Sig.", "#2", 0.4622463406),
        ],
    );
}

#[test]
fn stepwise_wilks_agrees_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /ANALYSIS=ALL /METHOD=WILKS /PIN=0.15 /POUT=0.20.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                6.357771673,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Wilks' Lambda",
                0.8176082013,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                11.88810924,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Wilks' Lambda",
                0.7056532364,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Wilks' Lambda",
                0.6916726153,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                0.4292146398,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Wilks' Lambda",
                0.9851632806,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Wilks' Lambda",
                0.6916726153,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.6916726153),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 57.0),
            ("Wilks' Lambda", "1", "F / Statistic", 12.7044649),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 57.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.736318025e-05),
            ("Variables in the Analysis", "1 / x3", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x3",
                "F to Remove",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                5.906859206,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Wilks' Lambda",
                0.5711774456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                10.54095053,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Wilks' Lambda",
                0.502500145,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                0.09731623627,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Wilks' Lambda",
                0.6892769781,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Wilks' Lambda",
                0.502500145,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.502500145),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 57.0),
            ("Wilks' Lambda", "2", "F / Statistic", 11.49934879),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 112.0),
            ("Wilks' Lambda", "2", "F / Sig.", 7.412050287e-08),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "F to Remove",
                11.31996999,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Wilks' Lambda",
                0.7056532364,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                10.54095053,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Wilks' Lambda",
                0.6916726153,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Min. Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "F to Enter",
                5.703167801,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Wilks' Lambda",
                0.4161878189,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.9838335158,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.9730858844,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.1028691078,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Wilks' Lambda",
                0.5006274505,
            ),
            (
                "Variables Entered/Removed",
                "3",
                "Wilks' Lambda",
                0.4161878189,
            ),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4161878189),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 57.0),
            ("Wilks' Lambda", "3", "F / Statistic", 10.08487883),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 110.0),
            ("Wilks' Lambda", "3", "F / Sig.", 7.274463824e-09),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9877869722,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                10.51147854,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Wilks' Lambda",
                0.5752696127,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                10.24108477,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Wilks' Lambda",
                0.5711774456,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                5.703167801,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Wilks' Lambda",
                0.502500145,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                0.2801358363,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Wilks' Lambda",
                0.4119140454,
            ),
        ],
    );
    // the order of entry
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1])))
        .collect();
    assert_eq!(
        steps,
        [("1", "x3"), ("2", "x2"), ("3", "x1")]
            .map(|(a, b): (&str, &str)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn stepwise_mahal_agrees_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /ANALYSIS=ALL /METHOD=MAHAL /PIN=0.15 /POUT=0.20.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                6.357771673,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. D Squared",
                0.2873993389,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                11.88810924,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. D Squared",
                0.001967403886,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. D Squared",
                0.3481867735,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                0.4292146398,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. D Squared",
                0.000576878739,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Min. D Squared",
                0.3481867735,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.6916726153),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 57.0),
            ("Wilks' Lambda", "1", "F / Statistic", 12.7044649),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 57.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.736318025e-05),
            ("Variables in the Analysis", "1 / x3", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x3",
                "F to Remove",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                5.906859206,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. D Squared",
                1.357406745,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                10.54095053,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. D Squared",
                0.3485910382,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                0.09731623627,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. D Squared",
                0.3580095044,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Min. D Squared",
                1.357406745,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.5711774456),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 57.0),
            ("Wilks' Lambda", "2", "F / Statistic", 9.048660141),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 112.0),
            ("Wilks' Lambda", "2", "F / Sig.", 2.271093589e-06),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "F to Remove",
                12.08041602,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Min. D Squared",
                0.2873993389,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                5.906859206,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Min. D Squared",
                0.3481867735,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "F to Enter",
                10.24108477,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. D Squared",
                1.613164577,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.8979820152,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.8979820152,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.232360746,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. D Squared",
                1.35846768,
            ),
            (
                "Variables Entered/Removed",
                "3",
                "Min. D Squared",
                1.613164577,
            ),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4161878189),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 57.0),
            ("Wilks' Lambda", "3", "F / Statistic", 10.08487883),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 110.0),
            ("Wilks' Lambda", "3", "F / Sig.", 7.274463824e-09),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9877869722,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                10.51147854,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Min. D Squared",
                1.290184923,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                5.703167801,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Min. D Squared",
                0.3485910382,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                10.24108477,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Min. D Squared",
                1.357406745,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                0.2801358363,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. D Squared",
                1.675487692,
            ),
        ],
    );
    // the order of entry
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1])))
        .collect();
    assert_eq!(
        steps,
        [("1", "x3"), ("2", "x1"), ("3", "x2")]
            .map(|(a, b): (&str, &str)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn stepwise_maxminf_agrees_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /ANALYSIS=ALL /METHOD=MAXMINF /PIN=0.15 /POUT=0.20.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                6.357771673,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. F",
                3.010850217,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                11.88810924,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. F",
                0.01947729848,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. F",
                3.447049058,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                0.4292146398,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. F",
                0.005711099516,
            ),
            ("Variables Entered/Removed", "1", "Min. F", 3.447049058),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.6916726153),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 57.0),
            ("Wilks' Lambda", "1", "F / Statistic", 12.7044649),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 57.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.736318025e-05),
            ("Variables in the Analysis", "1 / x3", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x3",
                "F to Remove",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                5.906859206,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. F",
                6.985485004,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                10.54095053,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. F",
                1.695253259,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                0.09731623627,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. F",
                1.741056748,
            ),
            ("Variables Entered/Removed", "2", "Min. F", 6.985485004),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.5711774456),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 57.0),
            ("Wilks' Lambda", "2", "F / Statistic", 9.048660141),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 112.0),
            ("Wilks' Lambda", "2", "F / Sig.", 2.271093589e-06),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "F to Remove",
                12.08041602,
            ),
            ("Variables in the Analysis", "2 / x3", "Min. F", 3.010850217),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                5.906859206,
            ),
            ("Variables in the Analysis", "2 / x1", "Min. F", 3.447049058),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "F to Enter",
                10.24108477,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. F",
                5.136655628,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.8979820152,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.8979820152,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.232360746,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. F",
                4.577404324,
            ),
            ("Variables Entered/Removed", "3", "Min. F", 5.136655628),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4161878189),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 57.0),
            ("Wilks' Lambda", "3", "F / Statistic", 10.08487883),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 110.0),
            ("Wilks' Lambda", "3", "F / Sig.", 7.274463824e-09),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9877869722,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                10.51147854,
            ),
            ("Variables in the Analysis", "3 / x3", "Min. F", 6.274372995),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                5.703167801,
            ),
            ("Variables in the Analysis", "3 / x1", "Min. F", 1.695253259),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                10.24108477,
            ),
            ("Variables in the Analysis", "3 / x2", "Min. F", 6.985485004),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                0.2801358363,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. F",
                3.928577721,
            ),
        ],
    );
    // the order of entry
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1])))
        .collect();
    assert_eq!(
        steps,
        [("1", "x3"), ("2", "x1"), ("3", "x2")]
            .map(|(a, b): (&str, &str)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn stepwise_minresid_agrees_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /ANALYSIS=ALL /METHOD=MINRESID /PIN=0.15 /POUT=0.20.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                6.357771673,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Sum of Unexplained Variation",
                2.60828138,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                11.88810924,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Sum of Unexplained Variation",
                2.384615446,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Sum of Unexplained Variation",
                2.321587933,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                0.4292146398,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Sum of Unexplained Variation",
                2.968610129,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Sum of Unexplained Variation",
                2.321587933,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.6916726153),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 57.0),
            ("Wilks' Lambda", "1", "F / Statistic", 12.7044649),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 57.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.736318025e-05),
            ("Variables in the Analysis", "1 / x3", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x3",
                "F to Remove",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                5.906859206,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Sum of Unexplained Variation",
                2.042634553,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                10.54095053,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Sum of Unexplained Variation",
                1.928128815,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                0.09731623627,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Sum of Unexplained Variation",
                2.31645208,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Sum of Unexplained Variation",
                1.928128815,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.502500145),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 57.0),
            ("Wilks' Lambda", "2", "F / Statistic", 11.49934879),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 112.0),
            ("Wilks' Lambda", "2", "F / Sig.", 7.412050287e-08),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "F to Remove",
                11.31996999,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Sum of Unexplained Variation",
                2.384615446,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                10.54095053,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Sum of Unexplained Variation",
                2.321587933,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Min. Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "F to Enter",
                5.703167801,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Sum of Unexplained Variation",
                1.683440615,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.9838335158,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.9730858844,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.1028691078,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Sum of Unexplained Variation",
                1.923826604,
            ),
            (
                "Variables Entered/Removed",
                "3",
                "Sum of Unexplained Variation",
                1.683440615,
            ),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4161878189),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 57.0),
            ("Wilks' Lambda", "3", "F / Statistic", 10.08487883),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 110.0),
            ("Wilks' Lambda", "3", "F / Sig.", 7.274463824e-09),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9877869722,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                10.51147854,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Sum of Unexplained Variation",
                2.061775085,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                10.24108477,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Sum of Unexplained Variation",
                2.042634553,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                5.703167801,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Sum of Unexplained Variation",
                1.928128815,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                0.2801358363,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Sum of Unexplained Variation",
                1.67160959,
            ),
        ],
    );
    // the order of entry
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1])))
        .collect();
    assert_eq!(
        steps,
        [("1", "x3"), ("2", "x2"), ("3", "x1")]
            .map(|(a, b): (&str, &str)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn stepwise_rao_agrees_with_r() {
    let t = run(
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /ANALYSIS=ALL /METHOD=RAO /PIN=0.15 /POUT=0.20.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                6.357771673,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Rao's V",
                12.71554335,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                11.88810924,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Rao's V",
                23.77621849,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Rao's V",
                25.4089298,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                0.4292146398,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Rao's V",
                0.8584292797,
            ),
            ("Variables Entered/Removed", "1", "Rao's V", 25.4089298),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.6916726153),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 57.0),
            ("Wilks' Lambda", "1", "F / Statistic", 12.7044649),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 57.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.736318025e-05),
            ("Variables in the Analysis", "1 / x3", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x3",
                "F to Remove",
                12.7044649,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.999101449,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                5.906859206,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Rao's V",
                37.78863289,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9881101842,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                10.54095053,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Rao's V",
                54.76107502,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.9841439621,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                0.09731623627,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Rao's V",
                25.61906613,
            ),
            ("Variables Entered/Removed", "2", "Rao's V", 54.76107502),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.502500145),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 57.0),
            ("Wilks' Lambda", "2", "F / Statistic", 11.49934879),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 112.0),
            ("Wilks' Lambda", "2", "F / Sig.", 7.412050287e-08),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "F to Remove",
                11.31996999,
            ),
            (
                "Variables in the Analysis",
                "2 / x3",
                "Rao's V",
                23.77621849,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9881101842,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                10.54095053,
            ),
            ("Variables in the Analysis", "2 / x2", "Rao's V", 25.4089298),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Min. Tolerance",
                0.9764828985,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "F to Enter",
                5.703167801,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x1",
                "Rao's V",
                67.40507672,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.9838335158,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.9730858844,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.1028691078,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Rao's V",
                55.0445159,
            ),
            ("Variables Entered/Removed", "3", "Rao's V", 67.40507672),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4161878189),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 57.0),
            ("Wilks' Lambda", "3", "F / Statistic", 10.08487883),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 110.0),
            ("Wilks' Lambda", "3", "F / Sig.", 7.274463824e-09),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9877869722,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                10.51147854,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Rao's V",
                36.73031167,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9764828985,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                10.24108477,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Rao's V",
                37.78863289,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.987344827,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                5.703167801,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Rao's V",
                54.76107502,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.8955064909,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                0.2801358363,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Rao's V",
                68.34018305,
            ),
        ],
    );
    // the order of entry
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1])))
        .collect();
    assert_eq!(
        steps,
        [("1", "x3"), ("2", "x2"), ("3", "x1")]
            .map(|(a, b): (&str, &str)| (a.to_string(), b.to_string()))
    );
}
