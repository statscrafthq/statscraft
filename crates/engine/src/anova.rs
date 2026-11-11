//! ANOVA: analysis of variance for factorial designs with covariates (the older command).
//!
//! Rules from the Command Syntax Reference (ANOVA) and the Algorithms manual (ANOVA):
//! - Each analysis list is fitted once per dependent variable, on the cases valid on every
//!   variable of the list, with factor values truncated to whole numbers inside their range and
//!   a positive weight. W is the sum of the weights. A factor's levels are the values in its
//!   range that occur.
//! - The model has the covariates, the main effects and the interactions up to MAXORDERS (five
//!   at most). An order of interactions whose columns are not of full rank (empty cells) is
//!   dropped with all higher orders, with a warning.
//! - Sums of squares are reductions R(A | B) = SSE(B) - SSE(A, B) of the least-squares fits,
//!   chosen by METHOD and COVARIATES as the manual's tables of sums of squares give them. With
//!   UNIQUE every effect is adjusted for all others: factor effects by the Type III hypothesis
//!   of the general linear model (equal to the regression approach with sum-to-zero coding),
//!   covariates by their removal from the full model.
//! - MCA deviations come from the main-effects model, with and without the covariates;
//!   STATISTICS=REG gives the covariate coefficients of the model in which the covariates are
//!   first entered.
//!
//! The fits reuse the general linear model of UNIANOVA (glm.rs): its design matrix of level
//! combinations, its sweep and its Type III hypotheses.

use crate::crosstabs::{Key, label};
use crate::descriptives::{case_weights, number};
use crate::dist::f_p;
use crate::glm::{Data, Effect, Fit, Part};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Anova, AnovaList, AnovaMethod, CovariateOrder};

const EPS: f64 = 1e-8;

/// The effects of the model: kind and the factors or covariate of each.
#[derive(Clone, Debug, PartialEq)]
enum Term {
    Covariate(usize),
    /// The factors of a main effect (one) or an interaction (several).
    Factors(Vec<usize>),
}

fn effect(term: &Term, factor_names: &[String], covariate_names: &[String]) -> Effect {
    let (label, parts, factors, covariates) = match term {
        Term::Covariate(c) => (
            covariate_names[*c].clone(),
            vec![Part::Covariate(*c)],
            Vec::new(),
            vec![*c],
        ),
        Term::Factors(fs) => (
            fs.iter()
                .map(|&f| factor_names[f].clone())
                .collect::<Vec<_>>()
                .join(" * "),
            fs.iter().map(|&f| Part::Factor(f)).collect(),
            fs.clone(),
            Vec::new(),
        ),
    };
    Effect {
        label,
        parts,
        factors,
        covariates,
        cells: Vec::new(),
        start: 0,
    }
}

/// Every combination of `k` of the first `n` factors, in the order of the list.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn grow(start: usize, n: usize, k: usize, now: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if now.len() == k {
            out.push(now.clone());
            return;
        }
        for i in start..n {
            now.push(i);
            grow(i + 1, n, k, now, out);
            now.pop();
        }
    }
    let mut out = Vec::new();
    grow(0, n, k, &mut Vec::new(), &mut out);
    out
}

/// A fitted model with the error sums of squares of its effects.
struct Model {
    fit: Fit,
    terms: Vec<Term>,
}

impl Model {
    /// The columns of the given terms (indices into `terms`), the intercept first.
    fn columns(&self, of: &[usize]) -> Vec<usize> {
        let mut cols: Vec<usize> = self.fit.effects[0].range().collect();
        for &t in of {
            cols.extend(self.fit.effects[t + 1].range());
        }
        cols
    }

    /// SSE and rank after fitting the intercept and the terms, in order.
    fn sse(&self, of: &[usize]) -> (f64, usize) {
        let (s, rank) = self.fit.error_after(&self.columns(of));
        (s[0][0], rank)
    }

    /// R(A | B) with its degrees of freedom.
    fn reduction(&self, a: &[usize], b: &[usize]) -> (f64, f64) {
        let (sse_b, rank_b) = self.sse(b);
        let both: Vec<usize> = b.iter().chain(a).copied().collect();
        let (sse_ab, rank_ab) = self.sse(&both);
        (sse_b - sse_ab, rank_ab as f64 - rank_b as f64)
    }

    /// The Type III sum of squares of the factor terms jointly.
    fn type3(&self, of: &[usize]) -> (f64, f64) {
        let l: Vec<Vec<f64>> = of
            .iter()
            .flat_map(|&t| self.fit.type34(t + 1, false))
            .collect();
        let (ssh, rank) = self.fit.hypothesis(&l, None);
        (ssh[0][0], rank as f64)
    }

    /// The coefficient of a covariate term.
    fn coefficient(&self, t: usize) -> f64 {
        self.fit.b[self.fit.effects[t + 1].start][0]
    }
}

/// One row of the ANOVA table.
struct Source {
    group: String,
    name: String,
    ss: f64,
    df: f64,
    coefficient: Option<f64>,
}

fn level_text(variable: &Variable, value: f64, currencies: &crate::Currencies) -> String {
    label(variable, &Key::Number(value), currencies)
}

impl Engine {
    pub(crate) fn anova(&mut self, command: &Anova) -> Result<Vec<Table>, Error> {
        let mut tables = Vec::new();
        for list in &command.lists {
            tables.extend(self.anova_list(command, list)?);
        }
        Ok(tables)
    }

    fn anova_list(&mut self, command: &Anova, list: &AnovaList) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let dependents = self.expand(&list.dependents)?;
        let mut factor_names = Vec::new();
        let mut ranges = Vec::new();
        for (items, min, max) in &list.factors {
            for name in self.expand(items)? {
                factor_names.push(name);
                ranges.push((*min, *max));
            }
        }
        let covariate_names = self.expand(&list.covariates)?;
        let get = |name: &String| data.variable(name);
        let dep_vars: Vec<Variable> = dependents.iter().map(get).collect::<Result<_, _>>()?;
        let factor_vars: Vec<Variable> = factor_names.iter().map(get).collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names.iter().map(get).collect::<Result<_, _>>()?;
        if let Some(v) = dep_vars
            .iter()
            .chain(&factor_vars)
            .chain(&cov_vars)
            .find(|v| v.is_string())
        {
            return Err(Error(format!(
                "ANOVA needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let weights = case_weights(data)?;
        let value = |var: &Variable, case: usize| -> Option<f64> {
            match (is_valid(var, case, command.include), var.values()) {
                (true, Values::Numeric(v)) => v.get(case),
                _ => None,
            }
        };
        // listwise over the whole list
        let mut rows = Vec::new();
        for case in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[case]);
            if w <= 0.0 {
                continue;
            }
            let y: Option<Vec<f64>> = dep_vars.iter().map(|v| value(v, case)).collect();
            let f: Option<Vec<f64>> = factor_vars
                .iter()
                .zip(&ranges)
                .map(|(v, (min, max))| {
                    value(v, case)
                        .map(f64::trunc)
                        .filter(|x| *x >= *min && *x <= *max)
                })
                .collect();
            let c: Option<Vec<f64>> = cov_vars.iter().map(|v| value(v, case)).collect();
            if let (Some(y), Some(f), Some(c)) = (y, f, c) {
                rows.push((case, y, f, c, w));
            }
        }
        let nf = factor_names.len();
        let mut levels: Vec<Vec<f64>> = vec![Vec::new(); nf];
        for (_, _, f, _, _) in &rows {
            for i in 0..nf {
                if !levels[i].contains(&f[i]) {
                    levels[i].push(f[i]);
                }
            }
        }
        levels.iter_mut().for_each(|l| l.sort_by(f64::total_cmp));
        for i in 0..nf {
            if levels[i].len() < 2 {
                return Err(Error(format!(
                    "ANOVA: the factor {} has {} level among the cases, but needs at least 2",
                    factor_names[i],
                    levels[i].len()
                )));
            }
            if command.method == AnovaMethod::Unique && levels[i][0] != ranges[i].0 {
                return Err(Error(format!(
                    "ANOVA: the lowest value {} of the factor {} has no cases; the regression approach needs it, so give the lowest value that occurs",
                    ranges[i].0, factor_names[i]
                )));
            }
        }
        let currencies = self.currencies();
        let level_labels: Vec<Vec<String>> = (0..nf)
            .map(|i| {
                levels[i]
                    .iter()
                    .map(|v| level_text(&factor_vars[i], *v, &currencies))
                    .collect()
            })
            .collect();
        let mut terms: Vec<Term> = (0..covariate_names.len()).map(Term::Covariate).collect();
        let top = command.max_order.min(nf);
        for k in 1..=top {
            terms.extend(combinations(nf, k).into_iter().map(Term::Factors));
        }
        let make = |terms: &[Term], dep: usize| -> Model {
            let d = Data {
                factors: factor_names.clone(),
                values: level_labels.clone(),
                labels: level_labels.clone(),
                labelled: false,
                covariates: covariate_names.clone(),
                case: rows.iter().map(|r| r.0).collect(),
                level: rows
                    .iter()
                    .map(|r| {
                        (0..nf)
                            .map(|i| {
                                levels[i]
                                    .iter()
                                    .position(|v| *v == r.2[i])
                                    .expect("a level")
                            })
                            .collect()
                    })
                    .collect(),
                cov: rows.iter().map(|r| r.3.clone()).collect(),
                y: rows.iter().map(|r| vec![r.1[dep]]).collect(),
                f: rows.iter().map(|r| r.4).collect(),
                g: vec![1.0; rows.len()],
            };
            let mut effects = vec![Effect {
                label: "Intercept".into(),
                parts: Vec::new(),
                factors: Vec::new(),
                covariates: Vec::new(),
                cells: Vec::new(),
                start: 0,
            }];
            effects.extend(
                terms
                    .iter()
                    .map(|t| effect(t, &factor_names, &covariate_names)),
            );
            let y = d.y.clone();
            Model {
                fit: Fit::new(&d, effects, &y, EPS),
                terms: terms.to_vec(),
            }
        };
        let order_of = |t: &Term| match t {
            Term::Covariate(_) => 0,
            Term::Factors(f) => f.len(),
        };
        let mut warnings = Vec::new();
        // an order of interactions that is not of full rank goes, with the orders above it
        if !dependents.is_empty() && !rows.is_empty() {
            let probe = make(&terms, 0);
            let mut keep_to = top;
            for k in 2..=top {
                let lower: Vec<usize> = (0..terms.len())
                    .filter(|&t| order_of(&terms[t]) < k)
                    .collect();
                let at: Vec<usize> = (0..terms.len())
                    .filter(|&t| order_of(&terms[t]) == k)
                    .collect();
                let expected: f64 = at
                    .iter()
                    .map(|&t| match &terms[t] {
                        Term::Factors(f) => {
                            f.iter().map(|&i| levels[i].len() as f64 - 1.0).product()
                        }
                        Term::Covariate(_) => 1.0,
                    })
                    .sum();
                let (_, df) = probe.reduction(&at, &lower);
                if df < expected - 0.5 {
                    keep_to = k - 1;
                    warnings.push(format!(
                        "ANOVA: the {k}-way interactions are not of full rank (empty cells), so the model keeps interactions up to order {keep_to}"
                    ));
                    break;
                }
            }
            terms.retain(|t| order_of(t) <= keep_to);
        }
        let n_cov = covariate_names.len();
        let cov_terms: Vec<usize> = (0..n_cov).collect();
        let main_terms: Vec<usize> = (0..terms.len())
            .filter(|&t| order_of(&terms[t]) == 1)
            .collect();
        let max_k = terms.iter().map(order_of).max().unwrap_or(1);
        let w_total: f64 = rows.iter().map(|r| r.4).sum();
        if w_total <= 1.0 {
            return Err(Error("ANOVA: there are no cases to analyze".into()));
        }
        let mut tables = Vec::new();
        let method_text = match (command.method, command.covariate_order) {
            (AnovaMethod::Unique, _) => "All effects entered simultaneously",
            (AnovaMethod::Experimental, CovariateOrder::First) => {
                "Experimental method: covariates entered first"
            }
            (AnovaMethod::Experimental, CovariateOrder::With) => {
                "Experimental method: covariates entered with the main effects"
            }
            (AnovaMethod::Experimental, CovariateOrder::After) => {
                "Experimental method: covariates entered after the main effects"
            }
            (AnovaMethod::Hierarchical, CovariateOrder::First) => {
                "Hierarchical method: covariates entered first"
            }
            (AnovaMethod::Hierarchical, CovariateOrder::With) => {
                "Hierarchical method: covariates entered with the main effects"
            }
            (AnovaMethod::Hierarchical, CovariateOrder::After) => {
                "Hierarchical method: covariates entered after the main effects"
            }
        };
        let unique = command.method == AnovaMethod::Unique;
        if unique && (command.mean || command.mca) {
            warnings.push(
                "ANOVA: MEAN and MCA are not available with METHOD=UNIQUE, so they are not shown"
                    .into(),
            );
        }
        for (dep, dep_name) in dependents.iter().enumerate() {
            let model = make(&terms, dep);
            let all: Vec<usize> = (0..terms.len()).collect();
            let (sst, _) = model.sse(&[]);
            let (sse, rank) = model.sse(&all);
            let df_model = rank as f64 - 1.0;
            let df_error = w_total - 1.0 - df_model;
            let without = |a: &[usize], from: &[usize]| -> Vec<usize> {
                from.iter().copied().filter(|t| !a.contains(t)).collect()
            };
            let cov_names = |t: usize| covariate_names[t].clone();
            let name_of = |t: usize| effect(&terms[t], &factor_names, &covariate_names).label;
            let mut sources: Vec<Source> = Vec::new();
            let mut push =
                |group: &str, name: String, (ss, df): (f64, f64), coefficient: Option<f64>| {
                    sources.push(Source {
                        group: group.to_string(),
                        name,
                        ss,
                        df,
                        coefficient,
                    });
                };
            // covariate coefficients where the covariates are first entered
            let coefficients: Vec<f64> = if command.reg && n_cov > 0 {
                let model_terms: Vec<Term> = match (unique, command.covariate_order) {
                    (true, _) => terms.clone(),
                    (false, CovariateOrder::First) => terms[..n_cov].to_vec(),
                    (false, _) => terms.iter().filter(|t| order_of(t) <= 1).cloned().collect(),
                };
                let m = make(&model_terms, dep);
                (0..n_cov).map(|t| m.coefficient(t)).collect()
            } else {
                Vec::new()
            };
            let coefficient = |t: usize| coefficients.get(t).copied();
            let interactions_of = |k: usize| -> Vec<usize> {
                (0..terms.len())
                    .filter(|&t| order_of(&terms[t]) == k && k > 1)
                    .collect()
            };
            let below = |k: usize| -> Vec<usize> {
                (0..terms.len())
                    .filter(|&t| order_of(&terms[t]) < k)
                    .collect()
            };
            let cm: Vec<usize> = cov_terms.iter().chain(&main_terms).copied().collect();
            // the combined rows of the covariates and main effects, and the rows within them
            let (cov_all, main_all) = match (unique, command.covariate_order) {
                (true, _) => (
                    model.reduction(&cov_terms, &without(&cov_terms, &all)),
                    model.type3(&main_terms),
                ),
                (false, CovariateOrder::First) => (
                    model.reduction(&cov_terms, &[]),
                    model.reduction(&main_terms, &cov_terms),
                ),
                (false, CovariateOrder::With) => {
                    let both = model.reduction(&cm, &[]);
                    (both, both)
                }
                (false, CovariateOrder::After) => (
                    model.reduction(&cov_terms, &main_terms),
                    model.reduction(&main_terms, &[]),
                ),
            };
            if n_cov > 0 {
                push("Covariates", "(Combined)".into(), cov_all, None);
                for t in 0..n_cov {
                    let others = without(&[t], &cov_terms);
                    let preceding: Vec<usize> = (0..t).collect();
                    let ss = match (command.method, command.covariate_order) {
                        (AnovaMethod::Unique, _) => model.reduction(&[t], &without(&[t], &all)),
                        (AnovaMethod::Experimental, CovariateOrder::First) => {
                            model.reduction(&[t], &others)
                        }
                        (AnovaMethod::Experimental, _) => model.reduction(
                            &[t],
                            &main_terms
                                .iter()
                                .chain(&others)
                                .copied()
                                .collect::<Vec<_>>(),
                        ),
                        (AnovaMethod::Hierarchical, CovariateOrder::First) => {
                            model.reduction(&[t], &preceding)
                        }
                        (AnovaMethod::Hierarchical, _) => model.reduction(
                            &[t],
                            &preceding
                                .iter()
                                .chain(&main_terms)
                                .copied()
                                .collect::<Vec<_>>(),
                        ),
                    };
                    push("", cov_names(t), ss, coefficient(t));
                }
            }
            push("Main Effects", "(Combined)".into(), main_all, None);
            for (k, &t) in main_terms.iter().enumerate() {
                let others = without(&[t], &main_terms);
                let preceding: Vec<usize> = main_terms[..k].to_vec();
                let with_cov =
                    |b: &[usize]| -> Vec<usize> { cov_terms.iter().chain(b).copied().collect() };
                let ss = match (command.method, command.covariate_order) {
                    (AnovaMethod::Unique, _) => model.type3(&[t]),
                    (AnovaMethod::Experimental, CovariateOrder::After) => {
                        model.reduction(&[t], &others)
                    }
                    (AnovaMethod::Experimental, _) => model.reduction(&[t], &with_cov(&others)),
                    (AnovaMethod::Hierarchical, CovariateOrder::First) => {
                        model.reduction(&[t], &with_cov(&preceding))
                    }
                    (AnovaMethod::Hierarchical, _) => model.reduction(&[t], &preceding),
                };
                push("", name_of(t), ss, None);
            }
            for k in 2..=max_k {
                let at = interactions_of(k);
                if at.is_empty() {
                    continue;
                }
                push(
                    &format!("{k}-Way Interactions"),
                    "(Combined)".into(),
                    model.reduction(&at, &below(k)),
                    None,
                );
                for &t in &at {
                    let ss = match unique {
                        true => model.type3(&[t]),
                        false => model.reduction(
                            &[t],
                            &below(k)
                                .into_iter()
                                .chain(without(&[t], &at))
                                .collect::<Vec<_>>(),
                        ),
                    };
                    push("", name_of(t), ss, None);
                }
            }
            push("Model", String::new(), (sst - sse, df_model), None);
            push("Residual", String::new(), (sse, df_error), None);
            push("Total", String::new(), (sst, w_total - 1.0), None);
            let ms_error = sse / df_error;
            let show_coefficients = !coefficients.is_empty();
            let source_rows: Vec<Vec<Cell>> = sources
                .iter()
                .map(|s| {
                    let tested = !matches!(s.group.as_str(), "Residual" | "Total");
                    let ms = s.ss / s.df;
                    let f = ms / ms_error;
                    let mut row = vec![
                        text(s.group.clone()),
                        text(s.name.clone()),
                        number(Some(s.ss)),
                        number(Some(s.df)),
                        number(Some(ms)),
                        if tested { number(Some(f)) } else { Cell::Empty },
                        if tested {
                            number(Some(f_p(f, s.df, df_error)))
                        } else {
                            Cell::Empty
                        },
                    ];
                    if s.group == "Total" {
                        row[4] = number(Some(s.ss / s.df));
                    }
                    if show_coefficients {
                        row.push(number(s.coefficient));
                    }
                    row
                })
                .collect();
            let mut columns = plain(&["", "", "Sum of Squares", "df", "Mean Square", "F", "Sig."]);
            if show_coefficients {
                columns.push((String::new(), "Raw Coefficient".into()));
            }
            let mut description = format!("{dep_name} by {}", factor_names.join(", "));
            if n_cov > 0 {
                description.push_str(&format!(" with {}", covariate_names.join(", ")));
            }
            tables.push(table(
                "ANOVA",
                2,
                columns,
                source_rows,
                Some(format!("{description}. {method_text}.")),
            ));
            let cells: Vec<Row> = rows
                .iter()
                .map(|r| Row {
                    y: r.1[dep],
                    level: (0..nf)
                        .map(|i| {
                            levels[i]
                                .iter()
                                .position(|v| *v == r.2[i])
                                .expect("a level")
                        })
                        .collect(),
                    w: r.4,
                })
                .collect();
            if !unique && command.mean {
                tables.push(cell_means(
                    dep_name,
                    &cells,
                    &factor_names,
                    &level_labels,
                    max_k,
                ));
            }
            if !unique && command.mca {
                // the main-effects models, without and with the covariates
                let mains: Vec<Term> = terms.iter().filter(|t| order_of(t) == 1).cloned().collect();
                let factors_model = make(&mains, dep);
                let with_cov: Vec<Term> =
                    terms.iter().filter(|t| order_of(t) <= 1).cloned().collect();
                let cov_model = (n_cov > 0).then(|| make(&with_cov, dep));
                let adjusted_shown = n_cov == 0
                    || command.method == AnovaMethod::Hierarchical
                    || command.covariate_order == CovariateOrder::After;
                tables.extend(mca_tables(
                    dep_name,
                    &cells,
                    &factor_names,
                    &level_labels,
                    sst,
                    adjusted_shown.then_some(&factors_model),
                    cov_model.as_ref(),
                ));
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }
}

/// One case of a dependent: its value, the level of every factor and its weight.
struct Row {
    y: f64,
    level: Vec<usize>,
    w: f64,
}

fn mean_of<'a>(rows: impl Iterator<Item = &'a Row>) -> (f64, f64) {
    let (mut sum, mut n) = (0.0, 0.0);
    for r in rows {
        sum += r.w * r.y;
        n += r.w;
    }
    (sum / n, n)
}

/// Means and counts of the dependent for every cell of each factor and combination of factors
/// up to the highest order of the model.
fn cell_means(
    dependent: &str,
    rows: &[Row],
    factors: &[String],
    labels: &[Vec<String>],
    max_order: usize,
) -> Table {
    let (grand, n) = mean_of(rows.iter());
    let mut out = vec![vec![
        text("Grand Mean"),
        Cell::Empty,
        number(Some(grand)),
        number(Some(n)),
    ]];
    for k in 1..=max_order {
        for combination in combinations(factors.len(), k) {
            let name = combination
                .iter()
                .map(|&f| factors[f].clone())
                .collect::<Vec<_>>()
                .join(" * ");
            let mut cells: Vec<Vec<usize>> = rows
                .iter()
                .map(|r| combination.iter().map(|&f| r.level[f]).collect())
                .collect();
            cells.sort();
            cells.dedup();
            for (k, cell) in cells.iter().enumerate() {
                let (mean, n) = mean_of(
                    rows.iter()
                        .filter(|r| combination.iter().zip(cell).all(|(&f, &l)| r.level[f] == l)),
                );
                let level = combination
                    .iter()
                    .zip(cell)
                    .map(|(&f, &l)| labels[f][l].clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(vec![
                    if k == 0 {
                        text(name.clone())
                    } else {
                        Cell::Empty
                    },
                    text(level),
                    number(Some(mean)),
                    number(Some(n)),
                ]);
            }
        }
    }
    table(
        "Cell Means",
        2,
        plain(&["", "", "Mean", "N"]),
        out,
        Some(format!("{dependent}: observed means of the cells")),
    )
}

/// The MCA deviations of every level of factor `i` from a main-effects model: the level
/// coefficients against the first level, centered on their weighted mean.
fn deviations(model: &Model, i: usize, rows: &[Row], k: usize) -> Vec<f64> {
    let t = model
        .terms
        .iter()
        .position(|t| *t == Term::Factors(vec![i]))
        .expect("a main effect");
    let e = &model.fit.effects[t + 1];
    let b = |l: usize| -> f64 {
        let at = e.cells.iter().position(|c| c[0] == l).expect("a level");
        model.fit.b[e.start + at][0]
    };
    let first = b(0);
    let raw: Vec<f64> = (0..k).map(|l| b(l) - first).collect();
    let (w, sum) = rows
        .iter()
        .fold((0.0, 0.0), |(w, s), r| (w + r.w, s + r.w * raw[r.level[i]]));
    raw.iter().map(|v| v - sum / w).collect()
}

fn mca_tables(
    dependent: &str,
    rows: &[Row],
    factors: &[String],
    labels: &[Vec<String>],
    sst: f64,
    factors_model: Option<&Model>,
    cov_model: Option<&Model>,
) -> Vec<Table> {
    let (grand, _) = mean_of(rows.iter());
    let mut table_rows = Vec::new();
    let mut summary = Vec::new();
    for (i, name) in factors.iter().enumerate() {
        let k = labels[i].len();
        let n: Vec<f64> = (0..k)
            .map(|l| rows.iter().filter(|r| r.level[i] == l).map(|r| r.w).sum())
            .collect();
        let unadjusted: Vec<f64> = (0..k)
            .map(|l| mean_of(rows.iter().filter(|r| r.level[i] == l)).0 - grand)
            .collect();
        let adjusted = factors_model.map(|m| deviations(m, i, rows, k));
        let covariates = cov_model.map(|m| deviations(m, i, rows, k));
        for l in 0..k {
            let mut row = vec![
                if l == 0 {
                    text(name.clone())
                } else {
                    Cell::Empty
                },
                text(labels[i][l].clone()),
                number(Some(n[l])),
                number(Some(grand + unadjusted[l])),
            ];
            for d in [&adjusted, &covariates].into_iter().flatten() {
                row.push(number(Some(grand + d[l])));
            }
            row.push(number(Some(unadjusted[l])));
            for d in [&adjusted, &covariates].into_iter().flatten() {
                row.push(number(Some(d[l])));
            }
            table_rows.push(row);
        }
        let share = |d: &[f64]| (n.iter().zip(d).map(|(n, d)| n * d * d).sum::<f64>() / sst).sqrt();
        let mut row = vec![text(name.clone()), number(Some(share(&unadjusted)))];
        for d in [&adjusted, &covariates].into_iter().flatten() {
            row.push(number(Some(share(d))));
        }
        summary.push(row);
    }
    let kinds: Vec<&str> = [
        Some("Unadjusted"),
        factors_model.map(|_| "Adjusted for Factors"),
        cov_model.map(|_| "Adjusted for Factors and Covariates"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut columns = plain(&["", "", "N"]);
    for group in ["Predicted Mean", "Deviation"] {
        columns.extend(kinds.iter().map(|k| (group.to_string(), k.to_string())));
    }
    let mut out = vec![table(
        "MCA",
        2,
        columns,
        table_rows,
        Some(format!("{dependent}: grand mean {grand}")),
    )];
    let mut columns = plain(&["", "Eta"]);
    columns.extend(
        kinds[1..]
            .iter()
            .map(|k| ("Beta".to_string(), k.to_string())),
    );
    out.push(table("Factor Summary", 1, columns, summary, None));
    let mut fit_rows = Vec::new();
    for (what, model) in [
        ("Factors", factors_model),
        ("Factors and Covariates", cov_model),
    ] {
        if let Some(m) = model {
            let all: Vec<usize> = (0..m.terms.len()).collect();
            let (sse, _) = m.sse(&all);
            let r2 = (sst - sse) / sst;
            fit_rows.push(vec![text(what), number(Some(r2.sqrt())), number(Some(r2))]);
        }
    }
    if !fit_rows.is_empty() {
        out.push(table(
            "Model Goodness of Fit",
            1,
            plain(&["", "R", "R Squared"]),
            fit_rows,
            None,
        ));
    }
    out
}
