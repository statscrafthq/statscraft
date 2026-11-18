//! The Multilayer Perceptron and Radial Basis Function dialogs (MLP and RBF). The two share
//! the variables, rescaling, partition, output and save choices. Options left as they are add
//! nothing to the syntax, so the commands run with their own defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::{names, number};
use statscraft_data::is_valid_name;

/// How the dependents are treated; the keyword is empty to use the level the data gives.
pub const LEVELS: [(&str, &str); 4] = [
    ("", "From the data"),
    ("S", "Scale"),
    ("O", "Ordinal"),
    ("N", "Nominal"),
];

/// How the covariates are rescaled; the first is the command's own default.
pub const COVARIATE_RESCALE: [(&str, &str); 4] = [
    ("STANDARDIZED", "Standardized"),
    ("NORMALIZED", "Normalized (0 to 1)"),
    ("ADJNORMALIZED", "Adjusted normalized (-1 to 1)"),
    ("NONE", "None"),
];

/// How the dependents are rescaled; the first leaves the choice to the command.
pub const DEPENDENT_RESCALE: [(&str, &str); 5] = [
    ("", "Default"),
    ("STANDARDIZED", "Standardized"),
    ("NORMALIZED", "Normalized (0 to 1)"),
    ("ADJNORMALIZED", "Adjusted normalized (-1 to 1)"),
    ("NONE", "None"),
];

/// The tables on request, in the order shown.
pub const PRINT: [(&str, &str); 6] = [
    ("CPS", "Case processing summary"),
    ("NETWORKINFO", "Network information"),
    ("SUMMARY", "Model summary"),
    ("CLASSIFICATION", "Classification"),
    ("SOLUTION", "Parameter estimates"),
    ("IMPORTANCE", "Variable importance"),
];

/// The charts on request, in the order shown.
pub const PLOT: [(&str, &str); 6] = [
    ("NETWORK", "Network diagram"),
    ("PREDICTED", "Predicted by observed"),
    ("RESIDUAL", "Residual by predicted"),
    ("ROC", "ROC curve"),
    ("GAIN", "Cumulative gains"),
    ("LIFT", "Lift"),
];

const PRINT_DEFAULT: [bool; 6] = [true, true, true, true, false, false];
const PLOT_DEFAULT: [bool; 6] = [true, false, false, false, false, false];

/// The activation of the hidden units of an MLP.
pub const HIDDEN: [(&str, &str); 2] = [("TANH", "Hyperbolic tangent"), ("SIGMOID", "Sigmoid")];

/// The activation of the output units; the first leaves the choice to the command.
pub const OUTPUT: [(&str, &str); 5] = [
    ("", "Default for the dependents"),
    ("IDENTITY", "Identity"),
    ("SIGMOID", "Sigmoid"),
    ("SOFTMAX", "Softmax"),
    ("TANH", "Hyperbolic tangent"),
];

/// How an MLP is trained; the first is the command's own default.
pub const TRAINING: [(&str, &str); 3] = [
    ("BATCH", "Batch"),
    ("ONLINE", "Online"),
    ("MINIBATCH", "Mini-batch"),
];

/// How an MLP is optimized; the first leaves the choice to the training type.
pub const OPTIMIZATION: [(&str, &str); 3] = [
    ("", "Default for the training type"),
    ("GRADIENTDESCENT", "Gradient descent"),
    ("SCALEDCONJUGATE", "Scaled conjugate gradient"),
];

/// The activation of the hidden units of an RBF; the first is the command's own default.
pub const RBF_HIDDEN: [(&str, &str); 2] = [
    ("NRBF", "Normalized radial basis function"),
    ("ORBF", "Ordinary radial basis function"),
];

/// What both networks take: the variables, rescaling, partition, output and save choices.
#[derive(Clone, PartialEq, Debug)]
pub struct Common {
    pub dependents: Vec<String>,
    /// Index into `LEVELS`, for every dependent.
    pub level: usize,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `COVARIATE_RESCALE`.
    pub rescale_covariates: usize,
    /// Index into `DEPENDENT_RESCALE`.
    pub rescale_dependents: usize,
    /// The cases are split by a variable (above 0 trains, 0 tests, below 0 holds out).
    pub by_variable: bool,
    pub partition_variable: String,
    pub training: String,
    pub testing: String,
    pub holdout: String,
    pub include_missing: bool,
    /// One flag per entry of `PRINT` and of `PLOT`.
    pub print: [bool; 6],
    pub plot: [bool; 6],
    pub save_predicted: bool,
    /// Names, separated by spaces, one per dependent; empty takes the default.
    pub predicted_names: String,
    pub save_probabilities: bool,
    /// Roots, separated by spaces, one per categorical dependent; empty takes the default.
    pub probability_roots: String,
}

impl Default for Common {
    fn default() -> Self {
        Self {
            dependents: Vec::new(),
            level: 0,
            factors: Vec::new(),
            covariates: Vec::new(),
            rescale_covariates: 0,
            rescale_dependents: 0,
            by_variable: false,
            partition_variable: String::new(),
            training: "70".into(),
            testing: "30".into(),
            holdout: "0".into(),
            include_missing: false,
            print: PRINT_DEFAULT,
            plot: PLOT_DEFAULT,
            save_predicted: false,
            predicted_names: String::new(),
            save_probabilities: false,
            probability_roots: String::new(),
        }
    }
}

/// Variable names typed in one field, for what SAVE creates.
fn new_names(text: &str, what: &str) -> Result<Vec<String>, String> {
    let list: Vec<String> = text.split_whitespace().map(String::from).collect();
    match list.iter().find(|n| !is_valid_name(n)) {
        Some(bad) => Err(format!("{bad} cannot name {what}")),
        None => Ok(list),
    }
}

impl Common {
    /// The command name, the variables, the rescaling and the partition.
    fn head(&self, command: &str) -> Result<String, String> {
        if self.dependents.is_empty() {
            return Err("add at least one dependent variable".into());
        }
        if self.factors.is_empty() && self.covariates.is_empty() {
            return Err("add at least one factor or covariate".into());
        }
        let mut all: Vec<&String> = self
            .dependents
            .iter()
            .chain(&self.factors)
            .chain(&self.covariates)
            .collect();
        if self.by_variable {
            if self.partition_variable.trim().is_empty() {
                return Err("choose the partition variable".into());
            }
            all.push(&self.partition_variable);
        }
        if let Some((_, name)) = all
            .iter()
            .enumerate()
            .find(|(i, n)| all[..*i].iter().any(|m| m.eq_ignore_ascii_case(n)))
        {
            return Err(format!("{name} has two roles; keep it in one"));
        }
        let mut text = String::from(command);
        for dependent in &self.dependents {
            text.push_str(&format!(
                " {}",
                names(std::slice::from_ref(dependent), "Dependent variables")?
            ));
            if self.level != 0 {
                text.push_str(&format!(" (MLEVEL={})", LEVELS[self.level].0));
            }
        }
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", names(&self.factors, "Factors")?));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", names(&self.covariates, "Covariates")?));
        }
        let mut rescale = Vec::new();
        if self.rescale_covariates != 0 {
            let key = COVARIATE_RESCALE[self.rescale_covariates].0;
            rescale.push(format!("COVARIATE={key}"));
        }
        if self.rescale_dependents != 0 {
            let key = DEPENDENT_RESCALE[self.rescale_dependents].0;
            rescale.push(format!("DEPENDENT={key}"));
        }
        if !rescale.is_empty() {
            text.push_str(&format!("\n  /RESCALE {}", rescale.join(" ")));
        }
        if self.by_variable {
            text.push_str(&format!(
                "\n  /PARTITION VARIABLE={}",
                self.partition_variable.trim()
            ));
        } else {
            let share = |text: &str, least, what| {
                whole(text, least, what)?.ok_or_else(|| format!("give {what}"))
            };
            let shares = (
                share(&self.training, 1, "the training share")?,
                share(&self.testing, 0, "the testing share")?,
                share(&self.holdout, 0, "the holdout share")?,
            );
            if shares != (70, 30, 0) {
                text.push_str(&format!(
                    "\n  /PARTITION TRAINING={} TESTING={} HOLDOUT={}",
                    shares.0, shares.1, shares.2
                ));
            }
        }
        Ok(text)
    }

    /// MISSING, PRINT, PLOT and SAVE, when they differ from the command's defaults.
    fn tail(&self) -> Result<String, String> {
        let mut text = String::new();
        if self.include_missing {
            text.push_str("\n  /MISSING USERMISSING=INCLUDE");
        }
        for (name, options, flags, default) in [
            ("PRINT", &PRINT, &self.print, PRINT_DEFAULT),
            ("PLOT", &PLOT, &self.plot, PLOT_DEFAULT),
        ] {
            if *flags != default {
                let on = keywords(options, flags);
                let list = if on.is_empty() {
                    "NONE".into()
                } else {
                    on.join(" ")
                };
                text.push_str(&format!("\n  /{name} {list}"));
            }
        }
        let mut save = Vec::new();
        if self.save_predicted {
            let list = new_names(&self.predicted_names, "a predicted value")?;
            save.push(match list.is_empty() {
                true => "PREDVAL".to_string(),
                false => format!("PREDVAL({})", list.join(" ")),
            });
        }
        if self.save_probabilities {
            let list = new_names(&self.probability_roots, "a pseudo-probability")?;
            save.push(match list.is_empty() {
                true => "PSEUDOPROB".to_string(),
                false => format!("PSEUDOPROB({})", list.join(" ")),
            });
        }
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        Ok(text)
    }
}

/// MLP: a network with one or two hidden layers.
#[derive(Clone, PartialEq, Debug)]
pub struct Mlp {
    pub common: Common,
    /// Search the number of hidden units, in one layer, between `min_units` and `max_units`.
    pub automatic: bool,
    pub min_units: String,
    pub max_units: String,
    pub layers: usize,
    /// Let the command pick the units of the layers, else `units` for each layer.
    pub auto_units: bool,
    pub units: [String; 2],
    /// Index into `HIDDEN`.
    pub hidden: usize,
    /// Index into `OUTPUT`.
    pub output: usize,
    /// Index into `TRAINING`.
    pub training: usize,
    /// Mini-batch size; empty is chosen by the command.
    pub batch_size: String,
    /// Index into `OPTIMIZATION`.
    pub optimization: usize,
    /// Stop after this many passes over the data; empty is chosen by the command.
    pub max_epochs: String,
    /// Stop after this many minutes; empty has no time limit.
    pub max_minutes: String,
    /// Stop after this many steps without a new lowest error.
    pub error_steps: String,
}

impl Default for Mlp {
    fn default() -> Self {
        Self {
            common: Common::default(),
            automatic: true,
            min_units: "1".into(),
            max_units: "50".into(),
            layers: 1,
            auto_units: true,
            units: ["5".into(), "3".into()],
            hidden: 0,
            output: 0,
            training: 0,
            batch_size: String::new(),
            optimization: 0,
            max_epochs: String::new(),
            max_minutes: "15".into(),
            error_steps: "1".into(),
        }
    }
}

impl Mlp {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = self.common.head("MLP")?;
        if self.automatic {
            let min = whole(&self.min_units, 1, "the fewest units")?.unwrap_or(1);
            let max = whole(&self.max_units, 1, "the most units")?.unwrap_or(50);
            if min >= max {
                return Err("the fewest units must be below the most".into());
            }
            if (min, max) != (1, 50) {
                text.push_str(&format!(
                    "\n  /ARCHITECTURE AUTOMATIC=YES (MINUNITS={min}, MAXUNITS={max})"
                ));
            }
        } else {
            text.push_str(&format!(
                "\n  /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS={}",
                self.layers
            ));
            if !self.auto_units {
                let mut units = Vec::new();
                for unit in &self.units[..self.layers] {
                    let n = whole(unit, 1, "the number of units")?
                        .ok_or("give the number of units of each layer")?;
                    units.push(n.to_string());
                }
                text.push_str(&format!(" (NUMUNITS={})", units.join(", ")));
            }
            if self.hidden != 0 {
                text.push_str(&format!(" HIDDENFUNCTION={}", HIDDEN[self.hidden].0));
            }
            if self.output != 0 {
                text.push_str(&format!(" OUTPUTFUNCTION={}", OUTPUT[self.output].0));
            }
        }
        let mut criteria = Vec::new();
        if self.training != 0 {
            criteria.push(format!("TRAINING={}", TRAINING[self.training].0));
            if self.optimization == 2 {
                return Err("online and mini-batch training use gradient descent".into());
            }
        }
        if self.training == 2
            && let Some(n) = whole(&self.batch_size, 2, "the mini-batch size")?
        {
            criteria.push(format!("MINIBATCHSIZE={n}"));
        }
        if self.optimization != 0 {
            criteria.push(format!(
                "OPTIMIZATION={}",
                OPTIMIZATION[self.optimization].0
            ));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        let mut stop = Vec::new();
        let steps = whole(&self.error_steps, 1, "the steps without improvement")?.unwrap_or(1);
        if steps != 1 {
            stop.push(format!("ERRORSTEPS={steps}"));
        }
        match whole(&self.max_minutes, 1, "the time limit")? {
            Some(15) => {}
            Some(n) => stop.push(format!("TRAININGTIMER=ON (MAXTIME={n})")),
            None => stop.push("TRAININGTIMER=OFF".into()),
        }
        if let Some(n) = whole(&self.max_epochs, 1, "the most passes")? {
            stop.push(format!("MAXEPOCHS={n}"));
        }
        if !stop.is_empty() {
            text.push_str(&format!("\n  /STOPPINGRULES {}", stop.join(" ")));
        }
        text.push_str(&self.common.tail()?);
        text.push('.');
        Ok(text)
    }
}

/// RBF: a network of Gaussian hidden units.
#[derive(Clone, PartialEq, Debug)]
pub struct Rbf {
    pub common: Common,
    /// Search the number of units; otherwise it is `units`.
    pub automatic: bool,
    /// Limits of the search; both empty leave them to the command.
    pub min_units: String,
    pub max_units: String,
    pub units: String,
    /// Index into `RBF_HIDDEN`.
    pub hidden: usize,
    /// Empty lets the command choose how far the Gaussians overlap.
    pub overlap: String,
}

impl Default for Rbf {
    fn default() -> Self {
        Self {
            common: Common::default(),
            automatic: true,
            min_units: String::new(),
            max_units: String::new(),
            units: "10".into(),
            hidden: 0,
            overlap: String::new(),
        }
    }
}

impl Rbf {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = self.common.head("RBF")?;
        let mut architecture = Vec::new();
        if self.automatic {
            let min = whole(&self.min_units, 1, "the fewest units")?;
            let max = whole(&self.max_units, 1, "the most units")?;
            if min.is_some() || max.is_some() {
                if min.zip(max).is_some_and(|(a, b)| a > b) {
                    return Err("the fewest units is above the most".into());
                }
                let part = |n: Option<u32>| n.map_or("AUTO".to_string(), |n| n.to_string());
                architecture.push(format!("MINUNITS={} MAXUNITS={}", part(min), part(max)));
            }
        } else {
            let n =
                whole(&self.units, 1, "the number of units")?.ok_or("give the number of units")?;
            architecture.push(format!("NUMUNITS={n}"));
        }
        if self.hidden != 0 {
            architecture.push(format!("HIDDENFUNCTION={}", RBF_HIDDEN[self.hidden].0));
        }
        if !architecture.is_empty() {
            text.push_str(&format!("\n  /ARCHITECTURE {}", architecture.join(" ")));
        }
        if !self.overlap.trim().is_empty() {
            let overlap = parse(&self.overlap, "the overlap")?;
            if overlap <= 0.0 {
                return Err("the overlap must be above 0".into());
            }
            text.push_str(&format!("\n  /CRITERIA OVERLAP={}", number(overlap)));
        }
        text.push_str(&self.common.tail()?);
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn mlp() -> Mlp {
        Mlp {
            common: Common {
                dependents: v(&["monthly_spend"]),
                covariates: v(&["age"]),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn mlp_writes_only_what_changed() {
        let mut d = mlp();
        assert_eq!(d.syntax().unwrap(), "MLP monthly_spend WITH age.");
        d.common.dependents = v(&["gender", "region"]);
        d.common.level = 3;
        d.common.factors = v(&["q2_recommend"]);
        d.common.rescale_covariates = 2;
        d.common.rescale_dependents = 2;
        d.common.testing = "20".into();
        d.common.holdout = "10".into();
        d.automatic = false;
        d.layers = 2;
        d.auto_units = false;
        d.hidden = 1;
        d.output = 3;
        d.training = 2;
        d.batch_size = "32".into();
        d.optimization = 1;
        d.error_steps = "3".into();
        d.max_minutes = String::new();
        d.max_epochs = "50".into();
        d.common.include_missing = true;
        d.common.print = [false, false, true, true, true, true];
        d.common.plot = [false; 6];
        d.common.save_predicted = true;
        d.common.predicted_names = "p1 p2".into();
        d.common.save_probabilities = true;
        assert_eq!(
            d.syntax().unwrap(),
            "MLP gender (MLEVEL=N) region (MLEVEL=N) BY q2_recommend WITH age\n  \
             /RESCALE COVARIATE=ADJNORMALIZED DEPENDENT=NORMALIZED\n  \
             /PARTITION TRAINING=70 TESTING=20 HOLDOUT=10\n  \
             /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS=2 (NUMUNITS=5, 3) HIDDENFUNCTION=SIGMOID \
             OUTPUTFUNCTION=SOFTMAX\n  \
             /CRITERIA TRAINING=MINIBATCH MINIBATCHSIZE=32 OPTIMIZATION=GRADIENTDESCENT\n  \
             /STOPPINGRULES ERRORSTEPS=3 TRAININGTIMER=OFF MAXEPOCHS=50\n  \
             /MISSING USERMISSING=INCLUDE\n  /PRINT SUMMARY CLASSIFICATION SOLUTION IMPORTANCE\n  \
             /PLOT NONE\n  /SAVE PREDVAL(p1 p2) PSEUDOPROB."
        );
        d.automatic = true;
        d.min_units = "2".into();
        d.max_units = "6".into();
        assert!(
            d.syntax()
                .unwrap()
                .contains("AUTOMATIC=YES (MINUNITS=2, MAXUNITS=6)")
        );
    }

    #[test]
    fn mlp_refuses_what_the_command_would() {
        let mut d = mlp();
        d.common.dependents.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one dependent variable"
        );
        let mut d = mlp();
        d.common.factors = v(&["age"]);
        assert_eq!(d.syntax().unwrap_err(), "age has two roles; keep it in one");
        let mut d = mlp();
        d.training = 1;
        d.optimization = 2;
        assert_eq!(
            d.syntax().unwrap_err(),
            "online and mini-batch training use gradient descent"
        );
        let mut d = mlp();
        d.min_units = "9".into();
        d.max_units = "3".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the fewest units must be below the most"
        );
    }

    #[test]
    fn rbf_writes_only_what_changed() {
        let mut d = Rbf {
            common: Common {
                dependents: v(&["monthly_spend"]),
                factors: v(&["region"]),
                covariates: v(&["age"]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "RBF monthly_spend BY region WITH age.");
        d.min_units = "3".into();
        assert!(
            d.syntax()
                .unwrap()
                .contains("/ARCHITECTURE MINUNITS=3 MAXUNITS=AUTO")
        );
        d.automatic = false;
        d.hidden = 1;
        d.overlap = "1.5".into();
        d.common.by_variable = true;
        d.common.partition_variable = "q2_recommend".into();
        d.common.save_predicted = true;
        assert_eq!(
            d.syntax().unwrap(),
            "RBF monthly_spend BY region WITH age\n  /PARTITION VARIABLE=q2_recommend\n  \
             /ARCHITECTURE NUMUNITS=10 HIDDENFUNCTION=ORBF\n  /CRITERIA OVERLAP=1.5\n  \
             /SAVE PREDVAL."
        );
        d.overlap = "0".into();
        assert_eq!(d.syntax().unwrap_err(), "the overlap must be above 0");
    }

    #[test]
    fn the_mlp_text_runs() {
        let mut session = open("survey_basic.sav");
        let mut d = mlp();
        d.automatic = false;
        d.common.save_predicted = true;
        assert!(run_in(&mut session, &d.syntax().unwrap()) >= 1);
    }
}
