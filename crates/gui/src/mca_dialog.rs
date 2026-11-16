//! The Multiple Correspondence dialog: MULTIPLE CORRESPONDENCE of nominal variables, each with
//! its weight, recoding, missing value handling and role. Options left as they are add nothing to
//! the syntax, so the command runs with its own defaults.

use crate::scaling_dialog::{
    INDEPENDENT, PrintLists, ScalingVariable, all_variables, analysis_list, analysis_settings,
    data_subcommands, label_length, model_subcommands, ndim, outfile_syntax, paren, per_variable,
    print_syntax, save_syntax, still,
};
use statscraft_data::Dataset;

#[cfg(test)]
use crate::scaling_dialog::{LISTWISE, case_numbers};

/// The tables of /PRINT; DESCRIP, OBJECT and QUANT take variable lists.
pub const MCA_PRINT: [(&str, &str); 7] = [
    ("CORR", "Correlations of quantified variables"),
    ("DESCRIP", "Descriptives"),
    ("DISCRIM", "Discrimination measures"),
    ("HISTORY", "Iteration history"),
    ("OBJECT", "Object scores"),
    ("OCORR", "Correlations of original variables"),
    ("QUANT", "Category quantifications"),
];

const MCA_DEFAULT_PRINT: [bool; 7] = [true, true, true, false, false, false, false];

/// The charts of /PLOT; every one but the biplot takes one list of variables.
pub const MCA_PLOTS: [(&str, &str); 7] = [
    ("OBJECT", "Object points"),
    ("CATEGORY", "Category points"),
    ("JOINTCAT", "Joint category points"),
    ("DISCRIM", "Discrimination measures"),
    ("BIPLOT", "Objects and centroids"),
    ("TRANS", "Transformations"),
    ("RESID", "Residuals"),
];

const MCA_DEFAULT_PLOTS: [bool; 7] = [true, false, false, true, false, false, false];
pub const BIPLOT: usize = 4;

/// The variables /SAVE adds, with the longest root the command keeps.
pub const MCA_SAVE: [(&str, &str, usize); 2] = [
    ("TRDATA", "Quantified variables", 3),
    ("OBJECT", "Object scores", 5),
];

/// The data /OUTFILE writes.
pub const MCA_OUTFILE: [(&str, &str); 3] = [
    ("DISCRDATA", "Recoded data"),
    ("TRDATA", "Quantified variables"),
    ("OBJECT", "Object scores"),
];

/// The settings of one analysis variable.
pub type McaVariable = ScalingVariable;

/// MULTIPLE CORRESPONDENCE: homogeneity analysis of nominal variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Mca {
    pub variables: Vec<String>,
    /// The settings of the analysis variables, found by name; a variable without one has the
    /// defaults.
    pub settings: Vec<McaVariable>,
    /// Variables outside the analysis that label objects or are shown beside them.
    pub labels: Vec<String>,
    /// The supplementary cases by number, with ranges such as 4-9.
    pub objects: String,
    pub dimensions: String,
    /// Index into `SCALING_NORMALIZATIONS`.
    pub normalization: usize,
    /// The custom normalization value.
    pub q: String,
    pub max_iterations: String,
    pub convergence: String,
    /// One flag per entry of `MCA_PRINT`.
    pub print: [bool; 7],
    /// The variables of the descriptives, quantifications and object score tables; none means
    /// the command's own choice.
    pub descrip: Vec<String>,
    pub quant: Vec<String>,
    pub object_shown: Vec<String>,
    /// Labels the object scores; empty for case numbers.
    pub object_label: String,
    /// One flag per entry of `MCA_PLOTS`.
    pub plots: [bool; 7],
    /// The variables of each plot (for the biplot, those with centroids); none means all, or
    /// for object points the case numbers.
    pub plot_variables: [Vec<String>; 7],
    /// Labels the objects of the biplot; none for case numbers.
    pub biplot_labels: Vec<String>,
    /// The dimensions plotted for each variable in transformation and residual plots.
    pub plot_dimensions: String,
    /// The longest label in the plots; empty for labels in full.
    pub label_length: String,
    pub ndim: bool,
    pub ndim_first: String,
    pub ndim_last: String,
    /// One flag per entry of `MCA_SAVE`, with its root; an empty root is the command's own.
    pub save: [bool; 2],
    pub save_roots: [String; 2],
    /// Per entry of `MCA_OUTFILE`: nowhere (0), a data file (1) or a new dataset (2).
    pub outfile: [usize; 3],
    pub outfile_files: [String; 3],
    pub outfile_datasets: [String; 3],
}

impl Default for Mca {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            settings: Vec::new(),
            labels: Vec::new(),
            objects: String::new(),
            dimensions: "2".into(),
            normalization: 0,
            q: "0".into(),
            max_iterations: "100".into(),
            convergence: "0.00001".into(),
            print: MCA_DEFAULT_PRINT,
            descrip: Vec::new(),
            quant: Vec::new(),
            object_shown: Vec::new(),
            object_label: String::new(),
            plots: MCA_DEFAULT_PLOTS,
            plot_variables: Default::default(),
            biplot_labels: Vec::new(),
            plot_dimensions: "1".into(),
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: "2".into(),
            save: [false; 2],
            save_roots: Default::default(),
            outfile: [0; 3],
            outfile_files: Default::default(),
            outfile_datasets: Default::default(),
        }
    }
}

impl Mca {
    /// Every variable on VARIABLES: the analysis ones, then the labelling ones not among them.
    pub fn all_variables(&self) -> Vec<String> {
        all_variables(&self.variables, &self.labels)
    }

    /// The biplot is not drawn with independent normalization, so the dialog offers it only
    /// with the others.
    pub fn biplot_allowed(&self) -> bool {
        self.normalization != INDEPENDENT
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let settings = analysis_settings(&self.variables, &self.settings)?;
        let all = self.all_variables();
        let mut entries = Vec::new();
        for s in &settings {
            entries.push((s.name.as_str(), s.weight_spec()?.unwrap_or_default()));
        }
        let mut text = format!(
            "MULTIPLE CORRESPONDENCE VARIABLES={}\n  /ANALYSIS={}",
            all.join(" "),
            analysis_list(&entries)
        );
        text.push_str(&data_subcommands(&settings, &self.objects, data)?);
        let (model, dims) = model_subcommands(
            &self.dimensions,
            self.normalization,
            &self.q,
            &self.max_iterations,
            &self.convergence,
        )?;
        text.push_str(&model);
        text.push_str(&print_syntax(
            &MCA_PRINT,
            &self.print,
            &MCA_DEFAULT_PRINT,
            (&all, &self.variables),
            PrintLists {
                descrip: &self.descrip,
                quant: &self.quant,
                shown: &self.object_shown,
                label: &self.object_label,
            },
            None,
        ));
        text.push_str(&self.plot_syntax(&all, dims)?);
        text.push_str(&save_syntax(&MCA_SAVE, &self.save, &self.save_roots)?);
        let (declare, outfile) = outfile_syntax(
            &MCA_OUTFILE,
            &self.outfile,
            &self.outfile_files,
            &self.outfile_datasets,
        )?;
        text.push_str(&outfile);
        text.push('.');
        Ok(declare + &text)
    }

    /// /PLOT when the charts, their lists or their labels differ from the default.
    fn plot_syntax(&self, all: &[String], dims: u32) -> Result<String, String> {
        let mut plots = self.plots;
        plots[BIPLOT] &= self.biplot_allowed();
        // object points may be labeled by any variable, the other plots show analysis ones
        let chosen: Vec<Vec<String>> = (0..MCA_PLOTS.len())
            .map(|i| match i {
                0 => still(all, &self.plot_variables[i]),
                _ => still(&self.variables, &self.plot_variables[i]),
            })
            .collect();
        let biplot_labels = still(all, &self.biplot_labels);
        if !plots.contains(&true) {
            return Ok("\n  /PLOT=NONE".into());
        }
        // every plot but the transformations takes the label length
        let labeled = plots.iter().enumerate().any(|(i, on)| *on && i != 5);
        let length = label_length(&self.label_length, labeled)?;
        let per_variable = per_variable(&self.plot_dimensions, plots[5] || plots[6], dims)?;
        let mut words = Vec::new();
        for (i, (keyword, _)) in MCA_PLOTS.iter().enumerate() {
            if !plots[i] {
                continue;
            }
            // a plot that needs a list gets every analysis variable when none is checked
            let every = |l: &[String]| match l.is_empty() {
                true => self.variables.clone(),
                false => l.to_vec(),
            };
            words.push(match *keyword {
                "CATEGORY" => format!("CATEGORY{}{length}", paren(&every(&chosen[i]))),
                "BIPLOT" => {
                    let centroids = match chosen[i].is_empty() {
                        true => String::new(),
                        false => format!("({})", paren(&chosen[i])),
                    };
                    format!("BIPLOT{centroids}{}{length}", paren(&biplot_labels))
                }
                "TRANS" | "RESID" => {
                    let list: Vec<String> = every(&chosen[i])
                        .iter()
                        .map(|n| format!("{n}{per_variable}"))
                        .collect();
                    match *keyword {
                        // the transformation plots take no label length
                        "TRANS" => format!("TRANS{}", paren(&list)),
                        _ => format!("RESID{}{length}", paren(&list)),
                    }
                }
                k => format!("{k}{}{length}", paren(&chosen[i])),
            });
        }
        if self.ndim {
            words.push(ndim(&self.ndim_first, &self.ndim_last, dims)?);
        } else if plots == MCA_DEFAULT_PLOTS
            && length.is_empty()
            && plots.iter().zip(&chosen).all(|(on, l)| !on || l.is_empty())
        {
            return Ok(String::new());
        }
        Ok(format!("\n  /PLOT={}", words.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::v;
    use statscraft_engine::session::Session;

    /// Six invented cases of three nominal answers.
    fn data() -> Dataset {
        let mut session = Session::default();
        session
            .run(
                "DATA LIST LIST /tea snack seat note (3F1 A4).
BEGIN DATA
1 2 1 aa
2 1 3 bb
3 3 2 cc
1 1 1 dd
2 3 2 ee
3 2 3 ff
END DATA.",
                None,
            )
            .unwrap();
        session.dataset().unwrap().clone()
    }

    fn base() -> Mca {
        Mca {
            variables: v(&["tea", "snack", "seat"]),
            ..Default::default()
        }
    }

    #[test]
    fn weights_cover_the_variables_since_the_previous_one() {
        let data = data();
        let mut d = base();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MULTIPLE CORRESPONDENCE VARIABLES=tea snack seat\n  /ANALYSIS=tea snack seat."
        );
        let mut snack = McaVariable::new("snack");
        snack.weight = "2".into();
        d.settings.push(snack);
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea(WEIGHT=1) snack(WEIGHT=2) seat.")
        );
        let mut seat = McaVariable::new("seat");
        seat.weight = "2".into();
        d.settings.push(seat);
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea(WEIGHT=1) snack seat(WEIGHT=2).")
        );
        // a supplementary variable keeps weight 1
        d.settings[0].supplementary = true;
        d.settings[0].weight = "0".into();
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/ANALYSIS=tea snack(WEIGHT=1) seat(WEIGHT=2)")
        );
    }

    #[test]
    fn supplementary_cases_take_numbers_and_ranges() {
        assert_eq!(
            case_numbers("1, 3 4-5", 9).unwrap(),
            ("1 3 4 TO 5".into(), 4)
        );
        assert_eq!(case_numbers("2 to 3 3", 9).unwrap(), ("2 TO 3 3".into(), 2));
        assert!(case_numbers("0", 9).is_err());
        assert!(case_numbers("1.5", 9).is_err());
        assert!(case_numbers("4-", 9).is_err());
        assert_eq!(
            case_numbers("5-3", 9).unwrap_err(),
            "the supplementary range 5-3 runs backward"
        );
        assert_eq!(
            case_numbers("10", 9).unwrap_err(),
            "supplementary case 10 is past the last case (9)"
        );
    }

    #[test]
    fn mca_checks_what_the_command_would_reject() {
        let data = data();
        let err = |change: &dyn Fn(&mut Mca)| {
            let mut d = base();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        let tea_err = |change: &dyn Fn(&mut McaVariable)| {
            let mut d = base();
            let mut s = McaVariable::new("tea");
            change(&mut s);
            d.settings.push(s);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.variables.truncate(1)),
            "add at least two analysis variables"
        );
        assert_eq!(
            tea_err(&|s| s.weight = "-1".into()),
            "the weight of tea must be above 0"
        );
        assert_eq!(
            tea_err(&|s| {
                s.discretize = 1;
                s.groups = "0".into();
            }),
            "the number of groups of tea must be a whole number of at least 1"
        );
        assert_eq!(
            tea_err(&|s| s.discretize = 2),
            "the interval width of tea must be a number"
        );
        assert_eq!(
            tea_err(&|s| {
                s.discretize = 2;
                s.width = "0".into();
            }),
            "the interval width of tea must be above 0"
        );
        assert_eq!(
            tea_err(&|s| {
                s.supplementary = true;
                s.missing = LISTWISE;
            }),
            "supplementary variable tea cannot leave cases out for missing values"
        );
        assert_eq!(
            err(&|d| {
                for name in ["tea", "snack", "seat"] {
                    let mut s = McaVariable::new(name);
                    s.supplementary = true;
                    d.settings.push(s);
                }
            }),
            "at least one analysis variable must not be supplementary"
        );
        assert_eq!(
            err(&|d| d.objects = "1-4".into()),
            "at least three cases must stay out of the supplementary ones"
        );
        assert_eq!(
            err(&|d| d.dimensions = "0".into()),
            "the number of dimensions must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.dimensions = "1000000000".into()),
            "the number of dimensions must be below 1000000000"
        );
        assert_eq!(
            err(&|d| {
                d.normalization = 4;
                d.q = "2".into();
            }),
            "the normalization value lies from -1 to 1"
        );
        assert_eq!(
            err(&|d| d.convergence = "0".into()),
            "the convergence criterion must be above 0"
        );
        assert_eq!(
            err(&|d| {
                d.plots[5] = true;
                d.plot_dimensions = "3".into();
            }),
            "the plotted dimensions per variable must be at most 2"
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
                d.save[1] = true;
                d.save_roots[1] = "scores".into();
            }),
            "the root of the object scores has at most 5 characters"
        );
        assert_eq!(
            err(&|d| {
                d.save[0] = true;
                d.save_roots[0] = "#q".into();
            }),
            "#q cannot start variable names"
        );
        assert_eq!(
            err(&|d| {
                d.outfile = [1, 2, 0];
                d.outfile_files[0] = "Same".into();
                d.outfile_datasets[1] = "same".into();
            }),
            "the quantified variables need a place of their own"
        );
        assert_eq!(err(&|d| d.outfile[2] = 2), "name the new dataset");
        // settings that are switched off are not checked
        let mut d = base();
        d.settings.push(McaVariable {
            groups: "x".into(),
            width: "-1".into(),
            ..McaVariable::new("tea")
        });
        d.q = "5".into();
        d.plots = [false; 7];
        d.label_length = "x".into();
        d.plot_dimensions = "9".into();
        d.ndim = true;
        d.ndim_last = "9".into();
        d.save_roots = ["#bad".into(), "toolong".into()];
        d.outfile_files[0] = "a".into();
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
        // lists of tables and plots left unchecked leave the defaults alone
        let mut d = base();
        d.quant = v(&["tea"]);
        d.plot_variables[2] = v(&["tea"]);
        d.biplot_labels = v(&["tea"]);
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/ANALYSIS=tea snack seat.")
        );
    }

    #[test]
    fn lists_keep_to_the_variables_still_chosen() {
        let data = data();
        let mut d = base();
        d.labels = v(&["note", "Tea"]);
        d.print[4] = true;
        d.object_label = "note".into();
        d.object_shown = v(&["seat", "gone"]);
        d.descrip = v(&["note"]);
        d.plot_variables[0] = v(&["note"]);
        d.plot_variables[1] = v(&["note", "snack"]);
        d.plots[1] = true;
        d.plots[BIPLOT] = true;
        d.biplot_labels = v(&["note"]);
        d.normalization = INDEPENDENT;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MULTIPLE CORRESPONDENCE VARIABLES=tea snack seat note\n  /ANALYSIS=tea snack seat\n  \
             /NORMALIZATION=INDEPENDENT\n  \
             /PRINT=CORR DESCRIP(note) DISCRIM OBJECT((note) seat)\n  \
             /PLOT=OBJECT(note) CATEGORY(snack) DISCRIM."
        );
    }
}
