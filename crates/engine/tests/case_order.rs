//! The order of the cases does not change a result. Each command runs on 60 invented cases with
//! fractional weights, a user-missing response, and system-missing predictors, and on the same
//! cases in another order; every table agrees cell by cell. Commands that depend on the order
//! by their definition (lists, sorting, a ranking of ties by position, a saved variable per
//! case) are not in the list.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const IN_ORDER: &str = "\
DATA LIST FREE /g h x z y b o s w.
BEGIN DATA
1 1 5.56 3.16 10.97 1 3 1 3
2 1 8.27 0.93 13.32 1 4 2 3
3 1 3.57 3.44 8.47 0 2 3 1
1 2 . 1.55 99 0 2 4 3
2 2 7.88 3.39 14.54 1 4 1 1.5
3 2 5.44 3.41 12.31 1 3 2 1.5
1 1 8.96 4.95 99 1 4 3 1.5
2 1 7.16 2.56 12.59 1 3 4 1.5
3 1 5.44 1.95 10.75 0 3 1 1.5
1 2 7.98 3.74 12.66 1 3 2 2
2 2 1.75 4.04 8.24 0 2 3 1.5
3 2 2.62 0.96 7.6 0 2 4 0.5
1 1 7.32 2.95 9.73 1 2 1 0.5
2 1 1.56 2.41 4.58 0 1 2 1
3 1 6.34 1.22 99 1 4 3 1.5
1 2 7.98 4.22 15.52 1 4 4 3
2 2 4.87 3.45 13.05 1 4 1 1.5
3 2 4.7 2.11 9.88 1 2 2 3
1 1 8.69 2.41 15.72 1 4 3 1.5
2 1 2.34 4.79 8.54 0 2 4 1.5
3 1 8.26 0.86 15.09 1 4 1 0.5
1 2 4.47 4.05 11.86 1 3 2 3
2 2 6.26 0.17 99 0 3 3 1
3 2 2.82 0.46 8.53 1 2 4 3
1 1 1.94 4.31 6.77 1 1 1 1
2 1 8.41 3.86 10.27 0 3 2 1.5
3 1 1.05 4.7 3.8 0 1 3 2
1 2 4.66 3.9 8.52 0 2 4 3
2 2 2.82 0.56 99 0 1 1 0.5
3 2 7.94 4.84 15.01 1 4 2 1
1 1 7.44 2.49 12.39 1 3 3 3
2 1 2.03 1.66 6.1 0 1 4 0.5
3 1 3.17 4.34 6.39 0 1 1 1.5
1 2 1.8 2.07 7.8 1 2 2 3
2 2 4.71 1.62 10.24 1 3 3 1
3 2 1.63 1.69 6.62 0 1 4 1.5
1 1 5.91 4.08 8.86 0 2 1 3
2 1 3.26 3.62 8.58 1 2 2 2
3 1 4.15 0.13 12.8 1 3 3 2
1 2 6.16 4.78 9.85 1 2 4 1
2 2 7.82 3.36 14.59 1 4 1 0.5
3 2 5.93 3.76 13.51 1 4 2 2
1 1 7.99 4.09 14.86 1 4 3 1.5
2 1 6.84 4.54 12.04 1 3 4 0.5
3 1 7.41 2.22 13.74 1 4 1 0.5
1 2 2.27 1.09 7.4 0 2 2 0.5
2 2 3.64 4.56 9.37 0 2 3 2
3 2 1.78 0.68 4.32 0 1 4 3
1 1 . 3.97 9.28 1 2 1 3
2 1 5.25 1.91 8.63 0 2 2 0.5
3 1 4.95 2.53 10.65 1 3 3 3
1 2 4.46 2.98 11.13 1 3 4 0.5
2 2 3.03 1.52 99 0 2 1 1
3 2 8.43 4.38 15.85 1 4 2 0.5
1 1 1.32 4.81 5.36 0 1 3 3
2 1 8.47 1.75 15.04 1 4 4 1.5
3 1 2.94 3.63 5.87 0 1 1 3
1 2 8.76 2.95 14.82 1 4 2 1
2 2 3.31 0.57 9.35 0 2 3 1
3 2 7.8 1.44 99 1 4 4 1.5
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
MISSING VALUES y (99).
WEIGHT BY w.
";
const SHUFFLED: &str = "\
DATA LIST FREE /g h x z y b o s w.
BEGIN DATA
1 1 5.56 3.16 10.97 1 3 1 3
1 2 . 1.55 99 0 2 4 3
3 1 8.26 0.86 15.09 1 4 1 0.5
1 1 . 3.97 9.28 1 2 1 3
1 1 8.69 2.41 15.72 1 4 3 1.5
2 2 3.64 4.56 9.37 0 2 3 2
3 1 1.05 4.7 3.8 0 1 3 2
3 1 5.44 1.95 10.75 0 3 1 1.5
2 1 3.26 3.62 8.58 1 2 2 2
2 1 2.03 1.66 6.1 0 1 4 0.5
3 2 8.43 4.38 15.85 1 4 2 0.5
1 2 7.98 3.74 12.66 1 3 2 2
3 1 4.95 2.53 10.65 1 3 3 3
3 2 7.94 4.84 15.01 1 4 2 1
2 2 4.71 1.62 10.24 1 3 3 1
3 1 7.41 2.22 13.74 1 4 1 0.5
3 2 5.44 3.41 12.31 1 3 2 1.5
3 1 4.15 0.13 12.8 1 3 3 2
1 1 5.91 4.08 8.86 0 2 1 3
2 2 7.88 3.39 14.54 1 4 1 1.5
1 2 2.27 1.09 7.4 0 2 2 0.5
2 1 1.56 2.41 4.58 0 1 2 1
1 2 6.16 4.78 9.85 1 2 4 1
3 2 5.93 3.76 13.51 1 4 2 2
1 1 7.32 2.95 9.73 1 2 1 0.5
2 1 6.84 4.54 12.04 1 3 4 0.5
3 1 3.57 3.44 8.47 0 2 3 1
1 2 4.46 2.98 11.13 1 3 4 0.5
1 2 7.98 4.22 15.52 1 4 4 3
2 2 7.82 3.36 14.59 1 4 1 0.5
2 1 5.25 1.91 8.63 0 2 2 0.5
3 2 4.7 2.11 9.88 1 2 2 3
1 2 8.76 2.95 14.82 1 4 2 1
2 2 2.82 0.56 99 0 1 1 0.5
3 2 7.8 1.44 99 1 4 4 1.5
1 1 7.44 2.49 12.39 1 3 3 3
3 2 2.62 0.96 7.6 0 2 4 0.5
3 2 1.78 0.68 4.32 0 1 4 3
1 1 7.99 4.09 14.86 1 4 3 1.5
2 1 8.47 1.75 15.04 1 4 4 1.5
2 2 3.31 0.57 9.35 0 2 3 1
2 1 2.34 4.79 8.54 0 2 4 1.5
2 1 8.27 0.93 13.32 1 4 2 3
1 1 8.96 4.95 99 1 4 3 1.5
2 2 1.75 4.04 8.24 0 2 3 1.5
2 2 3.03 1.52 99 0 2 1 1
3 1 2.94 3.63 5.87 0 1 1 3
1 2 1.8 2.07 7.8 1 2 2 3
2 1 7.16 2.56 12.59 1 3 4 1.5
3 1 3.17 4.34 6.39 0 1 1 1.5
3 1 6.34 1.22 99 1 4 3 1.5
2 2 6.26 0.17 99 0 3 3 1
1 1 1.94 4.31 6.77 1 1 1 1
3 2 2.82 0.46 8.53 1 2 4 3
1 1 1.32 4.81 5.36 0 1 3 3
2 2 4.87 3.45 13.05 1 4 1 1.5
1 2 4.47 4.05 11.86 1 3 2 3
2 1 8.41 3.86 10.27 0 3 2 1.5
3 2 1.63 1.69 6.62 0 1 4 1.5
1 2 4.66 3.9 8.52 0 2 4 3
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
MISSING VALUES y (99).
WEIGHT BY w.
";
const COMMANDS: &[&str] = &[
    "FREQUENCIES g h /STATISTICS=MEAN MEDIAN STDDEV SKEWNESS KURTOSIS /PERCENTILES=25 75.",
    "DESCRIPTIVES x z y /STATISTICS=MEAN STDDEV VARIANCE MIN MAX SKEWNESS KURTOSIS.",
    "MEANS y BY g h /CELLS=MEAN COUNT STDDEV MEDIAN.",
    "EXAMINE y BY g /PLOT=NONE /STATISTICS=DESCRIPTIVES.",
    "CROSSTABS g BY h /STATISTICS=CHISQ PHI LAMBDA /CELLS=COUNT EXPECTED.",
    "CORRELATIONS x z y /STATISTICS=DESCRIPTIVES.",
    "NONPAR CORR x z y.",
    "REGRESSION /DEPENDENT y /METHOD=ENTER x z.",
    "T-TEST GROUPS=h(1,2) /VARIABLES=y x.",
    "T-TEST PAIRS=x WITH z.",
    "T-TEST /TESTVAL=5 /VARIABLES=y.",
    "ONEWAY y BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY.",
    "GLM y BY g h WITH x.",
    "UNIANOVA y BY g WITH x /EMMEANS=TABLES(g).",
    "MANOVA y x BY g(1,3).",
    "LOGISTIC REGRESSION b WITH x z.",
    "NPAR TESTS /M-W=y BY h(1,2).",
    "NPAR TESTS /K-W=y BY g(1,3).",
    "NPAR TESTS /WILCOXON=x WITH z.",
    "NPAR TESTS /CHISQUARE=o.",
    "NPAR TESTS /RUNS(MEDIAN)=y.",
    "NPAR TESTS /K-S(NORMAL)=y.",
    "NPAR TESTS /BINOMIAL=b.",
    "FACTOR /VARIABLES x z y /PRINT=INITIAL EXTRACTION.",
    "RELIABILITY /VARIABLES=x z y /SUMMARY=TOTAL.",
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x z y.",
    "QUICK CLUSTER x z y /CRITERIA=CLUSTER(2).",
    "NOMREG o BY g WITH x.",
    "PLUM o BY g WITH x.",
    "GENLIN y BY g WITH x.",
    "GENLIN b (REFERENCE=FIRST) BY g WITH x /MODEL g x DISTRIBUTION=BINOMIAL LINK=LOGIT.",
    "CURVEFIT y WITH x.",
    "PARTIAL CORR x WITH y BY z.",
    "COXREG y /STATUS=b(1) /METHOD=ENTER x.",
    "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x z y.",
    "SUMMARIZE /TABLES=x y /CELLS=COUNT MEAN.",
    "CTABLES /TABLE y BY g.",
    "OLAP CUBES y BY g /CELLS=MEAN COUNT.",
    "2SLS y WITH x z /INSTRUMENTS=x g.",
    "ANACOR TABLE=g(1,3) BY o(1,4).",
    "ANOVA y BY g(1,3) h(1,2).",
    "CORRESPONDENCE TABLE=g(1,3) BY o(1,4).",
    "FACTOR /VARIABLES x z y /EXTRACTION=PC /ROTATION=VARIMAX /PRINT=ROTATION.",
    "FREQUENCIES x /FORMAT=NOTABLE /NTILES=4 /STATISTICS=MODE.",
    "GLM y BY g /LMATRIX=g 1 -1 0.",
    "KM y BY g /STATUS=b(1) /PRINT=MEAN.",
    "MIXED y BY g h WITH x /FIXED=g x /RANDOM=h.",
    "MULT RESPONSE GROUPS=$m 'm' (g h (1,3)) /FREQUENCIES=$m.",
    "NAIVEBAYES b BY g WITH x z.",
    "NPTESTS /INDEPENDENT TEST (y) GROUP (h).",
    "OPTIMAL BINNING /VARIABLES GUIDE=b BIN=x.",
    "PARTIAL CORR x y BY z.",
    "PLS y WITH x z.",
    "QUANTILE REGRESSION y WITH x z.",
    "RATIO STATISTICS x WITH z /PRINT=COD MEAN.",
    "ROC y BY b(1).",
    "SURVIVAL TABLE=y /INTERVALS=THRU 100 BY 10 /STATUS=b(1).",
    "TREE b [n] BY g [n] x [s] z [s].",
    "VARCOMP y BY g h /RANDOM=g.",
    "WEIGHTED KAPPA g h.",
];

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?} in {source}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn same(a: &Cell, b: &Cell) -> bool {
    match (a, b) {
        (Cell::Number(x), Cell::Number(y)) => {
            (x - y).abs() <= 1e-7 * x.abs().max(y.abs()).max(1e-3)
        }
        (Cell::Text(x), Cell::Text(y)) => x == y,
        (Cell::Empty, Cell::Empty) => true,
        _ => false,
    }
}

#[test]
fn the_order_of_the_cases_does_not_change_a_result() {
    for command in COMMANDS {
        let a = run(&format!("{IN_ORDER}{command}\n"));
        let b = run(&format!("{SHUFFLED}{command}\n"));
        assert_eq!(a.len(), b.len(), "{command}: number of tables");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.title, y.title, "{command}");
            assert_eq!(x.rows.len(), y.rows.len(), "{command}: {}", x.title);
            for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
                for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                    assert!(
                        same(ca, cb),
                        "{command}: {} row {r} column {c}: {ca:?} is not {cb:?}",
                        x.title
                    );
                }
            }
        }
    }
}
