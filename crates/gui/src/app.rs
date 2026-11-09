//! The window. Drawing only: every change to data and every result goes through `Session::run`
//! with syntax built by the plain modules.

use crate::dialogs::{
    Adjust, CROSSTABS_CELLS, Compute, Crosstabs, CustomTables, DESCRIPTIVES_STATISTICS,
    Descriptives, Frequencies, NewValue, OldValue, Recode, RecodeRule, label_command,
};
use crate::edits::{self, Field};
use crate::output::{self, OutputItem};
use crate::syntax_text::quote;
use crate::{editor, grid};
use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, FocusDirection, FontId, Key, Modifiers, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextFormat, TextStyle, Ui, UiBuilder, pos2,
    text::LayoutJob, vec2,
};
use statscraft_data::Dataset;
use statscraft_engine::live::{self, Action, Request, Response};
use statscraft_engine::session::{Done, Failure, RunResult, Session};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// A request from `statscraft --session gui` and where its reply goes.
type Remote = (Request, mpsc::Sender<Response>);

/// What to do when a run finishes.
enum After {
    Show,
    Editor,
    OpenData(PathBuf),
    SaveData(PathBuf),
    CloseDialog,
    Remote(Remote),
}

/// A run on a worker thread, so a large file or a long run leaves the window responsive.
struct Busy {
    label: String,
    after: After,
    done: mpsc::Receiver<Done>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Data,
    Variables,
    Syntax,
    Output,
}

/// The rule being put together in the Recode dialog.
#[derive(Default)]
struct RuleDraft {
    old: usize,
    low: String,
    high: String,
    new: usize,
    value: String,
}

enum Dialog {
    Frequencies(Frequencies),
    Descriptives(Descriptives),
    Crosstabs(Crosstabs),
    CustomTables(CustomTables),
    Compute(Compute, String),
    Recode(Recode, RuleDraft),
}

impl Dialog {
    fn title(&self) -> &'static str {
        match self {
            Dialog::Frequencies(_) => "Frequencies",
            Dialog::Descriptives(_) => "Descriptives",
            Dialog::Crosstabs(_) => "Crosstabs",
            Dialog::CustomTables(_) => "Custom Tables",
            Dialog::Compute(..) => "Compute Variable",
            Dialog::Recode(..) => "Recode",
        }
    }

    fn reset(&mut self) {
        *self = match self {
            Dialog::Frequencies(_) => Dialog::Frequencies(Default::default()),
            Dialog::Descriptives(_) => Dialog::Descriptives(Default::default()),
            Dialog::Crosstabs(_) => Dialog::Crosstabs(Default::default()),
            Dialog::CustomTables(_) => Dialog::CustomTables(Default::default()),
            Dialog::Compute(..) => Dialog::Compute(Default::default(), String::new()),
            Dialog::Recode(..) => Dialog::Recode(Default::default(), Default::default()),
        }
    }

    fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            Dialog::Frequencies(d) => d.syntax(),
            Dialog::Descriptives(d) => d.syntax(),
            Dialog::Crosstabs(d) => d.syntax(),
            Dialog::CustomTables(d) => d.syntax(),
            Dialog::Compute(d, label) => {
                let mut text = d.syntax()?;
                if let Some(label) = label_command(&d.target, label) {
                    text.push('\n');
                    text.push_str(&label);
                }
                Ok(text)
            }
            Dialog::Recode(d, _) => {
                let strings = d
                    .variables
                    .iter()
                    .filter(|name| data.variable(name).is_ok_and(|v| v.is_string()))
                    .count();
                if strings > 0 && strings < d.variables.len() {
                    return Err("recode string and numeric variables separately".into());
                }
                d.syntax(strings > 0)
            }
        }
    }
}

enum DialogAction {
    Ok,
    Paste,
    Reset,
    Cancel,
}

struct CellEdit {
    row: usize,
    column: usize,
    text: String,
    original: String,
    started: bool,
}

#[derive(Default)]
struct GridEvents {
    clicked: Option<(usize, usize)>,
    double_clicked: Option<(usize, usize)>,
    /// Rows in view, for Page Up and Page Down.
    page: usize,
}

/// The keys that move the selected cell of the Data View.
const GRID_KEYS: [Key; 9] = [
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::PageUp,
    Key::PageDown,
    Key::Home,
    Key::End,
    Key::Tab,
];

pub struct App {
    session: Session,
    tab: Tab,
    data_file: Option<PathBuf>,
    value_labels: bool,
    show_log: bool,
    selected: Option<(usize, usize)>,
    /// Scroll the selected cell into view on the next frame.
    follow: bool,
    page: usize,
    cell_edit: Option<CellEdit>,
    variable_selected: Option<(usize, usize)>,
    field_edit: Option<CellEdit>,
    syntax: String,
    syntax_file: Option<PathBuf>,
    syntax_errors: Vec<Failure>,
    selection: (usize, usize),
    show_help: bool,
    help_for: Option<(u32, usize)>,
    help_text: String,
    output: Vec<OutputItem>,
    table_widths: HashMap<(usize, usize), Vec<f32>>,
    scroll_to: Option<(usize, usize)>,
    dialog: Option<Dialog>,
    picked: Vec<String>,
    filter: String,
    status: Result<String, String>,
    title: String,
    requests: mpsc::Receiver<Remote>,
    busy: Option<Busy>,
    ctx: egui::Context,
    /// The listener for `--session gui`; dropping it removes the endpoint file.
    server: Option<live::Server>,
}

impl App {
    /// Arguments: data files (.sav, .zsav) are opened, syntax files (.sps) loaded into the
    /// syntax editor.
    pub fn new(args: &[String], ctx: &egui::Context) -> Self {
        let (sender, requests) = mpsc::channel::<Remote>();
        let app_ctx = ctx.clone();
        let ctx = ctx.clone();
        // the listener thread hands each request to the window and waits for the reply
        let server = live::serve(live::endpoint_file(), move |request| {
            let (reply, answer) = mpsc::channel();
            sender
                .send((request, reply))
                .map_err(|_| vec![Failure::plain("session", "the window is closing")])?;
            ctx.request_repaint();
            answer.recv().unwrap_or_else(|_| {
                Err(vec![Failure::plain(
                    "session",
                    "the window closed during the run",
                )])
            })
        });
        let status = match &server {
            Ok(_) => Ok("Ready".into()),
            Err(e) => Err(format!("the command line cannot reach this window: {e}")),
        };
        let mut app = App {
            session: Session::default(),
            tab: Tab::Data,
            data_file: None,
            value_labels: true,
            show_log: true,
            selected: None,
            follow: false,
            page: 20,
            cell_edit: None,
            variable_selected: None,
            field_edit: None,
            syntax: String::new(),
            syntax_file: None,
            syntax_errors: Vec::new(),
            selection: (0, 0),
            show_help: true,
            help_for: None,
            help_text: String::new(),
            output: Vec::new(),
            table_widths: HashMap::new(),
            scroll_to: None,
            dialog: None,
            picked: Vec::new(),
            filter: String::new(),
            status,
            title: "StatsCraft".into(),
            requests,
            busy: None,
            ctx: app_ctx,
            server: server.ok(),
        };
        for arg in args {
            let path = PathBuf::from(arg);
            if is_syntax_file(&path) {
                app.load_syntax(path);
            } else {
                app.open_data(path);
            }
        }
        app
    }

    /// Starts syntax on a worker thread; `finish` shows the result. One run at a time: requests
    /// from the command line wait their turn, the window's own actions are turned away.
    fn run(&mut self, source: &str, lines: Option<(u32, u32)>, after: After) {
        if self.busy.is_some() {
            self.status = Err("wait for the running command to finish".into());
            return;
        }
        let check = matches!(&after, After::Remote((r, _)) if r.action == Action::Check);
        let job = self.session.job(source, lines, check);
        let label = match &after {
            After::OpenData(path) => format!("Opening {}", path.display()),
            _ => format!("Running {}", job.label()),
        };
        let (sender, done) = mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(job.execute());
            ctx.request_repaint();
        });
        self.busy = Some(Busy { label, after, done });
    }

    /// Takes a finished run off the worker thread, then the next request from the command line.
    fn poll(&mut self) {
        if let Some(busy) = &self.busy {
            let result = match busy.done.try_recv() {
                Ok(done) => self.session.apply(done),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err(vec![Failure::plain("run", "the run stopped unexpectedly")])
                }
            };
            let busy = self.busy.take().expect("checked above");
            self.finish(busy.after, result);
        }
        while self.busy.is_none()
            && let Ok(remote) = self.requests.try_recv()
        {
            self.answer(remote);
        }
    }

    /// Answers `statscraft run --session gui` and the like. A run shows in the window like one
    /// of its own; describe and check change nothing.
    fn answer(&mut self, (mut request, reply): Remote) {
        if request.action == Action::Describe {
            let _ = reply.send(live::describe(&self.session, request.json));
            return;
        }
        request.enter_folder();
        let source = std::mem::take(&mut request.source);
        let lines = request.lines;
        self.run(&source, lines, After::Remote((request, reply)));
    }

    fn finish(&mut self, after: After, mut result: Result<RunResult, Vec<Failure>>) {
        match after {
            After::Remote((request, reply)) => {
                let _ = reply.send(live::reply(&mut result, &request));
                if request.action == Action::Check {
                    self.status = match &result {
                        Ok(r) => Ok(format!(
                            "checked {} command(s) from the command line",
                            r.commands.len()
                        )),
                        Err(errors) => Err(error_text(errors)),
                    };
                    return;
                }
                // the data may have changed under an open cell editor
                self.cell_edit = None;
                self.field_edit = None;
                self.show(result, false);
            }
            After::Show => {
                self.show(result, false);
            }
            After::Editor => {
                self.show(result, true);
            }
            After::OpenData(path) => {
                if self.show(result, false) {
                    self.data_file = Some(path);
                    self.selected = None;
                    self.cell_edit = None;
                    self.field_edit = None;
                    self.tab = Tab::Data;
                }
            }
            After::SaveData(path) => {
                if self.show(result, false) {
                    self.data_file = Some(path);
                }
            }
            After::CloseDialog => {
                if self.show(result, false) {
                    self.dialog = None;
                }
            }
        }
    }

    /// Puts the tables of a run in the output, or its errors in the status bar and, for the
    /// syntax editor, in the editor's error list.
    fn show(&mut self, result: Result<RunResult, Vec<Failure>>, from_editor: bool) -> bool {
        match result.map(|r| r.commands) {
            Ok(outputs) => {
                if from_editor {
                    self.syntax_errors.clear();
                }
                let first_new = self.output.len();
                for out in outputs.into_iter().filter(|o| !o.tables.is_empty()) {
                    let heading = match (from_editor, out.first_line == out.last_line) {
                        (false, _) => out.name,
                        (true, true) => format!("{}, line {}", out.name, out.first_line),
                        (true, false) => {
                            format!("{}, lines {}-{}", out.name, out.first_line, out.last_line)
                        }
                    };
                    self.output.push(OutputItem {
                        heading,
                        tables: out.tables,
                    });
                }
                if self.output.len() > first_new {
                    self.tab = Tab::Output;
                    self.scroll_to = Some((first_new, 0));
                }
                let summary = self.session.log.last().and_then(|e| e.result.as_ref().ok());
                self.status = Ok(summary.cloned().unwrap_or_default());
                true
            }
            Err(errors) => {
                self.status = Err(error_text(&errors));
                if from_editor {
                    self.syntax_errors = errors;
                }
                false
            }
        }
    }

    fn open_data(&mut self, path: PathBuf) {
        let syntax = format!("GET FILE={}.", quote(&path.to_string_lossy()));
        self.run(&syntax, None, After::OpenData(path));
    }

    fn save_data(&mut self, path: PathBuf) {
        let syntax = format!("SAVE OUTFILE={}.", quote(&path.to_string_lossy()));
        self.run(&syntax, None, After::SaveData(path));
    }

    fn load_syntax(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.syntax = text;
                self.syntax_file = Some(path);
                self.syntax_errors.clear();
                self.tab = Tab::Syntax;
            }
            Err(e) => self.status = Err(format!("{}: {e}", path.display())),
        }
    }

    fn save_syntax(&mut self, path: PathBuf) {
        match std::fs::write(&path, &self.syntax) {
            Ok(()) => {
                self.status = Ok(format!("syntax saved to {}", path.display()));
                self.syntax_file = Some(path);
            }
            Err(e) => self.status = Err(format!("{}: {e}", path.display())),
        }
    }

    fn menu(&mut self, ui: &mut Ui) {
        let has_data = self.session.dataset().is_some();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open data...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Data files", &["sav", "zsav"])
                        .pick_file()
                {
                    self.open_data(path);
                }
                if ui
                    .add_enabled(has_data, egui::Button::new("Save data"))
                    .clicked()
                {
                    match self.data_file.clone() {
                        Some(path) => self.save_data(path),
                        None => self.save_data_as(),
                    }
                }
                if ui
                    .add_enabled(has_data, egui::Button::new("Save data as..."))
                    .clicked()
                {
                    self.save_data_as();
                }
                ui.separator();
                if ui.button("New syntax").clicked() {
                    self.syntax.clear();
                    self.syntax_file = None;
                    self.syntax_errors.clear();
                    self.tab = Tab::Syntax;
                }
                if ui.button("Open syntax...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Syntax", &["sps"])
                        .pick_file()
                {
                    self.load_syntax(path);
                }
                if ui.button("Save syntax").clicked() {
                    match self.syntax_file.clone() {
                        Some(path) => self.save_syntax(path),
                        None => self.save_syntax_as(),
                    }
                }
                if ui.button("Save syntax as...").clicked() {
                    self.save_syntax_as();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !self.output.is_empty(),
                        egui::Button::new("Export output as HTML..."),
                    )
                    .clicked()
                {
                    self.export_html();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.value_labels, "Value labels in Data View");
                ui.checkbox(&mut self.show_log, "Log");
                ui.checkbox(
                    &mut self.show_help,
                    "Command help in the syntax editor (F1)",
                );
            });
            ui.menu_button("Analyze", |ui| {
                for (label, dialog) in [
                    ("Frequencies...", Dialog::Frequencies(Default::default())),
                    ("Descriptives...", Dialog::Descriptives(Default::default())),
                    ("Crosstabs...", Dialog::Crosstabs(Default::default())),
                    ("Custom Tables...", Dialog::CustomTables(Default::default())),
                ] {
                    if ui.add_enabled(has_data, egui::Button::new(label)).clicked() {
                        self.open_dialog(dialog);
                    }
                }
            });
            ui.menu_button("Transform", |ui| {
                if ui
                    .add_enabled(has_data, egui::Button::new("Compute Variable..."))
                    .clicked()
                {
                    self.open_dialog(Dialog::Compute(Default::default(), String::new()));
                }
                if ui
                    .add_enabled(has_data, egui::Button::new("Recode..."))
                    .clicked()
                {
                    self.open_dialog(Dialog::Recode(Default::default(), Default::default()));
                }
            });
        });
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.picked.clear();
        self.filter.clear();
    }

    fn save_data_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Data file", &["sav"])
            .add_filter("Compressed data file", &["zsav"])
            .save_file()
        {
            self.save_data(path);
        }
    }

    fn save_syntax_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Syntax", &["sps"])
            .save_file()
        {
            self.save_syntax(path);
        }
    }

    fn export_html(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Web page", &["html"])
            .save_file()
        {
            self.status = match std::fs::write(&path, output::html(&self.output)) {
                Ok(()) => Ok(format!("output exported to {}", path.display())),
                Err(e) => Err(format!("{}: {e}", path.display())),
            };
        }
    }

    fn tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Data, "Data"),
                (Tab::Variables, "Variables"),
                (Tab::Syntax, "Syntax"),
                (Tab::Output, "Output"),
            ] {
                ui.selectable_value(&mut self.tab, tab, RichText::new(label).size(15.0));
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if let Some(data) = self.session.dataset() {
                    ui.weak(format!(
                        "{} cases, {} variables",
                        data.case_count(),
                        data.variable_count()
                    ));
                    if let Some(name) = self.data_file.as_ref().and_then(|p| p.file_name()) {
                        ui.label(name.to_string_lossy());
                    }
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut Ui) {
        if let Some(busy) = &self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&busy.label);
            });
            return;
        }
        match &self.status {
            Ok(text) => ui.weak(text),
            Err(text) => ui.colored_label(ui.visuals().error_fg_color, text),
        };
    }

    fn log(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("Log");
            ui.weak("every command the window ran, in order");
            if ui.small_button("Copy syntax").clicked() {
                let text: Vec<&str> = self
                    .session
                    .log
                    .iter()
                    .filter(|e| e.result.is_ok())
                    .map(|e| e.syntax.as_str())
                    .collect();
                ui.ctx().copy_text(text.join("\n") + "\n");
            }
        });
        ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in &self.session.log {
                    ui.add(egui::Label::new(RichText::new(&entry.syntax).monospace()).wrap());
                    match &entry.result {
                        Ok(summary) => ui.weak(summary),
                        Err(errors) => {
                            ui.colored_label(ui.visuals().error_fg_color, error_text(errors))
                        }
                    };
                    ui.add_space(4.0);
                }
            });
    }

    fn no_data(ui: &mut Ui) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.heading("No data yet");
            ui.label("Open a .sav or .zsav file from the File menu, or run GET FILE in the syntax editor.");
        });
    }

    fn data_view(&mut self, ui: &mut Ui) {
        if self.session.dataset().is_none() {
            return Self::no_data(ui);
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.value_labels, "Show value labels");
            ui.weak(
                "Double-click a cell or press Enter to change it; arrows, Tab and Page keys move. The change runs as syntax and shows in the log.",
            );
        });
        let data = self.session.dataset().expect("checked above");
        let char_width = TextStyle::Body.resolve(ui.style()).size * 0.6;
        let labels = self.value_labels;
        let widths: Vec<f32> = data
            .variables()
            .map(|v| {
                let width = v
                    .format()
                    .trim_start_matches(|c: char| c.is_ascii_alphabetic());
                let mut width = width
                    .split('.')
                    .next()
                    .and_then(|w| w.parse().ok())
                    .unwrap_or(8);
                if labels {
                    let longest = v.value_labels().iter().map(|(_, l)| l.chars().count());
                    width = width.max(longest.max().unwrap_or(0));
                }
                grid::column_width(v.name(), width, char_width)
            })
            .collect();
        let headers: Vec<String> = data.variable_names().map(str::to_string).collect();
        // numbers align right, value labels and strings left
        let cell = |r: usize, c: usize| {
            let variable = data.variable_at(c);
            let plain = edits::cell_text(&variable, r, false);
            if !labels || variable.value_labels().is_empty() {
                return (plain, !variable.is_string());
            }
            let text = edits::cell_text(&variable, r, true);
            let right = !variable.is_string() && text == plain;
            (text, right)
        };
        let mut commit = false;
        let mut cancel = false;
        let edit = &mut self.cell_edit;
        let events = paint_grid(
            ui,
            "data",
            &headers,
            &widths,
            data.case_count(),
            self.selected,
            std::mem::take(&mut self.follow),
            &cell,
            edit.as_ref().map(|e| (e.row, e.column)),
            &mut |ui, rect| {
                if let Some(e) = edit.as_mut() {
                    (commit, cancel) = cell_editor(ui, rect, e);
                }
            },
        );
        self.page = events.page.max(1);
        if let Some(at) = events.clicked {
            self.selected = Some(at);
        }
        if let Some(at) = events.double_clicked {
            self.edit_cell(at);
        }
        if cancel {
            self.cell_edit = None;
        }
        if commit {
            self.commit_cell();
        }
    }

    fn edit_cell(&mut self, (row, column): (usize, usize)) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let text = edits::edit_text(&data.variable_at(column), row);
        self.selected = Some((row, column));
        self.cell_edit = Some(CellEdit {
            row,
            column,
            original: text.clone(),
            text,
            started: false,
        });
    }

    fn commit_cell(&mut self) {
        let (Some(e), Some(data)) = (self.cell_edit.take(), self.session.dataset()) else {
            return;
        };
        if e.text == e.original {
            return;
        }
        match edits::cell_edit(&data.variable_at(e.column), e.row, &e.text) {
            Ok(syntax) => self.run(&syntax, None, After::Show),
            Err(message) => self.status = Err(message),
        }
    }

    /// The keyboard in the Data View, read before any widget sees the keys: arrows, Page Up
    /// and Down, Home and End (with Ctrl the first and last cell), Tab and Shift+Tab, Enter to
    /// edit. While a cell is edited, Tab and the up and down arrows commit it and move on;
    /// Enter commits and Escape cancels in the editor itself.
    fn grid_keys(&mut self, ui: &mut Ui) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let (rows, columns) = (data.case_count(), data.variable_count());
        let editing = self.cell_edit.is_some();
        if self.tab != Tab::Data
            || self.dialog.is_some()
            || rows == 0
            || columns == 0
            || (!editing && ui.ctx().egui_wants_keyboard_input())
        {
            return;
        }
        let keys: &[Key] = if editing {
            &[Key::Tab, Key::ArrowUp, Key::ArrowDown]
        } else {
            &GRID_KEYS
        };
        let mut moves = Vec::new();
        let mut enter = false;
        ui.input_mut(|i| {
            for &key in keys {
                if i.consume_key(Modifiers::COMMAND, key) {
                    moves.push((key, true, false));
                } else if i.consume_key(Modifiers::SHIFT, key) {
                    moves.push((key, false, true));
                } else if i.consume_key(Modifiers::NONE, key) {
                    moves.push((key, false, false));
                }
            }
            enter = !editing && i.consume_key(Modifiers::NONE, Key::Enter);
        });
        if moves.is_empty() && !enter {
            return;
        }
        // no egui focus traversal with these keys: the grid has them
        ui.memory_mut(|m| m.move_focus(FocusDirection::None));
        if editing {
            self.commit_cell();
        }
        let mut at = self.selected.unwrap_or((0, 0));
        for (key, ctrl, shift) in moves {
            at = grid::moved(key, ctrl, shift, at, (rows, columns), self.page);
        }
        self.selected = Some(at);
        self.follow = true;
        if enter {
            self.edit_cell(at);
        }
    }

    fn variable_view(&mut self, ui: &mut Ui) {
        if self.session.dataset().is_none() {
            return Self::no_data(ui);
        }
        ui.weak("Double-click a cell to change it. Value labels are written as 1 = Yes; 2 = No, missing values as 98, 99 or LO THRU -1, 9.");
        let data = self.session.dataset().expect("checked above");
        let headers: Vec<String> = Field::ALL.iter().map(|f| f.title().to_string()).collect();
        let widths = [150.0, 80.0, 90.0, 60.0, 72.0, 260.0, 300.0, 170.0, 100.0];
        let cell = |r: usize, c: usize| {
            let field = Field::ALL[c];
            let text = edits::field_text(&data.variable_at(r), field);
            let right = matches!(field, Field::Width | Field::Decimals);
            (text, right)
        };
        let mut commit = false;
        let mut cancel = false;
        let edit = &mut self.field_edit;
        let events = paint_grid(
            ui,
            "variables",
            &headers,
            &widths,
            data.variable_count(),
            self.variable_selected,
            false,
            &cell,
            edit.as_ref().map(|e| (e.row, e.column)),
            &mut |ui, rect| {
                let Some(e) = edit.as_mut() else { return };
                if Field::ALL[e.column] != Field::Measure {
                    (commit, cancel) = cell_editor(ui, rect, e);
                    return;
                }
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ComboBox::new("measure_edit", "")
                        .selected_text(e.text.clone())
                        .width(rect.width() - 8.0)
                        .show_ui(ui, |ui| {
                            for level in ["Nominal", "Ordinal", "Scale"] {
                                if ui.selectable_label(e.text == level, level).clicked() {
                                    e.text = level.into();
                                    commit = true;
                                }
                            }
                        });
                });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            },
        );
        if let Some(at) = events.clicked {
            self.variable_selected = Some(at);
            // a text edit commits when it loses focus; the measure list just closes
            let measure = |e: &CellEdit| Field::ALL[e.column] == Field::Measure;
            if self
                .field_edit
                .as_ref()
                .is_some_and(|e| measure(e) && (e.row, e.column) != at)
            {
                self.field_edit = None;
            }
        }
        if let Some((row, column)) = events.double_clicked {
            let variable = data.variable_at(row);
            let field = Field::ALL[column];
            self.variable_selected = Some((row, column));
            if field.editable(variable.is_string()) {
                let text = edits::field_text(&variable, field);
                self.field_edit = Some(CellEdit {
                    row,
                    column,
                    original: text.clone(),
                    text,
                    started: false,
                });
            } else {
                self.status = Err(format!("{} cannot be changed here yet", field.title()));
            }
        }
        if cancel {
            self.field_edit = None;
        }
        if commit && let Some(e) = self.field_edit.take() {
            let data = self.session.dataset().expect("checked above");
            match edits::field_edit(&data.variable_at(e.row), Field::ALL[e.column], &e.text) {
                Ok(Some(syntax)) => {
                    self.run(&syntax, None, After::Show);
                }
                Ok(None) => {}
                Err(message) => self.status = Err(message),
            }
        }
    }

    fn syntax_view(&mut self, ui: &mut Ui) {
        let run_selection_key = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        if ui.input(|i| i.key_pressed(Key::F1)) {
            self.show_help = !self.show_help;
        }
        let mut run_all = false;
        let mut run_selection = run_selection_key;
        ui.horizontal(|ui| {
            run_all = ui.button("Run all").clicked();
            run_selection |= ui
                .button("Run selection")
                .on_hover_text("Runs the commands on the selected lines, or the command at the cursor (Ctrl+Enter)")
                .clicked();
            ui.toggle_value(&mut self.show_help, "Help (F1)");
            let name = self
                .syntax_file
                .as_ref()
                .map_or("unsaved syntax".into(), |p| p.display().to_string());
            ui.weak(name);
        });

        if self.show_help {
            egui::Panel::right("help")
                .resizable(true)
                .default_size(420.0)
                .min_size(240.0)
                .show(ui, |ui| {
                    // the help text is laid out in lines and indents, so it scrolls instead of wrapping
                    ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&self.help_text).monospace()).extend(),
                        );
                    });
                });
        }
        if !self.syntax_errors.is_empty() {
            egui::Panel::bottom("syntax_errors")
                .resizable(true)
                .default_size(80.0)
                .show(ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        for error in &self.syntax_errors {
                            ui.colored_label(ui.visuals().error_fg_color, error.text());
                        }
                    });
                });
        }

        let font = FontId::monospace(14.0);
        let error_lines: Vec<u32> = self.syntax_errors.iter().filter_map(|e| e.line).collect();
        let line_count = self.syntax.split('\n').count().max(1);
        let mut gutter = LayoutJob::default();
        for line in 1..=line_count {
            let error = error_lines.contains(&(line as u32));
            let color = if error {
                ui.visuals().error_fg_color
            } else {
                ui.visuals().weak_text_color()
            };
            let mark = if error { "!" } else { " " };
            gutter.append(
                &format!("{mark}{line:>4}\n"),
                0.0,
                TextFormat::simple(font.clone(), color),
            );
        }
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
            let job = LayoutJob::simple(
                text.as_str().to_owned(),
                FontId::monospace(14.0),
                ui.visuals().text_color(),
                f32::INFINITY,
            );
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.label(gutter);
                    });
                    let output = TextEdit::multiline(&mut self.syntax)
                        .id_salt("syntax_editor")
                        .code_editor()
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .desired_rows(30)
                        .margin(vec2(6.0, 4.0))
                        .hint_text("Type syntax here, for example FREQUENCIES VARIABLES=gender.")
                        .layouter(&mut layouter)
                        .show(ui);
                    if let Some(range) = output.cursor_range {
                        self.selection = (range.secondary.index.into(), range.primary.index.into());
                    }
                });
            });
        });

        // help follows the command under the cursor
        let line = editor::line_of(&self.syntax, self.selection.0.max(self.selection.1));
        let key = (line, self.syntax.len());
        if self.show_help && self.help_for != Some(key) {
            self.help_for = Some(key);
            let command = editor::command_at(&self.syntax, line);
            self.help_text = statscraft_syntax::help(command.as_deref()).unwrap_or_default();
        }

        if run_all {
            let source = self.syntax.clone();
            self.run(&source, None, After::Editor);
        } else if run_selection {
            let lines = editor::selected_lines(&self.syntax, self.selection.0, self.selection.1);
            let lines = editor::whole_commands(&self.syntax, lines);
            let source = self.syntax.clone();
            self.run(&source, Some(lines), After::Editor);
        }
    }

    fn output_view(&mut self, ui: &mut Ui) {
        if self.output.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.heading("No output yet");
                ui.label("Results of FREQUENCIES, CROSSTABS, CTABLES and the other procedures appear here.");
            });
            return;
        }
        egui::Panel::left("outline")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Outline");
                    if ui.small_button("Export HTML...").clicked() {
                        self.export_html();
                    }
                    if ui.small_button("Clear").clicked() {
                        self.output.clear();
                        self.table_widths.clear();
                    }
                });
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    for (i, item) in self.output.iter().enumerate() {
                        if ui
                            .selectable_label(false, RichText::new(&item.heading).strong())
                            .clicked()
                        {
                            self.scroll_to = Some((i, 0));
                        }
                        for (t, table) in item.tables.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                if ui.selectable_label(false, &table.title).clicked() {
                                    self.scroll_to = Some((i, t));
                                }
                            });
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                for (i, item) in self.output.iter().enumerate() {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&item.heading).weak().size(13.0));
                    for (t, table) in item.tables.iter().enumerate() {
                        let top = ui.cursor().min;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&table.title).strong().size(15.0));
                            if ui
                                .small_button("Copy")
                                .on_hover_text("Copy as tab-separated text, for a spreadsheet")
                                .clicked()
                            {
                                ui.ctx().copy_text(output::tsv(table));
                            }
                        });
                        let widths = self
                            .table_widths
                            .entry((i, t))
                            .or_insert_with(|| measure_table(ui, table));
                        paint_table(ui, table, widths);
                        if let Some(caption) = &table.caption {
                            ui.add(egui::Label::new(RichText::new(caption).weak()).wrap());
                        }
                        if self.scroll_to == Some((i, t)) {
                            ui.scroll_to_rect(
                                Rect::from_min_size(top, vec2(1.0, 40.0)),
                                Some(Align::TOP),
                            );
                            self.scroll_to = None;
                        }
                        ui.add_space(14.0);
                    }
                }
            });
        });
    }

    fn dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        let Some(data) = self.session.dataset() else {
            self.dialog = None;
            return;
        };
        let mut action = None;
        let mut open = true;
        let syntax = dialog.syntax(data);
        egui::Window::new(dialog.title())
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(720.0)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(230.0);
                        variable_picker(ui, data, &mut self.filter, &mut self.picked);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        dialog_body(ui, dialog, &mut self.picked);
                    });
                });
                ui.separator();
                ui.weak("Syntax");
                match &syntax {
                    Ok(text) => {
                        ui.add(egui::Label::new(RichText::new(text).monospace()).wrap());
                    }
                    Err(message) => {
                        ui.colored_label(ui.visuals().warn_fg_color, message);
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ready = syntax.is_ok();
                    if ui
                        .add_enabled(ready, egui::Button::new("OK"))
                        .on_hover_text("Run the syntax")
                        .clicked()
                    {
                        action = Some(DialogAction::Ok);
                    }
                    if ui
                        .add_enabled(ready, egui::Button::new("Paste"))
                        .on_hover_text("Put the syntax in the syntax editor")
                        .clicked()
                    {
                        action = Some(DialogAction::Paste);
                    }
                    if ui.button("Reset").clicked() {
                        action = Some(DialogAction::Reset);
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(DialogAction::Cancel);
                    }
                });
            });
        if !open {
            action = Some(DialogAction::Cancel);
        }
        match (action, syntax) {
            (Some(DialogAction::Ok), Ok(text)) => {
                self.run(&text, None, After::CloseDialog);
            }
            (Some(DialogAction::Paste), Ok(text)) => {
                editor::append(&mut self.syntax, &text);
                self.dialog = None;
                self.tab = Tab::Syntax;
            }
            (Some(DialogAction::Reset), _) => {
                if let Some(d) = self.dialog.as_mut() {
                    d.reset();
                }
                self.picked.clear();
            }
            (Some(DialogAction::Cancel), _) => self.dialog = None,
            _ => {}
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // removes the endpoint file, so the command line says at once that no window runs
        self.server = None;
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let title = match self.data_file.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("{} - StatsCraft", name.to_string_lossy()),
            None => "StatsCraft".to_string(),
        };
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.grid_keys(ui);
        egui::Panel::top("menu").show(ui, |ui| {
            self.menu(ui);
            ui.add_space(2.0);
            self.tabs(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.show_log {
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(150.0)
                .show(ui, |ui| self.log(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Data => self.data_view(ui),
            Tab::Variables => self.variable_view(ui),
            Tab::Syntax => self.syntax_view(ui),
            Tab::Output => self.output_view(ui),
        });
        let ctx = ui.ctx().clone();
        self.dialog_window(&ctx);
    }
}

fn error_text(errors: &[Failure]) -> String {
    errors
        .iter()
        .map(Failure::text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_syntax_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sps"))
}

/// A text editor over one grid cell. Returns (commit, cancel): Enter or leaving the cell
/// commits, Escape cancels.
fn cell_editor(ui: &mut Ui, rect: Rect, edit: &mut CellEdit) -> (bool, bool) {
    let response = ui.put(
        rect,
        TextEdit::singleline(&mut edit.text)
            .id_salt("cell_editor")
            .margin(vec2(4.0, 2.0)),
    );
    if !edit.started {
        response.request_focus();
        // start with the whole value selected, so typing replaces it
        if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), response.id) {
            let all = egui::text::CCursorRange::select_all_str(&edit.text);
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), response.id);
        }
        edit.started = true;
        return (false, false);
    }
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        return (false, true);
    }
    (response.lost_focus(), false)
}

/// Draws only the visible cells of a large grid, with a header row and row numbers that stay
/// in view, and reports clicks by (row, column).
#[allow(clippy::too_many_arguments)]
fn paint_grid(
    ui: &mut Ui,
    id: &str,
    headers: &[String],
    widths: &[f32],
    rows: usize,
    selected: Option<(usize, usize)>,
    follow: bool,
    cell: &dyn Fn(usize, usize) -> (String, bool),
    editing: Option<(usize, usize)>,
    editor: &mut dyn FnMut(&mut Ui, Rect),
) -> GridEvents {
    let font = TextStyle::Body.resolve(ui.style());
    let row_height = font.size + 10.0;
    let header_height = row_height + 2.0;
    let number_width = (rows.max(1).to_string().len() as f32 * font.size * 0.6 + 20.0).max(48.0);
    let offsets = grid::offsets(widths);
    let total = *offsets.last().unwrap_or(&0.0);
    let size = vec2(
        number_width + total,
        header_height + rows as f32 * row_height,
    );
    let mut events = GridEvents::default();
    ScrollArea::both()
        .id_salt(id)
        .auto_shrink(false)
        .show_viewport(ui, |ui, viewport| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            let origin = rect.min;
            let clip = ui.clip_rect();
            let painter = ui.painter().with_clip_rect(clip);
            let visuals = ui.visuals().clone();
            let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
            let header_fill = visuals.widgets.noninteractive.weak_bg_fill;
            let visible_rows = grid::visible_rows(
                viewport.min.y,
                viewport.max.y - header_height,
                row_height,
                rows,
            );
            let visible_columns =
                grid::visible_columns(&offsets, viewport.min.x, viewport.max.x - number_width);
            let x = |c: usize| origin.x + number_width + offsets[c];
            let y = |r: usize| origin.y + header_height + r as f32 * row_height;
            let cell_rect = |r: usize, c: usize| {
                Rect::from_min_max(pos2(x(c), y(r)), pos2(x(c + 1), y(r) + row_height))
            };
            let text_in =
                |painter: &egui::Painter, rect: Rect, text: String, right: bool, color: Color32| {
                    let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(clip));
                    if right {
                        painter.text(
                            rect.right_center() - vec2(6.0, 0.0),
                            Align2::RIGHT_CENTER,
                            text,
                            font.clone(),
                            color,
                        );
                    } else {
                        painter.text(
                            rect.left_center() + vec2(6.0, 0.0),
                            Align2::LEFT_CENTER,
                            text,
                            font.clone(),
                            color,
                        );
                    }
                };

            let (left, right) = (x(visible_columns.start), x(visible_columns.end));
            for r in visible_rows.clone() {
                if r % 2 == 1 {
                    let band = Rect::from_min_max(pos2(left, y(r)), pos2(right, y(r) + row_height));
                    painter.rect_filled(band, 0.0, visuals.faint_bg_color);
                }
                for c in visible_columns.clone() {
                    let (text, right) = cell(r, c);
                    if !text.is_empty() {
                        text_in(&painter, cell_rect(r, c), text, right, visuals.text_color());
                    }
                }
                painter.hline(left..=right, y(r) + row_height, line);
            }
            let bottom = y(rows).min(clip.bottom());
            for c in visible_columns.clone() {
                painter.vline(x(c + 1), clip.top()..=bottom, line);
            }
            if let Some((r, c)) = selected
                && visible_rows.contains(&r)
                && visible_columns.contains(&c)
            {
                let stroke = Stroke::new(2.0, visuals.selection.stroke.color);
                painter.rect_stroke(cell_rect(r, c), 0.0, stroke, StrokeKind::Inside);
            }
            events.page = visible_rows.len().saturating_sub(1);
            if follow && let Some((r, c)) = selected {
                // room for the header row and the row numbers, which cover the grid's top left
                let rect = cell_rect(r, c);
                let room =
                    Rect::from_min_max(rect.min - vec2(number_width, header_height), rect.max);
                ui.scroll_to_rect(room, None);
            }
            if let Some((r, c)) = editing
                && visible_rows.contains(&r)
                && visible_columns.contains(&c)
            {
                editor(ui, cell_rect(r, c));
            }

            // the header row and the row numbers stay in view while scrolling
            let top = origin.y + viewport.min.y;
            let side = origin.x + viewport.min.x;
            let header = Rect::from_min_max(
                pos2(clip.left(), top),
                pos2(clip.right(), top + header_height),
            );
            painter.rect_filled(header, 0.0, header_fill);
            for c in visible_columns.clone() {
                let rect = Rect::from_min_max(pos2(x(c), top), pos2(x(c + 1), top + header_height));
                text_in(
                    &painter,
                    rect,
                    headers[c].clone(),
                    false,
                    visuals.strong_text_color(),
                );
                painter.vline(x(c + 1), rect.y_range(), line);
            }
            painter.hline(clip.x_range(), top + header_height, line);
            let numbers = Rect::from_min_max(
                pos2(side, top + header_height),
                pos2(side + number_width, bottom),
            );
            painter.rect_filled(numbers, 0.0, header_fill);
            for r in visible_rows.clone() {
                let rect = Rect::from_min_max(
                    pos2(side, y(r)),
                    pos2(side + number_width, y(r) + row_height),
                );
                if rect.bottom() > top + header_height {
                    text_in(
                        &painter,
                        rect,
                        (r + 1).to_string(),
                        true,
                        visuals.weak_text_color(),
                    );
                }
            }
            painter.vline(side + number_width, clip.top()..=bottom, line);
            painter.rect_filled(
                Rect::from_min_size(pos2(side, top), vec2(number_width, header_height)),
                0.0,
                header_fill,
            );

            if let Some(pos) = response.interact_pointer_pos() {
                let at = pos - origin;
                let inside =
                    at.x > viewport.min.x + number_width && at.y > viewport.min.y + header_height;
                let c = offsets
                    .partition_point(|&o| o <= at.x - number_width)
                    .saturating_sub(1);
                let r = ((at.y - header_height) / row_height) as usize;
                if inside && r < rows && c < widths.len() {
                    if response.double_clicked() {
                        events.double_clicked = Some((r, c));
                    } else if response.clicked() {
                        events.clicked = Some((r, c));
                    }
                }
            }
        });
    events
}

/// Column widths of a result table, wide enough for every cell and spanning header.
fn measure_table(ui: &Ui, table: &statscraft_engine::Table) -> Vec<f32> {
    let font = TextStyle::Body.resolve(ui.style());
    let width = |text: String| {
        ui.ctx().fonts_mut(|f| {
            f.layout_no_wrap(text, font.clone(), Color32::WHITE)
                .size()
                .x
        }) + 16.0
    };
    let mut widths = vec![24.0f32; table.columns.len()];
    for r in 0..table.rows.len() {
        for (c, w) in widths.iter_mut().enumerate().take(table.rows[r].len()) {
            *w = w.max(width(output::cell_text(table, r, c)));
        }
    }
    for level in output::header_rows(table) {
        for cell in level {
            let span = cell.first..cell.first + cell.span;
            let have: f32 = widths[span.clone()].iter().sum();
            let need = width(cell.label.clone());
            if need > have {
                let extra = (need - have) / cell.span as f32;
                widths[span].iter_mut().for_each(|w| *w += extra);
            }
        }
    }
    widths
}

fn paint_table(ui: &mut Ui, table: &statscraft_engine::Table, widths: &[f32]) {
    let font = TextStyle::Body.resolve(ui.style());
    let row_height = font.size + 10.0;
    let headers = output::header_rows(table);
    let height = (headers.len() + table.rows.len()) as f32 * row_height;
    let (rect, _) = ui.allocate_exact_size(vec2(widths.iter().sum(), height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect.expand(1.0));
    let visuals = ui.visuals();
    let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    let offsets = grid::offsets(widths);
    let x = |c: usize| rect.left() + offsets[c];
    let mut y = rect.top();
    let header_fill = visuals.widgets.noninteractive.weak_bg_fill;
    let header_left = x(table.row_headers);
    painter.rect_filled(
        Rect::from_min_max(
            pos2(header_left, y),
            pos2(rect.right(), y + headers.len() as f32 * row_height),
        ),
        0.0,
        header_fill,
    );
    for level in &headers {
        for cell in level {
            let span = Rect::from_min_max(
                pos2(x(cell.first), y),
                pos2(x(cell.first + cell.span), y + row_height),
            );
            painter.text(
                span.center(),
                Align2::CENTER_CENTER,
                &cell.label,
                font.clone(),
                visuals.strong_text_color(),
            );
            if cell.first >= table.row_headers && !cell.label.is_empty() {
                painter.hline(span.shrink2(vec2(4.0, 0.0)).x_range(), span.bottom(), line);
            }
        }
        y += row_height;
    }
    painter.hline(rect.x_range(), y, Stroke::new(1.5, visuals.text_color()));
    let body_top = y;
    let clip = ui.clip_rect();
    let rows = grid::visible_rows(
        clip.top() - body_top,
        clip.bottom() - body_top,
        row_height,
        table.rows.len(),
    );
    for r in rows {
        let y = body_top + r as f32 * row_height;
        if r % 2 == 1 {
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + row_height)),
                0.0,
                visuals.faint_bg_color,
            );
        }
        for c in 0..table.rows[r].len().min(widths.len()) {
            let text = output::cell_text(table, r, c);
            let cell = Rect::from_min_max(pos2(x(c), y), pos2(x(c + 1), y + row_height));
            if c < table.row_headers {
                painter.text(
                    cell.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    visuals.text_color(),
                );
            } else {
                painter.text(
                    cell.right_center() - vec2(8.0, 0.0),
                    Align2::RIGHT_CENTER,
                    text,
                    font.clone(),
                    visuals.text_color(),
                );
            }
        }
    }
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.5, visuals.text_color()),
    );
}

/// The dataset's variables, filtered by name or label; a click picks or unpicks one.
fn variable_picker(ui: &mut Ui, data: &Dataset, filter: &mut String, picked: &mut Vec<String>) {
    ui.strong("Variables");
    ui.add(TextEdit::singleline(filter).hint_text("Filter"));
    let needle = filter.to_lowercase();
    let matches: Vec<usize> = (0..data.variable_count())
        .filter(|&i| {
            let v = data.variable_at(i);
            needle.is_empty()
                || v.name().to_lowercase().contains(&needle)
                || v.label()
                    .is_some_and(|l| l.to_lowercase().contains(&needle))
        })
        .collect();
    let row_height = ui.text_style_height(&TextStyle::Body) + 4.0;
    ScrollArea::vertical()
        .id_salt("picker")
        .max_height(320.0)
        .auto_shrink([false, true])
        .show_rows(ui, row_height, matches.len(), |ui, range| {
            for &i in &matches[range] {
                let v = data.variable_at(i);
                let name = v.name().to_string();
                let on = picked.contains(&name);
                let mut text = RichText::new(&name);
                if v.is_string() {
                    text = text.italics();
                }
                let response = ui.selectable_label(on, text);
                let response = match v.label() {
                    Some(label) => response.on_hover_text(label),
                    None => response,
                };
                if response.clicked() {
                    if on {
                        picked.retain(|p| p != &name);
                    } else {
                        picked.push(name);
                    }
                }
            }
        });
    ui.weak("Click to pick, then Add to a list.");
}

/// A list of variables in a dialog, with Add (the picked variables) and a remove button each.
fn target_list(ui: &mut Ui, title: &str, list: &mut Vec<String>, picked: &mut Vec<String>) {
    ui.push_id(title, |ui| {
        ui.horizontal(|ui| {
            ui.strong(title);
            if ui
                .add_enabled(!picked.is_empty(), egui::Button::new("Add").small())
                .clicked()
            {
                for name in picked.drain(..) {
                    if !list.contains(&name) {
                        list.push(name);
                    }
                }
            }
        });
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(260.0);
            if list.is_empty() {
                ui.weak("empty");
            }
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for (i, name) in list.iter().enumerate() {
                    if ui
                        .button(format!("{name}  \u{d7}"))
                        .on_hover_text("Remove")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                list.remove(i);
            }
        });
    });
}

fn checkboxes(ui: &mut Ui, options: &[(&str, &str)], flags: &mut [bool]) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .show(ui, |ui| {
            for (i, ((_, label), flag)) in options.iter().zip(flags.iter_mut()).enumerate() {
                ui.checkbox(flag, *label);
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

fn dialog_body(ui: &mut Ui, dialog: &mut Dialog, picked: &mut Vec<String>) {
    match dialog {
        Dialog::Frequencies(d) => target_list(ui, "Variables", &mut d.variables, picked),
        Dialog::Descriptives(d) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.add_space(6.0);
            ui.strong("Statistics");
            checkboxes(ui, &DESCRIPTIVES_STATISTICS, &mut d.statistics);
            ui.checkbox(
                &mut d.listwise,
                "Leave out a case missing on any listed variable",
            );
            ui.checkbox(&mut d.save_z_scores, "Save z-scores as new variables");
        }
        Dialog::Crosstabs(d) => {
            target_list(ui, "Rows", &mut d.rows, picked);
            target_list(ui, "Columns", &mut d.columns, picked);
            target_list(ui, "Layers", &mut d.layers, picked);
            ui.add_space(6.0);
            ui.strong("Cells");
            checkboxes(ui, &CROSSTABS_CELLS, &mut d.cells);
            ui.checkbox(&mut d.chi_square, "Chi-square tests");
        }
        Dialog::CustomTables(d) => {
            target_list(ui, "Rows (categories)", &mut d.rows, picked);
            target_list(ui, "Columns (categories)", &mut d.columns, picked);
            target_list(
                ui,
                "Means of (scale variables, in the rows)",
                &mut d.scale_rows,
                picked,
            );
            ui.add_space(6.0);
            ui.strong("Summaries");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut d.count, "Count");
                ui.checkbox(&mut d.column_percent, "Column %");
                ui.checkbox(&mut d.row_percent, "Row %");
                ui.checkbox(&mut d.mean, "Mean");
                ui.checkbox(&mut d.totals, "Totals");
            });
            ui.strong("Significance tests (need column variables)");
            ui.checkbox(&mut d.chi_square, "Chi-square test of independence");
            ui.checkbox(&mut d.compare_proportions, "Compare column proportions");
            ui.checkbox(&mut d.compare_means, "Compare column means");
            ui.horizontal(|ui| {
                ui.label("Significance level");
                ui.add(TextEdit::singleline(&mut d.alpha).desired_width(60.0));
                ComboBox::new("adjust", "")
                    .selected_text(d.adjust.label())
                    .show_ui(ui, |ui| {
                        for adjust in Adjust::ALL {
                            ui.selectable_value(&mut d.adjust, adjust, adjust.label());
                        }
                    });
            });
        }
        Dialog::Compute(d, label) => {
            egui::Grid::new("compute").num_columns(2).show(ui, |ui| {
                ui.label("Target variable");
                ui.text_edit_singleline(&mut d.target);
                ui.end_row();
                ui.label("Label (optional)");
                ui.text_edit_singleline(label);
                ui.end_row();
                ui.label("Expression");
                ui.add(
                    TextEdit::singleline(&mut d.expression)
                        .desired_width(320.0)
                        .hint_text("age / 10"),
                );
                ui.end_row();
                ui.label("Only where (optional)");
                ui.add(
                    TextEdit::singleline(&mut d.condition)
                        .desired_width(320.0)
                        .hint_text("gender = 1"),
                );
                ui.end_row();
            });
            if ui
                .button("Add picked variables to the expression")
                .clicked()
            {
                for name in picked.drain(..) {
                    if !d.expression.is_empty() && !d.expression.ends_with(' ') {
                        d.expression.push(' ');
                    }
                    d.expression.push_str(&name);
                }
            }
        }
        Dialog::Recode(d, draft) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.horizontal(|ui| {
                ui.label("Into new variables (optional, one name each)");
                ui.text_edit_singleline(&mut d.into);
            });
            ui.add_space(6.0);
            ui.strong("Rules, the first match wins");
            let mut remove = None;
            for (i, rule) in d.rules.iter().enumerate() {
                ui.horizontal(|ui| {
                    let text = rule
                        .syntax(false)
                        .or_else(|_| rule.syntax(true))
                        .unwrap_or_else(|e| e);
                    ui.monospace(text);
                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                d.rules.remove(i);
            }
            ui.horizontal_wrapped(|ui| {
                ComboBox::new("old", "")
                    .selected_text(OldValue::CHOICES[draft.old])
                    .show_index(ui, &mut draft.old, OldValue::CHOICES.len(), |i| {
                        OldValue::CHOICES[i]
                    });
                if draft.old <= 3 {
                    ui.add(TextEdit::singleline(&mut draft.low).desired_width(70.0));
                }
                if draft.old == 1 {
                    ui.label("through");
                    ui.add(TextEdit::singleline(&mut draft.high).desired_width(70.0));
                }
                ui.label("becomes");
                ComboBox::new("new", "")
                    .selected_text(NewValue::CHOICES[draft.new])
                    .show_index(ui, &mut draft.new, NewValue::CHOICES.len(), |i| {
                        NewValue::CHOICES[i]
                    });
                if draft.new == 0 {
                    ui.add(TextEdit::singleline(&mut draft.value).desired_width(70.0));
                }
                if ui.button("Add rule").clicked() {
                    d.rules.push(RecodeRule {
                        old: OldValue::from_choice(draft.old, &draft.low, &draft.high),
                        new: NewValue::from_choice(draft.new, &draft.value),
                    });
                    *draft = RuleDraft::default();
                }
            });
        }
    }
}
