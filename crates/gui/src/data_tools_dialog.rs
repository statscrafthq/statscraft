//! The data utility dialogs: Validate Data (VALIDATEDATA), Compare Datasets (COMPARE DATASETS)
//! and Apply Data Dictionary (APPLY DICTIONARY). Options left as they are add nothing to the
//! syntax, so the command runs with its own defaults.

use crate::dialogs::whole;
use crate::i18n::{t, t_args};
use crate::proximities_dialog::output_target;
use crate::stats_dialogs::parse;
use crate::syntax_text::{names, number, quote};
use statscraft_data::{Dataset, is_valid_name};

/// A name for a variable or dataset that the command will make; `None` when left empty.
fn new_name(text: &str, what: &str) -> Result<Option<String>, String> {
    match text.trim() {
        "" => Ok(None),
        name if is_valid_name(name) => Ok(Some(name.to_string())),
        name => Err(t_args(
            "core-cannot-name",
            &[("name", name.into()), ("what", what.into())],
        )),
    }
}

/// A threshold as `KEYWORD=value`, left out when it is the default. A percentage is above 0 and
/// at most 100, any other threshold 0 or more.
fn threshold(
    text: &str,
    what: &str,
    keyword: &str,
    percent: bool,
    default: f64,
) -> Result<Option<String>, String> {
    let n = parse(text, what)?;
    if percent && !(n > 0.0 && n <= 100.0) {
        return Err(t_args("core-percent-range", &[("what", what.into())]));
    }
    if n < 0.0 {
        return Err(t_args("core-zero-or-more", &[("what", what.into())]));
    }
    Ok((n != default).then(|| format!("{keyword}={}", number(n))))
}

/// VALIDATEDATA: the variables to check, the IDs, and the checks, reports and indicator
/// variables around them.
#[derive(Clone, PartialEq, Debug)]
pub struct DataValidation {
    pub variables: Vec<String>,
    pub ids: Vec<String>,
    /// Cross-variable rules by number, or ALL, as typed.
    pub rules: String,
    pub var_checks: bool,
    pub pct_missing: String,
    pub pct_equal: String,
    pub pct_unequal: String,
    pub cv: String,
    pub std_dev: String,
    pub incomplete_ids: bool,
    pub duplicate_ids: bool,
    pub report_empty: bool,
    /// Empty cases count every variable but the ID, not just the analysis variables.
    pub all_variables: bool,
    pub by_variable: bool,
    pub by_rule: bool,
    pub case_report: bool,
    pub min_violations: String,
    /// Empty for all the cases.
    pub case_limit: String,
    pub save: [bool; 4],
    /// The names of the saved variables; empty for the command's own names.
    pub save_names: [String; 4],
}

/// Keyword and label of each indicator variable VALIDATEDATA can save.
pub const VALIDATE_SAVE: [(&str, &str); 4] = [
    ("EMPTYCASE", "core-empty-case"),
    ("INCOMPLETEID", "core-incomplete-id"),
    ("DUPLICATEID", "core-duplicate-id"),
    ("RULEVIOLATIONS", "core-rule-violations"),
];

impl Default for DataValidation {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            ids: Vec::new(),
            rules: String::new(),
            var_checks: true,
            pct_missing: "70".into(),
            pct_equal: "95".into(),
            pct_unequal: "90".into(),
            cv: "0.001".into(),
            std_dev: "0".into(),
            incomplete_ids: true,
            duplicate_ids: true,
            report_empty: true,
            all_variables: false,
            by_variable: true,
            by_rule: false,
            case_report: true,
            min_violations: "1".into(),
            case_limit: "500".into(),
            save: [false; 4],
            save_names: Default::default(),
        }
    }
}

impl DataValidation {
    pub fn syntax(&self) -> Result<String, String> {
        let rules: Vec<&str> = self.rules.split_whitespace().collect();
        if self.variables.is_empty() && self.ids.is_empty() && rules.is_empty() {
            return Err(t("core-validate-empty").into());
        }
        let mut text = String::from("VALIDATEDATA");
        if !self.variables.is_empty() {
            text.push_str(&format!(" VARIABLES={}", self.variables.join(" ")));
        }
        if !self.ids.is_empty() {
            text.push_str(&format!(" ID={}", self.ids.join(" ")));
        }
        if !rules.is_empty() {
            if let Some(rule) = rules
                .iter()
                .find(|r| !r.eq_ignore_ascii_case("ALL") && r.parse::<u32>().is_err())
            {
                return Err(t_args("core-bad-rule", &[("rule", (*rule).into())]));
            }
            text.push_str(&format!(" CROSSVARRULES={}", rules.join(" ")));
        }
        let mut lines = Vec::new();
        if !self.var_checks {
            lines.push("/VARCHECKS STATUS=OFF".to_string());
        } else {
            let options: Vec<String> = [
                threshold(
                    &self.pct_missing,
                    t("core-pct-missing"),
                    "PCTMISSING",
                    true,
                    70.0,
                )?,
                threshold(
                    &self.pct_equal,
                    t("core-pct-largest"),
                    "PCTEQUAL",
                    true,
                    95.0,
                )?,
                threshold(
                    &self.pct_unequal,
                    t("core-pct-single"),
                    "PCTUNEQUAL",
                    true,
                    90.0,
                )?,
                threshold(&self.cv, t("core-cv"), "CV", false, 0.001)?,
                threshold(&self.std_dev, t("core-std-deviation"), "STDDEV", false, 0.0)?,
            ]
            .into_iter()
            .flatten()
            .collect();
            if !options.is_empty() {
                lines.push(format!("/VARCHECKS {}", options.join(" ")));
            }
        }
        match (self.incomplete_ids, self.duplicate_ids) {
            (true, true) => {}
            (false, false) => lines.push("/IDCHECKS NONE".into()),
            (true, false) => lines.push("/IDCHECKS INCOMPLETE".into()),
            (false, true) => lines.push("/IDCHECKS DUPLICATE".into()),
        }
        let mut case_checks = Vec::new();
        if !self.report_empty {
            case_checks.push("REPORTEMPTY=NO");
        }
        if self.all_variables {
            case_checks.push("SCOPE=ALLVARS");
        }
        if !case_checks.is_empty() {
            lines.push(format!("/CASECHECKS {}", case_checks.join(" ")));
        }
        match (self.by_variable, self.by_rule) {
            (true, false) => {}
            (false, false) => lines.push("/RULESUMMARIES NONE".into()),
            (true, true) => lines.push("/RULESUMMARIES BYVARIABLE BYRULE".into()),
            (false, true) => lines.push("/RULESUMMARIES BYRULE".into()),
        }
        let mut report = Vec::new();
        if !self.case_report {
            report.push("DISPLAY=NO".to_string());
        }
        if let Some(n) = whole(&self.min_violations, 1, t("core-least-violations"))?
            && n != 1
        {
            report.push(format!("MINVIOLATIONS={n}"));
        }
        match whole(&self.case_limit, 1, t("core-cases-listed"))? {
            None => report.push("CASELIMIT=NONE".into()),
            Some(500) => {}
            Some(n) => report.push(format!("CASELIMIT=FIRSTN({n})")),
        }
        if !report.is_empty() {
            lines.push(format!("/CASEREPORT {}", report.join(" ")));
        }
        let mut save = Vec::new();
        for (i, (keyword, _)) in VALIDATE_SAVE
            .iter()
            .enumerate()
            .filter(|(i, _)| self.save[*i])
        {
            match new_name(&self.save_names[i], t("core-a-variable"))? {
                Some(name) => save.push(format!("{keyword}({name})")),
                None => save.push((*keyword).to_string()),
            }
        }
        if !save.is_empty() {
            lines.push(format!("/SAVE {}", save.join(" ")));
        }
        for line in lines {
            text.push_str("\n  ");
            text.push_str(&line);
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of each dictionary property COMPARE DATASETS can compare.
pub const COMPARE_PROPERTIES: [(&str, &str); 9] = [
    ("MEASURE", "core-measurement-level"),
    ("LABEL", "core-variable-label"),
    ("VALUELABELS", "data-field-values"),
    ("COLUMNS", "core-column-width"),
    ("MISSING", "data-field-missing"),
    ("ALIGN", "data-field-align"),
    ("ROLE", "data-field-role"),
    ("ATTRIBUTES", "core-attributes"),
    ("WIDTH", "core-format-width"),
];

/// COMPARE DATASETS: the other file or dataset, how cases pair, what to compare, and what to
/// write about the differences.
#[derive(Clone, PartialEq, Debug)]
pub struct DatasetComparison {
    /// The other data is an open dataset, else a data file.
    pub open_dataset: bool,
    pub file: String,
    pub dataset: String,
    /// Empty compares every variable.
    pub variables: Vec<String>,
    pub ids: Vec<String>,
    pub flag: bool,
    pub flag_name: String,
    /// Where the matching cases go: 0 nowhere, 1 a data file, 2 a new dataset.
    pub matches: usize,
    pub match_file: String,
    pub match_dataset: String,
    /// The same for the cases that differ or have no partner.
    pub mismatches: usize,
    pub mismatch_file: String,
    pub mismatch_dataset: String,
    pub properties: [bool; 9],
    pub case_table: bool,
    /// Empty for all the cases.
    pub table_limit: String,
}

impl Default for DatasetComparison {
    fn default() -> Self {
        Self {
            open_dataset: false,
            file: String::new(),
            dataset: String::new(),
            variables: Vec::new(),
            ids: Vec::new(),
            flag: true,
            flag_name: String::new(),
            matches: 0,
            match_file: String::new(),
            match_dataset: String::new(),
            mismatches: 0,
            mismatch_file: String::new(),
            mismatch_dataset: String::new(),
            properties: [false; 9],
            case_table: true,
            table_limit: "100".into(),
        }
    }
}

impl DatasetComparison {
    pub fn syntax(&self) -> Result<String, String> {
        let other = match self.open_dataset {
            true => match new_name(&self.dataset, t("core-a-dataset"))? {
                Some(name) => name,
                None => return Err(t("core-name-compare-dataset").into()),
            },
            false => match self.file.trim() {
                "" => return Err(t("core-choose-compare-file").into()),
                path => quote(path),
            },
        };
        let variables = match self.variables.is_empty() {
            true => "ALL".to_string(),
            false => names(&self.variables, t("core-variables-to-compare"))?,
        };
        let mut declare = Vec::new();
        let mut targets = Vec::new();
        for (kind, choice, file, dataset, what) in [
            (
                "MATCH",
                self.matches,
                &self.match_file,
                &self.match_dataset,
                t("core-matching-cases"),
            ),
            (
                "MISMATCH",
                self.mismatches,
                &self.mismatch_file,
                &self.mismatch_dataset,
                t("core-differing-cases"),
            ),
        ] {
            if let Some(target) = output_target(choice, file, dataset, what)? {
                if choice == 2 {
                    declare.push(target.clone());
                }
                targets.push(format!("{kind}DATASET=YES {kind}NAME={target}"));
            }
        }
        if declare.len() == 2 && declare[0] == declare[1] {
            return Err(t("core-two-names").into());
        }
        let mut text: String = declare
            .iter()
            .map(|d| format!("DATASET DECLARE {d}.\n"))
            .collect();
        text.push_str(&format!("COMPARE DATASETS /COMPDATASET {other}"));
        text.push_str(&format!("\n  /VARIABLES {variables}"));
        if !self.ids.is_empty() {
            text.push_str(&format!("\n  /CASEID {}", self.ids.join(" ")));
        }
        let mut save = Vec::new();
        if !self.flag {
            save.push("FLAGMISMATCHES=NO".to_string());
        } else if let Some(name) = new_name(&self.flag_name, t("core-a-variable"))? {
            save.push(format!("VARNAME={name}"));
        }
        save.extend(targets);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        let mut output = Vec::new();
        let picked: Vec<&str> = COMPARE_PROPERTIES
            .iter()
            .zip(self.properties)
            .filter(|(_, on)| *on)
            .map(|((keyword, _), _)| *keyword)
            .collect();
        if !picked.is_empty() {
            output.push(format!("VARPROPERTIES={}", picked.join(" ")));
        }
        if !self.case_table {
            output.push("CASETABLE=NO".into());
        } else {
            match whole(&self.table_limit, 1, t("core-cases-shown"))? {
                None => output.push("TABLELIMIT=NONE".into()),
                Some(100) => {}
                Some(n) => output.push(format!("TABLELIMIT={n}")),
            }
        }
        if !output.is_empty() {
            text.push_str(&format!("\n  /OUTPUT {}", output.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of each property of a variable APPLY DICTIONARY can copy, with whether
/// the command copies it when /VARINFO is left out.
pub const COPY_VARINFO: [(&str, &str, bool); 9] = [
    ("VARLABEL", "core-variable-labels", true),
    ("VALLABELS", "data-field-values", true),
    ("MISSING", "data-field-missing", true),
    ("FORMATS", "core-formats", true),
    ("WIDTH", "core-column-widths", true),
    ("ALIGNMENT", "data-field-align", true),
    ("LEVEL", "core-measurement-levels", true),
    ("ROLE", "core-roles", true),
    ("ATTRIBUTES", "core-attributes", false),
];

/// The same for the properties of the file, as `/FILEINFO` leaves them out.
pub const COPY_FILEINFO: [(&str, &str, bool); 6] = [
    ("WEIGHT", "core-weight", true),
    ("FILELABEL", "core-file-label", false),
    ("DOCUMENTS", "core-documents", false),
    ("ATTRIBUTES", "core-file-attributes", false),
    ("MRSETS", "core-mr-sets", false),
    ("VARSETS", "core-variable-sets", false),
];

/// Whether a property can be merged into what the active data has, instead of replacing it.
pub fn can_merge(keyword: &str) -> bool {
    matches!(keyword, "VALLABELS" | "ATTRIBUTES" | "DOCUMENTS")
}

/// APPLY DICTIONARY: the source, the properties to copy, and the variables they go to.
#[derive(Clone, PartialEq, Debug)]
pub struct DictionaryCopy {
    pub open_dataset: bool,
    pub file: String,
    pub dataset: String,
    pub varinfo: [bool; 9],
    pub file_info: [bool; 6],
    /// Merge instead of replace, for the properties that allow it, in the order of
    /// `COPY_VARINFO` and `COPY_FILEINFO`.
    pub var_merge: [bool; 9],
    pub file_merge: [bool; 6],
    /// Empty: each variable takes the dictionary of the variable of its name.
    pub source_variable: String,
    pub targets: Vec<String>,
    pub new_variables: bool,
}

impl Default for DictionaryCopy {
    fn default() -> Self {
        Self {
            open_dataset: false,
            file: String::new(),
            dataset: String::new(),
            varinfo: COPY_VARINFO.map(|e| e.2),
            file_info: COPY_FILEINFO.map(|e| e.2),
            var_merge: [false; 9],
            file_merge: [false; 6],
            source_variable: String::new(),
            targets: Vec::new(),
            new_variables: false,
        }
    }
}

/// The checked keywords of a list as ` KEYWORD` or ` KEYWORD=MERGE`.
fn keywords(list: &[(&str, &str, bool)], on: &[bool], merge: &[bool]) -> String {
    list.iter()
        .enumerate()
        .filter(|(i, _)| on[*i])
        .map(|(i, e)| match can_merge(e.0) && merge[i] {
            true => format!(" {}=MERGE", e.0),
            false => format!(" {}", e.0),
        })
        .collect()
}

impl DictionaryCopy {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let from = match self.open_dataset {
            true => match new_name(&self.dataset, t("core-a-dataset"))? {
                Some(name) => name,
                None => return Err(t("core-name-copy-dataset").into()),
            },
            false => match self.file.trim() {
                "" => return Err(t("core-choose-copy-file").into()),
                path => quote(path),
            },
        };
        let mut text = format!("APPLY DICTIONARY FROM {from}");
        let source = self.source_variable.trim();
        if !source.is_empty() {
            if self.targets.is_empty() {
                return Err(t_args("core-add-targets", &[("source", source.into())]));
            }
            if !is_valid_name(source) {
                return Err(t_args("core-not-a-name", &[("name", source.into())]));
            }
            text.push_str(&format!("\n  /SOURCE VARIABLES={source}"));
        }
        if !self.targets.is_empty() {
            if let Some(gone) = self.targets.iter().find(|n| data.variable(n).is_err()) {
                return Err(t_args("core-gone", &[("name", gone.as_str().into())]));
            }
            text.push_str(&format!("\n  /TARGET VARIABLES={}", self.targets.join(" ")));
        }
        // with one source variable the command adds no variables
        if self.new_variables && source.is_empty() {
            text.push_str("\n  /NEWVARS");
        }
        if self.file_info != COPY_FILEINFO.map(|e| e.2) || self.file_merge.iter().any(|m| *m) {
            let list = keywords(&COPY_FILEINFO, &self.file_info, &self.file_merge);
            if list.is_empty() {
                // an empty /FILEINFO is the same as none, which copies the weight
                return Err(t("core-check-file-property").into());
            }
            text.push_str(&format!("\n  /FILEINFO{list}"));
        }
        if self.varinfo != COPY_VARINFO.map(|e| e.2) || self.var_merge.iter().any(|m| *m) {
            let list = keywords(&COPY_VARINFO, &self.varinfo, &self.var_merge);
            text.push_str(&format!("\n  /VARINFO{list}"));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn validation_needs_something_to_check_and_stays_short() {
        assert!(DataValidation::default().syntax().is_err());
        let d = DataValidation {
            variables: v(&["age", "income"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap(), "VALIDATEDATA VARIABLES=age income.");
    }

    #[test]
    fn validation_writes_every_changed_option() {
        let d = DataValidation {
            variables: v(&["age"]),
            ids: v(&["household", "member"]),
            rules: "1 2".into(),
            pct_missing: "40".into(),
            cv: "0.01".into(),
            incomplete_ids: false,
            report_empty: false,
            all_variables: true,
            by_variable: false,
            by_rule: true,
            min_violations: "2".into(),
            case_limit: String::new(),
            save: [true, false, true, false],
            save_names: ["", "", "dupgroup", ""].map(String::from),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "VALIDATEDATA VARIABLES=age ID=household member CROSSVARRULES=1 2\n  \
             /VARCHECKS PCTMISSING=40 CV=0.01\n  /IDCHECKS DUPLICATE\n  \
             /CASECHECKS REPORTEMPTY=NO SCOPE=ALLVARS\n  /RULESUMMARIES BYRULE\n  \
             /CASEREPORT MINVIOLATIONS=2 CASELIMIT=NONE\n  /SAVE EMPTYCASE DUPLICATEID(dupgroup)."
        );
        let off = DataValidation {
            var_checks: false,
            duplicate_ids: false,
            ..d
        };
        assert!(
            off.syntax()
                .unwrap()
                .contains("/VARCHECKS STATUS=OFF\n  /IDCHECKS NONE")
        );
        let bad = DataValidation {
            variables: v(&["a"]),
            pct_equal: "120".into(),
            ..Default::default()
        };
        assert!(bad.syntax().is_err());
    }

    #[test]
    fn comparison_names_the_file_and_the_outputs() {
        assert!(DatasetComparison::default().syntax().is_err());
        let d = DatasetComparison {
            file: "wave1.sav".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "COMPARE DATASETS /COMPDATASET 'wave1.sav'\n  /VARIABLES ALL."
        );
        let d = DatasetComparison {
            file: "it's.sav".into(),
            variables: v(&["q1", "q2"]),
            ids: v(&["respid"]),
            flag_name: "changed".into(),
            mismatches: 2,
            mismatch_dataset: "review".into(),
            properties: [false, true, true, false, false, false, false, false, false],
            table_limit: "50".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "DATASET DECLARE review.\n\
             COMPARE DATASETS /COMPDATASET 'it''s.sav'\n  /VARIABLES q1 q2\n  /CASEID respid\n  \
             /SAVE VARNAME=changed MISMATCHDATASET=YES MISMATCHNAME=review\n  \
             /OUTPUT VARPROPERTIES=LABEL VALUELABELS TABLELIMIT=50."
        );
        let d = DatasetComparison {
            open_dataset: true,
            dataset: "other".into(),
            flag: false,
            case_table: false,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "COMPARE DATASETS /COMPDATASET other\n  /VARIABLES ALL\n  \
             /SAVE FLAGMISMATCHES=NO\n  /OUTPUT CASETABLE=NO."
        );
    }

    #[test]
    fn applying_a_dictionary_copies_what_is_checked() {
        let data = Dataset::new(0);
        assert!(DictionaryCopy::default().syntax(&data).is_err());
        let mut d = DictionaryCopy {
            file: "template.sav".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "APPLY DICTIONARY FROM 'template.sav'."
        );
        d.varinfo = [true, true, false, false, false, false, false, false, true];
        d.var_merge[1] = true;
        d.file_info[2] = true;
        d.file_merge[2] = true;
        d.new_variables = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "APPLY DICTIONARY FROM 'template.sav'\n  /NEWVARS\n  /FILEINFO WEIGHT DOCUMENTS=MERGE\n  \
             /VARINFO VARLABEL VALLABELS=MERGE ATTRIBUTES."
        );
        d.file_info = [false; 6];
        assert!(d.syntax(&data).is_err());
        let d = DictionaryCopy {
            file: "t.sav".into(),
            source_variable: "q1".into(),
            ..Default::default()
        };
        assert!(d.syntax(&data).is_err());
    }
}
