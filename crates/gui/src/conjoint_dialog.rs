//! The conjoint dialogs: Data > Orthogonal Design > Generate (ORTHOPLAN) and Display (PLANCARDS),
//! and Analyze > Conjoint (CONJOINT). Options left as they are add nothing to the syntax, so the
//! command runs with its own defaults.

use crate::dialogs::whole;
use crate::syntax_text::{number, quote};
use statscraft_data::{Dataset, is_valid_name};

/// The variables a plan holds besides its factors.
fn is_bookkeeping(name: &str) -> bool {
    ["STATUS_", "CARD_"]
        .iter()
        .any(|k| name.eq_ignore_ascii_case(k))
}

/// A factor name: valid, and not one the plan keeps for itself.
fn factor_name(text: &str, what: &str) -> Result<String, String> {
    let name = text.trim();
    if name.is_empty() {
        return Err(format!("give {what} a name"));
    }
    if !is_valid_name(name) || is_bookkeeping(name) {
        return Err(format!("{name} cannot name a factor"));
    }
    Ok(name.to_string())
}

/// The number of different values of each factor of a plan, in the experimental profiles
/// (those with STATUS_ 0, or all of them without STATUS_).
pub fn plan_counts(data: &Dataset) -> Vec<(String, usize)> {
    let status: Option<Vec<Option<f64>>> = data
        .variable_names()
        .find(|n| n.eq_ignore_ascii_case("STATUS_"))
        .and_then(|n| data.variable(n).ok()?.numeric().ok())
        .map(|values| values.iter().collect());
    let mut out = Vec::new();
    for name in data.variable_names().filter(|n| !is_bookkeeping(n)) {
        let Ok(var) = data.variable(name) else {
            continue;
        };
        let Ok(values) = var.numeric() else { continue };
        let mut seen: Vec<f64> = values
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                status
                    .as_ref()
                    .is_none_or(|s| s.get(*i).copied().flatten() == Some(0.0))
            })
            .filter_map(|(_, v)| v)
            .filter(|v| !var.is_user_missing(*v))
            .collect();
        seen.sort_by(f64::total_cmp);
        seen.dedup();
        out.push((name.to_string(), seen.len()));
    }
    out
}

/// A level of a factor to generate: its value and an optional label.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Level {
    pub value: String,
    pub label: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlanFactor {
    pub name: String,
    pub label: String,
    pub levels: Vec<Level>,
}

impl Default for PlanFactor {
    fn default() -> Self {
        Self {
            name: String::new(),
            label: String::new(),
            levels: ["1", "2"]
                .map(|v| Level {
                    value: v.into(),
                    label: String::new(),
                })
                .into(),
        }
    }
}

/// What ORTHOPLAN does with the plan it makes.
pub const PLAN_OUTPUTS: [&str; 3] = [
    "Replace the active data",
    "Save to a data file",
    "Add to the active data (its variables must be these factors)",
];

/// ORTHOPLAN: a plan of profiles from the factors and their levels.
#[derive(Clone, PartialEq, Debug)]
pub struct OrthogonalPlan {
    pub factors: Vec<PlanFactor>,
    /// The factor being edited.
    pub at: usize,
    pub minimum: String,
    pub holdout: String,
    /// Holdouts mixed in among the plan's profiles, not after them.
    pub mixed: bool,
    /// Empty for the session's own seed.
    pub seed: String,
    /// Index into `PLAN_OUTPUTS`.
    pub output: usize,
    pub file: String,
}

impl Default for OrthogonalPlan {
    fn default() -> Self {
        Self {
            factors: vec![PlanFactor::default()],
            at: 0,
            minimum: String::new(),
            holdout: String::new(),
            mixed: false,
            seed: String::new(),
            output: 0,
            file: String::new(),
        }
    }
}

impl OrthogonalPlan {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let mut seen: Vec<String> = Vec::new();
        let mut items = Vec::new();
        let (mut total, mut full) = (0, 1.0_f64);
        for (i, f) in self.factors.iter().enumerate() {
            let name = factor_name(&f.name, &format!("factor {}", i + 1))?;
            if seen.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                return Err(format!("{name} is named twice"));
            }
            if !(2..=9).contains(&f.levels.len()) {
                return Err(format!("{name} needs 2 to 9 levels"));
            }
            let mut values = Vec::new();
            let mut text = name.clone();
            if !f.label.trim().is_empty() {
                text.push_str(&format!(" {}", quote(f.label.trim())));
            }
            text.push_str(" (");
            for (k, level) in f.levels.iter().enumerate() {
                let value = level
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("level {} of {name} needs a number", k + 1))?;
                if values.contains(&value) {
                    return Err(format!("{name} has the level {} twice", number(value)));
                }
                values.push(value);
                if k > 0 {
                    text.push(' ');
                }
                text.push_str(&number(value));
                if !level.label.trim().is_empty() {
                    text.push_str(&format!(" {}", quote(level.label.trim())));
                }
            }
            text.push(')');
            total += values.len();
            full *= values.len() as f64;
            seen.push(name);
            items.push(text);
        }
        if total > 90 {
            return Err("the factors have more than 90 levels in all".into());
        }
        let minimum = whole(&self.minimum, 1, "the minimum number of cases")?;
        let holdout = whole(&self.holdout, 1, "the number of holdout cases")?;
        for (n, what) in [(minimum, "the minimum"), (holdout, "the holdout count")] {
            if n.is_some_and(|n| f64::from(n) > full) {
                return Err(format!(
                    "{what} is more than the {full} combinations of the levels"
                ));
            }
        }
        let mut text = String::new();
        if let Some(seed) = whole(&self.seed, 1, "the seed")? {
            if seed > 1_999_999_999 {
                return Err("the seed must be at most 1999999999".into());
            }
            text.push_str(&format!("SET SEED={seed}.\n"));
        }
        text.push_str(&format!("ORTHOPLAN\n  /FACTORS={}", items.join("\n    ")));
        if let Some(n) = minimum {
            text.push_str(&format!("\n  /MINIMUM={n}"));
        }
        if let Some(n) = holdout {
            text.push_str(&format!("\n  /HOLDOUT={n}"));
            if self.mixed {
                text.push_str("\n  /MIXHOLD=YES");
            }
        }
        match self.output {
            0 => text.push_str("\n  /REPLACE"),
            1 => match self.file.trim() {
                "" => return Err("choose the file for the plan".into()),
                path => text.push_str(&format!("\n  /OUTFILE={}", quote(path))),
            },
            _ => {
                // the command keeps the cases of data whose variables are the factors, as
                // simulation profiles, and refuses any other data
                let mut theirs: Vec<String> = data
                    .variable_names()
                    .filter(|n| !is_bookkeeping(n))
                    .map(str::to_uppercase)
                    .collect();
                let mut ours: Vec<String> = seen.iter().map(|n| n.to_uppercase()).collect();
                theirs.sort();
                ours.sort();
                if !theirs.is_empty() && theirs != ours {
                    return Err(
                        "the variables of the active data are not these factors: replace it or save to a file"
                            .into(),
                    );
                }
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// The layouts of PLANCARDS, as the keyword and the words in the dialog.
pub const CARD_FORMATS: [(&str, &str); 3] = [
    ("LIST", "A list of the profiles"),
    ("CARD", "One card for each profile"),
    ("BOTH", "The list and the cards"),
];

/// PLANCARDS: the profiles of the plan in the active data, as a list or as cards.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PlanCards {
    /// None of them shows every factor.
    pub factors: Vec<String>,
    /// Index into `CARD_FORMATS`.
    pub format: usize,
    /// One line each.
    pub title: String,
    pub footer: String,
    /// A text file to write the cards to, instead of showing them.
    pub file: String,
}

/// The non-empty lines of a text, as quoted strings.
fn quoted_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .map(quote)
        .collect()
}

impl PlanCards {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        for name in &self.factors {
            if data.variable(name).is_err() {
                return Err(format!("{name} is no longer a variable"));
            }
        }
        let mut text = String::from("PLANCARDS");
        if !self.factors.is_empty() {
            text.push_str(&format!("\n  /FACTORS={}", self.factors.join(" ")));
        }
        if self.format > 0 {
            text.push_str(&format!("\n  /FORMAT={}", CARD_FORMATS[self.format].0));
        }
        for (keyword, lines) in [("TITLE", &self.title), ("FOOTER", &self.footer)] {
            let lines = quoted_lines(lines);
            if !lines.is_empty() {
                text.push_str(&format!("\n  /{keyword}={}", lines.join(" ")));
            }
        }
        // the list is always shown, so a file asks for cards
        if self.format > 0 && !self.file.trim().is_empty() {
            text.push_str(&format!("\n  /OUTFILE={}", quote(self.file.trim())));
        }
        text.push('.');
        Ok(text)
    }
}

/// How the responses are recorded, as the keyword and the words in the dialog.
pub const CONJOINT_RECORDS: [(&str, &str); 3] = [
    ("SCORE", "A score for each profile (higher is better)"),
    ("RANK", "A rank for each profile (1 is best)"),
    (
        "SEQUENCE",
        "The profile numbers, from the best down to the worst",
    ),
];

/// The models of a factor, as the keyword and the words in the dialog.
pub const CONJOINT_MODELS: [(&str, &str); 4] = [
    ("DISCRETE", "Discrete: a utility for each level"),
    ("LINEAR", "Linear: utility rises or falls with the value"),
    ("IDEAL", "Ideal point: best at a value in between"),
    ("ANTIIDEAL", "Anti-ideal point: worst at a value in between"),
];

/// What a subject's utilities are expected to do; only the first two models take it.
pub const CONJOINT_DIRECTIONS: [(&str, &str); 3] = [
    ("", "No expectation"),
    ("MORE", "More is better"),
    ("LESS", "Less is better"),
];

pub const CONJOINT_PRINTS: [(&str, &str); 4] = [
    ("ALL", "Analysis and simulations"),
    ("ANALYSIS", "Analysis only"),
    ("SIMULATION", "Simulations only"),
    ("NONE", "Nothing"),
];

pub const CONJOINT_PLOTS: [(&str, &str); 4] = [
    ("", "No charts"),
    ("SUMMARY", "Charts of the group"),
    ("SUBJECT", "Charts of each subject"),
    ("ALL", "Both"),
];

/// How one factor of the plan enters the analysis.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FactorModel {
    pub name: String,
    /// Index into `CONJOINT_MODELS`.
    pub model: usize,
    /// Index into `CONJOINT_DIRECTIONS`.
    pub direction: usize,
}

/// CONJOINT: utilities and importances from the preferences for the profiles of a plan.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ConjointAnalysis {
    /// The plan is a data file, not the active data.
    pub plan_from_file: bool,
    pub plan_file: String,
    /// The responses are in a data file, not the active data.
    pub data_from_file: bool,
    pub data_file: String,
    /// Index into `CONJOINT_RECORDS`.
    pub record: usize,
    /// The response variables, in the order of the profiles; TO is allowed.
    pub responses: String,
    pub subject: String,
    pub factors: Vec<FactorModel>,
    /// Index into `CONJOINT_PRINTS`.
    pub print: usize,
    pub summary_only: bool,
    pub utility: String,
    /// Index into `CONJOINT_PLOTS`.
    pub plot: usize,
    /// The factors of the plan file, read when it was chosen: the file they come from and the
    /// number of levels of each.
    pub file_levels: (String, Vec<(String, usize)>),
}

impl ConjointAnalysis {
    /// The factors of the plan and their level counts, when the dialog can tell.
    pub fn plan_levels(&self, data: &Dataset) -> Option<Vec<(String, usize)>> {
        match self.plan_from_file {
            false => Some(plan_counts(data)),
            true if self.file_levels.0 == self.plan_file.trim() => Some(self.file_levels.1.clone()),
            true => None,
        }
    }

    /// Reads the plan file once for each path, to know its factors; a file that cannot be read
    /// is left to the command to report.
    pub fn read_plan(&mut self) {
        let path = self.plan_file.trim().to_string();
        if !self.plan_from_file || path.is_empty() || self.file_levels.0 == path {
            return;
        }
        let levels = Dataset::read_sav(&path)
            .map(|d| plan_counts(&d))
            .unwrap_or_default();
        self.file_levels = (path, levels);
    }

    /// Words of the response list: variable names and TO.
    fn response_words(&self) -> Result<String, String> {
        let words: Vec<&str> = self.responses.split_whitespace().collect();
        if words.is_empty() {
            return Err("give the response variables".into());
        }
        for w in &words {
            if !w.eq_ignore_ascii_case("TO") && !is_valid_name(w) {
                return Err(format!("{w} is not a variable name"));
            }
        }
        Ok(words.join(" "))
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let source = |from_file: bool, file: &str, what: &str| -> Result<String, String> {
            match (from_file, file.trim()) {
                (false, _) => Ok("*".into()),
                (true, "") => Err(format!("choose the {what} file")),
                (true, path) => Ok(quote(path)),
            }
        };
        if !self.plan_from_file && !self.data_from_file {
            return Err(
                "the active data cannot be both the plan and the responses: use a file for one"
                    .into(),
            );
        }
        let plan = source(self.plan_from_file, &self.plan_file, "plan")?;
        let responses = source(self.data_from_file, &self.data_file, "data")?;
        let mut text = format!(
            "CONJOINT PLAN={plan}\n  /DATA={responses}\n  /{}={}",
            CONJOINT_RECORDS[self.record].0,
            self.response_words()?
        );
        let subject = self.subject.trim();
        if !subject.is_empty() {
            if !is_valid_name(subject) {
                return Err(format!("{subject} is not a variable name"));
            }
            text.push_str(&format!("\n  /SUBJECT={subject}"));
        }
        if !self.factors.is_empty() {
            let known = self.plan_levels(data).filter(|l| !l.is_empty());
            let mut seen: Vec<&str> = Vec::new();
            let mut items = Vec::new();
            for f in &self.factors {
                let name = f.name.trim();
                if !is_valid_name(name) {
                    return Err(format!("{name:?} is not a factor name"));
                }
                if seen.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                    return Err(format!("{name} is listed twice"));
                }
                seen.push(name);
                if let Some(known) = &known {
                    let Some((_, levels)) =
                        known.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))
                    else {
                        return Err(format!("{name} is not a factor of the plan"));
                    };
                    // a squared term cannot be fitted to two values
                    if f.model >= 2 && *levels < 3 {
                        return Err(format!(
                            "{name} has {levels} levels; IDEAL and ANTIIDEAL need at least three"
                        ));
                    }
                }
                let mut spec = CONJOINT_MODELS[f.model].0.to_string();
                let direction = CONJOINT_DIRECTIONS[f.direction].0;
                if f.model < 2 && !direction.is_empty() {
                    spec.push_str(&format!(" {direction}"));
                }
                items.push(format!("{name} ({spec})"));
            }
            text.push_str(&format!("\n  /FACTORS={}", items.join(" ")));
        }
        let print = CONJOINT_PRINTS[self.print].0;
        if self.print > 0 || self.summary_only {
            text.push_str(&format!("\n  /PRINT={print}"));
            if self.summary_only {
                text.push_str(" SUMMARYONLY");
            }
        }
        if !self.utility.trim().is_empty() {
            text.push_str(&format!("\n  /UTILITY={}", quote(self.utility.trim())));
        }
        if self.plot > 0 {
            text.push_str(&format!("\n  /PLOT={}", CONJOINT_PLOTS[self.plot].0));
        }
        text.push('.');
        Ok(text)
    }
}

/// The dialogs of this module, one variant of the window's dialogs.
#[derive(Clone, PartialEq, Debug)]
pub enum Design {
    Generate(OrthogonalPlan),
    Display(PlanCards),
    Analysis(Box<ConjointAnalysis>),
}

impl Design {
    pub fn title(&self) -> &'static str {
        match self {
            Design::Generate(_) => "Generate Orthogonal Design",
            Design::Display(_) => "Display Orthogonal Design",
            Design::Analysis(_) => "Conjoint",
        }
    }

    /// The same dialog with its defaults.
    pub fn reset(&self) -> Self {
        match self {
            Design::Generate(_) => Design::Generate(Default::default()),
            Design::Display(_) => Design::Display(Default::default()),
            Design::Analysis(_) => Design::Analysis(Default::default()),
        }
    }

    /// Generate makes its data, so it works without any, and takes no variables.
    pub fn generates(&self) -> bool {
        matches!(self, Design::Generate(_))
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        match self {
            Design::Generate(d) => d.syntax(data),
            Design::Display(d) => d.syntax(data),
            Design::Analysis(d) => d.syntax(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(value: &str, label: &str) -> Level {
        Level {
            value: value.into(),
            label: label.into(),
        }
    }

    fn brand() -> PlanFactor {
        PlanFactor {
            name: "brand".into(),
            label: "Brand".into(),
            levels: vec![level("1", "Acme"), level("2", "Brio"), level("3", "")],
        }
    }

    fn organic() -> PlanFactor {
        PlanFactor {
            name: "organic".into(),
            label: String::new(),
            levels: vec![level("0", "no"), level("1", "yes")],
        }
    }

    #[test]
    fn orthoplan_syntax() {
        let none = Dataset::new(0);
        let mut d = OrthogonalPlan {
            factors: vec![brand(), organic()],
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&none).unwrap(),
            "ORTHOPLAN\n  /FACTORS=brand 'Brand' (1 'Acme' 2 'Brio' 3)\n    \
             organic (0 'no' 1 'yes')\n  /REPLACE."
        );
        d.minimum = "6".into();
        d.holdout = "2".into();
        d.mixed = true;
        d.seed = "42".into();
        d.output = 1;
        d.file = "/tmp/it's.sav".into();
        assert_eq!(
            d.syntax(&none).unwrap(),
            "SET SEED=42.\nORTHOPLAN\n  /FACTORS=brand 'Brand' (1 'Acme' 2 'Brio' 3)\n    \
             organic (0 'no' 1 'yes')\n  /MINIMUM=6\n  /HOLDOUT=2\n  /MIXHOLD=YES\n  \
             /OUTFILE='/tmp/it''s.sav'."
        );
    }

    #[test]
    fn orthoplan_catches_what_the_command_refuses() {
        let none = Dataset::new(0);
        let with = |f: &dyn Fn(&mut OrthogonalPlan)| {
            let mut d = OrthogonalPlan {
                factors: vec![brand(), organic()],
                ..Default::default()
            };
            f(&mut d);
            d.syntax(&none).unwrap_err()
        };
        assert!(with(&|d| d.factors[0].name = "STATUS_".into()).contains("cannot name"));
        assert!(with(&|d| d.factors[1].name = "Brand".into()).contains("twice"));
        assert!(with(&|d| d.factors[0].levels.truncate(1)).contains("2 to 9"));
        assert!(with(&|d| d.factors[0].levels[2].value = "2".into()).contains("twice"));
        assert!(with(&|d| d.factors[0].levels[2].value = "x".into()).contains("needs a number"));
        assert!(with(&|d| d.holdout = "7".into()).contains("more than the 6"));
        assert!(with(&|d| d.output = 1).contains("choose the file"));
    }

    #[test]
    fn orthoplan_adds_only_to_data_of_its_factors() {
        let mut session = statscraft_engine::session::Session::default();
        session
            .run(
                "DATA LIST LIST /brand organic.\nBEGIN DATA\n1 0\nEND DATA.",
                None,
            )
            .unwrap();
        let mut d = OrthogonalPlan {
            factors: vec![brand(), organic()],
            output: 2,
            ..Default::default()
        };
        assert!(d.syntax(session.dataset().unwrap()).is_ok());
        d.factors[1].name = "fair".into();
        assert!(
            d.syntax(session.dataset().unwrap())
                .unwrap_err()
                .contains("not these factors")
        );
    }

    #[test]
    fn plancards_syntax() {
        let mut session = statscraft_engine::session::Session::default();
        session
            .run(
                "DATA LIST LIST /brand price.\nBEGIN DATA\n1 2\nEND DATA.",
                None,
            )
            .unwrap();
        let data = session.dataset().unwrap();
        let mut d = PlanCards::default();
        assert_eq!(d.syntax(data).unwrap(), "PLANCARDS.");
        d.factors = vec!["price".into(), "brand".into()];
        d.format = 2;
        d.title = "Profile )CARD\n\nOf 9".into();
        d.footer = "Score it".into();
        d.file = "cards.txt".into();
        assert_eq!(
            d.syntax(data).unwrap(),
            "PLANCARDS\n  /FACTORS=price brand\n  /FORMAT=BOTH\n  \
             /TITLE='Profile )CARD' 'Of 9'\n  /FOOTER='Score it'\n  /OUTFILE='cards.txt'."
        );
        d.factors = vec!["gone".into()];
        assert!(d.syntax(data).is_err());
        // the list has no file
        d.factors.clear();
        d.format = 0;
        assert!(!d.syntax(data).unwrap().contains("OUTFILE"));
    }

    fn plan_data() -> statscraft_engine::session::Session {
        let mut session = statscraft_engine::session::Session::default();
        session
            .run(
                "DATA LIST LIST /brand organic STATUS_.\nBEGIN DATA\n1 0 0\n2 1 0\n3 1 0\n\
                 1 1 0\n2 0 1\nEND DATA.",
                None,
            )
            .unwrap();
        session
    }

    #[test]
    fn conjoint_syntax() {
        let session = plan_data();
        let data = session.dataset().unwrap();
        let mut d = ConjointAnalysis {
            plan_from_file: true,
            plan_file: "plan.sav".into(),
            responses: "r1 TO r4".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(data).unwrap(),
            "CONJOINT PLAN='plan.sav'\n  /DATA=*\n  /SCORE=r1 TO r4."
        );
        d.record = 1;
        d.subject = "id".into();
        d.factors = vec![
            FactorModel {
                name: "price".into(),
                model: 1,
                direction: 2,
            },
            FactorModel {
                name: "brand".into(),
                model: 3,
                direction: 1,
            },
            FactorModel {
                name: "organic".into(),
                ..Default::default()
            },
        ];
        d.print = 1;
        d.summary_only = true;
        d.utility = "u.sav".into();
        d.plot = 3;
        assert_eq!(
            d.syntax(data).unwrap(),
            "CONJOINT PLAN='plan.sav'\n  /DATA=*\n  /RANK=r1 TO r4\n  /SUBJECT=id\n  \
             /FACTORS=price (LINEAR LESS) brand (ANTIIDEAL) organic (DISCRETE)\n  \
             /PRINT=ANALYSIS SUMMARYONLY\n  /UTILITY='u.sav'\n  /PLOT=ALL."
        );
    }

    #[test]
    fn conjoint_catches_what_the_command_refuses() {
        let session = plan_data();
        let data = session.dataset().unwrap();
        let mut d = ConjointAnalysis {
            responses: "r1 r2".into(),
            ..Default::default()
        };
        assert!(d.syntax(data).unwrap_err().contains("cannot be both"));
        d.data_from_file = true;
        assert!(d.syntax(data).unwrap_err().contains("choose the data file"));
        d.data_file = "d.sav".into();
        // the plan is the active data: brand has three levels, organic two
        d.factors = vec![FactorModel {
            name: "organic".into(),
            model: 2,
            direction: 0,
        }];
        assert!(d.syntax(data).unwrap_err().contains("at least three"));
        d.factors[0].model = 1;
        assert!(d.syntax(data).is_ok());
        d.factors[0].name = "colour".into();
        assert!(d.syntax(data).unwrap_err().contains("not a factor"));
        d.factors.clear();
        d.responses = "r1 +".into();
        assert!(d.syntax(data).is_err());
        d.responses = String::new();
        assert!(d.syntax(data).is_err());
    }

    #[test]
    fn plan_counts_use_the_experimental_profiles() {
        let session = plan_data();
        assert_eq!(
            plan_counts(session.dataset().unwrap()),
            vec![("brand".to_string(), 3), ("organic".to_string(), 2)]
        );
    }
}
