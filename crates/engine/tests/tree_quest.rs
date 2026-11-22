//! TREE with METHOD TYPE=QUEST on an invented sample of 150 cases: y (1, 2) and y3 (1 to 3) by
//! x1 (scale), x2 (scale, ties), o (ordinal 1 to 4), g (nominal 1 to 4), xm (x1 with 14
//! system-missing values), z (scale with 6 missing), r (scale), b (nominal, two categories that
//! follow y), v (scale whose spread, not its mean, follows y) and zv (scale, constant in class 2 of
//! y), g4 (g with category 4 split in two: five categories) and gm (g with 6 missing values). The expected trees come from an independent grower written in R from the manual's rules
//! (F by lm and anova, Pearson chi-square by chisq.test, Levene as the F on the absolute
//! deviations, 2-means by kmeans, the quadratic discriminant roots by polyroot, then the
//! surrogates and pruning of the CRT grower's checks).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE / y y3 x1 x2 o g xm z r b v zv g4 gm.
BEGIN DATA
1 1 29.4 8 1 2 29.4 6 22.01 1 29.9 4.1 2 2
1 1 10.5 8 4 4 10.5 2 15.01 1 26.4 5.4 5 4
2 2 42.5 12 2 4 42.5 8 26.49 1 24.7 8 4 .
1 1 11.1 7 1 2 11.1 2 16.58 1 30.7 0.5 2 2
2 2 45.8 8 3 2 45.8 9 25.33 2 35.2 8 2 2
2 3 52 10 3 4 52 10 26.69 1 32.8 8 5 4
2 2 43.3 5 2 3 43.3 9 21.66 1 30.2 8 3 3
1 2 39.9 4 1 4 39.9 . 22.32 1 27.1 1.2 5 4
2 2 48.3 5 1 3 48.3 10 22.71 2 31.9 8 3 3
2 3 58.1 5 2 1 58.1 12 25.33 1 11.7 8 1 1
2 3 53.2 4 2 2 . 11 23.82 2 36 8 2 2
1 1 14.7 12 2 1 14.7 3 8.8 1 29.4 7.2 1 1
1 1 13.6 7 4 1 13.6 3 16.28 1 30.6 8.1 1 1
1 1 24.9 9 1 2 24.9 5 18.72 2 31 9.4 2 2
1 1 35.4 11 1 1 35.4 7 15.05 1 28.8 2.7 1 1
1 3 45.2 9 2 4 45.2 9 24.04 1 26.1 9.1 5 4
1 2 51.6 3 1 1 51.6 10 20.02 1 30.9 5 1 .
2 3 53.8 8 3 4 . 11 31.17 2 48.9 8 5 4
1 1 17.7 8 4 3 17.7 4 17.53 1 30.4 6.3 3 3
1 1 22.6 7 2 2 22.6 5 16.84 1 26.8 7.2 2 2
2 2 41.8 10 2 2 41.8 8 22.22 2 15.7 8 2 2
2 3 48.7 3 2 2 . 10 24.67 2 33.1 8 2 2
2 2 53 9 4 1 53 11 25.39 2 25.6 8 1 1
1 1 26 3 2 1 26 5 17.75 1 29.3 2.3 1 1
2 3 55.6 6 1 2 55.6 11 28.79 2 44.4 8 2 2
1 1 15.9 8 1 2 15.9 3 14.75 1 31.8 8.6 2 2
1 2 51.9 2 3 3 51.9 10 27.5 1 32.1 8.1 3 3
1 1 11.2 12 1 1 11.2 2 13.56 1 31.5 6.2 1 1
2 3 56.2 3 2 3 56.2 11 24.41 2 24.3 8 3 3
2 1 15.2 3 4 1 15.2 3 15.22 2 27.9 8 1 1
1 3 55.9 2 4 2 55.9 11 25.68 1 26.6 0.4 2 2
1 3 57.3 12 1 3 57.3 11 27.89 1 32.3 0.5 3 3
1 2 56.9 2 1 3 56.9 11 24.84 1 35 6.7 3 3
2 3 46.9 4 1 4 46.9 9 25.86 2 49 8 5 4
1 1 41 11 2 3 41 8 22.77 1 31.2 9.7 3 3
1 1 23.3 9 3 3 23.3 5 20.34 1 29.8 5.4 3 3
2 2 40.6 7 4 3 40.6 8 24.94 2 15.6 8 3 3
1 1 49.8 6 1 1 49.8 10 20.93 1 28.3 1.5 1 1
2 1 10.5 3 1 1 10.5 2 11.16 2 16.9 8 1 1
1 1 37.9 3 2 1 37.9 8 19.88 2 30.5 5.2 1 .
2 3 45.9 12 1 4 45.9 9 27.1 2 28.7 8 4 .
1 1 26.7 2 2 2 26.7 5 16.7 1 30.9 7.3 2 2
2 2 50.3 10 2 4 50.3 10 29.05 1 14.5 8 4 4
1 1 16.6 3 3 4 16.6 3 15.09 1 30.4 8.8 5 4
1 1 15.8 6 1 3 15.8 3 13.16 1 30 3.5 3 3
1 2 26.8 9 3 2 26.8 5 18.88 2 27.4 3 2 2
1 3 33 10 4 3 33 7 17.65 1 26 7.7 3 3
1 1 17.5 2 1 2 . 4 13.22 2 32.8 8 2 2
2 2 47.8 11 1 4 47.8 10 28.49 2 10.2 8 4 4
1 2 49.2 2 3 3 49.2 10 25.69 1 28.1 6.2 3 3
1 1 18.7 9 2 4 18.7 4 17.31 1 29.7 4.9 4 4
2 2 35 12 4 3 35 7 18.73 2 32.8 8 3 3
1 1 27.6 8 1 4 27.6 6 20.25 1 28.8 7.4 4 4
1 1 35.9 1 4 1 . 7 22.64 2 26.5 5.4 1 1
2 2 38.9 8 1 4 38.9 8 25.58 1 14 8 4 4
2 2 54 5 2 3 54 11 30.28 2 27.6 8 3 3
2 2 37.3 8 4 2 37.3 7 22.07 2 29.8 8 2 2
1 2 46.1 3 4 4 46.1 9 28.94 2 32.4 8.6 5 4
1 1 16.8 11 4 2 16.8 3 16.57 1 25.9 2.8 2 2
1 1 47.4 4 3 1 47.4 9 25.93 1 31.6 5.4 1 1
2 3 51.2 12 3 2 51.2 10 26.06 2 49.6 8 2 2
1 2 47.7 3 4 4 47.7 10 25.69 1 28.2 8.1 5 4
1 1 49.5 12 2 1 . 10 21.1 1 28.3 0.9 1 1
2 3 44.4 12 3 3 . 9 20.7 2 33.7 8 3 3
1 3 54 11 3 1 54 11 23.13 1 26.3 2.5 1 1
1 1 25 1 4 1 25 5 19.14 1 31 8.7 1 1
2 1 20.9 12 2 4 20.9 4 17.15 2 31.4 8 4 4
1 1 23.9 11 3 1 23.9 5 15.95 2 35.8 2.2 1 1
2 3 49.9 9 1 2 49.9 10 24.51 1 18.6 8 2 2
1 3 53.1 5 4 4 53.1 11 29.16 1 32.7 2.2 5 4
2 2 18 12 3 3 18 4 19.81 2 28.9 8 3 3
2 3 59.3 12 4 4 59.3 12 29.61 2 18.5 8 5 4
1 2 45.8 5 3 3 45.8 9 26.07 1 30.9 4.5 3 3
1 3 50.9 3 3 4 50.9 10 30.27 1 30.8 4.7 5 4
2 2 44.9 9 4 3 . 9 22.25 2 55.7 8 3 3
1 1 29.7 6 2 1 29.7 6 14.91 1 31.8 6.8 1 1
2 2 39.3 7 4 3 39.3 8 25.75 2 25.5 8 3 3
1 1 49.4 4 1 1 49.4 10 20.08 1 26.6 4.8 1 1
1 1 12.1 9 2 3 12.1 2 15.92 1 30.8 8.3 3 3
1 1 47.8 7 2 3 . 10 21.03 1 30.9 9.6 3 3
1 2 48.5 3 2 2 48.5 10 24.25 1 33.7 5.6 2 2
1 1 45 3 3 1 45 9 24.7 1 26.4 7.5 1 1
1 3 32.9 10 3 2 32.9 7 21.62 1 29.7 8.3 2 2
1 2 45.9 1 2 3 45.9 9 22.59 1 27.5 7.5 3 3
1 1 17.8 10 1 3 17.8 4 14.18 2 28.5 6.7 3 3
1 1 24.9 3 1 4 24.9 5 20.19 2 31.9 6.9 5 4
2 2 56.1 11 1 3 56.1 11 24.28 2 35.2 8 3 3
1 1 22.1 11 1 3 22.1 4 12.48 1 33.5 3.3 3 3
2 3 11.8 11 3 2 11.8 2 16.3 2 47.7 8 2 2
2 1 14.8 5 4 1 14.8 . 11.03 1 36.9 8 1 .
1 1 44.1 10 4 1 44.1 9 18.92 1 32.8 3.5 1 1
1 1 12.3 9 1 3 12.3 2 15.3 1 24.9 6.1 3 3
1 1 21.5 7 3 3 21.5 4 13.95 1 28.3 6.1 3 3
2 3 47.8 8 3 4 47.8 10 29.51 2 27.4 8 5 4
2 1 28.1 3 2 1 28.1 6 15.06 2 27.8 8 1 1
1 2 37.6 7 3 3 37.6 8 18.25 1 29.4 6.7 3 3
1 1 31 11 1 1 31 6 15.83 1 33.7 9.7 1 1
1 2 21.2 7 4 2 . 4 18.47 2 30.9 1 2 2
2 2 59.6 7 1 1 59.6 12 24.84 2 35.3 8 1 1
1 1 26.7 2 2 3 26.7 5 17.54 2 30.8 3.3 3 3
2 1 24.6 3 2 2 24.6 5 17.1 2 41.4 8 2 2
1 3 42.9 11 3 4 42.9 9 22.71 2 30.4 7.3 5 4
1 1 23.5 10 2 4 23.5 5 19.04 2 29.3 4 4 4
2 1 12 10 4 3 . 2 14.68 1 37.2 8 3 3
1 1 19.1 10 3 1 19.1 4 10.97 1 30 9.9 1 1
2 1 41.2 9 2 1 41.2 8 20.16 2 19.5 8 1 1
1 2 37.4 10 4 2 37.4 7 25.75 2 28.3 4.9 2 2
1 3 45 8 4 3 45 9 28.25 2 33.1 8.2 3 3
1 1 31.1 12 1 3 31.1 6 17.93 1 30.1 7.9 3 3
2 1 26.7 3 1 3 26.7 5 19.92 1 31.8 8 3 3
1 2 54.5 4 4 1 54.5 11 25.01 1 28.6 1.6 1 1
1 2 13.8 7 4 2 13.8 3 18.84 1 31 7.9 2 2
1 1 50.4 10 2 1 50.4 10 23.71 1 31.4 5.7 1 1
2 1 52.5 11 1 2 52.5 10 28.65 2 17.8 8 2 2
2 3 35.6 9 4 4 35.6 7 25.88 2 23.9 8 4 4
1 1 12.9 3 3 1 12.9 3 11.12 1 30.9 7.4 1 1
2 2 41.9 8 1 4 41.9 8 24.64 2 28.6 8 4 4
1 2 16.4 7 1 4 16.4 3 17.76 1 26.3 8.9 5 4
2 1 46.4 8 3 1 46.4 9 21.38 1 37.5 8 1 1
1 1 19 4 4 3 19 4 14.76 1 30.3 6.2 3 .
2 1 18.8 9 1 2 . 4 17.05 2 19.3 8 2 2
1 2 52.3 12 1 1 52.3 10 21.86 1 27.9 2.7 1 1
1 1 15.1 2 1 3 15.1 3 11.86 1 29.6 1.3 3 3
2 2 51.9 8 3 3 51.9 10 31.65 2 33.5 8 3 3
1 1 29.3 5 1 1 29.3 6 13.11 2 27.7 9 1 1
1 1 18.3 3 2 3 18.3 4 18.2 2 30.7 3.8 3 3
2 1 24.8 4 4 2 24.8 5 22.64 2 37.1 8 2 2
1 1 33.7 9 4 1 33.7 7 19.76 1 31.7 8.6 1 1
2 3 40.4 6 2 2 40.4 8 20.86 1 39.8 8 2 2
2 2 51.2 9 2 4 51.2 10 28.8 2 15.1 8 5 4
2 3 54.2 12 3 4 54.2 11 29.98 2 15.5 8 4 4
1 1 13.2 8 1 3 13.2 3 13.93 1 28.8 8.1 3 3
2 3 39.3 10 4 4 . 8 24.97 2 48.8 8 4 4
1 1 30.1 3 3 2 30.1 6 21.06 1 24.2 7.7 2 2
1 2 59.7 6 1 3 59.7 12 25.92 1 30.9 4.9 3 3
1 1 29.2 5 3 4 29.2 6 17.23 2 26.2 9.6 5 4
2 1 42 5 3 1 42 8 22.9 2 46.8 8 1 1
1 3 55 7 3 3 55 11 25.79 1 27.8 8 3 3
2 2 25.6 4 2 4 25.6 5 20.16 1 26.3 8 4 4
1 2 39.7 6 1 4 39.7 . 22.66 2 31.4 3 5 4
2 3 57.4 11 2 4 57.4 11 30.74 2 26.7 8 4 4
2 1 30.9 7 4 1 30.9 6 17.7 1 62.7 8 1 1
1 2 26.4 9 3 3 26.4 5 18.46 1 28.4 5 3 3
1 2 55.8 5 4 1 55.8 11 23.61 2 29.3 2.9 1 1
2 2 38.3 7 4 2 38.3 . 24.24 2 36.3 8 2 2
2 2 44.2 12 1 4 44.2 9 25.32 2 31.9 8 5 4
2 2 33.9 3 2 2 . . 22.31 2 30.4 8 2 2
1 1 45.7 11 1 3 45.7 . 23.08 1 29.9 3.6 3 3
2 3 59.4 7 3 3 59.4 12 29.38 2 34.2 8 3 3
2 2 53.2 8 2 3 53.2 11 28.96 2 41.6 8 3 3
END DATA.
";

/// Parent node, class counts, the assigned class, the split variable, the test that chose it, its
/// statistic, df, error df and significance, and the split point of a scale or ordinal predictor.
type Node = (
    usize,
    &'static [f64],
    usize,
    &'static str,
    &'static str,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);

const EXPECTED_A: &[Node] = &[
    (
        0,
        &[88.0, 62.0],
        1,
        "x1",
        "F",
        Some(12.53259364),
        Some(1.0),
        Some(148.0),
        Some(0.0005349135583),
        Some(44.24012212),
    ),
    (
        0,
        &[59.0, 31.0],
        1,
        "x1",
        "F",
        Some(8.662092258),
        Some(1.0),
        Some(88.0),
        Some(0.004153647691),
        Some(37.49356583),
    ),
    (
        0,
        &[29.0, 31.0],
        2,
        "x2",
        "F",
        Some(10.05429715),
        Some(1.0),
        Some(58.0),
        Some(0.002428725314),
        Some(6.138670985),
    ),
    (1, &[52.0, 18.0], 1, "", "", None, None, None, None, None),
    (1, &[7.0, 13.0], 2, "", "", None, None, None, None, None),
    (2, &[19.0, 8.0], 1, "", "", None, None, None, None, None),
    (2, &[10.0, 23.0], 2, "", "", None, None, None, None, None),
];

const EXPECTED_B: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "x1",
        "F",
        Some(50.68026572),
        Some(2.0),
        Some(147.0),
        Some(1.816579629e-17),
        Some(34.72588732),
    ),
    (
        0,
        &[53.0, 8.0, 3.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[17.0, 40.0, 29.0],
        2,
        "x1",
        "F",
        Some(5.199051331),
        Some(2.0),
        Some(83.0),
        Some(0.007459532312),
        Some(54.77226282),
    ),
    (
        2,
        &[17.0, 35.0, 20.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (2, &[0.0, 5.0, 9.0], 3, "", "", None, None, None, None, None),
];

const EXPECTED_C: &[Node] = &[
    (
        0,
        &[88.0, 62.0],
        1,
        "b",
        "Chi-Square",
        Some(43.90275458),
        Some(1.0),
        None,
        Some(3.451028421e-11),
        None,
    ),
    (0, &[68.0, 14.0], 1, "", "", None, None, None, None, None),
    (0, &[20.0, 48.0], 2, "", "", None, None, None, None, None),
];

const EXPECTED_D: &[Node] = &[
    (
        0,
        &[88.0, 62.0],
        1,
        "v",
        "Levene F",
        Some(79.74862741),
        Some(1.0),
        Some(148.0),
        Some(1.53590164e-15),
        Some(34.34857785),
    ),
    (
        0,
        &[86.0, 41.0],
        1,
        "v",
        "F",
        Some(34.17839118),
        Some(1.0),
        Some(125.0),
        Some(4.112221839e-08),
        Some(25.35156197),
    ),
    (0, &[2.0, 21.0], 2, "", "", None, None, None, None, None),
    (1, &[2.0, 17.0], 2, "", "", None, None, None, None, None),
    (1, &[84.0, 24.0], 1, "", "", None, None, None, None, None),
];

const EXPECTED_E: &[Node] = &[
    (
        0,
        &[88.0, 62.0],
        1,
        "zv",
        "F",
        Some(45.99309583),
        Some(1.0),
        Some(148.0),
        Some(2.648109379e-10),
        Some(8.0),
    ),
    (0, &[65.0, 0.0], 1, "", "", None, None, None, None, None),
    (
        0,
        &[23.0, 62.0],
        2,
        "zv",
        "F",
        Some(90.59631568),
        Some(1.0),
        Some(83.0),
        Some(6.005563561e-15),
        Some(8.0),
    ),
    (2, &[2.0, 62.0], 2, "", "", None, None, None, None, None),
    (2, &[21.0, 0.0], 1, "", "", None, None, None, None, None),
];

const EXPECTED_F: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "x1",
        "F",
        Some(50.68026572),
        Some(2.0),
        Some(147.0),
        Some(1.816579629e-17),
        Some(30.81044301),
    ),
    (
        0,
        &[49.0, 7.0, 1.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[21.0, 41.0, 31.0],
        3,
        "x1",
        "F",
        Some(6.33567793),
        Some(2.0),
        Some(90.0),
        Some(0.00266498364),
        Some(45.09037698),
    ),
    (
        2,
        &[12.0, 17.0, 8.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[9.0, 24.0, 23.0],
        3,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_G: &[Node] = &[
    (
        0,
        &[88.0, 62.0],
        1,
        "b",
        "Chi-Square",
        Some(43.90275458),
        Some(1.0),
        None,
        Some(3.451028421e-11),
        None,
    ),
    (0, &[68.0, 14.0], 1, "", "", None, None, None, None, None),
    (
        0,
        &[20.0, 48.0],
        2,
        "z",
        "F",
        Some(10.96607522),
        Some(1.0),
        Some(63.0),
        Some(0.001539373067),
        Some(5.22315824),
    ),
    (2, &[10.0, 8.0], 1, "", "", None, None, None, None, None),
    (2, &[10.0, 40.0], 2, "", "", None, None, None, None, None),
];

const SURROGATES_G: &[(usize, &str, f64)] = &[
    (0, "xm", 0.1269071848),
    (0, "z", 0.07872852032),
    (0, "x2", 0.02941176471),
    (2, "xm", 1.0),
    (2, "x2", 0.05518707483),
];

const EXPECTED_H: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "z",
        "F",
        Some(49.90522955),
        Some(2.0),
        Some(141.0),
        Some(4.091057407e-17),
        Some(7.018504279),
    ),
    (
        0,
        &[54.0, 10.0, 4.0],
        1,
        "o",
        "F",
        Some(5.053646768),
        Some(2.0),
        Some(65.0),
        Some(0.009120432444),
        Some(2.744708995),
    ),
    (
        0,
        &[16.0, 38.0, 28.0],
        2,
        "z",
        "F",
        Some(4.766164778),
        Some(2.0),
        Some(73.0),
        Some(0.01133716873),
        Some(9.547235023),
    ),
    (
        1,
        &[34.0, 2.0, 0.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        1,
        &[20.0, 8.0, 4.0],
        1,
        "x2",
        "F",
        Some(5.138634427),
        Some(2.0),
        Some(29.0),
        Some(0.01229482603),
        Some(8.129470352),
    ),
    (
        2,
        &[8.0, 14.0, 8.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[8.0, 24.0, 20.0],
        2,
        "z",
        "F",
        Some(4.246583851),
        Some(2.0),
        Some(43.0),
        Some(0.02074699484),
        Some(10.0),
    ),
    (
        4,
        &[14.0, 3.0, 0.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (4, &[6.0, 5.0, 4.0], 1, "", "", None, None, None, None, None),
    (
        6,
        &[8.0, 15.0, 6.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        6,
        &[0.0, 9.0, 14.0],
        3,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_I: &[Node] = &[(0, &[88.0, 62.0], 1, "", "", None, None, None, None, None)];

const EXPECTED_P0: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "x1",
        "F",
        Some(50.68026572),
        Some(2.0),
        Some(147.0),
        Some(1.816579629e-17),
        Some(34.72588732),
    ),
    (
        0,
        &[53.0, 8.0, 3.0],
        1,
        "z",
        "Levene F",
        Some(4.763094227),
        Some(2.0),
        Some(59.0),
        Some(0.01208830317),
        Some(4.776836158),
    ),
    (
        0,
        &[17.0, 40.0, 29.0],
        2,
        "z",
        "F",
        Some(5.232082022),
        Some(2.0),
        Some(79.0),
        Some(0.007347343276),
        Some(10.92108989),
    ),
    (
        1,
        &[29.0, 5.0, 1.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        1,
        &[24.0, 3.0, 2.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[17.0, 31.0, 15.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[0.0, 9.0, 14.0],
        3,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_PB0: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "x1",
        "F",
        Some(50.68026572),
        Some(2.0),
        Some(147.0),
        Some(1.816579629e-17),
        Some(34.72588732),
    ),
    (
        0,
        &[53.0, 8.0, 3.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[17.0, 40.0, 29.0],
        2,
        "z",
        "F",
        Some(5.232082022),
        Some(2.0),
        Some(79.0),
        Some(0.007347343276),
        Some(10.92108989),
    ),
    (
        2,
        &[17.0, 31.0, 15.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[0.0, 9.0, 14.0],
        3,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_PB1: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "x1",
        "F",
        Some(50.68026572),
        Some(2.0),
        Some(147.0),
        Some(1.816579629e-17),
        Some(34.72588732),
    ),
    (
        0,
        &[53.0, 8.0, 3.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[17.0, 40.0, 29.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_N2: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "g4",
        "Chi-Square",
        Some(30.47086303),
        Some(8.0),
        None,
        Some(0.0001744584443),
        None,
    ),
    (
        0,
        &[30.0, 6.0, 2.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[40.0, 42.0, 30.0],
        2,
        "o",
        "F",
        Some(4.121319045),
        Some(2.0),
        Some(109.0),
        Some(0.01881827972),
        Some(1.079889979),
    ),
    (
        2,
        &[18.0, 11.0, 5.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[22.0, 31.0, 25.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const EXPECTED_N3: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "z",
        "F",
        Some(49.90522955),
        Some(2.0),
        Some(141.0),
        Some(4.091057407e-17),
        Some(7.018504279),
    ),
    (
        0,
        &[54.0, 12.0, 4.0],
        1,
        "x2",
        "Levene F",
        Some(5.893845765),
        Some(2.0),
        Some(67.0),
        Some(0.004386824883),
        Some(8.462121212),
    ),
    (
        0,
        &[16.0, 36.0, 28.0],
        2,
        "gm",
        "Chi-Square",
        Some(31.80107052),
        Some(6.0),
        None,
        Some(1.78145163e-05),
        None,
    ),
    (
        1,
        &[34.0, 7.0, 0.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        1,
        &[20.0, 5.0, 4.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[10.0, 5.0, 2.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[6.0, 31.0, 26.0],
        2,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

const SURROGATES_N3: &[(usize, &str, f64)] = &[
    (0, "gm", 0.1166673833),
    (0, "x2", 0.03849654859),
    (1, "z", 0.1466674688),
    (1, "gm", 0.09097133759),
];

const EXPECTED_N4: &[Node] = &[
    (
        0,
        &[70.0, 48.0, 32.0],
        1,
        "g",
        "Chi-Square",
        Some(29.77539109),
        Some(6.0),
        None,
        Some(4.336755423e-05),
        None,
    ),
    (
        0,
        &[30.0, 6.0, 2.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        0,
        &[40.0, 42.0, 30.0],
        3,
        "b",
        "Chi-Square",
        Some(7.89047619),
        Some(2.0),
        None,
        Some(0.01934660949),
        None,
    ),
    (
        2,
        &[27.0, 18.0, 11.0],
        1,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
    (
        2,
        &[13.0, 24.0, 19.0],
        3,
        "",
        "",
        None,
        None,
        None,
        None,
        None,
    ),
];

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
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

fn error(engine: &mut Engine, source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error")
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        Cell::Empty => "",
        other => panic!("not text: {other:?}"),
    }
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn column(table: &Table, name: &str) -> usize {
    table.columns.iter().position(|c| c == name).expect(name)
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "{actual} is not {expected}"
    );
}

fn engine() -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    engine
}

/// Runs a QUEST tree and compares its tree table with `expected`.
fn grown(command: &str, more: &str, classes: usize, expected: &[Node]) -> (Engine, Vec<Table>) {
    let mut engine = engine();
    let source = format!("{command} /PRINT MODELSUMMARY RISK TREETABLE{more} /TREE DISPLAY=NONE.");
    let tables = run(&mut engine, &source);
    let tree = table(&tables, "Tree Table");
    assert_eq!(tree.rows.len(), expected.len(), "{command}");
    let names = [
        "Parent Node",
        "Variable",
        "Test",
        "Statistic",
        "df",
        "Error df",
        "Sig.",
        "Split Point",
    ];
    let c: Vec<usize> = names.iter().map(|n| column(tree, n)).collect();
    let predicted = column(tree, "Predicted Category");
    for (id, (row, want)) in tree.rows.iter().zip(expected).enumerate() {
        if id > 0 {
            assert_eq!(number(&row[c[0]]), want.0 as f64, "parent of node {id}");
        }
        for j in 0..classes {
            // the N column of class j, after Node, Parent Node and Definition
            assert_eq!(number(&row[3 + 2 * j]), want.1[j], "node {id}, class {j}");
        }
        assert_eq!(
            text(&row[predicted]),
            want.2.to_string(),
            "prediction of node {id}"
        );
        assert_eq!(text(&row[c[1]]), want.3, "split variable of node {id}");
        assert_eq!(text(&row[c[2]]), want.4, "test of node {id}");
        for (k, expected) in [
            (3, want.5),
            (4, want.6),
            (5, want.7),
            (6, want.8),
            (7, want.9),
        ] {
            match expected {
                Some(e) => close(number(&row[c[k]]), e, 1e-8),
                None => assert!(matches!(row[c[k]], Cell::Empty), "node {id}, {}", names[k]),
            }
        }
    }
    (engine, tables)
}

const SMALL: &str = "/GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12";

#[test]
fn f_tests_on_scale_and_ordinal_predictors_two_classes() {
    let (engine, tables) = grown(
        &format!("TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        2,
        EXPECTED_A,
    );
    // the cut named in a node is the largest value of the data that goes left
    let tree = table(&tables, "Tree Table");
    assert_eq!(text(&tree.rows[1][2]), "x1 = <= 44.2");
    assert_eq!(text(&tree.rows[2][2]), "x1 = > 44.2");
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
    let summary = table(&tables, "Model Summary");
    assert!(summary.rows.iter().any(|r| text(&r[2]) == "QUEST"));
    assert!(
        summary
            .rows
            .iter()
            .any(|r| text(&r[1]) == "Alpha for Split Selection")
    );
    assert!(
        !summary
            .rows
            .iter()
            .any(|r| text(&r[1]) == "Impurity Measure")
    );
}

#[test]
fn three_classes_make_two_superclasses_by_two_means() {
    grown(
        &format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        3,
        EXPECTED_B,
    );
}

#[test]
fn a_nominal_predictor_of_two_categories_goes_by_the_chi_square() {
    grown(
        &format!("TREE y [n] BY x2 [s] b [n] o [o] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        2,
        EXPECTED_C,
    );
}

#[test]
fn levene_picks_a_predictor_whose_spread_differs() {
    grown(
        &format!("TREE y [n] BY v [s] x2 [s] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        2,
        EXPECTED_D,
    );
}

#[test]
fn a_superclass_of_zero_variance_is_cut_just_beyond_its_mean() {
    grown(
        &format!("TREE y [n] BY zv [s] x2 [s] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        2,
        EXPECTED_E,
    );
}

#[test]
fn the_priors_enter_the_quadratic_discriminant() {
    grown(
        &format!(
            "TREE y3 [n] BY x1 [s] o [o] /METHOD TYPE=QUEST MAXSURROGATES=0 /PRIORS EQUAL {SMALL}"
        ),
        "",
        3,
        EXPECTED_F,
    );
}

#[test]
fn missing_values_follow_the_surrogates() {
    let (_, tables) = grown(
        &format!(
            "TREE y [n] BY xm [s] z [s] x2 [s] o [o] b [n] /METHOD TYPE=QUEST MAXSURROGATES=3 {SMALL}"
        ),
        " SURROGATES",
        2,
        EXPECTED_G,
    );
    let found = table(&tables, "Surrogates");
    assert_eq!(found.rows.len(), SURROGATES_G.len());
    for (row, (node, variable, association)) in found.rows.iter().zip(SURROGATES_G) {
        assert_eq!(number(&row[0]), *node as f64);
        assert!(
            text(&row[2]).starts_with(variable),
            "{} for {variable}",
            text(&row[2])
        );
        close(number(&row[3]), *association, 1e-9);
    }
}

#[test]
fn alphasplit_sets_the_level_of_the_selection() {
    grown(
        &format!(
            "TREE y3 [n] BY x2 [s] o [o] z [s] /METHOD TYPE=QUEST MAXSURROGATES=0 /QUEST ALPHASPLIT=.2 {SMALL}"
        ),
        "",
        3,
        EXPECTED_H,
    );
}

#[test]
fn a_split_that_leaves_a_small_child_is_not_made() {
    grown(
        "TREE y [n] BY x1 [s] /METHOD TYPE=QUEST /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=62 MINCHILDSIZE=61",
        "",
        2,
        EXPECTED_I,
    );
}

#[test]
fn pruning_works_on_a_quest_tree() {
    let grow = "TREE y3 [n] BY x1 [s] x2 [s] o [o] z [s] /QUEST ALPHASPLIT=.2 /GROWTHLIMIT MAXDEPTH=5 MINPARENTSIZE=10 MINCHILDSIZE=5";
    grown(
        &format!("{grow} /METHOD TYPE=QUEST MAXSURROGATES=0"),
        "",
        3,
        EXPECTED_P0,
    );
    grown(
        &format!("{grow} /METHOD TYPE=QUEST MAXSURROGATES=0 PRUNE=SE(0)"),
        "",
        3,
        EXPECTED_PB0,
    );
    grown(
        &format!("{grow} /METHOD TYPE=QUEST MAXSURROGATES=0 PRUNE=SE(1)"),
        "",
        3,
        EXPECTED_PB1,
    );
}

#[test]
fn what_quest_cannot_do_is_refused_or_ignored() {
    let mut engine = engine();
    let message = error(&mut engine, "TREE r BY x1 /METHOD TYPE=QUEST.");
    assert!(message.contains("needs a nominal dependent"), "{message}");
    let message = error(&mut engine, "TREE o [o] BY x1 /METHOD TYPE=QUEST.");
    assert!(message.contains("needs a nominal dependent"), "{message}");
    // the settings of CRT are ignored with a warning, the importance too
    let mut engine = self::engine();
    run(
        &mut engine,
        "TREE y [n] BY x1 [s] /METHOD TYPE=QUEST /CRT IMPURITY=TWOING /PRINT IMPORTANCE.",
    );
    assert_eq!(engine.warnings().len(), 2, "{:?}", engine.warnings());
}

#[test]
fn a_nominal_predictor_of_more_categories_is_scored_and_cut() {
    grown(
        &format!("TREE y3 [n] BY g4 [n] o [o] /METHOD TYPE=QUEST MAXSURROGATES=0 {SMALL}"),
        "",
        3,
        EXPECTED_N2,
    );
}

#[test]
fn nominal_predictors_with_missing_values_follow_the_surrogates() {
    let (_, tables) = grown(
        &format!("TREE y3 [n] BY gm [n] z [s] x2 [s] /METHOD TYPE=QUEST MAXSURROGATES=2 {SMALL}"),
        " SURROGATES",
        3,
        EXPECTED_N3,
    );
    let found = table(&tables, "Surrogates");
    assert_eq!(found.rows.len(), SURROGATES_N3.len());
    for (row, (node, variable, association)) in found.rows.iter().zip(SURROGATES_N3) {
        assert_eq!(number(&row[0]), *node as f64);
        assert!(
            text(&row[2]).starts_with(variable),
            "{} for {variable}",
            text(&row[2])
        );
        close(number(&row[3]), *association, 1e-9);
    }
}

#[test]
fn equal_priors_and_a_nominal_predictor_with_three_classes() {
    grown(
        "TREE y3 [n] BY g [n] b [n] /METHOD TYPE=QUEST MAXSURROGATES=0 /PRIORS EQUAL /QUEST ALPHASPLIT=.2 /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=20 MINCHILDSIZE=8",
        "",
        3,
        EXPECTED_N4,
    );
}

#[test]
fn a_nominal_predictor_with_too_many_categories_is_refused() {
    let mut engine = Engine::default();
    let mut source = String::from(
        "DATA LIST FREE / y c.
BEGIN DATA
",
    );
    for i in 0..60 {
        source.push_str(&format!(
            "{} {}
",
            i % 2 + 1,
            i % 30 + 1
        ));
    }
    source.push_str(
        "END DATA.
",
    );
    run(&mut engine, &source);
    let message = error(&mut engine, "TREE y [n] BY c [n] /METHOD TYPE=QUEST.");
    assert!(message.contains("at most 25 categories"), "{message}");
}
