//! The Complex Samples sampling dialogs: Sampling Plan (CSPLAN SAMPLE) and Select a Sample
//! (CSSELECT).

use crate::stats_dialogs::parse;
use crate::syntax_text::{number, quote};

/// Keyword and label of every sampling method; the first four draw units with equal chances.
pub const SAMPLING_METHODS: [(&str, &str); 11] = [
    ("SIMPLE_WOR", "Simple random, without replacement"),
    ("SIMPLE_WR", "Simple random, with replacement"),
    ("SIMPLE_SYSTEMATIC", "Simple systematic"),
    ("SIMPLE_CHROMY", "Sequential (Chromy)"),
    ("PPS_WOR", "PPS, without replacement (Hanurav and Vijayan)"),
    ("PPS_WR", "PPS, with replacement"),
    ("PPS_SYSTEMATIC", "PPS systematic"),
    ("PPS_BREWER", "PPS Brewer (two units)"),
    ("PPS_MURTHY", "PPS Murthy (two units)"),
    ("PPS_SAMPFORD", "PPS Sampford"),
    ("PPS_CHROMY", "PPS sequential (Chromy)"),
];

/// Index of the last method with equal chances; the PPS methods come after it.
pub const LAST_SIMPLE_METHOD: usize = 3;

const SIMPLE_WR: usize = 1;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct SamplingStage {
    pub label: String,
    pub strata: Vec<String>,
    pub clusters: Vec<String>,
    /// Index into `SAMPLING_METHODS`.
    pub method: usize,
    /// The share of the units instead of their number.
    pub by_rate: bool,
    pub amount: String,
    pub min_size: String,
    pub max_size: String,
    /// The measure of size is the number of cases in the cluster instead of a variable.
    pub mos_from_data: bool,
    pub mos_variable: String,
    pub mos_min: String,
    pub mos_max: String,
    /// A name for the stage's inclusion probability variable; empty for the default.
    pub inclusion_name: String,
}

impl SamplingStage {
    pub fn is_pps(&self) -> bool {
        self.method > LAST_SIMPLE_METHOD
    }
}

/// A sample has at most this many stages here.
pub const MOST_SAMPLING_STAGES: usize = 2;

/// CSPLAN SAMPLE: how a complex sample is drawn, written to a plan file.
#[derive(Clone, PartialEq, Debug)]
pub struct SamplingPlan {
    pub path: String,
    /// A name for the final sample weight variable; empty for the default.
    pub weight_name: String,
    pub stages: Vec<SamplingStage>,
}

impl Default for SamplingPlan {
    fn default() -> Self {
        Self {
            path: String::new(),
            weight_name: String::new(),
            stages: vec![SamplingStage::default()],
        }
    }
}

fn optional(text: &str, what: &str) -> Result<Option<String>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse(text, what).map(|n| Some(number(n)))
}

impl SamplingPlan {
    /// The measure-of-size variables, which must be numbers.
    pub fn numeric(&self) -> impl Iterator<Item = &String> {
        self.stages
            .iter()
            .filter(|s| s.is_pps() && !s.mos_from_data)
            .map(|s| &s.mos_variable)
    }

    /// A second stage can follow a first one that has clusters and is not drawn with replacement.
    pub fn can_add_stage(&self) -> bool {
        self.stages.len() < MOST_SAMPLING_STAGES
            && self
                .stages
                .last()
                .is_some_and(|s| !s.clusters.is_empty() && s.method != SIMPLE_WR)
    }

    pub fn syntax(&self) -> Result<String, String> {
        let path = self.path.trim();
        if path.is_empty() {
            return Err("give the plan file".into());
        }
        let mut text = format!(
            "CSPLAN SAMPLE\n  /PLAN FILE={}",
            quote(&with_extension(path, "json"))
        );
        let weight = self.weight_name.trim();
        if !weight.is_empty() {
            text.push_str(&format!("\n  /PLANVARS SAMPLEWEIGHT={weight}"));
        }
        let last = self.stages.len() - 1;
        for (i, stage) in self.stages.iter().enumerate() {
            let n = i + 1;
            let (method, _) = SAMPLING_METHODS[stage.method];
            if i < last && stage.clusters.is_empty() {
                return Err(format!(
                    "stage {n} is followed by another stage, so it needs clusters"
                ));
            }
            if i < last && stage.method == SIMPLE_WR {
                return Err(format!(
                    "stage {n} is drawn with replacement, so it must be the last"
                ));
            }
            if i > 0 && stage.is_pps() {
                return Err(format!(
                    "stage {n}: PPS methods are for the first stage only"
                ));
            }
            if stage.is_pps() && stage.clusters.is_empty() {
                return Err(format!("stage {n}: a PPS method needs clusters"));
            }
            text.push_str("\n  /DESIGN");
            let label = stage.label.trim();
            if !label.is_empty() {
                text.push_str(&format!(" STAGELABEL={}", quote(label)));
            }
            if !stage.strata.is_empty() {
                text.push_str(&format!(" STRATA={}", stage.strata.join(" ")));
            }
            if !stage.clusters.is_empty() {
                text.push_str(&format!(" CLUSTER={}", stage.clusters.join(" ")));
            }
            text.push_str(&format!("\n  /METHOD TYPE={method}"));
            let what = if stage.by_rate { "rate" } else { "sample size" };
            let value = parse(&stage.amount, &format!("the {what} of stage {n}"))?;
            if stage.by_rate {
                if !(value > 0.0 && value <= 1.0) {
                    return Err(format!(
                        "the rate of stage {n} must be above 0 and at most 1"
                    ));
                }
                text.push_str(&format!("\n  /RATE VALUE={}", number(value)));
                let min = optional(&stage.min_size, &format!("the smallest size of stage {n}"))?;
                let max = optional(&stage.max_size, &format!("the largest size of stage {n}"))?;
                if let Some(min) = min {
                    text.push_str(&format!(" MINSIZE={min}"));
                }
                if let Some(max) = max {
                    text.push_str(&format!(" MAXSIZE={max}"));
                }
            } else {
                if value < 1.0 || value.fract() != 0.0 {
                    return Err(format!(
                        "the sample size of stage {n} must be a whole number of at least 1"
                    ));
                }
                text.push_str(&format!("\n  /SIZE VALUE={}", number(value)));
            }
            if stage.is_pps() {
                text.push_str("\n  /MOS ");
                if stage.mos_from_data {
                    text.push_str("SOURCE=FROMDATA");
                } else if stage.mos_variable.trim().is_empty() {
                    return Err(format!("choose the measure of size variable of stage {n}"));
                } else {
                    text.push_str(&format!("VARIABLE={}", stage.mos_variable.trim()));
                }
                let min = optional(
                    &stage.mos_min,
                    &format!("the smallest measure of size of stage {n}"),
                )?;
                let max = optional(
                    &stage.mos_max,
                    &format!("the largest measure of size of stage {n}"),
                )?;
                if let Some(min) = min {
                    text.push_str(&format!(" MIN={min}"));
                }
                if let Some(max) = max {
                    text.push_str(&format!(" MAX={max}"));
                }
            }
            let name = stage.inclusion_name.trim();
            if !name.is_empty() {
                text.push_str(&format!("\n  /STAGEVARS INCLPROB({name})"));
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// CSSELECT: draw the sample a plan describes from the active data.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SampleSelection {
    pub plan: String,
    /// A fixed seed instead of a random one.
    pub seed_fixed: bool,
    pub seed: String,
    /// Write the selected cases to a file instead of adding the weights to the active data.
    pub to_file: bool,
    pub outfile: String,
    /// Rename a variable of the data that has a name the new variables need.
    pub rename: bool,
    pub joint_file: String,
    pub counts: bool,
}

/// A file name with `extension` when it has none.
fn with_extension(path: &str, extension: &str) -> String {
    match std::path::Path::new(path).extension() {
        Some(_) => path.to_string(),
        None => format!("{path}.{extension}"),
    }
}

impl SampleSelection {
    pub fn syntax(&self) -> Result<String, String> {
        let plan = self.plan.trim();
        if plan.is_empty() {
            return Err("choose the plan file".into());
        }
        let mut text = format!(
            "CSSELECT\n  /PLAN FILE={}",
            quote(&with_extension(plan, "json"))
        );
        if self.seed_fixed {
            let seed = parse(&self.seed, "the seed")?;
            if seed < 0.0 || seed.fract() != 0.0 {
                return Err("the seed must be a whole number, 0 or more".into());
            }
            text.push_str(&format!("\n  /CRITERIA SEED={}", number(seed)));
        }
        if self.rename {
            text.push_str("\n  /DATA RENAMEVARS");
        }
        if self.to_file {
            let file = self.outfile.trim();
            if file.is_empty() {
                return Err("give the file for the selected cases".into());
            }
            text.push_str(&format!(
                "\n  /SAMPLEFILE OUTFILE={}",
                quote(&with_extension(file, "sav"))
            ));
        }
        let joint = self.joint_file.trim();
        if !joint.is_empty() {
            text.push_str(&format!(
                "\n  /JOINTPROB OUTFILE={}",
                quote(&with_extension(joint, "sav"))
            ));
        }
        if self.counts {
            text.push_str("\n  /PRINT SELECTION CPS");
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(strata: &str, clusters: &str, amount: &str) -> SamplingStage {
        let list = |t: &str| t.split_whitespace().map(String::from).collect();
        SamplingStage {
            strata: list(strata),
            clusters: list(clusters),
            amount: amount.into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_sampling_plan_writes_csplan_sample() {
        let mut d = SamplingPlan::default();
        assert_eq!(d.syntax().unwrap_err(), "give the plan file");
        d.path = "draw".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the sample size of stage 1 must be a number"
        );
        d.stages[0] = stage("region", "", "3");
        assert_eq!(
            d.syntax().unwrap(),
            "CSPLAN SAMPLE\n  /PLAN FILE='draw.json'\n  /DESIGN STRATA=region\n  \
             /METHOD TYPE=SIMPLE_WOR\n  /SIZE VALUE=3."
        );
        d.weight_name = "w".into();
        d.stages[0] = SamplingStage {
            label: "Areas".into(),
            method: 5,
            mos_variable: "acres".into(),
            mos_max: "40".into(),
            inclusion_name: "p1".into(),
            ..stage("region", "farm", "2")
        };
        assert_eq!(
            d.syntax().unwrap(),
            "CSPLAN SAMPLE\n  /PLAN FILE='draw.json'\n  /PLANVARS SAMPLEWEIGHT=w\n  \
             /DESIGN STAGELABEL='Areas' STRATA=region CLUSTER=farm\n  /METHOD TYPE=PPS_WR\n  \
             /SIZE VALUE=2\n  /MOS VARIABLE=acres MAX=40\n  /STAGEVARS INCLPROB(p1)."
        );
        assert!(d.can_add_stage());
        d.stages.push(SamplingStage {
            by_rate: true,
            min_size: "2".into(),
            ..stage("", "", "0.25")
        });
        let text = d.syntax().unwrap();
        assert!(
            text.ends_with("/DESIGN\n  /METHOD TYPE=SIMPLE_WOR\n  /RATE VALUE=0.25 MINSIZE=2."),
            "{text}"
        );
        d.stages[1].method = 4;
        assert!(d.syntax().unwrap_err().contains("first stage only"));
        d.stages[1].method = 0;
        d.stages[0].clusters.clear();
        assert!(d.syntax().unwrap_err().contains("needs clusters"));
    }

    #[test]
    fn the_selection_writes_csselect() {
        let mut d = SampleSelection::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the plan file");
        d.plan = "draw".into();
        assert_eq!(d.syntax().unwrap(), "CSSELECT\n  /PLAN FILE='draw.json'.");
        d.seed_fixed = true;
        d.to_file = true;
        d.rename = true;
        d.counts = true;
        d.joint_file = "joint".into();
        assert_eq!(d.syntax().unwrap_err(), "the seed must be a number");
        d.seed = "4711".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the file for the selected cases"
        );
        d.outfile = "picked.sav".into();
        assert_eq!(
            d.syntax().unwrap(),
            "CSSELECT\n  /PLAN FILE='draw.json'\n  /CRITERIA SEED=4711\n  /DATA RENAMEVARS\n  \
             /SAMPLEFILE OUTFILE='picked.sav'\n  /JOINTPROB OUTFILE='joint.sav'\n  \
             /PRINT SELECTION CPS."
        );
    }
}
