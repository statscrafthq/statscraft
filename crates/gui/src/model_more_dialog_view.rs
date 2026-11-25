//! Drawing of the Quantile Regression, Variance Components and Optimal Binning dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::i18n::t;
use crate::model_more_dialog::{
    BINNING_PRINT, OptimalBinning, QR_BANDWIDTHS, QR_METHODS, QR_PRINT, QR_SAVES,
    QuantileRegression, VARCOMP_ANOVA, VARCOMP_METHODS, VARCOMP_ML, VarianceComponents,
};
use eframe::egui::{self, Ui};

pub fn quantile_regression(ui: &mut Ui, d: &mut QuantileRegression, picked: &mut Vec<String>) {
    target_one(ui, t("stats-modelmore-dependent"), &mut d.dependent, picked);
    target_list(ui, t("stats-modelmore-factors"), &mut d.factors, picked);
    target_list(
        ui,
        t("stats-modelmore-covariates"),
        &mut d.covariates,
        picked,
    );
    ui.add_space(6.0);
    field(ui, t("stats-modelmore-quantiles"), &mut d.quantiles, 160.0);
    ui.weak(t("stats-modelmore-quantiles-hint"));
    ui.checkbox(&mut d.intercept, t("stats-modelmore-intercept"));
    ui.checkbox(&mut d.include, t("stats-modelmore-user-missing-factor"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-modelmore-estimation")).show(ui, |ui| {
        choice(ui, t("stats-modelmore-method"), &QR_METHODS, &mut d.method);
        ui.checkbox(&mut d.iid, t("stats-modelmore-iid"))
            .on_hover_text(t("stats-modelmore-iid-hint"));
        choice(
            ui,
            t("stats-modelmore-bandwidth"),
            &QR_BANDWIDTHS,
            &mut d.bandwidth,
        );
        field(
            ui,
            t("stats-modelmore-confidence-percent"),
            &mut d.level,
            50.0,
        );
    });
    egui::CollapsingHeader::new(t("stats-modelmore-output")).show(ui, |ui| {
        checkboxes(ui, &QR_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new(t("stats-modelmore-save")).show(ui, |ui| {
        for ((_, label), (on, name)) in QR_SAVES.iter().zip(&mut d.saves) {
            ui.horizontal(|ui| {
                ui.checkbox(on, t(label));
                ui.add_enabled(*on, egui::TextEdit::singleline(name).desired_width(90.0));
            });
        }
        ui.weak(t("stats-modelmore-save-names-hint"));
    });
}

pub fn variance_components(ui: &mut Ui, d: &mut VarianceComponents, picked: &mut Vec<String>) {
    target_one(ui, t("stats-modelmore-dependent"), &mut d.dependent, picked);
    target_list(ui, t("stats-modelmore-fixed-factors"), &mut d.fixed, picked);
    target_list(
        ui,
        t("stats-modelmore-random-factors"),
        &mut d.random,
        picked,
    );
    target_list(
        ui,
        t("stats-modelmore-covariates"),
        &mut d.covariates,
        picked,
    );
    target_one(
        ui,
        t("stats-modelmore-regression-weight"),
        &mut d.weight,
        picked,
    );
    ui.add_space(6.0);
    choice(
        ui,
        t("stats-modelmore-method"),
        &VARCOMP_METHODS,
        &mut d.method,
    );
    ui.checkbox(&mut d.intercept, t("stats-modelmore-intercept"));
    ui.checkbox(&mut d.include, t("stats-modelmore-include-user-missing"));
    ui.add_space(6.0);
    let iterates = VARCOMP_ML.contains(&d.method);
    egui::CollapsingHeader::new(t("stats-modelmore-criteria")).show(ui, |ui| {
        ui.add_enabled_ui(iterates, |ui| {
            field(ui, t("stats-modelmore-convergence"), &mut d.converge, 70.0);
            field(
                ui,
                t("stats-modelmore-most-iterations"),
                &mut d.iterate,
                70.0,
            );
        });
        field(ui, t("stats-modelmore-eps-label"), &mut d.eps, 70.0);
        ui.weak(t("stats-modelmore-criteria-hint"));
    });
    egui::CollapsingHeader::new(t("stats-modelmore-output")).show(ui, |ui| {
        ui.checkbox(&mut d.sums, t("stats-modelmore-sums-of-squares"));
        ui.add_enabled(
            VARCOMP_ANOVA.contains(&d.method),
            egui::Checkbox::new(&mut d.expected, t("stats-modelmore-expected-mean-squares")),
        );
        ui.horizontal(|ui| {
            ui.add_enabled(
                iterates,
                egui::Checkbox::new(&mut d.history, t("stats-modelmore-history-every")),
            );
            ui.add_enabled(
                iterates && d.history,
                egui::TextEdit::singleline(&mut d.every).desired_width(40.0),
            );
        });
    });
}

pub fn optimal_binning(ui: &mut Ui, d: &mut OptimalBinning, picked: &mut Vec<String>) {
    target_list(
        ui,
        t("stats-modelmore-variables-to-bin"),
        &mut d.variables,
        picked,
    );
    target_one(
        ui,
        t("stats-modelmore-guide-variable"),
        &mut d.guide,
        picked,
    );
    ui.weak(t("stats-modelmore-guide-hint"));
    ui.add_space(6.0);
    ui.add_enabled(
        !d.guide.is_empty(),
        egui::Checkbox::new(&mut d.equal, t("stats-modelmore-equal-instead")),
    );
    let equal = d.is_equal();
    ui.add_enabled_ui(equal, |ui| {
        field(ui, t("stats-modelmore-bins-at-most"), &mut d.bins, 50.0)
    });
    ui.add_enabled_ui(!equal, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.preprocess, t("stats-modelmore-first-group-into"));
            ui.add_enabled(
                d.preprocess,
                egui::TextEdit::singleline(&mut d.classes).desired_width(50.0),
            );
            ui.label(t("stats-modelmore-equal-classes"));
        });
    });
    field(ui, t("stats-modelmore-merge-label"), &mut d.merge, 50.0);
    ui.weak(t("stats-modelmore-merge-hint"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-modelmore-bin-ends")).show(ui, |ui| {
        ui.checkbox(&mut d.lower_observed, t("stats-modelmore-lower-observed"));
        ui.checkbox(&mut d.upper_observed, t("stats-modelmore-upper-observed"));
        ui.checkbox(&mut d.lower_exclusive, t("stats-modelmore-lower-exclusive"));
        ui.checkbox(&mut d.listwise, t("stats-modelmore-listwise"));
    });
    egui::CollapsingHeader::new(t("stats-modelmore-output")).show(ui, |ui| {
        for (i, ((_, label), flag)) in BINNING_PRINT.iter().zip(&mut d.print).enumerate() {
            ui.add_enabled(i < 2 || !equal, egui::Checkbox::new(flag, t(label)));
        }
    });
    egui::CollapsingHeader::new(t("stats-modelmore-save")).show(ui, |ui| {
        ui.checkbox(&mut d.save, t("stats-modelmore-bin-numbers"));
        ui.add_enabled_ui(d.save, |ui| {
            field(ui, t("stats-modelmore-names-each"), &mut d.into, 200.0)
        })
        .response
        .on_hover_text(t("stats-modelmore-names-hint"));
        field(ui, t("stats-modelmore-rules-file"), &mut d.rules, 200.0);
    });
}
