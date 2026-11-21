//! The direct marketing names DMROC, DMLOGISTIC, DMTREE and DMTABLES run the same commands as
//! ROC, LOGISTIC REGRESSION, TREE and CTABLES: every table they give equals the primary's.

use statscraft_engine::{Engine, Table};
use statscraft_syntax::parse;

/// A test mailing: id, region, age group, age, tenure in years, a score and whether it paid off.
const DATA: &str = "DATA LIST FREE /id region agegrp age tenure (5F3) score (F5.1) bought (F1).
BEGIN DATA.
1 2 1 30 7 37.0 0
2 2 3 58 1 32.5 0
3 1 1 23 2 11.8 0
4 2 1 24 14 39.7 0
5 1 3 61 11 55.8 0
6 1 2 35 1 14.8 0
7 1 2 39 7 25.9 1
8 3 1 32 2 16.0 0
9 3 1 33 6 10.5 1
10 3 1 34 8 2.8 0
11 2 3 70 6 57.8 1
12 1 1 32 12 19.5 0
13 1 3 57 5 36.9 0
14 2 3 59 2 15.2 1
15 2 1 31 13 51.1 1
16 3 1 25 13 18.0 0
17 2 2 42 12 23.3 1
18 1 1 26 5 11.1 0
19 3 1 25 1 0.9 0
20 3 2 49 5 33.8 0
21 3 2 43 1 0.8 0
22 2 1 24 4 28.3 0
23 1 3 68 4 39.3 0
24 1 2 49 7 16.1 0
25 1 2 48 14 30.8 0
26 3 2 45 4 -18.2 1
27 1 1 30 4 33.2 0
28 1 2 37 5 15.9 1
29 2 3 55 6 34.2 0
30 3 3 60 11 46.0 0
31 1 3 50 15 62.1 0
32 3 2 46 7 16.2 0
33 1 3 51 11 46.7 0
34 2 1 31 2 47.2 0
35 1 1 27 1 -21.5 0
36 3 1 22 2 0.2 0
37 3 2 45 3 29.8 0
38 2 1 28 2 9.3 0
39 2 3 51 8 52.0 1
40 3 2 37 8 34.9 0
END DATA.
VARIABLE LEVEL region agegrp (NOMINAL).
";

fn run(command: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{command}"));
    assert!(errors.is_empty(), "{command}: {errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for parsed in &commands {
        tables.extend(
            engine
                .execute(&parsed.command)
                .unwrap_or_else(|e| panic!("{command}: {e}")),
        );
    }
    tables
}

/// The alias gives tables, and the same ones as the primary command.
fn same(primary: &str, alias: &str) {
    let (first, second) = (run(primary), run(alias));
    assert!(!first.is_empty(), "{primary} gave no table");
    assert_eq!(format!("{first:?}"), format!("{second:?}"), "{alias}");
}

#[test]
fn dmroc_is_roc() {
    same(
        "ROC score BY bought (1) /PLOT=NONE /PRINT=SE COORDINATES.",
        "DMROC score BY bought (1) /PLOT=NONE /PRINT=SE COORDINATES.",
    );
    same(
        "ROC score BY bought (1) /PLOT=NONE.",
        "DMR score BY bought (1) /PLOT=NONE.",
    );
}

#[test]
fn dmlogistic_is_logistic_regression() {
    same(
        "LOGISTIC REGRESSION bought WITH age tenure /PRINT=CI(95).",
        "DMLOGISTIC bought WITH age tenure /PRINT=CI(95).",
    );
    same(
        "LOGISTIC REGRESSION bought WITH age.",
        "DML bought WITH age.",
    );
}

#[test]
fn dmtree_is_tree() {
    let tail = "/GROWTHLIMIT MINPARENTSIZE=10 MINCHILDSIZE=5 /PRINT=MODELSUMMARY CLASSIFICATION TREETABLE.";
    same(
        &format!("TREE bought [n] BY region [n] agegrp [o] age [s] {tail}"),
        &format!("DMTREE bought [n] BY region [n] agegrp [o] age [s] {tail}"),
    );
    same(
        &format!("TREE bought [n] BY region [n] {tail}"),
        &format!("DMTR bought [n] BY region [n] {tail}"),
    );
}

#[test]
fn dmtables_is_ctables() {
    let table = "/TABLE region BY bought [COUNT, ROWPCT.COUNT PCT8.1] /CATEGORIES VARIABLES=region bought TOTAL=YES.";
    same(&format!("CTABLES {table}"), &format!("DMTABLES {table}"));
    same(&format!("CTABLES {table}"), &format!("DMTA {table}"));
}
