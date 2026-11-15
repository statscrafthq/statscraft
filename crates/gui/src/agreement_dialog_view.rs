//! Drawing of the Weighted Kappa, Fleiss Multirater Kappa and Ratio Statistics dialogs.

use crate::agreement_dialog::{
    FleissKappa, RATIO_ORDERS, RATIO_SAVE, RATIO_STATISTICS, Ratio, WeightedKappa,
};
use crate::app::{checkboxes, choice, target_list, target_one};
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use std::path::PathBuf;

/// A confidence level field; empty means 95.
fn level(ui: &mut Ui, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Confidence level (%)");
        small_field(ui, true, text);
    });
}

pub fn weighted_kappa(ui: &mut Ui, d: &mut WeightedKappa, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.by, false, "Pair every rater with every other");
        ui.radio_value(&mut d.by, true, "Pair rows with columns");
    });
    if d.by {
        target_list(ui, "Row raters", &mut d.raters, picked);
        target_list(ui, "Column raters", &mut d.columns, picked);
    } else {
        target_list(ui, "Raters", &mut d.raters, picked);
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Agreement weights");
            ui.radio_value(&mut d.quadratic, false, "Linear");
            ui.radio_value(&mut d.quadratic, true, "Quadratic");
        });
        level(ui, &mut d.ci_level);
        ui.checkbox(&mut d.include, "Count user-missing values as categories");
    });
    egui::CollapsingHeader::new("Tables").show(ui, |ui| {
        ui.checkbox(&mut d.tabulate, "Table of counts for each pair");
        ui.add_enabled_ui(d.tabulate, |ui| {
            ui.checkbox(&mut d.descending, "Categories in descending order");
        });
    });
}

pub fn fleiss_kappa(ui: &mut Ui, d: &mut FleissKappa, picked: &mut Vec<String>) {
    target_list(ui, "Raters", &mut d.raters, picked);
    ui.weak("Each variable is one rater, each case one subject");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.category, "Agreement on each category");
        level(ui, &mut d.ci_level);
        ui.checkbox(&mut d.include, "Count user-missing values as categories");
    });
}

pub fn ratio(ui: &mut Ui, d: &mut Ratio, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_one(ui, "Numerator", &mut d.numerator, picked);
    target_one(ui, "Denominator", &mut d.denominator, picked);
    target_one(ui, "Group by", &mut d.group, picked);
    ui.add_enabled_ui(!d.group.is_empty(), |ui| {
        choice(ui, "Order of the groups", &RATIO_ORDERS, &mut d.order);
    });
    ui.checkbox(&mut d.include, "Include user-missing values");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Statistics")
        .default_open(true)
        .show(ui, |ui| {
            checkboxes(ui, &RATIO_STATISTICS, &mut d.statistics);
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.cin, "Confidence intervals, level (%)");
                small_field(ui, d.cin, &mut d.cin_level);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.bcoc, "Percent of ratios between");
                ui.add_enabled(
                    d.bcoc,
                    egui::TextEdit::singleline(&mut d.intervals).desired_width(140.0),
                );
            });
            ui.weak("Low and high, one pair per interval, pairs separated by ;");
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.wcoc, "Percent of ratios within");
                ui.add_enabled(
                    d.wcoc,
                    egui::TextEdit::singleline(&mut d.percents).desired_width(100.0),
                );
                ui.label("% of the median");
            });
        });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.checkbox(&mut d.show, "Show the statistics in a table");
        ui.label("Save the statistics to");
        output_picker(
            ui,
            &RATIO_SAVE,
            &mut d.save,
            &mut d.file,
            &mut d.dataset,
            folder,
        );
    });
}
