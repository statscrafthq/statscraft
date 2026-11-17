//! Drawing of the TwoStep Cluster and Identify Unusual Cases dialogs.

use crate::app::{checkboxes, field, target_list, target_one};
use crate::stats_dialogs_view::small_field;
use crate::twostep_dialog::{ANOMALY_PRINT, ANOMALY_SAVE, Anomaly, TWOSTEP_PRINT, TwoStep};
use eframe::egui::{self, Ui};

pub fn two_step(ui: &mut Ui, d: &mut TwoStep, picked: &mut Vec<String>) {
    target_list(ui, "Categorical", &mut d.categorical, picked);
    target_list(ui, "Continuous", &mut d.continuous, picked);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.strong("Distance");
        ui.radio_value(&mut d.euclidean, false, "Log-likelihood");
        ui.radio_value(&mut d.euclidean, true, "Euclidean");
    });
    ui.add_space(6.0);
    ui.strong("Number of clusters");
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.auto, true, "Find automatically, at most");
        small_field(ui, d.auto, &mut d.max);
        ui.add_enabled_ui(d.auto, |ui| {
            ui.radio_value(&mut d.aic, false, "by BIC");
            ui.radio_value(&mut d.aic, true, "by AIC");
        });
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.auto, false, "Fixed number");
        small_field(ui, !d.auto, &mut d.fixed);
    });
    ui.add_space(6.0);
    target_list(
        ui,
        "Continuous variables to leave as they are",
        &mut d.unstandardized,
        picked,
    );
    field(ui, "Noise handling, percent", &mut d.noise, 50.0);
    ui.checkbox(&mut d.include, "Include user-missing values");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.checkbox(&mut d.model_summary, "Model summary");
        checkboxes(ui, &TWOSTEP_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        ui.checkbox(&mut d.save, "Cluster membership");
        ui.add_enabled_ui(d.save, |ui| {
            field(ui, "Variable name", &mut d.save_name, 120.0);
        })
        .response
        .on_hover_text("Left empty, the variable is named TSC_1, or the next free number");
    });
}

pub fn anomaly(ui: &mut Ui, d: &mut Anomaly, picked: &mut Vec<String>) {
    target_list(ui, "Categorical", &mut d.categorical, picked);
    target_list(ui, "Scale", &mut d.scale, picked);
    target_one(ui, "Case identifier", &mut d.id, picked);
    ui.add_space(6.0);
    ui.strong("Cases to list");
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.by_number, false, "Highest percent");
        small_field(ui, !d.by_number, &mut d.percent);
        ui.radio_value(&mut d.by_number, true, "Highest number");
        small_field(ui, d.by_number, &mut d.number);
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.use_cutoff, "Only with an anomaly index of at least");
        small_field(ui, d.use_cutoff, &mut d.cutoff);
    });
    field(ui, "Reasons for each case", &mut d.reasons, 50.0);
    ui.horizontal(|ui| {
        ui.label("Peer groups, from");
        small_field(ui, true, &mut d.min_peers);
        ui.label("to");
        small_field(ui, true, &mut d.max_peers);
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Missing values").show(ui, |ui| {
        ui.checkbox(
            &mut d.apply_missing,
            "Keep cases with missing values (mean in, missing as a category)",
        );
        ui.add_enabled_ui(d.apply_missing, |ui| {
            ui.checkbox(
                &mut d.missing_share,
                "Add the share of missing values as a variable",
            );
        });
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &ANOMALY_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        checkboxes(ui, &ANOMALY_SAVE, &mut d.save);
    });
}
