//! The Control Charts dialog (SPCHART): X-bar charts with the range or the standard deviation,
//! individuals charts with or without the moving range, the p, np, c and u charts of counts,
//! and the process capability chart. Options left as they are add nothing to the syntax, so the
//! command runs with its own defaults.

use crate::dialogs::whole;
use crate::graph_dialogs::line;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{number, value};
use statscraft_data::Dataset;

pub const SPCHART_CHARTS: [(&str, &str); 9] = [
    ("XR", "X-bar and range"),
    ("XS", "X-bar and standard deviation"),
    ("IR", "Individuals and moving range"),
    ("I", "Individuals"),
    ("P", "p: share nonconforming"),
    ("NP", "np: number nonconforming"),
    ("C", "c: defects per subgroup"),
    ("U", "u: defects per unit"),
    ("CPCHART", "Process capability"),
];
pub const XR: usize = 0;
pub const XS: usize = 1;
pub const IR: usize = 2;
pub const I: usize = 3;
pub const P: usize = 4;
pub const NP: usize = 5;
pub const C: usize = 6;
pub const U: usize = 7;
pub const CAPABILITY: usize = 8;

/// How the data of a chart are laid out, as a list of choices per chart.
const VARIABLE_LAYOUTS: [(&str, &str); 2] = [
    ("", "Each case is a unit; a variable names its subgroup"),
    ("", "Each case is a subgroup; one variable per unit"),
];
const CAPABILITY_LAYOUTS: [(&str, &str); 3] = [
    ("", "Each case is a unit; a variable names its subgroup"),
    ("", "Each case is a subgroup; one variable per unit"),
    ("", "Each case is one value"),
];
const ATTRIBUTE_LAYOUTS: [(&str, &str); 2] = [
    ("", "Each case is a unit; a variable names its subgroup"),
    ("", "Each case is a subgroup with its count and size"),
];
const SINGLE_LAYOUT: [(&str, &str); 1] = [("", "Each case is one value")];

/// What a chart's data look like once the layout is chosen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    /// A variable and the variable that gathers the cases into subgroups.
    Units,
    /// One variable per unit of a subgroup, a case being a subgroup.
    Subgroups,
    /// One value per case.
    Single,
    /// A count and a size per case.
    Counts,
}

/// The rules of /RULES, with what each one finds.
pub const SPCHART_RULES: [(&str, &str); 11] = [
    ("UCL", "Beyond the upper control limit"),
    ("R2UPPER", "2 of 3 points past 2 sigma above the center"),
    ("R4UPPER", "4 of 5 points past 1 sigma above the center"),
    ("R8UPPER", "8 points in a row above the center"),
    ("R8LOWER", "8 points in a row below the center"),
    ("R4LOWER", "4 of 5 points past 1 sigma below the center"),
    ("R2LOWER", "2 of 3 points past 2 sigma below the center"),
    ("LCL", "Beyond the lower control limit"),
    ("TRUP", "6 points rising in a row"),
    ("TRDOWN", "6 points falling in a row"),
    ("ALTERNATING", "14 points up and down in turn"),
];

/// The capability and performance indices and the summary values of /STATISTICS.
pub const SPCHART_STATISTICS: [(&str, &str); 17] = [
    ("CP", "Cp"),
    ("CPL", "Cp, lower"),
    ("CPU", "Cp, upper"),
    ("CPK", "Cpk"),
    ("CR", "Capability ratio"),
    ("CPM", "Cpm (needs a target)"),
    ("PP", "Pp"),
    ("PPL", "Pp, lower"),
    ("PPU", "Pp, upper"),
    ("PPK", "Ppk"),
    ("PR", "Performance ratio"),
    ("PPM", "Ppm (needs a target)"),
    ("N", "Number of values"),
    ("MEAN", "Mean"),
    ("STDDEV", "Standard deviation"),
    ("CAPSIGMA", "Capability sigma"),
    ("AZOUT", "Percent outside the limits"),
];

/// The estimators of the capability sigma; the first is the chart's own.
pub const SPCHART_CAPSIGMAS: [(&str, &str); 5] = [
    ("", "The chart's own"),
    ("RBAR", "Mean of the subgroup ranges"),
    ("SBAR", "Mean of the subgroup standard deviations"),
    ("MRBAR", "Mean moving range"),
    ("WITHIN", "Within subgroups"),
];

pub const SPCHART_NORMALS: [(&str, &str); 4] = [
    ("CAPABILITY", "Capability sigma"),
    ("PERFORMANCE", "Standard deviation of all values"),
    ("BOTH", "Both"),
    ("NONE", "None"),
];

pub const SPCHART_REFERENCES: [(&str, &str); 4] = [
    ("", "Specification limits and target"),
    ("SL SIGMAS", "Limits, and the mean with sigmas"),
    ("SIGMAS", "The mean with sigmas"),
    ("NONE", "None"),
];

/// SPCHART: a control chart of a process, or its capability chart.
#[derive(Clone, PartialEq, Debug)]
pub struct ControlCharts {
    /// Index into `SPCHART_CHARTS`, and into the layouts of that chart.
    pub chart: usize,
    pub layout: usize,
    /// The process variable (the status variable of p and np, the defects of c and u).
    pub process: String,
    /// The variable that gathers cases into subgroups.
    pub group: String,
    /// The variables of a subgroup, one per unit.
    pub variables: Vec<String>,
    /// Names the points instead of numbering them by case.
    pub label: String,
    pub count: String,
    /// The size of a subgroup: a variable or one whole number.
    pub size_variable: bool,
    pub size: String,
    pub size_name: String,
    /// The value of the status variable that is nonconforming (true) or conforming (false).
    pub nonconforming: bool,
    pub status_value: String,
    pub xbar_only: bool,
    pub sigmas: String,
    pub minsample: String,
    pub span: String,
    pub rules: [bool; 11],
    /// Names the points in the table of broken rules.
    pub id: String,
    pub statistics: [bool; 17],
    /// Index into `SPCHART_CAPSIGMAS`.
    pub capsigma: usize,
    pub lsl: String,
    pub usl: String,
    pub target: String,
    pub report: bool,
    pub include: bool,
    /// Indices into `SPCHART_NORMALS` and `SPCHART_REFERENCES`.
    pub normal: usize,
    pub reference: usize,
    pub title: String,
    pub subtitle: String,
    pub footnote: String,
}

impl Default for ControlCharts {
    fn default() -> Self {
        Self {
            chart: XR,
            layout: 0,
            process: String::new(),
            group: String::new(),
            variables: Vec::new(),
            label: String::new(),
            count: String::new(),
            size_variable: true,
            size: String::new(),
            size_name: String::new(),
            nonconforming: true,
            status_value: String::new(),
            xbar_only: false,
            sigmas: "3".into(),
            minsample: "2".into(),
            span: "2".into(),
            rules: [false; 11],
            id: String::new(),
            statistics: [false; 17],
            capsigma: 0,
            lsl: String::new(),
            usl: String::new(),
            target: String::new(),
            report: false,
            include: false,
            normal: 0,
            reference: 0,
            title: String::new(),
            subtitle: String::new(),
            footnote: String::new(),
        }
    }
}

/// The statistics that need both limits, and the one-sided ones that need either.
const ONE_LIMIT: [&str; 2] = ["CPK", "PPK"];
const NO_LIMIT: [&str; 6] = ["N", "MEAN", "STDDEV", "CAPSIGMA", "LSL", "USL"];

impl ControlCharts {
    pub fn layouts(&self) -> &'static [(&'static str, &'static str)] {
        match self.chart {
            XR | XS => &VARIABLE_LAYOUTS,
            IR | I => &SINGLE_LAYOUT,
            CAPABILITY => &CAPABILITY_LAYOUTS,
            _ => &ATTRIBUTE_LAYOUTS,
        }
    }

    pub fn shape(&self) -> Shape {
        let layout = self.layout.min(self.layouts().len() - 1);
        match (self.chart, layout) {
            (IR | I, _) | (CAPABILITY, 2) => Shape::Single,
            (CAPABILITY | XR | XS, 1) => Shape::Subgroups,
            (P..=U, 1) => Shape::Counts,
            _ => Shape::Units,
        }
    }

    /// The statuses of p and np are told apart by a value.
    pub fn status(&self) -> bool {
        matches!(self.chart, P | NP) && self.shape() == Shape::Units
    }

    /// Whether points can be named by a variable: not in the capability chart.
    pub fn labelled(&self) -> bool {
        match self.shape() {
            Shape::Units => false,
            Shape::Subgroups | Shape::Single => self.chart != CAPABILITY,
            Shape::Counts => true,
        }
    }

    /// The charts of values have limits, statistics and a capability sigma.
    pub fn of_values(&self) -> bool {
        matches!(self.chart, XR | XS | IR | I | CAPABILITY)
    }

    /// The control charts check rules and draw limits at some number of sigmas.
    pub fn controls(&self) -> bool {
        self.chart != CAPABILITY
    }

    /// The indices into `SPCHART_CAPSIGMAS` that fit the data.
    pub fn capsigmas(&self) -> Vec<usize> {
        match self.shape() {
            Shape::Single => vec![0, 3],
            _ => vec![0, 1, 2, 4],
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let chart = self.chart;
        let key = SPCHART_CHARTS[chart].0;
        let shape = self.shape();
        let need = |slot: &str, what: &str| match slot.trim() {
            "" => Err(format!("set {what}")),
            name => Ok(name.to_string()),
        };
        let mut measured: Vec<String> = Vec::new();
        let mut spec = match shape {
            Shape::Units => {
                let what = match chart {
                    P | NP => "the status variable",
                    C | U => "the defects variable",
                    _ => "the process variable",
                };
                let process = need(&self.process, what)?;
                // the status of p and np may be a string
                if !matches!(chart, P | NP) {
                    measured.push(process.clone());
                }
                format!(
                    "{process} BY {}",
                    need(&self.group, "the subgroup variable")?
                )
            }
            Shape::Subgroups => {
                if self.variables.len() < 2 {
                    return Err("add at least two variables, one per unit of a subgroup".into());
                }
                measured.extend(self.variables.iter().cloned());
                self.variables.join(" ")
            }
            Shape::Single => {
                let process = need(&self.process, "the process variable")?;
                measured.push(process.clone());
                process
            }
            Shape::Counts => {
                let count = need(&self.count, "the count variable")?;
                measured.push(count.clone());
                let size = match self.size_variable {
                    true => {
                        let name = need(&self.size_name, "the size variable")?;
                        measured.push(name.clone());
                        name
                    }
                    false => whole(&self.size, 1, "the subgroup size")?
                        .ok_or("give the subgroup size")?
                        .to_string(),
                };
                format!("COUNT({count}) N({size})")
            }
        };
        numeric(data, &measured)?;
        if self.labelled() && !self.label.trim().is_empty() {
            spec.push_str(&format!(" BY {}", self.label.trim()));
        }
        if self.xbar_only && matches!(chart, XR | XS) {
            spec.push_str(" (XBARONLY)");
        }
        let mut text = format!("SPCHART /{key}={spec}");
        if self.status() {
            let status = self.process.trim();
            let string = data.variable(status).is_ok_and(|v| v.is_string());
            let given = match string {
                true => self.status_value.trim_end(),
                false => self.status_value.trim(),
            };
            if given.is_empty() {
                return Err(
                    "give the value of the status that is nonconforming or conforming".into(),
                );
            }
            let word = if self.nonconforming {
                "NONCONFORM"
            } else {
                "CONFORM"
            };
            text.push_str(&format!("\n  /{word}={}", value(given, string)?));
        }
        if self.controls() {
            let sigmas = parse(&self.sigmas, "the number of sigmas")?;
            if !(sigmas > 1.0 && sigmas <= 10.0) {
                return Err("the number of sigmas lies above 1 and at most 10".into());
            }
            if sigmas != 3.0 {
                text.push_str(&format!("\n  /SIGMAS={}", number(sigmas)));
            }
            let chosen: Vec<&str> = SPCHART_RULES
                .iter()
                .zip(self.rules)
                .filter(|(_, on)| *on)
                .map(|((k, _), _)| *k)
                .collect();
            if chosen.len() == SPCHART_RULES.len() {
                text.push_str("\n  /RULES=ALL");
            } else if !chosen.is_empty() {
                text.push_str(&format!("\n  /RULES={}", chosen.join(" ")));
            }
            if !chosen.is_empty() && !self.id.trim().is_empty() {
                text.push_str(&format!("\n  /ID={}", self.id.trim()));
            }
        }
        if matches!(shape, Shape::Subgroups | Shape::Units) && self.of_values() {
            let least = whole(&self.minsample, 2, "the smallest subgroup")?.unwrap_or(2);
            if least > 100 {
                return Err("the smallest subgroup is at most 100".into());
            }
            if least != 2 {
                text.push_str(&format!("\n  /MINSAMPLE={least}"));
            }
        }
        if shape == Shape::Single {
            let span = whole(&self.span, 2, "the span of the moving range")?.unwrap_or(2);
            if span > 100 {
                return Err("the span of the moving range is at most 100".into());
            }
            if span != 2 {
                text.push_str(&format!("\n  /SPAN={span}"));
            }
        }
        if self.of_values() {
            text.push_str(&self.capability_syntax(chart)?);
        }
        text.push_str(&self.missing_syntax());
        self.finish(text)
    }

    /// /MISSING when it differs from the default: REPORT keeps subgroups of missing labels, and
    /// INCLUDE counts user-missing values.
    fn missing_syntax(&self) -> String {
        let mut words = Vec::new();
        if self.report {
            words.push("REPORT");
        }
        if self.include {
            words.push("INCLUDE");
        }
        match words.is_empty() {
            true => String::new(),
            false => format!("\n  /MISSING={}", words.join(" ")),
        }
    }

    /// The limits, the statistics, the capability sigma and the options of the capability chart.
    fn capability_syntax(&self, chart: usize) -> Result<String, String> {
        let mut text = String::new();
        let limit = |slot: &str, what: &str| match slot.trim() {
            "" => Ok(None),
            t => parse(t, what).map(Some),
        };
        let (lsl, usl, target) = (
            limit(&self.lsl, "the lower specification limit")?,
            limit(&self.usl, "the upper specification limit")?,
            limit(&self.target, "the target")?,
        );
        if chart == CAPABILITY && (lsl.is_none() || usl.is_none()) {
            return Err("the capability chart needs both specification limits".into());
        }
        if let (Some(l), Some(u)) = (lsl, usl)
            && l >= u
        {
            return Err("the lower limit must be below the upper limit".into());
        }
        let both = lsl.is_some() && usl.is_some();
        for ((k, label), on) in SPCHART_STATISTICS.iter().zip(self.statistics) {
            if !on {
                continue;
            }
            let fits = if NO_LIMIT.contains(k) {
                true
            } else if ONE_LIMIT.contains(k) {
                lsl.is_some() || usl.is_some()
            } else {
                both
            };
            if !fits {
                return Err(format!("{label} needs the specification limits"));
            }
            if matches!(*k, "CPM" | "PPM") {
                match (target, lsl, usl) {
                    (None, ..) => return Err(format!("{label} needs a target")),
                    (Some(t), Some(l), Some(u)) if t < l || t > u => {
                        return Err("the target must lie between the limits".into());
                    }
                    _ => {}
                }
            }
        }
        if let Some(l) = lsl {
            text.push_str(&format!("\n  /LSL={}", number(l)));
        }
        if let Some(u) = usl {
            text.push_str(&format!("\n  /USL={}", number(u)));
        }
        if let Some(t) = target {
            text.push_str(&format!("\n  /TARGET={}", number(t)));
        }
        let chosen: Vec<&str> = SPCHART_STATISTICS
            .iter()
            .zip(self.statistics)
            .filter(|(_, on)| *on)
            .map(|((k, _), _)| *k)
            .collect();
        if !chosen.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", chosen.join(" ")));
        }
        if self.capsigmas().contains(&self.capsigma) && self.capsigma > 0 {
            text.push_str(&format!(
                "\n  /CAPSIGMA={}",
                SPCHART_CAPSIGMAS[self.capsigma].0
            ));
        }
        if chart == CAPABILITY {
            if self.normal > 0 {
                text.push_str(&format!("\n  /NORMAL={}", SPCHART_NORMALS[self.normal].0));
            }
            if self.reference > 0 {
                text.push_str(&format!(
                    "\n  /REFERENCE={}",
                    SPCHART_REFERENCES[self.reference].0
                ));
            }
        }
        Ok(text)
    }

    /// The titles, then the final stop.
    fn finish(&self, mut text: String) -> Result<String, String> {
        for (keyword, slot, what) in [
            ("TITLE", &self.title, "title"),
            ("SUBTITLE", &self.subtitle, "subtitle"),
            ("FOOTNOTE", &self.footnote, "footnote"),
        ] {
            if let Some(quoted) = line(slot, what)? {
                text.push_str(&format!("\n  /{keyword}={quoted}"));
            }
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

    fn xr() -> ControlCharts {
        ControlCharts {
            process: "age".into(),
            group: "region".into(),
            ..Default::default()
        }
    }

    #[test]
    fn variable_charts_write_their_data_options_and_limits() {
        let data = data();
        assert_eq!(xr().syntax(&data).unwrap(), "SPCHART /XR=age BY region.");
        let d = ControlCharts {
            chart: XS,
            xbar_only: true,
            sigmas: "2.5".into(),
            minsample: "3".into(),
            rules: [true; 11],
            id: "resp_id".into(),
            statistics: {
                let mut s = [false; 17];
                s[0] = true;
                s[3] = true;
                s[13] = true;
                s
            },
            capsigma: 4,
            lsl: "10".into(),
            usl: "90".into(),
            target: "50".into(),
            report: true,
            include: true,
            title: "Ages".into(),
            ..xr()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "SPCHART /XS=age BY region (XBARONLY)\n  /SIGMAS=2.5\n  /RULES=ALL\n  /ID=resp_id\n  \
             /MINSAMPLE=3\n  /LSL=10\n  /USL=90\n  /TARGET=50\n  /STATISTICS=CP CPK MEAN\n  \
             /CAPSIGMA=WITHIN\n  /MISSING=REPORT INCLUDE\n  /TITLE='Ages'."
        );
        let subgroups = ControlCharts {
            layout: 1,
            variables: v(&["q1_satisfaction", "q2_recommend"]),
            label: "resp_id".into(),
            rules: {
                let mut r = [false; 11];
                r[0] = true;
                r[7] = true;
                r
            },
            ..Default::default()
        };
        assert_eq!(
            subgroups.syntax(&data).unwrap(),
            "SPCHART /XR=q1_satisfaction q2_recommend BY resp_id\n  /RULES=UCL LCL."
        );
    }

    #[test]
    fn individuals_and_attribute_charts_write_what_they_need() {
        let data = data();
        let single = ControlCharts {
            chart: IR,
            process: "age".into(),
            span: "3".into(),
            capsigma: 3,
            ..Default::default()
        };
        assert_eq!(
            single.syntax(&data).unwrap(),
            "SPCHART /IR=age\n  /SPAN=3\n  /CAPSIGMA=MRBAR."
        );
        let p = ControlCharts {
            chart: P,
            process: "q5_1".into(),
            group: "region".into(),
            status_value: "0".into(),
            ..Default::default()
        };
        assert_eq!(
            p.syntax(&data).unwrap(),
            "SPCHART /P=q5_1 BY region\n  /NONCONFORM=0."
        );
        assert!(
            ControlCharts {
                status_value: String::new(),
                ..p.clone()
            }
            .syntax(&data)
            .is_err()
        );
        let counts = ControlCharts {
            chart: NP,
            layout: 1,
            count: "q1_satisfaction".into(),
            size_variable: false,
            size: "5".into(),
            label: "resp_id".into(),
            ..Default::default()
        };
        assert_eq!(
            counts.syntax(&data).unwrap(),
            "SPCHART /NP=COUNT(q1_satisfaction) N(5) BY resp_id."
        );
    }

    #[test]
    fn the_capability_chart_needs_limits_and_checks_its_statistics() {
        let data = data();
        let cap = ControlCharts {
            chart: CAPABILITY,
            process: "age".into(),
            layout: 2,
            ..Default::default()
        };
        assert!(
            cap.syntax(&data)
                .unwrap_err()
                .contains("both specification")
        );
        let d = ControlCharts {
            lsl: "10".into(),
            usl: "90".into(),
            normal: 2,
            reference: 1,
            ..cap.clone()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "SPCHART /CPCHART=age\n  /LSL=10\n  /USL=90\n  /NORMAL=BOTH\n  /REFERENCE=SL SIGMAS."
        );
        let mut statistics = [false; 17];
        statistics[5] = true;
        let cpm = ControlCharts {
            statistics,
            ..d.clone()
        };
        assert!(cpm.syntax(&data).unwrap_err().contains("needs a target"));
        let backwards = ControlCharts {
            lsl: "95".into(),
            ..d
        };
        assert!(backwards.syntax(&data).is_err());
    }
}
