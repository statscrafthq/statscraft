//! Parser tests of MODEL PROGRAM, DERIVATIVES, CLEAR MODEL PROGRAMS and NLR.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn nlr(source: &str) -> Nlr {
    match one(source) {
        Command::Nlr(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

fn parameter(name: &str, value: Option<f64>) -> ModelParameter {
    ModelParameter {
        name: name.into(),
        value,
    }
}

#[test]
fn model_program_names_the_parameters_with_optional_starting_values() {
    assert_eq!(
        one("MODEL PROGRAM a=.5 b=-1.6."),
        Command::ModelProgram(vec![parameter("a", Some(0.5)), parameter("b", Some(-1.6))])
    );
    // commas between parameters, and a parameter that FILE will supply
    assert_eq!(
        one("MODEL PROGRAM a, b=3, c."),
        Command::ModelProgram(vec![
            parameter("a", None),
            parameter("b", Some(3.0)),
            parameter("c", None)
        ])
    );
    assert!(err("MODEL PROGRAM a=1 A=2.").contains("named twice"));
    assert!(err("MODEL PROGRAM a=.").contains("end of command"));
}

#[test]
fn derivatives_and_clear_take_no_arguments() {
    assert_eq!(one("DERIVATIVES."), Command::Derivatives);
    assert_eq!(one("CLEAR MODEL PROGRAMS."), Command::ClearModelPrograms);
    assert!(!err("DERIVATIVES x.").is_empty());
}

#[test]
fn nlr_subcommands() {
    let c = nlr("NLR y.");
    assert_eq!(c.dependent, "y");
    assert_eq!(c.criteria, NlrCriteria::default());
    assert_eq!(c.save, NlrSave::default());
    let c = nlr("NLR y /FILE='start.sav' /OUTFILE=out /PRED=fitted \
         /SAVE=PRED RESID(res1) DERIVATIVES /CRITERIA=ITER(80) CKDER=0.25 SSCON .000001, PCON,0 RCON(1E-6).");
    assert_eq!(c.file.as_deref(), Some("start.sav"));
    assert_eq!(c.outfile.as_deref(), Some("out"));
    assert_eq!(c.pred.as_deref(), Some("fitted"));
    assert_eq!(
        c.save,
        NlrSave {
            pred: true,
            resid: Some("res1".into()),
            derivatives: true,
            loss: false
        }
    );
    assert_eq!(
        c.criteria,
        NlrCriteria {
            iterations: Some(80),
            ckder: Some(0.25),
            sscon: Some(0.000001),
            pcon: Some(0.0),
            rcon: Some(0.000001)
        }
    );
    // a bare RES names the residuals RES; SAVE without an equals sign
    assert_eq!(nlr("NLR y /SAVE RES.").save.resid.as_deref(), Some("RES"));
}

#[test]
fn nlr_checks_its_criteria() {
    assert!(err("NLR y /CRITERIA=ITER(0).").contains("whole number"));
    assert!(err("NLR y /CRITERIA=CKDER(2).").contains("0 to 1"));
    assert!(err("NLR y /CRITERIA=SSCON(-1).").contains("not negative"));
    assert!(err("NLR y /CRITERIA=BOGUS(1).").contains("expected"));
    assert!(err("NLR y /BOGUS.").contains("unknown subcommand"));
    assert!(!err("NLR.").is_empty());
}
