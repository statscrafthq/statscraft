//! GENLINMIXED on invented data, drawn once in R with a fixed seed: 12 clinics of 8 patients
//! with a dose, an arm and a cured flag from a logistic model with a clinic effect, and 10
//! people seen at 6 visits with a count of events (Poisson, a random intercept and slope in
//! time) and a score. The expected numbers come from a separate R program written for this
//! check from the algorithms' description (restricted pseudo likelihood with the predicted
//! random effects in the expansion, scale 1, iterated until the covariance parameters change
//! by less than 1e-13 relative): the estimates, their standard errors from (X'V^-1 X)^-1,
//! the covariance parameters with standard errors from 2 H^-1 (H by numerical derivatives; for
//! the linear UN structure in closed form, as finite differences were good to 1e-5 only) and
//! the -2 restricted log pseudo likelihood. That program finds each inner maximum with a
//! derivative-free optimizer polished by numerical Newton steps, independently of the analytic
//! derivatives used here. MASS::glmmPQL was not used: it maximizes the full likelihood of the
//! working model and estimates the scale.
//!
//! The later tests (repeated measures, the negative binomial, robust covariance, EMMEANS and
//! SAVE) come from a second such R program on the same data: R side V = W^-1/2 R W^-1/2 with R
//! block diagonal over persons, the negative binomial k from MASS::glm.nb on the model without
//! random effects, the sandwich over clinics, the means and contrasts by the delta method with
//! the step-down adjustments written out, and the saved predictions with the prediction error
//! variance from the mixed model equations.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const BINARY: &str = "DATA LIST FREE /clinic arm dose cured.
BEGIN DATA
1 1 3 0 1 2 2.6 1 1 1 3.2 1 1 2 2.2 1 1 1 1.6 1 1 2 1.3 1 1 1 3.3 1 1 2 1.7 1
2 1 2.3 0 2 2 4.6 1 2 1 4.7 0 2 2 3 1 2 1 4.7 0 2 2 4.3 1 2 1 1.1 1 2 2 3.6 1
3 1 1.8 0 3 2 3.4 0 3 1 4.2 1 3 2 2.9 1 3 1 3.1 0 3 2 2.4 1 3 1 2 0 3 2 1.7 0
4 1 2.4 0 4 2 2.4 0 4 1 2.3 0 4 2 4.2 0 4 1 4.6 0 4 2 1 0 4 1 3.6 0 4 2 2.6 0
5 1 1.9 1 5 2 4.2 1 5 1 2.8 0 5 2 1.7 0 5 1 2 1 5 2 3.7 1 5 1 2.4 0 5 2 1 1
6 1 1.3 0 6 2 2.9 1 6 1 2 1 6 2 2.1 1 6 1 3.7 0 6 2 2.6 0 6 1 2.8 0 6 2 1.6 1
7 1 3.5 1 7 2 2.4 1 7 1 4.4 1 7 2 4.5 1 7 1 3.1 1 7 2 1.1 0 7 1 2.4 0 7 2 4.3 1
8 1 3.7 0 8 2 2.6 0 8 1 2.3 1 8 2 1.4 0 8 1 3.3 0 8 2 3.1 0 8 1 3.6 1 8 2 2.5 1
9 1 2.5 1 9 2 1.6 1 9 1 4.9 1 9 2 4.6 1 9 1 3.7 1 9 2 4.1 1 9 1 1.6 1 9 2 1.5 0
10 1 4.7 1 10 2 1.6 1 10 1 1.6 1 10 2 2.7 1 10 1 3.7 0 10 2 3.7 1 10 1 4.5 1 10 2 4.3 1
11 1 2.8 1 11 2 2.8 1 11 1 5 1 11 2 4.2 1 11 1 1.2 1 11 2 4.2 0 11 1 3.5 0 11 2 4.6 1
12 1 1.8 0 12 2 3.4 1 12 1 2.5 0 12 2 1.4 1 12 1 4.7 1 12 2 5 1 12 1 3.7 1 12 2 3.3 1
END DATA.
";

const COUNTS: &str = "DATA LIST FREE /person treat time visits score.
BEGIN DATA
1 1 0 0 8.76 1 1 0.2 3 7.78 1 1 0.4 0 10.83 1 1 0.6 3 10.79 1 1 0.8 5 10.64 1 1 1 4 9.04
2 1 0 1 11.29 2 1 0.2 3 12.63 2 1 0.4 3 10.89 2 1 0.6 0 10.66 2 1 0.8 5 11.44 2 1 1 2 11.81
3 1 0 13 11.18 3 1 0.2 7 13.19 3 1 0.4 5 13.77 3 1 0.6 7 15.06 3 1 0.8 4 14.32 3 1 1 4 14.05
4 1 0 2 10.34 4 1 0.2 4 11 4 1 0.4 2 12.75 4 1 0.6 3 11.09 4 1 0.8 8 12.61 4 1 1 1 12.75
5 1 0 2 10.07 5 1 0.2 2 10.31 5 1 0.4 2 10.32 5 1 0.6 0 12.27 5 1 0.8 4 10.95 5 1 1 2 10.04
6 2 0 0 9.31 6 2 0.2 1 10.08 6 2 0.4 0 11.27 6 2 0.6 3 11.82 6 2 0.8 4 9.68 6 2 1 1 12.28
7 2 0 1 9.91 7 2 0.2 6 9.32 7 2 0.4 2 11.39 7 2 0.6 0 12.17 7 2 0.8 2 12.45 7 2 1 3 10.45
8 2 0 0 9.84 8 2 0.2 0 10.31 8 2 0.4 1 10.33 8 2 0.6 1 8.41 8 2 0.8 1 10.12 8 2 1 5 11.14
9 2 0 0 10.67 9 2 0.2 2 8.43 9 2 0.4 1 8.75 9 2 0.6 0 9.12 9 2 0.8 2 10.92 9 2 1 0 10.96
10 2 0 5 12.62 10 2 0.2 5 13.84 10 2 0.4 7 15.59 10 2 0.6 9 15.32 10 2 0.8 12 14.56 10 2 1 17 16.1
END DATA.
";

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn run_err(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("no error");
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The numbers of the row whose first cell starts with `first`, by column title.
fn value(tables: &[Table], title: &str, first: &str, column: &str) -> f64 {
    let t = table(tables, title);
    let c = t
        .column_titles()
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("no column {column} in {title}"));
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s.starts_with(first)))
        .unwrap_or_else(|| panic!("no row {first} in {title}"));
    match row[c] {
        Cell::Number(n) => n,
        ref other => panic!("{title} {first} {column}: {other:?}"),
    }
}

/// The numbers of the `index`-th row of a table, by column title (the last part of a grouped
/// one).
fn nth(tables: &[Table], title: &str, index: usize, column: &str) -> f64 {
    let t = table(tables, title);
    let c = t
        .column_titles()
        .iter()
        .position(|h| h == column || h.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {title}"));
    match t.rows[index][c] {
        Cell::Number(n) => n,
        ref other => panic!("{title} row {index} {column}: {other:?}"),
    }
}

fn close(got: f64, want: f64, what: &str) {
    let tolerance = 1e-6 * want.abs().max(1.0);
    assert!((got - want).abs() < tolerance, "{what}: {got} vs {want}");
}

#[test]
fn binary_logit_with_a_random_intercept() {
    // cured has the values 0 and 1: the last (1) is the reference, so 0 is the event
    let tables = run(&format!(
        "{BINARY}GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured\n\
         /FIXED EFFECTS=dose arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic\n\
         /BUILD_OPTIONS PCONVERGE=1E-10."
    ));
    let summary = table(&tables, "Model Summary");
    assert_eq!(summary.rows[1][1], Cell::Text("Binomial".into()));
    assert_eq!(summary.rows[2][1], Cell::Text("Logit".into()));
    let fixed = "Estimates of Fixed Effects";
    let want = [
        (
            "Intercept",
            -0.134767981299,
            0.742376279773,
            0.8563419948855,
        ),
        ("dose", -0.297405937204, 0.218027116166, 0.1758352991013),
        ("[arm=1", 0.959357952205, 0.465279805046, 0.0420077429735),
    ];
    for (name, b, se, p) in want {
        close(value(&tables, fixed, name, "Estimate"), b, name);
        close(value(&tables, fixed, name, "Std. Error"), se, name);
        close(value(&tables, fixed, name, "Sig."), p, name);
        close(value(&tables, fixed, name, "df"), 93.0, name);
    }
    let cov = "Estimates of Covariance Parameters";
    close(
        nth(&tables, cov, 0, "Estimate"),
        0.925627384018,
        "intercept variance",
    );
    close(
        nth(&tables, cov, 0, "Std. Error"),
        0.696155736754,
        "its standard error",
    );
    assert_eq!(table(&tables, cov).rows.len(), 1, "no residual parameter");
    close(
        value(
            &tables,
            "Information Criteria",
            "-2 Restricted Log Pseudo",
            "Value",
        ),
        429.64223572583,
        "-2 REPL",
    );
}

#[test]
fn poisson_log_with_a_random_intercept_and_slope() {
    let tables = run(&format!(
        "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person /FIELDS TARGET=visits\n\
         /TARGET_OPTIONS DISTRIBUTION=POISSON /FIXED EFFECTS=treat time\n\
         /RANDOM EFFECTS=time USE_INTERCEPT=TRUE SUBJECTS=person COVARIANCE_TYPE=UNSTRUCTURED\n\
         /BUILD_OPTIONS PCONVERGE=1E-10."
    ));
    let fixed = "Estimates of Fixed Effects";
    let want = [
        ("Intercept", 0.434563808860, 0.425800451061, 0.3117652846134),
        ("[treat=1", 0.232647396572, 0.463077072483, 0.6173254797116),
        ("time", 0.699985812598, 0.392280947601, 0.0796812361962),
    ];
    for (name, b, se, p) in want {
        close(value(&tables, fixed, name, "Estimate"), b, name);
        close(value(&tables, fixed, name, "Std. Error"), se, name);
        close(value(&tables, fixed, name, "Sig."), p, name);
        close(value(&tables, fixed, name, "df"), 57.0, name);
    }
    let cov = "Estimates of Covariance Parameters";
    let want = [
        (0.952216717378, 0.608235274330),
        (-0.679107285999, 0.581909819147),
        (0.925652192829, 0.683522526137),
    ];
    for (i, (est, se)) in want.into_iter().enumerate() {
        close(nth(&tables, cov, i, "Estimate"), est, "UN parameter");
        close(nth(&tables, cov, i, "Std. Error"), se, "its standard error");
    }
    close(
        value(
            &tables,
            "Information Criteria",
            "-2 Restricted Log Pseudo",
            "Value",
        ),
        149.97668549921,
        "-2 REPL",
    );
}

#[test]
fn normal_identity_is_mixed_itself() {
    let glmm = run(&format!(
        "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person /FIELDS TARGET=score\n\
         /FIXED EFFECTS=treat time /RANDOM EFFECTS=time USE_INTERCEPT=TRUE SUBJECTS=person\n\
         COVARIANCE_TYPE=UNSTRUCTURED /BUILD_OPTIONS DF_METHOD=SATTERTHWAITE."
    ));
    let mixed = run(&format!(
        "{COUNTS}MIXED score BY treat WITH time /FIXED=treat time\n\
         /RANDOM=INTERCEPT time | SUBJECT(person) COVTYPE(UN) /CRITERIA=SCORING(0)\n\
         /PRINT=SOLUTION TESTCOV."
    ));
    for title in [
        "Estimates of Fixed Effects",
        "Estimates of Covariance Parameters",
        "Type III Tests of Fixed Effects",
    ] {
        let (a, b) = (table(&glmm, title), table(&mixed, title));
        assert_eq!(a.rows.len(), b.rows.len(), "{title}");
        for (ra, rb) in a.rows.iter().zip(&b.rows) {
            for (ca, cb) in ra.iter().zip(rb) {
                match (ca, cb) {
                    (Cell::Number(x), Cell::Number(y)) => {
                        assert!(
                            (x - y).abs() <= 1e-9 * y.abs().max(1.0),
                            "{title}: {x} vs {y}"
                        )
                    }
                    _ => assert_eq!(ca, cb, "{title}"),
                }
            }
        }
    }
    let m2 = |t: &[Table], label: &str| value(t, "Information Criteria", label, "Value");
    close(
        m2(&glmm, "-2 Restricted Log Pseudo"),
        m2(&mixed, "-2 Restricted Log Likelihood"),
        "-2 REPL",
    );
}

#[test]
fn errors_for_what_is_not_supported_or_wrong() {
    let cases = [
        (
            format!("{BINARY}GENLINMIXED /FIELDS TARGET=cured /FIXED EFFECTS=dose."),
            "without RANDOM or REPEATED_MEASURES",
        ),
        (
            format!(
                "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person REPEATED_MEASURES=person /FIELDS TARGET=score."
            ),
            "cannot also be a subject",
        ),
        (
            format!(
                "{BINARY}GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured /FIXED EFFECTS=dose\n\
                 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic /EMMEANS TABLES=arm."
            ),
            "on FIXED",
        ),
        (
            format!(
                "{BINARY}GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured /RANDOM USE_INTERCEPT=TRUE SUBJECTS=arm."
            ),
            "SUBJECTS field of DATA_STRUCTURE",
        ),
        (
            format!(
                "{BINARY}GENLINMIXED /FIELDS TARGET=cured /FIXED EFFECTS=cured /RANDOM USE_INTERCEPT=TRUE."
            ),
            "on FIELDS",
        ),
        (
            format!(
                "{BINARY}GENLINMIXED /FIELDS TARGET=dose /TARGET_OPTIONS DISTRIBUTION=POISSON LINK=LOGIT /RANDOM USE_INTERCEPT=TRUE."
            ),
            "cannot be used",
        ),
        (
            format!(
                "{ORDINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=school REPEATED_MEASURES=x /FIELDS TARGET=grade\n\
                 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=school."
            ),
            "REPEATED_MEASURES cannot be used",
        ),
        (
            format!(
                "{ORDINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=school /FIELDS TARGET=grade\n\
                 /TARGET_OPTIONS LINK=LOG /RANDOM USE_INTERCEPT=TRUE SUBJECTS=school."
            ),
            "cannot be used with an ordinal",
        ),
        (
            format!(
                "{NOMINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=store /FIELDS TARGET=choice\n\
                 /TARGET_OPTIONS LINK=PROBIT /RANDOM USE_INTERCEPT=TRUE SUBJECTS=store."
            ),
            "logit link only",
        ),
        (
            format!(
                "{NOMINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=store /FIELDS TARGET=choice\n\
                 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=store /SAVE PEARSON_RESIDUALS."
            ),
            "not supported yet",
        ),
    ];
    for (source, part) in cases {
        let message = run_err(&source);
        assert!(message.contains(part), "{message}");
    }
}

/// The value of a saved numeric field at a case.
fn saved(engine: &Engine, name: &str, case: usize) -> f64 {
    match engine.dataset().unwrap().variable(name).unwrap().values() {
        statscraft_data::Values::Numeric(v) => v.get(case).expect("a value"),
        _ => panic!("{name} is a string"),
    }
}

fn run_engine(source: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

fn estimates(tables: &[Table], want: &[(&str, f64, f64)]) {
    let fixed = "Estimates of Fixed Effects";
    for &(name, b, se) in want {
        close(value(tables, fixed, name, "Estimate"), b, name);
        close(value(tables, fixed, name, "Std. Error"), se, name);
    }
}

#[test]
fn poisson_with_ar1_repeated_measures_and_no_random_effects() {
    let tables = run(&format!(
        "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person REPEATED_MEASURES=time COVARIANCE_TYPE=AR1\n\
         /FIELDS TARGET=visits /TARGET_OPTIONS DISTRIBUTION=POISSON /FIXED EFFECTS=treat time\n\
         /BUILD_OPTIONS PCONVERGE=1E-10."
    ));
    estimates(
        &tables,
        &[
            ("Intercept", 0.90720647677829, 0.5076789297060),
            ("[treat=1", -0.02792589474591, 0.5740688756182),
            ("time", 0.48228156469611, 0.4922861607244),
        ],
    );
    let cov = "Estimates of Covariance Parameters";
    close(
        nth(&tables, cov, 0, "Estimate"),
        4.6968162680458,
        "AR1 variance",
    );
    close(nth(&tables, cov, 1, "Estimate"), 0.7307402480839, "AR1 rho");
    close(
        nth(&tables, cov, 0, "Std. Error"),
        1.6174008760196,
        "its error",
    );
    close(
        nth(&tables, cov, 1, "Std. Error"),
        0.1037775854393,
        "its error",
    );
    close(
        value(
            &tables,
            "Information Criteria",
            "-2 Restricted Log Pseudo",
            "Value",
        ),
        153.35645362254,
        "-2 REPL",
    );
}

#[test]
fn normal_identity_with_repeated_measures_is_mixed_itself() {
    let glmm = run(&format!(
        "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person REPEATED_MEASURES=time COVARIANCE_TYPE=ARH1\n\
         /FIELDS TARGET=score /FIXED EFFECTS=treat time /RANDOM USE_INTERCEPT=TRUE SUBJECTS=person\n\
         /BUILD_OPTIONS DF_METHOD=SATTERTHWAITE."
    ));
    let mixed = run(&format!(
        "{COUNTS}MIXED score BY treat WITH time /FIXED=treat time\n\
         /RANDOM=INTERCEPT | SUBJECT(person) /REPEATED=time | SUBJECT(person) COVTYPE(ARH1)\n\
         /CRITERIA=SCORING(0) /PRINT=SOLUTION TESTCOV."
    ));
    for title in [
        "Estimates of Fixed Effects",
        "Estimates of Covariance Parameters",
        "Type III Tests of Fixed Effects",
    ] {
        let (a, b) = (table(&glmm, title), table(&mixed, title));
        assert_eq!(a.rows.len(), b.rows.len(), "{title}");
        for (ra, rb) in a.rows.iter().zip(&b.rows) {
            for (ca, cb) in ra.iter().zip(rb) {
                match (ca, cb) {
                    (Cell::Number(x), Cell::Number(y)) => {
                        assert!(
                            (x - y).abs() <= 1e-9 * y.abs().max(1.0),
                            "{title}: {x} vs {y}"
                        )
                    }
                    _ => assert_eq!(ca, cb, "{title}"),
                }
            }
        }
    }
}

#[test]
fn robust_covariance_and_marginal_means_of_a_binary_target() {
    let tables = run(&format!(
        "{BINARY}GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured\n\
         /FIXED EFFECTS=dose arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic\n\
         /BUILD_OPTIONS PCONVERGE=1E-10 COVB=ROBUST\n\
         /EMMEANS TABLES=arm COMPARE=arm CONTRAST=SIMPLE."
    ));
    estimates(
        &tables,
        &[
            ("Intercept", -0.1347679812211, 0.7593674828174),
            ("dose", -0.2974059371989, 0.2150848030421),
            ("[arm=1", 0.9593579520799, 0.3948777799728),
        ],
    );
    let fixed = "Estimates of Fixed Effects";
    close(
        value(&tables, fixed, "[arm=1", "Sig."),
        0.01704018543413,
        "robust p",
    );
    let means = "Estimated Marginal Means: arm";
    let want = [
        (
            0.4861171381130,
            0.08948881555905,
            0.3171452374886,
            0.6583232393553,
        ),
        (
            0.2660212905765,
            0.08640083447823,
            0.1308310534883,
            0.4660083487230,
        ),
    ];
    for (i, (m, se, lo, hi)) in want.into_iter().enumerate() {
        close(nth(&tables, means, i, "Mean"), m, "mean");
        close(nth(&tables, means, i, "Std. Error"), se, "its error");
        close(nth(&tables, means, i, "Lower Bound"), lo, "lower");
        close(nth(&tables, means, i, "Upper Bound"), hi, "upper");
    }
    let c = "Contrast Estimates: arm";
    close(
        nth(&tables, c, 0, "Contrast Estimate"),
        0.22009584753652,
        "contrast",
    );
    close(
        nth(&tables, c, 0, "Std. Error"),
        0.08525651478379,
        "its error",
    );
    close(nth(&tables, c, 0, "Adj. Sig."), 0.01139766745432, "its p");
    let f = nth(&tables, "Overall Test: arm", 0, "F");
    close(f, 2.58157218946470_f64.powi(2), "F of one contrast");
}

#[test]
fn negative_binomial_with_k_from_the_model_without_random_effects() {
    let tables = run(&format!(
        "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person /FIELDS TARGET=visits\n\
         /TARGET_OPTIONS DISTRIBUTION=NEGATIVE_BINOMIAL /FIXED EFFECTS=treat time\n\
         /RANDOM USE_INTERCEPT=TRUE SUBJECTS=person /BUILD_OPTIONS PCONVERGE=1E-10."
    ));
    let summary = table(&tables, "Model Summary");
    let Cell::Text(name) = &summary.rows[1][1] else {
        panic!("{:?}", summary.rows[1][1])
    };
    let k: f64 = name
        .trim_start_matches("Negative binomial (")
        .trim_end_matches(')')
        .parse()
        .unwrap();
    close(k, 0.6984394401926, "k");
    estimates(
        &tables,
        &[
            ("Intercept", 0.4341251565846, 0.4086046918268),
            ("[treat=1", 0.3744865917015, 0.4882312145654),
            ("time", 0.6370681541530, 0.3990057014688),
        ],
    );
    let cov = "Estimates of Covariance Parameters";
    close(nth(&tables, cov, 0, "Estimate"), 0.407413334663, "variance");
    close(
        nth(&tables, cov, 0, "Std. Error"),
        0.2877848609233,
        "its error",
    );
}

/// A Poisson model with a three-level group (person modulo 3) and a person intercept.
fn grouped(rest: &str) -> String {
    format!(
        "{COUNTS}COMPUTE grp = MOD(person, 3).\n\
         GENLINMIXED /DATA_STRUCTURE SUBJECTS=person /FIELDS TARGET=visits\n\
         /TARGET_OPTIONS DISTRIBUTION=POISSON /FIXED EFFECTS=grp time\n\
         /RANDOM USE_INTERCEPT=TRUE SUBJECTS=person /BUILD_OPTIONS PCONVERGE=1E-10\n{rest}."
    )
}

#[test]
fn pairwise_means_with_sequential_bonferroni() {
    let tables = run(&grouped(
        "/EMMEANS TABLES=grp COMPARE=grp /EMMEANS_OPTIONS PADJUST=SEQBONFERRONI",
    ));
    let means = "Estimated Marginal Means: grp";
    let want = [
        (
            2.189849984133,
            0.9476564419167,
            0.9202882541229,
            5.210805344438,
        ),
        (
            3.670327558322,
            1.3254077059035,
            1.7804783683404,
            7.566115165966,
        ),
        (
            1.823788311760,
            0.7903571690213,
            0.7655131677078,
            4.345064130083,
        ),
    ];
    for (i, (m, se, lo, hi)) in want.into_iter().enumerate() {
        close(nth(&tables, means, i, "Mean"), m, "mean");
        close(nth(&tables, means, i, "Std. Error"), se, "its error");
        close(nth(&tables, means, i, "Lower Bound"), lo, "lower");
        close(nth(&tables, means, i, "Upper Bound"), hi, "upper");
    }
    let c = "Contrast Estimates: grp";
    let want = [
        (
            -1.4804775741884,
            1.628463812587,
            0.7343604682649,
            -5.231271080181,
        ),
        (
            0.3660616723727,
            1.233407514891,
            0.7677257077371,
            -2.104750484389,
        ),
        (
            1.8465392465612,
            1.542395816398,
            0.7088287934725,
            -1.960129209643,
        ),
    ];
    for (i, (est, se, p, lo)) in want.into_iter().enumerate() {
        close(nth(&tables, c, i, "Contrast Estimate"), est, "contrast");
        close(nth(&tables, c, i, "Std. Error"), se, "its error");
        close(nth(&tables, c, i, "Adj. Sig."), p, "adjusted p");
        close(
            nth(&tables, c, i, "Lower Bound"),
            lo,
            "adjusted lower bound",
        );
    }
    let o = "Overall Test: grp";
    close(nth(&tables, o, 0, "F"), 0.7217965170991, "F");
    close(nth(&tables, o, 0, "Sig."), 0.4903430766602, "its p");
    close(nth(&tables, o, 0, "df1"), 2.0, "df1");
}

#[test]
fn deviation_contrasts_of_the_linear_predictor_with_sequential_sidak() {
    let tables = run(&grouped(
        "/EMMEANS TABLES=grp COMPARE=grp CONTRAST=DEVIATION\n\
         /EMMEANS_OPTIONS SCALE=TRANSFORMED PADJUST=SEQSIDAK",
    ));
    let c = "Contrast Estimates: grp";
    let want = [
        (
            -0.1111768855426,
            0.3441937477926,
            0.7478931968115,
            0.5783260452085,
        ),
        (
            0.4052709844034,
            0.3154570341900,
            0.3666741984455,
            1.1301841725324,
        ),
    ];
    for (i, (est, se, p, hi)) in want.into_iter().enumerate() {
        close(nth(&tables, c, i, "Contrast Estimate"), est, "contrast");
        close(nth(&tables, c, i, "Std. Error"), se, "its error");
        close(nth(&tables, c, i, "Adj. Sig."), p, "adjusted p");
        close(
            nth(&tables, c, i, "Upper Bound"),
            hi,
            "adjusted upper bound",
        );
    }
}

#[test]
fn save_and_the_model_file() {
    let file = std::env::temp_dir().join(format!("statscraft-glmm-{}.json", std::process::id()));
    let (engine, _) = run_engine(&grouped(&format!(
        "/SAVE PREDICTED_VALUES CONFIDENCE_INTERVALS(ci) PEARSON_RESIDUALS /OUTFILE MODEL='{}'",
        file.display()
    )));
    let want = [
        (
            0,
            1.970277010971,
            1.150007804714,
            3.375621873215,
            -1.403665562365,
        ),
        (
            1,
            2.1947853004005,
            1.3232620972741,
            3.6403086922667,
            0.5435201797238,
        ),
        (
            30,
            1.2115385316912,
            0.6326513763385,
            2.3201176329806,
            -1.1006991104254,
        ),
        (
            59,
            11.410677286623,
            8.209819349413,
            15.859491006799,
            1.654639828488,
        ),
    ];
    for (case, mu, lo, hi, r) in want {
        close(saved(&engine, "PredictedValue", case), mu, "predicted");
        close(saved(&engine, "ci_Lower", case), lo, "lower");
        close(saved(&engine, "ci_Upper", case), hi, "upper");
        close(
            saved(&engine, "PearsonResidual", case),
            r,
            "Pearson residual",
        );
    }
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::remove_file(&file).ok();
    assert!(
        text.contains("\"format\": \"statscraft-glmm-model\""),
        "{text}"
    );
    assert!(text.contains("\"link\": \"Log\""), "{text}");
    assert!(text.contains("0.5395504057"), "{text}");
}

#[test]
fn save_the_category_and_probabilities_of_a_binary_target() {
    let (engine, _) = run_engine(&format!(
        "{BINARY}GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured\n\
         /FIXED EFFECTS=dose arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic\n\
         /SAVE PREDICTED_VALUES(cat) PREDICTED_PROBABILITY(p) CONFIDENCE(conf)."
    ));
    for case in 0..96 {
        let (p0, p1) = (saved(&engine, "p_1", case), saved(&engine, "p_2", case));
        assert!((p0 + p1 - 1.0).abs() < 1e-12);
        let category = saved(&engine, "cat", case);
        assert_eq!(category, if p0 >= 0.5 { 0.0 } else { 1.0 });
        assert!((saved(&engine, "conf", case) - p0.max(p1)).abs() < 1e-12);
    }
}

// Multinomial targets, invented and drawn once in R with a fixed seed: 12 schools of 8 pupils
// with a score x, an arm and a grade 1 to 4 from a cumulative logit with a school effect, and
// 12 stores of 10 shoppers with a price x2 and a choice a, b or c from a generalized logit with
// a store effect for each category. The expected numbers come from a third R program written
// from the algorithms' description of the multinomial models (the expanded design with
// thresholds or a parameter set per category, a residual block (D' A^-1 D)^-1 per case, the
// start (y + 1/J) / 2, restricted pseudo likelihood with N the cases), its inner fits a
// derivative-free search over the variance polished by numerical Newton steps, the standard
// error of the variance from a numerical Hessian, and the prediction error covariance from
// the inverse of the mixed model equations.

const ORDINAL: &str = "DATA LIST FREE /school arm x grade.
BEGIN DATA
1 1 2.3 3 1 2 3.1 3 1 1 4.3 4 1 2 1.7 2 1 1 1.6 1 1 2 1.2 3 1 1 2.8 3 1 2 1.2 2
2 1 4.2 4 2 2 2.8 4 2 1 4.4 4 2 2 4.7 1 2 1 3.3 3 2 2 4.6 4 2 1 3.9 4 2 2 1.9 4
3 1 3 3 3 2 4.6 4 3 1 4.8 3 3 2 1 3 3 1 3.4 4 3 2 1.5 4 3 1 3.7 3 3 2 3.1 3
4 1 3.9 4 4 2 1.2 4 4 1 3.8 4 4 2 4.5 4 4 1 1.9 1 4 2 3.2 4 4 1 3.8 4 4 2 3.7 4
5 1 3.1 2 5 2 1 3 5 1 1.5 3 5 2 2.6 4 5 1 3.7 4 5 2 2 1 5 1 0.2 2 5 2 4.5 4
6 1 5 3 6 2 0.7 3 6 1 1.2 4 6 2 1.2 1 6 1 1.5 2 6 2 3.9 3 6 1 4.5 4 6 2 0.8 2
7 1 1.9 2 7 2 1.3 3 7 1 3.1 4 7 2 1 1 7 1 1.6 2 7 2 3.1 4 7 1 3 4 7 2 1.9 3
8 1 3.3 4 8 2 4 3 8 1 1.5 3 8 2 0.6 3 8 1 2.6 2 8 2 0.3 2 8 1 3.2 2 8 2 1.2 2
9 1 3.1 3 9 2 1.4 1 9 1 2.4 4 9 2 4.3 4 9 1 2.4 3 9 2 3.8 4 9 1 3.2 3 9 2 4 4
10 1 1.9 2 10 2 2.3 3 10 1 2.3 2 10 2 5 2 10 1 3.6 2 10 2 3.9 3 10 1 2.1 2 10 2 1.5 2
11 1 3.9 4 11 2 1.7 2 11 1 1.9 3 11 2 3.4 2 11 1 3.9 3 11 2 4.6 2 11 1 0.7 1 11 2 0.9 1
12 1 2.1 1 12 2 1.3 2 12 1 4.9 4 12 2 1.8 3 12 1 4.3 1 12 2 3.1 4 12 1 0.5 1 12 2 2.8 2
END DATA.
VARIABLE LEVEL grade arm (ORDINAL).
";

const NOMINAL: &str = "DATA LIST FREE /store x2 (F8.1) choice (A1).
BEGIN DATA
1 2.8 c 1 1.4 b 1 2.1 b 1 3.4 c 1 0 a 1 1.5 a 1 0.8 b 1 2.6 b 1 2.8 c 1 0.2 b
2 3.3 c 2 3.1 c 2 2.2 c 2 0.5 c 2 1.7 c 2 3.8 c 2 0.5 a 2 0.4 b 2 3.6 a 2 1.6 a
3 1.8 a 3 1.5 a 3 2.4 c 3 3.6 b 3 3.5 c 3 2.1 a 3 2.1 a 3 3.6 b 3 1.2 b 3 0.2 b
4 0.7 c 4 3.4 c 4 1.4 c 4 1.8 a 4 3.5 c 4 0.4 c 4 1.8 c 4 1.6 a 4 2 c 4 2.8 c
5 0.5 c 5 3.6 c 5 2.9 c 5 3.4 c 5 3.6 c 5 1.6 a 5 3.6 c 5 2.3 c 5 0.2 c 5 1.6 c
6 2 b 6 2.6 a 6 3.9 c 6 3.1 b 6 2 c 6 0.6 b 6 1.4 a 6 0.8 a 6 2.5 c 6 3.3 a
7 3.5 c 7 0.5 a 7 2.4 c 7 3.6 c 7 0.8 c 7 2.4 c 7 3.2 c 7 0 b 7 2.4 a 7 2.4 a
8 0.3 c 8 2.3 c 8 1.2 c 8 2.5 c 8 2 a 8 2.1 c 8 0.9 b 8 3.4 a 8 3 c 8 1 a
9 3.9 a 9 1 c 9 1.7 a 9 2.2 c 9 2.3 c 9 2.3 c 9 3.8 c 9 2.5 c 9 1.8 a 9 0.7 b
10 0.8 c 10 1.3 a 10 0.8 a 10 0.7 c 10 2.3 c 10 0.5 c 10 3.5 c 10 0.9 b 10 1.2 c 10 1 c
11 0.2 b 11 1.4 c 11 3.5 c 11 0.7 a 11 1.9 c 11 4 c 11 3.7 c 11 3.6 b 11 2.8 a 11 2.7 a
12 3.2 a 12 3 b 12 3.8 c 12 0.7 b 12 2.6 c 12 1.9 b 12 3.6 c 12 0.8 a 12 0.9 b 12 1.1 b
END DATA.
VARIABLE LEVEL choice (NOMINAL).
";

/// The same cases in every group: `rows` (without the group) repeated for groups 1 to 4.
fn copies(header: &str, rows: &str, tail: &str) -> String {
    let mut out = format!("{header}\nBEGIN DATA\n");
    for g in 1..=4 {
        for row in rows.lines() {
            out.push_str(&format!("{g} {row}\n"));
        }
    }
    out.push_str("END DATA.\n");
    out.push_str(tail);
    out
}

const SAME_ORDINAL: &str = "1 2.3 3\n2 3.1 3\n1 4.3 4\n2 1.7 2\n1 1.6 1\n2 1.2 3\n1 2.8 3\n2 1.2 2\n\
1 4.2 4\n2 2.8 4\n1 4.4 4\n2 4.7 1\n1 3.3 3\n2 4.6 4\n1 3.9 4\n2 1.9 4";

const SAME_NOMINAL: &str = "2.8 c\n1.4 b\n2.1 b\n3.4 c\n0 a\n1.5 a\n0.8 b\n2.6 b\n2.8 c\n0.2 b\n\
1.8 a\n1.5 a\n2.4 c\n3.6 b\n3.5 c\n2.1 a\n2.1 a\n3.6 b\n1.2 b\n0.2 b";

#[test]
fn ordinal_cumulative_logit_with_a_random_intercept() {
    let (engine, tables) = run_engine(&format!(
        "{ORDINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=school /FIELDS TARGET=grade\n\
         /FIXED EFFECTS=x arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=school SOLUTION=TRUE\n\
         /BUILD_OPTIONS PCONVERGE=1E-10 /EMMEANS TABLES=arm\n\
         /SAVE PREDICTED_VALUES(cat) PREDICTED_PROBABILITY(p) CONFIDENCE_INTERVALS(ci)."
    ));
    let summary = table(&tables, "Model Summary");
    assert_eq!(summary.rows[1][1], Cell::Text("Ordinal Multinomial".into()));
    assert_eq!(summary.rows[2][1], Cell::Text("Cumulative logit".into()));
    estimates(
        &tables,
        &[
            (
                "Threshold [grade = 1]",
                -0.58776380294061,
                0.533098281831191,
            ),
            ("Threshold [grade = 2]", 1.06033641336495, 0.521174506507039),
            ("Threshold [grade = 3]", 2.6578055891352, 0.58592050607979),
            ("x", 0.75407792622702, 0.17202636614446),
            ("[arm=1]", -0.354633119173736, 0.393631383763362),
        ],
    );
    let cov = "Estimates of Covariance Parameters";
    close(
        nth(&tables, cov, 0, "Estimate"),
        0.42636059936065,
        "variance",
    );
    close(
        nth(&tables, cov, 0, "Std. Error"),
        0.386326958845806,
        "its se",
    );
    let ic = "Information Criteria";
    close(nth(&tables, ic, 0, "Value"), 1094.12311053564, "-2 RLPL");
    // the thresholds are not tested
    let tests = "Type III Tests of Fixed Effects";
    assert_eq!(table(&tables, tests).rows.len(), 2);
    close(value(&tables, tests, "x", "F"), 19.2150900989586, "F x");
    close(
        value(&tables, tests, "arm", "F"),
        0.811669356307248,
        "F arm",
    );
    close(value(&tables, tests, "arm", "Denominator df"), 91.0, "df");
    let random = "Estimates of Random Effects";
    close(
        nth(&tables, random, 0, "Estimate"),
        -0.122337700299239,
        "school 1",
    );
    close(
        nth(&tables, random, 1, "Estimate"),
        0.489554441980363,
        "school 2",
    );
    // cases 1 and 5: cumulative probabilities and their intervals
    let want = [
        (
            0,
            [0.13644073151133, 0.45089083708081, 0.802246907189807],
            [0.0474251772052016, 0.221958280035495, 0.576075607924224],
            [0.333959884685866, 0.702691306602422, 0.923727915591091],
        ),
        (
            4,
            [0.211264910214878, 0.581949771332975, 0.873055786163034],
            [0.0767435598967738, 0.316458426182763, 0.680989212648156],
            [0.463266857753701, 0.807159940636676, 0.956817688140464],
        ),
    ];
    for (case, cum, lo, hi) in want {
        for j in 0..3 {
            close(saved(&engine, &format!("p_{}", j + 1), case), cum[j], "p");
            close(
                saved(&engine, &format!("ci_Lower_{}", j + 1), case),
                lo[j],
                "lo",
            );
            close(
                saved(&engine, &format!("ci_Upper_{}", j + 1), case),
                hi[j],
                "hi",
            );
        }
        assert_eq!(saved(&engine, "p_4", case), 1.0);
        assert!(engine.dataset().unwrap().variable("ci_Lower_4").is_err());
    }
    // case 1: 0.136, 0.314, 0.351, 0.198, so grade 3; case 5 has grade 2 most likely
    assert_eq!(saved(&engine, "cat", 0), 3.0);
    assert_eq!(saved(&engine, "cat", 4), 2.0);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("EMMEANS is ignored"))
    );
}

const CLOGLOG: [(&str, f64, f64); 5] = [
    (
        "Threshold [grade = 1]",
        -0.919828581621173,
        0.402350618887837,
    ),
    (
        "Threshold [grade = 2]",
        0.433671402308107,
        0.339906143073284,
    ),
    ("Threshold [grade = 3]", 1.54472551694959, 0.359306384879572),
    ("x", 0.553837255355936, 0.117163299411195),
    ("[arm=1]", -0.243114142000935, 0.265697482637268),
];

/// The variance, its standard error and the -2 restricted log pseudo likelihood.
const CLOGLOG_S2: (f64, f64, f64) = (0.256605185135404, 0.208357204550456, 859.924475023831);

#[test]
fn ordinal_complementary_log_log() {
    let tables = run(&format!(
        "{ORDINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=school /FIELDS TARGET=grade\n\
         /TARGET_OPTIONS DISTRIBUTION=MULTINOMIAL LINK=CLOGLOG\n\
         /FIXED EFFECTS=x arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=school\n\
         /BUILD_OPTIONS PCONVERGE=1E-10."
    ));
    let summary = table(&tables, "Model Summary");
    assert_eq!(
        summary.rows[2][1],
        Cell::Text("Cumulative complementary log-log".into())
    );
    estimates(&tables, &CLOGLOG);
    let cov = "Estimates of Covariance Parameters";
    close(nth(&tables, cov, 0, "Estimate"), CLOGLOG_S2.0, "variance");
    close(nth(&tables, cov, 0, "Std. Error"), CLOGLOG_S2.1, "its se");
    close(
        nth(&tables, "Information Criteria", 0, "Value"),
        CLOGLOG_S2.2,
        "-2 RLPL",
    );
}

#[test]
fn nominal_generalized_logit_with_a_random_intercept_per_category() {
    let (engine, tables) = run_engine(&format!(
        "{NOMINAL}GENLINMIXED /DATA_STRUCTURE SUBJECTS=store /FIELDS TARGET=choice\n\
         /FIXED EFFECTS=x2 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=store SOLUTION=TRUE\n\
         /BUILD_OPTIONS PCONVERGE=1E-10\n\
         /SAVE PREDICTED_VALUES(cat) PREDICTED_PROBABILITY(p) CONFIDENCE(conf)\n\
         CONFIDENCE_INTERVALS(ci)."
    ));
    let summary = table(&tables, "Model Summary");
    assert_eq!(summary.rows[1][1], Cell::Text("Nominal Multinomial".into()));
    assert_eq!(summary.rows[2][1], Cell::Text("Generalized logit".into()));
    // c, the last value, is the reference
    estimates(
        &tables,
        &[
            (
                "choice = a: Intercept",
                0.237465565007071,
                0.490491914334931,
            ),
            ("choice = a: x2", -0.442312749892767, 0.203350584274511),
            (
                "choice = b: Intercept",
                0.459763729591169,
                0.511742251825917,
            ),
            ("choice = b: x2", -0.761038264368251, 0.240942561749377),
        ],
    );
    let cov = "Estimates of Covariance Parameters";
    close(
        nth(&tables, cov, 0, "Estimate"),
        0.252117835731447,
        "variance",
    );
    close(
        nth(&tables, cov, 0, "Std. Error"),
        0.317370637487149,
        "its se",
    );
    close(
        nth(&tables, "Information Criteria", 0, "Value"),
        910.386435717094,
        "-2 RLPL",
    );
    let tests = "Type III Tests of Fixed Effects";
    close(
        value(&tables, tests, "Intercept", "F"),
        0.412363033319602,
        "F",
    );
    close(value(&tables, tests, "x2", "F"), 5.72323989929041, "F x2");
    close(value(&tables, tests, "x2", "Numerator df"), 2.0, "df1");
    close(value(&tables, tests, "x2", "Denominator df"), 116.0, "df2");
    let random = "Estimates of Random Effects";
    close(
        nth(&tables, random, 0, "Estimate"),
        -0.0771011789070554,
        "a",
    );
    close(nth(&tables, random, 1, "Estimate"), 0.471811446809093, "b");
    // cases 1 and 7: the probabilities of a, b and c with their intervals
    let want = [
        (
            0,
            [0.207255531119361, 0.183594924153139, 0.609149544727501],
            [0.0477019314945132, 0.0259501369087161, 0.425196634398896],
            [0.366809130744209, 0.341239711397561, 0.793102455056105],
        ),
        (
            6,
            [0.257124194687875, 0.430862750339355, 0.312013054972771],
            [0.058650033788472, 0.187245323532977, 0.133541547343887],
            [0.455598355587277, 0.674480177145732, 0.490484562601654],
        ),
    ];
    for (case, p, lo, hi) in want {
        for j in 0..3 {
            close(saved(&engine, &format!("p_{}", j + 1), case), p[j], "p");
            close(
                saved(&engine, &format!("ci_Lower_{}", j + 1), case),
                lo[j],
                "lo",
            );
            close(
                saved(&engine, &format!("ci_Upper_{}", j + 1), case),
                hi[j],
                "hi",
            );
        }
        close(
            saved(&engine, "conf", case),
            p[0].max(p[1]).max(p[2]),
            "conf",
        );
    }
    let category = |case: usize| match engine.dataset().unwrap().variable("cat").unwrap().values() {
        statscraft_data::Values::String(v) => v.get(case).trim_end().to_string(),
        _ => panic!("a string field"),
    };
    assert_eq!(category(0), "c");
    assert_eq!(category(6), "b");
}

#[test]
fn without_group_differences_the_fit_is_plum_and_nomreg() {
    // every group has the same cases, so the predicted random effects are 0 and the fixed
    // effects are the maximum likelihood estimates of the model without them
    let ord = copies(
        "DATA LIST FREE /school arm x grade.",
        SAME_ORDINAL,
        "VARIABLE LEVEL grade arm (ORDINAL).\n",
    );
    let tables = run(&format!(
        "{ord}GENLINMIXED /DATA_STRUCTURE SUBJECTS=school /FIELDS TARGET=grade\n\
         /FIXED EFFECTS=x arm /RANDOM USE_INTERCEPT=TRUE SUBJECTS=school\n\
         /BUILD_OPTIONS PCONVERGE=1E-10.\n\
         PLUM grade BY arm WITH x /PRINT=PARAMETER."
    ));
    let plum = &table(&tables, "Parameter Estimates").rows;
    let fixed = &table(&tables, "Estimates of Fixed Effects").rows;
    for row in 0..5 {
        let (Cell::Number(a), Cell::Number(b)) = (&fixed[row][1], &plum[row][2]) else {
            panic!("row {row}");
        };
        assert!((a - b).abs() < 1e-6, "row {row}: {a} vs {b}");
    }
    let nom = copies(
        "DATA LIST FREE /store x2 (F8.1) choice (A1).",
        SAME_NOMINAL,
        "",
    );
    let tables = run(&format!(
        "{nom}GENLINMIXED /DATA_STRUCTURE SUBJECTS=store /FIELDS TARGET=choice\n\
         /FIXED EFFECTS=x2 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=store\n\
         /BUILD_OPTIONS PCONVERGE=1E-10.\n\
         NOMREG choice WITH x2 /PRINT=PARAMETER."
    ));
    let nomreg = &table(&tables, "Parameter Estimates").rows;
    let fixed = &table(&tables, "Estimates of Fixed Effects").rows;
    // for the canonical logit the expected information is the observed one, so the standard
    // errors agree too while the variance is near 0
    for row in 0..4 {
        for (ours, theirs) in [(1, 2), (2, 3)] {
            let (Cell::Number(a), Cell::Number(b)) = (&fixed[row][ours], &nomreg[row][theirs])
            else {
                panic!("row {row}");
            };
            assert!(
                (a - b).abs() < 1e-4 * b.abs().max(1.0),
                "row {row}: {a} vs {b}"
            );
        }
    }
#[test]
fn pearson_residual_of_events_and_trials_is_on_the_proportion_scale() {
    // 8 groups of 5 rows drawn once in R (seed 20261118): a covariate x, m trials and r events
    // from a logistic model with a group intercept. The expected numbers come from a separate
    // R program for this check: restricted pseudo likelihood iterated to a relative change of
    // the group variance below 1e-11, then (r/m - mu) / sqrt(mu (1 - mu) / m). On the counts,
    // (r - m mu) / sqrt(mu (1 - mu)), the first residual would be 0.0526968 instead.
    let (engine, _) = run_engine(
        "DATA LIST FREE /grp x r m.
BEGIN DATA
1 1.9 5 6 1 2.5 6 7 1 3.4 13 14 1 1.4 11 13 1 1.3 8 9 2 1 3 8 2 2.2 4 8 2 0.9 6 11
2 3.4 7 14 2 2.3 10 15 3 3.5 7 7 3 3.8 13 15 3 2.6 6 8 3 3.7 10 12 3 3.1 10 11
4 1.5 3 9 4 2.4 8 13 4 3.7 10 10 4 3.8 6 8 4 0.8 2 7 5 2.7 5 9 5 1.2 5 13 5 3 9 13
5 2.5 4 9 5 3.1 11 14 6 0.9 2 5 6 3 7 14 6 3.6 3 5 6 1.5 5 7 6 2.6 5 7 7 3 1 7
7 3 4 9 7 2.5 5 11 7 0.8 4 14 7 1.2 2 14 8 2.1 4 5 8 2.9 9 11 8 1.6 11 14 8 0.2 3 6
8 3.6 5 7
END DATA.
GENLINMIXED /DATA_STRUCTURE SUBJECTS=grp /FIELDS TARGET=r TRIALS=FIELD(m)
 /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=LOGIT /FIXED EFFECTS=x
 /RANDOM USE_INTERCEPT=TRUE SUBJECTS=grp /BUILD_OPTIONS PCONVERGE=1E-12
 /SAVE PREDICTED_VALUES PEARSON_RESIDUALS.",
    );
    // case, predicted events, Pearson residual
    let want = [
        (0, 4.9802069384143, 0.021513391483724),
        (1, 6.0456041888311, -0.050230747136051),
        (16, 8.0404526267228, -0.023097082892074),
        (39, 5.8822038301413, -0.9102624246984),
    ];
    for (case, events, r) in want {
        close(saved(&engine, "PredictedValue", case), events, "predicted");
        close(
            saved(&engine, "PearsonResidual", case),
            r,
            "Pearson residual",
        );
    }
}
