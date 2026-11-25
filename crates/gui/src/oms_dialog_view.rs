//! Drawing of the Output Management dialogs.

use crate::app::{field, file_dialog};
use crate::i18n::t;
use crate::oms_dialog::{OMS_FORMATS, OMS_KINDS, Oms, Omsend};
use eframe::egui::{self, ComboBox, Ui};

pub fn oms(ui: &mut Ui, d: &mut Oms) {
    ui.strong(t("core-send-this-output"));
    for ((_, label), flag) in OMS_KINDS.iter().zip(&mut d.kinds) {
        ui.checkbox(flag, t(label));
    }
    ui.weak(t("core-oms-lasts"));
    ui.add_space(6.0);
    egui::CollapsingHeader::new(t("core-only-these-commands"))
        .default_open(true)
        .show(ui, |ui| {
            field(ui, t("core-commands"), &mut d.commands, 320.0);
            ui.weak(t("core-oms-commands-hint"));
            field(ui, t("settings-part-tables"), &mut d.subtypes, 320.0);
            ui.weak(t("core-oms-tables-hint"));
        });
    ui.add_space(6.0);
    ui.strong(t("core-destination"));
    ui.horizontal(|ui| {
        ui.label(t("data-format"));
        ComboBox::new("oms format", "")
            .selected_text(t(OMS_FORMATS[d.format].1))
            .show_index(
                ui,
                &mut d.format,
                OMS_FORMATS.len(),
                |i| t(OMS_FORMATS[i].1),
            );
    });
    ui.horizontal(|ui| {
        ui.label(t("core-file"));
        if ui.button(t("core-browse")).clicked()
            && let Some(chosen) = file_dialog(&None)
                .add_filter(t(OMS_FORMATS[d.format].1), OMS_FORMATS[d.format].2)
                .save_file()
        {
            d.file = chosen.to_string_lossy().into_owned();
        }
    });
    ui.add(egui::TextEdit::singleline(&mut d.file).desired_width(360.0));
    ui.weak(t("core-oms-no-file"));
    ui.checkbox(&mut d.viewer, t("core-oms-viewer"));
    field(ui, t("core-tag"), &mut d.tag, 120.0);
    ui.weak(t("core-tag-hint"));
}

pub fn omsend(ui: &mut Ui, d: &mut Omsend) {
    field(ui, t("core-tags"), &mut d.tags, 260.0);
    ui.weak(t("core-tags-hint"));
    field(ui, t("core-files"), &mut d.files, 260.0);
    ui.weak(t("core-files-hint"));
    ui.add_space(4.0);
    ui.weak(t("core-both-empty"));
}
