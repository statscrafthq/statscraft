//! The MANOVA (classic), Partial Least Squares (PLS) and Nearest Neighbor (KNN) dialogs. Options
//! left as they are add nothing to the syntax, so the command runs with its own defaults.

use crate::i18n::{FluentValue, t, t_args};
use crate::stats_dialogs::numeric;
use crate::syntax_text::names;
use statscraft_data::Dataset;
use std::collections::BTreeMap;

/// An error naming the first variable given in two roles.
fn one_role_each(names: &[&str]) -> Result<(), String> {
    match names
        .iter()
        .enumerate()
        .find(|(i, n)| names[..*i].iter().any(|m| m.eq_ignore_ascii_case(n)))
    {
        Some((_, name)) => Err(t_args(
            "stats-multivar-two-roles",
            &[("name", (*name).into())],
        )),
        None => Ok(()),
    }
}

/// A whole number the user typed, at least `least`.
fn whole(text: &str, what: &str, least: i64) -> Result<i64, String> {
    text.trim()
        .parse::<i64>()
        .ok()
        .filter(|n| *n >= least && *n < 1_000_000_000)
        .ok_or_else(|| {
            t_args(
                "stats-multivar-whole-at-least",
                &[("what", what.into()), ("least", least.to_string().into())],
            )
        })
}

/// A dataset or variable name the user typed for something the command creates.
fn new_name(text: &str, what: &str) -> Result<String, String> {
    let t = text.trim();
    let ok = t.chars().next().is_some_and(|c| c.is_alphabetic())
        && t.chars()
            .all(|c| c.is_alphanumeric() || "_.$#@".contains(c));
    match ok {
        true => Ok(t.to_string()),
        false => Err(t_args("stats-multivar-name-rule", &[("what", what.into())])),
    }
}

/// The error terms MANOVA /ERROR names; the first is the command's own default.
pub const MANOVA_ERRORS: [(&str, &str); 4] = [
    ("", "stats-multivar-error-default"),
    ("WITHIN", "stats-multivar-error-within"),
    ("RESIDUAL", "stats-multivar-error-residual"),
    ("WR", "stats-multivar-error-within-residual"),
];

/// The tables of MANOVA /PRINT: the text written, and the label. The last four are the extra
/// tables of SIGNIF and need two or more dependents.
pub const MANOVA_PRINT: [(&str, &str); 8] = [
    ("CELLINFO(MEANS)", "stats-multivar-cell-means"),
    ("HOMOGENEITY(ALL)", "stats-multivar-homogeneity"),
    ("PARAMETERS(ESTIM)", "stats-multivar-parameter-estimates"),
    ("ERROR(SSCP)", "stats-multivar-error-matrix"),
    ("EIGEN", "stats-multivar-eigenvalues"),
    ("DIMENR", "stats-multivar-dimension-reduction"),
    ("HYPOTH", "stats-multivar-hypothesis-matrix"),
    ("STEPDOWN", "stats-multivar-stepdown"),
];

/// The matrices MANOVA /TRANSFORM can build, as the keyword and the label; the first is no
/// transformation. SPECIAL needs a typed matrix and is left out.
pub const MANOVA_TRANSFORMS: [(&str, &str); 7] = [
    ("", "stats-multivar-transform-none"),
    ("DEVIATION", "stats-multivar-deviation"),
    ("SIMPLE", "stats-multivar-simple"),
    ("DIFFERENCE", "stats-multivar-difference"),
    ("HELMERT", "Helmert"),
    ("REPEATED", "stats-multivar-repeated"),
    ("POLYNOMIAL", "stats-multivar-polynomial"),
];

/// MANOVA, the older command for analysis of variance of one or several dependents.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Manova {
    pub dependents: Vec<String>,
    pub factors: Vec<String>,
    /// The lowest and highest value of each factor, by name.
    pub ranges: BTreeMap<String, (String, String)>,
    pub covariates: Vec<String>,
    /// Index into `MANOVA_ERRORS`.
    pub error: usize,
    /// Sums of squares of each term adjusted only for the terms before it.
    pub sequential: bool,
    /// Only the main effects of the factors instead of every interaction too.
    pub main_effects: bool,
    /// One flag per entry of `MANOVA_PRINT`.
    pub print: [bool; 8],
    /// Within-subjects factors: name and number of levels. The dependents are then the cells,
    /// the last factor changing fastest, once for each measure.
    pub ws_factors: Vec<(String, String)>,
    /// Measure names, separated by blanks; empty for one unnamed measure.
    pub measure: String,
    /// WSDESIGN with the main effects of the within-subjects factors only.
    pub ws_main_effects: bool,
    /// Index into `MANOVA_TRANSFORMS`; cannot be combined with within-subjects factors.
    pub transform: usize,
    pub orthonorm: bool,
    /// The transposed inverse of the matrix instead of the matrix itself.
    pub basis: bool,
}

impl Manova {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let dependents = names(&self.dependents, t("stats-multivar-the-dependents"))?;
        if self.factors.is_empty() {
            return Err(t("stats-multivar-add-factor").into());
        }
        let all: Vec<&str> = self
            .dependents
            .iter()
            .chain(&self.factors)
            .chain(&self.covariates)
            .map(String::as_str)
            .collect();
        one_role_each(&all)?;
        numeric(
            data,
            self.dependents
                .iter()
                .chain(&self.factors)
                .chain(&self.covariates),
        )?;
        let mut factors = Vec::new();
        for f in &self.factors {
            let (lo, hi) = self.ranges.get(f).cloned().unwrap_or_default();
            let factor = [("factor", FluentValue::from(f.as_str()))];
            let (lo, hi) = (
                whole(
                    &lo,
                    &t_args("stats-multivar-lowest-value-of", &factor),
                    -999_999,
                )?,
                whole(
                    &hi,
                    &t_args("stats-multivar-highest-value-of", &factor),
                    -999_999,
                )?,
            );
            if lo >= hi {
                return Err(t_args("stats-multivar-range-order", &factor));
            }
            factors.push(format!("{f}({lo},{hi})"));
        }
        let mut text = format!("MANOVA {dependents} BY {}", factors.join(" "));
        if !self.covariates.is_empty() {
            // with repeated measures a bare covariate would need one per cell; in parentheses
            // it is one value per case, constant over the cells
            let list = self.covariates.join(" ");
            match self.ws_factors.is_empty() {
                true => text.push_str(&format!(" WITH {list}")),
                false => text.push_str(&format!(" WITH ({list})")),
            }
        }
        text.push_str(&self.within_subjects()?);
        if self.transform != 0 {
            let flags = [(self.orthonorm, "ORTHONORM "), (self.basis, "BASIS ")];
            let flags: String = flags.iter().filter(|f| f.0).map(|f| f.1).collect();
            text.push_str(&format!(
                "\n  /TRANSFORM={flags}{}",
                MANOVA_TRANSFORMS[self.transform].0
            ));
        }
        if self.error != 0 {
            text.push_str(&format!("\n  /ERROR={}", MANOVA_ERRORS[self.error].0));
        }
        if self.sequential {
            text.push_str("\n  /METHOD=SEQUENTIAL");
        }
        let mut items: Vec<String> = Vec::new();
        let mut signif = Vec::new();
        for ((key, _), on) in MANOVA_PRINT.iter().zip(self.print) {
            match (on, key.contains('(')) {
                (true, true) => items.push((*key).to_string()),
                (true, false) => signif.push(*key),
                _ => {}
            }
        }
        if !signif.is_empty() {
            items.push(format!("SIGNIF({})", signif.join(" ")));
        }
        if !items.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", items.join(" ")));
        }
        if self.main_effects {
            text.push_str(&format!("\n  /DESIGN={}", self.factors.join(" ")));
        }
        text.push('.');
        Ok(text)
    }

    /// WSFACTORS, MEASURE and WSDESIGN; the first must follow the variables directly.
    fn within_subjects(&self) -> Result<String, String> {
        if self.ws_factors.is_empty() {
            return Ok(String::new());
        }
        if self.transform != 0 {
            return Err(t("stats-multivar-ws-and-transform").into());
        }
        let mut groups = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut cells = 1;
        for (name, levels) in &self.ws_factors {
            let name = new_name(name, t("stats-multivar-ws-factor-name"))?;
            if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                return Err(t_args(
                    "stats-multivar-ws-named-twice",
                    &[("name", name.as_str().into())],
                ));
            }
            let levels = whole(
                levels,
                &t_args(
                    "stats-multivar-levels-of",
                    &[("name", name.as_str().into())],
                ),
                2,
            )?;
            cells *= levels as usize;
            groups.push(format!("{name}({levels})"));
            names.push(name);
        }
        let measures: Vec<String> = self
            .measure
            .split_whitespace()
            .map(|m| new_name(m, t("stats-multivar-measure-name")))
            .collect::<Result<_, _>>()?;
        let expected = cells * measures.len().max(1);
        if self.dependents.len() != expected {
            let args = [
                ("expected", FluentValue::from(expected.to_string())),
                ("got", FluentValue::from(self.dependents.len().to_string())),
            ];
            return Err(match measures.len() > 1 {
                true => t_args("stats-multivar-ws-need-dependents-measures", &args),
                false => t_args("stats-multivar-ws-need-dependents", &args),
            });
        }
        let mut text = format!("\n  /WSFACTORS={}", groups.join(" "));
        if !measures.is_empty() {
            text.push_str(&format!("\n  /MEASURE={}", measures.join(" ")));
        }
        if self.ws_main_effects && self.ws_factors.len() > 1 {
            text.push_str(&format!("\n  /WSDESIGN={}", names.join(" ")));
        }
        Ok(text)
    }
}

/// The default number of latent factors of PLS.
const PLS_FACTORS: i64 = 5;

/// PLS: partial least squares regression.
#[derive(Clone, PartialEq, Debug)]
pub struct Pls {
    pub scale: Vec<String>,
    pub categorical: Vec<String>,
    /// The reference category of the categorical dependents: the first one instead of the last.
    pub first_reference: bool,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// The variable that names the cases in the saved dataset; may be empty.
    pub id: String,
    pub latent: String,
    pub cases: bool,
    pub cases_name: String,
    pub loadings: bool,
    pub loadings_name: String,
    pub predictors: bool,
    pub predictors_name: String,
}

impl Default for Pls {
    fn default() -> Self {
        Self {
            scale: Vec::new(),
            categorical: Vec::new(),
            first_reference: false,
            factors: Vec::new(),
            covariates: Vec::new(),
            id: String::new(),
            latent: PLS_FACTORS.to_string(),
            cases: false,
            cases_name: "plsCases".into(),
            loadings: false,
            loadings_name: "plsFactors".into(),
            predictors: false,
            predictors_name: "plsPredictors".into(),
        }
    }
}

impl Pls {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        if self.scale.is_empty() && self.categorical.is_empty() {
            return Err(t("stats-multivar-add-dependent").into());
        }
        if self.factors.is_empty() && self.covariates.is_empty() {
            return Err(t("stats-multivar-add-predictor").into());
        }
        let id = self.id.trim();
        let mut all: Vec<&str> = self
            .scale
            .iter()
            .chain(&self.categorical)
            .chain(&self.factors)
            .chain(&self.covariates)
            .map(String::as_str)
            .collect();
        if !id.is_empty() {
            all.push(id);
        }
        one_role_each(&all)?;
        numeric(data, self.scale.iter().chain(&self.covariates))?;
        let mut text = String::from("PLS");
        for n in &self.scale {
            text.push_str(&format!(" {n} MLEVEL=S"));
        }
        let reference = if self.first_reference {
            "FIRST"
        } else {
            "LAST"
        };
        for n in &self.categorical {
            text.push_str(&format!(" {n} MLEVEL=N REFERENCE={reference}"));
        }
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        if !id.is_empty() {
            text.push_str(&format!("\n  /ID VARIABLE={id}"));
        }
        let mut saved = Vec::new();
        for (on, key, name, what) in [
            (
                self.cases,
                "CASES",
                &self.cases_name,
                t("stats-multivar-name-cases-dataset"),
            ),
            (
                self.loadings,
                "LATENTFACTORS",
                &self.loadings_name,
                t("stats-multivar-name-factors-dataset"),
            ),
            (
                self.predictors,
                "PREDICTORS",
                &self.predictors_name,
                t("stats-multivar-name-predictors-dataset"),
            ),
        ] {
            if on {
                saved.push((key, new_name(name, what)?));
            }
        }
        for (i, (_, a)) in saved.iter().enumerate() {
            if saved[..i].iter().any(|(_, b)| b.eq_ignore_ascii_case(a)) {
                return Err(t_args(
                    "stats-multivar-datasets-same-name",
                    &[("name", a.as_str().into())],
                ));
            }
        }
        if !saved.is_empty() {
            let list: Vec<String> = saved.iter().map(|(k, n)| format!("{k}={n}")).collect();
            text.push_str(&format!("\n  /OUTDATASET {}", list.join(" ")));
        }
        let latent = whole(&self.latent, t("stats-multivar-latent-factors"), 1)?;
        if latent != PLS_FACTORS {
            text.push_str(&format!("\n  /CRITERIA LATENTFACTORS={latent}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// The measurement levels of the KNN target; the first leaves it to the variable.
pub const KNN_LEVELS: [(&str, &str); 4] = [
    ("", "stats-multivar-from-variable"),
    ("S", "stats-multivar-scale"),
    ("O", "stats-multivar-ordinal"),
    ("N", "stats-multivar-nominal"),
];

pub const KNN_METRICS: [(&str, &str); 2] = [
    ("EUCLID", "stats-multivar-euclidean"),
    ("CITYBLOCK", "stats-multivar-city-block"),
];

/// The new variables of KNN /SAVE: the keyword and the label.
pub const KNN_SAVE: [(&str, &str); 4] = [
    ("PREDVAL", "stats-multivar-predicted-value"),
    ("PREDPROB", "stats-multivar-predicted-probabilities"),
    ("PARTITION", "stats-multivar-partition"),
    ("FOLD", "stats-multivar-fold"),
];

/// KNN: nearest neighbor classification or prediction.
#[derive(Clone, PartialEq, Debug)]
pub struct Knn {
    pub target: String,
    /// Index into `KNN_LEVELS`.
    pub level: usize,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// The cases are split by a variable (above 0 trains) instead of at random.
    pub by_variable: bool,
    pub partition_variable: String,
    pub training: String,
    pub holdout: String,
    /// Choose the number of neighbors by cross-validation instead of fixing it.
    pub auto: bool,
    pub k: String,
    pub kmin: String,
    pub kmax: String,
    pub folds: String,
    /// Index into `KNN_METRICS`.
    pub metric: usize,
    pub scale_covariates: bool,
    pub select_features: bool,
    pub weight_features: bool,
    /// The median of the neighbors instead of the mean, for a scale target.
    pub median: bool,
    pub include_missing: bool,
    pub show_model: bool,
    /// One flag and one name per entry of `KNN_SAVE`; an empty name takes the default.
    pub save: [bool; 4],
    pub save_names: [String; 4],
}

impl Default for Knn {
    fn default() -> Self {
        Self {
            target: String::new(),
            level: 0,
            factors: Vec::new(),
            covariates: Vec::new(),
            by_variable: false,
            partition_variable: String::new(),
            training: "70".into(),
            holdout: "30".into(),
            auto: false,
            k: "3".into(),
            kmin: "3".into(),
            kmax: "5".into(),
            folds: "10".into(),
            metric: 0,
            scale_covariates: true,
            select_features: false,
            weight_features: false,
            median: false,
            include_missing: false,
            show_model: true,
            save: [false; 4],
            save_names: Default::default(),
        }
    }
}

impl Knn {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err(t("stats-multivar-choose-target").into());
        }
        if self.factors.is_empty() && self.covariates.is_empty() {
            return Err(t("stats-multivar-add-feature").into());
        }
        let partition = self.partition_variable.trim();
        let mut all: Vec<&str> = vec![target];
        all.extend(
            self.factors
                .iter()
                .chain(&self.covariates)
                .map(String::as_str),
        );
        if self.by_variable {
            if partition.is_empty() {
                return Err(t("stats-multivar-choose-partition").into());
            }
            all.push(partition);
        }
        one_role_each(&all)?;
        numeric(data, &self.covariates)?;
        let mut text = format!("KNN {target}");
        if self.level != 0 {
            text.push_str(&format!(" (MLEVEL={})", KNN_LEVELS[self.level].0));
        }
        if !self.factors.is_empty() {
            text.push_str(&format!(" BY {}", self.factors.join(" ")));
        }
        if !self.covariates.is_empty() {
            text.push_str(&format!(" WITH {}", self.covariates.join(" ")));
        }
        if !self.scale_covariates {
            text.push_str("\n  /RESCALE COVARIATE=NONE");
        }
        if self.by_variable {
            text.push_str(&format!("\n  /PARTITION VARIABLE={partition}"));
        } else {
            let training = whole(&self.training, t("stats-multivar-training-share"), 1)?;
            let holdout = whole(&self.holdout, t("stats-multivar-holdout-share"), 0)?;
            if (training, holdout) != (70, 30) {
                text.push_str(&format!(
                    "\n  /PARTITION TRAINING={training} HOLDOUT={holdout}"
                ));
            }
        }
        let mut model = Vec::new();
        if self.metric != 0 {
            model.push(format!("METRIC={}", KNN_METRICS[self.metric].0));
        }
        if self.auto {
            let (lo, hi) = (
                whole(&self.kmin, t("stats-multivar-smallest-k"), 1)?,
                whole(&self.kmax, t("stats-multivar-largest-k"), 1)?,
            );
            if lo >= hi {
                return Err(t("stats-multivar-k-order").into());
            }
            model.push(format!("NEIGHBORS=AUTO(KMIN={lo}, KMAX={hi})"));
        } else {
            let k = whole(&self.k, t("stats-multivar-number-of-neighbors"), 1)?;
            if k != 3 {
                model.push(format!("NEIGHBORS=FIXED(K={k})"));
            }
        }
        if self.select_features {
            model.push("FEATURES=AUTO".into());
        }
        if !model.is_empty() {
            text.push_str(&format!("\n  /MODEL {}", model.join(" ")));
        }
        let mut criteria = Vec::new();
        if self.median {
            criteria.push("PREDICTED=MEDIAN");
        }
        if self.weight_features {
            criteria.push("WEIGHTFEATURES=YES");
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA {}", criteria.join(" ")));
        }
        if self.auto && !self.select_features {
            let folds = whole(&self.folds, t("stats-multivar-number-of-folds"), 2)?;
            if folds != 10 {
                text.push_str(&format!("\n  /CROSSVALIDATION FOLDS={folds}"));
            }
        }
        if self.include_missing {
            text.push_str("\n  /MISSING USERMISSING=INCLUDE");
        }
        if !self.show_model {
            text.push_str("\n  /VIEWMODEL DISPLAY=NO");
        }
        let mut save = Vec::new();
        for (i, ((key, what), on)) in KNN_SAVE.iter().zip(self.save).enumerate() {
            if !on {
                continue;
            }
            let name = self.save_names[i].trim();
            match name.is_empty() {
                true => save.push((*key).to_string()),
                false => save.push(format!(
                    "{key}({})",
                    new_name(
                        name,
                        &t_args(
                            "stats-multivar-name-for",
                            &[("what", t(what).to_lowercase().into())]
                        )
                    )?
                )),
            }
        }
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE {}", save.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn manova_within_subjects_and_transform() {
        let data = Dataset::new(0);
        let mut d = Manova {
            dependents: v(&["t1", "t2", "t3", "t4"]),
            factors: v(&["g"]),
            ranges: [("g".to_string(), ("1".to_string(), "2".to_string()))]
                .into_iter()
                .collect(),
            ws_factors: vec![("trial".into(), "2".into()), ("dose".into(), "2".into())],
            ..Default::default()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MANOVA t1 t2 t3 t4 BY g(1,2)\n  /WSFACTORS=trial(2) dose(2)."
        );
        d.covariates = v(&["x"]);
        assert!(
            d.syntax(&data)
                .unwrap()
                .starts_with("MANOVA t1 t2 t3 t4 BY g(1,2) WITH (x)\n")
        );
        d.covariates.clear();
        d.ws_main_effects = true;
        d.measure = "score".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MANOVA t1 t2 t3 t4 BY g(1,2)\n  /WSFACTORS=trial(2) dose(2)\n  /MEASURE=score\n  \
             /WSDESIGN=trial dose."
        );
        d.measure = "a b".into();
        assert!(d.syntax(&data).unwrap_err().contains("need 8 dependents"));
        d.measure.clear();
        d.dependents.pop();
        assert!(d.syntax(&data).unwrap_err().contains("need 4 dependents"));
        d.dependents.push("t4".into());
        d.ws_factors[1].1 = "1".into();
        assert!(d.syntax(&data).unwrap_err().contains("levels of dose"));
        d.ws_factors[1] = ("trial".into(), "2".into());
        assert!(d.syntax(&data).unwrap_err().contains("named twice"));
        d.ws_factors.truncate(1);
        d.dependents.truncate(2);
        d.transform = 4;
        assert!(
            d.syntax(&data)
                .unwrap_err()
                .contains("cannot be used together")
        );
        d.ws_factors.clear();
        d.orthonorm = true;
        d.basis = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MANOVA t1 t2 BY g(1,2)\n  /TRANSFORM=ORTHONORM BASIS HELMERT."
        );
    }

    #[test]
    fn manova_writes_ranges_options_and_design() {
        let data = Dataset::new(0);
        let mut d = Manova {
            dependents: v(&["a", "b"]),
            factors: v(&["g", "h"]),
            covariates: v(&["x"]),
            ..Default::default()
        };
        assert!(d.syntax(&data).unwrap_err().contains("lowest value of g"));
        d.ranges.insert("g".into(), ("1".into(), "3".into()));
        d.ranges.insert("h".into(), ("0".into(), "1".into()));
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MANOVA a b BY g(1,3) h(0,1) WITH x."
        );
        d.error = 3;
        d.sequential = true;
        d.main_effects = true;
        d.print = [true, false, true, false, true, false, false, true];
        assert_eq!(
            d.syntax(&data).unwrap(),
            "MANOVA a b BY g(1,3) h(0,1) WITH x\n  /ERROR=WR\n  /METHOD=SEQUENTIAL\n  \
             /PRINT=CELLINFO(MEANS) PARAMETERS(ESTIM) SIGNIF(EIGEN STEPDOWN)\n  /DESIGN=g h."
        );
        d.ranges.insert("h".into(), ("2".into(), "2".into()));
        assert!(d.syntax(&data).unwrap_err().contains("below the highest"));
    }

    #[test]
    fn pls_writes_levels_ids_saves_and_factors() {
        let data = Dataset::new(0);
        let mut d = Pls {
            scale: v(&["y"]),
            covariates: v(&["x1", "x2"]),
            ..Default::default()
        };
        assert_eq!(d.syntax(&data).unwrap(), "PLS y MLEVEL=S WITH x1 x2.");
        d.categorical = v(&["c"]);
        d.first_reference = true;
        d.factors = v(&["f"]);
        d.id = "who".into();
        d.latent = "3".into();
        d.cases = true;
        d.predictors = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "PLS y MLEVEL=S c MLEVEL=N REFERENCE=FIRST BY f WITH x1 x2\n  /ID VARIABLE=who\n  \
             /OUTDATASET CASES=plsCases PREDICTORS=plsPredictors\n  /CRITERIA LATENTFACTORS=3."
        );
        d.predictors_name = "PLSCASES".into();
        assert!(d.syntax(&data).unwrap_err().contains("two saved datasets"));
    }

    #[test]
    fn knn_writes_only_what_differs_from_the_defaults() {
        let data = Dataset::new(0);
        let mut d = Knn {
            target: "plan".into(),
            covariates: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax(&data).unwrap(), "KNN plan WITH age.");
        d.level = 3;
        d.factors = v(&["region"]);
        d.training = "8".into();
        d.holdout = "2".into();
        d.auto = true;
        d.kmin = "2".into();
        d.kmax = "6".into();
        d.folds = "5".into();
        d.metric = 1;
        d.scale_covariates = false;
        d.select_features = true;
        d.weight_features = true;
        d.show_model = false;
        d.save = [true, true, false, false];
        d.save_names[0] = "guess".into();
        assert_eq!(
            d.syntax(&data).unwrap(),
            "KNN plan (MLEVEL=N) BY region WITH age\n  /RESCALE COVARIATE=NONE\n  \
             /PARTITION TRAINING=8 HOLDOUT=2\n  \
             /MODEL METRIC=CITYBLOCK NEIGHBORS=AUTO(KMIN=2, KMAX=6) FEATURES=AUTO\n  \
             /CRITERIA WEIGHTFEATURES=YES\n  /VIEWMODEL DISPLAY=NO\n  /SAVE PREDVAL(guess) PREDPROB."
        );
        d.select_features = false;
        assert!(
            d.syntax(&data)
                .unwrap()
                .contains("/CROSSVALIDATION FOLDS=5")
        );
        d.kmax = "2".into();
        assert!(d.syntax(&data).unwrap_err().contains("below the largest"));
    }
}
