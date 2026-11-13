//! CSPLAN, CSDESCRIPTIVES and CSTABULATE on the invented household survey of
//! tests/fixtures/cs_generate.py: four districts (strata), villages drawn in each (clusters),
//! households drawn in each village, and a weight.
//!
//! The expected numbers come from a general statistics package's survey library, used as a black
//! box on the same file: estimates, standard errors, intervals with the design df, covariance
//! matrices of proportions, its Rao-Scott F test of independence and nonlinear contrasts of totals.
//! Where the manual defines a number differently, the expected value was computed from that
//! package's variances with the manual's formula; each such case says so in a comment.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const CSV: &str = include_str!("../../../tests/fixtures/cs_households.csv");

/// The data, the user-missing heating code and a CD into a fresh folder for the plans.
fn data(folder: &str) -> String {
    let body: Vec<&str> = CSV.lines().skip(1).collect();
    let dir = std::env::temp_dir().join(format!("statscraft-cs-{folder}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    format!(
        "DATA LIST LIST(\",\") /district village hh nvill svill nhh w town members earn food heat online.\n\
         BEGIN DATA\n{}\nEND DATA.\nMISSING VALUES heat (9).\nCD '{}'.\n",
        body.join("\n"),
        dir.display()
    )
}

const WR: &str = "CSPLAN ANALYSIS /PLAN FILE='wr.json' /PLANVARS ANALYSISWEIGHT=w
  /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=WR.\n";

fn run(source: &str) -> (Engine, Vec<Table>) {
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
    (engine, tables)
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
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
                let Cell::Text(t) = cell else { continue };
                if t.is_empty() {
                    continue;
                }
                carried[level] = t.clone();
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
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

fn find<'t>(tables: &'t [Table], title: &str, nth: usize) -> &'t Table {
    tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title} number {nth}"))
}

fn cell<'t>(table: &'t Table, row: &str, column: &str) -> &'t Cell {
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{}: no row {row} in {:?}", table.title, row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| {
            panic!(
                "{}: no column {column} in {:?}",
                table.title,
                table.column_titles()
            )
        });
    &table.rows[r][c]
}

fn check(table: &Table, row: &str, column: &str, expected: f64, tolerance: f64) {
    let actual = match cell(table, row, column) {
        Cell::Number(n) => *n,
        other => panic!(
            "{} / {row} / {column}: not a number: {other:?}",
            table.title
        ),
    };
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{} / {row} / {column}: {actual} != {expected}",
        table.title
    );
}

/// Several cells of one table at a relative tolerance of 1e-8.
fn checks(table: &Table, cells: &[(&str, &str, f64)]) {
    for (row, column, expected) in cells {
        check(table, row, column, *expected, 1e-8);
    }
}

const LOWER: &str = "95% Confidence Interval / Lower";
const UPPER: &str = "95% Confidence Interval / Upper";

#[test]
fn csplan_writes_a_plan_that_view_shows_again() {
    let source = format!(
        "{}CSPLAN ANALYSIS /PLAN FILE='rates.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STAGELABEL='villages' STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR
           /INCLPROB MATRIX=district; 1 0.125; 2 0.09375; 3 0.16; 4 0.0961538461538
           /DESIGN /ESTIMATOR TYPE=WR.
         CSPLAN VIEW /PLAN FILE='rates.json' /PRINT PLAN MATRIX.\n",
        data("view")
    );
    let (_, tables) = run(&source);
    let written = find(&tables, "Analysis Plan", 0);
    let shown = find(&tables, "Analysis Plan", 1);
    assert_eq!(written.rows, shown.rows);
    let keys = row_keys(shown);
    assert!(
        keys.contains(&"Stage 2 / Estimator".to_string()),
        "{keys:?}"
    );
    assert_eq!(
        cell(shown, "Stage 1 / Label", "Value"),
        &Cell::Text("villages".into())
    );
    let matrix = find(&tables, "Plan Matrix", 0);
    assert_eq!(matrix.rows.len(), 4);
    assert_eq!(matrix.rows[2][2], Cell::Text("district = 3".into()));
    assert_eq!(matrix.rows[2][3], Cell::Number(0.16));
    let dir = std::env::temp_dir().join(format!("statscraft-cs-view-{}", std::process::id()));
    let json = std::fs::read_to_string(dir.join("rates.json")).unwrap();
    assert!(
        json.contains("\"format\": \"statscraft-analysis-plan\""),
        "{json}"
    );
    assert!(json.contains("\"estimator\": \"EQUAL_WOR\""), "{json}");
}

#[test]
fn means_sums_and_ratios_of_a_stratified_cluster_sample() {
    let source = format!(
        "{}{WR}CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=food earn /MEAN /SUM
           /RATIO NUMERATOR=food DENOMINATOR=earn
           /STATISTICS SE CIN DEFF DEFFSQRT CV COUNT POPSIZE.\n",
        data("means")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics", 0);
    checks(
        t,
        &[
            ("food / Mean", "Estimate", 397.55746832),
            ("food / Mean", "Standard Error", 8.77779767543),
            ("food / Mean", LOWER, 378.949368512),
            ("food / Mean", UPPER, 416.165568127),
            // with no missing values the package's design effect is the manual's
            ("food / Mean", "Design Effect", 1.14318924436),
            (
                "food / Mean",
                "Square Root of Design Effect",
                1.14318924436_f64.sqrt(),
            ),
            (
                "food / Mean",
                "Coefficient of Variation",
                8.77779767543 / 397.55746832,
            ),
            ("food / Mean", "Unweighted Count", 123.0),
            ("food / Mean", "Population Size", 12762.2167),
            ("earn / Mean", "Estimate", 1620.18448965),
            ("earn / Mean", "Standard Error", 80.8180688325),
            // the manual compares with simple random sampling over every design case, the
            // package over the cases with earnings only: computed with the manual's formula
            ("earn / Mean", "Design Effect", 3.53260263803),
            ("earn / Mean", "Unweighted Count", 119.0),
            ("earn / Mean", "Population Size", 12480.9834),
            ("food / Sum", "Estimate", 5073714.5614),
            ("food / Sum", "Standard Error", 539557.679096),
            ("food / Sum", "Design Effect", 26.5198170511),
        ],
    );
    let r = find(&tables, "Ratios", 0);
    checks(
        r,
        &[
            ("food / earn", "Estimate", 0.244516354028),
            ("food / earn", "Standard Error", 0.00857328478626),
            // the manual's formula, as for earn above
            ("food / earn", "Design Effect", 2.14825565516),
            ("food / earn", "Unweighted Count", 119.0),
        ],
    );
}

#[test]
fn t_test_and_with_replacement_design_effect() {
    let source = format!(
        "{}CSPLAN ANALYSIS /PLAN FILE='srs.json' /PLANVARS ANALYSISWEIGHT=w /SRSESTIMATOR TYPE=WR
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=WR.
         CSDESCRIPTIVES /PLAN FILE='srs.json' /SUMMARY VARIABLES=food /MEAN TTEST=400
           /STATISTICS SE DEFF.\n",
        data("ttest")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics", 0);
    checks(
        t,
        &[
            ("food / Mean", "Hypothesis Test / Test Value", 400.0),
            ("food / Mean", "Hypothesis Test / t", -0.278262472042),
            ("food / Mean", "Hypothesis Test / df", 16.0),
            ("food / Mean", "Hypothesis Test / Sig.", 0.784374829317),
            ("food / Mean", "Design Effect", 1.13217138749),
        ],
    );
}

#[test]
fn equal_probability_without_replacement_brings_the_finite_population_correction() {
    let source = format!(
        "{}CSPLAN ANALYSIS /PLAN FILE='size.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nvill.
         CSPLAN ANALYSIS /PLAN FILE='prob.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR
           /INCLPROB MATRIX=district; 1 0.125; 2 0.09375; 3 0.16; 4 0.09615384615384615.
         CSPLAN ANALYSIS /PLAN FILE='tenth.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /INCLPROB VALUE=0.1.
         CSPLAN ANALYSIS /PLAN FILE='two.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nvill
           /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nhh.
         CSDESCRIPTIVES /PLAN FILE='size.json' /SUMMARY VARIABLES=food /MEAN.
         CSDESCRIPTIVES /PLAN FILE='prob.json' /SUMMARY VARIABLES=food /MEAN.
         CSDESCRIPTIVES /PLAN FILE='tenth.json' /SUMMARY VARIABLES=food /MEAN.
         CSDESCRIPTIVES /PLAN FILE='two.json' /SUMMARY VARIABLES=food /MEAN /SUM.\n",
        data("wor")
    );
    let (_, tables) = run(&source);
    let se = "Standard Error";
    check(
        find(&tables, "Univariate Statistics", 0),
        "food / Mean",
        se,
        8.31701421298,
        1e-8,
    );
    check(
        find(&tables, "Univariate Statistics", 1),
        "food / Mean",
        se,
        8.31701421298,
        1e-8,
    );
    check(
        find(&tables, "Univariate Statistics", 2),
        "food / Mean",
        se,
        8.32735004835,
        1e-8,
    );
    let two = find(&tables, "Univariate Statistics", 3);
    check(two, "food / Mean", se, 8.93938792988, 1e-8);
    check(two, "food / Sum", se, 510769.860969, 1e-8);
}

#[test]
fn subpopulations_are_domains_of_the_whole_design() {
    let source = format!(
        "{}{WR}CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=food /MEAN
           /STATISTICS SE DEFF POPSIZE /SUBPOP TABLE=town.
         CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=food /MEAN
           /SUBPOP TABLE=town DISPLAY=SEPARATE.\n",
        data("domains")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics", 0);
    checks(
        t,
        &[
            ("1 / food / Mean", "Estimate", 415.176397775),
            ("1 / food / Mean", "Standard Error", 9.30454337543),
            // the package compares a domain with simple random sampling inside the domain, the
            // manual over the whole sample: computed with the manual's formula
            ("1 / food / Mean", "Design Effect", 0.622145727768),
            ("1 / food / Mean", "Population Size", 3358.9664),
            ("2 / food / Mean", "Standard Error", 10.7342440449),
            ("2 / food / Mean", "Design Effect", 1.10013471868),
        ],
    );
    // the whole population follows the subpopulations
    check(
        t,
        "Total / food / Mean",
        "Standard Error",
        8.77779767543,
        1e-8,
    );
    let second = find(&tables, "Univariate Statistics", 2);
    assert_eq!(second.caption.as_deref(), Some("Subpopulation: town = 2"));
    check(second, "food / Mean", "Standard Error", 10.7342440449, 1e-8);
    let whole = find(&tables, "Univariate Statistics", 3);
    assert_eq!(whole.caption.as_deref(), Some("The whole population"));
}

#[test]
fn listwise_scope_drops_cases_missing_any_analysis_variable() {
    let source = format!(
        "{}{WR}CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=food earn /MEAN
           /STATISTICS SE COUNT /MISSING SCOPE=LISTWISE.\n",
        data("listwise")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics", 0);
    checks(
        t,
        &[
            ("food / Mean", "Estimate", 396.161604261),
            ("food / Mean", "Standard Error", 8.54978312795),
            ("food / Mean", "Unweighted Count", 119.0),
        ],
    );
}

#[test]
fn one_way_table_with_sizes_percents_and_homogeneity() {
    let source = format!(
        "{}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=heat /CELLS POPSIZE TABLEPCT
           /STATISTICS SE CIN DEFF COUNT CUMULATIVE /TEST HOMOGENEITY.\n",
        data("oneway")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "heat", 0);
    let size = |leaf: &str| format!("Population Size / {leaf}");
    let pct = |leaf: &str| format!("Table Percent / {leaf}");
    // design effects: the manual's formula over every design case (the package uses the
    // cases with a heating code only)
    let expected = [
        (
            "1",
            5591.4081,
            890.840305977,
            3702.9110146,
            2.43823847599,
            45.047411861,
            5.47122754219,
            33.9103574392,
            56.7039152668,
            1.44878044697,
        ),
        (
            "2",
            3710.6308,
            557.63466706,
            2528.49811427,
            1.14049370716,
            29.8948513366,
            3.90361723528,
            22.319132411,
            38.7589691612,
            0.871122354851,
        ),
        (
            "3",
            3110.2349,
            762.243247757,
            1494.35139978,
            2.38420323304,
            25.0577368024,
            4.81774382951,
            16.2546603222,
            36.5477077719,
            1.4808455954,
        ),
    ];
    for (row, n, se, lo, deff, p, pse, plo, phi, pdeff) in expected {
        checks(
            t,
            &[
                (row, &size("Estimate"), n),
                (row, &size("Standard Error"), se),
                (row, &size(LOWER), lo),
                (row, &size("Design Effect"), deff),
                (row, &pct("Estimate"), p),
                (row, &pct("Standard Error"), pse),
                (row, &pct("Design Effect"), pdeff),
            ],
        );
        // the package finds the logit interval by a model fit, which differs in the 9th digit
        check(t, row, &pct(LOWER), plo, 1e-7);
        check(t, row, &pct(UPPER), phi, 1e-7);
    }
    check(t, "Total", &size("Standard Error"), 1372.73583313, 1e-8);
    check(
        t,
        "2",
        &pct("Cumulative"),
        45.047411861 + 29.8948513366,
        1e-8,
    );
    check(t, "3", "Unweighted Count", 29.0, 0.0);
    // the manual's test: the package's own goodness-of-fit test takes the null proportions for
    // the multinomial covariance, so this is computed from its covariance matrix of the
    // proportions with the manual's formula
    let h = find(&tables, "Tests of Homogeneity", 0);
    checks(
        h,
        &[
            ("Pearson", "Chi-Square", 7.83101851692),
            ("Pearson", "Adjusted F", 3.10677520618),
            ("Pearson", "df1", 1.82069306414),
            ("Pearson", "df2", 29.1310890262),
            ("Pearson", "Sig.", 0.064130482712),
            ("Likelihood Ratio", "Chi-Square", 7.58595034671),
            ("Likelihood Ratio", "Adjusted F", 3.00955008618),
            ("Likelihood Ratio", "Sig.", 0.0692003908467),
        ],
    );
}

#[test]
fn two_way_table_with_rao_scott_test_and_adjusted_residuals() {
    let source = format!(
        "{}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=heat BY online
           /CELLS ROWPCT TABLEPCT /STATISTICS SE EXPECTED RESID ASRESID /TEST INDEPENDENCE.\n",
        data("twoway")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "heat * online", 0);
    checks(
        t,
        &[
            ("2 / 1", "Row Percent / Estimate", 43.8956874934),
            ("2 / 1", "Row Percent / Standard Error", 10.1746023325),
            ("Total / Total", "Table Percent / Estimate", 100.0),
        ],
    );
    // expected percents and adjusted residuals: the manual's formulas on the package's
    // covariance matrix of the six table proportions
    let cells = ["1 / 0", "1 / 1", "2 / 0", "2 / 1", "3 / 0", "3 / 1"];
    let expected = [
        26.2562992478,
        18.7911126132,
        17.4244896707,
        12.4703616659,
        14.6051328762,
        10.4526039263,
    ];
    let adjusted = [
        0.00517068517138,
        -0.00517068517137,
        -0.348166369316,
        0.348166369316,
        0.328643466638,
        -0.328643466638,
    ];
    for (k, row) in cells.iter().enumerate() {
        check(t, row, "Table Percent / Expected", expected[k], 1e-8);
        check(
            t,
            row,
            "Table Percent / Adjusted Residual",
            adjusted[k],
            1e-7,
        );
    }
    let i = find(&tables, "Tests of Independence", 0);
    checks(
        i,
        &[
            ("Pearson", "Chi-Square", 0.151120349863),
            ("Pearson", "Adjusted F", 0.0809532142802),
            ("Pearson", "df1", 1.97130766725),
            ("Pearson", "df2", 31.5409226761),
            ("Pearson", "Sig.", 0.920199075896),
            // the package has no likelihood ratio version: the manual's formula
            ("Likelihood Ratio", "Chi-Square", 0.151308525961),
            ("Likelihood Ratio", "Adjusted F", 0.081054017779),
            ("Likelihood Ratio", "Sig.", 0.920105246955),
        ],
    );
}

#[test]
fn two_by_two_odds_ratio_relative_risk_and_risk_difference() {
    let source = format!(
        "{}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=town BY online
           /TEST INDEPENDENCE ODDSRATIO RELRISK RISKDIFF.\n",
        data("risks")
    );
    let (_, tables) = run(&source);
    let r = find(&tables, "Odds Ratios and Risks", 0);
    checks(
        r,
        &[
            ("Odds Ratio", "Estimate", 0.27913503591),
            ("Odds Ratio", "Standard Error", 0.150786637722),
            ("Odds Ratio", LOWER, 0.088813515165),
            ("Odds Ratio", UPPER, 0.877303056044),
            ("Relative Risk (online = 1)", "Estimate", 1.91281879258),
            (
                "Relative Risk (online = 1)",
                "Standard Error",
                0.463547479468,
            ),
            ("Relative Risk (online = 1)", LOWER, 1.1443601759),
            ("Relative Risk (online = 1)", UPPER, 3.19731131012),
            ("Risk Difference (online = 0)", "Estimate", -0.308534372906),
            (
                "Risk Difference (online = 0)",
                "Standard Error",
                0.122857394489,
            ),
            ("Risk Difference (online = 0)", LOWER, -0.568980414532),
        ],
    );
    let i = find(&tables, "Tests of Independence", 0);
    checks(
        i,
        &[
            ("Pearson", "Adjusted F", 5.89593430934),
            ("Pearson", "Sig.", 0.0273409788952),
        ],
    );
}

#[test]
fn tests_per_subpopulation_only_for_first_stage_strata() {
    let source = format!(
        "{}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=online /TEST HOMOGENEITY
           /SUBPOP TABLE=district.
         CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=online /TEST HOMOGENEITY
           /SUBPOP TABLE=town.\n",
        data("strata")
    );
    let (engine, tables) = run(&source);
    let by_stratum = find(&tables, "Tests of Homogeneity", 0);
    let keys = row_keys(by_stratum);
    assert_eq!(keys.len(), 10, "{keys:?}");
    assert_eq!(keys[0], "Total / Pearson");
    assert_eq!(keys[2], "1 / Pearson");
    let by_town = find(&tables, "Tests of Homogeneity", 1);
    assert_eq!(by_town.rows.len(), 2);
    assert!(engine.warnings()[0].contains("whole population only"));
    // the layered table has a block per district
    let t = find(&tables, "online", 0);
    assert_eq!(t.rows.len(), 12);
}

#[test]
fn classmissing_include_makes_a_user_missing_code_a_category() {
    let source = format!(
        "{}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=heat /STATISTICS COUNT
           /MISSING CLASSMISSING=INCLUDE.\n",
        data("include")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "heat", 0);
    assert_eq!(row_keys(t), ["1", "2", "3", "9", "Total"]);
    check(t, "9", "Unweighted Count", 3.0, 0.0);
    check(t, "Total", "Unweighted Count", 123.0, 0.0);
}

#[test]
fn design_and_plan_errors() {
    let d = data("errors");
    let unequal = format!(
        "{d}CSPLAN ANALYSIS /PLAN FILE='u.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN CLUSTER=village /ESTIMATOR TYPE=UNEQUAL_WOR.
         CSDESCRIPTIVES /PLAN FILE='u.json' /SUMMARY VARIABLES=food /MEAN.\n"
    );
    assert!(run_error(&unequal).contains("UNEQUAL_WOR"));
    let missing =
        format!("{d}CSDESCRIPTIVES /PLAN FILE='nowhere.json' /SUMMARY VARIABLES=food /MEAN.\n");
    assert!(run_error(&missing).contains("cannot read the plan"));
    let plan_var =
        format!("{d}{WR}CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=w /MEAN.\n");
    assert!(run_error(&plan_var).contains("plan variable"));
    let small = format!(
        "{d}CSPLAN ANALYSIS /PLAN FILE='s.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VALUE=4.
         CSDESCRIPTIVES /PLAN FILE='s.json' /SUMMARY VARIABLES=food /MEAN.\n"
    );
    assert!(run_error(&small).contains("population size below"));
    let weight_subpop =
        format!("{d}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=heat /SUBPOP TABLE=w.\n");
    assert!(run_error(&weight_subpop).contains("cannot be on SUBPOP"));
    let crossed =
        format!("{d}{WR}CSTABULATE /PLAN FILE='wr.json' /TABLES VARIABLES=heat BY heat.\n");
    assert!(run_error(&crossed).contains("both before and after BY"));
    let empty_subpop = format!(
        "{d}{WR}COMPUTE nothing = $SYSMIS.
         CSDESCRIPTIVES /PLAN FILE='wr.json' /SUMMARY VARIABLES=food /MEAN /SUBPOP TABLE=nothing.\n"
    );
    assert!(run_error(&empty_subpop).contains("no case in the design"));
}

#[test]
fn csplan_needs_no_data() {
    let dir = std::env::temp_dir().join(format!("statscraft-cs-nodata-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!(
        "CSPLAN ANALYSIS /PLAN FILE='{}' /PLANVARS ANALYSISWEIGHT=wt /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VALUE=900.\n",
        dir.join("plain.json").display()
    );
    let (_, tables) = run(&source);
    let plan = find(&tables, "Analysis Plan", 0);
    assert_eq!(
        cell(plan, "Stage 1 / Population size", "Value"),
        &Cell::Text("900".into())
    );
}

#[test]
fn a_weight_of_zero_leaves_the_design() {
    let source = format!(
        "{}COMPUTE w2 = w.
         IF (hh = 1) w2 = 0.
         CSPLAN ANALYSIS /PLAN FILE='zero.json' /PLANVARS ANALYSISWEIGHT=w2
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=WR.
         CSDESCRIPTIVES /PLAN FILE='zero.json' /SUMMARY VARIABLES=food /MEAN /STATISTICS COUNT.\n",
        data("zero")
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics", 0);
    check(t, "food / Mean", "Unweighted Count", 122.0, 0.0);
}
