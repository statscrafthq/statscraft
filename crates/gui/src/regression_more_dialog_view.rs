//! Drawing of the Curve Estimation, Probit Analysis and Weight Estimation dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::dialogs::labelled_range;
use crate::regression_more_dialog::{
    CURVE_CIN, CURVE_LOGISTIC, CURVE_MODELS, CURVE_SAVE, CurveEstimation, PROBIT_LOG_BASE,
    PROBIT_LOGS, PROBIT_MODELS, PROBIT_PRINT, Probit, WeightEstimation,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use statscraft_data::Dataset;

pub fn curve_estimation(ui: &mut Ui, d: &mut CurveEstimation, picked: &mut Vec<String>) {
    target_list(ui, "Dependents", &mut d.dependents, picked);
    ui.horizontal(|ui| {
        ui.label("Independent");
        ui.radio_value(&mut d.sequence, false, "A variable");
        ui.radio_value(&mut d.sequence, true, "The case sequence");
    });
    if !d.sequence {
        target_one(ui, "Independent", &mut d.independent, picked);
    }
    target_one(ui, "Case labels", &mut d.id, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Models").show(ui, |ui| {
        checkboxes(ui, &CURVE_MODELS, &mut d.models);
        ui.horizontal(|ui| {
            ui.label("Upper bound of Logistic (empty: none)");
            small_field(ui, d.models[CURVE_LOGISTIC], &mut d.upper_bound);
        });
        ui.weak("The last six curves need dependent values above 0");
        ui.checkbox(&mut d.constant, "Include a constant");
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.checkbox(&mut d.anova, "Analysis of variance for each model");
        ui.checkbox(&mut d.plot, "Chart of the curves");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        checkboxes(ui, &CURVE_SAVE, &mut d.save);
        ui.horizontal(|ui| {
            ui.label("Level of the intervals %");
            small_field(ui, d.save[CURVE_CIN], &mut d.ci_level);
        });
    });
}

pub fn probit(ui: &mut Ui, data: &Dataset, d: &mut Probit, picked: &mut Vec<String>) {
    target_one(ui, "Response frequency", &mut d.response, picked);
    target_one(ui, "Total observed", &mut d.total, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    let before = d.factor.name.clone();
    target_one(ui, "Factor", &mut d.factor.name, picked);
    // a new factor starts from its lowest and highest labelled value
    if d.factor.name != before {
        (d.factor.min, d.factor.max) = labelled_range(data, &d.factor.name);
    }
    if !d.factor.name.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Levels from");
            small_field(ui, true, &mut d.factor.min);
            ui.label("to");
            small_field(ui, true, &mut d.factor.max);
        });
        ui.weak("Each whole value in the range is a level; cases outside it are left out");
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        choice(ui, "Model", &PROBIT_MODELS, &mut d.model);
        ui.horizontal(|ui| {
            choice(ui, "Covariates take", &PROBIT_LOGS, &mut d.log);
            small_field(ui, d.log == PROBIT_LOG_BASE, &mut d.log_base);
        });
        ui.horizontal(|ui| {
            ui.label("Natural response rate");
            ui.radio_value(&mut d.natres, 0, "None");
            ui.radio_value(&mut d.natres, 1, "Estimated");
            ui.radio_value(&mut d.natres, 2, "Fixed at");
            small_field(ui, d.natres == 2, &mut d.natres_value);
        });
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &PROBIT_PRINT, &mut d.print);
        ui.weak("Levels, potency and the parallelism test need one covariate");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(
            ui,
            "Convergence (empty: the default)",
            &mut d.converge,
            60.0,
        );
        field(
            ui,
            "Maximum iterations (empty: the default)",
            &mut d.iterations,
            50.0,
        );
        field(ui, "Heterogeneity significance", &mut d.heterogeneity, 50.0);
        ui.weak("A worse fit than this widens the fiducial limits");
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
}

pub fn weight_estimation(ui: &mut Ui, d: &mut WeightEstimation, picked: &mut Vec<String>) {
    target_one(ui, "Dependent", &mut d.dependent, picked);
    target_list(ui, "Independents", &mut d.independents, picked);
    target_one(ui, "Weight source", &mut d.source, picked);
    ui.weak("Empty: the first independent");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Power").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.power_list, false, "From");
            small_field(ui, !d.power_list, &mut d.from);
            ui.label("to");
            small_field(ui, !d.power_list, &mut d.to);
            ui.label("by");
            small_field(ui, !d.power_list, &mut d.by);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.power_list, true, "These values");
            ui.add_enabled(
                d.power_list,
                egui::TextEdit::singleline(&mut d.powers).desired_width(160.0),
            );
        });
        ui.weak("A weight is the source to the minus power; left empty, the power is 1");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.constant, "Include a constant");
        ui.horizontal(|ui| {
            ui.label("Coefficients for");
            ui.radio_value(&mut d.print_all, false, "The best power");
            ui.radio_value(&mut d.print_all, true, "Every power");
        });
        ui.checkbox(
            &mut d.save_weight,
            "Save the best weights as a new variable",
        );
    });
}
