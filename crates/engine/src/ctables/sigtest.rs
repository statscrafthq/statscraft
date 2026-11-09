//! Significance tests of CTABLES: the chi-square test of independence (`/SIGTEST`) and the
//! pairwise comparisons of column proportions and column means (`/COMPARETEST`).
//!
//! The rules, in our words, from the command reference (CTABLES, SIGTEST and COMPARETEST, and
//! "Weights and Rounding") and the CTABLES chapter of the algorithms reference:
//! - Every test runs inside an innermost subtable: the innermost categorical variable of the rows
//!   against the innermost categorical variable of the columns, once for every combination of
//!   categories of the outer variables of both axes (the total of an outer variable is a subtable
//!   too). Totals of the two tested variables take no part in the test.
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
//! - Every column of the table gets a letter. For a significant pair the letter of the column
//!   with the smaller proportion or mean goes into the column with the larger one: uppercase when
//!   significant at the smaller of two ALPHA levels (or at the only one), lowercase at the larger.

use super::{CategoryList, Layout, Section, Term, combinations, list_of};
use crate::crosstabs::{chi_square_p, gamma_q, ln_gamma};
use crate::{Cell, Error, Table};
use statscraft_syntax::{Adjust, CompareKind, Comparetest, CtablesTable, Sigtest, Statistic};
use std::collections::HashMap;

// ---- distributions ----

/// The complementary error function: erfc(x) = Q(1/2, x^2) for x >= 0.
pub(crate) fn erfc(x: f64) -> f64 {
    let q = gamma_q(0.5, x * x);
    if x < 0.0 { 2.0 - q } else { q }
}

/// Two-sided significance of a standard normal z.
pub(crate) fn normal_p(z: f64) -> f64 {
    erfc(z.abs() / std::f64::consts::SQRT_2)
}

/// Two-sided significance of Student's t with `df` degrees of freedom: I_x(df/2, 1/2) at
/// x = df / (df + t^2).
pub(crate) fn t_p(t: f64, df: f64) -> f64 {
    beta_i(df / 2.0, 0.5, df / (df + t * t))
}

/// The regularized incomplete beta function I_x(a, b), by its continued fraction on the side of
/// x where it converges fast.
pub(crate) fn beta_i(a: f64, b: f64, x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_fraction(b, a, 1.0 - x) / b
    }
}

/// The continued fraction of I_x(a, b), evaluated by the modified Lentz method.
fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let guard = |v: f64| if v.abs() < TINY { TINY } else { v };
    let mut c = 1.0;
    let mut d = 1.0 / guard(1.0 - (a + b) * x / (a + 1.0));
    let mut h = d;
    for m in 1..100_000 {
        let m = m as f64;
        let even = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
        d = 1.0 / guard(1.0 + even * d);
        c = guard(1.0 + even / c);
        h *= d * c;
        let odd = -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0));
        d = 1.0 / guard(1.0 + odd * d);
        c = guard(1.0 + odd / c);
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

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
    let rows: Vec<&Vec<f64>> = counts
        .iter()
        .filter(|r| r.iter().sum::<f64>() > 0.0)
        .collect();
    let columns: Vec<usize> = (0..counts.first()?.len())
        .filter(|&j| rows.iter().map(|r| r[j]).sum::<f64>() > 0.0)
        .collect();
    if rows.len() < 2 || columns.len() < 2 {
        return None;
    }
    let row_totals: Vec<f64> = rows
        .iter()
        .map(|r| columns.iter().map(|&j| r[j]).sum())
        .collect();
    let column_totals: Vec<f64> = columns
        .iter()
        .map(|&j| rows.iter().map(|r| r[j]).sum())
        .collect();
    let total: f64 = row_totals.iter().sum();
    let mut value = 0.0;
    for (row, row_total) in rows.iter().zip(&row_totals) {
        for (&j, column_total) in columns.iter().zip(&column_totals) {
            let expected = row_total * column_total / total;
            value += (row[j] - expected).powi(2) / expected;
        }
    }
    let df = ((rows.len() - 1) * (columns.len() - 1)) as f64;
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

/// Column proportions test of one row: `counts[j]` is the weighted count of the row in column j,
/// `totals[j]` the weighted total of column j. Returns the pairs and which columns took part.
pub(crate) fn proportions(counts: &[f64], totals: &[f64]) -> (Vec<Pair>, Vec<bool>) {
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
            let pooled = (nj * share(j) + nk * share(k)) / (nj + nk);
            let z =
                (share(j) - share(k)) / (pooled * (1.0 - pooled) * (1.0 / nj + 1.0 / nk)).sqrt();
            pairs.push(pair(j, k, share(j), share(k), normal_p(z)));
        }
    }
    (pairs, tested)
}

/// The weighted size, mean and variance (divided by the weight minus 1) of one category.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Group {
    pub weight: f64,
    pub mean: f64,
    pub variance: f64,
}

/// The group of the valid (value, weight) pairs of a category.
pub(crate) fn group(values: &[(f64, f64)]) -> Group {
    let weight: f64 = values.iter().map(|(_, w)| w).sum();
    let mean = values.iter().map(|(x, w)| x * w).sum::<f64>() / weight;
    let squares: f64 = values.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
    Group {
        weight,
        mean,
        variance: squares / (weight - 1.0),
    }
}

/// Column means test of one subtable. `pool_all` pools the variance from every category taking
/// part, else from the two compared. Returns the pairs and which categories took part.
pub(crate) fn means(groups: &[Group], pool_all: bool) -> (Vec<Pair>, Vec<bool>) {
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
            let (variance, df) = if pool_all {
                (pooled, df_all)
            } else {
                let df = n(gi) + n(gj) - 2.0;
                let joint = (n(gi) - 1.0) * gi.variance + (n(gj) - 1.0) * gj.variance;
                (joint / df, df)
            };
            // without spread the test is undefined, and the comparison is not counted
            if variance.is_nan() || variance <= 0.0 {
                continue;
            }
            let t = (gi.mean - gj.mean) / (variance * (1.0 / n(gi) + 1.0 / n(gj))).sqrt();
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

/// The positions of the categories of a list that are not its total.
fn proper(list: &CategoryList) -> Vec<usize> {
    (0..list.categories.len())
        .filter(|&i| list.categories[i].key.is_some())
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

/// The test tables of one CTABLES table, in the order of their subcommands' kinds: chi-square,
/// then the comparisons.
pub(super) fn tables(
    table: &CtablesTable,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &HashMap<(usize, usize), Section>,
) -> Result<Vec<Table>, Error> {
    let tested = table.sigtest.is_some() || !table.comparetests.is_empty();
    let set_in_table = layout
        .rows
        .iter()
        .chain(&layout.columns)
        .flat_map(|term| term.iter())
        .find(|var| var.set.is_some());
    if let (true, Some(var)) = (tested, set_in_table) {
        return Err(Error(format!(
            "significance tests on a multiple response set ({}) are not supported yet; \
             leave out SIGTEST and COMPARETEST or the set",
            var.variable.name()
        )));
    }
    let mut out = Vec::new();
    if let Some(spec) = &table.sigtest {
        out.push(chi_square_table(spec, layout, lists, sections)?);
    }
    for spec in &table.comparetests {
        out.push(comparison_table(spec, layout, lists, sections)?);
    }
    Ok(out)
}

/// A section whose rows and columns are both categorical, without a scale variable.
fn categorical_section(row: &Term, column: &Term, section: &Section) -> bool {
    !section.has_scale && row.iter().any(|v| !v.scale) && column.iter().any(|v| !v.scale)
}

fn chi_square_table(
    spec: &Sigtest,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &HashMap<(usize, usize), Section>,
) -> Result<Table, Error> {
    let mut rows = Vec::new();
    let mut untested = false;
    for (r, row_term) in layout.rows.iter().enumerate() {
        for (c, column_term) in layout.columns.iter().enumerate() {
            let section = &sections[&(r, c)];
            if !categorical_section(row_term, column_term, section) {
                continue;
            }
            let (row_lists, column_lists) = (
                category_lists(row_term, lists),
                category_lists(column_term, lists),
            );
            let (nr, nc) = (row_lists.len(), column_lists.len());
            let inner_rows = proper(row_lists[nr - 1]);
            let inner_columns = proper(column_lists[nc - 1]);
            for row_outer in combinations(&row_lists[..nr - 1]) {
                for column_outer in combinations(&column_lists[..nc - 1]) {
                    let counts: Vec<Vec<f64>> = inner_rows
                        .iter()
                        .map(|&i| {
                            inner_columns
                                .iter()
                                .map(|&j| {
                                    let key =
                                        [&row_outer[..], &[i][..], &column_outer[..], &[j][..]]
                                            .concat();
                                    // whole cases: weighted cell counts are rounded for the test
                                    section.cells.get(&key).map_or(0.0, |t| t.count.round())
                                })
                                .collect()
                        })
                        .collect();
                    let mut cells = vec![
                        Cell::Text(side_label(row_term, lists, &row_outer)),
                        Cell::Text(side_label(column_term, lists, &column_outer)),
                    ];
                    match pearson(&counts) {
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
    if rows.is_empty() {
        return Err(Error(
            "/SIGTEST: the chi-square test needs a categorical variable in both the rows and the columns of one nesting, without a scale variable".into(),
        ));
    }
    let mut caption = format!(
        "Pearson chi-square test of independence in every subtable; rows and columns without cases are left out. * The statistic is significant at the {} level.",
        spec.alpha
    );
    if untested {
        caption.push_str(" A subtable with fewer than two nonempty rows or columns is not tested.");
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
    })
}

/// One column of a comparisons table: a category of the innermost column variable.
struct Column {
    term: usize,
    combo: Vec<usize>,
    path: Vec<String>,
}

/// The outcome of one family of comparisons, by category position of the innermost column
/// variable: whether it took part, and the significant smaller categories with their adjusted
/// p-value and case (true for uppercase).
#[derive(Default)]
struct Family {
    tested: HashMap<usize, bool>,
    marks: HashMap<usize, Vec<(usize, f64, bool)>>,
}

fn comparison_table(
    spec: &Comparetest,
    layout: &Layout,
    lists: &HashMap<String, CategoryList>,
    sections: &HashMap<(usize, usize), Section>,
) -> Result<Table, Error> {
    let means = spec.kind == CompareKind::Means;
    // which sections are tested
    let eligible = |r: usize, c: usize| {
        let (row, column) = (&layout.rows[r], &layout.columns[c]);
        let section = &sections[&(r, c)];
        if !means {
            return categorical_section(row, column, section);
        }
        let has_mean = section
            .summaries
            .iter()
            .any(|(_, s, _)| s.function.statistic == Statistic::Mean && !s.function.unweighted);
        row.iter().any(|v| v.scale) && has_mean && column.iter().any(|v| !v.scale)
    };

    // the columns of the table and their letters
    let mut columns: Vec<Column> = Vec::new();
    for (c, term) in layout.columns.iter().enumerate() {
        let term_lists = category_lists(term, lists);
        if term_lists.is_empty() || !(0..layout.rows.len()).any(|r| eligible(r, c)) {
            continue;
        }
        let inner = term_lists.len() - 1;
        for combo in combinations(&term_lists) {
            if term_lists[inner].categories[combo[inner]].key.is_some() {
                columns.push(Column {
                    term: c,
                    path: path(term, lists, &combo),
                    combo,
                });
            }
        }
    }
    let letters: HashMap<(usize, &[usize]), String> = columns
        .iter()
        .enumerate()
        .map(|(i, col)| ((col.term, &col.combo[..]), letter(i)))
        .collect();

    let mut rows: Vec<(Vec<String>, Vec<Cell>)> = Vec::new();
    let mut dots = false;
    for (r, term) in layout.rows.iter().enumerate() {
        if !(0..layout.columns.len()).any(|c| eligible(r, c)) {
            continue;
        }
        let row_lists = category_lists(term, lists);
        for row_combo in combinations(&row_lists) {
            // in the proportions test a row is a category of the innermost row variable
            if !means
                && row_lists[row_lists.len() - 1].categories[row_combo[row_combo.len() - 1]]
                    .key
                    .is_none()
            {
                continue;
            }
            let mut families: HashMap<(usize, Vec<usize>), Family> = HashMap::new();
            let mut cells = Vec::new();
            for column in &columns {
                if !eligible(r, column.term) {
                    cells.push(Cell::Empty);
                    continue;
                }
                let (outer, inner) = column.combo.split_at(column.combo.len() - 1);
                let family = families
                    .entry((column.term, outer.to_vec()))
                    .or_insert_with(|| {
                        let section = &sections[&(r, column.term)];
                        let column_lists = category_lists(&layout.columns[column.term], lists);
                        let inner_columns = proper(column_lists[column_lists.len() - 1]);
                        compare(spec, section, &row_lists, &row_combo, outer, &inner_columns)
                    });
                let text = if !family.tested[&inner[0]] {
                    dots = true;
                    ".".to_string()
                } else {
                    let marks = family.marks.get(&inner[0]).map_or(&[][..], |m| &m[..]);
                    let mut parts: Vec<String> = Vec::new();
                    for &(smaller, p, upper) in marks {
                        let key = [outer, &[smaller][..]].concat();
                        let mut text = letters[&(column.term, &key[..])].clone();
                        if !upper {
                            text = text.to_lowercase();
                        }
                        if spec.show_sig {
                            text.push_str(&format!(" ({})", p_text(p)));
                        }
                        parts.push(text);
                    }
                    parts.join(" ")
                };
                cells.push(if text.is_empty() {
                    Cell::Empty
                } else {
                    Cell::Text(text)
                });
            }
            rows.push((path(term, lists, &row_combo), cells));
        }
    }
    if rows.is_empty() || columns.is_empty() {
        return Err(Error(if means {
            "/COMPARETEST TYPE=MEAN needs a scale variable summarized by MEAN in the rows and a categorical variable in the columns".into()
        } else {
            "/COMPARETEST TYPE=PROP needs a categorical variable in both the rows and the columns of one nesting, without a scale variable".into()
        }));
    }

    let row_headers = rows.iter().map(|(p, _)| p.len()).max().unwrap_or(0).max(1);
    let levels = columns.iter().map(|c| c.path.len()).max().unwrap_or(0);
    let mut header = vec![String::new(); row_headers];
    let mut column_groups = vec![vec![String::new(); levels]; row_headers];
    for (i, column) in columns.iter().enumerate() {
        header.push(format!("({})", letter(i)));
        let mut groups = column.path.clone();
        groups.resize(levels, String::new());
        column_groups.push(groups);
    }
    let table_rows = rows
        .into_iter()
        .map(|(path, cells)| {
            let mut row: Vec<Cell> = (0..row_headers)
                .map(|i| path.get(i).map_or(Cell::Empty, |t| Cell::Text(t.clone())))
                .collect();
            row.extend(cells);
            row
        })
        .collect();
    Ok(Table {
        title: if means {
            "Comparisons of Column Means".into()
        } else {
            "Comparisons of Column Proportions".into()
        },
        row_headers,
        columns: header,
        column_groups,
        rows: table_rows,
        formats: Vec::new(),
        caption: Some(comparison_caption(spec, dots)),
    })
}

/// The comparisons of one row of one subtable.
fn compare(
    spec: &Comparetest,
    section: &Section,
    row_lists: &[&CategoryList],
    row_combo: &[usize],
    column_outer: &[usize],
    inner_columns: &[usize],
) -> Family {
    let key = |row: &[usize], j: usize| [row, column_outer, &[j][..]].concat();
    let (pairs, tested) = match spec.kind {
        CompareKind::Proportions => {
            let (outer_rows, _) = row_combo.split_at(row_combo.len() - 1);
            let count =
                |row: &[usize], j: usize| section.cells.get(&key(row, j)).map_or(0.0, |t| t.count);
            let inner_rows = proper(row_lists[row_lists.len() - 1]);
            let counts: Vec<f64> = inner_columns.iter().map(|&j| count(row_combo, j)).collect();
            let totals: Vec<f64> = inner_columns
                .iter()
                .map(|&j| {
                    inner_rows
                        .iter()
                        .map(|&i| count(&[outer_rows, &[i][..]].concat(), j))
                        .sum()
                })
                .collect();
            proportions(&counts, &totals)
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
            means(&groups, spec.pool_all)
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

fn p_text(p: f64) -> String {
    if p < 0.0005 {
        "<0.001".into()
    } else {
        format!("{p:.3}")
    }
}

fn comparison_caption(spec: &Comparetest, dots: bool) -> String {
    let means = spec.kind == CompareKind::Means;
    let (what, smaller) = if means {
        let pooled = if spec.pool_all {
            "pooled from all tested categories"
        } else {
            "pooled from the two categories compared"
        };
        (
            format!("Two-sided t-tests of the equality of column means, variance {pooled}"),
            "mean",
        )
    } else {
        (
            "Two-sided z-tests of the equality of column proportions".to_string(),
            "proportion",
        )
    };
    let levels = match spec.alpha[..] {
        [a] => format!("at the {a} level"),
        [a, b] => format!("uppercase letters at the {a} level, lowercase at the {b} level"),
        _ => unreachable!("one or two levels"),
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
    let mut caption = format!(
        "{what}, {levels}. For each significant pair the letter of the column with the smaller {smaller} appears in the column with the larger one. Significance values are {adjusted}."
    );
    if spec.show_sig {
        caption.push_str(" The adjusted significance follows each letter in parentheses.");
    }
    if dots {
        caption.push_str(if means {
            " A period marks a category that weighs less than 2 and is not compared."
        } else {
            " A period marks a column that is not compared in its row: its proportion is 0 or 1, or its total weighs less than 2."
        });
    }
    caption
}

#[cfg(test)]
mod tests {
    //! The expected values are worked out by hand from the formulas, or are textbook values of
    //! the normal and t distributions.

    use super::*;

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
        let (pairs, tested) = proportions(&[30.0, 20.0], &[50.0, 50.0]);
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
        let (pairs, _) = proportions(&[12.4, 6.2], &[20.4, 19.6]);
        assert!(close(pairs[0].p, normal_p(z), 1e-13));
        assert!(close(z, 1.849_1, 1e-4), "{z}");
    }

    #[test]
    fn proportions_leave_out_zero_one_and_small_columns() {
        // column 1 is all of its total, column 2 none, column 3 weighs 1.5 in total
        let (pairs, tested) = proportions(&[3.0, 10.0, 0.0, 1.0], &[6.0, 10.0, 8.0, 1.5]);
        assert_eq!(tested, [true, false, false, false]);
        assert!(pairs.is_empty());
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
        let (pairs, tested) = means(&[a, b, c], true);
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
        let (pairs, _) = means(&[a, c], false);
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
        let (pairs, _) = means(&[a, b], true);
        assert!(close(pairs[0].p, t_closed_form(t, 4), 1e-10));
    }

    #[test]
    fn means_skip_light_categories_and_zero_variance() {
        let one = group(&[(3.0, 1.0)]);
        let flat = group(&[(2.0, 1.0), (2.0, 1.0)]);
        let also_flat = group(&[(5.0, 1.0), (5.0, 1.0)]);
        let (pairs, tested) = means(&[one, flat, also_flat], true);
        assert_eq!(tested, [false, true, true]);
        assert!(pairs.is_empty(), "no spread at all: no comparison");
        let empty = group(&[]);
        assert_eq!(means(&[empty, flat], false).1, [false, true]);
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
    fn letters_go_past_z() {
        let keys: Vec<String> = [0, 1, 25, 26, 27, 51, 52, 701, 702].map(letter).into();
        assert_eq!(keys, ["A", "B", "Z", "AA", "AB", "AZ", "BA", "ZZ", "AAA"]);
    }
}
