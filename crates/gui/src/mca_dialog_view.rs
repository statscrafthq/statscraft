//! Drawing of the Multiple Correspondence dialog.

use crate::app::{checkboxes, choice, field, target_list};
use crate::mca_dialog::{
    BIPLOT, MCA_DISCRETIZE, MCA_MISSING, MCA_NORMALIZATIONS, MCA_OUTFILE, MCA_PLOTS, MCA_PRINT,
    MCA_SAVE, Mca, McaVariable,
};
use crate::model_dialogs_view::name_checkboxes;
use crate::mva_dialog::MVA_OUTPUT;
use crate::proximities_dialog_view::output_picker;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use std::path::PathBuf;

pub fn mca(ui: &mut Ui, d: &mut Mca, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_list(ui, "Analysis variables", &mut d.variables, picked);
    settings(ui, d);
    target_list(ui, "Labeling variables (optional)", &mut d.labels, picked);
    ui.weak("Not analyzed; they label objects or are listed beside the object scores");
    field(ui, "Supplementary cases", &mut d.objects, 160.0);
    ui.weak("Case numbers, ranges as 4-9; they are placed after the fit");
    ui.add_space(6.0);
    let all = d.all_variables();
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Dimensions in the solution", &mut d.dimensions, 40.0);
        ui.horizontal(|ui| {
            choice(
                ui,
                "Normalization",
                &MCA_NORMALIZATIONS,
                &mut d.normalization,
            );
            let custom = d.normalization == MCA_NORMALIZATIONS.len() - 1;
            small_field(ui, custom, &mut d.q);
        });
        ui.weak("A custom value runs from -1 (variable principal) to 1 (object principal)");
        field(ui, "Most iterations", &mut d.max_iterations, 50.0);
        field(
            ui,
            "Stop when the fit improves less than",
            &mut d.convergence,
            70.0,
        );
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &MCA_PRINT, &mut d.print);
        if d.print[1] {
            ui.label("Descriptives of");
            ui.push_id("descrip", |ui| name_checkboxes(ui, &all, &mut d.descrip));
        }
        if d.print[6] {
            ui.label("Quantifications of");
            ui.push_id("quant", |ui| {
                name_checkboxes(ui, &d.variables, &mut d.quant)
            });
        }
        if d.print[4] {
            ui.horizontal(|ui| {
                ui.label("Label object scores by");
                ComboBox::from_id_salt("object label")
                    .selected_text(match all.contains(&d.object_label) {
                        true => d.object_label.as_str(),
                        false => "case number",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.object_label, String::new(), "case number");
                        for name in &all {
                            ui.selectable_value(&mut d.object_label, name.clone(), name);
                        }
                    });
            });
            ui.label("List beside the object scores");
            ui.push_id("shown", |ui| name_checkboxes(ui, &all, &mut d.object_shown));
        }
        ui.weak("No variable checked: the analysis variables (none beside the object scores)");
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        let biplot = d.biplot_allowed();
        egui::Grid::new("mca plots").num_columns(2).show(ui, |ui| {
            for (i, (_, label)) in MCA_PLOTS.iter().enumerate() {
                // shown off where the syntax leaves it out
                let on = i != BIPLOT || biplot;
                let mut off = false;
                let flag = if on { &mut d.plots[i] } else { &mut off };
                ui.add_enabled(on, egui::Checkbox::new(flag, *label))
                    .on_disabled_hover_text("Not drawn with independent normalization");
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        for (i, (_, label)) in MCA_PLOTS.iter().enumerate() {
            if !d.plots[i] || (i == BIPLOT && !biplot) {
                continue;
            }
            match i {
                0 => ui.label("Object points labeled by (none: case numbers)"),
                BIPLOT => ui.label("Biplot centroids of"),
                _ => ui.label(format!("{label} of")),
            };
            let options = if i == 0 { &all } else { &d.variables };
            ui.push_id(("plot", i), |ui| {
                name_checkboxes(ui, options, &mut d.plot_variables[i])
            });
            if i == BIPLOT {
                ui.label("Biplot objects labeled by (none: case numbers)");
                ui.push_id("biplot labels", |ui| {
                    name_checkboxes(ui, &all, &mut d.biplot_labels)
                });
            }
        }
        ui.weak("No variable checked plots every analysis variable");
        ui.horizontal(|ui| {
            ui.label("Dimensions per variable in transformation and residual plots");
            small_field(ui, d.plots[5] || d.plots[6], &mut d.plot_dimensions);
        });
        field(
            ui,
            "Longest label, empty for full labels",
            &mut d.label_length,
            40.0,
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ndim, "Only dimensions from");
            small_field(ui, d.ndim, &mut d.ndim_first);
            ui.label("to");
            small_field(ui, d.ndim, &mut d.ndim_last);
        });
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        for (i, (_, label, longest)) in MCA_SAVE.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.save[i], *label);
                ui.label(format!("root (up to {longest} characters)"));
                small_field(ui, d.save[i], &mut d.save_roots[i]);
            });
        }
        ui.weak("An empty root uses the command's own");
    });
    egui::CollapsingHeader::new("Write to Files").show(ui, |ui| {
        for (i, (_, label)) in MCA_OUTFILE.iter().enumerate() {
            ui.label(*label);
            ui.push_id(("outfile", i), |ui| {
                output_picker(
                    ui,
                    &MVA_OUTPUT,
                    &mut d.outfile[i],
                    &mut d.outfile_files[i],
                    &mut d.outfile_datasets[i],
                    folder,
                )
            });
        }
    });
}

/// A row per analysis variable: its weight, recoding, missing values and role. A new variable
/// starts with the defaults.
fn settings(ui: &mut Ui, d: &mut Mca) {
    for name in &d.variables {
        if !d.settings.iter().any(|s| &s.name == name) {
            d.settings.push(McaVariable::new(name));
        }
    }
    if d.variables.is_empty() {
        return;
    }
    egui::Grid::new("mca variables")
        .num_columns(5)
        .show(ui, |ui| {
            for label in [
                "Variable",
                "Weight",
                "Recode",
                "Missing values",
                "Supplementary",
            ] {
                ui.strong(label);
            }
            ui.end_row();
            for name in &d.variables {
                let Some(s) = d.settings.iter_mut().find(|s| &s.name == name) else {
                    continue;
                };
                ui.label(name.as_str());
                small_field(ui, !s.supplementary, &mut s.weight);
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt((name, "recode"))
                        .selected_text(MCA_DISCRETIZE[s.discretize].1)
                        .show_index(ui, &mut s.discretize, MCA_DISCRETIZE.len(), |i| {
                            MCA_DISCRETIZE[i].1
                        });
                    match s.discretize {
                        1 => {
                            small_field(ui, true, &mut s.groups);
                            ui.checkbox(&mut s.uniform, "even counts");
                        }
                        2 => {
                            ui.label("width");
                            small_field(ui, true, &mut s.width);
                        }
                        _ => {}
                    }
                });
                ComboBox::from_id_salt((name, "missing"))
                    .selected_text(MCA_MISSING[s.missing].1)
                    .show_index(ui, &mut s.missing, MCA_MISSING.len(), |i| MCA_MISSING[i].1);
                ui.checkbox(&mut s.supplementary, "");
                ui.end_row();
            }
        });
    ui.weak("Groups follow a normal curve unless even counts is checked");
}
