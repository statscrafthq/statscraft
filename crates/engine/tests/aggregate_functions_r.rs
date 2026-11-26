//! Every function of AGGREGATE against R on 48 invented cases in six break groups (the fifth
//! holds only missing values, the sixth a single case), with case weights, the code -9 declared
//! user-missing in the source variable, and a string variable: SUM, MEAN, MEDIAN, SD, MAX, MIN,
//! FIRST, LAST, the percentage, fraction and count above, below, inside and outside bounds
//! (also bounds written the wrong way round, and two equal bounds, where inside means equal and
//! outside means not equal), N and NU with and without a variable, NMISS and NUMISS. Six runs: as
//! the data are, weighted (R is run on the cases written out as often as their weight), with the
//! period after each function name (user-missing values count as valid), weighted with the
//! period, with MISSING=COLUMNWISE (a break group with a missing value in its source variable
//! has none; the counts are not affected), and weighted columnwise with the period. The strings
//! run uses R's character order in the C locale. R gives the weighted median, the frequency
//! weighted standard deviation (divisor W - 1, none when W is 1 or less), the first and last
//! value the function uses, and the percentages of the weighted valid cases.

use statscraft_engine::Engine;
use statscraft_syntax::parse;

/// System-missing in a table.
const NAN: f64 = f64::NAN;

const DATA: &str = "DATA LIST FREE /g w x (F1 F1 F8.1) s (A8).
BEGIN DATA
1 1 22.1 date
1 4 -9.0 lime
1 3 17.9 plum
1 1 . plum
1 2 17.8 lime
1 4 22.6 date
1 2 15.7 pear
1 2 13.5 zz
1 4 24.7 lime
1 1 23.1 fig
1 3 20.9 plum
2 1 -9.0 lime
2 1 20.4 lime
2 4 27.3 date
2 1 . grape
2 3 16.1 plum
2 2 19.6 Cherry
2 4 26.2 Cherry
2 1 -9.0 plum
2 2 16.0 apple
3 3 28.1 zz
3 2 . plum
3 4 21.9 date
3 1 21.2 apple
3 1 25.5 plum
3 1 15.2 Cherry
3 2 -9.0 date
3 2 19.8 apple
3 4  9.3 kiwi
3 3 17.8 zz
3 3 26.5 kiwi
3 1 19.5 fig
3 4 -9.0 fig
4 3 14.2 apple
4 4 21.4 plum
4 3 -9.0 pear
4 4 18.6 apple
4 2 21.9 date
4 1 25.7 fig
4 4 . kiwi
4 4 24.1 plum
4 4 26.4 kiwi
4 4 18.5 fig
5 2 -9.0 grape
5 4 . date
5 1 -9.0 kiwi
5 4 . pear
6 2 15.0 pear
END DATA.
MISSING VALUES x (-9).
";

/// The expected value of each target in the six break groups.
type Expected = &'static [(&'static str, [f64; 6])];

/// A run: its name, weighted, columnwise, the functions, and what they give.
type Run = (&'static str, bool, bool, &'static str, Expected);

const RUNS: &[Run] = &[
    (
        "default",
        false,
        false,
        "/t1=SUM(x) /t2=MEAN(x) /t3=MEDIAN(x) /t4=SD(x) /t5=MAX(x) /t6=MIN(x) /t7=FIRST(x) /t8=LAST(x) /t9=PGT(x,20) /t10=PLT(x,20) /t11=PIN(x,15,25) /t12=POUT(x,15,25) /t13=FGT(x,20) /t14=FLT(x,20) /t15=FIN(x,15,25) /t16=FOUT(x,15,25) /t17=CGT(x,20) /t18=CLT(x,20) /t19=CIN(x,15,25) /t20=COUT(x,15,25) /t21=PIN(x,25,15) /t22=POUT(x,25,15) /t23=CIN(x,20.5,20.5) /t24=COUT(x,20.5,20.5) /t25=N /t26=NU /t27=N(x) /t28=NU(x) /t29=NMISS(x) /t30=NUMISS(x)",
        &[
            (
                "t1",
                [1.783e+02, 1.256e+02, 2.048e+02, 1.708e+02, NAN, 15.0],
            ),
            (
                "t2",
                [
                    1.98111111111111e+01,
                    2.09333333333333e+01,
                    2.048e+01,
                    2.135e+01,
                    NAN,
                    15.0,
                ],
            ),
            ("t3", [2.09e+01, 20.0, 2.05e+01, 2.165e+01, NAN, 15.0]),
            (
                "t4",
                [
                    3.76412155902424e+00,
                    4.85866922795395e+00,
                    5.61264445495862e+00,
                    4.12483765914317e+00,
                    NAN,
                    NAN,
                ],
            ),
            ("t5", [2.47e+01, 2.73e+01, 2.81e+01, 2.64e+01, NAN, 15.0]),
            ("t6", [1.35e+01, 16.0, 9.3e+00, 1.42e+01, NAN, 15.0]),
            ("t7", [2.21e+01, 2.04e+01, 2.81e+01, 1.42e+01, NAN, 15.0]),
            ("t8", [2.09e+01, 16.0, 1.95e+01, 1.85e+01, NAN, 15.0]),
            ("t9", [5.55555555555556e+01, 50.0, 50.0, 6.25e+01, NAN, 0.0]),
            (
                "t10",
                [4.44444444444444e+01, 50.0, 50.0, 3.75e+01, NAN, 100.0],
            ),
            (
                "t11",
                [
                    8.88888888888889e+01,
                    6.66666666666667e+01,
                    60.0,
                    6.25e+01,
                    NAN,
                    100.0,
                ],
            ),
            (
                "t12",
                [
                    1.11111111111111e+01,
                    3.33333333333333e+01,
                    40.0,
                    3.75e+01,
                    NAN,
                    0.0,
                ],
            ),
            (
                "t13",
                [5.55555555555556e-01, 5e-01, 5e-01, 6.25e-01, NAN, 0.0],
            ),
            (
                "t14",
                [4.44444444444444e-01, 5e-01, 5e-01, 3.75e-01, NAN, 1.0],
            ),
            (
                "t15",
                [
                    8.88888888888889e-01,
                    6.66666666666667e-01,
                    6e-01,
                    6.25e-01,
                    NAN,
                    1.0,
                ],
            ),
            (
                "t16",
                [
                    1.11111111111111e-01,
                    3.33333333333333e-01,
                    4e-01,
                    3.75e-01,
                    NAN,
                    0.0,
                ],
            ),
            ("t17", [5.0, 3.0, 5.0, 5.0, 0.0, 0.0]),
            ("t18", [4.0, 3.0, 5.0, 3.0, 0.0, 1.0]),
            ("t19", [8.0, 4.0, 6.0, 5.0, 0.0, 1.0]),
            ("t20", [1.0, 2.0, 4.0, 3.0, 0.0, 0.0]),
            (
                "t21",
                [
                    8.88888888888889e+01,
                    6.66666666666667e+01,
                    60.0,
                    6.25e+01,
                    NAN,
                    100.0,
                ],
            ),
            (
                "t22",
                [
                    1.11111111111111e+01,
                    3.33333333333333e+01,
                    40.0,
                    3.75e+01,
                    NAN,
                    0.0,
                ],
            ),
            ("t23", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ("t24", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t25", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t28", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t29", [2.0, 3.0, 3.0, 2.0, 4.0, 0.0]),
            ("t30", [2.0, 3.0, 3.0, 2.0, 4.0, 0.0]),
        ],
    ),
    (
        "weighted",
        true,
        false,
        "/t1=SUM(x) /t2=MEAN(x) /t3=MEDIAN(x) /t4=SD(x) /t5=MAX(x) /t6=MIN(x) /t7=FIRST(x) /t8=LAST(x) /t9=PGT(x,20) /t10=PLT(x,20) /t11=PIN(x,15,25) /t12=POUT(x,15,25) /t13=FGT(x,20) /t14=FLT(x,20) /t15=FIN(x,15,25) /t16=FOUT(x,15,25) /t17=CGT(x,20) /t18=CLT(x,20) /t19=CIN(x,15,25) /t20=COUT(x,15,25) /t21=PIN(x,25,15) /t22=POUT(x,25,15) /t23=CIN(x,20.5,20.5) /t24=COUT(x,20.5,20.5) /t25=N /t26=NU /t27=N(x) /t28=NU(x) /t29=NMISS(x) /t30=NUMISS(x)",
        &[
            ("t1", [4.448e+02, 3.539e+02, 463.0, 5.481e+02, NAN, 30.0]),
            (
                "t2",
                [
                    2.02181818181818e+01,
                    2.211875e+01,
                    2.01304347826087e+01,
                    2.10807692307692e+01,
                    NAN,
                    15.0,
                ],
            ),
            ("t3", [2.09e+01, 2.33e+01, 2.12e+01, 2.14e+01, NAN, 15.0]),
            (
                "t4",
                [
                    3.62000215254295e+00,
                    4.98801480216996e+00,
                    6.27103259458692e+00,
                    3.82633184454974e+00,
                    NAN,
                    0.0,
                ],
            ),
            ("t5", [2.47e+01, 2.73e+01, 2.81e+01, 2.64e+01, NAN, 15.0]),
            ("t6", [1.35e+01, 16.0, 9.3e+00, 1.42e+01, NAN, 15.0]),
            ("t7", [2.21e+01, 2.04e+01, 2.81e+01, 1.42e+01, NAN, 15.0]),
            ("t8", [2.09e+01, 16.0, 1.95e+01, 1.85e+01, NAN, 15.0]),
            (
                "t9",
                [
                    5.90909090909091e+01,
                    5.625e+01,
                    5.21739130434783e+01,
                    5.76923076923077e+01,
                    NAN,
                    0.0,
                ],
            ),
            (
                "t10",
                [
                    4.09090909090909e+01,
                    4.375e+01,
                    4.78260869565217e+01,
                    4.23076923076923e+01,
                    NAN,
                    100.0,
                ],
            ),
            (
                "t11",
                [
                    9.09090909090909e+01,
                    50.0,
                    5.21739130434783e+01,
                    6.92307692307692e+01,
                    NAN,
                    100.0,
                ],
            ),
            (
                "t12",
                [
                    9.09090909090909e+00,
                    50.0,
                    4.78260869565217e+01,
                    3.07692307692308e+01,
                    NAN,
                    0.0,
                ],
            ),
            (
                "t13",
                [
                    5.90909090909091e-01,
                    5.625e-01,
                    5.21739130434783e-01,
                    5.76923076923077e-01,
                    NAN,
                    0.0,
                ],
            ),
            (
                "t14",
                [
                    4.09090909090909e-01,
                    4.375e-01,
                    4.78260869565217e-01,
                    4.23076923076923e-01,
                    NAN,
                    1.0,
                ],
            ),
            (
                "t15",
                [
                    9.09090909090909e-01,
                    5e-01,
                    5.21739130434783e-01,
                    6.92307692307692e-01,
                    NAN,
                    1.0,
                ],
            ),
            (
                "t16",
                [
                    9.09090909090909e-02,
                    5e-01,
                    4.78260869565217e-01,
                    3.07692307692308e-01,
                    NAN,
                    0.0,
                ],
            ),
            ("t17", [13.0, 9.0, 12.0, 15.0, 0.0, 0.0]),
            ("t18", [9.0, 7.0, 11.0, 11.0, 0.0, 2.0]),
            ("t19", [20.0, 8.0, 12.0, 18.0, 0.0, 2.0]),
            ("t20", [2.0, 8.0, 11.0, 8.0, 0.0, 0.0]),
            (
                "t21",
                [
                    9.09090909090909e+01,
                    50.0,
                    5.21739130434783e+01,
                    6.92307692307692e+01,
                    NAN,
                    100.0,
                ],
            ),
            (
                "t22",
                [
                    9.09090909090909e+00,
                    50.0,
                    4.78260869565217e+01,
                    3.07692307692308e+01,
                    NAN,
                    0.0,
                ],
            ),
            ("t23", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ("t24", [22.0, 16.0, 23.0, 26.0, 0.0, 2.0]),
            ("t25", [27.0, 19.0, 31.0, 33.0, 11.0, 2.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [22.0, 16.0, 23.0, 26.0, 0.0, 2.0]),
            ("t28", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t29", [5.0, 3.0, 8.0, 7.0, 11.0, 0.0]),
            ("t30", [2.0, 3.0, 3.0, 2.0, 4.0, 0.0]),
        ],
    ),
    (
        "period",
        false,
        false,
        "/t1=SUM.(x) /t2=MEAN.(x) /t3=MEDIAN.(x) /t4=SD.(x) /t5=MAX.(x) /t6=MIN.(x) /t7=FIRST.(x) /t8=LAST.(x) /t9=PGT.(x,20) /t10=PLT.(x,20) /t11=PIN.(x,15,25) /t12=POUT.(x,15,25) /t13=FGT.(x,20) /t14=FLT.(x,20) /t15=FIN.(x,15,25) /t16=FOUT.(x,15,25) /t17=CGT.(x,20) /t18=CLT.(x,20) /t19=CIN.(x,15,25) /t20=COUT.(x,15,25) /t21=PIN.(x,25,15) /t22=POUT.(x,25,15) /t23=CIN.(x,20.5,20.5) /t24=COUT.(x,20.5,20.5) /t25=N. /t26=NU. /t27=N.(x) /t28=NU.(x) /t29=NMISS.(x) /t30=NUMISS.(x)",
        &[
            (
                "t1",
                [1.693e+02, 1.076e+02, 1.868e+02, 1.618e+02, -18.0, 15.0],
            ),
            (
                "t2",
                [
                    1.693e+01,
                    1.345e+01,
                    1.55666666666667e+01,
                    1.79777777777778e+01,
                    -9.0,
                    15.0,
                ],
            ),
            ("t3", [1.94e+01, 1.785e+01, 1.965e+01, 2.14e+01, -9.0, 15.0]),
            (
                "t4",
                [
                    9.77764456979969e+00,
                    1.44520883315477e+01,
                    1.25479685667249e+01,
                    1.08274856012116e+01,
                    0.0,
                    NAN,
                ],
            ),
            ("t5", [2.47e+01, 2.73e+01, 2.81e+01, 2.64e+01, -9.0, 15.0]),
            ("t6", [-9.0, -9.0, -9.0, -9.0, -9.0, 15.0]),
            ("t7", [2.21e+01, -9.0, 2.81e+01, 1.42e+01, -9.0, 15.0]),
            ("t8", [2.09e+01, 16.0, -9.0, 1.85e+01, -9.0, 15.0]),
            (
                "t9",
                [
                    50.0,
                    3.75e+01,
                    4.16666666666667e+01,
                    5.55555555555556e+01,
                    0.0,
                    0.0,
                ],
            ),
            (
                "t10",
                [
                    50.0,
                    6.25e+01,
                    5.83333333333333e+01,
                    4.44444444444444e+01,
                    100.0,
                    100.0,
                ],
            ),
            ("t11", [80.0, 50.0, 50.0, 5.55555555555556e+01, 0.0, 100.0]),
            ("t12", [20.0, 50.0, 50.0, 4.44444444444444e+01, 100.0, 0.0]),
            (
                "t13",
                [
                    5e-01,
                    3.75e-01,
                    4.16666666666667e-01,
                    5.55555555555556e-01,
                    0.0,
                    0.0,
                ],
            ),
            (
                "t14",
                [
                    5e-01,
                    6.25e-01,
                    5.83333333333333e-01,
                    4.44444444444444e-01,
                    1.0,
                    1.0,
                ],
            ),
            ("t15", [8e-01, 5e-01, 5e-01, 5.55555555555556e-01, 0.0, 1.0]),
            ("t16", [2e-01, 5e-01, 5e-01, 4.44444444444444e-01, 1.0, 0.0]),
            ("t17", [5.0, 3.0, 5.0, 5.0, 0.0, 0.0]),
            ("t18", [5.0, 5.0, 7.0, 4.0, 2.0, 1.0]),
            ("t19", [8.0, 4.0, 6.0, 5.0, 0.0, 1.0]),
            ("t20", [2.0, 4.0, 6.0, 4.0, 2.0, 0.0]),
            ("t21", [80.0, 50.0, 50.0, 5.55555555555556e+01, 0.0, 100.0]),
            ("t22", [20.0, 50.0, 50.0, 4.44444444444444e+01, 100.0, 0.0]),
            ("t23", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ("t24", [10.0, 8.0, 12.0, 9.0, 2.0, 1.0]),
            ("t25", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [10.0, 8.0, 12.0, 9.0, 2.0, 1.0]),
            ("t28", [10.0, 8.0, 12.0, 9.0, 2.0, 1.0]),
            ("t29", [1.0, 1.0, 1.0, 1.0, 2.0, 0.0]),
            ("t30", [1.0, 1.0, 1.0, 1.0, 2.0, 0.0]),
        ],
    ),
    (
        "weighted period",
        true,
        false,
        "/t1=SUM.(x) /t2=MEAN.(x) /t3=MEDIAN.(x) /t4=SD.(x) /t5=MAX.(x) /t6=MIN.(x) /t7=FIRST.(x) /t8=LAST.(x) /t9=PGT.(x,20) /t10=PLT.(x,20) /t11=PIN.(x,15,25) /t12=POUT.(x,15,25) /t13=FGT.(x,20) /t14=FLT.(x,20) /t15=FIN.(x,15,25) /t16=FOUT.(x,15,25) /t17=CGT.(x,20) /t18=CLT.(x,20) /t19=CIN.(x,15,25) /t20=COUT.(x,15,25) /t21=PIN.(x,25,15) /t22=POUT.(x,25,15) /t23=CIN.(x,20.5,20.5) /t24=COUT.(x,20.5,20.5) /t25=N. /t26=NU. /t27=N.(x) /t28=NU.(x) /t29=NMISS.(x) /t30=NUMISS.(x)",
        &[
            ("t1", [4.088e+02, 3.359e+02, 409.0, 5.211e+02, -27.0, 30.0]),
            (
                "t2",
                [
                    1.57230769230769e+01,
                    1.86611111111111e+01,
                    1.41034482758621e+01,
                    1.79689655172414e+01,
                    -9.0,
                    15.0,
                ],
            ),
            ("t3", [1.94e+01, 20.0, 1.95e+01, 2.14e+01, -9.0, 15.0]),
            (
                "t4",
                [
                    1.12510375589919e+01,
                    1.11005107931699e+01,
                    1.32331424504273e+01,
                    9.99957511412631e+00,
                    0.0,
                    0.0,
                ],
            ),
            ("t5", [2.47e+01, 2.73e+01, 2.81e+01, 2.64e+01, -9.0, 15.0]),
            ("t6", [-9.0, -9.0, -9.0, -9.0, -9.0, 15.0]),
            ("t7", [2.21e+01, -9.0, 2.81e+01, 1.42e+01, -9.0, 15.0]),
            ("t8", [2.09e+01, 16.0, -9.0, 1.85e+01, -9.0, 15.0]),
            (
                "t9",
                [
                    50.0,
                    50.0,
                    4.13793103448276e+01,
                    5.17241379310345e+01,
                    0.0,
                    0.0,
                ],
            ),
            (
                "t10",
                [
                    50.0,
                    50.0,
                    5.86206896551724e+01,
                    4.82758620689655e+01,
                    100.0,
                    100.0,
                ],
            ),
            (
                "t11",
                [
                    7.69230769230769e+01,
                    4.44444444444444e+01,
                    4.13793103448276e+01,
                    6.20689655172414e+01,
                    0.0,
                    100.0,
                ],
            ),
            (
                "t12",
                [
                    2.30769230769231e+01,
                    5.55555555555556e+01,
                    5.86206896551724e+01,
                    3.79310344827586e+01,
                    100.0,
                    0.0,
                ],
            ),
            (
                "t13",
                [
                    5e-01,
                    5e-01,
                    4.13793103448276e-01,
                    5.17241379310345e-01,
                    0.0,
                    0.0,
                ],
            ),
            (
                "t14",
                [
                    5e-01,
                    5e-01,
                    5.86206896551724e-01,
                    4.82758620689655e-01,
                    1.0,
                    1.0,
                ],
            ),
            (
                "t15",
                [
                    7.69230769230769e-01,
                    4.44444444444444e-01,
                    4.13793103448276e-01,
                    6.20689655172414e-01,
                    0.0,
                    1.0,
                ],
            ),
            (
                "t16",
                [
                    2.30769230769231e-01,
                    5.55555555555556e-01,
                    5.86206896551724e-01,
                    3.79310344827586e-01,
                    1.0,
                    0.0,
                ],
            ),
            ("t17", [13.0, 9.0, 12.0, 15.0, 0.0, 0.0]),
            ("t18", [13.0, 9.0, 17.0, 14.0, 3.0, 2.0]),
            ("t19", [20.0, 8.0, 12.0, 18.0, 0.0, 2.0]),
            ("t20", [6.0, 10.0, 17.0, 11.0, 3.0, 0.0]),
            (
                "t21",
                [
                    7.69230769230769e+01,
                    4.44444444444444e+01,
                    4.13793103448276e+01,
                    6.20689655172414e+01,
                    0.0,
                    100.0,
                ],
            ),
            (
                "t22",
                [
                    2.30769230769231e+01,
                    5.55555555555556e+01,
                    5.86206896551724e+01,
                    3.79310344827586e+01,
                    100.0,
                    0.0,
                ],
            ),
            ("t23", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ("t24", [26.0, 18.0, 29.0, 29.0, 3.0, 2.0]),
            ("t25", [27.0, 19.0, 31.0, 33.0, 11.0, 2.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [26.0, 18.0, 29.0, 29.0, 3.0, 2.0]),
            ("t28", [10.0, 8.0, 12.0, 9.0, 2.0, 1.0]),
            ("t29", [1.0, 1.0, 2.0, 4.0, 8.0, 0.0]),
            ("t30", [1.0, 1.0, 1.0, 1.0, 2.0, 0.0]),
        ],
    ),
    (
        "columnwise",
        false,
        true,
        "/t1=SUM(x) /t2=MEAN(x) /t3=MEDIAN(x) /t4=SD(x) /t5=MAX(x) /t6=MIN(x) /t7=FIRST(x) /t8=LAST(x) /t9=PGT(x,20) /t10=PLT(x,20) /t11=PIN(x,15,25) /t12=POUT(x,15,25) /t13=FGT(x,20) /t14=FLT(x,20) /t15=FIN(x,15,25) /t16=FOUT(x,15,25) /t17=CGT(x,20) /t18=CLT(x,20) /t19=CIN(x,15,25) /t20=COUT(x,15,25) /t21=PIN(x,25,15) /t22=POUT(x,25,15) /t23=CIN(x,20.5,20.5) /t24=COUT(x,20.5,20.5) /t25=N /t26=NU /t27=N(x) /t28=NU(x) /t29=NMISS(x) /t30=NUMISS(x)",
        &[
            ("t1", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t2", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t3", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t4", [NAN, NAN, NAN, NAN, NAN, NAN]),
            ("t5", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t6", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t7", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t8", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t9", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t10", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t11", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t12", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t13", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t14", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t15", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t16", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t17", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t18", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t19", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t20", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t21", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t22", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t23", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t24", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t25", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t28", [9.0, 6.0, 10.0, 8.0, 0.0, 1.0]),
            ("t29", [2.0, 3.0, 3.0, 2.0, 4.0, 0.0]),
            ("t30", [2.0, 3.0, 3.0, 2.0, 4.0, 0.0]),
        ],
    ),
    (
        "weighted columnwise period",
        true,
        true,
        "/t1=SUM.(x) /t2=MEAN.(x) /t3=MEDIAN.(x) /t4=SD.(x) /t5=MAX.(x) /t6=MIN.(x) /t7=FIRST.(x) /t8=LAST.(x) /t9=PGT.(x,20) /t10=PLT.(x,20) /t11=PIN.(x,15,25) /t12=POUT.(x,15,25) /t13=FGT.(x,20) /t14=FLT.(x,20) /t15=FIN.(x,15,25) /t16=FOUT.(x,15,25) /t17=CGT.(x,20) /t18=CLT.(x,20) /t19=CIN.(x,15,25) /t20=COUT.(x,15,25) /t21=PIN.(x,25,15) /t22=POUT.(x,25,15) /t23=CIN.(x,20.5,20.5) /t24=COUT.(x,20.5,20.5) /t25=N. /t26=NU. /t27=N.(x) /t28=NU.(x) /t29=NMISS.(x) /t30=NUMISS.(x)",
        &[
            ("t1", [NAN, NAN, NAN, NAN, NAN, 30.0]),
            ("t2", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t3", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t4", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t5", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t6", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t7", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t8", [NAN, NAN, NAN, NAN, NAN, 15.0]),
            ("t9", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t10", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t11", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t12", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t13", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t14", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t15", [NAN, NAN, NAN, NAN, NAN, 1.0]),
            ("t16", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t17", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t18", [NAN, NAN, NAN, NAN, NAN, 2.0]),
            ("t19", [NAN, NAN, NAN, NAN, NAN, 2.0]),
            ("t20", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t21", [NAN, NAN, NAN, NAN, NAN, 100.0]),
            ("t22", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t23", [NAN, NAN, NAN, NAN, NAN, 0.0]),
            ("t24", [NAN, NAN, NAN, NAN, NAN, 2.0]),
            ("t25", [27.0, 19.0, 31.0, 33.0, 11.0, 2.0]),
            ("t26", [11.0, 9.0, 13.0, 10.0, 4.0, 1.0]),
            ("t27", [26.0, 18.0, 29.0, 29.0, 3.0, 2.0]),
            ("t28", [10.0, 8.0, 12.0, 9.0, 2.0, 1.0]),
            ("t29", [1.0, 1.0, 2.0, 4.0, 8.0, 0.0]),
            ("t30", [1.0, 1.0, 1.0, 1.0, 2.0, 0.0]),
        ],
    ),
];

/// A run of the string functions: weighted, the functions, the first, last, largest and smallest
/// value of each group (joined with |), and the numbers the other functions give.
type StringRun = (bool, &'static str, [&'static str; 4], Expected);

const STRING_RUNS: &[StringRun] = &[
    (
        false,
        "/f=FIRST(s) /l=LAST(s) /mx=MAX(s) /mn=MIN(s) /pgt=PGT(s,'kiwi') /plt=PLT(s,'kiwi') /pin=PIN(s,'fig','plum') /pout=POUT(s,'fig','plum') /fgt=FGT(s,'kiwi') /cin=CIN(s,'fig','plum') /pinr=PIN(s,'plum','fig')",
        [
            "date|lime|zz|apple|grape|pear",
            "plum|apple|fig|fig|pear|pear",
            "zz|plum|zz|plum|pear|pear",
            "date|Cherry|Cherry|apple|date|pear",
        ],
        &[
            (
                "pgt",
                [
                    7.27272727272727e+01,
                    4.44444444444444e+01,
                    3.07692307692308e+01,
                    30.0,
                    25.0,
                    100.0,
                ],
            ),
            (
                "plt",
                [
                    2.72727272727273e+01,
                    5.55555555555556e+01,
                    5.38461538461538e+01,
                    50.0,
                    50.0,
                    0.0,
                ],
            ),
            (
                "pin",
                [
                    7.27272727272727e+01,
                    5.55555555555556e+01,
                    4.61538461538462e+01,
                    70.0,
                    75.0,
                    100.0,
                ],
            ),
            (
                "pout",
                [
                    2.72727272727273e+01,
                    4.44444444444444e+01,
                    5.38461538461538e+01,
                    30.0,
                    25.0,
                    0.0,
                ],
            ),
            (
                "fgt",
                [
                    7.27272727272727e-01,
                    4.44444444444444e-01,
                    3.07692307692308e-01,
                    3e-01,
                    2.5e-01,
                    1.0,
                ],
            ),
            ("cin", [8.0, 5.0, 6.0, 7.0, 3.0, 1.0]),
            (
                "pinr",
                [
                    7.27272727272727e+01,
                    5.55555555555556e+01,
                    4.61538461538462e+01,
                    70.0,
                    75.0,
                    100.0,
                ],
            ),
        ],
    ),
    (
        true,
        "/f=FIRST(s) /l=LAST(s) /mx=MAX(s) /mn=MIN(s) /pgt=PGT(s,'kiwi') /plt=PLT(s,'kiwi') /pin=PIN(s,'fig','plum') /pout=POUT(s,'fig','plum') /fgt=FGT(s,'kiwi') /cin=CIN(s,'fig','plum') /pinr=PIN(s,'plum','fig')",
        [
            "date|lime|zz|apple|grape|pear",
            "plum|apple|fig|fig|pear|pear",
            "zz|plum|zz|plum|pear|pear",
            "date|Cherry|Cherry|apple|date|pear",
        ],
        &[
            (
                "pgt",
                [
                    7.77777777777778e+01,
                    3.15789473684211e+01,
                    2.90322580645161e+01,
                    3.33333333333333e+01,
                    3.63636363636364e+01,
                    100.0,
                ],
            ),
            (
                "plt",
                [
                    2.22222222222222e+01,
                    6.84210526315789e+01,
                    4.83870967741936e+01,
                    4.24242424242424e+01,
                    5.45454545454545e+01,
                    0.0,
                ],
            ),
            (
                "pin",
                [
                    7.40740740740741e+01,
                    3.68421052631579e+01,
                    4.83870967741936e+01,
                    7.27272727272727e+01,
                    6.36363636363636e+01,
                    100.0,
                ],
            ),
            (
                "pout",
                [
                    2.59259259259259e+01,
                    6.31578947368421e+01,
                    5.16129032258064e+01,
                    2.72727272727273e+01,
                    3.63636363636364e+01,
                    0.0,
                ],
            ),
            (
                "fgt",
                [
                    7.77777777777778e-01,
                    3.15789473684211e-01,
                    2.90322580645161e-01,
                    3.33333333333333e-01,
                    3.63636363636364e-01,
                    1.0,
                ],
            ),
            ("cin", [20.0, 7.0, 15.0, 24.0, 7.0, 2.0]),
            (
                "pinr",
                [
                    7.40740740740741e+01,
                    3.68421052631579e+01,
                    4.83870967741936e+01,
                    7.27272727272727e+01,
                    6.36363636363636e+01,
                    100.0,
                ],
            ),
        ],
    ),
];

fn run(weighted: bool, columnwise: bool, command: &str) -> Engine {
    let weight = if weighted { "WEIGHT BY w.\n" } else { "" };
    let missing = if columnwise {
        " /MISSING=COLUMNWISE"
    } else {
        ""
    };
    let source = format!("{DATA}{weight}AGGREGATE /OUTFILE=*{missing} /BREAK=g {command}.");
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{command}: {errors:?}");
    let mut engine = Engine::default();
    for c in &commands {
        engine
            .execute(&c.command)
            .unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    engine
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let v = data.variable(name).unwrap().numeric().unwrap();
    (0..data.case_count()).map(|k| v.get(k)).collect()
}

fn same(got: Option<f64>, want: f64) -> bool {
    match got {
        None => want.is_nan(),
        Some(g) => !want.is_nan() && (g - want).abs() <= 1e-9 * want.abs() + 1e-12,
    }
}

#[test]
fn every_function_agrees_with_r() {
    let mut bad = Vec::new();
    for (name, weighted, columnwise, command, expected) in RUNS {
        let engine = run(*weighted, *columnwise, command);
        for (target, want) in *expected {
            let got = numbers(&engine, target);
            for (group, (g, w)) in got.iter().zip(want).enumerate() {
                if !same(*g, *w) {
                    bad.push(format!(
                        "{name} {target} group {}: {g:?} against {w}",
                        group + 1
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn strings_agree_with_r() {
    let mut bad = Vec::new();
    for (weighted, command, firsts, expected) in STRING_RUNS {
        let engine = run(*weighted, false, command);
        let data = engine.dataset().unwrap();
        for (target, text) in ["f", "l", "mx", "mn"].iter().zip(firsts) {
            let want: Vec<&str> = text.split('|').collect();
            let v = data.variable(target).unwrap().string().unwrap();
            for (group, w) in want.iter().enumerate() {
                if v.get(group) != *w {
                    bad.push(format!(
                        "{target} group {}: {:?} against {w:?}",
                        group + 1,
                        v.get(group)
                    ));
                }
            }
        }
        for (target, want) in *expected {
            for (group, (g, w)) in numbers(&engine, target).iter().zip(want).enumerate() {
                if !same(*g, *w) {
                    bad.push(format!(
                        "{target} group {} weighted {weighted}: {g:?} against {w}",
                        group + 1
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn first_with_a_period_takes_a_user_missing_value_and_keeps_its_status() {
    // group 5 starts with the code -9: FIRST. gives it, and the target is still user-missing there
    let engine = run(false, false, "/f=FIRST.(x) /plain=FIRST(x)");
    assert_eq!(numbers(&engine, "f")[4], Some(-9.0));
    assert!(
        engine
            .dataset()
            .unwrap()
            .variable("f")
            .unwrap()
            .is_user_missing(-9.0)
    );
    // without the period the first valid value is taken: group 5 has none
    assert_eq!(numbers(&engine, "plain")[4], None);
}
