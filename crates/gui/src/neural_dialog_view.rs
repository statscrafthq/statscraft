//! Drawing of the Multilayer Perceptron and Radial Basis Function dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::neural_dialog::{
    COVARIATE_RESCALE, Common, DEPENDENT_RESCALE, HIDDEN, LEVELS, Mlp, OPTIMIZATION, OUTPUT, PLOT,
    PRINT, RBF_HIDDEN, Rbf, TRAINING,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};

/// The parts the two dialogs share; `middle` draws what belongs to the network, between the
/// partition and the output.
fn body(ui: &mut Ui, d: &mut Common, picked: &mut Vec<String>, middle: impl FnOnce(&mut Ui)) {
    target_list(ui, "Dependent variables", &mut d.dependents, picked);
    choice(ui, "Dependents are", &LEVELS, &mut d.level);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Rescaling").show(ui, |ui| {
        choice(
            ui,
            "Covariates",
            &COVARIATE_RESCALE,
            &mut d.rescale_covariates,
        );
        choice(
            ui,
            "Scale dependents",
            &DEPENDENT_RESCALE,
            &mut d.rescale_dependents,
        );
    });
    egui::CollapsingHeader::new("Partition").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.by_variable, false, "At random, training");
            small_field(ui, !d.by_variable, &mut d.training);
            ui.label("testing");
            small_field(ui, !d.by_variable, &mut d.testing);
            ui.label("holdout");
            small_field(ui, !d.by_variable, &mut d.holdout);
        });
        ui.radio_value(
            &mut d.by_variable,
            true,
            "By a variable (above 0 trains, 0 tests, below 0 holds out)",
        );
        if d.by_variable {
            target_one(ui, "Partition variable", &mut d.partition_variable, picked);
        }
    });
    middle(ui);
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.strong("Tables");
        checkboxes(ui, &PRINT, &mut d.print);
        ui.strong("Charts");
        checkboxes(ui, &PLOT, &mut d.plot);
        ui.checkbox(&mut d.include_missing, "Keep user-missing categories");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        ui.checkbox(&mut d.save_predicted, "Predicted value or category");
        ui.add_enabled_ui(d.save_predicted, |ui| {
            field(
                ui,
                "Names, one per dependent",
                &mut d.predicted_names,
                160.0,
            );
        })
        .response
        .on_hover_text("Left empty, the names are the command's own");
        ui.checkbox(
            &mut d.save_probabilities,
            "Pseudo-probabilities of categorical dependents",
        );
        ui.add_enabled_ui(d.save_probabilities, |ui| {
            field(
                ui,
                "Name roots, one per dependent",
                &mut d.probability_roots,
                160.0,
            );
        })
        .response
        .on_hover_text("Left empty, the names are the command's own");
    });
}

pub fn mlp(ui: &mut Ui, d: &mut Mlp, picked: &mut Vec<String>) {
    let Mlp {
        common,
        automatic,
        min_units,
        max_units,
        layers,
        auto_units,
        units,
        hidden,
        output,
        training,
        batch_size,
        optimization,
        max_epochs,
        max_minutes,
        error_steps,
    } = d;
    body(ui, common, picked, |ui| {
        egui::CollapsingHeader::new("Architecture").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(automatic, true, "Choose the units of one layer, from");
                small_field(ui, *automatic, min_units);
                ui.label("to");
                small_field(ui, *automatic, max_units);
            });
            ui.radio_value(automatic, false, "Set the layers myself");
            ui.add_enabled_ui(!*automatic, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Hidden layers");
                    ui.radio_value(layers, 1, "1");
                    ui.radio_value(layers, 2, "2");
                });
                ui.checkbox(auto_units, "Let the command choose the units");
                ui.add_enabled_ui(!*auto_units, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Units in layer 1");
                        small_field(ui, true, &mut units[0]);
                        ui.label("layer 2");
                        small_field(ui, *layers == 2, &mut units[1]);
                    });
                });
                choice(ui, "Hidden units", &HIDDEN, hidden);
                choice(ui, "Output units", &OUTPUT, output);
            });
        });
        egui::CollapsingHeader::new("Training").show(ui, |ui| {
            choice(ui, "Training", &TRAINING, training);
            ui.horizontal(|ui| {
                ui.label("Mini-batch size");
                small_field(ui, *training == 2, batch_size);
            })
            .response
            .on_hover_text("Left empty, the size is chosen from the number of cases");
            choice(ui, "Optimization", &OPTIMIZATION, optimization);
            ui.horizontal(|ui| {
                ui.label("Stop after");
                small_field(ui, true, error_steps);
                ui.label("steps without a new lowest error");
            });
            ui.horizontal(|ui| {
                ui.label("Stop after");
                small_field(ui, true, max_minutes);
                ui.label("minutes (empty: no time limit)");
            });
            ui.horizontal(|ui| {
                ui.label("Stop after");
                small_field(ui, true, max_epochs);
                ui.label("passes over the data (empty: by the number of weights)");
            });
        });
    });
}

pub fn rbf(ui: &mut Ui, d: &mut Rbf, picked: &mut Vec<String>) {
    let Rbf {
        common,
        automatic,
        min_units,
        max_units,
        units,
        hidden,
        overlap,
    } = d;
    body(ui, common, picked, |ui| {
        egui::CollapsingHeader::new("Architecture").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(automatic, true, "Search the number of units, from");
                small_field(ui, *automatic, min_units);
                ui.label("to");
                small_field(ui, *automatic, max_units);
            })
            .response
            .on_hover_text("Leave both empty to let the command set the limits");
            ui.horizontal(|ui| {
                ui.radio_value(automatic, false, "Fixed number of units");
                small_field(ui, !*automatic, units);
            });
            choice(ui, "Hidden units", &RBF_HIDDEN, hidden);
            field(ui, "Overlap of the units (empty: automatic)", overlap, 50.0);
        });
    });
}
