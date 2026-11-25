//! RELIABILITY against R on 40 invented cases of five items (psych's alpha and ICC, and the
//! statistics written out from the Algorithms manual): Cronbach's alpha (raw and standardized),
//! the item-total statistics (corrected correlation, squared multiple correlation, alpha if
//! deleted), the scale statistics, the ANOVA with Tukey's nonadditivity, Hotelling's T-squared,
//! the intraclass correlations (psych::ICC), the split-half statistics, Guttman's lambdas 1 to
//! 6, the parallel model (the Box-corrected likelihood ratio chi-square of equal variances and
//! covariances) and McDonald's omega by the manual's closed form with its item-deleted values;
//! then the item summaries (variances over all ordered pairs, as the manual's denominators
//! have it), the strictly parallel model, the one-way intraclass correlations, Friedman's test
//! (friedman.test) with Kendall's W on 24 people's ranks of four items, and Cochran's Q by its
//! formula on 30 people's four pass or fail tasks.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /i1 i2 i3 i4 i5.
BEGIN DATA
3.8 2.81 3.02 3.11 5.205 
1.2 1.71 1.34 3.27 4.8 
2.69 2.19 3.39 3.04 5.685 
2.54 0.94 2.29 1.69 2.985 
3.47 4.66 2.09 2.58 5.925 
4.45 3.98 3.45 3.32 6.555 
2.8 2.51 2.87 2.61 4.905 
2.12 4.2 2.85 2.53 3.405 
3.17 3.83 3.43 3.62 5.325 
2.27 3.41 3.98 2.78 5.085 
2.26 2.58 3.01 2.65 3.075 
2.44 2.66 2.63 2.18 5.85 
0.51 1.49 1.82 1.45 2.025 
2.27 1.79 2.85 1.5 5.55 
2.33 2.3 2.6 2.51 4.53 
2.8 2.22 2.23 2.67 3.825 
5.44 3.3 4.81 5.13 2.94 
3.14 4.28 3.98 2.57 6.84 
2.28 3.36 3.38 2.35 3.975 
3.6 4.39 2.72 3.95 3.84 
2.57 0.91 1.06 0.61 1.605 
4.38 3.59 4.39 5.23 5.505 
4.6 3.83 3.32 3.1 5.88 
4.26 4.63 2.67 2.1 4.14 
3.22 3.83 3.04 2.56 6.765 
3.84 4.73 4.04 4.58 3.945 
2.71 2.56 3.6 3.32 3.075 
4.35 4.63 3.5 3.89 4.74 
0.62 1.89 1.37 2.03 1.59 
3.46 4.3 3.32 4.53 5.505 
4.24 4.22 3.42 4.3 5.115 
2.35 4.16 2.77 1.24 5.37 
1.69 3.57 2.81 1.71 4.065 
2.91 2.21 2.97 2.54 3.075 
2.65 1.51 2.69 3.13 4.02 
3.1 3.56 2.37 3.07 4.245 
2.29 2.87 3.93 3.53 2.85 
3.49 3.26 2.59 4.53 3.045 
3.86 3.73 3.01 4.53 2.625 
3.23 2.94 4.94 3.23 5.28 
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

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn column(t: &Table, column: &str) -> usize {
    t.column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()))
}

/// The number in `column` of row `row`.
fn at(t: &Table, row: usize, name: &str) -> f64 {
    match t.rows[row][column(t, name)] {
        Cell::Number(n) => n,
        ref other => panic!("{} row {row} {name}: {other:?}", t.title),
    }
}

/// The number in `name` of the first row whose cells include `label`.
fn cell(t: &Table, label: &str, name: &str) -> f64 {
    let c = column(t, name);
    let row = t
        .rows
        .iter()
        .find(|r| r.iter().any(|x| matches!(x, Cell::Text(s) if s == label)))
        .unwrap_or_else(|| panic!("no row {label} in {}", t.title));
    match row[c] {
        Cell::Number(n) => n,
        ref other => panic!("{label} {name}: {other:?}"),
    }
}

/// The last number of every row, in order.
fn last_numbers(t: &Table) -> Vec<f64> {
    t.rows
        .iter()
        .filter_map(|r| {
            r.iter().rev().find_map(|c| match c {
                Cell::Number(n) => Some(*n),
                _ => None,
            })
        })
        .collect()
}

fn check_all(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?}");
    for (g, w) in got.iter().zip(want) {
        near(*g, *w, what);
    }
}

#[test]
fn alpha_item_total_scale_anova_and_icc_agree_with_r() {
    let t = run(
        "RELIABILITY VARIABLES=i1 TO i5 /SCALE(total)=ALL /MODEL=ALPHA \
         /STATISTICS=SCALE ANOVA TUKEY HOTELLING /ICC=MODEL(MIXED) TYPE(CONSISTENCY) CIN=95 TESTVAL=0 \
         /SUMMARY=TOTAL.",
    );
    let rel = table(&t, "Reliability Statistics");
    near(at(rel, 0, "Cronbach's Alpha"), 0.7868368274, "alpha");
    near(
        at(rel, 0, "Cronbach's Alpha Based on Standardized Items"),
        0.8068282478,
        "standardized alpha",
    );
    let items = table(&t, "Item-Total Statistics");
    for (item, mean, variance, corrected, smc, alpha) in [
        (
            "i1",
            13.503125,
            10.4026137,
            0.7023774985,
            0.5488564571,
            0.7023416188,
        ),
        (
            "i2",
            13.349625,
            10.55043127,
            0.6474966239,
            0.4311746434,
            0.7193626948,
        ),
        (
            "i3",
            13.474375,
            11.77564511,
            0.6425433821,
            0.4525708063,
            0.7334093699,
        ),
        (
            "i4",
            13.506375,
            11.2083128,
            0.5436542111,
            0.5268783189,
            0.7533403027,
        ),
        (
            "i5",
            12.119,
            10.86028615,
            0.3885528537,
            0.2938363492,
            0.8249548413,
        ),
    ] {
        near(cell(items, item, "Scale Mean if Item Deleted"), mean, item);
        near(
            cell(items, item, "Scale Variance if Item Deleted"),
            variance,
            item,
        );
        near(
            cell(items, item, "Corrected Item-Total Correlation"),
            corrected,
            item,
        );
        near(cell(items, item, "Squared Multiple Correlation"), smc, item);
        near(
            cell(items, item, "Cronbach's Alpha if Item Deleted"),
            alpha,
            item,
        );
    }
    let scale = table(&t, "Scale Statistics");
    near(at(scale, 0, "Mean"), 16.488125, "scale mean");
    near(at(scale, 0, "Variance"), 16.2577637, "scale variance");
    // aov(): between people, between items, residual; Tukey's nonadditivity; Hotelling
    let anova = table(&t, "ANOVA");
    near(
        cell(anova, "Between People", "Sum of Squares"),
        126.810556875,
        "between people",
    );
    near(
        cell(anova, "Between Items", "Sum of Squares"),
        58.0611775,
        "between items",
    );
    near(
        cell(anova, "Between Items", "F"),
        20.942227338,
        "F of items",
    );
    near(
        cell(anova, "Residual", "Sum of Squares"),
        108.1253625,
        "residual",
    );
    near(
        cell(anova, "Nonadditivity", "Sum of Squares"),
        0.4969509568,
        "nonadditivity",
    );
    let hotelling = table(&t, "Hotelling");
    near(at(hotelling, 0, "Hotelling's T-Squared"), 49.30877164, "T2");
    near(at(hotelling, 0, "F"), 11.3789473, "F of T2");
    // psych::ICC: the consistency forms of the mixed model (ICC3 and ICC3k)
    let icc = table(&t, "Intraclass Correlation Coefficient");
    for (row, value, lower, upper) in [
        ("Single Measures", 0.4247082205, 0.2805518509, 0.5850760677),
        ("Average Measures", 0.7868368274, 0.6609906516, 0.8757825939),
    ] {
        near(cell(icc, row, "Intraclass Correlation"), value, row);
        near(cell(icc, row, "Lower Bound"), lower, row);
        near(cell(icc, row, "Upper Bound"), upper, row);
        near(cell(icc, row, "Value"), 4.691241868, row);
        near(cell(icc, row, "df2"), 156.0, row);
    }
    // the random-effects absolute forms (ICC2 and ICC2k)
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL(RANDOM) TYPE(ABSOLUTE).");
    let icc = table(&t, "Intraclass Correlation Coefficient");
    for (row, value, lower, upper) in [
        ("Single Measures", 0.3300460634, 0.1717070144, 0.5079949524),
        ("Average Measures", 0.7112496839, 0.5089641876, 0.8377281254),
    ] {
        near(cell(icc, row, "Intraclass Correlation"), value, row);
        near(cell(icc, row, "Lower Bound"), lower, row);
        near(cell(icc, row, "Upper Bound"), upper, row);
    }
}

#[test]
fn split_half_guttman_and_the_parallel_models_agree_with_r() {
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /MODEL=SPLIT(2).");
    // the alphas of the two parts, their numbers of items, the total, the correlation between
    // the forms, the Spearman-Brown coefficients (the manual's unequal length formula) and Guttman
    check_all(
        &last_numbers(table(&t, "Reliability Statistics")),
        &[
            0.7701897855,
            3.0,
            0.2014477799,
            2.0,
            5.0,
            0.7396859106,
            0.8503671911,
            0.8548380584,
            0.8286789333,
        ],
        "split-half",
    );
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /MODEL=GUTTMAN.");
    check_all(
        &last_numbers(table(&t, "Reliability Statistics")),
        &[
            0.6294694619,
            0.7933910969,
            0.7868368274,
            0.8286789333,
            0.7776169244,
            0.7890100662,
            5.0,
        ],
        "Guttman",
    );
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /MODEL=PARALLEL.");
    // the Box-corrected likelihood ratio chi-square of the compound symmetric covariance matrix
    let fit = last_numbers(table(&t, "Test for Goodness of Fit"));
    near(fit[0], 31.59037925, "chi-square");
    near(fit[1], 13.0, "df");
    near(fit[3], -1.141364133, "log determinant");
    near(fit[4], -0.2871261091, "constrained log determinant");
    let rel = last_numbers(table(&t, "Reliability Statistics"));
    // common variance, true variance (the mean covariance), error variance, reliability
    near(rel[0], 1.204799587, "common variance");
    near(rel[1], 0.511688288, "true variance");
    near(rel[2], 0.693111298, "error variance");
    near(rel[4], 0.7868368274, "reliability");
}

#[test]
fn omega_and_omega_if_deleted_follow_the_closed_form() {
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /MODEL=OMEGA /SUMMARY=TOTAL.");
    let rel = table(&t, "Reliability Statistics");
    near(at(rel, 0, "McDonald's Omega"), 0.7778964086, "omega");
    let items = table(&t, "Item-Total Statistics");
    for (item, omega) in [
        ("i1", 0.6818414231),
        ("i2", 0.7147474925),
        ("i3", 0.7257476711),
        ("i4", 0.7544105156),
        ("i5", 0.82924852),
    ] {
        near(
            cell(items, item, "McDonald's Omega if Item Deleted"),
            omega,
            item,
        );
    }
}

/// 24 people ranking four items 1 to 4.
const RANKS: &str = "DATA LIST FREE /r1 r2 r3 r4.
BEGIN DATA
3 2 1 4
1 3 2 4
1 2 3 4
2 3 1 4
4 2 1 3
1 4 3 2
1 3 2 4
1 3 2 4
2 1 4 3
1 2 3 4
3 2 1 4
1 3 4 2
4 3 2 1
2 1 3 4
1 3 2 4
2 4 1 3
2 1 3 4
1 4 2 3
2 1 4 3
1 2 3 4
2 3 1 4
1 3 2 4
3 2 1 4
4 3 1 2
END DATA.
";

/// 30 people on four pass (1) or fail (0) tasks.
const TASKS: &str = "DATA LIST FREE /t1 t2 t3 t4.
BEGIN DATA
1 1 1 0
1 0 0 0
1 0 1 0
1 0 0 0
0 0 0 0
0 1 0 1
1 1 1 0
1 0 0 0
1 0 0 0
1 0 0 1
0 1 0 0
1 0 1 0
0 1 1 0
0 1 0 1
1 0 1 0
1 0 0 0
0 0 1 0
0 1 1 1
1 1 1 0
1 1 1 0
0 1 1 0
1 1 1 0
1 0 0 1
0 1 1 0
0 1 0 0
1 1 0 1
1 1 1 0
1 0 1 1
1 0 0 0
1 1 0 0
END DATA.
";

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

#[test]
fn item_summaries_strict_parallel_and_one_way_icc_agree_with_r() {
    let t = run(
        "RELIABILITY VARIABLES=i1 TO i5 /SUMMARY=MEANS VARIANCE COV CORR /STATISTICS=DESCRIPTIVES.",
    );
    // mean, minimum, maximum, range, maximum over minimum, variance of the item means,
    // variances, covariances and correlations (the last two over the 20 ordered pairs)
    let summary = table(&t, "Summary Item Statistics");
    let want = [
        [
            3.297625,
            2.98175,
            4.369125,
            1.387375,
            1.465288840446,
            0.362882359375,
        ],
        [
            1.204799586538,
            0.725629166667,
            1.883146009615,
            1.157516842949,
            2.595190623698,
            0.176306973689,
        ],
        [
            0.5116882884615,
            0.1697772115385,
            0.72105,
            0.5512727884615,
            4.2470364159365,
            0.0234357121702,
        ],
        [
            0.4551442211341,
            0.1154471348402,
            0.6412279209493,
            0.5257807861091,
            5.5542991329917,
            0.0225122789644,
        ],
    ];
    for (row, want) in want.iter().enumerate() {
        for (c, name) in [
            "Mean",
            "Minimum",
            "Maximum",
            "Range",
            "Maximum / Minimum",
            "Variance",
        ]
        .iter()
        .enumerate()
        {
            near(
                at(summary, row, name),
                want[c],
                &format!("summary row {row} {name}"),
            );
        }
        near(at(summary, row, "N of Items"), 5.0, "items");
    }
    // the item means and standard deviations
    let items = table(&t, "Item Statistics");
    near(at(items, 4, "Mean"), 4.369125, "i5 mean");
    near(at(items, 4, "Std. Deviation"), 1.372277672, "i5 sd");

    // the strictly parallel model: the manual's formulas with the mean square within people
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /MODEL=STRICTPARALLEL.");
    let fit = last_numbers(table(&t, "Test for Goodness of Fit"));
    near(fit[0], 95.0789668229, "chi-square");
    near(fit[1], 17.0, "df");
    near(fit[3], -1.14136413344, "log determinant");
    near(fit[4], 1.39706136478, "constrained log determinant");
    let rel = last_numbers(table(&t, "Reliability Statistics"));
    for (got, want, what) in [
        (rel[0], 3.297625, "common mean"),
        (rel[1], 1.49510547404, "common variance"),
        (rel[2], 0.456439599038, "true variance"),
        (rel[3], 1.038665875, "error variance"),
        (rel[4], 0.293699557798, "common inter-item correlation"),
        (rel[5], 0.675234036854, "reliability"),
        (rel[6], 0.69959148409, "unbiased reliability"),
    ] {
        near(got, want, what);
    }

    // one-way random effects: 90% limits from the F quantiles, the test against 0.2
    let t = run("RELIABILITY VARIABLES=i1 TO i5 /ICC=MODEL(ONEWAY) CIN=90 TESTVAL=0.2.");
    let icc = table(&t, "Intraclass Correlation Coefficient");
    let want = [
        [
            0.298787774156,
            0.183311332251,
            0.438915140821,
            1.39133727801,
            39.0,
            160.0,
            0.0810784981852,
        ],
        [
            0.680563116169,
            0.52880953776,
            0.796388253817,
            2.50440710041,
            39.0,
            160.0,
            3.30260155552e-05,
        ],
    ];
    for (row, want) in want.iter().enumerate() {
        let got = &icc.rows[row];
        for (c, w) in want.iter().enumerate() {
            let Cell::Number(g) = got[c + 1] else {
                panic!("{got:?}")
            };
            near(g, *w, &format!("icc row {row} column {c}"));
        }
    }
}

#[test]
fn friedman_kendall_and_cochran_agree_with_r() {
    // the values are ranks: friedman.test gives 18.6 on 3 df, and W = chi-square / (n (k - 1))
    let t = run_on(
        RANKS,
        "RELIABILITY VARIABLES=r1 TO r4 /STATISTICS=ANOVA FRIEDMAN.",
    );
    let anova = table(&t, "ANOVA");
    near(at(anova, 1, "Friedman's Chi-Square"), 18.6, "Friedman");
    near(at(anova, 1, "Sig."), 0.000330721427364, "Friedman p");
    let caption = anova.caption.clone().expect("caption");
    let w: f64 = caption
        .split("W = ")
        .nth(1)
        .and_then(|s| s.trim_end_matches('.').split(". ").next())
        .and_then(|s| s.trim_end_matches('.').parse().ok())
        .expect("W in the caption");
    near(w, 0.258333333333, "Kendall's W");
    // Cochran's Q = (k - 1) (k sum C^2 - T^2) / (k T - sum R^2)
    let t = run_on(
        TASKS,
        "RELIABILITY VARIABLES=t1 TO t4 /STATISTICS=ANOVA COCHRAN.",
    );
    let anova = table(&t, "ANOVA");
    near(at(anova, 1, "Cochran's Q"), 10.8979591837, "Cochran");
    near(at(anova, 1, "Sig."), 0.0122906411447, "Cochran p");
}
