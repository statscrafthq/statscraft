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
use statscraft_syntax::{Contrast, LogisticMethodKind, Removal};

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
/// not explained by them, relative to its own sum of squares, is at most `eps` is redundant, and
/// so is one whose share is only the rounding residue of a constant.
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
    let mut floor = vec![0.0; p];
    let share = residue(rows.len());
    for &i in &rows {
        for r in 0..p {
            floor[r] += share * c[i] * x[i][wanted[r]] * x[i][wanted[r]];
            for s in 0..p {
                m[r][s] += c[i] * (x[i][wanted[r]] - mean[r]) * (x[i][wanted[s]] - mean[s]);
            }
        }
    }
    kept_columns(m, wanted, eps, &floor)
}

/// The share of a column's uncentered sum of squares that centering a constant over `n` rows can
/// leave by rounding: the error of a mean grows with n, its square with n squared. Anything at
/// most this share is such residue.
pub(crate) fn residue(n: usize) -> f64 {
    let n = n as f64;
    (1e-30 * n * n).clamp(1e-24, 1e-14)
}

/// The columns of `wanted` that are not redundant, from the cross-products `sscp` of all
/// columns as the model sees their variation. A column whose part left over is at most `share`
/// of its `raw` sum of squares is only rounding residue, whatever its own part.
pub(crate) fn independent_in(
    sscp: &Matrix,
    raw: &[f64],
    share: f64,
    wanted: &[usize],
    eps: f64,
) -> Vec<usize> {
    let m = wanted
        .iter()
        .map(|&r| wanted.iter().map(|&s| sscp[r][s]).collect())
        .collect();
    let floor: Vec<f64> = wanted.iter().map(|&c| share * raw[c]).collect();
    kept_columns(m, wanted, eps, &floor)
}

fn kept_columns(mut m: Matrix, wanted: &[usize], eps: f64, floor: &[f64]) -> Vec<usize> {
    let diag: Vec<f64> = (0..wanted.len()).map(|r| m[r][r]).collect();
    let mut kept = Vec::new();
    for k in 0..wanted.len() {
        if m[k][k] <= eps * diag[k] || m[k][k] <= floor.get(k).copied().unwrap_or(0.0) {
            continue;
        }
        crate::matrix::sweep(&mut m, k);
        kept.push(wanted[k]);
    }
    kept
}

/// The Wald statistic of the coefficients `b` at `positions`, with covariance `cov`.
pub(crate) fn wald(b: &[f64], cov: &Matrix, positions: &[usize]) -> f64 {
    let sub: Matrix = positions
        .iter()
        .map(|&r| positions.iter().map(|&s| cov[r][s]).collect())
        .collect();
    let beta: Vec<f64> = positions.iter().map(|&r| b[r]).collect();
    match crate::matrix::inverse(&sub) {
        Some((inv, _)) => (0..beta.len())
            .map(|r| beta[r] * (0..beta.len()).map(|s| inv[r][s] * beta[s]).sum::<f64>())
            .sum(),
        None => f64::NAN,
    }
}

pub(crate) fn sorted(model: &[usize]) -> Vec<usize> {
    let mut v = model.to_vec();
    v.sort_unstable();
    v
}

/// The terms with every `a TO b` (a term of three words, the middle one TO, which no variable
/// can be named) replaced by the single variables it spans.
pub(crate) fn expand_terms(
    data: &Dataset,
    terms: &[Vec<String>],
) -> Result<Vec<Vec<String>>, Error> {
    let mut out = Vec::new();
    for term in terms {
        match term.as_slice() {
            [a, to, b] if to == "TO" => {
                let range = statscraft_syntax::VarItem::Range(a.clone(), b.clone());
                out.extend(
                    crate::expand_in(data, &[range])?
                        .into_iter()
                        .map(|n| vec![n]),
                );
            }
            _ => out.push(term.clone()),
        }
    }
    Ok(out)
}

/// The name of a METHOD in the block captions.
pub(crate) fn method_name(kind: LogisticMethodKind) -> String {
    let removal = |r: Removal| match r {
        Removal::Cond => "Conditional",
        Removal::Lr => "Likelihood Ratio",
        Removal::Wald => "Wald",
    };
    match kind {
        LogisticMethodKind::Enter => "Enter".into(),
        LogisticMethodKind::Fstep(r) => format!("Forward Stepwise ({})", removal(r)),
        LogisticMethodKind::Bstep(r) => format!("Backward Stepwise ({})", removal(r)),
    }
}

/// "Step n" on the first row of a step, empty below it.
pub(crate) fn step_cell(first: bool, n: usize) -> Cell {
    if first {
        text(format!("Step {n}"))
    } else {
        Cell::Empty
    }
}

/// A string value's first eight bytes (whole characters only), which tell its categories apart
/// in COXREG.
pub(crate) fn prefix8(text: &str) -> &str {
    let text = text.trim_end();
    let mut end = text.len().min(8);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A string value's first eight characters, which tell its categories apart in LOGISTIC
/// REGRESSION.
fn first8_chars(text: &str) -> &str {
    let text = text.trim_end();
    let end = text.char_indices().nth(8).map_or(text.len(), |(i, _)| i);
    &text[..end]
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
    /// Strings are told apart by their first eight bytes (COXREG); else by eight characters.
    pub eight_bytes: bool,
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
            eight_bytes: false,
        };
        out.codings = (0..out.variables.len()).map(|_| None).collect();
        Ok(out)
    }

    /// Two values are one category: equal numbers, or strings with the same first eight bytes or
    /// characters.
    fn same(&self, a: &Key, b: &Key) -> bool {
        match (a, b) {
            (Key::Text(a), Key::Text(b)) if self.eight_bytes => prefix8(a) == prefix8(b),
            (Key::Text(a), Key::Text(b)) => first8_chars(a) == first8_chars(b),
            _ => a.compare(b).is_eq(),
        }
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
            // values that share their start are one category, named by the lowest of them
            let mut merged: Vec<Key> = Vec::new();
            for level in levels {
                if !merged.iter().any(|m| self.same(m, &level)) {
                    merged.push(level);
                }
            }
            let levels = merged;
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
        levels.iter().position(|l| self.same(l, key))
    }

    /// For design column `j`: each variable of its term with its coding column (None: the
    /// value).
    pub(crate) fn column_parts(&self, j: usize) -> &[(usize, Option<usize>)] {
        &self.recipes[j]
    }

    /// The design row of a valid case; NaN where its category is not among the coded ones.
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
                        // a category outside the coded ones (a case not in the fit) has no row
                        (Some(c), Some((_, m))) => self.level(v, k).map_or(f64::NAN, |at| m[at][c]),
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
