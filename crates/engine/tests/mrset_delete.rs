//! A multiple response set needs at least two variables, so deleting or dropping variables from
//! a set takes the set away once it has fewer; a set that keeps two or more stays.

use statscraft_engine::session::Session;

fn session() -> Session {
    let mut session = Session::default();
    session
        .run(
            "DATA LIST LIST /a1 a2 a3 b1 b2 c.\nBEGIN DATA\n1 0 1 1 1 5\n0 1 1 0 1 6\nEND DATA.\n\
             MRSETS /MDGROUP NAME=$a VARIABLES=a1 a2 a3 VALUE=1\n\
             /MDGROUP NAME=$b VARIABLES=b1 b2 VALUE=1.",
            None,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

fn sets(session: &Session) -> Vec<(String, Vec<String>)> {
    session
        .dataset()
        .unwrap()
        .multiple_response_sets()
        .iter()
        .map(|set| (set.name.clone(), set.variables.clone()))
        .collect()
}

fn run(session: &mut Session, source: &str) {
    session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

#[test]
fn delete_variables_takes_a_set_that_is_left_with_one_variable() {
    let mut s = session();
    run(&mut s, "DELETE VARIABLES b1.");
    assert_eq!(sets(&s), [("$a".into(), names(&["a1", "a2", "a3"]))]);
    run(&mut s, "DELETE VARIABLES a1.");
    assert_eq!(sets(&s), [("$a".into(), names(&["a2", "a3"]))]);
    run(&mut s, "DELETE VARIABLES a2.");
    assert!(sets(&s).is_empty());
    // the variables themselves went as asked
    let data = s.dataset().unwrap();
    assert!(data.variable("a3").is_ok() && data.variable("a1").is_err());
}

#[test]
fn deleting_a_whole_set_or_none_of_it() {
    let mut s = session();
    run(&mut s, "DELETE VARIABLES a1 TO a3.");
    assert_eq!(sets(&s), [("$b".into(), names(&["b1", "b2"]))]);
    run(&mut s, "DELETE VARIABLES c.");
    assert_eq!(sets(&s), [("$b".into(), names(&["b1", "b2"]))]);
}

#[test]
fn saving_with_keep_or_drop_follows_the_same_rule() {
    let mut s = session();
    run(&mut s, "MATCH FILES /FILE=* /KEEP=a1 a2 b1 c.");
    // $a keeps two, $b keeps one
    assert_eq!(sets(&s), [("$a".into(), names(&["a1", "a2"]))]);
    let mut s = session();
    run(&mut s, "MATCH FILES /FILE=* /DROP=a3 b2.");
    assert_eq!(sets(&s), [("$a".into(), names(&["a1", "a2"]))]);
}

#[test]
fn undo_brings_the_set_back_with_its_variables() {
    let mut s = session();
    let before = sets(&s);
    run(&mut s, "DELETE VARIABLES a1 a2.");
    assert_eq!(sets(&s), [("$b".into(), names(&["b1", "b2"]))]);
    assert!(s.undo());
    assert_eq!(sets(&s), before);
}

#[test]
fn a_deletion_that_touches_no_set_leaves_them_as_they_are() {
    let mut s = session();
    let before = sets(&s);
    run(&mut s, "DELETE VARIABLES c.");
    assert_eq!(sets(&s), before);
}
