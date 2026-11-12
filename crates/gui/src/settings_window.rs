//! Edit > Settings: every setting of the window in one dialog, by section. A change applies at
//! once and is saved. How tables show variables and values are settings of the session: a
//! choice there runs SET TVARS or SET TNUMBERS like any other command, and the session keeps it.

use crate::settings::Settings;
use crate::themes;
use eframe::egui::{self, Align2, ComboBox, Key, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Section {
    #[default]
    Editor,
    GridsAndOutput,
    Updates,
}

impl Section {
    const ALL: [Section; 3] = [Section::Editor, Section::GridsAndOutput, Section::Updates];

    fn title(self) -> &'static str {
        match self {
            Section::Editor => "Editor",
            Section::GridsAndOutput => "Grids and output",
            Section::Updates => "Updates",
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
                        ui.selectable_value(current, s, s.title());
                    }
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_width(320.0);
                    ui.set_min_height(140.0);
                    changed = body(ui, *current, settings);
                    if *current == Section::GridsAndOutput {
                        ui.add_space(8.0);
                        run = table_labels(ui, labels);
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

/// How tables show variables and values; a new choice gives its SET command.
fn table_labels(ui: &mut Ui, labels: &TableLabels) -> Option<String> {
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
                        run = Some(set_syntax(subcommand, keyword));
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
            ui.label("Syntax colours");
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
