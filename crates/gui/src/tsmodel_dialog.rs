//! The Create Models (TSMODEL) and Apply Models (TSAPPLY) dialogs. Options left as they are
//! add nothing to the syntax, so the command runs with its own defaults. The forecast period
//! is the PREDICT command written before the model command.

use crate::dialogs::whole;
use crate::stats_dialogs::numeric;
use crate::syntax_text::quote;
use statscraft_data::{Dataset, is_valid_name};

pub const TS_METHODS: [(&str, &str); 3] = [
    ("EXPERT", "Expert modeler (picks the best model)"),
    ("EXSMOOTH", "Exponential smoothing"),
    ("ARIMA", "ARIMA"),
];

pub const TS_SMOOTHING: [(&str, &str); 7] = [
    ("SIMPLE", "Simple"),
    ("SIMPLESEASONAL", "Simple seasonal"),
    ("HOLT", "Holt's linear trend"),
    ("BROWN", "Brown's linear trend"),
    ("DAMPEDTREND", "Damped trend"),
    ("WINTERSADDITIVE", "Winters' additive"),
    ("WINTERSMULTIPLICATIVE", "Winters' multiplicative"),
];

pub const TS_TRANSFORMS: [(&str, &str); 3] = [
    ("NONE", "None"),
    ("SQRT", "Square root"),
    ("LN", "Natural log"),
];

/// The fit measures of MODELSTATISTICS MODELFIT.
pub const TS_FIT: [(&str, &str); 8] = [
    ("SRSQUARE", "Stationary R-squared"),
    ("RSQUARE", "R-squared"),
    ("RMSE", "Root mean square error"),
    ("MAPE", "Mean absolute percentage error"),
    ("MAE", "Mean absolute error"),
    ("MAXAPE", "Maximum absolute percentage error"),
    ("MAXAE", "Maximum absolute error"),
    ("NORMBIC", "Normalized BIC"),
];

/// MODELDETAILS PRINT keywords; the last two need estimation, so TSAPPLY without
/// re-estimating cannot show them.
pub const TS_DETAILS: [(&str, &str); 4] = [
    ("PARAMETERS", "Parameter estimates"),
    ("FORECASTS", "Forecasts"),
    ("RESIDACF", "Residual autocorrelations"),
    ("RESIDPACF", "Residual partial autocorrelations"),
];

/// SERIESPLOT keywords; the fitted values and their limits need estimation too.
pub const TS_SERIES_PLOTS: [(&str, &str); 5] = [
    ("OBSERVED", "Observed values"),
    ("FORECAST", "Forecasts"),
    ("FIT", "Fitted values"),
    ("FORECASTCI", "Forecast limits"),
    ("FITCI", "Fit limits"),
];

/// SAVE keywords; the noise residuals need estimation too.
pub const TS_SAVES: [(&str, &str); 4] = [
    ("PREDICTED", "Predicted values"),
    ("LCL", "Lower confidence limits"),
    ("UCL", "Upper confidence limits"),
    ("NRESIDUAL", "Noise residuals"),
];

/// What both dialogs show and save.
#[derive(Clone, PartialEq, Debug)]
pub struct TsOutputs {
    pub fit: [bool; 8],
    pub details: [bool; 4],
    pub plots: [bool; 5],
    /// A flag and a root of each of `TS_SAVES`; an empty root is the command's own.
    pub saves: [(bool, String); 4],
    /// Empty: 95.
    pub level: String,
    /// Empty: the period of the date variables.
    pub season: String,
    /// The last case to forecast; empty: the forecast period already set.
    pub last: String,
}

impl Default for TsOutputs {
    fn default() -> Self {
        let mut fit = [false; 8];
        fit[0] = true;
        Self {
            fit,
            details: [false; 4],
            plots: [false; 5],
            saves: std::array::from_fn(|_| (false, String::new())),
            level: String::new(),
            season: String::new(),
            last: String::new(),
        }
    }
}

fn chosen<const N: usize>(
    table: &[(&'static str, &'static str); N],
    flags: &[bool; N],
) -> Vec<&'static str> {
    table
        .iter()
        .zip(flags)
        .filter(|(_, on)| **on)
        .map(|((k, _), _)| *k)
        .collect()
}

impl TsOutputs {
    /// The PREDICT command (or nothing) and the global subcommands.
    fn parts(&self, reestimate: bool) -> Result<(String, Vec<String>), String> {
        let predict = match whole(&self.last, 1, "the last case to forecast")? {
            Some(n) => format!("PREDICT THRU {n}.\n"),
            None => String::new(),
        };
        let mut lines = Vec::new();
        let fit = chosen(&TS_FIT, &self.fit);
        if fit.is_empty() {
            lines.push("/MODELSTATISTICS DISPLAY=NO".into());
        } else if fit != ["SRSQUARE"] {
            lines.push(format!("/MODELSTATISTICS MODELFIT=[{}]", fit.join(" ")));
        }
        let details = chosen(&TS_DETAILS, &self.details);
        if !details.is_empty() {
            lines.push(format!("/MODELDETAILS PRINT=[{}]", details.join(" ")));
        }
        let plots = chosen(&TS_SERIES_PLOTS, &self.plots);
        if !plots.is_empty() {
            lines.push(format!("/SERIESPLOT {}", plots.join(" ")));
        }
        let mut saves = Vec::new();
        for ((keyword, label), (on, root)) in TS_SAVES.iter().zip(&self.saves) {
            if !on {
                continue;
            }
            let root = root.trim();
            if root.is_empty() {
                saves.push((*keyword).to_string());
            } else if is_valid_name(root) {
                saves.push(format!("{keyword}({root})"));
            } else {
                return Err(format!("the root of the {label} is not a valid name"));
            }
        }
        if !saves.is_empty() {
            lines.push(format!("/SAVE {}", saves.join(" ")));
        }
        let mut auxiliary = Vec::new();
        if reestimate {
            auxiliary.push("REESTIMATE=YES".to_string());
        }
        let level = self.level.trim();
        if !level.is_empty() {
            match level.parse::<f64>() {
                Ok(v) if v > 0.0 && v < 100.0 => auxiliary.push(format!("CILEVEL={level}")),
                _ => return Err("the confidence level must be above 0 and below 100".into()),
            }
        }
        if let Some(n) = whole(&self.season, 1, "the season length")? {
            auxiliary.push(format!("SEASONLENGTH={n}"));
        }
        if !auxiliary.is_empty() {
            lines.push(format!("/AUXILIARY {}", auxiliary.join(" ")));
        }
        Ok((predict, lines))
    }
}

/// A list of lags as TSMODEL reads it: empty is order 0, one value an order.
fn lags(text: &str, what: &str) -> Result<String, String> {
    let mut list = Vec::new();
    for word in text.split([' ', ',']).filter(|w| !w.is_empty()) {
        list.push(whole(word, 0, what)?.unwrap_or(0).to_string());
    }
    Ok(match list.len() {
        0 => "0".into(),
        1 => list.remove(0),
        _ => format!("[{}]", list.join(" ")),
    })
}

/// TSMODEL: one model for each dependent series.
#[derive(Clone, PartialEq, Debug)]
pub struct Tsmodel {
    pub dependents: Vec<String>,
    pub independents: Vec<String>,
    /// Index into `TS_METHODS`.
    pub method: usize,
    pub expert_arima: bool,
    pub expert_smoothing: bool,
    pub seasonal: bool,
    /// Index into `TS_SMOOTHING`.
    pub smoothing: usize,
    /// Index into `TS_TRANSFORMS`, for the smoothing and ARIMA methods.
    pub transform: usize,
    /// ARIMA: the order or the lags of each part; empty is none.
    pub ar: String,
    pub ma: String,
    pub ar_seasonal: String,
    pub ma_seasonal: String,
    pub diff: String,
    pub diff_seasonal: String,
    pub constant: bool,
    pub outputs: TsOutputs,
    /// The model file to write; empty: none.
    pub file: String,
    /// Empty: Model.
    pub prefix: String,
}

impl Default for Tsmodel {
    fn default() -> Self {
        Self {
            dependents: Vec::new(),
            independents: Vec::new(),
            method: 0,
            expert_arima: true,
            expert_smoothing: true,
            seasonal: true,
            smoothing: 2,
            transform: 0,
            ar: String::new(),
            ma: String::new(),
            ar_seasonal: String::new(),
            ma_seasonal: String::new(),
            diff: String::new(),
            diff_seasonal: String::new(),
            constant: true,
            outputs: TsOutputs::default(),
            file: String::new(),
            prefix: String::new(),
        }
    }
}

impl Tsmodel {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.dependents.is_empty() {
            return Err("add at least one dependent series".into());
        }
        numeric(data, self.dependents.iter().chain(&self.independents))?;
        if self.method == 1 && !self.independents.is_empty() {
            return Err(
                "exponential smoothing takes no predictors: remove them or use ARIMA".into(),
            );
        }
        let (predict, mut lines) = self.outputs.parts(false)?;
        let mut model = format!("/MODEL DEPENDENT={}", self.dependents.join(" "));
        if !self.independents.is_empty() {
            model.push_str(&format!(" INDEPENDENT={}", self.independents.join(" ")));
        }
        if !self.file.trim().is_empty() {
            model.push_str(&format!(" OUTFILE={}", quote(self.file.trim())));
        }
        if !self.prefix.trim().is_empty() {
            model.push_str(&format!(" PREFIX={}", quote(self.prefix.trim())));
        }
        lines.push(model);
        let transform = match TS_TRANSFORMS[self.transform].0 {
            "NONE" => String::new(),
            t => format!(" TRANSFORM={t}"),
        };
        match self.method {
            0 => {
                let types: Vec<&str> = [
                    (self.expert_arima, "ARIMA"),
                    (self.expert_smoothing, "EXSMOOTH"),
                ]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, k)| *k)
                .collect();
                if types.is_empty() {
                    return Err("the expert modeler needs ARIMA or exponential smoothing".into());
                }
                if types.len() < 2 || !self.seasonal {
                    let mut text = "/EXPERTMODELER".to_string();
                    if types.len() < 2 {
                        text.push_str(&format!(" TYPE={}", types[0]));
                    }
                    if !self.seasonal {
                        text.push_str(" TRYSEASONAL=NO");
                    }
                    lines.push(text);
                }
            }
            1 => lines.push(format!(
                "/EXSMOOTH TYPE={}{transform}",
                TS_SMOOTHING[self.smoothing].0
            )),
            _ => {
                let mut text = format!(
                    "/ARIMA AR={} MA={}",
                    lags(&self.ar, "an autoregressive lag")?,
                    lags(&self.ma, "a moving average lag")?
                );
                for (keyword, value, what) in [
                    (
                        "ARSEASONAL",
                        &self.ar_seasonal,
                        "a seasonal autoregressive lag",
                    ),
                    (
                        "MASEASONAL",
                        &self.ma_seasonal,
                        "a seasonal moving average lag",
                    ),
                ] {
                    if !value.trim().is_empty() {
                        text.push_str(&format!(" {keyword}={}", lags(value, what)?));
                    }
                }
                for (keyword, value, what) in [
                    ("DIFF", &self.diff, "the differencing"),
                    (
                        "DIFFSEASONAL",
                        &self.diff_seasonal,
                        "the seasonal differencing",
                    ),
                ] {
                    if let Some(n) = whole(value, 0, what)? {
                        text.push_str(&format!(" {keyword}={n}"));
                    }
                }
                if !self.constant {
                    text.push_str(" CONSTANT=NO");
                }
                text.push_str(&transform);
                lines.push(text);
            }
        }
        Ok(format!("{predict}TSMODEL {}.", lines.join("\n  ")))
    }
}

/// TSAPPLY: the models of a model file, as saved or estimated again.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Tsapply {
    pub file: String,
    pub reestimate: bool,
    /// Model identifiers (such as Model_1), separated by spaces; empty: every model.
    pub models: String,
    /// False: use only `models`; true: use all but `models`.
    pub drop: bool,
    pub outputs: TsOutputs,
    /// The model file to write; empty: none.
    pub out_file: String,
}

impl Tsapply {
    pub fn syntax(&self) -> Result<String, String> {
        if self.file.trim().is_empty() {
            return Err("choose the model file".into());
        }
        let mut outputs = self.outputs.clone();
        if !self.reestimate {
            // residuals, fitted values and their limits need the data estimated again
            outputs.details[2] = false;
            outputs.details[3] = false;
            outputs.plots[2] = false;
            outputs.plots[4] = false;
            outputs.saves[3].0 = false;
        }
        let (predict, mut lines) = outputs.parts(self.reestimate)?;
        let mut model = format!("/MODEL FILE={}", quote(self.file.trim()));
        let ids: Vec<String> = self
            .models
            .split([' ', ','])
            .filter(|w| !w.is_empty())
            .map(quote)
            .collect();
        if !ids.is_empty() {
            let word = if self.drop { "DROP" } else { "KEEP" };
            model.push_str(&format!(" {word}=[{}]", ids.join(" ")));
        }
        if !self.out_file.trim().is_empty() {
            model.push_str(&format!(" OUTFILE={}", quote(self.out_file.trim())));
        }
        lines.push(model);
        Ok(format!("{predict}TSAPPLY {}.", lines.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Dataset {
        let mut s = statscraft_engine::session::Session::default();
        s.run(
            "DATA LIST LIST /a b.\nBEGIN DATA\n1 2\n2 3\nEND DATA.",
            None,
        )
        .unwrap();
        s.dataset().unwrap().clone()
    }

    #[test]
    fn tsmodel_text() {
        let d = Tsmodel {
            dependents: vec!["a".into()],
            ..Default::default()
        };
        assert_eq!(d.syntax(&data()).unwrap(), "TSMODEL /MODEL DEPENDENT=a.");
        let d = Tsmodel {
            dependents: vec!["a".into()],
            independents: vec!["b".into()],
            method: 2,
            ar: "1".into(),
            ma: "1 3".into(),
            diff: "1".into(),
            constant: false,
            transform: 2,
            file: "it's.json".into(),
            outputs: TsOutputs {
                last: "60".into(),
                details: [true, true, false, false],
                saves: [
                    (true, "fc".into()),
                    (false, String::new()),
                    (false, String::new()),
                    (true, String::new()),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data()).unwrap(),
            "PREDICT THRU 60.\nTSMODEL /MODELDETAILS PRINT=[PARAMETERS FORECASTS]\n  \
             /SAVE PREDICTED(fc) NRESIDUAL\n  \
             /MODEL DEPENDENT=a INDEPENDENT=b OUTFILE='it''s.json'\n  \
             /ARIMA AR=1 MA=[1 3] DIFF=1 CONSTANT=NO TRANSFORM=LN."
        );
        assert!(Tsmodel::default().syntax(&data()).is_err());
    }

    #[test]
    fn tsapply_text() {
        let d = Tsapply {
            file: "m.json".into(),
            reestimate: true,
            models: "Model_1 Model_2".into(),
            drop: true,
            outputs: TsOutputs {
                details: [false, true, true, false],
                level: "90".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "TSAPPLY /MODELDETAILS PRINT=[FORECASTS RESIDACF]\n  \
             /AUXILIARY REESTIMATE=YES CILEVEL=90\n  \
             /MODEL FILE='m.json' DROP=['Model_1' 'Model_2']."
        );
        // residual autocorrelations need re-estimation
        let d = Tsapply {
            reestimate: false,
            ..d
        };
        assert!(!d.syntax().unwrap().contains("RESIDACF"));
        assert!(Tsapply::default().syntax().is_err());
    }
}
