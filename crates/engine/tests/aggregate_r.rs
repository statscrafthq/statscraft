//! AGGREGATE functions against R on 30 invented cases in three groups with two missing values
//! and case weights: N, SUM, MEAN, SD, MEDIAN, MIN, MAX, the valid count, NMISS, the fraction,
//! percentage and count inside bounds (FGT, PGT, PIN, CIN) and FIRST and LAST, once as they
//! are and once with WEIGHT BY, where R is run on the cases written out as often as their
//! weight (N, SUM, MEAN, SD, MEDIAN, the counts and the percentages are weighted; NU, the
//! unweighted count of valid cases, and FIRST and LAST are not).

use statscraft_engine::Engine;
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g w x.
BEGIN DATA
1 3 8.8
1 3 9.9
1 3 11.8
1 1 .
1 3 8.7
1 3 14.1
1 1 11.1
1 2 8.8
1 3 13.7
2 3 13.1
2 1 10.4
2 2 8.9
2 2 14.6
2 3 11.7
2 1 12.5
2 2 14.1
2 1 .
2 2 14.1
2 3 16.5
2 1 11.4
3 2 12
3 1 11.7
3 3 10.4
3 3 11.3
3 1 8.4
3 1 14.4
3 2 12.4
3 3 14.2
3 2 10.3
3 3 11.5
END DATA.
";

const AGGREGATE: &str = "AGGREGATE /OUTFILE=* /BREAK=g /n=N /s=SUM(x) /m=MEAN(x) /sd=SD(x) \
    /med=MEDIAN(x) /mn=MIN(x) /mx=MAX(x) /nv=N(x) /nm=NMISS(x) /fgt=FGT(x,10) /pgt=PGT(x,10) \
    /pin=PIN(x,8,12) /cin=CIN(x,8,12) /f=FIRST(x) /l=LAST(x) /nu=NU(x).";

const TARGETS: [&str; 15] = [
    "n", "s", "m", "sd", "med", "mn", "mx", "nv", "nm", "fgt", "pgt", "pin", "cin", "f", "l",
];

fn aggregated(weighted: bool) -> Engine {
    let weight = if weighted { "WEIGHT BY w.\n" } else { "" };
    let (commands, errors) = parse(&format!("{DATA}{weight}{AGGREGATE}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        engine.execute(&command.command).expect("runs");
    }
    engine
}

fn column(engine: &Engine, name: &str) -> Vec<f64> {
    engine
        .dataset()
        .unwrap()
        .variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .map(|v| v.unwrap_or_else(|| panic!("{name} is missing")))
        .collect()
}

fn check(engine: &Engine, expected: [[f64; 15]; 3], what: &str) {
    for (i, target) in TARGETS.iter().enumerate() {
        let got = column(engine, target);
        assert_eq!(got.len(), 3, "{what} {target}");
        for (group, row) in expected.iter().enumerate() {
            assert!(
                (got[group] - row[i]).abs() <= 1e-9 * row[i].abs().max(1.0),
                "{what} {target} of group {}: {} vs {}",
                group + 1,
                got[group],
                row[i]
            );
        }
    }
}

#[test]
fn unweighted_aggregates_agree_with_r() {
    let engine = aggregated(false);
    check(
        &engine,
        [
            [
                9.0,
                86.9,
                10.8625,
                2.19019731662,
                10.5,
                8.7,
                14.1,
                8.0,
                1.0,
                0.5,
                50.0,
                75.0,
                6.0,
                8.8,
                13.7,
            ],
            [
                11.0,
                127.3,
                12.73,
                2.22563349284,
                12.8,
                8.9,
                16.5,
                10.0,
                1.0,
                0.9,
                90.0,
                40.0,
                4.0,
                13.1,
                11.4,
            ],
            [
                10.0,
                116.6,
                11.66,
                1.79022034646,
                11.6,
                8.4,
                14.4,
                10.0,
                0.0,
                0.9,
                90.0,
                70.0,
                7.0,
                12.0,
                11.5,
            ],
        ],
        "unweighted",
    );
    // NU: the valid cases, unweighted
    assert_eq!(column(&engine, "nu"), [8.0, 10.0, 10.0]);
}

#[test]
fn weighted_aggregates_agree_with_r_on_the_repeated_cases() {
    let engine = aggregated(true);
    check(
        &engine,
        [
            [
                22.0,
                229.7,
                10.9380952381,
                2.19373567015,
                9.9,
                8.7,
                14.1,
                21.0,
                1.0,
                0.47619047619,
                47.619047619,
                71.4285714286,
                15.0,
                8.8,
                13.7,
            ],
            [
                21.0,
                261.6,
                13.08,
                2.23456106605,
                13.1,
                8.9,
                16.5,
                20.0,
                1.0,
                0.9,
                90.0,
                35.0,
                7.0,
                13.1,
                11.4,
            ],
            [
                21.0,
                246.1,
                11.719047619,
                1.54389735657,
                11.5,
                8.4,
                14.4,
                21.0,
                0.0,
                0.952380952381,
                95.2380952381,
                71.4285714286,
                15.0,
                12.0,
                11.5,
            ],
        ],
        "weighted",
    );
    assert_eq!(column(&engine, "nu"), [8.0, 10.0, 10.0]);
}
