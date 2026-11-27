//! DISCRIMINANT stepwise selection against base R on 90 invented cases (three groups, five
//! predictors; x4 is half of x1 plus half of x2 with a little noise, so Wilks' lambda and Rao's V
//! enter it first and remove it again once x1 and x2 are in): the steps with a removal (the
//! removal is a step of its own, with the criterion after it), the orders of the other criteria,
//! and, on the same cases with missing values (left out listwise), frequency weights (the cases
//! written out as often as their weight) and SELECT (a flag), the steps of WILKS and RAO. The
//! manual's formulas written out in base R, as in discriminant_options_r.rs. Scripts in
//! lab/oracles/discriminant-r3.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x1 x2 x3 x4 x5 w sel.
BEGIN DATA.
1 -1.87 1.07 0.88 -0.6 -1.79 1 1
1 -1.11 -0.95 1.46 -0.88 0.78 2 1
1 0.4 -1.06 -1.2 -1.11 -0.89 3 1
1 2.34 0.34 -0.96 0.65 0.01 1 1
1 -0.16 -0.39 2.15 -0.28 1.88 1 1
1 -1.44 -0.48 2.34 -0.39 -0.42 2 1
1 0.6 0.76 0.15 0.64 -0.62 2 1
1 0.63 0.15 -0.27 1.04 -0.47 3 1
1 -1.46 0.76 -0.55 -0.45 -0.79 3 1
1 0.06 -0.87 0.33 -1.07 0.16 1 1
1 0.79 1.13 1.17 0.61 1.22 1 0
1 -0.75 -1.84 -0.59 -1.17 0.38 1 1
1 -2.47 0.99 -0.29 -0.69 1.07 1 1
1 0.65 0.23 0.15 0 0 2 0
1 1.22 0.45 0.49 0.05 -0.31 1 1
1 0.65 -1.2 -0.27 0.15 -1.86 1 1
1 -0.21 -1.25 -1.09 -0.29 0.31 1 1
1 0.86 -0.73 -0.82 0.14 0.77 3 1
1 1.27 -2.07 1.64 -0.95 -0.88 2 1
1 1.31 0.12 -2.1 0.08 1.53 3 1
1 1.7 -0.32 -2.07 1.06 0.22 3 1
1 -1.98 -0.87 -0.53 -1.08 0.86 2 1
1 -0.22 1.36 0.59 -0.24 1.22 1 1
1 0.5 0.37 0.22 0.8 -0.14 1 1
1 0.15 0.18 0.37 0.19 -1.75 1 1
1 0.14 -2.2 -0.5 -1.56 1.53 1 1
1 -1.4 -1.21 0.58 -0.56 -0.49 1 1
1 0.16 -1.07 0.97 -0.54 0.9 3 1
1 1.23 -0.4 0.16 1.09 0.48 1 1
1 -0.23 -1.78 1.02 -1 2.22 3 1
2 -0.48 0.03 0.25 -0.92 1.66 1 1
2 1.86 -0.32 1.07 0.54 -1.27 1 0
2 1.8 -0.2 -0.56 1.36 -1.03 1 1
2 0.74 0.44 -0.48 0.04 -0.08 1 0
2 1.15 0.34 -1.67 1.73 -0.4 1 1
2 0.38 2.63 -0.03 1.4 0.54 1 1
2 2.32 1.71 0.71 2.34 0.58 2 1
2 1.53 0.87 0.41 1.24 -1.06 2 1
2 3.04 0.94 -0.53 2.47 -0.26 1 1
2 0.81 0.84 -0.55 1.81 0.03 1 1
2 1.8 1.47 1.1 1.58 -2.02 3 1
2 0.72 2.62 -1.16 2.3 0.19 1 1
2 0.15 1.93 -0.79 1.53 -0.43 3 1
2 0.94 -0.08 1.34 -0.04 0.53 2 1
2 1.11 0.31 0.78 0.72 0.29 3 1
2 0.43 0.66 -0.54 1.5 0.91 2 1
2 1.39 1.13 -0.02 1.37 -0.08 2 1
2 2.79 1.24 0.71 0.87 -0.9 1 1
2 0.09 0.7 0.48 -0.07 -0.47 2 1
2 1.06 0.48 1.01 1.04 -0.5 2 1
2 3.12 1.29 -0.54 1.24 1.92 1 1
2 1.41 0.53 -0.06 0.79 1.16 2 1
2 1.66 1.4 -0.41 2.24 0.4 3 1
2 0 1.36 -2.07 0.76 2.04 3 0
2 -0.16 1.39 -0.13 0.63 -1.26 3 1
2 1.93 2.43 0.61 1.76 0.26 1 1
2 1.93 2.05 -0.39 2.17 -0.34 1 1
2 1.01 2.13 1 1.8 -0.4 1 1
2 1.4 0.78 -0.15 0.82 0.74 1 1
2 1.18 1.99 -0.8 1.45 0.35 1 1
3 2.14 -0.67 0.81 1.15 -0.54 2 1
3 4.16 -0.2 -0.47 2.58 -1.17 1 1
3 2.57 -0.32 0.72 1.3 0.26 3 1
3 2.95 1.02 0.38 1.76 -1.04 3 1
3 3.65 -0.9 -1.21 2.11 -1.07 3 1
3 1.22 1.77 0.24 1.04 0.7 3 1
3 2.67 1.16 -0.3 1.73 -0.75 3 1
3 1.61 -1.05 0.83 0.22 0.06 1 1
3 2.7 0.46 -0.11 1.63 0.75 3 1
3 3.06 1.12 1.31 2.47 0.2 3 1
3 1.22 0.19 0.2 0.83 0.48 3 1
3 4.09 -0.32 1.02 1.4 0.03 2 1
3 1.46 -0.71 0.68 0.8 -0.9 1 1
3 0.09 -0.34 -0.44 -0.71 0.75 3 0
3 3.18 0.59 2.61 2.25 -0.05 1 1
3 3.28 0.45 1.34 1.63 -0.67 3 1
3 3.58 -0.48 0.21 1.47 -1.95 1 1
3 1.44 -0.91 1.38 -0.76 -1.18 2 1
3 5.31 0.04 1.95 2.52 1.8 1 1
3 0.71 0.86 -0.29 0.84 -0.58 1 0
3 1.65 1.12 0.06 1.33 -0.89 3 1
3 0.44 -1.03 0.75 -0.26 1.14 3 0
3 2.33 -0.37 -1.63 0.75 -0.15 1 0
3 3.88 0.94 -0.62 2.65 0.18 1 1
3 2.27 0.66 1.66 1.53 0.72 1 1
3 2.02 0.88 1.54 1.19 0.91 1 0
3 1.65 -0.05 -0.02 0.4 0.28 3 1
3 1.2 -0.57 -1.43 0.46 1.1 2 0
3 1.53 2.3 -0.56 1.68 -1.74 1 1
3 0.9 1.44 -0.03 0.64 -0.43 2 1
END DATA.
";

const DATAM: &str = "DATA LIST FREE /g x1 x2 x3 x4 x5 w sel.
BEGIN DATA.
1 -1.87 1.07 0.88 -0.6 -1.79 1 1
1 -1.11 -0.95 1.46 -0.88 0.78 2 1
1 0.4 -1.06 -1.2 -1.11 -0.89 3 1
1 2.34 . -0.96 0.65 0.01 1 1
1 -0.16 -0.39 2.15 -0.28 1.88 1 1
1 -1.44 -0.48 2.34 -0.39 -0.42 2 1
1 0.6 0.76 0.15 0.64 -0.62 2 1
1 0.63 0.15 -0.27 1.04 -0.47 3 1
1 -1.46 0.76 -0.55 -0.45 -0.79 3 1
1 0.06 -0.87 0.33 -1.07 0.16 1 1
1 0.79 1.13 1.17 0.61 1.22 1 0
1 -0.75 -1.84 -0.59 -1.17 . 1 1
1 -2.47 0.99 -0.29 -0.69 1.07 1 1
1 0.65 0.23 0.15 0 0 2 0
1 1.22 0.45 0.49 0.05 -0.31 1 1
1 0.65 -1.2 -0.27 0.15 -1.86 1 1
1 -0.21 -1.25 -1.09 -0.29 0.31 1 1
1 0.86 -0.73 -0.82 0.14 0.77 3 1
1 1.27 -2.07 1.64 -0.95 -0.88 2 1
1 1.31 0.12 -2.1 0.08 1.53 3 1
1 1.7 -0.32 -2.07 1.06 0.22 3 1
1 -1.98 -0.87 -0.53 -1.08 0.86 2 1
1 -0.22 1.36 0.59 -0.24 1.22 1 1
1 0.5 0.37 0.22 0.8 -0.14 1 1
1 0.15 0.18 0.37 0.19 -1.75 1 1
1 0.14 -2.2 -0.5 -1.56 1.53 1 1
1 -1.4 -1.21 0.58 -0.56 -0.49 1 1
1 0.16 -1.07 0.97 -0.54 0.9 3 1
1 1.23 -0.4 0.16 1.09 0.48 1 1
1 -0.23 -1.78 1.02 -1 2.22 3 1
2 -0.48 0.03 0.25 -0.92 1.66 1 1
2 1.86 -0.32 1.07 0.54 -1.27 1 0
2 1.8 -0.2 -0.56 1.36 -1.03 1 1
2 0.74 0.44 -0.48 0.04 -0.08 1 0
2 1.15 0.34 -1.67 1.73 -0.4 1 1
2 0.38 2.63 -0.03 1.4 0.54 1 1
2 2.32 1.71 0.71 2.34 0.58 2 1
2 1.53 0.87 0.41 1.24 -1.06 2 1
2 3.04 0.94 -0.53 2.47 -0.26 1 1
2 0.81 . -0.55 1.81 0.03 1 1
2 1.8 1.47 1.1 1.58 -2.02 3 1
2 0.72 2.62 -1.16 2.3 0.19 1 1
2 0.15 1.93 -0.79 1.53 -0.43 3 1
2 0.94 -0.08 1.34 -0.04 0.53 2 1
2 1.11 0.31 0.78 0.72 0.29 3 1
2 0.43 0.66 -0.54 1.5 0.91 2 1
2 1.39 1.13 -0.02 1.37 -0.08 2 1
2 2.79 1.24 0.71 0.87 -0.9 1 1
2 0.09 0.7 0.48 -0.07 -0.47 2 1
2 1.06 0.48 1.01 1.04 -0.5 2 1
2 3.12 1.29 -0.54 1.24 1.92 1 1
2 1.41 0.53 -0.06 0.79 1.16 2 1
2 1.66 1.4 -0.41 2.24 0.4 3 1
2 0 1.36 -2.07 0.76 2.04 3 0
2 -0.16 1.39 -0.13 0.63 . 3 1
2 1.93 2.43 0.61 1.76 0.26 1 1
2 1.93 2.05 -0.39 2.17 -0.34 1 1
2 1.01 2.13 1 1.8 -0.4 1 1
2 1.4 0.78 -0.15 0.82 0.74 1 1
2 1.18 1.99 -0.8 1.45 0.35 1 1
3 2.14 -0.67 0.81 1.15 -0.54 2 1
3 4.16 -0.2 -0.47 2.58 -1.17 1 1
3 2.57 -0.32 0.72 1.3 0.26 3 1
3 2.95 1.02 0.38 1.76 -1.04 3 1
3 3.65 -0.9 -1.21 2.11 -1.07 3 1
3 1.22 1.77 0.24 1.04 0.7 3 1
3 2.67 1.16 -0.3 1.73 -0.75 3 1
3 1.61 -1.05 0.83 0.22 0.06 1 1
3 2.7 0.46 -0.11 1.63 0.75 3 1
3 3.06 1.12 1.31 2.47 0.2 3 1
3 1.22 . 0.2 0.83 0.48 3 1
3 4.09 -0.32 1.02 1.4 0.03 2 1
3 1.46 -0.71 0.68 0.8 -0.9 1 1
3 0.09 -0.34 -0.44 -0.71 0.75 3 0
3 3.18 0.59 2.61 2.25 -0.05 1 1
3 3.28 0.45 1.34 1.63 -0.67 3 1
3 3.58 -0.48 0.21 1.47 -1.95 1 1
3 1.44 -0.91 1.38 -0.76 -1.18 2 1
3 5.31 0.04 1.95 2.52 1.8 1 1
3 0.71 0.86 -0.29 0.84 -0.58 1 0
3 1.65 1.12 0.06 1.33 -0.89 3 1
3 0.44 -1.03 0.75 -0.26 1.14 3 0
3 2.33 -0.37 -1.63 0.75 -0.15 1 0
3 3.88 0.94 -0.62 2.65 0.18 1 1
3 2.27 0.66 1.66 1.53 0.72 1 1
3 2.02 0.88 1.54 1.19 0.91 1 0
3 1.65 -0.05 -0.02 0.4 0.28 3 1
3 . -0.57 -1.43 0.46 1.1 2 0
3 1.53 2.3 -0.56 1.68 -1.74 1 1
3 0.9 1.44 -0.03 0.64 -0.43 2 1
END DATA.
";

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

/// The row headers joined with " / ", numbers as they print, an empty header taking the one above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{n}"),
        Cell::Empty => String::new(),
    }
}

/// Each expectation: the table title, the row, the column (a title, or #n for the column at
/// position n) and the value.
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let t = tables
            .iter()
            .find(|t| t.title == *title)
            .unwrap_or_else(|| panic!("no table {title}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = match column.strip_prefix('#') {
            Some(n) => n.parse::<usize>().expect("a position"),
            None => t
                .column_titles()
                .iter()
                .position(|k| k == column)
                .unwrap_or_else(|| {
                    panic!("{title}: no column {column} in {:?}", t.column_titles())
                }),
        };
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-6 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn a_variable_leaves_again_wilks_agrees_with_r() {
    let t = run_on(
        DATA,
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /ANALYSIS=ALL /METHOD=WILKS /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Wilks' Lambda",
                0.5867776359,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                19.5628229,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Wilks' Lambda",
                0.6897883412,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                1.26074387,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Wilks' Lambda",
                0.971833715,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Wilks' Lambda",
                0.5866309284,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                0.6922801955,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Wilks' Lambda",
                0.9843348161,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Wilks' Lambda",
                0.5866309284,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.5866309284),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 87.0),
            ("Wilks' Lambda", "1", "F / Statistic", 30.65224445),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 87.0),
            ("Wilks' Lambda", "1", "F / Sig.", 8.392195479e-11),
            ("Variables in the Analysis", "1 / x4", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x4",
                "F to Remove",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                13.02892371,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Wilks' Lambda",
                0.4502162142,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.7489839241,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.7489839241,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                9.535779626,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Wilks' Lambda",
                0.4801514339,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9907911752,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9907911752,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                1.495751694,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Wilks' Lambda",
                0.5669109737,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.9757891441,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.9757891441,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                0.2222534759,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Wilks' Lambda",
                0.583614409,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Wilks' Lambda",
                0.4502162142,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.4502162142),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 87.0),
            ("Wilks' Lambda", "2", "F / Statistic", 21.0852215),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 172.0),
            ("Wilks' Lambda", "2", "F / Sig.", 3.663623963e-14),
            (
                "Variables in the Analysis",
                "2 / x4",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables in the Analysis",
                "2 / x4",
                "F to Remove",
                13.04293569,
            ),
            (
                "Variables in the Analysis",
                "2 / x4",
                "Wilks' Lambda",
                0.5867776359,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                13.02892371,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Wilks' Lambda",
                0.5866309284,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Tolerance",
                0.5620753171,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "F to Enter",
                4.615478414,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Wilks' Lambda",
                0.4061125929,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9806944263,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.5783135525,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                0.5664828979,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Wilks' Lambda",
                0.4442942125,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9746789983,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.5777978954,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                0.268173414,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Wilks' Lambda",
                0.44739318,
            ),
            (
                "Variables Entered/Removed",
                "3",
                "Wilks' Lambda",
                0.4061125929,
            ),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4061125929),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 87.0),
            ("Wilks' Lambda", "3", "F / Statistic", 16.12717694),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 170.0),
            ("Wilks' Lambda", "3", "F / Sig.", 1.194992793e-14),
            (
                "Variables in the Analysis",
                "3 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables in the Analysis",
                "3 / x4",
                "F to Remove",
                0.8110815946,
            ),
            (
                "Variables in the Analysis",
                "3 / x4",
                "Wilks' Lambda",
                0.4138629564,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.4424304214,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                7.748222531,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Wilks' Lambda",
                0.4801514339,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.5620753171,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                4.615478414,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Wilks' Lambda",
                0.4502162142,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9773153528,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Min. Tolerance",
                0.3247159476,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "F to Enter",
                0.6181415401,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Wilks' Lambda",
                0.4002222595,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Tolerance",
                0.973425586,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Min. Tolerance",
                0.3258421988,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "F to Enter",
                0.1969029571,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Wilks' Lambda",
                0.4042175541,
            ),
            (
                "Variables Entered/Removed",
                "4",
                "Wilks' Lambda",
                0.4138629564,
            ),
            ("Wilks' Lambda", "4", "Number of Variables", 2.0),
            ("Wilks' Lambda", "4", "Lambda", 0.4138629564),
            ("Wilks' Lambda", "4", "df1", 2.0),
            ("Wilks' Lambda", "4", "df2", 2.0),
            ("Wilks' Lambda", "4", "df3", 87.0),
            ("Wilks' Lambda", "4", "F / Statistic", 23.84057506),
            ("Wilks' Lambda", "4", "F / df1", 4.0),
            ("Wilks' Lambda", "4", "F / df2", 172.0),
            ("Wilks' Lambda", "4", "F / Sig.", 1.060569796e-15),
            (
                "Variables in the Analysis",
                "4 / x1",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "4 / x1",
                "F to Remove",
                28.66840668,
            ),
            (
                "Variables in the Analysis",
                "4 / x1",
                "Wilks' Lambda",
                0.6897883412,
            ),
            (
                "Variables in the Analysis",
                "4 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "4 / x2",
                "F to Remove",
                17.96568429,
            ),
            (
                "Variables in the Analysis",
                "4 / x2",
                "Wilks' Lambda",
                0.5867776359,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Min. Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "F to Enter",
                0.6923447172,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Wilks' Lambda",
                0.407229007,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "F to Enter",
                0.8110815946,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Wilks' Lambda",
                0.4061125929,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Min. Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "F to Enter",
                0.1435076214,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Wilks' Lambda",
                0.412470189,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[
        ("1", "x4", ""),
        ("2", "x1", ""),
        ("3", "x2", ""),
        ("4", "", "x4"),
    ];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn a_variable_leaves_again_rao_agrees_with_r() {
    let t = run_on(
        DATA,
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /ANALYSIS=ALL /METHOD=RAO /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Rao's V",
                61.26740947,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                19.5628229,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Rao's V",
                39.1256458,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                1.26074387,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Rao's V",
                2.521487739,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Rao's V",
                61.3044889,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                0.6922801955,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Rao's V",
                1.384560391,
            ),
            ("Variables Entered/Removed", "1", "Rao's V", 61.3044889),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.5866309284),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 87.0),
            ("Wilks' Lambda", "1", "F / Statistic", 30.65224445),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 87.0),
            ("Wilks' Lambda", "1", "F / Sig.", 8.392195479e-11),
            ("Variables in the Analysis", "1 / x4", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x4",
                "F to Remove",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Min. Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "F to Enter",
                13.02892371,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x1",
                "Rao's V",
                90.19886251,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.7489839241,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.7489839241,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                9.535779626,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Rao's V",
                81.28747856,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9907911752,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9907911752,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                1.495751694,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Rao's V",
                64.69317785,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.9757891441,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.9757891441,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                0.2222534759,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Rao's V",
                61.77656699,
            ),
            ("Variables Entered/Removed", "2", "Rao's V", 90.19886251),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.4502162142),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 87.0),
            ("Wilks' Lambda", "2", "F / Statistic", 21.0852215),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 172.0),
            ("Wilks' Lambda", "2", "F / Sig.", 3.663623963e-14),
            (
                "Variables in the Analysis",
                "2 / x4",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables in the Analysis",
                "2 / x4",
                "F to Remove",
                13.04293569,
            ),
            (
                "Variables in the Analysis",
                "2 / x4",
                "Rao's V",
                61.26740947,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                13.02892371,
            ),
            ("Variables in the Analysis", "2 / x1", "Rao's V", 61.3044889),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Tolerance",
                0.5620753171,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "F to Enter",
                4.615478414,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x2",
                "Rao's V",
                103.5078869,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9806944263,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.5783135525,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                0.5664828979,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Rao's V",
                91.90460213,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9746789983,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.5777978954,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                0.268173414,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Rao's V",
                90.87377122,
            ),
            ("Variables Entered/Removed", "3", "Rao's V", 103.5078869),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.4061125929),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 87.0),
            ("Wilks' Lambda", "3", "F / Statistic", 16.12717694),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 170.0),
            ("Wilks' Lambda", "3", "F / Sig.", 1.194992793e-14),
            (
                "Variables in the Analysis",
                "3 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables in the Analysis",
                "3 / x4",
                "F to Remove",
                0.8110815946,
            ),
            (
                "Variables in the Analysis",
                "3 / x4",
                "Rao's V",
                100.8725521,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.4424304214,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                7.748222531,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Rao's V",
                81.28747856,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.5620753171,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                4.615478414,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Rao's V",
                90.19886251,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9773153528,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Min. Tolerance",
                0.3247159476,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "F to Enter",
                0.6181415401,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x3",
                "Rao's V",
                105.3142479,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Tolerance",
                0.973425586,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Min. Tolerance",
                0.3258421988,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "F to Enter",
                0.1969029571,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Rao's V",
                104.042518,
            ),
            ("Variables Entered/Removed", "4", "Rao's V", 100.8725521),
            ("Wilks' Lambda", "4", "Number of Variables", 2.0),
            ("Wilks' Lambda", "4", "Lambda", 0.4138629564),
            ("Wilks' Lambda", "4", "df1", 2.0),
            ("Wilks' Lambda", "4", "df2", 2.0),
            ("Wilks' Lambda", "4", "df3", 87.0),
            ("Wilks' Lambda", "4", "F / Statistic", 23.84057506),
            ("Wilks' Lambda", "4", "F / df1", 4.0),
            ("Wilks' Lambda", "4", "F / df2", 172.0),
            ("Wilks' Lambda", "4", "F / Sig.", 1.060569796e-15),
            (
                "Variables in the Analysis",
                "4 / x1",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "4 / x1",
                "F to Remove",
                28.66840668,
            ),
            ("Variables in the Analysis", "4 / x1", "Rao's V", 39.1256458),
            (
                "Variables in the Analysis",
                "4 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "4 / x2",
                "F to Remove",
                17.96568429,
            ),
            (
                "Variables in the Analysis",
                "4 / x2",
                "Rao's V",
                61.26740947,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Min. Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "F to Enter",
                0.6923447172,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x3",
                "Rao's V",
                102.7871602,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "F to Enter",
                0.8110815946,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x4",
                "Rao's V",
                103.5078869,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Min. Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "F to Enter",
                0.1435076214,
            ),
            (
                "Variables Not in the Analysis",
                "4 / x5",
                "Rao's V",
                101.268244,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[
        ("1", "x4", ""),
        ("2", "x1", ""),
        ("3", "x2", ""),
        ("4", "", "x4"),
    ];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn the_other_orders_mahal_agrees_with_r() {
    let t = run_on(
        DATA,
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /ANALYSIS=ALL /METHOD=MAHAL /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. D Squared",
                0.9056954201,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                19.5628229,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. D Squared",
                0.4273957349,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                1.26074387,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. D Squared",
                0.02705686877,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. D Squared",
                4.261678543e-05,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                0.6922801955,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. D Squared",
                0.01895711002,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Min. D Squared",
                0.9056954201,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.5867776359),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 87.0),
            ("Wilks' Lambda", "1", "F / Statistic", 30.63370473),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 87.0),
            ("Wilks' Lambda", "1", "F / Sig.", 8.483978431e-11),
            ("Variables in the Analysis", "1 / x1", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x1",
                "F to Remove",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                17.96568429,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. D Squared",
                1.792487376,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                1.022261575,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. D Squared",
                1.059844389,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                13.04293569,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. D Squared",
                1.522810194,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                0.1074269284,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. D Squared",
                0.9148042656,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Min. D Squared",
                1.792487376,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.4138629564),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 87.0),
            ("Wilks' Lambda", "2", "F / Statistic", 23.84057506),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 172.0),
            ("Wilks' Lambda", "2", "F / Sig.", 1.060569796e-15),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                28.66840668,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Min. D Squared",
                0.4273957349,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                17.96568429,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Min. D Squared",
                0.9056954201,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                0.6923447172,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. D Squared",
                1.913608688,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.8110815946,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. D Squared",
                1.839426301,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                0.1435076214,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. D Squared",
                1.817679132,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[("1", "x1", ""), ("2", "x2", "")];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn the_other_orders_maxminf_agrees_with_r() {
    let t = run_on(
        DATA,
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /ANALYSIS=ALL /METHOD=MAXMINF /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. F",
                13.5854313,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                19.5628229,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. F",
                6.410936024,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                1.26074387,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. F",
                0.4058530315,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. F",
                0.0006392517815,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                0.6922801955,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. F",
                0.2843566504,
            ),
            ("Variables Entered/Removed", "1", "Min. F", 13.5854313),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.5867776359),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 87.0),
            ("Wilks' Lambda", "1", "F / Statistic", 30.63370473),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 87.0),
            ("Wilks' Lambda", "1", "F / Sig.", 8.483978431e-11),
            ("Variables in the Analysis", "1 / x1", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x1",
                "F to Remove",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                17.96568429,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. F",
                13.28913055,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                1.022261575,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. F",
                7.857467026,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                13.04293569,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. F",
                11.28979971,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                0.1074269284,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. F",
                6.782169556,
            ),
            ("Variables Entered/Removed", "2", "Min. F", 13.28913055),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.4138629564),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 87.0),
            ("Wilks' Lambda", "2", "F / Statistic", 23.84057506),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 172.0),
            ("Wilks' Lambda", "2", "F / Sig.", 1.060569796e-15),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                28.66840668,
            ),
            ("Variables in the Analysis", "2 / x1", "Min. F", 6.410936024),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                17.96568429,
            ),
            ("Variables in the Analysis", "2 / x2", "Min. F", 13.5854313),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                0.6923447172,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. F",
                9.34808842,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.8110815946,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. F",
                8.985703196,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                0.1435076214,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. F",
                8.879467024,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[("1", "x1", ""), ("2", "x2", "")];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn the_other_orders_minresid_agrees_with_r() {
    let t = run_on(
        DATA,
        "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /ANALYSIS=ALL /METHOD=MINRESID /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Sum of Unexplained Variation",
                2.08847721,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                19.5628229,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Sum of Unexplained Variation",
                2.326739534,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                1.26074387,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Sum of Unexplained Variation",
                2.938877493,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                30.65224445,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Sum of Unexplained Variation",
                2.132302286,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                0.6922801955,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Sum of Unexplained Variation",
                2.965972718,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Sum of Unexplained Variation",
                2.08847721,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.5867776359),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 87.0),
            ("Wilks' Lambda", "1", "F / Statistic", 30.63370473),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 87.0),
            ("Wilks' Lambda", "1", "F / Sig.", 8.483978431e-11),
            ("Variables in the Analysis", "1 / x1", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x1",
                "F to Remove",
                30.63370473,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9998796215,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                17.96568429,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Sum of Unexplained Variation",
                1.67470921,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9997543456,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                1.022261575,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Sum of Unexplained Variation",
                2.055974043,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.589553149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                13.04293569,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Sum of Unexplained Variation",
                1.764981354,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.9945087673,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                0.1074269284,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Sum of Unexplained Variation",
                2.084993892,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Sum of Unexplained Variation",
                1.67470921,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.4138629564),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 87.0),
            ("Wilks' Lambda", "2", "F / Statistic", 23.84057506),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 172.0),
            ("Wilks' Lambda", "2", "F / Sig.", 1.060569796e-15),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                28.66840668,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Sum of Unexplained Variation",
                2.326739534,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9998796215,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                17.96568429,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Sum of Unexplained Variation",
                2.08847721,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.9974723438,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                0.6923447172,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Sum of Unexplained Variation",
                1.656603059,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.3314131682,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                0.8110815946,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Sum of Unexplained Variation",
                1.655506499,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.9900683784,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                0.1435076214,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Sum of Unexplained Variation",
                1.670902453,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[("1", "x1", ""), ("2", "x2", "")];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn weights_missing_values_and_select_wilks_agrees_with_r() {
    let t = run_on(
        DATAM,
        "WEIGHT BY w.\nDISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /SELECT=sel(1) /ANALYSIS=ALL /METHOD=WILKS /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                74.68831591,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Wilks' Lambda",
                0.4691221128,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                37.50235783,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Wilks' Lambda",
                0.6376666328,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                2.619368577,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Wilks' Lambda",
                0.9618275622,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                67.08423155,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Wilks' Lambda",
                0.4959265214,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                3.628696382,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Wilks' Lambda",
                0.9478850449,
            ),
            (
                "Variables Entered/Removed",
                "1",
                "Wilks' Lambda",
                0.4691221128,
            ),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.4691221128),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 132.0),
            ("Wilks' Lambda", "1", "F / Statistic", 74.68831591),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 132.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.017760888e-22),
            ("Variables in the Analysis", "1 / x1", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x1",
                "F to Remove",
                74.68831591,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9813999496,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                36.17078724,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Wilks' Lambda",
                0.3022254398,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9781328475,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9781328475,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                4.001952149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Wilks' Lambda",
                0.4421098607,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.6556964186,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.6556964186,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                22.7860453,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Wilks' Lambda",
                0.3480447933,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.999999654,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.999999654,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                1.700379513,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Wilks' Lambda",
                0.4572518579,
            ),
            (
                "Variables Entered/Removed",
                "2",
                "Wilks' Lambda",
                0.3022254398,
            ),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.3022254398),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 132.0),
            ("Wilks' Lambda", "2", "F / Statistic", 53.64499158),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 262.0),
            ("Wilks' Lambda", "2", "F / Sig.", 5.491711814e-33),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                72.69870517,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Wilks' Lambda",
                0.6376666328,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                36.17078724,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Wilks' Lambda",
                0.4691221128,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9364716311,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.9364716311,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                6.310145544,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Wilks' Lambda",
                0.2754818888,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.3623573437,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.3623573437,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                1.286641324,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Wilks' Lambda",
                0.2963591637,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9868048215,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.9684505372,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                1.310650518,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Wilks' Lambda",
                0.2962518605,
            ),
            (
                "Variables Entered/Removed",
                "3",
                "Wilks' Lambda",
                0.2754818888,
            ),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.2754818888),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 132.0),
            ("Wilks' Lambda", "3", "F / Statistic", 39.22778586),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 260.0),
            ("Wilks' Lambda", "3", "F / Sig.", 8.013525876e-34),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.9505517797,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                76.11529097,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Wilks' Lambda",
                0.5980724137,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9395995788,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                39.31589922,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Wilks' Lambda",
                0.4421098607,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9364716311,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                6.310145544,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Wilks' Lambda",
                0.3022254398,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.3555381479,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.3555381479,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                1.346035278,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Wilks' Lambda",
                0.2698504436,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Tolerance",
                0.9865999437,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Min. Tolerance",
                0.9270964639,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "F to Enter",
                1.191489848,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Wilks' Lambda",
                0.2704852922,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[("1", "x1", ""), ("2", "x2", ""), ("3", "x3", "")];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}

#[test]
fn weights_missing_values_and_select_rao_agrees_with_r() {
    let t = run_on(
        DATAM,
        "WEIGHT BY w.\nDISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 x5 /SELECT=sel(1) /ANALYSIS=ALL /METHOD=RAO /PIN=0.15 /POUT=0.10.",
    );
    check(
        &t,
        &[
            ("Variables Not in the Analysis", "0 / x1", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "F to Enter",
                74.68831591,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x1",
                "Rao's V",
                149.3766318,
            ),
            ("Variables Not in the Analysis", "0 / x2", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "F to Enter",
                37.50235783,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x2",
                "Rao's V",
                75.00471566,
            ),
            ("Variables Not in the Analysis", "0 / x3", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "F to Enter",
                2.619368577,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x3",
                "Rao's V",
                5.238737154,
            ),
            ("Variables Not in the Analysis", "0 / x4", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "F to Enter",
                67.08423155,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x4",
                "Rao's V",
                134.1684631,
            ),
            ("Variables Not in the Analysis", "0 / x5", "Tolerance", 1.0),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Min. Tolerance",
                1.0,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "F to Enter",
                3.628696382,
            ),
            (
                "Variables Not in the Analysis",
                "0 / x5",
                "Rao's V",
                7.257392765,
            ),
            ("Variables Entered/Removed", "1", "Rao's V", 149.3766318),
            ("Wilks' Lambda", "1", "Number of Variables", 1.0),
            ("Wilks' Lambda", "1", "Lambda", 0.4691221128),
            ("Wilks' Lambda", "1", "df1", 1.0),
            ("Wilks' Lambda", "1", "df2", 2.0),
            ("Wilks' Lambda", "1", "df3", 132.0),
            ("Wilks' Lambda", "1", "F / Statistic", 74.68831591),
            ("Wilks' Lambda", "1", "F / df1", 2.0),
            ("Wilks' Lambda", "1", "F / df2", 132.0),
            ("Wilks' Lambda", "1", "F / Sig.", 2.017760888e-22),
            ("Variables in the Analysis", "1 / x1", "Tolerance", 1.0),
            (
                "Variables in the Analysis",
                "1 / x1",
                "F to Remove",
                74.68831591,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Min. Tolerance",
                0.9813999496,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "F to Enter",
                36.17078724,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x2",
                "Rao's V",
                244.9827663,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Tolerance",
                0.9781328475,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Min. Tolerance",
                0.9781328475,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "F to Enter",
                4.001952149,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x3",
                "Rao's V",
                166.4407765,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Tolerance",
                0.6556964186,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Min. Tolerance",
                0.6556964186,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "F to Enter",
                22.7860453,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x4",
                "Rao's V",
                203.9687255,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Tolerance",
                0.999999654,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Min. Tolerance",
                0.999999654,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "F to Enter",
                1.700379513,
            ),
            (
                "Variables Not in the Analysis",
                "1 / x5",
                "Rao's V",
                156.6727934,
            ),
            ("Variables Entered/Removed", "2", "Rao's V", 244.9827663),
            ("Wilks' Lambda", "2", "Number of Variables", 2.0),
            ("Wilks' Lambda", "2", "Lambda", 0.3022254398),
            ("Wilks' Lambda", "2", "df1", 2.0),
            ("Wilks' Lambda", "2", "df2", 2.0),
            ("Wilks' Lambda", "2", "df3", 132.0),
            ("Wilks' Lambda", "2", "F / Statistic", 53.64499158),
            ("Wilks' Lambda", "2", "F / df1", 4.0),
            ("Wilks' Lambda", "2", "F / df2", 262.0),
            ("Wilks' Lambda", "2", "F / Sig.", 5.491711814e-33),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "F to Remove",
                72.69870517,
            ),
            (
                "Variables in the Analysis",
                "2 / x1",
                "Rao's V",
                75.00471566,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Tolerance",
                0.9813999496,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "F to Remove",
                36.17078724,
            ),
            (
                "Variables in the Analysis",
                "2 / x2",
                "Rao's V",
                149.3766318,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Tolerance",
                0.9364716311,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Min. Tolerance",
                0.9364716311,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "F to Enter",
                6.310145544,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x3",
                "Rao's V",
                277.6175016,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Tolerance",
                0.3623573437,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Min. Tolerance",
                0.3623573437,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "F to Enter",
                1.286641324,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x4",
                "Rao's V",
                248.3554769,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Tolerance",
                0.9868048215,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Min. Tolerance",
                0.9684505372,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "F to Enter",
                1.310650518,
            ),
            (
                "Variables Not in the Analysis",
                "2 / x5",
                "Rao's V",
                249.689286,
            ),
            ("Variables Entered/Removed", "3", "Rao's V", 277.6175016),
            ("Wilks' Lambda", "3", "Number of Variables", 3.0),
            ("Wilks' Lambda", "3", "Lambda", 0.2754818888),
            ("Wilks' Lambda", "3", "df1", 3.0),
            ("Wilks' Lambda", "3", "df2", 2.0),
            ("Wilks' Lambda", "3", "df3", 132.0),
            ("Wilks' Lambda", "3", "F / Statistic", 39.22778586),
            ("Wilks' Lambda", "3", "F / df1", 6.0),
            ("Wilks' Lambda", "3", "F / df2", 260.0),
            ("Wilks' Lambda", "3", "F / Sig.", 8.013525876e-34),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Tolerance",
                0.9505517797,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "F to Remove",
                76.11529097,
            ),
            (
                "Variables in the Analysis",
                "3 / x1",
                "Rao's V",
                86.19755626,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Tolerance",
                0.9395995788,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "F to Remove",
                39.31589922,
            ),
            (
                "Variables in the Analysis",
                "3 / x2",
                "Rao's V",
                166.4407765,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Tolerance",
                0.9364716311,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "F to Remove",
                6.310145544,
            ),
            (
                "Variables in the Analysis",
                "3 / x3",
                "Rao's V",
                244.9827663,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Tolerance",
                0.3555381479,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Min. Tolerance",
                0.3555381479,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "F to Enter",
                1.346035278,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x4",
                "Rao's V",
                281.5731487,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Tolerance",
                0.9865999437,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Min. Tolerance",
                0.9270964639,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "F to Enter",
                1.191489848,
            ),
            (
                "Variables Not in the Analysis",
                "3 / x5",
                "Rao's V",
                282.0476825,
            ),
        ],
    );
    let table = t
        .iter()
        .find(|t| t.title == "Variables Entered/Removed")
        .expect("the steps");
    let steps: Vec<(String, String, String)> = table
        .rows
        .iter()
        .map(|r| (cell_text(&r[0]), cell_text(&r[1]), cell_text(&r[2])))
        .collect();
    let want: &[(&str, &str, &str)] = &[("1", "x1", ""), ("2", "x2", ""), ("3", "x3", "")];
    assert_eq!(steps.len(), want.len());
    for (got, want) in steps.iter().zip(want) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *want);
    }
}
