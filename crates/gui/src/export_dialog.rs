//! File > Export: one dialog for every way to export an output window.

use eframe::egui::{self, Align2, vec2};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Format {
    #[default]
    Excel,
    Html,
    Pdf,
}

/// What the dialog chose; kept for the next export of the session.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Choice {
    pub format: Format,
    /// Excel: all tables on one sheet, one below the other.
    pub one_sheet: bool,
    /// Only the tables selected in the output window.
    pub selected: bool,
    /// Hidden items too; kept in the settings when the export goes ahead.
    pub hidden: bool,
}

/// Draws the dialog while `open` holds the output window it exports; returns the choice when
/// Export is pressed.
pub fn show<W: Copy>(
    ctx: &egui::Context,
    open: &mut Option<W>,
    choice: &mut Choice,
    any_selected: bool,
) -> Option<(W, Choice)> {
    let window = (*open)?;
    if !any_selected {
        choice.selected = false;
    }
    let mut shown = true;
    let mut export = false;
    let mut cancel = false;
    egui::Window::new("Export output")
        .open(&mut shown)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.strong("Format");
            ui.radio_value(&mut choice.format, Format::Excel, "Excel workbook (.xlsx)");
            ui.radio_value(&mut choice.format, Format::Html, "Web page (.html)");
            ui.radio_value(&mut choice.format, Format::Pdf, "PDF document (.pdf)");
            ui.add_space(8.0);
            ui.add_enabled_ui(choice.format == Format::Excel, |ui| {
                ui.strong("Excel sheets");
                ui.radio_value(&mut choice.one_sheet, false, "One sheet per table");
                ui.radio_value(&mut choice.one_sheet, true, "All tables on one sheet");
            });
            ui.add_space(8.0);
            ui.strong("Items");
            ui.radio_value(&mut choice.selected, false, "All items of the output");
            ui.add_enabled_ui(any_selected, |ui| {
                ui.radio_value(&mut choice.selected, true, "Only the selected tables");
            });
            ui.checkbox(&mut choice.hidden, "Hidden items too")
                .on_hover_text("Off: only what the output shows. Copies follow this too.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                export = ui.button("Export...").clicked();
                cancel =
                    ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
            });
        });
    if export || cancel || !shown {
        *open = None;
    }
    export.then_some((window, *choice))
}
