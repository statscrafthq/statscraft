//! Edit > Settings: every setting of the window in one dialog, by section. A change applies at
//! once and is saved. How tables show variables and values are settings of the session: a
//! choice there runs SET TVARS or SET TNUMBERS like any other command, the session keeps it, and
//! the window saves it to run the same SET when it starts again. AI agents connects Statscraft
//! to the AI apps on this computer (connect_window.rs). Plugins lists the programs that colour
//! and comment tables (docs/plugins.md) and opens each one's own settings.

use crate::connect_window::Connect;
use crate::output::Part;
use crate::settings::{MIOUTPUT, Plugin, Settings};
use crate::themes;
use eframe::egui::{self, Align2, ComboBox, Key, Ui, vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Section {
    #[default]
    Editor,
    GridsAndOutput,
    Updates,
    AiAgents,
    Plugins,
}

impl Section {
    const ALL: [Section; 5] = [
        Section::Editor,
        Section::GridsAndOutput,
        Section::Updates,
        Section::AiAgents,
        Section::Plugins,
    ];

    fn title(self) -> &'static str {
        match self {
            Section::Editor => "Editor",
            Section::GridsAndOutput => "Grids and output",
            Section::Updates => "Updates",
            Section::AiAgents => "AI agents",
            Section::Plugins => "Plugins",
        }
    }
}

/// The session's SET TVARS, TNUMBERS and MIOUTPUT, as SHOW gives them.
pub struct TableLabels {
    pub variables: String,
    pub values: String,
    pub mioutput: String,
}

/// Each choice: the SET subcommand, its keywords and their names in the dialog.
const VARIABLE_CHOICES: (&str, [(&str, &str); 3]) = (
    "TVARS",
    [
        ("LABELS", "Labels"),
        ("NAMES", "Names"),
        ("BOTH", "Names and labels"),
    ],
);
const VALUE_CHOICES: (&str, [(&str, &str); 3]) = (
    "TNUMBERS",
    [
        ("LABELS", "Labels"),
        ("VALUES", "Values"),
        ("BOTH", "Values and labels"),
    ],
);

/// The dialog while `section` is set; closing it clears `section`. Returns the SET command a
/// choice of the session asks for, to run. `plugins` says what each plugin program is, in the
/// order of the settings; Settings... of a plugin puts its program in `open_plugin`.
pub fn show(
    ctx: &egui::Context,
    section: &mut Option<Section>,
    settings: &mut Settings,
    labels: &TableLabels,
    connect: &mut Connect,
    plugins: &[String],
    open_plugin: &mut Option<PathBuf>,
) -> Option<String> {
    let current = section.as_mut()?;
    let mut open = true;
    let mut close = false;
    let mut changed = false;
    let mut run = None;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    for s in Section::ALL {
                        if ui.selectable_value(current, s, s.title()).changed()
                            && s == Section::AiAgents
                        {
                            connect.refresh();
                        }
                    }
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    let agents = *current == Section::AiAgents;
                    let wide = agents || *current == Section::Plugins;
                    ui.set_width(if wide { 560.0 } else { 320.0 });
                    ui.set_min_height(140.0);
                    if agents {
                        connect.ui(ui);
                    }
                    changed = body(ui, *current, settings);
                    if *current == Section::Plugins {
                        changed |= plugin_list(ui, settings, plugins, open_plugin);
                    }
                    if *current == Section::GridsAndOutput {
                        ui.add_space(8.0);
                        if let Some((subcommand, keyword)) = table_labels(ui, labels) {
                            // kept for the next start, and run now like any SET
                            settings.keep_table_labels(subcommand, keyword);
                            changed = true;
                            run = Some(set_syntax(subcommand, keyword));
                        }
                        if let Some(parts) = imputation_output(ui, &labels.mioutput) {
                            settings.keep_mioutput(&parts);
                            changed = true;
                            run = Some(format!("SET MIOUTPUT=[{parts}]."));
                        }
                    }
                });
            });
            ui.separator();
            close = ui.button("Close").clicked() || ui.input(|i| i.key_pressed(Key::Escape));
        });
    if changed {
        // the window works on without them
        let _ = settings.save();
    }
    if close || !open {
        *section = None;
    }
    run
}

/// What a row of the plugin list asks for.
enum Step {
    Up(usize),
    Down(usize),
    Remove(usize),
}

/// The plugin programs in the order they run, each with its name and version (`lines`), its
/// path, whether it runs, buttons to move or remove it and Settings... for its own settings;
/// Add plugin... and whether Excel exports take the comments. True when one of them changed.
fn plugin_list(
    ui: &mut Ui,
    settings: &mut Settings,
    lines: &[String],
    open_plugin: &mut Option<PathBuf>,
) -> bool {
    let mut changed = false;
    let mut step = None;
    let count = settings.plugins.len();
    if count == 0 {
        ui.label("No plugins yet.");
    }
    for (i, plugin) in settings.plugins.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                let line = lines.get(i).map_or("", String::as_str);
                changed |= ui.checkbox(&mut plugin.on, line).changed();
            });
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.weak(plugin.path.display().to_string());
            });
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                if ui.add_enabled(i > 0, egui::Button::new("Up")).clicked() {
                    step = Some(Step::Up(i));
                }
                if ui
                    .add_enabled(i + 1 < count, egui::Button::new("Down"))
                    .clicked()
                {
                    step = Some(Step::Down(i));
                }
                if ui.button("Remove").clicked() {
                    step = Some(Step::Remove(i));
                }
                if ui.button("Settings...").clicked() {
                    *open_plugin = Some(plugin.path.clone());
                }
            });
        });
        ui.add_space(6.0);
    }
    match step {
        Some(Step::Up(i)) => settings.plugins.swap(i - 1, i),
        Some(Step::Down(i)) => settings.plugins.swap(i, i + 1),
        Some(Step::Remove(i)) => {
            settings.plugins.remove(i);
        }
        None => {}
    }
    changed |= step.is_some();
    if ui.button("Add plugin...").clicked()
        && let Some(path) = crate::app::file_dialog(&None).pick_file()
        // the same program twice would only run twice with the same settings
        && !settings.plugins.iter().any(|p| p.path == path)
    {
        settings.plugins.push(Plugin { path, on: true });
        changed = true;
    }
    ui.add_space(8.0);
    let mut comments = !settings.excel_comments_off;
    if ui
        .checkbox(&mut comments, "Write plugin comments as Excel comments")
        .changed()
    {
        settings.excel_comments_off = !comments;
        changed = true;
    }
    ui.add_space(8.0);
    ui.weak(
        "The plugins colour and comment tables in this order; where two style the same thing, \
         the lower one wins. Settings... sets a plugin's values for the active data.",
    );
    changed
}

/// How tables show variables and values; a new choice gives its SET subcommand and keyword.
fn table_labels(ui: &mut Ui, labels: &TableLabels) -> Option<(&'static str, &'static str)> {
    let mut run = None;
    for (title, current, (subcommand, choices)) in [
        ("Variables in tables", &labels.variables, VARIABLE_CHOICES),
        ("Values in tables", &labels.values, VALUE_CHOICES),
    ] {
        ui.label(title);
        let shown = choices
            .iter()
            .find(|(keyword, _)| keyword.eq_ignore_ascii_case(current))
            .map_or(current.as_str(), |(_, name)| name);
        ComboBox::from_id_salt(subcommand)
            .selected_text(shown)
            .width(220.0)
            .show_ui(ui, |ui| {
                for (keyword, name) in choices {
                    let selected = keyword.eq_ignore_ascii_case(current);
                    if ui.selectable_label(selected, name).clicked() && !selected {
                        run = Some((subcommand, keyword));
                    }
                }
            });
        ui.add_space(8.0);
    }
    run
}

/// The parts of output for multiply imputed data; a new choice gives the new keyword list. The
/// last part checked stays, since the command needs at least one.
fn imputation_output(ui: &mut Ui, current: &str) -> Option<String> {
    let upper = current.to_uppercase();
    let mut on = MIOUTPUT.map(|k| {
        upper
            .split(|c: char| !c.is_ascii_alphabetic())
            .any(|w| w == k)
    });
    let before = on;
    ui.label("Output for multiply imputed data");
    ui.horizontal_wrapped(|ui| {
        let checked = on.iter().filter(|o| **o).count();
        for ((keyword, name), part) in MIOUTPUT.iter().zip(MI_NAMES).zip(on.iter_mut()) {
            let last = *part && checked == 1;
            ui.add_enabled(!last, egui::Checkbox::new(part, name))
                .on_hover_text(format!("SET MIOUTPUT {keyword}"))
                .on_disabled_hover_text("At least one part is shown");
        }
    });
    (on != before).then(|| {
        let parts: Vec<&str> = MIOUTPUT
            .iter()
            .zip(on)
            .filter(|(_, o)| *o)
            .map(|(k, _)| *k)
            .collect();
        parts.join(" ")
    })
}

const MI_NAMES: [&str; 4] = ["Original data", "Each imputation", "Pooled", "Diagnostics"];

fn set_syntax(subcommand: &str, keyword: &str) -> String {
    format!("SET {subcommand}={keyword}.")
}

/// The settings of one section; true when one of them changed.
fn body(ui: &mut Ui, section: Section, settings: &mut Settings) -> bool {
    let mut changed = false;
    match section {
        Section::Editor => {
            ui.label("Syntax colors");
            let current = themes::by_name(&settings.theme).name;
            ComboBox::from_id_salt("syntax colours")
                .selected_text(current)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for theme in &themes::THEMES {
                        if ui
                            .selectable_label(theme.name == current, theme.name)
                            .clicked()
                        {
                            settings.theme = theme.name.to_string();
                            changed = true;
                        }
                    }
                });
            ui.add_space(8.0);
            let mut wrap = !settings.wrap_off;
            if ui.checkbox(&mut wrap, "Wrap long lines").changed() {
                settings.wrap_off = !wrap;
                changed = true;
            }
        }
        Section::GridsAndOutput => {
            ui.label("Paper of the grids and the output");
            let follow = &mut settings.paper_follows_system;
            changed |= ui.radio_value(follow, false, "Always light").changed();
            changed |= ui
                .radio_value(follow, true, "Follow the system theme")
                .changed();
            ui.add_space(8.0);
            let mut log = !settings.log_hidden;
            if ui
                .checkbox(&mut log, "Log under Syntax and Output")
                .changed()
            {
                settings.log_hidden = !log;
                changed = true;
            }
            ui.add_space(8.0);
            ui.label("Show in the output");
            let output = &mut settings.output;
            for (part, on) in Part::ALL.into_iter().zip(&mut output.kinds) {
                changed |= ui.checkbox(on, part.label()).changed();
            }
            changed |= ui
                .checkbox(&mut output.export_hidden, "Export hidden items")
                .on_hover_text("Exports and copies take what the output does not show too")
                .changed();
        }
        Section::Updates => {
            let mut on = settings.updates != Some(false);
            if ui
                .checkbox(&mut on, "Check for updates automatically")
                .changed()
            {
                settings.updates = Some(on);
                changed = true;
            }
        }
        Section::AiAgents | Section::Plugins => {}
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_runs_its_set_command() {
        assert_eq!(set_syntax(VARIABLE_CHOICES.0, "BOTH"), "SET TVARS=BOTH.");
        assert_eq!(
            set_syntax(VALUE_CHOICES.0, "VALUES"),
            "SET TNUMBERS=VALUES."
        );
        assert_eq!(
            VARIABLE_CHOICES.1.map(|(k, _)| k),
            ["LABELS", "NAMES", "BOTH"]
        );
        assert_eq!(
            VALUE_CHOICES.1.map(|(k, _)| k),
            ["LABELS", "VALUES", "BOTH"]
        );
    }
}
