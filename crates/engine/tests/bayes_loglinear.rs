//! BAYES LOGLINEAR on invented tables of bus lanes and trip outcomes. Expected values come from
//! our own code of the formulas in a general statistics package run as a black box (numerical
//! integration of Good's integral, the intrinsic sums by brute force), and its independent
//! implementation of the conjugate factors of contingency tables.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

/// Weighted counts: three lanes by three outcomes, and a 2 x 2 table of two ticket types by
/// two trip results in the first two lanes with their own weight.
const TRIPS: &str = "DATA LIST FREE /lane outcome n tick result m.
BEGIN DATA
1 1 14 1 1 18
1 2 6 1 2 7
1 3 3 2 1 9
2 1 7 2 2 16
2 2 11 1 1 0
2 3 5 1 1 0
3 1 4 1 1 0
3 2 8 1 1 0
3 3 13 1 1 0
END DATA.
";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
}

fn run(source: &str) -> Run {
    let source = format!("{TRIPS}{source}");
    let result = Session::default()
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    Run {
        tables: command.tables,
        warnings: command.warnings,
    }
}

fn fails(source: &str) -> String {
    let source = format!("{TRIPS}{source}");
    let errors = Session::default().run(&source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| {
            panic!(
                "no table {title}: {:?}",
                tables.iter().map(|t| &t.title).collect::<Vec<_>>()
            )
        })
}

fn at(table: &Table, row: usize, column: &str) -> f64 {
    let i = table
        .column_titles()
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.column_titles()));
    match &table.rows[row][i] {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn close(got: f64, expected: f64, tolerance: f64) {
    assert!(
        (got - expected).abs() <= tolerance * expected.abs(),
        "{got} != {expected}"
    );
}

/// BF01 of the lane table (WEIGHT BY n) or of the ticket table (WEIGHT BY m).
fn factor(weight: &str, table: &str, estbf: &str) -> f64 {
    let r = run(&format!(
        "WEIGHT BY {weight}.\nBAYES LOGLINEAR /TABLE {table} /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF {estbf}."
    ));
    at(find(&r.tables, "Bayes Factor"), 0, "Bayes Factor")
}

#[test]
fn conjugate_factors_of_every_design() {
    for (estbf, expected) in [
        ("MODEL=MULTINOMIAL", 0.0103308925744),
        ("FIX_MARGIN=ROW", 0.0328744885305),
        ("FIX_MARGIN=COLUMN", 0.0324372578331),
        ("MODEL=POISSON", 0.00300476855753),
        ("MODEL=POISSON SCALEPARAM=0.5", 3.89857716548e-05),
    ] {
        close(factor("n", "lane BY outcome", estbf), expected, 1e-9);
    }
    let shapes = "SHAPEPARAMS=2 1; 1.5 3";
    close(factor("m", "tick BY result", shapes), 0.0711042086697, 1e-9);
    let rows = format!("{shapes} FIX_MARGIN=ROW");
    close(factor("m", "tick BY result", &rows), 0.0891128855673, 1e-9);
}

#[test]
fn mixture_intrinsic_and_nonparametric_factors() {
    let mixture = "PRIOR_TYPE=MIXTURE_DIRICHLET";
    close(
        factor("n", "lane BY outcome", mixture),
        0.0965529459079,
        1e-6,
    );
    let columns = format!("{mixture} FIX_MARGIN=COLUMN");
    close(
        factor("n", "lane BY outcome", &columns),
        0.12077925896,
        1e-6,
    );
    let rows = format!("{mixture} FIX_MARGIN=ROW");
    close(factor("n", "lane BY outcome", &rows), 0.120689856185, 1e-6);
    close(factor("m", "tick BY result", mixture), 0.193344381699, 1e-6);
    for (estbf, expected) in [
        ("PRIOR_TYPE=INTRINSIC", 0.179930262717),
        ("PRIOR_TYPE=INTRINSIC FIX_MARGIN=ROW", 0.19027623066),
        ("PRIOR_TYPE=INTRINSIC FIX_MARGIN=COLUMN", 0.190304000967),
        ("MODEL=NONPARAMETRIC FIX_MARGIN=ROW", 0.12048030023),
        (
            "MODEL=NONPARAMETRIC FIX_MARGIN=ROW LAMBDAS=2 0.5",
            0.198230422615,
        ),
        ("MODEL=NONPARAMETRIC FIX_MARGIN=COLUMN", 0.119767399045),
    ] {
        close(factor("m", "tick BY result", estbf), expected, 1e-9);
    }
    // two rows of three columns
    let r = run(
        "WEIGHT BY n.\nTEMPORARY.\nSELECT IF lane < 3.\nBAYES LOGLINEAR /TABLE lane BY outcome\n\
         /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF MODEL=NONPARAMETRIC FIX_MARGIN=ROW.",
    );
    close(
        at(find(&r.tables, "Bayes Factor"), 0, "Bayes Factor"),
        0.780954846986,
        1e-9,
    );
}

#[test]
fn fallbacks_and_unusable_settings_warn() {
    let r = run(
        "WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /INFERENCE ANALYSIS=BAYESFACTOR\n\
         /ESTBF PRIOR_TYPE=INTRINSIC LAMBDAS=3.",
    );
    close(
        at(find(&r.tables, "Bayes Factor"), 0, "Bayes Factor"),
        0.0965529459079,
        1e-6,
    );
    assert!(r.warnings.iter().any(|w| w.contains("2 x 2")));
    assert!(r.warnings.iter().any(|w| w.contains("LAMBDAS")));
    // a row sum of the shapes too small for the null model
    let r = run(
        "WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /INFERENCE ANALYSIS=BAYESFACTOR\n\
         /ESTBF SHAPEPARAMS=0.5.",
    );
    assert_eq!(find(&r.tables, "Bayes Factor").rows[0][3], Cell::Empty);
    assert!(r.warnings.iter().any(|w| w.contains("SHAPEPARAMS")));
    let r = run(
        "WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /INFERENCE ANALYSIS=BAYESFACTOR\n\
         /ESTBF MODEL=NONPARAMETRIC FIX_MARGIN=ROW.",
    );
    assert!(r.warnings.iter().any(|w| w.contains("exactly two")));
    assert!(
        fails("WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF SHAPEPARAMS=1 2.")
            .contains("3 rows of 3")
    );
    assert!(
        fails("WEIGHT BY m.\nBAYES LOGLINEAR /TABLE tick BY result /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF MODEL=NONPARAMETRIC FIX_MARGIN=ROW LAMBDAS=1 2 3.")
            .contains("LAMBDAS")
    );
    // one column: the crosstab only
    let r = run(
        "WEIGHT BY m.\nTEMPORARY.\nSELECT IF result = 1.\nBAYES LOGLINEAR /TABLE tick BY result.",
    );
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("two rows and two columns"))
    );
    assert_eq!(r.tables.len(), 2);
}

#[test]
fn posterior_of_the_log_odds_ratio() {
    let source =
        "WEIGHT BY m.\nBAYES LOGLINEAR /TABLE tick BY result /CRITERIA SEED=2024 CILEVEL=90.";
    let r = run(source);
    let t = find(&r.tables, "Posterior Distribution");
    assert_eq!(t.rows.len(), 1);
    // digamma and trigamma of the counts plus 1 give the exact moments
    close(at(t, 0, "Mean"), 1.4540116746, 0.02);
    close(at(t, 0, "Variance"), 0.352931789817, 0.05);
    let ci = "90% Simultaneous Credible Interval";
    let (lower, upper) = (
        at(t, 0, &format!("{ci} / Lower")),
        at(t, 0, &format!("{ci} / Upper")),
    );
    // one term: the central interval, mean -/+ 1.645 sd of a nearly normal posterior
    close(lower, 1.4540116746 - 1.645 * 0.352931789817f64.sqrt(), 0.05);
    close(upper, 1.4540116746 + 1.645 * 0.352931789817f64.sqrt(), 0.05);
    // a fixed seed repeats itself
    assert_eq!(run(source).tables[2].rows, t.rows);
    // a 3 x 3 table has four terms against the last lane and outcome
    let r = run(
        "WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /CRITERIA SEED=5 POSTSAMPLES=1000.",
    );
    let t = find(&r.tables, "Posterior Distribution");
    assert_eq!(t.rows.len(), 4);
    assert_eq!(t.rows[0][0], Cell::Text("1".into()));
    assert!(at(t, 0, "Mean") > 2.0 && at(t, 0, "95% Simultaneous Credible Interval / Lower") > 0.0);
}

#[test]
fn print_tables_reuse_the_crosstab_tests() {
    let r = run(
        "WEIGHT BY m.\nBAYES LOGLINEAR /TABLE tick BY result /INFERENCE ANALYSIS=BAYESFACTOR\n\
         /PRINT CELLS=OBSERVED EXPECTED STATISTICS=CHISQ LRT.",
    );
    assert_eq!(r.tables.len(), 3);
    let crosstab = &r.tables[0];
    assert_eq!(crosstab.rows.len(), 6);
    let tests = find(&r.tables, "Chi-Square Tests");
    let names: Vec<&Cell> = tests.rows.iter().map(|row| &row[0]).collect();
    assert_eq!(names.len(), 4, "{names:?}");
    close(at(tests, 0, "Value"), 6.52173913043, 1e-10);
    // no expected count below 5: no exact test
    assert!(!tests.column_titles().iter().any(|c| c.starts_with("Exact")));
    let continuity = tests
        .rows
        .iter()
        .position(|row| row[0] == Cell::Text("Continuity Correction".into()))
        .unwrap();
    close(at(tests, continuity, "Value"), 5.152979066, 1e-9);
    close(
        at(tests, continuity, "Asymptotic Sig. (2-tailed)"),
        0.0232065264,
        1e-8,
    );
    // a small table adds Fisher's exact test
    let small = "DATA LIST FREE /g h w.\nBEGIN DATA\n1 1 6\n1 2 1\n2 1 2\n2 2 7\nEND DATA.\nWEIGHT BY w.\n\
                 BAYES LOGLINEAR /TABLE g BY h /INFERENCE ANALYSIS=BAYESFACTOR /PRINT SUPPRESS_TABLE=TRUE.";
    let result = Session::default().run(small, None).unwrap();
    let tables = &result.commands.last().unwrap().tables;
    let tests = find(tables, "Chi-Square Tests");
    let fisher = tests
        .rows
        .iter()
        .position(|row| row[0] == Cell::Text("Fisher's Exact Test".into()))
        .unwrap();
    close(
        at(tests, fisher, "Exact Sig. (2-tailed)"),
        0.0405594406,
        1e-8,
    );
    assert_eq!(tables.len(), 2);
    // the likelihood ratio of the lane table, categories from the highest
    let r = run(
        "WEIGHT BY n.\nBAYES LOGLINEAR /TABLE lane BY outcome /INFERENCE ANALYSIS=BAYESFACTOR\n\
         /PRINT STATISTICS=LRT /CRITERIA ORDER=DESCENDING.",
    );
    let tests = find(&r.tables, "Chi-Square Tests");
    close(at(tests, 0, "Value"), 15.2610712143, 1e-10);
    close(
        at(tests, 0, "Asymptotic Sig. (2-tailed)"),
        0.00418926892758,
        1e-8,
    );
    assert_eq!(r.tables[0].rows[0][0], Cell::Text("3".into()));
    // the conjugate factor does not depend on the order
    close(
        at(find(&r.tables, "Bayes Factor"), 0, "Bayes Factor"),
        0.0103308925744,
        1e-9,
    );
}

#[test]
fn large_and_flat_tables() {
    let factor_of = |counts: [f64; 4], estbf: &str| {
        let source = format!(
            "DATA LIST FREE /g h w.\nBEGIN DATA\n1 1 {}\n1 2 {}\n2 1 {}\n2 2 {}\nEND DATA.\nWEIGHT BY w.\n\
             BAYES LOGLINEAR /TABLE g BY h /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF {estbf}.",
            counts[0], counts[1], counts[2], counts[3]
        );
        let result = Session::default().run(&source, None).unwrap();
        at(
            find(&result.commands.last().unwrap().tables, "Bayes Factor"),
            0,
            "Bayes Factor",
        )
    };
    // 1040 cases: training tables of 500
    let big = [340.0, 210.0, 190.0, 300.0];
    close(
        factor_of(big, "PRIOR_TYPE=INTRINSIC"),
        4.53719055192e-10,
        1e-8,
    );
    // both rows above 5000: training rows of 5000
    let huge = [2700.0, 2600.0, 2900.0, 3100.0];
    close(
        factor_of(huge, "PRIOR_TYPE=INTRINSIC FIX_MARGIN=ROW"),
        0.126716697819,
        1e-8,
    );
    // the mixture on the same large table, and on a flat one where the tails of the weight count
    close(
        factor_of(big, "PRIOR_TYPE=MIXTURE_DIRICHLET"),
        3.64993747576e-11,
        1e-6,
    );
    let columns = "PRIOR_TYPE=MIXTURE_DIRICHLET FIX_MARGIN=COLUMN";
    close(factor_of(big, columns), 4.12509000124e-11, 1e-6);
    close(
        factor_of([10.0; 4], "PRIOR_TYPE=MIXTURE_DIRICHLET"),
        0.472012723571,
        1e-6,
    );
}
