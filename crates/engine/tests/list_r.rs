//! LIST against R on 14 invented cases: which cases are listed (CASES FROM, TO and BY, a number alone
//! meaning TO, after SELECT IF and N OF CASES, chosen again in each SPLIT FILE group), the case numbers
//! of FORMAT NUMBERED (counting the cases that remain, starting again in each split), VARIABLES in the
//! order written with a variable named twice listed twice and ALL after some, a system-missing
//! number as a period and a missing string as blank, and numbers as their print format writes them
//! (half away from zero, no zero before the point of a number below one). R picks the rows with
//! `seq` and indexing and writes the numbers by the guide's rule.

use statscraft_engine::{Cell, Engine};
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [&'static str],
    /// The cells of a row, separated by tabs.
    rows: &'static [&'static str],
}

const DATA: &str = r####"DATA LIST LIST /g (F1) x (F5.2) y (F8.2) s (A3).
BEGIN DATA
1 5 10.5 'a'
1 3 20 'bb'
1 . 0.25 ''
2 8 . 'dd'
2 1.25 5.125 'e'
2 2 6 'f'
2 . 7 'g'
3 9 8 ''
3 4 9 'ii'
3 6 10 'j'
3 7 11 'k'
3 2 12 'l'
4 5 13 'm'
4 11 14.75 'nn'
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"LIST of every variable, a missing number as a period"####,
        command: r####"LIST."####,
        columns: &[r####"g"####, r####"x"####, r####"y"####, r####"s"####],
        rows: &[
            r####"1	5.00	10.50	a"####,
            r####"1	3.00	20.00	bb"####,
            r####"1	.	.25	"####,
            r####"2	8.00	.	dd"####,
            r####"2	1.25	5.13	e"####,
            r####"2	2.00	6.00	f"####,
            r####"2	.	7.00	g"####,
            r####"3	9.00	8.00	"####,
            r####"3	4.00	9.00	ii"####,
            r####"3	6.00	10.00	j"####,
            r####"3	7.00	11.00	k"####,
            r####"3	2.00	12.00	l"####,
            r####"4	5.00	13.00	m"####,
            r####"4	11.00	14.75	nn"####,
        ],
    },
    Case {
        label: r####"VARIABLES in the order written, a variable twice listed twice, case numbers"####,
        command: r####"LIST VARIABLES=s x s /FORMAT=NUMBERED."####,
        columns: &[r####"Case"####, r####"s"####, r####"x"####, r####"s"####],
        rows: &[
            r####"1	a	5.00	a"####,
            r####"2	bb	3.00	bb"####,
            r####"3		.	"####,
            r####"4	dd	8.00	dd"####,
            r####"5	e	1.25	e"####,
            r####"6	f	2.00	f"####,
            r####"7	g	.	g"####,
            r####"8		9.00	"####,
            r####"9	ii	4.00	ii"####,
            r####"10	j	6.00	j"####,
            r####"11	k	7.00	k"####,
            r####"12	l	2.00	l"####,
            r####"13	m	5.00	m"####,
            r####"14	nn	11.00	nn"####,
        ],
    },
    Case {
        label: r####"ALL after some variables lists them again"####,
        command: r####"LIST VARIABLES=x ALL."####,
        columns: &[
            r####"x"####,
            r####"g"####,
            r####"x"####,
            r####"y"####,
            r####"s"####,
        ],
        rows: &[
            r####"5.00	1	5.00	10.50	a"####,
            r####"3.00	1	3.00	20.00	bb"####,
            r####".	1	.	.25	"####,
            r####"8.00	2	8.00	.	dd"####,
            r####"1.25	2	1.25	5.13	e"####,
            r####"2.00	2	2.00	6.00	f"####,
            r####".	2	.	7.00	g"####,
            r####"9.00	3	9.00	8.00	"####,
            r####"4.00	3	4.00	9.00	ii"####,
            r####"6.00	3	6.00	10.00	j"####,
            r####"7.00	3	7.00	11.00	k"####,
            r####"2.00	3	2.00	12.00	l"####,
            r####"5.00	4	5.00	13.00	m"####,
            r####"11.00	4	11.00	14.75	nn"####,
        ],
    },
    Case {
        label: r####"CASES FROM 3 TO 12 BY 4"####,
        command: r####"LIST VARIABLES=g x /CASES=FROM 3 TO 12 BY 4."####,
        columns: &[r####"g"####, r####"x"####],
        rows: &[r####"1	."####, r####"2	."####, r####"3	7.00"####],
    },
    Case {
        label: r####"CASES with a number alone is TO that number"####,
        command: r####"LIST VARIABLES=g y /CASES=5."####,
        columns: &[r####"g"####, r####"y"####],
        rows: &[
            r####"1	10.50"####,
            r####"1	20.00"####,
            r####"1	.25"####,
            r####"2	."####,
            r####"2	5.13"####,
        ],
    },
    Case {
        label: r####"CASES BY 3 with numbers"####,
        command: r####"LIST VARIABLES=x s /CASES=BY 3 /FORMAT=NUMBERED."####,
        columns: &[r####"Case"####, r####"x"####, r####"s"####],
        rows: &[
            r####"1	5.00	a"####,
            r####"4	8.00	dd"####,
            r####"7	.	g"####,
            r####"10	6.00	j"####,
            r####"13	5.00	m"####,
        ],
    },
    Case {
        label: r####"TO beyond the file ends at the last case"####,
        command: r####"LIST VARIABLES=g /CASES=FROM 12 TO 100."####,
        columns: &[r####"g"####],
        rows: &[r####"3"####, r####"4"####, r####"4"####],
    },
    Case {
        label: r####"SELECT IF before LIST: the numbers count the cases that remain"####,
        command: r####"COMPUTE k = $CASENUM.
SELECT IF (k <> 2 AND k <> 3 AND k <> 7).
LIST VARIABLES=g x /CASES=FROM 2 TO 6 /FORMAT=NUMBERED."####,
        columns: &[r####"Case"####, r####"g"####, r####"x"####],
        rows: &[
            r####"2	2	8.00"####,
            r####"3	2	1.25"####,
            r####"4	2	2.00"####,
            r####"5	3	9.00"####,
            r####"6	3	4.00"####,
        ],
    },
    Case {
        label: r####"N OF CASES limits the list"####,
        command: r####"N OF CASES 6.
LIST VARIABLES=g /FORMAT=NUMBERED."####,
        columns: &[r####"Case"####, r####"g"####],
        rows: &[
            r####"1	1"####,
            r####"2	1"####,
            r####"3	1"####,
            r####"4	2"####,
            r####"5	2"####,
            r####"6	2"####,
        ],
    },
    Case {
        label: r####"SPLIT FILE: the cases are chosen again in each group and the numbers start again"####,
        command: r####"SPLIT FILE BY g.
LIST VARIABLES=g x /CASES=FROM 2 TO 3 /FORMAT=NUMBERED."####,
        columns: &[r####"g"####, r####"Case"####, r####"g"####, r####"x"####],
        rows: &[
            r####"1	2	1	3.00"####,
            r####"1	3	1	."####,
            r####"2	2	2	1.25"####,
            r####"2	3	2	2.00"####,
            r####"3	2	3	4.00"####,
            r####"3	3	3	6.00"####,
            r####"4	2	4	11.00"####,
        ],
    },
    Case {
        label: r####"SPLIT FILE with BY 2"####,
        command: r####"SPLIT FILE BY g.
LIST VARIABLES=x /CASES=BY 2 /FORMAT=NUMBERED."####,
        columns: &[r####"g"####, r####"Case"####, r####"x"####],
        rows: &[
            r####"1	1	5.00"####,
            r####"1	3	."####,
            r####"2	1	8.00"####,
            r####"2	3	2.00"####,
            r####"3	1	9.00"####,
            r####"3	3	6.00"####,
            r####"3	5	2.00"####,
            r####"4	1	5.00"####,
        ],
    },
];

fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.trim_end().to_string(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
        other => format!("{other:?}"),
    }
}

#[test]
fn list_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        let (commands, errors) = parse(&format!("{DATA}{}", case.command));
        if !errors.is_empty() {
            failures.push(format!("{}: {errors:?}", case.label));
            continue;
        }
        let mut tables = Vec::new();
        let mut failed = false;
        for command in &commands {
            match engine.execute(&command.command) {
                Ok(t) => tables.extend(t),
                Err(error) => {
                    failures.push(format!(
                        "{}: line {}: {error}",
                        case.label, command.first_line
                    ));
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            continue;
        }
        let lists: Vec<_> = tables
            .iter()
            .filter(|t| {
                t.columns.len() == case.columns.len()
                    && t.columns.iter().zip(case.columns).all(|(a, b)| a == b)
            })
            .collect();
        if lists.is_empty() {
            let seen: Vec<_> = tables
                .iter()
                .map(|t| (&t.title, &t.columns, t.rows.len()))
                .collect();
            failures.push(format!(
                "{}: no table with the columns {:?}; the tables are {seen:?}",
                case.label, case.columns
            ));
            continue;
        }
        // under SPLIT FILE the group value stands on the first row of its group only
        let mut above = String::new();
        let mut got: Vec<String> = Vec::new();
        for table in &lists {
            for row in &table.rows {
                let mut cells: Vec<String> = row.iter().map(cell_text).collect();
                if case.columns.first() == Some(&"g") && matches!(row.first(), Some(Cell::Empty)) {
                    cells[0] = above.clone();
                } else if case.columns.first() == Some(&"g") {
                    above = cells[0].clone();
                }
                got.push(cells.join("\t").trim_end().to_string());
            }
        }
        let want: Vec<String> = case.rows.iter().map(|r| r.trim_end().to_string()).collect();
        if got != want {
            failures.push(format!("{}:\n  got  {got:?}\n  R has {want:?}", case.label));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
