//! The Reports dialogs: Case Summaries (SUMMARIZE), Codebook (CODEBOOK) and OLAP Cubes. Options left as
//! they are add nothing to the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::{MEANS_CELLS, Means, keywords};
use crate::mds_dialog::count;
use crate::syntax_text::{names, quote};
use statscraft_data::is_valid_name;

/// SUMMARIZE: the variables, groups, cells and analysis of variance as the Means dialog has
/// them, and the listing, totals, title and footnote of its own.
#[derive(Clone, PartialEq, Debug)]
pub struct Summarize {
    pub means: Means,
    /// FORMAT=LIST: the cases are listed too.
    pub list: bool,
    /// VALIDLIST: only cases valid on every summarized variable are listed.
    pub valid_only: bool,
    /// LIMIT: only the first cases count; empty for all.
    pub limit: String,
    pub case_numbers: bool,
    /// TOTAL: the total rows say so; off writes NOTOTAL.
    pub total_label: bool,
    /// What a system-missing value shows as in the listing; empty for the period.
    pub missing_text: String,
    /// MISSING=TABLE: a case missing on any summarized variable is left out of all of them.
    pub tablewise: bool,
    pub title: String,
    pub footnote: String,
}

impl Default for Summarize {
    fn default() -> Self {
        Self {
            means: Means::default(),
            list: false,
            valid_only: false,
            limit: String::new(),
            case_numbers: true,
            total_label: true,
            missing_text: String::new(),
            tablewise: false,
            title: String::new(),
            footnote: String::new(),
        }
    }
}

impl Summarize {
    pub fn syntax(&self) -> Result<String, String> {
        // MEANS writes the tables, cells and statistics the same way SUMMARIZE reads them
        let means = self.means.syntax()?;
        let body = means
            .strip_prefix("MEANS")
            .and_then(|t| t.strip_suffix('.'))
            .expect("MEANS syntax");
        let mut controls = Vec::new();
        for name in self.means.layers.iter().flatten() {
            if controls.contains(&name) {
                return Err(format!("{name} is in two layers; keep it in one"));
            }
            controls.push(name);
        }
        let mut text = format!("SUMMARIZE{body}");
        let mut format = Vec::new();
        if self.list {
            let mut list = String::from(if self.valid_only { "VALIDLIST" } else { "LIST" });
            if !self.limit.trim().is_empty() {
                let limit = count(&self.limit, 1, "the number of cases used", 1)?;
                list.push_str(&format!(" LIMIT={limit}"));
            }
            format.push(list);
            if !self.case_numbers {
                format.push("NOCASENUM".into());
            }
            if !self.missing_text.is_empty() {
                format.push(format!("MISSING={}", quote(&self.missing_text)));
            }
        }
        if !self.total_label {
            format.push("NOTOTAL".into());
        }
        if !format.is_empty() {
            text.push_str(&format!("\n  /FORMAT={}", format.join(" ")));
        }
        if self.tablewise {
            text.push_str("\n  /MISSING=TABLE");
        }
        for (what, value, keyword) in [
            ("the title", &self.title, "TITLE"),
            ("the footnote", &self.footnote, "FOOTNOTE"),
        ] {
            if value.len() > 255 {
                return Err(format!("{what} is longer than 255 bytes"));
            }
            if !value.is_empty() {
                text.push_str(&format!("\n  /{keyword}={}", quote(value)));
            }
        }
        text.push('.');
        Ok(text)
    }
}

/// What OLAP CUBES shows without /CELLS (MEAN, COUNT, STDDEV, SUM, NPCT, SPCT), as indices of
/// `MEANS_CELLS`.
const OLAP_DEFAULT_CELLS: [usize; 6] = [0, 1, 2, 6, 17, 18];

fn olap_default() -> [bool; 21] {
    let mut cells = [false; 21];
    for i in OLAP_DEFAULT_CELLS {
        cells[i] = true;
    }
    cells
}

/// The differences CREATE adds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OlapDifference {
    None,
    /// Between two categories of one grouping variable (GAC, GPC).
    Groups,
    /// Between two summary variables (VAC, VPC).
    Variables,
}

#[derive(Clone, PartialEq, Debug)]
pub struct OlapCubes {
    pub summary: Vec<String>,
    /// The grouping variables of every BY; each is nested in the one before.
    pub layers: Vec<Vec<String>>,
    /// The layer the dialog shows.
    pub layer: usize,
    /// One flag per entry of `MEANS_CELLS`; none checked writes NONE.
    pub cells: [bool; 21],
    pub difference: OlapDifference,
    /// The grouping variable whose categories are compared.
    pub group: String,
    /// Pairs of category values or of summary variable names, as typed.
    pub pairs: Vec<[String; 2]>,
    pub absolute: bool,
    pub percent: bool,
    pub hide_small: bool,
    pub hide_below: String,
    pub title: String,
    pub footnote: String,
}

impl Default for OlapCubes {
    fn default() -> Self {
        Self {
            summary: Vec::new(),
            layers: vec![Vec::new()],
            layer: 0,
            cells: olap_default(),
            difference: OlapDifference::None,
            group: String::new(),
            pairs: vec![Default::default()],
            absolute: true,
            percent: false,
            hide_small: false,
            hide_below: "5".into(),
            title: String::new(),
            footnote: String::new(),
        }
    }
}

impl OlapCubes {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("OLAP CUBES {}", names(&self.summary, "Summary variables")?);
        let layers: Vec<&Vec<String>> = self.layers.iter().filter(|l| !l.is_empty()).collect();
        if layers.is_empty() {
            return Err("add at least one variable to Group by".into());
        }
        for layer in &layers {
            text.push_str(&format!(" BY {}", layer.join(" ")));
        }
        if self.cells != olap_default() {
            let words = keywords(&MEANS_CELLS, &self.cells);
            if words.is_empty() {
                text.push_str("\n  /CELLS=NONE");
            } else {
                text.push_str(&format!("\n  /CELLS={}", words.join(" ")));
            }
        }
        if self.difference != OlapDifference::None {
            let groups = self.difference == OlapDifference::Groups;
            let types = match (self.absolute, self.percent, groups) {
                (false, false, _) => {
                    return Err("choose the change, the percent change or both".into());
                }
                (true, false, true) => "GAC",
                (false, true, true) => "GPC",
                (true, true, true) => "GAC GPC",
                (true, false, false) => "VAC",
                (false, true, false) => "VPC",
                (true, true, false) => "VAC VPC",
            };
            let mut pairs = Vec::new();
            for [a, b] in &self.pairs {
                let (a, b) = (a.trim(), b.trim());
                if a.is_empty() && b.is_empty() {
                    continue;
                }
                if a.is_empty() || b.is_empty() {
                    return Err("fill in both sides of each pair, or clear the pair".into());
                }
                if groups {
                    pairs.push(format!("({} {})", loose_value(a), loose_value(b)));
                } else {
                    for name in [a, b] {
                        if !self.summary.iter().any(|s| s == name) {
                            return Err(format!("{name} is not one of the summary variables"));
                        }
                    }
                    pairs.push(format!("({a} {b})"));
                }
            }
            if pairs.is_empty() {
                return Err("name at least one pair to compare".into());
            }
            if groups {
                let group = self.group.trim();
                if !layers.iter().any(|l| l.iter().any(|v| v == group)) {
                    return Err("choose the grouping variable whose categories are compared".into());
                }
                text.push_str(&format!(
                    "\n  /CREATE {types} ({group} {})",
                    pairs.join(" ")
                ));
            } else {
                text.push_str(&format!("\n  /CREATE {types} {}", pairs.join(" ")));
            }
        }
        for (what, value, keyword) in [
            ("the title", &self.title, "TITLE"),
            ("the footnote", &self.footnote, "FOOTNOTE"),
        ] {
            if value.len() > 255 {
                return Err(format!("{what} is longer than 255 bytes"));
            }
            if !value.is_empty() {
                text.push_str(&format!("\n  /{keyword}={}", quote(value)));
            }
        }
        if self.hide_small {
            let below = count(&self.hide_below, 2, "the count shown as small", 5)?;
            text.push_str(&format!("\n  /HIDESMALLCOUNTS COUNT={below}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// A typed category value: a number as it is, anything else as a string.
pub(crate) fn loose_value(text: &str) -> String {
    match text.parse::<f64>() {
        Ok(n) if n.is_finite() => text.to_string(),
        _ => quote(text),
    }
}

/// The parts of a variable's dictionary entry; all but the last show without VARINFO.
pub const CODEBOOK_VARINFO: [(&str, &str); 10] = [
    ("POSITION", "Position in the file"),
    ("LABEL", "Variable label"),
    ("TYPE", "Type"),
    ("FORMAT", "Display format"),
    ("MEASURE", "Measurement level"),
    ("ROLE", "Role"),
    ("ATTRIBUTES", "Custom attributes"),
    ("VALUELABELS", "Value labels"),
    ("MISSING", "Missing values"),
    ("RESERVEDATTRIBUTES", "Reserved attributes"),
];

const VARINFO_DEFAULT: [bool; 10] = [true, true, true, true, true, true, true, true, true, false];

/// The rows of the table about the file; none show without FILEINFO.
pub const CODEBOOK_FILEINFO: [(&str, &str); 8] = [
    ("NAME", "File name"),
    ("LOCATION", "Folder"),
    ("CASECOUNT", "Number of cases"),
    ("LABEL", "File label"),
    ("ATTRIBUTES", "Custom attributes"),
    ("DOCUMENTS", "Documents"),
    ("WEIGHT", "Weight variable"),
    ("RESERVEDATTRIBUTES", "Reserved attributes"),
];

/// The summary statistics; all show without STATISTICS.
pub const CODEBOOK_STATISTICS: [(&str, &str); 5] = [
    ("COUNT", "Counts"),
    ("PERCENT", "Percents"),
    ("MEAN", "Mean"),
    ("STDDEV", "Standard deviation"),
    ("QUARTILES", "Quartiles"),
];

/// The order of the tables; the first writes nothing.
pub const CODEBOOK_ORDERS: [(&str, &str); 5] = [
    ("", "As listed (file order without a list)"),
    ("FILE", "File order"),
    ("ALPHA", "By name"),
    ("MEASURE", "By measurement level"),
    ("ATTRIBUTE", "By a custom attribute"),
];

pub const CODEBOOK_ATTRIBUTE: usize = 4;

#[derive(Clone, PartialEq, Debug)]
pub struct Codebook {
    /// Variables and multiple response sets ($name); empty for every variable, then every set.
    pub variables: Vec<String>,
    /// One flag per entry of `CODEBOOK_VARINFO`.
    pub varinfo: [bool; 10],
    /// One flag per entry of `CODEBOOK_FILEINFO`.
    pub fileinfo: [bool; 8],
    /// One flag per entry of `CODEBOOK_STATISTICS`; none checked writes NONE.
    pub statistics: [bool; 5],
    pub maxcats: String,
    /// An index into `CODEBOOK_ORDERS`.
    pub order: usize,
    /// The custom attribute the tables are sorted by.
    pub attribute: String,
    pub descending: bool,
}

impl Default for Codebook {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            varinfo: VARINFO_DEFAULT,
            fileinfo: [false; 8],
            statistics: [true; 5],
            maxcats: "200".into(),
            order: 0,
            attribute: String::new(),
            descending: false,
        }
    }
}

impl Codebook {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = String::from("CODEBOOK");
        for name in &self.variables {
            text.push(' ');
            text.push_str(name);
        }
        if self.varinfo != VARINFO_DEFAULT {
            let words = keywords(&CODEBOOK_VARINFO, &self.varinfo);
            if words.is_empty() {
                return Err("choose at least one part of the variable entries".into());
            }
            text.push_str(&format!("\n  /VARINFO={}", words.join(" ")));
        }
        let words = keywords(&CODEBOOK_FILEINFO, &self.fileinfo);
        if !words.is_empty() {
            text.push_str(&format!("\n  /FILEINFO={}", words.join(" ")));
        }
        let words = keywords(&CODEBOOK_STATISTICS, &self.statistics);
        match words.len() {
            5 => {}
            0 => text.push_str("\n  /STATISTICS=NONE"),
            _ => text.push_str(&format!("\n  /STATISTICS={}", words.join(" "))),
        }
        let mut options = Vec::new();
        let maxcats = count(&self.maxcats, 0, "the most values", 200)?;
        if maxcats != 200 {
            options.push(format!("MAXCATS={maxcats}"));
        }
        match self.order {
            0 => {}
            CODEBOOK_ATTRIBUTE => {
                let name = self.attribute.trim();
                if name.is_empty() {
                    return Err("name the custom attribute to sort by".into());
                }
                if !is_valid_name(name) || name.starts_with('#') {
                    return Err(format!("{name} is not a valid attribute name"));
                }
                options.push(format!("VARORDER=ATTRIBUTE({name})"));
            }
            i => options.push(format!("VARORDER={}", CODEBOOK_ORDERS[i].0)),
        }
        if self.descending {
            options.push("SORT=DESCENDING".into());
        }
        if !options.is_empty() {
            text.push_str(&format!("\n  /OPTIONS {}", options.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{runs, v};

    fn summarize() -> Summarize {
        let mut d = Summarize::default();
        d.means.dependents = v(&["monthly_spend"]);
        d
    }

    #[test]
    fn case_summaries_write_only_what_changed() {
        let mut d = summarize();
        assert_eq!(d.syntax().unwrap(), "SUMMARIZE TABLES=monthly_spend.");
        // the listing settings count only while the cases are listed
        d.limit = "x".into();
        d.case_numbers = false;
        d.missing_text = "none".into();
        d.valid_only = true;
        assert_eq!(d.syntax().unwrap(), "SUMMARIZE TABLES=monthly_spend.");
        d.list = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "the number of cases used must be a whole number of at least 1"
        );
        d.limit = "40".into();
        d.total_label = false;
        d.tablewise = true;
        d.title = "Spend, it's monthly".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "SUMMARIZE TABLES=monthly_spend\n  \
             /FORMAT=VALIDLIST LIMIT=40 NOCASENUM MISSING='none' NOTOTAL\n  \
             /MISSING=TABLE\n  /TITLE='Spend, it''s monthly'."
        );
        assert!(runs(&syntax) >= 2);
    }

    #[test]
    fn case_summaries_check_what_the_command_would_reject() {
        let e = |change: &dyn Fn(&mut Summarize)| {
            let mut d = summarize();
            change(&mut d);
            d.syntax().unwrap_err()
        };
        assert_eq!(
            e(&|d| d.means.dependents.clear()),
            "add at least one variable to Dependent variables"
        );
        assert_eq!(
            e(&|d| d.means.layers = vec![v(&["region", "gender"]), v(&["region"])]),
            "region is in two layers; keep it in one"
        );
        assert_eq!(
            e(&|d| d.footnote = "x".repeat(256)),
            "the footnote is longer than 255 bytes"
        );
        assert_eq!(
            e(&|d| d.means.anova = true),
            "the analysis of variance needs a variable in Layer 1"
        );
    }

    #[test]
    fn olap_cubes_write_only_what_changed() {
        let mut d = OlapCubes {
            summary: v(&["monthly_spend", "age"]),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Group by"
        );
        d.layers = vec![v(&["region"]), v(&["gender"])];
        assert_eq!(
            d.syntax().unwrap(),
            "OLAP CUBES monthly_spend age BY region BY gender."
        );
        d.cells = [false; 21];
        d.cells[0] = true;
        d.cells[1] = true;
        d.difference = OlapDifference::Variables;
        d.pairs = vec![["monthly_spend".into(), "age".into()]];
        d.percent = true;
        d.title = "Spend, it's grouped".into();
        d.hide_small = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "OLAP CUBES monthly_spend age BY region BY gender\n  /CELLS=MEAN COUNT\n  \
             /CREATE VAC VPC (monthly_spend age)\n  /TITLE='Spend, it''s grouped'\n  \
             /HIDESMALLCOUNTS COUNT=5."
        );
        assert!(runs(&syntax) >= 2);
        d.difference = OlapDifference::Groups;
        d.pairs = vec![["1".into(), "2".into()]];
        d.group = "gender".into();
        d.absolute = false;
        d.cells = [false; 21];
        assert_eq!(
            d.syntax().unwrap(),
            "OLAP CUBES monthly_spend age BY region BY gender\n  /CELLS=NONE\n  \
             /CREATE GPC (gender (1 2))\n  /TITLE='Spend, it''s grouped'\n  \
             /HIDESMALLCOUNTS COUNT=5."
        );
        d.group = "plan".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose the grouping variable whose categories are compared"
        );
    }

    #[test]
    fn codebook_writes_only_what_changed() {
        let mut d = Codebook::default();
        assert_eq!(d.syntax().unwrap(), "CODEBOOK.");
        d.variables = v(&["region", "age"]);
        d.varinfo[0] = false;
        d.fileinfo[2] = true;
        d.statistics[1] = false;
        d.maxcats = "0".into();
        d.order = 3;
        d.descending = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CODEBOOK region age\n  \
             /VARINFO=LABEL TYPE FORMAT MEASURE ROLE ATTRIBUTES VALUELABELS MISSING\n  \
             /FILEINFO=CASECOUNT\n  /STATISTICS=COUNT MEAN STDDEV QUARTILES\n  \
             /OPTIONS MAXCATS=0 VARORDER=MEASURE SORT=DESCENDING."
        );
        assert!(runs(&syntax) >= 3);
        // the attribute is checked only when the tables are sorted by it
        d.attribute = "1st".into();
        assert!(d.syntax().is_ok());
        d.order = CODEBOOK_ATTRIBUTE;
        assert_eq!(d.syntax().unwrap_err(), "1st is not a valid attribute name");
        d.attribute = String::new();
        assert_eq!(
            d.syntax().unwrap_err(),
            "name the custom attribute to sort by"
        );
        d.attribute = "wave".into();
        d.statistics = [false; 5];
        d.descending = false;
        d.maxcats = "200".into();
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("/STATISTICS=NONE\n  /OPTIONS VARORDER=ATTRIBUTE(wave)."));
        assert!(runs(&syntax) >= 2);
        d.varinfo = [false; 10];
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose at least one part of the variable entries"
        );
        d.varinfo = VARINFO_DEFAULT;
        d.maxcats = "-1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the most values must be a whole number of at least 0"
        );
    }
}
