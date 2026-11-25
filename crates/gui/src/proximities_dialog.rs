//! The Distances dialog (PROXIMITIES). Options left as they are add nothing to the syntax, so
//! the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::i18n::{t, t_args};
use crate::model_dialogs::ClusterMeasure;
use crate::stats_dialogs::numeric;
use crate::syntax_text::{names, quote};
use statscraft_data::{Dataset, is_valid_name};

/// How /STANDARDIZE rescales the values; the first writes no subcommand.
pub const PROXIMITIES_STANDARDIZE: [(&str, &str); 7] = [
    ("", "stats-prox-none"),
    ("Z", "stats-prox-z-scores"),
    ("RANGE", "stats-prox-range-of-1"),
    ("RESCALE", "stats-prox-from-0-to-1"),
    ("MAX", "stats-prox-largest-value-1"),
    ("MEAN", "stats-prox-mean-of-1"),
    ("SD", "stats-prox-sd-of-1"),
];

/// What /MEASURE does to the computed values, in the order the command applies them.
pub const PROXIMITIES_TRANSFORMS: [(&str, &str); 3] = [
    ("ABSOLUTE", "stats-prox-absolute-values"),
    ("REVERSE", "stats-prox-reverse-sign"),
    ("RESCALE", "stats-prox-rescale-0-1"),
];

/// Where /MATRIX=OUT writes the matrix; the first writes none.
pub const PROXIMITIES_MATRIX: [&str; 4] = [
    "stats-prox-matrix-none",
    "stats-prox-a-data-file",
    "stats-prox-a-new-dataset",
    "stats-prox-replace-active-dataset",
];

/// PROXIMITIES: the distances or similarities between cases or between variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Distances {
    pub variables: Vec<String>,
    /// Labels the cases; not written between variables.
    pub id: String,
    pub between_variables: bool,
    pub measure: ClusterMeasure,
    /// Index into `PROXIMITIES_STANDARDIZE`; not written with a binary measure.
    pub standardize: usize,
    pub by_case: bool,
    /// One flag per entry of `PROXIMITIES_TRANSFORMS`.
    pub transforms: [bool; 3],
    pub print: bool,
    pub include: bool,
    /// Index into `PROXIMITIES_MATRIX`.
    pub matrix: usize,
    pub file: String,
    pub dataset: String,
}

impl Default for Distances {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            id: String::new(),
            between_variables: false,
            // Euclidean distance, the command's own default
            measure: ClusterMeasure {
                interval: 1,
                ..Default::default()
            },
            standardize: 0,
            by_case: false,
            transforms: [false; 3],
            print: true,
            include: false,
            matrix: 0,
            file: String::new(),
            dataset: String::new(),
        }
    }
}

/// Where a command writes the data it makes, as the syntax names it: nowhere (0), a data file
/// (1), a new dataset (2), which the caller declares first, or the active dataset (3).
pub(crate) fn output_target(
    choice: usize,
    file: &str,
    dataset: &str,
    what: &str,
) -> Result<Option<String>, String> {
    Ok(match choice {
        0 => None,
        1 => match file.trim() {
            "" => {
                return Err(t_args(
                    "stats-prox-choose-file-for",
                    &[("what", what.into())],
                ));
            }
            path => Some(quote(path)),
        },
        2 => match dataset.trim() {
            "" => return Err(t("stats-prox-name-new-dataset").into()),
            name if !is_valid_name(name) || name.starts_with('#') => {
                return Err(t_args(
                    "stats-prox-cannot-name-dataset",
                    &[("name", name.into())],
                ));
            }
            name => Some(name.to_string()),
        },
        _ => Some("*".into()),
    })
}

impl Distances {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let list = names(&self.variables, t("stats-prox-variables"))?;
        numeric(data, &self.variables)?;
        let measure = self.measure.syntax()?;
        let target = output_target(
            self.matrix,
            &self.file,
            &self.dataset,
            t("stats-prox-what-matrix"),
        )?;
        let mut text = String::new();
        // the command writes into a dataset only when one of that name exists
        if self.matrix == 2 {
            text.push_str(&format!("DATASET DECLARE {}.\n", self.dataset.trim()));
        }
        text.push_str(&format!("PROXIMITIES {list}"));
        if self.between_variables {
            text.push_str("\n  /VIEW=VARIABLE");
        }
        let method = PROXIMITIES_STANDARDIZE[self.standardize].0;
        // the command rejects standardizing for a binary measure; the dialog greys it out
        if !method.is_empty() && self.measure.data != 2 {
            let by = if self.by_case { "CASE" } else { "VARIABLE" };
            text.push_str(&format!("\n  /STANDARDIZE={by} {method}"));
        }
        let transforms = keywords(&PROXIMITIES_TRANSFORMS, &self.transforms);
        if measure != "EUCLID" || !transforms.is_empty() {
            let mut parts = vec![measure.as_str()];
            parts.extend(transforms);
            text.push_str(&format!("\n  /MEASURE={}", parts.join(" ")));
        }
        if !self.print {
            text.push_str("\n  /PRINT=NONE");
        }
        let id = self.id.trim();
        if !id.is_empty() && !self.between_variables {
            text.push_str(&format!("\n  /ID={id}"));
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        if let Some(target) = target {
            text.push_str(&format!("\n  /MATRIX=OUT({target})"));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn distances() -> Distances {
        Distances {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        }
    }

    #[test]
    fn distances_check_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut Distances)| {
            let mut d = distances();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.variables.clear()),
            "add at least one variable to Variables"
        );
        assert_eq!(
            err(&|d| d.variables.push("plan".into())),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| {
                d.measure.interval = 6;
                d.measure.power = "0".into();
            }),
            "the power must be above 0"
        );
        assert_eq!(
            err(&|d| {
                d.measure.interval = 7;
                d.measure.root = "-1".into();
            }),
            "the root must be above 0"
        );
        assert_eq!(
            err(&|d| {
                d.measure.data = 2;
                d.measure.absent = "1".into();
            }),
            "the values of presence and absence must differ"
        );
        assert_eq!(err(&|d| d.matrix = 1), "choose the file for the matrix");
        assert_eq!(err(&|d| d.matrix = 2), "name the new dataset");
        assert_eq!(
            err(&|d| {
                d.matrix = 2;
                d.dataset = "2nd".into();
            }),
            "2nd cannot name a dataset"
        );
    }

    #[test]
    fn distances_write_only_what_changed() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let mut d = distances();
        assert_eq!(d.syntax(&data).unwrap(), "PROXIMITIES age monthly_spend.");

        // between variables: standardized within each case, every change of the values, no
        // label, which only cases take
        d.between_variables = true;
        d.id = "resp_id".into();
        d.standardize = 3;
        d.by_case = true;
        d.transforms = [true; 3];
        d.print = false;
        d.include = true;
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "PROXIMITIES age monthly_spend\n  /VIEW=VARIABLE\n  /STANDARDIZE=CASE RESCALE\n  \
             /MEASURE=EUCLID ABSOLUTE REVERSE RESCALE\n  /PRINT=NONE\n  /MISSING=INCLUDE."
        );
        assert!(run_in(&mut session, &syntax) >= 1);

        // a binary measure leaves out the standardizing the command would refuse
        let mut d = distances();
        d.variables = v(&["q5_1", "q5_2", "q5_3"]);
        d.standardize = 1;
        d.measure.data = 2;
        d.measure.binary = 3;
        d.measure.present = "0".into();
        d.measure.absent = "1".into();
        d.id = "resp_id".into();
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "PROXIMITIES q5_1 q5_2 q5_3\n  /MEASURE=JACCARD(0,1)\n  /ID=resp_id."
        );
        assert!(run_in(&mut session, &syntax) >= 2);

        // the matrix goes to a dataset declared first
        let mut d = distances();
        d.matrix = 2;
        d.dataset = "gaps".into();
        d.print = false;
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "DATASET DECLARE gaps.\nPROXIMITIES age monthly_spend\n  /PRINT=NONE\n  \
             /MATRIX=OUT(gaps)."
        );
        run_in(&mut session, &syntax);
        d.matrix = 3;
        assert!(d.syntax(&data).unwrap().ends_with("/MATRIX=OUT(*)."));
        d.matrix = 1;
        d.file = "/tmp/m x.sav".into();
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/MATRIX=OUT('/tmp/m x.sav').")
        );
    }
}
