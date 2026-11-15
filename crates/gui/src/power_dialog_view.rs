//! Drawing of the Power Analysis dialogs.

use crate::app::{choice, field};
use crate::power_dialog::{POWER_METHODS, PowerAnalysis, PowerKind, needs_marginal, power_charts};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Checkbox, Ui};

pub fn power(ui: &mut Ui, d: &mut PowerAnalysis) {
    let size = match d.pairs() {
        true => "Find the number of pairs",
        false => "Find the sample size",
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
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Test").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.directional, false, "Two-sided");
            ui.radio_value(&mut d.directional, true, "One-sided");
        });
        field(ui, "Significance level", &mut d.significance, 50.0);
        if d.kind == PowerKind::ProportionsIndependent {
            choice(ui, "Test", &POWER_METHODS, &mut d.method);
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
