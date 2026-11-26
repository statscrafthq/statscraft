//! NOMREG and PLUM options against R on 140 invented cases: 24 subpopulations of two factors and
//! a four-valued covariate (a frequency weight of 1 to 3, 29 of the 96 ordinal cells empty). The
//! scripts are in lab/oracles/nomreg-plum-r. PLUM: every link with the goodness-of-fit sums,
//! pseudo R-squares and the test of parallel lines (the general model fitted by `optim` on the
//! cumulative link likelihood written out in R; the estimates start from `MASS::polr`), DELTA and
//! BIAS (cell counts changed before the fit), a scale model with the probit link, and the saved
//! probabilities. NOMREG: the dispersion scale (2, Pearson, deviance), CIN, DELTA and BIAS, and the
//! category order with a base (the logits fitted by `optim` on the multinomial likelihood from the
//! subpopulation counts, standard errors from `numDeriv::hessian`).

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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell {
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

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    &table.rows[r][c]
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{what}: {actual} against {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

const DATA: &str = "DATA LIST FREE /y4 y3 f g z w.
BEGIN DATA.
1 3 3 2 0 3
1 3 3 1 2 2
3 1 2 1 1 1
4 3 1 1 1 3
4 1 2 1 3 1
2 3 3 1 3 1
1 2 2 1 2 2
2 3 1 2 1 1
1 3 2 1 0 1
4 2 2 1 3 3
1 3 3 2 0 2
3 1 1 2 3 1
1 2 3 2 2 1
2 1 1 2 1 2
2 3 3 1 1 3
4 1 1 1 3 1
2 2 2 1 0 1
2 3 2 1 0 1
4 2 2 1 3 3
3 3 2 2 1 2
4 2 1 1 3 1
1 3 2 2 0 2
4 1 2 1 2 2
4 2 2 1 2 1
2 2 1 2 2 3
2 3 3 2 1 3
2 2 1 1 1 1
4 3 2 1 0 1
2 1 2 2 0 1
4 3 1 1 2 3
1 3 2 2 0 1
2 3 3 1 0 1
1 1 1 1 0 2
4 1 2 1 3 1
1 2 3 1 0 1
3 2 3 2 3 2
1 1 3 1 0 3
2 3 3 2 2 3
2 3 2 1 0 1
2 2 2 1 2 3
2 1 1 2 3 2
4 3 1 1 1 1
2 2 1 1 2 2
4 2 3 1 3 3
1 1 3 2 0 2
2 1 3 2 1 1
1 3 1 2 0 1
2 1 1 2 0 2
2 3 3 2 3 3
3 2 1 1 3 1
4 3 2 2 0 2
1 3 1 2 0 1
2 2 2 1 1 1
4 3 2 1 2 3
1 2 3 2 3 3
1 3 2 2 0 1
1 2 1 2 2 1
3 2 3 1 2 1
2 1 2 2 2 2
1 3 3 1 1 1
4 1 2 1 2 2
4 2 2 1 2 1
2 1 3 1 2 1
2 2 1 1 3 1
1 3 3 2 0 2
1 2 3 1 3 1
1 3 3 2 1 2
4 3 1 1 1 1
1 1 1 2 1 3
3 2 1 2 3 2
1 2 3 2 2 1
2 1 1 2 1 1
4 1 2 1 2 1
1 2 2 2 1 2
1 2 2 1 0 2
1 2 3 1 2 1
3 2 1 1 0 3
2 3 1 1 0 3
2 1 2 2 2 1
1 2 2 2 1 1
2 3 3 2 0 1
1 1 2 1 0 2
2 1 2 1 1 3
2 2 2 2 2 2
1 1 3 1 1 2
4 1 1 1 1 1
3 1 3 2 1 1
2 3 1 1 0 1
1 3 2 1 0 2
2 3 1 1 0 1
4 1 1 1 3 1
2 3 3 2 1 2
2 3 3 1 3 1
3 1 3 1 3 3
1 3 1 2 0 1
1 3 1 2 0 2
4 2 1 1 3 2
1 3 2 2 2 3
4 2 2 1 3 1
1 3 1 2 0 1
2 2 1 2 3 2
4 2 2 1 1 2
3 3 3 2 1 1
2 3 2 2 1 2
4 3 3 2 0 1
1 3 2 1 0 1
3 2 1 1 2 3
2 2 1 2 3 3
3 3 1 1 3 3
2 2 2 2 3 1
4 3 1 2 2 3
1 2 2 2 2 2
2 3 2 2 3 2
2 2 2 1 2 1
1 3 3 1 0 1
2 2 3 1 2 1
3 1 3 2 1 1
4 2 1 1 3 1
3 3 3 1 2 1
4 2 3 1 1 1
2 2 1 2 2 1
2 2 3 1 2 1
1 3 1 1 0 1
1 3 1 2 2 1
3 2 1 2 1 1
4 2 1 1 3 1
2 1 3 1 3 1
1 3 2 2 1 3
4 2 3 2 2 1
4 2 2 1 3 2
3 1 1 1 1 1
1 3 2 2 0 1
2 1 1 1 1 2
1 3 1 2 1 2
2 1 3 2 2 3
3 2 3 1 1 2
2 3 2 1 2 3
1 2 1 1 1 3
3 2 3 1 3 2
4 2 1 1 2 1
END DATA.
WEIGHT BY w.
";

const PLUM_ROWS: [&str; 7] = [
    "Threshold / [y4=1]",
    "Threshold / [y4=2]",
    "Threshold / [y4=3]",
    "Location / z",
    "Location / [f=1]",
    "Location / [f=2]",
    "Location / [g=1]",
];

fn estimates(tables: &[Table], rows: &[&str], b: &[f64], se: &[f64], what: &str) {
    for ((row, b), se) in rows.iter().zip(b).zip(se) {
        let est = number(cell(tables, "Parameter Estimates", row, "Estimate"));
        let err = number(cell(tables, "Parameter Estimates", row, "Std. Error"));
        assert_close(est, *b, 2e-6, &format!("{what} {row} estimate"));
        assert_close(err, *se, 2e-5, &format!("{what} {row} standard error"));
    }
}

fn plum(options: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let t = run(&mut engine, &format!("{DATA}{options}"));
    (engine, t)
}

type LinkCase = (
    &'static str,
    [f64; 7],
    [f64; 7],
    f64,
    [f64; 3],
    [f64; 2],
    [f64; 2],
);

#[test]
fn plum_every_link_with_fit_pseudo_r_squares_and_parallel_lines() {
    // per link: estimates, standard errors, -2LL final, Cox and Snell, Nagelkerke, McFadden,
    // Pearson, deviance, general model -2LL and the chi-square of parallel lines
    let cases: [LinkCase; 5] = [
        (
            "LOGIT",
            [
                1.545824595,
                3.417310194,
                4.303247953,
                0.8755777685,
                0.8310374051,
                0.5944374595,
                1.444404285,
            ],
            [
                0.3348040903,
                0.3923149103,
                0.428233873,
                0.1236623756,
                0.3061744543,
                0.3153074575,
                0.2633259038,
            ],
            222.61804,
            [0.3213626135, 0.3453507443, 0.14535738],
            [118.7632323, 122.2569172],
            [195.7725722, 26.84546787],
        ),
        (
            "PROBIT",
            [
                0.8776143938,
                1.971363094,
                2.481403622,
                0.5067026917,
                0.4944194223,
                0.3848730593,
                0.8106949232,
            ],
            [
                0.189452305,
                0.2105978363,
                0.2253038397,
                0.07117719353,
                0.1805475609,
                0.1850395422,
                0.1514182575,
            ],
            224.5868724,
            [0.3156773927, 0.3392411499, 0.1422293313],
            [129.4210976, 124.2257496],
            [200.2739143, 24.31295811],
        ),
        (
            "CLOGLOG",
            [
                0.2009611676,
                1.371497718,
                1.836232736,
                0.4125032976,
                0.3668429067,
                0.4118743782,
                0.7432057905,
            ],
            [
                0.1787198934,
                0.1730387418,
                0.1793078705,
                0.0737690245,
                0.1845946731,
                0.191341756,
                0.158801743,
            ],
            240.8752482,
            [0.2667783849, 0.2866920729, 0.1163506266],
            [178.023419, 140.5141254],
            [210.2268705, 30.64837772],
        ),
        (
            "NLOGLOG",
            [
                1.628942807,
                2.882899431,
                3.556916421,
                0.6472044666,
                0.7019638194,
                0.4753836335,
                0.9401378192,
            ],
            [
                0.2591662536,
                0.3023280572,
                0.3265804399,
                0.08395217327,
                0.2060504248,
                0.2114742463,
                0.1724237498,
            ],
            220.6888903,
            [0.3268874305, 0.3512879616, 0.1484223815],
            [105.378981, 120.3277674],
            [188.5142476, 32.17464263],
        ),
        (
            "CAUCHIT",
            [
                1.273428,
                3.199074407,
                4.127232809,
                0.851047871,
                0.5755057458,
                0.278338484,
                1.373957399,
            ],
            [
                0.3488904669,
                0.4953186406,
                0.5699721228,
                0.148671879,
                0.2867798361,
                0.3037969145,
                0.2751739486,
            ],
            230.4112167,
            [0.2985785778, 0.320865993, 0.1329757085],
            [119.1778265, 130.0500938],
            [185.0196555, 45.39156114],
        ),
    ];
    for (link, b, se, m2ll, pseudo, gof, general) in cases {
        let (_, t) = plum(&format!(
            "PLUM y4 BY f g WITH z /LINK={link} /PRINT=FIT SUMMARY PARAMETER TPARALLEL."
        ));
        estimates(&t, &PLUM_ROWS, &b, &se, link);
        let fit = |row: &str| {
            number(cell(
                &t,
                "Model Fitting Information",
                row,
                "-2 Log Likelihood",
            ))
        };
        assert_close(
            fit("Intercept Only"),
            314.107767,
            1e-7,
            "-2LL of the thresholds",
        );
        assert_close(fit("Final"), m2ll, 1e-7, &format!("{link} -2LL"));
        let r2 = |row: &str| number(cell(&t, "Pseudo R-Square", row, "Value"));
        assert_close(r2("Cox and Snell"), pseudo[0], 1e-7, "Cox and Snell");
        assert_close(r2("Nagelkerke"), pseudo[1], 1e-7, "Nagelkerke");
        assert_close(r2("McFadden"), pseudo[2], 1e-7, "McFadden");
        let g = |row: &str, c: &str| number(cell(&t, "Goodness-of-Fit", row, c));
        assert_close(
            g("Pearson", "Chi-Square"),
            gof[0],
            1e-6,
            &format!("{link} Pearson"),
        );
        assert_close(
            g("Deviance", "Chi-Square"),
            gof[1],
            1e-6,
            &format!("{link} deviance"),
        );
        assert_eq!(g("Pearson", "df"), 24.0 * 3.0 - 7.0);
        let p = |row: &str, c: &str| number(cell(&t, "Test of Parallel Lines", row, c));
        assert_close(
            p("General", "-2 Log Likelihood"),
            general[0],
            1e-6,
            &format!("{link} general"),
        );
        assert_close(
            p("General", "Chi-Square"),
            general[1],
            1e-5,
            &format!("{link} parallel chi-square"),
        );
        assert_eq!(p("General", "df"), 8.0);
    }
}

#[test]
fn plum_delta_and_bias_change_the_cell_counts_before_the_fit() {
    // DELTA goes into each empty cell, BIAS into every cell, so an empty cell gets both
    let cases: [(&str, [f64; 7], [f64; 7]); 3] = [
        (
            "DELTA(0.3)",
            [
                1.290994162,
                3.015619809,
                3.897673142,
                0.7854097648,
                0.7352396226,
                0.5135283409,
                1.2863069,
            ],
            [
                0.319113642,
                0.3649261317,
                0.3978722477,
                0.1179151107,
                0.2969320425,
                0.3045802452,
                0.2525629264,
            ],
        ),
        (
            "BIAS(0.1)",
            [
                1.410230773,
                3.21482078,
                4.101249148,
                0.8258007218,
                0.7823431169,
                0.5643458322,
                1.365286768,
            ],
            [
                0.3228181929,
                0.3739179525,
                0.4081023583,
                0.1193527837,
                0.297496358,
                0.3065504219,
                0.2549214578,
            ],
        ),
        (
            "DELTA(0.3) BIAS(0.1)",
            [
                1.179234644,
                2.854608676,
                3.738758945,
                0.7448446765,
                0.696740878,
                0.4906072063,
                1.222994516,
            ],
            [
                0.309171277,
                0.3503481413,
                0.3818271767,
                0.1143217195,
                0.2893555815,
                0.2968769722,
                0.2454831048,
            ],
        ),
    ];
    for (options, b, se) in cases {
        let (_, t) = plum(&format!("PLUM y4 BY f g WITH z /CRITERIA={options}."));
        estimates(&t, &PLUM_ROWS, &b, &se, options);
    }
    // 90% limits from the same standard error
    let (_, t) = plum("PLUM y4 BY f g WITH z /CRITERIA=CIN(90).");
    let at = |c: &str| number(cell(&t, "Parameter Estimates", "Location / z", c));
    let z90 = 1.6448536269514722;
    assert_close(
        at("90% Confidence Interval / Lower Bound"),
        at("Estimate") - z90 * at("Std. Error"),
        1e-9,
        "lower",
    );
    assert_close(
        at("90% Confidence Interval / Upper Bound"),
        at("Estimate") + z90 * at("Std. Error"),
        1e-9,
        "upper",
    );
}

#[test]
fn plum_scale_model_and_saved_probabilities() {
    let mut rows = PLUM_ROWS.to_vec();
    rows.push("Scale / [g=1]");
    let (_, t) = plum("PLUM y4 BY f g WITH z /LINK=PROBIT /SCALE=g /PRINT=PARAMETER.");
    estimates(
        &t,
        &rows,
        &[
            0.8882586392,
            1.937924375,
            2.422059221,
            0.4951569446,
            0.5035982814,
            0.3962909801,
            0.7836590319,
            -0.08253577426,
        ],
        &[
            0.1850677369,
            0.2168868108,
            0.2506171709,
            0.07235122829,
            0.1735868047,
            0.17782152,
            0.1551979949,
            0.1630222565,
        ],
        "probit scale model",
    );
    assert_close(
        number(cell(
            &t,
            "Model Fitting Information",
            "Final",
            "-2 Log Likelihood",
        )),
        224.3329793,
        1e-7,
        "-2LL of the scale model",
    );
    // case, the four probabilities, the predicted category, its probability, the observed one's
    let expected: [(usize, [f64; 4], f64, f64, f64); 5] = [
        (
            0,
            [0.8099234923, 0.1657353281, 0.01779787488, 0.006543304738],
            1.0,
            0.8099234923,
            0.8099234923,
        ),
        (
            1,
            [0.1719504021, 0.3865872963, 0.185969349, 0.2554929526],
            2.0,
            0.3865872963,
            0.1719504021,
        ),
        (
            2,
            [0.2047833797, 0.4012873387, 0.1759785799, 0.2179507017],
            2.0,
            0.4012873387,
            0.1759785799,
        ),
        (
            4,
            [0.03302665969, 0.1953169206, 0.1790431616, 0.5926132582],
            4.0,
            0.5926132582,
            0.5926132582,
        ),
        (
            7,
            [0.4508525306, 0.3831842752, 0.09656412605, 0.06939906806],
            1.0,
            0.4508525306,
            0.3831842752,
        ),
    ];
    let (engine, _) =
        plum("PLUM y4 BY f g WITH z /LINK=PROBIT /SAVE=ESTPROB(p) PREDCAT PCPROB ACPROB(a).");
    let probabilities: Vec<Vec<Option<f64>>> =
        (1..=4).map(|c| values(&engine, &format!("p{c}"))).collect();
    let predicted = values(&engine, "PRE_1");
    let top = values(&engine, "PCP_1");
    let actual = values(&engine, "a");
    for (case, p, category, best, own) in expected {
        for (c, want) in p.iter().enumerate() {
            assert_close(probabilities[c][case].unwrap(), *want, 1e-6, "ESTPROB");
        }
        assert_eq!(predicted[case], Some(category), "PREDCAT of case {case}");
        assert_close(top[case].unwrap(), best, 1e-6, "PCPROB");
        assert_close(actual[case].unwrap(), own, 1e-6, "ACPROB");
    }
}

fn nomreg(options: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    run(&mut engine, &format!("{DATA}{options}"))
}

/// The estimates and standard errors of the two logits (rows `a / name` and `b / name`).
fn logits(t: &[Table], first: &str, second: &str, b: &[f64], se: &[f64], what: &str) {
    let names = ["Intercept", "z", "[f=1]", "[f=2]", "[g=1]"];
    for (i, (b, se)) in b.iter().zip(se).enumerate() {
        let category = if i < 5 { first } else { second };
        let row = format!("{category} / {}", names[i % 5]);
        let est = number(cell(t, "Parameter Estimates", &row, "B"));
        let err = number(cell(t, "Parameter Estimates", &row, "Std. Error"));
        assert_close(est, *b, 2e-6, &format!("{what} {row} B"));
        assert_close(err, *se, 2e-5, &format!("{what} {row} standard error"));
    }
}

#[test]
fn nomreg_dispersion_scale_changes_only_the_covariance() {
    let b = [
        -1.329685558,
        0.3341766674,
        0.1713663697,
        -0.02601099977,
        0.5913794861,
        -2.547917031,
        0.9872388999,
        0.6207616994,
        0.4688659091,
        0.920178334,
    ];
    let t = nomreg("NOMREG y3 (BASE=LAST) BY f g WITH z /PRINT=PARAMETER FIT.");
    logits(
        &t,
        "1",
        "2",
        &b,
        &[
            0.4082782735,
            0.1673222997,
            0.4221216716,
            0.4288491178,
            0.3494225619,
            0.4715437089,
            0.1682080649,
            0.4186247199,
            0.4208710897,
            0.3398861444,
        ],
        "scale 1",
    );
    assert_close(
        number(cell(&t, "Goodness-of-Fit", "Pearson", "Chi-Square")),
        47.33491848,
        1e-7,
        "Pearson",
    );
    assert_close(
        number(cell(&t, "Goodness-of-Fit", "Deviance", "Chi-Square")),
        62.13568038,
        1e-7,
        "deviance",
    );
    assert_eq!(number(cell(&t, "Goodness-of-Fit", "Deviance", "df")), 38.0);
    // SCALE=2, then the Pearson and deviance chi-squares over the 38 df (1.2457 and 1.6351)
    let scaled: [(&str, [f64; 10]); 3] = [
        (
            "2",
            [
                0.5773926717,
                0.2366294656,
                0.596970193,
                0.6064842386,
                0.494158126,
                0.6668635083,
                0.2378821267,
                0.5920247564,
                0.5952016031,
                0.4806715951,
            ],
        ),
        (
            "PEARSON",
            [
                0.4556750901,
                0.1867466602,
                0.4711255612,
                0.4786339931,
                0.3899868489,
                0.5262849775,
                0.1877352534,
                0.4672226501,
                0.4697297999,
                0.3793433535,
            ],
        ),
        (
            "DEVIANCE",
            [
                0.5220775284,
                0.2139599836,
                0.5397794918,
                0.5483820766,
                0.4468169856,
                0.6029769155,
                0.2150926377,
                0.535307836,
                0.5381803357,
                0.434622486,
            ],
        ),
    ];
    for (option, se) in scaled {
        let t = nomreg(&format!(
            "NOMREG y3 (BASE=LAST) BY f g WITH z /SCALE={option} /PRINT=PARAMETER."
        ));
        logits(&t, "1", "2", &b, &se, option);
        // the Wald statistic uses the scaled standard error
        let est = number(cell(&t, "Parameter Estimates", "1 / z", "B"));
        let err = number(cell(&t, "Parameter Estimates", "1 / z", "Std. Error"));
        assert_close(
            number(cell(&t, "Parameter Estimates", "1 / z", "Wald")),
            (est / err).powi(2),
            1e-9,
            "Wald",
        );
    }
    // 90% limits of Exp(B)
    let t = nomreg("NOMREG y3 (BASE=LAST) BY f g WITH z /CRITERIA=CIN(90) /PRINT=PARAMETER.");
    let ci = [1.060728542, 1.839322614, 0.5927621877, 2.376656653];
    let lower = "90% Confidence Interval for Exp(B) / Lower Bound";
    let upper = "90% Confidence Interval for Exp(B) / Upper Bound";
    for (row, lo, hi) in [("1 / z", ci[0], ci[1]), ("1 / [f=1]", ci[2], ci[3])] {
        assert_close(
            number(cell(&t, "Parameter Estimates", row, lower)),
            lo,
            1e-6,
            row,
        );
        assert_close(
            number(cell(&t, "Parameter Estimates", row, upper)),
            hi,
            1e-6,
            row,
        );
    }
}

#[test]
fn nomreg_delta_and_bias_and_the_category_order_with_a_base() {
    // DELTA goes into each empty cell and BIAS into every cell; an empty cell gets both
    let cases: [(&str, [f64; 10], [f64; 10], f64); 3] = [
        (
            "DELTA(0.3)",
            [
                -1.288358919,
                0.335265762,
                0.1733571491,
                -0.02607249174,
                0.5338621575,
                -2.371662872,
                0.9315041982,
                0.5673158645,
                0.4093693307,
                0.8580686177,
            ],
            [
                0.4020823678,
                0.1642693477,
                0.4180279527,
                0.4240962901,
                0.3452496192,
                0.4548554694,
                0.1634093731,
                0.4111452095,
                0.4131236444,
                0.3335913456,
            ],
            145.3190579,
        ),
        (
            "BIAS(0.1)",
            [
                -1.288408887,
                0.3240217599,
                0.1657005835,
                -0.02372068598,
                0.569517363,
                -2.451147175,
                0.9507453096,
                0.5950949775,
                0.4484201376,
                0.8917694936,
            ],
            [
                0.3996037266,
                0.16343505,
                0.414056724,
                0.420265771,
                0.3425161464,
                0.4577184227,
                0.1636097139,
                0.4096563528,
                0.4117674025,
                0.3326740664,
            ],
            151.0832496,
        ),
        (
            "DELTA(0.3) BIAS(0.1)",
            [
                -1.249501383,
                0.325312527,
                0.167657023,
                -0.02412280652,
                0.5150462179,
                -2.286407886,
                0.8987631527,
                0.5453260152,
                0.3925164809,
                0.8329607071,
            ],
            [
                0.393925298,
                0.1606259241,
                0.4102471212,
                0.4158384462,
                0.3386421625,
                0.4425520221,
                0.1592713058,
                0.4028208534,
                0.4046570114,
                0.326905232,
            ],
            145.9904994,
        ),
    ];
    for (options, b, se, m2ll) in cases {
        let t = nomreg(&format!(
            "NOMREG y3 (BASE=LAST) BY f g WITH z /CRITERIA={options} /PRINT=PARAMETER MFI."
        ));
        logits(&t, "1", "2", &b, &se, options);
        let m2 = number(cell(
            &t,
            "Model Fitting Information",
            "Final",
            "Model Fitting Criteria / -2 Log Likelihood",
        ));
        assert_close(m2, m2ll, 1e-7, options);
    }
    // ORDER=DESCENDING puts 3, 2, 1 in that order, so BASE=LAST is category 1
    let t = nomreg("NOMREG y3 (BASE=LAST ORDER=DESCENDING) BY f g WITH z /PRINT=PARAMETER.");
    logits(
        &t,
        "3",
        "2",
        &[
            1.329686672,
            -0.3341776261,
            -0.1713663932,
            0.02601083795,
            -0.5913793201,
            -1.218230145,
            0.6530615498,
            0.4493950722,
            0.4948765563,
            0.3287989366,
        ],
        &[
            0.4082783349,
            0.1673223161,
            0.4221217059,
            0.4288491484,
            0.3494225858,
            0.5138179403,
            0.1775685682,
            0.4472902145,
            0.4580483328,
            0.3718877775,
        ],
        "descending",
    );
    // ORDER=DATA: the file meets 3, then 1, then 2; BASE=LAST is 2
    let t = nomreg("NOMREG y3 (BASE=LAST ORDER=DATA) BY f g WITH z /PRINT=PARAMETER.");
    logits(
        &t,
        "3",
        "1",
        &[
            2.547918682,
            -0.987239844,
            -0.6207619351,
            -0.4688661325,
            -0.9201783332,
            1.218232697,
            -0.6530628415,
            -0.4493954738,
            -0.4948772025,
            -0.3287988039,
        ],
        &[
            0.4715438402,
            0.1682081172,
            0.4186247846,
            0.4208711503,
            0.3398861924,
            0.5138180781,
            0.1775686203,
            0.4472902863,
            0.4580484126,
            0.3718878356,
        ],
        "data order",
    );
}
