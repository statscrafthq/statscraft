//! Drawing of the Power Analysis dialogs.

use crate::app::{choice, field};
use crate::i18n::{t, t_args};
use crate::power_dialog::{
    POWER_ADJUSTS, POWER_METHODS, POWER_VARIANCES, PowerAnalysis, PowerKind, needs_marginal,
    power_charts,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Checkbox, Ui};

pub fn power(ui: &mut Ui, d: &mut PowerAnalysis) {
    let anova = d.kind == PowerKind::OnewayAnova;
    let size = match (d.pairs(), anova) {
        (true, _) => t("stats-power-find-pairs"),
        (_, true) => t("stats-power-find-group-sizes"),
        _ => t("stats-power-find-sample-size"),
    };
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.find_size, false, t("stats-power-find-power"));
        ui.radio_value(&mut d.find_size, true, size);
    });
    if d.find_size {
        field(ui, t("stats-power-powers-to-reach"), &mut d.powers, 160.0);
        ui.weak(t("stats-power-powers-hint"));
        if d.two_groups() {
            field(ui, t("stats-power-size-ratio"), &mut d.ratio, 50.0);
        }
        if anova {
            field(ui, t("stats-power-relative-sizes"), &mut d.weights, 160.0);
            ui.weak(t("stats-power-weights-hint"));
        }
    } else if anova {
        field(ui, t("stats-power-group-sizes"), &mut d.n1, 160.0);
        ui.weak(t("stats-power-group-sizes-hint"));
    } else if d.two_groups() {
        ui.horizontal(|ui| {
            ui.label(t("stats-power-group-sizes"));
            small_field(ui, true, &mut d.n1);
            small_field(ui, true, &mut d.n2);
        });
    } else {
        let label = if d.pairs() {
            t("stats-power-number-of-pairs")
        } else {
            t("stats-power-sample-size")
        };
        field(ui, label, &mut d.n1, 60.0);
    }
    ui.add_space(6.0);
    match d.kind {
        PowerKind::MeansOneSample => {
            field(ui, t("stats-power-mean"), &mut d.mean1, 60.0);
            field(ui, t("stats-power-null-mean"), &mut d.null, 60.0);
            field(ui, t("stats-power-standard-deviation"), &mut d.sd1, 60.0);
        }
        PowerKind::MeansIndependent => {
            one_or_two(
                ui,
                t("stats-power-means"),
                [
                    t("stats-power-their-difference"),
                    t("stats-power-one-per-group"),
                ],
                &mut d.two_means,
                &mut d.mean1,
                &mut d.mean2,
            );
            one_or_two(
                ui,
                t("stats-power-standard-deviation"),
                [
                    t("stats-power-one-for-both"),
                    t("stats-power-one-per-group"),
                ],
                &mut d.two_sds,
                &mut d.sd1,
                &mut d.sd2,
            );
            ui.weak(t("stats-power-separate-variances-hint"));
        }
        PowerKind::MeansRelated => {
            one_or_two(
                ui,
                t("stats-power-means"),
                [
                    t("stats-power-their-difference"),
                    t("stats-power-one-per-measure"),
                ],
                &mut d.two_means,
                &mut d.mean1,
                &mut d.mean2,
            );
            one_or_two(
                ui,
                t("stats-power-standard-deviation"),
                [
                    t("stats-power-of-differences"),
                    t("stats-power-one-per-measure"),
                ],
                &mut d.two_sds,
                &mut d.sd1,
                &mut d.sd2,
            );
            ui.horizontal(|ui| {
                ui.label(t("stats-power-measures-correlation"));
                small_field(ui, d.two_sds, &mut d.rho);
            });
        }
        PowerKind::ProportionsOneSample => {
            field(ui, t("stats-power-null-proportion"), &mut d.null, 60.0);
            field(
                ui,
                t("stats-power-alternative-proportion"),
                &mut d.alternative,
                60.0,
            );
        }
        PowerKind::ProportionsIndependent => {
            ui.horizontal(|ui| {
                ui.label(t("stats-power-group-proportions"));
                small_field(ui, true, &mut d.p1);
                small_field(ui, true, &mut d.p2);
            });
        }
        PowerKind::ProportionsRelated => related(ui, d),
        PowerKind::OnewayAnova => {
            field(ui, t("stats-power-group-means"), &mut d.mean1, 160.0);
            field(ui, t("stats-power-pooled-sd"), &mut d.sd1, 60.0);
        }
        PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
            if d.kind == PowerKind::PartialCorr {
                field(ui, t("stats-power-held-fixed"), &mut d.partialout, 50.0);
            }
            field(ui, t("stats-power-null-correlation"), &mut d.null, 60.0);
            field(
                ui,
                t("stats-power-alternative-correlation"),
                &mut d.alternative,
                60.0,
            );
        }
        PowerKind::UnivariateLinear => regression(ui, d),
    }
    // the correlations and the regression need more cases than the other tests
    let least = d.least_size();
    if !d.find_size && least > 2 {
        ui.weak(t_args(
            "stats-power-least-size",
            &[("least", least.to_string().into())],
        ));
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-power-test")).show(ui, |ui| {
        if !matches!(d.kind, PowerKind::OnewayAnova | PowerKind::UnivariateLinear) {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.directional, false, t("stats-power-two-sided"));
                ui.radio_value(&mut d.directional, true, t("stats-power-one-sided"));
            });
        }
        field(ui, t("stats-power-significance"), &mut d.significance, 50.0);
        match d.kind {
            PowerKind::ProportionsIndependent => {
                choice(ui, t("stats-power-test"), &POWER_METHODS, &mut d.method);
            }
            PowerKind::PearsonOneSample => {
                ui.checkbox(&mut d.adjust_bias, t("stats-power-bias-adjustment"));
            }
            PowerKind::SpearmanOneSample => {
                choice(
                    ui,
                    t("stats-power-fisher-z-variance"),
                    &POWER_VARIANCES,
                    &mut d.variance,
                );
            }
            PowerKind::UnivariateLinear => {
                ui.horizontal(|ui| {
                    ui.label(t("stats-power-predictor-values"));
                    ui.radio_value(&mut d.random, false, t("stats-power-fixed"));
                    ui.radio_value(&mut d.random, true, t("stats-power-random"));
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
                ui.label(t("stats-power-power-from"));
                ui.radio_value(
                    &mut d.binomial,
                    false,
                    t("stats-power-normal-approximation"),
                );
                ui.radio_value(&mut d.binomial, true, t("stats-power-exact-binomial"));
            });
            ui.weak(t("stats-power-exact-hint"));
        }
        if d.has_continuity() {
            ui.checkbox(&mut d.continuity, t("stats-power-continuity"));
        }
        if d.has_pooled() {
            ui.checkbox(&mut d.pooled, t("stats-power-pooled-se"));
        }
        if d.proportions() {
            ui.horizontal(|ui| {
                ui.label(t("stats-power-timer"));
                small_field(ui, d.has_timer(), &mut d.timer);
            });
        }
    });
    if anova {
        egui::CollapsingHeader::new(t("stats-power-contrasts")).show(ui, |ui| {
            field(
                ui,
                t("stats-power-coefficients"),
                &mut d.coefficients,
                160.0,
            );
            ui.weak(t("stats-power-coefficients-hint"));
            ui.add_enabled_ui(!d.coefficients.trim().is_empty(), |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.directional, false, t("stats-power-two-sided"));
                    ui.radio_value(&mut d.directional, true, t("stats-power-one-sided"));
                });
            });
            ui.checkbox(&mut d.pairwise, t("stats-power-pairwise"));
            ui.add_enabled_ui(d.pairwise, |ui| {
                choice(
                    ui,
                    t("stats-power-adjustment"),
                    &POWER_ADJUSTS,
                    &mut d.adjust,
                );
            });
        });
    }
    egui::CollapsingHeader::new(t("stats-power-charts")).show(ui, |ui| {
        for (i, &(keywords, label, ranged)) in power_charts(d.kind).iter().enumerate() {
            let offered = !needs_marginal(d.kind, keywords) || d.marginal;
            ui.horizontal(|ui| {
                ui.add_enabled(offered, Checkbox::new(&mut d.charts[i], t(label)));
                if ranged {
                    let on = offered && d.charts[i];
                    ui.label(t("stats-power-from"));
                    small_field(ui, on, &mut d.from[i]);
                    ui.label(t("stats-power-to"));
                    small_field(ui, on, &mut d.to[i]);
                }
            });
        }
        ui.weak(t("stats-power-charts-hint"));
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
    field(
        ui,
        t("stats-power-predictors-in-model"),
        &mut d.total_predictors,
        50.0,
    );
    field(
        ui,
        t("stats-power-predictors-tested"),
        &mut d.test_predictors,
        50.0,
    );
    ui.horizontal(|ui| {
        ui.label(t("stats-power-effect"));
        ui.radio_value(&mut d.models, false, t("stats-power-partial-correlation"));
        ui.radio_value(&mut d.models, true, t("stats-power-two-models"));
    });
    if d.models {
        ui.horizontal(|ui| {
            ui.label(t("stats-power-r-squared-with-without"));
            small_field(ui, true, &mut d.p1);
            small_field(ui, true, &mut d.p2);
        });
    } else {
        field(
            ui,
            t("stats-power-partial-correlation"),
            &mut d.partial,
            60.0,
        );
    }
    ui.checkbox(&mut d.intercept, t("stats-power-intercept"));
}

/// The values of related proportions: shares of discordant pairs, or success rates with their
/// correlation; counts only with a given number of pairs.
fn related(ui: &mut Ui, d: &mut PowerAnalysis) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.marginal, false, t("stats-power-discordant-pairs"));
        ui.radio_value(&mut d.marginal, true, t("stats-power-success-rates"));
    });
    ui.add_enabled_ui(!d.find_size, |ui| {
        ui.horizontal(|ui| {
            ui.label(t("stats-power-given-as"));
            ui.radio_value(&mut d.counts, false, t("stats-power-proportions"));
            ui.radio_value(&mut d.counts, true, t("stats-power-counts-of-pairs"));
        });
    });
    let label = match d.marginal {
        true => t("stats-power-occasions"),
        false => t("stats-power-discordant-order"),
    };
    ui.horizontal(|ui| {
        ui.label(label);
        small_field(ui, true, &mut d.p1);
        small_field(ui, true, &mut d.p2);
    });
    ui.horizontal(|ui| {
        ui.label(t("stats-power-occasions-correlation"));
        small_field(ui, d.marginal, &mut d.correlation);
    });
    if d.find_size {
        ui.weak(t("stats-power-counts-hint"));
    }
}
