//! QUICK CLUSTER against R on 45 invented cases of three variables in four groups, two cases
//! missing a variable (left out): the manual's pass for the initial centers (written out in R, the
//! replacement tests of the Algorithms manual), the iteration history, the final centers, the
//! distances between them, the ANOVA table (the cluster and error mean squares from `anova(lm)`
//! on the final membership), the case counts and the saved membership and distance, then
//! NOINITIAL, KMEANS(UPDATE) (the first iteration begins with a pass that moves the nearest
//! center after each case; after it every iteration is a pass of assigning and averaging),
//! CONVERGE(0.05) and METHOD=CLASSIFY with given centers. Scripts in lab/oracles/quickcluster-r.
//!
//! KMEANS(UPDATE) is written in R as the syntax pages describe it: in every iteration a center is
//! recalculated each time a case is assigned (the center counting as one case of its mean), and
//! the final centers are the means of the cases that the last centers classify. The algorithm
//! pages describe a different order (that pass in the first iteration only, then iterations of
//! assigning all cases and averaging); the two readings do not agree and the engine follows the
//! syntax pages.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c.
BEGIN DATA.
1.21 -1.5 -0.28
1.36 0.98 0.77
-3.03 1.63 6.36
4.64 0.51 -0.03
-0.14 -0.67 -0.82
1.34 0.85 1.09
1.94 . 0.58
4.37 2.82 -1.59
-1.81 2.29 3.59
2.21 0.67 0.05
-0.51 1.51 0.98
-3.91 2.31 6.18
-4.49 3.27 6.81
-0.63 1.43 -1.49
-4.89 2.16 6.51
5.16 0.33 -0.13
3.42 1.94 -1.47
0.45 0.86 -1.07
0.84 0.02 -0.32
0.2 5.9 1.81
1.33 -4.2 -0.97
-1.07 5.53 1.94
-2.27 2.21 6.24
0.34 -1.63 0.07
1.12 5.26 4.91
1.37 5.91 0.15
-2.13 2.81 5.12
3.27 -1.11 -0.58
2.75 . -0.45
6 -0.18 0.17
5.95 2.57 -0.98
-0.82 0.39 0.56
-0.58 4.42 2.65
-3.9 1.68 4.5
3.8 -0.72 0.71
4.74 1.9 -0.39
0.38 4.42 4.58
-0.93 1.84 0.46
1.27 -2.85 -0.43
6.6 0.13 1.9
5.63 1 -0.59
-2.74 2.08 5.56
1.44 0.45 -1.9
-1.08 1.11 0.09
-2.24 2.75 6.24
END DATA.
";

fn run(syntax: &str) -> (Engine, Vec<Table>) {
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
    (engine, tables)
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// The centers of a table with one row per variable and one column per cluster, against R's
/// vector (cluster after cluster, a b c within each).
fn centers(t: &[Table], title: &str, want: &[f64]) {
    for (i, row) in table(t, title).rows.iter().enumerate() {
        for (j, cell) in row[1..].iter().enumerate() {
            near(
                number(cell),
                want[j * 3 + i],
                &format!("{title} variable {i} cluster {j}"),
            );
        }
    }
}

fn sizes(t: &[Table], want: &[f64]) {
    let rows = &table(t, "Number of Cases in each Cluster").rows;
    for (j, w) in want.iter().enumerate() {
        near(number(&rows[j][2]), *w, &format!("cluster {j} size"));
    }
}

fn history(t: &[Table], want: &[Vec<f64>]) {
    let rows = &table(t, "Iteration History").rows;
    assert_eq!(rows.len(), want.len(), "iterations");
    for (r, w) in rows.iter().zip(want) {
        for (j, v) in w.iter().enumerate() {
            near(number(&r[1 + j]), *v, "change");
        }
    }
}

#[test]
fn the_default_passes_agree_with_r() {
    let (engine, t) = run(
        "QUICK CLUSTER a b c /CRITERIA=CLUSTER(4) /PRINT=INITIAL ANOVA DISTANCE /SAVE=CLUSTER(cl) DISTANCE(dist).",
    );
    centers(
        &t,
        "Initial Cluster Centers",
        &[
            1.33, -4.2, -0.97, -0.93, 1.84, 0.46, -4.49, 3.27, 6.81, 6.6, 0.13, 1.9,
        ],
    );
    history(
        &t,
        &[
            vec![2.019431603, 1.375001182, 1.756785434, 2.565865916],
            vec![0.8974085608, 0.6058307657, 0.2782450071, 0.2468648103],
            vec![0.4420131503, 0.4353544159, 0.0, 0.0],
            vec![0.6518337556, 1.092658515, 0.0, 0.0],
            vec![0.1741911661, 0.4449931725, 0.0, 0.0],
            vec![0.3247421241, 1.044029071, 0.0, 0.0],
            vec![0.1770538347, 0.0, 0.0, 0.2538353895],
            vec![0.0, 0.0, 0.0, 0.0],
        ],
    );
    centers(
        &t,
        "Final Cluster Centers",
        &[
            0.48,
            -0.04625,
            -0.200625,
            0.2366666667,
            5.24,
            2.673333333,
            -3.141000001,
            2.319,
            5.711,
            4.870909091,
            0.8354545455,
            -0.2709090909,
        ],
    );
    sizes(&t, &[16.0, 6.0, 10.0, 11.0]);
    let upper = [
        6.021900587,
        7.324845302,
        5.400767713,
        4.47910989,
        7.038805791,
        10.10815665,
    ];
    let rows = &table(&t, "Distances between Final Cluster Centers").rows;
    let pairs = [(0, 1), (0, 2), (1, 2), (0, 3), (1, 3), (2, 3)];
    for ((i, j), w) in pairs.iter().zip(upper) {
        near(number(&rows[*i][1 + j]), w, "distance between centers");
    }
    // the ANOVA: cluster mean square, df, error mean square, df, F, p of a, b and c
    let anova = [
        113.6415836,
        3.0,
        1.113741391,
        39.0,
        102.0358806,
        1.646770292e-18,
        44.56612308,
        3.0,
        1.674316352,
        39.0,
        26.61750453,
        1.539952726e-09,
        88.73868571,
        3.0,
        1.20781559,
        39.0,
        73.47039273,
        4.218418671e-16,
    ];
    for (i, row) in table(&t, "ANOVA").rows.iter().enumerate() {
        for (c, w) in anova[i * 6..i * 6 + 6].iter().enumerate() {
            near(number(&row[1 + c]), *w, "ANOVA");
        }
    }
    // saved: the cases 7 and 29 miss a variable and get none
    let membership = [
        1.0, 1.0, 3.0, 4.0, 1.0, 1.0, 4.0, 3.0, 1.0, 1.0, 3.0, 3.0, 1.0, 3.0, 4.0, 4.0, 1.0, 1.0,
        2.0, 1.0, 2.0, 3.0, 1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 4.0, 1.0, 2.0, 3.0, 4.0, 4.0, 2.0, 1.0,
        1.0, 4.0, 4.0, 3.0, 1.0, 1.0, 3.0,
    ];
    let distance = [
        1.62867721,
        1.664242156,
        0.9530178382,
        0.4661296595,
        1.075680925,
        1.791250109,
        2.435021849,
        2.504205063,
        1.889107449,
        2.189951929,
        0.9007791072,
        1.982927886,
        2.252532342,
        1.929425562,
        0.5990936681,
        2.182424537,
        1.256185477,
        0.3850187698,
        1.087331085,
        4.309069152,
        1.526189882,
        1.024872187,
        1.612793215,
        2.404860818,
        2.846147494,
        1.269835816,
        2.538353895,
        1.581263748,
        2.162381798,
        1.568076673,
        1.157535697,
        1.565542398,
        2.128019682,
        1.079155623,
        2.080462021,
        2.445907695,
        2.921938903,
        2.863610967,
        0.8397102019,
        0.4906353024,
        2.013886653,
        1.963409523,
        1.130222544,
    ];
    let data = engine.dataset().unwrap();
    let (Values::Numeric(cl), Values::Numeric(dist)) = (
        data.variable("cl").unwrap().values(),
        data.variable("dist").unwrap().values(),
    ) else {
        panic!("saved variables are numeric")
    };
    assert_eq!(cl.get(6), None);
    assert_eq!(cl.get(28), None);
    for (k, case) in (0..45).filter(|c| *c != 6 && *c != 28).enumerate() {
        assert_eq!(
            cl.get(case),
            Some(membership[k]),
            "membership of case {}",
            case + 1
        );
        near(
            dist.get(case).expect("a distance"),
            distance[k],
            &format!("distance of case {}", case + 1),
        );
    }
}

#[test]
fn noinitial_update_converge_and_classify_agree_with_r() {
    let (_, t) = run("QUICK CLUSTER a b c /CRITERIA=CLUSTER(4) NOINITIAL /PRINT=INITIAL.");
    centers(
        &t,
        "Initial Cluster Centers",
        &[
            1.21, -1.5, -0.28, 1.36, 0.98, 0.77, -3.03, 1.63, 6.36, 4.64, 0.51, -0.03,
        ],
    );
    centers(
        &t,
        "Final Cluster Centers",
        &[
            1.222,
            -0.996,
            -0.625,
            -0.1227272727,
            2.715454545,
            0.8190909091,
            -2.4925,
            2.739166667,
            5.55,
            5.031,
            1.03,
            -0.24,
        ],
    );
    sizes(&t, &[10.0, 11.0, 12.0, 10.0]);
    assert_eq!(table(&t, "Iteration History").rows.len(), 4.0 as usize);
    let (_, t) = run("QUICK CLUSTER a b c /CRITERIA=CLUSTER(4) MXITER(30) /METHOD=KMEANS(UPDATE).");
    history(
        &t,
        &[
            vec![1.419101124, 1.092431655, 2.234416382, 2.624976719],
            vec![1.00808333, 0.3532010329, 0.1718781832, 0.21874806],
            vec![0.3502036907, 0.02354673553, 0.01322139871, 0.2262841308],
            vec![0.04377546134, 0.001569782368, 0.00101703067, 0.02057128461],
            vec![
                0.005471932668,
                0.0001046521579,
                7.823312846e-05,
                0.001870116783,
            ],
            vec![
                0.0006839915835,
                6.976810525e-06,
                6.017932959e-06,
                0.0001700106166,
            ],
            vec![
                8.549894793e-05,
                4.651207023e-07,
                4.629179194e-07,
                1.54555106e-05,
            ],
            vec![
                1.068736849e-05,
                3.100804657e-08,
                3.560907075e-08,
                1.405046418e-06,
            ],
            vec![
                1.335921061e-06,
                2.067203038e-09,
                2.739159492e-09,
                1.277314925e-07,
            ],
            vec![
                1.669901327e-07,
                1.378138922e-10,
                2.107049146e-10,
                1.161195484e-08,
            ],
            vec![
                2.087376657e-08,
                9.18748175e-12,
                1.620813639e-11,
                1.055631522e-09,
            ],
            vec![
                2.60922086e-09,
                6.120616137e-13,
                1.246215291e-12,
                9.596738554e-11,
            ],
            vec![
                3.261524963e-10,
                4.121724473e-14,
                9.569787596e-14,
                8.72387675e-12,
            ],
            vec![
                4.076922764e-11,
                2.775557562e-15,
                7.377764056e-15,
                7.928009029e-13,
            ],
            vec![
                5.096087462e-12,
                2.220446049e-16,
                9.930136613e-16,
                7.244460455e-14,
            ],
            vec![6.370725269e-13, 0.0, 0.0, 6.607517771e-15],
            vec![7.943527425e-14, 0.0, 0.0, 4.440892099e-16],
            vec![9.785720123e-15, 0.0, 0.0, 0.0],
            vec![1.421779192e-15, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
        ],
    );
    centers(
        &t,
        "Final Cluster Centers",
        &[
            1.307777778,
            -1.202222222,
            -0.5755555556,
            -0.075,
            2.560833333,
            0.6616666667,
            -2.4925,
            2.739166667,
            5.55,
            5.031,
            1.03,
            -0.24,
        ],
    );
    sizes(&t, &[9.0, 12.0, 12.0, 10.0]);
    // CONVERGE(0.05): the passes stop when no center moves more than 0.05 of the smallest distance
    // between the initial centers (7 iterations here, against 8 for no change at all)
    let (_, t) = run("QUICK CLUSTER a b c /CRITERIA=CLUSTER(4) CONVERGE(0.05).");
    assert_eq!(table(&t, "Iteration History").rows.len(), 7.0 as usize);
    centers(
        &t,
        "Final Cluster Centers",
        &[
            0.48,
            -0.04625,
            -0.200625,
            0.2366666667,
            5.24,
            2.673333333,
            -3.141000001,
            2.319,
            5.711,
            4.870909091,
            0.8354545455,
            -0.2709090909,
        ],
    );
    // CLASSIFY keeps the given centers: the cases go to the nearest and the means are the final centers
    let (_, t) = run(
        "QUICK CLUSTER a b c /CRITERIA=CLUSTER(4) /METHOD=CLASSIFY /INITIAL=(1.21 -1.5 -0.28 -0.51 1.51 0.98 1.33 -4.2 -0.97 -0.82 0.39 0.56).",
    );
    sizes(&t, &[18.0, 20.0, 1.0, 4.0]);
    centers(
        &t,
        "Final Cluster Centers",
        &[
            3.375,
            0.2044444444,
            -0.3672222222,
            -1.4365,
            2.9905,
            3.8225,
            1.33,
            -4.2,
            -0.97,
            -0.52,
            0.9475,
            -0.4775,
        ],
    );
}
