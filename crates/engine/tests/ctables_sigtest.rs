//! CTABLES significance tests (/SIGTEST and /COMPARETEST) end to end on the survey fixtures.
//! The expected statistics, p-values and letters are computed here from the raw values with plain
//! arithmetic (Simpson's rule for the normal tail, the closed forms of the t distribution), not
//! with the engine.
//!
//! The rules checked, in our words:
//! - every test stays inside an innermost subtable: the columns of one category of an outer
//!   column variable are compared only with each other, and each category of an outer row
//!   variable (its total too) is a subtable of its own;
//! - chi-square leaves out empty rows and columns and, with weights, rounds every cell count;
//! - the proportions test compares, row by row, the columns whose proportion lies strictly between
//!   0 and 1 and whose total weighs 2 or more, with the column totals rounded; a period marks the
//!   others;
//! - the means test compares the categories weighing 2 or more, with rounded weights and the
//!   variance pooled from all of them or from the pair;
//! - Bonferroni multiplies by the number of comparisons in a row of a subtable, Benjamini-Hochberg
//!   is the step-up rule; uppercase letters below the smaller level, lowercase below the larger.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(file: &str, source: &str) -> Result<Vec<Table>, String> {
    let mut engine = Engine::default();
    let source = format!("GET FILE='{}'.\n{source}", fixture(file));
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for c in &commands {
        tables.extend(engine.execute(&c.command).map_err(|e| e.to_string())?);
    }
    Ok(tables)
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Empty => String::new(),
        Cell::Number(n) => panic!("a number where text was expected: {n}"),
    }
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

/// One case of a fixture: the values the tests need, user-missing values as None.
struct Case {
    resp_id: f64,
    gender: f64,
    age: f64,
    region: f64,
    q1: Option<f64>,
    weight: f64,
    /// The channels q5_1 to q5_5 holding 1: the categories of the dichotomy set $q5.
    q5: Vec<usize>,
    /// The distinct values of q6_mention1 to 3: the categories of the category set $q6.
    q6: Vec<f64>,
}

/// The cases of a fixture; the weight is 1 when the file has no weight variable.
fn cases(file: &str) -> Vec<Case> {
    let data = Dataset::read_sav(fixture(file)).unwrap();
    let column = |name: &str| -> Vec<Option<f64>> {
        let variable = data.variable(name).unwrap();
        let values = variable.numeric().unwrap();
        values
            .iter()
            .map(|v| v.filter(|v| !variable.is_user_missing(*v)))
            .collect()
    };
    let weighted = data.weight_variable().is_some();
    let (ids, gender, age, region, q1, weight) = (
        column("resp_id"),
        column("gender"),
        column("age"),
        column("region"),
        column("q1_satisfaction"),
        column("weight"),
    );
    let q5: Vec<Vec<Option<f64>>> = (1..=5).map(|k| column(&format!("q5_{k}"))).collect();
    let q6: Vec<Vec<Option<f64>>> = (1..=3).map(|k| column(&format!("q6_mention{k}"))).collect();
    (0..data.case_count())
        .map(|i| {
            let mut mentions: Vec<f64> = q6.iter().filter_map(|v| v[i]).collect();
            mentions.sort_by(f64::total_cmp);
            mentions.dedup();
            Case {
                resp_id: ids[i].unwrap(),
                gender: gender[i].unwrap(),
                age: age[i].unwrap(),
                region: region[i].unwrap(),
                q1: q1[i],
                weight: if weighted { weight[i].unwrap() } else { 1.0 },
                q5: (0..5).filter(|&k| q5[k][i] == Some(1.0)).collect(),
                q6: mentions,
            }
        })
        .collect()
}

/// The derived variables of the syntax below, computed the same way here.
const DERIVED: &str = "COMPUTE x = (age / 3 + MOD(resp_id, 37) >= 30).\n\
                       COMPUTE agegrp = 1 + (age >= 40) + (age >= 60).\n\
                       COMPUTE y = age / 50 + MOD(resp_id, 7).\n";

impl Case {
    fn x(&self) -> f64 {
        f64::from(u8::from(self.age / 3.0 + self.resp_id % 37.0 >= 30.0))
    }
    fn agegrp(&self) -> f64 {
        1.0 + f64::from(u8::from(self.age >= 40.0)) + f64::from(u8::from(self.age >= 60.0))
    }
    fn y(&self) -> f64 {
        self.age / 50.0 + self.resp_id % 7.0
    }
}

/// Two-sided p of a standard normal z: twice the upper tail, by Simpson's rule.
fn normal_p(z: f64) -> f64 {
    let (from, steps) = (z.abs(), 40_000);
    let h = 40.0 / f64::from(steps);
    let density = |x: f64| (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let mut sum = density(from) + density(from + 40.0);
    for i in 1..steps {
        sum += density(from + f64::from(i) * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    2.0 * sum * h / 3.0
}

/// Two-sided p of Student's t with whole degrees of freedom, by the closed forms: with
/// theta = atan(|t| / sqrt(df)), P(|T| < |t|) is a finite series in sin and cos of theta.
fn t_p(t: f64, df: f64) -> f64 {
    let df = df as u32;
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

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Bonferroni,
    Bh,
    Unadjusted,
}

/// The cells of one family of comparisons. `stats` are the proportions or means, `tested` which
/// columns take part, `pairs` the comparisons (j, k, unadjusted p), `first` the position of the
/// family's first column among all columns of the table.
fn letters(
    stats: &[f64],
    tested: &[bool],
    pairs: &[(usize, usize, f64)],
    alpha: &[f64],
    mode: Mode,
    first: usize,
    show: bool,
) -> Vec<String> {
    let m = pairs.len() as f64;
    let bh = mode == Mode::Bh;
    let adjusted: Vec<f64> = if mode == Mode::Unadjusted {
        pairs.iter().map(|p| p.2).collect()
    } else if bh {
        let mut order: Vec<usize> = (0..pairs.len()).collect();
        order.sort_by(|a, b| pairs[*a].2.total_cmp(&pairs[*b].2));
        let mut out = vec![0.0; pairs.len()];
        let mut smallest = 1.0_f64;
        for (rank, &i) in order.iter().enumerate().rev() {
            smallest = smallest.min(pairs[i].2 * m / (rank + 1) as f64);
            out[i] = smallest;
        }
        out
    } else {
        pairs.iter().map(|p| (p.2 * m).min(1.0)).collect()
    };
    let below = |p: f64, a: f64| if bh { p <= a } else { p < a };
    let mut found: Vec<Vec<(usize, String)>> = vec![Vec::new(); stats.len()];
    for (&(j, k, _), p) in pairs.iter().zip(adjusted) {
        let (larger, smaller) = if stats[j] >= stats[k] { (j, k) } else { (k, j) };
        let key = (b'A' + (first + smaller) as u8) as char;
        let mut mark = if below(p, alpha[0]) {
            key.to_string()
        } else if alpha.len() > 1 && below(p, alpha[1]) {
            key.to_ascii_lowercase().to_string()
        } else {
            continue;
        };
        if show {
            let shown = if p < 0.0005 {
                "<0.001".to_string()
            } else {
                format!("{p:.3}")
            };
            mark.push_str(&format!(" ({shown})"));
        }
        found[larger].push((smaller, mark));
    }
    (0..stats.len())
        .map(|j| {
            if !tested[j] {
                return ".".into();
            }
            found[j].sort();
            let marks: Vec<&str> = found[j].iter().map(|(_, m)| m.as_str()).collect();
            marks.join(" ")
        })
        .collect()
}

/// The values of the columns, which take part, and the pairs (j, k, unadjusted p).
type Family = (Vec<f64>, Vec<bool>, Vec<(usize, usize, f64)>);

/// The proportions family of the cases in `rows` (one category of the row variable) against all
/// cases, over the column groups.
fn proportion_family(
    all: &[&Case],
    in_row: impl Fn(&Case) -> bool,
    column: impl Fn(&Case) -> f64,
    groups: &[f64],
) -> Family {
    let weight = |pick: &dyn Fn(&Case) -> bool| -> f64 {
        all.iter().filter(|c| pick(c)).map(|c| c.weight).sum()
    };
    let counts: Vec<f64> = groups
        .iter()
        .map(|&g| weight(&|c| column(c) == g && in_row(c)))
        .collect();
    let totals: Vec<f64> = groups
        .iter()
        .map(|&g| weight(&|c| column(c) == g))
        .collect();
    let shares: Vec<f64> = counts.iter().zip(&totals).map(|(f, c)| f / c).collect();
    let tested: Vec<bool> = (0..groups.len())
        .map(|j| totals[j] >= 2.0 && shares[j] > 0.0 && shares[j] < 1.0)
        .collect();
    let mut pairs = Vec::new();
    for j in 0..groups.len() {
        for k in j + 1..groups.len() {
            if tested[j] && tested[k] {
                let (nj, nk) = (totals[j].round(), totals[k].round());
                let pooled = (nj * shares[j] + nk * shares[k]) / (nj + nk);
                let se = (pooled * (1.0 - pooled) * (1.0 / nj + 1.0 / nk)).sqrt();
                pairs.push((j, k, normal_p((shares[j] - shares[k]) / se)));
            }
        }
    }
    (shares, tested, pairs)
}

/// The means family of y over the column groups: weight, mean and variance of each group, then
/// t-tests with the variance pooled from all groups or from each pair.
fn mean_family(
    cases: &[&Case],
    column: impl Fn(&Case) -> f64,
    groups: &[f64],
    pool_all: bool,
) -> Family {
    let mut stats = Vec::new();
    for &g in groups {
        let members: Vec<&&Case> = cases.iter().filter(|c| column(c) == g).collect();
        let w: f64 = members.iter().map(|c| c.weight).sum();
        let mean = members.iter().map(|c| c.weight * c.y()).sum::<f64>() / w;
        let ss: f64 = members
            .iter()
            .map(|c| c.weight * (c.y() - mean).powi(2))
            .sum();
        stats.push((w, mean, ss / (w - 1.0)));
    }
    let tested: Vec<bool> = stats.iter().map(|s| s.0 >= 2.0).collect();
    let n = |j: usize| stats[j].0.round();
    let chosen: Vec<usize> = (0..groups.len()).filter(|&j| tested[j]).collect();
    let df_all = chosen.iter().map(|&j| n(j)).sum::<f64>() - chosen.len() as f64;
    let pooled_all = chosen
        .iter()
        .map(|&j| (n(j) - 1.0) * stats[j].2)
        .sum::<f64>()
        / df_all;
    let mut pairs = Vec::new();
    for (a, &j) in chosen.iter().enumerate() {
        for &k in &chosen[a + 1..] {
            let (variance, df) = if pool_all {
                (pooled_all, df_all)
            } else {
                let df = n(j) + n(k) - 2.0;
                (
                    ((n(j) - 1.0) * stats[j].2 + (n(k) - 1.0) * stats[k].2) / df,
                    df,
                )
            };
            let t = (stats[j].1 - stats[k].1) / (variance * (1.0 / n(j) + 1.0 / n(k))).sqrt();
            pairs.push((j, k, t_p(t, df)));
        }
    }
    (stats.iter().map(|s| s.1).collect(), tested, pairs)
}

/// The cells of a comparisons table after its row headers.
fn cells(table: &Table, row: usize) -> Vec<String> {
    table.rows[row][table.row_headers..]
        .iter()
        .map(text)
        .collect()
}

/// Pearson's chi-square of a table of counts, leaving out empty rows and columns.
fn pearson(counts: &[Vec<f64>]) -> (f64, f64) {
    let rows: Vec<&Vec<f64>> = counts
        .iter()
        .filter(|r| r.iter().sum::<f64>() > 0.0)
        .collect();
    let columns: Vec<usize> = (0..counts[0].len())
        .filter(|&j| rows.iter().map(|r| r[j]).sum::<f64>() > 0.0)
        .collect();
    let total: f64 = rows.iter().map(|r| r.iter().sum::<f64>()).sum();
    let mut value = 0.0;
    for r in &rows {
        let row_total: f64 = r.iter().sum();
        for &j in &columns {
            let column_total: f64 = rows.iter().map(|r| r[j]).sum();
            let expected = row_total * column_total / total;
            value += (r[j] - expected).powi(2) / expected;
        }
    }
    (value, ((rows.len() - 1) * (columns.len() - 1)) as f64)
}

#[test]
fn chi_square_equals_the_pearson_chi_square_of_crosstabs_in_every_subtable() {
    let tables = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender > q1_satisfaction BY region /SIGTEST TYPE=CHISQUARE.\n\
         CROSSTABS /TABLES=q1_satisfaction BY region BY gender /STATISTICS=CHISQ.",
    )
    .unwrap();
    let chi = &tables[1];
    assert_eq!(chi.title, "Pearson Chi-Square Tests");
    assert_eq!(chi.rows.len(), 3, "one test per gender");
    assert_eq!(
        text(&chi.rows[0][0]),
        "Gender: Male > Q1. Overall, how satisfied are you with our service?"
    );
    assert_eq!(text(&chi.rows[0][1]), "Region of residence");
    let crosstabs = tables.last().unwrap();
    let pearson_rows: Vec<&Vec<Cell>> = crosstabs
        .rows
        .iter()
        .filter(|r| text(&r[1]) == "Pearson Chi-Square")
        .collect();
    assert_eq!(pearson_rows.len(), 3);
    let all = cases("survey_basic.sav");
    for (row, crosstab) in chi.rows.iter().zip(pearson_rows) {
        let (value, df, p) = (number(&row[2]), number(&row[3]), number(&row[4]));
        assert!((value - number(&crosstab[2])).abs() < 1e-12 * value);
        assert_eq!(df, number(&crosstab[3]));
        assert!((p - number(&crosstab[4])).abs() < 1e-12);
        assert_eq!(text(&row[5]), if p < 0.05 { "*" } else { "" });
        // and from the raw values
        let gender = match text(&crosstab[0]).as_str() {
            "Male" => 1.0,
            "Female" => 2.0,
            _ => 3.0,
        };
        let counts: Vec<Vec<f64>> = (1..=5)
            .map(|q| {
                (1..=5)
                    .map(|r| {
                        let hits = all.iter().filter(|c| {
                            c.gender == gender
                                && c.q1 == Some(f64::from(q))
                                && c.region == f64::from(r)
                        });
                        hits.count() as f64
                    })
                    .collect()
            })
            .collect();
        let (own, own_df) = pearson(&counts);
        assert!((value - own).abs() < 1e-10 * own, "{value} vs {own}");
        assert_eq!(df, own_df);
    }
}

#[test]
fn weighted_chi_square_rounds_the_cell_counts() {
    let tables = run(
        "survey_basic_phpwriter.sav",
        "CTABLES /TABLE q1_satisfaction BY region /SIGTEST TYPE=CHISQUARE ALPHA=0.5.",
    )
    .unwrap();
    let row = &tables[1].rows[0];
    let all = cases("survey_basic_phpwriter.sav");
    let sums: Vec<Vec<f64>> = (1..=5)
        .map(|q| {
            (1..=5)
                .map(|r| {
                    let cell = all
                        .iter()
                        .filter(|c| c.q1 == Some(f64::from(q)) && c.region == f64::from(r));
                    cell.map(|c| c.weight).sum()
                })
                .collect()
        })
        .collect();
    let rounded: Vec<Vec<f64>> = sums
        .iter()
        .map(|r| r.iter().map(|f| f.round()).collect())
        .collect();
    let (value, df) = pearson(&rounded);
    assert!(
        (number(&row[2]) - value).abs() < 1e-10 * value,
        "{row:?} {value}"
    );
    assert_eq!(number(&row[3]), df);
    // the rounding matters: the weights as they are give another statistic
    assert!((pearson(&sums).0 - value).abs() > 1e-3);
    assert_eq!(text(&row[5]), if number(&row[4]) < 0.5 { "*" } else { "" });
}

/// The proportions test of x by gender > agegrp, against the computation from the raw values.
fn check_proportions(file: &str) -> usize {
    let tables = run(
        file,
        &format!(
            "{DERIVED}CTABLES /TABLE x BY gender > agegrp\n\
             /COMPARETEST TYPE=PROP ALPHA=0.01, 0.05 SHOWSIG=YES."
        ),
    )
    .unwrap();
    let t = &tables[1];
    assert_eq!(t.title, "Comparisons of Column Proportions");
    // nine columns: three age groups under each gender, lettered across the table
    let keys: Vec<&str> = t.columns[t.row_headers..]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "(A)", "(B)", "(C)", "(D)", "(E)", "(F)", "(G)", "(H)", "(I)"
        ]
    );
    assert_eq!(
        t.column_groups[t.row_headers + 3],
        ["Gender", "Female", "agegrp", "1.00"]
    );
    let all = cases(file);
    let mut marks = 0;
    for (row, x) in [0.0, 1.0].into_iter().enumerate() {
        let mut want = Vec::new();
        for (g, gender) in [1.0, 2.0, 3.0].into_iter().enumerate() {
            let subtable: Vec<&Case> = all.iter().filter(|c| c.gender == gender).collect();
            let (shares, tested, pairs) =
                proportion_family(&subtable, |c| c.x() == x, Case::agegrp, &[1.0, 2.0, 3.0]);
            want.extend(letters(
                &shares,
                &tested,
                &pairs,
                &[0.01, 0.05],
                Mode::Bonferroni,
                3 * g,
                true,
            ));
        }
        assert_eq!(cells(t, row), want, "row x = {x}");
        marks += want.iter().filter(|w| w.contains('(')).count();
    }
    marks
}

#[test]
fn column_proportions_unweighted() {
    assert!(
        check_proportions("survey_basic.sav") > 0,
        "some pair is significant"
    );
}

#[test]
fn column_proportions_weighted_round_the_column_totals() {
    assert!(check_proportions("survey_basic_phpwriter.sav") > 0);
}

#[test]
fn column_proportions_mark_columns_that_cannot_be_compared() {
    // in the North nobody is of the third gender, so its proportion is 0 there
    let tables = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender BY region /COMPARETEST TYPE=PROP ADJUST=NONE.",
    )
    .unwrap();
    assert_eq!(cells(&tables[1], 2)[0], ".");
    assert!(
        tables[1]
            .caption
            .as_ref()
            .unwrap()
            .contains("A period marks")
    );
}

/// The means test of y by agegrp within each gender and the total.
fn check_means(file: &str, options: &str, pool_all: bool, mode: Mode, alpha: &[f64]) -> usize {
    let tables = run(
        file,
        &format!(
            "{DERIVED}CTABLES /TABLE gender > y BY agegrp /CATEGORIES VARIABLES=gender TOTAL=YES\n\
             /COMPARETEST TYPE=MEAN {options}."
        ),
    )
    .unwrap();
    let t = &tables[1];
    assert_eq!(t.title, "Comparisons of Column Means");
    assert_eq!(t.rows.len(), 4, "three genders and their total");
    assert_eq!(text(&t.rows[3][1]), "Total");
    let all = cases(file);
    let mut marks = 0;
    for (row, gender) in [Some(1.0), Some(2.0), Some(3.0), None]
        .into_iter()
        .enumerate()
    {
        let subtable: Vec<&Case> = all
            .iter()
            .filter(|c| gender.is_none_or(|g| c.gender == g))
            .collect();
        let (means, tested, pairs) =
            mean_family(&subtable, Case::agegrp, &[1.0, 2.0, 3.0], pool_all);
        let want = letters(
            &means,
            &tested,
            &pairs,
            alpha,
            mode,
            0,
            options.contains("SHOWSIG"),
        );
        assert_eq!(cells(t, row), want, "row {row}");
        marks += want.iter().filter(|w| !w.is_empty() && *w != ".").count();
    }
    marks
}

#[test]
fn column_means_unweighted_with_variance_pooled_from_all_categories() {
    let marks = check_means(
        "survey_basic.sav",
        "ALPHA=0.001 0.05 SHOWSIG=YES",
        true,
        Mode::Bonferroni,
        &[0.001, 0.05],
    );
    assert!(marks > 0);
}

#[test]
fn column_means_weighted_pooled_by_pair_with_benjamini_hochberg() {
    let marks = check_means(
        "survey_basic_phpwriter.sav",
        "MEANSVARIANCE=TESTEDCATS ADJUST=BH ALPHA=0.01, 0.1 SHOWSIG=YES",
        false,
        Mode::Bh,
        &[0.01, 0.1],
    );
    assert!(marks > 0);
}

#[test]
fn stacked_columns_continue_the_letters_and_compare_only_within_their_variable() {
    let tables = run(
        "survey_basic.sav",
        &format!(
            "{DERIVED}CTABLES /TABLE x BY gender + agegrp /COMPARETEST TYPE=PROP ADJUST=NONE."
        ),
    )
    .unwrap();
    let t = &tables[1];
    assert_eq!(
        t.columns[t.row_headers..],
        ["(A)", "(B)", "(C)", "(D)", "(E)", "(F)"]
    );
    let all = cases("survey_basic.sav");
    let everyone: Vec<&Case> = all.iter().collect();
    for (row, x) in [0.0, 1.0].into_iter().enumerate() {
        let gender = |c: &Case| c.gender;
        let (s1, t1, p1) = proportion_family(&everyone, |c| c.x() == x, gender, &[1.0, 2.0, 3.0]);
        let (s2, t2, p2) =
            proportion_family(&everyone, |c| c.x() == x, Case::agegrp, &[1.0, 2.0, 3.0]);
        let mut want = letters(&s1, &t1, &p1, &[0.05], Mode::Unadjusted, 0, false);
        want.extend(letters(&s2, &t2, &p2, &[0.05], Mode::Unadjusted, 3, false));
        assert_eq!(cells(t, row), want);
        for cell in &cells(t, row)[..3] {
            assert!(!cell.contains(['D', 'E', 'F']), "{cell}");
        }
    }
}

// ---- multiple response sets ----
//
// The CTABLES chapter of the algorithms reference gives each test a variant for a multiple
// response set, whose categories can share cases: the margins count every case once; the
// chi-square divides each term by 1 minus the share of the set's category and keeps all the set's
// categories in df ((R - 1)C, R(C - 1) or RC); the column proportions and column means tests take
// the rounded weight two columns share off the variance factor (2 w_jk / (w_j w_k)), the means
// test also off df, and always pools the variance from the two categories compared.

const SETS: &str = "MRSETS /MDGROUP NAME=$q5 VARIABLES=q5_1 TO q5_5 VALUE=1\n\
                    /MCGROUP NAME=$q6 VARIABLES=q6_mention1 TO q6_mention3.\n";

/// Variables that depend on the first channel of $q5, computed the same way in `Case`.
const SET_DERIVED: &str = "COMPUTE z = (q5_1 + MOD(resp_id, 3) >= 2).\n\
                           COMPUTE ym = MOD(resp_id, 7) + 4 * q5_1.\n";

impl Case {
    fn z(&self) -> f64 {
        let first = f64::from(u8::from(self.q5.contains(&0)));
        f64::from(u8::from(first + self.resp_id % 3.0 >= 2.0))
    }
    fn ym(&self) -> f64 {
        self.resp_id % 7.0 + 4.0 * f64::from(u8::from(self.q5.contains(&0)))
    }
}

/// The categories of a case on one side of a table, by position.
type Side = fn(&Case) -> Vec<usize>;

fn gender_side(c: &Case) -> Vec<usize> {
    vec![c.gender as usize - 1]
}

fn q5_side(c: &Case) -> Vec<usize> {
    c.q5.clone()
}

fn q6_side(c: &Case) -> Vec<usize> {
    c.q6.iter().map(|v| *v as usize - 1).collect()
}

/// The upper tail of the chi-square distribution with an even df: e^(-x/2) times the sum of
/// (x/2)^k / k! for k below df / 2.
fn chi_square_p_even(x: f64, df: f64) -> f64 {
    assert!((df as u32).is_multiple_of(2));
    let half = x / 2.0;
    let (mut term, mut sum) = (1.0, 1.0);
    for k in 1..df as u32 / 2 {
        term *= half / f64::from(k);
        sum += term;
    }
    (-half).exp() * sum
}

/// The chi-square statistic and df of a table whose sides may be sets, from the raw values.
fn set_chi_square(
    all: &[Case],
    rows: (Side, usize, bool),
    columns: (Side, usize, bool),
) -> (f64, f64) {
    let weight = |pick: &dyn Fn(&Case) -> bool| -> f64 {
        all.iter()
            .filter(|c| pick(c))
            .map(|c| c.weight)
            .sum::<f64>()
            .round()
    };
    let (row_of, column_of) = (rows.0, columns.0);
    let total = weight(&|c| !row_of(c).is_empty() && !column_of(c).is_empty());
    let r: Vec<f64> = (0..rows.1)
        .map(|i| weight(&|c| row_of(c).contains(&i) && !column_of(c).is_empty()))
        .collect();
    let k: Vec<f64> = (0..columns.1)
        .map(|j| weight(&|c| column_of(c).contains(&j) && !row_of(c).is_empty()))
        .collect();
    let kept = |m: &[f64], set: bool| -> Vec<usize> {
        (0..m.len())
            .filter(|&i| m[i] > 0.0 && !(set && m[i] >= total))
            .collect()
    };
    let (kr, kc) = (kept(&r, rows.2), kept(&k, columns.2));
    let mut value = 0.0;
    for &i in &kr {
        for &j in &kc {
            let f = weight(&|c| row_of(c).contains(&i) && column_of(c).contains(&j));
            let e = r[i] * k[j] / total;
            let mut divisor = e;
            if rows.2 {
                divisor *= 1.0 - r[i] / total;
            }
            if columns.2 {
                divisor *= 1.0 - k[j] / total;
            }
            value += (f - e).powi(2) / divisor;
        }
    }
    let free = |n: usize, set: bool| (if set { n } else { n - 1 }) as f64;
    (value, free(kr.len(), rows.2) * free(kc.len(), columns.2))
}

#[test]
fn chi_square_with_sets_uses_the_variant_for_multiple_responses() {
    for file in ["survey_basic.sav", "survey_basic_phpwriter.sav"] {
        let all = cases(file);
        for (syntax, rows, columns) in [
            (
                "$q5 BY gender",
                (q5_side as Side, 5, true),
                (gender_side as Side, 3, false),
            ),
            (
                "gender BY $q5",
                (gender_side as Side, 3, false),
                (q5_side as Side, 5, true),
            ),
            (
                "$q5 BY $q6",
                (q5_side as Side, 5, true),
                (q6_side as Side, 6, true),
            ),
        ] {
            let tables = run(
                file,
                &format!("{SETS}CTABLES /TABLE {syntax} /SIGTEST TYPE=CHISQUARE."),
            )
            .unwrap();
            let row = &tables[1].rows[0];
            let (value, df) = set_chi_square(&all, rows, columns);
            let got = number(&row[2]);
            assert!(
                (got - value).abs() < 1e-9 * value,
                "{file} {syntax}: {got} vs {value}"
            );
            assert_eq!(number(&row[3]), df, "{file} {syntax}");
            let p = chi_square_p_even(value, df);
            assert!((number(&row[4]) - p).abs() < 1e-9, "{file} {syntax}: p {p}");
            assert!(
                tables[1]
                    .caption
                    .as_ref()
                    .unwrap()
                    .contains("multiple response set")
            );
        }
    }
}

/// The proportions family of one row against the columns of a side that may be a set:
/// `in_row` picks the row's cases, `in_subtable` the cases with any row category.
fn set_proportion_family(
    all: &[Case],
    in_row: &dyn Fn(&Case) -> bool,
    in_subtable: &dyn Fn(&Case) -> bool,
    columns: Side,
    n: usize,
) -> Family {
    let weight = |pick: &dyn Fn(&Case) -> bool| -> f64 {
        all.iter().filter(|c| pick(c)).map(|c| c.weight).sum()
    };
    let counts: Vec<f64> = (0..n)
        .map(|j| weight(&|c| in_row(c) && columns(c).contains(&j)))
        .collect();
    let totals: Vec<f64> = (0..n)
        .map(|j| weight(&|c| in_subtable(c) && columns(c).contains(&j)))
        .collect();
    let shares: Vec<f64> = counts.iter().zip(&totals).map(|(f, c)| f / c).collect();
    let tested: Vec<bool> = (0..n)
        .map(|j| totals[j] >= 2.0 && shares[j] > 0.0 && shares[j] < 1.0)
        .collect();
    let mut pairs = Vec::new();
    for j in 0..n {
        for k in j + 1..n {
            if tested[j] && tested[k] {
                let (nj, nk) = (totals[j].round(), totals[k].round());
                let both = weight(&|c| {
                    in_subtable(c) && columns(c).contains(&j) && columns(c).contains(&k)
                })
                .round();
                let pooled = (nj * shares[j] + nk * shares[k]) / (nj + nk);
                let factor = 1.0 / nj + 1.0 / nk - 2.0 * both / (nj * nk);
                let se = (pooled * (1.0 - pooled) * factor).sqrt();
                pairs.push((j, k, normal_p((shares[j] - shares[k]) / se)));
            }
        }
    }
    (shares, tested, pairs)
}

#[test]
fn column_proportions_with_a_set_in_the_columns_or_the_rows() {
    for file in ["survey_basic.sav", "survey_basic_phpwriter.sav"] {
        let all = cases(file);
        // a set in the columns: its columns share cases
        let tables = run(
            file,
            &format!("{SETS}CTABLES /TABLE gender BY $q5 /COMPARETEST TYPE=PROP ADJUST=NONE."),
        )
        .unwrap();
        for g in 0..3 {
            let in_row = |c: &Case| c.gender as usize == g + 1;
            let (s, t, p) = set_proportion_family(&all, &in_row, &|_| true, q5_side, 5);
            let want = letters(&s, &t, &p, &[0.05], Mode::Unadjusted, 0, false);
            assert_eq!(cells(&tables[1], g), want, "{file} gender {g}");
        }
        // z depends on q5_1, so its first column differs from the others
        let tables = run(
            file,
            &format!("{SETS}{SET_DERIVED}CTABLES /TABLE z BY $q5 /COMPARETEST TYPE=PROP."),
        )
        .unwrap();
        let mut marks = 0;
        for (row, z) in [0.0, 1.0].into_iter().enumerate() {
            let in_row = |c: &Case| c.z() == z;
            let (s, t, p) = set_proportion_family(&all, &in_row, &|_| true, q5_side, 5);
            let want = letters(&s, &t, &p, &[0.05], Mode::Bonferroni, 0, false);
            assert_eq!(cells(&tables[1], row), want, "{file} z {z}");
            marks += want.iter().filter(|w| !w.is_empty() && *w != ".").count();
        }
        // a set in the rows: a column total counts each case with any response once
        let tables = run(
            file,
            &format!("{SETS}CTABLES /TABLE $q5 BY gender /COMPARETEST TYPE=PROP ADJUST=NONE."),
        )
        .unwrap();
        for i in 0..5 {
            let in_row = |c: &Case| c.q5.contains(&i);
            let any = |c: &Case| !c.q5.is_empty();
            let (s, t, p) = set_proportion_family(&all, &in_row, &any, gender_side, 3);
            let want = letters(&s, &t, &p, &[0.05], Mode::Unadjusted, 0, false);
            assert_eq!(cells(&tables[1], i), want, "{file} channel {i}");
        }
        assert!(marks > 0, "{file}: some pair is significant");
    }
}

#[test]
fn column_means_over_a_set_pool_by_pair_and_take_off_the_shared_cases() {
    for file in ["survey_basic.sav", "survey_basic_phpwriter.sav"] {
        let all = cases(file);
        // ALLCATS is the default, but a set always pools from the two categories compared
        let tables = run(
            file,
            &format!(
                "{SETS}{SET_DERIVED}CTABLES /TABLE ym [S] BY $q5 /COMPARETEST TYPE=MEAN ADJUST=NONE."
            ),
        )
        .unwrap();
        let members =
            |j: usize| -> Vec<&Case> { all.iter().filter(|c| c.q5.contains(&j)).collect() };
        let stats: Vec<(f64, f64, f64)> = (0..5)
            .map(|j| {
                let m = members(j);
                let w: f64 = m.iter().map(|c| c.weight).sum();
                let mean = m.iter().map(|c| c.weight * c.ym()).sum::<f64>() / w;
                let ss: f64 = m.iter().map(|c| c.weight * (c.ym() - mean).powi(2)).sum();
                (w, mean, ss / (w - 1.0))
            })
            .collect();
        let n = |j: usize| stats[j].0.round();
        let mut pairs = Vec::new();
        for j in 0..5 {
            for k in j + 1..5 {
                let both: f64 = all
                    .iter()
                    .filter(|c| c.q5.contains(&j) && c.q5.contains(&k))
                    .map(|c| c.weight)
                    .sum::<f64>()
                    .round();
                let df = n(j) + n(k) - 2.0;
                let variance = ((n(j) - 1.0) * stats[j].2 + (n(k) - 1.0) * stats[k].2) / df;
                let factor = 1.0 / n(j) + 1.0 / n(k) - 2.0 * both / (n(j) * n(k));
                let t = (stats[j].1 - stats[k].1) / (variance * factor).sqrt();
                pairs.push((j, k, t_p(t, df - both)));
            }
        }
        let means: Vec<f64> = stats.iter().map(|s| s.1).collect();
        let want = letters(
            &means,
            &[true; 5],
            &pairs,
            &[0.05],
            Mode::Unadjusted,
            0,
            false,
        );
        assert_eq!(cells(&tables[1], 0), want, "{file}");
        assert!(
            want.iter().any(|w| !w.is_empty()),
            "{file}: some pair is significant"
        );
    }
}

#[test]
fn sets_left_out_of_the_tests_are_named() {
    // INCLUDEMRSETS=NO: a stacked set is left out and the caption says so
    let tables = run(
        "survey_basic.sav",
        &format!(
            "{SETS}CTABLES /TABLE gender BY region + $q5 /SIGTEST TYPE=CHISQUARE INCLUDEMRSETS=NO."
        ),
    )
    .unwrap();
    assert_eq!(tables[1].rows.len(), 1, "only gender by region");
    assert!(
        tables[1]
            .caption
            .as_ref()
            .unwrap()
            .contains("INCLUDEMRSETS=NO")
    );
    let e = run(
        "survey_basic.sav",
        &format!("{SETS}CTABLES /TABLE gender BY $q5 /COMPARETEST TYPE=PROP INCLUDEMRSETS=NO."),
    )
    .unwrap_err();
    assert!(
        e.contains("nothing is left to test") && e.contains("INCLUDEMRSETS=NO"),
        "{e}"
    );
    // COUNTDUPLICATES=YES leaves out a category set, not a dichotomy set
    let tables = run(
        "survey_basic.sav",
        &format!(
            "{SETS}CTABLES /MRSETS COUNTDUPLICATES=YES /TABLE gender BY $q5 + $q6 /SIGTEST TYPE=CHISQUARE."
        ),
    )
    .unwrap();
    assert_eq!(tables[1].rows.len(), 1, "only gender by $q5");
    assert!(
        tables[1]
            .caption
            .as_ref()
            .unwrap()
            .contains("COUNTDUPLICATES=YES")
    );
}

// ---- MERGE=YES: the letters in the main table ----

/// The compared columns' letters of a main table with merged letters: every letters column, by
/// row; and the number of columns of values in between. Letters columns are headed by a key
/// such as "(a)" in SIMPLE style and by "Group" in APA style.
fn merged(table: &Table) -> (Vec<Vec<String>>, usize) {
    let letter_columns: Vec<usize> = (table.row_headers..table.columns.len())
        .filter(|&c| table.columns[c] == "Group" || table.columns[c].starts_with('('))
        .collect();
    let rows = table
        .rows
        .iter()
        .map(|row| letter_columns.iter().map(|&c| text(&row[c])).collect())
        .collect();
    (
        rows,
        table.columns.len() - table.row_headers - letter_columns.len(),
    )
}

#[test]
fn simple_merged_letters_repeat_the_separate_table_beside_each_column() {
    let base = format!("{DERIVED}CTABLES /TABLE x BY gender > agegrp\n/COMPARETEST TYPE=PROP");
    for (alpha, lowered) in [("ALPHA=0.01, 0.05", false), ("ALPHA=0.05", true)] {
        let apart = run("survey_basic.sav", &format!("{base} {alpha}.")).unwrap();
        let tables = run(
            "survey_basic.sav",
            &format!("{base} {alpha} MERGE=YES STYLE=SIMPLE."),
        )
        .unwrap();
        assert_eq!(tables.len(), 1, "one table to deliver");
        let main = &tables[0];
        // a letters column follows the count of each of the nine compared columns
        let (letters, values) = merged(main);
        assert_eq!(values, 9);
        assert_eq!(main.columns[main.row_headers + 1], "(a)");
        assert_eq!(
            main.column_groups[main.row_headers + 1],
            ["Gender", "Male", "agegrp", "1.00"]
        );
        for (row, cells_merged) in letters.iter().enumerate() {
            let separate: Vec<String> = cells(&apart[1], row)
                .into_iter()
                .map(|c| if lowered { c.to_lowercase() } else { c })
                .collect();
            assert_eq!(*cells_merged, separate, "{alpha} row {row}");
        }
        assert!(letters.iter().flatten().any(|c| !c.is_empty() && c != "."));
        let caption = main.caption.as_ref().unwrap();
        assert!(caption.contains("headed by its key"), "{caption}");
    }
}

#[test]
fn apa_letters_are_shared_exactly_by_columns_that_do_not_differ() {
    let source =
        format!("{DERIVED}CTABLES /TABLE gender > y BY agegrp /COMPARETEST TYPE=MEAN ALPHA=0.05");
    for file in ["survey_basic.sav", "survey_basic_phpwriter.sav"] {
        let apart = run(file, &format!("{source}.")).unwrap();
        let tables = run(file, &format!("{source} MERGE=YES.")).unwrap();
        assert_eq!(tables.len(), 1);
        let (letters, _) = merged(&tables[0]);
        let mut differences = 0;
        for (row, groups) in letters.iter().enumerate() {
            // from the separate table: which pairs differ, by the keys A, B, C
            let marks = cells(&apart[1], row);
            let differ = |j: usize, k: usize| {
                let key = |i: usize| char::from(b'A' + i as u8);
                marks[j].contains(key(k)) || marks[k].contains(key(j))
            };
            for j in 0..3 {
                for k in j + 1..3 {
                    if groups[j] == "." || groups[k] == "." {
                        continue;
                    }
                    let shared = groups[j].chars().any(|c| groups[k].contains(c));
                    assert_eq!(
                        shared,
                        !differ(j, k),
                        "{file} row {row}: {groups:?} {marks:?}"
                    );
                    differences += usize::from(differ(j, k));
                }
            }
        }
        assert!(differences > 0, "{file}: some pair differs");
    }
}

#[test]
fn merged_letters_stand_in_the_row_of_the_compared_summary() {
    let tables = run(
        "survey_basic.sav",
        &format!(
            "{DERIVED}CTABLES /TABLE gender > y [COUNT, MEAN] BY agegrp /SLABELS POSITION=ROW\n\
             /COMPARETEST TYPE=MEAN MERGE=YES STYLE=SIMPLE ALPHA=0.2."
        ),
    )
    .unwrap();
    let main = &tables[0];
    // per gender a Count row, then a Mean row; letters only in the Mean rows
    let (letters, values) = merged(main);
    assert_eq!(values, 3);
    for (row, cells) in letters.iter().enumerate() {
        let label = text(&main.rows[row][main.row_headers - 1]);
        if label == "Count" {
            assert!(cells.iter().all(String::is_empty), "{cells:?}");
        } else {
            assert_eq!(label, "Mean");
        }
    }
    assert!(letters.iter().flatten().any(|c| !c.is_empty()));
}

#[test]
fn a_table_without_the_needed_variables_is_an_error() {
    let fails = |syntax: &str| run("survey_basic.sav", syntax).unwrap_err();
    let e = fails("CTABLES /TABLE gender /SIGTEST TYPE=CHISQUARE.");
    assert!(
        e.contains("categorical variable in both the rows and the columns"),
        "{e}"
    );
    let e = fails("CTABLES /TABLE gender BY region /COMPARETEST TYPE=MEAN.");
    assert!(e.contains("scale variable summarized by MEAN"), "{e}");
    let e = fails("CTABLES /TABLE age [MEDIAN] BY region /COMPARETEST TYPE=MEAN.");
    assert!(e.contains("summarized by MEAN"), "{e}");
}
