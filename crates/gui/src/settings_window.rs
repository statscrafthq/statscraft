//! Edit > Settings: every setting of the window in one dialog, by section. A change applies at
//! once and is saved. How tables show variables and values are settings of the session: a
//! choice there runs SET TVARS or SET TNUMBERS like any other command, the session keeps it, and
//! the window saves it to run the same SET when it starts again. AI agents connects Statscraft
//! to the AI apps on this computer (connect_window.rs).

use crate::connect_window::Connect;
use crate::settings::Settings;
use crate::themes;
use eframe::egui::{self, Align2, ComboBox, Key, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Section {
    #[default]
    Editor,
    GridsAndOutput,
    Updates,
    AiAgents,
}

impl Section {
    const ALL: [Section; 4] = [
        Section::Editor,
        Section::GridsAndOutput,
        Section::Updates,
        Section::AiAgents,
    ];

    fn title(self) -> &'static str {
        match self {
            Section::Editor => "Editor",
            Section::GridsAndOutput => "Grids and output",
            Section::Updates => "Updates",
            Section::AiAgents => "AI agents",
        }
    }
}

/// The session's SET TVARS and TNUMBERS, as SHOW gives them.
pub struct TableLabels {
    pub variables: String,
    pub values: String,
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
/// choice of the session asks for, to run.
pub fn show(
    ctx: &egui::Context,
    section: &mut Option<Section>,
    settings: &mut Settings,
    labels: &TableLabels,
    connect: &mut Connect,
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
                    ui.set_width(if agents { 560.0 } else { 320.0 });
                    ui.set_min_height(140.0);
                    if agents {
                        connect.ui(ui);
                    }
                    changed = body(ui, *current, settings);
                    if *current == Section::GridsAndOutput {
                        ui.add_space(8.0);
                        if let Some((subcommand, keyword)) = table_labels(ui, labels) {
                            // kept for the next start, and run now like any SET
                            settings.keep_table_labels(subcommand, keyword);
                            changed = true;
                            run = Some(set_syntax(subcommand, keyword));
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
        Section::AiAgents => {}
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
