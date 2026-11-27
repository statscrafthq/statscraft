//! GLM tests of between-subjects effects with a covariate in other units, against R on 30
//! invented cases (y on the factors g and h and the covariate x; sums of squares of Type III
//! from sum-to-zero contrasts). The covariate is the data times 1E8, times 1E-8 and the data
//! as they are: the sums of squares do not depend on the unit. A covariate in hundred-millionths
//! left rounding residue of 1e-7 in the coefficients of the redundant factor column, above the
//! tolerance that decides which columns the hypothesis of the next effect uses, and its row had
//! no degrees of freedom. Script in lab/oracles/edge-cl.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g h x y.
BEGIN DATA
1 1 2.65 16.01
2 1 3.33 19.89
3 1 2.18 20.56
1 1 4.03 18.28
2 1 1.41 17.29
3 1 5.92 21.93
1 1 5.56 22.17
2 1 5.7 24.86
3 1 2.38 19.35
1 1 2.59 15.49
2 1 5.43 20.41
3 1 1.15 18.51
1 1 2.69 17.79
2 1 3.39 17.32
3 1 4.78 21.96
1 2 1.56 14.76
2 2 2.5 17.95
3 2 2.86 18.11
1 2 4.01 18.5
2 2 2.4 17.65
3 2 3.69 23.92
1 2 5.69 21.74
2 2 1.06 18.19
3 2 1.6 17.13
1 2 5.5 23.09
2 2 2.56 15.36
3 2 3.04 23.13
1 2 5.67 20.79
2 2 3.32 15.74
3 2 1.14 16.68
END DATA.
";

fn run(transform: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{DATA}{transform}GLM y BY g h WITH x /DESIGN=g h x."
    ));
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

fn check(tables: &[Table], what: &str) {
    let table = tables
        .iter()
        .find(|t| t.title == "Tests of Between-Subjects Effects")
        .expect("Tests of Between-Subjects Effects");
    let want = [
        ("g", 39.5619154007514, 2.0),
        ("h", 0.0013691136395, 1.0),
        ("x", 124.7203545488443, 1.0),
        ("Error", 77.1265921178227, 25.0),
    ];
    for (row, ss, df) in want {
        let r = table
            .rows
            .iter()
            .find(|r| matches!(&r[0], Cell::Text(t) if t == row))
            .unwrap_or_else(|| panic!("no row {row}"));
        let (Cell::Number(got_ss), Cell::Number(got_df)) = (&r[1], &r[2]) else {
            panic!("{what}: {row}: {r:?}")
        };
        assert!(
            (got_ss - ss).abs() <= 1e-8 * ss.abs(),
            "{what}: {row} SS {got_ss} is not {ss}"
        );
        assert_eq!(*got_df, df, "{what}: {row} df");
    }
}

#[test]
fn the_unchanged_data_agree_with_r() {
    check(&run(""), "base");
}

#[test]
fn a_covariate_in_hundred_millions_agrees_with_r() {
    check(&run("COMPUTE x = x * 1E8.\n"), "big");
}

#[test]
fn a_covariate_in_hundred_millionths_agrees_with_r() {
    check(&run("COMPUTE x = x * 1E-8.\n"), "small");
}
