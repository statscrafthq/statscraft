//! The lookup of a variable by name against a plain scan, through every change of the names:
//! adding, renaming, deleting and keeping in another order, with names that differ in case and
//! with letters outside ASCII.

use statscraft_data::Dataset;

/// What the lookup must give: the first variable whose name is the same ignoring case.
fn scan(data: &Dataset, name: &str) -> Option<usize> {
    let lower = |s: &str| s.chars().flat_map(char::to_lowercase).collect::<String>();
    let wanted = lower(name);
    data.variable_names().position(|n| lower(n) == wanted)
}

fn agrees(data: &Dataset, probes: &[&str]) {
    for name in data.variable_names().chain(probes.iter().copied()) {
        assert_eq!(data.variable_index(name), scan(data, name), "{name}");
        assert_eq!(
            data.variable_index(&name.to_uppercase()),
            scan(data, &name.to_uppercase()),
            "{name} upper"
        );
    }
}

fn base() -> Dataset {
    let mut data = Dataset::new(3);
    for name in ["id", "Õie", "Score", "a_b", "x1"] {
        data.set_numeric_values(name, vec![Some(1.0), None, Some(3.0)])
            .unwrap();
    }
    data
}

#[test]
fn lookups_follow_every_change_of_the_names() {
    let probes = ["nope", "ID", "õie", "ÕIE", "score", "SCORE", "ab", ""];
    let mut data = base();
    agrees(&data, &probes);
    // added at the end, and found at once
    data.set_numeric_values("Zeta", vec![None, None, None])
        .unwrap();
    assert_eq!(data.variable_index("zeta"), Some(5));
    agrees(&data, &probes);
    // renamed: the old name is gone, the new one is there in any case
    data.rename_variable("Score", "Punktid").unwrap();
    assert_eq!(data.variable_index("score"), None);
    assert_eq!(data.variable_index("PUNKTID"), Some(2));
    agrees(&data, &probes);
    data.rename_variable("x1", "X1").unwrap();
    agrees(&data, &probes);
    // a name that is taken is refused and nothing moves
    assert!(data.rename_variable("id", "õie").is_err());
    agrees(&data, &probes);
    // deleted: the positions after it move
    data.delete_variables(&["Õie"]).unwrap();
    assert_eq!(data.variable_index("õie"), None);
    assert_eq!(data.variable_index("a_b"), Some(2));
    agrees(&data, &probes);
    // kept in another order
    data.keep_variables(&["Zeta", "a_b", "id"]).unwrap();
    assert_eq!(data.variable_index("id"), Some(2));
    assert_eq!(data.variable_index("zeta"), Some(0));
    agrees(&data, &probes);
    // a copy has its own names
    let mut copy = data.clone();
    copy.rename_variable("id", "key").unwrap();
    assert_eq!(copy.variable_index("key"), Some(2));
    assert_eq!(data.variable_index("key"), None);
    assert_eq!(data.variable_index("id"), Some(2));
    agrees(&copy, &probes);
    agrees(&data, &probes);
}

#[test]
fn many_variables_are_added_and_renamed_with_lookups_between() {
    let mut data = Dataset::new(1);
    for i in 0..400 {
        data.set_numeric_values(&format!("v{i}"), vec![Some(i as f64)])
            .unwrap();
        // a lookup after every addition, as a command that builds variables one by one makes
        assert_eq!(data.variable_index(&format!("V{i}")), Some(i));
    }
    for i in (0..400).step_by(3) {
        data.rename_variable(&format!("v{i}"), &format!("w{i}"))
            .unwrap();
        assert_eq!(data.variable_index(&format!("v{i}")), None);
        assert_eq!(data.variable_index(&format!("W{i}")), Some(i));
    }
    let names: Vec<&str> = data.variable_names().collect();
    agrees(&data, &["v0", "w1", "q"]);
    assert_eq!(names.len(), 400);
}
