//! LOGLINEAR on small invented tables entered as weighted cells. The expected numbers come from
//! R: a Poisson regression with a row indicator for the multinomial constraint, fitted by
//! iteratively reweighted least squares to 1e-15 and read off in the contrast coding of each
//! test (explicit columns for the contrasts; `A:B` interactions as products). Standard errors,
//! hat values, adjusted residuals, generalized residuals and the dispersion measures are
//! computed from the final fitted values by the formulas of the Algorithms manual.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const T1: &str = "DATA LIST FREE /a b n.
BEGIN DATA
1 1 14  1 2 7  1 3 5
2 1 6   2 2 15 2 3 10
3 1 4   3 2 8  3 3 16
END DATA.
WEIGHT BY n.
";

const TIGHT: &str = "/CRITERIA=CONVERGE(1E-12) ITERATE(100)";

fn run(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn fails(data: &str, syntax: &str) -> String {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
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

fn nums(row: &[Cell]) -> Vec<f64> {
    row.iter()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn all<'a>(tables: &'a [Table], title: &str) -> Vec<&'a Table> {
    tables.iter().filter(|t| t.title == title).collect()
}

/// The coefficient and standard error of every parameter of a Parameter Estimates table.
fn estimates(table: &Table) -> Vec<(f64, f64)> {
    table
        .rows
        .iter()
        .map(|r| {
            let n = nums(r);
            // parameter, coefficient, standard error, z, lower, upper
            (n[1], n[2])
        })
        .collect()
}

fn check_estimates(table: &Table, want: &[(f64, f64)], what: &str) {
    let got = estimates(table);
    assert_eq!(got.len(), want.len(), "{what}: parameters");
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        near(g.0, w.0, &format!("{what}: coefficient {}", k + 1));
        near(g.1, w.1, &format!("{what}: standard error {}", k + 1));
    }
}

/// Likelihood ratio, Pearson, df from the Goodness-of-Fit table.
fn fit_of(table: &Table) -> (f64, f64, f64) {
    let lr = nums(&table.rows[0]);
    let pearson = nums(&table.rows[1]);
    (lr[0], pearson[0], lr[1])
}

#[test]
fn independence_agrees_with_r() {
    let t = run(
        T1,
        &format!("LOGLINEAR a b (1,3) {TIGHT} /PRINT=DEFAULT ESTIM COR /DESIGN=a b."),
    );
    check_estimates(
        find(&t, "Parameter Estimates"),
        &[
            (-0.0833328795391290, 0.156992039457267),
            (0.0925577869245355, 0.150262023826655),
            (-0.1596923084838035, 0.160643865780500),
            (0.0634512428304063, 0.151752014270406),
        ],
        "independence",
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 15.9027075753725, "likelihood ratio");
    near(pearson, 16.7664349178397, "Pearson");
    assert_eq!(df, 4.0);
    // expected counts, raw, standardized and adjusted residuals of the nine cells
    let cells = find(&t, "Cell Counts and Residuals");
    assert_eq!(cells.rows.len(), 9);
    let mu = [
        7.34117647058823,
        9.17647058823529,
        9.48235294117647,
        8.75294117647058,
        10.94117647058823,
        11.30588235294118,
        7.90588235294118,
        9.88235294117647,
        10.21176470588235,
    ];
    let std = [
        2.457619665307511,
        -0.718480464577069,
        -1.455619925658987,
        -0.930507604631492,
        1.227066664701025,
        -0.388375497062433,
        -1.3891334985345,
        -0.598784950852095,
        1.811322480908765,
    ];
    let adj = [
        3.482111519375399,
        -1.07207838350048,
        -2.192017575083186,
        -1.378088330186374,
        1.913854241576239,
        -0.611331800931607,
        -2.002444853783208,
        -0.909014958473866,
        2.775111232580563,
    ];
    for (k, row) in cells.rows.iter().enumerate() {
        // observed count, observed %, expected count, expected %, residual, std., adj.
        let n = nums(row);
        let v: Vec<f64> = n[n.len() - 7..].to_vec();
        near(v[2], mu[k], "expected");
        near(v[4], v[0] - mu[k], "residual");
        near(v[5], std[k], "standardized residual");
        near(v[6], adj[k], "adjusted residual");
    }
    // the correlation of the estimates: A1 with A2, B1 with B2, and A with B uncorrelated
    let cor = find(&t, "Correlation Matrix of the Parameter Estimates");
    near(nums(&cor.rows[0])[0], 1.0, "diagonal");
    near(nums(&cor.rows[0])[1], -0.497975517383285, "A1, A2");
    near(nums(&cor.rows[2])[3], -0.536648661192664, "B1, B2");
    assert!(nums(&cor.rows[0])[2].abs() < 1e-9);
}

#[test]
fn a_saturated_model_keeps_delta_in_the_cells() {
    // no DESIGN: the saturated model, with its estimates shown by default
    let t = run(T1, &format!("LOGLINEAR a b (1,3) {TIGHT}."));
    let est = find(&t, "Parameter Estimates");
    // the columns of the interaction are the products A1 B1, A1 B2, A2 B1, A2 B2
    check_estimates(
        est,
        &[
            (-0.0693246527505457, 0.166686808017789),
            (0.1207479125104711, 0.158659270233405),
            (-0.1839151657848206, 0.173322450943659),
            (0.0980118291682935, 0.158880431621407),
            (0.7267972278089431, 0.227881688613744),
            (-0.2143753960284353, 0.232990049406218),
            (-0.2656218099770113, 0.242382998401835),
            (0.3214890420934843, 0.209824400756195),
        ],
        "saturated",
    );
    // 0.5 stays in every cell: the observed counts are the counts plus a half, residuals are 0
    let cells = find(&t, "Cell Counts and Residuals");
    near(
        nums(&cells.rows[0])[nums(&cells.rows[0]).len() - 7],
        14.5,
        "observed",
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    assert!(lr.abs() < 1e-6 && pearson.abs() < 1e-6);
    assert_eq!(df, 0.0);
    // DELTA(0): the unadjusted saturated model
    let t = run(
        T1,
        "LOGLINEAR a b (1,3) /CRITERIA=DELTA(0) CONVERGE(1E-12) ITERATE(100).",
    );
    let cells = find(&t, "Cell Counts and Residuals");
    near(
        nums(&cells.rows[0])[nums(&cells.rows[0]).len() - 7],
        14.0,
        "observed",
    );
}

const LOGIT: &str = "DATA LIST FREE /resp grp n.
BEGIN DATA
1 1 20  1 2 11  1 3 14
2 1 18  2 2 6   2 3 16
END DATA.
WEIGHT BY n.
";

#[test]
fn a_logit_model_agrees_with_r_in_fit_residuals_dispersion_and_gresid() {
    let t = run(
        LOGIT,
        &format!(
            "LOGLINEAR resp(1,2) BY grp(1,3) {TIGHT} /PRINT=ESTIM\n\
             /GRESID=(2 1 0 0 0 -1) /GRESID=(3 -1 0 0 1 -2)\n\
             /DESIGN=resp, resp BY grp(1)."
        ),
    );
    check_estimates(
        find(&t, "Parameter Estimates"),
        &[
            (0.0540554572991544, 0.109322273296951),
            (0.0526896026335550, 0.122267880139963),
        ],
        "logit",
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 1.25110993024259, "likelihood ratio");
    near(pearson, 1.23273091964748, "Pearson");
    assert_eq!(df, 1.0);
    let cells = find(&t, "Cell Counts and Residuals");
    for (k, row) in cells.rows.iter().enumerate() {
        let adj = *nums(row).last().expect("adjusted");
        let want = if k % 2 == 0 {
            -1.11028416166650
        } else {
            1.11028416166650
        };
        near(adj, want, "adjusted residual");
    }
    let g = find(&t, "Generalized Residuals");
    let first = nums(&g.rows[0]);
    near(first[0], 35.0, "observed combination");
    near(first[1], 36.0204878072436, "expected combination");
    near(first[2], -1.02048780724358, "residual");
    near(first[3], -1.1102841616665, "adjusted residual");
    let second = nums(&g.rows[1]);
    near(second[0], 23.0, "observed combination");
    near(second[1], 32.1843902651923, "expected combination");
    near(second[3], -1.1102841616665, "adjusted residual");
    // dispersion: model, residual, total; then the measures of association
    let d = find(&t, "Analysis of Dispersion");
    let model = nums(&d.rows[0]);
    let resid = nums(&d.rows[1]);
    let total = nums(&d.rows[2]);
    near(model[0], 0.0929533637220672, "entropy due to model");
    near(resid[0], 58.677413233733, "entropy residual");
    near(total[0], 58.7703665974551, "entropy total");
    near(model[1], 0.0926194240108913, "concentration due to model");
    near(resid[1], 42.2603217524597, "concentration residual");
    near(total[1], 42.3529411764706, "concentration total");
    let m = find(&t, "Measures of Association");
    near(nums(&m.rows[0])[0], 0.00158163661558804, "entropy measure");
    near(
        nums(&m.rows[0])[1],
        0.00218684751136827,
        "concentration measure",
    );
}

#[test]
fn a_special_contrast_gives_a_linear_logit_model() {
    // the second row of the matrix, (1 2 4 8), is the metric of the years
    let t = run(
        "DATA LIST FREE /resp year n.
BEGIN DATA
1 1 9  1 2 12  1 3 15  1 4 22
2 1 21 2 2 17  2 3 13  2 4 8
END DATA.
WEIGHT BY n.
",
        &format!(
            "LOGLINEAR resp(1,2) BY year(1,4) {TIGHT} /PRINT=ESTIM\n\
             /CONTRAST(year)=SPECIAL(4*1, 1 2 4 8, 8*1)\n/DESIGN=resp, resp BY year(1)."
        ),
    );
    check_estimates(
        find(&t, "Parameter Estimates"),
        &[
            (-0.4780023411271673, 0.1672550355231857),
            (0.1267969184179702, 0.0379051445636488),
        ],
        "linear logit",
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 0.277177263680361, "likelihood ratio");
    near(pearson, 0.275575757898483, "Pearson");
    assert_eq!(df, 2.0);
}

const SQUARE: &str = "DATA LIST FREE /a b n.
BEGIN DATA
1 1 6  1 2 9  1 3 4
2 1 7  2 2 10 2 3 11
3 1 3  3 2 8  3 3 5
END DATA.
WEIGHT BY n.
";

#[test]
fn structural_zeros_from_a_matrix_and_a_variable_agree_with_r() {
    let want = [
        (-0.289211591225474, 0.248345229429715),
        (0.633413740202161, 0.298512063624287),
        (-0.498894884878037, 0.270003776201124),
        (0.607659975972827, 0.301449560216889),
    ];
    let t = run(
        SQUARE,
        &format!(
            "LOGLINEAR a b (1,3) {TIGHT} /PRINT=ESTIM /CWEIGHT=(0 1 1 1 0 1 1 1 0) /DESIGN=a b."
        ),
    );
    check_estimates(find(&t, "Parameter Estimates"), &want, "matrix");
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 0.0752897724707146, "likelihood ratio");
    near(pearson, 0.0756566751578336, "Pearson");
    assert_eq!(
        df, 1.0,
        "nine cells less three structural zeros less four parameters less one row"
    );
    // the structural zeros are left out of the cell table
    let cells = find(&t, "Cell Counts and Residuals");
    assert_eq!(cells.rows.len(), 6);
    let adj = [
        0.275057585166878,
        -0.275057585166877,
        -0.275057585166876,
        0.275057585166882,
        0.275057585166875,
        -0.275057585166878,
    ];
    for (row, want) in cells.rows.iter().zip(adj) {
        near(
            *nums(row).last().expect("adjusted"),
            want,
            "adjusted residual",
        );
    }
    // the same weights from a variable
    let t = run(
        &format!("{SQUARE}COMPUTE cw = 1.\nIF (a EQ b) cw = 0.\n"),
        &format!("LOGLINEAR a b (1,3) {TIGHT} /PRINT=ESTIM /CWEIGHT=cw /DESIGN=a b."),
    );
    check_estimates(find(&t, "Parameter Estimates"), &want, "variable");
}

#[test]
fn a_cell_covariate_agrees_with_r() {
    let t = run(
        "DATA LIST FREE /a b n x.
BEGIN DATA
1 1 10 1  1 2 14 3
2 1 17 2  2 2 9 5
3 1 8 9   3 2 21 4
END DATA.
WEIGHT BY n.
",
        &format!("LOGLINEAR a(1,3) b(1,2) WITH x {TIGHT} /PRINT=ESTIM /DESIGN=a b x."),
    );
    check_estimates(
        find(&t, "Parameter Estimates"),
        &[
            (-0.3676217369950325, 0.1948057120859203),
            (-0.0537103493821814, 0.1626600917673531),
            (-0.1091185642573104, 0.1179292614569988),
            (-0.1608231032640368, 0.0671127642339134),
        ],
        "covariate",
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 2.00697349365232, "likelihood ratio");
    near(pearson, 1.98872998951417, "Pearson");
    assert_eq!(df, 1.0);
}

#[test]
fn contrasts_agree_with_r() {
    let est = |contrast: &str| {
        let t = run(
            T1,
            &format!("LOGLINEAR a b (1,3) {TIGHT} /PRINT=ESTIM {contrast} /DESIGN=a b."),
        );
        estimates(find(&t, "Parameter Estimates"))
    };
    // contrasts of the margin of A are those of its deviations from the mean: A1 = -0.0833,
    // A2 = 0.0926, A3 = -A1 - A2
    let simple = est("/CONTRAST(a)=SIMPLE(1)");
    near(simple[0].0, 0.175890666463664, "simple 2 vs 1");
    near(simple[1].0, 0.0741079721537224, "simple 3 vs 1");
    near(simple[0].1, 0.265931575743964, "simple standard error");
    near(simple[1].1, 0.272352389700961, "simple standard error");
    // the B parameters keep the deviation coding
    near(simple[2].0, -0.1596923084838035, "B1");
    let default_simple = est("/CONTRAST(a)=SIMPLE");
    near(default_simple[0].0, -0.0741079721537214, "simple 1 vs last");
    let helmert = est("/CONTRAST(a)=HELMERT");
    near(helmert[0].0, -0.124999319308693, "helmert 1");
    near(helmert[1].0, 0.101782694309942, "helmert 2");
    let difference = est("/CONTRAST(a)=DIFFERENCE");
    near(difference[0].0, 0.175890666463664, "difference 1");
    near(difference[1].0, -0.0138373610781098, "difference 2");
    let repeated = est("/CONTRAST(a)=REPEATED");
    near(repeated[0].0, -0.175890666463664, "repeated 1");
    near(repeated[1].0, 0.101782694309942, "repeated 2");
    // orthonormal polynomials of B against R's contr.poly
    let poly = est("/CONTRAST(b)=POLYNOMIAL");
    near(poly[0].0, -0.0833328795391290, "A1 with polynomial B");
    near(poly[2].0, 0.1809722243843684, "linear B");
    near(poly[2].1, 0.192255989741277, "linear B standard error");
    near(poly[3].0, -0.0777115842399624, "quadratic B");
    near(poly[3].1, 0.185857501201023, "quadratic B standard error");
    // SPECIAL takes the transpose of the matrix as the basis, BASIS SPECIAL its inverse
    let special = est("/CONTRAST(a)=SPECIAL(3*1, 1 0 -1, 1 -2 1)");
    near(special[0].0, -0.0370539860768610, "special 1");
    near(special[0].1, 0.1361761948504806, "special 1 standard error");
    near(special[1].0, -0.0462788934622678, "special 2");
    near(special[1].1, 0.0751310119133276, "special 2 standard error");
    let basis = est("/CONTRAST(a)=BASIS SPECIAL(3*1, 1 0 -1, 1 -2 1)");
    near(basis[0].0, -0.0741079721537214, "basis special 1");
    near(
        basis[0].1,
        0.272352389700961,
        "basis special 1 standard error",
    );
    near(basis[1].0, -0.2776733607736062, "basis special 2");
    near(
        basis[1].1,
        0.450786071479965,
        "basis special 2 standard error",
    );
}

#[test]
fn settings_carry_to_later_designs_and_the_last_design_is_added() {
    let t = run(
        T1,
        &format!(
            "LOGLINEAR a b (1,3) {TIGHT} /PRINT=ESTIM /DESIGN=a b /PRINT=NONE /DESIGN=a\n\
             /PRINT=DEFAULT /CRITERIA=DELTA(0.25)."
        ),
    );
    // three models: the two designs and the saturated one the command adds
    assert_eq!(all(&t, "Goodness-of-Fit Tests").len(), 3);
    // ESTIM was asked for the first design only; the added saturated model shows its own
    assert_eq!(all(&t, "Parameter Estimates").len(), 2);
    // PRINT=NONE leaves the goodness of fit and no cell table for the second design
    assert_eq!(all(&t, "Cell Counts and Residuals").len(), 2);
    let fits = all(&t, "Goodness-of-Fit Tests");
    let (lr_a, _, df_a) = fit_of(fits[1]);
    // main effect of A alone: the fit is the margin of A over a flat B (R: 31.08, df 6)
    assert_eq!(df_a, 6.0);
    assert!(lr_a > 15.0);
    let (_, _, df_s) = fit_of(fits[2]);
    assert_eq!(df_s, 0.0);
    assert!(
        all(&t, "Convergence Information")[0]
            .caption
            .as_deref()
            .is_some_and(|c| c.contains("Design 1"))
    );
}

#[test]
fn a_sampling_zero_takes_a_degree_of_freedom_as_the_manual_states() {
    // one empty cell in an independence model: the fit is that of R (likelihood ratio
    // 21.2145262589318, Pearson 16.607165012407), and df = I (J - 1) - p - E counts the empty
    // cell in E, so it is 3 where a count of cells less parameters would give 4
    let t = run(
        "DATA LIST FREE /a b n.
BEGIN DATA
1 1 14  1 2 7  1 3 5
2 1 6   2 2 0  2 3 10
3 1 4   3 2 8  3 3 16
END DATA.
WEIGHT BY n.
",
        &format!("LOGLINEAR a b (1,3) {TIGHT} /DESIGN=a b."),
    );
    let (lr, pearson, df) = fit_of(find(&t, "Goodness-of-Fit Tests"));
    near(lr, 21.2145262589318, "likelihood ratio");
    near(pearson, 16.607165012407, "Pearson");
    assert_eq!(df, 3.0);
}

#[test]
fn cases_out_of_range_and_missing_are_counted_and_left_out() {
    let t = run(
        "DATA LIST FREE /a b.
BEGIN DATA
1 1  1 2  2 1  2 2  2 2  3 1  1 .  9 1  2.7 1
END DATA.
MISSING VALUES a (3).
",
        "LOGLINEAR a (1,2) b (1,2) /DESIGN=a b.",
    );
    let info = find(&t, "Data Information");
    let n = |label: &str| {
        let row = info
            .rows
            .iter()
            .find(|r| matches!(&r[1], Cell::Text(s) if s == label))
            .expect("row");
        nums(row)[0]
    };
    // 2.7 is cut to 2 and counts; 9 is out of range; the user-missing 3 and the missing b go
    assert_eq!(n("Valid"), 6.0);
    assert_eq!(n("Out of range"), 1.0);
    assert_eq!(n("Missing"), 2.0);
    // INCLUDE keeps the user-missing 3 only when the range holds it
    let t = run(
        "DATA LIST FREE /a b.
BEGIN DATA
1 1  1 2  2 1  2 2  3 1  3 2
END DATA.
MISSING VALUES a (3).
",
        "LOGLINEAR a (1,3) b (1,2) /MISSING=INCLUDE /DESIGN=a b.",
    );
    let info = find(&t, "Data Information");
    assert_eq!(nums(&info.rows[0])[0], 6.0);
}

#[test]
fn plots_and_the_design_matrix_are_produced() {
    let (commands, errors) = parse(&format!(
        "{T1}LOGLINEAR a b (1,3) /PRINT=DESIGN /PLOT=RESID NORMPROB /DESIGN=a b."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    // RESID: two scatter plots; NORMPROB: the normal and the detrended normal plot
    assert_eq!(engine.take_charts().len(), 4);
    let d = find(&tables, "Design Matrix");
    // nine cells, the cell values of the two factors and four parameters
    assert_eq!(d.rows.len(), 9);
    assert_eq!(nums(&d.rows[0]).len(), 4);
    // the first cell (a=1, b=1) has the deviation coding 1 0 for A and 1 0 for B
    assert_eq!(nums(&d.rows[0]), [1.0, 0.0, 1.0, 0.0]);
    // the last cell (3, 3) has the reference coding -1 -1 for both
    assert_eq!(nums(&d.rows[8]), [-1.0, -1.0, -1.0, -1.0]);
}

#[test]
fn misuse_is_refused() {
    assert!(
        fails(T1, "LOGLINEAR a b (1,3) /DESIGN=a c.")
            .contains("neither a factor nor a cell covariate")
    );
    assert!(
        fails(T1, "LOGLINEAR a b (1,3) /CWEIGHT=(1 1 1) /DESIGN=a b.").contains("has 3 values")
    );
    assert!(
        fails(
            T1,
            "LOGLINEAR a b (1,3) /CONTRAST(a)=SPECIAL(1 2 3 4) /DESIGN=a b."
        )
        .contains("need 9")
    );
    assert!(
        fails(T1, "LOGLINEAR a b (1,3) /CONTRAST(n)=SIMPLE /DESIGN=a b.").contains("not a factor")
    );
    assert!(
        fails(T1, "LOGLINEAR a b (1,3) /DESIGN=a(3).").contains("beyond the 2 contrast columns")
    );
    assert!(
        fails(T1, "LOGLINEAR a (1,3) b (1,3) WITH a /DESIGN=a b.").contains("more than once")
            || fails(T1, "LOGLINEAR a (1,3) b (1,3) WITH a /DESIGN=a b.")
                .contains("cannot also be")
    );
}
