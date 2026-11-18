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
        ("MANOVA y1 y2 y3 /WSFACTORS=t(2).", "do not divide"),
        (
            "MANOVA y1 y2 y3 /WSFACTORS=t(3) /TRANSFORM=POLYNOMIAL.",
            "cannot be used with WSFACTORS",
        ),
        (
            "MANOVA y1 BY a(1,3) /PRINT=DESIGN(BIAS).",
            "not supported yet",
        ),
        ("MANOVA y1 BY a(1,3) /PLOT=BOXPLOTS.", "not supported yet"),
        ("MANOVA y1 y2 /TRANSFORM=SPECIAL(1 1 1).", "square matrix"),
        ("MANOVA y1 y2 /RENAME=a.", "RENAME gives 1 names"),
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

// Repeated measures. The expected numbers come from R run as a separate program: `lm` fits
// of the transformed variables (orthonormal polynomial contrasts, `contr.poly`) compared with
// `anova` (the four multivariate statistics and the univariate F tests), `mauchly.test`, and the
// Greenhouse-Geisser and Huynh-Feldt epsilons and averaged F tests computed from their formulas
// on the residual matrices of those fits.

/// 12 made-up subjects: group g (1, 2), a score at three times y1 to y3, a covariate x.
const TIMES: &str = "DATA LIST FREE /g y1 y2 y3 x.
BEGIN DATA
1 12.1 14.3 15.0 3.2  1 10.4 13.8 13.1 2.7  1 11.7 12.9 16.2 4.1  1 9.8 11.5 12.7 2.2
1 13.2 15.9 17.4 3.8  1 10.9 12.2 14.8 3.0  2 14.5 15.1 19.3 4.4  2 12.8 16.7 18.2 3.9
2 15.3 15.8 21.0 5.1  2 13.1 14.2 17.6 3.3  2 16.0 18.4 20.5 4.8  2 12.2 15.5 16.9 2.9
END DATA.";

/// 15 made-up subjects in three unequal groups, six scores crossing a (2) and b (3).
const CROSSED: &str = "DATA LIST FREE /g v1 v2 v3 v4 v5 v6.
BEGIN DATA
1 5.2 6.1 7.3 4.8 6.6 8.0  1 4.7 5.9 6.8 5.1 5.7 7.2  1 6.0 6.4 8.1 5.5 7.0 8.9
1 5.5 7.0 7.7 4.9 6.2 8.4  2 6.3 6.8 7.0 6.9 8.1 9.6  2 5.8 7.3 7.9 6.1 7.4 9.1
2 6.6 6.5 8.4 7.2 8.8 10.2  2 5.1 6.9 7.2 6.4 7.7 9.3  2 6.9 7.7 8.8 7.5 8.3 10.9
3 4.4 5.0 5.9 4.2 5.1 6.0  3 4.9 5.6 6.3 4.0 5.8 6.7  3 3.8 4.7 5.2 3.9 4.6 5.5
3 5.3 5.9 6.9 4.7 5.5 6.4  3 4.1 5.2 6.1 4.4 5.0 6.2  3 4.6 4.9 5.8 4.1 5.3 5.9
END DATA.";

const AVG: &str = "Averaged Tests of Significance";
const MAUCHLY: &str = "Mauchly Sphericity Test";

#[test]
fn one_within_factor_with_a_between_factor() {
    let t = run(&format!(
        "{TIMES}\nMANOVA y1 y2 y3 BY g(1,2) /WSFACTORS=time(3) /PRINT=SIGNIF(UNIV GG HF EFSIZE)."
    ));
    let mut rows = four(
        "TIME (S = 1, M = 0, N = 3.5)",
        [
            (
                0.984482490812462,
                285.494995048171,
                2.0,
                9.0,
                7.22269124988921e-09,
            ),
            (
                0.0155175091875381,
                285.494995048166,
                2.0,
                9.0,
                7.22269124988972e-09,
            ),
            (
                63.4433322329259,
                285.494995048166,
                2.0,
                9.0,
                7.22269124988972e-09,
            ),
            (roy(63.4433322329259), 0.0, 0.0, 0.0, 0.0),
        ],
    );
    rows.extend(four(
        "G BY TIME (S = 1, M = 0, N = 3.5)",
        [
            (
                0.594558622248564,
                6.5990151645519,
                2.0,
                9.0,
                0.0172058570560864,
            ),
            (
                0.405441377751436,
                6.5990151645519,
                2.0,
                9.0,
                0.0172058570560864,
            ),
            (
                1.46644781434487,
                6.59901516455189,
                2.0,
                9.0,
                0.0172058570560865,
            ),
            (roy(1.46644781434486), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    check_mv(&t, &rows);
    check(
        &t,
        &[
            (SIG, "G", "SS", 84.64),
            (SIG, "G", "F", 14.5956199344714),
            (SIG, "G", "Sig of F", 0.00337123664626504),
            (AVG, "WITHIN CELLS", "SS", 13.4833333333333),
            (AVG, "WITHIN CELLS", "DF", 20.0),
            (AVG, "TIME", "SS", 107.165),
            (AVG, "TIME", "F", 79.479604449938),
            (AVG, "TIME", "Sig of F", 3.03920114360258e-10),
            (
                AVG,
                "TIME",
                "Sig. (Greenhouse-Geisser)",
                3.52060950279684e-07,
            ),
            (AVG, "TIME", "Sig. (Huynh-Feldt)", 3.39387545661415e-08),
            (
                AVG,
                "TIME",
                "Partial Eta Sq.",
                107.165 / (107.165 + 13.4833333333333),
            ),
            (AVG, "G BY TIME", "F", 3.24227441285538),
            (
                AVG,
                "G BY TIME",
                "Sig. (Greenhouse-Geisser)",
                0.0889731607725137,
            ),
            (AVG, "G BY TIME", "Sig. (Huynh-Feldt)", 0.0782778338634328),
            (MAUCHLY, "TIME", "Mauchly's W", 0.417664194986847),
            (MAUCHLY, "TIME", "Sig.", 0.0196662976456299),
            (
                MAUCHLY,
                "TIME",
                "Greenhouse-Geisser Epsilon",
                0.631977104248407,
            ),
            (MAUCHLY, "TIME", "Huynh-Feldt Epsilon", 0.75362760316393),
            (MAUCHLY, "TIME", "Lower-bound Epsilon", 0.5),
            (UNI, "TIME / T2", "F", 502.637856863512),
            (UNI, "TIME / T3", "F", 0.0539528738163426),
        ],
    );
    // AVONLY leaves only the averaged tests of the within-subjects effects
    let t = run(&format!(
        "{TIMES}\nMANOVA y1 y2 y3 BY g(1,2) /WSFACTORS=time(3) /PRINT=SIGNIF(AVONLY)."
    ));
    assert!(t.iter().all(|t| t.title != MV));
    check(&t, &[(AVG, "TIME", "F", 79.479604449938)]);
}

#[test]
fn a_covariate_constant_over_the_times() {
    let t = run(&format!(
        "{TIMES}\nMANOVA y1 y2 y3 BY g(1,2) WITH (x) /WSFACTORS=time(3)."
    ));
    let mut rows = four(
        "TIME (S = 1, M = 0, N = 3)",
        [
            (
                0.610621832583159,
                6.27278962900322,
                2.0,
                8.0,
                0.0229872165573495,
            ),
            (
                0.389378167416841,
                6.27278962900321,
                2.0,
                8.0,
                0.0229872165573495,
            ),
            (
                1.5681974072508,
                6.27278962900321,
                2.0,
                8.0,
                0.0229872165573495,
            ),
            (roy(1.5681974072508), 0.0, 0.0, 0.0, 0.0),
        ],
    );
    rows.extend(four(
        "G BY TIME (S = 1, M = 0, N = 3)",
        [
            (
                0.435857944226233,
                3.09041270556168,
                2.0,
                8.0,
                0.101287046451649,
            ),
            (
                0.564142055773767,
                3.09041270556168,
                2.0,
                8.0,
                0.101287046451649,
            ),
            (
                0.77260317639042,
                3.09041270556168,
                2.0,
                8.0,
                0.101287046451649,
            ),
            (roy(0.77260317639042), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    check_mv(&t, &rows);
    check(
        &t,
        &[
            (SIG, "G", "F", 11.3448191761442),
            (SIG, "G", "Sig of F", 0.00828051993682364),
            (AVG, "TIME", "SS", 3.70687239287257),
            (AVG, "WITHIN CELLS", "SS", 9.92800795372379),
            (AVG, "TIME", "F", 3.36037719664999),
            (AVG, "TIME", "Sig of F", 0.0575303568195991),
        ],
    );
}

#[test]
fn two_within_factors_with_unequal_groups() {
    let source = "MANOVA v1 TO v6 BY g(1,3) /WSFACTORS=a(2) b(3) /PRINT=SIGNIF(GG HF).";
    let t = run(&format!("{CROSSED}\n{source}"));
    let mut rows = four(
        "B (S = 1, M = 0, N = 4.5)",
        [
            (
                0.984096281611936,
                340.331073324821,
                2.0,
                11.0,
                1.28304746563965e-10,
            ),
            (
                0.0159037183880648,
                340.331073324812,
                2.0,
                11.0,
                1.28304746563984e-10,
            ),
            (
                61.8783769681477,
                340.331073324812,
                2.0,
                11.0,
                1.28304746563984e-10,
            ),
            (roy(61.8783769681477), 0.0, 0.0, 0.0, 0.0),
        ],
    );
    rows.extend(four(
        "G BY B (S = 2, M = -0.5, N = 4.5)",
        [
            (
                0.655052018192864,
                2.92227815671817,
                4.0,
                24.0,
                0.0421677390542968,
            ),
            (
                0.344962165904924,
                3.86433531786293,
                4.0,
                22.0,
                0.0158927590422137,
            ),
            (
                1.89882750845732,
                4.7470687711433,
                4.0,
                20.0,
                0.00740655983027505,
            ),
            (roy(1.89880585388111), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    rows.extend(four(
        "G BY A BY B (S = 2, M = -0.5, N = 4.5)",
        [
            (
                0.516884858973503,
                2.09107780512208,
                4.0,
                24.0,
                0.113412515351866,
            ),
            (
                0.483242149946882,
                2.4118908197091,
                4.0,
                22.0,
                0.079748627657885,
            ),
            (
                1.0690930855049,
                2.67273271376224,
                4.0,
                20.0,
                0.0619167963986243,
            ),
            (roy(1.06884718818374), 0.0, 0.0, 0.0, 0.0),
        ],
    ));
    check_mv(&t, &rows);
    check(
        &t,
        &[
            (SIG, "G", "F", 37.7354689357442),
            (SIG, "G", "Sig of F", 6.66660582904968e-06),
            (SIG, "A", "F", 27.9058720219849),
            (SIG, "A", "Sig of F", 0.000193773103872251),
            (SIG, "G BY A", "F", 26.1170188667034),
            (SIG, "G BY A", "Sig of F", 4.25104929721713e-05),
            (AVG, "B", "F", 404.908492127117),
            (AVG, "B", "Sig. (Greenhouse-Geisser)", 4.84663000939867e-19),
            (AVG, "G BY B", "F", 6.26781947446548),
            (AVG, "G BY B", "Sig of F", 0.00133584506646876),
            (
                AVG,
                "G BY B",
                "Sig. (Greenhouse-Geisser)",
                0.00139974912282477,
            ),
            (AVG, "G BY B", "Sig. (Huynh-Feldt)", 0.00133584506646876),
            (AVG, "A BY B", "F", 13.6687393674505),
            (
                AVG,
                "A BY B",
                "Sig. (Greenhouse-Geisser)",
                0.000331773803609623,
            ),
            (AVG, "G BY A BY B", "F", 1.76777068036644),
            (
                AVG,
                "G BY A BY B",
                "Sig. (Greenhouse-Geisser)",
                0.182180481343512,
            ),
            (MAUCHLY, "B", "Mauchly's W", 0.989648955653761),
            (MAUCHLY, "B", "Sig.", 0.944379255875143),
            (
                MAUCHLY,
                "B",
                "Greenhouse-Geisser Epsilon",
                0.989755002082429,
            ),
            (MAUCHLY, "B", "Huynh-Feldt Epsilon", 1.0),
            (MAUCHLY, "A BY B", "Mauchly's W", 0.797445223152294),
            (MAUCHLY, "A BY B", "Sig.", 0.287974965458691),
            (
                MAUCHLY,
                "A BY B",
                "Greenhouse-Geisser Epsilon",
                0.831562951852962,
            ),
        ],
    );
    // the same design through GLM: the same sphericity test and averaged tests
    let g = run(&format!(
        "{CROSSED}\nGLM v1 TO v6 BY g /WSFACTOR=a 2 Polynomial b 3 Polynomial."
    ));
    for (mine, theirs) in [
        (
            value(&t, MAUCHLY, "A BY B", "Mauchly's W"),
            value(&g, "Mauchly's Test of Sphericity", "a * b", "Mauchly's W"),
        ),
        (
            value(&t, MAUCHLY, "B", "Huynh-Feldt Epsilon"),
            value(
                &g,
                "Mauchly's Test of Sphericity",
                "b",
                "Epsilon / Huynh-Feldt",
            ),
        ),
        (
            value(&t, AVG, "G BY A BY B", "F"),
            value(
                &g,
                "Tests of Within-Subjects Effects",
                "a * b * g / Sphericity Assumed",
                "F",
            ),
        ),
    ] {
        assert!(
            (mine - theirs).abs() < 1e-9 * theirs.abs().max(1.0),
            "{mine} vs {theirs}"
        );
    }
}

#[test]
fn several_measures_average_over_the_contrasts_of_each() {
    let t = run(&format!(
        "{CROSSED}\nMANOVA v1 TO v6 BY g(1,3) /WSFACTORS=t(3) /MEASURE=p r."
    ));
    let amv = "Averaged Multivariate Tests of Significance";
    check(
        &t,
        &[
            (
                amv,
                "T (S = 2, M = -0.5, N = 10.5) / Wilks",
                "Value",
                0.0257934056807608,
            ),
            (
                amv,
                "T (S = 2, M = -0.5, N = 10.5) / Pillais",
                "Value",
                0.986826488247559,
            ),
            (
                amv,
                "G BY T (S = 2, M = 0.5, N = 10.5) / Wilks",
                "Value",
                0.383934366615585,
            ),
            (
                amv,
                "G BY T (S = 2, M = 0.5, N = 10.5) / Pillais",
                "Value",
                0.660655646259957,
            ),
            (AVG, "T", "F", 107.50582183601),
            (AVG, "G BY T", "F", 1.08124493019681),
        ],
    );
    assert!(t.iter().any(|t| t.title == MAUCHLY && t.rows.len() == 2));
}

#[test]
fn within_designs_simple_effects_and_names() {
    // MWITHIN b(2) is the mean of the second level of b; WSDESIGN without B BY A leaves it out
    let t = run(&format!(
        "{CROSSED}\nMANOVA v1 TO v6 BY g(1,3) /WSFACTORS=a(2) b(3) /WSDESIGN=a, MWITHIN b(2) \
         /RENAME=mean ab1 ab2 adiff ab3 ab4 /PRINT=TRANSFORM."
    ));
    check(
        &t,
        &[
            ("Transformation Matrix", "v1", "mean", 1.0 / 6f64.sqrt()),
            ("Transformation Matrix", "v1", "adiff", -1.0 / 6f64.sqrt()),
            ("Transformation Matrix", "v3", "ab1", 0.5),
            (SIG, "A", "F", 27.9058720219849),
        ],
    );
    // v2 and v5 over the root of 2: the group test is the one-way test of their mean
    let simple = value(&t, SIG, "G BY MWITHIN B(2)", "F");
    let s = run(&format!(
        "{CROSSED}\nCOMPUTE m = (v2 + v5) / 2.\nMANOVA m BY g(1,3)."
    ));
    let direct = value(&s, SIG, "G", "F");
    assert!(
        (simple - direct).abs() < 1e-9 * direct,
        "{simple} vs {direct}"
    );
}

#[test]
fn transformations_of_the_dependents() {
    // the F test of group a on one transformed variable, from R's anova of lm on that variable
    for (spec, var, f, sig) in [
        ("REPEATED", "T2", 14.1460896455227, 4.70613564531853e-05),
        ("HELMERT", "T2", 18.580234174412, 5.62487764418208e-06),
        ("DIFFERENCE", "T3", 2.65835232370065, 0.0865199420869344),
        ("SIMPLE", "T2", 10.9457789920231, 0.000269381749792855),
        ("DEVIATION", "T3", 4.39281075979312, 0.0212205918848656),
        ("POLYNOMIAL", "T3", 4.39281075979313, 0.0212205918848655),
        (
            "BASIS DEVIATION",
            "T2",
            10.9457789920231,
            0.000269381749792857,
        ),
        (
            "ORTHONORM SPECIAL(1 1 1 1 0 -1 2 -1 -1)",
            "T3",
            4.39281075979312,
            0.0212205918848657,
        ),
    ] {
        let t = run(&format!(
            "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) /TRANSFORM={spec} /PRINT=TRANSFORM /ANALYSIS={var}."
        ));
        let got = (value(&t, SIG, "A", "F"), value(&t, SIG, "A", "Sig of F"));
        assert!(
            (got.0 - f).abs() < 1e-8 * f && (got.1 - sig).abs() < 1e-8,
            "{spec}: {got:?} vs {f} {sig}"
        );
    }
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) /TRANSFORM(y1 y2 y3)=BASIS DEVIATION \
         /RENAME=m d1 d2 /PRINT=TRANSFORM /ANALYSIS=d1."
    ));
    check(
        &t,
        &[
            ("Transformation Matrix", "y1", "d1", 1.0),
            ("Transformation Matrix", "y3", "d1", -1.0),
            ("Transformation Matrix", "y2", "d2", 1.0),
            ("Transformation Matrix", "y1", "d2", 0.0),
        ],
    );
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) /TRANSFORM=ORTHONORM SPECIAL(1 1 1 1 0 -1 2 -1 -1) \
         /PRINT=TRANSFORM."
    ));
    check(
        &t,
        &[
            (
                "Transformation Matrix",
                "y1",
                "T2",
                std::f64::consts::FRAC_1_SQRT_2,
            ),
            ("Transformation Matrix", "y2", "T3", -0.816496580927726),
        ],
    );
}

#[test]
fn single_df_effect_sizes_and_intervals() {
    // R: sequential anova of y1 on the polynomial scores of a; coefficients and the inverse
    // cross products of lm with sum-to-zero coding; Bonferroni and Scheffe multipliers from qt
    // and qf; the QR decomposition of the design for the orthogonal estimates
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) /CONTRAST(a)=POLYNOMIAL /PRINT=SIGNIF(SINGLEDF EFSIZE)."
    ));
    check(
        &t,
        &[
            (SIG, "A (1)", "SS", 175.848216598704),
            (SIG, "A (1)", "F", 21.8961642327665),
            (SIG, "A (1)", "Sig of F", 5.75326139188984e-05),
            (SIG, "A (2)", "F", 0.0776285002088577),
            (SIG, "A", "Partial Eta Sq.", 0.42278601536489),
        ],
    );
    let lower = |extra: &str, column: &str| {
        let t = run(&format!(
            "{DATA}\nMANOVA y1 BY a(1,3) /PRINT=PARAMETERS(ESTIM COR ORTHO) {extra}."
        ));
        value(&t, "Parameter Estimates", "A / 2", column)
    };
    let pe = "Parameter Estimates";
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) /PRINT=PARAMETERS(ESTIM COR ORTHO)."
    ));
    check(
        &t,
        &[
            (pe, "A / 2", "Coeff.", -2.87055555555555),
            (pe, "A / 2", "Std. Err.", 0.698626960949053),
            (
                "Covariance Factors of Parameter Estimates",
                "2",
                "2",
                0.0607744107744108,
            ),
            (
                "Covariance Factors of Parameter Estimates",
                "2",
                "3",
                -0.0331649831649832,
            ),
            (
                "Correlations of Parameter Estimates",
                "2",
                "3",
                -0.532590015245738,
            ),
        ],
    );
    let ortho = |p: &str| value(&t, "Orthogonal Estimates", &format!("A / {p}"), "Estimate").abs();
    assert!((ortho("2") - 13.2607773753541).abs() < 1e-8);
    assert!((ortho("3") - 0.789578948837154).abs() < 1e-8);
    for (extra, column, want) in [
        (
            "/CINTERVAL=JOINT(.9) UNIVARIATE(BONFER)",
            "Lower -90%",
            -4.29734215513124,
        ),
        (
            "/CINTERVAL=JOINT UNIVARIATE(SCHEFFE)",
            "Lower -95%",
            -4.66966024282137,
        ),
        ("/CINTERVAL=INDIVIDUAL(.9)", "Lower -90%", -4.05630777069438),
    ] {
        let got = lower(extra, column);
        assert!((got - want).abs() < 1e-8, "{extra}: {got} vs {want}");
    }
}

#[test]
fn predicted_means_tables_and_casewise_residuals() {
    // R: cell means collapsed over b, and the fitted values of lm(y1 ~ a * b + x)
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) b(1,2) WITH x /PMEANS=TABLES(a) /RESIDUALS=CASEWISE."
    ));
    let cw = "Observed and Predicted Values for Each Case";
    check(
        &t,
        &[
            (cw, "1", "Predicted", 17.1607978704972),
            (cw, "1", "Std. Resid.", 1.17370342755635),
            (cw, "5", "Predicted", 14.6650534108271),
        ],
    );
    assert!(t.iter().any(|t| t.title == "Combined Adjusted Means"));
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 BY a(1,3) b(1,2) /PMEANS=VARIABLES(y1) TABLES(a) /PRINT=DESIGN(OVERALL ONEWAY)."
    ));
    check(
        &t,
        &[
            ("Combined Adjusted Means", "y1 / 1", "Weighted Mean", 17.5),
            (
                "Combined Adjusted Means",
                "y1 / 1",
                "Unweighted Mean",
                17.47,
            ),
            ("Design Matrix", "1", "2", 1.0),
            ("One-Way Basis Matrix", "3", "2", -1.0),
        ],
    );
    let pm = t
        .iter()
        .find(|t| t.title == "Observed and Predicted Means")
        .expect("predicted means");
    assert_eq!(pm.rows.len(), 6);
}

#[test]
fn covariates_that_vary_over_the_times() {
    // R: lm of the transformed scores on the groups and the same transformation of v4 to v6
    let t = run(&format!(
        "{CROSSED}\nMANOVA v1 v2 v3 BY g(1,3) WITH v4 v5 v6 /WSFACTORS=t(3)."
    ));
    check(
        &t,
        &[
            (
                MV,
                "T (S = 1, M = 0, N = 3.5) / Wilks",
                "Value",
                0.75851989118946,
            ),
            (
                MV,
                "T (S = 1, M = 0, N = 3.5) / Wilks",
                "Approx. F",
                1.4326064514187,
            ),
            (MV, "T (S = 1, M = 0, N = 3.5) / Wilks", "Error DF", 9.0),
            (SIG, "G", "F", 2.9945399902483),
            (SIG, "G", "Sig of F", 0.0915641637539739),
        ],
    );
}

#[test]
fn observed_power_of_f_and_t_tests() {
    // R: the noncentrality is F times its hypothesis df (|t| for a t test) and the power is
    // the upper tail of the noncentral F (two tails of the noncentral t) beyond the critical
    // value, from qf, pf, qt and pt with ncp
    let t = run(&format!(
        "{DATA}\nMANOVA y1 BY a(1,3) b(1,2) /PRINT=PARAMETERS /POWER=F(.1) T(.01)."
    ));
    let pe = "Parameter Estimates";
    check(
        &t,
        &[
            (SIG, "A", "Noncent. Parameter", 22.0291628046903),
            (SIG, "A", "Observed Power", 0.993881938010872),
            (pe, "A / 2", "t-Value", -4.1178788549549),
            (pe, "A / 2", "Noncent. Parameter", 4.1178788549549),
            (pe, "A / 2", "Observed Power", 0.900526361942133),
        ],
    );
    // multivariate (approximate): each criterion's eta squared over one less it, times the
    // error df of its F; Roy's root has no F and so no power
    let source = "MANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x";
    let t = run(&format!("{DATA}\n{source} /POWER."));
    let a = "A (S = 2, M = 0, N = 11)";
    let b = "B (S = 1, M = 0.5, N = 11)";
    let mut expected = Vec::new();
    for (row, lambda, power) in [
        (
            format!("{a} / Pillais"),
            38.7714682424973,
            0.997785078884834,
        ),
        (format!("{a} / Wilks"), 45.9884119424279, 0.999555121062253),
        (
            format!("{a} / Hotellings"),
            53.339182490171,
            0.999918092410058,
        ),
        (format!("{b} / Wilks"), 34.0050232200048, 0.997582910535303),
        ("A / y2".to_string(), 0.5025349018029, 0.0853371324502979),
        ("B / y3".to_string(), 21.4542719833618, 0.99370167216524),
    ] {
        let title = if row.contains('(') { MV } else { UNI };
        expected.push((title, row.clone(), "Noncent. Parameter", lambda));
        expected.push((title, row, "Observed Power", power));
    }
    let expected: Vec<(&str, &str, &str, f64)> = expected
        .iter()
        .map(|(t, r, c, v)| (*t, r.as_str(), *c, *v))
        .collect();
    check(&t, &expected);
    let mv = t.iter().find(|t| t.title == MV).unwrap();
    let roys = row_keys(mv)
        .iter()
        .position(|k| *k == format!("{a} / Roys"))
        .unwrap();
    assert!(matches!(mv.rows[roys].last(), Some(Cell::Empty)));
    // EXACT with one hypothesis df: Hotelling's trace times the error df, for every criterion;
    // an effect of more hypothesis df gets none
    let t = run(&format!("{DATA}\n{source} /POWER=EXACT F(.05)."));
    for name in ["Pillais", "Roys"] {
        let row = format!("{b} / {name}");
        check(
            &t,
            &[
                (MV, &row, "Noncent. Parameter", 36.8387751550052),
                (MV, &row, "Observed Power", 0.998773481027223),
            ],
        );
    }
    let mv = t.iter().find(|t| t.title == MV).unwrap();
    let pillais = row_keys(mv)
        .iter()
        .position(|k| *k == format!("{a} / Pillais"))
        .unwrap();
    assert!(matches!(mv.rows[pillais].last(), Some(Cell::Empty)));
}

#[test]
fn multivariate_intervals_of_the_estimates() {
    // R: the multipliers from the F approximation of each criterion (qf) at level .9 for the
    // three dependents, the error df 26 and a family of 2 (the term A) or 1; one df makes
    // every criterion Hotelling's T^2: sqrt(3 * 26 / 24 * qf(.9, 3, 24))
    let k = |cint: &str, row: &str| {
        let t = run(&format!(
            "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x /PRINT=PARAMETERS /CINTERVAL={cint}."
        ));
        let (title, coeff) = if row.ends_with("/ x") {
            ("Regression Coefficients", "B")
        } else {
            ("Parameter Estimates", "Coeff.")
        };
        let b = value(&t, title, row, coeff);
        (b - value(&t, title, row, "Lower -90%")) / value(&t, title, row, "Std. Err.")
    };
    for (cint, row, want) in [
        (
            "JOINT(.9) MULTIVARIATE(PILLAI)",
            "y1 / A / 2",
            3.91276237209361,
        ),
        (
            "JOINT(.9) MULTIVARIATE(WILKS)",
            "y2 / A / 3",
            3.71775646520788,
        ),
        ("JOINT(.9) MULT(HOTELLING)", "y3 / A / 2", 3.59580342940298),
        (
            "JOINT(.9) MULTIVARIATE(BONFER)",
            "y1 / A / 2",
            2.55894238571891,
        ),
        ("JOINT(.9) MULTIVARIATE(PILLAI)", "y1 / x", 2.75027571871239),
        (
            "INDIVIDUAL(.9) MULTIVARIATE(ROY)",
            "y2 / A / 2",
            2.75027571871239,
        ),
        (
            "INDIVIDUAL(.9) MULTIVARIATE(WILKS)",
            "y2 / B / 4",
            2.75027571871239,
        ),
    ] {
        let got = k(cint, row);
        assert!((got - want).abs() < 1e-8, "{cint} {row}: {got} vs {want}");
    }
    let message = run_error(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) /PRINT=PARAMETERS /CINTERVAL=JOINT MULTIVARIATE(ROY)."
    ));
    assert!(message.contains("largest root"), "{message}");
}

#[test]
fn principal_components_of_the_error_matrices() {
    // R: eigen of the within-cell covariance after x (and of its correlations), loadings as
    // vectors times root values with positive column sums; varimax of the first two with
    // Kaiser normalization, columns by their sums of squares (R's varimax stops a little early,
    // so the reference turns the pair by the exact angle until it no longer moves)
    let pc = |extra: &str| {
        run(&format!(
            "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x /PCOMPS {extra} /DESIGN."
        ))
    };
    let t = pc("COR ROTATE(VARIMAX) NCOMP(1)");
    let (cl, rl) = ("Component Loadings", "Rotated Component Loadings");
    check(
        &t,
        &[
            ("Principal Components", "1", "Eigenvalue", 1.17204642061415),
            ("Principal Components", "2", "Pct of Var", 32.390149102827),
            (cl, "y1", "Component 2", 0.911373857439108),
            (cl, "y3", "Component 3", -0.661462270041933),
            (rl, "y2", "Component 1", 0.763767173077893),
            (rl, "y1", "Component 2", 0.998375184931312),
            (rl, "y3", "Component 2", 0.0602758749460019),
        ],
    );
    let rotated = t.iter().find(|t| t.title == rl).unwrap();
    assert_eq!(rotated.column_titles().len(), 3);
    let t = pc("COV MINEIGEN(1.5)");
    check(
        &t,
        &[
            ("Principal Components", "3", "Eigenvalue", 1.4443934345362),
            ("Principal Components", "1", "Cum Pct", 51.9140760702648),
            (cl, "y2", "Component 1", 2.38291759138411),
            (cl, "y1", "Component 2", 1.97040804877439),
        ],
    );
    let loadings = t.iter().find(|t| t.title == cl).unwrap();
    assert_eq!(loadings.column_titles().len(), 3);
    assert!(!t.iter().any(|t| t.title == rl));
}

#[test]
fn discriminant_functions_of_each_effect() {
    // R: the eigenvectors of E^-1 H scaled to unit variance within the error (the scaling of
    // MASS::lda for one factor), signed so their correlations with the dependents add up to
    // more than 0; the estimates are the sum-to-zero coefficients times them; dimension
    // reduction Sig. 6.2e-05 for both roots and 0.066 for the second; the rotation as for
    // PCOMPS
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) /DISCRIM=ALL ALPHA(.25) /DESIGN."
    ));
    let raw = "Raw Discriminant Function Coefficients";
    let rc = "Rotated Correlations between Dependent and Canonical Variables";
    check(
        &t,
        &[
            (raw, "A / y1", "Function 1", 0.401975529748379),
            (raw, "A / y3", "Function 2", 0.478627565660424),
            (
                "Standardized Discriminant Function Coefficients",
                "A / y2",
                "Function 1",
                -0.712408044959842,
            ),
            (
                "Correlations between Dependent and Canonical Variables",
                "A / y3",
                "Function 2",
                0.91253639365901,
            ),
            (
                "Estimates of Effects for Canonical Variables",
                "A / 2",
                "Function 1",
                -1.09421700242209,
            ),
            (
                "Estimates of Effects for Canonical Variables",
                "A / 3",
                "Function 2",
                0.615471445385073,
            ),
            (rc, "A / y3", "Function 1", 0.896070453770352),
            (rc, "A / y1", "Function 2", 0.87391198572509),
        ],
    );
    // at ALPHA .05 only the first function is kept, so nothing is rotated
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) /DISCRIM RAW ROTATE ALPHA(.05) /DESIGN."
    ));
    let table = t.iter().find(|t| t.title == raw).unwrap();
    assert_eq!(table.column_titles().len(), 3);
    assert!(!t.iter().any(|t| t.title == rc));
    // the covariate: a canonical correlation analysis against the within-cell error
    let t = run(&format!(
        "{DATA}\nMANOVA y1 y2 y3 BY a(1,3) b(1,2) WITH x /DISCRIM RAW ESTIM /DESIGN."
    ));
    let r = "WITHIN CELLS Regression";
    check(
        &t,
        &[
            (raw, &format!("{r} / y2"), "Function 1", 0.216860369097278),
            (
                "Estimates of Effects for Canonical Variables",
                &format!("{r} / x"),
                "Function 1",
                0.672566919263979,
            ),
        ],
    );
}
