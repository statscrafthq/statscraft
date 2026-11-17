//! Drawing of the time series dialogs.

use crate::app::{choice, field, target_list, target_one};
use crate::stats_dialogs_view::small_field;
use crate::timeseries_dialog::{
    ARIMA_APPLY, ARIMA_CONSTANT, ARIMA_FORECAST, ARIMA_LAGS, ARIMA_LOG, ArimaModel, CHART_JOIN,
    CORRELOGRAM_DISPLAY, Correlogram, CreateRow, CreateSeries, DATE_CASES, Dates, ExpSmoothing,
    IFFT, Parameter, SERIES_FUNCTIONS, SMOOTHING_MODELS, SMOOTHING_PARAMETERS, Season, SeriesChart,
    SeriesOptions, TSET_NEWVAR, TSET_PRINT, TimeRange, TimeSeries, date_units,
};
use eframe::egui::{self, ComboBox, TextEdit, Ui};
use statscraft_data::Dataset;

pub fn body(ui: &mut Ui, data: &Dataset, d: &mut TimeSeries, picked: &mut Vec<String>) {
    match d {
        TimeSeries::Correlogram(d) => correlogram(ui, d, picked),
        TimeSeries::Season(d) => season(ui, d, picked),
        TimeSeries::Create(d) => create(ui, d, picked),
        TimeSeries::Options(d) => options(ui, d),
        TimeSeries::Dates(d) => dates(ui, d),
        TimeSeries::Smoothing(d) => smoothing(ui, d, picked),
        TimeSeries::Arima(d) => arima(ui, d, picked),
        TimeSeries::Range(d) => range(ui, data, d),
        TimeSeries::Chart(d) => chart(ui, d, picked),
        TimeSeries::Spectra(d) => crate::forecast_more_dialog_view::spectra(ui, d, picked),
        TimeSeries::Areg(d) => crate::forecast_more_dialog_view::areg(ui, d, picked),
        TimeSeries::TwoSls(d) => crate::forecast_more_dialog_view::two_sls(ui, d, picked),
    }
}

/// The log and differencing of the correlograms and the charts.
fn transform(
    ui: &mut Ui,
    ln: &mut bool,
    diff: &mut bool,
    diff_order: &mut String,
    sdiff: &mut bool,
    sdiff_order: &mut String,
    period: &mut String,
) {
    egui::CollapsingHeader::new("Transform")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(ln, "Natural log (before differencing)");
            ui.horizontal(|ui| {
                ui.checkbox(diff, "Difference, degree");
                small_field(ui, *diff, diff_order);
            });
            ui.horizontal(|ui| {
                ui.checkbox(sdiff, "Seasonal difference, degree");
                small_field(ui, *sdiff, sdiff_order);
                ui.label("period");
                small_field(ui, *sdiff, period);
            });
            ui.weak("An empty period takes TSET PERIOD or the date variables");
        });
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
    transform(
        ui,
        &mut d.ln,
        &mut d.diff,
        &mut d.diff_order,
        &mut d.sdiff,
        &mut d.sdiff_order,
        &mut d.period,
    );
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

/// What APPLY does, under its checkbox.
const APPLY_NOTE: &str = "What you set here replaces its settings; the rest is kept";

fn smoothing(ui: &mut Ui, d: &mut ExpSmoothing, picked: &mut Vec<String>) {
    target_list(ui, "Series", &mut d.variables, picked);
    ui.add_space(6.0);
    choice(ui, "Model", &SMOOTHING_MODELS, &mut d.model);
    if d.seasonal() {
        number(ui, "Period", true, &mut d.period);
        ui.weak("Empty: TSET PERIOD or the date variables");
        field(ui, "Initial seasonal factors", &mut d.factors, 200.0);
        ui.weak(
            "Empty: computed from the series. Else one number per case of the period, or a \
             variable that holds them, such as one SEASON saved",
        );
    }
    ui.add_space(6.0);
    ui.strong("Smoothing parameters");
    let uses = d.uses();
    for (((_, label), p), used) in SMOOTHING_PARAMETERS.iter().zip(&mut d.parameters).zip(uses) {
        if used {
            ui.push_id(label, |ui| parameter(ui, label, p));
        }
    }
    ui.weak(
        "The model uses only the parameters shown. A grid with empty fields takes its own range.",
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.initial, "Start level");
        small_field(ui, d.initial, &mut d.start);
        ui.label("and trend");
        small_field(ui, d.initial, &mut d.trend);
    });
    ui.weak("Off: computed from the first cases");
    ui.checkbox(&mut d.apply, "Apply the previous exponential smoothing");
    ui.weak(APPLY_NOTE);
}

/// One smoothing parameter: a value, or a grid from, to and by.
fn parameter(ui: &mut Ui, label: &str, p: &mut Parameter) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.radio_value(&mut p.grid, false, "Value");
        small_field(ui, !p.grid, &mut p.value);
        ui.radio_value(&mut p.grid, true, "Grid from");
        small_field(ui, p.grid, &mut p.from);
        ui.label("to");
        small_field(ui, p.grid, &mut p.to);
        ui.label("by");
        small_field(ui, p.grid, &mut p.step);
    });
}

fn arima(ui: &mut Ui, d: &mut ArimaModel, picked: &mut Vec<String>) {
    target_one(ui, "Dependent series", &mut d.dependent, picked);
    target_list(ui, "Regressors", &mut d.regressors, picked);
    ui.add_space(6.0);
    ui.strong("Model");
    egui::Grid::new("orders").show(ui, |ui| {
        ui.label("");
        for heading in ["Autoregressive", "Difference", "Moving average"] {
            ui.label(heading);
        }
        ui.end_row();
        for (label, orders) in [
            ("Nonseasonal", &mut d.orders),
            ("Seasonal", &mut d.seasonal),
        ] {
            ui.label(label);
            for order in orders.iter_mut() {
                small_field(ui, true, order);
            }
            ui.end_row();
        }
    });
    number(ui, "Period of the seasonal part", true, &mut d.period);
    ui.weak("Empty: TSET PERIOD or the date variables");
    choice(ui, "Constant", &ARIMA_CONSTANT, &mut d.constant);
    choice(ui, "Transform the dependent", &ARIMA_LOG, &mut d.log);
    egui::CollapsingHeader::new("Lags to fit").show(ui, |ui| {
        for ((_, label), lags) in ARIMA_LAGS.iter().zip(&mut d.lags) {
            field(ui, label, lags, 120.0);
        }
        ui.weak("Only these lags, separated by blanks; empty: every lag up to the order");
    });
    egui::CollapsingHeader::new("Criteria").show(ui, |ui| {
        number(ui, "Most iterations", true, &mut d.mxiter);
        number(ui, "Largest Marquardt constant", true, &mut d.mxlamb);
        number(
            ui,
            "Stop when the sum of squares changes less than (%)",
            true,
            &mut d.ssqpct,
        );
        number(ui, "Stop when no estimate changes by", true, &mut d.pareps);
        number(ui, "Confidence level (%)", true, &mut d.cinpct);
        ui.weak("Empty change or level: the TSET setting");
    });
    choice(
        ui,
        "Fitted values and forecasts",
        &ARIMA_FORECAST,
        &mut d.forecast,
    );
    choice(ui, "Apply the previous ARIMA", &ARIMA_APPLY, &mut d.apply);
    if d.apply > 0 {
        ui.weak(APPLY_NOTE);
    }
}

fn range(ui: &mut Ui, data: &Dataset, d: &mut TimeRange) {
    let units = date_units(data);
    if units.is_empty() {
        d.by_date = false;
    }
    ui.label(if d.predict {
        "Exponential smoothing and ARIMA forecast these cases. When they save variables, \
         cases past the end of the file are added to hold the forecasts."
    } else {
        "The time series procedures that follow use only these cases; the others stay in the \
         file unless you delete them."
    });
    ui.horizontal(|ui| {
        ui.label("Give the range by");
        ui.radio_value(&mut d.by_date, false, "Case numbers");
        ui.add_enabled_ui(!units.is_empty(), |ui| {
            ui.radio_value(&mut d.by_date, true, "Dates");
        });
    });
    if units.is_empty() {
        ui.weak("Dates need date variables: Data > Define Dates");
    }
    if d.by_date {
        egui::Grid::new("dates").show(ui, |ui| {
            ui.label("");
            for unit in units.iter().take(4) {
                ui.label(unit.name());
            }
            ui.end_row();
            for (label, values) in [("Start", &mut d.start_date), ("End", &mut d.end_date)] {
                ui.label(label);
                for value in values.iter_mut().take(units.len()) {
                    small_field(ui, true, value);
                }
                ui.end_row();
            }
        });
        ui.weak(format!(
            "A date needs its {}; the parts after it may stay empty",
            units[0].name()
        ));
    } else {
        number(ui, "Start case", true, &mut d.start);
        number(ui, "End case", true, &mut d.end);
    }
    if d.predict {
        ui.weak("Empty start: right after the historical period (USE). Empty end: the last case.");
        return;
    }
    ui.weak("Empty start: the first case. Empty end: the last case. Both empty: every case.");
    let open = match d.by_date {
        true => d
            .start_date
            .iter()
            .chain(&d.end_date)
            .all(|t| t.trim().is_empty()),
        false => d.start.trim().is_empty() && d.end.trim().is_empty(),
    };
    if open {
        d.permanent = false;
    }
    ui.add_enabled(
        !open,
        egui::Checkbox::new(&mut d.permanent, "Delete the cases outside the range"),
    );
}

fn chart(ui: &mut Ui, d: &mut SeriesChart, picked: &mut Vec<String>) {
    target_list(ui, "Series", &mut d.variables, picked);
    target_one(ui, "Case labels", &mut d.id, picked);
    ui.weak("Empty: TSET ID, the date variables or the case numbers");
    target_one(ui, "Mark where this variable changes", &mut d.mark, picked);
    ui.add_space(6.0);
    transform(
        ui,
        &mut d.ln,
        &mut d.diff,
        &mut d.diff_order,
        &mut d.sdiff,
        &mut d.sdiff_order,
        &mut d.period,
    );
    egui::CollapsingHeader::new("Format")
        .default_open(true)
        .show(ui, |ui| match d.variables.len() {
            0 => {
                ui.weak("The choices depend on the number of series");
            }
            1 => {
                ui.checkbox(
                    &mut d.fill,
                    if d.casewise {
                        "Fill the area left of the line"
                    } else {
                        "Fill the area under the line"
                    },
                );
                ui.horizontal(|ui| {
                    ui.label("Reference line");
                    ui.radio_value(&mut d.reference, 0, "None");
                    ui.radio_value(&mut d.reference, 1, "At the mean");
                    ui.radio_value(&mut d.reference, 2, "At");
                    small_field(ui, d.reference == 2, &mut d.reference_value);
                });
            }
            _ => choice(ui, "Several series", &CHART_JOIN, &mut d.join),
        });
    ui.checkbox(&mut d.apply, "Apply the previous chart of this kind");
    ui.weak(APPLY_NOTE);
}
