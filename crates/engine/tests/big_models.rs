//! Models with more parameters than a dense fit can hold, in every procedure that builds the
//! design as a dense matrix. A UNIANOVA of two factors of 200
//! levels over 20,000 cases had a column for each of about 19,000 level combinations, a row of
//! them for every case and cross-products as wide as they are long (about 14 GB), and was killed
//! when the machine ran out of memory; a NOMREG with a factor of 1,500 levels against one of
//! 1,200 categories had 1,800,000 parameters. Each now stops at once with an error that says
//! why. A factor of a hundred levels still fits. Invented data.

use statscraft_engine::Engine;
use statscraft_syntax::parse;

/// `cases` cases: a response, a factor of `a` levels and one of `b` levels, by a fixed
/// scramble so every combination of levels is different.
fn data(cases: usize, a: usize, b: usize) -> String {
    let mut text = String::from("DATA LIST FREE /y f1 f2 o w.\nBEGIN DATA\n");
    for i in 0..cases {
        let (f1, f2) = (1 + i % a, 1 + (i * 7 + i / a) % b);
        text.push_str(&format!(
            "{} {f1} {f2} {} 1\n",
            (i * 31 % 17) as f64 / 3.0,
            1 + i % 4
        ));
    }
    text.push_str("END DATA.\n");
    text
}

fn error_of(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("no error");
}

#[test]
fn a_design_too_wide_for_memory_is_refused() {
    let message = error_of(&format!("{}UNIANOVA y BY f1 f2.\n", data(20_000, 200, 200)));
    assert!(
        message.contains("UNIANOVA: the model has") && message.contains("more than fit in memory"),
        "{message}"
    );
}

#[test]
fn a_multinomial_model_with_too_many_parameters_is_refused() {
    let message = error_of(&format!("{}NOMREG f2 BY f1.\n", data(3_000, 800, 8)));
    assert!(
        message.contains("NOMREG: the model has") && message.contains("more than the 5000"),
        "{message}"
    );
}

#[test]
fn a_factor_of_a_hundred_levels_still_fits() {
    let source = format!("{}UNIANOVA y BY f1.\n", data(1_200, 120, 7));
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        engine.execute(&command.command).expect("fits");
    }
}

/// Two factors of 200 levels over 20,000 cases make about 19,000 columns of interaction.
fn refused(command: &str, name: &str) {
    let message = error_of(&format!("{}{command}\n", data(20_000, 200, 200)));
    assert!(
        message.starts_with(&format!("{name}: the model has"))
            && message.contains("more than fit in memory"),
        "{command}: {message}"
    );
}

#[test]
fn genlin_refuses_a_design_too_wide_for_memory() {
    refused("GENLIN y BY f1 f2 /MODEL f1 f2 f1*f2.", "GENLIN");
}

#[test]
fn mixed_refuses_fixed_and_random_designs_too_wide_for_memory() {
    refused("MIXED y BY f1 f2 /FIXED=f1 f2 f1*f2.", "MIXED");
    refused("MIXED y BY f1 f2 /FIXED=f1 /RANDOM=f1*f2.", "MIXED");
}

#[test]
fn quantile_regression_refuses_a_design_too_wide_for_memory() {
    refused(
        "QUANTILE REGRESSION y BY f1 f2 /MODEL f1 f2 f1*f2.",
        "QUANTILE REGRESSION",
    );
}

#[test]
fn plum_refuses_a_design_too_wide_for_memory() {
    refused("PLUM o BY f1 f2 /LOCATION=f1 f2 f1*f2.", "PLUM");
}

#[test]
fn pls_refuses_a_design_too_wide_for_memory() {
    refused("PLS y BY f1 f2 /MODEL f1 f2 f1*f2.", "PLS");
}

#[test]
fn varcomp_refuses_a_design_too_wide_for_memory() {
    refused(
        "VARCOMP y BY f1 f2 /RANDOM=f1 f2 /DESIGN=f1 f2 f1*f2.",
        "VARCOMP",
    );
}

#[test]
fn anova_refuses_a_design_too_wide_for_memory() {
    refused("ANOVA y BY f1(1,200) f2(1,200).", "ANOVA");
}

#[test]
fn csglm_refuses_a_design_too_wide_for_memory() {
    let dir = std::env::temp_dir().join(format!("statscraft-big-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plan = dir.join("plan.json").display().to_string();
    refused(
        &format!(
            "CSPLAN ANALYSIS /PLAN FILE='{plan}' /PLANVARS ANALYSISWEIGHT=w /DESIGN STRATA=o /ESTIMATOR TYPE=WR.\nCSGLM y BY f1 f2 /PLAN FILE='{plan}' /MODEL f1 f2 f1*f2."
        ),
        "CSGLM",
    );
}
