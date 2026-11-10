//! DESCRIPTIVES: summary statistics of numeric variables and their z-scores.

use crate::aggregate::statistic;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{AggregateFunction as F, DescriptiveStatistic as Stat, Descriptives};

const MAX_NAME_BYTES: usize = 64;

/// The weight of every case, or None without a weight variable. A missing, zero or negative
/// weight is 0, so the case counts for nothing.
pub(crate) fn case_weights(data: &Dataset) -> Result<Option<Vec<f64>>, Error> {
    let Some(name) = data.weight_variable() else {
        return Ok(None);
    };
    let variable = data.variable(name)?;
    let usable = |v: Option<f64>| v.filter(|v| !variable.is_user_missing(*v));
    let values = variable.numeric()?.iter();
    Ok(Some(
        values.map(|v| usable(v).unwrap_or(0.0).max(0.0)).collect(),
    ))
}

/// The statistics of one variable from its valid (value, weight) pairs, each None where it is
/// not defined.
pub(crate) struct Summary {
    pub n: f64,
    pub mean: Option<f64>,
    pub sd: Option<f64>,
    pub skewness: Option<f64>,
    pub se_skewness: Option<f64>,
    pub kurtosis: Option<f64>,
    pub se_kurtosis: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub sum: Option<f64>,
}

pub(crate) fn summarize(cells: &[(f64, f64)]) -> Summary {
    let w: f64 = cells.iter().map(|(_, w)| w).sum();
    let mean = statistic(F::Mean, cells);
    let sd = statistic(F::Sd, cells);
    let moment = |power: i32| {
        let mean = mean.unwrap_or(0.0);
        cells
            .iter()
            .map(|(x, w)| w * (x - mean).powi(power))
            .sum::<f64>()
    };
    // without spread neither skewness nor kurtosis nor their standard errors are computed
    let spread = sd.filter(|sd| sd * sd >= 1e-20);
    let se_skewness = spread
        .filter(|_| w > 2.0)
        .map(|_| (6.0 * w * (w - 1.0) / ((w - 2.0) * (w + 1.0) * (w + 3.0))).sqrt());
    let se_kurtosis = spread.filter(|_| w > 3.0).map(|_| {
        let se_skewness = se_skewness.unwrap_or(0.0);
        (4.0 * (w * w - 1.0) * se_skewness * se_skewness / ((w - 3.0) * (w + 5.0))).sqrt()
    });
    Summary {
        n: w,
        mean,
        sd,
        skewness: spread
            .filter(|_| w > 2.0)
            .map(|sd| w * moment(3) / ((w - 1.0) * (w - 2.0) * sd.powi(3))),
        se_skewness,
        kurtosis: spread.filter(|_| w > 3.0).map(|sd| {
            let m2 = moment(2);
            (w * (w + 1.0) * moment(4) - 3.0 * m2 * m2 * (w - 1.0))
                / ((w - 1.0) * (w - 2.0) * (w - 3.0) * sd.powi(4))
        }),
        se_kurtosis,
        min: cells.iter().map(|(x, _)| *x).reduce(f64::min),
        max: cells.iter().map(|(x, _)| *x).reduce(f64::max),
        sum: Some(statistic(F::Sum, cells).unwrap_or(0.0)),
    }
}

/// The columns of one statistic: header and value. KURTOSIS and SKEWNESS come with their
/// standard errors.
fn columns(stat: Stat, s: &Summary) -> Vec<(&'static str, Option<f64>)> {
    match stat {
        Stat::Mean => vec![("Mean", s.mean)],
        Stat::SeMean => vec![("S.E. Mean", s.sd.map(|sd| sd / s.n.sqrt()))],
        Stat::StdDev => vec![("Std Dev", s.sd)],
        Stat::Variance => vec![("Variance", s.sd.map(|sd| sd * sd))],
        Stat::Kurtosis => vec![("Kurtosis", s.kurtosis), ("S.E. Kurt", s.se_kurtosis)],
        Stat::Skewness => vec![("Skewness", s.skewness), ("S.E. Skew", s.se_skewness)],
        Stat::Range => vec![("Range", s.max.zip(s.min).map(|(max, min)| max - min))],
        Stat::Min => vec![("Minimum", s.min)],
        Stat::Max => vec![("Maximum", s.max)],
        Stat::Sum => vec![("Sum", s.sum)],
    }
}

pub(crate) fn number(value: Option<f64>) -> Cell {
    value
        .filter(|v| v.is_finite())
        .map_or(Cell::Empty, Cell::Number)
}

impl Engine {
    pub(crate) fn descriptives(&mut self, command: &Descriptives) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut variables: Vec<(String, Option<String>)> = Vec::new();
        for (item, z) in &command.variables {
            let names = self.expand(std::slice::from_ref(item))?;
            variables.extend(names.into_iter().map(|name| (name, z.clone())));
        }
        // a string variable gets no statistics, and a warning says so
        let mut strings = Vec::new();
        let mut columns_of = Vec::new();
        variables.retain(|(name, _)| match data.variable(name) {
            Ok(variable) if variable.is_string() => {
                strings.push(name.clone());
                false
            }
            _ => true,
        });
        for (name, _) in &variables {
            columns_of.push(data.variable(name)?);
        }
        if variables.is_empty() {
            let (names, verb) = match &strings[..] {
                [one] => (one.clone(), "is a string variable"),
                many => (many.join(" "), "are string variables"),
            };
            return Err(Error(format!(
                "DESCRIPTIVES needs a numeric variable, but {names} {verb}"
            )));
        }
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let valid = |index: usize, case: usize| {
            let variable = &columns_of[index];
            let value = variable.numeric().ok()?.get(case)?;
            (command.include || !variable.is_user_missing(value)).then_some(value)
        };
        let complete: Vec<bool> = (0..data.case_count())
            .map(|case| (0..variables.len()).all(|index| valid(index, case).is_some()))
            .collect();
        let counted = |case: usize| !command.listwise || complete[case];

        let total: f64 = (0..data.case_count()).map(weight).sum();
        let listwise: f64 = (0..data.case_count())
            .filter(|&case| complete[case])
            .map(weight)
            .sum();
        let summaries: Vec<Summary> = (0..variables.len())
            .map(|index| {
                let cells: Vec<(f64, f64)> = (0..data.case_count())
                    .filter(|&case| counted(case) && weight(case) > 0.0)
                    .filter_map(|case| Some((valid(index, case)?, weight(case))))
                    .collect();
                summarize(&cells)
            })
            .collect();

        let mut headers = vec!["Variable", "Label", "N"];
        let mut rows = Vec::new();
        for ((name, _), summary) in variables.iter().zip(&summaries) {
            let label = data.variable(name)?.label();
            let mut row = vec![
                Cell::Text(name.clone()),
                label.map_or(Cell::Empty, |l| Cell::Text(l.to_string())),
                Cell::Number(summary.n),
            ];
            for &stat in &command.statistics {
                row.extend(columns(stat, summary).into_iter().map(|(_, v)| number(v)));
            }
            rows.push(row);
        }
        let empty = Summary {
            n: 0.0,
            ..summarize(&[])
        };
        for stat in &command.statistics {
            headers.extend(columns(*stat, &empty).into_iter().map(|(header, _)| header));
        }
        // the complete cases, whichever way the statistics treat missing values
        for (title, n) in [
            ("Valid N (listwise)", listwise),
            ("Missing N (listwise)", total - listwise),
        ] {
            let mut row = vec![Cell::Text(title.into()), Cell::Empty, Cell::Number(n)];
            row.resize_with(headers.len(), || Cell::Empty);
            rows.push(row);
        }
        let table = Table {
            title: "Descriptive Statistics".into(),
            row_headers: 2,
            column_groups: vec![Vec::new(); headers.len()],
            columns: headers.into_iter().map(String::from).collect(),
            rows,
            ..Table::default()
        };

        // SAVE gives every variable a z-score; without it only those with a name in parentheses
        let scored: Vec<usize> = (0..variables.len())
            .filter(|&index| command.save || variables[index].1.is_some())
            .collect();
        if !scored.is_empty() {
            let chosen: Vec<(String, Option<String>)> =
                scored.iter().map(|&i| variables[i].clone()).collect();
            let names = self.z_names(&chosen)?;
            let zs: Vec<Vec<Option<f64>>> = scored
                .iter()
                .map(|&index| {
                    let summary = &summaries[index];
                    let spread = summary.sd.filter(|sd| *sd > 0.0);
                    (0..data.case_count())
                        .map(|case| {
                            let x = valid(index, case).filter(|_| counted(case))?;
                            Some((x - summary.mean?) / spread?)
                        })
                        .collect()
                })
                .collect();
            let labels: Vec<String> = chosen
                .iter()
                .zip(&names)
                .map(|((name, given), z)| {
                    let label = data.variable(name)?.label();
                    let natural = given.is_some() || *z == format!("Z{name}");
                    Ok(match label {
                        Some(label) if natural => format!("Zscore: {label}"),
                        Some(label) => format!("Zscore({name}) {label}"),
                        None => format!("Zscore({name})"),
                    })
                })
                .collect::<Result<_, Error>>()?;
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for ((name, values), label) in names.iter().zip(zs).zip(&labels) {
                    data.set_numeric_values(name, values)?;
                    data.set_variable_label(name, Some(label))?;
                }
                Ok(())
            })?;
        }
        if !strings.is_empty() {
            self.warn(format!(
                "no statistics for string variables: {}",
                strings.join(" ")
            ));
        }
        Ok(vec![table])
    }

    /// The name of each z-score: the one written after the variable, else Z plus the name of
    /// the variable, else the first free name of ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to
    /// ZZZZ09 and ZQZQ01 to ZQZQ09.
    fn z_names(&self, variables: &[(String, Option<String>)]) -> Result<Vec<String>, Error> {
        let data = self.active()?;
        let mut taken: Vec<String> = Vec::new();
        let free = |name: &str, taken: &[String]| {
            data.variable_index(name).is_none()
                && !taken.iter().any(|t| t.eq_ignore_ascii_case(name))
        };
        for (_, z) in variables {
            if let Some(z) = z {
                if !free(z, &taken) {
                    return Err(Error(format!(
                        "the z-score name {z} is already in use; choose another name"
                    )));
                }
                taken.push(z.clone());
            }
        }
        let mut names = Vec::new();
        for (name, z) in variables {
            let chosen = match z {
                Some(z) => z.clone(),
                None => {
                    let natural = format!("Z{name}");
                    let numbered = (1..100).map(|i| format!("ZSC{i:03}")).chain(
                        ["STDZ", "ZZZZ", "ZQZQ"]
                            .into_iter()
                            .flat_map(|stem| (1..10).map(move |i| format!("{stem}{i:02}"))),
                    );
                    let fits = natural.len() <= MAX_NAME_BYTES && free(&natural, &taken);
                    let mut candidates = std::iter::once(natural).filter(|_| fits).chain(numbered);
                    candidates
                        .find(|c| free(c, &taken))
                        .ok_or_else(|| Error(format!("no free name for the z-score of {name}")))?
                }
            };
            taken.push(chosen.clone());
            names.push(chosen);
        }
        Ok(names)
    }
}
