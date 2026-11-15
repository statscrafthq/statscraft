//! Drawing of the Multiple Imputation dialogs.

use crate::app::{choice, field, target_list, target_one};
use crate::imputation_dialog::{
    HISTORY_OUTPUT, IMPUTE_METHODS, IMPUTE_ROLES, IMPUTED_OUTPUT, Imputation, Limits,
};
use crate::model_dialogs_view::name_checkboxes;
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, TextEdit, Ui};
use std::path::PathBuf;

pub fn imputation(
    ui: &mut Ui,
    d: &mut Imputation,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    target_list(ui, "Variables in the model", &mut d.variables, picked);
    ui.weak("At least two; the measurement level picks the model of each");
    target_one(ui, "Analysis weight", &mut d.weight, picked);
    if d.patterns_only {
        ui.add_space(6.0);
        summaries(ui, d);
        return;
    }
    field(ui, "Number of imputations", &mut d.nimputations, 50.0);
    ui.label("Save the imputed data to");
    ui.push_id("imputed", |ui| {
        output_picker(
            ui,
            &IMPUTED_OUTPUT,
            &mut d.output,
            &mut d.file,
            &mut d.dataset,
            folder,
        );
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Method").show(ui, |ui| {
        choice(ui, "Method", &IMPUTE_METHODS, &mut d.method);
        ui.weak("Automatic takes monotone when the pattern allows it, otherwise iterative");
        ui.add_enabled_ui(d.custom(), |ui| {
            ui.add_enabled_ui(d.method == 1, |ui| {
                field(ui, "Iterations", &mut d.maxiter, 50.0);
            });
            ui.horizontal(|ui| {
                ui.label("Scale variables by");
                ui.radio_value(&mut d.pmm, false, "Linear regression");
                ui.radio_value(
                    &mut d.pmm,
                    true,
                    "Predictive mean matching from the nearest",
                );
                small_field(ui, d.pmm, &mut d.pmm_k);
            });
            ui.checkbox(
                &mut d.twoway,
                "Two-way interactions of categorical predictors",
            );
            field(ui, "Singularity tolerance", &mut d.singular, 80.0);
        })
        .response
        .on_disabled_hover_text("Automatic keeps its own model settings");
        field(
            ui,
            "Leave out variables missing in more than (%, empty for no limit)",
            &mut d.maxpctmissing,
            50.0,
        );
        field(ui, "Most parameters in a model", &mut d.maxmodelparam, 50.0);
    });
    egui::CollapsingHeader::new("Constraints").show(ui, |ui| {
        constraints(ui, d);
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        summaries(ui, d);
        ui.add_space(4.0);
        ui.checkbox(&mut d.models, "Imputation models");
        ui.checkbox(
            &mut d.descriptives,
            "Descriptive statistics of the imputed variables",
        );
        ui.add_enabled_ui(d.descriptives, |ui| {
            ui.label("Describe (none checked: every imputed variable)");
            name_checkboxes(ui, &d.variables, &mut d.described);
        });
        ui.add_space(4.0);
        ui.add_enabled_ui(d.has_history(), |ui| {
            ui.label("Save the iteration history to");
            ui.push_id("history", |ui| {
                output_picker(
                    ui,
                    &HISTORY_OUTPUT,
                    &mut d.history,
                    &mut d.history_file,
                    &mut d.history_dataset,
                    folder,
                );
            });
        })
        .response
        .on_disabled_hover_text("The monotone method does not iterate");
    });
}

/// The summaries of the missing values.
fn summaries(ui: &mut Ui, d: &mut Imputation) {
    ui.checkbox(&mut d.overall, "Overall summary of missing values");
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.variable_summary, "Variables missing in at least");
        small_field(ui, d.variable_summary, &mut d.minpct);
        ui.label("% of cases, at most");
        small_field(ui, d.variable_summary, &mut d.maxvars);
        ui.label("variables");
    });
    ui.checkbox(&mut d.patterns, "Patterns of missing values");
}

/// A row per variable of the model: its role, limits and rounding.
fn constraints(ui: &mut Ui, d: &mut Imputation) {
    for name in &d.variables {
        if !d.limits.iter().any(|l| &l.name == name) {
            d.limits.push(Limits {
                name: name.clone(),
                ..Default::default()
            });
        }
    }
    let bounded = d.bounded();
    egui::Grid::new("limits").num_columns(5).show(ui, |ui| {
        for label in ["Variable", "Role", "Minimum", "Maximum", "Round to"] {
            ui.strong(label);
        }
        ui.end_row();
        for name in &d.variables {
            let Some(l) = d.limits.iter_mut().find(|l| &l.name == name) else {
                continue;
            };
            ui.label(name.as_str());
            ComboBox::from_id_salt(("role", name.as_str()))
                .selected_text(IMPUTE_ROLES[l.role].1)
                .show_index(ui, &mut l.role, IMPUTE_ROLES.len(), |i| IMPUTE_ROLES[i].1);
            for text in [&mut l.min, &mut l.max] {
                ui.add_enabled(bounded, TextEdit::singleline(text).desired_width(70.0))
                    .on_disabled_hover_text("Predictive mean matching takes observed values");
            }
            ui.add(TextEdit::singleline(&mut l.rnd).desired_width(50.0));
            ui.end_row();
        }
    });
    ui.weak("Limits and rounding apply to scale variables; a date limit is written as a date");
    field(
        ui,
        "Draws per case within the limits",
        &mut d.maxcasedraws,
        50.0,
    );
    field(ui, "Sets of parameters drawn", &mut d.maxparamdraws, 50.0);
}
