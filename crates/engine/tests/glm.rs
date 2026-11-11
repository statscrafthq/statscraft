//! UNIANOVA and GLM on synthetic data. The expected numbers come from an independent fit in a
//! general statistics package: least squares with sum-to-zero codings for the Type II and III
//! tests, sequential fits for Type I, refits on replicated cases for frequency weights, its
//! marginal means and pairwise comparisons, hat values for the saved diagnostics, and for the
//! repeated measures model the multivariate linear model with polynomial contrasts written out
//! as matrices. PSPP comparisons are in reference_models.rs.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
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

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers of every row joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    for table in tables.iter().filter(|t| t.title == title) {
        let Some(r) = row_keys(table).iter().position(|k| k == row) else {
            continue;
        };
        let c = table
            .column_titles()
            .iter()
            .position(|t| t == column)
            .unwrap_or_else(|| {
                panic!("{title}: no column {column} in {:?}", table.column_titles())
            });
        return &table.rows[r][c];
    }
    panic!("no table {title} with row {row}");
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0)
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// y, the factors a (three levels) and b (two), a covariate x and a whole-number weight w;
/// 40 cases, unbalanced.
const DATA: &str = "DATA LIST FREE /y a b x w.
BEGIN DATA.
10.84 1 1 8.4 2
7.26 2 1 9.3 1
13.1 3 2 9.5 1
13.39 3 1 13.8 2
11.06 2 2 9.0 1
14.9 3 2 14.1 1
11.97 2 1 11.7 1
9.62 2 1 7.1 1
14.14 3 1 14.4 1
8.81 2 1 8.3 2
8.15 1 2 6.6 3
10.46 1 2 11.5 1
7.06 1 1 7.4 3
12.11 2 1 10.1 1
11.24 3 1 8.5 2
12.65 3 1 8.4 2
11.69 3 2 8.8 1
8.82 1 1 8.4 1
8.58 1 1 9.6 2
14.18 3 1 10.1 2
13.82 3 2 6.9 1
11.49 1 1 11.4 2
8.58 1 2 11.2 2
13.7 3 2 13.1 1
15.56 3 2 9.2 3
7.02 1 2 8.0 1
14.23 3 2 13.0 1
12.92 2 1 16.2 2
15.75 3 1 10.0 1
14.41 3 1 12.8 1
12.77 3 2 10.1 1
9.35 2 2 11.7 2
11.62 3 2 9.6 3
12.92 3 2 9.0 1
10.64 2 2 10.1 1
12.61 3 1 10.2 1
12.39 1 1 12.7 1
12.67 2 2 10.9 1
13.36 3 2 9.2 1
8.99 2 2 9.0 3
END DATA.";

/// A repeated measures design: a group g and three measures of 24 cases.
const RM_DATA: &str = "DATA LIST FREE /g t1 t2 t3.
BEGIN DATA.
1 14.28 15.25 19.04
1 9.43 9.41 10.75
1 9.58 12.83 11.69
1 13.98 12.36 13.51
1 8.93 10.7 12.37
1 8.76 9.1 9.32
1 4.32 9.09 7.81
1 10.41 14.7 12.7
1 9.78 8.85 9.19
1 10.05 10.08 12.06
1 5.92 9.8 7.1
1 9.08 8.54 12.13
2 9.41 11.07 14.82
2 11.05 14.22 13.52
2 14.37 13.36 15.7
2 8.01 11.04 9.45
2 11.22 11.21 14.67
2 5.84 7.58 5.39
2 15.19 14.63 15.51
2 9.47 11.27 12.27
2 9.42 12.97 12.95
2 7.17 11.67 13.72
2 11.42 12.43 11.96
2 9.27 9.74 10.33
END DATA.";

/// (case, table, row, column, value)
const EXPECTED: &[(&str, &str, &str, &str, f64)] = &[
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Type III Sum of Squares",
        4409.241434861177,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "F",
        1700.848580209374,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Sig.",
        1.2727985029924911e-30,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Partial Eta Squared",
        0.9804017478021646,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Observed Power",
        1.0,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a",
        "Type III Sum of Squares",
        134.10381178129109,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a",
        "F",
        25.865024771098227,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a",
        "Sig.",
        1.4855227932504225e-07,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a",
        "Partial Eta Squared",
        0.6034062714116927,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a",
        "Observed Power",
        0.9999971126408703,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "b",
        "Type III Sum of Squares",
        1.7939351644381674,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "b",
        "F",
        0.6920038565586946,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "b",
        "Sig.",
        0.41128833282558386,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "b",
        "Partial Eta Squared",
        0.019947070783801608,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "b",
        "Observed Power",
        0.12759858117958112,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a * b",
        "Type III Sum of Squares",
        2.993757809924105,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a * b",
        "F",
        0.5774155028392606,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a * b",
        "Sig.",
        0.5667562814640212,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a * b",
        "Partial Eta Squared",
        0.03284985228607649,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "a * b",
        "Observed Power",
        0.1379126021563094,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Error",
        "Type III Sum of Squares",
        88.14083189393945,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Corrected Model",
        "Type III Sum of Squares",
        135.70444560606057,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Corrected Total",
        "Type III Sum of Squares",
        223.8452775,
    ),
    (
        "factorial",
        "Tests of Between-Subjects Effects",
        "Total",
        "Type III Sum of Squares",
        5625.5185,
    ),
    (
        "factorial",
        "Levene's Test of Equality of Error Variances",
        "Based on Mean",
        "Levene Statistic",
        1.8022886114055,
    ),
    (
        "factorial",
        "Levene's Test of Equality of Error Variances",
        "Based on Mean",
        "Sig.",
        0.13878578572325095,
    ),
    (
        "factorial",
        "Levene's Test of Equality of Error Variances",
        "Based on Median",
        "Levene Statistic",
        1.689645719331773,
    ),
    (
        "factorial",
        "Levene's Test of Equality of Error Variances",
        "Based on Median and with adjusted df",
        "df2",
        33.099126464646226,
    ),
    (
        "factorial",
        "Descriptive Statistics",
        "2 / 1",
        "Mean",
        10.448333333333334,
    ),
    (
        "factorial",
        "Descriptive Statistics",
        "3 / Total",
        "Std. Deviation",
        1.2499791811131795,
    ),
    (
        "factorial",
        "Descriptive Statistics",
        "Total / 2",
        "N",
        20.0,
    ),
    (
        "type1",
        "Tests of Between-Subjects Effects",
        "a",
        "Type I Sum of Squares",
        131.48803343301438,
    ),
    (
        "type1",
        "Tests of Between-Subjects Effects",
        "b",
        "Type I Sum of Squares",
        1.2226543631221432,
    ),
    (
        "type1",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Type I Sum of Squares",
        5401.6732225,
    ),
    (
        "type2",
        "Tests of Between-Subjects Effects",
        "a",
        "Type II Sum of Squares",
        132.2376252961365,
    ),
    (
        "type2",
        "Tests of Between-Subjects Effects",
        "a * b",
        "Type II Sum of Squares",
        2.993757809924105,
    ),
    (
        "ancova",
        "Tests of Between-Subjects Effects",
        "x",
        "Type III Sum of Squares",
        26.749280816384505,
    ),
    (
        "ancova",
        "Tests of Between-Subjects Effects",
        "a",
        "Type III Sum of Squares",
        108.42503535645918,
    ),
    (
        "ancova",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Type III Sum of Squares",
        87.11285238219656,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "Intercept",
        "B",
        9.505916202885883,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "x",
        "B",
        0.3831548601622688,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "x",
        "Std. Error",
        0.10104518348977568,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "[a=1]",
        "t",
        -5.646802901051088,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "[a=1] * [b=1]",
        "Sig.",
        0.21582841530401795,
    ),
    (
        "ancova",
        "Parameter Estimates",
        "[a=2]",
        "95% Confidence Interval / Lower Bound",
        -4.345921529720669,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "1",
        "Mean",
        9.493367037487555,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "1",
        "Std. Error",
        0.4466019913099714,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "2",
        "Mean",
        10.471219487906524,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "2",
        "Std. Error",
        0.4130035951942355,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "3",
        "Mean",
        13.334574041408851,
    ),
    (
        "ancova",
        "Estimated Marginal Means: a",
        "3",
        "Std. Error",
        0.3193723162183903,
    ),
    (
        "ancova",
        "Pairwise Comparisons",
        "1 / 3",
        "Mean Difference (I-J)",
        -3.841207003921295,
    ),
    (
        "ancova",
        "Pairwise Comparisons",
        "1 / 3",
        "Sig.",
        1.9413786100495058e-07,
    ),
    (
        "ancova",
        "Pairwise Comparisons",
        "1 / 2",
        "Sig.",
        0.35376145940498743,
    ),
    (
        "ancova",
        "Pairwise Comparisons",
        "1 / 2",
        "95% Confidence Interval for Difference / Lower Bound",
        -2.5140771015380032,
    ),
    (
        "weighted",
        "Tests of Between-Subjects Effects",
        "a",
        "Type III Sum of Squares",
        184.77217823632324,
    ),
    (
        "weighted",
        "Tests of Between-Subjects Effects",
        "a",
        "F",
        49.88036133923954,
    ),
    (
        "weighted",
        "Tests of Between-Subjects Effects",
        "Error",
        "df",
        54.0,
    ),
    (
        "regwgt",
        "Tests of Between-Subjects Effects",
        "a",
        "Type III Sum of Squares",
        184.772178236323,
    ),
    (
        "regwgt",
        "Tests of Between-Subjects Effects",
        "x",
        "F",
        13.800880177263016,
    ),
    (
        "ancova",
        "Lack of Fit Tests",
        "Pure Error",
        "Sum of Squares",
        6.602650000000002,
    ),
    (
        "ancova",
        "Lack of Fit Tests",
        "Lack of Fit",
        "Sum of Squares",
        54.78890107755491,
    ),
    (
        "ancova",
        "Lack of Fit Tests",
        "Lack of Fit",
        "F",
        0.8298016868614101,
    ),
    (
        "ancova",
        "Contrast Results (K Matrix)",
        "Level 2 vs. Level 1 / Contrast Estimate",
        "y",
        0.9778524504189672,
    ),
    (
        "ancova",
        "Contrast Results (K Matrix)",
        "Level 3 vs. Level 1 / Std. Error",
        "y",
        0.5544731205102675,
    ),
    (
        "ancova",
        "Contrast Results (K Matrix)",
        "Level 3 vs. Level 1 / Sig.",
        "y",
        6.471262033498352e-08,
    ),
    (
        "ancova",
        "Test Results",
        "Contrast",
        "Sum of Squares",
        108.42503535645918,
    ),
    (
        "nested",
        "Tests of Between-Subjects Effects",
        "b(a)",
        "Type I Sum of Squares",
        4.216412173046256,
    ),
    (
        "nested",
        "Tests of Between-Subjects Effects",
        "b(a)",
        "df",
        3.0,
    ),
    (
        "noint",
        "Tests of Between-Subjects Effects",
        "x",
        "Type III Sum of Squares",
        5379.201082507846,
    ),
    (
        "noint",
        "Tests of Between-Subjects Effects",
        "Model",
        "Type III Sum of Squares",
        5379.201082507846,
    ),
    (
        "noint",
        "Tests of Between-Subjects Effects",
        "Error",
        "Type III Sum of Squares",
        246.31741749215362,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Pillai's Trace",
        "Value",
        0.5987674843209218,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Wilks' Lambda",
        "Value",
        0.40123251567907836,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Hotelling's Trace",
        "Value",
        1.4923204399511818,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Roy's Largest Root",
        "Value",
        1.4923204399511818,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Wilks' Lambda",
        "F",
        15.669364619487409,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time / Pillai's Trace",
        "Sig.",
        6.849506279710347e-05,
    ),
    (
        "rm",
        "Multivariate Tests",
        "time * g / Wilks' Lambda",
        "Value",
        0.9849711075424883,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time / Sphericity Assumed",
        "Type III Sum of Squares",
        58.105211111110606,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time / Sphericity Assumed",
        "F",
        15.374234517632075,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.5924333333333379,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "Error(time) / Sphericity Assumed",
        "Type III Sum of Squares",
        83.14655555555544,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time / Greenhouse-Geisser",
        "df",
        1.9912121585598062,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time / Greenhouse-Geisser",
        "Sig.",
        8.98455795928884e-06,
    ),
    (
        "rm",
        "Tests of Within-Subjects Effects",
        "time / Lower-bound",
        "Sig.",
        0.000731121962793214,
    ),
    (
        "rm",
        "Mauchly's Test of Sphericity",
        "time",
        "Mauchly's W",
        0.9955866875348184,
    ),
    (
        "rm",
        "Mauchly's Test of Sphericity",
        "time",
        "Approx. Chi-Square",
        0.0928846774168633,
    ),
    (
        "rm",
        "Mauchly's Test of Sphericity",
        "time",
        "Sig.",
        0.9546196035823286,
    ),
    (
        "rm",
        "Mauchly's Test of Sphericity",
        "time",
        "Epsilon / Greenhouse-Geisser",
        0.9956060792799031,
    ),
    (
        "rm",
        "Tests of Within-Subjects Contrasts",
        "time / Linear",
        "Type III Sum of Squares",
        55.46999999999946,
    ),
    (
        "rm",
        "Tests of Within-Subjects Contrasts",
        "time * g / Quadratic",
        "F",
        0.0035864765353162457,
    ),
    (
        "rm",
        "Tests of Within-Subjects Contrasts",
        "Error(time) / Quadratic",
        "Type III Sum of Squares",
        44.319263888888884,
    ),
    (
        "rm",
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Type III Sum of Squares",
        8805.087338888889,
    ),
    (
        "rm",
        "Tests of Between-Subjects Effects",
        "g",
        "F",
        0.7272779676497141,
    ),
    (
        "rm",
        "Tests of Between-Subjects Effects",
        "Error",
        "Type III Sum of Squares",
        388.78381111111145,
    ),
];

/// (case index, PRED RESID ZRESID SRESID COOK LEVER SEPRED DRESID)
const SAVED: &[(usize, [f64; 8])] = &[
    (
        0,
        [
            9.384389758130503,
            1.4556102418694972,
            1.0672047991311373,
            1.1751262568274603,
            0.041916307318947535,
            0.17524211023850156,
            0.5709743124733445,
            1.764893988817042,
        ],
    ),
    (
        1,
        [
            10.007705244146726,
            -2.7477052441467262,
            -2.0145256874429722,
            -2.2164760835807904,
            0.14776458435512183,
            0.17392492210586813,
            0.5688244320692047,
            -3.326217335053009,
        ],
    ),
    (
        6,
        [
            10.927276908536168,
            1.042723091463832,
            0.7644897345225091,
            0.8417990256826645,
            0.02150952366776167,
            0.17524211023850159,
            0.5709743124733445,
            1.2642778012894962,
        ],
    ),
];

/// The syntax of each case of EXPECTED.
const CASES: &[(&str, &str)] = &[
    (
        "factorial",
        "UNIANOVA y BY a b /PRINT=DESCRIPTIVE HOMOGENEITY ETASQ OPOWER.",
    ),
    ("type1", "UNIANOVA y BY a b /METHOD=SSTYPE(1)."),
    ("type2", "UNIANOVA y BY a b /METHOD=SSTYPE(2)."),
    (
        "ancova",
        "UNIANOVA y BY a b WITH x /PRINT=PARAMETER LOF /EMMEANS=TABLES(a) COMPARE ADJ(BONFERRONI) \
         /CONTRAST(a)=SIMPLE(1) /SAVE=PRED RESID ZRESID SRESID COOK LEVER SEPRED DRESID.",
    ),
    ("weighted", "WEIGHT BY w.\nUNIANOVA y BY a b WITH x."),
    ("regwgt", "UNIANOVA y BY a b WITH x /REGWGT=w."),
    (
        "nested",
        "UNIANOVA y BY a b /METHOD=SSTYPE(1) /DESIGN=a b WITHIN a.",
    ),
    ("noint", "UNIANOVA y WITH x /INTERCEPT=EXCLUDE."),
    (
        "rm",
        "GLM t1 t2 t3 BY g /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=g.",
    ),
];

#[test]
fn every_statistic_matches_the_independent_fit() {
    let mut failures = Vec::new();
    for &(case, syntax) in CASES {
        let data = if case == "rm" { RM_DATA } else { DATA };
        let mut engine = Engine::default();
        let tables = run(&mut engine, &format!("{data}\n{syntax}"));
        for &(c, title, row, column, expected) in EXPECTED.iter().filter(|e| e.0 == case) {
            let actual = number(cell(&tables, title, row, column));
            if !close(actual, expected) {
                failures.push(format!(
                    "{c}: {title} / {row} / {column}: {actual} vs {expected}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn saved_values_match_the_independent_diagnostics() {
    let mut engine = Engine::default();
    run(&mut engine, &format!("{DATA}\n{}", CASES[3].1));
    let names = [
        "PRE_1", "RES_1", "ZRE_1", "SRE_1", "COO_1", "LEV_1", "SEP_1", "DRE_1",
    ];
    for &(case, expected) in SAVED {
        for (name, want) in names.iter().zip(expected) {
            let got = values(&engine, name)[case].unwrap();
            assert!(close(got, want), "{name}[{case}]: {got} vs {want}");
        }
    }
}

#[test]
fn type_four_is_type_three_without_empty_cells_and_contrasts_agree_with_lmatrix() {
    let mut engine = Engine::default();
    let three = run(&mut engine, &format!("{DATA}\nUNIANOVA y BY a b WITH x."));
    let four = run(&mut engine, "UNIANOVA y BY a b WITH x /METHOD=SSTYPE(4).");
    for source in ["x", "a", "b", "a * b"] {
        let t3 = number(cell(
            &three,
            "Tests of Between-Subjects Effects",
            source,
            "Type III Sum of Squares",
        ));
        let t4 = number(cell(
            &four,
            "Tests of Between-Subjects Effects",
            source,
            "Type IV Sum of Squares",
        ));
        assert!(close(t3, t4), "{source}: {t3} vs {t4}");
    }
    // a1 - a2 of the marginal means, written as an L matrix
    let tables = run(
        &mut engine,
        "UNIANOVA y BY a b WITH x /LMATRIX='a1 vs a2' a 1 -1 0 a*b 1/2 1/2 -1/2 -1/2 0 0 \
         /EMMEANS=TABLES(a) COMPARE.",
    );
    let l = number(cell(
        &tables,
        "Contrast Results (K Matrix)",
        "L1 / Contrast Estimate",
        "y",
    ));
    let compare = number(cell(
        &tables,
        "Pairwise Comparisons",
        "1 / 2",
        "Mean Difference (I-J)",
    ));
    assert!(close(l, compare), "{l} vs {compare}");
    // a K matrix moves the hypothesis
    let tables = run(
        &mut engine,
        "UNIANOVA y BY a b WITH x /LMATRIX a 1 -1 0 a*b 1/2 1/2 -1/2 -1/2 0 0 /KMATRIX=-1.",
    );
    let diff = number(cell(
        &tables,
        "Contrast Results (K Matrix)",
        "L1 / Difference (Estimate - Hypothesized)",
        "y",
    ));
    assert!(close(diff, l + 1.0), "{diff} vs {}", l + 1.0);
}

#[test]
fn post_hoc_tests_equal_oneway_for_one_factor() {
    let mut engine = Engine::default();
    let ours = run(
        &mut engine,
        &format!("{DATA}\nUNIANOVA y BY a /POSTHOC=a(LSD BONFERRONI T2 SNK)."),
    );
    let oneway = run(&mut engine, "ONEWAY y BY a /POSTHOC=LSD BONFERRONI T2 SNK.");
    for title in ["Multiple Comparisons: y", "Homogeneous Subsets: y"] {
        let a = ours.iter().find(|t| t.title == title).unwrap();
        let b = oneway.iter().find(|t| t.title == title).unwrap();
        assert_eq!(a.rows.len(), b.rows.len(), "{title}");
        for (x, y) in a.rows.iter().zip(&b.rows) {
            for (p, q) in x.iter().zip(y) {
                match (p, q) {
                    (Cell::Number(p), Cell::Number(q)) => {
                        assert!(close(*p, *q), "{title}: {p} vs {q}")
                    }
                    (p, q) => assert_eq!(p, q, "{title}"),
                }
            }
        }
    }
    // with two factors the tests of separate variances are left out with a warning
    run(&mut engine, "UNIANOVA y BY a b /POSTHOC=a(GH LSD).");
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w == "POSTHOC: GH needs a model with one factor and is not run"),
        "{:?}",
        engine.warnings()
    );
}

#[test]
fn errors_name_the_problem() {
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y x BY a.")),
        "GLM with several dependent variables (the multivariate model) is not supported yet; run UNIANOVA for each dependent variable"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y x BY a /WSFACTOR=t 3.")),
        "WSFACTOR gives 3 level combinations for 2 dependent variables; they must be equal"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nUNIANOVA y BY a /DESIGN=a b.")),
        "b in the design is neither a factor nor a covariate of the model"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nUNIANOVA y BY a /SAVE=WPRED.")),
        "SAVE WPRED and WRESID need a REGWGT variable"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nUNIANOVA y BY a b /LMATRIX=a 1 -1.")),
        "LMATRIX gives 2 coefficients where the effect has 3 parameters"
    );
}
