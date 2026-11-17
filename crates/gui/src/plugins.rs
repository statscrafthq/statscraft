//! The plugins of Edit > Settings > Plugins (docs/plugins.md): they run in their order on a
//! thread of their own after every run that made tables with cell statistics, and again when
//! their settings change, and their answers merge table by table. The answers wait until no
//! run goes on, then go on the tables of the output window and of the window's document, so a
//! saved document keeps them. A plugin's settings are data file attributes of the active
//! dataset, which its Plugin Settings dialog (Settings... in that list) writes through syntax.

use crate::settings::{Preset, Settings, Template, active_template};
use crate::syntax_text::quote;
use crate::windows::WindowId;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;
use statscraft_engine::plugin::{self, Description, Kind, Value};
use statscraft_engine::{Table, parse_colour};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// The plugins' answers for the tables of one output window, merged in their order.
pub struct Done {
    pub window: WindowId,
    pub document: String,
    /// The settings generation the answer was asked under; an older one is dropped.
    pub generation: u64,
    pub answers: Vec<(u64, plugin::Answer)>,
    /// Why plugins failed; the others' answers still hold.
    pub errors: Vec<String>,
}

pub struct Plugins {
    /// The description of each plugin program asked so far, with the program it came from.
    described: Vec<(PathBuf, Result<Description, String>)>,
    sender: mpsc::Sender<Done>,
    receiver: mpsc::Receiver<Done>,
    /// Answers not yet put on the tables.
    pub waiting: Vec<Done>,
    /// The settings changed: every table is decorated again once no run goes on.
    pub redo: bool,
    /// Counts the changes of the plugin or its settings, so a slow answer asked before a change
    /// cannot overwrite a newer one.
    pub generation: u64,
    /// The open settings dialog.
    pub dialog: Option<Dialog>,
}

/// A plugin's Plugin Settings dialog while it is open: the description it was opened with, the
/// values typed, and the plugin's design templates as Cancel would leave them untouched.
pub struct Dialog {
    description: Description,
    fields: Vec<Field>,
    /// A copy of the plugin's templates, kept on OK.
    templates: Vec<Template>,
    /// The template OK makes active.
    chosen: Option<String>,
    /// The name typed for a new template.
    new_name: String,
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
            described: Vec::new(),
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

/// The attributes with the template laid over them: its values win for every setting not kept
/// per data file (with the setting's preset name); a setting the template lacks keeps what the
/// data or this computer has, else the plugin's default.
fn with_template(
    description: &Description,
    mut all: Vec<(String, Vec<String>)>,
    template: Option<&Template>,
) -> Vec<(String, Vec<String>)> {
    let Some(template) = template else {
        return all;
    };
    for s in description.settings.iter().filter(|s| !s.per_file) {
        let preset = s.presets.then(|| description.preset_attribute(&s.key));
        for name in [Some(description.attribute(&s.key)), preset]
            .into_iter()
            .flatten()
        {
            let found = template
                .values
                .iter()
                .find(|(a, _)| a.eq_ignore_ascii_case(&name));
            if let Some((_, value)) = found {
                all.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
                all.push((name, vec![value.clone()]));
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
    /// The description of a plugin program, asked once per program.
    pub fn description(&mut self, program: &Path) -> &Result<Description, String> {
        let at = match self.described.iter().position(|(p, _)| p == program) {
            Some(at) => at,
            None => {
                let described = plugin::describe(program);
                self.described.push((program.to_path_buf(), described));
                self.described.len() - 1
            }
        };
        &self.described[at].1
    }

    /// The plugins, their order or whether they run changed: they are asked again and every
    /// table is decorated again.
    pub fn changed(&mut self) {
        self.described.clear();
        self.again();
    }

    /// The plugins' settings changed: every table is decorated again, and answers asked before
    /// are dropped.
    pub fn again(&mut self) {
        self.redo = true;
        self.generation += 1;
        let generation = self.generation;
        self.waiting.retain(|d| d.generation == generation);
    }

    /// What Edit > Settings shows about a plugin: its name and version, or why it cannot be
    /// used.
    pub fn line(&mut self, program: &Path) -> String {
        match self.description(program) {
            Ok(d) => format!("{} {}", d.name, d.version),
            Err(e) => format!("Not usable: {e}"),
        }
    }

    /// Starts the plugins that are on, in their order, on the tables of an output window that
    /// have cell statistics, on a thread of its own, which also asks each for its settings, so
    /// a slow plugin never holds up the window; `wake` is called when they are done. Each
    /// plugin's answer is laid over those before it; one that fails leaves the others' answers
    /// standing. Nothing runs while no plugin is on.
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
        let on = settings.plugins.iter().filter(|p| p.on);
        let programs: Vec<PathBuf> = on.map(|p| p.path.clone()).collect();
        if programs.is_empty() || tables.is_empty() {
            return;
        }
        let attributes = attributes(settings, data);
        let templates = settings.templates.clone();
        let active = settings.active_templates.clone();
        let sender = self.sender.clone();
        let document = document.to_string();
        let generation = self.generation;
        std::thread::spawn(move || {
            let refs: Vec<(u64, &Table)> = tables.iter().map(|(id, t)| (*id, t)).collect();
            let (mut answers, mut errors) = (Vec::new(), Vec::new());
            for program in &programs {
                let result = plugin::describe(program).and_then(|description| {
                    let template = active_template(&templates, &active, &description);
                    let known = with_template(&description, attributes.clone(), template);
                    let values = description.values(|name| first_value(&known, name));
                    plugin::decorate(program, &values, &refs)
                });
                match result {
                    Ok(answer) => plugin::merge(&mut answers, answer),
                    Err(e) => errors.push(match program.file_name() {
                        Some(name) => format!("{}: {e}", name.to_string_lossy()),
                        None => e,
                    }),
                }
            }
            let _ = sender.send(Done {
                window,
                document,
                generation,
                answers,
                errors,
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

    /// Opens the Plugin Settings dialog of a plugin with the values of the active dataset; an
    /// error when the plugin is not usable.
    pub fn open_dialog(
        &mut self,
        settings: &Settings,
        data: Option<&Dataset>,
        program: &Path,
    ) -> Result<(), String> {
        // asked afresh: the program may have changed since
        self.described.retain(|(p, _)| p != program);
        let description = match self.description(program) {
            Ok(d) => d.clone(),
            Err(e) => return Err(format!("plugin: {e}")),
        };
        let template = settings.active_template(&description);
        let known = with_template(&description, attributes(settings, data), template);
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
        let fields = fields.collect();
        self.dialog = Some(Dialog {
            templates: settings.templates_of(&description),
            chosen: template.map(|t| t.name.clone()),
            description,
            fields,
            new_name: String::new(),
        });
        Ok(())
    }

    /// A plugin's Plugin Settings while it is open; returns, when OK is pressed, the syntax that
    /// keeps the values as data file attributes and the same values as (attribute, value)
    /// pairs. Presets saved or removed change the settings' presets at once; the templates and
    /// the active one change on OK.
    pub fn dialog_ui(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
    ) -> Option<(String, Vec<(String, String)>)> {
        let presets = &mut settings.presets;
        let Dialog {
            description,
            fields,
            templates,
            chosen,
            new_name,
        } = self.dialog.as_mut()?;
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let screen = ctx.content_rect();
        let sections = sections(&description.settings);
        egui::Window::new(format!("Plugin Settings: {}", description.name))
            .id(egui::Id::unique("plugin settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.weak(
                    "Kept with the active data (save the data file to take them along) and \
                     remembered on this computer for its file.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Template");
                    egui::ComboBox::from_id_salt("plugin template")
                        .selected_text(chosen.as_deref().unwrap_or("(none)"))
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(chosen.is_none(), "(none)").clicked() {
                                *chosen = None;
                            }
                            for t in templates.iter() {
                                let on = chosen.as_ref() == Some(&t.name);
                                if ui.selectable_label(on, &t.name).clicked() {
                                    load_template(description, fields, t);
                                    *chosen = Some(t.name.clone());
                                }
                            }
                        });
                    if ui
                        .add_enabled(chosen.is_some(), egui::Button::new("Delete"))
                        .clicked()
                    {
                        templates.retain(|t| Some(&t.name) != chosen.as_ref());
                        *chosen = None;
                    }
                    let name = egui::TextEdit::singleline(new_name)
                        .hint_text("Template name")
                        .desired_width(130.0);
                    ui.add(name);
                    let all_valid = fields.iter().all(|f| valid(f.setting.kind, &f.text));
                    let can = !new_name.trim().is_empty() && all_valid;
                    if ui
                        .add_enabled(can, egui::Button::new("Save as template"))
                        .clicked()
                    {
                        *chosen = Some(save_template(description, fields, templates, new_name));
                        new_name.clear();
                    }
                });
                ui.weak(
                    "A template holds every setting except those kept per data file, and is \
                     used for every data file until another is chosen.",
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
        let dialog = self.dialog.take()?;
        ok.then(|| dialog.finish(settings))
    }
}

impl Dialog {
    /// OK: the chosen template becomes the active one (or none) and takes the values of the
    /// template settings; every value, per file or not, goes to the data through syntax.
    fn finish(mut self, settings: &mut Settings) -> (String, Vec<(String, String)>) {
        if let Some(name) = &self.chosen {
            save_template(&self.description, &self.fields, &mut self.templates, name);
        }
        settings.set_templates(&self.description, self.templates, self.chosen);
        let pairs = attribute_values(&self.description, &self.fields);
        (attribute_syntax(&pairs), pairs)
    }
}

/// Puts a template's values in the fields of the settings it holds; the others keep theirs.
fn load_template(description: &Description, fields: &mut [Field], template: &Template) {
    let value = |name: String| {
        let found = template
            .values
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(&name));
        found.map(|(_, v)| v.clone())
    };
    for f in fields.iter_mut().filter(|f| !f.setting.per_file) {
        let Some(text) = value(description.attribute(&f.setting.key)) else {
            continue;
        };
        f.preset = value(description.preset_attribute(&f.setting.key))
            .filter(|name| f.setting.presets && !name.is_empty())
            .map(|name| (name, text.trim().to_string()));
        f.text = text;
    }
}

/// Keeps the values of the template settings under a name, replacing a template of that name
/// in its place, or adding it last; returns the name as kept.
fn save_template(
    description: &Description,
    fields: &[Field],
    templates: &mut Vec<Template>,
    name: &str,
) -> String {
    let mine = fields.iter().filter(|f| !f.setting.per_file);
    let values = attribute_values(description, mine);
    let template = Template::new(&description.name, name, &values);
    let name = template.name.clone();
    match templates.iter_mut().find(|t| t.name == name) {
        Some(t) => *t = template,
        None => templates.push(template),
    }
    name
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
fn attribute_values<'a>(
    description: &Description,
    fields: impl IntoIterator<Item = &'a Field>,
) -> Vec<(String, String)> {
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
        let pairs = vec![("plugin_example_limit".to_string(), "1024000".to_string())];
        settings.remember_values(&file, &pairs);
        // reading the file again: no attribute in the data, the remembered value holds
        session
            .run(&format!("GET FILE='{}'.", file.display()), None)
            .unwrap();
        let all = attributes(&settings, session.dataset());
        assert_eq!(
            first_value(&all, "plugin_example_limit").as_deref(),
            Some("1024000")
        );
        // an attribute of the data wins
        session
            .run(
                "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_limit('500').",
                None,
            )
            .unwrap();
        let all = attributes(&settings, session.dataset());
        assert_eq!(
            first_value(&all, "plugin_example_limit").as_deref(),
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
            answers: Vec::new(),
            errors: Vec::new(),
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
            per_file: false,
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
                key: "limit".into(),
                label: "Upper limit".into(),
                kind: Kind::Number,
                default: 0.into(),
                presets: true,
                per_file: false,
                group: None,
            }],
        };
        assert_eq!(
            description.preset_attribute("limit"),
            "plugin_house_limit_preset"
        );
        let mut field = Field {
            setting: description.settings[0].clone(),
            text: "1043000".into(),
            preset: Some(("Strict".into(), "1043000".into())),
            new_name: String::new(),
            style: None,
        };
        let syntax = attribute_syntax(&attribute_values(
            &description,
            std::slice::from_ref(&field),
        ));
        assert_eq!(
            syntax,
            "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_house_limit('1043000') \
             plugin_house_limit_preset('Strict')."
        );
        let mut session = statscraft_engine::session::Session::default();
        session
            .run("DATA LIST LIST /x (F1).\nBEGIN DATA\n1\nEND DATA.", None)
            .unwrap();
        session.run(&syntax, None).unwrap();
        let stored = attribute(session.dataset(), "plugin_house_limit_preset");
        assert_eq!(stored.as_deref(), Some("Strict"));
        field.text = "990000".into();
        assert_eq!(field.preset(), None);
        let syntax = attribute_syntax(&attribute_values(&description, &[field]));
        assert!(
            syntax.ends_with("plugin_house_limit_preset('')."),
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
                per_file: false,
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

    /// A plugin with a colour and a limit with presets in the template, and a universe size
    /// kept per data file.
    fn client_plugin() -> Description {
        let setting = |key: &str, kind, default: Value, presets, per_file| plugin::Setting {
            key: key.into(),
            label: key.into(),
            kind,
            default,
            presets,
            per_file,
            group: None,
        };
        Description {
            name: "House".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![
                setting("high", Kind::Colour, "#ffffff".into(), false, false),
                setting("limit", Kind::Number, 0.into(), true, false),
                setting("universe", Kind::Number, 0.into(), false, true),
            ],
        }
    }

    /// With a template active, its values win for the template settings, a setting it lacks
    /// keeps the data's value, and a per-file setting keeps the data's value even when the
    /// template names it. Without a template the data's values hold, as before.
    #[test]
    fn the_active_template_wins_for_its_settings_only() {
        let description = client_plugin();
        let data = |pairs: &[(&str, &str)]| -> Vec<(String, Vec<String>)> {
            let pairs = pairs.iter();
            pairs
                .map(|(a, v)| (a.to_string(), vec![v.to_string()]))
                .collect()
        };
        let known = data(&[
            ("plugin_house_high", "#000000"),
            ("PLUGIN_HOUSE_LIMIT", "5"),
            ("plugin_house_limit_preset", "Loose"),
            ("plugin_house_universe", "1200"),
        ]);
        let pairs = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(a, v)| (a.to_string(), v.to_string()))
                .collect()
        };
        let template = Template::new(
            "House",
            "Client A",
            &pairs(&[
                ("plugin_house_limit", "9"),
                ("plugin_house_limit_preset", "Strict"),
                ("plugin_house_universe", "99"),
            ]),
        );
        let values = |template| {
            let all = with_template(&description, known.clone(), template);
            (
                description.values(|name| first_value(&all, name)),
                first_value(&all, "plugin_house_limit_preset"),
            )
        };
        let (with, preset) = values(Some(&template));
        assert_eq!(with["limit"], 9.0);
        assert_eq!(preset.as_deref(), Some("Strict"));
        assert_eq!(with["high"], "#000000");
        assert_eq!(with["universe"], 1200.0);
        let (without, preset) = values(None);
        assert_eq!(without["limit"], 5.0);
        assert_eq!(preset.as_deref(), Some("Loose"));
        assert_eq!(without["universe"], 1200.0);
        // nothing in the data: the template, then the default
        let all = with_template(&description, Vec::new(), Some(&template));
        let values = description.values(|name| first_value(&all, name));
        assert_eq!(
            (&values["limit"], &values["high"]),
            (&9.0.into(), &"#ffffff".into())
        );
    }

    /// Choosing a template fills the template fields; OK makes it active, keeps the edited
    /// values in it (not the per-file one) and still writes every value to the data.
    #[test]
    fn ok_activates_and_updates_the_chosen_template() {
        let description = client_plugin();
        let field = |s: &plugin::Setting, text: &str| Field {
            setting: s.clone(),
            text: text.into(),
            preset: None,
            new_name: String::new(),
            style: None,
        };
        let s = &description.settings;
        let mut fields = vec![
            field(&s[0], "#000000"),
            field(&s[1], "5"),
            field(&s[2], "1200"),
        ];
        let mut templates = Vec::new();
        let name = save_template(&description, &fields, &mut templates, " Client\tA ");
        assert_eq!(name, "Client A");
        assert_eq!(templates[0].values.len(), 3, "{:?}", templates[0].values);
        let b = [
            ("plugin_house_high", "#ed7d31"),
            ("plugin_house_limit", "9"),
        ];
        let b = b.map(|(a, v)| (a.to_string(), v.to_string()));
        templates.push(Template::new("", "Client B", &b));
        fields[2].text = "1500".into();
        load_template(&description, &mut fields, &templates[1]);
        let texts: Vec<&str> = fields.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(texts, ["#ed7d31", "9", "1500"]);
        // edited after choosing: the template takes the edit on OK
        fields[1].text = "10".into();
        let dialog = Dialog {
            description: description.clone(),
            fields,
            templates,
            chosen: Some("Client B".into()),
            new_name: String::new(),
        };
        // another plugin's template and choice stay as they are
        let other = Template::new("Heat", "Client B", &[]);
        let mut settings = Settings {
            templates: vec![other.clone()],
            active_templates: vec![("Heat".into(), "Client B".into())],
            ..Settings::default()
        };
        let (syntax, _) = dialog.finish(&mut settings);
        let active = settings.active_template(&description).unwrap();
        let names = (active.plugin.as_str(), active.name.as_str());
        assert_eq!(names, ("House", "Client B"));
        assert_eq!(
            active.values,
            [
                ("plugin_house_high".to_string(), "#ed7d31".to_string()),
                ("plugin_house_limit".to_string(), "10".to_string()),
                ("plugin_house_limit_preset".to_string(), String::new()),
            ]
        );
        assert_eq!(settings.templates.len(), 3);
        assert_eq!(settings.templates[0], other);
        assert_eq!(settings.active_templates.len(), 2);
        assert_eq!(
            syntax,
            "DATAFILE ATTRIBUTE ATTRIBUTE=plugin_house_high('#ed7d31') plugin_house_limit('10') \
             plugin_house_limit_preset('') plugin_house_universe('1500')."
        );
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
            per_file: false,
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

    /// Two plugins decorate the same table in their order: the fill of the first and the bold
    /// of the second both land, and the second's text colour wins. A plugin that cannot start
    /// leaves its error without stopping the others, and one turned off does not run.
    #[cfg(unix)]
    #[test]
    fn two_plugins_decorate_the_same_table() {
        use crate::settings::Plugin;
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("statscraft-plugins-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a plugin as a shell script that styles cell (0, 1) of table 4
        let script = |name: &str, cell: &str| {
            let path = dir.join(name);
            let describe = format!(r#"{{"name": "{name}", "version": "1", "protocol": 1}}"#);
            let answer = format!(
                r#"{{"tables": [{{"id": 4, "cells": [{{"row": 0, "col": 1, {cell}}}]}}]}}"#
            );
            let text = format!(
                "#!/bin/sh\nif [ \"$1\" = --describe ]; then echo '{describe}'; exit 0; fi\n\
                 cat > /dev/null\necho '{answer}'\n"
            );
            std::fs::write(&path, text).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            Plugin { path, on: true }
        };
        let shade = script("shade", r##""fill": "#ff0000", "text": "#000000""##);
        let bold = script("bold", r##""bold": true, "text": "#9c0006""##);
        let off = Plugin {
            on: false,
            ..script("off", r##""fill": "#00ff00""##)
        };
        let missing = Plugin {
            path: dir.join("missing"),
            on: true,
        };
        let settings = Settings {
            plugins: vec![shade, missing, off, bold],
            ..Settings::default()
        };
        let mut table = Table {
            row_headers: 1,
            columns: vec!["".into(), "a".into()],
            rows: vec![vec![
                statscraft_engine::Cell::Text("r".into()),
                statscraft_engine::Cell::Number(1.0),
            ]],
            cell_stats: Some(Box::default()),
            ..Table::default()
        };
        let plugins = Plugins::default();
        plugins.start(&settings, None, (1, "doc"), vec![(4, table.clone())], || {});
        let done = plugins
            .receiver
            .recv_timeout(std::time::Duration::from_secs(60))
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(done.errors.len(), 1, "{:?}", done.errors);
        assert!(done.errors[0].starts_with("missing: "), "{:?}", done.errors);
        assert!(done.errors[0].contains("cannot start"), "{:?}", done.errors);
        let [(4, (cells, style))] = &done.answers[..] else {
            panic!("one answer for table 4: {:?}", done.answers);
        };
        table.set_decorations(cells.clone(), style.clone());
        let d = table.decoration(0, 1).unwrap();
        assert_eq!(d.fill.as_deref(), Some("#ff0000"));
        assert!(d.bold);
        assert_eq!(d.text.as_deref(), Some("#9c0006"));
    }
}
