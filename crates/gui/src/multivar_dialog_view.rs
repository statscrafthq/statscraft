//! Drawing of the MANOVA (classic), Partial Least Squares and Nearest Neighbor dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::dialogs::labelled_range;
use crate::i18n::{t, t_args};
use crate::multivar_dialog::{
    KNN_LEVELS, KNN_METRICS, KNN_SAVE, Knn, MANOVA_ERRORS, MANOVA_PRINT, MANOVA_TRANSFORMS, Manova,
    Pls,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;

pub fn manova(ui: &mut Ui, data: &Dataset, d: &mut Manova, picked: &mut Vec<String>) {
    ui.weak(t("stats-multivar-classic-hint"));
    target_list(
        ui,
        t("stats-multivar-dependents"),
        &mut d.dependents,
        picked,
    );
    target_list(ui, t("stats-multivar-factors"), &mut d.factors, picked);
    // a new factor starts from its lowest and highest labelled value
    for f in d.factors.clone() {
        let range = d
            .ranges
            .entry(f.clone())
            .or_insert_with(|| labelled_range(data, &f));
        ui.horizontal(|ui| {
            ui.label(t_args(
                "stats-multivar-values-from",
                &[("factor", f.as_str().into())],
            ));
            small_field(ui, true, &mut range.0);
            ui.label(t("stats-multivar-to"));
            small_field(ui, true, &mut range.1);
        });
    }
    if !d.factors.is_empty() {
        ui.weak(t("stats-multivar-level-range-hint"));
    }
    target_list(
        ui,
        t("stats-multivar-covariates"),
        &mut d.covariates,
        picked,
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-multivar-repeated-measures")).show(ui, |ui| {
        ui.weak(t("stats-multivar-cells-hint"));
        ui.weak(t("stats-multivar-covariates-hint"));
        let mut remove = None;
        for (i, (name, levels)) in d.ws_factors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(t("stats-multivar-factor"));
                ui.add(egui::TextEdit::singleline(name).desired_width(90.0));
                ui.label(t("stats-multivar-levels"));
                small_field(ui, true, levels);
                if ui
                    .small_button("\u{d7}")
                    .on_hover_text(t("stats-multivar-remove"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.ws_factors.remove(i);
        }
        if ui.button(t("stats-multivar-add-ws-factor")).clicked() {
            d.ws_factors.push((String::new(), "2".into()));
        }
        if !d.ws_factors.is_empty() {
            field(ui, t("stats-multivar-measure-names"), &mut d.measure, 120.0);
            ui.horizontal(|ui| {
                ui.label(t("stats-multivar-ws-terms"));
                ui.radio_value(
                    &mut d.ws_main_effects,
                    false,
                    t("stats-multivar-all-interactions"),
                );
                ui.radio_value(
                    &mut d.ws_main_effects,
                    true,
                    t("stats-multivar-main-effects-only"),
                );
            });
            let cells: Option<usize> = d
                .ws_factors
                .iter()
                .map(|f| f.1.trim().parse::<usize>().ok())
                .product();
            let measures = d.measure.split_whitespace().count().max(1);
            if let Some(cells) = cells {
                ui.weak(t_args(
                    "stats-multivar-needs-dependents",
                    &[
                        ("needed", (cells * measures).to_string().into()),
                        ("count", d.dependents.len().to_string().into()),
                    ],
                ));
            }
        }
    });
    egui::CollapsingHeader::new(t("stats-multivar-transform")).show(ui, |ui| {
        ui.add_enabled_ui(d.ws_factors.is_empty(), |ui| {
            choice(
                ui,
                t("stats-multivar-matrix"),
                &MANOVA_TRANSFORMS,
                &mut d.transform,
            );
            ui.add_enabled_ui(d.transform != 0, |ui| {
                ui.checkbox(&mut d.orthonorm, t("stats-multivar-orthonormal"));
                ui.checkbox(&mut d.basis, t("stats-multivar-transposed-inverse"));
            });
        });
        if !d.ws_factors.is_empty() {
            ui.weak(t("stats-multivar-transform-unavailable"));
        }
    });
    egui::CollapsingHeader::new(t("stats-multivar-model")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(t("stats-multivar-terms"));
            ui.radio_value(
                &mut d.main_effects,
                false,
                t("stats-multivar-all-interactions"),
            );
            ui.radio_value(
                &mut d.main_effects,
                true,
                t("stats-multivar-main-effects-only"),
            );
        });
        choice(
            ui,
            t("stats-multivar-error-term"),
            &MANOVA_ERRORS,
            &mut d.error,
        );
        ui.checkbox(&mut d.sequential, t("stats-multivar-sequential"));
    });
    egui::CollapsingHeader::new(t("stats-multivar-output")).show(ui, |ui| {
        checkboxes(ui, &MANOVA_PRINT, &mut d.print);
        ui.weak(t("stats-multivar-last-four-hint"));
    });
}

pub fn pls(ui: &mut Ui, d: &mut Pls, picked: &mut Vec<String>) {
    target_list(
        ui,
        t("stats-multivar-scale-dependents"),
        &mut d.scale,
        picked,
    );
    target_list(
        ui,
        t("stats-multivar-categorical-dependents"),
        &mut d.categorical,
        picked,
    );
    target_list(ui, t("stats-multivar-factors"), &mut d.factors, picked);
    target_list(
        ui,
        t("stats-multivar-covariates"),
        &mut d.covariates,
        picked,
    );
    target_one(ui, t("stats-multivar-case-labels"), &mut d.id, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-multivar-model")).show(ui, |ui| {
        field(ui, t("stats-multivar-latent-at-most"), &mut d.latent, 50.0);
        ui.horizontal(|ui| {
            ui.label(t("stats-multivar-reference-category"));
            ui.radio_value(&mut d.first_reference, false, t("stats-multivar-last"));
            ui.radio_value(&mut d.first_reference, true, t("stats-multivar-first"));
        });
    });
    egui::CollapsingHeader::new(t("stats-multivar-save")).show(ui, |ui| {
        for (on, name, label) in [
            (
                &mut d.cases,
                &mut d.cases_name,
                t("stats-multivar-save-cases"),
            ),
            (
                &mut d.loadings,
                &mut d.loadings_name,
                t("stats-multivar-save-loadings"),
            ),
            (
                &mut d.predictors,
                &mut d.predictors_name,
                t("stats-multivar-save-predictors"),
            ),
        ] {
            ui.horizontal(|ui| {
                ui.checkbox(on, label);
                ui.label(t("stats-multivar-as-dataset"));
                ui.add_enabled(*on, egui::TextEdit::singleline(name).desired_width(110.0));
            });
        }
    });
}

pub fn knn(ui: &mut Ui, d: &mut Knn, picked: &mut Vec<String>) {
    target_one(ui, t("stats-multivar-target"), &mut d.target, picked);
    choice(
        ui,
        t("stats-multivar-target-level"),
        &KNN_LEVELS,
        &mut d.level,
    );
    target_list(ui, t("stats-multivar-factors"), &mut d.factors, picked);
    target_list(
        ui,
        t("stats-multivar-covariates"),
        &mut d.covariates,
        picked,
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-multivar-neighbors")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.auto, false, t("stats-multivar-fixed-k"));
            small_field(ui, !d.auto, &mut d.k);
        });
        // the fixed field is enabled when auto is off, the others when it is on
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.auto, true, t("stats-multivar-choose-k-from"));
            small_field(ui, d.auto, &mut d.kmin);
            ui.label(t("stats-multivar-to"));
            small_field(ui, d.auto, &mut d.kmax);
        });
        ui.horizontal(|ui| {
            ui.label(t("stats-multivar-folds"));
            small_field(ui, d.auto && !d.select_features, &mut d.folds);
        });
        choice(
            ui,
            t("stats-multivar-distance"),
            &KNN_METRICS,
            &mut d.metric,
        );
        ui.checkbox(&mut d.scale_covariates, t("stats-multivar-rescale"));
        ui.checkbox(&mut d.select_features, t("stats-multivar-select-features"));
        ui.checkbox(&mut d.weight_features, t("stats-multivar-weight-features"));
        ui.checkbox(&mut d.median, t("stats-multivar-median"));
    });
    egui::CollapsingHeader::new(t("stats-multivar-partition-header")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.by_variable, false, t("stats-multivar-at-random"));
            small_field(ui, !d.by_variable, &mut d.training);
            ui.label(t("stats-multivar-holdout"));
            small_field(ui, !d.by_variable, &mut d.holdout);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.by_variable, true, t("stats-multivar-by-variable"));
        });
        if d.by_variable {
            target_one(
                ui,
                t("stats-multivar-partition-variable"),
                &mut d.partition_variable,
                picked,
            );
        }
    });
    egui::CollapsingHeader::new(t("stats-multivar-output")).show(ui, |ui| {
        ui.checkbox(&mut d.show_model, t("stats-multivar-show-model"));
        ui.checkbox(
            &mut d.include_missing,
            t("stats-multivar-keep-user-missing"),
        );
    });
    egui::CollapsingHeader::new(t("stats-multivar-save")).show(ui, |ui| {
        for (i, (_, label)) in KNN_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], t(label));
                ui.add_enabled(
                    d.save[i],
                    egui::TextEdit::singleline(&mut d.save_names[i])
                        .hint_text(t("stats-multivar-default-name"))
                        .desired_width(110.0),
                );
            });
        }
    });
}
