//! CROSSTABS /METHOD on invented tables. The exact values were computed in R by listing every
//! table with the observed margins, its hypergeometric probability and each statistic, and
//! summing (fisher.test agrees on Fisher's test); the Monte Carlo checks use the interval
//! formula worked by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// A 3 x 3 table, rows valued 1 to 3, columns 1, 2 and 4:
/// [[4, 1, 0], [2, 3, 1], [0, 2, 5]].
const DATA: &str = "DATA LIST FREE / r c w.
BEGIN DATA
1 1 4  1 2 1
2 1 2  2 2 3  2 4 1
3 2 2  3 4 5
END DATA.
WEIGHT BY w.";

fn run(syntax: &str) -> (Vec<Table>, Vec<String>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}\n{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (tables, engine.warnings().to_vec())
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    (tables.iter())
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("{title}"))
}

/// The cells of the row whose label cells (before `from`) contain `label`, from `from` on.
fn row<'a>(t: &'a Table, label: &str, nth: usize) -> &'a [Cell] {
    let rows: Vec<&Vec<Cell>> = (t.rows.iter())
        .filter(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
        .collect();
    rows.get(nth).unwrap_or_else(|| panic!("{label} {nth}"))
}

fn column(t: &Table, name: &str) -> usize {
    (t.columns.iter())
        .position(|c| c == name)
        .unwrap_or_else(|| panic!("{name} in {:?}", t.columns))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(cell: &Cell, want: f64) {
    let got = number(cell);
    assert!((got - want).abs() < 1e-10, "{got} vs {want}");
}

#[test]
fn exact_chi_square_tests() {
    let (tables, warnings) = run("CROSSTABS r BY c /STATISTICS=CHISQ /METHOD=EXACT.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let t = table(&tables, "Chi-Square Tests");
    let [two, one, point] = [
        "Exact Sig. (2-tailed)",
        "Exact Sig. (1-tailed)",
        "Point Probability",
    ]
    .map(|c| column(t, c));
    let pearson = row(t, "Pearson Chi-Square", 0);
    close(&pearson[two], 0.0146495661201544);
    close(&pearson[point], 0.00220367867426691);
    assert_eq!(pearson[one], Cell::Empty);
    let ratio = row(t, "Likelihood Ratio", 0);
    close(&ratio[two], 0.0211381755499403);
    close(&ratio[point], 0.00881471469706765);
    // Freeman-Halton: fisher.test gives 0.0208933223639105
    let fisher = row(t, "Fisher's Exact Test", 0);
    close(&fisher[two], 0.0208933223639106);
    // the probability of the observed table
    close(&fisher[point], 0.000367279779044485);
    assert_eq!(fisher[1], Cell::Empty);
    // the scores are the values 1, 2, 3 and 1, 2, 4
    let linear = row(t, "Linear-by-Linear Association", 0);
    close(&linear[two], 0.00136546460075872);
    close(&linear[one], 0.000668449197860963);
    close(&linear[point], 0.000504397563221093);
    // the asymptotic columns stay as they were
    assert_eq!(
        t.columns[..4],
        ["Statistic", "Value", "df", "Asymptotic Sig. (2-tailed)"]
    );
}

#[test]
fn exact_measures() {
    let (tables, _) = run(
        "CROSSTABS r BY c /STATISTICS=PHI CC LAMBDA UC BTAU CTAU GAMMA CORR KAPPA D /METHOD=EXACT TIMER(1).",
    );
    let t = table(&tables, "Symmetric Measures");
    let exact = column(t, "Exact Significance");
    for name in ["Phi", "Cramer's V", "Contingency Coefficient"] {
        close(&row(t, name, 0)[exact], 0.0146495661201544);
    }
    for name in ["Kendall's tau-b", "Kendall's tau-c"] {
        close(&row(t, name, 0)[exact], 0.00109204520969227);
    }
    assert_eq!(row(t, "Gamma", 0)[exact], Cell::Empty);
    close(
        &row(t, "Spearman Correlation", 0)[exact],
        0.00106266282736871,
    );
    close(&row(t, "Pearson's R", 0)[exact], 0.00136546460075872);
    // kappa pairs the values 1 and 2 of both variables; 3 and 4 have no partner
    close(&row(t, "Kappa", 0)[exact], 0.0377396296513944);
    assert_eq!(row(t, "N of Valid Cases", 0)[exact], Cell::Empty);

    let t = table(&tables, "Directional Measures");
    let exact = column(t, "Exact Significance");
    // lambda: symmetric (none), rows dependent, columns dependent
    assert_eq!(row(t, "Lambda", 0)[exact], Cell::Empty);
    let lambda: Vec<&Vec<Cell>> = (t.rows.iter())
        .filter(|r| {
            r.iter()
                .any(|c| matches!(c, Cell::Text(s) if s.ends_with("Dependent")))
        })
        .collect();
    // lambda, Goodman and Kruskal's tau, the uncertainty coefficient and Somers' d, each with
    // the rows and then the columns dependent
    let (ratio, s) = (0.0211381755499403, 0.00109204520969227);
    let expected = [0.0312383694736636, 0.0290346907993967, 0.0190569234686882];
    let expected = [&expected[..], &[0.0146495661201544, ratio, ratio, s, s]].concat();
    assert_eq!(lambda.len(), expected.len());
    for (r, want) in lambda.iter().zip(expected) {
        close(&r[exact], want);
    }
    close(&row(t, "Uncertainty Coefficient", 0)[exact], ratio);
    close(&row(t, "Somers' d", 0)[exact], s);
}

#[test]
fn descending_rows_and_strings_keep_the_exact_values() {
    let (tables, _) = run("CROSSTABS r BY c /STATISTICS=CHISQ BTAU /FORMAT=DVALUE /METHOD=EXACT.");
    let t = table(&tables, "Symmetric Measures");
    close(
        &row(t, "Kendall's tau-b", 0)[column(t, "Exact Significance")],
        0.00109204520969227,
    );
    let t = table(&tables, "Chi-Square Tests");
    close(
        &row(t, "Linear-by-Linear Association", 0)[column(t, "Exact Sig. (1-tailed)")],
        0.000668449197860963,
    );

    // string categories: no linear-by-linear row
    let (tables, _) = run("STRING g h (A1).
COMPUTE g = 'p'.
IF (r > 1) g = 'q'.
COMPUTE h = 'x'.
IF (c = 2) h = 'y'.
IF (c = 4) h = 'z'.
SELECT IF (r < 3).
CROSSTABS g BY h /STATISTICS=CHISQ /METHOD=EXACT.");
    let t = table(&tables, "Chi-Square Tests");
    // the table here is [[4, 1, 0], [2, 3, 1]]
    let two = column(t, "Exact Sig. (2-tailed)");
    close(&row(t, "Fisher's Exact Test", 0)[two], fisher_2x3());
    assert!(
        !t.rows
            .iter()
            .any(|r| r.contains(&Cell::Text("Linear-by-Linear Association".into())))
    );
}

/// fisher.test(matrix(c(4, 2, 1, 3, 0, 1), 2)) in R.
fn fisher_2x3() -> f64 {
    0.372294372294372
}

#[test]
fn a_two_by_two_keeps_its_fisher_values_and_adds_the_point_probability() {
    let (tables, _) = run("SELECT IF (r < 3 AND c < 4).
CROSSTABS r BY c /STATISTICS=CHISQ /METHOD=EXACT.");
    let t = table(&tables, "Chi-Square Tests");
    let fisher = row(t, "Fisher's Exact Test", 0);
    // the table is [[4, 1], [2, 3]] after the selection; fisher.test gives 0.523809523809524
    close(
        &fisher[column(t, "Exact Sig. (2-tailed)")],
        0.523809523809524,
    );
    close(
        &fisher[column(t, "Exact Sig. (1-tailed)")],
        0.261904761904762,
    );
    // the observed table alone: 5! 5! 6! 4! / (10! 4! 1! 2! 3!) = 10/42
    close(&fisher[column(t, "Point Probability")], 0.238095238095238);
    assert_eq!(
        row(t, "Continuity Correction", 0)[column(t, "Point Probability")],
        Cell::Empty
    );
}

#[test]
fn monte_carlo_estimates_hold_the_exact_value() {
    let syntax = "SET SEED=20261118.
CROSSTABS r BY c /STATISTICS=CHISQ BTAU /METHOD=MC CIN(99) SAMPLES(20000).";
    let (tables, _) = run(syntax);
    let t = table(&tables, "Chi-Square Tests");
    let sig = column(t, "Significance");
    assert_eq!(t.column_groups[sig], ["Monte Carlo Sig. (2-tailed)"]);
    assert_eq!(
        t.column_groups[sig + 1],
        ["Monte Carlo Sig. (2-tailed)", "99% Confidence Interval"]
    );
    for (name, exact) in [
        ("Pearson Chi-Square", 0.0146495661201544),
        ("Likelihood Ratio", 0.0211381755499403),
        ("Fisher's Exact Test", 0.0208933223639106),
        ("Linear-by-Linear Association", 0.00136546460075872),
    ] {
        let r = row(t, name, 0);
        let [p, low, high] =
            [r[sig].clone(), r[sig + 1].clone(), r[sig + 2].clone()].map(|c| number(&c));
        assert!(low <= exact && exact <= high, "{name}: {low} {p} {high}");
        // p +- z root(p (1 - p) / n), z the 99.5% normal quantile
        let half = 2.5758293035489 * (p * (1.0 - p) / 20000.0).sqrt();
        assert!((low - (p - half).max(0.0)).abs() < 1e-12 && (high - (p + half)).abs() < 1e-12);
    }
    let linear = row(t, "Linear-by-Linear Association", 0);
    let one = number(&linear[sig + 3]);
    assert!(
        (number(&linear[sig + 4])..=number(&linear[sig + 5])).contains(&0.000668449197860963),
        "{one}"
    );
    let m = table(&tables, "Symmetric Measures");
    let sig = column(m, "Significance");
    let tau = row(m, "Kendall's tau-b", 0);
    assert!((number(&tau[sig + 1])..=number(&tau[sig + 2])).contains(&0.00109204520969227));
    // the same seed draws the same tables
    let (again, _) = run(syntax);
    assert_eq!(table(&again, "Chi-Square Tests").rows, t.rows);
}

#[test]
fn fractional_weights_are_rounded_per_cell() {
    // every weight 0.2 lower rounds back to the table above
    let (tables, _) = run("COMPUTE w = w - 0.2.
CROSSTABS r BY c /STATISTICS=CHISQ /METHOD=EXACT.");
    let t = table(&tables, "Chi-Square Tests");
    close(
        &row(t, "Fisher's Exact Test", 0)[column(t, "Exact Sig. (2-tailed)")],
        0.0208933223639106,
    );
}
