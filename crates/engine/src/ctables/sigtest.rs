//! Significance tests of CTABLES: the chi-square test of independence (`/SIGTEST`) and the
//! pairwise comparisons of column proportions and column means (`/COMPARETEST`).
//!
//! The rules, in our words, from the command reference (CTABLES, SIGTEST and COMPARETEST, and
//! "Weights and Rounding") and the CTABLES chapter of the algorithms reference:
//! - Every test runs inside an innermost subtable: the innermost categorical variable of the rows
//!   against the innermost categorical variable of the columns, once for every combination of
//!   categories of the outer variables of both axes (the total of an outer variable is a subtable
//!   too). Totals of the two tested variables take no part in the test.
//! - Subtotals (SIGTEST and COMPARETEST CATEGORIES, and the algorithms reference: tests use the
//!   visible categories, not totals or subtotals): a test compares the shown categories, and an
//!   HSUBTOTAL takes the place of the categories it hides. CATEGORIES=SUBTOTALS puts every
//!   subtotal in place of its categories; categories outside any subtotal stay as they are.
//! - Chi-square: rows and columns without cases are structural zeros and are left out; with
//!   fewer than two of either there is no test. df = (R - 1)(C - 1); significant when p < alpha.
//! - Column proportions: each row of a subtable is tested on its own. A column takes part when its
//!   proportion is strictly between 0 and 1 and its total weighs at least 2. Every pair is
//!   compared with a z-test on the pooled proportion.
//! - Column means: a scale variable in the rows, summarized by MEAN, against a categorical variable
//!   in the columns. A category takes part when it weighs at least 2. Every pair is compared with
//!   a t-test whose variance is pooled from all taking part (MEANSVARIANCE=ALLCATS) or from the
//!   two compared (TESTEDCATS); a pair or a subtable whose pooled variance is 0 is not tested.
//! - The p-values of one row of a subtable are adjusted together: Bonferroni multiplies them by the
//!   number of comparisons (at most 1) and is significant below alpha; Benjamini-Hochberg is the
//!   step-up false discovery rate and is significant at alpha or below; NONE leaves them.
//! - Weights of the WEIGHT command are summed as they are and the sums rounded to whole numbers:
//!   the cell counts of the chi-square test, the column totals of the proportions test (the
//!   proportions themselves are not rounded), and the category weights of the means test (means
//!   and variances are not rounded).
//! - Effective base weights (/WEIGHT; algorithms reference, the chi-square's "Use of weights" and
//!   the column tests "With Effective Base Weighting"): nothing is rounded. The chi-square is
//!   multiplied by W / q, the subtable's weight over its sum of squared weights. The column tests
//!   are t-tests on the effective bases e = w^2 / q: a column proportion's base is the sum of
//!   the bases of its cells (of the column total when the rows are a set), a mean's variance is
//!   its weighted sum of squared deviations over w - w / e, and df is e_j + e_k - 2 (pooled from
//!   all: the sum of the bases minus their number). The cases two categories of a set share
//!   have the base 0.5 w_jk (e_j / w_j + e_k / w_k), taken off as with plain weights. Two
//!   categories are compared when both have a case and one of them has two or more. Where the
//!   manual is silent: the reference gives the chi-square factor both as E_SS and as W / q; we
//!   use W / q, which scales the statistic to the effective sample size.
//! - Every column of the table gets a letter. For a significant pair the letter of the column
//!   with the smaller proportion or mean goes into the column with the larger one: uppercase when
//!   significant at the smaller of two ALPHA levels (or at the only one), lowercase at the larger.
//! - Multiple response sets (algorithms reference, the set variants of each test; SIGTEST and
//!   COMPARETEST INCLUDEMRSETS): a case can be in several categories of a set, so margins and
//!   column totals count each case once. Chi-square divides each term by 1 minus the share of the
//!   set's category in the subtable (by both shares with sets on both sides), and a set side keeps
//!   all its categories in df: (R - 1)C, R(C - 1) or RC. Column proportions and means with a set
//!   in the columns take the rounded weight two columns share, w_jk, off the variance factor
//!   1/w_j + 1/w_k as 2 w_jk / (w_j w_k); the means test also takes it off df and always pools
//!   the variance from the two categories compared. INCLUDEMRSETS=NO leaves sets out of the
//!   tests; so does COUNTDUPLICATES=YES for a category set (the manual names category sets).
//!   Where the manual is silent: a set category every case has, whose share is 1, is left out of
//!   the chi-square like an empty one, and two columns whose shared weight leaves no variance
//!   are not compared.
//! - MERGE=YES (command reference COMPARETEST, Custom Tables manual "Merging Significance Results
//!   into the Main Table" and the APA-style example) puts the letters into the main table. Our
//!   table type has one value per cell, so each compared category gets a letters column after its
//!   summary columns. SIMPLE heads it with the category's key, lowercase as in the manual's
//!   figure, and lists the keys of the columns it is larger than. APA, the default, uses the
//!   smaller ALPHA only and gives letters such that two columns share one exactly when they do not
//!   differ significantly; the manual does not say how the letters are found, so we use the
//!   insert-and-absorb construction of a compact letter display.

use super::{
    CategoryList, Entry, Layout, MARGIN, Section, Slot, Tally, Term, TermVar, combinations,
    effective_base, list_of,
};
use crate::dist::{chi_square_p, normal_p, t_p};
use crate::{Cell, Error, Table};
use statscraft_syntax::{
    Adjust, CompareKind, CompareStyle, Comparetest, CtablesTable, Estimate, PercentBase, PercentOf,
    Sigtest, Statistic,
};
use std::collections::HashMap;

// ---- tests on aggregated cells ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChiSquare {
    pub value: f64,
    pub df: f64,
    pub p: f64,
}

/// Pearson's chi-square test of independence of a table of counts. Rows and columns without
/// cases are left out; None when fewer than two rows or two columns remain.
pub(crate) fn pearson(counts: &[Vec<f64>]) -> Option<ChiSquare> {
    let rows: Vec<f64> = counts.iter().map(|r| r.iter().sum()).collect();
    let columns: Vec<f64> = (0..counts.first()?.len())
        .map(|j| counts.iter().map(|r| r[j]).sum())
        .collect();
    let total = rows.iter().sum();
    pearson_margins(counts, &rows, &columns, total, (false, false))
}

/// Pearson's chi-square test from the cell counts, the row and column margins and the total
/// count of a subtable. `sets` tells which side is a multiple response set, whose margins count
/// each case once although a case can be in several of its categories. A set side divides each
/// term by 1 minus the share of the case's category in the total and keeps all its categories in
/// the degrees of freedom: (R - 1)C, R(C - 1) or RC. Rows and columns without cases are left out,
/// and so is a category of a set that every case has, which is not free to vary; None when
/// fewer than two rows or two columns remain.
pub(crate) fn pearson_margins(
    counts: &[Vec<f64>],
    rows: &[f64],
    columns: &[f64],
    total: f64,
    sets: (bool, bool),
) -> Option<ChiSquare> {
    let kept = |margins: &[f64], set: bool| -> Vec<usize> {
        (0..margins.len())
            .filter(|&i| margins[i] > 0.0 && !(set && margins[i] >= total))
            .collect()
    };
    let (kept_rows, kept_columns) = (kept(rows, sets.0), kept(columns, sets.1));
    if kept_rows.len() < 2 || kept_columns.len() < 2 {
        return None;
    }
    let share = |margin: f64, set: bool| if set { 1.0 - margin / total } else { 1.0 };
    let mut value = 0.0;
    for &i in &kept_rows {
        for &j in &kept_columns {
            let expected = rows[i] * columns[j] / total;
            value += (counts[i][j] - expected).powi(2)
                / (expected * share(rows[i], sets.0) * share(columns[j], sets.1));
        }
    }
    let free = |n: usize, set: bool| if set { n } else { n - 1 };
    let df = (free(kept_rows.len(), sets.0) * free(kept_columns.len(), sets.1)) as f64;
    Some(ChiSquare {
        value,
        df,
        p: chi_square_p(value, df),
    })
}

/// A comparison of two columns, by their positions, with its unadjusted p-value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pair {
    pub larger: usize,
    pub smaller: usize,
    pub p: f64,
}

fn pair(j: usize, k: usize, value_j: f64, value_k: f64, p: f64) -> Pair {
    let (larger, smaller) = if value_j >= value_k { (j, k) } else { (k, j) };
    Pair { larger, smaller, p }
}

/// The weight of the cases two columns of a set share, by column positions; 0 for a
/// categorical variable, whose columns share no case.
pub(crate) type Shared<'s> = &'s dyn Fn(usize, usize) -> f64;

/// Column proportions test of one row: `counts[j]` is the weighted count of the row in column j,
/// `totals[j]` the weighted total of column j. Returns the pairs and which columns took part.
/// Two columns of a set that share cases are not independent: the shared weight, rounded,
/// takes 2 n_jk / (n_j n_k) off the variance factor 1/n_j + 1/n_k.
pub(crate) fn proportions(
    counts: &[f64],
    totals: &[f64],
    shared: Shared,
) -> (Vec<Pair>, Vec<bool>) {
    let share = |j: usize| counts[j] / totals[j];
    let tested: Vec<bool> = (0..counts.len())
        .map(|j| totals[j] >= 2.0 && share(j) > 0.0 && share(j) < 1.0)
        .collect();
    let mut pairs = Vec::new();
    for j in 0..counts.len() {
        for k in j + 1..counts.len() {
            if !(tested[j] && tested[k]) {
                continue;
            }
            let (nj, nk) = (totals[j].round(), totals[k].round());
            let factor = 1.0 / nj + 1.0 / nk - 2.0 * shared(j, k).round() / (nj * nk);
            // columns of a set that hold the same cases cannot be compared
            if factor <= 0.0 {
                continue;
            }
            let pooled = (nj * share(j) + nk * share(k)) / (nj + nk);
            let z = (share(j) - share(k)) / (pooled * (1.0 - pooled) * factor).sqrt();
            pairs.push(pair(j, k, share(j), share(k), normal_p(z)));
        }
    }
    (pairs, tested)
}

/// The weighted size, mean and variance (divided by the weight minus 1) of one category, with
/// what effective base weights need: the weighted sum of squared deviations, the sum of squared
/// weights and the number of cases.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Group {
    pub weight: f64,
    pub mean: f64,
    pub variance: f64,
    pub deviations: f64,
    pub squares: f64,
    pub cases: f64,
}

/// The group of the valid (value, weight) pairs of a category.
pub(crate) fn group(values: &[(f64, f64)]) -> Group {
    let weight: f64 = values.iter().map(|(_, w)| w).sum();
    let mean = values.iter().map(|(x, w)| x * w).sum::<f64>() / weight;
    let deviations: f64 = values.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
    Group {
        weight,
        mean,
        variance: deviations / (weight - 1.0),
        deviations,
        squares: values.iter().map(|(_, w)| w * w).sum(),
        cases: values.len() as f64,
    }
}

/// Which categories take part in a test with effective base weights, by their numbers of
/// cases: a pair is compared when both have a case and one of them two or more.
fn compared_by_cases(cases: &[f64]) -> (Vec<bool>, impl Fn(usize, usize) -> bool + '_) {
    let tested = (0..cases.len())
        .map(|j| {
            cases[j] >= 2.0
                || (cases[j] >= 1.0 && (0..cases.len()).any(|k| k != j && cases[k] >= 2.0))
        })
        .collect();
    let pair = move |j: usize, k: usize| {
        cases[j] >= 1.0 && cases[k] >= 1.0 && (cases[j] >= 2.0 || cases[k] >= 2.0)
    };
    (tested, pair)
}

/// One column of a proportions test with effective base weights (/WEIGHT): the weight of the
/// row in the column, the column's weight, its effective base and its number of cases.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BaseColumn {
    pub count: f64,
    pub total: f64,
    pub base: f64,
    pub cases: f64,
}

/// Column proportions test of one row with effective base weights: a t-test with the effective
/// bases in place of the column sizes and df = e_j + e_k - 2; nothing is rounded. `shared` gives
/// the effective base of the cases two columns of a set share, which comes off the variance
/// factor as 2 e_jk / (e_j e_k) and off df.
pub(crate) fn proportions_effective(
    columns: &[BaseColumn],
    shared: Shared,
) -> (Vec<Pair>, Vec<bool>) {
    let cases: Vec<f64> = columns.iter().map(|c| c.cases).collect();
    let (tested, compared) = compared_by_cases(&cases);
    let share = |j: usize| columns[j].count / columns[j].total;
    let mut pairs = Vec::new();
    for j in 0..columns.len() {
        for k in j + 1..columns.len() {
            if !compared(j, k) {
                continue;
            }
            let (a, b) = (&columns[j], &columns[k]);
            let pooled = (a.count + b.count) / (a.total + b.total);
            let both = shared(j, k);
            let factor = 1.0 / a.base + 1.0 / b.base - 2.0 * both / (a.base * b.base);
            let variance = pooled * (1.0 - pooled) * factor;
            let df = a.base + b.base - both - 2.0;
            // a pooled proportion of 0 or 1, or columns that hold the same cases, leave
            // nothing to test
            if !(variance > 0.0 && df > 0.0) {
                continue;
            }
            let t = (share(j) - share(k)) / variance.sqrt();
            pairs.push(pair(j, k, share(j), share(k), t_p(t, df)));
        }
    }
    (pairs, tested)
}

/// Column means test of one subtable with effective base weights (/WEIGHT): every category's
/// variance is adjusted to its effective base e = w^2 / q, the pooled variance is the sum of the
/// squared deviations over the sum of w - w / e (of all categories with two cases or more, or of
/// the two compared), the t statistic uses 1 / e_i + 1 / e_j, and df is e_i + e_j - 2 (pooled
/// from all: the sum of their e minus their number). Nothing is rounded. `shared` gives the
/// weight two categories of a set share; its effective base 0.5 w_ij (e_i / w_i + e_j / w_j)
/// comes off the factor and off df, and sets always pool from the two compared.
pub(crate) fn means_effective(
    groups: &[Group],
    pool_all: bool,
    shared: Option<Shared>,
) -> (Vec<Pair>, Vec<bool>) {
    let pool_all = pool_all && shared.is_none();
    let cases: Vec<f64> = groups.iter().map(|g| g.cases).collect();
    let (tested, compared) = compared_by_cases(&cases);
    let e = |g: &Group| effective_base(g.weight, g.squares);
    let room = |g: &Group| g.weight - g.weight / e(g);
    let full: Vec<&Group> = groups.iter().filter(|g| g.cases >= 2.0).collect();
    let pooled =
        full.iter().map(|g| g.deviations).sum::<f64>() / full.iter().map(|g| room(g)).sum::<f64>();
    let df_all = full.iter().map(|g| e(g)).sum::<f64>() - full.len() as f64;
    let mut pairs = Vec::new();
    for i in 0..groups.len() {
        for j in i + 1..groups.len() {
            if !compared(i, j) {
                continue;
            }
            let (gi, gj) = (&groups[i], &groups[j]);
            let both = shared.map_or(0.0, |f| {
                0.5 * f(i, j) * (e(gi) / gi.weight + e(gj) / gj.weight)
            });
            let (variance, df) = if pool_all {
                (pooled, df_all)
            } else {
                let variance = (gi.deviations + gj.deviations) / (room(gi) + room(gj));
                (variance, e(gi) + e(gj) - both - 2.0)
            };
            let factor = 1.0 / e(gi) + 1.0 / e(gj) - 2.0 * both / (e(gi) * e(gj));
            if !(variance > 0.0 && factor > 0.0 && df > 0.0) {
                continue;
            }
            let t = (gi.mean - gj.mean) / (variance * factor).sqrt();
            pairs.push(pair(i, j, gi.mean, gj.mean, t_p(t, df)));
        }
    }
    (pairs, tested)
}

/// Column means test of one subtable. `pool_all` pools the variance from every category taking
/// part, else from the two compared. Returns the pairs and which categories took part. The
/// categories of a set (`shared` is Some) always pool from the two compared; the weight two of
/// them share, rounded, takes 2 w_ij / (w_i w_j) off the factor 1/w_i + 1/w_j and w_ij off the
/// degrees of freedom.
pub(crate) fn means(
    groups: &[Group],
    pool_all: bool,
    shared: Option<Shared>,
) -> (Vec<Pair>, Vec<bool>) {
    let pool_all = pool_all && shared.is_none();
    let shared = |i: usize, j: usize| shared.map_or(0.0, |f| f(i, j).round());
    let tested: Vec<bool> = groups.iter().map(|g| g.weight >= 2.0).collect();
    let n = |g: &Group| g.weight.round();
    let chosen: Vec<usize> = (0..groups.len()).filter(|&i| tested[i]).collect();
    let total: f64 = chosen.iter().map(|&i| n(&groups[i])).sum();
    let df_all = total - chosen.len() as f64;
    let pooled = chosen
        .iter()
        .map(|&i| (n(&groups[i]) - 1.0) * groups[i].variance)
        .sum::<f64>()
        / df_all;
    let mut pairs = Vec::new();
    for (a, &i) in chosen.iter().enumerate() {
        for &j in &chosen[a + 1..] {
            let (gi, gj) = (&groups[i], &groups[j]);
            let both = shared(i, j);
            let (variance, df) = if pool_all {
                (pooled, df_all)
            } else {
                let df = n(gi) + n(gj) - 2.0;
                let joint = (n(gi) - 1.0) * gi.variance + (n(gj) - 1.0) * gj.variance;
                (joint / df, df - both)
            };
            let factor = 1.0 / n(gi) + 1.0 / n(gj) - 2.0 * both / (n(gi) * n(gj));
            // without spread, or between categories of a set that hold the same cases, the test
            // is undefined, and the comparison is not counted
            if variance.is_nan() || variance <= 0.0 || factor <= 0.0 || df <= 0.0 {
                continue;
            }
            let t = (gi.mean - gj.mean) / (variance * factor).sqrt();
            pairs.push(pair(i, j, gi.mean, gj.mean, t_p(t, df)));
        }
    }
    (pairs, tested)
}

/// The p-values adjusted for the number of comparisons among them.
pub(crate) fn adjust(p: &[f64], method: Adjust) -> Vec<f64> {
    let m = p.len() as f64;
    match method {
        Adjust::None => p.to_vec(),
        Adjust::Bonferroni => p.iter().map(|p| (p * m).min(1.0)).collect(),
        Adjust::BenjaminiHochberg => {
            // from the largest p down: the smallest of m / rank * p over this rank and above
            let mut order: Vec<usize> = (0..p.len()).collect();
            order.sort_by(|&a, &b| p[a].total_cmp(&p[b]));
            let mut adjusted = vec![0.0; p.len()];
            let mut running = f64::INFINITY;
            for (rank, &i) in order.iter().enumerate().rev() {
                running = running.min(m / (rank as f64 + 1.0) * p[i]);
                adjusted[i] = running;
            }
            adjusted
        }
    }
}

/// Some(true) when an adjusted p-value is significant at the smaller level, Some(false) at the
/// larger one only, None when at neither.
pub(crate) fn significance(p: f64, alpha: &[f64], method: Adjust) -> Option<bool> {
    let below = |a: f64| match method {
        Adjust::BenjaminiHochberg => p <= a,
        _ => p < a,
    };
    if below(alpha[0]) {
        Some(true)
    } else if alpha.get(1).is_some_and(|a| below(*a)) {
        Some(false)
    } else {
        None
    }
}

/// The key of the column at `index`: A to Z, then AA, AB and so on.
pub(crate) fn letter(index: usize) -> String {
    let mut n = index + 1;
    let mut text = Vec::new();
    while n > 0 {
        n -= 1;
        text.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    text.reverse();
    String::from_utf8(text).expect("ASCII letters")
}

// ---- the result tables ----

/// Every combination of categories that tests take part in: computed categories and hidden
/// ones do not.
fn tested_combinations(lists: &[&CategoryList]) -> Vec<Vec<usize>> {
    let mut all = combinations(lists);
    all.retain(|combo| {
        combo.iter().zip(lists).all(|(&k, list)| {
            let category = &list.categories[k];
            category.computed.is_none() && !category.hidden
        })
    });
    all
}

/// The categories of the categorical variables of a term, outermost first.
fn category_lists<'l>(
    term: &Term,
    lists: &'l HashMap<String, CategoryList>,
) -> Vec<&'l CategoryList> {
    term.iter()
        .filter(|v| !v.scale)
        .map(|v| list_of(lists, v))
        .collect()
}

/// The positions of the categories of a tested variable that a test compares: the visible
/// categories, except that a subtotal takes the place of its categories when it hides them
/// (HSUBTOTAL) or when `subtotals` (CATEGORIES=SUBTOTALS) asks for it. Totals, other
/// subtotals and computed categories take no part.
fn tested_categories(list: &CategoryList, subtotals: bool) -> Vec<usize> {
    let replaces = |s: usize| {
        list.categories[s]
            .subtotal
            .is_some_and(|t| t.hides || subtotals)
    };
    (0..list.categories.len())
        .filter(|&i| {
            let c = &list.categories[i];
            match c.subtotal {
                Some(_) => replaces(i),
                None => c.key.is_some() && !c.hidden && !c.within.is_some_and(replaces),
            }
        })
        .collect()
}

/// The header path of a combination of categories, as the main table shows it.
fn path(term: &Term, lists: &HashMap<String, CategoryList>, combo: &[usize]) -> Vec<String> {
    let mut path = Vec::new();
    let mut c = 0;
    for var in term {
        path.extend(var.label.clone());
        if !var.scale {
            path.push(list_of(lists, var).categories[combo[c]].label.clone());
            c += 1;
        }
    }
    path
}

/// "Region: North > Gender": the outer categories of a term and its innermost variable.
fn side_label(term: &Term, lists: &HashMap<String, CategoryList>, outer: &[usize]) -> String {
    let vars: Vec<_> = term.iter().filter(|v| !v.scale).collect();
    let mut parts: Vec<String> = vars
        .iter()
        .zip(outer)
        .map(|(v, &k)| {
            let category = &list_of(lists, v).categories[k].label;
            match &v.label {
                Some(label) => format!("{label}: {category}"),
                None => category.clone(),
            }
        })
        .collect();
    let inner = vars.last().expect("a categorical variable");
    parts.push(
        inner
            .label
            .clone()
            .unwrap_or_else(|| inner.variable.name().to_string()),
    );
    parts.join(" > ")
}

/// The sections of one layer term, by row term and column term.
pub(super) type Sections<'s> = HashMap<(usize, usize), &'s Section>;

/// The tests of one layer of a CTABLES table (`layer` holds its categories): the tables that
/// follow the main one, in the order of their subcommands' kinds (chi-square, then the
/// comparisons), and the comparisons to merge into the main table (MERGE=YES).
#[allow(clippy::type_complexity)]
pub(super) fn tables<'t>(
    table: &'t CtablesTable,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &Sections,
    layer: &[usize],
) -> Result<(Vec<Table>, Vec<(&'t Comparetest, Comparisons)>), Error> {
    let (mut out, mut merged) = (Vec::new(), Vec::new());
    if let Some(spec) = &table.sigtest {
        out.push(chi_square_table(spec, layout, lists, sections, layer)?);
    }
    for spec in &table.comparetests {
        let found = comparisons(spec, layout, lists, sections, layer)?;
        if spec.merge {
            merged.push((spec, found));
        } else {
            out.push(comparison_table(spec, &found, layout, lists));
        }
    }
    Ok((out, merged))
}

/// A section whose rows and columns are both categorical, without a scale variable.
fn categorical_section(row: &Term, column: &Term, section: &Section) -> bool {
    !section.has_scale && row.iter().any(|v| !v.scale) && column.iter().any(|v| !v.scale)
}

/// The innermost categorical variable of a term, the one a test compares.
fn tested_variable<'t, 'a>(term: &'t Term<'a>) -> Option<&'t TermVar<'a>> {
    term.iter().rfind(|v| !v.scale)
}

/// Why the tests leave out a multiple response set: INCLUDEMRSETS=NO, or a category set whose
/// responses count duplicates. None for a categorical variable and a set that is tested.
fn set_left_out(term: &Term, include: bool) -> Option<&'static str> {
    let set = tested_variable(term)?.set.as_ref()?;
    if !include {
        Some("INCLUDEMRSETS=NO leaves out the multiple response sets.")
    } else if set.count_duplicates && set.counted.is_none() {
        Some(
            "A multiple category set is left out because /MRSETS COUNTDUPLICATES=YES counts duplicate responses.",
        )
    } else {
        None
    }
}

/// Which sides of a section are multiple response sets: the innermost row and column variable.
fn set_sides(row: &Term, column: &Term) -> (bool, bool) {
    let is_set = |term: &Term| tested_variable(term).is_some_and(|v| v.set.is_some());
    (is_set(row), is_set(column))
}

/// The cell count at a key, 0 without cases.
fn count_at(section: &Section, key: &[usize]) -> f64 {
    section.cells.get(key).map_or(0.0, |t| t.count)
}

fn chi_square_table(
    spec: &Sigtest,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &Sections,
    layer: &[usize],
) -> Result<Table, Error> {
    let mut rows = Vec::new();
    let (mut untested, mut with_sets) = (false, false);
    let mut left_out = Vec::new();
    for (r, row_term) in layout.rows.iter().enumerate() {
        for (c, column_term) in layout.columns.iter().enumerate() {
            let section = sections[&(r, c)];
            if !categorical_section(row_term, column_term, section) {
                continue;
            }
            let why = set_left_out(row_term, spec.include_mrsets)
                .or(set_left_out(column_term, spec.include_mrsets));
            if let Some(why) = why {
                left_out.push(why);
                continue;
            }
            let sets = set_sides(row_term, column_term);
            with_sets |= sets.0 || sets.1;
            let (row_lists, column_lists) = (
                category_lists(row_term, lists),
                category_lists(column_term, lists),
            );
            let (nr, nc) = (row_lists.len(), column_lists.len());
            let inner_rows = tested_categories(row_lists[nr - 1], spec.subtotals);
            let inner_columns = tested_categories(column_lists[nc - 1], spec.subtotals);
            for row_outer in tested_combinations(&row_lists[..nr - 1]) {
                for column_outer in tested_combinations(&column_lists[..nc - 1]) {
                    // whole cases: weighted counts are rounded for the test, unless the weights
                    // are effective base weights
                    let at = |i: usize, j: usize| {
                        let key = [&row_outer[..], &[i], &column_outer, &[j], layer].concat();
                        let count = count_at(section, &key);
                        if section.effective {
                            count
                        } else {
                            count.round()
                        }
                    };
                    let counts: Vec<Vec<f64>> = inner_rows
                        .iter()
                        .map(|&i| inner_columns.iter().map(|&j| at(i, j)).collect())
                        .collect();
                    let mut cells = vec![
                        Cell::Text(side_label(row_term, lists, &row_outer)),
                        Cell::Text(side_label(column_term, lists, &column_outer)),
                    ];
                    let test = if sets == (false, false) {
                        pearson(&counts)
                    } else {
                        // a set counts a case in several categories, so the margins count cases
                        let row_margins: Vec<f64> =
                            inner_rows.iter().map(|&i| at(i, MARGIN)).collect();
                        let column_margins: Vec<f64> =
                            inner_columns.iter().map(|&j| at(MARGIN, j)).collect();
                        pearson_margins(
                            &counts,
                            &row_margins,
                            &column_margins,
                            at(MARGIN, MARGIN),
                            sets,
                        )
                    };
                    // effective base weights: the statistic scaled to the effective base, by the
                    // subtable's weight over its sum of squared weights
                    let test = test.map(|test| match section.effective {
                        true => {
                            let key = [&row_outer[..], &[MARGIN], &column_outer, &[MARGIN], layer]
                                .concat();
                            let all = section.cells.get(&key);
                            let factor = all.map_or(1.0, |t| t.count / t.squares);
                            let value = test.value * factor;
                            ChiSquare {
                                value,
                                p: chi_square_p(value, test.df),
                                ..test
                            }
                        }
                        false => test,
                    });
                    match test {
                        Some(test) => {
                            let mark = if test.p < spec.alpha { "*" } else { "" };
                            cells.extend([
                                Cell::Number(test.value),
                                Cell::Number(test.df),
                                Cell::Number(test.p),
                                Cell::Text(mark.into()),
                            ]);
                        }
                        None => {
                            untested = true;
                            cells.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
                        }
                    }
                    rows.push(cells);
                }
            }
        }
    }
    left_out.sort();
    left_out.dedup();
    if rows.is_empty() {
        if let Some(why) = left_out.first() {
            return Err(Error(format!("/SIGTEST: nothing is left to test. {why}")));
        }
        return Err(Error(
            "/SIGTEST: the chi-square test needs a categorical variable in both the rows and the columns of one nesting, without a scale variable".into(),
        ));
    }
    let mut caption = format!(
        "Pearson chi-square test of independence in every subtable; rows and columns without cases are left out. * The statistic is significant at the {} level.",
        spec.alpha
    );
    if with_sets {
        caption.push_str(" With a multiple response set the margins count cases, each term is divided by 1 minus the share of the set's category, and the set keeps all its categories in df; a category every case has is left out.");
    }
    if untested {
        caption.push_str(" A subtable with fewer than two nonempty rows or columns is not tested.");
    }
    if sections.values().any(|s| s.effective) {
        caption.push_str(" With effective base weights the counts are not rounded, and the statistic is multiplied by the subtable's weight over its sum of squared weights.");
    }
    for why in left_out {
        caption.push(' ');
        caption.push_str(why);
    }
    let columns: Vec<String> = ["Rows", "Columns", "Chi-Square", "df", "Sig.", "Significant"]
        .map(String::from)
        .into();
    Ok(Table {
        title: "Pearson Chi-Square Tests".into(),
        row_headers: 2,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        formats: Vec::new(),
        caption: Some(caption),
        outline: None,
        hidden: false,
    })
}

/// One column of a comparisons table: a category of the innermost column variable.
struct Column {
    term: usize,
    combo: Vec<usize>,
    path: Vec<String>,
}

/// What one column shows in one row of comparisons.
enum Outcome {
    /// The column's section is not tested.
    Skipped,
    /// The column takes no part: a period.
    Untested,
    /// The significant columns with a smaller value, by position among the compared columns,
    /// with their adjusted p-value and case (true for uppercase).
    Marks(Vec<(usize, f64, bool)>),
}

/// One row of comparisons: a combination of the categories of a row term.
struct Row {
    term: usize,
    combo: Vec<usize>,
    /// By compared column.
    outcomes: Vec<Outcome>,
}

/// The outcome of one COMPARETEST over a table.
pub(super) struct Comparisons {
    columns: Vec<Column>,
    rows: Vec<Row>,
    /// Some column is marked with a period.
    dots: bool,
    left_out: Vec<&'static str>,
    with_sets: bool,
    /// The weights are effective base weights (/WEIGHT).
    effective: bool,
}

/// The outcome of one family of comparisons, by category position of the innermost column
/// variable: whether it took part, and the significant smaller categories with their adjusted
/// p-value and case (true for uppercase).
#[derive(Default)]
struct Family {
    tested: HashMap<usize, bool>,
    marks: HashMap<usize, Vec<(usize, f64, bool)>>,
}

/// Runs one COMPARETEST over every row of every tested subtable.
fn comparisons(
    spec: &Comparetest,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &Sections,
    layer: &[usize],
) -> Result<Comparisons, Error> {
    let means = spec.kind == CompareKind::Means;
    // which sections could be tested, and which of those leave out a set
    let suitable = |r: usize, c: usize| {
        let (row, column) = (&layout.rows[r], &layout.columns[c]);
        let section = sections[&(r, c)];
        if !means {
            return categorical_section(row, column, section);
        }
        let has_mean = section.summaries.iter().any(|(_, s, _)| {
            s.function.statistic == Statistic::Mean
                && !s.function.unweighted
                && s.function.estimate == Estimate::Value
        });
        row.iter().any(|v| v.scale) && has_mean && column.iter().any(|v| !v.scale)
    };
    let why_left_out = |r: usize, c: usize| {
        let row = (!means)
            .then(|| set_left_out(&layout.rows[r], spec.include_mrsets))
            .flatten();
        row.or(set_left_out(&layout.columns[c], spec.include_mrsets))
    };
    let eligible = |r: usize, c: usize| suitable(r, c) && why_left_out(r, c).is_none();
    let mut left_out: Vec<&str> = (0..layout.rows.len())
        .flat_map(|r| (0..layout.columns.len()).map(move |c| (r, c)))
        .filter(|&(r, c)| suitable(r, c))
        .filter_map(|(r, c)| why_left_out(r, c))
        .collect();
    left_out.sort();
    left_out.dedup();
    let with_sets = (0..layout.rows.len())
        .flat_map(|r| (0..layout.columns.len()).map(move |c| (r, c)))
        .filter(|&(r, c)| eligible(r, c))
        .any(|(r, c)| {
            let sides = set_sides(&layout.rows[r], &layout.columns[c]);
            sides.1 || (!means && sides.0)
        });

    // the columns of the table and their letters
    let mut columns: Vec<Column> = Vec::new();
    for (c, term) in layout.columns.iter().enumerate() {
        let term_lists = category_lists(term, lists);
        if term_lists.is_empty() || !(0..layout.rows.len()).any(|r| eligible(r, c)) {
            continue;
        }
        let inner = term_lists.len() - 1;
        let compared = tested_categories(term_lists[inner], spec.subtotals);
        for combo in tested_combinations(&term_lists[..inner]) {
            for &k in &compared {
                let combo = [&combo[..], &[k]].concat();
                columns.push(Column {
                    term: c,
                    path: path(term, lists, &combo),
                    combo,
                });
            }
        }
    }
    let positions: HashMap<(usize, &[usize]), usize> = columns
        .iter()
        .enumerate()
        .map(|(i, col)| ((col.term, &col.combo[..]), i))
        .collect();

    let mut rows: Vec<Row> = Vec::new();
    let mut dots = false;
    for (r, term) in layout.rows.iter().enumerate() {
        if !(0..layout.columns.len()).any(|c| eligible(r, c)) {
            continue;
        }
        let row_lists = category_lists(term, lists);
        let row_combos = if means {
            tested_combinations(&row_lists)
        } else {
            // in the proportions test a row is a compared category of the innermost row variable
            let inner = row_lists.len() - 1;
            let compared = tested_categories(row_lists[inner], spec.subtotals);
            let outer = tested_combinations(&row_lists[..inner]);
            outer
                .iter()
                .flat_map(|o| compared.iter().map(move |&k| [&o[..], &[k]].concat()))
                .collect()
        };
        for row_combo in row_combos {
            let mut families: HashMap<(usize, Vec<usize>), Family> = HashMap::new();
            let mut outcomes = Vec::new();
            for column in &columns {
                if !eligible(r, column.term) {
                    outcomes.push(Outcome::Skipped);
                    continue;
                }
                let (outer, inner) = column.combo.split_at(column.combo.len() - 1);
                let family = families
                    .entry((column.term, outer.to_vec()))
                    .or_insert_with(|| {
                        let section = sections[&(r, column.term)];
                        let column_term = &layout.columns[column.term];
                        let column_lists = category_lists(column_term, lists);
                        let inner_columns =
                            tested_categories(column_lists[column_lists.len() - 1], spec.subtotals);
                        let sets = set_sides(term, column_term);
                        let at = Place {
                            row: &row_combo,
                            column_outer: outer,
                            layer,
                        };
                        // the means test may have no categorical row variable
                        let inner_rows = row_lists
                            .last()
                            .map_or(Vec::new(), |list| tested_categories(list, spec.subtotals));
                        compare(spec, section, at, (&inner_rows, &inner_columns), sets)
                    });
                outcomes.push(if !family.tested[&inner[0]] {
                    dots = true;
                    Outcome::Untested
                } else {
                    let marks = family.marks.get(&inner[0]).map_or(&[][..], |m| &m[..]);
                    let marks = marks.iter().map(|&(smaller, p, upper)| {
                        let key = [outer, &[smaller][..]].concat();
                        (positions[&(column.term, &key[..])], p, upper)
                    });
                    Outcome::Marks(marks.collect())
                });
            }
            rows.push(Row {
                term: r,
                combo: row_combo,
                outcomes,
            });
        }
    }
    if rows.is_empty() || columns.is_empty() {
        if let Some(why) = left_out.first() {
            return Err(Error(format!(
                "/COMPARETEST: nothing is left to test. {why}"
            )));
        }
        return Err(Error(if means {
            "/COMPARETEST TYPE=MEAN needs a scale variable summarized by MEAN in the rows and a categorical variable in the columns".into()
        } else {
            "/COMPARETEST TYPE=PROP needs a categorical variable in both the rows and the columns of one nesting, without a scale variable".into()
        }));
    }
    Ok(Comparisons {
        columns,
        rows,
        dots,
        left_out,
        with_sets,
        effective: sections.values().any(|s| s.effective),
    })
}

/// The letters of one cell of a separate comparisons table or of a SIMPLE merged column.
fn simple_letters(spec: &Comparetest, marks: &[(usize, f64, bool)], merged: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    for &(smaller, p, upper) in marks {
        let mut text = letter(smaller);
        // merged keys are lowercase like the column keys, unless two levels need both cases
        if !upper || (merged && spec.alpha.len() == 1) {
            text = text.to_lowercase();
        }
        if spec.show_sig {
            text.push_str(&format!(" ({})", p_text(p)));
        }
        parts.push(text);
    }
    parts.join(" ")
}

fn comparison_table(
    spec: &Comparetest,
    found: &Comparisons,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
) -> Table {
    let paths: Vec<Vec<String>> = found
        .rows
        .iter()
        .map(|row| path(&layout.rows[row.term], lists, &row.combo))
        .collect();
    let row_headers = paths.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let levels = found
        .columns
        .iter()
        .map(|c| c.path.len())
        .max()
        .unwrap_or(0);
    let mut header = vec![String::new(); row_headers];
    let mut column_groups = vec![vec![String::new(); levels]; row_headers];
    for (i, column) in found.columns.iter().enumerate() {
        header.push(format!("({})", letter(i)));
        let mut groups = column.path.clone();
        groups.resize(levels, String::new());
        column_groups.push(groups);
    }
    let table_rows = found
        .rows
        .iter()
        .zip(paths)
        .map(|(row, path)| {
            let mut cells: Vec<Cell> = (0..row_headers)
                .map(|i| path.get(i).map_or(Cell::Empty, |t| Cell::Text(t.clone())))
                .collect();
            cells.extend(row.outcomes.iter().map(|outcome| match outcome {
                Outcome::Skipped => Cell::Empty,
                Outcome::Untested => Cell::Text(".".into()),
                Outcome::Marks(marks) if marks.is_empty() => Cell::Empty,
                Outcome::Marks(marks) => Cell::Text(simple_letters(spec, marks, false)),
            }));
            cells
        })
        .collect();
    Table {
        title: if spec.kind == CompareKind::Means {
            "Comparisons of Column Means".into()
        } else {
            "Comparisons of Column Proportions".into()
        },
        row_headers,
        columns: header,
        column_groups,
        rows: table_rows,
        formats: Vec::new(),
        caption: Some(comparison_caption(spec, found, None)),
        outline: None,
        hidden: false,
    }
}

/// The APA letters of one family of columns: columns that do not differ significantly share a
/// letter. Starting from one group of all tested columns, every significant pair splits each
/// group holding both into one without either; a group inside another is dropped. The groups,
/// ordered by their first column, get the letters a, b, c and so on; a column shows the letters
/// of its groups.
fn apa_letters(members: &[usize], pairs: &[(usize, usize)]) -> HashMap<usize, String> {
    let mut groups: Vec<Vec<usize>> = vec![members.to_vec()];
    for &(a, b) in pairs {
        let mut next: Vec<Vec<usize>> = Vec::new();
        for group in groups {
            if group.contains(&a) && group.contains(&b) {
                next.push(group.iter().copied().filter(|&m| m != a).collect());
                next.push(group.iter().copied().filter(|&m| m != b).collect());
            } else {
                next.push(group);
            }
        }
        next.sort();
        next.dedup();
        let inside = |g: &Vec<usize>, h: &Vec<usize>| g != h && g.iter().all(|m| h.contains(m));
        groups = next
            .iter()
            .filter(|g| !next.iter().any(|h| inside(g, h)))
            .cloned()
            .collect();
    }
    groups.sort();
    let mut out: HashMap<usize, String> = members.iter().map(|&m| (m, String::new())).collect();
    for (i, group) in groups.iter().enumerate() {
        let key = letter(i).to_lowercase();
        for m in group {
            out.get_mut(m).expect("a member").push_str(&key);
        }
    }
    out
}

/// The text of every column of one row of comparisons, as merged into the main table.
fn merged_letters(spec: &Comparetest, found: &Comparisons, row: &Row) -> Vec<String> {
    let mut texts: Vec<String> = row
        .outcomes
        .iter()
        .map(|outcome| match outcome {
            Outcome::Skipped => String::new(),
            Outcome::Untested => ".".into(),
            Outcome::Marks(marks) => match spec.style {
                CompareStyle::Simple => simple_letters(spec, marks, true),
                CompareStyle::Apa => String::new(),
            },
        })
        .collect();
    if spec.style == CompareStyle::Apa {
        // a family is the columns of one category combination of the outer column variables
        let mut families: HashMap<(usize, &[usize]), Vec<usize>> = HashMap::new();
        for (i, column) in found.columns.iter().enumerate() {
            if let Outcome::Marks(_) = row.outcomes[i] {
                let outer = &column.combo[..column.combo.len() - 1];
                families.entry((column.term, outer)).or_default().push(i);
            }
        }
        for members in families.values() {
            // only the smaller level counts in APA style
            let pairs: Vec<(usize, usize)> = members
                .iter()
                .flat_map(|&i| match &row.outcomes[i] {
                    Outcome::Marks(marks) => marks
                        .iter()
                        .filter(|m| m.2)
                        .map(|m| (i, m.0))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect();
            for (i, text) in apa_letters(members, &pairs) {
                texts[i] = text;
            }
        }
    }
    texts
}

/// Puts the letters of a COMPARETEST with MERGE=YES into the main table: after the columns of
/// every compared category comes a column with its letters, headed by the category's key in
/// SIMPLE style. The letters stand in the rows of the compared summary.
pub(super) fn merge(
    main: &mut Table,
    spec: &Comparetest,
    found: &Comparisons,
    row_entries: &[Entry],
    column_entries: &[Entry],
    slots: &[Slot],
) {
    let h = main.row_headers;
    let positions: HashMap<(usize, &[usize]), usize> = found
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| ((col.term, &col.combo[..]), i))
        .collect();
    // the new columns: an old one, or the letters of a compared category
    let mut order: Vec<Result<usize, usize>> = (0..h).map(Ok).collect();
    for (e, entry) in column_entries.iter().enumerate() {
        order.push(Ok(h + e));
        let last = column_entries
            .get(e + 1)
            .is_none_or(|next| next.term != entry.term || next.combo != entry.combo);
        if let (true, Some(&k)) = (last, positions.get(&(entry.term, &entry.combo[..]))) {
            order.push(Err(k));
        }
    }
    let old_path = |c: usize| {
        let mut path = main.column_groups.get(c).cloned().unwrap_or_default();
        path.push(main.columns[c].clone());
        path
    };
    let key = |k: usize| match spec.style {
        CompareStyle::Simple => format!("({})", letter(k).to_lowercase()),
        CompareStyle::Apa => "Group".to_string(),
    };
    let paths: Vec<Vec<String>> = order
        .iter()
        .map(|source| match *source {
            Ok(c) => old_path(c),
            Err(k) => [&found.columns[k].path[..], &[key(k)][..]].concat(),
        })
        .collect();
    let levels = paths.iter().map(Vec::len).max().unwrap_or(1);
    let at = |path: &[String], i: usize| path.get(i).cloned().unwrap_or_default();
    main.columns = paths.iter().map(|p| at(p, levels - 1)).collect();
    main.column_groups = paths
        .iter()
        .map(|p| (0..levels - 1).map(|i| at(p, i)).collect())
        .collect();

    // the summary whose rows get the letters when the summaries are in the rows
    let wanted = |s: &Slot| match spec.kind {
        CompareKind::Means => s.function.statistic == Statistic::Mean,
        CompareKind::Proportions => matches!(
            s.function.statistic,
            Statistic::Percent(PercentBase::Column, PercentOf::Count)
        ),
    };
    let letter_slot = slots
        .iter()
        .position(|s| wanted(s) && !s.function.unweighted && s.function.estimate == Estimate::Value)
        .or_else(|| {
            slots
                .iter()
                .position(|s| s.function.statistic == Statistic::Count)
        })
        .unwrap_or(0);
    let texts: HashMap<(usize, &[usize]), Vec<String>> = found
        .rows
        .iter()
        .map(|row| ((row.term, &row.combo[..]), merged_letters(spec, found, row)))
        .collect();
    for (r, entry) in row_entries.iter().enumerate() {
        let letters = texts
            .get(&(entry.term, &entry.combo[..]))
            .filter(|_| entry.slot.is_none_or(|s| s == letter_slot));
        // every old column is used once, so its cell can be moved
        let mut old_cells: Vec<Option<Cell>> = std::mem::take(&mut main.rows[r])
            .into_iter()
            .map(Some)
            .collect();
        let old_formats = std::mem::take(&mut main.formats[r]);
        for source in &order {
            match *source {
                Ok(c) => {
                    main.rows[r].push(old_cells[c].take().expect("each column once"));
                    main.formats[r].push(old_formats[c].clone());
                }
                Err(k) => {
                    let text = letters.map(|l| l[k].clone()).unwrap_or_default();
                    main.rows[r].push(if text.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Text(text)
                    });
                    main.formats[r].push(String::new());
                }
            }
        }
    }
    let note = comparison_caption(spec, found, Some(spec.style));
    main.caption = Some(match main.caption.take() {
        Some(caption) => format!("{caption} {note}"),
        None => note,
    });
}

/// Where a row of comparisons lies: its row categories, the categories of the outer column
/// variables of its subtable, and its layer.
#[derive(Clone, Copy)]
struct Place<'p> {
    row: &'p [usize],
    column_outer: &'p [usize],
    layer: &'p [usize],
}

/// The comparisons of one row of one subtable.
fn compare(
    spec: &Comparetest,
    section: &Section,
    at: Place,
    (inner_rows, inner_columns): (&[usize], &[usize]),
    sets: (bool, bool),
) -> Family {
    let set = sets.1;
    let (row_combo, column_outer, layer) = (at.row, at.column_outer, at.layer);
    let key = |row: &[usize], j: usize| [row, column_outer, &[j], layer].concat();
    // the cases two categories of a set in the columns share
    let shared_by = |row: &[usize], j: usize, k: usize, valid: bool| {
        let (a, b) = (inner_columns[j], inner_columns[k]);
        let pair = [a.min(b), a.max(b)];
        let tally = section
            .pairs
            .get(&[row, column_outer, layer, &pair[..]].concat());
        tally.map_or(0.0, |t| if valid { t.valid } else { t.count })
    };
    let (pairs, tested) = match spec.kind {
        CompareKind::Proportions => {
            let (outer_rows, _) = row_combo.split_at(row_combo.len() - 1);
            // a column total counts each case once, also when the rows are a set
            let margin = [outer_rows, &[MARGIN][..]].concat();
            let counts: Vec<f64> = inner_columns
                .iter()
                .map(|&j| count_at(section, &key(row_combo, j)))
                .collect();
            let totals: Vec<f64> = inner_columns
                .iter()
                .map(|&j| count_at(section, &key(&margin, j)))
                .collect();
            if section.effective {
                proportions_effective(
                    &effective_columns(section, at, (inner_rows, inner_columns), sets.0, &counts),
                    &|j, k| {
                        effective_shared(section, at, (inner_rows, inner_columns), sets, (j, k))
                    },
                )
            } else {
                proportions(&counts, &totals, &|j, k| shared_by(&margin, j, k, false))
            }
        }
        CompareKind::Means => {
            let groups: Vec<Group> = inner_columns
                .iter()
                .map(|&j| {
                    group(
                        section
                            .cells
                            .get(&key(row_combo, j))
                            .map_or(&[][..], |t| &t.values),
                    )
                })
                .collect();
            let shared = |j: usize, k: usize| shared_by(row_combo, j, k, true);
            let shared = set.then_some(&shared as Shared);
            if section.effective {
                means_effective(&groups, spec.pool_all, shared)
            } else {
                means(&groups, spec.pool_all, shared)
            }
        }
    };
    let adjusted = adjust(
        &pairs.iter().map(|p| p.p).collect::<Vec<f64>>(),
        spec.adjust,
    );
    let mut family = Family::default();
    for (position, took_part) in tested.into_iter().enumerate() {
        family.tested.insert(inner_columns[position], took_part);
    }
    for (pair, p) in pairs.iter().zip(adjusted) {
        if let Some(upper) = significance(p, &spec.alpha, spec.adjust) {
            family
                .marks
                .entry(inner_columns[pair.larger])
                .or_default()
                .push((inner_columns[pair.smaller], p, upper));
        }
    }
    for marks in family.marks.values_mut() {
        marks.sort_by_key(|m| m.0);
    }
    family
}

/// The tally of a cell of a subtable: row categories, the outer column categories, the inner
/// column category and the layer.
fn cell_at<'s>(section: &'s Section, at: Place, row: &[usize], j: usize) -> Option<&'s Tally> {
    section
        .cells
        .get(&[row, at.column_outer, &[j], at.layer].concat())
}

/// The effective base of a cell, 0 without cases.
fn cell_base(tally: Option<&Tally>) -> f64 {
    tally.map_or(0.0, |t| effective_base(t.count, t.squares))
}

/// The columns of one row of a proportions test with effective base weights. A column's
/// effective base is the sum of those of its cells in the subtable's rows; when the rows are a
/// set, whose cells share cases, it is the effective base of the column total.
fn effective_columns(
    section: &Section,
    at: Place,
    (inner_rows, inner_columns): (&[usize], &[usize]),
    row_set: bool,
    counts: &[f64],
) -> Vec<BaseColumn> {
    let outer_rows = &at.row[..at.row.len() - 1];
    let margin = [outer_rows, &[MARGIN][..]].concat();
    inner_columns
        .iter()
        .zip(counts)
        .map(|(&j, &count)| {
            let total = cell_at(section, at, &margin, j);
            let base = if row_set {
                cell_base(total)
            } else {
                inner_rows
                    .iter()
                    .map(|&i| cell_base(cell_at(section, at, &[outer_rows, &[i]].concat(), j)))
                    .sum()
            };
            BaseColumn {
                count,
                total: total.map_or(0.0, |t| t.count),
                base,
                cases: total.map_or(0.0, |t| t.cases),
            }
        })
        .collect()
}

/// The effective base of the cases two columns of a set share (columns j and k by position):
/// for every row 0.5 w_ijk (e_ij / w_ij + e_ik / w_ik), summed over the rows; when the rows are
/// a set too, 0.5 w_jk (e_j / w_j + e_k / w_k) from the column totals. 0 for a categorical
/// variable in the columns.
fn effective_shared(
    section: &Section,
    at: Place,
    (inner_rows, inner_columns): (&[usize], &[usize]),
    sets: (bool, bool),
    (j, k): (usize, usize),
) -> f64 {
    if !sets.1 {
        return 0.0;
    }
    let (a, b) = (inner_columns[j], inner_columns[k]);
    let pair = [a.min(b), a.max(b)];
    let outer_rows = &at.row[..at.row.len() - 1];
    let part = |row: &[usize]| {
        let shared = section
            .pairs
            .get(&[row, at.column_outer, at.layer, &pair[..]].concat())
            .map_or(0.0, |t| t.count);
        let ratio = |c: usize| {
            cell_at(section, at, row, c)
                .filter(|t| t.count > 0.0)
                .map_or(0.0, |t| effective_base(t.count, t.squares) / t.count)
        };
        0.5 * shared * (ratio(a) + ratio(b))
    };
    if sets.0 {
        part(&[outer_rows, &[MARGIN][..]].concat())
    } else {
        inner_rows
            .iter()
            .map(|&i| part(&[outer_rows, &[i]].concat()))
            .sum()
    }
}

fn p_text(p: f64) -> String {
    if p < 0.0005 {
        "<0.001".into()
    } else {
        format!("{p:.3}")
    }
}

/// The caption of a separate comparisons table (`merged` None) or of letters merged into the
/// main table in a style.
fn comparison_caption(
    spec: &Comparetest,
    found: &Comparisons,
    merged: Option<CompareStyle>,
) -> String {
    let (dots, left_out, with_sets) = (found.dots, &found.left_out, found.with_sets);
    let base = if found.effective {
        ", with effective base weights"
    } else {
        ""
    };
    let means = spec.kind == CompareKind::Means;
    let (what, smaller) = if means {
        let pooled = if spec.pool_all {
            "pooled from all tested categories"
        } else {
            "pooled from the two categories compared"
        };
        (
            format!("Two-sided t-tests of the equality of column means, variance {pooled}{base}"),
            "mean",
        )
    } else {
        (
            match found.effective {
                true => format!("Two-sided t-tests of the equality of column proportions{base}"),
                false => "Two-sided z-tests of the equality of column proportions".to_string(),
            },
            "proportion",
        )
    };
    let levels = match (merged, &spec.alpha[..]) {
        // APA style uses the smaller level only
        (Some(CompareStyle::Apa), [a, ..]) | (_, [a]) => format!("at the {a} level"),
        (_, [a, b]) => format!("uppercase letters at the {a} level, lowercase at the {b} level"),
        _ => unreachable!("one or two levels"),
    };
    let rule = match merged {
        None => format!(
            "When a pair differs significantly, the column with the larger {smaller} shows the letter of the other column."
        ),
        Some(CompareStyle::Simple) => format!(
            "Each compared column is followed by a column of letters headed by its key; when a pair differs significantly, the column with the larger {smaller} shows the key of the other column."
        ),
        Some(CompareStyle::Apa) => "Each compared column is followed by its Group letters; within a row of a subtable, two columns whose letters have none in common differ significantly.".to_string(),
    };
    let adjusted = match spec.adjust {
        Adjust::Bonferroni => {
            "adjusted for the comparisons within each row of a subtable by the Bonferroni correction"
        }
        Adjust::BenjaminiHochberg => {
            "adjusted for the comparisons within each row of a subtable by the Benjamini-Hochberg false discovery rate"
        }
        Adjust::None => "not adjusted for multiple comparisons",
    };
    let mut caption = format!("{what}, {levels}. {rule} Significance values are {adjusted}.");
    if spec.show_sig {
        caption.push_str(" The adjusted significance follows each letter in parentheses.");
    }
    if dots && found.effective {
        caption.push_str(" A period marks a category that is not compared: it has no case, or neither it nor any other has two.");
    } else if dots {
        caption.push_str(if means {
            " A period marks a category that weighs less than 2 and is not compared."
        } else {
            " A period marks a column that is not compared in its row: its proportion is 0 or 1, or its total weighs less than 2."
        });
    }
    if with_sets {
        caption.push_str(if means {
            " Categories of a multiple response set share cases: the shared weight reduces the standard error and the degrees of freedom, and the variance is pooled from the two categories compared."
        } else {
            " Columns of a multiple response set share cases: the shared weight reduces the standard error, and column totals count each case once."
        });
    }
    for why in left_out {
        caption.push(' ');
        caption.push_str(why);
    }
    caption
}

#[cfg(test)]
mod tests {
    //! The expected values are worked out by hand from the formulas, or are textbook values of
    //! the normal and t distributions.

    use super::*;
    use crate::dist::{beta_i, erfc};

    const NONE_SHARED: Shared = &|_, _| 0.0;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance * b.abs().max(1e-300)
    }

    #[test]
    fn erfc_matches_values_known_to_many_digits() {
        let cases = [
            (0.0, 1.0),
            (0.5, 0.479_500_122_186_953_5),
            (1.0, 0.157_299_207_050_285_13),
            (2.0, 0.004_677_734_981_047_266),
            (3.0, 2.209_049_699_858_544e-5),
            (5.0, 1.537_459_794_428_035e-12),
            (-1.0, 1.842_700_792_949_715),
        ];
        for (x, expected) in cases {
            assert!(close(erfc(x), expected, 1e-12), "{x}: {}", erfc(x));
        }
    }

    #[test]
    fn two_sided_normal_significance_known_values() {
        // z = 1.959963984540054 is the 97.5th percentile; z = 2 leaves 4.55% in both tails
        assert!(close(normal_p(1.959_963_984_540_054), 0.05, 1e-12));
        assert!(close(normal_p(-2.575_829_303_548_901), 0.01, 1e-12));
        assert!(close(normal_p(2.0), 0.045_500_263_896_358_42, 1e-12));
        assert_eq!(normal_p(0.0), 1.0);
    }

    /// Two-sided t significance by the closed forms for whole degrees of freedom: with
    /// theta = atan(t / sqrt(df)), P(|T| < t) is sin(theta) times a finite series in cos^2 for
    /// even df, and 2 / pi times theta plus such a series for odd df.
    fn t_closed_form(t: f64, df: u32) -> f64 {
        let theta = (t.abs() / f64::from(df).sqrt()).atan();
        let (s, c2) = (theta.sin(), theta.cos().powi(2));
        let inside = if df.is_multiple_of(2) {
            let (mut term, mut sum) = (1.0, 1.0);
            for k in 1..df / 2 {
                term *= c2 * f64::from(2 * k - 1) / f64::from(2 * k);
                sum += term;
            }
            s * sum
        } else {
            let (mut term, mut sum) = (1.0, if df > 1 { 1.0 } else { 0.0 });
            for k in 1..(df.saturating_sub(1)) / 2 {
                term *= c2 * f64::from(2 * k) / f64::from(2 * k + 1);
                sum += term;
            }
            2.0 / std::f64::consts::PI * (theta + s * theta.cos() * sum)
        };
        1.0 - inside
    }

    #[test]
    fn t_significance_matches_the_closed_forms() {
        for df in [1, 2, 3, 4, 5, 6, 9, 10, 17, 30, 61] {
            for t in [0.05, 0.4, 1.0, 1.7, 2.5, 4.0, 9.0] {
                let expected = t_closed_form(t, df);
                // the closed form takes 1 minus a number near 1, so far tails keep only an
                // absolute accuracy near 1e-16
                let got = t_p(t, f64::from(df));
                assert!(
                    (got - expected).abs() <= 1e-10 * expected + 1e-15,
                    "t {t} df {df}: {} vs {expected}",
                    t_p(t, f64::from(df))
                );
                assert_eq!(t_p(-t, f64::from(df)), t_p(t, f64::from(df)));
            }
        }
        // the Cauchy case: P(|T| > 1) = 1/2
        assert!(close(t_p(1.0, 1.0), 0.5, 1e-14));
    }

    #[test]
    fn t_significance_textbook_critical_values() {
        // two-sided 5% points of the t tables, and 1% for 20 degrees of freedom
        for (t, df, p) in [
            (12.706_204_736_174_7, 1.0, 0.05),
            (2.228_138_851_986_5, 10.0, 0.05),
            (2.042_272_456_301_2, 30.0, 0.05),
            (2.845_339_709_785_8, 20.0, 0.01),
        ] {
            assert!(close(t_p(t, df), p, 1e-9), "{t} {df}: {}", t_p(t, df));
        }
        // many degrees of freedom approach the normal distribution
        assert!(close(t_p(1.959_963_984_540_054, 1e7), 0.05, 1e-6));
        assert!(close(t_p(3.0, 1e6), normal_p(3.0), 1e-4));
    }

    #[test]
    fn beta_i_symmetry_and_closed_forms() {
        // I_x(1, 1) = x, I_x(a, 1) = x^a, I_x(a, b) = 1 - I_{1-x}(b, a)
        assert!(close(beta_i(1.0, 1.0, 0.3), 0.3, 1e-14));
        assert!(close(beta_i(3.5, 1.0, 0.7), 0.7_f64.powf(3.5), 1e-13));
        assert!(close(
            beta_i(2.0, 5.0, 0.2),
            1.0 - beta_i(5.0, 2.0, 0.8),
            1e-13
        ));
        assert_eq!((beta_i(2.0, 3.0, 0.0), beta_i(2.0, 3.0, 1.0)), (0.0, 1.0));
    }

    #[test]
    fn pearson_of_a_two_by_two_table() {
        // [[10, 20], [30, 40]]: row totals 30, 70, column totals 40, 60, N 100. Expected counts
        // 12, 18, 28, 42, every deviation 2, so chi-square = 4 (1/12 + 1/18 + 1/28 + 1/42)
        // = 200 / 252.
        let test = pearson(&[vec![10.0, 20.0], vec![30.0, 40.0]]).unwrap();
        assert!(close(test.value, 200.0 / 252.0, 1e-14));
        assert_eq!(test.df, 1.0);
        assert!(close(test.p, chi_square_p(200.0 / 252.0, 1.0), 1e-14));
    }

    #[test]
    fn pearson_leaves_out_empty_rows_and_columns() {
        let with_zeros = vec![
            vec![10.0, 0.0, 20.0],
            vec![0.0, 0.0, 0.0],
            vec![30.0, 0.0, 40.0],
        ];
        let test = pearson(&with_zeros).unwrap();
        assert!(close(test.value, 200.0 / 252.0, 1e-14));
        assert_eq!(test.df, 1.0);
        // one nonempty column is no test
        assert_eq!(pearson(&[vec![5.0, 0.0], vec![7.0, 0.0]]), None);
    }

    #[test]
    fn proportions_pool_the_two_columns() {
        // 30 of 50 against 20 of 50: pooled 0.5, z = 0.2 / sqrt(0.25 (1/50 + 1/50)) = 2, the
        // same as the square root of the chi-square of the 2 x 2 table [[30, 20], [20, 30]] = 4.
        let (pairs, tested) = proportions(&[30.0, 20.0], &[50.0, 50.0], NONE_SHARED);
        assert_eq!(tested, [true, true]);
        assert_eq!((pairs[0].larger, pairs[0].smaller), (0, 1));
        assert!(close(pairs[0].p, normal_p(2.0), 1e-14));
        let chi = pearson(&[vec![30.0, 20.0], vec![20.0, 30.0]]).unwrap();
        assert!(close(chi.value, 4.0, 1e-14) && close(chi.p, pairs[0].p, 1e-12));
    }

    #[test]
    fn proportions_round_the_column_totals_only() {
        // weighted: 12.4 of 20.4 against 6.2 of 19.6. The proportions use the weights as they
        // are; the totals become 20 and 20. p1 = 0.607843, p2 = 0.316327, pooled
        // (20 p1 + 20 p2) / 40 = 0.462085, z = 0.291517 / sqrt(0.462085 * 0.537915 * 0.1).
        let (p1, p2): (f64, f64) = (12.4 / 20.4, 6.2 / 19.6);
        let pooled = (p1 + p2) / 2.0;
        let z = (p1 - p2) / (pooled * (1.0 - pooled) * 0.1).sqrt();
        let (pairs, _) = proportions(&[12.4, 6.2], &[20.4, 19.6], NONE_SHARED);
        assert!(close(pairs[0].p, normal_p(z), 1e-13));
        assert!(close(z, 1.849_1, 1e-4), "{z}");
    }

    #[test]
    fn proportions_leave_out_zero_one_and_small_columns() {
        // column 1 is all of its total, column 2 none, column 3 weighs 1.5 in total
        let (pairs, tested) =
            proportions(&[3.0, 10.0, 0.0, 1.0], &[6.0, 10.0, 8.0, 1.5], NONE_SHARED);
        assert_eq!(tested, [true, false, false, false]);
        assert!(pairs.is_empty());
    }

    #[test]
    fn proportions_of_a_set_take_the_shared_cases_off_the_variance() {
        // 30 of 50 against 20 of 50 with 10 cases in both columns: the factor 1/50 + 1/50
        // becomes 1/50 + 1/50 - 2 * 10 / 2500 = 0.032; the pooled proportion is 0.5, so
        // z = 0.2 / sqrt(0.25 * 0.032)
        let (pairs, _) = proportions(&[30.0, 20.0], &[50.0, 50.0], &|_, _| 10.0);
        let z = 0.2 / (0.25_f64 * 0.032).sqrt();
        assert!(close(pairs[0].p, normal_p(z), 1e-13));
        // two columns that hold the same cases are not compared
        let (pairs, tested) = proportions(&[30.0, 20.0], &[50.0, 50.0], &|_, _| 50.0);
        assert!(pairs.is_empty() && tested == [true, true]);
    }

    #[test]
    fn pearson_with_a_set_on_one_or_both_sides() {
        // rows: 2 categories of a plain variable with 40 and 60 cases (W = 100); columns: 2
        // categories of a set held by 50 and 30 of them. Counts [[25, 10], [25, 20]].
        let f = [vec![25.0, 10.0], vec![25.0, 20.0]];
        let (r, c, w) = ([40.0, 60.0], [50.0, 30.0], 100.0);
        let mut by_hand = 0.0;
        for i in 0..2 {
            for j in 0..2 {
                let e: f64 = r[i] * c[j] / w;
                by_hand += (f[i][j] - e).powi(2) / (e * (1.0 - c[j] / w));
            }
        }
        let test = pearson_margins(&f, &r, &c, w, (false, true)).unwrap();
        assert!(close(test.value, by_hand, 1e-14));
        assert_eq!(test.df, 2.0);
        // with a set on both sides every term is divided by both shares and df is R C
        let both = pearson_margins(&f, &r, &c, w, (true, true)).unwrap();
        assert_eq!(both.df, 4.0);
        let e00 = 40.0 * 50.0 / 100.0;
        let first = (25.0_f64 - e00).powi(2) / (e00 * 0.6 * 0.5);
        assert!(both.value > first);
        // a set category every case has is left out
        let full = pearson_margins(&f, &r, &[50.0, 100.0], w, (false, true));
        assert_eq!(full, None);
        // without sets the margins give the plain test
        let plain = pearson(&[vec![10.0, 20.0], vec![30.0, 40.0]]).unwrap();
        let margins = pearson_margins(
            &[vec![10.0, 20.0], vec![30.0, 40.0]],
            &[30.0, 70.0],
            &[40.0, 60.0],
            100.0,
            (false, false),
        );
        assert_eq!(margins, Some(plain));
    }

    #[test]
    fn means_of_a_set_pool_by_pair_and_take_off_the_shared_weight() {
        // A = {1, 2, 3}, C = {2, 4, 6} with one case in both: pooled from the pair (2 + 8) / 4,
        // factor 1/3 + 1/3 - 2/9 = 4/9, df 3 + 3 - 1 - 2 = 3, even when ALLCATS is asked for
        let a = group(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)]);
        let c = group(&[(2.0, 1.0), (4.0, 1.0), (6.0, 1.0)]);
        let (pairs, _) = means(&[a, c], true, Some(&|_, _| 1.0));
        let t = 2.0 / (2.5_f64 * 4.0 / 9.0).sqrt();
        assert!(close(pairs[0].p, t_closed_form(t, 3), 1e-10));
    }

    #[test]
    fn means_with_variance_pooled_from_all_categories() {
        // A = {1, 2, 3}: mean 2, variance 1; B = {4, 5, 6}: mean 5, variance 1; C = {2, 4, 6}:
        // mean 4, variance 4. W = 9, k = 3, pooled = (2 + 2 + 8) / 6 = 2 with df 6.
        // A against B: t = -3 / sqrt(2 (1/3 + 1/3)) = -3 / sqrt(4/3) = -2.598076.
        let a = group(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)]);
        let b = group(&[(4.0, 1.0), (5.0, 1.0), (6.0, 1.0)]);
        let c = group(&[(2.0, 1.0), (4.0, 1.0), (6.0, 1.0)]);
        assert_eq!((a.mean, a.variance, c.variance), (2.0, 1.0, 4.0));
        let (pairs, tested) = means(&[a, b, c], true, None);
        assert_eq!(tested, [true, true, true]);
        assert_eq!(pairs.len(), 3);
        assert_eq!((pairs[0].larger, pairs[0].smaller), (1, 0));
        let t = 3.0 / (4.0_f64 / 3.0).sqrt();
        assert!(close(pairs[0].p, t_closed_form(t, 6), 1e-10));
        // B against C: t = 1 / sqrt(4/3) with the same df
        assert!(close(
            pairs[2].p,
            t_closed_form(1.0 / (4.0_f64 / 3.0).sqrt(), 6),
            1e-10
        ));
    }

    #[test]
    fn means_with_variance_pooled_from_the_pair() {
        // A against C: pooled (2 * 1 + 2 * 4) / 4 = 2.5 with df 4, t = -2 / sqrt(2.5 * 2/3)
        let a = group(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)]);
        let c = group(&[(2.0, 1.0), (4.0, 1.0), (6.0, 1.0)]);
        let (pairs, _) = means(&[a, c], false, None);
        let t = 2.0 / (2.5_f64 * 2.0 / 3.0).sqrt();
        assert!(close(pairs[0].p, t_closed_form(t, 4), 1e-10));
    }

    #[test]
    fn means_round_the_category_weights() {
        // weights 1.4 + 1.4 + 1.4 = 4.2 count as 4 cases in the test, but the mean is weighted:
        // A = {1, 2, 3} at 1.4 each: mean 2, variance 2.8 / 3.2 = 0.875; B = {5, 7} at 1.1 each
        // (2.2, rounded 2): mean 6, variance 2.2 / 1.2. Pooled from both:
        // (3 * 0.875 + 1 * 2.2 / 1.2) / (4 + 2 - 2), t = -4 / sqrt(pooled (1/4 + 1/2)).
        let a = group(&[(1.0, 1.4), (2.0, 1.4), (3.0, 1.4)]);
        let b = group(&[(5.0, 1.1), (7.0, 1.1)]);
        assert!(close(a.variance, 0.875, 1e-14) && close(b.variance, 2.2 / 1.2, 1e-14));
        let pooled: f64 = (3.0 * 0.875 + 2.2 / 1.2) / 4.0;
        let t = 4.0 / (pooled * 0.75).sqrt();
        let (pairs, _) = means(&[a, b], true, None);
        assert!(close(pairs[0].p, t_closed_form(t, 4), 1e-10));
    }

    #[test]
    fn means_skip_light_categories_and_zero_variance() {
        let one = group(&[(3.0, 1.0)]);
        let flat = group(&[(2.0, 1.0), (2.0, 1.0)]);
        let also_flat = group(&[(5.0, 1.0), (5.0, 1.0)]);
        let (pairs, tested) = means(&[one, flat, also_flat], true, None);
        assert_eq!(tested, [false, true, true]);
        assert!(pairs.is_empty(), "no spread at all: no comparison");
        let empty = group(&[]);
        assert_eq!(means(&[empty, flat], false, None).1, [false, true]);
    }

    #[test]
    fn bonferroni_and_benjamini_hochberg() {
        // m = 4. Bonferroni: 4 p, at most 1. BH: sorted 0.005, 0.01, 0.03, 0.04 give
        // 4 p / rank = 0.02, 0.02, 0.04, 0.04, each the minimum of itself and those above.
        let p = [0.01, 0.04, 0.03, 0.005];
        let b = adjust(&p, Adjust::Bonferroni);
        for (got, want) in b.iter().zip([0.04, 0.16, 0.12, 0.02]) {
            assert!(close(*got, want, 1e-12));
        }
        let bh = adjust(&p, Adjust::BenjaminiHochberg);
        for (got, want) in bh.iter().zip([0.02, 0.04, 0.04, 0.02]) {
            assert!(close(*got, want, 1e-12), "{bh:?}");
        }
        assert_eq!(adjust(&[0.6, 0.7], Adjust::Bonferroni), [1.0, 1.0]);
        // a later small p pulls an earlier larger one down: 0.03 at rank 2 of 2 stays, and
        // 0.02 at rank 1 becomes min(0.03, 0.04)
        assert_eq!(
            adjust(&[0.02, 0.03], Adjust::BenjaminiHochberg),
            [0.03, 0.03]
        );
        assert_eq!(adjust(&p, Adjust::None), p);
    }

    #[test]
    fn letter_case_follows_the_two_levels() {
        let levels = [0.01, 0.05];
        assert_eq!(significance(0.005, &levels, Adjust::None), Some(true));
        assert_eq!(significance(0.03, &levels, Adjust::None), Some(false));
        assert_eq!(significance(0.05, &levels, Adjust::Bonferroni), None);
        // the step-up rule of Benjamini-Hochberg includes the level itself
        assert_eq!(
            significance(0.05, &[0.05], Adjust::BenjaminiHochberg),
            Some(true)
        );
        assert_eq!(significance(0.2, &[0.05], Adjust::None), None);
    }

    #[test]
    fn apa_letters_by_insert_and_absorb() {
        let letters = |members: &[usize], pairs: &[(usize, usize)]| {
            let found = apa_letters(members, pairs);
            members.iter().map(|m| found[m].clone()).collect::<Vec<_>>()
        };
        // no difference: one letter for all
        assert_eq!(letters(&[0, 1, 2], &[]), ["a", "a", "a"]);
        // the first and last differ, the middle one differs from neither
        assert_eq!(letters(&[0, 1, 2], &[(0, 2)]), ["a", "ab", "b"]);
        // every pair differs: the letters follow the order of the columns
        assert_eq!(
            letters(&[0, 1, 2], &[(1, 0), (2, 0), (2, 1)]),
            ["a", "b", "c"]
        );
        // 0 differs from 2 and 3, 1 from 3: {0, 1}, {1, 2}, {2, 3}
        assert_eq!(
            letters(&[0, 1, 2, 3], &[(0, 2), (0, 3), (1, 3)]),
            ["a", "ab", "bc", "c"]
        );
    }

    #[test]
    fn effective_base_proportions_use_the_bases_and_the_shared_base() {
        // 30 of 50 against 20 of 50, effective bases 40 and 25 sharing 5: pooled 0.5, factor
        // 1/40 + 1/25 - 2 * 5 / (40 * 25) = 0.055, df 40 + 25 - 5 - 2 = 58
        let columns = [
            BaseColumn {
                count: 30.0,
                total: 50.0,
                base: 40.0,
                cases: 50.0,
            },
            BaseColumn {
                count: 20.0,
                total: 50.0,
                base: 25.0,
                cases: 50.0,
            },
        ];
        let (pairs, tested) = proportions_effective(&columns, &|_, _| 5.0);
        let t = 0.2 / (0.25_f64 * 0.055).sqrt();
        assert_eq!(tested, [true, true]);
        assert!(close(pairs[0].p, t_p(t, 58.0), 1e-13));
        // a column of one case is compared only with columns of two or more; proportions of 0
        // and 1 take part
        let one = |count: f64, cases: f64| BaseColumn {
            count,
            total: cases,
            base: cases,
            cases,
        };
        let (pairs, tested) = proportions_effective(
            &[one(1.0, 1.0), one(0.0, 1.0), one(1.0, 3.0), one(0.0, 0.0)],
            NONE_SHARED,
        );
        assert_eq!(tested, [true, true, true, false]);
        let compared: Vec<(usize, usize)> = pairs
            .iter()
            .map(|p| (p.larger.min(p.smaller), p.larger.max(p.smaller)))
            .collect();
        assert_eq!(compared, [(0, 2), (1, 2)]);
    }

    #[test]
    fn effective_base_means_adjust_the_variance_to_the_base() {
        // unit weights: the base is the number of cases and the test is the unweighted one,
        // here A = {1, 2, 3} against C = {2, 4, 6} with one shared case (see the set test above)
        let a = group(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)]);
        let c = group(&[(2.0, 1.0), (4.0, 1.0), (6.0, 1.0)]);
        let (pairs, _) = means_effective(&[a, c], true, Some(&|_, _| 1.0));
        let t = 2.0 / (2.5_f64 * 4.0 / 9.0).sqrt();
        assert!(close(pairs[0].p, t_closed_form(t, 3), 1e-10));
        // weights 1 and 3: w = 4, q = 10, e = 1.6; mean (1 + 9) / 4 = 2.5, squared deviations
        // 2.25 + 0.75 = 3, adjusted room 4 - 4 / 1.6 = 1.5. B = {5, 7} at 1: e = 2, room 1,
        // deviations 2. Pooled from the pair: 5 / 2.5 = 2, factor 1/1.6 + 1/2, df 1.6 + 2 - 2.
        let a = group(&[(1.0, 1.0), (3.0, 3.0)]);
        let b = group(&[(5.0, 1.0), (7.0, 1.0)]);
        let (pairs, tested) = means_effective(&[a, b], false, None);
        assert_eq!(tested, [true, true]);
        let t = (2.5 - 6.0) / (2.0_f64 * (1.0 / 1.6 + 0.5)).sqrt();
        assert!(close(pairs[0].p, t_p(t, 1.6), 1e-12));
    }

    #[test]
    fn letters_go_past_z() {
        let keys: Vec<String> = [0, 1, 25, 26, 27, 51, 52, 701, 702].map(letter).into();
        assert_eq!(keys, ["A", "B", "Z", "AA", "AB", "AZ", "BA", "ZZ", "AAA"]);
    }
}
