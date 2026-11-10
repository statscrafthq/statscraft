//! ONEWAY: one-way analysis of variance, descriptives, Levene's test, the Welch and
//! Brown-Forsythe tests, contrasts and post hoc multiple comparisons.
//!
//! Rules from the Command Syntax Reference (ONEWAY) and the Algorithms manual (ONEWAY, and the
//! appendix Post Hoc Tests):
//! - Every value of the factor that has cases is a group, in ascending order; the dependents and
//!   the factor must be numeric. Weights are used as they are, so df = W - k.
//! - MISSING=ANALYSIS leaves out cases missing on the dependent or the factor of each analysis,
//!   LISTWISE cases missing on any variable named.
//! - Contrast coefficients go to the groups in ascending order; a list of the wrong length is
//!   not tested and coefficients that do not sum to 0 are tested with a warning.
//! - Post hoc tests compare every ordered pair of groups. LSD, Bonferroni, Sidak, Scheffe and
//!   Tukey use the pooled variance; Games-Howell and Tamhane's T2 the separate variances with
//!   Satterthwaite's df.

use crate::descriptives::{case_weights, number};
use crate::dist::{
    f_p, f_quantile, studentized_range_cdf, studentized_range_quantile, t_p, t_quantile,
};
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Value as Stored, Values, Variable};
use statscraft_syntax::{Oneway, Posthoc};

/// The statistics of one group.
#[derive(Clone, Copy)]
struct Group {
    w: f64,
    mean: f64,
    /// The corrected sum of squares.
    css: f64,
    variance: f64,
    min: f64,
    max: f64,
}

fn group(pairs: &[(f64, f64)]) -> Group {
    let w: f64 = pairs.iter().map(|(_, w)| w).sum();
    let mean = pairs.iter().map(|(x, w)| x * w).sum::<f64>() / w;
    let squares: f64 = pairs.iter().map(|(x, w)| w * (x - mean) * (x - mean)).sum();
    Group {
        w,
        mean,
        css: squares,
        variance: squares / (w - 1.0),
        min: pairs.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
        max: pairs.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max),
    }
}

fn label(factor: &Variable, value: f64) -> String {
    let found = factor
        .value_labels()
        .iter()
        .find(|(v, _)| *v == Stored::Number(value));
    found.map_or_else(|| value.to_string(), |(_, l)| l.clone())
}

fn test_name(test: Posthoc) -> &'static str {
    match test {
        Posthoc::Lsd => "LSD",
        Posthoc::Bonferroni => "Bonferroni",
        Posthoc::Sidak => "Sidak",
        Posthoc::Tukey => "Tukey HSD",
        Posthoc::Scheffe => "Scheffe",
        Posthoc::GamesHowell => "Games-Howell",
        Posthoc::T2 => "Tamhane",
    }
}

fn ci_title(ci: f64) -> String {
    let percent = 100.0 * ci;
    if (percent - percent.round()).abs() < 1e-9 {
        format!("{percent:.0}% Confidence Interval")
    } else {
        format!("{}% Confidence Interval", (percent * 1e6).round() / 1e6)
    }
}

fn simple(title: String, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title,
        row_headers: columns.iter().take_while(|c| c.is_empty()).count(),
        column_groups: vec![Vec::new(); columns.len()],
        columns: columns.iter().map(|c| c.to_string()).collect(),
        rows,
        ..Table::default()
    }
}

/// Mean difference, standard error, significance and interval of one pair by one test.
fn compare(
    test: Posthoc,
    a: &Group,
    b: &Group,
    k: f64,
    within: f64,
    df: f64,
    alpha: f64,
) -> [f64; 5] {
    let diff = a.mean - b.mean;
    let pairs = k * (k - 1.0) / 2.0;
    let pooled = (within * (1.0 / a.w + 1.0 / b.w)).sqrt();
    let separate_parts = [a.variance / a.w, b.variance / b.w];
    let separate = (separate_parts[0] + separate_parts[1]).sqrt();
    let welch_df = (separate_parts[0] + separate_parts[1]).powi(2)
        / (separate_parts[0].powi(2) / (a.w - 1.0) + separate_parts[1].powi(2) / (b.w - 1.0));
    let (se, p, half) = match test {
        Posthoc::Lsd => {
            let p = t_p(diff / pooled, df);
            (pooled, p, t_quantile(alpha / 2.0, df) * pooled)
        }
        Posthoc::Bonferroni => {
            let p = (t_p(diff / pooled, df) * pairs).min(1.0);
            (pooled, p, t_quantile(alpha / (2.0 * pairs), df) * pooled)
        }
        Posthoc::Sidak => {
            let p = 1.0 - (1.0 - t_p(diff / pooled, df)).powf(pairs);
            let level = 1.0 - (1.0 - alpha).powf(1.0 / pairs);
            (pooled, p, t_quantile(level / 2.0, df) * pooled)
        }
        Posthoc::Scheffe => {
            let f = diff * diff / (pooled * pooled * (k - 1.0));
            let critical = ((k - 1.0) * f_quantile(alpha, k - 1.0, df)).sqrt();
            (pooled, f_p(f, k - 1.0, df), critical * pooled)
        }
        Posthoc::Tukey => {
            let q = diff.abs() / pooled * std::f64::consts::SQRT_2;
            let p = 1.0 - studentized_range_cdf(q, k, df);
            let critical = studentized_range_quantile(alpha, k, df) / std::f64::consts::SQRT_2;
            (pooled, p, critical * pooled)
        }
        Posthoc::GamesHowell => {
            let q = diff.abs() / separate * std::f64::consts::SQRT_2;
            let p = 1.0 - studentized_range_cdf(q, k, welch_df);
            let critical =
                studentized_range_quantile(alpha, k, welch_df) / std::f64::consts::SQRT_2;
            (separate, p, critical * separate)
        }
        Posthoc::T2 => {
            let p = 1.0 - (1.0 - t_p(diff / separate, welch_df)).powf(pairs);
            let level = 1.0 - (1.0 - alpha).powf(1.0 / pairs);
            (separate, p, t_quantile(level / 2.0, welch_df) * separate)
        }
    };
    [diff, se, p.clamp(0.0, 1.0), diff - half, diff + half]
}

impl Engine {
    pub(crate) fn oneway(&mut self, command: &Oneway) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let names = self.expand(&command.dependents)?;
        let dependents: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let factor = data.variable(&command.factor)?;
        for v in dependents.iter().chain([&factor]) {
            if v.is_string() {
                return Err(Error(format!(
                    "ONEWAY needs numeric variables, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let valid = |v: &Variable, c: usize| is_valid(v, c, command.include);
        let value = |v: &Variable, c: usize| match v.values() {
            Values::Numeric(values) => values.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let usable: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0 && valid(&factor, c))
            .filter(|&c| !command.listwise || dependents.iter().all(|d| valid(d, c)))
            .collect();
        // the groups of the contrasts: every factor value of a usable case
        let mut levels: Vec<f64> = usable.iter().map(|&c| value(&factor, c)).collect();
        levels.sort_by(f64::total_cmp);
        levels.dedup();
        let mut warnings = Vec::new();
        for (i, coefficients) in command.contrasts.iter().enumerate() {
            if coefficients.len() != levels.len() {
                warnings.push(format!(
                    "contrast {} has {} coefficients for {} groups and is not tested",
                    i + 1,
                    coefficients.len(),
                    levels.len()
                ));
            } else if coefficients.iter().sum::<f64>().abs() > 1e-8 {
                warnings.push(format!(
                    "the coefficients of contrast {} do not sum to 0",
                    i + 1
                ));
            }
        }
        let ci = ci_title(command.ci);
        let alpha = 1.0 - command.ci;
        let (mut descriptives, mut levene, mut anova, mut robust, mut contrast_rows) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut comparisons = Vec::new();
        for dependent in &dependents {
            let cases: Vec<usize> = usable
                .iter()
                .copied()
                .filter(|&c| valid(dependent, c))
                .collect();
            let pairs_of = |level: f64| -> Vec<(f64, f64)> {
                cases
                    .iter()
                    .filter(|&&c| value(&factor, c) == level)
                    .map(|&c| (value(dependent, c), weight(c)))
                    .collect()
            };
            let present: Vec<(f64, Group)> = levels
                .iter()
                .map(|&l| (l, pairs_of(l)))
                .filter(|(_, p)| !p.is_empty())
                .map(|(l, p)| (l, group(&p)))
                .collect();
            let all: Vec<(f64, f64)> = cases
                .iter()
                .map(|&c| (value(dependent, c), weight(c)))
                .collect();
            let total = group(&all);
            let k = present.len() as f64;
            let w = total.w;
            let within: f64 = present.iter().map(|(_, g)| g.css).sum();
            let grand = total.mean;
            let between: f64 = present
                .iter()
                .map(|(_, g)| g.w * (g.mean - grand).powi(2))
                .sum();
            let df_within = w - k;
            let ms_within = within / df_within;
            let name = || Cell::Text(dependent.name().to_string());
            if k < 2.0 {
                warnings.push(format!(
                    "no analysis of variance for {}: it needs at least two groups",
                    dependent.name()
                ));
            }
            if command.descriptives {
                let t_crit = |g: &Group| t_quantile(alpha / 2.0, g.w - 1.0);
                for (i, (label_value, g)) in present
                    .iter()
                    .map(|(l, g)| (Some(*l), g))
                    .chain([(None, &total)])
                    .enumerate()
                {
                    let se = (g.variance / g.w).sqrt();
                    let half = t_crit(g) * se;
                    let mut row = vec![
                        if i == 0 { name() } else { Cell::Empty },
                        Cell::Text(label_value.map_or("Total".into(), |l| label(&factor, l))),
                    ];
                    row.extend(
                        [
                            g.w,
                            g.mean,
                            g.variance.sqrt(),
                            se,
                            g.mean - half,
                            g.mean + half,
                            g.min,
                            g.max,
                        ]
                        .map(|v| number(Some(v))),
                    );
                    descriptives.push(row);
                }
            }
            if command.homogeneity {
                // Levene's test on absolute deviations from the group means
                let deviations: Vec<Vec<(f64, f64)>> = present
                    .iter()
                    .map(|(l, g)| {
                        pairs_of(*l)
                            .into_iter()
                            .map(|(x, w)| ((x - g.mean).abs(), w))
                            .collect()
                    })
                    .collect();
                let z: Vec<Group> = deviations.iter().map(|d| group(d)).collect();
                let z_all = z.iter().map(|g| g.w * g.mean).sum::<f64>() / w;
                let spread: f64 = z.iter().map(|g| g.w * (g.mean - z_all).powi(2)).sum();
                let inside: f64 = deviations
                    .iter()
                    .zip(&z)
                    .map(|(d, g)| d.iter().map(|(x, w)| w * (x - g.mean).powi(2)).sum::<f64>())
                    .sum();
                let statistic = (w - k) * spread / ((k - 1.0) * inside);
                levene.push(vec![
                    name(),
                    number(Some(statistic)),
                    number(Some(k - 1.0)),
                    number(Some(df_within)),
                    number(Some(f_p(statistic, k - 1.0, df_within))),
                ]);
            }
            let f = between / (k - 1.0) / ms_within;
            let rows = [
                ("Between Groups", between, k - 1.0, true),
                ("Within Groups", within, df_within, false),
                ("Total", between + within, w - 1.0, false),
            ];
            for (i, (source, ss, df, tested)) in rows.into_iter().enumerate() {
                anova.push(vec![
                    if i == 0 { name() } else { Cell::Empty },
                    Cell::Text(source.into()),
                    number(Some(ss)),
                    number(Some(df)),
                    number((i < 2).then_some(ss / df)),
                    number(tested.then_some(f)),
                    number(tested.then(|| f_p(f, k - 1.0, df_within))),
                ]);
            }
            if command.welch || command.brown_forsythe {
                let mut first = true;
                let mut push = |test: &str, statistic: f64, df1: f64, df2: f64| {
                    robust.push(vec![
                        if first { name() } else { Cell::Empty },
                        Cell::Text(test.into()),
                        number(Some(statistic)),
                        number(Some(df1)),
                        number(Some(df2)),
                        number(Some(f_p(statistic, df1, df2))),
                    ]);
                    first = false;
                };
                if command.welch {
                    let omega: Vec<f64> = present.iter().map(|(_, g)| g.w / g.variance).collect();
                    let u: f64 = omega.iter().sum();
                    let center = present
                        .iter()
                        .zip(&omega)
                        .map(|((_, g), o)| o * g.mean)
                        .sum::<f64>()
                        / u;
                    let top = present
                        .iter()
                        .zip(&omega)
                        .map(|((_, g), o)| o * (g.mean - center).powi(2))
                        .sum::<f64>()
                        / (k - 1.0);
                    let lambda: f64 = present
                        .iter()
                        .zip(&omega)
                        .map(|((_, g), o)| (1.0 - o / u).powi(2) / (g.w - 1.0))
                        .sum();
                    let statistic = top / (1.0 + 2.0 * (k - 2.0) / (k * k - 1.0) * lambda);
                    let df2 = 1.0 / (3.0 / (k * k - 1.0) * lambda);
                    if present
                        .iter()
                        .any(|(_, g)| g.variance.is_nan() || g.variance <= 0.0)
                    {
                        warnings.push(format!(
                            "no Welch test for {}: a group has no spread",
                            dependent.name()
                        ));
                    }
                    push("Welch", statistic, k - 1.0, df2);
                }
                if command.brown_forsythe {
                    let parts: Vec<f64> = present
                        .iter()
                        .map(|(_, g)| (1.0 - g.w / w) * g.variance)
                        .collect();
                    let bottom: f64 = parts.iter().sum();
                    let statistic = between / bottom;
                    let inverse: f64 = present
                        .iter()
                        .zip(&parts)
                        .map(|((_, g), part)| (part / bottom).powi(2) / (g.w - 1.0))
                        .sum();
                    push("Brown-Forsythe", statistic, k - 1.0, 1.0 / inverse);
                }
            }
            for (equal, title) in [
                (true, "Assume equal variances"),
                (false, "Does not assume equal variances"),
            ] {
                for (i, coefficients) in command.contrasts.iter().enumerate() {
                    if coefficients.len() != levels.len() {
                        continue;
                    }
                    let mut groups = Vec::new();
                    for (&level, &c) in levels.iter().zip(coefficients) {
                        match present.iter().find(|(l, _)| *l == level) {
                            Some((_, g)) => groups.push((c, *g)),
                            None if c == 0.0 => {}
                            None => groups.push((c, group(&[]))),
                        }
                    }
                    let value: f64 = groups.iter().map(|(c, g)| c * g.mean).sum();
                    let (se, df) = if equal {
                        let sum: f64 = groups.iter().map(|(c, g)| c * c / g.w).sum();
                        ((ms_within * sum).sqrt(), df_within)
                    } else {
                        let parts: Vec<(f64, f64)> = groups
                            .iter()
                            .map(|(c, g)| (c * c * g.variance / g.w, g.w))
                            .collect();
                        let sum: f64 = parts.iter().map(|p| p.0).sum();
                        let bottom: f64 = parts.iter().map(|(p, w)| p * p / (w - 1.0)).sum();
                        (sum.sqrt(), sum * sum / bottom)
                    };
                    let t = value / se;
                    contrast_rows.push(vec![
                        if equal && i == 0 { name() } else { Cell::Empty },
                        Cell::Text(if i == 0 { title.into() } else { String::new() }),
                        Cell::Number((i + 1) as f64),
                        number(Some(value)),
                        number(Some(se)),
                        number(Some(t)),
                        number(Some(df)),
                        number(Some(t_p(t, df))),
                    ]);
                }
            }
            if !command.posthoc.is_empty() && k >= 2.0 {
                let mut rows = Vec::new();
                for &test in &command.posthoc {
                    // each unordered pair once; the reverse direction mirrors it
                    let mut found = vec![vec![[f64::NAN; 5]; present.len()]; present.len()];
                    for i in 0..present.len() {
                        for j in i + 1..present.len() {
                            let r = compare(
                                test,
                                &present[i].1,
                                &present[j].1,
                                k,
                                ms_within,
                                df_within,
                                command.alpha,
                            );
                            found[i][j] = r;
                            found[j][i] = [-r[0], r[1], r[2], -r[4], -r[3]];
                        }
                    }
                    for i in 0..present.len() {
                        for j in (0..present.len()).filter(|&j| j != i) {
                            let first_of_test = i == 0 && j == 1;
                            let mut row = vec![
                                Cell::Text(if first_of_test {
                                    test_name(test).into()
                                } else {
                                    String::new()
                                }),
                                Cell::Text(if j == usize::from(i == 0) {
                                    label(&factor, present[i].0)
                                } else {
                                    String::new()
                                }),
                                Cell::Text(label(&factor, present[j].0)),
                            ];
                            row.extend(found[i][j].map(|v| number(Some(v))));
                            rows.push(row);
                        }
                    }
                }
                let mut table = simple(
                    format!("Multiple Comparisons: {}", dependent.name()),
                    &[
                        "",
                        "(I) group",
                        "(J) group",
                        "Mean Difference (I-J)",
                        "Std. Error",
                        "Sig.",
                        "Lower Bound",
                        "Upper Bound",
                    ],
                    rows,
                );
                table.columns[1] = format!("(I) {}", factor.name());
                table.columns[2] = format!("(J) {}", factor.name());
                let interval = ci_title(1.0 - command.alpha);
                table.column_groups[6] = vec![interval.clone()];
                table.column_groups[7] = vec![interval];
                table.row_headers = 3;
                comparisons.push(table);
            }
        }
        let mut out = Vec::new();
        if command.descriptives {
            let mut table = simple(
                "Descriptives".into(),
                &[
                    "",
                    "",
                    "N",
                    "Mean",
                    "Std. Deviation",
                    "Std. Error",
                    "Lower Bound",
                    "Upper Bound",
                    "Minimum",
                    "Maximum",
                ],
                descriptives,
            );
            table.columns[1] = factor.name().to_string();
            let level = format!("{ci} for Mean");
            table.column_groups[6] = vec![level.clone()];
            table.column_groups[7] = vec![level];
            out.push(table);
        }
        if command.homogeneity {
            out.push(simple(
                "Test of Homogeneity of Variances".into(),
                &["", "Levene Statistic", "df1", "df2", "Sig."],
                levene,
            ));
        }
        out.push(simple(
            "ANOVA".into(),
            &["", "", "Sum of Squares", "df", "Mean Square", "F", "Sig."],
            anova,
        ));
        if command.welch || command.brown_forsythe {
            out.push(simple(
                "Robust Tests of Equality of Means".into(),
                &["", "", "Statistic", "df1", "df2", "Sig."],
                robust,
            ));
        }
        let tested: Vec<(usize, &Vec<f64>)> = command
            .contrasts
            .iter()
            .enumerate()
            .filter(|(_, c)| c.len() == levels.len())
            .collect();
        if !tested.is_empty() {
            let mut columns = vec![String::new()];
            columns.extend(levels.iter().map(|&l| label(&factor, l)));
            let rows = tested
                .iter()
                .map(|(i, c)| {
                    let mut row = vec![Cell::Number((*i + 1) as f64)];
                    row.extend(c.iter().map(|&v| Cell::Number(v)));
                    row
                })
                .collect();
            let mut groups = vec![Vec::new()];
            groups.extend(vec![vec![factor.name().to_string()]; levels.len()]);
            out.push(Table {
                title: "Contrast Coefficients".into(),
                row_headers: 1,
                columns,
                column_groups: groups,
                rows,
                ..Table::default()
            });
            out.push(simple(
                "Contrast Tests".into(),
                &[
                    "",
                    "",
                    "Contrast",
                    "Value of Contrast",
                    "Std. Error",
                    "t",
                    "df",
                    "Sig. (2-tailed)",
                ],
                contrast_rows,
            ));
            out.last_mut().expect("pushed").row_headers = 3;
        }
        out.extend(comparisons);
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}
