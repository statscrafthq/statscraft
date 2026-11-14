//! Parser tests of COXREG, TIME PROGRAM and CLEAR TIME PROGRAM.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn coxreg(source: &str) -> Coxreg {
    match one(source) {
        Command::Coxreg(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

#[test]
fn defaults_and_status_values() {
    let c = coxreg("COXREG months WITH plan spend /STATUS=ended(1).");
    assert_eq!(c.time, "months");
    assert_eq!(c.terms.len(), 2);
    assert_eq!(c.status, "ended");
    assert_eq!(c.events, [MissingValue::Value(Value::Number(1.0))]);
    assert_eq!(c.methods.len(), 1);
    assert_eq!(c.methods[0].kind, LogisticMethodKind::Enter);
    assert_eq!(c.criteria, CoxregCriteria::default());
    assert_eq!(c.print, CoxregPrint::default());
    // EVENT, ranges, commas and LOST
    let c =
        coxreg("COXREG VARIABLES=months WITH spend /STATUS ended EVENT (1 THRU 3, 7) LOST (0).");
    assert_eq!(
        c.events,
        [
            MissingValue::Range(Bound::Number(1.0), Bound::Number(3.0)),
            MissingValue::Value(Value::Number(7.0)),
        ]
    );
    let c = coxreg("COXREG months WITH spend /STATUS=outcome('quit' 'lost').");
    assert_eq!(c.events.len(), 2);
    // no covariates: the null model
    let c = coxreg("COXREG months /STATUS=ended(1).");
    assert!(c.methods.is_empty());
}

#[test]
fn subcommands_and_abbreviations() {
    let c = coxreg(
        "COXREG months WITH plan spend plan BY spend /STATUS=ended(1) /STRATA=site
           /CATEGORICAL=plan /CONTRAST(plan)=SIMPLE(1) /METHOD=FSTEP(WALD) /MISSING=INCLUDE
           /PRINT=ALL /CRITERIA=PCON(0.001) LCON(0) ITERATE(30) PIN(0.1) POUT(0.2)
           /PLOT=SURVIVAL OMS /PATTERN=spend(40) BY plan /SAVE=SE(se_s) HAZARD PRESID(pr) /EXTERNAL.",
    );
    assert_eq!(c.terms[2], ["plan", "spend"]);
    assert_eq!(c.strata.as_deref(), Some("site"));
    assert_eq!(c.contrasts[0].1, Contrast::Simple(Some(1)));
    assert_eq!(c.methods[0].kind, LogisticMethodKind::Fstep(Removal::Wald));
    assert!(c.include);
    assert!(c.print.corr && c.print.baseline && !c.print.summary);
    assert_eq!(c.print.ci, Some(95.0));
    assert_eq!(c.criteria.bcon, 0.001);
    assert_eq!(c.criteria.iterate, 30);
    assert!(c.plot.survival && c.plot.oms && !c.plot.hazard);
    assert_eq!(c.patterns[0].values, [("spend".to_string(), 40.0)]);
    assert_eq!(c.patterns[0].by.as_deref(), Some("plan"));
    assert_eq!(
        c.save,
        [
            (CoxregTemp::Se, Some("se_s".to_string())),
            (CoxregTemp::Hazard, None),
            (CoxregTemp::Presid, Some("pr".to_string())),
        ]
    );
    // three-letter abbreviations; RESID is HAZARD; METHOD may be left out
    let c = coxreg(
        "COXREG months WITH spend /STAT=ended(1) /PRI=SUM BAS CI(90) /CRI=ITER(5) /BSTEP(LR) spend
           /SAV=RESID SUR.",
    );
    assert!(c.print.summary && c.print.baseline);
    assert_eq!(c.print.ci, Some(90.0));
    assert_eq!(c.criteria.iterate, 5);
    assert_eq!(c.methods[0].kind, LogisticMethodKind::Bstep(Removal::Lr));
    assert_eq!(c.save[0].0, CoxregTemp::Hazard);
    assert_eq!(c.save[1].0, CoxregTemp::Survival);
}

#[test]
fn errors() {
    assert!(err("COXREG months WITH spend.").contains("STATUS"));
    assert!(err("COXREG months age WITH spend /STATUS=ended(1).").contains("one survival time"));
    assert!(
        err("COXREG months WITH spend /STATUS=ended(1) /OUTFILE=COEFF('c.sav').")
            .contains("not supported yet")
    );
    assert!(err("COXREG months WITH spend /STATUS=ended(1) /SAVE=SE SE.").contains("only once"));
    assert!(
        err("COXREG months WITH spend /STATUS=ended(1) /CRITERIA=PIN(2).").contains("probability")
    );
    assert!(err("COXREG months /STATUS=ended(1) /METHOD=ENTER.").contains("WITH"));
    assert!(err("COXREG months WITH spend /STATUS=ended('a' 1).").contains("not both"));
}

#[test]
fn time_program_commands() {
    assert_eq!(one("TIME PROGRAM."), Command::TimeProgram);
    assert_eq!(one("CLEAR TIME PROGRAM."), Command::ClearTimeProgram);
}
