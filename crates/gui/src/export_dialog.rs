//! File > Export: one dialog for every way to export an output window.

use crate::i18n::t;
use eframe::egui::{self, Align2, vec2};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Format {
    #[default]
    Excel,
    Html,
    Pdf,
}

/// What the dialog chose; kept for the next export of the session.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Choice {
    pub format: Format,
    /// Excel: all tables on one sheet, one below the other.
    pub one_sheet: bool,
    /// Only the tables selected in the output window.
    pub selected: bool,
    /// Hidden items too; kept in the settings when the export goes ahead.
    pub hidden: bool,
}

/// Excel puts all tables on one sheet unless the dialog says otherwise.
impl Default for Choice {
    fn default() -> Self {
        Choice {
            format: Format::default(),
            one_sheet: true,
            selected: false,
            hidden: false,
        }
    }
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
    egui::Window::new(t("core-export-output"))
        .open(&mut shown)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.strong(t("data-format"));
            ui.radio_value(&mut choice.format, Format::Excel, t("core-excel-xlsx"));
            ui.radio_value(&mut choice.format, Format::Html, t("core-web-html"));
            ui.radio_value(&mut choice.format, Format::Pdf, t("core-pdf-document"));
            ui.add_space(8.0);
            ui.add_enabled_ui(choice.format == Format::Excel, |ui| {
                ui.strong(t("core-excel-sheets"));
                ui.radio_value(&mut choice.one_sheet, false, t("core-one-sheet-per-table"));
                ui.radio_value(&mut choice.one_sheet, true, t("core-all-on-one-sheet"));
            });
            ui.add_space(8.0);
            ui.strong(t("core-items"));
            ui.radio_value(&mut choice.selected, false, t("core-all-items"));
            ui.add_enabled_ui(any_selected, |ui| {
                ui.radio_value(&mut choice.selected, true, t("core-only-selected"));
            });
            ui.checkbox(&mut choice.hidden, t("core-hidden-too"))
                .on_hover_text(t("core-hidden-too-hint"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                export = ui.button(t("menu-export")).clicked();
                cancel = ui.button(t("button-cancel")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape));
            });
        });
    if export || cancel || !shown {
        *open = None;
    }
    export.then_some((window, *choice))
}
