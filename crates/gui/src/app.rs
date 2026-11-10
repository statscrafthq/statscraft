//! The window. Drawing only: every change to data and every result goes through `Session::run`
//! with syntax built by the plain modules.

use crate::analyze_dialogs::{
    Correlations, DUNNETT_SIDES, EQUAL_VARIANCE_TESTS, Explore, ExploreMissing, MEANS_CELLS, Means,
    ONEWAY_STATISTICS, Oneway, POST_HOC_TESTS, STANDARDIZERS, TTest, TTestKind,
};
use crate::data_dialogs::{
    AGGREGATE_FUNCTIONS, Aggregate, AggregateTo, Aggregated, MergeFiles, Mrsets, SelectCases,
    SortCases, WeightCases,
};
use crate::dialogs::{
    Adjust, CROSSTABS_CELLS, Categories, Compute, Count, Crosstabs, CustomTables,
    DESCRIPTIVES_STATISTICS, Descriptives, Frequencies, Letters, NewValue, OldValue, Recode,
    RecodeRule, Subtotal, is_set, label_command,
};
use crate::edits::{self, Field};
use crate::output;
use crate::output_view::{self, OutputView};
use crate::settings::Settings;
use crate::syntax_text::{number, quote};
use crate::updates::Updates;
use crate::{complete, editor, grid, highlight};
use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, FocusDirection, FontId, Key, Modifiers, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextStyle, Ui, UiBuilder, pos2,
    text::LayoutJob, vec2,
};
use statscraft_data::{Dataset, Value, Values};
use statscraft_engine::live::{self, Action, Request, Response};
use statscraft_engine::session::{Done, Failure, RunResult, Session};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The documentation online, in the repository.
const DOCS: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/tree/main/docs");

/// A request from `statscraft --session gui` and where its reply goes.
type Remote = (Request, mpsc::Sender<Response>);

/// What to do when a run finishes.
enum After {
    Show,
    Editor,
    OpenData(PathBuf),
    SaveData(PathBuf),
    /// With the dialog's title, for the output.
    CloseDialog(&'static str),
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

/// The categories panel of Custom Tables: the variable shown, its categories as (syntax
/// literal, shown text), and the subtotal being put together.
#[derive(Default)]
struct CategoryDraft {
    variable: String,
    categories: Vec<(String, String)>,
    label: String,
    chosen: Vec<String>,
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
    Explore(Explore),
    Crosstabs(Crosstabs),
    CustomTables(CustomTables, CategoryDraft),
    Compute(Compute, String),
    Recode(Recode, RuleDraft),
    /// With the label of the new variable and the value being put together.
    Count(Count, String, RuleDraft),
    Means(Means),
    TTest(TTest),
    Oneway(Oneway),
    Correlations(Correlations),
    Mrsets(Mrsets),
    SortCases(SortCases),
    SelectCases(SelectCases),
    WeightCases(WeightCases),
    /// With the function chosen for the next aggregated variables.
    Aggregate(Aggregate, usize),
    MergeFiles(MergeFiles),
}

impl Dialog {
    fn title(&self) -> &'static str {
        match self {
            Dialog::Frequencies(_) => "Frequencies",
            Dialog::Descriptives(_) => "Descriptives",
            Dialog::Explore(_) => "Explore",
            Dialog::Crosstabs(_) => "Crosstabs",
            Dialog::CustomTables(..) => "Custom Tables",
            Dialog::Compute(..) => "Compute Variable",
            Dialog::Recode(..) => "Recode",
            Dialog::Count(..) => "Count Values within Cases",
            Dialog::Means(_) => "Means",
            Dialog::TTest(d) => d.title(),
            Dialog::Oneway(_) => "One-Way ANOVA",
            Dialog::Correlations(_) => "Bivariate Correlations",
            Dialog::Mrsets(_) => "Define Multiple Response Sets",
            Dialog::SortCases(_) => "Sort Cases",
            Dialog::SelectCases(_) => "Select Cases",
            Dialog::WeightCases(_) => "Weight Cases",
            Dialog::Aggregate(..) => "Aggregate",
            Dialog::MergeFiles(d) => d.title(),
        }
    }

    fn reset(&mut self) {
        *self = match self {
            Dialog::Frequencies(_) => Dialog::Frequencies(Default::default()),
            Dialog::Descriptives(_) => Dialog::Descriptives(Default::default()),
            Dialog::Explore(_) => Dialog::Explore(Default::default()),
            Dialog::Crosstabs(_) => Dialog::Crosstabs(Default::default()),
            Dialog::CustomTables(..) => {
                Dialog::CustomTables(Default::default(), Default::default())
            }
            Dialog::Compute(..) => Dialog::Compute(Default::default(), String::new()),
            Dialog::Recode(..) => Dialog::Recode(Default::default(), Default::default()),
            Dialog::Count(..) => {
                Dialog::Count(Default::default(), String::new(), Default::default())
            }
            Dialog::Means(_) => Dialog::Means(Default::default()),
            Dialog::TTest(d) => Dialog::TTest(TTest::new(d.kind)),
            Dialog::Oneway(_) => Dialog::Oneway(Default::default()),
            Dialog::Correlations(_) => Dialog::Correlations(Default::default()),
            Dialog::Mrsets(_) => Dialog::Mrsets(Default::default()),
            Dialog::SortCases(_) => Dialog::SortCases(Default::default()),
            Dialog::SelectCases(_) => Dialog::SelectCases(Default::default()),
            Dialog::WeightCases(_) => Dialog::WeightCases(Default::default()),
            Dialog::Aggregate(..) => Dialog::Aggregate(Default::default(), 0),
            Dialog::MergeFiles(d) => Dialog::MergeFiles(MergeFiles {
                add_variables: d.add_variables,
                ..Default::default()
            }),
        }
    }

    fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            Dialog::Frequencies(d) => d.syntax(),
            Dialog::Descriptives(d) => d.syntax(),
            Dialog::Explore(d) => d.syntax(),
            Dialog::Crosstabs(d) => d.syntax(),
            Dialog::CustomTables(d, _) => d.syntax(),
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
            Dialog::Count(d, label, _) => {
                let strings = d.variables.iter().filter(|n| is_string(data, n)).count();
                if strings > 0 && strings < d.variables.len() {
                    return Err("count in string and numeric variables separately".into());
                }
                let mut text = d.syntax(strings > 0)?;
                if let Some(label) = label_command(&d.target, label) {
                    text.push('\n');
                    text.push_str(&label);
                }
                Ok(text)
            }
            Dialog::Means(d) => d.syntax(),
            Dialog::TTest(d) => d.syntax(is_string(data, &d.grouping)),
            Dialog::Oneway(d) => d.syntax(),
            Dialog::Correlations(d) => d.syntax(),
            Dialog::Mrsets(d) => {
                let strings = d.variables.iter().filter(|n| is_string(data, n)).count();
                if strings > 0 && strings < d.variables.len() {
                    return Err("the variables of a set must be all numeric or all strings".into());
                }
                d.syntax(strings > 0)
            }
            Dialog::SortCases(d) => d.syntax(),
            Dialog::SelectCases(d) => d.syntax(),
            Dialog::WeightCases(d) => d.syntax(),
            Dialog::Aggregate(d, _) => d.syntax(),
            Dialog::MergeFiles(d) => d.syntax(),
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

/// The completion list of the syntax editor.
struct Completion {
    list: Vec<complete::Candidate>,
    /// The bytes of the text a candidate replaces.
    range: std::ops::Range<usize>,
    selected: usize,
}

#[derive(Default)]
struct GridEvents {
    clicked: Option<(usize, usize)>,
    double_clicked: Option<(usize, usize)>,
    /// A column border dragged: the column, its new width, and whether the drag ended.
    resized: Option<(usize, f32, bool)>,
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
    /// A Data View column being dragged to a new width, until its VARIABLE WIDTH has run.
    data_resize: Option<(usize, f32)>,
    variable_selected: Option<(usize, usize)>,
    field_edit: Option<CellEdit>,
    syntax: String,
    syntax_file: Option<PathBuf>,
    syntax_errors: Vec<Failure>,
    selection: (usize, usize),
    wrap: bool,
    /// The data file before the last change, which undo puts back too.
    file_before_change: Option<PathBuf>,
    /// The syntax of the run going on or done last, for the syntax of its output.
    source: String,
    /// The full help of a command, from F1.
    help_window: Option<String>,
    completion: Option<Completion>,
    /// The About window is open.
    about: bool,
    editor_id: Option<egui::Id>,
    output: OutputView,
    /// Window size and position, the last folder, the recent files and the update check, kept
    /// between starts.
    settings: Settings,
    updates: Updates,
    dialog: Option<Dialog>,
    picked: Vec<String>,
    filter: String,
    status: Result<String, String>,
    /// The number of warnings of the last run, noted next to the status.
    warnings: usize,
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
    pub fn new(args: &[String], ctx: &egui::Context, mut settings: Settings) -> Self {
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
        let updates = Updates::new(&app_ctx, &mut settings);
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
            data_resize: None,
            variable_selected: None,
            field_edit: None,
            syntax: String::new(),
            syntax_file: None,
            syntax_errors: Vec::new(),
            selection: (0, 0),
            wrap: true,
            file_before_change: None,
            source: String::new(),
            help_window: None,
            completion: None,
            about: false,
            editor_id: None,
            output: OutputView::default(),
            settings,
            updates,
            dialog: None,
            picked: Vec::new(),
            filter: String::new(),
            status,
            warnings: 0,
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
        self.warnings = 0;
        let check = matches!(&after, After::Remote((r, _)) if r.action == Action::Check);
        let job = self.session.job(source, lines, check);
        self.source = source.to_string();
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
        self.data_resize = None;
        let changed = matches!(&result, Ok(r) if r.saved);
        let file = self.data_file.clone();
        let loaded = matches!(&result, Ok(r) if r.commands.iter().any(|c| c.name == "GET"));
        if changed && loaded && !matches!(after, After::OpenData(_)) {
            // a GET typed or sent from the command line: Save must not write this data over
            // the file that was open before
            self.data_file = None;
        }
        if changed {
            self.file_before_change = file;
        }
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
                self.show(result, "command line", false);
            }
            After::Show => {
                self.show(result, "window", false);
            }
            After::Editor => {
                self.show(result, "syntax editor", true);
            }
            After::OpenData(path) => {
                if self.show(result, "open file", false) {
                    self.add_recent(&path);
                    self.data_file = Some(path);
                    self.selected = None;
                    self.cell_edit = None;
                    self.field_edit = None;
                    self.tab = Tab::Data;
                }
            }
            After::SaveData(path) => {
                if self.show(result, "save file", false) {
                    self.add_recent(&path);
                    self.data_file = Some(path);
                }
            }
            After::CloseDialog(title) => {
                if self.show(result, title, false) {
                    self.dialog = None;
                }
            }
        }
    }

    /// Puts back the dataset and the data file from before the last change.
    fn undo(&mut self) {
        if self.busy.is_some() {
            return;
        }
        let syntax = self.session.undoable().unwrap_or_default().to_string();
        if self.session.undo() {
            self.data_file = self.file_before_change.take();
            self.cell_edit = None;
            self.field_edit = None;
            let first = syntax.lines().next().unwrap_or_default();
            self.status = Ok(format!("undone: {first}"));
        }
    }

    /// Puts the tables of a run in the output, or its errors in the status bar and, for the
    /// syntax editor, in the editor's error list.
    fn show(
        &mut self,
        result: Result<RunResult, Vec<Failure>>,
        origin: &str,
        from_editor: bool,
    ) -> bool {
        match result.map(|r| r.commands) {
            Ok(outputs) => {
                if from_editor {
                    self.syntax_errors.clear();
                }
                let mut commands = Vec::new();
                for out in outputs {
                    let heading = match (from_editor, out.first_line == out.last_line) {
                        (false, _) => out.name,
                        (true, true) => format!("{}, line {}", out.name, out.first_line),
                        (true, false) => {
                            format!("{}, lines {}-{}", out.name, out.first_line, out.last_line)
                        }
                    };
                    let syntax = output::source_lines(&self.source, out.first_line, out.last_line);
                    commands.push((heading, syntax, out.tables));
                }
                if self.output.add_run(origin, commands) {
                    self.tab = Tab::Output;
                }
                let last = self.session.log.last();
                let summary = last.and_then(|e| e.result.as_ref().ok());
                self.status = Ok(summary.cloned().unwrap_or_default());
                self.warnings = last.map_or(0, |e| e.warnings.len());
                if self.warnings > 0 {
                    self.show_log = true;
                }
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

    /// Files dropped on the window from the file manager.
    fn open_dropped(&mut self, paths: Vec<PathBuf>) {
        let drop = dropped(paths);
        if let Some(path) = drop.syntax {
            self.load_syntax(path);
        }
        if let Some(path) = drop.data {
            self.open_data(path);
        }
        if let Some(note) = drop.note {
            self.status = Err(note);
        }
    }

    fn save_data(&mut self, path: PathBuf) {
        let syntax = format!("SAVE OUTFILE={}.", quote(&path.to_string_lossy()));
        self.run(&syntax, None, After::SaveData(path));
    }

    fn load_syntax(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.add_recent(&path);
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
                self.add_recent(&path);
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
                    && let Some(path) = self
                        .file_dialog()
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
                    && let Some(path) = self
                        .file_dialog()
                        .add_filter("Syntax", &["sps"])
                        .pick_file()
                {
                    self.load_syntax(path);
                }
                let recent = self.settings.recent.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button("Recent files", |ui| {
                        for path in recent {
                            let name = path.file_name().map_or_else(
                                || path.display().to_string(),
                                |n| n.to_string_lossy().into_owned(),
                            );
                            if ui
                                .button(name)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                if is_syntax_file(&path) {
                                    self.load_syntax(path);
                                } else {
                                    self.open_data(path);
                                }
                            }
                        }
                    });
                });
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
                    self.export_html(false);
                }
                ui.add_enabled_ui(!self.output.is_empty(), |ui| {
                    ui.menu_button("Export output as Excel", |ui| {
                        if ui.button("One sheet per table...").clicked() {
                            self.export_xlsx(false, false);
                        }
                        if ui.button("All tables on one sheet...").clicked() {
                            self.export_xlsx(true, false);
                        }
                    });
                });
            });
            ui.menu_button("Edit", |ui| {
                let undo = self.busy.is_none() && self.session.undoable().is_some();
                let button = egui::Button::new("Undo last change").shortcut_text("Ctrl+Z");
                let response = ui.add_enabled(undo, button);
                let response = match self.session.undoable() {
                    Some(syntax) => response.on_hover_text(syntax),
                    None => response,
                };
                if response.clicked() {
                    self.undo();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.value_labels, "Value labels in Data View");
                ui.checkbox(&mut self.show_log, "Log");
                ui.checkbox(&mut self.wrap, "Wrap long lines in the syntax editor");
                ui.separator();
                ui.weak("Paper of the grids, the syntax and the output");
                let follow = &mut self.settings.paper_follows_system;
                let changed = ui.radio_value(follow, false, "Always light").changed()
                    | ui.radio_value(follow, true, "Follow the system theme")
                        .changed();
                if changed {
                    let _ = self.settings.save();
                }
            });
            ui.menu_button("Data", |ui| {
                let weight = self
                    .session
                    .dataset()
                    .and_then(|d| d.weight_variable())
                    .map(str::to_string);
                for (label, dialog) in [
                    (
                        "Define Multiple Response Sets...",
                        Dialog::Mrsets(Default::default()),
                    ),
                    ("Sort Cases...", Dialog::SortCases(Default::default())),
                    ("Select Cases...", Dialog::SelectCases(Default::default())),
                    (
                        "Weight Cases...",
                        Dialog::WeightCases(WeightCases::current(weight.as_deref())),
                    ),
                    ("Aggregate...", Dialog::Aggregate(Default::default(), 0)),
                ] {
                    if ui.add_enabled(has_data, egui::Button::new(label)).clicked() {
                        self.open_dialog(dialog);
                    }
                }
                ui.add_enabled_ui(has_data, |ui| {
                    ui.menu_button("Merge Files", |ui| {
                        for (label, add_variables) in
                            [("Add Cases...", false), ("Add Variables...", true)]
                        {
                            if ui.button(label).clicked() {
                                self.open_dialog(Dialog::MergeFiles(MergeFiles {
                                    add_variables,
                                    ..Default::default()
                                }));
                            }
                        }
                    });
                });
            });
            ui.menu_button("Analyze", |ui| {
                ui.add_enabled_ui(has_data, |ui| {
                    ui.menu_button("Descriptive Statistics", |ui| {
                        for (label, dialog) in [
                            ("Frequencies...", Dialog::Frequencies(Default::default())),
                            ("Descriptives...", Dialog::Descriptives(Default::default())),
                            ("Explore...", Dialog::Explore(Default::default())),
                            ("Crosstabs...", Dialog::Crosstabs(Default::default())),
                        ] {
                            if ui.button(label).clicked() {
                                self.open_dialog(dialog);
                            }
                        }
                    });
                });
                if ui
                    .add_enabled(has_data, egui::Button::new("Custom Tables..."))
                    .clicked()
                {
                    self.open_dialog(Dialog::CustomTables(Default::default(), Default::default()));
                }
                ui.add_enabled_ui(has_data, |ui| {
                    ui.menu_button("Compare Means", |ui| {
                        for (label, dialog) in [
                            ("Means...", Dialog::Means(Default::default())),
                            (
                                "One-Sample T Test...",
                                Dialog::TTest(TTest::new(TTestKind::OneSample)),
                            ),
                            (
                                "Independent-Samples T Test...",
                                Dialog::TTest(TTest::new(TTestKind::Independent)),
                            ),
                            (
                                "Paired-Samples T Test...",
                                Dialog::TTest(TTest::new(TTestKind::Paired)),
                            ),
                            ("One-Way ANOVA...", Dialog::Oneway(Default::default())),
                        ] {
                            if ui.button(label).clicked() {
                                self.open_dialog(dialog);
                            }
                        }
                    });
                    ui.menu_button("Correlate", |ui| {
                        if ui.button("Bivariate...").clicked() {
                            self.open_dialog(Dialog::Correlations(Default::default()));
                        }
                    });
                });
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
                if ui
                    .add_enabled(has_data, egui::Button::new("Count Values within Cases..."))
                    .clicked()
                {
                    self.open_dialog(Dialog::Count(
                        Default::default(),
                        String::new(),
                        Default::default(),
                    ));
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Documentation").on_hover_text(DOCS).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DOCS));
                }
                if ui.button("About StatsCraft").clicked() {
                    self.about = true;
                }
                ui.separator();
                self.updates.menu_items(ui, &mut self.settings);
            });
        });
    }

    /// A file dialog that opens in the folder of the last one.
    fn file_dialog(&self) -> rfd::FileDialog {
        file_dialog(&self.settings.folder)
    }

    /// Puts a file opened or saved first in the recent files, and keeps its folder for the
    /// next file dialog.
    fn add_recent(&mut self, path: &Path) {
        self.settings.remember_folder(path);
        self.settings.add_recent(path);
        // the window works on without them
        let _ = self.settings.save();
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.picked.clear();
        self.filter.clear();
    }

    fn save_data_as(&mut self) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Data file", &["sav"])
            .add_filter("Compressed data file", &["zsav"])
            .save_file()
        {
            self.save_data(path);
        }
    }

    fn save_syntax_as(&mut self) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Syntax", &["sps"])
            .save_file()
        {
            self.save_syntax(path);
        }
    }

    /// Exports the whole output, or with `selected` only the selected tables.
    fn export_html(&mut self, selected: bool) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Web page", &["html"])
            .save_file()
        {
            self.settings.remember_folder(&path);
            let chosen = &self.output.selected;
            let page = output::html(&self.output.runs, |id| !selected || chosen.contains(&id));
            self.status = match std::fs::write(&path, page) {
                Ok(()) => Ok(format!("output exported to {}", path.display())),
                Err(e) => Err(format!("{}: {e}", path.display())),
            };
        }
    }

    fn export_xlsx(&mut self, one_sheet: bool, selected: bool) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Excel workbook", &["xlsx"])
            .save_file()
        {
            self.settings.remember_folder(&path);
            let chosen = &self.output.selected;
            let tables = output::tables(&self.output.runs)
                .filter(|t| !selected || chosen.contains(&t.id))
                .map(|t| &t.table);
            self.status = match statscraft_export::write_xlsx(&path, tables, one_sheet) {
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
        ui.horizontal(|ui| {
            match &self.status {
                Ok(text) => ui.weak(text),
                Err(text) => ui.colored_label(ui.visuals().error_fg_color, text),
            };
            if let Some(note) = warning_note(self.warnings) {
                ui.colored_label(ui.visuals().warn_fg_color, note);
            }
        });
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
                    .filter(|e| e.result.is_ok() && !e.undone)
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
                        Ok(summary) if entry.undone => ui.weak(format!("{summary}; undone")),
                        Ok(summary) => ui.weak(summary),
                        Err(errors) => {
                            ui.colored_label(ui.visuals().error_fg_color, error_text(errors))
                        }
                    };
                    for warning in &entry.warnings {
                        ui.colored_label(ui.visuals().warn_fg_color, format!("warning: {warning}"));
                    }
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
        paper(ui, !self.settings.paper_follows_system);
        let data = self.session.dataset().expect("checked above");
        let char_width = TextStyle::Body.resolve(ui.style()).size * 0.6;
        let labels = self.value_labels;
        let mut widths: Vec<f32> = data
            .variables()
            .map(|v| {
                if let Some(chars) = v.display_width() {
                    return grid::width_of(chars, char_width);
                }
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
        if let Some((c, width)) = self.data_resize
            && c < widths.len()
        {
            widths[c] = width;
        }
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
        if let Some((c, width, done)) = events.resized {
            self.data_resize = Some((c, width));
            if done {
                let data = self.session.dataset().expect("checked above");
                let name = data.variable_at(c).name().to_string();
                self.run(
                    &grid::width_command(&name, width, char_width),
                    None,
                    After::Show,
                );
            }
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
        paper(ui, !self.settings.paper_follows_system);
        let data = self.session.dataset().expect("checked above");
        let headers: Vec<String> = Field::ALL.iter().map(|f| f.title().to_string()).collect();
        let mut widths = vec![150.0, 80.0, 90.0, 60.0, 72.0, 260.0, 300.0, 170.0, 100.0];
        if self.settings.variable_widths.len() == widths.len() {
            widths.clone_from(&self.settings.variable_widths);
        }
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
        if let Some((c, width, done)) = events.resized {
            widths[c] = width;
            self.settings.variable_widths = widths;
            if done {
                let _ = self.settings.save();
            }
        }
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
            let line = editor::line_of(&self.syntax, self.selection.0.max(self.selection.1));
            let command = editor::command_at(&self.syntax, line);
            self.help_window = statscraft_syntax::help(command.as_deref());
        }
        let mut run_all = false;
        let mut run_selection = run_selection_key;
        ui.horizontal(|ui| {
            run_all = ui.button("Run all").clicked();
            run_selection |= ui
                .button("Run selection")
                .on_hover_text("Runs the commands on the selected lines, or the command at the cursor (Ctrl+Enter). Ctrl+Space completes, F1 shows the help of the command at the cursor.")
                .clicked();
            let name = self
                .syntax_file
                .as_ref()
                .map_or("unsaved syntax".into(), |p| p.display().to_string());
            ui.weak(name);
        });
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

        // keys of an open completion list, before the editor sees them
        let explicit = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Space));
        let mut accept = None;
        if let Some(c) = self.completion.as_mut() {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    c.selected = (c.selected + 1).min(c.list.len() - 1);
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    c.selected = c.selected.saturating_sub(1);
                }
                if i.consume_key(Modifiers::NONE, Key::Enter)
                    || i.consume_key(Modifiers::NONE, Key::Tab)
                {
                    accept = Some(c.selected);
                }
            });
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.completion = None;
                // Escape also takes the focus from a text field: give it back
                if let Some(id) = self.editor_id {
                    ui.memory_mut(|m| m.request_focus(id));
                }
            }
        }
        if let Some(index) = accept {
            self.accept_completion(ui.ctx(), index);
        }

        let font = FontId::monospace(14.0);
        let error_lines: Vec<u32> = self.syntax_errors.iter().filter_map(|e| e.line).collect();
        let wrap = self.wrap;
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let visuals = ui.visuals();
            let mut job = highlight::cached_job(
                ui.ctx(),
                text.as_str(),
                visuals.dark_mode,
                visuals.text_color(),
                14.0,
            );
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let gutter = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0')) * 5.0 + 4.0;
        let area = if wrap {
            ScrollArea::vertical()
        } else {
            ScrollArea::both()
        };
        let light = !self.settings.paper_follows_system;
        let mut popup_at = None;
        let mut hovered_command = None;
        egui::CentralPanel::default().show(ui, |ui| {
            paper(ui, light);
            area.auto_shrink(false).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let left = ui.cursor().left();
                    ui.add_space(gutter);
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
                    self.editor_id = Some(output.response.id);
                    // a line number beside the first row of every line, which may wrap
                    let mut line = 1;
                    let mut starts_line = true;
                    for row in &output.galley.rows {
                        if starts_line {
                            let error = error_lines.contains(&line);
                            let color = if error {
                                ui.visuals().error_fg_color
                            } else {
                                ui.visuals().weak_text_color()
                            };
                            ui.painter().text(
                                pos2(left + gutter, output.galley_pos.y + row.pos.y),
                                Align2::RIGHT_TOP,
                                format!("{}{line}", if error { "!" } else { "" }),
                                font.clone(),
                                color,
                            );
                            line += 1;
                        }
                        starts_line = row.ends_with_newline;
                    }
                    let mut moved = false;
                    if let Some(range) = output.cursor_range {
                        let selection = (range.secondary.index.0, range.primary.index.0);
                        moved = selection != self.selection;
                        self.selection = selection;
                        let cursor = output.galley.pos_from_cursor(range.primary);
                        popup_at = Some(output.galley_pos + cursor.left_bottom().to_vec2());
                    }
                    let changed = output.response.changed();
                    if changed || explicit {
                        self.complete(explicit);
                    } else if moved {
                        self.completion = None;
                    }
                    if output.response.hovered()
                        && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
                    {
                        let at = output.galley.cursor_from_pos(pointer - output.galley_pos);
                        let byte = editor::byte_of(&self.syntax, at.index.0);
                        hovered_command = complete::command_at_byte(&self.syntax, byte)
                            .map(|spec| (spec, egui::Response::clone(&output.response)));
                    }
                });
            });
        });
        if let Some((spec, response)) = hovered_command
            && self.completion.is_none()
        {
            response.on_hover_ui_at_pointer(|ui| command_card(ui, spec));
        }
        if let Some(at) = popup_at {
            self.completion_popup(ui.ctx(), at);
        }
        if let Some(text) = &self.help_window {
            let mut open = true;
            egui::Window::new("Command help")
                .open(&mut open)
                .default_size([560.0, 420.0])
                .show(ui.ctx(), |ui| {
                    // the help text is laid out in lines and indents, so it scrolls instead of wrapping
                    ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(text).monospace()).extend());
                    });
                });
            if !open || ui.input(|i| i.key_pressed(Key::Escape)) {
                self.help_window = None;
            }
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

    /// Opens, updates or closes the completion list for the text at the cursor.
    fn complete(&mut self, explicit: bool) {
        let cursor = editor::byte_of(&self.syntax, self.selection.1);
        if self.selection.0 != self.selection.1 {
            self.completion = None;
            return;
        }
        let variables: Vec<(String, String)> = self
            .session
            .dataset()
            .map(|data| {
                data.variables()
                    .map(|v| {
                        (
                            v.name().to_string(),
                            v.label().unwrap_or_default().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (list, range) = complete::candidates(&self.syntax, cursor, &variables, explicit);
        self.completion = (!list.is_empty()).then_some(Completion {
            list,
            range,
            selected: 0,
        });
    }

    /// Puts a candidate of the completion list in the text and the cursor after it.
    fn accept_completion(&mut self, ctx: &egui::Context, index: usize) {
        let Some(c) = self.completion.take() else {
            return;
        };
        let Some(candidate) = c.list.get(index) else {
            return;
        };
        let (text, cursor) = complete::accept(&self.syntax, c.range, candidate);
        self.syntax = text;
        let at = self.syntax[..cursor].chars().count();
        self.selection = (at, at);
        if let Some(id) = self.editor_id {
            let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
            let cursor = egui::text::CCursor::new(at);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
            state.store(ctx, id);
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }

    /// The completion list under the cursor, with the summary and syntax of a command in it.
    fn completion_popup(&mut self, ctx: &egui::Context, at: egui::Pos2) {
        let Some(c) = self.completion.as_ref() else {
            return;
        };
        let mut clicked = None;
        egui::Area::new(egui::Id::unique("completion"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + vec2(0.0, 2.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(520.0);
                    ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (i, candidate) in c.list.iter().enumerate() {
                            let mut text = LayoutJob::default();
                            let mono = egui::TextFormat {
                                font_id: FontId::monospace(13.0),
                                color: ui.visuals().strong_text_color(),
                                ..Default::default()
                            };
                            text.append(&candidate.text, 0.0, mono);
                            if !candidate.detail.is_empty() {
                                let weak = egui::TextFormat {
                                    font_id: FontId::proportional(12.0),
                                    color: ui.visuals().weak_text_color(),
                                    ..Default::default()
                                };
                                text.append(&candidate.detail, 12.0, weak);
                            }
                            let row = ui.selectable_label(i == c.selected, text);
                            if i == c.selected {
                                row.scroll_to_me(None);
                            }
                            if row.clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
                    let selected = &c.list[c.selected];
                    if selected.what == complete::What::Command
                        && let Some(spec) = complete::spec(&selected.text)
                    {
                        ui.separator();
                        command_card(ui, spec);
                    }
                });
            });
        if let Some(i) = clicked {
            self.accept_completion(ctx, i);
        }
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
                        let sets = matches!(dialog, Dialog::CustomTables(..));
                        variable_picker(ui, data, sets, &mut self.filter, &mut self.picked);
                    });
                    ui.add_space(12.0);
                    // a long dialog scrolls, so OK stays in reach on a small screen
                    let height = (ctx.content_rect().height() - 260.0).max(240.0);
                    ScrollArea::vertical()
                        .id_salt("dialog_body")
                        .max_height(height)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                let folder = &mut self.settings.folder;
                                dialog_body(ui, data, dialog, &mut self.picked, folder);
                            });
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
                let title = self.dialog.as_ref().map_or("dialog", Dialog::title);
                self.run(&text, None, After::CloseDialog(title));
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
        self.updates.poll();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // removes the endpoint file, so the command line says at once that no window runs
        self.server = None;
        let _ = self.settings.save();
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
        // the size and position to open with next time (no position under Wayland)
        let (outer, inner) = ui.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
        if let Some(inner) = inner {
            self.settings.size = Some([inner.width(), inner.height()]);
        }
        if let Some(outer) = outer {
            self.settings.position = Some([outer.min.x, outer.min.y]);
        }
        let (files, hovering) = ui.input(|i| {
            let files: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect();
            (files, !i.raw.hovered_files.is_empty())
        });
        if !files.is_empty() {
            self.open_dropped(files);
        }
        if hovering {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("drop files"),
            ));
            let screen = ui.ctx().content_rect();
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(160));
            painter.text(
                screen.center(),
                Align2::CENTER_CENTER,
                "Drop .sav, .zsav or .sps files to open them",
                FontId::proportional(20.0),
                Color32::WHITE,
            );
        }
        self.grid_keys(ui);
        // Ctrl+Z inside a text field is that field's own undo
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z))
        {
            self.undo();
        }
        egui::Panel::top("menu").show(ui, |ui| {
            self.menu(ui);
            ui.add_space(2.0);
            self.tabs(ui);
        });
        if self.updates.visible() {
            egui::Panel::top("update").show(ui, |ui| self.updates.bar(ui));
        }
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
            Tab::Output => {
                self.output.light_paper = !self.settings.paper_follows_system;
                let request = self.output.ui(ui);
                match request {
                    Some(output_view::Request::ExportHtml) => self.export_html(true),
                    Some(output_view::Request::ExportXlsx { one_sheet }) => {
                        self.export_xlsx(one_sheet, true)
                    }
                    Some(output_view::Request::Status(status)) => self.status = status,
                    Some(output_view::Request::Paste(syntax)) => {
                        editor::append(&mut self.syntax, &syntax);
                        self.tab = Tab::Syntax;
                    }
                    Some(output_view::Request::Run(syntax)) => {
                        self.run(&syntax, None, After::Show);
                    }
                    None => {}
                }
            }
        });
        let ctx = ui.ctx().clone();
        self.dialog_window(&ctx);
        egui::Window::new("About StatsCraft")
            .open(&mut self.about)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.heading("StatsCraft");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Statistics with a command language, on .sav data files.");
                ui.label("License: MIT");
                ui.hyperlink_to("Source code", REPOSITORY);
                ui.hyperlink_to("Documentation", DOCS);
            });
    }
}

/// The rest of `ui` as a sheet of paper: light with dark text when `light`, whatever the system
/// theme, as a spreadsheet or a document page stays light in dark mode.
pub(crate) fn paper(ui: &mut Ui, light: bool) {
    if light {
        ui.style_mut().visuals = egui::Visuals::light();
    }
    let sheet = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(sheet, 0.0, ui.visuals().extreme_bg_color);
}

/// The summary and syntax of a command, in the completion list and over a command name.
fn command_card(ui: &mut Ui, spec: &statscraft_syntax::CommandSpec) {
    ui.set_max_width(560.0);
    ui.strong(spec.name);
    ui.label(spec.summary);
    ui.add(egui::Label::new(RichText::new(spec.syntax).monospace().size(12.0)).wrap());
}

/// What the status bar says about the warnings of the last run.
fn warning_note(warnings: usize) -> Option<String> {
    match warnings {
        0 => None,
        1 => Some("1 warning, see the log".into()),
        n => Some(format!("{n} warnings, see the log")),
    }
}

/// Whether the dataset has a string variable of that name.
fn is_string(data: &Dataset, name: &str) -> bool {
    data.variable(name.trim()).is_ok_and(|v| v.is_string())
}

fn error_text(errors: &[Failure]) -> String {
    errors
        .iter()
        .map(Failure::text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A file dialog that opens in `folder` when there is one.
fn file_dialog(folder: &Option<PathBuf>) -> rfd::FileDialog {
    match folder {
        Some(folder) => rfd::FileDialog::new().set_directory(folder),
        None => rfd::FileDialog::new(),
    }
}

fn is_syntax_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sps"))
}

fn is_data_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sav") || e.eq_ignore_ascii_case("zsav"))
}

/// What a drop of files from the file manager opens: one data file, one syntax file, and a
/// note naming the files left alone.
#[derive(Debug, Default, PartialEq)]
struct Dropped {
    data: Option<PathBuf>,
    syntax: Option<PathBuf>,
    note: Option<String>,
}

fn dropped(paths: Vec<PathBuf>) -> Dropped {
    let mut drop = Dropped::default();
    let (mut extra, mut other) = (Vec::new(), Vec::new());
    for path in paths {
        let slot = if is_data_file(&path) {
            &mut drop.data
        } else if is_syntax_file(&path) {
            &mut drop.syntax
        } else {
            other.push(path);
            continue;
        };
        if slot.is_none() {
            *slot = Some(path);
        } else {
            extra.push(path);
        }
    }
    let name = |p: &PathBuf| {
        p.file_name().map_or_else(
            || p.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let mut reasons = Vec::new();
    if !extra.is_empty() {
        reasons.push("one data file and one syntax file open at a time");
    }
    if !other.is_empty() {
        reasons.push("StatsCraft opens .sav, .zsav and .sps files");
    }
    if !reasons.is_empty() {
        let names: Vec<String> = extra.iter().chain(&other).map(name).collect();
        drop.note = Some(format!(
            "not opened: {} ({})",
            names.join(", "),
            reasons.join("; ")
        ));
    }
    drop
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

/// The narrowest a dragged column gets.
const MIN_COLUMN: f32 = 20.0;

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
            // a right-aligned text too wide for its cell shows its start, so a number cut
            // short never reads as a different number
            let text_in =
                |painter: &egui::Painter, rect: Rect, text: String, right: bool, color: Color32| {
                    let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(clip));
                    let galley = painter.layout_no_wrap(text, font.clone(), color);
                    let width = galley.size().x;
                    let x = if right && width <= rect.width() - 12.0 {
                        rect.right() - 6.0 - width
                    } else {
                        rect.left() + 6.0
                    };
                    let y = rect.center().y - galley.size().y / 2.0;
                    painter.galley(pos2(x, y), galley, color);
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
                // drag a column's right border in the header to change its width
                let border = Rect::from_center_size(
                    pos2(x(c + 1), rect.center().y),
                    vec2(8.0, header_height),
                );
                let handle = ui.interact(
                    border,
                    ui.make_persistent_id((id, "resize", c)),
                    Sense::drag(),
                );
                if handle.hovered() || handle.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                }
                if let Some(pointer) = handle.interact_pointer_pos()
                    && (handle.dragged() || handle.drag_stopped())
                {
                    let width = (pointer.x - x(c)).max(MIN_COLUMN);
                    events.resized = Some((c, width, handle.drag_stopped()));
                }
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

/// The dataset's variables, filtered by name or label, and with `sets` its multiple response
/// sets; a click picks or unpicks one.
fn variable_picker(
    ui: &mut Ui,
    data: &Dataset,
    sets: bool,
    filter: &mut String,
    picked: &mut Vec<String>,
) {
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
    let sets: Vec<_> = data
        .multiple_response_sets()
        .iter()
        .filter(|_| sets)
        .filter(|set| {
            needle.is_empty()
                || set.name.to_lowercase().contains(&needle)
                || set.label.to_lowercase().contains(&needle)
        })
        .collect();
    if !sets.is_empty() {
        ui.add_space(4.0);
        ui.strong("Multiple response sets");
        for set in sets {
            let on = picked.contains(&set.name);
            let response = ui
                .selectable_label(on, format!("{} (set)", set.name))
                .on_hover_text(format!("{} set: {}", set.kind, set.label));
            if response.clicked() {
                if on {
                    picked.retain(|p| p != &set.name);
                } else {
                    picked.push(set.name.clone());
                }
            }
        }
    }
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

/// A slot for one variable, filled with the first picked variable.
fn target_one(ui: &mut Ui, title: &str, slot: &mut String, picked: &mut Vec<String>) {
    ui.push_id(title, |ui| {
        ui.horizontal(|ui| {
            ui.strong(title);
            if ui
                .add_enabled(!picked.is_empty(), egui::Button::new("Set").small())
                .clicked()
            {
                *slot = picked.remove(0);
            }
            if slot.is_empty() {
                ui.weak("empty");
            } else if ui
                .button(format!("{slot}  \u{d7}"))
                .on_hover_text("Remove")
                .clicked()
            {
                slot.clear();
            }
        });
    });
}

/// A text field with a label in front, on one line.
fn field(ui: &mut Ui, label: &str, text: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(TextEdit::singleline(text).desired_width(width));
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

fn dialog_body(
    ui: &mut Ui,
    data: &Dataset,
    dialog: &mut Dialog,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
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
        Dialog::Explore(d) => {
            target_list(ui, "Dependent variables", &mut d.dependents, picked);
            target_list(ui, "Factors (each on its own)", &mut d.factors, picked);
            target_one(ui, "Label cases by", &mut d.id, picked);
            ui.add_space(6.0);
            ui.strong("Statistics");
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.descriptives, "Descriptives, mean interval at");
                ui.add_enabled(
                    d.descriptives,
                    TextEdit::singleline(&mut d.confidence).desired_width(50.0),
                );
                ui.label("%");
            });
            ui.checkbox(
                &mut d.mestimators,
                "M-estimators (Huber, Tukey, Hampel, Andrews)",
            );
            ui.checkbox(&mut d.outliers, "Five highest and lowest values");
            ui.checkbox(
                &mut d.percentiles,
                "Percentiles 5 to 95, with Tukey's hinges",
            );
            ui.checkbox(
                &mut d.normality,
                "Tests of normality (Kolmogorov-Smirnov, Shapiro-Wilk)",
            );
            ui.add_space(6.0);
            ui.strong("Missing values");
            for missing in ExploreMissing::ALL {
                ui.radio_value(&mut d.missing, missing, missing.label());
            }
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
        Dialog::CustomTables(d, draft) => {
            target_list(ui, "Rows (categories or sets)", &mut d.rows, picked);
            target_list(ui, "Columns (categories or sets)", &mut d.columns, picked);
            target_list(
                ui,
                "Layers (a table for each category)",
                &mut d.layers,
                picked,
            );
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
                ui.checkbox(&mut d.effective_count, "Effective count")
                    .on_hover_text("The effective base of the weighted count");
                ui.checkbox(&mut d.column_percent, "Column %");
                ui.checkbox(&mut d.row_percent, "Row %");
                ui.checkbox(&mut d.mean, "Mean");
                ui.checkbox(&mut d.totals, "Totals");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.hide_small_counts, "Hide counts below");
                ui.add_enabled(
                    d.hide_small_counts,
                    TextEdit::singleline(&mut d.small_count).desired_width(40.0),
                );
            });
            target_one(ui, "Effective base weight", &mut d.effective_weight, picked);
            egui::CollapsingHeader::new("Categories, totals and subtotals")
                .show(ui, |ui| categories_panel(ui, data, d, draft));
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
            ui.horizontal(|ui| {
                ui.label("Letters of the column comparisons");
                ComboBox::new("letters", "")
                    .selected_text(d.letters.label())
                    .show_ui(ui, |ui| {
                        for letters in Letters::ALL {
                            ui.selectable_value(&mut d.letters, letters, letters.label());
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
        Dialog::Count(d, label, draft) => {
            egui::Grid::new("count").num_columns(2).show(ui, |ui| {
                ui.label("Target variable");
                ui.text_edit_singleline(&mut d.target);
                ui.end_row();
                ui.label("Label (optional)");
                ui.text_edit_singleline(label);
                ui.end_row();
            });
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.add_space(6.0);
            ui.strong("Values to count");
            let mut remove = None;
            for (i, value) in d.values.iter().enumerate() {
                ui.horizontal(|ui| {
                    let text = value
                        .syntax(false)
                        .or_else(|_| value.syntax(true))
                        .unwrap_or_else(|e| e);
                    ui.monospace(text);
                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                d.values.remove(i);
            }
            // every old value of Recode but the last, "all other values"
            let choices = OldValue::CHOICES.len() - 1;
            ui.horizontal_wrapped(|ui| {
                ComboBox::new("count value", "")
                    .selected_text(OldValue::CHOICES[draft.old])
                    .show_index(ui, &mut draft.old, choices, |i| OldValue::CHOICES[i]);
                if draft.old <= 3 {
                    ui.add(TextEdit::singleline(&mut draft.low).desired_width(70.0));
                }
                if draft.old == 1 {
                    ui.label("through");
                    ui.add(TextEdit::singleline(&mut draft.high).desired_width(70.0));
                }
                if ui.button("Add value").clicked() {
                    d.values
                        .push(OldValue::from_choice(draft.old, &draft.low, &draft.high));
                    *draft = RuleDraft::default();
                }
            });
            ui.weak("A value or range also counts user-missing values.");
        }
        Dialog::Means(d) => {
            target_list(ui, "Dependent variables", &mut d.dependents, picked);
            ui.horizontal(|ui| {
                let count = d.layers.len();
                ui.label(format!("Layer {} of {count}", d.layer + 1));
                if ui
                    .add_enabled(d.layer > 0, egui::Button::new("Previous").small())
                    .clicked()
                {
                    d.layer -= 1;
                }
                let can_add = !d.layers[d.layer].is_empty();
                if ui
                    .add_enabled(
                        d.layer + 1 < count || can_add,
                        egui::Button::new("Next").small(),
                    )
                    .on_hover_text("The next layer is nested in this one")
                    .clicked()
                {
                    d.layer += 1;
                    if d.layer == count {
                        d.layers.push(Vec::new());
                    }
                }
            });
            target_list(
                ui,
                "Group by (control variables)",
                &mut d.layers[d.layer],
                picked,
            );
            ui.add_space(6.0);
            ui.strong("Statistics");
            checkboxes(ui, &MEANS_CELLS, &mut d.cells);
            let controls: Vec<String> = d.layers.iter().flatten().cloned().collect();
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.npct_within, "Percent of cases");
                ui.checkbox(&mut d.spct_within, "Percent of sum, within");
                ComboBox::new("within", "")
                    .selected_text(if d.within.is_empty() {
                        "a control variable"
                    } else {
                        &d.within
                    })
                    .show_ui(ui, |ui| {
                        for name in &controls {
                            ui.selectable_value(&mut d.within, name.clone(), name);
                        }
                    });
            });
            ui.checkbox(&mut d.anova, "Analysis of variance and eta (Layer 1)");
            ui.checkbox(&mut d.linearity, "Test for linearity");
        }
        Dialog::TTest(d) => match d.kind {
            TTestKind::OneSample => {
                target_list(ui, "Test variables", &mut d.variables, picked);
                field(ui, "Test value", &mut d.test_value, 70.0);
                t_test_options(ui, d);
            }
            TTestKind::Independent => {
                target_list(ui, "Test variables", &mut d.variables, picked);
                target_one(ui, "Grouping variable", &mut d.grouping, picked);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.cut_point, false, "Values");
                    ui.add(
                        TextEdit::singleline(&mut d.group1)
                            .desired_width(60.0)
                            .hint_text("1"),
                    );
                    if !d.cut_point {
                        ui.label("and");
                        ui.add(
                            TextEdit::singleline(&mut d.group2)
                                .desired_width(60.0)
                                .hint_text("2"),
                        );
                    }
                    ui.radio_value(&mut d.cut_point, true, "Cut point");
                });
                if d.cut_point {
                    ui.weak("Cases at or above the cut point form one group.");
                }
                t_test_options(ui, d);
            }
            TTestKind::Paired => {
                ui.strong("Pairs");
                let mut remove = None;
                for (i, (a, b)) in d.pairs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{a}  with  {b}"));
                        if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    d.pairs.remove(i);
                }
                if ui
                    .add_enabled(picked.len() >= 2, egui::Button::new("Add pair"))
                    .on_hover_text("Pairs the first two picked variables")
                    .clicked()
                {
                    let first = picked.remove(0);
                    d.pairs.push((first, picked.remove(0)));
                }
                t_test_options(ui, d);
            }
        },
        Dialog::Oneway(d) => {
            target_list(ui, "Dependent variables", &mut d.dependents, picked);
            target_one(ui, "Factor", &mut d.factor, picked);
            ui.add_space(6.0);
            egui::CollapsingHeader::new("Post hoc comparisons").show(ui, |ui| {
                let (equal, unequal) = POST_HOC_TESTS.split_at(EQUAL_VARIANCE_TESTS);
                let (equal_flags, unequal_flags) = d.post_hoc.split_at_mut(EQUAL_VARIANCE_TESTS);
                ui.weak("Equal variances assumed");
                checkboxes(ui, equal, equal_flags);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut d.dunnett, "Dunnett, control group");
                    ui.add_enabled_ui(d.dunnett, |ui| {
                        ui.radio_value(&mut d.dunnett_first, true, "first");
                        ui.radio_value(&mut d.dunnett_first, false, "last");
                        ComboBox::new("dunnett_side", "")
                            .selected_text(DUNNETT_SIDES[d.dunnett_side].1)
                            .show_index(ui, &mut d.dunnett_side, DUNNETT_SIDES.len(), |i| {
                                DUNNETT_SIDES[i].1
                            });
                    });
                });
                ui.weak("Equal variances not assumed");
                checkboxes(ui, unequal, unequal_flags);
                field(ui, "Significance level", &mut d.alpha, 60.0);
            });
            egui::CollapsingHeader::new("Options").show(ui, |ui| {
                checkboxes(ui, &ONEWAY_STATISTICS, &mut d.statistics);
                field(ui, "Confidence level %", &mut d.confidence, 60.0);
                ui.checkbox(
                    &mut d.listwise,
                    "Leave out a case missing on any listed variable",
                );
            });
        }
        Dialog::Correlations(d) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.add_space(6.0);
            ui.strong("Coefficients");
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.pearson, "Pearson");
                ui.checkbox(&mut d.kendall, "Kendall's tau-b");
                ui.checkbox(&mut d.spearman, "Spearman");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.one_tailed, false, "Two-tailed test");
                ui.radio_value(&mut d.one_tailed, true, "One-tailed test");
            });
            ui.checkbox(&mut d.flag, "Mark significant correlations with stars");
            ui.add_enabled_ui(d.pearson, |ui| {
                ui.checkbox(
                    &mut d.descriptives,
                    "Means and standard deviations (Pearson)",
                );
                ui.checkbox(
                    &mut d.cross_products,
                    "Cross-products and covariances (Pearson)",
                );
            });
            ui.checkbox(
                &mut d.listwise,
                "Leave out a case missing on any listed variable",
            );
        }
        Dialog::Mrsets(d) => {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.delete, false, "Define a set");
                ui.radio_value(&mut d.delete, true, "Delete sets");
            });
            let sets = data.multiple_response_sets();
            if d.delete {
                ui.strong("Sets to delete");
                if sets.is_empty() {
                    ui.weak("The dataset has no sets.");
                }
                for set in sets {
                    let mut on = d.sets.contains(&set.name);
                    if ui
                        .checkbox(&mut on, format!("{}  {}", set.name, set.label))
                        .changed()
                    {
                        if on {
                            d.sets.push(set.name.clone());
                        } else {
                            d.sets.retain(|s| s != &set.name);
                        }
                    }
                }
                return;
            }
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.dichotomy, true, "Dichotomies")
                    .on_hover_text("Each variable is a category, counted when it holds one value");
                ui.radio_value(&mut d.dichotomy, false, "Categories")
                    .on_hover_text("The values of the variables are the categories");
            });
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(
                    TextEdit::singleline(&mut d.name)
                        .desired_width(160.0)
                        .hint_text("$channels"),
                );
            });
            target_list(ui, "Variables", &mut d.variables, picked);
            if d.dichotomy {
                field(ui, "Counted value", &mut d.value, 70.0);
                ui.checkbox(
                    &mut d.counted_value_labels,
                    "Label categories with the value label of the counted value",
                );
            }
            let from_variables = d.dichotomy && d.counted_value_labels;
            if from_variables {
                ui.checkbox(
                    &mut d.label_from_variables,
                    "Take the set label from the first variable label",
                );
            }
            if !(from_variables && d.label_from_variables) {
                field(ui, "Label", &mut d.label, 260.0);
            }
            if !sets.is_empty() {
                let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
                ui.weak(format!(
                    "Sets now: {}. A set with the same name is replaced.",
                    names.join(", ")
                ));
            }
        }
        Dialog::SortCases(d) => {
            ui.horizontal(|ui| {
                ui.strong("Sort by");
                if ui
                    .add_enabled(!picked.is_empty(), egui::Button::new("Add").small())
                    .clicked()
                {
                    for name in picked.drain(..) {
                        if !d.keys.iter().any(|(k, _)| k == &name) {
                            d.keys.push((name, false));
                        }
                    }
                }
            });
            if d.keys.is_empty() {
                ui.weak("empty");
            }
            let mut remove = None;
            for (i, (name, descending)) in d.keys.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    let order = if *descending {
                        "descending"
                    } else {
                        "ascending"
                    };
                    if ui
                        .small_button(order)
                        .on_hover_text("Switch the order")
                        .clicked()
                    {
                        *descending = !*descending;
                    }
                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                d.keys.remove(i);
            }
            ui.weak("Cases with equal keys keep their order.");
        }
        Dialog::SelectCases(d) => {
            ui.strong("Keep the cases where");
            ui.add(
                TextEdit::multiline(&mut d.condition)
                    .desired_width(360.0)
                    .desired_rows(3)
                    .hint_text("age >= 18 AND gender = 2"),
            );
            if ui.button("Add picked variables to the condition").clicked() {
                for name in picked.drain(..) {
                    if !d.condition.is_empty() && !d.condition.ends_with(' ') {
                        d.condition.push(' ');
                    }
                    d.condition.push_str(&name);
                }
            }
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The other cases are deleted from the dataset. Edit, Undo last change brings them back.",
            );
        }
        Dialog::WeightCases(d) => {
            ui.radio_value(&mut d.on, false, "Do not weight cases");
            ui.radio_value(&mut d.on, true, "Weight cases by a variable");
            if d.on {
                target_one(ui, "Weight variable", &mut d.variable, picked);
            }
            ui.weak(match data.weight_variable() {
                Some(name) => format!("Now weighted by {name}."),
                None => "Now not weighted.".to_string(),
            });
        }
        Dialog::Aggregate(d, function) => {
            target_list(ui, "Group by (break variables)", &mut d.breaks, picked);
            ui.add_space(6.0);
            ui.strong("Aggregated variables");
            let mut remove = None;
            for (i, v) in d.variables.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut v.target).desired_width(120.0));
                    ui.monospace(format!(
                        "= {}({})",
                        AGGREGATE_FUNCTIONS[v.function].0, v.source
                    ));
                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                d.variables.remove(i);
            }
            ui.horizontal(|ui| {
                ComboBox::new("function", "")
                    .selected_text(AGGREGATE_FUNCTIONS[*function].1)
                    .show_index(ui, function, AGGREGATE_FUNCTIONS.len(), |i| {
                        AGGREGATE_FUNCTIONS[i].1
                    });
                if ui
                    .add_enabled(
                        !picked.is_empty(),
                        egui::Button::new("Add picked variables"),
                    )
                    .clicked()
                {
                    for name in picked.drain(..) {
                        d.variables.push(Aggregated::new(*function, &name));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.count, "Number of cases in the group as");
                ui.add_enabled(
                    d.count,
                    TextEdit::singleline(&mut d.count_name).desired_width(100.0),
                );
            });
            ui.add_space(6.0);
            ui.strong("Result");
            ui.radio_value(
                &mut d.to,
                AggregateTo::AddVariables,
                "Add the variables to every case",
            );
            ui.radio_value(
                &mut d.to,
                AggregateTo::Replace,
                "Replace the dataset with one case per group",
            );
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.to, AggregateTo::File, "Write to a data file");
                if d.to == AggregateTo::File {
                    ui.add(TextEdit::singleline(&mut d.path).desired_width(220.0));
                    if ui.button("Browse...").clicked()
                        && let Some(path) = file_dialog(folder)
                            .add_filter("Data file", &["sav"])
                            .save_file()
                    {
                        *folder = path.parent().map(Path::to_path_buf);
                        d.path = path.to_string_lossy().into_owned();
                    }
                }
            });
        }
        Dialog::MergeFiles(d) => {
            ui.horizontal(|ui| {
                ui.strong(if d.add_variables {
                    "File with the variables to add"
                } else {
                    "File with the cases to add"
                });
                if ui.button("Browse...").clicked()
                    && let Some(path) = file_dialog(folder)
                        .add_filter("Data file", &["sav", "zsav"])
                        .pick_file()
                {
                    *folder = path.parent().map(Path::to_path_buf);
                    d.path = path.to_string_lossy().into_owned();
                }
            });
            ui.add(TextEdit::singleline(&mut d.path).desired_width(360.0));
            if d.add_variables {
                ui.checkbox(
                    &mut d.table,
                    "The file is a lookup table: one case per key, adds no cases",
                );
                target_list(ui, "Key variables", &mut d.by, picked);
                ui.weak(if d.by.is_empty() {
                    "Without keys the cases are joined in file order."
                } else {
                    "Both files must be sorted by the keys (Data, Sort Cases)."
                });
            } else {
                ui.weak("Variables are matched by name; a variable missing from a file is blank.");
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

/// The categories of one variable of Custom Tables: order, totals, subtotals, missing values.
fn categories_panel(ui: &mut Ui, data: &Dataset, d: &mut CustomTables, draft: &mut CategoryDraft) {
    let mut names: Vec<String> = Vec::new();
    for name in d.rows.iter().chain(&d.columns).chain(&d.layers) {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    if names.is_empty() {
        ui.weak("Add variables to Rows, Columns or Layers first.");
        return;
    }
    if !names.contains(&draft.variable) {
        *draft = CategoryDraft::default();
        draft.variable = names[0].clone();
        draft.categories = categories(data, &draft.variable);
    }
    ComboBox::new("category_variable", "")
        .selected_text(&draft.variable)
        .show_ui(ui, |ui| {
            for name in &names {
                if ui.selectable_label(&draft.variable == name, name).clicked() {
                    *draft = CategoryDraft::default();
                    draft.variable = name.clone();
                    draft.categories = categories(data, name);
                }
            }
        });
    let Some(i) = d
        .categories
        .iter()
        .position(|c| c.variable == draft.variable)
    else {
        if ui.button("Set the categories of this variable").clicked() {
            d.categories
                .push(Categories::new(&draft.variable, d.totals));
        }
        ui.weak("Until then it follows Totals under Summaries.");
        return;
    };
    let scale = d.scale_rows.clone();
    let c = &mut d.categories[i];
    let listed = !c.subtotals.is_empty();
    ui.add_enabled_ui(!listed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Sort by");
            let key_label = |key: &str| match key {
                "" => "Value".to_string(),
                "LABEL" => "Label".to_string(),
                "COUNT" => "Count".to_string(),
                other => format!("Mean of {}", &other[5..other.len() - 1]),
            };
            ComboBox::new("category_key", "")
                .selected_text(key_label(&c.key))
                .show_ui(ui, |ui| {
                    let keys = ["", "LABEL", "COUNT"].map(String::from).into_iter();
                    for key in keys.chain(scale.iter().map(|s| format!("MEAN({s})"))) {
                        let text = key_label(&key);
                        ui.selectable_value(&mut c.key, key, text);
                    }
                });
            ui.radio_value(&mut c.descending, false, "ascending");
            ui.radio_value(&mut c.descending, true, "descending");
        });
    });
    if listed {
        ui.weak("With subtotals the categories keep their value order.");
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut c.total, "Total, labelled");
        ui.add_enabled(
            c.total,
            TextEdit::singleline(&mut c.total_label)
                .desired_width(120.0)
                .hint_text("Total"),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Totals and subtotals");
        ui.radio_value(&mut c.before, false, "after");
        ui.radio_value(&mut c.before, true, "before");
        ui.label("their categories");
    });
    ui.checkbox(&mut c.missing, "Show user-missing values as categories");
    if is_set(&c.variable) {
        ui.weak("Subtotals are for variables, not sets.");
    } else {
        ui.strong("Subtotals");
        let mut remove = None;
        for (s, subtotal) in c.subtotals.iter().enumerate() {
            ui.horizontal(|ui| {
                let label = match subtotal.label.trim() {
                    "" => "Subtotal",
                    label => label,
                };
                ui.label(format!("{label}: {}", subtotal.values.join(", ")));
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(s);
                }
            });
        }
        if let Some(s) = remove {
            c.subtotals.remove(s);
        }
        let used: Vec<&String> = c.subtotals.iter().flat_map(|s| &s.values).collect();
        let free: Vec<&(String, String)> = draft
            .categories
            .iter()
            .filter(|(literal, _)| !used.contains(&literal))
            .collect();
        // shortcut: the first 200 categories only; a variable with more is not one to subtotal
        ui.horizontal_wrapped(|ui| {
            for (literal, shown) in free.into_iter().take(200) {
                let mut on = draft.chosen.contains(literal);
                if ui.checkbox(&mut on, shown).changed() {
                    if on {
                        draft.chosen.push(literal.clone());
                    } else {
                        draft.chosen.retain(|l| l != literal);
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut draft.label)
                    .desired_width(140.0)
                    .hint_text("Subtotal label"),
            );
            if ui
                .add_enabled(!draft.chosen.is_empty(), egui::Button::new("Add subtotal"))
                .clicked()
            {
                // the categories in their value order
                let values = draft
                    .categories
                    .iter()
                    .map(|(literal, _)| literal)
                    .filter(|l| draft.chosen.contains(l))
                    .cloned()
                    .collect();
                c.subtotals.push(Subtotal {
                    label: std::mem::take(&mut draft.label),
                    values,
                });
                draft.chosen.clear();
            }
        });
        ui.add_enabled_ui(listed, |ui| {
            ui.checkbox(&mut c.hide_sources, "Hide the categories a subtotal sums");
            ui.checkbox(&mut c.other, "Show the categories outside the subtotals");
        });
    }
    if ui.button("Back to the defaults").clicked() {
        d.categories.remove(i);
    }
}

/// The valid categories of a variable, labelled or in the data, in ascending order, as
/// (syntax literal, shown text).
fn categories(data: &Dataset, name: &str) -> Vec<(String, String)> {
    let Ok(v) = data.variable(name) else {
        return Vec::new();
    };
    let shown =
        |literal: String, value: Value| match v.value_labels().iter().find(|(k, _)| *k == value) {
            Some((_, label)) => (literal.clone(), format!("{literal}  {label}")),
            None => (literal.clone(), literal),
        };
    let labelled = v.value_labels().iter().map(|(k, _)| k.clone());
    match v.values() {
        Values::Numeric(values) => {
            let mut numbers: Vec<f64> = labelled
                .filter_map(|k| match k {
                    Value::Number(n) => Some(n),
                    Value::String(_) => None,
                })
                .chain(values.iter().flatten())
                .filter(|n| !v.is_user_missing(*n))
                .collect();
            numbers.sort_by(f64::total_cmp);
            numbers.dedup();
            numbers
                .into_iter()
                .map(|n| shown(number(n), Value::Number(n)))
                .collect()
        }
        Values::String(values) => {
            let texts: BTreeSet<String> = labelled
                .filter_map(|k| match k {
                    Value::String(s) => Some(s),
                    Value::Number(_) => None,
                })
                .chain(values.iter().map(str::to_string))
                .filter(|s| !s.is_empty() && !v.is_user_missing_str(s))
                .collect();
            texts
                .into_iter()
                .map(|s| shown(quote(&s), Value::String(s)))
                .collect()
        }
    }
}

/// The options the three t tests share.
fn t_test_options(ui: &mut Ui, d: &mut TTest) {
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Confidence interval %", &mut d.confidence, 60.0);
        ui.checkbox(
            &mut d.listwise,
            "Leave out a case missing on any variable of the test",
        );
        ui.checkbox(&mut d.effect_sizes, "Effect sizes");
        if d.kind == TTestKind::Paired && d.effect_sizes {
            ComboBox::new("standardizer", "Scale of Cohen's d")
                .selected_text(STANDARDIZERS[d.standardizer].1)
                .show_index(ui, &mut d.standardizer, STANDARDIZERS.len(), |i| {
                    STANDARDIZERS[i].1
                });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    #[test]
    fn dropped_files_open_one_data_file_and_one_syntax_file() {
        let paths = |names: &[&str]| names.iter().map(PathBuf::from).collect::<Vec<_>>();
        let drop = dropped(paths(&["/d/a.SAV", "/d/job.sps"]));
        assert_eq!(drop.data, Some(PathBuf::from("/d/a.SAV")));
        assert_eq!(drop.syntax, Some(PathBuf::from("/d/job.sps")));
        assert_eq!(drop.note, None);
        let drop = dropped(paths(&["/d/b.zsav", "/d/c.sav", "/d/notes.txt"]));
        assert_eq!(drop.data, Some(PathBuf::from("/d/b.zsav")));
        assert_eq!(drop.syntax, None);
        assert_eq!(
            drop.note.unwrap(),
            "not opened: c.sav, notes.txt (one data file and one syntax file open at a time; \
             StatsCraft opens .sav, .zsav and .sps files)"
        );
        let drop = dropped(paths(&["/d/table.csv"]));
        assert_eq!(drop.data, None);
        assert_eq!(
            drop.note.unwrap(),
            "not opened: table.csv (StatsCraft opens .sav, .zsav and .sps files)"
        );
    }

    #[test]
    fn subtotal_categories_are_the_valid_values_in_order() {
        let session = open("survey_basic.sav");
        let data = session.dataset().unwrap();
        let q1 = categories(data, "q1_satisfaction");
        let literals: Vec<&str> = q1.iter().map(|(l, _)| l.as_str()).collect();
        // 98 and 99 are user-missing
        assert_eq!(literals, ["1", "2", "3", "4", "5"]);
        assert_eq!(q1[0].1, "1  Very dissatisfied");
        let plan = categories(data, "plan");
        let literals: Vec<&str> = plan.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(literals, ["'BASIC'", "'ENTERPRISE'", "'PLUS'", "'PREMIUM'"]);
        assert!(categories(data, "nonesuch").is_empty());
    }

    #[test]
    fn warnings_of_a_run_reach_the_log_and_the_status_note() {
        assert_eq!(warning_note(0), None);
        assert_eq!(warning_note(1).unwrap(), "1 warning, see the log");
        assert_eq!(warning_note(3).unwrap(), "3 warnings, see the log");
        // a string variable gets no statistics in DESCRIPTIVES, and a warning says so
        let syntax = Descriptives {
            variables: v(&["age", "plan"]),
            ..Default::default()
        }
        .syntax()
        .unwrap();
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        let warnings = &session.log.last().unwrap().warnings;
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("plan"));
    }
}
