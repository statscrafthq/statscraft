//! EXAMINE statistics against R on 25 invented values (two outliers, two ties): the
//! descriptives from base R (mean, trimmed mean with the fractional end weights of the manual,
//! the bias-adjusted skewness and kurtosis with their standard errors), the five percentile
//! definitions of the Algorithms manual (type 6, 4, 1 and 2 of quantile() for HAVERAGE,
//! WAVERAGE, EMPIRICAL and AEMPIRICAL; ROUND is the observation nearest to W p, written out),
//! Tukey's hinges (fivenum) and the four M-estimators by the manual's iteration (start at the
//! median, scale the median absolute deviation, stop at a relative change of 0.005 or after 30
//! steps). The same values given as counts with frequency weights must give the same results.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const VALUES: &str = "DATA LIST FREE /x.
BEGIN DATA
43.9 43.5 48.9 47.8 53.5 40.5 59.5 49.9 48 47.1 60.2 46.2 50.6 47.9 64.8 43.3 49.4 29.1 57.1 44.3
95.5 12.3 50 48.9 49.9
END DATA.
";

/// The same 25 values as 23 distinct ones with counts.
const COUNTS: &str = "DATA LIST FREE /x w.
BEGIN DATA
12.3 1 29.1 1 40.5 1 43.3 1 43.5 1 43.9 1 44.3 1 46.2 1 47.1 1 47.8 1 47.9 1 48 1
48.9 2 49.4 1 49.9 2 50 1 50.6 1 53.5 1 57.1 1 59.5 1 60.2 1 64.8 1 95.5 1
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

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The numbers of the row of the table `title` that has `label` among its text cells.
fn numbers(tables: &[Table], title: &str, label: &str) -> Vec<f64> {
    let t = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let row = t
        .rows
        .iter()
        .find(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
        .unwrap_or_else(|| panic!("no row {label} in {title}"));
    row.iter()
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

const STATISTICS: &str = "EXAMINE VARIABLES=x /PERCENTILES(5,10,25,50,75,90,95)=@METHOD@ \
    /STATISTICS=DESCRIPTIVES /MESTIMATORS=HUBER TUKEY HAMPEL ANDREW /PLOT=NONE.";

fn check(data: &str) {
    let t = run(data, &STATISTICS.replace("@METHOD@", "HAVERAGE"));
    let d = "Descriptives";
    near(numbers(&t, d, "Mean")[0], 49.284, "mean");
    near(numbers(&t, d, "Mean")[1], 2.79221345889, "SE of the mean");
    near(numbers(&t, d, "Lower Bound")[0], 43.5211546584, "lower");
    near(numbers(&t, d, "Upper Bound")[0], 55.0468453416, "upper");
    near(
        numbers(&t, d, "5% Trimmed Mean")[0],
        48.9255555556,
        "trimmed mean",
    );
    near(numbers(&t, d, "Median")[0], 48.9, "median");
    near(numbers(&t, d, "Variance")[0], 194.9114, "variance");
    near(numbers(&t, d, "Std. Deviation")[0], 13.9610672944, "sd");
    near(numbers(&t, d, "Interquartile Range")[0], 7.95, "IQR");
    near(numbers(&t, d, "Skewness")[0], 0.751422312065, "skewness");
    near(
        numbers(&t, d, "Skewness")[1],
        0.463683500801,
        "SE of skewness",
    );
    near(numbers(&t, d, "Kurtosis")[0], 6.019017896, "kurtosis");
    near(
        numbers(&t, d, "Kurtosis")[1],
        0.901720546253,
        "SE of kurtosis",
    );
    let m = numbers(&t, "M-Estimators", "x");
    for (got, want, what) in [
        (m[0], 48.5645859075, "Huber"),
        (m[1], 48.8272633444, "Tukey"),
        (m[2], 48.801017127, "Hampel"),
        (m[3], 48.8535609698, "Andrews"),
    ] {
        near(got, want, what);
    }
    let p = "Percentiles";
    let hinges = numbers(&t, p, "Tukey's Hinges");
    for (got, want) in hinges.iter().zip([44.3, 48.9, 50.6]) {
        near(*got, want, "hinge");
    }
    for (method, label, want) in [
        (
            "HAVERAGE",
            "Weighted Average at (W+1)p",
            [17.34, 35.94, 44.1, 48.9, 52.05, 62.04, 86.29],
        ),
        (
            "WAVERAGE",
            "Weighted Average at Wp",
            [16.5, 34.8, 44.0, 48.45, 50.45, 59.85, 63.65],
        ),
        (
            "ROUND",
            "Closest Observation",
            [12.3, 40.5, 43.9, 48.9, 50.6, 60.2, 64.8],
        ),
        (
            "EMPIRICAL",
            "Empirical Distribution Function",
            [29.1, 40.5, 44.3, 48.9, 50.6, 60.2, 64.8],
        ),
        (
            "AEMPIRICAL",
            "Empirical Distribution Function with Averaging",
            [29.1, 40.5, 44.3, 48.9, 50.6, 60.2, 64.8],
        ),
    ] {
        let t = run(
            data,
            &STATISTICS
                .replace("@METHOD@", method)
                .replace("/STATISTICS=DESCRIPTIVES", "/STATISTICS=NONE"),
        );
        let got = numbers(&t, p, label);
        assert_eq!(got.len(), 7, "{method}");
        for (got, want) in got.iter().zip(want) {
            near(*got, want, method);
        }
    }
}

#[test]
fn examine_agrees_with_r() {
    check(VALUES);
}

#[test]
fn frequency_weights_give_the_results_of_the_repeated_values() {
    check(COUNTS);
}

/// Small samples, where the percentile definitions meet the ends of the data and the
/// M-estimators run on a handful of values, with constants other than the defaults.
/// Expected values from R: quantile() types 6, 4, 1, 2, fivenum(), and the manual's iteration
/// written out (median start, median absolute deviation, relative step of 0.005, 30 steps).
/// values, the three hinges, the four percentile rows, the five M-estimates, the
/// trimmed mean.
type Small = (&'static str, [f64; 3], [[f64; 7]; 4], [f64; 5], f64);

const SMALL: [Small; 3] = [
    (
        "3.2 7.1 7.1 10.4 15.0 22.8",
        [7.1, 8.75, 15.0],
        [
            [3.2, 3.2, 6.125, 8.75, 16.95, 22.8, 22.8],
            [3.2, 3.2, 5.15, 7.1, 12.7, 18.12, 20.46],
            [3.2, 3.2, 7.1, 7.1, 15.0, 22.8, 22.8],
            [3.2, 3.2, 7.1, 8.75, 15.0, 22.8, 22.8],
        ],
        [
            9.39216504288,
            10.1422714665,
            7.85289331795,
            9.56977296296,
            9.24120681544,
        ],
        10.7037037037,
    ),
    (
        "2.5 4.0 9.5 31.0",
        [3.25, 6.75, 20.25],
        [
            [2.5, 2.5, 2.875, 6.75, 25.625, 31.0, 31.0],
            [2.5, 2.5, 2.5, 4.0, 9.5, 22.4, 26.7],
            [2.5, 2.5, 2.5, 4.0, 9.5, 31.0, 31.0],
            [2.5, 2.5, 3.25, 6.75, 20.25, 31.0, 31.0],
        ],
        [
            6.75,
            7.89890448413,
            4.88521417796,
            5.33333333333,
            5.22696925321,
        ],
        11.1944444444,
    ),
    (
        "1 2 3 4 5 6 7 8 9 40",
        [3.0, 5.5, 8.0],
        [
            [1.0, 1.1, 2.75, 5.5, 8.25, 36.9, 40.0],
            [1.0, 1.0, 2.5, 5.0, 7.5, 9.0, 24.5],
            [1.0, 1.0, 3.0, 5.0, 8.0, 9.0, 40.0],
            [1.0, 1.5, 3.0, 5.5, 8.0, 24.5, 40.0],
        ],
        [
            5.5,
            5.61107734654,
            5.01218189108,
            5.00336959225,
            5.00194168488,
        ],
        7.16666666667,
    ),
];

#[test]
fn small_samples_agree_with_r() {
    for (values, hinges, methods, mest, trimmed) in SMALL {
        let data = format!("DATA LIST FREE /x.\nBEGIN DATA\n{values}\nEND DATA.\n");
        let p = "Percentiles";
        for (method, label, want) in [
            ("HAVERAGE", "Weighted Average at (W+1)p", methods[0]),
            ("WAVERAGE", "Weighted Average at Wp", methods[1]),
            ("EMPIRICAL", "Empirical Distribution Function", methods[2]),
            (
                "AEMPIRICAL",
                "Empirical Distribution Function with Averaging",
                methods[3],
            ),
        ] {
            let t = run(
                &data,
                &format!(
                    "EXAMINE VARIABLES=x /PERCENTILES(5,10,25,50,75,90,95)={method} \
                     /STATISTICS=NONE /PLOT=NONE."
                ),
            );
            let got = numbers(&t, p, label);
            assert_eq!(got.len(), 7, "{method} of {values}");
            for (got, want) in got.iter().zip(want) {
                near(*got, want, &format!("{method} of {values}"));
            }
        }
        // one constant per estimator: Huber with 2.2 here, with 1 in a run of its own
        let t = run(
            &data,
            "EXAMINE VARIABLES=x /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=NONE \
             /MESTIMATORS=HUBER(2.2) TUKEY(3) HAMPEL(1.5,3,6) ANDREW(1.5) /PLOT=NONE.",
        );
        let got = numbers(&t, p, "Tukey's Hinges");
        assert_eq!(got.len(), 3, "hinges of {values}");
        for (got, want) in got.iter().zip(hinges) {
            near(*got, want, "hinge");
        }
        let d = run(
            &data,
            "EXAMINE VARIABLES=x /STATISTICS=DESCRIPTIVES /PLOT=NONE.",
        );
        near(
            numbers(&d, "Descriptives", "5% Trimmed Mean")[0],
            trimmed,
            &format!("trimmed mean of {values}"),
        );
        let one = run(
            &data,
            "EXAMINE VARIABLES=x /STATISTICS=NONE /MESTIMATORS=HUBER(1) /PLOT=NONE.",
        );
        near(
            numbers(&one, "M-Estimators", "x")[0],
            mest[0],
            &format!("Huber(1) of {values}"),
        );
        let mest = &mest[1..];
        let got = numbers(&t, "M-Estimators", "x");
        assert_eq!(got.len(), 4, "M-estimators of {values}");
        for (got, want) in got.iter().zip(mest) {
            near(*got, *want, &format!("M-estimator of {values}"));
        }
    }
}
