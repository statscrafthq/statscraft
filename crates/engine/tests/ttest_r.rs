//! T-TEST against R on 30 invented cases: the independent-samples test with equal and unequal
//! variances (`t.test`, and Levene's F as the F test of the absolute deviations from the group
//! means), the one-sample test at a 90% interval, the paired test with the correlation of the
//! pair, and the effect sizes of the manual: Cohen's d with its interval from the noncentral
//! t, Hedges' g with the exact correction J(k) (k = W - 1 for one sample, W1 + W2 - 2 for two
//! groups, 2W - 2 for pairs) and Glass's delta from the second group, with intervals found by
//! inverting R's `pt` with a noncentrality parameter.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b g.
BEGIN DATA
8.3 9.6 1
12.8 15.4 1
7.5 11.8 1
10.1 12.8 1
13.4 16.6 1
8.8 11.1 1
9.1 8.3 1
8.7 6.3 1
9.4 7.4 1
10.3 10.9 1
12.5 15.8 1
8.4 7.9 1
7.8 8.5 1
10.9 14.7 2
7.7 7.8 2
11 12.7 2
9.4 8.8 2
3.9 4 2
12.3 11.9 2
10.6 11.3 2
14.7 17.8 2
14.8 15.9 2
16.6 19 2
14 13.9 2
14.4 15 2
10.5 9.8 2
16.5 18.5 2
16.7 17.3 2
9.2 9.9 2
8.5 9.7 2
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

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The number in `column` of the row of table `title` that has `label` in its first two cells.
fn at(tables: &[Table], title: &str, label: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"));
    let column = table
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.column_titles()));
    let row = table
        .rows
        .iter()
        .find(|r| {
            r.iter()
                .take(3)
                .any(|c| matches!(c, Cell::Text(s) if s == label))
        })
        .unwrap_or_else(|| panic!("no row {label} in {title}"));
    match row[column] {
        Cell::Number(n) => n,
        ref other => panic!("{label} {column}: {other:?}"),
    }
}

#[test]
fn the_independent_samples_tests_agree_with_r() {
    let t = run("T-TEST GROUPS=g(1 2) /VARIABLES=a.");
    let title = "Independent Samples Test";
    let equal = "Equal variances assumed";
    let unequal = "Equal variances not assumed";
    near(at(&t, title, equal, "F"), 5.50009931655, 1e-9, "Levene F");
    near(
        at(&t, title, equal, "Sig."),
        0.0263316162113,
        1e-9,
        "Levene p",
    );
    for (label, tvalue, df, p, se, lower, upper) in [
        (
            equal,
            -1.89080383648,
            28.0,
            0.069039928019,
            1.10417736899,
            -4.34958761387,
            0.17402200301,
        ),
        (
            unequal,
            -2.03730796174,
            25.6974654125,
            0.0520409226627,
            1.02477526453,
            -4.19544615927,
            0.019880548408,
        ),
    ] {
        near(at(&t, title, label, "t"), tvalue, 1e-9, label);
        near(at(&t, title, label, "df"), df, 1e-9, label);
        near(at(&t, title, label, "Sig. (2-tailed)"), p, 1e-9, label);
        near(
            at(&t, title, label, "Mean Difference"),
            -2.08778280543,
            1e-9,
            label,
        );
        near(
            at(&t, title, label, "Std. Error Difference"),
            se,
            1e-9,
            label,
        );
        near(at(&t, title, label, "Lower"), lower, 1e-9, label);
        near(at(&t, title, label, "Upper"), upper, 1e-9, label);
    }
    let title = "Group Statistics";
    near(at(&t, title, "1", "Mean"), 9.77692307692, 1e-9, "mean 1");
    near(at(&t, title, "2", "Mean"), 11.8647058824, 1e-9, "mean 2");
    near(
        at(&t, title, "1", "Std. Deviation"),
        1.95667824222,
        1e-9,
        "sd 1",
    );
    near(
        at(&t, title, "2", "Std. Deviation"),
        3.58415631224,
        1e-9,
        "sd 2",
    );
    // effect sizes: the pooled standardizer, J(28), and the second group's sd for Glass
    let title = "Independent Samples Effect Sizes";
    for (label, standardizer, estimate, lower, upper) in [
        (
            "Cohen's d",
            2.99691448739,
            -0.696644103198,
            -1.43542630624,
            0.0538065665629,
        ),
        (
            "Hedges' correction",
            3.08029146229,
            -0.677787420766,
            -1.39657235214,
            0.0523501366099,
        ),
        (
            "Glass's delta",
            3.58415631224,
            -0.582503279307,
            -1.32347358293,
            0.175248322326,
        ),
    ] {
        near(
            at(&t, title, label, "Standardizer"),
            standardizer,
            1e-9,
            label,
        );
        near(
            at(&t, title, label, "Point Estimate"),
            estimate,
            1e-9,
            label,
        );
        near(at(&t, title, label, "Lower"), lower, 1e-8, label);
        near(at(&t, title, label, "Upper"), upper, 1e-8, label);
    }
}

#[test]
fn the_one_sample_test_and_its_effect_sizes_agree_with_r() {
    let t = run("T-TEST /TESTVAL=9 /VARIABLES=a /CRITERIA=CI(.90).");
    let title = "One-Sample Test";
    near(at(&t, title, "a", "t"), 3.43295973744, 1e-9, "t");
    near(at(&t, title, "a", "df"), 29.0, 1e-12, "df");
    near(
        at(&t, title, "a", "Sig. (2-tailed)"),
        0.00181728223987,
        1e-9,
        "p",
    );
    near(
        at(&t, title, "a", "Mean Difference"),
        1.96,
        1e-9,
        "difference",
    );
    near(at(&t, title, "a", "Lower"), 0.98990736079, 1e-9, "lower");
    near(at(&t, title, "a", "Upper"), 2.93009263921, 1e-9, "upper");
    let title = "One-Sample Effect Sizes";
    for (label, estimate, lower, upper) in [
        ("Cohen's d", 0.626769829068, 0.292751666437, 0.951079262942),
        (
            "Hedges' correction",
            0.610395457629,
            0.285103524642,
            0.926232302548,
        ),
    ] {
        near(
            at(&t, title, label, "Point Estimate"),
            estimate,
            1e-9,
            label,
        );
        near(at(&t, title, label, "Lower"), lower, 1e-8, label);
        near(at(&t, title, label, "Upper"), upper, 1e-8, label);
    }
}

#[test]
fn the_paired_test_and_its_effect_sizes_agree_with_r() {
    let t = run("T-TEST PAIRS=a WITH b (PAIRED).");
    let title = "Paired Samples Test";
    for (column, want) in [
        ("Mean", -1.05333333333),
        ("Std. Deviation", 1.67079030099),
        ("Std. Error Mean", 0.305043178905),
        ("Lower", -1.67721668496),
        ("Upper", -0.429449981707),
        ("t", -3.45306306181),
        ("df", 29.0),
        ("Sig. (2-tailed)", 0.00172419112422),
    ] {
        near(at(&t, title, "a - b", column), want, 1e-9, column);
    }
    let title = "Paired Samples Correlations";
    near(
        at(&t, title, "a & b", "Correlation"),
        0.912502961969,
        1e-9,
        "correlation",
    );
    near(
        at(&t, title, "a & b", "Sig."),
        2.19644846944e-12,
        1e-8,
        "correlation p",
    );
    // Cohen's d with the sd of the differences, Hedges' g with J(2W - 2) as the manual has it
    let title = "Paired Samples Effect Sizes";
    for (label, estimate, lower, upper) in [
        (
            "Cohen's d",
            -0.630440177148,
            -1.01852392022,
            -0.233309578954,
        ),
        (
            "Hedges' correction",
            -0.622246712469,
            -1.00528675662,
            -0.230277389916,
        ),
    ] {
        near(
            at(&t, title, label, "Point Estimate"),
            estimate,
            1e-9,
            label,
        );
        near(at(&t, title, label, "Lower"), lower, 1e-8, label);
        near(at(&t, title, label, "Upper"), upper, 1e-8, label);
    }
}
