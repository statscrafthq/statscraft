//! Parser tests of FACTOR and QUICK CLUSTER.

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

fn factor(source: &str) -> Factor {
    match one(source) {
        Command::Factor(f) => *f,
        other => panic!("{other:?}"),
    }
}

fn cluster(source: &str) -> QuickCluster {
    match one(source) {
        Command::QuickCluster(q) => *q,
        other => panic!("{other:?}"),
    }
}

#[test]
fn factor_defaults() {
    let f = factor("FACTOR VARIABLES=v1 TO v12.");
    assert_eq!(f.variables, [VarItem::Range("v1".into(), "v12".into())]);
    assert_eq!(f.print, FactorPrint::DEFAULT);
    assert_eq!(f.extraction, Extraction::PrincipalComponents);
    assert_eq!(f.rotation, Rotation::Varimax);
    assert_eq!(f.extraction_criteria, FactorCriteria::default());
    assert_eq!(f.missing, FactorMissing::Listwise);
    assert!(f.save.is_none() && f.ignored.is_empty());
}

#[test]
fn factor_abbreviations_and_aliases() {
    let f = factor(
        "FACTOR VARS=item1 TO item9 /SELECT=WAVE ('b') /MISS=MEANSUB /PRINT=DEF REPR KMO AIC \
         /EXTRACT=ULS /ROTATE=VARIMAX.",
    );
    assert_eq!(f.select, Some(("WAVE".into(), Value::String("b".into()))));
    assert_eq!(f.missing, FactorMissing::MeanSubstitution);
    let p = f.print;
    assert!(p.initial && p.extraction && p.rotation && p.aic && p.kmo && p.repr && !p.sig);
    assert_eq!(f.extraction, Extraction::UnweightedLeastSquares);
    assert_eq!(f.rotation, Rotation::Varimax);
    let f = factor("FACTOR VAR=a b c /EXTRACTION=PA2 /ROTATION=OBLIMIN(-.5).");
    assert_eq!(f.extraction, Extraction::PrincipalAxis);
    assert_eq!(f.rotation, Rotation::Oblimin(-0.5));
    assert_eq!(
        factor("FACTOR VAR=a b c /EXTRACTION=PA1.").extraction,
        Extraction::PrincipalComponents
    );
}

#[test]
fn factor_subcommand_order_rules() {
    // EXTRACTION without ROTATION: not rotated
    let f = factor("FACTOR VARIABLES=a b c /EXTRACTION=PAF.");
    assert_eq!(f.rotation, Rotation::None);
    // criteria apply to what follows them
    let f = factor(
        "FACTOR VARIABLES=a b c d /CRITERIA=FACTORS(2) ITERATE(50) /EXTRACTION=ML \
         /CRITERIA=RCONVERGE(.001) NOKAISER KAPPA(3) /ROTATION=PROMAX /CRITERIA=FACTORS(3).",
    );
    assert_eq!(f.extraction_criteria.factors, Some(2));
    assert_eq!(f.extraction_criteria.iterate, 50);
    assert_eq!(f.extraction_criteria.rconverge, 0.0001);
    assert_eq!(f.rotation, Rotation::Promax(3.0));
    assert!(!f.rotation_criteria.kaiser);
    assert_eq!(f.rotation_criteria.factors, Some(2));
    // without EXTRACTION the criteria at the end hold, and SAVE takes ( n, root )
    let f = factor("FACTOR VARIABLES=a b c d /ROTATION /SAVE REG (4,PCOMP) /CRITERIA FACTORS(4).");
    assert_eq!(f.extraction_criteria.factors, Some(4));
    assert_eq!(
        f.save,
        Some(FactorSave {
            method: ScoreMethod::Regression,
            count: Some(4),
            root: Some("PCOMP".into())
        })
    );
    // ANALYSIS cancels what came before it; EXTRACTION cancels ROTATION and SAVE
    let f = factor(
        "FACTOR VARIABLES=a b c d /PRINT=KMO /EXTRACTION=PAF /ANALYSIS=a TO c \
         /ROTATION=QUARTIMAX /SAVE=AR(ALL) /EXTRACTION=ML.",
    );
    assert_eq!(f.print, FactorPrint::DEFAULT);
    assert_eq!(f.extraction, Extraction::MaximumLikelihood);
    assert_eq!(f.rotation, Rotation::None);
    assert_eq!(f.ignored, ["PRINT", "EXTRACTION", "ROTATION", "SAVE"]);
    let f = factor("FACTOR VARIABLES=a b /SAVE (2) /FORMAT=SORT BLANK(.3) /DIAGONAL=.5 2*.4.");
    assert_eq!(f.save.unwrap().count, Some(2));
    assert!(f.sort && f.blank == Some(0.3));
    assert_eq!(f.diagonal, Some(vec![0.5, 0.4, 0.4]));
    let f = factor("FACTOR MATRIX=IN(COR=*) /ANALYSIS=a b.");
    assert_eq!(f.matrix_in, Some((MatrixKind::Correlation, None)));
}

#[test]
fn factor_errors() {
    assert_eq!(
        err("FACTOR VARIABLES=a b /ANALYSIS=a /MISSING=PAIRWISE."),
        "MISSING must come before ANALYSIS"
    );
    assert_eq!(
        err("FACTOR VARIABLES=a b /METHOD=COVARIANCE."),
        "COVARIANCE in FACTOR /METHOD is not supported yet"
    );
    assert!(err("FACTOR VARIABLES=a b /ROTATION=OBLIMIN(1).").contains("at most 0.8"));
    assert!(err("FACTOR VARIABLES=a b /ROTATION=PROMAX(1).").contains("above 1"));
    assert!(err("FACTOR VARIABLES=a b /CRITERIA=FACTORS(0).").contains("positive whole"));
    assert_eq!(err("FACTOR /ANALYSIS=a b."), "unexpected word ANALYSIS");
    assert_eq!(
        err("FACTOR MATRIX=IN(COR=*) OUT(FAC=*)."),
        "OUT in FACTOR /MATRIX is not supported yet"
    );
}

#[test]
fn quick_cluster_defaults_and_options() {
    let q = cluster("QUICK CLUSTER v1 TO v4.");
    assert_eq!(q.clusters, 2);
    assert_eq!(q.max_iterations, 10);
    assert_eq!(q.method, ClusterMethod::KMeans { update: false });
    assert!(q.print_initial && !q.print_anova && q.save.is_empty());
    let q = cluster(
        "QUICK CLUSTER A B C D E /CRITERIA=CLUSTERS(6) NOINITIAL MXITER(20) CONVERGE(.02) \
         /METHOD=KMEANS(UPDATE) /PRINT=CLUSTER ID(CASEID) DISTANCE ANOVA \
         /MISSING=PAIRWISE INCLUDE /SAVE=DISTANCE CLUSTER(GROUP) /OUTFILE='qc.sav'.",
    );
    assert_eq!(q.clusters, 6);
    assert!(q.no_initial && q.pairwise && q.include);
    assert_eq!((q.max_iterations, q.converge), (20, 0.02));
    assert_eq!(q.method, ClusterMethod::KMeans { update: true });
    assert!(!q.print_initial && q.print_cluster && q.print_distance && q.print_anova);
    assert_eq!(q.id.as_deref(), Some("CASEID"));
    assert_eq!(
        q.save,
        [
            (ClusterSave::Distance, None),
            (ClusterSave::Cluster, Some("GROUP".into()))
        ]
    );
    assert_eq!(q.outfile.as_deref(), Some("qc.sav"));
    let q = cluster(
        "QUICK CLUSTER A B /CRITERIA = CLUSTER(3) /INITIAL = (13 24\n 7 12\n 10 18) /METHOD=CLASSIFY /PRINT=NONE ANOVA.",
    );
    assert_eq!(q.initial, Some(vec![13.0, 24.0, 7.0, 12.0, 10.0, 18.0]));
    assert_eq!(q.method, ClusterMethod::Classify);
    assert!(!q.print_initial && !q.print_anova);
    assert!(err("QUICK CLUSTER a b /SAVE=CLUSTER CLUSTER.").contains("once each"));
}
