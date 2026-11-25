//! Drawing of the Factor Analysis, K-Means Cluster, Hierarchical Cluster, Discriminant, Binary
//! Logistic Regression, Univariate, Multivariate and Repeated Measures dialogs.

use crate::analyze_dialogs::{DUNNETT_SIDES, EQUAL_VARIANCE_TESTS, POST_HOC_TESTS};
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::i18n::t;
use crate::model_dialogs::{
    CLUSTER_BINARY, CLUSTER_COUNTS, CLUSTER_DATA, CLUSTER_INTERVAL, CLUSTER_METHODS, Categorical,
    ClusterMeasure, DISCRIMINANT_METHODS, DISCRIMINANT_SAVE, DISCRIMINANT_STATISTICS,
    DISCRIMINANT_STATISTICS_GROUPS, Discriminant, FACTOR_DESCRIPTIVES, FACTOR_EXTRACTIONS,
    FACTOR_MISSING, FACTOR_ROTATIONS, FACTOR_SCORES, FactorAnalysis, FactorContrast,
    GLM_ADJUSTMENTS, GLM_CONTRASTS, GLM_HET_TESTS, GLM_PRINT, GLM_SAVE, GLM_SAVE_GROUPS,
    HET_MODELS, Hierarchical, KMeans, LOGISTIC_CONTRASTS, LOGISTIC_METHODS, LOGISTIC_SAVE,
    LOGISTIC_SAVE_GROUPS, Logistic, MColumn, PRINT_M_MATRIX, PROFILE_ERROR_BARS, ROBUST_ESTIMATORS,
    RepeatedMeasures, SELECT_RELATIONS, SUM_OF_SQUARES, Solutions, UNIVARIATE_PRINT, Univariate,
    WithinFactor, glm_has_reference, has_reference,
};
use crate::stats_dialogs_view::{blocks, small_field};
use eframe::egui::{self, ComboBox, TextEdit, Ui};

/// A checkbox for each of `options`, adding to or taking from `chosen`.
pub(crate) fn name_checkboxes(ui: &mut Ui, options: &[String], chosen: &mut Vec<String>) {
    ui.horizontal_wrapped(|ui| {
        for name in options {
            let mut on = chosen.contains(name);
            if ui.checkbox(&mut on, name.as_str()).changed() {
                if on {
                    chosen.push(name.clone());
                } else {
                    chosen.retain(|c| c != name);
                }
            }
        }
    });
}

/// The post hoc tests of One-Way ANOVA and Univariate.
pub(crate) fn post_hoc_tests(
    ui: &mut Ui,
    flags: &mut [bool; 16],
    dunnett: &mut bool,
    dunnett_first: &mut bool,
    dunnett_side: &mut usize,
) {
    let (equal, unequal) = POST_HOC_TESTS.split_at(EQUAL_VARIANCE_TESTS);
    let (equal_flags, unequal_flags) = flags.split_at_mut(EQUAL_VARIANCE_TESTS);
    ui.weak(t("stats-model-equal-variances-assumed"));
    checkboxes(ui, equal, equal_flags);
    ui.horizontal(|ui| {
        ui.checkbox(dunnett, t("stats-model-dunnett-control"));
        ui.add_enabled_ui(*dunnett, |ui| {
            ui.radio_value(dunnett_first, true, t("stats-model-first"));
            ui.radio_value(dunnett_first, false, t("stats-model-last"));
            ComboBox::new("dunnett_side", "")
                .selected_text(crate::i18n::t(DUNNETT_SIDES[*dunnett_side].1))
                .show_index(ui, dunnett_side, DUNNETT_SIDES.len(), |i| {
                    crate::i18n::t(DUNNETT_SIDES[i].1)
                });
        });
    });
    ui.weak(t("stats-model-equal-variances-not-assumed"));
    checkboxes(ui, unequal, unequal_flags);
}

/// The checkboxes of saved variables in their groups.
fn save_groups(
    ui: &mut Ui,
    groups: &[(&'static str, std::ops::Range<usize>)],
    options: &[(&str, &str)],
    flags: &mut [bool],
) {
    for (title, range) in groups {
        ui.label(t(title));
        checkboxes(ui, &options[range.clone()], &mut flags[range.clone()]);
    }
}

pub fn factor(ui: &mut Ui, d: &mut FactorAnalysis, picked: &mut Vec<String>) {
    target_list(ui, t("stats-model-variables"), &mut d.variables, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-model-descriptives")).show(ui, |ui| {
        checkboxes(ui, &FACTOR_DESCRIPTIVES, &mut d.descriptives);
    });
    egui::CollapsingHeader::new(t("stats-model-extraction")).show(ui, |ui| {
        choice(
            ui,
            t("stats-model-method"),
            &FACTOR_EXTRACTIONS,
            &mut d.extraction,
        );
        ui.add_enabled_ui(d.covariance_offered(), |ui| {
            ui.horizontal(|ui| {
                ui.label(t("stats-model-analyze"));
                ui.radio_value(
                    &mut d.covariance,
                    false,
                    t("stats-model-correlation-matrix"),
                );
                ui.radio_value(&mut d.covariance, true, t("stats-model-covariance-matrix"));
            });
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.fixed, false, t("stats-model-eigenvalues-over"));
            small_field(ui, !d.fixed, &mut d.min_eigen);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.fixed, true, t("stats-model-fixed-number-of-factors"));
            small_field(ui, d.fixed, &mut d.factors);
        });
        field(
            ui,
            t("stats-model-maximum-iterations"),
            &mut d.iterations,
            50.0,
        );
    });
    egui::CollapsingHeader::new(t("stats-model-rotation")).show(ui, |ui| {
        choice(
            ui,
            t("stats-model-method"),
            &FACTOR_ROTATIONS,
            &mut d.rotation,
        );
        match FACTOR_ROTATIONS[d.rotation].0 {
            "OBLIMIN" => field(ui, "Delta", &mut d.delta, 50.0),
            "PROMAX" => field(ui, "Kappa", &mut d.kappa, 50.0),
            _ => {}
        }
    });
    egui::CollapsingHeader::new(t("stats-model-scores")).show(ui, |ui| {
        ui.checkbox(&mut d.save, t("stats-model-save-as-variables"));
        ui.add_enabled_ui(d.save, |ui| {
            choice(
                ui,
                t("stats-model-method"),
                &FACTOR_SCORES,
                &mut d.score_method,
            );
        });
        ui.checkbox(
            &mut d.score_coefficients,
            t("stats-model-display-score-coefficients"),
        );
    });
    egui::CollapsingHeader::new(t("stats-model-options")).show(ui, |ui| {
        ui.label(t("stats-model-missing-values"));
        for (i, (_, label)) in FACTOR_MISSING.iter().enumerate() {
            ui.radio_value(&mut d.missing, i, t(label));
        }
        ui.label(t("stats-model-coefficient-display"));
        ui.checkbox(&mut d.sort, t("stats-model-sorted-by-size"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.blank, t("stats-model-suppress-below"));
            small_field(ui, d.blank, &mut d.blank_value);
        });
    });
}

pub fn k_means(ui: &mut Ui, d: &mut KMeans, picked: &mut Vec<String>) {
    target_list(ui, t("stats-model-variables"), &mut d.variables, picked);
    field(
        ui,
        t("stats-model-number-of-clusters-field"),
        &mut d.clusters,
        50.0,
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.strong(t("stats-model-method"));
        ui.radio_value(
            &mut d.classify_only,
            false,
            t("stats-model-iterate-and-classify"),
        );
        ui.radio_value(&mut d.classify_only, true, t("stats-model-classify-only"));
    });
    ui.add_enabled_ui(!d.classify_only, |ui| {
        field(
            ui,
            t("stats-model-maximum-iterations"),
            &mut d.iterations,
            50.0,
        );
        field(
            ui,
            t("stats-model-convergence-criterion-field"),
            &mut d.converge,
            50.0,
        );
    });
    ui.add_space(6.0);
    ui.strong(t("stats-model-save"));
    ui.checkbox(&mut d.save_cluster, t("stats-model-cluster-membership"));
    ui.checkbox(&mut d.save_distance, t("stats-model-distance-from-center"));
    ui.add_space(6.0);
    ui.strong(t("stats-model-statistics"));
    ui.checkbox(&mut d.initial, t("stats-model-initial-centers"));
    ui.checkbox(&mut d.anova, t("stats-model-anova-table"));
    ui.checkbox(&mut d.membership, t("stats-model-cluster-information"));
    ui.add_enabled_ui(d.membership, |ui| {
        target_one(ui, t("stats-model-label-cases-by"), &mut d.label, picked);
    });
    ui.add_space(6.0);
    ui.strong(t("stats-model-missing-values"));
    ui.radio_value(&mut d.pairwise, false, t("stats-model-missing-listwise"));
    ui.radio_value(
        &mut d.pairwise,
        true,
        t("stats-model-missing-pairwise-distances"),
    );
}

/// No cluster membership, one solution or a range of them.
fn solutions(ui: &mut Ui, title: &str, s: &mut Solutions) {
    ui.push_id(title, |ui| {
        ui.label(title);
        ui.radio_value(&mut s.kind, 0, t("stats-model-solution-none"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.kind, 1, t("stats-model-single-solution"));
            small_field(ui, s.kind == 1, &mut s.clusters);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.kind, 2, t("stats-model-range-of-solutions"));
            small_field(ui, s.kind == 2, &mut s.min);
            ui.label(t("stats-model-to"));
            small_field(ui, s.kind == 2, &mut s.max);
        });
    });
}

pub fn hierarchical(ui: &mut Ui, d: &mut Hierarchical, picked: &mut Vec<String>) {
    target_list(ui, t("stats-model-variables"), &mut d.variables, picked);
    target_one(ui, t("stats-model-label-cases-by"), &mut d.label, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-model-statistics")).show(ui, |ui| {
        ui.checkbox(&mut d.schedule, t("stats-model-agglomeration-schedule"));
        ui.checkbox(&mut d.proximity, t("stats-model-proximity-matrix"));
        solutions(ui, t("stats-model-cluster-membership"), &mut d.membership);
    });
    egui::CollapsingHeader::new(t("stats-model-method")).show(ui, |ui| {
        choice(
            ui,
            t("stats-model-cluster-method"),
            &CLUSTER_METHODS,
            &mut d.method,
        );
        measure(ui, &mut d.measure);
    });
    egui::CollapsingHeader::new(t("stats-model-save")).show(ui, |ui| {
        solutions(ui, t("stats-model-cluster-membership"), &mut d.save);
    });
}

/// The kind of data, its measure and the measure's parameters.
pub(crate) fn measure(ui: &mut Ui, d: &mut ClusterMeasure) {
    ui.horizontal(|ui| {
        ui.label(t("stats-model-measure-for"));
        for (i, kind) in CLUSTER_DATA.iter().enumerate() {
            ui.radio_value(&mut d.data, i, t(kind));
        }
    });
    match d.data {
        0 => {
            choice(
                ui,
                t("stats-model-measure"),
                &CLUSTER_INTERVAL,
                &mut d.interval,
            );
            match CLUSTER_INTERVAL[d.interval].0 {
                "MINKOWSKI" => field(ui, t("stats-model-power"), &mut d.power, 50.0),
                "POWER" => {
                    ui.horizontal(|ui| {
                        ui.label(t("stats-model-power"));
                        small_field(ui, true, &mut d.power);
                        ui.label(t("stats-model-root"));
                        small_field(ui, true, &mut d.root);
                    })
                    .inner
                }
                _ => {}
            }
        }
        1 => choice(ui, t("stats-model-measure"), &CLUSTER_COUNTS, &mut d.counts),
        _ => {
            choice(ui, t("stats-model-measure"), &CLUSTER_BINARY, &mut d.binary);
            ui.horizontal(|ui| {
                ui.label(t("stats-model-present"));
                small_field(ui, true, &mut d.present);
                ui.label(t("stats-model-absent"));
                small_field(ui, true, &mut d.absent);
            });
        }
    }
}

pub fn discriminant(ui: &mut Ui, d: &mut Discriminant, picked: &mut Vec<String>) {
    target_one(ui, t("stats-model-grouping-variable"), &mut d.group, picked);
    ui.horizontal(|ui| {
        ui.label(t("stats-model-range-from"));
        small_field(ui, true, &mut d.min);
        ui.label(t("stats-model-to"));
        small_field(ui, true, &mut d.max);
    });
    target_list(ui, t("stats-model-independents"), &mut d.variables, picked);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.stepwise, false, t("stats-model-enter-together"));
        ui.radio_value(&mut d.stepwise, true, t("stats-model-use-stepwise"));
    });
    target_one(
        ui,
        t("stats-model-selection-variable"),
        &mut d.select,
        picked,
    );
    ui.add_enabled_ui(!d.select.is_empty(), |ui| {
        field(ui, t("stats-model-value"), &mut d.select_value, 50.0);
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-model-statistics")).show(ui, |ui| {
        save_groups(
            ui,
            &DISCRIMINANT_STATISTICS_GROUPS,
            &DISCRIMINANT_STATISTICS,
            &mut d.statistics,
        );
    });
    ui.add_enabled_ui(d.stepwise, |ui| {
        egui::CollapsingHeader::new(t("stats-model-method")).show(ui, |ui| {
            choice(
                ui,
                t("stats-model-method"),
                &DISCRIMINANT_METHODS,
                &mut d.method,
            );
            ui.horizontal(|ui| {
                ui.label(t("stats-model-criteria"));
                let mut use_f = d.use_f;
                ui.radio_value(&mut use_f, true, t("stats-model-f-value"));
                ui.radio_value(&mut use_f, false, t("stats-model-probability-of-f"));
                if use_f != d.use_f {
                    d.set_use_f(use_f);
                }
            });
            ui.horizontal(|ui| {
                ui.label(t("stats-model-entry"));
                small_field(ui, true, &mut d.entry);
                ui.label(t("stats-model-removal"));
                small_field(ui, true, &mut d.removal);
            });
        });
    });
    egui::CollapsingHeader::new(t("stats-model-classify")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(t("stats-model-prior-probabilities"));
            ui.radio_value(&mut d.priors_size, false, t("stats-model-all-groups-equal"));
            ui.radio_value(&mut d.priors_size, true, t("stats-model-from-group-sizes"));
        });
        ui.horizontal(|ui| {
            ui.label(t("stats-model-covariance-matrix"));
            ui.radio_value(&mut d.separate, false, t("stats-model-within-groups"));
            ui.radio_value(&mut d.separate, true, t("stats-model-separate-groups"));
        });
        ui.checkbox(&mut d.table, t("stats-model-summary-table"));
        ui.checkbox(&mut d.leave_one_out, t("stats-model-leave-one-out"));
    });
    egui::CollapsingHeader::new(t("stats-model-save")).show(ui, |ui| {
        for ((_, label), on) in DISCRIMINANT_SAVE.iter().zip(&mut d.save) {
            ui.checkbox(on, t(label));
        }
    });
}

/// A checkbox for each covariate that makes it categorical, with its contrast (a new one starts
/// at `default`, an index into `LOGISTIC_CONTRASTS`) and its reference category.
pub(crate) fn categorical_grid(
    ui: &mut Ui,
    covariates: &[String],
    categorical: &mut Vec<Categorical>,
    default: usize,
) {
    egui::Grid::new("categorical")
        .num_columns(3)
        .show(ui, |ui| {
            for name in covariates {
                let at = categorical.iter().position(|c| &c.name == name);
                let mut on = at.is_some();
                if ui.checkbox(&mut on, name.as_str()).changed() {
                    match at {
                        Some(i) => {
                            categorical.remove(i);
                        }
                        None => categorical.push(Categorical {
                            name: name.clone(),
                            contrast: default,
                            first: false,
                        }),
                    }
                }
                if let Some(c) = categorical.iter_mut().find(|c| &c.name == name) {
                    ComboBox::from_id_salt(("contrast", name))
                        .width(90.0)
                        .selected_text(t(LOGISTIC_CONTRASTS[c.contrast].1))
                        .show_index(ui, &mut c.contrast, LOGISTIC_CONTRASTS.len(), |i| {
                            t(LOGISTIC_CONTRASTS[i].1)
                        });
                    ui.add_enabled_ui(has_reference(c.contrast), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(t("stats-model-reference"));
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.radio_value(&mut c.first, false, t("stats-model-last"));
                            ui.radio_value(&mut c.first, true, t("stats-model-first"));
                        });
                    });
                }
                ui.end_row();
            }
        });
}

pub fn logistic(ui: &mut Ui, d: &mut Logistic, picked: &mut Vec<String>) {
    target_one(
        ui,
        t("stats-model-dependent-variable"),
        &mut d.dependent,
        picked,
    );
    ui.add_space(6.0);
    let block = blocks(ui, &mut d.blocks, &mut d.block);
    target_list(
        ui,
        t("stats-model-covariates"),
        &mut block.variables,
        picked,
    );
    choice(
        ui,
        t("stats-model-method"),
        &LOGISTIC_METHODS,
        &mut block.method,
    );
    ui.add_space(6.0);
    target_one(
        ui,
        t("stats-model-selection-variable"),
        &mut d.select,
        picked,
    );
    ui.add_enabled_ui(!d.select.is_empty(), |ui| {
        ui.horizontal(|ui| {
            choice(
                ui,
                t("stats-model-rule-value"),
                &SELECT_RELATIONS,
                &mut d.select_relation,
            );
            small_field(ui, true, &mut d.select_value);
        });
    });
    ui.add_space(6.0);
    let mut covariates: Vec<String> = Vec::new();
    for name in d.blocks.iter().flat_map(|b| &b.variables) {
        if !covariates.contains(name) {
            covariates.push(name.clone());
        }
    }
    egui::CollapsingHeader::new(t("stats-model-categorical")).show(ui, |ui| {
        categorical_grid(ui, &covariates, &mut d.categorical, 0);
    });
    egui::CollapsingHeader::new(t("stats-model-save")).show(ui, |ui| {
        save_groups(ui, &LOGISTIC_SAVE_GROUPS, &LOGISTIC_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new(t("stats-model-options")).show(ui, |ui| {
        ui.checkbox(&mut d.goodfit, t("stats-model-hosmer-lemeshow"));
        ui.checkbox(&mut d.iteration_history, t("stats-model-iteration-history"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ci, t("stats-model-ci-exp-b"));
            small_field(ui, d.ci, &mut d.ci_level);
        });
        field(
            ui,
            t("stats-model-classification-cutoff-field"),
            &mut d.cut,
            50.0,
        );
        ui.horizontal(|ui| {
            ui.label(t("stats-model-stepwise-entry"));
            small_field(ui, true, &mut d.entry);
            ui.label(t("stats-model-stepwise-removal"));
            small_field(ui, true, &mut d.removal);
        });
        field(
            ui,
            t("stats-model-maximum-iterations"),
            &mut d.iterations,
            50.0,
        );
        ui.checkbox(&mut d.constant, t("stats-model-include-constant"));
    });
}

pub fn univariate(ui: &mut Ui, d: &mut Univariate, picked: &mut Vec<String>) {
    if d.multivariate {
        target_list(
            ui,
            t("stats-model-dependent-variables"),
            &mut d.dependents,
            picked,
        );
    } else {
        target_one(
            ui,
            t("stats-model-dependent-variable"),
            &mut d.dependent,
            picked,
        );
    }
    target_list(ui, t("stats-model-fixed-factors"), &mut d.factors, picked);
    if !d.multivariate {
        target_list(ui, t("stats-model-random-factors"), &mut d.random, picked);
    }
    target_list(ui, t("stats-model-covariates"), &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-model-model")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom, false, t("stats-model-full-factorial"));
            ui.radio_value(&mut d.custom, true, t("stats-model-custom"));
        });
        ui.add_enabled_ui(d.custom, |ui| {
            let model = d.model();
            name_checkboxes(ui, &model, &mut d.building);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !d.building.is_empty(),
                        egui::Button::new(t("stats-model-main-effects")),
                    )
                    .clicked()
                {
                    d.add_terms(false);
                }
                if ui
                    .add_enabled(
                        d.building.len() >= 2,
                        egui::Button::new(t("stats-model-interaction")),
                    )
                    .clicked()
                {
                    d.add_terms(true);
                }
            });
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(260.0);
                if d.terms.is_empty() {
                    ui.weak(t("stats-model-empty"));
                }
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, term) in d.terms.iter().enumerate() {
                        if ui
                            .button(format!("{}  \u{d7}", term.join("*")))
                            .on_hover_text(t("stats-model-remove"))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    }
                });
                if let Some(i) = remove {
                    d.terms.remove(i);
                }
            });
        });
        choice(
            ui,
            t("stats-model-sum-of-squares"),
            &SUM_OF_SQUARES,
            &mut d.sstype,
        );
        ui.checkbox(&mut d.intercept, t("stats-model-include-intercept"));
    });
    egui::CollapsingHeader::new(t("stats-model-contrasts")).show(ui, |ui| {
        contrasts(ui, &d.factors, &mut d.contrasts);
    });
    egui::CollapsingHeader::new(t("stats-model-plots")).show(ui, |ui| plots(ui, d));
    egui::CollapsingHeader::new(t("stats-model-post-hoc")).show(ui, |ui| {
        ui.label(t("stats-model-post-hoc-tests-for"));
        name_checkboxes(ui, &d.factors, &mut d.post_hoc_factors);
        post_hoc_tests(
            ui,
            &mut d.post_hoc,
            &mut d.dunnett,
            &mut d.dunnett_first,
            &mut d.dunnett_side,
        );
    });
    egui::CollapsingHeader::new(t("stats-model-em-means")).show(ui, |ui| {
        ui.label(t("stats-model-display-means-for"));
        ui.checkbox(&mut d.means_overall, t("stats-model-overall"));
        name_checkboxes(ui, &d.factors, &mut d.means_factors);
        ui.checkbox(&mut d.compare, t("stats-model-compare-main-effects"));
        ui.add_enabled_ui(d.compare, |ui| {
            choice(
                ui,
                t("stats-model-adjustment"),
                &GLM_ADJUSTMENTS,
                &mut d.adjustment,
            );
        });
    });
    egui::CollapsingHeader::new(t("stats-model-save")).show(ui, |ui| {
        save_groups(ui, &GLM_SAVE_GROUPS, &GLM_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new(t("stats-model-options")).show(ui, |ui| {
        if d.multivariate {
            // the M matrix is shown only when there is one
            let at = PRINT_M_MATRIX;
            checkboxes(ui, &GLM_PRINT[..at], &mut d.print[..at]);
            ui.add_enabled_ui(!d.m_matrix.is_empty(), |ui| {
                ui.checkbox(&mut d.print[at], t(GLM_PRINT[at].1));
            });
            checkboxes(ui, &GLM_PRINT[at + 1..], &mut d.print[at + 1..]);
        } else {
            checkboxes(
                ui,
                &GLM_PRINT[..UNIVARIATE_PRINT],
                &mut d.print[..UNIVARIATE_PRINT],
            );
        }
        ui.label(t("stats-model-heteroskedasticity-tests"));
        for (i, (_, label)) in GLM_HET_TESTS.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.het[i], t(label));
                if let Some(model) = d.het_model.get_mut(i) {
                    ui.add_enabled_ui(d.het[i], |ui| {
                        ComboBox::from_id_salt(("het_model", i))
                            .selected_text(t(HET_MODELS[*model].1))
                            .show_index(ui, model, HET_MODELS.len(), |k| t(HET_MODELS[k].1));
                    });
                }
            });
        }
        if !d.multivariate {
            ui.checkbox(&mut d.robust, t("stats-model-robust-estimates"));
            ui.add_enabled_ui(d.robust, |ui| {
                choice(
                    ui,
                    t("stats-model-robust-covariance"),
                    &ROBUST_ESTIMATORS,
                    &mut d.robust_estimator,
                );
            });
        }
        field(ui, t("stats-model-significance-level"), &mut d.alpha, 50.0);
    });
    if d.multivariate {
        egui::CollapsingHeader::new(t("stats-model-m-matrix")).show(ui, |ui| m_matrix(ui, d));
    }
}

/// A contrast for each factor, with the reference category where the contrast has one.
fn contrasts(ui: &mut Ui, factors: &[String], contrasts: &mut Vec<FactorContrast>) {
    if factors.is_empty() {
        ui.weak(t("stats-model-add-fixed-factor-first"));
    }
    egui::Grid::new("glm_contrasts")
        .num_columns(3)
        .show(ui, |ui| {
            for name in factors {
                let i = match contrasts.iter().position(|c| &c.name == name) {
                    Some(i) => i,
                    None => {
                        contrasts.push(FactorContrast {
                            name: name.clone(),
                            contrast: 0,
                            first: false,
                        });
                        contrasts.len() - 1
                    }
                };
                let c = &mut contrasts[i];
                ui.label(name.as_str());
                ComboBox::from_id_salt(("glm_contrast", name))
                    .width(100.0)
                    .selected_text(t(GLM_CONTRASTS[c.contrast].1))
                    .show_index(ui, &mut c.contrast, GLM_CONTRASTS.len(), |i| {
                        t(GLM_CONTRASTS[i].1)
                    });
                ui.add_enabled_ui(glm_has_reference(c.contrast), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(t("stats-model-reference"));
                        ui.radio_value(&mut c.first, false, t("stats-model-last"));
                        ui.radio_value(&mut c.first, true, t("stats-model-first"));
                    });
                });
                ui.end_row();
            }
        });
}

/// The spread-versus-level and residual plots, and the profile plots of the factors.
fn plots(ui: &mut Ui, d: &mut Univariate) {
    ui.checkbox(&mut d.spread_level, t("stats-model-spread-versus-level"));
    ui.checkbox(&mut d.residual_plots, t("stats-model-residual-plots"));
    ui.add_space(4.0);
    ui.label(t("stats-model-profile-plots"));
    d.drop_stale_profile_draft();
    // a slot is open once the one before it is filled
    let open = [
        true,
        !d.profile_draft[0].is_empty(),
        !d.profile_draft[1].is_empty(),
    ];
    for ((slot, title), open) in d
        .profile_draft
        .iter_mut()
        .zip([
            t("stats-model-horizontal-axis"),
            t("stats-model-separate-lines"),
            t("stats-model-separate-charts"),
        ])
        .zip(open)
    {
        ui.add_enabled_ui(open, |ui| {
            ComboBox::new(title, title)
                .selected_text(if slot.is_empty() {
                    t("stats-model-factor-none")
                } else {
                    slot.as_str()
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(slot, String::new(), t("stats-model-factor-none"));
                    for f in &d.factors {
                        ui.selectable_value(slot, f.clone(), f.as_str());
                    }
                });
        });
    }
    if ui
        .add_enabled(
            !d.profile_draft[0].is_empty(),
            egui::Button::new(t("stats-model-add-plot")),
        )
        .clicked()
    {
        d.add_profile();
    }
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, plot) in d.profiles.iter().enumerate() {
            if ui
                .button(format!("{}  \u{d7}", plot.join("*")))
                .on_hover_text(t("stats-model-remove"))
                .clicked()
            {
                remove = Some(i);
            }
        }
    });
    if let Some(i) = remove {
        d.profiles.remove(i);
    }
    ui.add_enabled_ui(!d.profiles.is_empty(), |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.profile_bars, false, t("stats-model-lines"));
            ui.radio_value(&mut d.profile_bars, true, t("stats-model-bars"));
        });
        choice(
            ui,
            t("stats-model-error-bars"),
            &PROFILE_ERROR_BARS,
            &mut d.profile_errors,
        );
        ui.checkbox(&mut d.profile_mean_line, t("stats-model-grand-mean-line"));
    });
}

/// The columns of the M matrix: a label and a coefficient for each dependent variable.
fn m_matrix(ui: &mut Ui, d: &mut Univariate) {
    ui.weak(t("stats-model-m-matrix-hint"));
    let mut remove = None;
    egui::Grid::new("glm_m_matrix").show(ui, |ui| {
        ui.label("");
        for (i, column) in d.m_matrix.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut column.label)
                        .hint_text(t("stats-model-label-hint"))
                        .desired_width(80.0),
                );
                if ui
                    .small_button("\u{d7}")
                    .on_hover_text(t("stats-model-remove"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        ui.end_row();
        for dependent in &d.dependents {
            ui.label(dependent.as_str());
            for column in &mut d.m_matrix {
                let cell = column.coefficients.entry(dependent.clone()).or_default();
                ui.add(TextEdit::singleline(cell).desired_width(80.0));
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        d.m_matrix.remove(i);
    }
    if ui.small_button(t("stats-model-add-column")).clicked() {
        d.m_matrix.push(MColumn::default());
    }
}

pub fn repeated_measures(ui: &mut Ui, d: &mut RepeatedMeasures, picked: &mut Vec<String>) {
    ui.strong(t("stats-model-within-factors"));
    let mut remove = None;
    let only = d.within.len() == 1;
    for (i, w) in d.within.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label(t("stats-model-name"));
                ui.add(TextEdit::singleline(&mut w.name).desired_width(120.0));
                ui.label(t("stats-model-levels"));
                ui.add(TextEdit::singleline(&mut w.levels).desired_width(50.0));
                if ui
                    .add_enabled(!only, egui::Button::new("\u{d7}").small())
                    .on_hover_text(t("stats-model-remove"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        d.within.remove(i);
    }
    if ui.small_button(t("stats-model-add-factor")).clicked() {
        let name = format!("factor{}", d.within.len() + 1);
        d.within.push(WithinFactor {
            name,
            levels: String::new(),
        });
    }
    ui.add_space(6.0);
    target_list(
        ui,
        t("stats-model-within-variables"),
        &mut d.dependents,
        picked,
    );
    target_list(ui, t("stats-model-between-factors"), &mut d.factors, picked);
    target_list(ui, t("stats-model-covariates"), &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-model-options")).show(ui, |ui| {
        choice(
            ui,
            t("stats-model-sum-of-squares"),
            &SUM_OF_SQUARES,
            &mut d.sstype,
        );
        checkboxes(ui, &GLM_PRINT[..3], &mut d.print);
        field(ui, t("stats-model-significance-level"), &mut d.alpha, 50.0);
    });
}
