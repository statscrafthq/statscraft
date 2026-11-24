//! Data far from zero: 40 values near 1e9 with a standard deviation of 3 (a ratio of 3e8) and
//! 40 values near 1e-4 with a standard deviation of 1e-6. A naive sum of squares loses every
//! digit at this ratio; the statistics must stay close to the exact ones. The expected values
//! are computed in exact rational arithmetic from the decimal text of the data (R's `var`
//! agrees with them to 1e-9). The tolerances are the accuracy the procedures reach today:
//! 1e-7 for the standard deviation of DESCRIPTIVES, 1e-6 for the F of ONEWAY and the t of
//! T-TEST (single-pass sums of squares, about eps times the ratio), 1e-8 for the products of
//! CORRELATIONS and REGRESSION (two passes).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x y z.
BEGIN DATA
1 999999996.303 1999999999.637 0.00010016962
1 1000000004.77 1999999997.466 9.94501e-05
1 1000000004.738 2000000002.122 9.9794935e-05
1 1000000003.827 1999999998.652 9.992214e-05
1 999999995.016 1999999999.716 9.8884713e-05
1 999999996.711 1999999995.015 9.9178242e-05
1 1000000002.311 2000000004.681 0.000100276737
1 999999999.446 2000000001.583 0.000100605696
1 1000000000.579 2000000000.991 9.820028e-05
1 1000000002.927 2000000001.045 0.000101886871
1 999999993.991 1999999998.288 9.8725033e-05
1 1000000002.963 2000000001.593 0.000101612076
1 1000000002.128 1999999999.916 0.000100107219
1 1000000001.497 1999999999.219 9.963327e-05
1 1000000001.423 2000000001.641 9.974729e-05
1 1000000004.59 2000000005.242 9.9996625e-05
1 999999999.615 2000000003.261 9.9929161e-05
1 999999997.057 1999999995.012 0.000100031778
1 1000000000.637 2000000005.225 9.9649228e-05
1 1000000002.102 2000000000.983 0.000101135293
2 1000000008.39 2000000004.088 0.000101704619
2 1000000000.885 1999999997.494 0.000100705709
2 1000000000.17 2000000001.194 9.9826871e-05
2 1000000001.449 1999999996.726 0.000100659665
2 1000000002.281 2000000000.887 0.000101162289
2 1000000001.498 1999999999.227 9.8530456e-05
2 1000000003.073 2000000001.532 9.8073501e-05
2 1000000004.246 2000000002.216 9.901922e-05
2 1000000004.019 2000000003.921 0.000101668325
2 1000000001.759 1999999998.938 9.8540837e-05
2 999999993.485 1999999996.922 0.000100203413
2 1000000000.192 2000000000.55 9.9512915e-05
2 999999999.765 1999999999.873 9.9045387e-05
2 1000000002.225 1999999998.799 0.000101023577
2 1000000000.522 2000000001.537 9.9271553e-05
2 1000000004.518 1999999995.971 0.000100145136
2 1000000002.203 2000000002.209 9.9258472e-05
2 999999999.726 1999999996.73 9.9894872e-05
2 1000000003.687 2000000003.869 0.000102060601
2 1000000002.589 2000000002.926 9.9933069e-05
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

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs(),
        "{what}: {got} vs {want}"
    );
}

fn cell(tables: &[Table], title: &str, row: usize, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let at = table
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column}"));
    match table.rows[row][at] {
        Cell::Number(n) => n,
        ref other => panic!("{column}: {other:?}"),
    }
}

#[test]
fn descriptives_stay_accurate_far_from_zero() {
    let t = run("DESCRIPTIVES x z /STATISTICS=MEAN STDDEV VARIANCE.");
    let title = "Descriptive Statistics";
    near(cell(&t, title, 0, "Mean"), 1000000001.232825, 1e-14, "mean");
    near(cell(&t, title, 0, "Std Dev"), 3.089184963355, 1e-7, "sd");
    near(
        cell(&t, title, 0, "Variance"),
        9.543063737821,
        1e-7,
        "variance",
    );
    near(
        cell(&t, title, 1, "Mean"),
        9.997941985e-05,
        1e-9,
        "small mean",
    );
    near(
        cell(&t, title, 1, "Std Dev"),
        1.012008740207508e-06,
        1e-7,
        "small sd",
    );
}

#[test]
fn correlations_and_regression_stay_accurate_far_from_zero() {
    let t = run("CORRELATIONS /VARIABLES=x y /STATISTICS=XPROD.\n\
                 REGRESSION /STATISTICS=COEFF R /DEPENDENT=y /METHOD=ENTER x.");
    let title = "Correlations";
    let table = t.iter().find(|t| t.title == title).expect("the table");
    let number = |row: usize, column: usize| match table.rows[row][column] {
        Cell::Number(n) => n,
        ref other => panic!("{other:?}"),
    };
    // rows of x: correlation, significance, cross-products, covariance, N
    near(number(0, 3), 0.452412699350, 1e-8, "r");
    near(number(2, 3), 147.708407975, 1e-8, "cross-products");
    near(number(3, 3), 3.787395076282, 1e-8, "covariance");
    near(
        cell(&t, "Coefficients", 1, "B"),
        0.396874125578,
        1e-8,
        "slope",
    );
    near(cell(&t, "Model Summary", 0, "R"), 0.452412699350, 1e-8, "R");
}

#[test]
fn t_test_and_oneway_stay_accurate_far_from_zero() {
    let t = run("T-TEST GROUPS=g(1 2) /VARIABLES=x.\nONEWAY x BY g.");
    let title = "Independent Samples Test";
    near(cell(&t, title, 0, "t").abs(), 1.239436866734, 1e-6, "t");
    near(
        // two means near 1e9 differ by no more than a unit in their last place, 1.2e-7
        cell(&t, title, 0, "Mean Difference").abs(),
        1.2025500000,
        1e-6,
        "difference",
    );
    let anova = "ANOVA";
    near(
        cell(&t, anova, 0, "Sum of Squares"),
        14.461265025,
        1e-6,
        "between",
    );
    near(
        cell(&t, anova, 1, "Sum of Squares"),
        357.718220750,
        1e-8,
        "within",
    );
    near(cell(&t, anova, 0, "F"), 1.536203746619, 1e-6, "F");
}
