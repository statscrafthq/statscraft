//! Drawing of the MANOVA (classic), Partial Least Squares and Nearest Neighbor dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::dialogs::labelled_range;
use crate::multivar_dialog::{
    KNN_LEVELS, KNN_METRICS, KNN_SAVE, Knn, MANOVA_ERRORS, MANOVA_PRINT, MANOVA_TRANSFORMS, Manova,
    Pls,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;

pub fn manova(ui: &mut Ui, data: &Dataset, d: &mut Manova, picked: &mut Vec<String>) {
    ui.weak("The classic MANOVA command, not the Multivariate dialog");
    target_list(ui, "Dependents", &mut d.dependents, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    // a new factor starts from its lowest and highest labelled value
    for f in d.factors.clone() {
        let range = d
            .ranges
            .entry(f.clone())
            .or_insert_with(|| labelled_range(data, &f));
        ui.horizontal(|ui| {
            ui.label(format!("{f}: values from"));
            small_field(ui, true, &mut range.0);
            ui.label("to");
            small_field(ui, true, &mut range.1);
        });
    }
    if !d.factors.is_empty() {
        ui.weak("Each whole value in the range is a level; cases outside it are left out");
    }
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Repeated measures").show(ui, |ui| {
        ui.weak("The dependents are the cells of the factors, the last factor changing fastest");
        ui.weak("Covariates then stay as they are over the cells, one value per case");
        let mut remove = None;
        for (i, (name, levels)) in d.ws_factors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("Factor");
                ui.add(egui::TextEdit::singleline(name).desired_width(90.0));
                ui.label("levels");
                small_field(ui, true, levels);
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.ws_factors.remove(i);
        }
        if ui.button("Add within-subjects factor").clicked() {
            d.ws_factors.push((String::new(), "2".into()));
        }
        if !d.ws_factors.is_empty() {
            field(ui, "Measure names (optional)", &mut d.measure, 120.0);
            ui.horizontal(|ui| {
                ui.label("Within-subjects terms");
                ui.radio_value(&mut d.ws_main_effects, false, "All interactions");
                ui.radio_value(&mut d.ws_main_effects, true, "Main effects only");
            });
            let cells: Option<usize> = d
                .ws_factors
                .iter()
                .map(|f| f.1.trim().parse::<usize>().ok())
                .product();
            let measures = d.measure.split_whitespace().count().max(1);
            if let Some(cells) = cells {
                ui.weak(format!(
                    "Needs {} dependents, it has {}",
                    cells * measures,
                    d.dependents.len()
                ));
            }
        }
    });
    egui::CollapsingHeader::new("Transform").show(ui, |ui| {
        ui.add_enabled_ui(d.ws_factors.is_empty(), |ui| {
            choice(ui, "Matrix", &MANOVA_TRANSFORMS, &mut d.transform);
            ui.add_enabled_ui(d.transform != 0, |ui| {
                ui.checkbox(&mut d.orthonorm, "Make the rows orthonormal");
                ui.checkbox(&mut d.basis, "Use the transposed inverse of the matrix");
            });
        });
        if !d.ws_factors.is_empty() {
            ui.weak("Not available with repeated measures, which transform the dependents");
        }
    });
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Terms");
            ui.radio_value(&mut d.main_effects, false, "All interactions");
            ui.radio_value(&mut d.main_effects, true, "Main effects only");
        });
        choice(ui, "Error term", &MANOVA_ERRORS, &mut d.error);
        ui.checkbox(
            &mut d.sequential,
            "Sequential sums of squares (each term adjusted for those before it)",
        );
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &MANOVA_PRINT, &mut d.print);
        ui.weak("The last four need two or more dependents");
    });
}

pub fn pls(ui: &mut Ui, d: &mut Pls, picked: &mut Vec<String>) {
    target_list(ui, "Scale dependents", &mut d.scale, picked);
    target_list(ui, "Categorical dependents", &mut d.categorical, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    target_one(ui, "Case labels", &mut d.id, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Latent factors (at most)", &mut d.latent, 50.0);
        ui.horizontal(|ui| {
            ui.label("Reference category of categorical dependents");
            ui.radio_value(&mut d.first_reference, false, "Last");
            ui.radio_value(&mut d.first_reference, true, "First");
        });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for (on, name, label) in [
            (
                &mut d.cases,
                &mut d.cases_name,
                "Cases: predictions and scores",
            ),
            (
                &mut d.loadings,
                &mut d.loadings_name,
                "Latent factors: loadings and weights",
            ),
            (
                &mut d.predictors,
                &mut d.predictors_name,
                "Predictors: coefficients and importance",
            ),
        ] {
            ui.horizontal(|ui| {
                ui.checkbox(on, label);
                ui.label("as dataset");
                ui.add_enabled(*on, egui::TextEdit::singleline(name).desired_width(110.0));
            });
        }
    });
}

pub fn knn(ui: &mut Ui, d: &mut Knn, picked: &mut Vec<String>) {
    target_one(ui, "Target", &mut d.target, picked);
    choice(ui, "Target level", &KNN_LEVELS, &mut d.level);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Neighbors").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.auto, false, "Fixed k");
            small_field(ui, !d.auto, &mut d.k);
        });
        // the fixed field is enabled when auto is off, the others when it is on
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.auto, true, "Choose k from");
            small_field(ui, d.auto, &mut d.kmin);
            ui.label("to");
            small_field(ui, d.auto, &mut d.kmax);
        });
        ui.horizontal(|ui| {
            ui.label("Cross-validation folds");
            small_field(ui, d.auto && !d.select_features, &mut d.folds);
        });
        choice(ui, "Distance", &KNN_METRICS, &mut d.metric);
        ui.checkbox(
            &mut d.scale_covariates,
            "Rescale covariates to the range -1 to 1",
        );
        ui.checkbox(&mut d.select_features, "Select features automatically");
        ui.checkbox(
            &mut d.weight_features,
            "Weight each feature by its importance",
        );
        ui.checkbox(&mut d.median, "Predict a scale target by the median");
    });
    egui::CollapsingHeader::new("Partition").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.by_variable, false, "At random, training");
            small_field(ui, !d.by_variable, &mut d.training);
            ui.label("holdout");
            small_field(ui, !d.by_variable, &mut d.holdout);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.by_variable, true, "By a variable (above 0 trains)");
        });
        if d.by_variable {
            target_one(ui, "Partition variable", &mut d.partition_variable, picked);
        }
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.checkbox(&mut d.show_model, "Show the model tables");
        ui.checkbox(&mut d.include_missing, "Keep user-missing categories");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for (i, (_, label)) in KNN_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
                ui.add_enabled(
                    d.save[i],
                    egui::TextEdit::singleline(&mut d.save_names[i])
                        .hint_text("default name")
                        .desired_width(110.0),
                );
            });
        }
    });
}
