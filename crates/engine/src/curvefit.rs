//! CURVEFIT: eleven curves of one independent variable, or of the case sequence, fitted by
//! least squares on their linear forms.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Linear forms: LINEAR y on t; LOGARITHMIC y on ln t; INVERSE y on 1/t; QUADRATIC and CUBIC
//!   y on powers of t; COMPOUND, GROWTH and EXPONENTIAL ln y on t; POWER ln y on ln t; S ln y
//!   on 1/t; LGSTIC ln(1/y - 1/u) on t (1/u = 0 without UPPERBOUND). A model whose forms are
//!   undefined for some case (y or t not above 0 for a logarithm, t = 0 for 1/t) is not fitted.
//! - b0 = exp(constant) for COMPOUND, POWER, EXPONENTIAL and LGSTIC and b1 = exp(slope) for
//!   COMPOUND and LGSTIC, each standard error scaled by the same exp; t tests stay on the
//!   linear form. NOCONSTANT leaves b0 out (0 or 1).
//! - Fit, residual (original units) and the prediction interval
//!   back(yhat* -/+ t s sqrt(1 + h)), h the leverage x'(X'WX)^-1 x with the constant column.
//! - Listwise deletion over all named variables; case weights only with an independent; the
//!   case sequence is 1, 2, ... in the data as the procedure sees them.
//! - QUADRATIC and CUBIC are not fitted when a term's tolerance falls below 0.0001.

use crate::chart::{Axis, ChartKind};
use crate::descriptives::{case_weights, number};
use crate::dist::{t_p, t_quantile};
use crate::npar_tests::text;
use crate::plot::{Plot, Point, PointGroup};
use crate::regression::table;
use crate::wls::{LeastSquares, anova_table, least_squares, numeric_column, summary_table};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::Measure;
use statscraft_syntax::{CurveModel, Curvefit};

/// A case's fit, residual and lower and upper prediction limits.
type Saved = [Option<f64>; 4];

/// The smallest tolerance QUADRATIC and CUBIC accept.
const TOLERANCE: f64 = 1e-4;

fn label(model: CurveModel) -> &'static str {
    match model {
        CurveModel::Linear => "Linear",
        CurveModel::Logarithmic => "Logarithmic",
        CurveModel::Inverse => "Inverse",
        CurveModel::Quadratic => "Quadratic",
        CurveModel::Cubic => "Cubic",
        CurveModel::Compound => "Compound",
        CurveModel::Power => "Power",
        CurveModel::S => "S",
        CurveModel::Growth => "Growth",
        CurveModel::Exponential => "Exponential",
        CurveModel::Logistic => "Logistic",
    }
}

/// The predictors of the linear form at t; None where they are undefined.
fn terms(model: CurveModel, t: f64) -> Option<Vec<f64>> {
    match model {
        CurveModel::Logarithmic | CurveModel::Power => (t > 0.0).then(|| vec![t.ln()]),
        CurveModel::Inverse | CurveModel::S => (t != 0.0).then(|| vec![1.0 / t]),
        CurveModel::Quadratic => Some(vec![t, t * t]),
        CurveModel::Cubic => Some(vec![t, t * t, t * t * t]),
        _ => Some(vec![t]),
    }
}

/// The names of the predictors of the linear form.
fn term_names(model: CurveModel, x: &str) -> Vec<String> {
    match model {
        CurveModel::Logarithmic | CurveModel::Power => vec![format!("ln({x})")],
        CurveModel::Inverse | CurveModel::S => vec![format!("1 / {x}")],
        CurveModel::Quadratic => vec![x.to_string(), format!("{x} ** 2")],
        CurveModel::Cubic => vec![x.to_string(), format!("{x} ** 2"), format!("{x} ** 3")],
        _ => vec![x.to_string()],
    }
}

/// Whether the model is fitted on the log scale.
fn logged(model: CurveModel) -> bool {
    model >= CurveModel::Compound
}

/// The dependent of the linear form; None where it is undefined.
fn linear_y(model: CurveModel, y: f64, inv_u: f64) -> Option<f64> {
    match model {
        CurveModel::Logistic => (y > 0.0 && 1.0 / y - inv_u > 0.0).then(|| (1.0 / y - inv_u).ln()),
        m if logged(m) => (y > 0.0).then(|| y.ln()),
        _ => Some(y),
    }
}

/// From the linear form back to the original units.
fn back(model: CurveModel, v: f64, inv_u: f64) -> f64 {
    match model {
        CurveModel::Logistic => 1.0 / (v.exp() + inv_u),
        m if logged(m) => v.exp(),
        _ => v,
    }
}

/// Whether b0, and the slope b1, are reported as the exp of the linear form's coefficient.
fn exp_coefficients(model: CurveModel) -> (bool, bool) {
    use CurveModel::*;
    (
        matches!(model, Compound | Power | Exponential | Logistic),
        matches!(model, Compound | Logistic),
    )
}

/// A fitted model of one dependent.
struct Fitted {
    model: CurveModel,
    ls: LeastSquares,
}

impl Fitted {
    /// The reported constant (with its standard error) and slopes.
    fn constant(&self) -> Option<(f64, f64)> {
        let (exp_b0, _) = exp_coefficients(self.model);
        self.ls.constant.then(|| {
            let (b, se) = (self.ls.intercept, self.ls.intercept_se());
            if exp_b0 {
                (b.exp(), b.exp() * se)
            } else {
                (b, se)
            }
        })
    }
    fn slope(&self, a: usize) -> (f64, f64) {
        let (b, se) = (self.ls.slopes[a], self.ls.slope_se(a));
        if exp_coefficients(self.model).1 {
            (b.exp(), b.exp() * se)
        } else {
            (b, se)
        }
    }
    /// The fit and the prediction interval at t, in the original units.
    fn predict(&self, t: f64, inv_u: f64, t_crit: f64) -> Option<(f64, f64, f64)> {
        let x = terms(self.model, t)?;
        let (v, h) = self.ls.predict(&x);
        let half = t_crit * (self.ls.mse() * (1.0 + h)).sqrt();
        let (a, b) = (
            back(self.model, v - half, inv_u),
            back(self.model, v + half, inv_u),
        );
        Some((back(self.model, v, inv_u), a.min(b), a.max(b)))
    }
}

impl Engine {
    pub(crate) fn curvefit(&mut self, command: &Curvefit) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let dependents = self.expand(&command.dependents)?;
        let ys: Vec<Vec<f64>> = dependents
            .iter()
            .map(|n| numeric_column(data, n, false, "CURVEFIT"))
            .collect::<Result<_, _>>()?;
        let cases = data.case_count();
        let (x, x_text) = match &command.independent {
            Some(name) => (
                numeric_column(data, name, false, "CURVEFIT")?,
                shown.named(data, name),
            ),
            None => ((1..=cases).map(|k| k as f64).collect(), "Sequence".into()),
        };
        // case weights count only against an independent variable
        let weights = match command.independent {
            Some(_) => case_weights(data)?,
            None => None,
        };
        let id: Option<Vec<String>> = match &command.id {
            Some(name) => {
                let v = data.variable(name)?;
                Some(
                    (0..cases)
                        .map(|k| crate::cluster::value_text(&v, k))
                        .collect(),
                )
            }
            None => None,
        };
        let weight = |k: usize| weights.as_ref().map_or(1.0, |w| w[k]);
        let used: Vec<usize> = (0..cases)
            .filter(|&k| weight(k) > 0.0 && !x[k].is_nan() && ys.iter().all(|y| !y[k].is_nan()))
            .collect();
        let models = &command.models;
        let inv_u = match command.upper_bound {
            Some(u) if models.contains(&CurveModel::Logistic) => {
                if let Some(max) = used
                    .iter()
                    .flat_map(|&k| ys.iter().map(move |y| y[k]))
                    .reduce(f64::max)
                    && u <= max
                {
                    return Err(Error(format!(
                        "UPPERBOUND {u} must be above the largest value of the dependent variables, {max}"
                    )));
                }
                1.0 / u
            }
            _ => 0.0,
        };
        let mut warnings = Vec::new();
        if command.template {
            warnings.push("CURVEFIT: TEMPLATE is ignored; charts come as data".to_string());
        }
        let mut tables = vec![table(
            "Case Processing Summary",
            1,
            crate::regression::plain(&["", "N"]),
            vec![
                vec![text("Total Cases"), Cell::Number(cases as f64)],
                vec![
                    text("Excluded Cases"),
                    Cell::Number((cases - used.len()) as f64),
                ],
            ],
            Some("A case missing on any variable of the command is left out of every fit.".into()),
        )];
        let mut charts = Vec::new();
        let mut saves: Vec<(String, CurveModel, Vec<Saved>)> = Vec::new();
        let level = command.cin;
        let c: Vec<f64> = used.iter().map(|&k| weight(k)).collect();
        for (d, dependent) in dependents.iter().enumerate() {
            let y = &ys[d];
            let y_text = shown.named(data, dependent);
            let mut fitted = Vec::new();
            for &model in models {
                let rows: Option<Vec<(Vec<f64>, f64)>> = used
                    .iter()
                    .map(|&k| Some((terms(model, x[k])?, linear_y(model, y[k], inv_u)?)))
                    .collect();
                let Some(rows) = rows else {
                    warnings.push(format!(
                        "the {} model of {dependent} cannot be fitted: its logarithm or inverse is undefined for some case",
                        label(model)
                    ));
                    continue;
                };
                let (xs, yl): (Vec<Vec<f64>>, Vec<f64>) = rows.into_iter().unzip();
                let g = vec![1.0; xs.len()];
                let ls = least_squares(&xs, &yl, &c, &g, command.constant)
                    .filter(|ls| ls.df_res() > 0.0);
                match ls {
                    Some(ls) if ls.tolerance >= TOLERANCE => fitted.push(Fitted { model, ls }),
                    Some(_) => warnings.push(format!(
                        "the {} model of {dependent} is not fitted: a term's tolerance is below {TOLERANCE}",
                        label(model)
                    )),
                    None => warnings.push(format!(
                        "the {} model of {dependent} cannot be fitted: too few distinct cases",
                        label(model)
                    )),
                }
            }
            tables.push(estimates_table(
                &fitted,
                &y_text,
                &x_text,
                models,
                command.upper_bound,
            ));
            if command.anova {
                for f in &fitted {
                    let caption = Some(format!(
                        "Dependent variable: {y_text}. {} model.",
                        label(f.model)
                    ));
                    tables.push(summary_table(&f.ls, Vec::new(), caption.clone()));
                    tables.push(anova_table(&f.ls, caption));
                    tables.push(coefficients_table(f, &y_text, &x_text, command.upper_bound));
                }
            }
            if command.plot && !fitted.is_empty() {
                let mut order = used.clone();
                order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
                let point = |k: usize, v: f64, labelled: bool| Point {
                    x: x[k],
                    y: v,
                    z: None,
                    label: labelled
                        .then(|| id.as_ref().map(|id| id[k].clone()))
                        .flatten(),
                };
                let mut groups = vec![PointGroup {
                    name: "Observed".into(),
                    points: order.iter().map(|&k| point(k, y[k], true)).collect(),
                }];
                for f in &fitted {
                    groups.push(PointGroup {
                        name: label(f.model).into(),
                        points: order
                            .iter()
                            .filter_map(|&k| Some(point(k, f.predict(x[k], inv_u, 0.0)?.0, false)))
                            .collect(),
                    });
                }
                let kind = match command.independent {
                    Some(_) => ChartKind::Scatter,
                    None => ChartKind::Line,
                };
                charts.push(Chart::plot(
                    kind,
                    format!("CURVEFIT: {y_text}"),
                    Axis::titled(x_text.clone()),
                    Axis::titled(y_text.clone()),
                    Plot::Points {
                        groups,
                        lines: Vec::new(),
                        z_axis: None,
                    },
                ));
            }
            if command.save_pred || command.save_resid || command.save_cin {
                for f in &fitted {
                    let t_crit = t_quantile((1.0 - level / 100.0) / 2.0, f.ls.df_res());
                    let values = (0..cases)
                        .map(|k| {
                            let Some((fit, low, high)) = (!x[k].is_nan())
                                .then(|| f.predict(x[k], inv_u, t_crit))
                                .flatten()
                            else {
                                return [None; 4];
                            };
                            let finite = |v: f64| v.is_finite().then_some(v);
                            [finite(fit), finite(y[k] - fit), finite(low), finite(high)]
                        })
                        .collect();
                    saves.push((dependent.clone(), f.model, values));
                }
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        if !saves.is_empty() {
            let wanted = [
                command.save_pred,
                command.save_resid,
                command.save_cin,
                command.save_cin,
            ];
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (dependent, model, values) in &saves {
                    let roots = ["FIT", "ERR", "LCL", "UCL"];
                    let names = |n: usize| roots.map(|root| format!("{root}_{n}"));
                    let n = (1..)
                        .find(|&n| {
                            names(n)
                                .iter()
                                .zip(wanted)
                                .all(|(name, w)| !w || data.variable_index(name).is_none())
                        })
                        .expect("a free number");
                    let model = label(*model);
                    let labels = [
                        format!("Fit of {dependent}, {model} model"),
                        format!("Residual of {dependent}, {model} model"),
                        format!("{level}% lower prediction limit of {dependent}, {model} model"),
                        format!("{level}% upper prediction limit of {dependent}, {model} model"),
                    ];
                    for (i, name) in names(n).iter().enumerate().filter(|(i, _)| wanted[*i]) {
                        data.set_numeric_values(name, values.iter().map(|v| v[i]).collect())?;
                        data.set_variable_label(name, Some(&labels[i]))?;
                        data.set_format(name, "F11.5")?;
                        data.set_measure(name, Measure::Scale)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

/// R square, F test and the estimates of every fitted model of one dependent.
fn estimates_table(
    fitted: &[Fitted],
    y_text: &str,
    x_text: &str,
    models: &[CurveModel],
    upper: Option<f64>,
) -> Table {
    let logistic = models.contains(&CurveModel::Logistic);
    let rows = fitted
        .iter()
        .map(|f| {
            let (fv, sig) = f.ls.f();
            let mut row = vec![
                text(label(f.model)),
                number(Some(f.ls.r_square())),
                number(Some(fv)),
                Cell::Number(f.ls.df_reg()),
                Cell::Number(f.ls.df_res()),
                number(Some(sig)),
                number(f.constant().map(|c| c.0)),
            ];
            row.extend((0..3).map(|a| match a < f.ls.slopes.len() {
                true => Cell::Number(f.slope(a).0),
                false => Cell::Empty,
            }));
            if logistic {
                row.push(match f.model {
                    CurveModel::Logistic => number(upper),
                    _ => Cell::Empty,
                });
            }
            row
        })
        .collect();
    let group = |g: &str, names: &[&str]| -> Vec<(String, String)> {
        names
            .iter()
            .map(|n| (g.to_string(), n.to_string()))
            .collect()
    };
    let mut columns = vec![(String::new(), "Equation".to_string())];
    columns.extend(group(
        "Model Summary",
        &["R Square", "F", "df1", "df2", "Sig."],
    ));
    columns.extend(group(
        "Parameter Estimates",
        &["Constant", "b1", "b2", "b3"],
    ));
    if logistic {
        columns.extend(group("Parameter Estimates", &["Upper Bound"]));
    }
    let independent = match x_text {
        "Sequence" => "the case sequence".to_string(),
        x => x.to_string(),
    };
    table(
        "Model Summary and Parameter Estimates",
        1,
        columns,
        rows,
        Some(format!(
            "Dependent variable: {y_text}. The independent variable is {independent}."
        )),
    )
}

/// The coefficients of one model, back on the scale of its equation, constant last.
fn coefficients_table(f: &Fitted, y_text: &str, x_text: &str, upper: Option<f64>) -> Table {
    let df = f.ls.df_res();
    let mut rows: Vec<Vec<Cell>> = term_names(f.model, x_text)
        .into_iter()
        .enumerate()
        .map(|(a, name)| {
            let (b, se) = f.slope(a);
            let t = f.ls.slopes[a] / f.ls.slope_se(a);
            vec![
                text(name),
                Cell::Number(b),
                number(Some(se)),
                number(Some(f.ls.beta(a).0)),
                number(Some(t)),
                number(Some(t_p(t, df))),
            ]
        })
        .collect();
    if let Some((b, se)) = f.constant() {
        let t = f.ls.intercept / f.ls.intercept_se();
        rows.push(vec![
            text("(Constant)"),
            Cell::Number(b),
            number(Some(se)),
            Cell::Empty,
            number(Some(t)),
            number(Some(t_p(t, df))),
        ]);
    }
    let columns = vec![
        (String::new(), String::new()),
        ("Unstandardized Coefficients".into(), "B".into()),
        ("Unstandardized Coefficients".into(), "Std. Error".into()),
        ("Standardized Coefficients".into(), "Beta".into()),
        (String::new(), "t".into()),
        (String::new(), "Sig.".into()),
    ];
    let form = match (f.model, upper) {
        (CurveModel::Logistic, Some(u)) => {
            format!(" The dependent variable is ln(1 / {y_text} - 1 / {u}).")
        }
        (CurveModel::Logistic, None) => format!(" The dependent variable is ln(1 / {y_text})."),
        (m, _) if logged(m) => format!(" The dependent variable is ln({y_text})."),
        _ => String::new(),
    };
    table(
        "Coefficients",
        1,
        columns,
        rows,
        Some(format!("{} model of {y_text}.{form}", label(f.model))),
    )
}
