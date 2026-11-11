//! Edit > Settings: every setting of the window in one dialog, by section. A change applies at
//! once and is saved.

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

/// The dialog while `section` is set; closing it clears `section`.
pub fn show(ctx: &egui::Context, section: &mut Option<Section>, settings: &mut Settings) {
    let Some(current) = section.as_mut() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let mut changed = false;
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
