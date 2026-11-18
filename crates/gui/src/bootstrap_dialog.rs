//! The Bootstrap section shared by the dialogs of the commands BOOTSTRAP can resample: one
//! struct per dialog, its syntax written before the dialog's own command. Options left as they
//! are add nothing, so the command runs with its own defaults.

use crate::app::{field, target_list};
use eframe::egui::Ui;

/// BOOTSTRAP settings of a dialog.
#[derive(Clone, PartialEq, Debug)]
pub struct Bootstrap {
    pub on: bool,
    /// Number of resamples.
    pub samples: String,
    /// Confidence level in percent.
    pub level: String,
    /// Bias-corrected and accelerated intervals instead of percentile ones.
    pub bca: bool,
    pub stratified: bool,
    pub strata: Vec<String>,
}

impl Default for Bootstrap {
    fn default() -> Self {
        Self {
            on: false,
            samples: "1000".into(),
            level: "95".into(),
            bca: false,
            stratified: false,
            strata: Vec::new(),
        }
    }
}

impl Bootstrap {
    /// The BOOTSTRAP command and a line break, or nothing when the section is off.
    pub fn syntax(&self) -> Result<String, String> {
        if !self.on {
            return Ok(String::new());
        }
        let samples: u64 = self
            .samples
            .trim()
            .parse()
            .ok()
            .filter(|n| *n >= 1)
            .ok_or("the number of samples must be a whole number of at least 1")?;
        let level: f64 = self
            .level
            .trim()
            .parse()
            .ok()
            .filter(|l| *l > 0.0 && *l < 100.0)
            .ok_or("the confidence level must be a number above 0 and below 100")?;
        let mut text = String::from("BOOTSTRAP");
        if self.stratified {
            if self.strata.is_empty() {
                return Err("add at least one variable to Strata".into());
            }
            text.push_str(&format!(
                "\n  /SAMPLING METHOD=STRATIFIED(STRATA={})",
                self.strata.join(" ")
            ));
        }
        let mut criteria = Vec::new();
        if level != 95.0 {
            criteria.push(format!("CILEVEL={level}"));
        }
        if self.bca {
            criteria.push("CITYPE=BCA".to_string());
        }
        if samples != 1000 {
            criteria.push(format!("NSAMPLES={samples}"));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        text.push_str(".\n");
        Ok(text)
    }

    /// `command` with the BOOTSTRAP command in front when the section is on.
    pub fn before(&self, command: String) -> Result<String, String> {
        Ok(self.syntax()? + &command)
    }

    pub fn view(&mut self, ui: &mut Ui, picked: &mut Vec<String>) {
        ui.add_space(6.0);
        ui.checkbox(&mut self.on, "Perform bootstrapping");
        if !self.on {
            return;
        }
        field(ui, "Number of samples", &mut self.samples, 60.0);
        field(ui, "Confidence level %", &mut self.level, 50.0);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.bca, false, "Percentile interval");
            ui.radio_value(&mut self.bca, true, "BCa interval");
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.stratified, false, "Simple sampling");
            ui.radio_value(&mut self.stratified, true, "Stratified sampling");
        });
        if self.stratified {
            target_list(ui, "Strata", &mut self.strata, picked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_adds_nothing() {
        assert_eq!(Bootstrap::default().before("X.".into()).unwrap(), "X.");
    }

    #[test]
    fn defaults_alone_write_the_bare_command() {
        let b = Bootstrap {
            on: true,
            ..Default::default()
        };
        assert_eq!(b.before("X.".into()).unwrap(), "BOOTSTRAP.\nX.");
    }

    #[test]
    fn only_what_differs() {
        let b = Bootstrap {
            on: true,
            samples: "500".into(),
            level: "90".into(),
            bca: true,
            stratified: true,
            strata: vec!["g".into(), "h".into()],
        };
        assert_eq!(
            b.syntax().unwrap(),
            "BOOTSTRAP\n  /SAMPLING METHOD=STRATIFIED(STRATA=g h)\n  /CRITERIA CILEVEL=90 CITYPE=BCA NSAMPLES=500.\n"
        );
    }

    #[test]
    fn bad_input_is_named() {
        let on = Bootstrap {
            on: true,
            ..Default::default()
        };
        let samples = Bootstrap {
            samples: "0".into(),
            ..on.clone()
        };
        assert!(samples.syntax().unwrap_err().contains("samples"));
        let level = Bootstrap {
            level: "100".into(),
            ..on.clone()
        };
        assert!(level.syntax().unwrap_err().contains("level"));
        let strata = Bootstrap {
            stratified: true,
            ..on
        };
        assert!(strata.syntax().unwrap_err().contains("Strata"));
    }
}
