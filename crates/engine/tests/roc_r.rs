//! ROC and ROC ANALYSIS against R on 35 invented cases (15 positive, 20 negative, two
//! correlated scores with ties): the area, the nonparametric standard error with the manual's
//! tie terms (Q1, Q2 with the one third for ties), the interval, and the test of area 0.5 with
//! the null standard deviation sqrt((n+ + n- + 1) / (12 n+ n-)); TESTPOS(SMALL), which gives
//! the area 1 - W with the same error; and ROC ANALYSIS with PAIR=TRUE, whose errors, tests and
//! intervals follow DeLong's method (placement values and their covariances, written out in R).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y s1 s2.
BEGIN DATA
1 3.3 1.7
1 -0.2 1.2
1 0.3 -0.6
1 0.6 0.5
1 0 0.6
1 0.1 1.6
1 1.7 0.6
1 0.9 0.6
1 1.2 -0.3
1 3.2 1.8
1 1.4 0.7
1 3.7 3.9
1 3.3 2.9
1 1.3 1.2
1 2.9 3
0 0.5 0
0 -0.9 -0.5
0 -0.3 0.2
0 0 1.7
0 1 1.3
0 0.8 1
0 0.7 -1.1
0 1.3 1.1
0 -1.4 -0.7
0 1.3 -0.1
0 0.2 0.2
0 0.8 0.6
0 0.6 0.9
0 -1 0.1
0 -0.3 0.1
0 -0.9 0.6
0 0.7 1.2
0 0.1 1.2
0 -0.1 1.2
0 -0.4 0.5
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

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The numbers of the row of `table` whose first cell is `label`, by column title.
fn row(tables: &[Table], title: &str, label: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let at = table
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column}"));
    let row = table
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label}"));
    match row[at] {
        Cell::Number(n) => n,
        ref other => panic!("{label} {column}: {other:?}"),
    }
}

#[test]
fn the_nonparametric_area_error_interval_and_test_agree_with_r() {
    let t = run("ROC s1 s2 BY y (1) /PRINT=SE.");
    let title = "Area Under the Curve";
    for (label, area, se, p, lower, upper) in [
        (
            "s1",
            0.808333333333,
            0.0746384288536,
            0.00204695732859,
            0.662044700918,
            0.954621965749,
        ),
        (
            "s2",
            0.703333333333,
            0.091976781826,
            0.0420188576797,
            0.523062153541,
            0.883604513126,
        ),
    ] {
        near(row(&t, title, label, "Area"), area, label);
        near(row(&t, title, label, "Std. Error"), se, label);
        near(row(&t, title, label, "Asymptotic Sig."), p, label);
        near(row(&t, title, label, "Lower Bound"), lower, label);
        near(row(&t, title, label, "Upper Bound"), upper, label);
    }
    // a low score pointing to the positive state: the area is 1 - W, the error the same
    let t = run("ROC s1 BY y (1) /CRITERIA=TESTPOS(SMALL) /PRINT=SE.");
    near(row(&t, title, "s1", "Area"), 0.191666666667, "small area");
    near(
        row(&t, title, "s1", "Std. Error"),
        0.0746384288536,
        "small error",
    );
    near(
        row(&t, title, "s1", "Asymptotic Sig."),
        0.00204695732859,
        "small p",
    );
}

#[test]
fn the_paired_areas_follow_delong_in_r() {
    let t = run("ROC ANALYSIS s1 s2 BY y (1) /DESIGN PAIR=TRUE /PRINT SE=TRUE.");
    let title = "Area Under the ROC Curve";
    for (label, area, se, p, lower, upper) in [
        (
            "s1",
            0.808333333333,
            0.0754234772399,
            4.35054496509e-05,
            0.660506034354,
            0.956160632312,
        ),
        (
            "s2",
            0.703333333333,
            0.0932399729984,
            0.029201704494,
            0.520586344337,
            0.88608032233,
        ),
    ] {
        near(row(&t, title, label, "Area"), area, label);
        near(row(&t, title, label, "Std. Error"), se, label);
        near(row(&t, title, label, "Asymptotic Sig."), p, label);
        near(row(&t, title, label, "Lower Bound"), lower, label);
        near(row(&t, title, label, "Upper Bound"), upper, label);
    }
    let title = "Paired-Sample Area Difference Under the ROC Curves";
    let pair = "s1 - s2";
    near(row(&t, title, pair, "AUC Difference"), 0.105, "difference");
    near(
        row(&t, title, pair, "Std. Error Difference"),
        0.101068229268,
        "error",
    );
    near(row(&t, title, pair, "Asymptotic z"), 1.03890214324, "z");
    near(
        row(&t, title, pair, "Asymptotic Sig. (2-tail)"),
        0.29885024953,
        "p",
    );
    near(
        row(&t, title, pair, "Lower Bound"),
        -0.093090089347,
        "lower",
    );
    near(row(&t, title, pair, "Upper Bound"), 0.303090089347, "upper");
}
