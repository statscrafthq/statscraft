//! The Factor Analysis, K-Means Cluster, Hierarchical Cluster, Discriminant, Binary Logistic
//! Regression, Univariate and Repeated Measures dialogs. Options left as they are add nothing to
//! the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::{DUNNETT_SIDES, POST_HOC_TESTS, keywords};
use crate::dialogs::whole;
use crate::stats_dialogs::{Block, parse, percent};
use crate::syntax_text::{names, number, probability};

/// Keyword and label of every table of the descriptives on /PRINT.
pub const FACTOR_DESCRIPTIVES: [(&str, &str); 9] = [
    ("UNIVARIATE", "Univariate descriptives"),
    ("INITIAL", "Initial solution"),
    ("CORRELATION", "Coefficients"),
    ("SIG", "Significance levels"),
    ("DET", "Determinant"),
    ("KMO", "KMO and Bartlett's test of sphericity"),
    ("INV", "Inverse"),
    ("REPR", "Reproduced"),
    ("AIC", "Anti-image"),
];

/// The initial solution, which FACTOR shows without /PRINT.
const FACTOR_DEFAULT_DESCRIPTIVES: [bool; 9] =
    [false, true, false, false, false, false, false, false, false];

pub const FACTOR_EXTRACTIONS: [(&str, &str); 7] = [
    ("PC", "Principal components"),
    ("ULS", "Unweighted least squares"),
    ("GLS", "Generalized least squares"),
    ("ML", "Maximum likelihood"),
    ("PAF", "Principal axis factoring"),
    ("ALPHA", "Alpha factoring"),
    ("IMAGE", "Image factoring"),
];

pub const FACTOR_ROTATIONS: [(&str, &str); 6] = [
    ("NOROTATE", "None"),
    ("VARIMAX", "Varimax"),
    ("OBLIMIN", "Direct oblimin"),
    ("QUARTIMAX", "Quartimax"),
    ("EQUAMAX", "Equamax"),
    ("PROMAX", "Promax"),
];

/// Varimax, the rotation of FACTOR without /EXTRACTION and /ROTATION.
const FACTOR_DEFAULT_ROTATION: usize = 1;

pub const FACTOR_SCORES: [(&str, &str); 3] = [
    ("REG", "Regression"),
    ("BART", "Bartlett"),
    ("AR", "Anderson-Rubin"),
];

pub const FACTOR_MISSING: [(&str, &str); 3] = [
    ("LISTWISE", "Leave out a case missing on any variable"),
    ("PAIRWISE", "Use the valid cases of each pair of variables"),
    ("MEANSUB", "Replace a missing value with the mean"),
];

/// FACTOR of the variables.
#[derive(Clone, PartialEq, Debug)]
pub struct FactorAnalysis {
    pub variables: Vec<String>,
    /// One flag per entry of `FACTOR_DESCRIPTIVES`.
    pub descriptives: [bool; 9],
    /// Index into `FACTOR_EXTRACTIONS`.
    pub extraction: usize,
    /// A fixed number of factors instead of those with an eigenvalue over `min_eigen`.
    pub fixed: bool,
    pub min_eigen: String,
    pub factors: String,
    pub iterations: String,
    /// Index into `FACTOR_ROTATIONS`.
    pub rotation: usize,
    pub delta: String,
    pub kappa: String,
    pub save: bool,
    /// Index into `FACTOR_SCORES`.
    pub score_method: usize,
    /// The factor score coefficient matrix.
    pub score_coefficients: bool,
    /// Index into `FACTOR_MISSING`.
    pub missing: usize,
    pub sort: bool,
    pub blank: bool,
    pub blank_value: String,
}

impl Default for FactorAnalysis {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            descriptives: FACTOR_DEFAULT_DESCRIPTIVES,
            extraction: 0,
            fixed: false,
            min_eigen: "1".into(),
            factors: String::new(),
            iterations: "25".into(),
            rotation: FACTOR_DEFAULT_ROTATION,
            delta: "0".into(),
            kappa: "4".into(),
            save: false,
            score_method: 0,
            score_coefficients: false,
            missing: 0,
            sort: false,
            blank: false,
            blank_value: "0.10".into(),
        }
    }
}

impl FactorAnalysis {
    pub fn syntax(&self) -> Result<String, String> {
        if self.variables.len() < 2 {
            return Err("add at least two variables".into());
        }
        let mut text = format!("FACTOR /VARIABLES={}", self.variables.join(" "));
        if self.missing > 0 {
            text.push_str(&format!("\n  /MISSING={}", FACTOR_MISSING[self.missing].0));
        }
        if self.descriptives != FACTOR_DEFAULT_DESCRIPTIVES || self.score_coefficients {
            let mut print = keywords(&FACTOR_DESCRIPTIVES, &self.descriptives);
            print.extend(["EXTRACTION", "ROTATION"]);
            print.extend(self.score_coefficients.then_some("FSCORE"));
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        if self.sort || self.blank {
            let mut format = Vec::new();
            if self.sort {
                format.push("SORT".to_string());
            }
            if self.blank {
                let value = parse(&self.blank_value, "the smallest value shown")?;
                if value < 0.0 {
                    return Err("the smallest value shown cannot be negative".into());
                }
                format.push(format!("BLANK({})", number(value)));
            }
            text.push_str(&format!("\n  /FORMAT={}", format.join(" ")));
        }
        let mut criteria = Vec::new();
        if self.fixed {
            match whole(&self.factors, 1, "the number of factors")? {
                Some(n) => criteria.push(format!("FACTORS({n})")),
                None => return Err("give the number of factors".into()),
            }
        } else {
            let min = parse(&self.min_eigen, "the smallest eigenvalue")?;
            if min < 0.0 {
                return Err("the smallest eigenvalue cannot be negative".into());
            }
            if min != 1.0 {
                criteria.push(format!("MINEIGEN({})", number(min)));
            }
        }
        match whole(&self.iterations, 1, "the maximum number of iterations")? {
            Some(n) if n != 25 => criteria.push(format!("ITERATE({n})")),
            _ => {}
        }
        // CRITERIA applies to the EXTRACTION and ROTATION after it
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.extraction > 0 {
            let method = FACTOR_EXTRACTIONS[self.extraction].0;
            text.push_str(&format!("\n  /EXTRACTION={method}"));
        }
        // after an explicit EXTRACTION the factors are rotated only when ROTATION says so
        if self.rotation != FACTOR_DEFAULT_ROTATION || self.extraction > 0 {
            let mut rotation = FACTOR_ROTATIONS[self.rotation].0.to_string();
            if rotation == "OBLIMIN" {
                let delta = parse(&self.delta, "delta")?;
                if delta > 0.8 {
                    return Err("delta must be at most 0.8".into());
                }
                if delta != 0.0 {
                    rotation.push_str(&format!("({})", number(delta)));
                }
            } else if rotation == "PROMAX" {
                let kappa = parse(&self.kappa, "kappa")?;
                if kappa <= 1.0 {
                    return Err("kappa must be above 1".into());
                }
                if kappa != 4.0 {
                    rotation.push_str(&format!("({})", number(kappa)));
                }
            }
            text.push_str(&format!("\n  /ROTATION={rotation}"));
        }
        // after ROTATION, which cancels a SAVE before it
        if self.save {
            let method = FACTOR_SCORES[self.score_method].0;
            text.push_str(&format!("\n  /SAVE={method}(ALL)"));
        }
        text.push('.');
        Ok(text)
    }
}

/// QUICK CLUSTER of the variables.
#[derive(Clone, PartialEq, Debug)]
pub struct KMeans {
    pub variables: Vec<String>,
    pub clusters: String,
    /// Classify the cases by the initial centers without updating them.
    pub classify_only: bool,
    pub iterations: String,
    pub converge: String,
    pub save_cluster: bool,
    pub save_distance: bool,
    pub initial: bool,
    pub anova: bool,
    /// The cluster of each case, labelled by `label` when given.
    pub membership: bool,
    pub label: String,
    pub pairwise: bool,
}

impl Default for KMeans {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            clusters: "2".into(),
            classify_only: false,
            iterations: "10".into(),
            converge: "0".into(),
            save_cluster: false,
            save_distance: false,
            initial: true,
            anova: false,
            membership: false,
            label: String::new(),
            pairwise: false,
        }
    }
}

impl KMeans {
    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("QUICK CLUSTER {}", names(&self.variables, "Variables")?);
        if self.pairwise {
            text.push_str("\n  /MISSING=PAIRWISE");
        }
        let mut criteria = Vec::new();
        match whole(&self.clusters, 2, "the number of clusters")? {
            Some(2) => {}
            Some(k) => criteria.push(format!("CLUSTER({k})")),
            None => return Err("give the number of clusters".into()),
        }
        if !self.classify_only {
            match whole(&self.iterations, 1, "the maximum number of iterations")? {
                Some(n) if n != 10 => criteria.push(format!("MXITER({n})")),
                _ => {}
            }
            let converge = parse(&self.converge, "the convergence criterion")?;
            if !(0.0..1.0).contains(&converge) {
                return Err("the convergence criterion must be from 0 up to 1".into());
            }
            if converge != 0.0 {
                criteria.push(format!("CONVERGE({})", number(converge)));
            }
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.classify_only {
            text.push_str("\n  /METHOD=CLASSIFY");
        }
        if (self.initial, self.anova, self.membership) != (true, false, false) {
            let mut print = Vec::new();
            print.extend(self.initial.then_some("INITIAL".to_string()));
            print.extend(self.anova.then_some("ANOVA".to_string()));
            if self.membership {
                print.push("CLUSTER".into());
                let label = self.label.trim();
                if !label.is_empty() {
                    print.push(format!("ID({label})"));
                }
            }
            if print.is_empty() {
                print.push("NONE".into());
            }
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let save: Vec<&str> = [
            (self.save_cluster, "CLUSTER"),
            (self.save_distance, "DISTANCE"),
        ]
        .into_iter()
        .filter_map(|(on, keyword)| on.then_some(keyword))
        .collect();
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

pub const CLUSTER_METHODS: [(&str, &str); 7] = [
    ("BAVERAGE", "Between-groups linkage"),
    ("WAVERAGE", "Within-groups linkage"),
    ("SINGLE", "Nearest neighbor"),
    ("COMPLETE", "Furthest neighbor"),
    ("CENTROID", "Centroid clustering"),
    ("MEDIAN", "Median clustering"),
    ("WARD", "Ward's method"),
];

/// The kinds of data a measure is for.
pub const CLUSTER_DATA: [&str; 3] = ["Interval", "Counts", "Binary"];

pub const CLUSTER_INTERVAL: [(&str, &str); 8] = [
    ("SEUCLID", "Squared Euclidean distance"),
    ("EUCLID", "Euclidean distance"),
    ("COSINE", "Cosine"),
    ("CORRELATION", "Pearson correlation"),
    ("CHEBYCHEV", "Chebychev"),
    ("BLOCK", "Block"),
    ("MINKOWSKI", "Minkowski"),
    ("POWER", "Customized"),
];

pub const CLUSTER_COUNTS: [(&str, &str); 2] = [
    ("CHISQ", "Chi-squared measure"),
    ("PH2", "Phi-squared measure"),
];

/// The binary measures offered; the syntax has more.
pub const CLUSTER_BINARY: [(&str, &str); 11] = [
    ("BSEUCLID", "Squared Euclidean distance"),
    ("BEUCLID", "Euclidean distance"),
    ("SM", "Simple matching"),
    ("JACCARD", "Jaccard"),
    ("DICE", "Dice"),
    ("RR", "Russell and Rao"),
    ("RT", "Rogers and Tanimoto"),
    ("HAMANN", "Hamann"),
    ("OCHIAI", "Ochiai"),
    ("PHI", "Phi 4-point correlation"),
    ("Q", "Yule's Q"),
];

/// The solutions of the cluster membership: none, one number of clusters, or a range.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Solutions {
    /// 0 none, 1 single, 2 range.
    pub kind: usize,
    pub clusters: String,
    pub min: String,
    pub max: String,
}

impl Solutions {
    /// `CLUSTER(n)` or `CLUSTER(min,max)`, None for no solution.
    fn keyword(&self) -> Result<Option<String>, String> {
        let need = |text: &str, what: &str| -> Result<u32, String> {
            whole(text, 2, what)?.ok_or_else(|| format!("give {what}"))
        };
        Ok(match self.kind {
            0 => None,
            1 => Some(format!(
                "CLUSTER({})",
                need(&self.clusters, "the number of clusters")?
            )),
            _ => {
                let min = need(&self.min, "the smallest number of clusters")?;
                let max = need(&self.max, "the largest number of clusters")?;
                if min >= max {
                    return Err("the smallest number of clusters must be below the largest".into());
                }
                Some(format!("CLUSTER({min},{max})"))
            }
        })
    }
}

/// CLUSTER of the cases by the variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Hierarchical {
    pub variables: Vec<String>,
    pub label: String,
    /// Index into `CLUSTER_METHODS`.
    pub method: usize,
    /// Index into `CLUSTER_DATA`.
    pub data: usize,
    /// Indexes into `CLUSTER_INTERVAL`, `CLUSTER_COUNTS` and `CLUSTER_BINARY`.
    pub interval: usize,
    pub counts: usize,
    pub binary: usize,
    pub power: String,
    pub root: String,
    pub present: String,
    pub absent: String,
    pub schedule: bool,
    pub proximity: bool,
    pub membership: Solutions,
    pub save: Solutions,
}

impl Default for Hierarchical {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            label: String::new(),
            method: 0,
            data: 0,
            interval: 0,
            counts: 0,
            binary: 0,
            power: "2".into(),
            root: "2".into(),
            present: "1".into(),
            absent: "0".into(),
            schedule: true,
            proximity: false,
            membership: Solutions::default(),
            save: Solutions::default(),
        }
    }
}

impl Hierarchical {
    fn measure(&self) -> Result<String, String> {
        let positive = |text: &str, what: &str| -> Result<f64, String> {
            let n = parse(text, what)?;
            if n <= 0.0 {
                return Err(format!("{what} must be above 0"));
            }
            Ok(n)
        };
        Ok(match self.data {
            0 => {
                let keyword = CLUSTER_INTERVAL[self.interval].0;
                match keyword {
                    "MINKOWSKI" => {
                        format!("MINKOWSKI({})", number(positive(&self.power, "the power")?))
                    }
                    "POWER" => format!(
                        "POWER({},{})",
                        number(positive(&self.power, "the power")?),
                        number(positive(&self.root, "the root")?)
                    ),
                    _ => keyword.into(),
                }
            }
            1 => CLUSTER_COUNTS[self.counts].0.into(),
            _ => {
                let present = parse(&self.present, "the value of presence")?;
                let absent = parse(&self.absent, "the value of absence")?;
                if present == absent {
                    return Err("the values of presence and absence must differ".into());
                }
                let keyword = CLUSTER_BINARY[self.binary].0;
                if (present, absent) == (1.0, 0.0) {
                    keyword.into()
                } else {
                    format!("{keyword}({},{})", number(present), number(absent))
                }
            }
        })
    }

    pub fn syntax(&self) -> Result<String, String> {
        let mut text = format!("CLUSTER {}", names(&self.variables, "Variables")?);
        let measure = self.measure()?;
        if measure != "SEUCLID" {
            text.push_str(&format!("\n  /MEASURE={measure}"));
        }
        if self.method > 0 {
            text.push_str(&format!("\n  /METHOD={}", CLUSTER_METHODS[self.method].0));
        }
        let label = self.label.trim();
        if !label.is_empty() {
            text.push_str(&format!("\n  /ID={label}"));
        }
        let membership = self.membership.keyword()?;
        if !self.schedule || self.proximity || membership.is_some() {
            let mut print: Vec<String> = Vec::new();
            print.extend(self.schedule.then(|| "SCHEDULE".into()));
            print.extend(membership);
            print.extend(self.proximity.then(|| "DISTANCE".into()));
            if print.is_empty() {
                print.push("NONE".into());
            }
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        if let Some(save) = self.save.keyword()? {
            text.push_str(&format!("\n  /SAVE={save}"));
        }
        text.push('.');
        Ok(text)
    }
}

/// The stepwise methods of DISCRIMINANT.
pub const DISCRIMINANT_METHODS: [(&str, &str); 5] = [
    ("WILKS", "Wilks' lambda"),
    ("MAHAL", "Mahalanobis distance"),
    ("MAXMINF", "Smallest F ratio"),
    ("MINRESID", "Unexplained variance"),
    ("RAO", "Rao's V"),
];

/// Keyword and label of every table on /STATISTICS but the classification results.
pub const DISCRIMINANT_STATISTICS: [(&str, &str); 9] = [
    ("MEAN STDDEV", "Means"),
    ("UNIVF", "Univariate ANOVAs"),
    ("BOXM", "Box's M"),
    ("COEFF", "Fisher's"),
    ("RAW", "Unstandardized"),
    ("CORR", "Within-groups correlation"),
    ("COV", "Within-groups covariance"),
    ("GCOV", "Separate-groups covariance"),
    ("TCOV", "Total covariance"),
];

/// The groups of `DISCRIMINANT_STATISTICS`: title and the range of its entries.
pub const DISCRIMINANT_STATISTICS_GROUPS: [(&str, std::ops::Range<usize>); 3] = [
    ("Descriptives", 0..3),
    ("Function coefficients", 3..5),
    ("Matrices", 5..9),
];

pub const DISCRIMINANT_SAVE: [(&str, &str); 3] = [
    ("CLASS", "Predicted group membership"),
    ("SCORES", "Discriminant scores"),
    ("PROBS", "Probabilities of group membership"),
];

/// DISCRIMINANT of a grouping variable by independent variables.
#[derive(Clone, PartialEq, Debug)]
pub struct Discriminant {
    pub group: String,
    pub min: String,
    pub max: String,
    pub variables: Vec<String>,
    pub stepwise: bool,
    /// Index into `DISCRIMINANT_METHODS`.
    pub method: usize,
    /// The stepping criteria are values of F instead of their probabilities.
    pub use_f: bool,
    pub entry: String,
    pub removal: String,
    /// One flag per entry of `DISCRIMINANT_STATISTICS`.
    pub statistics: [bool; 9],
    /// Prior probabilities from the group sizes instead of equal.
    pub priors_size: bool,
    /// Classify by the separate-groups covariance matrices instead of the within-groups one.
    pub separate: bool,
    pub table: bool,
    pub leave_one_out: bool,
    /// One flag per entry of `DISCRIMINANT_SAVE`.
    pub save: [bool; 3],
    pub select: String,
    pub select_value: String,
}

impl Default for Discriminant {
    fn default() -> Self {
        Self {
            group: String::new(),
            min: String::new(),
            max: String::new(),
            variables: Vec::new(),
            stepwise: false,
            method: 0,
            use_f: true,
            entry: "3.84".into(),
            removal: "2.71".into(),
            statistics: [false; 9],
            priors_size: false,
            separate: false,
            table: false,
            leave_one_out: false,
            save: [false; 3],
            select: String::new(),
            select_value: String::new(),
        }
    }
}

impl Discriminant {
    /// Switches the stepping criteria between values of F and their probabilities, at their
    /// defaults.
    pub fn set_use_f(&mut self, use_f: bool) {
        self.use_f = use_f;
        let (entry, removal) = if use_f {
            ("3.84", "2.71")
        } else {
            ("0.05", "0.10")
        };
        self.entry = entry.into();
        self.removal = removal.into();
    }

    /// The variables that must be numeric.
    pub fn numeric(&self) -> impl Iterator<Item = &String> {
        [&self.group, &self.select]
            .into_iter()
            .filter(|n| !n.is_empty())
            .chain(&self.variables)
    }

    pub fn syntax(&self) -> Result<String, String> {
        let group = self.group.trim();
        if group.is_empty() {
            return Err("choose the grouping variable".into());
        }
        let bound = |text: &str, what: &str| -> Result<f64, String> {
            let n = parse(text, what)?;
            if n.fract() != 0.0 {
                return Err(format!("{what} must be a whole number"));
            }
            Ok(n)
        };
        let min = bound(&self.min, "the smallest group value")?;
        let max = bound(&self.max, "the largest group value")?;
        if min >= max {
            return Err("the smallest group value must be below the largest".into());
        }
        let variables = names(&self.variables, "Independents")?;
        if self.variables.iter().any(|n| n == group) {
            return Err(format!("{group} is the grouping variable"));
        }
        let mut text = format!(
            "DISCRIMINANT GROUPS={group}({},{})\n  /VARIABLES={variables}",
            number(min),
            number(max)
        );
        let select = self.select.trim();
        if !select.is_empty() {
            let value = parse(&self.select_value, "the value of the selection variable")?;
            text.push_str(&format!("\n  /SELECT={select}({})", number(value)));
        }
        // the analysis block, before the global subcommands
        if self.stepwise {
            text.push_str(&format!(
                "\n  /METHOD={}",
                DISCRIMINANT_METHODS[self.method].0
            ));
            let entry = parse(&self.entry, "the entry value")?;
            let removal = parse(&self.removal, "the removal value")?;
            if self.use_f {
                if !(removal >= 0.0 && entry > removal) {
                    return Err(
                        "the F to enter must be above the F to remove, which is 0 or more".into(),
                    );
                }
                if (entry, removal) != (3.84, 2.71) {
                    text.push_str(&format!(
                        "\n  /FIN={}\n  /FOUT={}",
                        number(entry),
                        number(removal)
                    ));
                }
            } else {
                if !(entry > 0.0 && entry < removal && removal <= 1.0) {
                    return Err(
                        "the probability to enter must be above 0 and below the probability to remove, which is at most 1"
                            .into(),
                    );
                }
                // PIN without POUT would remove by FOUT
                text.push_str(&format!(
                    "\n  /PIN={}\n  /POUT={}",
                    number(entry),
                    number(removal)
                ));
            }
        }
        if self.priors_size {
            text.push_str("\n  /PRIORS=SIZE");
        }
        let save = keywords(&DISCRIMINANT_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        if self.separate {
            text.push_str("\n  /CLASSIFY=SEPARATE");
        }
        let mut statistics = keywords(&DISCRIMINANT_STATISTICS, &self.statistics);
        statistics.extend(self.table.then_some("TABLE"));
        statistics.extend(self.leave_one_out.then_some("CROSSVALID"));
        if !statistics.is_empty() {
            text.push_str(&format!("\n  /STATISTICS={}", statistics.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// Keyword and label of every method of a block of covariates.
pub const LOGISTIC_METHODS: [(&str, &str); 7] = [
    ("ENTER", "Enter"),
    ("FSTEP(COND)", "Forward: Conditional"),
    ("FSTEP(LR)", "Forward: LR"),
    ("FSTEP(WALD)", "Forward: Wald"),
    ("BSTEP(COND)", "Backward: Conditional"),
    ("BSTEP(LR)", "Backward: LR"),
    ("BSTEP(WALD)", "Backward: Wald"),
];

/// Keyword and label of every contrast of a categorical covariate.
pub const LOGISTIC_CONTRASTS: [(&str, &str); 7] = [
    ("INDICATOR", "Indicator"),
    ("SIMPLE", "Simple"),
    ("DIFFERENCE", "Difference"),
    ("HELMERT", "Helmert"),
    ("REPEATED", "Repeated"),
    ("POLYNOMIAL", "Polynomial"),
    ("DEVIATION", "Deviation"),
];

/// The contrasts measured against a reference category.
pub fn has_reference(contrast: usize) -> bool {
    matches!(
        LOGISTIC_CONTRASTS[contrast].0,
        "INDICATOR" | "SIMPLE" | "DEVIATION"
    )
}

/// Keyword and label of every variable LOGISTIC REGRESSION saves.
pub const LOGISTIC_SAVE: [(&str, &str); 10] = [
    ("PRED", "Probabilities"),
    ("PGROUP", "Group membership"),
    ("RESID", "Unstandardized"),
    ("LRESID", "Logit"),
    ("SRESID", "Studentized"),
    ("ZRESID", "Standardized"),
    ("DEV", "Deviance"),
    ("COOK", "Cook's"),
    ("LEVER", "Leverage values"),
    ("DFBETA", "DfBeta"),
];

/// The groups of `LOGISTIC_SAVE`: title and the range of its entries.
pub const LOGISTIC_SAVE_GROUPS: [(&str, std::ops::Range<usize>); 3] = [
    ("Predicted values", 0..2),
    ("Residuals", 2..7),
    ("Influence", 7..10),
];

/// A categorical covariate: its name, its contrast and whether the reference is the first
/// category instead of the last.
#[derive(Clone, PartialEq, Debug)]
pub struct Categorical {
    pub name: String,
    /// Index into `LOGISTIC_CONTRASTS`.
    pub contrast: usize,
    pub first: bool,
}

/// LOGISTIC REGRESSION: one dependent variable and blocks of covariates.
#[derive(Clone, PartialEq, Debug)]
pub struct Logistic {
    pub dependent: String,
    /// Each block's method is an index into `LOGISTIC_METHODS`.
    pub blocks: Vec<Block>,
    /// The block the dialog shows.
    pub block: usize,
    pub categorical: Vec<Categorical>,
    /// One flag per entry of `LOGISTIC_SAVE`.
    pub save: [bool; 10],
    pub goodfit: bool,
    pub ci: bool,
    pub ci_level: String,
    pub iteration_history: bool,
    pub cut: String,
    pub entry: String,
    pub removal: String,
    pub iterations: String,
    pub constant: bool,
}

impl Default for Logistic {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            blocks: vec![Block::default()],
            block: 0,
            categorical: Vec::new(),
            save: [false; 10],
            goodfit: false,
            ci: false,
            ci_level: "95".into(),
            iteration_history: false,
            cut: "0.5".into(),
            entry: "0.05".into(),
            removal: "0.10".into(),
            iterations: "20".into(),
            constant: true,
        }
    }
}

impl Logistic {
    pub fn syntax(&self) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        let blocks: Vec<&Block> = self
            .blocks
            .iter()
            .filter(|b| !b.variables.is_empty())
            .collect();
        if blocks.is_empty() {
            return Err("add at least one variable to Covariates".into());
        }
        let covariates: Vec<&String> = blocks.iter().flat_map(|b| &b.variables).collect();
        if covariates.contains(&&dependent.to_string()) {
            return Err(format!("{dependent} is the dependent variable"));
        }
        if let Some(c) = self
            .categorical
            .iter()
            .find(|c| !covariates.contains(&&c.name))
        {
            return Err(format!("{} is categorical but in no block", c.name));
        }
        let mut text = format!("LOGISTIC REGRESSION VARIABLES={dependent}");
        for b in blocks {
            let method = LOGISTIC_METHODS[b.method].0;
            text.push_str(&format!("\n  /METHOD={method} {}", b.variables.join(" ")));
        }
        if !self.categorical.is_empty() {
            let list: Vec<&str> = self.categorical.iter().map(|c| c.name.as_str()).collect();
            text.push_str(&format!("\n  /CATEGORICAL={}", list.join(" ")));
        }
        for c in &self.categorical {
            let reference = has_reference(c.contrast) && c.first;
            if c.contrast > 0 || reference {
                let keyword = LOGISTIC_CONTRASTS[c.contrast].0;
                let first = if reference { "(1)" } else { "" };
                text.push_str(&format!("\n  /CONTRAST({})={keyword}{first}", c.name));
            }
        }
        let save = keywords(&LOGISTIC_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        let mut print = Vec::new();
        if self.goodfit {
            print.push("GOODFIT".to_string());
        }
        if self.iteration_history {
            print.push("ITER(1)".into());
        }
        if self.ci {
            let level = percent(&self.ci_level)?;
            if level.fract() != 0.0 {
                return Err("the confidence level must be a whole number from 1 to 99".into());
            }
            print.push(format!("CI({})", number(level)));
        }
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        let mut criteria = Vec::new();
        let entry = parse(&self.entry, "the entry probability")?;
        let removal = parse(&self.removal, "the removal probability")?;
        if !(entry > 0.0 && entry < removal && removal <= 1.0) {
            return Err(
                "the probability to enter must be above 0 and below the probability to remove, which is at most 1"
                    .into(),
            );
        }
        if entry != 0.05 {
            criteria.push(format!("PIN({})", number(entry)));
        }
        if removal != 0.10 {
            criteria.push(format!("POUT({})", number(removal)));
        }
        match whole(&self.iterations, 1, "the maximum number of iterations")? {
            Some(n) if n != 20 => criteria.push(format!("ITERATE({n})")),
            _ => {}
        }
        let cut = parse(&self.cut, "the classification cutoff")?;
        if !(cut > 0.0 && cut < 1.0) {
            return Err("the classification cutoff must lie between 0 and 1".into());
        }
        if cut != 0.5 {
            criteria.push(format!("CUT({})", number(cut)));
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if !self.constant {
            text.push_str("\n  /ORIGIN");
        }
        text.push('.');
        Ok(text)
    }
}

/// The types of sums of squares, keyword and label.
pub const SUM_OF_SQUARES: [(&str, &str); 4] = [
    ("1", "Type I"),
    ("2", "Type II"),
    ("3", "Type III"),
    ("4", "Type IV"),
];

/// Type III, the sums of squares without /METHOD.
const DEFAULT_SUM_OF_SQUARES: usize = 2;

/// Keyword and label of every table of the options on /PRINT: the first three are offered
/// with within-subjects factors too.
pub const GLM_PRINT: [(&str, &str); 5] = [
    ("DESCRIPTIVE", "Descriptive statistics"),
    ("ETASQ", "Estimates of effect size"),
    ("OPOWER", "Observed power"),
    ("PARAMETER", "Parameter estimates"),
    ("HOMOGENEITY", "Homogeneity tests"),
];

pub const GLM_ADJUSTMENTS: [(&str, &str); 3] = [
    ("LSD", "LSD (none)"),
    ("BONFERRONI", "Bonferroni"),
    ("SIDAK", "Sidak"),
];

/// Keyword and label of every variable UNIANOVA saves.
pub const GLM_SAVE: [(&str, &str); 8] = [
    ("PRED", "Unstandardized"),
    ("SEPRED", "Standard error"),
    ("RESID", "Unstandardized"),
    ("ZRESID", "Standardized"),
    ("SRESID", "Studentized"),
    ("DRESID", "Deleted"),
    ("COOK", "Cook's distance"),
    ("LEVER", "Leverage values"),
];

/// The groups of `GLM_SAVE`: title and the range of its entries.
pub const GLM_SAVE_GROUPS: [(&str, std::ops::Range<usize>); 3] = [
    ("Predicted values", 0..2),
    ("Residuals", 2..6),
    ("Diagnostics", 6..8),
];

/// `/METHOD`, `/PRINT` and `/CRITERIA` of UNIANOVA and GLM.
fn glm_options(
    text: &mut String,
    sstype: usize,
    print: Vec<&str>,
    alpha: &str,
) -> Result<(), String> {
    if sstype != DEFAULT_SUM_OF_SQUARES {
        let kind = SUM_OF_SQUARES[sstype].0;
        text.push_str(&format!("\n  /METHOD=SSTYPE({kind})"));
    }
    if !print.is_empty() {
        text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
    }
    let alpha = probability(alpha)?;
    if alpha != "0.05" {
        text.push_str(&format!("\n  /CRITERIA=ALPHA({alpha})"));
    }
    Ok(())
}

/// `dependents BY factors WITH covariates`.
fn model_variables(dependents: &str, factors: &[String], covariates: &[String]) -> String {
    let mut text = dependents.to_string();
    if !factors.is_empty() {
        text.push_str(&format!(" BY {}", factors.join(" ")));
    }
    if !covariates.is_empty() {
        text.push_str(&format!(" WITH {}", covariates.join(" ")));
    }
    text
}

/// UNIANOVA of one dependent variable.
#[derive(Clone, PartialEq, Debug)]
pub struct Univariate {
    pub dependent: String,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// The terms of `terms` instead of the full factorial model.
    pub custom: bool,
    /// Each term is a factor or covariate, or an interaction of several.
    pub terms: Vec<Vec<String>>,
    /// The factors and covariates checked to make the next terms of.
    pub building: Vec<String>,
    /// Index into `SUM_OF_SQUARES`.
    pub sstype: usize,
    pub intercept: bool,
    /// The factors compared by the post hoc tests.
    pub post_hoc_factors: Vec<String>,
    /// One flag per entry of `POST_HOC_TESTS`.
    pub post_hoc: [bool; 16],
    pub dunnett: bool,
    pub dunnett_first: bool,
    /// Index into `DUNNETT_SIDES`.
    pub dunnett_side: usize,
    pub means_overall: bool,
    /// The factors whose estimated marginal means are shown.
    pub means_factors: Vec<String>,
    pub compare: bool,
    /// Index into `GLM_ADJUSTMENTS`.
    pub adjustment: usize,
    /// One flag per entry of `GLM_SAVE`.
    pub save: [bool; 8],
    /// One flag per entry of `GLM_PRINT`.
    pub print: [bool; 5],
    pub alpha: String,
}

impl Default for Univariate {
    fn default() -> Self {
        Self {
            dependent: String::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            custom: false,
            terms: Vec::new(),
            building: Vec::new(),
            sstype: DEFAULT_SUM_OF_SQUARES,
            intercept: true,
            post_hoc_factors: Vec::new(),
            post_hoc: [false; 16],
            dunnett: false,
            dunnett_first: false,
            dunnett_side: 0,
            means_overall: false,
            means_factors: Vec::new(),
            compare: false,
            adjustment: 0,
            save: [false; 8],
            print: [false; 5],
            alpha: "0.05".into(),
        }
    }
}

impl Univariate {
    /// The factors and covariates, the variables the terms of a custom model are made of.
    pub fn model(&self) -> Vec<String> {
        self.factors
            .iter()
            .chain(&self.covariates)
            .cloned()
            .collect()
    }

    /// The checked variables each as a term of its own (`interaction` false) or together as one.
    pub fn add_terms(&mut self, interaction: bool) {
        let model = self.model();
        let checked: Vec<String> = model
            .into_iter()
            .filter(|n| self.building.contains(n))
            .collect();
        let new: Vec<Vec<String>> = if interaction {
            vec![checked]
        } else {
            checked.into_iter().map(|n| vec![n]).collect()
        };
        for term in new {
            if !self.terms.contains(&term) {
                self.terms.push(term);
            }
        }
        self.building.clear();
    }

    pub fn syntax(&self) -> Result<String, String> {
        let dependent = self.dependent.trim();
        if dependent.is_empty() {
            return Err("choose the dependent variable".into());
        }
        if self.factors.is_empty() && self.covariates.is_empty() {
            return Err("add a fixed factor or a covariate".into());
        }
        let model = self.model();
        if model.iter().any(|n| n == dependent) {
            return Err(format!("{dependent} is the dependent variable"));
        }
        if let Some(n) = self.factors.iter().find(|n| self.covariates.contains(n)) {
            return Err(format!("{n} is both a factor and a covariate"));
        }
        let mut text = format!(
            "UNIANOVA {}",
            model_variables(dependent, &self.factors, &self.covariates)
        );
        if !self.intercept {
            text.push_str("\n  /INTERCEPT=EXCLUDE");
        }
        let mut tests: Vec<String> = keywords(&POST_HOC_TESTS, &self.post_hoc)
            .into_iter()
            .map(str::to_string)
            .collect();
        if self.dunnett {
            let keyword = DUNNETT_SIDES[self.dunnett_side].0;
            // the control is the last category unless its position is given
            let control = if self.dunnett_first { "(1)" } else { "" };
            tests.push(format!("{keyword}{control}"));
        }
        let post_hoc: Vec<&str> = self
            .factors
            .iter()
            .filter(|f| self.post_hoc_factors.contains(f))
            .map(String::as_str)
            .collect();
        match (post_hoc.is_empty(), tests.is_empty()) {
            (true, true) => {}
            (true, false) => return Err("choose the factors of the post hoc tests".into()),
            (false, true) => return Err("choose at least one post hoc test".into()),
            (false, false) => text.push_str(&format!(
                "\n  /POSTHOC={}({})",
                post_hoc.join(" "),
                tests.join(" ")
            )),
        }
        if self.means_overall {
            text.push_str("\n  /EMMEANS=TABLES(OVERALL)");
        }
        for f in self
            .factors
            .iter()
            .filter(|f| self.means_factors.contains(f))
        {
            text.push_str(&format!("\n  /EMMEANS=TABLES({f})"));
            if self.compare {
                text.push_str(" COMPARE");
                if self.adjustment > 0 {
                    text.push_str(&format!(" ADJ({})", GLM_ADJUSTMENTS[self.adjustment].0));
                }
            }
        }
        let save = keywords(&GLM_SAVE, &self.save);
        if !save.is_empty() {
            text.push_str(&format!("\n  /SAVE={}", save.join(" ")));
        }
        glm_options(
            &mut text,
            self.sstype,
            keywords(&GLM_PRINT, &self.print),
            &self.alpha,
        )?;
        if self.custom {
            if self.terms.is_empty() {
                return Err("add at least one term to the model".into());
            }
            let mut terms = Vec::new();
            for term in &self.terms {
                if let Some(n) = term.iter().find(|n| !model.contains(n)) {
                    return Err(format!(
                        "{n} in the model is no longer a factor or covariate"
                    ));
                }
                terms.push(term.join("*"));
            }
            text.push_str(&format!("\n  /DESIGN={}", terms.join(" ")));
        }
        text.push('.');
        Ok(text)
    }
}

/// A within-subjects factor: its name and number of levels as typed.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct WithinFactor {
    pub name: String,
    pub levels: String,
}

/// GLM with within-subjects factors.
#[derive(Clone, PartialEq, Debug)]
pub struct RepeatedMeasures {
    pub within: Vec<WithinFactor>,
    /// One variable for each combination of levels, the first factor's levels changing slowest.
    pub dependents: Vec<String>,
    pub factors: Vec<String>,
    pub covariates: Vec<String>,
    /// Index into `SUM_OF_SQUARES`.
    pub sstype: usize,
    /// One flag per entry of the first three of `GLM_PRINT`.
    pub print: [bool; 3],
    pub alpha: String,
}

impl Default for RepeatedMeasures {
    fn default() -> Self {
        Self {
            within: vec![WithinFactor {
                name: "factor1".into(),
                levels: String::new(),
            }],
            dependents: Vec::new(),
            factors: Vec::new(),
            covariates: Vec::new(),
            sstype: DEFAULT_SUM_OF_SQUARES,
            print: [false; 3],
            alpha: "0.05".into(),
        }
    }
}

impl RepeatedMeasures {
    pub fn syntax(&self) -> Result<String, String> {
        let mut factors = Vec::new();
        let mut combinations = 1;
        for w in &self.within {
            let name = w.name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err("a within-subjects factor needs a name of one word".into());
            }
            if factors
                .iter()
                .any(|(f, _): &(&str, u32)| f.eq_ignore_ascii_case(name))
            {
                return Err(format!("{name} is the name of two within-subjects factors"));
            }
            let what = format!("the number of levels of {name}");
            let Some(levels) = whole(&w.levels, 2, &what)? else {
                return Err(format!("give the number of levels of {name}"));
            };
            combinations *= levels as usize;
            factors.push((name, levels));
        }
        if factors.is_empty() {
            return Err("add a within-subjects factor".into());
        }
        if self.dependents.len() != combinations {
            return Err(format!(
                "Within-subjects variables needs {combinations} variables, one for each combination of levels"
            ));
        }
        let between: Vec<&String> = self.factors.iter().chain(&self.covariates).collect();
        if let Some(n) = self.dependents.iter().find(|n| between.contains(n)) {
            return Err(format!("{n} is a within-subjects variable"));
        }
        if let Some(n) = self.factors.iter().find(|n| self.covariates.contains(n)) {
            return Err(format!("{n} is both a factor and a covariate"));
        }
        let mut text = format!(
            "GLM {}",
            model_variables(&self.dependents.join(" "), &self.factors, &self.covariates)
        );
        let within: Vec<String> = factors.iter().map(|(n, l)| format!("{n} {l}")).collect();
        text.push_str(&format!("\n  /WSFACTOR={}", within.join(" ")));
        glm_options(
            &mut text,
            self.sstype,
            keywords(&GLM_PRINT[..3], &self.print),
            &self.alpha,
        )?;
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{open, run_in, runs, v};

    const ITEMS: [&str; 5] = [
        "q1_satisfaction",
        "q2_recommend",
        "age",
        "monthly_spend",
        "q5_1",
    ];

    #[test]
    fn factor_analysis() {
        let mut d = FactorAnalysis {
            variables: v(&["age"]),
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "add at least two variables");
        d.variables = v(&ITEMS);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "FACTOR /VARIABLES=q1_satisfaction q2_recommend age monthly_spend q5_1."
        );
        assert!(runs(&syntax) >= 3);

        // every descriptive, a fixed number of factors, principal axis, oblimin, scores
        d.descriptives = [true; 9];
        d.score_coefficients = true;
        d.missing = 1;
        d.sort = true;
        d.blank = true;
        d.fixed = true;
        d.factors = "2".into();
        d.iterations = "50".into();
        d.extraction = 4;
        d.rotation = 2;
        d.delta = "-0.5".into();
        d.save = true;
        d.score_method = 2;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "FACTOR /VARIABLES=q1_satisfaction q2_recommend age monthly_spend q5_1\n  \
             /MISSING=PAIRWISE\n  \
             /PRINT=UNIVARIATE INITIAL CORRELATION SIG DET KMO INV REPR AIC EXTRACTION ROTATION \
             FSCORE\n  /FORMAT=SORT BLANK(0.1)\n  /CRITERIA=FACTORS(2) ITERATE(50)\n  \
             /EXTRACTION=PAF\n  /ROTATION=OBLIMIN(-0.5)\n  /SAVE=AR(ALL)."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 10);
        let data = session.dataset().unwrap();
        assert!(data.variable("FAC1_1").is_ok());
        assert!(data.variable("FAC2_1").is_ok());

        // an eigenvalue limit, maximum likelihood without rotation, promax after the default
        d.descriptives = FACTOR_DEFAULT_DESCRIPTIVES;
        d.score_coefficients = false;
        d.missing = 0;
        d.sort = false;
        d.blank = false;
        d.fixed = false;
        d.min_eigen = "0.5".into();
        d.iterations = "25".into();
        d.extraction = 3;
        d.rotation = 0;
        d.save = false;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "FACTOR /VARIABLES=q1_satisfaction q2_recommend age monthly_spend q5_1\n  \
             /CRITERIA=MINEIGEN(0.5)\n  /EXTRACTION=ML\n  /ROTATION=NOROTATE."
        );
        assert!(runs(&syntax) >= 3);
        d.min_eigen = "1".into();
        d.extraction = 0;
        d.rotation = 5;
        d.kappa = "3".into();
        let syntax = d.syntax().unwrap();
        assert!(syntax.ends_with("q5_1\n  /ROTATION=PROMAX(3)."));
        assert!(runs(&syntax) >= 4);

        d.kappa = "1".into();
        assert_eq!(d.syntax().unwrap_err(), "kappa must be above 1");
        d.rotation = 2;
        d.delta = "0.9".into();
        assert_eq!(d.syntax().unwrap_err(), "delta must be at most 0.8");
        d.fixed = true;
        d.factors.clear();
        assert_eq!(d.syntax().unwrap_err(), "give the number of factors");
    }

    #[test]
    fn k_means_cluster() {
        let mut d = KMeans::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["age", "monthly_spend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "QUICK CLUSTER age monthly_spend.");
        assert!(runs(&syntax) >= 3);

        d.clusters = "3".into();
        d.iterations = "20".into();
        d.converge = "0.02".into();
        d.anova = true;
        d.membership = true;
        d.label = "resp_id".into();
        d.save_cluster = true;
        d.save_distance = true;
        d.pairwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "QUICK CLUSTER age monthly_spend\n  /MISSING=PAIRWISE\n  \
             /CRITERIA=CLUSTER(3) MXITER(20) CONVERGE(0.02)\n  \
             /PRINT=INITIAL ANOVA CLUSTER ID(resp_id)\n  /SAVE=CLUSTER DISTANCE."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 5);
        let data = session.dataset().unwrap();
        assert!(data.variable("QCL_1").is_ok());
        assert!(data.variable("QCL_2").is_ok());

        // classify only: no iterations; no optional table at all
        d.classify_only = true;
        d.initial = false;
        d.anova = false;
        d.membership = false;
        d.save_cluster = false;
        d.save_distance = false;
        d.pairwise = false;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "QUICK CLUSTER age monthly_spend\n  /CRITERIA=CLUSTER(3)\n  /METHOD=CLASSIFY\n  \
             /PRINT=NONE."
        );
        assert!(runs(&syntax) >= 1);
        d.clusters = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the number of clusters must be a whole number of at least 2"
        );
    }

    #[test]
    fn hierarchical_cluster() {
        let mut d = Hierarchical::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["age", "monthly_spend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "CLUSTER age monthly_spend.");
        assert!(runs(&syntax) >= 2);

        // Ward with a customized measure, a label, every table, a range saved
        d.label = "resp_id".into();
        d.method = 6;
        d.interval = 7;
        d.power = "3".into();
        d.proximity = true;
        d.membership = Solutions {
            kind: 1,
            clusters: "3".into(),
            ..Default::default()
        };
        d.save = Solutions {
            kind: 2,
            min: "2".into(),
            max: "4".into(),
            ..Default::default()
        };
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CLUSTER age monthly_spend\n  /MEASURE=POWER(3,2)\n  /METHOD=WARD\n  /ID=resp_id\n  \
             /PRINT=SCHEDULE CLUSTER(3) DISTANCE\n  /SAVE=CLUSTER(2,4)."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 4);
        let data = session.dataset().unwrap();
        for name in ["CLU2_1", "CLU3_1", "CLU4_1"] {
            assert!(data.variable(name).is_ok(), "{name}");
        }

        // Minkowski, then counts, then a binary measure with its own values; no table at all
        d.interval = 6;
        d.membership.kind = 0;
        d.save.kind = 0;
        d.schedule = false;
        d.proximity = false;
        d.label.clear();
        d.method = 2;
        assert_eq!(
            d.syntax().unwrap(),
            "CLUSTER age monthly_spend\n  /MEASURE=MINKOWSKI(3)\n  /METHOD=SINGLE\n  /PRINT=NONE."
        );
        d.data = 1;
        d.counts = 1;
        let syntax = d.syntax().unwrap();
        assert!(syntax.contains("/MEASURE=PH2\n"));
        assert!(runs(&syntax) >= 1);
        d.variables = v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]);
        d.data = 2;
        d.binary = 2;
        d.method = 3;
        d.schedule = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "CLUSTER q5_1 q5_2 q5_3 q5_4 q5_5\n  /MEASURE=SM\n  /METHOD=COMPLETE."
        );
        assert!(runs(&syntax) >= 2);
        d.present = "0".into();
        d.absent = "1".into();
        let syntax = d.syntax().unwrap();
        assert!(syntax.contains("/MEASURE=SM(0,1)\n"));
        assert!(runs(&syntax) >= 2);

        d.absent = "0".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the values of presence and absence must differ"
        );
        d.data = 0;
        d.interval = 6;
        d.power = "0".into();
        assert_eq!(d.syntax().unwrap_err(), "the power must be above 0");
        d.interval = 0;
        d.save = Solutions {
            kind: 2,
            min: "4".into(),
            max: "3".into(),
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "the smallest number of clusters must be below the largest"
        );
        d.save = Solutions {
            kind: 1,
            ..Default::default()
        };
        assert_eq!(d.syntax().unwrap_err(), "give the number of clusters");
    }

    #[test]
    fn discriminant() {
        let mut d = Discriminant::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the grouping variable");
        d.group = "gender".into();
        d.min = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the largest group value must be a number"
        );
        d.max = "2".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Independents"
        );
        d.variables = v(&["age", "monthly_spend", "q1_satisfaction"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "DISCRIMINANT GROUPS=gender(1,2)\n  /VARIABLES=age monthly_spend q1_satisfaction."
        );
        assert!(runs(&syntax) >= 3);

        // stepwise by Mahalanobis with probabilities, every table, sizes as priors, saved
        // variables and a selection variable
        d.group = "region".into();
        d.max = "5".into();
        d.stepwise = true;
        d.method = 1;
        d.set_use_f(false);
        // lenient, since no variable of the survey tells the regions apart
        d.entry = "0.5".into();
        d.removal = "0.6".into();
        d.statistics = [true; 9];
        d.priors_size = true;
        d.separate = true;
        d.table = true;
        d.leave_one_out = true;
        d.save = [true; 3];
        d.select = "gender".into();
        d.select_value = "1".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "DISCRIMINANT GROUPS=region(1,5)\n  /VARIABLES=age monthly_spend q1_satisfaction\n  \
             /SELECT=gender(1)\n  /METHOD=MAHAL\n  /PIN=0.5\n  /POUT=0.6\n  /PRIORS=SIZE\n  \
             /SAVE=CLASS SCORES PROBS\n  /CLASSIFY=SEPARATE\n  \
             /STATISTICS=MEAN STDDEV UNIVF BOXM COEFF RAW CORR COV GCOV TCOV TABLE CROSSVALID."
        );
        let mut session = open("survey_basic.sav");
        let result = session.run(&syntax, None).unwrap();
        let titles: Vec<&str> = result.commands[0]
            .tables
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        for title in [
            "Covariance Matrices",
            "Test Results",
            "Canonical Discriminant Function Coefficients",
            "Classification Function Coefficients",
            "Classification Results",
        ] {
            assert!(titles.contains(&title), "{title} in {titles:?}");
        }
        let data = session.dataset().unwrap();
        assert!(data.variable("DSC_1").is_ok());

        // Rao's V with values of F; the entered-together method leaves the criteria out
        d.method = 4;
        d.set_use_f(true);
        d.entry = "4".into();
        d.removal = "3".into();
        d.statistics = [false; 9];
        d.priors_size = false;
        d.separate = false;
        d.table = false;
        d.leave_one_out = false;
        d.save = [false; 3];
        d.select.clear();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "DISCRIMINANT GROUPS=region(1,5)\n  /VARIABLES=age monthly_spend q1_satisfaction\n  \
             /METHOD=RAO\n  /FIN=4\n  /FOUT=3."
        );
        assert!(runs(&syntax) >= 3);
        d.entry = "2".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the F to enter must be above the F to remove, which is 0 or more"
        );
        d.stepwise = false;
        assert!(d.syntax().unwrap().ends_with("q1_satisfaction."));

        d.select = "gender".into();
        d.select_value.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the value of the selection variable must be a number"
        );
        d.variables.push("region".into());
        assert_eq!(d.syntax().unwrap_err(), "region is the grouping variable");
        d.max = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the smallest group value must be below the largest"
        );
        d.max = "1.5".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the largest group value must be a whole number"
        );
    }

    #[test]
    fn binary_logistic_regression() {
        let mut d = Logistic::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the dependent variable");
        d.dependent = "q5_1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Covariates"
        );
        d.blocks[0].variables = v(&["age", "monthly_spend"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "LOGISTIC REGRESSION VARIABLES=q5_1\n  /METHOD=ENTER age monthly_spend."
        );
        assert!(runs(&syntax) >= 4);

        // a stepwise block, categorical covariates, every saved variable and option
        d.blocks.push(Block {
            variables: v(&["region", "gender", "q2_recommend"]),
            method: 2,
        });
        d.blocks.push(Block::default());
        d.categorical = vec![
            Categorical {
                name: "region".into(),
                contrast: 0,
                first: true,
            },
            Categorical {
                name: "gender".into(),
                contrast: 3,
                first: false,
            },
        ];
        d.save = [true; 10];
        d.goodfit = true;
        d.ci = true;
        d.iteration_history = true;
        d.cut = "0.4".into();
        d.entry = "0.1".into();
        d.removal = "0.2".into();
        d.iterations = "30".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "LOGISTIC REGRESSION VARIABLES=q5_1\n  /METHOD=ENTER age monthly_spend\n  \
             /METHOD=FSTEP(LR) region gender q2_recommend\n  /CATEGORICAL=region gender\n  \
             /CONTRAST(region)=INDICATOR(1)\n  /CONTRAST(gender)=HELMERT\n  \
             /SAVE=PRED PGROUP RESID LRESID SRESID ZRESID DEV COOK LEVER DFBETA\n  \
             /PRINT=GOODFIT ITER(1) CI(95)\n  /CRITERIA=PIN(0.1) POUT(0.2) ITERATE(30) CUT(0.4)."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 8);
        let data = session.dataset().unwrap();
        assert!(data.variable("PRE_1").is_ok());
        assert!(data.variable("PGR_1").is_ok());

        // backward in the first block, no constant
        d.blocks.truncate(1);
        d.blocks[0].method = 6;
        d.categorical.clear();
        d.save = [false; 10];
        d.goodfit = false;
        d.ci = false;
        d.iteration_history = false;
        d.cut = "0.5".into();
        d.entry = "0.05".into();
        d.removal = "0.10".into();
        d.iterations = "20".into();
        d.constant = false;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "LOGISTIC REGRESSION VARIABLES=q5_1\n  /METHOD=BSTEP(WALD) age monthly_spend\n  \
             /ORIGIN."
        );
        assert!(runs(&syntax) >= 4);

        d.categorical.push(Categorical {
            name: "region".into(),
            contrast: 0,
            first: false,
        });
        assert_eq!(
            d.syntax().unwrap_err(),
            "region is categorical but in no block"
        );
        d.categorical.clear();
        d.blocks[0].variables.push("q5_1".into());
        assert_eq!(d.syntax().unwrap_err(), "q5_1 is the dependent variable");
        d.blocks[0].variables.pop();
        d.ci = true;
        d.ci_level = "95.5".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the confidence level must be a whole number from 1 to 99"
        );
        d.ci = false;
        d.entry = "0.2".into();
        assert!(
            d.syntax()
                .unwrap_err()
                .starts_with("the probability to enter must be above 0")
        );
    }

    #[test]
    fn univariate() {
        let mut d = Univariate::default();
        assert_eq!(d.syntax().unwrap_err(), "choose the dependent variable");
        d.dependent = "monthly_spend".into();
        assert_eq!(d.syntax().unwrap_err(), "add a fixed factor or a covariate");
        d.factors = v(&["region", "gender"]);
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "UNIANOVA monthly_spend BY region gender.");
        assert!(runs(&syntax) >= 2);

        // post hoc tests, means compared, saved variables, every option
        d.covariates = v(&["age"]);
        d.post_hoc_factors = v(&["region"]);
        d.post_hoc[7] = true;
        d.post_hoc[1] = true;
        d.dunnett = true;
        d.dunnett_first = true;
        d.means_overall = true;
        d.means_factors = v(&["gender", "region"]);
        d.compare = true;
        d.adjustment = 1;
        d.save = [true; 8];
        d.print = [true; 5];
        d.alpha = "0.01".into();
        d.sstype = 0;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "UNIANOVA monthly_spend BY region gender WITH age\n  \
             /POSTHOC=region(BONFERRONI TUKEY DUNNETT(1))\n  /EMMEANS=TABLES(OVERALL)\n  \
             /EMMEANS=TABLES(region) COMPARE ADJ(BONFERRONI)\n  \
             /EMMEANS=TABLES(gender) COMPARE ADJ(BONFERRONI)\n  \
             /SAVE=PRED SEPRED RESID ZRESID SRESID DRESID COOK LEVER\n  /METHOD=SSTYPE(1)\n  \
             /PRINT=DESCRIPTIVE ETASQ OPOWER PARAMETER HOMOGENEITY\n  /CRITERIA=ALPHA(0.01)."
        );
        let mut session = open("survey_basic.sav");
        assert!(run_in(&mut session, &syntax) >= 10);
        let data = session.dataset().unwrap();
        assert!(data.variable("PRE_1").is_ok());
        assert!(data.variable("COO_1").is_ok());

        // a custom model of main effects and one interaction, no intercept
        let mut d = Univariate {
            dependent: "monthly_spend".into(),
            factors: v(&["region", "gender"]),
            covariates: v(&["age"]),
            custom: true,
            intercept: false,
            ..Default::default()
        };
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one term to the model"
        );
        d.building = v(&["age", "region", "gender"]);
        d.add_terms(false);
        d.building = v(&["gender", "region"]);
        d.add_terms(true);
        // a term already in the model is not added twice
        d.building = v(&["age"]);
        d.add_terms(false);
        assert_eq!(d.terms.len(), 4);
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "UNIANOVA monthly_spend BY region gender WITH age\n  /INTERCEPT=EXCLUDE\n  \
             /DESIGN=region gender age region*gender."
        );
        assert!(runs(&syntax) >= 2);

        d.covariates.clear();
        assert_eq!(
            d.syntax().unwrap_err(),
            "age in the model is no longer a factor or covariate"
        );
        d.post_hoc[0] = true;
        assert_eq!(
            d.syntax().unwrap_err(),
            "choose the factors of the post hoc tests"
        );
        d.post_hoc[0] = false;
        d.post_hoc_factors = v(&["gender"]);
        assert_eq!(d.syntax().unwrap_err(), "choose at least one post hoc test");
        d.factors.push("monthly_spend".into());
        assert_eq!(
            d.syntax().unwrap_err(),
            "monthly_spend is the dependent variable"
        );
    }

    #[test]
    fn repeated_measures() {
        let mut d = RepeatedMeasures::default();
        assert_eq!(
            d.syntax().unwrap_err(),
            "give the number of levels of factor1"
        );
        d.within[0].levels = "4".into();
        d.dependents = v(&["q5_1", "q5_2", "q5_3"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "Within-subjects variables needs 4 variables, one for each combination of levels"
        );
        d.dependents.push("q5_4".into());
        let syntax = d.syntax().unwrap();
        assert_eq!(syntax, "GLM q5_1 q5_2 q5_3 q5_4\n  /WSFACTOR=factor1 4.");
        assert!(runs(&syntax) >= 3);

        // two within-subjects factors, between-subjects factor and covariate, every option
        d.within = vec![
            WithinFactor {
                name: "brand".into(),
                levels: "2".into(),
            },
            WithinFactor {
                name: "time".into(),
                levels: "2".into(),
            },
        ];
        d.factors = v(&["gender"]);
        d.covariates = v(&["age"]);
        d.sstype = 3;
        d.print = [true; 3];
        d.alpha = "0.1".into();
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "GLM q5_1 q5_2 q5_3 q5_4 BY gender WITH age\n  /WSFACTOR=brand 2 time 2\n  \
             /METHOD=SSTYPE(4)\n  /PRINT=DESCRIPTIVE ETASQ OPOWER\n  /CRITERIA=ALPHA(0.1)."
        );
        assert!(runs(&syntax) >= 4);

        d.within[1].name = "Brand".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "Brand is the name of two within-subjects factors"
        );
        d.within[1].name = "two words".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "a within-subjects factor needs a name of one word"
        );
        d.within[1].name = "time".into();
        d.within[1].levels = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the number of levels of time must be a whole number of at least 2"
        );
    }
}
