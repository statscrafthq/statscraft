//! Drawing of the conjoint dialogs.

use crate::app::{choice, field, file_dialog, target_list};
use crate::conjoint_dialog::{
    CARD_FORMATS, CONJOINT_DIRECTIONS, CONJOINT_MODELS, CONJOINT_PLOTS, CONJOINT_PRINTS,
    CONJOINT_RECORDS, ConjointAnalysis, Design, FactorModel, Level, OrthogonalPlan, PLAN_OUTPUTS,
    PlanCards, PlanFactor,
};
use eframe::egui::{self, TextEdit, Ui};
use statscraft_data::Dataset;
use std::path::{Path, PathBuf};

pub fn body(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut Design,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    match d {
        Design::Generate(d) => generate(ui, d, folder),
        Design::Display(d) => display(ui, d, picked, folder),
        Design::Analysis(d) => analysis(ui, data, d, picked, folder),
    }
}

/// A path with Browse: a data file to write or to read, or a text file to write.
fn path_row(
    ui: &mut Ui,
    label: &str,
    path: &mut String,
    save: bool,
    filter: (&str, &str),
    folder: &mut Option<PathBuf>,
) {
    ui.horizontal(|ui| {
        if !label.is_empty() {
            ui.label(label);
        }
        ui.add(TextEdit::singleline(path).desired_width(240.0));
        if ui
            .push_id(label, |ui| ui.button("Browse..."))
            .inner
            .clicked()
        {
            let dialog = file_dialog(folder).add_filter(filter.0, &[filter.1]);
            let chosen = match save {
                true => dialog.save_file(),
                false => dialog.pick_file(),
            };
            if let Some(mut chosen) = chosen {
                if save && chosen.extension().is_none() {
                    chosen.set_extension(filter.1);
                }
                *folder = chosen.parent().map(Path::to_path_buf);
                *path = chosen.to_string_lossy().into_owned();
            }
        }
    });
}

const DATA_FILE: (&str, &str) = ("Data file", "sav");

/// An active-data or a data-file choice, with the file's path after the second.
fn source_row(
    ui: &mut Ui,
    title: &str,
    from_file: &mut bool,
    path: &mut String,
    folder: &mut Option<PathBuf>,
) {
    ui.strong(title);
    ui.radio_value(from_file, false, "The active data");
    ui.horizontal(|ui| {
        ui.radio_value(from_file, true, "A data file");
        if *from_file {
            path_row(ui, "", path, false, DATA_FILE, folder);
        }
    });
}

fn generate(ui: &mut Ui, d: &mut OrthogonalPlan, folder: &mut Option<PathBuf>) {
    let count = d.factors.len();
    d.at = d.at.min(count - 1);
    ui.horizontal(|ui| {
        ui.strong(format!("Factor {} of {count}", d.at + 1));
        if ui
            .add_enabled(d.at > 0, egui::Button::new("Previous").small())
            .clicked()
        {
            d.at -= 1;
        }
        if ui
            .add_enabled(d.at + 1 < count, egui::Button::new("Next").small())
            .clicked()
        {
            d.at += 1;
        }
        if ui.add(egui::Button::new("New factor").small()).clicked() {
            d.factors.push(PlanFactor::default());
            d.at = d.factors.len() - 1;
        }
        if ui
            .add_enabled(count > 1, egui::Button::new("Remove factor").small())
            .clicked()
        {
            d.factors.remove(d.at);
            d.at = d.at.saturating_sub(1);
        }
    });
    let f = &mut d.factors[d.at];
    field(ui, "Name", &mut f.name, 140.0);
    field(ui, "Label", &mut f.label, 200.0);
    ui.label("Levels: a number and, if you like, a label to show for it");
    let mut remove = None;
    let can_remove = f.levels.len() > 2;
    egui::Grid::new("plan_levels").show(ui, |ui| {
        ui.weak("Value");
        ui.weak("Label");
        ui.end_row();
        for (i, level) in f.levels.iter_mut().enumerate() {
            ui.add(TextEdit::singleline(&mut level.value).desired_width(60.0));
            ui.add(TextEdit::singleline(&mut level.label).desired_width(160.0));
            if ui
                .add_enabled(can_remove, egui::Button::new("\u{d7}").small())
                .on_hover_text("Remove the level")
                .clicked()
            {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        f.levels.remove(i);
    }
    if ui
        .add_enabled(f.levels.len() < 9, egui::Button::new("Add level").small())
        .clicked()
    {
        // the next whole number after the largest value
        let next = f
            .levels
            .iter()
            .filter_map(|l| l.value.trim().parse::<f64>().ok())
            .fold(0.0, f64::max)
            + 1.0;
        f.levels.push(Level {
            value: format!("{next}"),
            label: String::new(),
        });
    }
    ui.add_space(8.0);
    ui.strong("Plan");
    field(ui, "Fewest profiles", &mut d.minimum, 50.0);
    ui.weak("Empty makes the smallest balanced plan; more profiles are made when the plan can have them");
    field(ui, "Holdout profiles", &mut d.holdout, 50.0);
    ui.add_enabled(
        !d.holdout.trim().is_empty(),
        egui::Checkbox::new(
            &mut d.mixed,
            "Mix the holdouts in among the plan's profiles",
        ),
    );
    field(ui, "Random seed", &mut d.seed, 90.0);
    ui.weak("Empty uses the session's seed; the same seed makes the same plan again");
    ui.add_space(8.0);
    ui.strong("Put the plan");
    for (i, label) in PLAN_OUTPUTS.iter().enumerate() {
        ui.radio_value(&mut d.output, i, *label);
    }
    if d.output == 1 {
        path_row(ui, "File", &mut d.file, true, DATA_FILE, folder);
    }
}

fn display(ui: &mut Ui, d: &mut PlanCards, picked: &mut Vec<String>, folder: &mut Option<PathBuf>) {
    ui.label("Shows the profiles of the active data, a plan from Generate or one typed in");
    target_list(
        ui,
        "Factors (empty shows every factor)",
        &mut d.factors,
        picked,
    );
    ui.add_space(6.0);
    choice(ui, "Format", &CARD_FORMATS, &mut d.format);
    ui.add_space(6.0);
    ui.strong("Title (one line each)");
    ui.add(
        TextEdit::multiline(&mut d.title)
            .desired_rows(2)
            .desired_width(300.0),
    );
    ui.strong("Footer (one line each)");
    ui.add(
        TextEdit::multiline(&mut d.footer)
            .desired_rows(2)
            .desired_width(300.0),
    );
    ui.weak("On a card, )CARD in a line is replaced by the number of the profile");
    if d.format > 0 {
        ui.add_space(6.0);
        ui.label("Write the cards to a text file instead of showing them (optional)");
        path_row(ui, "", &mut d.file, true, ("Text file", "txt"), folder);
    }
}

fn analysis(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut ConjointAnalysis,
    picked: &mut Vec<String>,
    folder: &mut Option<PathBuf>,
) {
    source_row(ui, "Plan", &mut d.plan_from_file, &mut d.plan_file, folder);
    d.read_plan();
    ui.add_space(6.0);
    source_row(
        ui,
        "Responses are in",
        &mut d.data_from_file,
        &mut d.data_file,
        folder,
    );
    ui.add_space(6.0);
    ui.strong("How the responses are recorded");
    for (i, (_, label)) in CONJOINT_RECORDS.iter().enumerate() {
        ui.radio_value(&mut d.record, i, *label);
    }
    typed_names(ui, "Response variables", &mut d.responses, picked, true);
    ui.weak("One for each profile of the plan that is rated, in the plan's order; TO is allowed");
    typed_names(
        ui,
        "Subject variable (optional)",
        &mut d.subject,
        picked,
        false,
    );
    ui.weak("Consecutive cases with one value are one subject; without it all cases are one");
    ui.add_space(6.0);
    ui.strong("Factors");
    ui.weak("A factor left out is analysed as discrete");
    let levels = d.plan_levels(data).unwrap_or_default();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !levels.is_empty(),
                egui::Button::new("Add the plan's factors").small(),
            )
            .clicked()
        {
            for (name, _) in &levels {
                if !d.factors.iter().any(|f| f.name.eq_ignore_ascii_case(name)) {
                    d.factors.push(FactorModel {
                        name: name.clone(),
                        ..Default::default()
                    });
                }
            }
        }
        if ui.add(egui::Button::new("New factor").small()).clicked() {
            d.factors.push(FactorModel::default());
        }
    });
    let mut remove = None;
    for (i, f) in d.factors.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut f.name).desired_width(90.0));
                choice(ui, "", &CONJOINT_MODELS, &mut f.model);
                ui.add_enabled_ui(f.model < 2, |ui| {
                    choice(ui, " ", &CONJOINT_DIRECTIONS, &mut f.direction);
                });
                if ui
                    .add(egui::Button::new("\u{d7}").small())
                    .on_hover_text("Remove")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        d.factors.remove(i);
    }
    ui.add_space(8.0);
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        choice(ui, "Print", &CONJOINT_PRINTS, &mut d.print);
        ui.checkbox(
            &mut d.summary_only,
            "Only the group's tables, not each subject's",
        );
        choice(ui, "Charts", &CONJOINT_PLOTS, &mut d.plot);
        ui.label("Save the utilities of each subject to a data file (optional)");
        path_row(ui, "", &mut d.utility, true, DATA_FILE, folder);
    });
}

/// Names typed in a field, with Add to put the picked variables in: after what is there
/// (`many`) or in its place.
fn typed_names(ui: &mut Ui, title: &str, text: &mut String, picked: &mut Vec<String>, many: bool) {
    ui.push_id(title, |ui| {
        ui.horizontal(|ui| {
            ui.strong(title);
            if ui
                .add_enabled(!picked.is_empty(), egui::Button::new("Add picked").small())
                .clicked()
            {
                let names: Vec<String> = if many {
                    std::mem::take(picked)
                } else {
                    vec![picked.remove(0)]
                };
                if many && !text.trim().is_empty() {
                    text.push(' ');
                } else {
                    text.clear();
                }
                text.push_str(&names.join(" "));
            }
        });
        ui.add(TextEdit::singleline(text).desired_width(300.0));
    });
}
