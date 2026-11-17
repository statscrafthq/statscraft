//! The TwoStep Cluster and Identify Unusual Cases dialogs (TWOSTEP CLUSTER and DETECTANOMALY).
//! Options left as they are add nothing to the syntax, so the commands run with their own
//! defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::{names, number};
use statscraft_data::is_valid_name;

/// The tables TWOSTEP CLUSTER prints on request.
pub const TWOSTEP_PRINT: [(&str, &str); 3] = [
    ("IC", "Information criterion for each number of clusters"),
    ("COUNT", "Cluster sizes"),
    ("SUMMARY", "Cluster profiles"),
];

/// The variables DETECTANOMALY saves, in the order shown.
pub const ANOMALY_SAVE: [(&str, &str); 8] = [
    ("ANOMALY", "Anomaly index"),
    ("PEERID", "Peer group"),
    ("PEERSIZE", "Peer group size"),
    ("PEERPCTSIZE", "Peer group size in percent"),
    ("REASONVAR", "Reason: variable"),
    ("REASONMEASURE", "Reason: variable impact"),
    ("REASONVALUE", "Reason: the case's value"),
    ("REASONNORM", "Reason: the peer group's norm"),
];

/// The tables DETECTANOMALY prints, in the order shown; the list of unusual cases is the default.
pub const ANOMALY_PRINT: [(&str, &str); 5] = [
    ("CPS", "Case processing summary"),
    ("ANOMALYLIST", "List of unusual cases"),
    ("NORMS", "Norms of the variables"),
    ("ANOMALYSUMMARY", "Anomaly index summary"),
    ("REASONSUMMARY", "Reason summary"),
];

/// TWOSTEP CLUSTER: groups of cases from categorical and continuous variables.
#[derive(Clone, PartialEq, Debug)]
pub struct TwoStep {
    pub categorical: Vec<String>,
    pub continuous: Vec<String>,
    pub euclidean: bool,
    /// The number of clusters is chosen from 1 up to `max`; otherwise it is `fixed`.
    pub auto: bool,
    pub max: String,
    pub aic: bool,
    pub fixed: String,
    /// Continuous variables left as they are; the others are standardized.
    pub unstandardized: Vec<String>,
    /// A percent; 0 does no noise handling.
    pub noise: String,
    pub include: bool,
    /// One flag per entry of `TWOSTEP_PRINT`.
    pub print: [bool; 3],
    pub model_summary: bool,
    pub save: bool,
    pub save_name: String,
}

impl Default for TwoStep {
    fn default() -> Self {
        Self {
            categorical: Vec::new(),
            continuous: Vec::new(),
            euclidean: false,
            auto: true,
            max: "15".into(),
            aic: false,
            fixed: "3".into(),
            unstandardized: Vec::new(),
            noise: "0".into(),
            include: false,
            print: [false, true, true],
            model_summary: true,
            save: false,
            save_name: String::new(),
        }
    }
}

impl TwoStep {
    pub fn syntax(&self) -> Result<String, String> {
        if self.categorical.is_empty() && self.continuous.is_empty() {
            return Err("add at least one variable to Categorical or Continuous".into());
        }
        if self.euclidean && !self.categorical.is_empty() {
            return Err("Euclidean distance takes continuous variables only".into());
        }
        let mut text = String::from("TWOSTEP CLUSTER");
        if !self.continuous.is_empty() {
            text.push_str(&format!(
                "\n  /CONTINUOUS VARIABLES={}",
                names(&self.continuous, "Continuous")?
            ));
        }
        if !self.categorical.is_empty() {
            text.push_str(&format!(
                "\n  /CATEGORICAL VARIABLES={}",
                names(&self.categorical, "Categorical")?
            ));
        }
        if self.euclidean {
            text.push_str("\n  /DISTANCE=EUCLIDEAN");
        }
        if self.auto {
            let max = whole(&self.max, 1, "the largest number of clusters")?
                .ok_or("give the largest number of clusters")?;
            if (max, self.aic) != (15, false) {
                let method = if self.aic { " AIC" } else { "" };
                text.push_str(&format!("\n  /NUMCLUSTERS=AUTO {max}{method}"));
            }
        } else {
            let fixed = whole(&self.fixed, 1, "the number of clusters")?
                .ok_or("give the number of clusters")?;
            text.push_str(&format!("\n  /NUMCLUSTERS=FIXED={fixed}"));
        }
        if let Some(name) = self
            .unstandardized
            .iter()
            .find(|n| !self.continuous.contains(n))
        {
            return Err(format!("{name} is not a continuous variable"));
        }
        if !self.unstandardized.is_empty() {
            text.push_str(&format!(
                "\n  /NOSTANDARDIZE VARIABLES={}",
                names(&self.unstandardized, "Not standardized")?
            ));
        }
        let noise = parse(&self.noise, "the noise percentage")?;
        if !(0.0..=100.0).contains(&noise) {
            return Err("the noise percentage must be from 0 to 100".into());
        }
        if noise != 0.0 {
            text.push_str(&format!("\n  /HANDLENOISE={}", number(noise)));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        let print = keywords(&TWOSTEP_PRINT, &self.print);
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        if !self.model_summary {
            text.push_str("\n  /VIEWMODEL DISPLAY=NO");
        }
        if self.save {
            text.push_str("\n  /SAVE CLUSTER");
            let name = self.save_name.trim();
            if !name.is_empty() {
                if !is_valid_name(name) {
                    return Err(format!("{name} cannot name a variable"));
                }
                text.push_str(&format!(" VARIABLE={name}"));
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// DETECTANOMALY: the cases that stray most from their peer group.
#[derive(Clone, PartialEq, Debug)]
pub struct Anomaly {
    pub categorical: Vec<String>,
    pub scale: Vec<String>,
    /// Names the cases in the list; not an analysis variable.
    pub id: String,
    /// List the highest `percent` of the cases, or `number` of them.
    pub by_number: bool,
    pub percent: String,
    pub number: String,
    /// Keep only cases with an index of at least `cutoff`.
    pub use_cutoff: bool,
    pub cutoff: String,
    pub min_peers: String,
    pub max_peers: String,
    pub reasons: String,
    /// Put the mean in for a missing scale value and take missing as a category.
    pub apply_missing: bool,
    pub missing_share: bool,
    /// One flag per entry of `ANOMALY_SAVE`.
    pub save: [bool; 8],
    /// One flag per entry of `ANOMALY_PRINT`.
    pub print: [bool; 5],
}

impl Default for Anomaly {
    fn default() -> Self {
        Self {
            categorical: Vec::new(),
            scale: Vec::new(),
            id: String::new(),
            by_number: false,
            percent: "5".into(),
            number: "10".into(),
            use_cutoff: true,
            cutoff: "2".into(),
            min_peers: "1".into(),
            max_peers: "15".into(),
            reasons: "1".into(),
            apply_missing: false,
            missing_share: false,
            save: [false; 8],
            print: [false, true, false, false, false],
        }
    }
}

impl Anomaly {
    pub fn syntax(&self) -> Result<String, String> {
        if self.categorical.is_empty() && self.scale.is_empty() {
            return Err("add at least one variable to Categorical or Scale".into());
        }
        let mut text = String::from("DETECTANOMALY\n  /VARIABLES");
        if !self.scale.is_empty() {
            text.push_str(&format!(" SCALE={}", names(&self.scale, "Scale")?));
        }
        if !self.categorical.is_empty() {
            text.push_str(&format!(
                " CATEGORICAL={}",
                names(&self.categorical, "Categorical")?
            ));
        }
        let id = self.id.trim();
        if !id.is_empty() {
            text.push_str(&format!(" ID={id}"));
        }
        if self.apply_missing {
            text.push_str("\n  /HANDLEMISSING APPLY=YES");
            if self.missing_share {
                text.push_str(" CREATEMISPROPVAR=YES");
            }
        }
        let mut criteria = Vec::new();
        let min = whole(&self.min_peers, 1, "the fewest peer groups")?.unwrap_or(1);
        let max = whole(&self.max_peers, 1, "the most peer groups")?.unwrap_or(15);
        if min > max {
            return Err("the fewest peer groups is above the most".into());
        }
        if min != 1 {
            criteria.push(format!("MINNUMPEERS={min}"));
        }
        if max != 15 {
            criteria.push(format!("MAXNUMPEERS={max}"));
        }
        match whole(&self.reasons, 0, "the number of reasons")?.unwrap_or(1) {
            1 => {}
            n => criteria.push(format!("NUMREASONS={n}")),
        }
        if self.by_number {
            let n = whole(&self.number, 0, "the number of cases")?
                .ok_or("give the number of cases to list")?;
            criteria.push(format!("NUMANOMALOUSCASES={n}"));
        } else {
            let percent = parse(&self.percent, "the percentage of cases")?;
            if !(0.0..=100.0).contains(&percent) {
                return Err("the percentage of cases must be from 0 to 100".into());
            }
            if percent != 5.0 {
                criteria.push(format!("PCTANOMALOUSCASES={}", number(percent)));
            }
        }
        if self.use_cutoff {
            let cutoff = parse(&self.cutoff, "the anomaly index cutoff")?;
            if cutoff < 0.0 {
                return Err("the anomaly index cutoff cannot be below 0".into());
            }
            if cutoff != 2.0 {
                criteria.push(format!("ANOMALYCUTPOINT={}", number(cutoff)));
            }
        } else {
            criteria.push("ANOMALYCUTPOINT=NONE".into());
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        let save = keywords(&ANOMALY_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        if self.print != Self::default().print {
            let print = keywords(&ANOMALY_PRINT, &self.print);
            let print = if print.is_empty() {
                "NONE".to_string()
            } else {
                print.join(" ")
            };
            text.push_str(&format!("\n  /PRINT {print}"));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    #[test]
    fn two_step_checks_what_the_command_would_reject() {
        let err = |change: &dyn Fn(&mut TwoStep)| {
            let mut d = TwoStep {
                continuous: v(&["age"]),
                ..Default::default()
            };
            change(&mut d);
            d.syntax().unwrap_err()
        };
        assert_eq!(
            err(&|d| d.continuous.clear()),
            "add at least one variable to Categorical or Continuous"
        );
        assert_eq!(
            err(&|d| {
                d.euclidean = true;
                d.categorical = v(&["region"]);
            }),
            "Euclidean distance takes continuous variables only"
        );
        assert_eq!(
            err(&|d| d.unstandardized = v(&["monthly_spend"])),
            "monthly_spend is not a continuous variable"
        );
        assert_eq!(
            err(&|d| d.noise = "101".into()),
            "the noise percentage must be from 0 to 100"
        );
        assert_eq!(
            err(&|d| {
                d.auto = false;
                d.fixed = "0".into();
            }),
            "the number of clusters must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| {
                d.save = true;
                d.save_name = "2nd".into();
            }),
            "2nd cannot name a variable"
        );
    }

    #[test]
    fn two_step_writes_only_what_changed() {
        let mut session = open("survey_basic.sav");
        let mut d = TwoStep {
            continuous: v(&["age", "monthly_spend"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "TWOSTEP CLUSTER\n  /CONTINUOUS VARIABLES=age monthly_spend\n  /PRINT=COUNT SUMMARY."
        );
        d.categorical = v(&["region", "gender"]);
        d.auto = true;
        d.max = "6".into();
        d.aic = true;
        d.unstandardized = v(&["age"]);
        d.noise = "25".into();
        d.include = true;
        d.print = [true; 3];
        d.model_summary = false;
        d.save = true;
        d.save_name = "segment".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "TWOSTEP CLUSTER\n  /CONTINUOUS VARIABLES=age monthly_spend\n  \
             /CATEGORICAL VARIABLES=region gender\n  /NUMCLUSTERS=AUTO 6 AIC\n  \
             /NOSTANDARDIZE VARIABLES=age\n  /HANDLENOISE=25\n  /MISSING=INCLUDE\n  \
             /PRINT=IC COUNT SUMMARY\n  /VIEWMODEL DISPLAY=NO\n  /SAVE CLUSTER VARIABLE=segment."
        );
        assert!(run_in(&mut session, &syntax) >= 1);

        let d = TwoStep {
            continuous: v(&["age", "monthly_spend"]),
            euclidean: true,
            auto: false,
            fixed: "2".into(),
            print: [false; 3],
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "TWOSTEP CLUSTER\n  /CONTINUOUS VARIABLES=age monthly_spend\n  \
             /DISTANCE=EUCLIDEAN\n  /NUMCLUSTERS=FIXED=2."
        );
        assert!(run_in(&mut session, &syntax) >= 1);
    }

    #[test]
    fn anomaly_checks_what_the_command_would_reject() {
        let err = |change: &dyn Fn(&mut Anomaly)| {
            let mut d = Anomaly {
                scale: v(&["age"]),
                ..Default::default()
            };
            change(&mut d);
            d.syntax().unwrap_err()
        };
        assert_eq!(
            err(&|d| d.scale.clear()),
            "add at least one variable to Categorical or Scale"
        );
        assert_eq!(
            err(&|d| {
                d.min_peers = "5".into();
                d.max_peers = "3".into();
            }),
            "the fewest peer groups is above the most"
        );
        assert_eq!(
            err(&|d| d.percent = "120".into()),
            "the percentage of cases must be from 0 to 100"
        );
        assert_eq!(
            err(&|d| {
                d.by_number = true;
                d.number.clear();
            }),
            "give the number of cases to list"
        );
        assert_eq!(
            err(&|d| d.cutoff = "-1".into()),
            "the anomaly index cutoff cannot be below 0"
        );
    }

    #[test]
    fn anomaly_writes_only_what_changed() {
        let mut session = open("survey_basic.sav");
        let mut d = Anomaly {
            scale: v(&["age", "monthly_spend"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "DETECTANOMALY\n  /VARIABLES SCALE=age monthly_spend."
        );
        d.categorical = v(&["region"]);
        d.id = "resp_id".into();
        d.apply_missing = true;
        d.missing_share = true;
        d.min_peers = "2".into();
        d.max_peers = "6".into();
        d.reasons = "2".into();
        d.by_number = true;
        d.number = "8".into();
        d.use_cutoff = false;
        d.save = [true; 8];
        d.print = [true; 5];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "DETECTANOMALY\n  /VARIABLES SCALE=age monthly_spend CATEGORICAL=region ID=resp_id\n  \
             /HANDLEMISSING APPLY=YES CREATEMISPROPVAR=YES\n  /CRITERIA MINNUMPEERS=2 \
             MAXNUMPEERS=6 NUMREASONS=2 NUMANOMALOUSCASES=8 ANOMALYCUTPOINT=NONE\n  \
             /SAVE ANOMALY PEERID PEERSIZE PEERPCTSIZE REASONVAR REASONMEASURE REASONVALUE \
             REASONNORM\n  /PRINT CPS ANOMALYLIST NORMS ANOMALYSUMMARY REASONSUMMARY."
        );
        assert!(run_in(&mut session, &syntax) >= 1);

        d.by_number = false;
        d.percent = "10".into();
        d.use_cutoff = true;
        d.cutoff = "1.5".into();
        d.print = [false; 5];
        let syntax = d.syntax().unwrap();
        assert!(syntax.contains("PCTANOMALOUSCASES=10 ANOMALYCUTPOINT=1.5"));
        assert!(syntax.ends_with("/PRINT NONE."));
    }
}
