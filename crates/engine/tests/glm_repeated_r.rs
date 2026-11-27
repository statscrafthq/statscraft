//! GLM with two within-subjects factors against R on 24 invented subjects (groups of 10 and 14) and
//! six measures, time (3 levels) by task (2 levels): for each within effect (time, task and
//! time * task) and each with the between factor, Mauchly's test of sphericity with the epsilons
//! (Greenhouse-Geisser, Huynh-Feldt as the manual caps it at 1, lower bound), the univariate tests
//! under sphericity assumed and each correction, the multivariate tests (Pillai's trace and
//! Wilks' lambda), the tests of the within-subjects contrasts (polynomial for time, one contrast
//! for task, the products for time * task), and the between-subjects tests on the average of the
//! cells. R has no routine for this on a design with two within factors and no `car`, so the
//! script writes the hypothesis and error SSCP matrices of the orthonormal contrasts of each
//! effect from `lm` on a matrix response (Type III for an unbalanced between factor: the
//! unweighted mean of the group means). Scripts in lab/oracles/glm-rm-r.
//!
//! One convention is on record: the tests of the within-subjects contrasts sum the levels of the
//! other within factors instead of averaging them, so their sums of squares are those of the
//! orthonormal contrast times the number of those levels (2 for time, 3 for task); F and the
//! significance do not change.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y1 y2 y3 y4 y5 y6.
BEGIN DATA.
1 8.56 12.42 12.99 12.2 14.09 12.51
1 10.38 9.13 12.95 15.24 16.79 19.01
1 9.72 12.02 11.28 11.62 11.45 16.53
1 12.21 17.36 13.74 17.62 19.24 18.45
1 14.18 14.89 15.38 19.33 16.92 16.9
1 9.28 13.26 10.38 10.77 16.56 17.61
1 13.04 15.94 14.27 16.99 17.71 18.66
1 12.47 14.44 12.54 13.3 18.74 16.51
1 8.2 9.29 7.66 12.14 15.04 17.3
1 9.35 10.12 11.21 10.96 14.2 12.12
2 9.29 10.1 12.05 14.91 15.05 15.53
2 7.77 9.65 9.03 11.47 12.14 14.56
2 13.89 11.62 12.24 17.43 17.19 19.11
2 11.52 13.36 14.3 14.22 19 17.53
2 11.32 13.35 12.42 13.87 15.06 20.33
2 9.77 8.9 9.84 13.17 12.86 19.15
2 12.1 15.82 15.51 15.81 17.3 23.82
2 9.87 8.16 10.73 14.61 12.78 12.94
2 9.21 8.32 9.81 12.61 14.01 13.8
2 9.05 9.26 9.28 14.26 13.78 13.19
2 8.22 8.56 8.46 12.99 13.59 12.79
2 12.95 12.99 13.17 14.71 16.76 15.68
2 10.86 9.28 12.23 11.84 15.92 18.11
2 8.27 11.71 10.57 13.02 12.16 13.81
END DATA.
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

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
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

const EFFECTS: [(&str, &str); 3] = [
    ("time", "time * g"),
    ("task", "task * g"),
    ("time * task", "time * task * g"),
];

const CORRECTIONS: [&str; 4] = [
    "Sphericity Assumed",
    "Greenhouse-Geisser",
    "Huynh-Feldt",
    "Lower-bound",
];

#[test]
fn sphericity_and_the_univariate_tests_with_each_correction_agree_with_r() {
    let t = run("GLM y1 y2 y3 y4 y5 y6 BY g /WSFACTOR=time 3 task 2.");
    // W, chi-square, df, p, then the three epsilons
    let mauchly = [
        [
            0.7681073032,
            5.540342595,
            2.0,
            0.0626512718,
            0.8117590133,
            0.9070366682,
            0.5,
        ],
        [1.0, 0.0, 0.0, f64::NAN, 1.0, 1.0, 1.0],
        [
            0.9920590759,
            0.1674250451,
            2.0,
            0.9196956098,
            0.9921216373,
            1.0,
            0.5,
        ],
    ];
    for ((effect, _), w) in EFFECTS.iter().zip(mauchly) {
        let at = |c: &str| value(&t, "Mauchly's Test of Sphericity", effect, c);
        near(at("Mauchly's W"), w[0], effect);
        near(at("Approx. Chi-Square"), w[1], effect);
        near(at("df"), w[2], effect);
        near(at("Epsilon / Greenhouse-Geisser"), w[4], effect);
        near(at("Epsilon / Huynh-Feldt"), w[5], effect);
        near(at("Epsilon / Lower-bound"), w[6], effect);
        if w[2] > 0.0 {
            near(at("Sig."), w[3], effect);
        }
    }
    // each effect and its interaction with g, under each correction: SS, df, MS, F, p
    let within: [[[f64; 5]; 4]; 6] = [
        [
            [552.4613297, 2.0, 276.2306648, 116.1481552, 2.790660498e-18],
            [
                552.4613297,
                1.623518027,
                340.2865386,
                116.1481552,
                2.916331595e-15,
            ],
            [
                552.4613297,
                1.814073336,
                304.541894,
                116.1481552,
                8.628704747e-17,
            ],
            [552.4613297, 1.0, 552.4613297, 116.1481552, 3.0405143e-10],
        ],
        [
            [4.073729683, 2.0, 2.036864841, 0.8564512334, 0.4316237652],
            [
                4.073729683,
                1.623518027,
                2.509198922,
                0.8564512334,
                0.4121426645,
            ],
            [
                4.073729683,
                1.814073336,
                2.245625687,
                0.8564512334,
                0.4225743559,
            ],
            [4.073729683, 1.0, 4.073729683, 0.8564512334, 0.3647746633],
        ],
        [
            [79.72047161, 1.0, 79.72047161, 41.19652743, 1.855751862e-06],
            [79.72047161, 1.0, 79.72047161, 41.19652743, 1.855751862e-06],
            [79.72047161, 1.0, 79.72047161, 41.19652743, 1.855751862e-06],
            [79.72047161, 1.0, 79.72047161, 41.19652743, 1.855751862e-06],
        ],
        [
            [
                0.05291049603,
                1.0,
                0.05291049603,
                0.02734214509,
                0.8701749373,
            ],
            [
                0.05291049603,
                1.0,
                0.05291049603,
                0.02734214509,
                0.8701749373,
            ],
            [
                0.05291049603,
                1.0,
                0.05291049603,
                0.02734214509,
                0.8701749373,
            ],
            [
                0.05291049603,
                1.0,
                0.05291049603,
                0.02734214509,
                0.8701749373,
            ],
        ],
        [
            [7.571429048, 2.0, 3.785714524, 1.574785425, 0.2184985254],
            [
                7.571429048,
                1.984243275,
                3.815776596,
                1.574785425,
                0.2186951658,
            ],
            [7.571429048, 2.0, 3.785714524, 1.574785425, 0.2184985254],
            [7.571429048, 1.0, 7.571429048, 1.574785425, 0.2226811482],
        ],
        [
            [13.26934016, 2.0, 6.634670079, 2.75989689, 0.07427192477],
            [
                13.26934016,
                1.984243275,
                6.687355491,
                2.75989689,
                0.0747598113,
            ],
            [13.26934016, 2.0, 6.634670079, 2.75989689, 0.07427192477],
            [13.26934016, 1.0, 13.26934016, 2.75989689, 0.1108403707],
        ],
    ];
    let errors = [
        [104.6434981, 44.0, 2.37826132],
        [42.57277214, 22.0, 1.935126006],
        [105.7740543, 44.0, 2.403955779],
    ];
    let sources: Vec<&str> = EFFECTS.iter().flat_map(|(a, b)| [*a, *b]).collect();
    for (k, (source, rows)) in sources.iter().zip(within).enumerate() {
        for (correction, w) in CORRECTIONS.iter().zip(rows) {
            let key = format!("{source} / {correction}");
            let at = |c: &str| value(&t, "Tests of Within-Subjects Effects", &key, c);
            near(at("Type III Sum of Squares"), w[0], &key);
            near(at("df"), w[1], &key);
            near(at("Mean Square"), w[2], &key);
            near(at("F"), w[3], &key);
            near(at("Sig."), w[4], &key);
            // the error term has the same epsilon: its df is the assumed df times it
            let error = errors[k / 2];
            let eps = w[1] / rows[0][1];
            let key = format!("Error({}) / {correction}", EFFECTS[k / 2].0);
            let at = |c: &str| value(&t, "Tests of Within-Subjects Effects", &key, c);
            near(at("Type III Sum of Squares"), error[0], &key);
            near(at("df"), error[1] * eps, &key);
            near(at("Mean Square"), error[0] / (error[1] * eps), &key);
        }
    }
}

#[test]
fn the_multivariate_tests_of_each_within_effect_agree_with_r() {
    let t = run("GLM y1 y2 y3 y4 y5 y6 BY g /WSFACTOR=time 3 task 2.");
    // Pillai's trace and Wilks' lambda: value, F, hypothesis df, error df, p
    let multivariate: [[[f64; 5]; 2]; 6] = [
        [
            [0.9176233595, 116.9633189, 2.0, 21.0, 4.129931083e-12],
            [0.08237664052, 116.9633189, 2.0, 21.0, 4.129931083e-12],
        ],
        [
            [0.1304049772, 1.574586129, 2.0, 21.0, 0.2305838978],
            [0.8695950228, 1.574586129, 2.0, 21.0, 0.2305838978],
        ],
        [
            [0.6518796064, 41.19652743, 1.0, 22.0, 1.855751862e-06],
            [0.3481203936, 41.19652743, 1.0, 22.0, 1.855751862e-06],
        ],
        [
            [0.001241282081, 0.02734214509, 1.0, 22.0, 0.8701749373],
            [0.9987587179, 0.02734214509, 1.0, 22.0, 0.8701749373],
        ],
        [
            [0.1178733752, 1.403053036, 2.0, 21.0, 0.2679629616],
            [0.8821266248, 1.403053036, 2.0, 21.0, 0.2679629616],
        ],
        [
            [0.2154143412, 2.882859962, 2.0, 21.0, 0.07829316289],
            [0.7845856588, 2.882859962, 2.0, 21.0, 0.07829316289],
        ],
    ];
    let sources: Vec<&str> = EFFECTS.iter().flat_map(|(a, b)| [*a, *b]).collect();
    for (source, rows) in sources.iter().zip(multivariate) {
        for (test, w) in ["Pillai's Trace", "Wilks' Lambda"].iter().zip(rows) {
            let key = format!("{source} / {test}");
            for (c, k) in [
                ("Value", 0),
                ("F", 1),
                ("Hypothesis df", 2),
                ("Error df", 3),
                ("Sig.", 4),
            ] {
                near(
                    value(&t, "Multivariate Tests", &key, c),
                    w[k],
                    &format!("{key} {c}"),
                );
            }
        }
    }
}

#[test]
fn the_contrast_tests_and_the_between_subjects_tests_agree_with_r() {
    let t = run("GLM y1 y2 y3 y4 y5 y6 BY g /WSFACTOR=time 3 task 2.");
    // per contrast: the effect's SS, F, p; its interaction with g: SS, F, p; the error SS and MS
    let contrasts: [(&str, &str, [f64; 8]); 5] = [
        (
            "time",
            "Linear",
            [
                1075.936126,
                217.8736755,
                6.787582087e-13,
                4.134259286,
                0.8371744794,
                0.3701264132,
                108.6436657,
                4.938348442,
            ],
        ),
        (
            "time",
            "Quadratic",
            [
                28.98653341,
                6.336274168,
                0.01961554582,
                4.013200079,
                0.8772603344,
                0.3591219412,
                100.6433305,
                4.57469684,
            ],
        ),
        (
            "task",
            "Linear",
            [
                239.1614148,
                41.19652743,
                1.855751862e-06,
                0.1587314881,
                0.02734214509,
                0.8701749373,
                127.7183164,
                5.805378019,
            ],
        ),
        (
            "time * task",
            "Linear * Linear",
            [
                0.419420119,
                0.1823379227,
                0.6735216755,
                11.33322012,
                4.926983043,
                0.03705830961,
                50.60517571,
                2.30023526,
            ],
        ),
        (
            "time * task",
            "Quadratic * Linear",
            [
                7.152008929,
                2.852046308,
                0.1053832002,
                1.93612004,
                0.7720773374,
                0.3890714979,
                55.16887857,
                2.507676299,
            ],
        ),
    ];
    for (effect, contrast, w) in contrasts {
        let interaction = EFFECTS
            .iter()
            .find(|(a, _)| *a == effect)
            .expect("an effect")
            .1;
        let at = |source: &str, c: &str| {
            value(
                &t,
                "Tests of Within-Subjects Contrasts",
                &format!("{source} / {contrast}"),
                c,
            )
        };
        near(at(effect, "Type III Sum of Squares"), w[0], effect);
        near(at(effect, "F"), w[1], effect);
        near(at(effect, "Sig."), w[2], effect);
        near(
            at(interaction, "Type III Sum of Squares"),
            w[3],
            interaction,
        );
        near(at(interaction, "F"), w[4], interaction);
        near(at(interaction, "Sig."), w[5], interaction);
        near(
            at(&format!("Error({effect})"), "Type III Sum of Squares"),
            w[6],
            effect,
        );
        near(at(&format!("Error({effect})"), "Mean Square"), w[7], effect);
    }
    // the average of the six cells
    let (intercept, g, error) = (
        [24953.77557, 1102.421458, 2.729949808e-20],
        [22.6393024, 1.000171405, 0.3281427155],
        [497.9792969, 22.63542259],
    );
    for (row, w) in [("Intercept", intercept), ("g", g)] {
        near(
            value(
                &t,
                "Tests of Between-Subjects Effects",
                row,
                "Type III Sum of Squares",
            ),
            w[0],
            row,
        );
        near(
            value(&t, "Tests of Between-Subjects Effects", row, "F"),
            w[1],
            row,
        );
        near(
            value(&t, "Tests of Between-Subjects Effects", row, "Sig."),
            w[2],
            row,
        );
    }
    near(
        value(
            &t,
            "Tests of Between-Subjects Effects",
            "Error",
            "Type III Sum of Squares",
        ),
        error[0],
        "error",
    );
    near(
        value(
            &t,
            "Tests of Between-Subjects Effects",
            "Error",
            "Mean Square",
        ),
        error[1],
        "error",
    );
}
