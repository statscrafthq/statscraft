//! Drawing of the Power Analysis dialogs.

use crate::app::{choice, field};
use crate::power_dialog::{
    POWER_ADJUSTS, POWER_METHODS, POWER_VARIANCES, PowerAnalysis, PowerKind, needs_marginal,
    power_charts,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Checkbox, Ui};

pub fn power(ui: &mut Ui, d: &mut PowerAnalysis) {
    let anova = d.kind == PowerKind::OnewayAnova;
    let size = match (d.pairs(), anova) {
        (true, _) => "Find the number of pairs",
        (_, true) => "Find the group sizes",
        _ => "Find the sample size",
    };
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.find_size, false, "Find the power");
        ui.radio_value(&mut d.find_size, true, size);
    });
    if d.find_size {
        field(ui, "Powers to reach", &mut d.powers, 160.0);
        ui.weak("One or more, each above 0 and below 1, separated by spaces");
        if d.two_groups() {
            field(ui, "Size of group 2 / size of group 1", &mut d.ratio, 50.0);
        }
        if anova {
            field(ui, "Relative group sizes", &mut d.weights, 160.0);
            ui.weak("One weight from 1 to 100 per group; empty for groups of equal size");
        }
    } else if anova {
        field(ui, "Group sizes", &mut d.n1, 160.0);
        ui.weak("One whole number of at least 2 per group, in the order of the means");
    } else if d.two_groups() {
        ui.horizontal(|ui| {
            ui.label("Group sizes");
            small_field(ui, true, &mut d.n1);
            small_field(ui, true, &mut d.n2);
        });
    } else {
        let label = if d.pairs() {
            "Number of pairs"
        } else {
            "Sample size"
        };
        field(ui, label, &mut d.n1, 60.0);
    }
    ui.add_space(6.0);
    match d.kind {
        PowerKind::MeansOneSample => {
            field(ui, "Mean", &mut d.mean1, 60.0);
            field(ui, "Null hypothesis mean", &mut d.null, 60.0);
            field(ui, "Standard deviation", &mut d.sd1, 60.0);
        }
        PowerKind::MeansIndependent => {
            one_or_two(
                ui,
                "Means",
                ["Their difference", "One per group"],
                &mut d.two_means,
                &mut d.mean1,
                &mut d.mean2,
            );
            one_or_two(
                ui,
                "Standard deviation",
                ["One for both groups", "One per group"],
                &mut d.two_sds,
                &mut d.sd1,
                &mut d.sd2,
            );
            ui.weak("Two different standard deviations give the test with separate variances");
        }
        PowerKind::MeansRelated => {
            one_or_two(
                ui,
                "Means",
                ["Their difference", "One per measure"],
                &mut d.two_means,
                &mut d.mean1,
                &mut d.mean2,
            );
            one_or_two(
                ui,
                "Standard deviation",
                ["Of the differences", "One per measure"],
                &mut d.two_sds,
                &mut d.sd1,
                &mut d.sd2,
            );
            ui.horizontal(|ui| {
                ui.label("Correlation of the measures");
                small_field(ui, d.two_sds, &mut d.rho);
            });
        }
        PowerKind::ProportionsOneSample => {
            field(ui, "Null hypothesis proportion", &mut d.null, 60.0);
            field(ui, "Alternative proportion", &mut d.alternative, 60.0);
        }
        PowerKind::ProportionsIndependent => {
            ui.horizontal(|ui| {
                ui.label("Proportions of group 1 and group 2");
                small_field(ui, true, &mut d.p1);
                small_field(ui, true, &mut d.p2);
            });
        }
        PowerKind::ProportionsRelated => related(ui, d),
        PowerKind::OnewayAnova => {
            field(ui, "Group means", &mut d.mean1, 160.0);
            field(ui, "Pooled standard deviation", &mut d.sd1, 60.0);
        }
        PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
            if d.kind == PowerKind::PartialCorr {
                field(ui, "Variables held fixed", &mut d.partialout, 50.0);
            }
            field(ui, "Null hypothesis correlation", &mut d.null, 60.0);
            field(ui, "Alternative correlation", &mut d.alternative, 60.0);
        }
        PowerKind::UnivariateLinear => regression(ui, d),
    }
    // the correlations and the regression need more cases than the other tests
    let least = d.least_size();
    if !d.find_size && least > 2 {
        ui.weak(format!("The sample size must be at least {least}"));
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Test").show(ui, |ui| {
        if !matches!(d.kind, PowerKind::OnewayAnova | PowerKind::UnivariateLinear) {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.directional, false, "Two-sided");
                ui.radio_value(&mut d.directional, true, "One-sided");
            });
        }
        field(ui, "Significance level", &mut d.significance, 50.0);
        match d.kind {
            PowerKind::ProportionsIndependent => {
                choice(ui, "Test", &POWER_METHODS, &mut d.method);
            }
            PowerKind::PearsonOneSample => {
                ui.checkbox(&mut d.adjust_bias, "Small-sample bias adjustment");
            }
            PowerKind::SpearmanOneSample => {
                choice(
                    ui,
                    "Variance of Fisher's z",
                    &POWER_VARIANCES,
                    &mut d.variance,
                );
            }
            PowerKind::UnivariateLinear => {
                ui.horizontal(|ui| {
                    ui.label("Predictor values");
                    ui.radio_value(&mut d.random, false, "Fixed");
                    ui.radio_value(&mut d.random, true, "Random (normal with the outcome)");
                });
            }
            PowerKind::MeansOneSample
            | PowerKind::MeansIndependent
            | PowerKind::MeansRelated
            | PowerKind::ProportionsOneSample
            | PowerKind::ProportionsRelated
            | PowerKind::OnewayAnova
            | PowerKind::PartialCorr => {}
        }
        if d.proportions() {
            ui.horizontal(|ui| {
                ui.label("Power from");
                ui.radio_value(&mut d.binomial, false, "The normal approximation");
                ui.radio_value(&mut d.binomial, true, "Exact binomial counts");
            });
            ui.weak("Exact counts go up to 500 trials, beyond that the approximation is used");
        }
        if d.has_continuity() {
            ui.checkbox(&mut d.continuity, "Continuity correction");
        }
        if d.has_pooled() {
            ui.checkbox(&mut d.pooled, "Pooled standard error");
        }
        if d.proportions() {
            ui.horizontal(|ui| {
                ui.label("Stop the exact search after minutes (empty: never)");
                small_field(ui, d.has_timer(), &mut d.timer);
            });
        }
    });
    if anova {
        egui::CollapsingHeader::new("Contrasts").show(ui, |ui| {
            field(ui, "Contrast coefficients", &mut d.coefficients, 160.0);
            ui.weak("One per group, empty for none; the last is changed if they do not sum to 0");
            ui.add_enabled_ui(!d.coefficients.trim().is_empty(), |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.directional, false, "Two-sided");
                    ui.radio_value(&mut d.directional, true, "One-sided");
                });
            });
            ui.checkbox(&mut d.pairwise, "Compare every pair of groups");
            ui.add_enabled_ui(d.pairwise, |ui| {
                choice(ui, "Adjustment", &POWER_ADJUSTS, &mut d.adjust);
            });
        });
    }
    egui::CollapsingHeader::new("Charts").show(ui, |ui| {
        for (i, &(keywords, label, ranged)) in power_charts(d.kind).iter().enumerate() {
            let offered = !needs_marginal(d.kind, keywords) || d.marginal;
            ui.horizontal(|ui| {
                ui.add_enabled(offered, Checkbox::new(&mut d.charts[i], label));
                if ranged {
                    let on = offered && d.charts[i];
                    ui.label("from");
                    small_field(ui, on, &mut d.from[i]);
                    ui.label("to");
                    small_field(ui, on, &mut d.to[i]);
                }
            });
        }
        ui.weak(
            "Empty ends take the default range. With several powers only the size chart is drawn",
        );
    });
}

/// One value or one per group or measure, with the radios that choose.
fn one_or_two(
    ui: &mut Ui,
    label: &str,
    choices: [&str; 2],
    two: &mut bool,
    first: &mut String,
    second: &mut String,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.radio_value(two, false, choices[0]);
        ui.radio_value(two, true, choices[1]);
    });
    ui.horizontal(|ui| {
        small_field(ui, true, first);
        if *two {
            small_field(ui, true, second);
        }
    });
}

/// The predictors of a linear regression and the effect of those tested: a partial correlation,
/// or the R squared of the model with and without them.
fn regression(ui: &mut Ui, d: &mut PowerAnalysis) {
    field(ui, "Predictors in the model", &mut d.total_predictors, 50.0);
    field(ui, "Predictors tested", &mut d.test_predictors, 50.0);
    ui.horizontal(|ui| {
        ui.label("Effect");
        ui.radio_value(&mut d.models, false, "Partial correlation");
        ui.radio_value(&mut d.models, true, "R squared of two models");
    });
    if d.models {
        ui.horizontal(|ui| {
            ui.label("R squared with and without the tested predictors");
            small_field(ui, true, &mut d.p1);
            small_field(ui, true, &mut d.p2);
        });
    } else {
        field(ui, "Partial correlation", &mut d.partial, 60.0);
    }
    ui.checkbox(&mut d.intercept, "The model has an intercept");
}

/// The values of related proportions: shares of discordant pairs, or success rates with their
/// correlation; counts only with a given number of pairs.
fn related(ui: &mut Ui, d: &mut PowerAnalysis) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.marginal, false, "Discordant pairs");
        ui.radio_value(&mut d.marginal, true, "Success rates");
    });
    ui.add_enabled_ui(!d.find_size, |ui| {
        ui.horizontal(|ui| {
            ui.label("Given as");
            ui.radio_value(&mut d.counts, false, "Proportions");
            ui.radio_value(&mut d.counts, true, "Counts of pairs");
        });
    });
    let label = match d.marginal {
        true => "First and second occasion",
        false => "Success then failure, failure then success",
    };
    ui.horizontal(|ui| {
        ui.label(label);
        small_field(ui, true, &mut d.p1);
        small_field(ui, true, &mut d.p2);
    });
    ui.horizontal(|ui| {
        ui.label("Correlation of the occasions");
        small_field(ui, d.marginal, &mut d.correlation);
    });
    if d.find_size {
        ui.weak("Counts need a given number of pairs; the values are proportions");
    }
}
