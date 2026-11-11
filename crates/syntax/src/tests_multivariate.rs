//! Parser tests of CLUSTER, DISCRIMINANT and ANOVA.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

fn cluster(source: &str) -> Cluster {
    match one(source) {
        Command::Cluster(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn discriminant(source: &str) -> Discriminant {
    match one(source) {
        Command::Discriminant(d) => *d,
        other => panic!("{other:?}"),
    }
}

fn anova(source: &str) -> Anova {
    match one(source) {
        Command::Anova(a) => *a,
        other => panic!("{other:?}"),
    }
}

#[test]
fn cluster_defaults() {
    let c = cluster("CLUSTER v1 TO v4.");
    assert_eq!(c.variables, [VarItem::Range("v1".into(), "v4".into())]);
    assert_eq!(c.measure, Proximity::SEuclid);
    assert_eq!(c.methods, [(Linkage::BetweenGroups, None)]);
    assert!(c.schedule && !c.distance && c.membership.is_none() && !c.plot);
    assert!(c.save.is_none() && c.matrix_in.is_none() && !c.include);
}

#[test]
fn cluster_abbreviations_and_the_manual_examples() {
    let c = cluster("CLUSTER V1 TO V4 /PLOT=DENDROGRAM /PRINT=CLUSTER (2 4).");
    assert_eq!(c.membership, Some((2, 4)));
    assert!(!c.schedule && c.plot);
    let c = cluster("CLUSTER V1 V2 V3 /METHOD=SINGLE COMPLETE WARDS.");
    assert_eq!(
        c.methods.iter().map(|m| m.0).collect::<Vec<_>>(),
        [Linkage::Single, Linkage::Complete, Linkage::Ward]
    );
    let c = cluster("CLUSTER A B C /METHOD=BAVERAGE SINGLE (SINMEM) WARD /SAVE=CLUSTERS(3,5).");
    assert_eq!(c.methods[1], (Linkage::Single, Some("SINMEM".into())));
    assert_eq!(c.save, Some((3, 5)));
    let c = cluster(
        "CLUSTER a b /MEAS=MINK(3) /METH=WAVE CENT MED DEF /PRIN=SCHED DIST /MISS=INCL /ID=name.",
    );
    assert_eq!(c.measure, Proximity::Minkowski(3.0));
    assert_eq!(
        c.methods.iter().map(|m| m.0).collect::<Vec<_>>(),
        [
            Linkage::WithinGroups,
            Linkage::Centroid,
            Linkage::Median,
            Linkage::BetweenGroups
        ]
    );
    assert!(c.schedule && c.distance && c.include);
    assert_eq!(c.id.as_deref(), Some("name"));
    for (text, measure) in [
        ("EUCLID", Proximity::Euclid),
        ("COSINE", Proximity::Cosine),
        ("CORR", Proximity::Correlation),
        ("CHEB", Proximity::Chebychev),
        ("BLOCK", Proximity::Block),
        ("POWER(4,2)", Proximity::Power(4.0, 2.0)),
        ("CHISQ", Proximity::Chisq),
        ("PH2", Proximity::Ph2),
        ("DEFAULT", Proximity::SEuclid),
        (
            "JACCARD",
            Proximity::Binary(BinaryMeasure::Jaccard, 1.0, Some(0.0)),
        ),
        ("SM(2)", Proximity::Binary(BinaryMeasure::Sm, 2.0, None)),
        (
            "Q(1,2)",
            Proximity::Binary(BinaryMeasure::Q, 1.0, Some(2.0)),
        ),
        (
            "BLWMN",
            Proximity::Binary(BinaryMeasure::Blwmn, 1.0, Some(0.0)),
        ),
        ("SS5", Proximity::Binary(BinaryMeasure::Ss5, 1.0, Some(0.0))),
        (
            "DISPER",
            Proximity::Binary(BinaryMeasure::Disper, 1.0, Some(0.0)),
        ),
    ] {
        assert_eq!(
            cluster(&format!("CLUSTER a b /MEASURE={text}.")).measure,
            measure,
            "{text}"
        );
    }
    // PRINT=NONE overrides the others; PLOT=NONE asks for no chart
    let c = cluster("CLUSTER a b /PRINT=DISTANCE NONE /PLOT=NONE.");
    assert!(!c.schedule && !c.distance && !c.plot);
    // matrix input without a variable list
    let c = cluster("CLUSTER /MATRIX=IN(*) /ID=city.");
    assert!(c.variables.is_empty());
    assert_eq!(c.matrix_in, Some(None));
    let c = cluster("CLUSTER /MATRIX=IN('dist.sav').");
    assert_eq!(c.matrix_in, Some(Some("dist.sav".into())));
}

#[test]
fn cluster_errors() {
    assert!(err("CLUSTER a b /MATRIX=OUT(*).").contains("not supported yet"));
    assert!(err("CLUSTER /PRINT=SCHEDULE.").contains("variable list"));
    assert!(err("CLUSTER a b /PRINT=CLUSTER(5,2).").contains("above the largest"));
    assert!(err("CLUSTER a b /MEASURE=SEUCLID EUCLID.").contains("one measure"));
    assert!(err("CLUSTER a b /MEASURE=MINKOWSKI(0).").contains("above 0"));
    assert!(err("CLUSTER a b /METHOD=AVERAGE.").contains("unexpected"));
}

#[test]
fn discriminant_defaults() {
    let d = discriminant("DISCRIMINANT GROUPS=OUTCOME (1,4) /VARIABLES=V1 TO V7.");
    assert_eq!((d.group.as_str(), d.min, d.max), ("OUTCOME", 1.0, 4.0));
    assert_eq!(d.analyses.len(), 1);
    let a = &d.analyses[0];
    assert_eq!(a.method, DiscriminantMethod::Direct);
    assert_eq!(
        (a.tolerance, a.fin, a.fout, a.vin),
        (0.001, 3.84, 2.71, 0.0)
    );
    assert_eq!(a.priors, Priors::Equal);
    assert!(a.variables.is_none() && a.save.is_none());
    assert!(d.history && !d.separate && !d.meansub);
    assert_eq!(d.classify, ClassifyCases::NonMissing);
    assert_eq!(d.statistics, DiscriminantStatistics::default());
}

#[test]
fn discriminant_blocks_and_abbreviations() {
    let d = discriminant(
        "DISC GROUPS=APPROVAL(1,5) /VARS=Q1 TO Q10 /SELECT=COMPLETE(1) /CLASSIFY=UNSELECTED.",
    );
    assert_eq!(d.select, Some(("COMPLETE".into(), 1.0)));
    assert_eq!(d.classify, ClassifyCases::Unselected);
    let d = discriminant(
        "DISCRIMINANT GROUPS=SUCCESS(0,1) /VARIABLES=A, B, C, D, E \
         /ANALYSIS=A TO C (2) D, E (1) /METH=WILKS /TOLER=.01 /MAXS=4 /FIN=4 /FOUT=3 \
         /PRIORS = 4*.15,.4 /SAVE CLASS=PRDCLASS SCORES=SCORE PROBS=PRB \
         /ANALYSIS=ALL /METHOD=RAO /VIN=.5 /PIN=.05 /POUT=.1 /FUNCTIONS=1,90,.5 /PRIORS=SIZE \
         /SAVE CLASS SCORES(SC) PROBS \
         /MISSING=INCLUDE /HISTORY=NOEND /CLASSIFY=UNCLASSIFIED SEPARATE MEANSUB \
         /STATISTICS=ALL /PLOT=MAP.",
    );
    assert_eq!(d.analyses.len(), 2);
    let a = &d.analyses[0];
    assert_eq!(
        a.variables,
        Some(vec![
            (vec![VarItem::Range("A".into(), "C".into())], 2),
            (
                vec![VarItem::Name("D".into()), VarItem::Name("E".into())],
                1
            ),
        ])
    );
    assert_eq!(a.method, DiscriminantMethod::Wilks);
    assert_eq!(
        (a.tolerance, a.max_steps, a.fin, a.fout),
        (0.01, Some(4), 4.0, 3.0)
    );
    assert_eq!(a.priors, Priors::Values(vec![0.15, 0.15, 0.15, 0.15, 0.4]));
    let s = a.save.as_ref().unwrap();
    assert_eq!(s.class, Some(Some("PRDCLASS".into())));
    assert_eq!(s.scores, Some(Some("SCORE".into())));
    let b = &d.analyses[1];
    assert_eq!(b.variables, Some(vec![(vec![VarItem::All], 1)]));
    assert_eq!(b.method, DiscriminantMethod::Rao);
    assert_eq!((b.vin, b.pin, b.pout), (0.5, Some(0.05), Some(0.1)));
    assert_eq!(b.functions, (Some(1), 90.0, 0.5));
    assert_eq!(b.priors, Priors::Size);
    let s = b.save.as_ref().unwrap();
    assert_eq!(
        (s.class.clone(), s.scores.clone()),
        (Some(None), Some(Some("SC".into())))
    );
    assert!(d.include && !d.history && d.separate && d.meansub && d.plot);
    assert_eq!(d.classify, ClassifyCases::Unclassified);
    assert!(d.statistics.crossvalid && d.statistics.boxm && d.statistics.fpair);
    // STATISTICS without keywords; a block subcommand after a global one is ignored
    let d = discriminant(
        "DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /STATISTICS /METHOD=WILKS /STAT=COV CORR.",
    );
    let st = d.statistics;
    assert!(st.mean && st.stddev && st.univf && st.cov && st.corr && !st.boxm);
    assert_eq!(d.ignored, ["METHOD"]);
    assert_eq!(d.analyses[0].method, DiscriminantMethod::Direct);
    for (word, method) in [
        ("MAHAL", DiscriminantMethod::Mahal),
        ("MAXMINF", DiscriminantMethod::MaxMinF),
        ("MINRESID", DiscriminantMethod::MinResid),
        ("DIRECT", DiscriminantMethod::Direct),
    ] {
        let d = discriminant(&format!(
            "DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /METHOD={word}."
        ));
        assert_eq!(d.analyses[0].method, method);
    }
}

#[test]
fn discriminant_errors() {
    assert!(err("DISCRIMINANT /VARIABLES=a b.").contains("GROUPS"));
    assert!(err("DISCRIMINANT GROUPS=g(1,2).").contains("VARIABLES"));
    assert!(err("DISCRIMINANT GROUPS=g(2,1) /VARIABLES=a.").contains("smaller first"));
    assert!(
        err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /ROTATE=COEFF.").contains("not supported yet")
    );
    assert!(
        err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /MATRIX=OUT(*).")
            .contains("not supported yet")
    );
    assert!(
        err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /TOLERANCE=2.").contains("between 0 and 1")
    );
    assert!(err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /ANALYSIS=a(100).").contains("0 to 99"));
    assert!(err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /PRIORS=-.5 1.5.").contains("negative"));
}

#[test]
fn anova_lists_and_subcommands() {
    let a = anova(
        "ANOVA VARIABLES=PRESTIGE BY REGION(1,9) SEX,RACE(1,2) /MAXORDERS=2 /STATISTICS=MEAN.",
    );
    assert_eq!(a.lists.len(), 1);
    let l = &a.lists[0];
    assert_eq!(l.dependents, [VarItem::Name("PRESTIGE".into())]);
    assert_eq!(l.factors.len(), 2);
    assert_eq!(
        l.factors[1].0,
        [VarItem::Name("SEX".into()), VarItem::Name("RACE".into())]
    );
    assert_eq!((l.factors[1].1, l.factors[1].2), (1.0, 2.0));
    assert_eq!(a.max_order, 2);
    assert!(a.mean && !a.reg && !a.mca);
    assert_eq!(a.method, AnovaMethod::Unique);
    assert_eq!(a.covariate_order, CovariateOrder::First);
    let a =
        anova("ANOVA VARIABLES=PRESTIGE BY REGION(1,9) SEX,RACE(1,2) /RINCOME BY SEX,RACE(1,2).");
    assert_eq!(a.lists.len(), 2);
    assert_eq!(a.lists[1].dependents, [VarItem::Name("RINCOME".into())]);
    let a = anova(
        "anova y y2 BY a(1,3) b(1.7,2.2) WITH c d /COVAR=AFTER /MAXO=NONE /METH=EXP /STAT /MISS=INCL.",
    );
    let l = &a.lists[0];
    assert_eq!(l.covariates.len(), 2);
    assert_eq!((l.factors[1].1, l.factors[1].2), (1.0, 2.0));
    assert_eq!(a.covariate_order, CovariateOrder::After);
    assert_eq!(a.max_order, 1);
    assert_eq!(a.method, AnovaMethod::Experimental);
    assert!(a.mean && a.reg && !a.mca && a.include);
    let a = anova(
        "ANOVA y BY a(1,2) /METHOD=HIERARCHICAL /COVARIATES=WITH /STATISTICS=ALL /STATISTICS MCA.",
    );
    assert_eq!(a.method, AnovaMethod::Hierarchical);
    assert_eq!(a.covariate_order, CovariateOrder::With);
    assert!(!a.mean && !a.reg && a.mca);
    // a dependent named like a subcommand still starts a list, since a list has BY
    let a = anova("ANOVA y BY a(1,2) /method BY a(1,2) /MAXORDERS=ALL.");
    assert_eq!(a.lists.len(), 2);
    assert_eq!(a.max_order, 5);
}

#[test]
fn anova_errors() {
    assert!(err("ANOVA y BY a.").contains("unexpected"));
    assert!(err("ANOVA y a(1,2).").contains("unexpected '('"));
    assert!(err("ANOVA y BY a(3,1).").contains("smaller value first"));
    assert!(err("ANOVA y BY a(1,2) /MAXORDERS=6.").contains("1 to 5"));
}
