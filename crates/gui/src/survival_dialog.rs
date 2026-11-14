//! The Life Tables (SURVIVAL) and Kaplan-Meier (KM) dialogs. Options left as they are add
//! nothing to the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::correspondence_dialog::TableVariable;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{number, value};
use statscraft_data::{Dataset, is_valid_name};

/// The tests of KM's /TEST; the log rank test alone is the default.
pub const KM_TESTS: [(&str, &str); 3] = [
    ("LOGRANK", "Log rank"),
    ("BRESLOW", "Breslow"),
    ("TARONE", "Tarone-Ware"),
];

const KM_DEFAULT_TESTS: [bool; 3] = [true, false, false];

/// The ways of KM's /COMPARE; the first is the default.
pub const KM_COMPARE: [(&str, &str); 4] = [
    ("", "All levels at once, pooled over strata"),
    ("OVERALL STRATA", "All levels at once, for each stratum"),
    ("PAIRWISE POOLED", "Each pair of levels, pooled over strata"),
    ("PAIRWISE STRATA", "Each pair of levels, for each stratum"),
];

/// The new variables of KM's /SAVE.
pub const KM_SAVE: [(&str, &str); 4] = [
    ("SURVIVAL", "Survival"),
    ("SE", "Standard error of survival"),
    ("HAZARD", "Cumulative hazard"),
    ("CUMEVENT", "Cumulative events"),
];

/// The charts of KM's /PLOT.
pub const KM_PLOTS: [(&str, &str); 4] = [
    ("SURVIVAL", "Survival"),
    ("OMS", "One minus survival"),
    ("HAZARD", "Hazard"),
    ("LOGSURV", "Log survival"),
];

/// KM: product-limit survival estimates of one time variable.
#[derive(Clone, PartialEq, Debug)]
pub struct KaplanMeier {
    pub time: String,
    pub status: String,
    /// The event is a range of values rather than a list.
    pub event_range: bool,
    /// One value or several, separated by spaces or commas (commas only for a string status).
    pub event_values: String,
    pub event_from: String,
    pub event_to: String,
    /// Values of cases lost to follow-up, in the same form as `event_values`; may be empty.
    pub lost: String,
    pub factor: String,
    pub strata: String,
    /// Labels the cases in the survival table.
    pub id: String,
    /// One flag per entry of `KM_TESTS`.
    pub tests: [bool; 3],
    /// Index into `KM_COMPARE`.
    pub compare: usize,
    pub trend: bool,
    /// Empty for evenly spaced weights.
    pub trend_weights: String,
    /// One flag and one optional new name per entry of `KM_SAVE`.
    pub save: [bool; 4],
    pub save_names: [String; 4],
    pub table: bool,
    pub mean: bool,
    pub quartiles: bool,
    /// More percentiles, separated by spaces or commas.
    pub percentiles: String,
    /// One flag per entry of `KM_PLOTS`.
    pub plots: [bool; 4],
}

impl Default for KaplanMeier {
    fn default() -> Self {
        Self {
            time: String::new(),
            status: String::new(),
            event_range: false,
            event_values: String::new(),
            event_from: String::new(),
            event_to: String::new(),
            lost: String::new(),
            factor: String::new(),
            strata: String::new(),
            id: String::new(),
            tests: KM_DEFAULT_TESTS,
            compare: 0,
            trend: false,
            trend_weights: String::new(),
            save: [false; 4],
            save_names: Default::default(),
            table: true,
            mean: true,
            quartiles: false,
            percentiles: String::new(),
            plots: [false; 4],
        }
    }
}

/// The parts of a typed list: separated by commas, and for numbers by spaces too.
fn items(text: &str, string: bool) -> impl Iterator<Item = &str> {
    text.split(move |c: char| c == ',' || (!string && c.is_whitespace()))
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// A typed list of values as literals for a status of the given type.
fn values(text: &str, string: bool) -> Result<String, String> {
    let list: Vec<String> = items(text, string)
        .map(|t| value(t, string))
        .collect::<Result<_, _>>()?;
    Ok(list.join(" "))
}

/// A typed list of numbers.
fn numbers(text: &str, what: &str) -> Result<Vec<f64>, String> {
    items(text, false).map(|t| parse(t, what)).collect()
}

/// The event values of a status variable as the command takes them, `1 2` or `3 THRU 5`, and
/// whether the status is a string.
pub(crate) fn event(
    data: &Dataset,
    status: &str,
    range: bool,
    list: &str,
    from: &str,
    to: &str,
) -> Result<(String, bool), String> {
    let string = match data.variable(status) {
        Ok(v) => v.is_string(),
        Err(_) => return Err(format!("{status} is no longer a variable")),
    };
    let event = if range {
        if string {
            return Err(format!(
                "{status} is a string: give its event values, not a range"
            ));
        }
        let from = parse(from, "the start of the event range")?;
        let to = parse(to, "the end of the event range")?;
        if from > to {
            return Err("the event range starts above its end".into());
        }
        format!("{} THRU {}", number(from), number(to))
    } else {
        match values(list, string)? {
            v if v.is_empty() => return Err("give the values that mark the event".into()),
            v => v,
        }
    };
    Ok((event, string))
}

/// The keywords of the checked saved variables, each with its new name when one is given,
/// checked: valid, not taken and not given twice.
pub(crate) fn saved(
    data: &Dataset,
    options: &[(&str, &str)],
    flags: &[bool],
    names: &[String],
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut given: Vec<&str> = Vec::new();
    for (i, (keyword, _)) in options.iter().enumerate() {
        if !flags[i] {
            continue;
        }
        let name = names[i].trim();
        if name.is_empty() {
            out.push(keyword.to_string());
            continue;
        }
        // a scratch name is valid elsewhere, but the command cannot save one
        if !is_valid_name(name) || name.starts_with('#') {
            return Err(format!("{name} cannot name a variable"));
        }
        if data.variable(name).is_ok() {
            return Err(format!("{name} is already a variable"));
        }
        if given.iter().any(|g| g.eq_ignore_ascii_case(name)) {
            return Err(format!("{name} names two saved variables"));
        }
        given.push(name);
        out.push(format!("{keyword}({name})"));
    }
    Ok(out)
}

impl KaplanMeier {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let time = self.time.trim();
        let status = self.status.trim();
        if time.is_empty() {
            return Err("choose the time variable".into());
        }
        if status.is_empty() {
            return Err("choose the status variable".into());
        }
        numeric(data, [&self.time])?;
        let (event, string) = event(
            data,
            status,
            self.event_range,
            &self.event_values,
            &self.event_from,
            &self.event_to,
        )?;
        let mut text = format!("KM {time}");
        let factor = self.factor.trim();
        if !factor.is_empty() {
            text.push_str(&format!(" BY {factor}"));
        }
        text.push_str(&format!("\n  /STATUS={status} EVENT({event})"));
        let lost = values(&self.lost, string)?;
        if !lost.is_empty() {
            text.push_str(&format!(" LOST({lost})"));
        }
        let strata = self.strata.trim();
        if !strata.is_empty() {
            text.push_str(&format!("\n  /STRATA={strata}"));
        }
        let id = self.id.trim();
        if !id.is_empty() {
            text.push_str(&format!("\n  /ID={id}"));
        }
        let plots = keywords(&KM_PLOTS, &self.plots);
        if !plots.is_empty() {
            text.push_str(&format!("\n  /PLOT={}", plots.join(" ")));
        }
        match (self.table, self.mean) {
            (true, true) => {}
            (true, false) => text.push_str("\n  /PRINT=TABLE"),
            (false, true) => text.push_str("\n  /PRINT=MEAN"),
            (false, false) => text.push_str("\n  /PRINT=NONE"),
        }
        let mut percentiles = numbers(&self.percentiles, "a percentile")?;
        if percentiles.iter().any(|p| !(*p > 0.0 && *p < 100.0)) {
            return Err("percentiles lie above 0 and below 100".into());
        }
        if !percentiles.is_empty() {
            if self.quartiles {
                percentiles.extend([25.0, 50.0, 75.0]);
            }
            percentiles.sort_by(f64::total_cmp);
            percentiles.dedup();
            let list: Vec<String> = percentiles.into_iter().map(number).collect();
            text.push_str(&format!("\n  /PERCENTILES=({})", list.join(" ")));
        } else if self.quartiles {
            text.push_str("\n  /PERCENTILES");
        }
        // the tests compare the factor levels, so they need a factor
        if !factor.is_empty() {
            text.push_str(&self.comparisons(!strata.is_empty())?);
        }
        text.push_str(&self.save_syntax(data)?);
        text.push('.');
        Ok(text)
    }

    /// /TEST, /COMPARE and /TREND, each only when changed.
    fn comparisons(&self, strata: bool) -> Result<String, String> {
        let mut text = String::new();
        if self.tests != KM_DEFAULT_TESTS {
            let tests = keywords(&KM_TESTS, &self.tests);
            if tests.is_empty() {
                return Err("check at least one test of the factor levels".into());
            }
            text.push_str(&format!("\n  /TEST={}", tests.join(" ")));
        }
        match KM_COMPARE[self.compare].0 {
            "" => {}
            k if k.ends_with("STRATA") && !strata => {
                return Err("comparing within each stratum needs a strata variable".into());
            }
            k => text.push_str(&format!("\n  /COMPARE={k}")),
        }
        // the trend feeds only all-levels-at-once tests; pairwise tests ignore it
        if self.trend && !self.pairwise() {
            let weights = numbers(&self.trend_weights, "a trend weight")?;
            match weights.is_empty() {
                true => text.push_str("\n  /TREND"),
                false => {
                    let list: Vec<String> = weights.into_iter().map(number).collect();
                    text.push_str(&format!("\n  /TREND=({})", list.join(" ")));
                }
            }
        }
        Ok(text)
    }

    pub(crate) fn pairwise(&self) -> bool {
        KM_COMPARE[self.compare].0.starts_with("PAIRWISE")
    }

    fn save_syntax(&self, data: &Dataset) -> Result<String, String> {
        let out = saved(data, &KM_SAVE, &self.save, &self.save_names)?;
        Ok(match out.is_empty() {
            true => String::new(),
            false => format!("\n  /SAVE={}", out.join(" ")),
        })
    }
}

/// The charts of SURVIVAL's /PLOTS.
pub const LIFE_PLOTS: [(&str, &str); 5] = [
    ("SURVIVAL", "Survival"),
    ("OMS", "One minus survival"),
    ("HAZARD", "Hazard"),
    ("DENSITY", "Density"),
    ("LOGSURV", "Log survival"),
];

/// How SURVIVAL compares the levels of the first factor.
pub const LIFE_COMPARE: [(&str, &str); 3] =
    [("", "None"), ("", "Overall"), ("PAIRWISE", "Pairwise")];

/// SURVIVAL: actuarial life tables of one or more time variables.
#[derive(Clone, PartialEq, Debug)]
pub struct LifeTables {
    pub times: Vec<String>,
    /// Each row `(end, width)`: from where the last row ended (0 first) through `end`.
    pub intervals: Vec<(String, String)>,
    pub status: String,
    pub event_from: String,
    /// Empty when one value marks the event.
    pub event_to: String,
    /// The first and the second order factor, each with its range; `roles` is not used.
    pub factor: TableVariable,
    pub second: TableVariable,
    pub table: bool,
    /// One flag per entry of `LIFE_PLOTS`.
    pub plots: [bool; 5],
    /// Index into `LIFE_COMPARE`.
    pub compare: usize,
    pub approximate: bool,
    pub listwise: bool,
    pub include: bool,
}

impl Default for LifeTables {
    fn default() -> Self {
        Self {
            times: Vec::new(),
            intervals: vec![Default::default()],
            status: String::new(),
            event_from: String::new(),
            event_to: String::new(),
            factor: TableVariable::default(),
            second: TableVariable::default(),
            table: true,
            plots: [false; 5],
            compare: 0,
            approximate: false,
            listwise: false,
            include: false,
        }
    }
}

impl LifeTables {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.times.is_empty() {
            return Err("add at least one time variable".into());
        }
        let status = self.status.trim();
        if status.is_empty() {
            return Err("choose the status variable".into());
        }
        let (factor, second) = (self.factor.name.trim(), self.second.name.trim());
        if factor.is_empty() && !second.is_empty() {
            return Err("choose the first factor before the second".into());
        }
        let used = [status, factor, second].map(String::from);
        numeric(data, self.times.iter().chain(&used))?;
        for f in [factor, second].into_iter().filter(|f| !f.is_empty()) {
            if self.times.iter().any(|t| t.eq_ignore_ascii_case(f)) {
                return Err(format!("{f} cannot be both a time variable and a factor"));
            }
        }
        if !second.is_empty() && factor.eq_ignore_ascii_case(second) {
            return Err(format!(
                "{factor} cannot be both the first and the second factor"
            ));
        }
        let mut text = format!("SURVIVAL TABLES={}", self.times.join(" "));
        for f in [&self.factor, &self.second] {
            if !f.name.trim().is_empty() {
                let (min, max) = f.range()?;
                text.push_str(&format!(" BY {}({min},{max})", f.name.trim()));
            }
        }
        text.push_str(&format!("\n  /INTERVAL={}", self.interval_syntax()?));
        let from = parse(&self.event_from, "the event value")?;
        let event = match self.event_to.trim() {
            "" => number(from),
            to => {
                let to = parse(to, "the end of the event range")?;
                if from > to {
                    return Err("the event range starts above its end".into());
                }
                format!("{},{}", number(from), number(to))
            }
        };
        text.push_str(&format!("\n  /STATUS={status}({event})"));
        if !self.table {
            text.push_str("\n  /PRINT=NOTABLE");
        }
        let plots = keywords(&LIFE_PLOTS, &self.plots);
        if !plots.is_empty() {
            text.push_str(&format!("\n  /PLOTS({})", plots.join(" ")));
        }
        if self.compare > 0 {
            if factor.is_empty() {
                return Err("comparing groups needs a first factor".into());
            }
            text.push_str("\n  /COMPARE");
            let calculate: Vec<&str> = [
                (self.approximate, "APPROXIMATE"),
                (self.compare == 2, "PAIRWISE"),
            ]
            .into_iter()
            .filter_map(|(on, k)| on.then_some(k))
            .collect();
            if !calculate.is_empty() {
                text.push_str(&format!("\n  /CALCULATE={}", calculate.join(" ")));
            }
        }
        if self.listwise || self.include {
            let missing = [(self.listwise, "LISTWISE"), (self.include, "INCLUDE")]
                .into_iter()
                .filter_map(|(on, k)| on.then_some(k))
                .collect::<Vec<_>>();
            text.push_str(&format!("\n  /MISSING={}", missing.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// `THRU end BY width ...`, checked: ends ascending above 0, widths above 0.
    fn interval_syntax(&self) -> Result<String, String> {
        let mut parts = Vec::new();
        let mut last = 0.0;
        for (i, (end, width)) in self.intervals.iter().enumerate() {
            let row = i + 1;
            let end = parse(end, &format!("the end of interval row {row}"))?;
            let width = parse(width, &format!("the width of interval row {row}"))?;
            if end <= last {
                return Err(format!(
                    "interval row {row} must end above {}",
                    number(last)
                ));
            }
            if width <= 0.0 {
                return Err(format!("the width of interval row {row} must be above 0"));
            }
            last = end;
            parts.push(format!("THRU {} BY {}", number(end), number(width)));
        }
        Ok(parts.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, v};

    fn km() -> KaplanMeier {
        KaplanMeier {
            time: "age".into(),
            status: "q5_1".into(),
            event_values: "1".into(),
            ..Default::default()
        }
    }

    fn life() -> LifeTables {
        LifeTables {
            times: v(&["age"]),
            intervals: vec![("60".into(), "10".into())],
            status: "q5_1".into(),
            event_from: "1".into(),
            ..Default::default()
        }
    }

    #[test]
    fn kaplan_meier_checks_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut KaplanMeier)| {
            let mut d = km();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.time.clear()), "choose the time variable");
        assert_eq!(err(&|d| d.status.clear()), "choose the status variable");
        assert_eq!(
            err(&|d| d.time = "plan".into()),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| d.event_values.clear()),
            "give the values that mark the event"
        );
        assert_eq!(
            err(&|d| {
                d.event_range = true;
                d.event_from = "3".into();
                d.event_to = "2".into();
            }),
            "the event range starts above its end"
        );
        assert_eq!(
            err(&|d| {
                d.status = "plan".into();
                d.event_range = true;
            }),
            "plan is a string: give its event values, not a range"
        );
        assert_eq!(
            err(&|d| d.percentiles = "50 100".into()),
            "percentiles lie above 0 and below 100"
        );
        assert_eq!(
            err(&|d| {
                d.factor = "gender".into();
                d.tests = [false; 3];
            }),
            "check at least one test of the factor levels"
        );
        assert_eq!(
            err(&|d| {
                d.factor = "gender".into();
                d.compare = 1;
            }),
            "comparing within each stratum needs a strata variable"
        );
        assert_eq!(
            err(&|d| {
                d.save = [true; 4];
                d.save_names[0] = "age".into();
            }),
            "age is already a variable"
        );
        assert_eq!(
            err(&|d| {
                d.save = [true; 4];
                d.save_names[1] = "s".into();
                d.save_names[2] = "S".into();
            }),
            "S names two saved variables"
        );
        // the factor's options are not written without a factor
        let mut d = km();
        d.tests = [false; 3];
        d.trend = true;
        assert_eq!(d.syntax(&data).unwrap(), "KM age\n  /STATUS=q5_1 EVENT(1).");
        // pairwise tests ignore a trend, so it is not written
        let mut d = km();
        d.factor = "gender".into();
        d.compare = 2;
        d.trend = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "KM age BY gender\n  /STATUS=q5_1 EVENT(1)\n  /COMPARE=PAIRWISE POOLED."
        );
    }

    #[test]
    fn kaplan_meier_writes_a_string_status_and_percentiles() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let mut d = km();
        d.status = "plan".into();
        d.event_values = "BASIC, PREMIUM".into();
        d.quartiles = true;
        d.percentiles = "90 50 10".into();
        d.table = false;
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "KM age\n  /STATUS=plan EVENT('BASIC' 'PREMIUM')\n  /PRINT=MEAN\n  \
             /PERCENTILES=(10 25 50 75 90)."
        );
        assert!(run_in(&mut session, &syntax) >= 2);
    }

    #[test]
    fn life_tables_check_what_the_command_would_reject() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut LifeTables)| {
            let mut d = life();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(err(&|d| d.times.clear()), "add at least one time variable");
        assert_eq!(err(&|d| d.status.clear()), "choose the status variable");
        assert_eq!(
            err(&|d| d.intervals = vec![Default::default()]),
            "the end of interval row 1 must be a number"
        );
        assert_eq!(
            err(&|d| d.intervals.push(("50".into(), "5".into()))),
            "interval row 2 must end above 60"
        );
        assert_eq!(
            err(&|d| d.intervals[0].1 = "0".into()),
            "the width of interval row 1 must be above 0"
        );
        assert_eq!(
            err(&|d| d.event_to = "0".into()),
            "the event range starts above its end"
        );
        assert_eq!(
            err(&|d| d.second.name = "gender".into()),
            "choose the first factor before the second"
        );
        assert_eq!(
            err(&|d| {
                d.factor.name = "age".into();
            }),
            "age cannot be both a time variable and a factor"
        );
        assert_eq!(
            err(&|d| {
                d.factor = TableVariable {
                    name: "gender".into(),
                    min: "1".into(),
                    max: "2".into(),
                    ..Default::default()
                };
                d.second = d.factor.clone();
            }),
            "gender cannot be both the first and the second factor"
        );
        assert_eq!(
            err(&|d| {
                d.factor.name = "gender".into();
                d.factor.min = "2".into();
                d.factor.max = "1".into();
            }),
            "the smallest category value of gender is above the largest"
        );
        assert_eq!(
            err(&|d| d.compare = 1),
            "comparing groups needs a first factor"
        );
        assert_eq!(
            err(&|d| d.status = "plan".into()),
            "plan is a string; the command needs numbers"
        );
    }
}
