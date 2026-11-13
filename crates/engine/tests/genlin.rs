//! GENLIN on synthetic data. The expected numbers come from an independent fit of the same
//! models in a general statistics package (its generalized linear model routine, its negative
//! binomial and proportional odds routines), with the standard errors from a numerical second
//! derivative of the log likelihood where the documented observed information differs from
//! the package's expected one. Where the documentation defines a statistic differently from
//! the package (the scale, the subpopulation deviance of a binary response, the multinomial
//! constant), the test converts as its comment says.

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

fn tables_of(source: &str) -> Vec<Table> {
    run(&mut Engine::default(), source)
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    if let Some(e) = errors.first() {
        return e.message.clone();
    }
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell {
                    carried[level] = t.clone();
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title} in {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = find(tables, title);
    let r = if table.row_headers == 0 {
        0
    } else {
        row_keys(table)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)))
    };
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

fn get(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    number(cell(tables, title, row, column))
}

fn close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{what}: {actual} against {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

const SE: &str = "Std. Error";
const B: &str = "B";
const PE: &str = "Parameter Estimates";
const FIT: &str = "Goodness of Fit";

/// Repairs at bike shops over a number of opening hours, by shop, zone and temperature.
const COUNTS: &str = "DATA LIST FREE /repairs shop (F1) zone (A5) hours temp.
BEGIN DATA.
97 1 north 75 19.9 13 2 north 55 17 13 3 south 59 7.6 15 1 south 51 17.1 9 2 north 50 8 14 3 north 40 11.7
10 1 south 31 13.5 3 2 south 50 17.9 6 3 north 30 8.3 17 1 north 49 11.4 12 2 south 76 15.3 18 3 south 74 25
5 1 north 39 16.1 4 2 north 75 13.6 7 3 south 38 28.8 43 1 south 76 11.8 27 2 north 69 20.1 25 3 north 64 12.9
4 1 south 22 14.2 10 2 south 40 24.1 41 3 north 77 23.7 16 1 north 78 10 16 2 south 69 15.2 7 3 south 33 14.1
38 1 north 73 13.7 9 2 north 43 19 4 3 south 38 11.9 24 1 south 65 26.7 17 2 north 40 29.1 11 3 north 33 12.4
21 1 south 75 16.9 1 2 south 68 10.3 14 3 north 63 21 19 1 north 71 9 3 2 south 57 12.8 13 3 south 41 12.3
END DATA.
COMPUTE lnh = LN(hours).
";

/// Whether a seed sprouted, by soil and moisture; every setting repeats four times.
const SEEDS: &str = "DATA LIST FREE /sprout (F1) soil (A4) moist.
BEGIN DATA.
0 clay 2 1 clay 4 1 clay 6 1 clay 8 0 clay 2 1 clay 4 1 clay 6 1 clay 8 0 clay 2 1 clay 4 1 clay 6 1 clay 8
1 clay 2 1 clay 4 1 clay 6 1 clay 8 0 loam 2 1 loam 4 1 loam 6 1 loam 8 1 loam 2 1 loam 4 1 loam 6 0 loam 8
1 loam 2 1 loam 4 1 loam 6 0 loam 8 1 loam 2 1 loam 4 1 loam 6 1 loam 8 0 sand 2 1 sand 4 0 sand 6 1 sand 8
0 sand 2 0 sand 4 1 sand 6 1 sand 8 0 sand 2 0 sand 4 1 sand 6 1 sand 8 0 sand 2 0 sand 4 1 sand 6 0 sand 8
END DATA.
";

/// The same seeds counted per soil and moisture: sprouted out of planted.
const TRAYS: &str = "DATA LIST FREE /soil (A4) moist ok planted.
BEGIN DATA.
clay 2 1 4 loam 2 3 4 sand 2 0 4 clay 4 4 4 loam 4 4 4 sand 4 1 4
clay 6 4 4 loam 6 4 4 sand 6 3 4 clay 8 4 4 loam 8 2 4 sand 8 3 4
END DATA.
";

/// The cost of a repair job by crew and job size.
const COSTS: &str = "DATA LIST FREE /cost crew size.
BEGIN DATA.
37.33 1 3.1 18.55 2 3.7 24.67 1 4.1 42.01 2 3.3 92.89 1 7.8 8.7 2 3.8
41.29 1 2.7 53.52 2 6.6 39.09 1 4.4 61.17 2 4.2 74.47 1 6.5 29.99 2 6.2
19.89 1 1.9 48.7 2 2.6 39.38 1 6.2 20.07 2 5.9 42.14 1 6.9 59.58 2 5.3
116.65 1 9.2 58.64 2 8.1 42.39 1 4.5 42.56 2 8.2 106.8 1 2.7 13.26 2 2.4
70.16 1 5.5 37.11 2 5.4 18.41 1 1.1 34.09 2 4.9 43.17 1 5.5 17.27 2 4.8
END DATA.
";

/// A taste rating from 1 to 4 by recipe and the amount of sugar.
const TASTES: &str = "DATA LIST FREE /taste recipe sugar.
BEGIN DATA.
2 1 1.7 2 2 3 4 3 3.5 4 1 2.2 4 2 2.6 2 3 3.2 4 1 3.7 4 2 1.9 1 3 1 2 1 0.3 1 2 1.7 1 3 1.5
4 1 3 2 2 2.7 1 3 1.2 3 1 1.6 2 2 3.6 4 3 3.4 1 1 0.2 2 2 2.8 3 3 4.5 3 1 3.5 1 2 2.9 1 3 0.2
1 1 2.1 2 2 2.1 1 3 2.6 1 1 0.1 1 2 1.6 1 3 4.5 3 1 2.7 4 2 0.1 2 3 1.1 4 1 4.5 1 2 0.4 3 3 0.3
2 1 1.7 1 2 2.4 2 3 1.8 1 1 3.7 1 2 4.1 2 3 4.4 4 1 0.4 2 2 2.3 3 3 0.2
END DATA.
";

#[test]
fn poisson_log_link_with_offset_factors_and_covariate() {
    let source = format!(
        "{COUNTS}GENLIN repairs BY shop zone WITH temp
  /MODEL shop zone temp DISTRIBUTION=POISSON LINK=LOG OFFSET=lnh
  /PRINT CPS DESCRIPTIVES MODELINFO FIT SUMMARY SOLUTION(EXPONENTIATED).
"
    );
    let t = tables_of(&source);
    let tol = 1e-7;
    for (row, b, se) in [
        ("(Intercept)", -2.3728723390191_f64, 0.17307140907605),
        ("[shop=1]", 0.5133412346991, 0.09748265262000),
        ("[shop=2]", -0.4440615729282, 0.11782406740205),
        ("[zone=north]", 0.5274349570969, 0.08579585563451),
        ("temp", 0.0471669239866, 0.00721654635595),
    ] {
        close(get(&t, PE, row, B), b, tol, row);
        close(get(&t, PE, row, SE), se, 1e-6, row);
        close(get(&t, PE, row, "Exp(B)"), b.exp(), tol, row);
        close(
            get(&t, PE, row, "Hypothesis Test / Wald Chi-Square"),
            (b / se).powi(2),
            1e-6,
            row,
        );
    }
    assert_eq!(get(&t, PE, "[shop=3]", B), 0.0);
    assert_eq!(get(&t, PE, "(Scale)", B), 1.0);
    close(
        get(&t, FIT, "Deviance", "Value"),
        152.574831177,
        tol,
        "deviance",
    );
    assert_eq!(get(&t, FIT, "Deviance", "df"), 31.0);
    close(
        get(&t, FIT, "Pearson Chi-Square", "Value"),
        153.283974669,
        tol,
        "pearson",
    );
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -154.100460417,
        tol,
        "ll",
    );
    close(
        get(&t, FIT, "Akaike's Information Criterion (AIC)", "Value"),
        318.200920834,
        tol,
        "aic",
    );
    close(
        get(&t, FIT, "Finite Sample Corrected AIC (AICC)", "Value"),
        320.200920834,
        tol,
        "aicc",
    );
    close(
        get(&t, FIT, "Bayesian Information Criterion (BIC)", "Value"),
        326.118515526,
        tol,
        "bic",
    );
    close(
        get(&t, FIT, "Consistent AIC (CAIC)", "Value"),
        331.118515526,
        tol,
        "caic",
    );
    close(
        get(&t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        147.725883552,
        1e-6,
        "omnibus",
    );
    assert_eq!(get(&t, "Omnibus Test", "", "df"), 4.0);
    // Type III Wald; the intercept's hypothesis averages the factor levels
    for (row, chi, df) in [
        ("(Intercept)", 235.540446556, 1.0),
        ("shop", 84.8959269196, 2.0),
        ("zone", 37.7924634053, 1.0),
        ("temp", 42.7185297917, 1.0),
    ] {
        close(
            get(
                &t,
                "Tests of Model Effects",
                row,
                "Type III / Wald Chi-Square",
            ),
            chi,
            1e-6,
            row,
        );
        assert_eq!(get(&t, "Tests of Model Effects", row, "Type III / df"), df);
    }
    assert_eq!(get(&t, "Case Processing Summary", "Excluded", "N"), 0.0);
    assert_eq!(
        get(
            &t,
            "Categorical Variable Information",
            "Factor / zone / north",
            "N"
        ),
        18.0
    );
    close(
        get(
            &t,
            "Continuous Variable Information",
            "Offset / lnh",
            "Maximum",
        ),
        78f64.ln(),
        1e-12,
        "offset max",
    );
}

#[test]
fn poisson_likelihood_ratio_tests_of_type_one_and_three() {
    let source = format!(
        "{COUNTS}GENLIN repairs BY shop zone WITH temp
  /MODEL shop zone temp DISTRIBUTION=POISSON OFFSET=lnh
  /CRITERIA ANALYSISTYPE=ALL(LR)
  /PRINT SUMMARY.
"
    );
    let t = tables_of(&source);
    let title = "Tests of Model Effects";
    // Type I adds the effects in order, starting from the model with no parameters
    for (row, chi) in [
        ("(Intercept)", 1322.74847714),
        ("shop", 78.6191950474),
        ("zone", 27.3608720367),
        ("temp", 41.7458164682),
    ] {
        close(
            get(&t, title, row, "Type I / Likelihood Ratio Chi-Square"),
            chi,
            1e-6,
            row,
        );
    }
    for (row, chi, df) in [
        ("(Intercept)", 272.781148487, 1.0),
        ("shop", 89.50535793, 2.0),
        ("zone", 39.13430519, 1.0),
        ("temp", 41.74581647, 1.0),
    ] {
        close(
            get(&t, title, row, "Type III / Likelihood Ratio Chi-Square"),
            chi,
            1e-6,
            row,
        );
        assert_eq!(get(&t, title, row, "Type III / df"), df);
    }
    assert!(t.iter().all(|t| t.title != PE), "PRINT names only SUMMARY");
}

#[test]
fn estimated_marginal_means_on_the_response_scale_with_pairwise_comparisons() {
    let source = format!(
        "{COUNTS}GENLIN repairs BY shop zone WITH temp
  /MODEL shop zone temp DISTRIBUTION=POISSON OFFSET=lnh
  /PRINT NONE
  /EMMEANS TABLES=shop COMPARE=shop
  /EMMEANS TABLES=shop SCALE=TRANSFORMED COMPARE=shop PADJUST=BONFERRONI.
"
    );
    let t = tables_of(&source);
    let emm = "Estimated Marginal Means: shop";
    let means = [0.429188297524, 0.164760505386, 0.256865990177];
    let ses = [0.024832961727, 0.0151709523942, 0.02060564689];
    for (i, (m, se)) in means.iter().zip(&ses).enumerate() {
        let row = format!("{}", i + 1);
        close(get(&t, emm, &row, "Mean"), *m, 1e-7, "mean");
        close(get(&t, emm, &row, SE), *se, 1e-6, "se");
    }
    let pairs = "Pairwise Comparisons: shop";
    close(
        get(&t, pairs, "1 / 2", "Mean Difference (I-J)"),
        0.264427792138,
        1e-7,
        "1-2",
    );
    close(get(&t, pairs, "1 / 2", SE), 0.0287682978811, 1e-6, "1-2 se");
    close(
        get(&t, pairs, "2 / 3", "Sig."),
        0.000194656489143,
        1e-4,
        "2-3 p",
    );
    close(
        get(&t, pairs, "3 / 2", "Mean Difference (I-J)"),
        0.092105484791,
        1e-7,
        "3-2",
    );
    close(
        get(&t, "Overall Test Results: shop", "", "Wald Chi-Square"),
        85.4950944472,
        1e-6,
        "overall",
    );
    // the second EMMEANS: means of the linear predictor, Bonferroni over the three pairs
    let second: Vec<&Table> = t.iter().filter(|t| t.title == emm).collect();
    assert_eq!(second.len(), 2);
    let log_mean = number(&second[1].rows[0][1]);
    close(log_mean, means[0].ln(), 1e-7, "transformed mean");
    let p_second: Vec<&Table> = t.iter().filter(|t| t.title == pairs).collect();
    let sig = p_second[1]
        .column_titles()
        .iter()
        .position(|c| c == "Sig.")
        .unwrap();
    let p = number(&p_second[1].rows[3][sig]);
    assert!(p > 0.0 && p <= 1.0);
}

#[test]
fn saved_predictions_and_residuals_of_a_poisson_model() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{COUNTS}GENLIN repairs BY shop zone WITH temp
  /MODEL shop zone temp DISTRIBUTION=POISSON OFFSET=lnh
  /PRINT NONE
  /SAVE XBPRED XBSTDERROR MEANPRED RESID PEARSONRESID DEVIANCERESID LEVERAGE COOK(cook).
"
        ),
    );
    for (name, expected) in [
        ("XBPredicted", [3.92401375365, 2.51967193815, 2.06313372718]),
        (
            "XBStandardError",
            [0.0745055999586, 0.0948665287583, 0.130181677004],
        ),
        (
            "MeanPredicted",
            [50.6031462719, 12.424519984, 7.87059550416],
        ),
        ("Residual", [46.3968537281, 0.575480016042, 5.12940449584]),
        (
            "PearsonResidual",
            [6.52228498605, 0.163264001869, 1.82836606283],
        ),
        (
            "DevianceResidual",
            [5.78286729624, 0.162027373913, 1.66984951283],
        ),
        ("Leverage", [0.280902337644, 0.111816434284, 0.133385099611]),
        ("cook", [4.62177859847, 0.000755633221734, 0.118743900997]),
    ] {
        let v = values(&engine, name);
        for (i, e) in expected.iter().enumerate() {
            close(v[i].unwrap(), *e, 1e-6, name);
        }
    }
}

#[test]
fn binary_logistic_models_the_first_value_unless_reference_says_otherwise() {
    let modeled_one = tables_of(&format!(
        "{SEEDS}GENLIN sprout (REFERENCE=FIRST) BY soil WITH moist
  /MODEL soil moist DISTRIBUTION=BINOMIAL LINK=LOGIT.
"
    ));
    let t = &modeled_one;
    for (row, b, se) in [
        ("(Intercept)", -2.519208059186, 1.122212999868),
        ("[soil=clay]", 2.065757097307, 0.924673337554),
        ("[soil=loam]", 2.065757097307, 0.924673337554),
        ("moist", 0.441720689869, 0.184927274179),
    ] {
        close(get(t, PE, row, B), b, 1e-6, row);
        close(get(t, PE, row, SE), se, 1e-6, row);
    }
    // deviance and Pearson over the 12 subpopulations; the log likelihood adds the binomial
    // coefficients of the subpopulations to the package's Bernoulli log likelihood
    close(
        get(t, FIT, "Deviance", "Value"),
        17.777094874,
        1e-6,
        "deviance",
    );
    assert_eq!(get(t, FIT, "Deviance", "df"), 8.0);
    close(
        get(t, FIT, "Pearson Chi-Square", "Value"),
        27.1386218848,
        1e-6,
        "pearson",
    );
    close(
        get(t, FIT, "Log Likelihood", "Value"),
        -14.1846077768,
        1e-7,
        "ll",
    );
    close(
        get(t, FIT, "Bayesian Information Criterion (BIC)", "Value"),
        43.8540195972,
        1e-7,
        "bic",
    );
    close(
        get(t, FIT, "Finite Sample Corrected AIC (AICC)", "Value"),
        37.2994481117,
        1e-7,
        "aicc",
    );
    close(
        get(t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        13.8086138541,
        1e-6,
        "omnibus",
    );
    close(
        get(
            t,
            "Tests of Model Effects",
            "soil",
            "Type III / Wald Chi-Square",
        ),
        6.85407421762,
        1e-6,
        "soil",
    );
    close(
        get(
            t,
            "Tests of Model Effects",
            "(Intercept)",
            "Type III / Wald Chi-Square",
        ),
        1.71939871612,
        1e-6,
        "intercept",
    );
    // by default the last value (1) is the reference, so the signs turn
    let t = tables_of(&format!(
        "{SEEDS}GENLIN sprout BY soil WITH moist /MODEL soil moist DISTRIBUTION=BINOMIAL.
"
    ));
    close(get(&t, PE, "moist", B), -0.441720689869, 1e-6, "flipped");
    let info = find(&t, "Model Information");
    assert!(
        info.caption
            .as_deref()
            .unwrap()
            .contains("models 0 as the response")
    );
}

#[test]
fn probit_standard_errors_follow_the_method() {
    let source = |method: &str| {
        format!(
            "{SEEDS}GENLIN sprout (REFERENCE=FIRST) BY soil WITH moist
  /MODEL soil moist DISTRIBUTION=BINOMIAL LINK=PROBIT {method}
  /PRINT SOLUTION.
"
        )
    };
    // the default (Fisher scoring, then Newton-Raphson) ends with the observed information,
    // which the package does not report: compared with its numerical second derivative
    let t = tables_of(&source(""));
    for (row, se) in [
        ("(Intercept)", 0.6052878180880),
        ("[soil=clay]", 0.5406311841346),
        ("moist", 0.0956474044481),
    ] {
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    close(get(&t, PE, "moist", B), 0.230089603348, 1e-6, "probit b");
    // Fisher scoring throughout gives the package's expected information
    let t = tables_of(&source("/CRITERIA METHOD=FISHER"));
    for (row, se) in [
        ("(Intercept)", 0.6089000316098),
        ("[soil=clay]", 0.5215584903287),
        ("moist", 0.0992308962401),
    ] {
        close(get(&t, PE, row, SE), se, 1e-6, row);
    }
}

#[test]
fn events_over_trials_with_a_complementary_log_log_link() {
    let t = tables_of(&format!(
        "{TRAYS}GENLIN ok OF planted BY soil WITH moist
  /MODEL soil moist LINK=CLOGLOG.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", -1.452806933210, 0.6058354762767),
        ("[soil=clay]", 1.227014713695, 0.5353231944305),
        ("[soil=loam]", 0.902706189310, 0.5004605144387),
        ("moist", 0.184726804478, 0.0877003633269),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    close(
        get(&t, FIT, "Deviance", "Value"),
        19.9539479459,
        1e-6,
        "deviance",
    );
    close(
        get(&t, FIT, "Pearson Chi-Square", "Value"),
        20.7442955886,
        1e-6,
        "pearson",
    );
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -15.2730343127,
        1e-7,
        "ll",
    );
    // N is the number of trials, 48, not the 12 cases
    close(
        get(&t, FIT, "Bayesian Information Criterion (BIC)", "Value"),
        46.0308726691,
        1e-7,
        "bic",
    );
    close(
        get(&t, FIT, "Finite Sample Corrected AIC (AICC)", "Value"),
        39.4763011836,
        1e-7,
        "aicc",
    );
}

#[test]
fn binary_and_events_over_trials_forms_give_the_same_fit_statistics() {
    let binary = tables_of(&format!(
        "{SEEDS}GENLIN sprout (REFERENCE=FIRST) BY soil WITH moist /MODEL soil moist DISTRIBUTION=BINOMIAL.
"
    ));
    let events = tables_of(&format!(
        "{TRAYS}GENLIN ok OF planted BY soil WITH moist /MODEL soil moist.
"
    ));
    for row in [
        "Deviance",
        "Pearson Chi-Square",
        "Log Likelihood",
        "Akaike's Information Criterion (AIC)",
        "Bayesian Information Criterion (BIC)",
    ] {
        close(
            get(&binary, FIT, row, "Value"),
            get(&events, FIT, row, "Value"),
            1e-9,
            row,
        );
    }
    close(
        get(&binary, PE, "moist", B),
        get(&events, PE, "moist", B),
        1e-9,
        "b",
    );
}

#[test]
fn gamma_log_link_with_the_scale_by_maximum_likelihood() {
    let t = tables_of(&format!(
        "{COSTS}GENLIN cost BY crew WITH size
  /MODEL crew size DISTRIBUTION=GAMMA LINK=LOG.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", 2.965321195074, 0.2460265111636),
        ("[crew=1]", 0.385331299198, 0.1737440973894),
        ("size", 0.122267849099, 0.0430792248538),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    close(get(&t, PE, "(Scale)", B), 0.226346730543, 1e-6, "scale");
    close(get(&t, PE, "(Scale)", SE), 0.056375026595, 1e-5, "scale se");
    close(
        get(&t, FIT, "Deviance", "Value"),
        7.04528416335,
        1e-6,
        "deviance",
    );
    close(
        get(&t, FIT, "Scaled Deviance", "Value"),
        7.04528416335 / 0.226346730543,
        1e-6,
        "scaled",
    );
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -130.662267318,
        1e-7,
        "ll",
    );
    // the scale counts as a parameter
    close(
        get(&t, FIT, "Akaike's Information Criterion (AIC)", "Value"),
        269.324534637,
        1e-7,
        "aic",
    );
    close(
        get(&t, FIT, "Consistent AIC (CAIC)", "Value"),
        278.929324163,
        1e-7,
        "caic",
    );
    close(
        get(&t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        11.2015319527,
        1e-5,
        "omnibus",
    );
}

#[test]
fn gamma_default_inverse_link_with_fisher_scoring() {
    // the package's standard errors are its expected information times its Pearson-based
    // scale; the test takes its unscaled matrix times the maximum likelihood scale
    let t = tables_of(&format!(
        "{COSTS}GENLIN cost BY crew WITH size
  /MODEL crew size DISTRIBUTION=GAMMA /CRITERIA METHOD=FISHER.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", 0.04121104144113, 0.005732176543898),
        ("[crew=1]", -0.00839158956384, 0.003973439273325),
        ("size", -0.00257573420630, 0.000832561081662),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    let info = find(&t, "Model Information");
    assert!(matches!(&info.rows[2][1], Cell::Text(l) if l == "Power(-1)"));
}

#[test]
fn inverse_gaussian_log_link() {
    let t = tables_of(&format!(
        "{COSTS}GENLIN cost BY crew WITH size
  /MODEL crew size DISTRIBUTION=IGAUSS LINK=LOG.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", 3.002759167094, 0.2796550598868),
        ("[crew=1]", 0.366420066404, 0.2052551589443),
        ("size", 0.116183551880, 0.0538745512894),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    close(get(&t, PE, "(Scale)", B), 0.00746090703506, 1e-6, "scale");
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -132.974188212,
        1e-7,
        "ll",
    );
    close(
        get(&t, FIT, "Deviance", "Value"),
        0.223827211052,
        1e-6,
        "deviance",
    );
}

#[test]
fn normal_identity_with_type_one_and_three_wald_tests() {
    let t = tables_of(&format!(
        "{COSTS}GENLIN cost BY crew WITH size
  /MODEL crew size /CRITERIA ANALYSISTYPE=ALL.
"
    ));
    // the maximum likelihood scale is the residual sum of squares over n
    let phi = 13090.25503 / 30.0;
    close(get(&t, PE, "(Scale)", B), phi, 1e-7, "scale");
    for (row, b, se) in [
        ("(Intercept)", 4.07653311960, 11.09496042635),
        ("[crew=1]", 18.97974501378, 7.63930534348),
        ("size", 6.42005309292, 1.92887425493),
    ] {
        close(get(&t, PE, row, B), b, 1e-7, row);
        close(get(&t, PE, row, SE), se, 1e-6, row);
    }
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -133.744545389,
        1e-8,
        "ll",
    );
    close(
        get(&t, FIT, "Akaike's Information Criterion (AIC)", "Value"),
        275.489090779,
        1e-8,
        "aic",
    );
    close(
        get(&t, FIT, "Bayesian Information Criterion (BIC)", "Value"),
        281.093880305,
        1e-8,
        "bic",
    );
    // Type I Wald chi-squares are the sequential sums of squares over the scale
    let title = "Tests of Model Effects";
    close(
        get(&t, title, "(Intercept)", "Type I / Wald Chi-Square"),
        140.0416263,
        1e-7,
        "t1 int",
    );
    close(
        get(&t, title, "crew", "Type I / Wald Chi-Square"),
        5.304520039,
        1e-7,
        "t1 crew",
    );
    close(
        get(&t, title, "size", "Type I / Wald Chi-Square"),
        11.0782052252,
        1e-7,
        "t1 size",
    );
    close(
        get(&t, title, "crew", "Type III / Wald Chi-Square"),
        6.17266927804,
        1e-7,
        "t3 crew",
    );
    close(
        get(&t, title, "size", "Type III / Wald Chi-Square"),
        11.0782052252,
        1e-7,
        "t3 size",
    );
}

#[test]
fn deviance_scale_gives_likelihood_ratio_chi_squares_and_f_tests() {
    let t = tables_of(&format!(
        "{COSTS}GENLIN cost BY crew WITH size
  /MODEL crew size /CRITERIA SCALE=DEVIANCE ANALYSISTYPE=3(LR).
"
    ));
    let phi = 484.824260215;
    close(get(&t, PE, "(Scale)", B), phi, 1e-8, "scale");
    // the standard errors are the usual least squares ones
    close(get(&t, PE, "[crew=1]", SE), 8.05253487563, 1e-7, "se");
    close(get(&t, PE, "size", SE), 2.03321198854, 1e-7, "se");
    let title = "Tests of Model Effects";
    close(
        get(&t, title, "crew", "Type III / Likelihood Ratio Chi-Square"),
        5.55540235024,
        1e-6,
        "crew",
    );
    close(
        get(&t, title, "size", "Type III / F"),
        9.97038470267,
        1e-6,
        "size F",
    );
    assert_eq!(get(&t, title, "size", "Type III / df2"), 27.0);
    close(
        get(&t, title, "size", "Type III / Sig. (F)"),
        0.003891,
        1e-3,
        "F p",
    );
    close(
        get(&t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        14.7444527378,
        1e-6,
        "omnibus",
    );
    // the log likelihood is at scale 1, the adjusted one at the estimate
    let rss = 13090.25503;
    let n = 30.0_f64;
    let at_one = -0.5 * rss - 0.5 * n * (2.0 * std::f64::consts::PI).ln();
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        at_one,
        1e-8,
        "ll at 1",
    );
    let adjusted = -0.5 * rss / phi - 0.5 * n * (phi.ln() + (2.0 * std::f64::consts::PI).ln());
    close(
        get(&t, FIT, "Adjusted Log Likelihood", "Value"),
        adjusted,
        1e-8,
        "adjusted",
    );
}

#[test]
fn negative_binomial_with_k_by_maximum_likelihood() {
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=NEGBIN(MLE) OFFSET=lnh.
"
    ));
    // the standard errors invert the joint information of b and ln k
    for (row, b, se) in [
        ("(Intercept)", -1.9378132406424, 0.3051993521463),
        ("[shop=1]", 0.3847826931921, 0.2220683947225),
        ("[shop=2]", -0.5105829980185, 0.2363107698291),
        ("temp", 0.0418464518189, 0.0168037053413),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-4, row);
    }
    close(
        get(&t, PE, "(Negative binomial)", B),
        0.23120338785,
        1e-6,
        "k",
    );
    close(
        get(&t, PE, "(Negative binomial)", SE),
        0.0709454472312,
        1e-4,
        "k se",
    );
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -121.739195694,
        1e-8,
        "ll",
    );
    close(
        get(&t, FIT, "Deviance", "Value"),
        36.9699727392,
        1e-6,
        "deviance",
    );
    // k counts against the degrees of freedom and as a parameter
    assert_eq!(get(&t, FIT, "Deviance", "df"), 31.0);
    close(
        get(&t, FIT, "Akaike's Information Criterion (AIC)", "Value"),
        253.478391388,
        1e-8,
        "aic",
    );
    close(
        get(&t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        14.8455552637,
        1e-5,
        "omnibus",
    );
}

#[test]
fn negative_binomial_with_k_fixed_at_its_default() {
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=NEGBIN OFFSET=lnh.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", -1.9420134230985, 0.558263541127),
        ("[shop=1]", 0.3631233663885, 0.421805323114),
        ("temp", 0.0425706369157, 0.0318437254368),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-5, row);
    }
    assert_eq!(get(&t, PE, "(Negative binomial)", B), 1.0);
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -132.409110078,
        1e-8,
        "ll",
    );
    close(
        get(&t, FIT, "Deviance", "Value"),
        11.1636012453,
        1e-6,
        "deviance",
    );
}

#[test]
fn poisson_with_the_pearson_scale() {
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh
  /CRITERIA SCALE=PEARSON ANALYSISTYPE=3(LR).
"
    ));
    let phi = 6.3259974034;
    close(get(&t, PE, "(Scale)", B), phi, 1e-8, "scale");
    close(get(&t, PE, "[shop=1]", SE), 0.242883172868, 1e-6, "se");
    close(get(&t, PE, "temp", SE), 0.017460978198, 1e-6, "se");
    let title = "Tests of Model Effects";
    close(
        get(&t, title, "temp", "Type III / Likelihood Ratio Chi-Square"),
        4.73796958932,
        1e-6,
        "temp",
    );
    assert_eq!(get(&t, title, "temp", "Type III / df2"), 32.0);
}

#[test]
fn ordinal_multinomial_with_the_cumulative_logit() {
    let t = tables_of(&format!(
        "{TASTES}GENLIN taste BY recipe WITH sugar
  /MODEL recipe sugar DISTRIBUTION=MULTINOMIAL LINK=CUMLOGIT.
"
    ));
    for (row, b, se) in [
        ("Threshold [taste = 1]", 0.256019291702, 0.695998239641),
        ("Threshold [taste = 2]", 1.542832570574, 0.730984400431),
        ("Threshold [taste = 3]", 2.259749197198, 0.775353203611),
        ("[recipe=1]", 0.892150292653, 0.674678933480),
        ("[recipe=2]", -0.185342770666, 0.673254947202),
        ("sugar", 0.301377208237, 0.218343622218),
    ] {
        close(get(&t, PE, row, B), b, 1e-5, row);
        close(get(&t, PE, row, SE), se, 1e-4, row);
    }
    // the full log likelihood adds the multinomial constant of the 41 subpopulations
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -55.6199928309,
        1e-7,
        "ll",
    );
    close(
        get(&t, FIT, "Akaike's Information Criterion (AIC)", "Value"),
        123.239985662,
        1e-7,
        "aic",
    );
    close(
        get(&t, FIT, "Bayesian Information Criterion (BIC)", "Value"),
        134.0799606,
        1e-7,
        "bic",
    );
    close(
        get(&t, FIT, "Deviance", "Value"),
        107.081102578,
        1e-6,
        "deviance",
    );
    close(
        get(&t, FIT, "Pearson Chi-Square", "Value"),
        123.546552678,
        1e-6,
        "pearson",
    );
    assert_eq!(get(&t, FIT, "Deviance", "df"), 117.0);
    close(
        get(&t, "Omnibus Test", "", "Likelihood Ratio Chi-Square"),
        4.23641743803,
        1e-6,
        "omnibus",
    );
    close(
        get(
            &t,
            "Tests of Model Effects",
            "recipe",
            "Type III / Wald Chi-Square",
        ),
        2.81381098334,
        1e-4,
        "recipe",
    );
    close(
        get(
            &t,
            "Tests of Model Effects",
            "sugar",
            "Type III / Wald Chi-Square",
        ),
        1.90519656374,
        1e-4,
        "sugar",
    );
    // the kernel leaves the constant out
    let k = tables_of(&format!(
        "{TASTES}GENLIN taste BY recipe WITH sugar
  /MODEL recipe sugar DISTRIBUTION=MULTINOMIAL /CRITERIA LIKELIHOOD=KERNEL /PRINT FIT.
"
    ));
    close(
        get(&k, FIT, "Log Likelihood", "Value"),
        -57.6994343726,
        1e-7,
        "kernel",
    );
}

#[test]
fn ordinal_saved_cumulative_probabilities_and_predicted_category() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{TASTES}GENLIN taste BY recipe WITH sugar
  /MODEL recipe sugar DISTRIBUTION=MULTINOMIAL /PRINT NONE
  /SAVE MEANPRED(cum:2) XBPRED PREDVAL.
"
        ),
    );
    // case 1: recipe 1, sugar 1.7
    let eta: f64 = 0.892150292653 + 0.301377208237 * 1.7;
    let gamma1 = 1.0 / (1.0 + (-(0.256019291702 - eta)).exp());
    close(
        values(&engine, "cum_1")[0].unwrap(),
        gamma1,
        1e-5,
        "cumulative 1",
    );
    assert!(
        engine.dataset().unwrap().variable("cum_3").is_err(),
        "only two asked for"
    );
    close(
        values(&engine, "XBPredicted_3")[0].unwrap(),
        2.259749197198 - eta,
        1e-5,
        "xb 3",
    );
    let predicted = values(&engine, "PredictedValue");
    assert!(
        predicted
            .iter()
            .all(|p| matches!(p, Some(v) if (1.0..=4.0).contains(v)))
    );
}

#[test]
fn a_frequency_weight_of_two_is_the_case_twice() {
    let weighted = tables_of(&format!(
        "{COUNTS}COMPUTE w = 1 + (shop = 2).
WEIGHT BY w.
GENLIN repairs BY shop WITH temp /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh.
"
    ));
    // the same records with every shop 2 record written twice
    let body = COUNTS
        .split("BEGIN DATA.\n")
        .nth(1)
        .unwrap()
        .split("END DATA.")
        .next()
        .unwrap();
    let words: Vec<&str> = body.split_whitespace().collect();
    let mut doubled = String::new();
    for record in words.chunks(5) {
        let times = if record[1] == "2" { 2 } else { 1 };
        for _ in 0..times {
            doubled.push_str(&record.join(" "));
            doubled.push('\n');
        }
    }
    let doubled = tables_of(&format!(
        "DATA LIST FREE /repairs shop (F1) zone (A5) hours temp.
BEGIN DATA.
{doubled}END DATA.
COMPUTE lnh = LN(hours).
GENLIN repairs BY shop WITH temp /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh.
"
    ));
    assert_eq!(
        get(&weighted, "Case Processing Summary", "Included", "N"),
        48.0
    );
    for row in ["(Intercept)", "[shop=2]", "temp"] {
        close(
            get(&weighted, PE, row, B),
            get(&doubled, PE, row, B),
            1e-9,
            row,
        );
        close(
            get(&weighted, PE, row, SE),
            get(&doubled, PE, row, SE),
            1e-9,
            row,
        );
    }
    for row in [
        "Deviance",
        "Log Likelihood",
        "Bayesian Information Criterion (BIC)",
    ] {
        close(
            get(&weighted, FIT, row, "Value"),
            get(&doubled, FIT, row, "Value"),
            1e-9,
            row,
        );
    }
}

#[test]
fn errors_for_combinations_the_manual_does_not_allow() {
    assert!(
        run_error(&format!(
            "{COUNTS}GENLIN repairs /MODEL DISTRIBUTION=POISSON LINK=LOGIT.\n"
        ))
        .contains("LINK=LOGIT cannot be used with DISTRIBUTION=POISSON")
    );
    assert!(
        run_error(&format!(
            "{COUNTS}GENLIN repairs /MODEL DISTRIBUTION=POISSON /CRITERIA SCALE=MLE.\n"
        ))
        .contains("SCALE=MLE")
    );
    assert!(
        run_error(&format!(
            "{COUNTS}GENLIN repairs BY shop /MODEL shop /REPEATED SUBJECT=shop.\n"
        ))
        .contains("not supported yet")
    );
    assert!(
        run_error(&format!(
            "{SEEDS}GENLIN moist /MODEL DISTRIBUTION=BINOMIAL.\n"
        ))
        .contains("exactly two values")
    );
    assert!(
        run_error(&format!("{COUNTS}GENLIN repairs BY shop WITH shop.\n"))
            .contains("more than one role")
    );
}

#[test]
fn robust_covariance_is_the_sandwich() {
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh /CRITERIA COVB=ROBUST.
"
    ));
    for (row, se) in [
        ("(Intercept)", 0.339572601801),
        ("[shop=1]", 0.282597734857),
        ("[shop=2]", 0.217589410915),
        ("temp", 0.018126695671),
    ] {
        close(get(&t, PE, row, SE), se, 1e-6, row);
    }
    close(
        get(
            &t,
            "Tests of Model Effects",
            "shop",
            "Type III / Wald Chi-Square",
        ),
        11.2690683856,
        1e-6,
        "robust wald",
    );
}

#[test]
fn simple_contrast_of_linear_predictor_means_with_sequential_bonferroni() {
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh /PRINT NONE
  /EMMEANS TABLES=shop CONTROL=temp(14) SCALE=TRANSFORMED COMPARE=shop CONTRAST=SIMPLE(1)
    PADJUST=SEQBONFERRONI.
"
    ));
    let title = "Individual Test Results: shop";
    close(
        get(&t, title, "2 vs. 1", "Contrast Estimate"),
        -0.940604419225,
        1e-7,
        "2 vs 1",
    );
    close(get(&t, title, "2 vs. 1", SE), 0.106634298216, 1e-6, "se");
    close(
        get(&t, title, "3 vs. 1", "Contrast Estimate"),
        -0.485319003035,
        1e-7,
        "3 vs 1",
    );
    // the smaller p is doubled, the larger kept
    close(
        get(&t, title, "2 vs. 1", "Sig."),
        2.27202098934e-18,
        1e-4,
        "adjusted",
    );
    close(
        get(&t, title, "3 vs. 1", "Sig."),
        5.01665653789e-07,
        1e-4,
        "adjusted",
    );
    close(
        get(&t, "Overall Test Results: shop", "", "Wald Chi-Square"),
        82.8810877371,
        1e-6,
        "overall",
    );
}

#[test]
fn a_scale_weight_divides_the_variance() {
    let t = tables_of(&format!(
        "{COSTS}COMPUTE w = size / 4.9166666666666667.
GENLIN cost BY crew WITH size /MODEL crew size SCALEWEIGHT=w.
"
    ));
    for (row, b, se) in [
        ("(Intercept)", -4.13037335591, 11.8945663948),
        ("[crew=1]", 22.0055337334, 7.26548626547),
        ("size", 7.59743855706, 1.91716506134),
    ] {
        close(get(&t, PE, row, B), b, 1e-6, row);
        close(get(&t, PE, row, SE), se, 1e-6, row);
    }
    close(get(&t, PE, "(Scale)", B), 394.499710599, 1e-6, "scale");
    close(
        get(&t, FIT, "Log Likelihood", "Value"),
        -133.691752816,
        1e-7,
        "ll",
    );
}

#[test]
fn binary_predicted_category_and_mean_intervals() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{SEEDS}GENLIN sprout (REFERENCE=FIRST) BY soil WITH moist
  /MODEL soil moist DISTRIBUTION=BINOMIAL /PRINT NONE
  /SAVE MEANPRED CIMEANPREDL CIMEANPREDU PREDVAL.
"
        ),
    );
    let mean = values(&engine, "MeanPredicted");
    let low = values(&engine, "CIMeanPredictedLower");
    let high = values(&engine, "CIMeanPredictedUpper");
    let predicted = values(&engine, "PredictedValue");
    // case 1: clay at moisture 2
    let eta: f64 = -2.519208059186 + 2.065757097307 + 0.441720689869 * 2.0;
    close(mean[0].unwrap(), 1.0 / (1.0 + (-eta).exp()), 1e-6, "mean");
    for i in 0..mean.len() {
        let (m, l, h) = (mean[i].unwrap(), low[i].unwrap(), high[i].unwrap());
        assert!(l < m && m < h && l > 0.0 && h < 1.0);
        assert_eq!(predicted[i].unwrap(), if m >= 0.5 { 1.0 } else { 0.0 });
    }
}

#[test]
fn factor_order_class_missing_and_excluded_cases() {
    // descending order makes the lowest shop the redundant one
    let t = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop (ORDER=DESCENDING) WITH temp
  /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh /PRINT SOLUTION.
"
    ));
    assert_eq!(get(&t, PE, "[shop=1]", B), 0.0);
    let ascending = tables_of(&format!(
        "{COUNTS}GENLIN repairs BY shop WITH temp
  /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh /PRINT SOLUTION.
"
    ));
    close(
        get(&t, PE, "[shop=3]", B),
        -get(&ascending, PE, "[shop=1]", B),
        1e-9,
        "turned",
    );
    // shop 3 user-missing: left out unless CLASSMISSING=INCLUDE; a negative count is left out
    let source = |missing: &str| {
        format!(
            "{COUNTS}MISSING VALUES shop (3).
IF ($CASENUM = 2) repairs = -1.
GENLIN repairs BY shop WITH temp /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh {missing}.
"
        )
    };
    let t = tables_of(&source(""));
    assert_eq!(get(&t, "Case Processing Summary", "Included", "N"), 23.0);
    assert_eq!(get(&t, "Case Processing Summary", "Excluded", "N"), 13.0);
    let t = tables_of(&source("/MISSING CLASSMISSING=INCLUDE"));
    assert_eq!(get(&t, "Case Processing Summary", "Included", "N"), 35.0);
}

#[test]
fn complete_separation_gives_a_warning() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST FREE /ok dose.
BEGIN DATA.
0 1 0 2 0 3 1 4 1 5 1 6
END DATA.
GENLIN ok WITH dose /MODEL dose DISTRIBUTION=BINOMIAL /PRINT NONE.
",
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("complete separation")),
        "{:?}",
        engine.warnings()
    );
}

#[test]
fn intercept_only_normal_model_without_model_subcommand() {
    let t = tables_of(&format!("{COSTS}GENLIN cost.\n"));
    let mean = 1353.95 / 30.0;
    close(get(&t, PE, "(Intercept)", B), mean, 1e-9, "mean");
    assert_eq!(get(&t, "Omnibus Test", "", "df"), 0.0);
}

#[test]
fn a_short_polynomial_metric_leaves_out_the_comparisons_only() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{COUNTS}GENLIN repairs BY shop WITH temp /MODEL shop temp DISTRIBUTION=POISSON OFFSET=lnh
  /EMMEANS TABLES=shop COMPARE=shop CONTRAST=POLYNOMIAL(1 2).
"
        ),
    );
    assert!(t.iter().any(|t| t.title == PE));
    assert!(
        t.iter()
            .any(|t| t.title == "Estimated Marginal Means: shop")
    );
    assert!(
        t.iter()
            .all(|t| !t.title.starts_with("Individual Test Results"))
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("POLYNOMIAL metric"))
    );
}

#[test]
fn events_predictions_reach_cases_outside_the_fit_and_impossible_values_are_missing() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{TRAYS}IF ($CASENUM = 1) ok = $SYSMIS.
GENLIN ok OF planted BY soil WITH moist /MODEL soil moist /PRINT NONE
  /SAVE MEANPRED RESID.
GENLIN ok OF planted /MODEL INTERCEPT=NO OFFSET=0 /PRINT NONE /SAVE COOK.
"
        ),
    );
    // the first tray has no count of sprouted seeds: a predicted number, no residual
    let mean = values(&engine, "MeanPredicted");
    assert!(mean[0].is_some_and(|m| m > 0.0 && m < 4.0));
    assert!(values(&engine, "Residual")[0].is_none());
    // a model with no parameters has no Cook's distance
    assert!(values(&engine, "CooksDistance").iter().all(Option::is_none));
}
