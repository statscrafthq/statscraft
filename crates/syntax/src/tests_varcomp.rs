//! Parser tests of VARCOMP.

use super::*;

fn varcomp(source: &str) -> Varcomp {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Varcomp(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn defaults_with_the_variable_list_and_random() {
    let c = varcomp("VARCOMP gain BY plot crop /RANDOM=plot.");
    assert_eq!(c.dependent, "gain");
    assert_eq!(c.random, ["plot"]);
    assert_eq!(c.method, VarcompMethod::Minque(1));
    assert!(c.intercept && !c.include && c.regwgt.is_none() && c.design.is_none());
    assert_eq!((c.converge, c.eps, c.iterate), (1e-8, 1e-8, 50));
    assert_eq!(c.print, VarcompPrint::default());
    assert!(c.outfile.is_none());
}

#[test]
fn every_subcommand_with_abbreviations() {
    let c = varcomp(
        "VARCOM gain BY plot crop WITH rain\n\
         /RAND=plot /RANDOM crop plot\n\
         /METH=SST(1) /INTER=EXCL /MISS=INCL /REGWGT=w1 w2\n\
         /CRIT=CONV(1E-10) EPS(1E-6) ITER(20) /CRIT=ITERATE(30)\n\
         /PRIN=EMS SS /PRINT=HIST(2)\n\
         /OUTF=VAREST CORB ('vc.sav')\n\
         /DESIGN=INTERCEPT plot crop(plot), rain.",
    );
    assert_eq!(c.random, ["plot", "crop"]);
    assert_eq!(c.method, VarcompMethod::Sstype(1));
    assert!(!c.intercept && c.include);
    assert_eq!(c.regwgt.as_deref(), Some("w2"));
    assert_eq!((c.converge, c.eps, c.iterate), (1e-10, 1e-6, 30));
    assert_eq!(
        c.print,
        VarcompPrint {
            ems: true,
            ss: true,
            history: Some(2)
        }
    );
    let out = c.outfile.unwrap();
    assert!(out.varest && out.matrix == Some(true) && out.file == "vc.sav");
    let labels: Vec<&str> = c
        .design
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    assert_eq!(labels, ["Intercept", "plot", "crop(plot)", "rain"]);
}

#[test]
fn methods_and_their_defaults() {
    let m = |s: &str| varcomp(&format!("VARCOMP y BY a /RANDOM=a /METHOD={s}.")).method;
    assert_eq!(m("MINQUE(0)"), VarcompMethod::Minque(0));
    assert_eq!(m("MINQUE"), VarcompMethod::Minque(1));
    assert_eq!(m("ML"), VarcompMethod::Ml);
    assert_eq!(m("REML"), VarcompMethod::Reml);
    assert_eq!(m("SSTYPE"), VarcompMethod::Sstype(3));
    let c = varcomp(
        "VARCOMP y BY a /RANDOM=a /METHOD=ML /METHOD=REML /PRINT=HISTORY /OUTFILE=COVB (vc).",
    );
    assert_eq!(c.method, VarcompMethod::Reml);
    assert_eq!(c.print.history, Some(1));
    let out = c.outfile.unwrap();
    assert!(!out.varest && out.matrix == Some(false) && out.file == "vc");
}

#[test]
fn errors_name_the_problem() {
    assert!(err("VARCOMP y BY a.").contains("RANDOM"));
    assert!(err("VARCOMP y /RANDOM=a.").contains("BY"));
    assert!(err("VARCOMP y BY a /RANDOM=a /METHOD=MINQUE(2).").contains("MINQUE is 0 or 1"));
    assert!(err("VARCOMP y BY a /RANDOM=a /METHOD=SSTYPE(2).").contains("SSTYPE is 1 or 3"));
    assert!(err("VARCOMP y BY a /RANDOM=a /METHOD.").contains("unexpected end"));
    assert!(err("VARCOMP y BY a /RANDOM=a /OUTFILE=COVB CORB ('f').").contains("not both"));
    assert!(err("VARCOMP y BY a /RANDOM=a /OUTFILE=('f').").contains("unexpected '('"));
    assert!(err("VARCOMP y BY a /RANDOM=a /CRITERIA=ITERATE(2.5).").contains("whole"));
    assert!(err("VARCOMP y BY a /RANDOM=a /CRITERIA=EPS(0).").contains("positive"));
}
