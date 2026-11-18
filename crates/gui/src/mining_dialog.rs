//! The Attribute Importance (AIM), Feature Selection (SELECTPRED) and Prepare Data for
//! Modeling (ADP) dialogs. Options left as they are add nothing to the syntax, so the commands
//! run with their own defaults.

use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::{names, number, quote};

/// A whole number the command needs, from text.
fn count(text: &str, least: u32, what: &str) -> Result<u32, String> {
    whole(text, least, what)?.ok_or_else(|| format!("give {what}"))
}

/// A number above `low` (or at least `low` when not `open`) and at most `high`, from text.
fn within(text: &str, what: &str, low: f64, open: bool, high: f64) -> Result<f64, String> {
    let n = parse(text, what)?;
    if n <= high && (n > low || (!open && n == low)) {
        return Ok(n);
    }
    let bound = if open { "above" } else { "at least" };
    Err(format!(
        "{what} must be {bound} {} and at most {}",
        number(low),
        number(high)
    ))
}

/// A subcommand line from its parts, left out when there are none.
fn sub(text: &mut String, name: &str, parts: Vec<String>) {
    if !parts.is_empty() {
        text.push_str(&format!("\n  /{name} {}", parts.join(" ")));
    }
}

fn yes_no(on: bool) -> &'static str {
    if on { "YES" } else { "NO" }
}

/// An error when a name is in the lists twice.
fn once_only(lists: &[&Vec<String>], what: &str) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for name in lists.iter().flat_map(|l| l.iter()) {
        let lower = name.to_lowercase();
        if seen.contains(&lower) {
            return Err(format!("{name} is listed twice in {what}"));
        }
        seen.push(lower);
    }
    Ok(())
}

/// AIM: how much each variable sets every group apart from all cases.
#[derive(Clone, PartialEq, Debug)]
pub struct Aim {
    pub group: String,
    pub categorical: Vec<String>,
    pub continuous: Vec<String>,
    /// Divide the significance level by the number of groups.
    pub bonferroni: bool,
    pub ci: String,
    pub hide_not_significant: bool,
    pub reference_lines: bool,
    pub include: bool,
    pub category_plot: bool,
    /// 0 none, 1 bars, 2 pie.
    pub cluster_plot: usize,
    pub error_bar: bool,
    pub importance: bool,
    /// One chart per group with the variables along the axis, instead of one per variable.
    pub by_variable: bool,
    /// Chart -log10 of the significance instead of the test statistic.
    pub pvalue: bool,
}

impl Default for Aim {
    fn default() -> Self {
        Self {
            group: String::new(),
            categorical: Vec::new(),
            continuous: Vec::new(),
            bonferroni: true,
            ci: "95".into(),
            hide_not_significant: false,
            reference_lines: true,
            include: false,
            category_plot: false,
            cluster_plot: 0,
            error_bar: false,
            importance: false,
            by_variable: false,
            pvalue: false,
        }
    }
}

impl Aim {
    pub fn syntax(&self) -> Result<String, String> {
        let group = self.group.trim();
        if group.is_empty() {
            return Err("set the grouping variable".into());
        }
        if self.categorical.is_empty() && self.continuous.is_empty() {
            return Err("add at least one variable to Categorical or Continuous".into());
        }
        once_only(&[&self.categorical, &self.continuous], "the variables")?;
        if self
            .categorical
            .iter()
            .chain(&self.continuous)
            .any(|n| n.eq_ignore_ascii_case(group))
        {
            return Err(format!(
                "{group} is the grouping variable, not an analysis variable"
            ));
        }
        let mut text = format!("AIM {group}");
        if !self.categorical.is_empty() {
            text.push_str(&format!(
                "\n  /CATEGORICAL {}",
                names(&self.categorical, "Categorical")?
            ));
        }
        if !self.continuous.is_empty() {
            text.push_str(&format!(
                "\n  /CONTINUOUS {}",
                names(&self.continuous, "Continuous")?
            ));
        }
        let ci = within(&self.ci, "the confidence level", 0.0, true, 99.999)?;
        let mut criteria = Vec::new();
        if !self.bonferroni {
            criteria.push("ADJUST=NONE".to_string());
        }
        if ci != 95.0 {
            criteria.push(format!("CI={}", number(ci)));
        }
        if self.hide_not_significant {
            criteria.push("HIDENOTSIG=YES".into());
        }
        if !self.reference_lines {
            criteria.push("SHOWREFLINE=NO".into());
        }
        sub(&mut text, "CRITERIA", criteria);
        if self.include {
            text.push_str("\n  /MISSING=INCLUDE");
        }
        let mut plot = Vec::new();
        if self.category_plot {
            plot.push("CATEGORY".to_string());
        }
        match self.cluster_plot {
            1 => plot.push("CLUSTER".into()),
            2 => plot.push("CLUSTER(TYPE=PIE)".into()),
            _ => {}
        }
        if self.error_bar {
            plot.push("ERRORBAR".into());
        }
        if self.importance {
            let mut options = Vec::new();
            if self.by_variable {
                options.push("X=VARIABLE");
            }
            if self.pvalue {
                options.push("Y=PVALUE");
            }
            match options.is_empty() {
                true => plot.push("IMPORTANCE".into()),
                false => plot.push(format!("IMPORTANCE({})", options.join(" "))),
            }
        }
        sub(&mut text, "PLOT", plot);
        text.push('.');
        Ok(text)
    }
}

/// The rankings SELECTPRED knows, in the order shown.
pub const RANKINGS: [(&str, &str); 4] = [
    ("PEARSONCHISQ", "Pearson chi-square significance"),
    ("LRCHISQ", "Likelihood-ratio chi-square significance"),
    ("CRAMERSV", "Cramer's V"),
    ("LAMBDA", "Lambda"),
];

/// The tables SELECTPRED prints, in the order shown; all are on by default.
pub const SELECT_PRINT: [(&str, &str); 4] = [
    ("CPS", "Case processing summary"),
    ("EXCLUDED", "Excluded predictors"),
    ("SELECTED", "Selected predictors"),
    ("SUMMARY", "Predictor ranking"),
];

/// The measurement level of the target: the label and the letter SELECTPRED takes.
pub const TARGET_LEVELS: [(&str, &str); 4] = [
    ("", "From the data"),
    ("S", "Scale"),
    ("O", "Ordinal"),
    ("N", "Nominal"),
];

/// SELECTPRED: screens and ranks predictors of a target.
#[derive(Clone, PartialEq, Debug)]
pub struct FeatureSelection {
    pub target: String,
    /// An index into `TARGET_LEVELS`.
    pub level: usize,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    pub except: Vec<String>,
    pub screening: bool,
    pub pct_missing: String,
    pub pct_equal: String,
    pub pct_unequal: String,
    pub min_sd: String,
    pub min_cv: String,
    /// Keep the number of predictors the command chooses, or `size` of them.
    pub auto_size: bool,
    pub size: String,
    pub pcutoff: String,
    /// An index into `RANKINGS`.
    pub ranking: usize,
    pub show_unselected: String,
    pub timer: String,
    pub include: bool,
    /// One flag per entry of `SELECT_PRINT`.
    pub print: [bool; 4],
    pub plot: bool,
}

impl Default for FeatureSelection {
    fn default() -> Self {
        Self {
            target: String::new(),
            level: 0,
            factors: Vec::new(),
            covariates: Vec::new(),
            except: Vec::new(),
            screening: true,
            pct_missing: "70".into(),
            pct_equal: "95".into(),
            pct_unequal: "90".into(),
            min_sd: "0".into(),
            min_cv: "0.001".into(),
            auto_size: true,
            size: "10".into(),
            pcutoff: "0.05".into(),
            ranking: 0,
            show_unselected: "0".into(),
            timer: "5".into(),
            include: false,
            print: [true; 4],
            plot: false,
        }
    }
}

impl FeatureSelection {
    pub fn syntax(&self) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err("set the target".into());
        }
        once_only(&[&self.factors, &self.covariates], "the predictors")?;
        if self
            .factors
            .iter()
            .chain(&self.covariates)
            .any(|n| n.eq_ignore_ascii_case(target))
        {
            return Err(format!("{target} is the target, not a predictor"));
        }
        let mut text = format!("SELECTPRED {target}");
        if self.level != 0 {
            text.push_str(&format!(" (MLEVEL={})", TARGET_LEVELS[self.level].0));
        }
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        if !self.except.is_empty() {
            text.push_str(&format!("\n  /EXCEPT VARIABLES={}", self.except.join(" ")));
        }
        let mut screening = Vec::new();
        if !self.screening {
            screening.push("STATUS=OFF".to_string());
        } else {
            let mut percent = |t: &str, what: &str, keyword: &str, default: f64| {
                let n = within(t, what, 0.0, true, 100.0)?;
                if n != default {
                    screening.push(format!("{keyword}={}", number(n)));
                }
                Ok::<(), String>(())
            };
            percent(&self.pct_missing, "the percent missing", "PCTMISSING", 70.0)?;
            percent(
                &self.pct_equal,
                "the percent in one category",
                "PCTEQUAL",
                95.0,
            )?;
            percent(
                &self.pct_unequal,
                "the percent of single-case categories",
                "PCTUNEQUAL",
                90.0,
            )?;
            let sd = parse(&self.min_sd, "the smallest standard deviation")?;
            if sd < 0.0 {
                return Err("the smallest standard deviation must be at least 0".into());
            }
            if sd != 0.0 {
                screening.push(format!("STDDEV={}", number(sd)));
            }
            let cv = parse(&self.min_cv, "the smallest coefficient of variation")?;
            if cv <= 0.0 {
                return Err("the smallest coefficient of variation must be above 0".into());
            }
            if cv != 0.001 {
                screening.push(format!("CV={}", number(cv)));
            }
        }
        sub(&mut text, "SCREENING", screening);
        let mut criteria = Vec::new();
        if !self.auto_size {
            let n = count(&self.size, 1, "the number of predictors to keep")?;
            criteria.push(format!("SIZE={n}"));
        }
        let cutoff = within(&self.pcutoff, "the significance cutoff", 0.0, true, 1.0)?;
        if cutoff != 0.05 {
            criteria.push(format!("PCUTOFF={}", number(cutoff)));
        }
        if self.ranking != 0 {
            criteria.push(format!("RANKING={}", RANKINGS[self.ranking].0));
        }
        let extra = whole(
            &self.show_unselected,
            0,
            "the number of unselected predictors to show",
        )?;
        if let Some(n) = extra.filter(|n| *n != 0) {
            criteria.push(format!("SHOWUNSELECTED={n}"));
        }
        let timer = parse(&self.timer, "the time limit")?;
        if timer < 0.0 {
            return Err("the time limit must be at least 0".into());
        }
        if timer != 5.0 {
            criteria.push(format!("TIMER={}", number(timer)));
        }
        sub(&mut text, "CRITERIA", criteria);
        if self.include {
            text.push_str("\n  /MISSING USERMISSING=INCLUDE");
        }
        if self.print != [true; 4] {
            let shown: Vec<&str> = SELECT_PRINT
                .iter()
                .zip(self.print)
                .filter_map(|((keyword, _), on)| on.then_some(*keyword))
                .collect();
            let list = if shown.is_empty() {
                "NONE".to_string()
            } else {
                shown.join(" ")
            };
            text.push_str(&format!("\n  /PRINT {list}"));
        }
        if self.plot {
            text.push_str("\n  /PLOT SUMMARY");
        }
        text.push('.');
        Ok(text)
    }
}

/// The units of the time since a date, then since a time of day; the first is chosen by the
/// command.
pub const DATE_UNITS: [(&str, &str); 4] = [
    ("AUTO", "Automatic"),
    ("YEARS", "Years"),
    ("MONTHS", "Months"),
    ("DAYS", "Days"),
];
pub const TIME_UNITS: [(&str, &str); 4] = [
    ("AUTO", "Automatic"),
    ("HOURS", "Hours"),
    ("MINUTES", "Minutes"),
    ("SECONDS", "Seconds"),
];

/// The parts of a date or time that can become fields of their own.
pub const EXTRACTS: [(&str, &str); 6] = [
    ("EXTRACTYEAR", "Year"),
    ("EXTRACTMONTH", "Month"),
    ("EXTRACTDAY", "Day"),
    ("EXTRACTHOUR", "Hour"),
    ("EXTRACTMINUTE", "Minute"),
    ("EXTRACTSECOND", "Second"),
];

/// Three whole numbers in quotes, as `YMD('2025-01-31')` or `HMS('12:30:00')` wants them.
fn stamp(text: &str, what: &str) -> Result<String, String> {
    let parts = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .count();
    match parts == 3 {
        true => Ok(quote(text.trim())),
        false => Err(format!(
            "{what} needs three numbers, for example 2025-01-31"
        )),
    }
}

/// ADP: prepares a target and its inputs and writes the preparation as syntax.
#[derive(Clone, PartialEq, Debug)]
pub struct PrepareData {
    pub target: String,
    pub inputs: Vec<String>,
    pub weight: String,
    pub date_duration: bool,
    /// Empty counts from today.
    pub date_reference: String,
    pub date_unit: usize,
    pub time_duration: bool,
    /// Empty counts from now.
    pub time_reference: String,
    pub time_unit: usize,
    /// One flag per entry of `EXTRACTS`.
    pub extract: [bool; 6],
    pub screen_missing: bool,
    pub max_missing: String,
    pub screen_categories: bool,
    pub max_categories: String,
    pub screen_single: bool,
    pub max_single: String,
    pub adjust_input: bool,
    pub adjust_target: bool,
    pub max_ordinal: String,
    pub min_continuous: String,
    pub outlier_input: bool,
    pub outlier_target: bool,
    pub cutoff: String,
    pub outliers_to_missing: bool,
    pub replace_input: bool,
    pub replace_target: bool,
    pub reorder_input: bool,
    pub reorder_target: bool,
    /// 0 z scores, 1 minimum and maximum, 2 none.
    pub rescale_input: usize,
    pub min: String,
    pub max: String,
    pub box_cox: bool,
    pub merge_supervised: bool,
    pub merge_unsupervised: bool,
    pub min_pct: String,
    pub binning: bool,
    pub selection: bool,
    pub pvalue: String,
    pub suffix_target: String,
    pub suffix_input: String,
    pub outfile: String,
}

impl Default for PrepareData {
    fn default() -> Self {
        Self {
            target: String::new(),
            inputs: Vec::new(),
            weight: String::new(),
            date_duration: true,
            date_reference: String::new(),
            date_unit: 0,
            time_duration: true,
            time_reference: String::new(),
            time_unit: 0,
            extract: [false; 6],
            screen_missing: true,
            max_missing: "50".into(),
            screen_categories: true,
            max_categories: "100".into(),
            screen_single: false,
            max_single: "95".into(),
            adjust_input: true,
            adjust_target: true,
            max_ordinal: "10".into(),
            min_continuous: "5".into(),
            outlier_input: true,
            outlier_target: true,
            cutoff: "3".into(),
            outliers_to_missing: false,
            replace_input: true,
            replace_target: false,
            reorder_input: false,
            reorder_target: false,
            rescale_input: 0,
            min: "0".into(),
            max: "100".into(),
            box_cox: true,
            merge_supervised: true,
            merge_unsupervised: false,
            min_pct: "10".into(),
            binning: true,
            selection: true,
            pvalue: "0.05".into(),
            suffix_target: "_transformed".into(),
            suffix_input: "_transformed".into(),
            outfile: String::new(),
        }
    }
}

/// `NAME=YES|NO` when it differs from the command's own default.
fn flag(parts: &mut Vec<String>, name: &str, on: bool, default: bool) {
    if on != default {
        parts.push(format!("{name}={}", yes_no(on)));
    }
}

/// A duration of a date or time: left out when it is the command's own (on, from now, automatic).
fn duration(
    parts: &mut Vec<String>,
    name: &str,
    (on, reference, unit): (bool, &str, &str),
    (word, what): (&str, &str),
) -> Result<(), String> {
    if !on {
        parts.push(format!("{name}=NO"));
        return Ok(());
    }
    let mut options = Vec::new();
    if !reference.trim().is_empty() {
        options.push(format!("REFERENCE={word}({})", stamp(reference, what)?));
    }
    if unit != "AUTO" {
        options.push(format!("UNIT={unit}"));
    }
    if !options.is_empty() {
        parts.push(format!("{name}=YES({})", options.join(" ")));
    }
    Ok(())
}

/// A screening limit: `NAME=NO`, or `NAME=YES` with its limit, when different from the default.
fn limit(
    parts: &mut Vec<String>,
    name: &str,
    (on, default_on): (bool, bool),
    (option, value, default): (&str, String, &str),
) {
    if !on {
        if default_on {
            parts.push(format!("{name}=NO"));
        }
    } else if value != default {
        parts.push(format!("{name}=YES({option}={value})"));
    } else if !default_on {
        parts.push(format!("{name}=YES"));
    }
}

impl PrepareData {
    pub fn syntax(&self) -> Result<String, String> {
        if self.inputs.is_empty() {
            return Err("add at least one variable to Inputs".into());
        }
        let target = self.target.trim();
        let weight = self.weight.trim();
        once_only(&[&self.inputs], "Inputs")?;
        if let Some(clash) = self.inputs.iter().find(|n| {
            n.eq_ignore_ascii_case(target) || (!weight.is_empty() && n.eq_ignore_ascii_case(weight))
        }) {
            return Err(format!(
                "{clash} cannot be an input and also the target or the weight"
            ));
        }
        if self.outfile.trim().is_empty() {
            return Err("give the file the preparation syntax is written to".into());
        }
        let mut text = String::from("ADP /FIELDS");
        if !target.is_empty() {
            text.push_str(&format!(" TARGET={target}"));
        }
        text.push_str(&format!(" INPUT={}", self.inputs.join(" ")));
        if !weight.is_empty() {
            text.push_str(&format!(" ANALYSISWEIGHT={weight}"));
        }

        let mut dates = Vec::new();
        duration(
            &mut dates,
            "DATEDURATION",
            (
                self.date_duration,
                &self.date_reference,
                DATE_UNITS[self.date_unit].0,
            ),
            ("YMD", "the reference date"),
        )?;
        duration(
            &mut dates,
            "TIMEDURATION",
            (
                self.time_duration,
                &self.time_reference,
                TIME_UNITS[self.time_unit].0,
            ),
            ("HMS", "the reference time"),
        )?;
        for ((keyword, _), on) in EXTRACTS.iter().zip(self.extract) {
            if on {
                dates.push(format!("{keyword}=YES"));
            }
        }
        sub(&mut text, "PREPDATETIME", dates);

        let mut screening = Vec::new();
        // a limit whose check is off is greyed out in the dialog, so its text is not read
        let missing = match self.screen_missing {
            true => number(within(
                &self.max_missing,
                "the percent missing",
                0.0,
                false,
                100.0,
            )?),
            false => "50".into(),
        };
        limit(
            &mut screening,
            "PCTMISSING",
            (self.screen_missing, true),
            ("MAXPCT", missing, "50"),
        );
        let categories = match self.screen_categories {
            true => count(&self.max_categories, 1, "the most categories")?.to_string(),
            false => "100".into(),
        };
        limit(
            &mut screening,
            "UNIQUECAT",
            (self.screen_categories, true),
            ("MAXCAT", categories, "100"),
        );
        let single = match self.screen_single {
            true => number(within(
                &self.max_single,
                "the percent in one category",
                0.0,
                false,
                100.0,
            )?),
            false => "95".into(),
        };
        limit(
            &mut screening,
            "SINGLECAT",
            (self.screen_single, false),
            ("MAXPCT", single, "95"),
        );
        sub(&mut text, "SCREENING", screening);

        let mut level = Vec::new();
        flag(&mut level, "INPUT", self.adjust_input, true);
        flag(&mut level, "TARGET", self.adjust_target, true);
        let ordinal = count(&self.max_ordinal, 1, "the most values of an ordinal field")?;
        let continuous = count(
            &self.min_continuous,
            1,
            "the fewest values of a continuous field",
        )?;
        if continuous > ordinal {
            return Err(
                "the fewest values of a continuous field cannot be above the most values of an ordinal one"
                    .into(),
            );
        }
        if ordinal != 10 {
            level.push(format!("MAXVALORDINAL={ordinal}"));
        }
        if continuous != 5 {
            level.push(format!("MINVALCONTINUOUS={continuous}"));
        }
        sub(&mut text, "ADJUSTLEVEL", level);

        let mut outliers = Vec::new();
        flag(&mut outliers, "INPUT", self.outlier_input, true);
        flag(&mut outliers, "TARGET", self.outlier_target, true);
        let cutoff = parse(&self.cutoff, "the outlier cutoff")?;
        if cutoff <= 0.0 {
            return Err("the outlier cutoff must be above 0".into());
        }
        if cutoff != 3.0 {
            outliers.push(format!("CUTOFF=SD({})", number(cutoff)));
        }
        if self.outliers_to_missing {
            outliers.push("REPLACEWITH=MISSING".into());
        }
        sub(&mut text, "OUTLIERHANDLING", outliers);

        let mut replace = Vec::new();
        flag(&mut replace, "INPUT", self.replace_input, true);
        flag(&mut replace, "TARGET", self.replace_target, false);
        sub(&mut text, "REPLACEMISSING", replace);

        let mut reorder = Vec::new();
        flag(&mut reorder, "INPUT", self.reorder_input, false);
        flag(&mut reorder, "TARGET", self.reorder_target, false);
        sub(&mut text, "REORDERNOMINAL", reorder);

        let mut rescale = Vec::new();
        match self.rescale_input {
            1 => {
                let min = parse(&self.min, "the minimum")?;
                let max = parse(&self.max, "the maximum")?;
                if max <= min {
                    return Err("the maximum must be above the minimum".into());
                }
                rescale.push(format!(
                    "INPUT=MINMAX(MIN={} MAX={})",
                    number(min),
                    number(max)
                ));
            }
            2 => rescale.push("INPUT=NONE".into()),
            _ => {}
        }
        if !self.box_cox {
            rescale.push("TARGET=NONE".into());
        }
        sub(&mut text, "RESCALE", rescale);

        // the three tests on the target share one p value; each is on by default
        let p = match self.merge_supervised || self.binning || self.selection {
            true => within(&self.pvalue, "the p value", 0.0, true, 1.0)?,
            false => 0.05,
        };
        let mut transform = Vec::new();
        let tested = |name: &str, on: bool, (yes, no): (&str, &str)| match (on, p != 0.05) {
            (true, true) => Some(format!("{name}={yes}(PVALUE={})", number(p))),
            (false, _) => Some(format!("{name}={no}")),
            _ => None,
        };
        transform.extend(tested(
            "MERGESUPERVISED",
            self.merge_supervised,
            ("YES", "NO"),
        ));
        if self.merge_unsupervised {
            let pct = within(
                &self.min_pct,
                "the smallest share of cases",
                0.0,
                false,
                100.0,
            )?;
            match pct == 10.0 {
                true => transform.push("MERGEUNSUPERVISED=YES".into()),
                false => transform.push(format!("MERGEUNSUPERVISED=YES(MINPCT={})", number(pct))),
            }
        }
        transform.extend(tested("BINNING", self.binning, ("SUPERVISED", "NONE")));
        transform.extend(tested("SELECTION", self.selection, ("YES", "NO")));
        sub(&mut text, "TRANSFORM", transform);

        let (st, si) = (self.suffix_target.trim(), self.suffix_input.trim());
        if (st, si) != ("_transformed", "_transformed") {
            let mut suffixes = Vec::new();
            if st != "_transformed" {
                suffixes.push(format!("TARGET={}", quote(st)));
            }
            if si != "_transformed" {
                suffixes.push(format!("INPUT={}", quote(si)));
            }
            text.push_str(&format!("\n  /CRITERIA SUFFIX({})", suffixes.join(" ")));
        }
        text.push_str(&format!(
            "\n  /OUTFILE PREPXML={}.",
            quote(self.outfile.trim())
        ));
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    #[test]
    fn aim_writes_only_what_changed() {
        let mut d = Aim {
            group: "region".into(),
            continuous: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "AIM region\n  /CONTINUOUS age.");
        d.categorical = v(&["gender"]);
        d.bonferroni = false;
        d.ci = "90".into();
        d.hide_not_significant = true;
        d.reference_lines = false;
        d.include = true;
        d.category_plot = true;
        d.cluster_plot = 2;
        d.error_bar = true;
        d.importance = true;
        d.by_variable = true;
        d.pvalue = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "AIM region\n  /CATEGORICAL gender\n  /CONTINUOUS age\n  \
             /CRITERIA ADJUST=NONE CI=90 HIDENOTSIG=YES SHOWREFLINE=NO\n  /MISSING=INCLUDE\n  \
             /PLOT CATEGORY CLUSTER(TYPE=PIE) ERRORBAR IMPORTANCE(X=VARIABLE Y=PVALUE)."
        );
        assert!(run_in(&mut open("survey_basic.sav"), &syntax) >= 1);
        d.group = "age".into();
        assert!(d.syntax().unwrap_err().contains("grouping variable"));
        d.group.clear();
        assert_eq!(d.syntax().unwrap_err(), "set the grouping variable");
    }

    #[test]
    fn feature_selection_writes_only_what_changed() {
        let mut d = FeatureSelection {
            target: "plan".into(),
            covariates: v(&["age", "monthly_spend"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "SELECTPRED plan WITH age monthly_spend."
        );
        d.level = 3;
        d.factors = v(&["region"]);
        d.except = v(&["age"]);
        d.pct_missing = "60".into();
        d.min_sd = "0.5".into();
        d.auto_size = false;
        d.size = "2".into();
        d.pcutoff = "0.1".into();
        d.ranking = 2;
        d.show_unselected = "1".into();
        d.timer = "0".into();
        d.include = true;
        d.print = [false, false, true, true];
        d.plot = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "SELECTPRED plan (MLEVEL=N) BY region WITH age monthly_spend\n  /EXCEPT VARIABLES=age\n  \
             /SCREENING PCTMISSING=60 STDDEV=0.5\n  \
             /CRITERIA SIZE=2 PCUTOFF=0.1 RANKING=CRAMERSV SHOWUNSELECTED=1 TIMER=0\n  \
             /MISSING USERMISSING=INCLUDE\n  /PRINT SELECTED SUMMARY\n  /PLOT SUMMARY."
        );
        assert!(run_in(&mut open("survey_basic.sav"), &syntax) >= 1);
        d.screening = false;
        d.print = [false; 4];
        let text = d.syntax().unwrap();
        assert!(
            text.contains("/SCREENING STATUS=OFF") && text.contains("/PRINT NONE"),
            "{text}"
        );
        d.pcutoff = "0".into();
        assert!(d.syntax().is_err());
    }

    #[test]
    fn prepare_data_writes_only_what_changed() {
        let mut d = PrepareData {
            inputs: v(&["age", "monthly_spend"]),
            outfile: "prep.sps".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "ADP /FIELDS INPUT=age monthly_spend\n  /OUTFILE PREPXML='prep.sps'."
        );
        d.target = "plan".into();
        d.weight = "resp_id".into();
        d.date_reference = "2025-01-01".into();
        d.date_unit = 2;
        d.time_duration = false;
        d.extract[0] = true;
        d.screen_missing = false;
        d.screen_single = true;
        d.max_single = "90".into();
        d.max_ordinal = "8".into();
        d.cutoff = "2.5".into();
        d.outliers_to_missing = true;
        d.replace_target = true;
        d.reorder_input = true;
        d.rescale_input = 1;
        d.box_cox = false;
        d.merge_unsupervised = true;
        d.min_pct = "20".into();
        d.binning = false;
        d.pvalue = "0.1".into();
        d.suffix_input = "_p".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "ADP /FIELDS TARGET=plan INPUT=age monthly_spend ANALYSISWEIGHT=resp_id\n  \
             /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2025-01-01') UNIT=MONTHS) \
             TIMEDURATION=NO EXTRACTYEAR=YES\n  \
             /SCREENING PCTMISSING=NO SINGLECAT=YES(MAXPCT=90)\n  /ADJUSTLEVEL MAXVALORDINAL=8\n  \
             /OUTLIERHANDLING CUTOFF=SD(2.5) REPLACEWITH=MISSING\n  /REPLACEMISSING TARGET=YES\n  \
             /REORDERNOMINAL INPUT=YES\n  /RESCALE INPUT=MINMAX(MIN=0 MAX=100) TARGET=NONE\n  \
             /TRANSFORM MERGESUPERVISED=YES(PVALUE=0.1) MERGEUNSUPERVISED=YES(MINPCT=20) \
             BINNING=NONE SELECTION=YES(PVALUE=0.1)\n  /CRITERIA SUFFIX(INPUT='_p')\n  \
             /OUTFILE PREPXML='prep.sps'."
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        d.date_reference = "soon".into();
        assert!(d.syntax().unwrap_err().contains("three numbers"));
        d.date_reference.clear();
        d.min_continuous = "11".into();
        assert!(d.syntax().is_err());
    }
}
