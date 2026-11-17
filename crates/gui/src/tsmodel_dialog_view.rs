//! Drawing of the Create Models and Apply Models dialogs.

use crate::app::{choice, field, file_dialog, target_list};
use crate::tsmodel_dialog::{
    TS_DETAILS, TS_FIT, TS_METHODS, TS_SAVES, TS_SERIES_PLOTS, TS_SMOOTHING, TS_TRANSFORMS,
    TsOutputs, Tsapply, Tsmodel,
};
use eframe::egui::{self, Ui};

/// A model file field with Browse; `save` picks a new file.
fn model_file(ui: &mut Ui, label: &str, path: &mut String, save: bool) {
    ui.horizontal(|ui| {
        ui.strong(label);
        if ui.button("Browse...").clicked() {
            let dialog = file_dialog(&None).add_filter("Model file", &["json"]);
            let chosen = if save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(mut chosen) = chosen {
                if save && chosen.extension().is_none() {
                    chosen.set_extension("json");
                }
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
    ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
}

fn flags<const N: usize>(ui: &mut Ui, table: &[(&str, &str); N], on: &mut [bool; N]) {
    for ((_, label), flag) in table.iter().zip(on.iter_mut()) {
        ui.checkbox(flag, *label);
    }
}

/// The outputs, saved variables and period; `estimated` is false for TSAPPLY as saved, which
/// cannot show what needs the data estimated again.
fn outputs(ui: &mut Ui, d: &mut TsOutputs, estimated: bool) {
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.weak("Fit measures in the table of each model; none ticked hides the table");
        flags(ui, &TS_FIT, &mut d.fit);
        ui.add_space(4.0);
        ui.strong("Details of each model");
        for (i, ((_, label), flag)) in TS_DETAILS.iter().zip(&mut d.details).enumerate() {
            ui.add_enabled(estimated || i < 2, egui::Checkbox::new(flag, *label));
        }
    });
    egui::CollapsingHeader::new("Charts").show(ui, |ui| {
        for (i, ((_, label), flag)) in TS_SERIES_PLOTS.iter().zip(&mut d.plots).enumerate() {
            ui.add_enabled(
                estimated || i == 0 || i == 1 || i == 3,
                egui::Checkbox::new(flag, *label),
            );
        }
    });
    egui::CollapsingHeader::new("New variables").show(ui, |ui| {
        for (i, ((_, label), (on, root))) in TS_SAVES.iter().zip(&mut d.saves).enumerate() {
            ui.horizontal(|ui| {
                ui.add_enabled(estimated || i < 3, egui::Checkbox::new(on, *label));
                ui.add(egui::TextEdit::singleline(root).desired_width(70.0));
            });
        }
        ui.weak("The root of the names; empty: the command's own (Predicted, LCL, UCL, NResidual)");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Last case to forecast", &mut d.last, 60.0);
        ui.weak("Empty: the forecast period already set (Forecast Period)");
        field(ui, "Confidence level (%)", &mut d.level, 60.0);
        ui.weak("Empty: 95");
        field(ui, "Season length", &mut d.season, 60.0);
        ui.weak("Empty: the period of the date variables");
    });
}

pub fn tsmodel(ui: &mut Ui, d: &mut Tsmodel, picked: &mut Vec<String>) {
    target_list(ui, "Dependent series", &mut d.dependents, picked);
    target_list(ui, "Independents (predictors)", &mut d.independents, picked);
    ui.weak("Predictors work with ARIMA and the expert modeler, not with exponential smoothing");
    ui.add_space(6.0);
    choice(ui, "Method", &TS_METHODS, &mut d.method);
    match d.method {
        0 => {
            ui.checkbox(&mut d.expert_smoothing, "Try exponential smoothing models");
            ui.checkbox(&mut d.expert_arima, "Try ARIMA models");
            ui.checkbox(&mut d.seasonal, "Try seasonal models");
        }
        1 => {
            choice(ui, "Type", &TS_SMOOTHING, &mut d.smoothing);
            choice(ui, "Transform", &TS_TRANSFORMS, &mut d.transform);
        }
        _ => {
            ui.weak("Orders or lags: one number is an order, several numbers are the lags");
            field(ui, "Autoregressive (AR)", &mut d.ar, 80.0);
            field(ui, "Moving average (MA)", &mut d.ma, 80.0);
            field(ui, "Seasonal AR", &mut d.ar_seasonal, 80.0);
            field(ui, "Seasonal MA", &mut d.ma_seasonal, 80.0);
            field(ui, "Differencing", &mut d.diff, 50.0);
            field(ui, "Seasonal differencing", &mut d.diff_seasonal, 50.0);
            ui.checkbox(&mut d.constant, "Include a constant");
            choice(ui, "Transform", &TS_TRANSFORMS, &mut d.transform);
        }
    }
    ui.add_space(6.0);
    outputs(ui, &mut d.outputs, true);
    ui.add_space(6.0);
    model_file(ui, "Save the models to", &mut d.file, true);
    ui.weak("Empty: not saved");
    field(ui, "Model name prefix", &mut d.prefix, 100.0);
    ui.weak("Empty: Model (Model_1, Model_2 ...)");
}

pub fn tsapply(ui: &mut Ui, d: &mut Tsapply) {
    model_file(ui, "Model file", &mut d.file, false);
    ui.add_space(6.0);
    ui.checkbox(
        &mut d.reestimate,
        "Estimate the models again on the current data",
    );
    ui.weak("Off: the saved estimates make the forecasts; residuals and fitted values need it on");
    field(ui, "Models", &mut d.models, 200.0);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.drop, false, "Use only these");
        ui.radio_value(&mut d.drop, true, "Use all but these");
    });
    ui.weak("Model names such as Model_1, separated by spaces; empty: every model");
    ui.add_space(6.0);
    outputs(ui, &mut d.outputs, d.reestimate);
    ui.add_space(6.0);
    model_file(ui, "Save the models used to", &mut d.out_file, true);
    ui.weak("Empty: not saved");
}
