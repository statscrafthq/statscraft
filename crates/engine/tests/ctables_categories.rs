//! CTABLES explicit category lists: subtotals, OTHERNM and MISSING, and sorting by a summary.
//! The expected numbers are counted here from the raw values of the fixtures, or come from a
//! second table that must agree by construction.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn try_run(file: &str, source: &str) -> Result<Vec<Table>, String> {
    let mut engine = Engine::default();
    let source = format!("GET FILE='{}'.\n{source}", fixture(file));
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for c in &commands {
        tables.extend(engine.execute(&c.command).map_err(|e| e.to_string())?);
    }
    Ok(tables)
}

fn run_all(file: &str, source: &str) -> Vec<Table> {
    try_run(file, source).unwrap_or_else(|e| panic!("{e}\n{source}"))
}

fn run(file: &str, source: &str) -> Table {
    run_all(file, source).remove(0)
}

fn error(file: &str, source: &str) -> String {
    match try_run(file, source) {
        Ok(_) => panic!("accepted:\n{source}"),
        Err(e) => e,
    }
}

/// The numbers of one row, rounded to 6 decimals; None for an empty cell.
fn numbers(t: &Table, row: usize) -> Vec<Option<f64>> {
    t.rows[row][t.row_headers..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => Some((n * 1e6).round() / 1e6),
            Cell::Empty => None,
            other => panic!("not a number: {other:?}"),
        })
        .collect()
}

fn all_numbers(t: &Table) -> Vec<Vec<Option<f64>>> {
    (0..t.rows.len()).map(|r| numbers(t, r)).collect()
}

/// The label of the innermost row header of a row.
fn row_label(t: &Table, row: usize) -> String {
    match &t.rows[row][t.row_headers - 1] {
        Cell::Text(text) => text.clone(),
        other => panic!("not a label: {other:?}"),
    }
}

/// The cells of a table as text, for comparing test tables whose header texts differ.
fn cells(t: &Table) -> Vec<Vec<String>> {
    t.rows
        .iter()
        .map(|row| {
            row[t.row_headers..]
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => format!("{n:.10}"),
                    Cell::Text(s) => s.clone(),
                    Cell::Empty => String::new(),
                })
                .collect()
        })
        .collect()
}

/// The raw values of a numeric variable; None for system-missing, user-missing codes kept.
fn raw(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn pct(part: f64, whole: f64) -> Option<f64> {
    Some((100.0 * part / whole * 1e6).round() / 1e6)
}

// Subtotals (Command Syntax Reference, CTABLES, CATEGORIES subcommand, "Explicit Category
// Specification" and "Totals", pages 494-496; Custom Tables manual, "Subtotals" and "What You See
// Is What Gets Subtotaled", pages 31-32): SUBTOTAL in an explicit list sums the categories before
// it back to the previous subtotal, or with POSITION=BEFORE the ones after it up to the next one.
// A subtotal counts a case once, takes summaries like a total, and a category left out of the list
// is left out of its subtotal too.
#[test]
fn a_subtotal_counts_the_cases_of_its_categories() {
    let data = Dataset::read_sav(fixture("survey_basic.sav")).unwrap();
    let (q1, gender) = (raw(&data, "q1_satisfaction"), raw(&data, "gender"));
    let n = data.case_count();
    let count = |f: &dyn Fn(usize) -> bool| (0..n).filter(|&i| f(i)).count() as f64;
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT, ROWPCT, TABLEPCT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High']\n\
         TOTAL=YES.",
    );
    let labels: Vec<String> = (0..t.rows.len()).map(|r| row_label(&t, r)).collect();
    assert_eq!(labels[2], "Low");
    assert_eq!(labels[6], "High");
    assert_eq!(labels[7], "Total");
    let shown = |i: usize| q1[i].is_some_and(|v| (1.0..=5.0).contains(&v));
    let all = count(&shown);
    for (row, low, high) in [(2, 1.0, 2.0), (6, 3.0, 5.0)] {
        let inside = |i: usize| q1[i].is_some_and(|v| v >= low && v <= high);
        for g in 0..3 {
            let is_g = |i: usize| gender[i] == Some(g as f64 + 1.0);
            let cell = count(&|i| inside(i) && is_g(i));
            let expected = [
                Some(cell),
                pct(cell, count(&|i| shown(i) && is_g(i))),
                pct(cell, count(&inside)),
                pct(cell, all),
            ];
            assert_eq!(numbers(&t, row)[g * 4..g * 4 + 4], expected, "{row} {g}");
        }
    }
    // the total counts every case once, not the subtotals on top of their categories
    for g in 0..3 {
        let is_g = |i: usize| gender[i] == Some(g as f64 + 1.0);
        assert_eq!(numbers(&t, 7)[g * 4], Some(count(&|i| shown(i) && is_g(i))));
    }

    // POSITION=BEFORE: a subtotal sums the categories after it, and the total comes first
    let before = run(
        "survey_basic.sav",
        "CTABLES /TABLE q1_satisfaction BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [SUBTOTAL='Low', 1, 2, SUBTOTAL='High', 3, 4, 5]\n\
         TOTAL=YES POSITION=BEFORE.",
    );
    let labels: Vec<String> = (0..before.rows.len())
        .map(|r| row_label(&before, r))
        .collect();
    assert_eq!(labels[..2], ["Total", "Low"]);
    assert_eq!(
        numbers(&before, 1),
        numbers(&t, 2).into_iter().step_by(4).collect::<Vec<_>>()
    );
    assert_eq!(
        numbers(&before, 4),
        numbers(&t, 6).into_iter().step_by(4).collect::<Vec<_>>()
    );

    // a category left out of the list is left out of its subtotal
    let partial = run(
        "survey_basic.sav",
        "CTABLES /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [2, SUBTOTAL, 5].",
    );
    assert_eq!(row_label(&partial, 1), "Subtotal");
    assert_eq!(numbers(&partial, 1), [Some(count(&|i| q1[i] == Some(2.0)))]);
    // a category after the last subtotal belongs to none
    assert_eq!(partial.rows.len(), 3);
}

// HSUBTOTAL (same pages): only the subtotal shows, so the table is the plain table of the
// categories merged, also in its percentages and tests. The tests take the subtotal in place of
// the categories it hides; with CATEGORIES=SUBTOTALS every subtotal does (SIGTEST and
// COMPARETEST, pages 499-501).
#[test]
fn an_hsubtotal_table_is_the_table_of_the_merged_categories() {
    let merged = "RECODE q1_satisfaction (1, 2 = 1) (3 THRU 5 = 2) INTO q1g.\n\
                  VALUE LABELS q1g 1 'Low' 2 'High'.\nVARIABLE LEVEL q1g (ORDINAL).\n";
    let tests = "/SIGTEST TYPE=CHISQUARE /COMPARETEST TYPE=PROP ALPHA=0.3 0.6 ADJUST=NONE";
    let plain = run_all(
        "survey_basic.sav",
        &format!(
            "{merged}CTABLES /TABLE q1g [COUNT, COLPCT, ROWPCT, TABLEPCT, COLPCT.TOTALN] BY region\n\
             /CATEGORIES VARIABLES=q1g TOTAL=YES {tests}."
        ),
    );
    let hidden = run_all(
        "survey_basic.sav",
        &format!(
            "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT, ROWPCT, TABLEPCT, COLPCT.TOTALN] BY region\n\
             /CATEGORIES VARIABLES=q1_satisfaction [1, 2, HSUBTOTAL='Low', 3, 4, 5, HSUBTOTAL='High']\n\
             TOTAL=YES {tests}."
        ),
    );
    assert_eq!(all_numbers(&hidden[0]), all_numbers(&plain[0]));
    assert_eq!(row_label(&hidden[0], 0), "Low");
    for k in 1..3 {
        assert_eq!(cells(&hidden[k]), cells(&plain[k]), "test table {k}");
    }
    // the significant differences are not all empty, so the comparison checks something
    assert!(cells(&plain[2]).iter().flatten().any(|c| !c.is_empty()));

    // shown subtotals with CATEGORIES=SUBTOTALS: the tests are those of the merged table
    let subtotals = "/SIGTEST TYPE=CHISQUARE CATEGORIES=SUBTOTALS\n\
                     /COMPARETEST TYPE=PROP ALPHA=0.3 0.6 ADJUST=NONE CATEGORIES=SUBTOTALS";
    let shown = run_all(
        "survey_basic.sav",
        &format!(
            "CTABLES /TABLE q1_satisfaction BY region\n\
             /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High']\n\
             {subtotals}."
        ),
    );
    assert_eq!(cells(&shown[1]), cells(&plain[1]));
    assert_eq!(cells(&shown[2]), cells(&plain[2]));
    // without it the shown categories are tested, as in the table without subtotals
    let categories = run_all(
        "survey_basic.sav",
        &format!(
            "CTABLES /TABLE q1_satisfaction BY region\n\
             /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High'] {tests}."
        ),
    );
    let without = run_all(
        "survey_basic.sav",
        &format!("CTABLES /TABLE q1_satisfaction BY region {tests}."),
    );
    assert_eq!(cells(&categories[1]), cells(&without[1]));
    assert_eq!(cells(&categories[2]), cells(&without[2]));

    // subtotals in the columns, and a scale variable summarized in them: the means test runs on
    // the subtotals that hide their categories
    let columns =
        "RECODE region (1, 2 = 1) (3 THRU 5 = 2) INTO rg.\nVARIABLE LEVEL rg (NOMINAL).\n";
    let plain = run_all(
        "survey_basic.sav",
        &format!(
            "{columns}CTABLES /TABLE gender > age [MEAN, COUNT] BY rg\n\
             /COMPARETEST TYPE=MEAN ALPHA=0.3 ADJUST=NONE."
        ),
    );
    let hidden = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender > age [MEAN, COUNT] BY region\n\
         /CATEGORIES VARIABLES=region [1, 2, HSUBTOTAL='1', 3, 4, 5, HSUBTOTAL='2']\n\
         /COMPARETEST TYPE=MEAN ALPHA=0.3 ADJUST=NONE.",
    );
    assert_eq!(all_numbers(&hidden[0]), all_numbers(&plain[0]));
    assert_eq!(cells(&hidden[1]), cells(&plain[1]));
}

// Subtotals of a multiple response set count a case once, however many of their categories it
// has, like the set's total.
#[test]
fn a_subtotal_of_a_set_counts_each_case_once() {
    let data = Dataset::read_sav(fixture("survey_basic_phpwriter.sav")).unwrap();
    let mentions: Vec<Vec<Option<f64>>> = (1..=3)
        .map(|k| raw(&data, &format!("q6_mention{k}")))
        .collect();
    let has = |i: usize, values: &[f64]| {
        mentions
            .iter()
            .any(|m| m[i].is_some_and(|v| values.contains(&v)))
    };
    let n = data.case_count();
    let t = run(
        "survey_basic_phpwriter.sav",
        "WEIGHT OFF.\nCTABLES /TABLE $q6 [COUNT, RESPONSES]\n\
         /CATEGORIES VARIABLES=$q6 [1, 2, SUBTOTAL='First two', 3, 4, 5, 6, HSUBTOTAL='Rest'].",
    );
    assert_eq!(row_label(&t, 2), "First two");
    assert_eq!(row_label(&t, 3), "Rest");
    let cases = |values: &[f64]| (0..n).filter(|&i| has(i, values)).count() as f64;
    let responses = |values: &[f64]| {
        (0..n)
            .map(|i| {
                let mut seen: Vec<f64> = mentions.iter().filter_map(|m| m[i]).collect();
                seen.retain(|v| values.contains(v));
                seen.sort_by(f64::total_cmp);
                seen.dedup();
                seen.len() as f64
            })
            .sum::<f64>()
    };
    let first = [1.0, 2.0];
    let rest = [3.0, 4.0, 5.0, 6.0];
    assert_eq!(
        numbers(&t, 2),
        [Some(cases(&first)), Some(responses(&first))]
    );
    assert_eq!(numbers(&t, 3), [Some(cases(&rest)), Some(responses(&rest))]);
    assert!(cases(&first) < numbers(&t, 0)[0].unwrap() + numbers(&t, 1)[0].unwrap());
}

// A computed category may use a subtotal: SUBTOTAL alone when the list has one, SUBTOTAL[n] by
// position (PCOMPUTE, page 504).
#[test]
fn computed_categories_name_subtotals_by_position() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /PCOMPUTE &d = EXPR(SUBTOTAL[2] - SUBTOTAL[1]) /TABLE q1_satisfaction BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, HSUBTOTAL='Low', 3, 4, 5, HSUBTOTAL='High', &d].",
    );
    let diff: Vec<Option<f64>> = numbers(&t, 1)
        .iter()
        .zip(numbers(&t, 0))
        .map(|(h, l)| Some(h.unwrap() - l.unwrap()))
        .collect();
    assert_eq!(numbers(&t, 2), diff);
    assert_eq!(row_label(&t, 2), "SUBTOTAL[2] - SUBTOTAL[1]");
    let tables = run_all(
        "survey_basic.sav",
        "CTABLES /PCOMPUTE &s = EXPR(SUBTOTAL * 2)\n\
         /TABLE region /CATEGORIES VARIABLES=region [1, 2, SUBTOTAL, &s].",
    );
    let r = &tables[0];
    assert_eq!(numbers(r, 3)[0], Some(numbers(r, 2)[0].unwrap() * 2.0));
    for (syntax, message) in [
        (
            "CTABLES /PCOMPUTE &s = EXPR(SUBTOTAL) /TABLE region\n\
             /CATEGORIES VARIABLES=region [1, SUBTOTAL, 2, SUBTOTAL, &s].",
            "name one by its position",
        ),
        (
            "CTABLES /PCOMPUTE &s = EXPR(SUBTOTAL[3]) /TABLE region\n\
             /CATEGORIES VARIABLES=region [1, SUBTOTAL, 2, SUBTOTAL, &s].",
            "asks for subtotal 3",
        ),
        (
            "CTABLES /PCOMPUTE &s = EXPR(SUBTOTAL) /TABLE region\n\
             /CATEGORIES VARIABLES=region [1, 2, &s].",
            "needs a SUBTOTAL",
        ),
    ] {
        let e = error("survey_basic.sav", syntax);
        assert!(e.contains(message), "{e}");
    }
}

fn labels(t: &Table) -> Vec<String> {
    (0..t.rows.len()).map(|r| row_label(t, r)).collect()
}

// OTHERNM and MISSING in an explicit list (CATEGORIES, "Explicit Category Specification", page
// 494): OTHERNM stands for every valid value the list does not name, ascending, where it
// stands; MISSING for every user-missing value. A value named twice takes its last place, so
// [OTHERNM, 3] is [1, 2, 4, 5, 3], as the manual's example says.
#[test]
fn othernm_and_missing_stand_for_the_values_not_named() {
    let file = "survey_basic.sav";
    let t = |list: &str| {
        run(
            file,
            &format!(
                "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT] BY gender\n\
                 /CATEGORIES VARIABLES=q1_satisfaction {list} TOTAL=YES."
            ),
        )
    };
    let explicit = t("[1, 2, 4, 5, 3]");
    for list in ["[OTHERNM, 3]", "[1 THRU 5, 3]", "[3, OTHERNM, 3]"] {
        let other = t(list);
        assert_eq!(labels(&other), labels(&explicit), "{list}");
        assert_eq!(all_numbers(&other), all_numbers(&explicit), "{list}");
    }
    // MISSING lists 98 and 99 where it stands; OTHERNM leaves them out
    let data = Dataset::read_sav(fixture(file)).unwrap();
    let q1 = raw(&data, "q1_satisfaction");
    let count = |v: f64| q1.iter().filter(|x| **x == Some(v)).count() as f64;
    let m = run(
        file,
        "CTABLES /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [MISSING, 4, OTHERNM].",
    );
    assert_eq!(
        labels(&m),
        [
            "Don't know",
            "Refused",
            "Satisfied",
            "Very dissatisfied",
            "Dissatisfied",
            "Neither satisfied nor dissatisfied",
            "Very satisfied"
        ]
    );
    let expected: Vec<Vec<Option<f64>>> = [98.0, 99.0, 4.0, 1.0, 2.0, 3.0, 5.0]
        .iter()
        .map(|v| vec![Some(count(*v))])
        .collect();
    assert_eq!(all_numbers(&m), expected);
    // a string variable: OTHERNM in ascending order of the text
    let s = run(
        file,
        "CTABLES /TABLE plan /CATEGORIES VARIABLES=plan ['PLUS', OTHERNM].",
    );
    let shown = labels(&s);
    assert_eq!(shown[0], "PLUS");
    let mut sorted = shown[1..].to_vec();
    sorted.sort();
    assert_eq!(shown[1..], sorted);
    assert!(shown.len() > 2 && !shown[1..].contains(&"PLUS".to_string()));

    // computed categories may sum them: [OTHERNM] and [MISSING] (PCOMPUTE, page 505)
    let c = run(
        file,
        "CTABLES /PCOMPUTE &o = EXPR([OTHERNM]) /PCOMPUTE &m = EXPR([MISSING])\n\
         /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [1, OTHERNM, &o, MISSING, &m].",
    );
    let others: f64 = [2.0, 3.0, 4.0, 5.0].iter().map(|v| count(*v)).sum();
    assert_eq!(numbers(&c, 5), [Some(others)]);
    assert_eq!(numbers(&c, 8), [Some(count(98.0) + count(99.0))]);
    let e = error(
        file,
        "CTABLES /PCOMPUTE &o = EXPR([OTHERNM]) /TABLE q1_satisfaction\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, &o].",
    );
    assert!(e.contains("needs OTHERNM"), "{e}");
}

// Sorting by a summary (CATEGORIES, KEY, "Data-Dependent Sorting", page 495): the categories
// are ordered by the summary of each, taken without regard to the other variables of the table;
// the total keeps its place. KEY=MEAN(age) orders by the mean of age in each category.
#[test]
fn categories_sorted_by_a_summary() {
    let file = "survey_basic_phpwriter.sav";
    let data = Dataset::read_sav(fixture(file)).unwrap();
    let (region, age, weight) = (
        raw(&data, "region"),
        raw(&data, "age"),
        raw(&data, "weight"),
    );
    let n = data.case_count();
    let by_region = |f: &dyn Fn(usize) -> f64| -> Vec<(usize, f64)> {
        (1..=5)
            .map(|r| {
                let total = (0..n).filter(|&i| region[i] == Some(r as f64)).map(f).sum();
                (r, total)
            })
            .collect()
    };
    let names = ["North", "South", "East", "West", "Central"];
    let order = |mut v: Vec<(usize, f64)>, descending: bool| -> Vec<String> {
        v.sort_by(|a, b| {
            if descending {
                b.1.total_cmp(&a.1)
            } else {
                a.1.total_cmp(&b.1)
            }
        });
        v.iter().map(|(r, _)| names[r - 1].to_string()).collect()
    };
    // unweighted counts, descending, with the total last
    let t = run(
        file,
        "WEIGHT OFF.\nCTABLES /TABLE region BY gender\n\
         /CATEGORIES VARIABLES=region ORDER=D KEY=COUNT TOTAL=YES.",
    );
    let mut expected = order(by_region(&|_| 1.0), true);
    expected.push("Total".into());
    assert_eq!(labels(&t), expected);
    // weighted counts, ascending; a column percentage orders like the count
    let weighted = by_region(&|i| weight[i].unwrap_or(0.0).max(0.0));
    for key in ["COUNT", "COLPCT.COUNT"] {
        let w = run(
            file,
            &format!(
                "WEIGHT BY weight.\nCTABLES /TABLE region /CATEGORIES VARIABLES=region KEY={key}."
            ),
        );
        assert_eq!(labels(&w), order(weighted.clone(), false), "{key}");
    }
    // the mean of age in each region
    let m = run(
        file,
        "WEIGHT OFF.\nCTABLES /TABLE region > age [MEAN] BY gender\n\
         /CATEGORIES VARIABLES=region ORDER=D KEY=MEAN(age).",
    );
    let sums = by_region(&|i| age[i].unwrap_or(0.0));
    let counts = by_region(&|i| f64::from(u8::from(age[i].is_some())));
    let means: Vec<(usize, f64)> = sums
        .iter()
        .zip(&counts)
        .map(|(s, c)| (s.0, s.1 / c.1))
        .collect();
    let shown: Vec<String> = (0..m.rows.len())
        .map(|r| match &m.rows[r][1] {
            Cell::Text(t) => t.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(shown, order(means, true));

    for (syntax, message) in [
        (
            "CTABLES /TABLE region /CATEGORIES VARIABLES=region KEY=MEAN.",
            "KEY=MEAN needs the variable",
        ),
        (
            "CTABLES /TABLE region BY gender /CATEGORIES VARIABLES=region KEY=COUNT\n\
             /CLABELS ROWLABELS=OPPOSITE.",
            "not available when /CLABELS",
        ),
        (
            "CTABLES /TABLE region /CATEGORIES VARIABLES=region KEY=MEAN(nope).",
            "unknown variable nope",
        ),
    ] {
        let e = error("survey_basic.sav", syntax);
        assert!(e.contains(message), "{e}");
    }
}
