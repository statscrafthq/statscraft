//! Drawing of the Weighted Kappa, Fleiss Multirater Kappa and Ratio Statistics dialogs.

use crate::agreement_dialog::{
    FleissKappa, RATIO_ORDERS, RATIO_SAVE, RATIO_STATISTICS, Ratio, WeightedKappa,
};
use crate::app::{checkboxes, choice, target_list, target_one};
use crate::i18n::t;
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use std::path::PathBuf;

/// A confidence level field; empty means 95.
fn level(ui: &mut Ui, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(t("stats-agree-confidence-level"));
        small_field(ui, true, text);
    });
}

pub fn weighted_kappa(ui: &mut Ui, d: &mut WeightedKappa, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.by, false, t("stats-agree-pair-every-rater"));
        ui.radio_value(&mut d.by, true, t("stats-agree-pair-rows-columns"));
    });
    if d.by {
        target_list(ui, t("stats-agree-row-raters"), &mut d.raters, picked);
        target_list(ui, t("stats-agree-column-raters"), &mut d.columns, picked);
    } else {
        target_list(ui, t("stats-agree-raters"), &mut d.raters, picked);
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-agree-criteria")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(t("stats-agree-agreement-weights"));
            ui.radio_value(&mut d.quadratic, false, t("stats-agree-linear"));
            ui.radio_value(&mut d.quadratic, true, t("stats-agree-quadratic"));
        });
        level(ui, &mut d.ci_level);
        ui.checkbox(&mut d.include, t("stats-agree-missing-as-categories"));
    });
    egui::CollapsingHeader::new(t("stats-agree-tables")).show(ui, |ui| {
        ui.checkbox(&mut d.tabulate, t("stats-agree-table-of-counts"));
        ui.add_enabled_ui(d.tabulate, |ui| {
            ui.checkbox(&mut d.descending, t("stats-agree-categories-descending"));
        });
    });
}

pub fn fleiss_kappa(ui: &mut Ui, d: &mut FleissKappa, picked: &mut Vec<String>) {
    target_list(ui, t("stats-agree-raters"), &mut d.raters, picked);
    ui.weak(t("stats-agree-each-variable-one-rater"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-agree-options")).show(ui, |ui| {
        ui.checkbox(&mut d.category, t("stats-agree-each-category"));
        level(ui, &mut d.ci_level);
        ui.checkbox(&mut d.include, t("stats-agree-missing-as-categories"));
    });
}

pub fn ratio(ui: &mut Ui, d: &mut Ratio, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_one(ui, t("stats-agree-numerator"), &mut d.numerator, picked);
    target_one(ui, t("stats-agree-denominator"), &mut d.denominator, picked);
    target_one(ui, t("stats-agree-group-by"), &mut d.group, picked);
    ui.add_enabled_ui(!d.group.is_empty(), |ui| {
        choice(
            ui,
            t("stats-agree-order-of-groups"),
            &RATIO_ORDERS,
            &mut d.order,
        );
    });
    ui.checkbox(&mut d.include, t("stats-agree-include-user-missing"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-agree-statistics"))
        .default_open(true)
        .show(ui, |ui| {
            checkboxes(ui, &RATIO_STATISTICS, &mut d.statistics);
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.cin, t("stats-agree-confidence-intervals-level"));
                small_field(ui, d.cin, &mut d.cin_level);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.bcoc, t("stats-agree-percent-between"));
                ui.add_enabled(
                    d.bcoc,
                    egui::TextEdit::singleline(&mut d.intervals).desired_width(140.0),
                );
            });
            ui.weak(t("stats-agree-low-and-high-hint"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.wcoc, t("stats-agree-percent-within"));
                ui.add_enabled(
                    d.wcoc,
                    egui::TextEdit::singleline(&mut d.percents).desired_width(100.0),
                );
                ui.label(t("stats-agree-percent-of-median"));
            });
        });
    egui::CollapsingHeader::new(t("stats-agree-output")).show(ui, |ui| {
        ui.checkbox(&mut d.show, t("stats-agree-show-in-table"));
        ui.label(t("stats-agree-save-statistics-to"));
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
