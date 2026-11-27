//! LOGISTIC REGRESSION options against R's `glm` on 150 invented cases (age, income, a
//! three-level region and a two-level sex as categorical covariates with the last category the
//! reference, frequency weights, a selection flag, missing values in the dependent variable and
//! in age): two blocks with the step, block and model chi-squares, the CUT value of the
//! classification table, PRINT=CORR and CI(90); an interaction of a covariate with a categorical
//! variable (its Wald test on the joint coefficients); ORIGIN (no constant, the initial model
//! has probability 1/2); case weights (frequencies); and SELECT, which fits the selected cases
//! and classifies the unselected ones by the fitted model. Scripts in lab/oracles/logistic-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y age inc region sex w sel (7F8.2).
BEGIN DATA.
1 45 30 3 1 2 1
0 50 29.5 2 1 1 0
0 51 46 3 1 2 1
0 33 28.5 2 2 3 1
1 56 51.8 3 2 1 1
0 36 53.8 3 1 2 1
0 . 47.9 3 2 1 1
0 55 42.6 1 2 3 1
1 36 59.2 1 2 2 0
1 32 47.4 2 1 1 1
1 59 55.8 3 2 3 1
0 35 55.6 1 1 2 1
0 49 65.3 3 2 1 1
0 52 21 2 1 3 0
. 41 42.6 3 1 1 1
0 39 54.8 2 1 1 1
1 44 44.4 3 1 3 1
0 40 25.8 1 1 3 1
1 49 63 3 2 1 1
1 41 82.6 2 1 1 1
0 50 42.4 1 2 2 0
1 35 26.8 1 2 1 1
0 29 52.1 3 1 1 1
0 44 60.5 2 2 2 1
1 42 46.3 1 1 1 0
0 53 55.8 3 2 1 1
0 19 17 3 2 1 0
1 36 28.9 1 1 1 1
0 44 65.2 2 2 2 1
1 42 70 1 1 2 0
1 42 76.1 2 1 2 1
0 26 44.5 3 1 3 1
0 39 32.4 2 1 2 0
1 54 53.7 1 2 1 0
0 40 40.9 1 1 3 1
0 46 43 1 1 1 1
1 40 79.4 2 2 1 1
0 52 49.6 1 2 1 1
0 41 53.5 1 1 2 0
0 43 62.1 3 2 2 1
0 53 55 2 2 1 0
0 55 47.1 3 2 3 1
0 43 28.2 1 1 2 0
0 45 52.4 2 2 2 1
0 40 56.3 2 2 2 1
0 37 53.9 3 1 1 1
1 58 69.7 3 2 1 0
1 32 80.7 1 1 1 1
1 63 40.9 1 2 1 0
0 45 20.3 1 1 1 0
1 57 33.2 3 1 3 1
1 59 59.7 1 2 1 1
0 35 33.2 3 1 3 1
0 50 37.1 3 1 1 1
1 20 47.7 2 1 1 1
1 60 64 1 2 3 1
1 37 65.8 1 2 1 1
1 48 46.3 2 2 2 1
0 40 33.8 2 1 1 1
0 44 58.7 1 2 2 1
0 . 62.5 1 1 1 1
0 40 42.5 1 1 1 1
0 24 47.4 3 1 2 1
0 52 50.5 3 1 2 1
1 47 83.9 2 2 1 1
0 42 51.6 2 2 2 1
0 39 40.9 2 2 1 1
0 43 61.2 2 2 1 1
1 51 69.1 3 2 1 1
1 41 43 3 1 1 1
1 57 63.3 3 2 2 1
0 25 40 2 2 3 1
0 43 47.5 1 1 1 1
1 36 46 2 1 2 1
0 34 70.8 1 2 2 1
0 36 56.3 1 2 1 1
1 40 80.5 3 1 2 1
0 24 13.4 2 1 1 1
0 20 64.6 1 1 1 1
0 46 63.4 1 2 1 0
0 57 18.6 2 2 1 1
0 44 53.8 2 2 3 0
0 42 66.4 1 2 3 1
0 46 47.3 2 1 1 1
1 66 26.8 3 2 1 1
0 44 48.9 2 2 1 0
1 52 74.7 1 1 1 1
1 47 50.6 2 2 1 1
0 34 18.8 3 1 1 1
1 43 64.8 2 1 2 0
1 43 68.2 1 2 3 0
0 42 52.4 1 2 1 1
0 51 52.5 3 2 1 1
0 42 54.4 2 2 1 0
0 33 64 2 2 3 1
0 32 65 3 1 1 1
0 49 42.4 1 1 1 0
0 43 58.1 1 2 1 1
. 46 56.8 3 1 1 1
1 24 61.3 1 1 1 1
0 41 32.4 1 2 1 1
0 33 47.5 2 2 2 1
0 65 32.2 3 1 2 1
0 40 53.4 1 2 2 1
0 63 40.8 1 2 2 1
1 48 60.3 3 2 2 1
1 60 31 1 2 1 0
0 40 56.1 2 1 1 1
0 52 64.5 1 1 2 0
0 22 23.8 2 2 2 1
0 20 69.3 2 2 2 1
0 73 47.1 1 2 3 0
0 44 42.1 3 1 1 1
1 54 53.6 1 1 1 1
0 37 46.8 3 2 1 1
0 40 39.7 2 2 2 0
0 48 73.7 2 2 1 1
1 46 61.2 1 2 1 1
0 53 61.5 2 1 1 1
1 . 41.1 3 1 1 0
0 40 47.5 1 2 1 1
0 42 40 3 1 2 1
0 49 56.8 1 1 1 0
1 53 65 1 1 1 1
0 39 44.5 2 2 2 0
0 55 43.7 3 2 3 1
0 45 35.9 3 1 2 1
0 43 36.4 2 2 2 0
0 46 45.8 2 1 1 1
0 48 36.7 2 2 1 0
1 33 24.5 1 2 2 0
1 35 55.5 3 2 1 1
0 44 42 2 2 3 1
1 50 40 3 1 2 1
1 35 51.3 1 1 3 1
0 43 48.9 1 2 1 1
1 74 60 2 1 1 1
1 55 66 1 1 1 1
0 59 35.7 1 2 1 1
0 62 29.2 3 1 2 1
1 51 67.1 2 1 3 1
0 49 27.9 2 2 1 1
1 51 54.2 3 1 1 1
0 47 54.4 1 1 1 1
0 57 51 1 1 1 0
1 41 43.5 2 1 1 1
0 62 25.7 3 1 3 0
1 63 24.5 1 1 2 1
0 30 33.1 2 1 2 1
1 38 40.5 3 1 1 0
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell
                    && !t.is_empty()
                {
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

/// Each expectation: the table title, which table of that title counting from the last (the
/// blocks each print their own), the row, the column and the value.
fn check(tables: &[Table], expected: &[(&str, usize, &str, &str, f64)]) {
    for (title, from_end, row, column, want) in expected {
        let same: Vec<&Table> = tables.iter().filter(|t| t.title == *title).collect();
        let t = same
            .get(same.len().wrapping_sub(1 + from_end))
            .unwrap_or_else(|| panic!("no table {title} ({from_end} from the end)"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-6 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn two_blocks_cut_value_correlations_and_intervals_agree_with_r() {
    let t = run(
        "LOGISTIC REGRESSION y WITH age inc region sex /CATEGORICAL=region sex /METHOD=ENTER age /METHOD=ENTER inc region sex /PRINT=CORR CI(90) /CRITERIA=CUT(0.4).",
    );
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Step",
                "Chi-square",
                3.584082913,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Step",
                "df",
                1.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Block",
                "Chi-square",
                3.584082913,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Block",
                "df",
                1.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Model",
                "Chi-square",
                3.584082913,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                1,
                "Step 1 / Model",
                "df",
                1.0,
            ),
            (
                "Model Summary",
                1,
                "Step 1",
                "-2 Log likelihood",
                185.6758499,
            ),
            (
                "Model Summary",
                1,
                "Step 1",
                "Cox & Snell R Square",
                0.02441482955,
            ),
            (
                "Model Summary",
                1,
                "Step 1",
                "Nagelkerke R Square",
                0.03349577604,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 0",
                "Predicted / 0",
                73.0,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 0",
                "Predicted / 1",
                20.0,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                78.49462366,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 1",
                "Predicted / 0",
                32.0,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 1",
                "Predicted / 1",
                20.0,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                38.46153846,
            ),
            (
                "Classification Table",
                1,
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                64.13793103,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "Chi-square",
                17.64147002,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "df",
                4.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "Chi-square",
                17.64147002,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "df",
                4.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "Chi-square",
                21.22555293,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "df",
                5.0,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "-2 Log likelihood",
                168.0343799,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Cox & Snell R Square",
                0.1361733127,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Nagelkerke R Square",
                0.1868221433,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 0",
                68.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 1",
                25.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                73.11827957,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 0",
                24.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 1",
                28.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                53.84615385,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                66.20689655,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "B",
                -4.953482385,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "S.E.",
                1.361704599,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Wald",
                13.23291239,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Sig.",
                0.0002750760693,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Exp(B)",
                0.007058784672,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "B",
                0.03830293671,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "S.E.",
                0.01935660763,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Wald",
                3.915666111,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Sig.",
                0.04783805405,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Exp(B)",
                1.03904595,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "90% C.I.for EXP(B) / Lower",
                1.006485088,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "90% C.I.for EXP(B) / Upper",
                1.072660191,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "B",
                0.05066726511,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "S.E.",
                0.01420736384,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Wald",
                12.71826649,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Sig.",
                0.0003621012621,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Exp(B)",
                1.051972807,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "90% C.I.for EXP(B) / Lower",
                1.027674242,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "90% C.I.for EXP(B) / Upper",
                1.076845893,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "B",
                -0.07881743734,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "S.E.",
                0.4466547137,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Wald",
                0.03113872149,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Sig.",
                0.8599312579,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Exp(B)",
                0.924208635,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "90% C.I.for EXP(B) / Lower",
                0.4433041999,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "90% C.I.for EXP(B) / Upper",
                1.926806922,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "B",
                -0.5950739541,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "S.E.",
                0.4979634569,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Wald",
                1.428061588,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Sig.",
                0.232081049,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Exp(B)",
                0.5515217771,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "90% C.I.for EXP(B) / Lower",
                0.2431319565,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "90% C.I.for EXP(B) / Upper",
                1.251074828,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "B",
                0.5976632592,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "S.E.",
                0.3915523851,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Wald",
                2.329879016,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Sig.",
                0.1269117406,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Exp(B)",
                1.817865952,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "90% C.I.for EXP(B) / Lower",
                0.9546761541,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "90% C.I.for EXP(B) / Upper",
                3.461526304,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Wald",
                1.702013211,
            ),
            ("Variables in the Equation", 0, "Step 1 / region", "df", 2.0),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Sig.",
                0.4269849102,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "Constant",
                1.0,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "age",
                -0.7968358524,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "inc",
                -0.6623944414,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "region(1)",
                -0.177798258,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "region(2)",
                -0.2249140381,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / Constant",
                "sex(1)",
                -0.4240549655,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / age",
                "Constant",
                -0.7968358524,
            ),
            ("Correlation Matrix", 0, "Step 1 / age", "age", 1.0),
            ("Correlation Matrix", 0, "Step 1 / age", "inc", 0.1780910431),
            (
                "Correlation Matrix",
                0,
                "Step 1 / age",
                "region(1)",
                0.04905622752,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / age",
                "region(2)",
                0.156748908,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / age",
                "sex(1)",
                0.213036154,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / inc",
                "Constant",
                -0.6623944414,
            ),
            ("Correlation Matrix", 0, "Step 1 / inc", "age", 0.1780910431),
            ("Correlation Matrix", 0, "Step 1 / inc", "inc", 1.0),
            (
                "Correlation Matrix",
                0,
                "Step 1 / inc",
                "region(1)",
                -0.09335164134,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / inc",
                "region(2)",
                -0.1182340051,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / inc",
                "sex(1)",
                0.2189511099,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "Constant",
                -0.177798258,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "age",
                0.04905622752,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "inc",
                -0.09335164134,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "region(1)",
                1.0,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "region(2)",
                0.5214050885,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(1)",
                "sex(1)",
                0.08064943169,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "Constant",
                -0.2249140381,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "age",
                0.156748908,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "inc",
                -0.1182340051,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "region(1)",
                0.5214050885,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "region(2)",
                1.0,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / region(2)",
                "sex(1)",
                0.1150883457,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / sex(1)",
                "Constant",
                -0.4240549655,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / sex(1)",
                "age",
                0.213036154,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / sex(1)",
                "inc",
                0.2189511099,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / sex(1)",
                "region(1)",
                0.08064943169,
            ),
            (
                "Correlation Matrix",
                0,
                "Step 1 / sex(1)",
                "region(2)",
                0.1150883457,
            ),
            ("Correlation Matrix", 0, "Step 1 / sex(1)", "sex(1)", 1.0),
        ],
    );
}

#[test]
fn an_interaction_with_a_categorical_variable_agree_with_r() {
    let t = run(
        "LOGISTIC REGRESSION y WITH age region /CATEGORICAL=region /METHOD=ENTER age region age BY region.",
    );
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "Chi-square",
                6.75708451,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "Chi-square",
                6.75708451,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "Chi-square",
                6.75708451,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "df",
                5.0,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "-2 Log likelihood",
                182.5028483,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Cox & Snell R Square",
                0.0455314474,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Nagelkerke R Square",
                0.06246659072,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 0",
                88.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 1",
                5.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                94.62365591,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 0",
                46.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 1",
                6.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                11.53846154,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                64.82758621,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "B",
                -2.917249518,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "S.E.",
                1.542082726,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Wald",
                3.57875369,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Sig.",
                0.05852304279,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Exp(B)",
                0.05408223511,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "B",
                0.05417796672,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "S.E.",
                0.03176650856,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Wald",
                2.908750262,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Sig.",
                0.08810011904,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Exp(B)",
                1.05567246,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "B",
                2.204245073,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "S.E.",
                1.996328247,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Wald",
                1.219146381,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Sig.",
                0.2695282401,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Exp(B)",
                9.063406774,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "B",
                0.8401422573,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "S.E.",
                2.146904992,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Wald",
                0.1531369548,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Sig.",
                0.6955554174,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Exp(B)",
                2.31669652,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(1)",
                "B",
                -0.04677522279,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(1)",
                "S.E.",
                0.04170933948,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(1)",
                "Wald",
                1.257665376,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(1)",
                "Sig.",
                0.262093466,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(1)",
                "Exp(B)",
                0.9543018788,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(2)",
                "B",
                -0.02901428848,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(2)",
                "S.E.",
                0.04670708256,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(2)",
                "Wald",
                0.3858854179,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(2)",
                "Sig.",
                0.5344701692,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region(2)",
                "Exp(B)",
                0.9714025845,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Wald",
                1.289904436,
            ),
            ("Variables in the Equation", 0, "Step 1 / region", "df", 2.0),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Sig.",
                0.5246876122,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region",
                "Wald",
                1.259884076,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region",
                "df",
                2.0,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age by region",
                "Sig.",
                0.532622672,
            ),
        ],
    );
}

#[test]
fn no_constant_with_origin_agree_with_r() {
    let t = run("LOGISTIC REGRESSION y WITH age inc /ORIGIN /METHOD=ENTER age inc.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "Chi-square",
                9.786425634,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "df",
                2.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "Chi-square",
                9.786425634,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "df",
                2.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "Chi-square",
                9.786425634,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "df",
                2.0,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "-2 Log likelihood",
                191.2262567,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Cox & Snell R Square",
                0.06526535357,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Nagelkerke R Square",
                0.08702047142,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 0",
                91.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 1",
                2.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                97.84946237,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 0",
                49.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 1",
                3.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                5.769230769,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                64.82758621,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "B",
                -0.02196335611,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "S.E.",
                0.01067141748,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Wald",
                4.235973027,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Sig.",
                0.0395759323,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Exp(B)",
                0.9782760822,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "B",
                0.01045752075,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "S.E.",
                0.009252753187,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Wald",
                1.277366478,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Sig.",
                0.258389269,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Exp(B)",
                1.010512392,
            ),
        ],
    );
}

#[test]
fn case_weights_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nLOGISTIC REGRESSION y WITH age inc region sex /CATEGORICAL=region sex /METHOD=ENTER age inc region sex.",
    );
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "Chi-square",
                40.65004802,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "Chi-square",
                40.65004802,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "Chi-square",
                40.65004802,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "df",
                5.0,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "-2 Log likelihood",
                259.4273087,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Cox & Snell R Square",
                0.1588446603,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Nagelkerke R Square",
                0.220279089,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 0",
                145.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / 1",
                11.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                92.94871795,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 0",
                46.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / 1",
                33.0,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                41.7721519,
            ),
            (
                "Classification Table",
                0,
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                75.74468085,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "B",
                -5.786319736,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "S.E.",
                1.15379735,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Wald",
                25.15045127,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Sig.",
                5.302724372e-07,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Exp(B)",
                0.003069257092,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "B",
                0.04256609153,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "S.E.",
                0.01555308424,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Wald",
                7.490218737,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Sig.",
                0.006203502028,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Exp(B)",
                1.04348502,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "B",
                0.05902342289,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "S.E.",
                0.0121187726,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Wald",
                23.72091899,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Sig.",
                1.11365448e-06,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Exp(B)",
                1.060800087,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "B",
                -0.1122353168,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "S.E.",
                0.3603665121,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Wald",
                0.096999662,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Sig.",
                0.7554603059,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Exp(B)",
                0.8938338986,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "B",
                -0.5956836709,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "S.E.",
                0.4102276217,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Wald",
                2.108538776,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Sig.",
                0.1464791278,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Exp(B)",
                0.5511856075,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "B",
                0.8898759917,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "S.E.",
                0.328703783,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Wald",
                7.329081907,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Sig.",
                0.006784770114,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Exp(B)",
                2.434827694,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Wald",
                2.337086879,
            ),
            ("Variables in the Equation", 0, "Step 1 / region", "df", 2.0),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Sig.",
                0.3108193389,
            ),
        ],
    );
}

#[test]
fn select_fits_the_selected_cases_and_classifies_the_others_agree_with_r() {
    let t = run(
        "LOGISTIC REGRESSION y WITH age inc region sex /CATEGORICAL=region sex /SELECT=sel EQ 1 /METHOD=ENTER age inc region sex.",
    );
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "Chi-square",
                20.21815212,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Step",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "Chi-square",
                20.21815212,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Block",
                "df",
                5.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "Chi-square",
                20.21815212,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                0,
                "Step 1 / Model",
                "df",
                5.0,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "-2 Log likelihood",
                125.9953092,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Cox & Snell R Square",
                0.1665199519,
            ),
            (
                "Model Summary",
                0,
                "Step 1",
                "Nagelkerke R Square",
                0.2274471364,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 0",
                "Predicted / 0",
                63.0,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 0",
                "Predicted / 1",
                7.0,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 0",
                "Predicted / Percentage Correct",
                90.0,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 1",
                "Predicted / 0",
                23.0,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 1",
                "Predicted / 1",
                18.0,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / 1",
                "Predicted / Percentage Correct",
                43.90243902,
            ),
            (
                "Classification Table",
                0,
                "Selected Cases / Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                72.97297297,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "B",
                -6.521140292,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "S.E.",
                1.695061046,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Wald",
                14.80050043,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Sig.",
                0.0001195037851,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / Constant",
                "Exp(B)",
                0.001471989647,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "B",
                0.0603649508,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "S.E.",
                0.02347702542,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Wald",
                6.61124972,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Sig.",
                0.01013365203,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / age",
                "Exp(B)",
                1.062224135,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "B",
                0.05477788875,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "S.E.",
                0.01724927795,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Wald",
                10.08483364,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Sig.",
                0.001494946585,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / inc",
                "Exp(B)",
                1.056305971,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "B",
                0.01011133573,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "S.E.",
                0.520420564,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Wald",
                0.0003774923575,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Sig.",
                0.9844987464,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(1)",
                "Exp(B)",
                1.010162628,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "B",
                -0.1546321543,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "S.E.",
                0.5601742434,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Wald",
                0.0761997138,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Sig.",
                0.782515073,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region(2)",
                "Exp(B)",
                0.8567302641,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "B",
                1.023669313,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "S.E.",
                0.4670882442,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Wald",
                4.803098526,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Sig.",
                0.02840860037,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / sex(1)",
                "Exp(B)",
                2.783389177,
            ),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Wald",
                0.1051364944,
            ),
            ("Variables in the Equation", 0, "Step 1 / region", "df", 2.0),
            (
                "Variables in the Equation",
                0,
                "Step 1 / region",
                "Sig.",
                0.9487895666,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 0",
                "Predicted / 0",
                19.0,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 0",
                "Predicted / 1",
                4.0,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 0",
                "Predicted / Percentage Correct",
                82.60869565,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 1",
                "Predicted / 0",
                8.0,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 1",
                "Predicted / 1",
                3.0,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / 1",
                "Predicted / Percentage Correct",
                27.27272727,
            ),
            (
                "Classification Table",
                0,
                "Unselected Cases / Overall Percentage",
                "Predicted / Percentage Correct",
                64.70588235,
            ),
        ],
    );
}
