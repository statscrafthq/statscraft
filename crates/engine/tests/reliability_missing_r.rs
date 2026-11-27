//! RELIABILITY against R on 40 invented cases of five items with missing values (system-missing in
//! two items and a user-missing -9 in two) and frequency weights: the default exclusion (a case
//! missing, user-missing included, on any item of VARIABLES is left out) against MISSING=INCLUDE
//! (user-missing values count), Cronbach's alpha, the standardized alpha and the item-total
//! statistics (scale mean and variance and alpha if the item is deleted, the corrected item-total
//! correlation) written out in R; frequency weights (the cases written out as often as their
//! weight, so N, the ANOVA degrees of freedom and the F tests follow the sum of the weights); a
//! scale of a subset of the variables with its own listwise cases; and the intraclass correlations
//! from the two-way ANOVA mean squares: the consistency type with a 90 percent interval and a
//! true value of 0.3 (the F test against it), the absolute agreement type. Scripts in
//! lab/oracles/reliability-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /i1 i2 i3 i4 i5 w (6F8.2).
BEGIN DATA.
4.3 1.2 2.3 3.4 3.7 1
4.4 4.8 2.7 2.5 3.1 2
4 4.1 3.6 3.6 3.4 1
4.4 . 5.4 5.7 4.4 1
0.8 1.8 2 1.8 0.7 1
4 5.2 2.9 5 2.8 3
3.1 3.3 -9 3.3 4 3
4 3.3 4.5 5.9 3.4 1
4.6 2.5 2.9 3.8 4 1
1.4 4 3.1 5.8 1.8 3
3.6 3 4.1 3.3 . 3
2.5 1.2 2 2.4 2.4 1
2.3 2.6 2.8 1.1 1.9 1
2.1 3.5 1.4 2.8 3.2 1
2.3 2.7 1.5 2.8 2.4 1
2.3 3.8 1.1 2.2 2.9 1
4 4.1 1.8 4.3 3.3 2
1.8 2.2 2.3 2.9 1.5 1
0.9 . 1.3 2.2 1.5 1
3.5 4.6 4 3.3 4.7 3
3.9 3.1 2.5 3.6 4.8 3
-9 2.3 1.3 4.9 3.8 3
1 2.2 0.9 2.9 2.7 1
4.9 4.9 3.8 4.1 4.4 3
3 5.3 1.2 3.8 1.7 1
2.6 2.5 2.2 3.1 3.6 1
3.6 3.7 5.2 3.9 4.2 3
3.5 3.2 3.4 2.7 3.1 1
4.3 3.5 1.8 4.1 3 1
3.6 4.6 3.3 5.5 . 2
2 3.3 1.3 3.4 . 2
2.4 2.4 2.3 2.9 2.6 1
3.8 4.1 5 3.6 3.6 2
2.7 1.4 3.9 4 3 2
1.7 1.8 2 1.6 3.1 2
2.6 3.5 3.5 1.9 2.8 1
2.8 3.9 2.3 2.8 2.9 2
3.7 4.3 2.9 2.5 3.4 1
3.4 2.8 2 3.9 3.2 2
2.1 0.8 2.5 3.7 2.3 3
END DATA.
MISSING VALUES i1 i2 i3 i4 i5 (-9).
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

fn number(t: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = t
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let c = table
        .column_titles()
        .iter()
        .position(|k| k == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    let r = if table.row_headers == 0 {
        0
    } else {
        table
            .rows
            .iter()
            .position(|r| matches!(&r[0], Cell::Text(s) if s == row))
            .unwrap_or_else(|| panic!("{title}: no row {row}"))
    };
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// The numbers of `alpha_tab` in R: alpha, standardized alpha, N, then for each item the scale
/// mean and variance if it is deleted, its corrected correlation and alpha if deleted.
fn alpha(syntax: &str, items: &[&str], want: &[f64]) {
    let t = run(syntax);
    near(
        number(&t, "Reliability Statistics", "", "Cronbach's Alpha"),
        want[0],
        "alpha",
    );
    near(
        number(
            &t,
            "Reliability Statistics",
            "",
            "Cronbach's Alpha Based on Standardized Items",
        ),
        want[1],
        "standardized alpha",
    );
    near(
        number(&t, "Case Processing Summary", "Valid", "N"),
        want[2],
        "valid cases",
    );
    for (i, item) in items.iter().enumerate() {
        let o = 3 + 4 * i;
        for (c, k) in [
            ("Scale Mean if Item Deleted", 0),
            ("Scale Variance if Item Deleted", 1),
            ("Corrected Item-Total Correlation", 2),
            ("Cronbach's Alpha if Item Deleted", 3),
        ] {
            near(
                number(&t, "Item-Total Statistics", item, c),
                want[o + k],
                &format!("{item} {c}"),
            );
        }
    }
}

const FIVE: [&str; 5] = ["i1", "i2", "i3", "i4", "i5"];

#[test]
fn missing_values_weights_and_a_subset_scale_agree_with_r() {
    // default: 33 cases are valid on all five items, the -9 counted as missing
    alpha(
        "RELIABILITY VARIABLES=i1 TO i5 /SUMMARY=TOTAL.",
        &FIVE,
        &[
            0.7472856865,
            0.7546162679,
            33.0,
            12.16969697,
            8.63092803,
            0.7123743087,
            0.624170622,
            12.02727273,
            9.687670455,
            0.4181165804,
            0.7432049094,
            12.53333333,
            9.919166667,
            0.4749839263,
            0.7161118027,
            11.91515152,
            10.15882576,
            0.432976857,
            0.7312395675,
            12.19090909,
            10.24460227,
            0.5639641316,
            0.6901195197,
            15.20909091,
            14.29710227,
        ],
    );
    // INCLUDE: the -9 is a value, 35 cases are valid
    alpha(
        "RELIABILITY VARIABLES=i1 TO i5 /MISSING=INCLUDE /SUMMARY=TOTAL.",
        &FIVE,
        &[
            0.4178304109,
            0.4945955476,
            35.0,
            11.87142857,
            11.31798319,
            0.2534653873,
            0.3457911358,
            11.40857143,
            15.62198319,
            0.3671208237,
            0.290562319,
            12.26571429,
            12.5652605,
            0.1812083106,
            0.420286424,
            11.22857143,
            17.70033613,
            0.1743457732,
            0.3955891895,
            11.5,
            17.77117647,
            0.2484496714,
            0.372033523,
            14.56857143,
            20.45339496,
        ],
    );
    // weights: the 33 cases weigh 54 together
    alpha(
        "WEIGHT BY w.\nRELIABILITY VARIABLES=i1 TO i5 /SUMMARY=TOTAL.",
        &FIVE,
        &[
            0.7305062434,
            0.7351112242,
            54.0,
            12.93518519,
            9.022700908,
            0.6699934163,
            0.6137502128,
            12.73888889,
            9.000157233,
            0.4977308859,
            0.685678939,
            13.19259259,
            9.975415793,
            0.4802572778,
            0.6883234488,
            12.55925926,
            11.02774284,
            0.3196835742,
            0.7460868867,
            12.9,
            10.35584906,
            0.5252507303,
            0.6753562828,
            16.08148148,
            14.4577638,
        ],
    );
    // a scale of four items has its own listwise cases (61 once weighted)
    alpha(
        "WEIGHT BY w.\nRELIABILITY VARIABLES=i1 TO i4 /SCALE(four)=i1 i2 i3 i4 /SUMMARY=TOTAL.",
        &["i1", "i2", "i3", "i4"],
        &[
            0.6790456203,
            0.6804829529,
            61.0,
            9.847540984,
            6.154535519,
            0.5498289358,
            0.5593342686,
            9.627868852,
            5.617043716,
            0.5021396145,
            0.5859595941,
            10.08360656,
            6.512393443,
            0.4136478755,
            0.6433571803,
            9.421311475,
            6.722704918,
            0.3904090662,
            0.6568747942,
            12.99344262,
            10.03995628,
        ],
    );
}

/// The numbers of `icc` in R: the consistency single measure (estimate, limits, F, df1, df2, p),
/// the consistency average measure (the same six), the absolute single and average estimates and
/// the F of the ANOVA.
fn icc_rows(t: &[Table], testval: &str, ci: &str, want: &[f64]) {
    let title = "Intraclass Correlation Coefficient";
    let f = |c: &str| format!("F Test with True Value {testval} / {c}");
    for (row, o) in [("Single Measures", 0), ("Average Measures", 7)] {
        near(
            number(t, title, row, "Intraclass Correlation"),
            want[o],
            row,
        );
        near(
            number(
                t,
                title,
                row,
                &format!("{ci}% Confidence Interval / Lower Bound"),
            ),
            want[o + 1],
            row,
        );
        near(
            number(
                t,
                title,
                row,
                &format!("{ci}% Confidence Interval / Upper Bound"),
            ),
            want[o + 2],
            row,
        );
        near(number(t, title, row, &f("Value")), want[o + 3], row);
        near(number(t, title, row, &f("df1")), want[o + 4], row);
        near(number(t, title, row, &f("df2")), want[o + 5], row);
        near(number(t, title, row, &f("Sig.")), want[o + 6], row);
    }
}

#[test]
fn intraclass_correlations_with_a_true_value_and_the_absolute_type_agree_with_r() {
    let t = run(
        "RELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL(MIXED) TYPE(CONSISTENCY) CIN=90 TESTVAL=0.3.",
    );
    icc_rows(
        &t,
        "0.3",
        "90",
        &[
            0.3716254274,
            0.2400518329,
            0.5256830414,
            1.259057367,
            32.0,
            128.0,
            0.1852049335,
            0.7472856865,
            0.6123123534,
            0.8471292611,
            2.769926207,
            32.0,
            128.0,
            2.836288659e-05,
            0.3614007158,
            0.7388785833,
            3.957037439,
        ],
    );
    // two-way random, consistency, at the default level and true value
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL(RANDOM) TYPE(CONSISTENCY).");
    icc_rows(
        &t,
        "0",
        "95",
        &[
            0.3716254274,
            0.2160963754,
            0.5545788953,
            3.957037439,
            32.0,
            128.0,
            1.50343202e-08,
            0.7472856865,
            0.5795378027,
            0.8615980648,
            3.957037439,
            32.0,
            128.0,
            1.50343202e-08,
            0.3614007158,
            0.7388785833,
            3.957037439,
        ],
    );
    // absolute agreement: the estimators use the mean square of the measures too; with a true value
    // of 0 the F is the ANOVA's
    let absolute = [
        0.3716254274,
        0.2400518329,
        0.5256830414,
        1.259057367,
        32.0,
        128.0,
        0.1852049335,
        0.7472856865,
        0.6123123534,
        0.8471292611,
        2.769926207,
        32.0,
        128.0,
        2.836288659e-05,
        0.3614007158,
        0.7388785833,
        3.957037439,
    ];
    for model in ["MIXED", "RANDOM"] {
        let t = run(&format!(
            "RELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL({model}) TYPE(ABSOLUTE)."
        ));
        let title = "Intraclass Correlation Coefficient";
        near(
            number(&t, title, "Single Measures", "Intraclass Correlation"),
            absolute[14],
            model,
        );
        near(
            number(&t, title, "Average Measures", "Intraclass Correlation"),
            absolute[15],
            model,
        );
        near(
            number(
                &t,
                title,
                "Single Measures",
                "F Test with True Value 0 / Value",
            ),
            absolute[16],
            model,
        );
    }
    // weighted: N is the sum of the weights, so df1 = 53 and df2 = 53 * 4
    let t =
        run("WEIGHT BY w.\nRELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL(MIXED) TYPE(CONSISTENCY).");
    icc_rows(
        &t,
        "0",
        "95",
        &[
            0.3515471717,
            0.2288421046,
            0.4929719685,
            3.710661103,
            53.0,
            212.0,
            7.012985598e-12,
            0.7305062434,
            0.5973840396,
            0.8293919377,
            3.710661103,
            53.0,
            212.0,
            7.012985598e-12,
            0.3398718073,
            0.7202238755,
            3.710661103,
        ],
    );
}
