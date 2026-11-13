//! Drawing of the Ordinal Regression, Multinomial Logistic Regression, Linear Mixed Models and
//! complex samples dialogs.

use crate::advanced_dialogs::{
    Advanced, LinearMixed, MIXED_SAVE, Multinomial, NOMREG_BASES, NOMREG_MODELS, NOMREG_PRINT,
    Ordinal, PLUM_LINKS, PLUM_PRINT, PROBABILITY_SAVE, RANDOM_COVARIANCES, REPEATED_COVARIANCES,
    SAMPLE_RESIDUALS, SAMPLE_STATISTICS, SAMPLE_TESTS, STAGE_ESTIMATORS, STAGE_RATES,
    SampleDescriptives, SampleOptions, SamplePlan, SampleTables, Stage, Terms,
};
use crate::app::{checkboxes, choice, field, file_dialog, target_list, target_one};
use crate::model_dialogs_view::name_checkboxes;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, TextEdit, Ui};
use std::path::{Path, PathBuf};

pub fn body(ui: &mut Ui, d: &mut Advanced, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    match d {
        Advanced::Ordinal(d) => ordinal(ui, d, picked),
        Advanced::Multinomial(d) => multinomial(ui, d, picked),
        Advanced::Mixed(d) => mixed(ui, d, picked),
        Advanced::Plan(d) => plan(ui, d, picked, folder),
        Advanced::Descriptives(d) => sample_descriptives(ui, d, picked, folder),
        Advanced::Tables(d) => sample_tables(ui, d, picked, folder),
    }
}

/// Checkboxes of the model's variables, Main effects and Interaction to make terms of the
/// checked ones, and the terms, each removed by a click.
fn terms_editor(ui: &mut Ui, id: &str, model: &[String], t: &mut Terms) {
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
        });
        ui.checkbox(&mut d.intercept, "Include intercept in the model");
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
        ui.checkbox(&mut d.solution, "Parameter estimates of the fixed effects");
        ui.checkbox(
            &mut d.covariance_tests,
            "Tests of the covariance parameters",
        );
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
    let (crosstabs, table_percent) = (d.crosstabs, d.cells[1]);
    let (residuals, cumulative) = (&mut d.residuals, &mut d.cumulative);
    sample_options(ui, &mut d.options, picked, |ui| {
        if !crosstabs {
            ui.checkbox(cumulative, "Cumulative values");
            return;
        }
        ui.add_enabled_ui(table_percent, |ui| {
            ui.weak("With table percents");
            checkboxes(ui, &SAMPLE_RESIDUALS, residuals);
        });
    });
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        if d.crosstabs {
            checkboxes(ui, &SAMPLE_TESTS, &mut d.tests);
        } else {
            ui.checkbox(&mut d.homogeneity, "Equal shares of the categories");
        }
    });
}
