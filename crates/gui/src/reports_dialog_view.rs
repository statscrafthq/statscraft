//! Drawing of the Case Summaries and Codebook dialogs.

use crate::analyze_dialogs::MEANS_CELLS;
use crate::app::{checkboxes, choice, field, means_body, target_list};
use crate::i18n::{t, t_args};
use crate::reports_dialog::{
    CODEBOOK_ATTRIBUTE, CODEBOOK_FILEINFO, CODEBOOK_ORDERS, CODEBOOK_STATISTICS, CODEBOOK_VARINFO,
    Codebook, OlapCubes, OlapDifference, Summarize,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, Ui};

pub fn summarize(ui: &mut Ui, d: &mut Summarize, picked: &mut Vec<String>) {
    means_body(ui, &mut d.means, picked);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-reports-listing")).show(ui, |ui| {
        ui.checkbox(&mut d.list, t("stats-reports-list-the-cases"));
        ui.add_enabled_ui(d.list, |ui| {
            ui.checkbox(&mut d.valid_only, t("stats-reports-only-valid-cases"));
            ui.horizontal(|ui| {
                ui.label(t("stats-reports-use-only-first"));
                small_field(ui, true, &mut d.limit);
                ui.label(t("stats-reports-cases-empty-for-all"));
            });
            ui.checkbox(&mut d.case_numbers, t("stats-reports-number-the-cases"));
            field(
                ui,
                t("stats-reports-show-sysmis-as"),
                &mut d.missing_text,
                120.0,
            );
        })
        .response
        .on_disabled_hover_text(t("stats-reports-turn-on-list"));
    });
    egui::CollapsingHeader::new(t("stats-reports-options")).show(ui, |ui| {
        ui.checkbox(&mut d.total_label, t("stats-reports-label-total-rows"));
        ui.checkbox(&mut d.tablewise, t("stats-reports-leave-out-missing"));
        field(ui, t("stats-reports-title"), &mut d.title, 260.0);
        field(ui, t("stats-reports-footnote"), &mut d.footnote, 260.0);
        ui.weak(t("stats-reports-new-line-hint"));
    });
}

pub fn codebook(ui: &mut Ui, d: &mut Codebook, picked: &mut Vec<String>) {
    target_list(
        ui,
        t("stats-reports-variables-and-sets"),
        &mut d.variables,
        picked,
    );
    ui.weak(t("stats-reports-empty-every-variable"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-reports-variable-entries")).show(ui, |ui| {
        checkboxes(ui, &CODEBOOK_VARINFO, &mut d.varinfo);
    });
    egui::CollapsingHeader::new(t("stats-reports-file")).show(ui, |ui| {
        ui.label(t("stats-reports-file-table-hint"));
        checkboxes(ui, &CODEBOOK_FILEINFO, &mut d.fileinfo);
    });
    egui::CollapsingHeader::new(t("stats-reports-statistics")).show(ui, |ui| {
        ui.weak(t("stats-reports-codebook-statistics-hint"));
        checkboxes(ui, &CODEBOOK_STATISTICS, &mut d.statistics);
        field(
            ui,
            t("stats-reports-leave-out-values"),
            &mut d.maxcats,
            50.0,
        );
    });
    egui::CollapsingHeader::new(t("stats-reports-order")).show(ui, |ui| {
        choice(ui, t("stats-reports-order"), &CODEBOOK_ORDERS, &mut d.order);
        if d.order == CODEBOOK_ATTRIBUTE {
            field(ui, t("stats-reports-attribute"), &mut d.attribute, 160.0);
        }
        ui.checkbox(&mut d.descending, t("stats-reports-descending"));
    });
}

pub fn olap_cubes(ui: &mut Ui, d: &mut OlapCubes, picked: &mut Vec<String>) {
    target_list(
        ui,
        t("stats-reports-summary-variables"),
        &mut d.summary,
        picked,
    );
    ui.horizontal(|ui| {
        let count = d.layers.len();
        ui.label(t_args(
            "stats-reports-layer-of",
            &[
                ("layer", (d.layer + 1).to_string().into()),
                ("count", count.to_string().into()),
            ],
        ));
        if ui
            .add_enabled(
                d.layer > 0,
                egui::Button::new(t("stats-reports-previous")).small(),
            )
            .clicked()
        {
            d.layer -= 1;
        }
        let can_add = !d.layers[d.layer].is_empty();
        if ui
            .add_enabled(
                d.layer + 1 < count || can_add,
                egui::Button::new(t("stats-reports-next")).small(),
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
        t("stats-reports-group-by-nested"),
        &mut d.layers[d.layer],
        picked,
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("stats-reports-statistics")).show(ui, |ui| {
        ui.weak(t("stats-reports-olap-statistics-hint"));
        checkboxes(ui, &MEANS_CELLS, &mut d.cells);
    });
    egui::CollapsingHeader::new(t("stats-reports-differences")).show(ui, |ui| {
        ui.radio_value(
            &mut d.difference,
            OlapDifference::None,
            t("stats-reports-none"),
        );
        ui.radio_value(
            &mut d.difference,
            OlapDifference::Groups,
            t("stats-reports-between-categories"),
        );
        ui.radio_value(
            &mut d.difference,
            OlapDifference::Variables,
            t("stats-reports-between-summary-variables"),
        );
        if d.difference == OlapDifference::None {
            return;
        }
        ui.checkbox(&mut d.absolute, t("stats-reports-second-minus-first"));
        ui.checkbox(&mut d.percent, t("stats-reports-percent-change"));
        let groups = d.difference == OlapDifference::Groups;
        if groups {
            let layers: Vec<String> = d.layers.iter().flatten().cloned().collect();
            ui.horizontal(|ui| {
                ui.label(t("stats-reports-grouping-variable"));
                egui::ComboBox::new("olap_group", "")
                    .selected_text(if d.group.is_empty() {
                        t("stats-reports-choose-one")
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
            t("stats-reports-pairs-of-values")
        } else {
            t("stats-reports-pairs-of-variables")
        });
        let mut remove = None;
        for (i, pair) in d.pairs.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut pair[0]).desired_width(90.0));
                ui.add(egui::TextEdit::singleline(&mut pair[1]).desired_width(90.0));
                if ui.small_button(t("stats-reports-remove")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.pairs.remove(i);
        }
        if ui.small_button(t("stats-reports-add-a-pair")).clicked() {
            d.pairs.push(Default::default());
        }
    });
    egui::CollapsingHeader::new(t("stats-reports-titles-and-counts")).show(ui, |ui| {
        field(ui, t("stats-reports-title"), &mut d.title, 260.0);
        field(ui, t("stats-reports-footnote"), &mut d.footnote, 260.0);
        ui.weak(t("stats-reports-new-line-hint"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.hide_small, t("stats-reports-show-counts-below"));
            small_field(ui, d.hide_small, &mut d.hide_below);
            ui.label(t("stats-reports-as-small"));
        });
    });
}
