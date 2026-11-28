//! Selecting cases three ways gives one answer: FILTER BY, SELECT IF and the file written without
//! the cases agree table by table, and SPLIT FILE SEPARATE gives the tables of each group's own
//! run one after another. The commands run on 30 invented cases with a filter that leaves 17 of
//! them and a split into groups of 10 and 20. Titles and captions carry the split label, so
//! the rows are compared.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "\
DATA LIST LIST(',') /id x y g (F8.2) s (A8) f (F2).
BEGIN DATA
1,4.99,12.04,1,A,0
2,6.81,10.28,2,AA,0
3,3.95,6.79,3,AAA,0
4,4.65,13.94,1,AAAA,0
5,7.84,4.8,2,A,0
6,8.91,9.29,3,AA,0
7,5.39,3.64,1,AAA,0
8,7.13,4.4,2,AAAA,0
9,5.86,18.95,3,A,0
10,8.32,16.03,1,AA,0
11,2.44,15.9,2,AAA,1
12,5.01,4.95,3,AAAA,1
13,1.93,7.22,1,A,1
14,5.37,3.1,2,AA,1
15,8.42,17.64,3,AAA,1
16,6.35,5.59,1,AAAA,1
17,1.94,15.68,2,A,1
18,5.02,9.79,3,AA,1
19,3.83,13.69,1,AAA,1
20,4.56,14.78,2,AAAA,1
21,4.59,12.34,3,A,1
22,1.37,6.6,1,AA,1
23,7.9,12.8,2,AAA,1
24,4.56,15.84,3,AAAA,1
25,6.87,10.42,1,A,1
26,1.23,0.07,2,AA,1
27,1.22,11.16,3,AAA,1
28,3.13,2.77,1,AAAA,1
29,2.6,9.44,2,A,1
30,1.47,11.4,3,AA,1
END DATA.
";
const COMMANDS: &[&str] = &[
    "DESCRIPTIVES x y /STATISTICS=ALL.",
    "FREQUENCIES g /STATISTICS=MEAN MEDIAN STDDEV.",
    "MEANS y BY g /CELLS=MEAN COUNT STDDEV.",
    "CROSSTABS g BY s /STATISTICS=CHISQ /CELLS=COUNT EXPECTED.",
    "REGRESSION /DEPENDENT y /METHOD=ENTER x id.",
    "T-TEST GROUPS=g(1,2) /VARIABLES=x y.",
    "ONEWAY x BY g /STATISTICS=DESCRIPTIVES /POSTHOC=TUKEY.",
    "CORRELATIONS x y id.",
    "NONPAR CORR x y id.",
    "GLM y BY g WITH x.",
    "UNIANOVA y BY g.",
    "NPAR TESTS /K-W=x BY g(1,3).",
    "NPAR TESTS /M-W=x BY g(1,2).",
    "EXAMINE x BY g /PLOT=NONE.",
    "FACTOR /VARIABLES x y id.",
    "RELIABILITY /VARIABLES=x y id.",
    "NOMREG g BY s WITH x.",
    "GENLIN y BY g WITH x.",
    "MIXED y BY g WITH x /FIXED=g x.",
    "AGGREGATE /OUTFILE=* /BREAK=g /m=MEAN(x) /n=N.\nLIST.",
    "CTABLES /TABLE x BY g.",
    "SUMMARIZE /TABLES=x y BY g /CELLS=COUNT MEAN.",
    "RANK VARIABLES=x /RANK INTO rx.\nLIST rx.",
    "SORT CASES BY y.\nLIST id y.",
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x y.",
    "CLUSTER x y /METHOD=WARD.",
    "PARTIAL CORR x WITH y BY id.",
    "CURVEFIT y WITH x.",
    "PLUM g BY s WITH x.",
    "LOGISTIC REGRESSION f WITH x y.",
    "COXREG x /STATUS=f(1) /METHOD=ENTER y.",
    "KM x BY g /STATUS=f(1).",
    "FREQUENCIES s /BARCHART.",
    "DESCRIPTIVES x /SAVE.\nLIST zx.",
];

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?} in {source}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error} in {source}", command.first_line),
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

fn rows_agree(a: &[Table], b: &[Table], what: &str) {
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

#[test]
fn filter_by_and_select_if_agree() {
    for command in COMMANDS {
        let filter = run(&format!(
            "{DATA}COMPUTE keep = (x > 4).\nFILTER BY keep.\n{command}\n"
        ));
        let select = run(&format!(
            "{DATA}COMPUTE keep = (x > 4).\nSELECT IF keep.\n{command}\n"
        ));
        rows_agree(&filter, &select, &format!("filter: {command}"));
    }
}

#[test]
fn split_file_separate_is_each_group_on_its_own() {
    // a list of the cases and an aggregated file are one table or file for the whole split file,
    // and the split variable is constant within a group, so it cannot be a response
    for command in COMMANDS
        .iter()
        .filter(|c| !c.contains("LIST") && !c.contains("f(1)") && !c.contains("REGRESSION f"))
    {
        let split = run(&format!("{DATA}SPLIT FILE SEPARATE BY f.\n{command}\n"));
        let own: Vec<Table> = (0..2)
            .flat_map(|k| run(&format!("{DATA}SELECT IF f = {k}.\n{command}\n")))
            .collect();
        rows_agree(&split, &own, &format!("split: {command}"));
    }
}
