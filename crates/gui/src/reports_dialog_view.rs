//! Drawing of the Case Summaries and Codebook dialogs.

use crate::app::{checkboxes, choice, field, means_body, target_list};
use crate::reports_dialog::{
    CODEBOOK_ATTRIBUTE, CODEBOOK_FILEINFO, CODEBOOK_ORDERS, CODEBOOK_STATISTICS, CODEBOOK_VARINFO,
    Codebook, Summarize,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};

pub fn summarize(ui: &mut Ui, d: &mut Summarize, picked: &mut Vec<String>) {
    means_body(ui, &mut d.means, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Listing").show(ui, |ui| {
        ui.checkbox(&mut d.list, "List the cases");
        ui.add_enabled_ui(d.list, |ui| {
            ui.checkbox(&mut d.valid_only, "Only cases valid on every variable");
            ui.horizontal(|ui| {
                ui.label("Use only the first");
                small_field(ui, true, &mut d.limit);
                ui.label("cases (empty for all)");
            });
            ui.checkbox(&mut d.case_numbers, "Number the cases");
            field(
                ui,
                "Show system-missing values as",
                &mut d.missing_text,
                120.0,
            );
        })
        .response
        .on_disabled_hover_text("Turn on List the cases");
    });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        ui.checkbox(&mut d.total_label, "Label the total rows Total");
        ui.checkbox(
            &mut d.tablewise,
            "Leave out a case missing on any summarized variable",
        );
        field(ui, "Title", &mut d.title, 260.0);
        field(ui, "Footnote", &mut d.footnote, 260.0);
        ui.weak("\\n in a title or footnote starts a new line");
    });
}

pub fn codebook(ui: &mut Ui, d: &mut Codebook, picked: &mut Vec<String>) {
    target_list(ui, "Variables and sets", &mut d.variables, picked);
    ui.weak("Empty: every variable, then every multiple response set");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Variable entries").show(ui, |ui| {
        checkboxes(ui, &CODEBOOK_VARINFO, &mut d.varinfo);
    });
    egui::CollapsingHeader::new("File").show(ui, |ui| {
        ui.label("A table about the data file (none checked: no table)");
        checkboxes(ui, &CODEBOOK_FILEINFO, &mut d.fileinfo);
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.weak("Counts and percents for categories, the rest for scale variables; none checked: no statistics");
        checkboxes(ui, &CODEBOOK_STATISTICS, &mut d.statistics);
        field(
            ui,
            "Leave out the values of a variable with more than",
            &mut d.maxcats,
            50.0,
        );
    });
    egui::CollapsingHeader::new("Order").show(ui, |ui| {
        choice(ui, "Order", &CODEBOOK_ORDERS, &mut d.order);
        if d.order == CODEBOOK_ATTRIBUTE {
            field(ui, "Attribute", &mut d.attribute, 160.0);
        }
        ui.checkbox(&mut d.descending, "Descending");
    });
}
