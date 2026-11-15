//! Drawing of what the Multiple Correspondence and Categorical Principal Components dialogs
//! share.

use crate::app::{choice, field};
use crate::model_dialogs_view::name_checkboxes;
use crate::mva_dialog::MVA_OUTPUT;
use crate::proximities_dialog_view::output_picker;
use crate::scaling_dialog::{
    SCALING_DISCRETIZE, SCALING_MISSING, SCALING_NORMALIZATIONS, ScalingVariable,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};
use std::path::PathBuf;

/// A row per analysis variable: its weight, the columns `extra` draws under `headers`, its
/// recoding, missing values and role. A new variable starts with the defaults.
pub fn variable_grid<L: Default>(
    ui: &mut Ui,
    variables: &[String],
    settings: &mut Vec<ScalingVariable<L>>,
    headers: &[&str],
    mut extra: impl FnMut(&mut Ui, &mut ScalingVariable<L>),
) {
    for name in variables {
        if !settings.iter().any(|s| &s.name == name) {
            settings.push(ScalingVariable::new(name));
        }
    }
    if variables.is_empty() {
        return;
    }
    egui::Grid::new("scaling variables")
        .num_columns(5 + headers.len())
        .show(ui, |ui| {
            ui.strong("Variable");
            ui.strong("Weight");
            for label in headers {
                ui.strong(*label);
            }
            for label in ["Recode", "Missing values", "Supplementary"] {
                ui.strong(label);
            }
            ui.end_row();
            for name in variables {
                let Some(s) = settings.iter_mut().find(|s| &s.name == name) else {
                    continue;
                };
                ui.label(name.as_str());
                small_field(ui, !s.supplementary, &mut s.weight);
                extra(ui, s);
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt((name, "recode"))
                        .selected_text(SCALING_DISCRETIZE[s.discretize].1)
                        .show_index(ui, &mut s.discretize, SCALING_DISCRETIZE.len(), |i| {
                            SCALING_DISCRETIZE[i].1
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
                    .selected_text(SCALING_MISSING[s.missing].1)
                    .show_index(ui, &mut s.missing, SCALING_MISSING.len(), |i| {
                        SCALING_MISSING[i].1
                    });
                ui.checkbox(&mut s.supplementary, "");
                ui.end_row();
            }
        });
    ui.weak("Groups follow a normal curve unless even counts is checked");
}

/// The number of dimensions, the normalization and the iteration criteria.
pub fn model_fields(
    ui: &mut Ui,
    dimensions: &mut String,
    normalization: &mut usize,
    q: &mut String,
    max_iterations: &mut String,
    convergence: &mut String,
) {
    field(ui, "Dimensions in the solution", dimensions, 40.0);
    ui.horizontal(|ui| {
        choice(ui, "Normalization", &SCALING_NORMALIZATIONS, normalization);
        let custom = *normalization == SCALING_NORMALIZATIONS.len() - 1;
        small_field(ui, custom, q);
    });
    ui.weak("A custom value runs from -1 (variable principal) to 1 (object principal)");
    field(ui, "Most iterations", max_iterations, 50.0);
    field(
        ui,
        "Stop when the fit improves less than",
        convergence,
        70.0,
    );
}

/// The lists of the descriptives, quantifications and object score tables, each shown while
/// its table is checked (`on`, in that order).
pub struct PrintListsMut<'a> {
    pub descrip: &'a mut Vec<String>,
    pub quant: &'a mut Vec<String>,
    pub shown: &'a mut Vec<String>,
    pub label: &'a mut String,
}

pub fn print_lists(
    ui: &mut Ui,
    on: [bool; 3],
    (all, analysis): (&[String], &[String]),
    lists: PrintListsMut,
) {
    if on[0] {
        ui.label("Descriptives of");
        ui.push_id("descrip", |ui| name_checkboxes(ui, all, lists.descrip));
    }
    if on[1] {
        ui.label("Quantifications of");
        ui.push_id("quant", |ui| name_checkboxes(ui, analysis, lists.quant));
    }
    if on[2] {
        let label = lists.label;
        ui.horizontal(|ui| {
            ui.label("Label object scores by");
            ComboBox::from_id_salt("object label")
                .selected_text(match all.contains(label) {
                    true => label.as_str(),
                    false => "case number",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(label, String::new(), "case number");
                    for name in all {
                        ui.selectable_value(label, name.clone(), name);
                    }
                });
        });
        ui.label("List beside the object scores");
        ui.push_id("shown", |ui| name_checkboxes(ui, all, lists.shown));
    }
    ui.weak("No variable checked: the analysis variables (none beside the object scores)");
}

/// The dimensions per variable in transformation and residual plots (editable while `per_on`),
/// the longest label and the range of dimensions.
pub fn plot_options(
    ui: &mut Ui,
    (per_on, plot_dimensions): (bool, &mut String),
    label_length: &mut String,
    ndim: &mut bool,
    ndim_first: &mut String,
    ndim_last: &mut String,
) {
    ui.horizontal(|ui| {
        ui.label("Dimensions per variable in transformation and residual plots");
        small_field(ui, per_on, plot_dimensions);
    });
    field(
        ui,
        "Longest label, empty for full labels",
        label_length,
        40.0,
    );
    ui.horizontal(|ui| {
        ui.checkbox(ndim, "Only dimensions from");
        small_field(ui, *ndim, ndim_first);
        ui.label("to");
        small_field(ui, *ndim, ndim_last);
    });
}

/// A checkbox and root per entry of `table` (keyword, label, longest root).
pub fn save_section(
    ui: &mut Ui,
    table: &[(&str, &str, usize)],
    flags: &mut [bool],
    roots: &mut [String],
) {
    for (i, (_, label, longest)) in table.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut flags[i], *label);
            ui.label(format!("root (up to {longest} characters)"));
            small_field(ui, flags[i], &mut roots[i]);
        });
    }
    ui.weak("An empty root uses the command's own");
}

/// A place to write each entry of `table`: nowhere, a data file or a new dataset.
pub fn outfile_section(
    ui: &mut Ui,
    table: &[(&str, &str)],
    choices: &mut [usize],
    (files, datasets): (&mut [String], &mut [String]),
    folder: &mut Option<PathBuf>,
) {
    for (i, (_, label)) in table.iter().enumerate() {
        ui.label(*label);
        ui.push_id(("outfile", i), |ui| {
            output_picker(
                ui,
                &MVA_OUTPUT,
                &mut choices[i],
                &mut files[i],
                &mut datasets[i],
                folder,
            )
        });
    }
}
