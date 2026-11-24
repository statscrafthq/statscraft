//! WEIGHTED KAPPA and FLEISS MULTIRATER KAPPA against R on 40 invented subjects rated 1 to 4 by
//! three raters: the linear and quadratic weighted kappas (also against `psych::cohen.kappa`),
//! their asymptotic standard errors, the z tests and the intervals written out in R from the
//! manual's formulas, and Fleiss' overall kappa and the kappa of each category with their
//! errors under chance agreement.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /r1 r2 r3.
BEGIN DATA
2 2 2
2 2 2
3 4 2
3 2 2
2 1 3
1 1 1
3 2 2
2 3 1
1 1 1
1 2 3
2 2 1
3 2 4
1 1 1
1 1 2
4 3 4
3 3 4
3 2 3
2 3 2
2 3 1
3 3 2
2 1 3
4 4 4
3 3 4
3 3 4
3 2 2
3 3 4
3 4 4
2 3 2
2 3 1
4 3 3
4 4 4
3 3 2
3 3 3
2 1 1
1 2 2
4 4 4
2 1 2
2 2 2
2 3 3
2 2 3
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

/// The number in `column` of the row of the table `title` whose first cell is `label`.
fn at(tables: &[Table], title: &str, label: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let column = table
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column}"));
    let row = table
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label}"));
    match row[column] {
        Cell::Number(n) => n,
        ref other => panic!("{label}: {other:?}"),
    }
}

#[test]
fn weighted_kappa_agrees_with_r_for_both_weightings() {
    // N, kappa, error, z, p, lower, upper
    let linear = [
        (
            "r1 - r2",
            [
                40.0,
                0.479553903346,
                0.0957628951997,
                4.45144546222,
                8.52941966505e-06,
                0.291862077699,
                0.667245728992,
            ],
        ),
        (
            "r1 - r3",
            [
                40.0,
                0.409090909091,
                0.0915757860007,
                3.86761538334,
                1.09904829621e-04,
                0.229605666674,
                0.588576151508,
            ],
        ),
        (
            "r2 - r3",
            [
                40.0,
                0.359823399558,
                0.11181220846,
                3.37630822799,
                7.34655666192e-04,
                0.140675497946,
                0.578971301171,
            ],
        ),
    ];
    let quadratic = [
        (
            "r1 - r2",
            [
                40.0,
                0.689807976366,
                0.0696450322620,
                4.37151243220,
                1.23388823014e-05,
                0.553306221431,
                0.826309731302,
            ],
        ),
        (
            "r1 - r3",
            [
                40.0,
                0.641025641026,
                0.0760389635812,
                4.12758235259,
                3.66597161198e-05,
                0.491992010985,
                0.790059271067,
            ],
        ),
        (
            "r2 - r3",
            [
                40.0,
                0.475609756098,
                0.123163178255,
                3.04015600538,
                2.36455633776e-03,
                0.234214362495,
                0.7170051497,
            ],
        ),
    ];
    let columns = [
        "N",
        "Weighted Kappa",
        "Asymptotic Standard Error",
        "z",
        "Sig.",
        "Lower Bound",
        "Upper Bound",
    ];
    for (syntax, expected) in [
        ("WEIGHTED KAPPA r1 r2 r3.", linear),
        (
            "WEIGHTED KAPPA r1 r2 r3 /CRITERIA WEIGHTING=QUADRATIC.",
            quadratic,
        ),
    ] {
        let t = run(syntax);
        for (pair, values) in expected {
            for (column, want) in columns.iter().zip(values) {
                near(
                    at(&t, "Weighted Kappa", pair, column),
                    want,
                    &format!("{pair} {column}"),
                );
            }
        }
    }
}

#[test]
fn fleiss_kappa_agrees_with_r_overall_and_by_category() {
    let t = run("FLEISS MULTIRATER KAPPA r1 r2 r3 /PRINT CATEGORY.");
    let overall = "Overall Agreement";
    for (column, want) in [
        ("Kappa", 0.220332505255),
        ("Asymptotic Standard Error", 0.0544373901311),
        ("z", 4.04744798978),
        ("Sig.", 5.17790824058e-05),
        ("Lower Bound", 0.113637181186),
        ("Upper Bound", 0.327027829324),
    ] {
        near(at(&t, overall, overall, column), want, column);
    }
    let title = "Agreement on Individual Categories";
    let categories = [
        ("1", 0.5, 0.387755102041, 4.24764432351, 2.16029998495e-05),
        (
            "2",
            0.439024390244,
            0.147885149738,
            1.62000064863,
            0.105232137576,
        ),
        (
            "3",
            0.324324324324,
            0.0231195050472,
            0.253261488654,
            0.800066153749,
        ),
        ("4", 0.55, 0.46, 5.03904752905, 4.67854300532e-07),
    ];
    for (label, probability, kappa, z, p) in categories {
        near(
            at(&t, title, label, "Conditional Probability"),
            probability,
            label,
        );
        near(at(&t, title, label, "Kappa"), kappa, label);
        near(
            at(&t, title, label, "Asymptotic Standard Error"),
            0.0912870929175,
            label,
        );
        near(at(&t, title, label, "z"), z, label);
        near(at(&t, title, label, "Sig."), p, label);
    }
}
