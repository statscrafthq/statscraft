//! A user-missing value does what a system-missing value does, and with INCLUDE what no
//! declaration does. Every command of the first list runs on data that declare -9, 99, 0 and 9
//! user-missing and on the same data with those values recoded to system-missing; every table
//! must agree. Every command of the second list names INCLUDE and runs on the declared data and
//! on the data with no declaration. In INCLUDE a command spares a factor only where the manual
//! says so (EXAMINE's INCLUDE is for dependent variables), so commands whose two runs differ
//! by design are not in the lists. Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DECLARED: &str = "\
DATA LIST FREE /g h x z y b o s.
BEGIN DATA
1 1 1.67 0.86 5.44 0 1 1
2 1 8.59 4.22 14.83 1 4 2
3 1 7.62 1.22 99 1 4 3
1 2 4.92 4.82 10.23 1 3 4
2 2 7.13 0.44 14.79 1 4 1
3 2 5.35 1.51 8.26 0 2 2
1 1 3.79 3.67 8.99 0 2 3
2 1 6.53 2.16 11.65 1 3 4
3 1 3.99 4.43 10.68 1 3 1
1 2 3.12 0.22 8.02 0 2 2
2 2 4.13 0.92 10.72 1 3 3
3 2 4.49 3.17 99 1 3 4
1 1 7.56 3.85 11.34 1 3 1
2 1 8.1 3.38 12.56 1 9 2
3 1 7.57 4.76 13.67 1 4 3
1 2 -9 3.12 9.43 1 2 4
2 2 6.66 4.39 12.98 1 3 1
3 2 5.4 2.99 11.71 1 9 2
1 1 8.41 4.52 15.68 1 9 3
2 1 5.77 0.04 10.69 1 3 4
3 1 -9 -9 11.25 0 9 1
1 2 2.86 2.84 3.44 0 1 2
2 2 2.67 3.42 8.74 0 2 3
0 2 3.14 -9 9.54 1 2 4
1 1 5.94 -9 99 1 3 1
0 1 1.79 2.06 8.18 0 2 2
3 1 4.87 -9 11.53 1 9 3
1 2 3.12 3.67 7.27 0 2 4
2 2 5.52 4.79 99 1 3 1
3 2 8.75 -9 18.22 1 4 2
1 1 4.76 -9 99 1 2 3
2 1 2.8 3.77 9.25 1 2 4
3 1 7.19 3.58 13.68 1 4 1
1 2 7.59 -9 11.62 1 3 2
2 2 1.83 1.7 6.47 0 1 3
3 2 3.24 3.05 5.38 0 1 4
1 1 3.07 0.19 8.1 1 2 1
2 1 -9 -9 10.19 0 3 2
3 1 6.62 0.98 14.53 1 4 3
1 2 6.82 1.99 11.3 0 3 4
2 2 1.67 2.23 6.99 0 1 1
3 2 6.46 1.66 14.44 1 4 2
1 1 8.97 4.78 15.36 1 4 3
2 1 8.58 1.33 15.36 1 9 4
3 1 6.39 1.37 13.64 1 4 1
1 2 7.63 3.61 99 1 4 2
2 2 8.21 1.61 11.68 1 3 3
3 2 7.95 0.29 12.43 1 3 4
1 1 3.58 -9 5.79 0 1 1
2 1 2.72 3.88 10.29 1 3 2
3 1 1.82 0.68 8.31 0 2 3
1 2 1.56 1.34 7.78 0 2 4
2 2 3.6 -9 8.64 0 2 1
3 2 6.72 2.57 13.94 1 9 2
1 1 -9 0.35 12.49 1 3 3
2 1 -9 1.2 99 1 3 4
3 1 6.24 2.38 13.31 1 4 1
1 2 1.67 4.82 99 0 9 2
2 2 -9 1.71 9.92 0 2 3
3 2 8.29 3.15 13.73 1 4 4
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
MISSING VALUES x z (-9) y (99) g (0) o (9).
";
const RECODED: &str = "\
DATA LIST FREE /g h x z y b o s.
BEGIN DATA
1 1 1.67 0.86 5.44 0 1 1
2 1 8.59 4.22 14.83 1 4 2
3 1 7.62 1.22 99 1 4 3
1 2 4.92 4.82 10.23 1 3 4
2 2 7.13 0.44 14.79 1 4 1
3 2 5.35 1.51 8.26 0 2 2
1 1 3.79 3.67 8.99 0 2 3
2 1 6.53 2.16 11.65 1 3 4
3 1 3.99 4.43 10.68 1 3 1
1 2 3.12 0.22 8.02 0 2 2
2 2 4.13 0.92 10.72 1 3 3
3 2 4.49 3.17 99 1 3 4
1 1 7.56 3.85 11.34 1 3 1
2 1 8.1 3.38 12.56 1 9 2
3 1 7.57 4.76 13.67 1 4 3
1 2 -9 3.12 9.43 1 2 4
2 2 6.66 4.39 12.98 1 3 1
3 2 5.4 2.99 11.71 1 9 2
1 1 8.41 4.52 15.68 1 9 3
2 1 5.77 0.04 10.69 1 3 4
3 1 -9 -9 11.25 0 9 1
1 2 2.86 2.84 3.44 0 1 2
2 2 2.67 3.42 8.74 0 2 3
0 2 3.14 -9 9.54 1 2 4
1 1 5.94 -9 99 1 3 1
0 1 1.79 2.06 8.18 0 2 2
3 1 4.87 -9 11.53 1 9 3
1 2 3.12 3.67 7.27 0 2 4
2 2 5.52 4.79 99 1 3 1
3 2 8.75 -9 18.22 1 4 2
1 1 4.76 -9 99 1 2 3
2 1 2.8 3.77 9.25 1 2 4
3 1 7.19 3.58 13.68 1 4 1
1 2 7.59 -9 11.62 1 3 2
2 2 1.83 1.7 6.47 0 1 3
3 2 3.24 3.05 5.38 0 1 4
1 1 3.07 0.19 8.1 1 2 1
2 1 -9 -9 10.19 0 3 2
3 1 6.62 0.98 14.53 1 4 3
1 2 6.82 1.99 11.3 0 3 4
2 2 1.67 2.23 6.99 0 1 1
3 2 6.46 1.66 14.44 1 4 2
1 1 8.97 4.78 15.36 1 4 3
2 1 8.58 1.33 15.36 1 9 4
3 1 6.39 1.37 13.64 1 4 1
1 2 7.63 3.61 99 1 4 2
2 2 8.21 1.61 11.68 1 3 3
3 2 7.95 0.29 12.43 1 3 4
1 1 3.58 -9 5.79 0 1 1
2 1 2.72 3.88 10.29 1 3 2
3 1 1.82 0.68 8.31 0 2 3
1 2 1.56 1.34 7.78 0 2 4
2 2 3.6 -9 8.64 0 2 1
3 2 6.72 2.57 13.94 1 9 2
1 1 -9 0.35 12.49 1 3 3
2 1 -9 1.2 99 1 3 4
3 1 6.24 2.38 13.31 1 4 1
1 2 1.67 4.82 99 0 9 2
2 2 -9 1.71 9.92 0 2 3
3 2 8.29 3.15 13.73 1 4 4
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
RECODE x z (-9=SYSMIS).
RECODE y (99=SYSMIS).
RECODE g (0=SYSMIS).
RECODE o (9=SYSMIS).
";
const UNDECLARED: &str = "\
DATA LIST FREE /g h x z y b o s.
BEGIN DATA
1 1 1.67 0.86 5.44 0 1 1
2 1 8.59 4.22 14.83 1 4 2
3 1 7.62 1.22 99 1 4 3
1 2 4.92 4.82 10.23 1 3 4
2 2 7.13 0.44 14.79 1 4 1
3 2 5.35 1.51 8.26 0 2 2
1 1 3.79 3.67 8.99 0 2 3
2 1 6.53 2.16 11.65 1 3 4
3 1 3.99 4.43 10.68 1 3 1
1 2 3.12 0.22 8.02 0 2 2
2 2 4.13 0.92 10.72 1 3 3
3 2 4.49 3.17 99 1 3 4
1 1 7.56 3.85 11.34 1 3 1
2 1 8.1 3.38 12.56 1 9 2
3 1 7.57 4.76 13.67 1 4 3
1 2 -9 3.12 9.43 1 2 4
2 2 6.66 4.39 12.98 1 3 1
3 2 5.4 2.99 11.71 1 9 2
1 1 8.41 4.52 15.68 1 9 3
2 1 5.77 0.04 10.69 1 3 4
3 1 -9 -9 11.25 0 9 1
1 2 2.86 2.84 3.44 0 1 2
2 2 2.67 3.42 8.74 0 2 3
0 2 3.14 -9 9.54 1 2 4
1 1 5.94 -9 99 1 3 1
0 1 1.79 2.06 8.18 0 2 2
3 1 4.87 -9 11.53 1 9 3
1 2 3.12 3.67 7.27 0 2 4
2 2 5.52 4.79 99 1 3 1
3 2 8.75 -9 18.22 1 4 2
1 1 4.76 -9 99 1 2 3
2 1 2.8 3.77 9.25 1 2 4
3 1 7.19 3.58 13.68 1 4 1
1 2 7.59 -9 11.62 1 3 2
2 2 1.83 1.7 6.47 0 1 3
3 2 3.24 3.05 5.38 0 1 4
1 1 3.07 0.19 8.1 1 2 1
2 1 -9 -9 10.19 0 3 2
3 1 6.62 0.98 14.53 1 4 3
1 2 6.82 1.99 11.3 0 3 4
2 2 1.67 2.23 6.99 0 1 1
3 2 6.46 1.66 14.44 1 4 2
1 1 8.97 4.78 15.36 1 4 3
2 1 8.58 1.33 15.36 1 9 4
3 1 6.39 1.37 13.64 1 4 1
1 2 7.63 3.61 99 1 4 2
2 2 8.21 1.61 11.68 1 3 3
3 2 7.95 0.29 12.43 1 3 4
1 1 3.58 -9 5.79 0 1 1
2 1 2.72 3.88 10.29 1 3 2
3 1 1.82 0.68 8.31 0 2 3
1 2 1.56 1.34 7.78 0 2 4
2 2 3.6 -9 8.64 0 2 1
3 2 6.72 2.57 13.94 1 9 2
1 1 -9 0.35 12.49 1 3 3
2 1 -9 1.2 99 1 3 4
3 1 6.24 2.38 13.31 1 4 1
1 2 1.67 4.82 99 0 9 2
2 2 -9 1.71 9.92 0 2 3
3 2 8.29 3.15 13.73 1 4 4
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
";
const EXCLUDING: &[&str] = &[
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
    "AGGREGATE /OUTFILE=* /BREAK=g /m=MEAN(y) /n=N /s=SUM(y).",
    "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x z y.",
    "SUMMARIZE /TABLES=x y /CELLS=COUNT MEAN.",
    "CTABLES /TABLE y BY g.",
    "OLAP CUBES y BY g /CELLS=MEAN COUNT.",
    "SURVIVAL TABLE=y /INTERVALS=THRU 100 BY 10 /STATUS=b(1).",
    "VARCOMP y BY g h /RANDOM=g.",
    "MIXED y BY g h WITH x /FIXED=g x /RANDOM=h.",
    "CORRESPONDENCE TABLE=g(1,3) BY o(1,4).",
    "ANOVA y BY g(1,3) h(1,2).",
    "HILOGLINEAR g(1,3) h(1,2) o(1,4) /MAXORDER=2.",
    "LOGLINEAR g(1,3) h(1,2) /DESIGN=g h.",
    "MULT RESPONSE GROUPS=$m 'm' (g h (1,3)) /FREQUENCIES=$m.",
    "2SLS y WITH x z /INSTRUMENTS=x g.",
    "QUANTILE REGRESSION y WITH x z.",
    "ROC y BY b(1).",
    "KNN b BY g WITH x z.",
    "NAIVEBAYES b BY g WITH x z.",
    "TREE b [n] BY g [n] x [s] z [s].",
    "PLS y WITH x z.",
    "HOMALS VARIABLES=g(3) h(2) o(4).",
    "CATPCA VARIABLES=x z y /ANALYSIS=x z y.",
    "RATIO STATISTICS x WITH z /PRINT=COD MEAN.",
    "NPTESTS /INDEPENDENT TEST (y) GROUP (h).",
    "WEIGHTED KAPPA g h.",
    "FREQUENCIES x /FORMAT=NOTABLE /NTILES=4 /STATISTICS=MODE.",
    "REGRESSION /STATISTICS=COEFF R ANOVA /DEPENDENT y /METHOD=STEPWISE x z.",
    "FACTOR /VARIABLES x z y /EXTRACTION=PC /ROTATION=VARIMAX.",
    "PARTIAL CORR x y BY z.",
    "CORRELATIONS x z y /MISSING=LISTWISE.",
    "CORRELATIONS x z y /MISSING=PAIRWISE.",
    "REGRESSION /MISSING=PAIRWISE /DEPENDENT y /METHOD=ENTER x z.",
    "REGRESSION /MISSING=MEANSUBSTITUTION /DEPENDENT y /METHOD=ENTER x z.",
    "T-TEST GROUPS=g(1,2) /VARIABLES=y /MISSING=LISTWISE.",
    "ONEWAY y BY g /MISSING=LISTWISE.",
    "NPAR TESTS /K-W=y BY g(1,3) /MISSING=LISTWISE.",
    "EXAMINE y BY g /MISSING=PAIRWISE /PLOT=NONE.",
    "MANOVA y x BY g(1,3).",
    "AGGREGATE /OUTFILE=* /BREAK=g /m=MEAN(y) /n=N /nm=NMISS(x).",
    "CTABLES /TABLE y BY g.",
    "SUMMARIZE /TABLES=x y BY g /CELLS=COUNT MEAN.",
];

const INCLUDING: &[&str] = &[
    "CORRELATIONS x z y /MISSING=PAIRWISE INCLUDE.",
    "CORRELATIONS x z y /MISSING=LISTWISE INCLUDE.",
    "NONPAR CORR x z y /MISSING=PAIRWISE INCLUDE.",
    "PARTIAL CORR x y BY z /MISSING=LISTWISE INCLUDE.",
    "REGRESSION /MISSING=LISTWISE INCLUDE /DEPENDENT y /METHOD=ENTER x z.",
    "REGRESSION /MISSING=PAIRWISE INCLUDE /DEPENDENT y /METHOD=ENTER x z.",
    "REGRESSION /MISSING=MEANSUBSTITUTION INCLUDE /DEPENDENT y /METHOD=ENTER x z.",
    "DESCRIPTIVES x z y /MISSING=VARIABLE INCLUDE.",
    "DESCRIPTIVES x z y /MISSING=LISTWISE INCLUDE.",
    "FREQUENCIES g o /MISSING=INCLUDE.",
    "FREQUENCIES x /MISSING=INCLUDE /NTILES=4 /STATISTICS=MEAN STDDEV.",
    "CROSSTABS g BY o /MISSING=INCLUDE.",
    "MEANS y BY g /MISSING=TABLE INCLUDE.",
    "MEANS y BY g /MISSING=DEPENDENT INCLUDE.",
    "T-TEST GROUPS=g(1,2) /VARIABLES=y /MISSING=ANALYSIS INCLUDE.",
    "T-TEST GROUPS=g(1,2) /VARIABLES=y x /MISSING=LISTWISE INCLUDE.",
    "T-TEST PAIRS=x WITH z /MISSING=ANALYSIS INCLUDE.",
    "T-TEST /TESTVAL=5 /VARIABLES=y /MISSING=INCLUDE.",
    "ONEWAY y BY g /MISSING=ANALYSIS INCLUDE /POSTHOC=TUKEY.",
    "ONEWAY y x BY g /MISSING=LISTWISE INCLUDE.",
    "NPAR TESTS /K-W=y BY g(1,3) /MISSING=ANALYSIS INCLUDE.",
    "NPAR TESTS /M-W=y BY g(1,2) /MISSING=LISTWISE INCLUDE.",
    "NPAR TESTS /WILCOXON=x WITH z /MISSING=ANALYSIS INCLUDE.",
    "NPAR TESTS /CHISQUARE=o /MISSING=ANALYSIS INCLUDE.",
    "NPAR TESTS /RUNS(MEDIAN)=y /MISSING=ANALYSIS INCLUDE.",
    "EXAMINE y BY g /MISSING=REPORT INCLUDE /PLOT=NONE.",
    "FACTOR /VARIABLES x z y /MISSING=LISTWISE INCLUDE.",
    "FACTOR /VARIABLES x z y /MISSING=PAIRWISE INCLUDE.",
    "FACTOR /VARIABLES x z y /MISSING=MEANSUB INCLUDE.",
    "RELIABILITY /VARIABLES=x z y /SUMMARY=TOTAL /MISSING=INCLUDE.",
    "GLM y BY g WITH x /MISSING=INCLUDE.",
    "UNIANOVA y BY g WITH x /MISSING=INCLUDE.",
    "MANOVA y x BY g(0,3) /MISSING=INCLUDE.",
    "LOGISTIC REGRESSION b WITH x z /MISSING=INCLUDE.",
    "DISCRIMINANT /GROUPS=g(0,3) /VARIABLES=x z y /MISSING=INCLUDE.",
    "NOMREG o BY g WITH x /MISSING=INCLUDE.",
    "PLUM o BY g WITH x /MISSING=INCLUDE.",
    "COXREG y /STATUS=b(1) /METHOD=ENTER x /MISSING=INCLUDE.",
    "QUICK CLUSTER x z y /MISSING=LISTWISE INCLUDE /CRITERIA=CLUSTER(2).",
    "CLUSTER x z y /MISSING=INCLUDE /METHOD=WARD.",
    "ANOVA y BY g(0,3) /MISSING=INCLUDE.",
    "MULT RESPONSE GROUPS=$m 'm' (g h (0,3)) /FREQUENCIES=$m /MISSING=INCLUDE.",
    "PROBIT b OF o WITH x /MISSING=INCLUDE.",
    "SUMMARIZE /TABLES=x y BY g /MISSING=INCLUDE /CELLS=COUNT MEAN.",
    "AGGREGATE /OUTFILE=* /BREAK=g /m=MEAN(y) /n=N /MISSING=COLUMNWISE.",
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

fn agree(first: &str, second: &str, commands: &[&str], what: &str) {
    for command in commands {
        let a = run(&format!("{first}{command}\n"));
        let b = run(&format!("{second}{command}\n"));
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
fn a_user_missing_value_is_left_out_like_a_system_missing_one() {
    agree(DECLARED, RECODED, EXCLUDING, "user-missing");
}

#[test]
fn include_counts_user_missing_values_as_valid() {
    agree(DECLARED, UNDECLARED, INCLUDING, "include");
}
