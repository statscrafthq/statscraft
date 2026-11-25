//! The names MOD_1, MOD_2, ... of the models the time series and curve commands make, so that
//! `APPLY 'MOD_2'` can repeat any of them, not only the previous one of its kind.
//!
//! A model is the command's resolved specifications. After each such command ran, the previous
//! slots of all families are kept under the next name; `APPLY 'name'` puts the named model's
//! slot back as the previous one and then runs as `APPLY` without a name.
//!
//! shortcut: the whole set of slots is cloned per model, so a very long session of such commands
//! keeps one copy of the specifications for each; upgrade to per-family storage if that grows.

use crate::timeseries::Models;
use crate::{Engine, Error};
use statscraft_syntax::{Command, Pplot};

#[derive(Default, Clone)]
pub(crate) struct ModelNames {
    next: usize,
    saved: Vec<Saved>,
}

#[derive(Clone)]
struct Saved {
    name: String,
    family: &'static str,
    models: Models,
    pplot: Option<Box<Pplot>>,
}

/// The kind of model a command makes, and the name its APPLY gives.
fn family(command: &Command) -> Option<(&'static str, Option<&str>)> {
    fn named(apply: &Option<Option<String>>) -> Option<&str> {
        apply.as_ref().and_then(|name| name.as_deref())
    }
    Some(match command {
        Command::Correlogram(c) => (c.kind.name(), named(&c.apply)),
        Command::Season(c) => ("SEASON", named(&c.apply)),
        Command::Exsmooth(c) => ("EXSMOOTH", named(&c.apply)),
        Command::Arima(c) => (
            "ARIMA",
            c.apply.as_ref().and_then(|(name, _)| name.as_deref()),
        ),
        Command::Areg(c) => (
            "AREG",
            c.apply.as_ref().and_then(|(name, _)| name.as_deref()),
        ),
        Command::Spectra(c) => ("SPECTRA", named(&c.apply)),
        Command::TwoStage(c) => ("2SLS", named(&c.apply)),
        Command::SeriesPlot(c) => (
            if c.casewise { "CASEPLOT" } else { "TSPLOT" },
            named(&c.apply),
        ),
        Command::Curvefit(c) => ("CURVEFIT", named(&c.apply)),
        Command::Wls(c) => ("WLS", named(&c.apply)),
        Command::Pplot(c) => ("PPLOT", named(&c.apply)),
        _ => return None,
    })
}

/// The command with its APPLY name taken out: it then repeats the previous model.
fn unnamed(command: &Command) -> Command {
    let mut command = command.clone();
    match &mut command {
        Command::Correlogram(c) => c.apply = Some(None),
        Command::Season(c) => c.apply = Some(None),
        Command::Exsmooth(c) => c.apply = Some(None),
        Command::Arima(c) => c.apply.iter_mut().for_each(|(name, _)| *name = None),
        Command::Areg(c) => c.apply.iter_mut().for_each(|(name, _)| *name = None),
        Command::Spectra(c) => c.apply = Some(None),
        Command::TwoStage(c) => c.apply = Some(None),
        Command::SeriesPlot(c) => c.apply = Some(None),
        Command::Curvefit(c) => c.apply = Some(None),
        Command::Wls(c) => c.apply = Some(None),
        Command::Pplot(c) => c.apply = Some(None),
        _ => {}
    }
    command
}

impl Models {
    /// Takes the slot of `family` from `from`.
    fn restore(&mut self, from: &Models, family: &str) {
        match family {
            "ACF" => self.correlograms[0].clone_from(&from.correlograms[0]),
            "PACF" => self.correlograms[1].clone_from(&from.correlograms[1]),
            "CCF" => self.correlograms[2].clone_from(&from.correlograms[2]),
            "SEASON" => self.season.clone_from(&from.season),
            "EXSMOOTH" => self.exsmooth.clone_from(&from.exsmooth),
            "ARIMA" => self.arima.clone_from(&from.arima),
            "AREG" => self.areg.clone_from(&from.areg),
            "SPECTRA" => self.spectra.clone_from(&from.spectra),
            "2SLS" => self.two_stage.clone_from(&from.two_stage),
            "TSPLOT" => self.plots[0].clone_from(&from.plots[0]),
            "CASEPLOT" => self.plots[1].clone_from(&from.plots[1]),
            "CURVEFIT" => self.curvefit.clone_from(&from.curvefit),
            "WLS" => self.wls.clone_from(&from.wls),
            _ => {}
        }
    }
}

impl Engine {
    /// For `APPLY 'name'`: makes the named model the previous one of its kind and returns the
    /// command as a plain APPLY. None for any other command.
    pub(crate) fn named_apply(&mut self, command: &Command) -> Result<Option<Command>, Error> {
        let Some((family, Some(name))) = family(command) else {
            return Ok(None);
        };
        let wanted = name.to_uppercase();
        let Some(saved) = self.model_names.saved.iter().find(|s| s.name == wanted) else {
            return Err(Error(format!(
                "{family} APPLY: there is no model named '{name}'; the models are named MOD_1, \
                 MOD_2, ... in the order the session made them"
            )));
        };
        if saved.family != family {
            return Err(Error(format!(
                "{family} APPLY: '{}' was made by {}, not by {family}",
                saved.name, saved.family
            )));
        }
        self.ts_models.restore(&saved.models, family);
        if family == "PPLOT" {
            self.pplot_model.clone_from(&saved.pplot);
        }
        Ok(Some(unnamed(command)))
    }

    /// Gives the model `command` just made the next name.
    pub(crate) fn name_model(&mut self, command: &Command) {
        let Some((family, _)) = family(command) else {
            return;
        };
        self.model_names.next += 1;
        let name = format!("MOD_{}", self.model_names.next);
        self.model_names.saved.push(Saved {
            name,
            family,
            models: self.ts_models.clone(),
            pplot: self.pplot_model.clone(),
        });
    }
}
