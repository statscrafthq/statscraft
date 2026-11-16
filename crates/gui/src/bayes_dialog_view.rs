//! Drawing of the Bayesian Statistics dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::bayes_dialog::{
    BAYES_ANALYSES, BAYES_CELLS, BAYES_COUNT_MODELS, BAYES_FACTORS, BAYES_G_PRIORS,
    BAYES_GROUP_PRIORS, BAYES_LINEAR_PRIORS, BAYES_MARGINS, BAYES_REPEATED_FACTORS, BAYES_SAVE,
    BAYES_SELECTS, BAYES_SUCCESSES, BAYES_TABLE_PRIORS, BAYES_VARIANCE_PRIORS, BayesKind, Bayesian,
    COUNT_MULTINOMIAL, COUNT_NONPARAMETRIC, COUNT_POISSON, FTestRow, G_HYPER, G_ROUDER, G_ZELLNER,
    GONEN, GROUP_INVCHISQ, HYPER, SELECT_CUTPOINT, SUCCESS_CUTPOINT, SUCCESS_LEVEL,
    variance_parameters,
};
use crate::stats_dialogs_view::small_field;
use crate::survival_dialog_view::save_fields;
use eframe::egui::{self, Checkbox, TextEdit, Ui};

pub fn bayes(ui: &mut Ui, d: &mut Bayesian, picked: &mut Vec<String>) {
    match d.kind {
        BayesKind::Related => pairs(ui, &mut d.pairs, picked),
        BayesKind::Correlation => {
            target_list(ui, "Variables", &mut d.variables, picked);
            ui.weak("Every variable is paired with each one after it");
        }
        BayesKind::Normal | BayesKind::Binomial | BayesKind::Poisson | BayesKind::Independent => {
            target_list(ui, "Test variables", &mut d.variables, picked)
        }
        BayesKind::Anova => {
            target_one(ui, "Dependent variable", &mut d.dependent, picked);
            target_one(ui, "Factor", &mut d.group, picked);
        }
        BayesKind::Regression => {
            target_one(ui, "Dependent variable", &mut d.dependent, picked);
            target_list(ui, "Factors", &mut d.factors, picked);
            target_list(ui, "Covariates", &mut d.covariates, picked);
        }
        BayesKind::Repeated => {
            target_list(ui, "Repeated variables", &mut d.variables, picked);
            ui.weak("Each variable holds one condition, measured on every case");
        }
        BayesKind::Loglinear => {
            target_one(ui, "Rows", &mut d.row, picked);
            target_one(ui, "Columns", &mut d.column, picked);
        }
    }
    if d.kind == BayesKind::Independent {
        groups(ui, d, picked);
    }
    if matches!(
        d.kind,
        BayesKind::Anova | BayesKind::Regression | BayesKind::Repeated
    ) {
        target_one(ui, "Regression weight", &mut d.weight, picked);
    }
    ui.add_space(6.0);
    choice(ui, "Analysis", &BAYES_ANALYSES, &mut d.analysis);
    match d.kind {
        BayesKind::Normal | BayesKind::Related => normal(ui, d),
        BayesKind::Binomial | BayesKind::Poisson => counts(ui, d),
        BayesKind::Independent => independent(ui, d),
        BayesKind::Correlation => correlation(ui, d),
        BayesKind::Anova | BayesKind::Regression => linear(ui, d, picked),
        BayesKind::Repeated => repeated(ui, d, picked),
        BayesKind::Loglinear => loglinear(ui, d),
    }
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        field(ui, "Credible interval %", &mut d.ci_level, 50.0);
        if d.numerical_offered() {
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
        if d.missing_offered() {
            ui.checkbox(&mut d.listwise, label);
        }
    });
}

/// A text field that can be switched off, with a label in front.
fn wide_field(ui: &mut Ui, on: bool, label: &str, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_enabled(on, TextEdit::singleline(text).desired_width(160.0));
    });
}

/// The Bayes factor, prior and plots of a one-way ANOVA or a regression, and the predictions,
/// saved values and F tests of a regression.
fn linear(ui: &mut Ui, d: &mut Bayesian, picked: &mut Vec<String>) {
    let regression = d.kind == BayesKind::Regression;
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            choice(ui, "Prior on g", &BAYES_G_PRIORS, &mut d.factor);
            ui.horizontal(|ui| match d.factor {
                G_ZELLNER => {
                    ui.label("g");
                    small_field(ui, true, &mut d.g);
                }
                G_HYPER => {
                    ui.label("a (above 2)");
                    small_field(ui, true, &mut d.hyper);
                }
                G_ROUDER => {
                    ui.label("Scale");
                    small_field(ui, true, &mut d.rouder);
                }
                _ => {}
            });
            if regression {
                ui.checkbox(
                    &mut d.compare_full,
                    "Also weigh each model without one variable against the full model",
                );
            }
        });
    });
    egui::CollapsingHeader::new("Prior").show(ui, |ui| {
        ui.add_enabled_ui(d.linear_prior_used(), |ui| {
            choice(ui, "Prior", &BAYES_LINEAR_PRIORS, &mut d.variance_prior);
            let on = d.variance_prior == 1;
            ui.horizontal(|ui| {
                ui.label("Error variance: shape");
                small_field(ui, on, &mut d.prior_a);
                ui.label("scale");
                small_field(ui, on, &mut d.prior_b);
            });
            wide_field(ui, on, "Means", &mut d.prior_mean);
            wide_field(ui, on, "Matrix (empty: identity)", &mut d.vmatrix);
            ui.weak(match regression {
                true => {
                    "One mean per parameter: the intercept, each category of every factor, \
                         then the covariates"
                }
                false => "One mean per group, in ascending order of the factor",
            });
            ui.weak("The matrix is the lower triangle, row by row, rows separated by ;");
        });
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        if regression {
            target_list(ui, "Parameters of", &mut d.plotted, picked);
            let factor_plotted = d.plotted.iter().any(|n| d.factors.contains(n));
            ui.horizontal(|ui| {
                ui.label("Categories per factor");
                small_field(ui, factor_plotted, &mut d.max_category);
            });
            ui.checkbox(&mut d.plot_intercept, "Intercept");
        } else {
            wide_field(ui, true, "Group means of", &mut d.plot_values);
            ui.weak("Values of the factor; those of a string factor separated by commas");
        }
        ui.checkbox(&mut d.plot_error, "Error variance");
        if regression {
            let predicting = d.predicting();
            ui.add_enabled(
                predicting,
                Checkbox::new(&mut d.plot_prediction, "Predictive distribution"),
            );
        }
    });
    if !regression {
        return;
    }
    egui::CollapsingHeader::new("Prediction").show(ui, |ui| {
        let model: Vec<String> = d.model().cloned().collect();
        for name in model {
            let typed = d.predict.entry(name.clone()).or_default();
            field(ui, &name, typed, 80.0);
        }
        ui.weak("A category of each factor and a number for each covariate; empty: none");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_fields(ui, &BAYES_SAVE, &mut d.save, &mut d.save_names);
    });
    egui::CollapsingHeader::new("F tests").show(ui, |ui| ftests(ui, &mut d.ftests, picked));
}

/// The F tests of a regression, each with its own variables.
fn ftests(ui: &mut Ui, tests: &mut Vec<FTestRow>, picked: &mut Vec<String>) {
    let mut remove = None;
    for (i, test) in tests.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Test {}", i + 1));
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
            field(ui, "Label", &mut test.label, 120.0);
            target_list(ui, "Variables", &mut test.variables, picked);
            ui.checkbox(&mut test.intercept, "With the intercept");
            field(ui, "Test values (empty: all 0)", &mut test.values, 120.0);
        });
    }
    if let Some(i) = remove {
        tests.remove(i);
    }
    if ui.button("Add test").clicked() {
        tests.push(FTestRow::default());
    }
    ui.weak("Test values: the intercept first, then one per parameter of each variable");
}

/// The Bayes factor and plots of repeated measures.
fn repeated(ui: &mut Ui, d: &mut Bayesian, picked: &mut Vec<String>) {
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            choice(ui, "Method", &BAYES_REPEATED_FACTORS, &mut d.factor);
            let on = d.draws_used();
            ui.horizontal(|ui| {
                ui.label("Draws");
                small_field(ui, on, &mut d.mc_samples);
                ui.label("Seed (empty: random)");
                small_field(ui, on, &mut d.seed);
            });
        });
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        target_list(ui, "Posterior means of", &mut d.plotted, picked);
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

/// The Bayes factor, posterior draws and crosstab of a two-way table.
fn loglinear(ui: &mut Ui, d: &mut Bayesian) {
    egui::CollapsingHeader::new("Bayes factor").show(ui, |ui| {
        ui.add_enabled_ui(d.factor(), |ui| {
            choice(ui, "Model", &BAYES_COUNT_MODELS, &mut d.count_model);
            ui.add_enabled_ui(d.count_model != COUNT_POISSON, |ui| {
                choice(ui, "Fixed by the design", &BAYES_MARGINS, &mut d.margin);
            });
            ui.add_enabled_ui(d.count_model == COUNT_MULTINOMIAL, |ui| {
                choice(ui, "Prior", &BAYES_TABLE_PRIORS, &mut d.factor);
            });
            let shapes = d.shapes_used();
            wide_field(ui, shapes, "Gamma shapes (empty: 1)", &mut d.shapes);
            ui.horizontal(|ui| {
                ui.label("Gamma scale (empty: from the data)");
                small_field(ui, d.count_model == COUNT_POISSON, &mut d.scale);
            });
            let nonparametric = d.count_model == COUNT_NONPARAMETRIC;
            wide_field(ui, nonparametric, "Dirichlet parameters", &mut d.lambdas);
            ui.weak(
                "Shapes: one value for every cell, or one row per table row separated by ;. \
                 The nonparametric model needs fixed row or column totals",
            );
        });
    });
    egui::CollapsingHeader::new("Posterior").show(ui, |ui| {
        let on = d.draws_used();
        ui.horizontal(|ui| {
            ui.label("Draws");
            small_field(ui, on, &mut d.mc_samples);
            ui.label("Seed (empty: random)");
            small_field(ui, on, &mut d.seed);
        });
    });
    egui::CollapsingHeader::new("Table").show(ui, |ui| {
        ui.checkbox(&mut d.suppress_table, "Hide the crosstab");
        ui.add_enabled_ui(!d.suppress_table, |ui| {
            checkboxes(ui, &BAYES_CELLS, &mut d.cells);
        });
        ui.checkbox(&mut d.chisq, "Chi-square tests");
        ui.checkbox(&mut d.lrt, "Likelihood ratio test");
        ui.checkbox(&mut d.descending, "Categories from the highest value");
    });
}
