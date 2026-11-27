//! CORRELATIONS, NONPAR CORR and PARTIAL CORR with missing values against R on 44 invented cases of
//! four variables with 2 to 4 values missing in each (one of them a user-missing 999): pairwise
//! deletion (each coefficient from the cases valid on its two variables, with its own N, the
//! one-tailed significance, the cross-products and covariances of the pair), listwise deletion
//! with a WITH list, MISSING=INCLUDE, Spearman's rho (t approximation) and Kendall's tau-b
//! (tie-corrected normal approximation) with ONETAIL, and partial correlations: two controls with
//! listwise deletion (the zero-order block, the descriptives and the partial block with N - 2 - 2
//! degrees of freedom) and one control with MISSING=ANALYSIS (the partial coefficient from the
//! pairwise zero-order ones, degrees of freedom from the smallest pair count less 2 less the
//! order). Scripts in lab/oracles/correlations-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c d (4F8.2).
BEGIN DATA.
30 40 5.9 94
33 30 9 89
. 25 6.2 84
46 52 9.4 96
52 . 5.5 84
50 48 9.7 100
65 47 6.9 75
37 38 . 104
69 62 10.1 70
37 999 8.7 61
63 57 5.7 76
63 52 5.8 .
60 43 8.2 113
50 33 6 102
62 49 7 80
82 58 7.1 70
. . 7.8 90
53 48 4.6 55
62 47 9.5 72
48 43 8.9 79
57 46 9 67
53 70 7.6 64
60 55 5.3 63
75 36 10.6 68
57 52 9.6 .
35 49 4 67
33 44 7 87
48 50 12.4 62
46 32 8.4 88
62 . . 92
62 51 8.5 82
59 44 7.5 88
55 53 10 67
46 35 6.9 96
49 40 9.4 75
48 56 7.8 73
48 44 6.5 113
43 37 5.6 100
38 36 8.4 70
40 36 5.2 80
44 38 . 81
46 41 8.1 82
52 51 7.5 83
48 40 7.2 97
END DATA.
MISSING VALUES b (999).
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

/// The row headers joined with " / ", an empty header taking the one above it.
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

fn value(tables: &[Table], title: &str, nth: usize, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title} number {nth}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

const VARS: [&str; 4] = ["a", "b", "c", "d"];
const PAIRS: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

#[test]
fn pairwise_deletion_gives_each_pair_its_own_cases() {
    let t = run("CORRELATIONS /VARIABLES=a b c d /PRINT=ONETAIL /STATISTICS=DESCRIPTIVES XPROD.");
    let desc = [
        [51.57142857, 11.50851828, 42.0],
        [45.2, 9.279699376, 40.0],
        [7.670731707, 1.809453495, 41.0],
        [81.88095238, 14.33433943, 42.0],
    ];
    for (v, want) in VARS.iter().zip(desc) {
        near(
            value(&t, "Descriptive Statistics", 0, v, "Mean"),
            want[0],
            v,
        );
        near(
            value(&t, "Descriptive Statistics", 0, v, "Std. Deviation"),
            want[1],
            v,
        );
        near(value(&t, "Descriptive Statistics", 0, v, "N"), want[2], v);
    }
    // N, r, one-tailed p, two-tailed p, cross-products, covariance
    let pairs: [[f64; 6]; 6] = [
        [
            39.0,
            0.4952568976,
            0.0006739913845,
            0.001347982769,
            1919.333333,
            50.50877193,
        ],
        [
            39.0,
            0.1719544544,
            0.1476100592,
            0.2952201184,
            138.625641,
            3.648043185,
        ],
        [
            40.0,
            -0.2461239169,
            0.06289428309,
            0.1257885662,
            -1631.75,
            -41.83974359,
        ],
        [
            38.0,
            0.09449450207,
            0.2862716386,
            0.5725432773,
            60.25789474,
            1.62859175,
        ],
        [
            38.0,
            -0.4499646706,
            0.002295090706,
            0.004590181411,
            -2271.631579,
            -61.39544808,
        ],
        [
            39.0,
            -0.1390902943,
            0.199202408,
            0.3984048161,
            -136.7076923,
            -3.59757085,
        ],
    ];
    for ((i, j), want) in PAIRS.iter().zip(pairs) {
        for (row, col) in [(VARS[*i], VARS[*j]), (VARS[*j], VARS[*i])] {
            let at = |kind: &str| value(&t, "Correlations", 0, &format!("{row} / {kind}"), col);
            near(at("N"), want[0], row);
            near(at("Pearson Correlation"), want[1], row);
            near(at("Sig. (1-tailed)"), want[2], row);
            near(at("Sum of Squares and Cross-products"), want[4], row);
            near(at("Covariance"), want[5], row);
        }
    }
    // the diagonal holds each variable's own sums over its valid cases
    let diag: [[f64; 2]; 4] = [
        [5430.285714, 132.445993],
        [3358.4, 86.11282051],
        [130.964878, 3.274121951],
        [8424.404762, 205.4732869],
    ];
    for (v, want) in VARS.iter().zip(diag) {
        near(
            value(
                &t,
                "Correlations",
                0,
                &format!("{v} / Sum of Squares and Cross-products"),
                v,
            ),
            want[0],
            v,
        );
        near(
            value(&t, "Correlations", 0, &format!("{v} / Covariance"), v),
            want[1],
            v,
        );
    }
}

#[test]
fn listwise_deletion_with_lists_and_user_missing_values_agree_with_r() {
    let t = run("CORRELATIONS /VARIABLES=a b WITH c d /MISSING=LISTWISE.");
    let listwise = [
        [0.2247029805, 0.1943726773],
        [-0.2936628769, 0.08685608776],
        [0.0516497664, 0.7682509023],
        [-0.4608142762, 0.005338410865],
    ];
    for (k, (row, col)) in [("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")]
        .iter()
        .enumerate()
    {
        near(
            value(
                &t,
                "Correlations",
                0,
                &format!("{row} / Pearson Correlation"),
                col,
            ),
            listwise[k][0],
            row,
        );
        near(
            value(
                &t,
                "Correlations",
                0,
                &format!("{row} / Sig. (2-tailed)"),
                col,
            ),
            listwise[k][1],
            row,
        );
    }
    // 999 is user-missing for b: left out by default, a valid value with INCLUDE
    let include = [40.0, -0.1703412278, 0.2933161225];
    let t = run("CORRELATIONS /VARIABLES=a b /MISSING=PAIRWISE INCLUDE.");
    near(
        value(&t, "Correlations", 0, "a / N", "b"),
        include[0],
        "N with INCLUDE",
    );
    near(
        value(&t, "Correlations", 0, "a / Pearson Correlation", "b"),
        include[1],
        "r with INCLUDE",
    );
    near(
        value(&t, "Correlations", 0, "a / Sig. (2-tailed)", "b"),
        include[2],
        "p with INCLUDE",
    );
}

#[test]
fn spearman_and_kendall_with_missing_values_agree_with_r() {
    let t = run("NONPAR CORR /VARIABLES=a b c /PRINT=BOTH ONETAIL.");
    let pairs = [(0, 1), (0, 2), (1, 2)];
    let (spearman, kendall) = (
        [
            [39.0, 0.5542189338, 0.0001257662869, 0.0002515325738],
            [39.0, 0.1389440632, 0.1994529036, 0.3989058072],
            [38.0, 0.1029532976, 0.2692495405, 0.538499081],
        ],
        [
            [39.0, 0.4058421889, 0.0001951167045, 0.000390233409],
            [39.0, 0.09518146835, 0.2012265586, 0.4024531171],
            [38.0, 0.08387828587, 0.2324978291, 0.4649956583],
        ],
    );
    for (k, (i, j)) in pairs.iter().enumerate() {
        for (row, col) in [(VARS[*i], VARS[*j]), (VARS[*j], VARS[*i])] {
            for (kind, want, label) in [
                ("Spearman's rho", spearman[k], "Correlation Coefficient"),
                ("Kendall's tau_b", kendall[k], "Correlation Coefficient"),
            ] {
                let at =
                    |c: &str| value(&t, "Correlations", 0, &format!("{kind} / {row} / {c}"), col);
                near(at("N"), want[0], kind);
                near(at(label), want[1], kind);
                near(at("Sig. (1-tailed)"), want[2], kind);
            }
        }
    }
    // listwise over a b c: 37 cases whatever the pair (the table shows no N then)
    let t = run("NONPAR CORR /VARIABLES=a b c /PRINT=BOTH /MISSING=LISTWISE.");
    let (spearman, kendall) = (
        [
            [37.0, 0.531140664, 0.0003593155152, 0.0007186310305],
            [37.0, 0.1736601667, 0.1519951128, 0.3039902257],
            [37.0, 0.06773525986, 0.3451919778, 0.6903839557],
        ],
        [
            [37.0, 0.3907051876, 0.0004644620798, 0.0009289241596],
            [37.0, 0.1245328813, 0.1438675453, 0.2877350906],
            [37.0, 0.05958957539, 0.3046079326, 0.6092158652],
        ],
    );
    for (k, (i, j)) in pairs.iter().enumerate() {
        for (kind, want) in [
            ("Spearman's rho", spearman[k]),
            ("Kendall's tau_b", kendall[k]),
        ] {
            let at = |c: &str| {
                value(
                    &t,
                    "Correlations",
                    0,
                    &format!("{kind} / {} / {c}", VARS[*i]),
                    VARS[*j],
                )
            };
            near(at("Correlation Coefficient"), want[1], kind);
            near(at("Sig. (2-tailed)"), want[3], kind);
        }
    }
}

#[test]
fn partial_correlations_with_listwise_and_pairwise_deletion_agree_with_r() {
    let t = run(
        "PARTIAL CORR /VARIABLES=a b BY c d /SIGNIFICANCE=ONETAIL /STATISTICS=CORR DESCRIPTIVES /MISSING=LISTWISE.",
    );
    let desc = [
        [51.82857143, 11.72851155, 35.0],
        [45.8, 8.979715704, 35.0],
        [7.74, 1.844100291, 35.0],
        [81.22857143, 14.659038, 35.0],
    ];
    for (v, want) in VARS.iter().zip(desc) {
        near(
            value(&t, "Descriptive Statistics", 0, v, "Mean"),
            want[0],
            v,
        );
        near(
            value(&t, "Descriptive Statistics", 0, v, "Std. Deviation"),
            want[1],
            v,
        );
        near(value(&t, "Descriptive Statistics", 0, v, "N"), want[2], v);
    }
    // the zero-order block: r, one-tailed p, two-tailed p, df = N - 2
    let zero: [[f64; 4]; 6] = [
        [0.4584963125, 0.002802339327, 0.005604678655, 33.0],
        [0.2247029805, 0.09718633866, 0.1943726773, 33.0],
        [-0.2936628769, 0.04342804388, 0.08685608776, 33.0],
        [0.0516497664, 0.3841254512, 0.7682509023, 33.0],
        [-0.4608142762, 0.002669205433, 0.005338410865, 33.0],
        [-0.1163295441, 0.2528699021, 0.5057398041, 33.0],
    ];
    for ((i, j), want) in PAIRS.iter().zip(zero) {
        for (row, col) in [(VARS[*i], VARS[*j]), (VARS[*j], VARS[*i])] {
            let at = |kind: &str| {
                value(
                    &t,
                    "Correlations",
                    0,
                    &format!("-none- / {row} / {kind}"),
                    col,
                )
            };
            near(at("Correlation"), want[0], row);
            near(at("Significance (1-tailed)"), want[1], row);
            near(at("df"), want[3], row);
        }
    }
    // a with b controlling for c and d: r, one-tailed p, two-tailed p, df = N - 2 - 2
    let partial = [0.3893019793, 0.01256787052, 0.02513574103, 31.0];
    for (row, col) in [("a", "b"), ("b", "a")] {
        let at = |kind: &str| {
            value(
                &t,
                "Correlations",
                0,
                &format!("c & d / {row} / {kind}"),
                col,
            )
        };
        near(at("Correlation"), partial[0], row);
        near(at("Significance (1-tailed)"), partial[1], row);
        near(at("df"), partial[3], row);
    }
    // pairwise deletion: from the zero-order coefficients of the pairs, df from the smallest pair N
    let t = run("PARTIAL CORR /VARIABLES=a b BY c /MISSING=ANALYSIS.");
    let pairwise = [0.4884364684, 0.002161227794, 35.0];
    near(
        value(&t, "Correlations", 0, "c / a / Correlation", "b"),
        pairwise[0],
        "pairwise r",
    );
    near(
        value(
            &t,
            "Correlations",
            0,
            "c / a / Significance (2-tailed)",
            "b",
        ),
        pairwise[1],
        "pairwise p",
    );
    near(
        value(&t, "Correlations", 0, "c / a / df", "b"),
        pairwise[2],
        "pairwise df",
    );
}
