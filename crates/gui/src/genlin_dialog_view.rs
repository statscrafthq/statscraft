//! Drawing of the Generalized Linear Models dialog.

use crate::advanced_dialogs::{NOMREG_BASES, forget_old_covariates, forget_old_factors};
use crate::advanced_dialogs_view::{means_editor, terms_editor};
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::genlin_dialog::{
    GENLIN_ADJUSTMENTS, GENLIN_ANALYSES, GENLIN_CONTRASTS, GENLIN_CORRELATIONS,
    GENLIN_DISTRIBUTIONS, GENLIN_LINKS, GENLIN_METHODS, GENLIN_MODELS, GENLIN_ORDERS, GENLIN_PRINT,
    GENLIN_SAVE, GENLIN_SCALES, GeneralizedLinear,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use statscraft_syntax::{GenlinDistribution, GenlinLink};

pub fn genlin(ui: &mut Ui, d: &mut GeneralizedLinear, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.label("Response");
        if ui.radio(!d.events, "One dependent variable").clicked() {
            d.set_events(false);
        }
        if ui.radio(d.events, "Events out of trials").clicked() && !d.events {
            d.set_events(true);
        }
    });
    if d.events {
        target_one(ui, "Events", &mut d.dependent, picked);
        ui.horizontal(|ui| {
            ui.label("Trials");
            ui.radio_value(&mut d.trials_by_variable, true, "Variable");
            ui.radio_value(
                &mut d.trials_by_variable,
                false,
                "Same number for every case",
            );
            small_field(ui, !d.trials_by_variable, &mut d.trials);
        });
        ui.add_enabled_ui(d.trials_by_variable, |ui| {
            target_one(ui, "Trials variable", &mut d.trials_variable, picked);
        });
    } else {
        target_one(ui, "Dependent variable", &mut d.dependent, picked);
    }
    ui.horizontal(|ui| {
        ui.label("Distribution");
        let mut chosen = d.distribution;
        ComboBox::new("distribution", "")
            .selected_text(GENLIN_DISTRIBUTIONS[chosen].1)
            .show_ui(ui, |ui| {
                for (i, (_, label)) in GENLIN_DISTRIBUTIONS.iter().enumerate() {
                    ui.add_enabled_ui(d.distribution_allowed(i), |ui| {
                        ui.selectable_value(&mut chosen, i, *label);
                    });
                }
            });
        if chosen != d.distribution {
            d.set_distribution(chosen);
        }
    });
    if matches!(
        GENLIN_DISTRIBUTIONS[d.distribution].0,
        GenlinDistribution::NegativeBinomial(_)
    ) {
        ui.horizontal(|ui| {
            ui.label("k");
            ui.radio_value(&mut d.estimate_k, false, "Value");
            small_field(ui, !d.estimate_k, &mut d.k);
            ui.radio_value(&mut d.estimate_k, true, "Estimated");
        });
    }
    ui.horizontal(|ui| {
        ui.label("Link");
        ComboBox::new("link", "")
            .selected_text(GENLIN_LINKS[d.link].1)
            .show_ui(ui, |ui| {
                for i in d.links() {
                    ui.selectable_value(&mut d.link, i, GENLIN_LINKS[i].1);
                }
            });
        if matches!(
            GENLIN_LINKS[d.link].0,
            GenlinLink::Power(_) | GenlinLink::OddsPower(_)
        ) {
            ui.label("exponent");
            small_field(ui, true, &mut d.power);
        }
    });
    if d.categorical() && GENLIN_DISTRIBUTIONS[d.distribution].0 == GenlinDistribution::Binomial {
        ui.horizontal(|ui| {
            ui.label("Reference category");
            for (i, label) in NOMREG_BASES.iter().enumerate() {
                ui.radio_value(&mut d.reference, i, *label);
            }
            small_field(ui, d.reference == 2, &mut d.reference_value);
        });
    } else if d.categorical() {
        choice(ui, "Category order", &GENLIN_ORDERS, &mut d.order);
    }
    target_list(ui, "Factors", &mut d.factors, picked);
    forget_old_factors(&d.factors, &mut d.means);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    forget_old_covariates(&d.covariates, &mut d.means_at);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, label) in GENLIN_MODELS.iter().enumerate() {
                ui.radio_value(&mut d.model, i, *label);
            }
        });
        ui.add_enabled_ui(d.model == 2, |ui| {
            let model = d.model_variables();
            terms_editor(ui, "model", &model, &mut d.terms);
        });
        // the ordinal multinomial always fits its thresholds
        let optional = d.intercept_optional();
        let mut forced = true;
        let intercept = if optional {
            &mut d.intercept
        } else {
            &mut forced
        };
        ui.add_enabled_ui(optional, |ui| {
            ui.checkbox(intercept, "Include intercept in the model");
        });
        target_one(ui, "Offset", &mut d.offset, picked);
        target_one(ui, "Scale weight", &mut d.scale_weight, picked);
    });
    egui::CollapsingHeader::new("Repeated measures").show(ui, |ui| repeated(ui, d, picked));
    egui::CollapsingHeader::new("Estimation").show(ui, |ui| {
        ui.horizontal(|ui| {
            choice(ui, "Method", &GENLIN_METHODS, &mut d.method);
            if d.method == 0 {
                ui.label("Fisher scoring iterations");
                small_field(ui, true, &mut d.fisher_iterations);
            }
        });
        ui.horizontal(|ui| {
            choice(ui, "Scale parameter", &GENLIN_SCALES, &mut d.scale);
            small_field(ui, d.scale == 3, &mut d.scale_value);
        });
        // under repeated measures the covariance is chosen in their section
        ui.add_enabled_ui(!d.repeated, |ui| {
            ui.horizontal(|ui| {
                ui.label("Covariance matrix");
                ui.radio_value(&mut d.robust, false, "Model-based");
                ui.radio_value(&mut d.robust, true, "Robust");
            });
        });
        field(ui, "Confidence interval %", &mut d.ci_level, 50.0);
        // with repeated measures these settle the starting model only
        let (iterations, convergence) = match d.repeated {
            true => (
                "Maximum iterations of the starting model",
                "Parameter convergence of the starting model",
            ),
            false => ("Maximum iterations", "Parameter convergence"),
        };
        field(ui, iterations, &mut d.iterations, 50.0);
        field(ui, "Maximum step-halving", &mut d.halvings, 50.0);
        field(ui, convergence, &mut d.convergence, 80.0);
        ui.horizontal(|ui| {
            ui.label("Log-likelihood");
            ui.radio_value(&mut d.kernel, false, "Full");
            ui.radio_value(&mut d.kernel, true, "Kernel");
        });
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.horizontal(|ui| {
            choice(ui, "Tests of effects", &GENLIN_ANALYSES, &mut d.analysis);
            // estimating equations have Wald tests only; a tick stays removable
            ui.add_enabled(
                !d.repeated || d.likelihood_ratio,
                egui::Checkbox::new(&mut d.likelihood_ratio, "Likelihood ratio"),
            );
        });
        let mut print = GENLIN_PRINT;
        if d.repeated {
            print[0].1 = "Case processing and correlated data summary";
            // the estimating equations have no omnibus test
            print[4].1 = "Tests of effects";
        }
        checkboxes(ui, &print, &mut d.print);
        ui.add_enabled_ui(d.print[5], |ui| {
            ui.checkbox(&mut d.exponentiated, "Exponentiated parameter estimates");
        });
        if d.repeated {
            ui.checkbox(&mut d.working_corr, "Working correlation matrix");
        }
    });
    egui::CollapsingHeader::new("EM means").show(ui, |ui| {
        if !d.means_allowed() {
            ui.weak("Not available for the ordinal multinomial distribution");
        }
        // left enabled while tables remain, so they can be removed
        ui.add_enabled_ui(d.means_allowed() || !d.means.is_empty(), |ui| {
            means_editor(
                ui,
                &d.factors,
                &d.covariates,
                &mut d.means,
                &mut d.means_at,
                &GENLIN_CONTRASTS,
                &GENLIN_ADJUSTMENTS,
            );
            ui.checkbox(
                &mut d.transformed,
                "Means on the scale of the linear predictor",
            );
        });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        let allowed = d.saves_allowed();
        egui::Grid::new("save").num_columns(2).show(ui, |ui| {
            for (i, ((_, label), flag)) in GENLIN_SAVE.iter().zip(&mut d.save).enumerate() {
                // a tick stays removable after the distribution changed
                ui.add_enabled(allowed[i] || *flag, egui::Checkbox::new(flag, *label));
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    });
    egui::CollapsingHeader::new("Missing values").show(ui, |ui| {
        ui.checkbox(&mut d.include, "User-missing values of factors are valid");
    });
}

/// Repeated measures of subjects, fitted by generalized estimating equations.
fn repeated(ui: &mut Ui, d: &mut GeneralizedLinear, picked: &mut Vec<String>) {
    ui.checkbox(
        &mut d.repeated,
        "Repeated measures of subjects (generalized estimating equations)",
    );
    ui.add_enabled_ui(d.repeated, |ui| {
        target_list(ui, "Subject variables", &mut d.subjects, picked);
        target_list(ui, "Within-subject variables", &mut d.within, picked);
        ui.weak("Subjects and their measurements are categories: factors or variables outside the model, never covariates.");
        if !d.sort {
            ui.weak("Without sorting each run of equal subject values is a subject with its records in file order; the within-subject variables still decide which cases are complete.");
        }
        ui.checkbox(
            &mut d.sort,
            "Sort the cases by subject and within-subject values",
        );
        ui.horizontal(|ui| {
            choice(
                ui,
                "Working correlation",
                &GENLIN_CORRELATIONS,
                &mut d.correlation,
            );
            if GENLIN_CORRELATIONS[d.correlation].0 == "MDEPENDENT" {
                ui.label("m");
                small_field(ui, true, &mut d.m);
            }
        });
        if GENLIN_CORRELATIONS[d.correlation].0 == "FIXED" {
            field(ui, "Below the diagonal, row by row", &mut d.fixed, 200.0);
        }
        ui.checkbox(
            &mut d.adjust,
            "Adjust the correlation estimate by the number of parameters",
        );
        ui.horizontal(|ui| {
            ui.label("Covariance matrix");
            ui.radio_value(&mut d.gee_robust, true, "Robust");
            ui.radio_value(&mut d.gee_robust, false, "Model-based");
        });
        field(ui, "Maximum iterations", &mut d.gee_iterations, 50.0);
        field(
            ui,
            "Update the matrix every n iterations (0 never)",
            &mut d.update,
            50.0,
        );
        ui.horizontal(|ui| {
            ui.label("Parameter convergence");
            ui.add(egui::TextEdit::singleline(&mut d.gee_convergence).desired_width(80.0));
            ui.radio_value(&mut d.gee_relative, false, "Absolute");
            ui.radio_value(&mut d.gee_relative, true, "Relative");
        });
        field(ui, "Hessian convergence (0 not used)", &mut d.hessian, 80.0);
    });
}
