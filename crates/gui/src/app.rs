//! The window. Drawing only: every change to data and every result goes through `Session::run`
//! with syntax built by the plain modules.

use crate::analyze_dialogs::{
    Correlations, Explore, ExploreMissing, MEANS_CELLS, MULT_RESPONSE_CELLS, Means, MrGroup,
    MultResponse, ONEWAY_STATISTICS, Oneway, STANDARDIZERS, TREE_LEVELS, TREE_SAVE, TTest,
    TTestKind, Tree,
};
use crate::connect_window::Connect;
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
use crate::output_view;
use crate::settings::Settings;
use crate::settings_window::{self, Section};
use crate::stats_dialogs::{NparKind, NparTests, PartialCorr, Regression, Reliability, numeric};
use crate::stats_dialogs_view;
use crate::syntax_text::{number, quote};
use crate::syntax_window;
use crate::updates::Updates;
use crate::windows::{Closing, Kind, Registry, WindowId, same};
use crate::{editor, grid, grid_menu, highlight, themes};
use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, FocusDirection, FontId, Key, Modifiers, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextStyle, Ui, UiBuilder, pos2, vec2,
};
use statscraft_data::{Dataset, Measure, Value, Values};
use statscraft_engine::Chart;
use statscraft_engine::live::{self, Action, Request, Response};
use statscraft_engine::session::{CommandResult, Failure, Queued, RunQueue, RunResult, Session};
use statscraft_export::picture::{self, Paper};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The documentation online.
const DOCS: &str = "https://docs.statscraft.org";

/// A request from `statscraft --session gui` and where its reply goes.
type Remote = (Request, mpsc::Sender<Response>);

/// What to do when a run finishes.
enum After {
    Show,
    /// A run from this syntax window, whose errors it lists.
    Editor(WindowId),
    OpenData(PathBuf),
    SaveData(PathBuf),
    /// With the dialog's title, for the output.
    CloseDialog(&'static str),
    /// A click in the window of this dataset made it active.
    Activate(String),
    /// The dataset or output document of this window was closed: the window goes.
    CloseWindow(WindowId),
    Remote(Remote),
    /// A run of this syntax window asked for from outside: replied to like `Remote`, shown like
    /// `Editor`.
    Window(Remote, WindowId),
}

/// The question about unsaved work, and the saves before the program ends.
enum Ask {
    /// The question, with what is unsaved and what follows the answer.
    Unsaved(Vec<String>, Then),
    /// Save was chosen before quitting: the datasets still to save once no run is going on,
    /// each by name (None for the active one) and file; then the program ends, unless a save
    /// failed.
    SavingToClose(Vec<(Option<String>, PathBuf)>),
}

/// What follows the question about unsaved work.
enum Then {
    /// The program ends; the question shows in this window.
    Quit(WindowId),
    /// This window closes, and the question shows in it.
    Close(WindowId),
    /// This syntax runs and closes the active dataset without a name, which is lost unless
    /// saved first. The question shows in the window.
    Replace(WindowId, String, Box<After>),
}

impl Then {
    fn window(&self) -> WindowId {
        match self {
            Then::Quit(id) | Then::Close(id) | Then::Replace(id, ..) => *id,
        }
    }
}

/// A run in the queue: what to do when it ends, and its syntax, for the output.
struct Run {
    after: After,
    source: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Data,
    Variables,
}

/// What a data window shows of its dataset beyond the data itself: the tab, the selections,
/// the cell or field being edited, the find bar and the small dialogs of its grids. The window
/// being drawn lends its view to `App::view`.
pub(crate) struct DataView {
    tab: Tab,
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
}

impl Default for DataView {
    fn default() -> Self {
        DataView {
            tab: Tab::Data,
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
        }
    }
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
    /// The view of the data window being drawn; the other windows keep theirs.
    view: DataView,
    value_labels: bool,
    show_log: bool,
    /// The dataset a click in its window is activating, until that run is done.
    activating: Option<String>,
    data_file: Option<PathBuf>,
    /// The open windows: data, syntax and output.
    windows: Registry,
    /// The window being drawn, which its menu acts on.
    here: WindowId,
    /// The window that shows the dialogs, Settings and About: the one they were opened from.
    popups: WindowId,
    /// The icon of every window.
    icon: Arc<egui::IconData>,
    /// The data file before the last change, which undo puts back too.
    file_before_change: Option<PathBuf>,
    /// The syntax of the run going on or done last, for the syntax of its output.
    source: String,
    /// The macros defined by the runs of the session, for completion and colours.
    macros: Vec<String>,
    /// The About window is open.
    about: bool,
    /// The Settings dialog is open at this section.
    settings_section: Option<Section>,
    /// Edit > Settings > AI agents.
    connect: Connect,
    /// The dataset as the last open or save left it, to tell whether the data changed since.
    data_saved: Option<Dataset>,
    /// The data file and saved data of the named datasets that are not active, so Save never
    /// writes one dataset over another's file.
    data_files: std::collections::HashMap<String, DataFile>,
    ask: Option<Ask>,
    /// The user chose to close: the next close request goes through.
    close_ok: bool,
    /// The app icon, shown in the About window.
    logo: egui::TextureHandle,
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
    /// Every run of every window and of the command line, one at a time in turn.
    runs: RunQueue<Run>,
    /// Data to open once nothing runs, as syntax that reads it: the name of a new dataset
    /// (DataSet2) is taken when its open starts, so data opened together gets different names.
    opens: std::collections::VecDeque<(String, After)>,
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
        let repaint = app_ctx.clone();
        let runs = RunQueue::new(move || repaint.request_repaint());
        let mut app = App {
            session: Session::default(),
            view: DataView::default(),
            value_labels: true,
            show_log: !settings.log_hidden,
            activating: None,
            data_file: None,
            windows: Registry::new(),
            here: 0,
            popups: 0,
            icon: Arc::new(crate::icon()),
            file_before_change: None,
            source: String::new(),
            macros: Vec::new(),
            about: false,
            settings_section: None,
            connect: Connect::default(),
            data_saved: None,
            data_files: Default::default(),
            ask: None,
            close_ok: false,
            logo,
            settings,
            updates,
            dialog: None,
            groups: Vec::new(),
            picked: Vec::new(),
            filter: String::new(),
            status,
            warnings: 0,
            title: String::new(),
            requests,
            runs,
            opens: Default::default(),
            ctx: app_ctx,
            server: server.ok(),
        };
        app.here = app.windows.root();
        app.popups = app.here;
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
            } else if is_output_file(&path) {
                app.open_output(path);
            } else {
                app.open_data(path);
            }
        }
        app
    }

    /// Puts syntax in the one run queue of the session; it runs on a worker thread once the
    /// runs before it are done, and `finish` shows the result.
    fn run(&mut self, source: &str, lines: Option<(u32, u32)>, after: After) {
        let check = matches!(&after, After::Remote((r, _)) if r.action == Action::Check);
        if let After::CloseWindow(id) = after
            && let Some(w) = self.windows.get_mut(id)
        {
            w.closing = true;
        }
        // relative file names: from the folder of the syntax that runs, or of the designated
        // syntax window
        let window = match &after {
            After::Editor(id) | After::Window(_, id) => Some(*id),
            _ => self.windows.designated_syntax(),
        };
        let file = window.and_then(|id| self.windows.syntax(id)?.file.as_deref());
        let base_dir = file.and_then(Path::parent).map(Path::to_path_buf);
        let tag = Run {
            after,
            source: source.to_string(),
        };
        let run = Queued {
            source: source.to_string(),
            lines,
            check,
            base_dir,
            tag,
        };
        self.runs.push(&mut self.session, run);
    }

    /// Shows a finished run, then takes the next request from the command line when nothing
    /// runs.
    fn poll(&mut self) {
        let active = active_name(&self.session);
        let designated = self.session.designated_output().map(str::to_string);
        if let Some((run, result)) = self.runs.poll(&mut self.session) {
            let named = |r: &RunResult| r.commands.iter().any(|c| c.name == "OUTPUT NAME");
            let renamed = designated.filter(|_| matches!(&result, Ok(r) if named(r)));
            self.follow_session(active, renamed);
            self.source = run.source;
            self.finish(run.after, result);
            self.macros = session_macros(&self.session.log);
        }
        while self.runs.is_idle()
            && let Some((open, after)) = self.opens.pop_front()
        {
            self.open_named(open, after);
        }
        while self.runs.is_idle()
            && let Ok(remote) = self.requests.try_recv()
        {
            self.answer(remote);
        }
    }

    /// Answers `statscraft run --session gui` and the like. A run shows in the window like one
    /// of its own; describe and check change nothing. A request is taken only when nothing
    /// runs, so it starts at once, in the caller's folder.
    fn answer(&mut self, (mut request, reply): Remote) {
        match request.action {
            Action::Describe => {
                let _ = reply.send(live::describe(&self.session, request.json));
            }
            Action::Run | Action::Check => {
                request.enter_folder();
                let source = std::mem::take(&mut request.source);
                let lines = request.lines;
                self.run(&source, lines, After::Remote((request, reply)));
            }
            Action::RunWindow => match live::window_source(&mut self.windows, &request) {
                // the lines widen to whole commands, as Ctrl+Enter does
                Ok((window, text)) => {
                    let lines = request.lines.map(|l| editor::whole_commands(&text, l));
                    self.run(&text, lines, After::Window((request, reply), window.id));
                }
                Err(errors) => {
                    let _ = reply.send(Err(errors));
                }
            },
            _ => self.answer_windows(request, reply),
        }
    }

    /// Lists, reads, edits, opens or saves syntax windows for the command line or an agent.
    /// Nothing runs. A new or opened window comes to the front; an edit shows at once, and
    /// Ctrl+Z in the editor takes it back.
    fn answer_windows(&mut self, request: Request, reply: mpsc::Sender<Response>) {
        let before = self.windows.ids();
        let named = live::find(&self.windows, request.window.as_deref()).ok();
        let answer = live::answer_windows(&mut self.windows, &request);
        if answer.is_ok() {
            let file = match request.action {
                Action::Open => request.path.clone(),
                Action::Save => named.and_then(|w| self.windows.syntax(w.id)?.file.clone()),
                _ => None,
            };
            if let Some(file) = &file {
                self.add_recent(file);
                if request.action == Action::Save {
                    self.status = Ok(format!("syntax saved to {}", file.display()));
                }
            }
            let mut front: Vec<WindowId> = self.windows.ids();
            front.retain(|id| !before.contains(id));
            if front.is_empty() && request.action == Action::Open {
                front.extend(file.and_then(|f| self.windows.with_file(&f)));
            }
            for id in front {
                self.front(id);
            }
        }
        let _ = reply.send(answer);
    }
    /// After a run or an undo, from the active dataset `was` before it: the file of each
    /// dataset follows it, and the data and output windows follow the session's datasets and
    /// documents. `renamed` is the document an OUTPUT NAME of the run renamed.
    fn follow_session(&mut self, was: Option<String>, renamed: Option<String>) {
        let now = active_name(&self.session);
        if now != was {
            let current = (self.data_file.take(), self.data_saved.take());
            (self.data_file, self.data_saved) =
                switch_data(&mut self.data_files, current, was.clone(), now.clone());
        }
        let shown = shown_datasets(&self.session);
        // a closed name given again later starts without the old file
        self.data_files
            .retain(|name, _| shown.iter().any(|s| same(s.as_deref(), Some(name))));
        let root = self.windows.root();
        let (_, mut closed) = self
            .windows
            .sync_data(&shown, was.as_deref(), now.as_deref());
        let documents: Vec<String> = self
            .session
            .output_documents()
            .into_iter()
            .map(|d| d.name)
            .collect();
        let designated = self.session.designated_output();
        let (_, gone) = self
            .windows
            .sync_output(&documents, designated, renamed.as_deref());
        closed.extend(gone);
        self.windows_closed(root, &closed);
    }

    fn finish(&mut self, after: After, mut result: Result<RunResult, Vec<Failure>>) {
        for view in self.windows.data_views() {
            view.data_resize = None;
        }
        self.warnings = 0;
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
                // every run sent here leaves its syntax in the Agent window; a failed one
                // changed nothing
                if request.action == Action::Run
                    && let Ok(r) = &mut result
                {
                    let syntax = live::last_syntax(&self.session);
                    r.trail = Some(live::keep_trail(&mut self.windows, &syntax));
                }
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
                for view in self.windows.data_views() {
                    view.cell_edit = None;
                    view.field_edit = None;
                }
                self.show(result, "command line", None);
            }
            After::Show => {
                self.show(result, "window", None);
            }
            After::Activate(name) => {
                if same(self.activating.as_deref(), Some(&name)) {
                    self.activating = None;
                }
                self.show(result, "window", None);
            }
            After::CloseWindow(id) => {
                if self.show(result, "window", None) {
                    self.drop_window(id);
                } else if let Some(w) = self.windows.get_mut(id) {
                    // the close failed: the window stays, and can be closed again
                    w.closing = false;
                }
            }
            After::Editor(id) => {
                self.show(result, "syntax editor", Some(id));
            }
            After::Window((request, reply), id) => {
                if let Ok(r) = &mut result {
                    let name = self
                        .windows
                        .syntax(id)
                        .map(|w| w.name())
                        .unwrap_or_default();
                    r.trail = Some(format!("syntax window {name}"));
                }
                let _ = reply.send(live::reply(&mut result, &request));
                for view in self.windows.data_views() {
                    view.cell_edit = None;
                    view.field_edit = None;
                }
                self.show(result, "syntax editor", Some(id));
            }
            After::OpenData(path) => {
                if self.show(result, "open file", None) {
                    self.add_recent(&path);
                    self.data_file = Some(path);
                    self.front_active_data();
                }
            }
            After::SaveData(path) => {
                if self.show(result, "save file", None) {
                    self.add_recent(&path);
                    self.data_file = Some(path);
                }
            }
            After::CloseDialog(title) => {
                if self.show(result, title, None) {
                    self.dialog = None;
                    if title == "Open Data" {
                        self.front_active_data();
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
        if !self.runs.is_idle() {
            return;
        }
        let syntax = self.session.undoable().unwrap_or_default().to_string();
        let was = active_name(&self.session);
        if self.session.undo() {
            self.macros = session_macros(&self.session.log);
            self.follow_session(was, None);
            self.data_file = self.file_before_change.take();
            self.view.cell_edit = None;
            self.view.field_edit = None;
            for view in self.windows.data_views() {
                view.cell_edit = None;
                view.field_edit = None;
            }
            let first = syntax.lines().next().unwrap_or_default();
            self.status = Ok(format!("undone: {first}"));
        }
    }

    /// Puts the tables of a run in the output, or its errors in the status bar and, for a
    /// syntax window, in the window's error list.
    fn show(
        &mut self,
        result: Result<RunResult, Vec<Failure>>,
        origin: &str,
        editor: Option<WindowId>,
    ) -> bool {
        let from_editor = editor.is_some();
        if let Some(w) = editor.and_then(|id| self.windows.syntax_mut(id)) {
            w.errors = result.as_ref().err().cloned().unwrap_or_default();
        }
        match result.map(|r| r.commands) {
            Ok(outputs) => {
                let mut shown = None;
                follow_modified(&mut self.windows, &outputs);
                for (document, label, commands) in output_groups(outputs, &self.source, from_editor)
                {
                    let label = label.as_deref().unwrap_or(origin);
                    if let Some(id) = output_window(&self.windows, document.as_deref())
                        && let Some(o) = self.windows.output_mut(id)
                        && o.view.add_run(label, commands)
                    {
                        shown = Some(id);
                    }
                }
                if let Some(id) = shown {
                    self.front(id);
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
                false
            }
        }
    }

    /// Opens an output file into a new output window, through OUTPUT OPEN.
    fn open_output(&mut self, path: PathBuf) {
        self.add_recent(&path);
        let syntax = format!("OUTPUT OPEN FILE={}.", quote(&path.to_string_lossy()));
        self.run(&syntax, None, After::Show);
    }

    /// The document of an output window, with the file it was opened from or saved to, and
    /// whether it holds output not saved.
    fn output_document(&self, id: WindowId) -> Option<statscraft_engine::OutputDocument> {
        output_document(&self.session, &self.windows.output(id)?.document)
    }

    /// What closing an output window would lose.
    fn unsaved_output(&self, id: WindowId) -> Option<String> {
        unsaved_output(&self.session, &self.windows.output(id)?.document)
    }

    /// The syntax that saves the document of an output window: to its file, or with `ask` (or
    /// without a file) to one chosen now. None when no file was chosen.
    fn save_output_syntax(&mut self, id: WindowId, ask: bool) -> Option<String> {
        let document = self.output_document(id)?;
        let file = document.file.filter(|_| !ask);
        let path = file.or_else(|| {
            let dialog = self
                .file_dialog()
                .add_filter("Statscraft output", &["scout"]);
            let mut path = dialog.save_file()?;
            if path.extension().is_none() {
                path.set_extension("scout");
            }
            Some(path)
        })?;
        self.add_recent(&path);
        let file = quote(&path.to_string_lossy());
        Some(format!(
            "OUTPUT SAVE NAME={} OUTFILE={file}.",
            document.name
        ))
    }

    /// File > Save output and Save output as: the document of an output window, through
    /// OUTPUT SAVE.
    fn save_output(&mut self, id: WindowId, ask: bool) {
        if let Some(syntax) = self.save_output_syntax(id, ask) {
            self.run(&syntax, None, After::Show);
        }
    }

    /// Deletes the selected tables of an output window from its document, then from the
    /// window, so both always hold the same items.
    fn delete_output(&mut self, id: WindowId) {
        if !self.runs.is_idle() {
            // the run works on a copy of the session that still has them
            self.status = Err("wait until the run ends to delete output".into());
            return;
        }
        if let Some(o) = self.windows.output_mut(id)
            && let Err(e) = delete_selected(&mut self.session, o)
        {
            self.status = Err(e);
        }
    }

    fn open_data(&mut self, path: PathBuf) {
        let get = format!("GET FILE={}.", quote(&path.to_string_lossy()));
        self.open_named(get, After::OpenData(path));
    }

    /// Runs syntax that reads data (GET, GET DATA) with a DATASET NAME after it, so the data
    /// open before stays open. It waits until nothing runs: the name (DataSet2) is taken when
    /// the open starts, so data opened together does not get the same name.
    fn open_named(&mut self, open: String, after: After) {
        if !self.runs.is_idle() {
            self.opens.push_back((open, after));
            return;
        }
        let syntax = self.session.open_syntax(&open);
        self.run_replacing(syntax, after);
    }

    /// Runs syntax that makes another dataset active (DATASET ACTIVATE, GET). The active
    /// dataset closes then when it has no name, as the manual describes; when it holds changes
    /// not saved, the question comes first.
    fn run_replacing(&mut self, syntax: String, after: After) {
        let lost = match active_name(&self.session) {
            Some(_) => None,
            None => self.unsaved_of(None),
        };
        match lost {
            None => self.run(&syntax, None, after),
            Some(item) if self.ask.is_none() => {
                let then = Then::Replace(self.here, syntax, Box::new(after));
                self.ask = Some(Ask::Unsaved(vec![item], then));
            }
            // another question is open: this waits for nothing and is dropped
            Some(_) => self.drop_activation(&after),
        }
    }

    /// An activation that will not run after all.
    fn drop_activation(&mut self, after: &After) {
        if let After::Activate(name) = after
            && same(self.activating.as_deref(), Some(name))
        {
            self.activating = None;
        }
    }

    /// A click in a data window makes its dataset active, through syntax, unless it is active
    /// or being activated already, or a question is open.
    fn click_activate(&mut self, id: WindowId) {
        if self.ask.is_some() {
            return;
        }
        let Some(name) = self.windows.data(id).and_then(|w| w.dataset.clone()) else {
            return;
        };
        let active = active_name(&self.session);
        if let Some(syntax) = activation(&name, active.as_deref(), self.activating.as_deref()) {
            self.activating = Some(name.clone());
            self.run_replacing(syntax, After::Activate(name));
        }
    }

    /// The active dataset once the activation clicked for has run.
    fn will_be_active(&self) -> Option<String> {
        self.activating
            .clone()
            .or_else(|| active_name(&self.session))
    }

    /// Brings the data window of the active dataset to the front.
    fn front_active_data(&mut self) {
        let active = active_name(&self.session);
        if let Some(id) = self.windows.with_dataset(active.as_deref()) {
            self.front(id);
        }
    }

    /// The file a dataset was opened from or saved to, and its data as that left it. None
    /// names the active dataset without a name.
    fn file_of(&self, name: Option<&str>) -> (Option<&Path>, Option<&Dataset>) {
        if same(name, active_name(&self.session).as_deref()) {
            return (self.data_file.as_deref(), self.data_saved.as_ref());
        }
        let kept = self
            .data_files
            .iter()
            .find(|(n, _)| same(Some(n), name))
            .map(|(_, f)| f);
        kept.map_or((None, None), |(file, saved)| {
            (file.as_deref(), saved.as_ref())
        })
    }

    /// What closing a dataset would lose: its changes since its file was opened or saved.
    fn unsaved_of(&self, name: Option<&str>) -> Option<String> {
        let data = dataset_named(&self.session, name);
        let (file, saved) = self.file_of(name);
        unsaved_data(data, saved, file, name).pop()
    }

    /// The syntax that closes the dataset of a data window; None for an empty window.
    fn close_data(&self, name: Option<&str>) -> Option<String> {
        let shown = shown_datasets(&self.session);
        let other = shown
            .iter()
            .flatten()
            .find(|o| !same(Some(o), name))
            .map(String::as_str);
        let has_data = dataset_named(&self.session, name).is_some();
        close_data(name, self.will_be_active().as_deref(), has_data, other)
    }

    /// Files dropped on the window from the file manager.
    fn open_dropped(&mut self, paths: Vec<PathBuf>) {
        let drop = dropped(paths);
        if let Some(path) = drop.syntax {
            self.load_syntax(path);
        }
        for path in drop.output {
            self.open_output(path);
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

    /// Opens a syntax file in a new syntax window, or brings the window that has it to the
    /// front.
    fn load_syntax(&mut self, path: PathBuf) {
        if let Some(id) = self.windows.with_file(&path) {
            self.front(id);
            return;
        }
        let text = std::fs::read(&path).map_err(|e| e.to_string());
        match text.and_then(|bytes| statscraft_syntax::decode_syntax(&bytes, None)) {
            Ok(text) => {
                self.add_recent(&path);
                self.windows.open_syntax(text, Some(&path));
            }
            Err(e) => self.status = Err(format!("{}: {e}", path.display())),
        }
    }

    /// Saves the syntax of a window to a file; false when it was not saved.
    fn save_syntax(&mut self, id: WindowId, path: PathBuf) -> bool {
        let Some(w) = self.windows.syntax_mut(id) else {
            return false;
        };
        match std::fs::write(&path, &w.text) {
            Ok(()) => {
                w.saved = w.text.clone();
                self.status = Ok(format!("syntax saved to {}", path.display()));
                w.file = Some(path.clone());
                self.add_recent(&path);
                true
            }
            Err(e) => {
                self.status = Err(format!("{}: {e}", path.display()));
                false
            }
        }
    }

    /// Saves a syntax window to its file, or to one chosen now; false when it was not saved.
    fn save_syntax_now(&mut self, id: WindowId) -> bool {
        match self.windows.syntax(id).map(|w| w.file.clone()) {
            Some(Some(path)) => self.save_syntax(id, path),
            Some(None) => self.save_syntax_as(id),
            None => false,
        }
    }

    fn save_syntax_as(&mut self, id: WindowId) -> bool {
        match self
            .file_dialog()
            .add_filter("Syntax", &["sps"])
            .save_file()
        {
            Some(path) => self.save_syntax(id, path),
            None => false,
        }
    }

    /// Brings a window to the front. A window not drawn yet opens in front anyway; a command
    /// to it now would make egui start its viewport without its title and size, and eframe
    /// would then have to make that native window again.
    fn front(&self, id: WindowId) {
        if self.windows.get(id).is_none_or(|w| w.size.is_none()) {
            return;
        }
        let viewport = self.windows.viewport(id);
        self.ctx
            .send_viewport_cmd_to(viewport, egui::ViewportCommand::Focus);
    }

    /// The dataset that File > Save data saves: the one of the data window, else the active one.
    fn data_here(&self) -> Option<String> {
        match self.windows.data(self.here) {
            Some(w) => w.dataset.clone(),
            None => active_name(&self.session),
        }
    }

    /// The output window that File > Export output exports: this one, else the designated one.
    fn output_here(&self) -> Option<WindowId> {
        match self.windows.output(self.here) {
            Some(_) => Some(self.here),
            None => self.windows.designated_output(),
        }
    }

    fn menu(&mut self, ui: &mut Ui) {
        let has_data = window_data(&self.session, &self.windows, self.here).is_some();
        let output = self.output_here();
        let has_output = output
            .and_then(|id| self.windows.output(id))
            .is_some_and(|o| !o.view.is_empty());
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
                    let name = self.data_here();
                    match self.file_of(name.as_deref()).0.map(Path::to_path_buf) {
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
                ui.menu_button("New", |ui| {
                    if ui.button("Syntax").clicked() {
                        self.windows.open_syntax(String::new(), None);
                    }
                    if ui.button("Output").clicked() {
                        self.run("OUTPUT NEW.", None, After::Show);
                    }
                });
                if ui.button("Open syntax...").clicked()
                    && let Some(path) = self
                        .file_dialog()
                        .add_filter("Syntax", &["sps"])
                        .pick_file()
                {
                    self.load_syntax(path);
                }
                if ui.button("Open output...").clicked()
                    && let Some(path) = self
                        .file_dialog()
                        .add_filter("Statscraft output", &["scout"])
                        .pick_file()
                {
                    self.open_output(path);
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
                                } else if is_output_file(&path) {
                                    self.open_output(path);
                                } else {
                                    self.open_data(path);
                                }
                            }
                        }
                    });
                });
                let syntax = self.windows.syntax(self.here).is_some();
                if ui
                    .add_enabled(syntax, egui::Button::new("Save syntax"))
                    .clicked()
                {
                    self.save_syntax_now(self.here);
                }
                if ui
                    .add_enabled(syntax, egui::Button::new("Save syntax as..."))
                    .clicked()
                {
                    self.save_syntax_as(self.here);
                }
                ui.separator();
                if ui
                    .add_enabled(output.is_some(), egui::Button::new("Save output"))
                    .clicked()
                    && let Some(id) = output
                {
                    self.save_output(id, false);
                }
                if ui
                    .add_enabled(output.is_some(), egui::Button::new("Save output as..."))
                    .clicked()
                    && let Some(id) = output
                {
                    self.save_output(id, true);
                }
                if ui
                    .add_enabled(has_output, egui::Button::new("Export output as HTML..."))
                    .clicked()
                    && let Some(id) = output
                {
                    self.export_html(id, false);
                }
                ui.add_enabled_ui(has_output, |ui| {
                    ui.menu_button("Export output as Excel", |ui| {
                        if let Some(id) = output {
                            if ui.button("One sheet per table...").clicked() {
                                self.export_xlsx(id, false, false);
                            }
                            if ui.button("All tables on one sheet...").clicked() {
                                self.export_xlsx(id, true, false);
                            }
                        }
                    });
                });
                ui.separator();
                if ui.button("Close").clicked() {
                    self.close_window(self.here);
                }
                if ui.button("Exit").clicked() {
                    self.quit(self.here);
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = self.runs.is_idle() && self.session.undoable().is_some();
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
                    self.popups = self.here;
                }
            });
            ui.menu_button("Data", |ui| {
                let data = window_data(&self.session, &self.windows, self.here);
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
            ui.menu_button("Window", |ui| self.window_menu(ui));
            ui.menu_button("Help", |ui| {
                if ui.button("Documentation").on_hover_text(DOCS).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DOCS));
                }
                if ui.button("About Statscraft").clicked() {
                    self.about = true;
                    self.popups = self.here;
                }
                // the same as Edit > Settings > AI agents, where people also look for it
                if ui.button("Connect an AI agent...").clicked() {
                    self.settings_section = Some(Section::AiAgents);
                    self.connect.refresh();
                    self.popups = self.here;
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
        // the dialogs show in the window they were opened from, and work on its data
        self.popups = self.here;
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

    /// The datasets with changes not saved, the active one first.
    fn unsaved_datasets(&self) -> Vec<Option<String>> {
        let shown = shown_datasets(&self.session).into_iter();
        shown
            .filter(|name| self.unsaved_of(name.as_deref()).is_some())
            .collect()
    }

    /// What would be lost if the program ended now: every dataset, then the syntax of every
    /// window, then the output.
    fn unsaved(&self) -> Vec<String> {
        let data = self.unsaved_datasets().into_iter();
        let mut items: Vec<String> = data
            .filter_map(|name| self.unsaved_of(name.as_deref()))
            .collect();
        items.extend(self.windows.unsaved());
        let ids = self.windows.ids().into_iter();
        items.extend(ids.filter_map(|id| self.unsaved_output(id)));
        items
    }

    /// The Window menu: every open window, where a click brings it to the front, and the
    /// designated syntax window (marked +), which takes pasted syntax.
    fn window_menu(&mut self, ui: &mut Ui) {
        let mut chosen = None;
        for w in self.windows.iter() {
            let name = self.window_name(w.id);
            let name = if w.designated {
                format!("{name} +")
            } else {
                name
            };
            if ui.selectable_label(w.id == self.here, name).clicked() {
                chosen = Some(w.id);
            }
        }
        if let Some(id) = chosen {
            self.front(id);
        }
        ui.separator();
        let designated = self.windows.get(self.here).is_some_and(|w| w.designated);
        let output = self.windows.output(self.here).map(|o| o.document.clone());
        let designate =
            (self.windows.syntax(self.here).is_some() || output.is_some()) && !designated;
        if ui
            .add_enabled(designate, egui::Button::new("Designate window"))
            .clicked()
        {
            match output {
                // the session's designated document follows, and the window with it
                Some(document) => {
                    let syntax = format!("OUTPUT ACTIVATE {document}.");
                    self.run(&syntax, None, After::Show);
                }
                None => self.windows.designate(self.here),
            }
        }
    }

    /// The name of a window, in its title bar and the Window menu. A data window names its
    /// file and dataset, and says when its dataset is the active one.
    fn window_name(&self, id: WindowId) -> String {
        match self.windows.get(id).map(|w| &w.kind) {
            Some(Kind::Syntax(s)) => s.name(),
            Some(Kind::Output(o)) => o.document.clone(),
            Some(Kind::Data(d)) => {
                let name = d.dataset.as_deref();
                if dataset_named(&self.session, name).is_none() {
                    return "Data".into();
                }
                let file = self.file_of(name).0.and_then(Path::file_name);
                let file = file.map(|f| f.to_string_lossy().into_owned());
                let title = match (file, name) {
                    (Some(file), Some(name)) => format!("{file} [{name}]"),
                    (Some(file), None) => file,
                    (None, Some(name)) => name.to_string(),
                    (None, None) => "Untitled".into(),
                };
                match same(name, active_name(&self.session).as_deref()) {
                    true => format!("{title} (active)"),
                    false => title,
                }
            }
            None => String::new(),
        }
    }

    /// File > Close and the close button of a window. Closing the last window quits; a window
    /// with unsaved syntax or data asks first.
    fn close_window(&mut self, id: WindowId) {
        if self.ask.is_some() || self.windows.get(id).is_some_and(|w| w.closing) {
            return;
        }
        let data = self.windows.data(id).map(|w| w.dataset.clone());
        let item = data.and_then(|name| self.unsaved_of(name.as_deref()));
        let item = item.or_else(|| self.unsaved_output(id));
        match self.windows.closing(id, item) {
            Closing::Quit => self.quit(id),
            Closing::Ask(items) => self.ask = Some(Ask::Unsaved(items, Then::Close(id))),
            Closing::Close => self.close_now(id),
        }
    }

    /// Closes a window without a question. A data window closes its dataset and an output
    /// window its document, through syntax; the window goes once that has run.
    fn close_now(&mut self, id: WindowId) {
        if let Some(name) = self.windows.data(id).map(|w| w.dataset.clone()) {
            match self.close_data(name.as_deref()) {
                Some(syntax) => self.run(&syntax, None, After::CloseWindow(id)),
                None => self.drop_window(id),
            }
        } else if let Some(document) = self.windows.output(id).map(|o| o.document.clone()) {
            let syntax = format!("OUTPUT CLOSE {document}.");
            self.run(&syntax, None, After::CloseWindow(id));
        } else {
            self.drop_window(id);
        }
    }

    /// Save chosen before a data window closes: its dataset is saved, made active first when
    /// it is not, then closed. False when no file was chosen.
    fn save_and_close(&mut self, id: WindowId) -> bool {
        let Some(name) = self.windows.data(id).map(|w| w.dataset.clone()) else {
            return false;
        };
        let file = self.file_of(name.as_deref()).0.map(Path::to_path_buf);
        let Some(path) = file.or_else(|| self.data_file_dialog()) else {
            return false;
        };
        let activate = !same(name.as_deref(), self.will_be_active().as_deref());
        let mut syntax = String::new();
        if let Some(name) = name.as_deref().filter(|_| activate) {
            syntax = syntax_window::activate(name) + "\n";
        }
        syntax.push_str(&format!("SAVE OUTFILE={}.", quote(&path.to_string_lossy())));
        // closed as the active dataset, which it is by then
        let shown = shown_datasets(&self.session);
        let other = shown
            .iter()
            .flatten()
            .find(|o| !same(Some(o), name.as_deref()));
        if let Some(close) = close_data(
            name.as_deref(),
            name.as_deref(),
            true,
            other.map(|o| o.as_str()),
        ) {
            syntax = format!("{syntax}\n{close}");
        }
        match activate {
            true => self.run_replacing(syntax, After::CloseWindow(id)),
            false => self.run(&syntax, None, After::CloseWindow(id)),
        }
        true
    }

    /// Takes a window away. When the root drew it, the root draws the next window, in that
    /// window's place. The last window is never taken away: the program ends with it.
    fn drop_window(&mut self, id: WindowId) {
        if self.windows.ids() == [id] {
            return self.quit(id);
        }
        let root = self.windows.root();
        self.windows.close(id);
        self.windows_closed(root, &[id]);
    }

    /// After windows closed: the dialogs show in the root when theirs closed, and when the root
    /// drew one of them, it draws the next window, in that window's place.
    fn windows_closed(&mut self, root: WindowId, closed: &[WindowId]) {
        if closed.contains(&self.popups) {
            self.popups = self.windows.root();
        }
        if closed.contains(&root) {
            let viewport = egui::ViewportId::ROOT;
            if let Some(w) = self.windows.get(self.windows.root()) {
                if let Some(position) = w.position {
                    let command = egui::ViewportCommand::OuterPosition(position);
                    self.ctx.send_viewport_cmd_to(viewport, command);
                }
                if let Some(size) = w.size {
                    let command = egui::ViewportCommand::InnerSize(size);
                    self.ctx.send_viewport_cmd_to(viewport, command);
                }
            }
            self.ctx
                .send_viewport_cmd_to(viewport, egui::ViewportCommand::Focus);
        }
    }

    /// File > Exit, the close of the last window and the update installer: with unsaved work
    /// anywhere, the question comes first, in window `here`.
    fn quit(&mut self, here: WindowId) {
        if self.ask.is_some() {
            return;
        }
        let items = self.unsaved();
        if items.is_empty() {
            self.end();
        } else {
            self.ask = Some(Ask::Unsaved(items, Then::Quit(here)));
        }
    }

    /// Ends the program: the root closes, and every window with it.
    fn end(&mut self) {
        self.close_ok = true;
        self.ctx
            .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }

    /// The root's close requests (its title bar, Alt+F4) close only the window it draws; the
    /// program ends with the last window. With Save chosen before quitting, the program ends
    /// once the saves are done.
    fn close_request(&mut self, ctx: &egui::Context) {
        if self.close_ok {
            return;
        }
        if let Some(Ask::SavingToClose(pending)) = &mut self.ask
            && self.runs.is_idle()
        {
            let next = (!pending.is_empty()).then(|| pending.remove(0));
            match next {
                Some((name, path)) => {
                    // a dataset that is not active is saved as the active one
                    let active = active_name(&self.session);
                    let mut syntax = format!("SAVE OUTFILE={}.", quote(&path.to_string_lossy()));
                    if let Some(name) = name.filter(|n| !same(Some(n), active.as_deref())) {
                        syntax = format!("{}\n{syntax}", syntax_window::activate(&name));
                    }
                    self.run(&syntax, None, After::SaveData(path));
                }
                None if self.unsaved().is_empty() => self.end(),
                // a save failed, and the status bar says why
                None => self.keep_open(),
            }
        }
        if self.close_ok {
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_window(self.windows.root());
        }
        if self.updates.take_quit() {
            if self.ask.is_some() {
                // a quit during a question is dropped, a restart with it
                self.updates.cancel_restart();
            } else {
                self.quit(self.windows.root());
            }
        }
    }

    /// The program goes on after all: no saves wait, and no restart.
    fn keep_open(&mut self) {
        self.ask = None;
        self.updates.cancel_restart();
    }

    /// The question about unsaved work, before a window closes or the program ends: Save,
    /// Don't save or Cancel. Escape or a click beside it cancels. It shows in its own window.
    fn unsaved_window(&mut self, ctx: &egui::Context) {
        let Some(Ask::Unsaved(items, then)) = &self.ask else {
            return;
        };
        if then.window() != self.here {
            return;
        }
        enum Answer {
            Save,
            Discard,
            Cancel,
        }
        let mut answer = None;
        let syntax = matches!(then, Then::Close(id) if self.windows.syntax(*id).is_some());
        let output = matches!(then, Then::Close(id) if self.windows.output(*id).is_some());
        let modal = egui::Modal::new(egui::Id::unique("unsaved")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(match then {
                Then::Quit(_) => "Save the changes before closing?",
                Then::Close(_) if syntax => "Save the changes to the syntax first?",
                Then::Close(_) if output => "Save the output first?",
                Then::Close(_) | Then::Replace(..) => "Save the changes to the data first?",
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
            (Answer::Save, Then::Quit(_)) => self.save_before_close(),
            (Answer::Discard, Then::Quit(_)) => self.end(),
            (Answer::Cancel, Then::Quit(_)) => self.keep_open(),
            (Answer::Save, Then::Close(id)) if self.windows.data(id).is_some() => {
                self.save_and_close(id);
            }
            (Answer::Save, Then::Close(id)) if self.windows.output(id).is_some() => {
                // saved, then closed in the same run; no file chosen keeps the window
                if let Some(save) = self.save_output_syntax(id, false) {
                    let document = self.output_document(id).map(|d| d.name);
                    let close = format!("OUTPUT CLOSE {}.", document.unwrap_or_default());
                    self.run(&format!("{save}\n{close}"), None, After::CloseWindow(id));
                }
            }
            (Answer::Save, Then::Close(id)) => {
                if self.save_syntax_now(id) {
                    self.drop_window(id);
                }
            }
            (Answer::Discard, Then::Close(id)) => self.close_now(id),
            (Answer::Cancel, Then::Close(_)) => {}
            (Answer::Save, Then::Replace(_, syntax, after)) => {
                let file = self.data_file.clone();
                match file.or_else(|| self.data_file_dialog()) {
                    Some(path) => {
                        let save = format!("SAVE OUTFILE={}.", quote(&path.to_string_lossy()));
                        self.run(&format!("{save}\n{syntax}"), None, *after);
                    }
                    None => self.drop_activation(&after),
                }
            }
            (Answer::Discard, Then::Replace(_, syntax, after)) => self.run(&syntax, None, *after),
            (Answer::Cancel, Then::Replace(_, _, after)) => self.drop_activation(&after),
        }
    }

    /// Saves the syntax of every window, then the data, each to its file or to one chosen now;
    /// a cancelled file dialog or a failed save keeps the program open.
    fn save_before_close(&mut self) {
        for id in self.windows.ids() {
            let unsaved = self
                .windows
                .syntax(id)
                .is_some_and(|w| w.unsaved().is_some());
            if unsaved && !self.save_syntax_now(id) {
                self.keep_open();
                return;
            }
        }
        // the output saves run first; the datasets wait until no run goes on
        for id in self.windows.ids() {
            if self.unsaved_output(id).is_some() {
                match self.save_output_syntax(id, false) {
                    Some(syntax) => self.run(&syntax, None, After::Show),
                    None => return self.keep_open(),
                }
            }
        }
        let mut pending = Vec::new();
        for name in self.unsaved_datasets() {
            let file = self.file_of(name.as_deref()).0.map(Path::to_path_buf);
            match file.or_else(|| self.data_file_dialog()) {
                Some(path) => pending.push((name, path)),
                None => return self.keep_open(),
            }
        }
        self.ask = Some(Ask::SavingToClose(pending));
    }

    /// Exports the whole output of a window, or with `selected` only the selected tables.
    fn export_html(&mut self, id: WindowId, selected: bool) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Web page", &["html"])
            .save_file()
        {
            self.settings.remember_folder(&path);
            let Some(view) = self.windows.output(id).map(|o| &o.view) else {
                return;
            };
            let chosen = &view.selected;
            let page = output::html(&view.runs, |id| !selected || chosen.contains(&id));
            self.status = match std::fs::write(&path, page) {
                Ok(()) => Ok(format!("output exported to {}", path.display())),
                Err(e) => Err(format!("{}: {e}", path.display())),
            };
        }
    }

    /// Saves a chart as a PNG or SVG picture on the paper the window shows it on.
    fn save_chart(&mut self, chart: &Chart, png: bool, paper: Paper) {
        let (kind, extension) = match png {
            true => ("PNG image", "png"),
            false => ("SVG image", "svg"),
        };
        let name: String = chart
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || " -_".contains(*c))
            .take(60)
            .collect();
        let Some(path) = self
            .file_dialog()
            .add_filter(kind, &[extension])
            .set_file_name(format!("{}.{extension}", name.trim()))
            .save_file()
        else {
            return;
        };
        self.settings.remember_folder(&path);
        let bytes = match png {
            true => picture::chart_png(chart, paper),
            false => picture::chart_svg(chart, paper).map(|svg| Ok(svg.into_bytes())),
        };
        self.status = match bytes {
            Some(Ok(bytes)) => match std::fs::write(&path, bytes) {
                Ok(()) => Ok(format!("chart saved to {}", path.display())),
                Err(e) => Err(format!("{}: {e}", path.display())),
            },
            Some(Err(e)) => Err(e),
            None => Err("this kind of chart has no picture yet".into()),
        };
    }

    fn export_xlsx(&mut self, id: WindowId, one_sheet: bool, selected: bool) {
        if let Some(path) = self
            .file_dialog()
            .add_filter("Excel workbook", &["xlsx"])
            .save_file()
        {
            self.settings.remember_folder(&path);
            let Some(view) = self.windows.output(id).map(|o| &o.view) else {
                return;
            };
            let chosen = &view.selected;
            let items = output::tables(&view.runs)
                .filter(|t| !selected || chosen.contains(&t.id))
                .map(|t| match &t.chart {
                    Some(chart) => statscraft_export::Item::Chart(chart),
                    None => statscraft_export::Item::Table(&t.table),
                });
            self.status = match statscraft_export::write_xlsx(&path, items, one_sheet) {
                Ok(()) => Ok(format!("output exported to {}", path.display())),
                Err(e) => Err(format!("{}: {e}", path.display())),
            };
        }
    }

    fn tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [(Tab::Data, "Data"), (Tab::Variables, "Variables")] {
                ui.selectable_value(&mut self.view.tab, tab, RichText::new(label).size(15.0));
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if let Some(data) = window_data(&self.session, &self.windows, self.here) {
                    ui.weak(format!(
                        "{} cases, {} variables",
                        data.case_count(),
                        data.variable_count()
                    ));
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        if let Some((run, label)) = self.runs.running() {
            let text = match &run.after {
                After::OpenData(path) => format!("Opening {}", path.display()),
                _ => format!("Running {label}"),
            };
            let waiting = self.runs.waiting();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(text);
                if waiting > 0 {
                    ui.strong(format!("{waiting} waiting"));
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            // the split, filter and weight in force, each opening its dialog
            let mut open = None;
            let data = window_data(&self.session, &self.windows, self.here);
            if let Some(states) = data.map(data_states) {
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
    /// from a data window in the designated output window.
    fn status_line(&mut self, ui: &mut Ui) {
        ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
            if let Some(note) = warning_note(self.warnings)
                && ui
                    .link(RichText::new(note).color(ui.visuals().warn_fg_color))
                    .clicked()
            {
                self.show_log = true;
                if self.windows.data(self.here).is_some()
                    && let Some(id) = self.windows.designated_output()
                {
                    self.front(id);
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
        if window_data(&self.session, &self.windows, self.here).is_none() {
            return Self::no_data(ui);
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.value_labels, "Show value labels");
            self.find_bar(ui);
        });
        paper(ui, !self.settings.paper_follows_system);
        let data = window_data(&self.session, &self.windows, self.here).expect("checked above");
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
        if let Some((c, width)) = self.view.data_resize
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
        let edit = &mut self.view.cell_edit;
        let events = paint_grid(
            ui,
            "data",
            &headers,
            &widths,
            data.case_count(),
            self.view.selected.as_ref(),
            std::mem::take(&mut self.view.follow),
            &cell,
            &filtered_out(data),
            edit.as_ref().map(|e| (e.row, e.column)),
            &mut |ui, rect| {
                if let Some(e) = edit.as_mut() {
                    (commit, cancel) = cell_editor(ui, rect, e);
                }
            },
        );
        self.view.page = events.page.max(1);
        let modifiers = ui.input(|i| i.modifiers);
        if select(&mut self.view.selected, &events, modifiers) {
            self.view.follow = true;
        }
        self.open_menu(events.secondary);
        if let Some(response) = &events.response {
            response.context_menu(|ui| self.grid_menu(ui));
        }
        if let Some((c, width, done)) = events.resized {
            self.view.data_resize = Some((c, width));
            if done {
                let data =
                    window_data(&self.session, &self.windows, self.here).expect("checked above");
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
            self.view.cell_edit = None;
        }
        if commit {
            self.commit_cell();
        }
    }

    fn edit_cell(&mut self, (row, column): (usize, usize)) {
        let Some(data) = window_data(&self.session, &self.windows, self.here) else {
            return;
        };
        let text = edits::edit_text(&data.variable_at(column), row);
        self.view.selected = Some(grid::Selection::cell((row, column)));
        self.view.cell_edit = Some(CellEdit {
            row,
            column,
            original: text.clone(),
            text,
            started: false,
        });
    }

    fn commit_cell(&mut self) {
        let (Some(e), Some(data)) = (
            self.view.cell_edit.take(),
            window_data(&self.session, &self.windows, self.here),
        ) else {
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
        let Some(data) = window_data(&self.session, &self.windows, self.here) else {
            return;
        };
        let variables = self.view.tab == Tab::Variables;
        let (rows, columns, editing) = match self.view.tab {
            Tab::Data => (
                data.case_count(),
                data.variable_count(),
                self.view.cell_edit.is_some(),
            ),
            Tab::Variables => (data.variable_count(), Field::ALL.len(), false),
        };
        if self.dialog.is_some()
            || self.view.confirm.is_some()
            || self.view.format_dialog.is_some()
            || self.view.labels_dialog.is_some()
            || self.view.missing_dialog.is_some()
            || (variables && self.view.field_edit.is_some())
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
            (
                &mut self.view.variable_selected,
                &mut self.view.variable_follow,
            )
        } else {
            (&mut self.view.selected, &mut self.view.follow)
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
                self.view.page,
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
        if !matches!(self.view.tab, Tab::Data | Tab::Variables) || self.dialog.is_some() {
            return;
        }
        let (open, next) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::NONE, Key::F3),
            )
        });
        if open {
            self.view.find.get_or_insert_with(String::new);
            self.view.find_focus = true;
        }
        if next {
            self.find_next(false);
        }
    }

    /// The find bar, while it is open: typing goes to the first match from the current
    /// variable, Enter to the next one, Escape closes it.
    fn find_bar(&mut self, ui: &mut Ui) {
        let Some(text) = self.view.find.as_mut() else {
            return;
        };
        let response = ui.add(
            TextEdit::singleline(text)
                .hint_text("Find a variable by name or label")
                .desired_width(260.0),
        );
        if std::mem::take(&mut self.view.find_focus) {
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
            self.view.find = None;
        }
    }

    /// Goes to the next variable that matches the find text: its row in the Variable View, its
    /// column in the Data View. `here` counts the current variable as a match too.
    fn find_next(&mut self, here: bool) {
        let (Some(needle), Some(data)) = (
            self.view.find.clone(),
            window_data(&self.session, &self.windows, self.here),
        ) else {
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
        let variables = self.view.tab == Tab::Variables;
        let cursor = if variables {
            self.view.variable_selected.as_ref().map(|s| s.cursor.0)
        } else {
            self.view.selected.as_ref().map(|s| s.cursor.1)
        };
        let after = if here {
            cursor.and_then(|c| c.checked_sub(1))
        } else {
            cursor
        };
        match grid::find(&items, &needle, after) {
            Some(i) if variables => {
                self.view.variable_selected = Some(grid::Selection::cell((i, 0)));
                self.view.variable_follow = true;
            }
            Some(i) => {
                let row = self.view.selected.as_ref().map_or(0, |s| s.cursor.0);
                self.view.selected = Some(grid::Selection::cell((row, i)));
                self.view.follow = true;
            }
            None if needle.trim().is_empty() => {}
            None => self.status = Err(format!("no variable name or label holds {needle:?}")),
        }
    }

    /// Puts the selection of the grid in view on the clipboard as tab-separated text.
    fn copy_selection(&mut self, ctx: &egui::Context) {
        let Some(data) = window_data(&self.session, &self.windows, self.here) else {
            return;
        };
        let (text, wide) = if self.view.tab == Tab::Variables {
            let Some(s) = &self.view.variable_selected else {
                return;
            };
            let headers: Vec<String> = Field::ALL.iter().map(|f| f.title().to_string()).collect();
            let size = (data.variable_count(), Field::ALL.len());
            let text = grid::copy(s, size, &headers, |r, c| {
                edits::field_text(&data.variable_at(r), Field::ALL[c])
            });
            (text, s.is_wide())
        } else {
            let Some(s) = &self.view.selected else {
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
        let Some(data) = window_data(&self.session, &self.windows, self.here) else {
            return;
        };
        let variable_view = self.view.tab == Tab::Variables;
        if !variable_view
            && let Some(s) = &self.view.selected
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
            true => self.view.variable_selected.as_ref().map(|s| &s.rows),
            false => self.view.selected.as_ref().map(|s| &s.columns),
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
            self.view.confirm = Some(confirm);
        }
    }

    /// The Type and Format dialog: a format from a list, its width and decimals.
    fn format_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.view.format_dialog.as_mut() else {
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
            self.view.format_dialog = None;
        }
    }

    /// The value labels dialog: a list of codes and labels, or many pasted at once.
    fn labels_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.view.labels_dialog.as_mut() else {
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
            self.view.labels_dialog = None;
        }
    }

    /// The missing values dialog: up to three values, or a range and one more value.
    fn missing_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.view.missing_dialog.as_mut() else {
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
            self.view.missing_dialog = None;
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
    /// Puts syntax from a dialog or the output at the end of the designated syntax window, or
    /// of a new one.
    fn paste(&mut self, text: &str) {
        let id = match self.windows.designated_syntax() {
            Some(id) => id,
            None => self.windows.open_syntax(String::new(), None),
        };
        if let Some(w) = self.windows.syntax_mut(id) {
            editor::append(&mut w.text, text);
        }
        self.front(id);
    }

    /// The question before syntax that deletes or reorders runs: the question, the syntax, the
    /// button that runs it and Cancel. Only a click runs it, so a stray Enter never deletes;
    /// Escape or a click beside the window cancels. A yes clears the selections, which may
    /// point at other variables or cases afterwards.
    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.view.confirm else {
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
                let syntax = self
                    .view
                    .confirm
                    .take()
                    .map(|c| c.syntax)
                    .unwrap_or_default();
                self.view.selected = None;
                self.view.variable_selected = None;
                self.run(&syntax, None, After::Show);
            }
            Some(false) => self.view.confirm = None,
            None => {}
        }
    }

    /// A right click on a grid: the menu acts on what the click selected.
    fn open_menu(&mut self, hit: Option<grid::Hit>) {
        let Some(hit) = hit else {
            return;
        };
        let variable_view = self.view.tab == Tab::Variables;
        let selection = match variable_view {
            true => &self.view.variable_selected,
            false => &self.view.selected,
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
        self.view.menu = target.map(|t| (t, variable_view));
    }

    /// The right-click menu of a grid: its target and the split, filter and weight in force
    /// at the top, then the items.
    fn grid_menu(&mut self, ui: &mut Ui) {
        let (Some((target, variable_view)), Some(data)) = (
            self.view.menu.clone(),
            window_data(&self.session, &self.windows, self.here),
        ) else {
            return ui.close();
        };
        let entries = grid_menu::entries(data, &target, !self.runs.is_idle());
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
            self.view.variable_selected = Some(s);
            self.view.variable_follow = true;
        } else {
            let row = self.view.selected.as_ref().map_or(0, |s| s.cursor.0);
            let mut s = grid::Selection::cell((row, index));
            s.click_column(index, false, false);
            self.view.selected = Some(s);
            self.view.follow = true;
        }
    }

    /// Does what a menu item asks for.
    fn act(&mut self, action: grid_menu::Action, ctx: &egui::Context) {
        match action {
            grid_menu::Action::Run(syntax) => self.run(&syntax, None, After::Show),
            grid_menu::Action::Ask(confirm) => self.view.confirm = Some(confirm),
            grid_menu::Action::Open(dialog) => self.open_dialog(*dialog),
            grid_menu::Action::Copy => self.copy_selection(ctx),
            grid_menu::Action::EditCell(case, column) => self.edit_cell((case, column)),
            grid_menu::Action::EditField(row, field) => {
                self.view.tab = Tab::Variables;
                self.view.variable_follow = true;
                self.edit_field((row, field));
            }
        }
    }

    fn variable_view(&mut self, ui: &mut Ui) {
        if window_data(&self.session, &self.windows, self.here).is_none() {
            return Self::no_data(ui);
        }
        if self.view.find.is_some() {
            ui.horizontal(|ui| self.find_bar(ui));
        }
        paper(ui, !self.settings.paper_follows_system);
        let data = window_data(&self.session, &self.windows, self.here).expect("checked above");
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
        let edit = &mut self.view.field_edit;
        let events = paint_grid(
            ui,
            "variables",
            &headers,
            &widths,
            data.variable_count(),
            self.view.variable_selected.as_ref(),
            std::mem::take(&mut self.view.variable_follow),
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
            self.view.rows_before_click = self
                .view
                .variable_selected
                .as_ref()
                .map(|s| s.rows.clone())
                .unwrap_or_default();
        }
        if select(&mut self.view.variable_selected, &events, modifiers) {
            self.view.variable_follow = true;
        }
        self.open_menu(events.secondary);
        if let Some(response) = &events.response {
            response.context_menu(|ui| self.grid_menu(ui));
        }
        if let Some(at) = events.clicked {
            // a text edit commits when it loses focus; a list just closes
            let list = |e: &CellEdit| Field::ALL[e.column].choices().is_some();
            if self
                .view
                .field_edit
                .as_ref()
                .is_some_and(|e| list(e) && (e.row, e.column) != at)
            {
                self.view.field_edit = None;
            }
        }
        if let Some(at) = events.double_clicked {
            self.edit_field(at);
        }
        if cancel {
            self.view.field_edit = None;
        }
        if commit && let Some(e) = self.view.field_edit.take() {
            let data = window_data(&self.session, &self.windows, self.here).expect("checked above");
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
        let Some(data) = window_data(&self.session, &self.windows, self.here) else {
            return;
        };
        let variable = data.variable_at(row);
        let field = Field::ALL[column];
        // the other variables of the same type whose rows are selected can get the same labels
        let rows = match &self.view.variable_selected {
            Some(s) if s.rows.contains(&row) => &s.rows,
            _ => &self.view.rows_before_click,
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
                self.view.format_dialog = Some(edits::FormatChoice::of(&variable));
                return;
            }
            Field::Values => {
                self.view.labels_dialog = Some(edits::LabelsEditor::of(&variable, others));
                return;
            }
            Field::Missing => {
                self.view.missing_dialog = Some(edits::MissingEditor::of(&variable, others));
                return;
            }
            _ => {}
        }
        self.view.variable_selected = Some(grid::Selection::cell((row, column)));
        if field.editable(variable.is_string()) {
            let text = edits::field_text(&variable, field);
            self.view.field_edit = Some(CellEdit {
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

    fn dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        // opening a file needs no data
        let empty;
        let data = match window_data(&self.session, &self.windows, self.popups) {
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
        let import = matches!(dialog, Dialog::ImportData(_));
        let mut syntax = dialog.syntax(data);
        let raw = syntax.clone();
        if import {
            // with a dataset name, as a data file opened from the window gets
            syntax = syntax.map(|text| self.session.open_syntax(&text));
        }
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
                match (import, raw) {
                    (true, Ok(raw)) => self.open_named(raw, After::CloseDialog(title)),
                    _ => self.run(&text, None, After::CloseDialog(title)),
                }
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

impl App {
    /// One window, in the root viewport or its own. A data window lends its view to
    /// `self.view` while it is drawn.
    fn window(&mut self, ui: &mut Ui, id: WindowId) {
        if let Some(w) = self.windows.data_mut(id) {
            std::mem::swap(&mut self.view, &mut w.view);
        }
        self.draw(ui, id);
        match self.windows.data_mut(id) {
            Some(w) => std::mem::swap(&mut self.view, &mut w.view),
            // the window closed while it was drawn
            None => self.view = DataView::default(),
        }
    }

    /// Every window has the menu bar, the status bar and its question about unsaved work; the
    /// dialogs show in the window they were opened from.
    fn draw(&mut self, ui: &mut Ui, id: WindowId) {
        let Some(kind) = self.windows.get(id).map(|w| WindowKind::of(&w.kind)) else {
            return;
        };
        self.here = id;
        let data = kind == WindowKind::Data;
        let (outer, inner, focused, close, pressed) = ui.input(|i| {
            let v = i.viewport();
            let pressed = i.pointer.any_pressed();
            (
                v.outer_rect,
                v.inner_rect,
                v.focused,
                v.close_requested(),
                pressed,
            )
        });
        if let Some(w) = self.windows.get_mut(id) {
            w.position = outer.map(|r| r.min);
            w.size = inner.map(|r| r.size());
        }
        if focused == Some(true) {
            self.windows.focused = Some(id);
            // the size and position a window of its kind opens with (no position on Wayland)
            let (size, position) = self.settings.window_mut(kind.name());
            if let Some(inner) = inner {
                *size = Some([inner.width(), inner.height()]);
            }
            if let Some(outer) = outer {
                *position = Some([outer.min.x, outer.min.y]);
            }
        }
        // a click anywhere in a data window makes its dataset the active one
        if data && pressed {
            self.click_activate(id);
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
                "Drop data, workbook, text, .sps or .scout files to open them",
                FontId::proportional(20.0),
                Color32::WHITE,
            );
        }
        if data {
            self.find_keys(ui);
            self.grid_keys(ui);
        }
        // Ctrl+Z inside a text field is that field's own undo
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z))
        {
            self.undo();
        }
        let mut run = None;
        egui::Panel::top(("menu", id)).show(ui, |ui| {
            self.menu(ui);
            match kind {
                WindowKind::Data => {
                    ui.add_space(2.0);
                    self.tabs(ui);
                }
                WindowKind::Syntax => {
                    ui.add_space(2.0);
                    run = self.syntax_toolbar(ui, id);
                }
                WindowKind::Output => {}
            }
        });
        if id == self.windows.root() && self.updates.visible() {
            egui::Panel::top("update").show(ui, |ui| self.updates.bar(ui));
        }
        egui::Panel::bottom(("status", id)).show(ui, |ui| self.status_bar(ui));
        // the log shows under the syntax and the output; the grids get the whole height
        if self.show_log && !data {
            egui::Panel::bottom(("log", id))
                .resizable(true)
                .default_size(150.0)
                .show(ui, |ui| self.log(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            // the scroll positions and selections of each window are its own
            ui.push_id(("window", id), |ui| match kind {
                WindowKind::Data if self.view.tab == Tab::Variables => self.variable_view(ui),
                WindowKind::Data => self.data_view(ui),
                WindowKind::Syntax => {
                    if let Some(w) = self.windows.syntax_mut(id) {
                        let data = self.session.dataset();
                        if w.ui(ui, id, data, &self.macros, &self.settings) {
                            run = Some(true);
                        }
                    }
                }
                WindowKind::Output => self.output_view(ui, id),
            });
        });
        if let Some(selection) = run
            && let Some(w) = self.windows.syntax(id)
        {
            let lines = selection.then(|| w.selected_lines());
            let source = w.text.clone();
            self.run(&source, lines, After::Editor(id));
        }
        let ctx = ui.ctx().clone();
        if data {
            self.confirm_window(&ctx);
            self.format_window(&ctx);
            self.labels_window(&ctx);
            self.missing_window(&ctx);
        }
        if self.popups == id {
            self.popups(&ctx);
        }
        self.unsaved_window(&ctx);
        // the root's close requests come to `close_request`
        if close && id != self.windows.root() {
            self.close_window(id);
        }
    }

    /// The tables of an output window, with its navigator.
    fn output_view(&mut self, ui: &mut Ui, id: WindowId) {
        let light = !self.settings.paper_follows_system;
        let Some(o) = self.windows.output_mut(id) else {
            return;
        };
        o.view.light_paper = light;
        match o.view.ui(ui) {
            Some(output_view::Request::ExportHtml) => self.export_html(id, true),
            Some(output_view::Request::ExportXlsx { one_sheet }) => {
                self.export_xlsx(id, one_sheet, true)
            }
            Some(output_view::Request::SaveChart { chart, png, paper }) => {
                self.save_chart(&chart, png, paper)
            }
            Some(output_view::Request::Status(status)) => self.status = status,
            Some(output_view::Request::Paste(syntax)) => self.paste(&syntax),
            Some(output_view::Request::Run(syntax)) => {
                self.run(&syntax, None, After::Show);
            }
            Some(output_view::Request::Delete) => self.delete_output(id),
            None => {}
        }
    }

    /// The toolbar of a syntax window: Run all, Run selection, the Active list and the file.
    /// Returns a run asked for: `true` for the selection.
    fn syntax_toolbar(&mut self, ui: &mut Ui, id: WindowId) -> Option<bool> {
        let mut run = None;
        let mut chosen = None;
        ui.horizontal(|ui| {
            if ui.button("Run all").clicked() {
                run = Some(false);
            }
            if ui
                .button("Run selection")
                .on_hover_text("Ctrl+Enter")
                .clicked()
            {
                run = Some(true);
            }
            ui.separator();
            ui.label("Active");
            let (names, active) = syntax_window::datasets(&self.session);
            ui.add_enabled_ui(!names.is_empty(), |ui| {
                ComboBox::from_id_salt(("active", id))
                    .selected_text(active.clone().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for name in names {
                            let is_active = active.as_ref() == Some(&name);
                            if ui.selectable_label(is_active, &name).clicked() && !is_active {
                                chosen = Some(name);
                            }
                        }
                    });
            });
            if let Some(file) = self.windows.syntax(id).and_then(|w| w.file.as_ref()) {
                ui.separator();
                ui.weak(file.display().to_string());
            }
        });
        if let Some(name) = chosen {
            self.run(&syntax_window::activate(&name), None, After::Show);
        }
        run
    }

    /// The dialogs, Settings and About, in the window they were opened from.
    fn popups(&mut self, ctx: &egui::Context) {
        self.dialog_window(ctx);
        let log_hidden = self.settings.log_hidden;
        let labels = settings_window::TableLabels {
            variables: self.session.setting("TVARS"),
            values: self.session.setting("TNUMBERS"),
        };
        let set = settings_window::show(
            ctx,
            &mut self.settings_section,
            &mut self.settings,
            &labels,
            &mut self.connect,
        );
        if let Some(syntax) = set {
            self.run(&syntax, None, After::Show);
        }
        if self.settings.log_hidden != log_hidden {
            self.show_log = !self.settings.log_hidden;
        }
        egui::Window::new("About Statscraft")
            .open(&mut self.about)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
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

    /// The root draws the first window of the registry, and every other window as an
    /// immediate viewport (see windows.rs).
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let root = self.windows.root();
        let title = format!("{} - Statscraft", self.window_name(root));
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        // eframe 0.37 paints the root outside its event loop while the root is hidden (its
        // first frame) or minimized. A window that has no native window yet cannot get one
        // there (eframe panics), so it opens on the first frame the root is shown. Windows
        // open already are drawn every frame, or eframe would drop them.
        let hidden = frame
            .winit_window()
            .is_some_and(|w| w.is_visible() == Some(false) || w.is_minimized() == Some(true));
        for id in self.windows.ids() {
            if id == root {
                continue;
            }
            let viewport = self.windows.viewport(id);
            let made = ui.input(|i| {
                let info = i.raw.viewports.get(&viewport);
                info.is_some_and(|v| v.inner_rect.is_some())
            });
            if hidden && !made {
                ui.ctx().request_repaint();
                continue;
            }
            let builder = self.builder(id);
            ui.ctx()
                .show_viewport_immediate(viewport, builder, |ui, _| self.window(ui, id));
        }
        self.window(ui, root);
    }
}

impl App {
    /// The native window of a window that is not the root. A new one opens with the size and
    /// position the last window of its kind had, a little lower and to the right for each
    /// window of that kind open already; once open, the window keeps what the user gives it.
    fn builder(&self, id: WindowId) -> egui::ViewportBuilder {
        let mut builder = egui::ViewportBuilder::default()
            .with_title(format!("{} - Statscraft", self.window_name(id)))
            // the same X11 class and Wayland app id as the root (main.rs)
            .with_app_id("statscraft")
            .with_min_inner_size([480.0, 320.0])
            .with_icon(Arc::clone(&self.icon));
        let Some(w) = self.windows.get(id) else {
            return builder;
        };
        if w.size.is_some() {
            return builder;
        }
        let kind = WindowKind::of(&w.kind);
        let (size, position) = self.settings.window(kind.name());
        builder = builder.with_inner_size(size.unwrap_or(kind.size()));
        if let Some([x, y]) = position {
            let open = self.windows.iter().filter(|o| o.id != id);
            let others = open.filter(|o| WindowKind::of(&o.kind) == kind).count() as f32;
            builder = builder.with_position([x + 30.0 * others, y + 30.0 * others]);
        }
        builder
    }
}

/// The kinds of window, without what they hold.
#[derive(Clone, Copy, PartialEq, Debug)]
enum WindowKind {
    Data,
    Syntax,
    Output,
}

impl WindowKind {
    fn of(kind: &Kind) -> WindowKind {
        match kind {
            Kind::Data(_) => WindowKind::Data,
            Kind::Syntax(_) => WindowKind::Syntax,
            Kind::Output(_) => WindowKind::Output,
        }
    }

    /// Its name in the window settings.
    fn name(self) -> &'static str {
        match self {
            WindowKind::Data => "data",
            WindowKind::Syntax => "syntax",
            WindowKind::Output => "output",
        }
    }

    /// The size a window of this kind opens with the first time.
    fn size(self) -> [f32; 2] {
        match self {
            WindowKind::Data => [1100.0, 760.0],
            WindowKind::Syntax => [900.0, 680.0],
            WindowKind::Output => [1000.0, 760.0],
        }
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

/// The file a dataset was opened from or saved to, and the data as that left it.
type DataFile = (Option<PathBuf>, Option<Dataset>);

fn active_name(session: &Session) -> Option<String> {
    session.named_datasets().1.map(str::to_string)
}

/// The data of a dataset by name; None names the active dataset when it has no name.
fn dataset_named<'a>(session: &'a Session, name: Option<&str>) -> Option<&'a Dataset> {
    let (others, active) = session.named_datasets();
    if same(name, active) {
        return session.dataset();
    }
    let name = name?;
    let other = others.iter().find(|(n, _)| n.eq_ignore_ascii_case(name));
    other.and_then(|(_, data)| data.as_ref())
}

/// The data a window shows: a data window its dataset's, every other window the active one.
fn window_data<'a>(session: &'a Session, windows: &Registry, id: WindowId) -> Option<&'a Dataset> {
    match windows.data(id) {
        Some(w) => dataset_named(session, w.dataset.as_deref()),
        None => session.dataset(),
    }
}

/// The datasets that hold data, each of which has a data window: the active one first (None
/// when it has no name), then the other named ones.
fn shown_datasets(session: &Session) -> Vec<Option<String>> {
    let (others, active) = session.named_datasets();
    let active = session.dataset().map(|_| active.map(str::to_string));
    let others = others.iter().filter(|(_, data)| data.is_some());
    active
        .into_iter()
        .chain(others.map(|(name, _)| Some(name.clone())))
        .collect()
}

/// The syntax a click in the window of dataset `shown` runs: DATASET ACTIVATE, unless the
/// dataset is active or a click has asked for it already.
fn activation(shown: &str, active: Option<&str>, activating: Option<&str>) -> Option<String> {
    let busy = same(Some(shown), active) || same(Some(shown), activating);
    (!busy).then(|| syntax_window::activate(shown))
}

/// The syntax that closes the dataset `name` of a data window, with `active` the active
/// dataset when it runs and `other` another dataset with data. DATASET CLOSE of the active
/// dataset only takes its name away (the manual), so another dataset is activated first, or
/// NEW FILE empties the session when there is none. The active dataset without a name closes
/// the same way. None when the window shows no data.
fn close_data(
    name: Option<&str>,
    active: Option<&str>,
    has_data: bool,
    other: Option<&str>,
) -> Option<String> {
    let close = |n: &str| format!("DATASET CLOSE {n}.");
    match (name, other) {
        (Some(n), _) if !same(Some(n), active) => Some(close(n)),
        (Some(n), Some(o)) => Some(format!("{}\n{}", syntax_window::activate(o), close(n))),
        (Some(n), None) => Some(format!("{}\nNEW FILE.", close(n))),
        (None, _) if !has_data => None,
        (None, Some(o)) => Some(format!("NEW FILE.\n{}", syntax_window::activate(o))),
        (None, None) => Some("NEW FILE.".into()),
    }
}

/// The output of a run's commands as the output windows take it: per document a run label
/// (None for the origin of the run) and each command's heading, syntax and tables. A file
/// that OUTPUT OPEN read is a run of its own, labelled with the file.
type Groups = Vec<(
    Option<String>,
    Option<String>,
    Vec<(String, String, Vec<output_view::Shown>)>,
)>;

fn output_groups(outputs: Vec<CommandResult>, source: &str, from_editor: bool) -> Groups {
    let mut groups: Groups = Vec::new();
    for out in outputs {
        if let Some((file, items)) = out.opened {
            let commands = statscraft_engine::output_file::by_command(&items);
            groups.push((out.document, Some(file), commands));
            continue;
        }
        let heading = match (from_editor, out.first_line == out.last_line) {
            (false, _) => out.name,
            (true, true) => format!("{}, line {}", out.name, out.first_line),
            (true, false) => {
                format!("{}, lines {}-{}", out.name, out.first_line, out.last_line)
            }
        };
        let syntax = output::source_lines(source, out.first_line, out.last_line);
        // a chart keeps its data table: for Show data, Copy and a kind not drawn yet
        let mut tables: Vec<_> = out.tables.into_iter().map(|t| (t, None)).collect();
        let charts = out.charts.into_iter();
        tables.extend(charts.map(|chart| (chart.data_table(), Some(chart))));
        let command = (heading, syntax, tables);
        let group = groups
            .iter_mut()
            .find(|(d, label, _)| label.is_none() && *d == out.document);
        match group {
            Some((_, _, commands)) => commands.push(command),
            None => groups.push((out.document, None, vec![command])),
        }
    }
    groups
}

/// The output windows follow what OUTPUT MODIFY changed in their documents.
fn follow_modified(windows: &mut Registry, outputs: &[CommandResult]) {
    for modified in outputs.iter().flat_map(|out| &out.modified) {
        if let Some(id) = windows.with_document(&modified.document)
            && let Some(window) = windows.output_mut(id)
        {
            window.view.apply_modified(&modified.items);
        }
    }
}

fn output_document(session: &Session, name: &str) -> Option<statscraft_engine::OutputDocument> {
    let documents = session.output_documents().into_iter();
    documents
        .into_iter()
        .find(|d| same(Some(&d.name), Some(name)))
}

/// What closing the window of an output document would lose.
fn unsaved_output(session: &Session, name: &str) -> Option<String> {
    let document = output_document(session, name).filter(|d| d.unsaved)?;
    let file = document.file.as_deref().and_then(Path::file_name);
    Some(match file {
        Some(file) => format!("Output: {} ({})", document.name, file.to_string_lossy()),
        None => format!("Output: {}, not saved to a file yet", document.name),
    })
}

/// Deletes the selected tables of an output window from the session's document, then from
/// the window; neither changes when the document refuses.
fn delete_selected(
    session: &mut Session,
    window: &mut crate::windows::OutputWindow,
) -> Result<(), String> {
    let positions = window.view.selected_positions();
    session.delete_output(&window.document, &positions)?;
    window.view.delete_selected();
    Ok(())
}

/// The output window that shows the tables of a command: the window of their document, else
/// the designated output window (a document renamed or closed later in the same run).
fn output_window(windows: &Registry, document: Option<&str>) -> Option<WindowId> {
    document
        .and_then(|d| windows.with_document(d))
        .or_else(|| windows.designated_output())
}

/// The file and saved data when the active dataset changes from `old` to `new`: the current
/// ones are kept under the old name and the new dataset's come back. A name given to the
/// unnamed active dataset keeps its file.
fn switch_data(
    kept: &mut std::collections::HashMap<String, DataFile>,
    current: DataFile,
    old: Option<String>,
    new: Option<String>,
) -> DataFile {
    let restored = new.and_then(|name| kept.remove(&name));
    match old {
        Some(old) => {
            kept.insert(old, current);
            restored.unwrap_or_default()
        }
        None => restored.unwrap_or(current),
    }
}

/// The data when it differs from its file: when it is not what the last open or save left.
/// It is named by its file, else by its dataset name.
fn unsaved_data(
    data: Option<&Dataset>,
    data_saved: Option<&Dataset>,
    data_file: Option<&Path>,
    dataset: Option<&str>,
) -> Vec<String> {
    let file = data_file.and_then(Path::file_name);
    let name = match (file, dataset) {
        (Some(file), _) => file.to_string_lossy().into_owned(),
        (None, Some(dataset)) => format!("{dataset}, not saved to a file yet"),
        (None, None) => "not saved to a file yet".into(),
    };
    data.is_some_and(|d| Some(d) != data_saved)
        .then(|| format!("Data: {name}"))
        .into_iter()
        .collect()
}

/// A run whose last command is GET, GET DATA or SAVE leaves the data as that file holds it;
/// a DATASET NAME after it, as opening a file from the window adds, changes no data.
fn ends_on_file(commands: &[CommandResult]) -> bool {
    commands
        .iter()
        .rfind(|c| c.name != "DATASET NAME")
        .is_some_and(|c| matches!(c.name.as_str(), "GET" | "GET DATA" | "SAVE"))
}

/// What the status bar says about the warnings of the last run.
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
const OPENS: &str = "Statscraft opens .sav, .zsav, .xlsx, .xlsm, .xls, .csv, .txt, .tab, .dat, .sps and .scout files";

/// A workbook or text file, which opens through the Open Data dialog.
fn is_import_file(path: &Path) -> bool {
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    is_workbook(path) || matches!(extension.as_deref(), Some("csv" | "txt" | "tab" | "dat"))
}

fn is_output_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("scout"))
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
    /// Output files, each opening in an output window of its own.
    output: Vec<PathBuf>,
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
        } else if is_output_file(&path) {
            drop.output.push(path);
            continue;
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
    // the cells' full size in f64; it is never laid out, only the visible part is drawn
    let content = (
        *offsets.last().unwrap_or(&0.0),
        grid::row_top(rows, row_height),
    );
    let mut events = GridEvents::default();

    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    // scroll bars only where the cells do not fit, each taking room from the other's view
    let bar = 10.0;
    let fits = |room: f32, used: f64| used <= f64::from(room);
    let mut vertical = !fits(rect.height() - header_height, content.1);
    let horizontal = !fits(
        rect.width() - number_width - vertical as u8 as f32 * bar,
        content.0,
    );
    vertical = vertical || !fits(rect.height() - header_height - bar, content.1) && horizontal;
    let view = Rect::from_min_max(
        rect.min,
        rect.max - vec2(vertical as u8 as f32 * bar, horizontal as u8 as f32 * bar),
    );
    let body = (
        f64::from((view.width() - number_width).max(0.0)),
        f64::from((view.height() - header_height).max(0.0)),
    );

    // the scroll is kept in f64 points: a million rows lie further down than f32 holds exactly
    let scroll_id = ui.make_persistent_id((id, "scroll"));
    let (mut sx, mut sy) = ui.data(|d| d.get_temp::<(f64, f64)>(scroll_id).unwrap_or_default());
    if ui.is_enabled() && ui.rect_contains_pointer(rect) {
        let delta = ui.input(|i| i.smooth_scroll_delta);
        if delta != egui::Vec2::ZERO {
            sx -= f64::from(delta.x);
            sy -= f64::from(delta.y);
            ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        }
    }
    if follow && let Some((r, c)) = selected.map(|s| s.cursor) {
        // in the cells' own points: the header and the row numbers lie outside them
        let top = grid::row_top(r.min(rows.saturating_sub(1)), row_height);
        sy = grid::scroll_to(top, top + f64::from(row_height), body.1, sy);
        let c = c.min(widths.len().saturating_sub(1));
        if c + 1 < offsets.len() {
            sx = grid::scroll_to(offsets[c], offsets[c + 1], body.0, sx);
        }
    }
    if vertical {
        let track = Rect::from_min_max(
            pos2(view.right(), rect.top()),
            pos2(rect.right(), view.bottom()),
        );
        scroll_bar(
            ui,
            scroll_id.with("v"),
            track,
            true,
            &mut sy,
            content.1,
            body.1,
        );
    }
    if horizontal {
        let track = Rect::from_min_max(
            pos2(rect.left(), view.bottom()),
            pos2(view.right(), rect.bottom()),
        );
        scroll_bar(
            ui,
            scroll_id.with("h"),
            track,
            false,
            &mut sx,
            content.0,
            body.0,
        );
    }
    sx = grid::clamp_scroll(sx, content.0, body.0);
    sy = grid::clamp_scroll(sy, content.1, body.1);
    ui.data_mut(|d| d.insert_temp(scroll_id, (sx, sy)));

    let clip = view.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    let visuals = ui.visuals().clone();
    let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    let header_fill = visuals.widgets.noninteractive.weak_bg_fill;
    let visible_rows = grid::visible_rows(sy, sy + body.1, row_height, rows);
    let visible_columns = grid::visible_columns(&offsets, sx, sx + body.0);
    // every position is taken relative to the view, so it stays a small f32
    let cells = view.min + vec2(number_width, header_height);
    let x = |c: usize| cells.x + grid::on_screen(offsets[c], sx);
    let y = |r: usize| cells.y + grid::on_screen(grid::row_top(r, row_height), sy);
    let cell_rect = |r: usize, c: usize| {
        Rect::from_min_max(pos2(x(c), y(r)), pos2(x(c + 1), y(r) + row_height))
    };
    // a text too wide for its cell shows its start and ends in an ellipsis, so a number
    // cut short never reads as a different number, and it keeps the alignment of its
    // column: a right-aligned column stays flush at the right
    let text_in =
        |painter: &egui::Painter, rect: Rect, text: String, align: Align, color: Color32| {
            let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
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
                (Some(text), align) => text_in(&painter, rect, text, align, visuals.text_color()),
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
    let bottom = y(visible_rows.end).min(clip.bottom());
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
    if let Some((r, c)) = editing
        && visible_rows.contains(&r)
        && visible_columns.contains(&c)
    {
        ui.scope_builder(UiBuilder::new().max_rect(view), |ui| {
            ui.set_clip_rect(clip);
            editor(ui, cell_rect(r, c));
        });
    }

    // the header row and the row numbers stay in view while the cells scroll
    let (top, side) = (view.top(), view.left());
    let header = Rect::from_min_max(view.min, pos2(view.right(), top + header_height));
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
        let border =
            Rect::from_center_size(pos2(x(c + 1), rect.center().y), vec2(8.0, header_height))
                .intersect(clip);
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
    painter.hline(view.x_range(), top + header_height, line);
    let numbers = Rect::from_min_max(
        pos2(side, top + header_height),
        pos2(side + number_width, bottom),
    );
    painter.rect_filled(numbers, 0.0, header_fill);
    // a row scrolled half under the header shows only the part below it
    let number_painter = painter.with_clip_rect(numbers.intersect(clip));
    for r in visible_rows.clone() {
        let rect = Rect::from_min_max(
            pos2(side, y(r)),
            pos2(side + number_width, y(r) + row_height),
        );
        if selected.is_some_and(|s| s.rows.contains(&r)) {
            number_painter.rect_filled(rect, 0.0, visuals.selection.bg_fill);
        }
        text_in(
            &number_painter,
            rect,
            (r + 1).to_string(),
            Align::Max,
            visuals.weak_text_color(),
        );
        if struck(r) {
            let shrunk = rect.shrink2(vec2(6.0, 3.0));
            let stroke = Stroke::new(1.5, visuals.text_color());
            number_painter.line_segment([shrunk.left_bottom(), shrunk.right_top()], stroke);
        }
    }
    painter.vline(side + number_width, clip.top()..=bottom, line);
    painter.rect_filled(
        Rect::from_min_size(view.min, vec2(number_width, header_height)),
        0.0,
        header_fill,
    );

    let sizes = (number_width, header_height, row_height);
    let at = |pos: egui::Pos2| grid::hit(pos - view.min, (sx, sy), sizes, &offsets, rows);
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
    events
}

/// A scroll bar along `track` for cells `content` points long in a view `view` long: drag the
/// thumb to move, or click beside it to move a page.
fn scroll_bar(
    ui: &mut Ui,
    id: egui::Id,
    track: Rect,
    vertical: bool,
    scroll: &mut f64,
    content: f64,
    view: f64,
) {
    let along = |v: egui::Vec2| if vertical { v.y } else { v.x };
    let length = along(track.size());
    let thumb = (f64::from(length) * view / content)
        .clamp(20.0_f64.min(f64::from(length)), f64::from(length)) as f32;
    let (room, range) = (f64::from(length - thumb), (content - view).max(0.0));
    let start = if range > 0.0 {
        (*scroll / range * room) as f32
    } else {
        0.0
    };
    let thumb_rect = if vertical {
        Rect::from_min_size(track.min + vec2(0.0, start), vec2(track.width(), thumb))
    } else {
        Rect::from_min_size(track.min + vec2(start, 0.0), vec2(thumb, track.height()))
    };
    let response = ui.interact(track, id, Sense::click_and_drag());
    if response.dragged() && room > 0.0 {
        *scroll += f64::from(along(response.drag_delta())) / room * range;
    } else if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let at = along(pointer.to_vec2());
        if at < along(thumb_rect.min.to_vec2()) {
            *scroll -= view;
        } else if at > along(thumb_rect.max.to_vec2()) {
            *scroll += view;
        }
    }
    *scroll = scroll.clamp(0.0, range);
    let visuals = ui.visuals();
    let painter = ui.painter();
    painter.rect_filled(track, 0.0, visuals.extreme_bg_color);
    let fill = ui.style().interact(&response).bg_fill;
    painter.rect_filled(thumb_rect.shrink(2.0), 3.0, fill);
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
    fn unsaved_data_is_listed() {
        let mut session = open("survey_basic.sav");
        let saved = session.dataset().cloned();
        let file = Some(Path::new("/d/survey.sav"));
        let data = |session: &Session| unsaved_data(session.dataset(), saved.as_ref(), file, None);
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
        let never = unsaved_data(session.dataset(), None, None, None);
        assert_eq!(never, ["Data: not saved to a file yet"]);
        let named = unsaved_data(session.dataset(), None, None, Some("DataSet2"));
        assert_eq!(named, ["Data: DataSet2, not saved to a file yet"]);
        // no data at all: nothing to save
        assert!(unsaved_data(None, saved.as_ref(), file, None).is_empty());
    }

    /// The windows a session's datasets and documents give, as `follow_session` makes them.
    fn follow(r: &mut Registry, session: &Session, was: Option<&str>) {
        let now = active_name(session);
        r.sync_data(&shown_datasets(session), was, now.as_deref());
        let documents: Vec<String> = session
            .output_documents()
            .into_iter()
            .map(|d| d.name)
            .collect();
        r.sync_output(&documents, session.designated_output(), None);
    }

    fn opened(session: &mut Session, file: &str) {
        let get = format!(
            "GET FILE={}.",
            quote(&crate::dialogs::testing::fixture(file))
        );
        let syntax = session.open_syntax(&get);
        run_in(session, &syntax);
    }

    #[test]
    fn a_click_in_a_data_window_writes_dataset_activate_once() {
        let mut session = Session::default();
        let mut r = Registry::new();
        opened(&mut session, "survey_basic.sav");
        follow(&mut r, &session, None);
        opened(&mut session, "survey_basic.sav");
        follow(&mut r, &session, Some("DataSet1"));
        let first = r.with_dataset(Some("DataSet1")).unwrap();
        let second = r.with_dataset(Some("DataSet2")).unwrap();
        assert_eq!(r.ids(), [first, second]);
        // each window shows its own dataset, active or not
        assert_eq!(active_name(&session).as_deref(), Some("DataSet2"));
        assert!(window_data(&session, &r, first).is_some());
        // a click in the active dataset's window runs nothing
        assert_eq!(activation("DataSet2", Some("DataSet2"), None), None);
        // the first click in the other one activates it; more clicks while it runs add nothing
        let syntax = activation("DataSet1", Some("DataSet2"), None).unwrap();
        assert_eq!(syntax, "DATASET ACTIVATE DataSet1.");
        assert_eq!(
            activation("DataSet1", Some("DataSet2"), Some("DataSet1")),
            None
        );
        run_in(&mut session, &syntax);
        follow(&mut r, &session, Some("DataSet2"));
        assert_eq!(activation("DataSet1", Some("DataSet1"), None), None);
        let log: Vec<&str> = session.log.iter().map(|e| e.syntax.as_str()).collect();
        let activations = log.iter().filter(|s| s.starts_with("DATASET ACTIVATE"));
        assert_eq!(activations.count(), 1);
        // the windows stay as they were
        assert_eq!(r.ids(), [first, second]);
    }

    #[test]
    fn closing_a_data_window_closes_its_dataset_and_leaves_the_others() {
        let mut session = Session::default();
        let mut r = Registry::new();
        opened(&mut session, "survey_basic.sav");
        follow(&mut r, &session, None);
        opened(&mut session, "survey_basic.sav");
        follow(&mut r, &session, Some("DataSet1"));
        // a dataset that is not active: DATASET CLOSE alone
        let other = close_data(Some("DataSet1"), Some("DataSet2"), true, Some("DataSet2"));
        assert_eq!(other.as_deref(), Some("DATASET CLOSE DataSet1."));
        // the active one: another is activated first, as closing the active dataset only
        // takes its name away
        let syntax = close_data(Some("DataSet2"), Some("DataSet2"), true, Some("DataSet1"));
        let syntax = syntax.unwrap();
        assert_eq!(
            syntax,
            "DATASET ACTIVATE DataSet1.\nDATASET CLOSE DataSet2."
        );
        run_in(&mut session, &syntax);
        assert_eq!(shown_datasets(&session), [Some("DataSet1".to_string())]);
        // no warning about data lost
        assert!(session.log.last().unwrap().warnings.is_empty());
        follow(&mut r, &session, Some("DataSet2"));
        assert_eq!(r.iter().count(), 1);
        // the last dataset: NEW FILE empties the session, and the window stays, empty
        let syntax = close_data(Some("DataSet1"), Some("DataSet1"), true, None).unwrap();
        assert_eq!(syntax, "DATASET CLOSE DataSet1.\nNEW FILE.");
        run_in(&mut session, &syntax);
        assert!(shown_datasets(&session).is_empty());
        follow(&mut r, &session, Some("DataSet1"));
        assert_eq!(r.data(r.root()).unwrap().dataset, None);
        // the active dataset without a name, from a GET typed in syntax
        let get = format!(
            "GET FILE={}.",
            quote(&crate::dialogs::testing::fixture("survey_basic.sav"))
        );
        run_in(&mut session, &get);
        assert_eq!(shown_datasets(&session), [None]);
        assert_eq!(
            close_data(None, None, true, None).as_deref(),
            Some("NEW FILE.")
        );
        let syntax = close_data(None, None, true, Some("DataSet3")).unwrap();
        assert_eq!(syntax, "NEW FILE.\nDATASET ACTIVATE DataSet3.");
        // an empty window closes without syntax
        assert_eq!(close_data(None, None, false, None), None);
    }

    #[test]
    fn tables_go_to_the_window_of_the_designated_document() {
        let mut session = open("survey_basic.sav");
        let mut r = Registry::new();
        let outputs = |r: &Registry| -> Vec<WindowId> {
            let ids = r.iter().map(|w| w.id);
            ids.filter(|&id| r.output(id).is_some()).collect()
        };
        let run = |session: &mut Session, syntax: &str| -> Vec<CommandResult> {
            session.run(syntax, None).unwrap().commands
        };
        let out = run(&mut session, "FREQUENCIES gender.");
        follow(&mut r, &session, None);
        let one = r.with_document("Document1").unwrap();
        assert_eq!(output_window(&r, out[0].document.as_deref()), Some(one));
        // OUTPUT NEW: a new window, which the next tables go to
        let out = run(&mut session, "OUTPUT NEW.\nFREQUENCIES region.");
        follow(&mut r, &session, None);
        let two = r.with_document("Document2").unwrap();
        assert_eq!(outputs(&r), [one, two]);
        assert_eq!(r.designated_output(), Some(two));
        assert_eq!(output_window(&r, out[1].document.as_deref()), Some(two));
        // Window > Designate on the first window runs OUTPUT ACTIVATE
        let out = run(
            &mut session,
            "OUTPUT ACTIVATE Document1.\nFREQUENCIES gender.",
        );
        follow(&mut r, &session, None);
        assert_eq!(r.designated_output(), Some(one));
        assert_eq!(output_window(&r, out[1].document.as_deref()), Some(one));
        // closing a window runs OUTPUT CLOSE, and the window goes
        run(&mut session, "OUTPUT CLOSE Document2.");
        follow(&mut r, &session, None);
        assert_eq!(r.with_document("Document2"), None);
        assert_eq!(outputs(&r), [one]);
        // a document renamed after its tables in the same run: the designated window shows them
        let out = run(&mut session, "FREQUENCIES gender.\nOUTPUT NAME men.");
        assert!(out.iter().any(|c| c.name == "OUTPUT NAME"));
        let documents: Vec<String> = session
            .output_documents()
            .into_iter()
            .map(|d| d.name)
            .collect();
        r.sync_output(&documents, session.designated_output(), Some("Document1"));
        assert_eq!(r.with_document("men"), Some(one));
        assert_eq!(output_window(&r, out[0].document.as_deref()), Some(one));
    }

    #[test]
    fn the_output_window_and_its_document_always_hold_the_same_items() {
        let dir = std::env::temp_dir().join(format!("statscraft-gui-scout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut session = open("survey_basic.sav");
        let mut r = Registry::new();
        // a run shown in the windows, as `show` does
        let show = |session: &mut Session, r: &mut Registry, syntax: &str| {
            let out = session.run(syntax, None).unwrap().commands;
            follow(r, session, None);
            follow_modified(r, &out);
            for (document, label, commands) in output_groups(out, syntax, false) {
                let id = output_window(r, document.as_deref()).unwrap();
                let view = &mut r.output_mut(id).unwrap().view;
                view.add_run(label.as_deref().unwrap_or("window"), commands);
            }
        };
        let titles = |session: &Session, r: &Registry, name: &str| {
            let window = r.output(r.with_document(name).unwrap()).unwrap();
            let shown = output::tables(&window.view.runs).map(|t| t.table.title.clone());
            let items = session.output_items(name).unwrap();
            let held = statscraft_engine::output_file::leaves(items).into_iter();
            let held: Vec<String> = held.map(|(_, item)| item.shown().0.title).collect();
            assert_eq!(shown.collect::<Vec<_>>(), held);
            held
        };
        show(
            &mut session,
            &mut r,
            "FREQUENCIES gender /BARCHART.\nDESCRIPTIVES age.\nCROSSTABS gender BY region.",
        );
        let all = titles(&session, &r, "Document1");
        assert!(all.len() >= 4, "{all:?}");
        assert_eq!(
            unsaved_output(&session, "Document1").as_deref(),
            Some("Output: Document1, not saved to a file yet")
        );
        // the second table and the last go from the window and the document
        let id = r.with_document("Document1").unwrap();
        let window = r.output_mut(id).unwrap();
        let ids: Vec<u64> = output::tables(&window.view.runs).map(|t| t.id).collect();
        window.view.selected = [ids[1], ids[ids.len() - 1]].into();
        delete_selected(&mut session, window).unwrap();
        let left = titles(&session, &r, "Document1");
        assert_eq!(left.len(), all.len() - 2);
        assert_eq!(left[0], all[0]);
        // saved: nothing to ask about when the window closes
        let file = dir.join("a.scout");
        let save = format!("OUTPUT SAVE OUTFILE={}.", quote(&file.to_string_lossy()));
        session.run(&save, None).unwrap();
        assert_eq!(unsaved_output(&session, "Document1"), None);
        // opened into a window of its own, as a run named after the file; new tables follow
        let open = format!(
            "OUTPUT OPEN FILE={} NAME=again.\nFREQUENCIES region.",
            quote(&file.to_string_lossy())
        );
        show(&mut session, &mut r, &open);
        let again = titles(&session, &r, "again");
        assert_eq!(again[..left.len()], left[..]);
        assert_eq!(again.len(), left.len() + 1);
        let window = r.output(r.with_document("again").unwrap()).unwrap();
        let labels: Vec<&str> = window
            .view
            .runs
            .iter()
            .map(|run| run.label.as_str())
            .collect();
        assert!(labels[0].ends_with("a.scout"), "{labels:?}");
        assert!(labels[1].ends_with("window"), "{labels:?}");
        assert_eq!(window.view.runs[0].items[0].heading, "FREQUENCIES");
        // OUTPUT MODIFY changes the window as it changes the document
        show(
            &mut session,
            &mut r,
            "OUTPUT MODIFY NAME=Document1 /SELECT TABLES /IF COMMANDS=[LAST] \
             /DELETEOBJECT DELETE=YES\n/SELECT TABLES /TABLE TABLETITLE='T)INDEX'\n\
             /SELECT HEADINGS /IF COMMANDS=['frequencies'] /OBJECTPROPERTIES OUTLINELABEL='First'.",
        );
        let changed = titles(&session, &r, "Document1");
        assert!(changed.len() < left.len(), "{changed:?}");
        assert_eq!(changed[0], "T1");
        let window = r.output(r.with_document("Document1").unwrap()).unwrap();
        assert_eq!(window.view.runs[0].items[0].heading, "First");
        // Clear output deletes everything from both
        let window = r.output_mut(r.with_document("again").unwrap()).unwrap();
        window.view.selected = output::tables(&window.view.runs).map(|t| t.id).collect();
        delete_selected(&mut session, window).unwrap();
        assert!(titles(&session, &r, "again").is_empty());
        // a window out of step with its document deletes nothing from either
        let window = r.output_mut(r.with_document("Document1").unwrap()).unwrap();
        window.view.selected = ids.iter().copied().collect();
        let before = output::tables(&window.view.runs).count();
        let positions = window.view.selected_positions();
        assert!(session.delete_output("Document1", &[before + 3]).is_err());
        assert_eq!(positions.len(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn each_named_dataset_keeps_its_own_file() {
        let mut kept = std::collections::HashMap::new();
        let file = |name: &str| (Some(PathBuf::from(name)), None::<Dataset>);
        let name = |n: &str| Some(n.to_string());
        // DATASET NAME on the unnamed data keeps its file
        let now = switch_data(&mut kept, file("a.sav"), None, name("DataSet1"));
        assert_eq!(now.0.as_deref(), Some(Path::new("a.sav")));
        // a second file opened: the first keeps its file under its name
        let now = switch_data(&mut kept, now, name("DataSet1"), name("DataSet2"));
        assert_eq!(now.0, None);
        // back to the first: its file, never the second's
        let now = switch_data(&mut kept, file("b.sav"), name("DataSet2"), name("DataSet1"));
        assert_eq!(now.0.as_deref(), Some(Path::new("a.sav")));
        let now = switch_data(&mut kept, now, name("DataSet1"), name("DataSet2"));
        assert_eq!(now.0.as_deref(), Some(Path::new("b.sav")));
    }

    #[test]
    fn a_run_ending_with_get_or_save_leaves_the_data_saved() {
        let commands = |names: &[&str]| -> Vec<CommandResult> {
            let command = |name: &&str| CommandResult {
                name: name.to_string(),
                first_line: 1,
                last_line: 1,
                tables: Vec::new(),
                charts: Vec::new(),
                warnings: Vec::new(),
                document: None,
                opened: None,
                modified: Vec::new(),
            };
            names.iter().map(command).collect()
        };
        assert!(ends_on_file(&commands(&["GET"])));
        assert!(ends_on_file(&commands(&["GET DATA"])));
        assert!(ends_on_file(&commands(&["COMPUTE", "SAVE"])));
        assert!(!ends_on_file(&commands(&["GET", "COMPUTE"])));
        assert!(!ends_on_file(&commands(&["SAVE TRANSLATE"])));
        assert!(!ends_on_file(&[]));
        assert!(ends_on_file(&commands(&["GET", "DATASET NAME"])));
        assert!(!ends_on_file(&commands(&["DATASET NAME"])));
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
