//! Drawing of the Curve Estimation, Probit Analysis and Weight Estimation dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::dialogs::labelled_range;
use crate::i18n::t;
use crate::regression_more_dialog::{
    CURVE_CIN, CURVE_LOGISTIC, CURVE_MODELS, CURVE_SAVE, CurveEstimation, PROBIT_LOG_BASE,
    PROBIT_LOGS, PROBIT_MODELS, PROBIT_PRINT, Probit, WeightEstimation,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;

pub fn curve_estimation(ui: &mut Ui, d: &mut CurveEstimation, picked: &mut Vec<String>) {
    target_list(ui, t("stats-regmore-dependents"), &mut d.dependents, picked);
    ui.horizontal(|ui| {
        ui.label(t("stats-regmore-independent"));
        ui.radio_value(&mut d.sequence, false, t("stats-regmore-a-variable"));
        ui.radio_value(&mut d.sequence, true, t("stats-regmore-case-sequence"));
    });
    if !d.sequence {
        target_one(
            ui,
            t("stats-regmore-independent"),
            &mut d.independent,
            picked,
        );
    }
    target_one(ui, t("stats-regmore-case-labels"), &mut d.id, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-regmore-models")).show(ui, |ui| {
        checkboxes(ui, &CURVE_MODELS, &mut d.models);
        ui.horizontal(|ui| {
            ui.label(t("stats-regmore-upper-bound-label"));
            small_field(ui, d.models[CURVE_LOGISTIC], &mut d.upper_bound);
        });
        ui.weak(t("stats-regmore-last-six-positive"));
        ui.checkbox(&mut d.constant, t("stats-regmore-include-constant"));
    });
    egui::CollapsingHeader::new(t("stats-regmore-output")).show(ui, |ui| {
        ui.checkbox(&mut d.anova, t("stats-regmore-anova-each-model"));
        ui.checkbox(&mut d.plot, t("stats-regmore-chart-curves"));
    });
    egui::CollapsingHeader::new(t("stats-regmore-save")).show(ui, |ui| {
        checkboxes(ui, &CURVE_SAVE, &mut d.save);
        ui.horizontal(|ui| {
            ui.label(t("stats-regmore-interval-level"));
            small_field(ui, d.save[CURVE_CIN], &mut d.ci_level);
        });
    });
}

pub fn probit(ui: &mut Ui, data: &Dataset, d: &mut Probit, picked: &mut Vec<String>) {
    target_one(
        ui,
        t("stats-regmore-response-frequency"),
        &mut d.response,
        picked,
    );
    target_one(ui, t("stats-regmore-total-observed"), &mut d.total, picked);
    target_list(ui, t("stats-regmore-covariates"), &mut d.covariates, picked);
    let before = d.factor.name.clone();
    target_one(ui, t("stats-regmore-factor"), &mut d.factor.name, picked);
    // a new factor starts from its lowest and highest labelled value
    if d.factor.name != before {
        (d.factor.min, d.factor.max) = labelled_range(data, &d.factor.name);
    }
    if !d.factor.name.is_empty() {
        ui.horizontal(|ui| {
            ui.label(t("stats-regmore-levels-from"));
            small_field(ui, true, &mut d.factor.min);
            ui.label(t("stats-regmore-to"));
            small_field(ui, true, &mut d.factor.max);
        });
        ui.weak(t("stats-regmore-level-range-hint"));
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-regmore-model")).show(ui, |ui| {
        choice(ui, t("stats-regmore-model"), &PROBIT_MODELS, &mut d.model);
        ui.horizontal(|ui| {
            choice(
                ui,
                t("stats-regmore-covariates-take"),
                &PROBIT_LOGS,
                &mut d.log,
            );
            small_field(ui, d.log == PROBIT_LOG_BASE, &mut d.log_base);
        });
        ui.horizontal(|ui| {
            ui.label(t("stats-regmore-natural-rate-label"));
            ui.radio_value(&mut d.natres, 0, t("stats-regmore-natres-none"));
            ui.radio_value(&mut d.natres, 1, t("stats-regmore-estimated"));
            ui.radio_value(&mut d.natres, 2, t("stats-regmore-fixed-at"));
            small_field(ui, d.natres == 2, &mut d.natres_value);
        });
    });
    egui::CollapsingHeader::new(t("stats-regmore-statistics")).show(ui, |ui| {
        checkboxes(ui, &PROBIT_PRINT, &mut d.print);
        ui.weak(t("stats-regmore-one-covariate-hint"));
    });
    egui::CollapsingHeader::new(t("stats-regmore-options")).show(ui, |ui| {
        field(
            ui,
            t("stats-regmore-convergence-default"),
            &mut d.converge,
            60.0,
        );
        field(
            ui,
            t("stats-regmore-iterations-default"),
            &mut d.iterations,
            50.0,
        );
        field(
            ui,
            t("stats-regmore-heterogeneity-label"),
            &mut d.heterogeneity,
            50.0,
        );
        ui.weak(t("stats-regmore-heterogeneity-hint"));
        ui.checkbox(&mut d.include, t("stats-regmore-include-user-missing"));
    });
}

pub fn weight_estimation(ui: &mut Ui, d: &mut WeightEstimation, picked: &mut Vec<String>) {
    target_one(ui, t("stats-regmore-dependent"), &mut d.dependent, picked);
    target_list(
        ui,
        t("stats-regmore-independents"),
        &mut d.independents,
        picked,
    );
    target_one(ui, t("stats-regmore-weight-source"), &mut d.source, picked);
    ui.weak(t("stats-regmore-weight-source-hint"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-regmore-power")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.power_list, false, t("stats-regmore-from"));
            small_field(ui, !d.power_list, &mut d.from);
            ui.label(t("stats-regmore-to"));
            small_field(ui, !d.power_list, &mut d.to);
            ui.label(t("stats-regmore-by"));
            small_field(ui, !d.power_list, &mut d.by);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.power_list, true, t("stats-regmore-these-values"));
            ui.add_enabled(
                d.power_list,
                egui::TextEdit::singleline(&mut d.powers).desired_width(160.0),
            );
        });
        ui.weak(t("stats-regmore-power-hint"));
    });
    egui::CollapsingHeader::new(t("stats-regmore-options")).show(ui, |ui| {
        ui.checkbox(&mut d.constant, t("stats-regmore-include-constant"));
        ui.horizontal(|ui| {
            ui.label(t("stats-regmore-coefficients-for"));
            ui.radio_value(&mut d.print_all, false, t("stats-regmore-best-power"));
            ui.radio_value(&mut d.print_all, true, t("stats-regmore-every-power"));
        });
        ui.checkbox(&mut d.save_weight, t("stats-regmore-save-weights"));
    });
}
