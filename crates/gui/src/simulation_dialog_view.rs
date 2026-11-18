//! Drawing of the Build Simulation Plan and Run Simulation dialogs.

use crate::app::{field, file_dialog};
use crate::simulation_dialog::{
    CorrelationRow, FixedRow, SIM_DISTRIBUTIONS, SaveCases, SimInputRow, SimPlan, SimRun,
    Simulation, can_cut,
};
use eframe::egui::{ComboBox, TextEdit, Ui};
use std::path::PathBuf;

pub fn body(ui: &mut Ui, d: &mut Simulation, folder: &mut Option<PathBuf>) {
    match d {
        Simulation::Plan(d) => plan(ui, d, folder),
        Simulation::Run(d) => run(ui, d, folder),
    }
}

/// A file field with Browse; `save` picks a file to write.
fn file_row(ui: &mut Ui, label: &str, path: &mut String, save: bool, folder: &Option<PathBuf>) {
    ui.horizontal(|ui| {
        ui.strong(label);
        if ui.button("Browse...").clicked() {
            let dialog = file_dialog(folder);
            let chosen = if save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(chosen) = chosen {
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
    ui.add(TextEdit::singleline(path).desired_width(360.0));
}

/// A button that adds a row, and the rows with a remove button each.
fn rows<T: Default>(
    ui: &mut Ui,
    title: &str,
    rows: &mut Vec<T>,
    mut draw: impl FnMut(&mut Ui, &mut T),
) {
    ui.horizontal(|ui| {
        ui.strong(title);
        if ui.button("Add").clicked() {
            rows.push(T::default());
        }
    });
    let mut remove = None;
    for (i, row) in rows.iter_mut().enumerate() {
        ui.push_id((title, i), |ui| {
            ui.horizontal_wrapped(|ui| {
                draw(ui, row);
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        rows.remove(i);
    }
}

fn small(ui: &mut Ui, text: &mut String, width: f32) {
    ui.add(TextEdit::singleline(text).desired_width(width));
}

fn input_row(ui: &mut Ui, row: &mut SimInputRow) {
    ui.label("Input");
    small(ui, &mut row.name, 90.0);
    ComboBox::from_id_salt("distribution")
        .selected_text(SIM_DISTRIBUTIONS[row.dist].1)
        .show_ui(ui, |ui| {
            for (i, (_, label, _)) in SIM_DISTRIBUTIONS.iter().enumerate() {
                ui.selectable_value(&mut row.dist, i, *label);
            }
        });
    let params = SIM_DISTRIBUTIONS[row.dist].2;
    for ((_, label), value) in params.iter().zip(&mut row.params) {
        ui.label(*label);
        small(ui, value, 70.0);
    }
    if !params.is_empty() && can_cut(SIM_DISTRIBUTIONS[row.dist].0) {
        ui.label("Cut below");
        small(ui, &mut row.min, 50.0);
        ui.label("above");
        small(ui, &mut row.max, 50.0);
    }
}

fn plan(ui: &mut Ui, d: &mut SimPlan, folder: &Option<PathBuf>) {
    ui.strong("Model");
    ui.weak("One line per target, such as: margin = units * (price - cost)");
    ui.add(
        TextEdit::multiline(&mut d.model)
            .desired_rows(3)
            .desired_width(420.0),
    );
    file_row(ui, "Model file", &mut d.model_file, true, folder);
    ui.weak("Typed lines are written here; with no lines, an existing model file is used");
    ui.add_space(6.0);
    rows(ui, "Simulated inputs", &mut d.inputs, input_row);
    ui.weak("Fit to the data takes the field of the same name in the active dataset");
    ui.add_space(6.0);
    rows(
        ui,
        "Fixed inputs",
        &mut d.fixed,
        |ui, row: &mut FixedRow| {
            ui.label("Input");
            small(ui, &mut row.name, 90.0);
            ui.label("Value");
            small(ui, &mut row.value, 70.0);
        },
    );
    ui.add_space(6.0);
    rows(
        ui,
        "Correlations",
        &mut d.correlations,
        |ui, row: &mut CorrelationRow| {
            small(ui, &mut row.first, 90.0);
            ui.label("with");
            small(ui, &mut row.second, 90.0);
            ui.label("r");
            small(ui, &mut row.value, 50.0);
        },
    );
    ui.weak("Pairs not listed are uncorrelated");
    ui.add_space(6.0);
    field(ui, "Targets to report", &mut d.targets, 200.0);
    ui.weak("Names separated by spaces; empty: every target of the model");
    field(ui, "Number of cases", &mut d.max_cases, 80.0);
    ui.weak("Empty: 100000");
    ui.horizontal(|ui| {
        ui.label("Stop early when the mean of");
        small(ui, &mut d.stop_target, 90.0);
        ui.label("has a confidence interval no wider than");
        small(ui, &mut d.stop_width, 60.0);
    });
    ui.add_space(6.0);
    file_row(ui, "Save the plan to", &mut d.plan_file, true, folder);
    ui.checkbox(&mut d.quiet, "Leave the plan out of the output");
}

fn run(ui: &mut Ui, d: &mut SimRun, folder: &Option<PathBuf>) {
    file_row(ui, "Plan file", &mut d.plan_file, false, folder);
    ui.add_space(6.0);
    field(ui, "Seed", &mut d.seed, 100.0);
    ui.weak("Empty: the command's own, so a rerun gives the same cases");
    ui.add_space(6.0);
    ui.strong("Charts");
    ui.checkbox(&mut d.pdf, "Histogram of each target");
    ui.checkbox(&mut d.cdf, "Cumulative distribution of each target");
    ui.checkbox(&mut d.boxplot, "Boxplot");
    ui.checkbox(
        &mut d.scatter,
        "Scatterplots of each target with each input",
    );
    ui.add_space(6.0);
    ui.strong("Sensitivity (tornado)");
    ui.checkbox(
        &mut d.tornado_corr,
        "Correlation of the target with each input",
    );
    ui.horizontal(|ui| {
        ui.checkbox(
            &mut d.tornado_delta,
            "Change of the mean when an input moves",
        );
        small(ui, &mut d.delta, 40.0);
        ui.label("standard deviations");
    });
    ui.checkbox(
        &mut d.tornado_variance,
        "Share of the variance of each input",
    );
    ui.add_space(6.0);
    ui.strong("Tables");
    ui.checkbox(
        &mut d.descriptives,
        "Descriptives of the targets and inputs",
    );
    field(ui, "Confidence level (%)", &mut d.level, 60.0);
    ui.weak("Empty: 95");
    ui.checkbox(&mut d.percentiles, "Percentiles of the targets");
    field(ui, "Percentiles", &mut d.ptiles, 140.0);
    ui.weak("Percentages separated by spaces, such as 5 50 95; empty: the command's own");
    ui.checkbox(&mut d.associations, "Correlations of the inputs");
    ui.add_space(6.0);
    ui.strong("Save the cases");
    ui.radio_value(&mut d.save, SaveCases::No, "Do not save");
    ui.radio_value(&mut d.save, SaveCases::Active, "Replace the active dataset");
    ui.radio_value(&mut d.save, SaveCases::File, "Write to a file");
    if d.save == SaveCases::File {
        file_row(ui, "File", &mut d.out_file, true, folder);
    }
}
