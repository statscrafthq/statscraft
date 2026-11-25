//! Drawing of the Linear Regression, Reliability Analysis, Nonparametric Tests and Partial
//! Correlations dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::i18n::{t, t_args};
use crate::model_dialogs::SELECT_RELATIONS;
use crate::stats_dialogs::{
    Block, ICC_MODELS, NparKind, NparTests, PartialCorr, REGRESSION_METHODS, REGRESSION_MISSING,
    REGRESSION_SAVE, REGRESSION_SAVE_GROUPS, REGRESSION_STATISTICS, RELIABILITY_ANOVA,
    RELIABILITY_DESCRIPTIVES, RELIABILITY_INTER_ITEM, RELIABILITY_MODELS, RELIABILITY_SUMMARIES,
    Regression, Reliability,
};
use eframe::egui::{self, TextEdit, Ui};

/// A text field that takes part only when `on`.
pub(crate) fn small_field(ui: &mut Ui, on: bool, text: &mut String) {
    ui.add_enabled(on, TextEdit::singleline(text).desired_width(50.0));
}

/// Previous and Next through the blocks, with a new block after the last one that has
/// variables; returns the block shown.
pub(crate) fn blocks<'a>(ui: &mut Ui, blocks: &'a mut Vec<Block>, at: &mut usize) -> &'a mut Block {
    ui.horizontal(|ui| {
        let count = blocks.len();
        ui.label(t_args(
            "stats-stats-block-of",
            &[
                ("block", (*at + 1).to_string().into()),
                ("count", count.to_string().into()),
            ],
        ));
        if ui
            .add_enabled(
                *at > 0,
                egui::Button::new(t("stats-stats-previous")).small(),
            )
            .clicked()
        {
            *at -= 1;
        }
        let can_add = !blocks[*at].variables.is_empty();
        if ui
            .add_enabled(
                *at + 1 < count || can_add,
                egui::Button::new(t("stats-stats-next")).small(),
            )
            .clicked()
        {
            *at += 1;
            if *at == count {
                blocks.push(Default::default());
            }
        }
    });
    &mut blocks[*at]
}

pub fn regression(ui: &mut Ui, d: &mut Regression, picked: &mut Vec<String>) {
    target_one(
        ui,
        t("stats-stats-dependent-variable"),
        &mut d.dependent,
        picked,
    );
    ui.add_space(6.0);
    let block = blocks(ui, &mut d.blocks, &mut d.block);
    target_list(
        ui,
        t("stats-stats-independent-variables"),
        &mut block.variables,
        picked,
    );
    choice(
        ui,
        t("stats-stats-method"),
        &REGRESSION_METHODS,
        &mut block.method,
    );
    ui.add_space(6.0);
    target_one(
        ui,
        t("stats-stats-selection-variable"),
        &mut d.select,
        picked,
    );
    ui.add_enabled_ui(!d.select.is_empty(), |ui| {
        ui.horizontal(|ui| {
            choice(
                ui,
                t("stats-stats-rule-value"),
                &SELECT_RELATIONS,
                &mut d.select_relation,
            );
            small_field(ui, true, &mut d.select_value);
        });
    });
    ui.add_space(6.0);
    ui.strong(t("stats-stats-statistics"));
    checkboxes(ui, &REGRESSION_STATISTICS, &mut d.statistics);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.ci, t("stats-stats-ci-level"));
        small_field(ui, d.ci, &mut d.ci_level);
    });
    ui.checkbox(&mut d.descriptives, t("stats-stats-descriptives"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("button-save")).show(ui, |ui| {
        for (title, range) in REGRESSION_SAVE_GROUPS {
            ui.label(t(title));
            checkboxes(ui, &REGRESSION_SAVE[range.clone()], &mut d.save[range]);
        }
    });
    egui::CollapsingHeader::new(t("stats-stats-options")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(t("stats-stats-stepping-criteria"));
            let mut use_f = d.use_f;
            ui.radio_value(&mut use_f, false, t("stats-stats-probability-of-f"));
            ui.radio_value(&mut use_f, true, t("stats-stats-f-value"));
            if use_f != d.use_f {
                d.set_use_f(use_f);
            }
        });
        ui.horizontal(|ui| {
            ui.label(t("stats-stats-entry"));
            small_field(ui, true, &mut d.entry);
            ui.label(t("stats-stats-removal"));
            small_field(ui, true, &mut d.removal);
        });
        ui.checkbox(&mut d.constant, t("stats-stats-include-constant"));
        ui.label(t("stats-stats-missing-values"));
        for (i, (_, label)) in REGRESSION_MISSING.iter().enumerate() {
            ui.radio_value(&mut d.missing, i, t(label));
        }
    });
}

pub fn reliability(ui: &mut Ui, d: &mut Reliability, picked: &mut Vec<String>) {
    target_list(ui, t("stats-stats-items"), &mut d.items, picked);
    choice(
        ui,
        t("stats-stats-model"),
        &RELIABILITY_MODELS,
        &mut d.model,
    );
    field(ui, t("stats-stats-scale-label"), &mut d.label, 200.0);
    ui.add_space(6.0);
    ui.strong(t("stats-stats-descriptives-for"));
    checkboxes(ui, &RELIABILITY_DESCRIPTIVES, &mut d.descriptives);
    ui.strong(t("stats-stats-inter-item"));
    checkboxes(ui, &RELIABILITY_INTER_ITEM, &mut d.inter_item);
    ui.strong(t("stats-stats-summaries"));
    checkboxes(ui, &RELIABILITY_SUMMARIES, &mut d.summaries);
    ui.add_space(6.0);
    choice(
        ui,
        t("stats-stats-anova-table"),
        &RELIABILITY_ANOVA,
        &mut d.anova,
    );
    ui.checkbox(&mut d.hotelling, t("stats-stats-hotelling"));
    ui.checkbox(&mut d.tukey, t("stats-stats-tukey-additivity"));
    ui.add_space(6.0);
    ui.checkbox(&mut d.icc, t("stats-stats-icc"));
    ui.add_enabled_ui(d.icc, |ui| {
        ui.push_id("icc", |ui| {
            choice(ui, t("stats-stats-model"), &ICC_MODELS, &mut d.icc_model);
        });
        ui.add_enabled_ui(ICC_MODELS[d.icc_model].0 != "ONEWAY", |ui| {
            ui.horizontal(|ui| {
                ui.label(t("stats-stats-type"));
                ui.radio_value(&mut d.icc_absolute, false, t("stats-stats-consistency"));
                ui.radio_value(
                    &mut d.icc_absolute,
                    true,
                    t("stats-stats-absolute-agreement"),
                );
            });
        });
        field(
            ui,
            t("stats-stats-confidence-interval"),
            &mut d.icc_level,
            50.0,
        );
        field(
            ui,
            t("stats-stats-test-value-label"),
            &mut d.icc_test_value,
            50.0,
        );
    });
}

pub fn npar_tests(ui: &mut Ui, d: &mut NparTests, picked: &mut Vec<String>) {
    if d.kind == NparKind::TwoRelated {
        ui.strong(t("stats-stats-pairs"));
        let mut remove = None;
        for (i, (a, b)) in d.pairs.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(t_args(
                    "stats-stats-pair-with",
                    &[("first", a.as_str().into()), ("second", b.as_str().into())],
                ));
                if ui
                    .small_button("\u{d7}")
                    .on_hover_text(t("stats-stats-remove"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.pairs.remove(i);
        }
        if ui
            .add_enabled(
                picked.len() >= 2,
                egui::Button::new(t("stats-stats-add-pair-button")),
            )
            .clicked()
        {
            let first = picked.remove(0);
            d.pairs.push((first, picked.remove(0)));
        }
    } else {
        target_list(
            ui,
            t("stats-stats-test-variables-label"),
            &mut d.variables,
            picked,
        );
    }
    ui.add_space(6.0);
    match d.kind {
        NparKind::ChiSquare => {
            ui.strong(t("stats-stats-categories"));
            ui.radio_value(&mut d.range, false, t("stats-stats-every-value"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.range, true, t("stats-stats-whole-numbers-from"));
                small_field(ui, d.range, &mut d.low);
                ui.label(t("stats-stats-through"));
                small_field(ui, d.range, &mut d.high);
            });
            ui.strong(t("stats-stats-expected-values"));
            ui.radio_value(
                &mut d.expected_given,
                false,
                t("stats-stats-all-categories-equal"),
            );
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.expected_given, true, t("stats-stats-values"));
                ui.add_enabled(
                    d.expected_given,
                    TextEdit::singleline(&mut d.expected).desired_width(180.0),
                );
            });
        }
        NparKind::Binomial => {
            field(
                ui,
                t("stats-stats-test-proportion-label"),
                &mut d.proportion,
                50.0,
            );
            ui.strong(t("stats-stats-two-categories"));
            ui.radio_value(&mut d.cut, false, t("stats-stats-two-values"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.cut, true, t("stats-stats-cut-point-split"));
                small_field(ui, d.cut, &mut d.cut_value);
            });
        }
        NparKind::TwoIndependent | NparKind::KIndependent => {
            target_one(
                ui,
                t("stats-stats-grouping-variable"),
                &mut d.grouping,
                picked,
            );
            let two = d.kind == NparKind::TwoIndependent;
            ui.horizontal(|ui| {
                ui.label(if two {
                    t("stats-stats-group-1")
                } else {
                    t("stats-stats-lowest-group")
                });
                small_field(ui, true, &mut d.low);
                ui.label(if two {
                    t("stats-stats-group-2")
                } else {
                    t("stats-stats-highest-group")
                });
                small_field(ui, true, &mut d.high);
            });
        }
        _ => {}
    }
    let tests = d.kind.tests();
    if !tests.is_empty() {
        ui.add_space(6.0);
        ui.strong(match d.kind {
            NparKind::Runs => t("stats-stats-cut-points"),
            NparKind::OneSampleKs => t("stats-stats-test-distributions"),
            _ => t("stats-stats-tests"),
        });
        checkboxes(ui, tests, &mut d.tests);
    }
    if d.kind == NparKind::Runs {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.cut, t("stats-stats-custom"));
            small_field(ui, d.cut, &mut d.cut_value);
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-stats-options")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.descriptives, t("stats-stats-descriptives"));
            ui.checkbox(&mut d.quartiles, t("stats-stats-quartiles"));
        });
        ui.checkbox(&mut d.listwise, t("stats-stats-exclude-any-test"));
        ui.checkbox(&mut d.exact, t("stats-stats-exact"))
            .on_hover_text(t("stats-stats-exact-hint"));
    });
}

pub fn partial_corr(ui: &mut Ui, d: &mut PartialCorr, picked: &mut Vec<String>) {
    target_list(ui, t("stats-stats-variables"), &mut d.variables, picked);
    target_list(
        ui,
        t("stats-stats-controlling-for-label"),
        &mut d.controls,
        picked,
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.one_tailed, false, t("stats-stats-two-tailed"));
        ui.radio_value(&mut d.one_tailed, true, t("stats-stats-one-tailed"));
    });
    ui.checkbox(&mut d.condensed, t("stats-stats-condensed"));
    ui.add_space(6.0);
    ui.strong(t("stats-stats-statistics"));
    ui.checkbox(&mut d.descriptives, t("stats-stats-means-sds"));
    ui.checkbox(&mut d.zero_order, t("stats-stats-zero-order"));
    ui.add_space(6.0);
    ui.strong(t("stats-stats-missing-values"));
    ui.radio_value(&mut d.pairwise, false, t("stats-stats-exclude-listwise"));
    ui.radio_value(&mut d.pairwise, true, t("stats-stats-exclude-pairwise"));
}
