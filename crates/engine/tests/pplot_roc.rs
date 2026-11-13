//! PPLOT and ROC on small made-up data sets. PPLOT's plotting positions, estimates and
//! expected values were computed independently with the quantile functions of a general
//! statistics package; ROC's area, standard error and significance by brute force over every
//! pair and triple of cases, and the unweighted and weighted results agree with another
//! open-source implementation.

use statscraft_engine::chart::ChartKind;
use statscraft_engine::plot::{Plot, Point, ReferenceLine};
use statscraft_engine::{Cell, Chart, Engine, Table};
use statscraft_syntax::parse;

/// Hours each of ten pumps ran before its first repair; three ran 4.4 hours.
const PUMPS: &str = "DATA LIST LIST /hours.
BEGIN DATA
3.2
7.5
1.8
4.4
4.4
9.1
2.6
5.0
6.3
4.4
END DATA.
";

/// A blood marker and whether the patient relapsed (1) in ten patients, with a case
/// weight; the marker of patient 11 is missing.
const RELAPSE: &str = "DATA LIST LIST /marker relapse w.
BEGIN DATA
2.1 0 1
3.4 0 2
3.4 1 1
4.0 0 1
4.8 1 1
5.5 0 1
5.5 1 3
6.2 1 1
7.0 1 1
1.5 0 1
. 1 1
END DATA.
";

/// The tables and charts of the last command, and the warnings.
fn run(data: &str, source: &str) -> (Vec<Table>, Vec<Chart>, Vec<String>) {
    let (commands, errors) = parse(&format!("{data}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let (mut tables, mut charts) = (Vec::new(), Vec::new());
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
        charts = engine.take_charts();
    }
    (tables, charts, engine.warnings().to_vec())
}

fn close(got: f64, want: f64, tolerance: f64) {
    assert!((got - want).abs() < tolerance, "{got} against {want}");
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn plot(chart: &Chart) -> (&[Point], &[ReferenceLine]) {
    match &chart.plot {
        Some(Plot::Points { groups, lines, .. }) => (&groups[0].points, lines),
        other => panic!("{other:?}"),
    }
}

/// Where each pump's point is in a plot, which lists the points by observed value.
const SORTED: [usize; 10] = [2, 8, 0, 3, 4, 9, 1, 6, 7, 5];

/// Blom positions of the pumps in case order; the three 4.4 share the mean rank 5.
const BLOM: [f64; 10] = [
    0.25609756097561,
    0.841463414634146,
    0.0609756097560976,
    0.451219512195122,
    0.451219512195122,
    0.939024390243902,
    0.158536585365854,
    0.646341463414634,
    0.74390243902439,
    0.451219512195122,
];

#[test]
fn normal_q_q_and_detrended_plots_by_default() {
    let (tables, charts, warnings) = run(PUMPS, "PPLOT VARIABLES=hours.");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].kind, ChartKind::Qq);
    assert_eq!(charts[0].title, "Normal Q-Q plot of hours");
    assert_eq!(charts[1].kind, ChartKind::DetrendedQq);
    let (points, lines) = plot(&charts[0]);
    // the expected value is the normal quantile at the Blom position, mean 4.87 and S 2.2336
    let expected = [
        3.40604150455501,
        7.10470263451708,
        1.41542471114165,
        4.59620257534761,
        4.59620257534761,
        8.32457528885835,
        2.63529736548292,
        5.7086340188616,
        6.33395849544499,
        4.59620257534761,
    ];
    for (case, e) in expected.iter().enumerate() {
        close(points[SORTED[case]].y, *e, 1e-9);
    }
    assert_eq!(points[SORTED[1]].x, 7.5);
    assert_eq!(lines[0].slope, 1.0);
    let (detrended, lines) = plot(&charts[1]);
    close(detrended[SORTED[1]].y, 7.5 - 7.10470263451708, 1e-9);
    assert_eq!((lines[0].intercept, lines[0].slope), (0.0, 0.0));
    let estimates = tables
        .iter()
        .find(|t| t.title == "Estimated Distribution Parameters")
        .unwrap();
    close(num(&estimates.rows[0][2]), 4.87, 1e-12);
    close(num(&estimates.rows[1][2]), 2.23360694841326, 1e-12);
}

#[test]
fn p_p_plot_against_the_estimated_normal() {
    let (_, charts, _) = run(PUMPS, "PPLOT hours /TYPE=P-P /PLOT=NORMAL.");
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].kind, ChartKind::Pp);
    let (points, _) = plot(&charts[0]);
    let cdf = [0.227329737894797, 0.880495555640933, 0.0846496966284439];
    for case in 0..3 {
        close(points[SORTED[case]].x, BLOM[case], 1e-12);
        close(points[SORTED[case]].y, cdf[case], 1e-9);
    }
}

#[test]
fn estimates_of_each_distribution() {
    // (subcommand, estimates, expected values of the first three pumps)
    let cases: [(&str, &[f64], &[f64]); 6] = [
        (
            "EXPONENTIAL",
            &[0.205338809034908],
            &[1.44076701565289, 8.96941936333923, 0.306390329749472],
        ),
        (
            "GAMMA",
            &[4.75383844457807, 0.976147524554019],
            &[3.27148609487769, 7.0280135685071, 1.9774565402159],
        ),
        (
            "WEIBULL",
            &[5.5240794203124, 2.33183727726799],
            &[3.2766342065559, 7.17802793839282, 1.68696670615718],
        ),
        (
            "LNORMAL",
            &[4.40368967593948, 0.487023774951915],
            &[3.20027237230553, 7.16856456602036, 2.07341001865765],
        ),
        (
            "LOGISTIC",
            &[4.87, 1.23145205167512],
            &[3.55683934809159, 6.92548699340848, 1.50275752049146],
        ),
        ("PARETO", &[1.8, 1.11774794153033], &[]),
    ];
    for (name, estimates, expected) in cases {
        let (tables, charts, warnings) = run(
            PUMPS,
            &format!("PPLOT hours /DISTRIBUTION={name} /PLOT=NORMAL."),
        );
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
        let table = tables.last().unwrap();
        assert_eq!(table.title, "Estimated Distribution Parameters");
        for (row, want) in table.rows.iter().zip(estimates) {
            close(num(&row[2]), *want, 1e-8);
        }
        let (points, _) = plot(&charts[0]);
        for (case, want) in expected.iter().enumerate() {
            close(points[SORTED[case]].y, *want, 1e-6);
        }
    }
    let others: [(&str, &[f64]); 3] = [
        ("LAPLACE", &[4.87, 1.57939861972841]),
        ("HNORMAL", &[5.31102626617493]),
        ("UNIFORM", &[1.8, 9.1]),
    ];
    for (name, estimates) in others {
        let (tables, ..) = run(PUMPS, &format!("PPLOT hours /DISTRIBUTION={name}."));
        let table = tables.last().unwrap();
        for (row, want) in table.rows.iter().zip(estimates) {
            close(num(&row[2]), *want, 1e-10);
        }
    }
    // beta on the hours in tenths of 10 hours
    let (tables, ..) = run(
        PUMPS,
        "COMPUTE share = hours / 10.\nPPLOT share /DISTRIBUTION=BETA.",
    );
    let table = tables.last().unwrap();
    close(num(&table.rows[0][2]), 1.95171912206855, 1e-9);
    close(num(&table.rows[1][2]), 2.05591767889357, 1e-9);
}

#[test]
fn given_degrees_of_freedom_tukey_fractions_and_low_ties() {
    let (tables, charts, _) = run(
        PUMPS,
        "PPLOT hours /DISTRIBUTION=CHI(4) /FRACTION=TUKEY /TIES=LOW /PLOT=NORMAL.",
    );
    // nothing estimated, so no estimates table
    assert!(
        tables
            .iter()
            .all(|t| t.title != "Estimated Distribution Parameters")
    );
    let (points, _) = plot(&charts[0]);
    let want = [
        1.96641892142018,
        6.55609110040041,
        0.821931661432687,
        2.49706576408784,
    ];
    for (case, w) in want.iter().enumerate() {
        close(points[SORTED[case]].y, *w, 1e-6);
    }
    assert_eq!(charts[0].title, "Chi-square Q-Q plot of hours");
}

#[test]
fn differencing_logs_and_standardizing_come_first() {
    // the differences of 1, 3, 6, 10, 15 are 2, 3, 4, 5: their z-scores are plotted
    let data = "DATA LIST LIST /v.\nBEGIN DATA\n1\n3\n6\n10\n15\nEND DATA.\n";
    let (tables, charts, _) = run(data, "PPLOT v /DIFF /STANDARDIZE /PLOT=NORMAL.");
    let (points, _) = plot(&charts[0]);
    let sd = (5.0f64 / 3.0).sqrt();
    let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
    for (x, d) in xs.iter().zip([2.0, 3.0, 4.0, 5.0]) {
        close(*x, (d - 3.5) / sd, 1e-12);
    }
    let cases = &tables[1];
    assert_eq!(cases.title, "Case Processing Summary");
    let column: Vec<f64> = cases.rows.iter().map(|r| num(&r[1])).collect();
    assert_eq!(column, [5.0, 0.0, 1.0, 4.0]);
    // LN: the logs of 1, e, e^3 differ by 1 and 2; a value of 0 or less becomes missing
    let data =
        "DATA LIST LIST /v.\nBEGIN DATA\n1\n2.718281828459045\n20.085536923187668\n0\nEND DATA.\n";
    let (_, charts, _) = run(data, "PPLOT v /LN /DIFF=1 /PLOT=NORMAL.");
    let (points, _) = plot(&charts[0]);
    assert_eq!(points.len(), 2);
    close(points[0].x, 1.0, 1e-12);
    close(points[1].x, 2.0, 1e-12);
    // seasonal differencing over a period of 2: 5-1, 7-2, 10-5
    let data = "DATA LIST LIST /v.\nBEGIN DATA\n1\n2\n5\n7\n10\nEND DATA.\n";
    let (_, charts, _) = run(data, "PPLOT v /SDIFF /PERIOD=2 /PLOT=NORMAL.");
    let xs: Vec<f64> = plot(&charts[0]).0.iter().map(|p| p.x).collect();
    assert_eq!(xs, [4.0, 5.0, 5.0]);
    let (_, _, warnings) = run(data, "PPLOT v /SDIFF.");
    assert!(warnings[0].contains("PERIOD"), "{warnings:?}");
}

#[test]
fn apply_repeats_the_previous_specifications() {
    let source = "PPLOT hours /DISTRIBUTION=EXPONENTIAL(0.5) /TYPE=P-P /PLOT=DETRENDED.\n\
                  COMPUTE twice = hours * 2.\n\
                  PPLOT twice /APPLY /PLOT=BOTH.";
    let (tables, charts, _) = run(PUMPS, source);
    // P-P and both plots; the distribution applies but its rate is estimated again
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].kind, ChartKind::Pp);
    let table = tables.last().unwrap();
    assert_eq!(table.title, "Estimated Distribution Parameters");
    close(num(&table.rows[0][2]), 1.0 / 9.74, 1e-12);
}

#[test]
fn values_the_distribution_cannot_take_leave_the_variable_out() {
    let data = "DATA LIST LIST /v.\nBEGIN DATA\n-1\n2\n3\nEND DATA.\n";
    let (_, charts, warnings) = run(data, "PPLOT v /DISTRIBUTION=GAMMA.");
    assert!(charts.is_empty());
    assert!(warnings[0].contains("negative"), "{warnings:?}");
}

#[test]
fn roc_area_error_significance_and_interval() {
    let (tables, charts, _) = run(
        RELAPSE,
        "ROC marker BY relapse (1) /PRINT=SE COORDINATES /CRITERIA=CI(90) /PLOT=CURVE(REFERENCE).",
    );
    let summary = &tables[0];
    let counts: Vec<f64> = summary.rows.iter().map(|r| num(&r[1])).collect();
    assert_eq!(counts, [5.0, 5.0, 1.0]);
    let area = &tables[1].rows[0];
    close(num(&area[1]), 0.84, 1e-12);
    close(num(&area[2]), 0.1296559549988612, 1e-12);
    close(num(&area[3]), 0.07580017458236132, 1e-9);
    close(num(&area[4]), 0.6267349321642663, 1e-9);
    close(num(&area[5]), 1.0532650678357336, 1e-9);
    assert!(tables[1].caption.as_ref().unwrap().contains("marker"));
    // nine cutoffs from 0.5 to 8, sensitivity falling from 1 to 0
    let coordinates = &tables[2];
    let cuts: Vec<f64> = coordinates.rows.iter().map(|r| num(&r[1])).collect();
    let want = [0.5, 1.8, 2.75, 3.7, 4.4, 5.15, 5.85, 6.6, 8.0];
    for (c, w) in cuts.iter().zip(want) {
        close(*c, w, 1e-12);
    }
    let at_3_7: Vec<f64> = coordinates.rows[3][2..].iter().map(num).collect();
    assert_eq!(at_3_7, [0.8, 0.4]);
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].kind, ChartKind::Roc);
    let (points, lines) = plot(&charts[0]);
    assert_eq!((points[0].x, points[0].y), (0.0, 0.0));
    assert_eq!((points[8].x, points[8].y), (1.0, 1.0));
    assert_eq!(lines.len(), 1);
}

#[test]
fn roc_weights_small_direction_and_negexpo() {
    let (tables, ..) = run(
        RELAPSE,
        "WEIGHT BY w.\nROC marker BY relapse (1) /PRINT=SE.",
    );
    let counts: Vec<f64> = tables[0].rows.iter().map(|r| num(&r[1])).collect();
    assert_eq!(counts, [7.0, 6.0, 1.0]);
    let area = &tables[1].rows[0];
    close(num(&area[1]), 0.8690476190476191, 1e-12);
    close(num(&area[2]), 0.1034359153782913, 1e-12);
    close(num(&area[3]), 0.026809130715595764, 1e-9);
    // small values positive: the area turns around, the error stays
    let (tables, ..) = run(
        RELAPSE,
        "ROC marker BY relapse (1) /CRITERIA=TESTPOS(SMALL) DISTRIBUTION(NEGEXPO) /PRINT=SE.",
    );
    let area = &tables[1].rows[0];
    close(num(&area[1]), 0.16, 1e-12);
    close(num(&area[2]), 0.13475575106012405, 1e-12);
    // unequal groups under NEGEXPO fall back to the nonparametric error
    let (tables, ..) = run(
        RELAPSE,
        "WEIGHT BY w.\nROC marker BY relapse (1) /CRITERIA=DISTRIBUTION(NEGEXPO) /PRINT=SE.",
    );
    close(num(&tables[1].rows[0][2]), 0.1034359153782913, 1e-12);
    assert!(
        tables[1]
            .caption
            .as_ref()
            .unwrap()
            .contains("no distribution")
    );
}

#[test]
fn roc_string_state_and_errors() {
    let data = "DATA LIST LIST /score (F4.1) outcome (A3).
BEGIN DATA
1.0 no
2.0 yes
3.0 no
4.0 yes
END DATA.
";
    let (tables, ..) = run(data, "ROC score BY outcome ('yes') /PLOT=NONE.");
    close(num(&tables[1].rows[0][1]), 0.75, 1e-12);
    let (commands, _) = parse(&format!("{data}ROC score BY outcome ('maybe').\n"));
    let mut engine = Engine::default();
    let mut last = Ok(Vec::new());
    for command in &commands {
        last = engine.execute(&command.command);
    }
    assert!(last.unwrap_err().0.contains("both states"));
}
