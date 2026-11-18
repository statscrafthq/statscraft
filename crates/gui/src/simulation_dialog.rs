//! The Build Simulation Plan (SIMPLAN) and Run Simulation (SIMRUN) dialogs. The model is typed
//! as one expression per line and written with BEGIN EXPR to a model file, or an existing model
//! file is used. Options left as they are add nothing to the syntax.

use crate::dialogs::whole;
use crate::syntax_text::{number, quote};
use statscraft_data::{Dataset, is_valid_name};

/// A distribution of an input: the keyword, the label and the parameters (keyword, label).
pub type SimDistribution = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// The distributions of an input. The first entry fits the distribution to the field of the
/// same name in the data.
pub const SIM_DISTRIBUTIONS: [SimDistribution; 13] = [
    ("", "Fit to the data", &[]),
    (
        "NORMAL",
        "Normal",
        &[("MEAN", "Mean"), ("STDDEV", "Std. dev.")],
    ),
    ("UNIFORM", "Uniform", &[("MIN", "Min"), ("MAX", "Max")]),
    (
        "TRIANGULAR",
        "Triangular",
        &[("MIN", "Min"), ("MAX", "Max"), ("MODE", "Mode")],
    ),
    (
        "LNORMAL",
        "Lognormal",
        &[("A", "Median"), ("B", "Std. dev. of the log")],
    ),
    ("EXP", "Exponential", &[("SCALE", "Rate")]),
    ("GAMMA", "Gamma", &[("SHAPE", "Shape"), ("SCALE", "Rate")]),
    (
        "BETA",
        "Beta",
        &[("SHAPE1", "Shape 1"), ("SHAPE2", "Shape 2")],
    ),
    ("WEIBULL", "Weibull", &[("A", "Scale"), ("B", "Shape")]),
    ("BERNOULLI", "Bernoulli", &[("PROB", "Probability")]),
    (
        "BINOM",
        "Binomial",
        &[("N", "Trials"), ("PROB", "Probability")],
    ),
    ("POISSON", "Poisson", &[("MEAN", "Mean")]),
    (
        "CATEGORICAL",
        "Categorical",
        &[("CATS", "Values"), ("PROBS", "Probabilities")],
    ),
];

/// Whether the distribution can be cut with MINVAL and MAXVAL: the bounded ones cannot.
pub fn can_cut(keyword: &str) -> bool {
    !matches!(
        keyword,
        "BERNOULLI" | "CATEGORICAL" | "TRIANGULAR" | "UNIFORM"
    )
}

/// One simulated input.
#[derive(Clone, PartialEq, Debug)]
pub struct SimInputRow {
    pub name: String,
    /// An index of `SIM_DISTRIBUTIONS`.
    pub dist: usize,
    pub params: [String; 3],
    /// Cut the distribution below and above; empty: not cut.
    pub min: String,
    pub max: String,
}

impl Default for SimInputRow {
    fn default() -> Self {
        Self {
            name: String::new(),
            dist: 1,
            params: Default::default(),
            min: String::new(),
            max: String::new(),
        }
    }
}

/// An input with a fixed value.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct FixedRow {
    pub name: String,
    pub value: String,
}

/// The correlation of two simulated inputs.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct CorrelationRow {
    pub first: String,
    pub second: String,
    pub value: String,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Simulation {
    Plan(Box<SimPlan>),
    Run(Box<SimRun>),
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct SimPlan {
    /// One `target = expression` per line.
    pub model: String,
    /// Written when expressions are typed; otherwise an existing model file; empty: no model.
    pub model_file: String,
    pub inputs: Vec<SimInputRow>,
    pub fixed: Vec<FixedRow>,
    pub correlations: Vec<CorrelationRow>,
    /// Names to report; empty: every target of the model.
    pub targets: String,
    /// Empty: 100000.
    pub max_cases: String,
    /// Stop early when the confidence interval of the mean of this target is at most as wide
    /// as the width; both empty: no early stop.
    pub stop_target: String,
    pub stop_width: String,
    pub plan_file: String,
    /// Leave the plan out of the output.
    pub quiet: bool,
}

impl Default for Simulation {
    fn default() -> Self {
        Simulation::Plan(Default::default())
    }
}

/// Where the cases are saved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SaveCases {
    #[default]
    No,
    Active,
    File,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SimRun {
    pub plan_file: String,
    /// Empty: the command's own.
    pub seed: String,
    pub pdf: bool,
    pub cdf: bool,
    pub scatter: bool,
    pub boxplot: bool,
    pub tornado_corr: bool,
    pub tornado_delta: bool,
    pub delta: String,
    pub tornado_variance: bool,
    pub descriptives: bool,
    /// Empty: 95.
    pub level: String,
    pub percentiles: bool,
    /// Percentages such as 5 50 95; empty: the command's own.
    pub ptiles: String,
    pub associations: bool,
    pub save: SaveCases,
    pub out_file: String,
}

impl Default for SimRun {
    fn default() -> Self {
        Self {
            plan_file: String::new(),
            seed: String::new(),
            pdf: true,
            cdf: false,
            scatter: false,
            boxplot: false,
            tornado_corr: true,
            tornado_delta: false,
            delta: "1".into(),
            tornado_variance: false,
            descriptives: true,
            level: String::new(),
            percentiles: false,
            ptiles: String::new(),
            associations: false,
            save: SaveCases::No,
            out_file: String::new(),
        }
    }
}

impl Simulation {
    pub fn title(&self) -> &'static str {
        match self {
            Simulation::Plan(_) => "Build Simulation Plan",
            Simulation::Run(_) => "Run Simulation",
        }
    }

    pub fn reset(&self) -> Self {
        match self {
            Simulation::Plan(_) => Simulation::Plan(Default::default()),
            Simulation::Run(_) => Simulation::Run(Default::default()),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            Simulation::Plan(d) => d.syntax(data),
            Simulation::Run(d) => d.syntax(),
        }
    }
}

/// A finite number the user typed.
fn finite(text: &str, what: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{what} must be a number"))
}

/// Numbers separated by spaces or commas, at least one.
fn list(text: &str, what: &str) -> Result<String, String> {
    let items: Vec<&str> = text
        .split([' ', ',', ';'])
        .filter(|t| !t.is_empty())
        .collect();
    if items.is_empty() {
        return Err(format!("{what} needs at least one number"));
    }
    let numbers: Result<Vec<String>, String> =
        items.iter().map(|t| finite(t, what).map(number)).collect();
    Ok(numbers?.join(" "))
}

fn name(text: &str, what: &str) -> Result<String, String> {
    let text = text.trim();
    if is_valid_name(text) {
        Ok(text.to_string())
    } else {
        Err(format!("{what}: {text:?} is not a valid variable name"))
    }
}

impl SimInputRow {
    fn syntax(&self) -> Result<String, String> {
        let who = format!("input {}", self.name.trim());
        let mut text = format!("  /SIMINPUT INPUT={}", name(&self.name, "an input")?);
        let (keyword, _, params) = SIM_DISTRIBUTIONS[self.dist];
        if keyword.is_empty() {
            return Ok(text);
        }
        let cut = !self.min.trim().is_empty() || !self.max.trim().is_empty();
        if cut && !can_cut(keyword) {
            return Err(format!("{who}: a {keyword} distribution cannot be cut"));
        }
        let mut given = Vec::new();
        for ((p, label), value) in params.iter().zip(&self.params) {
            let what = format!("{who}: {label}");
            let value = if keyword == "CATEGORICAL" {
                list(value, &what)?
            } else {
                number(finite(value, &what)?)
            };
            given.push(format!("{p}={value}"));
        }
        text += &format!(" TYPE=MANUAL DISTRIBUTION={keyword}({})", given.join(" "));
        for (word, value) in [("MINVAL", &self.min), ("MAXVAL", &self.max)] {
            if !value.trim().is_empty() {
                let n = finite(value, &format!("{who}: {word}"))?;
                text += &format!(" {word}={}", number(n));
            }
        }
        Ok(text)
    }
}

impl SimPlan {
    /// The names of the simulated and fixed inputs, in order.
    fn input_names(&self) -> Vec<String> {
        let simulated = self.inputs.iter().map(|r| r.name.trim());
        let fixed = self.fixed.iter().map(|r| r.name.trim());
        simulated.chain(fixed).map(str::to_string).collect()
    }

    /// The BEGIN EXPR block of the typed expressions; empty text when none are typed.
    fn model_block(&self, data: &Dataset) -> Result<String, String> {
        let file = self.model_file.trim();
        let mut computes = Vec::new();
        for line in self.model.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let line = line.trim_end_matches('.').trim();
            let body = match line.get(..8) {
                Some(head) if head.eq_ignore_ascii_case("COMPUTE ") => line[8..].trim(),
                _ => line,
            };
            if !body.contains('=') {
                return Err(format!("the model line {line:?} needs target = expression"));
            }
            computes.push(format!("COMPUTE {body}."));
        }
        if computes.is_empty() {
            return Ok(String::new());
        }
        if file.is_empty() {
            return Err("choose the model file to write the expressions to".into());
        }
        // NUMERIC names the inputs the data lacks, and only those
        let mut new: Vec<String> = Vec::new();
        for n in self.input_names() {
            if data.variable(&n).is_err() && !new.contains(&n) {
                new.push(n);
            }
        }
        let mut text = format!("BEGIN EXPR /OUTFILE PREPXML={}.\n", quote(file));
        if !new.is_empty() {
            text += &format!("NUMERIC {}.\n", new.join(" "));
        }
        text += &computes.join("\n");
        text += "\nEND EXPR.\n";
        Ok(text)
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.inputs.is_empty() {
            return Err("add at least one simulated input".into());
        }
        let plan = self.plan_file.trim();
        if plan.is_empty() {
            return Err("choose the plan file to write".into());
        }
        let mut text = self.model_block(data)?;
        text += "SIMPLAN CREATE";
        let file = self.model_file.trim();
        if !file.is_empty() {
            text += &format!(" /MODEL FILE={}", quote(file));
        }
        let targets: Vec<String> = self
            .targets
            .split_whitespace()
            .map(|t| name(t, "a target"))
            .collect::<Result<_, _>>()?;
        if !targets.is_empty() {
            text += &format!("\n  /TARGETOPTS TARGET={}", targets.join(" "));
        }
        for row in &self.inputs {
            text += &format!("\n{}", row.syntax()?);
        }
        for row in &self.fixed {
            let n = name(&row.name, "a fixed input")?;
            let value = finite(&row.value, &format!("the value of {n}"))?;
            text += &format!("\n  /FIXEDINPUT INPUT={n} VALUE={}", number(value));
        }
        text += &self.correlations_text()?;
        let max = whole(&self.max_cases, 1, "the number of cases")?;
        let stop = (self.stop_target.trim(), self.stop_width.trim());
        if max.is_some() || stop != ("", "") {
            text += "\n  /STOPCRITERIA";
            if let Some(n) = max {
                text += &format!(" MAXCASES={n}");
            }
            if stop != ("", "") {
                if stop.0.is_empty() || stop.1.is_empty() {
                    return Err("an early stop needs both the target and the width".into());
                }
                let width = finite(stop.1, "the width of the confidence interval")?;
                text += &format!(
                    " TARGET={} TYPE=MEAN(THRESH={} UNIT=VALUE)",
                    name(stop.0, "the stopping target")?,
                    number(width)
                );
            }
        }
        text += &format!("\n  /PLAN FILE={}", quote(plan));
        if self.quiet {
            text += " DISPLAY=NO";
        }
        text += ".";
        Ok(text)
    }

    /// The pairs as a lower triangle over the inputs they name; other pairs are uncorrelated.
    fn correlations_text(&self) -> Result<String, String> {
        if self.correlations.is_empty() {
            return Ok(String::new());
        }
        let mut order: Vec<String> = Vec::new();
        let mut pairs = Vec::new();
        for row in &self.correlations {
            let a = name(&row.first, "a correlation")?;
            let b = name(&row.second, "a correlation")?;
            if a == b {
                return Err(format!("{a} cannot be correlated with itself"));
            }
            for n in [&a, &b] {
                if !self.inputs.iter().any(|r| r.name.trim() == n) {
                    return Err(format!("{n} in the correlations is not a simulated input"));
                }
                if !order.contains(n) {
                    order.push(n.clone());
                }
            }
            let r = finite(&row.value, &format!("the correlation of {a} and {b}"))?;
            if !(-1.0..=1.0).contains(&r) {
                return Err(format!("the correlation of {a} and {b} is from -1 to 1"));
            }
            pairs.push((a, b, r));
        }
        let at = |n: &str| order.iter().position(|o| o == n).expect("listed above");
        let k = order.len();
        let mut matrix = vec![vec![0.0; k]; k];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        for (a, b, r) in pairs {
            let (i, j) = (at(&a), at(&b));
            matrix[i.max(j)][i.min(j)] = r;
        }
        let rows: Vec<String> = matrix
            .iter()
            .enumerate()
            .map(|(i, row)| {
                row[..=i]
                    .iter()
                    .map(|r| number(*r))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect();
        Ok(format!(
            "\n  /CORRELATIONS VARORDER={} CORRMATRIX={}",
            order.join(" "),
            rows.join("; ")
        ))
    }
}

impl SimRun {
    pub fn syntax(&self) -> Result<String, String> {
        let plan = self.plan_file.trim();
        if plan.is_empty() {
            return Err("choose the plan file to run".into());
        }
        let mut text = format!("SIMRUN /PLAN FILE={}", quote(plan));
        if let Some(seed) = whole(&self.seed, 1, "the seed")? {
            if seed > 2_147_483_646 {
                return Err("the seed must be at most 2147483646".into());
            }
            text += &format!("\n  /CRITERIA SEED={seed}");
        }
        match (self.pdf, self.cdf) {
            (true, false) => {}
            (false, false) => text += "\n  /DISTRIBUTION DISPLAY=NONE",
            (pdf, _) => {
                let pdf = if pdf { "PDF " } else { "" };
                text += &format!("\n  /DISTRIBUTION DISPLAY={pdf}CDF");
            }
        }
        if self.scatter {
            text += "\n  /SCATTERPLOT DISPLAY=YES";
        }
        if self.boxplot {
            text += "\n  /BOXPLOT DISPLAY=YES";
        }
        let mut kinds = Vec::new();
        if self.tornado_corr {
            kinds.push("CORR".to_string());
        }
        if self.tornado_delta {
            let k = finite(&self.delta, "the number of standard deviations")?;
            if !(k > 0.0 && k <= 10.0) {
                return Err("the number of standard deviations is above 0 and at most 10".into());
            }
            kinds.push(format!("DELTASTDDEV({})", number(k)));
        }
        if self.tornado_variance {
            kinds.push("VARIANCE".into());
        }
        if kinds.is_empty() {
            text += "\n  /TORNADO DISPLAY=NO";
        } else if kinds != ["CORR"] {
            text += &format!("\n  /TORNADO TYPE={}", kinds.join(" "));
        }
        let level = self.level.trim();
        let mut print = Vec::new();
        if !self.descriptives {
            print.push("DESCRIPTIVES=NO".to_string());
        } else if !level.is_empty() {
            let l = finite(level, "the confidence level")?;
            if !(50.0..100.0).contains(&l) {
                return Err("the confidence level is from 50 to below 100".into());
            }
            print.push(format!("DESCRIPTIVES=YES(CILEVEL={})", number(l)));
        }
        if self.percentiles {
            let tiles = self.ptiles.trim();
            if tiles.is_empty() {
                print.push("PERCENTILES=YES".into());
            } else {
                let values = list(tiles, "the percentiles")?;
                let ok = values
                    .split(' ')
                    .all(|v| v.parse::<f64>().is_ok_and(|v| (0.0..=100.0).contains(&v)));
                if !ok {
                    return Err("the percentiles are percentages from 0 to 100".into());
                }
                print.push(format!("PERCENTILES=YES(PTILES={values})"));
            }
        }
        if self.associations {
            print.push("ASSOCIATIONS=YES".into());
        }
        if !print.is_empty() {
            text += &format!("\n  /PRINT {}", print.join(" "));
        }
        match self.save {
            SaveCases::No => {}
            SaveCases::Active => text += "\n  /OUTFILE FILE=*",
            SaveCases::File => {
                let file = self.out_file.trim();
                if file.is_empty() {
                    return Err("choose the file to save the cases to".into());
                }
                text += &format!("\n  /OUTFILE FILE={}", quote(file));
            }
        }
        text += ".";
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Dataset {
        let mut s = statscraft_engine::session::Session::default();
        s.run("DATA LIST LIST /x.\nBEGIN DATA\n1\n2\nEND DATA.", None)
            .unwrap();
        s.dataset().unwrap().clone()
    }

    fn normal(name: &str, mean: &str) -> SimInputRow {
        SimInputRow {
            name: name.into(),
            params: [mean.into(), "2".into(), String::new()],
            ..Default::default()
        }
    }

    #[test]
    fn plan_text() {
        let d = SimPlan {
            model: "margin = units * (price - cost)\nCOMPUTE big = margin > 100.".into(),
            model_file: "it's.json".into(),
            inputs: vec![
                SimInputRow {
                    name: "units".into(),
                    dist: 4,
                    params: ["1000".into(), "0.2".into(), String::new()],
                    min: "900".into(),
                    ..Default::default()
                },
                SimInputRow {
                    name: "cost".into(),
                    dist: 12,
                    params: ["1 2 3".into(), "0.2, 0.3, 0.5".into(), String::new()],
                    ..Default::default()
                },
                SimInputRow {
                    name: "x".into(),
                    dist: 0,
                    ..Default::default()
                },
            ],
            fixed: vec![FixedRow {
                name: "price".into(),
                value: "12".into(),
            }],
            correlations: vec![CorrelationRow {
                first: "cost".into(),
                second: "units".into(),
                value: "-0.4".into(),
            }],
            targets: "margin".into(),
            max_cases: "5000".into(),
            stop_target: "margin".into(),
            stop_width: "50".into(),
            plan_file: "p.splan".into(),
            quiet: true,
        };
        assert_eq!(
            d.syntax(&data()).unwrap(),
            "BEGIN EXPR /OUTFILE PREPXML='it''s.json'.\nNUMERIC units cost price.\n\
             COMPUTE margin = units * (price - cost).\nCOMPUTE big = margin > 100.\nEND EXPR.\n\
             SIMPLAN CREATE /MODEL FILE='it''s.json'\n  /TARGETOPTS TARGET=margin\n  \
             /SIMINPUT INPUT=units TYPE=MANUAL DISTRIBUTION=LNORMAL(A=1000 B=0.2) \
             MINVAL=900\n  \
             /SIMINPUT INPUT=cost TYPE=MANUAL DISTRIBUTION=CATEGORICAL(CATS=1 2 3 \
             PROBS=0.2 0.3 0.5)\n  \
             /SIMINPUT INPUT=x\n  /FIXEDINPUT INPUT=price VALUE=12\n  \
             /CORRELATIONS VARORDER=cost units CORRMATRIX=1; -0.4, 1\n  \
             /STOPCRITERIA MAXCASES=5000 TARGET=margin TYPE=MEAN(THRESH=50 UNIT=VALUE)\n  \
             /PLAN FILE='p.splan' DISPLAY=NO."
        );
        let d = SimPlan {
            inputs: vec![normal("a", "0")],
            plan_file: "p.splan".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data()).unwrap(),
            "SIMPLAN CREATE\n  /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=2)\n  \
             /PLAN FILE='p.splan'."
        );
    }

    #[test]
    fn plan_needs() {
        let ok = SimPlan {
            inputs: vec![normal("a", "0")],
            plan_file: "p".into(),
            ..Default::default()
        };
        assert!(SimPlan::default().syntax(&data()).is_err());
        let cut_uniform = SimPlan {
            inputs: vec![SimInputRow {
                dist: 2,
                min: "1".into(),
                params: ["0".into(), "5".into(), String::new()],
                ..normal("a", "0")
            }],
            ..ok.clone()
        };
        assert!(cut_uniform.syntax(&data()).is_err());
        let no_file = SimPlan {
            model: "t = a".into(),
            ..ok.clone()
        };
        assert!(no_file.syntax(&data()).is_err());
        let no_equals = SimPlan {
            model: "a + 1".into(),
            model_file: "m".into(),
            ..ok.clone()
        };
        assert!(no_equals.syntax(&data()).is_err());
        let bad_number = SimPlan {
            inputs: vec![normal("a", "lots")],
            ..ok.clone()
        };
        assert!(bad_number.syntax(&data()).is_err());
        let stray = SimPlan {
            correlations: vec![CorrelationRow {
                first: "a".into(),
                second: "b".into(),
                value: "0.3".into(),
            }],
            ..ok
        };
        assert!(
            stray
                .syntax(&data())
                .unwrap_err()
                .contains("not a simulated input")
        );
    }

    #[test]
    fn run_text() {
        let d = SimRun {
            plan_file: "p.splan".into(),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "SIMRUN /PLAN FILE='p.splan'.");
        let d = SimRun {
            plan_file: "p.splan".into(),
            seed: "4711".into(),
            pdf: false,
            cdf: true,
            scatter: true,
            boxplot: true,
            tornado_delta: true,
            delta: "2".into(),
            tornado_variance: true,
            level: "90".into(),
            percentiles: true,
            ptiles: "5 50 95".into(),
            associations: true,
            save: SaveCases::File,
            out_file: "draws.sav".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "SIMRUN /PLAN FILE='p.splan'\n  /CRITERIA SEED=4711\n  /DISTRIBUTION DISPLAY=CDF\n  \
             /SCATTERPLOT DISPLAY=YES\n  /BOXPLOT DISPLAY=YES\n  \
             /TORNADO TYPE=CORR DELTASTDDEV(2) VARIANCE\n  \
             /PRINT DESCRIPTIVES=YES(CILEVEL=90) PERCENTILES=YES(PTILES=5 50 95) \
             ASSOCIATIONS=YES\n  \
             /OUTFILE FILE='draws.sav'."
        );
        let d = SimRun {
            plan_file: "p".into(),
            pdf: false,
            tornado_corr: false,
            descriptives: false,
            save: SaveCases::Active,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "SIMRUN /PLAN FILE='p'\n  /DISTRIBUTION DISPLAY=NONE\n  /TORNADO DISPLAY=NO\n  \
             /PRINT DESCRIPTIVES=NO\n  /OUTFILE FILE=*."
        );
        assert!(SimRun::default().syntax().is_err());
        let bad = SimRun {
            plan_file: "p".into(),
            seed: "0".into(),
            ..Default::default()
        };
        assert!(bad.syntax().is_err());
    }
}
