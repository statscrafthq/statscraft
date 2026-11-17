//! The Spectral Analysis (SPECTRA), Autoregression (AREG) and 2-Stage Least Squares (2SLS)
//! dialogs. Options left as they are add nothing to the syntax, so the command runs with its
//! own defaults and the session's TSET settings.

use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::number;
use statscraft_data::{Dataset, is_valid_name};

/// The windows of SPECTRA /WINDOW, HAMMING (the default) first, NONE last.
pub const SPECTRA_WINDOWS: [(&str, &str); 7] = [
    ("HAMMING", "Hamming"),
    ("TUKEY", "Tukey"),
    ("PARZEN", "Parzen"),
    ("BARTLETT", "Bartlett"),
    ("UNIT", "Unit (equal weights)"),
    ("DANIELL", "Daniell"),
    ("NONE", "None (the raw periodogram)"),
];
pub const SPECTRA_NO_WINDOW: usize = 6;

/// The charts of SPECTRA /PLOT; from `SPECTRA_CROSS_PLOTS` on they need the cross-spectrum.
pub const SPECTRA_PLOTS: [(&str, &str); 8] = [
    ("P", "Periodogram"),
    ("S", "Spectral density"),
    ("CS", "Cospectral density"),
    ("QS", "Quadrature spectrum"),
    ("PH", "Phase spectrum"),
    ("A", "Cross amplitude"),
    ("G", "Gain"),
    ("K", "Squared coherency"),
];
pub const SPECTRA_CROSS_PLOTS: usize = 2;

pub const SPECTRA_AXES: [(&str, &str); 3] = [
    ("", "Both by frequency and by period"),
    ("FREQ", "By frequency"),
    ("PERIOD", "By period"),
];

/// The variables of SPECTRA /SAVE: keyword, label and the root a new dialog starts with; from
/// `SPECTRA_CROSS_SAVES` on they need the cross-spectrum.
pub const SPECTRA_SAVES: [(&str, &str, &str); 14] = [
    ("FREQ", "Frequency", "freq"),
    ("PER", "Period", "per"),
    ("SIN", "Sine coefficient", "sin"),
    ("COS", "Cosine coefficient", "cos"),
    ("P", "Periodogram", "p"),
    ("S", "Spectral density", "s"),
    ("RC", "Real part of the cross-periodogram", "rc"),
    ("IC", "Imaginary part of the cross-periodogram", "ic"),
    ("CS", "Cospectral density", "cs"),
    ("QS", "Quadrature spectrum", "qs"),
    ("PH", "Phase spectrum", "ph"),
    ("A", "Cross amplitude", "a"),
    ("G", "Gain", "g"),
    ("K", "Squared coherency", "k"),
];
pub const SPECTRA_CROSS_SAVES: usize = 6;

/// SPECTRA: the periodogram and smoothed spectral density of series, and with `cross` the
/// cross-spectrum of the first series with each later one.
#[derive(Clone, PartialEq, Debug)]
pub struct Spectra {
    pub variables: Vec<String>,
    pub center: bool,
    pub cross: bool,
    /// Index into `SPECTRA_WINDOWS`.
    pub window: usize,
    /// Empty: the window's own span (5 for HAMMING). Not read with NONE.
    pub span: String,
    /// PLOT=NONE.
    pub no_plot: bool,
    /// Flags of `SPECTRA_PLOTS`; none set: the command's own chart.
    pub plots: [bool; 8],
    /// Index into `SPECTRA_AXES`.
    pub axis: usize,
    /// A flag and a root of each of `SPECTRA_SAVES`.
    pub saves: [(bool, String); 14],
}

impl Default for Spectra {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            center: true,
            cross: false,
            window: 0,
            span: String::new(),
            no_plot: false,
            plots: [false; 8],
            axis: 0,
            saves: SPECTRA_SAVES.map(|(_, _, root)| (false, root.to_string())),
        }
    }
}

impl Spectra {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.variables.is_empty() {
            return Err("add at least one series".into());
        }
        numeric(data, &self.variables)?;
        let mut text = format!("SPECTRA VARIABLES={}", self.variables.join(" "));
        text.push_str(if self.center {
            "\n  /CENTER"
        } else {
            "\n  /CENTER NO"
        });
        let cross = self.cross && self.variables.len() > 1;
        if cross {
            text.push_str("\n  /CROSS");
        }
        let keyword = SPECTRA_WINDOWS[self.window].0;
        let span = whole(&self.span, 1, "the span")?;
        match (self.window, span) {
            (0, None) => {}
            (SPECTRA_NO_WINDOW, _) => text.push_str("\n  /WINDOW=NONE"),
            (_, None) => text.push_str(&format!("\n  /WINDOW={keyword}")),
            (_, Some(n)) => text.push_str(&format!("\n  /WINDOW={keyword}({n})")),
        }
        let plots: Vec<&str> = SPECTRA_PLOTS
            .iter()
            .zip(self.plots)
            .enumerate()
            .filter(|(i, (_, on))| *on && (*i < SPECTRA_CROSS_PLOTS || cross))
            .map(|(_, ((keyword, _), _))| *keyword)
            .collect();
        if self.no_plot {
            text.push_str("\n  /PLOT=NONE");
        } else if !plots.is_empty() {
            text.push_str(&format!("\n  /PLOT={}", plots.join(" ")));
            let axis = SPECTRA_AXES[self.axis].0;
            if !axis.is_empty() {
                text.push_str(&format!(" BY {axis}"));
            }
        }
        let mut saves = Vec::new();
        for (i, ((keyword, label, _), (on, root))) in
            SPECTRA_SAVES.iter().zip(&self.saves).enumerate()
        {
            if !on || (i >= SPECTRA_CROSS_SAVES && !cross) {
                continue;
            }
            let root = root.trim();
            if !is_valid_name(root) {
                return Err(format!("the root of the {label} is not a valid name"));
            }
            saves.push(format!("{keyword}({root})"));
        }
        if !saves.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", saves.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// The estimators of AREG /METHOD, Prais-Winsten (the default) first.
pub const AREG_METHODS: [(&str, &str); 3] = [
    ("PW", "Prais-Winsten"),
    ("CO", "Cochrane-Orcutt"),
    ("ML", "Exact maximum likelihood"),
];

/// CONSTANT or NOCONSTANT, the TSET setting first.
pub const CONSTANT_CHOICES: [(&str, &str); 3] = [
    ("", "As the TSET setting says"),
    ("CONSTANT", "In the model"),
    ("NOCONSTANT", "Not in the model"),
];

/// AREG: a regression whose errors follow a first-order autoregressive process.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Areg {
    pub dependent: String,
    pub independents: Vec<String>,
    /// Index into `AREG_METHODS`.
    pub method: usize,
    /// Index into `CONSTANT_CHOICES`.
    pub constant: usize,
    /// Empty: start from 0.
    pub rho: String,
    /// Empty: 10.
    pub mxiter: String,
}

impl Areg {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.dependent.is_empty() {
            return Err("set the dependent series".into());
        }
        if self.independents.is_empty() {
            return Err("add at least one independent variable".into());
        }
        if self.independents.contains(&self.dependent) {
            return Err(format!(
                "{} is both the dependent and an independent variable",
                self.dependent
            ));
        }
        numeric(
            data,
            std::iter::once(&self.dependent).chain(&self.independents),
        )?;
        let mut text = format!(
            "AREG VARIABLES={} WITH {}",
            self.dependent,
            self.independents.join(" ")
        );
        if self.method > 0 {
            text.push_str(&format!("\n  /METHOD={}", AREG_METHODS[self.method].0));
        }
        if self.constant > 0 {
            text.push_str(&format!("\n  /{}", CONSTANT_CHOICES[self.constant].0));
        }
        if !self.rho.trim().is_empty() {
            let rho = parse(&self.rho, "rho")?;
            if rho <= -1.0 || rho >= 1.0 {
                return Err("rho must be above -1 and below 1".into());
            }
            text.push_str(&format!("\n  /RHO={}", number(rho)));
        }
        if let Some(n) = whole(&self.mxiter, 1, "the most iterations")? {
            text.push_str(&format!("\n  /MXITER={n}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// 2SLS: two-stage least squares of one equation.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TwoSls {
    pub dependent: String,
    pub explanatory: Vec<String>,
    pub instruments: Vec<String>,
    /// Index into `CONSTANT_CHOICES`.
    pub constant: usize,
    /// PRINT=COV: the covariances of the estimates.
    pub covariances: bool,
    pub save_predicted: bool,
    pub save_residuals: bool,
}

impl TwoSls {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.dependent.is_empty() {
            return Err("set the dependent variable".into());
        }
        if self.explanatory.is_empty() {
            return Err("add at least one explanatory variable".into());
        }
        if self.explanatory.contains(&self.dependent) {
            return Err(format!(
                "{} is both the dependent and an explanatory variable",
                self.dependent
            ));
        }
        if self.instruments.len() < self.explanatory.len() {
            return Err(format!(
                "add at least as many instrumental variables as explanatory ones ({})",
                self.explanatory.len()
            ));
        }
        numeric(
            data,
            std::iter::once(&self.dependent)
                .chain(&self.explanatory)
                .chain(&self.instruments),
        )?;
        let mut text = format!(
            "2SLS {} WITH {}\n  /INSTRUMENTS={}",
            self.dependent,
            self.explanatory.join(" "),
            self.instruments.join(" ")
        );
        if self.constant > 0 {
            text.push_str(&format!("\n  /{}", CONSTANT_CHOICES[self.constant].0));
        }
        if self.covariances {
            text.push_str("\n  /PRINT=COV");
        }
        let saves: Vec<&str> = [
            (self.save_predicted, "PRED"),
            (self.save_residuals, "RESID"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !saves.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", saves.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn spectra_writes_its_options_and_drops_the_cross_ones_for_one_series() {
        let data = Dataset::new(0);
        let mut d = Spectra {
            variables: names(&["flow"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "SPECTRA VARIABLES=flow\n  /CENTER."
        );
        d.cross = true;
        d.plots[7] = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "SPECTRA VARIABLES=flow\n  /CENTER."
        );
        d.variables.push("rain".into());
        d.center = false;
        d.window = 2;
        d.span = "7".into();
        d.plots[1] = true;
        d.axis = 1;
        d.saves[4] = (true, "pg".into());
        d.saves[13] = (true, "co".into());
        assert_eq!(
            d.syntax(&data).unwrap(),
            "SPECTRA VARIABLES=flow rain\n  /CENTER NO\n  /CROSS\n  /WINDOW=PARZEN(7)\n  \
             /PLOT=S K BY FREQ\n  /SAVE=P(pg) K(co)."
        );
        d.saves[0] = (true, "1x".into());
        assert!(d.syntax(&data).is_err());
    }

    #[test]
    fn areg_writes_its_options() {
        let data = Dataset::new(0);
        let mut d = Areg {
            dependent: "cost".into(),
            independents: names(&["degree_days"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "AREG VARIABLES=cost WITH degree_days."
        );
        d.method = 1;
        d.constant = 2;
        d.rho = "0.3".into();
        d.mxiter = "20".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "AREG VARIABLES=cost WITH degree_days\n  /METHOD=CO\n  /NOCONSTANT\n  /RHO=0.3\n  \
             /MXITER=20."
        );
        d.rho = "1".into();
        assert!(d.syntax(&data).is_err());
    }

    #[test]
    fn two_sls_needs_enough_instruments_and_writes_its_options() {
        let data = Dataset::new(0);
        let mut d = TwoSls {
            dependent: "yield".into(),
            explanatory: names(&["fertilizer", "rain"]),
            instruments: names(&["subsidy"]),
            ..Default::default()
        };
        assert!(d.syntax(&data).is_err());
        d.instruments.push("soil".into());
        assert_eq!(
            d.syntax(&data).unwrap(),
            "2SLS yield WITH fertilizer rain\n  /INSTRUMENTS=subsidy soil."
        );
        d.constant = 1;
        d.covariances = true;
        d.save_predicted = true;
        d.save_residuals = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "2SLS yield WITH fertilizer rain\n  /INSTRUMENTS=subsidy soil\n  /CONSTANT\n  \
             /PRINT=COV\n  /SAVE=PRED RESID."
        );
    }
}
