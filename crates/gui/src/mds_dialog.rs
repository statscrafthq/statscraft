//! The Multidimensional Scaling dialogs: PROXSCAL and ALSCAL. Options left as they are add
//! nothing to the syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::dialogs::whole;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{names, number, quote};
use statscraft_data::{Dataset, Value, is_valid_name};

/// Where PROXSCAL finds the proximities.
pub const PROXSCAL_FORMATS: [&str; 3] = [
    "Square matrices, one variable per object",
    "One proximity per case, placed by a row and a column variable",
    "A matrix file written by PROXIMITIES or CLUSTER",
];

/// Which triangle of a square matrix holds the proximities; the first is the default.
pub const PROXSCAL_SHAPES: [(&str, &str); 3] = [
    ("LOWER", "Lower triangle"),
    ("UPPER", "Upper triangle"),
    ("BOTH", "Both triangles, averaged"),
];

pub const PROXSCAL_MODELS: [(&str, &str); 4] = [
    ("IDENTITY", "Identity"),
    ("WEIGHTED", "Weighted Euclidean"),
    ("GENERALIZED", "Generalized Euclidean"),
    ("REDUCED", "Reduced rank"),
];

pub const PROXSCAL_TRANSFORMATIONS: [(&str, &str); 4] = [
    ("RATIO", "Ratio"),
    ("INTERVAL", "Interval"),
    ("ORDINAL", "Ordinal"),
    ("SPLINE", "Spline"),
];

pub const PROXSCAL_STARTS: [(&str, &str); 3] = [
    ("SIMPLEX", "Simplex"),
    ("TORGERSON", "Torgerson"),
    ("RANDOM", "Random"),
];

/// The tables of /PRINT.
pub const PROXSCAL_PRINT: [(&str, &str); 10] = [
    ("INPUT", "Input proximities"),
    ("RANDOM", "Stress of each random start"),
    ("HISTORY", "Iteration history"),
    ("STRESS", "Stress and fit measures"),
    ("DECOMPOSITION", "Stress by object and source"),
    ("COMMON", "Common space"),
    ("DISTANCES", "Distances"),
    ("WEIGHTS", "Space weights"),
    ("INDIVIDUAL", "Individual spaces"),
    ("TRANSFORMATIONS", "Transformed proximities"),
];

const PROXSCAL_DEFAULT_PRINT: [bool; 10] = [
    false, false, false, true, false, true, false, true, false, false,
];

/// The charts of /PLOT; the last three take sources.
pub const PROXSCAL_PLOT: [(&str, &str); 6] = [
    ("STRESS", "Stress by dimensions"),
    ("COMMON", "Common space"),
    ("WEIGHTS", "Space weights"),
    ("INDIVIDUAL", "Individual spaces"),
    ("TRANSFORMATIONS", "Transformations"),
    ("RESIDUALS", "Residuals"),
];

const PROXSCAL_DEFAULT_PLOT: [bool; 6] = [false, true, true, false, false, false];

/// The files of /OUTFILE.
pub const PROXSCAL_OUTFILE: [(&str, &str); 4] = [
    ("COMMON", "Common space"),
    ("WEIGHTS", "Space weights"),
    ("DISTANCES", "Distances"),
    ("TRANSFORMATIONS", "Transformed proximities"),
];

/// PROXSCAL: multidimensional scaling by majorization.
#[derive(Clone, PartialEq, Debug)]
pub struct Proxscal {
    /// Index into `PROXSCAL_FORMATS`.
    pub format: usize,
    /// The columns of the square matrices, or the variables that hold the proximities.
    pub variables: Vec<String>,
    /// Tells the stacked matrices, or the proximities of one variable, apart; may be empty.
    pub source: String,
    pub row: String,
    pub column: String,
    /// One per proximity variable, or none.
    pub weights: Vec<String>,
    pub matrix_file: String,
    /// Index into `PROXSCAL_SHAPES`.
    pub shape: usize,
    pub similarities: bool,
    /// Index into `PROXSCAL_MODELS`.
    pub model: usize,
    pub rank: String,
    /// Index into `PROXSCAL_TRANSFORMATIONS`.
    pub transformation: usize,
    pub untie: bool,
    pub degree: String,
    pub knots: String,
    pub unconditional: bool,
    /// Index into `PROXSCAL_STARTS`.
    pub start: usize,
    pub random_starts: String,
    pub min_dimensions: String,
    pub max_dimensions: String,
    pub max_iterations: String,
    pub diff_stress: String,
    pub min_stress: String,
    pub acceleration: bool,
    /// One flag per entry of `PROXSCAL_PRINT`.
    pub print: [bool; 10],
    /// One flag per entry of `PROXSCAL_PLOT`.
    pub plot: [bool; 6],
    /// The sources of the individual, transformation and residuals plots; none means all.
    pub plot_sources: [Vec<String>; 3],
    /// One path per entry of `PROXSCAL_OUTFILE`; empty writes nothing.
    pub outfiles: [String; 4],
}

impl Default for Proxscal {
    fn default() -> Self {
        Self {
            format: 0,
            variables: Vec::new(),
            source: String::new(),
            row: String::new(),
            column: String::new(),
            weights: Vec::new(),
            matrix_file: String::new(),
            shape: 0,
            similarities: false,
            model: 0,
            rank: "2".into(),
            transformation: 0,
            untie: false,
            degree: "2".into(),
            knots: "1".into(),
            unconditional: false,
            start: 0,
            random_starts: "1".into(),
            min_dimensions: "2".into(),
            max_dimensions: "2".into(),
            max_iterations: "100".into(),
            diff_stress: "0.0001".into(),
            min_stress: "0.0001".into(),
            acceleration: true,
            print: PROXSCAL_DEFAULT_PRINT,
            plot: PROXSCAL_DEFAULT_PLOT,
            plot_sources: Default::default(),
            outfiles: Default::default(),
        }
    }
}

/// A whole number from `least`, the default when the field is empty.
pub(crate) fn count(text: &str, least: u32, what: &str, default: u32) -> Result<u32, String> {
    Ok(whole(text, least, what)?.unwrap_or(default))
}

/// A number from 0 to 1, the default when the field is empty.
fn fraction(text: &str, what: &str, default: f64) -> Result<f64, String> {
    if text.trim().is_empty() {
        return Ok(default);
    }
    match parse(text, what)? {
        v if (0.0..=1.0).contains(&v) => Ok(v),
        _ => Err(format!("{what} must be from 0 to 1")),
    }
}

/// The valid values of a numeric variable, ascending, each once.
fn distinct(data: &Dataset, name: &str) -> Vec<f64> {
    let Ok(var) = data.variable(name) else {
        return Vec::new();
    };
    let Ok(values) = var.numeric() else {
        return Vec::new();
    };
    let mut out: Vec<f64> = values
        .iter()
        .flatten()
        .filter(|v| !var.is_user_missing(*v))
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup();
    out
}

/// What the command calls the source of code `k`: its value label, else SRC_ and the code.
fn source_name(data: &Dataset, name: &str, k: f64) -> String {
    data.variable(name)
        .ok()
        .and_then(|v| {
            v.value_labels()
                .iter()
                .find(|(value, _)| *value == Value::Number(k))
                .map(|(_, label)| label.clone())
        })
        .unwrap_or_else(|| format!("SRC_{}", number(k)))
}

impl Proxscal {
    fn square(&self) -> bool {
        self.format == 0
    }

    fn by_case(&self) -> bool {
        self.format == 1
    }

    /// The source variable counts: stacked matrices, or one proximity variable by case.
    pub(crate) fn source_applies(&self) -> bool {
        self.square() || (self.by_case() && self.variables.len() <= 1)
    }

    /// The sources as the command names them, for the plots that take sources; only names the
    /// syntax can write are offered.
    pub fn sources(&self, data: &Dataset) -> Vec<String> {
        let source = self.source.trim();
        let all = match self.format {
            _ if !source.is_empty() && self.source_applies() => distinct(data, source)
                .into_iter()
                .map(|k| source_name(data, source, k))
                .collect(),
            0 if !self.variables.is_empty() => (1..=data.case_count() / self.variables.len())
                .map(|k| format!("SRC_{k}"))
                .collect(),
            1 => self.variables.clone(),
            _ => vec!["SRC_1".to_string()],
        };
        all.into_iter().filter(|n| is_valid_name(n)).collect()
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let mut text = String::from("PROXSCAL");
        let source = self.source.trim();
        // the number of objects, where the dialog can tell it
        let mut objects = None;
        if self.format == 2 {
            if self.matrix_file.trim().is_empty() {
                return Err("choose the matrix file".into());
            }
        } else {
            text.push_str(&format!(" {}", names(&self.variables, "the proximities")?));
            let (row, column) = (self.row.trim(), self.column.trim());
            let extra = [row, column, source].map(String::from);
            numeric(
                data,
                self.variables.iter().chain(&self.weights).chain(&extra),
            )?;
            if !self.weights.is_empty() && self.weights.len() != self.variables.len() {
                return Err(format!(
                    "give one weight variable for each of the {} proximity variables",
                    self.variables.len()
                ));
            }
            if self.square() {
                let n = self.variables.len();
                let cases = data.case_count();
                if source.is_empty() && (cases == 0 || !cases.is_multiple_of(n)) {
                    return Err(format!(
                        "square matrices of {n} variables need a multiple of {n} cases; there are {cases}"
                    ));
                }
                if !source.is_empty() {
                    text.push_str(&format!("\n  /TABLE={source}"));
                }
                objects = Some(n);
            } else {
                if row.is_empty() || column.is_empty() {
                    return Err("choose the row and the column variable".into());
                }
                if !source.is_empty() && !self.source_applies() {
                    return Err("a source variable works with one proximity variable only".into());
                }
                text.push_str(&format!("\n  /TABLE={row} BY {column}"));
                if !source.is_empty() {
                    text.push_str(&format!(" BY {source}"));
                }
                let codes = distinct(data, row)
                    .into_iter()
                    .chain(distinct(data, column));
                objects = Some(codes.fold(0.0, f64::max) as usize);
            }
            if self.shape > 0 {
                text.push_str(&format!("\n  /SHAPE={}", PROXSCAL_SHAPES[self.shape].0));
            }
        }
        if let Some(n) = objects
            && n < 3
        {
            return Err(format!("scaling needs at least 3 objects; there are {n}"));
        }
        match self.start {
            0 => {}
            1 => text.push_str("\n  /INITIAL=TORGERSON"),
            _ => {
                let n = count(&self.random_starts, 1, "the number of random starts", 1)?;
                text.push_str(&format!("\n  /INITIAL=RANDOM({n})"));
            }
        }
        if !self.weights.is_empty() && self.format != 2 {
            text.push_str(&format!("\n  /WEIGHTS={}", self.weights.join(" ")));
        }
        if self.unconditional {
            text.push_str("\n  /CONDITION=UNCONDITIONAL");
        }
        text.push_str(&self.transformation_syntax()?);
        if self.similarities {
            text.push_str("\n  /PROXIMITIES=SIMILARITIES");
        }
        let min = count(&self.min_dimensions, 1, "the fewest dimensions", 2)?;
        let max = count(&self.max_dimensions, 1, "the most dimensions", 2)?;
        if min > max {
            return Err("the fewest dimensions are more than the most".into());
        }
        if let Some(n) = objects
            && max as usize > n - 1
        {
            return Err(format!("{n} objects allow at most {} dimensions", n - 1));
        }
        match self.model {
            0 => {}
            3 => {
                let rank = count(&self.rank, 1, "the rank", 2)?;
                if rank >= max {
                    return Err("the rank must be below the most dimensions".into());
                }
                text.push_str(&format!("\n  /MODEL=REDUCED({rank})"));
            }
            m => text.push_str(&format!("\n  /MODEL={}", PROXSCAL_MODELS[m].0)),
        }
        if !self.acceleration {
            text.push_str("\n  /ACCELERATION=NONE");
        }
        let mut criteria = Vec::new();
        if (min, max) != (2, 2) {
            criteria.push(match min == max {
                true => format!("DIMENSIONS({min})"),
                false => format!("DIMENSIONS({min},{max})"),
            });
        }
        let iterations = count(&self.max_iterations, 1, "the most iterations", 100)?;
        if iterations != 100 {
            criteria.push(format!("MAXITER({iterations})"));
        }
        for (field, keyword, what) in [
            (&self.diff_stress, "DIFFSTRESS", "the stress improvement"),
            (&self.min_stress, "MINSTRESS", "the smallest stress"),
        ] {
            let v = fraction(field, what, 0.0001)?;
            if v != 0.0001 {
                criteria.push(format!("{keyword}({})", number(v)));
            }
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        if self.print != PROXSCAL_DEFAULT_PRINT {
            let print = keywords(&PROXSCAL_PRINT, &self.print);
            match print.is_empty() {
                true => text.push_str("\n  /PRINT=NONE"),
                false => text.push_str(&format!("\n  /PRINT={}", print.join(" "))),
            }
        }
        if self.plot != PROXSCAL_DEFAULT_PLOT {
            text.push_str(&self.plot_syntax(data));
        }
        let outfiles: Vec<String> = PROXSCAL_OUTFILE
            .iter()
            .zip(&self.outfiles)
            .filter(|(_, path)| !path.trim().is_empty())
            .map(|((keyword, _), path)| format!("{keyword}({})", quote(path.trim())))
            .collect();
        if !outfiles.is_empty() {
            text.push_str(&format!("\n  /OUTFILE={}", outfiles.join(" ")));
        }
        if self.format == 2 {
            text.push_str(&format!(
                "\n  /MATRIX=IN({})",
                quote(self.matrix_file.trim())
            ));
        }
        text.push('.');
        Ok(text)
    }

    /// /TRANSFORMATION when not ratio.
    fn transformation_syntax(&self) -> Result<String, String> {
        Ok(match self.transformation {
            0 => String::new(),
            1 => "\n  /TRANSFORMATION=INTERVAL".into(),
            2 if self.untie => "\n  /TRANSFORMATION=ORDINAL(UNTIE)".into(),
            2 => "\n  /TRANSFORMATION=ORDINAL".into(),
            _ => {
                let degree = count(&self.degree, 1, "the degree of the spline", 2)?;
                if degree > 3 {
                    return Err("the degree of the spline goes from 1 to 3".into());
                }
                let knots = count(&self.knots, 0, "the number of interior knots", 1)?;
                format!("\n  /TRANSFORMATION=SPLINE DEGREE={degree} INKNOT={knots}")
            }
        })
    }

    /// /PLOT with the charts checked; the charts by source name the sources still offered.
    fn plot_syntax(&self, data: &Dataset) -> String {
        // reading the source codes costs a pass over the data: only when a plot needs them
        let offered = match self.plot[3..].contains(&true) {
            true => self.sources(data),
            false => Vec::new(),
        };
        let mut plots = Vec::new();
        for (i, (keyword, _)) in PROXSCAL_PLOT.iter().enumerate() {
            if !self.plot[i] {
                continue;
            }
            if i < 3 {
                plots.push(keyword.to_string());
                continue;
            }
            let chosen: Vec<&str> = offered
                .iter()
                .filter(|s| self.plot_sources[i - 3].contains(s))
                .map(String::as_str)
                .collect();
            plots.push(match chosen.is_empty() {
                true => format!("{keyword}(ALL)"),
                false => format!("{keyword}({})", chosen.join(" ")),
            });
        }
        match plots.is_empty() {
            true => "\n  /PLOT=NONE".into(),
            false => format!("\n  /PLOT={}", plots.join(" ")),
        }
    }
}

/// The distance measures offered when ALSCAL makes its matrix from the data; the first is the
/// default of PROXIMITIES.
pub const ALSCAL_MEASURES: [(&str, &str); 4] = [
    ("EUCLID", "Euclidean distance"),
    ("SEUCLID", "Squared Euclidean distance"),
    ("CHEBYCHEV", "Chebychev"),
    ("BLOCK", "Block"),
];

/// How each variable is rescaled before the distances; the first leaves it as it is.
pub const ALSCAL_STANDARDIZE: [(&str, &str); 7] = [
    ("", "None"),
    ("Z", "Z scores"),
    ("SD", "Standard deviation 1"),
    ("RANGE", "Range 1"),
    ("MAX", "Largest value 1"),
    ("MEAN", "Mean 1"),
    ("RESCALE", "From 0 to 1"),
];

pub const ALSCAL_LEVELS: [(&str, &str); 4] = [
    ("ORDINAL", "Ordinal"),
    ("INTERVAL", "Interval"),
    ("RATIO", "Ratio"),
    ("NOMINAL", "Nominal"),
];

pub const ALSCAL_PLOTS: [(&str, &str); 3] = [
    ("", "None"),
    ("DEFAULT", "Configuration and fit"),
    ("ALL", "Configuration, fit and transformations"),
];

/// The dataset that holds the distances made from the data, so the open data stay active.
pub(crate) const DISTANCES: &str = "alscal_distances";

/// ALSCAL: multidimensional scaling by alternating least squares.
#[derive(Clone, PartialEq, Debug)]
pub struct Alscal {
    /// The variables are data to make distances of, not the columns of square matrices.
    pub from_data: bool,
    pub variables: Vec<String>,
    /// From data: distances between the cases instead of between the variables.
    pub between_cases: bool,
    /// Index into `ALSCAL_MEASURES`.
    pub measure: usize,
    /// Index into `ALSCAL_STANDARDIZE`.
    pub standardize: usize,
    /// Index into `ALSCAL_LEVELS`.
    pub level: usize,
    pub untie: bool,
    pub similar: bool,
    pub degree: String,
    pub unconditional: bool,
    pub indscal: bool,
    pub min_dimensions: String,
    pub max_dimensions: String,
    pub converge: String,
    pub iterations: String,
    pub stress_min: String,
    pub cutoff: String,
    pub negative: bool,
    pub print_data: bool,
    pub print_header: bool,
    /// Index into `ALSCAL_PLOTS`.
    pub plot: usize,
    pub outfile: String,
}

impl Default for Alscal {
    fn default() -> Self {
        Self {
            from_data: false,
            variables: Vec::new(),
            between_cases: false,
            measure: 0,
            standardize: 0,
            level: 0,
            untie: false,
            similar: false,
            degree: "1".into(),
            unconditional: false,
            indscal: false,
            min_dimensions: "2".into(),
            max_dimensions: "2".into(),
            converge: "0.001".into(),
            iterations: "30".into(),
            stress_min: "0.005".into(),
            cutoff: "0".into(),
            negative: false,
            print_data: false,
            print_header: false,
            plot: 0,
            outfile: String::new(),
        }
    }
}

impl Alscal {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let list = names(&self.variables, "Variables")?;
        numeric(data, &self.variables)?;
        let cases = data.case_count();
        let mut text = String::new();
        // the number of objects and of matrices
        let (objects, matrices) = if self.from_data {
            text.push_str(&format!("DATASET DECLARE {DISTANCES}.\nPROXIMITIES {list}"));
            if !self.between_cases {
                text.push_str("\n  /VIEW=VARIABLE");
            }
            if self.standardize > 0 {
                let method = ALSCAL_STANDARDIZE[self.standardize].0;
                text.push_str(&format!("\n  /STANDARDIZE=VARIABLE {method}"));
            }
            if self.measure > 0 {
                let measure = ALSCAL_MEASURES[self.measure].0;
                text.push_str(&format!("\n  /MEASURE={measure}"));
            }
            text.push_str(&format!(
                "\n  /PRINT=NONE\n  /MATRIX=OUT({DISTANCES}).\nALSCAL"
            ));
            let n = match self.between_cases {
                true => cases,
                false => self.variables.len(),
            };
            (n, 1)
        } else {
            let n = self.variables.len();
            if cases == 0 || !cases.is_multiple_of(n) {
                return Err(format!(
                    "square matrices of {n} variables need a multiple of {n} cases; there are {cases}"
                ));
            }
            text.push_str(&format!("ALSCAL VARIABLES={list}"));
            (n, cases / n)
        };
        if objects < 4 {
            return Err(format!(
                "scaling needs at least 4 objects; there are {objects}"
            ));
        }
        match self.level {
            0 => {
                let options: Vec<&str> = [(self.untie, "UNTIE"), (self.similar, "SIMILAR")]
                    .into_iter()
                    .filter_map(|(on, k)| on.then_some(k))
                    .collect();
                if !options.is_empty() {
                    text.push_str(&format!("\n  /LEVEL=ORDINAL({})", options.join(" ")));
                }
            }
            3 => text.push_str("\n  /LEVEL=NOMINAL"),
            l => {
                let degree = count(&self.degree, 1, "the degree", 1)?;
                if degree > 4 {
                    return Err("the degree goes from 1 to 4".into());
                }
                text.push_str(&format!("\n  /LEVEL={}({degree})", ALSCAL_LEVELS[l].0));
            }
        }
        if self.unconditional {
            text.push_str("\n  /CONDITION=UNCONDITIONAL");
        }
        let min = count(&self.min_dimensions, 1, "the fewest dimensions", 2)?;
        let max = count(&self.max_dimensions, 1, "the most dimensions", 2)?;
        if max > 6 {
            return Err("the command scales at most 6 dimensions".into());
        }
        if min > max {
            return Err("the fewest dimensions are more than the most".into());
        }
        if max as usize > objects - 1 {
            return Err(format!(
                "{objects} objects allow at most {} dimensions",
                objects - 1
            ));
        }
        if self.indscal {
            if min < 2 {
                return Err("the individual differences model needs at least 2 dimensions".into());
            }
            if matrices < 2 {
                return Err(
                    "the individual differences model needs several matrices; there is one".into(),
                );
            }
            text.push_str("\n  /MODEL=INDSCAL");
        }
        let mut criteria = Vec::new();
        let converge = match self.converge.trim() {
            "" => 0.001,
            t => parse(t, "the convergence")?,
        };
        if converge < 0.0 {
            return Err("the convergence cannot be negative".into());
        }
        if converge != 0.001 {
            criteria.push(format!("CONVERGE({})", number(converge)));
        }
        let iterations = count(&self.iterations, 1, "the most iterations", 30)?;
        if iterations != 30 {
            criteria.push(format!("ITER({iterations})"));
        }
        let stress_min = fraction(&self.stress_min, "the smallest S-stress", 0.005)?;
        if stress_min != 0.005 {
            criteria.push(format!("STRESSMIN({})", number(stress_min)));
        }
        let cutoff = match self.cutoff.trim() {
            "" => 0.0,
            t => parse(t, "the cutoff")?,
        };
        if cutoff != 0.0 {
            criteria.push(format!("CUTOFF({})", number(cutoff)));
        }
        // the Euclidean model has no weights to keep from going negative
        if self.negative && self.indscal {
            criteria.push("NEGATIVE".into());
        }
        if (min, max) != (2, 2) {
            criteria.push(match min == max {
                true => format!("DIMENS({min})"),
                false => format!("DIMENS({min},{max})"),
            });
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        let print: Vec<&str> = [(self.print_data, "DATA"), (self.print_header, "HEADER")]
            .into_iter()
            .filter_map(|(on, k)| on.then_some(k))
            .collect();
        if !print.is_empty() {
            text.push_str(&format!("\n  /PRINT={}", print.join(" ")));
        }
        if self.plot > 0 {
            text.push_str(&format!("\n  /PLOT={}", ALSCAL_PLOTS[self.plot].0));
        }
        let outfile = self.outfile.trim();
        if !outfile.is_empty() {
            text.push_str(&format!("\n  /OUTFILE={}", quote(outfile)));
        }
        if self.from_data {
            text.push_str(&format!(
                "\n  /MATRIX=IN({DISTANCES}).\nDATASET CLOSE {DISTANCES}"
            ));
        }
        text.push('.');
        Ok(text)
    }
}

/// How the cases form sources; the last four take a number or a variable.
pub const PREFSCAL_LAYOUTS: [&str; 5] = [
    "All cases are one source",
    "Cases per source",
    "Number of sources",
    "Row variable (numbers the rows of a source)",
    "Source variable (numbers the sources)",
];

/// Keyword and label of each transformation of the proximities.
pub const PREFSCAL_TRANSFORMATIONS: [(&str, &str); 4] = [
    ("NONE", "None"),
    ("LINEAR", "Linear"),
    ("ORDINAL", "Ordinal"),
    ("SPLINE", "Spline"),
];

pub const PREFSCAL_CONDITIONS: [(&str, &str); 3] = [
    ("ROW", "Row"),
    ("MATRIX", "Matrix (source)"),
    ("UNCONDITIONAL", "Unconditional"),
];

pub const PREFSCAL_MODELS: [(&str, &str); 3] = [
    ("IDENTITY", "Identity"),
    ("WEIGHTED", "Weighted Euclidean"),
    ("GENERALIZED", "Generalized Euclidean"),
];

/// The first is the command's own start; the last two take a number.
pub const PREFSCAL_STARTS: [(&str, &str); 6] = [
    ("CLASSICAL", "Classical, triangle bounds"),
    ("CLASSICAL(SPEARMAN)", "Classical, Spearman"),
    ("CORRESPONDENCE", "Correspondence"),
    ("ROSSCLIFF", "Ross-Cliff"),
    ("CENTROIDS", "Centroids"),
    ("RANDOM", "Random starts"),
];

pub const PREFSCAL_PRINT: [(&str, &str); 11] = [
    ("INPUT", "Input data"),
    ("MULTIPLE", "Multiple random starts"),
    ("INITIAL", "Initial configuration"),
    ("HISTORY", "Iteration history"),
    ("MEASURES", "Measures"),
    ("DECOMPOSITION", "Decomposition of stress"),
    ("COMMON", "Common space"),
    ("DISTANCES", "Distances"),
    ("WEIGHTS", "Weights"),
    ("INDIVIDUAL", "Individual spaces"),
    ("TRANSFORMATIONS", "Transformed proximities"),
];

pub const PREFSCAL_PLOT: [(&str, &str); 10] = [
    ("MULTIPLE", "Random starts"),
    ("INITIAL", "Initial common space"),
    ("STRESS", "Scree plot"),
    ("COMMON", "Common space"),
    ("WEIGHTS", "Dimension weights"),
    ("INDIVIDUAL", "Individual spaces"),
    ("TRANSFORMATIONS", "Transformation plots"),
    ("SHEPARD", "Shepard plots"),
    ("FIT", "Fit plots"),
    ("RESIDUALS", "Residual plots"),
];

// what the command prints and plots when PRINT and PLOT are left out
const PREFSCAL_PRINT_DEFAULT: [bool; 11] = [
    false, false, false, false, true, false, true, false, true, false, false,
];
const PREFSCAL_PLOT_DEFAULT: [bool; 10] = [
    false, false, false, true, true, false, false, false, false, false,
];

/// PREFSCAL: multidimensional unfolding of a preference table, the variables being its columns
/// and the cases its rows.
#[derive(Clone, PartialEq, Debug)]
pub struct Prefscal {
    pub variables: Vec<String>,
    /// Index into `PREFSCAL_LAYOUTS`.
    pub layout: usize,
    /// The count, or the variable, of the layout.
    pub layout_value: String,
    pub similarities: bool,
    /// Index into `PREFSCAL_TRANSFORMATIONS`.
    pub transformation: usize,
    pub intercept: bool,
    pub untie: bool,
    pub degree: String,
    pub knots: String,
    /// Index into `PREFSCAL_CONDITIONS`.
    pub condition: usize,
    /// Index into `PREFSCAL_MODELS`.
    pub model: usize,
    /// Index into `PREFSCAL_STARTS`.
    pub start: usize,
    /// For the centroid and random starts; empty: 1.
    pub start_count: String,
    pub lambda: String,
    pub omega: String,
    pub min_dimensions: String,
    pub max_dimensions: String,
    pub max_iterations: String,
    pub diff_stress: String,
    pub min_stress: String,
    pub print: [bool; 11],
    pub plot: [bool; 10],
}

impl Default for Prefscal {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            layout: 0,
            layout_value: String::new(),
            similarities: false,
            transformation: 2,
            intercept: false,
            untie: false,
            degree: "2".into(),
            knots: "1".into(),
            condition: 0,
            model: 0,
            start: 0,
            start_count: String::new(),
            lambda: "0.5".into(),
            omega: "1".into(),
            min_dimensions: "2".into(),
            max_dimensions: "2".into(),
            max_iterations: "5000".into(),
            diff_stress: "0.000001".into(),
            min_stress: "0.0001".into(),
            print: PREFSCAL_PRINT_DEFAULT,
            plot: PREFSCAL_PLOT_DEFAULT,
        }
    }
}

/// A keyword list of PRINT or PLOT, written only when it differs from the command's own.
fn prefscal_list(
    name: &str,
    table: &[(&str, &str)],
    on: &[bool],
    default: &[bool],
) -> Option<String> {
    if on == default {
        return None;
    }
    let words: Vec<&str> = table
        .iter()
        .zip(on)
        .filter_map(|((k, _), on)| on.then_some(*k))
        .collect();
    Some(match words.is_empty() {
        true => format!("\n  /{name}=NONE"),
        false => format!("\n  /{name}={}", words.join(" ")),
    })
}

impl Prefscal {
    /// Whether the layout names a variable instead of a count.
    pub fn layout_is_variable(&self) -> bool {
        self.layout >= 3
    }

    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        let list = names(&self.variables, "the preference table")?;
        numeric(data, &self.variables)?;
        let mut text = format!("PREFSCAL {list}");
        let value = self.layout_value.trim();
        if self.layout > 0 {
            let key = ["", "ROWS", "SOURCES", "ROWS", "SOURCES"][self.layout];
            let inner = if self.layout_is_variable() {
                if value.is_empty() {
                    return Err("choose the variable of the layout".into());
                }
                numeric(data, [&self.layout_value])?;
                value.to_string()
            } else {
                match count(value, 1, "the layout number", 0)? {
                    0 => return Err("give the number of the layout".into()),
                    n => n.to_string(),
                }
            };
            text.push_str(&format!("\n  /INPUT={key}({inner})"));
        }
        if self.similarities {
            text.push_str("\n  /PROXIMITIES=SIMILARITIES");
        }
        if self.start > 0 {
            let key = PREFSCAL_STARTS[self.start].0;
            let n = count(&self.start_count, 1, "the number of starts", 1)?;
            match self.start >= 4 && !self.start_count.trim().is_empty() {
                true => text.push_str(&format!("\n  /INITIAL={key}({n})")),
                false => text.push_str(&format!("\n  /INITIAL={key}")),
            }
        }
        if self.condition > 0 {
            text.push_str(&format!(
                "\n  /CONDITION={}",
                PREFSCAL_CONDITIONS[self.condition].0
            ));
        }
        match self.transformation {
            0 | 1 => text.push_str(&format!(
                "\n  /TRANSFORMATION={}{}",
                PREFSCAL_TRANSFORMATIONS[self.transformation].0,
                if self.intercept { "(INTERCEPT)" } else { "" }
            )),
            2 if self.untie => text.push_str("\n  /TRANSFORMATION=ORDINAL(UNTIE)"),
            2 => {}
            _ => {
                let degree = count(&self.degree, 1, "the degree", 2)?;
                if degree > 3 {
                    return Err("the degree of the spline goes from 1 to 3".into());
                }
                let knots = count(&self.knots, 0, "the number of inner knots", 1)?;
                let intercept = if self.intercept { "INTERCEPT " } else { "" };
                text.push_str(&format!(
                    "\n  /TRANSFORMATION=SPLINE({intercept}ORDER={degree} INKNOT={knots})"
                ));
            }
        }
        if self.model > 0 {
            text.push_str(&format!("\n  /MODEL={}", PREFSCAL_MODELS[self.model].0));
        }
        let number_or = |text: &str, what: &str, default: f64| match text.trim() {
            "" => Ok(default),
            t => parse(t, what),
        };
        let lambda = number_or(&self.lambda, "LAMBDA", 0.5)?;
        let omega = number_or(&self.omega, "OMEGA", 1.0)?;
        if lambda <= 0.0 || lambda > 1.0 {
            return Err("LAMBDA must be above 0 and at most 1".into());
        }
        if omega < 0.0 {
            return Err("OMEGA cannot be negative".into());
        }
        let mut penalty = Vec::new();
        if lambda != 0.5 {
            penalty.push(format!("LAMBDA({})", number(lambda)));
        }
        if omega != 1.0 {
            penalty.push(format!("OMEGA({})", number(omega)));
        }
        if !penalty.is_empty() {
            text.push_str(&format!("\n  /PENALTY={}", penalty.join(" ")));
        }
        let min = count(&self.min_dimensions, 1, "the fewest dimensions", 2)?;
        let max = count(&self.max_dimensions, 1, "the most dimensions", min)?;
        if min > max {
            return Err("the fewest dimensions are more than the most".into());
        }
        let mut criteria = Vec::new();
        if (min, max) != (2, 2) {
            criteria.push(format!("DIMENSIONS({min},{max})"));
        }
        let iterations = count(&self.max_iterations, 0, "the most iterations", 5000)?;
        if iterations != 5000 {
            criteria.push(format!("MAXITER({iterations})"));
        }
        for (field, key, what, default) in [
            (
                &self.diff_stress,
                "DIFFSTRESS",
                "the stress change",
                0.000001,
            ),
            (&self.min_stress, "MINSTRESS", "the smallest stress", 0.0001),
        ] {
            let v = number_or(field, what, default)?;
            if v < 0.0 {
                return Err(format!("{what} cannot be negative"));
            }
            if v != default {
                criteria.push(format!("{key}({})", number(v)));
            }
        }
        if !criteria.is_empty() {
            text.push_str(&format!("\n  /CRITERIA={}", criteria.join(" ")));
        }
        text.extend(prefscal_list(
            "PRINT",
            &PREFSCAL_PRINT,
            &self.print,
            &PREFSCAL_PRINT_DEFAULT,
        ));
        text.extend(prefscal_list(
            "PLOT",
            &PREFSCAL_PLOT,
            &self.plot,
            &PREFSCAL_PLOT_DEFAULT,
        ));
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{run_in, v};
    use statscraft_engine::session::Session;

    /// Two invented taste panels, each rating how unlike five snacks are, lower triangle.
    pub(crate) fn snacks() -> Session {
        let mut session = Session::default();
        session
            .run(
                "DATA LIST LIST /panel chips nuts pretzel popcorn crackers.
BEGIN DATA
1 0 . . . .
1 3 0 . . .
1 6 4 0 . .
1 8 7 2 0 .
1 5 6 5 4 0
2 0 . . . .
2 2 0 . . .
2 7 5 0 . .
2 9 6 3 0 .
2 4 7 6 5 0
END DATA.",
                None,
            )
            .unwrap();
        session
    }

    fn proxscal() -> Proxscal {
        Proxscal {
            variables: v(&["chips", "nuts", "pretzel", "popcorn", "crackers"]),
            ..Default::default()
        }
    }

    fn alscal() -> Alscal {
        Alscal {
            variables: v(&["chips", "nuts", "pretzel", "popcorn", "crackers"]),
            ..Default::default()
        }
    }

    #[test]
    fn proxscal_defaults_add_nothing() {
        let mut session = snacks();
        let data = session.dataset().unwrap().clone();
        let syntax = proxscal().syntax(&data).unwrap();
        assert_eq!(syntax, "PROXSCAL chips nuts pretzel popcorn crackers.");
        assert!(run_in(&mut session, &syntax) >= 2);
        assert_eq!(proxscal().sources(&data), v(&["SRC_1", "SRC_2"]));
    }

    #[test]
    fn proxscal_checks_what_the_command_would_reject() {
        let data = snacks().dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut Proxscal)| {
            let mut d = proxscal();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.variables.clear()),
            "add at least one variable to the proximities"
        );
        assert_eq!(
            err(&|d| d.variables.truncate(3)),
            "square matrices of 3 variables need a multiple of 3 cases; there are 10"
        );
        assert_eq!(
            err(&|d| {
                d.variables.truncate(2);
                d.source = "panel".into();
            }),
            "scaling needs at least 3 objects; there are 2"
        );
        assert_eq!(
            err(&|d| d.max_dimensions = "5".into()),
            "5 objects allow at most 4 dimensions"
        );
        assert_eq!(
            err(&|d| d.min_dimensions = "3".into()),
            "the fewest dimensions are more than the most"
        );
        assert_eq!(
            err(&|d| d.model = 3),
            "the rank must be below the most dimensions"
        );
        assert_eq!(
            err(&|d| d.weights = v(&["panel"])),
            "give one weight variable for each of the 5 proximity variables"
        );
        assert_eq!(
            err(&|d| {
                d.transformation = 3;
                d.degree = "4".into();
            }),
            "the degree of the spline goes from 1 to 3"
        );
        assert_eq!(
            err(&|d| d.min_stress = "2".into()),
            "the smallest stress must be from 0 to 1"
        );
        assert_eq!(
            err(&|d| {
                d.format = 1;
                d.row = "panel".into();
            }),
            "choose the row and the column variable"
        );
        assert_eq!(
            err(&|d| {
                d.format = 1;
                d.row = "panel".into();
                d.column = "panel".into();
                d.source = "chips".into();
            }),
            "a source variable works with one proximity variable only"
        );
        assert_eq!(err(&|d| d.format = 2), "choose the matrix file");
    }

    #[test]
    fn alscal_checks_what_the_command_would_reject() {
        let data = snacks().dataset().unwrap().clone();
        let err = |change: &dyn Fn(&mut Alscal)| {
            let mut d = alscal();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert_eq!(
            err(&|d| d.variables.truncate(4)),
            "square matrices of 4 variables need a multiple of 4 cases; there are 10"
        );
        assert_eq!(
            err(&|d| {
                d.from_data = true;
                d.variables.truncate(3);
            }),
            "scaling needs at least 4 objects; there are 3"
        );
        assert_eq!(
            err(&|d| d.max_dimensions = "7".into()),
            "the command scales at most 6 dimensions"
        );
        assert_eq!(
            err(&|d| d.max_dimensions = "5".into()),
            "5 objects allow at most 4 dimensions"
        );
        assert_eq!(
            err(&|d| {
                d.indscal = true;
                d.min_dimensions = "1".into();
            }),
            "the individual differences model needs at least 2 dimensions"
        );
        assert_eq!(
            err(&|d| {
                d.from_data = true;
                d.indscal = true;
            }),
            "the individual differences model needs several matrices; there is one"
        );
        assert_eq!(
            err(&|d| {
                d.level = 1;
                d.degree = "5".into();
            }),
            "the degree goes from 1 to 4"
        );
        assert_eq!(
            err(&|d| d.converge = "-1".into()),
            "the convergence cannot be negative"
        );
    }

    #[test]
    fn alscal_defaults_add_nothing_and_negative_weights_need_indscal() {
        let data = snacks().dataset().unwrap().clone();
        assert_eq!(
            alscal().syntax(&data).unwrap(),
            "ALSCAL VARIABLES=chips nuts pretzel popcorn crackers."
        );
        let mut d = alscal();
        d.negative = true;
        assert_eq!(
            d.syntax(&data).unwrap(),
            "ALSCAL VARIABLES=chips nuts pretzel popcorn crackers."
        );
    }

    fn prefscal() -> Prefscal {
        Prefscal {
            variables: v(&["a", "b", "c"]),
            ..Default::default()
        }
    }

    #[test]
    fn prefscal_defaults_add_nothing() {
        let data = Dataset::new(0);
        assert_eq!(prefscal().syntax(&data).unwrap(), "PREFSCAL a b c.");
    }

    #[test]
    fn prefscal_writes_every_option() {
        let mut d = prefscal();
        d.layout = 1;
        d.layout_value = "12".into();
        d.similarities = true;
        d.start = 5;
        d.start_count = "4".into();
        d.condition = 1;
        d.transformation = 3;
        d.intercept = true;
        d.degree = "3".into();
        d.knots = "2".into();
        d.model = 1;
        d.lambda = "0.8".into();
        d.omega = "0".into();
        d.min_dimensions = "1".into();
        d.max_dimensions = "3".into();
        d.max_iterations = "100".into();
        d.min_stress = "0.001".into();
        d.print[3] = true;
        d.plot[7] = true;
        d.plot[4] = false;
        assert_eq!(
            d.syntax(&Dataset::new(0)).unwrap(),
            "PREFSCAL a b c\n  /INPUT=ROWS(12)\n  /PROXIMITIES=SIMILARITIES\n  /INITIAL=RANDOM(4)\n  \
             /CONDITION=MATRIX\n  /TRANSFORMATION=SPLINE(INTERCEPT ORDER=3 INKNOT=2)\n  \
             /MODEL=WEIGHTED\n  /PENALTY=LAMBDA(0.8) OMEGA(0)\n  \
             /CRITERIA=DIMENSIONS(1,3) MAXITER(100) MINSTRESS(0.001)\n  \
             /PRINT=HISTORY MEASURES COMMON WEIGHTS\n  /PLOT=COMMON SHEPARD."
        );
        let mut d = prefscal();
        d.print = [false; 11];
        d.layout = 4;
        d.layout_value = "person".into();
        d.untie = true;
        assert_eq!(
            d.syntax(&Dataset::new(0)).unwrap(),
            "PREFSCAL a b c\n  /INPUT=SOURCES(person)\n  /TRANSFORMATION=ORDINAL(UNTIE)\n  /PRINT=NONE."
        );
    }

    #[test]
    fn prefscal_checks_what_the_command_would_reject() {
        let data = Dataset::new(0);
        let err = |change: &dyn Fn(&mut Prefscal)| {
            let mut d = prefscal();
            change(&mut d);
            d.syntax(&data).unwrap_err()
        };
        assert!(err(&|d| d.variables.clear()).contains("preference table"));
        assert!(err(&|d| d.layout = 1).contains("layout"));
        assert!(err(&|d| d.layout = 3).contains("choose the variable"));
        assert!(err(&|d| d.lambda = "0".into()).contains("LAMBDA"));
        assert!(err(&|d| d.omega = "-1".into()).contains("OMEGA"));
        assert!(err(&|d| d.min_dimensions = "3".into()).contains("fewest"));
        assert!(
            err(&|d| {
                d.transformation = 3;
                d.degree = "4".into();
            })
            .contains("1 to 3")
        );
    }
}
