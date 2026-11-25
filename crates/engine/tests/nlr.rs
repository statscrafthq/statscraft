//! NLR against R on 12 invented points of a saturating curve. The model of the first tests is
//! a x / (b + x), the last a (1 - exp(-k x)); the weights are invented too. The optimum comes
//! from a Gauss-Newton iteration written out in R with the analytic Jacobian, run to 1e-15 (nls()
//! agrees with it to 1e-9 where its own criterion lets it); the standard errors, the 95% intervals
//! (t with n - p degrees of freedom), the sums of squares of the analysis of variance (the
//! corrected total about the weighted mean) and the correlation of the two estimates are written
//! out from it. The runs give the criteria that stop NLR tight values, so the stop does not
//! decide the comparison: NLR must agree on every number to 1e-6.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y w g.
BEGIN DATA
0.5 2.21 1 1  1 2.495 1 1  1.5 3.48 2 1  2 4.215 2 1  3 5.035 1 1  4 5.701 1 1
5 6.606 0.5 2  6 6.756 0.5 2  8 7.346 1 2  10 8.209 1 2  12 8.006 2 2  15 8.869 2 2
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn fails(syntax: &str) -> String {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.0;
        }
    }
    panic!("no error");
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-6 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The numbers of the row labelled `label` of the table `title`.
fn row(tables: &[Table], title: &str, label: &str) -> Vec<f64> {
    let t = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label} in {title}"));
    row.iter()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

struct Want {
    estimates: [f64; 2],
    se: [f64; 2],
    lower: [f64; 2],
    upper: [f64; 2],
    sse: f64,
    ss_regression: f64,
    ss_uncorrected: f64,
    ss_corrected: f64,
    r_square: f64,
    correlation: f64,
}

fn check(tables: &[Table], names: [&str; 2], want: &Want) {
    for (k, name) in names.iter().enumerate() {
        let r = row(tables, "Parameter Estimates", name);
        near(r[0], want.estimates[k], "estimate");
        near(r[1], want.se[k], "standard error");
        near(r[2], want.lower[k], "lower bound");
        near(r[3], want.upper[k], "upper bound");
    }
    let a = "Analysis of Variance";
    let regression = row(tables, a, "Regression");
    assert_eq!(regression[0], 2.0);
    near(regression[1], want.ss_regression, "regression SS");
    let residual = row(tables, a, "Residual");
    assert_eq!(residual[0], 10.0);
    near(residual[1], want.sse, "residual SS");
    near(residual[2], want.sse / 10.0, "residual MS");
    let uncorrected = row(tables, a, "Uncorrected Total");
    assert_eq!(uncorrected[0], 12.0);
    near(uncorrected[1], want.ss_uncorrected, "uncorrected SS");
    let corrected = row(tables, a, "Corrected Total");
    assert_eq!(corrected[0], 11.0);
    near(corrected[1], want.ss_corrected, "corrected SS");
    let caption = tables
        .iter()
        .find(|t| t.title == a)
        .and_then(|t| t.caption.clone())
        .expect("caption");
    assert!(
        caption.contains(&format!("{:.3}", want.r_square)),
        "{caption}"
    );
    let corr = row(
        tables,
        "Asymptotic Correlation Matrix of the Parameter Estimates",
        names[0],
    );
    near(corr[0], 1.0, "correlation with itself");
    near(corr[1], want.correlation, "correlation");
}

const MM: Want = Want {
    estimates: [10.2670933528832, 2.9314961966467],
    se: [0.33869772610108, 0.28369896547377],
    lower: [9.512427790278, 2.2993755094063],
    upper: [11.021758915488, 3.5636168838871],
    sse: 0.87838497382554,
    ss_regression: 451.34935702617,
    ss_uncorrected: 452.227742,
    ss_corrected: 56.30531,
    r_square: 0.984399606825,
    correlation: 0.9061838188068,
};

#[test]
fn numerical_derivatives_agree_with_r() {
    let t = run(
        "MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nNLR y /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    check(&t, ["a", "b"], &MM);
}

#[test]
fn derivatives_given_by_the_program_agree_with_r() {
    let t = run(
        "MODEL PROGRAM a=10 b=2.\nCOMPUTE pred = a * x / (b + x).\nDERIVATIVES.\n\
         COMPUTE D.a = x / (b + x).\nCOMPUTE D.b = -a * x / (b + x)**2.\nNLR y /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    check(&t, ["a", "b"], &MM);
    // only one of the two given, the other estimated: the same answer
    let t = run(
        "MODEL PROGRAM a=10 b=2.\nCOMPUTE pred = a * x / (b + x).\nDERIVATIVES.\n\
         COMPUTE D.b = -a * x / (b + x)**2.\nNLR y /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    check(&t, ["a", "b"], &MM);
}

#[test]
fn case_weights_agree_with_r_and_leave_the_degrees_of_freedom_alone() {
    let t = run(
        "WEIGHT BY w.\nMODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nNLR y /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    check(
        &t,
        ["a", "b"],
        &Want {
            estimates: [10.2919130047145, 2.9442451737641],
            se: [0.28311261252749, 0.24487125049695],
            lower: [9.6610987932547, 2.3986380267974],
            upper: [10.922727216174, 3.4898523207308],
            sse: 0.99991163282538,
            ss_regression: 579.21826636717,
            ss_uncorrected: 580.218178,
            ss_corrected: 77.7387454,
            r_square: 0.987137538332,
            correlation: 0.87855811044157,
        },
    );
}

#[test]
fn another_model_and_a_name_for_the_predicted_values() {
    let t = run(
        "MODEL PROGRAM a=10 k=0.3.\nCOMPUTE fit = a * (1 - EXP(-k * x)).\nNLR y /PRED=fit /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    let r = row(&t, "Parameter Estimates", "a");
    near(r[0], 8.32898109075922, "a");
    near(r[1], 0.278387691802580, "se of a");
    let r = row(&t, "Parameter Estimates", "k");
    near(r[0], 0.32412854818159, "k");
    near(r[1], 0.031237062045977, "se of k");
    near(
        row(&t, "Analysis of Variance", "Residual")[1],
        1.9532951368954,
        "residual SS",
    );
}

#[test]
fn saved_values_agree_with_r() {
    let t = run("MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
         NLR y /SAVE=PRED RESID(res1) DERIVATIVES /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).\n\
         COMPUTE check = res1 + pred - y.\nDESCRIPTIVES check PRED res1 D.a D.b /STATISTICS=MEAN MIN MAX.");
    let d = t
        .iter()
        .find(|t| t.title == "Descriptive Statistics")
        .expect("descriptives");
    let cell = |label: &str, at: usize| match &d
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .expect("row")[at]
    {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    };
    // columns: Variable, Label, N, Mean, Minimum, Maximum
    near(cell("PRED", 4), 1.4960082664402, "smallest prediction");
    near(cell("PRED", 5), 8.588597326421, "largest prediction");
    near(cell("res1", 4), -0.24535797591859, "smallest residual");
    near(cell("res1", 5), 0.71399173355978, "largest residual");
    assert!(cell("check", 4).abs() < 1e-9 && cell("check", 5).abs() < 1e-9);
    // the derivative of the fitted curve with respect to a at the first and the last case, and
    let b = 2.9314961966467;
    near(cell("D.a", 4), 0.5 / (b + 0.5), "D.a at x = 0.5");
    near(cell("D.a", 5), 15.0 / (b + 15.0), "D.a at x = 15");
    // the smallest derivative with respect to b falls at the case x = 3
    near(
        cell("D.b", 4),
        -10.2670933528832 * 3.0 / (b + 3.0).powi(2),
        "D.b at x = 3",
    );
}

#[test]
fn split_groups_start_from_the_file_and_write_one_case_each() {
    let t = run(
        "DATASET NAME data.\nDATASET DECLARE start.\nDATASET DECLARE final.\n\
         MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
         NLR y /OUTFILE=start /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).\n\
         SPLIT FILE BY g.\n\
         MODEL PROGRAM a b=1.\nCOMPUTE pred = a * x / (b + x).\n\
         NLR y /FILE=start /OUTFILE=final /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).\n\
         SPLIT FILE OFF.\nDATASET ACTIVATE final.\n\
         DESCRIPTIVES a b SSE NCASES /STATISTICS=MEAN MIN MAX.",
    );
    // the groups share one layered table, the group value in front of the first row of each
    let layered = t
        .iter()
        .filter(|t| t.title == "Parameter Estimates")
        .nth(1)
        .expect("the table of the groups");
    let numbers: Vec<Vec<f64>> = layered
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .filter_map(|c| match c {
                    Cell::Number(n) => Some(*n),
                    _ => None,
                })
                .collect()
        })
        .collect();
    assert_eq!(numbers.len(), 4);
    near(numbers[0][0], 8.3827562295417, "a of group 1");
    near(numbers[1][0], 1.9815887281473, "b of group 1");
    near(numbers[2][0], 10.5184976681652, "a of group 2");
    near(numbers[3][0], 3.1847312686696, "b of group 2");
    // a started from the file: the first iteration of the groups is the estimate of all cases
    let history = t
        .iter()
        .filter(|t| t.title == "Iteration History")
        .nth(1)
        .expect("history of the groups");
    match &history.rows[0][3] {
        Cell::Number(a) => near(*a, 10.2670933528832, "a at the start of group 1"),
        other => panic!("{other:?}"),
    }
    // the file the groups wrote: one case each with the parameters, SSE and NCASES
    let d = t
        .iter()
        .rfind(|t| t.title == "Descriptive Statistics")
        .expect("descriptives");
    let cell = |label: &str, at: usize| match &d
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .expect("row")[at]
    {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    };
    assert_eq!(cell("a", 2), 2.0);
    near(cell("a", 4), 8.3827562295417, "smallest a written");
    near(cell("a", 5), 10.5184976681652, "largest a written");
    near(cell("SSE", 4), 0.26176539820375, "smaller SSE written");
    assert_eq!((cell("NCASES", 4), cell("NCASES", 5)), (6.0, 6.0));
}

#[test]
fn the_stop_and_the_iterations_are_reported() {
    let t =
        run("MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nNLR y /CRITERIA=ITER(2).");
    let h = t
        .iter()
        .find(|t| t.title == "Iteration History")
        .expect("history");
    assert_eq!(h.rows.len(), 3, "the start and two iterations");
    assert!(
        h.caption
            .as_deref()
            .unwrap_or("")
            .contains("maximum number"),
        "{:?}",
        h.caption
    );
}

#[test]
fn misuse_is_refused() {
    assert!(fails("NLR y.").contains("needs a MODEL PROGRAM"));
    assert!(
        fails("MODEL PROGRAM a=1.\nCOMPUTE other = a * x.\nNLR y.")
            .contains("does not compute PRED")
    );
    assert!(
        fails("MODEL PROGRAM a b=1.\nCOMPUTE pred = a * x / (b + x).\nNLR y.")
            .contains("no starting value")
    );
    assert!(fails("MODEL PROGRAM x=1.\nCOMPUTE pred = x.\nNLR y.").contains("name of a variable"));
    assert!(
        fails(
            "MODEL PROGRAM sse=1 b=1.\nCOMPUTE pred = sse * x / (b + x).\nNLR y /OUTFILE=nowhere."
        )
        .contains("cannot be named SSE")
    );
    // a derivative that is wrong stops the run; CKDER(0) switches the check off
    let wrong = "MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nDERIVATIVES.\n\
                 COMPUTE D.a = 3 * x.\nNLR y";
    assert!(fails(&format!("{wrong}.")).contains("disagrees"));
    // the program is used up by a successful NLR
    assert!(
        fails(
            "MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nCLEAR MODEL PROGRAMS.\nNLR y."
        )
        .contains("needs a MODEL PROGRAM")
    );
}

/// The saved variables of every split group land in the same columns of the active data.
#[test]
fn saved_values_of_split_groups_are_kept() {
    let t = run(
        "SPLIT FILE BY g.\nMODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
         NLR y /SAVE=PRED RESID(r) /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).\n\
         SPLIT FILE OFF.\nCOMPUTE back = r + PRED - y.\n\
         DESCRIPTIVES PRED r back /STATISTICS=MEAN MIN MAX.",
    );
    let d = t
        .iter()
        .find(|t| t.title == "Descriptive Statistics")
        .expect("descriptives");
    let cell = |label: &str, at: usize| match &d
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .expect("row")[at]
    {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    };
    // all twelve cases have a value from their own group's fit
    assert_eq!(cell("PRED", 2), 12.0);
    assert_eq!(cell("r", 2), 12.0);
    assert!(cell("back", 4).abs() < 1e-9 && cell("back", 5).abs() < 1e-9);
    // group 1: x = 0.5 with a = 8.3827562295417, b = 1.9815887281473 gives the smallest value
    near(
        cell("PRED", 4),
        8.3827562295417 * 0.5 / (1.9815887281473 + 0.5),
        "smallest prediction",
    );
    // group 2: x = 15 with a = 10.5184976681652, b = 3.1847312686696 gives the largest
    near(
        cell("PRED", 5),
        10.5184976681652 * 15.0 / (3.1847312686696 + 15.0),
        "largest prediction",
    );
}

/// A label and the formats of the predicted values given inside the model program go to the
/// saved variables: the prediction takes both, the residuals and the derivatives the format.
#[test]
fn format_and_label_of_the_predicted_values_reach_the_saved_variables() {
    let t = run("MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
         VARIABLE LABELS pred 'Fitted response'.\nFORMATS pred (F8.4).\n\
         NLR y /SAVE=PRED RESID(r) DERIVATIVES.\n\
         DISPLAY DICTIONARY /VARIABLES=PRED r D.a.");
    let text = t
        .iter()
        .flat_map(|t| t.rows.iter().flatten())
        .filter_map(|c| match c {
            Cell::Text(s) => Some(s.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|");
    assert!(text.contains("Fitted response"), "{text}");
    assert!(text.matches("F8.4").count() >= 3, "{text}");
}

/// A program that recomputes the weight from the parameters: the weights are those of the
/// model program at every trial. R: Gauss-Newton for the sum of w(theta) (y - f)^2 with
/// w = 1 / f (a Poisson-like weighting), the derivative of w included in the Jacobian as the
/// manual has it.
#[test]
fn a_weight_computed_from_the_parameters_agrees_with_r() {
    let t = run(
        "WEIGHT BY w.\nMODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\nCOMPUTE w = 1 / pred.\n\
         NLR y /CRITERIA=SSCON(0) PCON(1E-10) RCON(0).",
    );
    let a = row(&t, "Parameter Estimates", "a");
    let b = row(&t, "Parameter Estimates", "b");
    near(a[0], 9.82499463854944, "a");
    near(b[0], 2.52034145159067, "b");
}
