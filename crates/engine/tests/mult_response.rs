//! MULT RESPONSE on synthetic data whose counts are worked out by hand in the comments.

use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Cell, Table, cell_text};

/// m1 to m3: a dichotomy group (1 = yes); c1 to c3: a category group of codes 1 to 4 (9 is out
/// of range); p1 p2 and a1 a2: two category groups to pair; gender 9 is out of its range.
const DATA: &str = "\
DATA LIST LIST /id gender r m1 m2 m3 c1 c2 c3 p1 p2 a1 a2 w.
BEGIN DATA
1 1 1 1 0 1 1 2 . 1 2 1 3 2
2 1 1 0 0 0 2 . . 2 . 2 . 1
3 2 2 1 1 . 3 1 9 1 1 3 3 1
4 2 2 0 1 1 . . . . 2 . 1 1
5 1 1 1 1 1 4 4 . . . . . 0.5
6 2 2 . 0 0 2 3 1 2 2 1 . 1
7 9 1 1 0 0 1 . . 1 . 2 2 1
8 1 2 0 0 0 . . . . . . . 1
END DATA.
VARIABLE LABELS m1 'Tea' m2 'Coffee'.
VALUE LABELS c1 1 'Red' 2 'Green' 3 'Blue' 4 'Black' / gender 1 'Man' 2 'Woman'.
";

fn run(source: &str) -> RunResult {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn tables(result: &RunResult) -> Vec<&Table> {
    result.commands.iter().flat_map(|c| &c.tables).collect()
}

/// The numbers of the row whose header cells read `headers`, in order.
fn numbers(table: &Table, headers: &[&str]) -> Vec<Option<f64>> {
    let row = table
        .rows
        .iter()
        .position(|row| {
            headers
                .iter()
                .enumerate()
                .all(|(i, h)| row[i] == Cell::Text(h.to_string()))
        })
        .unwrap_or_else(|| panic!("no row {headers:?} in {table:#?}"));
    table.rows[row][headers.len()..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => Some((n * 1e4).round() / 1e4),
            _ => None,
        })
        .collect()
}

fn s(values: &[f64]) -> Vec<Option<f64>> {
    values
        .iter()
        .map(|&v| Some((v * 1e4).round() / 1e4))
        .collect()
}

#[test]
fn frequencies_of_a_dichotomy_and_a_category_group() {
    let result = run(
        "MULT RESPONSE GROUPS=md 'Drinks' (m1 TO m3 (1)) mc (c1 c2 c3 (1,4))\n\
                      /FREQUENCIES=md mc.",
    );
    let t = tables(&result);
    assert_eq!(t.len(), 3);
    // md: valid cases 1, 3, 4, 5, 7; mc: valid 1, 2, 3, 5, 6, 7
    assert_eq!(
        numbers(t[0], &["Drinks"]),
        s(&[5.0, 62.5, 3.0, 37.5, 8.0, 100.0])
    );
    assert_eq!(
        numbers(t[0], &["mc"]),
        s(&[6.0, 75.0, 2.0, 25.0, 8.0, 100.0])
    );
    // m1 in cases 1, 3, 5, 7; m2 in 3, 4, 5; m3 in 1, 4, 5
    assert_eq!(t[1].title, "Drinks Frequencies");
    assert_eq!(t[1].columns[0], "Drinks");
    assert_eq!(numbers(t[1], &["Tea"]), s(&[4.0, 40.0, 80.0]));
    assert_eq!(numbers(t[1], &["Coffee"]), s(&[3.0, 30.0, 60.0]));
    assert_eq!(numbers(t[1], &["m3"]), s(&[3.0, 30.0, 60.0]));
    assert_eq!(numbers(t[1], &["Total"]), s(&[10.0, 100.0, 200.0]));
    assert!(t[1].caption.as_ref().unwrap().contains("value 1"));
    // 1 four times, 2 three, 3 twice, 4 twice (case 5 gives it twice)
    assert_eq!(
        numbers(t[2], &["Red"]),
        s(&[4.0, 400.0 / 11.0, 400.0 / 6.0])
    );
    assert_eq!(
        numbers(t[2], &["Black"]),
        s(&[2.0, 200.0 / 11.0, 200.0 / 6.0])
    );
    assert_eq!(numbers(t[2], &["Total"]), s(&[11.0, 100.0, 1100.0 / 6.0]));
}

#[test]
fn listwise_missing_nolabels_and_single_variables() {
    let result = run(
        "MUL RES GRO=md (m1 m2 m3 (1)) mc (c1 TO c3 (1 4)) /VAR=gender (1,2)\n\
         /FRE=md TO mc gender /MIS=MDGROUP MRGROUP /FORMAT=NOLABELS DOUBLE.",
    );
    let t = tables(&result);
    // MDGROUP: case 3 (m3 missing) drops out; MRGROUP: only case 6 has three valid codes
    assert_eq!(
        numbers(t[0], &["md"]),
        s(&[4.0, 50.0, 4.0, 50.0, 8.0, 100.0])
    );
    assert_eq!(
        numbers(t[0], &["mc"]),
        s(&[1.0, 12.5, 7.0, 87.5, 8.0, 100.0])
    );
    // gender 9 is out of range
    assert_eq!(
        numbers(t[0], &["gender"]),
        s(&[7.0, 87.5, 1.0, 12.5, 8.0, 100.0])
    );
    assert_eq!(numbers(t[1], &["Tea"]), s(&[3.0, 37.5, 75.0]));
    // NOLABELS shows values; dichotomy categories keep their variable labels
    assert_eq!(numbers(t[2], &["1"]), s(&[1.0, 100.0 / 3.0, 100.0]));
    assert_eq!(numbers(t[3], &["1"]), s(&[4.0, 400.0 / 7.0, 400.0 / 7.0]));
    assert_eq!(
        result.commands[0].warnings,
        ["FORMAT=DOUBLE only changes the printed layout; Statscraft ignores it"]
    );
}

#[test]
fn crosstabs_based_on_cases_and_on_responses() {
    let source = "MULT RESPONSE GROUPS=md (m1 m2 m3 (1)) /VARIABLES=gender (1,2)\n\
                  /TABLES=md BY gender /CELLS=ALL";
    let result = run(&format!("{source}."));
    let t = tables(&result);
    // valid: cases 1 and 5 (men), 3 and 4 (women); case 7 has gender 9
    assert_eq!(
        numbers(t[0], &["md by gender"]),
        s(&[4.0, 50.0, 4.0, 50.0, 8.0, 100.0])
    );
    let x = t[1];
    assert_eq!(x.columns[2..], ["Man", "Woman", "Total"]);
    assert_eq!(numbers(x, &["Tea", "Count"]), s(&[2.0, 1.0, 3.0]));
    // totals are cases: Tea 3 cases, 2 men, 4 cases in all
    assert_eq!(
        numbers(x, &["Tea", "Row %"]),
        s(&[200.0 / 3.0, 100.0 / 3.0, 100.0])
    );
    assert_eq!(numbers(x, &["Tea", "Column %"]), s(&[100.0, 50.0, 75.0]));
    assert_eq!(numbers(x, &["Tea", "Total %"]), s(&[50.0, 25.0, 75.0]));
    assert_eq!(numbers(x, &["Coffee", "Count"]), s(&[1.0, 2.0, 3.0]));
    assert_eq!(numbers(x, &["Total", "Count"]), s(&[2.0, 2.0, 4.0]));
    assert_eq!(
        numbers(x, &["Total", "Column %"]),
        s(&[100.0, 100.0, 100.0])
    );
    assert!(x.caption.as_ref().unwrap().contains("based on cases"));

    let result = run(&format!("{source} /BASE=RESPONSES."));
    let x = tables(&result)[1];
    // totals are the sums of the cells: men 5 responses, women 4
    assert_eq!(
        numbers(x, &["Tea", "Column %"]),
        s(&[40.0, 25.0, 100.0 / 3.0])
    );
    assert_eq!(numbers(x, &["Total", "Count"]), s(&[5.0, 4.0, 9.0]));
    assert!(x.caption.as_ref().unwrap().contains("based on responses"));
}

#[test]
fn control_variables_give_layers() {
    let result = run(
        "MULT RESPONSE GROUPS=md (m1 m2 m3 (1)) /VARIABLES=gender r (1,2)\n\
         /TABLES=md BY gender BY r.",
    );
    let x = tables(&result)[1];
    assert_eq!(x.title, "md by gender by r");
    // layer r = 1: cases 1 and 5, both men; r = 2: cases 3 and 4, both women
    assert_eq!(
        numbers(x, &["1", "Tea", "Count"]),
        vec![Some(2.0), None, Some(2.0)]
    );
    assert_eq!(
        numbers(x, &["2", "Coffee", "Count"]),
        vec![None, Some(2.0), Some(2.0)]
    );
    assert_eq!(
        numbers(x, &["2", "Total", "Count"]),
        vec![None, Some(2.0), Some(2.0)]
    );
}

#[test]
fn paired_tables_pair_the_variables_of_category_groups() {
    let groups = "MULT RESPONSE GROUPS=ps (p1 p2 (1,2)) pa (a1 a2 (1,3))";
    let result = run(&format!(
        "{groups} /TABLES=ps BY pa (PAIRED) /CELLS=COLUMN."
    ));
    let t = tables(&result);
    // pairs: case 1 (1,1) (2,3); 2 (2,2); 3 (1,3) (1,3); 4 (2,1); 6 (2,1); 7 (1,2)
    assert_eq!(
        numbers(t[0], &["ps by pa"]),
        s(&[6.0, 75.0, 2.0, 25.0, 8.0, 100.0])
    );
    let x = t[1];
    assert_eq!(numbers(x, &["1", "Count"]), s(&[1.0, 1.0, 2.0, 4.0]));
    assert_eq!(numbers(x, &["2", "Count"]), s(&[2.0, 1.0, 1.0, 4.0]));
    assert_eq!(
        numbers(x, &["1", "Column %"]),
        s(&[100.0 / 3.0, 50.0, 200.0 / 3.0, 50.0])
    );
    assert_eq!(numbers(x, &["Total", "Count"]), s(&[3.0, 2.0, 3.0, 8.0]));
    assert!(x.caption.as_ref().unwrap().contains("Paired"));
    // unpaired, every response of a case meets every other
    let x = tables(&run(&format!("{groups} /TABLES=ps BY pa.")))[1].clone();
    // the totals are cases: ps = 1 is given by case 1 once, case 3 twice and case 7 once
    assert_eq!(numbers(&x, &["1", "Count"]), s(&[1.0, 2.0, 5.0, 4.0]));
    assert_eq!(numbers(&x, &["2", "Count"]), s(&[4.0, 1.0, 1.0, 5.0]));
}

#[test]
fn weights_count_as_they_are() {
    let result = run("WEIGHT BY w.\nMULT RESPONSE GROUPS=md (m1 m2 m3 (1)) /FREQUENCIES=md.");
    let t = tables(&result);
    // valid: 2 + 1 + 1 + 0.5 + 1; Tea: 2 + 1 + 0.5 + 1
    assert_eq!(
        numbers(t[0], &["md"])[..4],
        s(&[5.5, 5.5 / 8.5 * 100.0, 3.0, 3.0 / 8.5 * 100.0])
    );
    assert_eq!(
        numbers(t[1], &["Tea"]),
        s(&[4.5, 4.5 / 10.5 * 100.0, 4.5 / 5.5 * 100.0])
    );
    assert_eq!(cell_text(t[1], 3, 1), "10.50");
}

#[test]
fn errors_name_what_is_wrong() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    let mut error = |source: &str| {
        let errors = session.run(source, None).unwrap_err();
        errors[0].text()
    };
    assert!(error("MULT RESPONSE GROUPS=g (m1 m2 (1)) /FREQUENCIES=gender.").contains("neither"));
    assert!(error("MULT RESPONSE GROUPS=m1 (m1 m2 (1)) /FREQUENCIES=m1.").contains("group name"));
    assert!(error("MULT RESPONSE FREQUENCIES=x /GROUPS=g (m1 (1)).").contains("before"));
    assert!(error("MULT RESPONSE GROUPS=g (m1 m2 (1)).").contains("FREQUENCIES"));
    assert!(
        error("MULT RESPONSE GROUPS=g (c1 c2 (1,4)) h (p1 p2 (1,2)) /TABLES=g BY h (PAIRED) /BASE=CASES.")
            .contains("PAIRED")
    );
    assert!(
        error(
            "MULT RESPONSE GROUPS=g (m1 m2 (1)) /VARIABLES=gender (1,2) /FREQUENCIES=g TO gender."
        )
        .contains("joins a group and a variable")
    );
    assert!(error("MULT RESPONSE GROUPS=g (c1 c2 (4,1)) /FREQUENCIES=g.").contains("backwards"));
}

#[test]
fn the_manual_layout_with_unquoted_labels_and_a_slash_between_groups() {
    let result = run(
        "MULT RESPONSE GROUPS=ps CHILD SEX (p1 p2 (1,2))\n /pa 'AGE' (a1 a2 (1,3))\n\
         /TABLES=ps BY pa (PAIRED).",
    );
    let x = tables(&result)[1];
    assert_eq!(x.columns[0], "CHILD SEX");
    assert_eq!(numbers(x, &["Total", "Count"]), s(&[3.0, 2.0, 3.0, 8.0]));
}
