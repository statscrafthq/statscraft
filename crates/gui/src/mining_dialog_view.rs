//! Drawing of the Attribute Importance, Feature Selection and Prepare Data for Modeling dialogs.

use crate::app::{checkboxes, field, target_list, target_one};
use crate::mining_dialog::{
    Aim, DATE_UNITS, EXTRACTS, FeatureSelection, PrepareData, RANKINGS, SELECT_PRINT,
    TARGET_LEVELS, TIME_UNITS,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};

pub fn aim(ui: &mut Ui, d: &mut Aim, picked: &mut Vec<String>) {
    target_one(
        ui,
        "Grouping variable (for example a saved cluster)",
        &mut d.group,
        picked,
    );
    target_list(ui, "Categorical", &mut d.categorical, picked);
    target_list(ui, "Continuous", &mut d.continuous, picked);
    ui.add_space(6.0);
    ui.checkbox(&mut d.include, "Include user-missing values");
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        field(ui, "Confidence level, percent", &mut d.ci, 50.0);
        ui.checkbox(
            &mut d.bonferroni,
            "Adjust the level for the number of groups (Bonferroni)",
        );
        ui.checkbox(
            &mut d.hide_not_significant,
            "Leave what is not significant out of the charts",
        );
        ui.checkbox(
            &mut d.reference_lines,
            "Show critical values or the overall mean",
        );
    });
    egui::CollapsingHeader::new("Charts").show(ui, |ui| {
        ui.checkbox(&mut d.category_plot, "Category shares within the groups");
        ui.horizontal(|ui| {
            ui.label("Group sizes");
            ui.radio_value(&mut d.cluster_plot, 0, "None");
            ui.radio_value(&mut d.cluster_plot, 1, "Bars");
            ui.radio_value(&mut d.cluster_plot, 2, "Pie");
        });
        ui.checkbox(&mut d.error_bar, "Group means with their intervals");
        ui.checkbox(&mut d.importance, "Importance of the variables");
        ui.add_enabled_ui(d.importance, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.by_variable, false, "One chart per variable");
                ui.radio_value(&mut d.by_variable, true, "One chart per group");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.pvalue, false, "Test statistic");
                ui.radio_value(&mut d.pvalue, true, "Minus log of the significance");
            });
        });
    });
}

pub fn feature_selection(ui: &mut Ui, d: &mut FeatureSelection, picked: &mut Vec<String>) {
    target_one(ui, "Target", &mut d.target, picked);
    ui.horizontal(|ui| {
        ui.label("Target level");
        for (i, (_, label)) in TARGET_LEVELS.iter().enumerate() {
            ui.radio_value(&mut d.level, i, *label);
        }
    });
    ui.add_space(6.0);
    target_list(
        ui,
        "Predictors that are categories (factors)",
        &mut d.factors,
        picked,
    );
    target_list(
        ui,
        "Predictors that are numbers (covariates)",
        &mut d.covariates,
        picked,
    );
    target_list(ui, "Variables to leave out", &mut d.except, picked);
    ui.label("With both predictor lists empty, every other variable is a predictor.");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Screening").show(ui, |ui| {
        ui.checkbox(&mut d.screening, "Drop unusable predictors first");
        ui.add_enabled_ui(d.screening, |ui| {
            field(ui, "Most percent missing", &mut d.pct_missing, 50.0);
            field(
                ui,
                "Most percent of cases in one category",
                &mut d.pct_equal,
                50.0,
            );
            field(
                ui,
                "Most percent of categories with a single case",
                &mut d.pct_unequal,
                50.0,
            );
            field(ui, "Smallest standard deviation", &mut d.min_sd, 50.0);
            field(ui, "Smallest coefficient of variation", &mut d.min_cv, 60.0);
        });
    });
    egui::CollapsingHeader::new("Selection")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.auto_size, true, "Keep a number chosen automatically");
                ui.radio_value(&mut d.auto_size, false, "Keep");
                small_field(ui, !d.auto_size, &mut d.size);
            });
            field(
                ui,
                "Keep only predictors with a significance below",
                &mut d.pcutoff,
                50.0,
            );
            ui.strong("Rank by");
            for (i, (_, label)) in RANKINGS.iter().enumerate() {
                ui.radio_value(&mut d.ranking, i, *label);
            }
            field(
                ui,
                "Further unselected predictors to show",
                &mut d.show_unselected,
                50.0,
            );
            field(ui, "Time limit in minutes (0 for none)", &mut d.timer, 50.0);
            ui.checkbox(&mut d.include, "Include user-missing values of categories");
        });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &SELECT_PRINT, &mut d.print);
        ui.checkbox(&mut d.plot, "Chart of the ranking");
    });
}

pub fn prepare_data(ui: &mut Ui, d: &mut PrepareData, picked: &mut Vec<String>) {
    target_one(ui, "Target (optional)", &mut d.target, picked);
    target_list(ui, "Inputs", &mut d.inputs, picked);
    target_one(ui, "Analysis weight (optional)", &mut d.weight, picked);
    ui.add_space(6.0);
    field(
        ui,
        "Write the preparation syntax to the file",
        &mut d.outfile,
        220.0,
    );
    ui.horizontal(|ui| {
        ui.label("Name added to a prepared target");
        ui.add(egui::TextEdit::singleline(&mut d.suffix_target).desired_width(100.0));
        ui.label("and input");
        ui.add(egui::TextEdit::singleline(&mut d.suffix_input).desired_width(100.0));
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Dates and times").show(ui, |ui| {
        ui.checkbox(
            &mut d.date_duration,
            "Time since a date, in place of the date",
        );
        ui.add_enabled_ui(d.date_duration, |ui| {
            field(
                ui,
                "Reference date (year-month-day, empty for today)",
                &mut d.date_reference,
                100.0,
            );
            ui.horizontal(|ui| {
                for (i, (_, label)) in DATE_UNITS.iter().enumerate() {
                    ui.radio_value(&mut d.date_unit, i, *label);
                }
            });
        });
        ui.checkbox(
            &mut d.time_duration,
            "Time since a time of day, in place of the time",
        );
        ui.add_enabled_ui(d.time_duration, |ui| {
            field(
                ui,
                "Reference time (hour:minute:second, empty for now)",
                &mut d.time_reference,
                100.0,
            );
            ui.horizontal(|ui| {
                for (i, (_, label)) in TIME_UNITS.iter().enumerate() {
                    ui.radio_value(&mut d.time_unit, i, *label);
                }
            });
        });
        ui.strong("Also make a field of the");
        checkboxes(ui, &EXTRACTS, &mut d.extract);
    });
    egui::CollapsingHeader::new("Screening").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.screen_missing,
                "Drop fields with more percent missing than",
            );
            small_field(ui, d.screen_missing, &mut d.max_missing);
        });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.screen_categories,
                "Drop nominal fields with more categories than",
            );
            small_field(ui, d.screen_categories, &mut d.max_categories);
        });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.screen_single,
                "Drop fields with more percent in one category than",
            );
            small_field(ui, d.screen_single, &mut d.max_single);
        });
    });
    egui::CollapsingHeader::new("Levels, outliers and missing values").show(ui, |ui| {
        ui.checkbox(
            &mut d.adjust_input,
            "Adjust the level of inputs by their number of values",
        );
        ui.checkbox(&mut d.adjust_target, "Adjust the level of the target");
        field(
            ui,
            "Most values of an ordinal field",
            &mut d.max_ordinal,
            50.0,
        );
        field(
            ui,
            "Fewest values of a continuous field",
            &mut d.min_continuous,
            50.0,
        );
        ui.add_space(4.0);
        ui.checkbox(&mut d.outlier_input, "Handle outliers of inputs");
        ui.checkbox(&mut d.outlier_target, "Handle outliers of the target");
        field(
            ui,
            "Outlier cutoff, in standard deviations",
            &mut d.cutoff,
            50.0,
        );
        ui.checkbox(
            &mut d.outliers_to_missing,
            "Make outliers missing instead of the cutoff value",
        );
        ui.add_space(4.0);
        ui.checkbox(
            &mut d.replace_input,
            "Replace missing values of inputs (mean, median or mode)",
        );
        ui.checkbox(
            &mut d.replace_target,
            "Replace missing values of the target",
        );
    });
    egui::CollapsingHeader::new("Transformations").show(ui, |ui| {
        ui.checkbox(
            &mut d.reorder_input,
            "Renumber nominal inputs by their count",
        );
        ui.checkbox(
            &mut d.reorder_target,
            "Renumber a nominal target by its count",
        );
        ui.add_space(4.0);
        ui.strong("Continuous inputs");
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.rescale_input, 0, "Z scores");
            ui.radio_value(&mut d.rescale_input, 1, "Minimum and maximum");
            ui.radio_value(&mut d.rescale_input, 2, "As they are");
        });
        ui.add_enabled_ui(d.rescale_input == 1, |ui| {
            ui.horizontal(|ui| {
                ui.label("from");
                small_field(ui, true, &mut d.min);
                ui.label("to");
                small_field(ui, true, &mut d.max);
            });
        });
        ui.checkbox(
            &mut d.box_cox,
            "Box-Cox transformation of a continuous target",
        );
        ui.add_space(4.0);
        ui.strong("Using the target");
        ui.checkbox(
            &mut d.merge_supervised,
            "Merge categories that relate alike to the target",
        );
        ui.checkbox(
            &mut d.binning,
            "Bin continuous inputs by the target's classes",
        );
        ui.checkbox(&mut d.selection, "Drop inputs not related to the target");
        field(ui, "Significance level of these tests", &mut d.pvalue, 50.0);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.merge_unsupervised,
                "Merge small categories until each holds percent",
            );
            small_field(ui, d.merge_unsupervised, &mut d.min_pct);
        });
    });
}
