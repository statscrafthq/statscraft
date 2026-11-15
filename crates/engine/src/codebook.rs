//! CODEBOOK: one table per variable and multiple response set with its dictionary entry and
//! summary statistics, and an optional table about the data file.
//!
//! Rules from the Command Syntax Reference (CODEBOOK):
//! - The list names variables and sets, each once; without it every variable comes in file
//!   order, then every set. A level in brackets changes only which statistics an entry gets:
//!   nominal and ordinal variables and sets get counts and percents of their values, scale
//!   variables the mean, standard deviation and quartiles, plus counts of labeled values.
//! - Value labels show with VARINFO VALUELABELS, and always when counts or percents are asked.
//!   Above MAXCATS values (scale variables: labeled values) values, labels and counts are left
//!   out.
//! - Percents are of all cases the filter keeps, missing ones included. SPLIT FILE is ignored.
//! - VARORDER: FILE, ALPHA (by name), VARLIST (as listed; the default with a list), MEASURE
//!   (nominal, ordinal, scale, unknown; sets count as nominal), ATTRIBUTE(name) (without the
//!   attribute first, then with an empty value, then by value). DESCENDING reverses.
//! - ATTRIBUTES are custom attributes, RESERVEDATTRIBUTES those whose name starts with "$";
//!   names starting with "@" or "$@" are never shown.
//!
//! Choices where the manual is silent:
//! - Counts are weighted; a case with a missing, zero or negative weight is left out. A
//!   numeric variable of unknown level gets the level its values suggest (as reading text
//!   data does) for its statistics; the table still says Unknown.
//! - Quartiles follow the FREQUENCIES rule, (W + 1) p.
//! - The valid values of a nominal or ordinal variable are its labeled values and the values
//!   found; the missing values are its defined discrete missing values, the labeled or found
//!   values in a missing range, and System when cases are system-missing. Without counts only
//!   labeled values are listed.
//! - Within a VARORDER group, and for ALPHA ties, file order holds; a set's name sorts without
//!   its "$". With ATTRIBUTE the values compare without regard to case.
//! - The table always reads the data, also with STATISTICS NONE.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number, summarize};
use crate::examine::Distribution;
use crate::format::{Stat, category_text, procedure_format};
use crate::pass::default_measure;
use crate::session_commands::missing_text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, MultipleResponseSet, Value, Values, Variable};
use statscraft_syntax::{
    Codebook, CodebookFile as F, CodebookInfo as I, CodebookOrder, CodebookStatistic as S, Level,
    VarItem,
};

/// A variable (by position) or a set (by position among the sets), with the level given in
/// brackets.
#[derive(Clone, Copy)]
enum Entry {
    Variable(usize, Option<Level>),
    Set(usize),
}

/// What every table of one command shares.
struct Context<'a> {
    command: &'a Codebook,
    data: &'a Dataset,
    /// The cases the filter keeps, with their weights.
    cases: Vec<(usize, f64)>,
    total: f64,
    counts: bool,
    scale_min: usize,
    currencies: crate::format::Currencies,
}

impl Context<'_> {
    fn info(&self, info: I) -> bool {
        self.command.varinfo.contains(&info)
    }

    fn statistic(&self, statistic: S) -> bool {
        self.command.statistics.contains(&statistic)
    }

    /// The count and percent cells of a weight, as the statistics ask.
    fn tally(&self, weight: Option<f64>) -> Vec<Cell> {
        let mut cells = Vec::new();
        if self.statistic(S::Count) {
            cells.push(number(weight));
        }
        if self.statistic(S::Percent) {
            cells.push(number(weight.map(|w| 100.0 * w / self.total)));
        }
        cells
    }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn measure_name(measure: Measure) -> String {
    title_case(measure.as_str())
}

/// Custom attributes (`reserved` false) or those starting with "$", without the hidden ones
/// starting with "@" or "$@"; an attribute with several values gives one row per value.
fn attributes(list: &[(String, Vec<String>)], reserved: bool) -> Vec<(String, String)> {
    let shown = |name: &str| {
        let hidden = name.starts_with('@') || name.starts_with("$@");
        !hidden && name.starts_with('$') == reserved
    };
    let mut out = Vec::new();
    for (name, values) in list.iter().filter(|(name, _)| shown(name)) {
        match values.as_slice() {
            [one] => out.push((name.clone(), one.clone())),
            many => out.extend(
                (many.iter().enumerate()).map(|(i, v)| (format!("{name}[{}]", i + 1), v.clone())),
            ),
        }
    }
    out
}

/// A table of rows (section, item, values...); a section name shows on its first row only.
fn table(title: String, columns: Vec<String>, mut rows: Vec<Vec<Cell>>) -> Table {
    let mut previous = None;
    for row in &mut rows {
        let section = row[0].clone();
        if Some(&section) == previous.as_ref() {
            row[0] = Cell::Empty;
        }
        previous = Some(section);
    }
    // every row as wide as the header
    for row in &mut rows {
        row.resize(columns.len(), Cell::Empty);
    }
    Table {
        title,
        row_headers: 2,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        ..Table::default()
    }
}

impl Engine {
    pub(crate) fn codebook(&mut self, command: &Codebook) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        let tables = self.codebook_tables(command);
        self.end_pass(before.as_ref());
        tables
    }

    fn codebook_tables(&self, command: &Codebook) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let weights = case_weights(data)?;
        let filter = data
            .filter_variable()
            .map(|f| data.variable(f))
            .transpose()?;
        let cases: Vec<(usize, f64)> = (0..data.case_count())
            .filter(|&c| filter.is_none_or(|f| f.keeps(c)))
            .map(|c| (c, weights.as_ref().map_or(1.0, |w| w[c])))
            // a missing, zero or negative weight leaves the case out
            .filter(|&(_, w)| w > 0.0)
            .collect();
        let context = Context {
            command,
            data,
            total: cases.iter().map(|c| c.1).sum(),
            cases,
            counts: command.statistics.contains(&S::Count)
                || command.statistics.contains(&S::Percent),
            scale_min: self.scale_min(),
            currencies: self.currencies(),
        };
        let mut entries = self.entries(command)?;
        order(data, command, &mut entries);
        let mut out = Vec::new();
        if !command.fileinfo.is_empty() {
            out.push(file_table(&context));
        }
        for entry in entries {
            out.push(match entry {
                Entry::Variable(index, level) => variable_table(&context, index, level),
                Entry::Set(index) => set_table(&context, &data.multiple_response_sets()[index])?,
            });
        }
        Ok(out)
    }

    /// The variables and sets of the list, as written; every one without a list.
    fn entries(&self, command: &Codebook) -> Result<Vec<Entry>, Error> {
        let data = self.active()?;
        let sets = data.multiple_response_sets();
        if command.items.is_empty() {
            let variables = (0..data.variable_count()).map(|i| Entry::Variable(i, None));
            return Ok(variables.chain((0..sets.len()).map(Entry::Set)).collect());
        }
        let mut entries = Vec::new();
        let mut named = |entry: Entry, name: &str| -> Result<(), Error> {
            let same = |e: &Entry| match (e, entry) {
                (Entry::Variable(a, _), Entry::Variable(b, _)) => *a == b,
                (Entry::Set(a), Entry::Set(b)) => *a == b,
                _ => false,
            };
            if entries.iter().any(same) {
                return Err(Error(format!(
                    "CODEBOOK: {name} is named more than once; name each variable and set once"
                )));
            }
            entries.push(entry);
            Ok(())
        };
        for item in &command.items {
            match &item.item {
                VarItem::Name(name) if name.eq_ignore_ascii_case("$ALL") => {
                    for (i, set) in sets.iter().enumerate() {
                        named(Entry::Set(i), &set.name)?;
                    }
                }
                VarItem::Name(name) if name.starts_with('$') => {
                    let index = (sets.iter())
                        .position(|s| s.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| {
                            Error(format!(
                                "CODEBOOK: unknown multiple response set {name} (define it with MRSETS)"
                            ))
                        })?;
                    if item.level == Some(Level::Scale) {
                        return Err(Error(format!(
                            "CODEBOOK: {name} is a multiple response set, which cannot be scale; use [N] or [O]"
                        )));
                    }
                    named(Entry::Set(index), name)?;
                }
                _ => {
                    let names = self.expand(std::slice::from_ref(&item.item))?;
                    for (i, name) in names.iter().enumerate() {
                        let index = data.variable_index(name).expect("expanded names exist");
                        let level = item.level.filter(|_| i + 1 == names.len());
                        if level == Some(Level::Scale) && data.variable_at(index).is_string() {
                            return Err(Error(format!(
                                "CODEBOOK: {name} is a string variable, which cannot be scale; use [N] or [O]"
                            )));
                        }
                        named(Entry::Variable(index, level), name)?;
                    }
                }
            }
        }
        Ok(entries)
    }
}

/// Sorts the entries as VARORDER and SORT say.
fn order(data: &Dataset, command: &Codebook, entries: &mut [Entry]) {
    let sets = data.multiple_response_sets();
    let order = match &command.order {
        Some(CodebookOrder::Varlist) if command.items.is_empty() => &CodebookOrder::File,
        Some(order) => order,
        None if command.items.is_empty() => &CodebookOrder::File,
        None => &CodebookOrder::Varlist,
    };
    let file_place = |e: &Entry| match e {
        Entry::Variable(i, _) => (0, *i),
        Entry::Set(i) => (1, *i),
    };
    if *order != CodebookOrder::Varlist {
        entries.sort_by_key(file_place);
    }
    let name = |e: &Entry| match e {
        Entry::Variable(i, _) => data.variable_at(*i).name().to_lowercase(),
        Entry::Set(i) => sets[*i].name.trim_start_matches('$').to_lowercase(),
    };
    match order {
        CodebookOrder::Alpha => entries.sort_by_key(name),
        CodebookOrder::Measure => entries.sort_by_key(|e| match e {
            Entry::Variable(i, _) => match data.variable_at(*i).measure() {
                Measure::Nominal => 0,
                Measure::Ordinal => 1,
                Measure::Scale => 2,
                Measure::Unknown => 3,
            },
            Entry::Set(_) => 0,
        }),
        CodebookOrder::Attribute(attribute) => entries.sort_by_key(|e| match e {
            Entry::Variable(i, _) => {
                let variable = data.variable_at(*i);
                let found =
                    (variable.attributes().iter()).find(|(n, _)| n.eq_ignore_ascii_case(attribute));
                match found.map(|(_, values)| values.join(", ").to_lowercase()) {
                    None => (0, String::new()),
                    Some(value) if value.trim().is_empty() => (1, String::new()),
                    Some(value) => (2, value),
                }
            }
            Entry::Set(_) => (0, String::new()),
        }),
        CodebookOrder::File | CodebookOrder::Varlist => {}
    }
    if command.descending {
        entries.reverse();
    }
}

fn file_table(context: &Context) -> Table {
    let data = context.data;
    let mut rows = Vec::new();
    let file = data.file();
    for item in [
        F::Name,
        F::Location,
        F::CaseCount,
        F::Label,
        F::Attributes,
        F::Documents,
        F::Weight,
        F::ReservedAttributes,
    ] {
        if !context.command.fileinfo.contains(&item) {
            continue;
        }
        let path_part = |part: Option<&std::ffi::OsStr>| {
            part.map_or(Cell::Empty, |p| text(p.to_string_lossy()))
        };
        match item {
            F::Name => rows.push(vec![
                text("File Name"),
                Cell::Empty,
                path_part(file.and_then(|f| f.file_name())),
            ]),
            F::Location => rows.push(vec![
                text("Location"),
                Cell::Empty,
                path_part(file.and_then(|f| f.parent()).map(|p| p.as_os_str())),
            ]),
            F::CaseCount => rows.push(vec![
                text("Number of Cases"),
                Cell::Empty,
                Cell::Number(data.case_count() as f64),
            ]),
            F::Label => rows.push(vec![
                text("Label"),
                Cell::Empty,
                data.file_label().map_or(Cell::Empty, text),
            ]),
            F::Attributes | F::ReservedAttributes => {
                let reserved = item == F::ReservedAttributes;
                let section = ["Custom Attributes", "Reserved Attributes"][usize::from(reserved)];
                for (name, value) in attributes(data.attributes(), reserved) {
                    rows.push(vec![text(section), text(name), text(value)]);
                }
            }
            F::Documents => {
                for line in data.documents() {
                    rows.push(vec![text("Documents"), Cell::Empty, text(line.trim_end())]);
                }
            }
            F::Weight => rows.push(vec![
                text("Weight"),
                Cell::Empty,
                text(data.weight_variable().unwrap_or("None")),
            ]),
        }
    }
    table(
        "File Information".into(),
        vec![String::new(), String::new(), "Value".into()],
        rows,
    )
}

/// The header of a variable or set table: the Count and Percent columns as asked.
fn columns(context: &Context) -> Vec<String> {
    let mut columns = vec![String::new(), String::new(), "Value".to_string()];
    if context.statistic(S::Count) {
        columns.push("Count".into());
    }
    if context.statistic(S::Percent) {
        columns.push("Percent".into());
    }
    columns
}

fn variable_table(context: &Context, index: usize, level: Option<Level>) -> Table {
    let data = context.data;
    let variable = data.variable_at(index);
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let standard = |item: &str, value: Cell| vec![text("Standard Attributes"), text(item), value];
    if context.info(I::Position) {
        rows.push(standard("Position", Cell::Number(index as f64 + 1.0)));
    }
    if context.info(I::Label) {
        rows.push(standard(
            "Label",
            variable.label().map_or(Cell::Empty, text),
        ));
    }
    if context.info(I::Type) {
        let kind = if variable.is_string() {
            "String"
        } else {
            "Numeric"
        };
        rows.push(standard("Type", text(kind)));
    }
    if context.info(I::Format) {
        rows.push(standard("Format", text(variable.format())));
    }
    if context.info(I::Measure) {
        rows.push(standard(
            "Measurement",
            text(measure_name(variable.measure())),
        ));
    }
    if context.info(I::Role) {
        let role = title_case(variable.role().unwrap_or("input"));
        rows.push(standard("Role", text(role)));
    }
    if context.info(I::Missing) {
        let defined = missing_text(variable.missing_values());
        let cell = if defined.is_empty() {
            Cell::Empty
        } else {
            text(defined)
        };
        rows.push(standard("Missing Values", cell));
    }
    for (info, section, reserved) in [
        (I::Attributes, "Custom Attributes", false),
        (I::ReservedAttributes, "Reserved Attributes", true),
    ] {
        if context.info(info) {
            for (name, value) in attributes(variable.attributes(), reserved) {
                rows.push(vec![text(section), text(name), text(value)]);
            }
        }
    }
    let measure = match level {
        Some(Level::Scale) => Measure::Scale,
        Some(_) => Measure::Nominal,
        None if variable.is_string() => Measure::Nominal,
        None => match variable.measure() {
            Measure::Unknown => default_measure(data, index, context.scale_min),
            measure => measure,
        },
    };
    let scale = measure == Measure::Scale;
    let mut formats: Vec<Vec<String>> = Vec::new();
    if scale {
        let pairs: Vec<(f64, f64)> = match variable.values() {
            Values::Numeric(values) => (context.cases.iter())
                .filter_map(|&(c, w)| {
                    let x = values.get(c)?;
                    (!variable.is_user_missing(x)).then_some((x, w))
                })
                .collect(),
            Values::String(_) => Vec::new(),
        };
        let summary = summarize(&pairs);
        let distribution = Distribution::new(&pairs);
        let mut statistics: Vec<(&str, Option<f64>, Stat)> = Vec::new();
        if context.statistic(S::Mean) {
            statistics.push(("Mean", summary.mean, Stat::Value));
        }
        if context.statistic(S::StdDev) {
            statistics.push(("Standard Deviation", summary.sd, Stat::Spread));
        }
        if context.statistic(S::Quartiles) {
            for (p, name) in [
                (25.0, "Percentile 25"),
                (50.0, "Percentile 50"),
                (75.0, "Percentile 75"),
            ] {
                let value = distribution.frequencies_percentile(p);
                statistics.push((name, value, Stat::Value));
            }
        }
        for (name, value, stat) in statistics {
            formats.resize(rows.len(), Vec::new());
            formats.push(vec![
                String::new(),
                String::new(),
                procedure_format(variable.format(), stat),
            ]);
            rows.push(vec![
                text("Central Tendency and Dispersion"),
                text(name),
                number(value),
            ]);
        }
    }
    let mut caption = None;
    if context.counts || context.info(I::ValueLabels) {
        let values = value_rows(context, &variable, scale);
        if values.valid > context.command.maxcats {
            caption = Some(format!(
                "{} has more than {} {}values (MAXCATS), so its values, labels and counts are not shown.",
                variable.name(),
                context.command.maxcats,
                if scale { "labeled " } else { "" }
            ));
        } else {
            rows.extend(values.rows);
        }
    }
    if !formats.is_empty() {
        formats.resize(rows.len(), Vec::new());
        let width = columns(context).len();
        for row in &mut formats {
            row.resize(width, String::new());
        }
    }
    let mut table = table(variable.name().to_string(), columns(context), rows);
    table.formats = formats;
    table.caption = caption;
    table
}

/// The rows of the values of a variable, and the number of its valid categories as MAXCATS
/// counts them.
struct ValueRows {
    rows: Vec<Vec<Cell>>,
    valid: usize,
}

fn value_rows(context: &Context, variable: &Variable, scale: bool) -> ValueRows {
    let keys = categories(variable, true);
    let mut found: Vec<(Key, f64)> = Vec::new();
    let mut system = 0.0;
    for &(case, weight) in &context.cases {
        match &keys[case] {
            None => system += weight,
            Some(key) => match found.binary_search_by(|(k, _)| k.compare(key)) {
                Ok(at) => found[at].1 += weight,
                Err(at) => found.insert(at, (key.clone(), weight)),
            },
        }
    }
    let key_of = |value: &Value| match value {
        Value::Number(x) => Key::Number(*x),
        Value::String(s) => Key::Text(s.trim_end().to_string()),
    };
    let labels: Vec<(Key, &str)> = (variable.value_labels().iter())
        .map(|(value, label)| (key_of(value), label.as_str()))
        .collect();
    let label_of = |key: &Key| labels.iter().find(|(k, _)| k == key).map(|(_, l)| *l);
    let missing = |key: &Key| match key {
        Key::Number(x) => variable.is_user_missing(*x),
        Key::Text(t) => variable.is_user_missing_str(t),
    };
    let defined: Vec<Key> = match variable.missing_values() {
        statscraft_data::MissingValues::Numbers(values) => {
            values.iter().map(|&x| Key::Number(x)).collect()
        }
        statscraft_data::MissingValues::Range { extra, .. } => {
            extra.iter().map(|&x| Key::Number(x)).collect()
        }
        statscraft_data::MissingValues::Strings(values) => {
            values.iter().map(|s| Key::Text(s.clone())).collect()
        }
        statscraft_data::MissingValues::None => Vec::new(),
    };
    // every value to show: labeled, found (with counts) and defined missing values
    let mut all: Vec<Key> = labels.iter().map(|(k, _)| k.clone()).collect();
    if context.counts {
        all.extend(found.iter().map(|(k, _)| k.clone()));
        all.extend(defined);
    }
    all.sort_by(|a, b| a.compare(b));
    all.dedup_by(|a, b| a.compare(b).is_eq());
    let format = context.currencies.resolve(variable.format());
    let shown = |key: &Key| match key {
        Key::Number(x) => category_text(*x, &format),
        Key::Text(t) => t.clone(),
    };
    let count = |key: &Key| {
        let at = found.binary_search_by(|(k, _)| k.compare(key)).ok();
        Some(at.map_or(0.0, |at| found[at].1))
    };
    let valid_section = if scale {
        "Labeled Values"
    } else {
        "Valid Values"
    };
    let mut rows = Vec::new();
    let mut valid = 0;
    for key in all.iter().filter(|k| !missing(k)) {
        let label = label_of(key);
        if scale && label.is_none() {
            continue;
        }
        valid += 1;
        let mut row = vec![
            text(valid_section),
            text(shown(key)),
            label.map_or(Cell::Empty, text),
        ];
        row.extend(context.tally(count(key)));
        rows.push(row);
    }
    for key in all.iter().filter(|k| missing(k)) {
        let label = label_of(key);
        let mut row = vec![
            text("Missing Values"),
            text(shown(key)),
            label.map_or(Cell::Empty, text),
        ];
        row.extend(context.tally(count(key)));
        rows.push(row);
    }
    if context.counts && system > 0.0 {
        let mut row = vec![text("Missing Values"), text("System"), Cell::Empty];
        row.extend(context.tally(Some(system)));
        rows.push(row);
    }
    ValueRows { rows, valid }
}

fn set_table(context: &Context, set: &MultipleResponseSet) -> Result<Table, Error> {
    let data = context.data;
    let variables: Vec<Variable> = (set.variables.iter())
        .map(|name| data.variable(name))
        .collect::<Result<_, _>>()?;
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let standard = |item: &str, value: Cell| vec![text("Standard Attributes"), text(item), value];
    if context.info(I::Label) {
        let label = (!set.label.is_empty()).then(|| text(&set.label));
        rows.push(standard("Label", label.unwrap_or(Cell::Empty)));
    }
    let dichotomy = set.counted_value.is_some();
    if context.info(I::Type) {
        rows.push(standard("Type", text("Multiple Response Set")));
        let kind = if dichotomy {
            "Multiple dichotomy"
        } else {
            "Multiple category"
        };
        rows.push(standard("Set Type", text(kind)));
        if let Some(counted) = &set.counted_value {
            rows.push(standard("Counted Value", text(counted.trim())));
        }
        rows.push(standard("Variables", text(set.variables.join(", "))));
    }
    let mut caption = None;
    if context.counts || context.info(I::ValueLabels) {
        let keys: Vec<Vec<Option<Key>>> = variables.iter().map(|v| categories(v, false)).collect();
        // (category text, label, count)
        let mut categories_found: Vec<(String, Option<String>, f64)> = Vec::new();
        if let Some(counted) = &set.counted_value {
            let counted = match variables.first().is_some_and(|v| v.is_string()) {
                true => Some(Key::Text(counted.trim_end().to_string())),
                false => counted.trim().parse::<f64>().ok().map(Key::Number),
            };
            for (variable, keys) in variables.iter().zip(&keys) {
                let count: f64 = (context.cases.iter())
                    .filter(|&&(c, _)| counted.is_some() && keys[c] == counted)
                    .map(|&(_, w)| w)
                    .sum();
                let counted_label = (variable.value_labels().iter())
                    .find(|(value, _)| match (value, &counted) {
                        (Value::Number(x), Some(Key::Number(n))) => x == n,
                        (Value::String(s), Some(Key::Text(t))) => s.trim_end() == t,
                        _ => false,
                    })
                    .map(|(_, l)| l.clone());
                let label = counted_label
                    .filter(|_| set.counted_value_labels)
                    .or_else(|| variable.label().map(str::to_string));
                categories_found.push((variable.name().to_string(), label, count));
            }
        } else {
            let mut found: Vec<(Key, f64)> = Vec::new();
            for &(case, weight) in &context.cases {
                let mut own: Vec<&Key> = keys.iter().filter_map(|k| k[case].as_ref()).collect();
                own.sort_by(|a, b| a.compare(b));
                own.dedup_by(|a, b| a.compare(b).is_eq());
                for key in own {
                    match found.binary_search_by(|(k, _)| k.compare(key)) {
                        Ok(at) => found[at].1 += weight,
                        Err(at) => found.insert(at, (key.clone(), weight)),
                    }
                }
            }
            // labeled values never found count 0
            for variable in &variables {
                for (value, _) in variable.value_labels() {
                    let key = match value {
                        Value::Number(x) => Key::Number(*x),
                        Value::String(s) => Key::Text(s.trim_end().to_string()),
                    };
                    let user_missing = match &key {
                        Key::Number(x) => variable.is_user_missing(*x),
                        Key::Text(t) => variable.is_user_missing_str(t),
                    };
                    if let (false, Err(at)) = (
                        user_missing,
                        found.binary_search_by(|(k, _)| k.compare(&key)),
                    ) {
                        found.insert(at, (key, 0.0));
                    }
                }
            }
            let label_of = |key: &Key| {
                variables.iter().find_map(|v| {
                    v.value_labels().iter().find_map(|(value, label)| {
                        let same = match (value, key) {
                            (Value::Number(x), Key::Number(n)) => x == n,
                            (Value::String(s), Key::Text(t)) => s.trim_end() == t,
                            _ => false,
                        };
                        same.then(|| label.clone())
                    })
                })
            };
            let format = variables
                .first()
                .map(|v| context.currencies.resolve(v.format()))
                .unwrap_or_default();
            for (key, count) in found {
                let label = label_of(&key);
                if !context.counts && label.is_none() {
                    continue;
                }
                let shown = match &key {
                    Key::Number(x) => category_text(*x, &format),
                    Key::Text(t) => t.clone(),
                };
                categories_found.push((shown, label, count));
            }
        }
        if categories_found.len() > context.command.maxcats {
            caption = Some(format!(
                "{} has more than {} categories (MAXCATS), so its categories and counts are not shown.",
                set.name, context.command.maxcats
            ));
        } else {
            for (shown, label, count) in categories_found {
                let mut row = vec![
                    text("Valid Values"),
                    text(shown),
                    label.map_or(Cell::Empty, text),
                ];
                row.extend(context.tally(Some(count)));
                rows.push(row);
            }
        }
    }
    let mut table = table(set.name.clone(), columns(context), rows);
    table.caption = caption;
    Ok(table)
}
