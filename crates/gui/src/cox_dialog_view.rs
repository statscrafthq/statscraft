//! Drawing of the Cox Regression dialog.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::cox_dialog::{COX_DEFAULT_CONTRAST, COX_PLOTS, COX_SAVE, Cox, TIME_COVARIATE};
use crate::model_dialogs::LOGISTIC_METHODS;
use crate::model_dialogs_view::categorical_grid;
use crate::stats_dialogs_view::{blocks, small_field};
use crate::survival_dialog_view::{event_fields, save_fields};
use eframe::egui::{self, ComboBox, TextEdit, Ui};
use statscraft_data::Dataset;

pub fn cox(ui: &mut Ui, data: &Dataset, d: &mut Cox, picked: &mut Vec<String>) {
    target_one(ui, "Time", &mut d.time, picked);
    target_one(ui, "Status", &mut d.status, picked);
    event_fields(
        ui,
        &mut d.event_range,
        &mut d.event_values,
        &mut d.event_from,
        &mut d.event_to,
    );
    ui.weak("List values with spaces or commas; a string status takes commas and no range");
    ui.add_space(6.0);
    let block = blocks(ui, &mut d.blocks, &mut d.block);
    // the picked variables as one interaction term, before Add takes them one by one
    let term = picked.join(" BY ");
    if ui
        .add_enabled(picked.len() >= 2, egui::Button::new("Add as a*b").small())
        .on_hover_text("Add the picked variables to this block as one interaction")
        .clicked()
        && !block.variables.contains(&term)
    {
        block.variables.push(term);
        picked.clear();
    }
    target_list(ui, "Covariates", &mut block.variables, picked);
    choice(ui, "Method", &LOGISTIC_METHODS, &mut block.method);
    target_one(ui, "Strata", &mut d.strata, picked);
    ui.add_space(6.0);
    let covariates = d.covariates();
    egui::CollapsingHeader::new("Categorical").show(ui, |ui| {
        categorical_grid(ui, &covariates, &mut d.categorical, COX_DEFAULT_CONTRAST);
        ui.weak("String covariates are always categorical");
    });
    let timed = !d.time_expression.trim().is_empty();
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        // left enabled so choices made before the expression can still be cleared
        if timed {
            ui.weak("Plots and patterns are not available with a time-dependent covariate");
        }
        {
            checkboxes(ui, &COX_PLOTS, &mut d.plots);
            ui.horizontal(|ui| {
                ui.label("Separate lines for");
                let shown = match d.pattern_by.as_str() {
                    "" => "none",
                    name => name,
                };
                ComboBox::from_id_salt("pattern_by")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.pattern_by, String::new(), "none");
                        let categorical = covariates.iter().filter(|c| {
                            d.categorical.iter().any(|k| &k.name == *c)
                                || data.variable(c).is_ok_and(|v| v.is_string())
                        });
                        for name in categorical {
                            ui.selectable_value(&mut d.pattern_by, name.clone(), name);
                        }
                    });
            });
            ui.label("Draw at these covariate values; an empty one stays at the mean");
            egui::Grid::new("pattern").num_columns(2).show(ui, |ui| {
                for name in &covariates {
                    let at = d.pattern.iter().position(|(n, _)| n == name);
                    let mut text = at.map(|i| d.pattern[i].1.clone()).unwrap_or_default();
                    ui.label(name.as_str());
                    let field = TextEdit::singleline(&mut text).desired_width(50.0);
                    // a row is kept only once a value is typed
                    if ui.add_enabled(*name != d.pattern_by, field).changed() {
                        match at {
                            Some(i) => d.pattern[i].1 = text,
                            None => d.pattern.push((name.clone(), text)),
                        }
                    }
                    ui.end_row();
                }
            });
        }
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_fields(ui, &COX_SAVE, &mut d.save, &mut d.save_names);
        if timed {
            ui.weak("With a time-dependent covariate only DfBeta can be saved");
        }
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ci, "Confidence interval for exp(B), level %");
            small_field(ui, d.ci, &mut d.ci_level);
        });
        ui.checkbox(&mut d.correlations, "Correlation of estimates");
        ui.checkbox(&mut d.baseline, "Baseline function");
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.last_step, false, "Every step");
            ui.radio_value(&mut d.last_step, true, "Last step of each block");
        });
        ui.horizontal(|ui| {
            ui.label("Probability for stepwise entry");
            small_field(ui, true, &mut d.entry);
            ui.label("removal");
            small_field(ui, true, &mut d.removal);
        });
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
    egui::CollapsingHeader::new("Time-Dependent Covariate").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("{TIME_COVARIATE} ="));
            ui.add(TextEdit::singleline(&mut d.time_expression).desired_width(220.0));
        });
        ui.weak(format!(
            "An expression in T_, the time; {TIME_COVARIATE} joins the first block"
        ));
    });
}
