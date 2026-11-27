//! Case selection against R: FILTER (a zero, a system-missing and a user-missing value of the
//! filter variable leave a case out, FILTER OFF, FILTER with no keyword, a renamed filter variable
//! still filtering and a deleted one not, a filter computed from a condition), TEMPORARY with
//! FILTER, SELECT IF, WEIGHT and SPLIT FILE (each lasts one procedure), SELECT IF (which deletes
//! the cases it leaves out) with FILTER and WEIGHT together, N OF CASES before and after SELECT IF
//! (the first n cases that are selected either way), WEIGHT (a zero, negative or missing weight
//! leaves a case out), and SPLIT FILE with a filter or a selection and weights. The cases are 30
//! invented rows; after each run DESCRIPTIVES shows the cases that are used, and the expected N
//! (the sum of the weights), sum, mean and standard deviation are worked out in R on the rows the
//! rules of the manual leave in. R has no filters of its own: it subsets the rows and computes
//! the statistics, while the rules (a deleted filter variable turns the filter off, a renamed one
//! keeps it, a temporary filter returns to the status before it) are the manual's.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    /// N, sum, mean and standard deviation of x in every DESCRIPTIVES table, in order (a split
    /// file gives one for each group); NAN for a standard deviation that needs two cases.
    expected: &'static [[f64; 4]],
}

const DATA: &str = r####"DATA LIST LIST /id g x f h w (F3 F1 F5.1 F2 F1 F5.2).
BEGIN DATA
1 1 4.5 1 1 1
2 2 7 0 1 2
3 3 . 1 0 0.5
4 2 2 . 0 3
5 1 9.5 1 1 .
6 3 -9 2 . 1
7 2 6 0 1 2
8 1 3.5 1 0 -1
9 3 8 9 1 1
10 1 1 1 1 4
11 2 . 1 0 1
12 3 5 0 1 0
13 1 7.5 -1 1 2
14 2 2.5 1 0 1.5
15 3 6.5 . 0 1
16 3 4 1 1 3
17 2 -9 1 1 2
18 1 3 0 . 1
19 2 8.5 1 1 .
20 3 5.5 1 0 1
21 1 2 2 1 2
22 1 9 0 1 0.25
23 2 1.5 1 0 1
24 3 . 9 1 3
25 2 6 1 1 1
26 3 7 1 0 2
27 1 4 0 0 1
28 2 3 1 1 0
29 1 5 . 1 2
30 3 8 1 1 1
END DATA.
MISSING VALUES x (-9) f (9).
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"FILTER and FILTER OFF"####,
        command: r####"FILTER BY f.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
FILTER OFF.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [15.0, 74.0, 4.93333333333333, 2.72466686473953],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"FILTER with no keyword is FILTER OFF"####,
        command: r####"FILTER BY f.
FILTER.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[[25.0, 129.5, 5.18, 2.49532896962839]],
    },
    Case {
        label: r####"TEMPORARY FILTER lasts one procedure"####,
        command: r####"TEMPORARY.
FILTER BY f.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [15.0, 74.0, 4.93333333333333, 2.72466686473953],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"TEMPORARY SELECT IF lasts one procedure"####,
        command: r####"TEMPORARY.
SELECT IF (x > 3).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [18.0, 114.5, 6.36111111111111, 1.82148578295751],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"SELECT IF stays"####,
        command: r####"SELECT IF (x > 3).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [18.0, 114.5, 6.36111111111111, 1.82148578295751],
            [18.0, 114.5, 6.36111111111111, 1.82148578295751],
        ],
    },
    Case {
        label: r####"FILTER and SELECT IF together"####,
        command: r####"FILTER BY f.
SELECT IF (g = 2).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
FILTER OFF.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [5.0, 21.5, 4.3, 2.8853076092507],
            [8.0, 36.5, 4.5625, 2.62457479549409],
        ],
    },
    Case {
        label: r####"a renamed filter variable still filters, a deleted one does not"####,
        command: r####"FILTER BY f.
RENAME VARIABLES (f = fx).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DELETE VARIABLES fx.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [15.0, 74.0, 4.93333333333333, 2.72466686473953],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"TEMPORARY FILTER over a FILTER already set"####,
        command: r####"FILTER BY f.
TEMPORARY.
FILTER BY h.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [16.0, 94.0, 5.875, 2.53968501984006],
            [15.0, 74.0, 4.93333333333333, 2.72466686473953],
        ],
    },
    Case {
        label: r####"a filter computed from a condition"####,
        command: r####"COMPUTE keep = (x > 3 AND g <> 1).
FILTER BY keep.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[[11.0, 71.5, 6.5, 1.37840487520902]],
    },
    Case {
        label: r####"WEIGHT, a zero, negative or missing weight leaves a case out"####,
        command: r####"WEIGHT BY w.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
WEIGHT OFF.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [32.75, 144.0, 4.3969465648855, 2.40055567488777],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"WEIGHT with FILTER and SELECT IF"####,
        command: r####"WEIGHT BY w.
FILTER BY f.
SELECT IF (g <> 3).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[[12.5, 38.75, 3.1, 2.50651325463325]],
    },
    Case {
        label: r####"TEMPORARY WEIGHT lasts one procedure"####,
        command: r####"TEMPORARY.
WEIGHT BY w.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [32.75, 144.0, 4.3969465648855, 2.40055567488777],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
    Case {
        label: r####"N OF CASES before SELECT IF"####,
        command: r####"N OF CASES 7.
SELECT IF (x > 3).
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[[7.0, 43.5, 6.21428571428571, 2.09875246390848]],
    },
    Case {
        label: r####"N OF CASES after SELECT IF"####,
        command: r####"SELECT IF (x > 3).
N OF CASES 7.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[[7.0, 43.5, 6.21428571428571, 2.09875246390848]],
    },
    Case {
        label: r####"SPLIT FILE with FILTER"####,
        command: r####"SORT CASES BY g.
SPLIT FILE BY g.
FILTER BY f.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [6.0, 28.0, 4.66666666666667, 3.2659863237109],
            [5.0, 21.5, 4.3, 2.8853076092507],
            [4.0, 24.5, 6.125, 1.75],
        ],
    },
    Case {
        label: r####"SPLIT FILE with SELECT IF and WEIGHT"####,
        command: r####"SORT CASES BY g.
SPLIT FILE BY g.
SELECT IF (x > 2).
WEIGHT BY w.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [7.25, 38.75, 5.3448275862069, 1.82813264439899],
            [6.5, 35.75, 5.5, 1.84637236468999],
            [9.0, 54.0, 6.0, 1.67705098312484],
        ],
    },
    Case {
        label: r####"TEMPORARY SPLIT FILE lasts one procedure"####,
        command: r####"SORT CASES BY g.
TEMPORARY.
SPLIT FILE BY g.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV.
DESCRIPTIVES x /STATISTICS=SUM MEAN STDDEV."####,
        expected: &[
            [10.0, 49.0, 4.9, 2.88482620312251],
            [8.0, 36.5, 4.5625, 2.62457479549409],
            [7.0, 44.0, 6.28571428571429, 1.52362350055011],
            [25.0, 129.5, 5.18, 2.49532896962839],
        ],
    },
];

/// The N, sum, mean and standard deviation the tables give for x, one entry for each row of x.
fn observed(tables: &[Table]) -> Vec<[f64; 4]> {
    let mut out = Vec::new();
    for table in tables
        .iter()
        .filter(|t| t.title == "Descriptive Statistics")
    {
        let titles = table.column_titles();
        let column = |name: &str| titles.iter().position(|c| c == name);
        let variable = column("Variable").unwrap_or(0);
        let (Some(n), Some(sum), Some(mean), Some(sd)) = (
            column("N"),
            column("Sum"),
            column("Mean"),
            column("Std Dev"),
        ) else {
            continue;
        };
        for row in &table.rows {
            if matches!(row.get(variable), Some(Cell::Text(t)) if t == "x") {
                let value = |c: usize| match row.get(c) {
                    Some(Cell::Number(v)) => *v,
                    _ => f64::NAN,
                };
                out.push([value(n), value(sum), value(mean), value(sd)]);
            }
        }
    }
    out
}

fn run(case: &Case) -> Result<Vec<Table>, String> {
    let (commands, errors) = parse(&format!("{DATA}{}", case.command));
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        let more = engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
        tables.extend(more);
    }
    Ok(tables)
}

fn same(got: f64, want: f64) -> bool {
    (got.is_nan() && want.is_nan()) || (got - want).abs() <= 1e-9 * want.abs().max(1.0)
}

#[test]
fn case_selection_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let tables = match run(case) {
            Ok(tables) => tables,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        let got = observed(&tables);
        if got.is_empty() {
            let shape: Vec<String> = tables
                .iter()
                .map(|t| {
                    format!(
                        "{} {:?} {} {:?}",
                        t.title,
                        t.column_titles(),
                        t.row_headers,
                        t.rows.first()
                    )
                })
                .collect();
            failures.push(format!("{}: no x row in {shape:?}", case.label));
            continue;
        }
        let wrong = got.len() != case.expected.len()
            || got
                .iter()
                .zip(case.expected)
                .any(|(g, w)| g.iter().zip(w).any(|(a, b)| !same(*a, *b)));
        if wrong {
            failures.push(format!(
                "{}: got {got:?}, R has {:?}",
                case.label, case.expected
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
