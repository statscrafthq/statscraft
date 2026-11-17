//! QUANTILE REGRESSION on invented data. The expected numbers come from an independent check
//! in a scientific Python stack: the coefficients from a general linear programming solver
//! (with each coefficient's range over the optimal face computed to confirm the solution is
//! unique), the standard errors, tests, intervals, pseudo R squared and mean absolute error
//! written out from their formulas in the algorithms description.

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

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
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

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0),
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

/// Fifteen invented cases: a factor grp with three levels and a covariate x.
const DATA: &str = "DATA LIST FREE /grp x y.
BEGIN DATA.
1 1 3.1 2 2 6.4 3 3 4.0 1 4 7.3 2 5 9.9 3 6 6.2 1 7 10.8 2 8 13.5
3 9 8.1 1 10 14.6 2 11 16.2 3 12 12.7 1 13 17.4 2 14 21.3 3 15 13.9
END DATA.
";

const ROWS: [&str; 4] = ["Intercept", "x", "[grp=1]", "[grp=2]"];

fn check(
    tables: &[Table],
    q: &str,
    b: &[f64],
    se: &[f64],
    sig: &[f64],
    lower: &[f64],
    level: &str,
) {
    for (i, row) in ROWS.iter().enumerate() {
        let key = format!("{q} / {row}");
        let get = |column: &str| number(cell(tables, "Parameter Estimates", &key, column));
        close(get("Coefficient"), b[i], &format!("{key} coefficient"));
        close(get("Std. Error"), se[i], &format!("{key} standard error"));
        close(get("Sig."), sig[i], &format!("{key} significance"));
        close(
            get(&format!("{level} Confidence Interval / Lower Bound")),
            lower[i],
            &format!("{key} lower"),
        );
        close(get("df"), 11.0, "df");
    }
    let redundant = format!("{q} / [grp=3]");
    assert_eq!(
        number(cell(
            tables,
            "Parameter Estimates",
            &redundant,
            "Coefficient"
        )),
        0.0
    );
    assert_eq!(
        cell(tables, "Parameter Estimates", &redundant, "Std. Error"),
        &Cell::Empty
    );
}

#[test]
fn three_quantiles_with_iid_errors() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /CRITERIA QUANTILE=0.25 0.5 0.65 METHOD=SIMPLEX /PRINT PARAMETER COVB CORB."
        ),
    );
    check(
        &tables,
        "0.25",
        &[
            -1.6999999999999993,
            1.0888888888888888,
            4.644444444444444,
            5.9222222222222225,
        ],
        &[
            0.7365164674659552,
            0.06338924472673693,
            0.6708487093726336,
            0.6618031443154754,
        ],
        &[
            0.041432470645966266,
            2.7144648688553096e-09,
            2.509638836896937e-05,
            2.216862895332057e-06,
        ],
        &[
            -3.3210618150492826,
            0.9493701019364744,
            3.1679163904544785,
            4.4656033226876275,
        ],
        "95%",
    );
    check(
        &tables,
        "0.5",
        &[
            -0.7666666666666657,
            1.122222222222222,
            3.5777777777777775,
            5.055555555555556,
        ],
        &[
            1.1884065719259338,
            0.10228175247983531,
            1.0824483228860327,
            1.0678528461580024,
        ],
        &[
            0.5320730972134902,
            2.904548074282519e-07,
            0.007012188408586382,
            0.0006150544281161453,
        ],
        &[
            -3.3823318956206956,
            0.8971016028668272,
            1.1953250825489432,
            2.705227288009453,
        ],
        "95%",
    );
    check(
        &tables,
        "0.65",
        &[
            -0.8000000000000016,
            1.166666666666667,
            3.4333333333333336,
            4.966666666666666,
        ],
        &[
            0.9484764788627194,
            0.08163185793121697,
            0.8639103805846227,
            0.8522616176937459,
        ],
        &[
            0.4169370276303539,
            1.8940976294964443e-08,
            0.0021804395645431663,
            0.00011456717429952852,
        ],
        &[
            -2.8875826546645755,
            0.9869961587700589,
            1.5318794060249665,
            3.090851493614444,
        ],
        "95%",
    );
    let quality = |q: &str, column: &str| number(cell(&tables, "Model Quality", q, column));
    close(
        quality("0.25", "Pseudo R Squared"),
        0.8487455197132616,
        "R2 0.25",
    );
    close(
        quality("0.25", "Mean Absolute Error"),
        0.7155555555555552,
        "MAE 0.25",
    );
    close(
        quality("0.5", "Pseudo R Squared"),
        0.8658410732714138,
        "R2 0.5",
    );
    close(
        quality("0.65", "Mean Absolute Error"),
        0.6266666666666675,
        "MAE 0.65",
    );
    close(
        number(cell(
            &tables,
            "Parameter Covariance",
            "0.5 / Intercept",
            "x",
        )),
        -0.09415401201311667,
        "covariance",
    );
    close(
        number(cell(
            &tables,
            "Parameter Correlation",
            "0.5 / Intercept",
            "x",
        )),
        -0.09415401201311667 / (1.1884065719259338 * 0.10228175247983531),
        "correlation",
    );
    assert_eq!(
        number(cell(&tables, "Case Processing Summary", "Included", "N")),
        15.0
    );
    // by default the first effects that can be shown: x as lines, grp as a table; then one
    // chart per estimated parameter across the quantiles
    assert!(tables.iter().any(|t| t.title == "Predicted Values: grp"));
    let titles: Vec<String> = engine.take_charts().into_iter().map(|c| c.title).collect();
    assert_eq!(
        titles,
        [
            "Predicted y by x",
            "Parameter estimates by quantile: Intercept",
            "Parameter estimates by quantile: x",
            "Parameter estimates by quantile: [grp=1]",
            "Parameter estimates by quantile: [grp=2]"
        ]
    );
}

#[test]
fn interior_point_reaches_the_same_vertex() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /CRITERIA QUANTILE=0.25 METHOD=INTERIOR_POINT."
        ),
    );
    close(
        number(cell(
            &tables,
            "Parameter Estimates",
            "0.25 / x",
            "Coefficient",
        )),
        1.0888888888888888,
        "x",
    );
    close(
        number(cell(
            &tables,
            "Parameter Estimates",
            "0.25 / [grp=2]",
            "Coefficient",
        )),
        5.9222222222222225,
        "grp=2",
    );
}

#[test]
fn densities_at_each_case_without_iid() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /CRITERIA IID=FALSE /SAVE PRED CIPREDU(up)."
        ),
    );
    check(
        &tables,
        "0.5",
        &[
            -0.7666666666666657,
            1.122222222222222,
            3.5777777777777775,
            5.055555555555556,
        ],
        &[
            1.2099146081667147,
            0.057665160204359175,
            1.2022285211720871,
            1.1763059055312621,
        ],
        &[
            0.5392590123644816,
            7.162940835784916e-10,
            0.012610377907743416,
            0.0012605413600965834,
        ],
        &[
            -3.4296707642091806,
            0.9953020603586217,
            0.9316906436495445,
            2.466523713763313,
        ],
        "95%",
    );
    close(
        values(&engine, "PRED")[0].unwrap(),
        3.9333333333333336,
        "predicted",
    );
    close(
        values(&engine, "up")[0].unwrap(),
        4.346226654791114,
        "upper bound",
    );
}

#[test]
fn hall_sheather_bandwidth_and_a_90_percent_level() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /CRITERIA BANDWIDTH=HALL_SHEATHER CILEVEL=90 /SAVE CIPREDL."
        ),
    );
    check(
        &tables,
        "0.5",
        &[
            -0.7666666666666657,
            1.122222222222222,
            3.5777777777777775,
            5.055555555555556,
        ],
        &[
            0.7060923367579419,
            0.06077075246995974,
            0.6431371920871107,
            0.6344652825635125,
        ],
        &[
            0.30079451838270027,
            1.2554765258632804e-09,
            0.0001694243789817575,
            6.784084471013767e-06,
        ],
        &[
            -2.034727174853252,
            1.0130849504402228,
            2.4227774581648296,
            3.9161289866052105,
        ],
        "90%",
    );
    close(
        values(&engine, "CIPREDL")[0].unwrap(),
        2.98503906551631,
        "lower bound",
    );
}

/// Eleven invented cases with a frequency weight f and a regression weight g.
const WEIGHTED: &str = "DATA LIST FREE /x y f g.
BEGIN DATA.
0.5 2.2 1 1  1.2 2.0 2 0.5  2.0 3.9 1 2  2.7 3.1 3 1  3.1 5.6 1 1.5  4.4 4.8 2 1
5.0 7.7 1 1  5.8 6.1 1 0.8  6.6 9.4 2 1  7.3 7.9 1 1.2  8.1 10.6 1 1  9.0 1.0 0.3 1
9.5 2.0 1 0  9.9 . 1 1
END DATA.
WEIGHT BY f.
";

#[test]
fn frequency_and_regression_weights() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{WEIGHTED}QUANTILE REGRESSION y WITH x /CRITERIA QUANTILE=0.6 0.3 /MODEL x REGWGT=g /SAVE PRED RESID(res) CIPREDL."
        ),
    );
    let get = |row: &str, column: &str| number(cell(&tables, "Parameter Estimates", row, column));
    close(
        get("0.3 / Intercept", "Coefficient"),
        0.28260869565217306,
        "b0 0.3",
    );
    close(get("0.3 / x", "Coefficient"), 1.0434782608695654, "b1 0.3");
    close(
        get("0.3 / Intercept", "Std. Error"),
        0.68412409606484,
        "se0 0.3",
    );
    close(get("0.3 / x", "Sig."), 5.7030976600167705e-06, "sig1 0.3");
    close(
        get("0.6 / Intercept", "Coefficient"),
        1.6333333333333335,
        "b0 0.6",
    );
    close(get("0.6 / x", "Std. Error"), 0.16061313130634708, "se1 0.6");
    close(
        get("0.6 / x", "95% Confidence Interval / Lower Bound"),
        0.7888524274028266,
        "lower 0.6",
    );
    close(get("0.6 / x", "df"), 14.0, "df");
    let quality = |q: &str, column: &str| number(cell(&tables, "Model Quality", q, column));
    close(quality("0.3", "Pseudo R Squared"), 0.6249745544284689, "R2");
    close(
        quality("0.6", "Mean Absolute Error"),
        0.9608333333333335,
        "MAE",
    );
    // the weight of 0.3 rounds to 0, the REGWGT of 0 and the missing y leave three cases out
    assert_eq!(
        number(cell(&tables, "Case Processing Summary", "Included", "N")),
        16.0
    );
    close(
        values(&engine, "PRED_30")[0].unwrap(),
        0.8043478260869558,
        "predicted",
    );
    close(
        values(&engine, "CIPREDL_30")[0].unwrap(),
        -0.4114561118495029,
        "lower",
    );
    close(
        values(&engine, "CIPREDL_60")[0].unwrap(),
        0.8792204542929716,
        "lower 0.6",
    );
    close(values(&engine, "res_60")[0].unwrap(), 2.2 - 2.2, "residual");
    // a case outside the fit still gets a prediction; the missing y has no residual
    close(
        values(&engine, "PRED_60")[12].unwrap(),
        1.6333333333333335 + 9.5 * 1.1333333333333333,
        "outside",
    );
    assert_eq!(values(&engine, "res_60")[13], None);
}

#[test]
fn prediction_tables_and_lines() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /PREDICTION_TABLES grp /PREDICTION_LINES x /PLOT PREDICTED_BY_OBSERVED=TRUE."
        ),
    );
    // the factor's cells with x at its mean, 8
    close(
        number(cell(
            &tables,
            "Predicted Values: grp",
            "1",
            "Quantile / 0.5",
        )),
        -0.7666666666666657 + 8.0 * 1.122222222222222 + 3.5777777777777775,
        "cell 1",
    );
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 2);
    let line = &charts[0];
    // at x = 1 the factor's levels are averaged
    close(
        line.series[0].values[0],
        -0.7666666666666657 + 1.122222222222222 + (3.5777777777777775 + 5.055555555555556) / 3.0,
        "line start",
    );
}

#[test]
fn outfile_writes_the_covariances_to_a_dataset() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}DATASET NAME main.\nDATASET DECLARE cov.\nQUANTILE REGRESSION y BY grp WITH x /CRITERIA QUANTILE=0.25 0.5 /OUTFILE COVB=cov.\nDATASET ACTIVATE cov."
        ),
    );
    let quantile = values(&engine, "Quantile");
    let p1 = values(&engine, "P1");
    let p2 = values(&engine, "P2");
    // rows per quantile: EST SE SIG DF and five COV rows
    assert_eq!(quantile.len(), 18);
    assert_eq!(quantile[9], Some(0.5));
    close(p2[0].unwrap(), 1.0888888888888888, "estimate");
    close(p1[10].unwrap(), 1.1884065719259338, "standard error");
    close(p2[13].unwrap(), -0.09415401201311667, "covariance");
    assert_eq!(values(&engine, "P5")[1], None);
}

#[test]
fn duplicates_and_grids_of_quantiles() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /CRITERIA QUANTILE=0.5 0.25 TO 0.65 BY 0.2 0.25 /PRINT."
        ),
    );
    let quality = tables.iter().find(|t| t.title == "Model Quality").unwrap();
    assert_eq!(row_keys(quality), ["0.25", "0.45", "0.5", "0.65"]);
    assert!(!tables.iter().any(|t| t.title == "Parameter Estimates"));
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("0.25 is given more than once"))
    );
}

#[test]
fn errors_name_the_problem() {
    assert!(run_error("DATA LIST FREE /y x.\nBEGIN DATA.\n1 1 1 2 1 3\nEND DATA.\nQUANTILE REGRESSION y WITH x.").contains("constant"));
    assert!(
        run_error(&format!(
            "{DATA}QUANTILE REGRESSION y /MODEL INTERCEPT=FALSE."
        ))
        .contains("intercept or at least one predictor")
    );
    assert!(
        run_error(&format!(
            "{DATA}QUANTILE REGRESSION y WITH x /CRITERIA QUANTILE=1.5."
        ))
        .contains("between 0 and 1")
    );
    assert!(run_error(&format!("{DATA}QUANTILE REGRESSION y WITH x /CRITERIA QUANTILE=0.1 TO 0.3 BY 0.1 0.5 TO 0.9 BY 0.1.")).contains("only one grid"));
    assert!(
        run_error(&format!(
            "{DATA}QUANTILE REGRESSION y WITH x /PREDICTION_LINES grp."
        ))
        .contains("neither a factor nor a covariate")
    );
    assert!(
        run_error(&format!(
            "{DATA}QUANTILE REGRESSION y BY grp WITH x /MODEL x /PREDICTION_TABLES grp."
        ))
        .contains("not an effect")
    );
}

/// 301 invented cases from a formula, so that both methods are compared on a larger problem;
/// the expected coefficients come from the same check as above.
#[test]
fn both_methods_agree_on_a_larger_problem() {
    let mut data = String::from("DATA LIST FREE /x g y.\nBEGIN DATA.\n");
    for i in 0..301u32 {
        let x: f64 = format!("{:.1}", f64::from(i * 37 % 101) / 10.0)
            .parse()
            .unwrap();
        let g = i % 2 + 1;
        let noise = (f64::from(i) * 1.7).sin() * 3.0 + (f64::from(i) * 0.31).cos() * 2.0;
        let shift = if g == 1 { 1.5 } else { 0.0 };
        data.push_str(&format!("{x} {g} {:.2}\n", 2.0 + 0.5 * x + shift + noise));
    }
    data.push_str("END DATA.\n");
    for method in ["SIMPLEX", "INTERIOR_POINT"] {
        let mut engine = Engine::default();
        let tables = run(
            &mut engine,
            &format!(
                "{data}QUANTILE REGRESSION y BY g WITH x /CRITERIA QUANTILE=0.33 METHOD={method}."
            ),
        );
        let get = |row: &str| number(cell(&tables, "Parameter Estimates", row, "Coefficient"));
        close(get("0.33 / Intercept"), 0.7440425531914895, method);
        close(get("0.33 / x"), 0.48297872340425524, method);
        close(get("0.33 / [g=1]"), 1.777872340425532, method);
    }
}
