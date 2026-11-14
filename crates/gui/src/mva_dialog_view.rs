//! Drawing of the Missing Value Analysis dialog.

use crate::app::{choice, field, target_list, target_one};
use crate::model_dialogs_view::name_checkboxes;
use crate::mva_dialog::{
    EM_DISTRIBUTIONS, Estimation, MVA_ADDTYPES, MVA_OUTPUT, MissingValueAnalysis,
};
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};
use std::path::PathBuf;

pub fn mva(
    ui: &mut Ui,
    d: &mut MissingValueAnalysis,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    target_list(ui, "Quantitative variables", &mut d.quantitative, picked);
    target_list(ui, "Categorical variables", &mut d.categorical, picked);
    ui.weak("String variables are always categorical");
    field(ui, "Most categories", &mut d.maxcat, 50.0);
    target_one(ui, "Label cases by", &mut d.id, picked);
    ui.add_space(6.0);
    ui.checkbox(&mut d.univariate, "Univariate statistics");
    ui.horizontal(|ui| {
        ui.label("Estimate by");
        ui.checkbox(&mut d.listwise, "Listwise");
        ui.checkbox(&mut d.pairwise, "Pairwise");
        ui.checkbox(&mut d.em.on, "EM");
        ui.checkbox(&mut d.regression.on, "Regression");
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Patterns").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.tpattern,
                "Tabulated patterns, leaving out those under",
            );
            small_field(ui, d.tpattern, &mut d.tpattern_percent);
            ui.label("% of cases");
            ui.add_enabled(
                d.tpattern,
                egui::Checkbox::new(&mut d.tpattern_nosort, "Keep the variable order"),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.mpattern, "Cases with missing values");
            ui.add_enabled(
                d.mpattern,
                egui::Checkbox::new(&mut d.mpattern_nosort, "Keep the variable order"),
            );
        });
        ui.checkbox(&mut d.dpattern, "Every case");
        ui.add_enabled_ui(d.dpattern, |ui| {
            target_list(ui, "Sort cases by", &mut d.sort, picked);
            if !d.sort.is_empty() {
                ui.label("Descending");
                name_checkboxes(ui, &d.sort, &mut d.descending);
            }
        });
        target_list(ui, "Describe with", &mut d.describe, picked);
        ui.weak("Their values or means show beside each pattern table");
    });
    egui::CollapsingHeader::new("Descriptives").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ttest, "T tests by variables missing in at least");
            small_field(ui, d.ttest, &mut d.ttest_percent);
            ui.label("% of cases");
            ui.add_enabled(d.ttest, egui::Checkbox::new(&mut d.prob, "Probabilities"));
        });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut d.crosstab,
                "Crosstabs of variables missing in at least",
            );
            small_field(ui, d.crosstab, &mut d.crosstab_percent);
            ui.label("% of cases");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.mismatch, "Mismatches, leaving out patterns under");
            small_field(ui, d.mismatch, &mut d.mismatch_percent);
            ui.label("% of cases");
            ui.add_enabled(
                d.mismatch,
                egui::Checkbox::new(&mut d.mismatch_nosort, "Keep the variable order"),
            );
        });
    });
    let quantitative = d.quantitative.clone();
    egui::CollapsingHeader::new("EM").show(ui, |ui| {
        ui.push_id("em", |ui| {
            ui.add_enabled_ui(d.em.on, |ui| {
                let em = &mut d.em;
                choice(ui, "Distribution", &EM_DISTRIBUTIONS, &mut em.distribution);
                match em.distribution {
                    1 => field(ui, "Degrees of freedom", &mut em.df, 50.0),
                    2 => {
                        field(ui, "Ratio of standard deviations", &mut em.lambda, 50.0);
                        field(ui, "Mixing proportion", &mut em.proportion, 50.0);
                    }
                    _ => {}
                }
                field(ui, "Maximum iterations", &mut em.iterations, 50.0);
                field(ui, "Convergence", &mut em.convergence, 80.0);
                estimation(ui, em, &quantitative, folder);
            })
            .response
            .on_disabled_hover_text("Turn on EM under Estimate by");
        });
    });
    egui::CollapsingHeader::new("Regression").show(ui, |ui| {
        ui.push_id("regression", |ui| {
            ui.add_enabled_ui(d.regression.on, |ui| {
                let r = &mut d.regression;
                ui.horizontal(|ui| {
                    choice(ui, "Add to each estimate", &MVA_ADDTYPES, &mut r.addtype);
                    if r.addtype == 3 {
                        ui.label("df");
                        small_field(ui, true, &mut r.df);
                    }
                });
                ui.weak(
                    "Automatic adds a complete case's residual, or a normal draw when under half the cases are complete",
                );
                field(ui, "Most predictors (empty for no limit)", &mut r.npredictors, 50.0);
                field(ui, "F to enter", &mut r.flimit, 50.0);
                estimation(ui, r, &quantitative, folder);
            })
            .response
            .on_disabled_hover_text("Turn on Regression under Estimate by");
        });
    });
}

/// What EM and Regression share: the variables, the tolerance and the saved data.
fn estimation(
    ui: &mut Ui,
    e: &mut Estimation,
    quantitative: &[String],
    folder: &mut Option<PathBuf>,
) {
    field(ui, "Tolerance", &mut e.tolerance, 80.0);
    ui.label("Estimate (none checked: all quantitative variables)");
    name_checkboxes(ui, quantitative, &mut e.predicted);
    ui.label("From (none checked: all quantitative variables)");
    ui.push_id("predictors", |ui| {
        name_checkboxes(ui, quantitative, &mut e.predictors);
    });
    ui.label("Save the data with the missing values filled in");
    output_picker(
        ui,
        &MVA_OUTPUT,
        &mut e.output,
        &mut e.file,
        &mut e.dataset,
        folder,
    );
}
