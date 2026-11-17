//! Drawing of the Case Summaries and Codebook dialogs.

use crate::analyze_dialogs::MEANS_CELLS;
use crate::app::{checkboxes, choice, field, means_body, target_list};
use crate::reports_dialog::{
    CODEBOOK_ATTRIBUTE, CODEBOOK_FILEINFO, CODEBOOK_ORDERS, CODEBOOK_STATISTICS, CODEBOOK_VARINFO,
    Codebook, OlapCubes, OlapDifference, Summarize,
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

pub fn olap_cubes(ui: &mut Ui, d: &mut OlapCubes, picked: &mut Vec<String>) {
    target_list(ui, "Summary variables", &mut d.summary, picked);
    ui.horizontal(|ui| {
        let count = d.layers.len();
        ui.label(format!("Layer {} of {count}", d.layer + 1));
        if ui
            .add_enabled(d.layer > 0, egui::Button::new("Previous").small())
            .clicked()
        {
            d.layer -= 1;
        }
        let can_add = !d.layers[d.layer].is_empty();
        if ui
            .add_enabled(
                d.layer + 1 < count || can_add,
                egui::Button::new("Next").small(),
            )
            .clicked()
        {
            d.layer += 1;
            if d.layer == count {
                d.layers.push(Vec::new());
            }
        }
    });
    target_list(
        ui,
        "Group by (each layer is nested in the one before)",
        &mut d.layers[d.layer],
        picked,
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        ui.weak("None checked: no statistics, only the case counts");
        checkboxes(ui, &MEANS_CELLS, &mut d.cells);
    });
    egui::CollapsingHeader::new("Differences").show(ui, |ui| {
        ui.radio_value(&mut d.difference, OlapDifference::None, "None");
        ui.radio_value(
            &mut d.difference,
            OlapDifference::Groups,
            "Between two categories of a grouping variable",
        );
        ui.radio_value(
            &mut d.difference,
            OlapDifference::Variables,
            "Between two summary variables",
        );
        if d.difference == OlapDifference::None {
            return;
        }
        ui.checkbox(&mut d.absolute, "Second minus first");
        ui.checkbox(&mut d.percent, "Change as a percent of the first");
        let groups = d.difference == OlapDifference::Groups;
        if groups {
            let layers: Vec<String> = d.layers.iter().flatten().cloned().collect();
            ui.horizontal(|ui| {
                ui.label("Grouping variable");
                egui::ComboBox::new("olap_group", "")
                    .selected_text(if d.group.is_empty() {
                        "choose one"
                    } else {
                        &d.group
                    })
                    .show_ui(ui, |ui| {
                        for name in &layers {
                            ui.selectable_value(&mut d.group, name.clone(), name);
                        }
                    });
            });
        }
        ui.label(if groups {
            "Pairs of category values (first, second)"
        } else {
            "Pairs of summary variables (first, second)"
        });
        let mut remove = None;
        for (i, pair) in d.pairs.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut pair[0]).desired_width(90.0));
                ui.add(egui::TextEdit::singleline(&mut pair[1]).desired_width(90.0));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.pairs.remove(i);
        }
        if ui.small_button("Add a pair").clicked() {
            d.pairs.push(Default::default());
        }
    });
    egui::CollapsingHeader::new("Titles and counts").show(ui, |ui| {
        field(ui, "Title", &mut d.title, 260.0);
        field(ui, "Footnote", &mut d.footnote, 260.0);
        ui.weak("\\n in a title or footnote starts a new line");
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.hide_small, "Show counts below");
            small_field(ui, d.hide_small, &mut d.hide_below);
            ui.label("as small");
        });
    });
}
