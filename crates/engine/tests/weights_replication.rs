//! A case weight does what the cases it stands for do: a frequency weight of 2 is the case
//! twice, and a case of weight 0 is no case. Each command below is run on weighted data and on
//! the same data written out (the case repeated by its weight; the zero-weight cases removed),
//! and every table has to agree cell by cell. The two data sets are invented. Commands that print
//! the number of cases unweighted (case processing summaries, a list of the cases) or pick
//! among cases by position are not in the list.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const ZERO_WEIGHTED: &str = "\
DATA LIST FREE /g h x z y b o w.
BEGIN DATA
1 1 7.39 1.63 12.83 1 3 1
2 1 3.34 4.77 9.71 1 2 3
3 1 6.05 2.5 12.87 1 3 0.5
1 2 2.38 4.07 3.81 0 1 2
2 2 3.65 0.59 7.87 0 2 0.5
2 2 -11.98 15.14 35.28 0 4 0
3 2 2.64 0.96 9.44 1 2 3
1 1 8.21 2.98 12.98 1 3 2
2 1 6.95 4.25 14.32 1 4 1
3 1 8.33 3.92 16.57 1 4 3
1 2 8.8 1.76 11.59 1 3 0.5
3 1 -13.72 -0.52 -88.34 1 4 0
2 2 7.01 4.73 11.98 1 3 0.5
3 2 4.22 0.69 10.54 1 3 1
1 1 1.42 3.16 5.44 0 1 0.5
2 1 1.94 4.93 8.82 0 2 1
3 1 6.21 4.11 12.37 1 3 3
2 2 39.86 33.95 54.35 1 3 0
1 2 1.73 4.22 8.81 0 2 0.5
2 2 5.6 0.31 8.8 1 2 1
3 2 6.61 1.2 12.49 1 3 0.5
1 1 8.8 0.52 15.04 1 4 2
2 1 7.18 0.43 13.39 1 4 2
2 2 68.98 34.79 -20.53 1 3 0
3 1 6.36 3.85 14.96 1 4 3
1 2 4.34 2.53 6.46 0 1 0.5
2 2 5.97 1.48 11.49 1 3 2
3 2 1.15 2.43 9.16 1 2 1
1 1 1.72 4.39 3.61 0 1 0.5
1 2 73.08 12.49 66.72 0 1 0
2 1 2.32 4.76 6.14 0 1 0.5
3 1 5.26 4.09 9.2 1 2 0.5
1 2 7.76 1.42 13.63 1 4 3
2 2 3.59 4.57 9.55 0 2 0.5
3 2 8.88 3.84 15.22 1 4 3
2 2 67.84 9.29 -42.15 0 3 0
1 1 4.2 4.34 7.04 1 2 3
2 1 7.75 3.29 14.97 1 4 1
3 1 2.16 1.8 8.09 0 2 3
1 2 5.99 3.03 9.38 0 2 3
2 2 1.96 3.46 8.8 1 2 1.5
2 1 -33.99 25.23 -64.12 0 4 0
3 2 1.48 2.58 4.52 0 1 1
1 1 8.04 1.5 12.07 1 3 1
2 1 1.38 3.74 7.16 0 2 3
3 1 7.8 0.38 15.14 1 4 2
1 2 1.31 3.92 6.35 0 1 1.5
3 2 -5.35 37.44 68.9 1 2 0
2 2 6.07 4.12 9.74 0 2 1
3 2 1.36 4.99 6.8 0 1 1
1 1 2.43 2.41 4.23 0 1 1.5
2 1 3.6 1.15 7.32 0 2 3
3 1 4.09 3.3 9.38 0 2 1
1 2 2.69 3.07 8.31 0 2 1.5
2 2 3.72 3.22 9.7 1 2 2
3 2 3.03 3.98 10.31 1 3 2
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
WEIGHT BY w.
";
const ZERO_REMOVED: &str = "\
DATA LIST FREE /g h x z y b o w.
BEGIN DATA
1 1 7.39 1.63 12.83 1 3 1
2 1 3.34 4.77 9.71 1 2 3
3 1 6.05 2.5 12.87 1 3 0.5
1 2 2.38 4.07 3.81 0 1 2
2 2 3.65 0.59 7.87 0 2 0.5
3 2 2.64 0.96 9.44 1 2 3
1 1 8.21 2.98 12.98 1 3 2
2 1 6.95 4.25 14.32 1 4 1
3 1 8.33 3.92 16.57 1 4 3
1 2 8.8 1.76 11.59 1 3 0.5
2 2 7.01 4.73 11.98 1 3 0.5
3 2 4.22 0.69 10.54 1 3 1
1 1 1.42 3.16 5.44 0 1 0.5
2 1 1.94 4.93 8.82 0 2 1
3 1 6.21 4.11 12.37 1 3 3
1 2 1.73 4.22 8.81 0 2 0.5
2 2 5.6 0.31 8.8 1 2 1
3 2 6.61 1.2 12.49 1 3 0.5
1 1 8.8 0.52 15.04 1 4 2
2 1 7.18 0.43 13.39 1 4 2
3 1 6.36 3.85 14.96 1 4 3
1 2 4.34 2.53 6.46 0 1 0.5
2 2 5.97 1.48 11.49 1 3 2
3 2 1.15 2.43 9.16 1 2 1
1 1 1.72 4.39 3.61 0 1 0.5
2 1 2.32 4.76 6.14 0 1 0.5
3 1 5.26 4.09 9.2 1 2 0.5
1 2 7.76 1.42 13.63 1 4 3
2 2 3.59 4.57 9.55 0 2 0.5
3 2 8.88 3.84 15.22 1 4 3
1 1 4.2 4.34 7.04 1 2 3
2 1 7.75 3.29 14.97 1 4 1
3 1 2.16 1.8 8.09 0 2 3
1 2 5.99 3.03 9.38 0 2 3
2 2 1.96 3.46 8.8 1 2 1.5
3 2 1.48 2.58 4.52 0 1 1
1 1 8.04 1.5 12.07 1 3 1
2 1 1.38 3.74 7.16 0 2 3
3 1 7.8 0.38 15.14 1 4 2
1 2 1.31 3.92 6.35 0 1 1.5
2 2 6.07 4.12 9.74 0 2 1
3 2 1.36 4.99 6.8 0 1 1
1 1 2.43 2.41 4.23 0 1 1.5
2 1 3.6 1.15 7.32 0 2 3
3 1 4.09 3.3 9.38 0 2 1
1 2 2.69 3.07 8.31 0 2 1.5
2 2 3.72 3.22 9.7 1 2 2
3 2 3.03 3.98 10.31 1 3 2
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
WEIGHT BY w.
";
const FREQUENCY_WEIGHTED: &str = "\
DATA LIST FREE /g h x z y b o w.
BEGIN DATA
1 1 7.52 0.48 14.98 1 4 2
2 1 6.95 4.5 11.19 0 3 2
3 1 7.18 0.94 13.53 1 4 3
1 2 1.08 4.6 4.86 0 1 2
2 2 8.82 2.03 15.69 1 4 2
3 2 3.92 1.03 10.78 1 3 2
1 1 2.17 3.22 4.06 0 1 1
2 1 6.0 3.34 11.3 1 3 3
3 1 1.66 4.33 9.05 1 2 2
1 2 2.82 3.95 6.36 0 1 3
2 2 2.05 2.88 5.15 0 1 3
3 2 3.42 3.36 6.45 0 1 1
1 1 2.65 0.79 10.18 1 3 2
2 1 6.76 4.9 13.55 1 4 3
3 1 4.73 1.97 10.57 1 3 1
1 2 7.58 2.41 13.42 1 4 1
2 2 5.63 1.56 11.51 1 3 2
3 2 7.95 4.7 15.64 1 4 1
1 1 4.83 0.46 7.9 0 2 1
2 1 4.93 1.67 9.38 1 2 2
3 1 1.58 4.04 4.03 0 1 1
1 2 4.85 1.74 9.11 1 2 1
2 2 4.26 2.37 8.14 1 2 3
3 2 6.19 2.46 11.69 0 3 3
1 1 4.64 1.58 10.21 1 3 1
2 1 6.73 4.17 12.53 1 3 1
3 1 6.06 2.46 10.96 1 3 3
1 2 8.18 2.14 12.15 1 3 3
2 2 6.81 1.47 12.64 1 3 2
3 2 7.75 0.63 15.5 1 4 3
1 1 4.37 1.44 8.31 0 2 3
2 1 5.9 1.83 11.24 1 3 1
3 1 3.26 4.35 8.4 0 2 1
1 2 1.22 4.33 6.5 0 1 2
2 2 7.56 3.23 14.8 1 4 1
3 2 5.66 4.48 13.36 1 4 3
1 1 6.95 4.62 10.48 1 3 3
2 1 2.15 2.52 8.77 1 2 1
3 1 2.42 1.83 6.69 1 1 1
1 2 4.3 2.89 7.65 0 2 2
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
WEIGHT BY w.
";
const FREQUENCY_REPEATED: &str = "\
DATA LIST FREE /g h x z y b o w.
BEGIN DATA
1 1 7.52 0.48 14.98 1 4 1
1 1 7.52 0.48 14.98 1 4 1
2 1 6.95 4.5 11.19 0 3 1
2 1 6.95 4.5 11.19 0 3 1
3 1 7.18 0.94 13.53 1 4 1
3 1 7.18 0.94 13.53 1 4 1
3 1 7.18 0.94 13.53 1 4 1
1 2 1.08 4.6 4.86 0 1 1
1 2 1.08 4.6 4.86 0 1 1
2 2 8.82 2.03 15.69 1 4 1
2 2 8.82 2.03 15.69 1 4 1
3 2 3.92 1.03 10.78 1 3 1
3 2 3.92 1.03 10.78 1 3 1
1 1 2.17 3.22 4.06 0 1 1
2 1 6.0 3.34 11.3 1 3 1
2 1 6.0 3.34 11.3 1 3 1
2 1 6.0 3.34 11.3 1 3 1
3 1 1.66 4.33 9.05 1 2 1
3 1 1.66 4.33 9.05 1 2 1
1 2 2.82 3.95 6.36 0 1 1
1 2 2.82 3.95 6.36 0 1 1
1 2 2.82 3.95 6.36 0 1 1
2 2 2.05 2.88 5.15 0 1 1
2 2 2.05 2.88 5.15 0 1 1
2 2 2.05 2.88 5.15 0 1 1
3 2 3.42 3.36 6.45 0 1 1
1 1 2.65 0.79 10.18 1 3 1
1 1 2.65 0.79 10.18 1 3 1
2 1 6.76 4.9 13.55 1 4 1
2 1 6.76 4.9 13.55 1 4 1
2 1 6.76 4.9 13.55 1 4 1
3 1 4.73 1.97 10.57 1 3 1
1 2 7.58 2.41 13.42 1 4 1
2 2 5.63 1.56 11.51 1 3 1
2 2 5.63 1.56 11.51 1 3 1
3 2 7.95 4.7 15.64 1 4 1
1 1 4.83 0.46 7.9 0 2 1
2 1 4.93 1.67 9.38 1 2 1
2 1 4.93 1.67 9.38 1 2 1
3 1 1.58 4.04 4.03 0 1 1
1 2 4.85 1.74 9.11 1 2 1
2 2 4.26 2.37 8.14 1 2 1
2 2 4.26 2.37 8.14 1 2 1
2 2 4.26 2.37 8.14 1 2 1
3 2 6.19 2.46 11.69 0 3 1
3 2 6.19 2.46 11.69 0 3 1
3 2 6.19 2.46 11.69 0 3 1
1 1 4.64 1.58 10.21 1 3 1
2 1 6.73 4.17 12.53 1 3 1
3 1 6.06 2.46 10.96 1 3 1
3 1 6.06 2.46 10.96 1 3 1
3 1 6.06 2.46 10.96 1 3 1
1 2 8.18 2.14 12.15 1 3 1
1 2 8.18 2.14 12.15 1 3 1
1 2 8.18 2.14 12.15 1 3 1
2 2 6.81 1.47 12.64 1 3 1
2 2 6.81 1.47 12.64 1 3 1
3 2 7.75 0.63 15.5 1 4 1
3 2 7.75 0.63 15.5 1 4 1
3 2 7.75 0.63 15.5 1 4 1
1 1 4.37 1.44 8.31 0 2 1
1 1 4.37 1.44 8.31 0 2 1
1 1 4.37 1.44 8.31 0 2 1
2 1 5.9 1.83 11.24 1 3 1
3 1 3.26 4.35 8.4 0 2 1
1 2 1.22 4.33 6.5 0 1 1
1 2 1.22 4.33 6.5 0 1 1
2 2 7.56 3.23 14.8 1 4 1
3 2 5.66 4.48 13.36 1 4 1
3 2 5.66 4.48 13.36 1 4 1
3 2 5.66 4.48 13.36 1 4 1
1 1 6.95 4.62 10.48 1 3 1
1 1 6.95 4.62 10.48 1 3 1
1 1 6.95 4.62 10.48 1 3 1
2 1 2.15 2.52 8.77 1 2 1
3 1 2.42 1.83 6.69 1 1 1
1 2 4.3 2.89 7.65 0 2 1
1 2 4.3 2.89 7.65 0 2 1
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
";

const COMMANDS: &[&str] = &[
    "2SLS y WITH x z /INSTRUMENTS=x g.",
    "ANACOR TABLE=g(1,3) BY o(1,4).",
    "ANOVA y BY g(1,3) h(1,2).",
    "CORRELATIONS x z y /STATISTICS=DESCRIPTIVES.",
    "CORRESPONDENCE TABLE=g(1,3) BY o(1,4).",
    "CROSSTABS g BY h /STATISTICS=CHISQ PHI LAMBDA /CELLS=COUNT EXPECTED.",
    "DESCRIPTIVES x z y /STATISTICS=MEAN STDDEV VARIANCE MIN MAX SKEWNESS KURTOSIS.",
    "FACTOR /VARIABLES x z y /EXTRACTION=PC /ROTATION=VARIMAX /PRINT=ROTATION.",
    "FACTOR /VARIABLES x z y /PRINT=INITIAL EXTRACTION.",
    "FREQUENCIES g h /STATISTICS=MEAN MEDIAN STDDEV SKEWNESS KURTOSIS /PERCENTILES=25 75.",
    "FREQUENCIES x /FORMAT=NOTABLE /NTILES=4 /STATISTICS=MODE.",
    "GENLIN b (REFERENCE=FIRST) BY g WITH x /MODEL g x DISTRIBUTION=BINOMIAL LINK=LOGIT.",
    "GENLIN y BY g WITH x.",
    "GLM y BY g h WITH x.",
    "GLM y BY g /LMATRIX=g 1 -1 0.",
    "KM y BY g /STATUS=b(1) /PRINT=MEAN.",
    "MANOVA y x BY g(1,3).",
    "MEANS y BY g h /CELLS=MEAN COUNT STDDEV MEDIAN.",
    "MIXED y BY g h WITH x /FIXED=g x /RANDOM=h.",
    "MULT RESPONSE GROUPS=$m 'm' (g h (1,3)) /FREQUENCIES=$m.",
    "NAIVEBAYES b BY g WITH x z.",
    "NOMREG o BY g WITH x.",
    "NONPAR CORR x z y.",
    "NPAR TESTS /BINOMIAL=b.",
    "NPAR TESTS /CHISQUARE=o.",
    "NPAR TESTS /K-S(NORMAL)=y.",
    "NPAR TESTS /K-W=y BY g(1,3).",
    "NPAR TESTS /M-W=y BY h(1,2).",
    "NPAR TESTS /RUNS(MEDIAN)=y.",
    "NPAR TESTS /WILCOXON=x WITH z.",
    "NPTESTS /INDEPENDENT TEST (y) GROUP (h).",
    "OLAP CUBES y BY g /CELLS=MEAN COUNT.",
    "ONEWAY y BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY.",
    "OPTIMAL BINNING /VARIABLES GUIDE=b BIN=x.",
    "PARTIAL CORR x WITH y BY z.",
    "PARTIAL CORR x y BY z.",
    "PLS y WITH x z.",
    "PLUM o BY g WITH x.",
    "QUANTILE REGRESSION y WITH x z.",
    "RATIO STATISTICS x WITH z /PRINT=COD MEAN.",
    "REGRESSION /DEPENDENT y /METHOD=ENTER x z.",
    "RELIABILITY /VARIABLES=x z y /SUMMARY=TOTAL.",
    "ROC y BY b(1).",
    "ROC y BY b(1).",
    "SUMMARIZE /TABLES=x y /CELLS=COUNT MEAN.",
    "SURVIVAL TABLE=y /INTERVALS=THRU 100 BY 10 /STATUS=b(1).",
    "TREE b [n] BY g [n] x [s] z [s].",
    "T-TEST GROUPS=h(1,2) /VARIABLES=y x.",
    "T-TEST PAIRS=x WITH z.",
    "T-TEST /TESTVAL=5 /VARIABLES=y.",
    "UNIANOVA y BY g WITH x /EMMEANS=TABLES(g).",
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

fn agree(weighted: &str, written_out: &str, what: &str) {
    for command in COMMANDS {
        let a = run(&format!("{weighted}{command}\n"));
        let b = run(&format!("{written_out}{command}\n"));
        assert_eq!(a.len(), b.len(), "{what}: {command}: number of tables");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.title, y.title, "{what}: {command}");
            assert_eq!(x.rows.len(), y.rows.len(), "{what}: {command}: {}", x.title);
            for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
                for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                    assert!(
                        same(ca, cb),
                        "{what}: {command}: {} row {r} column {c}: {ca:?} is not {cb:?}",
                        x.title
                    );
                }
            }
        }
    }
}

#[test]
fn a_frequency_weight_is_the_case_repeated() {
    agree(FREQUENCY_WEIGHTED, FREQUENCY_REPEATED, "repeated");
}

#[test]
fn a_case_of_weight_zero_is_no_case() {
    agree(ZERO_WEIGHTED, ZERO_REMOVED, "zero");
}
