//! Groups with nothing to compare: every group one case (no error degrees of freedom) and a
//! factor with one level. The post hoc tests used to loop for ever on the studentized range
//! without degrees of freedom, and the Levene tests of the spread-versus-level plot printed
//! NaN for one group.

use statscraft_engine::{Engine, Table};
use statscraft_syntax::parse;

const THREE_CASES: &str = "DATA LIST FREE /g y.
BEGIN DATA
1 4.5
2 6.1
3 5.2
END DATA.
";

const ONE_GROUP: &str = "DATA LIST FREE /g y.
BEGIN DATA
1 4.5
1 6.1
1 5.2
1 7.9
1 3.8
END DATA.
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    let mut warnings = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
        warnings.extend(engine.warnings().iter().map(|w| w.to_string()));
    }
    (tables, warnings)
}

#[test]
fn post_hoc_tests_without_error_degrees_of_freedom_are_left_out() {
    let (tables, warnings) = run(&format!(
        "{THREE_CASES}ONEWAY y BY g /POSTHOC=TUKEY SNK DUNCAN GH T2 SCHEFFE WALLER."
    ));
    assert!(tables.iter().any(|t| t.title == "ANOVA"));
    assert!(!tables.iter().any(|t| t.title == "Multiple Comparisons"));
    assert!(!tables.iter().any(|t| t.title.starts_with("Homogeneous")));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("no post hoc tests") && w.contains("degrees of freedom")),
        "{warnings:?}"
    );
}

#[test]
fn the_spread_versus_level_tests_need_two_groups() {
    let (tables, warnings) = run(&format!(
        "{ONE_GROUP}EXAMINE y BY g /PLOT=SPREADLEVEL /STATISTICS=NONE."
    ));
    assert!(
        !tables
            .iter()
            .any(|t| t.title == "Test of Homogeneity of Variance")
    );
    assert!(
        warnings.iter().any(|w| w.contains("one group only")),
        "{warnings:?}"
    );
}
