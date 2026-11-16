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
use statscraft_engine::plugin::{self, Description, Kind};
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
fn attribute(data: Option<&Dataset>, name: &str) -> Option<String> {
    first_value(data?.attributes(), name)
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
        let attributes = data.map(|d| d.attributes().to_vec()).unwrap_or_default();
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
        let values = description.values(|name| attribute(data, name));
        let fields = description.settings.iter().map(|s| {
            let text = match &values[&s.key] {
                plugin::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let preset = attribute(data, &description.preset_attribute(&s.key))
                .filter(|name| s.presets && !name.is_empty())
                .map(|name| (name, text.trim().to_string()));
            Field {
                setting: s.clone(),
                text,
                preset,
                new_name: String::new(),
            }
        });
        self.dialog = Some((description.clone(), fields.collect()));
        Ok(())
    }

    /// Edit > Plugin Settings while it is open; returns the syntax that keeps the values as
    /// data file attributes when OK is pressed. Presets saved or removed change `presets`.
    pub fn dialog_ui(&mut self, ctx: &egui::Context, presets: &mut Vec<Preset>) -> Option<String> {
        let (description, fields) = self.dialog.as_mut()?;
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Plugin Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.weak("Kept with the active data: save the data file to keep them.");
                ui.add_space(6.0);
                egui::Grid::new("plugin settings").show(ui, |ui| {
                    for f in fields.iter_mut() {
                        ui.label(&f.setting.label);
                        field(ui, f.setting.kind, &mut f.text);
                        if f.setting.presets && matches!(f.setting.kind, Kind::Number | Kind::Text)
                        {
                            let attribute = description.attribute(&f.setting.key);
                            ui.horizontal(|ui| preset_ui(ui, f, presets, &attribute));
                        }
                        ui.end_row();
                    }
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
        if !(ok || cancel || !open) {
            return None;
        }
        // the description the dialog was opened with: the program may have changed since
        let (description, fields) = self.dialog.take()?;
        ok.then(|| attribute_syntax(&description, &fields))
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
        Kind::Number | Kind::Text => {
            ui.text_edit_singleline(text);
        }
    }
}

fn valid(kind: Kind, text: &str) -> bool {
    kind != Kind::Number || text.trim().parse::<f64>().is_ok()
}

/// DATAFILE ATTRIBUTE that keeps every value under its attribute name, and for a setting with
/// presets the name of the preset its value came from (empty when none).
fn attribute_syntax(description: &Description, fields: &[Field]) -> String {
    let mut values = Vec::new();
    for f in fields {
        let key = &f.setting.key;
        let text = quote(f.text.trim());
        values.push(format!("{}({text})", description.attribute(key)));
        if f.setting.presets {
            let preset = quote(f.preset().unwrap_or(""));
            values.push(format!("{}({preset})", description.preset_attribute(key)));
        }
    }
    format!("DATAFILE ATTRIBUTE ATTRIBUTE={}.", values.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            })
            .collect();
        let syntax = attribute_syntax(&description, &fields);
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
        };
        let syntax = attribute_syntax(&description, std::slice::from_ref(&field));
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
        let syntax = attribute_syntax(&description, &[field]);
        assert!(
            syntax.ends_with("plugin_house_universe_preset('')."),
            "{syntax}"
        );
    }
}
