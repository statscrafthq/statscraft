//! Frequency weights (WEIGHT BY) against R on 28 invented cases weighing 73 in all: R is run on
//! the cases written out as often as their weight, and every procedure must give the same
//! numbers with the weight as with the repeated rows (N is the sum of the weights, the degrees
//! of freedom follow from it): T-TEST (pooled, separate variances, Levene and paired), ONEWAY with
//! Tukey's comparisons, CORRELATIONS, NONPAR CORR, REGRESSION and the CROSSTABS chi-square tests.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g w x y z a.
BEGIN DATA
1 4 12.8 9.4 12 13.3
1 2 9.1 5.5 5.2 9.2
1 1 10.9 9.2 7.3 13.2
1 1 7.9 7.7 5.9 6.7
1 4 12.1 8.1 7.1 12.5
1 2 14.9 9.4 5.9 15.2
1 4 9.3 9.8 6.3 13.3
1 4 12.7 10.2 9.1 10.9
1 3 10.9 8.9 6.7 11.9
2 2 10.2 5.5 8.4 12
2 1 21.5 15.1 8.2 22.2
2 3 10 6.9 3.9 11.3
2 1 14.7 8.6 7.7 14
2 2 11.5 10.8 10.9 16.5
2 4 11.5 12.3 6.3 13.6
2 1 11.6 10.9 8.7 17.7
2 3 7.5 8.6 6.2 6.4
2 3 12.6 9.5 6.8 14.9
2 4 15.3 11.3 9.7 11.9
3 3 11.9 10.9 8.9 14.8
3 1 14.4 13 6.5 16.2
3 4 19.6 14.9 7.3 19.2
3 3 15.1 11.7 10.2 13.9
3 4 15.6 11.3 8.5 14.7
3 4 13.7 9.9 9.9 15.6
3 2 19.3 19 10.1 15.3
3 1 17.7 13.1 9.2 18.4
3 2 15.7 12.4 13.3 18.9
END DATA.
WEIGHT BY w.
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

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(s) => s,
        _ => "",
    }
}

/// The number in `column` of the row of table `title` whose leading cells, with the empty ones
/// taking the label above them, are `key`.
fn at(tables: &[Table], title: &str, key: &[&str], column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let titles = table.column_titles();
    let at = titles
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {titles:?}"));
    let mut carried = vec![String::new(); key.len()];
    for row in &table.rows {
        for (i, label) in carried.iter_mut().enumerate() {
            let cell = text(&row[i]);
            if !cell.is_empty() {
                *label = cell.to_string();
            } else if let Cell::Number(n) = row[i] {
                *label = format!("{n}");
            }
        }
        if carried == key {
            return match row[at] {
                Cell::Number(n) => n,
                ref other => panic!("{key:?}: {other:?}"),
            };
        }
    }
    panic!("no row {key:?} in {title}");
}

#[test]
fn weighted_t_tests_agree_with_r_on_the_repeated_cases() {
    let t = run("T-TEST GROUPS=g(1 2) /VARIABLES=x.\nT-TEST PAIRS=x WITH a (PAIRED).");
    let title = "Independent Samples Test";
    let equal = ["x", "Equal variances assumed"];
    near(at(&t, title, &equal, "F"), 2.21351154788, "Levene F");
    near(at(&t, title, &equal, "Sig."), 0.143485969532, "Levene p");
    near(at(&t, title, &equal, "t"), -0.748374581182, "t");
    near(at(&t, title, &equal, "df"), 47.0, "df");
    near(
        at(&t, title, &equal, "Sig. (2-tailed)"),
        0.457962994786,
        "p",
    );
    near(
        at(&t, title, &equal, "Std. Error Difference"),
        0.728467642241,
        "se",
    );
    near(at(&t, title, &equal, "Lower"), -2.0106545355, "lower");
    near(at(&t, title, &equal, "Upper"), 0.92032120217, "upper");
    let unequal = ["x", "Equal variances not assumed"];
    near(at(&t, title, &unequal, "t"), -0.740965029247, "Welch t");
    near(at(&t, title, &unequal, "df"), 37.1690815463, "Welch df");
    near(
        at(&t, title, &unequal, "Sig. (2-tailed)"),
        0.463370377791,
        "Welch p",
    );
    near(
        at(&t, title, &unequal, "Std. Error Difference"),
        0.735752221964,
        "Welch se",
    );
    near(
        at(&t, title, &unequal, "Lower"),
        -2.03571339864,
        "Welch lower",
    );
    near(
        at(&t, title, &unequal, "Upper"),
        0.945380065311,
        "Welch upper",
    );
    let pair = ["Pair 1", "x - a"];
    let title = "Paired Samples Test";
    near(at(&t, title, &pair, "Mean"), -0.653424657534, "mean");
    near(at(&t, title, &pair, "Std. Deviation"), 2.18759648624, "sd");
    near(
        at(&t, title, &pair, "Std. Error Mean"),
        0.256038802352,
        "se",
    );
    near(at(&t, title, &pair, "t"), -2.55205324948, "t");
    near(at(&t, title, &pair, "df"), 72.0, "df");
    near(
        at(&t, title, &pair, "Sig. (2-tailed)"),
        0.0128314162236,
        "p",
    );
    near(at(&t, title, &pair, "Lower"), -1.16382868167, "lower");
    near(at(&t, title, &pair, "Upper"), -0.143020633394, "upper");
    near(
        at(
            &t,
            "Paired Samples Correlations",
            &["Pair 1", "x & a"],
            "Correlation",
        ),
        0.754374475955,
        "correlation",
    );
}

#[test]
fn weighted_oneway_and_tukey_agree_with_r_on_the_repeated_cases() {
    let t = run("ONEWAY x BY g /POSTHOC=TUKEY.");
    let anova = |row: &str, column: &str| at(&t, "ANOVA", &["x", row], column);
    near(
        anova("Between Groups", "Sum of Squares"),
        265.032849772,
        "SSB",
    );
    near(
        anova("Within Groups", "Sum of Squares"),
        458.142766667,
        "SSW",
    );
    near(anova("Within Groups", "df"), 70.0, "df");
    near(anova("Between Groups", "F"), 20.2472906197, "F");
    near(anova("Between Groups", "Sig."), 1.15211833778e-07, "p");
    let title = "Multiple Comparisons: x";
    // R's TukeyHSD gives the differences the other way round (2-1, 3-1, 3-2)
    for (i, j, difference, lower, upper, p) in [
        (
            "1",
            "2",
            -0.545166666667,
            -2.29581849518,
            1.20548516185,
            0.737220616070,
        ),
        (
            "1",
            "3",
            -4.295166666667,
            -6.04581849518,
            -2.54451483815,
            3.87901624377e-07,
        ),
        (
            "2",
            "3",
            -3.75,
            -5.51842539920,
            -1.98157460080,
            8.97895759588e-06,
        ),
    ] {
        let key = ["Tukey HSD", i, j];
        near(
            at(&t, title, &key, "Mean Difference (I-J)"),
            difference,
            "difference",
        );
        near(at(&t, title, &key, "Lower Bound"), lower, "lower");
        near(at(&t, title, &key, "Upper Bound"), upper, "upper");
        // R's ptukey is good to about 1e-9 in absolute terms
        let got = at(&t, title, &key, "Sig.");
        assert!((got - p).abs() < 1e-9, "p: {got} vs {p}");
    }
}

#[test]
fn weighted_correlations_and_regression_agree_with_r_on_the_repeated_cases() {
    let t = run(
        "CORRELATIONS /VARIABLES=x y.\nNONPAR CORR /VARIABLES=x y /PRINT=BOTH.\n\
                 REGRESSION /STATISTICS=COEFF CI R ANOVA /DEPENDENT=z /METHOD=ENTER x y.",
    );
    let pearson = |row: &str| at(&t, "Correlations", &["x", row], "y");
    near(pearson("Pearson Correlation"), 0.784202242107, "Pearson");
    near(pearson("Sig. (2-tailed)"), 2.29950369692e-16, "Pearson p");
    near(pearson("N"), 73.0, "N");
    let regression = |model: &str, column: &str| at(&t, model, &["1", "Regression"], column);
    near(regression("ANOVA", "F"), 7.43286110011, "F");
    near(regression("ANOVA", "df"), 2.0, "df");
    let summary = |column: &str| at(&t, "Model Summary", &["1"], column);
    near(summary("R Square"), 0.175167568422, "R square");
    near(summary("Adjusted R Square"), 0.15160092752, "adjusted");
    near(summary("Std. Error of the Estimate"), 1.95732281252, "se");
    for (label, b, se, tvalue, p, lower, upper) in [
        (
            "(Constant)",
            4.3837950539105,
            1.017398653799,
            4.308827260133,
            5.25786867213e-05,
            2.3546574213082,
            6.412932686513,
        ),
        (
            "x",
            0.2462610505665,
            0.117299204314,
            2.09942643692,
            3.93863651568e-02,
            0.0123151643015,
            0.480206936832,
        ),
        (
            "y",
            0.0511454320047,
            0.141960511675,
            0.360279287538,
            7.19723121888e-01,
            -0.2319858808853,
            0.334276744895,
        ),
    ] {
        let key = ["1", label];
        near(at(&t, "Coefficients", &key, "B"), b, label);
        near(at(&t, "Coefficients", &key, "Std. Error"), se, label);
        near(at(&t, "Coefficients", &key, "t"), tvalue, label);
        near(at(&t, "Coefficients", &key, "Sig."), p, label);
        near(at(&t, "Coefficients", &key, "Lower Bound"), lower, label);
        near(at(&t, "Coefficients", &key, "Upper Bound"), upper, label);
    }
}

#[test]
fn weighted_rank_correlations_and_chi_square_agree_with_r_on_the_repeated_cases() {
    let t = run(
        "NONPAR CORR /VARIABLES=x y /PRINT=BOTH.\nCOMPUTE hi = x > 13.\n\
                 CROSSTABS g BY hi /STATISTICS=CHISQ.",
    );
    let rank = |test: &str, row: &str| at(&t, "Correlations", &[test, "x", row], "y");
    near(
        rank("Spearman's rho", "Correlation Coefficient"),
        0.714288054527,
        "rho",
    );
    near(
        rank("Spearman's rho", "Sig. (2-tailed)"),
        1.29062972307e-12,
        "rho p",
    );
    near(
        rank("Kendall's tau_b", "Correlation Coefficient"),
        0.53848232956,
        "tau",
    );
    near(
        rank("Kendall's tau_b", "Sig. (2-tailed)"),
        7.77826342026e-11,
        "tau p",
    );
    let title = "Chi-Square Tests: g by hi";
    near(
        at(&t, title, &["Pearson Chi-Square"], "Value"),
        35.5591810345,
        "chi-square",
    );
    near(
        at(
            &t,
            title,
            &["Pearson Chi-Square"],
            "Asymptotic Sig. (2-tailed)",
        ),
        1.8985496009e-08,
        "p",
    );
    near(
        at(&t, title, &["Likelihood Ratio"], "Value"),
        39.0797092132,
        "likelihood ratio",
    );
    near(
        at(&t, title, &["Linear-by-Linear Association"], "Value"),
        31.638130904,
        "linear",
    );
    near(at(&t, title, &["N of Valid Cases"], "Value"), 73.0, "N");
}
