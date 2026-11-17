//! ROC ANALYSIS on a small made-up data set. Every expected number was computed separately
//! by brute force from the definitions in a short script: the areas over every pair of a
//! positive and a negative case, the nonparametric errors over every triple, the DeLong
//! variances and covariances from double sums over the pairs, and the normal tests and
//! intervals from them.

use statscraft_engine::chart::ChartKind;
use statscraft_engine::plot::Plot;
use statscraft_engine::{Cell, Chart, Engine, Table};
use statscraft_syntax::parse;

/// Two made-up screening scores (t2 between 0 and 1), the true state, a clinic group and a
/// case weight. Case 12 has a weight below one half, case 14 a missing t1.
const SCREEN: &str = "DATA LIST LIST /sick t1 t2 grp w.
BEGIN DATA
1 7.2 0.90 1 1
1 5.8 0.60 2 2
1 6.4 0.80 1 1
1 4.1 0.70 2 1
1 5.8 0.40 1 1.4
1 8.0 0.95 2 1
0 3.3 0.20 1 1
0 5.8 0.50 2 1
0 6.6 0.60 1 1.6
0 2.9 0.75 2 1
0 4.1 0.30 1 1
0 6.1 0.45 2 0.4
0 3.8 0.35 1 1
1 . 0.85 2 1
END DATA.
";

fn run(source: &str) -> Result<(Vec<Table>, Vec<Chart>, Vec<String>), String> {
    let (commands, errors) = parse(&format!("{SCREEN}{source}\n"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let (mut tables, mut charts) = (Vec::new(), Vec::new());
    for command in &commands {
        tables = engine.execute(&command.command).map_err(|e| e.0)?;
        charts = engine.take_charts();
    }
    Ok((tables, charts, engine.warnings().to_vec()))
}

fn ok(source: &str) -> (Vec<Table>, Vec<Chart>, Vec<String>) {
    run(source).unwrap_or_else(|e| panic!("{source}: {e}"))
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

/// The numbers of a row from column `from` on, against `want` at 1e-9 relative.
fn row_close(row: &[Cell], from: usize, want: &[f64]) {
    for (cell, w) in row[from..].iter().zip(want) {
        let got = num(cell);
        assert!(
            (got - w).abs() <= 1e-9 * w.abs().max(1e-3),
            "{got} against {w} in {row:?}"
        );
    }
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no {title}"))
}

#[test]
fn paired_design_delong_errors_and_comparison() {
    let (tables, charts, _) = ok("ROC ANALYSIS t1 t2 BY sick (1) /DESIGN PAIR=TRUE
        /PRINT SE=TRUE COORDINATES=ROC PR CLASSIFIER=TRUE
        /PLOT CURVE=ROC(REFERENCE) PR MODELQUALITY=TRUE.");
    let counts: Vec<f64> = tables[0].rows.iter().map(|r| num(&r[1])).collect();
    assert_eq!(counts, [6.0, 7.0, 1.0, 14.0]);
    let areas = titled(&tables, "Area Under the ROC Curve");
    // area, DeLong error, significance with that error, 95% interval
    row_close(
        &areas.rows[0],
        1,
        &[
            0.7738095238095238,
            0.13454663382717996,
            0.04184554564061344,
            0.5101029672671527,
            1.037516080351895,
        ],
    );
    row_close(
        &areas.rows[1],
        1,
        &[
            0.8452380952380952,
            0.11435183272589129,
            0.002535379787955688,
            0.6211126215291997,
            1.069363568946991,
        ],
    );
    let caption = areas.caption.as_deref().unwrap();
    assert!(
        caption.contains("t1") && caption.contains("DeLong"),
        "{caption}"
    );
    let pairs = titled(
        &tables,
        "Paired-Sample Area Difference Under the ROC Curves",
    );
    assert_eq!(pairs.rows[0][0], Cell::Text("t1 - t2".into()));
    row_close(
        &pairs.rows[0],
        1,
        &[
            -0.5205269629105767,
            0.6026963401705403,
            -0.0714285714285714,
            0.13722357633343651,
            -0.34038183887188983,
            0.19752469601474704,
        ],
    );
    // Gini index, Max K-S and its cutoff
    let metrics = titled(&tables, "Classifier Evaluation Metrics");
    row_close(&metrics.rows[0], 1, &[0.5476190476190477, 3.0 / 7.0, 3.95]);
    row_close(
        &metrics.rows[1],
        1,
        &[0.6904761904761905, 23.0 / 42.0, 0.55],
    );
    // eleven cutoffs of t1 from 1.9 to 9, then thirteen of t2 from 0 to 1 (all inside (0, 1))
    let roc = titled(&tables, "Coordinates of the ROC Curve");
    assert_eq!(roc.rows.len(), 24);
    row_close(&roc.rows[3], 1, &[3.95, 1.0, 4.0 / 7.0]);
    row_close(&roc.rows[11], 1, &[0.0, 1.0, 1.0]);
    row_close(&roc.rows[23], 1, &[1.0, 0.0, 0.0]);
    let pr = titled(&tables, "Coordinates of the Precision-Recall Curve");
    row_close(&pr.rows[0], 1, &[1.9, 6.0 / 13.0, 1.0]);
    row_close(&pr.rows[5], 1, &[5.95, 0.6, 0.5]);
    assert_eq!(pr.rows[10][2], Cell::Empty);
    assert!(pr.caption.as_deref().unwrap().contains("between 0 and 1"));

    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    assert_eq!(kinds, [ChartKind::Bar, ChartKind::Roc, ChartKind::Line]);
    let Some(Plot::Points { groups, lines, .. }) = &charts[1].plot else {
        panic!("{:?}", charts[1].plot)
    };
    assert_eq!((groups.len(), lines.len()), (2, 1));
    let curve = &groups[0].points;
    assert_eq!((curve[0].x, curve[0].y), (0.0, 0.0));
    assert_eq!((curve[10].x, curve[10].y), (1.0, 1.0));
    // the precision-recall curve starts at the first true positive's precision
    let Some(Plot::Points { groups, .. }) = &charts[2].plot else {
        panic!()
    };
    let first = &groups[0].points[0];
    assert_eq!((first.x, first.y), (0.0, 1.0));
}

#[test]
fn independent_groups_by_level_midpoint_and_direction() {
    let (tables, charts, _) =
        ok("ROC ANALYSIS t1 t2 BY sick (1) /DESIGN GROUP=grp SELECT=LEVEL(1 2) /PRINT SE=TRUE.");
    let summary = &tables[0];
    let counts: Vec<f64> = summary.rows.iter().map(|r| num(&r[2])).collect();
    assert_eq!(counts, [3.0, 4.0, 3.0, 3.0, 13.0, 1.0, 14.0]);
    assert_eq!(summary.rows[0][0], Cell::Text("= 1".into()));
    let areas = titled(&tables, "Area Under the ROC Curve");
    // area, nonparametric error, test of 0.5 with the null error, interval
    row_close(
        &areas.rows[0],
        2,
        &[
            0.8333333333333334,
            0.17010345435994284,
            0.15729920705028513,
            0.49993668914199263,
            1.1667299775246742,
        ],
    );
    row_close(
        &areas.rows[1],
        2,
        &[
            0.6111111111111112,
            0.2518790834951008,
            0.6625205835400574,
            0.11743717900175654,
            1.1047850432204658,
        ],
    );
    row_close(
        &areas.rows[3],
        2,
        &[0.7777777777777778, 0.21596118129056668, 0.27523352407483437],
    );
    let diff = titled(
        &tables,
        "Independent-Group Area Difference Under the ROC Curves",
    );
    row_close(
        &diff.rows[0],
        1,
        &[
            0.7311434644864528,
            0.4646915282101335,
            0.2222222222222222,
            0.3039379178179602,
            -0.3734851502370744,
            0.8179295946815188,
        ],
    );
    row_close(
        &diff.rows[1],
        1,
        &[
            0.5672506752453633,
            0.570543883362581,
            0.13888888888888884,
            0.24484570041069179,
        ],
    );
    // one ROC chart per test variable, the two groups in each
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[1].subtitle.as_deref(), Some("t2"));
    assert_eq!(charts[0].legend.as_deref(), Some("Grouped by grp"));

    // MIDPOINT 1.5: the cases at or above it (group 2) come first
    let (tables, ..) = ok("ROC ANALYSIS t1 BY sick (1) /DESIGN GROUP=grp SELECT=MIDPOINT.");
    let areas = titled(&tables, "Area Under the ROC Curve");
    assert_eq!(areas.rows[0][1], Cell::Text("≥ 1.5".into()));
    row_close(&areas.rows[0], 2, &[0.6111111111111112]);
    row_close(&areas.rows[1], 2, &[0.8333333333333334]);

    // small values positive: areas turn around, errors stay, PAIR is ignored with SELECT
    let (tables, ..) = ok("ROC ANALYSIS t1 BY sick (1) /CRITERIA TESTPOS=SMALL
        /DESIGN PAIR=TRUE GROUP=grp SELECT=LEVEL(1 2) /PRINT SE=TRUE /PLOT CURVE=NONE.");
    let areas = titled(&tables, "Area Under the ROC Curve");
    row_close(
        &areas.rows[0],
        2,
        &[
            0.16666666666666663,
            0.17010345435994284,
            0.15729920705028513,
            -0.1667299775246741,
        ],
    );
    let diff = titled(
        &tables,
        "Independent-Group Area Difference Under the ROC Curves",
    );
    row_close(&diff.rows[0], 1, &[-0.7311434644864528]);
}

#[test]
fn rounded_weights_count_cases_and_negexpo() {
    // case 12 (weight 0.4) is missing; weights 2, 1.4 and 1.6 count as 2, 1 and 2 cases,
    // which makes seven positive and seven negative cases, so NEGEXPO applies. The expected
    // numbers repeat each case as often as its rounded weight says.
    let (tables, _, warnings) = ok("WEIGHT BY w.
        ROC ANALYSIS t1 t2 BY sick (1) /CRITERIA DISTRIBUTION=NEGEXPO /PRINT SE=TRUE.");
    let counts: Vec<f64> = tables[0].rows.iter().map(|r| num(&r[1])).collect();
    assert_eq!(counts, [7.0, 7.0, 2.0, 16.0]);
    assert!(tables[0].caption.as_deref().unwrap().contains("rounded"));
    let areas = titled(&tables, "Area Under the ROC Curve");
    row_close(
        &areas.rows[0],
        1,
        &[0.7346938775510204, 0.1381704224592883, 0.14171996321058997],
    );
    row_close(
        &areas.rows[1],
        1,
        &[0.8163265306122449, 0.11899755150260426, 0.04764542558436182],
    );
    assert!(areas.caption.as_deref().unwrap().contains("bi-negative"));
    assert!(warnings.is_empty(), "{warnings:?}");
    // weighted paired design: DeLong errors and the comparison
    let (tables, ..) = ok("WEIGHT BY w.
        ROC ANALYSIS t1 t2 BY sick (1) /DESIGN PAIR=TRUE /PRINT SE=TRUE.");
    row_close(
        &titled(&tables, "Area Under the ROC Curve").rows[0],
        1,
        &[0.7346938775510204, 0.14490754016422308, 0.1053158184157523],
    );
    let pairs = titled(
        &tables,
        "Paired-Sample Area Difference Under the ROC Curves",
    );
    row_close(&pairs.rows[0], 1, &[-0.5524911066193569]);
    row_close(
        &pairs.rows[0],
        3,
        &[-0.08163265306122447, 0.14775378659165628],
    );
    // without weights the groups are unequal and NEGEXPO falls back
    let (tables, ..) =
        ok("ROC ANALYSIS t1 BY sick (1) /CRITERIA DISTRIBUTION=NEGEXPO /PRINT SE=TRUE.");
    let caption = titled(&tables, "Area Under the ROC Curve").caption.clone();
    assert!(caption.unwrap().contains("no distribution"));
    // NEGEXPO in the paired design: a warning and the DeLong errors; a repeated subcommand
    // warns too
    let (tables, _, warnings) = ok("ROC ANALYSIS t1 t2 BY sick (1) /DESIGN PAIR=TRUE
        /CRITERIA DISTRIBUTION=NEGEXPO /PRINT SE=TRUE /PRINT SE=TRUE.");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    row_close(
        &titled(&tables, "Area Under the ROC Curve").rows[0],
        2,
        &[0.13454663382717996],
    );
}

#[test]
fn design_errors() {
    let err = |source: &str| run(source).unwrap_err();
    assert!(err("ROC ANALYSIS t1 BY sick (1) /DESIGN PAIR=TRUE.").contains("at least two"));
    assert!(err("ROC ANALYSIS t1 BY sick (1) /DESIGN GROUP=grp.").contains("SELECT"));
    assert!(err("ROC ANALYSIS t1 BY sick (1) /DESIGN SELECT=MIDPOINT.").contains("GROUP"));
    assert!(err("ROC ANALYSIS t1 BY sick (1) /DESIGN GROUP=grp SELECT=LEVEL(1 3).").contains("3"));
    assert!(err("ROC ANALYSIS t1 BY sick ('x').").contains("type"));
    // a group without negative cases
    assert!(
        err("ROC ANALYSIS t1 BY sick (1) /DESIGN GROUP=t2 SELECT=CUTPOINT(0.85).")
            .contains("both states")
    );
}
