//! CORRELATIONS, NONPAR CORR and PARTIAL CORR against R on 30 invented cases of four variables
//! with ties (`cor.test` for Pearson, Spearman with its t approximation and Kendall's tau-b with
//! the tie-corrected normal approximation; the cross-products and covariances from the
//! definitions; the partial correlations from the residuals of `lm` with the t test on
//! N - 2 - k degrees of freedom).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c e.
BEGIN DATA
51 40 13 21
48 55 8 21
66 65 10 21
60 60 8 14
65 60 6 19
46 47 6 13
54 40 11 12
59 46 10 21
37 34 12 16
43 34 12 11
39 43 12 13
47 51 9 19
52 37 10 17
40 48 6 15
40 38 9 18
41 44 13 8
50 48 16 13
59 35 13 7
50 54 9 16
41 44 12 19
34 51 9 20
48 60 14 9
56 47 10 15
58 67 9 23
49 49 12 17
51 43 10 22
73 63 12 18
50 57 13 18
48 38 9 18
50 44 9 12
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

/// The number in column `column` (a variable) of the row whose leading labels are `key`: the
/// leading cells of a row repeat the label above when they are empty.
fn at(table: &Table, key: &[&str], column: &str) -> f64 {
    let titles = table.column_titles();
    let lead = titles
        .iter()
        .position(|c| ["a", "b", "c", "e"].contains(&c.as_str()))
        .expect("a variable column");
    let at = titles.iter().position(|c| c == column).expect("the column");
    let mut carried = vec![String::new(); lead];
    for row in &table.rows {
        for (i, label) in carried.iter_mut().enumerate() {
            if !text(&row[i]).is_empty() {
                *label = text(&row[i]).to_string();
            }
        }
        if carried == key {
            return match row[at] {
                Cell::Number(n) => n,
                ref other => panic!("{key:?}: {other:?}"),
            };
        }
    }
    panic!("no row {key:?}");
}

/// Per pair: Pearson r and p, covariance, cross-products, Spearman rho and p, Kendall tau-b and p.
fn expected() -> Vec<(&'static str, &'static str, [f64; 8])> {
    vec![
        (
            "a",
            "b",
            [
                0.519379584343,
                0.0032690688226,
                44.6781609195,
                1295.66666667,
                0.369655039155,
                0.0443781113502,
                0.255924170616,
                0.0528452280762,
            ],
        ),
        (
            "a",
            "c",
            [
                -0.0563012846509,
                0.767607436997,
                -1.24137931034,
                -36.0,
                -0.00553963964678,
                0.976822529555,
                0.00250048451582,
                0.98549236303,
            ],
        ),
        (
            "a",
            "e",
            [
                0.179751833957,
                0.341868843763,
                6.96551724138,
                202.0,
                0.20145438572,
                0.285737918465,
                0.129348831123,
                0.331903852834,
            ],
        ),
        (
            "b",
            "c",
            [
                -0.209304803758,
                0.26697834729,
                -4.82068965517,
                -139.8,
                -0.253833961032,
                0.175889424396,
                -0.19753827675,
                0.150990932035,
            ],
        ),
        (
            "b",
            "e",
            [
                0.335299782129,
                0.0700915497297,
                13.5724137931,
                393.6,
                0.327295682479,
                0.0774812176415,
                0.26588370842,
                0.046170072205,
            ],
        ),
        (
            "c",
            "e",
            [
                -0.340331660929,
                0.0657336457213,
                -3.53103448276,
                -102.4,
                -0.311158928253,
                0.094187363613,
                -0.219899692372,
                0.112991038913,
            ],
        ),
    ]
}

fn variable_table<'a>(tables: &'a [Table], title: &str, nth: usize) -> &'a Table {
    tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title}"))
}

#[test]
fn pearson_with_cross_products_and_covariances_agrees_with_r() {
    let t = run("CORRELATIONS /VARIABLES=a b c e /PRINT=TWOTAIL /STATISTICS=XPROD.");
    let table = variable_table(&t, "Correlations", 0);
    for (x, y, v) in expected() {
        for (row, want) in [
            ("Pearson Correlation", v[0]),
            ("Sig. (2-tailed)", v[1]),
            ("Covariance", v[2]),
            ("Sum of Squares and Cross-products", v[3]),
        ] {
            near(at(table, &[x, row], y), want, &format!("{x} {y} {row}"));
            near(at(table, &[y, row], x), want, &format!("{y} {x} {row}"));
        }
    }
}

#[test]
fn spearman_and_kendall_agree_with_r() {
    let t = run("NONPAR CORR /VARIABLES=a b c e /PRINT=BOTH TWOTAIL.");
    let table = variable_table(&t, "Correlations", 0);
    for (x, y, v) in expected() {
        for (test, coefficient, p) in [
            ("Kendall's tau_b", v[6], v[7]),
            ("Spearman's rho", v[4], v[5]),
        ] {
            let what = format!("{test} {x} {y}");
            near(
                at(table, &[test, x, "Correlation Coefficient"], y),
                coefficient,
                &what,
            );
            near(at(table, &[test, x, "Sig. (2-tailed)"], y), p, &what);
        }
    }
}

#[test]
fn partial_correlations_agree_with_r() {
    // a and b controlling for c; a and e controlling for b and c
    let t = run("PARTIAL CORR /VARIABLES=a b BY c /SIGNIFICANCE=TWOTAIL.\n\
                 PARTIAL CORR /VARIABLES=a e BY b c /SIGNIFICANCE=TWOTAIL.");
    for (nth, control, y, r, df, p) in [
        (0, "c", "b", 0.519917816075, 27.0, 0.00384231869238),
        (1, "b & c", "e", 0.0265705031447, 26.0, 0.893235633949),
    ] {
        let table = variable_table(&t, "Correlations", nth);
        let key = [control, "a"];
        near(at(table, &[key[0], key[1], "Correlation"], y), r, "r");
        near(
            at(table, &[key[0], key[1], "Significance (2-tailed)"], y),
            p,
            "p",
        );
        near(at(table, &[key[0], key[1], "df"], y), df, "df");
    }
}
