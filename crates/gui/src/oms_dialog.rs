//! The Output Management dialogs: OMS starts a request that sends chosen output to a file, and
//! OMSEND ends requests. Options left as they are add nothing to the syntax.

use crate::i18n::t;
use crate::syntax_text::quote;

/// Keyword and label of the kinds of output a request can choose.
pub const OMS_KINDS: [(&str, &str); 4] = [
    ("TABLES", "settings-part-tables"),
    ("CHARTS", "settings-part-charts"),
    ("WARNINGS", "core-warnings"),
    ("TEXTS", "core-oms-texts"),
];

/// Keyword, label and file extensions of each destination format.
pub const OMS_FORMATS: [(&str, &str, &[&str]); 7] = [
    ("SAV", "core-oms-sav", &["sav"]),
    ("HTML", "HTML", &["html", "htm"]),
    ("TEXT", "core-oms-text", &["txt"]),
    ("TABTEXT", "core-oms-tab-text", &["txt", "tsv"]),
    ("XLSX", "Excel", &["xlsx"]),
    ("DOC", "Word", &["docx", "doc"]),
    ("PDF", "PDF", &["pdf"]),
];

/// OMS: which output of the commands that follow goes where.
#[derive(Clone, PartialEq, Debug)]
pub struct Oms {
    pub kinds: [bool; 4],
    /// Command names such as Frequencies, separated by commas; empty: every command.
    pub commands: String,
    /// Table titles, separated by commas; empty: every table.
    pub subtypes: String,
    /// Index into `OMS_FORMATS`.
    pub format: usize,
    /// Empty: no file is written.
    pub file: String,
    /// Whether the chosen output still shows in the results.
    pub viewer: bool,
    pub tag: String,
}

impl Default for Oms {
    fn default() -> Self {
        Self {
            kinds: [true, false, false, false],
            commands: String::new(),
            subtypes: String::new(),
            format: 0,
            file: String::new(),
            viewer: true,
            tag: String::new(),
        }
    }
}

/// The texts of a comma separated field as a bracket list of quoted texts; None when empty.
fn bracket(text: &str) -> Option<String> {
    let items: Vec<String> = text
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(quote)
        .collect();
    (!items.is_empty()).then(|| format!("[{}]", items.join(" ")))
}

impl Oms {
    pub fn syntax(&self) -> Result<String, String> {
        let kinds: Vec<&str> = OMS_KINDS
            .iter()
            .zip(self.kinds)
            .filter_map(|((k, _), on)| on.then_some(*k))
            .collect();
        if kinds.is_empty() {
            return Err(t("core-oms-no-kind").into());
        }
        let file = self.file.trim();
        if file.is_empty() && self.viewer {
            return Err(t("core-oms-no-file-error").into());
        }
        let mut text = format!("OMS /SELECT {}", kinds.join(" "));
        let mut conditions = Vec::new();
        if let Some(list) = bracket(&self.commands) {
            conditions.push(format!("COMMANDS={list}"));
        }
        if let Some(list) = bracket(&self.subtypes) {
            conditions.push(format!("SUBTYPES={list}"));
        }
        if !conditions.is_empty() {
            text.push_str(&format!("\n  /IF {}", conditions.join(" ")));
        }
        text.push_str("\n  /DESTINATION");
        if !file.is_empty() {
            text.push_str(&format!(
                " FORMAT={} OUTFILE={}",
                OMS_FORMATS[self.format].0,
                quote(file)
            ));
        }
        if !self.viewer {
            text.push_str(" VIEWER=NO");
        }
        let tag = self.tag.trim();
        if !tag.is_empty() {
            if tag.starts_with('$') {
                return Err(t("core-oms-tag-dollar").into());
            }
            text.push_str(&format!("\n  /TAG={}", quote(tag)));
        }
        text.push('.');
        Ok(text)
    }
}

/// OMSEND: ends the requests of some tags or files, or all of them.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Omsend {
    /// Tags separated by commas.
    pub tags: String,
    /// Files separated by commas.
    pub files: String,
}

impl Omsend {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = String::from("OMSEND");
        if let Some(list) = bracket(&self.tags) {
            text.push_str(&format!(" TAG={list}"));
        }
        if let Some(list) = bracket(&self.files) {
            text.push_str(&format!(" FILE={list}"));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oms_writes_a_request() {
        let d = Oms {
            file: "stats.sav".into(),
            commands: "Descriptives, T-Test".into(),
            subtypes: "Descriptive Statistics".into(),
            viewer: false,
            tag: "desc".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "OMS /SELECT TABLES\n  /IF COMMANDS=['Descriptives' 'T-Test'] SUBTYPES=['Descriptive Statistics']\n  \
             /DESTINATION FORMAT=SAV OUTFILE='stats.sav' VIEWER=NO\n  /TAG='desc'."
        );
        let d = Oms {
            kinds: [true, true, true, true],
            format: 4,
            file: "all.xlsx".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap(),
            "OMS /SELECT TABLES CHARTS WARNINGS TEXTS\n  /DESTINATION FORMAT=XLSX OUTFILE='all.xlsx'."
        );
        let hide = Oms {
            viewer: false,
            ..Default::default()
        };
        assert_eq!(
            hide.syntax().unwrap(),
            "OMS /SELECT TABLES\n  /DESTINATION VIEWER=NO."
        );
    }

    #[test]
    fn oms_names_what_is_missing() {
        let err = |change: &dyn Fn(&mut Oms)| {
            let mut d = Oms {
                file: "a.sav".into(),
                ..Default::default()
            };
            change(&mut d);
            d.syntax().unwrap_err()
        };
        assert!(err(&|d| d.kinds = [false; 4]).contains("kind of output"));
        assert!(err(&|d| d.file.clear()).contains("choose a file"));
        assert!(err(&|d| d.tag = "$x".into()).contains("dollar"));
    }

    #[test]
    fn omsend_ends_by_tag_file_or_everything() {
        assert_eq!(Omsend::default().syntax().unwrap(), "OMSEND.");
        let d = Omsend {
            tags: "desc, other".into(),
            files: "a.sav".into(),
        };
        assert_eq!(
            d.syntax().unwrap(),
            "OMSEND TAG=['desc' 'other'] FILE=['a.sav']."
        );
    }
}
