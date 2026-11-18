//! Drawing of the Output Management dialogs.

use crate::app::{field, file_dialog};
use crate::oms_dialog::{OMS_FORMATS, OMS_KINDS, Oms, Omsend};
use eframe::egui::{self, ComboBox, Ui};

pub fn oms(ui: &mut Ui, d: &mut Oms) {
    ui.strong("Send this output");
    for ((_, label), flag) in OMS_KINDS.iter().zip(&mut d.kinds) {
        ui.checkbox(flag, *label);
    }
    ui.weak("The request lasts until it is ended (File > Output Management > End)");
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Only these commands and tables")
        .default_open(true)
        .show(ui, |ui| {
            field(ui, "Commands", &mut d.commands, 320.0);
            ui.weak("Names such as Frequencies, separated by commas; empty: every command");
            field(ui, "Tables", &mut d.subtypes, 320.0);
            ui.weak("Table titles, separated by commas; a trailing * matches the rest of a title");
        });
    ui.add_space(6.0);
    ui.strong("Destination");
    ui.horizontal(|ui| {
        ui.label("Format");
        ComboBox::new("oms format", "")
            .selected_text(OMS_FORMATS[d.format].1)
            .show_index(ui, &mut d.format, OMS_FORMATS.len(), |i| OMS_FORMATS[i].1);
    });
    ui.horizontal(|ui| {
        ui.label("File");
        if ui.button("Browse...").clicked()
            && let Some(chosen) = file_dialog(&None)
                .add_filter(OMS_FORMATS[d.format].1, OMS_FORMATS[d.format].2)
                .save_file()
        {
            d.file = chosen.to_string_lossy().into_owned();
        }
    });
    ui.add(egui::TextEdit::singleline(&mut d.file).desired_width(360.0));
    ui.weak("Empty: no file, only the results are changed");
    ui.checkbox(&mut d.viewer, "Also show the output in the results");
    field(ui, "Tag", &mut d.tag, 120.0);
    ui.weak("A name to end the request by; empty: none");
}

pub fn omsend(ui: &mut Ui, d: &mut Omsend) {
    field(ui, "Tags", &mut d.tags, 260.0);
    ui.weak("Requests with these tags end, separated by commas");
    field(ui, "Files", &mut d.files, 260.0);
    ui.weak("Requests writing these files end, separated by commas");
    ui.add_space(4.0);
    ui.weak("Both empty: every request ends");
}
