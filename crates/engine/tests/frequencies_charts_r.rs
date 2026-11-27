//! FREQUENCIES charts against R on 150 invented cases with frequency weights that are not whole
//! numbers: the histogram (the weighted count of each bin for the edges the engine's rule picks,
//! nine bins of width 10 from 10 by default and ten of width 5 with MIN(30) MAX(80)), the normal
//! curve (the weighted mean, the standard deviation over W - 1 and the total), the bar chart as
//! counts and as percentages of the valid weight, and the pie chart with a slice for the missing
//! values. The bin rule itself (Sturges moved to a round width) is the engine's own, as the
//! manual does not give it. Scripts in lab/oracles/freqcharts-r.

use statscraft_engine::{Chart, Engine};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x z wt (3F8.2).
BEGIN DATA.
55.95 2 1.3
42.7 5 1.5
39.97 1 1.1
. 4 2.6
66.38 4 2.4
42.58 1 2.4
91.37 5 2.3
82.16 3 0.9
71.7 . 2
64.79 4 2.7
57.07 1 2.4
53.5 3 2
47.05 4 2.6
59.6 9 2.7
37.34 2 2.9
73.16 2 1.1
54.04 5 2.9
48.62 2 0.9
59.29 9 2.5
45.25 3 2.1
65.52 4 0.7
57.56 4 2
39.88 4 2.2
65.33 9 2.1
37.94 4 2.3
49.32 3 0.6
82.84 2 0.7
48.3 2 1.3
13.4 5 2.3
. 3 1.7
55.04 3 1.9
33.78 4 2.8
55.55 1 1.5
58.03 5 1.5
58.39 2 2.3
42.07 2 0.7
55.67 4 2.3
57.06 5 1.3
29.88 1 1.6
63.72 2 2.5
32.55 3 2.5
36.26 2 1.4
28.08 3 2.1
69.25 5 2.1
41.43 3 1.5
42.33 5 0.9
37.02 2 2.3
51.62 1 2.5
44.24 2 2.2
61.63 1 0.8
61.99 1 1.8
65 3 1.9
53 9 1.6
37.71 3 0.6
61.36 2 1.6
53.34 5 1.6
48.27 2 1.6
35.94 5 1.8
30.03 1 0.6
69.18 . 0.8
52.32 3 2.8
60.63 4 2
53.94 4 2
59 1 2.1
48.71 3 2
50.38 9 2.6
44.05 3 0.9
75.94 1 1.5
48.75 5 0.7
45.69 2 1.5
38.19 1 2.3
24.03 3 2.3
45.21 4 2.5
29.89 5 2.6
38.99 2 2.5
36.47 2 2
. 2 2.2
34.26 1 2.2
70.89 1 1.4
28.54 3 1.5
68.72 2 0.9
38.08 3 2.1
39.26 4 2.4
66.68 3 1.4
42.6 5 2.6
35.46 1 1.2
39.28 2 2.2
46.06 1 1.6
45.51 2 2.5
39.54 3 2.5
77.44 9 0.5
35.12 3 1.4
60.72 3 2.8
60.8 1 1
71.41 5 1.8
47.19 4 1.7
30.33 1 2
44.74 4 2.7
62.92 3 1.9
44.12 5 0.5
54.49 4 2.2
37.25 1 1.6
74.8 1 1
38.15 1 2.9
56.83 3 1
71.22 9 0.7
61.4 4 1.7
39.34 1 0.6
24.85 1 1.6
55.57 4 1.7
60.71 1 1.5
64.22 2 0.8
34.21 4 2.2
63.42 1 2
37.47 9 0.8
57.76 2 0.6
37.94 5 2
39.46 2 2.8
51.42 2 0.6
67.52 2 2.9
51.82 5 0.9
48.25 2 2.6
62.93 3 1.4
51.62 3 2.4
65.56 1 2.1
53.37 1 1.3
53.43 2 2.3
57.44 1 1.2
64.43 2 1
51 1 2.1
33.29 2 1.1
42.25 5 0.7
35.49 9 2.4
56.06 1 1.8
53.29 9 1.9
57.13 2 1
68.49 3 1.2
24.41 3 1.4
25.99 2 2
52.62 1 2.3
50.48 4 2.3
54.75 3 0.5
40.35 2 2.6
56.23 2 0.7
46.73 9 1
76.27 2 1.4
62.25 1 2.8
31.39 4 2.2
64 3 0.6
57.17 2 2.4
END DATA.
MISSING VALUES z (9).
VALUE LABELS z 1 'one' 2 'two' 3 'three' 4 'four' 5 'five' 9 'none'.
";

fn charts(syntax: &str) -> Vec<Chart> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut out = Vec::new();
    for command in &commands {
        engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("{e}"));
        out = engine.take_charts();
    }
    out
}

fn assert_close(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g - w).abs() <= 1e-8 + 1e-7 * w.abs(),
            "{what}: {got:?} against {want:?}"
        );
    }
}

fn bins(chart: &Chart) -> Vec<[f64; 2]> {
    chart
        .categories
        .iter()
        .map(|c| c.bin.expect("a bin"))
        .collect()
}

#[test]
fn the_histogram_and_its_normal_curve_agree_with_r() {
    let charts = charts("WEIGHT BY wt.\nFREQUENCIES x /HISTOGRAM=NORMAL /FORMAT=NOTABLE.");
    let chart = &charts[0];
    // nine bins of width 10 from 10 (the engine's rule); R counts the weighted cases of each
    let edges: Vec<[f64; 2]> = (0..9)
        .map(|i| [10.0 + 10.0 * i as f64, 20.0 + 10.0 * i as f64])
        .collect();
    assert_eq!(bins(chart), edges);
    assert_close(
        &chart.series[0].values,
        &[2.3, 15.1, 61.9, 44.4, 71.0, 47.4, 11.4, 1.6, 2.3],
        "counts",
    );
    let normal = chart.normal.as_ref().expect("a normal curve");
    assert_close(
        &[normal.mean, normal.std_dev, normal.total],
        &[49.81677156, 13.65937505, 257.4],
        "normal curve",
    );
    // the bins cover the data: the smallest value 13.4 and the largest 91.37
    assert!(edges[0][0] <= 13.4 && edges[8][1] > 91.37);
}

#[test]
fn min_and_max_bound_the_bars_agree_with_r() {
    let charts = charts("WEIGHT BY wt.\nFREQUENCIES x /HISTOGRAM MIN(30) MAX(80) /FORMAT=NOTABLE.");
    let chart = &charts[0];
    let edges: Vec<[f64; 2]> = (0..10)
        .map(|i| [30.0 + 5.0 * i as f64, 35.0 + 5.0 * i as f64])
        .collect();
    assert_eq!(bins(chart), edges);
    assert_close(
        &chart.series[0].values,
        &[15.6, 46.3, 19.2, 25.2, 36.8, 34.2, 28.9, 18.5, 8.0, 3.4],
        "counts",
    );
}

#[test]
fn bar_chart_percentages_and_the_pie_with_a_missing_slice_agree_with_r() {
    let bar = charts("WEIGHT BY wt.\nFREQUENCIES z /BARCHART /FORMAT=NOTABLE.");
    assert_close(
        &bar[0].series[0].values,
        &[54.8, 61.4, 47.9, 48.2, 30.0],
        "bar counts",
    );
    let percent = charts("WEIGHT BY wt.\nFREQUENCIES z /BARCHART=PERCENT /FORMAT=NOTABLE.");
    assert_close(
        &percent[0].series[0].values,
        &[22.616591, 25.340487, 19.76888155, 19.89269501, 12.38134544],
        "bar percents",
    );
    let pie = charts("WEIGHT BY wt.\nFREQUENCIES z /PIECHART=PERCENT /FORMAT=NOTABLE.");
    assert_close(
        &pie[0].series[0].values,
        &[
            20.76544146,
            23.26638878,
            18.1508147,
            18.26449413,
            11.3679424,
            8.18491853,
        ],
        "pie percents",
    );
    assert!(pie[0].categories.last().expect("a slice").missing);
}
