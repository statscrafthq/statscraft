//! The Correspondence Analysis dialog: CORRESPONDENCE of a row variable by a column variable,
//! each with its range of category values. Options left as they are add nothing to the syntax.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::parse;
use crate::syntax_text::number;
use statscraft_data::Dataset;
use std::collections::BTreeMap;

/// What a category of the row or column variable is in the analysis; active when not listed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Supplementary,
    /// In equal group n (from 1): the categories of a group get equal scores.
    Equal(usize),
}

/// The row or the column of the table.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TableVariable {
    pub name: String,
    pub min: String,
    pub max: String,
    /// The categories that are not active, by value.
    pub roles: BTreeMap<i64, Role>,
}

/// A side of the table, checked.
struct Checked {
    range: (i64, i64),
    supplementary: Vec<i64>,
    groups: Vec<Vec<i64>>,
}

impl TableVariable {
    /// The smallest and the largest category value, checked.
    pub fn range(&self) -> Result<(i64, i64), String> {
        let name = self.name.trim();
        let bound = |text: &str, what: &str| -> Result<i64, String> {
            match text.trim() {
                "" => Err(format!("give the {what} category value of {name}")),
                t => t
                    .parse::<i64>()
                    .ok()
                    .filter(|n| n.abs() < 1_000_000_000)
                    .ok_or_else(|| {
                        format!("the {what} category value of {name} must be a whole number")
                    }),
            }
        };
        let (min, max) = (bound(&self.min, "smallest")?, bound(&self.max, "largest")?);
        if min > max {
            return Err(format!(
                "the smallest category value of {name} is above the largest"
            ));
        }
        if max - min >= 10_000 {
            return Err(format!(
                "the range of {name} holds more than 10000 categories"
            ));
        }
        Ok((min, max))
    }

    /// Forgets the roles of categories outside a valid range, after the range changed.
    pub fn keep_roles_in_range(&mut self) {
        if let Ok((min, max)) = self.range() {
            self.roles.retain(|value, _| (min..=max).contains(value));
        }
    }

    fn checked(&self) -> Result<Checked, String> {
        let name = self.name.trim();
        let range = self.range()?;
        let mut supplementary = Vec::new();
        let mut groups: BTreeMap<usize, Vec<i64>> = BTreeMap::new();
        for (&value, &role) in &self.roles {
            // a role left outside a changed range is not shown, so it does not count
            if value < range.0 || value > range.1 {
                continue;
            }
            match role {
                Role::Supplementary => supplementary.push(value),
                Role::Equal(g) => groups.entry(g).or_default().push(value),
            }
        }
        // the manual's limits for a variable
        if supplementary.len() > 200 {
            return Err(format!("{name} has more than 200 supplementary categories"));
        }
        let merged_points: usize = groups.values().map(Vec::len).sum();
        if merged_points > 200 {
            return Err(format!(
                "{name} has more than 200 categories in equal groups"
            ));
        }
        if let Some((g, _)) = groups.iter().find(|(_, members)| members.len() < 2) {
            return Err(format!(
                "equal group {g} of {name} needs at least two categories"
            ));
        }
        let merged: usize = groups.values().map(|g| g.len() - 1).sum();
        let active = (range.1 - range.0 + 1) as usize - supplementary.len() - merged;
        if active < 2 {
            return Err(format!(
                "{name} needs at least two active categories (an equal group counts as one)"
            ));
        }
        Ok(Checked {
            range,
            supplementary,
            groups: groups.into_values().collect(),
        })
    }
}

/// The distance measures with their standardization; the first is the default.
pub const CA_DISTANCES: [(&str, &str); 6] = [
    ("", "Chi-square"),
    ("RCMEAN", "Euclidean, row and column means removed"),
    ("RMEAN", "Euclidean, row means removed"),
    ("CMEAN", "Euclidean, column means removed"),
    ("RSUM", "Euclidean, equal row totals, row means removed"),
    (
        "CSUM",
        "Euclidean, equal column totals, column means removed",
    ),
];

/// The normalizations; the last takes a value from -1 to 1.
pub const CA_NORMALIZATIONS: [(&str, &str); 5] = [
    ("SYMMETRICAL", "Symmetrical"),
    ("PRINCIPAL", "Principal"),
    ("RPRINCIPAL", "Row principal"),
    ("CPRINCIPAL", "Column principal"),
    ("", "Custom value"),
];

/// The tables on /PRINT, besides the permuted tables.
pub const CA_PRINT: [(&str, &str); 7] = [
    ("TABLE", "Correspondence table"),
    ("RPROFILES", "Row profiles"),
    ("CPROFILES", "Column profiles"),
    ("RPOINTS", "Overview of row points"),
    ("CPOINTS", "Overview of column points"),
    ("RCONF", "Confidence of row points"),
    ("CCONF", "Confidence of column points"),
];

const CA_DEFAULT_PRINT: [bool; 7] = [true, false, false, true, true, false, false];

/// The plots on /PLOT.
pub const CA_PLOTS: [(&str, &str); 5] = [
    ("BIPLOT", "Biplot"),
    ("RPOINTS", "Row points"),
    ("CPOINTS", "Column points"),
    ("TRROWS", "Transformed row categories"),
    ("TRCOLUMNS", "Transformed column categories"),
];

const CA_DEFAULT_PLOTS: [bool; 5] = [true, false, false, false, false];

/// CORRESPONDENCE: simple correspondence analysis of a two-way table.
#[derive(Clone, PartialEq, Debug)]
pub struct Correspondence {
    pub row: TableVariable,
    pub column: TableVariable,
    pub dimensions: String,
    /// Index into `CA_DISTANCES`.
    pub distance: usize,
    /// Index into `CA_NORMALIZATIONS`.
    pub normalization: usize,
    /// The custom normalization value.
    pub q: String,
    /// One flag per entry of `CA_PRINT`.
    pub print: [bool; 7],
    pub permutation: bool,
    /// The dimensions of the permuted tables.
    pub permutation_dims: String,
    /// One flag per entry of `CA_PLOTS`.
    pub plots: [bool; 5],
    /// The longest label in the plots; empty for labels in full.
    pub label_length: String,
    /// Plot a range of dimensions instead of every pair.
    pub ndim: bool,
    pub ndim_first: String,
    /// Empty: up to the last dimension.
    pub ndim_last: String,
}

impl Default for Correspondence {
    fn default() -> Self {
        Self {
            row: TableVariable::default(),
            column: TableVariable::default(),
            dimensions: "2".into(),
            distance: 0,
            normalization: 0,
            q: "0".into(),
            print: CA_DEFAULT_PRINT,
            permutation: false,
            permutation_dims: "1".into(),
            plots: CA_DEFAULT_PLOTS,
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: String::new(),
        }
    }
}

impl Correspondence {
    /// The biplot is not drawn with principal normalization, so the dialog offers it only
    /// with the others.
    pub fn biplot_allowed(&self) -> bool {
        CA_NORMALIZATIONS[self.normalization].0 != "PRINCIPAL"
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        for (v, what) in [(&self.row, "row"), (&self.column, "column")] {
            let name = v.name.trim();
            if name.is_empty() {
                return Err(format!("choose the {what} variable"));
            }
            match data.variable(name) {
                Err(_) => return Err(format!("{name} is no longer a variable")),
                Ok(var) if var.is_string() => {
                    return Err(format!("{name} is a string; the command needs numbers"));
                }
                Ok(_) => {}
            }
        }
        let (row, column) = (self.row.name.trim(), self.column.name.trim());
        if row.eq_ignore_ascii_case(column) {
            return Err(format!(
                "{row} cannot be both the row and the column variable"
            ));
        }
        let (r, c) = (self.row.checked()?, self.column.checked()?);
        let mut text = format!(
            "CORRESPONDENCE TABLE={row}({},{}) BY {column}({},{})",
            r.range.0, r.range.1, c.range.0, c.range.1
        );
        let list = |values: &[i64]| {
            let values: Vec<String> = values.iter().map(i64::to_string).collect();
            format!("({})", values.join(","))
        };
        let supplementary: Vec<String> = [(row, &r), (column, &c)]
            .iter()
            .filter(|(_, s)| !s.supplementary.is_empty())
            .map(|(name, s)| format!("{name}{}", list(&s.supplementary)))
            .collect();
        if !supplementary.is_empty() {
            text.push_str(&format!("\n  /SUPPLEMENTARY={}", supplementary.join(" ")));
        }
        let equal: Vec<String> = [(row, &r), (column, &c)]
            .iter()
            .filter(|(_, s)| !s.groups.is_empty())
            .map(|(name, s)| {
                let groups: Vec<String> = s.groups.iter().map(|g| list(g)).collect();
                format!("{name}{}", groups.concat())
            })
            .collect();
        if !equal.is_empty() {
            text.push_str(&format!("\n  /EQUAL={}", equal.join(" ")));
        }
        match CA_DISTANCES[self.distance].0 {
            "" => {}
            "RCMEAN" => text.push_str("\n  /MEASURE=EUCLID"),
            s => text.push_str(&format!("\n  /MEASURE=EUCLID\n  /STANDARDIZE={s}")),
        }
        // the engine lowers more dimensions than the table allows, empty categories left out
        let dims = whole(&self.dimensions, 1, "the number of dimensions")?.unwrap_or(2) as usize;
        // the syntax takes no larger number
        if dims >= 1_000_000_000 {
            return Err("the number of dimensions must be below 1000000000".into());
        }
        if dims != 2 {
            text.push_str(&format!("\n  /DIMENSION={dims}"));
        }
        match CA_NORMALIZATIONS[self.normalization].0 {
            "SYMMETRICAL" => {}
            "" => {
                let q = parse(&self.q, "the normalization value")?;
                if !(-1.0..=1.0).contains(&q) {
                    return Err("the normalization value lies from -1 to 1".into());
                }
                if q != 0.0 {
                    text.push_str(&format!("\n  /NORMALIZATION={}", number(q)));
                }
            }
            k => text.push_str(&format!("\n  /NORMALIZATION={k}")),
        }
        if self.print != CA_DEFAULT_PRINT || self.permutation {
            let mut print: Vec<String> = keywords(&CA_PRINT, &self.print)
                .into_iter()
                .map(String::from)
                .collect();
            if self.permutation {
                match whole(
                    &self.permutation_dims,
                    1,
                    "the dimensions of the permuted tables",
                )? {
                    Some(n) if n >= 1_000_000_000 => {
                        return Err(
                            "the dimensions of the permuted tables must be below 1000000000".into(),
                        );
                    }
                    // the command keeps to the dimensions it computes
                    Some(n) if n > 1 => print.push(format!("PERMUTATION({n})")),
                    _ => print.push("PERMUTATION".into()),
                }
            }
            if print.is_empty() {
                print.push("NONE".into());
            }
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        text.push_str(&self.plot(dims)?);
        text.push('.');
        Ok(text)
    }

    /// The /PLOT subcommand, empty for the default biplot.
    fn plot(&self, dims: usize) -> Result<String, String> {
        let mut plots = self.plots;
        plots[0] &= self.biplot_allowed();
        let chosen = keywords(&CA_PLOTS, &plots);
        if chosen.is_empty() {
            // principal normalization draws nothing by default
            return Ok(match self.biplot_allowed() {
                true => "\n  /PLOT=NONE".into(),
                false => String::new(),
            });
        }
        let length = match self.label_length.trim() {
            "" => None,
            t => match t.parse::<u32>() {
                // the command treats a longer length as 60
                Ok(n) => Some(n.min(60)),
                _ => return Err("the label length is a whole number of 0 or more".into()),
            },
        };
        let mut words: Vec<String> = chosen
            .into_iter()
            .map(|k| match length {
                Some(n) => format!("{k}({n})"),
                None => k.to_string(),
            })
            .collect();
        if self.ndim {
            if dims < 2 {
                return Err("plotting a range of dimensions needs at least two".into());
            }
            let first = whole(&self.ndim_first, 1, "the first plotted dimension")?.unwrap_or(1);
            if first as usize >= dims {
                return Err(format!(
                    "the first plotted dimension must be below the number of dimensions ({dims})"
                ));
            }
            let last = match whole(&self.ndim_last, 1, "the last plotted dimension")? {
                None => "MAX".to_string(),
                Some(l) if l > first && l as usize <= dims => l.to_string(),
                Some(_) => {
                    return Err(format!(
                        "the last plotted dimension must be above the first and at most {dims}"
                    ));
                }
            };
            words.push(format!("NDIM({first},{last})"));
        } else if plots == CA_DEFAULT_PLOTS && length.is_none() {
            return Ok(String::new());
        }
        Ok(format!("\n  /PLOT={}", words.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in};

    fn side(name: &str, min: &str, max: &str) -> TableVariable {
        TableVariable {
            name: name.into(),
            min: min.into(),
            max: max.into(),
            roles: BTreeMap::new(),
        }
    }

    #[test]
    fn correspondence_analysis() {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let mut d = Correspondence::default();
        assert_eq!(d.syntax(&data).unwrap_err(), "choose the row variable");
        d.row = side("region", "1", "5");
        d.column = side("q6_mention1", "1", "6");
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "CORRESPONDENCE TABLE=region(1,5) BY q6_mention1(1,6)."
        );
        assert!(statscraft_syntax::parse(&syntax).1.is_empty());
        // the table, the summary, both overviews and the biplot
        assert!(run_in(&mut session, &syntax) >= 4);

        // every option away from its default
        d.row.roles.insert(5, Role::Supplementary);
        d.column.roles.insert(1, Role::Equal(1));
        d.column.roles.insert(2, Role::Equal(1));
        d.column.roles.insert(6, Role::Supplementary);
        d.dimensions = "3".into();
        d.distance = 4;
        d.normalization = 4;
        d.q = "0.5".into();
        d.print = [true; 7];
        d.permutation = true;
        d.permutation_dims = "2".into();
        d.plots = [true; 5];
        d.label_length = "10".into();
        d.ndim = true;
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "CORRESPONDENCE TABLE=region(1,5) BY q6_mention1(1,6)\n  \
             /SUPPLEMENTARY=region(5) q6_mention1(6)\n  /EQUAL=q6_mention1(1,2)\n  \
             /MEASURE=EUCLID\n  /STANDARDIZE=RSUM\n  /DIMENSION=3\n  /NORMALIZATION=0.5\n  \
             /PRINT=TABLE RPROFILES CPROFILES RPOINTS CPOINTS RCONF CCONF PERMUTATION(2)\n  \
             /PLOT=BIPLOT(10) RPOINTS(10) CPOINTS(10) TRROWS(10) TRCOLUMNS(10) NDIM(1,MAX)."
        );
        assert!(statscraft_syntax::parse(&syntax).1.is_empty());
        assert!(run_in(&mut session, &syntax) >= 8);

        // principal normalization leaves the biplot out; no tables and no plots
        d = Correspondence {
            row: side("region", "1", "5"),
            column: side("q6_mention1", "1", "6"),
            distance: 1,
            normalization: 1,
            print: [false; 7],
            plots: [true, true, false, false, false],
            ..Default::default()
        };
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(
            syntax,
            "CORRESPONDENCE TABLE=region(1,5) BY q6_mention1(1,6)\n  /MEASURE=EUCLID\n  \
             /NORMALIZATION=PRINCIPAL\n  /PRINT=NONE\n  /PLOT=RPOINTS."
        );
        assert!(run_in(&mut session, &syntax) >= 1);
        d.plots = CA_DEFAULT_PLOTS;
        assert!(!d.syntax(&data).unwrap().contains("/PLOT"));
        d.normalization = 2;
        assert!(d.syntax(&data).unwrap().ends_with("/PRINT=NONE."));
        d.plots = [false; 5];
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
        // equal groups in the order of their numbers, a custom value of 0 is the default
        d = Correspondence {
            row: side("region", "1", "5"),
            column: side("q6_mention1", "1", "6"),
            normalization: 4,
            ..Default::default()
        };
        d.row.roles.insert(1, Role::Equal(2));
        d.row.roles.insert(4, Role::Equal(2));
        d.row.roles.insert(2, Role::Equal(1));
        d.row.roles.insert(3, Role::Equal(1));
        d.dimensions = "1".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CORRESPONDENCE TABLE=region(1,5) BY q6_mention1(1,6)\n  \
             /EQUAL=region(2,3)(1,4)\n  /DIMENSION=1."
        );
        // more dimensions than the table allows are left to the engine, which lowers them
        d.dimensions = "3".into();
        let syntax = d.syntax(&data).unwrap();
        assert!(syntax.ends_with("/DIMENSION=3."));
        assert!(run_in(&mut session, &syntax) >= 4);
        // a side of two categories with the default two dimensions: one is computed
        d = Correspondence {
            row: side("gender", "1", "2"),
            column: side("region", "1", "5"),
            ..Default::default()
        };
        let syntax = d.syntax(&data).unwrap();
        assert_eq!(syntax, "CORRESPONDENCE TABLE=gender(1,2) BY region(1,5).");
        assert!(run_in(&mut session, &syntax) >= 4);
    }

    #[test]
    fn a_changed_range_forgets_the_roles_outside_it() {
        let mut v = side("region", "1", "5");
        v.roles.insert(1, Role::Supplementary);
        v.roles.insert(4, Role::Equal(1));
        v.roles.insert(5, Role::Equal(1));
        // a range still being typed keeps every role
        v.max.clear();
        v.keep_roles_in_range();
        assert_eq!(v.roles.len(), 3);
        v.min = "2".into();
        v.max = "4".into();
        v.keep_roles_in_range();
        assert_eq!(
            v.roles.into_iter().collect::<Vec<_>>(),
            [(4, Role::Equal(1))]
        );
    }

    #[test]
    fn correspondence_checks_what_would_fail_when_run() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let base = Correspondence {
            row: side("region", "1", "5"),
            column: side("q6_mention1", "1", "6"),
            ..Default::default()
        };
        let err = |change: &dyn Fn(&mut Correspondence)| {
            let mut d = base.clone();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.column.name.clear()),
            "choose the column variable"
        );
        assert_eq!(
            err(&|d| d.column.name = "gone".into()),
            "gone is no longer a variable"
        );
        assert_eq!(
            err(&|d| d.column.name = "plan".into()),
            "plan is a string; the command needs numbers"
        );
        assert_eq!(
            err(&|d| d.column.name = "Region".into()),
            "region cannot be both the row and the column variable"
        );
        assert_eq!(
            err(&|d| d.row.max.clear()),
            "give the largest category value of region"
        );
        assert_eq!(
            err(&|d| d.row.min = "1.5".into()),
            "the smallest category value of region must be a whole number"
        );
        assert_eq!(
            err(&|d| d.row.min = "6".into()),
            "the smallest category value of region is above the largest"
        );
        assert_eq!(
            err(&|d| d.row.max = "10001".into()),
            "the range of region holds more than 10000 categories"
        );
        let mut d = base.clone();
        d.row.max = "300".into();
        for value in 1..=201 {
            d.row.roles.insert(value, Role::Supplementary);
        }
        assert_eq!(
            d.syntax(&data).unwrap_err(),
            "region has more than 200 supplementary categories"
        );
        // roles outside the range are not shown, so they leave the syntax as it is
        let mut d = base.clone();
        d.row.roles.insert(7, Role::Supplementary);
        d.column.roles.insert(0, Role::Equal(3));
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CORRESPONDENCE TABLE=region(1,5) BY q6_mention1(1,6)."
        );
        assert_eq!(
            err(&|d| {
                d.row.roles.insert(2, Role::Equal(1));
            }),
            "equal group 1 of region needs at least two categories"
        );
        assert_eq!(
            err(&|d| {
                d.row.max = "2".into();
                d.row.roles.insert(1, Role::Supplementary);
            }),
            "region needs at least two active categories (an equal group counts as one)"
        );
        assert_eq!(
            err(&|d| d.dimensions = "0".into()),
            "the number of dimensions must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| {
                d.normalization = 4;
                d.q = "1.5".into();
            }),
            "the normalization value lies from -1 to 1"
        );
        // more permuted dimensions than the solution, and a label over 60, are the command's to cap
        let mut d = base.clone();
        d.permutation = true;
        d.permutation_dims = "3".into();
        d.label_length = "61".into();
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.contains("PERMUTATION(3)") && syntax.contains("(60)"),
            "{syntax}"
        );
        // without a plot the label length is not written, so not checked
        let mut d = base.clone();
        d.label_length = "61".into();
        d.plots = [false; 5];
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
        assert_eq!(
            err(&|d| d.dimensions = "1000000000".into()),
            "the number of dimensions must be below 1000000000"
        );
        assert_eq!(
            err(&|d| {
                d.permutation = true;
                d.permutation_dims = "1000000000".into();
            }),
            "the dimensions of the permuted tables must be below 1000000000"
        );
        assert_eq!(
            err(&|d| {
                d.ndim = true;
                d.ndim_last = "1000000000".into();
            }),
            "the last plotted dimension must be above the first and at most 2"
        );
        assert_eq!(
            err(&|d| {
                d.ndim = true;
                d.ndim_first = "2".into();
            }),
            "the first plotted dimension must be below the number of dimensions (2)"
        );
        assert_eq!(
            err(&|d| {
                d.ndim = true;
                d.ndim_last = "3".into();
            }),
            "the last plotted dimension must be above the first and at most 2"
        );
        assert_eq!(
            err(&|d| {
                d.ndim = true;
                d.dimensions = "1".into();
            }),
            "plotting a range of dimensions needs at least two"
        );
    }
}
