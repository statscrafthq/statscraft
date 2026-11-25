//! CNLR on the 12 invented points of tests/nlr.rs, model a x / (b + x), against R. Where a
//! constraint is active at the optimum, the constraint is substituted into the model and the
//! one-parameter least squares problem solved in R with optimize() to 1e-14 (R has no
//! constrained solver of its own that is exact); the pseudo-Huber loss fit is the root of the
//! gradient by Newton steps from numDeriv. Derivatives are given, so the solver's gradient is
//! exact; the criteria are set tight so the stop does not decide the comparison.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y.
BEGIN DATA
0.5 2.21  1 2.495  1.5 3.48  2 4.215  3 5.035  4 5.701
5 6.606  6 6.756  8 7.346  10 8.209  12 8.006  15 8.869
END DATA.
";

const MODEL: &str = "MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
    DERIVATIVES.\nCOMPUTE D.a = x / (b + x).\nCOMPUTE D.b = -a * x / (b + x)**2.\n";

const TIGHT: &str = "/CRITERIA=OPTOLERANCE(1E-24) FPRECISION(1E-26) ITER(300) MINORITERATION(300)";

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

fn estimate(tables: &[Table], name: &str) -> f64 {
    let t = tables
        .iter()
        .find(|t| t.title == "Parameter Estimates")
        .expect("estimates");
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
        .expect("row");
    match row[1] {
        Cell::Number(n) => n,
        _ => panic!("no number"),
    }
}

fn residual_ss(tables: &[Table]) -> f64 {
    let t = tables
        .iter()
        .find(|t| t.title == "Analysis of Variance")
        .expect("anova");
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == "Residual"))
        .expect("row");
    match row[2] {
        Cell::Number(n) => n,
        _ => panic!("no number"),
    }
}

#[test]
fn an_active_bound_agrees_with_r() {
    let t = run(&format!("{MODEL}CNLR y /BOUNDS b <= 2.5 {TIGHT}."));
    near(estimate(&t, "a"), 9.79030086872863, "a");
    near(estimate(&t, "b"), 2.5, "b");
    near(residual_ss(&t), 1.10101781870932, "SSE");
    // the bound a loose one: the unconstrained least squares estimates
    let t = run(&format!("{MODEL}CNLR y /BOUNDS b <= 9; a >= 0 {TIGHT}."));
    near(estimate(&t, "a"), 10.2670933528832, "a");
    near(estimate(&t, "b"), 2.9314961966467, "b");
}

#[test]
fn a_linear_equality_and_a_linear_inequality_agree_with_r() {
    let t = run(&format!("{MODEL}CNLR y /BOUNDS a = 4 * b {TIGHT}."));
    near(estimate(&t, "b"), 2.44349631614595, "b");
    near(estimate(&t, "a"), 9.77398526458382, "a");
    near(residual_ss(&t), 1.18007948546982, "SSE");
    // a - 3 b <= 0 is active at the optimum (the free estimates give 1.47)
    let t = run(&format!("{MODEL}CNLR y /BOUNDS a - 3 * b <= 0 {TIGHT}."));
    near(estimate(&t, "b"), 3.65015066982343, "b");
    near(estimate(&t, "a"), 10.9504520094703, "a");
    near(residual_ss(&t), 1.32231881417926, "SSE");
    // the chain form of two relations: 2 <= b <= 2.5 puts b on 2.5
    let t = run(&format!("{MODEL}CNLR y /BOUNDS 2 <= b <= 2.5 {TIGHT}."));
    near(estimate(&t, "b"), 2.5, "b");
}

#[test]
fn a_nonlinear_constraint_agrees_with_r() {
    let t = run(&format!(
        "{MODEL}CONSTRAINED FUNCTIONS.\nCOMPUTE cf = a - 2 * b**2.\nCNLR y /BOUNDS cf GE 0 {TIGHT}."
    ));
    near(estimate(&t, "b"), 2.17306141784803, "b");
    near(estimate(&t, "a"), 9.4443918514794, "a");
    near(residual_ss(&t), 1.6637790983734, "SSE");
}

#[test]
fn a_loss_function_agrees_with_r() {
    // pseudo-Huber loss with delta 0.5, derivatives of the loss given
    let program = "MODEL PROGRAM a=8 b=1.\nCOMPUTE pred = a * x / (b + x).\n\
        COMPUTE r = y - pred.\nCOMPUTE loss = 0.25 * (SQRT(1 + (r / 0.5)**2) - 1).\n\
        DERIVATIVES.\nCOMPUTE D.a = -(r / SQRT(1 + (r / 0.5)**2)) * x / (b + x).\n\
        COMPUTE D.b = (r / SQRT(1 + (r / 0.5)**2)) * a * x / (b + x)**2.\n";
    let t = run(&format!("{program}CNLR y /LOSS=loss {TIGHT}."));
    let h = t
        .iter()
        .find(|t| t.title == "Iteration History")
        .expect("history");
    let caption = h.caption.clone().expect("caption");
    assert!(caption.contains("Loss function: loss"), "{caption}");
    let last = h.rows.last().expect("rows");
    let numbers: Vec<f64> = last
        .iter()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect();
    near(numbers[1], 0.358555330604953, "loss");
    near(numbers[2], 10.333584722977, "a");
    near(numbers[3], 3.00558365806179, "b");
    // without the squared loss no analysis of variance is shown
    assert!(!t.iter().any(|t| t.title == "Analysis of Variance"));
}

#[test]
fn an_unconstrained_fit_of_another_model_agrees_with_r() {
    let t = run(&format!(
        "MODEL PROGRAM a=3 b=-0.45.\nCOMPUTE pred = a * LN(x + b).\nDERIVATIVES.\n\
         COMPUTE D.a = LN(x + b).\nCOMPUTE D.b = a / (x + b).\nCNLR y {TIGHT}."
    ));
    near(estimate(&t, "a"), 3.27355897107, "a");
    near(estimate(&t, "b"), 1.44461087127, "b");
    near(residual_ss(&t), 0.946467679841551, "SSE");
}

#[test]
fn misuse_is_refused() {
    assert!(
        fails(&format!("{MODEL}CNLR y /BOUNDS c <= 1."))
            .contains("neither a parameter nor a constrained function")
    );
    assert!(
        fails(&format!(
            "{MODEL}CONSTRAINED FUNCTIONS.\nCOMPUTE cf = a - b.\nCNLR y /BOUNDS cf + a >= 0."
        ))
        .contains("cannot be combined")
    );
    assert!(
        fails(&format!("{MODEL}CNLR y /BOUNDS b >= 5; b <= 4.")).contains("exclude every value")
    );
    assert!(
        fails(&format!(
            "{MODEL}CONSTRAINED FUNCTIONS.\nCOMPUTE cf = a - b.\nNLR y."
        ))
        .contains("belongs to CNLR")
    );
    assert!(fails(&format!("{MODEL}CNLR y /LOSS=nothing.")).contains("does not compute nothing"));
}
