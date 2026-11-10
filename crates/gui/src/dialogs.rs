//! The state of every Analyze and Transform dialog and the syntax it builds. A dialog never
//! touches data: OK runs the syntax, Paste puts it in the syntax editor.

use crate::syntax_text::{names, probability, quote, value};

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Frequencies {
    pub variables: Vec<String>,
}

impl Frequencies {
    pub fn syntax(&self) -> Result<String, String> {
        Ok(format!(
            "FREQUENCIES VARIABLES={}.",
            names(&self.variables, "Variables")?
        ))
    }
}

/// Keyword and label of every statistic Descriptives offers, in the order shown.
pub const DESCRIPTIVES_STATISTICS: [(&str, &str); 10] = [
    ("MEAN", "Mean"),
    ("STDDEV", "Standard deviation"),
    ("MINIMUM", "Minimum"),
    ("MAXIMUM", "Maximum"),
    ("SEMEAN", "Standard error of the mean"),
    ("VARIANCE", "Variance"),
    ("RANGE", "Range"),
    ("SUM", "Sum"),
    ("SKEWNESS", "Skewness"),
    ("KURTOSIS", "Kurtosis"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Descriptives {
    pub variables: Vec<String>,
    /// One flag per entry of `DESCRIPTIVES_STATISTICS`.
    pub statistics: [bool; 10],
    pub save_z_scores: bool,
    pub listwise: bool,
}

impl Default for Descriptives {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            statistics: [
                true, true, true, true, false, false, false, false, false, false,
            ],
            save_z_scores: false,
            listwise: false,
        }
    }
}

impl Descriptives {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!(
            "DESCRIPTIVES VARIABLES={}",
            names(&self.variables, "Variables")?
        );
        let statistics: Vec<&str> = DESCRIPTIVES_STATISTICS
            .iter()
            .zip(self.statistics)
            .filter_map(|((keyword, _), on)| on.then_some(*keyword))
            .collect();
        if statistics.is_empty() {
            return Err("choose at least one statistic".into());
        }
        text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        if self.listwise {
            text.push_str("\n  /MISSING=LISTWISE");
        }
        if self.save_z_scores {
            text.push_str("\n  /SAVE");
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of every cell statistic Crosstabs offers, in the order shown.
pub const CROSSTABS_CELLS: [(&str, &str); 8] = [
    ("COUNT", "Observed count"),
    ("ROW", "Row percentage"),
    ("COLUMN", "Column percentage"),
    ("TOTAL", "Total percentage"),
    ("EXPECTED", "Expected count"),
    ("RESID", "Residual"),
    ("SRESID", "Standardized residual"),
    ("ASRESID", "Adjusted standardized residual"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Crosstabs {
    pub rows: Vec<String>,
    pub columns: Vec<String>,
    pub layers: Vec<String>,
    /// One flag per entry of `CROSSTABS_CELLS`.
    pub cells: [bool; 8],
    pub chi_square: bool,
}

impl Default for Crosstabs {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            columns: Vec::new(),
            layers: Vec::new(),
            cells: [true, false, false, false, false, false, false, false],
            chi_square: false,
        }
    }
}

impl Crosstabs {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!(
            "CROSSTABS /TABLES={} BY {}",
            names(&self.rows, "Rows")?,
            names(&self.columns, "Columns")?
        );
        if !self.layers.is_empty() {
            text.push_str(&format!(" BY {}", self.layers.join(" ")));
        }
        let cells: Vec<&str> = CROSSTABS_CELLS
            .iter()
            .zip(self.cells)
            .filter_map(|((keyword, _), on)| on.then_some(*keyword))
            .collect();
        let cells = if cells.is_empty() {
            "NONE".to_string()
        } else {
            cells.join(" ")
        };
        text.push_str(&format!("\n  /CELLS={cells}"));
        if self.chi_square {
            text.push_str("\n  /STATISTICS=CHISQ");
        }
        text.push('.');
        Ok(text)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Adjust {
    #[default]
    Bonferroni,
    BenjaminiHochberg,
    None,
}

impl Adjust {
    pub const ALL: [Adjust; 3] = [Adjust::Bonferroni, Adjust::BenjaminiHochberg, Adjust::None];

    pub fn keyword(self) -> &'static str {
        match self {
            Adjust::Bonferroni => "BONFERRONI",
            Adjust::BenjaminiHochberg => "BH",
            Adjust::None => "NONE",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Adjust::Bonferroni => "Bonferroni",
            Adjust::BenjaminiHochberg => "Benjamini-Hochberg",
            Adjust::None => "No adjustment",
        }
    }
}

/// Where the column comparison tests of Custom Tables put their letters.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Letters {
    #[default]
    Separate,
    /// In the main table, letters that columns share unless they differ.
    Apa,
    /// In the main table, the keys of the columns that differ.
    Simple,
}

impl Letters {
    pub const ALL: [Letters; 3] = [Letters::Separate, Letters::Apa, Letters::Simple];

    fn options(self) -> &'static str {
        match self {
            Letters::Separate => "",
            Letters::Apa => " MERGE=YES",
            Letters::Simple => " MERGE=YES STYLE=SIMPLE",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Letters::Separate => "In a table of their own",
            Letters::Apa => "In the main table, shared letters (APA)",
            Letters::Simple => "In the main table, column keys",
        }
    }
}

/// A simple custom table: categorical variables and multiple response sets ($name) stacked in
/// the rows and the columns, scale variables whose means are shown in the rows, and stacked
/// layer variables that give a table for each of their categories.
#[derive(Clone, PartialEq, Debug)]
pub struct CustomTables {
    pub rows: Vec<String>,
    pub columns: Vec<String>,
    pub scale_rows: Vec<String>,
    pub layers: Vec<String>,
    pub count: bool,
    pub column_percent: bool,
    pub row_percent: bool,
    pub mean: bool,
    pub totals: bool,
    pub chi_square: bool,
    pub compare_proportions: bool,
    pub compare_means: bool,
    pub alpha: String,
    pub adjust: Adjust,
    pub letters: Letters,
    pub hide_small_counts: bool,
    pub small_count: String,
}

impl Default for CustomTables {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            columns: Vec::new(),
            scale_rows: Vec::new(),
            layers: Vec::new(),
            count: true,
            column_percent: false,
            row_percent: false,
            mean: true,
            totals: false,
            chi_square: false,
            compare_proportions: false,
            compare_means: false,
            alpha: "0.05".into(),
            adjust: Adjust::Bonferroni,
            letters: Letters::Separate,
            hide_small_counts: false,
            small_count: "5".into(),
        }
    }
}

impl CustomTables {
    pub fn syntax(&self) -> Result<String, String> {
        if self.rows.is_empty() && self.scale_rows.is_empty() {
            return Err("add at least one variable to Rows or Means".into());
        }
        let mut summaries = Vec::new();
        for (on, function) in [
            (self.count, "COUNT"),
            (self.column_percent, "COLPCT.COUNT PCT40.1"),
            (self.row_percent, "ROWPCT.COUNT PCT40.1"),
        ] {
            if on {
                summaries.push(function);
            }
        }
        if !self.rows.is_empty() && summaries.is_empty() {
            return Err("choose count, column percentage or row percentage for the rows".into());
        }
        if !self.scale_rows.is_empty() && !self.mean {
            return Err("variables under Means need the Mean statistic".into());
        }
        if self.scale_rows.iter().any(|name| is_set(name)) {
            return Err("a multiple response set has no mean; put it in Rows".into());
        }
        let summary = summaries.join(", ");
        let mut rows: Vec<String> = self
            .rows
            .iter()
            .map(|name| format!("{} [{summary}]", categorical(name)))
            .collect();
        rows.extend(
            self.scale_rows
                .iter()
                .map(|name| format!("{name} [S] [MEAN]")),
        );
        let mut text = String::from("CTABLES");
        if self.hide_small_counts {
            let below = self
                .small_count
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n >= 2)
                .ok_or("hide counts below a whole number of 2 or more")?;
            text.push_str("\n  /HIDESMALLCOUNTS");
            if below != 5 {
                text.push_str(&format!(" COUNT={below}"));
            }
        }
        text.push_str(&format!("\n  /TABLE {}", rows.join(" + ")));
        let stack = |names: &[String]| {
            names
                .iter()
                .map(|n| categorical(n))
                .collect::<Vec<_>>()
                .join(" + ")
        };
        if !self.columns.is_empty() || !self.layers.is_empty() {
            text.push_str(" BY");
            if !self.columns.is_empty() {
                text.push_str(&format!(" {}", stack(&self.columns)));
            }
        }
        if !self.layers.is_empty() {
            text.push_str(&format!(" BY {}", stack(&self.layers)));
        }
        let categorical: Vec<&str> = self
            .rows
            .iter()
            .chain(&self.columns)
            .map(String::as_str)
            .collect();
        if self.totals && !categorical.is_empty() {
            text.push_str(&format!(
                "\n  /CATEGORIES VARIABLES={} TOTAL=YES",
                categorical.join(" ")
            ));
        }
        let tests = self.chi_square || self.compare_proportions || self.compare_means;
        if tests {
            if self.columns.is_empty() {
                return Err("significance tests need at least one variable in Columns".into());
            }
            let alpha = probability(&self.alpha)?;
            let adjust = self.adjust.keyword();
            let letters = self.letters.options();
            if (self.chi_square || self.compare_proportions) && self.rows.is_empty() {
                return Err("chi-square and proportion tests need a variable in Rows".into());
            }
            if self.chi_square {
                text.push_str(&format!("\n  /SIGTEST TYPE=CHISQUARE ALPHA={alpha}"));
            }
            if self.compare_proportions {
                text.push_str(&format!(
                    "\n  /COMPARETEST TYPE=PROP ALPHA={alpha} ADJUST={adjust}{letters}"
                ));
            }
            if self.compare_means {
                if self.scale_rows.is_empty() {
                    return Err("the test of means needs a variable under Means".into());
                }
                text.push_str(&format!(
                    "\n  /COMPARETEST TYPE=MEAN ALPHA={alpha} ADJUST={adjust}{letters}"
                ));
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// A multiple response set: its name starts with $.
pub fn is_set(name: &str) -> bool {
    name.starts_with('$')
}

/// A variable forced to be categorical in CTABLES; a set is categorical already.
fn categorical(name: &str) -> String {
    if is_set(name) {
        name.to_string()
    } else {
        format!("{name} [C]")
    }
}

/// COMPUTE, or IF when a condition is given.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Compute {
    pub target: String,
    pub expression: String,
    pub condition: String,
}

/// User text that goes into a command as it is: trimmed, without a closing period that would
/// end the command early.
pub fn clause(text: &str) -> &str {
    let text = text.trim();
    text.strip_suffix('.').unwrap_or(text).trim_end()
}

impl Compute {
    pub fn syntax(&self) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err("give the name of the target variable".into());
        }
        let expression = clause(&self.expression);
        if expression.is_empty() {
            return Err("give an expression".into());
        }
        Ok(match clause(&self.condition) {
            "" => format!("COMPUTE {target} = {expression}."),
            condition => format!("IF ({condition}) {target} = {expression}."),
        })
    }
}

/// The old values of one recode rule.
#[derive(Clone, PartialEq, Debug)]
pub enum OldValue {
    Value(String),
    Range(String, String),
    LowestThru(String),
    ThruHighest(String),
    Missing,
    SystemMissing,
    Else,
}

impl OldValue {
    /// Names for the choice in the dialog, in the order of `OldValue::from_choice`.
    pub const CHOICES: [&str; 7] = [
        "Value",
        "Range",
        "Lowest through",
        "Through highest",
        "Any missing",
        "System-missing",
        "All other values",
    ];

    pub fn from_choice(choice: usize, a: &str, b: &str) -> Self {
        let (a, b) = (a.to_string(), b.to_string());
        match choice {
            0 => OldValue::Value(a),
            1 => OldValue::Range(a, b),
            2 => OldValue::LowestThru(a),
            3 => OldValue::ThruHighest(a),
            4 => OldValue::Missing,
            5 => OldValue::SystemMissing,
            _ => OldValue::Else,
        }
    }

    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let range = |text: &str| {
            if string {
                Err("ranges work only with numeric variables".to_string())
            } else {
                value(text, false)
            }
        };
        Ok(match self {
            OldValue::Value(v) => value(v, string)?,
            OldValue::Range(low, high) => format!("{} THRU {}", range(low)?, range(high)?),
            OldValue::LowestThru(high) => format!("LOWEST THRU {}", range(high)?),
            OldValue::ThruHighest(low) => format!("{} THRU HIGHEST", range(low)?),
            OldValue::Missing => "MISSING".into(),
            OldValue::SystemMissing => "SYSMIS".into(),
            OldValue::Else => "ELSE".into(),
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum NewValue {
    Value(String),
    SystemMissing,
    Copy,
}

impl NewValue {
    pub const CHOICES: [&str; 3] = ["Value", "System-missing", "Copy old value"];

    pub fn from_choice(choice: usize, a: &str) -> Self {
        match choice {
            0 => NewValue::Value(a.to_string()),
            1 => NewValue::SystemMissing,
            _ => NewValue::Copy,
        }
    }

    fn syntax(&self, string: bool) -> Result<String, String> {
        Ok(match self {
            NewValue::Value(v) => value(v, string)?,
            NewValue::SystemMissing => "SYSMIS".into(),
            NewValue::Copy => "COPY".into(),
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct RecodeRule {
    pub old: OldValue,
    pub new: NewValue,
}

impl RecodeRule {
    /// The rule as the dialog lists it, which is also its syntax.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        Ok(format!(
            "({} = {})",
            self.old.syntax(string)?,
            self.new.syntax(string)?
        ))
    }
}

/// RECODE in place, or INTO new variables when `into` names them.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Recode {
    pub variables: Vec<String>,
    /// Target names separated by spaces, one per variable; empty to recode in place.
    pub into: String,
    pub rules: Vec<RecodeRule>,
}

impl Recode {
    /// `string` tells whether the variables are string variables, so values get quotes.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let variables = names(&self.variables, "Variables")?;
        if self.rules.is_empty() {
            return Err("add at least one rule".into());
        }
        let rules = self
            .rules
            .iter()
            .map(|rule| rule.syntax(string))
            .collect::<Result<Vec<_>, _>>()?;
        let mut text = format!("RECODE {variables} {}", rules.join(" "));
        let into: Vec<&str> = self.into.split_whitespace().collect();
        if !into.is_empty() {
            if into.len() != self.variables.len() {
                return Err(format!(
                    "give one new name per variable: {} variables, {} names",
                    self.variables.len(),
                    into.len()
                ));
            }
            text.push_str(&format!(" INTO {}", into.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// COUNT: how many of the variables hold one of the values, per case, into a new variable.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Count {
    pub target: String,
    pub variables: Vec<String>,
    /// Any old value of Recode except "all other values".
    pub values: Vec<OldValue>,
}

impl Count {
    /// `string` tells whether the variables are string variables, so values get quotes.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err("give the name of the target variable".into());
        }
        let variables = names(&self.variables, "Variables")?;
        if self.values.is_empty() {
            return Err("add at least one value to count".into());
        }
        let values = self
            .values
            .iter()
            .map(|v| match v {
                OldValue::Else => Err("all other values cannot be counted".to_string()),
                v => v.syntax(string),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!(
            "COUNT {target} = {variables} ({}).",
            values.join(" ")
        ))
    }
}

/// A variable label for a new variable made by Compute or Recode, as its own command.
pub fn label_command(name: &str, label: &str) -> Option<String> {
    let label = label.trim();
    (!label.is_empty()).then(|| format!("VARIABLE LABELS {} {}.", name.trim(), quote(label)))
}

/// What the dialog tests share: every test runs the syntax it builds through the engine, to
/// prove the engine takes it.
#[cfg(test)]
pub(crate) mod testing {
    use crate::syntax_text::quote;
    use statscraft_engine::session::Session;

    pub fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// The path of a file in tests/fixtures.
    pub fn fixture(name: &str) -> String {
        format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    /// A session with a fixture open.
    pub fn open(name: &str) -> Session {
        let mut session = Session::default();
        session
            .run(&format!("GET FILE={}.", quote(&fixture(name))), None)
            .unwrap();
        session
    }

    /// Runs the syntax in the session and returns the number of tables it made.
    pub fn run_in(session: &mut Session, syntax: &str) -> usize {
        let result = session
            .run(syntax, None)
            .unwrap_or_else(|e| panic!("{syntax}\n{e:?}"));
        result.commands.iter().map(|o| o.tables.len()).sum()
    }

    /// Runs the syntax on the survey file and returns the number of tables it made.
    pub fn runs(syntax: &str) -> usize {
        run_in(&mut open("survey_basic.sav"), syntax)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{open, run_in, runs, v};
    use super::*;

    #[test]
    fn frequencies() {
        let mut d = Frequencies::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["gender", "region"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "FREQUENCIES VARIABLES=gender region.");
        assert_eq!(runs(&syntax), 2);
    }

    #[test]
    fn descriptives() {
        let mut d = Descriptives {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "DESCRIPTIVES VARIABLES=age monthly_spend\n  /STATISTICS=MEAN STDDEV MINIMUM MAXIMUM."
        );
        d.statistics = [false; 10];
        assert_eq!(d.syntax().unwrap_err(), "choose at least one statistic");
        d.statistics[7] = true;
        d.statistics[9] = true;
        d.listwise = true;
        d.save_z_scores = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "DESCRIPTIVES VARIABLES=age monthly_spend\n  /STATISTICS=SUM KURTOSIS\n  /MISSING=LISTWISE\n  /SAVE."
        );
        assert_eq!(runs(&syntax), 1);
    }

    #[test]
    fn crosstabs() {
        let mut d = Crosstabs {
            rows: v(&["region"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Columns"
        );
        d.columns = v(&["gender"]);
        assert_eq!(
            d.syntax().unwrap(),
            "CROSSTABS /TABLES=region BY gender\n  /CELLS=COUNT."
        );
        d.layers = v(&["plan"]);
        d.cells = [true, true, true, false, true, false, false, true];
        d.chi_square = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CROSSTABS /TABLES=region BY gender BY plan\n  /CELLS=COUNT ROW COLUMN EXPECTED ASRESID\n  /STATISTICS=CHISQ."
        );
        assert!(runs(&syntax) >= 2);
        d.cells = [false; 8];
        assert!(d.syntax().unwrap().contains("/CELLS=NONE"));
    }

    #[test]
    fn custom_tables_counts_and_percentages() {
        let mut d = CustomTables::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Rows or Means"
        );
        d.rows = v(&["region"]);
        assert_eq!(d.syntax().unwrap(), "CTABLES\n  /TABLE region [C] [COUNT].");
        d.rows = v(&["region", "q1_satisfaction"]);
        d.columns = v(&["gender"]);
        d.column_percent = true;
        d.row_percent = true;
        d.totals = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CTABLES\n  /TABLE region [C] [COUNT, COLPCT.COUNT PCT40.1, ROWPCT.COUNT PCT40.1] + \
             q1_satisfaction [C] [COUNT, COLPCT.COUNT PCT40.1, ROWPCT.COUNT PCT40.1] BY gender [C]\n  \
             /CATEGORIES VARIABLES=region q1_satisfaction gender TOTAL=YES."
        );
        assert_eq!(runs(&syntax), 1);
        d.count = false;
        d.column_percent = false;
        d.row_percent = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose count, column percentage or row percentage for the rows"
        );
    }

    #[test]
    fn custom_tables_means_and_tests() {
        let mut d = CustomTables {
            rows: v(&["region"]),
            scale_rows: v(&["age"]),
            column_percent: true,
            chi_square: true,
            compare_proportions: true,
            compare_means: true,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "significance tests need at least one variable in Columns"
        );
        d.columns = v(&["gender"]);
        d.alpha = "0.01".into();
        d.adjust = Adjust::BenjaminiHochberg;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CTABLES\n  /TABLE region [C] [COUNT, COLPCT.COUNT PCT40.1] + age [S] [MEAN] BY gender [C]\n  \
             /SIGTEST TYPE=CHISQUARE ALPHA=0.01\n  \
             /COMPARETEST TYPE=PROP ALPHA=0.01 ADJUST=BH\n  \
             /COMPARETEST TYPE=MEAN ALPHA=0.01 ADJUST=BH."
        );
        assert_eq!(runs(&syntax), 4);

        d.alpha = "5%".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the significance level must be a number between 0 and 1"
        );
        d.alpha = "0.05".into();
        d.mean = false;
        assert_eq!(
            d.syntax().unwrap_err(),
            "variables under Means need the Mean statistic"
        );
        d.mean = true;
        d.scale_rows.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the test of means needs a variable under Means"
        );
        d.compare_means = false;
        d.rows.clear();
        d.scale_rows = v(&["age"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "chi-square and proportion tests need a variable in Rows"
        );
        d.chi_square = false;
        d.compare_proportions = false;
        d.compare_means = true;
        d.adjust = Adjust::None;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CTABLES\n  /TABLE age [S] [MEAN] BY gender [C]\n  /COMPARETEST TYPE=MEAN ALPHA=0.05 ADJUST=NONE."
        );
        assert_eq!(runs(&syntax), 2);
    }

    #[test]
    fn custom_tables_sets_layers_letters_and_small_counts() {
        let mut d = CustomTables {
            rows: v(&["$q5", "region"]),
            columns: v(&["gender"]),
            layers: v(&["$q6", "plan"]),
            column_percent: true,
            totals: true,
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CTABLES\n  /TABLE $q5 [COUNT, COLPCT.COUNT PCT40.1] + region [C] [COUNT, COLPCT.COUNT PCT40.1] \
             BY gender [C] BY $q6 + plan [C]\n  \
             /CATEGORIES VARIABLES=$q5 region gender TOTAL=YES."
        );
        let mut session = open("survey_basic_phpwriter.sav");
        assert!(run_in(&mut session, &syntax) >= 2);

        // layers without columns, small counts hidden
        d.columns.clear();
        d.layers = v(&["gender"]);
        d.totals = false;
        d.hide_small_counts = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CTABLES\n  /HIDESMALLCOUNTS\n  /TABLE $q5 [COUNT, COLPCT.COUNT PCT40.1] + \
             region [C] [COUNT, COLPCT.COUNT PCT40.1] BY BY gender [C]."
        );
        assert!(run_in(&mut session, &syntax) >= 3);
        d.small_count = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "hide counts below a whole number of 2 or more"
        );
        d.small_count = "10".into();
        assert!(d.syntax().unwrap().contains("/HIDESMALLCOUNTS COUNT=10\n"));

        // letters in the main table
        d.hide_small_counts = false;
        d.layers.clear();
        d.columns = v(&["gender"]);
        d.scale_rows = v(&["age"]);
        d.compare_proportions = true;
        d.compare_means = true;
        d.letters = Letters::Apa;
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with(
            "/COMPARETEST TYPE=PROP ALPHA=0.05 ADJUST=BONFERRONI MERGE=YES\n  \
             /COMPARETEST TYPE=MEAN ALPHA=0.05 ADJUST=BONFERRONI MERGE=YES."
        ));
        assert!(run_in(&mut session, &syntax) >= 1);
        d.letters = Letters::Simple;
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("ADJUST=BONFERRONI MERGE=YES STYLE=SIMPLE."));
        assert!(run_in(&mut session, &syntax) >= 1);

        d.scale_rows = v(&["$q6"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "a multiple response set has no mean; put it in Rows"
        );
    }

    #[test]
    fn count_values() {
        let mut d = Count::default();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "give the name of the target variable"
        );
        d.target = "channels".into();
        d.variables = v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]);
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "add at least one value to count"
        );
        d.values = vec![OldValue::Value("1".into())];
        let syntax = d.syntax(false).unwrap();
        assert_eq!(syntax, "COUNT channels = q5_1 q5_2 q5_3 q5_4 q5_5 (1).");
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        let data = session.dataset().unwrap();
        let counts = data.variable("channels").unwrap();
        assert!(
            counts
                .numeric()
                .unwrap()
                .iter()
                .flatten()
                .all(|n| (0.0..=5.0).contains(&n))
        );

        d.target = "extremes".into();
        d.variables = v(&["q1_satisfaction", "q2_recommend"]);
        d.values = vec![
            OldValue::LowestThru("1".into()),
            OldValue::Range("5".into(), "10".into()),
            OldValue::ThruHighest("98".into()),
            OldValue::Missing,
            OldValue::SystemMissing,
        ];
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "COUNT extremes = q1_satisfaction q2_recommend \
             (LOWEST THRU 1 5 THRU 10 98 THRU HIGHEST MISSING SYSMIS)."
        );
        assert_eq!(runs(&syntax), 0);
        d.values.push(OldValue::Else);
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "all other values cannot be counted"
        );

        let strings = Count {
            target: "basic".into(),
            variables: v(&["plan"]),
            values: vec![OldValue::Value("BASIC".into())],
        };
        let syntax = strings.syntax(true).unwrap();
        assert_eq!(syntax, "COUNT basic = plan ('BASIC').");
        assert_eq!(runs(&syntax), 0);
        let range = Count {
            values: vec![OldValue::Range("a".into(), "b".into())],
            ..strings
        };
        assert_eq!(
            range.syntax(true).unwrap_err(),
            "ranges work only with numeric variables"
        );
    }

    #[test]
    fn compute_and_if() {
        let mut d = Compute::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the name of the target variable"
        );
        d.target = " older ".into();
        assert_eq!(d.syntax().unwrap_err(), "give an expression");
        d.expression = "age + 10.".into();
        assert_eq!(d.syntax().unwrap(), "COMPUTE older = age + 10.");
        d.condition = "gender = 1".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "IF (gender = 1) older = age + 10.");
        assert_eq!(runs(&syntax), 0);
        assert_eq!(
            label_command("older", "Age in ten years").unwrap(),
            "VARIABLE LABELS older 'Age in ten years'."
        );
        assert_eq!(label_command("older", " "), None);
    }

    #[test]
    fn recode_numeric_into_new_variables() {
        let rule = |old: OldValue, new: NewValue| RecodeRule { old, new };
        let mut d = Recode {
            variables: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax(false).unwrap_err(), "add at least one rule");
        d.rules = vec![
            rule(
                OldValue::LowestThru("29".into()),
                NewValue::Value("1".into()),
            ),
            rule(
                OldValue::Range("30".into(), "59".into()),
                NewValue::Value("2".into()),
            ),
            rule(
                OldValue::ThruHighest("60".into()),
                NewValue::Value("3".into()),
            ),
            rule(OldValue::Missing, NewValue::SystemMissing),
            rule(OldValue::Else, NewValue::Copy),
        ];
        d.into = "age_group extra".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "give one new name per variable: 1 variables, 2 names"
        );
        d.into = " age_group ".into();
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "RECODE age (LOWEST THRU 29 = 1) (30 THRU 59 = 2) (60 THRU HIGHEST = 3) \
             (MISSING = SYSMIS) (ELSE = COPY) INTO age_group."
        );
        assert_eq!(runs(&syntax), 0);
        d.rules
            .push(rule(OldValue::Value("x".into()), NewValue::Copy));
        assert_eq!(d.syntax(false).unwrap_err(), "\"x\" is not a number");
    }

    #[test]
    fn recode_strings_in_place() {
        let d = Recode {
            variables: v(&["plan"]),
            into: String::new(),
            rules: vec![
                RecodeRule {
                    old: OldValue::from_choice(0, "BASIC", ""),
                    new: NewValue::from_choice(0, "Basic's"),
                },
                RecodeRule {
                    old: OldValue::from_choice(6, "", ""),
                    new: NewValue::from_choice(2, ""),
                },
            ],
        };
        let syntax = d.syntax(true).unwrap();
        assert_eq!(syntax, "RECODE plan ('BASIC' = 'Basic''s') (ELSE = COPY).");
        assert_eq!(runs(&syntax), 0);
        let range = Recode {
            rules: vec![RecodeRule {
                old: OldValue::from_choice(1, "a", "b"),
                new: NewValue::Copy,
            }],
            ..d
        };
        assert_eq!(
            range.syntax(true).unwrap_err(),
            "ranges work only with numeric variables"
        );
    }
}
