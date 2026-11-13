//! KM and SURVIVAL on a made-up follow-up study: weeks until 36 customers cancelled a
//! subscription (outcome 1), were still subscribed when the study ended (0) or moved away and
//! were lost to it (2), on three plans in two regions, with tied times. The expected numbers
//! come from an independent general statistics package: its product-limit fit and log rank
//! test, and our own code there of the Breslow, Tarone-Ware, trend, life table and Gehan
//! formulas, the log rank part of which agrees with the package's own test.

use statscraft_engine::chart::ChartKind;
use statscraft_engine::plot::Plot;
use statscraft_engine::{Cell, Chart, Engine, Table};
use statscraft_syntax::parse;

const STUDY: &str = "DATA LIST LIST /weeks outcome plan region.
BEGIN DATA
3 1 1 1
5 1 1 2
5 1 1 1
5 0 1 2
8 1 1 1
9 2 1 1
12 1 1 2
12 1 1 1
15 0 1 2
18 1 1 1
22 1 1 2
22 0 1 1
2 1 2 2
6 1 2 1
7 0 2 1
10 1 2 2
12 1 2 1
14 2 2 2
16 1 2 1
19 0 2 2
21 1 2 1
25 1 2 2
28 0 2 1
30 0 2 2
4 1 3 1
8 1 3 2
8 1 3 2
11 0 3 1
13 1 3 2
17 1 3 1
20 0 3 2
24 1 3 1
26 2 3 2
29 1 3 1
33 1 3 2
35 1 3 1
END DATA.
";

/// The tables and charts of the last command, the warnings and the engine.
fn run(source: &str) -> (Vec<Table>, Vec<Chart>, Vec<String>, Engine) {
    let (commands, errors) = parse(&format!("{STUDY}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let (mut tables, mut charts) = (Vec::new(), Vec::new());
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
        charts = engine.take_charts();
    }
    let warnings = engine.warnings().to_vec();
    (tables, charts, warnings, engine)
}

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-9, "{got} against {want}");
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("{other:?}"),
    }
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

#[test]
fn km_estimates_errors_means_and_medians() {
    let (tables, ..) = run("KM weeks BY plan /STATUS=outcome(1) /PERCENTILES.");
    let summary = table(&tables, "Case Processing Summary");
    // plan 1: 12 cases, 8 events, 4 censored (the lost one among them)
    assert_eq!(
        summary.rows[0][1..].iter().map(num).collect::<Vec<_>>(),
        [12.0, 8.0, 4.0, 100.0 / 3.0]
    );
    assert_eq!(text(&summary.rows[3][0]), "Overall");
    close(num(&summary.rows[3][1]), 36.0);

    // the survival table: estimate and Greenwood error on the last event row of a time
    let st = table(&tables, "Survival Table");
    let estimates: Vec<(f64, f64, f64)> = st
        .rows
        .iter()
        .filter(|r| r[3] != Cell::Empty)
        .map(|r| (num(&r[1]), num(&r[3]), num(&r[4])))
        .collect();
    let want = [
        (3.0, 0.916666666667, 0.079785592313),
        (5.0, 0.75, 0.125),
        (8.0, 0.65625, 0.140190300351),
        (12.0, 0.4375, 0.157115657594),
        (18.0, 0.291666666667, 0.158585933982),
        (22.0, 0.145833333333, 0.130080956215),
        (2.0, 0.916666666667, 0.079785592313),
        (6.0, 0.833333333333, 0.107582870728),
        (10.0, 0.740740740741, 0.129482573848),
        (12.0, 0.648148148148, 0.14261129694),
        (16.0, 0.54012345679, 0.154428117822),
        (21.0, 0.405092592593, 0.164588958393),
        (25.0, 0.270061728395, 0.155548529982),
    ];
    for (got, want) in estimates.iter().zip(&want) {
        assert_eq!(got.0, want.0);
        assert!((got.1 - want.1).abs() < 1e-11 && (got.2 - want.2).abs() < 1e-11);
    }
    // plan 3 ends with an event: the estimate and its error reach 0
    let last = st.rows.last().unwrap();
    assert_eq!(
        (num(&last[1]), num(&last[3]), num(&last[4])),
        (35.0, 0.0, 0.0)
    );
    // tied events at 5 in plan 1: one row each, the estimate on the second, then the censored
    // case; the cumulative events and the remaining cases run on
    assert_eq!(text(&st.rows[1][2]), "Event");
    assert_eq!(st.rows[1][3], Cell::Empty);
    assert_eq!(text(&st.rows[3][2]), "Censored");
    assert_eq!((num(&st.rows[3][5]), num(&st.rows[3][6])), (3.0, 8.0));

    let means = table(&tables, "Means and Medians for Survival Time");
    // (mean, its error, median, its error) per plan and overall
    let want = [
        // the package leaves out the d/(d-1) factor where the longest time holds an event
        // and a censored case: its 2.12433041375 squared times 8/7 is 2.27100473845 squared
        (13.5, 2.27100473845, 12.0, 2.87297202457),
        (18.7453703704, 2.88605230152, 21.0, 6.0944940022),
        (21.7291666667, 3.2871786167, 24.0, 9.67700596489),
        (18.9062568026, 1.94837115923, 18.0, 4.73509648196),
    ];
    for (row, w) in means.rows.iter().zip(&want) {
        assert!((num(&row[1]) - w.0).abs() < 1e-9, "{row:?}");
        assert!((num(&row[2]) - w.1).abs() < 1e-9, "{row:?}");
        assert!((num(&row[5]) - w.2).abs() < 1e-9, "{row:?}");
        assert!((num(&row[6]) - w.3).abs() < 1e-9, "{row:?}");
        let z = 1.959963984540054;
        close(num(&row[3]), w.0 - z * w.1);
        close(num(&row[8]), w.2 + z * w.3);
    }
    assert!(means.caption.as_deref().unwrap().contains("censored"));

    // percentiles: the p% time is where survival first falls to p% or lower; the package
    // reads its quantile q as the 1 - q survival level, so its 0.75 quantile is our 25%
    let pct = table(&tables, "Percentiles");
    assert_eq!(pct.column_groups[1], ["25.0%"]);
    let row = |r: usize| pct.rows[r][1..].to_vec();
    let plan1 = row(0);
    close(num(&plan1[0]), 22.0);
    close(num(&plan1[1]), 4.45991849879);
    close(num(&plan1[4]), 5.0);
    close(num(&plan1[5]), 2.4);
    // plan 2 never falls to 25%
    assert_eq!((&row(1)[0], &row(1)[1]), (&Cell::Empty, &Cell::Empty));
    close(num(&row(1)[4]), 10.0);
    close(num(&row(1)[5]), 4.19523539268);
    close(num(&row(2)[0]), 33.0);
    close(num(&row(2)[1]), 3.54651628754);
    close(num(&row(2)[5]), 4.32);
    close(num(&row(3)[1]), 3.63198162696);
    close(num(&row(3)[5]), 2.0436480193);

    // the default comparison: the log rank test over all plans
    let overall = table(&tables, "Overall Comparisons");
    assert_eq!(overall.rows.len(), 1);
    close(num(&overall.rows[0][1]), 2.61482637919);
    assert_eq!(num(&overall.rows[0][2]), 2.0);
    close(num(&overall.rows[0][3]), 0.270518933256);
}

/// (chi-square, df) of each row of a comparison table, from its last three columns.
fn statistics(t: &Table) -> Vec<(f64, f64, f64)> {
    t.rows
        .iter()
        .map(|r| {
            let n = r.len();
            (num(&r[n - 3]), num(&r[n - 2]), num(&r[n - 1]))
        })
        .collect()
}

#[test]
fn km_tests_pairwise_strata_and_trend() {
    let (tables, ..) = run("KM weeks BY plan /STATUS=outcome(1) /PRINT=NONE \
         /TEST=LOGRANK BRESLOW TARONE /COMPARE=OVERALL POOLED /COMPARE=PAIRWISE.");
    assert!(tables.iter().all(|t| t.title != "Survival Table"));
    let overall = statistics(table(&tables, "Overall Comparisons"));
    for (got, want) in overall
        .iter()
        .zip([2.61482637919, 1.90779804403, 2.2382670169])
    {
        close(got.0, want);
    }
    close(overall[1].2, 0.385236047564);
    let pairs = statistics(table(&tables, "Pairwise Comparisons"));
    let want = [
        1.18143896871,
        2.3898251891,
        0.179862669539,
        0.94566385897,
        1.58549210295,
        0.132591268239,
        1.06974676554,
        1.94113528665,
        0.163783672914,
    ];
    for (got, want) in pairs.iter().zip(want) {
        close(got.0, want);
        assert_eq!(got.1, 1.0);
    }

    // pooled over the regions, and within each region
    let (tables, ..) = run(
        "KM weeks BY plan /STATUS=outcome(1) /STRATA=region /PRINT=NONE \
         /TEST=LOGRANK BRESLOW TARONE /COMPARE=OVERALL POOLED /COMPARE=OVERALL STRATA.",
    );
    let comparisons: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Overall Comparisons")
        .collect();
    let pooled = statistics(comparisons[0]);
    for (got, want) in pooled
        .iter()
        .zip([2.5294786492, 1.76074451469, 2.08726000591])
    {
        close(got.0, want);
    }
    assert!(
        comparisons[0]
            .caption
            .as_deref()
            .unwrap()
            .contains("Adjusted for region")
    );
    let within = statistics(comparisons[1]);
    let want = [
        1.97606155268,
        0.756099645544,
        1.91403116644,
        0.209065264398,
        2.02206421897,
        0.388098542817,
    ];
    for (got, want) in within.iter().zip(want) {
        close(got.0, want);
    }
    close(within[0].2, 0.372309129565);

    // the trend over the plans, with the default weights -1, 0, 1 and with given ones
    let (tables, ..) =
        run("KM weeks BY plan /STATUS=outcome(1) /PRINT=NONE /TEST=LOGRANK BRESLOW TARONE /TREND.");
    let trend = statistics(table(&tables, "Overall Comparisons"));
    for (got, want) in trend
        .iter()
        .zip([2.29330968706, 1.71467623458, 2.01745041928])
    {
        close(got.0, want);
        assert_eq!(got.1, 1.0);
    }
    close(trend[0].2, 0.129932593604);
    let (tables, _, warnings, _) =
        run("KM weeks BY plan /STATUS=outcome(1) /STRATA=region /PRINT=NONE /TREND=(2 4 6 8).");
    close(
        statistics(table(&tables, "Overall Comparisons"))[0].0,
        2.38252581481,
    );
    assert!(warnings.iter().any(|w| w.contains("extra")));
}

#[test]
fn km_lost_cases_ids_and_weights() {
    let (tables, ..) = run("STRING who (A4).
COMPUTE who = CONCAT('c', LTRIM(STRING($CASENUM, F3))).
KM weeks BY plan /STATUS=outcome EVENT(1) LOST(2) /ID=who /PRINT=TABLE.");
    let summary = table(&tables, "Case Processing Summary");
    assert_eq!(summary.columns.len(), 7);
    // plan 1: 3 censored, 1 lost
    assert_eq!(num(&summary.rows[0][3]), 3.0);
    assert_eq!(num(&summary.rows[0][5]), 1.0);
    let st = table(&tables, "Survival Table");
    assert_eq!(text(&st.rows[0][1]), "c1");
    assert!(st.rows.iter().any(|r| text(&r[3]) == "Lost"));
    assert!(tables.iter().all(|t| !t.title.starts_with("Means")));

    // a weight of 2 on every case changes the counts but no estimate or test
    let plain = run("KM weeks BY plan /STATUS=outcome(1) /PRINT=MEAN /TEST=BRESLOW.").0;
    let weighted = run("COMPUTE w = 2.
WEIGHT BY w.
KM weeks BY plan /STATUS=outcome(1) /PRINT=MEAN /TEST=BRESLOW.")
    .0;
    let mean = |t: &[Table]| num(&table(t, "Means and Medians for Survival Time").rows[0][1]);
    close(mean(&weighted), mean(&plain));
    let summary = table(&weighted, "Case Processing Summary");
    assert_eq!(num(&summary.rows[0][1]), 24.0);
}

#[test]
fn km_saves_values_at_each_case() {
    let (_, _, _, engine) = run(
        "KM weeks BY plan /STATUS=outcome(1) /SAVE=SURVIVAL HAZARD SE \
         CUMEVENT(events).",
    );
    let data = engine.dataset().unwrap();
    let get = |name: &str| -> Vec<Option<f64>> {
        data.variable(name)
            .unwrap()
            .numeric()
            .unwrap()
            .iter()
            .collect()
    };
    let (s, h, se, cum) = (get("SUR_1"), get("HAZ_1"), get("SE_1"), get("events"));
    // case 4: plan 1, censored at 5, after the two events at 5
    close(s[3].unwrap(), 0.75);
    close(h[3].unwrap(), -(0.75f64).ln());
    close(se[3].unwrap(), 0.125);
    assert_eq!(cum[3], Some(3.0));
    // case 36: plan 3's last event takes the estimate to 0, where the hazard has no value
    assert_eq!((s[35], h[35]), (Some(0.0), None));
}

#[test]
fn km_plots_are_step_curves_with_censored_ticks() {
    let (_, charts, ..) = run("KM weeks BY plan /STATUS=outcome(1) /STRATA=region \
         /PRINT=NONE /PLOT=SURVIVAL LOGSURV HAZARD OMS.");
    // four functions for each of two regions
    assert_eq!(charts.len(), 8);
    for chart in &charts {
        assert_eq!(chart.kind, ChartKind::Line);
        let Some(Plot::Points { groups, .. }) = &chart.plot else {
            panic!("{chart:?}");
        };
        assert_eq!(groups.len(), 3);
        for g in groups {
            for w in g.points.windows(2) {
                assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{w:?}");
                assert!(w[1].x >= w[0].x);
            }
        }
    }
    assert_eq!(charts[0].subtitle.as_deref(), Some("region = 1"));
    let Some(Plot::Points { groups, .. }) = &charts[0].plot else {
        unreachable!()
    };
    // region 1, plan 1: the censored cases at 9 (lost) and 22 are marked on the curve
    let marks: Vec<(f64, f64)> = groups[0]
        .points
        .iter()
        .filter(|p| p.label.is_some())
        .map(|p| (p.x, p.y))
        .collect();
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].0, 9.0);
    // the log survival curve of plan 3 in region 1 stops before the estimate reaches 0
    let Some(Plot::Points { groups, .. }) = &charts[1].plot else {
        unreachable!()
    };
    assert!(groups[2].points.iter().all(|p| p.y.is_finite()));
    assert!(charts[0].vega_lite()["layer"][1]["mark"]["type"] == "tick");
}

#[test]
fn km_without_a_factor_and_with_a_string_status() {
    let (tables, _, warnings, _) = run("STRING done (A3).
RECODE outcome (1='yes') (ELSE='no') INTO done.
KM weeks /STATUS=done('yes') /TEST=BRESLOW.");
    assert!(warnings.iter().any(|w| w.contains("factor")));
    let means = table(&tables, "Means and Medians for Survival Time");
    assert_eq!(means.rows.len(), 1);
    close(num(&means.rows[0][1]), 18.9062568026);
}

/// The life table columns from the interval start on, as numbers (None for an empty cell).
fn lines(t: &Table, lead: usize) -> Vec<Vec<Option<f64>>> {
    t.rows
        .iter()
        .map(|r| {
            r[lead..]
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => Some(*n),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

#[test]
fn survival_life_tables_and_medians() {
    let (tables, ..) =
        run("SURVIVAL TABLES=weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 /STATUS=outcome(1).");
    let t = table(&tables, "Life Table of weeks");
    let rows = lines(t, 1);
    // plan 1 has five lines, plan 2 seven, plan 3 seven
    assert_eq!(rows.len(), 19);
    assert_eq!(text(&t.rows[0][0]), "plan = 1");
    // start, entering, withdrawn, exposed, events, q, p, P, se(P), f, se(f), hazard, se(hazard)
    let want_plan1_second = [
        5.0,
        11.0,
        2.0,
        10.0,
        3.0,
        0.3,
        0.7,
        0.641666666667,
        0.144100819902,
        0.055,
        0.0269953699734,
        0.0705882352941,
        0.0401145377057,
    ];
    for (got, want) in rows[1].iter().zip(want_plan1_second) {
        assert!((got.unwrap() - want).abs() < 1e-11, "{got:?} {want}");
    }
    let plan1_last = &rows[4];
    close(plan1_last[7].unwrap(), 0.101851851852);
    close(plan1_last[8].unwrap(), 0.128081904257);
    close(plan1_last[10].unwrap(), 0.0310642546282);
    close(plan1_last[12].unwrap(), 0.173205080757);
    // plan 2's open interval from 30: one withdrawal, no density or hazard
    let open = &rows[11];
    assert_eq!(open[0], Some(30.0));
    close(open[7].unwrap(), 0.233508403361);
    close(open[8].unwrap(), 0.155630146094);
    assert_eq!(open[9..], [None, None, None, None]);
    // plan 3 ends with every remaining case having the event
    close(rows[18][7].unwrap(), 0.0);
    close(rows[17][10].unwrap(), 0.0240433405986);

    let medians = table(&tables, "Median Survival Time");
    for (row, want) in medians
        .rows
        .iter()
        .zip([13.3116883117, 20.7287449393, 23.2592592593])
    {
        close(num(&row[3]), want);
    }
}

#[test]
fn survival_stretched_intervals_and_the_open_median() {
    let (tables, _, warnings, _) =
        run("SURVIVAL weeks /INTERVAL=THRU 20 BY 6 THRU 30 BY 4 /STATUS=outcome(1) FOR weeks.");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("0, 6, 12, 18, 24, 28, 32"))
    );
    let rows = lines(table(&tables, "Life Table of weeks"), 0);
    let starts: Vec<f64> = rows.iter().map(|r| r[0].unwrap()).collect();
    assert_eq!(starts, [0.0, 6.0, 12.0, 18.0, 24.0, 28.0, 32.0]);
    close(rows[2][7].unwrap(), 0.506018567546);
    close(rows[4][10].unwrap(), 0.01667593419023);
    close(rows[5][12].unwrap(), 0.0706959513293);
    close(
        num(&table(&tables, "Median Survival Time").rows[0][3]),
        18.2973491454,
    );
    // survival stays above one half up to the open interval
    let (tables, ..) = run("SURVIVAL weeks /INTERVAL=THRU 10 BY 5 /STATUS=outcome(1).");
    let median = &table(&tables, "Median Survival Time").rows[0][3];
    assert_eq!(text(median), "10+");
}

#[test]
fn survival_gehan_comparisons() {
    let (tables, ..) = run("SURVIVAL weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 \
         /STATUS=outcome(1) /PRINT=NOTABLE /COMPARE /CALCULATE=PAIRWISE.");
    assert!(tables.iter().all(|t| !t.title.starts_with("Life Table")));
    let overall = statistics(table(&tables, "Overall Comparisons"));
    close(overall[0].0, 1.80144121005);
    assert_eq!(overall[0].1, 2.0);
    close(overall[0].2, 0.406276789136);
    let pairs = statistics(table(&tables, "Pairwise Comparisons"));
    for (got, want) in pairs
        .iter()
        .zip([0.945960485133, 1.55924882629, 0.133746635909])
    {
        close(got.0, want);
    }
    // within each region, as a second order control
    let (tables, ..) = run(
        "SURVIVAL weeks BY plan(1,3) BY region(1,2) /INTERVAL=THRU 30 BY 5 \
         /STATUS=outcome(1) /COMPARE /CALCULATE=COMPARE.",
    );
    assert_eq!(tables.len(), 1);
    let within = statistics(&tables[0]);
    close(within[0].0, 1.84622902753);
    close(within[1].0, 0.193179317932);
    // approximate: each time scored by the start of its interval
    let (tables, ..) = run("SURVIVAL weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 \
         /STATUS=outcome(1) /COMPARE /CALCULATE=APPROXIMATE.");
    close(
        statistics(table(&tables, "Overall Comparisons"))[0].0,
        1.36730602514,
    );
}

#[test]
fn survival_plots_missing_values_and_listwise() {
    let (_, charts, ..) = run("SURVIVAL weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 \
         /STATUS=outcome(1) /PLOTS(SURVIVAL HAZARD)=weeks BY plan.");
    assert_eq!(charts.len(), 2);
    let Some(Plot::Points { groups, .. }) = &charts[1].plot else {
        panic!()
    };
    // plan 1's hazard: a level across each closed interval it has
    assert_eq!(groups[0].points.len(), 10);
    close(groups[0].points[2].y, 0.0705882352941);

    // a missing time leaves that case out of its variable's tables only (GROUPWISE); LISTWISE
    // leaves it out of every table
    let source = "COMPUTE days = weeks * 7.
IF ($CASENUM = 1) weeks = $SYSMIS.
SURVIVAL weeks days /INTERVAL=THRU 300 BY 50 /STATUS=outcome(1)";
    let groupwise = run(&format!("{source}.")).0;
    let listwise = run(&format!("{source} /MISSING=LISTWISE.")).0;
    let entering = |t: &[Table], title: &str| num(&table(t, title).rows[0][1]);
    assert_eq!(entering(&groupwise, "Life Table of days"), 36.0);
    assert_eq!(entering(&groupwise, "Life Table of weeks"), 35.0);
    assert_eq!(entering(&listwise, "Life Table of days"), 35.0);
}
