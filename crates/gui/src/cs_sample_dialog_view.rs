//! Drawing of the Complex Samples sampling dialogs.

use crate::advanced_dialogs_view::plan_path;
use crate::app::{choice, field, file_dialog, target_list, target_one};
use crate::cs_sample_dialog::{
    LAST_SIMPLE_METHOD, SAMPLING_METHODS, SampleSelection, SamplingPlan, SamplingStage,
};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, TextEdit, Ui};
use std::path::{Path, PathBuf};

pub fn sampling_plan(
    ui: &mut Ui,
    d: &mut SamplingPlan,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    plan_path(ui, &mut d.path, true, folder);
    field(
        ui,
        "Sample weight variable (optional)",
        &mut d.weight_name,
        140.0,
    );
    let mut remove = None;
    let only = d.stages.len() == 1;
    for (i, stage) in d.stages.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("sampling stage", i), |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Stage {}", i + 1));
                ui.label("Label");
                ui.add(TextEdit::singleline(&mut stage.label).desired_width(140.0));
                if ui
                    .add_enabled(!only, egui::Button::new("\u{d7}").small())
                    .on_hover_text("Remove")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
            target_list(ui, "Strata", &mut stage.strata, picked);
            target_list(ui, "Clusters", &mut stage.clusters, picked);
            stage_method(ui, stage, i == 0, picked);
        });
    }
    if let Some(i) = remove {
        d.stages.remove(i);
    }
    if ui
        .add_enabled(d.can_add_stage(), egui::Button::new("Add stage").small())
        .on_disabled_hover_text(
            "A second stage needs clusters in the first one, drawn without replacement",
        )
        .clicked()
    {
        d.stages.push(SamplingStage::default());
    }
}

fn stage_method(ui: &mut Ui, stage: &mut SamplingStage, first: bool, picked: &mut Vec<String>) {
    // later stages draw with equal chances only
    let count = if first {
        SAMPLING_METHODS.len()
    } else {
        LAST_SIMPLE_METHOD + 1
    };
    if stage.method >= count {
        stage.method = 0;
    }
    choice(ui, "Method", &SAMPLING_METHODS[..count], &mut stage.method);
    ui.horizontal(|ui| {
        ui.radio_value(&mut stage.by_rate, false, "Number of units");
        ui.radio_value(&mut stage.by_rate, true, "Share of the units");
        small_field(ui, true, &mut stage.amount);
    });
    if stage.by_rate {
        ui.horizontal(|ui| {
            ui.label("At least");
            small_field(ui, true, &mut stage.min_size);
            ui.label("at most");
            small_field(ui, true, &mut stage.max_size);
            ui.label("units");
        });
    }
    if stage.is_pps() {
        ui.horizontal(|ui| {
            ui.label("Measure of size");
            ui.radio_value(&mut stage.mos_from_data, false, "Variable");
            ui.radio_value(&mut stage.mos_from_data, true, "Cases in the cluster");
        });
        ui.add_enabled_ui(!stage.mos_from_data, |ui| {
            target_one(ui, "Size variable", &mut stage.mos_variable, picked);
        });
        ui.horizontal(|ui| {
            ui.label("Size at least");
            small_field(ui, true, &mut stage.mos_min);
            ui.label("at most");
            small_field(ui, true, &mut stage.mos_max);
        });
    }
    field(
        ui,
        "Inclusion probability variable (optional)",
        &mut stage.inclusion_name,
        140.0,
    );
}

pub fn select_sample(ui: &mut Ui, d: &mut SampleSelection, folder: &mut Option<PathBuf>) {
    plan_path(ui, &mut d.plan, false, folder);
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.radio_value(&mut d.seed_fixed, false, "Random");
        ui.radio_value(&mut d.seed_fixed, true, "Fixed");
        small_field(ui, d.seed_fixed, &mut d.seed);
    });
    ui.add_space(6.0);
    ui.strong("The selected cases");
    ui.radio_value(
        &mut d.to_file,
        false,
        "Add the weights to the data (cases outside the sample have none)",
    );
    ui.radio_value(
        &mut d.to_file,
        true,
        "Write only the selected cases to a file",
    );
    ui.weak("A plan that draws with replacement needs the file");
    ui.add_enabled_ui(d.to_file, |ui| {
        data_file(ui, "File", &mut d.outfile, folder)
    });
    ui.checkbox(
        &mut d.rename,
        "Rename a variable that has the name of a new one",
    );
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Joint probabilities").show(ui, |ui| {
        ui.weak("For plans drawn by PPS without replacement; empty for the default file");
        data_file(ui, "Joint file", &mut d.joint_file, folder);
    });
    ui.checkbox(&mut d.counts, "Show the case counts per stratum");
}

/// A data file to write: a field and Browse.
fn data_file(ui: &mut Ui, label: &str, path: &mut String, folder: &mut Option<PathBuf>) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(TextEdit::singleline(path).desired_width(260.0));
        if ui
            .push_id((label, "browse"), |ui| ui.button("Browse..."))
            .inner
            .clicked()
        {
            let dialog = file_dialog(folder).add_filter("Data file", &["sav"]);
            if let Some(mut chosen) = dialog.save_file() {
                if chosen.extension().is_none() {
                    chosen.set_extension("sav");
                }
                *folder = chosen.parent().map(Path::to_path_buf);
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
}
