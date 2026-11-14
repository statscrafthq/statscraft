//! Drawing of the Distances dialog.

use crate::app::{checkboxes, choice, file_dialog, target_list, target_one};
use crate::model_dialogs_view::measure;
use crate::proximities_dialog::{
    Distances, PROXIMITIES_MATRIX, PROXIMITIES_STANDARDIZE, PROXIMITIES_TRANSFORMS,
};
use eframe::egui::{self, TextEdit, Ui};
use std::path::{Path, PathBuf};

pub fn distances(
    ui: &mut Ui,
    d: &mut Distances,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    target_list(ui, "Variables", &mut d.variables, picked);
    ui.add_enabled_ui(!d.between_variables, |ui| {
        target_one(ui, "Label cases by", &mut d.id, picked);
    })
    .response
    .on_disabled_hover_text("Labels are for distances between cases");
    ui.horizontal(|ui| {
        ui.label("Compute distances");
        ui.radio_value(&mut d.between_variables, false, "Between cases");
        ui.radio_value(&mut d.between_variables, true, "Between variables");
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Measures").show(ui, |ui| {
        measure(ui, &mut d.measure);
        ui.add_space(4.0);
        ui.add_enabled_ui(d.measure.data != 2, |ui| {
            choice(
                ui,
                "Standardize values",
                &PROXIMITIES_STANDARDIZE,
                &mut d.standardize,
            );
            ui.add_enabled_ui(d.standardize > 0, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.by_case, false, "Within each variable");
                    ui.radio_value(&mut d.by_case, true, "Within each case");
                });
            });
        })
        .response
        .on_disabled_hover_text("A binary measure takes the values as they are");
        ui.add_space(4.0);
        ui.label("Transform the results");
        checkboxes(ui, &PROXIMITIES_TRANSFORMS, &mut d.transforms);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.print, "Proximity matrix table");
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
    egui::CollapsingHeader::new("Matrix Output").show(ui, |ui| {
        output_picker(
            ui,
            &PROXIMITIES_MATRIX,
            &mut d.matrix,
            &mut d.file,
            &mut d.dataset,
            folder,
        );
    });
}

/// One radio button per place in `labels` to write data to; the second takes a data file with
/// Browse, the third the name of a new dataset.
pub(crate) fn output_picker(
    ui: &mut Ui,
    labels: &[&str],
    choice: &mut usize,
    file: &mut String,
    dataset: &mut String,
    folder: &mut Option<PathBuf>,
) {
    for (i, label) in labels.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.radio_value(choice, i, *label);
            match i {
                1 if *choice == 1 => {
                    ui.add(TextEdit::singleline(file).desired_width(220.0));
                    if ui.button("Browse...").clicked()
                        && let Some(mut path) = file_dialog(folder)
                            .add_filter("Data file", &["sav"])
                            .save_file()
                    {
                        if path.extension().is_none() {
                            path.set_extension("sav");
                        }
                        *folder = path.parent().map(Path::to_path_buf);
                        *file = path.to_string_lossy().into_owned();
                    }
                }
                2 if *choice == 2 => {
                    ui.add(TextEdit::singleline(dataset).desired_width(120.0));
                }
                _ => {}
            }
        });
    }
}
