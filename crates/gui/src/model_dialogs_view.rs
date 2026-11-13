//! Drawing of the Factor Analysis, K-Means Cluster, Hierarchical Cluster, Discriminant, Binary
//! Logistic Regression, Univariate, Multivariate and Repeated Measures dialogs.

use crate::analyze_dialogs::{DUNNETT_SIDES, EQUAL_VARIANCE_TESTS, POST_HOC_TESTS};
use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::model_dialogs::{
    CLUSTER_BINARY, CLUSTER_COUNTS, CLUSTER_DATA, CLUSTER_INTERVAL, CLUSTER_METHODS, Categorical,
    DISCRIMINANT_METHODS, DISCRIMINANT_SAVE, DISCRIMINANT_STATISTICS,
    DISCRIMINANT_STATISTICS_GROUPS, Discriminant, FACTOR_DESCRIPTIVES, FACTOR_EXTRACTIONS,
    FACTOR_MISSING, FACTOR_ROTATIONS, FACTOR_SCORES, FactorAnalysis, GLM_ADJUSTMENTS,
    GLM_CONTRASTS, GLM_PRINT, GLM_SAVE, GLM_SAVE_GROUPS, Hierarchical, KMeans, LOGISTIC_CONTRASTS,
    LOGISTIC_METHODS, LOGISTIC_SAVE, LOGISTIC_SAVE_GROUPS, Logistic, MColumn, PROFILE_ERROR_BARS,
    RepeatedMeasures, SUM_OF_SQUARES, Solutions, UNIVARIATE_PRINT, Univariate, WithinFactor,
    has_reference,
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
    ui.weak("Equal variances assumed");
    checkboxes(ui, equal, equal_flags);
    ui.horizontal(|ui| {
        ui.checkbox(dunnett, "Dunnett, control group");
        ui.add_enabled_ui(*dunnett, |ui| {
            ui.radio_value(dunnett_first, true, "first");
            ui.radio_value(dunnett_first, false, "last");
            ComboBox::new("dunnett_side", "")
                .selected_text(DUNNETT_SIDES[*dunnett_side].1)
                .show_index(ui, dunnett_side, DUNNETT_SIDES.len(), |i| {
                    DUNNETT_SIDES[i].1
                });
        });
    });
    ui.weak("Equal variances not assumed");
    checkboxes(ui, unequal, unequal_flags);
}

/// The checkboxes of saved variables in their groups.
fn save_groups(
    ui: &mut Ui,
    groups: &[(&str, std::ops::Range<usize>)],
    options: &[(&str, &str)],
    flags: &mut [bool],
) {
    for (title, range) in groups {
        ui.label(*title);
        checkboxes(ui, &options[range.clone()], &mut flags[range.clone()]);
    }
}

pub fn factor(ui: &mut Ui, d: &mut FactorAnalysis, picked: &mut Vec<String>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Descriptives").show(ui, |ui| {
        checkboxes(ui, &FACTOR_DESCRIPTIVES, &mut d.descriptives);
    });
    egui::CollapsingHeader::new("Extraction").show(ui, |ui| {
        choice(ui, "Method", &FACTOR_EXTRACTIONS, &mut d.extraction);
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.fixed, false, "Eigenvalues over");
            small_field(ui, !d.fixed, &mut d.min_eigen);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.fixed, true, "Fixed number of factors");
            small_field(ui, d.fixed, &mut d.factors);
        });
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
    });
    egui::CollapsingHeader::new("Rotation").show(ui, |ui| {
        choice(ui, "Method", &FACTOR_ROTATIONS, &mut d.rotation);
        match FACTOR_ROTATIONS[d.rotation].0 {
            "OBLIMIN" => field(ui, "Delta", &mut d.delta, 50.0),
            "PROMAX" => field(ui, "Kappa", &mut d.kappa, 50.0),
            _ => {}
        }
    });
    egui::CollapsingHeader::new("Scores").show(ui, |ui| {
        ui.checkbox(&mut d.save, "Save as variables");
        ui.add_enabled_ui(d.save, |ui| {
            choice(ui, "Method", &FACTOR_SCORES, &mut d.score_method);
        });
        ui.checkbox(
            &mut d.score_coefficients,
            "Display factor score coefficient matrix",
        );
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.label("Missing values");
        for (i, (_, label)) in FACTOR_MISSING.iter().enumerate() {
            ui.radio_value(&mut d.missing, i, *label);
        }
        ui.label("Coefficient display");
        ui.checkbox(&mut d.sort, "Sorted by size");
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.blank, "Suppress absolute values below");
            small_field(ui, d.blank, &mut d.blank_value);
        });
    });
}

pub fn k_means(ui: &mut Ui, d: &mut KMeans, picked: &mut Vec<String>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    field(ui, "Number of clusters", &mut d.clusters, 50.0);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.strong("Method");
        ui.radio_value(&mut d.classify_only, false, "Iterate and classify");
        ui.radio_value(&mut d.classify_only, true, "Classify only");
    });
    ui.add_enabled_ui(!d.classify_only, |ui| {
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        field(ui, "Convergence criterion", &mut d.converge, 50.0);
    });
    ui.add_space(6.0);
    ui.strong("Save");
    ui.checkbox(&mut d.save_cluster, "Cluster membership");
    ui.checkbox(&mut d.save_distance, "Distance from cluster center");
    ui.add_space(6.0);
    ui.strong("Statistics");
    ui.checkbox(&mut d.initial, "Initial cluster centers");
    ui.checkbox(&mut d.anova, "ANOVA table");
    ui.checkbox(&mut d.membership, "Cluster information for each case");
    ui.add_enabled_ui(d.membership, |ui| {
        target_one(ui, "Label cases by", &mut d.label, picked);
    });
    ui.add_space(6.0);
    ui.strong("Missing values");
    ui.radio_value(
        &mut d.pairwise,
        false,
        "Leave out a case missing on any variable",
    );
    ui.radio_value(
        &mut d.pairwise,
        true,
        "Leave out a case only from the distances where its value is missing",
    );
}

/// No cluster membership, one solution or a range of them.
fn solutions(ui: &mut Ui, title: &str, s: &mut Solutions) {
    ui.push_id(title, |ui| {
        ui.label(title);
        ui.radio_value(&mut s.kind, 0, "None");
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.kind, 1, "Single solution, clusters");
            small_field(ui, s.kind == 1, &mut s.clusters);
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.kind, 2, "Range of solutions, from");
            small_field(ui, s.kind == 2, &mut s.min);
            ui.label("to");
            small_field(ui, s.kind == 2, &mut s.max);
        });
    });
}

pub fn hierarchical(ui: &mut Ui, d: &mut Hierarchical, picked: &mut Vec<String>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    target_one(ui, "Label cases by", &mut d.label, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.checkbox(&mut d.schedule, "Agglomeration schedule");
        ui.checkbox(&mut d.proximity, "Proximity matrix");
        solutions(ui, "Cluster membership", &mut d.membership);
    });
    egui::CollapsingHeader::new("Method").show(ui, |ui| {
        choice(ui, "Cluster method", &CLUSTER_METHODS, &mut d.method);
        ui.horizontal(|ui| {
            ui.label("Measure for");
            for (i, kind) in CLUSTER_DATA.iter().enumerate() {
                ui.radio_value(&mut d.data, i, *kind);
            }
        });
        match d.data {
            0 => {
                choice(ui, "Measure", &CLUSTER_INTERVAL, &mut d.interval);
                match CLUSTER_INTERVAL[d.interval].0 {
                    "MINKOWSKI" => field(ui, "Power", &mut d.power, 50.0),
                    "POWER" => {
                        ui.horizontal(|ui| {
                            ui.label("Power");
                            small_field(ui, true, &mut d.power);
                            ui.label("Root");
                            small_field(ui, true, &mut d.root);
                        })
                        .inner
                    }
                    _ => {}
                }
            }
            1 => choice(ui, "Measure", &CLUSTER_COUNTS, &mut d.counts),
            _ => {
                choice(ui, "Measure", &CLUSTER_BINARY, &mut d.binary);
                ui.horizontal(|ui| {
                    ui.label("Present");
                    small_field(ui, true, &mut d.present);
                    ui.label("Absent");
                    small_field(ui, true, &mut d.absent);
                });
            }
        }
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        solutions(ui, "Cluster membership", &mut d.save);
    });
}

pub fn discriminant(ui: &mut Ui, d: &mut Discriminant, picked: &mut Vec<String>) {
    target_one(ui, "Grouping variable", &mut d.group, picked);
    ui.horizontal(|ui| {
        ui.label("Range from");
        small_field(ui, true, &mut d.min);
        ui.label("to");
        small_field(ui, true, &mut d.max);
    });
    target_list(ui, "Independents", &mut d.variables, picked);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.stepwise, false, "Enter independents together");
        ui.radio_value(&mut d.stepwise, true, "Use stepwise method");
    });
    target_one(ui, "Selection variable", &mut d.select, picked);
    ui.add_enabled_ui(!d.select.is_empty(), |ui| {
        field(ui, "Value", &mut d.select_value, 50.0);
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        save_groups(
            ui,
            &DISCRIMINANT_STATISTICS_GROUPS,
            &DISCRIMINANT_STATISTICS,
            &mut d.statistics,
        );
    });
    ui.add_enabled_ui(d.stepwise, |ui| {
        egui::CollapsingHeader::new("Method").show(ui, |ui| {
            choice(ui, "Method", &DISCRIMINANT_METHODS, &mut d.method);
            ui.horizontal(|ui| {
                ui.label("Criteria");
                let mut use_f = d.use_f;
                ui.radio_value(&mut use_f, true, "F value");
                ui.radio_value(&mut use_f, false, "Probability of F");
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
        });
    });
    egui::CollapsingHeader::new("Classify").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Prior probabilities");
            ui.radio_value(&mut d.priors_size, false, "All groups equal");
            ui.radio_value(&mut d.priors_size, true, "From group sizes");
        });
        ui.horizontal(|ui| {
            ui.label("Covariance matrix");
            ui.radio_value(&mut d.separate, false, "Within-groups");
            ui.radio_value(&mut d.separate, true, "Separate-groups");
        });
        ui.checkbox(&mut d.table, "Summary table");
        ui.checkbox(&mut d.leave_one_out, "Leave-one-out classification");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for ((_, label), on) in DISCRIMINANT_SAVE.iter().zip(&mut d.save) {
            ui.checkbox(on, *label);
        }
    });
}

pub fn logistic(ui: &mut Ui, d: &mut Logistic, picked: &mut Vec<String>) {
    target_one(ui, "Dependent variable", &mut d.dependent, picked);
    ui.add_space(6.0);
    let block = blocks(ui, &mut d.blocks, &mut d.block);
    target_list(ui, "Covariates", &mut block.variables, picked);
    choice(ui, "Method", &LOGISTIC_METHODS, &mut block.method);
    ui.add_space(6.0);
    let mut covariates: Vec<String> = Vec::new();
    for name in d.blocks.iter().flat_map(|b| &b.variables) {
        if !covariates.contains(name) {
            covariates.push(name.clone());
        }
    }
    egui::CollapsingHeader::new("Categorical").show(ui, |ui| {
        egui::Grid::new("categorical")
            .num_columns(3)
            .show(ui, |ui| {
                for name in &covariates {
                    let at = d.categorical.iter().position(|c| &c.name == name);
                    let mut on = at.is_some();
                    if ui.checkbox(&mut on, name.as_str()).changed() {
                        match at {
                            Some(i) => {
                                d.categorical.remove(i);
                            }
                            None => d.categorical.push(Categorical {
                                name: name.clone(),
                                contrast: 0,
                                first: false,
                            }),
                        }
                    }
                    if let Some(c) = d.categorical.iter_mut().find(|c| &c.name == name) {
                        ComboBox::from_id_salt(("contrast", name))
                            .width(90.0)
                            .selected_text(LOGISTIC_CONTRASTS[c.contrast].1)
                            .show_index(ui, &mut c.contrast, LOGISTIC_CONTRASTS.len(), |i| {
                                LOGISTIC_CONTRASTS[i].1
                            });
                        ui.add_enabled_ui(has_reference(c.contrast), |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Reference");
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.radio_value(&mut c.first, false, "last");
                                ui.radio_value(&mut c.first, true, "first");
                            });
                        });
                    }
                    ui.end_row();
                }
            });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_groups(ui, &LOGISTIC_SAVE_GROUPS, &LOGISTIC_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.goodfit, "Hosmer-Lemeshow goodness of fit");
        ui.checkbox(&mut d.iteration_history, "Iteration history");
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ci, "Confidence interval for exp(B), level %");
            small_field(ui, d.ci, &mut d.ci_level);
        });
        field(ui, "Classification cutoff", &mut d.cut, 50.0);
        ui.horizontal(|ui| {
            ui.label("Probability for stepwise entry");
            small_field(ui, true, &mut d.entry);
            ui.label("removal");
            small_field(ui, true, &mut d.removal);
        });
        field(ui, "Maximum iterations", &mut d.iterations, 50.0);
        ui.checkbox(&mut d.constant, "Include the constant in the model");
    });
}

pub fn univariate(ui: &mut Ui, d: &mut Univariate, picked: &mut Vec<String>) {
    if d.multivariate {
        target_list(ui, "Dependent variables", &mut d.dependents, picked);
    } else {
        target_one(ui, "Dependent variable", &mut d.dependent, picked);
    }
    target_list(ui, "Fixed factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.custom, false, "Full factorial");
            ui.radio_value(&mut d.custom, true, "Custom");
        });
        ui.add_enabled_ui(d.custom, |ui| {
            let model = d.model();
            name_checkboxes(ui, &model, &mut d.building);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!d.building.is_empty(), egui::Button::new("Main effects"))
                    .clicked()
                {
                    d.add_terms(false);
                }
                if ui
                    .add_enabled(d.building.len() >= 2, egui::Button::new("Interaction"))
                    .clicked()
                {
                    d.add_terms(true);
                }
            });
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(260.0);
                if d.terms.is_empty() {
                    ui.weak("empty");
                }
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, term) in d.terms.iter().enumerate() {
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
                    d.terms.remove(i);
                }
            });
        });
        choice(ui, "Sum of squares", &SUM_OF_SQUARES, &mut d.sstype);
        ui.checkbox(&mut d.intercept, "Include intercept in the model");
    });
    egui::CollapsingHeader::new("Contrasts").show(ui, |ui| {
        contrasts(ui, &d.factors, &mut d.contrasts);
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| plots(ui, d));
    egui::CollapsingHeader::new("Post hoc").show(ui, |ui| {
        ui.label("Post hoc tests for");
        name_checkboxes(ui, &d.factors, &mut d.post_hoc_factors);
        post_hoc_tests(
            ui,
            &mut d.post_hoc,
            &mut d.dunnett,
            &mut d.dunnett_first,
            &mut d.dunnett_side,
        );
    });
    egui::CollapsingHeader::new("EM means").show(ui, |ui| {
        ui.label("Display means for");
        ui.checkbox(&mut d.means_overall, "Overall");
        name_checkboxes(ui, &d.factors, &mut d.means_factors);
        ui.checkbox(&mut d.compare, "Compare main effects");
        ui.add_enabled_ui(d.compare, |ui| {
            choice(ui, "Adjustment", &GLM_ADJUSTMENTS, &mut d.adjustment);
        });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_groups(ui, &GLM_SAVE_GROUPS, &GLM_SAVE, &mut d.save);
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        let offered = if d.multivariate {
            GLM_PRINT.len()
        } else {
            UNIVARIATE_PRINT
        };
        checkboxes(ui, &GLM_PRINT[..offered], &mut d.print[..offered]);
        field(ui, "Significance level", &mut d.alpha, 50.0);
    });
    if d.multivariate {
        egui::CollapsingHeader::new("M matrix").show(ui, |ui| m_matrix(ui, d));
    }
}

/// A contrast for each factor, with the reference category where the contrast has one.
fn contrasts(ui: &mut Ui, factors: &[String], contrasts: &mut Vec<Categorical>) {
    if factors.is_empty() {
        ui.weak("Add a fixed factor first");
    }
    egui::Grid::new("glm_contrasts")
        .num_columns(3)
        .show(ui, |ui| {
            for name in factors {
                let i = match contrasts.iter().position(|c| &c.name == name) {
                    Some(i) => i,
                    None => {
                        contrasts.push(Categorical {
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
                    .selected_text(GLM_CONTRASTS[c.contrast].1)
                    .show_index(ui, &mut c.contrast, GLM_CONTRASTS.len(), |i| {
                        GLM_CONTRASTS[i].1
                    });
                ui.add_enabled_ui(
                    matches!(GLM_CONTRASTS[c.contrast].0, "DEVIATION" | "SIMPLE"),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Reference");
                            ui.radio_value(&mut c.first, false, "last");
                            ui.radio_value(&mut c.first, true, "first");
                        });
                    },
                );
                ui.end_row();
            }
        });
}

/// The spread-versus-level and residual plots, and the profile plots of the factors.
fn plots(ui: &mut Ui, d: &mut Univariate) {
    ui.checkbox(&mut d.spread_level, "Spread versus level");
    ui.checkbox(
        &mut d.residual_plots,
        "Residuals against observed and predicted",
    );
    ui.add_space(4.0);
    ui.label("Profile plots of estimated marginal means");
    for (slot, title) in
        d.profile_draft
            .iter_mut()
            .zip(["Horizontal axis", "Separate lines", "Separate charts"])
    {
        ComboBox::new(title, title)
            .selected_text(if slot.is_empty() {
                "None"
            } else {
                slot.as_str()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(slot, String::new(), "None");
                for f in &d.factors {
                    ui.selectable_value(slot, f.clone(), f.as_str());
                }
            });
    }
    if ui
        .add_enabled(
            !d.profile_draft[0].is_empty(),
            egui::Button::new("Add plot"),
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
                .on_hover_text("Remove")
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
            ui.radio_value(&mut d.profile_bars, false, "Lines");
            ui.radio_value(&mut d.profile_bars, true, "Bars");
        });
        choice(ui, "Error bars", &PROFILE_ERROR_BARS, &mut d.profile_errors);
        ui.checkbox(&mut d.profile_mean_line, "Reference line at the grand mean");
    });
}

/// The columns of the M matrix: a label and a coefficient for each dependent variable.
fn m_matrix(ui: &mut Ui, d: &mut Univariate) {
    ui.weak("Transforms the dependent variables in the test of the intercept; an empty cell is 0.");
    let mut remove = None;
    egui::Grid::new("glm_m_matrix").show(ui, |ui| {
        ui.label("");
        for (i, column) in d.m_matrix.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut column.label)
                        .hint_text("label")
                        .desired_width(80.0),
                );
                if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
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
    if ui.small_button("Add column").clicked() {
        d.m_matrix.push(MColumn::default());
    }
}

pub fn repeated_measures(ui: &mut Ui, d: &mut RepeatedMeasures, picked: &mut Vec<String>) {
    ui.strong("Within-subjects factors");
    let mut remove = None;
    let only = d.within.len() == 1;
    for (i, w) in d.within.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(TextEdit::singleline(&mut w.name).desired_width(120.0));
                ui.label("Levels");
                ui.add(TextEdit::singleline(&mut w.levels).desired_width(50.0));
                if ui
                    .add_enabled(!only, egui::Button::new("\u{d7}").small())
                    .on_hover_text("Remove")
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
    if ui.small_button("Add factor").clicked() {
        let name = format!("factor{}", d.within.len() + 1);
        d.within.push(WithinFactor {
            name,
            levels: String::new(),
        });
    }
    ui.add_space(6.0);
    target_list(ui, "Within-subjects variables", &mut d.dependents, picked);
    target_list(ui, "Between-subjects factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        choice(ui, "Sum of squares", &SUM_OF_SQUARES, &mut d.sstype);
        checkboxes(ui, &GLM_PRINT[..3], &mut d.print);
        field(ui, "Significance level", &mut d.alpha, 50.0);
    });
}
