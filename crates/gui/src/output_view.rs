//! The Output tab: a navigator of runs, commands and tables beside the tables themselves. Each
//! table is measured once, and a table out of view only takes its place, so hundreds of tables
//! scroll smoothly.

use crate::output::{self, OutputItem, OutputRun, OutputTable};
use eframe::egui::{
    self, Align, Color32, FontId, Key, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, pos2, text::LayoutJob, vec2,
};
use statscraft_engine::Table;
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

/// What the Output tab asks of the window.
pub enum Request {
    /// Export the selected tables, as a web page or as an Excel workbook (with one sheet for all).
    ExportHtml,
    ExportXlsx {
        one_sheet: bool,
    },
    Status(Result<String, String>),
    /// Put the syntax in the syntax editor.
    Paste(String),
    /// Run the syntax again.
    Run(String),
}

/// The context menu of a table.
#[derive(Clone, Copy)]
enum Menu {
    Copy,
    CopyText,
    Delete,
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

    /// The title, the table and the caption.
    fn block_height(&self) -> f32 {
        self.title_height + 6.0 + self.table_height() + 4.0 + self.caption_height
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
}

impl OutputView {
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Adds the tables of one run, with a heading per command; returns whether there were any.
    /// Each command comes with its heading and the syntax that made it.
    pub fn add_run(&mut self, origin: &str, commands: Vec<(String, String, Vec<Table>)>) -> bool {
        let mut items = Vec::new();
        for (heading, syntax, tables) in commands {
            if tables.is_empty() {
                continue;
            }
            let tables = tables
                .into_iter()
                .map(|table| {
                    self.next_id += 1;
                    OutputTable {
                        id: self.next_id,
                        table,
                    }
                })
                .collect();
            items.push(OutputItem {
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
            label: format!("Run {}: {origin}", self.run_count),
            items,
        });
        self.scroll_to = Some((first, 2));
        true
    }

    pub fn clear(&mut self) {
        self.runs.clear();
        self.selected.clear();
        self.layouts.clear();
    }

    fn delete_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        output::retain(&mut self.runs, |id| !selected.contains(&id));
        self.layouts.retain(|id, _| !selected.contains(id));
    }

    fn selected_tables(&self) -> Vec<&Table> {
        output::tables(&self.runs)
            .filter(|t| self.selected.contains(&t.id))
            .map(|t| &t.table)
            .collect()
    }

    /// Puts the selected tables on the clipboard: as rich text with plain text beside it, or
    /// as tab-separated text only.
    fn copy(&mut self, ui: &Ui, rich: bool) -> Result<String, String> {
        let tables = self.selected_tables();
        let text: Vec<String> = tables.iter().map(|t| output::tsv(t)).collect();
        let text = text.join("\n");
        let what = match tables.len() {
            1 => "1 table".to_string(),
            n => format!("{n} tables"),
        };
        if !rich {
            ui.ctx().copy_text(text);
            return Ok(format!("{what} copied as text"));
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
            Some(Ok(())) => Ok(format!("{what} copied")),
            _ => {
                ui.ctx().copy_text(text);
                Err(format!(
                    "{what} copied as text: the clipboard took no rich text"
                ))
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
                ui.heading("No output yet");
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
                self.delete_selected();
            }
            if all {
                self.selected = output::tables(&self.runs).map(|t| t.id).collect();
            }
        }
        ui.horizontal(|ui| {
            let some = !self.selected.is_empty();
            if ui
                .add_enabled(some, egui::Button::new("Copy"))
                .on_hover_text("Copy the selected tables with their layout (Ctrl+C)")
                .clicked()
            {
                request = Some(Request::Status(self.copy(ui, true)));
            }
            if ui
                .add_enabled(some, egui::Button::new("Copy as text"))
                .on_hover_text("Tab-separated text, for a spreadsheet")
                .clicked()
            {
                request = Some(Request::Status(self.copy(ui, false)));
            }
            if ui
                .add_enabled(some, egui::Button::new("Delete"))
                .on_hover_text("Delete the selected tables (Delete)")
                .clicked()
            {
                self.delete_selected();
            }
            ui.add_enabled_ui(some, |ui| {
                ui.menu_button("Export selection", |ui| {
                    if ui.button("Web page...").clicked() {
                        request = Some(Request::ExportHtml);
                    }
                    if ui.button("Excel, one sheet per table...").clicked() {
                        request = Some(Request::ExportXlsx { one_sheet: false });
                    }
                    if ui.button("Excel, all tables on one sheet...").clicked() {
                        request = Some(Request::ExportXlsx { one_sheet: true });
                    }
                });
            });
            ui.separator();
            if ui.button("Select all").clicked() {
                self.selected = output::tables(&self.runs).map(|t| t.id).collect();
            }
            if ui.button("Clear output").clicked() {
                self.clear();
            }
            match self.selected.len() {
                0 => {}
                1 => {
                    ui.weak("1 table selected");
                }
                n => {
                    ui.weak(format!("{n} tables selected"));
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
                    ui.weak("Syntax");
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(item.syntax.clone());
                        request = Some(Request::Status(Ok("syntax copied".into())));
                    }
                    if ui.small_button("Paste").clicked() {
                        request = Some(Request::Paste(item.syntax.clone()));
                    }
                    if ui.small_button("Run again").clicked() {
                        request = Some(Request::Run(item.syntax.clone()));
                    }
                });
                ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(&item.syntax).monospace()).extend());
                });
            });
        request
    }

    /// One row per run, command and table, of which only the rows in view are drawn.
    fn navigator(&mut self, ui: &mut Ui, ctrl: bool) {
        // long titles are cut short here; the tables show them whole
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let mut rows: Vec<(usize, Option<usize>, Option<usize>)> = Vec::new();
        for (r, run) in self.runs.iter().enumerate() {
            rows.push((r, None, None));
            if self.collapsed.contains(&run.label) {
                continue;
            }
            for (i, item) in run.items.iter().enumerate() {
                rows.push((r, Some(i), None));
                rows.extend((0..item.tables.len()).map(|t| (r, Some(i), Some(t))));
            }
        }
        let mut picked: Option<Vec<u64>> = None;
        let mut toggle = None;
        let height = ui.spacing().interact_size.y;
        ScrollArea::vertical()
            .auto_shrink(false)
            .show_rows(ui, height, rows.len(), |ui, range| {
                for &(r, item, table) in &rows[range] {
                    let run = &self.runs[r];
                    match (item, table) {
                        (None, _) => {
                            let open = !self.collapsed.contains(&run.label);
                            let mark = if open { "\u{25be}" } else { "\u{25b8}" };
                            let text = RichText::new(format!("{mark} {}", run.label)).weak();
                            if ui.selectable_label(false, text).clicked() {
                                toggle = Some(run.label.clone());
                            }
                        }
                        (Some(i), None) => {
                            let ids: Vec<u64> = run.items[i].tables.iter().map(|t| t.id).collect();
                            let all = ids.iter().all(|id| self.selected.contains(id));
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let heading = RichText::new(&run.items[i].heading).strong();
                                let row = ui.selectable_label(all, heading);
                                let syntax = &run.items[i].syntax;
                                let row = row.on_hover_ui(|ui| {
                                    ui.label(RichText::new(syntax).monospace());
                                });
                                if row.clicked() {
                                    picked = Some(ids);
                                }
                            });
                        }
                        (Some(i), Some(t)) => {
                            let t = &run.items[i].tables[t];
                            ui.horizontal(|ui| {
                                ui.add_space(22.0);
                                let on = self.selected.contains(&t.id);
                                if ui.selectable_label(on, &t.table.title).clicked() {
                                    picked = Some(vec![t.id]);
                                }
                            });
                        }
                    }
                }
            });
        if let Some(label) = toggle
            && !self.collapsed.remove(&label)
        {
            self.collapsed.insert(label);
        }
        if let Some(ids) = picked {
            self.scroll_to = ids.first().map(|&id| (id, 2));
            self.pick(&ids, ctrl);
        }
    }

    /// The tables one below the other, each under its command's heading when it is the first of
    /// the command; only the tables in view are drawn.
    fn tables(&mut self, ui: &mut Ui, ctrl: bool) -> Option<Request> {
        let font = TextStyle::Body.resolve(ui.style());
        let scale = (font.size, ui.ctx().pixels_per_point());
        if self.measured_at != scale {
            self.layouts.clear();
            self.measured_at = scale;
        }
        // where every table goes: (top, heading of its command if it is the first, table)
        let mut blocks: Vec<(f32, Option<&str>, &OutputTable)> = Vec::new();
        let (mut bottom, mut width) = (0.0f32, 0.0f32);
        for item in self.runs.iter().flat_map(|run| &run.items) {
            for (i, t) in item.tables.iter().enumerate() {
                let layout = self
                    .layouts
                    .entry(t.id)
                    .or_insert_with(|| measure(ui, &t.table, &font));
                let heading = (i == 0).then_some(item.heading.as_str());
                blocks.push((bottom, heading, t));
                let above = if heading.is_some() { HEADING } else { 0.0 };
                bottom += above + layout.block_height() + GAP;
                width = width.max(layout.block_width + 8.0);
            }
        }
        let mut clicked = None;
        let mut secondary = None;
        let mut menu = None;
        let mut deselect = false;
        let layouts = &self.layouts;
        let selected = &self.selected;
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
                for &(top, heading, t) in &blocks[first..] {
                    if top > viewport.max.y {
                        break;
                    }
                    let layout = &layouts[&t.id];
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
                    let rect = Rect::from_min_size(
                        pos2(area.left(), y),
                        vec2(layout.block_width, layout.block_height()),
                    );
                    paint_block(ui, &t.table, layout, rect, &font);
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
                    if response.clicked() {
                        clicked = Some(t.id);
                    }
                    if response.secondary_clicked() {
                        secondary = Some(t.id);
                    }
                    response.context_menu(|ui| {
                        for (label, action) in [
                            ("Copy", Menu::Copy),
                            ("Copy as text", Menu::CopyText),
                            ("Delete", Menu::Delete),
                        ] {
                            if ui.button(label).clicked() {
                                menu = Some(action);
                                ui.close();
                            }
                        }
                    });
                }
                if let Some((id, frames)) = *scroll_to
                    && let Some(&(top, ..)) = blocks.iter().find(|(.., t)| t.id == id)
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
            Some(Menu::Copy) => Some(Request::Status(self.copy(ui, true))),
            Some(Menu::CopyText) => Some(Request::Status(self.copy(ui, false))),
            Some(Menu::Delete) => {
                self.delete_selected();
                None
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
    let mut job = LayoutJob::simple(text, font.clone(), color, wrap);
    job.halign = align;
    let galley: Arc<_> = painter.ctx().fonts_mut(|f| f.layout_job(job));
    painter.galley(pos, galley, color);
}

/// Title, table and caption of one table in `rect`, painting only the rows in view.
fn paint_block(ui: &Ui, table: &Table, layout: &Layout, rect: Rect, font: &FontId) {
    let visuals = ui.visuals();
    let clip = ui.clip_rect();
    let painter = ui
        .painter()
        .with_clip_rect(rect.expand(1.0).intersect(clip));
    let strong = visuals.strong_text_color();
    let normal = visuals.text_color();
    let width = layout.width();
    let wide = width.max(CAPTION_MIN);
    text(
        &painter,
        rect.min,
        Align::LEFT,
        table.title.clone(),
        &FontId::proportional(TITLE_SIZE),
        strong,
        wide,
    );
    let top = rect.top() + layout.title_height + 6.0;
    let offsets = crate::grid::offsets(&layout.widths);
    let x = |c: usize| rect.left() + offsets[c];
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
    for (level, h) in output::header_rows(table)
        .iter()
        .zip(&layout.header_heights)
    {
        for cell in level {
            let span = Rect::from_min_max(
                pos2(x(cell.first), y),
                pos2(x(cell.first + cell.span), y + h),
            );
            text(
                &painter,
                pos2(span.center().x, y + PAD_Y),
                Align::Center,
                cell.label.clone(),
                font,
                strong,
                span.width() - 2.0 * PAD_X,
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
            if c < table.row_headers {
                // the first row in view paints a label merged from above it
                let start = if layout.spans[r][c] == 0 && r == first {
                    (0..r).rev().find(|&s| layout.spans[s][c] > 0).unwrap_or(r)
                } else {
                    r
                };
                if layout.spans[start][c] > 0 {
                    let label = output::cell_text(table, start, c);
                    text(
                        &painter,
                        pos2(x(c) + PAD_X, row_y(start) + PAD_Y),
                        Align::LEFT,
                        label,
                        font,
                        normal,
                        layout.widths[c] - 2.0 * PAD_X,
                    );
                }
                // a line under a label ends where its span ends
                let ends = r + 1 == rows || layout.spans[r + 1][c] > 0;
                if ends {
                    painter.hline(x(c)..=x(c + 1), y1, line);
                }
                continue;
            }
            let shown = output::cell_text(table, r, c);
            if shown.is_empty() {
                continue;
            }
            let (pos, align) = if output::right_aligned(table, r, c) {
                (pos2(x(c + 1) - PAD_X, y0 + PAD_Y), Align::RIGHT)
            } else {
                (pos2(x(c) + PAD_X, y0 + PAD_Y), Align::LEFT)
            };
            text(&painter, pos, align, shown, font, normal, f32::INFINITY);
        }
    }
    let bottom = header_bottom + tops.last().copied().unwrap_or(0.0);
    if table.row_headers > 0 {
        painter.vline(x(table.row_headers), header_bottom..=bottom, line);
    }
    painter.hline(rect.left()..=right, bottom, rule);
    if let Some(caption) = &table.caption {
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
