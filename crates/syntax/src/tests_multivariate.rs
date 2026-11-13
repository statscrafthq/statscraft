//! Parser tests of CLUSTER, DISCRIMINANT, ANOVA and CORRESPONDENCE.

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
    assert!(c.schedule && !c.distance && c.membership.is_none());
    assert!(c.plot.vertical && !c.plot.horizontal && !c.plot.dendrogram);
    assert!(c.save.is_none() && c.matrix_in.is_none() && !c.include);
}

#[test]
fn cluster_abbreviations_and_method_lists() {
    let c = cluster("CLUSTER s1 TO s5 /PLOT=DENDROGRAM /PRINT=CLUSTER (3 6).");
    assert_eq!(c.membership, Some((3, 6)));
    assert!(!c.schedule && c.plot.dendrogram && !c.plot.vertical);
    let c = cluster("CLUSTER a b /PLOT=VICICLE(2,9,3) HICICLE.");
    assert!(c.plot.vertical && c.plot.horizontal && c.plot.range == (1, None, 1));
    let c = cluster("CLUSTER a b /PLOT=HICICLE(2,9,3).");
    assert_eq!((c.plot.range, c.plot.vertical), ((2, Some(9), 3), false));
    let c = cluster("CLUSTER p q r /METHOD=COMPLETE WARDS SINGLE.");
    assert_eq!(
        c.methods.iter().map(|m| m.0).collect::<Vec<_>>(),
        [Linkage::Complete, Linkage::Ward, Linkage::Single]
    );
    let c = cluster("CLUSTER x y z /METHOD=WARD BAVERAGE SINGLE (LINKS) /SAVE=CLUSTERS(2,4).");
    assert_eq!(c.methods[2], (Linkage::Single, Some("LINKS".into())));
    assert_eq!(c.save, Some((2, 4)));
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
    assert!(!c.schedule && !c.distance && !c.plot.any());
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
        "DISC GROUPS=TIER(1,4) /VARS=S1 TO S8 /SELECT=SAMPLE(2) /CLASSIFY=UNSELECTED.",
    );
    assert_eq!(d.select, Some(("SAMPLE".into(), 2.0)));
    assert_eq!(d.classify, ClassifyCases::Unselected);
    let d = discriminant(
        "DISCRIMINANT GROUPS=PASSED(0,1) /VARIABLES=P, Q, R, S, T \
         /ANALYSIS=P TO R (2) S, T (1) /METH=WILKS /TOLER=.02 /MAXS=5 /FIN=3.5 /FOUT=2.5 \
         /PRIORS = 3*.2,.4 /SAVE CLASS=PCLASS SCORES=DSC PROBS=PP \
         /ANALYSIS=ALL /METHOD=RAO /VIN=.4 /PIN=.04 /POUT=.09 /FUNCTIONS=2,80,.3 /PRIORS=SIZE \
         /SAVE CLASS SCORES(SC) PROBS \
         /MISSING=INCLUDE /HISTORY=NOEND /CLASSIFY=UNCLASSIFIED SEPARATE MEANSUB \
         /STATISTICS=ALL /PLOT=MAP.",
    );
    assert_eq!(d.analyses.len(), 2);
    let a = &d.analyses[0];
    assert_eq!(
        a.variables,
        Some(vec![
            (vec![VarItem::Range("P".into(), "R".into())], 2),
            (
                vec![VarItem::Name("S".into()), VarItem::Name("T".into())],
                1
            ),
        ])
    );
    assert_eq!(a.method, DiscriminantMethod::Wilks);
    assert_eq!(
        (a.tolerance, a.max_steps, a.fin, a.fout),
        (0.02, Some(5), 3.5, 2.5)
    );
    assert_eq!(a.priors, Priors::Values(vec![0.2, 0.2, 0.2, 0.4]));
    let s = a.save.as_ref().unwrap();
    assert_eq!(s.class, Some(Some("PCLASS".into())));
    assert_eq!(s.scores, Some(Some("DSC".into())));
    let b = &d.analyses[1];
    assert_eq!(b.variables, Some(vec![(vec![VarItem::All], 1)]));
    assert_eq!(b.method, DiscriminantMethod::Rao);
    assert_eq!((b.vin, b.pin, b.pout), (0.4, Some(0.04), Some(0.09)));
    assert_eq!(b.functions, (Some(2), 80.0, 0.3));
    assert_eq!(b.priors, Priors::Size);
    let s = b.save.as_ref().unwrap();
    assert_eq!(
        (s.class.clone(), s.scores.clone()),
        (Some(None), Some(Some("SC".into())))
    );
    assert!(d.include && !d.history && d.separate && d.meansub && d.plot.map);
    assert!(!d.plot.combined && d.plot.cases.is_none());
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
fn discriminant_plot_keywords() {
    let plot = |source: &str| {
        discriminant(&format!(
            "DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b {source}."
        ))
        .plot
    };
    let all = DiscriminantPlots {
        combined: true,
        separate: true,
        map: true,
        cases: Some(None),
    };
    assert_eq!(plot(""), DiscriminantPlots::default());
    assert!(plot("/PLOT").combined);
    assert_eq!(plot("/PLOT").cases, Some(None));
    assert!(!plot("/PLOT").map);
    assert_eq!(plot("/PLOT=ALL"), all);
    assert_eq!(plot("/PLOT=SEPARATE CASES(15)").cases, Some(Some(15)));
    assert!(plot("/PLOT=SEPARATE CASES(15)").separate);
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
        "ANOVA VARIABLES=SCORE BY CLASS(1,6) GENDER,SCHOOL(1,2) /MAXORDERS=2 /STATISTICS=MEAN.",
    );
    assert_eq!(a.lists.len(), 1);
    let l = &a.lists[0];
    assert_eq!(l.dependents, [VarItem::Name("SCORE".into())]);
    assert_eq!(l.factors.len(), 2);
    assert_eq!(
        l.factors[1].0,
        [
            VarItem::Name("GENDER".into()),
            VarItem::Name("SCHOOL".into())
        ]
    );
    assert_eq!((l.factors[1].1, l.factors[1].2), (1.0, 2.0));
    assert_eq!(a.max_order, 2);
    assert!(a.mean && !a.reg && !a.mca);
    assert_eq!(a.method, AnovaMethod::Unique);
    assert_eq!(a.covariate_order, CovariateOrder::First);
    let a = anova(
        "ANOVA VARIABLES=SCORE BY CLASS(1,6) GENDER,SCHOOL(1,2) /WAGE BY GENDER,SCHOOL(1,2).",
    );
    assert_eq!(a.lists.len(), 2);
    assert_eq!(a.lists[1].dependents, [VarItem::Name("WAGE".into())]);
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

fn correspondence(source: &str) -> Correspondence {
    match one(source) {
        Command::Correspondence(c) => *c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn correspondence_defaults() {
    let c = correspondence("CORRESPONDENCE TABLE=shop(1,4) BY fruit(2,6).");
    assert_eq!(
        c.table,
        CorrespondenceTable::Variables {
            row: "shop".into(),
            row_range: (1.0, 4.0),
            column: "fruit".into(),
            column_range: (2.0, 6.0),
        }
    );
    assert_eq!(c.measure, CorrespondenceMeasure::Chisq);
    assert_eq!(c.standardize, CorrespondenceStandardize::Rcmean);
    assert_eq!(c.dimension, 2);
    assert_eq!(c.normalization, CorrespondenceNormalization::Q(0.0));
    assert_eq!(c.print, CorrespondencePrint::default());
    assert_eq!(c.plot.biplot, Some(None));
    assert_eq!(c.plot.ndim, None);
}

#[test]
fn correspondence_subcommands_and_abbreviations() {
    let c = correspondence(
        "CORRESPONDENCE /TABLE=ALL(6,3) /SUPP ROW(2) COLUMN(1,3) ROW(5) /EQUAL ROW(1,4)(3,6)
         /MEAS=EUCLID /STAN=CSUM /DIM=3 /NORM=-0.5
         /PRINT=RPROF CPROF RCONF PERM(2) /PLOT=NDIM(1,MAX) TRROWS(12) BIPLOT CPOINTS(80)
         /OUTFILE=SCORE('s.sav') VARIANCE(v).",
    );
    assert_eq!(
        c.table,
        CorrespondenceTable::All {
            rows: 6,
            columns: 3
        }
    );
    assert_eq!(c.supplementary_rows, [2.0, 5.0]);
    assert_eq!(c.supplementary_columns, [1.0, 3.0]);
    assert_eq!(c.equal_rows, [vec![1.0, 4.0], vec![3.0, 6.0]]);
    assert_eq!(c.measure, CorrespondenceMeasure::Euclid);
    assert_eq!(c.standardize, CorrespondenceStandardize::Csum);
    assert_eq!(c.dimension, 3);
    assert_eq!(c.normalization, CorrespondenceNormalization::Q(-0.5));
    assert!(c.print.rprofiles && c.print.cprofiles && c.print.rconf);
    assert!(!c.print.table && !c.print.rpoints && !c.print.cconf);
    assert_eq!(c.print.permutation, Some(2));
    assert_eq!(c.plot.ndim, Some((1, None)));
    assert_eq!(c.plot.trrows, Some(Some(12)));
    assert_eq!(c.plot.biplot, Some(None));
    assert_eq!(c.plot.cpoints, Some(Some(60)));
    assert_eq!(c.plot.rpoints, None);
    assert_eq!(c.outfile_score.as_deref(), Some("s.sav"));
    assert_eq!(c.outfile_variance.as_deref(), Some("v"));
    // the last of a repeated subcommand counts; PLOT NONE and PRINT NONE
    let c = correspondence(
        "CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /SUPPLEMENTARY a(1) /SUPPLEMENTARY b(2)
         /NORMALIZATION=PRINCIPAL /PRINT=NONE /PLOT=NONE.",
    );
    assert!(c.supplementary_rows.is_empty());
    assert_eq!(c.supplementary_columns, [2.0]);
    assert_eq!(c.normalization, CorrespondenceNormalization::Principal);
    assert!(!c.print.table && !c.print.rpoints && !c.print.cpoints);
    assert_eq!(c.plot.biplot, None);
    let c =
        correspondence("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /NORMALIZATION=RPRINCIPAL /PLOT.");
    assert_eq!(c.normalization, CorrespondenceNormalization::Q(1.0));
    assert_eq!(c.plot.biplot, Some(None));
}

#[test]
fn correspondence_errors() {
    assert!(err("CORRESPONDENCE /DIMENSION=2 /TABLE=a(1,2) BY b(1,2).").contains("starts with"));
    assert!(err("CORRESPONDENCE TABLE=a(3,1) BY b(1,2).").contains("smaller first"));
    assert!(err("CORRESPONDENCE TABLE=a(1.5,3) BY b(1,2).").contains("whole numbers"));
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY a(1,2).").contains("two different"));
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /EQUAL a(1).").contains("at least two"));
    assert!(
        err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /SUPPLEMENTARY c(1).").contains("unexpected")
    );
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /STANDARDIZE=RSUM.").contains("EUCLID"));
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /NORMALIZATION=1.5.").contains("-1 to 1"));
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /DIMENSION=0.").contains("at least 1"));
    assert!(err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /PLOT=NDIM(2,2).").contains("larger last"));
    assert!(
        err("CORRESPONDENCE TABLE=a(1,3) BY b(1,3) /OUTFILE=SCORE(x) VARIANCE(x).")
            .contains("different")
    );
}
