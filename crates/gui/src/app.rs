//! The window. Drawing only: every change to data and every result goes through `Session::run`
//! with syntax built by the plain modules.

use crate::analyze_dialogs::{
    Correlations, Explore, ExploreMissing, MEANS_CELLS, MULT_RESPONSE_CELLS, Means, MrGroup,
    MultResponse, ONEWAY_STATISTICS, Oneway, STANDARDIZERS, TREE_LEVELS, TREE_SAVE, TTest,
    TTestKind, Tree,
};
use crate::data_dialogs::{
    AGGREGATE_FUNCTIONS, Aggregate, AggregateTo, Aggregated, EXPORT_NAMES, ExportData,
    FILTER_VARIABLE, ImportData, MergeFiles, Mrsets, Restructure, RestructureKind,
    SORT_VARIABLES_KEYS, SelectCases, SelectMode, SortCases, SortVariables, SplitFile, SplitMode,
    TEXT_DELIMITERS, WeightCases, is_workbook,
};
use crate::dialogs::{
    Adjust, Autorecode, CROSSTABS_CELLS, Categories, Compute, Count, Crosstabs, CustomTables,
    DESCRIPTIVES_STATISTICS, Descriptives, FREQUENCIES_ORDERS, FREQUENCIES_STATISTICS, Frequencies,
    Letters, NewValue, OldValue, RANK_FRACTIONS, RANK_FUNCTIONS, RANK_TIES, Rank, Recode,
    RecodeRule, Subtotal, is_set, label_command,
};
use crate::edits::{self, Field};
use crate::model_dialogs::{
    Discriminant, FactorAnalysis, Hierarchical, KMeans, Logistic, RepeatedMeasures, Univariate,
};
use crate::model_dialogs_view;
use crate::output;
use crate::output_view::{self, OutputView};
use crate::settings::Settings;
use crate::settings_window::{self, Section};
use crate::stats_dialogs::{NparKind, NparTests, PartialCorr, Regression, Reliability, numeric};
use crate::stats_dialogs_view;
use crate::syntax_text::{number, quote};
use crate::updates::Updates;
use crate::{complete, editor, grid, grid_menu, highlight, themes};
use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, FocusDirection, FontId, Key, Modifiers, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextStyle, Ui, UiBuilder, pos2,
    text::LayoutJob, vec2,
};
use statscraft_data::{Dataset, Measure, Value, Values};
use statscraft_engine::live::{self, Action, Request, Response};
use statscraft_engine::session::{CommandResult, Done, Failure, RunResult, Session};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The documentation online.
const DOCS: &str = "https://docs.statscraft.org";

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

/// The question about unsaved work, and the saves before the window closes.
enum Ask {
    /// The question, with what is unsaved and what follows the answer.
    Unsaved(Vec<String>, Then),
    /// Save was chosen before closing: the data file still to save once no run is going on;
    /// then the window closes, unless a save failed.
    SavingToClose(Option<PathBuf>),
}

/// What follows the question about unsaved work.
enum Then {
    Close,
    /// The syntax of this file, or new syntax, takes the editor.
    Syntax(Option<PathBuf>),
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
pub(crate) struct CategoryDraft {
    variable: String,
    categories: Vec<(String, String)>,
    label: String,
    chosen: Vec<String>,
}

/// The rule being put together in the Recode dialog.
#[derive(Default)]
pub(crate) struct RuleDraft {
    old: usize,
    low: String,
    high: String,
    new: usize,
    value: String,
}

pub(crate) enum Dialog {
    Frequencies(Frequencies),
    Descriptives(Descriptives),
    Explore(Explore),
    Crosstabs(Crosstabs),
    CustomTables(CustomTables, CategoryDraft),
    Compute(Compute, String),
    Recode(Recode, RuleDraft),
    /// With the label of the new variable and the value being put together.
    Count(Count, String, RuleDraft),
    Rank(Rank),
    Autorecode(Autorecode),
    Means(Means),
    TTest(TTest),
    Oneway(Oneway),
    Correlations(Correlations),
    Mrsets(Mrsets),
    SortCases(SortCases),
    SplitFile(SplitFile),
    SelectCases(SelectCases),
    WeightCases(WeightCases),
    /// With the function chosen for the next aggregated variables.
    Aggregate(Aggregate, usize),
    MergeFiles(MergeFiles),
    MultResponse(MultResponse),
    Tree(Tree),
    Regression(Regression),
    Reliability(Reliability),
    NparTests(NparTests),
    PartialCorr(PartialCorr),
    FactorAnalysis(FactorAnalysis),
    KMeans(KMeans),
    Hierarchical(Hierarchical),
    Discriminant(Discriminant),
    Logistic(Logistic),
    Univariate(Univariate),
    RepeatedMeasures(RepeatedMeasures),
    Restructure(Restructure),
    SortVariables(SortVariables),
    ImportData(ImportData),
    ExportData(ExportData),
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
            Dialog::Rank(_) => "Rank Cases",
            Dialog::Autorecode(_) => "Automatic Recode",
            Dialog::Means(_) => "Means",
            Dialog::TTest(d) => d.title(),
            Dialog::Oneway(_) => "One-Way ANOVA",
            Dialog::Correlations(_) => "Bivariate Correlations",
            Dialog::Mrsets(_) => "Define Multiple Response Sets",
            Dialog::SortCases(_) => "Sort Cases",
            Dialog::SplitFile(_) => "Split File",
            Dialog::SelectCases(_) => "Select Cases",
            Dialog::WeightCases(_) => "Weight Cases",
            Dialog::Aggregate(..) => "Aggregate",
            Dialog::MergeFiles(d) => d.title(),
            Dialog::MultResponse(d) => d.title(),
            Dialog::Tree(_) => "Classification Tree",
            Dialog::Regression(_) => "Linear Regression",
            Dialog::Reliability(_) => "Reliability Analysis",
            Dialog::NparTests(d) => d.title(),
            Dialog::PartialCorr(_) => "Partial Correlations",
            Dialog::FactorAnalysis(_) => "Factor Analysis",
            Dialog::KMeans(_) => "K-Means Cluster Analysis",
            Dialog::Hierarchical(_) => "Hierarchical Cluster Analysis",
            Dialog::Discriminant(_) => "Discriminant Analysis",
            Dialog::Logistic(_) => "Logistic Regression",
            Dialog::Univariate(_) => "Univariate",
            Dialog::RepeatedMeasures(_) => "Repeated Measures",
            Dialog::Restructure(_) => "Restructure",
            Dialog::SortVariables(_) => "Sort Variables",
            Dialog::ImportData(_) => "Open Data",
            Dialog::ExportData(_) => "Export Data",
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
            Dialog::Rank(_) => Dialog::Rank(Default::default()),
            Dialog::Autorecode(_) => Dialog::Autorecode(Default::default()),
            Dialog::Means(_) => Dialog::Means(Default::default()),
            Dialog::TTest(d) => Dialog::TTest(TTest::new(d.kind)),
            Dialog::Oneway(_) => Dialog::Oneway(Default::default()),
            Dialog::Correlations(_) => Dialog::Correlations(Default::default()),
            Dialog::Mrsets(_) => Dialog::Mrsets(Default::default()),
            Dialog::SortCases(_) => Dialog::SortCases(Default::default()),
            Dialog::SplitFile(_) => Dialog::SplitFile(Default::default()),
            Dialog::SelectCases(_) => Dialog::SelectCases(Default::default()),
            Dialog::WeightCases(_) => Dialog::WeightCases(Default::default()),
            Dialog::Aggregate(..) => Dialog::Aggregate(Default::default(), 0),
            Dialog::MergeFiles(d) => Dialog::MergeFiles(MergeFiles {
                add_variables: d.add_variables,
                ..Default::default()
            }),
            Dialog::MultResponse(d) => Dialog::MultResponse(MultResponse {
                crosstabs: d.crosstabs,
                ..Default::default()
            }),
            Dialog::Tree(_) => Dialog::Tree(Default::default()),
            Dialog::Regression(_) => Dialog::Regression(Default::default()),
            Dialog::Reliability(_) => Dialog::Reliability(Default::default()),
            Dialog::NparTests(d) => Dialog::NparTests(NparTests::new(d.kind)),
            Dialog::PartialCorr(_) => Dialog::PartialCorr(Default::default()),
            Dialog::FactorAnalysis(_) => Dialog::FactorAnalysis(Default::default()),
            Dialog::KMeans(_) => Dialog::KMeans(Default::default()),
            Dialog::Hierarchical(_) => Dialog::Hierarchical(Default::default()),
            Dialog::Discriminant(_) => Dialog::Discriminant(Default::default()),
            Dialog::Logistic(_) => Dialog::Logistic(Default::default()),
            Dialog::Univariate(_) => Dialog::Univariate(Default::default()),
            Dialog::RepeatedMeasures(_) => Dialog::RepeatedMeasures(Default::default()),
            Dialog::Restructure(_) => Dialog::Restructure(Default::default()),
            Dialog::SortVariables(_) => Dialog::SortVariables(Default::default()),
            Dialog::ImportData(d) => Dialog::ImportData(ImportData::new(Path::new(&d.path))),
            Dialog::ExportData(_) => Dialog::ExportData(Default::default()),
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
            Dialog::Rank(d) => match d.variables.iter().find(|n| is_string(data, n)) {
                Some(name) => Err(format!(
                    "{name} is a string; Automatic Recode makes it a number to rank"
                )),
                None => d.syntax(),
            },
            Dialog::Autorecode(d) => {
                let strings = d.variables.iter().filter(|n| is_string(data, n)).count();
                if d.group && strings > 0 && strings < d.variables.len() {
                    return Err("one scheme needs all numeric or all string variables".into());
                }
                d.syntax()
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
                let text = d.syntax(strings > 0)?;
                if !d.delete && d.dichotomy && d.counted_value_labels {
                    counted_labels(data, &d.variables, &d.value)?;
                }
                Ok(text)
            }
            Dialog::SortCases(d) => d.syntax(),
            Dialog::SplitFile(d) => d.syntax(),
            Dialog::SelectCases(d) => d.syntax(),
            Dialog::WeightCases(d) => d.syntax(),
            Dialog::Aggregate(d, _) => d.syntax(),
            Dialog::MergeFiles(d) => d.syntax(),
            Dialog::MultResponse(d) => {
                let mut names = d.groups.iter().flat_map(|g| &g.variables);
                match names
                    .find(|n| is_string(data, n))
                    .or_else(|| d.table_variables().into_iter().find(|n| is_string(data, n)))
                {
                    Some(name) => Err(format!("{name} is a string; the command counts numbers")),
                    None => d.syntax(),
                }
            }
            Dialog::Tree(d) => {
                let scale = data
                    .variable(&d.dependent)
                    .is_ok_and(|v| v.measure() == Measure::Scale);
                if scale && TREE_LEVELS[d.dependent_level].0.is_empty() {
                    return Err(format!(
                        "{} is a scale variable; choose Nominal or Ordinal",
                        d.dependent
                    ));
                }
                d.syntax(is_string(data, &d.dependent))
            }
            Dialog::Regression(d) => numeric(data, d.variables()).and_then(|()| d.syntax()),
            Dialog::Reliability(d) => numeric(data, &d.items).and_then(|()| d.syntax()),
            Dialog::NparTests(d) => numeric(data, d.variables()).and_then(|()| d.syntax()),
            Dialog::PartialCorr(d) => {
                numeric(data, d.variables.iter().chain(&d.controls)).and_then(|()| d.syntax())
            }
            Dialog::FactorAnalysis(d) => numeric(data, &d.variables).and_then(|()| d.syntax()),
            Dialog::KMeans(d) => numeric(data, &d.variables).and_then(|()| d.syntax()),
            // the label variable may be a string
            Dialog::Hierarchical(d) => numeric(data, &d.variables).and_then(|()| d.syntax()),
            Dialog::Discriminant(d) => numeric(data, d.numeric()).and_then(|()| d.syntax()),
            // a string covariate is categorical, a string dependent has its two values
            Dialog::Logistic(d) => d.syntax(),
            // factors may be strings
            Dialog::Univariate(d) => {
                numeric(data, std::iter::once(&d.dependent).chain(&d.covariates))
                    .and_then(|()| d.syntax())
            }
            Dialog::RepeatedMeasures(d) => {
                numeric(data, d.dependents.iter().chain(&d.covariates)).and_then(|()| d.syntax())
            }
            Dialog::Restructure(d) => d.syntax(),
            Dialog::SortVariables(d) => d.syntax(),
            Dialog::ImportData(d) => d.syntax(),
            Dialog::ExportData(d) => d.syntax(),
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
    /// A click on a row number or a column header.
    row_clicked: Option<usize>,
    column_clicked: Option<usize>,
    /// A column border dragged: the column, its new width, and whether the drag ended.
    resized: Option<(usize, f32, bool)>,
    /// Rows in view, for Page Up and Page Down.
    page: usize,
    /// Where a right click landed, for the grid's menu.
    secondary: Option<grid::Hit>,
    /// A drag with the left button: where it began and what is under the pointer now.
    dragged: Option<(grid::Hit, grid::Hit)>,
    /// The grid's response, which opens its menu.
    response: Option<egui::Response>,
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
    selected: Option<grid::Selection>,
    /// Scroll the selected cell into view on the next frame.
    follow: bool,
    page: usize,
    cell_edit: Option<CellEdit>,
    /// A Data View column being dragged to a new width, until its VARIABLE WIDTH has run.
    data_resize: Option<(usize, f32)>,
    variable_selected: Option<grid::Selection>,
    variable_follow: bool,
    field_edit: Option<CellEdit>,
    /// The text of the find bar over the grids, while it is open.
    find: Option<String>,
    /// Put the keyboard in the find bar on the next frame.
    find_focus: bool,
    /// A question before syntax runs, such as deleting variables: it runs once the user says yes.
    confirm: Option<grid_menu::Confirm>,
    /// What the open right-click menu of a grid acts on, and whether it is the Variable View's.
    menu: Option<(grid_menu::Target, bool)>,
    /// The Type and Format dialog of a variable.
    format_dialog: Option<edits::FormatChoice>,
    labels_dialog: Option<edits::LabelsEditor>,
    missing_dialog: Option<edits::MissingEditor>,
    /// The Variable View rows selected before the last plain click, which the first click of
    /// a double-click on one of them takes away.
    rows_before_click: BTreeSet<usize>,
    syntax: String,
    syntax_file: Option<PathBuf>,
    syntax_errors: Vec<Failure>,
    selection: (usize, usize),
    /// The data file before the last change, which undo puts back too.
    file_before_change: Option<PathBuf>,
    /// The syntax of the run going on or done last, for the syntax of its output.
    source: String,
    /// The full help of a command, from F1.
    help_window: Option<String>,
    completion: Option<Completion>,
    /// The macros defined by the runs of the session, for completion and colours.
    macros: Vec<String>,
    /// The About window is open.
    about: bool,
    /// The Settings dialog is open at this section.
    settings_section: Option<Section>,
    /// The dataset as the last open or save left it, to tell whether the data changed since.
    data_saved: Option<Dataset>,
    /// The syntax text as last loaded or saved; empty for new syntax.
    syntax_saved: String,
    ask: Option<Ask>,
    /// The user chose to close: the next close request goes through.
    close_ok: bool,
    /// The app icon, shown in the About window.
    logo: egui::TextureHandle,
    editor_id: Option<egui::Id>,
    output: OutputView,
    /// Window size and position, the last folder, the recent files and the update check, kept
    /// between starts.
    settings: Settings,
    updates: Updates,
    dialog: Option<Dialog>,
    /// The multiple response groups of the last Multiple Response dialog, for the next one.
    groups: Vec<MrGroup>,
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
        let logo = logo(ctx);
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
        app_ctx.all_styles_mut(|style| {
            square(&mut style.visuals);
            // no fade at the edges of a scroll area: flat
            style.spacing.scroll.fade.strength = 0.0;
        });
        let updates = Updates::new(&app_ctx, &mut settings);
        let mut app = App {
            session: Session::default(),
            tab: Tab::Data,
            data_file: None,
            value_labels: true,
            show_log: !settings.log_hidden,
            selected: None,
            follow: false,
            page: 20,
            cell_edit: None,
            data_resize: None,
            variable_selected: None,
            variable_follow: false,
            field_edit: None,
            find: None,
            find_focus: false,
            confirm: None,
            menu: None,
            format_dialog: None,
            labels_dialog: None,
            missing_dialog: None,
            rows_before_click: BTreeSet::new(),
            syntax: String::new(),
            syntax_file: None,
            syntax_errors: Vec::new(),
            selection: (0, 0),
            file_before_change: None,
            source: String::new(),
            help_window: None,
            completion: None,
            macros: Vec::new(),
            about: false,
            settings_section: None,
            data_saved: None,
            syntax_saved: String::new(),
            ask: None,
            close_ok: false,
            logo,
            editor_id: None,
            output: OutputView::default(),
            settings,
            updates,
            dialog: None,
            groups: Vec::new(),
            picked: Vec::new(),
            filter: String::new(),
            status,
            warnings: 0,
            title: "Statscraft".into(),
            requests,
            busy: None,
            ctx: app_ctx,
            server: server.ok(),
        };
        // the table labels chosen in Edit > Settings, as a SET the session then holds
        if let Some(syntax) = app.settings.startup_syntax() {
            app.source = syntax.clone();
            if let Err(errors) = app.session.run(&syntax, None) {
                app.status = Err(error_text(&errors));
            }
        }
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
        let folder = self.syntax_file.as_ref().and_then(|f| f.parent());
        self.session.set_base_dir(folder.map(Path::to_path_buf));
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
            self.macros = session_macros(&self.session.log);
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
        let loaded = matches!(&result, Ok(r) if r.commands.iter().any(|c| c.name == "GET" || c.name == "GET DATA"));
        let on_file = matches!(&result, Ok(r) if ends_on_file(&r.commands));
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
                    if title == "Open Data" {
                        self.selected = None;
                        self.tab = Tab::Data;
                    }
                }
            }
        }
        // a check returned above: it writes no file
        if on_file {
            self.data_saved = self.session.dataset().cloned();
        }
    }

    /// Puts back the dataset and the data file from before the last change.
    fn undo(&mut self) {
        if self.busy.is_some() {
            return;
        }
        let syntax = self.session.undoable().unwrap_or_default().to_string();
        if self.session.undo() {
            self.macros = session_macros(&self.session.log);
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
            self.replace_syntax(Some(path));
        }
        match drop.data {
            Some(path) if is_data_file(&path) => self.open_data(path),
            Some(path) => {
                self.settings.remember_folder(&path);
                self.open_dialog(Dialog::ImportData(ImportData::new(&path)));
            }
            None => {}
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
        let text = std::fs::read(&path).map_err(|e| e.to_string());
        match text.and_then(|bytes| statscraft_syntax::decode_syntax(&bytes, None)) {
            Ok(text) => {
                self.add_recent(&path);
                self.syntax_saved = text.clone();
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
                self.syntax_saved = self.syntax.clone();
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
                        .add_filter("Excel workbooks", &["xlsx", "xlsm", "xls"])
                        .add_filter("Text files", &["csv", "txt", "tab", "dat"])
                        .pick_file()
                {
                    if is_data_file(&path) {
                        self.open_data(path);
                    } else {
                        // the folder is kept even when the dialog is cancelled
                        self.settings.remember_folder(&path);
                        self.open_dialog(Dialog::ImportData(ImportData::new(&path)));
                    }
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
                if ui
                    .add_enabled(has_data, egui::Button::new("Export data..."))
                    .clicked()
                {
                    self.open_dialog(Dialog::ExportData(Default::default()));
                }
                ui.separator();
                if ui.button("New syntax").clicked() {
                    self.replace_syntax(None);
                }
                if ui.button("Open syntax...").clicked()
                    && let Some(path) = self
                        .file_dialog()
                        .add_filter("Syntax", &["sps"])
                        .pick_file()
                {
                    self.replace_syntax(Some(path));
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
                                    self.replace_syntax(Some(path));
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
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
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
                ui.separator();
                if ui.button("Settings...").clicked() {
                    self.settings_section = Some(Section::default());
                }
            });
            ui.menu_button("Data", |ui| {
                let data = self.session.dataset();
                let weight = data.and_then(|d| d.weight_variable()).map(str::to_string);
                let split = SplitFile::current(data.and_then(|d| d.split_file()));
                for (label, dialog) in [
                    (
                        "Define Multiple Response Sets...",
                        Dialog::Mrsets(Default::default()),
                    ),
                    ("Sort Cases...", Dialog::SortCases(Default::default())),
                    (
                        "Sort Variables...",
                        Dialog::SortVariables(Default::default()),
                    ),
                    ("Restructure...", Dialog::Restructure(Default::default())),
                    ("Split File...", Dialog::SplitFile(split)),
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
                    ui.menu_button("General Linear Model", |ui| {
                        if ui.button("Univariate...").clicked() {
                            self.open_dialog(Dialog::Univariate(Default::default()));
                        }
                        if ui.button("Repeated Measures...").clicked() {
                            self.open_dialog(Dialog::RepeatedMeasures(Default::default()));
                        }
                    });
                    ui.menu_button("Correlate", |ui| {
                        if ui.button("Bivariate...").clicked() {
                            self.open_dialog(Dialog::Correlations(Default::default()));
                        }
                        if ui.button("Partial...").clicked() {
                            self.open_dialog(Dialog::PartialCorr(Default::default()));
                        }
                    });
                    ui.menu_button("Regression", |ui| {
                        if ui.button("Linear...").clicked() {
                            self.open_dialog(Dialog::Regression(Default::default()));
                        }
                        if ui.button("Binary Logistic...").clicked() {
                            self.open_dialog(Dialog::Logistic(Default::default()));
                        }
                    });
                    ui.menu_button("Classify", |ui| {
                        if ui.button("K-Means Cluster...").clicked() {
                            self.open_dialog(Dialog::KMeans(Default::default()));
                        }
                        if ui.button("Hierarchical Cluster...").clicked() {
                            self.open_dialog(Dialog::Hierarchical(Default::default()));
                        }
                        if ui.button("Discriminant...").clicked() {
                            self.open_dialog(Dialog::Discriminant(Default::default()));
                        }
                        if ui.button("Tree...").clicked() {
                            self.open_dialog(Dialog::Tree(Default::default()));
                        }
                    });
                    ui.menu_button("Dimension Reduction", |ui| {
                        if ui.button("Factor...").clicked() {
                            self.open_dialog(Dialog::FactorAnalysis(Default::default()));
                        }
                    });
                    ui.menu_button("Scale", |ui| {
                        if ui.button("Reliability Analysis...").clicked() {
                            self.open_dialog(Dialog::Reliability(Default::default()));
                        }
                    });
                    ui.menu_button("Nonparametric Tests", |ui| {
                        for kind in NparKind::ALL {
                            if ui.button(kind.menu()).clicked() {
                                self.open_dialog(Dialog::NparTests(NparTests::new(kind)));
                            }
                        }
                    });
                    ui.menu_button("Multiple Response", |ui| {
                        for (label, crosstabs) in
                            [("Frequencies...", false), ("Crosstabs...", true)]
                        {
                            if ui.button(label).clicked() {
                                self.open_dialog(Dialog::MultResponse(MultResponse {
                                    crosstabs,
                                    ..Default::default()
                                }));
                            }
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
                ui.separator();
                for (label, dialog) in [
                    ("Rank Cases...", Dialog::Rank(Default::default())),
                    (
                        "Automatic Recode...",
                        Dialog::Autorecode(Default::default()),
                    ),
                ] {
                    if ui.add_enabled(has_data, egui::Button::new(label)).clicked() {
                        self.open_dialog(dialog);
                    }
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Documentation").on_hover_text(DOCS).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DOCS));
                }
                if ui.button("About Statscraft").clicked() {
                    self.about = true;
                }
                ui.separator();
                self.updates.menu_items(ui);
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

    fn open_dialog(&mut self, mut dialog: Dialog) {
        if let Dialog::MultResponse(d) = &mut dialog {
            d.groups = self.groups.clone();
        }
        self.dialog = Some(dialog);
        self.picked.clear();
        self.filter.clear();
    }

    fn save_data_as(&mut self) {
        if let Some(path) = self.data_file_dialog() {
            self.save_data(path);
        }
    }

    fn data_file_dialog(&self) -> Option<PathBuf> {
        self.file_dialog()
            .add_filter("Data file", &["sav"])
            .add_filter("Compressed data file", &["zsav"])
            .save_file()
    }

    /// What would be lost if the window closed now.
    fn unsaved(&self) -> Vec<String> {
        unsaved_items(
            self.session.dataset(),
            self.data_saved.as_ref(),
            self.data_file.as_deref(),
            &self.syntax,
            &self.syntax_saved,
            self.syntax_file.as_deref(),
        )
    }

    /// Every close comes here as a close request: the title bar, Alt+F4, File > Exit, and the
    /// update installer or restart, which send `ViewportCommand::Close`. With unsaved work the
    /// window stays open and asks; with Save chosen it closes once the saves are done.
    fn close_request(&mut self, ctx: &egui::Context) {
        if self.close_ok {
            return;
        }
        if let Some(Ask::SavingToClose(pending)) = &mut self.ask
            && self.busy.is_none()
        {
            match pending.take() {
                Some(path) => self.save_data(path),
                None if self.unsaved().is_empty() => {
                    self.close_ok = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                // a save failed, and the status bar says why
                None => self.keep_open(),
            }
        }
        if self.ask.is_some() {
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                // a close during the syntax question is dropped, a restart with it
                if matches!(self.ask, Some(Ask::Unsaved(_, Then::Syntax(_)))) {
                    self.updates.cancel_restart();
                }
            }
        } else if let Some(items) = hold_close(ctx, || self.unsaved()) {
            self.ask = Some(Ask::Unsaved(items, Then::Close));
        }
    }

    /// The window stays open after all: no saves wait, and no restart.
    fn keep_open(&mut self) {
        self.ask = None;
        self.updates.cancel_restart();
    }

    /// New syntax (`None`) or a syntax file takes the editor; unsaved syntax is asked about
    /// first.
    fn replace_syntax(&mut self, next: Option<PathBuf>) {
        if self.ask.is_some() {
            return;
        }
        let file = self.syntax_file.as_deref();
        let items = unsaved_items(None, None, None, &self.syntax, &self.syntax_saved, file);
        if items.is_empty() {
            self.take_syntax(next);
        } else {
            self.ask = Some(Ask::Unsaved(items, Then::Syntax(next)));
        }
    }

    fn take_syntax(&mut self, next: Option<PathBuf>) {
        match next {
            Some(path) => self.load_syntax(path),
            None => {
                self.syntax.clear();
                self.syntax_saved.clear();
                self.syntax_file = None;
                self.syntax_errors.clear();
                self.tab = Tab::Syntax;
            }
        }
    }

    /// Saves the syntax to its file, or to one chosen now; false when it was not saved.
    fn save_syntax_now(&mut self) -> bool {
        match self.syntax_file.clone() {
            Some(path) => self.save_syntax(path),
            None => self.save_syntax_as(),
        }
        self.syntax == self.syntax_saved
    }

    /// The question about unsaved work, before closing or before other syntax takes the
    /// editor: Save, Don't save or Cancel. Escape or a click beside it cancels.
    fn unsaved_window(&mut self, ctx: &egui::Context) {
        let Some(Ask::Unsaved(items, then)) = &self.ask else {
            return;
        };
        enum Answer {
            Save,
            Discard,
            Cancel,
        }
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::unique("unsaved")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(match then {
                Then::Close => "Save the changes before closing?",
                Then::Syntax(_) => "Save the changes to the syntax first?",
            });
            ui.add_space(4.0);
            for item in items {
                ui.label(item);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add(crate::brand::primary(ui, "Save")).clicked() {
                    answer = Some(Answer::Save);
                }
                if ui.button("Don't save").clicked() {
                    answer = Some(Answer::Discard);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(Answer::Cancel);
        }
        let Some(answer) = answer else {
            return;
        };
        let Some(Ask::Unsaved(_, then)) = self.ask.take() else {
            return;
        };
        match (answer, then) {
            (Answer::Save, Then::Close) => self.save_before_close(),
            (Answer::Discard, Then::Close) => {
                self.close_ok = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Answer::Cancel, Then::Close) => self.keep_open(),
            (Answer::Save, Then::Syntax(next)) => {
                if self.save_syntax_now() {
                    self.take_syntax(next);
                }
            }
            (Answer::Discard, Then::Syntax(next)) => self.take_syntax(next),
            (Answer::Cancel, Then::Syntax(_)) => {}
        }
    }

    /// Saves the syntax, then the data, each to its file or to one chosen now; a cancelled
    /// file dialog or a failed save keeps the window open.
    fn save_before_close(&mut self) {
        if self.syntax != self.syntax_saved && !self.save_syntax_now() {
            self.keep_open();
            return;
        }
        let data = self.session.dataset();
        let pending = if data.is_some() && data != self.data_saved.as_ref() {
            match self.data_file.clone().or_else(|| self.data_file_dialog()) {
                Some(path) => Some(path),
                None => {
                    self.keep_open();
                    return;
                }
            }
        } else {
            None
        };
        self.ask = Some(Ask::SavingToClose(pending));
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

    fn status_bar(&mut self, ui: &mut Ui) {
        if let Some(busy) = &self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&busy.label);
            });
            return;
        }
        ui.horizontal(|ui| {
            // the split, filter and weight in force, each opening its dialog
            let mut open = None;
            if let Some(states) = self.session.dataset().map(data_states) {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    for (text, dialog) in states.into_iter().rev() {
                        if ui.link(text).clicked() {
                            open = Some(dialog);
                        }
                    }
                    self.status_line(ui);
                });
            } else {
                self.status_line(ui);
            }
            if let Some(dialog) = open {
                self.open_dialog(dialog);
            }
        });
    }

    /// The result of the last run and its warnings; a click on the warnings shows the log,
    /// from the Data and Variable Views in the Syntax view.
    fn status_line(&mut self, ui: &mut Ui) {
        ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
            if let Some(note) = warning_note(self.warnings)
                && ui
                    .link(RichText::new(note).color(ui.visuals().warn_fg_color))
                    .clicked()
            {
                self.show_log = true;
                if !shows_log(self.tab) {
                    self.tab = Tab::Syntax;
                }
            }
            // a long message is cut at the links of the split, filter and weight
            let text = match &self.status {
                Ok(text) => RichText::new(text).color(ui.visuals().weak_text_color()),
                Err(text) => RichText::new(text).color(ui.visuals().error_fg_color),
            };
            let full = text.text().to_string();
            ui.add(egui::Label::new(text).truncate())
                .on_hover_text(full);
        });
    }

    fn log(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("Log");
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
        });
    }

    fn data_view(&mut self, ui: &mut Ui) {
        if self.session.dataset().is_none() {
            return Self::no_data(ui);
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.value_labels, "Show value labels");
            self.find_bar(ui);
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
        let currencies = self.session.currencies();
        let cell = |r: usize, c: usize| {
            let variable = data.variable_at(c);
            let labelled = labels && !variable.value_labels().is_empty();
            let sysmis = matches!(variable.values(), Values::Numeric(v) if v.get(r).is_none());
            let text = edits::cell_text(&variable, r, labelled, &currencies);
            ((!sysmis).then_some(text), cell_align(&variable))
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
            self.selected.as_ref(),
            std::mem::take(&mut self.follow),
            &cell,
            &filtered_out(data),
            edit.as_ref().map(|e| (e.row, e.column)),
            &mut |ui, rect| {
                if let Some(e) = edit.as_mut() {
                    (commit, cancel) = cell_editor(ui, rect, e);
                }
            },
        );
        self.page = events.page.max(1);
        let modifiers = ui.input(|i| i.modifiers);
        if select(&mut self.selected, &events, modifiers) {
            self.follow = true;
        }
        self.open_menu(events.secondary);
        if let Some(response) = &events.response {
            response.context_menu(|ui| self.grid_menu(ui));
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
        self.selected = Some(grid::Selection::cell((row, column)));
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

    /// The keyboard in the Data View and the Variable View, read before any widget sees the
    /// keys: arrows, Page Up and Down, Home and End, with Ctrl to the edges, with Shift to
    /// extend the selection; Tab and Shift+Tab; Enter to edit; Ctrl+C copies the selection;
    /// Delete deletes the selected variables or clears the selected cells. While a Data View cell is
    /// edited, Tab and the up and down arrows commit it and move on; Enter commits and Escape
    /// cancels in the editor itself.
    fn grid_keys(&mut self, ui: &mut Ui) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let variables = self.tab == Tab::Variables;
        let (rows, columns, editing) = match self.tab {
            Tab::Data => (
                data.case_count(),
                data.variable_count(),
                self.cell_edit.is_some(),
            ),
            Tab::Variables => (data.variable_count(), Field::ALL.len(), false),
            _ => return,
        };
        if self.dialog.is_some()
            || self.confirm.is_some()
            || self.format_dialog.is_some()
            || self.labels_dialog.is_some()
            || self.missing_dialog.is_some()
            || (variables && self.field_edit.is_some())
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
        let (mut enter, mut copy, mut delete) = (false, false, false);
        ui.input_mut(|i| {
            for &key in keys {
                // the most modifiers first: a shortcut matches with extra Shift too
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, key) {
                    moves.push((key, true, true));
                } else if i.consume_key(Modifiers::COMMAND, key) {
                    moves.push((key, true, false));
                } else if i.consume_key(Modifiers::SHIFT, key) {
                    moves.push((key, false, true));
                } else if i.consume_key(Modifiers::NONE, key) {
                    moves.push((key, false, false));
                }
            }
            enter = !editing && i.consume_key(Modifiers::NONE, Key::Enter);
            copy = !editing && i.events.iter().any(|e| matches!(e, egui::Event::Copy));
            delete = !editing && i.consume_key(Modifiers::NONE, Key::Delete);
        });
        if copy {
            self.copy_selection(ui.ctx());
        }
        if delete {
            self.ask_delete();
        }
        if moves.is_empty() && !enter {
            return;
        }
        // no egui focus traversal with these keys: the grid has them
        ui.memory_mut(|m| m.move_focus(FocusDirection::None));
        if editing {
            self.commit_cell();
        }
        let (selection, follow) = if variables {
            (&mut self.variable_selected, &mut self.variable_follow)
        } else {
            (&mut self.selected, &mut self.follow)
        };
        let s = selection.get_or_insert_with(Default::default);
        for (key, ctrl, shift) in moves {
            let tab = key == Key::Tab;
            let at = grid::moved(
                key,
                ctrl,
                shift && tab,
                s.cursor,
                (rows, columns),
                self.page,
            );
            if shift && !tab {
                s.extend_to(at);
            } else {
                *s = grid::Selection::cell(at);
            }
        }
        *follow = true;
        let at = s.cursor;
        if enter && variables {
            self.edit_field(at);
        } else if enter {
            self.edit_cell(at);
        }
    }

    /// Ctrl+F opens the find bar, F3 goes to the next match.
    fn find_keys(&mut self, ui: &mut Ui) {
        if !matches!(self.tab, Tab::Data | Tab::Variables) || self.dialog.is_some() {
            return;
        }
        let (open, next) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::NONE, Key::F3),
            )
        });
        if open {
            self.find.get_or_insert_with(String::new);
            self.find_focus = true;
        }
        if next {
            self.find_next(false);
        }
    }

    /// The find bar, while it is open: typing goes to the first match from the current
    /// variable, Enter to the next one, Escape closes it.
    fn find_bar(&mut self, ui: &mut Ui) {
        let Some(text) = self.find.as_mut() else {
            return;
        };
        let response = ui.add(
            TextEdit::singleline(text)
                .hint_text("Find a variable by name or label")
                .desired_width(260.0),
        );
        if std::mem::take(&mut self.find_focus) {
            response.request_focus();
        }
        let (enter, escape) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if response.changed() {
            self.find_next(true);
        }
        if response.lost_focus() && enter {
            self.find_next(false);
            response.request_focus();
        }
        if (response.lost_focus() || response.has_focus()) && escape {
            self.find = None;
        }
    }

    /// Goes to the next variable that matches the find text: its row in the Variable View, its
    /// column in the Data View. `here` counts the current variable as a match too.
    fn find_next(&mut self, here: bool) {
        let (Some(needle), Some(data)) = (self.find.clone(), self.session.dataset()) else {
            return;
        };
        let items: Vec<(String, String)> = data
            .variables()
            .map(|v| {
                (
                    v.name().to_string(),
                    v.label().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let variables = self.tab == Tab::Variables;
        let cursor = if variables {
            self.variable_selected.as_ref().map(|s| s.cursor.0)
        } else {
            self.selected.as_ref().map(|s| s.cursor.1)
        };
        let after = if here {
            cursor.and_then(|c| c.checked_sub(1))
        } else {
            cursor
        };
        match grid::find(&items, &needle, after) {
            Some(i) if variables => {
                self.variable_selected = Some(grid::Selection::cell((i, 0)));
                self.variable_follow = true;
            }
            Some(i) => {
                let row = self.selected.as_ref().map_or(0, |s| s.cursor.0);
                self.selected = Some(grid::Selection::cell((row, i)));
                self.follow = true;
            }
            None if needle.trim().is_empty() => {}
            None => self.status = Err(format!("no variable name or label holds {needle:?}")),
        }
    }

    /// Puts the selection of the grid in view on the clipboard as tab-separated text.
    fn copy_selection(&mut self, ctx: &egui::Context) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let (text, wide) = if self.tab == Tab::Variables {
            let Some(s) = &self.variable_selected else {
                return;
            };
            let headers: Vec<String> = Field::ALL.iter().map(|f| f.title().to_string()).collect();
            let size = (data.variable_count(), Field::ALL.len());
            let text = grid::copy(s, size, &headers, |r, c| {
                edits::field_text(&data.variable_at(r), Field::ALL[c])
            });
            (text, s.is_wide())
        } else {
            let Some(s) = &self.selected else {
                return;
            };
            let headers: Vec<String> = data.variable_names().map(str::to_string).collect();
            let size = (data.case_count(), data.variable_count());
            let labels = self.value_labels;
            let currencies = self.session.currencies();
            let text = grid::copy(s, size, &headers, |r, c| {
                edits::cell_text(&data.variable_at(c), r, labels, &currencies)
            });
            (text, s.is_wide())
        };
        let text = match text {
            Ok(text) => text,
            Err(message) => return self.status = Err(message),
        };
        let lines = text.lines().count().saturating_sub(1);
        ctx.copy_text(text);
        self.status = Ok(match wide {
            true => format!("{lines} row(s) copied with their headers"),
            false => "the cell copied".into(),
        });
    }

    /// Delete: asks before deleting the variables whose rows (Variable View) or columns (Data
    /// View) are selected; in the Data View it clears the selected cells otherwise.
    fn ask_delete(&mut self) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let variable_view = self.tab == Tab::Variables;
        if !variable_view
            && let Some(s) = &self.selected
            && s.rows.is_empty()
            && s.columns.is_empty()
        {
            let (rows, columns) = s.span((data.case_count(), data.variable_count()));
            if let (Some(&first), Some(&last)) = (rows.first(), rows.last()) {
                let variables: Vec<_> = columns.iter().map(|&c| data.variable_at(c)).collect();
                let syntax = edits::clear_cells(&variables, first, last);
                self.run(&syntax, None, After::Show);
            }
            return;
        }
        let picked = match variable_view {
            true => self.variable_selected.as_ref().map(|s| &s.rows),
            false => self.selected.as_ref().map(|s| &s.columns),
        };
        let picked: Vec<usize> = picked.into_iter().flatten().copied().collect();
        if picked.is_empty() {
            if variable_view {
                self.status = Err("select variables by their row numbers to delete them".into());
            }
            return;
        }
        let target = grid_menu::Target::Variables(picked);
        if let Ok(grid_menu::Action::Ask(confirm)) =
            grid_menu::action(data, &target, grid_menu::Item::Delete)
        {
            self.confirm = Some(confirm);
        }
    }

    /// The Type and Format dialog: a format from a list, its width and decimals.
    fn format_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.format_dialog.as_mut() else {
            return;
        };
        let mut answer = None;
        let syntax = d.syntax();
        egui::Window::new(format!("Type and format of {}", d.name))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                egui::Grid::new("format grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Format");
                        let before = d.kind;
                        ComboBox::new("format kind", "")
                            .selected_text(edits::FORMAT_KINDS[d.kind].label)
                            .height(400.0)
                            .show_index(ui, &mut d.kind, edits::FORMAT_KINDS.len(), |i| {
                                edits::FORMAT_KINDS[i].label
                            });
                        if d.kind != before {
                            d.fit();
                        }
                        ui.end_row();
                        ui.label("Width");
                        ui.add(TextEdit::singleline(&mut d.width).desired_width(60.0));
                        ui.end_row();
                        let decimals = edits::FORMAT_KINDS[d.kind].decimals;
                        ui.label("Decimals");
                        ui.add_enabled(
                            decimals,
                            TextEdit::singleline(&mut d.decimals).desired_width(60.0),
                        );
                        ui.end_row();
                    });
                if d.changes_type() {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "A new type drops the value labels and missing values.",
                    );
                }
                answer = dialog_footer(ui, &syntax, false);
            });
        if self.finish_small(answer, syntax) {
            self.format_dialog = None;
        }
    }

    /// The value labels dialog: a list of codes and labels, or many pasted at once.
    fn labels_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.labels_dialog.as_mut() else {
            return;
        };
        let syntax = d.syntax();
        let mut answer = None;
        egui::Window::new(format!("Value labels of {}", d.name))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut d.quick, false, "One by one");
                    ui.selectable_value(&mut d.quick, true, "Paste many");
                });
                ui.separator();
                if d.quick {
                    ui.add(
                        TextEdit::multiline(&mut d.pasted)
                            .desired_width(f32::INFINITY)
                            .desired_rows(8)
                            .font(TextStyle::Monospace),
                    );
                    ui.radio_value(
                        &mut d.replace,
                        false,
                        "Update: add new codes, change the labels of codes there are",
                    );
                    ui.radio_value(
                        &mut d.replace,
                        true,
                        "Replace: the pasted list is the whole set",
                    );
                } else {
                    let mut remove = None;
                    ScrollArea::vertical()
                        .max_height(300.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let row = ui.text_style_height(&TextStyle::Body) + 6.0;
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [80.0, row],
                                    egui::Label::new(RichText::new("Code").strong()),
                                );
                                ui.strong("Label");
                            });
                            for (i, (code, label)) in d.pairs.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add_sized([80.0, row], TextEdit::singleline(code));
                                    ui.add_sized([320.0, row], TextEdit::singleline(label));
                                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            }
                        });
                    if let Some(i) = remove {
                        d.pairs.remove(i);
                    }
                    if ui.button("Add a label").clicked() {
                        d.pairs.push(Default::default());
                    }
                }
                if !d.others.is_empty() {
                    ui.checkbox(
                        &mut d.apply_to_others,
                        format!("Also for {}", edits::listed(&d.others)),
                    );
                }
                answer = dialog_footer(ui, &syntax, false);
            });
        if self.finish_small(answer, syntax) {
            self.labels_dialog = None;
        }
    }

    /// The missing values dialog: up to three values, or a range and one more value.
    fn missing_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.missing_dialog.as_mut() else {
            return;
        };
        let syntax = d.syntax();
        let mut answer = None;
        egui::Window::new(format!("Missing values of {}", d.name))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.radio_value(&mut d.range, false, "Up to three values");
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    for v in &mut d.values {
                        ui.add_enabled(!d.range, TextEdit::singleline(v).desired_width(80.0));
                    }
                });
                if !d.string {
                    ui.radio_value(&mut d.range, true, "A range and one value more");
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.add_enabled(
                            d.range,
                            TextEdit::singleline(&mut d.low).desired_width(70.0),
                        );
                        ui.label("through");
                        ui.add_enabled(
                            d.range,
                            TextEdit::singleline(&mut d.high).desired_width(70.0),
                        );
                        ui.label("and");
                        ui.add_enabled(
                            d.range,
                            TextEdit::singleline(&mut d.extra).desired_width(70.0),
                        );
                    });
                }
                if !d.others.is_empty() {
                    ui.checkbox(
                        &mut d.apply_to_others,
                        format!("Also for {}", edits::listed(&d.others)),
                    );
                }
                answer = dialog_footer(ui, &syntax, false);
            });
        if self.finish_small(answer, syntax) {
            self.missing_dialog = None;
        }
    }

    /// Does what OK, Paste or Cancel of a small dialog asks for; true when the dialog closes.
    fn finish_small(
        &mut self,
        action: Option<DialogAction>,
        syntax: Result<String, String>,
    ) -> bool {
        match (action, syntax) {
            (Some(DialogAction::Ok), Ok(text)) => self.run(&text, None, After::Show),
            (Some(DialogAction::Paste), Ok(text)) => self.paste(&text),
            (Some(DialogAction::Cancel), _) => {}
            _ => return false,
        }
        true
    }

    /// Puts syntax at the end of the syntax editor and shows it.
    fn paste(&mut self, text: &str) {
        editor::append(&mut self.syntax, text);
        self.tab = Tab::Syntax;
    }

    /// The question before syntax that deletes or reorders runs: the question, the syntax, the
    /// button that runs it and Cancel. Only a click runs it, so a stray Enter never deletes;
    /// Escape or a click beside the window cancels. A yes clears the selections, which may
    /// point at other variables or cases afterwards.
    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.confirm else {
            return;
        };
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(&c.question);
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(&c.syntax).monospace()).wrap());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(c.button).clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(false);
        }
        match answer {
            Some(true) => {
                let syntax = self.confirm.take().map(|c| c.syntax).unwrap_or_default();
                self.selected = None;
                self.variable_selected = None;
                self.run(&syntax, None, After::Show);
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    /// A right click on a grid: the menu acts on what the click selected.
    fn open_menu(&mut self, hit: Option<grid::Hit>) {
        let Some(hit) = hit else {
            return;
        };
        let variable_view = self.tab == Tab::Variables;
        let selection = match variable_view {
            true => &self.variable_selected,
            false => &self.selected,
        };
        let picked = |set: &BTreeSet<usize>| set.iter().copied().collect();
        let target = match (hit, variable_view, selection) {
            (grid::Hit::Header(_), false, Some(s)) => {
                Some(grid_menu::Target::Variables(picked(&s.columns)))
            }
            (grid::Hit::Number(_), false, Some(s)) => {
                Some(grid_menu::Target::Cases(picked(&s.rows)))
            }
            (grid::Hit::Cell(case, column), false, _) => {
                Some(grid_menu::Target::Cell { case, column })
            }
            (grid::Hit::Number(_), true, Some(s)) => {
                Some(grid_menu::Target::Variables(picked(&s.rows)))
            }
            (grid::Hit::Cell(variable, field), true, _) => {
                Some(grid_menu::Target::Field { variable, field })
            }
            _ => None,
        };
        self.menu = target.map(|t| (t, variable_view));
    }

    /// The right-click menu of a grid: its target and the split, filter and weight in force
    /// at the top, then the items.
    fn grid_menu(&mut self, ui: &mut Ui) {
        let (Some((target, variable_view)), Some(data)) =
            (self.menu.clone(), self.session.dataset())
        else {
            return ui.close();
        };
        let entries = grid_menu::entries(data, &target, self.busy.is_some());
        if entries.is_empty() {
            return ui.close();
        }
        ui.weak(grid_menu::heading(data, &target));
        ui.separator();
        let mut chosen = None;
        for e in &entries {
            match e {
                None => {
                    ui.separator();
                }
                Some(e) if menu_entry(ui, e) => chosen = Some(e.item),
                Some(_) => {}
            }
        }
        let Some(item) = chosen else {
            return;
        };
        ui.close();
        let action = grid_menu::action(data, &target, item);
        // the new variable takes the place of the one it went before
        if item == grid_menu::Item::InsertBefore
            && let grid_menu::Target::Variables(columns) = &target
            && let Some(&column) = columns.first()
        {
            self.select_variable(column, variable_view);
        }
        match action {
            Ok(action) => self.act(action, ui.ctx()),
            Err(message) => self.status = Err(message),
        }
    }

    /// Selects one variable, as a row of the Variable View or a column of the Data View, and
    /// scrolls it into view.
    fn select_variable(&mut self, index: usize, variable_view: bool) {
        if variable_view {
            let mut s = grid::Selection::cell((index, 0));
            s.click_row(index, false, false);
            self.variable_selected = Some(s);
            self.variable_follow = true;
        } else {
            let row = self.selected.as_ref().map_or(0, |s| s.cursor.0);
            let mut s = grid::Selection::cell((row, index));
            s.click_column(index, false, false);
            self.selected = Some(s);
            self.follow = true;
        }
    }

    /// Does what a menu item asks for.
    fn act(&mut self, action: grid_menu::Action, ctx: &egui::Context) {
        match action {
            grid_menu::Action::Run(syntax) => self.run(&syntax, None, After::Show),
            grid_menu::Action::Ask(confirm) => self.confirm = Some(confirm),
            grid_menu::Action::Open(dialog) => self.open_dialog(*dialog),
            grid_menu::Action::Copy => self.copy_selection(ctx),
            grid_menu::Action::EditCell(case, column) => self.edit_cell((case, column)),
            grid_menu::Action::EditField(row, field) => {
                self.tab = Tab::Variables;
                self.variable_follow = true;
                self.edit_field((row, field));
            }
        }
    }

    fn variable_view(&mut self, ui: &mut Ui) {
        if self.session.dataset().is_none() {
            return Self::no_data(ui);
        }
        if self.find.is_some() {
            ui.horizontal(|ui| self.find_bar(ui));
        }
        paper(ui, !self.settings.paper_follows_system);
        let data = self.session.dataset().expect("checked above");
        let headers: Vec<String> = Field::ALL.iter().map(|f| f.title().to_string()).collect();
        let mut widths = vec![
            // all twelve fit a window 1280 points wide
            130.0, 76.0, 80.0, 56.0, 72.0, 230.0, 230.0, 120.0, 68.0, 72.0, 84.0, 84.0,
        ];
        if self.settings.variable_widths.len() == widths.len() {
            widths.clone_from(&self.settings.variable_widths);
        }
        let cell = |r: usize, c: usize| {
            let field = Field::ALL[c];
            let text = edits::field_text(&data.variable_at(r), field);
            let right = matches!(field, Field::Width | Field::Decimals | Field::Columns);
            (Some(text), if right { Align::Max } else { Align::Min })
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
            self.variable_selected.as_ref(),
            std::mem::take(&mut self.variable_follow),
            &cell,
            &|_| false,
            edit.as_ref().map(|e| (e.row, e.column)),
            &mut |ui, rect| {
                let Some(e) = edit.as_mut() else { return };
                let Some(choices) = Field::ALL[e.column].choices() else {
                    (commit, cancel) = cell_editor(ui, rect, e);
                    return;
                };
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ComboBox::new("choice_edit", "")
                        .selected_text(e.text.clone())
                        .width(rect.width() - 8.0)
                        .show_ui(ui, |ui| {
                            for &level in choices {
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
        let modifiers = ui.input(|i| i.modifiers);
        if events.clicked.is_some() {
            self.rows_before_click = self
                .variable_selected
                .as_ref()
                .map(|s| s.rows.clone())
                .unwrap_or_default();
        }
        if select(&mut self.variable_selected, &events, modifiers) {
            self.variable_follow = true;
        }
        self.open_menu(events.secondary);
        if let Some(response) = &events.response {
            response.context_menu(|ui| self.grid_menu(ui));
        }
        if let Some(at) = events.clicked {
            // a text edit commits when it loses focus; a list just closes
            let list = |e: &CellEdit| Field::ALL[e.column].choices().is_some();
            if self
                .field_edit
                .as_ref()
                .is_some_and(|e| list(e) && (e.row, e.column) != at)
            {
                self.field_edit = None;
            }
        }
        if let Some(at) = events.double_clicked {
            self.edit_field(at);
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

    /// Starts editing a cell of the Variable View.
    fn edit_field(&mut self, (row, column): (usize, usize)) {
        let Some(data) = self.session.dataset() else {
            return;
        };
        let variable = data.variable_at(row);
        let field = Field::ALL[column];
        // the other variables of the same type whose rows are selected can get the same labels
        let rows = match &self.variable_selected {
            Some(s) if s.rows.contains(&row) => &s.rows,
            _ => &self.rows_before_click,
        };
        let others: Vec<String> = match rows.contains(&row) {
            true => rows
                .iter()
                .filter(|&&r| r != row && r < data.variable_count())
                .map(|&r| data.variable_at(r))
                .filter(|v| v.is_string() == variable.is_string())
                .map(|v| v.name().to_string())
                .collect(),
            false => Vec::new(),
        };
        match field {
            Field::Type | Field::Format => {
                self.format_dialog = Some(edits::FormatChoice::of(&variable));
                return;
            }
            Field::Values => {
                self.labels_dialog = Some(edits::LabelsEditor::of(&variable, others));
                return;
            }
            Field::Missing => {
                self.missing_dialog = Some(edits::MissingEditor::of(&variable, others));
                return;
            }
            _ => {}
        }
        self.variable_selected = Some(grid::Selection::cell((row, column)));
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
                .on_hover_text("Ctrl+Enter")
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
        let wrap = !self.settings.wrap_off;
        let macros = self.macros.join(" ");
        let theme = themes::by_name(&self.settings.theme);
        let theme_index = themes::THEMES
            .iter()
            .position(|t| t.name == theme.name)
            .unwrap_or(0);
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job =
                highlight::cached_job(ui.ctx(), text.as_str(), &macros, theme_index, 14.0);
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let gutter = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0')) * 5.0 + 4.0;
        let area = if wrap {
            ScrollArea::vertical()
        } else {
            ScrollArea::both()
        };
        let mut popup_at = None;
        let mut hovered_command = None;
        egui::CentralPanel::default().show(ui, |ui| {
            editor_paper(ui, theme);
            // the text fills the tab however tall the window is
            let height = ui.available_height();
            area.auto_shrink(false).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let left = ui.cursor().left();
                    ui.add_space(gutter);
                    let output = TextEdit::multiline(&mut self.syntax)
                        .id_salt("syntax_editor")
                        .code_editor()
                        // no frame: the paper behind it sets the text apart
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 4)))
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .min_size(vec2(0.0, height))
                        .margin(vec2(6.0, 4.0))
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
        let (list, range) =
            complete::candidates(&self.syntax, cursor, &variables, &self.macros, explicit);
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
        // opening a file needs no data
        let empty;
        let data = match self.session.dataset() {
            Some(data) => data,
            None if matches!(dialog, Dialog::ImportData(_)) => {
                empty = Dataset::new(0);
                &empty
            }
            None => {
                self.dialog = None;
                return;
            }
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
                // the lists and a long dialog scroll, so the syntax and OK stay in reach on a
                // small screen or at a large scale: the title, syntax and buttons take the rest
                let lines = syntax.as_ref().map_or(1, |text| text.lines().count()) as f32;
                let row = ui.text_style_height(&TextStyle::Monospace);
                let height = (ctx.content_rect().height() - 150.0 - lines * row).max(160.0);
                ui.horizontal_top(|ui| {
                    // Sort Variables and Open Data take no variables
                    if !matches!(dialog, Dialog::SortVariables(_) | Dialog::ImportData(_)) {
                        ui.vertical(|ui| {
                            ui.set_width(230.0);
                            let sets = matches!(dialog, Dialog::CustomTables(..));
                            let list = (height - 90.0).clamp(80.0, 320.0);
                            let (filter, picked) = (&mut self.filter, &mut self.picked);
                            variable_picker(ui, data, sets, filter, picked, list);
                        });
                        ui.add_space(12.0);
                    }
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
                action = dialog_footer(ui, &syntax, true);
            });
        if !open {
            action = Some(DialogAction::Cancel);
        }
        if let Some(Dialog::MultResponse(d)) = &self.dialog {
            self.groups.clone_from(&d.groups);
        }
        match (action, syntax) {
            (Some(DialogAction::Ok), Ok(text)) => {
                let title = self.dialog.as_ref().map_or("dialog", Dialog::title);
                self.run(&text, None, After::CloseDialog(title));
            }
            (Some(DialogAction::Paste), Ok(text)) => {
                self.paste(&text);
                self.dialog = None;
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
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.updates.poll();
        self.close_request(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // removes the endpoint file, so the command line says at once that no window runs
        self.server = None;
        let _ = self.settings.save();
        self.updates.restart();
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let title = match self.data_file.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("{} - Statscraft", name.to_string_lossy()),
            None => "Statscraft".to_string(),
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
        // files Finder opened with the app take the same way as files dropped on the window
        #[cfg(target_os = "macos")]
        let files = [files, crate::open_files::take()].concat();
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
                "Drop data, workbook, text or .sps files to open them",
                FontId::proportional(20.0),
                Color32::WHITE,
            );
        }
        self.find_keys(ui);
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
        if self.show_log && shows_log(self.tab) {
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
                    Some(output_view::Request::Paste(syntax)) => self.paste(&syntax),
                    Some(output_view::Request::Run(syntax)) => {
                        self.run(&syntax, None, After::Show);
                    }
                    None => {}
                }
            }
        });
        let ctx = ui.ctx().clone();
        self.dialog_window(&ctx);
        self.confirm_window(&ctx);
        self.unsaved_window(&ctx);
        let log_hidden = self.settings.log_hidden;
        let labels = settings_window::TableLabels {
            variables: self.session.setting("TVARS"),
            values: self.session.setting("TNUMBERS"),
        };
        let set = settings_window::show(
            &ctx,
            &mut self.settings_section,
            &mut self.settings,
            &labels,
        );
        if let Some(syntax) = set {
            self.run(&syntax, None, After::Show);
        }
        if self.settings.log_hidden != log_hidden {
            self.show_log = !self.settings.log_hidden;
        }
        self.format_window(&ctx);
        self.labels_window(&ctx);
        self.missing_window(&ctx);
        egui::Window::new("About Statscraft")
            .open(&mut self.about)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(egui::vec2(48.0, 48.0)));
                    ui.heading("Statscraft");
                });
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Statistics with a command language, on .sav data files.");
                ui.label("License: MIT");
                ui.hyperlink_to("Source code", REPOSITORY);
                ui.hyperlink_to("Documentation", DOCS);
                ui.collapsing("Color themes", |ui| {
                    ui.add(egui::Label::new(RichText::new(themes::NOTICES).small()).wrap());
                });
                ui.collapsing("Fonts", |ui| {
                    ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(crate::fonts::NOTICES).small()).wrap(),
                        );
                    });
                });
            });
    }
}

/// The app icon as a texture, at 128 px so it stays sharp on a high-density screen.
fn logo(ctx: &egui::Context) -> egui::TextureHandle {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icons/statscraft-128.png"))
        .expect("the bundled icon is a valid PNG");
    let size = [icon.width as usize, icon.height as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
    ctx.load_texture("logo", image, egui::TextureOptions::LINEAR)
}

/// The rest of `ui` as a sheet of paper: light with dark text when `light`, whatever the system
/// theme, as a spreadsheet or a document page stays light in dark mode.
pub(crate) fn paper(ui: &mut Ui, light: bool) {
    if light {
        ui.style_mut().visuals = egui::Visuals::light();
        square(&mut ui.style_mut().visuals);
    }
    let sheet = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(sheet, 0.0, ui.visuals().extreme_bg_color);
}

/// The rest of `ui` as the syntax editor's sheet, in the colours of its theme.
fn editor_paper(ui: &mut Ui, theme: &themes::Theme) {
    let visuals = &mut ui.style_mut().visuals;
    *visuals = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    square(visuals);
    visuals.extreme_bg_color = theme.background;
    visuals.text_cursor.stroke.color = theme.text;
    let sheet = ui.available_rect_before_wrap();
    ui.painter().rect_filled(sheet, 0.0, theme.background);
}

/// Square corners and no shadows for every control, menu and window, to match the flat square
/// grids, and the brand's accent colour.
pub(crate) fn square(visuals: &mut egui::Visuals) {
    crate::brand::accent(visuals);
    let none = egui::CornerRadius::ZERO;
    let widgets = &mut visuals.widgets;
    for state in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = none;
    }
    visuals.menu_corner_radius = none;
    visuals.window_corner_radius = none;
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
}

/// The summary and syntax of a command, in the completion list and over a command name.
fn command_card(ui: &mut Ui, spec: &statscraft_syntax::CommandSpec) {
    ui.set_max_width(560.0);
    ui.strong(spec.name);
    ui.label(spec.summary);
    ui.add(egui::Label::new(RichText::new(spec.syntax).monospace().size(12.0)).wrap());
}

/// What the status bar says about the warnings of the last run.
/// The log panel shows under the syntax editor and the output; the grids get the whole height.
/// The data and the syntax that differ from their files: the data when it is not what the last
/// open or save left, the syntax when its text is not the file's (new syntax when not empty).
fn unsaved_items(
    data: Option<&Dataset>,
    data_saved: Option<&Dataset>,
    data_file: Option<&Path>,
    syntax: &str,
    syntax_saved: &str,
    syntax_file: Option<&Path>,
) -> Vec<String> {
    let name = |file: Option<&Path>, none: &str| {
        file.and_then(Path::file_name)
            .map_or_else(|| none.to_string(), |n| n.to_string_lossy().into_owned())
    };
    let mut items = Vec::new();
    if data.is_some_and(|d| Some(d) != data_saved) {
        items.push(format!(
            "Data: {}",
            name(data_file, "not saved to a file yet")
        ));
    }
    if syntax != syntax_saved {
        items.push(format!("Syntax: {}", name(syntax_file, "new syntax")));
    }
    items
}

/// A run whose last command is GET, GET DATA or SAVE leaves the data as that file holds it.
fn ends_on_file(commands: &[CommandResult]) -> bool {
    commands
        .last()
        .is_some_and(|c| matches!(c.name.as_str(), "GET" | "GET DATA" | "SAVE"))
}

/// Answers a close request: with unsaved work it cancels the close and returns what is unsaved.
fn hold_close(ctx: &egui::Context, unsaved: impl FnOnce() -> Vec<String>) -> Option<Vec<String>> {
    if !ctx.input(|i| i.viewport().close_requested()) {
        return None;
    }
    let items = unsaved();
    if items.is_empty() {
        return None;
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    Some(items)
}

fn shows_log(tab: Tab) -> bool {
    matches!(tab, Tab::Syntax | Tab::Output)
}

fn warning_note(warnings: usize) -> Option<String> {
    match warnings {
        0 => None,
        1 => Some("1 warning, see the log".into()),
        n => Some(format!("{n} warnings, see the log")),
    }
}

/// Where the Data View draws a variable's values and value labels: its alignment, else
/// numbers to the right and strings to the left.
fn cell_align(variable: &statscraft_data::Variable) -> Align {
    match variable.alignment() {
        Some("left") => Align::Min,
        Some("center") => Align::Center,
        Some(_) => Align::Max,
        None if variable.is_string() => Align::Min,
        None => Align::Max,
    }
}

/// The cases the FILTER in force leaves out, whose row numbers the Data View strikes through.
fn filtered_out(data: &Dataset) -> impl Fn(usize) -> bool + '_ {
    let filter = data
        .filter_variable()
        .and_then(|name| data.variable(name).ok());
    move |case| filter.as_ref().is_some_and(|f| !f.keeps(case))
}

/// The split, filter and weight in force, as the status bar names them, each with the dialog
/// that changes it.
fn data_states(data: &Dataset) -> Vec<(String, Dialog)> {
    let mut states = Vec::new();
    if let Some(split) = data.split_file() {
        let how = if split.layered { "layered" } else { "separate" };
        states.push((
            format!("Split by {} ({how})", split.variables.join(" ")),
            Dialog::SplitFile(SplitFile::current(Some(split))),
        ));
    }
    if let Some(name) = data.filter_variable() {
        states.push((
            format!("Filter: {name}"),
            Dialog::SelectCases(Default::default()),
        ));
    }
    if let Some(name) = data.weight_variable() {
        states.push((
            format!("Weight: {name}"),
            Dialog::WeightCases(WeightCases::current(Some(name))),
        ));
    }
    states
}

/// The macros defined by the commands of the session that ran, each name once.
fn session_macros(log: &[statscraft_engine::session::LogEntry]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for entry in log.iter().filter(|e| e.result.is_ok() && !e.undone) {
        for name in highlight::macro_names(&entry.syntax) {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
    }
    names
}

/// Whether the dataset has a string variable of that name.
/// Labelling the categories of a set by the counted value needs a label for that value in
/// every variable, a different one each (ignoring case), as MRSETS checks when it runs.
fn counted_labels(data: &Dataset, variables: &[String], counted: &str) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for name in variables {
        let label = data.variable(name).ok().and_then(|v| {
            v.value_labels().iter().find_map(|(value, label)| {
                let hit = match value {
                    Value::Number(n) => counted.trim().parse::<f64>() == Ok(*n),
                    Value::String(s) => s.trim_end() == counted.trim_end(),
                };
                hit.then(|| label.to_lowercase())
            })
        });
        match label {
            None => return Err(format!("{name} has no value label for the counted value")),
            Some(label) if seen.contains(&label) => {
                return Err(format!(
                    "{name} has the same label for the counted value as another variable"
                ));
            }
            Some(label) => seen.push(label),
        }
    }
    Ok(())
}

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

/// The body of Open Data: the sheet, names row and range of a workbook, or the delimiter and
/// names row of a text file.
fn import_body(ui: &mut Ui, d: &mut ImportData) {
    ui.label(&d.path);
    if let Some(problem) = &d.problem {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
        return;
    }
    ui.add_space(6.0);
    if is_workbook(Path::new(&d.path)) {
        ui.horizontal(|ui| {
            ui.strong("Sheet");
            let shown = d.sheets.get(d.sheet).cloned().unwrap_or_default();
            ComboBox::new("import sheet", "")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (i, name) in d.sheets.iter().enumerate() {
                        ui.selectable_value(&mut d.sheet, i, name);
                    }
                });
        });
        ui.checkbox(&mut d.names, "Read variable names from the first row");
        ui.horizontal(|ui| {
            ui.strong("Range");
            ui.add(
                TextEdit::singleline(&mut d.range)
                    .hint_text("whole sheet")
                    .desired_width(120.0),
            );
        });
    } else {
        ui.horizontal(|ui| {
            ui.strong("Delimiter");
            for (c, label) in TEXT_DELIMITERS {
                ui.radio_value(&mut d.delimiter, c, label);
            }
        });
        ui.checkbox(&mut d.names, "Read variable names from the first line");
        ui.weak("First line");
        ui.label(RichText::new(&d.first_line).monospace());
    }
}

/// The files the window opens, for the note on files it leaves alone.
const OPENS: &str =
    "Statscraft opens .sav, .zsav, .xlsx, .xlsm, .xls, .csv, .txt, .tab, .dat and .sps files";

/// A workbook or text file, which opens through the Open Data dialog.
fn is_import_file(path: &Path) -> bool {
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    is_workbook(path) || matches!(extension.as_deref(), Some("csv" | "txt" | "tab" | "dat"))
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
        let slot = if is_data_file(&path) || is_import_file(&path) {
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
        reasons.push(OPENS);
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

/// The foot of every dialog that writes syntax: the syntax, or why it has none yet, then OK,
/// Paste, Reset when `reset`, and Cancel. Escape cancels, unless it closes a list.
fn dialog_footer(
    ui: &mut Ui,
    syntax: &Result<String, String>,
    reset: bool,
) -> Option<DialogAction> {
    // a list drawn open in this frame, above, takes the Escape
    let escape = ui.input(|i| i.key_pressed(Key::Escape)) && !ui.ctx().any_popup_open();
    ui.separator();
    ui.weak("Syntax");
    match syntax {
        Ok(text) => {
            ui.add(egui::Label::new(RichText::new(text).monospace()).wrap());
        }
        Err(message) => {
            ui.colored_label(ui.visuals().warn_fg_color, message);
        }
    }
    ui.add_space(6.0);
    let mut action = escape.then_some(DialogAction::Cancel);
    ui.horizontal(|ui| {
        let ready = syntax.is_ok();
        if ui
            .add_enabled(ready, crate::brand::primary(ui, "OK"))
            .clicked()
        {
            action = Some(DialogAction::Ok);
        }
        if ui.add_enabled(ready, egui::Button::new("Paste")).clicked() {
            action = Some(DialogAction::Paste);
        }
        if reset && ui.button("Reset").clicked() {
            action = Some(DialogAction::Reset);
        }
        if ui.button("Cancel").clicked() {
            action = Some(DialogAction::Cancel);
        }
    });
    action
}

/// A click in a grid changes its selection: a cell (with Shift the block to it), a row number
/// or a column header (with Shift a range, with Ctrl one more or one less). A drag selects from
/// where it began to where it is. True when a drag across cells moved the cursor, which then
/// scrolls into view.
fn select(
    selection: &mut Option<grid::Selection>,
    events: &GridEvents,
    modifiers: Modifiers,
) -> bool {
    let (shift, ctrl) = (modifiers.shift, modifiers.command);
    if let Some(at) = events.clicked {
        match selection {
            Some(s) if shift => s.extend_to(at),
            _ => *selection = Some(grid::Selection::cell(at)),
        }
    }
    if let Some(r) = events.row_clicked {
        selection
            .get_or_insert_with(Default::default)
            .click_row(r, shift, ctrl);
    }
    if let Some(c) = events.column_clicked {
        selection
            .get_or_insert_with(Default::default)
            .click_column(c, shift, ctrl);
    }
    if let (Some(s), Some((from, to))) = (selection.as_mut(), events.dragged) {
        s.drag(from, to);
    }
    // a right click keeps a selection of whole rows or columns that holds it, for a menu over
    // all of them; a cell becomes the selected cell
    match events.secondary {
        Some(grid::Hit::Header(c)) => selection
            .get_or_insert_with(Default::default)
            .right_click_column(c),
        Some(grid::Hit::Number(r)) => selection
            .get_or_insert_with(Default::default)
            .right_click_row(r),
        Some(grid::Hit::Cell(r, c)) => *selection = Some(grid::Selection::cell((r, c))),
        _ => {}
    }
    matches!(
        events.dragged,
        Some((grid::Hit::Cell(..), grid::Hit::Cell(..)))
    )
}

/// One item of a grid's right-click menu, off with the reason on hover when it cannot be used;
/// true when it was clicked.
fn menu_entry(ui: &mut Ui, e: &grid_menu::Entry) -> bool {
    let button = egui::Button::new(e.label.as_str());
    let button = match e.shortcut {
        Some(key) => button.shortcut_text(key),
        None => button,
    };
    let response = ui.add_enabled(e.enabled, button);
    match &e.hover {
        Some(text) => response
            .on_hover_text(text)
            .on_disabled_hover_text(text)
            .clicked(),
        None => response.clicked(),
    }
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
/// in view, and reports clicks and drags by (row, column). The number of a row that `struck`
/// names gets a diagonal stroke, as a case the filter leaves out. A cell without text (None)
/// is system-missing and shows a small grey dot, where an empty string shows nothing.
#[allow(clippy::too_many_arguments)]
fn paint_grid(
    ui: &mut Ui,
    id: &str,
    headers: &[String],
    widths: &[f32],
    rows: usize,
    selected: Option<&grid::Selection>,
    follow: bool,
    cell: &dyn Fn(usize, usize) -> (Option<String>, Align),
    struck: &dyn Fn(usize) -> bool,
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
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
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
            // a text too wide for its cell shows its start and ends in an ellipsis, so a number
            // cut short never reads as a different number, and it keeps the alignment of its
            // column: a right-aligned column stays flush at the right
            let text_in = |painter: &egui::Painter,
                           rect: Rect,
                           text: String,
                           align: Align,
                           color: Color32| {
                let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(clip));
                let format = egui::TextFormat::simple(font.clone(), color);
                let mut job = egui::text::LayoutJob::single_section(text, format);
                job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 12.0);
                let galley = painter.layout_job(job);
                let width = galley.size().x;
                let x = match align {
                    Align::Max => rect.right() - 6.0 - width,
                    Align::Center => rect.center().x - width / 2.0,
                    Align::Min => rect.left() + 6.0,
                };
                let y = rect.center().y - galley.size().y / 2.0;
                painter.galley(pos2(x, y), galley, color);
            };

            let (left, right) = (x(visible_columns.start), x(visible_columns.end));
            // a selection of more than one cell is shaded, the cursor framed
            let wide = selected.filter(|s| s.is_wide());
            let marked = visuals.selection.bg_fill.gamma_multiply(0.3);
            for r in visible_rows.clone() {
                if r % 2 == 1 {
                    let band = Rect::from_min_max(pos2(left, y(r)), pos2(right, y(r) + row_height));
                    painter.rect_filled(band, 0.0, visuals.faint_bg_color);
                }
                for c in visible_columns.clone() {
                    if wide.is_some_and(|s| s.contains(r, c)) {
                        painter.rect_filled(cell_rect(r, c), 0.0, marked);
                    }
                    let rect = cell_rect(r, c);
                    match cell(r, c) {
                        (Some(text), _) if text.is_empty() => {}
                        (Some(text), align) => {
                            text_in(&painter, rect, text, align, visuals.text_color())
                        }
                        (None, align) => {
                            let x = match align {
                                Align::Max => rect.right() - 8.0,
                                Align::Center => rect.center().x,
                                Align::Min => rect.left() + 8.0,
                            };
                            let dot = pos2(x, rect.center().y);
                            painter.circle_filled(dot, 1.5, visuals.weak_text_color());
                        }
                    }
                }
                painter.hline(left..=right, y(r) + row_height, line);
            }
            let bottom = y(rows).min(clip.bottom());
            for c in visible_columns.clone() {
                painter.vline(x(c + 1), clip.top()..=bottom, line);
            }
            if let Some((r, c)) = selected.map(|s| s.cursor)
                && visible_rows.contains(&r)
                && visible_columns.contains(&c)
            {
                let stroke = Stroke::new(2.0, visuals.selection.stroke.color);
                painter.rect_stroke(cell_rect(r, c), 0.0, stroke, StrokeKind::Inside);
            }
            events.page = visible_rows.len().saturating_sub(1);
            if follow && let Some((r, c)) = selected.map(|s| s.cursor) {
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
                if selected.is_some_and(|s| s.columns.contains(&c)) {
                    painter.rect_filled(rect, 0.0, visuals.selection.bg_fill);
                }
                text_in(
                    &painter,
                    rect,
                    headers[c].clone(),
                    Align::Min,
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
                    if selected.is_some_and(|s| s.rows.contains(&r)) {
                        painter.rect_filled(rect, 0.0, visuals.selection.bg_fill);
                    }
                    text_in(
                        &painter,
                        rect,
                        (r + 1).to_string(),
                        Align::Max,
                        visuals.weak_text_color(),
                    );
                    if struck(r) {
                        let shrunk = rect.shrink2(vec2(6.0, 3.0));
                        let stroke = Stroke::new(1.5, visuals.text_color());
                        painter.line_segment([shrunk.left_bottom(), shrunk.right_top()], stroke);
                    }
                }
            }
            painter.vline(side + number_width, clip.top()..=bottom, line);
            painter.rect_filled(
                Rect::from_min_size(pos2(side, top), vec2(number_width, header_height)),
                0.0,
                header_fill,
            );

            let sizes = (number_width, header_height, row_height);
            let at = |pos: egui::Pos2| grid::hit(pos - origin, viewport.min, sizes, &offsets, rows);
            let hit = response.interact_pointer_pos().and_then(at);
            let left = egui::PointerButton::Primary;
            if response.dragged_by(left) {
                let from = ui.input(|i| i.pointer.press_origin()).and_then(at);
                // the press that began the drag selects as a click would
                if response.drag_started_by(left) {
                    match from {
                        Some(grid::Hit::Header(c)) => events.column_clicked = Some(c),
                        Some(grid::Hit::Number(r)) => events.row_clicked = Some(r),
                        Some(grid::Hit::Cell(r, c)) => events.clicked = Some((r, c)),
                        _ => {}
                    }
                }
                events.dragged = from.zip(hit);
            }
            match hit {
                Some(grid::Hit::Header(c)) if response.clicked() => events.column_clicked = Some(c),
                Some(grid::Hit::Number(r)) if response.clicked() => events.row_clicked = Some(r),
                Some(grid::Hit::Cell(r, c)) if response.double_clicked() => {
                    events.double_clicked = Some((r, c))
                }
                Some(grid::Hit::Cell(r, c)) if response.clicked() => events.clicked = Some((r, c)),
                _ => {}
            }
            if response.secondary_clicked() {
                events.secondary = hit;
            }
            events.response = Some(response);
        });
    events
}

/// The dataset's variables, filtered by name or label, and with `sets` its multiple response
/// sets; a click picks or unpicks one. The list is at most `height` high.
fn variable_picker(
    ui: &mut Ui,
    data: &Dataset,
    sets: bool,
    filter: &mut String,
    picked: &mut Vec<String>,
    height: f32,
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
        .max_height(height)
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
}

/// A list of variables in a dialog, with Add (the picked variables) and a remove button each.
pub(crate) fn target_list(
    ui: &mut Ui,
    title: &str,
    list: &mut Vec<String>,
    picked: &mut Vec<String>,
) {
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
pub(crate) fn target_one(ui: &mut Ui, title: &str, slot: &mut String, picked: &mut Vec<String>) {
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
pub(crate) fn field(ui: &mut Ui, label: &str, text: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(TextEdit::singleline(text).desired_width(width));
    });
}

/// A list to choose one of `options` by its label, with a label in front, on one line.
pub(crate) fn choice(ui: &mut Ui, label: &str, options: &[(&str, &str)], index: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(label);
        ComboBox::new(label, "")
            .selected_text(options[*index].1)
            .show_index(ui, index, options.len(), |i| options[i].1);
    });
}

pub(crate) fn checkboxes(ui: &mut Ui, options: &[(&str, &str)], flags: &mut [bool]) {
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
        Dialog::Frequencies(d) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.add_space(6.0);
            ui.checkbox(&mut d.tables, "Display frequency tables");
            ui.add_enabled_ui(d.tables, |ui| {
                choice(ui, "Order rows by", &FREQUENCIES_ORDERS, &mut d.order);
                field(
                    ui,
                    "Leave out tables with more values than",
                    &mut d.limit,
                    50.0,
                );
            });
            ui.add_space(6.0);
            ui.strong("Statistics");
            checkboxes(ui, &FREQUENCIES_STATISTICS, &mut d.statistics);
            ui.add_space(6.0);
            ui.strong("Percentile values");
            ui.checkbox(&mut d.quartiles, "Quartiles");
            field(ui, "Cut points for equal groups:", &mut d.ntiles, 50.0);
            field(ui, "Percentiles:", &mut d.percentiles, 180.0);
        }
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
                ui.checkbox(&mut d.effective_count, "Effective count");
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
            if !(d.rows.is_empty() && d.columns.is_empty() && d.layers.is_empty()) {
                egui::CollapsingHeader::new("Categories, totals and subtotals")
                    .show(ui, |ui| categories_panel(ui, data, d, draft));
            }
            ui.strong("Significance tests");
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
                ui.add(TextEdit::singleline(&mut d.expression).desired_width(320.0));
                ui.end_row();
                ui.label("Only where (optional)");
                ui.add(TextEdit::singleline(&mut d.condition).desired_width(320.0));
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
                model_dialogs_view::post_hoc_tests(
                    ui,
                    &mut d.post_hoc,
                    &mut d.dunnett,
                    &mut d.dunnett_first,
                    &mut d.dunnett_side,
                );
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
                ui.radio_value(&mut d.dichotomy, true, "Dichotomies");
                ui.radio_value(&mut d.dichotomy, false, "Categories");
            });
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(TextEdit::singleline(&mut d.name).desired_width(160.0));
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
                ui.weak(format!("Sets now: {}", names.join(", ")));
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
                    if ui.small_button(order).clicked() {
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
        }
        Dialog::SplitFile(d) => {
            ui.radio_value(&mut d.mode, SplitMode::Off, "Analyze all cases together");
            ui.radio_value(
                &mut d.mode,
                SplitMode::Layered,
                "Compare groups in the same tables",
            );
            ui.radio_value(
                &mut d.mode,
                SplitMode::Separate,
                "Tables of their own for each group",
            );
            if d.mode != SplitMode::Off {
                target_list(ui, "Groups based on", &mut d.variables, picked);
                ui.checkbox(&mut d.sort, "Sort the cases by these variables first");
            }
            ui.weak(match data.split_file() {
                Some(split) => format!("Now split by {}.", split.variables.join(" ")),
                None => "Now not split.".to_string(),
            });
        }
        Dialog::SelectCases(d) => {
            ui.radio_value(&mut d.mode, SelectMode::All, "All cases");
            ui.radio_value(
                &mut d.mode,
                SelectMode::Filter,
                format!("Filter: leave the other cases out of analyses ({FILTER_VARIABLE})"),
            );
            ui.radio_value(&mut d.mode, SelectMode::Delete, "Delete the other cases");
            if d.mode != SelectMode::All {
                ui.add_space(6.0);
                ui.strong("Keep the cases where");
                ui.add(
                    TextEdit::multiline(&mut d.condition)
                        .desired_width(360.0)
                        .desired_rows(3),
                );
                if ui.button("Add picked variables to the condition").clicked() {
                    for name in picked.drain(..) {
                        if !d.condition.is_empty() && !d.condition.ends_with(' ') {
                            d.condition.push(' ');
                        }
                        d.condition.push_str(&name);
                    }
                }
            }
            if d.mode == SelectMode::Delete {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "The other cases are deleted from the dataset.",
                );
            }
            ui.weak(match data.filter_variable() {
                Some(name) => format!("Now filtered by {name}."),
                None => "Now no filter.".to_string(),
            });
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
        Dialog::ImportData(d) => import_body(ui, d),
        Dialog::ExportData(d) => {
            ui.horizontal(|ui| {
                ui.strong("File");
                if ui.button("Browse...").clicked()
                    && let Some(path) = file_dialog(folder)
                        .add_filter("Excel workbook", &["xlsx"])
                        .add_filter("CSV", &["csv"])
                        .add_filter("Tab-delimited text", &["txt"])
                        .save_file()
                {
                    *folder = path.parent().map(Path::to_path_buf);
                    d.path = path.to_string_lossy().into_owned();
                }
            });
            ui.add(TextEdit::singleline(&mut d.path).desired_width(360.0));
            ui.add_space(6.0);
            ui.strong("First row");
            ui.horizontal(|ui| {
                for (i, label) in EXPORT_NAMES.iter().enumerate() {
                    ui.radio_value(&mut d.names, i, *label);
                }
            });
            ui.checkbox(&mut d.labels, "Value labels instead of values");
            ui.checkbox(&mut d.recode_missing, "User-missing values as empty");
            ui.add_space(6.0);
            target_list(ui, "Variables (none: all)", &mut d.variables, picked);
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
            }
        }
        Dialog::MultResponse(d) => {
            ui.horizontal(|ui| {
                ui.strong("Groups");
                if ui
                    .add_enabled(!picked.is_empty(), egui::Button::new("Add").small())
                    .clicked()
                {
                    d.groups.push(MrGroup {
                        variables: std::mem::take(picked),
                        ..Default::default()
                    });
                }
            });
            if d.groups.is_empty() {
                ui.weak("empty");
            }
            let mut remove = None;
            for (i, g) in d.groups.iter_mut().enumerate() {
                ui.push_id(("group", i), |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.add(TextEdit::singleline(&mut g.name).desired_width(110.0));
                            ui.label("Label");
                            ui.add(TextEdit::singleline(&mut g.label).desired_width(170.0));
                            if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut g.dichotomy, true, "Dichotomies, counted value");
                            ui.add_enabled(
                                g.dichotomy,
                                TextEdit::singleline(&mut g.value).desired_width(40.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut g.dichotomy, false, "Categories from");
                            if !g.dichotomy {
                                ui.add(TextEdit::singleline(&mut g.value).desired_width(40.0));
                                ui.label("through");
                                ui.add(TextEdit::singleline(&mut g.max).desired_width(40.0));
                            }
                        });
                        target_list(ui, "Variables", &mut g.variables, picked);
                    });
                });
            }
            if let Some(i) = remove {
                d.groups.remove(i);
            }
            if d.crosstabs {
                ui.add_space(6.0);
                let groups: Vec<String> = d
                    .groups
                    .iter()
                    .map(|g| g.name.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .collect();
                for (title, list) in [
                    ("Rows", &mut d.rows),
                    ("Columns", &mut d.columns),
                    ("Layers", &mut d.layers),
                ] {
                    target_list(ui, title, list, picked);
                    if !groups.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Add group");
                            for name in &groups {
                                if ui.small_button(name).clicked() && !list.contains(name) {
                                    list.push(name.clone());
                                }
                            }
                        });
                    }
                }
                let variables: Vec<String> = d.table_variables().into_iter().cloned().collect();
                if !variables.is_empty() {
                    ui.add_space(6.0);
                    ui.strong("Ranges of the variables");
                }
                for name in variables {
                    if !d.ranges.iter().any(|(n, ..)| *n == name) {
                        // the lowest and highest labelled values, as a start
                        let codes: Vec<f64> = data
                            .variable(&name)
                            .map(|v| {
                                v.value_labels()
                                    .iter()
                                    .filter_map(|(value, _)| match value {
                                        Value::Number(n) => Some(*n),
                                        Value::String(_) => None,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let bound = |f: fn(f64, f64) -> f64| {
                            codes
                                .iter()
                                .copied()
                                .reduce(f)
                                .map(number)
                                .unwrap_or_default()
                        };
                        d.ranges
                            .push((name.clone(), bound(f64::min), bound(f64::max)));
                    }
                    if let Some((_, min, max)) = d.ranges.iter_mut().find(|(n, ..)| *n == name) {
                        ui.horizontal(|ui| {
                            ui.label(name.as_str());
                            ui.add(TextEdit::singleline(min).desired_width(40.0));
                            ui.label("through");
                            ui.add(TextEdit::singleline(max).desired_width(40.0));
                        });
                    }
                }
                ui.add_space(6.0);
                ui.strong("Cells");
                checkboxes(ui, &MULT_RESPONSE_CELLS, &mut d.cells);
                ui.add_enabled_ui(d.cells.contains(&true), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Percentages based on");
                        ui.radio_value(&mut d.base_responses, false, "cases");
                        ui.radio_value(&mut d.base_responses, true, "responses");
                    });
                });
            }
            ui.add_space(6.0);
            ui.strong("Missing values");
            ui.checkbox(
                &mut d.missing_dichotomies,
                "Leave out a case missing on any variable of a dichotomy group",
            );
            ui.checkbox(
                &mut d.missing_categories,
                "Leave out a case missing on any variable of a category group",
            );
        }
        Dialog::Tree(d) => {
            target_one(ui, "Dependent variable", &mut d.dependent, picked);
            choice(ui, "Level", &TREE_LEVELS[..3], &mut d.dependent_level);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.strong("Predictors");
                if ui
                    .add_enabled(!picked.is_empty(), egui::Button::new("Add").small())
                    .clicked()
                {
                    for name in picked.drain(..) {
                        if !d.predictors.iter().any(|(p, _)| p == &name) {
                            d.predictors.push((name, 0));
                        }
                    }
                }
            });
            if d.predictors.is_empty() {
                ui.weak("empty");
            }
            let mut remove = None;
            egui::Grid::new("predictors").num_columns(3).show(ui, |ui| {
                for (i, (name, level)) in d.predictors.iter_mut().enumerate() {
                    ui.label(name.as_str());
                    ComboBox::from_id_salt(("predictor level", i))
                        .selected_text(TREE_LEVELS[*level].1)
                        .show_index(ui, level, TREE_LEVELS.len(), |i| TREE_LEVELS[i].1);
                    if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                d.predictors.remove(i);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.strong("Method");
                ui.radio_value(&mut d.exhaustive, false, "CHAID");
                ui.radio_value(&mut d.exhaustive, true, "Exhaustive CHAID");
            });
            ui.add_space(6.0);
            ui.strong("Growth limits");
            field(
                ui,
                "Maximum depth, empty for automatic",
                &mut d.max_depth,
                40.0,
            );
            field(ui, "Fewest cases in a parent node", &mut d.min_parent, 50.0);
            field(ui, "Fewest cases in a child node", &mut d.min_child, 50.0);
            ui.add_space(6.0);
            field(ui, "Target categories", &mut d.targets, 160.0);
            ui.add_space(6.0);
            ui.strong("Save");
            checkboxes(ui, &TREE_SAVE, &mut d.save);
        }
        Dialog::Regression(d) => stats_dialogs_view::regression(ui, d, picked),
        Dialog::Reliability(d) => stats_dialogs_view::reliability(ui, d, picked),
        Dialog::NparTests(d) => stats_dialogs_view::npar_tests(ui, d, picked),
        Dialog::PartialCorr(d) => stats_dialogs_view::partial_corr(ui, d, picked),
        Dialog::FactorAnalysis(d) => model_dialogs_view::factor(ui, d, picked),
        Dialog::KMeans(d) => model_dialogs_view::k_means(ui, d, picked),
        Dialog::Hierarchical(d) => model_dialogs_view::hierarchical(ui, d, picked),
        Dialog::Discriminant(d) => model_dialogs_view::discriminant(ui, d, picked),
        Dialog::Logistic(d) => model_dialogs_view::logistic(ui, d, picked),
        Dialog::Univariate(d) => model_dialogs_view::univariate(ui, d, picked),
        Dialog::RepeatedMeasures(d) => model_dialogs_view::repeated_measures(ui, d, picked),
        Dialog::Restructure(d) => {
            ui.radio_value(
                &mut d.kind,
                RestructureKind::VariablesToCases,
                "Variables to cases",
            );
            ui.radio_value(
                &mut d.kind,
                RestructureKind::CasesToVariables,
                "Cases to variables",
            );
            ui.radio_value(
                &mut d.kind,
                RestructureKind::Transpose,
                "Transpose cases and variables",
            );
            ui.add_space(6.0);
            match d.kind {
                RestructureKind::VariablesToCases => {
                    let mut remove = None;
                    for (i, make) in d.makes.iter_mut().enumerate() {
                        ui.push_id(("make", i), |ui| {
                            ui.horizontal(|ui| {
                                ui.label("New variable");
                                ui.add(TextEdit::singleline(&mut make.name).desired_width(120.0));
                                if i > 0
                                    && ui.small_button("\u{d7}").on_hover_text("Remove").clicked()
                                {
                                    remove = Some(i);
                                }
                            });
                            target_list(ui, "Made from", &mut make.from, picked);
                        });
                    }
                    if let Some(i) = remove {
                        d.makes.remove(i);
                    }
                    if ui.button("Another new variable").clicked() {
                        d.makes.push(Default::default());
                    }
                    ui.add_space(6.0);
                    field(ui, "Index variable", &mut d.index, 120.0);
                    field(ui, "Case number variable", &mut d.id, 120.0);
                    target_list(ui, "Copied variables, empty for all", &mut d.keep, picked);
                }
                RestructureKind::CasesToVariables => {
                    target_list(ui, "Identifier variables", &mut d.ids, picked);
                    target_list(ui, "Index variables", &mut d.indexes, picked);
                    ui.checkbox(&mut d.sort, "Sort the cases by these variables first");
                    ui.horizontal(|ui| {
                        ui.label("New variables in order of the");
                        ui.radio_value(&mut d.by_index, false, "variable");
                        ui.radio_value(&mut d.by_index, true, "index");
                    });
                }
                RestructureKind::Transpose => {
                    target_list(ui, "Variables, empty for all", &mut d.variables, picked);
                    target_one(ui, "Name the new variables by", &mut d.names, picked);
                }
            }
        }
        Dialog::SortVariables(d) => {
            choice(ui, "Sort the variables by", &SORT_VARIABLES_KEYS, &mut d.by);
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.descending, false, "Ascending");
                ui.radio_value(&mut d.descending, true, "Descending");
            });
        }
        Dialog::Rank(d) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            target_list(ui, "Rank within groups of", &mut d.by, picked);
            ui.horizontal(|ui| {
                ui.label("Rank 1 goes to the");
                ui.radio_value(&mut d.descending, false, "smallest value");
                ui.radio_value(&mut d.descending, true, "largest value");
            });
            ui.add_space(6.0);
            ui.strong("New variables");
            egui::Grid::new("rank functions")
                .num_columns(2)
                .show(ui, |ui| {
                    for (i, (_, label)) in RANK_FUNCTIONS.iter().enumerate() {
                        ui.checkbox(&mut d.functions[i], *label);
                        ui.add_enabled(
                            d.functions[i],
                            TextEdit::singleline(&mut d.into[i]).desired_width(200.0),
                        );
                        ui.end_row();
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut d.ntiles, "Percentile groups:");
                        ui.add_enabled(
                            d.ntiles,
                            TextEdit::singleline(&mut d.groups).desired_width(32.0),
                        );
                    });
                    ui.add_enabled(
                        d.ntiles,
                        TextEdit::singleline(&mut d.into[7]).desired_width(200.0),
                    );
                    ui.end_row();
                });
            ui.add_space(6.0);
            choice(ui, "Ties get the", &RANK_TIES, &mut d.ties);
            let fraction = d.uses_fraction();
            ui.add_enabled_ui(fraction, |ui| {
                choice(ui, "Proportion formula", &RANK_FRACTIONS, &mut d.fraction);
            });
            ui.checkbox(&mut d.include_missing, "Rank user-missing values too");
            ui.checkbox(&mut d.print, "Table of the new variables");
        }
        Dialog::Autorecode(d) => {
            target_list(ui, "Variables", &mut d.variables, picked);
            field(ui, "New names, one per variable:", &mut d.into, 220.0);
            ui.add_space(6.0);
            ui.checkbox(&mut d.descending, "Number from the largest value");
            ui.checkbox(&mut d.group, "One scheme for all the variables");
            ui.checkbox(&mut d.blank_missing, "Blank strings become a missing value");
            ui.checkbox(&mut d.print, "Table of old and new values");
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
    ui.horizontal(|ui| {
        ui.checkbox(&mut c.total, "Total, labeled");
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
    if !is_set(&c.variable) {
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
mod dialog_walk;

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
        let drop = dropped(paths(&["/d/b.zsav", "/d/c.sav", "/d/notes.pdf"]));
        assert_eq!(drop.data, Some(PathBuf::from("/d/b.zsav")));
        assert_eq!(drop.syntax, None);
        assert_eq!(
            drop.note.unwrap(),
            format!(
                "not opened: c.sav, notes.pdf (one data file and one syntax file open at a time; \
                 {OPENS})"
            )
        );
        // a workbook or text file counts as the data file; it opens the Open Data dialog
        let drop = dropped(paths(&["/d/table.csv", "/d/wave.XLSX"]));
        assert_eq!(drop.data, Some(PathBuf::from("/d/table.csv")));
        assert_eq!(
            drop.note.unwrap(),
            "not opened: wave.XLSX (one data file and one syntax file open at a time)"
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

    #[test]
    fn unsaved_data_and_syntax_are_listed() {
        let mut session = open("survey_basic.sav");
        let saved = session.dataset().cloned();
        let file = Some(Path::new("/d/survey.sav"));
        let data = |session: &Session| {
            unsaved_items(session.dataset(), saved.as_ref(), file, "", "", None)
        };
        assert!(data(&session).is_empty());
        // an analysis changes no data
        run_in(&mut session, "FREQUENCIES q1_satisfaction.");
        assert!(data(&session).is_empty());
        run_in(&mut session, "COMPUTE x = 1.");
        assert_eq!(data(&session), ["Data: survey.sav"]);
        // undo puts back the data as it was saved
        assert!(session.undo());
        assert!(data(&session).is_empty());
        // data never saved, such as an imported workbook
        let never = unsaved_items(session.dataset(), None, None, "", "", None);
        assert_eq!(never, ["Data: not saved to a file yet"]);
        // no data at all: nothing to save
        assert!(unsaved_items(None, saved.as_ref(), file, "", "", None).is_empty());
        // syntax: new and not empty, or changed since its file was loaded or saved
        let job = Some(Path::new("/d/job.sps"));
        assert!(unsaved_items(None, None, None, "", "", None).is_empty());
        let new = unsaved_items(None, None, None, "FREQ x.", "", None);
        assert_eq!(new, ["Syntax: new syntax"]);
        assert!(unsaved_items(None, None, None, "FREQ x.", "FREQ x.", job).is_empty());
        let changed = unsaved_items(None, None, None, "FREQ y.", "FREQ x.", job);
        assert_eq!(changed, ["Syntax: job.sps"]);
    }

    #[test]
    fn a_run_ending_with_get_or_save_leaves_the_data_saved() {
        let commands = |names: &[&str]| -> Vec<CommandResult> {
            let command = |name: &&str| CommandResult {
                name: name.to_string(),
                first_line: 1,
                last_line: 1,
                tables: Vec::new(),
                warnings: Vec::new(),
            };
            names.iter().map(command).collect()
        };
        assert!(ends_on_file(&commands(&["GET"])));
        assert!(ends_on_file(&commands(&["GET DATA"])));
        assert!(ends_on_file(&commands(&["COMPUTE", "SAVE"])));
        assert!(!ends_on_file(&commands(&["GET", "COMPUTE"])));
        assert!(!ends_on_file(&commands(&["SAVE TRANSLATE"])));
        assert!(!ends_on_file(&[]));
    }

    /// The title bar, Alt+F4, File > Exit and the update installer all reach the window as a
    /// close request (egui-winit turns `ViewportCommand::Close` into one).
    #[test]
    fn a_close_request_is_held_only_with_unsaved_work() {
        let ctx = egui::Context::default();
        let close = |requested: bool, unsaved: Vec<String>| {
            let mut input = egui::RawInput::default();
            if requested {
                let root = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
                root.events.push(egui::ViewportEvent::Close);
            }
            let mut held = None;
            let mut out = ctx.run_ui(input, |ui| held = hold_close(ui.ctx(), || unsaved.clone()));
            out.textures_delta.clear();
            let commands = &out.viewport_output[&egui::ViewportId::ROOT].commands;
            let cancelled = commands.contains(&egui::ViewportCommand::CancelClose);
            (held, cancelled)
        };
        let items = vec!["Data: survey.sav".to_string()];
        assert_eq!(close(true, items.clone()), (Some(items.clone()), true));
        assert_eq!(close(true, Vec::new()), (None, false));
        assert_eq!(close(false, items), (None, false));
    }

    #[test]
    fn the_log_shows_under_the_syntax_and_the_output_only() {
        assert!(shows_log(Tab::Syntax) && shows_log(Tab::Output));
        assert!(!shows_log(Tab::Data) && !shows_log(Tab::Variables));
    }

    #[test]
    fn data_view_cells_follow_the_alignment_of_their_variable() {
        let mut session = open("survey_basic.sav");
        let aligns = |session: &Session| -> Vec<Align> {
            let data = session.dataset().unwrap();
            ["n", "s"]
                .map(|name| cell_align(&data.variable(name).unwrap()))
                .to_vec()
        };
        // new variables have no setting: numbers right, strings left
        run_in(&mut session, "NUMERIC n.\nSTRING s (A8).");
        assert_eq!(aligns(&session), [Align::Max, Align::Min]);
        run_in(&mut session, "VARIABLE ALIGNMENT n (LEFT) s (CENTER).");
        assert_eq!(aligns(&session), [Align::Min, Align::Center]);
        run_in(&mut session, "VARIABLE ALIGNMENT s (RIGHT).");
        assert_eq!(aligns(&session), [Align::Min, Align::Max]);
    }

    #[test]
    fn the_cases_a_filter_leaves_out_are_struck() {
        let mut session = open("survey_basic.sav");
        let struck = |session: &Session| -> Vec<bool> {
            let out = filtered_out(session.dataset().unwrap());
            (0..6).map(out).collect()
        };
        assert_eq!(struck(&session), [false; 6]);
        // 1, missing, 3 (user-missing), 0, 1, 2
        run_in(
            &mut session,
            "COMPUTE f = MOD($CASENUM, 4).\nIF ($CASENUM = 2) f = $SYSMIS.\n\
             MISSING VALUES f (3).\nFILTER BY f.",
        );
        assert_eq!(struck(&session), [false, true, true, true, false, false]);
        // the cases stay, only marked
        assert_eq!(session.dataset().unwrap().case_count(), 200);
        run_in(&mut session, "FILTER OFF.");
        assert_eq!(struck(&session), [false; 6]);
    }

    #[test]
    fn the_status_bar_names_the_split_filter_and_weight_with_their_dialogs() {
        let mut session = open("survey_basic.sav");
        let states = |session: &Session| -> Vec<(String, &'static str)> {
            data_states(session.dataset().unwrap())
                .into_iter()
                .map(|(text, dialog)| (text, dialog.title()))
                .collect()
        };
        assert!(states(&session).is_empty());
        run_in(
            &mut session,
            "SORT CASES BY region gender.\nSPLIT FILE SEPARATE BY region gender.\n\
             COMPUTE filter_$ = (age >= 40).\nFILTER BY filter_$.\nWEIGHT BY weight.",
        );
        let states = states(&session);
        assert_eq!(
            states,
            [
                (
                    "Split by region gender (separate)".to_string(),
                    "Split File"
                ),
                ("Filter: filter_$".to_string(), "Select Cases"),
                ("Weight: weight".to_string(), "Weight Cases"),
            ]
        );
        run_in(&mut session, "SPLIT FILE BY region.");
        let (text, dialog) = data_states(session.dataset().unwrap()).remove(0);
        assert_eq!(text, "Split by region (layered)");
        let Dialog::SplitFile(d) = dialog else {
            panic!("not the Split File dialog")
        };
        assert_eq!((d.mode, d.variables), (SplitMode::Layered, v(&["region"])));
    }
}
