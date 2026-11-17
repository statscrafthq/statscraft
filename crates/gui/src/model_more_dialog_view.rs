//! Drawing of the Quantile Regression, Variance Components and Optimal Binning dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::model_more_dialog::{
    BINNING_PRINT, OptimalBinning, QR_BANDWIDTHS, QR_METHODS, QR_PRINT, QR_SAVES,
    QuantileRegression, VARCOMP_ANOVA, VARCOMP_METHODS, VARCOMP_ML, VarianceComponents,
};
use eframe::egui::{self, Ui};

pub fn quantile_regression(ui: &mut Ui, d: &mut QuantileRegression, picked: &mut Vec<String>) {
    target_one(ui, "Dependent", &mut d.dependent, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    field(ui, "Quantiles", &mut d.quantiles, 160.0);
    ui.weak("Numbers from 0 to 1 with spaces between; 0.5 is the median");
    ui.checkbox(&mut d.intercept, "Intercept in the model");
    ui.checkbox(&mut d.include, "Count user-missing factor values as valid");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Estimation").show(ui, |ui| {
        choice(ui, "Method", &QR_METHODS, &mut d.method);
        ui.checkbox(&mut d.iid, "Standard errors assume identical errors")
            .on_hover_text(
                "Off: the model is refitted around each quantile and every case gets its own density",
            );
        choice(ui, "Bandwidth", &QR_BANDWIDTHS, &mut d.bandwidth);
        field(ui, "Confidence level, percent", &mut d.level, 50.0);
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &QR_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for ((_, label), (on, name)) in QR_SAVES.iter().zip(&mut d.saves) {
            ui.horizontal(|ui| {
                ui.checkbox(on, *label);
                ui.add_enabled(*on, egui::TextEdit::singleline(name).desired_width(90.0));
            });
        }
        ui.weak(
            "Name left empty: PRED, RESID, CIPREDL, CIPREDU; several quantiles add their digits",
        );
    });
}

pub fn variance_components(ui: &mut Ui, d: &mut VarianceComponents, picked: &mut Vec<String>) {
    target_one(ui, "Dependent", &mut d.dependent, picked);
    target_list(ui, "Fixed factors", &mut d.fixed, picked);
    target_list(ui, "Random factors", &mut d.random, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    target_one(ui, "Regression weight", &mut d.weight, picked);
    ui.add_space(6.0);
    choice(ui, "Method", &VARCOMP_METHODS, &mut d.method);
    ui.checkbox(&mut d.intercept, "Intercept in the model");
    ui.checkbox(&mut d.include, "Include user-missing values");
    ui.add_space(6.0);
    let iterates = VARCOMP_ML.contains(&d.method);
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        ui.add_enabled_ui(iterates, |ui| {
            field(ui, "Convergence", &mut d.converge, 70.0);
            field(ui, "Most iterations", &mut d.iterate, 70.0);
        });
        field(ui, "Tolerance for redundant columns", &mut d.eps, 70.0);
        ui.weak("Empty: the command's own values (1E-8, 50 iterations, 1E-8)");
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        ui.checkbox(&mut d.sums, "Sums of squares");
        ui.add_enabled(
            VARCOMP_ANOVA.contains(&d.method),
            egui::Checkbox::new(&mut d.expected, "Expected mean squares (ANOVA methods)"),
        );
        ui.horizontal(|ui| {
            ui.add_enabled(
                iterates,
                egui::Checkbox::new(&mut d.history, "Iteration history, every"),
            );
            ui.add_enabled(
                iterates && d.history,
                egui::TextEdit::singleline(&mut d.every).desired_width(40.0),
            );
        });
    });
}

pub fn optimal_binning(ui: &mut Ui, d: &mut OptimalBinning, picked: &mut Vec<String>) {
    target_list(ui, "Variables to bin", &mut d.variables, picked);
    target_one(ui, "Guide variable", &mut d.guide, picked);
    ui.weak("With a guide the cuts separate its values best; without one the bins hold equal numbers of cases");
    ui.add_space(6.0);
    ui.add_enabled(
        !d.guide.is_empty(),
        egui::Checkbox::new(&mut d.equal, "Equal frequency bins instead of guided ones"),
    );
    let equal = d.is_equal();
    ui.add_enabled_ui(equal, |ui| field(ui, "Bins, at most", &mut d.bins, 50.0));
    ui.add_enabled_ui(!equal, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.preprocess, "First group the values into");
            ui.add_enabled(
                d.preprocess,
                egui::TextEdit::singleline(&mut d.classes).desired_width(50.0),
            );
            ui.label("equal frequency classes");
        });
    });
    field(
        ui,
        "Merge a bin smaller than this multiple of its smaller neighbour",
        &mut d.merge,
        50.0,
    );
    ui.weak("Empty or 0: no merging");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Bin ends").show(ui, |ui| {
        ui.checkbox(
            &mut d.lower_observed,
            "Lowest bin starts at the smallest value",
        );
        ui.checkbox(
            &mut d.upper_observed,
            "Highest bin ends at the largest value",
        );
        ui.checkbox(
            &mut d.lower_exclusive,
            "A bin excludes its lower end point and includes its upper",
        );
        ui.checkbox(&mut d.listwise, "Use only cases valid on every variable");
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        for (i, ((_, label), flag)) in BINNING_PRINT.iter().zip(&mut d.print).enumerate() {
            ui.add_enabled(i < 2 || !equal, egui::Checkbox::new(flag, *label));
        }
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        ui.checkbox(&mut d.save, "Bin numbers as new variables");
        ui.add_enabled_ui(d.save, |ui| {
            field(ui, "Names, one for each variable", &mut d.into, 200.0)
        })
        .response
        .on_hover_text("Left empty, the names are the variable's name with _bin added");
        field(
            ui,
            "Write the rules as syntax to the file",
            &mut d.rules,
            200.0,
        );
    });
}
