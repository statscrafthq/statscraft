//! The Missing Value Analysis (MVA) dialog. Options left as they are add nothing to the syntax,
//! so the command runs with its own defaults.

use crate::dialogs::whole;
use crate::mds_dialog::count;
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::parse;
use crate::syntax_text::number;
use statscraft_data::Dataset;

/// Where EM and REGRESSION save the data with the missing values filled in; the first saves
/// nothing.
pub const MVA_OUTPUT: [&str; 3] = ["None", "A data file", "A new dataset"];

/// The distributions EM can assume; the first writes nothing.
pub const EM_DISTRIBUTIONS: [(&str, &str); 3] = [
    ("", "Normal"),
    ("TDF", "Student's t"),
    ("LAMBDA", "Mixed normal"),
];

/// What REGRESSION adds to each estimate; the first writes nothing, so the command picks
/// residuals, or normal draws when under half the cases are complete.
pub const MVA_ADDTYPES: [(&str, &str); 5] = [
    ("", "Automatic"),
    ("RESIDUAL", "Residual of a complete case"),
    ("NORMAL", "Normal draw"),
    ("T", "Student's t draw"),
    ("NONE", "Nothing"),
];

/// Words MVA keeps for itself, so its variable lists cannot hold variables of these names.
const RESERVED: [&str; 5] = ["VARIABLES", "SORT", "NOSORT", "DESCRIBE", "WITH"];

/// EM or REGRESSION: the variables and the settings of one way to estimate.
#[derive(Clone, PartialEq, Debug)]
pub struct Estimation {
    pub on: bool,
    /// The variables to estimate, among the quantitative ones; none means all of them.
    pub predicted: Vec<String>,
    /// The variables to estimate from, among the quantitative ones; none means all of them.
    pub predictors: Vec<String>,
    pub tolerance: String,
    /// EM: an index into `EM_DISTRIBUTIONS`.
    pub distribution: usize,
    /// The degrees of freedom of Student's t, for EM and for REGRESSION's draws.
    pub df: String,
    /// EM's mixed normal: the ratio of the standard deviations and the mixing proportion.
    pub lambda: String,
    pub proportion: String,
    pub iterations: String,
    pub convergence: String,
    /// REGRESSION: an index into `MVA_ADDTYPES`.
    pub addtype: usize,
    /// REGRESSION: empty for no limit.
    pub npredictors: String,
    pub flimit: String,
    /// An index into `MVA_OUTPUT`.
    pub output: usize,
    pub file: String,
    pub dataset: String,
}

impl Default for Estimation {
    fn default() -> Self {
        Self {
            on: false,
            predicted: Vec::new(),
            predictors: Vec::new(),
            tolerance: "0.001".into(),
            distribution: 0,
            df: "5".into(),
            lambda: "3".into(),
            proportion: "0.1".into(),
            iterations: "25".into(),
            convergence: "0.0001".into(),
            addtype: 0,
            npredictors: String::new(),
            flimit: "4".into(),
            output: 0,
            file: String::new(),
            dataset: String::new(),
        }
    }
}

/// A number above 0 and below 1, `default` when the field is empty.
fn unit(text: &str, what: &str, default: f64) -> Result<f64, String> {
    let n = match text.trim() {
        "" => default,
        t => parse(t, what)?,
    };
    match n > 0.0 && n < 1.0 {
        true => Ok(n),
        false => Err(format!("{what} must be above 0 and below 1")),
    }
}

/// A percentage from 0 to 100, `default` when the field is empty.
fn percent(text: &str, what: &str, default: f64) -> Result<f64, String> {
    let n = match text.trim() {
        "" => default,
        t => parse(t, what)?,
    };
    match (0.0..=100.0).contains(&n) {
        true => Ok(n),
        false => Err(format!("{what} must be from 0 to 100")),
    }
}

impl Estimation {
    /// The subcommand, and where it saves the filled-in data. Variables no longer quantitative
    /// are left out of the lists.
    fn syntax(
        &self,
        em: bool,
        quantitative: &[String],
    ) -> Result<(String, Option<String>), String> {
        let (name, what) = match em {
            true => ("EM", "the EM data"),
            false => ("REGRESSION", "the regression data"),
        };
        let kept = |list: &[String]| -> Vec<String> {
            list.iter()
                .filter(|n| quantitative.contains(n))
                .cloned()
                .collect()
        };
        let mut text = format!("\n  /{name}");
        let predicted = kept(&self.predicted);
        if !predicted.is_empty() {
            text.push_str(&format!(" {}", predicted.join(" ")));
        }
        let predictors = kept(&self.predictors);
        if !predictors.is_empty() {
            text.push_str(&format!(" WITH {}", predictors.join(" ")));
        }
        let mut criteria = Vec::new();
        let tolerance = unit(&self.tolerance, "the tolerance", 0.001)?;
        if tolerance != 0.001 {
            criteria.push(format!("TOLERANCE={}", number(tolerance)));
        }
        if em {
            let convergence = unit(&self.convergence, "the convergence", 0.0001)?;
            if convergence != 0.0001 {
                criteria.push(format!("CONVERGENCE={}", number(convergence)));
            }
            let iterations = count(&self.iterations, 1, "the most iterations", 25)?;
            if iterations != 25 {
                criteria.push(format!("ITERATIONS={iterations}"));
            }
            match self.distribution {
                1 => {
                    let df = whole(&self.df, 2, "the degrees of freedom of t")?
                        .ok_or("give the degrees of freedom of t")?;
                    criteria.push(format!("TDF={df}"));
                }
                2 => {
                    let lambda = parse(&self.lambda, "the ratio of standard deviations")?;
                    if lambda <= 0.0 {
                        return Err("the ratio of standard deviations must be above 0".into());
                    }
                    let proportion = match self.proportion.trim() {
                        "" => return Err("give the mixing proportion".into()),
                        t => unit(t, "the mixing proportion", 0.0)?,
                    };
                    criteria.push(format!("LAMBDA={}", number(lambda)));
                    criteria.push(format!("PROPORTION={}", number(proportion)));
                }
                _ => {}
            }
        } else {
            let flimit = match self.flimit.trim() {
                "" => 4.0,
                t => parse(t, "the F to enter")?,
            };
            if flimit < 0.0 {
                return Err("the F to enter cannot be negative".into());
            }
            if flimit != 4.0 {
                criteria.push(format!("FLIMIT={}", number(flimit)));
            }
            if let Some(n) = whole(&self.npredictors, 0, "the most predictors")? {
                criteria.push(format!("NPREDICTORS={n}"));
            }
            match self.addtype {
                0 => {}
                3 => {
                    let df = parse(&self.df, "the degrees of freedom of t")?;
                    if df <= 0.0 {
                        return Err("the degrees of freedom of t must be above 0".into());
                    }
                    criteria.push(format!("ADDTYPE=T({})", number(df)));
                }
                i => criteria.push(format!("ADDTYPE={}", MVA_ADDTYPES[i].0)),
            }
        }
        let target = output_target(self.output, &self.file, &self.dataset, what)?;
        if let Some(target) = &target {
            criteria.push(format!("OUTFILE={target}"));
        }
        if !criteria.is_empty() {
            text.push_str(&format!(" ({})", criteria.join(" ")));
        }
        Ok((text, target))
    }
}

/// MVA: where values are missing, and estimates that allow for it.
#[derive(Clone, PartialEq, Debug)]
pub struct MissingValueAnalysis {
    /// Numeric variables analyzed as measurements.
    pub quantitative: Vec<String>,
    /// Variables analyzed by category; string variables always are.
    pub categorical: Vec<String>,
    pub maxcat: String,
    /// Labels the cases of the pattern tables.
    pub id: String,
    pub univariate: bool,
    pub listwise: bool,
    pub pairwise: bool,
    /// TPATTERN: the patterns tabulated, with the share of cases below which a pattern is left
    /// out, and the variables kept in list order.
    pub tpattern: bool,
    pub tpattern_percent: String,
    pub tpattern_nosort: bool,
    /// MPATTERN: the cases with a missing value.
    pub mpattern: bool,
    pub mpattern_nosort: bool,
    /// DPATTERN: every case, maybe sorted by variables; those in `descending` go downward.
    pub dpattern: bool,
    pub sort: Vec<String>,
    pub descending: Vec<String>,
    /// The variables every pattern table shows beside the patterns.
    pub describe: Vec<String>,
    pub ttest: bool,
    pub ttest_percent: String,
    pub prob: bool,
    pub crosstab: bool,
    pub crosstab_percent: String,
    pub mismatch: bool,
    pub mismatch_percent: String,
    pub mismatch_nosort: bool,
    pub em: Estimation,
    pub regression: Estimation,
}

impl Default for MissingValueAnalysis {
    fn default() -> Self {
        Self {
            quantitative: Vec::new(),
            categorical: Vec::new(),
            maxcat: "25".into(),
            id: String::new(),
            univariate: true,
            listwise: false,
            pairwise: false,
            tpattern: false,
            tpattern_percent: "1".into(),
            tpattern_nosort: false,
            mpattern: false,
            mpattern_nosort: false,
            dpattern: false,
            sort: Vec::new(),
            descending: Vec::new(),
            describe: Vec::new(),
            ttest: false,
            ttest_percent: "5".into(),
            prob: false,
            crosstab: false,
            crosstab_percent: "5".into(),
            mismatch: false,
            mismatch_percent: "5".into(),
            mismatch_nosort: false,
            em: Estimation::default(),
            regression: Estimation::default(),
        }
    }
}

impl MissingValueAnalysis {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let quantitative = &self.quantitative;
        if quantitative.is_empty() && self.categorical.is_empty() {
            return Err("add at least one variable".into());
        }
        if let Some(name) = quantitative.iter().find(|n| self.categorical.contains(n)) {
            return Err(format!("{name} is both quantitative and categorical"));
        }
        let string = |name: &String| data.variable(name).is_ok_and(|v| v.is_string());
        if let Some(name) = quantitative.iter().find(|n| string(n)) {
            return Err(format!(
                "{name} is a string; put it with the categorical variables"
            ));
        }
        let patterns = self.tpattern || self.mpattern || self.dpattern;
        let describe: &[String] = if patterns { &self.describe } else { &[] };
        let sort: &[String] = if self.dpattern { &self.sort } else { &[] };
        let clash = quantitative
            .iter()
            .chain(&self.categorical)
            .chain(describe)
            .chain(sort)
            .find(|n| RESERVED.iter().any(|r| n.eq_ignore_ascii_case(r)));
        if let Some(name) = clash {
            return Err(format!(
                "a variable named {name} cannot be used here, as MVA keeps the word; rename it"
            ));
        }
        let needs_quantitative = [
            (self.ttest, "the t tests"),
            (self.listwise, "listwise estimates"),
            (self.pairwise, "pairwise estimates"),
            (self.em.on, "EM estimates"),
            (self.regression.on, "regression estimates"),
        ];
        if quantitative.is_empty()
            && let Some((_, what)) = needs_quantitative.iter().find(|(on, _)| *on)
        {
            return Err(format!("{what} need a quantitative variable"));
        }
        if self.crosstab && self.categorical.is_empty() {
            return Err("crosstabulations need a categorical variable".into());
        }
        let anything = self.univariate
            || self.ttest
            || self.crosstab
            || self.mismatch
            || patterns
            || needs_quantitative.iter().any(|(on, _)| *on);
        if !anything {
            return Err("choose at least one table or estimate".into());
        }

        let all: Vec<&str> = quantitative
            .iter()
            .chain(&self.categorical)
            .map(String::as_str)
            .collect();
        let mut text = format!("MVA VARIABLES={}", all.join(" "));
        // string variables are categorical without being named
        let numeric: Vec<&str> = self
            .categorical
            .iter()
            .filter(|n| !string(n))
            .map(String::as_str)
            .collect();
        if !numeric.is_empty() {
            text.push_str(&format!("\n  /CATEGORICAL={}", numeric.join(" ")));
        }
        let maxcat = count(&self.maxcat, 1, "the most categories", 25)?;
        if maxcat != 25 {
            text.push_str(&format!("\n  /MAXCAT={maxcat}"));
        }
        let id = self.id.trim();
        if !id.is_empty() {
            text.push_str(&format!("\n  /ID={id}"));
        }
        if !self.univariate {
            text.push_str("\n  /NOUNIVARIATE");
        }
        if self.ttest {
            text.push_str("\n  /TTEST");
            let p = percent(&self.ttest_percent, "the t test percentage", 5.0)?;
            if p != 5.0 {
                text.push_str(&format!(" PERCENT={}", number(p)));
            }
            if self.prob {
                text.push_str(" PROB");
            }
        }
        if self.crosstab {
            text.push_str("\n  /CROSSTAB");
            let p = percent(&self.crosstab_percent, "the crosstab percentage", 5.0)?;
            if p != 5.0 {
                text.push_str(&format!(" PERCENT={}", number(p)));
            }
        }
        if self.mismatch {
            text.push_str("\n  /MISMATCH");
            let p = percent(&self.mismatch_percent, "the mismatch percentage", 5.0)?;
            if p != 5.0 {
                text.push_str(&format!(" PERCENT={}", number(p)));
            }
            if self.mismatch_nosort {
                text.push_str(" NOSORT");
            }
        }
        let describe = match self.describe.is_empty() {
            true => String::new(),
            false => format!(" DESCRIBE={}", self.describe.join(" ")),
        };
        if self.dpattern {
            text.push_str("\n  /DPATTERN");
            if !self.sort.is_empty() {
                let keys: Vec<String> = self
                    .sort
                    .iter()
                    .map(|n| match self.descending.contains(n) {
                        true => format!("{n}(DESCENDING)"),
                        false => n.clone(),
                    })
                    .collect();
                text.push_str(&format!(" SORT={}", keys.join(" ")));
            }
            text.push_str(&describe);
        }
        if self.mpattern {
            text.push_str("\n  /MPATTERN");
            if self.mpattern_nosort {
                text.push_str(" NOSORT");
            }
            text.push_str(&describe);
        }
        if self.tpattern {
            text.push_str("\n  /TPATTERN");
            if self.tpattern_nosort {
                text.push_str(" NOSORT");
            }
            let p = percent(&self.tpattern_percent, "the pattern percentage", 1.0)?;
            if p != 1.0 {
                text.push_str(&format!(" PERCENT={}", number(p)));
            }
            text.push_str(&describe);
        }
        if self.listwise {
            text.push_str("\n  /LISTWISE");
        }
        if self.pairwise {
            text.push_str("\n  /PAIRWISE");
        }
        let mut targets: Vec<(Option<String>, usize)> = Vec::new();
        for (estimation, em) in [(&self.em, true), (&self.regression, false)] {
            if estimation.on {
                let (part, target) = estimation.syntax(em, quantitative)?;
                text.push_str(&part);
                targets.push((target, estimation.output));
            }
        }
        let saved: Vec<&(Option<String>, usize)> =
            targets.iter().filter(|(t, _)| t.is_some()).collect();
        if !saved.is_empty() && data.split_file().is_some() {
            return Err(
                "the filled-in data cannot be saved while the file is split; turn the split off or save nothing"
                    .into(),
            );
        }
        if let [a, b] = saved.as_slice()
            && a.1 == b.1
            // dataset names ignore case, file paths may not
            && match a.1 {
                2 => a.0.as_deref().map(str::to_lowercase) == b.0.as_deref().map(str::to_lowercase),
                _ => a.0 == b.0,
            }
        {
            return Err("EM and regression must save to different places".into());
        }
        let mut declare = String::new();
        for (target, output) in saved {
            // the command writes into a dataset only when one of that name exists
            if *output == 2
                && let Some(name) = target
            {
                declare.push_str(&format!("DATASET DECLARE {name}.\n"));
            }
        }
        text.push('.');
        Ok(declare + &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn mva() -> MissingValueAnalysis {
        MissingValueAnalysis {
            quantitative: v(&["age", "q2_recommend"]),
            ..Default::default()
        }
    }

    #[test]
    fn mva_checks_what_the_command_would_reject() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let err = |data: &Dataset, change: &dyn Fn(&mut MissingValueAnalysis)| {
            let mut d = mva();
            change(&mut d);
            d.syntax(data).unwrap_err()
        };
        let e = |change: &dyn Fn(&mut MissingValueAnalysis)| err(&data, change);
        assert_eq!(e(&|d| d.quantitative.clear()), "add at least one variable");
        assert_eq!(
            e(&|d| d.categorical.push("age".into())),
            "age is both quantitative and categorical"
        );
        assert_eq!(
            e(&|d| d.quantitative.push("plan".into())),
            "plan is a string; put it with the categorical variables"
        );
        assert_eq!(
            e(&|d| {
                d.quantitative.clear();
                d.categorical.push("region".into());
                d.em.on = true;
            }),
            "EM estimates need a quantitative variable"
        );
        assert_eq!(
            e(&|d| d.crosstab = true),
            "crosstabulations need a categorical variable"
        );
        assert_eq!(
            e(&|d| d.univariate = false),
            "choose at least one table or estimate"
        );
        assert_eq!(
            e(&|d| d.maxcat = "0".into()),
            "the most categories must be a whole number of at least 1"
        );
        assert_eq!(
            e(&|d| {
                d.ttest = true;
                d.ttest_percent = "101".into();
            }),
            "the t test percentage must be from 0 to 100"
        );
        assert_eq!(
            e(&|d| {
                d.tpattern = true;
                d.tpattern_percent = "-1".into();
            }),
            "the pattern percentage must be from 0 to 100"
        );
        assert_eq!(
            e(&|d| {
                d.em.on = true;
                d.em.distribution = 1;
                d.em.df = "1".into();
            }),
            "the degrees of freedom of t must be a whole number of at least 2"
        );
        assert_eq!(
            e(&|d| {
                d.em.on = true;
                d.em.distribution = 2;
                d.em.lambda = "0".into();
            }),
            "the ratio of standard deviations must be above 0"
        );
        assert_eq!(
            e(&|d| {
                d.em.on = true;
                d.em.distribution = 2;
                d.em.proportion = "1".into();
            }),
            "the mixing proportion must be above 0 and below 1"
        );
        assert_eq!(
            e(&|d| {
                d.em.on = true;
                d.em.convergence = "1".into();
            }),
            "the convergence must be above 0 and below 1"
        );
        assert_eq!(
            e(&|d| {
                d.regression.on = true;
                d.regression.addtype = 3;
                d.regression.df = "0".into();
            }),
            "the degrees of freedom of t must be above 0"
        );
        assert_eq!(
            e(&|d| {
                d.regression.on = true;
                d.regression.flimit = "-1".into();
            }),
            "the F to enter cannot be negative"
        );
        assert_eq!(
            e(&|d| {
                d.regression.on = true;
                d.regression.output = 1;
            }),
            "choose the file for the regression data"
        );
        assert_eq!(
            e(&|d| {
                for x in [&mut d.em, &mut d.regression] {
                    x.on = true;
                    x.output = 2;
                    x.dataset = "filled".into();
                }
            }),
            "EM and regression must save to different places"
        );
        // file names that differ only in case are different files
        let mut d = mva();
        for (x, name) in [(&mut d.em, "a.sav"), (&mut d.regression, "A.sav")] {
            x.on = true;
            x.output = 1;
            x.file = name.into();
        }
        assert!(d.syntax(&data).is_ok());
        // the settings of a way to estimate that is off are not checked
        let mut d = mva();
        d.em.df = "x".into();
        d.em.distribution = 1;
        assert!(d.syntax(&data).is_ok());

        run_in(&mut session, "SORT CASES BY region.\nSPLIT FILE BY region.");
        let split = session.dataset().unwrap().clone();
        assert_eq!(
            err(&split, &|d| {
                d.em.on = true;
                d.em.output = 2;
                d.em.dataset = "filled".into();
            }),
            "the filled-in data cannot be saved while the file is split; turn the split off or save nothing"
        );
    }

    #[test]
    fn mva_reserved_words_cannot_name_variables_on_its_lists() {
        let mut session = open("survey_basic.sav");
        run_in(&mut session, "COMPUTE sort = age.\nEXECUTE.");
        let data = session.dataset().unwrap().clone();
        let mut d = mva();
        d.dpattern = true;
        d.sort = v(&["sort"]);
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "a variable named sort cannot be used here, as MVA keeps the word; rename it"
        );
        // a sort list of a pattern that is off is not written, so it does not matter
        d.dpattern = false;
        d.mpattern = true;
        assert!(d.syntax(&data).is_ok());
    }

    #[test]
    fn mva_writes_only_what_changed() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let mut d = mva();
        // a way to estimate left off writes nothing, whatever its settings
        d.regression.output = 1;
        assert_eq!(d.syntax(&data).unwrap(), "MVA VARIABLES=age q2_recommend.");

        // a string category is not named on CATEGORICAL; settings at their defaults add nothing
        d.categorical = v(&["region", "plan"]);
        d.ttest = true;
        d.crosstab = true;
        d.mismatch = true;
        d.tpattern = true;
        d.em.on = true;
        d.regression.on = true;
        d.regression.output = 0;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MVA VARIABLES=age q2_recommend region plan\n  /CATEGORICAL=region\n  /TTEST\n  \
             /CROSSTAB\n  /MISMATCH\n  /TPATTERN\n  /EM\n  /REGRESSION."
        );

        // a predicted variable that is no longer quantitative is left out
        let mut d = mva();
        d.em.on = true;
        d.em.predicted = v(&["age", "monthly_spend"]);
        d.em.distribution = 2;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MVA VARIABLES=age q2_recommend\n  /EM age (LAMBDA=3 PROPORTION=0.1)."
        );
    }
}
