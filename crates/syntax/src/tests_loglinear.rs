//! Parser tests of HILOGLINEAR and GENLOG.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn hilo(source: &str) -> Hiloglinear {
    match one(source) {
        Command::Hiloglinear(h) => *h,
        other => panic!("{other:?}"),
    }
}

fn genlog(source: &str) -> Genlog {
    match one(source) {
        Command::Genlog(g) => *g,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn hiloglinear_ranges_apply_to_their_group() {
    let h = hilo("HILOGLINEAR route season (1,4) delay (0 1).");
    assert_eq!(h.ranges.len(), 2);
    assert_eq!(h.ranges[0].variables.len(), 2);
    assert_eq!((h.ranges[0].min, h.ranges[0].max), (1.0, 4.0));
    assert_eq!((h.ranges[1].min, h.ranges[1].max), (0.0, 1.0));
    // no DESIGN: one default design, flagged for the warning
    assert_eq!(h.designs.len(), 1);
    assert!(h.implicit_design);
    assert_eq!(h.designs[0].terms, None);
    assert_eq!(h.designs[0].print, None);
    assert_eq!(h.designs[0].criteria, HiloCriteria::default());
    let h = hilo("HILOGLINEAR ALL (1,2).");
    assert_eq!(h.ranges[0].variables, vec![VarItem::All]);
}

#[test]
fn hiloglinear_settings_carry_over_and_method_applies_once() {
    let h = hilo(
        "HILO a b c (1,3) /METH /MAXORDER=2 /CRIT=P(0.01) ITER(40) DELTA(0) MAXST(5) CONV(0.001) \
         /PRINT=FREQ ASSOC /PLOT /DESIGN=a*b, c /DESIGN a*b*c /PLOT=NONE /DESIGN.",
    );
    assert!(!h.implicit_design);
    assert_eq!(h.designs.len(), 3);
    let d = &h.designs[0];
    assert!(d.backward);
    assert_eq!(d.maxorder, Some(2));
    assert_eq!(d.criteria.p, 0.01);
    assert_eq!(d.criteria.iterate, 40);
    assert_eq!(d.criteria.delta, 0.0);
    assert_eq!(d.criteria.maxsteps, 5);
    assert_eq!(d.criteria.converge, Some(0.001));
    assert_eq!(
        d.terms,
        Some(vec![vec!["a".to_string(), "b".into()], vec!["c".into()]])
    );
    let print = d.print.unwrap();
    assert!(print.freq && print.association && !print.resid && !print.estim);
    assert!(d.plot_resid && d.plot_normal);
    // the second design keeps the settings but not METHOD
    let d = &h.designs[1];
    assert!(!d.backward);
    assert_eq!(d.maxorder, Some(2));
    assert_eq!(d.terms.as_ref().unwrap()[0].len(), 3);
    assert!(d.plot_resid);
    let d = &h.designs[2];
    assert_eq!(d.terms, None);
    assert!(!d.plot_resid && !d.plot_normal);
    // DEFAULT resets the criteria
    let h = hilo("HILOGLINEAR a b (1,2) /CRITERIA=ITERATE(5) /CRITERIA=DEFAULT /DESIGN=a b.");
    assert_eq!(h.designs[0].criteria, HiloCriteria::default());
}

#[test]
fn hiloglinear_cell_weights_and_print_keywords() {
    let h = hilo("HILOGLINEAR a b (1,3) /CWEIGHT=(2*1 0, 3*1 0 2*1) /PRINT=NONE /DESIGN=a b.");
    assert_eq!(
        h.designs[0].cweight,
        Some(CellWeights::Values(vec![
            1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0
        ]))
    );
    assert_eq!(h.designs[0].print, Some(HiloPrint::default()));
    let h = hilo(
        "HILOGLINEAR a b (1,3) /CWEIGHT=w /PRINT=ALL /PLOT=NORMPLOT /MISSING=INCLUDE /DESIGN.",
    );
    assert_eq!(
        h.designs[0].cweight,
        Some(CellWeights::Variable("w".into()))
    );
    assert!(h.include);
    assert!(h.designs[0].print.unwrap().estim);
    assert!(h.designs[0].plot_normal && !h.designs[0].plot_resid);
}

#[test]
fn hiloglinear_errors() {
    let m = err("HILOGLINEAR a b.");
    assert!(m.contains("end of command"), "{m}");
    let m = err("HILOGLINEAR a b (3,1).");
    assert!(m.contains("minimum first"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /CRITERIA.");
    assert!(m.contains("at least one keyword"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /CRITERIA=P(1).");
    assert!(m.contains("below 1"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /CRITERIA=MAXSTEPS(100).");
    assert!(m.contains("1 to 99"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /CWEIGHT=w /CWEIGHT=v.");
    assert!(m.contains("only"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /MISSING=INCLUDE /MISSING=EXCLUDE.");
    assert!(m.contains("once"), "{m}");
    let m = err("HILOGLINEAR a b (1,2) /CWEIGHT=(2*).");
    assert!(m.contains("unexpected ')'"), "{m}");
}

#[test]
fn genlog_defaults() {
    let g = genlog("GENLOG genre branch.");
    assert_eq!(g.dependents.len(), 2);
    assert!(g.independents.is_empty() && g.covariates.is_empty());
    assert!(!g.multinomial && !g.include);
    assert_eq!(g.criteria, GenlogCriteria::default());
    assert_eq!(g.print, GenlogPrint::default());
    assert!(g.print.freq && g.print.resid && g.print.adjresid && g.print.dev);
    assert!(!g.print.estim && !g.print.zresid);
    assert_eq!(g.plot, GenlogPlot::default());
    assert_eq!(g.design, None);
}

#[test]
fn genlog_logit_with_covariates_and_every_subcommand() {
    let g = genlog(
        "GENLOG outcome BY clinic shift WITH dose /CSTR=cw /GRES=g1 g2 /GLOR=odds \
         /MODEL=MULTI /CRIT=CONV(0.0001) ITERATION(50) DELTA(0) CIN(90) EPS(1E-10) \
         /PRINT=ESTIM COV CORR DESIGN ITER ZRESID /PLOT=RESID(DEV) NORMPROB(ADJRESID DEV) \
         /SAVE=PRED (fitted) RESID ADJ /MISSING=INCLUDE \
         /DESIGN=outcome outcome BY clinic(2), outcome*shift dose.",
    );
    assert_eq!(g.independents.len(), 2);
    assert_eq!(g.covariates.len(), 1);
    assert_eq!(g.cstructure.as_deref(), Some("cw"));
    assert_eq!(g.gresid.len(), 2);
    assert_eq!(g.glor.len(), 1);
    assert!(g.multinomial && g.include);
    assert_eq!(g.criteria.converge, 0.0001);
    assert_eq!(g.criteria.iterate, 50);
    assert_eq!(g.criteria.delta, 0.0);
    assert_eq!(g.criteria.cin, 90.0);
    assert_eq!(g.criteria.eps, 1e-10);
    let p = g.print;
    assert!(p.estim && p.cov && p.corr && p.design && p.iteration && p.zresid);
    assert!(!p.freq && !p.resid && !p.dev);
    assert_eq!(
        g.plot.resid,
        Some(GenlogResiduals {
            adjusted: false,
            deviance: true
        })
    );
    assert_eq!(
        g.plot.normprob,
        Some(GenlogResiduals {
            adjusted: true,
            deviance: true
        })
    );
    assert_eq!(
        g.save,
        vec![
            (GenlogSaveKind::Pred, Some("fitted".into())),
            (GenlogSaveKind::Resid, None),
            (GenlogSaveKind::Adjresid, None)
        ]
    );
    let design = g.design.unwrap();
    assert_eq!(design.len(), 4);
    assert_eq!(design[1][1].name, "clinic");
    assert_eq!(design[1][1].category, Some(2));
    assert_eq!(design[2].len(), 2);
    assert_eq!(design[3][0].name, "dose");
}

#[test]
fn genlog_print_none_and_plot_none() {
    let g = genlog("GENLOG a b /PRINT=NONE /PLOT=NONE.");
    assert_eq!(
        g.print,
        GenlogPrint {
            freq: false,
            resid: false,
            adjresid: false,
            dev: false,
            ..GenlogPrint::default()
        }
    );
    assert_eq!(g.plot.resid, None);
    assert_eq!(g.plot.normprob, None);
    let g = genlog("GENLOG a b /PRINT=ALL /PLOT=RESID.");
    assert!(g.print.corr && g.print.freq);
    assert!(g.plot.resid.unwrap().adjusted && g.plot.normprob.is_none());
}

#[test]
fn genlog_errors() {
    let m = err("GENLOG a BY b.");
    assert!(m.contains("MULTINOMIAL"), "{m}");
    let m = err("GENLOG a /CRITERIA=CIN(40).");
    assert!(m.contains("50 to 99.99"), "{m}");
    let m = err("GENLOG a /SAVE=PRED PRED.");
    assert!(m.contains("only once"), "{m}");
    let m = err("GENLOG a /DESIGN=a(0).");
    assert!(m.contains("positive whole number"), "{m}");
    let m = err("GENLOG a /MODEL=BINOMIAL.");
    assert!(m.contains("BINOMIAL"), "{m}");
}
