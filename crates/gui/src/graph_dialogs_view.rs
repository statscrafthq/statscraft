//! Drawing of the Graphs menu's dialogs: Chart Builder, P-P and Q-Q Plots and ROC Curve.

use crate::app::{choice, field, target_list, target_one};
use crate::graph_dialogs::{
    COUNT_FUNCTIONS, ChartBuilder, ChartKind, ERROR_BARS, GRAPH_FUNCTIONS, PPLOT_DISTRIBUTIONS,
    PPLOT_FRACTIONS, PPLOT_PLOTS, PPLOT_TIES, ProbabilityPlot, RocCurve,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};

pub fn chart_builder(ui: &mut Ui, d: &mut ChartBuilder, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.label("Chart type");
        ComboBox::new("chart_kind", "")
            .selected_text(d.kind.label())
            .show_ui(ui, |ui| {
                for kind in ChartKind::ALL {
                    ui.selectable_value(&mut d.kind, kind, kind.label());
                }
            });
    });
    ui.add_space(6.0);
    let kind = d.kind;
    if kind.summarizes() {
        let functions = kind.functions();
        if !functions.contains(&d.function) {
            d.function = functions[0];
        }
        ui.horizontal(|ui| {
            ui.label("Bars, points or slices show");
            ComboBox::new("graph_function", "")
                .selected_text(GRAPH_FUNCTIONS[d.function].1)
                .show_ui(ui, |ui| {
                    for i in functions {
                        ui.selectable_value(&mut d.function, i, GRAPH_FUNCTIONS[i].1);
                    }
                });
        });
        match GRAPH_FUNCTIONS[d.function] {
            ("PTILE", ..) => field(ui, "Percentile", &mut d.low, 50.0),
            (_, _, 1) => field(ui, "Value", &mut d.low, 50.0),
            (_, _, 2) => {
                ui.horizontal(|ui| {
                    ui.label("From");
                    small_field(ui, true, &mut d.low);
                    ui.label("to");
                    small_field(ui, true, &mut d.high);
                });
            }
            _ => {}
        }
        if d.function >= COUNT_FUNCTIONS {
            target_list(ui, "Variables", &mut d.variables, picked);
        }
        let title = match kind {
            ChartKind::Pie => "Slices by",
            _ => "Category axis",
        };
        target_one(ui, title, &mut d.category, picked);
    }
    match kind {
        ChartKind::Histogram => {
            target_one(ui, "Variable", &mut d.x, picked);
            ui.checkbox(&mut d.normal, "Normal curve");
        }
        ChartKind::SimpleScatter | ChartKind::GroupedScatter => {
            target_one(ui, "X axis", &mut d.x, picked);
            target_one(ui, "Y axis", &mut d.y, picked);
        }
        ChartKind::OverlayScatter => {
            target_list(ui, "X axis variables", &mut d.variables, picked);
            target_list(ui, "Y axis variables", &mut d.y_variables, picked);
            ui.checkbox(
                &mut d.pair,
                "Pair the lists in order instead of every combination",
            );
        }
        ChartKind::MatrixScatter => target_list(ui, "Variables", &mut d.variables, picked),
        ChartKind::ErrorBar => {
            target_list(ui, "Variables", &mut d.variables, picked);
            target_one(ui, "Category axis", &mut d.category, picked);
        }
        _ => {}
    }
    if let Some(title) = kind.group() {
        target_one(ui, title, &mut d.group, picked);
    }
    if kind.scatter() {
        target_one(ui, "Label points by", &mut d.label, picked);
    }
    match kind {
        ChartKind::ErrorBar => {
            ui.add_space(6.0);
            ui.strong("Bars show");
            for (i, (_, label)) in ERROR_BARS.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.error_bar, i, *label);
                    let text = match i {
                        0 => &mut d.level,
                        1 => &mut d.multiplier,
                        _ => &mut d.deviations,
                    };
                    small_field(ui, d.error_bar == i, text);
                });
            }
        }
        ChartKind::Pareto => {
            ui.checkbox(&mut d.cumulative, "Cumulative percentage line");
        }
        _ => {}
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Titles").show(ui, |ui| {
        field(ui, "Title", &mut d.title, 300.0);
        field(ui, "Subtitle", &mut d.subtitle, 300.0);
        field(ui, "Footnote", &mut d.footnote, 300.0);
    });
    egui::CollapsingHeader::new("Panels").show(ui, |ui| {
        ui.weak("A chart for each combination of categories");
        target_list(ui, "Columns by", &mut d.panel_columns, picked);
        target_list(ui, "Rows by", &mut d.panel_rows, picked);
    });
    egui::CollapsingHeader::new("Missing values").show(ui, |ui| {
        ui.radio_value(
            &mut d.by_variable,
            false,
            "Leave out a case missing on any variable of the chart",
        );
        ui.radio_value(
            &mut d.by_variable,
            true,
            "Leave out a case only where its value is missing",
        );
        ui.checkbox(&mut d.report, "Show categories of missing values");
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
}

pub fn probability_plot(ui: &mut Ui, d: &mut ProbabilityPlot, picked: &mut Vec<String>) {
    target_list(ui, "Variables", &mut d.variables, picked);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Test distribution");
        ComboBox::new("pplot_distribution", "")
            .selected_text(PPLOT_DISTRIBUTIONS[d.distribution].1)
            .show_index(ui, &mut d.distribution, PPLOT_DISTRIBUTIONS.len(), |i| {
                PPLOT_DISTRIBUTIONS[i].1
            });
    });
    let must_give = d.must_give();
    ui.add_enabled_ui(!must_give, |ui| {
        ui.checkbox(&mut d.estimate, "Estimate the parameters from the data");
    });
    let given = must_give || !d.estimate;
    let labels = PPLOT_DISTRIBUTIONS[d.distribution].2;
    ui.horizontal(|ui| {
        for (label, text) in labels.iter().zip(d.parameters.iter_mut()) {
            if !label.is_empty() {
                ui.label(*label);
                small_field(ui, given, text);
            }
        }
    });
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Ranks and plots").show(ui, |ui| {
        choice(ui, "Proportion formula", &PPLOT_FRACTIONS, &mut d.fraction);
        choice(ui, "Tied values get the", &PPLOT_TIES, &mut d.ties);
        choice(ui, "Show", &PPLOT_PLOTS, &mut d.plot);
    });
    egui::CollapsingHeader::new("Transform").show(ui, |ui| {
        ui.checkbox(&mut d.ln, "Natural log");
        ui.checkbox(&mut d.standardize, "Standardize values");
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.diff, "Difference, degree");
            small_field(ui, d.diff, &mut d.diff_degree);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.sdiff, "Seasonal difference, degree");
            small_field(ui, d.sdiff, &mut d.sdiff_degree);
            ui.label("period");
            small_field(ui, d.sdiff, &mut d.period);
        });
    });
}

pub fn roc(ui: &mut Ui, d: &mut RocCurve, picked: &mut Vec<String>) {
    target_list(ui, "Test variables", &mut d.tests, picked);
    target_one(ui, "State variable", &mut d.state, picked);
    field(ui, "Value of the positive state", &mut d.value, 80.0);
    ui.add_space(6.0);
    ui.strong("Display");
    ui.checkbox(&mut d.curve, "ROC curve");
    ui.add_enabled_ui(d.curve, |ui| {
        ui.checkbox(&mut d.reference, "With the diagonal reference line");
    });
    ui.checkbox(
        &mut d.se,
        "Standard error and confidence interval of the area",
    );
    ui.add_enabled_ui(d.se, |ui| {
        ui.checkbox(&mut d.model_quality, "Chart of the model quality");
    });
    ui.checkbox(
        &mut d.coordinates,
        "Table of cutoffs with sensitivity and specificity",
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.label("A positive test is");
        ui.radio_value(&mut d.small_positive, false, "A larger test value");
        ui.radio_value(&mut d.small_positive, true, "A smaller test value");
        ui.checkbox(
            &mut d.exclude_cutoff,
            "A value equal to the cutoff counts as negative",
        );
        // both only shape the standard error part, so they wait for it
        ui.add_enabled_ui(d.se, |ui| {
            field(ui, "Confidence level %", &mut d.level, 50.0);
            ui.label("Standard error of the area");
            ui.radio_value(&mut d.negexpo, false, "Nonparametric");
            ui.radio_value(&mut d.negexpo, true, "Bi-negative exponential");
        });
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    });
}
