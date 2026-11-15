//! Parser tests of the POWER commands.

use super::*;

fn power(source: &str) -> Power {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Power(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    format!("{:?}", errors[0])
}

#[test]
fn one_sample_mean_defaults() {
    let c = power("POWER MEANS ONESAMPLE /PARAMETERS N=25 SD=4 MEAN=1.5.");
    assert_eq!(c.kind, PowerKind::MeansOneSample);
    assert!(!c.directional);
    assert_eq!(c.significance, 0.05);
    assert_eq!(c.n, vec![25.0]);
    assert!(c.power.is_empty());
    assert_eq!(
        (c.sd.clone(), c.mean.clone(), c.null),
        (vec![4.0], vec![1.5], None)
    );
    assert!(c.plots.is_empty());
}

#[test]
fn abbreviated_keywords_and_power_grid() {
    let c = power(
        "POW MEANS ONES /PAR TES=DIR SIG=0.01 POWER=0.95 0.6 TO 0.8 BY 0.1 0.7 SD=2 MEA=1 NUL=0.2.",
    );
    assert!(c.directional);
    assert_eq!(c.significance, 0.01);
    assert_eq!(c.power, vec![0.95, 0.6, 0.7, 0.8]);
    assert_eq!(c.repeated, vec![0.7]);
    assert_eq!(c.null, Some(0.2));
}

#[test]
fn independent_means_two_values_and_plots() {
    let c = power(
        "POWER MEANS INDEPENDENT /PLOT NRATIO ES(0 3) ES_XAXIS NRATIO_YAXIS(0.5 2) VROTATE(40) \
         /PARAMETERS N=12 18 SD=3 5 MEAN=10 7.",
    );
    assert_eq!(c.n, vec![12.0, 18.0]);
    assert_eq!(c.sd, vec![3.0, 5.0]);
    assert_eq!(c.plots.len(), 3);
    assert_eq!(c.plots[1].x_range, Some((0.0, 3.0)));
    let surface = &c.plots[2];
    assert_eq!(surface.x, "ES");
    assert_eq!(surface.y.as_deref(), Some("NRATIO"));
    assert_eq!(surface.y_range, Some((0.5, 2.0)));
}

#[test]
fn proportions_keywords() {
    let c = power(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=(0.3 0.45) POWER=0.8 NRATIO=1.5 \
         METHOD=LRT ESTIMATE=BINOMIAL TIMER=2 CONTINUITY=TRUE POOLED=FALSE.",
    );
    assert_eq!(c.method, ProportionTest::LikelihoodRatio);
    assert!(c.binomial && c.continuity && !c.pooled);
    assert_eq!((c.ratio, c.timer), (1.5, Some(2.0)));
    let c =
        power("POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.3 0.45 N=20 20 METHOD=T.");
    assert_eq!(c.method, ProportionTest::T);
    let c = power(
        "POWER PROPORTIONS RELATED /PARAMETERS NPAIRS=60 VALUES=COUNTS(9 17) \
         /PLOT DISCORDANT RISK_RATIO(1 3).",
    );
    assert!(c.counts && !c.marginal);
    assert_eq!(c.proportions, vec![9.0, 17.0]);
    assert_eq!(c.plots[0].x, "DISCORDANT");
    let c = power(
        "POWER PROPORTIONS ONESAMPLE /PARAMETERS N=30 ALTERNATIVE=0.7 /PLOT NULL NULL_XAXIS ALTERNATIVE_YAXIS.",
    );
    assert_eq!(c.plots[0].x, "NULL");
    assert_eq!(c.plots[1].y.as_deref(), Some("ALTERNATIVE"));
}

#[test]
fn errors_name_the_rule() {
    assert!(err("POWER MEANS ONESAMPLE /PARAMETERS SD=1 MEAN=1.").contains("needs N"));
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS N=5 POWER=.8 SD=1 MEAN=1.").contains("not both")
    );
    assert!(err("POWER MEANS ONESAMPLE /PARAMETERS N=1 SD=1 MEAN=1.").contains("at least 2"));
    assert!(err("POWER MEANS ONESAMPLE /PARAMETERS N=5 SD=0 MEAN=1.").contains("above 0"));
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS POWER=1 SD=1 MEAN=1.").contains("between 0 and 1")
    );
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS POWER=.5 TO .6 BY .1 .7 TO .8 BY .1 SD=1 MEAN=1.")
            .contains("one grid")
    );
    assert!(err("POWER MEANS ONESAMPLE /PARAMETERS N=5 MEAN=1.").contains("needs SD"));
    assert!(
        err("POWER MEANS RELATED /PARAMETERS NPAIRS=5 SD=2 2 RHO=1 MEAN=1.")
            .contains("RHO cannot be 1")
    );
    assert!(
        err("POWER MEANS RELATED /PARAMETERS NPAIRS=5 SD=2 RHO=0 MEAN=1.").contains("not be 0")
    );
    assert!(
        err("POWER PROPORTIONS ONESAMPLE /PARAMETERS POWER=.8 NULL=.4 ALTERNATIVE=.4.")
            .contains("must differ")
    );
    assert!(
        err("POWER PROPORTIONS RELATED /PARAMETERS POWER=.8 VALUES=COUNTS(3 4).")
            .contains("COUNTS needs NPAIRS")
    );
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS N=5 SD=1 MEAN=1 /PLOT N_XAXIS.")
            .contains("both axes")
    );
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS N=5 SD=1 MEAN=1 /PLOT N(9 4).").contains("upwards")
    );
    assert!(
        err("POWER MEANS ONESAMPLE /PARAMETERS N=5 SD=1 MEAN=1 /PLOT VROTATE(400).")
            .contains("0 to 359")
    );
    assert!(
        err("POWER PROPORTIONS INDEPENDENT /PARAMETERS N=1 1 PROPORTIONS=.2 .3 METHOD=T.")
            .contains("more than 2")
    );
}

#[test]
fn oneway_anova_keywords() {
    let c = power(
        "POWER ONEWAY ANOVA /PLOT TOTAL_N(40 90) POOLED_SD_XAXIS TOTAL_N_YAXIS \
         /CONTRAST COEF=-2 1 1 TEST=DIRECTIONAL PAIR=TRUE ADJ=SIDAK \
         /PAR GROUP_MEANS=48 52 57 POOL=9 POWER=0.85 GROUP_W=2 1 1.",
    );
    assert_eq!(c.kind, PowerKind::OnewayAnova);
    assert_eq!(c.mean, vec![48.0, 52.0, 57.0]);
    assert_eq!(
        (c.sd.clone(), c.weights.clone()),
        (vec![9.0], vec![2.0, 1.0, 1.0])
    );
    assert_eq!(c.contrast, vec![-2.0, 1.0, 1.0]);
    assert!(c.directional && c.pairwise && !c.empty_contrast);
    assert_eq!(c.adjust, PairwiseAdjust::Sidak);
    assert_eq!(c.plots[0].x_range, Some((40.0, 90.0)));
    assert_eq!(c.plots[1].x, "POOLED_SD");
    let c = power(
        "POWER ONEWAY ANOVA /PARAMETERS GROUP_SIZES=8 8 9 GROUP_MEANS=1 2 4 POOLED_SD=2 \
         GROUP_WEIGHTS=EQUAL /CONTRAST ADJUST=LSD.",
    );
    assert!(c.empty_contrast && c.weights.is_empty());
}

#[test]
fn correlation_and_regression_keywords() {
    let c = power(
        "POWER PEARSON ONESAMPLE /PARAMETERS ADJUST BIAS=FALSE ALT=-0.3 NUL=0.1 N=40 \
         /PLOT NULL N_YAXIS(10 80) DIFF_XAXIS.",
    );
    assert!(!c.adjust_bias);
    assert_eq!((c.null, c.alternative), (Some(0.1), Some(-0.3)));
    assert_eq!(c.plots[1].x, "DIFF");
    assert_eq!(c.plots[1].y_range, Some((10.0, 80.0)));
    let c = power("POWER SPEARMAN ONESAMPLE /PARAMETERS VARIANCE=CC N=3 ALTERNATIVE=1.");
    assert_eq!(c.variance, RankVariance::Cc);
    let c = power(
        "POWER PARTIALCORR /PARAMETERS PARTIALOUT=2 POWER=0.9 ALTERNATIVE=0.25 /PLOT PARTIALOUT.",
    );
    assert_eq!(c.partialout, 2.0);
    let c = power(
        "POWER UNIVARIATE LINEAR /PARAMETERS MODEL=RANDOM TOTAL_PREDICTORS=7 TEST_PREDICTORS=3 \
         FULL_MODEL=0.31 NESTED_MODEL=0.24 INTERCEPT=FALSE N=8 /PLOT PARTIAL_CORR.",
    );
    assert!(c.random && !c.intercept);
    assert_eq!(c.full_nested, Some((0.31, 0.24)));
    assert_eq!(c.plots[0].x, "PARTIAL_CORR");
}

#[test]
fn errors_of_the_later_commands() {
    let anova = "POWER ONEWAY ANOVA /PARAMETERS GROUP_MEANS=1 2 3 POOLED_SD=1";
    assert!(err(&format!("{anova} GROUP_SIZES=5 5.")).contains("2 values for 3 GROUP_MEANS"));
    assert!(err(&format!("{anova} GROUP_SIZES=5 5 1.")).contains("at least 2"));
    assert!(err(&format!("{anova} POWER=.8 GROUP_WEIGHTS=1 2 200.")).contains("1 to 100"));
    assert!(
        err(&format!("{anova} POWER=.8 /CONTRAST COEFFICIENTS=0 0 0.")).contains("cannot all be 0")
    );
    assert!(err(&format!("{anova} POWER=.8 TEST=DIRECTIONAL.")).contains("unexpected"));
    // GRO fits GROUP_SIZES, GROUP_MEANS and GROUP_WEIGHTS
    assert!(
        err("POWER ONEWAY ANOVA /PARAMETERS GRO=1 2 POOLED_SD=1 POWER=.8.").contains("unexpected")
    );
    assert!(err("POWER PEARSON ONESAMPLE /PARAMETERS N=3 ALTERNATIVE=.3.").contains("at least 4"));
    assert!(
        err("POWER PEARSON ONESAMPLE /PARAMETERS N=9 ALTERNATIVE=1.2.").contains("from -1 to 1")
    );
    assert!(
        err("POWER PEARSON ONESAMPLE /PARAMETERS POWER=.8 ALTERNATIVE=1.").contains("strictly")
    );
    assert!(
        err("POWER SPEARMAN ONESAMPLE /PARAMETERS POWER=.8 ALTERNATIVE=0.").contains("must differ")
    );
    assert!(
        err("POWER PARTIALCORR /PARAMETERS PARTIALOUT=3 N=6 ALTERNATIVE=.3.")
            .contains("at least 7")
    );
    assert!(err("POWER PARTIALCORR /PARAMETERS N=9 ALTERNATIVE=.3.").contains("needs PARTIALOUT"));
    let linear = "POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=4 TEST_PREDICTORS=2";
    assert!(err(&format!("{linear} N=5 PARTIAL_CORR=.3.")).contains("at least 6"));
    assert!(err(&format!("{linear} POWER=.8 PARTIAL_CORR=0.")).contains("cannot be 0"));
    assert!(err(&format!("{linear} N=20.")).contains("needs PARTIAL_CORR"));
    assert!(err(&format!("{linear} N=20 FULL_MODEL=.3.")).contains("go together"));
    assert!(
        err(&format!("{linear} N=20 FULL_MODEL=.3 NESTED_MODEL=.4.")).contains("at least NESTED")
    );
    assert!(
        err(
            "POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=2 TEST_PREDICTORS=2 N=20 \
             FULL_MODEL=.3 NESTED_MODEL=.2."
        )
        .contains("below TOTAL_PREDICTORS")
    );
    assert!(
        err(&format!(
            "{linear} N=20 PARTIAL_CORR=.3 FULL_MODEL=.3 NESTED_MODEL=.2."
        ))
        .contains("not both")
    );
}
