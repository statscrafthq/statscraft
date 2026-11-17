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

/// The numbers of the `index`-th row of a table, by column title.
fn nth(tables: &[Table], title: &str, index: usize, column: &str) -> f64 {
    let t = table(tables, title);
    let c = t
        .column_titles()
        .iter()
        .position(|h| h == column)
        .expect("column");
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
            "without RANDOM",
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
                "{COUNTS}GENLINMIXED /DATA_STRUCTURE SUBJECTS=person /FIELDS TARGET=visits /RANDOM USE_INTERCEPT=TRUE SUBJECTS=person."
            ),
            "not supported yet",
        ),
    ];
    for (source, part) in cases {
        let message = run_err(&source);
        assert!(message.contains(part), "{message}");
    }
}
