//! Drawing of the Distances dialog.

use crate::app::{checkboxes, choice, file_dialog, target_list, target_one};
use crate::i18n::t;
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
    target_list(ui, t("stats-prox-variables"), &mut d.variables, picked);
    ui.add_enabled_ui(!d.between_variables, |ui| {
        target_one(ui, t("stats-prox-label-cases-by"), &mut d.id, picked);
    })
    .response
    .on_disabled_hover_text(t("stats-prox-labels-between-cases"));
    ui.horizontal(|ui| {
        ui.label(t("stats-prox-compute-distances"));
        ui.radio_value(
            &mut d.between_variables,
            false,
            t("stats-prox-between-cases"),
        );
        ui.radio_value(
            &mut d.between_variables,
            true,
            t("stats-prox-between-variables"),
        );
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-prox-measures")).show(ui, |ui| {
        measure(ui, &mut d.measure);
        ui.add_space(4.0);
        ui.add_enabled_ui(d.measure.data != 2, |ui| {
            choice(
                ui,
                t("stats-prox-standardize-values"),
                &PROXIMITIES_STANDARDIZE,
                &mut d.standardize,
            );
            ui.add_enabled_ui(d.standardize > 0, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.by_case, false, t("stats-prox-within-each-variable"));
                    ui.radio_value(&mut d.by_case, true, t("stats-prox-within-each-case"));
                });
            });
        })
        .response
        .on_disabled_hover_text(t("stats-prox-binary-as-they-are"));
        ui.add_space(4.0);
        ui.label(t("stats-prox-transform-results"));
        checkboxes(ui, &PROXIMITIES_TRANSFORMS, &mut d.transforms);
    });
    egui::CollapsingHeader::new(t("stats-prox-options")).show(ui, |ui| {
        ui.checkbox(&mut d.print, t("stats-prox-matrix-table"));
        ui.checkbox(&mut d.include, t("stats-prox-include-user-missing"));
    });
    egui::CollapsingHeader::new(t("stats-prox-matrix-output")).show(ui, |ui| {
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
/// Browse, the third the name of a new dataset. A blank label offers no button. A label may be
/// a catalog key.
pub(crate) fn output_picker(
    ui: &mut Ui,
    labels: &[&'static str],
    choice: &mut usize,
    file: &mut String,
    dataset: &mut String,
    folder: &mut Option<PathBuf>,
) {
    for (i, label) in labels.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
        ui.horizontal(|ui| {
            ui.radio_value(choice, i, t(label));
            match i {
                1 if *choice == 1 => {
                    ui.add(TextEdit::singleline(file).desired_width(220.0));
                    if ui.button(t("stats-prox-browse")).clicked()
                        && let Some(mut path) = file_dialog(folder)
                            .add_filter(t("filter-data-file"), &["sav"])
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
