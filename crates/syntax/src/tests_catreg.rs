//! Parser tests of CATREG.

use super::*;

fn catreg(source: &str) -> Catreg {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Catreg(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

fn name(s: &str) -> VarItem {
    VarItem::Name(s.into())
}

const SPORD: ScalingLevel = ScalingLevel::SplineOrdinal {
    degree: 2,
    inknot: 2,
};

#[test]
fn catreg_defaults() {
    let c = catreg("CATREG VARIABLES=mood sleep diet /ANALYSIS=mood WITH sleep diet.");
    assert_eq!(c.dependent, ("mood".into(), SPORD));
    assert_eq!(
        c.predictors,
        vec![(vec![name("sleep"), name("diet")], SPORD)]
    );
    assert_eq!(c.initial, CatregInitial::Numerical);
    assert_eq!(c.common.max_iterations, 100);
    assert_eq!(c.common.criterion, 0.00001);
    assert_eq!(c.regularization, None);
    assert_eq!(c.resample, CatregResample::None);
    assert_eq!(c.print, CatregPrint::defaults());
    assert_eq!(c.plot, CatregPlot::default());
    assert!(c.save_trdata.is_none() && c.outfile_trdata.is_none() && c.repeated.is_empty());
}

#[test]
fn catreg_subcommands_and_abbreviations() {
    let c = catreg(
        "CATREG VAR=mood sleep diet walk /ANAL mood (LEVEL=NUME) WITH sleep TO diet (LEVEL=SPNOM DEGREE=1 INKNOT=3)
         walk (LEV=ORDI) /DISCRET=diet(GROUPING NCAT=5 DISTR=UNIFORM) walk(MULTIPLYING)
         /MISS=sleep(MODEIMPU) walk(EXTRACAT) /SUPP=OBJECT(1 TO 3 7) /INIT=MULTISTART(ALL)('signs.sav')
         /MAXIT=50 /CRITIT=.000001 /REGU=ENET(.5 2.5 .25)(.01 3.8 .05)('regu.sav') /RESAM=CROSSVAL(5)
         /PRI=R COEFF R QUANT(sleep TO walk) REGU /PLO=TRANS(sleep walk)(80) RESID(walk) REGU(.75 1.5)
         /SAVE=PRED(guess) RES /OUT=TRDATA('t.sav') DISCRDATA('d.sav').",
    );
    assert_eq!(c.dependent, ("mood".into(), ScalingLevel::Numeric));
    assert_eq!(
        c.predictors[0],
        (
            vec![VarItem::Range("sleep".into(), "diet".into())],
            ScalingLevel::SplineNominal {
                degree: 1,
                inknot: 3
            }
        )
    );
    assert_eq!(c.predictors[1], (vec![name("walk")], ScalingLevel::Ordinal));
    assert_eq!(
        c.common.discretization[0].1,
        Discretize::Grouping {
            ncat: Some(5),
            normal: false
        }
    );
    assert_eq!(c.common.missing[0].1, McaMissing::Active(Impute::Mode));
    assert_eq!(
        c.common.missing[1].1,
        McaMissing::Active(Impute::ExtraCategory)
    );
    assert_eq!(c.common.supplementary_objects, vec![(1, 3), (7, 7)]);
    assert_eq!(
        c.initial,
        CatregInitial::Multistart(None, "signs.sav".into())
    );
    assert_eq!(
        (c.common.max_iterations, c.common.criterion),
        (50, 0.000001)
    );
    assert_eq!(
        c.regularization,
        Some((
            CatregPenalty::Enet((0.5, 2.5, 0.25), (0.01, 3.8, 0.05)),
            Some("regu.sav".into())
        ))
    );
    assert_eq!(c.resample, CatregResample::Crossval(5));
    assert!(c.print.r && c.print.coeff && c.print.regu && !c.print.anova);
    assert_eq!(
        c.print.quant,
        Some(vec![VarItem::Range("sleep".into(), "walk".into())])
    );
    assert_eq!(
        c.plot.trans,
        Some((vec![name("sleep"), name("walk")], Some(60)))
    );
    assert_eq!(c.plot.resid, Some((vec![name("walk")], None)));
    assert_eq!(c.plot.regu, Some(vec![0.75, 1.5]));
    assert_eq!(
        (c.save_pred, c.save_res),
        (Some("guess".into()), Some("RES".into()))
    );
    assert_eq!(c.save_trdata, None);
    assert_eq!(c.outfile_trdata, Some("t.sav".into()));
    assert_eq!(c.outfile_discrdata, Some("d.sav".into()));
}

#[test]
fn catreg_options_and_repeats() {
    let c = catreg(
        "CATREG VARIABLES=a b c /ANALYSIS=a WITH b c /PRINT=NONE /PRINT=HISTORY /INITIAL=FIXSIGNS(3)('s.sav')
         /REGULARIZATION=RIDGE /RESAMPLE=BOOTSTRAP /SAVE /OUTFILE='q.sav' /MISSING=ALL(MODEIMPU).",
    );
    assert_eq!(c.repeated, vec!["PRINT".to_string()]);
    assert_eq!(
        c.print,
        CatregPrint {
            history: true,
            ..CatregPrint::default()
        }
    );
    assert_eq!(c.initial, CatregInitial::Fixsigns(3, "s.sav".into()));
    assert_eq!(
        c.regularization,
        Some((CatregPenalty::Ridge((0.0, 1.0, 0.02)), None))
    );
    assert_eq!(c.resample, CatregResample::Bootstrap(50));
    assert_eq!(c.save_trdata, Some("TRA".into()));
    assert_eq!(c.outfile_trdata, Some("q.sav".into()));
    let c = catreg(
        "CATREG VARIABLES=a b /ANALYSIS=a WITH b /INITIAL=MULTISTART('s.sav') /SAVE=TRDATA(longroot).",
    );
    assert_eq!(
        c.initial,
        CatregInitial::Multistart(Some(50.0), "s.sav".into())
    );
    assert_eq!(c.save_trdata, Some("longr".into()));
    let c = catreg("CATREG VARIABLES=a b /ANALYSIS=a WITH b /REGULARIZATION=LASSO(.2 .2 0).");
    assert_eq!(
        c.regularization,
        Some((CatregPenalty::Lasso((0.2, 0.2, 0.0)), None))
    );
}

#[test]
fn catreg_errors() {
    assert!(err("CATREG VARIABLES=a b.").contains("ANALYSIS"));
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a b.").contains("WITH"));
    assert!(err("CATREG ANALYSIS=a WITH b /VARIABLES=a b.").contains("VARIABLES"));
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a WITH b (LEVEL=MNOM).").contains("MNOM"));
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a WITH b (WEIGHT=2).").contains("WEIGHT"));
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /CRITITER=0.5.").contains("0.1"));
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /INITIAL=MULTISTART(ALL).").contains("file")
    );
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /INITIAL=MULTISTART(120)('s').")
            .contains("100")
    );
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /REGULARIZATION=RIDGE(1 0 .1).")
            .contains("start")
    );
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /REGULARIZATION=LASSO(1 2).")
            .contains("three")
    );
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /SAVE=PRED(p(2)).").contains("no number"));
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /MISSING=ALL a(LISTWISE).").contains("ALL")
    );
    assert!(
        err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /OUTFILE=TRDATA('x') DISCRDATA('X').")
            .contains("its own")
    );
    assert!(err("CATREG VARIABLES=a b /ANALYSIS=a WITH b /PLOT=OBJECT.").contains("unexpected"));
}
