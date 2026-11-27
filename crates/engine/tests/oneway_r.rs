//! ONEWAY against R on 36 invented values in four unequal groups (8, 11, 7 and 10 cases) and, for
//! the homogeneous subsets, in five groups (6, 9, 7, 8 and 6 cases) where the range tests part
//! ways: the descriptives with the fixed- and random-effects rows (Brownlee's standard error and
//! the between-groups component), the four Levene tests, the ANOVA table, the Welch and
//! Brown-Forsythe tests (`oneway.test`; the Brown-Forsythe from its formula), contrasts with
//! pooled and separate variances (Satterthwaite's df), the polynomial decomposition (the
//! weighted terms are the sequential sums of squares on the group value, the unweighted terms the
//! equal-weight orthogonal polynomial contrasts), Tamhane's T2, Dunnett's T3 and C (the
//! studentized maximum modulus by integration, `qtukey`), Dunnett's t in its three forms
//! (`mnormt::pmt` and `sadmvt` for the many-to-one t), and the subsets of the SNK, Tukey,
//! Tukey-b, Duncan and the two R-E-G-W tests from the manual's search written out in R. Scripts in
//! lab/oracles/oneway-r.
//!
//! The Dunnett t two-sided significance is twice the one-sided one less the overlap of the two
//! tails, which is far below the precision of the comparison here (1e-10).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y y2.
BEGIN DATA.
1 24.2 11.1
1 21.9 10
1 19.7 12.2
1 19.4 10.9
1 20.6 8.8
1 20.8 5
1 22.1 10.7
1 23.7 13.3
2 26.6 10.1
2 17.6 7
2 21.4 7.8
2 23.2 17.9
2 19.4 10.7
2 23 14.1
2 23.4 17.4
2 30.1 12.3
2 18.8 19.6
2 22.3 10.4
2 23.4 11.7
3 23.7 15.1
3 21.2 16.7
3 19.9 13.7
3 23.5 17.5
3 23.7 18.6
3 22.3 18.2
3 26.8 17
4 30 14.7
4 26 17
4 37.3 18.6
4 20.6 10.9
4 31 15.4
4 30.7 16.7
4 33.6 13
4 40.7 18.1
4 29.3 13.8
4 37.3 20.9
END DATA.
";
const DATA5: &str = "DATA LIST FREE /g y.
BEGIN DATA.
1 12.4
1 8.6
1 14.7
1 10.2
1 11
1 4.3
2 10.5
2 11.8
2 10.9
2 12.6
2 10.6
2 12.6
2 11.9
2 13.3
2 8.9
3 13.9
3 16.5
3 15.4
3 8.1
3 15.3
3 13.7
3 15.2
4 10.4
4 15.4
4 12.2
4 14.5
4 8.8
4 15.1
4 15.9
4 14.6
5 15.2
5 21.2
5 12.7
5 17
5 19.1
5 17.9
END DATA.
";

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
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

fn run(syntax: &str) -> Vec<Table> {
    run_on(DATA, syntax)
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

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
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
    &table.rows[r][c]
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    match cell(tables, title, row, column) {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * (1.0 + expected.abs()),
        "{what}: {actual} is not {expected}"
    );
}

fn near(actual: f64, expected: f64, what: &str) {
    close(actual, expected, 1e-7, what);
}

const CI: &str = "95% Confidence Interval for Mean";

#[test]
fn descriptives_effects_levene_anova_and_the_robust_tests_agree_with_r() {
    let t = run("ONEWAY y BY g /STATISTICS=ALL.");
    let groups: [[f64; 8]; 4] = [
        [
            8.0,
            21.55,
            1.755806693,
            0.6207714095,
            20.08210887,
            23.01789113,
            19.4,
            24.2,
        ],
        [
            11.0,
            22.65454545,
            3.538746568,
            1.066972236,
            20.27718316,
            25.03190775,
            17.6,
            30.1,
        ],
        [
            7.0,
            23.01428571,
            2.198809202,
            0.8310717611,
            20.98072637,
            25.04784506,
            19.9,
            26.8,
        ],
        [
            10.0,
            31.65,
            5.898634494,
            1.865312008,
            27.43037108,
            35.86962892,
            20.6,
            40.7,
        ],
    ];
    for (i, want) in groups.iter().enumerate() {
        let row = format!("y / {}", i + 1);
        for (c, w) in [
            "N",
            "Mean",
            "Std. Deviation",
            "Std. Error",
            &format!("{CI} / Lower Bound"),
            &format!("{CI} / Upper Bound"),
            "Minimum",
            "Maximum",
        ]
        .iter()
        .zip(want)
        {
            near(
                value(&t, "Descriptives", &row, c),
                *w,
                &format!("{row} {c}"),
            );
        }
    }
    let total = [
        36.0,
        24.97777778,
        5.642953184,
        0.9404921973,
        23.06847711,
        26.88707844,
        17.6,
        40.7,
    ];
    for (c, w) in [
        "N",
        "Mean",
        "Std. Deviation",
        "Std. Error",
        &format!("{CI} / Lower Bound"),
        &format!("{CI} / Upper Bound"),
        "Minimum",
        "Maximum",
    ]
    .iter()
    .zip(total)
    {
        near(
            value(&t, "Descriptives", "y / Total", c),
            w,
            &format!("total {c}"),
        );
    }
    let fixed = [3.908967431, 0.6514945719, 23.65072676, 26.30482879];
    near(
        value(&t, "Descriptives", "y / Fixed Effects", "Std. Deviation"),
        fixed[0],
        "fixed sd",
    );
    near(
        value(&t, "Descriptives", "y / Fixed Effects", "Std. Error"),
        fixed[1],
        "fixed se",
    );
    near(
        value(
            &t,
            "Descriptives",
            "y / Fixed Effects",
            &format!("{CI} / Lower Bound"),
        ),
        fixed[2],
        "fixed lower",
    );
    near(
        value(
            &t,
            "Descriptives",
            "y / Fixed Effects",
            &format!("{CI} / Upper Bound"),
        ),
        fixed[3],
        "fixed upper",
    );
    // Brownlee's standard error and Snedecor and Cochran's component
    let random = [21.6936037, 2.452597607, 17.17251759, 32.78303797];
    near(
        value(
            &t,
            "Descriptives",
            "y / Random Effects",
            "Between-Component Variance",
        ),
        random[0],
        "component",
    );
    near(
        value(&t, "Descriptives", "y / Random Effects", "Std. Error"),
        random[1],
        "random se",
    );
    near(
        value(
            &t,
            "Descriptives",
            "y / Random Effects",
            &format!("{CI} / Lower Bound"),
        ),
        random[2],
        "random lower",
    );
    near(
        value(
            &t,
            "Descriptives",
            "y / Random Effects",
            &format!("{CI} / Upper Bound"),
        ),
        random[3],
        "random upper",
    );
    let anova = [
        625.5413781,
        3.0,
        208.5137927,
        13.64616706,
        6.665547978e-06,
        488.9608442,
        32.0,
        15.28002638,
    ];
    for (row, ss, df, ms) in [("Between Groups", 0, 1, 2), ("Within Groups", 5, 6, 7)] {
        near(
            value(&t, "ANOVA", &format!("y / {row}"), "Sum of Squares"),
            anova[ss],
            row,
        );
        near(
            value(&t, "ANOVA", &format!("y / {row}"), "df"),
            anova[df],
            row,
        );
        near(
            value(&t, "ANOVA", &format!("y / {row}"), "Mean Square"),
            anova[ms],
            row,
        );
    }
    near(value(&t, "ANOVA", "y / Between Groups", "F"), anova[3], "F");
    near(
        value(&t, "ANOVA", "y / Between Groups", "Sig."),
        anova[4],
        "F sig",
    );
    let levene = [
        3.002329389,
        0.04486710165,
        2.474811659,
        0.07932152775,
        2.474811659,
        19.13631024,
        0.09247417252,
        3.082839987,
        0.04117775202,
    ];
    let at = |row: &str, c: &str| {
        value(
            &t,
            "Test of Homogeneity of Variances",
            &format!("y / {row}"),
            c,
        )
    };
    near(at("Based on Mean", "Levene Statistic"), levene[0], "Levene");
    near(at("Based on Mean", "Sig."), levene[1], "Levene sig");
    near(
        at("Based on Median", "Levene Statistic"),
        levene[2],
        "median",
    );
    near(at("Based on Median", "Sig."), levene[3], "median sig");
    near(
        at("Based on Median and with adjusted df", "df2"),
        levene[5],
        "adjusted df",
    );
    near(
        at("Based on Median and with adjusted df", "Sig."),
        levene[6],
        "adjusted sig",
    );
    near(
        at("Based on trimmed mean", "Levene Statistic"),
        levene[7],
        "trimmed",
    );
    near(
        at("Based on trimmed mean", "Sig."),
        levene[8],
        "trimmed sig",
    );
    let robust = |row: &str, c: &str| {
        value(
            &t,
            "Robust Tests of Equality of Means",
            &format!("y / {row}"),
            c,
        )
    };
    let (welch, bf) = (
        [8.227433071, 3.0, 17.17259804, 0.001305668451],
        [15.59265104, 3.0, 19.8512388, 1.898707991e-05],
    );
    near(robust("Welch", "Statistic"), welch[0], "Welch");
    near(robust("Welch", "df2"), welch[2], "Welch df");
    near(robust("Welch", "Sig."), welch[3], "Welch sig");
    near(
        robust("Brown-Forsythe", "Statistic"),
        bf[0],
        "Brown-Forsythe",
    );
    near(robust("Brown-Forsythe", "df2"), bf[2], "Brown-Forsythe df");
    near(
        robust("Brown-Forsythe", "Sig."),
        bf[3],
        "Brown-Forsythe sig",
    );
}

#[test]
fn contrasts_and_the_polynomial_decomposition_agree_with_r() {
    let t = run(
        "ONEWAY y BY g /CONTRAST=-3 -1 1 3 /CONTRAST=1 0 0 -1 /CONTRAST=1 1 -1 -1 /POLYNOMIAL=3.",
    );
    // value, pooled se, t, df, sig, separate se, t, df, sig
    let contrasts: [[f64; 9]; 3] = [
        [
            30.65974026,
            5.874862351,
            5.218801468,
            32.0,
            1.051443516e-05,
            6.050770555,
            5.067080297,
            12.08867192,
            0.0002702962648,
        ],
        [
            -10.1,
            1.854186057,
            -5.447134046,
            32.0,
            5.408093152e-06,
            1.965895733,
            -5.137607164,
            10.93157101,
            0.0003309491063,
        ],
        [
            -10.45974026,
            2.647632934,
            -3.950600601,
            32.0,
            0.0004023001567,
            2.386180223,
            -4.383466161,
            20.57823862,
            0.0002707190328,
        ],
    ];
    for (i, want) in contrasts.iter().enumerate() {
        let (pooled, separate) = (
            format!("y / Assume equal variances / {}", i + 1),
            format!("y / Does not assume equal variances / {}", i + 1),
        );
        let at = |row: &str, c: &str| value(&t, "Contrast Tests", row, c);
        near(at(&pooled, "Value of Contrast"), want[0], &pooled);
        near(at(&pooled, "Std. Error"), want[1], &pooled);
        near(at(&pooled, "t"), want[2], &pooled);
        near(at(&pooled, "df"), want[3], &pooled);
        near(at(&pooled, "Sig. (2-tailed)"), want[4], &pooled);
        near(at(&separate, "Std. Error"), want[5], &separate);
        near(at(&separate, "t"), want[6], &separate);
        near(at(&separate, "df"), want[7], &separate);
        near(at(&separate, "Sig. (2-tailed)"), want[8], &separate);
    }
    let (weighted, unweighted, deviation) = (
        [481.2959467, 109.3042171, 34.94121426],
        [416.1650988, 123.6326909, 34.94121426],
        [144.2454313, 34.94121426],
    );
    let (weighted_f, weighted_p, unweighted_p) = (
        [31.49837145, 7.153404997, 2.286724734],
        [3.343764824e-06, 0.01168751832, 0.1402990687],
        [1.051443516e-05, 0.007692438415, 0.1402990687],
    );
    for (i, term) in ["Linear", "Quadratic", "Cubic"].iter().enumerate() {
        let w = format!("y / {term} Term: Weighted");
        near(value(&t, "ANOVA", &w, "Sum of Squares"), weighted[i], &w);
        near(value(&t, "ANOVA", &w, "F"), weighted_f[i], &w);
        near(value(&t, "ANOVA", &w, "Sig."), weighted_p[i], &w);
        let u = format!("y / {term} Term: Unweighted");
        near(value(&t, "ANOVA", &u, "Sum of Squares"), unweighted[i], &u);
        near(value(&t, "ANOVA", &u, "Sig."), unweighted_p[i], &u);
    }
    for (i, term) in ["Linear", "Quadratic"].iter().enumerate() {
        let row = format!("y / {term} Term: Deviation");
        near(
            value(&t, "ANOVA", &row, "Sum of Squares"),
            deviation[i],
            &row,
        );
    }
}

#[test]
fn tamhane_dunnett_t3_c_and_dunnett_t_agree_with_r() {
    let t = run("ONEWAY y BY g /POSTHOC=T2 T3 C.");
    // pair, then T2: difference... as printed by the script
    let pairs: [((u32, u32), [f64; 10]); 6] = [
        (
            (1, 2),
            [
                -1.104545455,
                1.234417634,
                0.945721453,
                -4.825576437,
                2.616485528,
                0.9297107923,
                -4.7878439,
                2.578752991,
                -4.959363864,
                2.750272955,
            ],
        ),
        (
            (1, 3),
            [
                -1.464285714,
                1.037322233,
                0.7059900496,
                -4.750048724,
                1.821477295,
                0.6544991897,
                -4.697739174,
                1.769167746,
                -4.998896348,
                2.070324919,
            ],
        ),
        (
            (1, 4),
            [
                -10.1,
                1.965895733,
                0.001984052453,
                -16.39158793,
                -3.808412074,
                0.001883175788,
                -16.28307096,
                -3.916929043,
                -16.27405395,
                -3.925946053,
            ],
        ),
        (
            (2, 3),
            [
                -0.3597402597,
                1.352445941,
                0.9999227811,
                -4.414580025,
                3.695099505,
                0.9998832788,
                -4.375762948,
                3.656282428,
                -4.702832256,
                3.983351736,
            ],
        ),
        (
            (2, 4),
            [
                -8.995454545,
                2.148911036,
                0.005127712516,
                -15.53379042,
                -2.457118667,
                0.004942465361,
                -15.46089795,
                -2.530011136,
                -15.67084176,
                -2.320067331,
            ],
        ),
        (
            (3, 4),
            [
                -8.635714286,
                2.04207472,
                0.006748382641,
                -15.02984291,
                -2.241585659,
                0.006401105193,
                -14.93723499,
                -2.334193582,
                -15.12562746,
                -2.14580111,
            ],
        ),
    ];
    for ((i, j), want) in pairs {
        for (sign, a, b) in [(1.0, i, j), (-1.0, j, i)] {
            let key = |test: &str| format!("{test} / {a} / {b}");
            let at = |test: &str, c: &str| value(&t, "Multiple Comparisons: y", &key(test), c);
            near(
                at("Tamhane", "Mean Difference (I-J)"),
                sign * want[0],
                "difference",
            );
            near(at("Tamhane", "Std. Error"), want[1], "se");
            near(at("Tamhane", "Sig."), want[2], "T2 sig");
            let (l, h) = (
                if sign > 0.0 { want[3] } else { -want[4] },
                if sign > 0.0 { want[4] } else { -want[3] },
            );
            near(
                at("Tamhane", "95% Confidence Interval / Lower Bound"),
                l,
                "T2 lower",
            );
            near(
                at("Tamhane", "95% Confidence Interval / Upper Bound"),
                h,
                "T2 upper",
            );
            near(at("Dunnett T3", "Sig."), want[5], "T3 sig");
            let (l, h) = (
                if sign > 0.0 { want[6] } else { -want[7] },
                if sign > 0.0 { want[7] } else { -want[6] },
            );
            near(
                at("Dunnett T3", "95% Confidence Interval / Lower Bound"),
                l,
                "T3 lower",
            );
            near(
                at("Dunnett T3", "95% Confidence Interval / Upper Bound"),
                h,
                "T3 upper",
            );
            // qtukey has a tolerance of 1e-4 in R, which leaves the C limits good to about 1e-7
            let (l, h) = (
                if sign > 0.0 { want[8] } else { -want[9] },
                if sign > 0.0 { want[9] } else { -want[8] },
            );
            close(
                at("Dunnett C", "95% Confidence Interval / Lower Bound"),
                l,
                1e-6,
                "C lower",
            );
            close(
                at("Dunnett C", "95% Confidence Interval / Upper Bound"),
                h,
                1e-6,
                "C upper",
            );
        }
    }
    // Dunnett t against the last group (two-sided, then one-sided below), and against group 2 (above)
    let t = run("ONEWAY y BY g /POSTHOC=DUNNETT DUNNETTL.");
    let two: [[f64; 6]; 3] = [
        [
            1.0,
            -10.1,
            1.854186057,
            1.594396801e-05,
            -14.6869539,
            -5.513046102,
        ],
        [
            2.0,
            -8.995454545,
            1.707950803,
            2.690935714e-05,
            -13.22064627,
            -4.770262817,
        ],
        [
            3.0,
            -8.635714286,
            1.926360182,
            0.0002582323486,
            -13.4012152,
            -3.870213376,
        ],
    ];
    let less: [[f64; 5]; 3] = [
        [1.0, -10.1, 1.854186057, 7.971984007e-06, -6.110125108],
        [
            2.0,
            -8.995454545,
            1.707950803,
            1.345467857e-05,
            -5.320251618,
        ],
        [3.0, -8.635714286, 1.926360182, 0.0001291161743, -4.49053366],
    ];
    for want in two {
        let row = format!("Dunnett t (2-sided) / {} / 4", want[0]);
        near(
            value(&t, "Multiple Comparisons: y", &row, "Mean Difference (I-J)"),
            want[1],
            &row,
        );
        near(
            value(&t, "Multiple Comparisons: y", &row, "Std. Error"),
            want[2],
            &row,
        );
        close(
            value(&t, "Multiple Comparisons: y", &row, "Sig."),
            want[3],
            1e-4,
            &row,
        );
        close(
            value(
                &t,
                "Multiple Comparisons: y",
                &row,
                "95% Confidence Interval / Lower Bound",
            ),
            want[4],
            1e-5,
            &row,
        );
        close(
            value(
                &t,
                "Multiple Comparisons: y",
                &row,
                "95% Confidence Interval / Upper Bound",
            ),
            want[5],
            1e-5,
            &row,
        );
    }
    for want in less {
        let row = format!("Dunnett t (<control) / {} / 4", want[0]);
        close(
            value(&t, "Multiple Comparisons: y", &row, "Sig."),
            want[3],
            1e-5,
            &row,
        );
        close(
            value(
                &t,
                "Multiple Comparisons: y",
                &row,
                "95% Confidence Interval / Upper Bound",
            ),
            want[4],
            1e-5,
            &row,
        );
        assert_eq!(
            cell(
                &t,
                "Multiple Comparisons: y",
                &row,
                "95% Confidence Interval / Lower Bound"
            ),
            &Cell::Empty
        );
    }
    let t = run("ONEWAY y BY g /POSTHOC=DUNNETTR(2).");
    let greater: [[f64; 5]; 3] = [
        [1.0, -1.104545455, 1.816341544, 0.9256824952, -5.025538408],
        [3.0, 0.3597402597, 1.889961433, 0.6969118414, -3.720178237],
        [4.0, 8.995454545, 1.707950803, 1.350679017e-05, 5.308448054],
    ];
    for want in greater {
        let row = format!("Dunnett t (>control) / {} / 2", want[0]);
        close(
            value(&t, "Multiple Comparisons: y", &row, "Sig."),
            want[3],
            1e-5,
            &row,
        );
        close(
            value(
                &t,
                "Multiple Comparisons: y",
                &row,
                "95% Confidence Interval / Lower Bound",
            ),
            want[4],
            1e-5,
            &row,
        );
    }
}

#[test]
fn homogeneous_subsets_of_every_range_test_agree_with_r() {
    // the groups of each subset as the script found them, in ascending order of the means
    let expected: [(&str, &str, &[&str]); 6] = [
        ("SNK", "Student-Newman-Keuls", &["1234", "5"]),
        ("TUKEY", "Tukey HSD", &["1234", "345"]),
        ("BTUKEY", "Tukey B", &["1234", "35"]),
        ("DUNCAN", "Duncan", &["12", "234", "5"]),
        ("QREGW", "Ryan-Einot-Gabriel-Welsch Range", &["1234", "345"]),
        (
            "FREGW",
            "Ryan-Einot-Gabriel-Welsch F",
            &["124", "234", "345"],
        ),
    ];
    for (keyword, title, subsets) in expected {
        let t = run_on(DATA5, &format!("ONEWAY y BY g /POSTHOC={keyword}."));
        let table = t
            .iter()
            .find(|t| t.title == "Homogeneous Subsets: y")
            .expect("subsets");
        // a group belongs to the subsets whose column holds its mean
        let keys = row_keys(table);
        for group in 1..=5u32 {
            let row = keys
                .iter()
                .position(|k| *k == format!("{title} / {group}"))
                .expect("a group row");
            let member: Vec<usize> = (0..subsets.len())
                .filter(|s| matches!(table.rows[row][3 + s], Cell::Number(_)))
                .collect();
            let want: Vec<usize> = (0..subsets.len())
                .filter(|s| subsets[*s].contains(&group.to_string()))
                .collect();
            assert_eq!(member, want, "{keyword}: group {group}");
        }
        assert_eq!(
            table.column_titles().len(),
            3 + subsets.len(),
            "{keyword}: number of subsets"
        );
    }
}
