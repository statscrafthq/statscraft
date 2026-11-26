//! LOGISTIC REGRESSION against R, the parts the first R test (logistic_r.rs, same 120 cases) does
//! not reach: every kind of categorical contrast, and the stepwise methods with each removal
//! statistic. The scripts are in lab/oracles/logistic-r.
//!
//! Contrasts: a four-level factor `ag` (age cut at 35, 45 and 55) fitted with `glm` on the
//! columns of each contrast written from its definition (the effect of a category against the
//! reference, the mean of the others, the next one and so on; the coding of a category is the
//! row of the inverse of the matrix of the contrasts with the mean row on top; polynomial
//! columns from `poly`).
//!
//! Stepwise: R carries out the manual's steps (score statistic to enter; Wald, likelihood ratio
//! or the conditional statistic, with the conditional estimates of the rest given the removed
//! parameter at 0, to remove), with the stop rule of a model met before. The expected numbers are
//! those of the steps: the scores, the removal statistics and the final fit.

use super::logistic_r::DATA;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!(
        "{DATA}COMPUTE ag = 1 + (age > 35) + (age > 45) + (age > 55).\nCOMPUTE z = MOD(age * 7 + inc * 3, 11).\nCOMPUTE q = inc + MOD(age, 5) * 3.\n{syntax}"
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The rows of every table of that title in block 1 (or only the table of the last step),
/// each as (step, row label, the numbers that follow).
fn steps(tables: &[Table], title: &str) -> Vec<(usize, String, Vec<Option<f64>>)> {
    let mut out = Vec::new();
    for t in tables.iter().filter(|t| t.title == title) {
        if t.caption.as_deref().is_some_and(|c| c.contains("Block 0")) {
            continue;
        }
        let mut step = 0;
        for r in &t.rows {
            if let Cell::Text(s) = &r[0]
                && let Some(n) = s.strip_prefix("Step ")
            {
                step = n.parse().unwrap();
            }
            let label = match &r[1] {
                Cell::Text(s) => s.clone(),
                _ => continue,
            };
            let numbers = r[2..]
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => Some(*n),
                    _ => None,
                })
                .collect();
            out.push((step, label, numbers));
        }
    }
    out
}

fn row<'a>(
    rows: &'a [(usize, String, Vec<Option<f64>>)],
    step: usize,
    label: &str,
) -> &'a [Option<f64>] {
    &rows
        .iter()
        .find(|(s, l, _)| *s == step && l == label)
        .unwrap_or_else(|| panic!("no row {label} in step {step}"))
        .2
}

#[test]
fn every_contrast_agrees_with_the_glm_on_its_columns() {
    // (keyword, the three coefficients of ag, their standard errors); inc is 0.046390733
    // (0.015617679) and the constant moves with the coding
    let cases: [(&str, [f64; 3], [f64; 3], f64); 12] = [
        (
            "INDICATOR",
            [-0.970307093, -0.738966022, 1.068722734],
            [0.609866955, 0.546008623, 0.573484182],
            0.0,
        ),
        (
            "INDICATOR(2)",
            [-0.231341071, 1.807688756, 0.738966022],
            [0.609313576, 0.562395035, 0.546008623],
            -2.701821252,
        ),
        (
            "DEVIATION",
            [-0.810169498, -0.578828427, 1.228860329],
            [0.396930468, 0.339548351, 0.364755649],
            -2.122992825,
        ),
        (
            "DEVIATION(2)",
            [-0.810169498, 1.228860329, 0.160137595],
            [0.396930468, 0.364755649, 0.344399089],
            -2.122992825,
        ),
        (
            "SIMPLE",
            [-0.970307093, -0.738966022, 1.068722734],
            [0.609866955, 0.546008623, 0.573484182],
            0.0,
        ),
        (
            "SIMPLE(1)",
            [0.231341071, 2.039029827, 0.970307093],
            [0.609313576, 0.641497269, 0.609866955],
            -2.122992825,
        ),
        (
            "DIFFERENCE",
            [0.231341071, 1.923359291, 0.213516794],
            [0.609313576, 0.520660832, 0.459198786],
            -2.122992825,
        ),
        (
            "HELMERT",
            [-1.080225997, -1.273327389, 1.068722734],
            [0.529240624, 0.474326648, 0.573484182],
            -2.122992825,
        ),
        (
            "REPEATED",
            [-0.231341071, -1.807688756, 1.068722734],
            [0.609313576, 0.562395035, 0.573484182],
            -2.122992825,
        ),
        (
            "POLYNOMIAL",
            [1.055113280, -0.650031903, -0.995667220],
            [0.429663738, 0.424725036, 0.399377028],
            -2.122992825,
        ),
        (
            "POLYNOMIAL(1,2,4,8)",
            [0.788659550, -1.256361301, -0.571603986],
            [0.410703836, 0.438728651, 0.404158168],
            -2.122992825,
        ),
        (
            "SPECIAL(1 1 1 1, 3 -1 -1 -1, 0 1 -1 0, 0 0 1 -1)",
            [-3.240677992, -1.807688756, 1.068722734],
            [1.587721873, 0.562395035, 0.573484182],
            -2.122992825,
        ),
    ];
    for (keyword, b, se, constant) in cases {
        let t = run(&format!(
            "LOGISTIC REGRESSION y WITH inc ag /CATEGORICAL=ag /CONTRAST(ag)={keyword} /CRITERIA=BCON(1E-12)."
        ));
        let rows = steps(&t, "Variables in the Equation");
        let last = rows.iter().map(|r| r.0).max().unwrap();
        let inc = row(&rows, last, "inc");
        near(inc[0].unwrap(), 0.046390733, 1e-6, keyword);
        near(inc[1].unwrap(), 0.015617679, 1e-6, keyword);
        for (j, name) in ["ag(1)", "ag(2)", "ag(3)"].iter().enumerate() {
            let values = row(&rows, last, name);
            near(values[0].unwrap(), b[j], 1e-6, &format!("{keyword} {name}"));
            near(
                values[1].unwrap(),
                se[j],
                1e-6,
                &format!("{keyword} {name} se"),
            );
        }
        // the constant is the one of the model without the coding's shift only when the coding
        // does not move it: the indicator codings put the reference at 0
        if constant != 0.0 {
            near(
                row(&rows, last, "Constant")[0].unwrap(),
                constant,
                1e-6,
                keyword,
            );
        }
    }
}

/// The statistic, significance and log likelihood of each of four removals.
type Removals = [(f64, f64, f64); 4];

const VARS: &str = "age inc ag z reg /CATEGORICAL=ag reg";

#[test]
fn forward_stepwise_with_each_removal_statistic() {
    // the scores after reg has entered (reg's own score at the start was 13.297549, the largest
    // significance): age, inc, ag (3 df) and z
    let scores = [
        ("age", 8.913756),
        ("inc", 9.543419),
        ("ag", 14.086743),
        ("z", 2.808875),
    ];
    // the removal statistics of the four variables of the last step (reg, inc, ag, z) under
    // each statistic: statistic, significance and, where a model is fitted, its log likelihood
    let last_step: [(&str, Removals); 3] = [
        (
            "COND",
            [
                (19.134785, 0.000070, -69.071134),
                (14.509870, 0.000139, -66.758676),
                (21.341268, 0.000089, -70.174376),
                (4.031820, 0.044650, -61.519651),
            ],
        ),
        (
            "LR",
            [
                (18.386469, 0.000102, -68.69698),
                (13.922679, 0.000190, -66.46508),
                (20.335846, 0.000145, -69.67166),
                (3.982041, 0.045988, -61.49476),
            ],
        ),
        (
            "WALD",
            [
                (14.311457, 0.000780, f64::NAN),
                (11.564307, 0.000672, f64::NAN),
                (16.280207, 0.000993, f64::NAN),
                (3.750175, 0.052802, f64::NAN),
            ],
        ),
    ];
    for (test, expected) in last_step {
        let t = run(&format!(
            "LOGISTIC REGRESSION y WITH {VARS} /METHOD=FSTEP({test}) /CRITERIA=PIN(.15) POUT(.10) BCON(1E-12)."
        ));
        // entered in the order reg, inc, ag, z; age (score 1.074 at the end) never enters
        let equation = t
            .iter()
            .rfind(|t| t.title == "Variables in the Equation")
            .unwrap();
        assert!(
            equation
                .caption
                .as_deref()
                .unwrap()
                .contains("entered on step 4: z"),
            "{test}"
        );
        let outside = steps(&t, "Variables not in the Equation");
        for (name, score) in scores {
            near(row(&outside, 1, name)[0].unwrap(), score, 1e-6, "score");
        }
        near(
            row(&outside, 4, "age")[0].unwrap(),
            1.074034,
            1e-6,
            "last score",
        );
        // the last step's statistics
        let (removal, equation_rows) = (
            steps(&t, "Model if Term Removed"),
            steps(&t, "Variables in the Equation"),
        );
        for (variable, (stat, sig, ll)) in ["reg", "inc", "ag", "z"].iter().zip(expected) {
            if test == "WALD" {
                // the Wald statistic and its significance are in the equation
                let r = row(&equation_rows, 4, variable);
                near(r[2].unwrap(), stat, 1e-6, variable);
                near(r[4].unwrap(), sig, 2e-3, variable);
            } else {
                let r = row(&removal, 4, variable);
                near(r[0].unwrap(), ll, 1e-6, variable);
                near(r[1].unwrap(), stat, 1e-6, variable);
                near(r[3].unwrap(), sig, 2e-3, variable);
            }
        }
        // the final equation has the same coefficients whatever the removal statistic
        near(
            row(&equation_rows, 4, "inc")[0].unwrap(),
            0.064741,
            1e-5,
            "inc",
        );
        near(row(&equation_rows, 4, "z")[0].unwrap(), 0.162097, 1e-5, "z");
    }
}

#[test]
fn backward_stepwise_with_each_removal_statistic() {
    // the significance of the removal of age, inc, ag, z and reg from the full model, then of
    // inc, ag, z and reg after age has left (age's score then is 1.074, p 0.300, so it stays out)
    let sigs: [(&str, [f64; 5], [f64; 4]); 3] = [
        (
            "LR",
            [0.297447, 0.000146, 0.014228, 0.041356, 0.000060],
            [0.000190, 0.000145, 0.045988, 0.000102],
        ),
        (
            "COND",
            [0.296932, 0.000105, 0.012816, 0.040006, 0.000036],
            [0.000139, 0.000089, 0.044650, 0.000070],
        ),
        (
            "WALD",
            [0.303227, 0.000565, 0.025757, 0.048301, 0.000745],
            [0.000672, 0.000993, 0.052802, 0.000780],
        ),
    ];
    for (test, first, second) in sigs {
        let t = run(&format!(
            "LOGISTIC REGRESSION y WITH {VARS} /METHOD=BSTEP({test}) /CRITERIA=PIN(.15) POUT(.10) BCON(1E-12)."
        ));
        let equation = steps(&t, "Variables in the Equation");
        let removal = steps(&t, "Model if Term Removed");
        // the significance sits in the last column of either table
        let sig = |step: usize, name: &str| -> f64 {
            if test == "WALD" {
                row(&equation, step, name)[4].unwrap()
            } else {
                row(&removal, step, name)[3].unwrap()
            }
        };
        for (name, want) in ["age", "inc", "ag", "z", "reg"].iter().zip(first) {
            near(sig(1, name), want, 2e-3, &format!("{test} step 1 {name}"));
        }
        for (name, want) in ["inc", "ag", "z", "reg"].iter().zip(second) {
            near(sig(2, name), want, 2e-3, &format!("{test} step 2 {name}"));
        }
        let outside = steps(&t, "Variables not in the Equation");
        near(
            row(&outside, 2, "age")[0].unwrap(),
            1.074034,
            1e-6,
            "age score",
        );
        let last = t
            .iter()
            .rfind(|t| t.title == "Variables in the Equation")
            .unwrap();
        assert!(
            last.caption
                .as_deref()
                .unwrap()
                .contains("removed on step 2: age"),
            "{test}"
        );
    }
}

#[test]
fn a_removal_that_would_repeat_a_model_stops_the_forward_steps() {
    // inc, age and z enter; z (p about 0.06) is then above POUT = .05, but the model without it
    // has been fitted before, so the steps stop and z stays
    let removal = [("WALD", 0.060837), ("LR", 0.056634), ("COND", 0.056214)];
    for (test, sig) in removal {
        let t = run(&format!(
            "LOGISTIC REGRESSION y WITH age inc q z /METHOD=FSTEP({test}) /CRITERIA=PIN(.25) POUT(.05) BCON(1E-12)."
        ));
        let equation = steps(&t, "Variables in the Equation");
        assert_eq!(
            equation
                .iter()
                .filter(|r| r.1 == "z")
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            [3],
            "{test}"
        );
        let r = row(&equation, 3, "z");
        near(r[0].unwrap(), 0.128911, 1e-5, "z B");
        near(r[1].unwrap(), 0.068765, 1e-5, "z se");
        near(row(&equation, 3, "inc")[0].unwrap(), 0.035815, 1e-5, "inc");
        near(row(&equation, 3, "age")[0].unwrap(), 0.046133, 1e-5, "age");
        near(
            row(&equation, 3, "Constant")[0].unwrap(),
            -4.443965,
            1e-5,
            "constant",
        );
        if test == "WALD" {
            near(r[4].unwrap(), sig, 2e-3, "z Wald sig");
        } else {
            let removal = steps(&t, "Model if Term Removed");
            near(
                row(&removal, 3, "z")[3].unwrap(),
                sig,
                2e-3,
                "z removal sig",
            );
        }
    }
}
