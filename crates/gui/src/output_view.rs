//! An output window: a navigator of runs, commands and every element of the output (headings,
//! titles, tables, notes and charts, each with its own visibility) beside the output itself.
//! Each table is measured once, and a table out of view only takes its place, so hundreds of
//! tables scroll smoothly.

use crate::i18n::{self, t, t_args};
use crate::office::{self, Program};
use crate::output::{self, Exported, OutputItem, OutputRun, OutputTable, Part, Show, Visibility};
use eframe::egui::{
    self, Align, Color32, FontId, Key, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, pos2, text::LayoutJob, vec2,
};
use statscraft_engine::live;
use statscraft_engine::office::is_office_chart;
use statscraft_engine::output_file::Shown as CommandShown;
use statscraft_engine::output_modify::ModifiedItem;
use statscraft_engine::plugin::Answer;
use statscraft_engine::{Chart, Decoration, Table, parse_colour};
use statscraft_export::picture::{self, Paper, Scene};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 4.0;
/// Row labels wider than this wrap.
const ROW_LABEL_MAX: f32 = 320.0;
/// Column headers wider than this wrap, between words.
const HEADER_MAX: f32 = 160.0;
/// Captions wrap at the table's width, or at this when the table is narrower.
const CAPTION_MIN: f32 = 480.0;
const TITLE_SIZE: f32 = 15.0;
/// Room for a command's heading above its first table.
const HEADING: f32 = 30.0;
/// Space below a table.
const GAP: f32 = 22.0;
/// Space around the output, so a table's border stands off the edge.
const MARGIN: f32 = 14.0;
/// The narrowest and widest a chart is drawn; between them it takes the window's width.
const CHART_WIDTH: (f32, f32) = (320.0, 760.0);

/// What an output window asks of the app.
pub enum Request {
    /// Open File > Export for the selected tables.
    Export,
    Status(Result<String, String>),
    /// Put the syntax in the syntax editor.
    Paste(String),
    /// Run the syntax again.
    Run(String),
    /// Delete the selected tables, from the engine's document first (`positions`), then from
    /// the view (`delete_selected`), so both always hold the same items.
    Delete,
    /// Save a chart as a picture, PNG or SVG, on this paper.
    SaveChart {
        chart: Box<Chart>,
        png: bool,
        paper: Paper,
    },
}

/// The context menu of a table.
#[derive(Clone, Copy)]
enum Menu {
    Copy,
    CopyText,
    Delete,
    /// The chart of the table the menu belongs to, in an office program.
    Open(Program),
    /// Show the chart as its data table, or as a picture again.
    ShowData,
    /// Save the chart as a picture: PNG or SVG.
    Save {
        png: bool,
    },
}

/// A row of the navigator, by the places of its run, command and table.
#[derive(Clone, Copy)]
enum Row {
    Run(usize),
    Command(usize, usize),
    Heading(usize, usize),
    Table(usize, usize, usize),
    /// A table's title or notes.
    Part(usize, usize, usize, Part),
}

/// An element shown or hidden in the navigator.
#[derive(Clone, Copy)]
enum Flip {
    /// A heading, title or notes, by the id of its command or table.
    Hidden((u64, Part)),
    /// A table or chart, by its id.
    Table(u64),
}

/// A navigator row of one element: its box, checked unless the element is hidden itself, and
/// its name, greyed when it does not show. Returns whether the box and the name were clicked.
fn element(
    ui: &mut Ui,
    indent: f32,
    mut own: bool,
    shown: bool,
    label: RichText,
    selected: bool,
) -> (bool, bool) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        let flipped = ui
            .add(egui::Checkbox::without_text(&mut own))
            .on_hover_text(t("output-show-element"))
            .changed();
        let label = if shown { label } else { label.weak().italics() };
        (flipped, ui.selectable_label(selected, label).clicked())
    })
    .inner
}

/// Where everything of one table goes, measured once.
struct Layout {
    widths: Vec<f32>,
    header_heights: Vec<f32>,
    /// The top of every body row below the header, and the bottom of the last.
    row_tops: Vec<f32>,
    /// How many rows each row label spans, see `output::row_spans`.
    spans: Vec<Vec<usize>>,
    title_height: f32,
    caption_height: f32,
    /// The table, or the title or caption when they are wider.
    block_width: f32,
}

impl Layout {
    fn width(&self) -> f32 {
        self.widths.iter().sum()
    }

    fn header_height(&self) -> f32 {
        self.header_heights.iter().sum()
    }

    fn table_height(&self) -> f32 {
        self.header_height() + self.row_tops.last().copied().unwrap_or(0.0)
    }

    /// The title and the caption when they show, and the table.
    fn block_height(&self, title: bool, notes: bool) -> f32 {
        let title = if title { self.title_height + 6.0 } else { 0.0 };
        let notes = if notes { self.caption_height } else { 0.0 };
        title + self.table_height() + 4.0 + notes
    }
}

#[derive(Default)]
pub struct OutputView {
    pub runs: Vec<OutputRun>,
    /// Runs that made tables so far, for their numbers.
    run_count: usize,
    next_id: u64,
    pub selected: HashSet<u64>,
    /// The runs folded in the navigator, by label.
    collapsed: HashSet<String>,
    layouts: HashMap<u64, Layout>,
    /// The body font size and scale the layouts were measured at.
    measured_at: (f32, f32),
    /// The table to bring into view, and in how many more frames: the first frame after new
    /// output still scrolls within the old height, so the next one scrolls again.
    scroll_to: Option<(u64, u8)>,
    /// Kept open: on Linux the copied text lives as long as the clipboard does.
    clipboard: Option<arboard::Clipboard>,
    /// The tables on light paper whatever the system theme.
    pub light_paper: bool,
    /// A table of the command whose syntax shows under the navigator.
    syntax_of: Option<u64>,
    /// Every chart laid out as a picture, or None for a kind that has no drawing yet.
    scenes: HashMap<u64, Option<Scene>>,
    /// The width, paper (dark or not) and kinds shown the scenes were laid out for.
    scenes_at: (f32, bool, Show),
    /// Charts shown as their data table instead of a picture.
    as_data: HashSet<u64>,
    /// The kinds of element shown, from the settings.
    pub show: Show,
    /// Headings, titles and notes hidden one by one, by the id of their command or table; a
    /// table's own visibility is `Table::hidden`.
    pub(crate) hidden: HashSet<(u64, Part)>,
}

impl OutputView {
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Adds the tables of one run, with a heading per command; returns whether there were any.
    /// Each command comes with its heading and the syntax that made it; a chart comes as its
    /// data table with the chart.
    pub fn add_run(&mut self, origin: &str, commands: Vec<CommandShown>) -> bool {
        let mut items = Vec::new();
        for command in commands {
            let (heading, syntax, tables) = (command.heading, command.syntax, command.leaves);
            if tables.is_empty() {
                continue;
            }
            self.next_id += 1;
            let id = self.next_id;
            if command.heading_hidden {
                self.hidden.insert((id, Part::Heading));
            }
            let tables = tables
                .into_iter()
                .map(|(table, chart)| {
                    self.next_id += 1;
                    OutputTable {
                        id: self.next_id,
                        table,
                        chart,
                    }
                })
                .collect();
            items.push(OutputItem {
                id,
                heading,
                syntax,
                tables,
            });
        }
        let Some(first) = items.first().map(|item: &OutputItem| item.tables[0].id) else {
            return false;
        };
        self.run_count += 1;
        self.runs.push(OutputRun {
            label: t_args(
                "core-run-label",
                &[("number", self.run_count.into()), ("origin", origin.into())],
            ),
            items,
        });
        self.scroll_to = Some((first, 2));
        true
    }

    /// Follows what OUTPUT MODIFY changed in the document this window shows: tables replaced,
    /// headings renamed and tables deleted, each found by its place among all the tables.
    pub fn apply_modified(&mut self, changes: &[ModifiedItem]) {
        let by_place: HashMap<usize, &ModifiedItem> =
            changes.iter().map(|m| (m.position, m)).collect();
        let mut gone = HashSet::new();
        let mut place = 0;
        for item in self.runs.iter_mut().flat_map(|run| &mut run.items) {
            for t in &mut item.tables {
                if let Some(m) = by_place.get(&place) {
                    if m.deleted {
                        gone.insert(t.id);
                    } else {
                        if let Some(heading) = &m.heading {
                            item.heading = heading.clone();
                        }
                        match m.heading_hidden {
                            Some(true) => self.hidden.insert((item.id, Part::Heading)),
                            Some(false) => self.hidden.remove(&(item.id, Part::Heading)),
                            None => false,
                        };
                        if let Some(table) = &m.table {
                            t.table = table.clone();
                            t.chart = m.chart.clone();
                        }
                        self.layouts.remove(&t.id);
                        self.scenes.remove(&t.id);
                    }
                }
                place += 1;
            }
        }
        output::retain(&mut self.runs, |id| !gone.contains(&id));
        self.selected.retain(|id| !gone.contains(id));
        self.layouts.retain(|id, _| !gone.contains(id));
        self.scenes.retain(|id, _| !gone.contains(id));
        self.as_data.retain(|id| !gone.contains(id));
    }

    /// The tables with cell statistics, with their ids, for the plugin: with `last` only the
    /// new ones of the last run that have no decorations yet.
    pub fn plugin_tables(&self, last: bool) -> Vec<(u64, Table)> {
        let runs = match last {
            true => &self.runs[self.runs.len().saturating_sub(1)..],
            false => &self.runs[..],
        };
        let wanted = |t: &&OutputTable| {
            t.table.cell_stats.is_some() && !(last && !t.table.decorations.is_empty())
        };
        let found = output::tables(runs).filter(wanted);
        found.map(|t| (t.id, t.table.clone())).collect()
    }

    /// Puts a plugin's decorations on the tables of the given ids and returns them by the
    /// places of their tables, as the document counts them (see `selected_positions`).
    pub fn decorate(&mut self, answer: Vec<(u64, Answer)>) -> Vec<(usize, Answer)> {
        let mut by_id: HashMap<u64, Answer> = answer.into_iter().collect();
        let mut placed = Vec::new();
        let all = self.runs.iter_mut().flat_map(|run| &mut run.items);
        for (place, t) in all.flat_map(|item| &mut item.tables).enumerate() {
            if let Some((decorations, style)) = by_id.remove(&t.id) {
                t.table.set_decorations(decorations, style);
                let set = (t.table.decorations.clone(), t.table.style.clone());
                placed.push((place, set));
            }
        }
        placed
    }

    pub fn delete_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        output::retain(&mut self.runs, |id| !selected.contains(&id));
        self.layouts.retain(|id, _| !selected.contains(id));
        self.scenes.retain(|id, _| !selected.contains(id));
        self.as_data.retain(|id| !selected.contains(id));
    }

    /// What the window shows.
    fn visibility(&self) -> Visibility<'_> {
        Visibility {
            show: self.show,
            hidden: &self.hidden,
            all: false,
        }
    }

    /// Everything this window holds for a ReadOutput of `document`: each command and table
    /// with what hides or selects it, and the kinds of element the window shows.
    pub fn contents(&self, document: &str) -> live::OutputContents {
        let show = |part| self.show.kind(part);
        let commands = self.runs.iter().flat_map(|run| &run.items);
        let commands = commands.map(|item| live::OutputCommand {
            id: item.id,
            heading: item.heading.clone(),
            hidden: self.hidden.contains(&(item.id, Part::Heading)),
            items: item
                .tables
                .iter()
                .map(|t| live::OutputItem {
                    id: t.id,
                    hidden: t.table.hidden,
                    selected: self.selected.contains(&t.id),
                    // a chart goes as a run's result gives it, without its data table
                    table: t.chart.is_none().then(|| t.table.clone()),
                    chart: t.chart.clone(),
                })
                .collect(),
        });
        live::OutputContents {
            document: document.to_string(),
            shows: live::OutputShows {
                headings: show(Part::Heading),
                titles: show(Part::Title),
                tables: show(Part::Table),
                notes: show(Part::Notes),
                charts: show(Part::Chart),
            },
            commands: commands.collect(),
        }
    }

    /// What exports and copies take: what shows, or everything with "Export hidden items".
    pub fn exported(&self) -> Visibility<'_> {
        Visibility {
            all: self.show.export_hidden,
            ..self.visibility()
        }
    }

    /// The paper the charts were drawn on last.
    fn paper(&self) -> Paper {
        match self.scenes_at.1 {
            true => Paper::Dark,
            false => Paper::Light,
        }
    }

    /// The places of the selected tables among all, in order: as the engine counts the items
    /// of the document (`output_file::leaves`).
    pub fn selected_positions(&self) -> Vec<usize> {
        let all = output::tables(&self.runs).enumerate();
        all.filter(|(_, t)| self.selected.contains(&t.id))
            .map(|(i, _)| i)
            .collect()
    }

    /// Puts the selected tables on the clipboard: as rich text with plain text beside it, or
    /// as tab-separated text only. One chart alone goes, where the system allows, as an
    /// editable office chart with its data as text beside it. What does not show stays out,
    /// as in exports.
    fn copy(&mut self, ui: &Ui, rich: bool) -> Result<String, String> {
        let shown = self.exported();
        let copied: Vec<Exported> = output::tables(&self.runs)
            .filter(|t| self.selected.contains(&t.id) && shown.table(t))
            .map(|t| shown.shown(t))
            .collect();
        if copied.is_empty() {
            return Err(t("output-selected-hidden").into());
        }
        let tables: Vec<&Table> = copied.iter().map(|(t, ..)| t).collect();
        let text: Vec<String> = tables.iter().map(|t| output::tsv(t)).collect();
        let text = text.join("\n");
        let chart = match &copied[..] {
            [(_, Some(chart), _)] => Some(chart),
            _ => None,
        };
        if let Some(chart) = chart.filter(|_| rich) {
            match office::copy(chart, &text, self.paper()) {
                Ok(true) => return Ok(t("output-chart-copied").into()),
                Ok(false) => {}
                Err(e) => return Err(e),
            }
        }
        let what = [("count", tables.len().into())];
        if !rich {
            ui.ctx().copy_text(text);
            return Ok(t_args("output-copied-as-text", &what));
        }
        let html = output::clipboard_html(&tables);
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        match self
            .clipboard
            .as_mut()
            .map(|c| c.set_html(&html, Some(&text)))
        {
            Some(Ok(())) => Ok(t_args("output-copied", &what)),
            _ => {
                ui.ctx().copy_text(text);
                Err(t_args("output-copied-no-rich-text", &what))
            }
        }
    }

    /// Selects the tables, or with Ctrl adds them to the selection (or takes them out when all
    /// are in it already). A click on the selection as it is takes it away.
    fn pick(&mut self, ids: &[u64], ctrl: bool) {
        self.syntax_of = ids.first().copied();
        let same =
            self.selected.len() == ids.len() && ids.iter().all(|id| self.selected.contains(id));
        if !ctrl && same {
            self.selected.clear();
        } else if !ctrl {
            self.selected.clear();
            self.selected.extend(ids);
        } else if ids.iter().all(|id| self.selected.contains(id)) {
            ids.iter().for_each(|id| {
                self.selected.remove(id);
            });
        } else {
            self.selected.extend(ids);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Option<Request> {
        if self.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.heading(t("output-none-yet"));
            });
            return None;
        }
        let mut request = None;
        let some = !self.selected.is_empty();
        // keys when no text field has them: Ctrl+C copies, Delete deletes, Ctrl+A selects all
        if !ui.ctx().text_edit_focused() {
            let (copy, delete, all, escape) = ui.input_mut(|i| {
                (
                    i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                    i.consume_key(egui::Modifiers::NONE, Key::Delete),
                    i.consume_key(egui::Modifiers::COMMAND, Key::A),
                    i.key_pressed(Key::Escape),
                )
            });
            if escape {
                self.selected.clear();
            }
            if copy && some {
                request = Some(Request::Status(self.copy(ui, true)));
            }
            if delete && some {
                request = Some(Request::Delete);
            }
            if all {
                self.selected = output::tables(&self.runs).map(|t| t.id).collect();
            }
        }
        ui.horizontal(|ui| {
            let some = !self.selected.is_empty();
            if ui
                .add_enabled(some, egui::Button::new(t("output-copy")))
                .on_hover_text(t("output-copy-hint"))
                .clicked()
            {
                request = Some(Request::Status(self.copy(ui, true)));
            }
            if ui
                .add_enabled(some, egui::Button::new(t("output-copy-as-text")))
                .on_hover_text(t("output-copy-as-text-hint"))
                .clicked()
            {
                request = Some(Request::Status(self.copy(ui, false)));
            }
            if ui
                .add_enabled(some, egui::Button::new(t("output-delete")))
                .on_hover_text(t("output-delete-hint"))
                .clicked()
            {
                request = Some(Request::Delete);
            }
            if ui
                .add_enabled(some, egui::Button::new(t("menu-export")))
                .on_hover_text(t("output-export-hint"))
                .clicked()
            {
                request = Some(Request::Export);
            }
            ui.separator();
            if ui.button(t("output-select-all")).clicked() {
                self.selected = output::tables(&self.runs).map(|t| t.id).collect();
            }
            if ui.button(t("output-clear")).clicked() {
                self.selected = output::tables(&self.runs).map(|t| t.id).collect();
                request = Some(Request::Delete);
            }
            match self.selected.len() {
                0 => {}
                n => {
                    ui.weak(t_args("output-selected", &[("count", n.into())]));
                }
            }
        });
        // Ctrl as it was at the click, which may have ended within the same frame
        let ctrl = ui.input(|i| {
            i.modifiers.command
                || i.events.iter().any(|e| {
                    matches!(e, egui::Event::PointerButton { modifiers, .. } if modifiers.command)
                })
        });
        egui::Panel::left("navigator")
            .resizable(true)
            .default_size(240.0)
            .max_size(480.0)
            .show(ui, |ui| {
                if let Some(r) = self.item_syntax(ui) {
                    request = Some(r);
                }
                self.navigator(ui, ctrl);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            crate::app::paper(ui, self.light_paper);
            if let Some(r) = self.tables(ui, ctrl) {
                request = Some(r);
            }
        });
        request
    }

    /// The syntax of the command picked last, under the navigator, to copy, paste or run again.
    fn item_syntax(&mut self, ui: &mut Ui) -> Option<Request> {
        let item = output::item_of(&self.runs, self.syntax_of?)?;
        let mut request = None;
        egui::Panel::bottom("item_syntax")
            .resizable(true)
            .default_size(150.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.weak(t("dialog-syntax"));
                    if ui.small_button(t("output-copy")).clicked() {
                        ui.ctx().copy_text(item.syntax.clone());
                        request = Some(Request::Status(Ok(t("output-syntax-copied").into())));
                    }
                    if ui.small_button(t("button-paste")).clicked() {
                        request = Some(Request::Paste(item.syntax.clone()));
                    }
                    if ui.small_button(t("output-run-again")).clicked() {
                        request = Some(Request::Run(item.syntax.clone()));
                    }
                });
                ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(&item.syntax).monospace()).extend());
                });
            });
        request
    }

    /// One row per run, command and element (heading, table or chart, title, notes), of which
    /// only the rows in view are drawn. Each element has a box that shows or hides it.
    fn navigator(&mut self, ui: &mut Ui, ctrl: bool) {
        // long titles are cut short here; the tables show them whole
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let mut rows: Vec<Row> = Vec::new();
        for (r, run) in self.runs.iter().enumerate() {
            rows.push(Row::Run(r));
            if self.collapsed.contains(&run.label) {
                continue;
            }
            for (i, item) in run.items.iter().enumerate() {
                rows.extend([Row::Command(r, i), Row::Heading(r, i)]);
                for (t, table) in item.tables.iter().enumerate() {
                    rows.extend([Row::Table(r, i, t), Row::Part(r, i, t, Part::Title)]);
                    if table.table.caption.is_some() {
                        rows.push(Row::Part(r, i, t, Part::Notes));
                    }
                }
            }
        }
        let mut picked: Option<Vec<u64>> = None;
        let mut toggle = None;
        let mut flip = None;
        let shown = self.visibility();
        let height = ui.spacing().interact_size.y;
        ScrollArea::vertical()
            .auto_shrink(false)
            .show_rows(ui, height, rows.len(), |ui, range| {
                for &row in &rows[range] {
                    let (r, i) = match row {
                        Row::Run(r) => {
                            let run = &self.runs[r];
                            let open = !self.collapsed.contains(&run.label);
                            let mark = if open { "\u{25be}" } else { "\u{25b8}" };
                            let text = RichText::new(format!("{mark} {}", run.label)).weak();
                            if ui.selectable_label(false, text).clicked() {
                                toggle = Some(run.label.clone());
                            }
                            continue;
                        }
                        Row::Command(r, i) | Row::Heading(r, i) => (r, i),
                        Row::Table(r, i, _) | Row::Part(r, i, ..) => (r, i),
                    };
                    let item = &self.runs[r].items[i];
                    let ids = || item.tables.iter().map(|t| t.id).collect::<Vec<u64>>();
                    let all = item.tables.iter().all(|t| self.selected.contains(&t.id));
                    match row {
                        Row::Run(_) => {}
                        Row::Command(..) => {
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let heading = RichText::new(&item.heading).strong();
                                let row = ui.selectable_label(all, heading);
                                let row = row.on_hover_ui(|ui| {
                                    ui.label(RichText::new(&item.syntax).monospace());
                                });
                                if row.clicked() {
                                    picked = Some(ids());
                                }
                            });
                        }
                        Row::Heading(..) => {
                            let key = (item.id, Part::Heading);
                            let own = !self.hidden.contains(&key);
                            let label = RichText::new(t("output-heading"));
                            let (flipped, clicked) =
                                element(ui, 22.0, own, shown.heading(item), label, all);
                            flip = flip.or(flipped.then_some(Flip::Hidden(key)));
                            if clicked {
                                picked = Some(ids());
                            }
                        }
                        Row::Table(.., t) => {
                            let t = &item.tables[t];
                            let on = self.selected.contains(&t.id);
                            // the outline label OUTPUT MODIFY gave
                            let label = t.table.outline.as_deref().unwrap_or(&t.table.title);
                            let own = !t.table.hidden;
                            let label = RichText::new(label);
                            let (flipped, clicked) =
                                element(ui, 22.0, own, shown.table(t), label, on);
                            flip = flip.or(flipped.then_some(Flip::Table(t.id)));
                            if clicked {
                                picked = Some(vec![t.id]);
                            }
                        }
                        Row::Part(.., t, part) => {
                            let t = &item.tables[t];
                            let on = self.selected.contains(&t.id);
                            let key = (t.id, part);
                            let own = !self.hidden.contains(&key);
                            let label = match part {
                                Part::Title => i18n::t("output-title"),
                                _ => i18n::t("output-notes"),
                            };
                            let label = RichText::new(label);
                            let (flipped, clicked) =
                                element(ui, 46.0, own, shown.part(t, part), label, on);
                            flip = flip.or(flipped.then_some(Flip::Hidden(key)));
                            if clicked {
                                picked = Some(vec![t.id]);
                            }
                        }
                    }
                }
            });
        if let Some(label) = toggle
            && !self.collapsed.remove(&label)
        {
            self.collapsed.insert(label);
        }
        match flip {
            Some(Flip::Hidden(key)) => {
                if !self.hidden.remove(&key) {
                    self.hidden.insert(key);
                }
                // a chart's picture holds its title and notes
                self.scenes.remove(&key.0);
            }
            Some(Flip::Table(id)) => {
                let all = self.runs.iter_mut().flat_map(|run| &mut run.items);
                let t = all.flat_map(|item| &mut item.tables).find(|t| t.id == id);
                t.into_iter().for_each(|t| t.table.hidden = !t.table.hidden);
            }
            None => {}
        }
        if let Some(ids) = picked {
            self.scroll_to = ids.first().map(|&id| (id, 2));
            self.pick(&ids, ctrl);
        }
    }

    /// The tables one below the other, each under its command's heading when it is the first of
    /// the command; only the elements shown, and of those only the ones in view, are drawn.
    fn tables(&mut self, ui: &mut Ui, ctrl: bool) -> Option<Request> {
        let font = TextStyle::Body.resolve(ui.style());
        let scale = (font.size, ui.ctx().pixels_per_point());
        if self.measured_at != scale {
            self.layouts.clear();
            self.measured_at = scale;
        }
        // charts as pictures as wide as the window allows, on the paper it shows
        let chart_width = (ui.available_width() - 2.0 * MARGIN - 16.0)
            .clamp(CHART_WIDTH.0, CHART_WIDTH.1)
            .round();
        let dark = ui.visuals().dark_mode;
        if self.scenes_at != (chart_width, dark, self.show) {
            self.scenes.clear();
            self.scenes_at = (chart_width, dark, self.show);
        }
        let paper = self.paper();
        let shown = Visibility {
            show: self.show,
            hidden: &self.hidden,
            all: false,
        };
        for t in output::tables(&self.runs) {
            if t.chart.is_some() && !self.scenes.contains_key(&t.id) {
                // the picture holds the chart's title and notes only when they show
                let scene = shown
                    .shown(t)
                    .1
                    .and_then(|c| picture::scene(&c, chart_width, paper));
                self.scenes.insert(t.id, scene);
            }
        }
        // where every table goes: (top, heading of its command if it shows above it, table, the
        // picture of its chart when it shows as one, and whether its title and notes show)
        type Block<'a> = (
            f32,
            Option<&'a str>,
            &'a OutputTable,
            Option<&'a Scene>,
            (bool, bool),
        );
        let mut blocks: Vec<Block> = Vec::new();
        let (mut bottom, mut width) = (0.0f32, 0.0f32);
        for item in self.runs.iter().flat_map(|run| &run.items) {
            // hidden tables stay in the navigator only; the heading goes above the first shown
            let heading = shown.heading(item).then_some(item.heading.as_str());
            for (i, t) in item.tables.iter().filter(|t| shown.table(t)).enumerate() {
                let scene = match self.as_data.contains(&t.id) {
                    true => None,
                    false => self.scenes.get(&t.id).and_then(Option::as_ref),
                };
                let parts = (shown.part(t, Part::Title), shown.part(t, Part::Notes));
                let size = match scene {
                    Some(scene) => vec2(scene.width, scene.height),
                    None => {
                        let layout = self
                            .layouts
                            .entry(t.id)
                            .or_insert_with(|| measure(ui, &t.table, &font));
                        vec2(layout.block_width, layout.block_height(parts.0, parts.1))
                    }
                };
                let heading = heading.filter(|_| i == 0);
                blocks.push((bottom, heading, t, scene, parts));
                let above = if heading.is_some() { HEADING } else { 0.0 };
                bottom += above + size.y + GAP;
                width = width.max(size.x + 8.0);
            }
        }
        let mut clicked = None;
        let mut secondary = None;
        let mut menu = None;
        let mut deselect = false;
        let layouts = &self.layouts;
        let selected = &self.selected;
        let scenes = &self.scenes;
        let as_data = &self.as_data;
        let scroll_to = &mut self.scroll_to;
        ScrollArea::both()
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                // a margin keeps the tables off the edge; a click beside them takes the selection away
                let (outer, background) = ui.allocate_exact_size(
                    vec2(width + 2.0 * MARGIN, bottom + 2.0 * MARGIN),
                    Sense::click(),
                );
                if background.clicked() {
                    deselect = true;
                }
                let area = outer.shrink(MARGIN);
                let first = blocks
                    .partition_point(|(top, ..)| *top <= viewport.min.y)
                    .saturating_sub(1);
                for &(top, heading, t, scene, (title, notes)) in &blocks[first..] {
                    if top > viewport.max.y {
                        break;
                    }
                    let mut y = area.top() + top;
                    if let Some(heading) = heading {
                        ui.painter().text(
                            pos2(area.left(), y + HEADING - 6.0),
                            egui::Align2::LEFT_BOTTOM,
                            heading,
                            FontId::proportional(13.0),
                            ui.visuals().weak_text_color(),
                        );
                        y += HEADING;
                    }
                    let rect = match scene {
                        Some(scene) => {
                            let rect = Rect::from_min_size(
                                pos2(area.left(), y),
                                vec2(scene.width, scene.height),
                            );
                            let clip = rect.expand(1.0).intersect(ui.clip_rect());
                            let painter = ui.painter().with_clip_rect(clip);
                            crate::chart_paint::paint(&painter, scene, rect.min);
                            rect
                        }
                        None => {
                            let layout = &layouts[&t.id];
                            let rect = Rect::from_min_size(
                                pos2(area.left(), y),
                                vec2(layout.block_width, layout.block_height(title, notes)),
                            );
                            paint_block(ui, &t.table, layout, rect, &font, (title, notes));
                            rect
                        }
                    };
                    if selected.contains(&t.id) {
                        let stroke = Stroke::new(2.0, ui.visuals().selection.stroke.color);
                        let frame = rect.expand(4.0);
                        ui.painter()
                            .rect_stroke(frame, 0.0, stroke, StrokeKind::Outside);
                    }
                    let response = ui.interact(
                        rect,
                        ui.make_persistent_id(("output table", t.id)),
                        Sense::click(),
                    );
                    // a plugin's comment on the cell under the pointer
                    let comment = response.hover_pos().filter(|_| scene.is_none());
                    let comment = comment
                        .and_then(|pos| comment_at(&t.table, &layouts[&t.id], rect, title, pos));
                    let response = match comment {
                        Some(comment) => response.on_hover_text_at_pointer(comment),
                        None => response,
                    };
                    if response.clicked() {
                        clicked = Some(t.id);
                    }
                    if response.secondary_clicked() {
                        secondary = Some(t.id);
                    }
                    let office = t.chart.as_ref().is_some_and(is_office_chart);
                    let drawn = matches!(scenes.get(&t.id), Some(Some(_)));
                    response.context_menu(|ui| {
                        for (label, action) in [
                            (i18n::t("output-copy"), Menu::Copy),
                            (i18n::t("output-copy-as-text"), Menu::CopyText),
                            (i18n::t("output-delete"), Menu::Delete),
                        ] {
                            if ui.button(label).clicked() {
                                menu = Some((action, t.id));
                                ui.close();
                            }
                        }
                        if drawn {
                            ui.separator();
                            let show = match as_data.contains(&t.id) {
                                true => i18n::t("output-show-chart"),
                                false => i18n::t("output-show-data"),
                            };
                            for (label, action) in [
                                (show, Menu::ShowData),
                                (i18n::t("output-save-png"), Menu::Save { png: true }),
                                (i18n::t("output-save-svg"), Menu::Save { png: false }),
                            ] {
                                if ui.button(label).clicked() {
                                    menu = Some((action, t.id));
                                    ui.close();
                                }
                            }
                        }
                        // a chart with no office form goes in as a picture
                        if office || drawn {
                            ui.separator();
                            for program in Program::ALL {
                                if ui.button(program.label()).clicked() {
                                    menu = Some((Menu::Open(program), t.id));
                                    ui.close();
                                }
                            }
                        }
                    });
                }
                if let Some((id, frames)) = *scroll_to
                    && let Some(&(top, ..)) = blocks.iter().find(|(_, _, t, ..)| t.id == id)
                {
                    let at = area.top() + top;
                    let room = Rect::from_min_size(pos2(area.left(), at), vec2(1.0, 40.0));
                    ui.scroll_to_rect(room, Some(Align::TOP));
                    *scroll_to = (frames > 1).then_some((id, frames - 1));
                }
            });
        // a right click on a table outside the selection selects it alone, for its menu
        if let Some(id) = secondary
            && !self.selected.contains(&id)
        {
            self.pick(&[id], false);
        }
        if let Some(id) = clicked {
            self.pick(&[id], ctrl);
        } else if deselect {
            self.selected.clear();
        }
        match menu {
            Some((Menu::Copy, _)) => Some(Request::Status(self.copy(ui, true))),
            Some((Menu::CopyText, _)) => Some(Request::Status(self.copy(ui, false))),
            Some((Menu::Delete, _)) => Some(Request::Delete),
            Some((Menu::Open(program), id)) => {
                let chart = output::tables(&self.runs).find(|t| t.id == id);
                let chart = chart.and_then(|t| t.chart.as_ref())?;
                Some(Request::Status(office::open(chart, program)))
            }
            Some((Menu::ShowData, id)) => {
                if !self.as_data.remove(&id) {
                    self.as_data.insert(id);
                }
                None
            }
            Some((Menu::Save { png }, id)) => {
                let chart = output::tables(&self.runs).find(|t| t.id == id);
                let chart = chart.and_then(|t| t.chart.clone())?;
                Some(Request::SaveChart {
                    chart: Box::new(chart),
                    png,
                    paper: self.paper(),
                })
            }
            None => None,
        }
    }
}

/// The width of a text without wrapping, and of its longest word.
fn text_widths(ui: &Ui, text: &str, font: &FontId) -> (f32, f32) {
    let width = |t: &str| {
        ui.ctx().fonts_mut(|f| {
            f.layout_no_wrap(t.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
    };
    let word = text.split_whitespace().map(width).fold(0.0, f32::max);
    (width(text), word)
}

fn text_size(ui: &Ui, text: &str, font: &FontId, wrap: f32) -> Vec2 {
    ui.ctx().fonts_mut(|f| {
        f.layout(text.to_owned(), font.clone(), Color32::WHITE, wrap)
            .size()
    })
}

/// Column widths that fit every number and keep words whole, with long row labels and column
/// headers wrapped; row heights that fit the wrapped labels.
fn measure(ui: &Ui, table: &Table, font: &FontId) -> Layout {
    let columns = table.columns.len();
    let mut widths = vec![24.0f32; columns];
    for r in 0..table.rows.len() {
        for (c, w) in widths.iter_mut().enumerate().take(table.rows[r].len()) {
            let text = output::cell_text(table, r, c);
            let (full, word) = text_widths(ui, &text, font);
            let need = if c < table.row_headers {
                full.min(ROW_LABEL_MAX).max(word)
            } else {
                full
            };
            *w = w.max(need + 2.0 * PAD_X);
        }
    }
    let headers = output::header_rows(table);
    // innermost labels first, so a spanning label only widens what is still too narrow
    for level in headers.iter().rev() {
        for cell in level {
            let span = cell.first..cell.first + cell.span;
            let (full, word) = text_widths(ui, &cell.label, font);
            let need = full
                .min(HEADER_MAX.max(ROW_LABEL_MAX * (cell.span as f32 - 1.0)))
                .max(word)
                + 2.0 * PAD_X;
            let have: f32 = widths[span.clone()].iter().sum();
            if need > have {
                let extra = (need - have) / cell.span as f32;
                widths[span].iter_mut().for_each(|w| *w += extra);
            }
        }
    }
    let line = text_size(ui, "Ag", font, f32::INFINITY).y + 2.0 * PAD_Y;
    let header_heights = headers
        .iter()
        .map(|level| {
            level
                .iter()
                .map(|cell| {
                    let width: f32 = widths[cell.first..cell.first + cell.span].iter().sum();
                    text_size(ui, &cell.label, font, width - 2.0 * PAD_X).y + 2.0 * PAD_Y
                })
                .fold(line, f32::max)
        })
        .collect();
    let spans = output::row_spans(table);
    let mut heights = vec![line; table.rows.len()];
    let label_height = |r: usize, c: usize| {
        let text = output::cell_text(table, r, c);
        text_size(ui, &text, font, widths[c] - 2.0 * PAD_X).y + 2.0 * PAD_Y
    };
    for (r, row) in spans.iter().enumerate() {
        for (c, &span) in row.iter().enumerate() {
            if span == 1 {
                heights[r] = heights[r].max(label_height(r, c));
            }
        }
    }
    // a merged label taller than its rows makes the last of them taller
    for (r, row) in spans.iter().enumerate() {
        for (c, &span) in row.iter().enumerate() {
            if span > 1 {
                let have: f32 = heights[r..r + span].iter().sum();
                let need = label_height(r, c);
                if need > have {
                    heights[r + span - 1] += need - have;
                }
            }
        }
    }
    let mut row_tops = vec![0.0];
    for h in heights {
        row_tops.push(row_tops.last().unwrap() + h);
    }
    let width: f32 = widths.iter().sum();
    let title = FontId::proportional(TITLE_SIZE);
    let wrap = width.max(CAPTION_MIN);
    let title_size = text_size(ui, &table.title, &title, wrap);
    let caption_size = table
        .caption
        .as_ref()
        .map_or(Vec2::ZERO, |caption| text_size(ui, caption, font, wrap));
    Layout {
        widths,
        header_heights,
        row_tops,
        spans,
        title_height: title_size.y,
        caption_height: caption_size.y,
        block_width: width.max(title_size.x).max(caption_size.x),
    }
}

/// The plugin's comment on the body cell of a table drawn in `rect` at `pos`, if any.
fn comment_at(
    table: &Table,
    layout: &Layout,
    rect: Rect,
    title: bool,
    pos: Pos2,
) -> Option<String> {
    let above = if title {
        layout.title_height + 6.0
    } else {
        0.0
    };
    let y = pos.y - rect.top() - above - layout.header_height();
    let r = layout
        .row_tops
        .partition_point(|t| *t <= y)
        .checked_sub(1)?;
    let x = f64::from(pos.x - rect.left());
    let offsets = crate::grid::offsets(&layout.widths);
    let c = offsets.partition_point(|o| *o <= x).checked_sub(1)?;
    table.decoration(r, c)?.comment.clone()
}

/// A text laid out at `pos`: left, centred on or right of it as `align` says, wrapped at `wrap`.
fn text(
    painter: &Painter,
    pos: Pos2,
    align: Align,
    text: String,
    font: &FontId,
    color: Color32,
    wrap: f32,
) {
    styled_text(painter, pos, align, text, font, color, wrap, None);
}

/// `text` in the bold and italic a plugin gave the cell. Bold is the text drawn twice, a
/// little apart, as the window's fonts have no bold face.
#[allow(clippy::too_many_arguments)]
fn styled_text(
    painter: &Painter,
    pos: Pos2,
    align: Align,
    text: String,
    font: &FontId,
    color: Color32,
    wrap: f32,
    look: Option<&Decoration>,
) {
    let mut job = LayoutJob::simple(text, font.clone(), color, wrap);
    job.halign = align;
    let (bold, italic) = look.map_or((false, false), |d| (d.bold, d.italic));
    job.sections
        .iter_mut()
        .for_each(|s| s.format.italics = italic);
    let galley: Arc<_> = painter.ctx().fonts_mut(|f| f.layout_job(job));
    if bold {
        painter.galley(pos + vec2(0.6, 0.0), galley.clone(), color);
    }
    painter.galley(pos, galley, color);
}

/// A plugin's colour #rrggbb as a window colour.
fn rgb(colour: &Option<String>) -> Option<Color32> {
    let [r, g, b] = colour.as_deref().and_then(parse_colour)?;
    Some(Color32::from_rgb(r, g, b))
}

/// Paints a plugin's fill on `cell` and returns the colour of its text: its own, else dark on
/// a fill (the fills plugins give are light, whatever the paper), else `normal`.
fn fill_cell(painter: &Painter, cell: Rect, look: Option<&Decoration>, normal: Color32) -> Color32 {
    let Some(d) = look else {
        return normal;
    };
    let mut colour = normal;
    if let Some(fill) = rgb(&d.fill) {
        painter.rect_filled(cell, 0.0, fill);
        colour = Color32::from_gray(20);
    }
    rgb(&d.text).unwrap_or(colour)
}

/// A plugin's borders of one cell, drawn after the fills so no fill covers them.
fn cell_borders(lines: &mut Vec<(Pos2, Pos2, Color32)>, cell: Rect, look: Option<&Decoration>) {
    let Some(b) = look.and_then(|d| d.border.as_ref()) else {
        return;
    };
    let sides = [
        (&b.top, cell.left_top(), cell.right_top()),
        (&b.bottom, cell.left_bottom(), cell.right_bottom()),
        (&b.left, cell.left_top(), cell.left_bottom()),
        (&b.right, cell.right_top(), cell.right_bottom()),
    ];
    for (colour, from, to) in sides {
        if let Some(colour) = rgb(colour) {
            lines.push((from, to, colour));
        }
    }
}

/// Title, table and caption of one table in `rect`, painting only the rows in view, and the
/// title and caption only when they show.
fn paint_block(
    ui: &Ui,
    table: &Table,
    layout: &Layout,
    rect: Rect,
    font: &FontId,
    (title, notes): (bool, bool),
) {
    let visuals = ui.visuals();
    let clip = ui.clip_rect();
    let painter = ui
        .painter()
        .with_clip_rect(rect.expand(1.0).intersect(clip));
    let strong = visuals.strong_text_color();
    let normal = visuals.text_color();
    let width = layout.width();
    let wide = width.max(CAPTION_MIN);
    let mut top = rect.top();
    if title {
        text(
            &painter,
            rect.min,
            Align::LEFT,
            table.title.clone(),
            &FontId::proportional(TITLE_SIZE),
            strong,
            wide,
        );
        top += layout.title_height + 6.0;
    }
    let offsets = crate::grid::offsets(&layout.widths);
    let x = |c: usize| rect.left() + offsets[c] as f32;
    let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    let rule = Stroke::new(1.5, normal);
    let right = rect.left() + width;

    // header: every level, a group label underlined over its columns
    let header_bottom = top + layout.header_height();
    painter.rect_filled(
        Rect::from_min_max(pos2(x(table.row_headers), top), pos2(right, header_bottom)),
        0.0,
        visuals.widgets.noninteractive.weak_bg_fill,
    );
    let mut y = top;
    // the plugin's borders, drawn last
    let mut borders = Vec::new();
    for (number, (level, h)) in output::header_rows(table)
        .iter()
        .zip(&layout.header_heights)
        .enumerate()
    {
        for cell in level {
            let span = Rect::from_min_max(
                pos2(x(cell.first), y),
                pos2(x(cell.first + cell.span), y + h),
            );
            let look = table.header_decoration(number, cell.first);
            let colour = fill_cell(&painter, span, look, strong);
            for c in cell.first..cell.first + cell.span {
                let place = Rect::from_min_max(pos2(x(c), y), pos2(x(c + 1), y + h));
                cell_borders(&mut borders, place, table.header_decoration(number, c));
            }
            styled_text(
                &painter,
                pos2(span.center().x, y + PAD_Y),
                Align::Center,
                cell.label.clone(),
                font,
                colour,
                span.width() - 2.0 * PAD_X,
                look,
            );
            if cell.first >= table.row_headers && !cell.label.is_empty() {
                painter.hline(span.shrink2(vec2(4.0, 0.0)).x_range(), span.bottom(), line);
            }
        }
        y += h;
    }
    painter.hline(rect.left()..=right, header_bottom, rule);

    // body: the rows in view, a merged row label from its first row
    let tops = &layout.row_tops;
    let rows = table.rows.len();
    let first = tops
        .partition_point(|t| header_bottom + t < clip.top())
        .saturating_sub(1)
        .min(rows);
    let last = tops
        .partition_point(|t| header_bottom + t < clip.bottom())
        .min(rows);
    let row_y = |r: usize| header_bottom + tops[r];
    for r in first..last {
        let (y0, y1) = (row_y(r), row_y(r + 1));
        if r % 2 == 1 {
            painter.rect_filled(
                Rect::from_min_max(pos2(x(table.row_headers), y0), pos2(right, y1)),
                0.0,
                visuals.faint_bg_color,
            );
        }
        painter.hline(x(table.row_headers)..=right, y1, line);
        for c in 0..table.rows[r].len().min(layout.widths.len()) {
            let place = Rect::from_min_max(pos2(x(c), y0), pos2(x(c + 1), y1));
            cell_borders(&mut borders, place, table.decoration(r, c));
            if c < table.row_headers {
                // the first row in view paints a label merged from above it
                let start = if layout.spans[r][c] == 0 && r == first {
                    (0..r).rev().find(|&s| layout.spans[s][c] > 0).unwrap_or(r)
                } else {
                    r
                };
                if layout.spans[start][c] > 0 {
                    let end = row_y(start + layout.spans[start][c]);
                    let merged = Rect::from_min_max(pos2(x(c), row_y(start)), pos2(x(c + 1), end));
                    let look = table.decoration(start, c);
                    let colour = fill_cell(&painter, merged, look, normal);
                    let label = output::cell_text(table, start, c);
                    styled_text(
                        &painter,
                        pos2(x(c) + PAD_X, row_y(start) + PAD_Y),
                        Align::LEFT,
                        label,
                        font,
                        colour,
                        layout.widths[c] - 2.0 * PAD_X,
                        look,
                    );
                }
                // a line under a label ends where its span ends
                let ends = r + 1 == rows || layout.spans[r + 1][c] > 0;
                if ends {
                    painter.hline(x(c)..=x(c + 1), y1, line);
                }
                continue;
            }
            // a plugin's fill, text colour and a corner mark for its comment
            let look = table.decoration(r, c);
            let colour = fill_cell(&painter, place, look, normal);
            if look.is_some_and(|d| d.comment.is_some()) {
                let corner = place.right_top();
                let mark = vec![corner - vec2(6.0, 0.0), corner, corner + vec2(0.0, 6.0)];
                let red = Color32::from_rgb(200, 40, 40);
                painter.add(egui::Shape::convex_polygon(mark, red, Stroke::NONE));
            }
            let shown = output::cell_text(table, r, c);
            if shown.is_empty() {
                continue;
            }
            let right = output::right_aligned(table, r, c);
            let (pos, align) = match look.and_then(|d| d.align.as_deref()) {
                Some("center") => (pos2(place.center().x, y0 + PAD_Y), Align::Center),
                Some("right") => (pos2(x(c + 1) - PAD_X, y0 + PAD_Y), Align::RIGHT),
                None if right => (pos2(x(c + 1) - PAD_X, y0 + PAD_Y), Align::RIGHT),
                _ => (pos2(x(c) + PAD_X, y0 + PAD_Y), Align::LEFT),
            };
            styled_text(
                &painter,
                pos,
                align,
                shown,
                font,
                colour,
                f32::INFINITY,
                look,
            );
        }
    }
    for (from, to, colour) in borders {
        painter.line_segment([from, to], Stroke::new(1.0, colour));
    }
    let bottom = header_bottom + tops.last().copied().unwrap_or(0.0);
    if table.row_headers > 0 {
        painter.vline(x(table.row_headers), header_bottom..=bottom, line);
    }
    painter.hline(rect.left()..=right, bottom, rule);
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        text(
            &painter,
            pos2(rect.left(), bottom + 4.0),
            Align::LEFT,
            caption.clone(),
            font,
            visuals.weak_text_color(),
            wide,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_hidden_in_the_document_is_hidden_in_the_view_and_follows_changes() {
        let table = || (statscraft_engine::Table::default(), None);
        let command = |heading: &str, heading_hidden| CommandShown {
            heading: heading.into(),
            heading_hidden,
            syntax: String::new(),
            leaves: vec![table()],
        };
        let mut view = OutputView::default();
        view.add_run("file", vec![command("A", true), command("B", false)]);
        let (a, b) = (view.runs[0].items[0].id, view.runs[0].items[1].id);
        assert!(view.hidden.contains(&(a, Part::Heading)));
        assert!(!view.hidden.contains(&(b, Part::Heading)));
        let change = |position, heading_hidden| ModifiedItem {
            position,
            deleted: false,
            heading: None,
            heading_hidden: Some(heading_hidden),
            table: Some(statscraft_engine::Table::default()),
            chart: None,
        };
        view.apply_modified(&[change(0, false), change(1, true)]);
        assert!(!view.hidden.contains(&(a, Part::Heading)));
        assert!(view.hidden.contains(&(b, Part::Heading)));
    }

    #[test]
    fn a_click_on_the_selection_as_it_is_takes_it_away() {
        let mut view = OutputView::default();
        view.pick(&[1], false);
        assert_eq!(view.selected, HashSet::from([1]));
        view.pick(&[2], true);
        assert_eq!(view.selected, HashSet::from([1, 2]));
        // a plain click on one of several selects that one alone
        view.pick(&[2], false);
        assert_eq!(view.selected, HashSet::from([2]));
        // and a second plain click takes it away
        view.pick(&[2], false);
        assert!(view.selected.is_empty());
        // a command's tables picked twice in the navigator likewise
        view.pick(&[3, 4], false);
        view.pick(&[3, 4], false);
        assert!(view.selected.is_empty());
    }
}
