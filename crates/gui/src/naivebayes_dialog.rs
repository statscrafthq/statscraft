//! The Naive Bayes (NAIVEBAYES) dialog. Options left as they are add nothing to the syntax, so
//! the command runs with its own defaults.

use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, percent};
use crate::syntax_text::quote;
use statscraft_data::{Dataset, is_valid_name};

/// How the predictors are chosen.
pub const NB_SELECTION: [&str; 4] = [
    "Choose automatically",
    "Add predictors up to a largest number",
    "Add exactly this many predictors",
    "Use every predictor",
];

/// What picks the best of the subsets passed; the first is the command's own rule.
pub const NB_BEST: [&str; 3] = [
    "The command's choice (test cases when the data are split)",
    "Lowest pseudo-BIC",
    "Best fit to the test cases",
];

pub const NB_PARTITIONS: [&str; 3] = [
    "Use all cases to train",
    "Draw the training cases at random",
    "Training cases are those where a variable is 1",
];

pub const NB_PRINT: [(&str, &str); 5] = [
    ("CPS", "Case processing summary"),
    ("EXCLUDED", "Excluded predictors"),
    ("SUMMARY", "Selection summary"),
    ("SELECTED", "Selected predictors"),
    ("CLASSIFICATION", "Classification table"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct NaiveBayes {
    pub target: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `NB_SELECTION`.
    pub selection: usize,
    /// The largest or exact number of predictors.
    pub size: String,
    /// Index into `NB_BEST`.
    pub best: usize,
    /// Intervals for a scale predictor; empty: 10.
    pub bins: String,
    /// Index into `NB_PARTITIONS`.
    pub partition: usize,
    pub percent: String,
    pub partition_variable: String,
    pub include_missing: bool,
    pub print: [bool; 5],
    pub save_value: bool,
    pub value_name: String,
    pub save_probability: bool,
    pub probability_root: String,
    /// How many categories get a probability variable; empty: the command's own.
    pub probability_count: String,
    pub model_file: String,
}

impl Default for NaiveBayes {
    fn default() -> Self {
        Self {
            target: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            selection: 0,
            size: String::new(),
            best: 0,
            bins: String::new(),
            partition: 0,
            percent: "70".into(),
            partition_variable: String::new(),
            include_missing: false,
            print: [true; 5],
            save_value: false,
            value_name: String::new(),
            save_probability: false,
            probability_root: String::new(),
            probability_count: String::new(),
            model_file: String::new(),
        }
    }
}

/// A name for a new variable, or None for an empty field.
fn new_variable(text: &str, what: &str) -> Result<Option<String>, String> {
    let t = text.trim();
    match (t.is_empty(), is_valid_name(t)) {
        (true, _) => Ok(None),
        (false, true) => Ok(Some(t.to_string())),
        _ => Err(format!("{what} is not a valid variable name")),
    }
}

impl NaiveBayes {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err("choose the target variable".into());
        }
        numeric(data, &self.covariates)?;
        let mut text = format!("NAIVEBAYES {target}");
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        match self.partition {
            1 => {
                let p = percent(&self.percent)
                    .map_err(|_| "the training percent must be above 0 and below 100")?;
                text.push_str(&format!("\n  /TRAININGSAMPLE PERCENT={p}"));
            }
            2 => {
                let v = self.partition_variable.trim();
                if v.is_empty() {
                    return Err("choose the partition variable".into());
                }
                numeric(data, [&self.partition_variable])?;
                text.push_str(&format!("\n  /TRAININGSAMPLE VARIABLE={v}"));
            }
            _ => {}
        }
        let best = match self.best {
            1 => " (BESTSUBSET=PSEUDOBIC)",
            2 if self.partition == 0 => {
                return Err("the test cases come from a training partition; choose one".into());
            }
            2 => " (BESTSUBSET=TESTDATA)",
            _ => "",
        };
        let size = whole(&self.size, 1, "the number of predictors")?;
        match (self.selection, size) {
            (1 | 2, None) => return Err("give the number of predictors".into()),
            (1, Some(n)) => text.push_str(&format!("\n  /SUBSET MAXSIZE={n}{best}")),
            (2, Some(n)) => text.push_str(&format!("\n  /SUBSET EXACTSIZE={n}")),
            (3, _) => text.push_str("\n  /SUBSET NOSELECTION"),
            _ if !best.is_empty() => text.push_str(&format!("\n  /SUBSET MAXSIZE=AUTO{best}")),
            _ => {}
        }
        if let Some(bins) = whole(&self.bins, 2, "the number of intervals")? {
            text.push_str(&format!("\n  /CRITERIA BINS={bins}"));
        }
        if self.include_missing {
            text.push_str("\n  /MISSING USERMISSING=INCLUDE");
        }
        // the keywords named replace the defaults, which are all five on
        if self.print != [true; 5] {
            let named: Vec<&str> = NB_PRINT
                .iter()
                .zip(self.print)
                .filter_map(|((k, _), on)| on.then_some(*k))
                .collect();
            match named.is_empty() {
                true => text.push_str("\n  /PRINT NONE"),
                false => text.push_str(&format!("\n  /PRINT {}", named.join(" "))),
            }
        }
        let mut saves = Vec::new();
        if self.save_value {
            saves.push(
                match new_variable(&self.value_name, "the predicted value name")? {
                    Some(n) => format!("PREDVAL({n})"),
                    None => "PREDVAL".into(),
                },
            );
        }
        if self.save_probability {
            let root = new_variable(&self.probability_root, "the probability root")?;
            let count = whole(&self.probability_count, 1, "the number of probabilities")?;
            saves.push(match (root, count) {
                (None, None) => "PREDPROB".into(),
                (root, None) => format!("PREDPROB({})", root.unwrap_or_default()),
                (root, Some(n)) => format!("PREDPROB({}:{n})", root.unwrap_or_default()),
            });
        }
        if !saves.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", saves.join(" ")));
        }
        let file = self.model_file.trim();
        if !file.is_empty() {
            text.push_str(&format!("\n  /OUTFILE MODEL={}", quote(file)));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::v;

    fn base() -> NaiveBayes {
        NaiveBayes {
            target: "churn".into(),
            factors: v(&["region", "plan"]),
            covariates: v(&["tenure"]),
            ..Default::default()
        }
    }

    #[test]
    fn defaults_add_nothing() {
        assert_eq!(
            base().syntax(&Dataset::new(0)).unwrap(),
            "NAIVEBAYES churn BY region plan WITH tenure."
        );
    }

    #[test]
    fn every_option_is_written() {
        let d = NaiveBayes {
            selection: 1,
            size: "3".into(),
            best: 2,
            bins: "5".into(),
            partition: 1,
            percent: "70".into(),
            include_missing: true,
            print: [true, false, true, false, false],
            save_value: true,
            save_probability: true,
            probability_root: "prob".into(),
            probability_count: "2".into(),
            model_file: "nb.json".into(),
            ..base()
        };
        assert_eq!(
            d.syntax(&Dataset::new(0)).unwrap(),
            "NAIVEBAYES churn BY region plan WITH tenure\n  /TRAININGSAMPLE PERCENT=70\n  \
             /SUBSET MAXSIZE=3 (BESTSUBSET=TESTDATA)\n  /CRITERIA BINS=5\n  \
             /MISSING USERMISSING=INCLUDE\n  /PRINT CPS SUMMARY\n  \
             /SAVE PREDVAL PREDPROB(prob:2)\n  /OUTFILE MODEL='nb.json'."
        );
    }

    #[test]
    fn mistakes_are_named() {
        let data = Dataset::new(0);
        let err = |change: &dyn Fn(&mut NaiveBayes)| {
            let mut d = base();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.target.clear()), "choose the target variable");
        assert_eq!(err(&|d| d.selection = 2), "give the number of predictors");
        assert_eq!(
            err(&|d| d.best = 2),
            "the test cases come from a training partition; choose one"
        );
        assert!(err(&|d| d.bins = "1".into()).contains("at least 2"));
        assert!(
            err(&|d| {
                d.partition = 1;
                d.percent = "100".into();
            })
            .contains("above 0")
        );
    }
}
