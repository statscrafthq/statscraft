//! Drawing of the time series dialogs.

use crate::app::{choice, target_list};
use crate::stats_dialogs_view::small_field;
use crate::timeseries_dialog::{
    CORRELOGRAM_DISPLAY, Correlogram, CreateRow, CreateSeries, DATE_CASES, Dates, IFFT,
    SERIES_FUNCTIONS, Season, SeriesOptions, TSET_NEWVAR, TSET_PRINT, TimeSeries,
};
use eframe::egui::{self, ComboBox, TextEdit, Ui};

pub fn body(ui: &mut Ui, d: &mut TimeSeries, picked: &mut Vec<String>) {
    match d {
        TimeSeries::Correlogram(d) => correlogram(ui, d, picked),
        TimeSeries::Season(d) => season(ui, d, picked),
        TimeSeries::Create(d) => create(ui, d, picked),
        TimeSeries::Options(d) => options(ui, d),
        TimeSeries::Dates(d) => dates(ui, d),
    }
}

/// A label and a small number field on one line.
fn number(ui: &mut Ui, label: &str, on: bool, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        small_field(ui, on, text);
    });
}

fn correlogram(ui: &mut Ui, d: &mut Correlogram, picked: &mut Vec<String>) {
    target_list(ui, "Series", &mut d.variables, picked);
    if d.cross {
        ui.weak("Each series is paired with each one after it");
    } else {
        ui.horizontal(|ui| {
            ui.label("Show");
            for (i, label) in CORRELOGRAM_DISPLAY.iter().enumerate() {
                ui.radio_value(&mut d.display, i, *label);
            }
        });
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Transform")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut d.ln, "Natural log (before differencing)");
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.diff, "Difference, degree");
                small_field(ui, d.diff, &mut d.diff_order);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.sdiff, "Seasonal difference, degree");
                small_field(ui, d.sdiff, &mut d.sdiff_order);
                ui.label("period");
                small_field(ui, d.sdiff, &mut d.period);
            });
            ui.weak("An empty period takes TSET PERIOD or the date variables");
        });
    egui::CollapsingHeader::new("Options").show(ui, |ui| {
        number(ui, "Most lags", true, &mut d.max_lag);
        ui.weak(if d.cross {
            "Empty: the TSET setting, at first 7"
        } else {
            "Empty: the TSET setting, at first 16"
        });
        if !d.cross {
            ui.add_enabled_ui(d.display != 1, |ui| {
                ui.label("Standard errors of the autocorrelations");
                ui.radio_value(&mut d.ma_errors, false, "Independence (white noise)");
                ui.radio_value(&mut d.ma_errors, true, "Bartlett (moving average)");
            });
        }
    });
}

fn season(ui: &mut Ui, d: &mut Season, picked: &mut Vec<String>) {
    target_list(ui, "Series", &mut d.variables, picked);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Model");
        ui.radio_value(&mut d.additive, false, "Multiplicative");
        ui.radio_value(&mut d.additive, true, "Additive");
    });
    ui.label("Moving average for an even period");
    ui.radio_value(
        &mut d.equal,
        false,
        "Over the period plus 1, half weight at both ends",
    );
    ui.radio_value(&mut d.equal, true, "Over the period, equal weights");
    number(ui, "Period", true, &mut d.period);
    ui.weak("Empty: TSET PERIOD or the date variables");
}

fn create(ui: &mut Ui, d: &mut CreateSeries, picked: &mut Vec<String>) {
    ui.strong("New series");
    if d.rows.is_empty() {
        ui.weak("none yet: pick series, choose a function and add them");
    }
    let mut remove = None;
    for (i, row) in d.rows.iter_mut().enumerate() {
        ui.push_id(i, |ui| create_row(ui, row, &mut remove, i));
    }
    if let Some(i) = remove {
        d.rows.remove(i);
    }
    ui.weak("Names are separated by blanks: FFT makes two, a range of lags one per order");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ComboBox::new("function", "")
            .selected_text(SERIES_FUNCTIONS[d.function].label)
            .show_index(ui, &mut d.function, SERIES_FUNCTIONS.len(), |i| {
                SERIES_FUNCTIONS[i].label
            });
        // IFFT reads the picked series in pairs: a real part, then an imaginary part
        let ifft = d.function == IFFT;
        let ready = !picked.is_empty() && (!ifft || picked.len().is_multiple_of(2));
        if ui
            .add_enabled(ready, egui::Button::new("Add picked series"))
            .clicked()
        {
            let size = if ifft { 2 } else { 1 };
            for sources in picked.chunks(size) {
                d.rows.push(CreateRow::new(d.function, sources.to_vec()));
            }
            picked.clear();
        }
    });
    if d.function == IFFT {
        ui.weak("Pick the real and imaginary parts in pairs");
    }
}

fn create_row(ui: &mut Ui, row: &mut CreateRow, remove: &mut Option<usize>, i: usize) {
    let f = &SERIES_FUNCTIONS[row.function];
    ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut row.names).desired_width(150.0));
        ui.monospace(format!("= {}({})", f.keyword, row.sources.join(", ")));
        if !f.first.is_empty() {
            ui.label(f.first);
            small_field(ui, true, &mut row.first);
        }
        if !f.second.is_empty() {
            ui.label(f.second);
            small_field(ui, true, &mut row.second);
        }
        if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
            *remove = Some(i);
        }
    });
}

fn options(ui: &mut Ui, d: &mut SeriesOptions) {
    ui.weak("Only the settings you change are written; the others keep their session values");
    ui.checkbox(
        &mut d.reset,
        "First put every setting back to its starting value",
    );
    ui.add_space(6.0);
    choice(ui, "New variables", &TSET_NEWVAR, &mut d.newvar);
    number(
        ui,
        "Most new variables per procedure",
        true,
        &mut d.mxnewvars,
    );
    number(ui, "Most autocorrelation lags", true, &mut d.mxauto);
    number(ui, "Most cross-correlation lags", true, &mut d.mxcross);
    number(ui, "Period", true, &mut d.period);
    number(ui, "Confidence level (%)", true, &mut d.cin);
    ui.checkbox(&mut d.include, "Treat user-missing values as valid");
    ui.checkbox(
        &mut d.acfse_ma,
        "Bartlett standard errors for autocorrelations",
    );
    choice(ui, "Output", &TSET_PRINT, &mut d.print);
}

fn dates(ui: &mut Ui, d: &mut Dates) {
    let before = d.case;
    ui.horizontal(|ui| {
        ui.label("Cases are");
        ComboBox::new("cases", "")
            .selected_text(DATE_CASES[d.case].0)
            .show_index(ui, &mut d.case, DATE_CASES.len(), |i| DATE_CASES[i].0);
    });
    if d.case != before {
        d.starts = Default::default();
    }
    ui.add_space(6.0);
    ui.strong("First case");
    for (i, (keyword, label, _, _)) in DATE_CASES[d.case].1.iter().enumerate() {
        if !label.is_empty() {
            number(ui, label, true, &mut d.starts[i]);
        }
        if *keyword == "OBS" {
            number(ui, "Observations per cycle", true, &mut d.period);
        }
    }
    ui.weak("Empty: the first value of its kind. Earlier date variables are replaced.");
}
