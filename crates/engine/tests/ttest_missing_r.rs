//! T-TEST against R on 42 invented cases with missing values (system-missing in the groups and in
//! three variables, and a user-missing 999 in one): the independent-samples test with a group pair
//! out of three (`GROUPS=g3(1 3)`) at a 90% interval, a cut point on a continuous variable
//! (`GROUPS=x3(5)`), the three ways of treating missing values (ANALYSIS leaves out a case in the
//! test it is missing from, LISTWISE in all of them, INCLUDE keeps the user-missing value), the
//! one-sample test on several variables, and the paired tests of PAIRS lists (every pair, WITH,
//! WITH (PAIRED)) with the three standardizers of the related-samples effect sizes: the standard
//! deviation of the differences (noncentral t), the corrected one (the correlation-adjusted
//! standard error, normal-theory interval) and the average of the variances (noncentral t).
//! Every number is written out in R from the manual's formulas (t.test's values, Levene's F as the
//! F test of the absolute deviations from the group means, `pt` with a noncentrality parameter
//! inverted for the effect size limits). Scripts in lab/oracles/ttest-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g3 g x1 x2 x3 (5F8.2).
BEGIN DATA.
1 1 24.8 24.2 11.4
2 2 24.2 27.9 4.5
3 1 26.4 26.2 8.2
1 1 . 16.5 5
2 2 22.5 19.1 5
3 . 23.3 22.6 6
1 1 19.1 15.7 3.9
2 2 24.2 24.9 5.6
3 1 32.6 . 10.9
1 1 23.8 19.9 5.9
2 2 20.4 23.1 5.5
3 1 28.3 26.8 .
1 1 18.1 20.6 6.6
2 2 19.7 17.3 3.6
3 1 25.2 15.7 7.5
1 1 19 15.8 6.1
2 2 27.4 24.9 5.1
3 1 21.4 18.8 5.6
1 1 . . 8
2 2 26.1 25.8 7.4
3 1 29.4 24.2 7
1 1 17.4 999 2.3
2 2 21.5 20.6 7.4
3 1 27.4 23.7 6.2
1 1 18.2 21.2 4.6
2 2 22.1 22.3 5.2
3 1 24.2 22.1 3.6
1 1 27.1 22.5 6.7
2 2 19.7 22.2 6.3
3 . 20.9 17.9 4.8
1 1 21.2 18.3 5.9
2 2 18.9 18.6 5.6
3 1 22.5 . 4.9
1 1 17.1 16.8 6.4
2 2 12.4 12.1 1.4
3 1 22.8 24.8 7.8
1 1 21.3 17.7 5.6
2 2 24.1 25.8 6
3 1 29.2 25 7.2
1 1 13.5 16.3 .
2 2 18.5 18.9 2.3
3 1 18.7 18.1 2.5
END DATA.
MISSING VALUES x2 (999).
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

/// A number of the row `row` (headers joined) in the table whose title starts with `title`; the
/// column is found by its last words ("Mean" also finds "Paired Differences / Mean").
fn at(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column || t.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    // the effect size limits come from inverting a noncentral t, good to about 1e-8
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// The numbers of `ind` in R: groups 1 and 2 (N, mean, sd, se), Levene F and p, the pooled and the
/// unpooled test (t, df, p, [difference, se,] limits), then the standardizer, estimate and limits
/// of Cohen's d, Hedges' correction and Glass's delta.
fn independent(syntax: &str, variable: &str, groups: (&str, &str), w: &[f64; 35]) {
    let t = run(syntax);
    let (g1, g2) = groups;
    for (g, o) in [(g1, 0), (g2, 4)] {
        let key = format!("{variable} / {g}");
        for (c, k) in ["N", "Mean", "Std. Deviation", "Std. Error Mean"]
            .iter()
            .zip(o..)
        {
            near(
                at(&t, "Group Statistics", &key, c),
                w[k],
                &format!("{key} {c}"),
            );
        }
    }
    let table = "Independent Samples Test";
    let equal = format!("{variable} / Equal variances assumed");
    let unequal = format!("{variable} / Equal variances not assumed");
    for (c, k) in [
        ("F", 8),
        ("Sig.", 9),
        ("t", 10),
        ("df", 11),
        ("Sig. (2-tailed)", 12),
        ("Mean Difference", 13),
        ("Std. Error Difference", 14),
        ("Lower", 15),
        ("Upper", 16),
    ] {
        near(at(&t, table, &equal, c), w[k], &format!("equal {c}"));
    }
    near(
        at(&t, table, &unequal, "Mean Difference"),
        w[13],
        "unequal difference",
    );
    for (c, k) in [
        ("t", 17),
        ("df", 18),
        ("Sig. (2-tailed)", 19),
        ("Std. Error Difference", 20),
        ("Lower", 21),
        ("Upper", 22),
    ] {
        near(at(&t, table, &unequal, c), w[k], &format!("unequal {c}"));
    }
    let es = "Independent Samples Effect Sizes";
    for (label, o) in [
        ("Cohen's d", 23),
        ("Hedges' correction", 27),
        ("Glass's delta", 31),
    ] {
        let key = format!("{variable} / {label}");
        for (c, k) in ["Standardizer", "Point Estimate", "Lower", "Upper"]
            .iter()
            .zip(o..)
        {
            near(at(&t, es, &key, c), w[k], &format!("{label} {c}"));
        }
    }
}

/// The numbers of `one` in R: N, mean, sd, se, t, df, p, difference, limits, then the standardizer,
/// estimate and limits of Cohen's d and of Hedges' correction.
fn one_sample(t: &[Table], variable: &str, w: &[f64; 18]) {
    for (c, k) in [
        ("N", 0),
        ("Mean", 1),
        ("Std. Deviation", 2),
        ("Std. Error Mean", 3),
    ] {
        near(
            at(t, "One-Sample Statistics", variable, c),
            w[k],
            &format!("{variable} {c}"),
        );
    }
    for (c, k) in [
        ("t", 4),
        ("df", 5),
        ("Sig. (2-tailed)", 6),
        ("Mean Difference", 7),
        ("Lower", 8),
        ("Upper", 9),
    ] {
        near(
            at(t, "One-Sample Test", variable, c),
            w[k],
            &format!("{variable} {c}"),
        );
    }
    for (label, o) in [("Cohen's d", 10), ("Hedges' correction", 14)] {
        let key = format!("{variable} / {label}");
        for (c, k) in ["Standardizer", "Point Estimate", "Lower", "Upper"]
            .iter()
            .zip(o..)
        {
            near(
                at(t, "One-Sample Effect Sizes", &key, c),
                w[k],
                &format!("{key} {c}"),
            );
        }
    }
}

/// The numbers of `paired` in R: N, mean and sd of each, mean and sd of the difference, its se and
/// limits, t, df, p, the correlation and its p, then standardizer, estimate and limits of Cohen's d
/// and of Hedges' correction.
fn paired_pair(t: &[Table], pair: &str, left: &str, right: &str, w: &[f64; 23]) {
    let p = format!("{pair} / ");
    for (c, k) in [("N", 0), ("Mean", 1), ("Std. Deviation", 2)] {
        near(
            at(t, "Paired Samples Statistics", &format!("{p}{left}"), c),
            w[k],
            &format!("{left} {c}"),
        );
    }
    for (c, k) in [("Mean", 3), ("Std. Deviation", 4)] {
        near(
            at(t, "Paired Samples Statistics", &format!("{p}{right}"), c),
            w[k],
            &format!("{right} {c}"),
        );
    }
    let row = format!("{p}{left} - {right}");
    for (c, k) in [
        ("Mean", 5),
        ("Std. Deviation", 6),
        ("Std. Error Mean", 7),
        ("Lower", 8),
        ("Upper", 9),
        ("t", 10),
        ("df", 11),
        ("Sig. (2-tailed)", 12),
    ] {
        near(
            at(t, "Paired Samples Test", &row, c),
            w[k],
            &format!("{row} {c}"),
        );
    }
    let corr = format!("{p}{left} & {right}");
    near(
        at(t, "Paired Samples Correlations", &corr, "Correlation"),
        w[13],
        "correlation",
    );
    near(
        at(t, "Paired Samples Correlations", &corr, "Sig."),
        w[14],
        "correlation p",
    );
    for (label, o) in [("Cohen's d", 15), ("Hedges' correction", 19)] {
        let key = format!("{row} / {label}");
        for (c, k) in ["Standardizer", "Point Estimate", "Lower", "Upper"]
            .iter()
            .zip(o..)
        {
            near(
                at(t, "Paired Samples Effect Sizes", &key, c),
                w[k],
                &format!("{key} {c}"),
            );
        }
    }
}

#[test]
fn independent_samples_with_a_group_pair_a_cut_point_and_the_missing_value_options() {
    // two of the three groups, at a 90% interval
    independent(
        "T-TEST GROUPS=g3(1 3) /VARIABLES=x1 x2 /CRITERIA=CI(.90).",
        "x1",
        ("1", "3"),
        &[
            12.0,
            20.05,
            3.778768438,
            1.090836487,
            14.0,
            25.16428571,
            3.893901967,
            1.040689076,
            0.06032199646,
            0.8080758878,
            -3.38411602,
            24.0,
            0.002451406403,
            -5.114285714,
            1.511261932,
            -7.699876672,
            -2.528694757,
            -3.392261174,
            23.59620602,
            0.002442797205,
            1.507633243,
            -7.695440494,
            -2.533130935,
            3.841560788,
            -1.331304123,
            -2.038462498,
            -0.599641625,
            3.967057733,
            -1.289188628,
            -1.973976213,
            -0.5806721022,
            3.893901967,
            -1.313408955,
            -2.063966976,
            -0.521732745,
        ],
    );
    independent(
        "T-TEST GROUPS=g3(1 3) /VARIABLES=x1 x2 /CRITERIA=CI(.90).",
        "x2",
        ("1", "3"),
        &[
            12.0,
            18.79166667,
            2.839160674,
            0.8195950897,
            12.0,
            22.15833333,
            3.658976095,
            1.056255417,
            0.9190273946,
            0.3481482982,
            -2.51818615,
            22.0,
            0.01957305244,
            -3.366666667,
            1.336941142,
            -5.662387627,
            -1.070945706,
            -2.51818615,
            20.72172548,
            0.02009647645,
            1.336941142,
            -5.668623771,
            -1.064709563,
            3.274823613,
            -1.028045191,
            -1.735024001,
            -0.2995376875,
            3.392015515,
            -0.9925269069,
            -1.675080065,
            -0.2891888577,
            3.658976095,
            -0.9201116869,
            -1.645103864,
            -0.1584096885,
        ],
    );
    // a continuous grouping variable cut at 5: at or above against below, cases missing it left out
    independent(
        "T-TEST GROUPS=x3(5) /VARIABLES=x1.",
        "x1",
        (">= 5", "< 5"),
        &[
            27.0,
            23.59259259,
            3.780305841,
            0.7275201983,
            11.0,
            19.61818182,
            3.366546653,
            1.015052008,
            0.5915430589,
            0.44684003,
            3.027517262,
            36.0,
            0.004537471672,
            3.974410774,
            1.312762383,
            1.312005262,
            6.636816287,
            3.182466789,
            20.80170547,
            0.004519432575,
            1.248845954,
            1.375785144,
            6.573036405,
            3.670054806,
            1.082929543,
            0.3321853137,
            1.820308509,
            3.748792996,
            1.060184112,
            0.3252082226,
            1.782075458,
            3.366546653,
            1.180560136,
            0.2944973064,
            2.028828841,
        ],
    );
    // ANALYSIS: each dependent keeps the cases that have it; LISTWISE: only the cases that have both;
    // INCLUDE: the 999 of x2 is a value
    independent(
        "T-TEST GROUPS=g /VARIABLES=x1 x2.",
        "x1",
        ("1", "2"),
        &[
            24.0,
            22.8625,
            4.75136708,
            0.9698687439,
            14.0,
            21.55,
            3.771298491,
            1.007921918,
            1.667940663,
            0.2047654914,
            0.8824733386,
            36.0,
            0.3833757125,
            1.3125,
            1.487297058,
            -1.703878241,
            4.328878241,
            0.9383257132,
            32.48005968,
            0.3550027526,
            1.39876802,
            -1.535046567,
            4.160046567,
            4.42257859,
            0.2967725668,
            -0.367918724,
            0.9573988769,
            4.517461596,
            0.2905392713,
            -0.3601911023,
            0.9372900435,
            3.771298491,
            0.3480233673,
            -0.3306453807,
            1.01395785,
        ],
    );
    independent(
        "T-TEST GROUPS=g /VARIABLES=x1 x2.",
        "x2",
        ("1", "2"),
        &[
            22.0,
            20.49545455,
            3.733946881,
            0.7960801498,
            14.0,
            21.67857143,
            4.224867512,
            1.129143338,
            0.01920443979,
            0.8905995321,
            -0.880806686,
            34.0,
            0.3846065435,
            -1.183116883,
            1.343219689,
            -3.912867721,
            1.546633955,
            -0.8563629562,
            25.2705658,
            0.3998449147,
            1.38156009,
            -4.026949214,
            1.660715447,
            3.928901646,
            -0.3011317131,
            -0.9728089467,
            0.3749094026,
            4.018308281,
            -0.2944315867,
            -0.9511641229,
            0.366567736,
            4.224867512,
            -0.2800364461,
            -0.9533346635,
            0.4036580687,
        ],
    );
    independent(
        "T-TEST GROUPS=g /VARIABLES=x1 x2 /MISSING=LISTWISE.",
        "x1",
        ("1", "2"),
        &[
            21.0,
            22.67619048,
            4.436880071,
            0.9682066083,
            14.0,
            21.55,
            3.771298491,
            1.007921918,
            1.306433736,
            0.2612660885,
            0.7794967954,
            33.0,
            0.4412419125,
            1.126190476,
            1.444766012,
            -1.813208076,
            4.065589028,
            0.8057938905,
            30.93785403,
            0.4265121077,
            1.397616052,
            -1.724498346,
            3.976879298,
            4.187329717,
            0.2689519461,
            -0.412393581,
            0.9462739248,
            4.285598823,
            0.2627848575,
            -0.4029373649,
            0.9245757919,
            3.771298491,
            0.2986214109,
            -0.3926387257,
            0.9788268307,
        ],
    );
    independent(
        "T-TEST GROUPS=g /VARIABLES=x1 x2 /MISSING=LISTWISE.",
        "x2",
        ("1", "2"),
        &[
            21.0,
            20.68571429,
            3.71527734,
            0.8107399828,
            14.0,
            21.67857143,
            4.224867512,
            1.129143338,
            0.03530650374,
            0.8521055023,
            -0.7333390501,
            33.0,
            0.468529637,
            -0.9928571429,
            1.35388555,
            -3.747358004,
            1.761643719,
            -0.7142554017,
            25.46072657,
            0.4815667883,
            1.390058991,
            -3.853112684,
            1.867398398,
            3.923933114,
            -0.2530260109,
            -0.9301117405,
            0.4278488173,
            4.016020775,
            -0.2472241053,
            -0.9087842078,
            0.4180382115,
            4.224867512,
            -0.2350031427,
            -0.9127466586,
            0.451531276,
        ],
    );
    independent(
        "T-TEST GROUPS=g /VARIABLES=x1 x2 /MISSING=ANALYSIS INCLUDE.",
        "x2",
        ("1", "2"),
        &[
            23.0,
            63.03913043,
            204.0649133,
            42.55047583,
            14.0,
            21.67857143,
            4.224867512,
            1.129143338,
            2.428213527,
            0.1281661202,
            0.754072014,
            35.0,
            0.4558495766,
            41.36055901,
            54.84961415,
            -69.99007753,
            152.7111955,
            0.9716931023,
            22.03097674,
            0.3417516333,
            42.56545499,
            -46.90759589,
            129.6287139,
            161.8082462,
            0.2556146549,
            -0.413250768,
            0.9208700044,
            165.3819453,
            0.2500911386,
            -0.4043209303,
            0.9009711432,
            4.224867512,
            9.789788411,
            6.011263427,
            13.55787267,
        ],
    );
}

#[test]
fn one_sample_tests_of_several_variables_under_each_missing_option() {
    let t = run("T-TEST /TESTVAL=19 /VARIABLES=x1 x2 x3.");
    one_sample(
        &t,
        "x1",
        &[
            40.0,
            22.365,
            4.303817626,
            0.6804933165,
            4.944942027,
            39.0,
            1.48984689e-05,
            3.365,
            1.988572348,
            4.741427652,
            4.303817626,
            0.7818639851,
            0.4231244146,
            1.132898192,
            4.388855033,
            0.7667147752,
            0.4149260569,
            1.110947427,
        ],
    );
    one_sample(
        &t,
        "x2",
        &[
            38.0,
            20.91842105,
            3.851279553,
            0.6247600423,
            3.070652607,
            37.0,
            0.003989515461,
            1.918421053,
            0.6525369638,
            3.184305141,
            3.851279553,
            0.4981256297,
            0.1576686577,
            0.8326007627,
            3.931607927,
            0.4879482106,
            0.1544472615,
            0.8155895382,
        ],
    );
    one_sample(
        &t,
        "x3",
        &[
            40.0,
            5.7875,
            2.032705346,
            0.3213989353,
            -41.10934588,
            39.0,
            9.935686317e-34,
            -13.2125,
            -13.86259071,
            -12.56240929,
            2.032705346,
            -6.499958306,
            -7.966283967,
            -5.027182508,
            2.072868757,
            -6.374016665,
            -7.811931151,
            -4.92977702,
        ],
    );
    let t = run("T-TEST /TESTVAL=19 /VARIABLES=x1 x2 x3 /MISSING=LISTWISE.");
    one_sample(
        &t,
        "x1",
        &[
            35.0,
            22.29428571,
            3.753346966,
            0.6344314315,
            5.192500798,
            34.0,
            9.664162814e-06,
            3.294285714,
            2.004965921,
            4.583605508,
            3.753346966,
            0.8776928284,
            0.4819387245,
            1.264095663,
            3.838758654,
            0.8581643212,
            0.4712156748,
            1.235969762,
        ],
    );
    one_sample(
        &t,
        "x2",
        &[
            35.0,
            21.00857143,
            3.730113658,
            0.6305042857,
            3.185658645,
            34.0,
            0.003089406384,
            2.008571429,
            0.7272325559,
            3.289910301,
            3.730113658,
            0.5384745916,
            0.1800377655,
            0.8900051248,
            3.814996645,
            0.526493629,
            0.1760319577,
            0.8702026712,
        ],
    );
    one_sample(
        &t,
        "x3",
        &[
            35.0,
            5.725714286,
            1.850332698,
            0.3127633104,
            -42.44195298,
            34.0,
            4.885222196e-31,
            -13.27428571,
            -13.90989723,
            -12.63867419,
            1.850332698,
            -7.173999427,
            -8.899447086,
            -5.441450872,
            1.892439127,
            -7.014379233,
            -8.701435993,
            -5.320379571,
        ],
    );
    let t = run("T-TEST /TESTVAL=19 /VARIABLES=x1 x2 x3 /MISSING=LISTWISE INCLUDE.");
    one_sample(
        &t,
        "x1",
        &[
            36.0,
            22.15833333,
            3.78820538,
            0.6313675633,
            5.002368694,
            35.0,
            1.592845753e-05,
            3.158333333,
            1.876589037,
            4.440077629,
            3.78820538,
            0.8337281157,
            0.4490694162,
            1.209520287,
            3.871871737,
            0.8157122829,
            0.4393655818,
            1.18338405,
        ],
    );
    one_sample(
        &t,
        "x2",
        &[
            36.0,
            48.175,
            163.0400273,
            27.17333788,
            1.073662725,
            35.0,
            0.2903206831,
            29.175,
            -25.98980867,
            84.33980867,
            163.0400273,
            0.1789437875,
            -0.1516299677,
            0.5070101567,
            166.6409316,
            0.1750770337,
            -0.1483534318,
            0.4960542946,
        ],
    );
    one_sample(
        &t,
        "x3",
        &[
            36.0,
            5.630555556,
            1.910993675,
            0.3184989458,
            -41.97641663,
            35.0,
            1.575285477e-31,
            -13.36944444,
            -14.01603168,
            -12.72285721,
            1.910993675,
            -6.996069438,
            -8.656268419,
            -5.328907758,
            1.953199908,
            -6.844893035,
            -8.469217171,
            -5.213756656,
        ],
    );
}

#[test]
fn paired_tests_of_pair_lists_with_each_standardizer() {
    // every pair of three variables, listwise over the three, the corrected standardizer
    let t = run("T-TEST PAIRS=x1 x2 x3 /ES STANDARDIZER(CORRECTED_SD) /MISSING=LISTWISE.");
    paired_pair(
        &t,
        "Pair 1",
        "x1",
        "x2",
        &[
            35.0,
            22.29428571,
            3.753346966,
            21.00857143,
            3.730113658,
            1.285714286,
            2.799804915,
            0.4732534073,
            0.3239476472,
            2.247480924,
            2.716756532,
            34.0,
            0.01029223143,
            0.7200665073,
            1.081539484e-06,
            3.741841113,
            0.3436047248,
            0.0729845421,
            0.6142249075,
            3.783753529,
            0.3397986353,
            0.0721760966,
            0.6074211741,
        ],
    );
    paired_pair(
        &t,
        "Pair 2",
        "x1",
        "x3",
        &[
            35.0,
            22.29428571,
            3.753346966,
            5.725714286,
            1.850332698,
            16.56857143,
            3.152195447,
            0.5328182787,
            15.48575441,
            17.65138845,
            31.09610179,
            34.0,
            1.472618326e-26,
            0.5453616396,
            0.0007039166282,
            3.305712661,
            5.012102723,
            3.734196802,
            6.290008644,
            3.342739996,
            4.956583954,
            3.692833322,
            6.220334586,
        ],
    );
    paired_pair(
        &t,
        "Pair 3",
        "x2",
        "x3",
        &[
            35.0,
            21.00857143,
            3.730113658,
            5.725714286,
            1.850332698,
            15.28285714,
            3.305742394,
            0.5587724498,
            14.1472949,
            16.41841939,
            27.35077069,
            34.0,
            9.820399637e-25,
            0.4643285623,
            0.004955505156,
            3.19377829,
            4.78519664,
            3.553470926,
            6.016922354,
            3.229551847,
            4.732191297,
            3.514109336,
            5.950273257,
        ],
    );
    // WITH (PAIRED): the first with the first, the second with the second; the average of the variances
    let t = run("T-TEST PAIRS=x1 x2 WITH x3 x1 (PAIRED) /ES STANDARDIZER(AVERAGE).");
    paired_pair(
        &t,
        "Pair 1",
        "x1",
        "x3",
        &[
            38.0,
            22.44210526,
            4.05486905,
            5.75,
            2.050543764,
            16.69210526,
            3.148606005,
            0.5107713407,
            15.65718422,
            17.7270263,
            32.68019157,
            37.0,
            6.78394585e-29,
            0.645422593,
            1.213155921e-05,
            3.212996479,
            5.19518318,
            3.971023147,
            6.412396675,
            3.246025616,
            5.142320868,
            3.930616975,
            6.347148906,
        ],
    );
    paired_pair(
        &t,
        "Pair 2",
        "x2",
        "x1",
        &[
            37.0,
            21.03783784,
            3.832416655,
            22.21891892,
            4.055780363,
            -1.181081081,
            2.80305764,
            0.4608198373,
            -2.115667029,
            -0.2464951334,
            -2.562999648,
            36.0,
            0.01470232465,
            0.7488573739,
            9.782286485e-08,
            3.945679395,
            -0.2993352888,
            -0.6268511195,
            0.03214847981,
            3.987383644,
            -0.2962045257,
            -0.6202948516,
            0.03181223722,
        ],
    );
    // WITH without PAIRED: the left one against each of the right ones, the default standardizer
    let t = run("T-TEST PAIRS=x1 WITH x2 x3.");
    paired_pair(
        &t,
        "Pair 1",
        "x1",
        "x2",
        &[
            37.0,
            22.21891892,
            4.055780363,
            21.03783784,
            3.832416655,
            1.181081081,
            2.80305764,
            0.4608198373,
            0.2464951334,
            2.115667029,
            2.562999648,
            36.0,
            0.01470232465,
            0.7488573739,
            9.782286485e-08,
            2.80305764,
            0.4213545467,
            0.08217756095,
            0.7551673279,
            2.832684835,
            0.4169475779,
            0.08131806164,
            0.7472689943,
        ],
    );
    paired_pair(
        &t,
        "Pair 2",
        "x1",
        "x3",
        &[
            38.0,
            22.44210526,
            4.05486905,
            5.75,
            2.050543764,
            16.69210526,
            3.148606005,
            0.5107713407,
            15.65718422,
            17.7270263,
            32.68019157,
            37.0,
            6.78394585e-29,
            0.645422593,
            1.213155921e-05,
            3.148606005,
            5.30142712,
            4.054002627,
            6.54193657,
            3.180973217,
            5.247483749,
            4.012752117,
            6.475370699,
        ],
    );
}
