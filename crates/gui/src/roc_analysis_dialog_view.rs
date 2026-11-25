//! Drawing of the ROC Analysis dialog.

use crate::app::{choice, field, target_list, target_one};
use crate::i18n::t;
use crate::roc_analysis_dialog::{CUTPOINT, LEVEL, ROC_CURVES, ROC_SELECTS, RocAnalysis};
use eframe::egui::{self, Ui};

pub fn roc_analysis(ui: &mut Ui, d: &mut RocAnalysis, picked: &mut Vec<String>) {
    target_list(ui, t("stats-roc-test-variables"), &mut d.tests, picked);
    target_one(ui, t("stats-roc-state-variable"), &mut d.state, picked);
    field(ui, t("stats-roc-positive-value"), &mut d.value, 80.0);
    ui.add_space(6.0);
    ui.strong(t("stats-roc-comparison-of-areas"));
    let grouped = d.grouped();
    ui.add_enabled(
        !grouped,
        egui::Checkbox::new(&mut d.pair, t("stats-roc-compare-same-cases")),
    );
    target_one(
        ui,
        t("stats-roc-grouping-variable-optional"),
        &mut d.group,
        picked,
    );
    if grouped {
        choice(ui, t("stats-roc-groups"), &ROC_SELECTS, &mut d.select);
        match d.select {
            LEVEL => {
                ui.horizontal(|ui| {
                    ui.label(t("stats-roc-values"));
                    ui.add(egui::TextEdit::singleline(&mut d.first).desired_width(70.0));
                    ui.add(egui::TextEdit::singleline(&mut d.second).desired_width(70.0));
                });
            }
            CUTPOINT => field(ui, t("stats-roc-value"), &mut d.cut, 70.0),
            _ => {
                ui.label(t("stats-roc-a-numeric-variable"));
            }
        }
        ui.weak(t("stats-roc-area-per-group"));
    }
    ui.add_space(6.0);
    ui.strong(t("stats-roc-display"));
    choice(ui, t("stats-roc-curves"), &ROC_CURVES, &mut d.curve);
    let (roc, pr) = d.draws();
    ui.add_enabled(
        roc,
        egui::Checkbox::new(&mut d.reference, t("stats-roc-diagonal-reference")),
    );
    ui.add_enabled(
        pr,
        egui::Checkbox::new(&mut d.pr_reference, t("stats-roc-share-positive")),
    );
    ui.add_enabled(
        pr,
        egui::Checkbox::new(&mut d.interpolate_fp, t("stats-roc-interpolate-fp")),
    );
    ui.checkbox(&mut d.model_quality, t("stats-roc-model-quality"));
    ui.checkbox(&mut d.se, t("stats-roc-standard-error"));
    ui.checkbox(&mut d.classifier, t("stats-roc-classifier"));
    ui.checkbox(&mut d.coordinates_roc, t("stats-roc-coordinates-roc"));
    ui.checkbox(&mut d.coordinates_pr, t("stats-roc-coordinates-pr"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-roc-options")).show(ui, |ui| {
        ui.label(t("stats-roc-positive-test-is"));
        ui.radio_value(&mut d.small_positive, false, t("stats-roc-larger-value"));
        ui.radio_value(&mut d.small_positive, true, t("stats-roc-smaller-value"));
        ui.checkbox(&mut d.exclude_cutoff, t("stats-roc-cutoff-negative"));
        field(ui, t("stats-roc-confidence-level"), &mut d.level, 50.0);
        ui.label(t("stats-roc-se-of-area"));
        ui.radio_value(&mut d.negexpo, false, t("stats-roc-nonparametric"));
        ui.add_enabled_ui(!(d.pair && !grouped), |ui| {
            ui.radio_value(&mut d.negexpo, true, t("stats-roc-bi-negative-exponential"));
        });
        ui.checkbox(&mut d.include, t("stats-roc-user-missing-valid"));
    });
}
