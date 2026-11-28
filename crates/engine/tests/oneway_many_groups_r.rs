//! ONEWAY post hoc tests over many groups: the pairs are worked out on several threads, and the
//! table is what one thread gives. 40 invented groups of 3 cases (780 pairs, so the pairs are
//! shared among threads) against R's TukeyHSD: the mean difference, the significance and the
//! interval of eight pairs. Script in lab/oracles/edge-cl (tukey40.R).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y.
BEGIN DATA
1 -0.84
1 1.02
1 0.95
2 -0.12
2 0.66
2 -0.54
3 -0.12
3 4.87
3 -0.75
4 -1.53
4 1.36
4 1.81
5 -1.27
5 -4.17
5 -0.88
6 0.65
6 1.89
6 -3.06
7 1.64
7 1.26
7 0.92
8 1.87
8 -0.1
8 0.31
9 1.33
9 3.14
9 1.28
10 1.82
10 3.01
10 0.75
11 4.64
11 2.08
11 -1.23
12 2.79
12 3.19
12 4.77
13 2.27
13 2.87
13 3.33
14 3.88
14 7.76
14 1.72
15 1.06
15 2.16
15 1.29
16 3.07
16 2.16
16 3.84
17 0.53
17 5.24
17 1.49
18 2.1
18 3.56
18 2.08
19 4.81
19 3.6
19 4.83
20 6.21
20 4.41
20 1.37
21 1.74
21 4.55
21 2.44
22 5.76
22 4.29
22 2.66
23 4.16
23 4.67
23 3.05
24 2.65
24 2.8
24 2.67
25 6.58
25 4.76
25 6.47
26 4.74
26 6.64
26 4.42
27 7.47
27 7.4
27 7.42
28 6.57
28 6.11
28 4.96
29 6.18
29 4.87
29 7.35
30 3.81
30 5.92
30 3.14
31 4.33
31 4.69
31 6.39
32 6.43
32 4.2
32 7.78
33 5.11
33 6.34
33 7.9
34 6.38
34 6.24
34 7.45
35 9.8
35 7.4
35 8.62
36 7.65
36 7.86
36 7.33
37 9.87
37 7.38
37 9.34
38 7.93
38 7.08
38 6.25
39 5.38
39 6.26
39 5.61
40 4.24
40 10.47
40 8.84
END DATA.
";

fn tables(command: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{command}\n"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut out = Vec::new();
    for command in &commands {
        out.extend(engine.execute(&command.command).expect("runs"));
    }
    out
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn tukey_over_forty_groups_agrees_with_r() {
    let all = tables("ONEWAY y BY g /POSTHOC=TUKEY.");
    let table = all
        .iter()
        .find(|t| t.title.starts_with("Multiple Comparisons"))
        .expect("Multiple Comparisons");
    assert_eq!(table.rows.len(), 40 * 39);
    let want = [
        (
            12,
            1,
            3.20666666667,
            0.845371499791,
            -1.88152246607,
            8.2948557994,
        ),
        (
            32,
            3,
            4.80333333333,
            0.0947444221874,
            -0.284855799403,
            9.89152246607,
        ),
        (
            19,
            6,
            4.58666666667,
            0.147837520351,
            -0.501522466069,
            9.6748557994,
        ),
        (
            34,
            8,
            5.99666666667,
            0.00469136263207,
            0.908477533931,
            11.0848557994,
        ),
        (36, 12, 4.03, 0.382231328504, -1.05818913274, 9.11818913274),
        (36, 16, 4.59, 0.146871939532, -0.498189132736, 9.67818913274),
        (
            29,
            21,
            3.22333333333,
            0.838175001924,
            -1.8648557994,
            8.31152246607,
        ),
        (
            39,
            37,
            -3.11333333333,
            0.882278365729,
            -8.20152246607,
            1.9748557994,
        ),
    ];
    for (i, j, diff, p, lower, upper) in want {
        // the first group is named on its first row only
        let mut current = 0.0;
        let row = table
            .rows
            .iter()
            .find(|r| {
                let level = |cell: &Cell| match cell {
                    Cell::Text(t) => t.trim().parse::<f64>().ok(),
                    _ => None,
                };
                if let Some(first) = level(&r[1]) {
                    current = first;
                }
                current == i as f64 && level(&r[2]) == Some(j as f64)
            })
            .unwrap_or_else(|| panic!("no pair {i} {j}"));
        let got = [
            number(&row[3]),
            number(&row[5]),
            number(&row[6]),
            number(&row[7]),
        ];
        for (g, w, what) in [
            (got[0], diff, "difference"),
            (got[1], p, "significance"),
            (got[2], lower, "lower bound"),
            (got[3], upper, "upper bound"),
        ] {
            assert!(
                (g - w).abs() <= 1e-6 * w.abs().max(1.0),
                "{i}-{j} {what}: {g} is not {w}"
            );
        }
    }
}
