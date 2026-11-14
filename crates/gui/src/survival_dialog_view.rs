//! Drawing of the Life Tables and Kaplan-Meier dialogs.

use crate::app::{checkboxes, choice, field, target_list, target_one};
use crate::correspondence_dialog_view::table_variable;
use crate::stats_dialogs_view::small_field;
use crate::survival_dialog::{
    KM_COMPARE, KM_PLOTS, KM_SAVE, KM_TESTS, KaplanMeier, LIFE_COMPARE, LIFE_PLOTS, LifeTables,
};
use eframe::egui::{self, TextEdit, Ui};
use statscraft_data::Dataset;

pub fn kaplan_meier(ui: &mut Ui, d: &mut KaplanMeier, picked: &mut Vec<String>) {
    target_one(ui, "Time", &mut d.time, picked);
    target_one(ui, "Status", &mut d.status, picked);
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.event_range, false, "Event values");
        ui.add_enabled(
            !d.event_range,
            TextEdit::singleline(&mut d.event_values).desired_width(140.0),
        );
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.event_range, true, "Event range from");
        small_field(ui, d.event_range, &mut d.event_from);
        ui.label("to");
        small_field(ui, d.event_range, &mut d.event_to);
    });
    field(ui, "Lost to follow-up (optional)", &mut d.lost, 140.0);
    ui.weak("List values with spaces or commas; a string status takes commas and no range");
    ui.add_space(6.0);
    target_one(ui, "Factor", &mut d.factor, picked);
    target_one(ui, "Strata", &mut d.strata, picked);
    target_one(ui, "Label cases by", &mut d.id, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Compare Factor").show(ui, |ui| {
        ui.add_enabled_ui(!d.factor.trim().is_empty(), |ui| {
            checkboxes(ui, &KM_TESTS, &mut d.tests);
            choice(ui, "Compare", &KM_COMPARE, &mut d.compare);
            let overall = !d.pairwise();
            ui.horizontal(|ui| {
                ui.add_enabled(
                    overall,
                    egui::Checkbox::new(&mut d.trend, "Linear trend across levels, weights"),
                )
                .on_disabled_hover_text(
                    "A trend applies only when all levels are compared at once",
                );
                ui.add_enabled(
                    overall && d.trend,
                    TextEdit::singleline(&mut d.trend_weights).desired_width(100.0),
                );
            });
            ui.weak("Empty weights are evenly spaced");
        })
        .response
        .on_disabled_hover_text("Choose a factor first");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for (i, (_, label)) in KM_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
                ui.label("name");
                small_field(ui, d.save[i], &mut d.save_names[i]);
            });
        }
        ui.weak("An empty name gets one made up");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.table, "Survival table");
        ui.checkbox(&mut d.mean, "Mean and median survival time");
        ui.checkbox(&mut d.quartiles, "Quartiles");
        field(ui, "More percentiles", &mut d.percentiles, 140.0);
        ui.label("Plots");
        checkboxes(ui, &KM_PLOTS, &mut d.plots);
    });
}

pub fn life_tables(ui: &mut Ui, data: &Dataset, d: &mut LifeTables, picked: &mut Vec<String>) {
    target_list(ui, "Time variables", &mut d.times, picked);
    ui.add_space(6.0);
    ui.strong("Intervals");
    let mut remove = None;
    let rows = d.intervals.len();
    for (i, (end, width)) in d.intervals.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label(if i == 0 { "0 through" } else { "then through" });
                ui.add(TextEdit::singleline(end).desired_width(50.0));
                ui.label("by");
                ui.add(TextEdit::singleline(width).desired_width(50.0));
                if rows > 1 && ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        d.intervals.remove(i);
    }
    if ui.small_button("Add a row").clicked() {
        d.intervals.push(Default::default());
    }
    ui.add_space(6.0);
    target_one(ui, "Status", &mut d.status, picked);
    ui.horizontal(|ui| {
        ui.label("Event value");
        small_field(ui, true, &mut d.event_from);
        ui.label("through");
        small_field(ui, true, &mut d.event_to);
    });
    ui.weak("Leave the second empty for a single value");
    ui.add_space(6.0);
    table_variable(ui, data, "First factor", &mut d.factor, picked);
    table_variable(ui, data, "Second factor", &mut d.second, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.table, "Life tables");
        ui.label("Plots");
        checkboxes(ui, &LIFE_PLOTS, &mut d.plots);
        choice(
            ui,
            "Compare levels of the first factor",
            &LIFE_COMPARE,
            &mut d.compare,
        );
        ui.add_enabled(
            d.compare > 0,
            egui::Checkbox::new(&mut d.approximate, "Approximate, by interval"),
        );
    });
    egui::CollapsingHeader::new("Missing Values").show(ui, |ui| {
        ui.radio_value(
            &mut d.listwise,
            false,
            "Leave a case out of the tables it misses",
        );
        ui.radio_value(
            &mut d.listwise,
            true,
            "Leave a case out of every table (listwise)",
        );
        ui.checkbox(&mut d.include, "Include user-missing values");
    });
}
