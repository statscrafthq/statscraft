//! The Categorical Principal Components dialog: CATPCA of variables each with its scaling level
//! (with the spline's degree and knots), weight, recoding, missing value handling and role,
//! with a start or fixed configuration, rotation, tables, plots, saved variables and written
//! data. What it shares with Multiple Correspondence is in `scaling_dialog`. Options left as
//! they are add nothing to the syntax, so the command runs with its own defaults.

use crate::loglinear_dialog::optional;
use crate::scaling_dialog::{
    INDEPENDENT, PrintLists, ScalingVariable, all_variables, analysis_list, analysis_settings,
    bounded, data_subcommands, label_length, model_subcommands, ndim, outfile_syntax, paren,
    per_variable, print_syntax, save_syntax, setting, still,
};
use crate::syntax_text::{number, quote};
use statscraft_data::Dataset;

/// The optimal scaling levels; the first is the default, the first two are splines.
pub const CATPCA_LEVELS: [(&str, &str); 6] = [
    ("SPORD", "Spline ordinal"),
    ("SPNOM", "Spline nominal"),
    ("ORDI", "Ordinal"),
    ("NOMI", "Nominal"),
    ("MNOM", "Multiple nominal"),
    ("NUME", "Numeric"),
];

pub const MNOM: usize = 4;

/// The highest spline degree the dialog takes; higher ones only cost time.
const MOST_DEGREE: u32 = 100;

/// The scaling level of a variable, with the degree and interior knots of a spline.
#[derive(Clone, PartialEq, Debug)]
pub struct Level {
    /// Index into `CATPCA_LEVELS`.
    pub level: usize,
    pub degree: String,
    pub knots: String,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            level: 0,
            degree: "2".into(),
            knots: "2".into(),
        }
    }
}

impl Level {
    pub fn spline(&self) -> bool {
        self.level < 2
    }

    /// LEVEL with DEGREE and INKNOT for ANALYSIS; None for a spline ordinal of the default
    /// shape. The degree and knots count only for a spline.
    fn spec(&self, name: &str) -> Result<Option<String>, String> {
        let keyword = CATPCA_LEVELS[self.level].0;
        if !self.spline() {
            return Ok(Some(format!("LEVEL={keyword}")));
        }
        let what = format!("the spline degree of {name}");
        let degree = bounded(&self.degree, 1, &what)?.unwrap_or(2);
        if degree > MOST_DEGREE {
            return Err(format!("{what} goes from 1 to {MOST_DEGREE}"));
        }
        // the command lowers more knots than the categories allow
        let knots = bounded(&self.knots, 0, &format!("the interior knots of {name}"))?.unwrap_or(2);
        if self.level == 0 && degree == 2 && knots == 2 {
            return Ok(None);
        }
        let mut words = vec![format!("LEVEL={keyword}")];
        if degree != 2 {
            words.push(format!("DEGREE={degree}"));
        }
        if knots != 2 {
            words.push(format!("INKNOT={knots}"));
        }
        Ok(Some(words.join(" ")))
    }
}

/// The settings of one analysis variable.
pub type CatpcaVariable = ScalingVariable<Level>;

/// Where the objects start: from the command's own start, or a file's coordinates as a start
/// or fixed.
pub const CATPCA_CONFIGURATIONS: [(&str, &str); 3] = [
    ("", "The command's own start"),
    ("INITIAL", "Start from a file"),
    ("FIXED", "Fixed to a file"),
];

const FIXED: usize = 2;

pub const CATPCA_ROTATIONS: [(&str, &str); 6] = [
    ("", "None"),
    ("VARIMAX", "Varimax"),
    ("EQUAMAX", "Equamax"),
    ("QUARTIMAX", "Quartimax"),
    ("PROMAX", "Promax"),
    ("OBLIMIN", "Direct oblimin"),
];

pub const PROMAX: usize = 4;
pub const OBLIMIN: usize = 5;

/// The tables of /PRINT; DESCRIP, OBJECT and QUANT take variable lists.
pub const CATPCA_PRINT: [(&str, &str); 8] = [
    ("CORR", "Correlations of quantified variables"),
    ("DESCRIP", "Descriptives"),
    ("HISTORY", "Iteration history"),
    ("LOADING", "Component loadings"),
    ("OBJECT", "Object scores"),
    ("OCORR", "Correlations of original variables"),
    ("QUANT", "Category quantifications"),
    ("VAF", "Variance accounted for"),
];

const CATPCA_DEFAULT_PRINT: [bool; 8] = [true, true, false, true, false, false, false, false];
pub const PRINT_DESCRIP: usize = 1;
pub const PRINT_LOADING: usize = 3;
pub const PRINT_OBJECT: usize = 4;
pub const PRINT_QUANT: usize = 6;

/// The charts of /PLOT.
pub const CATPCA_PLOTS: [(&str, &str); 10] = [
    ("OBJECT", "Object points"),
    ("LOADING", "Component loadings"),
    ("CATEGORY", "Category points"),
    ("JOINTCAT", "Joint category points"),
    ("TRANS", "Transformations"),
    ("RESID", "Residuals"),
    ("BIPLOT", "Biplot"),
    ("TRIPLOT", "Triplot"),
    ("PROJCENTR", "Projected centroids"),
    ("VAF", "Variance accounted for"),
];

const CATPCA_DEFAULT_PLOTS: [bool; 10] = [
    true, true, false, false, false, false, false, false, false, false,
];
pub const OBJECT: usize = 0;
pub const LOADING: usize = 1;
pub const TRANS: usize = 4;
pub const RESID: usize = 5;
pub const BIPLOT: usize = 6;
pub const TRIPLOT: usize = 7;
pub const PROJCENTR: usize = 8;
pub const VAF: usize = 9;

/// The variables /SAVE adds, with the longest root the command keeps.
pub const CATPCA_SAVE: [(&str, &str, usize); 3] = [
    ("TRDATA", "Quantified variables", 5),
    ("OBJECT", "Object scores", 5),
    ("APPROX", "Approximations", 5),
];

/// The data /OUTFILE writes.
pub const CATPCA_OUTFILE: [(&str, &str); 4] = [
    ("DISCRDATA", "Recoded data"),
    ("TRDATA", "Quantified variables"),
    ("OBJECT", "Object scores"),
    ("APPROX", "Approximations"),
];

/// CATPCA: principal components of variables quantified by optimal scaling.
#[derive(Clone, PartialEq, Debug)]
pub struct Catpca {
    pub variables: Vec<String>,
    /// The settings of the analysis variables, found by name; a variable without one has the
    /// defaults.
    pub settings: Vec<CatpcaVariable>,
    /// Variables outside the analysis that label objects or are shown beside them.
    pub labels: Vec<String>,
    /// The supplementary cases by number, with ranges such as 4-9.
    pub objects: String,
    /// Index into `CATPCA_CONFIGURATIONS`, and the data file it reads.
    pub configuration: usize,
    pub configuration_file: String,
    pub dimensions: String,
    /// Index into `SCALING_NORMALIZATIONS`.
    pub normalization: usize,
    /// The custom normalization value.
    pub q: String,
    pub max_iterations: String,
    pub convergence: String,
    /// Index into `CATPCA_ROTATIONS`, with the parameters of the oblique ones.
    pub rotation: usize,
    pub kappa: String,
    pub delta: String,
    pub kaiser: bool,
    /// One flag per entry of `CATPCA_PRINT`.
    pub print: [bool; 8],
    /// Loadings sorted by size.
    pub sort_loadings: bool,
    /// The variables of the descriptives, quantifications and object score tables; none means
    /// the command's own choice.
    pub descrip: Vec<String>,
    pub quant: Vec<String>,
    pub object_shown: Vec<String>,
    /// Labels the object scores; empty for case numbers.
    pub object_label: String,
    /// One flag per entry of `CATPCA_PLOTS`.
    pub plots: [bool; 10],
    /// The variables of each plot: the labeling ones for object points, the ones projected on
    /// for projected centroids; none means all, or for object points the case numbers.
    pub plot_variables: [Vec<String>; 10],
    /// Centroids of multiple nominal variables in the loadings plot, of these (none: all).
    pub loading_centroids: bool,
    pub centroids: Vec<String>,
    /// Centroids instead of loadings in the biplot.
    pub biplot_centroids: bool,
    /// Label the objects of the biplot and the triplot; none for case numbers.
    pub biplot_labels: Vec<String>,
    pub triplot_labels: Vec<String>,
    /// The variable whose centroids are projected.
    pub projected: String,
    /// The dimensions plotted for each multiple nominal variable in transformation and
    /// residual plots.
    pub plot_dimensions: String,
    /// The longest label in the plots; empty for labels in full.
    pub label_length: String,
    pub ndim: bool,
    pub ndim_first: String,
    pub ndim_last: String,
    /// One flag per entry of `CATPCA_SAVE`, with its root; an empty root is the command's own.
    pub save: [bool; 3],
    pub save_roots: [String; 3],
    /// Per entry of `CATPCA_OUTFILE`: nowhere (0), a data file (1) or a new dataset (2).
    pub outfile: [usize; 4],
    pub outfile_files: [String; 4],
    pub outfile_datasets: [String; 4],
}

impl Default for Catpca {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            settings: Vec::new(),
            labels: Vec::new(),
            objects: String::new(),
            configuration: 0,
            configuration_file: String::new(),
            dimensions: "2".into(),
            normalization: 0,
            q: "0".into(),
            max_iterations: "100".into(),
            convergence: "0.00001".into(),
            rotation: 0,
            kappa: "4".into(),
            delta: "0".into(),
            kaiser: true,
            print: CATPCA_DEFAULT_PRINT,
            sort_loadings: false,
            descrip: Vec::new(),
            quant: Vec::new(),
            object_shown: Vec::new(),
            object_label: String::new(),
            plots: CATPCA_DEFAULT_PLOTS,
            plot_variables: Default::default(),
            loading_centroids: false,
            centroids: Vec::new(),
            biplot_centroids: false,
            biplot_labels: Vec::new(),
            triplot_labels: Vec::new(),
            projected: String::new(),
            plot_dimensions: "1".into(),
            label_length: String::new(),
            ndim: false,
            ndim_first: "1".into(),
            ndim_last: "2".into(),
            save: [false; 3],
            save_roots: Default::default(),
            outfile: [0; 4],
            outfile_files: Default::default(),
            outfile_datasets: Default::default(),
        }
    }
}

impl Catpca {
    /// The settings of an analysis variable, the defaults when it has none yet.
    pub fn setting(&self, name: &str) -> CatpcaVariable {
        setting(&self.settings, name)
    }

    /// Every variable on VARIABLES: the analysis ones, then the labelling ones not among them.
    pub fn all_variables(&self) -> Vec<String> {
        all_variables(&self.variables, &self.labels)
    }

    /// The analysis variables that are (`multiple`) or are not multiple nominal.
    pub fn by_level(&self, multiple: bool) -> Vec<String> {
        self.variables
            .iter()
            .filter(|n| (self.setting(n).level.level == MNOM) == multiple)
            .cloned()
            .collect()
    }

    /// The biplot and triplot are not drawn with independent normalization, so the dialog
    /// offers them only with the others.
    pub fn joint_plots_allowed(&self) -> bool {
        self.normalization != INDEPENDENT
    }

    /// The variables a plot may list.
    pub fn plot_options(&self, plot: usize) -> Vec<String> {
        match plot {
            OBJECT => self.all_variables(),
            BIPLOT if self.biplot_centroids => self.variables.clone(),
            LOADING | BIPLOT | PROJCENTR => self.by_level(false),
            _ => self.variables.clone(),
        }
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let settings = analysis_settings(&self.variables, &self.settings)?;
        let all = self.all_variables();
        let mut entries = Vec::new();
        for s in &settings {
            let parts: Vec<String> = [s.weight_spec()?, s.level.spec(&s.name)?]
                .into_iter()
                .flatten()
                .collect();
            entries.push((s.name.as_str(), parts.join(" ")));
        }
        let mut text = format!(
            "CATPCA VARIABLES={}\n  /ANALYSIS={}",
            all.join(" "),
            analysis_list(&entries)
        );
        let fixed = self.configuration == FIXED;
        // a fixed configuration places every variable after it and keeps every case
        if fixed && !self.objects.trim().is_empty() {
            return Err("a fixed configuration takes no supplementary cases".into());
        }
        if fixed && settings.iter().any(|s| s.supplementary) {
            return Err("a fixed configuration takes no supplementary variables".into());
        }
        text.push_str(&data_subcommands(&settings, &self.objects, data)?);
        if self.configuration > 0 {
            let file = self.configuration_file.trim();
            if file.is_empty() {
                return Err("choose the configuration file".into());
            }
            text.push_str(&format!(
                "\n  /CONFIGURATION={}({})",
                CATPCA_CONFIGURATIONS[self.configuration].0,
                quote(file)
            ));
        }
        let (model, dims) = model_subcommands(
            &self.dimensions,
            self.normalization,
            &self.q,
            &self.max_iterations,
            &self.convergence,
        )?;
        // the scores of a fixed configuration keep their own scale
        if fixed && model.contains("/NORMALIZATION") {
            return Err("a fixed configuration keeps its own scale; leave the normalization at variable principal".into());
        }
        text.push_str(&model);
        text.push_str(&self.rotation_syntax(&settings, dims)?);
        text.push_str(&print_syntax(
            &CATPCA_PRINT,
            &self.print,
            &CATPCA_DEFAULT_PRINT,
            (&all, &self.variables),
            PrintLists {
                descrip: &self.descrip,
                quant: &self.quant,
                shown: &self.object_shown,
                label: &self.object_label,
            },
            self.sort_loadings.then_some(("LOADING", "LOADING(SORT)")),
        ));
        text.push_str(&self.plot_syntax(&all, dims)?);
        text.push_str(&save_syntax(&CATPCA_SAVE, &self.save, &self.save_roots)?);
        let (declare, outfile) = outfile_syntax(
            &CATPCA_OUTFILE,
            &self.outfile,
            &self.outfile_files,
            &self.outfile_datasets,
        )?;
        text.push_str(&outfile);
        text.push('.');
        Ok(declare + &text)
    }

    /// /ROTATION with its parameter and NOKAISER when they differ from the defaults.
    fn rotation_syntax(&self, settings: &[CatpcaVariable], dims: u32) -> Result<String, String> {
        if self.rotation == 0 {
            return Ok(String::new());
        }
        // the command skips a rotation it cannot do, with a warning
        if dims < 2 {
            return Err("a rotation needs at least two dimensions".into());
        }
        let loaded = settings
            .iter()
            .filter(|s| !s.supplementary && s.level.level != MNOM)
            .count();
        if loaded < 2 {
            return Err(
                "a rotation needs two active variables that are not multiple nominal".into(),
            );
        }
        let mut word = CATPCA_ROTATIONS[self.rotation].0.to_string();
        match self.rotation {
            PROMAX => {
                if let Some(k) = optional(&self.kappa, "the kappa of promax")? {
                    if !(1.0..10000.0).contains(&k) {
                        return Err("the kappa of promax lies from 1 to below 10000".into());
                    }
                    if k != 4.0 {
                        word.push_str(&format!("({})", number(k)));
                    }
                }
            }
            OBLIMIN => {
                if let Some(delta) = optional(&self.delta, "the delta of oblimin")? {
                    if delta > 0.8 {
                        return Err("the delta of oblimin is at most 0.8".into());
                    }
                    if delta != 0.0 {
                        word.push_str(&format!("({})", number(delta)));
                    }
                }
            }
            _ => {}
        }
        if !self.kaiser {
            word.push_str(" NOKAISER");
        }
        Ok(format!("\n  /ROTATION={word}"))
    }

    /// /PLOT when the charts, their lists or their labels differ from the default.
    fn plot_syntax(&self, all: &[String], dims: u32) -> Result<String, String> {
        let mut plots = self.plots;
        plots[BIPLOT] &= self.joint_plots_allowed();
        plots[TRIPLOT] &= self.joint_plots_allowed();
        let chosen: Vec<Vec<String>> = (0..CATPCA_PLOTS.len())
            .map(|i| still(&self.plot_options(i), &self.plot_variables[i]))
            .collect();
        let multiple = self.by_level(true);
        let centroids = (plots[LOADING] && self.loading_centroids)
            .then(|| paren(&still(&multiple, &self.centroids)));
        if !plots.contains(&true) {
            return Ok("\n  /PLOT=NONE".into());
        }
        let labeled = plots.iter().enumerate().any(|(i, on)| *on && i != VAF);
        let length = label_length(&self.label_length, labeled)?;
        let per_variable = per_variable(&self.plot_dimensions, plots[TRANS] || plots[RESID], dims)?;
        // a plot that needs a list gets every analysis variable when none is checked
        let every = |l: &[String]| match l.is_empty() {
            true => self.variables.clone(),
            false => l.to_vec(),
        };
        let mut words = Vec::new();
        for (i, (keyword, _)) in CATPCA_PLOTS.iter().enumerate() {
            if !plots[i] {
                continue;
            }
            let list = &chosen[i];
            words.push(match i {
                LOADING => match (list.is_empty(), &centroids) {
                    (true, None) => format!("LOADING{length}"),
                    (false, None) => format!("LOADING{}{length}", paren(list)),
                    (true, Some(c)) => format!("LOADING(CENTR{c}){length}"),
                    (false, Some(c)) => format!("LOADING({} (CENTR{c})){length}", list.join(" ")),
                },
                TRANS | RESID => {
                    // only a multiple nominal variable has more than one dimension to show
                    let named: Vec<String> = every(list)
                        .iter()
                        .map(|n| match multiple.contains(n) {
                            true => format!("{n}{per_variable}"),
                            false => n.clone(),
                        })
                        .collect();
                    format!("{keyword}{}{length}", paren(&named))
                }
                BIPLOT => {
                    let labels = still(all, &self.biplot_labels);
                    let kind = match self.biplot_centroids {
                        true => "CENTR",
                        false => "LOADING",
                    };
                    match list.is_empty() && labels.is_empty() && !self.biplot_centroids {
                        true => format!("BIPLOT{length}"),
                        // the kind is written out so a label variable is not read as one
                        false => format!("BIPLOT({kind}{}){}{length}", paren(list), paren(&labels)),
                    }
                }
                TRIPLOT => {
                    let labels = still(all, &self.triplot_labels);
                    match list.is_empty() && labels.is_empty() {
                        true => format!("TRIPLOT{length}"),
                        false => format!(
                            "TRIPLOT({}{}){length}",
                            every(list).join(" "),
                            paren(&labels)
                        ),
                    }
                }
                PROJCENTR => {
                    if !self.variables.contains(&self.projected) {
                        return Err("choose the variable whose centroids are projected".into());
                    }
                    let onto = match list.is_empty() {
                        true => self.by_level(false),
                        false => list.clone(),
                    };
                    if onto.is_empty() {
                        return Err("projected centroids need a variable that is not multiple nominal to project onto".into());
                    }
                    format!("PROJCENTR({}, {}){length}", self.projected, onto.join(" "))
                }
                VAF => "VAF".into(),
                _ => {
                    let list = match *keyword {
                        // the category plots need a list
                        "CATEGORY" => every(list),
                        _ => list.clone(),
                    };
                    format!("{keyword}{}{length}", paren(&list))
                }
            });
        }
        if self.ndim {
            words.push(ndim(&self.ndim_first, &self.ndim_last, dims)?);
        } else if plots == CATPCA_DEFAULT_PLOTS
            && length.is_empty()
            && centroids.is_none()
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

    /// Six invented cases of three answers on a scale and a note.
    fn data() -> Dataset {
        let mut session = Session::default();
        session
            .run(
                "DATA LIST LIST /calm pace mood note (3F1 A4).
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

    fn base() -> Catpca {
        Catpca {
            variables: v(&["calm", "pace", "mood"]),
            ..Default::default()
        }
    }

    fn level(name: &str, change: &dyn Fn(&mut Level)) -> CatpcaVariable {
        let mut s = CatpcaVariable::new(name);
        change(&mut s.level);
        s
    }

    #[test]
    fn levels_write_only_what_differs_from_a_spline_ordinal() {
        let data = data();
        let mut d = base();
        d.settings = vec![
            level("calm", &|l| l.degree = "1".into()),
            level("pace", &|l| {
                l.level = 1;
                l.knots = "0".into();
            }),
            level("mood", &|l| {
                l.level = MNOM;
                // a spline's shape does not count for other levels
                l.degree = "x".into();
            }),
        ];
        assert_eq!(
            d.syntax(&data).unwrap(),
            "CATPCA VARIABLES=calm pace mood\n  \
             /ANALYSIS=calm(LEVEL=SPORD DEGREE=1) pace(LEVEL=SPNOM INKNOT=0) mood(LEVEL=MNOM)."
        );
        d.settings = vec![level("pace", &|l| l.level = 5)];
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/ANALYSIS=calm(WEIGHT=1) pace(LEVEL=NUME) mood.")
        );
    }

    #[test]
    fn catpca_checks_what_the_command_would_reject() {
        let data = data();
        let err = |change: &dyn Fn(&mut Catpca)| {
            let mut d = base();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.settings = vec![level("calm", &|l| l.degree = "0".into())]),
            "the spline degree of calm must be a whole number of at least 1"
        );
        assert_eq!(
            err(&|d| d.settings = vec![level("calm", &|l| l.degree = "101".into())]),
            "the spline degree of calm goes from 1 to 100"
        );
        assert_eq!(
            err(&|d| d.settings = vec![level("calm", &|l| l.knots = "-1".into())]),
            "the interior knots of calm must be a whole number of at least 0"
        );
        assert_eq!(
            err(&|d| {
                d.rotation = 1;
                d.dimensions = "1".into();
            }),
            "a rotation needs at least two dimensions"
        );
        assert_eq!(
            err(&|d| {
                d.rotation = 1;
                d.settings = vec![level("calm", &|l| l.level = MNOM)];
                d.settings.push(CatpcaVariable {
                    supplementary: true,
                    ..CatpcaVariable::new("pace")
                });
            }),
            "a rotation needs two active variables that are not multiple nominal"
        );
        assert_eq!(
            err(&|d| {
                d.rotation = PROMAX;
                d.kappa = "0.5".into();
            }),
            "the kappa of promax lies from 1 to below 10000"
        );
        assert_eq!(
            err(&|d| {
                d.rotation = OBLIMIN;
                d.delta = "0.9".into();
            }),
            "the delta of oblimin is at most 0.8"
        );
        assert_eq!(
            err(&|d| d.configuration = 1),
            "choose the configuration file"
        );
        assert_eq!(
            err(&|d| {
                d.configuration = FIXED;
                d.configuration_file = "start.sav".into();
                d.objects = "1".into();
            }),
            "a fixed configuration takes no supplementary cases"
        );
        assert_eq!(
            err(&|d| {
                d.configuration = FIXED;
                d.configuration_file = "start.sav".into();
                d.normalization = 1;
            }),
            "a fixed configuration keeps its own scale; leave the normalization at variable principal"
        );
        assert_eq!(
            err(&|d| d.plots[PROJCENTR] = true),
            "choose the variable whose centroids are projected"
        );
        assert_eq!(
            err(&|d| {
                d.plots[PROJCENTR] = true;
                d.projected = "calm".into();
                d.settings = ["calm", "pace", "mood"]
                    .iter()
                    .map(|n| level(n, &|l| l.level = MNOM))
                    .collect();
            }),
            "projected centroids need a variable that is not multiple nominal to project onto"
        );
        assert_eq!(
            err(&|d| {
                d.save[2] = true;
                d.save_roots[2] = "approx".into();
            }),
            "the root of the approximations has at most 5 characters"
        );
        // settings that are switched off are not checked
        let mut d = base();
        d.kappa = "0".into();
        d.delta = "5".into();
        d.kaiser = false;
        d.plots = [false; 10];
        d.projected = "gone".into();
        d.plot_dimensions = "9".into();
        d.loading_centroids = true;
        assert!(d.syntax(&data).unwrap().ends_with("/PLOT=NONE."));
        // biplot and triplot are left out with independent normalization
        let mut d = base();
        d.normalization = INDEPENDENT;
        d.plots[BIPLOT] = true;
        d.plots[TRIPLOT] = true;
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/NORMALIZATION=INDEPENDENT.")
        );
    }

    #[test]
    fn plot_forms_parse() {
        let data = data();
        let mut d = base();
        d.labels = v(&["note"]);
        d.settings = vec![level("mood", &|l| l.level = MNOM)];
        d.rotation = OBLIMIN;
        d.delta = "-0.5".into();
        d.plots = [false; 10];
        d.plots[LOADING] = true;
        d.loading_centroids = true;
        d.plots[BIPLOT] = true;
        d.biplot_centroids = true;
        d.plot_variables[BIPLOT] = v(&["mood"]);
        d.plots[TRIPLOT] = true;
        d.triplot_labels = v(&["note"]);
        let syntax = d.syntax(&data).unwrap();
        assert!(
            syntax.ends_with(
                "/ROTATION=OBLIMIN(-0.5)\n  \
                 /PLOT=LOADING(CENTR) BIPLOT(CENTR(mood)) TRIPLOT(calm pace mood(note))."
            ),
            "{syntax}"
        );
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{errors:?}");
        // a label variable after the biplot follows the kind written out
        d.plots = [false; 10];
        d.plots[BIPLOT] = true;
        d.biplot_centroids = false;
        d.biplot_labels = v(&["note"]);
        let syntax = d.syntax(&data).unwrap();
        assert!(syntax.ends_with("/PLOT=BIPLOT(LOADING)(note)."), "{syntax}");
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{errors:?}");
    }
}
