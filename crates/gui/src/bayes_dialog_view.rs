//! Drawing of the Bayesian Statistics dialogs.

use crate::app::{choice, field, target_list, target_one};
use crate::bayes_dialog::{
    BAYES_ANALYSES, BAYES_FACTORS, BAYES_GROUP_PRIORS, BAYES_SELECTS, BAYES_SUCCESSES,
    BAYES_VARIANCE_PRIORS, BayesKind, Bayesian, GONEN, GROUP_INVCHISQ, HYPER, SELECT_CUTPOINT,
    SUCCESS_CUTPOINT, SUCCESS_LEVEL, variance_parameters,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Checkbox, Ui};

pub fn bayes(ui: &mut Ui, d: &mut Bayesian, picked: &mut Vec<String>) {
    match d.kind {
        BayesKind::Related => pairs(ui, &mut d.pairs, picked),
        BayesKind::Correlation => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.weak("Every variable is paired with each one after it");
        }
        _ => target_list(ui, "Test variables", &mut d.variables, picked),
    }
    if d.kind == BayesKind::Independent {
        groups(ui, d, picked);
    }
    ui.add_space(6.0);
    choice(ui, "Analysis", &BAYES_ANALYSES, &mut d.analysis);
    match d.kind {
        BayesKind::Normal | BayesKind::Related => normal(ui, d),
        BayesKind::Binomial | BayesKind::Poisson => counts(ui, d),
        BayesKind::Independent => independent(ui, d),
        BayesKind::Correlation => correlation(ui, d),
    }
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        field(ui, "Credible interval %", &mut d.ci_level, 50.0);
        if !matches!(d.kind, BayesKind::Binomial | BayesKind::Poisson) {
            field(ui, "Tolerance", &mut d.tolerance, 80.0);
            field(ui, "Most iterations", &mut d.max_iterations, 60.0);
        }
        if d.monte_carlo_offered() {
            ui.checkbox(&mut d.monte_carlo, "Bayes factor by Monte Carlo draws");
            ui.horizontal(|ui| {
                ui.label("Draws");
                small_field(ui, d.monte_carlo, &mut d.mc_samples);
                ui.label("Seed (empty: random)");
                small_field(ui, d.monte_carlo, &mut d.seed);
            });
        }
        let label = match d.kind {
            BayesKind::Correlation => {
                "Leave out a case missing on any variable (else pair by pair)"
            }
            _ => "Leave out a case missing on any variable (else test by test)",
        };
        ui.checkbox(&mut d.listwise, label);
    });
}

/// The pairs of a related-samples test, each added from two picked variables.
fn pairs(ui: &mut Ui, list: &mut Vec<(String, String)>, picked: &mut Vec<String>) {
    ui.strong("Pairs (first minus second)");
    let mut remove = None;
    for (i, (a, b)) in list.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{a}  with  {b}"));
            if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui
        .add_enabled(picked.len() >= 2, egui::Button::new("Add pair"))
        .clicked()
    {
        let first = picked.remove(0);
        list.push((first, picked.remove(0)));
    }
}

/// The grouping variable and how it splits the cases into two groups.
fn groups(ui: &mut Ui, d: &mut Bayesian, picked: &mut Vec<String>) {
    target_one(ui, "Grouping variable", &mut d.group, picked);
    choice(ui, "Groups", &BAYES_SELECTS, &mut d.select);
    ui.horizontal(|ui| match d.select {
        0 => {
            ui.label("Group 1");
            small_field(ui, true, &mut d.level1);
            ui.label("Group 2");
            small_field(ui, true, &mut d.level2);
        }
        SELECT_CUTPOINT => {
            ui.label("Cut point");
            small_field(ui, true, &mut d.cutpoint);
        }
        _ => {}
    });
    ui.weak("The difference is group 2 minus group 1");
}

/// The test and the priors of a normal mean, one sample or the differences of pairs.
fn normal(ui: &mut Ui, d: &mut Bayesian) {
    egui::CollapsingHeader::new("Test").show(ui, |ui| {
        field(
            ui,
            "Known variance (empty: unknown)",
            &mut d.known_variance,
            60.0,
        );
        ui.add_enabled_ui(d.factor(), |ui| {
            field(ui, "Test value", &mut d.null, 60.0);
            field(
                ui,
                "g of the alternative (empty: integrated out)",
                &mut d.g,
                60.0,
            );
        });
        ui.weak("The test value and g shape the Bayes factor; g is 1 with a known variance");
    });
    let known = !d.known_variance.trim().is_empty();
    egui::CollapsingHeader::new("Prior").show(ui, |ui| {
        ui.add_enabled_ui(d.posterior(), |ui| {
            ui.add_enabled_ui(!known, |ui| {
                choice(
                    ui,
                    "Variance",
                    &BAYES_VARIANCE_PRIORS,
                    &mut d.variance_prior,
                );
                let count = variance_parameters(d.variance_prior);
                ui.horizontal(|ui| {
                    ui.label("Parameters");
                    small_field(ui, count > 0, &mut d.prior_a);
                    small_field(ui, count > 1, &mut d.prior_b);
                });
            });
            let used = d.mean_prior_used();
            ui.add_enabled(
                used,
                Checkbox::new(&mut d.normal_mean, "Normal prior on the mean"),
            );
            ui.horizontal(|ui| {
                let on = used && d.normal_mean;
                ui.label("Mean");
                small_field(ui, on, &mut d.prior_mean);
                ui.label("Variance");
                small_field(ui, on && known, &mut d.prior_mean_variance);
                ui.label("Kappa");
                let kappa = d.kappa_used();
                small_field(ui, kappa, &mut d.kappa);
            });
            ui.weak(
                "The mean prior needs a known variance or a conjugate prior on the variance; \
                 with the latter its variance comes from kappa",
            );
        });
    });
}

/// The success values and the beta or gamma priors of a proportion or a rate.
fn counts(ui: &mut Ui, d: &mut Bayesian) {
    if d.kind == BayesKind::Binomial {
        choice(ui, "Success", &BAYES_SUCCESSES, &mut d.success);
        if matches!(d.success, SUCCESS_CUTPOINT | SUCCESS_LEVEL) {
            field(ui, "Cut point or values", &mut d.success_value, 120.0);
            ui.weak("Values of a string variable are separated by commas");
        }
    }
    let what = match d.kind {
        BayesKind::Binomial => "Beta prior (a b)",
        _ => "Gamma prior (shape rate)",
    };
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            ui.label(what);
            field(ui, "Under the null hypothesis", &mut d.null_prior, 80.0);
            field(ui, "Under the alternative", &mut d.alter_prior, 80.0);
        });
    });
    egui::CollapsingHeader::new("Prior").show(ui, |ui| {
        ui.add_enabled_ui(d.posterior(), |ui| {
            field(
                ui,
                &format!("Conjugate {}", what.to_lowercase()),
                &mut d.conjugate,
                80.0,
            );
        });
    });
}

/// The variances, the priors of two group means and the Bayes factor of their difference.
fn independent(ui: &mut Ui, d: &mut Bayesian) {
    egui::CollapsingHeader::new("Prior").show(ui, |ui| {
        ui.add_enabled_ui(d.posterior(), |ui| {
            ui.checkbox(&mut d.known_variances, "Known group variances");
            ui.horizontal(|ui| {
                small_field(ui, d.known_variances, &mut d.variance1);
                small_field(ui, d.known_variances, &mut d.variance2);
            });
            ui.add_enabled_ui(!d.known_variances, |ui| {
                ui.checkbox(&mut d.equal_variances, "Equal group variances");
                ui.add_enabled_ui(d.equal_variances, |ui| {
                    choice(ui, "Variance", &BAYES_GROUP_PRIORS, &mut d.variance_prior);
                    let on = d.variance_prior == GROUP_INVCHISQ;
                    ui.horizontal(|ui| {
                        ui.label("Parameters");
                        small_field(ui, on, &mut d.prior_a);
                        small_field(ui, on, &mut d.prior_b);
                    });
                });
            });
            ui.add_enabled_ui(d.known_variances, |ui| {
                ui.checkbox(&mut d.normal_mean, "Normal priors on the group means");
                ui.horizontal(|ui| {
                    ui.label("Means");
                    small_field(ui, d.normal_mean, &mut d.group_means[0]);
                    small_field(ui, d.normal_mean, &mut d.group_means[1]);
                    ui.label("Variances");
                    small_field(ui, d.normal_mean, &mut d.group_means[2]);
                    small_field(ui, d.normal_mean, &mut d.group_means[3]);
                });
            });
            ui.weak("Unequal unknown variances need more than five cases per group");
        });
    });
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            choice(ui, "Method", &BAYES_FACTORS, &mut d.factor);
            ui.horizontal(|ui| match d.factor {
                GONEN => {
                    ui.label("Mean");
                    small_field(ui, true, &mut d.gonen_mean);
                    ui.label("Variance");
                    small_field(ui, true, &mut d.gonen_variance);
                }
                HYPER => {
                    ui.label("a (above -1, at most -0.5)");
                    small_field(ui, true, &mut d.hyper);
                }
                _ => {
                    ui.label("Scale");
                    small_field(ui, true, &mut d.rouder);
                }
            });
        });
    });
}

fn correlation(ui: &mut Ui, d: &mut Bayesian) {
    egui::CollapsingHeader::new("Prior").show(ui, |ui| {
        ui.add_enabled_ui(d.posterior(), |ui| {
            field(ui, "Power c of (1 - rho squared)", &mut d.cvalue, 60.0);
            ui.weak("0 gives a uniform prior on the correlation");
        });
    });
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.fraction, false, "JZS");
                ui.radio_value(&mut d.fraction, true, "Fractional");
            });
            ui.horizontal(|ui| {
                ui.label("Fraction");
                small_field(ui, d.fraction, &mut d.fraction_value);
                ui.label("Null correlation");
                small_field(ui, d.fraction, &mut d.fraction_null);
            });
        });
    });
}
