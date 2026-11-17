//! Parser tests of KNN.

use super::*;

fn knn(source: &str) -> Knn {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Knn(k) => *k,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn knn_defaults() {
    let k = knn("KNN y BY a b WITH x.");
    assert_eq!(k.target.as_deref(), Some("y"));
    assert_eq!(k.level, None);
    assert_eq!(k.factors.as_ref().map(Vec::len), Some(2));
    assert_eq!(k.covariates.as_ref().map(Vec::len), Some(1));
    assert!(k.rescale && !k.city_block && !k.median && !k.weight_features && !k.include);
    assert_eq!(
        k.partition,
        KnnPartition::Ratio {
            training: 70.0,
            holdout: 30.0
        }
    );
    assert_eq!(k.neighbors, KnnNeighbors::Fixed(3));
    assert_eq!(k.feature_selection, None);
    assert_eq!(k.feature_count, KnnFeatureCount::Auto);
    assert_eq!(k.folds, None);
    assert!(k.view && k.print_cps);
    assert_eq!(k.max_categories, 25);
}

#[test]
fn knn_every_subcommand() {
    let k = knn(
        "KNN y (MLEVEL=N) BY a WITH x z /EXCEPT VARIABLES=z /CASELABELS VARIABLE=id
        /FOCALCASES VARIABLE=f /RESCALE COVARIATE=NONE /PARTITION TRAINING=5 HOLDOUT=5
        /MODEL METRIC=CITYBLOCK NEIGHBORS=AUTO(KMAX=7, KMIN=2) FEATURES=AUTO(FORCE=a)
        /CRITERIA NUMFEATURES=ERRORRATIO(MINCHANGE=0.05) PREDICTED=MEDIAN WEIGHTFEATURES=YES
        /CROSSVALIDATION FOLDS=4 /MISSING USERMISSING=INCLUDE /VIEWMODEL DISPLAY=NO /PRINT NONE
        /SAVE PREDVAL(p) PREDPROB PARTITION FOLD(f2) MAXCAT(3)
        /OUTFILE MODEL='m.json' FOCALCASES=near.",
    );
    assert_eq!(k.level, Some(KnnLevel::Nominal));
    assert_eq!(k.except.len(), 1);
    assert_eq!(k.case_labels.as_deref(), Some("id"));
    assert_eq!(k.focal.as_deref(), Some("f"));
    assert!(!k.rescale && k.city_block && k.median && k.weight_features && k.include);
    assert!(!k.view && !k.print_cps);
    assert_eq!(k.neighbors, KnnNeighbors::Auto { min: 2, max: 7 });
    assert_eq!(k.feature_selection.as_ref().map(Vec::len), Some(1));
    assert_eq!(k.feature_count, KnnFeatureCount::ErrorRatio(0.05));
    assert_eq!(k.folds, Some(KnnFolds::Count(4)));
    assert_eq!(
        k.save,
        vec![
            (KnnSave::PredVal, Some("p".into())),
            (KnnSave::PredProb, None),
            (KnnSave::Partition, None),
            (KnnSave::Fold, Some("f2".into())),
        ]
    );
    assert_eq!(k.max_categories, 3);
    assert_eq!(k.model_file.as_deref(), Some("m.json"));
    assert_eq!(k.focal_file.as_deref(), Some("near"));

    let k = knn(
        "KNN WITH x /PARTITION VARIABLE=part /MODEL NEIGHBORS=FIXED(K=1) FEATURES=AUTO
        /CRITERIA NUMFEATURES=FIXED(2) /CROSSVALIDATION VARIABLE=fold.",
    );
    assert_eq!(k.target, None);
    assert_eq!(k.factors, None);
    assert_eq!(k.partition, KnnPartition::Variable("part".into()));
    assert_eq!(k.neighbors, KnnNeighbors::Fixed(1));
    assert_eq!(k.feature_selection, Some(Vec::new()));
    assert_eq!(k.feature_count, KnnFeatureCount::Fixed(2));
    assert_eq!(k.folds, Some(KnnFolds::Variable("fold".into())));
    // an empty BY is kept for the warning
    assert_eq!(knn("KNN y BY WITH x.").factors, Some(Vec::new()));
}

#[test]
fn knn_errors() {
    assert!(err("KNN y.").contains("at least one predictor"));
    assert!(err("KNN y BY.").contains("at least one predictor"));
    assert!(err("KNN y WITH x /PRINT CPS /PRINT NONE.").contains("once"));
    assert!(err("KNN y WITH x /MODEL.").contains("empty"));
    assert!(err("KNN y WITH x /PARTITION TRAINING=6 VARIABLE=v.").contains("not both"));
    assert!(err("KNN y WITH x /PARTITION TRAINING=0.").contains("TRAINING"));
    assert!(err("KNN y WITH x /MODEL NEIGHBORS=AUTO(KMIN=4, KMAX=4).").contains("less than"));
    assert!(!err("KNN y WITH x /MODEL NEIGHBORS=AUTO(KMIN=2).").is_empty());
    assert!(err("KNN y WITH x /MODEL METRIC=EUCLID METRIC=CITYBLOCK.").contains("twice"));
    assert!(err("KNN y WITH x /CROSSVALIDATION FOLDS=1.").contains("FOLDS"));
    assert!(err("KNN y WITH x /CROSSVALIDATION FOLDS=3 VARIABLE=v.").contains("not both"));
    assert!(err("KNN y WITH x /PRINT CPS NONE.").contains("NONE"));
    assert!(err("KNN y WITH x /CRITERIA NUMFEATURES=ERRORRATIO(MINCHANGE=0).").contains("above 0"));
    // keywords are written in full
    assert!(!err("KNN y WITH x /MOD METRIC=EUCLID.").is_empty());
    assert!(!err("KNN y WITH x /MODEL METRIC=EUC.").is_empty());
    assert!(!err("KNN y (MLEVEL=X) WITH x.").is_empty());
}
