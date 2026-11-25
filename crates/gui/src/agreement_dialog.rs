//! The Weighted Kappa (WEIGHTED KAPPA), Fleiss Multirater Kappa (FLEISS MULTIRATER KAPPA) and
//! Ratio Statistics (RATIO STATISTICS) dialogs. Options left as they are add nothing to the
//! syntax, so the command runs with its own defaults.

use crate::advanced_dialogs::ci_level;
use crate::analyze_dialogs::keywords;
use crate::i18n::{t, t_args};
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{names, number, quote};
use statscraft_data::Dataset;

/// `ASYMPTOTICCILEVEL=n` when the level is not the default 95.
fn level_criterion(text: &str) -> Result<Option<String>, String> {
    let level = ci_level(text)?;
    Ok((level != 95.0).then(|| format!("ASYMPTOTICCILEVEL={}", number(level))))
}

/// WEIGHTED KAPPA: Cohen's kappa with agreement weights for pairs of raters.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct WeightedKappa {
    /// Every pair of these, or with `by` each of these with each of `columns`.
    pub raters: Vec<String>,
    pub by: bool,
    pub columns: Vec<String>,
    pub quadratic: bool,
    /// CLASSMISSING=INCLUDE: user-missing values are categories.
    pub include: bool,
    /// TABULATION(ALL): a table of counts for each pair.
    pub tabulate: bool,
    /// ORDER(DESCENDING) of the categories in those tables.
    pub descending: bool,
    /// Empty for 95.
    pub ci_level: String,
}

impl WeightedKappa {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = String::from("WEIGHTED KAPPA ");
        if self.by {
            let rows = names(&self.raters, t("stats-agree-row-raters"))?;
            let columns = names(&self.columns, t("stats-agree-column-raters"))?;
            if self.raters.len() == 1 && self.columns == self.raters {
                return Err(t("stats-agree-lists-pair-nothing").into());
            }
            text.push_str(&format!("{rows} BY {columns}"));
        } else {
            if self.raters.len() < 2 {
                return Err(t("stats-agree-two-raters").into());
            }
            text.push_str(&self.raters.join(" "));
        }
        if self.include {
            text.push_str("\n  /MISSING CLASSMISSING=INCLUDE");
        }
        let mut criteria: Vec<String> = level_criterion(&self.ci_level)?.into_iter().collect();
        if self.quadratic {
            criteria.push("WEIGHTING=QUADRATIC".into());
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        if self.tabulate {
            text.push_str("\n  /PRINT TABULATION(ALL)");
            if self.descending {
                text.push_str(" ORDER(DESCENDING)");
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// FLEISS MULTIRATER KAPPA: the agreement of several raters, one variable each.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FleissKappa {
    pub raters: Vec<String>,
    /// CLASSMISSING=INCLUDE: user-missing values are categories.
    pub include: bool,
    /// PRINT CATEGORY: a row per category.
    pub category: bool,
    /// Empty for 95.
    pub ci_level: String,
}

impl FleissKappa {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.raters.len() < 2 {
            return Err(t("stats-agree-two-raters").into());
        }
        let strings = self
            .raters
            .iter()
            .filter(|n| data.variable(n).is_ok_and(|v| v.is_string()))
            .count();
        if strings > 0 && strings < self.raters.len() {
            return Err(t("stats-agree-raters-one-type").into());
        }
        let mut text = format!("FLEISS MULTIRATER KAPPA {}", self.raters.join(" "));
        if self.include {
            text.push_str("\n  /MISSING CLASSMISSING=INCLUDE");
        }
        if let Some(level) = level_criterion(&self.ci_level)? {
            text.push_str(&format!("\n  /CRITERIA {level}"));
        }
        if self.category {
            text.push_str("\n  /PRINT CATEGORY");
        }
        text.push('.');
        Ok(text)
    }
}

/// The statistics of the ratio that take no setting, in the order they are written.
pub const RATIO_STATISTICS: [(&str, &str); 12] = [
    ("MEDIAN", "stats-agree-median"),
    ("MEAN", "stats-agree-mean"),
    ("WGTMEAN", "stats-agree-weighted-mean"),
    ("MIN", "stats-agree-minimum"),
    ("MAX", "stats-agree-maximum"),
    ("RANGE", "stats-agree-range"),
    ("STDDEV", "stats-agree-standard-deviation"),
    ("AAD", "stats-agree-average-absolute-deviation"),
    ("COD", "stats-agree-cod"),
    ("PRD", "stats-agree-prd"),
    ("MNCOV", "stats-agree-cov-mean"),
    ("MDCOV", "stats-agree-cov-median"),
];

/// How the groups are ordered; the first writes nothing.
pub const RATIO_ORDERS: [(&str, &str); 3] = [
    ("ASCENDING", "stats-agree-ascending"),
    ("DESCENDING", "stats-agree-descending"),
    ("NOSORT", "stats-agree-as-they-appear"),
];

/// Where the statistics are saved; the command cannot replace the active dataset.
pub const RATIO_SAVE: [&str; 3] = [
    "stats-agree-do-not-save",
    "stats-agree-a-data-file",
    "stats-agree-a-new-dataset",
];

/// RATIO STATISTICS: the ratio of two variables, overall and by group.
#[derive(Clone, PartialEq, Debug)]
pub struct Ratio {
    pub numerator: String,
    pub denominator: String,
    pub group: String,
    /// Index into `RATIO_ORDERS`.
    pub order: usize,
    /// One flag per entry of `RATIO_STATISTICS`.
    pub statistics: [bool; 12],
    /// CIN: confidence limits around the mean, the weighted mean and the median.
    pub cin: bool,
    /// Empty for 95.
    pub cin_level: String,
    /// BCOC: the share of ratios in each interval.
    pub bcoc: bool,
    /// Pairs of low and high, separated by semicolons.
    pub intervals: String,
    /// WCOC: the share of ratios within these percents of the median.
    pub wcoc: bool,
    pub percents: String,
    pub include: bool,
    /// PRINT: the statistics show in a table.
    pub show: bool,
    /// Index into `RATIO_SAVE`.
    pub save: usize,
    pub file: String,
    pub dataset: String,
}

impl Default for Ratio {
    fn default() -> Self {
        Self {
            numerator: String::new(),
            denominator: String::new(),
            group: String::new(),
            order: 0,
            statistics: [false; 12],
            cin: false,
            cin_level: String::new(),
            bcoc: false,
            intervals: String::new(),
            wcoc: false,
            percents: String::new(),
            include: false,
            show: true,
            save: 0,
            file: String::new(),
            dataset: String::new(),
        }
    }
}

/// The numbers of a field, split at spaces and commas.
fn numbers(text: &str, what: &str) -> Result<Vec<f64>, String> {
    text.split([' ', ','])
        .filter(|t| !t.is_empty())
        .map(|t| parse(t, what))
        .collect()
}

impl Ratio {
    /// The statistics as PRINT and OUTFILE write them.
    fn request(&self) -> Result<String, String> {
        let mut words: Vec<String> = keywords(&RATIO_STATISTICS, &self.statistics)
            .into_iter()
            .map(String::from)
            .collect();
        if self.cin {
            match ci_level(&self.cin_level)? {
                95.0 => words.push("CIN".into()),
                level => words.push(format!("CIN({})", number(level))),
            }
        }
        if self.bcoc {
            let mut pairs = Vec::new();
            for pair in self.intervals.split(';').filter(|p| !p.trim().is_empty()) {
                let &[low, high] = numbers(pair, t("stats-agree-what-interval-end"))?.as_slice()
                else {
                    return Err(t_args(
                        "stats-agree-not-low-and-high",
                        &[("pair", format!("{:?}", pair.trim()).into())],
                    ));
                };
                if low > high {
                    return Err(t_args(
                        "stats-agree-interval-backwards",
                        &[
                            ("low", low.to_string().into()),
                            ("high", high.to_string().into()),
                        ],
                    ));
                }
                pairs.push(format!("({},{})", number(low), number(high)));
            }
            if pairs.is_empty() {
                return Err(t("stats-agree-type-an-interval").into());
            }
            words.push(format!("BCOC({})", pairs.join(" ")));
        }
        if self.wcoc {
            let list = numbers(&self.percents, t("stats-agree-what-percent-of-median"))?;
            if list.is_empty() {
                return Err(t("stats-agree-type-a-percent").into());
            }
            if list.iter().any(|p| *p <= 0.0 || *p >= 100.0) {
                return Err(t("stats-agree-percents-between").into());
            }
            let list: Vec<String> = list.into_iter().map(number).collect();
            words.push(format!("WCOC({})", list.join(" ")));
        }
        if words.is_empty() {
            return Err(t("stats-agree-choose-a-statistic").into());
        }
        Ok(words.join(" "))
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let (numerator, denominator) = (self.numerator.trim(), self.denominator.trim());
        if numerator.is_empty() || denominator.is_empty() {
            return Err(t("stats-agree-set-numerator-denominator").into());
        }
        if numerator == denominator {
            return Err(t("stats-agree-numerator-denominator-differ").into());
        }
        numeric(data, [&self.numerator, &self.denominator])?;
        let request = self.request()?;
        let target = output_target(
            self.save,
            &self.file,
            &self.dataset,
            t("stats-agree-what-statistics"),
        )?;
        if !self.show && target.is_none() {
            return Err(t("stats-agree-show-or-save").into());
        }
        let mut text = String::new();
        // the command saves into a dataset only when one of that name exists
        if self.save == 2 {
            text.push_str(&format!("DATASET DECLARE {}.\n", self.dataset.trim()));
        }
        text.push_str(&format!("RATIO STATISTICS {numerator} WITH {denominator}"));
        let group = self.group.trim();
        if !group.is_empty() {
            text.push_str(&format!(" BY {group}"));
            if self.order > 0 {
                text.push_str(&format!(" ({})", RATIO_ORDERS[self.order].0));
            }
        }
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        if self.show {
            text.push_str(&format!("\n  /PRINT={request}"));
        }
        if let Some(target) = target {
            // a file comes back quoted, a dataset name bare; the command takes both quoted
            let target = if self.save == 2 {
                quote(&target)
            } else {
                target
            };
            text.push_str(&format!("\n  /OUTFILE({target})={request}"));
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
    fn weighted_kappa_writes_only_what_changed() {
        let mut d = WeightedKappa {
            raters: v(&["q5_1", "q5_2"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "WEIGHTED KAPPA q5_1 q5_2.");
        // the order of the categories counts only with the tables
        d.descending = true;
        d.ci_level = "95".into();
        assert_eq!(d.syntax().unwrap(), "WEIGHTED KAPPA q5_1 q5_2.");
        d.by = true;
        d.columns = v(&["q5_3"]);
        d.quadratic = true;
        d.include = true;
        d.tabulate = true;
        d.ci_level = "90".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "WEIGHTED KAPPA q5_1 q5_2 BY q5_3\n  /MISSING CLASSMISSING=INCLUDE\n  \
             /CRITERIA ASYMPTOTICCILEVEL=90 WEIGHTING=QUADRATIC\n  \
             /PRINT TABULATION(ALL) ORDER(DESCENDING)."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 1);
    }

    #[test]
    fn weighted_kappa_checks_what_the_command_would_reject() {
        let e = |d: WeightedKappa| d.syntax().unwrap_err();
        let one = WeightedKappa {
            raters: v(&["q5_1"]),
            ..Default::default()
        };
        assert_eq!(e(one.clone()), "add at least two variables to Raters");
        let by = WeightedKappa {
            by: true,
            ..one.clone()
        };
        assert_eq!(e(by.clone()), "add at least one variable to Column raters");
        assert_eq!(
            e(WeightedKappa {
                columns: v(&["q5_1"]),
                ..by.clone()
            }),
            "the two lists pair no two different variables"
        );
        assert_eq!(
            e(WeightedKappa {
                raters: v(&["q5_1", "q5_2"]),
                ci_level: "100".into(),
                ..Default::default()
            }),
            "the confidence level must be a percentage between 0 and 100"
        );
    }

    #[test]
    fn fleiss_kappa_checks_the_raters() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let mut d = FleissKappa {
            raters: v(&["q5_1"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "add at least two variables to Raters"
        );
        d.raters.push("plan".into());
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "the raters must be all numeric or all string"
        );
        d.raters = v(&["q5_1", "q5_2", "q5_3"]);
        assert_eq!(
            d.syntax(&data).unwrap(),
            "FLEISS MULTIRATER KAPPA q5_1 q5_2 q5_3."
        );
        d.ci_level = "0".into();
        assert!(d.syntax(&data).is_err());
    }

    #[test]
    fn ratio_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let base = Ratio {
            numerator: "monthly_spend".into(),
            denominator: "age".into(),
            statistics: [true; 12],
            ..Default::default()
        };
        let e = |change: &dyn Fn(&mut Ratio)| {
            let mut d = base.clone();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            e(&|d| d.denominator.clear()),
            "set the numerator and the denominator"
        );
        assert_eq!(
            e(&|d| d.denominator = "monthly_spend".into()),
            "the numerator and the denominator must differ"
        );
        assert_eq!(
            e(&|d| d.numerator = "plan".into()),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            e(&|d| d.statistics = [false; 12]),
            "choose at least one statistic"
        );
        assert_eq!(
            e(&|d| {
                d.bcoc = true;
                d.intervals = "0.5 0.9; 2".into();
            }),
            "\"2\" is not a low and a high value"
        );
        assert_eq!(
            e(&|d| {
                d.bcoc = true;
                d.intervals = "3 1".into();
            }),
            "the interval 3 to 1 runs backwards"
        );
        assert_eq!(
            e(&|d| d.bcoc = true),
            "type at least one interval of ratios"
        );
        assert_eq!(
            e(&|d| {
                d.wcoc = true;
                d.percents = "10 100".into();
            }),
            "percents of the median must lie between 0 and 100"
        );
        assert_eq!(
            e(&|d| {
                d.wcoc = true;
                d.percents = "ten".into();
            }),
            "each percent of the median must be a number"
        );
        assert_eq!(e(&|d| d.show = false), "show the statistics or save them");
        assert_eq!(e(&|d| d.save = 1), "choose the file for the statistics");
        assert_eq!(e(&|d| d.save = 2), "name the new dataset");
        // the settings of a statistic left off are not checked
        let mut d = base.clone();
        d.intervals = "x".into();
        d.percents = "x".into();
        d.cin_level = "x".into();
        d.file = "".into();
        assert!(d.syntax(&data).is_ok());
    }

    #[test]
    fn ratio_writes_the_statistics_to_a_file() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let path =
            std::env::temp_dir().join(format!("statscraft-ratio-{}.sav", std::process::id()));
        let d = Ratio {
            numerator: "monthly_spend".into(),
            denominator: "age".into(),
            group: "region".into(),
            order: 2,
            statistics: [
                true, false, false, false, false, false, false, false, false, false, false, false,
            ],
            cin: true,
            cin_level: "90".into(),
            show: false,
            save: 1,
            file: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            format!(
                "RATIO STATISTICS monthly_spend WITH age BY region (NOSORT)\n  \
                 /OUTFILE('{}')=MEDIAN CIN(90).",
                path.display()
            )
        );
        run_in(&mut session, &syntax);
        assert!(path.exists());
        std::fs::remove_file(path).unwrap();
    }
}
