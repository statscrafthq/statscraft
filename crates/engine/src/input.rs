//! Input programs: INPUT PROGRAM and FILE TYPE build the cases of a new active dataset from
//! records, with END CASE, END FILE, REREAD, REPEATING DATA and LEAVE.
//!
//! Rules, from the descriptions of these commands:
//! - The commands between INPUT PROGRAM and END INPUT PROGRAM run as one block, again and
//!   again from the top, until a DATA LIST finds no more records or END FILE runs. The case
//!   that was being built then is dropped.
//! - A program without END CASE (or REPEATING DATA) builds one case at the end of each run.
//!   With END CASE a case is built at each END CASE, and the run goes on after it.
//! - Before each case, and after each END CASE, every variable is system-missing or blank
//!   again, except scratch variables and those named on LEAVE, which keep their value; a
//!   numeric LEAVE variable starts at 0.
//! - A DATA LIST inside reads the next record or records of its data into the case. With
//!   END=var the variable is 0, and 1 when the data are used up; the other variables keep
//!   their values then, and the program goes on. A DATA LIST without variables passes over
//!   a record.
//! - REREAD makes the next DATA LIST of the same data read the last record again, from
//!   COLUMN on (column COLUMN becomes column 1). Of several REREAD in a row the last counts.
//! - REPEATING DATA builds one case per repeating group of the record the last DATA LIST
//!   read: OCCURS groups from column STARTS, LENGTH columns each; the other variables are
//!   spread to each case. With CONTINUED, a group that would pass the end column goes on the
//!   next record, from the column given; ID compares an identifier on those records. A
//!   record is at least 80 columns; numeric fields past its end are system-missing with a
//!   warning, strings blank.
//! - FILE TYPE reads one record at a time and finds its RECORD TYPE by the record
//!   identifier; the DATA LIST after RECORD TYPE reads that record. MIXED: each record of a
//!   listed type is a case. GROUPED: the records with one case identifier make a case, with
//!   warnings for undefined, duplicate, missing and out-of-order records as asked. NESTED:
//!   each record of the lowest level is a case with the values of the records above it;
//!   records before the first one of the highest level are skipped; DUPLICATE=CASE makes a
//!   case of a record that has no record below it; SPREAD=NO gives the values to the first
//!   case only.
//!
//! The program runs on an engine of its own whose dataset is the one case being built, as
//! in the case-by-case pass, so DO IF, LOOP and the other transformations work unchanged. A
//! first run on no case at all gives the dictionary, in the order the commands define it.

use crate::data_list::{self, Pending, Read, Record};
use crate::expr::Column;
use crate::rows::is_scratch;
use crate::{Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{
    Command, DataField, DataLayout, DataList, Duplicate, Expr, FileKind, FileType, FormatType,
    IdField, Place, RecordType, RepeatingData, Value, VarItem,
};
use std::collections::{HashMap, VecDeque};

/// A record is at least this long, as inline data are; blanks pad a shorter one.
const RECORD_LENGTH: usize = 80;

#[derive(Clone)]
enum Kind {
    Input,
    FileType(Box<FileType>),
}

/// An INPUT PROGRAM or FILE TYPE block with its commands.
#[derive(Clone)]
pub(crate) struct Program {
    kind: Kind,
    body: Vec<Command>,
}

impl Program {
    pub(crate) fn opener(&self) -> &'static str {
        match self.kind {
            Kind::Input => "INPUT PROGRAM",
            Kind::FileType(_) => "FILE TYPE",
        }
    }

    pub(crate) fn closer(&self) -> &'static str {
        match self.kind {
            Kind::Input => "END INPUT PROGRAM",
            Kind::FileType(_) => "END FILE TYPE",
        }
    }

    /// The program reads inline data, so it waits for BEGIN DATA.
    fn inline(&self) -> bool {
        match &self.kind {
            Kind::Input => (self.body.iter())
                .any(|c| matches!(c, Command::DataList(list) if list.file.is_none())),
            Kind::FileType(file_type) => file_type.file.is_none(),
        }
    }

    fn files(&self) -> Vec<String> {
        let mut files: Vec<String> = match &self.kind {
            Kind::FileType(file_type) => file_type.file.iter().cloned().collect(),
            Kind::Input => Vec::new(),
        };
        for command in &self.body {
            if let Command::DataList(list) = command {
                files.extend(list.file.iter().cloned());
            }
        }
        files
    }
}

/// The records of one data source, and how far reading has come.
#[derive(Clone)]
struct Source {
    records: Vec<Record>,
    next: usize,
    /// The first record of the case the last DATA LIST read, or the record FILE TYPE matched.
    current: Option<usize>,
    /// FREE data: the values of the last line not read yet, and where the line is.
    rest: VecDeque<Option<String>>,
    rest_place: String,
    skipped: bool,
    /// END= was set to 1.
    ended: bool,
    length: usize,
}

impl Source {
    fn new(records: Vec<Record>) -> Source {
        let longest = records.iter().map(|r| r.text.len()).max().unwrap_or(0);
        Source {
            records,
            next: 0,
            current: None,
            rest: VecDeque::new(),
            rest_place: String::new(),
            skipped: false,
            ended: false,
            length: longest.max(RECORD_LENGTH),
        }
    }

    fn take(&mut self) -> Option<(usize, Record)> {
        let record = self.records.get(self.next)?.clone();
        self.next += 1;
        Some((self.next - 1, record))
    }
}

/// What the engine that builds the cases knows besides the case.
#[derive(Clone, Default)]
pub(crate) struct InputState {
    /// By lower-case file name; inline data under "".
    sources: HashMap<String, Source>,
    last_source: Option<String>,
    /// REREAD: the source and the column.
    reread: Option<(String, usize)>,
    built: Vec<Dataset>,
    end_file: bool,
    /// Records read so far.
    read: usize,
    /// FILE TYPE: its source; a DATA LIST reads the record the RECORD TYPE matched.
    file_type: Option<String>,
    warnings: Vec<String>,
}

fn source_key(file: Option<&str>) -> String {
    file.map_or(String::new(), str::to_lowercase)
}

/// The error that unwinds the commands at END FILE or at the end of the data;
/// `InputState::end_file` tells it from a real error.
fn stop() -> Error {
    Error("the input program has ended".into())
}

/// The commands of a program that build cases themselves.
fn builds_cases(command: &Command) -> bool {
    match command {
        Command::EndCase | Command::RepeatingData(_) => true,
        Command::DoRepeat(repeat) => {
            (repeat.body.iter()).any(|raw| raw.name == "END CASE" || raw.name == "REPEATING DATA")
        }
        _ => false,
    }
}

/// A command that cannot stand inside the program, with why.
fn refused(program: &Program, command: &Command) -> Option<&'static str> {
    let input = matches!(program.kind, Kind::Input);
    Some(match command {
        Command::InputProgram
        | Command::FileType(_)
        | Command::EndInputProgram
        | Command::EndFileType => {
            "an input program or FILE TYPE cannot stand inside another; close the open one first"
        }
        Command::SelectIf(_) | Command::Sample(_) | Command::Filter { .. } | Command::Temporary => {
            "SELECT IF, SAMPLE, FILTER and TEMPORARY cannot stand inside an input program; put \
             them after its end"
        }
        Command::BeginData(_) => "BEGIN DATA comes after the end of the input program",
        Command::RecordType(_) if input => "RECORD TYPE belongs inside FILE TYPE",
        Command::EndCase if !input => {
            "END CASE is for INPUT PROGRAM; in FILE TYPE the record types build the cases"
        }
        Command::DataList(_) => return None,
        c if crate::output::stage(c) == crate::output::Stage::Pass => {
            "this command reads or replaces the data and cannot stand inside an input program"
        }
        _ => return None,
    })
}

/// The text of a record from `column` (1-based) on.
fn from_column(record: &Record, column: usize) -> Record {
    let mut start = (column - 1).min(record.text.len());
    while !record.text.is_char_boundary(start) {
        start += 1;
    }
    Record {
        place: record.place.clone(),
        text: record.text[start..].to_string(),
    }
}

fn same(a: &Read, b: &Read) -> bool {
    match (a, b) {
        (Read::Number(x), Read::Number(y)) => x == y,
        (Read::Text(x), Read::Text(y)) => x.trim_end() == y.trim_end(),
        _ => false,
    }
}

fn shown(read: &Read) -> String {
    match read {
        Read::Number(Some(n)) => n.to_string(),
        Read::Number(None) => ".".into(),
        Read::Text(t) => format!("'{}'", t.trim_end()),
    }
}

/// The first 80 characters of a record, for a warning.
fn head(record: &Record) -> String {
    record.text.chars().take(80).collect()
}

/// The identifier field of FILE TYPE, with other columns when `columns` says so.
fn id_field(id: &IdField, columns: Option<(u32, u32)>) -> DataField {
    let mut format = id.format;
    let mut column = id.column;
    if let Some((first, last)) = columns {
        column = first;
        format.width = (last - first + 1).min(32767) as u16;
    }
    DataField {
        name: id.name.clone().unwrap_or_default(),
        format,
        display: id.display,
        record: 1,
        column,
    }
}

fn read_id(field: &DataField, record: &Record, warnings: &mut Vec<String>) -> Read {
    let text = data_list::columns(&record.text, field);
    data_list::read(&text, field, true, &record.place, warnings)
}

/// One RECORD TYPE of a FILE TYPE with the commands after it.
struct Section {
    spec: RecordType,
    body: Vec<Command>,
    /// The variables its commands define.
    variables: Vec<String>,
    builds: bool,
}

impl Section {
    fn matches(&self, id: &Read) -> bool {
        self.spec.values.iter().any(|value| match (value, id) {
            (Value::Number(v), Read::Number(Some(n))) => v == n,
            (Value::String(v), Read::Text(t)) => v.trim_end() == t.trim_end(),
            _ => false,
        })
    }
}

impl Engine {
    /// INPUT PROGRAM and FILE TYPE, up to their end: the commands are collected, then run.
    /// None when the command is not part of such a block.
    pub(crate) fn collect_input(&mut self, command: &Command) -> Option<Result<Vec<Table>, Error>> {
        if self.input.is_some() || self.row.is_some() || self.loop_depth > 0 {
            return None;
        }
        let Some(program) = self.collecting_input.as_mut() else {
            let kind = match command {
                Command::InputProgram => Kind::Input,
                Command::FileType(file_type) => Kind::FileType(file_type.clone()),
                _ => return None,
            };
            return Some(self.open_program(kind));
        };
        let closes = matches!(
            (&program.kind, command),
            (Kind::Input, Command::EndInputProgram) | (Kind::FileType(_), Command::EndFileType)
        );
        if closes {
            let program = self.collecting_input.take().expect("checked above");
            return Some(self.close_program(program));
        }
        if let Some(why) = refused(program, command) {
            self.collecting_input = None;
            return Some(Err(Error(why.into())));
        }
        program.body.push(command.clone());
        Some(Ok(Vec::new()))
    }

    fn open_program(&mut self, kind: Kind) -> Result<Vec<Table>, Error> {
        if self.collecting.is_some() || !self.blocks.is_empty() || self.pending_depth > 0 {
            return Err(Error(
                "an input program or FILE TYPE cannot stand inside DO IF or LOOP".into(),
            ));
        }
        if self.pending_data.is_some() {
            return Err(Error(
                "the DATA LIST or input program before this command has no data yet: put BEGIN \
                 DATA, the data lines and END DATA right after it"
                    .into(),
            ));
        }
        // the transformations waiting for the data pass run on the dataset that goes
        self.flush()?;
        self.collecting_input = Some(Program {
            kind,
            body: Vec::new(),
        });
        Ok(Vec::new())
    }

    fn close_program(&mut self, program: Program) -> Result<Vec<Table>, Error> {
        self.park_active();
        self.dataset = None;
        self.scratch_assigned.clear();
        self.temporary = None;
        self.case_limit = None;
        self.leave.clear();
        self.vectors.clear();
        self.clean = None;
        self.pending = None;
        if program.inline() {
            self.pending_data = Some(Pending::Program(program));
            return Ok(Vec::new());
        }
        self.run_program(program, None)
    }

    /// Builds the new active dataset from the records.
    pub(crate) fn run_program(
        &mut self,
        program: Program,
        inline: Option<Vec<Record>>,
    ) -> Result<Vec<Table>, Error> {
        let mut sources = HashMap::new();
        if let Some(records) = inline {
            sources.insert(String::new(), Source::new(records));
        }
        for file in program.files() {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                sources.entry(source_key(Some(&file)))
            {
                entry.insert(Source::new(self.file_records(&file)?));
            }
        }
        let file_type = match &program.kind {
            Kind::FileType(file_type) => Some(source_key(file_type.file.as_deref())),
            Kind::Input => None,
        };
        let mut engine = Engine {
            dataset: Some(Dataset::new(0)),
            session: self.session.settings_only(),
            row: Some(Box::default()),
            input: Some(Box::new(InputState {
                sources,
                file_type,
                ..InputState::default()
            })),
            ..Engine::default()
        };
        engine.random = std::mem::take(&mut self.random);
        let result = match &program.kind {
            Kind::Input => engine.run_input(&program.body),
            Kind::FileType(file_type) => engine.run_file_type(file_type, &program.body),
        };
        self.random = std::mem::take(&mut engine.random);
        let (tables, template) = result?;
        let state = engine.input.take().expect("set above");
        let mut data = Dataset::from_parts(&template, &state.built)?;
        let scratch: Vec<String> = (data.variable_names())
            .filter(|n| is_scratch(n))
            .map(str::to_string)
            .collect();
        if scratch.len() == data.variable_count() {
            return Err(Error(format!(
                "the {} defines no variables to keep, only scratch variables",
                program.opener()
            )));
        }
        let names: Vec<&str> = scratch.iter().map(String::as_str).collect();
        data.delete_variables(&names)?;
        self.dataset = Some(data);
        self.default_measures();
        self.emit_case_output(std::mem::take(&mut engine.case_outputs))?;
        let mut seen: Vec<String> = Vec::new();
        for warning in state.warnings {
            if !seen.contains(&warning) {
                seen.push(warning);
            }
        }
        let hidden = seen.len().saturating_sub(data_list::SHOWN_WARNINGS);
        seen.truncate(data_list::SHOWN_WARNINGS);
        seen.into_iter().for_each(|w| self.warn(w));
        if hidden > 0 {
            self.warn(format!("{hidden} more warnings while reading the data"));
        }
        Ok(tables)
    }

    fn state(&mut self) -> &mut InputState {
        self.input
            .as_mut()
            .expect("only an input program engine comes here")
    }

    fn keep_warnings(&mut self) {
        let warnings = std::mem::take(&mut self.warnings);
        self.state().warnings.extend(warnings);
    }

    /// The commands on no case: the dictionary. Returns their tables.
    fn declare_all(&mut self, body: &[Command]) -> Result<Vec<Table>, Error> {
        let mut tables = Vec::new();
        for command in body {
            tables.extend(self.execute(command)?);
            self.keep_warnings();
        }
        self.check_closed()?;
        Ok(tables)
    }

    fn check_closed(&self) -> Result<(), Error> {
        if self.collecting.is_some() {
            return Err(Error(
                "LOOP without END LOOP inside the input program".into(),
            ));
        }
        if !self.blocks.is_empty() {
            return Err(Error(
                "DO IF without END IF inside the input program".into(),
            ));
        }
        Ok(())
    }

    /// Runs the commands on the case being built. True when the program has ended.
    fn run_commands(&mut self, body: &[Command]) -> Result<bool, Error> {
        for command in body {
            match self.execute(command) {
                Ok(_) => self.keep_warnings(),
                Err(_) if self.state().end_file => return Ok(true),
                Err(e) => return Err(e),
            }
        }
        self.check_closed()?;
        Ok(false)
    }

    /// The first case: blank, with scratch and LEAVE numbers at 0.
    fn start_case(&mut self, template: &Dataset) -> Result<(), Error> {
        let mut case = Dataset::from_parts(template, &[Dataset::new(1)])?;
        let zero: Vec<String> = (case.variables())
            .filter(|v| !v.is_string() && self.carried(v.name()))
            .map(|v| v.name().to_string())
            .collect();
        for name in zero {
            case.set_numeric_values(&name, vec![Some(0.0)])?;
        }
        self.dataset = Some(case);
        Ok(())
    }

    /// Every variable but the scratch and LEAVE ones is blank again.
    fn reset_case(&mut self) -> Result<(), Error> {
        let data = self.active()?;
        let names: Vec<(String, Option<usize>)> = (data.variables())
            .filter(|v| !self.carried(v.name()))
            .map(|v| (v.name().to_string(), v.string_width()))
            .collect();
        self.reset(&names)
    }

    fn reset(&mut self, names: &[(String, Option<usize>)]) -> Result<(), Error> {
        let data = self.active_mut()?;
        for (name, width) in names {
            match width {
                Some(width) => data.set_string_values(name, *width, vec![String::new()])?,
                None => data.set_numeric_values(name, vec![None])?,
            }
        }
        Ok(())
    }

    /// Adds the case as it is now to the dataset being built.
    fn build_case(&mut self) -> Result<(), Error> {
        let case = self.active()?.clone();
        self.state().built.push(case);
        if let Some(row) = self.row.as_mut() {
            row.case += 1;
        }
        Ok(())
    }

    /// True when the command runs for the case: there is one, and the open DO IF branch or
    /// loop holds it.
    fn open_case(&self) -> Result<bool, Error> {
        let one = self.active()?.case_count() == 1;
        Ok(one && self.active_cases().is_none_or(|active| active[0]))
    }

    fn in_program(&self, command: &str) -> Result<(), Error> {
        match self.input {
            Some(_) => Ok(()),
            None => Err(Error(format!(
                "{command} is only allowed inside INPUT PROGRAM or FILE TYPE"
            ))),
        }
    }

    fn run_input(&mut self, body: &[Command]) -> Result<(Vec<Table>, Dataset), Error> {
        let tables = self.declare_all(body)?;
        let template = self.active()?.clone();
        self.start_case(&template)?;
        let explicit = body.iter().any(builds_cases);
        loop {
            let read = self.state().read;
            if self.run_commands(body)? {
                break;
            }
            if !explicit {
                self.build_case()?;
            }
            if self.state().read == read {
                return Err(Error(
                    "the input program ran from top to bottom without reading a record or \
                     reaching END FILE, so it would never end: read data with DATA LIST, or end \
                     it with END FILE"
                        .into(),
                ));
            }
            self.reset_case()?;
        }
        Ok((tables, template))
    }

    pub(crate) fn end_case(&mut self) -> Result<(), Error> {
        self.in_program("END CASE")?;
        if !self.open_case()? {
            return Ok(());
        }
        self.build_case()?;
        self.reset_case()
    }

    pub(crate) fn end_file(&mut self) -> Result<(), Error> {
        self.in_program("END FILE")?;
        if !self.open_case()? {
            return Ok(());
        }
        self.state().end_file = true;
        Err(stop())
    }

    /// LEAVE: in an input program the variables are not reset; elsewhere the pass keeps them.
    pub(crate) fn leave(&mut self, items: &[VarItem]) -> Result<(), Error> {
        let mut names = Vec::new();
        for item in items {
            // a variable the waiting case-by-case transformations define is not there yet
            if let VarItem::Name(name) = item
                && self.input.is_none()
                && self
                    .active()
                    .is_ok_and(|d| d.variable_index(name).is_none())
                && self.defined_since_pass(name)
            {
                names.push(name.clone());
                continue;
            }
            names.extend(self.expand(std::slice::from_ref(item)).map_err(|e| {
                Error(format!(
                    "{}; LEAVE names variables that the commands before it define",
                    e.0
                ))
            })?);
        }
        if let Some(name) = names.iter().find(|n| is_scratch(n)) {
            return Err(Error(format!(
                "LEAVE cannot name the scratch variable {name}: a scratch variable keeps its \
                 value from case to case anyway"
            )));
        }
        if self.input.is_some() {
            self.leave.extend(names.iter().map(|n| n.to_lowercase()));
            return Ok(());
        }
        self.leave_in_pass(names)
    }

    /// The variables of a DATA LIST or REPEATING DATA, on the pass on no case.
    fn declare_fields(&mut self, fields: &[DataField], end: Option<&str>) -> Result<(), Error> {
        let data = self.active_mut()?;
        for field in fields {
            let string = field.format.kind == FormatType::A;
            if let Ok(variable) = data.variable(&field.name) {
                if variable.is_string() != string {
                    return Err(Error(format!(
                        "{} is read as a number in one place and as a string in another",
                        field.name
                    )));
                }
                continue;
            }
            if string {
                data.set_string_values(&field.name, usize::from(field.format.width), Vec::new())?;
            } else {
                data.set_numeric_values(&field.name, Vec::new())?;
                let shown = field.display;
                let text = format!("{}{}.{}", shown.kind.name(), shown.width, shown.decimals);
                data.set_format(&field.name, &text)?;
            }
        }
        if let Some(end) = end
            && data.variable_index(end).is_none()
        {
            data.set_numeric_values(end, Vec::new())?;
            data.set_format(end, "F1.0")?;
        }
        Ok(())
    }

    /// Puts the values read into the case.
    fn store(&mut self, fields: &[DataField], values: Vec<Read>) -> Result<(), Error> {
        let data = self.active_mut()?;
        for (field, value) in fields.iter().zip(values) {
            match value {
                Read::Number(v) => data.set_numeric_values(&field.name, vec![v])?,
                Read::Text(text) => {
                    let width = data.variable(&field.name)?.string_width().unwrap_or(1);
                    let text = crate::transform::fit(text, width);
                    data.set_string_values(&field.name, width, vec![text])?;
                }
            }
        }
        Ok(())
    }

    /// DATA LIST inside an input program: the next case of its data into the case being built.
    pub(crate) fn program_data_list(&mut self, list: &DataList) -> Result<(), Error> {
        if self.active()?.case_count() == 0 {
            return self.declare_fields(&list.fields, list.end.as_deref());
        }
        if !self.open_case()? {
            return Ok(());
        }
        let mut warnings = Vec::new();
        let state = self.state();
        if let Some(key) = state.file_type.clone() {
            if list.layout != (DataLayout::Fixed { records: 1 }) {
                return Err(Error(
                    "inside FILE TYPE a DATA LIST reads one record in fixed columns".into(),
                ));
            }
            let column = state.reread.take().map_or(1, |(_, column)| column);
            let source = &state.sources[&key];
            let index = source.current.expect("the record of the RECORD TYPE");
            let record = from_column(&source.records[index], column);
            let case = data_list::fixed(list, &[record], 1, &mut warnings).pop();
            state.warnings.extend(warnings);
            return self.store(&list.fields, case.unwrap_or_default());
        }
        let key = source_key(list.file.as_deref());
        let reread = match state.reread.take() {
            Some((source, column)) if source == key => Some(column),
            _ => None,
        };
        state.last_source = Some(key.clone());
        let Some(source) = state.sources.get_mut(&key) else {
            return Err(Error("DATA LIST has no data to read".into()));
        };
        if !source.skipped {
            source.skipped = true;
            source.next = (source.next + list.skip as usize).min(source.records.len());
        }
        let again = reread.and_then(|column| {
            let index = source.current?;
            Some((index, from_column(&source.records[index], column)))
        });
        let mut read = 0;
        let case: Option<Vec<Read>> = match list.layout {
            DataLayout::Fixed { records } => {
                let wanted = records as usize;
                let mut chunk = Vec::new();
                let mut first = None;
                if let Some((index, record)) = again {
                    first = Some(index);
                    chunk.push(record);
                }
                while chunk.len() < wanted {
                    let Some((index, record)) = source.take() else {
                        break;
                    };
                    read += 1;
                    first.get_or_insert(index);
                    chunk.push(record);
                }
                if chunk.len() < wanted {
                    if let Some(record) = chunk.first() {
                        warnings.push(format!(
                            "{}: the last case has {} of its {wanted} records and is left out",
                            record.place,
                            chunk.len()
                        ));
                    }
                    None
                } else {
                    source.current = first;
                    data_list::fixed(list, &chunk, records, &mut warnings).pop()
                }
            }
            DataLayout::List => {
                let record = again.or_else(|| {
                    read += 1;
                    source.take()
                });
                record.map(|(index, record)| {
                    source.current = Some(index);
                    let case = data_list::free(list, &[record], &mut warnings).pop();
                    case.unwrap_or_default()
                })
            }
            DataLayout::Free => {
                if let Some((_, record)) = again {
                    source.rest = data_list::values(&record.text, &list.delimiters).into();
                    source.rest_place = record.place;
                }
                let mut found = Vec::new();
                while found.len() < list.fields.len() {
                    if let Some(value) = source.rest.pop_front() {
                        found.push((value, source.rest_place.clone()));
                        continue;
                    }
                    let Some((index, record)) = source.take() else {
                        break;
                    };
                    read += 1;
                    source.current = Some(index);
                    source.rest = data_list::values(&record.text, &list.delimiters).into();
                    source.rest_place = record.place;
                }
                if found.len() < list.fields.len() {
                    if let Some((_, place)) = found.first() {
                        warnings.push(format!(
                            "{place}: the last case has {} of its {} values and is left out",
                            found.len(),
                            list.fields.len()
                        ));
                    }
                    None
                } else {
                    let values = found.into_iter().zip(&list.fields);
                    let read = |((value, place), field): ((Option<String>, String), _)| match value
                    {
                        Some(text) => data_list::read(&text, field, false, &place, &mut warnings),
                        None if field.format.kind == FormatType::A => Read::Text(String::new()),
                        None => Read::Number(None),
                    };
                    Some(values.map(read).collect())
                }
            }
        };
        state.read += read;
        state.warnings.extend(warnings);
        let Some(case) = case else {
            return self.end_of_data(list, &key);
        };
        self.store(&list.fields, case)?;
        if let Some(end) = &list.end {
            self.active_mut()?
                .set_numeric_values(end, vec![Some(0.0)])?;
        }
        Ok(())
    }

    /// No record is left for a DATA LIST: END= turns 1 the first time, else the program ends.
    fn end_of_data(&mut self, list: &DataList, key: &str) -> Result<(), Error> {
        let state = self.state();
        let source = state.sources.get_mut(key).expect("read above");
        if let Some(end) = &list.end
            && !source.ended
        {
            source.ended = true;
            // reaching the end counts as reading: the next run of the program sees END = 1
            state.read += 1;
            self.active_mut()?
                .set_numeric_values(end, vec![Some(1.0)])?;
            return Ok(());
        }
        state.end_file = true;
        Err(stop())
    }

    pub(crate) fn reread(
        &mut self,
        file: Option<&str>,
        column: Option<&Expr>,
    ) -> Result<(), Error> {
        self.in_program("REREAD")?;
        if !self.open_case()? {
            return Ok(());
        }
        let column = match column {
            None => 1,
            Some(expr) => match self.evaluator(None)?.eval(expr)? {
                Column::Number(values) => match values.first().copied().flatten() {
                    Some(c) if c >= 1.0 => c.min(1e6) as usize,
                    _ => {
                        self.warn("the COLUMN of REREAD is missing or below 1; column 1 is used");
                        1
                    }
                },
                Column::String(_) => {
                    return Err(Error("the COLUMN of REREAD must be a number".into()));
                }
            },
        };
        let state = self.state();
        let key = match file {
            Some(file) => source_key(Some(file)),
            None => match (&state.file_type, &state.last_source) {
                (Some(key), _) | (None, Some(key)) => key.clone(),
                (None, None) => {
                    return Err(Error(
                        "REREAD needs a DATA LIST before it that read a record".into(),
                    ));
                }
            },
        };
        state.reread = Some((key, column));
        Ok(())
    }

    /// The value of a STARTS, OCCURS or LENGTH place for the case.
    fn place_value(&self, place: &Place) -> Result<Option<f64>, Error> {
        match place {
            Place::Column(n) => Ok(Some(f64::from(*n))),
            Place::Variable(name) => Ok(self.active()?.variable(name)?.numeric()?.get(0)),
        }
    }

    pub(crate) fn repeating_data(&mut self, repeating: &RepeatingData) -> Result<(), Error> {
        self.in_program("REPEATING DATA")?;
        if self.active()?.case_count() == 0 {
            self.declare_fields(&repeating.fields, None)?;
            let places = [Some(&repeating.starts), repeating.ends.as_ref()];
            let places = places
                .into_iter()
                .chain([Some(&repeating.occurs), repeating.length.as_ref()])
                .flatten();
            let data = self.active()?;
            for place in places {
                if let Place::Variable(name) = place
                    && !data.variable(name).is_ok_and(|v| !v.is_string())
                {
                    return Err(Error(format!(
                        "REPEATING DATA: {name} must be a numeric variable defined before it"
                    )));
                }
            }
            if let Some((_, _, name)) = &repeating.id
                && data.variable_index(name).is_none()
            {
                return Err(Error(format!(
                    "REPEATING DATA: the ID variable {name} must be defined on the DATA LIST \
                     before it"
                )));
            }
            return Ok(());
        }
        if !self.open_case()? {
            return Ok(());
        }
        let Some(occurs) = self.place_value(&repeating.occurs)?.filter(|n| *n >= 1.0) else {
            return Ok(());
        };
        let mut count = occurs.min(1e6) as usize;
        let Some(starts) = self.place_value(&repeating.starts)?.filter(|n| *n >= 1.0) else {
            self.warn("the STARTS column of REPEATING DATA is missing; no cases from this record");
            return Ok(());
        };
        let ends = match &repeating.ends {
            None => None,
            Some(place) => match self.place_value(place)? {
                Some(end) => Some(end.max(0.0) as usize),
                None => {
                    self.warn(
                        "the end column on STARTS of REPEATING DATA is missing; no cases from \
                         this record",
                    );
                    return Ok(());
                }
            },
        };
        let mut length = repeating.group_length as usize;
        if let Some(place) = &repeating.length {
            match self.place_value(place)?.filter(|n| *n >= 1.0) {
                Some(n) => length = n.min(1e6) as usize,
                None => {
                    self.warn("the LENGTH of REPEATING DATA is missing; one case is built");
                    count = 1;
                }
            }
        }
        // the identifier of the input case, compared with the one on continuation records
        let id = match &repeating.id {
            Some((first, last, name)) => {
                let variable = self.active()?.variable(name)?;
                let value = match variable.string_width() {
                    Some(_) => Read::Text(variable.string()?.get(0).to_string()),
                    None => Read::Number(variable.numeric()?.get(0)),
                };
                let format = statscraft_syntax::Format {
                    kind: if variable.is_string() {
                        FormatType::A
                    } else {
                        FormatType::F
                    },
                    width: (last - first + 1).min(32767) as u16,
                    decimals: 0,
                };
                let field = DataField {
                    name: name.clone(),
                    format,
                    display: format,
                    record: 1,
                    column: *first,
                };
                Some((field, value))
            }
            None => None,
        };
        let mut warnings = Vec::new();
        let state = self.state();
        let key = match (&state.file_type, &state.last_source) {
            (Some(key), _) | (None, Some(key)) => key.clone(),
            (None, None) => {
                return Err(Error(
                    "REPEATING DATA needs a DATA LIST before it that read a record".into(),
                ));
            }
        };
        if repeating
            .file
            .as_deref()
            .is_some_and(|f| source_key(Some(f)) != key)
        {
            return Err(Error(
                "the FILE of REPEATING DATA must be the file its DATA LIST or FILE TYPE reads"
                    .into(),
            ));
        }
        let source = state.sources.get_mut(&key).expect("read before");
        let Some(first) = source.current else {
            return Err(Error(
                "REPEATING DATA needs a DATA LIST before it that read a record".into(),
            ));
        };
        let logical = source.length;
        let mut record = source.records[first].clone();
        let starts = starts.min(1e6) as usize;
        let mut position = starts;
        let mut limit = ends.unwrap_or(logical);
        if limit < starts {
            warnings.push(format!(
                "{}: the end column on STARTS comes before the start; only continuation records \
                 give cases",
                record.place
            ));
            position = limit + 1;
        }
        let mut groups: Vec<Vec<Read>> = Vec::new();
        let mut read = 0;
        while groups.len() < count {
            if let Some((start, end)) = repeating.continued
                && position + length - 1 > limit
            {
                let Some((_, next)) = source.take() else {
                    warnings.push(format!(
                        "{}: the data end before all {count} groups of the input case are read",
                        record.place
                    ));
                    break;
                };
                read += 1;
                record = next;
                if let Some((field, value)) = &id {
                    let found = read_id(field, &record, &mut warnings);
                    if !same(&found, value) {
                        return Err(Error(format!(
                            "{}: the ID {} of this continuation record is not {}, the ID of its \
                             input case; reading stops",
                            record.place,
                            shown(&found),
                            shown(value)
                        )));
                    }
                }
                position = start as usize;
                limit = end.map_or(logical, |end| end as usize);
                if position + length - 1 > limit {
                    return Err(Error(format!(
                        "{}: a repeating group of {length} columns does not fit in the columns \
                         CONTINUED gives",
                        record.place
                    )));
                }
                continue;
            }
            let values = repeating.fields.iter().map(|field| {
                let column = position + field.column as usize - 1;
                let width = usize::from(field.format.width);
                if field.format.kind != FormatType::A && column - 1 + width > logical {
                    warnings.push(format!(
                        "{}: {} lies past the end of the record and is system-missing",
                        record.place, field.name
                    ));
                    return Read::Number(None);
                }
                let at = DataField {
                    column: column as u32,
                    ..field.clone()
                };
                let text = data_list::columns(&record.text, &at);
                data_list::read(&text, field, true, &record.place, &mut warnings)
            });
            groups.push(values.collect());
            position += length;
        }
        state.read += read;
        state.warnings.extend(warnings);
        for values in groups {
            self.store(&repeating.fields, values)?;
            self.build_case()?;
        }
        Ok(())
    }

    fn run_file_type(
        &mut self,
        file_type: &FileType,
        body: &[Command],
    ) -> Result<(Vec<Table>, Dataset), Error> {
        let kind = file_type.kind;
        let mut sections: Vec<Section> = Vec::new();
        for command in body {
            match command {
                Command::RecordType(spec) => sections.push(Section {
                    spec: spec.clone(),
                    body: Vec::new(),
                    variables: Vec::new(),
                    builds: false,
                }),
                other => match sections.last_mut() {
                    Some(section) => section.body.push(other.clone()),
                    None => {
                        return Err(Error(
                            "FILE TYPE needs a RECORD TYPE before the commands that read a record"
                                .into(),
                        ));
                    }
                },
            }
        }
        check_sections(file_type, &sections)?;
        // the dictionary: the identifiers, then the variables of each record type in order
        let mut tables = Vec::new();
        for id in [Some(&file_type.record), file_type.case.as_ref()]
            .into_iter()
            .flatten()
        {
            if id.name.is_some() {
                self.declare_fields(&[id_field(id, None)], None)?;
            }
        }
        for section in &mut sections {
            let before: Vec<String> = self
                .active()?
                .variable_names()
                .map(str::to_lowercase)
                .collect();
            if kind != FileKind::Mixed {
                let lists = section.body.iter().filter_map(|c| match c {
                    Command::DataList(list) => Some(list),
                    _ => None,
                });
                for field in lists.flat_map(|l| &l.fields) {
                    if before.contains(&field.name.to_lowercase()) {
                        return Err(Error(format!(
                            "{} is defined for two record types; in a grouped or nested file \
                             each variable belongs to one record type",
                            field.name
                        )));
                    }
                }
            }
            tables.extend(self.declare_all(&section.body)?);
            section.variables = (self.active()?.variable_names())
                .filter(|n| !before.contains(&n.to_lowercase()))
                .map(str::to_string)
                .collect();
            section.builds = section.body.iter().any(builds_cases);
        }
        let template = self.active()?.clone();
        self.start_case(&template)?;
        let walk = Walk {
            file_type,
            sections: &sections,
        };
        match kind {
            FileKind::Mixed => self.mixed(&walk)?,
            FileKind::Grouped => self.grouped(&walk)?,
            FileKind::Nested => self.nested(&walk)?,
        }
        Ok((tables, template))
    }

    /// The next record of a FILE TYPE, made the current one, with the index of its RECORD
    /// TYPE: None for a type that is not defined (after a warning when WILD=WARN).
    fn next_record(&mut self, walk: &Walk) -> Option<(Record, Option<usize>, Read)> {
        let state = self.state();
        let key = state.file_type.clone().expect("a FILE TYPE");
        let source = state.sources.get_mut(&key).expect("read before");
        let (index, record) = source.take()?;
        source.current = Some(index);
        state.read += 1;
        let mut warnings = Vec::new();
        let id = read_id(
            &id_field(&walk.file_type.record, None),
            &record,
            &mut warnings,
        );
        let found = (walk.sections.iter()).position(|s| s.spec.values.is_empty() || s.matches(&id));
        if found.is_none() && walk.wild() {
            warnings.push(format!(
                "{}: record type {} is not defined: {}",
                record.place,
                shown(&id),
                head(&record)
            ));
        }
        state.warnings.extend(warnings);
        Some((record, found, id))
    }

    /// Puts an identifier read from the record into its variable, if it has one.
    fn set_id(&mut self, id: &IdField, value: Read) -> Result<(), Error> {
        if id.name.is_some() {
            self.store(&[id_field(id, None)], vec![value])?;
        }
        Ok(())
    }

    /// Runs the commands of a record type. True when END FILE ended the file.
    fn run_section(&mut self, section: &Section) -> Result<bool, Error> {
        self.run_commands(&section.body)
    }

    fn mixed(&mut self, walk: &Walk) -> Result<(), Error> {
        while let Some((_, found, id)) = self.next_record(walk) {
            let Some(s) = found.filter(|&s| !walk.sections[s].spec.skip) else {
                continue;
            };
            self.reset_case()?;
            self.set_id(&walk.file_type.record, id)?;
            if self.run_section(&walk.sections[s])? {
                break;
            }
            if !walk.sections[s].builds {
                self.build_case()?;
            }
        }
        Ok(())
    }

    fn grouped(&mut self, walk: &Walk) -> Result<(), Error> {
        let file_type = walk.file_type;
        let case = file_type.case.as_ref().expect("GROUPED has CASE");
        let sections = walk.sections;
        let mut group: Option<Read> = None;
        let mut seen = vec![false; sections.len()];
        let mut furthest = 0;
        let mut place = String::new();
        loop {
            let next = self.next_record(walk);
            let mut warnings = Vec::new();
            let record = match next {
                Some((record, Some(s), id)) if !sections[s].spec.skip => Some((record, s, id)),
                Some(_) => continue,
                None => None,
            };
            let case_id = record.as_ref().map(|(record, s, _)| {
                let field = id_field(case, sections[*s].spec.case);
                read_id(&field, record, &mut warnings)
            });
            let new_group = match (&group, &case_id) {
                (Some(old), Some(new)) => !same(old, new),
                _ => true,
            };
            if new_group && let Some(old) = &group {
                for (s, section) in sections.iter().enumerate() {
                    let warn = section.spec.missing.or(file_type.missing).unwrap_or(true);
                    if !seen[s] && !section.spec.skip && warn {
                        warnings.push(format!(
                            "{place}: case {} has no record of type {}",
                            shown(old),
                            values_text(&section.spec)
                        ));
                    }
                }
                self.build_case()?;
            }
            self.state().warnings.extend(warnings);
            let Some(((record, s, id), case_id)) = record.zip(case_id) else {
                break;
            };
            place = record.place.clone();
            if new_group {
                self.reset_case()?;
                group = Some(match &case_id {
                    Read::Number(n) => Read::Number(*n),
                    Read::Text(t) => Read::Text(t.clone()),
                });
                seen.iter_mut().for_each(|s| *s = false);
                furthest = 0;
                self.set_id(case, case_id)?;
            }
            let duplicate = sections[s].spec.duplicate.or(file_type.duplicate);
            if seen[s] && duplicate.unwrap_or(Duplicate::Warn) == Duplicate::Warn {
                let warning = format!(
                    "{}: a second record of this type for the case: {}",
                    place,
                    head(&record)
                );
                self.state().warnings.push(warning);
            }
            if file_type.ordered.unwrap_or(true) && s < furthest {
                let warning = format!(
                    "{}: the record is out of the order of the RECORD TYPE commands",
                    place
                );
                self.state().warnings.push(warning);
            }
            seen[s] = true;
            furthest = furthest.max(s);
            self.set_id(&file_type.record, id)?;
            if self.run_section(&sections[s])? {
                break;
            }
        }
        Ok(())
    }

    fn nested(&mut self, walk: &Walk) -> Result<(), Error> {
        let file_type = walk.file_type;
        let levels: Vec<usize> = (0..walk.sections.len())
            .filter(|&s| !walk.sections[s].spec.skip)
            .collect();
        let Some(&lowest) = levels.last() else {
            return Ok(());
        };
        let level_of = |s: usize| levels.iter().position(|&l| l == s);
        let blank = |engine: &Engine, from: usize| -> Vec<(String, Option<usize>)> {
            let data = engine.dataset.as_ref().expect("the case");
            let names = levels[from..]
                .iter()
                .flat_map(|&s| &walk.sections[s].variables);
            names
                .filter_map(|n| data.variable(n).ok())
                .map(|v| (v.name().to_string(), v.string_width()))
                .collect()
        };
        let mut set = vec![false; levels.len()];
        let mut used = vec![false; levels.len()];
        let mut top: Option<Read> = None;
        let duplicate = file_type.duplicate.unwrap_or(Duplicate::NoWarn);
        while let Some((record, found, id)) = self.next_record(walk) {
            let Some(level) = found.and_then(level_of) else {
                continue;
            };
            let s = levels[level];
            // records before the first one of the highest level are skipped
            if level > 0 && top.is_none() {
                continue;
            }
            let mut warnings = Vec::new();
            if let Some(case) = &file_type.case {
                let field = id_field(case, walk.sections[s].spec.case);
                let value = read_id(&field, &record, &mut warnings);
                if level == 0 {
                    top = Some(match &value {
                        Read::Number(n) => Read::Number(*n),
                        Read::Text(t) => Read::Text(t.clone()),
                    });
                    self.set_id(case, value)?;
                } else if let Some(top) = &top
                    && !same(top, &value)
                {
                    warnings.push(format!(
                        "{}: case identifier {} differs from {} on its highest-level record",
                        record.place,
                        shown(&value),
                        shown(top)
                    ));
                }
            } else if level == 0 {
                top = Some(Read::Number(None));
            }
            if s != lowest {
                if set[level] && !used[level] {
                    match duplicate {
                        Duplicate::Warn => warnings.push(format!(
                            "{}: a second record of this type with no record below the first: {}",
                            record.place,
                            head(&record)
                        )),
                        Duplicate::Case => {
                            let names = blank(self, level + 1);
                            self.reset(&names)?;
                            self.build_case()?;
                        }
                        Duplicate::NoWarn => {}
                    }
                }
                let names = blank(self, level);
                self.reset(&names)?;
                set[level..].iter_mut().for_each(|s| *s = false);
                used[level..].iter_mut().for_each(|u| *u = false);
                set[level] = true;
            } else {
                for (l, &above) in levels.iter().enumerate().take(level) {
                    let spec = &walk.sections[above].spec;
                    if !set[l] && spec.missing.or(file_type.missing).unwrap_or(false) {
                        warnings.push(format!(
                            "{}: no record of type {} above this one",
                            record.place,
                            values_text(spec)
                        ));
                    }
                }
                let names = blank(self, level);
                self.reset(&names)?;
            }
            self.state().warnings.extend(warnings);
            self.set_id(&file_type.record, id)?;
            if self.run_section(&walk.sections[s])? {
                break;
            }
            if s == lowest {
                if !walk.sections[s].builds {
                    self.build_case()?;
                }
                used.iter_mut().for_each(|u| *u = true);
                // SPREAD=NO: the values went to the first case only
                for (l, &s) in levels.iter().enumerate().take(level) {
                    if !walk.sections[s].spec.spread {
                        let names = blank(self, l);
                        let own: Vec<_> = (names.into_iter())
                            .filter(|(n, _)| walk.sections[s].variables.contains(n))
                            .collect();
                        self.reset(&own)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// A FILE TYPE with its record types, while its records are read.
struct Walk<'a> {
    file_type: &'a FileType,
    sections: &'a [Section],
}

impl Walk<'_> {
    fn wild(&self) -> bool {
        (self.file_type.wild).unwrap_or(self.file_type.kind == FileKind::Grouped)
    }
}

fn values_text(spec: &RecordType) -> String {
    let values = spec.values.iter().map(|v| match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("'{s}'"),
    });
    values.collect::<Vec<_>>().join(", ")
}

/// The rules of FILE TYPE and RECORD TYPE that need both.
fn check_sections(file_type: &FileType, sections: &[Section]) -> Result<(), Error> {
    let kind = file_type.kind;
    let fail = |message: &str| Err(Error(message.to_string()));
    if sections.is_empty() {
        return fail("FILE TYPE needs at least one RECORD TYPE");
    }
    let string = file_type.record.format.kind == FormatType::A;
    for (i, section) in sections.iter().enumerate() {
        let spec = &section.spec;
        let other = spec.values.is_empty();
        if other && i + 1 != sections.len() {
            return fail("RECORD TYPE OTHER must be the last RECORD TYPE");
        }
        if other && (file_type.wild).unwrap_or(kind == FileKind::Grouped) {
            return fail(
                "RECORD TYPE OTHER cannot go with WILD=WARN (the default for grouped files); \
                 give WILD=NOWARN",
            );
        }
        if kind == FileKind::Nested && other != spec.skip {
            return fail("in a nested file SKIP goes with OTHER only, and OTHER with SKIP");
        }
        if spec
            .values
            .iter()
            .any(|v| matches!(v, Value::String(_)) != string)
        {
            return fail(if string {
                "the record identifier is a string: give the RECORD TYPE values in quotes"
            } else {
                "the record identifier is a number: give the RECORD TYPE values as numbers"
            });
        }
        if kind == FileKind::Mixed
            && (spec.case.is_some() || spec.duplicate.is_some() || spec.missing.is_some())
        {
            return fail(
                "CASE, DUPLICATE and MISSING on RECORD TYPE are for grouped and nested files",
            );
        }
        if spec.case.is_some() && file_type.case.is_none() {
            return fail("CASE on RECORD TYPE needs CASE on FILE TYPE");
        }
        if spec.duplicate.is_some() && kind != FileKind::Grouped {
            return fail("DUPLICATE on RECORD TYPE is for grouped files");
        }
        if !spec.spread && kind != FileKind::Nested {
            return fail("SPREAD is for nested files");
        }
        if spec.skip && !section.body.is_empty() {
            return fail("a RECORD TYPE with SKIP takes no DATA LIST or other commands");
        }
    }
    Ok(())
}
