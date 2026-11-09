//! RENAME VARIABLES and AGGREGATE move variables through temporary names of the form
//! `@sc.<purpose>.<n>`. Users may give their own variables such names, so the temporary ones
//! must avoid every name in use or requested, in any case, and never take over a user variable.

use statscraft_data::{Measure, MissingValues};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in commands {
        engine.execute(&command.command).unwrap();
    }
}

fn survey() -> Engine {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "GET FILE='{}/../../tests/fixtures/survey_basic.sav'.",
            env!("CARGO_MANIFEST_DIR")
        ),
    );
    engine
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let variable = engine.dataset().unwrap().variable(name).unwrap();
    variable.numeric().unwrap().iter().collect()
}

#[test]
fn rename_preserves_occupied_temporary_names_and_swaps() {
    for occupied in ["@sc.renaming.0", "@SC.RENAMING.0"] {
        let mut engine = survey();
        run(
            &mut engine,
            &format!(
                "COMPUTE {occupied}=99.\nCOMPUTE @sc.renaming.1=88.\n\
                 VARIABLE LABELS {occupied} 'Keep me'.\nRENAME VARIABLES (resp_id=renamed)."
            ),
        );
        assert_eq!(column(&engine, occupied), vec![Some(99.0); 200]);
        assert_eq!(column(&engine, "@sc.renaming.1"), vec![Some(88.0); 200]);
        assert_eq!(
            column(&engine, "renamed")[..2],
            [Some(1001.0), Some(1002.0)]
        );

        run(
            &mut engine,
            &format!("RENAME VARIABLES (renamed {occupied}={occupied} renamed)."),
        );
        assert_eq!(column(&engine, "renamed"), vec![Some(99.0); 200]);
        assert_eq!(column(&engine, occupied)[..2], [Some(1001.0), Some(1002.0)]);
        let data = engine.dataset().unwrap();
        assert_eq!(data.variable("renamed").unwrap().label(), Some("Keep me"));
        assert_eq!(data.variable_count(), 21);
    }
}

#[test]
fn rename_to_names_of_the_temporary_form() {
    let mut engine = survey();
    run(
        &mut engine,
        "RENAME VARIABLES (resp_id gender=@SC.RENAMING.1 @sc.renaming.0).",
    );
    assert_eq!(
        column(&engine, "@sc.renaming.1")[..2],
        [Some(1001.0), Some(1002.0)]
    );
    assert_eq!(
        column(&engine, "@sc.renaming.0")[..4],
        [Some(1.0), Some(1.0), Some(2.0), Some(3.0)]
    );
    assert_eq!(engine.dataset().unwrap().variable_count(), 19);
}

#[test]
fn rename_collision_errors_leave_original_variables_intact() {
    let mut engine = survey();
    run(
        &mut engine,
        "COMPUTE @SC.RENAMING.0=99.\nVARIABLE LABELS @SC.RENAMING.0 'Keep me'.",
    );
    let (commands, errors) = parse("RENAME VARIABLES (resp_id gender=renamed age).");
    assert!(errors.is_empty());
    assert!(engine.execute(&commands[0].command).is_err());
    assert_eq!(
        column(&engine, "resp_id")[..2],
        [Some(1001.0), Some(1002.0)]
    );
    assert_eq!(
        column(&engine, "gender")[..4],
        [Some(1.0), Some(1.0), Some(2.0), Some(3.0)]
    );
    assert_eq!(column(&engine, "@sc.renaming.0"), vec![Some(99.0); 200]);
    let data = engine.dataset().unwrap();
    assert_eq!(
        data.variable("@sc.renaming.0").unwrap().label(),
        Some("Keep me")
    );
    assert!(data.variable_index("renamed").is_none());
    assert_eq!(data.variable_count(), 20);
}

#[test]
fn aggregate_keeps_a_break_variable_with_a_temporary_name() {
    for occupied in ["@sc.aggregating.0", "@SC.AGGREGATING.0"] {
        let mut engine = survey();
        run(
            &mut engine,
            &format!(
                "COMPUTE {occupied}=1.\nVARIABLE LABELS {occupied} 'Break label'.\n\
                 AGGREGATE /OUTFILE=* /BREAK={occupied} /n=N."
            ),
        );
        assert_eq!(column(&engine, occupied), [Some(1.0)]);
        assert_eq!(column(&engine, "n"), [Some(200.0)]);
        let data = engine.dataset().unwrap();
        assert_eq!(
            data.variable(occupied).unwrap().label(),
            Some("Break label")
        );
        assert_eq!(data.variable_count(), 2);
    }
}

#[test]
fn aggregate_targets_may_have_names_of_the_temporary_form() {
    let mut engine = survey();
    run(
        &mut engine,
        "AGGREGATE /OUTFILE=* /@SC.AGGREGATING.1=N /@sc.aggregating.0=MAX(resp_id).",
    );
    assert_eq!(column(&engine, "@sc.aggregating.1"), [Some(200.0)]);
    assert_eq!(column(&engine, "@sc.aggregating.0"), [Some(1200.0)]);
    assert_eq!(engine.dataset().unwrap().variable_count(), 2);
}

#[test]
fn aggregate_targets_take_nothing_from_a_dropped_variable_with_a_temporary_name() {
    for occupied in ["@sc.aggregating.0", "@SC.AGGREGATING.0"] {
        let mut engine = survey();
        run(
            &mut engine,
            &format!(
                "COMPUTE {occupied}=2.\nVARIABLE LABELS {occupied} 'Old weight'.\n\
                 VALUE LABELS {occupied} 2 'Double'.\nMISSING VALUES {occupied} (99).\n\
                 VARIABLE LEVEL {occupied} (NOMINAL).\nWEIGHT BY {occupied}.\n\
                 AGGREGATE /OUTFILE=* /n=N."
            ),
        );
        // N counts weighted cases: 200 cases of weight 2
        assert_eq!(column(&engine, "n"), [Some(400.0)]);
        let data = engine.dataset().unwrap();
        assert_eq!(data.weight_variable(), None);
        assert_eq!(data.variable_count(), 1);
        let n = data.variable("n").unwrap();
        assert_eq!(n.label(), None);
        assert!(n.value_labels().is_empty());
        assert_eq!(n.missing_values(), &MissingValues::None);
        // a new numeric variable has no measurement level until one is set
        assert_eq!(n.measure(), Measure::Unknown);
    }
}
