//! GLM /RANDOM: the effects named (and every effect that contains one of them and no
//! covariate) are random; the model is Y = X0 b0 + sum Xi bi + e with bi ~ N(0, s_i^2 I) and
//! e ~ N(0, s_e^2 W^-1), as the Algorithms manual has it.
//!
//! Rules, in our words:
//! - For the estimable function L of an effect (the rows its sum of squares type uses), the
//!   expected hypothesis sum of squares is the quadratic form of the fixed parameters, plus
//!   for each random effect i the variance s_i^2 times trace(Xi' W X G L' (L G L')^-1 L G X' W Xi),
//!   plus s_e^2 times the rank s of L. Divided by s it gives the expected mean square, whose
//!   coefficient of s_e^2 is 1; the quadratic term is reported by the fixed effects that L
//!   touches.
//! - The error term of an effect is the combination q1 MS1 + ... + qk MSk + q(k+1) MSE of the
//!   mean squares of the random effects and the residual whose expected value equals that of
//!   the effect with its own quadratic term set to 0 (and, for a random effect, its own
//!   variance too). The q solve the matching of the variance coefficients, by least squares
//!   when no exact solution exists; a random effect without degrees of freedom has q = 0.
//! - The degrees of freedom of the error term are Satterthwaite's: its mean square squared
//!   over the sum of (q_i MS_i)^2 / s_i.

use crate::dist::f_p;
use crate::glm::{Data, Fit, Part, effect_sscp, ginverse, roman};
use crate::matrix::{Matrix, multiply, transpose};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Error, Table};
use statscraft_syntax::Glm;

/// Whether each effect of the design is random under RANDOM: it contains a named factor and no
/// covariate.
fn random_flags(command: &Glm, d: &Data, fit: &Fit) -> Result<Vec<bool>, Error> {
    let mut named = Vec::new();
    for name in &command.random {
        let Some(f) = d.factors.iter().position(|n| n.eq_ignore_ascii_case(name)) else {
            return Err(Error(format!(
                "RANDOM names {name}, which is not a factor of the model (the intercept and the covariates cannot be random)"
            )));
        };
        named.push(f);
    }
    Ok(fit
        .effects
        .iter()
        .map(|e| {
            e.covariates.is_empty()
                && e.parts
                    .iter()
                    .any(|p| matches!(p, Part::Factor(f) if named.contains(f)))
        })
        .collect())
}

/// What is known of one effect: its rows of L, degrees of freedom, mean square and expected
/// mean square.
struct Term {
    at: usize,
    ms: f64,
    df: f64,
    /// The coefficient of each random effect's variance in the expected mean square.
    coefficients: Vec<f64>,
    /// The fixed effects the estimable function touches.
    quadratic: Vec<usize>,
}

/// The tests of the effects against their error terms and the Expected Mean Squares table, for
/// one dependent variable.
pub(crate) fn random_tables(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    dependent: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Table>, Error> {
    if !command.wsfactors.is_empty() {
        return Err(Error(
            "RANDOM cannot be used with WSFACTOR (a model with a within-subjects factor)".into(),
        ));
    }
    let random = random_flags(command, d, fit)?;
    let sscp = effect_sscp(fit, command.sstype);
    let g = &fit.ginv;
    let p = fit.p;
    let dfe = fit.n - fit.rank as f64;
    let mse = fit.s[0][0] / dfe;
    let randoms: Vec<usize> = (0..fit.effects.len()).filter(|&e| random[e]).collect();
    let mut terms: Vec<Term> = Vec::new();
    for (e, (ss_matrix, rank_df)) in sscp.iter().enumerate() {
        let l = match command.sstype {
            1 => fit.type1(e),
            2 => fit.type2(e),
            t => fit.type34(e, t == 4),
        };
        if l.is_empty() {
            continue;
        }
        let (ss, df) = (&ss_matrix[0][0], *rank_df as f64);
        if df <= 0.0 {
            continue;
        }
        // M = G L' (L G L')^- L G
        let lt = transpose(&l);
        let lgl = multiply(&multiply(&l, g), &lt);
        let (inverse, rank) = ginverse(&lgl, 1e-10);
        let glt = multiply(g, &lt);
        let m = multiply(&multiply(&glt, &inverse), &transpose(&glt));
        // the coefficient of each random effect: the sum over its columns of a'Ma with a the
        // column of X'WX (Xi' W X as rows)
        let coefficients: Vec<f64> = randoms
            .iter()
            .map(|&k| {
                let mut total = 0.0;
                for c in fit.effects[k].range() {
                    let a: Vec<f64> = (0..p).map(|r| fit.zwz[r][c]).collect();
                    for r in 0..p {
                        for s in 0..p {
                            total += a[r] * m[r][s] * a[s];
                        }
                    }
                }
                total / rank.max(1) as f64
            })
            .collect();
        let quadratic: Vec<usize> = (0..fit.effects.len())
            .filter(|&f| !random[f])
            .filter(|&f| {
                fit.effects[f]
                    .range()
                    .any(|c| l.iter().any(|row| row[c].abs() > 1e-8))
            })
            .collect();
        terms.push(Term {
            at: e,
            ms: ss / df,
            df,
            coefficients,
            quadratic,
        });
    }
    // the variance components: the random effects with degrees of freedom, then the residual
    let usable: Vec<usize> = randoms
        .iter()
        .enumerate()
        .filter(|(_, k)| terms.iter().any(|t| t.at == **k))
        .map(|(i, _)| i)
        .collect();
    let names: Vec<String> = usable
        .iter()
        .map(|&i| format!("Var({})", fit.effects[randoms[i]].label))
        .chain(std::iter::once("Var(Error)".to_string()))
        .collect();
    let m = names.len();
    // E[v][i]: the coefficient of component v in the expected mean square of component i's term
    let mut expected: Matrix = vec![vec![0.0; m]; m];
    for (col, &i) in usable.iter().enumerate() {
        let t = terms.iter().find(|t| t.at == randoms[i]).expect("usable");
        for (row, &j) in usable.iter().enumerate() {
            expected[row][col] = t.coefficients[j];
        }
        expected[m - 1][col] = 1.0;
    }
    expected[m - 1][m - 1] = 1.0;
    let mean_square_of = |col: usize| -> (f64, f64) {
        if col + 1 == m {
            (mse, dfe)
        } else {
            let t = terms
                .iter()
                .find(|t| t.at == randoms[usable[col]])
                .expect("usable");
            (t.ms, t.df)
        }
    };
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    let mut ems_rows = Vec::new();
    for t in &terms {
        let own = usable.iter().position(|&i| randoms[i] == t.at);
        // the coefficients under the null hypothesis: the effect's own variance goes
        let mut target: Vec<f64> = usable.iter().map(|&j| t.coefficients[j]).collect();
        target.push(1.0);
        if let Some(own) = own {
            target[own] = 0.0;
        }
        // least squares: q = (E'E)^- E' target
        let et = transpose(&expected);
        let (inverse, _) = ginverse(&multiply(&et, &expected), 1e-12);
        let rhs: Vec<f64> = (0..m)
            .map(|i| (0..m).map(|v| et[i][v] * target[v]).sum())
            .collect();
        let q: Vec<f64> = (0..m)
            .map(|i| (0..m).map(|j| inverse[i][j] * rhs[j]).sum())
            .collect();
        let fitted: Vec<f64> = (0..m)
            .map(|v| (0..m).map(|i| expected[v][i] * q[i]).sum())
            .collect();
        let scale = target.iter().fold(1.0_f64, |a, b| a.max(b.abs()));
        let label = &fit.effects[t.at].label;
        if fitted
            .iter()
            .zip(&target)
            .any(|(a, b)| (a - b).abs() > 1e-7 * scale)
        {
            warnings.push(format!(
                "GLM RANDOM: no combination of the mean squares of the random effects has the expected mean square of {label} under its null hypothesis; the nearest one is used"
            ));
        }
        let mut ms_error = 0.0;
        let mut spread = 0.0;
        let mut pieces = Vec::new();
        for (col, &coefficient) in q.iter().enumerate() {
            if coefficient.abs() < 1e-10 {
                continue;
            }
            let (ms, df) = mean_square_of(col);
            ms_error += coefficient * ms;
            if df > 0.0 {
                spread += (coefficient * ms).powi(2) / df;
            }
            let term = if col + 1 == m {
                "MS(Error)".to_string()
            } else {
                format!("MS({})", fit.effects[randoms[usable[col]]].label)
            };
            pieces.push(format!("{}*{term}", trim(coefficient)));
        }
        let df_error = if spread > 0.0 {
            ms_error * ms_error / spread
        } else {
            f64::NAN
        };
        let f = (ms_error > 0.0).then(|| t.ms / ms_error);
        let sig = f.filter(|_| df_error > 0.0).map(|f| f_p(f, t.df, df_error));
        if ms_error <= 0.0 {
            warnings.push(format!(
                "GLM RANDOM: the error term of {label} is not a positive mean square; its F is left out"
            ));
        }
        rows.push(vec![
            text(label.clone()),
            text("Hypothesis"),
            Cell::Number(t.ms * t.df),
            Cell::Number(t.df),
            Cell::Number(t.ms),
            f.map_or(Cell::Empty, Cell::Number),
            sig.map_or(Cell::Empty, Cell::Number),
        ]);
        rows.push(vec![
            Cell::Empty,
            text("Error"),
            Cell::Number(ms_error * df_error),
            Cell::Number(df_error),
            Cell::Number(ms_error),
            Cell::Empty,
            Cell::Empty,
        ]);
        notes.push(format!(
            "{label}: {}",
            pieces.join(" + ").replace("+ -", "- ")
        ));
        // the expected mean square row
        let mut line = vec![text(label.clone())];
        for &i in &usable {
            // rounding residue of the sweeps is not a coefficient
            let c = t.coefficients[i];
            line.push(Cell::Number(if c.abs() < 1e-9 { 0.0 } else { c }));
        }
        line.push(Cell::Number(1.0));
        let quadratic = if t.quadratic.is_empty() {
            Cell::Empty
        } else {
            text(
                t.quadratic
                    .iter()
                    .map(|&f| fit.effects[f].label.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        line.push(quadratic);
        ems_rows.push(line);
    }
    let mut error_line = vec![text("Error")];
    error_line.extend(std::iter::repeat_n(Cell::Number(0.0), m - 1));
    error_line.push(Cell::Number(1.0));
    error_line.push(Cell::Empty);
    ems_rows.push(error_line);
    let mut columns = plain(&["Source"]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    columns.push((String::new(), "Quadratic Term".into()));
    let kind = format!("Type {}", roman(command.sstype));
    let mut tests = table(
        "Tests of Between-Subjects Effects",
        2,
        {
            let mut c = plain(&["Source", ""]);
            c.extend(plain(&[
                &format!("{kind} Sum of Squares"),
                "df",
                "Mean Square",
                "F",
                "Sig.",
            ]));
            c
        },
        rows,
        Some(format!(
            "Dependent Variable: {dependent}. Random effects: {}. The error term of each effect is the combination of mean squares that has its expected mean square under the null hypothesis; the degrees of freedom of the Error row are Satterthwaite's. Error terms: {}.",
            randoms
                .iter()
                .map(|&k| fit.effects[k].label.clone())
                .collect::<Vec<_>>()
                .join(", "),
            notes.join("; ")
        )),
    );
    tests.row_headers = 2;
    let ems = table(
        "Expected Mean Squares",
        1,
        columns,
        ems_rows,
        Some(format!(
            "Dependent Variable: {dependent}. {kind} sums of squares. Each row gives the coefficient of the variance of every random effect and of the error in the expected mean square of its effect; the Quadratic Term names the fixed effects that enter it."
        )),
    );
    Ok(vec![tests, ems])
}

/// A coefficient written briefly: 1, 0.5, 0.3333.
fn trim(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}
