//! Drawing of the Multiple Correspondence dialog.

use crate::app::{checkboxes, field, target_list};
use crate::mca_dialog::{BIPLOT, MCA_OUTFILE, MCA_PLOTS, MCA_PRINT, MCA_SAVE, Mca};
use crate::model_dialogs_view::name_checkboxes;
use crate::scaling_dialog_view::{
    PrintListsMut, model_fields, outfile_section, plot_options, print_lists, save_section,
    variable_grid,
};
use eframe::egui::{self, Ui};
use std::path::PathBuf;

pub fn mca(ui: &mut Ui, d: &mut Mca, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_list(ui, "Analysis variables", &mut d.variables, picked);
    variable_grid(ui, &d.variables, &mut d.settings, &[], |_, _| {});
    target_list(ui, "Labeling variables (optional)", &mut d.labels, picked);
    ui.weak("Not analyzed; they label objects or are listed beside the object scores");
    field(ui, "Supplementary cases", &mut d.objects, 160.0);
    ui.weak("Case numbers, ranges as 4-9; they are placed after the fit");
    ui.add_space(6.0);
    let all = d.all_variables();
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        model_fields(
            ui,
            &mut d.dimensions,
            &mut d.normalization,
            &mut d.q,
            &mut d.max_iterations,
            &mut d.convergence,
        );
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &MCA_PRINT, &mut d.print);
        print_lists(
            ui,
            [d.print[1], d.print[6], d.print[4]],
            (&all, &d.variables),
            PrintListsMut {
                descrip: &mut d.descrip,
                quant: &mut d.quant,
                shown: &mut d.object_shown,
                label: &mut d.object_label,
            },
        );
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
        plot_options(
            ui,
            (d.plots[5] || d.plots[6], &mut d.plot_dimensions),
            &mut d.label_length,
            &mut d.ndim,
            &mut d.ndim_first,
            &mut d.ndim_last,
        );
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_section(ui, &MCA_SAVE, &mut d.save, &mut d.save_roots);
    });
    egui::CollapsingHeader::new("Write to Files").show(ui, |ui| {
        outfile_section(
            ui,
            &MCA_OUTFILE,
            &mut d.outfile,
            (&mut d.outfile_files, &mut d.outfile_datasets),
            folder,
        );
    });
}
