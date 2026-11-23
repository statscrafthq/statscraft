//! MEANS statistics against R on 30 invented values in three groups: every cell statistic
//! (mean, count, standard deviation, variance, median, standard error, sum, minimum, maximum,
//! range, the bias-adjusted skewness and kurtosis with their standard errors, first, last,
//! harmonic and geometric means) per group and for all cases, the percentages of the total,
//! the ANOVA table with the test of linearity (the linearity F against the within-groups mean
//! square, as the manual's table has it) and eta, and the grouped median of the manual's
//! formula (linear interpolation between the cumulative midpoints) with ties.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y g.
BEGIN DATA
20.2 1 
19.3 1 
14.2 1 
23.9 1 
26 1 
18.2 1 
21.6 1 
23.7 1 
14.9 1 
21.3 2 
19.7 2 
23.3 2 
25.9 2 
19.8 2 
41.7 2 
20.4 2 
24.8 2 
29.3 2 
25.2 2 
20.4 2 
21.1 3 
29.8 3 
34.7 3 
35.6 3 
30.3 3 
31.5 3 
45 3 
31.1 3 
29.9 3 
30.7 3 
END DATA.
";

fn run(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The number in `column` of the first row whose cells include `label`.
fn cell(t: &Table, label: &str, column: &str) -> f64 {
    let at = t
        .column_titles()
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()));
    let row = t
        .rows
        .iter()
        .find(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
        .unwrap_or_else(|| panic!("no row {label}"));
    match row[at] {
        Cell::Number(n) => n,
        ref other => panic!("{label} {column}: {other:?}"),
    }
}

#[test]
fn the_cell_statistics_agree_with_r() {
    let t = run(
        DATA,
        "MEANS TABLES=y BY g /CELLS=MEAN COUNT STDDEV VARIANCE MEDIAN SEMEAN SUM MIN MAX RANGE \
         SKEW SESKEW KURT SEKURT FIRST LAST HARMONIC GEOMETRIC NPCT SPCT.",
    );
    let report = table(&t, "Report");
    let columns = [
        "Mean",
        "N",
        "Std. Deviation",
        "Variance",
        "Median",
        "Std. Error of Mean",
        "Sum",
        "Minimum",
        "Maximum",
        "Range",
        "Skewness",
        "Std. Error of Skewness",
        "Kurtosis",
        "Std. Error of Kurtosis",
        "First",
        "Last",
        "Harmonic Mean",
        "Geometric Mean",
    ];
    let expected: [(&str, [f64; 18]); 4] = [
        (
            "1",
            [
                20.222222222,
                9.0,
                4.0378762295,
                16.304444444,
                20.2,
                1.3459587432,
                182.0,
                14.2,
                26.0,
                11.8,
                -0.22194450883,
                0.7171371656,
                -0.96180800692,
                1.3997084244,
                20.2,
                14.9,
                19.455545724,
                19.845853476,
            ],
        ),
        (
            "2",
            [
                24.709090909,
                11.0,
                6.4138061314,
                41.136909091,
                23.3,
                1.9338353105,
                271.8,
                19.7,
                41.7,
                22.0,
                2.1110887001,
                0.66068747264,
                5.1316743044,
                1.2794157893,
                21.3,
                20.4,
                23.610309707,
                24.10048162,
            ],
        ),
        (
            "3",
            [
                31.97,
                10.0,
                5.9901864189,
                35.882333333,
                30.9,
                1.8942632693,
                319.7,
                21.1,
                45.0,
                23.9,
                0.62646264603,
                0.68704291862,
                2.9611474559,
                1.33424877,
                21.1,
                30.7,
                30.959489268,
                31.468887084,
            ],
        ),
        (
            "Total",
            [
                25.783333333,
                30.0,
                7.283712739,
                53.052471264,
                24.35,
                1.3298179232,
                773.5,
                14.2,
                45.0,
                30.8,
                0.83508883157,
                0.42689239595,
                0.63242666613,
                0.83274561836,
                20.2,
                30.7,
                23.971350426,
                24.850635163,
            ],
        ),
    ];
    for (label, want) in expected {
        for (column, want) in columns.iter().zip(want) {
            near(
                cell(report, label, column),
                want,
                &format!("{label} {column}"),
            );
        }
    }
    // the shares of the cases and of the sum
    for (label, n, sum) in [
        ("1", 30.0, 182.0 / 773.5 * 100.0),
        ("2", 36.6666666667, 271.8 / 773.5 * 100.0),
        ("3", 33.3333333333, 319.7 / 773.5 * 100.0),
    ] {
        near(cell(report, label, "% of Total N"), n, label);
        near(cell(report, label, "% of Total Sum"), sum, label);
    }
}

#[test]
fn anova_linearity_and_eta_agree_with_r() {
    let t = run(
        DATA,
        "MEANS TABLES=y BY g /CELLS=MEAN /STATISTICS=ANOVA LINEARITY.",
    );
    let anova = table(&t, "ANOVA Table");
    near(
        cell(anova, "(Combined)", "Sum of Squares"),
        673.776020202,
        "between SS",
    );
    near(cell(anova, "(Combined)", "F"), 10.5186725252, "F");
    near(cell(anova, "(Combined)", "Sig."), 0.000418884568923, "p");
    near(
        cell(anova, "Linearity", "Sum of Squares"),
        660.387009373,
        "linearity SS",
    );
    // 660.387009373 / (864.745646465 / 27): the within-groups mean square
    near(
        cell(anova, "Linearity", "F"),
        660.387009373 / (864.745646465 / 27.0),
        "linearity F",
    );
    near(
        cell(anova, "Deviation from Linearity", "Sum of Squares"),
        13.3890108289,
        "deviation",
    );
    near(
        cell(anova, "Deviation from Linearity", "F"),
        0.418045807871,
        "deviation F",
    );
    near(
        cell(anova, "Within Groups", "Sum of Squares"),
        864.745646465,
        "within SS",
    );
    let measures = table(&t, "Measures of Association");
    near(cell(measures, "y * g", "Eta"), 0.661768310727, "eta");
    near(
        cell(measures, "y * g", "Eta Squared"),
        0.437937297082,
        "eta squared",
    );
    near(cell(measures, "y * g", "R"), 0.655160117961, "R");
    near(
        cell(measures, "y * g", "R Squared"),
        0.429234780167,
        "R squared",
    );
}

#[test]
fn the_grouped_median_interpolates_between_the_cumulative_midpoints() {
    // 25 values with two ties: the median is 48.9, the grouped median 48.6 (R: the manual's
    // cc_i = sum of the frequencies before i plus half of its own, linear between the values)
    let data = "DATA LIST FREE /x.
BEGIN DATA
43.9 43.5 48.9 47.8 53.5 40.5 59.5 49.9 48 47.1 60.2 46.2 50.6 47.9 64.8 43.3 49.4 29.1 57.1 44.3
95.5 12.3 50 48.9 49.9
END DATA.
";
    let t = run(data, "MEANS TABLES=x /CELLS=MEAN MEDIAN GMEDIAN.");
    let report = table(&t, "Report");
    let at = report
        .column_titles()
        .iter()
        .position(|c| c == "Grouped Median")
        .unwrap();
    let Cell::Number(g) = report.rows[0][at] else {
        panic!("no grouped median")
    };
    near(g, 48.6, "grouped median");
    // without ties it is the median of the group (the groups of the first test)
    let t = run(DATA, "MEANS TABLES=y BY g /CELLS=MEDIAN GMEDIAN.");
    let report = table(&t, "Report");
    for label in ["1", "2", "3", "Total"] {
        near(
            cell(report, label, "Grouped Median"),
            cell(report, label, "Median"),
            label,
        );
    }
}
