//! The ROC Analysis dialog (ROC ANALYSIS): areas under the curves of several test variables,
//! compared on the same cases or between two groups, with precision-recall curves and
//! classifier measures. The older ROC Curve dialog stays beside it. Options left as they are
//! add nothing to the syntax, so the command runs with its own defaults.

use crate::stats_dialogs::{numeric, parse, percent};
use crate::syntax_text::{names, number, value};
use statscraft_data::Dataset;

/// The curves of /PLOT, the ROC curve first (the command's own choice).
pub const ROC_CURVES: [(&str, &str); 4] = [
    ("ROC", "ROC curve"),
    ("PR", "Precision-recall curve"),
    ("ROC PR", "Both curves"),
    ("NONE", "No curves"),
];
const NO_CURVES: usize = 3;

/// How a grouping variable makes the two groups.
pub const ROC_SELECTS: [(&str, &str); 3] = [
    ("LEVEL", "Two values of the variable"),
    (
        "MIDPOINT",
        "At or above the midpoint of its range, and below it",
    ),
    ("CUTPOINT", "At or above a value, and below it"),
];
pub const LEVEL: usize = 0;
pub const CUTPOINT: usize = 2;

/// ROC ANALYSIS of test variables against the positive value of a state variable.
#[derive(Clone, PartialEq, Debug)]
pub struct RocAnalysis {
    pub tests: Vec<String>,
    pub state: String,
    pub value: String,
    /// Compare the areas of the test variables measured on the same cases.
    pub pair: bool,
    /// Compare two groups of cases, made by this variable; empty for no groups.
    pub group: String,
    /// Index into `ROC_SELECTS`, with the two values or the value it needs.
    pub select: usize,
    pub first: String,
    pub second: String,
    pub cut: String,
    /// Index into `ROC_CURVES`, with the reference lines of the curves.
    pub curve: usize,
    pub reference: bool,
    pub pr_reference: bool,
    /// Fill the precision-recall curve in along the false positives, not the true ones.
    pub interpolate_fp: bool,
    pub model_quality: bool,
    pub se: bool,
    pub classifier: bool,
    pub coordinates_roc: bool,
    pub coordinates_pr: bool,
    /// Smaller test values point to the positive state.
    pub small_positive: bool,
    /// A value equal to the cutoff counts as negative.
    pub exclude_cutoff: bool,
    pub negexpo: bool,
    pub level: String,
    pub include: bool,
}

impl Default for RocAnalysis {
    fn default() -> Self {
        Self {
            tests: Vec::new(),
            state: String::new(),
            value: String::new(),
            pair: false,
            group: String::new(),
            select: 0,
            first: String::new(),
            second: String::new(),
            cut: String::new(),
            curve: 0,
            reference: false,
            pr_reference: false,
            interpolate_fp: false,
            model_quality: false,
            se: false,
            classifier: false,
            coordinates_roc: false,
            coordinates_pr: false,
            small_positive: false,
            exclude_cutoff: false,
            negexpo: false,
            level: "95".into(),
            include: false,
        }
    }
}

impl RocAnalysis {
    pub fn grouped(&self) -> bool {
        !self.group.trim().is_empty()
    }

    /// Whether the ROC curve and the precision-recall curve are drawn.
    pub fn draws(&self) -> (bool, bool) {
        (
            self.curve == 0 || self.curve == 2,
            self.curve == 1 || self.curve == 2,
        )
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        numeric(data, &self.tests)?;
        let tests = names(&self.tests, "Test variables")?;
        let string = |name: &str| data.variable(name.trim()).is_ok_and(|v| v.is_string());
        let state = self.state.trim();
        if state.is_empty() {
            return Err("set the state variable".into());
        }
        if self.value.trim().is_empty() {
            return Err("give the value of the positive state".into());
        }
        // string values are compared without their trailing blanks only
        let given = match string(state) {
            true => self.value.trim_end(),
            false => self.value.trim(),
        };
        let positive = value(given, string(state))?;
        let mut text = format!("ROC ANALYSIS {tests} BY {state} ({positive})");
        let grouped = self.grouped();
        // groups take the place of the paired design
        let paired = self.pair && !grouped;
        if paired && self.tests.len() < 2 {
            return Err(
                "comparing areas on the same cases needs at least two test variables".into(),
            );
        }
        if grouped {
            let group = self.group.trim();
            let by = string(group);
            let select = match self.select {
                LEVEL => {
                    let (a, b) = (self.first.trim(), self.second.trim());
                    if a.is_empty() || b.is_empty() {
                        return Err("give the two values of the grouping variable".into());
                    }
                    if a == b {
                        return Err("the two group values must differ".into());
                    }
                    format!("LEVEL({} {})", value(a, by)?, value(b, by)?)
                }
                i if by => {
                    return Err(format!(
                        "{group} is a string; {} needs a numeric grouping variable",
                        ROC_SELECTS[i].0
                    ));
                }
                CUTPOINT => format!("CUTPOINT({})", number(parse(&self.cut, "the cut point")?)),
                _ => "MIDPOINT".into(),
            };
            text.push_str(&format!("\n  /DESIGN GROUP={group} SELECT={select}"));
        } else if paired {
            text.push_str("\n  /DESIGN PAIR=TRUE");
        }
        let mut criteria = Vec::new();
        if self.exclude_cutoff {
            criteria.push("CUTOFF=EXCLUDE".to_string());
        }
        if self.small_positive {
            criteria.push("TESTPOS=SMALL".into());
        }
        let level = percent(&self.level)?;
        if level != 95.0 {
            criteria.push(format!("CI={}", number(level)));
        }
        // the paired design takes the nonparametric error only
        if self.negexpo && !paired {
            criteria.push("DISTRIBUTION=NEGEXPO".into());
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        let (roc, pr) = self.draws();
        let mut plot = Vec::new();
        if self.curve == NO_CURVES {
            plot.push("CURVE=NONE".to_string());
        } else if self.curve != 0 || self.reference {
            let mut curves = Vec::new();
            if roc {
                curves.push(if self.reference {
                    "ROC(REFERENCE)"
                } else {
                    "ROC"
                });
            }
            if pr {
                curves.push(if self.pr_reference {
                    "PR(REFERENCE)"
                } else {
                    "PR"
                });
            }
            plot.push(format!("CURVE={}", curves.join(" ")));
        }
        if pr && self.interpolate_fp {
            plot.push("PR_INTERPOLATE=FP".into());
        }
        if self.model_quality {
            plot.push("MODELQUALITY=TRUE".into());
        }
        if !plot.is_empty() {
            text.push_str(&format!("\n  /PLOT {}", plot.join(" ")));
        }
        let mut print = Vec::new();
        if self.se {
            print.push("SE=TRUE".to_string());
        }
        if self.classifier {
            print.push("CLASSIFIER=TRUE".into());
        }
        let coordinates: Vec<&str> = [("ROC", self.coordinates_roc), ("PR", self.coordinates_pr)]
            .iter()
            .filter(|(_, on)| *on)
            .map(|(k, _)| *k)
            .collect();
        if !coordinates.is_empty() {
            print.push(format!("COORDINATES={}", coordinates.join(" ")));
        }
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT {}", print.join(" ")));
        }
        if self.include {
            text.push_str("\n  /MISSING USERMISSING=INCLUDE");
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, v};

    fn data() -> Dataset {
        open("survey_basic.sav").dataset().unwrap().clone()
    }

    fn base() -> RocAnalysis {
        RocAnalysis {
            tests: v(&["age", "monthly_spend"]),
            state: "q5_1".into(),
            value: "1".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_least_and_the_most_the_dialog_writes() {
        let data = data();
        assert_eq!(
            base().syntax(&data).unwrap(),
            "ROC ANALYSIS age monthly_spend BY q5_1 (1)."
        );
        let d = RocAnalysis {
            pair: true,
            curve: 2,
            reference: true,
            pr_reference: true,
            interpolate_fp: true,
            model_quality: true,
            se: true,
            classifier: true,
            coordinates_roc: true,
            coordinates_pr: true,
            small_positive: true,
            exclude_cutoff: true,
            negexpo: true,
            level: "90".into(),
            include: true,
            ..base()
        };
        // the nonparametric error is the only one of the paired design
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ROC ANALYSIS age monthly_spend BY q5_1 (1)\n  /DESIGN PAIR=TRUE\n  \
             /CRITERIA CUTOFF=EXCLUDE TESTPOS=SMALL CI=90\n  \
             /PLOT CURVE=ROC(REFERENCE) PR(REFERENCE) PR_INTERPOLATE=FP MODELQUALITY=TRUE\n  \
             /PRINT SE=TRUE CLASSIFIER=TRUE COORDINATES=ROC PR\n  /MISSING USERMISSING=INCLUDE."
        );
    }

    #[test]
    fn groups_need_their_values_and_curves_follow_the_choice() {
        let data = data();
        let d = RocAnalysis {
            group: "plan".into(),
            first: "BASIC".into(),
            second: "PRO".into(),
            curve: 1,
            ..base()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ROC ANALYSIS age monthly_spend BY q5_1 (1)\n  \
             /DESIGN GROUP=plan SELECT=LEVEL('BASIC' 'PRO')\n  /PLOT CURVE=PR."
        );
        let cut = RocAnalysis {
            group: "gender".into(),
            select: 2,
            cut: "1.5".into(),
            curve: 3,
            ..base()
        };
        assert_eq!(
            cut.syntax(&data).unwrap(),
            "ROC ANALYSIS age monthly_spend BY q5_1 (1)\n  \
             /DESIGN GROUP=gender SELECT=CUTPOINT(1.5)\n  /PLOT CURVE=NONE."
        );
        let midpoint = RocAnalysis {
            select: 1,
            ..d.clone()
        };
        assert!(
            midpoint
                .syntax(&data)
                .unwrap_err()
                .contains("numeric grouping")
        );
        let no_second = RocAnalysis {
            second: String::new(),
            ..d
        };
        assert!(no_second.syntax(&data).is_err());
        let one = RocAnalysis {
            tests: v(&["age"]),
            pair: true,
            ..base()
        };
        assert!(one.syntax(&data).unwrap_err().contains("at least two"));
    }
}
