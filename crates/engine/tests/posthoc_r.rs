//! ONEWAY post hoc comparisons against R on 30 invented values in four unequal groups (7, 9,
//! 6 and 8 cases): LSD, Bonferroni and Sidak from the pooled-variance t quantiles, Tukey's HSD
//! (Tukey-Kramer) from `ptukey` and `qtukey`, Scheffe from the F distribution, and
//! Games-Howell from `ptukey` and `qtukey` with Welch's degrees of freedom: mean difference,
//! standard error, significance and the interval of every pair.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y g.
BEGIN DATA
10.4 1
11 1
10.2 1
10.9 1
9.3 1
10.2 1
8.3 1
13.5 2
10.9 2
8.1 2
9.8 2
14.8 2
14.3 2
4.5 2
2.9 2
12 2
13.4 3
8 3
17 3
16.1 3
19.4 3
18 3
10.9 4
10.1 4
11.6 4
12.4 4
12.7 4
9.6 4
7.6 4
14.5 4
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

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => s.clone(),
        _ => String::new(),
    }
}

/// For the pair (i, j) with i < j: the mean difference, the pooled and the separate standard
/// errors, and for LSD, Bonferroni, Sidak, Tukey, Scheffe and Games-Howell the significance and
/// the lower and upper bound.
fn expected() -> Vec<((u32, u32), [f64; 21])> {
    vec![
        (
            (1, 2),
            [
                -0.0460317460317,
                1.60685152396,
                1.4535178567,
                9.773647550703e-01,
                -3.34896235707,
                3.25689886501,
                1.000000000000e+00,
                -4.63442997324,
                4.54236648117,
                9.999999998655e-01,
                -4.62009893088,
                4.52803543882,
                9.999913180605e-01,
                -4.45413944108,
                4.36207594901,
                9.999935707519e-01,
                -4.84658286919,
                4.75451937712,
                9.999876792832e-01,
                -4.58125065872,
                4.48918716666,
            ],
        ),
        (
            (1, 3),
            [
                -5.27380952381,
                1.77391605092,
                1.71656363207,
                6.283900926262e-03,
                -8.92014618815,
                -1.62747285947,
                3.770340555757e-02,
                -10.3392640116,
                -0.208355036039,
                3.711603376425e-02,
                -10.3234429692,
                -0.224176078421,
                3.002472838087e-02,
                -10.1402286533,
                -0.407390394352,
                5.149862323683e-02,
                -10.5734744671,
                0.0258554194758,
                8.645821539838e-02,
                -11.4061160206,
                0.858496972984,
            ],
        ),
        (
            (1, 4),
            [
                -1.13214285714,
                1.65020404919,
                0.832142857143,
                4.987465734835e-01,
                -4.52418586003,
                2.25990014574,
                1.000000000000e+00,
                -5.84433512938,
                3.58004941509,
                9.841385047102e-01,
                -5.82961743843,
                3.56533172414,
                9.014698963803e-01,
                -5.65918039556,
                3.39489468127,
                9.242859937655e-01,
                -6.06221186703,
                3.79792615275,
                5.489830408305e-01,
                -3.67970692226,
                1.41542120797,
            ],
        ),
        (
            (2, 3),
            [
                -5.22777777778,
                1.68048554835,
                2.19119731631,
                4.489148968056e-03,
                -8.68206529362,
                -1.77349026194,
                2.693489380834e-02,
                -10.0264395059,
                -0.429116049624,
                2.663441019834e-02,
                -10.0114517431,
                -0.444103812472,
                2.187316081432e-02,
                -9.83788715415,
                -0.617668401401,
                3.880206669568e-02,
                -10.2483143154,
                -0.207241240182,
                1.371448699957e-01,
                -11.8140434899,
                1.3584879343,
            ],
        ),
        (
            (2, 4),
            [
                -1.08611111111,
                1.54933112228,
                1.59600022267,
                4.895180020063e-01,
                -4.27080684317,
                2.09858462094,
                1.000000000000e+00,
                -5.51025862493,
                3.33803640271,
                9.823036952586e-01,
                -5.49644059034,
                3.32421836812,
                8.957398375776e-01,
                -5.33642194596,
                3.16419972374,
                9.197600714902e-01,
                -5.71481709189,
                3.54259486967,
                9.025190934682e-01,
                -5.81999237857,
                3.64777015635,
            ],
        ),
        (
            (3, 4),
            [
                4.14166666667,
                1.72198526768,
                1.83877493293,
                2.357768789812e-02,
                0.60207525604,
                7.68125807729,
                1.414661273887e-01,
                -0.775498380422,
                9.05883171376,
                1.333850651481e-01,
                -0.760140493622,
                9.04347382696,
                1.010275240896e-01,
                -0.582289713875,
                8.86562304721,
                1.497838371944e-01,
                -1.00285240528,
                9.28618573861,
                1.988743526577e-01,
                -1.94500764649,
                10.2283409798,
            ],
        ),
    ]
}

#[test]
fn the_pairwise_comparisons_agree_with_r() {
    let t = run("ONEWAY y BY g /POSTHOC=LSD BONFERRONI SIDAK TUKEY SCHEFFE GH.");
    let table = t
        .iter()
        .find(|t| t.title.starts_with("Multiple Comparisons"))
        .expect("the table");
    // each test's tolerance: R's ptukey is good to about 1e-8 for fractional degrees of freedom
    let tests = [
        ("LSD", 0usize, 1e-9),
        ("Bonferroni", 1, 1e-9),
        ("Sidak", 2, 1e-9),
        ("Tukey HSD", 3, 1e-8),
        ("Scheffe", 4, 1e-9),
        ("Games-Howell", 5, 1e-6),
    ];
    let mut test = String::new();
    let mut first = 0;
    let mut seen = 0;
    for row in &table.rows {
        if !text(&row[0]).is_empty() {
            test = text(&row[0]);
        }
        if !text(&row[1]).is_empty() {
            first = text(&row[1]).parse::<u32>().expect("a group");
        }
        let second: u32 = text(&row[2]).parse().expect("a group");
        if first >= second {
            continue;
        }
        let &(_, at, tolerance) = tests
            .iter()
            .find(|(name, _, _)| *name == test)
            .unwrap_or_else(|| panic!("unexpected test {test}"));
        let (_, values) = expected()
            .into_iter()
            .find(|(pair, _)| *pair == (first, second))
            .expect("a pair");
        let what = format!("{test} {first}-{second}");
        near(number(&row[3]), values[0], 1e-9, &what);
        let se = if at == 5 { values[2] } else { values[1] };
        near(number(&row[4]), se, 1e-9, &what);
        near(number(&row[5]), values[3 + 3 * at], tolerance, &what);
        near(number(&row[6]), values[4 + 3 * at], tolerance, &what);
        near(number(&row[7]), values[5 + 3 * at], tolerance, &what);
        seen += 1;
    }
    assert_eq!(seen, 36, "six pairs for each of six tests");
}
