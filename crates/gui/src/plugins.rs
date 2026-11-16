//! The plugin of Edit > Settings > Plugins (docs/plugins.md): it runs on a thread of its own
//! after every run that made tables with cell statistics, and again when its settings change.
//! Its answers wait until no run goes on, then go on the tables of the output window and of
//! the window's document, so a saved document keeps them. Its settings are data file
//! attributes of the active dataset, which Edit > Plugin Settings writes through syntax.

use crate::settings::{Preset, Settings};
use crate::syntax_text::quote;
use crate::windows::WindowId;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;
use statscraft_engine::plugin::{self, Description, Kind, Value};
use statscraft_engine::{Table, parse_colour};
use std::path::PathBuf;
use std::sync::mpsc;

/// A plugin's answer for the tables of one output window.
pub struct Done {
    pub window: WindowId,
    pub document: String,
    /// The settings generation the answer was asked under; an older one is dropped.
    pub generation: u64,
    pub result: Result<Vec<(u64, plugin::Answer)>, String>,
}

pub struct Plugins {
    /// The description of the plugin program, with the program it came from.
    described: Option<(PathBuf, Result<Description, String>)>,
    sender: mpsc::Sender<Done>,
    receiver: mpsc::Receiver<Done>,
    /// Answers not yet put on the tables.
    pub waiting: Vec<Done>,
    /// The settings changed: every table is decorated again once no run goes on.
    pub redo: bool,
    /// Counts the changes of the plugin or its settings, so a slow answer asked before a change
    /// cannot overwrite a newer one.
    pub generation: u64,
    /// The open settings dialog: the description it was opened with and the values typed.
    pub dialog: Option<(Description, Vec<Field>)>,
}

/// One setting in Edit > Plugin Settings.
pub struct Field {
    setting: plugin::Setting,
    text: String,
    /// The preset the value came from, with the preset's value: OK keeps its name with the
    /// data while the text still equals that value.
    preset: Option<(String, String)>,
    /// The name typed for a new preset.
    new_name: String,
    /// A style setting while its Define window is open.
    style: Option<StyleEdit>,
}

/// A style setting as its Define window edits it: an empty font or size, a colour switched off
/// and an empty alignment leave the key out of the style; bold and italic are left out until
/// ticked or unticked.
#[derive(Debug, Clone, PartialEq)]
struct StyleEdit {
    font: String,
    size: String,
    bold: Option<bool>,
    italic: Option<bool>,
    text_on: bool,
    text: String,
    fill_on: bool,
    fill: String,
    align: String,
}

impl StyleEdit {
    /// The style of a setting's JSON text; anything but an object reads as an empty style.
    fn read(json: &str) -> StyleEdit {
        let value = json.parse::<Value>().unwrap_or_default();
        let get = |key: &str| value.get(key).filter(|v| !v.is_null());
        let text = |key: &str| get(key).and_then(Value::as_str).map(str::to_string);
        StyleEdit {
            font: text("font").unwrap_or_default(),
            size: get("size").map(Value::to_string).unwrap_or_default(),
            bold: get("bold").and_then(Value::as_bool),
            italic: get("italic").and_then(Value::as_bool),
            text_on: text("text").is_some(),
            text: text("text").unwrap_or_else(|| "#000000".into()),
            fill_on: text("fill").is_some(),
            fill: text("fill").unwrap_or_else(|| "#ffffff".into()),
            align: text("align").unwrap_or_default(),
        }
    }

    /// The style as compact JSON, the keys not set left out.
    fn write(&self) -> String {
        let mut map = plugin::Map::new();
        let mut put = |key: &str, value: Value| map.insert(key.into(), value);
        if !self.font.trim().is_empty() {
            put("font", self.font.trim().into());
        }
        if let Ok(size) = self.size.trim().parse::<f64>() {
            // 8 rather than 8.0; sizes are small, so the whole number fits
            let whole = size.fract() == 0.0 && size.abs() < 1e9;
            put(
                "size",
                if whole {
                    (size as i64).into()
                } else {
                    size.into()
                },
            );
        }
        if let Some(bold) = self.bold {
            put("bold", bold.into());
        }
        if let Some(italic) = self.italic {
            put("italic", italic.into());
        }
        if self.text_on {
            put("text", self.text.trim().into());
        }
        if self.fill_on {
            put("fill", self.fill.trim().into());
        }
        if !self.align.is_empty() {
            put("align", self.align.clone().into());
        }
        Value::Object(map).to_string()
    }

    /// Why the style cannot be kept: a size that is not 1 to 409 points or a colour that is not
    /// #rrggbb.
    fn problem(&self) -> Option<&'static str> {
        let size = self.size.trim();
        let size_ok =
            size.is_empty() || size.parse().is_ok_and(|s: f64| (1.0..=409.0).contains(&s));
        let colour = |on: bool, c: &str| !on || parse_colour(c.trim()).is_some();
        match () {
            _ if !size_ok => Some("the size is a number of points from 1 to 409"),
            _ if !colour(self.text_on, &self.text) => Some("the text colour is not #rrggbb"),
            _ if !colour(self.fill_on, &self.fill) => Some("the fill is not #rrggbb"),
            _ => None,
        }
    }
}

impl Field {
    /// The name of the preset the text holds the value of, if any.
    fn preset(&self) -> Option<&str> {
        let (name, value) = self.preset.as_ref()?;
        (value == self.text.trim()).then_some(name.as_str())
    }
}

impl Default for Plugins {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Plugins {
            described: None,
            sender,
            receiver,
            waiting: Vec::new(),
            redo: false,
            generation: 0,
            dialog: None,
        }
    }
}

/// The first value of a data file attribute, by name in any case.
#[cfg(test)]
fn attribute(data: Option<&Dataset>, name: &str) -> Option<String> {
    first_value(data?.attributes(), name)
}

/// The attributes the plugin's settings are read from: the active data's own, then the values
/// remembered for its file that the data does not carry (a file read again before it was saved).
fn attributes(settings: &Settings, data: Option<&Dataset>) -> Vec<(String, Vec<String>)> {
    let mut all = data.map(|d| d.attributes().to_vec()).unwrap_or_default();
    if let Some(file) = data.and_then(Dataset::file) {
        for (name, value) in settings.values_for(file) {
            if !all.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                all.push((name, value));
            }
        }
    }
    all
}

fn first_value(attributes: &[(String, Vec<String>)], name: &str) -> Option<String> {
    let mut found = attributes
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case(name));
    found.next()?.1.first().cloned()
}

impl Plugins {
    /// The description of the chosen plugin program, asked once per program; None without one.
    pub fn description(&mut self, settings: &Settings) -> Option<&Result<Description, String>> {
        let program = settings.plugin.as_ref()?;
        if self.described.as_ref().is_none_or(|(p, _)| p != program) {
            self.described = Some((program.clone(), plugin::describe(program)));
        }
        self.described.as_ref().map(|(_, d)| d)
    }

    /// The plugin program or whether it runs changed: it is asked again and every table is
    /// decorated again.
    pub fn changed(&mut self) {
        self.described = None;
        self.again();
    }

    /// The plugin's settings changed: every table is decorated again, and answers asked before
    /// are dropped.
    pub fn again(&mut self) {
        self.redo = true;
        self.generation += 1;
        let generation = self.generation;
        self.waiting.retain(|d| d.generation == generation);
    }

    /// What Edit > Settings shows about the plugin.
    pub fn line(&mut self, settings: &Settings) -> String {
        match self.description(settings) {
            None => "No plugin chosen".into(),
            Some(Ok(d)) => format!("{} {}", d.name, d.version),
            Some(Err(e)) => format!("Not usable: {e}"),
        }
    }

    /// Starts the plugin on the tables of an output window that have cell statistics, on a
    /// thread of its own, which also asks it for its settings, so a slow plugin never holds up
    /// the window; `wake` is called when it is done. Nothing runs while the plugin is off or
    /// not chosen.
    pub fn start(
        &self,
        settings: &Settings,
        data: Option<&Dataset>,
        (window, document): (WindowId, &str),
        tables: Vec<(u64, Table)>,
        wake: impl Fn() + Send + 'static,
    ) {
        let tables: Vec<(u64, Table)> = tables
            .into_iter()
            .filter(|(_, t)| t.cell_stats.is_some())
            .collect();
        if settings.plugin_off || tables.is_empty() {
            return;
        }
        let Some(program) = settings.plugin.clone() else {
            return;
        };
        let attributes = attributes(settings, data);
        let sender = self.sender.clone();
        let document = document.to_string();
        let generation = self.generation;
        std::thread::spawn(move || {
            let refs: Vec<(u64, &Table)> = tables.iter().map(|(id, t)| (*id, t)).collect();
            let result = plugin::describe(&program).and_then(|description| {
                let values = description.values(|name| first_value(&attributes, name));
                plugin::decorate(&program, &values, &refs)
            });
            let _ = sender.send(Done {
                window,
                document,
                generation,
                result,
            });
            wake();
        });
    }

    /// Takes the answers that came in.
    pub fn receive(&mut self) {
        let generation = self.generation;
        let current = self
            .receiver
            .try_iter()
            .filter(|d| d.generation == generation);
        self.waiting.extend(current);
    }

    /// Opens Edit > Plugin Settings with the values of the active dataset; an error when no
    /// plugin is usable.
    pub fn open_dialog(
        &mut self,
        settings: &Settings,
        data: Option<&Dataset>,
    ) -> Result<(), String> {
        // asked afresh: the program may have changed since
        self.described = None;
        let description = match self.description(settings) {
            Some(Ok(d)) => d.clone(),
            Some(Err(e)) => return Err(format!("plugin: {e}")),
            None => return Err("choose a plugin in Edit > Settings > Plugins first".into()),
        };
        let known = attributes(settings, data);
        let values = description.values(|name| first_value(&known, name));
        let fields = description.settings.iter().map(|s| {
            let text = match &values[&s.key] {
                plugin::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let preset = first_value(&known, &description.preset_attribute(&s.key))
                .filter(|name| s.presets && !name.is_empty())
                .map(|name| (name, text.trim().to_string()));
            Field {
                setting: s.clone(),
                text,
                preset,
                new_name: String::new(),
                style: None,
            }
        });
        self.dialog = Some((description.clone(), fields.collect()));
        Ok(())
    }

    /// Edit > Plugin Settings while it is open; returns, when OK is pressed, the syntax that
    /// keeps the values as data file attributes and the same values as (attribute, value)
    /// pairs. Presets saved or removed change `presets`.
    pub fn dialog_ui(
        &mut self,
        ctx: &egui::Context,
        presets: &mut Vec<Preset>,
    ) -> Option<(String, Vec<(String, String)>)> {
        let (description, fields) = self.dialog.as_mut()?;
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let screen = ctx.content_rect();
        let sections = sections(&description.settings);
        egui::Window::new("Plugin Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.weak(
                    "Kept with the active data (save the data file to take them along) and \
                     remembered on this computer for its file.",
                );
                ui.add_space(6.0);
                let mut column = |ui: &mut Ui, sections: &[(String, Vec<usize>)]| {
                    for (n, (group, list)) in sections.iter().enumerate() {
                        if !group.is_empty() {
                            ui.add_space(6.0);
                            ui.strong(group);
                        }
                        egui::Grid::new(("plugin settings", group)).show(ui, |ui| {
                            for &i in list {
                                setting_ui(ui, &mut fields[i], description, presets);
                            }
                        });
                        if n + 1 < sections.len() {
                            ui.add_space(4.0);
                        }
                    }
                };
                egui::ScrollArea::vertical()
                    .max_height(screen.height() * 0.7)
                    .show(ui, |ui| {
                        // two columns of sections on a wide screen, the first holding about
                        // half the rows
                        if screen.width() < 1100.0 || sections.len() < 2 {
                            return column(ui, &sections);
                        }
                        let rows = |s: &(String, Vec<usize>)| s.1.len() + 1;
                        let half = sections.iter().map(rows).sum::<usize>().div_ceil(2);
                        let mut split = 0;
                        let mut taken = 0;
                        while split < sections.len() - 1 && taken < half {
                            taken += rows(&sections[split]);
                            split += 1;
                        }
                        let (left, right) = sections.split_at(split);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| column(ui, left));
                            ui.add_space(18.0);
                            ui.vertical(|ui| column(ui, right));
                        });
                    });
                let bad = fields.iter().find(|f| !valid(f.setting.kind, &f.text));
                if let Some(f) = bad {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("{}: not a number", f.setting.label),
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ok = ui
                        .add_enabled(bad.is_none(), egui::Button::new("OK"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        for f in fields.iter_mut() {
            let Some(edit) = &mut f.style else {
                continue;
            };
            match style_window(ctx, &f.setting, edit) {
                Some(true) => {
                    f.text = edit.write();
                    f.style = None;
                }
                Some(false) => f.style = None,
                None => {}
            }
        }
        if !(ok || cancel || !open) {
            return None;
        }
        // the description the dialog was opened with: the program may have changed since
        let (description, fields) = self.dialog.take()?;
        ok.then(|| {
            let pairs = attribute_values(&description, &fields);
            (attribute_syntax(&pairs), pairs)
        })
    }
}

/// The settings by section: those without a group first, then each group in the order it first
/// appears; each with the places of its settings.
fn sections(settings: &[plugin::Setting]) -> Vec<(String, Vec<usize>)> {
    let mut sections = vec![(String::new(), Vec::new())];
    for (i, s) in settings.iter().enumerate() {
        let group = s.group.as_deref().unwrap_or("").trim();
        match sections.iter_mut().find(|(g, _)| g == group) {
            Some((_, list)) => list.push(i),
            None => sections.push((group.to_string(), vec![i])),
        }
    }
    sections.retain(|(_, list)| !list.is_empty());
    sections
}

/// One row of Edit > Plugin Settings: the label, the value and the presets, or for a style its
/// preview and Define.
fn setting_ui(ui: &mut Ui, f: &mut Field, description: &Description, presets: &mut Vec<Preset>) {
    ui.label(&f.setting.label);
    if f.setting.kind == Kind::Style {
        ui.horizontal(|ui| {
            preview(ui, &StyleEdit::read(&f.text));
            if ui.button("Define...").clicked() {
                f.style = Some(StyleEdit::read(&f.text));
            }
        });
    } else {
        field(ui, f.setting.kind, &mut f.text);
    }
    if f.setting.presets && matches!(f.setting.kind, Kind::Number | Kind::Text) {
        let attribute = description.attribute(&f.setting.key);
        ui.horizontal(|ui| preset_ui(ui, f, presets, &attribute));
    }
    ui.end_row();
}

/// A sample of a style in a small box: its fill, text colour, bold, italic and alignment, and
/// its font and size as words (the window keeps its own font).
fn preview(ui: &mut Ui, style: &StyleEdit) {
    let colour = |on: bool, c: &str| {
        let [r, g, b] = parse_colour(c.trim()).filter(|_| on)?;
        Some(egui::Color32::from_rgb(r, g, b))
    };
    let words = [style.font.trim(), style.size.trim()].join(" ");
    let mut text = egui::RichText::new(match words.trim() {
        "" => "Sample",
        words => words,
    });
    if style.bold == Some(true) {
        text = text.strong();
    }
    if style.italic == Some(true) {
        text = text.italics();
    }
    if let Some(c) = colour(style.text_on, &style.text) {
        text = text.color(c);
    }
    let align = match style.align.as_str() {
        "center" => egui::Align::Center,
        "right" => egui::Align::Max,
        _ => egui::Align::Min,
    };
    egui::Frame::new()
        .fill(colour(style.fill_on, &style.fill).unwrap_or_default())
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.set_width(130.0);
            ui.with_layout(egui::Layout::top_down(align), |ui| ui.label(text));
        });
}

/// The Define window of a style setting: font, size, bold, italic, text colour, fill and
/// alignment, with a preview. Some(true) on OK, Some(false) on Cancel or when closed.
fn style_window(
    ctx: &egui::Context,
    setting: &plugin::Setting,
    edit: &mut StyleEdit,
) -> Option<bool> {
    let (mut open, mut done) = (true, None);
    egui::Window::new(format!("Define Style: {}", setting.label))
        .id(egui::Id::unique(("plugin style", &setting.key)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new(("plugin style grid", &setting.key)).show(ui, |ui| {
                ui.label("Font");
                ui.add(egui::TextEdit::singleline(&mut edit.font).hint_text("not set"));
                ui.end_row();
                ui.label("Size");
                ui.horizontal(|ui| {
                    let size = egui::TextEdit::singleline(&mut edit.size)
                        .hint_text("not set")
                        .desired_width(60.0);
                    ui.add(size);
                    ui.label("points");
                });
                ui.end_row();
                for (name, value) in [("Bold", &mut edit.bold), ("Italic", &mut edit.italic)] {
                    ui.label(name);
                    let mut on = value.unwrap_or(false);
                    if ui.checkbox(&mut on, "").changed() {
                        *value = Some(on);
                    }
                    ui.end_row();
                }
                let colours = [
                    ("Text colour", "none", &mut edit.text_on, &mut edit.text),
                    ("Fill", "no fill", &mut edit.fill_on, &mut edit.fill),
                ];
                for (name, none, on, colour) in colours {
                    ui.label(name);
                    ui.horizontal(|ui| {
                        let mut off = !*on;
                        if ui.checkbox(&mut off, none).changed() {
                            *on = !off;
                        }
                        if *on {
                            field(ui, Kind::Colour, colour);
                        }
                    });
                    ui.end_row();
                }
                ui.label("Alignment");
                let shown = |a: &str| {
                    if a.is_empty() {
                        "not set".to_string()
                    } else {
                        a.to_string()
                    }
                };
                egui::ComboBox::from_id_salt(("plugin style align", &setting.key))
                    .selected_text(shown(&edit.align))
                    .show_ui(ui, |ui| {
                        for a in ["", "left", "center", "right"] {
                            ui.selectable_value(&mut edit.align, a.to_string(), shown(a));
                        }
                    });
                ui.end_row();
                ui.label("Preview");
                preview(ui, edit);
                ui.end_row();
            });
            let problem = edit.problem();
            if let Some(problem) = problem {
                ui.colored_label(ui.visuals().error_fg_color, problem);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(problem.is_none(), egui::Button::new("OK"))
                    .clicked()
                {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
    match open {
        true => done,
        false => Some(false),
    }
}

/// The presets of a setting: a list to choose one from, which fills the field, Remove for the
/// chosen one, and a name field with Save as preset for the value typed.
fn preset_ui(ui: &mut Ui, field: &mut Field, presets: &mut Vec<Preset>, attribute: &str) {
    let chosen = field.preset().map(str::to_string);
    egui::ComboBox::from_id_salt(("plugin preset", attribute))
        .selected_text(chosen.as_deref().unwrap_or("Presets"))
        .show_ui(ui, |ui| {
            for p in presets.iter().filter(|p| p.attribute == attribute) {
                let shown = format!("{}: {}", p.name, p.value);
                if ui
                    .selectable_label(chosen.as_ref() == Some(&p.name), shown)
                    .clicked()
                {
                    field.text = p.value.clone();
                    field.preset = Some((p.name.clone(), p.value.clone()));
                }
            }
        });
    if ui
        .add_enabled(chosen.is_some(), egui::Button::new("Remove"))
        .clicked()
    {
        presets.retain(|p| !(p.attribute == attribute && Some(&p.name) == chosen.as_ref()));
        field.preset = None;
    }
    let name = egui::TextEdit::singleline(&mut field.new_name)
        .hint_text("Preset name")
        .desired_width(110.0);
    ui.add(name);
    let can = !field.new_name.trim().is_empty() && valid(field.setting.kind, &field.text);
    if ui
        .add_enabled(can, egui::Button::new("Save as preset"))
        .clicked()
    {
        let preset = Preset::new(attribute, &field.new_name, &field.text);
        presets.retain(|p| !(p.attribute == attribute && p.name == preset.name));
        field.preset = Some((preset.name.clone(), preset.value.clone()));
        presets.push(preset);
        field.new_name.clear();
    }
}

/// One value: a number or text in a text field, a colour with its picker, yes or no in a box.
fn field(ui: &mut Ui, kind: Kind, text: &mut String) {
    match kind {
        Kind::Bool => {
            let mut on = text == "true";
            if ui.checkbox(&mut on, "").changed() {
                *text = on.to_string();
            }
        }
        Kind::Colour => {
            ui.horizontal(|ui| {
                let mut rgb = parse_colour(text).unwrap_or([255, 255, 255]);
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    *text = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                }
                ui.add(egui::TextEdit::singleline(text).desired_width(80.0));
            });
        }
        Kind::Number | Kind::Text | Kind::Style => {
            ui.text_edit_singleline(text);
        }
    }
}

fn valid(kind: Kind, text: &str) -> bool {
    kind != Kind::Number || text.trim().parse::<f64>().is_ok()
}

/// DATAFILE ATTRIBUTE that keeps every value under its attribute name, and for a setting with
/// presets the name of the preset its value came from (empty when none).
fn attribute_syntax(pairs: &[(String, String)]) -> String {
    let values: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("{name}({})", quote(value)))
        .collect();
    format!("DATAFILE ATTRIBUTE ATTRIBUTE={}.", values.join(" "))
}

/// Every value under its attribute name, with the preset name of a setting that has presets.
fn attribute_values(description: &Description, fields: &[Field]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for f in fields {
        let key = &f.setting.key;
        pairs.push((description.attribute(key), f.text.trim().to_string()));
        if f.setting.presets {
            let preset = f.preset().unwrap_or("").to_string();
            pairs.push((description.preset_attribute(key), preset));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values chosen for a data file come back when the file is read again without its new
    /// attributes; the data's own attributes win over them.
    #[test]
    fn values_remembered_for_a_file_hold_after_it_is_read_again() {
        let dir =
            std::env::temp_dir().join(format!("statscraft-plugin-values-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("survey.sav");
        let mut session = statscraft_engine::session::Session::default();
        let make = "DATA LIST FREE / a.\nBEGIN DATA\n1 2\nEND DATA.\n";
        session
            .run(&format!("{make}SAVE OUTFILE='{}'.", file.display()), None)
            .unwrap();
        let mut settings = Settings::default();
        let file = std::path::absolute(&file).unwrap();
        let pairs = vec![(
            "plugin_margins_population".to_string(),
            "1024000".to_string(),
        )];
        settings.remember_values(&file, &pairs);
        // reading the file again: no attribute in the data, the remembered value holds
        session
            .run(&format!("GET FILE='{}'.", file.display()), None)
            .unwrap();
        let all = attributes(&settings, session.dataset());
        assert_eq!(
            first_value(&all, "plugin_margins_population").as_deref(),
            Some("1024000")
        );
        // an attribute of the data wins
        session
            .run(
                "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_margins_population('500').",
                None,
            )
            .unwrap();
        let all = attributes(&settings, session.dataset());
        assert_eq!(
            first_value(&all, "plugin_margins_population").as_deref(),
            Some("500")
        );
        // the settings file keeps them
        let read = Settings::parse(&settings.text());
        assert_eq!(read.file_values, settings.file_values);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An answer asked before the settings changed is dropped; one asked after is kept.
    #[test]
    fn an_answer_from_before_a_change_is_dropped() {
        let mut plugins = Plugins::default();
        let done = |generation| Done {
            window: 1,
            document: "doc".into(),
            generation,
            result: Ok(Vec::new()),
        };
        plugins.sender.send(done(0)).unwrap();
        plugins.waiting.push(done(0));
        plugins.again();
        plugins.receive();
        assert!(plugins.waiting.is_empty());
        plugins.sender.send(done(1)).unwrap();
        plugins.receive();
        assert_eq!(plugins.waiting.len(), 1);
    }

    #[test]
    fn plugin_settings_are_kept_as_data_file_attributes() {
        let setting = |key: &str, kind, default: f64| plugin::Setting {
            key: key.into(),
            label: key.into(),
            kind,
            default: default.into(),
            presets: false,
            group: None,
        };
        let description = Description {
            name: "Example".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![
                setting("threshold", Kind::Number, 0.05),
                setting("note", Kind::Text, 0.0),
            ],
        };
        let fields: Vec<Field> = description
            .settings
            .iter()
            .cloned()
            .zip(["0.1".to_string(), "it's".to_string()])
            .map(|(setting, text)| Field {
                setting,
                text,
                preset: None,
                new_name: String::new(),
                style: None,
            })
            .collect();
        let syntax = attribute_syntax(&attribute_values(&description, &fields));
        assert_eq!(
            syntax,
            "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_threshold('0.1') plugin_example_note('it''s')."
        );
        // the syntax runs, and the values come back from the data
        let mut session = statscraft_engine::session::Session::default();
        session
            .run("DATA LIST LIST /x (F1).\nBEGIN DATA\n1\nEND DATA.", None)
            .unwrap();
        session.run(&syntax, None).unwrap();
        let values = description.values(|name| attribute(session.dataset(), name));
        assert_eq!(values["threshold"], 0.1);
        assert_eq!(values["note"], "it's");
        assert!(valid(Kind::Number, " 2.5 ") && !valid(Kind::Number, "x"));
    }

    /// A value chosen from a preset keeps the preset's name with the data; once the value is
    /// typed over, the name is cleared.
    #[test]
    fn a_preset_name_is_kept_beside_its_value() {
        let description = Description {
            name: "House".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![plugin::Setting {
                key: "universe".into(),
                label: "Population".into(),
                kind: Kind::Number,
                default: 0.into(),
                presets: true,
                group: None,
            }],
        };
        assert_eq!(
            description.preset_attribute("universe"),
            "plugin_house_universe_preset"
        );
        let mut field = Field {
            setting: description.settings[0].clone(),
            text: "1043000".into(),
            preset: Some(("Adults".into(), "1043000".into())),
            new_name: String::new(),
            style: None,
        };
        let syntax = attribute_syntax(&attribute_values(
            &description,
            std::slice::from_ref(&field),
        ));
        assert_eq!(
            syntax,
            "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_house_universe('1043000') \
             plugin_house_universe_preset('Adults')."
        );
        let mut session = statscraft_engine::session::Session::default();
        session
            .run("DATA LIST LIST /x (F1).\nBEGIN DATA\n1\nEND DATA.", None)
            .unwrap();
        session.run(&syntax, None).unwrap();
        let stored = attribute(session.dataset(), "plugin_house_universe_preset");
        assert_eq!(stored.as_deref(), Some("Adults"));
        field.text = "990000".into();
        assert_eq!(field.preset(), None);
        let syntax = attribute_syntax(&attribute_values(&description, &[field]));
        assert!(
            syntax.ends_with("plugin_house_universe_preset('')."),
            "{syntax}"
        );
    }

    /// A style edited in its Define window becomes compact JSON with only the keys set, is kept
    /// as a data file attribute and comes back to the plugin as an object.
    #[test]
    fn a_style_is_kept_as_json_text() {
        let mut edit =
            StyleEdit::read(r##"{"font":"Verdana","size":8,"bold":true,"fill":"#ddebf7"}"##);
        assert_eq!((edit.font.as_str(), edit.size.as_str()), ("Verdana", "8"));
        assert_eq!((edit.bold, edit.italic), (Some(true), None));
        assert!(edit.fill_on && !edit.text_on);
        // no fill, an alignment, italic switched off: explicit false, unlike a key never set
        edit.fill_on = false;
        edit.align = "center".into();
        edit.italic = Some(false);
        edit.size = "8.5".into();
        let json = edit.write();
        assert_eq!(
            json,
            r#"{"font":"Verdana","size":8.5,"bold":true,"italic":false,"align":"center"}"#
        );
        assert_eq!(StyleEdit::read(&json).write(), json);
        assert_eq!(edit.problem(), None);
        edit.size = "500".into();
        assert!(edit.problem().is_some());
        edit.size.clear();
        edit.text_on = true;
        edit.text = "navy".into();
        assert!(edit.problem().is_some());
        // anything but an object reads as an empty style
        assert_eq!(StyleEdit::read("[1]").write(), "{}");

        let description = Description {
            name: "House".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![plugin::Setting {
                key: "head".into(),
                label: "Header".into(),
                kind: Kind::Style,
                default: plugin::Value::Object(plugin::Map::new()),
                presets: false,
                group: Some("Header".into()),
            }],
        };
        let field = Field {
            setting: description.settings[0].clone(),
            text: r#"{"bold":true,"font":"it's"}"#.into(),
            preset: None,
            new_name: String::new(),
            style: None,
        };
        let syntax = attribute_syntax(&attribute_values(&description, &[field]));
        assert_eq!(
            syntax,
            r#"DATAFILE ATTRIBUTE ATTRIBUTE=plugin_house_head('{"bold":true,"font":"it''s"}')."#
        );
        let mut session = statscraft_engine::session::Session::default();
        session
            .run("DATA LIST LIST /x (F1).\nBEGIN DATA\n1\nEND DATA.", None)
            .unwrap();
        session.run(&syntax, None).unwrap();
        let values = description.values(|name| attribute(session.dataset(), name));
        assert_eq!(values["head"]["font"], "it's");
        assert_eq!(values["head"]["bold"], true);
    }

    /// Settings without a group come first, then each group in the order it first appears.
    #[test]
    fn settings_are_shown_in_sections() {
        let setting = |key: &str, group: Option<&str>| plugin::Setting {
            key: key.into(),
            label: key.into(),
            kind: Kind::Bool,
            default: true.into(),
            presets: false,
            group: group.map(str::to_string),
        };
        let settings = [
            setting("a", Some("Look")),
            setting("b", None),
            setting("c", Some("Statistics")),
            setting("d", Some("Look")),
        ];
        let shown = sections(&settings);
        let want = [("", vec![1]), ("Look", vec![0, 3]), ("Statistics", vec![2])];
        assert_eq!(shown, want.map(|(g, list)| (g.to_string(), list)));
        // no setting without a group: no empty section
        assert_eq!(sections(&settings[..1]), [("Look".to_string(), vec![0])]);
    }
}
