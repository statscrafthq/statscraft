//! Drawing of the Control Charts dialog.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::spchart_dialog::{
    C, CAPABILITY, ControlCharts, SPCHART_CAPSIGMAS, SPCHART_CHARTS, SPCHART_NORMALS,
    SPCHART_REFERENCES, SPCHART_RULES, SPCHART_STATISTICS, Shape, U, XR, XS,
};
use eframe::egui::{self, ComboBox, Ui};

pub fn control_charts(ui: &mut Ui, d: &mut ControlCharts, picked: &mut Vec<String>) {
    choice(ui, "Chart", &SPCHART_CHARTS, &mut d.chart);
    let layouts = d.layouts();
    d.layout = d.layout.min(layouts.len() - 1);
    if layouts.len() > 1 {
        ui.label("Data");
        for (i, (_, label)) in layouts.iter().enumerate() {
            ui.radio_value(&mut d.layout, i, *label);
        }
    }
    ui.add_space(6.0);
    let shape = d.shape();
    let (process, group) = match d.chart {
        crate::spchart_dialog::P | crate::spchart_dialog::NP => {
            ("Status variable", "Subgroup variable")
        }
        C | U => ("Defects variable", "Subgroup variable"),
        _ => ("Process variable", "Subgroup variable"),
    };
    match shape {
        Shape::Units => {
            target_one(ui, process, &mut d.process, picked);
            target_one(ui, group, &mut d.group, picked);
        }
        Shape::Subgroups => {
            target_list(ui, "Variables of a subgroup", &mut d.variables, picked);
            ui.weak("Two or more; each case is one subgroup");
        }
        Shape::Single => target_one(ui, process, &mut d.process, picked),
        Shape::Counts => {
            target_one(ui, "Count variable", &mut d.count, picked);
            ui.horizontal(|ui| {
                ui.label("Subgroup size");
                ui.radio_value(&mut d.size_variable, true, "a variable");
                ui.radio_value(&mut d.size_variable, false, "one number");
            });
            match d.size_variable {
                true => target_one(ui, "Size variable", &mut d.size_name, picked),
                false => field(ui, "Size", &mut d.size, 60.0),
            }
        }
    }
    if d.status() {
        ui.horizontal(|ui| {
            ui.label("The value");
            ui.add(egui::TextEdit::singleline(&mut d.status_value).desired_width(80.0));
            ui.radio_value(&mut d.nonconforming, true, "is nonconforming");
            ui.radio_value(&mut d.nonconforming, false, "is conforming");
        });
        ui.weak("Any other value of the status is the opposite");
    }
    if d.labelled() {
        target_one(ui, "Label points by (optional)", &mut d.label, picked);
    }
    if matches!(d.chart, XR | XS) {
        ui.checkbox(&mut d.xbar_only, "Only the X-bar chart");
    }
    ui.add_space(6.0);
    if d.controls() {
        egui::CollapsingHeader::new("Control limits").show(ui, |ui| {
            field(ui, "Sigmas", &mut d.sigmas, 50.0);
            ui.weak("Above 1 and at most 10; the limits lie this far from the center line");
            if d.of_values() && matches!(shape, Shape::Units | Shape::Subgroups) {
                field(ui, "Smallest subgroup kept", &mut d.minsample, 50.0);
            }
            if shape == Shape::Single {
                field(ui, "Span of the moving range", &mut d.span, 50.0);
            }
        });
        egui::CollapsingHeader::new("Rules for out of control points").show(ui, |ui| {
            let mut all = d.rules.iter().all(|r| *r);
            if ui.checkbox(&mut all, "All rules").changed() {
                d.rules = [all; 11];
            }
            checkboxes(ui, &SPCHART_RULES, &mut d.rules);
            ui.add_enabled_ui(d.rules.iter().any(|r| *r), |ui| {
                target_one(ui, "Name points by", &mut d.id, picked);
            });
            ui.weak("Rules are checked on the first chart; no ID: the label or the case number");
        });
    } else {
        if matches!(shape, Shape::Units | Shape::Subgroups) {
            field(ui, "Smallest subgroup kept", &mut d.minsample, 50.0);
        }
        if shape == Shape::Single {
            field(ui, "Span of the moving range", &mut d.span, 50.0);
        }
    }
    if d.of_values() {
        egui::CollapsingHeader::new("Specification limits").show(ui, |ui| {
            field(ui, "Lower limit", &mut d.lsl, 70.0);
            field(ui, "Upper limit", &mut d.usl, 70.0);
            field(ui, "Target", &mut d.target, 70.0);
            if d.chart == CAPABILITY {
                ui.weak("Both limits are needed");
            }
        });
        egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
            checkboxes(ui, &SPCHART_STATISTICS, &mut d.statistics);
            ui.weak("Most need both limits; a target for Cpm and Ppm");
            let fits = d.capsigmas();
            d.capsigma = if fits.contains(&d.capsigma) {
                d.capsigma
            } else {
                0
            };
            ui.horizontal(|ui| {
                ui.label("Capability sigma from");
                ComboBox::from_id_salt("capsigma")
                    .selected_text(SPCHART_CAPSIGMAS[d.capsigma].1)
                    .show_ui(ui, |ui| {
                        for i in fits {
                            ui.selectable_value(&mut d.capsigma, i, SPCHART_CAPSIGMAS[i].1);
                        }
                    });
            });
        });
    }
    if d.chart == CAPABILITY {
        egui::CollapsingHeader::new("Capability chart").show(ui, |ui| {
            choice(ui, "Normal curve", &SPCHART_NORMALS, &mut d.normal);
            choice(
                ui,
                "Reference values in the footnote",
                &SPCHART_REFERENCES,
                &mut d.reference,
            );
        });
    }
    egui::CollapsingHeader::new("Missing values and titles").show(ui, |ui| {
        ui.checkbox(&mut d.report, "Keep a subgroup for missing subgroup values");
        ui.checkbox(&mut d.include, "Treat user-missing values as valid");
        field(ui, "Title", &mut d.title, 260.0);
        field(ui, "Subtitle", &mut d.subtitle, 260.0);
        field(ui, "Footnote", &mut d.footnote, 260.0);
    });
}
