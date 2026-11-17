//! Drawing of the Proportions dialog.

use crate::app::{checkboxes, choice, field, target_list};
use crate::proportions_dialog::{
    PROPORTIONS_GROUPS, PROPORTIONS_KINDS, PROPORTIONS_SUCCESS, Proportions, ProportionsKind,
};
use eframe::egui::{self, Ui};

pub fn proportions(ui: &mut Ui, d: &mut Proportions, picked: &mut Vec<String>) {
    ui.horizontal(|ui| {
        ui.label("Compare");
        for (kind, label) in PROPORTIONS_KINDS {
            ui.radio_value(&mut d.kind, kind, label);
        }
    });
    ui.add_space(6.0);
    target_list(ui, "Variables", &mut d.variables, picked);
    let k = d.kind;
    match k {
        ProportionsKind::OneSample => {
            field(ui, "Test value (a proportion)", &mut d.test_value, 60.0);
        }
        ProportionsKind::Paired => {
            target_list(ui, "Pair with (optional)", &mut d.with, picked);
            ui.checkbox(&mut d.in_order, "Pair the first with the first, and so on");
            ui.weak("Without a second list every variable is paired with each one after it");
        }
        ProportionsKind::Independent => {
            target_list(ui, "Grouping variable", &mut d.group, picked);
            choice(ui, "Groups", &PROPORTIONS_GROUPS, &mut d.groups);
            match PROPORTIONS_GROUPS[d.groups].0 {
                "LEVEL" => {
                    ui.horizontal(|ui| {
                        ui.label("Group 1");
                        ui.add(
                            egui::TextEdit::singleline(&mut d.group_values[0]).desired_width(60.0),
                        );
                        ui.label("Group 2");
                        ui.add(
                            egui::TextEdit::singleline(&mut d.group_values[1]).desired_width(60.0),
                        );
                    });
                }
                "CUTPOINT" => field(ui, "Cut point", &mut d.cutpoint, 60.0),
                _ => {}
            }
        }
    }
    ui.add_space(6.0);
    let i = match k {
        ProportionsKind::OneSample => 0,
        ProportionsKind::Paired => 1,
        ProportionsKind::Independent => 2,
    };
    egui::CollapsingHeader::new("Tests").show(ui, |ui| {
        ui.weak("None checked: no tests");
        checkboxes(ui, k.tests(), &mut d.tests[i]);
    });
    egui::CollapsingHeader::new("Confidence intervals").show(ui, |ui| {
        ui.weak("None checked: no intervals");
        checkboxes(ui, k.intervals(), &mut d.intervals[i]);
        field(ui, "Confidence level (percent)", &mut d.level, 50.0);
    });
    egui::CollapsingHeader::new("Success and missing values").show(ui, |ui| {
        ui.label("A case is a success when its value is");
        choice(ui, "", &PROPORTIONS_SUCCESS, &mut d.success);
        match PROPORTIONS_SUCCESS[d.success].0 {
            "LEVEL" => field(ui, "Values", &mut d.success_values, 120.0),
            "CUTPOINT" => field(ui, "Cut point", &mut d.success_cutpoint, 60.0),
            _ => {}
        }
        ui.checkbox(&mut d.listwise, "Leave out a case missing on any variable");
        ui.checkbox(&mut d.include_missing, "Count user-missing values as valid");
    });
}
