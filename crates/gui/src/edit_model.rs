//! The data editor against a plain model of the data. Every change the editor offers writes
//! syntax; here the model, a table of values and dictionary texts kept in Rust, takes the same
//! change by hand, and the dataset the syntax makes must be that table to the bit: the cell the
//! user typed and nothing else, with weight, filter, split, sets and attributes where they were.
//! Undo must bring back the table exactly, and redo the changed one.

use crate::edits::{self, FORMAT_KINDS, Field, FormatChoice, LabelsEditor, MissingEditor};
use crate::grid_menu::{self, Action, Item, Target};
use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;

#[derive(Clone, PartialEq, Debug)]
enum Cell {
    Num(Option<u64>),
    Str(String),
}

fn num(v: Option<f64>) -> Cell {
    Cell::Num(v.map(f64::to_bits))
}

/// One variable: what the Variable View shows, the dictionary entry as the engine holds it, and
/// the values.
#[derive(Clone, PartialEq, Debug)]
struct Col {
    fields: Vec<String>,
    entry: String,
    cells: Vec<Cell>,
}

/// The dataset as the editor can see and change it.
#[derive(Clone, PartialEq, Debug)]
struct Table {
    cols: Vec<Col>,
    /// Weight, filter, split, sets, attributes, documents and the file label.
    state: String,
}

fn col(data: &Dataset, at: usize) -> Col {
    let v = data.variable_at(at);
    let cells = match v.values() {
        Values::Numeric(values) => values.iter().map(num).collect(),
        Values::String(values) => values.iter().map(|s| Cell::Str(s.to_string())).collect(),
    };
    Col {
        fields: Field::ALL
            .iter()
            .map(|&f| edits::field_text(&v, f, false))
            .collect(),
        entry: format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            v.label(),
            v.format(),
            v.string_width(),
            v.measure(),
            v.value_labels(),
            v.missing_values(),
            v.display_width(),
            (v.alignment(), v.role()),
        ),
        cells,
    }
}

fn table(session: &Session) -> Table {
    let data = session.dataset().unwrap();
    Table {
        cols: (0..data.variable_count()).map(|i| col(data, i)).collect(),
        state: format!(
            "weight {:?} filter {:?} split {:?} sets {:?} {:?} attributes {:?} label {:?} docs {:?} cases {}",
            data.weight_variable(),
            data.filter_variable(),
            data.split_file(),
            data.multiple_response_sets(),
            data.variable_sets(),
            data.attributes(),
            data.file_label(),
            data.documents(),
            data.case_count(),
        ),
    }
}

impl Table {
    fn at(&self, name: &str) -> usize {
        self.cols
            .iter()
            .position(|c| c.fields[0] == name)
            .unwrap_or_else(|| panic!("no variable {name}"))
    }

    fn cases(&self) -> usize {
        self.cols.first().map_or(0, |c| c.cells.len())
    }

    /// The cases in the order of `order`.
    fn reordered(&self, order: &[usize]) -> Table {
        let mut table = self.clone();
        for c in &mut table.cols {
            c.cells = order.iter().map(|&i| c.cells[i].clone()).collect();
        }
        table.state = self.state.replace(
            &format!("cases {}", self.cases()),
            &format!("cases {}", order.len()),
        );
        table
    }
}

/// Runs syntax the way the window does and fails the test with the engine's message.
fn run(session: &mut Session, syntax: &str) {
    if let Err(errors) = session.run(syntax, None) {
        panic!("{syntax}\n{errors:?}");
    }
}

/// What differs between two tables, in a few words.
fn diff(a: &Table, b: &Table) -> String {
    let names = |t: &Table| {
        t.cols
            .iter()
            .map(|c| c.fields[0].clone())
            .collect::<Vec<_>>()
    };
    if names(a) != names(b) {
        return format!("variables {:?} against {:?}", names(a), names(b));
    }
    for (x, y) in a.cols.iter().zip(&b.cols) {
        if x.fields != y.fields {
            return format!(
                "{} fields {:?} against {:?}",
                x.fields[0], x.fields, y.fields
            );
        }
        if x.entry != y.entry {
            return format!("{} entry {} against {}", x.fields[0], x.entry, y.entry);
        }
        if x.cells.len() != y.cells.len() {
            return format!(
                "{} has {} and {} cases",
                x.fields[0],
                x.cells.len(),
                y.cells.len()
            );
        }
        if let Some(i) = (0..x.cells.len()).find(|&i| x.cells[i] != y.cells[i]) {
            return format!(
                "{} case {}: {:?} against {:?}",
                x.fields[0],
                i + 1,
                x.cells[i],
                y.cells[i]
            );
        }
    }
    format!("state {} against {}", a.state, b.state)
}

/// Runs `syntax`; undo must give back the table from before and redo the one after, each
/// exactly. Returns both and leaves the change undone.
fn op(session: &mut Session, syntax: &str) -> (Table, Table) {
    let before = table(session);
    run(session, syntax);
    let after = table(session);
    // a run that changes nothing leaves nothing to undo
    if after == before {
        return (before, after);
    }
    assert!(session.undo(), "{syntax}\nnothing to undo");
    let undone = table(session);
    assert!(
        undone == before,
        "{syntax}\nundo differs: {}",
        diff(&undone, &before)
    );
    assert!(session.redo(), "{syntax}\nnothing to redo");
    let redone = table(session);
    assert!(
        redone == after,
        "{syntax}\nredo differs: {}",
        diff(&redone, &after)
    );
    assert!(session.undo());
    (before, after)
}

fn same(what: &str, syntax: &str, got: &Table, model: &Table) {
    assert!(
        got == model,
        "{what}\n{syntax}\nthe dataset differs from the model: {}",
        diff(got, model)
    );
}

/// Five cases of every kind of cell, written as syntax.
const DATA: &str = "DATA LIST LIST /id (F3) age (F3) income (F10.2) grp (F1) name (A8) joined (ADATE10) dur (TIME8).
BEGIN DATA
1 34 1234.5 1 Anna 03/15/2020 01:02:03
2 51 . 2 'Bo B' 11/01/2023 00:10:00
3 -7 0.001 3 Cy 01/01/1999 23:59:59
4 99 1E7 1 '' 02/29/2024 00:00:01
5 . -2.75 2 dee 12/31/2025 12:00:00
END DATA.
VARIABLE LABELS age 'Age in years' /income 'Income, in euros' /name 'Given name'.
VALUE LABELS grp 1 'Group A' 2 'Group B' 3 'Group C'.
VALUE LABELS name 'Anna' 'The first'.
MISSING VALUES age (99) /income (LOWEST THRU -1) /grp (3) /name ('Cy').
VARIABLE LEVEL grp (NOMINAL) /age (SCALE) /id (ORDINAL).
VARIABLE ALIGNMENT name (CENTER).
VARIABLE WIDTH age (12).
VARIABLE ROLE /TARGET income.";

/// The same, with weight, filter, split, a set, attributes, documents and a file label.
const STATE: &str = "COMPUTE w = 1 + MOD(id, 2).
COMPUTE keep = id <> 3.
FORMATS w (F2) keep (F1).
WEIGHT BY w.
FILTER BY keep.
SPLIT FILE LAYERED BY grp.
MRSETS /MDGROUP NAME=$m CATEGORYLABELS=VARLABELS VARIABLES=age income VALUE=1.
VARIABLE ATTRIBUTE VARIABLES=age ATTRIBUTE=note('kept').
DATAFILE ATTRIBUTE ATTRIBUTE=source('synthetic').
DOCUMENT one line of notes.
FILE LABEL A small test file.";

fn plain() -> Session {
    let mut session = Session::default();
    run(&mut session, DATA);
    session
}

fn with_state() -> Session {
    let mut session = plain();
    run(&mut session, STATE);
    session
}

fn data(session: &Session) -> &Dataset {
    session.dataset().unwrap()
}

fn variable(session: &Session, name: &str) -> usize {
    data(session).variable_index(name).unwrap()
}

/// The syntax of a menu item: a run, or what a confirmed question runs.
fn menu(session: &Session, target: &Target, item: Item) -> String {
    match grid_menu::action(data(session), target, item).unwrap() {
        Action::Run(syntax) => syntax,
        Action::Ask(confirm) => confirm.syntax,
        _ => panic!("{item:?} runs no syntax"),
    }
}

/// The days from 1970-01-01 to a civil date (proleptic Gregorian), by the usual count.
fn days(year: i64, month: i64, day: i64) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The value of a date on the scale of seconds from 1582-10-14, which is 141 428 days before
/// 1970-01-01.
fn seconds(year: i64, month: i64, day: i64, time: i64) -> f64 {
    ((days(year, month, day) + 141_428) * 86_400 + time) as f64
}

#[test]
fn the_model_reads_the_fixture() {
    let session = with_state();
    let t = table(&session);
    assert_eq!(t.cases(), 5);
    assert_eq!(t.cols.len(), 9);
    assert_eq!(
        t.cols[t.at("joined")].cells[0],
        num(Some(seconds(2020, 3, 15, 0)))
    );
    assert_eq!(t.cols[t.at("dur")].cells[0], num(Some(3723.0)));
    assert_eq!(t.cols[t.at("name")].cells[3], Cell::Str(String::new()));
    assert!(t.state.contains("weight Some(\"w\")") && t.state.contains("$m"));
}

fn both(test: impl Fn(&mut Session, &str)) {
    test(&mut plain(), "plain");
    test(&mut with_state(), "state");
}

fn cell_of(t: &Table, name: &str, case: usize) -> Cell {
    t.cols[t.at(name)].cells[case].clone()
}

fn set(t: &Table, name: &str, case: usize, cell: Cell) -> Table {
    let mut t = t.clone();
    let at = t.at(name);
    t.cols[at].cells[case] = cell;
    t
}

fn edit(session: &Session, name: &str, case: usize, text: &str) -> Result<String, String> {
    edits::cell_edit(&data(session).variable(name).unwrap(), case, text, false)
}

/// What a number typed into a cell must become.
fn typed_number(text: &str) -> Cell {
    num(text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|_| !text.trim().is_empty()))
}

#[test]
fn a_typed_number_is_stored_to_the_bit() {
    let texts = [
        "7",
        "-0.5",
        "1e-3",
        "1E5",
        "5.",
        ".5",
        "+3",
        "0.1",
        "0.30000000000000004",
        "123456789.123456789",
        "1e308",
        "1.7976931348623157e308",
        "4.9e-324",
        "2.2250738585072014e-308",
        "9007199254740993",
        "-1e15",
        "",
        "  42  ",
    ];
    both(|session, state| {
        let before = table(session);
        for name in ["age", "income", "id"] {
            for (i, text) in texts.iter().enumerate() {
                let case = i % 5;
                let syntax = edit(session, name, case, text).unwrap();
                let (_, after) = op(session, &syntax);
                same(
                    &format!("{state}: {text:?} into {name}"),
                    &syntax,
                    &after,
                    &set(&before, name, case, typed_number(text)),
                );
            }
        }
    });
}

#[test]
fn text_that_is_not_a_number_is_refused_before_any_syntax() {
    let session = plain();
    for text in [
        "abc", "1,5", "1 2", "NaN", "inf", "-inf", "1e999", "0x10", "--1", "1e",
    ] {
        assert!(
            edit(&session, "age", 0, text).is_err(),
            "{text:?} was accepted"
        );
    }
}

#[test]
fn a_typed_string_is_stored_as_typed() {
    let texts = [
        "x",
        "",
        "it's",
        "  lead",
        "trail  ",
        "\u{c4}\u{d6}\u{dc}",
        "12345678",
        "a;b",
        "a\\b",
        "'",
        "''",
        "a\nb",
    ];
    both(|session, state| {
        let before = table(session);
        for (i, text) in texts.iter().enumerate() {
            let case = i % 5;
            let syntax = edit(session, "name", case, text).unwrap();
            let (_, after) = op(session, &syntax);
            same(
                &format!("{state}: {text:?} into name"),
                &syntax,
                &after,
                &set(
                    &before,
                    "name",
                    case,
                    Cell::Str(text.trim_end().to_string()),
                ),
            );
        }
    });
}

#[test]
fn a_string_longer_than_its_width_is_cut_to_it() {
    let mut session = plain();
    let syntax = edit(&session, "name", 0, "123456789").unwrap();
    let (before, after) = op(&mut session, &syntax);
    same(
        "too long",
        &syntax,
        &after,
        &set(&before, "name", 0, Cell::Str("12345678".into())),
    );
}

#[test]
fn a_typed_date_is_that_day() {
    let texts = [
        ("2024-02-29", seconds(2024, 2, 29, 0)),
        ("1999-12-31", seconds(1999, 12, 31, 0)),
        ("1583-01-01", seconds(1583, 1, 1, 0)),
        ("9999-12-31", seconds(9999, 12, 31, 0)),
        ("2000-02-29 10:30", seconds(2000, 2, 29, 37_800)),
        ("2026-11-09 23:59:59", seconds(2026, 11, 9, 86_399)),
        (" 2026-1-9 ", seconds(2026, 1, 9, 0)),
    ];
    both(|session, state| {
        let before = table(session);
        for (i, (text, value)) in texts.iter().enumerate() {
            let syntax = edit(session, "joined", i % 5, text).unwrap();
            let (_, after) = op(session, &syntax);
            same(
                &format!("{state}: {text:?} into joined"),
                &syntax,
                &after,
                &set(&before, "joined", i % 5, num(Some(*value))),
            );
        }
    });
}

#[test]
fn a_date_that_does_not_exist_is_refused_or_not_stored_as_another_day() {
    let mut session = plain();
    for text in [
        "2023-02-29",
        "2024-04-31",
        "2024-06-31",
        "1582-12-31",
        "0000-01-01",
        "2024-00-10",
    ] {
        match edit(&session, "joined", 0, text) {
            Err(_) => {}
            Ok(syntax) => {
                let before = table(&session);
                match session.run(&syntax, None) {
                    Err(_) => assert!(table(&session) == before, "{text}: failed run changed data"),
                    Ok(_) => {
                        let got = cell_of(&table(&session), "joined", 0);
                        panic!("{text:?} was accepted and became {got:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn editing_a_cell_without_changing_its_text_keeps_every_bit() {
    both(|session, state| {
        let before = table(session);
        let mut syntax = String::new();
        let data = data(session);
        for v in data.variables() {
            for case in 0..data.case_count() {
                let text = edits::edit_text(&v, case, false);
                syntax.push_str(&edits::cell_edit(&v, case, &text, false).unwrap());
                syntax.push('\n');
            }
        }
        let (_, after) = op(session, &syntax);
        same(&format!("{state}: retyped cells"), &syntax, &after, &before);
    });
}

#[test]
fn a_value_typed_under_the_empty_row_adds_a_case() {
    both(|session, state| {
        let before = table(session);
        let cases = before.cases();
        for (name, text, cell) in [
            ("age", "12", num(Some(12.0))),
            ("name", "new", Cell::Str("new".into())),
            ("joined", "2024-02-29", num(Some(seconds(2024, 2, 29, 0)))),
            ("age", "", num(None)),
        ] {
            let syntax = format!("ADD CASE.\n{}", edit(session, name, cases, text).unwrap());
            let (_, after) = op(session, &syntax);
            let mut model = before.clone();
            for c in &mut model.cols {
                c.cells.push(match c.cells[0] {
                    Cell::Num(_) => num(None),
                    Cell::Str(_) => Cell::Str(String::new()),
                });
            }
            let at = model.at(name);
            *model.cols[at].cells.last_mut().unwrap() = cell;
            model.state = before
                .state
                .replace(&format!("cases {cases}"), &format!("cases {}", cases + 1));
            same(
                &format!("{state}: new case with {name}"),
                &syntax,
                &after,
                &model,
            );
        }
    });
}

#[test]
fn delete_clears_the_selected_cells_and_nothing_else() {
    both(|session, state| {
        let before = table(session);
        for (first, last) in [(0, 0), (1, 3), (0, 4), (4, 4)] {
            let syntax = {
                let data = data(session);
                let block: Vec<_> = ["age", "name", "joined"]
                    .iter()
                    .map(|n| data.variable(n).unwrap())
                    .collect();
                edits::clear_cells(&block, first, last)
            };
            let (_, after) = op(session, &syntax);
            let mut model = before.clone();
            for name in ["age", "name", "joined"] {
                let at = model.at(name);
                let empty = match name {
                    "name" => Cell::Str(String::new()),
                    _ => num(None),
                };
                model.cols[at].cells[first..=last].fill(empty);
            }
            same(
                &format!("{state}: clear {first}..{last}"),
                &syntax,
                &after,
                &model,
            );
        }
    });
}

#[test]
fn delete_case_removes_that_case_again_and_again() {
    both(|session, state| {
        // delete the middle, then the first, then the last of what is left, as three runs
        let mut model = table(session);
        for pick in [2usize, 0, 2] {
            let syntax = menu(session, &Target::Cases(vec![pick]), Item::DeleteCase);
            let (_, after) = op(session, &syntax);
            let order: Vec<usize> = (0..model.cases()).filter(|&i| i != pick).collect();
            let next = model.reordered(&order);
            same(
                &format!("{state}: delete case {}", pick + 1),
                &syntax,
                &after,
                &next,
            );
            run(session, &syntax);
            model = next;
        }
        assert_eq!(model.cases(), 2);
        // and the last two, to no case at all
        for _ in 0..2 {
            let syntax = menu(session, &Target::Cases(vec![0]), Item::DeleteCase);
            run(session, &syntax);
        }
        assert_eq!(table(session).cases(), 0);
        assert!(session.undo());
        assert_eq!(table(session).cases(), 1);
    });
}

fn names(t: &Table) -> Vec<String> {
    t.cols.iter().map(|c| c.fields[0].clone()).collect()
}

/// A column of a new numeric variable, F8.2 and no value at any case.
fn assert_new_numeric(c: &Col, name: &str, cases: usize) {
    assert_eq!(c.fields[0], name);
    assert_eq!(c.fields[2], "F8.2");
    assert_eq!(c.fields[1], "Numeric");
    assert_eq!(c.cells, vec![num(None); cases]);
    assert!(c.fields[5].is_empty() && c.fields[6].is_empty() && c.fields[7].is_empty());
}

#[test]
fn insert_variable_puts_one_empty_variable_in_place_and_keeps_the_rest() {
    both(|session, state| {
        let cols = table(session).cols.len();
        for at in 0..cols {
            let syntax = menu(session, &Target::Variables(vec![at]), Item::InsertBefore);
            let (before, after) = op(session, &syntax);
            let mut model = before.clone();
            assert_new_numeric(&after.cols[at], "new_1", before.cases());
            model.cols.insert(at, after.cols[at].clone());
            same(
                &format!("{state}: insert before {at}"),
                &syntax,
                &after,
                &model,
            );
        }
    });
}

#[test]
fn insert_variable_twice_names_the_second_new_2() {
    let mut session = with_state();
    let syntax = menu(&session, &Target::Variables(vec![1]), Item::InsertBefore);
    run(&mut session, &syntax);
    let syntax = menu(&session, &Target::Variables(vec![1]), Item::InsertBefore);
    let (before, after) = op(&mut session, &syntax);
    assert_new_numeric(&after.cols[1], "new_2", 5);
    assert_eq!(names(&after)[..3], ["id", "new_2", "new_1"]);
    let mut model = before.clone();
    model.cols.insert(1, after.cols[1].clone());
    same("second insert", &syntax, &after, &model);
}

#[test]
fn a_name_in_the_empty_variable_row_adds_a_variable_at_the_end() {
    both(|session, state| {
        let syntax = edits::new_variable("fresh").unwrap();
        let (before, after) = op(session, &syntax);
        assert_new_numeric(after.cols.last().unwrap(), "fresh", before.cases());
        let mut model = before.clone();
        model.cols.push(after.cols.last().unwrap().clone());
        same(&format!("{state}: new variable"), &syntax, &after, &model);
    });
}

#[test]
fn delete_variables_removes_those_and_only_those() {
    both(|session, state| {
        let count = table(session).cols.len();
        for pick in [
            vec![0],
            vec![count - 1],
            vec![1, 2],
            vec![0, 2, 4],
            vec![1, 2, 3, 4],
        ] {
            let syntax = menu(session, &Target::Variables(pick.clone()), Item::Delete);
            let (before, after) = op(session, &syntax);
            let mut model = before.clone();
            model.cols = before
                .cols
                .iter()
                .enumerate()
                .filter(|(i, _)| !pick.contains(i))
                .map(|(_, c)| c.clone())
                .collect();
            assert_eq!(
                after.cols,
                model.cols,
                "{state}: delete {pick:?}: {}",
                diff(&after, &model)
            );
        }
    });
}

#[test]
fn deleting_the_variables_of_the_state_ends_what_depended_on_them() {
    let mut session = with_state();
    for (name, gone, kept) in [
        ("w", "weight None", "filter Some(\"keep\")"),
        ("keep", "filter None", "weight Some(\"w\")"),
        ("grp", "split None", "weight Some(\"w\")"),
        ("age", "sets [] []", "weight Some(\"w\")"),
    ] {
        let at = variable(&session, name);
        let syntax = menu(&session, &Target::Variables(vec![at]), Item::Delete);
        let (_, after) = op(&mut session, &syntax);
        assert!(after.state.contains(gone), "{name}: {}", after.state);
        assert!(after.state.contains(kept), "{name}: {}", after.state);
    }
}

fn field_syntax(
    session: &Session,
    name: &str,
    field: Field,
    text: &str,
) -> Result<Option<String>, String> {
    edits::field_edit(&data(session).variable(name).unwrap(), field, text, false)
}

/// The model of a Variable View edit: the field texts at these places become these.
fn with_fields(before: &Table, after: &Table, name: &str, changes: &[(usize, &str)]) -> Table {
    let mut model = before.clone();
    let at = model.at(name);
    for (i, text) in changes {
        model.cols[at].fields[*i] = text.to_string();
    }
    // the entry holds the same facts as the field texts; the fields are what is compared
    model.cols[at].entry = after.cols[at].entry.clone();
    model.state = after.state.clone();
    model
}

#[test]
fn a_variable_view_edit_changes_that_field_and_nothing_else() {
    use Field::*;
    type Case<'a> = (&'a str, Field, &'a str, Vec<(usize, &'a str)>);
    let cases: Vec<Case> = vec![
        ("age", Name, "years", vec![(0, "years")]),
        ("age", Label, "How old", vec![(5, "How old")]),
        ("age", Label, "", vec![(5, "")]),
        (
            "age",
            Label,
            "it's a \"label\"",
            vec![(5, "it's a \"label\"")],
        ),
        (
            "age",
            Label,
            "\u{d5}ppur\u{fc}li",
            vec![(5, "\u{d5}ppur\u{fc}li")],
        ),
        (
            "income",
            Format,
            "F12.1",
            vec![(2, "F12.1"), (3, "12"), (4, "1")],
        ),
        ("income", Width, "14", vec![(2, "F14.2"), (3, "14")]),
        ("income", Decimals, "3", vec![(2, "F11.3"), (4, "3")]),
        (
            "income",
            Format,
            "COMMA10.2",
            vec![(2, "COMMA10.2"), (3, "10")],
        ),
        (
            "income",
            Format,
            "DOLLAR9.2",
            vec![(2, "DOLLAR9.2"), (3, "9")],
        ),
        (
            "joined",
            Format,
            "date11",
            vec![(2, "DATE11"), (3, "11"), (4, "0")],
        ),
        (
            "grp",
            Values,
            "1 = Low; 2 = High",
            vec![(6, "1 = Low; 2 = High")],
        ),
        ("grp", Values, "", vec![(6, "")]),
        ("grp", Values, "1.5 = Half", vec![(6, "1.5 = Half")]),
        (
            "name",
            Values,
            "a = Alpha; b = Beta",
            vec![(6, "a = Alpha; b = Beta")],
        ),
        ("age", Missing, "98, 99", vec![(7, "98, 99")]),
        ("age", Missing, "", vec![(7, "")]),
        (
            "income",
            Missing,
            "LO THRU 0, 999",
            vec![(7, "LO THRU 0, 999")],
        ),
        ("income", Missing, "5 THRU HI", vec![(7, "5 THRU HI")]),
        ("name", Missing, "x, y", vec![(7, "x, y")]),
        ("age", Columns, "20", vec![(8, "20")]),
        ("name", Align, "Right", vec![(9, "Right")]),
        ("age", Measure, "Nominal", vec![(10, "Nominal")]),
        ("age", Measure, "ordinal", vec![(10, "Ordinal")]),
        ("age", Role, "Both", vec![(11, "Both")]),
        ("age", Role, "none", vec![(11, "None")]),
    ];
    both(|session, state| {
        for (name, field, text, changes) in &cases {
            let syntax = field_syntax(session, name, *field, text)
                .unwrap_or_else(|e| panic!("{state}: {name} {field:?} {text:?}: {e}"))
                .unwrap_or_else(|| panic!("{name} {field:?} {text:?}: no change"));
            let (before, after) = op(session, &syntax);
            let model = with_fields(&before, &after, name, changes);
            same(
                &format!("{state}: {name} {field:?} {text:?}"),
                &syntax,
                &after,
                &model,
            );
        }
    });
}

#[test]
fn a_variable_view_edit_that_cannot_be_right_is_refused_and_changes_nothing() {
    use Field::*;
    let cases = [
        ("age", Type, "String"),
        ("age", Name, ""),
        ("age", Name, "1abc"),
        ("age", Name, "has space"),
        ("age", Name, "income"),
        ("age", Name, "INCOME"),
        ("age", Name, "all"),
        ("age", Name, "$sysmis"),
        ("age", Width, "abc"),
        ("age", Width, "-3"),
        ("age", Width, "0"),
        ("age", Width, "100"),
        ("age", Decimals, "9"),
        ("age", Decimals, "x"),
        ("age", Format, "F"),
        ("age", Format, "A5"),
        ("age", Format, "ZZ9"),
        ("age", Format, "F8.2.1"),
        ("name", Format, "F8"),
        ("name", Width, "10"),
        ("age", Columns, "0"),
        ("age", Columns, "-1"),
        ("age", Columns, "abc"),
        ("age", Align, "up"),
        ("age", Measure, "huge"),
        ("age", Role, "boss"),
        ("grp", Values, "1 Low"),
        ("grp", Values, "x = Low"),
        ("age", Missing, "abc"),
        ("age", Missing, "1, 2, 3, 4"),
        ("age", Missing, "LO THRU"),
        ("name", Missing, "a, b, c, d"),
    ];
    let mut session = with_state();
    for (name, field, text) in cases {
        let before = table(&session);
        let syntax = match field_syntax(&session, name, field, text) {
            Err(_) | Ok(None) => continue,
            Ok(Some(syntax)) => syntax,
        };
        match session.run(&syntax, None) {
            Err(_) => assert!(
                table(&session) == before,
                "{name} {field:?} {text:?}: failed run changed data"
            ),
            Ok(_) => {
                let after = table(&session);
                panic!(
                    "{name} {field:?} {text:?} was accepted: {}",
                    diff(&before, &after)
                );
            }
        }
    }
}

#[test]
fn renaming_follows_into_the_weight_filter_split_and_sets() {
    for (old, new) in [
        ("w", "weights"),
        ("keep", "kept"),
        ("grp", "group"),
        ("age", "years"),
    ] {
        let mut session = with_state();
        let syntax = field_syntax(&session, old, Field::Name, new)
            .unwrap()
            .unwrap();
        let (before, after) = op(&mut session, &syntax);
        assert!(
            !after.state.contains(&format!("\"{old}\"")),
            "{old} is still named: {}",
            after.state
        );
        assert_eq!(
            after.state,
            before
                .state
                .replace(&format!("\"{old}\""), &format!("\"{new}\""))
        );
        let at = before.at(old);
        assert_eq!(after.cols[at].cells, before.cols[at].cells);
    }
}

fn kind_of(keyword: &str) -> usize {
    FORMAT_KINDS
        .iter()
        .position(|k| k.keyword == keyword)
        .unwrap()
}

fn choice(
    session: &Session,
    name: &str,
    keyword: &str,
    width: &str,
    decimals: &str,
) -> FormatChoice {
    let mut c = FormatChoice::of(&data(session).variable(name).unwrap());
    c.kind = kind_of(keyword);
    c.width = width.into();
    c.decimals = decimals.into();
    c
}

#[test]
fn the_format_dialog_changes_the_format_and_keeps_the_values() {
    both(|session, state| {
        for (name, keyword, width, decimals, shown) in [
            ("income", "F", "12", "3", "F12.3"),
            ("income", "COMMA", "10", "2", "COMMA10.2"),
            ("income", "DOLLAR", "10", "0", "DOLLAR10.0"),
            ("income", "PCT", "9", "1", "PCT9.1"),
            ("income", "E", "12", "3", "E12.3"),
            ("id", "N", "5", "0", "N5.0"),
            ("age", "F", "5", "0", "F5.0"),
            ("joined", "DATE", "11", "0", "DATE11"),
            ("joined", "SDATE", "10", "0", "SDATE10"),
            ("dur", "TIME", "10", "2", "TIME10.2"),
        ] {
            let syntax = choice(session, name, keyword, width, decimals)
                .syntax()
                .unwrap();
            let (before, after) = op(session, &syntax);
            let at = before.at(name);
            assert_eq!(after.cols[at].fields[2], shown, "{state}: {name} {syntax}");
            assert_eq!(
                after.cols[at].cells, before.cols[at].cells,
                "{state}: {name} {syntax}"
            );
            for (i, (a, b)) in after.cols.iter().zip(&before.cols).enumerate() {
                if i != at {
                    assert!(a == b, "{state}: {name} {syntax}: {} changed", a.fields[0]);
                }
            }
            assert_eq!(after.state, before.state);
        }
    });
}

fn strs(t: &Table, name: &str) -> Vec<String> {
    t.cols[t.at(name)]
        .cells
        .iter()
        .map(|c| match c {
            Cell::Str(s) => s.clone(),
            other => panic!("{other:?} is not a string"),
        })
        .collect()
}

#[test]
fn a_string_changes_its_width_and_a_number_becomes_text_and_back() {
    let mut session = plain();
    // wider: the same text; narrower: cut, and a label on a cut value goes with it
    let syntax = choice(&session, "name", "A", "12", "0").syntax().unwrap();
    let (before, after) = op(&mut session, &syntax);
    assert_eq!(strs(&after, "name"), strs(&before, "name"));
    assert_eq!(after.cols[after.at("name")].fields[2], "A12");
    let syntax = choice(&session, "name", "A", "3", "0").syntax().unwrap();
    let (_, after) = op(&mut session, &syntax);
    assert_eq!(strs(&after, "name"), ["Ann", "Bo", "Cy", "", "dee"]);
    // a number becomes its text without padding, system-missing becomes blank, and the labels
    // and missing values of a number do not carry over
    let syntax = choice(&session, "age", "A", "6", "0").syntax().unwrap();
    let (before, after) = op(&mut session, &syntax);
    assert_eq!(strs(&after, "age"), ["34", "51", "-7", "99", ""]);
    let at = before.at("age");
    assert!(!before.cols[at].fields[7].is_empty());
    assert!(after.cols[at].fields[6].is_empty() && after.cols[at].fields[7].is_empty());
    let syntax = choice(&session, "grp", "A", "2", "0").syntax().unwrap();
    let (before, after) = op(&mut session, &syntax);
    assert_eq!(strs(&after, "grp"), ["1", "2", "3", "1", "2"]);
    assert!(before.cols[before.at("grp")].fields[6].starts_with("1 = Group A"));
    assert!(after.cols[after.at("grp")].fields[6].is_empty());
    // and back: text that reads as a number becomes it, anything else system-missing
    run(
        &mut session,
        "STRING t (A5).\nCOMPUTE t = '12'.\nIF (id = 2) t = '3.5'.\nIF (id = 3) t = ' 7'.\nIF (id = 4) t = 'x'.\nIF (id = 5) t = ''.",
    );
    let syntax = choice(&session, "t", "F", "8", "0").syntax().unwrap();
    let (_, after) = op(&mut session, &syntax);
    let t = &after.cols[after.at("t")];
    assert_eq!(
        t.cells,
        [
            num(Some(12.0)),
            num(Some(3.5)),
            num(Some(7.0)),
            num(None),
            num(None)
        ]
    );
    // a string cannot be given a date format straight away
    assert!(choice(&session, "t", "DATE", "11", "0").syntax().is_err());
}

fn labels_of(session: &Session, name: &str, apply: &[&str]) -> LabelsEditor {
    let others = apply.iter().map(|s| s.to_string()).collect();
    LabelsEditor::of(&data(session).variable(name).unwrap(), others, false)
}

#[test]
fn the_labels_dialog_sets_exactly_the_labels_it_shows() {
    both(|session, state| {
        let mut d = labels_of(session, "grp", &[]);
        d.pairs = vec![
            ("1".into(), "One".into()),
            ("2".into(), "Two".into()),
            ("7".into(), "It's seven".into()),
        ];
        let syntax = d.syntax().unwrap();
        let (before, after) = op(session, &syntax);
        let at = before.at("grp");
        assert_eq!(
            after.cols[at].fields[6], "1 = One; 2 = Two; 7 = It's seven",
            "{state}"
        );
        assert_eq!(after.cols[at].cells, before.cols[at].cells);
        // a string variable
        let mut d = labels_of(session, "name", &[]);
        d.pairs = vec![
            ("Anna".into(), "First".into()),
            ("dee".into(), "Last".into()),
        ];
        let syntax = d.syntax().unwrap();
        let (before, after) = op(session, &syntax);
        let at = before.at("name");
        assert_eq!(
            after.cols[at].fields[6], "Anna = First; dee = Last",
            "{state}"
        );
        assert_eq!(after.cols[at].cells, before.cols[at].cells);
        // many pasted at once: added to the labels, or replacing them
        let mut d = labels_of(session, "grp", &[]);
        d.quick = true;
        d.pasted = "2\tTwo\n4\tFour".into();
        let syntax = d.syntax().unwrap();
        let (_, added) = op(session, &syntax);
        let at = added.at("grp");
        // a code that is pasted again moves to the end of the list
        assert_eq!(
            added.cols[at].fields[6], "1 = Group A; 3 = Group C; 2 = Two; 4 = Four",
            "{state}"
        );
        d.replace = true;
        let syntax = d.syntax().unwrap();
        let (_, replaced) = op(session, &syntax);
        assert_eq!(replaced.cols[at].fields[6], "2 = Two; 4 = Four", "{state}");
        // the same labels on other variables
        let mut d = labels_of(session, "grp", &["id", "age"]);
        d.apply_to_others = true;
        let syntax = d.syntax().unwrap();
        let (before, after) = op(session, &syntax);
        for name in ["id", "age"] {
            let at = before.at(name);
            assert_eq!(
                after.cols[at].fields[6], "1 = Group A; 2 = Group B; 3 = Group C",
                "{state}: {name}"
            );
            assert_eq!(after.cols[at].cells, before.cols[at].cells);
        }
    });
}

#[test]
fn the_missing_values_dialog_sets_exactly_what_it_shows() {
    both(|session, state| {
        let others = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut d = MissingEditor::of(&data(session).variable("age").unwrap(), vec![], false);
        d.values = ["-1".into(), "98".into(), "99.5".into()];
        let (before, after) = op(session, &d.syntax().unwrap());
        let at = before.at("age");
        assert_eq!(after.cols[at].fields[7], "-1, 98, 99.5", "{state}");
        assert_eq!(after.cols[at].cells, before.cols[at].cells);
        let mut d = MissingEditor::of(&data(session).variable("income").unwrap(), vec![], false);
        d.range = true;
        d.low = "LO".into();
        d.high = "0".into();
        d.extra = "999".into();
        let (before, after) = op(session, &d.syntax().unwrap());
        let at = before.at("income");
        assert_eq!(after.cols[at].fields[7], "LO THRU 0, 999", "{state}");
        let mut d = MissingEditor::of(&data(session).variable("income").unwrap(), vec![], false);
        d.range = true;
        d.low = "10".into();
        d.high = "HI".into();
        let (_, after) = op(session, &d.syntax().unwrap());
        assert_eq!(after.cols[at].fields[7], "10 THRU HI", "{state}");
        let mut d = MissingEditor::of(&data(session).variable("name").unwrap(), vec![], false);
        d.values = ["Anna".into(), "dee".into(), String::new()];
        let (before, after) = op(session, &d.syntax().unwrap());
        let at = before.at("name");
        assert_eq!(after.cols[at].fields[7], "Anna, dee", "{state}");
        assert_eq!(after.cols[at].cells, before.cols[at].cells);
        // to other variables too
        let mut d = MissingEditor::of(
            &data(session).variable("age").unwrap(),
            others(&["id", "grp"]),
            false,
        );
        d.apply_to_others = true;
        d.values = ["-9".into(), String::new(), String::new()];
        let (before, after) = op(session, &d.syntax().unwrap());
        for name in ["age", "id", "grp"] {
            let at = before.at(name);
            assert_eq!(after.cols[at].fields[7], "-9", "{state}: {name}");
            assert_eq!(after.cols[at].cells, before.cols[at].cells);
        }
    });
}

/// A sort ends a split file, which is why the menu asks first; nothing else of the state moves.
fn after_sort(t: Table) -> Table {
    let mut t = t;
    t.state = t.state.replace(
        "split Some(SplitFile { variables: [\"grp\"], layered: true })",
        "split None",
    );
    t
}

/// The cases in the order `SORT CASES BY` the keys gives: system-missing first, ties in file
/// order, a descending key reversing only its own comparison.
fn sorted(t: &Table, keys: &[(&str, bool)]) -> Table {
    let mut order: Vec<usize> = (0..t.cases()).collect();
    order.sort_by(|&a, &b| {
        for (name, descending) in keys {
            let c = &t.cols[t.at(name)].cells;
            let ordering = match (&c[a], &c[b]) {
                (Cell::Num(x), Cell::Num(y)) => {
                    let value = |v: &Option<u64>| v.map(f64::from_bits);
                    let (x, y) = (value(x), value(y));
                    match (x, y) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, _) => std::cmp::Ordering::Less,
                        (_, None) => std::cmp::Ordering::Greater,
                        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap(),
                    }
                }
                (Cell::Str(x), Cell::Str(y)) => x.cmp(y),
                _ => unreachable!(),
            };
            let ordering = if *descending {
                ordering.reverse()
            } else {
                ordering
            };
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
        }
        std::cmp::Ordering::Equal
    });
    t.reordered(&order)
}

#[test]
fn sorting_from_the_menu_orders_the_cases_and_undo_puts_them_back() {
    both(|session, state| {
        let names = ["id", "age", "income", "grp", "name", "joined", "dur"];
        for name in names {
            for descending in [false, true] {
                let item = if descending {
                    Item::SortDescending
                } else {
                    Item::SortAscending
                };
                let target = Target::Variables(vec![variable(session, name)]);
                let syntax = menu(session, &target, item);
                let (before, after) = op(session, &syntax);
                same(
                    &format!("{state}: sort by {name} {descending}"),
                    &syntax,
                    &after,
                    &after_sort(sorted(&before, &[(name, descending)])),
                );
                // from a cell of the column too
                let cell = Target::Cell {
                    case: 1,
                    column: variable(session, name),
                };
                assert_eq!(menu(session, &cell, item), syntax);
            }
        }
        // several columns: one direction for all, in file order
        for (picked, descending) in [
            (["grp", "name"], false),
            (["age", "grp"], true),
            (["grp", "income"], false),
        ] {
            let item = if descending {
                Item::SortDescending
            } else {
                Item::SortAscending
            };
            let mut at: Vec<usize> = picked.iter().map(|n| variable(session, n)).collect();
            at.sort_unstable();
            let target = Target::Variables(at.clone());
            let syntax = menu(session, &target, item);
            let (before, after) = op(session, &syntax);
            let mut keys: Vec<(&str, bool)> = Vec::new();
            for name in before.cols.iter().map(|c| c.fields[0].as_str()) {
                if picked.contains(&name) {
                    keys.push((name, descending));
                }
            }
            same(
                &format!("{state}: sort by {picked:?}"),
                &syntax,
                &after,
                &after_sort(sorted(&before, &keys)),
            );
        }
    });
}

#[test]
fn sorting_twice_in_a_row_undoes_one_sort_at_a_time() {
    let mut session = plain();
    let t0 = table(&session);
    let first = menu(
        &session,
        &Target::Variables(vec![variable(&session, "age")]),
        Item::SortAscending,
    );
    run(&mut session, &first);
    let t1 = table(&session);
    let second = menu(
        &session,
        &Target::Variables(vec![variable(&session, "name")]),
        Item::SortDescending,
    );
    run(&mut session, &second);
    assert!(session.undo());
    assert!(table(&session) == t1);
    assert!(session.undo());
    assert!(table(&session) == t0);
}

#[test]
fn a_resized_column_changes_only_its_display_width() {
    both(|session, state| {
        for (name, pixels) in [("age", 120.0f32), ("name", 40.0)] {
            let syntax = crate::grid::width_command(name, pixels, 8.0);
            let (before, after) = op(session, &syntax);
            let at = before.at(name);
            let mut model = before.clone();
            model.cols[at].fields[8] = after.cols[at].fields[8].clone();
            model.cols[at].entry = after.cols[at].entry.clone();
            assert_ne!(
                after.cols[at].fields[8], before.cols[at].fields[8],
                "{state}: {name}"
            );
            same(&format!("{state}: resize {name}"), &syntax, &after, &model);
        }
    });
}

#[test]
fn filter_from_a_cell_keeps_the_data_and_undo_ends_it() {
    let mut session = plain();
    let cell = Target::Cell {
        case: 0,
        column: variable(&session, "grp"),
    };
    let syntax = menu(&session, &cell, Item::Filter);
    let (before, after) = op(&mut session, &syntax);
    assert!(after.state.contains("filter Some("), "{}", after.state);
    assert_eq!(after.cases(), before.cases());
    for c in &before.cols {
        let a = &after.cols[after.at(&c.fields[0])];
        assert_eq!(a.cells, c.cells);
    }
    run(&mut session, &syntax);
    let off = menu(&session, &cell, Item::FilterOff);
    run(&mut session, &off);
    assert!(table(&session).state.contains("filter None"));
}

/// Under SET DECIMAL=COMMA the grid shows and takes a decimal comma.
fn comma_session(state: bool) -> Session {
    let mut session = if state { with_state() } else { plain() };
    run(&mut session, "SET DECIMAL=COMMA.");
    session
}

fn comma_edit(session: &Session, name: &str, case: usize, text: &str) -> Result<String, String> {
    edits::cell_edit(&data(session).variable(name).unwrap(), case, text, true)
}

#[test]
fn a_comma_typed_under_set_decimal_comma_is_stored_to_the_bit() {
    let texts = [
        ("3,5", 3.5),
        ("-0,5", -0.5),
        (",5", 0.5),
        ("5,", 5.0),
        ("1,5e3", 1500.0),
        ("0,30000000000000004", 0.30000000000000004),
        ("123456789,123456789", 123_456_789.123_456_79),
        ("7", 7.0),
        ("  42  ", 42.0),
    ];
    for state in [false, true] {
        let mut session = comma_session(state);
        let before = table(&session);
        for name in ["age", "income", "id"] {
            for (i, (text, value)) in texts.iter().enumerate() {
                let case = i % 5;
                let syntax = comma_edit(&session, name, case, text).unwrap();
                let (_, after) = op(&mut session, &syntax);
                same(
                    &format!("{text:?} into {name}"),
                    &syntax,
                    &after,
                    &set(&before, name, case, num(Some(*value))),
                );
            }
        }
    }
}

#[test]
fn under_a_decimal_comma_a_period_or_a_second_comma_is_refused_with_the_reason() {
    let session = comma_session(false);
    for text in ["3.5", "1.234,5", "1,2,3", "abc", "1;5"] {
        assert!(comma_edit(&session, "income", 0, text).is_err(), "{text:?}");
    }
    let message = comma_edit(&session, "income", 0, "3.5").unwrap_err();
    assert!(message.contains("comma"), "{message}");
    // the period stays the mark when the setting is the default
    let plain = plain();
    let dot = edits::cell_edit(&data(&plain).variable("income").unwrap(), 0, "3.5", false);
    assert!(dot.is_ok());
    assert!(edits::cell_edit(&data(&plain).variable("income").unwrap(), 0, "3,5", false).is_err());
}

#[test]
fn under_a_decimal_comma_the_cells_show_and_edit_with_a_comma_and_keep_every_bit() {
    for state in [false, true] {
        let mut session = comma_session(state);
        let before = table(&session);
        let currencies = session.currencies();
        let v = data(&session).variable("income").unwrap();
        assert_eq!(edits::cell_text(&v, 0, false, &currencies, true), "1234,50");
        assert_eq!(edits::cell_text(&v, 4, false, &currencies, true), "-2,75");
        assert_eq!(edits::edit_text(&v, 4, true), "-2,75");
        assert_eq!(
            edits::cell_text(&v, 0, false, &currencies, false),
            "1234.50"
        );
        // grouped and dollar formats keep their own marks: COMMA shows a period for the point
        run(&mut session, "FORMATS income (COMMA11.2).");
        let v = data(&session).variable("income").unwrap();
        assert_eq!(
            edits::cell_text(&v, 0, false, &currencies, true),
            "1,234.50"
        );
        run(&mut session, "FORMATS income (DOT11.2).");
        let v = data(&session).variable("income").unwrap();
        assert_eq!(
            edits::cell_text(&v, 0, false, &currencies, true),
            "1.234,50"
        );
        assert!(session.undo() && session.undo());
        // every cell retyped as the editor shows it keeps its bits
        let mut syntax = String::new();
        let data = data(&session);
        for v in data.variables() {
            for case in 0..data.case_count() {
                let text = edits::edit_text(&v, case, true);
                syntax.push_str(&edits::cell_edit(&v, case, &text, true).unwrap());
                syntax.push('\n');
            }
        }
        let (_, after) = op(&mut session, &syntax);
        same("retyped under a comma", &syntax, &after, &before);
    }
}

/// The session with a length of time in each of the three duration formats and a date in each
/// of the date formats the editor types as a date.
fn with_times(comma: bool) -> Session {
    let mut session = plain();
    run(
        &mut session,
        "COMPUTE m = dur.\nCOMPUTE d = dur.\nCOMPUTE jd = joined.\nCOMPUTE q = joined.\nCOMPUTE my = joined.\n\
         COMPUTE wk = joined.\nCOMPUTE ymd = joined.\n\
         FORMATS m (MTIME11.2) d (DTIME13.2) jd (JDATE7) q (QYR8) my (MOYR8) wk (WKYR10) ymd (YMDHMS19).",
    );
    if comma {
        run(&mut session, "SET DECIMAL=COMMA.");
    }
    session
}

fn typed(
    session: &Session,
    name: &str,
    case: usize,
    text: &str,
    comma: bool,
) -> Result<String, String> {
    edits::cell_edit(&data(session).variable(name).unwrap(), case, text, comma)
}

#[test]
fn a_typed_length_of_time_is_those_seconds() {
    // hh:mm:ss, hh:mm, a fraction as typed, a negative one, many hours, and plain seconds
    let clock = [
        ("01:02:03", 3723.0),
        ("00:10", 600.0),
        ("12:5", 43_500.0),
        ("-01:00:00", -3600.0),
        ("30:00:00", 108_000.0),
        ("01:02:03.5", 3723.5),
        ("00:00:00.1", 0.1),
        ("99:59:59.999", 359_999.999),
        (" 7:08:09 ", 25_689.0),
        ("3600", 3600.0),
        ("-0.25", -0.25),
    ];
    let minutes = [
        ("10:30", 630.0),
        ("90:15.25", 5415.25),
        ("01:02:03", 3723.0),
        ("-0:30", -30.0),
    ];
    let days = [
        ("1 02:03:04", 93_784.0),
        ("-2 00:00:00", -172_800.0),
        ("0 23:59:59.5", 86_399.5),
        ("2:30", 9000.0),
        ("12:34:56", 45_296.0),
    ];
    for comma in [false, true] {
        let mut session = with_times(comma);
        let before = table(&session);
        let shown = |s: &str| {
            if comma {
                s.replace('.', ",")
            } else {
                s.to_string()
            }
        };
        for (name, cases) in [("dur", &clock[..]), ("m", &minutes[..]), ("d", &days[..])] {
            for (i, (text, seconds)) in cases.iter().enumerate() {
                let case = i % 5;
                let syntax = typed(&session, name, case, &shown(text), comma).unwrap();
                let (_, after) = op(&mut session, &syntax);
                same(
                    &format!("comma {comma}: {text:?} into {name}"),
                    &syntax,
                    &after,
                    &set(&before, name, case, num(Some(*seconds))),
                );
            }
        }
    }
}

#[test]
fn a_length_of_time_that_is_not_one_is_refused() {
    let session = with_times(false);
    for (name, text) in [
        ("dur", "01:60:00"),
        ("dur", "01:02:60"),
        ("dur", "1:2:3:4"),
        ("dur", "a:b"),
        ("dur", "01:02:03."),
        ("dur", "01:02:03,5"),
        ("dur", "::"),
        ("dur", ":"),
        ("dur", "01::02"),
        ("dur", "1 02:03:04"),
        ("dur", "--1:00"),
        ("dur", "01:-2:00"),
        ("m", "10:60"),
        ("m", "1:10:60"),
        ("m", "1:60:00"),
        ("d", "1 24:00:00"),
        ("d", "x 01:00:00"),
        ("d", "99999999999999999999 00:00:00"),
    ] {
        assert!(
            typed(&session, name, 0, text, false).is_err(),
            "{name} {text:?}"
        );
    }
    let comma = with_times(true);
    assert!(typed(&comma, "dur", 0, "01:02:03.5", true).is_err());
    assert!(typed(&comma, "dur", 0, "01:02:03,5", true).is_ok());
}

#[test]
fn a_length_of_time_is_edited_as_it_is_shown_and_keeps_every_bit() {
    // values that need care: fractions, negatives, more than a day, zero, large
    let values = [
        0.1,
        3723.1,
        86_399.999,
        1e10,
        -7261.25,
        0.0,
        59.9999,
        359_999.5,
        -0.5,
        123_456_789.25,
    ];
    for comma in [false, true] {
        let mut session = with_times(comma);
        for (i, v) in values.iter().enumerate() {
            let case = i % 5;
            run(
                &mut session,
                &format!(
                    "IF ($CASENUM = {}) dur = {v}.\nIF ($CASENUM = {}) m = {v}.\nIF ($CASENUM = {}) d = {v}.",
                    case + 1,
                    case + 1,
                    case + 1
                ),
            );
            let before = table(&session);
            let mut syntax = String::new();
            let data = data(&session);
            for name in ["dur", "m", "d"] {
                let var = data.variable(name).unwrap();
                for c in 0..data.case_count() {
                    let text = edits::edit_text(&var, c, comma);
                    syntax.push_str(&edits::cell_edit(&var, c, &text, comma).unwrap());
                    syntax.push('\n');
                }
            }
            let (_, after) = op(&mut session, &syntax);
            same(
                &format!("comma {comma}: retyped times after {v}"),
                &syntax,
                &after,
                &before,
            );
        }
    }
    // the text shown
    let session = with_times(false);
    let v = |n: &str| data(&session).variable(n).unwrap();
    assert_eq!(edits::edit_text(&v("dur"), 0, false), "01:02:03");
    assert_eq!(edits::edit_text(&v("m"), 0, false), "62:03");
    assert_eq!(edits::edit_text(&v("d"), 0, false), "0 01:02:03");
    assert_eq!(edits::edit_text(&v("dur"), 1, false), "00:10:00");
    assert_eq!(edits::edit_text(&v("dur"), 3, false), "00:00:01");
    assert_eq!(edits::edit_text(&v("dur"), 4, false), "12:00:00");
}

#[test]
fn the_date_formats_that_show_a_part_of_a_date_take_a_whole_date() {
    let texts = [
        ("2024-02-29", seconds(2024, 2, 29, 0)),
        ("1999-12-31", seconds(1999, 12, 31, 0)),
        ("9999-12-31", seconds(9999, 12, 31, 0)),
    ];
    let mut session = with_times(false);
    let before = table(&session);
    for name in ["jd", "q", "my", "wk"] {
        for (i, (text, value)) in texts.iter().enumerate() {
            let syntax = typed(&session, name, i, text, false).unwrap();
            let (_, after) = op(&mut session, &syntax);
            same(
                &format!("{text:?} into {name}"),
                &syntax,
                &after,
                &set(&before, name, i, num(Some(*value))),
            );
        }
        // the editor starts with the whole date
        let var = data(&session).variable(name).unwrap();
        assert_eq!(edits::edit_text(&var, 0, false), "2020-03-15", "{name}");
    }
    let syntax = typed(&session, "ymd", 1, "2024-02-29 10:30:15", false).unwrap();
    let (_, after) = op(&mut session, &syntax);
    same(
        "ymd",
        &syntax,
        &after,
        &set(&before, "ymd", 1, num(Some(seconds(2024, 2, 29, 37_815)))),
    );
    let var = data(&session).variable("ymd").unwrap();
    assert_eq!(edits::edit_text(&var, 0, false), "2020-03-15 00:00:00");
}

fn comma_field(
    session: &Session,
    name: &str,
    field: Field,
    text: &str,
) -> Result<Option<String>, String> {
    edits::field_edit(&data(session).variable(name).unwrap(), field, text, true)
}

fn comma_text(session: &Session, name: &str, field: Field) -> String {
    edits::field_text(&data(session).variable(name).unwrap(), field, true)
}

#[test]
fn the_variables_tab_shows_and_takes_a_decimal_comma_in_labels_and_missing_values() {
    // value labels pair a code with a label by "=", pairs by ";"; missing values go by ";"
    let cases = [
        (
            "income",
            Field::Values,
            "1,5 = Low; 2,25 = Mid; -0,125 = Below",
        ),
        ("income", Field::Values, "7 = Seven"),
        ("income", Field::Missing, "-99; 98,5"),
        ("income", Field::Missing, "LO THRU -1,5; 999,25"),
        ("income", Field::Missing, "0,5 THRU HI"),
        ("income", Field::Missing, "12"),
        ("age", Field::Missing, "98; 99,5; 100"),
        ("name", Field::Missing, "x; y"),
    ];
    for state in [false, true] {
        let mut session = comma_session(state);
        for (name, field, text) in cases {
            let syntax = comma_field(&session, name, field, text)
                .unwrap_or_else(|e| panic!("{name} {field:?} {text:?}: {e}"))
                .unwrap_or_else(|| panic!("{name} {field:?} {text:?}: no change"));
            let (before, after) = op(&mut session, &syntax);
            // the fields read back as typed, with the comma, and nothing else changed
            run(&mut session, &syntax);
            assert_eq!(
                comma_text(&session, name, field),
                text,
                "{state}: {name} {field:?}"
            );
            // typing what is shown is no change
            assert_eq!(comma_field(&session, name, field, text).unwrap(), None);
            assert!(session.undo());
            let at = before.at(name);
            for (i, (a, b)) in after.cols.iter().zip(&before.cols).enumerate() {
                if i != at {
                    assert!(a == b, "{state}: {name} {field:?} changed {}", a.fields[0]);
                }
            }
            assert_eq!(after.cols[at].cells, before.cols[at].cells);
        }
    }
}

#[test]
fn the_exact_numbers_of_labels_and_missing_values_reach_the_dictionary() {
    let mut session = comma_session(false);
    let syntax = comma_field(&session, "income", Field::Values, "1,5 = Low; 0,1 = Tenth")
        .unwrap()
        .unwrap();
    run(&mut session, &syntax);
    let variable = data(&session).variable("income").unwrap();
    let codes: Vec<f64> = variable
        .value_labels()
        .iter()
        .map(|(v, _)| match v {
            statscraft_data::Value::Number(n) => *n,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(codes, [1.5, 0.1]);
    let syntax = comma_field(&session, "income", Field::Missing, "LO THRU 0,1; 1234,5")
        .unwrap()
        .unwrap();
    run(&mut session, &syntax);
    let variable = data(&session).variable("income").unwrap();
    assert_eq!(
        *variable.missing_values(),
        statscraft_data::MissingValues::Range {
            low: f64::MIN,
            high: 0.1,
            extra: Some(1234.5)
        }
    );
}

#[test]
fn under_a_decimal_comma_a_period_in_labels_or_missing_values_is_refused() {
    let session = comma_session(false);
    for (field, text) in [
        (Field::Values, "1.5 = Low"),
        (Field::Missing, "1.5"),
        (Field::Missing, "LO THRU 0.5"),
        (Field::Missing, "1,5, 2,5"),
        (Field::Missing, "1,5.2"),
    ] {
        match comma_field(&session, "income", field, text) {
            Err(message) => assert!(!message.is_empty()),
            Ok(Some(syntax)) => {
                // the engine takes it only if it means the same as the text says
                let mut s = comma_session(false);
                assert!(
                    s.run(&syntax, None).is_err(),
                    "{field:?} {text:?} gave {syntax}"
                );
            }
            Ok(None) => panic!("{field:?} {text:?}"),
        }
    }
    // with the setting off a comma list and a period are as before
    let plain_session = plain();
    let v = data(&plain_session).variable("income").unwrap();
    assert_eq!(edits::field_text(&v, Field::Missing, false), "LO THRU -1");
    assert!(edits::field_edit(&v, Field::Missing, "1.5, 2.5", false).is_ok());
    assert!(edits::field_edit(&v, Field::Values, "1,5 = Low", false).is_err());
}

#[test]
fn the_dialogs_show_and_take_a_decimal_comma() {
    for state in [false, true] {
        let mut session = comma_session(state);
        // value labels: the list, then pasted lines separated by a semicolon
        let mut d = LabelsEditor::of(&data(&session).variable("income").unwrap(), vec![], true);
        d.pairs = vec![("1,5".into(), "Low".into()), ("2,25".into(), "Mid".into())];
        let (_, after) = op(&mut session, &d.syntax().unwrap());
        assert_eq!(
            after.cols[after.at("income")].fields[6],
            "1.5 = Low; 2.25 = Mid"
        );
        run(&mut session, &d.syntax().unwrap());
        assert_eq!(
            comma_text(&session, "income", Field::Values),
            "1,5 = Low; 2,25 = Mid"
        );
        let mut again = LabelsEditor::of(&data(&session).variable("income").unwrap(), vec![], true);
        assert_eq!(again.pairs[0], ("1,5".to_string(), "Low".to_string()));
        again.quick = true;
        again.pasted = "3,5;High\n4,75;Top".into();
        let syntax = again.syntax().unwrap();
        run(&mut session, &syntax);
        assert_eq!(
            comma_text(&session, "income", Field::Values),
            "1,5 = Low; 2,25 = Mid; 3,5 = High; 4,75 = Top"
        );
        // a comma alone separates nothing under a decimal comma
        again.pasted = "1,5,Low".into();
        assert!(again.syntax().is_err());
        again.quick = false;
        again.pairs[0].0 = "1.5".into();
        assert!(again.syntax().is_err());
        // missing values: three values, or a range and one more
        let mut m = MissingEditor::of(&data(&session).variable("income").unwrap(), vec![], true);
        m.range = false;
        m.values = ["-1,5".into(), "99,5".into(), String::new()];
        run(&mut session, &m.syntax().unwrap());
        assert_eq!(comma_text(&session, "income", Field::Missing), "-1,5; 99,5");
        let mut m = MissingEditor::of(&data(&session).variable("income").unwrap(), vec![], true);
        assert_eq!(m.values[0], "-1,5");
        m.range = true;
        m.low = "LO".into();
        m.high = "0,5".into();
        m.extra = "999,25".into();
        run(&mut session, &m.syntax().unwrap());
        assert_eq!(
            comma_text(&session, "income", Field::Missing),
            "LO THRU 0,5; 999,25"
        );
        let m = MissingEditor::of(&data(&session).variable("income").unwrap(), vec![], true);
        assert_eq!(
            (m.low.as_str(), m.high.as_str(), m.extra.as_str()),
            ("LO", "0,5", "999,25")
        );
        let mut bad = m.clone();
        bad.high = "0.5".into();
        assert!(bad.syntax().is_err());
    }
}
