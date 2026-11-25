//! CROSSTABS statistics against R, on a 4 x 3 ordinal table (n = 100), a 2 x 2 table (n = 70),
//! a 3 x 3 square table and three 2 x 2 strata. The values come from R written out per
//! statistic: chisq.test, fisher.test, binom.test and mantelhaen.test where R has them, and
//! the Algorithms manual's formulas typed in for the others (the standard errors of the
//! ordinal measures, lambda, Goodman and Kruskal's tau, the uncertainty coefficient, kappa,
//! Breslow-Day and Tarone). The standard errors of gamma, tau-b, tau-c, Somers' d, lambda,
//! the uncertainty coefficient, Pearson's r and kappa also equal the delta method on the
//! cell counts, a check made in R by numerical derivatives. The manual's own formulas differ
//! from the delta method in the symmetric Somers' d, Spearman's r and Goodman and Kruskal's
//! tau, and are followed.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// A 4 x 3 table of counts, row by row: 14 8 3 / 9 12 6 / 4 11 13 / 2 6 12.
const ORDINAL: &str = "DATA LIST FREE /r c w.
BEGIN DATA
1 1 14 1 2 8 1 3 3  2 1 9 2 2 12 2 3 6
3 1 4 3 2 11 3 3 13  4 1 2 4 2 6 4 3 12
END DATA.
WEIGHT BY w.
";

/// 21 9 / 12 28.
const TWO: &str = "DATA LIST FREE /x y w.
BEGIN DATA
1 1 21 1 2 9 2 1 12 2 2 28
END DATA.
WEIGHT BY w.
";

/// 18 5 2 / 4 15 6 / 1 7 12.
const SQUARE: &str = "DATA LIST FREE /k1 k2 w.
BEGIN DATA
1 1 18 1 2 5 1 3 2  2 1 4 2 2 15 2 3 6  3 1 1 3 2 7 3 3 12
END DATA.
WEIGHT BY w.
";

/// Three 2 x 2 tables: (10 6 / 4 12), (7 5 / 9 11), (12 8 / 3 6).
const STRATA: &str = "DATA LIST FREE /x y z w.
BEGIN DATA
1 1 1 10 1 2 1 6 2 1 1 4 2 2 1 12
1 1 2 7 1 2 2 5 2 1 2 9 2 2 2 11
1 1 3 12 1 2 3 8 2 1 3 3 2 2 3 6
END DATA.
WEIGHT BY w.
";

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

/// The number in `column` of the row of the table whose title starts with `title` and whose
/// cells include every one of `labels`.
fn get(tables: &[Table], title: &str, labels: &[&str], column: &str) -> f64 {
    let t = tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"));
    // a label that a row leaves empty takes the one above it (the first rows of a block)
    let mut carried: Vec<String> = Vec::new();
    let at = t
        .column_titles()
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()));
    for row in &t.rows {
        for (k, cell) in row.iter().take(t.row_headers.max(3)).enumerate() {
            if let Cell::Text(s) = cell
                && !s.is_empty()
            {
                if k >= carried.len() {
                    carried.resize(k + 1, String::new());
                }
                carried[k] = s.clone();
                carried.truncate(k + 1);
            }
        }
        let own: Vec<&str> = row
            .iter()
            .filter_map(|c| match c {
                Cell::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        let all: Vec<&str> = carried.iter().map(String::as_str).chain(own).collect();
        if labels.iter().all(|l| all.contains(l))
            && let Cell::Number(n) = row[at]
        {
            return n;
        }
    }
    panic!("no row {labels:?} with a number in {column} of {title}");
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

#[test]
fn ordinal_table_agrees_with_r() {
    let t = run(ORDINAL, "CROSSTABS /TABLES=r BY c /STATISTICS=ALL.");
    let chi = "Chi-Square Tests";
    near(
        get(&t, chi, &["Pearson Chi-Square"], "Value"),
        21.8261073302,
        "chi",
    );
    near(
        get(
            &t,
            chi,
            &["Pearson Chi-Square"],
            "Asymptotic Sig. (2-tailed)",
        ),
        0.001301926899,
        "p",
    );
    near(
        get(&t, chi, &["Likelihood Ratio"], "Value"),
        22.2681677074,
        "G2",
    );
    near(
        get(&t, chi, &["Linear-by-Linear Association"], "Value"),
        19.7383032711,
        "LBL",
    );
    let sym = "Symmetric Measures";
    near(get(&t, sym, &["Phi"], "Value"), 0.4671841963, "phi");
    near(get(&t, sym, &["Cramer's V"], "Value"), 0.3303491133, "V");
    near(
        get(&t, sym, &["Contingency Coefficient"], "Value"),
        0.4232704593,
        "CC",
    );
    // value, standard error and T of the ordinal and interval measures
    for (label, value, ase, t_value) in [
        ("Kendall's tau-b", 0.3928463141, 0.0748315124, 5.2162071501),
        ("Kendall's tau-c", 0.4146, 0.0794830397, 5.2162071501),
        ("Gamma", 0.5402658327, 0.0956019203, 5.2162071501),
        (
            "Spearman Correlation",
            0.4480121027,
            0.0836941102,
            4.9608021938,
        ),
        ("Pearson's R", 0.4465162939, 0.0836443290, 4.9401059594),
    ] {
        near(get(&t, sym, &[label], "Value"), value, label);
        near(
            get(&t, sym, &[label], "Asymptotic Standard Error"),
            ase,
            label,
        );
        near(get(&t, sym, &[label], "Approximate T"), t_value, label);
    }
    // the table is 4 x 3, so kappa is taken over the values 1 to 4 of both variables
    near(get(&t, sym, &["Kappa"], "Value"), 0.1671217914, "kappa");
    near(
        get(&t, sym, &["Kappa"], "Asymptotic Standard Error"),
        0.0625809010,
        "kappa ASE",
    );
    near(
        get(&t, sym, &["Kappa"], "Approximate T"),
        2.9085225247,
        "kappa T",
    );
    let dir = "Directional Measures";
    for (label, who, value, ase, t_value) in [
        (
            "Lambda",
            "Symmetric",
            0.1851851852,
            0.0846399343,
            2.0498001542,
        ),
        (
            "Lambda",
            "r Dependent",
            0.1527777778,
            0.0818574292,
            1.7438363073,
        ),
        (
            "Lambda",
            "c Dependent",
            0.2222222222,
            0.1119894735,
            1.7774285064,
        ),
        (
            "Uncertainty Coefficient",
            "Symmetric",
            0.0900792616,
            0.0357356191,
            2.5138226191,
        ),
        (
            "Uncertainty Coefficient",
            "r Dependent",
            0.0807747424,
            0.0320647415,
            2.5138226191,
        ),
        (
            "Uncertainty Coefficient",
            "c Dependent",
            0.1018064423,
            0.0403674087,
            2.5138226191,
        ),
        (
            "Somers' d",
            "Symmetric",
            0.3921679909,
            0.0747023017,
            5.2162071501,
        ),
        (
            "Somers' d",
            "r Dependent",
            0.4166415436,
            0.0792359977,
            5.2162071501,
        ),
        (
            "Somers' d",
            "c Dependent",
            0.3704100777,
            0.0707475709,
            5.2162071501,
        ),
    ] {
        let what = format!("{label} {who}");
        near(get(&t, dir, &[label, who], "Value"), value, &what);
        near(
            get(&t, dir, &[label, who], "Asymptotic Standard Error"),
            ase,
            &what,
        );
        near(get(&t, dir, &[label, who], "Approximate T"), t_value, &what);
    }
    // Goodman and Kruskal's tau: the manual's standard error, the chi-square significance
    for (who, value, ase, sig) in [
        ("r Dependent", 0.0704322694, 0.0295863420, 0.00189759450824),
        ("c Dependent", 0.1039378670, 0.0408663333, 0.00218230614979),
    ] {
        let label = "Goodman and Kruskal tau";
        near(get(&t, dir, &[label, who], "Value"), value, who);
        near(
            get(&t, dir, &[label, who], "Asymptotic Standard Error"),
            ase,
            who,
        );
        near(
            get(&t, dir, &[label, who], "Approximate Significance"),
            sig,
            who,
        );
    }
    near(
        get(&t, dir, &["Eta", "r Dependent"], "Value"),
        0.4465341199,
        "eta r",
    );
    near(
        get(&t, dir, &["Eta", "c Dependent"], "Value"),
        0.4519314985,
        "eta c",
    );
}

#[test]
fn two_by_two_table_agrees_with_r() {
    let t = run(
        TWO,
        "CROSSTABS /TABLES=x BY y /STATISTICS=CHISQ PHI RISK MCNEMAR GAMMA D CORR.",
    );
    let chi = "Chi-Square Tests";
    near(
        get(&t, chi, &["Pearson Chi-Square"], "Value"),
        11.0073710074,
        "chi",
    );
    near(
        get(&t, chi, &["Likelihood Ratio"], "Value"),
        11.2909070068,
        "G2",
    );
    near(
        get(&t, chi, &["Continuity Correction"], "Value"),
        9.4606538357,
        "continuity",
    );
    near(
        get(
            &t,
            chi,
            &["Continuity Correction"],
            "Asymptotic Sig. (2-tailed)",
        ),
        0.002099262307,
        "continuity p",
    );
    near(
        get(&t, chi, &["Fisher's Exact Test"], "Exact Sig. (2-sided)"),
        0.001502746971,
        "fisher",
    );
    near(
        get(&t, chi, &["Fisher's Exact Test"], "Exact Sig. (1-sided)"),
        0.0009514677798,
        "fisher one-sided",
    );
    near(
        get(&t, chi, &["McNemar Test"], "Exact Sig. (2-sided)"),
        0.6636238098,
        "mcnemar",
    );
    near(
        get(&t, chi, &["Linear-by-Linear Association"], "Value"),
        10.8501228501,
        "LBL",
    );
    let risk = "Risk Estimate";
    near(
        get(&t, risk, &["Odds Ratio for x (1 / 2)"], "Value"),
        5.4444444444,
        "OR",
    );
    near(
        get(
            &t,
            risk,
            &["Odds Ratio for x (1 / 2)"],
            "95% Confidence Interval / Lower",
        ),
        1.9378982321,
        "OR lower",
    );
    near(
        get(
            &t,
            risk,
            &["Odds Ratio for x (1 / 2)"],
            "95% Confidence Interval / Upper",
        ),
        15.2959401159,
        "OR upper",
    );
    near(
        get(&t, risk, &["For cohort y = 1"], "Value"),
        2.3333333333,
        "RR1",
    );
    near(
        get(
            &t,
            risk,
            &["For cohort y = 1"],
            "95% Confidence Interval / Lower",
        ),
        1.3759278497,
        "RR1 lower",
    );
    near(
        get(&t, risk, &["For cohort y = 2"], "Value"),
        0.4285714286,
        "RR2",
    );
    near(
        get(
            &t,
            risk,
            &["For cohort y = 2"],
            "95% Confidence Interval / Upper",
        ),
        0.7677774949,
        "RR2 upper",
    );
    let sym = "Symmetric Measures";
    near(get(&t, sym, &["Gamma"], "Value"), 0.6896551724, "gamma");
    near(
        get(&t, sym, &["Gamma"], "Asymptotic Standard Error"),
        0.1381851415,
        "gamma ASE",
    );
    near(
        get(&t, sym, &["Gamma"], "Approximate T"),
        3.5862576621,
        "gamma T",
    );
}

#[test]
fn square_table_kappa_and_bowker_agree_with_r() {
    let t = run(
        SQUARE,
        "CROSSTABS /TABLES=k1 BY k2 /STATISTICS=KAPPA MCNEMAR.",
    );
    let sym = "Symmetric Measures";
    near(get(&t, sym, &["Kappa"], "Value"), 0.4615384615, "kappa");
    near(
        get(&t, sym, &["Kappa"], "Asymptotic Standard Error"),
        0.0864222038,
        "kappa ASE",
    );
    near(
        get(&t, sym, &["Kappa"], "Approximate T"),
        5.4520999605,
        "kappa T",
    );
    let chi = "Chi-Square Tests";
    near(
        get(&t, chi, &["McNemar-Bowker Test"], "Value"),
        0.5213675214,
        "Bowker",
    );
    near(
        get(
            &t,
            chi,
            &["McNemar-Bowker Test"],
            "Asymptotic Sig. (2-tailed)",
        ),
        0.9141725991,
        "Bowker p",
    );
}

#[test]
fn layers_agree_with_r_on_the_homogeneity_and_the_common_odds_ratio() {
    let t = run(STRATA, "CROSSTABS /TABLES=x BY y BY z /STATISTICS=CMH(1).");
    let hom = "Tests of Homogeneity";
    near(
        get(&t, hom, &["Breslow-Day"], "Chi-Squared"),
        1.0140050794,
        "Breslow-Day",
    );
    near(
        get(&t, hom, &["Tarone's"], "Chi-Squared"),
        1.0130559365,
        "Tarone",
    );
    let cond = "Tests of Conditional Independence";
    near(
        get(&t, cond, &["Cochran's"], "Chi-Squared"),
        5.9293983271,
        "Cochran",
    );
    // R's mantelhaen.test with the continuity correction
    near(
        get(&t, cond, &["Mantel-Haenszel"], "Chi-Squared"),
        4.7686932612,
        "Mantel-Haenszel",
    );
    near(
        get(
            &t,
            cond,
            &["Mantel-Haenszel"],
            "Asymptotic Significance (2-sided)",
        ),
        0.02898181538,
        "Mantel-Haenszel p",
    );
    let or = "Mantel-Haenszel Common Odds Ratio";
    near(
        get(&t, or, &["Estimate"], "Value"),
        2.8952690502,
        "common OR",
    );
    near(
        get(&t, or, &["Common Odds Ratio Lower Bound"], "Value"),
        1.2084502845,
        "lower",
    );
    near(
        get(&t, or, &["Common Odds Ratio Upper Bound"], "Value"),
        6.9366385864,
        "upper",
    );
}

/// Cases of a 2 x 2 table with fractional weights, two cases in three of the cells, so that
/// rounding each case (CASE) and rounding the sum of a cell (CELL) differ.
const FRACTIONAL: &str = "DATA LIST FREE /x y w.
BEGIN DATA
1 1 2.4  1 1 2.4  1 2 3.6  1 2 3.6  2 1 5.5  2 2 4.4  2 2 4.4
END DATA.
WEIGHT BY w.
";

#[test]
fn count_keywords_agree_with_r_on_fractional_weights() {
    // R: chisq.test(correct = FALSE) on the table the keyword makes of the weights
    for (count, pearson, likelihood, n) in [
        ("ASIS", 0.00648170276326, 0.00647938625557, 26.3),
        ("CASE ROUND", 0.247619047619, 0.248712034874, 26.0),
        ("CELL ROUND", 0.00767045454545, 0.00766725692231, 27.0),
        ("CASE TRUNCATE", 0.00561660561661, 0.00561346849499, 23.0),
        ("CELL TRUNCATE", 0.0111888111888, 0.0111969029709, 24.0),
    ] {
        let t = run(
            FRACTIONAL,
            &format!("CROSSTABS /TABLES=x BY y /COUNT {count} /STATISTICS=CHISQ."),
        );
        let chi = "Chi-Square Tests";
        near(
            get(&t, chi, &["Pearson Chi-Square"], "Value"),
            pearson,
            count,
        );
        near(
            get(&t, chi, &["Likelihood Ratio"], "Value"),
            likelihood,
            count,
        );
        near(get(&t, chi, &["N of Valid Cases"], "Value"), n, count);
    }
}

/// 12 0 / 5 9: an empty cell, with the chi-square statistics and the exact test.
#[test]
fn table_with_an_empty_cell_agrees_with_r() {
    let t = run(
        "DATA LIST FREE /x y w.
BEGIN DATA
1 1 12  1 2 0  2 1 5  2 2 9
END DATA.
WEIGHT BY w.
",
        "CROSSTABS /TABLES=x BY y /STATISTICS=CHISQ.",
    );
    let chi = "Chi-Square Tests";
    near(
        get(&t, chi, &["Pearson Chi-Square"], "Value"),
        11.7983193277,
        "chi",
    );
    near(
        get(&t, chi, &["Continuity Correction"], "Value"),
        9.12892934952,
        "continuity",
    );
    near(
        get(&t, chi, &["Fisher's Exact Test"], "Exact Sig. (2-sided)"),
        0.000711142404506,
        "fisher",
    );
    near(
        get(&t, chi, &["Fisher's Exact Test"], "Exact Sig. (1-sided)"),
        0.000640732265446,
        "fisher one-sided",
    );
}
