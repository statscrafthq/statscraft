//! TEMPORARY lasts one procedure. After TEMPORARY a WEIGHT, a SPLIT FILE, a FILTER, a SELECT IF
//! or a COMPUTE changes what the next command sees and the one after it sees the data as they
//! were: the two commands together give the tables of the command with the change as a plain
//! statement, and then those of the command on the untouched data. Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "\
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
COMPUTE half = 1 + MOD($CASENUM, 2).
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
];

fn run(source: &str) -> Result<Vec<Table>, String> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?} in {source}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(tables)
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

fn agree(a: &[Table], b: &[Table], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: number of tables");
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.rows.len(), y.rows.len(), "{what}: {}", x.title);
        for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
            for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                assert!(
                    same(ca, cb),
                    "{what}: {} row {r} column {c}: {ca:?} is not {cb:?}",
                    x.title
                );
            }
        }
    }
}

/// `change` (a statement or several) after TEMPORARY, against the same change as a plain
/// statement followed by the command, then the command on the data without it.
fn lasts_one_command(change: &str) {
    for command in COMMANDS {
        let temporary = run(&format!(
            "{DATA}TEMPORARY.\n{change}\n{command}\n{command}\n"
        ));
        let what = format!("{change}: {command}");
        match run(&format!("{DATA}{change}\n{command}\n")) {
            Ok(mut expected) => {
                expected.extend(run(&format!("{DATA}{command}\n")).expect("on the data"));
                agree(&temporary.expect(&what), &expected, &what);
            }
            // a command the change leaves without cases enough fails the same way
            Err(_) => assert!(temporary.is_err(), "{what}"),
        }
    }
}

#[test]
fn a_temporary_weight_lasts_one_command() {
    lasts_one_command("WEIGHT BY w.");
}

#[test]
fn a_temporary_split_file_lasts_one_command() {
    lasts_one_command("SPLIT FILE SEPARATE BY half.");
}

#[test]
fn a_temporary_filter_lasts_one_command() {
    lasts_one_command("COMPUTE keep = (x > 3).\nFILTER BY keep.");
}

#[test]
fn a_temporary_select_if_lasts_one_command() {
    lasts_one_command("SELECT IF x > 3.");
}

#[test]
fn a_temporary_compute_lasts_one_command() {
    lasts_one_command("COMPUTE x = x * 2 + 1.");
}

#[test]
fn a_temporary_recode_lasts_one_command() {
    lasts_one_command("RECODE g (1=3) (3=1).");
}
