//! Drawing of the Categorical Regression dialog.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::catreg_dialog::{
    CATREG_INITIALS, CATREG_LEVELS, CATREG_MISSING, CATREG_PENALTIES, CATREG_PRINT,
    CATREG_RESAMPLES, CATREG_SAVE, CategoricalRegression, ENET, PRINT_QUANT, PRINT_REGU,
};
use crate::model_dialogs_view::name_checkboxes;
use crate::scaling_dialog::{SCALING_DISCRETIZE, ScalingVariable};
use crate::scaling_dialog_view::save_section;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};

pub fn catreg(ui: &mut Ui, d: &mut CategoricalRegression, picked: &mut Vec<String>) {
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    target_list(ui, "Independent variables", &mut d.independents, picked);
    let all = d.variables();
    for name in &all {
        if !d.settings.iter().any(|s| &s.name == name) {
            d.settings.push(ScalingVariable::new(name));
        }
    }
    if !all.is_empty() {
        ui.add_space(6.0);
        egui::Grid::new("catreg variables")
            .num_columns(4)
            .show(ui, |ui| {
                for label in ["Variable", "Scaling level", "Recode", "Missing values"] {
                    ui.strong(label);
                }
                ui.end_row();
                for name in &all {
                    let Some(s) = d.settings.iter_mut().find(|s| &s.name == name) else {
                        continue;
                    };
                    ui.label(name.as_str());
                    ui.horizontal(|ui| {
                        let level = &mut s.level;
                        ComboBox::from_id_salt((name, "level"))
                            .selected_text(CATREG_LEVELS[level.level].1)
                            .show_index(ui, &mut level.level, CATREG_LEVELS.len(), |i| {
                                CATREG_LEVELS[i].1
                            });
                        if level.spline() {
                            ui.label("degree");
                            small_field(ui, true, &mut level.degree);
                            ui.label("knots");
                            small_field(ui, true, &mut level.knots);
                        }
                    });
                    ui.horizontal(|ui| {
                        ComboBox::from_id_salt((name, "recode"))
                            .selected_text(SCALING_DISCRETIZE[s.discretize].1)
                            .show_index(ui, &mut s.discretize, SCALING_DISCRETIZE.len(), |i| {
                                SCALING_DISCRETIZE[i].1
                            });
                        match s.discretize {
                            1 => {
                                small_field(ui, true, &mut s.groups);
                                ui.checkbox(&mut s.uniform, "even counts");
                            }
                            2 => {
                                ui.label("width");
                                small_field(ui, true, &mut s.width);
                            }
                            _ => {}
                        }
                    });
                    ComboBox::from_id_salt((name, "missing"))
                        .selected_text(CATREG_MISSING[s.level.missing].1)
                        .show_index(ui, &mut s.level.missing, CATREG_MISSING.len(), |i| {
                            CATREG_MISSING[i].1
                        });
                    ui.end_row();
                }
            });
        ui.weak("A spline has a degree from 1 and interior knots from 0");
        ui.weak("Codes are whole numbers from 1; fractions are cut into 7 groups unless recoded");
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Most iterations", &mut d.max_iterations, 50.0);
        field(
            ui,
            "Stop when the fit improves less than",
            &mut d.convergence,
            70.0,
        );
        ui.weak("From 0.000001 to 0.1");
        choice(ui, "Start", &CATREG_INITIALS, &mut d.initial);
        choice(ui, "Penalty", &CATREG_PENALTIES, &mut d.penalty);
        if d.penalized() {
            if d.penalty != 2 {
                range_fields(
                    ui,
                    if d.penalty == ENET {
                        "Ridge"
                    } else {
                        "Penalties"
                    },
                    if d.penalty == ENET {
                        &mut d.enet_ridge
                    } else {
                        &mut d.ridge
                    },
                );
            }
            if d.penalty >= 2 {
                range_fields(
                    ui,
                    if d.penalty == ENET {
                        "Lasso"
                    } else {
                        "Penalties"
                    },
                    &mut d.lasso,
                );
            }
            ui.weak("Each value from start to stop by step makes a model");
        }
        choice(ui, "Prediction error", &CATREG_RESAMPLES, &mut d.resample);
        if d.resample > 0 {
            field(
                ui,
                if d.resample == 1 { "Folds" } else { "Samples" },
                &mut d.resample_count,
                50.0,
            );
            ui.weak("Empty: 10 folds or 50 samples");
        }
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &CATREG_PRINT, &mut d.print);
        if !d.penalized() {
            d.print[PRINT_REGU] = false;
            ui.weak("The penalty table needs a penalty");
        }
        if d.print[PRINT_QUANT] {
            ui.label("Quantifications of");
            ui.push_id("quant", |ui| name_checkboxes(ui, &all, &mut d.quant));
            ui.weak("None checked: every variable");
        }
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        ui.checkbox(&mut d.plot_trans, "Transformations");
        if d.plot_trans {
            ui.push_id("trans", |ui| {
                name_checkboxes(ui, &all, &mut d.plot_trans_variables)
            });
        }
        ui.checkbox(&mut d.plot_resid, "Residuals of the predictors");
        if d.plot_resid {
            ui.push_id("resid", |ui| {
                name_checkboxes(ui, &d.independents, &mut d.plot_resid_variables)
            });
        }
        ui.add_enabled(
            d.penalized(),
            egui::Checkbox::new(&mut d.plot_regu, "Coefficients against the penalty"),
        );
        ui.weak("None checked in a list: every variable");
        field(
            ui,
            "Longest label, empty for full labels",
            &mut d.label_length,
            40.0,
        );
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_section(ui, &CATREG_SAVE, &mut d.save, &mut d.save_roots);
    });
}

/// The start, stop and step of a penalty.
fn range_fields(ui: &mut Ui, label: &str, values: &mut [String; 3]) {
    ui.horizontal(|ui| {
        ui.label(format!("{label}: from"));
        small_field(ui, true, &mut values[0]);
        ui.label("to");
        small_field(ui, true, &mut values[1]);
        ui.label("by");
        small_field(ui, true, &mut values[2]);
    });
}
