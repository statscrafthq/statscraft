//! Drawing of the complex samples General Linear Model and Logistic Regression dialogs.

use crate::advanced_dialogs::{forget_old_covariates, forget_old_factors};
use crate::advanced_dialogs_view::{means_editor, plan_path, terms_editor, tests_editor};
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::cs_model_dialog::{
    CS_ADJUSTMENTS, CS_CONTRASTS, CS_INTERCEPTS, CS_PRINT, CS_REFERENCES, CS_STATISTICS, CS_TESTS,
    GLM_SAVE, LOGISTIC_SAVE, OddsRatio, SampleModel,
};
use crate::genlin_dialog::GENLIN_MODELS;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use std::path::PathBuf;

pub fn sample_model(
    ui: &mut Ui,
    d: &mut SampleModel,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    plan_path(ui, &mut d.plan, false, folder);
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    if d.logistic {
        ui.horizontal(|ui| {
            ui.label("Reference category");
            for (i, label) in CS_REFERENCES.iter().enumerate() {
                ui.radio_value(&mut d.reference, i, *label);
            }
            small_field(ui, d.reference == 2, &mut d.reference_value);
        });
    }
    target_list(ui, "Factors", &mut d.factors, picked);
    forget_old_factors(&d.factors, &mut d.means);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    forget_old_covariates(&d.covariates, &mut d.means_at);
    ui.add_space(6.0);
    let model = d.model_variables();
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.add_enabled_ui(d.intercept != 2, |ui| {
            ui.horizontal(|ui| {
                for (i, label) in GENLIN_MODELS.iter().enumerate() {
                    ui.radio_value(&mut d.model, i, *label);
                }
            });
            ui.add_enabled_ui(d.model == 2, |ui| {
                terms_editor(ui, "model", &model, &mut d.terms);
            });
        });
        choice(ui, "Intercept", &CS_INTERCEPTS, &mut d.intercept);
        ui.add_enabled_ui(d.intercept == 0, |ui| {
            ui.checkbox(&mut d.show_intercept, "Show the intercept in the tables");
        });
    });
    let (statistics, tables) = d.offered();
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.label("Of the estimates");
        checkboxes(
            ui,
            &CS_STATISTICS[..statistics],
            &mut d.statistics[..statistics],
        );
        choice(ui, "Test statistic", &CS_TESTS, &mut d.test_type);
        choice(
            ui,
            "Adjustment of p values",
            &CS_ADJUSTMENTS,
            &mut d.adjustment,
        );
    });
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        ui.weak("INTERCEPT comes first in a row. End a row with = value to test against it");
        tests_editor(ui, &mut d.tests);
    });
    if d.logistic {
        egui::CollapsingHeader::new("Odds ratios").show(ui, |ui| {
            odds_editor(ui, d);
        });
    } else {
        egui::CollapsingHeader::new("EM means").show(ui, |ui| {
            means_editor(
                ui,
                &d.factors,
                &d.covariates,
                &mut d.means,
                &mut d.means_at,
                CS_CONTRASTS,
                &[],
            );
        });
    }
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Confidence interval %", &mut d.ci_level, 50.0);
        field(
            ui,
            "Degrees of freedom (empty: from the design)",
            &mut d.df,
            50.0,
        );
        if d.logistic {
            field(ui, "Maximum iterations", &mut d.iterations, 50.0);
            field(ui, "Maximum step-halving", &mut d.halvings, 50.0);
            field(ui, "Parameter convergence", &mut d.pconverge, 80.0);
            field(ui, "Log-likelihood convergence", &mut d.lconverge, 80.0);
            field(
                ui,
                "Check for separation from iteration (0: never)",
                &mut d.separation,
                50.0,
            );
        }
        ui.checkbox(
            &mut d.include,
            "User-missing values of factors and design variables are valid",
        );
    });
    egui::CollapsingHeader::new("Subpopulation").show(ui, |ui| {
        target_one(ui, "Variable", &mut d.domain, picked);
        field(ui, "Value", &mut d.domain_value, 80.0);
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &CS_PRINT[..tables], &mut d.print[..tables]);
        if d.logistic {
            ui.horizontal(|ui| {
                ui.label("Iteration history: every n-th iteration, n =");
                small_field(ui, d.print[7], &mut d.history);
            });
        }
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        let options = if d.logistic { LOGISTIC_SAVE } else { GLM_SAVE };
        for (i, (_, label)) in options.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
                ui.label(if d.logistic && i == 0 { "root" } else { "name" });
                small_field(ui, d.save[i], &mut d.save_names[i]);
            });
        }
        if d.logistic {
            ui.horizontal(|ui| {
                ui.label("Probabilities of the first categories, how many");
                small_field(ui, d.save[0], &mut d.probabilities);
            });
        }
    });
}

/// Odds ratios, each of one factor (against a reference level) or covariate (by its changes),
/// with the values the other variables are held at.
fn odds_editor(ui: &mut Ui, d: &mut SampleModel) {
    let model = d.model_variables();
    let mut remove = None;
    for (i, o) in d.odds.iter_mut().enumerate() {
        ui.push_id(("odds", i), |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Odds ratio {}", i + 1));
                let shown = match o.variable.as_str() {
                    "" => "choose",
                    v => v,
                };
                ComboBox::new("variable", "")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for n in &model {
                            ui.selectable_value(&mut o.variable, n.clone(), n.as_str());
                        }
                    });
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
            if o.variable.is_empty() {
                return;
            }
            let label = if d.covariates.contains(&o.variable) {
                "Changes (empty: 1)"
            } else {
                "Reference: LOW, HIGH or a value (empty: the highest)"
            };
            field(ui, label, &mut o.reference, 80.0);
            o.control.retain(|(n, _)| model.contains(n));
            let others: Vec<&String> = model.iter().filter(|n| **n != o.variable).collect();
            if !others.is_empty() {
                ui.label("Others held at (empty: the highest level or the mean)");
            }
            for n in others {
                let at = match o.control.iter().position(|(c, _)| c == n) {
                    Some(at) => at,
                    None => {
                        o.control.push((n.clone(), String::new()));
                        o.control.len() - 1
                    }
                };
                ui.push_id(("control", n), |ui| {
                    field(ui, n, &mut o.control[at].1, 60.0)
                });
            }
        });
    }
    if let Some(i) = remove {
        d.odds.remove(i);
    }
    if ui.small_button("Add odds ratio").clicked() {
        d.odds.push(OddsRatio::default());
    }
}
