//! Parser tests of CLUSTER, PROXIMITIES, DISCRIMINANT, ANOVA, CORRESPONDENCE and MULTIPLE
//! CORRESPONDENCE.

use super::*;
use crate::VariableTransformation;

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
    // OUT, alone or with IN on one or two subcommands
    let c = cluster("CLUSTER a b /MATRIX=OUT(*).");
    assert_eq!(c.matrix_out, Some(None));
    let c = cluster("CLUSTER /MATRIX=IN(*) OUT('next.sav').");
    assert_eq!(
        (c.matrix_in, c.matrix_out),
        (Some(None), Some(Some("next.sav".into())))
    );
    let c = cluster("CLUSTER /MATRIX=OUT(m) /MATRIX=IN(*).");
    assert_eq!(
        (c.matrix_in, c.matrix_out),
        (Some(None), Some(Some("m".into())))
    );
}

#[test]
fn cluster_errors() {
    assert!(err("CLUSTER a b /MATRIX=OUT(*) OUT(m).").contains("OUT only once"));
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
fn discriminant_rotate_keywords() {
    use crate::discriminant::DiscriminantRotate::{Coeff, None, Structure};
    for (text, want) in [
        ("/ROTATE=COEFF", Coeff),
        ("/ROTATE=STRUCTURE", Structure),
        ("/ROTATE=NONE", None),
        ("", None),
    ] {
        let d = discriminant(&format!(
            "DISCRIMINANT GROUPS=g(1,3) /VARIABLES=a b {text}."
        ));
        assert_eq!(d.rotate, want, "{text}");
    }
}

#[test]
fn discriminant_matrix_out() {
    let d = discriminant("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /MATRIX=OUT(*).");
    assert_eq!(d.matrix.output, Some(None));
    let d = discriminant("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /MATRIX=OUT('m.sav').");
    assert_eq!(d.matrix.output, Some(Some("m.sav".into())));
}

#[test]
fn discriminant_matrix_in() {
    let d = discriminant("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /MATRIX=IN(*).");
    assert_eq!(d.matrix.input, Some(None));
    let d = discriminant("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a b /MATRIX=IN(m) OUT('o.sav').");
    assert_eq!(d.matrix.input, Some(Some("m".into())));
    assert_eq!(d.matrix.output, Some(Some("o.sav".into())));
}

#[test]
fn discriminant_errors() {
    assert!(err("DISCRIMINANT /VARIABLES=a b.").contains("GROUPS"));
    assert!(err("DISCRIMINANT GROUPS=g(1,2).").contains("VARIABLES"));
    assert!(err("DISCRIMINANT GROUPS=g(2,1) /VARIABLES=a.").contains("smaller first"));
    assert!(
        err("DISCRIMINANT GROUPS=g(1,2) /VARIABLES=a /ROTATE=VARIMAX.").contains("word VARIMAX")
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
    assert_eq!(c.dimension, None);
    assert_eq!(c.normalization, CorrespondenceNormalization::Q(0.0));
    assert_eq!(c.print, CorrespondencePrint::default());
    assert_eq!(c.plot.biplot, Some(None));
    assert_eq!(c.plot.ndim, None);
}

#[test]
fn correspondence_subcommands_and_abbreviations() {
    let c = correspondence(
        "CORRESPONDENCE /TABLE=ALL(6,3) /SUPP ROW(2) COL(1,3) ROW(5) /EQUAL ROW(1,4)(3,6)
         /MEAS=EUCLID /STAN=CSU /DIM=3 /NORM=-0.5
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
    assert_eq!(c.dimension, Some(3));
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

fn proximities(source: &str) -> Proximities {
    match one(source) {
        Command::Proximities(p) => *p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn proximities_defaults_and_keywords() {
    let p = proximities("PROXIMITIES a b c.");
    assert_eq!(p.measure_or_default(), Proximity::Euclid);
    assert!(p.print && !p.view_variables && p.standardize.is_none());
    let p = proximities(
        "PROXIMITIES a TO c /VIEW=VARIABLE /STANDARDIZE=CASE MAX /MEASURE=RESCALE POWER(3,2) ABS \
         /PRINT=NONE /ID=name /MISSING=INCLUDE /MATRIX=OUT(*) IN('m.sav').",
    );
    assert!(p.view_variables && !p.print && p.include && p.absolute && p.rescale && !p.reverse);
    assert_eq!(p.standardize, Some((Standardize::Max, true)));
    assert_eq!(p.measure, Some(Proximity::Power(3.0, 2.0)));
    assert_eq!(p.matrix_out, Some(None));
    assert_eq!(p.matrix_in, Some(Some("m.sav".into())));
    // STANDARDIZE alone means Z by variable; Z and SD are written in full
    assert_eq!(
        proximities("PROXIMITIES a b /STANDARDIZE.").standardize,
        Some((Standardize::Z, false))
    );
    assert_eq!(
        proximities("PROXIMITIES a b /STANDARDIZE=SD.").standardize,
        Some((Standardize::Sd, false))
    );
    let p = proximities("PROXIMITIES a b /MEASURE=DEFAULT.");
    assert_eq!(
        (p.measure, p.measure_or_default()),
        (None, Proximity::Euclid)
    );
    let p = proximities("PROXIMITIES /MATRIX=IN(*) /MEASURE=NONE REVERSE.");
    assert!(p.no_measure && p.reverse && p.variables.is_empty());
    assert_eq!(
        proximities("PROXIMITIES a b /MEASURE=SM(2)").measure,
        Some(Proximity::Binary(BinaryMeasure::Sm, 2.0, None))
    );
}

#[test]
fn proximities_errors() {
    assert!(err("PROXIMITIES a b /MEASURE=EUCLID BLOCK.").contains("one measure"));
    assert!(err("PROXIMITIES a b /MEASURE=NONE.").contains("MATRIX=IN"));
    assert!(err("PROXIMITIES /MEASURE=EUCLID.").contains("variable list"));
    assert!(err("PROXIMITIES a b /STANDARDIZE=Z /MEASURE=JACCARD.").contains("binary"));
    assert!(err("PROXIMITIES a b /STANDARDIZE=Z RANGE.").contains("one method"));
}

fn proxscal(source: &str) -> Proxscal {
    match one(source) {
        Command::Proxscal(p) => *p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn proxscal_defaults_and_keywords() {
    let p = proxscal("PROXSCAL VARIABLES=a TO e.");
    assert_eq!(p.dimensions, (2, 2));
    assert_eq!(p.transformation, ProxTransformation::Ratio);
    assert_eq!(p.initial, ScalingStart::Simplex);
    assert!(p.print.common && p.print.stress && p.plot.common && p.acceleration);
    let p = proxscal(
        "PROXSCAL a b c d /TABLE=r BY c BY s /SHAPE=BOTH /INITIAL=RANDOM(5) /WEIGHTS=w1 TO w4 \
         /CONDITION=UNCONDITIONAL /TRANSFORMATION=SPLINE DEGREE=3 INKNOT=2 /PROXIMITIES=SIMILARITIES \
         /MODEL=REDUCED(1) /ACCELERATION=NONE /CRITERIA=DIMENSIONS(1,3) MAXITER(50) DIFFSTRESS(.001) \
         /PRINT=HISTORY DISTANCES /PLOT=TRANSFORMATIONS(src1 src2)(src3) RESIDUALS(ALL) \
         /OUTFILE=COMMON(space) DISTANCES('d.sav').",
    );
    let table = p.table.unwrap();
    assert_eq!(
        (table.row.as_deref(), table.source.as_deref()),
        (Some("r"), Some("s"))
    );
    assert_eq!(p.shape, ProxShape::Both);
    assert_eq!(p.initial, ScalingStart::Random(5));
    assert!(p.unconditional && p.similarities && !p.acceleration);
    assert_eq!(
        p.transformation,
        ProxTransformation::Spline {
            degree: 3,
            inknot: 2
        }
    );
    assert_eq!(p.model, ScalingModel::Reduced(1));
    assert_eq!((p.dimensions, p.max_iterations), ((1, 3), 50));
    assert!(p.print.history && p.print.distances && !p.print.common && !p.print.stress);
    let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        p.plot.transformations,
        Some(vec![names(&["src1", "src2"]), names(&["src3"])])
    );
    assert_eq!(p.plot.residuals, Some(vec![Vec::new()]));
    assert!(!p.plot.common);
    assert_eq!(p.outfile.common.as_deref(), Some("space"));
    assert_eq!(p.outfile.distances.as_deref(), Some("d.sav"));
    assert_eq!(
        proxscal("PROXSCAL a b c /TRANSFORMATION=ORDINAL(UNTIE)").transformation,
        ProxTransformation::Ordinal { untie: true }
    );
    assert_eq!(proxscal("PROXSCAL /MATRIX=IN(*).").matrix_in, Some(None));
    assert_eq!(proxscal("PROXSCAL a b c.").restrict_coordinates, None);
    let c = proxscal("PROXSCAL a b c /RESTRICTIONS=COORDINATES('f.sav') u TO v.")
        .restrict_coordinates
        .unwrap();
    assert_eq!((c.file.as_str(), c.variables.len()), ("f.sav", 1));
    let c = proxscal("PROXSCAL a b c /RESTRICTIONS=COORDINATES(set) ALL.")
        .restrict_coordinates
        .unwrap();
    assert_eq!((c.file.as_str(), c.variables.len()), ("set", 0));
    let c = proxscal(
        "PROXSCAL a b c /RESTRICTIONS=VARIABLES('v.sav') x y z (ORDINAL(UNTIE) NOMINAL SPLINE DEGREE=3 INKNOT=2) \
         /PRINT=VARIABLES /OUTFILE=VARIABLES(q).",
    );
    let v = c.restrict_variables.unwrap();
    assert_eq!(
        (v.file.as_str(), v.variables.len(), v.first),
        ("v.sav", 3, None)
    );
    assert_eq!(
        v.kinds,
        [
            VariableTransformation::Ordinal { untie: true },
            VariableTransformation::Nominal,
            VariableTransformation::Spline {
                degree: 3,
                inknot: 2
            },
        ]
    );
    assert!(c.print.variables && !c.print.correlations);
    assert_eq!(c.outfile.variables.as_deref(), Some("q"));
    let v = proxscal("PROXSCAL a b c /RESTRICTIONS=VARIABLES(f) FIRST(2) (INTERVAL).")
        .restrict_variables
        .unwrap();
    assert_eq!(
        (v.first, v.kinds),
        (Some(2), vec![VariableTransformation::Interval])
    );
    let v = proxscal("PROXSCAL a b c /RESTRICTIONS=VARIABLES(f) ALL.")
        .restrict_variables
        .unwrap();
    assert!(v.variables.is_empty() && v.first.is_none() && v.kinds.is_empty());
    // FIRST and ALL are keywords only where the syntax has them
    let v = proxscal("PROXSCAL a b c /RESTRICTIONS=VARIABLES(f) first all.")
        .restrict_variables
        .unwrap();
    assert!(v.first.is_none() && v.variables.len() == 2);
    let v = proxscal("PROXSCAL a b c /RESTRICTIONS=VARIABLES(f) ALL (NOMINAL) /PRINT=NONE.")
        .restrict_variables
        .unwrap();
    assert!(v.variables.is_empty() && v.kinds == [VariableTransformation::Nominal]);
    // the last RESTRICTIONS wins
    let c = proxscal("PROXSCAL a b c /RESTRICTIONS=VARIABLES(f) /RESTRICTIONS=COORDINATES(g).");
    assert!(c.restrict_variables.is_none() && c.restrict_coordinates.is_some());
    let c = proxscal("PROXSCAL a b c /PRINT=NONE.");
    assert!(!c.print.variables && !c.print.correlations);
    let c = proxscal("PROXSCAL a b c.");
    assert!(c.print.variables && c.print.correlations);
    let ScalingStart::File(f) = proxscal("PROXSCAL a b c /INITIAL=('s.sav') u v.").initial else {
        panic!("a file start")
    };
    assert_eq!((f.file.as_str(), f.variables.len()), ("s.sav", 2));
    let ScalingStart::File(f) = proxscal("PROXSCAL a b c /INITIAL=(set).").initial else {
        panic!("a file start")
    };
    assert!(f.variables.is_empty());
    assert!(err("PROXSCAL a b c /INITIAL=CENTROIDS.").contains("word CENTROIDS"));
    let c = proxscal("PROXSCAL a b c.");
    assert!(c.plot.correlations && c.plot.common && c.plot.variables.is_none());
    let c = proxscal("PROXSCAL a b c /PLOT=VARIABLES(x y) CORRELATIONS.");
    assert_eq!(
        c.plot.variables,
        Some(vec!["x".to_string(), "y".to_string()])
    );
    assert!(c.plot.correlations && !c.plot.common);
    let c = proxscal("PROXSCAL a b c /PLOT=VARIABLES(ALL).");
    assert_eq!(c.plot.variables, Some(Vec::new()));
    assert!(!c.plot.correlations);
}

#[test]
fn proxscal_errors() {
    assert!(err("PROXSCAL a b c /CRITERIA=DIMENSIONS(3,2).").contains("above"));
    assert!(err("PROXSCAL a b c /CRITERIA=DIFFSTRESS(2).").contains("from 0 to 1"));
    assert!(err("PROXSCAL a b c /TRANSFORMATION=SPLINE DEGREE=4.").contains("1 to 3"));
    assert!(err("PROXSCAL a b c /MODEL=REDUCED(2).").contains("rank"));
    assert!(err("PROXSCAL a b c /RESTRICTIONS=PLANE('f.sav').").contains("word PLANE"));
    assert!(err("PROXSCAL a b c /RESTRICTIONS=VARIABLES('f.sav') (RATIO).").contains("word RATIO"));
    assert!(err("PROXSCAL a b c /RESTRICTIONS=VARIABLES('f.sav') ().").contains("unexpected"));
    assert!(err("PROXSCAL /SHAPE=UPPER.").contains("variable list"));
}

fn alscal(source: &str) -> Alscal {
    match one(source) {
        Command::Alscal(a) => *a,
        other => panic!("{other:?}"),
    }
}

#[test]
fn alscal_defaults_and_keywords() {
    let a = alscal("ALSCAL VARIABLES=a TO f.");
    assert_eq!(
        a.level,
        AlscalLevel::Ordinal {
            untie: false,
            similar: false
        }
    );
    assert_eq!(
        (a.dimensions, a.iterations, a.converge, a.stress_min),
        ((2, 2), 30, 0.001, 0.005)
    );
    assert!(a.plot.is_none() && !a.indscal);
    let a = alscal(
        "ALSCAL VAR=a TO f /LEVEL=ORDINAL(UNTIE SIMILAR) /CONDITION=UNCONDITIONAL /METHOD=INDSCAL \
         /CRITERIA=NEGATIVE CUTOFF(1) CONVERGE(0) ITER(50) STRESSMIN(.01) DIMENS(2,4) NOULB \
         /PRINT=DATA HEADER /PLOT=ALL /OUTFILE=conf.",
    );
    assert_eq!(
        a.level,
        AlscalLevel::Ordinal {
            untie: true,
            similar: true
        }
    );
    assert!(a.unconditional && a.indscal && a.negative && a.print_data && a.print_header);
    assert_eq!((a.cutoff, a.iterations, a.dimensions), (1.0, 50, (2, 4)));
    assert_eq!(a.plot, Some(true));
    assert_eq!(a.outfile.as_deref(), Some("conf"));
    assert_eq!(
        alscal("ALSCAL VARIABLES=a b c d /LEVEL=INTERVAL(2)").level,
        AlscalLevel::Interval(2)
    );
    assert_eq!(
        alscal("ALSCAL VARIABLES=a b c d /LEVEL=RATIO").level,
        AlscalLevel::Ratio(1)
    );
    assert_eq!(alscal("ALSCAL VARIABLES=a b c d /PLOT").plot, Some(false));
    assert_eq!(alscal("ALSCAL /MATRIX=IN(*).").matrix_in, Some(None));
}

#[test]
fn alscal_file() {
    assert_eq!(alscal("ALSCAL VARIABLES=a b c d.").file, None);
    let f = alscal("ALSCAL VARIABLES=a b c d /FILE=start.")
        .file
        .unwrap();
    assert_eq!(
        (f.name.as_str(), f.config, f.weights, f.listed),
        ("start", true, false, false)
    );
    let f = alscal("ALSCAL VARIABLES=a b c d /FILE='s.sav' CONFIG(INITIAL) SUBJWGHT.")
        .file
        .unwrap();
    assert_eq!(
        (f.name.as_str(), f.config, f.weights, f.listed),
        ("s.sav", true, true, true)
    );
    let f = alscal("ALSCAL VARIABLES=a b c d /FILE=start SUBJWGHT.")
        .file
        .unwrap();
    assert!(!f.config && f.weights);
}

#[test]
fn alscal_errors() {
    assert!(err("ALSCAL VARIABLES=a b c d /SHAPE=RECTANGULAR.").contains("not supported"));
    assert!(alscal("ALSCAL VARIABLES=a b c d /SHAPE=ASYMMETRIC.").asymmetric);
    assert!(!alscal("ALSCAL VARIABLES=a b c d /SHAPE=ASYMMETRIC /SHAPE=SYMMETRIC.").asymmetric);
    assert!(!alscal("ALSCAL VARIABLES=a b c d.").asymmetric);
    assert!(err("ALSCAL VARIABLES=a b c d /MODEL=GEMSCAL.").contains("not supported"));
    assert!(err("ALSCAL VARIABLES=a b c d /CONDITION=ROW.").contains("not supported"));
    assert!(err("ALSCAL VARIABLES=a b c d /FILE=start CONFIG(FIXED).").contains("FIXED"));
    assert!(err("ALSCAL VARIABLES=a b c d /FILE=start STIMWGHT.").contains("STIMWGHT"));
    assert!(err("ALSCAL VARIABLES=a b c d /LEVEL=RATIO(5).").contains("1 to 4"));
    assert!(err("ALSCAL VARIABLES=a b c d /CRITERIA=DIMENS(7).").contains("1 to 6"));
    assert!(
        err("ALSCAL VARIABLES=a b c d /MODEL=INDSCAL /CRITERIA=DIMENS(1).")
            .contains("2 dimensions")
    );
    assert!(err("ALSCAL /LEVEL=RATIO.").contains("VARIABLES"));
}

fn mca(source: &str) -> MultipleCorrespondence {
    match one(source) {
        Command::MultipleCorrespondence(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn name(n: &str) -> VarItem {
    VarItem::Name(n.into())
}

#[test]
fn multiple_correspondence_defaults() {
    let c = mca("MULTIPLE CORRESPONDENCE hue size shape /ANALYSIS=hue size shape.");
    assert_eq!(
        c.common.variables,
        vec![name("hue"), name("size"), name("shape")]
    );
    assert_eq!(c.analysis.len(), 1);
    assert_eq!(c.analysis[0].1, 1.0);
    assert_eq!(c.common.dimension, 2);
    assert_eq!(c.common.normalization, McaNormalization::Q(-1.0));
    assert_eq!(
        (c.common.max_iterations, c.common.criterion),
        (100, 0.00001)
    );
    assert_eq!(c.print, McaPrint::defaults());
    assert_eq!(c.plot, McaPlots::default());
    assert!(c.save_trdata.is_none() && c.outfile_object.is_none());
}

#[test]
fn multiple_correspondence_subcommands_and_abbreviations() {
    let c = mca("MULT CORR /VARIABLES=a TO f /ANAL=a b (WEIGHT=3) c d e
         /DISC=b (GROUPING NCAT=4 DISTR=UNIFORM) c (EQINTV=2.5) d (MULT) e
         /MISS=a (ACTIVE EXTRACAT) b c (LISTWISE) d (PASSIVE EXTRACAT) e (ACT RANDIMPU)
         /SUPP=OBJECT(2 5 TO 7) VARIABLE(e)
         /CONFIG=FIXED('start.sav') /DIM=3 /NORM=0.4 /MAXITER=40 /CRITITER=.001
         /PRINT=QUANT(a b) OBJECT((f) a TO c) HIST NONE DESCRIP OCORR
         /PLOT=CATEGORIES(a) TRANS(a b(2) c) RESID(d(3))(10) BIPLOT((a b))(f)(0) NDIM(1,3)
         /SAVE=TRDATA(QUANTITY(2)) OBJECT /OUTFILE=TRDATA('t.sav') OBJECT(scores).");
    assert_eq!(c.analysis[0], (vec![name("a"), name("b")], 3.0));
    assert_eq!(c.analysis[1].1, 1.0);
    assert_eq!(
        c.common.discretization,
        vec![
            (
                vec![name("b")],
                Discretize::Grouping {
                    ncat: Some(4),
                    normal: false
                }
            ),
            (vec![name("c")], Discretize::Intervals(2.5)),
            (vec![name("d")], Discretize::Multiplying),
            (
                vec![name("e")],
                Discretize::Grouping {
                    ncat: None,
                    normal: true
                }
            ),
        ]
    );
    assert_eq!(
        c.common.missing[0].1,
        McaMissing::Active(Impute::ExtraCategory)
    );
    assert_eq!(
        c.common.missing[1],
        (vec![name("b"), name("c")], McaMissing::Listwise)
    );
    assert_eq!(
        c.common.missing[2].1,
        McaMissing::Passive(Impute::ExtraCategory)
    );
    assert_eq!(c.common.missing[3].1, McaMissing::Active(Impute::Random));
    assert_eq!(c.common.supplementary_objects, vec![(2, 2), (5, 7)]);
    assert_eq!(c.common.supplementary_variables, vec![name("e")]);
    assert_eq!(c.common.configuration, Some((true, "start.sav".into())));
    assert_eq!((c.common.dimension, c.common.max_iterations), (3, 40));
    assert_eq!(c.common.normalization, McaNormalization::Q(0.4));
    // NONE drops QUANT, OBJECT and HISTORY
    assert_eq!(
        c.print,
        McaPrint {
            descrip: Some(Vec::new()),
            ocorr: true,
            ..McaPrint::default()
        }
    );
    assert_eq!(c.plot.ndim, Some((1, 3)));
    assert_eq!(c.plot.charts[0], (McaPlot::Category(vec![name("a")]), None));
    assert_eq!(
        c.plot.charts[1],
        (
            McaPlot::Trans(vec![(name("a"), 1), (name("b"), 2), (name("c"), 1)]),
            None
        )
    );
    assert_eq!(
        c.plot.charts[2],
        (McaPlot::Resid(vec![(name("d"), 3)]), Some(10))
    );
    assert_eq!(
        c.plot.charts[3],
        (
            McaPlot::Biplot(vec![name("a"), name("b")], vec![name("f")]),
            Some(0)
        )
    );
    // roots are cut to three and five characters
    assert_eq!(c.save_trdata, Some(("QUA".into(), Some(2))));
    assert_eq!(c.save_object, Some(("OBSCO".into(), None)));
    assert_eq!(c.outfile_trdata.as_deref(), Some("t.sav"));
    assert_eq!(c.outfile_object.as_deref(), Some("scores"));
}

#[test]
fn multiple_correspondence_print_and_plot_keyword_rules() {
    let c = mca("MULTIPLE CORRESPONDENCE x y /ANALYSIS x y /PRINT /PLOT.");
    assert_eq!(c.print, McaPrint::defaults());
    assert_eq!(c.plot, McaPlots::default());
    let c = mca(
        "MULTIPLE CORRESPONDENCE x y /ANALYSIS x y /PRINT=DISCRIM DISCRIM NONE CORR /PLOT=NONE.",
    );
    assert_eq!(
        c.print,
        McaPrint {
            corr: true,
            ..McaPrint::default()
        }
    );
    assert!(c.plot.charts.is_empty());
    let c = mca(
        "MULTIPLE CORRESPONDENCE x y /ANALYSIS x y /PLOT=OBJECT(x)(20) JOINTCAT NONE DISCRIM(5).",
    );
    assert_eq!(c.plot.charts, vec![(McaPlot::Discrim(Vec::new()), Some(5))]);
}

#[test]
fn multiple_correspondence_errors() {
    assert!(err("MULTIPLE CORRESPONDENCE x y.").contains("ANALYSIS"));
    assert!(err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /DIM=2 /DIM=3.").contains("twice"));
    assert!(err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /PRINT=QUANT QUANT.").contains("twice"));
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /PLOT=RESID(x) RESID(y).").contains("twice")
    );
    assert!(err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /PLOT=TRANS.").contains("unexpected"));
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /NORMALIZATION=1.5.").contains("-1 to 1")
    );
    assert!(err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y (WEIGHT=0).").contains("positive"));
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /MISSING=ALL x (ACTIVE).").contains("ALL")
    );
    assert!(err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /CRITITER=0.").contains("positive"));
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /OUTFILE=OBJECT(*).")
            .contains("active dataset")
    );
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /OUTFILE=OBJECT('a') TRDATA('A').")
            .contains("its own file")
    );
    assert!(
        err("MULTIPLE CORRESPONDENCE x y /ANALYSIS=x y /DISC=x(EQINTV=0).").contains("positive")
    );
}

fn catpca(source: &str) -> Catpca {
    match one(source) {
        Command::Catpca(c) => *c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn catpca_defaults() {
    let c = catpca("CATPCA VARIABLES=pace crowd /ANALYSIS=pace crowd.");
    assert_eq!(c.common.variables, vec![name("pace"), name("crowd")]);
    assert_eq!(c.analysis[0].1, CatpcaVariable::default());
    assert_eq!(
        c.analysis[0].1.level,
        ScalingLevel::SplineOrdinal {
            degree: 2,
            inknot: 2
        }
    );
    assert_eq!(c.common.dimension, 2);
    assert_eq!(c.print, CatpcaPrint::defaults());
    assert_eq!(c.plot, CatpcaPlots::default());
    assert!(c.rotation.is_none() && c.resample.is_none() && c.ellipses.is_empty());
}

#[test]
fn catpca_subcommands_and_abbreviations() {
    let c = catpca(
        "CATPCA pace TO shade /ANAL=pace crowd (WEIGHT=2 LEVEL=ORDI) parking (LEV=SPNOM DEGREE=3)
           shade (INKNOT=0) light (LEVEL=MNOM) noise (LEVEL=NUME) view (LEVEL=NOMI)
         /DISC=pace (RANKING) /MISS=shade (ACTIVE EXTRACAT) /SUPP=OBJECT(3) VARIABLE(view)
         /DIM=3 /NORM=SYMMETRICAL /ROT=PROMAX(2) NOKAISER
         /PRINT=LOADING(SORT) VAF QUANT(pace) OBJECT((light) pace) HIST
         /PLOT=LOADING(pace crowd (CENTR(light)))(10) BIPLOT(CENTR(light))(pace) TRIPLOT(pace crowd (shade))
           PROJCENTR(light, pace crowd) TRANS(light(2)) VAF NDIM(1,3) OBELLAREA(> STDEV 2) OBJECT OBJECT(pace)
         /SAVE=TRDATA(QUANTITIES(2)) APPROX(FIT) LDELLAREA
         /OUTFILE='trans.sav' OBJECT(scores).",
    );
    assert_eq!(c.analysis.len(), 6);
    assert_eq!(c.analysis[0].0, vec![name("pace"), name("crowd")]);
    assert_eq!(c.analysis[0].1.weight, 2.0);
    assert_eq!(c.analysis[0].1.level, ScalingLevel::Ordinal);
    assert_eq!(
        c.analysis[1].1.level,
        ScalingLevel::SplineNominal {
            degree: 3,
            inknot: 2
        }
    );
    assert_eq!(
        c.analysis[2].1.level,
        ScalingLevel::SplineOrdinal {
            degree: 2,
            inknot: 0
        }
    );
    assert_eq!(c.analysis[3].1.level, ScalingLevel::MultipleNominal);
    assert_eq!(c.analysis[4].1.level, ScalingLevel::Numeric);
    assert_eq!(c.analysis[5].1.level, ScalingLevel::Nominal);
    assert_eq!(c.common.discretization[0].1, Discretize::Ranking);
    assert_eq!(c.common.normalization, McaNormalization::Q(0.0));
    assert_eq!(c.rotation, Some((Rotation::Promax(2.0), false)));
    assert_eq!(c.print.loading, Some(true));
    assert!(c.print.vaf && c.print.history && !c.print.corr);
    assert_eq!(
        c.print.object,
        Some((Some("light".into()), vec![name("pace")]))
    );
    // a repeated keyword replaces the earlier one
    assert_eq!(
        c.plot.charts[0],
        (
            CatpcaPlot::Loading(vec![name("pace"), name("crowd")], Some(vec![name("light")])),
            Some(10)
        )
    );
    assert_eq!(
        c.plot.charts[1].0,
        CatpcaPlot::Biplot {
            centroids: true,
            variables: vec![name("light")],
            labels: vec![name("pace")]
        }
    );
    assert_eq!(
        c.plot.charts[2].0,
        CatpcaPlot::Triplot(vec![name("pace"), name("crowd")], vec![name("shade")])
    );
    assert_eq!(
        c.plot.charts[3].0,
        CatpcaPlot::Projcentr("light".into(), vec![name("pace"), name("crowd")])
    );
    assert_eq!(
        c.plot.charts[4].0,
        CatpcaPlot::Trans(vec![(name("light"), 2)])
    );
    assert_eq!(c.plot.charts[5].0, CatpcaPlot::Vaf);
    assert_eq!(c.plot.charts[6].0, CatpcaPlot::Object(vec![name("pace")]));
    assert_eq!(c.plot.charts.len(), 7);
    assert_eq!(c.plot.ndim, Some((1, 3)));
    assert_eq!(c.save_trdata, Some(("QUANT".into(), Some(2))));
    assert_eq!(c.save_approx.as_deref(), Some("FIT"));
    assert_eq!(c.ellipses, vec!["PLOT OBELLAREA", "SAVE LDELLAREA"]);
    assert_eq!(c.outfile_trdata.as_deref(), Some("trans.sav"));
    assert_eq!(c.outfile_object.as_deref(), Some("scores"));
}

#[test]
fn catpca_print_plot_and_resample_rules() {
    let c = catpca(
        "CATPCA a b /ANALYSIS a b /PRINT=VAF VAF NONE CORR /PLOT=RESID(a) NONE LOADING((CENTR)).",
    );
    assert_eq!(
        c.print,
        CatpcaPrint {
            corr: true,
            ..CatpcaPrint::default()
        }
    );
    assert_eq!(
        c.plot.charts,
        vec![(CatpcaPlot::Loading(Vec::new(), Some(Vec::new())), None)]
    );
    let c = catpca("CATPCA a b /ANALYSIS a b /RESAMPLE=BOOTSTRAP(5000, REFLEC).");
    let b = c.resample.unwrap();
    assert_eq!(
        (b.samples, b.percent, b.balanced, b.procrustes),
        (5000, 95.0, true, false)
    );
    let c = catpca("CATPCA a b /ANALYSIS a b /RESAMPLE=BOOT(200 90 UNBALANCED).");
    let b = c.resample.unwrap();
    assert_eq!((b.samples, b.percent, b.balanced), (200, 90.0, false));
}

#[test]
fn catpca_errors() {
    assert!(err("CATPCA a b.").contains("ANALYSIS"));
    assert!(err("CATPCA a b /ANALYSIS=a b (LEVEL=ORDI DEGREE=2).").contains("SPORD"));
    assert!(err("CATPCA a b /ANALYSIS=a b (LEVEL=CURVED).").contains("unexpected"));
    assert!(
        err("CATPCA a b /ANALYSIS=a b /ROTATION=VARIMAX /RESAMPLE=BOOTSTRAP.").contains("rotate")
    );
    assert!(err("CATPCA a b /ANALYSIS=a b /SAVE=APPROX(fit(2)).").contains("root name only"));
    let c = catpca("CATPCA a b /ANALYSIS=a b /ROTATION=VARIMAX /RESAMPLE=NONE.");
    assert!(c.rotation.is_some() && c.resample.is_none());
    assert!(err("CATPCA a b /ANALYSIS=a b /ROTATION=PROMAX(0.5).").contains("kappa"));
    assert!(err("CATPCA a b /ANALYSIS=a b /ROTATION=OBLIMIN(0.9).").contains("delta"));
    assert!(err("CATPCA a b /ANALYSIS=a b /RESAMPLE=BOOTSTRAP(10 100).").contains("confidence"));
    assert!(err("CATPCA a b /ANALYSIS=a b /PRINT=QUANT QUANT.").contains("twice"));
    assert!(err("CATPCA a b /ANALYSIS=a b /PLOT=CATEGORY.").contains("unexpected"));
    assert!(err("CATPCA a b /ANALYSIS=a b /OUTFILE=OBJECT('x') APPROX('X').").contains("its own"));
    assert!(err("CATPCA a b /ANALYSIS=a b /DIM=2 /DIMENSION=3.").contains("twice"));
}
