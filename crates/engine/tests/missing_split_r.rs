//! Missing values and SPLIT FILE against R on 36 invented cases (three variables with nine
//! missing values in all, two split groups): pairwise correlations (`cor` with
//! `use = "pairwise.complete.obs"`, `cor.test` on each pair's own cases, so the N differs from
//! pair to pair), listwise correlations on the 28 complete cases, REGRESSION with listwise
//! deletion and with mean substitution (`lm` on the complete cases and on the filled data), the
//! T-TEST that drops a case only for its own variables, and the split-file results of
//! CORRELATIONS, REGRESSION and DESCRIPTIVES against R run on each group.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /s x y z.
BEGIN DATA
1 10.8 10 4.8
1 8 8.5 2.8
1 . 4.3 2.6
1 11.2 11.4 4
1 13.1 . 4.2
1 8 5.6 2.3
1 18.4 10.6 6.4
1 16.5 15.4 .
1 14.2 10 7.1
1 . . 2.9
1 11.1 7.5 2.8
1 10.3 9.1 7.6
1 8.9 7.2 3.9
1 11.6 11.3 3.7
1 6.9 8.6 4.6
1 14.5 13.6 5.2
1 10.4 9.4 4.7
1 9.3 6.5 3
2 11.6 11.3 9.7
2 8.6 8.4 .
2 12.9 10.2 6.5
2 11.2 7.4 5.9
2 7.4 4.3 7.3
2 12.9 13.2 5.7
2 . 7.2 2.7
2 9.4 9.9 5.1
2 16.6 13.2 6.9
2 9.2 9.5 4.9
2 1.7 3.6 6.3
2 8.2 . 3.5
2 10.7 8.3 8.4
2 6.1 10.1 5.2
2 10.8 . 6.1
2 11.3 8.9 7.9
2 11.4 7.9 6.1
2 7.9 3.7 2.3
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

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(s) => s,
        _ => "",
    }
}

/// The number in `column` of the `nth` table called `title`, in the row whose leading cells
/// are `key`, the empty ones taking the label above them (a number counts as its text).
fn at(tables: &[Table], title: &str, nth: usize, key: &[&str], column: &str) -> f64 {
    let table = tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title}"));
    let titles = table.column_titles();
    let at = titles
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {titles:?}"));
    let mut carried = vec![String::new(); key.len()];
    for row in &table.rows {
        for (i, label) in carried.iter_mut().enumerate() {
            let cell = text(&row[i]);
            if !cell.is_empty() {
                *label = cell.to_string();
            } else if let Cell::Number(n) = row[i] {
                *label = format!("{n}");
            }
        }
        if carried == key {
            return match row[at] {
                Cell::Number(n) => n,
                ref other => panic!("{key:?}: {other:?}"),
            };
        }
    }
    panic!("no row {key:?} in {title}");
}

#[test]
fn pairwise_and_listwise_correlations_agree_with_r() {
    let t = run("CORRELATIONS /VARIABLES=x y z /MISSING=PAIRWISE.\n\
                 CORRELATIONS /VARIABLES=x y z /MISSING=LISTWISE.");
    let pairwise = |a: &str, row: &str, b: &str| at(&t, "Correlations", 0, &[a, row], b);
    // each pair uses its own cases: 30, 31 and 30
    for (a, b, r, p, n) in [
        ("x", "y", 0.735304924649, 3.68249924931e-06, 30.0),
        ("x", "z", 0.277113083737, 0.131239948123, 31.0),
        ("y", "z", 0.339897011679, 0.0661015361797, 30.0),
    ] {
        near(pairwise(a, "Pearson Correlation", b), r, "r");
        near(pairwise(a, "Sig. (2-tailed)", b), p, "p");
        near(pairwise(a, "N", b), n, "N");
        near(pairwise(b, "Pearson Correlation", a), r, "r");
    }
    // listwise: the 28 complete cases
    let listwise = |a: &str, row: &str, b: &str| at(&t, "Correlations", 1, &[a, row], b);
    near(
        listwise("x", "Pearson Correlation", "y"),
        0.697330043900,
        "listwise r",
    );
    near(
        listwise("x", "Sig. (2-tailed)", "y"),
        3.72606346203e-05,
        "listwise p",
    );
    near(
        listwise("x", "Pearson Correlation", "z"),
        0.281521157300,
        "listwise r",
    );
    near(
        listwise("y", "Pearson Correlation", "z"),
        0.263626838000,
        "listwise r",
    );
}

#[test]
fn regression_with_listwise_deletion_and_mean_substitution_agrees_with_r() {
    let t = run(
        "REGRESSION /MISSING=LISTWISE /STATISTICS=COEFF R ANOVA /DEPENDENT=z \
                 /METHOD=ENTER x y.\n\
                 REGRESSION /MISSING=MEANSUBSTITUTION /STATISTICS=COEFF R ANOVA /DEPENDENT=z \
                 /METHOD=ENTER x y.",
    );
    // (nth table, R square, adjusted, standard error, total df, B and standard error of x and y)
    for (nth, r2, adjusted, se, total, b) in [
        (
            0,
            0.0880742113872,
            0.0151201482982,
            1.89734161806,
            27.0,
            [(0.1105931978, 0.1549788951), (0.0948114917, 0.1928125132)],
        ),
        (
            1,
            0.0884048914154,
            0.033156703,
            1.83855576053,
            35.0,
            [(0.0399312169, 0.1361972346), (0.1714651726, 0.1579029058)],
        ),
    ] {
        near(
            at(&t, "Model Summary", nth, &["1"], "R Square"),
            r2,
            "R square",
        );
        near(
            at(&t, "Model Summary", nth, &["1"], "Adjusted R Square"),
            adjusted,
            "adjusted",
        );
        near(
            at(
                &t,
                "Model Summary",
                nth,
                &["1"],
                "Std. Error of the Estimate",
            ),
            se,
            "se",
        );
        near(
            at(&t, "ANOVA", nth, &["1", "Total"], "df"),
            total,
            "total df",
        );
        for (label, (b, se)) in ["x", "y"].iter().zip(b) {
            near(at(&t, "Coefficients", nth, &["1", label], "B"), b, label);
            near(
                at(&t, "Coefficients", nth, &["1", label], "Std. Error"),
                se,
                label,
            );
        }
    }
    near(
        at(&t, "Coefficients", 0, &["1", "(Constant)"], "B"),
        3.3863548949,
        "constant",
    );
    near(
        at(&t, "Coefficients", 1, &["1", "(Constant)"], "B"),
        3.1333267148,
        "constant",
    );
    near(
        at(&t, "ANOVA", 0, &["1", "Regression"], "F"),
        1.20725573954,
        "F",
    );
}

#[test]
fn t_test_and_split_file_results_agree_with_r() {
    let t = run(
        "T-TEST GROUPS=s(1 2) /VARIABLES=x.\nSORT CASES BY s.\nSPLIT FILE BY s.\n\
                 CORRELATIONS /VARIABLES=x y.\nREGRESSION /STATISTICS=R COEFF /DEPENDENT=z \
                 /METHOD=ENTER x.\nDESCRIPTIVES x.",
    );
    // the T-TEST drops a case for its own variable only: 33 cases with a group and an x
    let title = "Independent Samples Test";
    let key = ["x", "Equal variances assumed"];
    near(at(&t, title, 0, &key, "t"), 1.40072089498, "t");
    near(at(&t, title, 0, &key, "df"), 31.0, "df");
    near(
        at(&t, title, 0, &key, "Sig. (2-tailed)"),
        0.171228442573,
        "p",
    );
    // per split group: r, p and N of x and y, the regression of z on x, the mean and sd of x
    for (group, r, p, n, r2, b, mean, sd, count) in [
        (
            "1",
            0.71788299137,
            0.00257980724246,
            15.0,
            0.302221164322,
            0.29024749621,
            11.45,
            3.18412730064,
            16.0,
        ),
        (
            "2",
            0.732628661291,
            0.00189297067717,
            15.0,
            0.0940313179736,
            0.164307712834,
            9.87647058824,
            3.26315212099,
            17.0,
        ),
    ] {
        near(
            at(
                &t,
                "Correlations",
                0,
                &[group, "x", "Pearson Correlation"],
                "y",
            ),
            r,
            "r",
        );
        near(
            at(&t, "Correlations", 0, &[group, "x", "Sig. (2-tailed)"], "y"),
            p,
            "p",
        );
        near(at(&t, "Correlations", 0, &[group, "x", "N"], "y"), n, "N");
        near(
            at(&t, "Model Summary", 0, &[group, "1"], "R Square"),
            r2,
            "R square",
        );
        near(at(&t, "Coefficients", 0, &[group, "1", "x"], "B"), b, "B");
        near(
            at(&t, "Descriptive Statistics", 0, &[group, "x"], "Mean"),
            mean,
            "mean",
        );
        near(
            at(&t, "Descriptive Statistics", 0, &[group, "x"], "Std Dev"),
            sd,
            "sd",
        );
        near(
            at(&t, "Descriptive Statistics", 0, &[group, "x"], "N"),
            count,
            "N",
        );
    }
}
