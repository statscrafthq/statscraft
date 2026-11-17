//! Drawing of the complex samples General Linear Model and Logistic Regression dialogs.

use crate::advanced_dialogs::{forget_old_covariates, forget_old_factors};
use crate::advanced_dialogs_view::{means_editor, plan_path, terms_editor, tests_editor};
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::cs_model_dialog::{
    COX_BASELINES, COX_INTERVALS, COX_PLOTS, COX_PRINT, COX_SAVE, COX_TIES, CS_ADJUSTMENTS,
    CS_CONTRASTS, CS_INTERCEPTS, CS_LEVELS, CS_PRINT, CS_STATISTICS, CS_TESTS, CsCox, GLM_SAVE,
    Held, LOGISTIC_SAVE, ORDINAL_LINKS, ORDINAL_PARALLEL, ORDINAL_SAVE, OddsRatio, SampleModel,
};
use crate::genlin_dialog::GENLIN_MODELS;
use crate::stats_dialogs_view::small_field;
use crate::survival_dialog_view::{event_fields, save_fields};
use eframe::egui::{self, ComboBox, TextEdit, Ui};
use std::path::PathBuf;

pub fn sample_model(
    ui: &mut Ui,
    d: &mut SampleModel,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    plan_path(ui, &mut d.plan, false, folder);
    if let Some(c) = &mut d.cox {
        target_one(
            ui,
            "Start time (empty: from the beginning)",
            &mut c.start,
            picked,
        );
        target_one(ui, "End time", &mut d.dependent, picked);
        target_one(ui, "Status", &mut c.status, picked);
        event_fields(
            ui,
            &mut c.event_range,
            &mut c.event_values,
            &mut c.event_from,
            &mut c.event_to,
        );
        ui.weak("List values with spaces or commas; a string status takes commas and no range");
        target_one(ui, "Baseline strata (optional)", &mut c.strata, picked);
    } else {
        target_one(ui, "Dependent variable", &mut d.dependent, picked);
    }
    if let Some(o) = &mut d.ordinal {
        ui.checkbox(
            &mut o.descending,
            "Order the categories from the highest down",
        );
    } else if d.logistic {
        ui.horizontal(|ui| {
            ui.label("Reference category");
            for (i, label) in CS_LEVELS.iter().enumerate() {
                ui.radio_value(&mut d.reference, i, *label);
            }
            small_field(ui, d.reference == 2, &mut d.reference_value);
        });
    }
    target_list(ui, "Factors", &mut d.factors, picked);
    forget_old_factors(&d.factors, &mut d.means);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    forget_old_covariates(&d.covariates, &mut d.means_at);
    d.forget_old_odds();
    ui.add_space(6.0);
    let model = d.model_variables();
    let ordinal = d.ordinal.is_some();
    let cox = d.cox.is_some();
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        if let Some(o) = &mut d.ordinal {
            choice(ui, "Link function", &ORDINAL_LINKS, &mut o.link);
        }
        ui.add_enabled_ui(d.intercept != 2 || ordinal, |ui| {
            ui.horizontal(|ui| {
                for (i, label) in GENLIN_MODELS.iter().enumerate() {
                    ui.radio_value(&mut d.model, i, *label);
                }
            });
            ui.add_enabled_ui(d.model == 2, |ui| {
                terms_editor(ui, "model", &model, &mut d.terms);
            });
        });
        // the thresholds of an ordinal model take the intercept's place, and the baseline
        // hazard of a survival model does
        if !ordinal && !cox {
            choice(ui, "Intercept", &CS_INTERCEPTS, &mut d.intercept);
            ui.add_enabled_ui(d.intercept == 0, |ui| {
                ui.checkbox(&mut d.show_intercept, "Show the intercept in the tables");
            });
        }
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
    if !ordinal {
        egui::CollapsingHeader::new("Tests").show(ui, |ui| {
            if !cox {
                ui.weak("INTERCEPT comes first in a row");
            }
            ui.weak("End a row with = value to test against it");
            tests_editor(ui, &mut d.tests);
        });
    }
    if d.logistic {
        egui::CollapsingHeader::new("Odds ratios").show(ui, |ui| {
            odds_editor(ui, d);
        });
    } else if !cox {
        egui::CollapsingHeader::new("EM means").show(ui, |ui| {
            means_editor(
                ui,
                &d.factors,
                &d.covariates,
                &mut d.means,
                &mut d.means_at,
                &CS_CONTRASTS,
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
        if let Some(c) = &mut d.cox {
            choice(ui, "Tied event times", &COX_TIES, &mut c.ties);
        }
        if d.logistic || cox {
            field(ui, "Maximum iterations", &mut d.iterations, 50.0);
            field(ui, "Maximum step-halving", &mut d.halvings, 50.0);
            field(ui, "Parameter convergence", &mut d.pconverge, 80.0);
            field(ui, "Log-likelihood convergence", &mut d.lconverge, 80.0);
        }
        if d.logistic {
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
    if let Some(o) = &mut d.ordinal {
        egui::CollapsingHeader::new("Parallel lines").show(ui, |ui| {
            checkboxes(ui, &ORDINAL_PARALLEL, &mut o.parallel);
        });
    }
    egui::CollapsingHeader::new("Subpopulation").show(ui, |ui| {
        target_one(ui, "Variable", &mut d.domain, picked);
        field(ui, "Value", &mut d.domain_value, 80.0);
    });
    if let Some(c) = &mut d.cox {
        cox_sections(ui, c, &d.factors, picked);
        return;
    }
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
        if let Some(o) = &mut d.ordinal {
            for (i, (_, label)) in ORDINAL_SAVE.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut o.save[i], *label);
                    ui.label(if i < 2 { "root" } else { "name" });
                    small_field(ui, o.save[i], &mut o.save_names[i]);
                    if i < 2 {
                        ui.label("how many");
                        let count = if i == 0 {
                            &mut o.cumulative_count
                        } else {
                            &mut o.category_count
                        };
                        small_field(ui, o.save[i], count);
                    }
                });
            }
            return;
        }
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

/// The sections only CSCOXREG has: the tables, the baseline functions with their plots and
/// patterns, and the saved variables.
fn cox_sections(ui: &mut Ui, c: &mut CsCox, factors: &[String], picked: &mut Vec<String>) {
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &COX_PRINT, &mut c.print);
        ui.horizontal(|ui| {
            ui.label("Iteration history: every n-th iteration, n =");
            small_field(ui, c.print[7], &mut c.history);
        });
    });
    egui::CollapsingHeader::new("Baseline functions").show(ui, |ui| {
        choice(ui, "Estimated by", &COX_BASELINES, &mut c.baseline);
        choice(
            ui,
            "Interval of the survival on",
            &COX_INTERVALS,
            &mut c.interval,
        );
        ui.label("Plots");
        checkboxes(ui, &COX_PLOTS, &mut c.plots);
        ui.checkbox(&mut c.plot_ci, "With confidence intervals");
        ui.separator();
        ui.label("Curves for a pattern of values (need a plot)");
        ui.weak("One line per variable: its name and the value. The others stay at the reference");
        ui.add(
            TextEdit::multiline(&mut c.pattern)
                .desired_rows(2)
                .desired_width(200.0),
        );
        target_one(ui, "A line for each level of", &mut c.pattern_by, picked);
        if !c.pattern_by.is_empty() && !factors.contains(&c.pattern_by) {
            ui.weak("Pick one of the factors");
        }
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_fields(ui, &COX_SAVE, &mut c.save, &mut c.save_names);
        ui.weak("The first three take a root: one variable per estimate");
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
                            let chosen =
                                ui.selectable_value(&mut o.variable, n.clone(), n.as_str());
                            // a new variable starts from its default level and no changes
                            if chosen.changed() {
                                o.level = 0;
                                o.reference.clear();
                                o.covariate = d.covariates.contains(n);
                            }
                        }
                    });
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
            if o.variable.is_empty() {
                return;
            }
            if d.covariates.contains(&o.variable) {
                field(ui, "Changes (empty: 1)", &mut o.reference, 80.0);
            } else {
                ui.push_id("reference", |ui| {
                    level_choice(ui, "Reference", false, &mut o.level, &mut o.reference);
                });
            }
            o.control.retain(|h| model.contains(&h.name));
            let others: Vec<&String> = model.iter().filter(|n| **n != o.variable).collect();
            if !others.is_empty() {
                ui.label("Others held at");
            }
            for n in others {
                let at = match o.control.iter().position(|h| &h.name == n) {
                    Some(at) => at,
                    None => {
                        o.control.push(Held {
                            name: n.clone(),
                            covariate: d.covariates.contains(n),
                            ..Default::default()
                        });
                        o.control.len() - 1
                    }
                };
                let covariate = d.covariates.contains(n);
                let h = &mut o.control[at];
                ui.push_id(("control", n), |ui| {
                    level_choice(ui, n, covariate, &mut h.level, &mut h.value);
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

/// The highest or lowest level of a factor or a value typed for it; for a covariate its mean
/// or a value.
fn level_choice(ui: &mut Ui, label: &str, covariate: bool, level: &mut usize, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        if covariate {
            ui.radio_value(level, 0, "Mean");
        } else {
            ui.radio_value(level, 0, CS_LEVELS[0]);
            ui.radio_value(level, 1, CS_LEVELS[1]);
        }
        ui.radio_value(level, 2, CS_LEVELS[2]);
        small_field(ui, *level == 2, value);
    });
}
