//! Drawing of the Linear Regression, Reliability Analysis, Nonparametric Tests and Partial
//! Correlations dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::stats_dialogs::{
    ICC_MODELS, NparKind, NparTests, PartialCorr, REGRESSION_METHODS, REGRESSION_MISSING,
    REGRESSION_SAVE, REGRESSION_SAVE_GROUPS, REGRESSION_STATISTICS, RELIABILITY_ANOVA,
    RELIABILITY_DESCRIPTIVES, RELIABILITY_INTER_ITEM, RELIABILITY_MODELS, RELIABILITY_SUMMARIES,
    Regression, Reliability,
};
use eframe::egui::{self, TextEdit, Ui};

/// A text field that takes part only when `on`.
fn small_field(ui: &mut Ui, on: bool, text: &mut String) {
    ui.add_enabled(on, TextEdit::singleline(text).desired_width(50.0));
}

pub fn regression(ui: &mut Ui, d: &mut Regression, picked: &mut Vec<String>) {
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let count = d.blocks.len();
        ui.label(format!("Block {} of {count}", d.block + 1));
        if ui
            .add_enabled(d.block > 0, egui::Button::new("Previous").small())
            .clicked()
        {
            d.block -= 1;
        }
        let can_add = !d.blocks[d.block].variables.is_empty();
        if ui
            .add_enabled(
                d.block + 1 < count || can_add,
                egui::Button::new("Next").small(),
            )
            .clicked()
        {
            d.block += 1;
            if d.block == count {
                d.blocks.push(Default::default());
            }
        }
    });
    let block = &mut d.blocks[d.block];
    target_list(ui, "Independent variables", &mut block.variables, picked);
    choice(ui, "Method", &REGRESSION_METHODS, &mut block.method);
    ui.add_space(6.0);
    ui.strong("Statistics");
    checkboxes(ui, &REGRESSION_STATISTICS, &mut d.statistics);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.ci, "Confidence intervals, level %");
        small_field(ui, d.ci, &mut d.ci_level);
    });
    ui.checkbox(&mut d.descriptives, "Descriptives");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for (title, range) in REGRESSION_SAVE_GROUPS {
            ui.label(title);
            checkboxes(ui, &REGRESSION_SAVE[range.clone()], &mut d.save[range]);
        }
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stepping criteria");
            let mut use_f = d.use_f;
            ui.radio_value(&mut use_f, false, "Probability of F");
            ui.radio_value(&mut use_f, true, "F value");
            if use_f != d.use_f {
                d.set_use_f(use_f);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Entry");
            small_field(ui, true, &mut d.entry);
            ui.label("Removal");
            small_field(ui, true, &mut d.removal);
        });
        ui.checkbox(&mut d.constant, "Include the constant in the equation");
        ui.label("Missing values");
        for (i, (_, label)) in REGRESSION_MISSING.iter().enumerate() {
            ui.radio_value(&mut d.missing, i, *label);
        }
    });
}

pub fn reliability(ui: &mut Ui, d: &mut Reliability, picked: &mut Vec<String>) {
    target_list(ui, "Items", &mut d.items, picked);
    choice(ui, "Model", &RELIABILITY_MODELS, &mut d.model);
    field(ui, "Scale label", &mut d.label, 200.0);
    ui.add_space(6.0);
    ui.strong("Descriptives for");
    checkboxes(ui, &RELIABILITY_DESCRIPTIVES, &mut d.descriptives);
    ui.strong("Inter-item");
    checkboxes(ui, &RELIABILITY_INTER_ITEM, &mut d.inter_item);
    ui.strong("Summaries");
    checkboxes(ui, &RELIABILITY_SUMMARIES, &mut d.summaries);
    ui.add_space(6.0);
    choice(ui, "ANOVA table", &RELIABILITY_ANOVA, &mut d.anova);
    ui.checkbox(&mut d.hotelling, "Hotelling's T-squared");
    ui.checkbox(&mut d.tukey, "Tukey's test of additivity");
    ui.add_space(6.0);
    ui.checkbox(&mut d.icc, "Intraclass correlation coefficient");
    ui.add_enabled_ui(d.icc, |ui| {
        ui.push_id("icc", |ui| {
            choice(ui, "Model", &ICC_MODELS, &mut d.icc_model);
        });
        ui.add_enabled_ui(ICC_MODELS[d.icc_model].0 != "ONEWAY", |ui| {
            ui.horizontal(|ui| {
                ui.label("Type");
                ui.radio_value(&mut d.icc_absolute, false, "Consistency");
                ui.radio_value(&mut d.icc_absolute, true, "Absolute agreement");
            });
        });
        field(ui, "Confidence interval %", &mut d.icc_level, 50.0);
        field(ui, "Test value", &mut d.icc_test_value, 50.0);
    });
}

pub fn npar_tests(ui: &mut Ui, d: &mut NparTests, picked: &mut Vec<String>) {
    if d.kind == NparKind::TwoRelated {
        ui.strong("Pairs");
        let mut remove = None;
        for (i, (a, b)) in d.pairs.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{a}  with  {b}"));
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.pairs.remove(i);
        }
        if ui
            .add_enabled(picked.len() >= 2, egui::Button::new("Add pair"))
            .clicked()
        {
            let first = picked.remove(0);
            d.pairs.push((first, picked.remove(0)));
        }
    } else {
        target_list(ui, "Test variables", &mut d.variables, picked);
    }
    ui.add_space(6.0);
    match d.kind {
        NparKind::ChiSquare => {
            ui.strong("Categories");
            ui.radio_value(&mut d.range, false, "Every value in the data");
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.range, true, "Whole numbers from");
                small_field(ui, d.range, &mut d.low);
                ui.label("through");
                small_field(ui, d.range, &mut d.high);
            });
            ui.strong("Expected values");
            ui.radio_value(&mut d.expected_given, false, "All categories equal");
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.expected_given, true, "Values");
                ui.add_enabled(
                    d.expected_given,
                    TextEdit::singleline(&mut d.expected).desired_width(180.0),
                );
            });
        }
        NparKind::Binomial => {
            field(ui, "Test proportion", &mut d.proportion, 50.0);
            ui.strong("Two categories");
            ui.radio_value(&mut d.cut, false, "The two values in the data");
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.cut, true, "Up to the cut point, and above it");
                small_field(ui, d.cut, &mut d.cut_value);
            });
        }
        NparKind::TwoIndependent | NparKind::KIndependent => {
            target_one(ui, "Grouping variable", &mut d.grouping, picked);
            let two = d.kind == NparKind::TwoIndependent;
            ui.horizontal(|ui| {
                ui.label(if two { "Group 1" } else { "Lowest group" });
                small_field(ui, true, &mut d.low);
                ui.label(if two { "Group 2" } else { "Highest group" });
                small_field(ui, true, &mut d.high);
            });
        }
        _ => {}
    }
    let tests = d.kind.tests();
    if !tests.is_empty() {
        ui.add_space(6.0);
        ui.strong(match d.kind {
            NparKind::Runs => "Cut points",
            NparKind::OneSampleKs => "Test distributions",
            _ => "Tests",
        });
        checkboxes(ui, tests, &mut d.tests);
    }
    if d.kind == NparKind::Runs {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.cut, "Custom");
            small_field(ui, d.cut, &mut d.cut_value);
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.descriptives, "Descriptives");
            ui.checkbox(&mut d.quartiles, "Quartiles");
        });
        ui.checkbox(
            &mut d.listwise,
            "Leave out a case missing on any variable of any test",
        );
    });
}

pub fn partial_corr(ui: &mut Ui, d: &mut PartialCorr, picked: &mut Vec<String>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    target_list(ui, "Controlling for", &mut d.controls, picked);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.one_tailed, false, "Two-tailed test");
        ui.radio_value(&mut d.one_tailed, true, "One-tailed test");
    });
    ui.checkbox(
        &mut d.condensed,
        "Coefficients only, significant ones marked with stars",
    );
    ui.add_space(6.0);
    ui.strong("Statistics");
    ui.checkbox(&mut d.descriptives, "Means and standard deviations");
    ui.checkbox(&mut d.zero_order, "Zero-order correlations");
    ui.add_space(6.0);
    ui.strong("Missing values");
    ui.radio_value(
        &mut d.pairwise,
        false,
        "Leave out a case missing on any listed variable",
    );
    ui.radio_value(
        &mut d.pairwise,
        true,
        "Leave out a case only from the correlations where its value is missing",
    );
}
