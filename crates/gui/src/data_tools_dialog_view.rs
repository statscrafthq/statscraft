//! Drawing of the Validate Data, Compare Datasets and Apply Data Dictionary dialogs.

use crate::app::{field, file_dialog, target_list};
use crate::data_tools_dialog::{
    COMPARE_PROPERTIES, COPY_FILEINFO, COPY_VARINFO, DataValidation, DatasetComparison,
    DictionaryCopy, VALIDATE_SAVE, can_merge,
};
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
        ui.radio_value(open_dataset, false, "A data file");
        if !*open_dataset {
            ui.add(TextEdit::singleline(file).desired_width(220.0));
            if ui.button("Browse...").clicked()
                && let Some(path) = file_dialog(folder)
                    .add_filter("Data file", &["sav"])
                    .pick_file()
            {
                *folder = path.parent().map(Path::to_path_buf);
                *file = path.to_string_lossy().into_owned();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.radio_value(open_dataset, true, "An open dataset");
        if *open_dataset {
            ui.add(TextEdit::singleline(dataset).desired_width(120.0));
        }
    });
}

pub fn validate(ui: &mut Ui, d: &mut DataValidation, picked: &mut Vec<String>) {
    target_list(ui, "Analysis variables", &mut d.variables, picked);
    target_list(ui, "Case identifier variables", &mut d.ids, picked);
    ui.weak("Together they identify a case; used by the ID checks");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Variable checks")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.var_checks, "Flag suspicious variables");
            ui.add_enabled_ui(d.var_checks, |ui| {
                field(ui, "Percent missing above", &mut d.pct_missing, 50.0);
                field(
                    ui,
                    "Largest category above (percent)",
                    &mut d.pct_equal,
                    50.0,
                );
                field(
                    ui,
                    "Categories of one case above (percent)",
                    &mut d.pct_unequal,
                    50.0,
                );
                field(ui, "Coefficient of variation below", &mut d.cv, 60.0);
                field(ui, "Standard deviation below", &mut d.std_dev, 60.0);
                ui.weak("The last two apply to scale variables; 0 turns the first one off");
            });
        });
    egui::CollapsingHeader::new("Identifier and case checks")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.incomplete_ids, "Incomplete identifiers");
            ui.checkbox(&mut d.duplicate_ids, "Duplicate identifiers");
            ui.checkbox(&mut d.report_empty, "Empty cases");
            ui.add_enabled(
                d.report_empty,
                egui::Checkbox::new(
                    &mut d.all_variables,
                    "Judge emptiness on every variable, not only the analysis variables",
                ),
            );
        });
    egui::CollapsingHeader::new("Rules").show(ui, |ui| {
        field(ui, "Cross-variable rules", &mut d.rules, 120.0);
        ui.weak("Rule numbers or ALL; the rules are kept in the data file's dictionary");
        ui.horizontal(|ui| {
            ui.label("Summaries");
            ui.checkbox(&mut d.by_variable, "By variable");
            ui.checkbox(&mut d.by_rule, "By rule");
        });
    });
    egui::CollapsingHeader::new("Case report").show(ui, |ui| {
        ui.checkbox(&mut d.case_report, "List the cases that break rules");
        ui.add_enabled_ui(d.case_report, |ui| {
            field(
                ui,
                "Least number of violations",
                &mut d.min_violations,
                50.0,
            );
            field(ui, "Most cases listed", &mut d.case_limit, 50.0);
            ui.weak("Empty: all of them");
        });
    });
    egui::CollapsingHeader::new("Save indicator variables").show(ui, |ui| {
        ui.weak("A name left empty gets the command's own name");
        for (i, (_, label)) in VALIDATE_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
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
        "Compare the active dataset with",
        &mut d.open_dataset,
        &mut d.file,
        &mut d.dataset,
        folder,
    );
    ui.add_space(6.0);
    target_list(ui, "Variables to compare", &mut d.variables, picked);
    ui.weak("Empty: all variables");
    target_list(ui, "Case identifier variables", &mut d.ids, picked);
    ui.weak("Without them, the first case of each file pairs with the first of the other; with them, both files must be sorted by them");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Save")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.flag, "Add a variable that flags differing cases");
            ui.add_enabled_ui(d.flag, |ui| {
                field(ui, "Its name", &mut d.flag_name, 120.0);
                ui.weak(
                    "Empty: CasesCompare. 0 same, 1 differs, -1 no such case in the other file",
                );
            });
            ui.strong("Write cases that match to");
            output_picker(
                ui,
                &["Nowhere", "A data file", "A new dataset"],
                &mut d.matches,
                &mut d.match_file,
                &mut d.match_dataset,
                folder,
            );
            ui.push_id("mismatches", |ui| {
                ui.strong("Write cases that differ or have no partner to");
                output_picker(
                    ui,
                    &["Nowhere", "A data file", "A new dataset"],
                    &mut d.mismatches,
                    &mut d.mismatch_file,
                    &mut d.mismatch_dataset,
                    folder,
                );
            });
        });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.strong("Also compare these properties of the variables");
        egui::Grid::new("compare_properties")
            .num_columns(3)
            .show(ui, |ui| {
                for (i, (_, label)) in COMPARE_PROPERTIES.iter().enumerate() {
                    ui.checkbox(&mut d.properties[i], *label);
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.case_table, "List the differing values, at most");
            small_field(ui, d.case_table, &mut d.table_limit);
            ui.label("cases (empty: all)");
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
        "Copy the dictionary from",
        &mut d.open_dataset,
        &mut d.file,
        &mut d.dataset,
        folder,
    );
    ui.add_space(6.0);
    ui.strong("Properties of the variables");
    for (i, (keyword, label, _)) in COPY_VARINFO.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.varinfo[i], *label);
            if can_merge(keyword) && d.varinfo[i] {
                ui.checkbox(&mut d.var_merge[i], "Merge with what is there");
            }
        });
    }
    ui.add_space(6.0);
    ui.strong("Properties of the file");
    ui.weak("Without a change here only the weight is copied");
    for (i, (keyword, label, _)) in COPY_FILEINFO.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.file_info[i], *label);
            if can_merge(keyword) && d.file_info[i] {
                ui.checkbox(&mut d.file_merge[i], "Merge with what is there");
            }
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Variables").show(ui, |ui| {
        field(
            ui,
            "Copy from one variable of the file",
            &mut d.source_variable,
            120.0,
        );
        ui.weak("Empty: variables match by name");
        target_list(ui, "Only to these variables", &mut d.targets, picked);
        ui.weak("Empty: all variables");
        ui.add_enabled(
            d.source_variable.trim().is_empty(),
            egui::Checkbox::new(
                &mut d.new_variables,
                "Add the variables the active data lacks",
            ),
        );
    });
}
