//! UNIANOVA options against R's `lm` on 66 invented cases (a three-level factor a and a two-level
//! factor b, unbalanced, a covariate x, frequency weights w, missing values in y and x): the sums
//! of squares of Types I, II, III and IV with the partial eta squared, noncentrality and observed
//! power (the hypothesis tests as model comparisons on sum-to-zero codings: sequential fits for
//! Type I, each effect adjusted for those that do not contain it for Type II, for all the others
//! for Type III, which Type IV equals when no cell is empty); the parameter estimates with the
//! last level as reference and their intervals (the noncentrality parameter of a parameter is |t|,
//! its power by the noncentral t); Levene's test based on the mean, of the residuals of the
//! model (so that a covariate is accounted for, as the engine does it) and the cell statistics; frequency weights (the cases written out) and a regression weight
//! (weighted least squares); an intercept left out; and a nested effect (b within a, tested on the
//! unweighted mean of its cells). The intercept row follows the manual: Type I fits it first, Type II
//! adjusts it for the covariate only (it is contained in the pure factor effects but not in an effect
//! with a covariate), Type III for everything. Scripts in
//! lab/oracles/uniano-r.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y a b x w (5F8.2).
BEGIN DATA.
28.7 2 2 20.2 2
12.28 1 1 28.5 2
31.59 1 2 39.4 3
24.43 1 2 28.8 3
. 1 2 25.3 1
32.47 3 2 31.1 2
26.19 3 2 25.2 2
27.23 3 1 20.1 2
22.8 3 1 27.3 1
28.84 3 1 28.4 1
16.92 3 1 . 2
27.21 2 2 24.3 2
25.19 2 1 26 1
24.22 3 1 31.9 3
32.2 1 2 41.5 3
26.36 3 1 27.1 1
26.08 3 2 26.1 2
21.52 1 2 34.1 3
19.6 1 1 28.4 2
26.04 3 2 18.9 2
25.36 2 1 31 1
30.45 1 2 44.6 3
23.52 1 1 39.5 1
23.23 1 1 30.3 1
22.27 3 1 24.2 1
16.28 1 2 27.7 2
21.87 2 1 27.3 3
25.77 2 1 37.1 2
34.49 3 2 27.3 2
30.28 2 2 30.9 1
31.27 3 1 37.6 1
24.34 1 1 37.8 3
25.88 2 2 36 1
35.22 3 2 39.7 1
20.01 1 1 23.3 1
31.23 3 1 40.7 2
28.44 1 1 43.3 1
16.84 3 1 18.6 3
25.02 2 2 29.8 3
. 1 2 19.7 1
24.36 3 1 24.6 1
29.15 3 1 34.4 2
19.79 1 1 32.3 2
21.8 1 1 30.8 3
24.6 2 1 28.3 1
25.35 3 2 25.3 1
22.14 3 2 21.6 2
27.06 1 1 33.1 1
24.41 2 2 25.9 1
22.97 1 2 30.5 2
28.93 2 2 32.7 3
21.58 3 1 31 1
20.31 2 1 23.2 1
25.44 1 1 35.1 1
30.57 1 2 36.3 2
34.93 3 2 39 1
25.29 1 1 31 2
28.85 2 1 36.7 1
21.74 2 2 28 1
15.27 1 1 28.2 2
19.38 2 1 28.2 1
18.95 1 1 18.4 2
29 2 1 43.5 1
23.5 3 1 23.8 1
29.41 2 2 26.8 2
19.47 2 1 17.6 1
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell
                    && !t.is_empty()
                {
                    carried[level] = t.clone();
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

/// Each expectation: the table title, the row, the column and the value.
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
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-7 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn type_one_sums_of_squares_with_effect_sizes_and_power_agree_with_r() {
    let t =
        run("UNIANOVA y BY a b WITH x /METHOD=SSTYPE(1) /PRINT=ETASQ OPOWER /DESIGN=a b a*b x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type I Sum of Squares",
                136.9652039,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                68.48260193,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 6.705285549),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.002451023415,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.1932064653,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                13.4105711,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9011806152,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type I Sum of Squares",
                212.8854494,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                212.8854494,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 20.84409305),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                2.774223304e-05,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.2712517283,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                20.84409305,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9942315738,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type I Sum of Squares",
                6.387539614,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                3.193769807,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.3127091835,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.7327315493,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.01104483437,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.6254183669,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.09737510084,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type I Sum of Squares",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 50.36414313),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                2.394800799e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.473506782,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                50.36414313,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999997298,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type I Sum of Squares",
                40076.59779,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                40076.59779,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                3923.989808,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                1.518614218e-53,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.9859296122,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                3923.989808,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                1.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type I Sum of Squares",
                571.9406997,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 56.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                10.21322678,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type I Sum of Squares",
                41519.1571,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type I Sum of Squares",
                1442.559308,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                62.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type I Sum of Squares",
                870.6186083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                145.1031014,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                14.20737094,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                9.324344309e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6035236149,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                85.24422565,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.9999999895,
            ),
        ],
    );
}

#[test]
fn type_two_sums_of_squares_with_effect_sizes_and_power_agree_with_r() {
    let t =
        run("UNIANOVA y BY a b WITH x /METHOD=SSTYPE(2) /PRINT=ETASQ OPOWER /DESIGN=a b a*b x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type II Sum of Squares",
                297.5530402,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                148.7765201,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 14.56704363),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                8.060596801e-06,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.3422141259,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                29.13408726,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9983694944,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type II Sum of Squares",
                175.0787398,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                175.0787398,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 17.14235311),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.0001179505903,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.2343697239,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                17.14235311,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9825011632,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type II Sum of Squares",
                4.557630752,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                2.278815376,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.2231239377,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.8007234661,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.007905713705,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.4462478755,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.08324497515,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type II Sum of Squares",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 50.36414313),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                2.394800799e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.473506782,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                50.36414313,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999997298,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type II Sum of Squares",
                585.9505865,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                585.9505865,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                57.37173952,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                3.889614971e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.5060497419,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                57.37173952,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                0.9999999788,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type II Sum of Squares",
                571.9406997,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 56.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                10.21322678,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type II Sum of Squares",
                41519.1571,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type II Sum of Squares",
                1442.559308,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                62.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type II Sum of Squares",
                870.6186083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                145.1031014,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                14.20737094,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                9.324344309e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6035236149,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                85.24422565,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.9999999895,
            ),
        ],
    );
}

#[test]
fn type_four_sums_of_squares_with_effect_sizes_and_power_agree_with_r() {
    let t =
        run("UNIANOVA y BY a b WITH x /METHOD=SSTYPE(4) /PRINT=ETASQ OPOWER /DESIGN=a b a*b x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type IV Sum of Squares",
                295.5795769,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                147.7897884,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 14.47043051),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                8.590101653e-06,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.3407177733,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                28.94086103,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9982768193,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type IV Sum of Squares",
                173.8422969,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                173.8422969,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 17.0212902),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.0001238465667,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.233100376,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                17.0212902,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9818700111,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type IV Sum of Squares",
                4.557630752,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                2.278815376,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.2231239377,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.8007234661,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.007905713705,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.4462478755,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.08324497515,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type IV Sum of Squares",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 50.36414313),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                2.394800799e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.473506782,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                50.36414313,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999997298,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type IV Sum of Squares",
                336.308963,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                336.308963,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                32.92876681,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                4.034428755e-07,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.370282508,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                32.92876681,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                0.9998823403,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type IV Sum of Squares",
                571.9406997,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 56.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                10.21322678,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type IV Sum of Squares",
                41519.1571,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type IV Sum of Squares",
                1442.559308,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                62.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type IV Sum of Squares",
                870.6186083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                145.1031014,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                14.20737094,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                9.324344309e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6035236149,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                85.24422565,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.9999999895,
            ),
        ],
    );
}

#[test]
fn type_three_parameters_levene_and_descriptives_agree_with_r() {
    let t = run(
        "UNIANOVA y BY a b WITH x /PRINT=ETASQ OPOWER PARAMETER HOMOGENEITY DESCRIPTIVE /DESIGN=a b a*b x.",
    );
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                295.5795769,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                147.7897884,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 14.47043051),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                8.590101653e-06,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.3407177733,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                28.94086103,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9982768193,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                173.8422969,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                173.8422969,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 17.0212902),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.0001238465667,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.233100376,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                17.0212902,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9818700111,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type III Sum of Squares",
                4.557630752,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                2.278815376,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.2231239377,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.8007234661,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.007905713705,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.4462478755,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.08324497515,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type III Sum of Squares",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                514.3804154,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 50.36414313),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                2.394800799e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.473506782,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                50.36414313,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999997298,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                336.308963,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                336.308963,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                32.92876681,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                4.034428755e-07,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.370282508,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                32.92876681,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                0.9998823403,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                571.9406997,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 56.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                10.21322678,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type III Sum of Squares",
                41519.1571,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                1442.559308,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                62.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                870.6186083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                145.1031014,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                14.20737094,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                9.324344309e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6035236149,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                85.24422565,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.9999999895,
            ),
            ("Parameter Estimates", "Intercept", "B", 16.14704196),
            (
                "Parameter Estimates",
                "Intercept",
                "Std. Error",
                2.126992693,
            ),
            ("Parameter Estimates", "Intercept", "t", 7.591489154),
            ("Parameter Estimates", "Intercept", "Sig.", 3.6450991e-10),
            (
                "Parameter Estimates",
                "Intercept",
                "95% Confidence Interval / Lower Bound",
                11.88616358,
            ),
            (
                "Parameter Estimates",
                "Intercept",
                "95% Confidence Interval / Upper Bound",
                20.40792033,
            ),
            (
                "Parameter Estimates",
                "Intercept",
                "Partial Eta Squared",
                0.5071754705,
            ),
            (
                "Parameter Estimates",
                "Intercept",
                "Noncent. Parameter",
                7.591489154,
            ),
            (
                "Parameter Estimates",
                "Intercept",
                "Observed Power",
                0.9999999807,
            ),
            ("Parameter Estimates", "[a=1]", "B", -6.253608378),
            ("Parameter Estimates", "[a=1]", "Std. Error", 1.62071517),
            ("Parameter Estimates", "[a=1]", "t", -3.85854868),
            ("Parameter Estimates", "[a=1]", "Sig.", 0.0002973121507),
            (
                "Parameter Estimates",
                "[a=1]",
                "95% Confidence Interval / Lower Bound",
                -9.500291,
            ),
            (
                "Parameter Estimates",
                "[a=1]",
                "95% Confidence Interval / Upper Bound",
                -3.006925757,
            ),
            (
                "Parameter Estimates",
                "[a=1]",
                "Partial Eta Squared",
                0.2100258766,
            ),
            (
                "Parameter Estimates",
                "[a=1]",
                "Noncent. Parameter",
                3.85854868,
            ),
            (
                "Parameter Estimates",
                "[a=1]",
                "Observed Power",
                0.9664950313,
            ),
            ("Parameter Estimates", "[a=2]", "B", -2.390558899),
            ("Parameter Estimates", "[a=2]", "Std. Error", 1.506523927),
            ("Parameter Estimates", "[a=2]", "t", -1.586804468),
            ("Parameter Estimates", "[a=2]", "Sig.", 0.118187966),
            (
                "Parameter Estimates",
                "[a=2]",
                "95% Confidence Interval / Lower Bound",
                -5.408488974,
            ),
            (
                "Parameter Estimates",
                "[a=2]",
                "95% Confidence Interval / Upper Bound",
                0.627371176,
            ),
            (
                "Parameter Estimates",
                "[a=2]",
                "Partial Eta Squared",
                0.04302865168,
            ),
            (
                "Parameter Estimates",
                "[a=2]",
                "Noncent. Parameter",
                1.586804468,
            ),
            (
                "Parameter Estimates",
                "[a=2]",
                "Observed Power",
                0.344665379,
            ),
            ("Parameter Estimates", "[b=1]", "B", -3.944277416),
            ("Parameter Estimates", "[b=1]", "Std. Error", 1.385855596),
            ("Parameter Estimates", "[b=1]", "t", -2.846095529),
            ("Parameter Estimates", "[b=1]", "Sig.", 0.006173557161),
            (
                "Parameter Estimates",
                "[b=1]",
                "95% Confidence Interval / Lower Bound",
                -6.720479776,
            ),
            (
                "Parameter Estimates",
                "[b=1]",
                "95% Confidence Interval / Upper Bound",
                -1.168075056,
            ),
            (
                "Parameter Estimates",
                "[b=1]",
                "Partial Eta Squared",
                0.126368595,
            ),
            (
                "Parameter Estimates",
                "[b=1]",
                "Noncent. Parameter",
                2.846095529,
            ),
            (
                "Parameter Estimates",
                "[b=1]",
                "Observed Power",
                0.7987079664,
            ),
            ("Parameter Estimates", "x", "B", 0.4625752258),
            ("Parameter Estimates", "x", "Std. Error", 0.0651810939),
            ("Parameter Estimates", "x", "t", 7.096769909),
            ("Parameter Estimates", "x", "Sig.", 2.394800799e-09),
            (
                "Parameter Estimates",
                "x",
                "95% Confidence Interval / Lower Bound",
                0.3320018044,
            ),
            (
                "Parameter Estimates",
                "x",
                "95% Confidence Interval / Upper Bound",
                0.5931486472,
            ),
            (
                "Parameter Estimates",
                "x",
                "Partial Eta Squared",
                0.473506782,
            ),
            (
                "Parameter Estimates",
                "x",
                "Noncent. Parameter",
                7.096769909,
            ),
            ("Parameter Estimates", "x", "Observed Power", 0.9999997298),
            ("Parameter Estimates", "[a=1] * [b=1]", "B", 1.29990817),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "Std. Error",
                1.999750183,
            ),
            ("Parameter Estimates", "[a=1] * [b=1]", "t", 0.6500352799),
            ("Parameter Estimates", "[a=1] * [b=1]", "Sig.", 0.5183272792),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "95% Confidence Interval / Lower Bound",
                -2.706072824,
            ),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "95% Confidence Interval / Upper Bound",
                5.305889164,
            ),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "Partial Eta Squared",
                0.007488954259,
            ),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "Noncent. Parameter",
                0.6500352799,
            ),
            (
                "Parameter Estimates",
                "[a=1] * [b=1]",
                "Observed Power",
                0.09792423592,
            ),
            ("Parameter Estimates", "[a=2] * [b=1]", "B", 0.3414208596),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "Std. Error",
                2.02113168,
            ),
            ("Parameter Estimates", "[a=2] * [b=1]", "t", 0.168925589),
            ("Parameter Estimates", "[a=2] * [b=1]", "Sig.", 0.8664638654),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "95% Confidence Interval / Lower Bound",
                -3.707392421,
            ),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "95% Confidence Interval / Upper Bound",
                4.39023414,
            ),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "Partial Eta Squared",
                0.0005093093044,
            ),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "Noncent. Parameter",
                0.168925589,
            ),
            (
                "Parameter Estimates",
                "[a=2] * [b=1]",
                "Observed Power",
                0.05316444019,
            ),
            (
                "Levene's Test of Equality of Error Variances",
                "Based on Mean",
                "Levene Statistic",
                0.7674394237,
            ),
            (
                "Levene's Test of Equality of Error Variances",
                "Based on Mean",
                "df1",
                5.0,
            ),
            (
                "Levene's Test of Equality of Error Variances",
                "Based on Mean",
                "df2",
                57.0,
            ),
            (
                "Levene's Test of Equality of Error Variances",
                "Based on Mean",
                "Sig.",
                0.5771154883,
            ),
            ("Descriptive Statistics", "1 / 1", "Mean", 21.78714286),
            (
                "Descriptive Statistics",
                "1 / 1",
                "Std. Deviation",
                4.490309958,
            ),
            ("Descriptive Statistics", "1 / 1", "N", 14.0),
            ("Descriptive Statistics", "1 / 2", "Mean", 26.25125),
            (
                "Descriptive Statistics",
                "1 / 2",
                "Std. Deviation",
                5.807201041,
            ),
            ("Descriptive Statistics", "1 / 2", "N", 8.0),
            ("Descriptive Statistics", "2 / 1", "Mean", 23.98),
            (
                "Descriptive Statistics",
                "2 / 1",
                "Std. Deviation",
                3.579189852,
            ),
            ("Descriptive Statistics", "2 / 1", "N", 10.0),
            ("Descriptive Statistics", "2 / 2", "Mean", 26.84222222),
            (
                "Descriptive Statistics",
                "2 / 2",
                "Std. Deviation",
                2.796246492,
            ),
            ("Descriptive Statistics", "2 / 2", "N", 9.0),
            ("Descriptive Statistics", "3 / 1", "Mean", 25.35769231),
            (
                "Descriptive Statistics",
                "3 / 1",
                "Std. Deviation",
                4.174286673,
            ),
            ("Descriptive Statistics", "3 / 1", "N", 13.0),
            ("Descriptive Statistics", "3 / 2", "Mean", 29.21222222),
            (
                "Descriptive Statistics",
                "3 / 2",
                "Std. Deviation",
                5.014917192,
            ),
            ("Descriptive Statistics", "3 / 2", "N", 9.0),
            ("Descriptive Statistics", "1 / Total", "Mean", 23.41045455),
            (
                "Descriptive Statistics",
                "1 / Total",
                "Std. Deviation",
                5.343600959,
            ),
            ("Descriptive Statistics", "1 / Total", "N", 22.0),
            ("Descriptive Statistics", "2 / Total", "Mean", 25.33578947),
            (
                "Descriptive Statistics",
                "2 / Total",
                "Std. Deviation",
                3.469334415,
            ),
            ("Descriptive Statistics", "2 / Total", "N", 19.0),
            ("Descriptive Statistics", "3 / Total", "Mean", 26.93454545),
            (
                "Descriptive Statistics",
                "3 / Total",
                "Std. Deviation",
                4.827034708,
            ),
            ("Descriptive Statistics", "3 / Total", "N", 22.0),
            ("Descriptive Statistics", "Total / 1", "Mean", 23.63432432),
            (
                "Descriptive Statistics",
                "Total / 1",
                "Std. Deviation",
                4.327184445,
            ),
            ("Descriptive Statistics", "Total / 1", "N", 37.0),
            ("Descriptive Statistics", "Total / 2", "Mean", 27.48076923),
            (
                "Descriptive Statistics",
                "Total / 2",
                "Std. Deviation",
                4.65857053,
            ),
            ("Descriptive Statistics", "Total / 2", "N", 26.0),
            (
                "Descriptive Statistics",
                "Total / Total",
                "Mean",
                25.22174603,
            ),
            (
                "Descriptive Statistics",
                "Total / Total",
                "Std. Deviation",
                4.823596751,
            ),
            ("Descriptive Statistics", "Total / Total", "N", 63.0),
        ],
    );
}

#[test]
fn frequency_weights_agree_with_r() {
    let t = run("WEIGHT BY w.\nUNIANOVA y BY a b WITH x /PRINT=ETASQ OPOWER /DESIGN=a b a*b x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                559.1069717,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                279.5534858,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 27.78641081),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                2.32842285e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.3526802468,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                55.57282162,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9999997269,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                399.8332989,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                399.8332989,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 39.74170548),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                7.495368476e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.2803811718,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                39.74170548,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9999908185,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type III Sum of Squares",
                8.924029194,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                4.462014597,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.4435050068,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.643014686,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.008621205082,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.8870100136,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.1204641168,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type III Sum of Squares",
                911.7676293,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                911.7676293,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 90.62577002),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                9.416731603e-16,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.4704758351,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                90.62577002,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                1.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                495.7272996,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                495.7272996,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                49.27315557,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                2.527235619e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.3257230629,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                49.27315557,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                0.9999997017,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                1026.201468,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 102.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                10.0607987,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type III Sum of Squares",
                71644.6668,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 109.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                2641.857167,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                108.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                1615.655699,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                269.2759499,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                26.76486807,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                6.000983688e-19,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6115605793,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                160.5892084,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                1.0,
            ),
        ],
    );
}

#[test]
fn a_regression_weight_agree_with_r() {
    let t = run("UNIANOVA y BY a b WITH x /REGWGT=w /PRINT=ETASQ OPOWER /DESIGN=a b a*b x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                559.1069717,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                279.5534858,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 15.25528437),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                5.144321185e-06,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.3526802468,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                30.51056874,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9989031472,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                399.8332989,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                399.8332989,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 21.81897556),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                1.920438956e-05,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Partial Eta Squared",
                0.2803811718,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Noncent. Parameter",
                21.81897556,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Observed Power",
                0.9957264924,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type III Sum of Squares",
                8.924029194,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                4.462014597,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                0.2434929449,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.7847105822,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Partial Eta Squared",
                0.008621205082,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Noncent. Parameter",
                0.4869858898,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Observed Power",
                0.08642184306,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type III Sum of Squares",
                911.7676293,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                911.7676293,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 49.75532472),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                2.820813617e-09,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.4704758351,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                49.75532472,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999996637,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                495.7272996,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                495.7272996,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                27.05192855,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                2.901381534e-06,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.3257230629,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                27.05192855,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                0.9991842485,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                1026.201468,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 56.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                18.32502621,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type III Sum of Squares",
                71644.6668,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                2641.857167,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                62.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                1615.655699,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                6.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                269.2759499,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                14.69443737,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                5.387971694e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.6115605793,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                88.16662422,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.9999999954,
            ),
        ],
    );
}

#[test]
fn an_intercept_left_out_agree_with_r() {
    let t = run("UNIANOVA y BY a WITH x /INTERCEPT=EXCLUDE /PRINT=ETASQ OPOWER /DESIGN=a x.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                914.2736997,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 3.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                304.7578999,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 23.92398173),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                2.959649255e-10,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.5488328944,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                71.77194518,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.9999999887,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Type III Sum of Squares",
                554.0170338,
            ),
            ("Tests of Between-Subjects Effects", "x", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Mean Square",
                554.0170338,
            ),
            ("Tests of Between-Subjects Effects", "x", "F", 43.49122171),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Sig.",
                1.307693028e-08,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Partial Eta Squared",
                0.4243409434,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Noncent. Parameter",
                43.49122171,
            ),
            (
                "Tests of Between-Subjects Effects",
                "x",
                "Observed Power",
                0.9999969751,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                751.5770702,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 59.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                12.73859441,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type III Sum of Squares",
                41519.1571,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 63.0),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Type III Sum of Squares",
                40767.58003,
            ),
            ("Tests of Between-Subjects Effects", "Model", "df", 4.0),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Mean Square",
                10191.89501,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "F",
                800.0800307,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Sig.",
                1.200925278e-50,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Partial Eta Squared",
                0.9818980653,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Noncent. Parameter",
                3200.320123,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Model",
                "Observed Power",
                1.0,
            ),
        ],
    );
}

#[test]
fn a_nested_effect_agree_with_r() {
    let t = run("UNIANOVA y BY a b /PRINT=ETASQ OPOWER /DESIGN=a b(a).");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                93.0281396,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                46.5140698,
            ),
            ("Tests of Between-Subjects Effects", "a", "F", 2.340979523),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.1052634342,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Partial Eta Squared",
                0.07469388509,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Noncent. Parameter",
                4.681959046,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Observed Power",
                0.4556210118,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Type III Sum of Squares",
                249.0943069,
            ),
            ("Tests of Between-Subjects Effects", "b(a)", "df", 3.0),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Mean Square",
                83.03143563,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "F",
                4.178840755,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Sig.",
                0.009545704698,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Partial Eta Squared",
                0.177730938,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Noncent. Parameter",
                12.53652227,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b(a)",
                "Observed Power",
                0.8305557055,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                39581.70585,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Mean Square",
                39581.70585,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                1992.084616,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                1.325579271e-46,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Partial Eta Squared",
                0.9717084848,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Noncent. Parameter",
                1992.084616,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Observed Power",
                1.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                1152.430434,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 58.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                19.86949025,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Total",
                "Type III Sum of Squares",
                41805.4435,
            ),
            ("Tests of Between-Subjects Effects", "Total", "df", 64.0),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                1510.401436,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                63.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                357.9710016,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                5.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Mean Square",
                71.59420032,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                3.603222802,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.006614444184,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Partial Eta Squared",
                0.2370038806,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Noncent. Parameter",
                18.01611401,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Observed Power",
                0.8983145089,
            ),
        ],
    );
}
