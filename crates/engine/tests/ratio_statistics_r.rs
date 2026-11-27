//! RATIO STATISTICS against R on 64 invented cases (appraised value over price, by town with
//! missing values, frequency weights with halves and fractions that round, a zero and a missing
//! weight, a user-missing numerator, a zero denominator and a negative numerator): every
//! statistic of PRINT (mean, median, weighted mean with their intervals and the median's actual
//! coverage, minimum, maximum, range, standard deviation, AAD, PRD, COD, both coefficients of
//! variation, the coefficients of concentration) for each group and overall, at levels 95 and 90,
//! with MISSING=INCLUDE and without weights. R follows the formulas of the algorithms reference
//! on the ratios written out as often as their rounded weight. Scripts in lab/oracles/ratio-r.
//!
//! Choices where the manual is silent, as in the engine: MDCOV is over F - 1; the weighted mean's
//! interval takes frequency weights as repeated cases (the variances of the means over F, which
//! is the manual's sum f^2 / F^2 only for unit weights), so with weights R uses that variance and
//! without weights the manual's own.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /appraise price town w (4F8.2).
BEGIN DATA.
170 211 2 3
210 244 3 0.5
106 156 1 2
296 317 3 1
279 . 3 3
256 316 3 2.4
205 248 1 1
100 142 . 1
89 138 3 1
182 209 3 0.5
80 120 1 2.4
. 363 2 2
283 343 1 3
78 108 2 1
162 157 2 0
146 152 1 0.5
303 300 2 2.4
360 303 1 3
193 331 3 1
269 295 3 0.5
229 290 1 1
999 368 3 2.4
303 263 1 2
176 177 1 1
346 301 1 .
334 331 1 0.5
103 106 1 2.4
412 289 1 1
224 222 2 1
125 106 1 2
337 257 3 0.5
136 92 2 3
128 0 2 2
109 151 3 2.4
112 127 3 1
276 298 2 2
222 270 2 3
104 168 1 2
232 239 1 1
446 . 1 1
284 267 3 2
140 135 3 1
323 354 2 1
127 193 . 2.4
313 389 3 2
294 240 2 2.4
125 126 2 2.4
101 143 2 2
165 175 1 1
-5 90 1 2.4
168 156 1 0.5
219 218 2 0.5
79 144 2 2.4
390 343 1 0.5
191 178 1 0.5
243 226 3 2
256 296 3 0.5
95 127 1 2
389 334 2 1
212 223 2 0.5
107 108 1 0.5
113 129 1 1
108 132 2 1
269 363 2 2.4
END DATA.
MISSING VALUES appraise (999).
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

/// The row headers joined with " / ", numbers as they print.
fn row_keys(table: &Table) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| match &row[0] {
            Cell::Text(t) => t.clone(),
            Cell::Number(n) => format!("{n}"),
            Cell::Empty => String::new(),
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value.
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let t = tables
            .iter()
            .find(|t| t.title == *title)
            .unwrap_or_else(|| panic!("no table {title}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        if want.is_nan() {
            // a group too small for the interval's level leaves the cell empty
            assert_eq!(t.rows[r][c], Cell::Empty, "{title} / {row} / {column}");
            continue;
        }
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-8 + 1e-7 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn weighted_by_group_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nRATIO STATISTICS appraise WITH price BY town /PRINT=MEAN MEDIAN WGTMEAN CIN(95) MIN MAX RANGE STDDEV AAD PRD COD MNCOV MDCOV BCOC(0.8,1.2) BCOC(0.9,1.1) WCOC(10 20).",
    );
    check(
        &t,
        &[
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Mean",
                0.9211082282,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8750801903,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Upper Bound",
                0.9671362661,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Median",
                0.8968770853,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Lower Bound",
                0.8222222222,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Upper Bound",
                0.9716981132,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Actual Coverage",
                96.01463714,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Weighted Mean",
                0.9235151387,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8804183537,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.9666119238,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Maximum",
                1.47826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Range",
                0.9296497585,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Std. Deviation",
                0.2146825697,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Average Absolute Deviation",
                0.1720455785,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Price Related Differential",
                0.9973937509,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Dispersion",
                19.18273767,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Mean Centered",
                23.30698642,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Median Centered",
                24.09044801,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.8 to 1.2",
                65.11627907,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.9 to 1.1",
                31.39534884,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 10% of Median",
                30.23255814,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 20% of Median",
                62.79069767,
            ),
            ("Case Processing Summary", "Overall", "Count", 86.0),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Mean",
                0.94379349,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8707617581,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Mean / Upper Bound",
                1.016825222,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Median",
                0.9707112971,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Lower Bound",
                0.8250728863,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Upper Bound",
                1.073033708,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Actual Coverage",
                96.49179666,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Weighted Mean",
                0.9684441198,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8901736576,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                1.046714582,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Minimum",
                0.619047619,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Maximum",
                1.425605536,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Range",
                0.8065579173,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Std. Deviation",
                0.2059642061,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Average Absolute Deviation",
                0.1690867775,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Price Related Differential",
                0.9745461516,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Dispersion",
                17.41885337,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Variation, Mean Centered",
                21.82301618,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Variation, Median Centered",
                21.40391514,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, 0.8 to 1.2",
                69.6969697,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, 0.9 to 1.1",
                30.3030303,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, Within 10% of Median",
                27.27272727,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, Within 20% of Median",
                57.57575758,
            ),
            ("Case Processing Summary", "1", "Count", 33.0),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Mean",
                0.939955435,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8479583398,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Mean / Upper Bound",
                1.03195253,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Median",
                0.9193019376,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Lower Bound",
                0.8056872038,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Upper Bound",
                1.004587156,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Actual Coverage",
                95.72260547,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Weighted Mean",
                0.9224533496,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8525525539,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.9923541454,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Maximum",
                1.47826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Range",
                0.9296497585,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Std. Deviation",
                0.246372746,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Average Absolute Deviation",
                0.1847244353,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Price Related Differential",
                1.01897341,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Dispersion",
                20.09398956,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Variation, Mean Centered",
                26.21110925,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Variation, Median Centered",
                26.89722059,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, 0.8 to 1.2",
                60.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, 0.9 to 1.1",
                33.33333333,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, Within 10% of Median",
                33.33333333,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, Within 20% of Median",
                63.33333333,
            ),
            ("Case Processing Summary", "2", "Count", 30.0),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Mean",
                0.8925585939,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8114700291,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Mean / Upper Bound",
                0.9736471588,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Median",
                0.867839131,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Lower Bound",
                0.8046272494,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Upper Bound",
                1.037037037,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Actual Coverage",
                95.86105347,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Weighted Mean",
                0.8904299584,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8091161734,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.9717437433,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Minimum",
                0.583081571,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Maximum",
                1.311284047,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Range",
                0.7282024757,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Std. Deviation",
                0.1732608308,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Average Absolute Deviation",
                0.1298839957,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Price Related Differential",
                1.00239057,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Dispersion",
                14.96636774,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Variation, Mean Centered",
                19.41170383,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Variation, Median Centered",
                20.17737892,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, 0.8 to 1.2",
                75.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, 0.9 to 1.1",
                35.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, Within 10% of Median",
                50.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, Within 20% of Median",
                65.0,
            ),
            ("Case Processing Summary", "3", "Count", 20.0),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Mean",
                0.6734291761,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Mean / Lower Bound",
                0.6071765507,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Mean / Upper Bound",
                0.7396818015,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Median",
                0.6580310881,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Lower Bound",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Upper Bound",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Actual Coverage",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Weighted Mean",
                0.6704545455,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.6118375693,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.7290715216,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Minimum",
                0.6580310881,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Maximum",
                0.7042253521,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Range",
                0.04619426403,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Std. Deviation",
                0.02667027077,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Average Absolute Deviation",
                0.01539808801,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Price Related Differential",
                1.004436737,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Dispersion",
                2.340024398,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Variation, Mean Centered",
                3.960367581,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Variation, Median Centered",
                4.963941361,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, 0.8 to 1.2",
                0.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, 0.9 to 1.1",
                0.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, Within 10% of Median",
                100.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, Within 20% of Median",
                100.0,
            ),
            ("Case Processing Summary", ".", "Count", 3.0),
        ],
    );
}

#[test]
fn weighted_level_90_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nRATIO STATISTICS appraise WITH price BY town /PRINT=MEAN MEDIAN WGTMEAN CIN(90) MIN MAX RANGE STDDEV AAD PRD COD MNCOV MDCOV BCOC(0.8,1.2) BCOC(0.9,1.1) WCOC(10 20).",
    );
    check(
        &t,
        &[
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Mean",
                0.9211082282,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Mean / Lower Bound",
                0.8826105805,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Mean / Upper Bound",
                0.9596058759,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Median",
                0.8968770853,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Median / Lower Bound",
                0.8250728863,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Median / Upper Bound",
                0.9707112971,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Median / Actual Coverage",
                93.38474025,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Weighted Mean",
                0.9235151387,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Weighted Mean / Lower Bound",
                0.887469178,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "90% Confidence Interval for Weighted Mean / Upper Bound",
                0.9595610995,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Maximum",
                1.47826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Range",
                0.9296497585,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Std. Deviation",
                0.2146825697,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Average Absolute Deviation",
                0.1720455785,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Price Related Differential",
                0.9973937509,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Dispersion",
                19.18273767,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Mean Centered",
                23.30698642,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Median Centered",
                24.09044801,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.8 to 1.2",
                65.11627907,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.9 to 1.1",
                31.39534884,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 10% of Median",
                30.23255814,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 20% of Median",
                62.79069767,
            ),
            ("Case Processing Summary", "Overall", "Count", 86.0),
        ],
    );
}

#[test]
fn unweighted_by_group_agree_with_r() {
    let t = run(
        "RATIO STATISTICS appraise WITH price BY town /PRINT=MEAN MEDIAN WGTMEAN CIN(95) MIN MAX RANGE STDDEV AAD PRD COD MNCOV MDCOV BCOC(0.8,1.2) BCOC(0.9,1.1) WCOC(10 20).",
    );
    check(
        &t,
        &[
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Mean",
                0.9286024881,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Lower Bound",
                0.875989146,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Upper Bound",
                0.9812158303,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Median",
                0.9299642199,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Lower Bound",
                0.8606557377,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Upper Bound",
                0.9920634921,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Actual Coverage",
                95.20596759,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Weighted Mean",
                0.9376174076,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.884262324,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.9909724913,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Maximum",
                1.47826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Range",
                0.9296497585,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Std. Deviation",
                0.2000989701,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Average Absolute Deviation",
                0.1578999446,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Price Related Differential",
                0.9903852899,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Dispersion",
                16.97914191,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Mean Centered",
                21.54839909,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Median Centered",
                21.51735309,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.8 to 1.2",
                68.96551724,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.9 to 1.1",
                37.93103448,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 10% of Median",
                36.20689655,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 20% of Median",
                62.06896552,
            ),
            ("Case Processing Summary", "Overall", "Count", 58.0),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Mean",
                0.9674798184,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8825168644,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Mean / Upper Bound",
                1.052442772,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Median",
                0.9716981132,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Lower Bound",
                0.8266129032,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Upper Bound",
                1.076923077,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Median / Actual Coverage",
                96.53103352,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Weighted Mean",
                0.9927204659,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.9001629573,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                1.085277975,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Minimum",
                0.619047619,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Maximum",
                1.425605536,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Range",
                0.8065579173,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Std. Deviation",
                0.1964768332,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Average Absolute Deviation",
                0.1509157551,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Price Related Differential",
                0.974574265,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Dispersion",
                15.53113596,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Variation, Mean Centered",
                20.30810664,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Variation, Median Centered",
                20.22481735,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, 0.8 to 1.2",
                73.91304348,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, 0.9 to 1.1",
                39.13043478,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, Within 10% of Median",
                34.7826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "1",
                "Coefficient of Concentration, Within 20% of Median",
                69.56521739,
            ),
            ("Case Processing Summary", "1", "Count", 23.0),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Mean",
                0.9371655531,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8300401207,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Mean / Upper Bound",
                1.044290985,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Median",
                0.9384235712,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Lower Bound",
                0.8056872038,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Upper Bound",
                1.01,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Median / Actual Coverage",
                96.91162109,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Weighted Mean",
                0.9377382465,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8485346025,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                1.026941891,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Maximum",
                1.47826087,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Range",
                0.9296497585,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Std. Deviation",
                0.2154191687,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Average Absolute Deviation",
                0.1590689985,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Price Related Differential",
                0.9993892822,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Dispersion",
                16.95066103,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Variation, Mean Centered",
                22.98624486,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Variation, Median Centered",
                22.95584475,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, 0.8 to 1.2",
                66.66666667,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, 0.9 to 1.1",
                44.44444444,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, Within 10% of Median",
                44.44444444,
            ),
            (
                "Ratio Statistics for appraise / price",
                "2",
                "Coefficient of Concentration, Within 20% of Median",
                61.11111111,
            ),
            ("Case Processing Summary", "2", "Count", 18.0),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Mean",
                0.8917114728,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Mean / Lower Bound",
                0.7908897217,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Mean / Upper Bound",
                0.9925332239,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Median",
                0.8708133971,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Lower Bound",
                0.8046272494,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Upper Bound",
                1.037037037,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Median / Actual Coverage",
                96.484375,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Weighted Mean",
                0.8893996755,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.785045902,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.993753449,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Minimum",
                0.583081571,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Maximum",
                1.311284047,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Range",
                0.7282024757,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Std. Deviation",
                0.1820605122,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Average Absolute Deviation",
                0.1283055335,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Price Related Differential",
                1.002599278,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Dispersion",
                14.73398709,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Variation, Mean Centered",
                20.41697542,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Variation, Median Centered",
                21.05400409,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, 0.8 to 1.2",
                73.33333333,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, 0.9 to 1.1",
                33.33333333,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, Within 10% of Median",
                53.33333333,
            ),
            (
                "Ratio Statistics for appraise / price",
                "3",
                "Coefficient of Concentration, Within 20% of Median",
                66.66666667,
            ),
            ("Case Processing Summary", "3", "Count", 15.0),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Mean",
                0.6811282201,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Mean / Lower Bound",
                0.3876513319,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Mean / Upper Bound",
                0.9746051083,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Median",
                0.6811282201,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Lower Bound",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Upper Bound",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Median / Actual Coverage",
                f64::NAN,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Weighted Mean",
                0.6776119403,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.3909368644,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                0.9642870162,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Minimum",
                0.6580310881,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Maximum",
                0.7042253521,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Range",
                0.04619426403,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Std. Deviation",
                0.03266427735,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Average Absolute Deviation",
                0.02309713201,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Price Related Differential",
                1.005189223,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Dispersion",
                3.391010875,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Variation, Mean Centered",
                4.795613569,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Variation, Median Centered",
                4.795613569,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, 0.8 to 1.2",
                0.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, 0.9 to 1.1",
                0.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, Within 10% of Median",
                100.0,
            ),
            (
                "Ratio Statistics for appraise / price",
                ".",
                "Coefficient of Concentration, Within 20% of Median",
                100.0,
            ),
            ("Case Processing Summary", ".", "Count", 2.0),
        ],
    );
}

#[test]
fn weighted_include_user_missing_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nRATIO STATISTICS appraise WITH price BY town /PRINT=MEAN MEDIAN WGTMEAN CIN(95) MIN MAX RANGE STDDEV AAD PRD COD MNCOV MDCOV BCOC(0.8,1.2) BCOC(0.9,1.1) WCOC(10 20) /MISSING=INCLUDE.",
    );
    check(
        &t,
        &[
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Mean",
                0.9618710847,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Lower Bound",
                0.8893043246,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Mean / Upper Bound",
                1.034437845,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Median",
                0.9121468927,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Lower Bound",
                0.8250728863,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Upper Bound",
                0.9716981132,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Median / Actual Coverage",
                95.77893884,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Weighted Mean",
                0.9904539454,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Lower Bound",
                0.8893620773,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "95% Confidence Interval for Weighted Mean / Upper Bound",
                1.091545813,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Minimum",
                0.5486111111,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Maximum",
                2.714673913,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Range",
                2.166062802,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Std. Deviation",
                0.3424902968,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Average Absolute Deviation",
                0.2091083951,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Price Related Differential",
                0.971141656,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Dispersion",
                22.92485966,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Mean Centered",
                35.60667352,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Variation, Median Centered",
                37.94587444,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.8 to 1.2",
                63.63636364,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, 0.9 to 1.1",
                30.68181818,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 10% of Median",
                30.68181818,
            ),
            (
                "Ratio Statistics for appraise / price",
                "Overall",
                "Coefficient of Concentration, Within 20% of Median",
                59.09090909,
            ),
            ("Case Processing Summary", "Overall", "Count", 88.0),
        ],
    );
}
