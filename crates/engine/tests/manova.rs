//! MANOVA on synthetic data. The expected numbers come from a general statistics package used
//! as a black box: multivariate tests of each effect entered last into its multivariate linear
//! model (all four statistics with their F approximations; Roy's root there is the largest
//! eigenvalue l, shown here as l / (1 + l)), F tests by comparing nested linear models,
//! sequential analysis of variance tables, and coefficients of linear models coded with
//! sum-to-zero, simple and difference contrasts.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    if let Some(e) = errors.first() {
        return e.to_string();
    }
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers of every row joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                if text.is_empty() {
                    continue;
                }
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    for table in tables.iter().filter(|t| t.title == title) {
        let Some(r) = row_keys(table).iter().position(|k| k == row) else {
            continue;
        };
        let c = table
            .column_titles()
            .iter()
            .position(|t| t == column)
            .unwrap_or_else(|| {
                panic!("{title}: no column {column} in {:?}", table.column_titles())
            });
        return match &table.rows[r][c] {
            Cell::Number(n) => *n,
            other => panic!("{title} / {row} / {column}: not a number: {other:?}"),
        };
    }
    let keys: Vec<Vec<String>> = tables
        .iter()
        .filter(|t| t.title == title)
        .map(row_keys)
        .collect();
    panic!("no table {title} with row {row}: {keys:?}");
}

fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    let failures: Vec<String> = expected
        .iter()
        .filter_map(|&(title, row, column, want)| {
            let got = value(tables, title, row, column);
            let ok = (got - want).abs() <= 1e-8 * want.abs().max(1.0);
            (!ok).then(|| format!("{title} / {row} / {column}: {got} vs {want}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// 33 made-up cases: factors a (1 to 3) and b (1, 2) in unbalanced cells, a covariate x and
/// three scores y1 to y3.
const DATA: &str = "DATA LIST FREE /a b x y1 y2 y3.
BEGIN DATA
1 1 5.5 19.5 42.3 14  1 2 4.9 17 41.9 10.6  1 2 5.8 20.6 38.9 11.4  1 2 4.2 13.9 37.9 11.5  1 1 3.1 15.9 36.4 14.6  1 2 2.6 15.9 38.4 7.5
1 1 6.1 17.9 36.8 13.6  1 2 3.2 15.5 37.8 8.8  1 1 4.2 16.1 31.3 14.9  1 2 8.3 23.9 45.7 12.8  1 1 8.5 16.3 38.8 14.5  2 1 5.5 21.8 41.5 15.3
2 2 8.5 23.4 43.1 14.1  2 1 7.8 19.2 41.2 13.8  2 2 2.1 18.8 35.6 11.8  2 1 6.6 20.1 39.9 14.1  2 2 3.3 17.7 40 13.1  2 1 6.2 23.1 36.8 13.8
2 2 7.6 24.6 38.5 11.6  2 1 5.4 17.6 36.2 14.3  2 2 5.7 19.4 43.1 15  3 1 4.4 20.7 40 10.2  3 2 3.8 22.9 37.1 11.4  3 1 3.3 22 37.2 12.9
3 2 4.5 22.4 37 10.5  3 1 4.5 22.7 37 11.9  3 2 4.3 24.2 38.4 12.1  3 1 7.6 26.3 39.8 14.2  3 2 8.3 26.1 43.9 13.2  3 1 2.1 16.7 34.9 12.2
3 2 6.6 28.8 38.4 11.5  3 1 4.9 21.8 33.6 13.2  3 2 3.6 21.9 36.9 8.8
END DATA.";

const MV: &str = "Multivariate Tests of Significance";
const SIG: &str = "Tests of Significance";
const UNI: &str = "Univariate F-tests";

fn four(effect: &str, rows: [(f64, f64, f64, f64, f64); 4]) -> Vec<(String, String, f64)> {
    let mut out = Vec::new();
    for (name, (v, f, d1, d2, p)) in ["Pillais", "Wilks", "Hotellings", "Roys"].iter().zip(rows) {
        let row = format!("{effect} / {name}");
        out.push((row.clone(), "Value".to_string(), v));
        if *name != "Roys" {
            out.push((row.clone(), "Approx. F".to_string(), f));
            out.push((row.clone(), "Hypoth. DF".to_string(), d1));
            out.push((row.clone(), "Error DF".to_string(), d2));
            out.push((row, "Sig. of F".to_string(), p));
        }
    }
    out
}

fn check_mv(tables: &[Table], rows: &[(String, String, f64)]) {
    let expected: Vec<(&str, &str, &str, f64)> = rows
        .iter()
        .map(|(r, c, v)| (MV, r.as_str(), c.as_str(), *v))
        .collect();
    check(tables, &expected);
}

fn roy(l: f64) -> f64 {
    l / (1.0 + l)
}

#[test]
fn multivariate_unique_tests_with_a_covariate() {
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x\n /PRINT=SIGNIF(EIGEN DIMENR HYPOTH STEPDOWN)."
    ));
    let mut rows = four(
        "A (S = 2, M = 0, N = 11)",
        [
            (
                0.873511929228999,
                6.46191137374956,
                6.0,
                50.0,
                4.3265808663428e-05,
            ),
            (
                0.260815772570874,
                7.664735323738,
                6.0,
                48.0,
                8.37930313474465e-06,
            ),
            (
                2.319094890877,
                8.88986374836184,
                6.0,
                46.0,
                1.89571841718356e-06,
            ),
            (roy(2.07032800061178), 0.0, 0.0, 0.0, 0.0),
        ],
    );
    rows.extend(four(
        "B (S = 1, M = 0.5, N = 11)",
        [
            (
                0.586242730927434,
                11.3350077400016,
                3.0,
                24.0,
                7.97258529350298e-05,
            ),
            (
                0.413757269072566,
                11.3350077400016,
                3.0,
                24.0,
                7.97258529350298e-05,
            ),
            (
                1.4168759675002,
                11.3350077400016,
                3.0,
                24.0,
                7.97258529350296e-05,
            ),
            (roy(1.4168759675002), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    rows.extend(four(
        "A BY B (S = 2, M = 0, N = 11)",
        [
            (
                0.311627319637832,
                1.53810492228423,
                6.0,
                50.0,
                0.185217105507328,
            ),
            (
                0.69412703494298,
                1.6021946868464,
                6.0,
                48.0,
                0.16709952393264,
            ),
            (
                0.432368421582747,
                1.65741228273386,
                6.0,
                46.0,
                0.153173696238455,
            ),
            (roy(0.412259587277444), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    rows.extend(four(
        "WITHIN CELLS Regression (S = 1, M = 0.5, N = 11)",
        [
            (
                0.64800659172306,
                14.7276983371967,
                3.0,
                24.0,
                1.19749020552201e-05,
            ),
            (
                0.35199340827694,
                14.7276983371967,
                3.0,
                24.0,
                1.19749020552201e-05,
            ),
            (
                1.84096229214959,
                14.7276983371967,
                3.0,
                24.0,
                1.197490205522e-05,
            ),
            (roy(1.84096229214959), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    check_mv(&t, &rows);
    check(
        &t,
        &[
            (UNI, "A / y1", "Hypoth. SS", 201.627223214843),
            (UNI, "A / y1", "Error SS", 103.27426280038),
            (UNI, "A / y1", "F", 25.3805142803044),
            (UNI, "A / y1", "Sig. of F", 7.72366293961371e-07),
            (UNI, "A / y2", "F", 0.251267450901445),
            (UNI, "A / y3", "Sig. of F", 0.0295912363098141),
            (
                "Eigenvalues and Canonical Correlations",
                "A / 1",
                "Eigenvalue",
                2.07032800061178,
            ),
            (
                "Eigenvalues and Canonical Correlations",
                "A / 2",
                "Eigenvalue",
                0.248766890265219,
            ),
            ("Hypothesis SSCP Matrix", "y1", "y2", -17.0625561452521),
            ("Hypothesis SSCP Matrix", "y3", "y3", 12.0496625635841),
            ("Regression Coefficients", "y1 / x", "B", 1.03989352486257),
            (
                "Regression Coefficients",
                "y1 / x",
                "Std. Err.",
                0.193747344887694,
            ),
            (
                "Regression Coefficients",
                "y2 / x",
                "t-Value",
                4.1187545144522,
            ),
            (
                "Regression Coefficients",
                "y3 / x",
                "Sig. t",
                0.00918656468187196,
            ),
        ],
    );
}

#[test]
fn the_same_model_through_glm_agrees() {
    let m = run(&format!("{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x."));
    let g = run(&format!("{DATA}\nGLM y1 y2 y3 BY a b WITH x."));
    for (mrow, grow) in [
        ("A (S = 2, M = 0, N = 11) / Pillais", "a / Pillai's Trace"),
        ("A (S = 2, M = 0, N = 11) / Wilks", "a / Wilks' Lambda"),
        (
            "A BY B (S = 2, M = 0, N = 11) / Hotellings",
            "a * b / Hotelling's Trace",
        ),
    ] {
        let mine = value(&m, MV, mrow, "Approx. F");
        let theirs = value(&g, "Multivariate Tests", grow, "F");
        assert!(
            (mine - theirs).abs() < 1e-9 * theirs,
            "{mrow}: {mine} vs {theirs}"
        );
    }
}

#[test]
fn univariate_full_factorial_and_parameters() {
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) b(1,2) /PRINT=PARAMETERS(ESTIM)."
    ));
    check(
        &t,
        &[
            (SIG, "WITHIN CELLS", "SS", 217.700333333334),
            (SIG, "WITHIN CELLS", "DF", 27.0),
            (SIG, "A", "SS", 177.620595764272),
            (SIG, "A", "F", 11.0145814023452),
            (SIG, "A", "Sig of F", 0.000317888904320373),
            (SIG, "B", "SS", 12.8751616161616),
            (SIG, "A BY B", "F", 0.5450121401696),
            (SIG, "A BY B", "Sig of F", 0.586080496329512),
            (SIG, "(Total)", "SS", 417.401818181818),
            ("Parameter Estimates", "A / 2", "Coeff.", -2.890555555555553),
            (
                "Parameter Estimates",
                "A / 2",
                "Std. Err.",
                0.701952548234256,
            ),
            ("Parameter Estimates", "A / 3", "t-Value", 0.291815605157788),
            ("Parameter Estimates", "B / 4", "Sig. t", 0.217155888461171),
            (
                "Parameter Estimates",
                "A BY B / 6",
                "Coeff.",
                0.417222222222222,
            ),
        ],
    );
}

#[test]
fn error_terms_of_a_main_effects_design() {
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) b(1,2) /DESIGN a b /ERROR=RESIDUAL /DESIGN a b /ERROR=WR /DESIGN a, b."
    ));
    let designs: Vec<&Table> = t.iter().filter(|t| t.title == SIG).collect();
    assert_eq!(designs.len(), 3);
    let one = |i: usize, row: &str, col: &str| value(&[designs[i].clone()], SIG, row, col);
    let close = |a: f64, b: f64| assert!((a - b).abs() <= 1e-8 * b.abs().max(1.0), "{a} vs {b}");
    close(one(0, "A", "SS"), 180.130202808471);
    close(one(0, "A", "F"), 11.1702067731378);
    close(one(0, "A", "Sig of F"), 0.000291859084524807);
    close(one(0, "B", "F"), 1.79102551134692);
    close(one(1, "RESIDUAL", "SS"), 8.78883885819522);
    close(one(1, "A", "F"), 20.4953356996081);
    close(one(1, "A", "Sig of F"), 0.0465217205246175);
    close(one(2, "A", "F"), 11.5320653762384);
    close(one(2, "A", "Sig of F"), 0.000206519446065472);
}

#[test]
fn sequential_sums_of_squares() {
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) b(1,2) /METHOD=SEQUENTIAL /DESIGN.\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) /METHOD=SEQ /PRINT SIGNIF(BRIEF)."
    ));
    check(
        &t,
        &[
            (SIG, "A", "SS", 176.47165151515),
            (SIG, "A", "F", 10.94333324610378),
            (SIG, "B", "Sig of F", 0.191963156559563),
            (MV, "A", "Wilks Lambda", 0.305937535952868),
            (MV, "A", "Approx. F", 6.73281960492083),
            (MV, "B", "Approx. F", 11.38655195351035),
            (MV, "A BY B", "Sig. of F", 0.120949197227208),
        ],
    );
}

#[test]
fn nested_design_and_numbered_error_terms() {
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) /DESIGN a, b WITHIN a.\nMANOVA y1 BY a(1,3) b(1,2) /DESIGN = a VS 1, b W a = 1 VS WITHIN."
    ));
    let mut rows = four(
        "A (S = 2, M = 0, N = 11.5)",
        [
            (
                0.817817091317256,
                5.995475042561,
                6.0,
                52.0,
                8.13030281421067e-05,
            ),
            (
                0.312793819140061,
                6.56678345917993,
                6.0,
                50.0,
                3.68656874287498e-05,
            ),
            (
                1.77943180569496,
                7.11772722277985,
                6.0,
                48.0,
                1.8323812820174e-05,
            ),
            (roy(1.50129752068353), 0.0, 0.0, 0.0, 0.0),
        ],
    );
    rows.extend(four(
        "B WITHIN A (S = 3, M = -0.5, N = 11.5)",
        [
            (
                0.709354661890696,
                2.78707133348097,
                9.0,
                81.0,
                0.00674660897572751,
            ),
            (
                0.33743799830615,
                3.81311917976595,
                9.0,
                60.9940366951145,
                0.000725555437374359,
            ),
            (
                1.82483708066623,
                4.79864565656677,
                9.0,
                71.0,
                5.24512185954839e-05,
            ),
            (roy(1.74538758492131), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    check_mv(&t, &rows);
    check(
        &t,
        &[
            (SIG, "A", "SS", 177.620595764272),
            (SIG, "A", "F", 11.4693415929118),
            (SIG, "A", "Sig of F", 0.0393332647186248),
            (SIG, "ERROR 1", "SS", 23.2298333333333),
        ],
    );
}

#[test]
fn contrasts_partitions_and_the_constant() {
    let t = run(&format!(
        "{DATA}\nMANOVA y2 BY a(1,3) /CONTRAST(a)=SIMPLE(1) /PRINT=PARAM.\n\
         MANOVA y3 BY a(1,3) b(1,2) /CONTRAST(a)=DIFFERENCE /DESIGN a(1) a(2) b.\n\
         MANOVA y1 BY a(1,3) /DESIGN CONSTANT a."
    ));
    check(
        &t,
        &[
            ("Parameter Estimates", "A / 2", "Coeff.", 0.844545454545451),
            (
                "Parameter Estimates",
                "A / 2",
                "Std. Err.",
                1.336482462249629,
            ),
            ("Parameter Estimates", "A / 3", "Coeff.", -0.895454545454550),
            (
                "Parameter Estimates",
                "A / 3",
                "Std. Err.",
                1.276811870918739,
            ),
            (SIG, "A(1)", "SS", 10.1958030370772),
            (SIG, "A(1)", "F", 5.44586542610406),
            (SIG, "A(1)", "Sig of F", 0.0273072560003533),
            (SIG, "A(2)", "F", 5.38993478868602),
            (SIG, "CONSTANT", "SS", 13618.0090046041),
            (SIG, "CONSTANT", "F", 1695.67919115479),
        ],
    );
    // the same partitions written with PARTITION
    let p = run(&format!(
        "{DATA}\nMANOVA y3 BY a(1,3) b(1,2) /CONTRAST(a)=DIFFERENCE /PARTITION(a)=(1,1) /DESIGN a(2) a(1) b."
    ));
    check(&p, &[(SIG, "A(2)", "SS", 10.0910891453018)]);
}

#[test]
fn unsupported_parts_say_so() {
    for (source, words) in [
        ("MANOVA y1 y2 y3 /WSFACTORS=t(3).", "not supported yet"),
        (
            "MANOVA y1 BY a(1,3) /TRANSFORM=POLYNOMIAL.",
            "not supported yet",
        ),
        ("MANOVA y1 BY a(1,3) /PRINT=DESIGN.", "not supported yet"),
        ("MANOVA y1 BY a(1,3) /DISCRIM.", "not supported yet"),
        (
            "MANOVA y1 BY a(1,3) b(1,2) /DESIGN a MWITHIN b(1).",
            "not supported yet",
        ),
    ] {
        let message = run_error(&format!("{DATA}\n{source}"));
        assert!(message.contains(words), "{source}: {message}");
    }
    let message = run_error(&format!("{DATA}\nMANOVA y1 BY a(1,3) WITH x /DESIGN a x."));
    assert!(message.contains("ANALYSIS"), "{message}");
}

#[test]
fn cell_information_means_and_homogeneity() {
    // cell statistics and observed means from the same package; the predicted means from the
    // within-cell slope of y1 on x (1.03989352486257, checked above); Box's M against GLM
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 BY a(1,3) b(1,2) WITH x /PRINT=CELLINFO(MEANS COV) HOMOGENEITY \
         ERROR(SSCP) /OMEANS=TABLES(a) /PMEANS /ANALYSIS=y1 WITH x."
    ));
    let g = run(&format!("{DATA}\nGLM y1 y2 BY a b /PRINT=HOMOGENEITY."));
    let cells = "Cell Means and Standard Deviations";
    let pm = "Observed and Predicted Means";
    check(
        &t,
        &[
            (cells, "y1 / 1 / 1", "Mean", 17.14),
            (cells, "y1 / 1 / 1", "Std. Dev.", 1.53883072493371),
            (cells, "y1 / 1 / 1", "Upper 95% CI", 19.05071071048),
            (cells, "x / 1 / 1", "Mean", 5.48),
            ("Combined Observed Means", "y1 / 1", "Weighted Mean", 17.5),
            (
                "Combined Observed Means",
                "y1 / 1",
                "Unweighted Mean",
                17.47,
            ),
            (pm, "y1 / 1 / 1", "Est. Mean", 17.14),
            (pm, "y1 / 1 / 1", "Adj. Mean", 11.4413834837531),
            (pm, "y1 / 1 / 1", "Std. Resid.", 0.0),
            ("Error SSCP Matrix", "y1", "y1", 103.27426280038),
            (
                "Box's M Test of Homogeneity of Dispersion Matrices",
                "Box's M",
                "Value",
                value(
                    &g,
                    "Box's Test of Equality of Covariance Matrices",
                    "Box's M",
                    "Value",
                ),
            ),
        ],
    );
    assert!(
        t.iter()
            .any(|t| t.title == "Within-Cell Variance-Covariance Matrix")
    );
    let homogeneity = t
        .iter()
        .find(|t| t.title == "Univariate Homogeneity of Variance Tests")
        .expect("homogeneity");
    assert_eq!(row_keys(homogeneity).len(), 4);
}
