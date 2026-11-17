//! Drawing of the ROC Analysis dialog.

use crate::app::{choice, field, target_list, target_one};
use crate::roc_analysis_dialog::{CUTPOINT, LEVEL, ROC_CURVES, ROC_SELECTS, RocAnalysis};
use eframe::egui::{self, Ui};

pub fn roc_analysis(ui: &mut Ui, d: &mut RocAnalysis, picked: &mut Vec<String>) {
    target_list(ui, "Test variables", &mut d.tests, picked);
    target_one(ui, "State variable", &mut d.state, picked);
    field(ui, "Value of the positive state", &mut d.value, 80.0);
    ui.add_space(6.0);
    ui.strong("Comparison of areas");
    let grouped = d.grouped();
    ui.add_enabled(
        !grouped,
        egui::Checkbox::new(
            &mut d.pair,
            "Compare the test variables on the same cases (two or more)",
        ),
    );
    target_one(ui, "Grouping variable (optional)", &mut d.group, picked);
    if grouped {
        choice(ui, "Groups", &ROC_SELECTS, &mut d.select);
        match d.select {
            LEVEL => {
                ui.horizontal(|ui| {
                    ui.label("Values");
                    ui.add(egui::TextEdit::singleline(&mut d.first).desired_width(70.0));
                    ui.add(egui::TextEdit::singleline(&mut d.second).desired_width(70.0));
                });
            }
            CUTPOINT => field(ui, "Value", &mut d.cut, 70.0),
            _ => {
                ui.label("A numeric variable");
            }
        }
        ui.weak("Each test variable gets an area per group, and the groups are compared");
    }
    ui.add_space(6.0);
    ui.strong("Display");
    choice(ui, "Curves", &ROC_CURVES, &mut d.curve);
    let (roc, pr) = d.draws();
    ui.add_enabled(
        roc,
        egui::Checkbox::new(&mut d.reference, "Diagonal reference line on the ROC curve"),
    );
    ui.add_enabled(
        pr,
        egui::Checkbox::new(
            &mut d.pr_reference,
            "Share of positive cases on the precision-recall curve",
        ),
    );
    ui.add_enabled(
        pr,
        egui::Checkbox::new(
            &mut d.interpolate_fp,
            "Fill the precision-recall curve in by false positives",
        ),
    );
    ui.checkbox(
        &mut d.model_quality,
        "Chart of the lower bounds of the areas",
    );
    ui.checkbox(&mut d.se, "Standard error, test and interval of the areas");
    ui.checkbox(
        &mut d.classifier,
        "Classifier measures (Gini index, largest gap)",
    );
    ui.checkbox(&mut d.coordinates_roc, "Coordinates of the ROC curve");
    ui.checkbox(
        &mut d.coordinates_pr,
        "Coordinates of the precision-recall curve",
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.label("A positive test is");
        ui.radio_value(&mut d.small_positive, false, "A larger test value");
        ui.radio_value(&mut d.small_positive, true, "A smaller test value");
        ui.checkbox(
            &mut d.exclude_cutoff,
            "A value equal to the cutoff counts as negative",
        );
        field(ui, "Confidence level %", &mut d.level, 50.0);
        ui.label("Standard error of the area");
        ui.radio_value(&mut d.negexpo, false, "Nonparametric");
        ui.add_enabled_ui(!(d.pair && !grouped), |ui| {
            ui.radio_value(&mut d.negexpo, true, "Bi-negative exponential");
        });
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
}
