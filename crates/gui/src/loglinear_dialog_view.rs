//! Drawing of the Loglinear dialogs.

use crate::advanced_dialogs_view::terms_editor;
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::correspondence_dialog::TableVariable;
use crate::dialogs::labelled_range;
use crate::loglinear_dialog::{
    GENLOG_PLOTTED, GENLOG_PRINT, GENLOG_SAVE, GeneralLoglinear, HILO_PLOTS, HILO_PRINT,
    ModelSelection,
};
use crate::survival_dialog_view::save_fields;
use eframe::egui::{self, TextEdit, Ui};
use statscraft_data::Dataset;

pub fn model_selection(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut ModelSelection,
    picked: &mut Vec<String>,
) {
    target_list(ui, "Factors", &mut d.factors, picked);
    ranges(ui, data, d);
    target_one(ui, "Cell weights", &mut d.cweight, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom, false, "Saturated");
            ui.radio_value(&mut d.custom, true, "Custom generating class");
        });
        ui.add_enabled_ui(d.custom, |ui| {
            terms_editor(ui, "class", &d.factors, &mut d.terms);
        });
        ui.weak("Each term brings in every term made of part of its factors");
        field(
            ui,
            "Highest order of terms (empty: all)",
            &mut d.maxorder,
            50.0,
        );
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.backward, false, "Enter in one step");
            ui.radio_value(&mut d.backward, true, "Backward elimination");
        });
        ui.add_enabled_ui(d.backward, |ui| {
            field(ui, "Most steps", &mut d.maxsteps, 50.0);
            field(ui, "Probability to remove", &mut d.probability, 50.0);
        });
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.print_chosen, false, "Default tables");
            ui.radio_value(&mut d.print_chosen, true, "These tables");
        });
        ui.add_enabled_ui(d.print_chosen, |ui| {
            checkboxes(ui, &HILO_PRINT, &mut d.print);
        });
        ui.weak("By default a saturated model shows all four, another model the first two");
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        checkboxes(ui, &HILO_PLOTS, &mut d.plots);
        ui.weak("Drawn for a model that is not saturated");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        field(
            ui,
            "Convergence (empty: from the largest count)",
            &mut d.converge,
            60.0,
        );
        field(
            ui,
            "Added to each cell (saturated model)",
            &mut d.delta,
            50.0,
        );
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
}

/// A row per factor with the smallest and largest category value; a new factor starts from its
/// lowest and highest labelled value that is not user-missing.
fn ranges(ui: &mut Ui, data: &Dataset, d: &mut ModelSelection) {
    for name in &d.factors {
        if d.range_of(name).is_none() {
            let (min, max) = labelled_range(data, name);
            d.ranges.push(TableVariable {
                name: name.clone(),
                min,
                max,
                ..Default::default()
            });
        }
    }
    if d.factors.is_empty() {
        return;
    }
    egui::Grid::new("ranges").num_columns(3).show(ui, |ui| {
        for label in ["Factor", "From", "To"] {
            ui.strong(label);
        }
        ui.end_row();
        for name in &d.factors {
            let Some(r) = d.ranges.iter_mut().find(|r| &r.name == name) else {
                continue;
            };
            ui.label(name.as_str());
            ui.add(TextEdit::singleline(&mut r.min).desired_width(50.0));
            ui.add(TextEdit::singleline(&mut r.max).desired_width(50.0));
            ui.end_row();
        }
    });
    ui.weak("Every whole value of the range is a category; cases outside it are left out");
}

pub fn general_loglinear(ui: &mut Ui, d: &mut GeneralLoglinear, picked: &mut Vec<String>) {
    if d.logit {
        target_list(ui, "Dependent", &mut d.dependents, picked);
    }
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Cell covariates", &mut d.covariates, picked);
    target_one(ui, "Cell structure", &mut d.cstructure, picked);
    ui.weak("Each cell takes the mean of its cases; a cell weight of 0 leaves the cell out");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Contrasts").show(ui, |ui| {
        target_list(ui, "Generalized residuals", &mut d.gresid, picked);
        target_list(ui, "Generalized log-odds ratios", &mut d.glor, picked);
        ui.weak("Variables of coefficients per cell; those of a log-odds ratio sum to 0");
    });
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        if d.logit {
            ui.weak("A logit model is multinomial");
        } else {
            ui.horizontal(|ui| {
                ui.label("Distribution of counts");
                ui.radio_value(&mut d.multinomial, false, "Poisson");
                ui.radio_value(&mut d.multinomial, true, "Multinomial");
            });
        }
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom, false, "Saturated");
            ui.radio_value(&mut d.custom, true, "Custom");
        });
        ui.add_enabled_ui(d.custom, |ui| {
            terms_editor(ui, "model", &d.model_variables(), &mut d.terms);
        });
        ui.weak("Covariates enter only through custom terms");
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &GENLOG_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        ui.checkbox(
            &mut d.residual_plots,
            "Residuals against observed and expected counts",
        );
        ui.checkbox(&mut d.normal_plots, "Normal and detrended normal plots");
        ui.add_enabled_ui(d.residual_plots || d.normal_plots, |ui| {
            choice(ui, "Residuals", &GENLOG_PLOTTED, &mut d.plotted);
        });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_fields(ui, &GENLOG_SAVE, &mut d.save, &mut d.save_names);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        field(ui, "Convergence", &mut d.converge, 60.0);
        field(ui, "Added to each cell", &mut d.delta, 50.0);
        field(ui, "Confidence level %", &mut d.ci_level, 50.0);
        field(ui, "Redundancy tolerance", &mut d.eps, 60.0);
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
}
