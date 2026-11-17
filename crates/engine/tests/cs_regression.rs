//! CSGLM, CSLOGISTIC and CSORDINAL on the invented household survey of
//! tests/fixtures/cs_generate.py: four districts (strata), villages drawn in each (clusters),
//! households drawn in each village, and a weight.
//!
//! The expected numbers come from a general statistics package's survey library, used as a black
//! box on the same file: survey-weighted linear and binary logistic fits with their design-based
//! covariance matrices, and design covariances of totals. The multinomial model was fitted there
//! by solving the weighted score equations and forming the sandwich from those covariances.
//! CSORDINAL: see the comment of its first test.
//! Where the manual defines a number differently from the package (the df of the t and F tests,
//! the simple random sampling variance behind design effects and the adjusted tests, the Wald F)
//! the expected value was computed from the package's estimates and covariances with the
//! manual's formula.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const CSV: &str = include_str!("../../../tests/fixtures/cs_households.csv");

/// The data, the user-missing heating code and a CD into a fresh folder for the plans.
fn data(folder: &str) -> String {
    let body: Vec<&str> = CSV.lines().skip(1).collect();
    let dir =
        std::env::temp_dir().join(format!("statscraft-csreg-{folder}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    format!(
        "DATA LIST LIST(\",\") /district village hh nvill svill nhh w town members earn food heat online.\n\
         BEGIN DATA\n{}\nEND DATA.\nMISSING VALUES heat (9).\nCD '{}'.\n\
         CSPLAN ANALYSIS /PLAN FILE='wr.json' /PLANVARS ANALYSISWEIGHT=w\n\
         \x20 /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=WR.\n",
        body.join("\n"),
        dir.display()
    )
}

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

fn value(table: &Table, row: &str, column: &str) -> f64 {
    match cell(table, row, column) {
        Cell::Number(n) => *n,
        other => panic!(
            "{} / {row} / {column}: not a number: {other:?}",
            table.title
        ),
    }
}

/// Several cells of one table at a relative tolerance of 1e-7.
fn checks(table: &Table, cells: &[(&str, &str, f64)]) {
    for (row, column, expected) in cells {
        let actual = value(table, row, column);
        assert!(
            (actual - expected).abs() <= 1e-7 * expected.abs().max(1e-3),
            "{} / {row} / {column}: {actual} != {expected}",
            table.title
        );
    }
}

/// The values of a numeric variable, None for system-missing.
fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let values = data.variable(name).unwrap().numeric().unwrap();
    (0..data.case_count()).map(|k| values.get(k)).collect()
}

const LOWER: &str = "95% Confidence Interval / Lower";
const UPPER: &str = "95% Confidence Interval / Upper";
const T: &str = "Hypothesis Test / t";
const SIG: &str = "Hypothesis Test / Sig.";

const GLM: &str = "CSGLM food BY heat WITH members earn /PLAN FILE='wr.json'";

#[test]
fn csglm_estimates_design_effects_r_square_and_wald_f_tests() {
    let source = format!(
        "{}{GLM} /STATISTICS PARAMETER SE CINTERVAL TTEST DEFF DEFFSQRT.\n",
        data("glm")
    );
    let (_, tables) = run(&source);
    let p = find(&tables, "Parameter Estimates", 0);
    checks(
        p,
        &[
            ("Intercept", "Estimate", 102.531710704),
            ("Intercept", "Std. Error", 17.7037050209),
            ("Intercept", LOWER, 65.0015326142),
            ("Intercept", UPPER, 140.061888794),
            ("Intercept", T, 5.79153971345),
            ("Intercept", SIG, 2.75465675195e-05),
            ("Intercept", "Hypothesis Test / df", 16.0),
            ("Intercept", "Design Effect", 1.68597056648),
            (
                "Intercept",
                "Square Root of Design Effect",
                1.68597056648_f64.sqrt(),
            ),
            ("[heat=1]", "Estimate", 5.52447526981),
            ("[heat=1]", "Std. Error", 9.99972633489),
            ("[heat=1]", T, 0.552462645956),
            ("[heat=1]", SIG, 0.58826682688),
            ("[heat=1]", "Design Effect", 0.923323210708),
            ("[heat=2]", "Estimate", 1.23192011797),
            ("[heat=2]", "Design Effect", 1.09387067941),
            ("[heat=3] (redundant)", "Estimate", 0.0),
            ("members", "Estimate", 58.0665340199),
            ("members", "Std. Error", 4.64612955213),
            ("members", LOWER, 48.2171793615),
            ("members", "Design Effect", 1.49607963076),
            ("earn", "Estimate", 0.0814403601253),
            ("earn", "Std. Error", 0.00803631522876),
            ("earn", "Design Effect", 1.02124271049),
        ],
    );
    assert_eq!(cell(p, "[heat=3] (redundant)", "Std. Error"), &Cell::Empty);
    checks(
        find(&tables, "Model Summary", 0),
        &[("", "R Square", 0.774524397144)],
    );
    let t = find(&tables, "Tests of Model Effects", 0);
    checks(
        t,
        &[
            // the Wald F of the manual: (df - r + 1) / (r df) times the Wald chi-square
            ("(Corrected Model)", "Wald F", 63.5391441563),
            ("(Corrected Model)", "df1", 4.0),
            ("(Corrected Model)", "df2", 13.0),
            ("(Corrected Model)", "Sig.", 2.10378905761e-08),
            // Type III: the intercept at the mean of the heating levels
            ("(Intercept)", "Wald F", 30.877324072),
            ("(Intercept)", "Sig.", 4.33739188923e-05),
            ("heat", "Wald F", 0.166724724554),
            ("heat", "df2", 15.0),
            ("heat", "Sig.", 0.847979688315),
            ("members", "Wald F", 156.19573706),
            ("members", "Sig.", 1.13923856621e-09),
        ],
    );
}

#[test]
fn csglm_adjusted_and_chi_square_tests() {
    let source = format!(
        "{d}{GLM} /TEST TYPE=ADJF /PRINT NONE.\n{GLM} /TEST TYPE=CHISQUARE /PRINT NONE.\n\
         {GLM} /TEST TYPE=ADJCHISQUARE /PRINT NONE.\n",
        d = data("adj")
    );
    let (_, tables) = run(&source);
    assert!(
        tables
            .iter()
            .all(|t| t.title != "Sample Design Information")
    );
    checks(
        find(&tables, "Tests of Model Effects", 0),
        &[
            ("(Corrected Model)", "Adjusted Wald F", 111.007960231),
            ("(Corrected Model)", "df1", 3.59526147129),
            ("(Corrected Model)", "df2", 3.59526147129 * 16.0),
            ("heat", "Adjusted Wald F", 0.161297013929),
            ("heat", "df1", 1.97117385129),
            ("heat", "df2", 31.5387816207),
            ("heat", "Sig.", 0.848825175004),
            // one row: the same as the Wald F
            ("members", "Adjusted Wald F", 156.19573706),
            ("members", "Sig.", 1.13923856621e-09),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 1),
        &[
            ("heat", "Wald Chi-Square", 0.355679412383),
            ("heat", "df", 2.0),
            ("heat", "Sig.", 0.83707659093),
            ("members", "Sig.", 7.67175684966e-36),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 2),
        &[
            ("heat", "Adjusted Wald Chi-Square", 0.317944456148),
            ("heat", "df", 1.97117385129),
            ("heat", "Sig.", 0.848071435974),
            (
                "(Corrected Model)",
                "Adjusted Wald Chi-Square",
                399.102642426,
            ),
        ],
    );
}

#[test]
fn csglm_custom_tests_and_estimated_marginal_means() {
    let source = format!(
        "{}{GLM} /CUSTOM LABEL='gas against wood' LMATRIX=heat 1 -1 0
           /CUSTOM LMATRIX=1 heat 1/3 1/3 1/3 members 3 earn 1500; heat 1 -1 0 KMATRIX=400; 0
           /EMMEANS TABLES=heat COMPARE=heat /TEST PADJUST=BONFERRONI /PRINT LMATRIX
           /CUSTOM KMATRIX=100.
         {GLM} /EMMEANS TABLES=heat COMPARE /TEST PADJUST=SEQBONFERRONI /PRINT NONE.
         {GLM} /EMMEANS TABLES=heat COMPARE=heat CONTRAST=SIMPLE(3) /TEST PADJUST=SIDAK /PRINT NONE.\n",
        data("custom")
    );
    let (_, tables) = run(&source);
    let c1 = find(&tables, "Custom Hypothesis Tests", 0);
    assert!(
        c1.caption
            .as_deref()
            .unwrap()
            .starts_with("gas against wood.")
    );
    checks(
        c1,
        &[
            ("Contrast 1", "Estimate", 4.29255515183),
            ("Contrast 1", "Std. Error", 9.42672560182),
            ("Contrast 1", "Wald F", 0.207352863095),
            // one row tested: Bonferroni leaves it as it is
            ("Contrast 1", "Sig.", 0.654968675883),
            ("Overall Test", "Wald F", 0.207352863095),
        ],
    );
    let c2 = find(&tables, "Custom Hypothesis Tests", 1);
    checks(
        c2,
        &[
            ("Contrast 1", "Estimate", 401.143984748),
            ("Contrast 1", "Hypothesized Value", 400.0),
            (
                "Contrast 1",
                "Difference (Estimate - Hypothesized)",
                1.143984748,
            ),
            ("Contrast 1", "Std. Error", 3.11439978332),
            ("Contrast 1", "Wald F", 0.134924788917),
            // two rows: Bonferroni doubles the p value, at most 1
            ("Contrast 1", "Sig.", 1.0),
            ("Contrast 2", "Sig.", (2.0 * 0.654968675883_f64).min(1.0)),
            ("Overall Test", "df1", 2.0),
        ],
    );
    // KMATRIX alone: the intercept's estimable function, here the intercept itself
    let k = find(&tables, "Custom Hypothesis Tests", 2);
    let z: f64 = (102.531710704 - 100.0) / 17.7037050209;
    checks(
        k,
        &[
            ("Contrast 1", "Estimate", 102.531710704),
            ("Contrast 1", "Hypothesized Value", 100.0),
            ("Contrast 1", "Wald F", z * z),
        ],
    );
    let em = find(&tables, "Estimated Marginal Means", 0);
    checks(
        em,
        &[
            ("1", "Mean", 401.047365741),
            ("1", "Std. Error", 7.368492532),
            ("2", "Mean", 396.75481059),
            ("2", "Std. Error", 4.6676387425),
            ("3", "Mean", 395.522890472),
            ("3", "Std. Error", 7.23747024509),
        ],
    );
    let compare = find(&tables, "Individual Test Results", 0);
    checks(
        compare,
        &[
            ("1 vs. 3", "Estimate", 5.52447526981),
            ("1 vs. 3", "Sig.", 1.0),
            ("Overall Test", "Wald F", 0.166724724554),
            ("Overall Test", "Sig.", 0.847979688315),
        ],
    );
    checks(
        find(&tables, "Individual Test Results", 1),
        &[("1 vs. 3", "Sig.", 1.0), ("2 vs. 3", "Sig.", 1.0)],
    );
    checks(
        find(&tables, "Individual Test Results", 2),
        &[("1 vs. 3", "Sig.", 0.830475794152)],
    );
    // L matrices of the model effects and of both CUSTOM subcommands
    let l = find(&tables, "Contrast Coefficients (L Matrix)", 5);
    assert!(
        l.caption
            .as_deref()
            .unwrap()
            .starts_with("gas against wood.")
    );
    assert_eq!(l.rows[0][2], Cell::Number(1.0));
    assert_eq!(l.rows[0][3], Cell::Number(-1.0));
}

#[test]
fn csglm_domain_two_stages_and_interaction() {
    let source = format!(
        "{}{GLM} /DOMAIN VARIABLE=town(1) /STATISTICS PARAMETER SE DEFF.
         CSPLAN ANALYSIS /PLAN FILE='two.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nvill
           /DESIGN CLUSTER=hh /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nhh.
         CSGLM food BY heat WITH members earn /PLAN FILE='two.json' /STATISTICS SE DEFF /PRINT NONE.
         CSGLM food BY heat town WITH members /PLAN FILE='wr.json' /MODEL heat town heat*town members
           /PRINT NONE.
         CSGLM food BY heat town WITH members /PLAN FILE='wr.json' /MODEL heat town heat*town members
           /TEST TYPE=ADJF /PRINT NONE.\n",
        data("domain")
    );
    let (_, tables) = run(&source);
    let info = find(&tables, "Sample Design Information", 0);
    assert_eq!(
        cell(info, "Sampling Design Degrees of Freedom", "Value"),
        &Cell::Number(16.0)
    );
    assert_eq!(
        cell(info, "Subpopulation / Unweighted Count", "Value"),
        &Cell::Number(27.0)
    );
    checks(
        find(&tables, "Parameter Estimates", 0),
        &[
            ("[heat=1]", "Estimate", 38.8272473688),
            ("[heat=1]", "Std. Error", 18.0052567563),
            ("[heat=1]", "Design Effect", 1.24446602801),
            ("members", "Estimate", 48.8747375375),
            ("members", "Std. Error", 6.76134217932),
            ("members", "Design Effect", 1.37591768841),
        ],
    );
    checks(
        find(&tables, "Model Summary", 0),
        &[("", "R Square", 0.7012035329)],
    );
    checks(
        find(&tables, "Tests of Model Effects", 0),
        &[
            ("heat", "Wald F", 2.34877918984),
            ("heat", "Sig.", 0.129595962212),
        ],
    );
    checks(
        find(&tables, "Parameter Estimates", 1),
        &[
            ("Intercept", "Std. Error", 17.3283822691),
            ("Intercept", "Design Effect", 1.61524237308),
            ("[heat=1]", "Std. Error", 10.1546254686),
            ("[heat=1]", "Design Effect", 0.952149938545),
            ("members", "Std. Error", 4.59638674204),
            ("members", "Design Effect", 1.46421619375),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 1),
        &[("heat", "Wald F", 0.161209352385)],
    );
    // Type III with an interaction: the main effects as with sum-to-zero coding
    checks(
        find(&tables, "Tests of Model Effects", 2),
        &[
            ("heat", "Wald F", 2.81999352601),
            ("heat", "df2", 15.0),
            ("heat", "Sig.", 0.0912775110136),
            ("town", "Wald F", 15.8920483716),
            ("town", "Sig.", 0.00106177774962),
            ("heat * town", "Wald F", 0.548738031489),
            ("heat * town", "Sig.", 0.58884505619),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 3),
        &[
            ("heat", "Adjusted Wald F", 3.77624696306),
            ("heat", "df1", 1.64672901922),
            ("heat * town", "Adjusted Wald F", 0.834890793367),
            ("heat * town", "df1", 1.68186634636),
        ],
    );
}

#[test]
fn csglm_saves_predicted_values_and_residuals() {
    let source = format!(
        "{}{GLM} /SAVE PRED RESID(res) /PRINT NONE.\n{GLM} /SAVE PRED /PRINT NONE.\n\
         {GLM} /SAVE PRED RESID(Predicted_2) /STATISTICS SE CINTERVAL /PRINT NONE.\n",
        data("save")
    );
    let (engine, tables) = run(&source);
    // the interval without PARAMETER
    checks(
        find(&tables, "Parameter Estimates", 2),
        &[("members", LOWER, 48.2171793615)],
    );
    // a name given in SAVE is kept for its column; the default name moves on
    assert_eq!(column(&engine, "Predicted_3"), column(&engine, "Predicted"));
    let pred = column(&engine, "Predicted");
    let res = column(&engine, "res");
    assert!((pred[0].unwrap() - 454.013064788).abs() < 1e-7);
    assert!((res[0].unwrap() + 1.01306478826).abs() < 1e-7);
    // case 6 has a user-missing heating code: no prediction
    assert_eq!(pred[5], None);
    // the second run finds the name taken and adds a suffix
    assert!(engine.dataset().unwrap().variable("Predicted_1").is_ok());
}

const LOGISTIC: &str = "CSLOGISTIC online BY heat WITH members /PLAN FILE='wr.json'";

#[test]
fn cslogistic_binary_estimates_pseudo_r_squares_odds_ratios_and_classification() {
    let source = format!(
        "{}{LOGISTIC} /STATISTICS PARAMETER EXP SE CINTERVAL TTEST DEFF
           /ODDSRATIOS FACTOR=[heat(1)] /ODDSRATIOS COVARIATE=[members(2)]
           /PRINT SUMMARY CLASSTABLE HISTORY.
         {LOGISTIC} /TEST TYPE=ADJF /PRINT NONE.\n",
        data("binary")
    );
    let (_, tables) = run(&source);
    let p = find(&tables, "Parameter Estimates", 0);
    // the reference category is the highest, 1: the logit is of having no line
    checks(
        p,
        &[
            ("0 / Intercept", "Estimate", 0.622498689738),
            ("0 / Intercept", "Std. Error", 0.438654265864),
            ("0 / Intercept", "Design Effect", 0.67173071143),
            ("0 / [heat=1]", "Estimate", -0.0672083432301),
            ("0 / [heat=1]", "Std. Error", 0.511701491808),
            ("0 / [heat=1]", "Design Effect", 1.16689667508),
            ("0 / [heat=2]", "Estimate", -0.152056529934),
            ("0 / [heat=2]", "Design Effect", 1.04639839092),
            ("0 / members", "Estimate", -0.0762702570918),
            ("0 / members", "Std. Error", 0.135394332312),
            ("0 / members", "Design Effect", 0.680536553681),
            ("0 / members", "Exp(B)", (-0.0762702570918_f64).exp()),
            (
                "0 / members",
                "95% Confidence Interval for Exp(B) / Lower",
                0.695382364716,
            ),
        ],
    );
    checks(
        find(&tables, "Pseudo R Squares", 0),
        &[
            ("", "Cox and Snell", 0.00299950801945),
            ("", "Nagelkerke", 0.0040369873461),
            ("", "McFadden", 0.00221094079283),
        ],
    );
    let history = find(&tables, "Iteration History", 0);
    checks(history, &[("0", "-2 Log Pseudo Likelihood", 16864.6142605)]);
    let last = history.rows.last().unwrap();
    assert!(matches!(last[1], Cell::Number(v) if (v - 16827.3275968).abs() < 1e-6));
    let or = find(&tables, "Odds Ratios", 0);
    checks(
        or,
        &[
            ("0 / heat 2 vs. 1", "Odds Ratio", 0.918651737364),
            ("0 / heat 2 vs. 1", LOWER, 0.386899444441),
            ("0 / heat 2 vs. 1", UPPER, 2.18124121574),
            ("0 / heat 3 vs. 1", "Odds Ratio", 1.06951828186),
            ("0 / heat 3 vs. 1", LOWER, 0.361479350448),
        ],
    );
    checks(
        find(&tables, "Odds Ratios", 1),
        &[
            ("0 / members up by 2", "Odds Ratio", 0.858524110848),
            ("0 / members up by 2", UPPER, 1.52425506831),
        ],
    );
    checks(
        find(&tables, "Classification", 0),
        &[
            ("0", "Predicted / 0", 7234.6082),
            ("0", "Predicted / 1", 0.0),
            ("1", "Predicted / 0", 5177.6656),
            (
                "Overall Percent",
                "Predicted / Percent Correct",
                100.0 * 7234.6082 / 12412.2738,
            ),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 1),
        &[
            ("heat", "Adjusted Wald F", 0.0444581791169),
            ("heat", "df1", 1.9544253043),
            ("heat", "Sig.", 0.953982489473),
            ("(Corrected Model)", "Adjusted Wald F", 0.146266399586),
            ("(Corrected Model)", "df2", 39.023063787),
        ],
    );
}

#[test]
fn cslogistic_multinomial_with_custom_tests_and_saved_probabilities() {
    let source = format!(
        "{}CSLOGISTIC heat (LOW) BY town WITH members /PLAN FILE='wr.json'
           /STATISTICS PARAMETER SE DEFF /CUSTOM LMATRIX=ALL 0 0 0 1 0 0 0 0 ALL 0 0 0 0 0 0 0 -1
           /CUSTOM LMATRIX=members 1 /PRINT SUMMARY CLASSTABLE /SAVE PREDPROB(pp) PREDVAL.
         CSLOGISTIC heat (LOW) BY town WITH members /PLAN FILE='wr.json' /PRINT NONE
           /SAVE PREDPROB(pp:1).\n",
        data("multinomial")
    );
    let (engine, tables) = run(&source);
    let p = find(&tables, "Parameter Estimates", 0);
    checks(
        p,
        &[
            ("2 / Intercept", "Estimate", -1.10152221436),
            ("2 / Intercept", "Std. Error", 0.614090943378),
            ("2 / Intercept", "Design Effect", 1.05240409314),
            ("2 / [town=1]", "Estimate", 1.53286443961),
            ("2 / [town=1]", "Std. Error", 0.4639150949),
            ("2 / [town=1]", "Design Effect", 0.692144310785),
            ("2 / members", "Estimate", 0.124709736938),
            ("2 / members", "Design Effect", 1.3196310086),
            ("3 / Intercept", "Estimate", -0.192121298802),
            ("3 / [town=1]", "Std. Error", 0.727828799807),
            ("3 / members", "Estimate", -0.336306689652),
            ("3 / members", "Std. Error", 0.268186159308),
            ("3 / members", "Design Effect", 1.61078356812),
        ],
    );
    checks(
        find(&tables, "Pseudo R Squares", 0),
        &[
            ("", "Cox and Snell", 0.145726951315),
            ("", "Nagelkerke", 0.165291124296),
            ("", "McFadden", 0.0738068382174),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 0),
        &[
            ("(Corrected Model)", "Wald F", 2.88579499237),
            ("(Intercept)", "Wald F", 0.943201451701),
            ("town", "Wald F", 5.14259854198),
            ("town", "df1", 2.0),
            ("town", "Sig.", 0.0199153007804),
        ],
    );
    checks(
        find(&tables, "Custom Hypothesis Tests", 0),
        &[
            ("Contrast 1", "Estimate", 0.461016426591),
            ("Contrast 1", "Std. Error", 0.242871763916),
            ("Contrast 1", "Sig.", 0.0758619326436),
        ],
    );
    let each = find(&tables, "Custom Hypothesis Tests", 1);
    checks(
        each,
        &[
            ("Contrast 1 (2)", "Estimate", 0.124709736938),
            ("Contrast 1 (3)", "Estimate", -0.336306689652),
            ("Overall Test", "df1", 2.0),
        ],
    );
    checks(
        find(&tables, "Classification", 0),
        &[
            ("1", "Predicted / 1", 4981.3815),
            ("2", "Predicted / 3", 853.6766),
            ("3", "Predicted / 2", 802.38),
        ],
    );
    let probs: Vec<Vec<Option<f64>>> = (1..=3)
        .map(|k| column(&engine, &format!("pp_{k}")))
        .collect();
    let total: f64 = probs.iter().map(|p| p[0].unwrap()).sum();
    assert!((total - 1.0).abs() < 1e-12);
    let predicted = column(&engine, "PredictedValue");
    let best = (0..3)
        .max_by(|&a, &b| probs[a][0].unwrap().total_cmp(&probs[b][0].unwrap()))
        .unwrap();
    assert_eq!(predicted[0], Some(best as f64 + 1.0));
    // the saved category keeps the dependent's format; a second run finds pp_1 taken
    let data = engine.dataset().unwrap();
    assert_eq!(data.variable("PredictedValue").unwrap().format(), "F8.2");
    assert_eq!(column(&engine, "pp_1_1"), probs[0]);
}

#[test]
fn model_errors() {
    let d = data("errors");
    let cases = [
        (
            "CSGLM food BY district /PLAN FILE='wr.json'.",
            "district is a plan variable",
        ),
        (
            "CSGLM food WITH members /PLAN FILE='wr.json' /DOMAIN VARIABLE=w(1).",
            "plan variable other than a stratum or cluster",
        ),
        (
            "CSGLM food WITH members /PLAN FILE='wr.json' /DOMAIN VARIABLE=members(1).",
            "in the model",
        ),
        (
            "CSGLM food WITH members /PLAN FILE='wr.json' /INTERCEPT INCLUDE=NO /CUSTOM KMATRIX=1.",
            "does not have",
        ),
        (
            "CSGLM food BY heat /PLAN FILE='wr.json' /CUSTOM LMATRIX=heat 1 0 0.",
            "not estimable",
        ),
        (
            "CSGLM food BY heat /PLAN FILE='wr.json' /CUSTOM LMATRIX=heat 1 -1.",
            "coefficients",
        ),
        (
            "CSLOGISTIC food BY heat /PLAN FILE='wr.json' /DOMAIN VARIABLE=town(7).",
            "no case",
        ),
        (
            "CSLOGISTIC online BY heat town /PLAN FILE='wr.json' /MODEL heat /ODDSRATIOS FACTOR=[town].",
            "in no effect",
        ),
    ];
    for (syntax, expected) in cases {
        let error = run_error(&format!("{d}{syntax}\n"));
        assert!(error.contains(expected), "{syntax}: {error}");
    }
}

// CSORDINAL: heat (1, 2, 3) as an ordered response, user-missing 9 out of the model but in the
// design. The expected estimates come from the survey library's svyolr (fitted with a tight
// optimizer tolerance) for the logit and probit links, and for the other links from maximizing
// the manual's weighted log likelihood in the same package. The standard errors come from the
// manual's sandwich computed there: the package's design covariance of the total of the case
// scores (svytotal) between inverse Hessians taken by Richardson extrapolation (numDeriv);
// svyolr's own standard errors agree with them to about 1e-6. The pseudo R squares, the Wald F
// and the design effects use the manual's formulas on those numbers.

const ORDINAL: &str = "CSORDINAL heat BY town WITH members /PLAN FILE='wr.json'";

#[test]
fn csordinal_logit_estimates_design_effects_and_tests() {
    let source = format!(
        "{}{ORDINAL} /STATISTICS PARAMETER SE CINTERVAL TTEST DEFF EXP
           /PRINT SUMMARY HISTORY COVB CLASSTABLE.
         {ORDINAL} /CRITERIA METHOD=FISHER /PRINT NONE /STATISTICS PARAMETER.\n",
        data("ordinal")
    );
    let (_, tables) = run(&source);
    let p = find(&tables, "Parameter Estimates", 0);
    checks(
        p,
        &[
            ("Threshold / [heat=1]", "Estimate", -0.2696434311753),
            ("Threshold / [heat=1]", "Std. Error", 0.6908148018072),
            ("Threshold / [heat=1]", "Design Effect", 1.516999593262),
            ("Threshold / [heat=2]", "Estimate", 1.1674668789298),
            ("Threshold / [heat=2]", "Std. Error", 0.8339282328597),
            ("Threshold / [heat=2]", "Design Effect", 1.862936327308),
            ("Location / [town=1]", "Estimate", 1.3593620052443),
            ("Location / [town=1]", "Std. Error", 0.4767017577092),
            ("Location / [town=1]", "Design Effect", 1.452386656131),
            ("Location / [town=1]", T, 1.3593620052443 / 0.4767017577092),
            ("Location / [town=1]", "Exp(B)", 1.3593620052443_f64.exp()),
            ("Location / members", "Estimate", -0.1471277610708),
            ("Location / members", "Std. Error", 0.2153695733205),
            ("Location / members", "Design Effect", 1.802547829599),
            (
                "Location / members",
                LOWER,
                -0.1471277610708 - 2.119905299221 * 0.2153695733205,
            ),
        ],
    );
    assert!(matches!(
        cell(p, "Location / [town=2] (redundant)", "Estimate"),
        Cell::Number(v) if *v == 0.0
    ));
    // -2 log likelihoods 24977.39769894 (model) and 26487.89561452 (thresholds only), N the
    // summed weights 12412.2738
    checks(
        find(&tables, "Pseudo R Squares", 0),
        &[
            ("", "Cox and Snell", 0.11458063961746567),
            ("", "Nagelkerke", 0.1299633497714196),
            ("", "McFadden", 0.05702596905252011),
        ],
    );
    let history = find(&tables, "Iteration History", 0);
    checks(
        history,
        &[("0", "-2 Log Pseudo Likelihood", 26487.89561452)],
    );
    let last = history.rows.last().unwrap();
    assert!(matches!(last[1], Cell::Number(v) if (v - 24977.39769894).abs() < 1e-6));
    // Wald X2 of both coefficients 11.94322029468, F = (16 - 2 + 1) / (2 16) X2
    checks(
        find(&tables, "Tests of Model Effects", 0),
        &[
            ("(Corrected Model)", "Wald F", 15.0 / 32.0 * 11.94322029468),
            ("(Corrected Model)", "df2", 15.0),
            (
                "town",
                "Wald F",
                (1.3593620052443_f64 / 0.4767017577092).powi(2),
            ),
        ],
    );
    let covb = find(&tables, "Covariances of Parameter Estimates", 0);
    checks(
        covb,
        &[(
            "Threshold [heat=2]",
            "Threshold [heat=2]",
            0.8339282328597_f64.powi(2),
        )],
    );
    assert_eq!(covb.rows.len(), 4);
    let class = find(&tables, "Classification", 0);
    assert_eq!(class.rows.len(), 4);
    // Fisher scoring until convergence and then Newton-Raphson reach the same estimates
    checks(
        find(&tables, "Parameter Estimates", 1),
        &[("Location / [town=1]", "Estimate", 1.3593620052443)],
    );
}

#[test]
fn csordinal_other_links_descending_domain_and_two_stages() {
    let source = format!(
        "{}CSPLAN ANALYSIS /PLAN FILE='two.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=district CLUSTER=village /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nvill
           /DESIGN CLUSTER=hh /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=nhh.
         CSORDINAL heat WITH members /PLAN FILE='wr.json' /LINK FUNCTION=CLOGLOG
           /STATISTICS PARAMETER SE /PRINT NONE.
         CSORDINAL heat BY town /PLAN FILE='wr.json' /LINK FUNCTION=NLOGLOG
           /STATISTICS PARAMETER SE /PRINT NONE.
         CSORDINAL heat WITH members /PLAN FILE='wr.json' /LINK FUNCTION=CAUCHIT
           /STATISTICS PARAMETER SE /PRINT NONE.
         CSORDINAL heat (DESCENDING) BY town WITH members /PLAN FILE='two.json'
           /LINK FUNCTION=PROBIT /DOMAIN VARIABLE=online(1) /CRITERIA METHOD=FISHER(3)
           /STATISTICS PARAMETER SE EXP /PRINT SUMMARY.\n",
        data("ordinal-links")
    );
    let (engine, tables) = run(&source);
    // the categories from the highest down: thresholds at 3 and 2
    let p = find(&tables, "Parameter Estimates", 3);
    checks(
        p,
        &[
            ("Threshold / [heat=3]", "Estimate", -1.42250360876398),
            ("Threshold / [heat=3]", "Std. Error", 0.7644948795249),
            ("Threshold / [heat=2]", "Estimate", -0.30888310400079),
            ("Threshold / [heat=2]", "Std. Error", 0.6313620557213),
            ("Location / [town=1]", "Estimate", -1.48907406618779),
            ("Location / [town=1]", "Std. Error", 0.3319895444865),
            ("Location / members", "Estimate", 0.04545198916347),
            ("Location / members", "Std. Error", 0.2112081273894),
        ],
    );
    // EXP needs the logit link
    assert!(!p.column_titles().iter().any(|t| t == "Exp(B)"));
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("EXP needs the logit link"))
    );
    // -2 log likelihoods 9104.691713765 and 11013.12000468 over the domain's 5177.6656
    checks(
        find(&tables, "Pseudo R Squares", 0),
        &[
            ("", "Cox and Snell", 0.30829004870976173),
            ("", "Nagelkerke", 0.35000709915821976),
            ("", "McFadden", 0.17328679702972616),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects", 3),
        &[("(Corrected Model)", "Wald F", 15.0 / 32.0 * 22.25532996719)],
    );
    checks(
        find(&tables, "Parameter Estimates", 0),
        &[
            ("Threshold / [heat=1]", "Estimate", -1.1534281878369),
            ("Threshold / [heat=1]", "Std. Error", 0.4170797852392),
            ("Threshold / [heat=2]", "Estimate", -0.2880558362289),
            ("Location / members", "Estimate", -0.2245628128587),
            ("Location / members", "Std. Error", 0.1268759546030),
        ],
    );
    checks(
        find(&tables, "Parameter Estimates", 1),
        &[
            ("Threshold / [heat=1]", "Estimate", 0.5135886830021),
            ("Threshold / [heat=2]", "Std. Error", 0.2550921329121),
            ("Location / [town=1]", "Estimate", 1.0405024013230),
            ("Location / [town=1]", "Std. Error", 0.2881318040469),
        ],
    );
    checks(
        find(&tables, "Parameter Estimates", 2),
        &[
            ("Threshold / [heat=1]", "Estimate", -0.5861347774668),
            ("Threshold / [heat=2]", "Estimate", 0.5604389820631),
            ("Threshold / [heat=2]", "Std. Error", 0.7997897850824),
            ("Location / members", "Std. Error", 0.1957400692590),
        ],
    );
}

#[test]
fn csordinal_thresholds_only_and_errors() {
    let d = data("ordinal-errors");
    let (_, tables) = run(&format!(
        "{d}CSORDINAL heat /PLAN FILE='wr.json' /STATISTICS PARAMETER /PRINT SUMMARY.\n"
    ));
    // without predictors the start is the fit: the thresholds are the logits of the weighted
    // cumulative shares of the categories (weights summed by hand from the data file), and the
    // pseudo R squares are 0
    let p = find(&tables, "Parameter Estimates", 0);
    let logit = |s: f64| (s / (1.0 - s)).ln();
    checks(
        p,
        &[
            (
                "Threshold / [heat=1]",
                "Estimate",
                logit(5591.4081 / 12412.2738),
            ),
            (
                "Threshold / [heat=2]",
                "Estimate",
                logit(9302.0389 / 12412.2738),
            ),
        ],
    );
    assert!(matches!(
        cell(find(&tables, "Pseudo R Squares", 0), "", "McFadden"),
        Cell::Number(v) if v.abs() < 1e-12
    ));
    let cases = [
        (
            "CSORDINAL heat WITH members /PLAN FILE='wr.json' /DOMAIN VARIABLE=members(2).",
            "in the model",
        ),
        (
            "COMPUTE one = 1.\nCSORDINAL one WITH members /PLAN FILE='wr.json'.",
            "CSORDINAL: the dependent variable one has fewer than two categories",
        ),
    ];
    for (syntax, expected) in cases {
        let error = run_error(&format!("{d}{syntax}\n"));
        assert!(error.contains(expected), "{syntax}: {error}");
    }
}
