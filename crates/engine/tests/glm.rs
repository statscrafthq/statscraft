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

/// A dependent without variance has no R squared: the caption shows a period, not NaN.
#[test]
fn constant_dependent_has_no_r_squared() {
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nCOMPUTE k = 3.\nUNIANOVA k BY a."),
    );
    let caption = tables
        .iter()
        .find(|t| t.title == "Tests of Between-Subjects Effects")
        .and_then(|t| t.caption.clone())
        .unwrap();
    assert!(
        caption.contains("R Squared = . (Adjusted R Squared = .)"),
        "{caption}"
    );
}

#[test]
fn errors_name_the_problem() {
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

/// With the cell a = 3, b = 1 empty, the F test of a at b = 1 rests on the one estimable
/// difference (a = 1 against a = 2) instead of coming out empty.
#[test]
fn emmeans_simple_effect_uses_the_estimable_differences() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        "DATA LIST FREE /plot a b y.
BEGIN DATA
1 1 1 12.3 1 1 2 13.5 1 2 1 13.4 1 2 2 15.5 1 3 2 14.6 2 1 1 13.2 2 1 2 14.7 2 2 1 14.7
2 2 2 17.2 2 3 2 16.3 3 1 1 14.4 3 1 2 14.8 3 2 1 14.0 3 2 2 17.9 3 3 2 17.3 4 1 1 15.2
4 1 2 14.3 4 2 1 14.8 4 2 2 17.5 4 3 2 17.4
END DATA.
UNIANOVA y BY a b /DESIGN=a b a*b /EMMEANS=TABLES(a*b) COMPARE(a).",
    );
    assert_eq!(
        number(cell(&t, "Univariate Tests", "b = 1: Contrast", "df")),
        1.0
    );
    assert_eq!(
        number(cell(&t, "Univariate Tests", "b = 2: Contrast", "df")),
        2.0
    );
}

fn contrast_numbers(syntax: &str) -> Vec<f64> {
    let mut engine = Engine::default();
    let tables = run(&mut engine, &format!("{DATA}\n{syntax}"));
    let table = tables
        .iter()
        .find(|t| t.title == "Contrast Results (K Matrix)")
        .expect("a contrast table");
    table
        .rows
        .iter()
        .flatten()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

// The manual's example: SPECIAL(1 1 1 1 -1 0 0 1 -1) is the matrix with the constant first and the
// contrasts (1 -1 0) and (0 1 -1) after it, which are the REPEATED contrasts; 2 -1 -1 is the
// LMATRIX contrast a 2 -1 -1.
#[test]
fn contrast_special_is_the_matrix_read_column_by_column() {
    let repeated = contrast_numbers("UNIANOVA y BY a /CONTRAST(a)=REPEATED.");
    let special = contrast_numbers("UNIANOVA y BY a /CONTRAST(a)=SPECIAL(1 1 1 1 -1 0 0 1 -1).");
    assert!(!repeated.is_empty());
    assert_eq!(repeated.len(), special.len());
    for (a, b) in repeated.iter().zip(&special) {
        assert!((a - b).abs() < 1e-9 * a.abs().max(1.0), "{a} vs {b}");
    }
    // a contrast of 2 -1 -1 against the mean as the first row: estimate = 2 m1 - m2 - m3
    let own = contrast_numbers("UNIANOVA y BY a /CONTRAST(a)=SPECIAL(1 1 1 2 -1 -1 0 1 -1).");
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{DATA}\nUNIANOVA y BY a /LMATRIX=\"c\" a 2 -1 -1."),
    );
    let estimate = tables
        .iter()
        .flat_map(|t| t.rows.iter().flatten())
        .find_map(|c| match c {
            Cell::Number(n) if (*n - own[0]).abs() < 1e-9 => Some(*n),
            _ => None,
        });
    assert!(
        estimate.is_some(),
        "the LMATRIX contrast estimate {} is not in its tables",
        own[0]
    );
    // the wrong number of values names what is accepted
    let error = run_error(&format!(
        "{DATA}\nUNIANOVA y BY a /CONTRAST(a)=SPECIAL(1 1 1 1)."
    ));
    assert!(error.contains("SPECIAL has 4 values"), "{error}");
}

/// The cell of the row whose first text cell is `row`, in the column called `column`.
fn named_cell(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables.iter().find(|t| t.title.starts_with(title)).unwrap();
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    let r = table
        .rows
        .iter()
        .position(|r| {
            r.iter()
                .any(|cell| matches!(cell, Cell::Text(t) if t == row))
        })
        .unwrap_or_else(|| panic!("{title}: no row {row}"));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

// With VS the pairwise tests take the mean square and df of another effect as the error term, so
// each standard error is the one of the model's error scaled by sqrt(MS(effect) / MS(error)), and
// the table says so.
#[test]
fn posthoc_vs_takes_the_mean_square_of_another_effect_as_the_error_term() {
    let mut engine = Engine::default();
    let base = run(
        &mut engine,
        &format!("{DATA}\nUNIANOVA y BY a b /POSTHOC=a(LSD)."),
    );
    let vs = run(&mut engine, "UNIANOVA y BY a b /POSTHOC=a(LSD) VS b.");
    let ms_b = named_cell(&vs, "Tests of Between-Subjects Effects", "b", "Mean Square");
    let ms_error = named_cell(
        &vs,
        "Tests of Between-Subjects Effects",
        "Error",
        "Mean Square",
    );
    let table = |tables: &[Table]| {
        tables
            .iter()
            .find(|t| t.title.starts_with("Multiple Comparisons"))
            .unwrap()
            .clone()
    };
    let (a, b) = (table(&base), table(&vs));
    let se = a
        .column_titles()
        .iter()
        .position(|t| t == "Std. Error")
        .unwrap();
    let diff = a
        .column_titles()
        .iter()
        .position(|t| t == "Mean Difference (I-J)")
        .unwrap();
    assert_eq!(a.rows.len(), b.rows.len());
    let scale = (ms_b / ms_error).sqrt();
    for (x, y) in a.rows.iter().zip(&b.rows) {
        let (Cell::Number(sx), Cell::Number(sy)) = (&x[se], &y[se]) else {
            continue;
        };
        assert!(
            (sx * scale - sy).abs() < 1e-9 * sy.abs().max(1.0),
            "{sx} {sy} {scale}"
        );
        // the differences of the means do not depend on the error term
        assert_eq!(x[diff], y[diff]);
    }
    let caption = b.caption.clone().unwrap_or_default();
    assert!(
        caption.contains("The error term is the mean square of b"),
        "{caption}"
    );
    // the error term cannot be the intercept or a main effect named on the same POSTHOC
    let error = run_error(&format!("{DATA}\nUNIANOVA y BY a b /POSTHOC=a(LSD) VS a."));
    assert!(error.contains("cannot take a"), "{error}");
    let error = run_error(&format!("{DATA}\nUNIANOVA y BY a b /POSTHOC=a(LSD) VS c."));
    assert!(error.contains("c"), "{error}");
}

// The error term of VS is the one of each dependent in turn: for y2 = 3 y every standard error of
// y2 is three times that of y.
#[test]
fn posthoc_vs_uses_the_mean_square_of_each_dependent() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{DATA}\nCOMPUTE y2 = 3 * y.\nGLM y y2 BY a b /POSTHOC=a(LSD) VS b."),
    );
    let table = |dependent: &str| {
        tables
            .iter()
            .find(|t| t.title == format!("Multiple Comparisons: {dependent}"))
            .unwrap_or_else(|| panic!("no table for {dependent}"))
    };
    let (one, three) = (table("y"), table("y2"));
    let se = one
        .column_titles()
        .iter()
        .position(|t| t == "Std. Error")
        .unwrap();
    let mut compared = 0;
    for (x, y) in one.rows.iter().zip(&three.rows) {
        if let (Cell::Number(a), Cell::Number(b)) = (&x[se], &y[se]) {
            assert!((3.0 * a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} {b}");
            compared += 1;
        }
    }
    assert!(compared > 0);
}

// A balanced 3 x 2 design, four cases a cell. Expected values from anova(lm(y ~ a * b)) in R: the
// mean squares are those of a, b, a:b; the error terms are worked out from them with
// Satterthwaite's approximation (T1), a given mean square and df (T2) and fractions (T3).
const TEST_DATA: &str = "DATA LIST FREE / y a b.
BEGIN DATA
11.52 1 1
10.76 1 1
12.19 1 1
11.9 1 1
13.44 1 2
12.49 1 2
10.52 1 2
11.59 1 2
14 2 1
13.88 2 1
12.67 2 1
12.12 2 1
12.98 2 2
12.55 2 2
12.63 2 2
12.77 2 2
14.07 3 1
12.86 3 1
12.8 3 1
12.62 3 1
14.94 3 2
13.57 3 2
14.71 3 2
14.69 3 2
END DATA.
";

#[test]
fn test_tests_an_effect_against_a_combination_a_value_or_fractions() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{TEST_DATA}UNIANOVA y BY a b /TEST=a VS b + a*b /TEST=a VS 4.5 DF(10)\n\
             /TEST=a VS 1/2*b + 0.5*a*b DF(7) /DESIGN=a b a*b."
        ),
    );
    let tests: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title.starts_with("Tests of Hypothesis"))
        .collect();
    assert_eq!(tests.len(), 3);
    let value = |t: &Table, r: usize, c: usize| match &t.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    };
    let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-8 * b.abs().max(1.0), "{a} vs {b}");
    // T1: MS(b) + MS(a*b) = 2.92355 on 2.87999827729518 df, F = 2.70813879244982
    near(value(tests[0], 0, 3), 7.917_379_166_666_66);
    near(value(tests[0], 1, 3), 2.923_55);
    near(value(tests[0], 1, 2), 2.879_998_277_295_18);
    near(value(tests[0], 0, 4), 2.708_138_792_449_82);
    near(value(tests[0], 0, 5), 0.217_938_229_491_985);
    // T2: the value 4.5 on 10 df
    near(value(tests[1], 1, 3), 4.5);
    near(value(tests[1], 1, 2), 10.0);
    near(value(tests[1], 0, 4), 1.759_417_592_592_59);
    near(value(tests[1], 0, 5), 0.221_464_251_602_36);
    // T3: half of each, 7 df given
    near(value(tests[2], 1, 3), 1.461_775);
    near(value(tests[2], 0, 4), 5.416_277_584_899_89);
    near(value(tests[2], 0, 5), 0.037_896_196_443_983_7);
    let caption = tests[0].caption.clone().unwrap_or_default();
    assert!(
        caption.contains("Hypothesis: a") && caption.contains("Satterthwaite"),
        "{caption}"
    );
}

#[test]
fn test_adds_duplicate_effects_and_names_what_it_cannot_use() {
    // 3*b - b - a*b is 2 b - a*b: 2 * 1.2558375 - 1.6677125 = 0.8439625
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{TEST_DATA}UNIANOVA y BY a b /TEST=a VS 3*b - b - a*b DF(5) /DESIGN=a b a*b."),
    );
    let t = tables
        .iter()
        .find(|t| t.title == "Tests of Hypothesis 1")
        .unwrap();
    let Cell::Number(ms) = t.rows[1][3] else {
        panic!()
    };
    assert!((ms - 0.843_962_5).abs() < 1e-8, "{ms}");
    // a negative combination, an unknown effect, several dependents
    let error = run_error(&format!(
        "{TEST_DATA}UNIANOVA y BY a b /TEST=a VS a*b - 5*b /DESIGN=a b a*b."
    ));
    assert!(error.contains("negative"), "{error}");
    let error = run_error(&format!(
        "{TEST_DATA}UNIANOVA y BY a b /TEST=a VS a*b*c /DESIGN=a b a*b."
    ));
    assert!(
        error.contains("not an effect of the design") || error.contains("c"),
        "{error}"
    );
    let error = run_error(&format!(
        "{TEST_DATA}COMPUTE y2 = y + a.\nGLM y y2 BY a b /TEST=a VS b /DESIGN=a b a*b."
    ));
    assert!(error.contains("univariate"), "{error}");
}

/// A balanced 3 x 2 design with two cases in every cell; the values do not matter for the L
/// matrices.
const BALANCED: &str = "DATA LIST FREE / y a b.
BEGIN DATA
1 1 1
2 1 1
3 1 2
5 1 2
4 2 1
6 2 1
7 2 2
8 2 2
9 3 1
8 3 1
11 3 2
12 3 2
END DATA.
";

fn parameter_rows(table: &Table) -> Vec<(String, Vec<f64>)> {
    table
        .rows
        .iter()
        .map(|r| {
            let Cell::Text(name) = &r[0] else {
                panic!("{r:?}")
            };
            let values = r[1..]
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => *n,
                    other => panic!("{other:?}"),
                })
                .collect();
            (name.clone(), values)
        })
        .collect()
}

// With one factor of three levels the free coefficients are the intercept and the first two
// levels; the third parameter is redundant and its coefficient is intercept - level 1 - level 2.
#[test]
fn print_gef_gives_the_general_form_of_the_estimable_functions() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{BALANCED}UNIANOVA y BY a /PRINT=GEF."),
    );
    let t = tables
        .iter()
        .find(|t| t.title == "General Estimable Function")
        .unwrap();
    assert_eq!(t.columns, ["Parameter", "L1", "L2", "L3"]);
    let rows = parameter_rows(t);
    let expected: Vec<(&str, [f64; 3])> = vec![
        ("Intercept", [1.0, 0.0, 0.0]),
        ("[a=1]", [0.0, 1.0, 0.0]),
        ("[a=2]", [0.0, 0.0, 1.0]),
        ("[a=3] (redundant)", [1.0, -1.0, -1.0]),
    ];
    assert_eq!(rows.len(), expected.len());
    for ((name, values), (want_name, want)) in rows.iter().zip(&expected) {
        assert_eq!(name, want_name);
        assert_eq!(values, want, "{name}");
    }
}

// In a balanced design the Type III L matrix of a is the contrast of its levels with the cells of
// the interaction at half the weight each, in the columns that are not redundant.
#[test]
fn print_test_lmatrix_gives_the_l_matrix_of_every_effect() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{BALANCED}UNIANOVA y BY a b /DESIGN=a b a*b /PRINT=TEST(LMATRIX)."),
    );
    let titles: Vec<&str> = tables
        .iter()
        .filter(|t| t.title.starts_with("Contrast Coefficients (L' Matrix)"))
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Contrast Coefficients (L' Matrix): Intercept",
            "Contrast Coefficients (L' Matrix): a",
            "Contrast Coefficients (L' Matrix): b",
            "Contrast Coefficients (L' Matrix): a * b",
        ]
    );
    let a = tables
        .iter()
        .find(|t| t.title == "Contrast Coefficients (L' Matrix): a")
        .unwrap();
    assert_eq!(a.columns, ["Parameter", "L1", "L2"]);
    let rows = parameter_rows(a);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).unwrap().1.clone();
    assert_eq!(row("Intercept"), [0.0, 0.0]);
    assert_eq!(row("[a=1]"), [1.0, 0.0]);
    assert_eq!(row("[a=2]"), [0.0, 1.0]);
    assert_eq!(row("[b=1]"), [0.0, 0.0]);
    assert_eq!(row("[a=1] * [b=1]"), [0.5, 0.0]);
    assert_eq!(row("[a=2] * [b=1]"), [0.0, 0.5]);
    assert!(a.caption.clone().unwrap_or_default().contains("Type III"));
    // ESTIMABLE is another name; a one-factor model has the intercept and a
    let estimable = run(&mut engine, "UNIANOVA y BY a /PRINT=TEST(ESTIMABLE).");
    assert_eq!(
        estimable
            .iter()
            .filter(|t| t.title.starts_with("Contrast Coefficients (L' Matrix)"))
            .count(),
        2
    );
}

#[test]
fn print_test_lmatrix_also_shows_the_matrices_of_contrast_and_lmatrix() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{BALANCED}UNIANOVA y BY a /CONTRAST(a)=REPEATED /LMATRIX=\"c\" a 1 -1 0 /PRINT=TEST(LMATRIX)."
        ),
    );
    let find = |title: &str| {
        tables
            .iter()
            .find(|t| t.title == title)
            .unwrap_or_else(|| panic!("no {title}"))
    };
    let repeated = find("Contrast Coefficients (L' Matrix): a Repeated Contrast");
    assert_eq!(repeated.columns.len(), 3);
    let rows = parameter_rows(repeated);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).unwrap().1.clone();
    // level 1 against level 2, level 2 against level 3
    assert_eq!(row("[a=1]"), [1.0, 0.0]);
    assert_eq!(row("[a=2]"), [-1.0, 1.0]);
    assert_eq!(row("Intercept"), [0.0, 0.0]);
    let custom = find("Contrast Coefficients (L' Matrix): c");
    let rows = parameter_rows(custom);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).unwrap().1.clone();
    assert_eq!(row("[a=1]"), [1.0]);
    assert_eq!(row("[a=2]"), [-1.0]);
}

const ROBUST_DATA: &str = "DATA LIST FREE /y a x w.
BEGIN DATA
3.207 1 1.55 1.6
8.481 2 9.68 1.56
7.32 3 4.68 1.61
7.333 1 7.77 1.7
6.6 2 4.08 1.63
7.266 3 5.39 1.45
3.987 1 2.07 0.87
5.429 2 1.87 1.33
13.997 3 7.8 0.85
4.11 1 1.94 0.89
8.898 2 4.34 1.93
3.445 3 0.02 1.79
10.477 1 8.35 0.91
7.291 2 8.3 1.33
12.006 3 9.57 1.82
16.605 1 9.48 0.68
8.799 2 6.01 1.32
5.568 3 2.62 0.85
6.059 1 6.43 1.4
9.221 2 5.26 1.57
7.105 3 8.81 1.47
6.754 1 6.07 0.78
11.029 2 7.4 0.86
14.979 3 8.03 0.61
9.794 1 9.02 0.68
3.326 2 0.93 1.43
8.665 3 5.75 0.75
5.57 1 5.75 0.72
11.109 2 9.44 0.78
5.38 3 2.19 1.36
5.81 1 4.78 0.5
3.734 2 0.45 0.99
12.003 3 7.99 0.62
4.766 1 3.68 1.41
8.461 2 5.63 0.99
3.831 3 2.47 1.69
END DATA.
";

fn robust_se(tables: &[Table], row: &str) -> f64 {
    named_cell(
        tables,
        "Robust Parameter Estimates",
        row,
        "Robust Std. Error",
    )
}

// The estimators written out from the Algorithms manual in a general statistics package, with
// the fit's own residuals and the weighted hat values of lm(): G X' diag(w omega) X G.
#[test]
fn robust_standard_errors_follow_hc0_to_hc4() {
    let mut engine = Engine::default();
    let cases = [
        ("HC0", [0.8370491124, 0.7714849893, 0.1232635049]),
        ("HC1", [0.8878246554, 0.8182834013, 0.1307406903]),
        ("HC2", [0.8907333699, 0.8198236825, 0.1319780772]),
        ("HC3", [0.9480120947, 0.8713639330, 0.1413285113]),
        ("HC4", [0.8948403415, 0.8227155465, 0.1334924820]),
    ];
    for (hc, want) in cases {
        let t = run(
            &mut engine,
            &format!("{ROBUST_DATA}UNIANOVA y BY a WITH x /ROBUST={hc} /DESIGN=a x."),
        );
        for (row, want) in ["[a=1]", "[a=2]", "x"].into_iter().zip(want) {
            let got = robust_se(&t, row);
            assert!(close(got, want), "{hc} {row}: {got} against {want}");
        }
        let b = named_cell(&t, "Robust Parameter Estimates", "x", "B");
        assert!(close(b, 0.9306395539));
        // the redundant level stays out
        let table = t
            .iter()
            .find(|t| t.title == "Robust Parameter Estimates")
            .unwrap();
        assert!(row_keys(table).contains(&"[a=3] (redundant)".to_string()));
    }
}

// Bare ROBUST is HC3. The Wald F of an effect uses the rows of its Type III hypothesis, and with
// one row it is the square of the robust t of that parameter. LMATRIX gets a second, robust set.
#[test]
fn robust_wald_tests_and_a_second_set_of_lmatrix_results() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{ROBUST_DATA}UNIANOVA y BY a WITH x /ROBUST /LMATRIX='a1 vs a3' a 1 0 -1 /DESIGN=a x."
        ),
    );
    let title = "Robust Tests of Between-Subjects Effects";
    assert!(close(named_cell(&t, title, "a", "F"), 1.37717635985));
    assert!(close(named_cell(&t, title, "a", "Hypothesis df"), 2.0));
    assert!(close(named_cell(&t, title, "a", "Sig."), 0.266840891071));
    assert!(close(named_cell(&t, title, "x", "F"), 43.361414941));
    assert!(close(named_cell(&t, title, "x", "Error df"), 32.0));
    let k = t
        .iter()
        .find(|x| x.title == "Robust Contrast Results (K Matrix)")
        .unwrap();
    // rows: estimate, hypothesized, difference, standard error, Sig.
    assert!(close(number(&k.rows[0][2]), -1.546175342));
    assert!(close(number(&k.rows[3][2]), 0.9480120947));
    let wald = t.iter().find(|x| x.title == "Robust Test Results").unwrap();
    let f = number(&wald.rows[0][1]);
    assert!(close(f, (1.546175342_f64 / 0.9480120947).powi(2)), "{f}");
    // the model-based results stay as they were
    assert!(t.iter().any(|x| x.title == "Contrast Results (K Matrix)"));
    assert!(
        t.iter()
            .any(|x| x.title == "Tests of Between-Subjects Effects")
    );
}

// With a regression weight the leverage includes the weight, as the weighted hat values of lm().
#[test]
fn robust_standard_errors_with_a_regression_weight() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{ROBUST_DATA}UNIANOVA y BY a WITH x /REGWGT=w /ROBUST=HC3 /DESIGN=a x."),
    );
    for (row, want) in [
        ("[a=1]", 0.7974155447),
        ("[a=2]", 0.8315477713),
        ("x", 0.1287443037),
    ] {
        let got = robust_se(&t, row);
        assert!(close(got, want), "{row}: {got} against {want}");
    }
    let t = run(
        &mut engine,
        &format!("{ROBUST_DATA}UNIANOVA y BY a WITH x /REGWGT=w /ROBUST=HC0 /DESIGN=a x."),
    );
    assert!(close(robust_se(&t, "x"), 0.1108898304));
}

#[test]
fn robust_needs_one_dependent_and_a_valid_estimator() {
    let mut engine = Engine::default();
    run(&mut engine, &format!("{ROBUST_DATA}COMPUTE y2 = y * 2."));
    let before = engine.warnings().len();
    let t = run(&mut engine, "GLM y y2 BY a /ROBUST.");
    assert!(t.iter().all(|t| !t.title.starts_with("Robust")));
    assert!(engine.warnings().len() > before);
    let (_, errors) = parse("UNIANOVA y BY a /ROBUST=HC9.");
    assert!(
        errors[0].to_string().contains("HC4"),
        "the choices are named"
    );
}

// A case alone in its cell has leverage 1 and a zero residual: it adds nothing to the sandwich,
// so HC2 to HC4 stay finite (the estimators written out in R, with that case's term set to 0).
#[test]
fn robust_standard_errors_survive_a_case_alone_in_its_cell() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        "DATA LIST FREE /y a.
BEGIN DATA
1.2 1 2.1 1 1.7 1 2.9 2 3.4 2 3.1 2 4.2 2 5.0 3 4.4 3 6.1 4
END DATA.
UNIANOVA y BY a /ROBUST=HC3 /DESIGN=a.",
    );
    for (row, want) in [
        ("[a=1]", 0.3188521078),
        ("[a=2]", 0.3299831646),
        ("[a=3]", 0.4242640687),
    ] {
        let got = robust_se(&t, row);
        assert!(close(got, want), "{row}: {got} against {want}");
    }
}

fn het(tables: &[Table], row: &str, column: &str) -> f64 {
    named_cell(tables, "Heteroskedasticity Tests", row, column)
}

// The tests of the Algorithms manual written out in a general statistics package: the squared
// residuals regressed on the auxiliary model with lm()'s own residuals and fitted values.
#[test]
fn heteroskedasticity_tests_with_the_default_and_given_auxiliary_models() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{ROBUST_DATA}UNIANOVA y BY a WITH x /PRINT=MBP BP F WHITE /DESIGN=a x."),
    );
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Statistic"),
        9.331726203
    ));
    assert!(close(het(&t, "Modified Breusch-Pagan Test", "df"), 2.0));
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Sig."),
        0.009411121966
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Statistic"),
        17.78047883
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Sig."),
        0.0001377266803
    ));
    assert!(close(het(&t, "F Test", "Statistic"), 5.773657624));
    assert!(close(het(&t, "F Test", "Error df"), 33.0));
    assert!(close(het(&t, "F Test", "Sig."), 0.007078230066));
    // White: the squares and cross products leave 7 independent columns of the design
    assert!(close(het(&t, "White's Test", "Statistic"), 11.59862256));
    assert!(close(het(&t, "White's Test", "df"), 6.0));
    // the models of the design subcommands
    let t = run(
        &mut engine,
        &format!(
            "{ROBUST_DATA}UNIANOVA y BY a WITH x /PRINT=MBP BP F /MBPDESIGN=x /BPDESIGN=x /FDESIGN=x /DESIGN=a x."
        ),
    );
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Statistic"),
        8.264767084
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Statistic"),
        15.74751691
    ));
    assert!(close(het(&t, "Breusch-Pagan Test", "df"), 1.0));
    assert!(close(het(&t, "F Test", "Statistic"), 10.13159261));
    assert!(close(het(&t, "F Test", "Error df"), 34.0));
    // by itself a design subcommand names the model of DESIGN
    let t = run(
        &mut engine,
        &format!(
            "{ROBUST_DATA}UNIANOVA y BY a WITH x /PRINT=MBP BP F /MBPDESIGN /BPDESIGN /FDESIGN /DESIGN=a x."
        ),
    );
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Statistic"),
        8.612992734
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Statistic"),
        16.41101889
    ));
    assert!(close(het(&t, "F Test", "Statistic"), 3.354580572));
    assert!(close(het(&t, "F Test", "df"), 3.0));
}

// With a regression weight the squared residuals are weighted (u = w e^2) and the fitted values
// are those of the weighted fit.
#[test]
fn heteroskedasticity_tests_with_a_regression_weight() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{ROBUST_DATA}UNIANOVA y BY a WITH x /REGWGT=w /PRINT=MBP BP F WHITE /DESIGN=a x."
        ),
    );
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Statistic"),
        10.15047789
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Statistic"),
        15.9446542
    ));
    assert!(close(het(&t, "F Test", "Statistic"), 6.479148225));
    assert!(close(het(&t, "White's Test", "Statistic"), 11.79219222));
}

#[test]
fn heteroskedasticity_tests_of_each_dependent_and_their_refusals() {
    let mut engine = Engine::default();
    run(&mut engine, &format!("{ROBUST_DATA}COMPUTE y2 = y * y."));
    let t = run(&mut engine, "GLM y y2 BY a WITH x /PRINT=BP /DESIGN=a x.");
    let tables: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "Heteroskedasticity Tests")
        .collect();
    assert_eq!(tables.len(), 2, "one table for each dependent variable");
    assert!(
        tables[0]
            .caption
            .as_deref()
            .unwrap()
            .contains("Dependent Variable: y.")
    );
    assert!(
        tables[1]
            .caption
            .as_deref()
            .unwrap()
            .contains("Dependent Variable: y2.")
    );
    let message = run_error(&format!(
        "{ROBUST_DATA}UNIANOVA y BY a /PRINT=BP /BPDESIGN=nosuch."
    ));
    assert!(message.contains("nosuch"), "{message}");
}

// Without an intercept the auxiliary model of DESIGN and White's still carry the constant the
// formulas assume (the manual: the auxiliary model always has an intercept).
#[test]
fn heteroskedasticity_tests_add_the_constant_to_a_model_without_one() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{ROBUST_DATA}UNIANOVA y WITH x /INTERCEPT=EXCLUDE /PRINT=MBP BP WHITE /MBPDESIGN /BPDESIGN /DESIGN=x."
        ),
    );
    assert!(close(het(&t, "Modified Breusch-Pagan Test", "df"), 1.0));
    assert!(close(
        het(&t, "Modified Breusch-Pagan Test", "Statistic"),
        2.506791501
    ));
    assert!(close(
        het(&t, "Breusch-Pagan Test", "Statistic"),
        1.389287108
    ));
    assert!(close(het(&t, "White's Test", "df"), 1.0));
    assert!(close(het(&t, "White's Test", "Statistic"), 4.662785395));
}

const RANDOM_BALANCED: &str = "DATA LIST FREE /y A B.
BEGIN DATA
11.225 1 1
12.128 1 1
11.305 1 1
9.592 1 2
8.868 1 2
10.313 1 2
11.68 1 3
11.819 1 3
10.941 1 3
10.396 1 4
12.662 1 4
10.338 1 4
11.025 2 1
11.169 2 1
9.921 2 1
11.068 2 2
10.995 2 2
9.243 2 2
13.227 2 3
13.009 2 3
11.62 2 3
10.734 2 4
11.226 2 4
10.399 2 4
9.817 3 1
10.185 3 1
8.088 3 1
9.14 3 2
7.941 3 2
8.661 3 2
11.318 3 3
9.011 3 3
12.069 3 3
9.805 3 4
11.287 3 4
11.357 3 4
END DATA.
";
const RANDOM_UNBALANCED: &str = "DATA LIST FREE /y A B.
BEGIN DATA
11.225 1 1
11.305 1 1
9.592 1 2
8.868 1 2
10.313 1 2
11.819 1 3
10.941 1 3
10.396 1 4
10.338 1 4
11.025 2 1
11.169 2 1
11.068 2 2
10.995 2 2
9.243 2 2
13.227 2 3
13.009 2 3
10.734 2 4
10.399 2 4
9.817 3 1
10.185 3 1
8.088 3 1
9.14 3 2
7.941 3 2
11.318 3 3
9.011 3 3
12.069 3 3
9.805 3 4
11.287 3 4
11.357 3 4
END DATA.
";
const RANDOM_THREE: &str = "DATA LIST FREE /y A B C.
BEGIN DATA
2.488 1 1 1
3.022 1 1 1
1.629 1 1 2
1.113 1 1 2
0.953 1 2 1
1.744 1 2 1
1.392 1 2 2
0.841 1 2 2
4.301 1 3 1
3.453 1 3 1
3.432 1 3 2
2.631 1 3 2
5.876 2 1 1
7.659 2 1 1
6.476 2 1 2
5.162 2 1 2
3.765 2 2 1
4.522 2 2 1
4.403 2 2 2
4.733 2 2 2
6.316 2 3 1
6.532 2 3 1
6.656 2 3 2
8.054 2 3 2
2.577 3 1 1
2.048 3 1 1
2.558 3 1 2
2.445 3 1 2
2.531 3 2 1
1.776 3 2 1
2.866 3 2 2
1.064 3 2 2
3.741 3 3 1
2.48 3 3 1
4.027 3 3 2
1.853 3 3 2
END DATA.
";

fn mixed(tables: &[Table], source: &str, kind: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == "Tests of Between-Subjects Effects")
        .unwrap();
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap();
    let mut current = String::new();
    for row in &table.rows {
        if let Cell::Text(s) = &row[0] {
            current = s.clone();
        }
        if current == source && matches!(&row[1], Cell::Text(k) if k == kind) {
            return number(&row[c]);
        }
    }
    panic!("no {source} {kind}");
}

fn ems(tables: &[Table], source: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == "Expected Mean Squares")
        .unwrap();
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap();
    let row = table
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == source))
        .unwrap();
    number(&row[c])
}

// A fixed, B random in a balanced design (3 x 4 x 3): the expected mean squares are the
// textbook ones (A: s2 + 3 s2AB + Q, B: s2 + 3 s2AB + 9 s2B, AB: s2 + 3 s2AB), A and the
// intercept are tested against what the unrestricted model gives, and the F tests are those of
// aov()'s mean squares in a general statistics package.
#[test]
fn random_balanced_two_way_gives_the_textbook_expected_mean_squares() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{RANDOM_BALANCED}UNIANOVA y BY A B /RANDOM=B /DESIGN=A B A*B."),
    );
    assert!(close(ems(&t, "B", "Var(B)"), 9.0));
    assert!(close(ems(&t, "B", "Var(A * B)"), 3.0));
    assert!(close(ems(&t, "A", "Var(A * B)"), 3.0));
    assert!(close(ems(&t, "A", "Var(B)"), 0.0));
    assert!(close(ems(&t, "A * B", "Var(A * B)"), 3.0));
    // the residual always has the coefficient 1 (the sweep's inverse is a reflexive one)
    for source in ["Intercept", "A", "B", "A * B", "Error"] {
        assert!(close(ems(&t, source, "Var(Error)"), 1.0), "{source}");
    }
    let quadratic = t
        .iter()
        .find(|t| t.title == "Expected Mean Squares")
        .unwrap();
    assert!(
        quadratic
            .column_titles()
            .contains(&"Quadratic Term".to_string())
    );
    assert!(close(mixed(&t, "A", "Hypothesis", "F"), 4.61420332412));
    assert!(close(mixed(&t, "A", "Hypothesis", "Sig."), 0.0611632219428));
    assert!(close(mixed(&t, "A", "Error", "df"), 6.0));
    assert!(close(mixed(&t, "B", "Hypothesis", "F"), 5.8798165512));
    assert!(close(mixed(&t, "B", "Hypothesis", "Sig."), 0.0321691976167));
    assert!(close(mixed(&t, "A * B", "Hypothesis", "F"), 1.38084596984));
    assert!(close(mixed(&t, "A * B", "Error", "df"), 24.0));
    assert!(close(
        mixed(&t, "Intercept", "Hypothesis", "F"),
        595.514053405
    ));
    assert!(close(
        mixed(&t, "Intercept", "Hypothesis", "Sig."),
        0.000150838964648
    ));
}

// Unbalanced: the expected mean squares, the error terms (a mix of MS(A*B) and MS(Error) for A
// and B) and their Satterthwaite degrees of freedom from an independent build in a general
// statistics package of the same formulas (the Type III estimable functions written from the
// cell means, the trace formula, the matching of the expected mean squares by least squares).
#[test]
fn random_unbalanced_two_way_gives_satterthwaite_error_terms() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{RANDOM_UNBALANCED}UNIANOVA y BY A B /RANDOM=B /DESIGN=A B A*B."),
    );
    assert!(close(ems(&t, "Intercept", "Var(B)"), 6.967741935));
    assert!(close(ems(&t, "Intercept", "Var(A * B)"), 2.322580645));
    assert!(close(ems(&t, "A", "Var(A * B)"), 2.332288401));
    assert!(close(ems(&t, "B", "Var(B)"), 6.982758621));
    assert!(close(ems(&t, "B", "Var(A * B)"), 2.327586207));
    assert!(close(ems(&t, "A * B", "Var(A * B)"), 2.355350742));
    for (source, f, p, df) in [
        ("Intercept", 520.5605613, 0.0001837074586, 3.00170048),
        ("A", 4.223309772, 0.07080027694, 6.080821698),
        ("B", 5.188534793, 0.04094664202, 6.097539984),
        ("A * B", 1.471332288, 0.2464121838, 17.0),
    ] {
        assert!(close(mixed(&t, source, "Hypothesis", "F"), f), "{source} F");
        assert!(
            close(mixed(&t, source, "Hypothesis", "Sig."), p),
            "{source} p"
        );
        assert!(close(mixed(&t, source, "Error", "df"), df), "{source} df");
    }
}

// Fully random three-way design: the test of A uses MS(AB) + MS(AC) - MS(ABC), a combination
// with a negative coefficient, with Satterthwaite's degrees of freedom (as aov()'s mean squares
// combined in a general statistics package).
#[test]
fn random_three_way_synthesizes_an_error_term_with_a_negative_coefficient() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{RANDOM_THREE}UNIANOVA y BY A B C /RANDOM=A B C /DESIGN=A B C A*B*C B*C A*C A*B."
        ),
    );
    assert!(close(mixed(&t, "A", "Error", "Mean Square"), 1.3903895));
    assert!(close(mixed(&t, "A", "Error", "df"), 3.18326666697));
    assert!(close(mixed(&t, "A", "Hypothesis", "F"), 34.8246985827));
    assert!(close(mixed(&t, "A", "Hypothesis", "Sig."), 0.0068588138297));
    assert!(close(
        mixed(&t, "A * B * C", "Hypothesis", "F"),
        0.717388568237
    ));
    assert!(close(ems(&t, "A", "Var(A)"), 12.0));
    assert!(close(ems(&t, "A * B * C", "Var(A * B * C)"), 2.0));
}

#[test]
fn random_needs_factors_and_no_within_subjects_factor_and_treats_several_dependents_apart() {
    let message = run_error(&format!(
        "{RANDOM_BALANCED}UNIANOVA y BY A WITH B /RANDOM=B."
    ));
    assert!(message.contains("RANDOM names"), "{message}");
    let message = run_error(&format!(
        "{RANDOM_BALANCED}UNIANOVA y BY A B /RANDOM=nosuch."
    ));
    assert!(message.contains("nosuch"), "{message}");
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!("{RANDOM_BALANCED}COMPUTE y2 = y * 2."),
    );
    let t = run(&mut engine, "GLM y y2 BY A B /RANDOM=B /DESIGN=A B A*B.");
    let tests = t
        .iter()
        .filter(|t| t.title == "Tests of Between-Subjects Effects")
        .count();
    assert_eq!(tests, 2, "a univariate mixed model for each dependent");
    assert!(t.iter().all(|t| !t.title.contains("Multivariate")));
}
