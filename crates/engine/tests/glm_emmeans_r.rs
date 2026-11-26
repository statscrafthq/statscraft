//! UNIANOVA EMMEANS against R on 66 invented cases (a, three levels, by b, two levels, unbalanced,
//! and two covariates): `lm`'s coefficients and covariance, the estimated marginal means written
//! as averages of the cell predictions with the covariates held at their means or at the WITH
//! values, and the pairwise comparisons with LSD, Bonferroni and Sidak adjustments (adjusted
//! significance and the confidence interval at the adjusted level), the univariate tests of
//! COMPARE, simple effects with COMPARE(factor), and a model whose slopes differ by a, where the
//! covariate value moves the means and the differences. The script is lab/oracles/glm-emmeans-r.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y a b x x2.
BEGIN DATA.
9.3 2 1 9 0.9
12.14 1 2 12.5 4.7
12.42 2 1 11.7 2.9
11.72 2 1 11.1 0.1
16.2 3 2 12.7 0.3
18.45 3 2 10.2 0.9
10.44 1 2 14.4 4.7
14.21 3 1 16.3 4.5
13.91 2 1 12.1 5.2
15.97 3 2 11.4 4.4
10.78 2 2 11.2 2.5
9.48 2 1 9.8 4
9.32 2 2 8 0
5.98 1 2 7.3 3.4
9.11 1 1 8.2 3.7
10.02 1 2 10.8 4.5
9.31 1 1 10.5 0.8
12.82 2 2 11.8 5.3
10.5 2 2 10.1 2.3
13.09 3 2 10.2 1.4
6.62 1 1 8.5 2.1
7 2 2 8.5 5.9
13.98 3 2 6 5.8
13.13 3 1 8 2.1
10.97 3 1 8.2 3.6
9.98 2 2 8.9 5.3
8.84 2 1 6.2 1.6
9.55 3 2 5.4 3.5
13.14 2 1 16.4 3.8
9.24 1 2 9.6 2.6
13.84 1 1 14.6 1.5
11.26 3 1 11.1 2.8
13.56 2 2 12.5 1
11.58 1 1 10 4.9
12.87 3 2 7.2 2.1
10.14 3 1 7.7 0.1
6.72 2 1 9.4 5.3
10.59 2 2 7.4 1.9
16.01 3 2 8.1 0.8
14.87 3 1 11.2 1
7.37 2 1 7.2 3.4
11.12 2 1 10.3 2
11.16 1 2 8.6 1
10.39 2 1 12.7 4.9
8.71 1 2 7.8 3.4
8.41 1 2 6.4 2.6
10.34 1 2 10.1 3.2
11.46 2 1 12.4 3.1
11.23 3 1 12.6 5.1
6.88 2 1 8.8 4.6
8.72 1 1 10.2 4.2
9.76 1 2 10.3 4.5
9.31 1 1 10.6 0.4
14.93 3 2 13.4 3.2
6.07 1 1 11.6 5.5
8.12 1 1 12.1 5.5
12.54 1 2 9.9 1.1
13.56 2 2 9.2 0.6
9.31 1 2 11.2 1.4
7.41 2 1 8.7 4.8
8.03 1 2 9.2 3.4
9.44 2 1 14 5
8.95 2 2 11.8 5.2
10.25 1 2 13.1 2.2
6.29 1 1 5.5 2.5
8.05 1 2 5.9 3.1
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
                if let Cell::Text(t) = cell {
                    carried[level] = t.clone();
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

/// The `nth` table with this title (EMMEANS repeats titles) and a number of one of its rows.
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
        .unwrap_or_else(|| panic!("{title}: no column {column}"));
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

const LOWER: &str = "95% Confidence Interval / Lower Bound";
const UPPER: &str = "95% Confidence Interval / Upper Bound";
const DIFF_LOWER: &str = "95% Confidence Interval for Difference / Lower Bound";
const DIFF_UPPER: &str = "95% Confidence Interval for Difference / Upper Bound";

/// Means, standard errors and limits of the cells named by `rows`.
fn means(
    t: &[Table],
    title: &str,
    nth: usize,
    rows: &[&str],
    est: &[f64],
    se: &[f64],
    limits: &[f64],
) {
    for (i, row) in rows.iter().enumerate() {
        near(
            value(t, title, nth, row, "Mean"),
            est[i],
            &format!("{title} {row} mean"),
        );
        near(
            value(t, title, nth, row, "Std. Error"),
            se[i],
            &format!("{title} {row} se"),
        );
        near(
            value(t, title, nth, row, LOWER),
            limits[i],
            &format!("{title} {row} lower"),
        );
        near(
            value(t, title, nth, row, UPPER),
            limits[rows.len() + i],
            &format!("{title} {row} upper"),
        );
    }
}

/// One comparison table: difference, standard error, significance and limits of (I) against (J).
fn pair(t: &[Table], nth: usize, prefix: &str, i: usize, j: usize, want: [f64; 5], what: &str) {
    let key = |a: usize, b: usize| format!("{prefix}{a} / {b}");
    for (a, b, sign) in [(i, j, 1.0), (j, i, -1.0)] {
        let row = key(a, b);
        let at = |c: &str| value(t, "Pairwise Comparisons", nth, &row, c);
        near(
            at("Mean Difference (I-J)"),
            sign * want[0],
            &format!("{what} {row} difference"),
        );
        near(at("Std. Error"), want[1], &format!("{what} {row} se"));
        near(at("Sig."), want[2], &format!("{what} {row} sig"));
        let (lo, hi) = if sign > 0.0 {
            (want[3], want[4])
        } else {
            (-want[4], -want[3])
        };
        near(at(DIFF_LOWER), lo, &format!("{what} {row} lower"));
        near(at(DIFF_UPPER), hi, &format!("{what} {row} upper"));
    }
}

fn contrast(t: &[Table], nth: usize, row: &str, want: [f64; 5], what: &str) {
    let at = |c: &str| value(t, "Univariate Tests", nth, row, c);
    near(at("Sum of Squares"), want[0], &format!("{what} SS"));
    assert_eq!(at("df"), want[1]);
    near(at("Mean Square"), want[2], &format!("{what} MS"));
    near(at("F"), want[3], &format!("{what} F"));
    near(at("Sig."), want[4], &format!("{what} sig"));
}

type Pairs = [[f64; 5]; 3];

/// The table of the levels of `a` (three), its pairwise comparisons and, when given, the
/// univariate test.
fn check_a(
    syntax: &str,
    est: [f64; 3],
    se: [f64; 3],
    limits: [f64; 6],
    pairs: Pairs,
    ftest: Option<[f64; 5]>,
) {
    let t = run(syntax);
    means(
        &t,
        "Estimated Marginal Means: a",
        0,
        &["1", "2", "3"],
        &est,
        &se,
        &limits,
    );
    for ((i, j), want) in [(1, 2), (1, 3), (2, 3)].into_iter().zip(pairs) {
        pair(&t, 0, "", i, j, want, syntax);
    }
    if let Some(f) = ftest {
        contrast(&t, 0, "Contrast", f, syntax);
    }
}

/// `a` compared inside each level of `b`: the pairs and the univariate tests of both levels.
fn check_a_within_b(syntax: &str, pairs: [Pairs; 2], tests: [[f64; 5]; 2]) {
    let t = run(syntax);
    for b in 1..=2usize {
        for ((i, j), want) in [(1, 2), (1, 3), (2, 3)].into_iter().zip(pairs[b - 1]) {
            pair(&t, 0, &format!("{b} / "), i, j, want, syntax);
        }
        contrast(&t, 0, &format!("b = {b}: Contrast"), tests[b - 1], syntax);
    }
}

#[test]
fn marginal_means_and_pairwise_comparisons_with_every_adjustment() {
    let (est, se, limits, f) = (
        [9.350318057, 10.32502637, 13.25811006],
        [0.3281405243, 0.3288418304, 0.407746338],
        [
            8.693473432,
            9.666777929,
            12.44191709,
            10.00716268,
            10.98327481,
            14.07430304,
        ],
        [147.9438616, 2.0, 73.97193081, 28.66711072, 2.201021847e-09],
    );
    let lsd: Pairs = [
        [
            -0.9747083134,
            0.4645476778,
            0.0402547048,
            -1.904601522,
            -0.04481510461,
        ],
        [
            -3.907792006,
            0.5240067886,
            4.989293354e-10,
            -4.956705556,
            -2.858878455,
        ],
        [
            -2.933083692,
            0.5258385492,
            6.688760547e-07,
            -3.98566391,
            -1.880503474,
        ],
    ];
    let bonferroni: Pairs = [
        [
            -0.9747083134,
            0.4645476778,
            0.1207641144,
            -2.120003359,
            0.1705867319,
        ],
        [
            -3.907792006,
            0.5240067886,
            1.496788006e-09,
            -5.199677441,
            -2.615906571,
        ],
        [
            -2.933083692,
            0.5258385492,
            2.006628164e-06,
            -4.229485146,
            -1.636682238,
        ],
    ];
    let sidak: Pairs = [
        [
            -0.9747083134,
            0.4645476778,
            0.115968021,
            -2.116866748,
            0.1674501207,
        ],
        [
            -3.907792006,
            0.5240067886,
            1.496788027e-09,
            -5.196139363,
            -2.619444648,
        ],
        [
            -2.933083692,
            0.5258385492,
            2.006626822e-06,
            -4.225934701,
            -1.640232683,
        ],
    ];
    let base = "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a)";
    check_a(&format!("{base} COMPARE."), est, se, limits, lsd, Some(f));
    check_a(
        &format!("{base} COMPARE ADJ(LSD)."),
        est,
        se,
        limits,
        lsd,
        Some(f),
    );
    check_a(
        &format!("{base} COMPARE ADJ(BONFERRONI)."),
        est,
        se,
        limits,
        bonferroni,
        Some(f),
    );
    check_a(
        &format!("{base} COMPARE ADJ(SIDAK)."),
        est,
        se,
        limits,
        sidak,
        Some(f),
    );
    let t = run("UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(OVERALL).");
    let grand = t
        .iter()
        .find(|t| t.title == "Estimated Marginal Means: Grand Mean")
        .expect("grand mean");
    for (c, w) in [10.97781816, 0.2052956807].iter().enumerate() {
        match &grand.rows[0][c] {
            Cell::Number(n) => near(*n, *w, "grand mean"),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn simple_effects_compare_one_factor_inside_the_levels_of_the_other() {
    // the six cells in R's order: a within b
    let (est, se) = (
        [
            8.897584179,
            9.861586577,
            11.80303111,
            9.803051934,
            10.78846616,
            14.71318902,
        ],
        [
            0.5080712067,
            0.4191600802,
            0.6106473576,
            0.4155876337,
            0.5081842305,
            0.5415285771,
        ],
    );
    let t = run("UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b).");
    for a in 1..=3usize {
        for b in 1..=2usize {
            let (k, row) = ((a - 1) + 3 * (b - 1), format!("{a} / {b}"));
            near(
                value(&t, "Estimated Marginal Means: a * b", 0, &row, "Mean"),
                est[k],
                &row,
            );
            near(
                value(&t, "Estimated Marginal Means: a * b", 0, &row, "Std. Error"),
                se[k],
                &row,
            );
        }
    }
    let f = [
        [34.90211704, 2.0, 17.45105852, 6.762989979, 0.002290662625],
        [139.1000648, 2.0, 69.55003238, 26.95344649, 5.279144034e-09],
    ];
    check_a_within_b(
        "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b) COMPARE(a) ADJ(LSD).",
        [
            [
                [
                    -0.9640023978,
                    0.6577780609,
                    0.1481731325,
                    -2.280688243,
                    0.3526834474,
                ],
                [
                    -2.905446929,
                    0.7945113794,
                    0.0005517581764,
                    -4.495834249,
                    -1.31505961,
                ],
                [
                    -1.941444531,
                    0.7401290644,
                    0.01111420652,
                    -3.42297382,
                    -0.4599152427,
                ],
            ],
            [
                [
                    -0.9854142289,
                    0.6559313828,
                    0.1384416493,
                    -2.298403546,
                    0.3275750885,
                ],
                [
                    -4.910137082,
                    0.680967706,
                    1.298067848e-09,
                    -6.273242045,
                    -3.547032119,
                ],
                [
                    -3.924722853,
                    0.7413698682,
                    1.919839098e-06,
                    -5.40873588,
                    -2.440709826,
                ],
            ],
        ],
        f,
    );
    check_a_within_b(
        "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b) COMPARE(a) ADJ(BONFERRONI).",
        [
            [
                [
                    -0.9640023978,
                    0.6577780609,
                    0.4445193975,
                    -2.585687297,
                    0.6576825016,
                ],
                [
                    -2.905446929,
                    0.7945113794,
                    0.001655274529,
                    -4.864233917,
                    -0.9466599418,
                ],
                [
                    -1.941444531,
                    0.7401290644,
                    0.03334261955,
                    -3.766157453,
                    -0.1167316097,
                ],
            ],
            [
                [
                    -0.9854142289,
                    0.6559313828,
                    0.4153249478,
                    -2.602546331,
                    0.6317178735,
                ],
                [
                    -4.910137082,
                    0.680967706,
                    3.894203545e-09,
                    -6.588993692,
                    -3.231280471,
                ],
                [
                    -3.924722853,
                    0.7413698682,
                    5.759517294e-06,
                    -5.75249485,
                    -2.096950856,
                ],
            ],
        ],
        f,
    );
    check_a_within_b(
        "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b) COMPARE(a) ADJ(SIDAK).",
        [
            [
                [
                    -0.9640023978,
                    0.6577780609,
                    0.3819067481,
                    -2.581246001,
                    0.653241205,
                ],
                [
                    -2.905446929,
                    0.7945113794,
                    0.001654361386,
                    -4.858869401,
                    -0.9520244574,
                ],
                [
                    -1.941444531,
                    0.7401290644,
                    0.03297341568,
                    -3.761160125,
                    -0.1217289377,
                ],
            ],
            [
                [
                    -0.9854142289,
                    0.6559313828,
                    0.3604800622,
                    -2.598117504,
                    0.6272890457,
                ],
                [
                    -4.910137082,
                    0.680967706,
                    3.894203626e-09,
                    -6.58439582,
                    -3.235878344,
                ],
                [
                    -3.924722853,
                    0.7413698682,
                    5.759506236e-06,
                    -5.747489144,
                    -2.101956562,
                ],
            ],
        ],
        f,
    );
    // b inside the levels of a: two cells each, one pair
    let t = run("UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b) COMPARE(b) ADJ(BONFERRONI).");
    let (pairs, f) = (
        [
            [
                -0.9054677552,
                0.6565013709,
                0.173118525,
                -2.219598028,
                0.4086625172,
            ],
            [
                -0.9268795864,
                0.6598067689,
                0.1654215988,
                -2.247626332,
                0.3938671592,
            ],
            [
                -2.910157908,
                0.8168589141,
                0.0007420259935,
                -4.545278678,
                -1.275037137,
            ],
        ],
        [
            [4.908601037, 1.0, 4.908601037, 1.902281147, 0.173118525],
            [5.09209072, 1.0, 5.09209072, 1.97339081, 0.1654215988],
            [32.75080922, 1.0, 32.75080922, 12.69226129, 0.0007420259935],
        ],
    );
    for a in 1..=3usize {
        pair(
            &t,
            0,
            &format!("{a} / "),
            1,
            2,
            pairs[a - 1],
            &format!("b at a = {a}"),
        );
        contrast(
            &t,
            0,
            &format!("a = {a}: Contrast"),
            f[a - 1],
            &format!("b at a = {a}"),
        );
    }
}

#[test]
fn with_values_move_the_covariates_the_means_are_held_at() {
    // additive covariates: the means move with the covariates and the differences stay
    check_a(
        "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a) WITH(x=8 x2=5) COMPARE ADJ(BONFERRONI).",
        [7.349712312, 8.324420626, 11.25750432],
        [0.4457969275, 0.4484313534, 0.537557448],
        [
            6.457352808,
            7.426787745,
            10.18146618,
            8.242071816,
            9.222053506,
            12.33354246,
        ],
        [
            [
                -0.9747083134,
                0.4645476778,
                0.1207641144,
                -2.120003359,
                0.1705867319,
            ],
            [
                -3.907792006,
                0.5240067886,
                1.496788006e-09,
                -5.199677441,
                -2.615906571,
            ],
            [
                -2.933083692,
                0.5258385492,
                2.006628164e-06,
                -4.229485146,
                -1.636682238,
            ],
        ],
        None,
    );
    check_a(
        "UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a) WITH(x=12) COMPARE ADJ(BONFERRONI).",
        [10.36737734, 11.34208565, 14.27516934],
        [0.3678124447, 0.3588277645, 0.4351944483],
        [
            9.631120738,
            10.62381384,
            13.40403301,
            11.10363394,
            12.06035746,
            15.14630568,
        ],
        [
            [
                -0.9747083134,
                0.4645476778,
                0.1207641144,
                -2.120003359,
                0.1705867319,
            ],
            [
                -3.907792006,
                0.5240067886,
                1.496788006e-09,
                -5.199677441,
                -2.615906571,
            ],
            [
                -2.933083692,
                0.5258385492,
                2.006628164e-06,
                -4.229485146,
                -1.636682238,
            ],
        ],
        None,
    );
    // the cells at x = 8 and x2 = 5
    let t = run("UNIANOVA y BY a b WITH x x2 /EMMEANS=TABLES(a*b) WITH(x=8 x2=5).");
    let est = [
        6.896978435,
        7.860980832,
        9.802425364,
        7.80244619,
        8.787860419,
        12.71258327,
    ];
    for a in 1..=3usize {
        for b in 1..=2usize {
            let row = format!("{a} / {b}");
            near(
                value(&t, "Estimated Marginal Means: a * b", 0, &row, "Mean"),
                est[(a - 1) + 3 * (b - 1)],
                &row,
            );
        }
    }
}

#[test]
fn slopes_that_differ_by_a_make_the_means_and_differences_depend_on_the_value_held() {
    check_a(
        "UNIANOVA y BY a b WITH x x2 /DESIGN=x x2 a b a*b x*a /EMMEANS=TABLES(a) COMPARE ADJ(SIDAK).",
        [9.348733645, 10.30663079, 13.24919601],
        [0.3311339231, 0.3321262039, 0.4112134596],
        [
            8.685392687,
            9.641302054,
            12.42543646,
            10.0120746,
            10.97195953,
            14.07295555,
        ],
        [
            [
                -0.9578971448,
                0.4686923109,
                0.1309057727,
                -2.111467058,
                0.1956727684,
            ],
            [
                -3.900462363,
                0.5286715429,
                2.464741078e-09,
                -5.201656284,
                -2.599268441,
            ],
            [
                -2.942565218,
                0.5303834772,
                2.44827719e-06,
                -4.247972641,
                -1.637157795,
            ],
        ],
        Some([147.4573574, 2.0, 73.72867869, 28.10834186, 3.529041079e-09]),
    );
    check_a(
        "UNIANOVA y BY a b WITH x x2 /DESIGN=x x2 a b a*b x*a /EMMEANS=TABLES(a) WITH(x=8) COMPARE ADJ(SIDAK).",
        [8.238490934, 8.92322877, 12.30623795],
        [0.4317306473, 0.466882863, 0.5103409452],
        [
            7.373630521,
            7.987950008,
            11.28390219,
            9.103351346,
            9.858507532,
            13.32857371,
        ],
        [
            [
                -0.6847378359,
                0.6352277443,
                0.6355186764,
                -2.248193427,
                0.878717755,
            ],
            [
                -4.067747018,
                0.6668458968,
                3.134275198e-07,
                -5.709022852,
                -2.426471185,
            ],
            [
                -3.383009182,
                0.6894441634,
                2.506033389e-05,
                -5.079905046,
                -1.686113319,
            ],
        ],
        Some([106.0003867, 2.0, 53.00019334, 20.20580837, 2.475383647e-07]),
    );
    check_a(
        "UNIANOVA y BY a b WITH x x2 /DESIGN=x x2 a b a*b x*a /EMMEANS=TABLES(a) WITH(x=13 x2=5) COMPARE ADJ(SIDAK).",
        [9.985558148, 11.31509264, 13.65842722],
        [0.5680663778, 0.5332439326, 0.6495685819],
        [
            8.847584449,
            10.24687668,
            12.35718499,
            11.12353185,
            12.3833086,
            14.95966946,
        ],
        [
            [
                -1.329534488,
                0.7378746456,
                0.2135512086,
                -3.145629975,
                0.4865609999,
            ],
            [
                -3.672869076,
                0.7988239269,
                7.438628248e-05,
                -5.638976107,
                -1.706762044,
            ],
            [
                -2.343334588,
                0.7874604327,
                0.01286880147,
                -4.281473196,
                -0.4051959799,
            ],
        ],
        Some([55.97289755, 2.0, 27.98644877, 10.66956147, 0.0001186003473]),
    );
}
