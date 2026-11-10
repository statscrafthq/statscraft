//! The Data menu dialogs: multiple response sets, sorting, selecting and weighting cases,
//! aggregating, and merging files.

use crate::syntax_text::{names, quote, value};

/// MRSETS: define a dichotomy or category set, or delete sets.
#[derive(Clone, PartialEq, Debug)]
pub struct Mrsets {
    pub delete: bool,
    /// A dichotomy set (MDGROUP) rather than a category set (MCGROUP).
    pub dichotomy: bool,
    pub name: String,
    pub label: String,
    pub variables: Vec<String>,
    /// The counted value of a dichotomy set.
    pub value: String,
    /// CATEGORYLABELS=COUNTEDVALUES: categories get the value label of the counted value.
    pub counted_value_labels: bool,
    /// LABELSOURCE=VARLABEL: the set label is the first variable label (with counted value labels).
    pub label_from_variables: bool,
    /// The sets to delete.
    pub sets: Vec<String>,
}

impl Default for Mrsets {
    fn default() -> Self {
        Self {
            delete: false,
            dichotomy: true,
            name: String::new(),
            label: String::new(),
            variables: Vec::new(),
            value: "1".into(),
            counted_value_labels: false,
            label_from_variables: false,
            sets: Vec::new(),
        }
    }
}

impl Mrsets {
    /// `string` tells whether the variables are string variables, so the value gets quotes.
    pub fn syntax(&self, string: bool) -> Result<String, String> {
        if self.delete {
            if self.sets.is_empty() {
                return Err("choose the sets to delete".into());
            }
            return Ok(format!("MRSETS /DELETE NAME=[{}].", self.sets.join(" ")));
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err("give the set a name".into());
        }
        let name = if name.starts_with('$') {
            name.to_string()
        } else {
            format!("${name}")
        };
        if self.variables.len() < 2 {
            return Err("a set needs at least two variables".into());
        }
        let label = self.label.trim();
        let mut text = format!(
            "MRSETS /{} NAME={name}",
            if self.dichotomy { "MDGROUP" } else { "MCGROUP" }
        );
        let from_variables =
            self.dichotomy && self.counted_value_labels && self.label_from_variables;
        if from_variables {
            text.push_str(" LABELSOURCE=VARLABEL");
        } else if !label.is_empty() {
            text.push_str(&format!(" LABEL={}", quote(label)));
        }
        if self.dichotomy && self.counted_value_labels {
            text.push_str(" CATEGORYLABELS=COUNTEDVALUES");
        }
        text.push_str(&format!("\n  VARIABLES={}", self.variables.join(" ")));
        if self.dichotomy {
            let counted = if string {
                value(self.value.trim_end(), true)?
            } else {
                let counted = value(&self.value, false)?;
                if counted.contains(['.', 'e']) {
                    return Err("the counted value must be a whole number".into());
                }
                counted
            };
            text.push_str(&format!(" VALUE={counted}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// SORT CASES by keys, each ascending or descending.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct SortCases {
    /// Variable and whether it sorts descending.
    pub keys: Vec<(String, bool)>,
}

impl SortCases {
    pub fn syntax(&self) -> Result<String, String> {
        if self.keys.is_empty() {
            return Err("add at least one variable to Sort by".into());
        }
        // a direction covers the variables written before it, back to the previous direction,
        // so it goes after each run of keys that share one; the last run needs it only when
        // descending, ascending being the default
        let mut text = String::from("SORT CASES BY");
        for (i, (name, descending)) in self.keys.iter().enumerate() {
            text.push(' ');
            text.push_str(name);
            match self.keys.get(i + 1) {
                Some((_, next)) if next == descending => {}
                Some(_) => text.push_str(if *descending { " (D)" } else { " (A)" }),
                None if *descending => text.push_str(" (D)"),
                None => {}
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// SELECT IF: keeps the cases where the condition holds and deletes the others.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct SelectCases {
    pub condition: String,
}

impl SelectCases {
    pub fn syntax(&self) -> Result<String, String> {
        match crate::dialogs::clause(&self.condition) {
            "" => Err("give the condition the kept cases meet".into()),
            condition => Ok(format!("SELECT IF {condition}.")),
        }
    }
}

/// WEIGHT BY a variable, or WEIGHT OFF.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct WeightCases {
    pub on: bool,
    pub variable: String,
}

impl WeightCases {
    /// The dialog as it opens: showing the weight variable in use, if any.
    pub fn current(weight: Option<&str>) -> Self {
        Self {
            on: weight.is_some(),
            variable: weight.unwrap_or_default().to_string(),
        }
    }

    pub fn syntax(&self) -> Result<String, String> {
        if !self.on {
            return Ok("WEIGHT OFF.".into());
        }
        match self.variable.trim() {
            "" => Err("choose the weight variable".into()),
            name => Ok(format!("WEIGHT BY {name}.")),
        }
    }
}

/// Keyword and label of every AGGREGATE function the dialog offers, in the order shown.
pub const AGGREGATE_FUNCTIONS: [(&str, &str); 10] = [
    ("MEAN", "Mean"),
    ("SUM", "Sum"),
    ("SD", "Standard deviation"),
    ("MEDIAN", "Median"),
    ("MIN", "Minimum"),
    ("MAX", "Maximum"),
    ("FIRST", "First value"),
    ("LAST", "Last value"),
    ("N", "Number of valid values"),
    ("NMISS", "Number of missing values"),
];

/// One aggregated variable: target = FUNCTION(source).
#[derive(Clone, PartialEq, Debug)]
pub struct Aggregated {
    pub target: String,
    /// Index into `AGGREGATE_FUNCTIONS`.
    pub function: usize,
    pub source: String,
}

impl Aggregated {
    /// The new variable for a source, named after it and the function: age_mean.
    pub fn new(function: usize, source: &str) -> Self {
        Self {
            target: format!(
                "{source}_{}",
                AGGREGATE_FUNCTIONS[function].0.to_lowercase()
            ),
            function,
            source: source.to_string(),
        }
    }
}

/// Where AGGREGATE puts its result.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum AggregateTo {
    /// Every case gets the values of its group.
    #[default]
    AddVariables,
    /// The dataset becomes one case per group.
    Replace,
    File,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Aggregate {
    pub breaks: Vec<String>,
    pub variables: Vec<Aggregated>,
    /// Also the number of cases of each group, under this name; empty for none.
    pub count: bool,
    pub count_name: String,
    pub to: AggregateTo,
    pub path: String,
}

impl Default for Aggregate {
    fn default() -> Self {
        Self {
            breaks: Vec::new(),
            variables: Vec::new(),
            count: false,
            count_name: "N_BREAK".into(),
            to: AggregateTo::AddVariables,
            path: String::new(),
        }
    }
}

impl Aggregate {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = match self.to {
            AggregateTo::AddVariables => "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES".to_string(),
            AggregateTo::Replace => "AGGREGATE /OUTFILE=*".to_string(),
            AggregateTo::File => match self.path.trim() {
                "" => return Err("choose the file to write".into()),
                path => format!("AGGREGATE /OUTFILE={}", quote(path)),
            },
        };
        if !self.breaks.is_empty() {
            text.push_str(&format!("\n  /BREAK={}", self.breaks.join(" ")));
        }
        let mut targets = Vec::new();
        for v in &self.variables {
            let target = v.target.trim();
            if target.is_empty() {
                return Err(format!(
                    "give a name to the {} of {}",
                    AGGREGATE_FUNCTIONS[v.function].1.to_lowercase(),
                    v.source
                ));
            }
            text.push_str(&format!(
                "\n  /{target} = {}({})",
                AGGREGATE_FUNCTIONS[v.function].0, v.source
            ));
            targets.push(target);
        }
        if self.count {
            let name = self.count_name.trim();
            if name.is_empty() {
                return Err("give a name to the number of cases".into());
            }
            text.push_str(&format!("\n  /{name} = N"));
            targets.push(name);
        }
        if targets.is_empty() {
            return Err("add at least one aggregated variable".into());
        }
        if let Some(twice) = targets.iter().enumerate().find_map(|(i, t)| {
            targets[..i]
                .iter()
                .any(|u| u.eq_ignore_ascii_case(t))
                .then_some(t)
        }) {
            return Err(format!("the name {twice} is used twice"));
        }
        text.push('.');
        Ok(text)
    }
}

/// ADD FILES (add cases) or MATCH FILES (add variables) of the active dataset and one file.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct MergeFiles {
    pub add_variables: bool,
    pub path: String,
    /// With add variables: the file is a lookup table that adds no cases.
    pub table: bool,
    /// With add variables: the key variables.
    pub by: Vec<String>,
}

impl MergeFiles {
    pub fn title(&self) -> &'static str {
        if self.add_variables {
            "Add Variables"
        } else {
            "Add Cases"
        }
    }

    pub fn syntax(&self) -> Result<String, String> {
        let path = match self.path.trim() {
            "" => return Err("choose the data file to add".into()),
            path => quote(path),
        };
        if !self.add_variables {
            return Ok(format!("ADD FILES /FILE=* /FILE={path}."));
        }
        let file = if self.table { "TABLE" } else { "FILE" };
        let mut text = format!("MATCH FILES /FILE=* /{file}={path}");
        if self.table || !self.by.is_empty() {
            text.push_str(&format!("\n  /BY {}", names(&self.by, "Key variables")?));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{fixture, open, run_in, v};

    #[test]
    fn define_and_delete_sets() {
        let mut d = Mrsets::default();
        assert_eq!(d.syntax(false).unwrap_err(), "give the set a name");
        d.name = "channels".into();
        d.variables = v(&["q5_1"]);
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "a set needs at least two variables"
        );
        d.variables = v(&["q5_1", "q5_2", "q5_3"]);
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "MRSETS /MDGROUP NAME=$channels\n  VARIABLES=q5_1 q5_2 q5_3 VALUE=1."
        );
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        let sets = session.dataset().unwrap().multiple_response_sets();
        assert_eq!(sets[0].name, "$channels");
        assert_eq!(sets[0].counted_value.as_deref(), Some("1"));

        d.label = "Contact channels".into();
        d.counted_value_labels = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "MRSETS /MDGROUP NAME=$channels LABEL='Contact channels' CATEGORYLABELS=COUNTEDVALUES\n  \
             VARIABLES=q5_1 q5_2 q5_3 VALUE=1."
        );
        // counted value labels must differ between the variables
        run_in(
            &mut session,
            "VALUE LABELS q5_1 1 'Phone' / q5_2 1 'Email' / q5_3 1 'Live chat'.",
        );
        run_in(&mut session, &syntax);
        d.label_from_variables = true;
        let syntax = d.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "MRSETS /MDGROUP NAME=$channels LABELSOURCE=VARLABEL CATEGORYLABELS=COUNTEDVALUES\n  \
             VARIABLES=q5_1 q5_2 q5_3 VALUE=1."
        );
        run_in(&mut session, &syntax);
        let set = &session.dataset().unwrap().multiple_response_sets()[0];
        assert!(set.counted_value_labels && set.label_from_variables);

        d.value = "1.5".into();
        assert_eq!(
            d.syntax(false).unwrap_err(),
            "the counted value must be a whole number"
        );

        let category = Mrsets {
            dichotomy: false,
            name: "$mentions".into(),
            label: "Providers".into(),
            variables: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
            ..Default::default()
        };
        let syntax = category.syntax(false).unwrap();
        assert_eq!(
            syntax,
            "MRSETS /MCGROUP NAME=$mentions LABEL='Providers'\n  \
             VARIABLES=q6_mention1 q6_mention2 q6_mention3."
        );
        run_in(&mut session, &syntax);
        assert_eq!(session.dataset().unwrap().multiple_response_sets().len(), 2);

        let strings = Mrsets {
            name: "$plans".into(),
            variables: v(&["plan", "q3_comment"]),
            value: "BASIC".into(),
            ..Default::default()
        };
        let syntax = strings.syntax(true).unwrap();
        assert!(syntax.ends_with("VARIABLES=plan q3_comment VALUE='BASIC'."));
        run_in(&mut session, &syntax);

        let mut delete = Mrsets {
            delete: true,
            ..Default::default()
        };
        assert_eq!(
            delete.syntax(false).unwrap_err(),
            "choose the sets to delete"
        );
        delete.sets = v(&["$channels", "$plans"]);
        let syntax = delete.syntax(false).unwrap();
        assert_eq!(syntax, "MRSETS /DELETE NAME=[$channels $plans].");
        run_in(&mut session, &syntax);
        let sets = session.dataset().unwrap().multiple_response_sets();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].name, "$mentions");
    }

    #[test]
    fn sort_cases() {
        let mut d = SortCases::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Sort by"
        );
        d.keys = vec![("region".into(), false), ("gender".into(), false)];
        assert_eq!(d.syntax().unwrap(), "SORT CASES BY region gender.");
        d.keys.push(("age".into(), true));
        d.keys.push(("resp_id".into(), true));
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "SORT CASES BY region gender (A) age resp_id (D).");
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        d.keys = vec![("age".into(), true), ("resp_id".into(), false)];
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "SORT CASES BY age (D) resp_id.");
        run_in(&mut session, &syntax);
        let data = session.dataset().unwrap();
        let age = data.variable("age").unwrap();
        let ages: Vec<f64> = age.numeric().unwrap().iter().flatten().collect();
        assert!(ages.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn select_cases() {
        let mut d = SelectCases::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the condition the kept cases meet"
        );
        d.condition = " age >= 40. ".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "SELECT IF age >= 40.");
        let mut session = open("survey_basic.sav");
        let before = session.dataset().unwrap().case_count();
        run_in(&mut session, &syntax);
        let after = session.dataset().unwrap().case_count();
        assert!(after > 0 && after < before);
    }

    #[test]
    fn weight_cases() {
        let mut d = WeightCases::current(None);
        assert_eq!(d.syntax().unwrap(), "WEIGHT OFF.");
        d.on = true;
        assert_eq!(d.syntax().unwrap_err(), "choose the weight variable");
        d.variable = "weight".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "WEIGHT BY weight.");
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        let data = session.dataset().unwrap();
        assert_eq!(data.weight_variable(), Some("weight"));
        assert_eq!(WeightCases::current(data.weight_variable()), d);
        run_in(&mut session, "WEIGHT OFF.");
        assert_eq!(session.dataset().unwrap().weight_variable(), None);
    }

    #[test]
    fn aggregate() {
        let mut d = Aggregate::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one aggregated variable"
        );
        d.breaks = v(&["region"]);
        d.variables = vec![
            Aggregated::new(0, "age"),
            Aggregated::new(8, "monthly_spend"),
        ];
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES\n  /BREAK=region\n  \
             /age_mean = MEAN(age)\n  /monthly_spend_n = N(monthly_spend)."
        );
        let mut session = open("survey_basic.sav");
        let cases = session.dataset().unwrap().case_count();
        run_in(&mut session, &syntax);
        let data = session.dataset().unwrap();
        assert_eq!(data.case_count(), cases);
        assert!(data.variable("age_mean").is_ok());

        d.variables[1].target = "AGE_MEAN".into();
        assert_eq!(d.syntax().unwrap_err(), "the name AGE_MEAN is used twice");
        d.variables[1].target = " ".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give a name to the number of valid values of monthly_spend"
        );
        d.variables.clear();
        d.count = true;
        d.to = AggregateTo::Replace;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "AGGREGATE /OUTFILE=*\n  /BREAK=region\n  /N_BREAK = N."
        );
        run_in(&mut session, &syntax);
        let data = session.dataset().unwrap();
        assert_eq!(data.variable_count(), 2);
        assert!(data.case_count() <= 6);

        d.to = AggregateTo::File;
        assert_eq!(d.syntax().unwrap_err(), "choose the file to write");
        let path = std::env::temp_dir().join(format!(
            "statscraft-gui-aggregate-{}.sav",
            std::process::id()
        ));
        d.path = path.to_string_lossy().into_owned();
        d.count_name = "groups".into();
        d.breaks.clear();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            format!("AGGREGATE /OUTFILE={}\n  /groups = N.", quote(&d.path))
        );
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn merge_files() {
        let mut d = MergeFiles::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the data file to add");
        d.path = fixture("survey_basic_phpwriter.sav");
        let path = quote(&d.path);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, format!("ADD FILES /FILE=* /FILE={path}."));
        let mut session = open("survey_basic.sav");
        let cases = session.dataset().unwrap().case_count();
        run_in(&mut session, &syntax);
        assert!(session.dataset().unwrap().case_count() > cases);

        d.add_variables = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, format!("MATCH FILES /FILE=* /FILE={path}."));
        let mut session = open("survey_basic.sav");
        run_in(&mut session, &syntax);
        assert_eq!(session.dataset().unwrap().case_count(), cases);

        d.by = v(&["resp_id"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            format!("MATCH FILES /FILE=* /FILE={path}\n  /BY resp_id.")
        );
        run_in(&mut session, &syntax);

        d.table = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            format!("MATCH FILES /FILE=* /TABLE={path}\n  /BY resp_id.")
        );
        run_in(&mut session, &syntax);
        assert_eq!(session.dataset().unwrap().case_count(), cases);
        d.by.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Key variables"
        );
    }
}
