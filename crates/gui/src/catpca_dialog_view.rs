//! Drawing of the Categorical Principal Components dialog.

use crate::app::{checkboxes, choice, field, target_list};
use crate::catpca_dialog::{
    BIPLOT, CATPCA_CONFIGURATIONS, CATPCA_LEVELS, CATPCA_OUTFILE, CATPCA_PLOTS, CATPCA_PRINT,
    CATPCA_ROTATIONS, CATPCA_SAVE, Catpca, LOADING, OBJECT, OBLIMIN, PRINT_DESCRIP, PRINT_LOADING,
    PRINT_OBJECT, PRINT_QUANT, PROJCENTR, PROMAX, RESID, TRANS, TRIPLOT, VAF,
};
use crate::mds_dialog_view::data_path;
use crate::model_dialogs_view::name_checkboxes;
use crate::scaling_dialog_view::{
    PrintListsMut, model_fields, outfile_section, plot_options, print_lists, save_section,
    variable_grid,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use std::path::PathBuf;

pub fn catpca(ui: &mut Ui, d: &mut Catpca, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    target_list(ui, "Analysis variables", &mut d.variables, picked);
    variable_grid(
        ui,
        &d.variables,
        &mut d.settings,
        &["Scaling level"],
        |ui, s| {
            let level = &mut s.level;
            ui.horizontal(|ui| {
                ComboBox::from_id_salt((&s.name, "level"))
                    .selected_text(CATPCA_LEVELS[level.level].1)
                    .show_index(ui, &mut level.level, CATPCA_LEVELS.len(), |i| {
                        CATPCA_LEVELS[i].1
                    });
                if level.spline() {
                    ui.label("degree");
                    small_field(ui, true, &mut level.degree);
                    ui.label("knots");
                    small_field(ui, true, &mut level.knots);
                }
            });
        },
    );
    ui.weak("A spline has a degree from 1 and interior knots from 0");
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
        choice(
            ui,
            "Object scores",
            &CATPCA_CONFIGURATIONS,
            &mut d.configuration,
        );
        if d.configuration > 0 {
            data_path(
                ui,
                "Configuration file",
                &mut d.configuration_file,
                false,
                folder,
            );
            ui.weak("Its first numeric variables give the scores, one per dimension and case");
        }
    });
    egui::CollapsingHeader::new("Rotation").show(ui, |ui| {
        choice(ui, "Method", &CATPCA_ROTATIONS, &mut d.rotation);
        ui.horizontal(|ui| {
            ui.label("Kappa");
            small_field(ui, d.rotation == PROMAX, &mut d.kappa);
            ui.label("Delta");
            small_field(ui, d.rotation == OBLIMIN, &mut d.delta);
        });
        ui.weak("Promax kappa from 1 to below 10000; oblimin delta at most 0.8");
        ui.add_enabled(
            d.rotation > 0,
            egui::Checkbox::new(&mut d.kaiser, "Kaiser normalization"),
        );
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &CATPCA_PRINT, &mut d.print);
        if d.print[PRINT_LOADING] {
            ui.checkbox(&mut d.sort_loadings, "Sort loadings by size");
        }
        print_lists(
            ui,
            [
                d.print[PRINT_DESCRIP],
                d.print[PRINT_QUANT],
                d.print[PRINT_OBJECT],
            ],
            (&all, &d.variables),
            PrintListsMut {
                descrip: &mut d.descrip,
                quant: &mut d.quant,
                shown: &mut d.object_shown,
                label: &mut d.object_label,
            },
        );
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| plots(ui, d, &all));
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        save_section(ui, &CATPCA_SAVE, &mut d.save, &mut d.save_roots);
        ui.weak("Multiple nominal variables keep 3 characters of the root of quantified variables");
    });
    egui::CollapsingHeader::new("Write to Files").show(ui, |ui| {
        outfile_section(
            ui,
            &CATPCA_OUTFILE,
            &mut d.outfile,
            (&mut d.outfile_files, &mut d.outfile_datasets),
            folder,
        );
    });
}

fn plots(ui: &mut Ui, d: &mut Catpca, all: &[String]) {
    let joint = d.joint_plots_allowed();
    egui::Grid::new("catpca plots")
        .num_columns(2)
        .show(ui, |ui| {
            for (i, (_, label)) in CATPCA_PLOTS.iter().enumerate() {
                // shown off where the syntax leaves it out
                let on = joint || (i != BIPLOT && i != TRIPLOT);
                let mut off = false;
                let flag = if on { &mut d.plots[i] } else { &mut off };
                ui.add_enabled(on, egui::Checkbox::new(flag, *label))
                    .on_disabled_hover_text("Not drawn with independent normalization");
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    let multiple = d.by_level(true);
    for (i, (_, label)) in CATPCA_PLOTS.iter().enumerate() {
        let off = !joint && (i == BIPLOT || i == TRIPLOT);
        if !d.plots[i] || off || i == VAF {
            continue;
        }
        if i == BIPLOT {
            ui.checkbox(
                &mut d.biplot_centroids,
                "Biplot with centroids instead of loadings",
            );
        }
        if i == PROJCENTR {
            ui.horizontal(|ui| {
                ui.label("Project the centroids of");
                ComboBox::from_id_salt("projected")
                    .selected_text(match d.variables.contains(&d.projected) {
                        true => d.projected.as_str(),
                        false => "choose",
                    })
                    .show_ui(ui, |ui| {
                        for name in &d.variables {
                            ui.selectable_value(&mut d.projected, name.clone(), name);
                        }
                    });
            });
        }
        match i {
            OBJECT => ui.label("Object points labeled by (none: case numbers)"),
            LOADING => ui.label("Loadings of"),
            PROJCENTR => ui.label("Onto the vectors of"),
            _ => ui.label(format!("{label} of")),
        };
        let options = d.plot_options(i);
        ui.push_id(("plot", i), |ui| {
            name_checkboxes(ui, &options, &mut d.plot_variables[i])
        });
        match i {
            LOADING => {
                ui.checkbox(
                    &mut d.loading_centroids,
                    "With centroids of multiple nominal variables",
                );
                if d.loading_centroids {
                    ui.push_id("centroids", |ui| {
                        name_checkboxes(ui, &multiple, &mut d.centroids)
                    });
                }
            }
            BIPLOT => {
                ui.label("Biplot objects labeled by (none: case numbers)");
                ui.push_id("biplot labels", |ui| {
                    name_checkboxes(ui, all, &mut d.biplot_labels)
                });
            }
            TRIPLOT => {
                ui.label("Triplot objects labeled by (none: case numbers)");
                ui.push_id("triplot labels", |ui| {
                    name_checkboxes(ui, all, &mut d.triplot_labels)
                });
            }
            _ => {}
        }
    }
    ui.weak("No variable checked plots every variable the plot can show");
    plot_options(
        ui,
        (d.plots[TRANS] || d.plots[RESID], &mut d.plot_dimensions),
        &mut d.label_length,
        &mut d.ndim,
        &mut d.ndim_first,
        &mut d.ndim_last,
    );
    ui.weak("Only multiple nominal variables have more than one dimension to plot");
}
