//! Drawing of the Naive Bayes dialog.

use crate::app::{field, file_dialog, target_list, target_one};
use crate::naivebayes_dialog::{NB_BEST, NB_PARTITIONS, NB_PRINT, NB_SELECTION, NaiveBayes};
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, Ui};

fn combo(ui: &mut Ui, label: &str, options: &[&str], index: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(label);
        ComboBox::new(label, "")
            .selected_text(options[*index])
            .show_index(ui, index, options.len(), |i| options[i]);
    });
}

pub fn naive_bayes(ui: &mut Ui, d: &mut NaiveBayes, picked: &mut Vec<String>) {
    target_one(ui, "Target", &mut d.target, picked);
    target_list(ui, "Factors", &mut d.factors, picked);
    target_list(ui, "Covariates", &mut d.covariates, picked);
    ui.weak("With no factors and covariates, every other variable is a predictor");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Predictors").show(ui, |ui| {
        combo(ui, "Selection", &NB_SELECTION, &mut d.selection);
        ui.horizontal(|ui| {
            ui.label("Number of predictors");
            small_field(ui, matches!(d.selection, 1 | 2), &mut d.size);
        });
        combo(ui, "Best subset by", &NB_BEST, &mut d.best);
        ui.horizontal(|ui| {
            ui.label("Intervals for a scale predictor");
            small_field(ui, true, &mut d.bins);
        });
        ui.weak("Empty: 10");
    });
    egui::CollapsingHeader::new("Partition").show(ui, |ui| {
        combo(ui, "Training cases", &NB_PARTITIONS, &mut d.partition);
        match d.partition {
            1 => field(ui, "Training percent", &mut d.percent, 50.0),
            2 => target_one(ui, "Partition variable", &mut d.partition_variable, picked),
            _ => {}
        }
        ui.weak("The cases left out of training test the model");
    });
    egui::CollapsingHeader::new("Output").show(ui, |ui| {
        for ((_, label), flag) in NB_PRINT.iter().zip(&mut d.print) {
            ui.checkbox(flag, *label);
        }
        ui.checkbox(&mut d.include_missing, "Keep user-missing categories");
    });
    egui::CollapsingHeader::new("Save").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.save_value, "Predicted category");
            ui.add_enabled(
                d.save_value,
                egui::TextEdit::singleline(&mut d.value_name)
                    .hint_text("default name")
                    .desired_width(110.0),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.save_probability, "Predicted probabilities");
            ui.add_enabled(
                d.save_probability,
                egui::TextEdit::singleline(&mut d.probability_root)
                    .hint_text("default root")
                    .desired_width(110.0),
            );
            ui.label("for the first");
            small_field(ui, d.save_probability, &mut d.probability_count);
            ui.label("categories");
        });
        ui.horizontal(|ui| {
            ui.strong("Model file");
            if ui.button("Browse...").clicked()
                && let Some(mut chosen) = file_dialog(&None)
                    .add_filter("Model file", &["json"])
                    .save_file()
            {
                if chosen.extension().is_none() {
                    chosen.set_extension("json");
                }
                d.model_file = chosen.to_string_lossy().into_owned();
            }
        });
        ui.add(egui::TextEdit::singleline(&mut d.model_file).desired_width(360.0));
        ui.weak("Empty: the model is not saved");
    });
}
