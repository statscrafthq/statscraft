//! TREE with METHOD TYPE=CRT on an invented sample of 150 cases: y (1, 2) and y3 (1 to 3) by x1
//! (scale, distinct values), x2 (scale, ties), o (ordinal 1 to 4), g (nominal 1 to 4) and xm (x1
//! with 14 system-missing values) and z (x1 over 5, rounded, 6 system-missing). The expected trees come from an independent brute-force grower
//! written in R from the manual's rules (every candidate cut, every subset of a nominal
//! predictor, p(j,t) from the priors and the class totals of the cases with a value of the
//! predictor, the split with the largest improvement). For a binary dependent, a Gini measure and
//! priors from the data, the improvements of the splits that survive pruning agree with rpart
//! (cp = 0, no surrogates): its improve column divided by the number of cases.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE / y y3 x1 x2 o g xm z.
BEGIN DATA
1 1 29.4 8 1 2 29.4 6
1 1 10.5 8 4 4 10.5 2
2 2 42.5 12 2 4 42.5 8
1 1 11.1 7 1 2 11.1 2
2 2 45.8 8 3 2 45.8 9
2 3 52 10 3 4 52 10
2 2 43.3 5 2 3 43.3 9
1 2 39.9 4 1 4 39.9 .
2 2 48.3 5 1 3 48.3 10
2 3 58.1 5 2 1 58.1 12
2 3 53.2 4 2 2 . 11
1 1 14.7 12 2 1 14.7 3
1 1 13.6 7 4 1 13.6 3
1 1 24.9 9 1 2 24.9 5
1 1 35.4 11 1 1 35.4 7
1 3 45.2 9 2 4 45.2 9
1 2 51.6 3 1 1 51.6 10
2 3 53.8 8 3 4 . 11
1 1 17.7 8 4 3 17.7 4
1 1 22.6 7 2 2 22.6 5
2 2 41.8 10 2 2 41.8 8
2 3 48.7 3 2 2 . 10
2 2 53 9 4 1 53 11
1 1 26 3 2 1 26 5
2 3 55.6 6 1 2 55.6 11
1 1 15.9 8 1 2 15.9 3
1 2 51.9 2 3 3 51.9 10
1 1 11.2 12 1 1 11.2 2
2 3 56.2 3 2 3 56.2 11
2 1 15.2 3 4 1 15.2 3
1 3 55.9 2 4 2 55.9 11
1 3 57.3 12 1 3 57.3 11
1 2 56.9 2 1 3 56.9 11
2 3 46.9 4 1 4 46.9 9
1 1 41 11 2 3 41 8
1 1 23.3 9 3 3 23.3 5
2 2 40.6 7 4 3 40.6 8
1 1 49.8 6 1 1 49.8 10
2 1 10.5 3 1 1 10.5 2
1 1 37.9 3 2 1 37.9 8
2 3 45.9 12 1 4 45.9 9
1 1 26.7 2 2 2 26.7 5
2 2 50.3 10 2 4 50.3 10
1 1 16.6 3 3 4 16.6 3
1 1 15.8 6 1 3 15.8 3
1 2 26.8 9 3 2 26.8 5
1 3 33 10 4 3 33 7
1 1 17.5 2 1 2 . 4
2 2 47.8 11 1 4 47.8 10
1 2 49.2 2 3 3 49.2 10
1 1 18.7 9 2 4 18.7 4
2 2 35 12 4 3 35 7
1 1 27.6 8 1 4 27.6 6
1 1 35.9 1 4 1 . 7
2 2 38.9 8 1 4 38.9 8
2 2 54 5 2 3 54 11
2 2 37.3 8 4 2 37.3 7
1 2 46.1 3 4 4 46.1 9
1 1 16.8 11 4 2 16.8 3
1 1 47.4 4 3 1 47.4 9
2 3 51.2 12 3 2 51.2 10
1 2 47.7 3 4 4 47.7 10
1 1 49.5 12 2 1 . 10
2 3 44.4 12 3 3 . 9
1 3 54 11 3 1 54 11
1 1 25 1 4 1 25 5
2 1 20.9 12 2 4 20.9 4
1 1 23.9 11 3 1 23.9 5
2 3 49.9 9 1 2 49.9 10
1 3 53.1 5 4 4 53.1 11
2 2 18 12 3 3 18 4
2 3 59.3 12 4 4 59.3 12
1 2 45.8 5 3 3 45.8 9
1 3 50.9 3 3 4 50.9 10
2 2 44.9 9 4 3 . 9
1 1 29.7 6 2 1 29.7 6
2 2 39.3 7 4 3 39.3 8
1 1 49.4 4 1 1 49.4 10
1 1 12.1 9 2 3 12.1 2
1 1 47.8 7 2 3 . 10
1 2 48.5 3 2 2 48.5 10
1 1 45 3 3 1 45 9
1 3 32.9 10 3 2 32.9 7
1 2 45.9 1 2 3 45.9 9
1 1 17.8 10 1 3 17.8 4
1 1 24.9 3 1 4 24.9 5
2 2 56.1 11 1 3 56.1 11
1 1 22.1 11 1 3 22.1 4
2 3 11.8 11 3 2 11.8 2
2 1 14.8 5 4 1 14.8 .
1 1 44.1 10 4 1 44.1 9
1 1 12.3 9 1 3 12.3 2
1 1 21.5 7 3 3 21.5 4
2 3 47.8 8 3 4 47.8 10
2 1 28.1 3 2 1 28.1 6
1 2 37.6 7 3 3 37.6 8
1 1 31 11 1 1 31 6
1 2 21.2 7 4 2 . 4
2 2 59.6 7 1 1 59.6 12
1 1 26.7 2 2 3 26.7 5
2 1 24.6 3 2 2 24.6 5
1 3 42.9 11 3 4 42.9 9
1 1 23.5 10 2 4 23.5 5
2 1 12 10 4 3 . 2
1 1 19.1 10 3 1 19.1 4
2 1 41.2 9 2 1 41.2 8
1 2 37.4 10 4 2 37.4 7
1 3 45 8 4 3 45 9
1 1 31.1 12 1 3 31.1 6
2 1 26.7 3 1 3 26.7 5
1 2 54.5 4 4 1 54.5 11
1 2 13.8 7 4 2 13.8 3
1 1 50.4 10 2 1 50.4 10
2 1 52.5 11 1 2 52.5 10
2 3 35.6 9 4 4 35.6 7
1 1 12.9 3 3 1 12.9 3
2 2 41.9 8 1 4 41.9 8
1 2 16.4 7 1 4 16.4 3
2 1 46.4 8 3 1 46.4 9
1 1 19 4 4 3 19 4
2 1 18.8 9 1 2 . 4
1 2 52.3 12 1 1 52.3 10
1 1 15.1 2 1 3 15.1 3
2 2 51.9 8 3 3 51.9 10
1 1 29.3 5 1 1 29.3 6
1 1 18.3 3 2 3 18.3 4
2 1 24.8 4 4 2 24.8 5
1 1 33.7 9 4 1 33.7 7
2 3 40.4 6 2 2 40.4 8
2 2 51.2 9 2 4 51.2 10
2 3 54.2 12 3 4 54.2 11
1 1 13.2 8 1 3 13.2 3
2 3 39.3 10 4 4 . 8
1 1 30.1 3 3 2 30.1 6
1 2 59.7 6 1 3 59.7 12
1 1 29.2 5 3 4 29.2 6
2 1 42 5 3 1 42 8
1 3 55 7 3 3 55 11
2 2 25.6 4 2 4 25.6 5
1 2 39.7 6 1 4 39.7 .
2 3 57.4 11 2 4 57.4 11
2 1 30.9 7 4 1 30.9 6
1 2 26.4 9 3 3 26.4 5
1 2 55.8 5 4 1 55.8 11
2 2 38.3 7 4 2 38.3 .
2 2 44.2 12 1 4 44.2 9
2 2 33.9 3 2 2 . .
1 1 45.7 11 1 3 45.7 .
2 3 59.4 7 3 3 59.4 12
2 2 53.2 8 2 3 53.2 11
END DATA.
";

/// Parent node, class counts, the assigned class, the split variable and its improvement.
type Node = (usize, &'static [f64], usize, &'static str, Option<f64>);

const EXPECTED_A: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "x2", Some(0.00901371437957)),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0413301181577)),
    (1, &[14.0, 8.0], 1, "", None),
    (1, &[35.0, 6.0], 1, "x1", Some(0.00414595246431)),
    (2, &[17.0, 5.0], 1, "", None),
    (2, &[22.0, 43.0], 2, "x2", Some(0.00335864135864)),
    (4, &[17.0, 5.0], 1, "", None),
    (4, &[18.0, 1.0], 1, "", None),
    (6, &[9.0, 12.0], 2, "", None),
    (6, &[13.0, 31.0], 2, "", None),
];

const EXPECTED_B: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "x1", Some(0.00233057525611)),
    (0, &[39.0, 48.0], 2, "g", Some(0.0296412325752)),
    (1, &[11.0, 5.0], 1, "", None),
    (1, &[38.0, 9.0], 1, "x1", Some(0.00167582205029)),
    (2, &[28.0, 19.0], 1, "x1", Some(0.00742992232354)),
    (2, &[11.0, 29.0], 2, "x1", Some(0.00272222222222)),
    (4, &[19.0, 3.0], 1, "", None),
    (4, &[19.0, 6.0], 1, "", None),
    (5, &[6.0, 8.0], 2, "", None),
    (5, &[22.0, 11.0], 1, "", None),
    (6, &[8.0, 16.0], 2, "", None),
    (6, &[3.0, 13.0], 2, "", None),
];

const EXPECTED_C: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "g", Some(0.14142021721)),
    (0, &[30.0, 6.0, 2.0], 1, "x2", Some(0.00169284750337)),
    (0, &[40.0, 42.0, 30.0], 2, "o", Some(0.0453013824885)),
    (1, &[11.0, 2.0, 0.0], 1, "", None),
    (1, &[19.0, 4.0, 2.0], 1, "", None),
    (2, &[29.0, 22.0, 11.0], 1, "g", Some(0.0373271889401)),
    (2, &[11.0, 20.0, 19.0], 2, "x2", Some(0.0469587301587)),
    (5, &[24.0, 11.0, 7.0], 1, "", None),
    (5, &[5.0, 11.0, 4.0], 2, "", None),
    (6, &[9.0, 17.0, 9.0], 2, "", None),
    (6, &[2.0, 3.0, 10.0], 3, "", None),
];

const EXPECTED_D: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "x1", Some(0.0994389148367)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.00478963767183)),
    (0, &[18.0, 41.0, 31.0], 3, "x1", Some(0.0189522968006)),
    (1, &[33.0, 2.0, 0.0], 1, "x2", Some(0.00101359063204)),
    (1, &[19.0, 5.0, 1.0], 1, "", None),
    (2, &[18.0, 33.0, 17.0], 2, "x2", Some(0.00754039690876)),
    (2, &[0.0, 8.0, 14.0], 3, "", None),
    (3, &[16.0, 2.0, 0.0], 1, "", None),
    (3, &[17.0, 0.0, 0.0], 1, "", None),
    (5, &[9.0, 22.0, 6.0], 2, "", None),
    (5, &[9.0, 11.0, 11.0], 3, "", None),
];

const EXPECTED_E: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "x1", Some(0.158942508893)),
    (0, &[52.0, 7.0, 1.0], 1, "o", Some(0.00467421428051)),
    (0, &[18.0, 41.0, 31.0], 2, "g", Some(0.0603481867284)),
    (1, &[33.0, 2.0, 0.0], 1, "g", Some(0.00170710888916)),
    (1, &[19.0, 5.0, 1.0], 1, "", None),
    (2, &[14.0, 6.0, 2.0], 1, "", None),
    (2, &[4.0, 35.0, 29.0], 2, "x1", Some(0.0212108330278)),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (3, &[20.0, 0.0, 0.0], 1, "", None),
    (6, &[3.0, 30.0, 16.0], 2, "", None),
    (6, &[1.0, 5.0, 13.0], 3, "", None),
];

const EXPECTED_F: &[Node] = &[
    (0, &[88.0, 62.0], 1, "xm", Some(0.0555547181763)),
    (0, &[47.0, 12.0], 1, "o", Some(0.00359797799584)),
    (0, &[41.0, 50.0], 2, "xm", Some(0.00542903032041)),
    (1, &[17.0, 2.0], 1, "", None),
    (1, &[30.0, 10.0], 1, "xm", Some(0.00285306203543)),
    (2, &[7.0, 13.0], 2, "", None),
    (2, &[34.0, 37.0], 2, "o", Some(0.00894254415381)),
    (4, &[19.0, 8.0], 1, "", None),
    (4, &[11.0, 2.0], 1, "", None),
    (6, &[24.0, 32.0], 2, "", None),
    (6, &[10.0, 5.0], 1, "", None),
];

const EXPECTED_G: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "x2", Some(0.00302120669056)),
    (0, &[39.0, 48.0], 2, "x2", Some(0.034061386889)),
    (1, &[23.0, 9.0], 1, "", None),
    (1, &[26.0, 5.0], 1, "", None),
    (2, &[23.0, 12.0], 1, "", None),
    (2, &[16.0, 36.0], 2, "", None),
];

const EXPECTED_H: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "", None),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0413301181577)),
    (2, &[17.0, 5.0], 1, "", None),
    (2, &[22.0, 43.0], 2, "", None),
];

const EXPECTED_S1: &[Node] = &[
    (0, &[88.0, 62.0], 1, "xm", Some(0.0555547181763)),
    (0, &[49.0, 15.0], 1, "x2", Some(0.0117888388123)),
    (0, &[39.0, 47.0], 2, "x2", Some(0.0469601612857)),
    (1, &[14.0, 9.0], 1, "", None),
    (1, &[35.0, 6.0], 1, "x2", Some(0.0037593495935)),
    (2, &[17.0, 4.0], 1, "", None),
    (2, &[22.0, 43.0], 2, "g", Some(0.0219242979243)),
    (4, &[23.0, 2.0], 1, "", None),
    (4, &[12.0, 4.0], 1, "", None),
    (6, &[17.0, 18.0], 2, "", None),
    (6, &[5.0, 25.0], 2, "", None),
];
/// Node, surrogate variable and association.
const SURROGATES_S1: &[(usize, &str, f64)] = &[
    (0, "z", 0.948331791002),
    (0, "x2", 0.040062642369),
    (0, "g", 0.0229071753986),
    (1, "xm", 0.0570664553828),
    (2, "xm", 0.0317732624999),
    (4, "xm", 0.178815723376),
    (4, "g", 0.0625),
    (4, "z", 0.0608090372176),
    (6, "x2", 0.2),
    (6, "xm", 0.128819782248),
    (6, "z", 0.0550433606003),
];

const EXPECTED_S2: &[Node] = &[
    (0, &[88.0, 62.0], 1, "xm", Some(0.0555547181763)),
    (0, &[49.0, 14.0], 1, "x2", Some(0.0174658869396)),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0502711647272)),
    (1, &[15.0, 10.0], 1, "", None),
    (1, &[34.0, 4.0], 1, "o", Some(0.00327485380117)),
    (2, &[16.0, 3.0], 1, "", None),
    (2, &[23.0, 45.0], 2, "g", Some(0.018311546841)),
    (4, &[14.0, 0.0], 1, "", None),
    (4, &[20.0, 4.0], 1, "", None),
    (6, &[17.0, 19.0], 2, "", None),
    (6, &[6.0, 26.0], 2, "", None),
];
/// Node, surrogate variable and association.
const SURROGATES_S2: &[(usize, &str, f64)] = &[
    (0, "x2", 0.040062642369),
    (1, "xm", 0.146055808656),
    (4, "xm", 0.0814212356074),
    (6, "x2", 0.1875),
];

const EXPECTED_S3: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "xm", Some(0.172318667386)),
    (0, &[52.0, 8.0, 1.0], 1, "g", Some(0.0059636155719)),
    (0, &[18.0, 40.0, 31.0], 2, "g", Some(0.0600495462736)),
    (1, &[32.0, 2.0, 0.0], 1, "o", Some(0.00166980539862)),
    (1, &[20.0, 6.0, 1.0], 1, "", None),
    (2, &[14.0, 6.0, 2.0], 1, "", None),
    (2, &[4.0, 34.0, 29.0], 2, "xm", Some(0.0227635039243)),
    (3, &[19.0, 0.0, 0.0], 1, "", None),
    (3, &[13.0, 2.0, 0.0], 1, "", None),
    (6, &[4.0, 31.0, 18.0], 2, "", None),
    (6, &[0.0, 3.0, 11.0], 3, "", None),
];
/// Node, surrogate variable and association.
const SURROGATES_S3: &[(usize, &str, f64)] = &[
    (0, "z", 1.0),
    (0, "g", 0.0153614376799),
    (1, "xm", 0.11096240973),
    (2, "xm", 0.00515037110337),
    (3, "xm", 0.100789762795),
    (6, "z", 0.741709146409),
];

const EXPECTED_P1_B0: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "x2", Some(0.00901371437957)),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0413301181577)),
    (1, &[14.0, 8.0], 1, "x2", Some(0.0114081996435)),
    (1, &[35.0, 6.0], 1, "", None),
    (2, &[17.0, 5.0], 1, "o", Some(0.0126262626263)),
    (2, &[22.0, 43.0], 2, "g", Some(0.0219242979243)),
    (3, &[5.0, 0.0], 1, "", None),
    (3, &[9.0, 8.0], 1, "x1", Some(0.00544494720965)),
    (5, &[7.0, 5.0], 1, "g", Some(0.01)),
    (5, &[10.0, 0.0], 1, "", None),
    (6, &[17.0, 18.0], 2, "x2", Some(0.0170062111801)),
    (6, &[5.0, 25.0], 2, "", None),
    (8, &[1.0, 3.0], 2, "", None),
    (8, &[8.0, 5.0], 1, "", None),
    (9, &[5.0, 1.0], 1, "", None),
    (9, &[2.0, 4.0], 2, "", None),
    (11, &[8.0, 15.0], 2, "", None),
    (11, &[9.0, 3.0], 1, "", None),
];
const EXPECTED_P1_B05: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "", None),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0413301181577)),
    (2, &[17.0, 5.0], 1, "o", Some(0.0126262626263)),
    (2, &[22.0, 43.0], 2, "g", Some(0.0219242979243)),
    (3, &[7.0, 5.0], 1, "g", Some(0.01)),
    (3, &[10.0, 0.0], 1, "", None),
    (4, &[17.0, 18.0], 2, "x2", Some(0.0170062111801)),
    (4, &[5.0, 25.0], 2, "", None),
    (5, &[5.0, 1.0], 1, "", None),
    (5, &[2.0, 4.0], 2, "", None),
    (7, &[8.0, 15.0], 2, "", None),
    (7, &[9.0, 3.0], 1, "", None),
];
const EXPECTED_P1_B1: &[Node] = &[
    (0, &[88.0, 62.0], 1, "x1", Some(0.0528960408685)),
    (0, &[49.0, 14.0], 1, "", None),
    (0, &[39.0, 48.0], 2, "x2", Some(0.0413301181577)),
    (2, &[17.0, 5.0], 1, "", None),
    (2, &[22.0, 43.0], 2, "g", Some(0.0219242979243)),
    (4, &[17.0, 18.0], 2, "x2", Some(0.0170062111801)),
    (4, &[5.0, 25.0], 2, "", None),
    (5, &[8.0, 15.0], 2, "", None),
    (5, &[9.0, 3.0], 1, "", None),
];
const EXPECTED_P2_B0: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "x1", Some(0.158942508893)),
    (0, &[52.0, 7.0, 1.0], 1, "", None),
    (0, &[18.0, 41.0, 31.0], 2, "g", Some(0.0603481867284)),
    (2, &[14.0, 6.0, 2.0], 1, "x1", Some(0.0541666666667)),
    (2, &[4.0, 35.0, 29.0], 2, "x1", Some(0.0212108330278)),
    (3, &[14.0, 0.0, 0.0], 1, "", None),
    (3, &[0.0, 6.0, 2.0], 2, "", None),
    (4, &[3.0, 30.0, 16.0], 2, "x2", Some(0.0132459208412)),
    (4, &[1.0, 5.0, 13.0], 3, "g", Some(0.0154230315558)),
    (7, &[1.0, 21.0, 6.0], 2, "", None),
    (7, &[2.0, 9.0, 10.0], 3, "o", Some(0.00938418571064)),
    (8, &[1.0, 0.0, 9.0], 3, "", None),
    (8, &[0.0, 5.0, 4.0], 2, "x1", Some(0.00340277777778)),
    (10, &[2.0, 6.0, 3.0], 2, "", None),
    (10, &[0.0, 3.0, 7.0], 3, "", None),
    (12, &[0.0, 3.0, 1.0], 2, "", None),
    (12, &[0.0, 2.0, 3.0], 3, "", None),
];
const EXPECTED_P2_B1: &[Node] = &[
    (0, &[70.0, 48.0, 32.0], 1, "x1", Some(0.158942508893)),
    (0, &[52.0, 7.0, 1.0], 1, "", None),
    (0, &[18.0, 41.0, 31.0], 2, "g", Some(0.0603481867284)),
    (2, &[14.0, 6.0, 2.0], 1, "x1", Some(0.0541666666667)),
    (2, &[4.0, 35.0, 29.0], 2, "x1", Some(0.0212108330278)),
    (3, &[14.0, 0.0, 0.0], 1, "", None),
    (3, &[0.0, 6.0, 2.0], 2, "", None),
    (4, &[3.0, 30.0, 16.0], 2, "x2", Some(0.0132459208412)),
    (4, &[1.0, 5.0, 13.0], 3, "", None),
    (7, &[1.0, 21.0, 6.0], 2, "", None),
    (7, &[2.0, 9.0, 10.0], 3, "o", Some(0.00938418571064)),
    (10, &[2.0, 6.0, 3.0], 2, "", None),
    (10, &[0.0, 3.0, 7.0], 3, "", None),
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

/// Runs a CRT tree and compares its tree table with `expected`.
fn grown(command: &str, classes: usize, expected: &[Node]) -> (Engine, Vec<Table>) {
    grown_printing(command, "", classes, expected)
}

fn grown_printing(
    command: &str,
    more: &str,
    classes: usize,
    expected: &[Node],
) -> (Engine, Vec<Table>) {
    let mut engine = engine();
    let source = format!("{command} /PRINT MODELSUMMARY RISK TREETABLE{more} /TREE DISPLAY=NONE.");
    let tables = run(&mut engine, &source);
    let tree = table(&tables, "Tree Table");
    assert_eq!(tree.rows.len(), expected.len(), "{command}");
    let (parent, variable, gain, predicted) = (
        column(tree, "Parent Node"),
        column(tree, "Variable"),
        column(tree, "Improvement"),
        column(tree, "Predicted Category"),
    );
    for (id, (row, want)) in tree.rows.iter().zip(expected).enumerate() {
        if id > 0 {
            assert_eq!(number(&row[parent]), want.0 as f64, "parent of node {id}");
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
        assert_eq!(text(&row[variable]), want.3, "split variable of node {id}");
        match want.4 {
            Some(g) => close(number(&row[gain]), g, 1e-9),
            None => assert!(matches!(row[gain], Cell::Empty), "node {id} splits"),
        }
    }
    (engine, tables)
}

fn risk(tables: &[Table]) -> (f64, f64) {
    let risk = table(tables, "Risk");
    (number(&risk.rows[0][0]), number(&risk.rows[0][1]))
}

#[test]
fn gini_on_scale_and_ordinal_predictors() {
    let (engine, tables) = grown(
        "TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        2,
        EXPECTED_A,
    );
    // the share misclassified: 41 of 150
    let (estimate, error) = risk(&tables);
    close(estimate, 41.0 / 150.0, 1e-12);
    close(
        error,
        (41.0 / 150.0 * (1.0 - 41.0 / 150.0) / 150.0_f64).sqrt(),
        1e-12,
    );
    // the cut is a value of the data
    let tree = table(&tables, "Tree Table");
    assert_eq!(text(&tree.rows[1][2]), "x1 = <= 33.7");
    assert_eq!(text(&tree.rows[2][2]), "x1 = > 33.7");
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
    let summary = table(&tables, "Model Summary");
    assert!(summary.rows.iter().any(|r| text(&r[2]) == "CRT"));
}

#[test]
fn gini_with_a_nominal_predictor_of_two_classes() {
    grown(
        "TREE y [n] BY x1 [s] g [n] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        2,
        EXPECTED_B,
    );
}

#[test]
fn twoing_lists_the_subsets_of_a_nominal_predictor_for_three_classes() {
    let (_, tables) = grown(
        "TREE y3 [n] BY x2 [s] o [o] g [n] /METHOD TYPE=CRT /CRT IMPURITY=TWOING /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        3,
        EXPECTED_C,
    );
    let (estimate, error) = risk(&tables);
    close(estimate, 58.0 / 150.0, 1e-12);
    close(error, 0.03976225643794674, 1e-9);
    let tree = table(&tables, "Tree Table");
    assert_eq!(text(&tree.rows[1][2]), "g = 1");
    assert_eq!(text(&tree.rows[7][2]), "g = 2; 3");
}

#[test]
fn ordered_twoing_with_equal_priors() {
    let (_, tables) = grown(
        "TREE y3 [o] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT /CRT IMPURITY=ORDEREDTWOING /PRIORS EQUAL /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        3,
        EXPECTED_D,
    );
    // each class's share misclassified, weighted by its prior
    let (estimate, error) = risk(&tables);
    close(estimate, 0.33918650793650795, 1e-9);
    close(error, 0.03835738882534424, 1e-9);
}

#[test]
fn custom_priors_are_normalized_and_drive_assignment_and_saved_probabilities() {
    let mut command = String::from(
        "TREE y3 [n] BY x1 [s] o [o] g [n] /METHOD TYPE=CRT /PRIORS CUSTOM=1 [5] 2 [3] 3 [2] /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
    );
    let (_, tables) = grown(&command, 3, EXPECTED_E);
    let (estimate, error) = risk(&tables);
    close(estimate, 0.2598214285714286, 1e-9);
    close(error, 0.0305513767602687, 1e-9);
    // node 2 holds 18, 41 and 31 cases; with these priors class 2 is its assignment
    command.push_str(" /SAVE NODEID(node) PREDPROB(p)");
    let mut engine = engine();
    run(&mut engine, &format!("{command}."));
    let data = engine.dataset().expect("data");
    let values = |name: &str| -> Vec<Option<f64>> {
        data.variable(name)
            .unwrap()
            .numeric()
            .unwrap()
            .iter()
            .collect()
    };
    let node = values("node");
    // a case of node 5: counts 14, 6 and 2, class totals 70, 48 and 32
    let row = node
        .iter()
        .position(|&n| n == Some(5.0))
        .expect("a case in node 5");
    let want = [0.5 * 14.0 / 70.0, 0.3 * 6.0 / 48.0, 0.2 * 2.0 / 32.0];
    let sum: f64 = want.iter().sum();
    for (j, expected) in want.iter().enumerate() {
        let p = values(&format!("p_{}", j + 1));
        close(p[row].unwrap(), expected / sum, 1e-12);
    }
}

#[test]
fn a_child_below_the_minimum_size_is_not_offered() {
    // the best cut of node 1 on x2 leaves 22 cases on one side, so the best cut with 30 or more
    // on both sides is taken
    grown(
        "TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=31 MINCHILDSIZE=30",
        2,
        EXPECTED_G,
    );
}

#[test]
fn a_split_below_the_minimum_improvement_is_not_made() {
    grown(
        "TREE y [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12 /CRT MINIMPROVEMENT=.01",
        2,
        EXPECTED_H,
    );
}

#[test]
fn a_missing_split_value_goes_to_the_larger_child_without_surrogates() {
    let (engine, _) = grown(
        "TREE y [n] BY xm [s] o [o] /METHOD TYPE=CRT MAXSURROGATES=0 /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        2,
        EXPECTED_F,
    );
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
}

/// Runs a tree with surrogates and compares the tree table and the Surrogates table.
fn with_surrogates(
    command: &str,
    classes: usize,
    nodes: &[Node],
    surrogates: &[(usize, &str, f64)],
) {
    let (_, tables) = grown_printing(command, " SURROGATES", classes, nodes);
    let found = table(&tables, "Surrogates");
    assert_eq!(found.rows.len(), surrogates.len(), "{command}");
    for (row, (node, variable, association)) in found.rows.iter().zip(surrogates) {
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
fn surrogates_route_cases_with_a_missing_split_value() {
    // z, which follows xm closely, is the root's first surrogate; a case with neither value
    // goes by the next surrogate it has, or by the majority
    with_surrogates(
        "TREE y [n] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        2,
        EXPECTED_S1,
        SURROGATES_S1,
    );
}

#[test]
fn the_number_of_surrogates_is_limited() {
    with_surrogates(
        "TREE y [n] BY xm [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT MAXSURROGATES=1 /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        2,
        EXPECTED_S2,
        SURROGATES_S2,
    );
}

#[test]
fn surrogates_with_priors_and_a_nominal_predictor() {
    with_surrogates(
        "TREE y3 [n] BY xm [s] z [s] o [o] g [n] /METHOD TYPE=CRT MAXSURROGATES=2 /PRIORS CUSTOM=1 [5] 2 [3] 3 [2] /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
        3,
        EXPECTED_S3,
        SURROGATES_S3,
    );
}

#[test]
fn what_cannot_be_asked_says_so() {
    let mut engine = engine();
    let message = error(
        &mut engine,
        "TREE y3 [n] BY x1 [s] /METHOD TYPE=CRT /CRT IMPURITY=ORDEREDTWOING.",
    );
    assert!(message.contains("ordinal dependent"), "{message}");
    let message = error(&mut engine, "TREE x1 [s] BY o [o] /METHOD TYPE=CRT.");
    assert!(message.contains("scale dependent variable"), "{message}");
    let message = error(
        &mut engine,
        "TREE y3 [n] BY x1 [s] /METHOD TYPE=CRT /PRIORS CUSTOM=1 [1] 2 [1].",
    );
    assert!(
        message.contains("no probability for category 3"),
        "{message}"
    );
}

#[test]
fn settings_of_another_method_are_ignored_with_a_warning() {
    let mut engine = engine();
    run(
        &mut engine,
        "TREE y [n] BY x1 [s] o [o] /PRIORS EQUAL /CRT IMPURITY=TWOING /METHOD MAXSURROGATES=2 /TREE DISPLAY=NONE.",
    );
    let warnings = engine.warnings().join("\n");
    for what in ["CRT", "PRIORS", "METHOD MAXSURROGATES"] {
        assert!(warnings.contains(what), "{what}: {warnings}");
    }
    // and CRT ignores what belongs to CHAID
    let mut engine = self::engine();
    run(
        &mut engine,
        "TREE y [n] BY x1 [s] FORCE = o [o] /METHOD TYPE=CRT /CHAID ALPHASPLIT=.01 /SCORES EQUALINCREMENTS /TREE DISPLAY=NONE.",
    );
    let warnings = engine.warnings().join("\n");
    for what in ["CHAID applies", "SCORES applies", "FORCE applies"] {
        assert!(warnings.contains(what), "{what}: {warnings}");
    }
}

const PRUNING: &str = " /GROWTHLIMIT MINPARENTSIZE=8 MINCHILDSIZE=4";

#[test]
fn pruning_keeps_the_smallest_tree_within_the_standard_errors() {
    // the grown tree has 35 nodes; its weakest-link sequence has risks 0.2067 (for 18 and 10
    // leaves), 0.22, 0.2333, 0.2733 and 0.4133 (rpart's cost-complexity table agrees)
    let base = "TREE y [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT";
    let (_, tables) = grown(&format!("{base} PRUNE=SE(0){PRUNING}"), 2, EXPECTED_P1_B0);
    // nothing is lost: the pruned tree misclassifies as many cases as the grown one
    close(risk(&tables).0, 31.0 / 150.0, 1e-12);
    grown(&format!("{base} PRUNE=SE(.5){PRUNING}"), 2, EXPECTED_P1_B05);
    // SE without a number is one standard error
    let (_, tables) = grown(&format!("{base} PRUNE=SE{PRUNING}"), 2, EXPECTED_P1_B1);
    let summary = table(&tables, "Model Summary");
    assert!(summary.rows.iter().any(|r| text(&r[1]) == "Pruning"));
}

#[test]
fn pruning_with_priors_uses_the_weighted_risk() {
    let priors = " /PRIORS CUSTOM=1 [5] 2 [3] 3 [2]";
    let base = "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT";
    grown(
        &format!("{base} PRUNE=SE(0){priors}{PRUNING}"),
        3,
        EXPECTED_P2_B0,
    );
    grown(
        &format!("{base} PRUNE=SE(1){priors}{PRUNING}"),
        3,
        EXPECTED_P2_B1,
    );
}

/// A predictor, its importance and the importance relative to the largest.
type Importance = (&'static str, f64, f64);

/// The importance table: variable, importance and normalized importance, largest first.
fn importance(command: &str, extra: &str) -> (Engine, Vec<(String, f64, f64)>) {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!("{command} /PRINT RISK{extra} /TREE DISPLAY=NONE."),
    );
    let found = table(&tables, "Independent Variable Importance");
    let rows = found
        .rows
        .iter()
        .map(|r| (text(&r[0]).to_string(), number(&r[1]), number(&r[2])))
        .collect();
    (engine, rows)
}

#[test]
fn importance_adds_the_improvements_of_main_and_surrogate_splits() {
    // three scenarios of the R implementation: all surrogates, two surrogates with priors on
    // three classes, and a pruned tree (the cut splits count for nothing)
    let cases: [(&str, &[Importance]); 3] = [
        (
            "TREE y [n] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
            &[
                ("x2", 0.0803362942517, 100.0),
                ("xm", 0.0620704475419, 77.2632695098),
                ("z", 0.0535388659953, 66.6434349431),
                ("g", 0.0261889519748, 32.5991536188),
            ],
        ),
        (
            "TREE y3 [n] BY xm [s] z [s] o [o] g [n] /METHOD TYPE=CRT MAXSURROGATES=2 /PRIORS CUSTOM=1 [5] 2 [3] 3 [2] /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12",
            &[
                ("xm", 0.202038920557, 100.0),
                ("z", 0.173229626924, 85.74072087),
                ("g", 0.0681865160131, 33.7491983351),
                ("o", 0.00166980539862, 0.826477093627),
            ],
        ),
        (
            "TREE y [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT PRUNE=SE /GROWTHLIMIT MINPARENTSIZE=8 MINCHILDSIZE=4",
            &[
                ("x2", 0.0616949706965, 100.0),
                ("x1", 0.0556174785447, 90.1491287164),
                ("g", 0.0259981208998, 42.1397734796),
                ("o", 0.00528937728938, 8.57343350627),
            ],
        ),
    ];
    for (command, expected) in cases {
        let (_, found) = importance(command, " IMPORTANCE");
        assert_eq!(found.len(), expected.len(), "{command}");
        for ((name, value, normalized), (want_name, want, want_normalized)) in
            found.iter().zip(expected)
        {
            assert_eq!(name, want_name, "{command}");
            close(*value, *want, 1e-9);
            close(*normalized, *want_normalized, 1e-9);
        }
    }
}

#[test]
fn the_importance_chart_has_one_bar_per_variable() {
    let (mut engine, found) = importance(
        "TREE y [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT PRUNE=SE /GROWTHLIMIT MINPARENTSIZE=8 MINCHILDSIZE=4",
        " IMPORTANCE /PLOT IMPORTANCE",
    );
    let charts = engine.take_charts();
    let chart = charts
        .iter()
        .find(|c| c.title.contains("importance"))
        .expect("a chart");
    let labels: Vec<&str> = chart.categories.iter().map(|c| c.label.as_str()).collect();
    let names: Vec<&str> = found.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(labels, names);
    close(chart.series[0].values[1], 90.1491287164, 1e-9);
}
