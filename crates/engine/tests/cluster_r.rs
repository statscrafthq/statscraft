//! CLUSTER and QUICK CLUSTER against R on 20 invented cases in three groups: the whole
//! agglomeration schedule (the coefficient of every stage) of single, complete, between-groups
//! average, centroid, median and Ward linkage against `hclust` on the squared Euclidean
//! distances (Ward's coefficient is the cumulative sum of half the `ward.D` heights, the
//! within-cluster sum of squares), the three-cluster solution of each against `cutree`, and
//! k-means from given initial centers (`kmeans` with Lloyd's algorithm) for the final
//! centers, the sizes and the distance of every case to its center.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /id x y z.
BEGIN DATA
1 0.77 -0.24 0.16
2 0.06 1.64 1.26
3 -0.32 -0.21 1.15
4 -0.51 -0.88 -0.72
5 -1.15 -0.57 1.61
6 1.05 -0.01 -0.35
7 -0.76 -1.43 -1.07
8 0.58 -0.98 0.7
9 3.9 1.2 3.9
10 1.86 4.18 1.66
11 3.15 3.65 1.57
12 3.01 3.43 3.92
13 3.6 4.07 1.56
14 3 3.15 3.7
15 4.31 3.95 2.4
16 0.02 2.42 -3.23
17 0.25 4.66 -3.29
18 -0.28 4.59 -2.41
19 1.61 4.15 -3.23
20 1.02 5.07 -2.23
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

const SINGLE: [f64; 19] = [
    0.1269, 0.379, 0.3914, 0.4875, 0.8753, 1.0301, 1.0602, 1.2241, 1.6054, 1.8846, 1.9531, 2.1133,
    2.8224, 3.579, 4.0461, 4.6525, 5.0741, 9.8516, 15.2602,
];
const COMPLETE: [f64; 19] = [
    0.1269, 0.379, 0.3914, 0.4875, 1.0301, 1.0602, 1.9528, 2.1245, 2.2643, 4.4381, 5.7654, 5.8109,
    6.4707, 6.603, 9.0225, 15.5262, 18.0596, 53.3229, 76.9902,
];
const AVERAGE: [f64; 19] = [
    0.1269,
    0.379,
    0.3914,
    0.4875,
    1.0301,
    1.0602,
    1.5698,
    1.6743,
    1.9187,
    2.9153,
    3.8686,
    4.26268333333,
    5.02485,
    5.20895,
    6.272225,
    6.43586666667,
    8.18178333333,
    34.7421785714,
    38.61122,
];
const CENTROID: [f64; 19] = [
    0.1269,
    0.379,
    0.3914,
    0.4875,
    1.0301,
    1.0602,
    1.47195,
    1.57955,
    1.65365,
    2.37112222222,
    3.45442222222,
    3.748475,
    3.701141,
    4.5530375,
    5.41950625,
    6.20598367347,
    9.04783055556,
    30.06640625,
    27.2537728938,
];
const MEDIAN: [f64; 19] = [
    0.1269,
    0.379,
    0.3914,
    0.4875,
    1.0301,
    1.0602,
    1.47195,
    1.57955,
    1.65365,
    2.1891625,
    3.3778125,
    3.89167812500,
    4.1099375,
    4.54502187500,
    5.51404062500,
    7.81449453125,
    8.20279921875,
    23.2195271484,
    31.5034652832,
];
const WARD: [f64; 19] = [
    0.06345,
    0.25295,
    0.44865,
    0.6924,
    1.20745,
    1.73755,
    2.71885,
    3.77188333333,
    4.86913333333,
    6.65265833333,
    9.243475,
    12.4216916667,
    15.873175,
    20.20878,
    24.70695,
    32.2463133333,
    42.62083,
    138.1632,
    257.070395,
];

const DISTANCES: [f64; 20] = [
    0.830876795921,
    2.17978353283,
    0.865393696534,
    1.28512888459,
    1.70440788839,
    1.32755649597,
    1.9286799242,
    0.960237600805,
    2.57825245356,
    1.90702835397,
    1.14190317328,
    1.27339254412,
    1.35466390138,
    1.08365782686,
    1.22627775926,
    1.86238664085,
    0.690756107465,
    1.01743992452,
    1.14196497319,
    1.20895988354,
];

#[test]
fn every_stage_of_the_agglomeration_schedules_agrees_with_hclust() {
    for (method, want) in [
        ("SINGLE", SINGLE),
        ("COMPLETE", COMPLETE),
        ("BAVERAGE", AVERAGE),
        ("CENTROID", CENTROID),
        ("MEDIAN", MEDIAN),
        ("WARD", WARD),
    ] {
        let t = run(&format!(
            "CLUSTER x y z /METHOD={method} /ID=id /PRINT=SCHEDULE /SAVE=CLUSTER(3)."
        ));
        let schedule = table(&t, "Agglomeration Schedule");
        assert_eq!(schedule.rows.len(), 19, "{method}");
        for (stage, (row, want)) in schedule.rows.iter().zip(want).enumerate() {
            near(
                number(&row[3]),
                want,
                &format!("{method} stage {}", stage + 1),
            );
        }
    }
}

#[test]
fn kmeans_from_given_centers_agrees_with_r() {
    let t = run(
        "QUICK CLUSTER x y z /INITIAL=(-0.2 -0.3 0.4 3.2 3.4 2.7 0.5 4.2 -2.9) \
                 /CRITERIA=CLUSTER(3) MXITER(100) /PRINT=INITIAL CLUSTER.",
    );
    let centers = table(&t, "Final Cluster Centers");
    let expected = [
        [-0.035, 3.26142857143, 0.524],
        [-0.335, 3.37571428571, 4.178],
        [0.3425, 2.67285714286, -2.878],
    ];
    for (row, want) in centers.rows.iter().zip(expected) {
        for (j, want) in want.iter().enumerate() {
            near(number(&row[1 + j]), *want, "center");
        }
    }
    let sizes = table(&t, "Number of Cases in each Cluster");
    let counts: Vec<f64> = sizes.rows.iter().take(3).map(|r| number(&r[2])).collect();
    assert_eq!(counts, [8.0, 7.0, 5.0]);
    let membership = table(&t, "Cluster Membership");
    let clusters: Vec<f64> = membership.rows.iter().map(|r| number(&r[1])).collect();
    // cases 1-8, 9-15 and 16-20 form the three clusters
    let want: Vec<f64> = (1..=20)
        .map(|case| match case {
            1..=8 => 1.0,
            9..=15 => 2.0,
            _ => 3.0,
        })
        .collect();
    assert_eq!(clusters, want);
    let distances: Vec<f64> = membership.rows.iter().map(|r| number(&r[2])).collect();
    for (i, (got, want)) in distances.iter().zip(DISTANCES).enumerate() {
        near(*got, want, &format!("distance of case {}", i + 1));
    }
}
