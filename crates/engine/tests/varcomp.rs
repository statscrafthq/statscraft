//! VARCOMP on an invented crossed design: a fixed factor a (2 levels), random factors b (3)
//! and c (4) and the random interaction a*b, two cases per cell (48), the same with ten cases
//! dropped (38), and that unbalanced file with regression weights 1, 2 and 0.5 in turn.
//!
//! The expected numbers come from R 4.3.3 run as a separate program on the same values, with
//! dense n x n matrices: MINQUE straight from its definition (S_ij = tr(R V_i R V_j),
//! q_i = y'R V_i R y), ML and REML by iterating the score equations to a relative change of
//! 1E-14 with the inverse expected information as their covariance, and the ANOVA method from
//! projections (Type I sequential, Type III by sum-to-zero coding with each effect dropped),
//! each coefficient of an expected mean square being tr(Z_i'M Z_i) / df. In the balanced file
//! the coefficients agree with the textbook ones (16 and 8 for b, 12 for c, 8 for a*b) and all
//! methods but ML give the same variances.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// The dependent values in the order a, b, c, replicate (the last fastest).
const Y: [f64; 48] = [
    17.8, 19.5, 17.6, 16.5, 20.0, 19.5, 19.7, 18.7, 20.7, 21.8, 20.3, 20.7, 24.4, 23.4, 22.9, 23.8,
    17.2, 17.8, 17.0, 16.3, 21.5, 20.0, 18.5, 20.9, 19.9, 21.3, 21.1, 19.8, 25.8, 21.2, 23.1, 21.9,
    21.8, 23.5, 21.6, 22.9, 25.9, 22.7, 22.0, 24.4, 20.2, 20.9, 17.8, 18.1, 21.6, 20.8, 21.0, 23.4,
];

/// The cases (counted from 1) left out of the unbalanced file.
const DROPPED: [usize; 10] = [2, 5, 11, 12, 19, 26, 31, 38, 41, 47];

fn data(unbalanced: bool) -> String {
    let mut lines = Vec::new();
    for (i, y) in Y.iter().enumerate() {
        if unbalanced && DROPPED.contains(&(i + 1)) {
            continue;
        }
        let (a, b, c) = (i / 24 + 1, (i / 8) % 3 + 1, (i / 2) % 4 + 1);
        // the weights 1, 2 and 0.5 by the position in the file, counted from 1
        let w = [1.0, 2.0, 0.5][(lines.len() + 1) % 3];
        lines.push(format!("{a} {b} {c} {y} {w}"));
    }
    format!(
        "DATA LIST LIST /a b c y w.\nBEGIN DATA\n{}\nEND DATA.\n",
        lines.join("\n")
    )
}

fn run(source: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(table: &Table, row: &str, column: &str) -> f64 {
    let r = table
        .rows
        .iter()
        .position(|cells| matches!(&cells[0], Cell::Text(t) if t == row))
        .unwrap_or_else(|| panic!("{}: no row {row}", table.title));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{}: no column {column}", table.title));
    match table.rows[r][c] {
        Cell::Number(n) => n,
        ref other => panic!("{}: {row} / {column} is {other:?}", table.title),
    }
}

const COMPONENTS: [&str; 4] = ["Var(b)", "Var(c)", "Var(a * b)", "Var(Error)"];

fn close(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1.0),
        "{what}: got {got}, want {want}"
    );
}

fn estimates(unbalanced: bool, options: &str) -> (Engine, Vec<Table>) {
    run(&format!(
        "{}VARCOMP y BY a b c /RANDOM=b c /DESIGN=a b c a*b {options}.",
        data(unbalanced)
    ))
}

fn check(unbalanced: bool, options: &str, want: [f64; 4]) {
    let (_, tables) = estimates(unbalanced, options);
    let t = find(&tables, "Variance Estimates");
    for (name, w) in COMPONENTS.iter().zip(want) {
        close(number(t, name, "Estimate"), w, &format!("{options} {name}"));
    }
}

const BALANCED: [f64; 4] = [
    2.37739583333333,
    1.88425391737891,
    0.483707264957265,
    1.22284188034188,
];

#[test]
fn balanced_design_every_method() {
    for method in [
        "",
        "/METHOD=MINQUE(0)",
        "/METHOD=MINQUE(1)",
        "/METHOD=REML",
        "/METHOD=SSTYPE(1)",
        "/METHOD=SSTYPE(3)",
    ] {
        check(false, method, BALANCED);
    }
    check(
        false,
        "/METHOD=ML",
        [
            1.84246334851452,
            1.65091518203771,
            0.271428687026038,
            1.22357050379169,
        ],
    );
}

#[test]
fn unbalanced_design_every_method() {
    let cases: [(&str, [f64; 4]); 6] = [
        (
            "/METHOD=MINQUE(0)",
            [
                2.95776360386399,
                2.50860149175031,
                0.609114325355412,
                1.10304882657898,
            ],
        ),
        (
            "/METHOD=MINQUE",
            [
                2.88711264662476,
                2.16117256797914,
                0.376582651124798,
                1.37154719316891,
            ],
        ),
        (
            "/METHOD=ML",
            [
                2.20331631552055,
                1.96558128969821,
                0.158677589654194,
                1.37157499638841,
            ],
        ),
        (
            "/METHOD=REML",
            [
                2.86434224662798,
                2.21046732516051,
                0.352968974028332,
                1.37167905662175,
            ],
        ),
        (
            "/METHOD=SSTYPE(1)",
            [
                3.0293212073293,
                2.35204191259552,
                0.324377458670759,
                1.37487464765196,
            ],
        ),
        (
            "/METHOD=SSTYPE",
            [
                2.92406987379683,
                2.12142973259653,
                0.324377458669288,
                1.37487464765196,
            ],
        ),
    ];
    for (method, want) in cases {
        check(true, method, want);
    }
}

#[test]
fn regression_weights_every_method() {
    let cases: [(&str, [f64; 4]); 6] = [
        (
            "/METHOD=MINQUE(0)",
            [
                2.60788008894058,
                2.33216489451985,
                0.502272968523296,
                1.28410705301277,
            ],
        ),
        (
            "/METHOD=MINQUE(1)",
            [
                2.53368691103329,
                1.99577516876932,
                0.545382792287131,
                1.43075309639869,
            ],
        ),
        (
            "/METHOD=ML",
            [
                1.94711532597374,
                1.76028313237101,
                0.298768654405168,
                1.43055750759557,
            ],
        ),
        (
            "/METHOD=REML",
            [
                2.51994438346521,
                2.00230211179998,
                0.541058970237087,
                1.4312451951349,
            ],
        ),
        (
            "/METHOD=SSTYPE(1)",
            [
                2.60562516393323,
                2.13967415202084,
                0.532015925726958,
                1.43330368340209,
            ],
        ),
        (
            "/METHOD=SSTYPE(3)",
            [
                2.52589417151163,
                2.07591439088962,
                0.532015925725489,
                1.43330368340209,
            ],
        ),
    ];
    for (method, want) in cases {
        check(true, &format!("{method} /REGWGT=w"), want);
    }
}

#[test]
fn likelihood_covariance_and_history() {
    // the inverse expected information in the variances, by R as described at the top
    let ml: [[f64; 4]; 4] = [
        [
            4.87394301825070,
            -0.343501044221438,
            -0.0489456719902080,
            0.00170578540885933,
        ],
        [
            -0.343501044221438,
            2.84761632540873,
            0.00151937155109888,
            -0.0131775417267580,
        ],
        [
            -0.0489456719902080,
            0.00151937155109888,
            0.100708145717070,
            -0.0210620489093508,
        ],
        [
            0.00170578540885933,
            -0.0131775417267580,
            -0.0210620489093508,
            0.129692924585560,
        ],
    ];
    let reml: [[f64; 4]; 4] = [
        [
            10.0192184091182,
            -0.0000657330927338312,
            -0.165289829963700,
            -0.0000321345294499643,
        ],
        [
            -0.0000657330927338312,
            3.71778428404666,
            0.00182407515604871,
            -0.0141525035678180,
        ],
        [
            -0.165289829963700,
            0.00182407515604871,
            0.333856696165046,
            -0.0209546125614945,
        ],
        [
            -0.0000321345294499643,
            -0.0141525035678180,
            -0.0209546125614945,
            0.129746124453378,
        ],
    ];
    for (method, cov, ll) in [
        ("ML", ml, -69.6097064749676),
        ("REML", reml, -65.2331778237036),
    ] {
        let (_, tables) = estimates(true, &format!("/METHOD={method} /PRINT=HISTORY(3)"));
        let t = find(&tables, "Asymptotic Covariance Matrix");
        for (i, row) in COMPONENTS.iter().enumerate() {
            for (j, column) in COMPONENTS.iter().enumerate() {
                let got = number(t, row, column);
                assert!(
                    (got - cov[i][j]).abs() <= 1e-7 * cov[i][i].abs().max(1.0),
                    "{method} {row} {column}: {got} vs {}",
                    cov[i][j]
                );
            }
        }
        let h = find(&tables, "Iteration History");
        let last = h.rows.last().unwrap();
        let Cell::Number(got) = last[1] else {
            panic!("{last:?}")
        };
        close(got, ll, method);
        // every third iteration and the last
        for row in &h.rows[..h.rows.len() - 1] {
            let Cell::Text(it) = &row[0] else { panic!() };
            assert_eq!(it.parse::<usize>().unwrap() % 3, 0, "{method}");
        }
    }
}

#[test]
fn sums_of_squares_and_expected_mean_squares() {
    // (type, row, SS, df, coefficients of b, c, a*b and error)
    let rows: [(u8, &str, f64, f64, [f64; 4]); 6] = [
        (
            1,
            "b",
            84.4107697244898,
            2.0,
            [12.6315789473684, 0.218047463875447, 6.32763898489187, 1.0],
        ),
        (
            1,
            "c",
            69.2864265014452,
            3.0,
            [0.0, 9.22305607671461, 0.0850441281934011, 1.0],
        ),
        (
            1,
            "a * b",
            6.75670215005163,
            2.0,
            [0.0, 0.0, 6.17637377018645, 1.0],
        ),
        (
            3,
            "b",
            78.6405484394309,
            2.0,
            [12.2949514125492, 0.0, 6.14747570627461, 1.0],
        ),
        (
            3,
            "c",
            62.0093495038041,
            3.0,
            [0.0, 9.09523809523809, 0.0, 1.0],
        ),
        (
            3,
            "a * b",
            6.75670215003335,
            2.0,
            [0.0, 0.0, 6.17637377018644, 1.0],
        ),
    ];
    for t in [1, 3] {
        let (_, tables) = estimates(true, &format!("/METHOD=SSTYPE({t}) /PRINT=SS EMS"));
        let ss = find(&tables, "ANOVA");
        let ems = find(&tables, "Expected Mean Squares");
        let ss_column = format!("Type {} Sum of Squares", if t == 1 { "I" } else { "III" });
        close(number(ss, "Error", &ss_column), 39.8713647819069, "SSE");
        close(number(ss, "Error", "df"), 29.0, "dfe");
        for (_, row, want_ss, df, coef) in rows.iter().filter(|r| r.0 == t) {
            close(number(ss, row, &ss_column), *want_ss, row);
            close(number(ss, row, "df"), *df, row);
            close(number(ss, row, "Mean Square"), want_ss / df, row);
            for (name, w) in COMPONENTS.iter().zip(coef) {
                let got = number(ems, row, &format!("Variance Component / {name}"));
                assert!(
                    (got - w).abs() <= 1e-8,
                    "type {t} {row} {name}: {got} vs {w}"
                );
            }
        }
        // the fixed factor enters its own expected mean square, never that of a random effect
        let quadratic = |row: &str| {
            let r = ems
                .rows
                .iter()
                .find(|cells| matches!(&cells[0], Cell::Text(x) if x == row))
                .unwrap();
            match r.last().unwrap() {
                Cell::Text(x) => x.clone(),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(quadratic("a"), "a");
        assert_eq!(quadratic("b"), "");
        assert_eq!(quadratic("Error"), "");
    }
}

#[test]
fn a_frequency_weight_counts_the_case_that_many_times() {
    // the first case three times over, against a weight of 3 on it
    let first = "1 1 1 17.8 2\n";
    let tripled = data(true).replacen(first, &first.repeat(3), 1);
    let weighted = format!(
        "{}COMPUTE f = 1.\nIF ($CASENUM = 1) f = 3.\nWEIGHT BY f.\n",
        data(true)
    );
    for method in [
        "MINQUE(0)",
        "MINQUE(1)",
        "ML",
        "REML",
        "SSTYPE(1)",
        "SSTYPE(3)",
    ] {
        let design = format!("VARCOMP y BY a b c /RANDOM=b c /DESIGN=a b c a*b /METHOD={method}.");
        let (_, plain) = run(&format!("{tripled}{design}"));
        let (_, counted) = run(&format!("{weighted}{design}"));
        let (p, c) = (
            find(&plain, "Variance Estimates"),
            find(&counted, "Variance Estimates"),
        );
        for name in COMPONENTS {
            close(
                number(c, name, "Estimate"),
                number(p, name, "Estimate"),
                &format!("{method} {name}"),
            );
        }
    }
}

#[test]
fn outfile_writes_estimates_and_the_correlation_matrix() {
    let source = format!(
        "{}DATASET NAME cells.\nDATASET DECLARE vc.\n\
         VARCOMP y BY a b c /RANDOM=b c /DESIGN=a b c a*b /METHOD=REML /OUTFILE=VAREST CORB (vc).\n\
         DATASET ACTIVATE vc.\n",
        data(true)
    );
    let (engine, tables) = run(&source);
    let est = find(&tables, "Variance Estimates");
    let cov = find(&tables, "Asymptotic Covariance Matrix");
    let out = engine.dataset().unwrap();
    assert_eq!(out.case_count(), 4);
    let estimate = out.variable("Estimate").unwrap().numeric().unwrap();
    for (k, name) in COMPONENTS.iter().enumerate() {
        close(
            estimate.get(k).unwrap(),
            number(est, name, "Estimate"),
            name,
        );
        for (j, other) in COMPONENTS.iter().enumerate() {
            let r = out
                .variable(&format!("VC{}", j + 1))
                .unwrap()
                .numeric()
                .unwrap()
                .get(k)
                .unwrap();
            let want = number(cov, name, other)
                / (number(cov, name, name) * number(cov, other, other)).sqrt();
            close(r, want, &format!("{name} {other}"));
        }
    }
}

#[test]
fn defaults_warnings_and_errors() {
    // the default design is the full factorial; every effect with b or c is random
    let (engine, tables) = run(&format!(
        "{}VARCOMP y BY a b /RANDOM=b /PRINT=EMS.",
        data(false)
    ));
    let t = find(&tables, "Variance Estimates");
    let rows: Vec<String> = t
        .rows
        .iter()
        .map(|r| match &r[0] {
            Cell::Text(x) => x.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(rows, ["Var(b)", "Var(a * b)", "Var(Error)"]);
    assert!(find(&tables, "Factor Level Information").rows.len() == 5);
    assert!(engine.warnings().iter().any(|w| w.contains("SSTYPE only")));
    let fails = |syntax: &str| {
        let (commands, errors) = parse(&format!("{}{syntax}", data(false)));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        let mut last = None;
        for c in &commands {
            if let Err(e) = engine.execute(&c.command) {
                last = Some(e.to_string());
            }
        }
        last.unwrap_or_else(|| panic!("{syntax} did not fail"))
    };
    assert!(fails("VARCOMP y BY a b /RANDOM=c.").contains("not a factor"));
    assert!(fails("VARCOMP y BY a b /RANDOM=b /DESIGN=a.").contains("RANDOM"));
    assert!(
        fails("VARCOMP y BY a b /RANDOM=b /DESIGN=a*b b /METHOD=SSTYPE(1).")
            .contains("degrees of freedom")
    );
}
