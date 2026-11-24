//! CROSSTABS cell statistics against R on a 3 x 3 table of 85 weighted cases: counts, expected
//! counts, row, column and total percentages, residuals, standardized and adjusted standardized
//! residuals (`chisq.test`: `expected`, `residuals`, `stdres`), and, in CTABLES, the Pearson
//! chi-square test of the table and the z-tests of column proportions with the Bonferroni
//! correction (pooled proportion, written out in R: row 1 and row 3 each have a significant
//! pair, row 2 none).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /r c w.
BEGIN DATA
1 1 12
1 2 7
1 3 3
2 1 9
2 2 15
2 3 6
3 1 4
3 2 10
3 3 19
END DATA.
WEIGHT BY w.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(s) => s,
        _ => "",
    }
}

#[test]
fn cell_counts_percentages_and_residuals_agree_with_r() {
    let t = run("CROSSTABS r BY c /CELLS=COUNT EXPECTED ROW COLUMN TOTAL RESID SRESID ASRESID.");
    let table = t.iter().find(|t| t.title == "r by c").expect("the table");
    let expected: [(&str, [[f64; 3]; 3]); 8] = [
        (
            "Count",
            [[12.0, 7.0, 3.0], [9.0, 15.0, 6.0], [4.0, 10.0, 19.0]],
        ),
        (
            "Expected",
            [
                [6.47058823529, 8.28235294118, 7.24705882353],
                [8.82352941176, 11.29411764706, 9.88235294118],
                [9.70588235294, 12.42352941176, 10.87058823529],
            ],
        ),
        (
            "Row %",
            [
                [54.5454545455, 31.8181818182, 13.6363636364],
                [30.0, 50.0, 20.0],
                [12.1212121212, 30.3030303030, 57.5757575758],
            ],
        ),
        (
            "Column %",
            [
                [48.0, 21.875, 10.7142857143],
                [36.0, 46.875, 21.4285714286],
                [16.0, 31.25, 67.8571428571],
            ],
        ),
        (
            "Total %",
            [
                [14.11764705882, 8.23529411765, 3.52941176471],
                [10.58823529412, 17.64705882353, 7.05882352941],
                [4.70588235294, 11.76470588235, 22.35294117647],
            ],
        ),
        (
            "Residual",
            [
                [5.529411764706, -1.28235294118, -4.24705882353],
                [0.176470588235, 3.70588235294, -3.88235294118],
                [-5.705882352941, -2.42352941176, 8.12941176471],
            ],
        ),
        (
            "Std. Residual",
            [
                [2.1737372632955, -0.445585146739, -1.57763808917],
                [0.0594088525786, 1.102720566172, -1.23499396113],
                [-1.8314931617197, -0.687584057412, 2.46565664564],
            ],
        ),
        (
            "Adjusted Residual",
            [
                [3.0052463345798, -0.655452797966, -2.23778627630],
                [0.0879049072992, 1.736061558279, -1.87484335185],
                [-2.7870639264691, -1.113281821164, 3.84957180635],
            ],
        ),
    ];
    let mut row = 0;
    for (label, values) in expected {
        let mut seen = 0;
        let mut current = String::new();
        for r in &table.rows {
            if !text(&r[0]).is_empty() {
                current = text(&r[0]).to_string();
            }
            if text(&r[1]) != label {
                continue;
            }
            if current == "Total" {
                continue;
            }
            let i: usize = current.parse::<usize>().expect("a row value") - 1;
            for j in 0..3 {
                let got = number(&r[2 + j]);
                assert!(
                    (got - values[i][j]).abs() <= 1e-9 * values[i][j].abs().max(1e-3),
                    "{label} {i} {j}: {got} vs {}",
                    values[i][j]
                );
            }
            seen += 1;
        }
        assert_eq!(seen, 3, "{label}");
        row += 1;
    }
    assert_eq!(row, 8);
}

#[test]
fn ctables_chi_square_and_column_proportion_tests_agree_with_r() {
    let t = run("CTABLES /TABLE r BY c [COUNT] /SIGTEST TYPE=CHISQUARE.");
    let chi = t
        .iter()
        .find(|t| t.title == "Pearson Chi-Square Tests")
        .expect("the table");
    near(number(&chi.rows[0][2]), 20.06395563, 1e-8, "chi-square");
    near(number(&chi.rows[0][3]), 4.0, 1e-12, "df");
    near(number(&chi.rows[0][4]), 0.0004851, 1e-3, "p");
    let t = run("CTABLES /TABLE r BY c [COLPCT.COUNT PCT40.1] \
                 /COMPARETEST TYPE=PROP ALPHA=0.05 ADJUST=BONFERRONI.");
    let letters = t
        .iter()
        .find(|t| t.title == "Comparisons of Column Proportions")
        .expect("the table");
    // R: the Bonferroni p of row 1, columns 1 and 3, is 0.0079 (column A is larger, so it shows
    // C); of row 3 columns 1 and 3 0.00043 and of columns 2 and 3 0.0139 (column C is larger,
    // so it shows A and B); no other pair is significant, the next smallest being 0.1136
    let got: Vec<Vec<&str>> = letters
        .rows
        .iter()
        .map(|r| r[2..].iter().map(text).collect())
        .collect();
    assert_eq!(
        got,
        [["C", "", ""], ["", "", ""], ["", "", "A B"]],
        "{got:?}"
    );
}

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs(),
        "{what}: {got} vs {want}"
    );
}
