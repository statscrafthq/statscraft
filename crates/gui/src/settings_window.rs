//! Edit > Settings: every setting of the window in one dialog, by section. A change applies at
//! once and is saved. How tables show variables and values are settings of the session: a
//! choice there runs SET TVARS or SET TNUMBERS like any other command, the session keeps it, and
//! the window saves it to run the same SET when it starts again. AI agents connects Statscraft
//! to the AI apps on this computer (connect_window.rs). Plugins lists the programs that colour
//! and comment tables (docs/plugins.md) and opens each one's own settings. Language sets the
//! language of the window (i18n.rs); it applies at once.

use crate::connect_window::Connect;
use crate::i18n::{self, LANGUAGES, t, t_args};
use crate::output::Part;
use crate::plugins::Row;
use crate::settings::{MIOUTPUT, Plugin, Settings};
use crate::themes;
use eframe::egui::{self, Align2, ComboBox, Key, Ui, vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Section {
    Language,
    #[default]
    Editor,
    GridsAndOutput,
    Updates,
    AiAgents,
    Plugins,
}

impl Section {
    const ALL: [Section; 6] = [
        Section::Language,
        Section::Editor,
        Section::GridsAndOutput,
        Section::Updates,
        Section::AiAgents,
        Section::Plugins,
    ];

    fn title(self) -> &'static str {
        match self {
            Section::Language => t("settings-language"),
            Section::Editor => t("settings-editor"),
            Section::GridsAndOutput => t("settings-grids-and-output"),
            Section::Updates => t("settings-updates"),
            Section::AiAgents => t("settings-ai-agents"),
            Section::Plugins => t("settings-plugins"),
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
type Choices = (&'static str, [(&'static str, &'static str); 3]);

fn variable_choices() -> Choices {
    (
        "TVARS",
        [
            ("LABELS", t("settings-labels")),
            ("NAMES", t("settings-names")),
            ("BOTH", t("settings-names-and-labels")),
        ],
    )
}

fn value_choices() -> Choices {
    (
        "TNUMBERS",
        [
            ("LABELS", t("settings-labels")),
            ("VALUES", t("settings-values")),
            ("BOTH", t("settings-values-and-labels")),
        ],
    )
}

/// What a button of a plugin's row asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginAction {
    /// Settings...: its Plugin Settings dialog.
    Settings,
    /// Check now: an update check.
    Check,
    /// Update: install the newer version found.
    Update,
}

/// The dialog while `section` is set; closing it clears `section`. Returns the SET command a
/// choice of the session asks for, to run. `plugins` says what each plugin program is, in the
/// order of the settings; a button of a plugin's row puts its program and what it asks for
/// in `action`.
pub fn show(
    ctx: &egui::Context,
    section: &mut Option<Section>,
    settings: &mut Settings,
    labels: &TableLabels,
    connect: &mut Connect,
    plugins: &[Row],
    action: &mut Option<(PathBuf, PluginAction)>,
) -> Option<String> {
    let current = section.as_mut()?;
    let mut open = true;
    let mut close = false;
    let mut changed = false;
    let mut run = None;
    egui::Window::new(t("settings-title"))
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
                        changed |= plugin_list(ui, settings, plugins, action);
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
            close =
                ui.button(t("button-close")).clicked() || ui.input(|i| i.key_pressed(Key::Escape));
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

/// The plugin programs in the order they run, each with its name and version (`rows`), its
/// path, whether it runs, buttons to move or remove it, Settings... for its own settings, and
/// for a plugin that offers updates Check now, a newer version with Update, and what the last
/// check or update did; Add plugin... and whether Excel exports take the comments. True when
/// one of them changed.
fn plugin_list(
    ui: &mut Ui,
    settings: &mut Settings,
    rows: &[Row],
    action: &mut Option<(PathBuf, PluginAction)>,
) -> bool {
    let mut changed = false;
    let mut step = None;
    let count = settings.plugins.len();
    if count == 0 {
        ui.label(t("settings-no-plugins"));
    }
    for (i, plugin) in settings.plugins.iter_mut().enumerate() {
        let row = rows.get(i);
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                let line = row.map_or("", |r| r.line.as_str());
                changed |= ui.checkbox(&mut plugin.on, line).changed();
            });
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.weak(plugin.path.display().to_string());
            });
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                if ui
                    .add_enabled(i > 0, egui::Button::new(t("settings-up")))
                    .clicked()
                {
                    step = Some(Step::Up(i));
                }
                if ui
                    .add_enabled(i + 1 < count, egui::Button::new(t("settings-down")))
                    .clicked()
                {
                    step = Some(Step::Down(i));
                }
                if ui.button(t("settings-remove")).clicked() {
                    step = Some(Step::Remove(i));
                }
                if ui.button(t("settings-plugin-settings")).clicked() {
                    *action = Some((plugin.path.clone(), PluginAction::Settings));
                }
                let Some(row) = row.filter(|r| r.updates) else {
                    return;
                };
                if row.busy {
                    ui.spinner();
                } else if ui.button(t("settings-check-now")).clicked() {
                    *action = Some((plugin.path.clone(), PluginAction::Check));
                }
            });
            if let Some((version, notes)) = row.and_then(|r| r.available.as_ref()) {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(t_args(
                        "settings-version-available",
                        &[("version", version.into())],
                    ));
                    let busy = row.is_some_and(|r| r.busy);
                    if ui
                        .add_enabled(!busy, egui::Button::new(t("settings-update")))
                        .clicked()
                    {
                        *action = Some((plugin.path.clone(), PluginAction::Update));
                    }
                });
                if !notes.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.weak(notes);
                    });
                }
            }
            if let Some(status) = row.and_then(|r| r.status.as_ref()) {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.weak(status);
                });
            }
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
    if ui.button(t("settings-add-plugin")).clicked()
        && let Some(path) = crate::app::file_dialog(&None).pick_file()
        // the same program twice would only run twice with the same settings
        && !settings.plugins.iter().any(|p| p.path == path)
    {
        settings.plugins.push(Plugin { path, on: true });
        changed = true;
    }
    ui.add_space(8.0);
    ui.weak(t("settings-plugins-note"));
    changed
}

/// How tables show variables and values; a new choice gives its SET subcommand and keyword.
fn table_labels(ui: &mut Ui, labels: &TableLabels) -> Option<(&'static str, &'static str)> {
    let mut run = None;
    for (title, current, (subcommand, choices)) in [
        (
            t("settings-variables-in-tables"),
            &labels.variables,
            variable_choices(),
        ),
        (
            t("settings-values-in-tables"),
            &labels.values,
            value_choices(),
        ),
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
    ui.label(t("settings-imputed-output"));
    ui.horizontal_wrapped(|ui| {
        let checked = on.iter().filter(|o| **o).count();
        for ((keyword, name), part) in MIOUTPUT.iter().zip(mi_names()).zip(on.iter_mut()) {
            let last = *part && checked == 1;
            ui.add_enabled(!last, egui::Checkbox::new(part, name))
                .on_hover_text(format!("SET MIOUTPUT {keyword}"))
                .on_disabled_hover_text(t("settings-one-part-shown"));
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

fn mi_names() -> [&'static str; 4] {
    [
        t("settings-mi-original"),
        t("settings-mi-each"),
        t("settings-mi-pooled"),
        t("settings-mi-diagnostics"),
    ]
}

fn set_syntax(subcommand: &str, keyword: &str) -> String {
    format!("SET {subcommand}={keyword}.")
}

/// The settings of one section; true when one of them changed.
fn body(ui: &mut Ui, section: Section, settings: &mut Settings) -> bool {
    let mut changed = false;
    match section {
        Section::Language => {
            ui.label(t("settings-window-language"));
            let current = &mut settings.language;
            let name = |code: &str| match LANGUAGES.iter().find(|l| l.code == code) {
                Some(l) => l.name,
                None => t("settings-language-system"),
            };
            ComboBox::from_id_salt("language")
                .selected_text(name(current))
                .width(220.0)
                .show_ui(ui, |ui| {
                    let codes = std::iter::once("").chain(LANGUAGES.iter().map(|l| l.code));
                    for code in codes {
                        if ui.selectable_label(*current == code, name(code)).clicked()
                            && *current != code
                        {
                            *current = code.to_string();
                            i18n::set_language(code);
                            changed = true;
                        }
                    }
                });
            ui.add_space(8.0);
            ui.weak(t("settings-language-note"));
        }
        Section::Editor => {
            ui.label(t("settings-syntax-colors"));
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
            if ui.checkbox(&mut wrap, t("settings-wrap")).changed() {
                settings.wrap_off = !wrap;
                changed = true;
            }
        }
        Section::GridsAndOutput => {
            ui.label(t("settings-paper"));
            let follow = &mut settings.paper_follows_system;
            changed |= ui
                .radio_value(follow, false, t("settings-always-light"))
                .changed();
            changed |= ui
                .radio_value(follow, true, t("settings-follow-system"))
                .changed();
            ui.add_space(8.0);
            let mut log = !settings.log_hidden;
            if ui.checkbox(&mut log, t("settings-log")).changed() {
                settings.log_hidden = !log;
                changed = true;
            }
            ui.add_space(8.0);
            ui.label(t("settings-show-in-output"));
            let output = &mut settings.output;
            for (part, on) in Part::ALL.into_iter().zip(&mut output.kinds) {
                changed |= ui.checkbox(on, part.label()).changed();
            }
        }
        Section::Updates => {
            let mut on = settings.updates != Some(false);
            if ui.checkbox(&mut on, t("settings-check-updates")).changed() {
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
        assert_eq!(set_syntax(variable_choices().0, "BOTH"), "SET TVARS=BOTH.");
        assert_eq!(
            set_syntax(value_choices().0, "VALUES"),
            "SET TNUMBERS=VALUES."
        );
        assert_eq!(
            variable_choices().1.map(|(k, _)| k),
            ["LABELS", "NAMES", "BOTH"]
        );
        assert_eq!(
            value_choices().1.map(|(k, _)| k),
            ["LABELS", "VALUES", "BOTH"]
        );
    }
}
