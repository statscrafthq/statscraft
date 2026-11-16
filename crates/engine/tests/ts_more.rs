//! SPECTRA, AREG and 2SLS on invented series. The expected numbers come from a general
//! statistics package run as a black box: its raw periodogram (half of ours), its smoothing
//! kernels, coherency and phase; its regression with AR(1) errors by exact likelihood; and its
//! least squares fits, from which the Prais-Winsten and Cochrane-Orcutt iterations and the two
//! stages of 2SLS were put together.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Two days of hourly tide heights at a pier (meters) and boats passing the harbor mouth.
const TIDE: &str = "3.1 4.6 5.2 4.0 2.5 1.4 1.9 3.3 4.8 5.5 4.4 2.9 1.6 1.8 3.0 4.5 5.3 4.6 3.0 \
1.7 1.5 2.8 4.2 5.0";
const BOATS: &str = "12 15 18 16 11 9 10 13 17 19 16 12 8 10 12 16 18 17 12 9 8 11 15 17";

/// Thirty months of outdoor temperature and the power drawn by a greenhouse heater.
const TEMP: &str = "10.8 16.3 18 14 14.3 9.6 4 3.1 1.7 3.9 5.3 13.8 12.8 16.7 18.1 16.3 12.9 8 \
4.9 2.3 1.4 1.5 4.9 9.7 14 18.2 20.1 18.1 14.3 7.8";
const POWER: &str = "88.7 74.4 70 85.3 88.8 99.5 116.7 114.9 116.5 112.2 104.6 84.7 84.5 77.4 \
73 75.9 86.5 100.9 109.3 117.3 115.9 114.2 107.5 93.7 83.5 74.4 72.3 76.4 82.3 99.3";

/// Twenty farm plots: yield, fertilizer used, the subsidy rate, soil acidity and rainfall.
const PLOTS: &str = "68.0 16.2 1.6 6.0 58\n81.6 30.0 8.6 7.2 21\n96.7 33.4 8.3 5.6 53\n\
71.4 18.0 1.1 5.7 58\n91.1 28.8 6.7 6.3 54\n81.0 25.5 5.5 7.0 40\n83.7 29.7 7.7 6.2 39\n\
73.6 22.1 5.4 6.6 46\n80.2 24.8 4.0 5.8 42\n114.2 40.3 8.5 6.7 53\n91.8 29.6 6.0 6.9 53\n\
70.5 21.8 4.9 5.7 39\n95.4 33.7 8.9 6.0 43\n85.3 25.3 4.3 6.1 56\n74.8 22.3 2.9 6.5 32\n\
75.3 20.8 2.9 6.6 46\n70.9 20.4 5.6 6.2 52\n88.5 29.2 8.3 6.2 55\n67.6 19.6 2.1 6.3 22\n\
69.4 24.6 7.8 6.8 27";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
    engine: Engine,
}

/// The values of each series side by side, one case per line.
fn columns(names: &str, series: &[&str]) -> String {
    let split: Vec<Vec<&str>> = series
        .iter()
        .map(|s| s.split_whitespace().collect())
        .collect();
    let rows: Vec<String> = (0..split[0].len())
        .map(|i| split.iter().map(|s| s[i]).collect::<Vec<_>>().join(" "))
        .collect();
    format!(
        "DATA LIST FREE /{names}.\nBEGIN DATA\n{}\nEND DATA.\n",
        rows.join("\n")
    )
}

fn run(source: &str) -> Run {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    let warnings = engine.warnings().to_vec();
    Run {
        tables,
        warnings,
        engine,
    }
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error")
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
        other => panic!("not a number: {other:?}"),
    }
}

/// The value in the row whose first cell is `name`, at column `at`.
fn row_value(table: &Table, name: &str, at: usize) -> f64 {
    let row = table
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
        .unwrap_or_else(|| panic!("no row {name}"));
    num(&row[at])
}

/// The column named `header` of a frequency table, by K.
fn by_frequency(table: &Table, header: &str) -> Vec<f64> {
    let at = table
        .columns
        .iter()
        .position(|c| c == header)
        .unwrap_or_else(|| panic!("no column {header}"));
    table.rows.iter().map(|r| num(&r[at])).collect()
}

fn variable(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    let v = data.variable(name).expect("the variable");
    let values = v.numeric().expect("numeric");
    (0..data.case_count()).map(|k| values.get(k)).collect()
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

// ---------------------------------------------------------------- SPECTRA

#[test]
fn spectra_periodogram_and_default_window() {
    let r = run(&format!(
        "{}SPECTRA VARIABLES=tide.\n",
        columns("tide", &[TIDE])
    ));
    let t = find(&r.tables, "Univariate Statistics");
    assert_eq!(t.rows.len(), 13);
    let p = by_frequency(t, "Periodogram");
    // twice the reference's raw periodogram
    for (k, expected) in [
        (1, 0.2094007),
        (3, 27.0219468),
        (6, 0.7416667),
        (12, 0.2133333),
    ] {
        assert!(close(p[k], expected, 1e-6), "{k}: {}", p[k]);
    }
    // 2 n times the squared mean at frequency 0; the cosine term there is twice the mean
    assert!(close(p[0], 568.5633, 1e-3));
    assert!(close(
        by_frequency(t, "Cosine Transform")[0],
        6.883333,
        1e-6
    ));
    assert!(close(by_frequency(t, "Sine Transform")[3], 0.5568317, 1e-6));
    let s = by_frequency(t, "Spectral Density Estimate");
    // Tukey-Hamming of span 5
    for (k, expected) in [(0, 0.2935341412), (3, 14.8146768136), (12, 0.1862878980)] {
        assert!(close(s[k], expected, 1e-8), "{k}: {}", s[k]);
    }
    let model = find(&r.tables, "Model Description");
    assert!(close(row_value(model, "Weight W(0)", 1), 0.448, 1e-12));
    assert!(close(
        row_value(model, "Weight W(-2)", 1),
        0.03514087304,
        1e-10
    ));
    // the periodogram by frequency and by period
    let charts = r.engine.clone().take_charts();
    assert_eq!(charts.len(), 2);
    assert!(close(
        charts[0].series[0].values[3],
        27.0219468f64.ln(),
        1e-6
    ));
    assert_eq!(charts[1].categories.len(), 12);
}

#[test]
fn spectra_daniell_matches_the_reference_inside_and_mirrors_at_the_ends() {
    let r = run(&format!(
        "{}SPECTRA VARIABLES=tide /WINDOW=DANIELL(6) /PLOT=S BY FREQ.\n",
        columns("tide", &[TIDE])
    ));
    let s = by_frequency(
        find(&r.tables, "Univariate Statistics"),
        "Spectral Density Estimate",
    );
    // span 6 runs as 5; the reference agrees up to K = 10, where its own wrapping starts
    for (k, expected) in [(1, 5.8113276), (4, 8.1076266), (10, 0.2211575)] {
        assert!(close(s[k], expected, 1e-6), "{k}: {}", s[k]);
    }
    // past n / 2 the values mirror as l_(n + 1 - K): 0.2 (I_10 + I_11 + 2 I_12 + I_11)
    assert!(close(s[12], 0.1910582, 1e-6), "{}", s[12]);
    assert_eq!(r.engine.clone().take_charts().len(), 1);
}

#[test]
fn spectra_cross_statistics() {
    let r = run(&format!(
        "{}SPECTRA VARIABLES=tide boats /CENTER /CROSS /WINDOW=TUKEY(5) /PLOT=K PH.\n",
        columns("tide boats", &[TIDE, BOATS])
    ));
    let t = find(&r.tables, "Bivariate Statistics");
    let k2 = by_frequency(t, "Squared Coherency");
    let phase = by_frequency(t, "Phase Spectrum Estimate");
    let gain = by_frequency(t, "Gain");
    // coherency and phase agree with the reference
    assert!(close(k2[3], 0.9943881663, 1e-9));
    assert!(close(phase[7], 0.23809298715, 1e-9));
    assert!(close(gain[3], 2.508139784, 1e-8));
    // near frequency 0 the mirrored values
    assert!(close(k2[0], 0.9138690758, 1e-9));
    assert!(close(
        by_frequency(t, "Quadrature Spectrum Estimate")[10],
        -0.04558340606,
        1e-10
    ));
    assert!(close(
        by_frequency(t, "Cospectral Density Estimate")[2],
        19.7314746763,
        1e-8
    ));
    // centered: nothing at frequency 0
    let p = by_frequency(&r.tables[1], "Periodogram");
    assert!(p[0].abs() < 1e-9);
    // K and PH by frequency and by period
    assert_eq!(r.engine.clone().take_charts().len(), 4);
}

#[test]
fn spectra_odd_series_drops_the_first_case() {
    let odd = format!("9.9 {TIDE}");
    let r = run(&format!("{}SPECTRA tide.\n", columns("tide", &[&odd])));
    let model = find(&r.tables, "Model Description");
    assert!(
        model
            .caption
            .as_deref()
            .unwrap_or("")
            .contains("first case")
    );
    let p = by_frequency(find(&r.tables, "Univariate Statistics"), "Periodogram");
    assert!(close(p[3], 27.0219468, 1e-6));
}

#[test]
fn spectra_save_and_apply() {
    let r = run(&format!(
        "{}COMPUTE pg_2 = 1.\nSPECTRA VARIABLES=tide boats /CROSS /SAVE=FREQ(fr) P(pg) CS(co) K(coh).\n",
        columns("tide boats", &[TIDE, BOATS])
    ));
    let fr = variable(&r.engine, "fr");
    assert_eq!(fr.len(), 24);
    assert!(close(fr[6].unwrap(), 0.25, 1e-12));
    assert_eq!(fr[13], None);
    let pg = variable(&r.engine, "pg_1");
    assert!(close(pg[3].unwrap(), 27.0219468, 1e-6));
    assert!(variable(&r.engine, "co_1_2")[0].is_some());
    assert!(variable(&r.engine, "coh_1_2")[12].is_some());
    // pg_2 existed, so it was not saved
    assert_eq!(variable(&r.engine, "pg_2")[0], Some(1.0));
    assert!(
        r.warnings.iter().any(|w| w.contains("pg_2 exists already")),
        "{:?}",
        r.warnings
    );
    // APPLY keeps the series and CROSS, but not SAVE
    let mut engine = r.engine;
    let (commands, _) = parse("SPECTRA APPLY /PLOT=NONE.\n");
    let tables = engine.execute(&commands[0].command).unwrap();
    assert!(tables.iter().any(|t| t.title == "Bivariate Statistics"));
    assert!(engine.dataset().unwrap().variable("fr_1").is_err());
}

#[test]
fn spectra_own_weights_and_errors() {
    let tide = columns("tide", &[TIDE]);
    let r = run(&format!("{tide}SPECTRA tide /WINDOW=1 2 1.\n"));
    let s = by_frequency(
        find(&r.tables, "Univariate Statistics"),
        "Spectral Density Estimate",
    );
    let p = by_frequency(find(&r.tables, "Univariate Statistics"), "Periodogram");
    assert!(close(s[5], 0.25 * p[4] + 0.5 * p[5] + 0.25 * p[6], 1e-12));
    let gap = columns("tide", &[&TIDE.replacen("4.6", ".", 1)]);
    assert!(error(&format!("{gap}SPECTRA tide.\n")).contains("missing values"));
    let short = columns("tide", &["1 2 3 4 5"]);
    assert!(error(&format!("{short}SPECTRA tide.\n")).contains("six cases"));
    assert!(error(&format!("{tide}SPECTRA tide /WINDOW=1 0 1.\n")).contains("middle"));
    assert!(error(&format!("{tide}SPECTRA APPLY.\n")).contains("no earlier"));
}

// ---------------------------------------------------------------- AREG

fn heater() -> String {
    columns("temp power", &[TEMP, POWER])
}

#[test]
fn areg_prais_winsten() {
    let r = run(&format!("{}AREG power WITH temp.\n", heater()));
    let est = find(&r.tables, "Parameter Estimates");
    assert!(close(row_value(est, "Rho (AR1)", 1), 0.5458407613, 1e-6));
    assert!(close(row_value(est, "Rho (AR1)", 2), 0.1612518003, 1e-6));
    assert!(close(row_value(est, "temp", 1), -2.581026749, 1e-6));
    assert!(close(row_value(est, "temp", 2), 0.1100718113, 1e-6));
    assert!(close(row_value(est, "Constant", 1), 120.415702638, 1e-5));
    assert!(close(row_value(est, "Constant", 2), 1.4545700174, 1e-6));
    let history = find(&r.tables, "Iteration History");
    assert_eq!(history.rows.len(), 6);
    let last = history.rows.last().unwrap();
    assert!(close(num(&last[3]), 1.771287685, 1e-6));
    assert!(close(num(&last[4]), 5.270086773, 1e-6));
    let fit = variable(&r.engine, "FIT_1");
    let err = variable(&r.engine, "ERR_1");
    let sep = variable(&r.engine, "SEP_1");
    for (case, f, e, s) in [
        (0, 92.54061375, -3.2180077538, 2.880361611),
        (1, 76.24860310, -1.8486030959, 2.543794025),
        (29, 99.62485321, -0.3248532109, 2.478859263),
    ] {
        assert!(close(fit[case].unwrap(), f, 1e-5), "{case}");
        assert!(close(err[case].unwrap(), e, 1e-5), "{case}");
        assert!(close(sep[case].unwrap(), s, 1e-6), "{case}");
    }
}

/// A constant series has no residuals to measure rho from.
#[test]
fn areg_constant_series_is_an_error() {
    for method in ["PW", "CO"] {
        let message = error(&format!(
            "{}COMPUTE flat = 2.\nAREG flat WITH temp /METHOD={method}.\n",
            heater()
        ));
        assert!(message.contains("same value in every case"), "{message}");
    }
}

#[test]
fn areg_cochrane_orcutt_from_a_given_rho() {
    let r = run(&format!(
        "{}TSET PRINT=DETAILED.\nAREG power WITH temp /METHOD=CO /RHO=0.5.\n",
        heater()
    ));
    let est = find(&r.tables, "Parameter Estimates");
    assert!(close(row_value(est, "Rho (AR1)", 1), 0.536526113, 1e-6));
    assert!(close(row_value(est, "temp", 1), -2.580648837, 1e-6));
    assert!(close(row_value(est, "temp", 2), 0.106863215, 1e-6));
    assert!(close(row_value(est, "Constant", 2), 1.431737369, 1e-6));
    let history = find(&r.tables, "Iteration History");
    assert_eq!(history.rows.len(), 4);
    // DETAILED adds the coefficients
    assert_eq!(history.rows[0].len(), 7);
    let sep = variable(&r.engine, "SEP_1");
    assert!(close(sep[0].unwrap(), 2.805139327, 1e-6));
    assert!(close(sep[29].unwrap(), 2.431877847, 1e-6));
    let err = variable(&r.engine, "ERR_1");
    assert!(close(err[0].unwrap(), -3.6178710692, 1e-5));
}

#[test]
fn areg_exact_likelihood() {
    let r = run(&format!(
        "{}TSET CNVERGE=1E-7.\nAREG power WITH temp /METHOD=ML /MXITER=50.\n",
        heater()
    ));
    let est = find(&r.tables, "Parameter Estimates");
    // the likelihood is flat here: ours stops 3e-6 below the reference maximum
    assert!(close(row_value(est, "Rho (AR1)", 1), 0.5723153213, 5e-4));
    assert!(close(row_value(est, "temp", 1), -2.5755034093, 3e-4));
    assert!(close(row_value(est, "Constant", 1), 120.3378259089, 3e-3));
    assert!(close(row_value(est, "temp", 2), 0.1112573303, 3e-3));
    assert!(variable(&r.engine, "FIT_1")[5].is_some());
}

#[test]
fn areg_apply_fit_forecasts_and_errors() {
    // the last three months held back, forecast from the fit
    let r = run(&format!(
        "{}USE 1 THRU 27.\nPREDICT 28 THRU 30.\nAREG power WITH temp.\n\
         TSET NEWVAR=ALL.\nAREG APPLY FIT.\n",
        heater()
    ));
    let first = variable(&r.engine, "FIT_1");
    let second = variable(&r.engine, "FIT_2");
    for (a, b) in first.iter().zip(&second) {
        assert!(close(a.unwrap(), b.unwrap(), 1e-9));
    }
    // forecasts have no residual and a wider standard error
    let err = variable(&r.engine, "ERR_1");
    let sep = variable(&r.engine, "SEP_1");
    assert_eq!(err[28], None);
    assert!(sep[29].unwrap() > sep[27].unwrap());
    assert_eq!(find(&r.tables, "Iteration History").rows.len(), 1);
    // APPLY INITIAL starts from the earlier rho unless RHO is given again
    let r = run(&format!(
        "{}AREG power WITH temp.\nAREG APPLY INITIAL /RHO=0.2 /MXITER=1.\n",
        heater()
    ));
    let history = find(&r.tables, "Iteration History");
    assert!(close(num(&history.rows[0][1]), 0.2, 1e-12));
    let gap = columns("temp power", &[TEMP, &POWER.replacen("70", ".", 1)]);
    assert!(error(&format!("{gap}AREG power WITH temp.\n")).contains("METHOD=ML"));
    // ML takes the gap
    run(&format!("{gap}AREG power WITH temp /METHOD=ML.\n"));
}

// ---------------------------------------------------------------- 2SLS

fn plots() -> String {
    format!("DATA LIST FREE /yield fert subsidy ph rain.\nBEGIN DATA\n{PLOTS}\nEND DATA.\n")
}

#[test]
fn two_stage_least_squares() {
    let r = run(&format!(
        "{}2SLS yield WITH fert rain /INSTRUMENTS=subsidy ph rain /PRINT=COV.\n",
        plots()
    ));
    let c = find(&r.tables, "Coefficients");
    assert!(close(row_value(c, "fert", 1), 1.631305899569, 1e-9));
    assert!(close(row_value(c, "fert", 2), 0.1284025533885, 1e-9));
    assert!(close(row_value(c, "rain", 1), 0.301313022625, 1e-9));
    assert!(close(row_value(c, "rain", 2), 0.0569454258689, 1e-9));
    assert!(close(row_value(c, "Constant", 1), 26.060787405961, 1e-8));
    assert!(close(row_value(c, "Constant", 2), 4.0633974861003, 1e-8));
    let s = find(&r.tables, "Model Summary");
    assert!(close(row_value(s, "R Square", 1), 0.605592548884, 1e-9));
    assert!(close(
        row_value(s, "Std. Error of the Estimate", 1),
        8.332749f64.sqrt(),
        1e-6
    ));
    let a = find(&r.tables, "ANOVA");
    assert!(close(row_value(a, "Residual", 1), 141.6567, 1e-4));
    let corr = find(&r.tables, "Correlation Matrix of the Estimates");
    assert!(close(row_value(corr, "fert", 2), -0.07698427, 1e-7));
    find(&r.tables, "Covariance Matrix of the Estimates");
    let fit = variable(&r.engine, "FIT_1");
    let err = variable(&r.engine, "ERR_1");
    assert!(close(fit[0].unwrap(), 69.9641, 1e-4));
    assert!(close(err[0].unwrap(), -1.964098, 1e-6));
}

#[test]
fn two_stage_weights_no_constant_and_save() {
    let weighted = format!(
        "{}COMPUTE w = MOD($CASENUM - 1, 4) + 1.\nIF (w = 4) w = 1.\nWEIGHT BY w.\n\
         2SLS yield WITH fert rain /INSTRUMENTS=subsidy ph /INSTRUMENTS=rain /SAVE=RESID.\n",
        plots()
    );
    let r = run(&weighted);
    let c = find(&r.tables, "Coefficients");
    assert!(close(row_value(c, "fert", 1), 1.566346944756, 1e-9));
    assert!(close(row_value(c, "fert", 2), 0.0996787545309, 1e-9));
    assert!(close(row_value(c, "Constant", 1), 28.614047190596, 1e-8));
    // SAVE=RESID: only the residual, under a free number
    assert!(variable(&r.engine, "ERR_1")[0].is_some());
    assert!(r.engine.dataset().unwrap().variable("FIT_1").is_err());
    let r = run(&format!(
        "{}TSET NEWVAR=NONE.\n2SLS EQUATION=yield WITH fert rain /INSTRUMENTS=subsidy ph rain /NOCONSTANT.\n",
        plots()
    ));
    let c = find(&r.tables, "Coefficients");
    assert!(close(row_value(c, "fert", 1), 2.259652748336, 1e-9));
    assert!(close(row_value(c, "rain", 1), 0.507909142867, 1e-9));
    assert!(r.engine.dataset().unwrap().variable("FIT_1").is_err());
}

#[test]
fn two_stage_equations_apply_and_errors() {
    let r = run(&format!(
        "{}2SLS yield WITH fert /fert WITH yield rain /INSTRUMENTS=subsidy ph rain.\n\
         2SLS APPLY /INSTRUMENTS=subsidy rain.\n",
        plots()
    ));
    // two equations, each with its own tables; APPLY swapped the instruments
    assert_eq!(
        r.tables
            .iter()
            .filter(|t| t.title == "Coefficients")
            .count(),
        2
    );
    assert!(variable(&r.engine, "FIT_2")[0].is_some());
    // all predictors as instruments: ordinary least squares
    let ols = run(&format!(
        "{}2SLS yield WITH fert rain /INSTRUMENTS=fert rain.\n",
        plots()
    ));
    let c = find(&ols.tables, "Coefficients");
    assert!(close(row_value(c, "fert", 1), 1.841162196119, 1e-9));
    assert!(close(row_value(c, "Constant", 1), 20.963924776842, 1e-8));
    assert!(
        error(&format!(
            "{}2SLS yield WITH fert rain /INSTRUMENTS=subsidy.\n",
            plots()
        ))
        .contains("fewer than")
    );
    // a dependent without variance gets no equation, as in REGRESSION
    let r = run(&format!(
        "{}COMPUTE flat = 4.\n2SLS flat WITH fert /yield WITH fert /INSTRUMENTS=subsidy ph.\n",
        plots()
    ));
    let coefficients = r.tables.iter().filter(|t| t.title == "Coefficients");
    assert_eq!(coefficients.count(), 1);
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("flat has no variance") && w.contains("equation 1")),
        "{:?}",
        r.warnings
    );
}
