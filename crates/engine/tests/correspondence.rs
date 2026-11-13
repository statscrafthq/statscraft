//! CORRESPONDENCE on synthetic data. The expected numbers come from an independent
//! computation in a general statistics package: its correspondence analysis routine for the
//! singular values and standard scores, a singular value decomposition of the standardized
//! table written out from the formulas for the masses, inertias and contributions, the
//! transition formulas on profiles for supplementary points, and the delta method with
//! central differences of the whole solution for the standard deviations and correlations.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn tables(source: &str) -> Vec<Table> {
    run(&mut Engine::default(), &format!("{DATA}{source}"))
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let keys: Vec<String> = table
        .rows
        .iter()
        .map(|r| match &r[0] {
            Cell::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect();
    let r = keys
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {keys:?}"));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    &table.rows[r][c]
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn check(tables: &[Table], title: &str, row: &str, column: &str, expected: f64, tolerance: f64) {
    let actual = number(cell(tables, title, row, column));
    assert!(
        (actual - expected).abs() <= tolerance,
        "{title} / {row} / {column}: {actual} against {expected}"
    );
}

/// Commuters by home district (1 to 5) and main way to work (1 to 4), as cell counts with a
/// weight. District 6 and way 5 are extra categories for SUPPLEMENTARY; district 7 lies
/// outside every range used, and one case has a missing way.
const DATA: &str = "DATA LIST FREE /district way n.
BEGIN DATA.
1 1 23 1 2 11 1 3 9 1 4 30 1 5 4
2 1 14 2 2 26 2 3 18 2 4 8 2 5 10
3 1 7 3 2 19 3 3 31 3 4 12 3 5 7
4 1 20 4 2 9 4 3 15 4 4 25 4 5 3
5 1 11 5 2 17 5 3 6 5 4 22 5 5 6
6 1 5 6 2 8 6 3 12 6 4 9 6 5 2
7 1 40 7 2 1 3 . 50
END DATA.
WEIGHT BY n.
";

/// A row label, its mass, scores, inertia, shares of the dimensions and of the point.
type RowPoint = (&'static str, f64, [f64; 2], f64, [f64; 2], [f64; 2]);

/// A STANDARDIZE keyword, its singular values, the scores of row 1 and column 3, the standard
/// deviations of row 5 and its mass.
type EuclidCase = (&'static str, [f64; 3], [f64; 2], [f64; 2], [f64; 2], f64);

const BASE: &str = "CORRESPONDENCE TABLE=district(1,5) BY way(1,4)";

/// Singular values, inertia, chi-square and proportions; the scores and contributions of the
/// symmetrical normalization; the confidence statistics.
#[test]
fn summary_points_and_confidence() {
    let t = tables(&format!(
        "{BASE} /PRINT=DEFAULT RPROFILES CPROFILES RCONF CCONF."
    ));
    let s = "Summary";
    for (dim, sv, inertia, share, sd) in [
        (
            "1",
            0.369471436209,
            0.136509142174,
            0.754362556331,
            0.0504195650734,
        ),
        (
            "2",
            0.182901762075,
            0.0334530545701,
            0.184864774334,
            0.0569473297307,
        ),
        (
            "3",
            0.104868477577,
            0.0109973975894,
            0.0607726693346,
            0.0518609046333,
        ),
    ] {
        check(&t, s, dim, "Singular Value", sv, 1e-10);
        check(&t, s, dim, "Inertia", inertia, 1e-10);
        check(
            &t,
            s,
            dim,
            "Proportion of Inertia / Accounted for",
            share,
            1e-10,
        );
        check(
            &t,
            s,
            dim,
            "Confidence Singular Value / Standard Deviation",
            sd,
            1e-8,
        );
    }
    check(
        &t,
        s,
        "1",
        "Confidence Singular Value / Correlation / 2",
        0.017905366603,
        1e-7,
    );
    check(&t, s, "Total", "Inertia", 0.180959594334, 1e-10);
    check(&t, s, "Total", "Chi Square", 60.2595449131, 1e-8);
    check(&t, s, "Total", "Sig.", 2.02427410742e-08, 1e-15);
    check(
        &t,
        s,
        "2",
        "Proportion of Inertia / Cumulative",
        0.939227330665,
        1e-10,
    );

    let r = "Overview Row Points";
    let rows: [RowPoint; 5] = [
        (
            "1",
            0.219219219219,
            [-0.686051631758, -0.0851284333804],
            0.0387617405149,
            [0.2792616894854, 0.00868580388729],
            [0.983489728979, 0.00749622353297],
        ),
        (
            "2",
            0.198198198198,
            [0.576255552842, 0.5472544908234],
            0.0384426413408,
            [0.1781349268958, 0.32453420784412],
            [0.632553987282, 0.28241193076784],
        ),
        (
            "3",
            0.207207207207,
            [0.846579206526, -0.4507948397615],
            0.0638777986221,
            [0.4019381071197, 0.23022094901419],
            [0.858956122371, 0.12056761717974],
        ),
        (
            "4",
            0.207207207207,
            [-0.397505955202, -0.4058026937484],
            0.0190614659272,
            [0.0886158213634, 0.18655926364747],
            [0.634624315021, 0.32741328768630],
        ),
        (
            "5",
            0.168168168168,
            [-0.338163423404, 0.5214430188324],
            0.0208159479286,
            [0.0520494551357, 0.24999977560692],
            [0.341335715077, 0.40177157267093],
        ),
    ];
    for (row, mass, score, inertia, of_point, of_dim) in rows {
        check(&t, r, row, "Mass", mass, 1e-10);
        check(&t, r, row, "Inertia", inertia, 1e-10);
        for d in 0..2 {
            let k = (d + 1).to_string();
            check(
                &t,
                r,
                row,
                &format!("Score in Dimension / {k}"),
                score[d],
                1e-9,
            );
            let point = format!("Contribution / Of Point to Inertia of Dimension / {k}");
            check(&t, r, row, &point, of_point[d], 1e-9);
            let dim = format!("Contribution / Of Dimension to Inertia of Point / {k}");
            check(&t, r, row, &dim, of_dim[d], 1e-9);
        }
        let total = "Contribution / Of Dimension to Inertia of Point / Total";
        check(&t, r, row, total, of_dim[0] + of_dim[1], 1e-9);
    }
    check(&t, r, "Active Total", "Mass", 1.0, 1e-12);
    check(&t, r, "Active Total", "Inertia", 0.180959594334, 1e-10);

    let c = "Overview Column Points";
    for (col, score, of_point, of_dim) in [
        (
            "1",
            [-0.485575547039, 0.0122371234139],
            [0.143730778313, 0.000184398685324],
            [0.738599633652, 0.000232215482683],
        ),
        (
            "3",
            [0.769163133423, -0.5431841275985],
            [0.379873421983, 0.382700570947263],
            [0.801973000589, 0.197994893749467],
        ),
    ] {
        for d in 0..2 {
            let k = (d + 1).to_string();
            check(
                &t,
                c,
                col,
                &format!("Score in Dimension / {k}"),
                score[d],
                1e-9,
            );
            let point = format!("Contribution / Of Point to Inertia of Dimension / {k}");
            check(&t, c, col, &point, of_point[d], 1e-9);
            let dim = format!("Contribution / Of Dimension to Inertia of Point / {k}");
            check(&t, c, col, &dim, of_dim[d], 1e-9);
        }
    }
    check(&t, c, "4", "Mass", 0.291291291291, 1e-10);
    check(&t, c, "2", "Inertia", 0.0399302156582, 1e-10);

    let rc = "Confidence Row Points";
    for (row, sd, cor) in [
        ("1", [0.131698938419, 0.194134542583], -0.0260291004901),
        ("2", [0.19852247652, 0.286482123259], -0.312630193389),
        ("5", [0.222074941276, 0.373931784298], 0.270342008267),
    ] {
        check(
            &t,
            rc,
            row,
            "Standard Deviation in Dimension / 1",
            sd[0],
            1e-7,
        );
        check(
            &t,
            rc,
            row,
            "Standard Deviation in Dimension / 2",
            sd[1],
            1e-7,
        );
        check(&t, rc, row, "Correlation / 1-2", cor, 1e-6);
    }
    let cc = "Confidence Column Points";
    for (col, sd, cor) in [
        ("2", [0.202600712909, 0.100006211168], -0.68236501209),
        ("3", [0.164627810788, 0.126951860764], 0.838351986537),
    ] {
        check(
            &t,
            cc,
            col,
            "Standard Deviation in Dimension / 1",
            sd[0],
            1e-7,
        );
        check(
            &t,
            cc,
            col,
            "Standard Deviation in Dimension / 2",
            sd[1],
            1e-7,
        );
        check(&t, cc, col, "Correlation / 1-2", cor, 1e-6);
    }

    // the input table and the profiles
    check(&t, "Correspondence Table", "3", "way / 3", 31.0, 0.0);
    check(&t, "Correspondence Table", "3", "Active Margin", 69.0, 0.0);
    check(
        &t,
        "Correspondence Table",
        "Active Margin",
        "Active Margin",
        333.0,
        0.0,
    );
    check(&t, "Row Profiles", "1", "way / 4", 30.0 / 73.0, 1e-12);
    check(&t, "Row Profiles", "Mass", "way / 4", 97.0 / 333.0, 1e-12);
    check(&t, "Column Profiles", "2", "way / 2", 26.0 / 82.0, 1e-12);
    check(&t, "Column Profiles", "2", "Mass", 66.0 / 333.0, 1e-12);
}

#[test]
fn normalizations_scale_scores_and_deviations() {
    // principal: both sides carry the singular values
    let t = tables(&format!(
        "{BASE} /NORMALIZATION=PRINCIPAL /PRINT=RPOINTS CPOINTS RCONF CCONF /PLOT=NONE."
    ));
    check(
        &t,
        "Overview Row Points",
        "1",
        "Score in Dimension / 1",
        -0.417010735931,
        1e-9,
    );
    check(
        &t,
        "Overview Row Points",
        "5",
        "Score in Dimension / 2",
        0.2230056170518,
        1e-9,
    );
    check(
        &t,
        "Overview Column Points",
        "3",
        "Score in Dimension / 2",
        -0.23230364042285,
        1e-9,
    );
    check(
        &t,
        "Confidence Row Points",
        "4",
        "Standard Deviation in Dimension / 2",
        0.0871978257119,
        1e-7,
    );
    check(
        &t,
        "Confidence Column Points",
        "1",
        "Standard Deviation in Dimension / 1",
        0.09135491688,
        1e-7,
    );
    // contributions do not depend on the normalization
    check(
        &t,
        "Overview Row Points",
        "3",
        "Contribution / Of Point to Inertia of Dimension / 1",
        0.4019381071197,
        1e-9,
    );
    // row principal: the columns keep their standard scores
    let t = tables(&format!("{BASE} /NORMALIZATION=RPRINCIPAL /PRINT=CPOINTS."));
    check(
        &t,
        "Overview Column Points",
        "2",
        "Score in Dimension / 2",
        1.5496009726630,
        1e-9,
    );
    check(
        &t,
        "Overview Column Points",
        "4",
        "Score in Dimension / 1",
        -1.064520263570,
        1e-9,
    );
    // a value: rows get the power 0.7 of each singular value, columns 0.3
    let t = tables(&format!(
        "{BASE} /NORMALIZATION=0.4 /PRINT=RPOINTS CPOINTS."
    ));
    let x = -1.128668403192 * 0.369471436209_f64.powf(0.7);
    check(
        &t,
        "Overview Row Points",
        "1",
        "Score in Dimension / 1",
        x,
        1e-9,
    );
    let y = 1.5496009726630 * 0.182901762075_f64.powf(0.3);
    check(
        &t,
        "Overview Column Points",
        "2",
        "Score in Dimension / 2",
        y,
        1e-9,
    );
}

#[test]
fn supplementary_points_are_projected() {
    let t = tables(
        "CORRESPONDENCE TABLE=district(1,6) BY way(1,5) /SUPPLEMENTARY district(6) way(5) /PRINT=TABLE RPOINTS CPOINTS.",
    );
    // the active part is the base table
    check(&t, "Summary", "1", "Singular Value", 0.369471436209, 1e-10);
    let r = "Overview Row Points";
    check(&t, r, "6", "Score in Dimension / 1", 0.37627237856, 1e-9);
    check(&t, r, "6", "Score in Dimension / 2", -0.370027306692, 1e-9);
    check(&t, r, "6", "Mass", 0.102102102102, 1e-10);
    check(&t, r, "6", "Inertia", 0.00882900068407, 1e-10);
    check(
        &t,
        r,
        "6",
        "Contribution / Of Point to Inertia of Dimension / 1",
        0.0,
        0.0,
    );
    check(
        &t,
        r,
        "6",
        "Contribution / Of Dimension to Inertia of Point / 1",
        0.604934953964,
        1e-9,
    );
    check(
        &t,
        r,
        "6",
        "Contribution / Of Dimension to Inertia of Point / 2",
        0.289606677891,
        1e-9,
    );
    check(&t, r, "Active Total", "Mass", 1.0, 1e-12);
    let c = "Overview Column Points";
    check(&t, c, "5", "Score in Dimension / 1", 0.51631461237, 1e-9);
    check(&t, c, "5", "Score in Dimension / 2", 0.708525582992, 1e-9);
    check(&t, c, "5", "Mass", 0.0900900900901, 1e-10);
    check(
        &t,
        c,
        "5",
        "Contribution / Of Dimension to Inertia of Point / 2",
        0.481799399235,
        1e-9,
    );
    check(&t, c, "1", "Score in Dimension / 1", -0.485575547039, 1e-9);
    // margins leave the supplementary row and column out
    check(
        &t,
        "Correspondence Table",
        "Active Margin",
        "Active Margin",
        333.0,
        0.0,
    );
    check(&t, "Correspondence Table", "6", "Active Margin", 34.0, 0.0);
    check(
        &t,
        "Correspondence Table",
        "Active Margin",
        "way / 5",
        30.0,
        0.0,
    );
}

#[test]
fn equal_rows_share_their_scores() {
    let t = tables(&format!(
        "{BASE} /EQUAL district(2,4) /PRINT=RPOINTS CPOINTS RCONF."
    ));
    let s = "Summary";
    check(&t, s, "1", "Singular Value", 0.328170724902, 1e-10);
    check(&t, s, "3", "Singular Value", 0.0798991737693, 1e-10);
    check(&t, s, "Total", "Chi Square", 42.6751413951, 1e-8);
    check(
        &t,
        s,
        "2",
        "Confidence Singular Value / Standard Deviation",
        0.0536085671436,
        1e-8,
    );
    let r = "Overview Row Points";
    check(&t, r, "2", "Mass", 0.405405405405, 1e-10);
    for row in ["2", "4"] {
        check(&t, r, row, "Score in Dimension / 1", 0.0634249237087, 1e-9);
        check(&t, r, row, "Score in Dimension / 2", -0.1231911617151, 1e-9);
    }
    assert_eq!(cell(&t, r, "4", "Mass"), &Cell::Empty);
    check(
        &t,
        r,
        "5",
        "Contribution / Of Point to Inertia of Dimension / 2",
        0.730563404818700,
        1e-9,
    );
    check(
        &t,
        "Overview Column Points",
        "3",
        "Score in Dimension / 1",
        0.887129336029,
        1e-9,
    );
    check(
        &t,
        "Confidence Row Points",
        "5",
        "Standard Deviation in Dimension / 1",
        0.206651259062,
        1e-7,
    );
    check(
        &t,
        "Confidence Row Points",
        "5",
        "Correlation / 1-2",
        0.441289114874,
        1e-6,
    );
}

#[test]
fn euclidean_measure_with_each_standardization() {
    // (keyword, singular values, row 1 scores, column 3 scores, row 5 sd, row 5 masses)
    let cases: [EuclidCase; 4] = [
        (
            "RMEAN",
            [0.376860992019, 0.180633590781, 0.134099470925],
            [0.768254088091, -0.0814059573415],
            [-0.719409177684, -0.5412085036783],
            [0.251472231199, 0.674301202121],
            0.168168168168,
        ),
        (
            "CMEAN",
            [0.37589970903, 0.189685489507, 0.114826176051],
            [-0.749407974459, -0.146532292169],
            [0.758050226086, -0.674549809783],
            [0.212946314911, 0.337141657239],
            0.2,
        ),
        (
            "RSUM",
            [0.371983813185, 0.183913104898, 0.139719971941],
            [0.772920095422, -0.093659221058],
            [-0.726615744546, -0.5265518105696],
            [0.253516408905, 0.632505987908],
            0.2,
        ),
        (
            "CSUM",
            [0.374598657443, 0.191929054705, 0.118927700101],
            [-0.744232843169, -0.166715986763],
            [0.770873752582, -0.644648590038],
            [0.210842609878, 0.351386456345],
            0.2,
        ),
    ];
    for (keyword, sv, row1, col3, sd5, mass5) in cases {
        let t = tables(&format!(
            "{BASE} /MEASURE=EUCLID /STANDARDIZE={keyword} /PRINT=RPOINTS CPOINTS RCONF."
        ));
        for (d, value) in sv.iter().enumerate() {
            check(
                &t,
                "Summary",
                &(d + 1).to_string(),
                "Singular Value",
                *value,
                1e-10,
            );
        }
        assert_eq!(cell(&t, "Summary", "Total", "Chi Square"), &Cell::Empty);
        let r = "Overview Row Points";
        check(&t, r, "1", "Score in Dimension / 1", row1[0], 1e-9);
        check(&t, r, "1", "Score in Dimension / 2", row1[1], 1e-9);
        check(&t, r, "5", "Mass", mass5, 1e-10);
        let c = "Overview Column Points";
        check(&t, c, "3", "Score in Dimension / 1", col3[0], 1e-9);
        check(&t, c, "3", "Score in Dimension / 2", col3[1], 1e-9);
        let rc = "Confidence Row Points";
        check(
            &t,
            rc,
            "5",
            "Standard Deviation in Dimension / 1",
            sd5[0],
            1e-7,
        );
        check(
            &t,
            rc,
            "5",
            "Standard Deviation in Dimension / 2",
            sd5[1],
            1e-7,
        );
    }
    // the default standardization of the Euclidean measure is the chi-square one
    let t = tables(&format!("{BASE} /MEASURE=EUCLID /PRINT=RPOINTS."));
    check(&t, "Summary", "1", "Singular Value", 0.369471436209, 1e-10);
}

#[test]
fn table_read_as_cases() {
    let source = "DATA LIST FREE /ROWCAT_ bus bike car walk.
BEGIN DATA.
11 23 11 9 30
12 14 26 18 8
13 7 19 31 12
14 20 9 15 25
15 11 17 6 22
16 1 1 1 1
END DATA.
VALUE LABELS ROWCAT_ 11 'north' 15 'south'.
CORRESPONDENCE TABLE=ALL(5,4) /SUPPLEMENTARY COLUMN(4) /PRINT=TABLE RPOINTS CPOINTS.
CORRESPONDENCE TABLE=ALL(5,4).
";
    let t = run(&mut Engine::default(), source);
    // the first run: walk is supplementary; the second is the base table
    let second: Vec<Table> = t[t.len() - 4..].to_vec();
    check(
        &second,
        "Summary",
        "1",
        "Singular Value",
        0.369471436209,
        1e-10,
    );
    check(
        &second,
        "Overview Row Points",
        "north",
        "Score in Dimension / 1",
        -0.686051631758,
        1e-9,
    );
    check(
        &second,
        "Overview Column Points",
        "car",
        "Score in Dimension / 2",
        -0.5431841275985,
        1e-9,
    );
    check(
        &second,
        "Correspondence Table",
        "south",
        "Column / bike",
        17.0,
        0.0,
    );
    let first = &t[..4];
    check(
        first,
        "Correspondence Table",
        "north",
        "Active Margin",
        43.0,
        0.0,
    );
    assert!(matches!(
        cell(first, "Overview Column Points", "walk", "Mass"),
        Cell::Number(_)
    ));
}

#[test]
fn permutation_orders_by_scores() {
    let t = tables(&format!("{BASE} /PRINT=PERMUTATION(2) /PLOT=NONE."));
    let permuted: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "Permuted Correspondence Table")
        .collect();
    assert_eq!(permuted.len(), 2);
    let order: Vec<String> = permuted[0]
        .rows
        .iter()
        .map(|r| match &r[0] {
            Cell::Text(s) => s.clone(),
            _ => String::new(),
        })
        .collect();
    // dimension 1 row scores: 1 -0.686, 4 -0.398, 5 -0.338, 2 0.576, 3 0.847
    assert_eq!(order, ["1", "4", "5", "2", "3", "Active Margin"]);
    assert_eq!(
        permuted[0].columns[1..5],
        ["4", "1", "2", "3"].map(String::from)
    );
}

#[test]
fn plots_as_data() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!("{DATA}{BASE} /DIMENSION=3 /PLOT=BIPLOT(1) RPOINTS TRCOLUMNS NDIM(1,MAX)."),
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Row and Column Points",
            "Row and Column Points",
            "Row Points",
            "Row Points",
            "way, Dimension 1",
            "way, Dimension 2",
            "way, Dimension 3"
        ]
    );
    let Some(statscraft_engine::plot::Plot::Points { groups, .. }) = &charts[1].plot else {
        panic!("points");
    };
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].points[0].label.as_deref(), Some("1"));
    assert_eq!(charts[1].value_axis.title, "Dimension 3");
    assert!((groups[1].points[2].x - 0.769163133423).abs() < 1e-9);
    // principal: no biplot
    run(&mut engine, &format!("{BASE} /NORMALIZATION=PRINCIPAL."));
    assert!(engine.take_charts().is_empty());
    assert!(engine.warnings().iter().any(|w| w.contains("no biplot")));
}

#[test]
fn outfile_writes_scores_and_variances() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}DATASET DECLARE sc.\nDATASET DECLARE va.\n{BASE} /PRINT=NONE /PLOT=NONE /OUTFILE=SCORE(sc) VARIANCE(va).\nDATASET ACTIVATE sc.\n"
        ),
    );
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 9);
    let Values::Numeric(dim1) = data.variable("DIM1").unwrap().values() else {
        panic!("numeric");
    };
    assert!((dim1.get(0).unwrap() - -0.686051631758).abs() < 1e-9);
    assert!((dim1.get(7).unwrap() - 0.769163133423).abs() < 1e-9);
    run(&mut engine, "DATASET ACTIVATE va.\n");
    let data = engine.dataset().unwrap();
    // 2 rows for the singular values, 2 for each of 5 rows and 4 columns
    assert_eq!(data.case_count(), 20);
    let Values::Numeric(dim1) = data.variable("DIM1").unwrap().values() else {
        panic!("numeric");
    };
    assert!((dim1.get(0).unwrap() - 0.0504195650734_f64.powi(2)).abs() < 1e-9);
    assert!((dim1.get(2).unwrap() - 0.131698938419_f64.powi(2)).abs() < 1e-8);
}

#[test]
fn errors_and_warnings() {
    assert!(
        run_error("CORRESPONDENCE TABLE=district(1,1) BY way(1,4).").contains("two active rows")
    );
    assert!(
        run_error(&format!("{BASE} /SUPPLEMENTARY district(9).")).contains("not a category value")
    );
    assert!(
        run_error(&format!(
            "{BASE} /SUPPLEMENTARY district(2) /EQUAL district(2,3)."
        ))
        .contains("supplementary")
    );
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}{BASE} /DIMENSION=5 /PLOT=NONE."),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("at most 3")));
    let summary = t.iter().find(|t| t.title == "Summary").unwrap();
    // three dimensions and the total
    assert_eq!(summary.rows.len(), 4);
    let overview = t.iter().find(|t| t.title == "Overview Row Points").unwrap();
    assert!(overview.columns.iter().filter(|c| *c == "3").count() >= 2);
}
