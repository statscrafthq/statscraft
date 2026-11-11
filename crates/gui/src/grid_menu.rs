//! The right-click menus of the Data View and the Variable View: what a menu offers for what was
//! clicked, and the syntax or the prefilled dialog each item becomes.

use crate::analyze_dialogs::Explore;
use crate::app::Dialog;
use crate::data_dialogs::{FILTER_VARIABLE, SelectCases, SelectMode, SortCases};
use crate::dialogs::{Crosstabs, CustomTables, Descriptives, Frequencies, Recode};
use crate::edits::{self, Field, listed};
use crate::syntax_text::{number, quote};
use statscraft_data::{Dataset, Measure, Values};
use statscraft_engine::Currencies;

/// What a right click landed on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Variables by their index, in file order: Data View columns or Variable View rows.
    Variables(Vec<usize>),
    /// Data View cases by their index, from row numbers.
    Cases(Vec<usize>),
    /// A Data View cell.
    Cell { case: usize, column: usize },
    /// A Variable View cell: the variable of its row and the field of its column.
    Field { variable: usize, field: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    SortAscending,
    SortDescending,
    Frequencies,
    Descriptives,
    Information,
    FrequenciesDialog,
    DescriptivesDialog,
    ExploreDialog,
    CrosstabsDialog,
    CustomTablesDialog,
    Measure(Measure),
    Rename,
    Properties,
    ShowInData,
    RecodeDialog,
    InsertBefore,
    Copy,
    Delete,
    DeleteCase,
    Edit,
    ClearValue,
    Filter,
    FilterOff,
    KeepOnlyDialog,
}

/// One item of a menu.
#[derive(Debug)]
pub struct Entry {
    pub item: Item,
    pub label: String,
    pub enabled: bool,
    /// Why the item is off, or what it leaves out.
    pub hover: Option<String>,
    /// For a choice of one of several: whether it is the current one.
    pub checked: Option<bool>,
    /// The submenu the item sits in.
    pub submenu: Option<&'static str>,
    pub shortcut: Option<&'static str>,
}

fn entry(item: Item, label: &str) -> Entry {
    Entry {
        item,
        label: label.to_string(),
        enabled: true,
        hover: None,
        checked: None,
        submenu: None,
        shortcut: None,
    }
}

impl Entry {
    /// Off, with the reason on hover, when `ready` is an error.
    fn ready(mut self, ready: &Result<(), String>) -> Self {
        if let Err(why) = ready {
            self.enabled = false;
            self.hover = Some(why.clone());
        }
        self
    }

    fn under(mut self, submenu: &'static str) -> Self {
        self.submenu = Some(submenu);
        self
    }

    fn key(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
}

/// A question asked before an action runs: what it will do, the syntax, and the button that
/// runs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Confirm {
    pub question: String,
    pub syntax: String,
    pub button: &'static str,
}

/// What choosing an item does.
pub enum Action {
    Run(String),
    Ask(Confirm),
    Open(Box<Dialog>),
    /// Ctrl+C: the selection to the clipboard.
    Copy,
    /// Start editing a Data View cell: (case, column).
    EditCell(usize, usize),
    /// Start editing a Variable View field: (variable, field).
    EditField(usize, usize),
    /// The variable's row in the Variable View.
    Properties(usize),
    /// The variable's column in the Data View.
    ShowInData(usize),
}

fn dialog(dialog: Dialog) -> Action {
    Action::Open(Box::new(dialog))
}

const MORE: &str = "More analyses";
const MEASURE: &str = "Measure";
pub const MEASURES: [(Measure, &str); 3] = [
    (Measure::Nominal, "Nominal"),
    (Measure::Ordinal, "Ordinal"),
    (Measure::Scale, "Scale"),
];

/// The variables a target concerns that the dataset still has, in file order.
fn columns(data: &Dataset, target: &Target) -> Vec<usize> {
    let mut columns = match target {
        Target::Variables(columns) => columns.clone(),
        Target::Cases(_) => return Vec::new(),
        Target::Cell { column, .. } => vec![*column],
        Target::Field { variable, .. } => vec![*variable],
    };
    columns.retain(|&c| c < data.variable_count());
    columns.sort_unstable();
    columns.dedup();
    columns
}

fn name(data: &Dataset, column: usize) -> String {
    data.variable_at(column).name().to_string()
}

fn numeric(data: &Dataset, columns: &[usize]) -> Vec<usize> {
    let mut numeric = columns.to_vec();
    numeric.retain(|&c| !data.variable_at(c).is_string());
    numeric
}

/// A variable list for syntax, in file order: a run of three or more neighbouring variables is
/// written `a TO c`, so a wide selection stays short. `columns` must be sorted.
pub fn varlist(data: &Dataset, columns: &[usize]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < columns.len() {
        let mut j = i;
        while j + 1 < columns.len() && columns[j + 1] == columns[j] + 1 {
            j += 1;
        }
        if j - i >= 2 {
            parts.push(format!(
                "{} TO {}",
                name(data, columns[i]),
                name(data, columns[j])
            ));
        } else {
            parts.extend(columns[i..=j].iter().map(|&c| name(data, c)));
        }
        i = j + 1;
    }
    parts.join(" ")
}

/// `text` cut to about `most` characters.
fn short(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((at, _)) => format!("{}...", &text[..at]),
        None => text.to_string(),
    }
}

/// The first line of a menu: what it acts on.
pub fn heading(data: &Dataset, target: &Target) -> String {
    if let Target::Cases(rows) = target {
        return match rows[..] {
            [one] => format!("Case {}", one + 1),
            _ => format!("{} cases", rows.len()),
        };
    }
    if let Target::Cell { case, column } = *target
        && column < data.variable_count()
    {
        return format!("{}, case {}", name(data, column), case + 1);
    }
    match columns(data, target)[..] {
        [one] => {
            let variable = data.variable_at(one);
            match variable.label() {
                Some(label) => format!("{}: {}", variable.name(), short(label, 48)),
                None => variable.name().to_string(),
            }
        }
        ref several => format!("{} variables", several.len()),
    }
}

/// The items of the menu over `target`. None stands for a separator. `variable_view` tells
/// which grid was clicked, `busy` that a run is going on.
pub fn entries(
    data: &Dataset,
    target: &Target,
    variable_view: bool,
    busy: bool,
) -> Vec<Option<Entry>> {
    let runs: Result<(), String> = match busy {
        true => Err("wait for the running command to finish".into()),
        false => Ok(()),
    };
    let cases = match data.case_count() {
        0 => Err("no cases".into()),
        _ => runs.clone(),
    };
    if let Target::Cases(rows) = target {
        let one = match rows[..] {
            [_] => runs.clone(),
            _ => Err("one case at a time".into()),
        };
        return vec![
            Some(entry(Item::Copy, "Copy").key("Ctrl+C")),
            Some(entry(Item::DeleteCase, "Delete case").ready(&one)),
        ];
    }
    if let Target::Cell { case, column } = *target {
        if case >= data.case_count() || column >= data.variable_count() {
            return Vec::new();
        }
        return cell_entries(data, case, column, &runs);
    }
    let columns = columns(data, target);
    if columns.is_empty() {
        return Vec::new();
    }
    if let Target::Field { variable, .. } = *target {
        let mut list = vec![Some(entry(Item::Edit, "Edit").key("Enter")), None];
        list.extend(entries(
            data,
            &Target::Variables(vec![variable]),
            true,
            busy,
        ));
        return list;
    }
    let one = columns.len() == 1;
    let numeric = numeric(data, &columns);
    let strings: Vec<String> = columns
        .iter()
        .filter(|c| !numeric.contains(c))
        .map(|&c| name(data, c))
        .collect();
    let (ascending, descending) = match one {
        true => ("Sort ascending", "Sort descending"),
        false => ("Sort ascending by these", "Sort descending by these"),
    };
    let mut descriptives = entry(Item::Descriptives, "Descriptives").ready(&cases);
    let no_mean = match (numeric.is_empty(), one) {
        (false, _) => Ok(()),
        (true, true) => Err("a string variable has no mean".to_string()),
        (true, false) => Err("string variables have no mean".to_string()),
    };
    descriptives = descriptives.ready(&no_mean);
    if descriptives.enabled && !strings.is_empty() {
        descriptives.hover = Some(format!("leaves out {}", listed(&strings)));
    }
    let mut list = vec![
        Some(entry(Item::SortAscending, ascending).ready(&cases)),
        Some(entry(Item::SortDescending, descending).ready(&cases)),
        None,
        Some(entry(Item::Frequencies, "Frequencies").ready(&cases)),
        Some(descriptives),
        Some(entry(Item::Information, "Variable information").ready(&runs)),
        Some(entry(Item::FrequenciesDialog, "Frequencies...").under(MORE)),
        Some(
            entry(Item::DescriptivesDialog, "Descriptives...")
                .ready(&no_mean)
                .under(MORE),
        ),
        Some(
            entry(Item::ExploreDialog, "Explore...")
                .ready(&no_mean)
                .under(MORE),
        ),
        Some(entry(Item::CrosstabsDialog, "Crosstabs...").under(MORE)),
        Some(entry(Item::CustomTablesDialog, "Custom Tables...").under(MORE)),
    ];
    for (measure, label) in MEASURES {
        let mut choice = entry(Item::Measure(measure), label)
            .ready(&runs)
            .under(MEASURE);
        choice.checked = Some(
            columns
                .iter()
                .all(|&c| data.variable_at(c).measure() == measure),
        );
        list.push(Some(choice));
    }
    list.push(None);
    if one {
        list.push(Some(entry(Item::Rename, "Rename")));
        list.push(Some(match variable_view {
            true => entry(Item::ShowInData, "Show in Data View"),
            false => entry(Item::Properties, "Properties"),
        }));
        list.push(Some(entry(Item::RecodeDialog, "Recode...")));
        list.push(Some(
            entry(Item::InsertBefore, "Insert variable before").ready(&runs),
        ));
        list.push(None);
    }
    list.push(Some(entry(Item::Copy, "Copy").key("Ctrl+C")));
    let delete = if one {
        "Delete variable"
    } else {
        "Delete variables"
    };
    list.push(Some(entry(Item::Delete, delete).key("Del").ready(&runs)));
    list
}

/// The menu over a Data View cell: the cell itself, a filter on its value, sorting by its
/// variable.
fn cell_entries(
    data: &Dataset,
    case: usize,
    column: usize,
    runs: &Result<(), String>,
) -> Vec<Option<Entry>> {
    let variable = data.variable_at(column);
    let mut condition = short(&condition(data, case, column), 40);
    // only a value label differs from the value, so the currency settings do not matter here
    let plain = Currencies::default();
    let label = edits::cell_text(&variable, case, true, &plain);
    if label != edits::cell_text(&variable, case, false, &plain) {
        condition.push_str(&format!(" ({})", short(&label, 30)));
    }
    let mut filter = entry(Item::Filter, &format!("Only cases where {condition}")).ready(runs);
    let current = data.filter_variable();
    if let (true, Some(current)) = (filter.enabled, current) {
        filter.hover = Some(format!("replaces the filter {current}"));
    }
    let mut list = vec![
        Some(entry(Item::Copy, "Copy").key("Ctrl+C")),
        Some(entry(Item::Edit, "Edit").key("Enter")),
        Some(entry(Item::ClearValue, "Clear value").ready(runs)),
        None,
        Some(filter),
    ];
    if current.is_some() {
        list.push(Some(entry(Item::FilterOff, "Stop filtering").ready(runs)));
    }
    let keep = format!("Keep only cases where {condition}...");
    list.push(Some(entry(Item::KeepOnlyDialog, &keep)));
    list.push(None);
    let name = variable.name();
    let ascending = format!("Sort ascending by {name}");
    let descending = format!("Sort descending by {name}");
    list.push(Some(entry(Item::SortAscending, &ascending).ready(runs)));
    list.push(Some(entry(Item::SortDescending, &descending).ready(runs)));
    list
}

/// The condition that a case holds the value of the cell: the stored value, not the one shown,
/// so a number shown rounded still matches. A numeric user-missing value is compared through
/// VALUE, as the comparison of a missing value is missing and would keep no case; system-missing
/// is SYSMIS. String user-missing values compare as valid ones.
pub fn condition(data: &Dataset, case: usize, column: usize) -> String {
    let variable = data.variable_at(column);
    let name = variable.name();
    match variable.values() {
        Values::String(values) => format!("{name} = {}", quote(values.get(case).trim_end())),
        Values::Numeric(values) => match values.get(case) {
            None => format!("SYSMIS({name})"),
            Some(v) if variable.is_user_missing(v) => format!("VALUE({name}) = {}", number(v)),
            Some(v) => format!("{name} = {}", number(v)),
        },
    }
}

/// The variable a new filter goes into: filter_$, unless a variable of that name holds other
/// data than the filter in force; then the first free of filter_$_2, filter_$_3, ...
fn filter_name(data: &Dataset) -> String {
    let current = data.filter_variable();
    std::iter::once(FILTER_VARIABLE.to_string())
        .chain((2..).map(|i| format!("{FILTER_VARIABLE}_{i}")))
        .find(|n| {
            data.variable_index(n).is_none() || current.is_some_and(|c| c.eq_ignore_ascii_case(n))
        })
        .expect("one of the names is free")
}

/// A filter that leaves out the cases where the variable of the cell holds another value. It
/// replaces the filter in force.
pub fn filter(data: &Dataset, case: usize, column: usize) -> String {
    let condition = condition(data, case, column);
    let name = filter_name(data);
    format!(
        "COMPUTE {name} = ({condition}).\nVARIABLE LABELS {name} {}.\nFILTER BY {name}.",
        quote(&short(&condition, 120))
    )
}

/// What choosing `item` in the menu over `target` does.
pub fn action(data: &Dataset, target: &Target, item: Item) -> Result<Action, String> {
    if let Target::Cases(rows) = target {
        return Ok(match (item, &rows[..]) {
            (Item::DeleteCase, &[case]) => Action::Ask(delete_case(case)),
            _ => Action::Copy,
        });
    }
    if let Target::Cell { case, column } = *target {
        if case >= data.case_count() || column >= data.variable_count() {
            return Err("the case is gone".into());
        }
        let variable = data.variable_at(column);
        match item {
            Item::Edit => return Ok(Action::EditCell(case, column)),
            Item::ClearValue => return Ok(Action::Run(edits::cell_edit(&variable, case, "")?)),
            Item::Filter => return Ok(Action::Run(filter(data, case, column))),
            Item::FilterOff => return Ok(Action::Run("FILTER OFF.".into())),
            Item::KeepOnlyDialog => {
                return Ok(dialog(Dialog::SelectCases(SelectCases {
                    mode: SelectMode::Delete,
                    condition: condition(data, case, column),
                })));
            }
            _ => {}
        }
    }
    if let (Target::Field { variable, field }, Item::Edit) = (target, item)
        && *variable < data.variable_count()
    {
        return Ok(Action::EditField(*variable, *field));
    }
    let columns = columns(data, target);
    let first = *columns.first().ok_or("the variable is gone")?;
    let all = varlist(data, &columns);
    let names: Vec<String> = columns.iter().map(|&c| name(data, c)).collect();
    let numeric = numeric(data, &columns);
    let numeric_names: Vec<String> = numeric.iter().map(|&c| name(data, c)).collect();
    Ok(match item {
        Item::SortAscending | Item::SortDescending => {
            let descending = item == Item::SortDescending;
            let syntax = SortCases {
                keys: vec![(all, descending)],
            }
            .syntax()?;
            match data.split_file() {
                Some(split) => Action::Ask(Confirm {
                    question: format!(
                        "Sorting the cases ends the split by {}.",
                        split.variables.join(" ")
                    ),
                    syntax,
                    button: "Sort",
                }),
                None => Action::Run(syntax),
            }
        }
        Item::Frequencies => {
            let syntax = Frequencies {
                variables: vec![all],
                ..Default::default()
            }
            .syntax()?;
            match columns.len() {
                n if n > 100 => Action::Ask(Confirm {
                    question: format!("Frequencies makes a table for each of the {n} variables."),
                    syntax,
                    button: "Run",
                }),
                _ => Action::Run(syntax),
            }
        }
        Item::Descriptives => Action::Run(
            Descriptives {
                variables: vec![varlist(data, &numeric)],
                ..Default::default()
            }
            .syntax()?,
        ),
        Item::Information => Action::Run(format!("DISPLAY DICTIONARY /VARIABLES={all}.")),
        Item::FrequenciesDialog => dialog(Dialog::Frequencies(Frequencies {
            variables: names,
            ..Default::default()
        })),
        Item::DescriptivesDialog => dialog(Dialog::Descriptives(Descriptives {
            variables: numeric_names,
            ..Default::default()
        })),
        Item::ExploreDialog => dialog(Dialog::Explore(Explore {
            dependents: numeric_names,
            ..Default::default()
        })),
        Item::CrosstabsDialog => {
            let mut d = Crosstabs::default();
            match &names[..] {
                [row, column] => (d.rows, d.columns) = (vec![row.clone()], vec![column.clone()]),
                _ => d.rows = names,
            }
            dialog(Dialog::Crosstabs(d))
        }
        Item::CustomTablesDialog => {
            let mut d = CustomTables::default();
            for &c in &columns {
                let variable = data.variable_at(c);
                let scale = !variable.is_string() && variable.measure() == Measure::Scale;
                let list = if scale {
                    &mut d.scale_rows
                } else {
                    &mut d.rows
                };
                list.push(variable.name().to_string());
            }
            dialog(Dialog::CustomTables(d, Default::default()))
        }
        Item::Measure(measure) => Action::Run(format!(
            "VARIABLE LEVEL {all} ({}).",
            measure.as_str().to_ascii_uppercase()
        )),
        Item::Rename => Action::EditField(first, Field::Name as usize),
        Item::Properties => Action::Properties(first),
        Item::ShowInData => Action::ShowInData(first),
        Item::RecodeDialog => dialog(Dialog::Recode(
            Recode {
                variables: names,
                ..Default::default()
            },
            Default::default(),
        )),
        Item::InsertBefore => Action::Run(insert_before(data, first)),
        Item::Copy => Action::Copy,
        Item::Delete => {
            let mut question = match names.len() {
                1 => format!("Delete {}? Ctrl+Z brings it back.", names[0]),
                _ => format!("Delete {}? Ctrl+Z brings them back.", listed(&names)),
            };
            let among = |name: Option<&str>| {
                name.is_some_and(|name| names.iter().any(|n| n.eq_ignore_ascii_case(name)))
            };
            if among(data.weight_variable()) {
                question.push_str(" This also ends weighting.");
            }
            if among(data.filter_variable()) {
                question.push_str(" This also ends the filter.");
            }
            Action::Ask(Confirm {
                question,
                syntax: format!("DELETE VARIABLES {all}."),
                button: "Delete",
            })
        }
        Item::DeleteCase
        | Item::Edit
        | Item::ClearValue
        | Item::Filter
        | Item::FilterOff
        | Item::KeepOnlyDialog => return Err("this item belongs to another menu".into()),
    })
}

/// A new numeric variable before the variable at `before`, named new_1 or the first free of
/// new_2, new_3, ... MATCH FILES puts it in place and drops the weight, so the weight is set
/// again.
pub fn insert_before(data: &Dataset, before: usize) -> String {
    let new = (1..)
        .map(|i| format!("new_{i}"))
        .find(|n| data.variable_index(n).is_none())
        .expect("one of the names is free");
    let earlier: Vec<usize> = (0..before).collect();
    let keep = match before {
        0 => new.clone(),
        _ => format!("{} {new}", varlist(data, &earlier)),
    };
    let mut syntax = format!("NUMERIC {new} (F8.2).\nMATCH FILES /FILE=* /KEEP={keep} ALL.");
    if let Some(weight) = data.weight_variable() {
        syntax.push_str(&format!("\nWEIGHT BY {weight}."));
    }
    syntax
}

/// Deletes the case at `case` (from 0). $CASENUM cannot be used in SELECT IF, so a scratch
/// variable counts the cases; it sees every case, filtered or not.
fn delete_case(case: usize) -> Confirm {
    let n = case + 1;
    Confirm {
        question: format!("Delete case {n}? Ctrl+Z brings it back."),
        syntax: format!("COMPUTE #row = #row + 1.\nSELECT IF #row <> {n}."),
        button: "Delete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};
    use statscraft_engine::session::Session;

    fn data(session: &Session) -> &Dataset {
        session.dataset().unwrap()
    }

    fn index(session: &Session, name: &str) -> usize {
        data(session).variable_index(name).unwrap()
    }

    fn target(session: &Session, names: &[&str]) -> Target {
        Target::Variables(names.iter().map(|n| index(session, n)).collect())
    }

    /// The item's entry in the menu over `names` of the Data View.
    fn find(session: &Session, names: &[&str], item: Item) -> Option<Entry> {
        let entries = entries(data(session), &target(session, names), false, false);
        entries.into_iter().flatten().find(|e| e.item == item)
    }

    fn syntax(session: &Session, names: &[&str], item: Item) -> String {
        match action(data(session), &target(session, names), item).unwrap() {
            Action::Run(syntax) => syntax,
            Action::Ask(confirm) => confirm.syntax,
            _ => panic!("{item:?} runs no syntax"),
        }
    }

    /// The values of a numeric variable, system-missing as None.
    fn numbers(session: &Session, name: &str) -> Vec<Option<f64>> {
        let variable = data(session).variable(name).unwrap();
        variable.numeric().unwrap().iter().collect()
    }

    #[test]
    fn sort_by_one_column_both_ways() {
        let mut session = open("survey_basic.sav");
        let up = syntax(&session, &["age"], Item::SortAscending);
        assert_eq!(up, "SORT CASES BY age.");
        run_in(&mut session, &up);
        let ages = numbers(&session, "age");
        assert!(ages.windows(2).all(|w| w[0] <= w[1]));
        let down = syntax(&session, &["age"], Item::SortDescending);
        assert_eq!(down, "SORT CASES BY age (D).");
        run_in(&mut session, &down);
        let ages = numbers(&session, "age");
        assert!(ages.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn sort_by_several_columns_in_file_order() {
        let mut session = open("survey_basic.sav");
        // clicked as region, then gender: the syntax follows the file
        let up = syntax(&session, &["region", "gender"], Item::SortAscending);
        assert_eq!(up, "SORT CASES BY gender region.");
        let entry = find(&session, &["region", "gender"], Item::SortAscending).unwrap();
        assert_eq!(entry.label, "Sort ascending by these");
        run_in(&mut session, &up);
        let pairs: Vec<_> = numbers(&session, "gender")
            .into_iter()
            .zip(numbers(&session, "region"))
            .collect();
        assert!(pairs.windows(2).all(|w| w[0] <= w[1]));
        let down = syntax(&session, &["region", "gender"], Item::SortDescending);
        assert_eq!(down, "SORT CASES BY gender region (D).");
        run_in(&mut session, &down);
    }

    #[test]
    fn sort_asks_first_when_split_is_on() {
        let mut session = open("survey_basic.sav");
        run_in(&mut session, "SORT CASES BY region.\nSPLIT FILE BY region.");
        let Action::Ask(confirm) = action(
            data(&session),
            &target(&session, &["age"]),
            Item::SortAscending,
        )
        .unwrap() else {
            panic!("sorting with a split asks first");
        };
        assert_eq!(
            confirm.question,
            "Sorting the cases ends the split by region."
        );
        assert_eq!(confirm.syntax, "SORT CASES BY age.");
        run_in(&mut session, &confirm.syntax);
        assert!(data(&session).split_file().is_none());
    }

    #[test]
    fn frequencies_is_the_dialog_default() {
        let session = open("survey_basic.sav");
        let quick = syntax(&session, &["gender", "plan"], Item::Frequencies);
        let dialog = Frequencies {
            variables: v(&["gender plan"]),
            ..Default::default()
        };
        assert_eq!(quick, dialog.syntax().unwrap());
        assert_eq!(quick, "FREQUENCIES VARIABLES=gender plan.");
        // one frequency table per variable
        assert_eq!(run_in(&mut open("survey_basic.sav"), &quick), 2);
    }

    #[test]
    fn descriptives_leaves_out_strings() {
        let session = open("survey_basic.sav");
        let quick = syntax(
            &session,
            &["age", "plan", "monthly_spend"],
            Item::Descriptives,
        );
        assert_eq!(
            quick,
            "DESCRIPTIVES VARIABLES=age monthly_spend\n  /STATISTICS=MEAN STDDEV MINIMUM MAXIMUM."
        );
        let entry = find(
            &session,
            &["age", "plan", "monthly_spend"],
            Item::Descriptives,
        );
        let entry = entry.unwrap();
        assert!(entry.enabled);
        assert_eq!(entry.hover.as_deref(), Some("leaves out plan"));
        assert_eq!(run_in(&mut open("survey_basic.sav"), &quick), 1);
    }

    #[test]
    fn descriptives_is_off_for_a_string() {
        let session = open("survey_basic.sav");
        let entry = find(&session, &["plan"], Item::Descriptives).unwrap();
        assert!(!entry.enabled);
        assert_eq!(
            entry.hover.as_deref(),
            Some("a string variable has no mean")
        );
        let entry = find(&session, &["plan", "q3_comment"], Item::Descriptives).unwrap();
        assert_eq!(
            entry.hover.as_deref(),
            Some("string variables have no mean")
        );
        assert!(
            !find(&session, &["plan"], Item::ExploreDialog)
                .unwrap()
                .enabled
        );
        // the other analyses take strings
        assert!(
            find(&session, &["plan"], Item::Frequencies)
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn variable_information_shows_the_dictionary() {
        let mut session = open("survey_basic.sav");
        let quick = syntax(&session, &["gender", "age"], Item::Information);
        assert_eq!(quick, "DISPLAY DICTIONARY /VARIABLES=gender age.");
        // the variables and the value labels of gender
        assert_eq!(run_in(&mut session, &quick), 2);
    }

    #[test]
    fn measure_for_several_variables() {
        let mut session = open("survey_basic.sav");
        let names = ["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"];
        let nominal = find(&session, &names, Item::Measure(Measure::Nominal)).unwrap();
        assert_eq!(
            (nominal.checked, nominal.submenu),
            (Some(true), Some(MEASURE))
        );
        let quick = syntax(&session, &names, Item::Measure(Measure::Ordinal));
        assert_eq!(quick, "VARIABLE LEVEL q5_1 TO q5_5 (ORDINAL).");
        run_in(&mut session, &quick);
        for name in names {
            let measure = data(&session).variable(name).unwrap().measure();
            assert_eq!(measure, Measure::Ordinal);
        }
        let ordinal = find(&session, &names, Item::Measure(Measure::Ordinal)).unwrap();
        assert_eq!(ordinal.checked, Some(true));
        // a mix has none checked
        let mixed = find(&session, &["age", "q5_1"], Item::Measure(Measure::Scale)).unwrap();
        assert_eq!(mixed.checked, Some(false));
    }

    #[test]
    fn a_long_selection_is_written_with_to() {
        let session = open("survey_basic.sav");
        let data = data(&session);
        // resp_id gender age region, q5_1 q5_2, q6_mention1 to q6_mention3
        let columns = [0, 1, 2, 3, 10, 11, 15, 16, 17];
        assert_eq!(
            varlist(data, &columns),
            "resp_id TO region q5_1 q5_2 q6_mention1 TO q6_mention3"
        );
        let target = Target::Variables(columns.to_vec());
        for item in [Item::SortAscending, Item::Frequencies, Item::Information] {
            let Ok(Action::Run(syntax)) = action(data, &target, item) else {
                panic!("{item:?} runs at once");
            };
            assert!(syntax.contains("resp_id TO region q5_1 q5_2 q6_mention1 TO q6_mention3"));
            run_in(&mut open("survey_basic.sav"), &syntax);
        }
        // Descriptives leaves out no string here, so the runs stay whole
        let Ok(Action::Run(syntax)) = action(data, &target, Item::Descriptives) else {
            panic!("Descriptives runs at once");
        };
        assert!(syntax.starts_with("DESCRIPTIVES VARIABLES=resp_id TO region q5_1 q5_2"));
        // a string inside a run breaks it: age monthly_spend plan signup_date
        assert_eq!(varlist(data, &[2]), "age");
        let mixed = Target::Variables(vec![5, 6, 7, 8, 9]);
        let Ok(Action::Run(syntax)) = action(data, &mixed, Item::Descriptives) else {
            panic!("Descriptives runs at once");
        };
        assert!(
            syntax.starts_with("DESCRIPTIVES VARIABLES=q2_recommend monthly_spend signup_date\n")
        );
        run_in(&mut open("survey_basic.sav"), &syntax);
    }

    #[test]
    fn analyses_are_off_without_cases() {
        let mut session = open("survey_basic.sav");
        run_in(&mut session, "SELECT IF resp_id < 0.");
        assert_eq!(data(&session).case_count(), 0);
        for item in [
            Item::SortAscending,
            Item::SortDescending,
            Item::Frequencies,
            Item::Descriptives,
        ] {
            let entry = find(&session, &["age"], item).unwrap();
            assert!(!entry.enabled, "{item:?}");
            assert_eq!(entry.hover.as_deref(), Some("no cases"));
        }
        // the dictionary stays at hand
        for item in [
            Item::Information,
            Item::Measure(Measure::Ordinal),
            Item::Rename,
        ] {
            assert!(find(&session, &["age"], item).unwrap().enabled, "{item:?}");
        }
        let quick = syntax(&session, &["age"], Item::Information);
        run_in(&mut session, &quick);
    }

    #[test]
    fn items_that_run_wait_for_a_run_going_on() {
        let session = open("survey_basic.sav");
        let target = target(&session, &["age"]);
        let entries = entries(data(&session), &target, false, true);
        let entries: Vec<Entry> = entries.into_iter().flatten().collect();
        let off = |item| {
            let entry = entries.iter().find(|e| e.item == item).unwrap();
            (!entry.enabled).then(|| entry.hover.clone().unwrap())
        };
        let wait = Some("wait for the running command to finish".to_string());
        assert_eq!(off(Item::SortAscending), wait);
        assert_eq!(off(Item::Information), wait);
        assert_eq!(off(Item::Measure(Measure::Scale)), wait);
        // dialogs and the clipboard do not run anything yet
        assert_eq!(off(Item::FrequenciesDialog), None);
        assert_eq!(off(Item::Copy), None);
    }

    #[test]
    fn crosstabs_of_two_columns_prefills_rows_and_columns() {
        let session = open("survey_basic.sav");
        let Ok(Action::Open(opened)) = action(
            data(&session),
            &target(&session, &["region", "gender"]),
            Item::CrosstabsDialog,
        ) else {
            panic!("Crosstabs opens its dialog");
        };
        let Dialog::Crosstabs(d) = *opened else {
            panic!("Crosstabs opens its dialog");
        };
        assert_eq!(
            (d.rows.clone(), d.columns.clone()),
            (v(&["gender"]), v(&["region"]))
        );
        run_in(&mut open("survey_basic.sav"), &d.syntax().unwrap());
    }

    #[test]
    fn the_other_dialogs_are_prefilled() {
        let session = open("survey_basic.sav");
        let open_with =
            |names: &[&str], item| match action(data(&session), &target(&session, names), item)
                .unwrap()
            {
                Action::Open(dialog) => *dialog,
                _ => panic!("{item:?} opens a dialog"),
            };
        let names = ["gender", "age", "plan"];
        let Dialog::Frequencies(d) = open_with(&names, Item::FrequenciesDialog) else {
            panic!()
        };
        assert_eq!(d.variables, v(&names));
        let Dialog::Descriptives(d) = open_with(&names, Item::DescriptivesDialog) else {
            panic!()
        };
        assert_eq!(d.variables, v(&["gender", "age"]));
        let Dialog::Explore(d) = open_with(&names, Item::ExploreDialog) else {
            panic!()
        };
        assert_eq!(d.dependents, v(&["gender", "age"]));
        // a scale variable goes to the scale rows of Custom Tables
        let Dialog::CustomTables(d, _) = open_with(&names, Item::CustomTablesDialog) else {
            panic!()
        };
        assert_eq!(
            (d.rows.clone(), d.scale_rows.clone()),
            (v(&["gender", "plan"]), v(&["age"]))
        );
        run_in(&mut open("survey_basic.sav"), &d.syntax().unwrap());
        let Dialog::Recode(d, _) = open_with(&["region"], Item::RecodeDialog) else {
            panic!()
        };
        assert_eq!(d.variables, v(&["region"]));
    }

    #[test]
    fn one_variable_has_rename_and_its_properties_several_do_not() {
        let session = open("survey_basic.sav");
        let data = data(&session);
        let age = index(&session, "age");
        let items = |target: &Target, variable_view| {
            let entries = entries(data, target, variable_view, false);
            entries
                .into_iter()
                .flatten()
                .map(|e| e.item)
                .collect::<Vec<_>>()
        };
        let one = items(&Target::Variables(vec![age]), false);
        assert!(one.contains(&Item::Rename) && one.contains(&Item::Properties));
        assert!(!one.contains(&Item::ShowInData));
        let several = items(&Target::Variables(vec![age, age + 1]), false);
        assert!(!several.contains(&Item::Rename) && !several.contains(&Item::RecodeDialog));
        let target = Target::Variables(vec![age]);
        assert!(matches!(
            action(data, &target, Item::Rename),
            Ok(Action::EditField(c, 0)) if c == age
        ));
        assert!(
            matches!(action(data, &target, Item::Properties), Ok(Action::Properties(c)) if c == age)
        );
        assert_eq!(heading(data, &target), "age: Age in years");
        assert_eq!(
            heading(data, &Target::Variables(vec![1, 2, 3])),
            "3 variables"
        );
    }

    fn ask(session: &Session, target: &Target, item: Item) -> Confirm {
        match action(data(session), target, item) {
            Ok(Action::Ask(confirm)) => confirm,
            _ => panic!("{item:?} asks first"),
        }
    }

    #[test]
    fn delete_variables_warns_about_weight_and_filter() {
        let mut session = open("survey_basic.sav");
        let one = ask(&session, &target(&session, &["age"]), Item::Delete);
        assert_eq!(one.question, "Delete age? Ctrl+Z brings it back.");
        assert_eq!(one.syntax, "DELETE VARIABLES age.");
        run_in(
            &mut session,
            "WEIGHT BY weight.\nCOMPUTE adult = age >= 30.\nFILTER BY adult.",
        );
        let names = ["region", "adult", "weight"];
        let all = ask(&session, &target(&session, &names), Item::Delete);
        assert_eq!(
            all.question,
            "Delete region, weight and adult? Ctrl+Z brings them back. This also ends weighting. \
             This also ends the filter."
        );
        assert_eq!(all.syntax, "DELETE VARIABLES region weight adult.");
        run_in(&mut session, &all.syntax);
        let data = data(&session);
        assert_eq!(data.variable_count(), 17);
        assert!(data.weight_variable().is_none() && data.filter_variable().is_none());
        // a run of three is written with TO
        let run = ask(
            &session,
            &target(&session, &["q5_1", "q5_2", "q5_3"]),
            Item::Delete,
        );
        assert_eq!(run.syntax, "DELETE VARIABLES q5_1 TO q5_3.");
        run_in(&mut session, &run.syntax);
        assert!(data_after(&session, "q5_2").is_none());
    }

    fn data_after(session: &Session, name: &str) -> Option<usize> {
        data(session).variable_index(name)
    }

    #[test]
    fn insert_variable_keeps_its_place_and_the_weight() {
        let mut session = open("survey_basic.sav");
        run_in(
            &mut session,
            "WEIGHT BY weight.\nCOMPUTE adult = age >= 30.\nFILTER BY adult.",
        );
        let age = index(&session, "age");
        let syntax = syntax(&session, &["age"], Item::InsertBefore);
        assert_eq!(
            syntax,
            "NUMERIC new_1 (F8.2).\nMATCH FILES /FILE=* /KEEP=resp_id gender new_1 ALL.\n\
             WEIGHT BY weight."
        );
        run_in(&mut session, &syntax);
        let data = data(&session);
        assert_eq!(data.variable_at(age).name(), "new_1");
        assert_eq!(data.variable_at(age + 1).name(), "age");
        assert_eq!(data.variable_count(), 21);
        assert_eq!(data.case_count(), 200);
        assert_eq!(data.weight_variable(), Some("weight"));
        assert_eq!(data.filter_variable(), Some("adult"));
        // the dictionary of the other variables stays
        assert_eq!(data.variable("gender").unwrap().value_labels().len(), 3);
        // further on, the variables before it are written with TO
        let later = syntax_of(&session, "plan");
        assert!(later.contains("/KEEP=resp_id TO monthly_spend new_2 ALL."));
        run_in(&mut session, &later);
    }

    fn syntax_of(session: &Session, name: &str) -> String {
        syntax(session, &[name], Item::InsertBefore)
    }

    #[test]
    fn insert_before_the_first_column() {
        let mut session = open("survey_basic.sav");
        let syntax = syntax_of(&session, "resp_id");
        assert_eq!(
            syntax,
            "NUMERIC new_1 (F8.2).\nMATCH FILES /FILE=* /KEEP=new_1 ALL."
        );
        run_in(&mut session, &syntax);
        assert_eq!(data(&session).variable_at(0).name(), "new_1");
        assert_eq!(data(&session).variable_at(1).name(), "resp_id");
    }

    #[test]
    fn the_new_name_is_the_first_free() {
        let mut session = open("survey_basic.sav");
        run_in(&mut session, "NUMERIC new_1 new_3.");
        assert!(syntax_of(&session, "age").starts_with("NUMERIC new_2 (F8.2)."));
    }

    #[test]
    fn delete_case_removes_exactly_that_case() {
        for filter in [false, true] {
            let mut session = open("survey_basic.sav");
            if filter {
                // the second case is filtered out; it is deleted all the same
                run_in(&mut session, "COMPUTE f = resp_id <> 1002.\nFILTER BY f.");
            }
            let ids = numbers(&session, "resp_id");
            let confirm = ask(&session, &Target::Cases(vec![1]), Item::DeleteCase);
            assert_eq!(confirm.question, "Delete case 2? Ctrl+Z brings it back.");
            assert_eq!(
                confirm.syntax,
                "COMPUTE #row = #row + 1.\nSELECT IF #row <> 2."
            );
            run_in(&mut session, &confirm.syntax);
            let after = numbers(&session, "resp_id");
            assert_eq!(after.len(), 199);
            assert_eq!(after[..1], ids[..1]);
            assert_eq!(after[1..], ids[2..]);
        }
    }

    #[test]
    fn the_case_menu_copies_and_deletes_one_case() {
        let session = open("survey_basic.sav");
        let data = data(&session);
        let items = |rows: Vec<usize>| {
            let entries = entries(data, &Target::Cases(rows), false, false);
            let entries = entries.into_iter().flatten();
            entries.map(|e| (e.item, e.enabled)).collect::<Vec<_>>()
        };
        assert_eq!(
            items(vec![11]),
            [(Item::Copy, true), (Item::DeleteCase, true)]
        );
        assert_eq!(
            items(vec![3, 4]),
            [(Item::Copy, true), (Item::DeleteCase, false)]
        );
        assert_eq!(heading(data, &Target::Cases(vec![11])), "Case 12");
        assert_eq!(heading(data, &Target::Cases(vec![3, 4])), "2 cases");
        assert!(matches!(
            action(data, &Target::Cases(vec![3, 4]), Item::Copy),
            Ok(Action::Copy)
        ));
    }

    #[test]
    fn frequencies_asks_above_100_variables() {
        let mut session = open("wide.sav");
        let count = data(&session).variable_count();
        let target = Target::Variables((0..count).collect());
        let confirm = ask(&session, &target, Item::Frequencies);
        assert_eq!(
            confirm.question,
            "Frequencies makes a table for each of the 300 variables."
        );
        let first = data(&session).variable_at(0).name().to_string();
        let last = data(&session).variable_at(count - 1).name().to_string();
        assert_eq!(
            confirm.syntax,
            format!("FREQUENCIES VARIABLES={first} TO {last}.")
        );
        assert_eq!(run_in(&mut session, &confirm.syntax), 300);
        // up to 100 it runs at once
        let hundred = Target::Variables((0..100).collect());
        assert!(matches!(
            action(data(&session), &hundred, Item::Frequencies),
            Ok(Action::Run(_))
        ));
    }

    /// The first case (from 0) whose value of `name` meets `test`.
    fn case_where(session: &Session, name: &str, test: impl Fn(Option<f64>) -> bool) -> usize {
        numbers(session, name).into_iter().position(test).unwrap()
    }

    fn cell(session: &Session, case: usize, name: &str) -> Target {
        Target::Cell {
            case,
            column: index(session, name),
        }
    }

    /// Runs the filter on the cell and returns the number of cases it keeps, checking that the
    /// filter is in force.
    fn filtered(session: &mut Session, case: usize, name: &str) -> usize {
        let Ok(Action::Run(syntax)) =
            action(data(session), &cell(session, case, name), Item::Filter)
        else {
            panic!("the filter runs at once");
        };
        run_in(session, &syntax);
        let filter = data(session).filter_variable().unwrap().to_string();
        let values = numbers(session, &filter);
        values.iter().filter(|v| **v == Some(1.0)).count()
    }

    #[test]
    fn the_filter_uses_the_stored_value() {
        let mut session = open("survey_basic.sav");
        // a value with more decimals than the format shows
        run_in(&mut session, "IF ($CASENUM = 4) monthly_spend = 12.3456.");
        let data = data(&session);
        let column = index(&session, "monthly_spend");
        assert_eq!(
            edits::cell_text(&data.variable_at(column), 3, false, &Currencies::default()),
            "12.35"
        );
        assert_eq!(condition(data, 3, column), "monthly_spend = 12.3456");
        assert_eq!(
            filter(data, 3, column),
            "COMPUTE filter_$ = (monthly_spend = 12.3456).\n\
             VARIABLE LABELS filter_$ 'monthly_spend = 12.3456'.\nFILTER BY filter_$."
        );
        assert_eq!(filtered(&mut session, 3, "monthly_spend"), 1);
        // a common value keeps every case that holds it, and Frequencies counts the rest out
        let gender = numbers(&session, "gender");
        let women = gender.iter().filter(|g| **g == Some(2.0)).count();
        let case = case_where(&session, "gender", |g| g == Some(2.0));
        assert_eq!(filtered(&mut session, case, "gender"), women);
        assert_eq!(data_of(&session).filter_variable(), Some("filter_$"));
        assert_eq!(run_in(&mut session, "FREQUENCIES VARIABLES=gender."), 1);
    }

    fn data_of(session: &Session) -> &Dataset {
        data(session)
    }

    #[test]
    fn a_user_missing_value_is_matched_with_value() {
        let mut session = open("survey_basic.sav");
        let case = case_where(&session, "q1_satisfaction", |v| v == Some(99.0));
        let column = index(&session, "q1_satisfaction");
        assert_eq!(
            condition(data(&session), case, column),
            "VALUE(q1_satisfaction) = 99"
        );
        let refused = numbers(&session, "q1_satisfaction");
        let refused = refused.iter().filter(|v| **v == Some(99.0)).count();
        assert!(refused > 0);
        assert_eq!(filtered(&mut session, case, "q1_satisfaction"), refused);
        // a valid value is compared as it is
        let case = case_where(&session, "q1_satisfaction", |v| v == Some(4.0));
        assert_eq!(
            condition(data(&session), case, column),
            "q1_satisfaction = 4"
        );
    }

    #[test]
    fn a_system_missing_cell_filters_on_sysmis() {
        let mut session = open("survey_basic.sav");
        let case = case_where(&session, "monthly_spend", |v| v.is_none());
        let column = index(&session, "monthly_spend");
        assert_eq!(
            condition(data(&session), case, column),
            "SYSMIS(monthly_spend)"
        );
        assert_eq!(filtered(&mut session, case, "monthly_spend"), 19);
    }

    #[test]
    fn a_string_value_is_quoted() {
        let mut session = open("survey_basic.sav");
        let column = index(&session, "plan");
        assert_eq!(condition(data(&session), 0, column), "plan = 'BASIC'");
        run_in(&mut session, "IF ($CASENUM = 1) plan = 'O''Brien'.");
        assert_eq!(condition(data(&session), 0, column), "plan = 'O''Brien'");
        assert_eq!(filtered(&mut session, 0, "plan"), 1);
        // an empty comment matches the blank ones
        let comments = data(&session).variable("q3_comment").unwrap();
        let comments: Vec<String> = comments
            .string()
            .unwrap()
            .iter()
            .map(|c| c.trim().to_string())
            .collect();
        let blank = comments.iter().position(|c| c.is_empty()).unwrap();
        let blanks = comments.iter().filter(|c| c.is_empty()).count();
        assert_eq!(
            condition(data(&session), blank, index(&session, "q3_comment")),
            "q3_comment = ''"
        );
        assert_eq!(filtered(&mut session, blank, "q3_comment"), blanks);
    }

    #[test]
    fn the_filter_name_never_overwrites_a_variable() {
        let mut session = open("survey_basic.sav");
        run_in(&mut session, "COMPUTE filter_$ = 7.");
        assert!(filter(data(&session), 0, 1).starts_with("COMPUTE filter_$_2 = (gender = 1)."));
        filtered(&mut session, 0, "gender");
        assert_eq!(data(&session).filter_variable(), Some("filter_$_2"));
        // the user's variable is untouched
        assert!(
            numbers(&session, "filter_$")
                .iter()
                .all(|v| *v == Some(7.0))
        );
        // the filter in force is replaced in place
        assert!(filter(data(&session), 0, 2).starts_with("COMPUTE filter_$_2 = (age = 49)."));
    }

    #[test]
    fn clear_value_is_the_cell_edit_with_empty_text() {
        let mut session = open("survey_basic.sav");
        let run = |session: &Session, case, name| match action(
            data(session),
            &cell(session, case, name),
            Item::ClearValue,
        ) {
            Ok(Action::Run(syntax)) => syntax,
            _ => panic!("Clear value runs at once"),
        };
        let age = run(&session, 2, "age");
        assert_eq!(age, "IF ($CASENUM = 3) age = $SYSMIS.");
        let plan = run(&session, 2, "plan");
        assert_eq!(plan, "IF ($CASENUM = 3) plan = ''.");
        run_in(&mut session, &format!("{age}\n{plan}"));
        assert_eq!(numbers(&session, "age")[2], None);
    }

    #[test]
    fn the_cell_menu_filters_keeps_and_sorts() {
        let mut session = open("survey_basic.sav");
        let target = cell(&session, 1, "gender");
        let labels = |session: &Session| {
            let entries = entries(data(session), &target, false, false);
            let entries = entries.into_iter().flatten();
            entries.map(|e| e.label).collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&session),
            [
                "Copy",
                "Edit",
                "Clear value",
                "Only cases where gender = 1 (Male)",
                "Keep only cases where gender = 1 (Male)...",
                "Sort ascending by gender",
                "Sort descending by gender"
            ]
        );
        assert_eq!(heading(data(&session), &target), "gender, case 2");
        assert!(matches!(
            action(data(&session), &target, Item::Edit),
            Ok(Action::EditCell(1, 1))
        ));
        assert!(matches!(
            action(data(&session), &target, Item::SortDescending),
            Ok(Action::Run(s)) if s == "SORT CASES BY gender (D)."
        ));
        // with a filter on, Stop filtering turns it off
        filtered(&mut session, 1, "gender");
        assert!(labels(&session).contains(&"Stop filtering".to_string()));
        let Ok(Action::Run(off)) = action(data(&session), &target, Item::FilterOff) else {
            panic!("Stop filtering runs at once");
        };
        run_in(&mut session, &off);
        assert!(data(&session).filter_variable().is_none());
        // Keep only opens Select Cases, which deletes the other cases
        let Ok(Action::Open(opened)) = action(data(&session), &target, Item::KeepOnlyDialog) else {
            panic!("Keep only opens a dialog");
        };
        let Dialog::SelectCases(d) = *opened else {
            panic!("Keep only opens Select Cases");
        };
        assert_eq!(d.syntax().unwrap(), "SELECT IF gender = 1.");
        run_in(&mut session, &d.syntax().unwrap());
        assert_eq!(data(&session).case_count(), 106);
    }

    #[test]
    fn show_in_data_view_selects_the_column() {
        let mut session = open("survey_basic.sav");
        let names = ["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"];
        let items = |session: &Session, target: &Target| {
            let entries = entries(data(session), target, true, false);
            entries
                .into_iter()
                .flatten()
                .map(|e| e.item)
                .collect::<Vec<_>>()
        };
        // rows of the Variable View: the column menu, with Show in Data View for one
        let one = items(&session, &target(&session, &["age"]));
        assert!(one.contains(&Item::ShowInData) && !one.contains(&Item::Properties));
        let age = index(&session, "age");
        assert!(matches!(
            action(data(&session), &target(&session, &["age"]), Item::ShowInData),
            Ok(Action::ShowInData(c)) if c == age
        ));
        // five rows set to ordinal in one step
        let rows = target(&session, &names);
        assert!(!items(&session, &rows).contains(&Item::ShowInData));
        let Ok(Action::Run(syntax)) =
            action(data(&session), &rows, Item::Measure(Measure::Ordinal))
        else {
            panic!("the measure runs at once");
        };
        assert_eq!(syntax, "VARIABLE LEVEL q5_1 TO q5_5 (ORDINAL).");
        run_in(&mut session, &syntax);
        // a cell of a row: Edit for that field first, then the menu of its variable
        let label = Field::Label as usize;
        let field = Target::Field {
            variable: age,
            field: label,
        };
        let cell = items(&session, &field);
        assert_eq!(cell[..2], [Item::Edit, Item::SortAscending]);
        assert!(cell.contains(&Item::ShowInData) && cell.contains(&Item::Delete));
        assert_eq!(heading(data(&session), &field), "age: Age in years");
        assert!(matches!(
            action(data(&session), &field, Item::Edit),
            Ok(Action::EditField(v, f)) if v == age && f == label
        ));
        assert!(matches!(
            action(data(&session), &field, Item::Frequencies),
            Ok(Action::Run(s)) if s == "FREQUENCIES VARIABLES=age."
        ));
    }
}
