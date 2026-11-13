//! The covariates of LOGISTIC REGRESSION and COXREG: terms of variables and interactions
//! (`a BY b`), the categorical variables coded by their contrasts, and the design columns the
//! terms give. A categorical variable with k categories becomes k - 1 columns; an interaction
//! with one is the product of the codings; the columns of a term enter and leave together.

use crate::contrast::coding;
use crate::crosstabs::{Key, categories};
use crate::descriptives::number;
use crate::matrix::Matrix;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::Shown;
use crate::{Cell, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::Contrast;

/// One term of the model: a variable or an interaction, with its design columns.
pub(crate) struct TermInfo {
    pub label: String,
    pub columns: Vec<usize>,
    pub categorical: bool,
}

pub(crate) fn same_term(a: &[String], b: &[String]) -> bool {
    let norm = |t: &[String]| {
        let mut v: Vec<String> = t.iter().map(|s| s.to_lowercase()).collect();
        v.sort();
        v
    };
    norm(a) == norm(b)
}

/// The distinct terms of the lists, in the order they first occur.
pub(crate) fn distinct_terms<'a>(lists: impl Iterator<Item = &'a Vec<String>>) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    for term in lists {
        if !out.iter().any(|t| same_term(t, term)) {
            out.push(term.clone());
        }
    }
    out
}

/// The terms of one METHOD: its list, or the `default` list when it names none, as positions
/// in `term_vars`.
pub(crate) fn listed_terms(
    list: &[Vec<String>],
    default: &[Vec<String>],
    term_vars: &[Vec<String>],
) -> Vec<usize> {
    let list = if list.is_empty() { default } else { list };
    let mut out: Vec<usize> = list
        .iter()
        .filter_map(|t| term_vars.iter().position(|u| same_term(u, t)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The columns of `wanted` that are not linear combinations of the ones before them (and of
/// the constant when `centered`), over the rows with a positive weight: a column whose share
/// not explained by them, relative to its own sum of squares, is at most `eps` is redundant.
pub(crate) fn independent(
    x: &[Vec<f64>],
    c: &[f64],
    wanted: &[usize],
    centered: bool,
    eps: f64,
) -> Vec<usize> {
    let p = wanted.len();
    let rows: Vec<usize> = (0..x.len()).filter(|&i| c[i] > 0.0).collect();
    let w: f64 = rows.iter().map(|&i| c[i]).sum();
    let mean: Vec<f64> = wanted
        .iter()
        .map(|&j| {
            if centered {
                rows.iter().map(|&i| c[i] * x[i][j]).sum::<f64>() / w
            } else {
                0.0
            }
        })
        .collect();
    let mut m = vec![vec![0.0; p]; p];
    for &i in &rows {
        for r in 0..p {
            for s in 0..p {
                m[r][s] += c[i] * (x[i][wanted[r]] - mean[r]) * (x[i][wanted[s]] - mean[s]);
            }
        }
    }
    let diag: Vec<f64> = (0..p).map(|r| m[r][r]).collect();
    let mut kept = Vec::new();
    for k in 0..p {
        if m[k][k] <= eps * diag[k] || m[k][k] <= 0.0 {
            continue;
        }
        crate::matrix::sweep(&mut m, k);
        kept.push(wanted[k]);
    }
    kept
}

pub(crate) fn clone_key(key: &Key) -> Key {
    match key {
        Key::Number(n) => Key::Number(*n),
        Key::Text(t) => Key::Text(t.clone()),
    }
}

/// The variables of the terms, their values, and once coded the design columns.
pub(crate) struct Covariates<'a> {
    pub variables: Vec<Variable<'a>>,
    /// The category of every case for each variable, None where it is missing.
    pub keys: Vec<Vec<Option<Key>>>,
    categorical: Vec<bool>,
    /// The categories of each categorical variable and their coding (a row per category).
    pub codings: Vec<Option<(Vec<Key>, Matrix)>>,
    /// The name of every design column, such as `plan(1)` or `age by plan(2)`.
    pub names: Vec<String>,
    pub terms: Vec<TermInfo>,
    /// Per column: for each variable of its term, which coding column (None: the value).
    recipes: Vec<Vec<(usize, Option<usize>)>>,
}

impl<'a> Covariates<'a> {
    /// The variables of `term_vars`. `roles` names the other variables of the command, which
    /// cannot be covariates; `command` names the command in messages. String variables and
    /// those on `categorical` are categorical.
    pub(crate) fn resolve(
        data: &'a Dataset,
        term_vars: &[Vec<String>],
        roles: &[(&str, &str)],
        categorical: &[String],
        contrasts: &[(String, Contrast)],
        include: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Covariates<'a>, Error> {
        let mut variables: Vec<Variable> = Vec::new();
        for name in term_vars.iter().flatten() {
            let v = data.variable(name)?;
            if let Some((_, message)) = roles
                .iter()
                .find(|(role, _)| v.name().eq_ignore_ascii_case(role))
            {
                return Err(Error(format!("{} {message}", v.name())));
            }
            if !variables.iter().any(|w| w.name() == v.name()) {
                variables.push(v);
            }
        }
        let is_categorical: Vec<bool> = variables
            .iter()
            .map(|v| v.is_string() || categorical.iter().any(|c| c.eq_ignore_ascii_case(v.name())))
            .collect();
        for name in categorical {
            if !variables
                .iter()
                .any(|v| v.name().eq_ignore_ascii_case(name))
            {
                warnings.push(format!(
                    "{name} is on CATEGORICAL but in no term of the model; it is ignored"
                ));
            }
        }
        for (name, _) in contrasts {
            if !variables
                .iter()
                .zip(&is_categorical)
                .any(|(v, &c)| v.name().eq_ignore_ascii_case(name) && c)
            {
                return Err(Error(format!(
                    "CONTRAST names {name}, which is not a categorical variable of the model"
                )));
            }
        }
        let keys = variables.iter().map(|v| categories(v, include)).collect();
        let mut out = Covariates {
            variables,
            keys,
            categorical: is_categorical,
            codings: Vec::new(),
            names: Vec::new(),
            terms: Vec::new(),
            recipes: Vec::new(),
        };
        out.codings = (0..out.variables.len()).map(|_| None).collect();
        Ok(out)
    }

    /// Case `k` is valid on every variable.
    pub(crate) fn valid(&self, k: usize) -> bool {
        self.keys.iter().all(|col| col[k].is_some())
    }

    pub(crate) fn index(&self, name: &str) -> usize {
        self.variables
            .iter()
            .position(|v| v.name().eq_ignore_ascii_case(name))
            .expect("a variable of the terms")
    }

    /// Codes the categorical variables over the cases with `used` set (their categories in
    /// ascending order), with `default` where no CONTRAST names the variable, and lays out the
    /// design columns of `term_vars`.
    pub(crate) fn code(
        &mut self,
        term_vars: &[Vec<String>],
        used: &[bool],
        contrasts: &[(String, Contrast)],
        default: &Contrast,
        shown: &Shown,
    ) -> Result<(), Error> {
        for (at, v) in self.variables.iter().enumerate() {
            if !self.categorical[at] {
                continue;
            }
            let mut levels: Vec<Key> = Vec::new();
            for k in (0..used.len()).filter(|&k| used[k]) {
                let key = self.keys[at][k].as_ref().expect("valid");
                if !levels.iter().any(|l| l.compare(key).is_eq()) {
                    levels.push(clone_key(key));
                }
            }
            levels.sort_by(|a, b| a.compare(b));
            if levels.len() < 2 {
                return Err(Error(format!(
                    "the categorical variable {} has fewer than two categories among the cases used",
                    v.name()
                )));
            }
            let kind = contrasts
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(v.name()))
                .map_or(default, |(_, c)| c);
            let matrix = coding(kind, levels.len())
                .map_err(|e| Error(format!("CONTRAST({}): {e}", v.name())))?;
            self.codings[at] = Some((levels, matrix));
        }
        for term in term_vars {
            let parts: Vec<usize> = term.iter().map(|n| self.index(n)).collect();
            let mut combos: Vec<Vec<(usize, Option<usize>)>> = vec![Vec::new()];
            for &v in &parts {
                let choices: Vec<Option<usize>> = match &self.codings[v] {
                    Some((_, m)) => (0..m[0].len()).map(Some).collect(),
                    None => vec![None],
                };
                combos = combos
                    .into_iter()
                    .flat_map(|prefix| {
                        choices.iter().map(move |&c| {
                            let mut next = prefix.clone();
                            next.push((v, c));
                            next
                        })
                    })
                    .collect();
            }
            let mut columns = Vec::new();
            for combo in combos {
                let name = combo
                    .iter()
                    .map(|&(v, c)| match c {
                        Some(c) => format!("{}({})", shown.variable(&self.variables[v]), c + 1),
                        None => shown.variable(&self.variables[v]),
                    })
                    .collect::<Vec<_>>()
                    .join(" by ");
                columns.push(self.names.len());
                self.names.push(name);
                self.recipes.push(combo);
            }
            let categorical = parts.iter().any(|&v| self.codings[v].is_some());
            self.terms.push(TermInfo {
                label: parts
                    .iter()
                    .map(|&v| shown.variable(&self.variables[v]))
                    .collect::<Vec<_>>()
                    .join(" by "),
                columns,
                categorical,
            });
        }
        Ok(())
    }

    /// The position of case `k`'s category of variable `v` among its coded categories.
    pub(crate) fn level(&self, v: usize, k: usize) -> Option<usize> {
        let (levels, _) = self.codings[v].as_ref()?;
        let key = self.keys[v][k].as_ref()?;
        levels.iter().position(|l| l.compare(key).is_eq())
    }

    /// For design column `j`: each variable of its term with its coding column (None: the
    /// value).
    pub(crate) fn column_parts(&self, j: usize) -> &[(usize, Option<usize>)] {
        &self.recipes[j]
    }

    /// The design row of a valid case.
    pub(crate) fn row(&self, k: usize) -> Vec<f64> {
        let numeric = |v: &Variable| match v.values() {
            Values::Numeric(values) => values.get(k).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        self.recipes
            .iter()
            .map(|recipe| {
                recipe
                    .iter()
                    .map(|&(v, c)| match (c, &self.codings[v]) {
                        (Some(c), Some((_, m))) => {
                            m[self.level(v, k).expect("a category of the analysis")][c]
                        }
                        _ => numeric(&self.variables[v]),
                    })
                    .product()
            })
            .collect()
    }

    /// The table of the codings with the frequency of every category, or None without
    /// categorical variables. `frequency(v, level)` gives the weighted count.
    pub(crate) fn codings_table(
        &self,
        title: &str,
        shown: &Shown,
        frequency: impl Fn(usize, usize) -> f64,
    ) -> Option<Table> {
        let width = self
            .codings
            .iter()
            .flatten()
            .map(|(_, m)| m[0].len())
            .max()?;
        let mut columns = plain(&["", "", "Frequency"]);
        columns.extend((1..=width).map(|j| ("Parameter coding".to_string(), format!("({j})"))));
        let mut rows = Vec::new();
        for (v, coded) in self.codings.iter().enumerate() {
            let Some((levels, m)) = coded else { continue };
            for (at, level) in levels.iter().enumerate() {
                let mut row = vec![
                    if at == 0 {
                        text(shown.variable(&self.variables[v]))
                    } else {
                        Cell::Empty
                    },
                    text(shown.value(&self.variables[v], level)),
                    Cell::Number(frequency(v, at)),
                ];
                row.extend((0..width).map(|j| number(m[at].get(j).copied())));
                rows.push(row);
            }
        }
        Some(table(title, 2, columns, rows, None))
    }
}
