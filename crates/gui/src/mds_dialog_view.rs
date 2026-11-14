//! Drawing of the Multidimensional Scaling dialogs.

use crate::app::{checkboxes, choice, field, file_dialog, target_list, target_one};
use crate::mds_dialog::{
    ALSCAL_LEVELS, ALSCAL_MEASURES, ALSCAL_PLOTS, ALSCAL_STANDARDIZE, Alscal, PROXSCAL_FORMATS,
    PROXSCAL_MODELS, PROXSCAL_OUTFILE, PROXSCAL_PLOT, PROXSCAL_PRINT, PROXSCAL_SHAPES,
    PROXSCAL_STARTS, PROXSCAL_TRANSFORMATIONS, Proxscal,
};
use crate::model_dialogs_view::name_checkboxes;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, TextEdit, Ui};
use statscraft_data::Dataset;
use std::path::{Path, PathBuf};

/// A data file path with Browse: one to write (`save`) or one to read.
fn data_path(
    ui: &mut Ui,
    label: &str,
    path: &mut String,
    save: bool,
    folder: &mut Option<PathBuf>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(TextEdit::singleline(path).desired_width(220.0));
        if ui
            .push_id(label, |ui| ui.button("Browse..."))
            .inner
            .clicked()
        {
            let dialog = file_dialog(folder).add_filter("Data file", &["sav"]);
            let chosen = match save {
                true => dialog.save_file(),
                false => dialog.pick_file(),
            };
            if let Some(chosen) = chosen {
                *folder = chosen.parent().map(Path::to_path_buf);
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
}

/// A choice of radio buttons, the ones after `supported` shown but not available yet.
fn radios(ui: &mut Ui, options: &[&str], supported: usize, index: &mut usize) {
    ui.horizontal_wrapped(|ui| {
        for (i, label) in options.iter().enumerate() {
            if ui
                .add_enabled(i < supported, egui::RadioButton::new(*index == i, *label))
                .on_disabled_hover_text("Not supported yet")
                .clicked()
            {
                *index = i;
            }
        }
    });
}

pub fn proxscal(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut Proxscal,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    ui.strong("Data format");
    for (i, label) in PROXSCAL_FORMATS.iter().enumerate() {
        ui.radio_value(&mut d.format, i, *label);
    }
    ui.add_space(6.0);
    match d.format {
        2 => data_path(ui, "Matrix file", &mut d.matrix_file, false, folder),
        _ => {
            let title = match d.format {
                0 => "Objects (matrix columns)",
                _ => "Proximities",
            };
            target_list(ui, title, &mut d.variables, picked);
            if d.format == 1 {
                target_one(ui, "Row object", &mut d.row, picked);
                target_one(ui, "Column object", &mut d.column, picked);
            }
            ui.add_enabled_ui(d.source_applies(), |ui| {
                target_one(ui, "Sources (optional)", &mut d.source, picked);
            })
            .response
            .on_disabled_hover_text("Several proximity variables are the sources themselves");
            target_list(ui, "Weights (optional)", &mut d.weights, picked);
            ui.weak("One weight variable for each proximity variable, in the same order");
            choice(ui, "Matrix shape", &PROXSCAL_SHAPES, &mut d.shape);
        }
    }
    ui.checkbox(&mut d.similarities, "Proximities are similarities");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            choice(ui, "Scaling model", &PROXSCAL_MODELS, &mut d.model);
            ui.label("rank");
            small_field(ui, d.model == 3, &mut d.rank);
        });
        choice(
            ui,
            "Transformation",
            &PROXSCAL_TRANSFORMATIONS,
            &mut d.transformation,
        );
        ui.add_enabled(
            d.transformation == 2,
            egui::Checkbox::new(&mut d.untie, "Untie tied proximities"),
        );
        ui.horizontal(|ui| {
            let spline = d.transformation == 3;
            ui.label("Spline degree");
            small_field(ui, spline, &mut d.degree);
            ui.label("interior knots");
            small_field(ui, spline, &mut d.knots);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.unconditional, false, "Transform within each source");
            ui.radio_value(&mut d.unconditional, true, "Across all sources");
        });
        ui.horizontal(|ui| {
            ui.label("Dimensions from");
            small_field(ui, true, &mut d.min_dimensions);
            ui.label("to");
            small_field(ui, true, &mut d.max_dimensions);
        });
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.horizontal(|ui| {
            choice(ui, "Start", &PROXSCAL_STARTS, &mut d.start);
            ui.label("starts");
            small_field(ui, d.start == 2, &mut d.random_starts);
        });
        field(ui, "Most iterations", &mut d.max_iterations, 50.0);
        field(
            ui,
            "Stop when stress improves less than",
            &mut d.diff_stress,
            70.0,
        );
        field(ui, "Stop when stress is below", &mut d.min_stress, 70.0);
        ui.checkbox(&mut d.acceleration, "Relaxed updates (faster)");
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.label("Tables");
        checkboxes(ui, &PROXSCAL_PRINT, &mut d.print);
        ui.label("Plots");
        checkboxes(ui, &PROXSCAL_PLOT, &mut d.plot);
        let sources = match d.plot[3..].contains(&true) {
            true => d.sources(data),
            false => Vec::new(),
        };
        for i in 0..3 {
            if d.plot[i + 3] && sources.len() > 1 {
                ui.label(format!("{} of", PROXSCAL_PLOT[i + 3].1));
                ui.push_id(i, |ui| {
                    name_checkboxes(ui, &sources, &mut d.plot_sources[i])
                });
            }
        }
        ui.weak("No source checked plots every source");
    });
    egui::CollapsingHeader::new("Save to Files").show(ui, |ui| {
        for (i, (_, label)) in PROXSCAL_OUTFILE.iter().enumerate() {
            data_path(ui, label, &mut d.outfiles[i], true, folder);
        }
    });
}

pub fn alscal(ui: &mut Ui, d: &mut Alscal, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    ui.radio_value(
        &mut d.from_data,
        false,
        "The variables hold square matrices of proximities",
    );
    ui.radio_value(&mut d.from_data, true, "Create distances from the data");
    if d.from_data {
        ui.indent("from data", |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.between_cases, false, "Between variables");
                ui.radio_value(&mut d.between_cases, true, "Between cases");
            });
            choice(ui, "Measure", &ALSCAL_MEASURES, &mut d.measure);
            choice(ui, "Standardize", &ALSCAL_STANDARDIZE, &mut d.standardize);
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.label("Matrix shape");
        let mut shape = 0;
        radios(
            ui,
            &["Symmetric", "Asymmetric", "Rectangular"],
            1,
            &mut shape,
        );
        choice(ui, "Level of measurement", &ALSCAL_LEVELS, &mut d.level);
        ui.horizontal(|ui| {
            let ordinal = d.level == 0;
            ui.add_enabled(ordinal, egui::Checkbox::new(&mut d.untie, "Untie ties"));
            ui.add_enabled(ordinal, egui::Checkbox::new(&mut d.similar, "Similarities"));
            ui.label("degree");
            small_field(ui, d.level == 1 || d.level == 2, &mut d.degree);
        });
        ui.label("Conditionality");
        let mut condition = usize::from(d.unconditional);
        radios(ui, &["Matrix", "Unconditional", "Row"], 2, &mut condition);
        d.unconditional = condition == 1;
        ui.label("Scaling model");
        let mut model = usize::from(d.indscal);
        radios(
            ui,
            &[
                "Euclidean distance",
                "Individual differences (INDSCAL)",
                "Asymmetric",
                "Asymmetric individual differences",
                "Generalized",
            ],
            2,
            &mut model,
        );
        d.indscal = model == 1;
        ui.add_enabled(
            d.indscal,
            egui::Checkbox::new(&mut d.negative, "Allow negative subject weights"),
        );
        ui.horizontal(|ui| {
            ui.label("Dimensions from");
            small_field(ui, true, &mut d.min_dimensions);
            ui.label("to");
            small_field(ui, true, &mut d.max_dimensions);
        });
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(
            ui,
            "Stop when S-stress improves less than",
            &mut d.converge,
            70.0,
        );
        field(ui, "Stop when S-stress is below", &mut d.stress_min, 70.0);
        field(ui, "Most iterations", &mut d.iterations, 50.0);
        field(
            ui,
            "Proximities below this are missing",
            &mut d.cutoff,
            70.0,
        );
        ui.checkbox(&mut d.print_data, "Show the data");
        ui.checkbox(&mut d.print_header, "Show the model and its options");
        choice(ui, "Plots", &ALSCAL_PLOTS, &mut d.plot);
        data_path(ui, "Save coordinates to", &mut d.outfile, true, folder);
    });
}
