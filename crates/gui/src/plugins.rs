//! The plugin of Edit > Settings > Plugins (docs/plugins.md): it runs on a thread of its own
//! after every run that made tables with cell statistics, and again when its settings change.
//! Its answers wait until no run goes on, then go on the tables of the output window and of
//! the window's document, so a saved document keeps them. Its settings are data file
//! attributes of the active dataset, which Edit > Plugin Settings writes through syntax.

use crate::settings::Settings;
use crate::syntax_text::quote;
use crate::windows::WindowId;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;
use statscraft_engine::plugin::{self, Description, Kind};
use statscraft_engine::{Decoration, Table, parse_colour};
use std::path::PathBuf;
use std::sync::mpsc;

/// A plugin's answer for the tables of one output window.
pub struct Done {
    pub window: WindowId,
    pub document: String,
    /// The settings generation the answer was asked under; an older one is dropped.
    pub generation: u64,
    pub result: Result<Vec<(u64, Vec<Decoration>)>, String>,
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
    pub dialog: Option<(Description, Vec<(plugin::Setting, String)>)>,
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
            (s.clone(), text)
        });
        self.dialog = Some((description.clone(), fields.collect()));
        Ok(())
    }

    /// Edit > Plugin Settings while it is open; returns the syntax that keeps the values as
    /// data file attributes when OK is pressed.
    pub fn dialog_ui(&mut self, ctx: &egui::Context) -> Option<String> {
        let (_, fields) = self.dialog.as_mut()?;
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Plugin Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.weak("Kept with the active data: save the data file to keep them.");
                ui.add_space(6.0);
                egui::Grid::new("plugin settings").show(ui, |ui| {
                    for (setting, text) in fields.iter_mut() {
                        ui.label(&setting.label);
                        field(ui, setting.kind, text);
                        ui.end_row();
                    }
                });
                let bad = fields.iter().find(|(s, t)| !valid(s.kind, t));
                if let Some((setting, _)) = bad {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("{}: not a number", setting.label),
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

/// DATAFILE ATTRIBUTE that keeps every value under its attribute name.
fn attribute_syntax(description: &Description, fields: &[(plugin::Setting, String)]) -> String {
    let values: Vec<String> = fields
        .iter()
        .map(|(s, text)| format!("{}({})", description.attribute(&s.key), quote(text.trim())))
        .collect();
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
        let fields: Vec<(plugin::Setting, String)> = description
            .settings
            .iter()
            .cloned()
            .zip(["0.1".to_string(), "it's".to_string()])
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
}
