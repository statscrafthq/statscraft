//! Drawing of the Spectral Analysis, Autoregression and 2-Stage Least Squares dialogs.

use crate::app::{choice, field, target_list, target_one};
use crate::forecast_more_dialog::{
    AREG_METHODS, Areg, CONSTANT_CHOICES, SPECTRA_AXES, SPECTRA_CROSS_PLOTS, SPECTRA_CROSS_SAVES,
    SPECTRA_NO_WINDOW, SPECTRA_PLOTS, SPECTRA_SAVES, SPECTRA_WINDOWS, Spectra, TwoSls,
};
use eframe::egui::{self, Ui};

pub fn spectra(ui: &mut Ui, d: &mut Spectra, picked: &mut Vec<String>) {
    target_list(ui, "Series", &mut d.variables, picked);
    ui.add_space(6.0);
    ui.checkbox(&mut d.center, "Subtract the mean of each series first");
    let two = d.variables.len() > 1;
    ui.add_enabled(
        two,
        egui::Checkbox::new(
            &mut d.cross,
            "Bivariate analysis: the first series with each later one",
        ),
    );
    let cross = d.cross && two;
    ui.add_space(6.0);
    ui.strong("Spectral window");
    choice(ui, "Window", &SPECTRA_WINDOWS, &mut d.window);
    ui.horizontal(|ui| {
        ui.label("Span");
        ui.add_enabled(
            d.window != SPECTRA_NO_WINDOW,
            egui::TextEdit::singleline(&mut d.span).desired_width(50.0),
        );
    });
    ui.weak("Empty span: the window's own (5 for Hamming); a span of 1 is the raw periodogram");
    ui.add_space(6.0);
    ui.strong("Plots");
    ui.checkbox(&mut d.no_plot, "Draw no plots");
    ui.add_enabled_ui(!d.no_plot, |ui| {
        egui::Grid::new("spectra_plots")
            .num_columns(2)
            .show(ui, |ui| {
                for (i, ((_, label), on)) in SPECTRA_PLOTS.iter().zip(&mut d.plots).enumerate() {
                    ui.add_enabled(
                        i < SPECTRA_CROSS_PLOTS || cross,
                        egui::Checkbox::new(on, *label),
                    );
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        choice(ui, "Axis", &SPECTRA_AXES, &mut d.axis);
        ui.weak("None ticked: the periodogram");
    });
    egui::CollapsingHeader::new("New variables").show(ui, |ui| {
        for (i, ((_, label, _), (on, root))) in SPECTRA_SAVES.iter().zip(&mut d.saves).enumerate() {
            ui.horizontal(|ui| {
                ui.add_enabled(
                    i < SPECTRA_CROSS_SAVES || cross,
                    egui::Checkbox::new(on, *label),
                );
                ui.add(egui::TextEdit::singleline(root).desired_width(70.0));
            });
        }
        ui.weak("The root of the names; a name that exists is skipped");
    });
}

pub fn areg(ui: &mut Ui, d: &mut Areg, picked: &mut Vec<String>) {
    target_one(ui, "Dependent", &mut d.dependent, picked);
    target_list(ui, "Independents", &mut d.independents, picked);
    ui.add_space(6.0);
    choice(ui, "Method", &AREG_METHODS, &mut d.method);
    choice(ui, "Constant", &CONSTANT_CHOICES, &mut d.constant);
    field(ui, "Starting rho", &mut d.rho, 60.0);
    ui.weak("Above -1 and below 1; empty: 0");
    field(ui, "Most iterations", &mut d.mxiter, 60.0);
    ui.weak("Empty: 10. Fitted values, residuals and limits: Time Series Options, new variables");
}

pub fn two_sls(ui: &mut Ui, d: &mut TwoSls, picked: &mut Vec<String>) {
    target_one(ui, "Dependent", &mut d.dependent, picked);
    target_list(ui, "Explanatory variables", &mut d.explanatory, picked);
    target_list(ui, "Instrumental variables", &mut d.instruments, picked);
    ui.weak("At least as many instruments as explanatory variables");
    ui.add_space(6.0);
    choice(ui, "Constant", &CONSTANT_CHOICES, &mut d.constant);
    ui.checkbox(&mut d.covariances, "Covariances of the estimates");
    ui.checkbox(&mut d.save_predicted, "Save the predicted values");
    ui.checkbox(&mut d.save_residuals, "Save the residuals");
}
