//! Drawing of the Generalized Linear Mixed Models dialog.

use crate::advanced_dialogs_view::terms_editor;
use crate::app::{choice, field, target_list, target_one};
use crate::glmm_dialog::{
    EmmeansBlock, GLMM_ADJUSTMENTS, GLMM_CONFIDENCES, GLMM_CONTRASTS, GLMM_COVARIANCES,
    GLMM_DF_METHODS, GLMM_DISTRIBUTIONS, GLMM_FORMS, GLMM_LINKS, GLMM_ORDERS, GLMM_RESIDUALS,
    GLMM_SAVES, GenlinMixed, RandomBlock,
};
use crate::model_dialogs_view::name_checkboxes;
use eframe::egui::{self, Ui};

/// None, a value or a field, with the field or the number to go with it.
fn form(ui: &mut Ui, label: &str, form: &mut usize, text: &mut String, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.label(label);
        for (i, name) in GLMM_FORMS.iter().enumerate() {
            ui.radio_value(form, i, *name);
        }
    });
    match *form {
        1 => field(ui, "Value", text, 60.0),
        2 => target_one(ui, label, text, picked),
        _ => {}
    }
}

pub fn glmm(ui: &mut Ui, d: &mut GenlinMixed, picked: &mut Vec<String>) {
    target_list(ui, "Subjects", &mut d.subjects, picked);
    d.forget_old_subjects();
    target_list(ui, "Repeated measures fields", &mut d.repeated, picked);
    ui.weak("They order the cases of a subject; a model needs them or a random block");
    if !d.repeated.is_empty() {
        choice(ui, "Residual covariance", &GLMM_RESIDUALS, &mut d.residual);
    }
    ui.add_space(6.0);
    target_one(ui, "Target", &mut d.target, picked);
    target_list(ui, "Fixed effect fields", &mut d.inputs, picked);
    ui.weak("A scale field enters as a covariate, any other as a factor");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Target").show(ui, |ui| {
        choice(ui, "Distribution", &GLMM_DISTRIBUTIONS, &mut d.distribution);
        ui.horizontal_wrapped(|ui| {
            ui.label("Link");
            for (i, (_, label)) in GLMM_LINKS.iter().enumerate() {
                let radio = egui::RadioButton::new(d.link == i, *label);
                if ui.add_enabled(d.link_allowed(i), radio).clicked() {
                    d.link = i;
                }
            }
        });
        if !d.link_allowed(d.link) {
            ui.weak("This link does not go with the distribution");
        }
        if GLMM_LINKS[d.link].0 == "POWER" {
            field(ui, "Exponent of the power link", &mut d.power, 50.0);
        }
        form(ui, "Trials", &mut d.trials_form, &mut d.trials, picked);
        form(ui, "Offset", &mut d.offset_form, &mut d.offset, picked);
        target_one(ui, "Analysis weight", &mut d.weight, picked);
        field(
            ui,
            "Reference value of a binomial or nominal target",
            &mut d.reference,
            60.0,
        );
        ui.weak(
            "Empty: the last value in the order below; a binomial model is for the other value",
        );
    });
    egui::CollapsingHeader::new("Fixed effects").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom_fixed, false, "Main effects");
            ui.radio_value(&mut d.custom_fixed, true, "Custom");
        });
        let inputs = d.inputs.clone();
        ui.add_enabled_ui(d.custom_fixed, |ui| {
            terms_editor(ui, "fixed", &inputs, &mut d.fixed);
        });
        ui.checkbox(&mut d.intercept, "Include intercept");
    });
    egui::CollapsingHeader::new("Random effects")
        .default_open(true)
        .show(ui, |ui| {
            let inputs = d.inputs.clone();
            let mut remove = None;
            for (i, b) in d.random.iter_mut().enumerate() {
                ui.push_id(("random block", i), |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Block {}", i + 1));
                        if ui
                            .button("\u{d7}")
                            .on_hover_text("Remove the block")
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                    ui.checkbox(&mut b.intercept, "Include intercept");
                    terms_editor(ui, "terms", &inputs, &mut b.terms);
                    choice(ui, "Covariance type", &GLMM_COVARIANCES, &mut b.covariance);
                    ui.label("Subjects");
                    if d.subjects.is_empty() {
                        ui.weak("no subjects");
                    }
                    name_checkboxes(ui, &d.subjects, &mut b.subjects);
                    ui.checkbox(&mut b.solution, "Predicted random effects of every subject");
                });
                ui.separator();
            }
            if let Some(i) = remove {
                d.random.remove(i);
            }
            if ui.small_button("Add block").clicked() {
                d.random.push(RandomBlock::default());
            }
        });
    egui::CollapsingHeader::new("Estimation").show(ui, |ui| {
        choice(ui, "Order of the target values", &GLMM_ORDERS, &mut d.order);
        choice(ui, "Degrees of freedom", &GLMM_DF_METHODS, &mut d.df_method);
        field(ui, "Confidence interval %", &mut d.ci_level, 50.0);
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        field(
            ui,
            "Convergence of the covariance parameters",
            &mut d.pconverge,
            70.0,
        );
        ui.checkbox(&mut d.robust, "Robust standard errors");
    });
    egui::CollapsingHeader::new("Estimated means").show(ui, |ui| {
        let inputs = d.inputs.clone();
        let mut remove = None;
        for (i, e) in d.emmeans.iter_mut().enumerate() {
            ui.push_id(("means block", i), |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("Means {}", i + 1));
                    if ui
                        .button("\u{d7}")
                        .on_hover_text("Remove the block")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
                ui.label("Factors (none: the overall mean)");
                name_checkboxes(ui, &inputs, &mut e.tables);
                if !e.tables.contains(&e.compare) {
                    e.compare.clear();
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Compare the levels of");
                    ui.radio_value(&mut e.compare, String::new(), "no factor");
                    for f in inputs.iter().filter(|f| e.tables.contains(f)) {
                        ui.radio_value(&mut e.compare, f.clone(), f.as_str());
                    }
                });
                if !e.compare.is_empty() {
                    choice(ui, "Contrast", &GLMM_CONTRASTS, &mut e.contrast);
                }
            });
            ui.separator();
        }
        if let Some(i) = remove {
            d.emmeans.remove(i);
        }
        if ui.small_button("Add means").clicked() {
            d.emmeans.push(EmmeansBlock::default());
        }
        field(ui, "Covariates held at", &mut d.control, 120.0);
        ui.weak("name=value, separated by spaces; empty: at their mean");
        ui.checkbox(&mut d.transformed, "Means on the scale of the link");
        choice(
            ui,
            "Adjustment of the contrasts",
            &GLMM_ADJUSTMENTS,
            &mut d.adjust,
        );
    });
    egui::CollapsingHeader::new("Save and export").show(ui, |ui| {
        ui.label("Save to the data (name or root; empty: the default)");
        for (i, (_, label)) in GLMM_SAVES.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
                ui.add_enabled(
                    d.save[i],
                    egui::TextEdit::singleline(&mut d.save_names[i]).desired_width(100.0),
                );
            });
        }
        ui.weak("Probabilities and confidence need a categorical target");
        choice(
            ui,
            "Confidence measures",
            &GLMM_CONFIDENCES,
            &mut d.confidence,
        );
        field(ui, "Most categories saved", &mut d.max_categories, 50.0);
        field(ui, "Model file (JSON)", &mut d.outfile, 220.0);
        ui.weak("Empty: no file");
    });
}
