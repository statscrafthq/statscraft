//! Statistical plots as data: their exact numbers on small made-up data sets, checked against
//! hand computations and an independent computation (scipy) where a formula is given.

use statscraft_engine::chart::{ChartKind, Tier};
use statscraft_engine::plot::{BoxSummary, Plot, Point};
use statscraft_engine::{Cell, Chart, Engine, Table};
use statscraft_syntax::parse;

/// Minutes a courier waited at 15 stops, by depot; stop 3 and stop 8 waited long.
const STOPS: &str = "DATA LIST LIST /wait depot (F2) stop (A1).
BEGIN DATA
5 1 a
12 2 b
30 1 c
2 1 d
13 2 e
4 1 f
18 2 g
13 1 h
5 1 i
10 2 j
6 1 k
15 2 l
7 1 m
19 2 n
8 1 o
END DATA.
VALUE LABELS depot 1 'Harbour' 2 'Airport'.
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

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-9, "{got} against {want}");
}

fn boxes(chart: &Chart) -> &[BoxSummary] {
    match &chart.plot {
        Some(Plot::Boxes { boxes }) => boxes,
        other => panic!("{other:?}"),
    }
}

fn points(chart: &Chart) -> &[Point] {
    match &chart.plot {
        Some(Plot::Points { groups, .. }) => &groups[0].points,
        other => panic!("{other:?}"),
    }
}

fn marks(marks: &[statscraft_engine::plot::Mark]) -> Vec<(f64, &str)> {
    marks.iter().map(|m| (m.value, m.case.as_str())).collect()
}

#[test]
fn boxplots_use_tukeys_hinges_and_mark_far_cases() {
    let (_, charts, warnings) = run(STOPS, "EXAMINE wait BY depot /PLOT BOXPLOT.");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 2);
    let total = &charts[0];
    assert_eq!(
        (total.kind, total.tier),
        (ChartKind::Boxplot, Tier::Statistical)
    );
    assert_eq!(total.title, "Boxplot of wait");
    // 15 cases: hinges at depth 4.5 (5.5 and 14), median 10; a box of 8.5, so 30 lies
    // between 1.5 and 3 box lengths above
    let b = &boxes(total)[0];
    assert_eq!(b.label, "wait");
    assert_eq!(
        [
            b.lower_whisker,
            b.lower_hinge,
            b.median,
            b.upper_hinge,
            b.upper_whisker
        ],
        [2.0, 5.5, 10.0, 14.0, 19.0]
    );
    assert_eq!(marks(&b.outliers), [(30.0, "3")]);
    assert!(b.extremes.is_empty());

    let by = &charts[1];
    assert_eq!(by.title, "Boxplot of wait by depot");
    assert_eq!(by.category_axis.title, "depot");
    let [harbour, airport] = boxes(by) else {
        panic!()
    };
    // Harbour, 9 cases: hinges at depth 3 (5 and 8), median 6, step 4.5: 13 is an outlier
    // (12.5 or more), 30 an extreme (17 or more)
    assert_eq!((harbour.label.as_str(), harbour.n), ("Harbour", 9.0));
    assert_eq!(
        [
            harbour.lower_whisker,
            harbour.lower_hinge,
            harbour.median,
            harbour.upper_hinge,
            harbour.upper_whisker
        ],
        [2.0, 5.0, 6.0, 8.0, 8.0]
    );
    assert_eq!(marks(&harbour.outliers), [(13.0, "8")]);
    assert_eq!(marks(&harbour.extremes), [(30.0, "3")]);
    // Airport, 6 cases: hinges at depth 2 (12 and 18), median 14, nothing marked
    assert_eq!(
        [
            airport.lower_whisker,
            airport.lower_hinge,
            airport.median,
            airport.upper_hinge,
            airport.upper_whisker
        ],
        [10.0, 12.0, 14.0, 18.0, 19.0]
    );
    assert!(airport.outliers.is_empty() && airport.extremes.is_empty());

    // the data table: a row per box
    let table = by.data_table();
    assert_eq!(
        table.columns,
        [
            "depot",
            "N",
            "Lower whisker",
            "Lower hinge",
            "Median",
            "Upper hinge",
            "Upper whisker",
            "Outliers",
            "Extremes"
        ]
    );
    assert_eq!(table.rows[0][7], Cell::Text("13 (case 8)".into()));
    assert_eq!(table.rows[0][8], Cell::Text("30 (case 3)".into()));
    assert_eq!(table.caption.as_deref(), Some("Boxplot."));
}

#[test]
fn boxplot_cases_take_the_id_variable() {
    let (_, charts, _) = run(
        STOPS,
        "EXAMINE wait BY depot /ID=stop /PLOT BOXPLOT /NOTOTAL.",
    );
    assert_eq!(charts.len(), 1);
    let harbour = &boxes(&charts[0])[0];
    assert_eq!(marks(&harbour.outliers), [(13.0, "h")]);
    assert_eq!(marks(&harbour.extremes), [(30.0, "c")]);
}

#[test]
fn compare_variables_puts_every_dependent_in_one_plot() {
    let (_, charts, _) = run(
        STOPS,
        "COMPUTE twice = wait * 2.\nEXAMINE wait twice BY depot /PLOT BOXPLOT /COMPARE VARIABLES /NOTOTAL.",
    );
    assert_eq!(charts.len(), 1);
    let names: Vec<(&str, Option<&str>)> = boxes(&charts[0])
        .iter()
        .map(|b| (b.label.as_str(), b.group.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("Harbour", Some("wait")),
            ("Harbour", Some("twice")),
            ("Airport", Some("wait")),
            ("Airport", Some("twice"))
        ]
    );
    assert_eq!(boxes(&charts[0])[1].median, 12.0);
    assert_eq!(charts[0].legend.as_deref(), Some("Variable"));
    // without factors: a box per dependent
    let (_, charts, _) = run(
        STOPS,
        "COMPUTE twice = wait * 2.\nEXAMINE wait twice /PLOT BOXPLOT /COMPARE VARIABLES.",
    );
    let labels: Vec<&str> = boxes(&charts[0]).iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["wait", "twice"]);
}

#[test]
fn stem_and_leaf_lines_and_the_text_form() {
    let (_, charts, _) = run(STOPS, "EXAMINE wait BY depot /PLOT STEMLEAF /NOTOTAL.");
    assert_eq!(charts.len(), 2);
    let harbour = &charts[0];
    assert_eq!(harbour.kind, ChartKind::StemLeaf);
    assert_eq!(
        harbour.title,
        "Stem-and-leaf plot of wait (depot = Harbour)"
    );
    // 7 values inside the outlier bounds, 2 to 8: 8 lines wanted, width 0.75 goes to 1, a stem
    // per whole number with tenths as leaves; 13 and 30 are extremes
    let Some(Plot::StemLeaf(display)) = &harbour.plot else {
        panic!()
    };
    assert_eq!((display.stem_width, display.each_leaf), (1.0, 1.0));
    assert_eq!(display.low_extremes, None);
    let high = display.high_extremes.as_ref().unwrap();
    assert_eq!((high.frequency, high.bound), (2.0, 13.0));
    let table = harbour.data_table();
    assert_eq!(table.columns, ["Frequency", "Stem", "Leaf"]);
    let text: Vec<Vec<Cell>> = table.rows.clone();
    let row = |n: f64, stem: &str, leaf: &str| {
        vec![
            Cell::Number(n),
            Cell::Text(stem.into()),
            Cell::Text(leaf.into()),
        ]
    };
    assert_eq!(
        text,
        [
            row(1.0, "2", "0"),
            row(0.0, "3", ""),
            row(1.0, "4", "0"),
            row(2.0, "5", "00"),
            row(1.0, "6", "0"),
            row(1.0, "7", "0"),
            row(1.0, "8", "0"),
            row(2.0, "Extremes", "(>=13)"),
        ]
    );
    assert_eq!(
        table.caption.as_deref(),
        Some("Stem-and-leaf plot. Stem width: 1; each leaf: 1 case(s)")
    );
}

#[test]
fn normal_q_q_points_follow_the_ranks() {
    let (_, charts, warnings) = run(STOPS, "EXAMINE wait BY depot /PLOT NPPLOT /NOTOTAL.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    use ChartKind::{DetrendedNormalQq as D, NormalQq as N};
    assert_eq!(kinds, [N, D, N, D]);
    // Harbour: 5 is tied at ranks 3 and 4, so its score is at 3.5 / 10; values from scipy
    let harbour = points(&charts[0]);
    let want = [
        (2.0, -1.2815515655446008),
        (4.0, -0.8416212335729142),
        (5.0, -0.3853204664075676),
        (6.0, 0.0),
        (7.0, 0.2533471031357998),
        (8.0, 0.5244005127080407),
        (13.0, 0.8416212335729144),
        (30.0, 1.2815515655446008),
    ];
    assert_eq!(harbour.len(), want.len());
    for (p, (x, y)) in harbour.iter().zip(want) {
        close(p.x, x);
        close(p.y, y);
    }
    let detrended = points(&charts[1]);
    close(detrended[2].y, -0.07263625891343384);
    close(detrended[7].y, 1.2044992290551209);
    // Airport: mean 14.5, standard deviation sqrt(12.3); the line is (x - mean) / sd
    let airport = points(&charts[3]);
    let want = [
        -0.21552786027431048,
        -0.14688361370738812,
        -0.24768709159144567,
        -0.03744588266465468,
        0.43201658796348863,
        0.2155278602743107,
    ];
    for (p, y) in airport.iter().zip(want) {
        close(p.y, y);
    }
    let Some(Plot::Points { lines, .. }) = &charts[2].plot else {
        panic!()
    };
    let sd = 12.3f64.sqrt();
    close(lines[0].slope, 1.0 / sd);
    close(lines[0].intercept, -14.5 / sd);
    assert_eq!(charts[2].category_axis.title, "Observed value");
    assert_eq!(charts[2].value_axis.title, "Expected normal");
}

#[test]
fn weighted_ranks_give_the_mean_rank_of_the_weight() {
    let data = "DATA LIST LIST /x w (F2).
BEGIN DATA
1 1
2 3
4 1
END DATA.
WEIGHT BY w.
";
    let (_, charts, _) = run(data, "EXAMINE x /PLOT NPPLOT.");
    // W = 5: ranks 1, 3 (the mean of 2 to 4) and 5, over W + 1 = 6
    let p = points(&charts[0]);
    close(p[0].y, -0.9674215661017014);
    close(p[1].y, 0.0);
    close(p[2].y, 0.9674215661017014);
}

#[test]
fn spread_versus_level_with_its_slope_and_levene_tests() {
    let (tables, charts, warnings) =
        run(STOPS, "EXAMINE wait BY depot /PLOT SPREADLEVEL /NOTOTAL.");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 1);
    let chart = &charts[0];
    assert_eq!(chart.kind, ChartKind::SpreadLevel);
    // medians 6 and 14, interquartile ranges (HAVERAGE) 6 and 6.75, as natural logs
    let p = points(chart);
    close(p[0].x, 6f64.ln());
    close(p[0].y, 6f64.ln());
    close(p[1].x, 14f64.ln());
    close(p[1].y, 6.75f64.ln());
    assert_eq!(p[1].label.as_deref(), Some("Airport"));
    let slope = 0.13901018893468983;
    let Some(Plot::Points { lines, .. }) = &chart.plot else {
        panic!()
    };
    close(lines[0].slope, slope);
    assert_eq!(
        chart.footnote.as_deref(),
        Some("Slope = 0.139; power for transformation = 0.861")
    );
    // Levene's tests, against scipy (mean, median) and by hand (trimmed mean)
    let levene = tables
        .iter()
        .find(|t| t.title == "Test of Homogeneity of Variance")
        .unwrap();
    let num = |r: usize, c: usize| match levene.rows[r][c] {
        Cell::Number(n) => n,
        _ => panic!(),
    };
    close(num(0, 2), 1.1691915451738968);
    close(num(0, 5), 0.29921843790576563);
    close(num(1, 2), 0.33408849557522136);
    assert!((num(1, 5) - 0.5731380219516902).abs() < 1e-7);
    close(num(2, 4), 8.514517609268019);
    assert!((num(2, 5) - 0.5782246930039404).abs() < 1e-7);
    close(num(3, 2), 0.716004193504213);
}

#[test]
fn spread_versus_level_after_a_power() {
    let (tables, charts, _) = run(
        STOPS,
        "EXAMINE wait BY depot /PLOT SPREADLEVEL(0.5) /NOTOTAL.",
    );
    let p = points(&charts[0]);
    // square roots: medians of the roots are the roots of the medians (6 and 14)
    close(p[0].x, 6f64.sqrt());
    assert!(!charts[0].footnote.as_deref().unwrap().contains("power"));
    let levene = tables.last().unwrap();
    assert_eq!(
        levene.caption.as_deref(),
        Some("On the values raised to the power 0.5.")
    );
    // a log of 0 is refused with a warning
    let data = "DATA LIST LIST /x g (F2).\nBEGIN DATA\n0 1\n2 1\n3 2\n5 2\nEND DATA.\n";
    let (_, charts, warnings) = run(data, "EXAMINE x BY g /PLOT SPREADLEVEL(0).");
    assert!(charts.is_empty());
    assert_eq!(
        warnings,
        ["no spread-versus-level plot of x by g: some values cannot take the power transformation"]
    );
    let (_, _, warnings) = run(data, "EXAMINE x /PLOT SPREADLEVEL.");
    assert_eq!(
        warnings,
        ["SPREADLEVEL needs a factor after BY, so no spread-versus-level plot is made"]
    );
}

#[test]
fn default_plots_are_the_stem_and_leaf_and_the_boxplot() {
    let (_, charts, warnings) = run(STOPS, "EXAMINE wait.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    assert_eq!(kinds, [ChartKind::StemLeaf, ChartKind::Boxplot]);
    let (_, charts, _) = run(STOPS, "EXAMINE wait /PLOT HISTOGRAM.");
    assert_eq!(charts[0].kind, ChartKind::Histogram);
    assert_eq!(charts[0].series[0].values.iter().sum::<f64>(), 15.0);
    let (_, charts, _) = run(STOPS, "EXAMINE wait /PLOT NONE.");
    assert!(charts.is_empty());
}
