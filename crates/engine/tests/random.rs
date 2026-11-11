//! SET SEED, RNG and MTINDEX, and the random functions UNIFORM, NORMAL and RV.*.

use statscraft_engine::session::Session;

const DATA: &str = "\
DATA LIST LIST /id g.
BEGIN DATA
1 1
2 2
3 1
4 2
5 1
END DATA.
";

fn session() -> Session {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    session
}

fn values(session: &Session, name: &str) -> Vec<Option<f64>> {
    let data = session.dataset().unwrap();
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn run(session: &mut Session, source: &str) {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn the_same_seed_gives_the_same_numbers() {
    let mut s = session();
    run(
        &mut s,
        "SET SEED=12345.\nCOMPUTE a = UNIFORM(1).\nSET SEED 12345.\nCOMPUTE b = UNIFORM(1).\n\
         COMPUTE c = UNIFORM(1).",
    );
    let (a, b, c) = (values(&s, "a"), values(&s, "b"), values(&s, "c"));
    assert_eq!(a, b);
    assert_ne!(b, c);
    // the congruential generator of the documentation: seed = 397204094 * seed mod (2^31 - 1)
    let mut seed = 12_345u64;
    let expected: Vec<Option<f64>> = (0..5)
        .map(|_| {
            seed = 397_204_094 * seed % 2_147_483_647;
            Some(seed as f64 / 2_147_483_648.0)
        })
        .collect();
    assert_eq!(a, expected);
}

#[test]
fn a_new_session_starts_from_the_default_seed() {
    let mut first = session();
    run(&mut first, "COMPUTE x = RV.NORMAL(10, 2).");
    let mut second = session();
    run(
        &mut second,
        "SET SEED=2000000.\nCOMPUTE x = RV.NORMAL(10, 2).",
    );
    assert_eq!(values(&first, "x"), values(&second, "x"));
}

#[test]
fn the_mersenne_twister_restarts_from_its_index() {
    let mut s = session();
    run(
        &mut s,
        "SET RNG=MT MTINDEX=-12345.678.\nCOMPUTE a = RV.UNIFORM(5, 6).\n\
         SET MTINDEX=-12345.678.\nCOMPUTE b = RV.UNIFORM(5, 6).\nSET RNG=MC.\n\
         COMPUTE c = RV.UNIFORM(5, 6).",
    );
    let (a, b, c) = (values(&s, "a"), values(&s, "b"), values(&s, "c"));
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.iter().all(|v| (5.0..6.0).contains(&v.unwrap())));
}

#[test]
fn random_numbers_are_drawn_only_where_the_value_is_used() {
    let mut s = session();
    run(
        &mut s,
        "SET SEED=99.\nDO IF g = 1.\nCOMPUTE r = UNIFORM(10).\nEND IF.\nSET SEED=99.\n\
         COMPUTE every = UNIFORM(10).\nCOMPUTE bad = RV.NORMAL(0, -1).",
    );
    let (r, all) = (values(&s, "r"), values(&s, "every"));
    // cases 1, 3 and 5 got the first three numbers of the sequence
    assert_eq!([r[0], r[2], r[4]], [all[0], all[1], all[2]]);
    assert_eq!((r[1], r[3]), (None, None));
    // a parameter out of range gives system-missing
    assert!(values(&s, "bad").iter().all(Option::is_none));
}

#[test]
fn the_case_by_case_pass_draws_a_new_number_for_every_case() {
    let mut s = session();
    // LAG makes the pass run case by case
    run(&mut s, "COMPUTE r = UNIFORM(1) + 0 * LAG(id).\nEXECUTE.");
    let r = values(&s, "r");
    let mut distinct = r.clone();
    distinct.dedup();
    assert_eq!(distinct.len(), 5, "{r:?}");
}

#[test]
fn sample_follows_set_seed() {
    let kept = |seed: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!("SET SEED={seed}.\nSAMPLE 2 FROM 5.\nEXECUTE."),
        );
        values(&s, "id")
    };
    assert_eq!(kept("777"), kept("777"));
    let seeds = ["1", "2", "3", "4", "5", "6", "7", "8"];
    let samples: Vec<_> = seeds.iter().map(|s| kept(s)).collect();
    assert!(samples.iter().any(|s| *s != samples[0]));
}

#[test]
fn bad_seeds_are_errors_and_show_seed_shows_the_state() {
    let mut s = session();
    for bad in [
        "SET SEED=0.",
        "SET SEED=2000000000.",
        "SET RNG=XY.",
        "SET MTINDEX=abc.",
    ] {
        assert!(s.run(bad, None).is_err(), "{bad}");
    }
    run(&mut s, "SET SEED=RANDOM.");
    let result = s.run("SET SEED=5.\nSHOW SEED.", None).unwrap();
    let table = &result.commands[1].tables[0];
    assert_eq!(table.rows[0][1], statscraft_engine::Cell::Text("5".into()));
}
