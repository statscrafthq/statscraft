//! MVA on an invented commuting survey of 36 respondents, where the travel time is more often
//! missing for long trips and the rating more often refused (99) or unknown (98) for older
//! people. Every expected number was computed independently in a general statistics package:
//! the t tests with its Welch test, the pairwise and listwise statistics with its covariance
//! functions, the EM estimates by a separate program of the manual's formulas whose maximum
//! likelihood form agrees with a published EM package to 1e-10, and Little's test, whose
//! maximum likelihood form agrees with a published implementation.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const SURVEY: &str = "DATA LIST LIST /resp district (F1) mode (A4) age km minutes rating.
BEGIN DATA
101 2 bus 37 7.9 . 6
102 2 car 48 12.2 . 8
103 1 bus 42 7.3 31 6
104 1 car 52 14 41 3
105 3 bus 20 31.5 33 7
106 1 bike 36 . . 6
107 . bike 50 11.4 32 99
108 1 bus 68 8.6 32 98
109 1 bike 42 0.9 . 98
110 2 bus 44 14.4 . 5
111 1 car 44 5.6 23 8
112 2 bike 27 4.9 16 9
113 2 bus 27 4.1 24 8
114 1 bike 49 9.5 22 9
115 1 bike 43 8.3 28 4
116 2 bus 38 13.3 . 98
117 1 bike 20 . . 4
118 2 bike 25 8.7 . 6
119 2 bus 52 13.1 48 98
120 2 bike 46 6.3 18 .
121 1 car 27 10.7 . 5
122 . car 62 9.7 . 5
123 3 bike 24 11 28 5
124 2 car 49 4 8 98
125 1 car 37 9.6 16 98
126 1 bike 33 9.9 . 5
127 3 bike 23 5.1 20 6
128 1 bus 69 6.9 29 99
129 3 car 32 11.7 . 4
130 2 na 44 3.9 19 7
131 2 bike . 12.6 . 7
132 2 bike 28 19.1 . 2
133 3 car 52 16.3 . 99
134 1 bus 46 5.9 21 8
135 2 bus 49 11.1 . 6
136 1 bus 46 4.4 23 5
END DATA.
MISSING VALUES rating (99 98) /mode ('na').
";

/// The tables of the last command and the warnings of the run.
fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let (commands, errors) = parse(&format!("{SURVEY}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    (tables, engine.warnings().to_vec())
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        Cell::Empty => f64::NAN,
        other => panic!("{other:?}"),
    }
}

/// The numbers of the row whose leading headers are `labels`.
fn row(table: &Table, labels: &[&str]) -> Vec<f64> {
    let found = table
        .rows
        .iter()
        .find(|r| {
            labels
                .iter()
                .zip(r.iter())
                .all(|(l, c)| matches!(c, Cell::Text(t) if t == l))
        })
        .unwrap_or_else(|| panic!("no row {labels:?} in {}", table.title));
    found[table.row_headers..].iter().map(num).collect()
}

/// Checks numbers to a relative tolerance; NaN stands for an empty cell.
fn close(got: &[f64], want: &[f64], tolerance: f64) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        if w.is_nan() {
            assert!(g.is_nan(), "{got:?} against {want:?}");
        } else {
            assert!(
                (g - w).abs() <= tolerance * w.abs().max(1.0),
                "{got:?} against {want:?}"
            );
        }
    }
}

const NAN: f64 = f64::NAN;
const VARS: &str =
    "MVA VARIABLES=district mode age km minutes rating /CATEGORICAL=district /ID=resp";

#[test]
fn univariate_statistics() {
    let (tables, _) = run(&format!("{VARS}."));
    assert_eq!(tables.len(), 1);
    let t = find(&tables, "Univariate Statistics");
    close(
        &row(t, &["district"]),
        &[34.0, NAN, NAN, 2.0, 5.55555555555556, NAN, NAN],
        1e-12,
    );
    close(
        &row(t, &["mode"]),
        &[35.0, NAN, NAN, 1.0, 2.77777777777778, NAN, NAN],
        1e-12,
    );
    close(
        &row(t, &["age"]),
        &[
            35.0,
            40.88571428571429,
            12.67877204244171,
            1.0,
            2.77777777777778,
            0.0,
            0.0,
        ],
        1e-12,
    );
    // the 31.5 km commute lies beyond the upper fence
    close(
        &row(t, &["km"]),
        &[
            34.0,
            9.82058823529412,
            5.52979693921470,
            2.0,
            5.55555555555556,
            0.0,
            1.0,
        ],
        1e-12,
    );
    close(
        &row(t, &["minutes"]),
        &[
            20.0,
            25.6,
            9.19038399060327,
            16.0,
            44.44444444444444,
            0.0,
            0.0,
        ],
        1e-12,
    );
    // 99 and 98 are user-missing, the one blank system-missing
    close(
        &row(t, &["rating"]),
        &[
            26.0,
            5.92307692307692,
            1.78713350196513,
            10.0,
            27.77777777777778,
            0.0,
            0.0,
        ],
        1e-12,
    );
    let (tables, _) = run(&format!("{VARS} /NOUNIVARIATE /LISTWISE."));
    assert!(tables.iter().all(|t| t.title != "Univariate Statistics"));
}

#[test]
fn separate_variance_t_tests() {
    let (tables, _) = run(&format!("{VARS} /NOUNI /TTEST PROB."));
    let t = find(&tables, "Separate Variance t Tests");
    // columns: age km minutes rating; rows only for district, km, minutes and rating (5% or more missing)
    let indicators: Vec<String> = t
        .rows
        .iter()
        .filter_map(|r| match &r[0] {
            Cell::Text(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert!(!indicators.iter().any(|s| s == "mode" || s == "age"));
    close(
        &row(t, &["rating", "t"]),
        &[
            -3.1688878076244924,
            0.579139904222902,
            -0.156205811148798,
            NAN,
        ],
        1e-10,
    );
    close(
        &row(t, &["rating", "df"]),
        &[17.27629804408264, 21.25790974499417, 7.726020185708908, NAN],
        1e-10,
    );
    close(
        &row(t, &["rating", "P(2-tail)"]),
        &[
            0.0055242943342904,
            0.568581563615939,
            0.879874960385092,
            NAN,
        ],
        1e-8,
    );
    close(
        &row(t, &["rating", "# Present"]),
        &[25.0, 24.0, 13.0, 26.0],
        0.0,
    );
    close(
        &row(t, &["rating", "# Missing"]),
        &[10.0, 10.0, 7.0, 0.0],
        0.0,
    );
    close(
        &row(t, &["rating", "Mean(Present)"]),
        &[
            37.12,
            10.145833333333334,
            25.307692307692307,
            5.92307692307692,
        ],
        1e-12,
    );
    close(
        &row(t, &["rating", "Mean(Missing)"]),
        &[50.3, 9.04, 26.142857142857142, NAN],
        1e-12,
    );
    close(
        &row(t, &["minutes", "t"]),
        &[
            1.114905913453575,
            -1.430109320545092,
            NAN,
            1.8373416056422867,
        ],
        1e-10,
    );
    close(
        &row(t, &["minutes", "df"]),
        &[32.4454310760724, 31.99866890641377, NAN, 22.742294277579937],
        1e-10,
    );
    close(
        &row(t, &["district", "t"]),
        &[-2.514309247179568, -0.588009923005304, NAN, NAN],
        1e-10,
    );
    close(
        &row(t, &["km", "P(2-tail)"]),
        &[0.322056871844989, NAN, NAN, 0.490115235368099],
        1e-8,
    );
    // NOT, NODF and NOMEANS leave the counts
    let (tables, _) = run(&format!("{VARS} /NOUNI /TTEST PERCENT=0 NOT NODF NOMEANS."));
    let t = find(&tables, "Separate Variance t Tests");
    assert_eq!(t.rows.len(), 6 * 2);
    close(&row(t, &["mode", "# Missing"]), &[1.0, 1.0, 1.0, 1.0], 0.0);
}

#[test]
fn crosstab_of_categorical_variables() {
    let (tables, _) = run(&format!("{VARS} /NOUNI /CROSSTAB PERCENT=0."));
    let t = find(&tables, "Crosstabulation of district");
    assert_eq!(&t.columns[3..], ["Total", "1", "2", "3"]);
    close(
        &row(t, &["rating", "Present", "Count"]),
        &[26.0, 11.0, 10.0, 4.0],
        0.0,
    );
    close(
        &row(t, &["rating", "Present", "Percent"]),
        &[
            72.22222222222221,
            73.33333333333333,
            71.42857142857143,
            80.0,
        ],
        1e-12,
    );
    close(
        &row(t, &["rating", "Missing", "% SysMis"]),
        &[2.77777777777778, 0.0, 7.14285714285714, 0.0],
        1e-12,
    );
    close(
        &row(t, &["rating", "Missing", "% A (99)"]),
        &[8.33333333333333, 6.66666666666667, 0.0, 20.0],
        1e-12,
    );
    close(
        &row(t, &["rating", "Missing", "% B (98)"]),
        &[16.66666666666666, 20.0, 21.42857142857143, 0.0],
        1e-12,
    );
    close(
        &row(t, &["km", "Missing", "% SysMis"]),
        &[5.55555555555556, 13.3333333333333, 0.0, 0.0],
        1e-12,
    );
    close(
        &row(t, &["mode", "Missing", "% A (na)"]),
        &[2.77777777777778, 0.0, 7.14285714285714, 0.0],
        1e-12,
    );
    // mode, a string, is categorical too
    let m = find(&tables, "Crosstabulation of mode");
    assert_eq!(&m.columns[3..], ["Total", "bike", "bus", "car"]);
    // the default PERCENT=5 leaves out mode and age
    let (tables, _) = run(&format!("{VARS} /NOUNI /CROSSTAB."));
    let t = find(&tables, "Crosstabulation of district");
    assert!(
        t.rows
            .iter()
            .all(|r| !matches!(&r[0], Cell::Text(s) if s == "age" || s == "mode"))
    );
}

#[test]
fn mismatch_and_patterns() {
    let (tables, _) = run(&format!(
        "{VARS} /NOUNI /MISMATCH PERCENT=0 /TPATTERN /MPATTERN /DPATTERN SORT=km(DESCENDING) DESCRIBE=mode."
    ));
    // variables by number of missing values: mode, age, district, km, rating, minutes
    let m = find(&tables, "Percent Mismatch of Indicator Variables");
    assert_eq!(
        &m.columns[1..],
        ["mode", "age", "district", "km", "rating", "minutes"]
    );
    close(
        &row(m, &["minutes"]),
        &[
            47.2222222222222,
            41.6666666666667,
            44.4444444444444,
            38.8888888888889,
            55.5555555555556,
            44.4444444444444,
        ],
        1e-12,
    );
    close(
        &row(m, &["district"]),
        &[
            8.33333333333333,
            8.33333333333333,
            5.55555555555556,
            11.11111111111111,
            27.77777777777778,
            44.44444444444444,
        ],
        1e-12,
    );
    let t = find(&tables, "Tabulated Patterns");
    let counts: Vec<f64> = t.rows.iter().map(|r| num(&r[0])).collect();
    assert_eq!(counts, [12.0, 1.0, 6.0, 1.0, 9.0, 1.0, 1.0, 2.0, 3.0]);
    // the last pattern: minutes and rating missing
    let marks: Vec<bool> = t.rows[8][1..]
        .iter()
        .map(|c| matches!(c, Cell::Text(x) if x == "X"))
        .collect();
    assert_eq!(marks, [false, false, false, false, true, true]);
    let mp = find(&tables, "Missing Patterns");
    assert_eq!(mp.rows.len(), 24);
    // the first case with a missing value in the pattern order is respondent 130, mode 'na' (A)
    assert_eq!(mp.rows[0][0], Cell::Text("130".into()));
    assert_eq!(mp.rows[0][3], Cell::Text("A".into()));
    let dp = find(&tables, "Data Patterns");
    assert_eq!(dp.rows.len(), 36);
    // sorted by km descending: the 31.5 km commute first, marked extremely high, then missing km last
    assert_eq!(dp.rows[0][0], Cell::Text("105".into()));
    assert_eq!(dp.rows[0][3 + 3], Cell::Text("+".into()));
    assert_eq!(dp.rows[0][3 + 6], Cell::Text("bus".into()));
    assert_eq!(dp.rows[35][3 + 3], Cell::Text("S".into()));
    // respondent 107: district system-missing, rating refused
    let r107 = dp
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text("107".into()))
        .unwrap();
    assert_eq!(
        (&r107[3], &r107[8], num(&r107[1])),
        (&Cell::Text("S".into()), &Cell::Text("A".into()), 2.0)
    );
}

#[test]
fn listwise_and_pairwise() {
    let (tables, _) = run(&format!("{VARS} /NOUNI /LISTWISE /PAIRWISE."));
    let means = find(&tables, "Listwise Means");
    close(
        &row(means, &["Listwise"]),
        &[36.9166666666667, 9.3, 25.8333333333333, 6.5],
        1e-12,
    );
    let cov = find(&tables, "Listwise Covariances");
    close(
        &row(cov, &["age"]),
        &[
            135.90151515151516,
            -26.21818181818182,
            16.34848484848485,
            -4.68181818181818,
        ],
        1e-12,
    );
    close(
        &row(cov, &["rating"]),
        &[
            -4.68181818181818,
            -2.17272727272727,
            -9.27272727272727,
            3.90909090909091,
        ],
        1e-12,
    );
    let cor = find(&tables, "Listwise Correlations");
    close(
        &row(cor, &["minutes"]),
        &[
            0.206837623564449,
            0.597525200951465,
            1.0,
            -0.691725492016921,
        ],
        1e-12,
    );
    let n = find(&tables, "Pairwise Frequencies");
    close(&row(n, &["km"]), &[33.0, 34.0, 20.0, 24.0], 0.0);
    let m = find(&tables, "Pairwise Means");
    close(
        &row(m, &["rating"]),
        &[
            37.12,
            10.14583333333333,
            25.30769230769231,
            5.92307692307692,
        ],
        1e-12,
    );
    let sd = find(&tables, "Pairwise Standard Deviations");
    close(
        &row(sd, &["minutes"]),
        &[
            13.4786380695724,
            6.16433716076424,
            9.19038399060327,
            1.89804217618158,
        ],
        1e-12,
    );
    let cov = find(&tables, "Pairwise Covariances");
    close(
        &row(cov, &["minutes"]),
        &[
            30.5368421052632,
            30.19263157894737,
            84.46315789473684,
            -8.76282051282051,
        ],
        1e-12,
    );
    let cor = find(&tables, "Pairwise Correlations");
    close(
        &row(cor, &["rating"]),
        &[
            0.0792250614501151,
            -0.2901840840551552,
            -0.6827061329780614,
            1.0,
        ],
        1e-12,
    );
}

#[test]
fn em_estimates_and_little_test() {
    let (tables, warnings) = run(&format!("{VARS} /NOUNI /EM."));
    // 44% of the travel times are missing: 25 iterations do not reach the default criterion
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("did not converge in 25")),
        "{warnings:?}"
    );
    let means = find(&tables, "EM Means");
    close(
        &row(means, &["EM"]),
        &[
            40.83433124082163,
            9.94940627431808,
            27.35856852816368,
            6.05821728910498,
        ],
        1e-10,
    );
    let cov = find(&tables, "EM Covariances");
    close(
        &row(cov, &["age"]),
        &[
            160.61053470478768,
            -15.29539643207262,
            12.3506038742759,
            0.719894865415903,
        ],
        1e-10,
    );
    close(
        &row(cov, &["minutes"]),
        &[
            12.35060387427588,
            27.89453571830445,
            72.9516875042939,
            -13.90891890952549,
        ],
        1e-10,
    );
    close(
        &row(cov, &["rating"]),
        &[
            0.719894865415903,
            -4.174372244655661,
            -13.90891890952549,
            4.249100775450195,
        ],
        1e-10,
    );
    let cor = find(&tables, "EM Correlations");
    close(
        &row(cor, &["rating"]),
        &[
            0.0275571021675068,
            -0.3653008051300661,
            -0.7900001764736061,
            1.0,
        ],
        1e-10,
    );
    let little = find(&tables, "Little's MCAR Test");
    close(
        &little.rows[0].iter().map(num).collect::<Vec<_>>(),
        &[14.856699668898692, 12.0, 0.249366882175385],
        1e-9,
    );
    // run to a tight criterion, the fixed point of the manual's (n - 1) form
    let (tables, warnings) = run(&format!(
        "{VARS} /NOUNI /EM (CONVERGENCE=1E-12 ITERATIONS=500)."
    ));
    assert!(warnings.is_empty(), "{warnings:?}");
    close(
        &row(find(&tables, "EM Means"), &["EM"]),
        &[
            40.83420240477193,
            9.94941190729913,
            27.35865849422565,
            6.05838276900118,
        ],
        1e-9,
    );
    close(
        &row(find(&tables, "EM Covariances"), &["minutes"]),
        &[
            12.328038062228009,
            27.8890634151767,
            72.8782575413723,
            -13.94075795679736,
        ],
        1e-8,
    );
}

#[test]
fn em_with_t_and_mixed_normal() {
    let (tables, _) = run(&format!("{VARS} /NOUNI /EM (TDF=4)."));
    close(
        &row(find(&tables, "EM Means"), &["EM"]),
        &[
            40.75760473289902,
            9.25536120109262,
            27.15714767573121,
            6.08685951951948,
        ],
        1e-10,
    );
    close(
        &row(find(&tables, "EM Covariances"), &["km"]),
        &[
            -1.13961495552610,
            15.00030948585032,
            21.3735807548544,
            -3.92478863288034,
        ],
        1e-9,
    );
    let (tables, _) = run(&format!("{VARS} /NOUNI /EM (LAMBDA=3 PROPORTION=0.1)."));
    close(
        &row(find(&tables, "EM Means"), &["EM"]),
        &[
            40.76374705669642,
            9.90835616485268,
            27.34690210433791,
            6.09084980394489,
        ],
        1e-10,
    );
    close(
        &row(find(&tables, "EM Covariances"), &["age"]),
        &[
            146.26666023987525,
            -13.19019302113457,
            10.63357375569781,
            1.16443268023595,
        ],
        1e-9,
    );
}

#[test]
fn em_without_missing_values_is_the_sample_moments() {
    // the 13 cases complete on the four quantitative variables
    let (tables, warnings) = run(
        "SELECT IF NOT (MISSING(age) OR MISSING(km) OR MISSING(minutes) OR MISSING(rating)).\n\
         MVA VARIABLES=age km minutes rating /NOUNI /EM.",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let means = find(&tables, "EM Means");
    close(
        &row(means, &["EM"]),
        &[
            37.46153846153846,
            8.88461538461539,
            25.30769230769231,
            6.53846153846154,
        ],
        1e-12,
    );
    assert_eq!(
        means.caption.as_deref(),
        Some("Converged after 1 iterations.")
    );
    // Little's statistic is 0 on 0 degrees of freedom, so no significance
    let little = find(&tables, "Little's MCAR Test");
    assert_eq!(
        little.rows[0],
        [Cell::Number(0.0), Cell::Number(0.0), Cell::Empty]
    );
}

#[test]
fn regression_estimates() {
    let (tables, _) = run(&format!("{VARS} /NOUNI /REGRESSION (ADDTYPE=NONE)."));
    let means = find(&tables, "Regression Means");
    close(
        &row(means, &["Regression"]),
        &[
            40.88571428571429,
            9.82058823529412,
            26.49536544081949,
            5.91212582427220,
        ],
        1e-10,
    );
    let cov = find(&tables, "Regression Covariances");
    close(
        &row(cov, &["minutes"]),
        &[
            20.51219410446344,
            25.90877455124532,
            61.35345470973419,
            -8.405843641402445,
        ],
        1e-10,
    );
    close(
        &row(cov, &["rating"]),
        &[
            -0.0401995778754777,
            -2.615784680608573,
            -8.405843641402445,
            2.605613135658935,
        ],
        1e-9,
    );
    close(
        &row(find(&tables, "Regression Correlations"), &["km"]),
        &[
            -0.1888773739720554,
            1.0,
            0.616020591386211,
            -0.3017969823414192,
        ],
        1e-10,
    );
    let (tables, _) = run(&format!(
        "{VARS} /NOUNI /REGRESSION (ADDTYPE=NONE NPREDICTORS=1 FLIMIT=0)."
    ));
    close(
        &row(find(&tables, "Regression Means"), &["Regression"]),
        &[
            40.85088742142113,
            9.87064268783663,
            26.31699244311074,
            5.90918569626134,
        ],
        1e-10,
    );
    close(
        &row(find(&tables, "Regression Covariances"), &["minutes"]),
        &[
            12.46050801692797,
            24.11373340865827,
            53.50188999781446,
            -7.912745023815468,
        ],
        1e-10,
    );
}

/// The column `name` of the LIST table.
fn listed(tables: &[Table], name: &str) -> Vec<f64> {
    let list = find(tables, "List");
    let at = list.columns.iter().position(|c| c == name).unwrap();
    let value = |c: &Cell| match c {
        Cell::Text(t) => t.trim().parse().unwrap_or(f64::NAN),
        other => num(other),
    };
    list.rows.iter().map(|r| value(&r[at])).collect()
}

#[test]
fn outfile_holds_the_filled_data() {
    let (tables, _) = run(&format!(
        "DATASET NAME survey.\nDATASET DECLARE em_filled.\nDATASET DECLARE reg_filled.\n\
         {VARS} /NOUNI /EM (OUTFILE=em_filled) /REGRESSION (ADDTYPE=NONE OUTFILE=reg_filled).\n\
         DATASET ACTIVATE em_filled.\nFORMATS minutes rating (F20.12).\nLIST resp minutes rating."
    ));
    let minutes = listed(&tables, "minutes");
    // respondents 101, 102, 106 and 109 had no travel time; 103 keeps its own
    close(
        &minutes[..9],
        &[
            25.6878788365666,
            24.5701458746970,
            31.0,
            41.0,
            33.0,
            27.1064289039195,
            32.0,
            32.0,
            18.5716442679998,
        ],
        1e-9,
    );
    let rating = listed(&tables, "rating");
    // the refusals (99) and unknowns (98) of 107 to 109 are replaced, 120 (blank) too
    close(
        &rating[6..9],
        &[5.40058258879393, 5.70393466957216, 7.31650596964441],
        1e-9,
    );
    close(&rating[19..20], &[8.04025661280033], 1e-9);
    let (tables, _) = run(&format!(
        "DATASET NAME survey.\nDATASET DECLARE reg_filled.\n\
         {VARS} /NOUNI /REGRESSION (ADDTYPE=NONE OUTFILE=reg_filled).\n\
         DATASET ACTIVATE reg_filled.\nFORMATS minutes rating (F20.12).\nLIST resp minutes rating."
    ));
    close(
        &listed(&tables, "minutes")[..2],
        &[22.5691376066355, 25.6745836832907],
        1e-9,
    );
    // 109 has every predictor of rating but minutes; 107 and 108 the same pattern
    close(
        &listed(&tables, "rating")[6..9],
        &[5.25909451133778, 5.25909451133778, 5.92307692307692],
        1e-9,
    );
}

#[test]
fn predicted_and_predictor_lists() {
    let (tables, _) = run(&format!(
        "{VARS} /EM minutes WITH km /REGRESSION rating WITH age minutes (ADDTYPE=NONE)."
    ));
    let means = find(&tables, "EM Means");
    assert_eq!(&means.columns[1..], ["minutes"]);
    close(&row(means, &["EM"]), &[26.43433760939703], 1e-10);
    close(
        &row(find(&tables, "EM Covariances"), &["minutes"]),
        &[79.7721719163439],
        1e-10,
    );
    close(
        &find(&tables, "Little's MCAR Test").rows[0]
            .iter()
            .map(num)
            .collect::<Vec<_>>(),
        &[1.753184359214458, 1.0, 0.185476914355263],
        1e-9,
    );
    let means = find(&tables, "Regression Means");
    assert_eq!(&means.columns[1..], ["rating"]);
    close(&row(means, &["Regression"]), &[5.9121258242722], 1e-10);
    close(
        &row(find(&tables, "Regression Covariances"), &["rating"]),
        &[2.60561313565894],
        1e-10,
    );
    // with two estimates, the summaries compare them with all values
    let s = find(&tables, "Summary of Estimated Means");
    close(
        &row(s, &["All Values"]),
        &[40.88571428571429, 9.82058823529412, 25.6, 5.92307692307692],
        1e-12,
    );
    close(&row(s, &["EM"]), &[NAN, NAN, 26.43433760939703, NAN], 1e-10);
    close(
        &row(
            find(&tables, "Summary of Estimated Standard Deviations"),
            &["Regression"],
        ),
        &[NAN, NAN, NAN, 2.60561313565894f64.sqrt()],
        1e-10,
    );
}

#[test]
fn random_errors_follow_the_seed() {
    let source = format!("SET SEED=4242.\n{VARS} /NOUNI /REGRESSION.");
    let (first, _) = run(&source);
    let (second, _) = run(&source);
    let means = find(&first, "Regression Means");
    assert_eq!(means.rows, find(&second, "Regression Means").rows);
    // 13 of the 36 cases are complete, fewer than half: normal errors by default
    assert!(
        means.caption.as_deref().unwrap().contains("normal errors"),
        "{:?}",
        means.caption
    );
    let (other, _) = run(&format!("SET SEED=99.\n{VARS} /NOUNI /REGRESSION."));
    assert_ne!(means.rows, find(&other, "Regression Means").rows);
    // residuals and t errors when asked for; the observed values stay
    for addtype in ["RESIDUAL", "T(3)", "NORMAL"] {
        let (tables, _) = run(&format!(
            "DATASET NAME survey.\nDATASET DECLARE out.\n{VARS} /NOUNI /REGRESSION (ADDTYPE={addtype} OUTFILE=out).\n\
             DATASET ACTIVATE out.\nFORMATS minutes (F20.12).\nLIST minutes."
        ));
        let minutes = listed(&tables, "minutes");
        assert_eq!(minutes[2], 31.0);
        assert!(minutes.iter().all(|m| m.is_finite()));
    }
}

#[test]
fn maxcat_and_errors() {
    let (tables, warnings) = run(&format!("{VARS} /MAXCAT=2."));
    assert!(tables.is_empty());
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("district has 3 categories")),
        "{warnings:?}"
    );
    let fails = |source: &str| -> String {
        let (commands, errors) = parse(&format!("{SURVEY}{source}"));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        for command in &commands[..commands.len() - 1] {
            engine.execute(&command.command).unwrap();
        }
        engine
            .execute(&commands.last().unwrap().command)
            .unwrap_err()
            .0
    };
    assert!(fails("MVA age km /CATEGORICAL=district.").contains("not on the MVA variable list"));
    assert!(fails("MVA age km /EM minutes.").contains("not on the MVA variable list"));
    assert!(
        fails("SPLIT FILE BY district.\nMVA age km /EM (OUTFILE='x.sav').")
            .contains("not supported yet")
    );
}

#[test]
fn split_files_give_a_table_per_group() {
    let (tables, _) =
        run("SORT CASES BY district.\nSPLIT FILE SEPARATE BY district.\nMVA age km minutes.");
    assert_eq!(
        tables
            .iter()
            .filter(|t| t.title.starts_with("Univariate"))
            .count(),
        4
    );
}

#[test]
fn summaries_ranges_and_categorical_only() {
    // univariate and pairwise are two ways to estimate, so the summaries appear
    let (tables, _) = run(&format!("{VARS} /PAIRWISE."));
    let s = find(&tables, "Summary of Estimated Means");
    assert_eq!(s.rows.len(), 2);
    assert_eq!(row(s, &["Pairwise"]), row(s, &["All Values"]));
    // without univariate statistics only the requested estimates are compared
    let (tables, _) = run(&format!("{VARS} /NOUNI /LISTWISE /EM."));
    let labels: Vec<&Cell> = find(&tables, "Summary of Estimated Means")
        .rows
        .iter()
        .map(|r| &r[0])
        .collect();
    assert_eq!(
        labels,
        [&Cell::Text("Listwise".into()), &Cell::Text("EM".into())]
    );
    // an estimate with nothing to estimate is skipped, the other tables stay
    let (tables, warnings) = run("SELECT IF resp = 103.\nMVA age km /TTEST PERCENT=0 /EM.");
    assert!(tables.iter().any(|t| t.title == "Univariate Statistics"));
    assert!(tables.iter().all(|t| t.title != "EM Means"));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("EM: no quantitative variable")),
        "{warnings:?}"
    );
    // a user-missing range from LOWEST is named by its keyword
    let (tables, _) = run(&format!(
        "MISSING VALUES km (LO THRU 4).\n{VARS} /NOUNI /CROSSTAB PERCENT=0."
    ));
    let t = find(&tables, "Crosstabulation of district");
    row(t, &["km", "Missing", "% A (LOWEST THRU 4)"]);
    let (tables, warnings) = run("MVA district mode /CATEGORICAL=district /TTEST.");
    assert_eq!(tables.len(), 1);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("no quantitative variable")),
        "{warnings:?}"
    );
}

#[test]
fn repeated_names_all_in_lists_and_empty_variables() {
    // a repeated name counts once, and ALL inside EM means the MVA list
    let (once, _) = run(&format!("{VARS} /NOUNI /EM."));
    let (twice, _) = run(
        "MVA VARIABLES=district mode age km minutes rating km /CATEGORICAL=district /ID=resp \
         /NOUNI /EM ALL WITH ALL.",
    );
    assert_eq!(find(&once, "EM Means").rows, find(&twice, "EM Means").rows);
    // a variable without values is left out of EM with a warning, not a convergence failure
    let (tables, warnings) = run(
        "COMPUTE empty = $SYSMIS.\nMVA VARIABLES=age km minutes rating empty /NOUNI /EM (ITERATIONS=500).",
    );
    assert_eq!(
        &find(&tables, "EM Means").columns[1..],
        ["age", "km", "minutes", "rating"]
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("empty has fewer than two")),
        "{warnings:?}"
    );
    assert!(
        !warnings.iter().any(|w| w.contains("converge")),
        "{warnings:?}"
    );
}

#[test]
fn outfile_drops_the_missing_definitions_of_filled_variables() {
    let (tables, _) = run(&format!(
        "DATASET NAME survey.\nDATASET DECLARE filled.\n{VARS} /NOUNI /EM (OUTFILE=filled).\n\
         DATASET ACTIVATE filled.\nDESCRIPTIVES rating minutes."
    ));
    let t = find(&tables, "Descriptive Statistics");
    let at = t.columns.iter().position(|c| c == "N").unwrap();
    let n: Vec<f64> = t.rows.iter().take(2).map(|r| num(&r[at])).collect();
    assert_eq!(n, [36.0, 36.0]);
}

#[test]
fn regression_skips_predictors_the_pairwise_covariances_overfit_and_npredictors_zero_is_the_mean() {
    // b is known only where a is far from its mean, so the pairwise covariance of a and b is
    // larger than the variances allow: a cannot predict b, while c, which follows b, can
    let data = "DATA LIST LIST /a b c.
BEGIN DATA
1 1 1.1
2 2 1.8
9 9 9.2
10 10 9.9
5.5 . -5
5.5 . 16
5.5 . -4
5.5 . 15
5.5 . -3
5.5 . 15
END DATA.
";
    let source = |options: &str| -> Vec<Table> {
        let (commands, errors) = parse(&format!("{data}MVA a b c /NOUNI /REGRESSION ({options})."));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        let mut tables = Vec::new();
        for command in &commands {
            tables = engine.execute(&command.command).unwrap();
        }
        tables
    };
    let means = |tables: &[Table]| row(find(tables, "Regression Means"), &["Regression"]);
    // b's valid values average 5.5; the filled ones follow c, whose deviations there sum to 0.4
    let b = means(&source("ADDTYPE=NONE FLIMIT=0"))[1];
    assert!(b > 5.505 && b < 5.6, "{b}");
    let zero = means(&source("NPREDICTORS=0"));
    close(&zero, &[5.5, 5.5, 5.6], 1e-12);
}

#[test]
fn tpattern_keeps_a_pattern_at_exactly_the_percent() {
    // 7 of 100 cases miss y: exactly 7 percent, which the limit keeps
    let rows: String = (1..=100)
        .map(|i| {
            if i <= 7 {
                format!("{i} .\n")
            } else {
                format!("{i} {i}\n")
            }
        })
        .collect();
    let (commands, errors) = parse(&format!(
        "DATA LIST LIST /x y.\nBEGIN DATA\n{rows}END DATA.\nMVA x y /NOUNI /TPATTERN PERCENT=7."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine.execute(&command.command).unwrap();
    }
    let counts: Vec<f64> = find(&tables, "Tabulated Patterns")
        .rows
        .iter()
        .map(|r| num(&r[0]))
        .collect();
    assert_eq!(counts, [93.0, 7.0]);
}

#[test]
fn empty_and_switched_off_cases_say_why() {
    // a variable with no values has no standard deviation
    let (tables, _) = run("COMPUTE none = $SYSMIS.\nMVA age none.");
    assert!(row(find(&tables, "Univariate Statistics"), &["none"])[2].is_nan());
    let (tables, warnings) =
        run("MVA district mode /CATEGORICAL=district /NOUNI /LISTWISE /PAIRWISE.");
    assert!(tables.is_empty());
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("need a quantitative variable")),
        "{warnings:?}"
    );
    let (_, warnings) = run("MVA age minutes /NOUNI /TTEST NOT NODF NOCOUNTS NOMEANS.");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("every statistic is turned off")),
        "{warnings:?}"
    );
}
