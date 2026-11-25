//! Drawing of the One Sample, Independent Samples and Related Samples dialogs of Nonparametric
//! Tests.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::i18n::t;
use crate::nptests_dialog::{
    BINOMIAL_INTERVALS, INDEPENDENT_K, INDEPENDENT_TWO, KS_DISTRIBUTIONS, NP_COMPARE, NpKind,
    Nptests, RELATED_K, RELATED_TWO, RUNS_CUTS,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, TextEdit, Ui};

/// A checkbox with a text field after it that takes part only when ticked.
fn checked_text(ui: &mut Ui, on: &mut bool, label: &str, text: &mut String, enabled: bool) {
    ui.horizontal(|ui| {
        ui.checkbox(on, label);
        ui.add_enabled(
            enabled && *on,
            TextEdit::singleline(text).desired_width(140.0),
        );
    });
}

/// A label and a small field that takes part only when `on`.
fn labelled(ui: &mut Ui, label: &str, on: bool, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        small_field(ui, on, text);
    });
}

pub fn nptests(ui: &mut Ui, d: &mut Nptests, picked: &mut Vec<String>) {
    target_list(ui, t("stats-nptests-test-fields"), &mut d.fields, picked);
    if d.kind == NpKind::Independent {
        target_one(ui, t("stats-nptests-groups"), &mut d.group, picked);
    }
    ui.add_space(6.0);
    ui.radio_value(
        &mut d.customize,
        false,
        t("stats-nptests-choose-automatically"),
    );
    ui.radio_value(&mut d.customize, true, t("stats-nptests-customize-tests"));
    ui.add_enabled_ui(d.customize, |ui| {
        egui::CollapsingHeader::new(t("stats-nptests-tests"))
            .default_open(true)
            .show(ui, |ui| match d.kind {
                NpKind::OneSample => one_sample(ui, d),
                NpKind::Independent => independent(ui, d),
                NpKind::Related => related(ui, d),
            });
    });
    egui::CollapsingHeader::new(t("stats-nptests-options")).show(ui, |ui| {
        field(
            ui,
            t("stats-nptests-significance-level"),
            &mut d.alpha,
            50.0,
        );
        field(
            ui,
            t("stats-nptests-confidence-level"),
            &mut d.ci_level,
            50.0,
        );
        ui.radio_value(
            &mut d.listwise,
            false,
            t("stats-nptests-exclude-test-by-test"),
        );
        ui.radio_value(&mut d.listwise, true, t("stats-nptests-exclude-listwise"));
        ui.checkbox(&mut d.include, t("stats-nptests-user-missing-valid"));
    });
}

fn one_sample(ui: &mut Ui, d: &mut Nptests) {
    ui.checkbox(&mut d.binomial, t("stats-nptests-binomial-test"));
    ui.add_enabled_ui(d.binomial, |ui| {
        ui.indent("binomial", |ui| {
            labelled(
                ui,
                t("stats-nptests-test-proportion"),
                true,
                &mut d.proportion,
            );
            checked_text(
                ui,
                &mut d.success_list,
                t("stats-nptests-success-values"),
                &mut d.success,
                true,
            );
            ui.weak(t("stats-nptests-otherwise-first-value"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.cut_value, t("stats-nptests-cut-continuous-at"));
                small_field(ui, d.cut_value, &mut d.cut);
            });
            ui.weak(t("stats-nptests-otherwise-halfway"));
            ui.label(t("stats-nptests-confidence-intervals"));
            checkboxes(ui, &BINOMIAL_INTERVALS, &mut d.intervals);
        });
    });
    ui.checkbox(&mut d.chisquare, t("stats-nptests-chi-square-test"));
    ui.add_enabled_ui(d.chisquare, |ui| {
        ui.indent("chisquare", |ui| {
            ui.radio_value(
                &mut d.expected_custom,
                false,
                t("stats-nptests-equal-expected"),
            );
            ui.radio_value(
                &mut d.expected_custom,
                true,
                t("stats-nptests-expected-of-categories"),
            );
            ui.add_enabled_ui(d.expected_custom, |ui| {
                field(ui, t("stats-nptests-categories"), &mut d.categories, 140.0);
                field(
                    ui,
                    t("stats-nptests-frequencies"),
                    &mut d.frequencies,
                    140.0,
                );
            });
        });
    });
    ui.checkbox(&mut d.ks, t("stats-nptests-ks-test"));
    ui.add_enabled_ui(d.ks, |ui| {
        ui.indent("ks", |ui| ks(ui, d));
    });
    ui.checkbox(&mut d.runs, t("stats-nptests-runs-test"));
    ui.add_enabled_ui(d.runs, |ui| {
        ui.indent("runs", |ui| {
            checked_text(
                ui,
                &mut d.runs_list,
                t("stats-nptests-first-group-categorical"),
                &mut d.runs_values,
                true,
            );
            ui.weak(t("stats-nptests-otherwise-two-values"));
            ui.horizontal(|ui| {
                choice(
                    ui,
                    t("stats-nptests-cut-continuous-at"),
                    &RUNS_CUTS,
                    &mut d.runs_cut,
                );
                small_field(ui, d.runs_cut == 2, &mut d.runs_cut_value);
            });
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.wilcoxon, t("stats-nptests-wilcoxon-median"));
        small_field(ui, d.wilcoxon, &mut d.test_median);
    });
}

fn ks(ui: &mut Ui, d: &mut Nptests) {
    let [normal, uniform, exponential, poisson] = &mut d.distributions;
    ui.checkbox(normal, t(KS_DISTRIBUTIONS[0]));
    ui.add_enabled_ui(*normal, |ui| {
        ui.indent("normal", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.normal_custom, t("stats-nptests-mean"));
                small_field(ui, d.normal_custom, &mut d.normal_mean);
                ui.label(t("stats-nptests-std-deviation"));
                small_field(ui, d.normal_custom, &mut d.normal_sd);
            });
            ui.add_enabled_ui(!d.normal_custom, |ui| {
                ui.checkbox(&mut d.lilliefors, t("stats-nptests-lilliefors"));
            });
        });
    });
    ui.checkbox(uniform, t(KS_DISTRIBUTIONS[1]));
    ui.add_enabled_ui(*uniform, |ui| {
        ui.indent("uniform", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.uniform_custom, t("stats-nptests-minimum"));
                small_field(ui, d.uniform_custom, &mut d.uniform_min);
                ui.label(t("stats-nptests-maximum"));
                small_field(ui, d.uniform_custom, &mut d.uniform_max);
            });
        });
    });
    ui.checkbox(exponential, t(KS_DISTRIBUTIONS[2]));
    ui.add_enabled_ui(*exponential, |ui| {
        ui.indent("exponential", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.exponential_custom, t("stats-nptests-mean"));
                small_field(ui, d.exponential_custom, &mut d.exponential_mean);
            });
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(poisson, t(KS_DISTRIBUTIONS[3]));
        ui.label(t("stats-nptests-mean-lower"));
        small_field(ui, *poisson, &mut d.poisson_mean);
    });
    ui.weak(t("stats-nptests-parameters-from-data"));
    labelled(ui, t("stats-nptests-mc-samples"), true, &mut d.samples);
    labelled(ui, t("stats-nptests-mc-level"), true, &mut d.mc_level);
}

/// The k-sample tests, each with its multiple comparisons; `extra` draws the settings of one.
fn k_tests(
    ui: &mut Ui,
    d: &mut Nptests,
    tests: &[(&str, &'static str); 3],
    mut extra: impl FnMut(&mut Ui, &mut Nptests, usize),
) {
    for (i, (_, label)) in tests.iter().enumerate() {
        ui.checkbox(&mut d.k_tests[i], t(label));
        ui.add_enabled_ui(d.k_tests[i], |ui| {
            ui.indent(i, |ui| {
                ui.push_id(i, |ui| {
                    extra(ui, d, i);
                    choice(
                        ui,
                        t("stats-nptests-multiple-comparisons"),
                        &NP_COMPARE,
                        &mut d.compare[i],
                    );
                });
            });
        });
    }
}

fn independent(ui: &mut Ui, d: &mut Nptests) {
    ui.label(t("stats-nptests-two-groups"));
    checkboxes(ui, &INDEPENDENT_TWO, &mut d.two_tests);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.moses, t("stats-nptests-moses-trim"));
        small_field(ui, d.moses, &mut d.trim);
    });
    ui.weak(t("stats-nptests-empty-trims"));
    ui.label(t("stats-nptests-two-or-more-groups"));
    k_tests(ui, d, &INDEPENDENT_K, |ui, d, i| match INDEPENDENT_K[i].0 {
        "MEDIAN" => {
            labelled(
                ui,
                t("stats-nptests-median-to-compare"),
                true,
                &mut d.median_value,
            );
            ui.weak(t("stats-nptests-empty-uses-pooled-median"));
        }
        "JONCKHEERE_TERPSTRA" => {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.descending, false, t("stats-nptests-ascending"));
                ui.radio_value(&mut d.descending, true, t("stats-nptests-descending"));
            });
        }
        _ => {}
    });
}

fn related(ui: &mut Ui, d: &mut Nptests) {
    ui.label(t("stats-nptests-two-fields"));
    ui.checkbox(&mut d.mcnemar, "McNemar");
    checkboxes(ui, &RELATED_TWO, &mut d.two_tests);
    ui.label(t("stats-nptests-two-or-more-fields"));
    k_tests(ui, d, &RELATED_K, |_, _, _| {});
    let categorical = d.mcnemar || d.k_tests[0];
    checked_text(
        ui,
        &mut d.success_list,
        t("stats-nptests-success-values-mcnemar"),
        &mut d.success,
        categorical,
    );
    ui.weak(t("stats-nptests-otherwise-first-value"));
}
