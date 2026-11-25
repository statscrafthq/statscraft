//! A syntax window: its text and file, the editor with colors, completion and F1 help, and the
//! Active list of its toolbar. Runs go to the window's one session (app.rs).

use crate::settings::Settings;
use crate::{complete, editor, highlight, themes};
use eframe::egui::{
    self, Align2, FontId, Key, Modifiers, RichText, ScrollArea, TextEdit, Ui, pos2,
    text::LayoutJob, vec2,
};
use statscraft_data::Dataset;
use statscraft_engine::session::{Failure, Session};
use std::path::{Path, PathBuf};

/// The completion list of the editor.
struct Completion {
    list: Vec<complete::Candidate>,
    /// The bytes of the text a candidate replaces.
    range: std::ops::Range<usize>,
    selected: usize,
}

/// The find bar of the editor.
struct Find {
    search: editor::Search,
    /// The replacement text, and whether its row shows.
    with: String,
    replace: bool,
    /// Focus the find field in the next frame.
    focus: bool,
    /// What Replace all did, until the search changes.
    note: Option<String>,
}

/// What the find bar asks for.
#[derive(Clone, Copy, PartialEq)]
enum FindStep {
    /// The first match from the start of the selection, while the text is typed.
    First,
    Next,
    Previous,
    Replace,
    ReplaceAll,
    Close,
}

/// The background of a match, and a stronger one for the match selected.
const MATCH: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(255, 200, 0, 90);
const CURRENT: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(255, 140, 0, 170);

pub(crate) struct SyntaxWindow {
    pub text: String,
    /// The text as last loaded or saved; empty for new syntax.
    pub saved: String,
    pub file: Option<PathBuf>,
    /// The name until the syntax is saved to a file.
    untitled: String,
    /// The errors of the last run from this window.
    pub errors: Vec<Failure>,
    /// The selection as character offsets (anchor, cursor).
    selection: (usize, usize),
    editor_id: Option<egui::Id>,
    completion: Option<Completion>,
    /// The full help of a command, from F1.
    help: Option<String>,
    find: Option<Find>,
    /// The find bar's text, options and replacement when it last closed.
    last_find: (editor::Search, String),
    /// Scroll the editor to the selection in the next frame.
    reveal: bool,
}

impl SyntaxWindow {
    pub fn new(untitled: String, text: String, file: Option<PathBuf>) -> Self {
        SyntaxWindow {
            saved: text.clone(),
            text,
            file,
            untitled,
            errors: Vec::new(),
            selection: (0, 0),
            editor_id: None,
            completion: None,
            help: None,
            find: None,
            last_find: Default::default(),
            reveal: false,
        }
    }

    /// Opens the find bar, with the replace row if `replace`, and the selection as the text to
    /// find when it is on one line.
    pub fn open_find(&mut self, replace: bool) {
        let (a, b) = (
            self.selection.0.min(self.selection.1),
            self.selection.0.max(self.selection.1),
        );
        let selected: String = self.text.chars().skip(a).take(b - a).collect();
        let last = &self.last_find;
        let find = self.find.get_or_insert_with(|| Find {
            search: last.0.clone(),
            with: last.1.clone(),
            replace: false,
            focus: true,
            note: None,
        });
        if !selected.is_empty() && !selected.contains('\n') {
            find.search.text = selected;
        }
        find.replace |= replace;
        find.focus = true;
    }

    /// The selection as bytes, start first.
    fn selected_bytes(&self) -> std::ops::Range<usize> {
        let (a, b) = self.selection;
        editor::byte_of(&self.text, a.min(b))..editor::byte_of(&self.text, a.max(b))
    }

    /// Selects the bytes `range` in the editor and scrolls to them.
    fn select_bytes(&mut self, ctx: &egui::Context, range: std::ops::Range<usize>) {
        let start = self.text[..range.start].chars().count();
        let end = start + self.text[range].chars().count();
        self.selection = (start, end);
        self.reveal = true;
        if let Some(id) = self.editor_id {
            let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(start),
                    egui::text::CCursor::new(end),
                )));
            state.store(ctx, id);
        }
    }

    /// Does what the find bar asks.
    fn find_step(&mut self, ctx: &egui::Context, step: FindStep) {
        let Some(find) = self.find.as_mut() else {
            return;
        };
        if step == FindStep::Close {
            self.last_find = (find.search.clone(), find.with.clone());
            self.find = None;
            if let Some(id) = self.editor_id {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            return;
        }
        let search = find.search.clone();
        let with = find.with.clone();
        find.note = None;
        let matches = search.matches(&self.text);
        let selection = self.selected_bytes();
        let from = match step {
            FindStep::First => editor::step(&matches, selection.start..selection.start, false),
            FindStep::Previous => editor::step(&matches, selection, true),
            FindStep::Replace if matches.contains(&selection) => {
                self.text.replace_range(selection.clone(), &with);
                self.completion = None;
                let after = selection.start + with.len();
                let matches = search.matches(&self.text);
                match editor::step(&matches, after..after, false) {
                    Some(i) => self.select_bytes(ctx, matches[i].clone()),
                    None => self.select_bytes(ctx, after..after),
                }
                return;
            }
            FindStep::ReplaceAll => {
                if !matches.is_empty() {
                    self.text = editor::replace_all(&self.text, &matches, &with);
                    self.completion = None;
                    // the cursor stays before the text it was in front of
                    let before = matches
                        .iter()
                        .take_while(|m| m.end <= selection.start)
                        .count();
                    let at = selection.start + before * with.len()
                        - matches[..before].iter().map(|m| m.len()).sum::<usize>();
                    let at = at.min(self.text.len());
                    let at = (0..=at)
                        .rev()
                        .find(|&b| self.text.is_char_boundary(b))
                        .unwrap_or(0);
                    self.select_bytes(ctx, at..at);
                }
                if let Some(find) = self.find.as_mut() {
                    find.note = Some(format!("Replaced {}", matches.len()));
                }
                return;
            }
            _ => editor::step(&matches, selection, false),
        };
        if let Some(i) = from {
            self.select_bytes(ctx, matches[i].clone());
        }
    }

    /// The find bar over the editor: the text to find, its options and how many matches there
    /// are, and the replace row.
    fn find_bar(&mut self, ui: &mut Ui, salt: u64) -> Option<FindStep> {
        let selection = self.selected_bytes();
        let find = self.find.as_mut()?;
        let matches = find.search.matches(&self.text);
        let mut step = None;
        egui::Panel::top(("syntax_find", salt)).show(ui, |ui| {
            let (enter, shift, escape) = ui.input(|i| {
                (
                    i.key_pressed(Key::Enter),
                    i.modifiers.shift,
                    i.key_pressed(Key::Escape),
                )
            });
            let mut fields = Vec::new();
            ui.horizontal(|ui| {
                let field = ui.add(
                    TextEdit::singleline(&mut find.search.text)
                        .id_salt(("find_text", salt))
                        .hint_text("Find")
                        .desired_width(240.0),
                );
                if std::mem::take(&mut find.focus) {
                    field.request_focus();
                }
                if field.changed() {
                    step = Some(FindStep::First);
                }
                if field.lost_focus() && enter {
                    step = Some(if shift {
                        FindStep::Previous
                    } else {
                        FindStep::Next
                    });
                    field.request_focus();
                }
                fields.push(field);
                if ui.checkbox(&mut find.search.case, "Match case").changed()
                    | ui.checkbox(&mut find.search.word, "Whole word").changed()
                {
                    step = Some(FindStep::First);
                }
                let current = matches.iter().position(|m| *m == selection);
                let count = match (&find.note, current, matches.len()) {
                    (Some(note), ..) => RichText::new(note),
                    _ if find.search.text.is_empty() => RichText::new(""),
                    (_, _, 0) => RichText::new("No matches").color(ui.visuals().error_fg_color),
                    (_, Some(i), n) => RichText::new(format!("{} of {n}", i + 1)),
                    (_, None, 1) => RichText::new("1 match"),
                    (_, None, n) => RichText::new(format!("{n} matches")),
                };
                ui.label(count);
                let any = !matches.is_empty();
                if ui
                    .add_enabled(any, egui::Button::new("Previous"))
                    .on_hover_text("Shift+F3")
                    .clicked()
                {
                    step = Some(FindStep::Previous);
                }
                if ui
                    .add_enabled(any, egui::Button::new("Next"))
                    .on_hover_text("F3")
                    .clicked()
                {
                    step = Some(FindStep::Next);
                }
                ui.toggle_value(&mut find.replace, "Replace")
                    .on_hover_text("Ctrl+H");
                if ui.button("Close").on_hover_text("Escape").clicked() {
                    step = Some(FindStep::Close);
                }
            });
            if find.replace {
                ui.horizontal(|ui| {
                    let field = ui.add(
                        TextEdit::singleline(&mut find.with)
                            .id_salt(("replace_text", salt))
                            .hint_text("Replace with")
                            .desired_width(240.0),
                    );
                    if field.lost_focus() && enter {
                        step = Some(FindStep::Replace);
                        field.request_focus();
                    }
                    fields.push(field);
                    let any = !matches.is_empty();
                    if ui
                        .add_enabled(any, egui::Button::new("Replace"))
                        .on_hover_text("Enter")
                        .clicked()
                    {
                        step = Some(FindStep::Replace);
                    }
                    if ui
                        .add_enabled(any, egui::Button::new("Replace all"))
                        .clicked()
                    {
                        step = Some(FindStep::ReplaceAll);
                    }
                });
            }
            if escape && fields.iter().any(|f| f.has_focus() || f.lost_focus()) {
                step = Some(FindStep::Close);
            }
        });
        step
    }

    /// The file name, or Syntax1 and so on for syntax not saved yet.
    pub fn name(&self) -> String {
        self.file.as_deref().and_then(Path::file_name).map_or_else(
            || self.untitled.clone(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// What would be lost if the window closed now: text that is not what its file holds, or
    /// new syntax that is not empty.
    pub fn unsaved(&self) -> Option<String> {
        (self.text != self.saved).then(|| format!("Syntax: {}", self.name()))
    }

    /// The lines of the whole commands the selection touches.
    pub fn selected_lines(&self) -> (u32, u32) {
        let lines = editor::selected_lines(&self.text, self.selection.0, self.selection.1);
        editor::whole_commands(&self.text, lines)
    }

    /// The editor, its error list, completion and help. Returns true when Ctrl+Enter asks to
    /// run the selection.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        salt: u64,
        data: Option<&Dataset>,
        macros: &[String],
        settings: &Settings,
    ) -> bool {
        let run_selection = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        // find and replace: Shift first, as a shortcut matches with extra Shift too
        let (find, replace, previous, next) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::COMMAND, Key::H),
                i.consume_key(Modifiers::SHIFT, Key::F3),
                i.consume_key(Modifiers::NONE, Key::F3),
            )
        });
        if find || replace {
            self.open_find(replace);
        }
        let step = self.find_bar(ui, salt).or(if previous {
            Some(FindStep::Previous)
        } else if next {
            Some(FindStep::Next)
        } else {
            None
        });
        match step {
            Some(_) if self.find.is_none() => self.open_find(false),
            Some(step) => self.find_step(ui.ctx(), step),
            None => {}
        }
        if ui.input(|i| i.key_pressed(Key::F1)) {
            let line = editor::line_of(&self.text, self.selection.0.max(self.selection.1));
            let command = editor::command_at(&self.text, line);
            self.help = statscraft_syntax::help(command.as_deref());
        }
        if !self.errors.is_empty() {
            egui::Panel::bottom(("syntax_errors", salt))
                .resizable(true)
                .default_size(80.0)
                .show(ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        for error in &self.errors {
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
        let error_lines: Vec<u32> = self.errors.iter().filter_map(|e| e.line).collect();
        let wrap = !settings.wrap_off;
        let macro_names = macros.join(" ");
        let theme = themes::by_name(&settings.theme);
        let search = self.find.as_ref().map(|f| f.search.clone());
        let (a, b) = self.selection;
        let theme_index = themes::THEMES
            .iter()
            .position(|t| t.name == theme.name)
            .unwrap_or(0);
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job =
                highlight::cached_job(ui.ctx(), text.as_str(), &macro_names, theme_index, 14.0);
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            if let Some(search) = &search {
                // the text laid out may be newer than the window's: find in it
                let text = text.as_str();
                let selection = editor::byte_of(text, a.min(b))..editor::byte_of(text, a.max(b));
                let marks: Vec<_> = search
                    .matches(text)
                    .into_iter()
                    .map(|m| {
                        let color = if m == selection { CURRENT } else { MATCH };
                        (m, color)
                    })
                    .collect();
                highlight::mark(&mut job, &marks);
            }
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
            // the text fills the window however tall it is
            let height = ui.available_height();
            area.id_salt(("syntax_scroll", salt))
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let left = ui.cursor().left();
                        ui.add_space(gutter);
                        let output = TextEdit::multiline(&mut self.text)
                            .id_salt(("syntax_editor", salt))
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
                        if std::mem::take(&mut self.reveal) {
                            let (a, b) = self.selection;
                            let at = |i| output.galley.pos_from_cursor(egui::text::CCursor::new(i));
                            let rect = at(a.min(b)).union(at(a.max(b)));
                            ui.scroll_to_rect(
                                rect.translate(output.galley_pos.to_vec2()),
                                Some(egui::Align::Center),
                            );
                        }
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
                            self.complete(explicit, data, macros);
                        } else if moved {
                            self.completion = None;
                        }
                        if output.response.hovered()
                            && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
                        {
                            let at = output.galley.cursor_from_pos(pointer - output.galley_pos);
                            let byte = editor::byte_of(&self.text, at.index.0);
                            hovered_command = complete::command_at_byte(&self.text, byte)
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
            self.completion_popup(ui.ctx(), at, salt);
        }
        if let Some(text) = &self.help {
            let mut open = true;
            egui::Window::new("Command help")
                .id(egui::Id::unique(("command_help", salt)))
                .open(&mut open)
                .default_size([560.0, 420.0])
                .show(ui.ctx(), |ui| {
                    // the help text is laid out in lines and indents, so it scrolls instead of wrapping
                    ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(text).monospace()).extend());
                    });
                });
            if !open || ui.input(|i| i.key_pressed(Key::Escape)) {
                self.help = None;
            }
        }
        run_selection
    }

    /// Opens, updates or closes the completion list for the text at the cursor.
    fn complete(&mut self, explicit: bool, data: Option<&Dataset>, macros: &[String]) {
        let cursor = editor::byte_of(&self.text, self.selection.1);
        if self.selection.0 != self.selection.1 {
            self.completion = None;
            return;
        }
        let variables: Vec<(String, String)> = data
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
        let (list, range) = complete::candidates(&self.text, cursor, &variables, macros, explicit);
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
        let (text, cursor) = complete::accept(&self.text, c.range, candidate);
        self.text = text;
        let at = self.text[..cursor].chars().count();
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
    fn completion_popup(&mut self, ctx: &egui::Context, at: egui::Pos2, salt: u64) {
        let Some(c) = self.completion.as_ref() else {
            return;
        };
        let mut clicked = None;
        egui::Area::new(egui::Id::unique(("completion", salt)))
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
}

/// The datasets of the session for the Active list: every name, and the active one's. An
/// active dataset without a name is listed as "unnamed".
pub(crate) fn datasets(session: &Session) -> (Vec<String>, Option<String>) {
    let (others, active) = session.named_datasets();
    let active = match active {
        Some(name) => Some(name.to_string()),
        None => session.dataset().map(|_| "unnamed".to_string()),
    };
    let mut names: Vec<String> = active.iter().cloned().collect();
    names.extend(others.iter().map(|(name, _)| name.clone()));
    (names, active)
}

/// The syntax the Active list runs.
pub(crate) fn activate(name: &str) -> String {
    format!("DATASET ACTIVATE {name}.")
}

/// The rest of `ui` as the editor's sheet, in the colors of its theme.
fn editor_paper(ui: &mut Ui, theme: &themes::Theme) {
    let visuals = &mut ui.style_mut().visuals;
    *visuals = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    crate::app::square(visuals);
    visuals.extreme_bg_color = theme.background;
    visuals.text_cursor.stroke.color = theme.text;
    let sheet = ui.available_rect_before_wrap();
    ui.painter().rect_filled(sheet, 0.0, theme.background);
}

/// The summary and syntax of a command, in the completion list and over a command name.
fn command_card(ui: &mut Ui, spec: &statscraft_syntax::CommandSpec) {
    ui.set_max_width(560.0);
    ui.strong(spec.name);
    ui.label(spec.summary);
    ui.add(egui::Label::new(RichText::new(spec.syntax).monospace().size(12.0)).wrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in};

    #[test]
    fn the_active_list_names_every_dataset_and_writes_dataset_activate() {
        assert_eq!(datasets(&Session::default()), (Vec::new(), None));
        let mut session = open("survey_basic.sav");
        assert_eq!(
            datasets(&session),
            (vec!["unnamed".to_string()], Some("unnamed".to_string()))
        );
        run_in(&mut session, "DATASET NAME first.");
        run_in(&mut session, "DATASET COPY second.");
        let (names, active) = datasets(&session);
        assert_eq!(names, ["first", "second"]);
        assert_eq!(active.as_deref(), Some("first"));
        // choosing one runs this, and the session changes its active dataset
        let syntax = activate("second");
        assert_eq!(syntax, "DATASET ACTIVATE second.");
        run_in(&mut session, &syntax);
        let (names, active) = datasets(&session);
        assert_eq!(names, ["second", "first"]);
        assert_eq!(active.as_deref(), Some("second"));
        assert_eq!(
            session.log.last().unwrap().syntax,
            "DATASET ACTIVATE second."
        );
    }

    #[test]
    fn a_syntax_window_is_unsaved_when_its_text_is_not_its_file() {
        let mut w = SyntaxWindow::new("Syntax1".into(), String::new(), None);
        assert_eq!(w.unsaved(), None);
        w.text = "FREQ x.".into();
        assert_eq!(w.unsaved().as_deref(), Some("Syntax: Syntax1"));
        let mut w = SyntaxWindow::new("Syntax2".into(), "FREQ x.".into(), Some("/d/a.sps".into()));
        assert_eq!(w.name(), "a.sps");
        assert_eq!(w.unsaved(), None);
        w.text.push_str("\nDESC y.");
        assert_eq!(w.unsaved().as_deref(), Some("Syntax: a.sps"));
    }
}
