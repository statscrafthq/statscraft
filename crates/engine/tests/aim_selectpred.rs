//! AIM and SELECTPRED on invented data. The expected numbers come from R run as a black box
//! on the same data: t.test(x[g == i], mu = overall mean) and chisq.test(counts, p = overall
//! shares) for AIM, qt and qchisq for the critical values; for SELECTPRED the data expanded
//! by the rounded weights, then chisq.test(correct = FALSE), oneway.test(var.equal = TRUE)
//! and cor.test, with the likelihood ratio chi-square, Cramer's V and lambda worked out from
//! the same table.

use statscraft_engine::chart::ChartKind;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Groups g 1 to 3, scores x and y, category c; one case lacks x, one has c user-missing (9).
const AIM_DATA: &str = "DATA LIST FREE /g x y c.
BEGIN DATA.
1 2.1 10 1
1 3.4 12 1
1 2.8 11 2
1 3.9 14 1
1 2.5 13 1
2 5.6 12 2
2 6.1 15 2
2 4.9 11 3
2 5.4 16 2
2 6.6 10 2
3 3.3 20 3
3 4.2 18 3
3 3.8 22 1
3 4.6 19 3
3 3.1 21 3
2 . 13 1
3 4.0 17 9
END DATA.
MISSING VALUES c (9).
";

/// Target t (three classes) and scale target ys; factors a, b, sparse, idc, k and
/// covariates u, v; frequency weight w. Case 16 weighs 0.3 (left out), case 17 weighs 1.6
/// (counts 2), case 18 lacks the targets, case 19 every predictor but t, case 20 lacks v.
const SELECT_DATA: &str = "DATA LIST FREE /t a b u v w ys sparse idc k.
BEGIN DATA.
1 1 1 2.3 5.1 1 10.2 1 101 5
1 1 2 3.1 4.8 1 11.5 . 102 5
1 2 1 2.8 6.0 2 9.8 . 103 5
1 1 1 1.9 5.5 1 12.1 . 104 5
1 3 2 2.6 4.9 1 10.9 2 105 5
2 2 2 4.4 5.2 1 14.3 . 106 5
2 2 1 3.9 6.1 1 15.0 . 107 5
2 3 2 4.8 4.7 2 13.2 . 108 5
2 2 2 5.2 5.8 1 16.4 1 109 5
2 1 1 4.1 5.0 1 14.8 . 110 5
3 3 2 6.3 5.4 1 18.7 . 111 5
3 3 1 5.9 6.2 1 19.9 2 112 5
3 3 2 6.8 4.6 1 17.5 . 113 5
3 2 2 7.1 5.9 2 20.3 . 114 5
3 3 1 6.0 5.3 1 18.1 . 115 5
1 1 2 2.0 5.6 0.3 9.9 . 116 5
2 3 1 4.6 5.1 1.6 15.6 1 117 5
. 2 1 3.0 5.0 1 . . 118 5
3 . . . . 1 19.0 . . .
1 1 1 2.4 . 1 11.0 . 120 5
END DATA.
VARIABLE LEVEL t a b sparse idc k (NOMINAL) /u v w ys (SCALE).
WEIGHT BY w.
";

fn run(data: &str, syntax: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables = more,
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

fn fails(data: &str, syntax: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("{syntax} ran without an error");
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn label(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-8 * b.abs().max(1.0)
}

fn assert_close(got: f64, want: f64) {
    assert!(close(got, want), "{got} != {want}");
}

/// The rows of a table as (label of column `key`, numbers of `columns`).
fn rows(table: &Table, columns: &[usize]) -> Vec<Vec<f64>> {
    table
        .rows
        .iter()
        .map(|r| columns.iter().map(|&c| number(&r[c])).collect())
        .collect()
}

#[test]
fn aim_tests_each_group_against_all_cases() {
    let (_, tables) = run(AIM_DATA, "AIM g /CATEGORICAL c /CONTINUOUS x y.");
    let means = find(&tables, "Group Means against the Overall Mean");
    // per variable: three groups and the all-cases row; t, df, Sig.
    let want_t = [
        (-3.7879692824, 0.0193026365388, "No"),
        (5.36102296816, 0.00584231544907, "Yes"),
        (-1.27332494728, 0.271868471739, "No"),
        (-4.14835978296, 0.0142792715546, "Yes"),
        (-1.84291930791, 0.139126660569, "No"),
        (7.16534871602, 0.00200823048512, "Yes"),
    ];
    let groups: Vec<&Vec<Cell>> = means
        .rows
        .iter()
        .filter(|r| label(&r[1]) != "All cases")
        .collect();
    assert_eq!(groups.len(), 6);
    for (row, (t, p, pass)) in groups.iter().zip(want_t) {
        assert_close(number(&row[5]), t);
        assert_eq!(number(&row[6]), 4.0);
        assert_close(number(&row[7]), p);
        assert_eq!(label(&row[8]), pass);
    }
    assert_close(number(&groups[1][3]), 5.72);
    // 15 cases: the one without x and the user-missing c are left out
    assert_eq!(number(&means.rows[3][2]), 15.0);
    assert!(means.caption.as_deref().unwrap().contains("0.0167"));

    let shares = find(&tables, "Group Category Shares against the Overall Shares");
    for row in &shares.rows {
        assert_close(number(&row[3]), 5.2);
        assert_eq!(number(&row[4]), 2.0);
        assert_close(number(&row[5]), 0.0742735782143);
        assert_eq!(label(&row[6]), "No");
    }
}

#[test]
fn aim_include_and_no_adjustment() {
    let (_, tables) = run(
        AIM_DATA,
        "AIM g /CATEGORICAL c /CONTINUOUS x y /MISSING INCLUDE /CRITERIA ADJUST=NONE.",
    );
    let means = find(&tables, "Group Means against the Overall Mean");
    let groups = Table {
        rows: means
            .rows
            .iter()
            .filter(|r| label(&r[1]) != "All cases")
            .cloned()
            .collect(),
        ..means.clone()
    };
    let t = rows(&groups, &[5, 6, 7]);
    assert_close(t[2][0], -1.35548678049);
    assert_eq!(t[2][1], 5.0);
    assert_close(t[2][2], 0.233274437484);
    // without the adjustment 0.0198 passes 0.05
    assert_eq!(label(&means.rows[0][8]), "Yes");
    let shares = find(&tables, "Group Category Shares against the Overall Shares");
    let x = rows(shares, &[3, 4, 5]);
    assert_close(x[0][0], 5.88);
    assert_close(x[2][0], 5.73333333333);
    assert_eq!(x[2][1], 3.0);
    assert_close(x[2][2], 0.125329989843);
}

#[test]
fn aim_charts() {
    let (mut engine, _) = run(
        AIM_DATA,
        "AIM g /CATEGORICAL c /CONTINUOUS x y /CRITERIA HIDENOTSIG=YES \
         /PLOT CATEGORY CLUSTER(TYPE=PIE) ERRORBAR IMPORTANCE.",
    );
    let charts = engine.take_charts();
    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            ChartKind::Pie,
            ChartKind::Bar,
            ChartKind::ErrorBar,
            ChartKind::ErrorBar,
            ChartKind::Bar,
            ChartKind::Bar,
        ]
    );
    assert_eq!(charts[0].series[0].values, [5.0, 5.0, 5.0]);
    // x: only group 2 passes; its interval uses t(1 - 0.05 / 6, 4)
    let bars = &charts[2];
    assert_eq!(bars.categories.len(), 1);
    assert_close(bars.series[0].values[0], 5.72);
    assert_close(
        bars.error_bars.as_ref().unwrap().margins[0][0],
        1.15747166039,
    );
    // importance of x by group: t with both critical values; c passes nowhere: no chart
    let x = &charts[4];
    assert_close(x.series[0].values[0], 5.36102296816);
    assert_close(x.series[1].values[0], 3.96078648277);
    assert_close(x.series[2].values[0], -3.96078648277);
    let y = &charts[5];
    assert_eq!(y.categories.len(), 2);

    let (mut engine, _) = run(
        AIM_DATA,
        "AIM g /CATEGORICAL c /CONTINUOUS x /PLOT IMPORTANCE(X=VARIABLE Y=TEST).",
    );
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 3);
    let names: Vec<&str> = charts[0].series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Student's t",
            "Chi-square",
            "Critical value",
            "Negative critical value"
        ]
    );
    // the chi-square critical value at 0.05 / 3 with 2 degrees of freedom
    assert_close(charts[0].series[2].values[1], 8.18868912444);
    assert_close(charts[0].series[1].values[1], 5.2);

    let (mut engine, _) = run(
        AIM_DATA,
        "AIM g /CONTINUOUS x /CRITERIA ADJUST=NONE SHOWREFLINE=NO /PLOT IMPORTANCE(Y=PVALUE).",
    );
    let charts = engine.take_charts();
    assert_eq!(charts[0].series.len(), 1);
    // x alone keeps the case whose c is user-missing: 16 cases
    assert_close(charts[0].series[0].values[1], -0.00571570332749f64.log10());
}

#[test]
fn aim_errors() {
    assert!(fails(AIM_DATA, "AIM g /CONTINUOUS g x.").contains("grouping variable"));
    let mut data = AIM_DATA.to_string();
    data.push_str("STRING s (A2).\nCOMPUTE s = 'a'.\n");
    assert!(fails(&data, "AIM g /CONTINUOUS s.").contains("string"));
}

#[test]
fn selectpred_categorical_target() {
    let (mut engine, tables) = run(
        SELECT_DATA,
        "SELECTPRED t BY a b sparse idc k WITH u v /CRITERIA SHOWUNSELECTED=10 /PLOT SUMMARY.",
    );
    let cps = find(&tables, "Case Processing Summary");
    // included cases and their weight; target missing; every predictor missing; weight
    assert_eq!(
        (number(&cps.rows[0][1]), number(&cps.rows[0][2])),
        (17.0, 21.0)
    );
    assert_eq!(number(&cps.rows[1][1]), 1.0);
    assert_eq!(number(&cps.rows[2][1]), 1.0);
    assert_eq!(number(&cps.rows[3][1]), 1.0);
    assert_eq!(number(&cps.rows[5][1]), 20.0);

    let excluded = find(&tables, "Excluded Predictors");
    let reasons: Vec<(String, String)> = excluded
        .rows
        .iter()
        .map(|r| (label(&r[0]), label(&r[2])))
        .collect();
    assert_eq!(reasons[0].0, "sparse");
    assert!(reasons[0].1.contains("Missing values over 70%"));
    assert_eq!(reasons[1], ("k".to_string(), "Constant".to_string()));

    // mixed predictors: Pearson chi-square for the factors, F for the covariates
    let ranking = find(&tables, "Predictor Ranking");
    let order: Vec<String> = ranking.rows.iter().map(|r| label(&r[1])).collect();
    assert_eq!(order, ["u", "idc", "a", "b", "v"]);
    let want = [
        (
            21.0,
            128.344297082,
            2.0,
            Some(18.0),
            2.22785492034e-11,
            "Yes",
        ),
        (21.0, 42.0, 32.0, None, 0.111074510135, "No"),
        (21.0, 7.43650793651, 4.0, None, 0.114541834137, "No"),
        (21.0, 1.90909090909, 2.0, None, 0.384987098923, "No"),
        (20.0, 0.694925967616, 2.0, Some(17.0), 0.512744316313, "No"),
    ];
    for (row, (n, stat, df1, df2, p, selected)) in ranking.rows.iter().zip(want) {
        assert_eq!(number(&row[3]), n);
        assert_close(number(&row[5]), stat);
        assert_eq!(number(&row[6]), df1);
        assert_eq!(df2.map(|_| number(&row[7])), df2);
        assert!((number(&row[8]) - p).abs() < 1e-9 * p.max(1e-3), "{row:?}");
        assert_eq!(label(&row[9]), selected);
    }
    let selected = find(&tables, "Selected Predictors");
    assert_eq!(label(&selected.rows[0][1]), "");
    assert_eq!(label(&selected.rows[1][1]), "u");
    // the chart: the five ranked predictors, 1 - Sig. for the selected one only
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].categories.len(), 5);
    assert_close(charts[0].series[0].values[0], 1.0 - 2.22785492034e-11);
    assert!(charts[0].series[0].values[1].is_nan());
    assert_close(charts[0].series[1].values[4], 1.0 - 0.512744316313);
}

#[test]
fn selectpred_rankings_of_categorical_predictors() {
    let ranked = |criteria: &str| -> Vec<(String, f64, String)> {
        let (_, tables) = run(
            SELECT_DATA,
            &format!(
                "SELECTPRED t BY a b idc /CRITERIA {criteria} /PRINT SUMMARY(LAMBDA LRCHISQ)."
            ),
        );
        let table = find(&tables, "Predictor Ranking");
        let last = table.columns.len() - 1;
        table
            .rows
            .iter()
            .map(|r| (label(&r[1]), number(&r[5]), label(&r[last])))
            .collect()
    };
    let lambda = ranked("RANKING=LAMBDA");
    let names: Vec<&str> = lambda.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(names, ["idc", "a", "b"]);
    assert_close(lambda[1].1, 0.230769230769);
    assert_close(lambda[2].1, 0.0769230769231);
    // no significance ranks: all three are selected
    assert!(lambda.iter().all(|r| r.2 == "Yes"));

    let v = ranked("RANKING=CRAMERSV");
    assert_close(v[0].1, 1.0);
    assert_close(v[1].1, 0.420784639423);
    assert_close(v[2].1, 0.301511344578);

    let lr = ranked("RANKING=LRCHISQ PCUTOFF=0.06 SHOWUNSELECTED=5");
    let names: Vec<&str> = lr.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(names, ["idc", "a", "b"]);
    assert_close(lr[0].1, 45.855022);
    assert_close(lr[1].1, 8.37577424019);
    assert_eq!(
        lr.iter().map(|r| r.2.as_str()).collect::<Vec<_>>(),
        ["Yes", "No", "No"]
    );

    // the extra columns: lambda then likelihood ratio, before Selected
    let (_, tables) = run(
        SELECT_DATA,
        "SELECTPRED t BY a b idc /PRINT SUMMARY(LAMBDA LRCHISQ) /CRITERIA SIZE=2 PCUTOFF=1.",
    );
    let table = find(&tables, "Predictor Ranking");
    assert_eq!(table.columns[9], "Lambda");
    assert_eq!(table.columns[10], "Likelihood Ratio");
    let a = table.rows.iter().find(|r| label(&r[1]) == "a").unwrap();
    assert_close(number(&a[9]), 0.230769230769);
    assert_close(number(&a[10]), 8.37577424019);
    // SIZE=2 with no cutoff: idc and a
    let chosen: Vec<String> = table
        .rows
        .iter()
        .filter(|r| label(&r[11]) == "Yes")
        .map(|r| label(&r[1]))
        .collect();
    assert_eq!(chosen, ["idc", "a"]);
}

#[test]
fn selectpred_screening_settings() {
    let (_, tables) = run(
        SELECT_DATA,
        "SELECTPRED t BY a b idc WITH u /SCREENING PCTUNEQUAL=70 PCTEQUAL=40 CV=0.5 /PRINT EXCLUDED.",
    );
    assert_eq!(tables.len(), 1);
    let reasons: Vec<(String, String)> = tables[0]
        .rows
        .iter()
        .map(|r| (label(&r[0]), label(&r[2])))
        .collect();
    // a's largest category holds 9 of 21 (42.9%); b 11 of 21; idc 13 of 17 categories have
    // one case; u has sd / mean about 0.39
    assert_eq!(reasons.len(), 4);
    assert!(reasons[0].1.contains("over 40%"), "{reasons:?}");
    assert!(reasons[1].1.contains("over 40%"));
    assert!(reasons[2].1.contains("categories hold one case"));
    assert!(reasons[3].1.contains("Coefficient of variation"));
    let (_, tables) = run(
        SELECT_DATA,
        "SELECTPRED t BY a b idc WITH u /SCREENING STATUS=OFF PCTUNEQUAL=10 /PRINT EXCLUDED.",
    );
    assert!(tables.is_empty());
}

#[test]
fn selectpred_scale_target_from_the_dictionary() {
    let (engine, tables) = run(SELECT_DATA, "SELECTPRED ys /EXCEPT VARIABLES=sparse idc.");
    let ranking = find(&tables, "Predictor Ranking");
    let order: Vec<String> = ranking.rows.iter().map(|r| label(&r[1])).collect();
    assert_eq!(order, ["t", "u"]);
    let want = [
        (22.0, 110.313374374, 3.48797933936e-11),
        (21.0, 0.928507414551, 1.29090865742e-09),
    ];
    for (row, (n, stat, p)) in ranking.rows.iter().zip(want) {
        assert_eq!(number(&row[3]), n);
        assert_close(number(&row[5]), stat);
        assert!((number(&row[8]) - p).abs() < 1e-9 * p, "{row:?}");
    }
    assert_eq!(label(&ranking.rows[1][4]), "Pearson Correlation");
    let selected = find(&tables, "Selected Predictors");
    assert_eq!(label(&selected.rows[0][1]), "t");
    assert_eq!(label(&selected.rows[1][1]), "u");
    let _ = engine;

    let (_, tables) = run(
        SELECT_DATA,
        "SELECTPRED ys BY a b WITH v /CRITERIA PCUTOFF=1 SHOWUNSELECTED=5 /PRINT SUMMARY.",
    );
    let ranking = &tables[0];
    let got: Vec<(String, f64, f64)> = ranking
        .rows
        .iter()
        .map(|r| (label(&r[1]), number(&r[5]), number(&r[8])))
        .collect();
    assert_eq!(got[0].0, "a");
    assert_close(got[0].1, 2.38611154418);
    assert_close(got[0].2, 0.120449133021);
    assert_close(got[1].1, 1.43194598427);
    assert_close(got[1].2, 0.246166015535);
    assert_close(got[2].1, 0.252685788721);
    assert_close(got[2].2, 0.282443289758);
}

#[test]
fn selectpred_messages() {
    assert!(fails(SELECT_DATA, "SELECTPRED t BY a WITH a.").contains("both"));
    assert!(fails(SELECT_DATA, "SELECTPRED w.").contains("weight"));
    assert!(fails(SELECT_DATA, "SELECTPRED k BY a.").contains("two categories"));
    let (engine, _) = run(
        SELECT_DATA,
        "SELECTPRED t BY a WITH u /CRITERIA RANKING=LAMBDA /PRINT SUMMARY(CRAMERSV).",
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("RANKING=LAMBDA"))
    );
    let (engine, _) = run(SELECT_DATA, "SELECTPRED ys BY a /PRINT SUMMARY(LAMBDA).");
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("SUMMARY(LAMBDA)"))
    );
}
