//! SPCHART on small made-up data sets. Every expected number was computed by hand from the
//! formulas of the Algorithms manual in a separate script, with d2, d3 and c4 taken from R's
//! integrate() and lgamma() over their definitions.

use statscraft_engine::{Cell, Chart, Engine, Table};
use statscraft_syntax::parse;

/// Fill weights of four batches of four bottles, the batches mixed in case order.
const FILLS: &str = "DATA LIST LIST /batch fill.
BEGIN DATA
1 10.2
2 9.9
3 10.6
4 9.6
1 9.8
2 10.0
3 10.1
4 9.9
1 10.1
2 10.3
3 10.2
4 10.0
1 10.4
2 9.7
3 10.5
4 9.8
END DATA.
";

/// Eight readings of a gauge.
const GAUGE: &str = "DATA LIST LIST /reading.
BEGIN DATA
5.1
4.8
5.4
5.0
4.6
5.3
5.2
4.9
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

fn fails(data: &str, source: &str) -> String {
    let (commands, errors) = parse(&format!("{data}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut last = Ok(Vec::new());
    for command in &commands {
        last = engine.execute(&command.command);
    }
    last.expect_err("the command should fail").to_string()
}

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-6, "{got} against {want}");
}

/// The values of the series whose name starts with `name`.
fn series<'a>(chart: &'a Chart, name: &str) -> &'a [f64] {
    &chart
        .series
        .iter()
        .find(|s| s.name.starts_with(name))
        .unwrap_or_else(|| panic!("no series {name} in {:?}", chart.series))
        .values
}

fn all_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        if w.is_nan() {
            assert!(g.is_nan(), "{got:?} against {want:?}");
        } else {
            close(*g, *w);
        }
    }
}

/// The value of a statistic in the Process Statistics table, found by its group and label.
fn statistic(tables: &[Table], group: &str, label: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == "Process Statistics")
        .expect("statistics table");
    let row = table
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text(group.into()) && r[1] == Cell::Text(label.into()))
        .unwrap_or_else(|| panic!("no {label} in {:?}", table.rows));
    match row[2] {
        Cell::Number(n) => n,
        ref other => panic!("{other:?}"),
    }
}

#[test]
fn xr_units_by_batch_with_capability() {
    let (tables, charts, _) = run(
        FILLS,
        "SPCHART /XR=fill BY batch /LSL=9.5 /USL=10.9 /STATISTICS=CP CPK PP PPK CAPSIGMA STDDEV.",
    );
    assert_eq!(charts.len(), 2);
    let (xbar, range) = (&charts[0], &charts[1]);
    let labels: Vec<&str> = xbar.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["1", "2", "3", "4"]);
    all_close(series(xbar, "Mean"), &[10.125, 9.975, 10.35, 9.825]);
    all_close(series(xbar, "Center"), &[10.06875; 4]);
    all_close(series(xbar, "UCL"), &[10.451263521852997; 4]);
    all_close(series(xbar, "LCL"), &[9.686236478147002; 4]);
    all_close(series(xbar, "USL"), &[10.9; 4]);
    all_close(series(xbar, "LSL"), &[9.5; 4]);
    // a constant series carries its value in its name
    assert!(xbar.series.iter().any(|s| s.name == "UCL = 10.4513"));
    assert_eq!(xbar.title, "Control Chart: fill");
    assert_eq!(xbar.footnote.as_deref(), Some("Sigma level: 3"));
    all_close(series(range, "Range"), &[0.6, 0.6, 0.5, 0.4]);
    all_close(series(range, "Center"), &[0.525; 4]);
    all_close(series(range, "UCL"), &[1.1980770662742934; 4]);
    all_close(series(range, "LCL"), &[0.0; 4]);
    let capability = "Process Capability Indices";
    let performance = "Process Performance Indices";
    close(statistic(&tables, capability, "Cp"), 0.9150003333333331);
    close(statistic(&tables, capability, "CpK"), 0.7434377708333325);
    close(statistic(&tables, performance, "Pp"), 0.8133900512837778);
    close(statistic(&tables, performance, "PpK"), 0.6608794166680689);
    close(
        statistic(&tables, "Process Data", "Capability sigma"),
        0.25500901456866515,
    );
    close(
        statistic(&tables, "Process Data", "Standard deviation"),
        0.28686524130097507,
    );
    // XBARONLY leaves the range chart out
    let (_, charts, _) = run(FILLS, "SPCHART fill BY batch (XBARONLY).");
    assert_eq!(charts.len(), 1);
}

#[test]
fn xs_subgroups_of_unequal_size() {
    // the second case misses one value, so its subgroup has two units and its own limits
    let data = "DATA LIST LIST /a b c.
BEGIN DATA
5.0 5.2 4.9
5.3 5.1 .
4.8 5.0 5.1
5.2 5.4 5.3
4.9 4.7 5.0
END DATA.
";
    let (tables, charts, _) = run(
        data,
        "SPCHART /XS=a b c /CAPSIGMA=WITHIN /STATISTICS=CAPSIGMA.",
    );
    let (xbar, s) = (&charts[0], &charts[1]);
    let labels: Vec<&str> = xbar.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["1", "2", "3", "4", "5"]);
    let wide = 5.343898156300122;
    all_close(
        series(xbar, "UCL"),
        &[wide, 5.406739618620138, wide, wide, wide],
    );
    let low = 4.784673272271306;
    all_close(series(xbar, "LCL"), &[low, 4.72183180995129, low, low, low]);
    all_close(series(xbar, "Center"), &[5.064285714285714; 5]);
    let center = 0.1430674406278223;
    all_close(
        series(s, "Center"),
        &[center, 0.12880595031755143, center, center, center],
    );
    let upper = 0.3674214467824335;
    all_close(
        series(s, "UCL"),
        &[upper, 0.42074874891500735, upper, upper, upper],
    );
    all_close(series(s, "LCL"), &[0.0; 5]);
    close(
        statistic(&tables, "Process Data", "Capability sigma"),
        0.1414213562373095,
    );
    // MINSAMPLE=3 drops the subgroup of two
    let (_, charts, _) = run(data, "SPCHART /XS=a b c /MINSAMPLE=3.");
    assert_eq!(charts[0].categories.len(), 4);
}

#[test]
fn individuals_and_moving_ranges() {
    let (_, charts, _) = run(GAUGE, "SPCHART /IR=reading.");
    let (i, mr) = (&charts[0], &charts[1]);
    all_close(series(i, "Center"), &[5.0375; 8]);
    all_close(series(i, "UCL"), &[6.100972307805894; 8]);
    all_close(series(i, "LCL"), &[3.974027692194105; 8]);
    let mut ranges = vec![f64::NAN];
    ranges.extend([0.3, 0.6, 0.4, 0.4, 0.7, 0.1, 0.3]);
    all_close(series(mr, "Moving range"), &ranges);
    all_close(series(mr, "Center"), &[0.4; 8]);
    all_close(series(mr, "UCL"), &[1.3066127691811256; 8]);
    all_close(series(mr, "LCL"), &[0.0; 8]);
    let (_, charts, _) = run(GAUGE, "SPCHART /IR=reading /SPAN=3.");
    all_close(series(&charts[0], "UCL"), &[6.160054106000126; 8]);
    all_close(series(&charts[1], "UCL"), &[1.630574479372454; 8]);
    assert!(series(&charts[1], "Moving range")[1].is_nan());
    // I alone draws one chart; a span above the number of values is an error
    let (_, charts, _) = run(GAUGE, "SPCHART /I=reading.");
    assert_eq!(charts.len(), 1);
    assert!(fails(GAUGE, "SPCHART /I=reading /SPAN=9.").contains("span"));
}

#[test]
fn weights_repeat_individuals() {
    // weights 2 and 1: the same as the series 1, 1, 3, 2, 2, 4 unweighted
    let data = "DATA LIST LIST /x w.
BEGIN DATA
1 2
3 1
2 2
4 1
5 0.5
END DATA.
WEIGHT BY w.
";
    let (_, charts, _) = run(data, "SPCHART /I=x.");
    all_close(series(&charts[0], "x"), &[1.0, 1.0, 3.0, 2.0, 2.0, 4.0]);
    // mean 13/6, mean moving range (0 + 2 + 1 + 0 + 2) / 5 = 1, d2(2) = 2 / sqrt(pi)
    all_close(
        series(&charts[0], "UCL"),
        &[13.0 / 6.0 + 3.0 / std::f64::consts::FRAC_2_SQRT_PI; 6],
    );
}

#[test]
fn capability_statistics_of_individuals() {
    let (tables, charts, warnings) = run(
        GAUGE,
        "SPCHART /CPCHART=reading /LSL=4.5 /USL=5.6 /TARGET=5.0 \
         /STATISTICS=CP CPL CPU K CR CPM CZL CZU CZOUT PZOUT PPM AZOUT N.",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let chart = &charts[0];
    assert_eq!(chart.title, "Process Capability");
    let normal = chart.normal.as_ref().expect("normal curve");
    close(normal.mean, 5.0375);
    close(normal.std_dev, 0.3544907692686316);
    let c = "Process Capability Indices";
    for (label, want) in [
        ("Cp", 0.5171737862499998),
        ("CpL", 0.5054198365624997),
        ("CpU", 0.5289277359375),
        ("K", 0.022727272727273058),
        ("CR", 1.9335860141925365),
        ("CpM", 0.5143041110857575),
        ("Z lower", 1.516259509687499),
        ("Z upper", 1.5867832078125),
        ("Estimated % outside the limits", 12.100774949119536),
    ] {
        let got = statistic(&tables, c, label);
        assert!((got - want).abs() < 1e-6, "{label}: {got} against {want}");
    }
    let p = "Process Performance Indices";
    close(
        statistic(&tables, p, "Estimated % outside the limits"),
        3.956765646903282,
    );
    close(statistic(&tables, p, "PpM"), 0.6801503804482463);
    close(statistic(&tables, "Process Data", "N"), 8.0);
    // no value lies at or beyond 4.5 or 5.6
    close(
        statistic(
            &tables,
            "Assessing Normality",
            "Observed % outside the limits",
        ),
        0.0,
    );
    // NORMAL=PERFORMANCE draws the curve of the standard deviation
    let (_, charts, _) = run(
        GAUGE,
        "SPCHART /CPCHART=reading /LSL=4.5 /USL=5.6 /NORMAL=PERFORMANCE.",
    );
    close(
        charts[0].normal.as_ref().unwrap().std_dev,
        0.2669269563007829,
    );
}

#[test]
fn statistics_that_lack_limits_are_left_out_with_a_warning() {
    let (tables, _, warnings) = run(
        GAUGE,
        "SPCHART /IR=reading /USL=5.6 /STATISTICS=CP CPK CPM.",
    );
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    let table = tables
        .iter()
        .find(|t| t.title == "Process Statistics")
        .unwrap();
    assert_eq!(table.rows.len(), 1);
    // with USL only, CpK is the one-sided CpU
    close(
        statistic(&tables, "Process Capability Indices", "CpK"),
        0.5289277359375,
    );
}

#[test]
fn p_and_np_charts_of_counts() {
    let data = "DATA LIST LIST /week bad lot.
BEGIN DATA
1 3 50
2 5 50
3 2 40
4 8 60
5 4 50
6 9 5
END DATA.
";
    // week 6 has more defects than units, so it is dropped
    let (_, charts, warnings) = run(data, "SPCHART /P=COUNT(bad) N(lot) BY week.");
    assert!(warnings.is_empty());
    let p = &charts[0];
    assert_eq!(p.categories.len(), 5);
    all_close(
        series(p, "Proportion"),
        &[0.06, 0.1, 0.05, 0.13333333333333333, 0.08],
    );
    all_close(series(p, "Center"), &[0.088; 5]);
    let u = 0.20819184664526957;
    all_close(
        series(p, "UCL"),
        &[u, u, 0.2223785697200264, 0.1977196427263596, u],
    );
    all_close(series(p, "LCL"), &[0.0; 5]);
    let (_, charts, warnings) = run(data, "SPCHART /NP=COUNT(bad) N(lot) BY week.");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let np = &charts[0];
    all_close(series(np, "Center"), &[4.4, 4.4, 3.52, 5.28, 4.4]);
    let u = 10.409592332263479;
    all_close(
        series(np, "UCL"),
        &[u, u, 8.895142788801056, 11.863178563581576, u],
    );
}

#[test]
fn p_chart_of_units_with_a_status() {
    // two lots of four: one and three failures
    let data = "DATA LIST LIST /lot (F1) result (A4).
BEGIN DATA
1 pass
1 pass
1 fail
1 pass
2 fail
2 fail
2 pass
2 fail
END DATA.
";
    let (_, charts, _) = run(data, "SPCHART /P=result BY lot /CONFORM='pass'.");
    all_close(series(&charts[0], "Proportion"), &[0.25, 0.75]);
    // p-bar 0.5, sigma sqrt(0.25 / 4) = 0.25: the limits stop at 0 and 1
    all_close(series(&charts[0], "UCL"), &[1.0, 1.0]);
    all_close(series(&charts[0], "LCL"), &[0.0, 0.0]);
    let (_, charts, _) = run(
        data,
        "SPCHART /NP=result BY lot /NONCONFORM='fail' /SIGMAS=1.5.",
    );
    all_close(series(&charts[0], "Number"), &[1.0, 3.0]);
    // center 2, sigma sqrt(4 * 0.5 * 0.5) = 1
    all_close(series(&charts[0], "UCL"), &[3.5, 3.5]);
    all_close(series(&charts[0], "LCL"), &[0.5, 0.5]);
}

#[test]
fn u_and_c_charts() {
    let data = "DATA LIST LIST /day flaws.
BEGIN DATA
1 0
1 2
1 1
2 3
2 1
3 0
3 0
3 1
3 2
4 1
4 4
END DATA.
";
    let (_, charts, _) = run(data, "SPCHART /U=flaws BY day.");
    let u = &charts[0];
    all_close(series(u, "Defects per unit"), &[1.0, 2.0, 0.75, 2.5]);
    all_close(series(u, "Center"), &[1.3636363636363635; 4]);
    all_close(
        series(u, "UCL"),
        &[
            3.38623595102609,
            3.840804835170675,
            3.1152589879997903,
            3.840804835170675,
        ],
    );
    let (_, charts, warnings) = run(data, "SPCHART /C=flaws BY day.");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    all_close(
        series(&charts[0], "Center"),
        &[
            4.090909090909091,
            2.727272727272727,
            5.454545454545454,
            2.727272727272727,
        ],
    );
    // equal sizes: c-bar 5 and its limits
    let data = "DATA LIST LIST /defects.
BEGIN DATA
4
7
3
9
5
2
END DATA.
";
    let (_, charts, _) = run(data, "SPCHART /C=COUNT(defects) N(1).");
    all_close(series(&charts[0], "UCL"), &[11.70820393249937; 6]);
    all_close(series(&charts[0], "LCL"), &[0.0; 6]);
}

#[test]
fn rules_name_the_points_out_of_control() {
    let data = "DATA LIST LIST /x (F4.1) tag (A2).
BEGIN DATA
10 a
10.2 b
9.9 c
10.1 d
9.8 e
10 f
10.1 g
9.9 h
13 i
10 j
END DATA.
";
    // mean 10.3, UCL 12.545: the ninth point is above it
    let (tables, _, _) = run(data, "SPCHART /I=x /RULES=UCL LCL /ID=tag.");
    let table = &tables[0];
    assert_eq!(table.title, "Rule Violations");
    assert_eq!(
        table.rows,
        [vec![
            Cell::Text("i".into()),
            Cell::Text("Above the upper control limit".into())
        ]]
    );
    let (tables, _, _) = run(data, "SPCHART /I=x /RULES=LCL.");
    assert!(tables[0].rows.is_empty());
}
