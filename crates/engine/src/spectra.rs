//! SPECTRA: the periodogram and smoothed spectral density of series, and the cross-spectral
//! statistics of pairs.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - A series has no missing values and at least six cases; of an odd number of cases the
//!   first is left out. CENTER takes the mean off first.
//! - At the frequencies K / n, K = 0 .. n / 2: cosine and sine terms a_K and b_K, (2 / n)
//!   times the sums of x_t cos and x_t sin of 2 pi K (t - 1) / n; periodogram (a_K^2 + b_K^2)
//!   n / 2, which at frequency 0 is 2 n times the squared mean.
//! - The density smooths the periodogram with the weights of a window over its span (an even
//!   span less one, half-width p the integer part of span / 2). Past the ends the values are
//!   mirrored: l_-K = l_K, l_0 = l_1 and l_K = l_(n + 1 - K) above n / 2.
//! - The windows: Tukey-Hamming 0.54 D_p(t) + 0.23 D_p(t + pi / p) + 0.23 D_p(t - pi / p),
//!   Tukey-Hanning with 0.5, 0.25 and 0.25, Parzen (2 + cos t) F_(p/2)(t)^2 / p, Bartlett
//!   F_p(t), Daniell (UNIT) 1, where D is the Dirichlet and F the Fejer kernel. Own weights
//!   run from w_-p to w_p (an even count lacks w_p); w_0 must be above 0.
//! - Pairs (CROSS): the first series against each later one. Cross-periodogram real part
//!   (n / 2)(a_x a_y + b_x b_y), imaginary part (n / 2)(a_x b_y - b_x a_y); cospectrum and
//!   quadrature spectrum smooth them; cross amplitude sqrt(C^2 + Q^2); squared coherency
//!   A^2 / (s_x s_y); gain A / s_x; phase the angle of (C, Q).
//! - SAVE: one variable per series (root_n, n its place on VARIABLES) or pair (root_1_n);
//!   FREQ and PER keep the root. A name that exists already is not saved.
//!
//! Choices where the manual is silent:
//! - The window kernels are evaluated at t = 2 pi k / m for k = 0 .. p, m the span (the
//!   Fourier frequencies of the span itself), and every window, own weights too, is divided
//!   by its sum, so the density is a weighted moving average of the periodogram. This is the
//!   reading under which the Fejer and Dirichlet kernels reproduce those of a general
//!   statistics package run as a black box, and the four named windows sum to the span.
//! - The cosine term at frequency 0 is twice the mean, from the same formula as the others.
//! - The cross-periodogram's two parts are mirrored at the ends like the periodogram.
//! - Charts are data: the periodogram and the density on natural logarithms, the other
//!   statistics as they are; by period from the longest period down, without frequency 0.
//! - The saved values start at the first case the series uses; the cases after the last
//!   frequency are system-missing.

use crate::chart::{Axis, Category, Series, Statistic};
use crate::descriptives::number;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::read_series;
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::Measure;
use statscraft_syntax::{
    Spectra, SpectraBy, SpectraItem, SpectraPlot, SpectraSmoothing, SpectraWindow,
};
use std::f64::consts::PI;

/// Dirichlet kernel of order q at t.
fn dirichlet(q: f64, t: f64) -> f64 {
    let half = (t / 2.0).sin();
    if half.abs() < 1e-12 {
        2.0 * q + 1.0
    } else {
        ((2.0 * q + 1.0) * t / 2.0).sin() / half
    }
}

/// Fejer kernel of order q at t.
fn fejer(q: f64, t: f64) -> f64 {
    let half = (t / 2.0).sin();
    if half.abs() < 1e-12 {
        q
    } else {
        ((q * t / 2.0).sin() / half).powi(2) / q
    }
}

/// The weights w_-p .. w_p of a window, summing to 1; None for no smoothing.
pub(crate) fn window_weights(smoothing: &SpectraSmoothing) -> Result<Option<Vec<f64>>, Error> {
    let raw: Vec<f64> = match smoothing {
        SpectraSmoothing::Weights(given) => {
            let mut w = given.clone();
            if w.len() % 2 == 0 {
                // the last weight w_p is not given
                w.push(0.0);
            }
            if w[w.len() / 2] <= 0.0 {
                return Err(Error(
                    "SPECTRA: the weight of the value being smoothed (the middle one) must be above 0"
                        .into(),
                ));
            }
            w
        }
        SpectraSmoothing::Named(SpectraWindow::None, _) => return Ok(None),
        SpectraSmoothing::Named(kind, span) => {
            let span = span.unwrap_or(5);
            if span < 1 {
                return Err(Error(format!(
                    "SPECTRA: the span of the window is at least 1, got {span}"
                )));
            }
            let m = if span % 2 == 0 { span - 1 } else { span };
            if m == 1 {
                return Ok(None);
            }
            let p = (m / 2) as usize;
            let pf = p as f64;
            let upper: Vec<f64> = (0..=p)
                .map(|k| {
                    let t = 2.0 * PI * k as f64 / m as f64;
                    let shifted = |c0: f64, c1: f64| {
                        c0 * dirichlet(pf, t)
                            + c1 * (dirichlet(pf, t + PI / pf) + dirichlet(pf, t - PI / pf))
                    };
                    match kind {
                        SpectraWindow::Hamming => shifted(0.54, 0.23),
                        SpectraWindow::Tukey => shifted(0.5, 0.25),
                        SpectraWindow::Parzen => (2.0 + t.cos()) * fejer(pf / 2.0, t).powi(2) / pf,
                        SpectraWindow::Bartlett => fejer(pf, t),
                        SpectraWindow::Daniell | SpectraWindow::None => 1.0,
                    }
                })
                .collect();
            upper[1..].iter().rev().chain(&upper).copied().collect()
        }
    };
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        return Err(Error(
            "SPECTRA: the window weights must have a sum above 0".into(),
        ));
    }
    Ok(Some(raw.iter().map(|w| w / sum).collect()))
}

/// The cosine and sine terms a_K, b_K for K = 0 .. n / 2 (n even).
pub(crate) fn fourier_terms(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len();
    let (cos, sin): (Vec<f64>, Vec<f64>) = (0..n)
        .map(|j| {
            let angle = 2.0 * PI * j as f64 / n as f64;
            (angle.cos(), angle.sin())
        })
        .unzip();
    (0..=n / 2)
        .map(|k| {
            let (mut a, mut b) = (0.0, 0.0);
            for (t, v) in x.iter().enumerate() {
                let j = (k * t) % n;
                a += v * cos[j];
                b += v * sin[j];
            }
            (2.0 * a / n as f64, 2.0 * b / n as f64)
        })
        .unzip()
}

/// Smooths values at K = 0 .. q with weights w_-p .. w_p, mirroring past the ends.
pub(crate) fn smooth(values: &[f64], weights: Option<&[f64]>, n: usize) -> Vec<f64> {
    let Some(w) = weights else {
        return values.to_vec();
    };
    let q = values.len() as i64 - 1;
    let p = (w.len() / 2) as i64;
    let at = |mut k: i64| -> f64 {
        loop {
            if k < 0 {
                k = -k;
            } else if k == 0 {
                // the value at frequency 0 is read as the one at the first frequency
                k = 1.min(q);
                if k == 0 {
                    break;
                }
            } else if k > q {
                // as the Algorithms manual prints it (l_K = l_(n + 1 - K)), not the n - K of
                // the periodogram's own symmetry
                k = n as i64 + 1 - k;
            } else {
                break;
            }
        }
        values[k as usize]
    };
    (0..=q)
        .map(|k| (-p..=p).map(|j| w[(j + p) as usize] * at(k + j)).sum())
        .collect()
}

/// The univariate results of one series.
struct Univariate {
    a: Vec<f64>,
    b: Vec<f64>,
    periodogram: Vec<f64>,
    density: Vec<f64>,
}

const ITEM_NAMES: [(SpectraItem, &str); 14] = [
    (SpectraItem::Freq, "Frequency"),
    (SpectraItem::Per, "Period"),
    (SpectraItem::Sin, "Sine Transform"),
    (SpectraItem::Cos, "Cosine Transform"),
    (SpectraItem::P, "Periodogram"),
    (SpectraItem::S, "Spectral Density Estimate"),
    (SpectraItem::Rc, "Cross-Periodogram (Real)"),
    (SpectraItem::Ic, "Cross-Periodogram (Imaginary)"),
    (SpectraItem::Cs, "Cospectral Density Estimate"),
    (SpectraItem::Qs, "Quadrature Spectrum Estimate"),
    (SpectraItem::Ph, "Phase Spectrum Estimate"),
    (SpectraItem::A, "Cross-Amplitude"),
    (SpectraItem::G, "Gain"),
    (SpectraItem::K, "Squared Coherency"),
];

fn item_name(item: SpectraItem) -> &'static str {
    ITEM_NAMES
        .iter()
        .find(|(i, _)| *i == item)
        .map(|(_, n)| *n)
        .expect("every item is named")
}

impl Engine {
    /// The command over the previous SPECTRA for APPLY; SAVE is never carried over.
    pub(crate) fn spectra_spec(&mut self, command: &Spectra) -> Result<Spectra, Error> {
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "SPECTRA APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous SPECTRA"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.spectra else {
                    return Err(Error(
                        "SPECTRA APPLY: there is no earlier SPECTRA to apply".into(),
                    ));
                };
                let mut given = (**previous).clone();
                if !command.variables.is_empty() {
                    given.variables.clone_from(&command.variables);
                }
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field.clone();
                        })*
                    };
                }
                over!(center, cross, window, plot);
                given.save.clone_from(&command.save);
                given
            }
        };
        given.apply = None;
        Ok(given)
    }

    pub(crate) fn spectra(&mut self, spec: &Spectra) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&spec.variables)?;
        let include = self.tset.include;
        let center = spec.center.unwrap_or(false);
        let cross = spec.cross.unwrap_or(false) && names.len() > 1;
        let smoothing = spec
            .window
            .clone()
            .unwrap_or(SpectraSmoothing::Named(SpectraWindow::Hamming, Some(5)));
        let weights = window_weights(&smoothing)?;
        let mut warnings = Vec::new();
        if spec.cross == Some(true) && names.len() < 2 {
            warnings.push("SPECTRA: CROSS needs at least two series; it is ignored".to_string());
        }
        let cases = data.case_count();
        let mut series = Vec::new();
        for name in &names {
            let (values, _, _) = read_series(data, name, include, "SPECTRA")?;
            if values.iter().any(Option::is_none) {
                return Err(Error(format!(
                    "SPECTRA: {name} has missing values; replace them (RMV) or leave the cases \
                     out with USE"
                )));
            }
            series.push(values.into_iter().flatten().collect::<Vec<f64>>());
        }
        if cases < 6 {
            return Err(Error(format!(
                "SPECTRA needs at least six cases, but the series have {cases}"
            )));
        }
        // of an odd number of cases the first is left out
        let skip = cases % 2;
        let n = cases - skip;
        let q = n / 2;
        let univariate: Vec<Univariate> = series
            .iter()
            .map(|x| {
                let mut x = x[skip..].to_vec();
                if center {
                    let mean = x.iter().sum::<f64>() / n as f64;
                    x.iter_mut().for_each(|v| *v -= mean);
                }
                let (a, b) = fourier_terms(&x);
                let periodogram: Vec<f64> = a
                    .iter()
                    .zip(&b)
                    .map(|(a, b)| (a * a + b * b) * n as f64 / 2.0)
                    .collect();
                let density = smooth(&periodogram, weights.as_deref(), n);
                Univariate {
                    a,
                    b,
                    periodogram,
                    density,
                }
            })
            .collect();
        let freq: Vec<f64> = (0..=q).map(|k| k as f64 / n as f64).collect();
        let period: Vec<Option<f64>> = (0..=q)
            .map(|k| (k > 0).then(|| n as f64 / k as f64))
            .collect();

        // the bivariate statistics of the first series with each later one
        struct Pair {
            other: usize,
            columns: Vec<(SpectraItem, Vec<Option<f64>>)>,
        }
        let mut pairs = Vec::new();
        if cross {
            let x = &univariate[0];
            for (other, y) in univariate.iter().enumerate().skip(1) {
                let half = n as f64 / 2.0;
                let rc: Vec<f64> = (0..=q)
                    .map(|k| half * (x.a[k] * y.a[k] + x.b[k] * y.b[k]))
                    .collect();
                let ic: Vec<f64> = (0..=q)
                    .map(|k| half * (x.a[k] * y.b[k] - x.b[k] * y.a[k]))
                    .collect();
                let c = smooth(&rc, weights.as_deref(), n);
                let qs = smooth(&ic, weights.as_deref(), n);
                let amplitude: Vec<f64> = c.iter().zip(&qs).map(|(c, q)| c.hypot(*q)).collect();
                let ratio = |num: f64, den: f64| (den > 0.0).then(|| num / den);
                let coherency = (0..=q)
                    .map(|k| ratio(amplitude[k].powi(2), x.density[k] * y.density[k]))
                    .collect();
                let gain = (0..=q).map(|k| ratio(amplitude[k], x.density[k])).collect();
                let phase = (0..=q)
                    .map(|k| (c[k] != 0.0 || qs[k] != 0.0).then(|| qs[k].atan2(c[k])))
                    .collect();
                let some = |v: &[f64]| v.iter().map(|v| Some(*v)).collect::<Vec<_>>();
                pairs.push(Pair {
                    other,
                    columns: vec![
                        (SpectraItem::Rc, some(&rc)),
                        (SpectraItem::Ic, some(&ic)),
                        (SpectraItem::Cs, some(&c)),
                        (SpectraItem::Qs, some(&qs)),
                        (SpectraItem::A, some(&amplitude)),
                        (SpectraItem::K, coherency),
                        (SpectraItem::G, gain),
                        (SpectraItem::Ph, phase),
                    ],
                });
            }
        }
        let uni_columns = |u: &Univariate| -> Vec<(SpectraItem, Vec<Option<f64>>)> {
            let some = |v: &[f64]| v.iter().map(|v| Some(*v)).collect::<Vec<_>>();
            vec![
                (SpectraItem::Sin, some(&u.b)),
                (SpectraItem::Cos, some(&u.a)),
                (SpectraItem::P, some(&u.periodogram)),
                (SpectraItem::S, some(&u.density)),
            ]
        };

        // tables
        let label = |name: &str| shown.named(data, name);
        let window_text = match &smoothing {
            SpectraSmoothing::Weights(_) => "Own weights".to_string(),
            SpectraSmoothing::Named(kind, _) => match kind {
                SpectraWindow::Hamming => "Tukey-Hamming",
                SpectraWindow::Tukey => "Tukey-Hanning",
                SpectraWindow::Parzen => "Parzen",
                SpectraWindow::Bartlett => "Bartlett",
                SpectraWindow::Daniell => "Daniell (equal weights)",
                SpectraWindow::None => "None",
            }
            .to_string(),
        };
        let mut rows = vec![
            vec![
                text("Series"),
                text(
                    names
                        .iter()
                        .map(|n| label(n))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ],
            vec![text("Centered"), text(if center { "Yes" } else { "No" })],
            vec![
                text("Cross-spectral analysis"),
                text(if cross { "Yes" } else { "No" }),
            ],
            vec![text("Spectral window"), text(window_text)],
            vec![
                text("Span"),
                number(weights.as_ref().map(|w| w.len() as f64).or(Some(1.0))),
            ],
            vec![text("Cases used"), Cell::Number(n as f64)],
        ];
        if let Some(w) = &weights {
            let p = (w.len() / 2) as i64;
            for (i, v) in w.iter().enumerate() {
                rows.push(vec![
                    text(format!("Weight W({})", i as i64 - p)),
                    number(Some(*v)),
                ]);
            }
        }
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            rows,
            (skip == 1)
                .then(|| "The number of cases is odd, so the first case is left out.".to_string()),
        )];
        let freq_rows = |columns: &[(SpectraItem, Vec<Option<f64>>)]| -> Vec<Vec<Cell>> {
            (0..=q)
                .map(|k| {
                    let mut row = vec![number(Some(freq[k])), number(period[k])];
                    row.extend(columns.iter().map(|(_, v)| number(v[k])));
                    row
                })
                .collect()
        };
        let headers = |columns: &[(SpectraItem, Vec<Option<f64>>)]| {
            let mut h = vec!["Frequency", "Period"];
            h.extend(columns.iter().map(|(i, _)| item_name(*i)));
            plain(&h)
        };
        for (name, u) in names.iter().zip(&univariate) {
            let columns = uni_columns(u);
            tables.push(table(
                "Univariate Statistics",
                0,
                headers(&columns),
                freq_rows(&columns),
                Some(format!("Series: {}.", label(name))),
            ));
        }
        for pair in &pairs {
            tables.push(table(
                "Bivariate Statistics",
                0,
                headers(&pair.columns),
                freq_rows(&pair.columns),
                Some(format!(
                    "{} with {}. Gain is of the second series over the first; phase in radians.",
                    label(&names[0]),
                    label(&names[pair.other])
                )),
            ));
        }

        // charts
        let plot = spec.plot.clone().unwrap_or(SpectraPlot {
            items: vec![SpectraItem::P],
            all: false,
            by: None,
        });
        let wanted = |item: SpectraItem| {
            plot.items.contains(&item) || (plot.all && (cross || !item.bivariate()))
        };
        let by_freq = plot.by != Some(SpectraBy::Period);
        let by_period = plot.by != Some(SpectraBy::Frequency);
        let mut charts = Vec::new();
        let mut chart = |title: String, item: SpectraItem, values: &[Option<f64>]| {
            let logged = matches!(item, SpectraItem::P | SpectraItem::S);
            let shown_value = |v: Option<f64>| match v {
                Some(v) if logged && v > 0.0 => v.ln(),
                Some(_) if logged => f64::NAN,
                Some(v) => v,
                None => f64::NAN,
            };
            let y_title = if logged {
                format!("{} (natural log)", item_name(item))
            } else {
                item_name(item).to_string()
            };
            let category = |label: String, value: f64| Category {
                label,
                value: Cell::Number(value),
                missing: false,
                bin: None,
            };
            let series = |v: Vec<f64>| Series {
                name: item_name(item).into(),
                statistic: Statistic::Value,
                variable: None,
                value: Cell::Empty,
                values: v,
            };
            if by_freq {
                charts.push(Chart::lines(
                    format!("{title} by Frequency"),
                    Axis::titled("Frequency"),
                    Axis::titled(y_title.clone()),
                    freq.iter()
                        .map(|f| category(format!("{f:.4}"), *f))
                        .collect(),
                    vec![series(values.iter().map(|v| shown_value(*v)).collect())],
                ));
            }
            if by_period {
                // longest period first, frequency 0 left out
                let ks: Vec<usize> = (1..=q).collect();
                charts.push(Chart::lines(
                    format!("{title} by Period"),
                    Axis::titled("Period (natural log scale)"),
                    Axis::titled(y_title),
                    ks.iter()
                        .map(|&k| {
                            let p = n as f64 / k as f64;
                            category(format!("{p:.2}"), p)
                        })
                        .collect(),
                    vec![series(ks.iter().map(|&k| shown_value(values[k])).collect())],
                ));
            }
        };
        for (name, u) in names.iter().zip(&univariate) {
            for (item, values) in uni_columns(u) {
                if wanted(item) {
                    chart(
                        format!("{}: {}", item_name(item), label(name)),
                        item,
                        &values,
                    );
                }
            }
        }
        for pair in &pairs {
            for (item, values) in &pair.columns {
                if wanted(*item) {
                    chart(
                        format!(
                            "{}: {} with {}",
                            item_name(*item),
                            label(&names[0]),
                            label(&names[pair.other])
                        ),
                        *item,
                        values,
                    );
                }
            }
        }
        if !cross && plot.items.iter().any(|i| i.bivariate()) {
            warnings.push(
                "SPECTRA: the plots of pairs of series (CS, QS, PH, A, G, K) need CROSS; they are \
                 left out"
                    .into(),
            );
        }
        self.charts.extend(charts);

        // saved variables
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        for (item, root) in &spec.save {
            let full = |v: &[Option<f64>]| {
                let mut column = vec![None; cases];
                column[skip..skip + v.len()].copy_from_slice(v);
                column
            };
            match item {
                SpectraItem::Freq | SpectraItem::Per => {
                    let values: Vec<Option<f64>> = match item {
                        SpectraItem::Freq => freq.iter().map(|f| Some(*f)).collect(),
                        _ => period.clone(),
                    };
                    saves.push((root.clone(), item_name(*item).into(), full(&values)));
                }
                i if i.bivariate() => {
                    if !cross {
                        warnings.push(format!(
                            "SPECTRA: SAVE {} needs CROSS; it is left out",
                            item_name(*i)
                        ));
                        continue;
                    }
                    for pair in &pairs {
                        let (_, values) = pair
                            .columns
                            .iter()
                            .find(|(c, _)| c == i)
                            .expect("every pair statistic");
                        saves.push((
                            format!("{root}_1_{}", pair.other + 1),
                            format!(
                                "{} of {} with {}",
                                item_name(*i),
                                names[0],
                                names[pair.other]
                            ),
                            full(values),
                        ));
                    }
                }
                i => {
                    for (s, (name, u)) in names.iter().zip(&univariate).enumerate() {
                        let (_, values) = uni_columns(u)
                            .into_iter()
                            .find(|(c, _)| c == i)
                            .expect("every series statistic");
                        saves.push((
                            format!("{root}_{}", s + 1),
                            format!("{} of {name}", item_name(*i)),
                            full(&values),
                        ));
                    }
                }
            }
        }
        let data = self.active()?;
        saves.retain(|(name, _, _)| {
            let exists = data.variable_index(name).is_some();
            let long = name.len() > 64;
            if exists || long {
                warnings.push(format!(
                    "SPECTRA: {name} {}, so it is not saved",
                    if exists {
                        "exists already"
                    } else {
                        "is longer than 64 bytes"
                    }
                ));
            }
            !exists && !long
        });
        let most = self.tset.mxnewvars as usize;
        if saves.len() > most {
            warnings.push(format!(
                "SPECTRA would make {} new variables, more than TSET MXNEWVARS={most}; none are saved",
                saves.len()
            ));
            saves.clear();
        }
        for w in warnings {
            self.warn(w);
        }
        if !saves.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saves {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F11.5")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
                Ok(())
            })?;
        }
        self.ts_models.spectra = Some(Box::new(Spectra {
            save: Vec::new(),
            ..spec.clone()
        }));
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(kind: SpectraWindow, span: i64) -> Vec<f64> {
        window_weights(&SpectraSmoothing::Named(kind, Some(span)))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn windows_sum_to_one_and_match_their_kernels() {
        // Bartlett of span 5: the Fejer kernel of order 2 at 2 pi k / 5, 1 + cos t, over 5
        let w = named(SpectraWindow::Bartlett, 5);
        let c = (2.0 * PI / 5.0).cos();
        let c2 = (4.0 * PI / 5.0).cos();
        let expected = [
            (1.0 + c2) / 5.0,
            (1.0 + c) / 5.0,
            0.4,
            (1.0 + c) / 5.0,
            (1.0 + c2) / 5.0,
        ];
        for (a, b) in w.iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{w:?}");
        }
        // an even span loses one; equal weights
        assert_eq!(named(SpectraWindow::Daniell, 4), vec![1.0 / 3.0; 3]);
        for kind in [
            SpectraWindow::Hamming,
            SpectraWindow::Tukey,
            SpectraWindow::Parzen,
            SpectraWindow::Bartlett,
        ] {
            for span in [3, 5, 7, 9, 11] {
                let w = named(kind, span);
                assert_eq!(w.len(), span as usize);
                assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                assert!((w[0] - w[w.len() - 1]).abs() < 1e-12);
            }
        }
        // the Tukey-Hamming weights of span 5 by hand: 2.24, 1.20438, 0.17562 over 5
        let w = named(SpectraWindow::Hamming, 5);
        assert!((w[2] - 0.448).abs() < 1e-12, "{w:?}");
        assert!((w[1] - 0.240_859).abs() < 1e-5, "{w:?}");
        assert!(
            window_weights(&SpectraSmoothing::Named(SpectraWindow::Daniell, Some(1)))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn own_weights_of_even_count_lack_the_last() {
        let w = window_weights(&SpectraSmoothing::Weights(vec![1.0, 2.0, 4.0, 1.0])).unwrap();
        assert_eq!(w, Some(vec![0.125, 0.25, 0.5, 0.125, 0.0]));
        assert!(window_weights(&SpectraSmoothing::Weights(vec![1.0, 0.0, 1.0])).is_err());
    }

    #[test]
    fn smoothing_mirrors_at_both_ends() {
        let values = [100.0, 1.0, 2.0, 3.0, 4.0];
        // n = 8, q = 4; span 3 with weights 1/4, 1/2, 1/4
        let w = [0.25, 0.5, 0.25];
        let s = smooth(&values, Some(&w), 8);
        // K = 0: l_-1 = l_1, l_0 read as l_1, l_1
        assert!((s[0] - 1.0).abs() < 1e-12);
        // K = 1: l_0 (read as l_1), l_1, l_2
        assert!((s[1] - (0.25 + 0.5 + 0.5)).abs() < 1e-12);
        // K = 4: l_3, l_4, l_5 = l_(n + 1 - 5) = l_4
        assert!((s[4] - (0.75 + 2.0 + 1.0)).abs() < 1e-12);
    }

    #[test]
    fn periodogram_of_a_pure_cosine() {
        // 8 cases of cos(2 pi 2 t / 8): all power at K = 2, a_2 = 1, periodogram n / 2 = 4
        let x: Vec<f64> = (0..8)
            .map(|t| (2.0 * PI * 2.0 * t as f64 / 8.0).cos())
            .collect();
        let (a, b) = fourier_terms(&x);
        assert!((a[2] - 1.0).abs() < 1e-12 && b[2].abs() < 1e-12);
        assert!(a[1].abs() < 1e-12 && a[0].abs() < 1e-12);
    }
}
