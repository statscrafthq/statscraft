//! Drawing of the One Sample, Independent Samples and Related Samples dialogs of Nonparametric
//! Tests.

use crate::app::{checkboxes, choice, field, target_list, target_one};
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
    target_list(ui, "Test fields", &mut d.fields, picked);
    if d.kind == NpKind::Independent {
        target_one(ui, "Groups", &mut d.group, picked);
    }
    ui.add_space(6.0);
    ui.radio_value(
        &mut d.customize,
        false,
        "Choose tests automatically from the data",
    );
    ui.radio_value(&mut d.customize, true, "Customize tests");
    ui.add_enabled_ui(d.customize, |ui| {
        egui::CollapsingHeader::new("Tests")
            .default_open(true)
            .show(ui, |ui| match d.kind {
                NpKind::OneSample => one_sample(ui, d),
                NpKind::Independent => independent(ui, d),
                NpKind::Related => related(ui, d),
            });
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        field(ui, "Significance level", &mut d.alpha, 50.0);
        field(ui, "Confidence level (%)", &mut d.ci_level, 50.0);
        ui.radio_value(&mut d.listwise, false, "Exclude cases test by test");
        ui.radio_value(&mut d.listwise, true, "Exclude cases listwise");
        ui.checkbox(
            &mut d.include,
            "User-missing values of categorical fields are valid",
        );
    });
}

fn one_sample(ui: &mut Ui, d: &mut Nptests) {
    ui.checkbox(&mut d.binomial, "Binomial test");
    ui.add_enabled_ui(d.binomial, |ui| {
        ui.indent("binomial", |ui| {
            labelled(ui, "Test proportion", true, &mut d.proportion);
            checked_text(
                ui,
                &mut d.success_list,
                "Success values",
                &mut d.success,
                true,
            );
            ui.weak("Otherwise the first value found");
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.cut_value, "Cut continuous fields at");
                small_field(ui, d.cut_value, &mut d.cut);
            });
            ui.weak("Otherwise halfway between the lowest and highest value");
            ui.label("Confidence intervals");
            checkboxes(ui, &BINOMIAL_INTERVALS, &mut d.intervals);
        });
    });
    ui.checkbox(&mut d.chisquare, "Chi-square test");
    ui.add_enabled_ui(d.chisquare, |ui| {
        ui.indent("chisquare", |ui| {
            ui.radio_value(&mut d.expected_custom, false, "Equal expected frequencies");
            ui.radio_value(
                &mut d.expected_custom,
                true,
                "Expected frequencies of categories",
            );
            ui.add_enabled_ui(d.expected_custom, |ui| {
                field(ui, "Categories", &mut d.categories, 140.0);
                field(ui, "Frequencies", &mut d.frequencies, 140.0);
            });
        });
    });
    ui.checkbox(&mut d.ks, "Kolmogorov-Smirnov test");
    ui.add_enabled_ui(d.ks, |ui| {
        ui.indent("ks", |ui| ks(ui, d));
    });
    ui.checkbox(&mut d.runs, "Runs test");
    ui.add_enabled_ui(d.runs, |ui| {
        ui.indent("runs", |ui| {
            checked_text(
                ui,
                &mut d.runs_list,
                "First group of categorical fields",
                &mut d.runs_values,
                true,
            );
            ui.weak("Otherwise the two values found");
            ui.horizontal(|ui| {
                choice(ui, "Cut continuous fields at", &RUNS_CUTS, &mut d.runs_cut);
                small_field(ui, d.runs_cut == 2, &mut d.runs_cut_value);
            });
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.wilcoxon, "Wilcoxon signed-rank test, median");
        small_field(ui, d.wilcoxon, &mut d.test_median);
    });
}

fn ks(ui: &mut Ui, d: &mut Nptests) {
    let [normal, uniform, exponential, poisson] = &mut d.distributions;
    ui.checkbox(normal, KS_DISTRIBUTIONS[0]);
    ui.add_enabled_ui(*normal, |ui| {
        ui.indent("normal", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.normal_custom, "Mean");
                small_field(ui, d.normal_custom, &mut d.normal_mean);
                ui.label("Std. deviation");
                small_field(ui, d.normal_custom, &mut d.normal_sd);
            });
            ui.add_enabled_ui(!d.normal_custom, |ui| {
                ui.checkbox(
                    &mut d.lilliefors,
                    "Lilliefors table instead of Monte Carlo samples",
                );
            });
        });
    });
    ui.checkbox(uniform, KS_DISTRIBUTIONS[1]);
    ui.add_enabled_ui(*uniform, |ui| {
        ui.indent("uniform", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.uniform_custom, "Minimum");
                small_field(ui, d.uniform_custom, &mut d.uniform_min);
                ui.label("Maximum");
                small_field(ui, d.uniform_custom, &mut d.uniform_max);
            });
        });
    });
    ui.checkbox(exponential, KS_DISTRIBUTIONS[2]);
    ui.add_enabled_ui(*exponential, |ui| {
        ui.indent("exponential", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.exponential_custom, "Mean");
                small_field(ui, d.exponential_custom, &mut d.exponential_mean);
            });
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(poisson, KS_DISTRIBUTIONS[3]);
        ui.label("mean");
        small_field(ui, *poisson, &mut d.poisson_mean);
    });
    ui.weak("Parameters left unticked come from the data");
    labelled(ui, "Monte Carlo samples", true, &mut d.samples);
    labelled(
        ui,
        "Monte Carlo confidence level (%)",
        true,
        &mut d.mc_level,
    );
}

/// The k-sample tests, each with its multiple comparisons; `extra` draws the settings of one.
fn k_tests(
    ui: &mut Ui,
    d: &mut Nptests,
    tests: &[(&str, &str); 3],
    mut extra: impl FnMut(&mut Ui, &mut Nptests, usize),
) {
    for (i, (_, label)) in tests.iter().enumerate() {
        ui.checkbox(&mut d.k_tests[i], *label);
        ui.add_enabled_ui(d.k_tests[i], |ui| {
            ui.indent(i, |ui| {
                ui.push_id(i, |ui| {
                    extra(ui, d, i);
                    choice(ui, "Multiple comparisons", &NP_COMPARE, &mut d.compare[i]);
                });
            });
        });
    }
}

fn independent(ui: &mut Ui, d: &mut Nptests) {
    ui.label("Two groups");
    checkboxes(ui, &INDEPENDENT_TWO, &mut d.two_tests);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.moses, "Moses extreme reactions, cases to trim");
        small_field(ui, d.moses, &mut d.trim);
    });
    ui.weak("Empty trims 5% of the control group from each end");
    ui.label("Two or more groups");
    k_tests(ui, d, &INDEPENDENT_K, |ui, d, i| match INDEPENDENT_K[i].0 {
        "MEDIAN" => {
            labelled(ui, "Median to compare with", true, &mut d.median_value);
            ui.weak("Empty uses the median of all groups");
        }
        "JONCKHEERE_TERPSTRA" => {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.descending, false, "Ascending");
                ui.radio_value(&mut d.descending, true, "Descending");
            });
        }
        _ => {}
    });
}

fn related(ui: &mut Ui, d: &mut Nptests) {
    ui.label("Two fields");
    ui.checkbox(&mut d.mcnemar, "McNemar");
    checkboxes(ui, &RELATED_TWO, &mut d.two_tests);
    ui.label("Two or more fields");
    k_tests(ui, d, &RELATED_K, |_, _, _| {});
    let categorical = d.mcnemar || d.k_tests[0];
    checked_text(
        ui,
        &mut d.success_list,
        "Success values (McNemar, Cochran)",
        &mut d.success,
        categorical,
    );
    ui.weak("Otherwise the first value found");
}
