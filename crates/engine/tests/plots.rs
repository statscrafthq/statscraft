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

#[test]
fn oneway_means_plot_has_a_point_per_group() {
    let (_, charts, warnings) = run(STOPS, "ONEWAY wait BY depot /PLOT MEANS.");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 1);
    let chart = &charts[0];
    assert_eq!(
        (chart.kind, chart.tier),
        (ChartKind::Line, Tier::Presentation)
    );
    assert_eq!(chart.title, "Means plot of wait by depot");
    let labels: Vec<&str> = chart.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Harbour", "Airport"]);
    // 80 / 9 and 87 / 6
    close(chart.series[0].values[0], 80.0 / 9.0);
    close(chart.series[0].values[1], 14.5);
    let (_, charts, _) = run(STOPS, "ONEWAY wait BY depot.");
    assert!(charts.is_empty());
}

/// Eight made-up shoppers rate four store traits.
const RATINGS: &str = "DATA LIST LIST /price range staff parking (F1).
BEGIN DATA
3 4 2 5
5 6 3 4
2 2 4 6
6 7 5 3
4 5 6 2
7 6 2 4
1 3 5 5
5 4 6 1
END DATA.
";

#[test]
fn factor_scree_and_loading_plots() {
    let (_, charts, warnings) = run(
        RATINGS,
        "FACTOR /VARIABLES price range staff parking /CRITERIA FACTORS(2) /ROTATION NOROTATE /PLOT EIGEN ROTATION.",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 2);
    // the eigenvalues of the correlation matrix, from numpy
    let scree = &charts[0];
    assert_eq!(scree.kind, ChartKind::Line);
    let want = [
        2.2512347449536207,
        1.46716335201886,
        0.20557824126548718,
        0.07602366176203187,
    ];
    for (got, want) in scree.series[0].values.iter().zip(want) {
        close(*got, want);
    }
    assert_eq!(scree.category_axis.title, "Component Number");
    // unrotated loadings: eigenvectors times the roots of the eigenvalues, signs aside
    let loadings = &charts[1];
    assert_eq!(loadings.title, "Component Plot");
    let p = points(loadings);
    let want = [
        (0.9134773514513892, 0.32711875160108284),
        (0.8819814887186661, 0.3368878781107229),
        (0.15891144022496595, 0.9627724449332391),
        (0.7833579520641957, 0.5654488935608044),
    ];
    for (p, (x, y)) in p.iter().zip(want) {
        close(p.x.abs(), x);
        close(p.y.abs(), y);
        assert!(p.z.is_none());
    }
    assert_eq!(p[2].label.as_deref(), Some("staff"));
    // three factors: one plot with depth; pairs: one plot each
    let (_, charts, _) = run(
        RATINGS,
        "FACTOR /VARIABLES price range staff parking /CRITERIA FACTORS(3) /PLOT ROTATION.",
    );
    assert_eq!(charts[0].title, "Rotated Component Plot");
    assert!(points(&charts[0])[0].z.is_some());
    let (_, charts, warnings) = run(
        RATINGS,
        "FACTOR /VARIABLES price range staff parking /CRITERIA FACTORS(2) /PLOT ROTATION(1,2)(1,3).",
    );
    assert_eq!(charts.len(), 1);
    assert_eq!(
        warnings,
        ["FACTOR: no loading plot of component 1 and 3: the solution has 2"]
    );
}

/// Visits to a made-up museum by two promotions.
const VISITS: &str = "DATA LIST LIST /visits posters flyers (F2).
BEGIN DATA
3 1 2
4 2 1
8 3 4
7 4 3
12 5 6
11 6 5
15 7 8
19 8 9
END DATA.
";

#[test]
fn regression_residual_plots_and_partial_plots() {
    let (_, charts, warnings) = run(
        VISITS,
        "REGRESSION /DEPENDENT visits /METHOD ENTER posters flyers /RESIDUALS HISTOGRAM NORMPROB \
         /SCATTERPLOT (*ZRESID, *ZPRED) (visits, posters) /PARTIALPLOT.",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    use ChartKind::*;
    assert_eq!(
        kinds,
        [Histogram, NormalPp, Scatter, Scatter, Scatter, Scatter]
    );
    // the histogram of the 8 standardized residuals, with its normal curve
    assert_eq!(charts[0].series[0].values.iter().sum::<f64>(), 8.0);
    assert!(charts[0].normal.is_some());
    // P-P: k / 8 against the normal probability of each standardized residual (numpy)
    let pp = points(&charts[1]);
    let expected = [
        0.11997270604607779,
        0.25260744782062333,
        0.28045031513691576,
        0.34020677474559957,
        0.5910235748458692,
        0.655342264308017,
        0.8397432378277568,
        0.8871418130240516,
    ];
    for (k, (p, y)) in pp.iter().zip(expected).enumerate() {
        close(p.x, (k + 1) as f64 / 8.0);
        close(p.y, y);
    }
    assert_eq!(charts[2].value_axis.title, "Standardized Residual");
    assert_eq!(
        charts[2].category_axis.title,
        "Standardized Predicted Value"
    );
    let scatter = points(&charts[3]);
    assert_eq!((scatter[2].x, scatter[2].y), (3.0, 8.0));
    // partial plots: the slope through the residuals is the coefficient (0.8627 for posters)
    let partial = points(&charts[4]);
    close(partial[0].x, -1.2702702702702702);
    close(partial[0].y, -1.6351351351351342);
    let sxy: f64 = partial.iter().map(|p| p.x * p.y).sum();
    let sxx: f64 = partial.iter().map(|p| p.x * p.x).sum();
    close(sxy / sxx, 0.8627450980392164);
    assert_eq!(
        charts[5].title,
        "Partial regression plot of visits and flyers"
    );
    // one independent: no partial plot
    let (_, charts, warnings) = run(
        VISITS,
        "REGRESSION /DEPENDENT visits /METHOD ENTER posters /PARTIALPLOT.",
    );
    assert!(charts.is_empty());
    assert_eq!(
        warnings,
        [
            "PARTIALPLOT needs two independents in the equation of visits, so no partial plot is made"
        ]
    );
}

#[test]
fn cluster_dendrogram_and_icicle() {
    let data = "DATA LIST LIST /x (F2).\nBEGIN DATA\n1\n2\n4\n8\n9\nEND DATA.\n";
    let (_, charts, warnings) = run(data, "CLUSTER x /METHOD SINGLE /PLOT DENDROGRAM VICICLE.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let kinds: Vec<ChartKind> = charts.iter().map(|c| c.kind).collect();
    assert_eq!(kinds, [ChartKind::Icicle, ChartKind::Dendrogram]);
    // squared distances: 1 and 2 join at 1, 8 and 9 at 1, 4 joins at 4, the rest at 16
    let Some(Plot::Merges { order, merges, .. }) = &charts[1].plot else {
        panic!()
    };
    assert_eq!(order, &[0, 1, 2, 3, 4]);
    let joins: Vec<(usize, usize, f64)> =
        merges.iter().map(|m| (m.left, m.right, m.height)).collect();
    assert_eq!(joins, [(0, 1, 1.0), (3, 4, 1.0), (0, 2, 4.0), (0, 3, 16.0)]);
    // rescaled: 1 at the first join, 25 at the last, 4 lies 3/15 of the way
    let rescaled: Vec<f64> = merges.iter().map(|m| m.rescaled).collect();
    for (got, want) in rescaled
        .iter()
        .zip([1.0, 1.0, 1.0 + 24.0 * 3.0 / 15.0, 25.0])
    {
        close(*got, want);
    }
    // the icicle as a table: the cluster of every case at 1 to 4 clusters
    let table = charts[0].data_table();
    let rows: Vec<Vec<f64>> = table
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Cell::Number(n) => *n,
                    _ => panic!(),
                })
                .collect()
        })
        .collect();
    assert_eq!(
        rows,
        [
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            vec![2.0, 1.0, 1.0, 1.0, 2.0, 2.0],
            vec![3.0, 1.0, 1.0, 2.0, 3.0, 3.0],
            vec![4.0, 1.0, 1.0, 2.0, 3.0, 4.0],
        ]
    );
    // the default is a vertical icicle plot; a range keeps some solutions
    let (_, charts, _) = run(data, "CLUSTER x /METHOD SINGLE.");
    assert_eq!(charts.len(), 1);
    let (_, charts, _) = run(data, "CLUSTER x /METHOD SINGLE /PLOT HICICLE(2,4,2).");
    assert_eq!(charts[0].data_table().rows.len(), 2);
    let (_, charts, _) = run(data, "CLUSTER x /METHOD SINGLE /PLOT NONE.");
    assert!(charts.is_empty());
}

#[test]
fn logistic_classification_plot_bins_the_predicted_probabilities() {
    // made-up trial sign-ups (1) by minutes on the site
    let data = "DATA LIST LIST /signup minutes (F2).
BEGIN DATA
0 1
0 2
0 3
1 4
0 5
1 6
0 7
1 8
1 9
1 10
END DATA.
";
    let (tables, charts, warnings) =
        run(data, "LOGISTIC REGRESSION signup WITH minutes /CLASSPLOT.");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(charts.len(), 1);
    let chart = &charts[0];
    assert_eq!(chart.title, "Classification plot, step 1");
    assert_eq!(chart.categories.len(), 40);
    assert_eq!(chart.categories[20].bin, Some([0.5, 0.525]));
    let (no, yes) = (&chart.series[0].values, &chart.series[1].values);
    assert_eq!(no.iter().sum::<f64>(), 5.0);
    assert_eq!(yes.iter().sum::<f64>(), 5.0);
    // the bins from 0.5 up hold what the Classification Table predicts as 1
    let above = |v: &[f64]| v[20..].iter().sum::<f64>();
    let classification = tables
        .iter()
        .filter(|t| t.title == "Classification Table")
        .last()
        .unwrap();
    let rows = &classification.rows;
    assert_eq!(rows[0][3], Cell::Number(above(no)));
    assert_eq!(rows[1][3], Cell::Number(above(yes)));
}

#[test]
fn tree_diagram_nodes_carry_their_statistics() {
    // a made-up loyalty survey: renewing (1) or not (2) by plan (1 to 3), with counts
    let data = "DATA LIST LIST /plan renew w (F3).
BEGIN DATA
1 1 40
1 2 10
2 1 15
2 2 35
3 1 25
3 2 25
END DATA.
WEIGHT BY w.
";
    let (tables, charts, _) = run(data, "TREE renew [n] BY plan [n].");
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].kind, ChartKind::TreeDiagram);
    let Some(Plot::Tree { nodes }) = &charts[0].plot else {
        panic!()
    };
    let summary = tables.iter().find(|t| t.title == "Model Summary").unwrap();
    let count = summary
        .rows
        .iter()
        .find(|r| r[1] == Cell::Text("Number of Nodes".into()))
        .map(|r| r[2].clone());
    assert_eq!(count, Some(Cell::Number(nodes.len() as f64)));
    let root = &nodes[0];
    assert_eq!(root.parent, None);
    let stat = |node: &statscraft_engine::plot::TreeNode, name: &str| {
        node.statistics
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(stat(root, "Total n"), Some(Cell::Number(150.0)));
    assert_eq!(stat(root, "Split by"), Some(Cell::Text("plan".into())));
    assert!(
        nodes[1..]
            .iter()
            .all(|n| n.parent == Some(0) && n.split.starts_with("plan = "))
    );
    let table = charts[0].data_table();
    assert_eq!(table.columns[..3], ["Node", "Parent", "Split"]);
    assert_eq!(table.rows.len(), nodes.len());
}
