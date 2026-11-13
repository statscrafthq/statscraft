//! Drawing of the Ordinal Regression, Multinomial Logistic Regression, Linear Mixed Models and
//! complex samples dialogs, and the editors of terms, means and tests they share with the
//! Generalized Linear Models dialog.

use crate::advanced_dialogs::forget_old_factors;
use crate::advanced_dialogs::{
    Advanced, ContrastTest, LinearMixed, MIXED_COMPARISONS, MIXED_PRINT, MIXED_SAVE, MeansTable,
    Multinomial, NOMREG_BASES, NOMREG_MODELS, NOMREG_PRINT, NOMREG_STEP_METHODS, NOMREG_STEP_RULES,
    Ordinal, PLUM_LINKS, PLUM_PRINT, PROBABILITY_SAVE, RANDOM_COVARIANCES, REPEATED_COVARIANCES,
    SAMPLE_RESIDUALS, SAMPLE_STATISTICS, SAMPLE_TESTS, STAGE_ESTIMATORS, STAGE_RATES,
    SampleDescriptives, SampleOptions, SamplePlan, SampleTables, Stage, Terms,
};
use crate::app::{checkboxes, choice, field, file_dialog, target_list, target_one};
use crate::model_dialogs::GLM_ADJUSTMENTS;
use crate::model_dialogs_view::name_checkboxes;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, TextEdit, Ui};
use std::path::{Path, PathBuf};

pub fn body(ui: &mut Ui, d: &mut Advanced, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    match d {
        Advanced::Ordinal(d) => ordinal(ui, d, picked),
        Advanced::Multinomial(d) => multinomial(ui, d, picked),
        Advanced::Mixed(d) => mixed(ui, d, picked),
        Advanced::Genlin(d) => crate::genlin_dialog_view::genlin(ui, d, picked),
        Advanced::Plan(d) => plan(ui, d, picked, folder),
        Advanced::Descriptives(d) => sample_descriptives(ui, d, picked, folder),
        Advanced::Tables(d) => sample_tables(ui, d, picked, folder),
    }
}

/// Checkboxes of the model's variables, Main effects and Interaction to make terms of the
/// checked ones, and the terms, each removed by a click.
pub(crate) fn terms_editor(ui: &mut Ui, id: &str, model: &[String], t: &mut Terms) {
    ui.push_id(id, |ui| {
        name_checkboxes(ui, model, &mut t.building);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!t.building.is_empty(), egui::Button::new("Main effects"))
                .clicked()
            {
                t.add(model, false);
            }
            if ui
                .add_enabled(t.building.len() >= 2, egui::Button::new("Interaction"))
                .clicked()
            {
                t.add(model, true);
            }
        });
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(260.0);
            if t.terms.is_empty() {
                ui.weak("empty");
            }
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for (i, term) in t.terms.iter().enumerate() {
                    if ui
                        .button(format!("{}  \u{d7}", term.join("*")))
                        .on_hover_text("Remove")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                t.terms.remove(i);
            }
        });
    });
}

/// Tables of estimated marginal means, each of crossed factors (none for the overall mean) with
/// an optional comparison of one factor's levels, and the values the covariates are held at.
pub(crate) fn means_editor(
    ui: &mut Ui,
    factors: &[String],
    covariates: &[String],
    tables: &mut Vec<MeansTable>,
    at: &mut Vec<(String, String)>,
    comparisons: &[(&str, &str)],
    adjustments: &[(&str, &str)],
) {
    let mut remove = None;
    for (i, t) in tables.iter_mut().enumerate() {
        ui.push_id(("means", i), |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Table {}", i + 1));
                if t.factors.is_empty() {
                    ui.weak("overall mean");
                }
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
            name_checkboxes(ui, factors, &mut t.factors);
            if !t.factors.contains(&t.compare) {
                t.compare.clear();
            }
            ui.horizontal(|ui| {
                ui.label("Compare the levels of");
                let shown = match t.compare.as_str() {
                    "" => "no factor".to_string(),
                    c => c.to_string(),
                };
                ComboBox::new("compare", "")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut t.compare, String::new(), "no factor");
                        for f in factors.iter().filter(|f| t.factors.contains(f)) {
                            ui.selectable_value(&mut t.compare, f.clone(), f.as_str());
                        }
                    });
            });
            ui.add_enabled_ui(!t.compare.is_empty(), |ui| {
                choice(ui, "Comparison", comparisons, &mut t.contrast);
                choice(ui, "Adjustment", adjustments, &mut t.adjustment);
            });
        });
    }
    if let Some(i) = remove {
        tables.remove(i);
    }
    if ui.small_button("Add table").clicked() {
        tables.push(MeansTable::default());
    }
    if !covariates.is_empty() {
        ui.label("Covariates held at (empty: the mean)");
        for c in covariates {
            let i = match at.iter().position(|(n, _)| n == c) {
                Some(i) => i,
                None => {
                    at.push((c.clone(), String::new()));
                    at.len() - 1
                }
            };
            ui.push_id(("at", c), |ui| field(ui, c, &mut at[i].1, 60.0));
        }
    }
}

/// Custom tests, each a label and rows of coefficients typed one row per line.
fn tests_editor(ui: &mut Ui, tests: &mut Vec<ContrastTest>) {
    ui.weak("One row per line: an effect, then a coefficient for each of its parameters");
    let mut remove = None;
    for (i, t) in tests.iter_mut().enumerate() {
        ui.push_id(("test", i), |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Test {}", i + 1));
                ui.label("Label");
                ui.add(TextEdit::singleline(&mut t.label).desired_width(160.0));
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
            ui.add(
                TextEdit::multiline(&mut t.rows)
                    .desired_rows(2)
                    .desired_width(320.0)
                    .font(egui::TextStyle::Monospace),
            );
        });
    }
    if let Some(i) = remove {
        tests.remove(i);
    }
    if ui.small_button("Add test").clicked() {
        tests.push(ContrastTest::default());
    }
}

/// The confidence level, maximum iterations and maximum step-halving.
fn iteration_fields(
    ui: &mut Ui,
    ci_level: &mut String,
    iterations: &mut String,
    halvings: &mut String,
) {
    field(ui, "Confidence interval %", ci_level, 50.0);
    field(ui, "Maximum iterations", iterations, 50.0);
    field(ui, "Maximum step-halving", halvings, 50.0);
}

fn ordinal(ui: &mut Ui, d: &mut Ordinal, picked: &mut Vec<String>) {
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    choice(ui, "Link", &PLUM_LINKS, &mut d.link);
    ui.add_space(6.0);
    let model = d.model();
    egui::CollapsingHeader::new("Location").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom_location, false, "Main effects");
            ui.radio_value(&mut d.custom_location, true, "Custom");
        });
        ui.add_enabled_ui(d.custom_location, |ui| {
            terms_editor(ui, "location", &model, &mut d.location);
        });
    });
    egui::CollapsingHeader::new("Scale").show(ui, |ui| {
        ui.checkbox(&mut d.scale, "Model the scale by these terms");
        ui.add_enabled_ui(d.scale, |ui| {
            terms_editor(ui, "scale", &model, &mut d.scale_terms);
        });
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        checkboxes(ui, &PLUM_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        checkboxes(ui, &PROBABILITY_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        iteration_fields(ui, &mut d.ci_level, &mut d.iterations, &mut d.halvings);
        field(ui, "Delta added to empty cells", &mut d.delta, 50.0);
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        ui.weak("INTERCEPT stands for the thresholds");
        tests_editor(ui, &mut d.tests);
    });
}

fn multinomial(ui: &mut Ui, d: &mut Multinomial, picked: &mut Vec<String>) {
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    ui.horizontal(|ui| {
        ui.label("Reference category");
        for (i, label) in NOMREG_BASES.iter().enumerate() {
            ui.radio_value(&mut d.base, i, *label);
        }
        small_field(ui, d.base == 2, &mut d.base_value);
    });
    ui.horizontal(|ui| {
        ui.label("Category order");
        ui.radio_value(&mut d.descending, false, "Ascending");
        ui.radio_value(&mut d.descending, true, "Descending");
    });
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, label) in NOMREG_MODELS.iter().enumerate() {
                ui.radio_value(&mut d.model, i, *label);
            }
        });
        ui.add_enabled_ui(d.model == 2, |ui| {
            let model = d.model_variables();
            terms_editor(ui, "model", &model, &mut d.terms);
            ui.add_space(4.0);
            ui.label("Stepwise terms, entered or removed by the method");
            choice(ui, "Method", &NOMREG_STEP_METHODS, &mut d.step_method);
            terms_editor(ui, "stepwise", &model, &mut d.step_terms);
        });
        ui.checkbox(&mut d.intercept, "Include intercept in the model");
    });
    egui::CollapsingHeader::new("Stepping").show(ui, |ui| {
        ui.add_enabled_ui(d.model == 2 && !d.step_terms.terms.is_empty(), |ui| {
            choice(ui, "Hierarchy rule", &NOMREG_STEP_RULES, &mut d.step_rule);
            field(ui, "Least effects in the model", &mut d.min_effects, 50.0);
            field(ui, "Most effects (empty: all)", &mut d.max_effects, 50.0);
            field(ui, "Probability for entry", &mut d.entry, 50.0);
            field(ui, "Probability for removal", &mut d.removal, 50.0);
            ui.horizontal(|ui| {
                ui.label("Entry test");
                ui.radio_value(&mut d.score_entry, false, "Likelihood ratio");
                ui.radio_value(&mut d.score_entry, true, "Score");
            });
            ui.horizontal(|ui| {
                ui.label("Removal test");
                ui.radio_value(&mut d.wald_removal, false, "Likelihood ratio");
                ui.radio_value(&mut d.wald_removal, true, "Wald");
            });
        });
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &NOMREG_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        checkboxes(ui, &PROBABILITY_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        iteration_fields(ui, &mut d.ci_level, &mut d.iterations, &mut d.halvings);
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
}

fn mixed(ui: &mut Ui, d: &mut LinearMixed, picked: &mut Vec<String>) {
    target_list(ui, "Subjects", &mut d.subjects, picked);
    d.forget_old_subjects();
    target_list(ui, "Repeated", &mut d.repeated, picked);
    ui.add_enabled_ui(!d.repeated.is_empty(), |ui| {
        choice(
            ui,
            "Repeated covariance type",
            &REPEATED_COVARIANCES,
            &mut d.repeated_covariance,
        );
    });
    ui.add_space(6.0);
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    forget_old_factors(&d.factors, &mut d.means);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    let model = d.model();
    egui::CollapsingHeader::new("Fixed").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom_fixed, false, "Main effects");
            ui.radio_value(&mut d.custom_fixed, true, "Custom");
        });
        ui.add_enabled_ui(d.custom_fixed, |ui| {
            terms_editor(ui, "fixed", &model, &mut d.fixed);
        });
        ui.checkbox(&mut d.intercept, "Include intercept");
    });
    egui::CollapsingHeader::new("Random").show(ui, |ui| {
        ui.checkbox(&mut d.random, "Random effects");
        ui.add_enabled_ui(d.random, |ui| {
            choice(
                ui,
                "Covariance type",
                &RANDOM_COVARIANCES,
                &mut d.random_covariance,
            );
            ui.checkbox(&mut d.random_intercept, "Include intercept");
            terms_editor(ui, "random", &model, &mut d.random_terms);
            ui.label("Subject combination");
            if d.subjects.is_empty() {
                ui.weak("no subjects");
            }
            ui.push_id("random subjects", |ui| {
                name_checkboxes(ui, &d.subjects, &mut d.random_subjects);
            });
        });
    });
    egui::CollapsingHeader::new("Estimation").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Method");
            ui.radio_value(&mut d.ml, false, "Restricted maximum likelihood");
            ui.radio_value(&mut d.ml, true, "Maximum likelihood");
        });
        iteration_fields(ui, &mut d.ci_level, &mut d.iterations, &mut d.halvings);
        ui.horizontal(|ui| {
            ui.label("Degrees of freedom");
            ui.radio_value(&mut d.residual_df, false, "Satterthwaite");
            ui.radio_value(&mut d.residual_df, true, "Residual");
        });
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &MIXED_PRINT, &mut d.print);
    });
    egui::CollapsingHeader::new("EM means").show(ui, |ui| {
        means_editor(
            ui,
            &d.factors,
            &d.covariates,
            &mut d.means,
            &mut d.means_at,
            &MIXED_COMPARISONS,
            &GLM_ADJUSTMENTS,
        );
    });
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        tests_editor(ui, &mut d.tests);
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        checkboxes(ui, &MIXED_SAVE, &mut d.save);
    });
}

/// The plan file: a field and Browse, to write a new plan (`save`) or pick one.
fn plan_path(ui: &mut Ui, path: &mut String, save: bool, folder: &mut Option<PathBuf>) {
    ui.horizontal(|ui| {
        ui.strong("Plan file");
        if ui.button("Browse...").clicked() {
            let dialog = file_dialog(folder).add_filter("Plan", &["json"]);
            let chosen = if save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(mut chosen) = chosen {
                // the pickers of the other dialogs list .json files only
                if save && chosen.extension().is_none() {
                    chosen.set_extension("json");
                }
                *folder = chosen.parent().map(Path::to_path_buf);
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
    ui.add(TextEdit::singleline(path).desired_width(360.0));
    ui.add_space(6.0);
}

fn plan(ui: &mut Ui, d: &mut SamplePlan, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    plan_path(ui, &mut d.path, true, folder);
    target_one(ui, "Sample weight", &mut d.weight, picked);
    ui.label("Design effects compare with simple random sampling");
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.srs_with_replacement, false, "Without replacement");
        ui.radio_value(&mut d.srs_with_replacement, true, "With replacement");
    });
    let mut remove = None;
    let only = d.stages.len() == 1;
    for (i, stage) in d.stages.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("stage", i), |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Stage {}", i + 1));
                ui.label("Label");
                ui.add(TextEdit::singleline(&mut stage.label).desired_width(140.0));
                if ui
                    .add_enabled(!only, egui::Button::new("\u{d7}").small())
                    .on_hover_text("Remove")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
            target_list(ui, "Strata", &mut stage.strata, picked);
            target_list(ui, "Clusters", &mut stage.clusters, picked);
            choice(ui, "Estimation", &STAGE_ESTIMATORS, &mut stage.estimator);
            if stage.estimator == 1 {
                choice(ui, "Given by", &STAGE_RATES, &mut stage.rate);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut stage.rate_by_variable, false, "Value");
                    small_field(ui, !stage.rate_by_variable, &mut stage.rate_value);
                    ui.radio_value(&mut stage.rate_by_variable, true, "Variable");
                });
                ui.add_enabled_ui(stage.rate_by_variable, |ui| {
                    target_one(ui, "Read from", &mut stage.rate_variable, picked);
                });
            }
        });
    }
    if let Some(i) = remove {
        d.stages.remove(i);
    }
    if ui
        .add_enabled(d.can_add_stage(), egui::Button::new("Add stage").small())
        .on_disabled_hover_text("Up to three stages, each after one drawn without replacement")
        .clicked()
    {
        d.stages.push(Stage::default());
    }
}

/// Statistics with `more` of the analysis, subpopulations and missing values.
fn sample_options(
    ui: &mut Ui,
    o: &mut SampleOptions,
    picked: &mut Vec<String>,
    more: impl FnOnce(&mut Ui),
) {
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &SAMPLE_STATISTICS, &mut o.statistics);
        ui.horizontal(|ui| {
            ui.checkbox(&mut o.ci, "Confidence interval, level %");
            small_field(ui, o.ci, &mut o.ci_level);
        });
        more(ui);
    });
    egui::CollapsingHeader::new("Subpopulations").show(ui, |ui| {
        target_list(ui, "Subpopulations", &mut o.subpopulations, picked);
        ui.checkbox(&mut o.separate, "A table for each subpopulation variable");
    });
    egui::CollapsingHeader::new("Missing values").show(ui, |ui| {
        ui.checkbox(
            &mut o.listwise,
            "Use only the cases valid on every variable",
        );
        ui.checkbox(
            &mut o.include,
            "User-missing values of categories are valid",
        );
    });
}

fn sample_descriptives(
    ui: &mut Ui,
    d: &mut SampleDescriptives,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    plan_path(ui, &mut d.plan, false, folder);
    target_list(ui, "Measures", &mut d.summary, picked);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.mean, "Mean");
        ui.checkbox(&mut d.sum, "Sum");
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Ratio").show(ui, |ui| {
        target_list(ui, "Numerators", &mut d.numerators, picked);
        target_list(ui, "Denominators", &mut d.denominators, picked);
    });
    let population_size = &mut d.population_size;
    sample_options(ui, &mut d.options, picked, |ui| {
        ui.checkbox(population_size, "Population size");
    });
}

fn sample_tables(
    ui: &mut Ui,
    d: &mut SampleTables,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    plan_path(ui, &mut d.plan, false, folder);
    if d.crosstabs {
        target_list(ui, "Rows", &mut d.variables, picked);
        target_one(ui, "Column", &mut d.column, picked);
    } else {
        target_list(ui, "Variables", &mut d.variables, picked);
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Cells").show(ui, |ui| {
        let options = d.cell_options();
        checkboxes(ui, options, &mut d.cells[..options.len()]);
    });
    let crosstabs = d.crosstabs;
    let (residuals, cumulative) = (&mut d.residuals, &mut d.cumulative);
    sample_options(ui, &mut d.options, picked, |ui| {
        if !crosstabs {
            ui.checkbox(cumulative, "Cumulative values");
            return;
        }
        // left enabled, so a tick can be taken back when the message asks for table percents
        ui.weak("With table percents");
        checkboxes(ui, &SAMPLE_RESIDUALS, residuals);
    });
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        if d.crosstabs {
            checkboxes(ui, &SAMPLE_TESTS, &mut d.tests);
        } else {
            ui.checkbox(&mut d.homogeneity, "Equal shares of the categories");
        }
    });
}
