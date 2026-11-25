//! Drawing of the Validate Data, Compare Datasets and Apply Data Dictionary dialogs.

use crate::app::{field, file_dialog, target_list};
use crate::data_tools_dialog::{
    COMPARE_PROPERTIES, COPY_FILEINFO, COPY_VARINFO, DataValidation, DatasetComparison,
    DictionaryCopy, VALIDATE_SAVE, can_merge,
};
use crate::i18n::t;
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, TextEdit, Ui};
use std::path::{Path, PathBuf};

/// The data to read: a data file chosen with Browse, or an open dataset by name.
fn source_picker(
    ui: &mut Ui,
    title: &str,
    open_dataset: &mut bool,
    file: &mut String,
    dataset: &mut String,
    folder: &mut Option<PathBuf>,
) {
    ui.strong(title);
    ui.horizontal(|ui| {
        ui.radio_value(open_dataset, false, t("core-a-data-file"));
        if !*open_dataset {
            ui.add(TextEdit::singleline(file).desired_width(220.0));
            if ui.button(t("core-browse")).clicked()
                && let Some(path) = file_dialog(folder)
                    .add_filter(t("filter-data-file"), &["sav"])
                    .pick_file()
            {
                *folder = path.parent().map(Path::to_path_buf);
                *file = path.to_string_lossy().into_owned();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.radio_value(open_dataset, true, t("core-an-open-dataset"));
        if *open_dataset {
            ui.add(TextEdit::singleline(dataset).desired_width(120.0));
        }
    });
}

pub fn validate(ui: &mut Ui, d: &mut DataValidation, picked: &mut Vec<String>) {
    target_list(ui, t("core-analysis-variables"), &mut d.variables, picked);
    target_list(ui, t("core-case-id-variables"), &mut d.ids, picked);
    ui.weak(t("core-ids-together"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("core-variable-checks"))
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.var_checks, t("core-flag-suspicious"));
            ui.add_enabled_ui(d.var_checks, |ui| {
                field(ui, t("core-pct-missing-above"), &mut d.pct_missing, 50.0);
                field(ui, t("core-largest-above"), &mut d.pct_equal, 50.0);
                field(ui, t("core-single-above"), &mut d.pct_unequal, 50.0);
                field(ui, t("core-cv-below"), &mut d.cv, 60.0);
                field(ui, t("core-sd-below"), &mut d.std_dev, 60.0);
                ui.weak(t("core-last-two-scale"));
            });
        });
    egui::CollapsingHeader::new(t("core-id-case-checks"))
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.incomplete_ids, t("core-incomplete-ids"));
            ui.checkbox(&mut d.duplicate_ids, t("core-duplicate-ids"));
            ui.checkbox(&mut d.report_empty, t("core-empty-cases"));
            ui.add_enabled(
                d.report_empty,
                egui::Checkbox::new(&mut d.all_variables, t("core-empty-all-vars")),
            );
        });
    egui::CollapsingHeader::new(t("core-rules-heading")).show(ui, |ui| {
        field(ui, t("core-cross-rules"), &mut d.rules, 120.0);
        ui.weak(t("core-rules-hint"));
        ui.horizontal(|ui| {
            ui.label(t("core-summaries"));
            ui.checkbox(&mut d.by_variable, t("core-by-variable"));
            ui.checkbox(&mut d.by_rule, t("core-by-rule"));
        });
    });
    egui::CollapsingHeader::new(t("core-case-report")).show(ui, |ui| {
        ui.checkbox(&mut d.case_report, t("core-list-breaking"));
        ui.add_enabled_ui(d.case_report, |ui| {
            field(
                ui,
                t("core-least-number-violations"),
                &mut d.min_violations,
                50.0,
            );
            field(ui, t("core-most-cases-listed"), &mut d.case_limit, 50.0);
            ui.weak(t("core-empty-all-of-them"));
        });
    });
    egui::CollapsingHeader::new(t("core-save-indicators")).show(ui, |ui| {
        ui.weak(t("core-empty-name-default"));
        for (i, (_, label)) in VALIDATE_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], t(label));
                if d.save[i] {
                    small_field(ui, true, &mut d.save_names[i]);
                }
            });
        }
    });
}

pub fn compare(
    ui: &mut Ui,
    d: &mut DatasetComparison,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    source_picker(
        ui,
        t("core-compare-with"),
        &mut d.open_dataset,
        &mut d.file,
        &mut d.dataset,
        folder,
    );
    ui.add_space(6.0);
    target_list(
        ui,
        t("core-variables-to-compare-cap"),
        &mut d.variables,
        picked,
    );
    ui.weak(t("core-empty-all-variables"));
    target_list(ui, t("core-case-id-variables"), &mut d.ids, picked);
    ui.weak(t("core-case-id-hint"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("button-save"))
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.flag, t("core-flag-differing"));
            ui.add_enabled_ui(d.flag, |ui| {
                field(ui, t("core-its-name"), &mut d.flag_name, 120.0);
                ui.weak(t("core-flag-hint"));
            });
            ui.strong(t("core-write-matching"));
            output_picker(
                ui,
                &[
                    t("core-nowhere"),
                    t("core-a-data-file"),
                    t("core-a-new-dataset"),
                ],
                &mut d.matches,
                &mut d.match_file,
                &mut d.match_dataset,
                folder,
            );
            ui.push_id("mismatches", |ui| {
                ui.strong(t("core-write-differing"));
                output_picker(
                    ui,
                    &[
                        t("core-nowhere"),
                        t("core-a-data-file"),
                        t("core-a-new-dataset"),
                    ],
                    &mut d.mismatches,
                    &mut d.mismatch_file,
                    &mut d.mismatch_dataset,
                    folder,
                );
            });
        });
    egui::CollapsingHeader::new(t("core-output")).show(ui, |ui| {
        ui.strong(t("core-also-compare"));
        egui::Grid::new("compare_properties")
            .num_columns(3)
            .show(ui, |ui| {
                for (i, (_, label)) in COMPARE_PROPERTIES.iter().enumerate() {
                    ui.checkbox(&mut d.properties[i], t(label));
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.case_table, t("core-list-differing"));
            small_field(ui, d.case_table, &mut d.table_limit);
            ui.label(t("core-cases-empty-all"));
        });
    });
}

pub fn apply_dictionary(
    ui: &mut Ui,
    d: &mut DictionaryCopy,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    source_picker(
        ui,
        t("core-copy-dictionary-from"),
        &mut d.open_dataset,
        &mut d.file,
        &mut d.dataset,
        folder,
    );
    ui.add_space(6.0);
    ui.strong(t("core-variable-properties"));
    for (i, (keyword, label, _)) in COPY_VARINFO.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.varinfo[i], t(label));
            if can_merge(keyword) && d.varinfo[i] {
                ui.checkbox(&mut d.var_merge[i], t("core-merge-existing"));
            }
        });
    }
    ui.add_space(6.0);
    ui.strong(t("core-file-properties"));
    ui.weak(t("core-only-weight"));
    for (i, (keyword, label, _)) in COPY_FILEINFO.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.file_info[i], t(label));
            if can_merge(keyword) && d.file_info[i] {
                ui.checkbox(&mut d.file_merge[i], t("core-merge-existing"));
            }
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("core-variables")).show(ui, |ui| {
        field(ui, t("core-copy-from-one"), &mut d.source_variable, 120.0);
        ui.weak(t("core-match-by-name"));
        target_list(ui, t("core-only-these"), &mut d.targets, picked);
        ui.weak(t("core-empty-all-variables"));
        ui.add_enabled(
            d.source_variable.trim().is_empty(),
            egui::Checkbox::new(&mut d.new_variables, t("core-add-lacking")),
        );
    });
}
