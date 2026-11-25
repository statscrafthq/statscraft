//! Drawing of the Proportions dialog.

use crate::app::{checkboxes, choice, field, target_list};
use crate::i18n::t;
use crate::proportions_dialog::{
    PROPORTIONS_GROUPS, PROPORTIONS_KINDS, PROPORTIONS_SUCCESS, Proportions, ProportionsKind,
};
use eframe::egui::{self, Ui};

pub fn proportions(ui: &mut Ui, d: &mut Proportions, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.label(t("stats-prop-compare"));
        for (kind, label) in PROPORTIONS_KINDS {
            ui.radio_value(&mut d.kind, kind, t(label));
        }
    });
    ui.add_space(6.0);
    target_list(ui, t("stats-prop-variables"), &mut d.variables, picked);
    let k = d.kind;
    match k {
        ProportionsKind::OneSample => {
            field(ui, t("stats-prop-test-value"), &mut d.test_value, 60.0);
        }
        ProportionsKind::Paired => {
            target_list(ui, t("stats-prop-pair-with"), &mut d.with, picked);
            ui.checkbox(&mut d.in_order, t("stats-prop-in-order"));
            ui.weak(t("stats-prop-pairs-hint"));
        }
        ProportionsKind::Independent => {
            target_list(ui, t("stats-prop-grouping-variable"), &mut d.group, picked);
            choice(
                ui,
                t("stats-prop-groups"),
                &PROPORTIONS_GROUPS,
                &mut d.groups,
            );
            match PROPORTIONS_GROUPS[d.groups].0 {
                "LEVEL" => {
                    ui.horizontal(|ui| {
                        ui.label(t("stats-prop-group-1"));
                        ui.add(
                            egui::TextEdit::singleline(&mut d.group_values[0]).desired_width(60.0),
                        );
                        ui.label(t("stats-prop-group-2"));
                        ui.add(
                            egui::TextEdit::singleline(&mut d.group_values[1]).desired_width(60.0),
                        );
                    });
                }
                "CUTPOINT" => field(ui, t("stats-prop-cut-point"), &mut d.cutpoint, 60.0),
                _ => {}
            }
        }
    }
    ui.add_space(6.0);
    let i = k.index();
    egui::CollapsingHeader::new(t("stats-prop-tests")).show(ui, |ui| {
        ui.weak(t("stats-prop-no-tests"));
        checkboxes(ui, k.tests(), &mut d.tests[i]);
    });
    egui::CollapsingHeader::new(t("stats-prop-intervals")).show(ui, |ui| {
        ui.weak(t("stats-prop-no-intervals"));
        checkboxes(ui, k.intervals(), &mut d.intervals[i]);
        field(ui, t("stats-prop-level"), &mut d.level, 50.0);
    });
    egui::CollapsingHeader::new(t("stats-prop-success-missing")).show(ui, |ui| {
        ui.label(t("stats-prop-success-when"));
        choice(ui, "", &PROPORTIONS_SUCCESS, &mut d.success);
        match PROPORTIONS_SUCCESS[d.success].0 {
            "LEVEL" => field(ui, t("stats-prop-values"), &mut d.success_values, 120.0),
            "CUTPOINT" => field(ui, t("stats-prop-cut-point"), &mut d.success_cutpoint, 60.0),
            _ => {}
        }
        ui.checkbox(&mut d.listwise, t("stats-prop-listwise"));
        ui.checkbox(&mut d.include_missing, t("stats-prop-include-missing"));
    });
}
