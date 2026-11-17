//! Parser tests of MLP and RBF.

use super::*;

fn neural(source: &str) -> Neural {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Neural(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn mlp_defaults() {
    let n = neural("MLP churn BY plan WITH age.");
    assert!(!n.rbf);
    assert_eq!(n.dependents, vec![("churn".to_string(), None)]);
    assert_eq!(n.partition, Partition::Shares(70.0, 30.0, 0.0));
    assert!(n.architecture.automatic);
    assert_eq!(n.criteria, NeuralCriteria::default());
    assert_eq!(n.stopping, StoppingRules::default());
    assert_eq!(n.print, [true, true, true, true, false, false]);
    assert_eq!(n.plot, [true, false, false, false, false, false]);
    assert_eq!(n.rescale_covariates, Rescale::Standardized);
    assert_eq!(n.rescale_dependents, None);
}

#[test]
fn mlp_every_subcommand() {
    let n = neural(
        "MLP spend (MLEVEL=S) churn (MLEVEL=N) BY plan region WITH age TO income
          /EXCEPT VARIABLES=region
          /RESCALE COVARIATE=ADJNORMALIZED DEPENDENT=NORMALIZED(CORRECTION=0.05)
          /PARTITION TRAINING=5 TESTING=3 HOLDOUT=2
          /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS=2 (NUMUNITS=6, 3) HIDDENFUNCTION=SIGMOID OUTPUTFUNCTION=SIGMOID
          /CRITERIA TRAINING=MINIBATCH MINIBATCHSIZE=20 MEMSIZE=500 OPTIMIZATION=GRADIENTDESCENT
             LEARNINGINITIAL=0.3 LEARNINGLOWER=0.01 MOMENTUM=0.5 LEARNINGEPOCHS=4 LAMBDAINITIAL=0.0000002
             SIGMAINITIAL=0.00002 INTERVALCENTER=0.1 INTERVALOFFSET=0.2
          /STOPPINGRULES ERRORSTEPS=3 (DATA=BOTH) TRAININGTIMER=OFF MAXEPOCHS=40 ERRORCHANGE=0.01 ERRORRATIO=0.02
          /MISSING USERMISSING=INCLUDE
          /PRINT SOLUTION IMPORTANCE
          /PLOT ROC GAIN LIFT PREDICTED RESIDUAL
          /SAVE PREDVAL(p1 p2) PSEUDOPROB(pp:3)
          /OUTFILE MODEL='a.json' 'b.json'.",
    );
    assert_eq!(n.dependents[0], ("spend".to_string(), Some(Level::Scale)));
    assert_eq!(n.dependents[1], ("churn".to_string(), Some(Level::Nominal)));
    assert_eq!(n.except.len(), 1);
    assert_eq!(n.rescale_covariates, Rescale::AdjNormalized(0.0));
    assert_eq!(n.rescale_dependents, Some(Rescale::Normalized(0.05)));
    assert_eq!(n.partition, Partition::Shares(5.0, 3.0, 2.0));
    let a = &n.architecture;
    assert!(!a.automatic && a.hidden_layers == 2 && a.units == vec![6, 3]);
    assert_eq!(
        (a.hidden, a.output),
        (Activation::Sigmoid, Some(Activation::Sigmoid))
    );
    let c = n.criteria;
    assert_eq!(c.training, Training::MiniBatch);
    assert_eq!(
        (c.minibatch, c.memsize, c.scaled_conjugate),
        (Some(20), 500, Some(false))
    );
    assert_eq!(
        (c.learning_initial, c.learning_lower, c.momentum),
        (0.3, 0.01, 0.5)
    );
    assert_eq!(
        (c.learning_epochs, c.interval_center, c.interval_offset),
        (4, 0.1, 0.2)
    );
    let s = n.stopping;
    assert_eq!(
        (s.error_steps, s.both, s.max_time, s.max_epochs),
        (3, true, None, Some(40))
    );
    assert!(n.include);
    assert_eq!(n.print, [false, false, false, false, true, true]);
    assert_eq!(n.plot, [false, true, true, true, true, true]);
    assert_eq!(n.predval, Some(vec!["p1".into(), "p2".into()]));
    assert_eq!(n.pseudoprob, Some(vec![("pp".into(), 3)]));
    assert_eq!(n.outfile.len(), 2);
}

#[test]
fn rbf_architecture_and_overlap() {
    let n = neural(
        "RBF y BY f /ARCHITECTURE MINUNITS=2 MAXUNITS=AUTO HIDDENFUNCTION=ORBF /CRITERIA OVERLAP=1.5.",
    );
    assert!(n.rbf && n.architecture.automatic && n.architecture.orbf);
    assert_eq!(
        (n.architecture.min_units, n.architecture.max_units),
        (Some(2), None)
    );
    assert_eq!(n.criteria.overlap, Some(1.5));
    let n = neural("RBF y WITH x /ARCHITECTURE NUMUNITS=4 /PARTITION VARIABLE=part.");
    assert!(!n.architecture.automatic && n.architecture.units == vec![4]);
    assert_eq!(n.partition, Partition::Variable("part".into()));
}

#[test]
fn empty_by_warns() {
    let n = neural("MLP y BY WITH x.");
    assert_eq!(n.warnings.len(), 1);
    assert_eq!(n.covariates.len(), 1);
}

#[test]
fn neural_errors() {
    assert!(
        err("MLP y WITH x /ARCHITECTURE AUTOMATIC=YES HIDDENLAYERS=2.").contains("AUTOMATIC=NO")
    );
    assert!(
        err("MLP y WITH x /ARCHITECTURE AUTOMATIC=YES (MINUNITS=5, MAXUNITS=3).")
            .contains("less than")
    );
    assert!(
        err("MLP y WITH x /CRITERIA TRAINING=ONLINE OPTIMIZATION=SCALEDCONJUGATE.")
            .contains("BATCH")
    );
    assert!(err("MLP y WITH x /CRITERIA LAMBDAINITIAL=0.01.").contains("LAMBDAINITIAL"));
    assert!(
        err("MLP y WITH x /CRITERIA LEARNINGINITIAL=0.1 LEARNINGLOWER=0.2.")
            .contains("LEARNINGLOWER")
    );
    assert!(err("MLP y WITH x /PRINT NONE CPS.").contains("NONE"));
    assert!(err("MLP y WITH x /PRINT CPS /PRINT SUMMARY.").contains("twice"));
    assert!(err("MLP y WITH x /CRITERIA MOMENTUM=0.5 MOMENTUM=0.6.").contains("twice"));
    assert!(err("MLP y WITH x /PRINT.").contains("empty"));
    assert!(err("MLP y z WITH x /OUTFILE MODEL='a.json'.").contains("one file per dependent"));
    assert!(err("RBF y WITH x /ARCHITECTURE NUMUNITS=3 MINUNITS=2.").contains("NUMUNITS"));
    assert!(err("RBF y WITH x /ARCHITECTURE MINUNITS=2.").contains("together"));
    assert!(err("RBF y WITH x /STOPPINGRULES ERRORSTEPS=2.").contains("STOPPINGRULES"));
    assert!(err("MLP y WITH x /PARTITION TRAINING=0 TESTING=5.").contains("TRAINING"));
}
