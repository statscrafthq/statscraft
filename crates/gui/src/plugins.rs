//! The plugins of Edit > Settings > Plugins (docs/plugins.md): they run in their order on a
//! thread of their own after every run that made tables with cell statistics, and again when
//! their settings change, and their answers merge table by table. The answers wait until no
//! run goes on, then go on the tables of the output window and of the window's document, so a
//! saved document keeps them. A plugin's settings are data file attributes of the active
//! dataset, which its Plugin Settings dialog (Settings... in that list) writes through syntax.

use crate::settings::{PluginCheck, Preset, Settings, Template, active_template};
use crate::syntax_text::quote;
use crate::windows::WindowId;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;
use statscraft_engine::plugin::{self, Description, Kind, Update, Value};
use statscraft_engine::plugin_template::{TemplateFile, with_template};
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
    /// Plugin runs started and not answered yet.
    running: usize,
    /// The programs whose update check or update runs.
    busy: Vec<PathBuf>,
    /// What the last update check or update of a program in this session did.
    status: Vec<(PathBuf, String)>,
    update_sender: mpsc::Sender<UpdateEvent>,
    update_receiver: mpsc::Receiver<UpdateEvent>,
}

/// A result of a plugin's update check or update, from its thread.
enum UpdateEvent {
    Checked(PathBuf, Result<Option<Update>, String>),
    /// The version the plugin describes itself with after the update.
    Installed(PathBuf, Result<String, String>),
}

/// What Edit > Settings shows in a plugin's row.
#[derive(Debug, Default, PartialEq)]
pub struct Row {
    /// The name and version, or why the plugin cannot be used.
    pub line: String,
    /// The plugin offers updates, so it has Check now.
    pub updates: bool,
    /// Its update check or update runs.
    pub busy: bool,
    /// A newer version to install, with its notes line.
    pub available: Option<(String, String)>,
    /// What its last update check or update did.
    pub status: Option<String>,
}

/// Whether a plugin last checked for updates at `checked` (seconds since 1970) checks again
/// now: never checked, or a day or more ago (or the clock went back as far).
pub fn check_due(checked: Option<u64>, now: u64) -> bool {
    checked.is_none_or(|t| now.abs_diff(t) >= crate::updates::DAY)
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
    /// What the last Export or Import said when it failed; empty otherwise.
    note: String,
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
        let (update_sender, update_receiver) = mpsc::channel();
        Plugins {
            described: Vec::new(),
            sender,
            receiver,
            waiting: Vec::new(),
            redo: false,
            generation: 0,
            dialog: None,
            running: 0,
            busy: Vec::new(),
            status: Vec::new(),
            update_sender,
            update_receiver,
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
    /// used, and its updates. An update found earlier shows only while it is newer than the
    /// plugin.
    pub fn row(&mut self, program: &Path, settings: &Settings) -> Row {
        let busy = self.busy.iter().any(|p| p == program);
        let status = self.status.iter().find(|(p, _)| p == program);
        let status = status.map(|(_, s)| s.clone());
        let d = match self.description(program) {
            Ok(d) => d,
            Err(e) => {
                let line = format!("Not usable: {e}");
                return Row {
                    line,
                    busy,
                    status,
                    ..Row::default()
                };
            }
        };
        let found = settings
            .plugin_check(program)
            .and_then(|c| c.update.as_ref());
        let available = found
            .filter(|u| d.updates && plugin::newer(&u.version, &d.version))
            .map(|u| (u.version.clone(), u.notes.clone()));
        Row {
            line: format!("{} {}", d.name, d.version),
            updates: d.updates,
            busy,
            available,
            status,
        }
    }

    /// Starts an update check of every plugin that offers updates and has not checked for a
    /// day, each on a thread of its own; `ctx` repaints when one is done.
    pub fn checks_due(&mut self, settings: &Settings, ctx: &egui::Context) {
        let now = crate::updates::now();
        for p in &settings.plugins {
            let offers = matches!(self.description(&p.path), Ok(d) if d.updates);
            let checked = settings.plugin_check(&p.path).map(|c| c.checked);
            if offers && check_due(checked, now) {
                self.check(&p.path, ctx);
            }
        }
    }

    /// Starts an update check of a plugin on a thread of its own, unless one runs.
    pub fn check(&mut self, program: &Path, ctx: &egui::Context) {
        if self.busy.iter().any(|p| p == program) {
            return;
        }
        self.busy.push(program.to_path_buf());
        let (sender, ctx, program) = (self.update_sender.clone(), ctx.clone(), program.to_owned());
        std::thread::spawn(move || {
            let found = plugin::update_check(&program);
            let _ = sender.send(UpdateEvent::Checked(program, found));
            ctx.request_repaint();
        });
    }

    /// Installs a plugin's update on a thread of its own and asks the plugin its version
    /// afterwards, unless a check or update of it runs.
    pub fn install(&mut self, program: &Path, update: Update, ctx: &egui::Context) {
        if self.busy.iter().any(|p| p == program) {
            return;
        }
        self.busy.push(program.to_path_buf());
        let (sender, ctx, program) = (self.update_sender.clone(), ctx.clone(), program.to_owned());
        std::thread::spawn(move || {
            let done = plugin::install_update(&program, &update)
                .and_then(|()| plugin::describe(&program).map(|d| d.version));
            let _ = sender.send(UpdateEvent::Installed(program, done));
            ctx.request_repaint();
        });
    }

    /// Takes the update checks and updates that finished: a check is kept in the settings
    /// (saved), and after an update every plugin is described and every table decorated again.
    pub fn receive_updates(&mut self, settings: &mut Settings) {
        let events: Vec<UpdateEvent> = self.update_receiver.try_iter().collect();
        for event in events {
            let (program, status, update) = match event {
                UpdateEvent::Checked(program, found) => {
                    let status = match &found {
                        Ok(Some(_)) => None,
                        Ok(None) => Some("Up to date.".to_string()),
                        Err(e) => Some(format!("The update check failed: {e}")),
                    };
                    (program, status, found.ok().flatten())
                }
                UpdateEvent::Installed(program, done) => {
                    let status = match &done {
                        Ok(version) => format!("Updated to {version}."),
                        Err(e) => format!("The update failed: {e}"),
                    };
                    let kept = settings
                        .plugin_check(&program)
                        .and_then(|c| c.update.clone());
                    // a failed update stays on offer
                    let update = if done.is_ok() { None } else { kept };
                    if done.is_ok() {
                        self.changed();
                    }
                    (program, Some(status), update)
                }
            };
            self.busy.retain(|p| *p != program);
            self.status.retain(|(p, _)| *p != program);
            if let Some(status) = status {
                self.status.push((program.clone(), status));
            }
            settings.keep_check(PluginCheck {
                path: program,
                checked: crate::updates::now(),
                update,
            });
            // the window works on without them
            let _ = settings.save();
        }
    }

    /// Starts the plugins that are on, in their order, on the tables of an output window that
    /// have cell statistics, on a thread of its own, which also asks each for its settings, so
    /// a slow plugin never holds up the window; `wake` is called when they are done. Each
    /// plugin's answer is laid over those before it; one that fails leaves the others' answers
    /// standing. Nothing runs while no plugin is on.
    pub fn start(
        &mut self,
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
        self.running += 1;
        std::thread::spawn(move || {
            // Done always goes back, also when a plugin run panics: a held export counts on it
            let run = std::panic::AssertUnwindSafe(|| {
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
                (answers, errors)
            });
            let (answers, errors) = std::panic::catch_unwind(run).unwrap_or_else(|_| {
                (
                    Vec::new(),
                    vec!["a plugin run stopped with an error".into()],
                )
            });
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
        let done: Vec<Done> = self.receiver.try_iter().collect();
        self.running = self.running.saturating_sub(done.len());
        let current = done.into_iter().filter(|d| d.generation == generation);
        self.waiting.extend(current);
    }

    /// Whether plugin runs are still out, whose answers have not reached the tables.
    pub fn running(&self) -> bool {
        self.running > 0 || !self.waiting.is_empty()
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
            note: String::new(),
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
        let folder = settings.folder.clone();
        let presets = &mut settings.presets;
        let Dialog {
            description,
            fields,
            templates,
            chosen,
            new_name,
            note,
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
                    let export = chosen.is_some() && all_valid;
                    if ui
                        .add_enabled(export, egui::Button::new("Export..."))
                        .on_hover_text(
                            "Write the chosen template and this plugin's presets to a file, \
                             for statscraft --plugin-template or for Import on another computer",
                        )
                        .clicked()
                        && let Some(name) = chosen.as_deref()
                        && let Some(path) = crate::app::file_dialog(&folder)
                            .set_file_name(format!("{name}.txt"))
                            .add_filter("Plugin template", &["txt"])
                            .save_file()
                    {
                        let text = export_file(description, fields, name, presets).text();
                        *note = match std::fs::write(&path, text) {
                            Ok(()) => String::new(),
                            Err(e) => format!("{}: {e}", path.display()),
                        };
                    }
                    if ui.button("Import...").clicked()
                        && let Some(path) = crate::app::file_dialog(&folder)
                            .add_filter("Plugin template", &["txt"])
                            .pick_file()
                    {
                        let read = std::fs::read_to_string(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|text| TemplateFile::parse(&text))
                            .and_then(|file| {
                                import_file(description, file, fields, templates, presets)
                            });
                        *note = match read {
                            Ok(name) => {
                                if name.is_some() {
                                    *chosen = name;
                                }
                                String::new()
                            }
                            Err(e) => format!("{}: {e}", path.display()),
                        };
                    }
                });
                if !note.is_empty() {
                    ui.colored_label(ui.visuals().error_fg_color, note.as_str());
                }
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

/// What Export writes: the template as the dialog holds it now under `name`, and the saved
/// values of this plugin's settings.
fn export_file(
    description: &Description,
    fields: &[Field],
    name: &str,
    presets: &[Preset],
) -> TemplateFile {
    let mine = fields.iter().filter(|f| !f.setting.per_file);
    let values = attribute_values(description, mine);
    let attributes = preset_attributes(description);
    TemplateFile {
        templates: vec![Template::new(&description.name, name, &values)],
        presets: presets
            .iter()
            .filter(|p| attributes.contains(&p.attribute))
            .map(|p| (p.attribute.clone(), p.name.clone(), p.value.clone()))
            .collect(),
    }
}

/// The attributes of the settings that offer presets.
fn preset_attributes(description: &Description) -> Vec<String> {
    let offers = description.settings.iter().filter(|s| s.presets);
    offers.map(|s| description.attribute(&s.key)).collect()
}

/// Import: the file's templates join the dialog's, one of the same name replaced, and its
/// presets join the saved ones; the first template is loaded into the fields and its name
/// returned. Nothing changes when the file holds something that is not for this plugin.
fn import_file(
    description: &Description,
    file: TemplateFile,
    fields: &mut [Field],
    templates: &mut Vec<Template>,
    presets: &mut Vec<Preset>,
) -> Result<Option<String>, String> {
    if let Some(t) = file.templates.iter().find(|t| !t.belongs_to(description)) {
        return Err(format!(
            "the template {} is not for {}",
            t.name, description.name
        ));
    }
    let attributes = preset_attributes(description);
    if let Some(p) = file.presets.iter().find(|p| !attributes.contains(&p.0)) {
        return Err(format!(
            "the preset {} is for {}, which {} does not offer presets for",
            p.1, p.0, description.name
        ));
    }
    for (attribute, name, value) in &file.presets {
        let preset = Preset::new(attribute, name, value);
        presets.retain(|p| !(p.attribute == preset.attribute && p.name == preset.name));
        presets.push(preset);
    }
    let mut first = None;
    for t in &file.templates {
        let t = Template::new(&description.name, &t.name, &t.values);
        first.get_or_insert_with(|| t.name.clone());
        match templates.iter_mut().find(|x| x.name == t.name) {
            Some(x) => *x = t,
            None => templates.push(t),
        }
    }
    if let Some(t) = first
        .as_ref()
        .and_then(|name| templates.iter().find(|t| &t.name == name))
    {
        load_template(description, fields, t);
    }
    Ok(first)
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
        match f.setting.choices.is_empty() {
            true => field(ui, f.setting.kind, &mut f.text),
            false => choice(ui, &f.setting.key, &f.setting.choices, &mut f.text),
        }
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

/// A text setting with choices: a list of them, the stored text shown even when it is none.
fn choice(ui: &mut Ui, key: &str, choices: &[String], text: &mut String) {
    egui::ComboBox::from_id_salt(("plugin choice", key))
        .selected_text(text.as_str())
        .show_ui(ui, |ui| {
            for c in choices {
                ui.selectable_value(text, c.clone(), c);
            }
        });
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

    #[test]
    fn an_update_check_is_due_once_a_day() {
        let day = crate::updates::DAY;
        let now = 1_790_000_000;
        assert!(check_due(None, now));
        assert!(!check_due(Some(now), now));
        assert!(!check_due(Some(now - day + 1), now));
        assert!(check_due(Some(now - day), now));
        // a clock set back a day or more checks again too
        assert!(check_due(Some(now + day), now));
    }

    /// A row shows an update found earlier only while it is newer than the plugin, and only
    /// for a plugin that offers updates.
    #[cfg(unix)]
    #[test]
    fn a_row_offers_a_newer_version_only() {
        use crate::settings::Plugin;
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("statscraft-rows-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = |name: &str, updates: bool| {
            let path = dir.join(name);
            let describe = format!(
                r#"{{"name": "{name}", "version": "1.2.0", "protocol": 1, "updates": {updates}}}"#
            );
            std::fs::write(&path, format!("#!/bin/sh\necho '{describe}'\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let (offers, silent) = (script("offers", true), script("silent", false));
        let found = |path: &Path, version: &str| PluginCheck {
            path: path.to_path_buf(),
            checked: 1,
            update: Some(Update {
                version: version.into(),
                url: "https://example.com/p".into(),
                sha256: "ab".repeat(32),
                notes: "Faster".into(),
            }),
        };
        let mut settings = Settings {
            plugins: [&offers, &silent]
                .map(|p| Plugin {
                    path: p.clone(),
                    on: true,
                })
                .into(),
            plugin_checks: vec![found(&offers, "1.3"), found(&silent, "1.3")],
            ..Settings::default()
        };
        let mut plugins = Plugins::default();
        let row = plugins.row(&offers, &settings);
        assert_eq!(row.line, "offers 1.2.0");
        assert!(row.updates && !row.busy);
        assert_eq!(row.available, Some(("1.3".into(), "Faster".into())));
        assert_eq!(plugins.row(&silent, &settings).available, None);
        // the plugin was updated by hand meanwhile
        settings.keep_check(found(&offers, "1.2"));
        assert_eq!(plugins.row(&offers, &settings).available, None);
        let missing = plugins.row(&dir.join("missing"), &settings);
        assert!(missing.line.starts_with("Not usable"), "{}", missing.line);
        std::fs::remove_dir_all(dir).unwrap();
    }

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
            choices: Vec::new(),
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
            updates: false,
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
            updates: false,
            settings: vec![plugin::Setting {
                choices: Vec::new(),
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
            updates: false,
            settings: vec![plugin::Setting {
                choices: Vec::new(),
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
            choices: Vec::new(),
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
            updates: false,
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
            note: String::new(),
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
            choices: Vec::new(),
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
        let mut plugins = Plugins::default();
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

    #[test]
    fn export_then_import_carries_a_template_and_its_presets_to_another_dialog() {
        let description = Description {
            name: "House".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![plugin::Setting {
                choices: Vec::new(),
                key: "limit".into(),
                label: "Limit".into(),
                kind: Kind::Text,
                default: "".into(),
                presets: true,
                per_file: false,
                group: None,
            }],
            updates: false,
        };
        let field = |text: &str| Field {
            setting: description.settings[0].clone(),
            text: text.into(),
            preset: None,
            new_name: String::new(),
            style: None,
        };
        let attribute = description.attribute("limit");
        let presets = vec![
            Preset::new(&attribute, "Strict", "5"),
            Preset::new("plugin_other_limit", "Not mine", "1"),
        ];
        let file = export_file(&description, &[field("5")], "Client A", &presets);
        // only this plugin's presets leave, and the file reads back as written
        assert_eq!(file.presets.len(), 1);
        let file = TemplateFile::parse(&file.text()).unwrap();
        let mut fields = [field("")];
        let mut templates = vec![Template::new("House", "Client A", &[])];
        let mut kept = vec![Preset::new(&attribute, "Strict", "9")];
        let chosen = import_file(&description, file, &mut fields, &mut templates, &mut kept);
        assert_eq!(chosen, Ok(Some("Client A".into())));
        // the template of the same name was replaced and its values are in the fields
        assert_eq!(templates.len(), 1);
        assert_eq!(fields[0].text, "5");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].value, "5");
        // a template of another plugin changes nothing
        let other = TemplateFile::parse("plugin-template Heat\tX\tplugin_heat_low\t1\n").unwrap();
        let before = templates.clone();
        let result = import_file(&description, other, &mut fields, &mut templates, &mut kept);
        assert!(result.is_err());
        assert_eq!(templates, before);
    }
}
