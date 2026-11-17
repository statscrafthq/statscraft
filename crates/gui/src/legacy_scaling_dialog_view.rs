//! Drawing of the HOMALS, PRINCALS, OVERALS and ANACOR dialogs.

use crate::app::{checkboxes, choice, field, target_list};
use crate::correspondence_dialog_view::table_variable;
use crate::dialogs::{labelled_range, whole};
use crate::legacy_scaling_dialog::{
    ANACOR_NORMALIZATIONS, ANACOR_PLOTS, ANACOR_PRINT, Anacor, JOINT, LEGACY_LEVELS, LegacyScaling,
    LegacyVariable, ScalingKind,
};
use crate::model_dialogs_view::name_checkboxes;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use statscraft_data::Dataset;

pub fn legacy_scaling(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut LegacyScaling,
    picked: &mut Vec<String>,
) {
    let spec = d.spec();
    target_list(ui, "Variables", &mut d.variables, picked);
    // a new variable starts from its highest labelled category
    for name in &d.variables {
        if !d.settings.iter().any(|s| &s.name == name) {
            let max = labelled_range(data, name).1;
            d.settings.push(LegacyVariable::new(name, &max));
        }
    }
    let overals = d.kind == ScalingKind::Overals;
    let sets = whole(&d.sets, 2, "").ok().flatten().unwrap_or(2).min(99) as usize;
    if !d.variables.is_empty() {
        egui::Grid::new("legacy variables")
            .num_columns(4)
            .show(ui, |ui| {
                ui.strong("Variable");
                ui.strong("Largest category");
                if spec.levels {
                    ui.strong("Level");
                }
                if overals {
                    ui.strong("Set");
                }
                ui.end_row();
                for name in &d.variables {
                    let Some(s) = d.settings.iter_mut().find(|s| &s.name == name) else {
                        continue;
                    };
                    ui.label(name.as_str());
                    small_field(ui, true, &mut s.max);
                    if spec.levels {
                        ComboBox::from_id_salt((name, "level"))
                            .selected_text(LEGACY_LEVELS[s.level].1)
                            .show_index(ui, &mut s.level, LEGACY_LEVELS.len(), |i| {
                                LEGACY_LEVELS[i].1
                            });
                    }
                    if overals {
                        ComboBox::from_id_salt((name, "set"))
                            .selected_text(s.set.to_string())
                            .show_ui(ui, |ui| {
                                for k in 1..=sets.max(s.set) {
                                    ui.selectable_value(&mut s.set, k, k.to_string());
                                }
                            });
                    }
                    ui.end_row();
                }
            });
        ui.weak("The categories are the whole values from 1 up to the largest; anything else is missing");
    }
    if overals {
        field(ui, "Number of sets", &mut d.sets, 40.0);
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Dimensions in the solution", &mut d.dimensions, 40.0);
        field(ui, "Most iterations", &mut d.max_iterations, 50.0);
        field(
            ui,
            "Stop when the fit improves less than",
            &mut d.convergence,
            70.0,
        );
        if overals {
            ui.checkbox(&mut d.random_start, "Start from random scores");
        }
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, spec.print, &mut d.print);
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        checkboxes(ui, spec.plots, &mut d.plots);
        for (i, (key, label)) in spec.plots.iter().enumerate() {
            if d.plots[i] && spec.plot_lists.contains(key) {
                ui.label(format!("{label}: variables (none: the command's choice)"));
                ui.push_id(("plot", i), |ui| {
                    name_checkboxes(ui, &d.variables, &mut d.plot_variables[i])
                });
            }
        }
        field(
            ui,
            "Longest label, empty for the command's own",
            &mut d.label_length,
            40.0,
        );
        ndim_row(ui, &mut d.ndim, &mut d.ndim_first, &mut d.ndim_last);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        ui.checkbox(&mut d.save, "Object scores");
        ui.horizontal(|ui| {
            ui.label("root (up to 6 characters)");
            small_field(ui, d.save, &mut d.save_root);
            ui.label("dimensions");
            small_field(ui, d.save, &mut d.save_dims);
        });
        ui.weak("An empty root uses the command's own, an empty count saves every dimension");
    });
}

fn ndim_row(ui: &mut Ui, on: &mut bool, first: &mut String, last: &mut String) {
    ui.horizontal(|ui| {
        ui.checkbox(on, "Only dimensions from");
        small_field(ui, *on, first);
        ui.label("to");
        small_field(ui, *on, last);
    });
    ui.weak("An empty last dimension plots up to the highest one");
}

pub fn anacor(ui: &mut Ui, data: &Dataset, d: &mut Anacor, picked: &mut Vec<String>) {
    ui.weak("The older command; Correspondence Analysis offers more");
    table_variable(ui, data, "Row variable", &mut d.row, picked);
    table_variable(ui, data, "Column variable", &mut d.column, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Dimensions in the solution", &mut d.dimensions, 40.0);
        ui.horizontal(|ui| {
            choice(
                ui,
                "Normalization",
                &ANACOR_NORMALIZATIONS,
                &mut d.normalization,
            );
            let custom = d.normalization == ANACOR_NORMALIZATIONS.len() - 1;
            small_field(ui, custom, &mut d.q);
        });
        ui.weak("A custom value runs from -2 to 2; 1 is row principal, -1 column principal");
        ui.checkbox(&mut d.row_variances, "Spread of the row scores");
        ui.checkbox(&mut d.column_variances, "Spread of the column scores");
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &ANACOR_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        let joint = d.joint_allowed();
        egui::Grid::new("anacor plots")
            .num_columns(2)
            .show(ui, |ui| {
                for (i, (_, label)) in ANACOR_PLOTS.iter().enumerate() {
                    // shown off where the syntax leaves it out
                    let on = i != JOINT || joint;
                    let mut off = false;
                    let flag = if on { &mut d.plots[i] } else { &mut off };
                    ui.add_enabled(on, egui::Checkbox::new(flag, *label))
                        .on_disabled_hover_text("Not drawn with principal normalization");
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        field(
            ui,
            "Longest label, empty for the command's own",
            &mut d.label_length,
            40.0,
        );
        ndim_row(ui, &mut d.ndim, &mut d.ndim_first, &mut d.ndim_last);
    });
}
