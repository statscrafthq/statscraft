//! TREE with COSTS CUSTOM on an invented sample of 150 cases (y with 2 classes, y3 with 3, by x1,
//! x2, o and g among others). The expected CRT trees with Gini and costs, their assignments and
//! risks come from an independent grower written in R from the manual's rules: the Gini measure
//! with costs i(t) = sum over i and j of C(i|j) p(i|t) p(j|t), the assignment of the class with the
//! least expected cost, the risk and its standard error as the mean cost of the assignments (M1 for
//! priors from the data, M2 otherwise). Other checks are by symmetry: a cost of c on every
//! misclassification scales improvements, risk and error by c.

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

/// Parent node, class counts, the assigned class, the split variable and its improvement.
type Node = (usize, &'static [f64], usize, &'static str, Option<f64>);

const EXPECTED_K1: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 3, "x1", Some(0.451577777778)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.0127587301587)),
    (0, &[18.0, 41.0, 31.0], 3, "g", Some(0.163808080808)),
    (1, &[33.0, 2.0, 0.0], 1, "g", Some(0.00406349206349)),
    (1, &[19.0, 5.0, 1.0], 1, "", None),
    (2, &[14.0, 6.0, 2.0], 2, "", None),
    (2, &[4.0, 35.0, 29.0], 3, "x1", Some(0.035)),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (3, &[20.0, 0.0, 0.0], 1, "", None),
    (6, &[4.0, 30.0, 17.0], 3, "", None),
    (6, &[0.0, 5.0, 12.0], 3, "", None),
];
const RISK_K1: (f64, f64) = (0.553333333333, 0.0676833586418);

const EXPECTED_K2: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 3, "x1", Some(0.453881899556)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.0190353374129)),
    (0, &[18.0, 41.0, 31.0], 3, "g", Some(0.122991099095)),
    (1, &[33.0, 2.0, 0.0], 1, "g", Some(0.00566886537659)),
    (1, &[19.0, 5.0, 1.0], 2, "", None),
    (2, &[14.0, 6.0, 2.0], 2, "", None),
    (2, &[4.0, 35.0, 29.0], 3, "x1", Some(0.0430184473708)),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (3, &[20.0, 0.0, 0.0], 1, "", None),
    (6, &[3.0, 30.0, 16.0], 3, "", None),
    (6, &[1.0, 5.0, 13.0], 3, "", None),
];
const RISK_K2: (f64, f64) = (0.523412698413, 0.0494226013367);

const EXPECTED_K3: &[Node] = &[
    (0, &[88.0, 62.0], 2, "x1", Some(0.132240102171)),
    (0, &[49.0, 14.0], 2, "x2", Some(0.0225342859489)),
    (0, &[39.0, 48.0], 2, "x2", Some(0.103325295394)),
    (1, &[14.0, 8.0], 2, "", None),
    (1, &[35.0, 6.0], 1, "x1", Some(0.0103648811608)),
    (2, &[17.0, 5.0], 2, "", None),
    (2, &[22.0, 43.0], 2, "x2", Some(0.0083966033966)),
    (4, &[17.0, 5.0], 2, "", None),
    (4, &[18.0, 1.0], 1, "", None),
    (6, &[9.0, 12.0], 2, "", None),
    (6, &[13.0, 31.0], 2, "", None),
];
const RISK_K3: (f64, f64) = (0.493333333333, 0.0469009989627);

const EXPECTED_PB0: &[(usize, &[f64], usize, &str)] = &[
    (0, &[70.0, 48.0, 32.0], 2, "x1"),
    (0, &[52.0, 7.0, 1.0], 1, "o"),
    (0, &[18.0, 41.0, 31.0], 3, "x1"),
    (1, &[33.0, 2.0, 0.0], 1, "x2"),
    (1, &[19.0, 5.0, 1.0], 1, "x2"),
    (2, &[18.0, 33.0, 17.0], 2, "o"),
    (2, &[0.0, 8.0, 14.0], 3, ""),
    (3, &[16.0, 2.0, 0.0], 1, "x2"),
    (3, &[17.0, 0.0, 0.0], 1, ""),
    (4, &[9.0, 0.0, 0.0], 1, ""),
    (4, &[10.0, 5.0, 1.0], 2, ""),
    (5, &[11.0, 17.0, 6.0], 2, ""),
    (5, &[7.0, 16.0, 11.0], 2, "x2"),
    (7, &[11.0, 0.0, 0.0], 1, ""),
    (7, &[5.0, 2.0, 0.0], 2, ""),
    (12, &[6.0, 14.0, 4.0], 2, ""),
    (12, &[1.0, 2.0, 7.0], 3, ""),
];

const EXPECTED_PB1: &[(usize, &[f64], usize, &str)] = &[
    (0, &[70.0, 48.0, 32.0], 2, "x1"),
    (0, &[52.0, 7.0, 1.0], 1, "o"),
    (0, &[18.0, 41.0, 31.0], 3, "x1"),
    (1, &[33.0, 2.0, 0.0], 1, ""),
    (1, &[19.0, 5.0, 1.0], 1, "x2"),
    (2, &[18.0, 33.0, 17.0], 2, "o"),
    (2, &[0.0, 8.0, 14.0], 3, ""),
    (4, &[9.0, 0.0, 0.0], 1, ""),
    (4, &[10.0, 5.0, 1.0], 2, ""),
    (5, &[11.0, 17.0, 6.0], 2, ""),
    (5, &[7.0, 16.0, 11.0], 2, "x2"),
    (10, &[6.0, 14.0, 4.0], 2, ""),
    (10, &[1.0, 2.0, 7.0], 3, ""),
];

const EXPECTED_A1: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 3, "x1", Some(0.130462536446)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.00907743031857)),
    (0, &[18.0, 41.0, 31.0], 3, "g", Some(0.0326284616994)),
    (1, &[33.0, 2.0, 0.0], 1, "g", Some(0.00366002905148)),
    (1, &[19.0, 5.0, 1.0], 1, "", None),
    (2, &[17.0, 23.0, 9.0], 2, "x1", Some(0.0221958007823)),
    (2, &[1.0, 18.0, 22.0], 3, "x1", Some(0.0348837653236)),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (3, &[20.0, 0.0, 0.0], 1, "", None),
    (5, &[17.0, 16.0, 3.0], 2, "", None),
    (5, &[0.0, 7.0, 6.0], 3, "", None),
    (6, &[0.0, 16.0, 9.0], 2, "", None),
    (6, &[1.0, 2.0, 13.0], 3, "", None),
];
const RISK_A1: (f64, f64) = (0.343525179856, 0.0441425363519);

const EXPECTED_A2: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 3, "x1", Some(0.0949257563006)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.00998744669804)),
    (0, &[18.0, 41.0, 31.0], 3, "g", Some(0.0293262835907)),
    (1, &[33.0, 2.0, 0.0], 1, "g", Some(0.00409025438461)),
    (1, &[19.0, 5.0, 1.0], 1, "", None),
    (2, &[17.0, 23.0, 9.0], 3, "x1", Some(0.0226556297579)),
    (2, &[1.0, 18.0, 22.0], 3, "x1", Some(0.0288676393084)),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (3, &[20.0, 0.0, 0.0], 1, "", None),
    (5, &[17.0, 16.0, 3.0], 2, "", None),
    (5, &[0.0, 7.0, 6.0], 3, "", None),
    (6, &[0.0, 16.0, 9.0], 3, "", None),
    (6, &[1.0, 2.0, 13.0], 3, "", None),
];
const RISK_A2: (f64, f64) = (0.311996336996, 0.0386510710614);

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

fn engine() -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    engine
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
        (actual - expected).abs() <= tolerance * expected.abs().max(1e-300),
        "{actual} is not {expected}"
    );
}

const SMALL: &str = "/GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12";

/// The tree table and the risk of a command.
/// The rows of the tree table, the columns of the assignment, the variable and the improvement,
/// the risk and its error, and the warnings.
type Grown = (Vec<Vec<Cell>>, Vec<usize>, (f64, f64), Vec<String>);

fn grown(command: &str) -> Grown {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!("{command} /PRINT RISK TREETABLE /TREE DISPLAY=NONE."),
    );
    let tree = table(&tables, "Tree Table");
    let improvement = tree.columns.iter().position(|c| c == "Improvement");
    let columns: Vec<usize> = ["Predicted Category", "Variable"]
        .iter()
        .map(|c| column(tree, c))
        .chain(improvement)
        .collect();
    let risk = table(&tables, "Risk");
    (
        tree.rows.clone(),
        columns,
        (number(&risk.rows[0][0]), number(&risk.rows[0][1])),
        engine.warnings().to_vec(),
    )
}

/// The tree of `command` against the R grower's nodes and risk.
fn matches_r(command: &str, classes: usize, expected: &[Node], risk: (f64, f64)) {
    let (rows, columns, found, warnings) = grown(command);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(rows.len(), expected.len(), "{command}");
    let parent = 1;
    for (id, (row, want)) in rows.iter().zip(expected).enumerate() {
        if id > 0 {
            assert_eq!(number(&row[parent]), want.0 as f64, "parent of node {id}");
        }
        for j in 0..classes {
            assert_eq!(number(&row[3 + 2 * j]), want.1[j], "node {id}, class {j}");
        }
        assert_eq!(
            text(&row[columns[0]]),
            want.2.to_string(),
            "assignment of node {id}"
        );
        assert_eq!(
            text(&row[columns[1]]),
            want.3,
            "split variable of node {id}"
        );
        match want.4 {
            Some(g) => close(number(&row[columns[2]]), g, 1e-9),
            None => assert!(matches!(row[columns[2]], Cell::Empty), "node {id} splits"),
        }
    }
    close(found.0, risk.0, 1e-9);
    close(found.1, risk.1, 1e-9);
}

#[test]
fn three_classes_with_asymmetric_costs_and_priors_from_the_data() {
    matches_r(
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3] {SMALL}"
        ),
        3,
        EXPECTED_K1,
        RISK_K1,
    );
}

#[test]
fn three_classes_with_asymmetric_costs_and_equal_priors() {
    matches_r(
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /PRIORS EQUAL /COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3] {SMALL}"
        ),
        3,
        EXPECTED_K2,
        RISK_K2,
    );
}

#[test]
fn two_classes_with_a_cost_on_one_kind_of_mistake() {
    matches_r(
        &format!(
            "TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT /COSTS CUSTOM=2 1 [4] {SMALL}"
        ),
        2,
        EXPECTED_K3,
        RISK_K3,
    );
}

#[test]
fn one_cost_on_every_mistake_scales_improvements_risk_and_error() {
    let plain =
        "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /CRT MINIMPROVEMENT=.0000001";
    let costs = "/COSTS CUSTOM=1 2 [3] 1 3 [3] 2 1 [3] 2 3 [3] 3 1 [3] 3 2 [3]";
    let (a, columns, risk_a, _) = grown(&format!("{plain} {SMALL}"));
    let (b, _, risk_b, _) = grown(&format!("{plain} {costs} {SMALL}"));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(text(&x[columns[0]]), text(&y[columns[0]]));
        assert_eq!(text(&x[columns[1]]), text(&y[columns[1]]));
        if let (Cell::Number(g), Cell::Number(h)) = (&x[columns[2]], &y[columns[2]]) {
            close(*h, 3.0 * g, 1e-9);
        }
    }
    close(risk_b.0, 3.0 * risk_a.0, 1e-9);
    close(risk_b.1, 3.0 * risk_a.1, 1e-9);
}

#[test]
fn two_classes_scale_the_improvement_by_the_mean_of_the_two_costs() {
    let plain = "TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT";
    let (a, columns, _, _) = grown(&format!("{plain} {SMALL}"));
    // 4 for a case of class 2 assigned class 1, 1 the other way: the Gini measure is (4 + 1) / 2
    // times the plain one, so the same splits are best
    let (b, _, _, _) = grown(&format!("{plain} /COSTS CUSTOM=2 1 [4] {SMALL}"));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(text(&x[columns[1]]), text(&y[columns[1]]));
        if let (Cell::Number(g), Cell::Number(h)) = (&x[columns[2]], &y[columns[2]]) {
            close(*h, 2.5 * g, 1e-9);
        }
    }
}

#[test]
fn costs_leave_chaid_quest_and_twoing_trees_as_they_are_and_change_assignments() {
    let costs = "/COSTS CUSTOM=3 1 [9] 3 2 [9] 2 1 [4]";
    let methods = [
        "/METHOD TYPE=CHAID",
        "/METHOD TYPE=CRT /CRT IMPURITY=TWOING",
        "/METHOD TYPE=QUEST",
    ];
    let mut moved = false;
    for method in methods {
        let command = format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] {method} {SMALL}");
        let (a, columns_a, risk_a, _) = grown(&command);
        let (b, columns_b, risk_b, _) = grown(&format!("{command} {costs}"));
        assert_eq!(a.len(), b.len(), "{method}");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(text(&x[columns_a[1]]), text(&y[columns_b[1]]), "{method}");
            for k in 3..9 {
                assert_eq!(x[k], y[k], "{method}");
            }
            moved |= text(&x[columns_a[0]]) != text(&y[columns_b[0]]);
        }
        assert!((risk_a.0 - risk_b.0).abs() > 1e-9, "{method}");
    }
    assert!(moved, "no assignment changed");
}

#[test]
fn what_costs_cannot_say_is_warned() {
    let mut engine = engine();
    run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY x1 [s] /METHOD TYPE=CRT /COSTS CUSTOM=1 1 [2] 7 1 [3] 2 1 [4] {SMALL}."
        ),
    );
    let warnings = engine.warnings().to_vec();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("classifying category 1 correctly")),
        "{warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("not in the analysis")),
        "{warnings:?}"
    );
    let mut engine = self::engine();
    run(
        &mut engine,
        "TREE r [s] BY x1 [s] /METHOD TYPE=CRT /COSTS CUSTOM=1 2 [3].",
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("COSTS needs a categorical"))
    );
}

#[test]
fn pruning_counts_the_cost_of_each_nodes_assignment() {
    for (b, expected) in [(0, EXPECTED_PB0), (1, EXPECTED_PB1)] {
        let command = format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] z [s] /METHOD TYPE=CRT MAXSURROGATES=0 PRUNE=SE({b})              /CRT MINIMPROVEMENT=.0000001 /COSTS CUSTOM=3 1 [6] 3 2 [2] 2 1 [3] 1 3 [2]              /GROWTHLIMIT MAXDEPTH=5 MINPARENTSIZE=10 MINCHILDSIZE=5"
        );
        let (rows, columns, _, _) = grown(&command);
        assert_eq!(rows.len(), expected.len(), "b = {b}");
        for (id, (row, want)) in rows.iter().zip(expected).enumerate() {
            if id > 0 {
                assert_eq!(
                    number(&row[1]),
                    want.0 as f64,
                    "parent of node {id}, b = {b}"
                );
            }
            for j in 0..3 {
                assert_eq!(number(&row[3 + 2 * j]), want.1[j], "node {id}, class {j}");
            }
            assert_eq!(
                text(&row[columns[0]]),
                want.2.to_string(),
                "assignment of node {id}"
            );
            assert_eq!(
                text(&row[columns[1]]),
                want.3,
                "split variable of node {id}"
            );
        }
    }
}

#[test]
fn adjusted_priors_fold_the_costs_into_the_priors_and_drop_the_costs() {
    // C(j), the sum over the assigned categories of the cost of j, is 2, 4 and 7 here; the priors
    // times that, scaled to sum to 1, grow the tree, assign its nodes and weigh the risk
    let costs = "/COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3]";
    matches_r(
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {costs} /PRIORS ADJUST=YES {SMALL}"
        ),
        3,
        EXPECTED_A1,
        RISK_A1,
    );
    matches_r(
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {costs} /PRIORS EQUAL ADJUST=YES {SMALL}"
        ),
        3,
        EXPECTED_A2,
        RISK_A2,
    );
}

#[test]
fn adjusting_with_equal_costs_changes_nothing_and_chaid_ignores_the_priors() {
    let tree = "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n]";
    for method in ["/METHOD TYPE=CRT", "/METHOD TYPE=QUEST"] {
        let (a, _, risk_a, _) = grown(&format!("{tree} {method} {SMALL}"));
        let (b, _, risk_b, _) = grown(&format!("{tree} {method} /PRIORS ADJUST=YES {SMALL}"));
        assert_eq!(a, b, "{method}");
        assert_eq!(risk_a, risk_b, "{method}");
    }
    // CHAID takes no priors, so the adjustment is ignored (with a warning) and the costs still
    // decide the assignments
    let costs = "/COSTS CUSTOM=3 1 [9] 2 1 [4]";
    let chaid = format!("{tree} /METHOD TYPE=CHAID {costs} {SMALL}");
    let (a, _, risk_a, _) = grown(&chaid);
    let (b, _, risk_b, warnings) = grown(&format!("{chaid} /PRIORS ADJUST=YES"));
    assert_eq!(a, b);
    assert_eq!(risk_a, risk_b);
    assert!(
        warnings.iter().any(|w| w.contains("PRIORS")),
        "{warnings:?}"
    );
}
