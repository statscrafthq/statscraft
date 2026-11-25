//! SIMPLAN, SIMRUN and the BEGIN EXPR block: simulation plans and Monte Carlo runs.
//!
//! Rules, from the command descriptions and the simulation algorithms:
//! - BEGIN EXPR to END EXPR collects NUMERIC, STRING, VARIABLE LEVEL, VALUE LABELS and COMPUTE
//!   without running them; END EXPR writes the model: the expressions in order, the declared
//!   inputs, levels and labels. A variable is computed once; targets are numeric.
//! - The plan holds every simulated input with its distribution, every fixed input, the targets,
//!   the correlations and the stopping rule. Every input of a model is simulated or fixed.
//! - SIMRUN draws correlated standard normals, takes each input as F⁻¹(Φ(z)) (NORTA), with the
//!   correlation of each pair of normals searched by bisection on 1000 draws until the inputs
//!   correlate as planned (within 0.01); then it evaluates the expressions.
//! - Tornado: the Pearson correlation of target and input; the mean change of the target when
//!   one input moves by k standard deviations; the total share of variance (Sobol) from a second
//!   sample whose column of that input replaces the first sample's.
//! - Files whose name starts with `*\` stay in memory for the session.

use crate::chart::{Axis, Category, Chart, ChartKind, Orientation, Series, Statistic, Tier};
use crate::plot::{BoxSummary, Plot, Point, PointGroup};
use crate::sim_dist::{Dist, Params, autofit, empirical, fit};
use crate::twostep::plain_table;
use crate::{Cell, Engine, Error, Table};
use serde::{Deserialize, Serialize};
use statscraft_data::{Dataset, Measure, Value};
use statscraft_syntax::{
    BinaryOp, Command, Expr, Level, SimInput, Simplan, Simrun, StopRule, UnaryOp, VarItem,
};

/// What the engine keeps between commands: an open BEGIN EXPR block and the files kept in
/// memory.
#[derive(Default, Clone)]
pub(crate) struct State {
    block: Option<(String, Vec<Command>)>,
    files: std::collections::HashMap<String, String>,
}

const MODEL_FORMAT: &str = "statscraft-simulation-model";
const PLAN_FORMAT: &str = "statscraft-simulation-plan";

/// A model of expressions, as END EXPR writes it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Model {
    format: String,
    /// (target, expression) in the order they are computed.
    expressions: Vec<(String, String)>,
    /// The inputs NUMERIC declared, which are not in the active dataset.
    #[serde(default)]
    declared: Vec<String>,
    /// (name, scale | ordinal | nominal)
    #[serde(default)]
    levels: Vec<(String, String)>,
    #[serde(default)]
    labels: Vec<(String, Vec<(f64, String)>)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Field {
    name: String,
    level: String,
    #[serde(default)]
    format: Option<String>,
    output: bool,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    min_inclusive: bool,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    max_inclusive: bool,
    #[serde(default)]
    labels: Vec<(f64, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FitRow {
    family: String,
    statistic: f64,
    p: Option<f64>,
    params: Params,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Input {
    #[serde(flatten)]
    field: Field,
    family: String,
    params: Params,
    #[serde(default)]
    trials: bool,
    #[serde(default)]
    minval: Option<f64>,
    #[serde(default)]
    maxval: Option<f64>,
    fitted: bool,
    /// The parameter a sensitivity analysis varies, with its value for each iteration.
    #[serde(default)]
    iterate: Option<(String, Vec<Vec<f64>>)>,
    /// The goodness of fit of the tested distributions, best first.
    #[serde(default)]
    fit: Vec<FitRow>,
    #[serde(default)]
    statistic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Fixed {
    #[serde(flatten)]
    field: Field,
    /// One value, or one per iteration.
    values: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stop {
    target: String,
    /// MEAN (confidence interval width) or TAIL.
    mean: bool,
    thresh: f64,
    /// UNIT=PERCENT or PERCENTILE.
    relative: bool,
    level: f64,
    left: bool,
    frequency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    format: String,
    #[serde(default)]
    model: Option<Model>,
    #[serde(default)]
    targets: Vec<Field>,
    inputs: Vec<Input>,
    #[serde(default)]
    fixed: Vec<Fixed>,
    /// Over `inputs`; empty for none.
    #[serde(default)]
    correlations: Vec<Vec<f64>>,
    max_cases: usize,
    #[serde(default)]
    stop: Option<Stop>,
}

/// The expression as syntax, every operation in parentheses.
fn expr_text(e: &Expr) -> String {
    match e {
        Expr::Number(n) => format!("{n}"),
        Expr::String(s) => format!("'{}'", s.replace('\'', "''")),
        Expr::Variable(v) => v.clone(),
        Expr::Unary { op, operand } => match op {
            UnaryOp::Neg => format!("(-{})", expr_text(operand)),
            UnaryOp::Not => format!("(NOT {})", expr_text(operand)),
        },
        Expr::Binary { op, left, right } => {
            let op = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Pow => "**",
                BinaryOp::Eq => "=",
                BinaryOp::Ne => "<>",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::And => "AND",
                BinaryOp::Or => "OR",
            };
            format!("({} {op} {})", expr_text(left), expr_text(right))
        }
        Expr::Call { name, args } => {
            let args: Vec<String> = args.iter().map(expr_text).collect();
            format!("{name}({})", args.join(", "))
        }
        Expr::Range(a, b) => format!("{a} TO {b}"),
    }
}

/// The variables an expression reads.
fn variables(e: &Expr, out: &mut Vec<String>) -> Result<(), Error> {
    match e {
        Expr::Variable(v) if !v.starts_with('$') => {
            if !out.iter().any(|o| o.eq_ignore_ascii_case(v)) {
                out.push(v.clone());
            }
        }
        Expr::Unary { operand, .. } => variables(operand, out)?,
        Expr::Binary { left, right, .. } => {
            variables(left, out)?;
            variables(right, out)?;
        }
        Expr::Call { args, .. } => args.iter().try_for_each(|a| variables(a, out))?,
        Expr::Range(..) => {
            return Err(Error(
                "a model expression cannot use a TO list; name each variable".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

impl Model {
    /// The COMPUTE commands and the inputs: what the expressions read before computing it.
    fn compile(&self) -> Result<(Vec<Command>, Vec<String>), Error> {
        let (mut commands, mut inputs) = (Vec::new(), Vec::new());
        for (target, expression) in &self.expressions {
            let (parsed, errors) =
                statscraft_syntax::parse(&format!("COMPUTE {target} = {expression}."));
            let command = match (parsed.into_iter().next(), errors.first()) {
                (Some(p), None) => p.command,
                _ => {
                    return Err(Error(format!(
                        "the model expression of {target} is invalid"
                    )));
                }
            };
            if let Command::Compute { expr, .. } = &command {
                let mut read = Vec::new();
                variables(expr, &mut read)?;
                for v in read {
                    let computed = commands.iter().any(|c: &Command| {
                        matches!(c, Command::Compute { target, .. } if target.eq_ignore_ascii_case(&v))
                    });
                    if !computed && !inputs.iter().any(|i: &String| i.eq_ignore_ascii_case(&v)) {
                        inputs.push(v);
                    }
                }
            }
            commands.push(command);
        }
        Ok((commands, inputs))
    }

    fn level(&self, name: &str) -> Option<String> {
        let found = self
            .levels
            .iter()
            .rev()
            .find(|(n, _)| n.eq_ignore_ascii_case(name));
        found.map(|(_, l)| l.clone())
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Scale => "scale",
        Level::Ordinal => "ordinal",
        Level::Nominal => "nominal",
    }
}

fn find<'a>(names: impl IntoIterator<Item = &'a String>, name: &str) -> bool {
    names.into_iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// "F,4" as a print format such as F12.4.
fn print_format(spec: &str) -> String {
    let (kind, d) = spec.split_once(',').unwrap_or((spec, "0"));
    let d: usize = d.parse().unwrap_or(0);
    format!("{kind}{}.{d}", (d + 8).max(12))
}

/// A number for a text cell, to six decimals without trailing zeros.
fn short(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn params_text(params: &Params) -> String {
    let text = |(name, values): &(String, Vec<f64>)| match name.as_str() {
        "POINTS" => format!("{} points", values.len()),
        _ => {
            let values: Vec<String> = values.iter().map(|v| short(*v)).collect();
            format!("{name}={}", values.join(" "))
        }
    };
    params.iter().map(text).collect::<Vec<_>>().join("; ")
}

impl Engine {
    fn put_file(&mut self, who: &str, file: &str, text: String) -> Result<(), Error> {
        if file.starts_with("*\\") || self.dry_run {
            self.simulation.files.insert(file.to_lowercase(), text);
            return Ok(());
        }
        let path = self.path(file);
        std::fs::write(&path, text + "\n")
            .map_err(|e| Error(format!("{who} cannot write {}: {e}", path.display())))
    }

    fn get_file(&self, file: &str) -> Result<String, Error> {
        if let Some(text) = self.simulation.files.get(&file.to_lowercase()) {
            return Ok(text.clone());
        }
        let path = self.path(file);
        std::fs::read_to_string(&path)
            .map_err(|e| Error(format!("cannot read {}: {e}", path.display())))
    }

    /// The commands of a BEGIN EXPR block are kept, not run. None for any other command.
    pub(crate) fn collect_expr(&mut self, command: &Command) -> Option<Result<Vec<Table>, Error>> {
        match (command, &mut self.simulation.block) {
            (Command::BeginExpr(file), None) => {
                self.simulation.block = Some((file.clone(), Vec::new()));
                Some(Ok(Vec::new()))
            }
            (Command::BeginExpr(_), Some(_)) => Some(Err(Error(
                "BEGIN EXPR inside BEGIN EXPR: end the first block with END EXPR".into(),
            ))),
            (Command::EndExpr, None) => Some(Err(Error("END EXPR without BEGIN EXPR".into()))),
            (Command::EndExpr, Some(_)) => {
                let (file, commands) = self.simulation.block.take().expect("matched");
                Some(self.end_expr(&file, &commands).map(|()| Vec::new()))
            }
            (
                Command::Numeric(_)
                | Command::String(_)
                | Command::VariableLevel(_)
                | Command::ValueLabels(_)
                | Command::Compute { .. },
                Some((_, commands)),
            ) => {
                commands.push(command.clone());
                Some(Ok(Vec::new()))
            }
            (_, Some(_)) => {
                self.simulation.block = None;
                Some(Err(Error(
                    "only NUMERIC, STRING, VARIABLE LEVEL, VALUE LABELS and COMPUTE may stand \
                     between BEGIN EXPR and END EXPR; the block is dropped"
                        .into(),
                )))
            }
            _ => None,
        }
    }

    fn end_expr(&mut self, file: &str, commands: &[Command]) -> Result<(), Error> {
        let mut model = Model {
            format: MODEL_FORMAT.into(),
            ..Model::default()
        };
        let names = |items: &[VarItem]| -> Result<Vec<String>, Error> {
            items
                .iter()
                .map(|i| match i {
                    VarItem::Name(n) => Ok(n.clone()),
                    _ => Err(Error("name each variable in a BEGIN EXPR block".into())),
                })
                .collect()
        };
        for command in commands {
            match command {
                Command::Numeric(groups) => model
                    .declared
                    .extend(groups.iter().flat_map(|g| g.0.clone())),
                Command::String(_) => {
                    return Err(Error("string inputs are not supported yet".into()));
                }
                Command::VariableLevel(groups) => {
                    for (items, level) in groups {
                        for n in names(items)? {
                            model.levels.push((n, level_name(*level).into()));
                        }
                    }
                }
                Command::ValueLabels(groups) => {
                    for g in groups {
                        let mut labels = Vec::new();
                        for (value, label) in &g.labels {
                            match value {
                                statscraft_syntax::Value::Number(v) => {
                                    labels.push((*v, label.clone()))
                                }
                                _ => {
                                    return Err(Error(
                                        "string values are not supported yet".into(),
                                    ));
                                }
                            }
                        }
                        for n in names(&g.vars)? {
                            model.labels.push((n, labels.clone()));
                        }
                    }
                }
                Command::Compute { target, expr } => {
                    if find(model.expressions.iter().map(|e| &e.0), target)
                        || find(&model.declared, target)
                    {
                        return Err(Error(format!(
                            "{target} is computed twice or declared as an input; a model \
                             variable gets one value"
                        )));
                    }
                    model.expressions.push((target.clone(), expr_text(expr)));
                }
                _ => {}
            }
        }
        if model.expressions.is_empty() {
            return Err(Error("the BEGIN EXPR block has no COMPUTE".into()));
        }
        let (_, inputs) = model.compile()?;
        let in_data = |n: &str| {
            self.dataset
                .as_ref()
                .is_some_and(|d| d.variable_index(n).is_some())
        };
        for input in &inputs {
            if find(model.expressions.iter().map(|e| &e.0), input) {
                return Err(Error(format!(
                    "{input} is used before the COMPUTE that gives it"
                )));
            }
            match (find(&model.declared, input), in_data(input)) {
                (false, false) => {
                    return Err(Error(format!(
                        "the input {input} is neither in the active dataset nor declared with NUMERIC"
                    )));
                }
                (true, true) => {
                    return Err(Error(format!(
                        "{input} is in the active dataset; do not declare it with NUMERIC"
                    )));
                }
                _ => {}
            }
        }
        let text = serde_json::to_string_pretty(&model).expect("a model serializes");
        self.put_file("END EXPR", file, text)
    }

    /// The model in a file from END EXPR or in a plan.
    fn read_model(&self, file: &str) -> Result<Model, Error> {
        let text = self.get_file(file)?;
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let unsupported = || {
            Error(format!(
                "{file} is no model from BEGIN EXPR or plan from SIMPLAN; model files of other \
                 procedures (PMML) are not supported yet"
            ))
        };
        match value.get("format").and_then(|f| f.as_str()) {
            Some(MODEL_FORMAT) => serde_json::from_value(value).map_err(|_| unsupported()),
            Some(PLAN_FORMAT) => serde_json::from_value::<Plan>(value)
                .ok()
                .and_then(|p| p.model)
                .ok_or_else(|| Error(format!("the plan {file} has no model"))),
            _ => Err(unsupported()),
        }
    }

    fn read_sim_plan(&self, file: &str) -> Result<Plan, Error> {
        let text = self.get_file(file)?;
        serde_json::from_str::<Plan>(&text)
            .ok()
            .filter(|p| p.format == PLAN_FORMAT)
            .ok_or_else(|| Error(format!("{file} is no simulation plan from SIMPLAN")))
    }

    pub(crate) fn simplan(&mut self, c: &Simplan) -> Result<Vec<Table>, Error> {
        if c.view {
            return Ok(plan_tables(&self.read_sim_plan(&c.plan)?));
        }
        if c.source.is_some() {
            return Err(Error(
                "SIMPLAN SOURCE='file' is not supported yet; leave it out to fit to the active dataset"
                    .into(),
            ));
        }
        let contingency = c
            .inputs
            .iter()
            .any(|i| i.distribution.as_ref().is_some_and(|d| d.contingency));
        if c.multiway || contingency {
            return Err(Error(
                "contingency tables of categorical inputs (MULTIWAY=YES) are not supported yet"
                    .into(),
            ));
        }
        let model = c
            .model
            .as_ref()
            .map(|(file, _)| self.read_model(file))
            .transpose()?;
        let (model_inputs, targets) = match &model {
            Some(m) => (
                m.compile()?.1,
                m.expressions.iter().map(|e| e.0.clone()).collect(),
            ),
            None => (Vec::new(), Vec::<String>::new()),
        };
        let data = self.dataset.as_ref();
        let measure = |name: &str| {
            data.and_then(|d| d.variable(name).ok())
                .map(|v| match v.measure() {
                    Measure::Nominal => "nominal",
                    Measure::Ordinal => "ordinal",
                    _ => "scale",
                })
        };
        let mut specs: Vec<(&SimInput, String, Option<String>)> = Vec::new();
        for spec in &c.inputs {
            for f in &spec.inputs {
                if model.is_some() && !find(&model_inputs, &f.name) {
                    return Err(Error(format!("{} is no input of the model", f.name)));
                }
                if specs.iter().any(|s| s.1.eq_ignore_ascii_case(&f.name)) {
                    return Err(Error(format!("{} is simulated twice", f.name)));
                }
                specs.push((spec, f.name.clone(), f.mapto.clone()));
            }
        }
        // the cases every fitted input is valid on; weights count
        let fits_data = |s: &SimInput| {
            s.autofit.is_some()
                || s.distribution
                    .as_ref()
                    .is_some_and(|d| d.family == "EMPIRICAL")
        };
        let mut columns: Vec<Option<Vec<Option<f64>>>> = Vec::new();
        let n = data.map_or(0, |d| d.case_count().min(c.ncases.unwrap_or(usize::MAX)));
        let mut valid = vec![true; n];
        let weights: Vec<f64> = match data.and_then(|d| d.weight_variable()) {
            Some(w) => {
                let v = data.expect("weighted").variable(w)?;
                let values = v.numeric()?;
                (0..n)
                    .map(|i| {
                        values
                            .get(i)
                            .filter(|x| !v.is_user_missing(*x))
                            .unwrap_or(0.0)
                            .max(0.0)
                    })
                    .collect()
            }
            None => vec![1.0; n],
        };
        for (spec, name, mapto) in &specs {
            let field = mapto.as_deref().unwrap_or(name);
            let Some(variable) = data.and_then(|d| d.variable(field).ok()) else {
                if fits_data(spec) {
                    return Err(Error(format!(
                        "{name} is fitted to the data, but the active dataset has no field {field}; \
                         name it with MAPTO or give TYPE=MANUAL with a DISTRIBUTION"
                    )));
                }
                columns.push(None);
                continue;
            };
            let values = variable.numeric().map_err(|_| {
                Error(format!(
                    "{field} is a string; string inputs are not supported yet"
                ))
            })?;
            let level = model
                .as_ref()
                .and_then(|m| m.level(name))
                .unwrap_or_else(|| measure(field).unwrap_or("scale").into());
            let categorical = match &spec.autofit {
                Some(list) => {
                    list.iter().any(|f| f == "CATEGORICAL")
                        || (list.is_empty() && level == "nominal")
                }
                None => false,
            };
            let column: Vec<Option<f64>> = (0..n)
                .map(|i| {
                    values.get(i).filter(|x| {
                        categorical && c.include_missing || !variable.is_user_missing(*x)
                    })
                })
                .collect();
            if fits_data(spec) {
                for (ok, v) in valid.iter_mut().zip(&column) {
                    *ok &= v.is_some();
                }
            }
            columns.push(Some(column));
        }
        for (ok, w) in valid.iter_mut().zip(&weights) {
            *ok &= *w > 0.0;
        }
        let mut inputs = Vec::new();
        for ((spec, name, mapto), column) in specs.iter().zip(&columns) {
            let field = mapto.as_deref().unwrap_or(name);
            let level = model
                .as_ref()
                .and_then(|m| m.level(name))
                .unwrap_or_else(|| measure(field).unwrap_or("scale").into());
            let sample = || -> (Vec<f64>, Vec<f64>) {
                let column = column.as_ref().expect("checked above");
                (0..n)
                    .filter(|&i| valid[i])
                    .map(|i| (column[i].expect("valid"), weights[i]))
                    .unzip()
            };
            let format = spec
                .inputs
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                .and_then(|f| f.format.clone());
            let mut input = Input {
                field: Field {
                    name: name.clone(),
                    level: level.clone(),
                    format,
                    output: spec.output,
                    labels: model
                        .as_ref()
                        .and_then(|m| {
                            m.labels
                                .iter()
                                .rev()
                                .find(|l| l.0.eq_ignore_ascii_case(name))
                                .map(|l| l.1.clone())
                        })
                        .unwrap_or_default(),
                    ..Field::default()
                },
                family: String::new(),
                params: Vec::new(),
                trials: false,
                minval: spec.minval,
                maxval: spec.maxval,
                fitted: fits_data(spec),
                iterate: None,
                fit: Vec::new(),
                statistic: String::new(),
            };
            match (&spec.autofit, &spec.distribution) {
                (Some(list), _) => {
                    let (x, w) = sample();
                    let families: Vec<&'static str> = match (list.is_empty(), level.as_str()) {
                        (true, "nominal") => vec!["CATEGORICAL"],
                        (true, "ordinal") => vec!["BINOM", "NEGBIN", "POISSON"],
                        (true, _) => vec![
                            "BETA",
                            "EXP",
                            "GAMMA",
                            "LNORMAL",
                            "NORMAL",
                            "TRIANGULAR",
                            "UNIFORM",
                            "WEIBULL",
                        ],
                        _ => crate::sim_dist::PARAMS
                            .iter()
                            .map(|p| p.0)
                            .filter(|f| list.iter().any(|l| l == f))
                            .collect(),
                    };
                    let fits = autofit(&families, &x, &w, c.ks, c.max_iterations);
                    let best = fits.first().ok_or_else(|| {
                        Error(format!(
                            "none of {} fits the data of {name} ({} valid cases)",
                            families.join(" "),
                            x.len()
                        ))
                    })?;
                    input.family = best.family.into();
                    input.params = best.params.clone();
                    input.statistic = match (best.family, c.ks) {
                        ("CATEGORICAL" | "BINOM" | "NEGBIN" | "POISSON", _) => "Chi-Square",
                        (_, true) => "Kolmogorov-Smirnov",
                        _ => "Anderson-Darling",
                    }
                    .into();
                    input.fit = fits
                        .iter()
                        .map(|f| FitRow {
                            family: f.family.into(),
                            statistic: f.statistic,
                            p: f.p,
                            params: f.params.clone(),
                        })
                        .collect();
                }
                (None, Some(d)) if d.family == "EMPIRICAL" => {
                    let (x, w) = sample();
                    if x.is_empty() {
                        return Err(Error(format!(
                            "{name} has no valid values to build its empirical distribution"
                        )));
                    }
                    (input.family, input.params) = match level.as_str() {
                        "nominal" => {
                            return Err(Error(format!(
                                "{name} is nominal: give TYPE=AUTOFIT(CATEGORICAL) rather than EMPIRICAL"
                            )));
                        }
                        "ordinal" => (
                            "CATEGORICAL".into(),
                            fit("CATEGORICAL", &x, &w, 0).expect("categories"),
                        ),
                        _ => ("EMPIRICAL".into(), empirical(&x, &w, c.bins)),
                    };
                }
                (None, Some(d)) => {
                    input.family = d.family.clone();
                    input.trials = d.trials;
                    for (pname, sets) in &d.params {
                        if statscraft_syntax::is_iterated(&d.family, pname, sets) {
                            let values: Vec<Vec<f64>> = match sets.len() {
                                1 => sets[0].iter().map(|v| vec![*v]).collect(),
                                _ => sets.clone(),
                            };
                            input.params.push((pname.clone(), values[0].clone()));
                            input.iterate = Some((pname.clone(), values));
                        } else {
                            input.params.push((pname.clone(), sets[0].clone()));
                        }
                    }
                }
                (None, None) => unreachable!("the parser asks for a distribution"),
            }
            let check = |params: &Params| {
                Dist::new(&input.family, params, input.trials)
                    .map_err(|e| Error(format!("{name}: {e}")))
            };
            check(&input.params)?;
            if let Some((pname, values)) = &input.iterate {
                for v in values {
                    let mut params = input.params.clone();
                    params
                        .iter_mut()
                        .filter(|p| &p.0 == pname)
                        .for_each(|p| p.1 = v.clone());
                    check(&params)?;
                }
            }
            let bounded = [
                "BERNOULLI",
                "CATEGORICAL",
                "EMPIRICAL",
                "TRIANGULAR",
                "UNIFORM",
                "USER_RANGES",
            ];
            if (input.minval.is_some() || input.maxval.is_some())
                && bounded.contains(&input.family.as_str())
            {
                return Err(Error(format!(
                    "MINVAL and MAXVAL do not apply to the {} distribution of {name}",
                    input.family
                )));
            }
            if let (Some(a), Some(b)) = (input.minval, input.maxval)
                && a >= b
            {
                return Err(Error(format!("{name}: MINVAL must be below MAXVAL")));
            }
            inputs.push(input);
        }
        let mut fixed: Vec<Fixed> = Vec::new();
        for spec in &c.fixed {
            for f in &spec.inputs {
                if model.is_some() && !find(&model_inputs, &f.name) {
                    return Err(Error(format!("{} is no input of the model", f.name)));
                }
                if find(inputs.iter().map(|i| &i.field.name), &f.name)
                    || find(fixed.iter().map(|x| &x.field.name), &f.name)
                {
                    return Err(Error(format!("{} is given twice as an input", f.name)));
                }
                let level = model
                    .as_ref()
                    .and_then(|m| m.level(&f.name))
                    .unwrap_or_else(|| "scale".into());
                fixed.push(Fixed {
                    field: Field {
                        name: f.name.clone(),
                        level,
                        format: f.format.clone(),
                        output: true,
                        ..Field::default()
                    },
                    values: spec.values.clone(),
                });
            }
        }
        let missing: Vec<&String> = model_inputs
            .iter()
            .filter(|n| {
                !find(inputs.iter().map(|i| &i.field.name), n)
                    && !find(fixed.iter().map(|f| &f.field.name), n)
            })
            .collect();
        if !missing.is_empty() {
            let missing: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            return Err(Error(format!(
                "every model input is simulated (SIMINPUT) or fixed (FIXEDINPUT); not given: {}",
                missing.join(" ")
            )));
        }
        // correlations: given, or Pearson over the fitted cases for the fields in the data
        let k = inputs.len();
        let categorical = |i: &Input| i.family == "CATEGORICAL";
        let mut r = vec![vec![0.0; k]; k];
        (0..k).for_each(|i| r[i][i] = 1.0);
        match &c.correlations {
            Some((order, matrix)) => {
                let index: Vec<usize> = order
                    .iter()
                    .map(|n| {
                        inputs.iter().position(|i| i.field.name.eq_ignore_ascii_case(n)).filter(|&i| !categorical(&inputs[i])).ok_or_else(|| {
                            Error(format!("{n} on VARORDER is no simulated input with a non-categorical distribution"))
                        })
                    })
                    .collect::<Result<_, _>>()?;
                for (a, &i) in index.iter().enumerate() {
                    for (b, &j) in index.iter().enumerate() {
                        if i != j {
                            r[i][j] = matrix[a][b];
                        }
                    }
                }
            }
            None => {
                for i in 0..k {
                    for j in 0..i {
                        if let (Some(x), Some(y)) = (&columns[i], &columns[j])
                            && !categorical(&inputs[i])
                            && !categorical(&inputs[j])
                        {
                            let rows: Vec<usize> = (0..n)
                                .filter(|&c| valid[c] && x[c].is_some() && y[c].is_some())
                                .collect();
                            let a: Vec<f64> = rows.iter().map(|&c| x[c].expect("some")).collect();
                            let b: Vec<f64> = rows.iter().map(|&c| y[c].expect("some")).collect();
                            let w: Vec<f64> = rows.iter().map(|&c| weights[c]).collect();
                            let v = pearson(&a, &b, Some(&w));
                            if v.is_finite() {
                                (r[i][j], r[j][i]) = (v, v);
                            }
                        }
                    }
                }
            }
        }
        if cholesky(&r).is_none() {
            return Err(Error("the correlation matrix of the inputs is not positive definite (inputs fitted to the same field correlate fully); give CORRELATIONS".into()));
        }
        let mut target_fields: Vec<Field> = targets
            .iter()
            .map(|t| Field {
                name: t.clone(),
                level: model
                    .as_ref()
                    .and_then(|m| m.level(t))
                    .unwrap_or_else(|| "scale".into()),
                output: true,
                min_inclusive: true,
                max_inclusive: true,
                labels: model
                    .as_ref()
                    .and_then(|m| {
                        m.labels
                            .iter()
                            .rev()
                            .find(|l| l.0.eq_ignore_ascii_case(t))
                            .map(|l| l.1.clone())
                    })
                    .unwrap_or_default(),
                ..Field::default()
            })
            .collect();
        for opts in &c.targets {
            for f in &opts.targets {
                let t = target_fields
                    .iter_mut()
                    .find(|t| t.name.eq_ignore_ascii_case(&f.name))
                    .ok_or_else(|| Error(format!("{} is no target of the model", f.name)))?;
                t.format = f.format.clone().or(t.format.take());
                t.output = opts.output;
                t.comment = opts.comment.clone().or(t.comment.take());
                if let Some(filter) = opts.filter {
                    (t.min, t.min_inclusive, t.max, t.max_inclusive) = (
                        filter.min,
                        filter.min_inclusive,
                        filter.max,
                        filter.max_inclusive,
                    );
                }
            }
        }
        for (vars, labels) in &c.value_labels {
            let labels: Vec<(f64, String)> = labels
                .iter()
                .map(|(v, l)| match v {
                    statscraft_syntax::Value::Number(v) => Ok((*v, l.clone())),
                    _ => Err(Error("string values are not supported yet".into())),
                })
                .collect::<Result<_, _>>()?;
            for v in vars {
                let field = inputs
                    .iter_mut()
                    .map(|i| &mut i.field)
                    .chain(fixed.iter_mut().map(|f| &mut f.field))
                    .find(|f| f.name.eq_ignore_ascii_case(v));
                field
                    .ok_or_else(|| Error(format!("VALUELABELS: {v} is no input")))?
                    .labels = labels.clone();
            }
        }
        let stop = match (&c.stop_target, c.stop_rule) {
            (Some(t), Some(rule)) => {
                if !find(&targets, t) {
                    return Err(Error(format!(
                        "STOPCRITERIA TARGET={t} is no target of the model"
                    )));
                }
                Some(match rule {
                    StopRule::Mean {
                        thresh,
                        percent,
                        level,
                    } => Stop {
                        target: t.clone(),
                        mean: true,
                        thresh,
                        relative: percent,
                        level,
                        left: false,
                        frequency: 0,
                    },
                    StopRule::Tail {
                        thresh,
                        percentile,
                        left,
                        frequency,
                    } => Stop {
                        target: t.clone(),
                        mean: false,
                        thresh,
                        relative: percentile,
                        level: 0.0,
                        left,
                        frequency,
                    },
                })
            }
            _ => None,
        };
        let plan = Plan {
            format: PLAN_FORMAT.into(),
            model,
            targets: target_fields,
            inputs,
            fixed,
            correlations: if (0..k).any(|i| (0..k).any(|j| i != j && r[i][j] != 0.0)) {
                r
            } else {
                Vec::new()
            },
            max_cases: c.max_cases,
            stop,
        };
        let text = serde_json::to_string_pretty(&plan).expect("a plan serializes");
        self.put_file("SIMPLAN", &c.plan, text)?;
        Ok(if c.display {
            plan_tables(&plan)
        } else {
            Vec::new()
        })
    }
}

/// What SIMPLAN shows of a plan.
fn plan_tables(plan: &Plan) -> Vec<Table> {
    let text = |s: &str| Cell::Text(s.into());
    let number = |v: Option<f64>| v.map_or(Cell::Empty, Cell::Number);
    let mut tables = vec![plain_table(
        "Simulated Inputs",
        1,
        &[
            "Input",
            "Distribution",
            "Parameters",
            "Minimum",
            "Maximum",
            "Source",
        ],
        plan.inputs
            .iter()
            .map(|i| {
                let mut params = i.params.clone();
                if let Some((name, values)) = &i.iterate {
                    let all: Vec<f64> = values.iter().flatten().copied().collect();
                    params
                        .iter_mut()
                        .filter(|p| &p.0 == name)
                        .for_each(|p| p.1 = all.clone());
                }
                let family = match (i.family.as_str(), i.trials) {
                    ("NEGBIN", true) => "NEGBIN (trials)".into(),
                    (f, _) => f.to_string(),
                };
                vec![
                    text(&i.field.name),
                    text(&family),
                    text(&params_text(&params)),
                    number(i.minval),
                    number(i.maxval),
                    text(if i.fitted {
                        "Fitted to the data"
                    } else {
                        "Given"
                    }),
                ]
            })
            .collect(),
    )];
    let fits: Vec<Vec<Cell>> = plan
        .inputs
        .iter()
        .flat_map(|i| {
            i.fit.iter().map(move |f| {
                vec![
                    Cell::Text(i.field.name.clone()),
                    Cell::Text(f.family.clone()),
                    Cell::Text(i.statistic.clone()),
                    Cell::Number(f.statistic),
                    f.p.map_or(Cell::Empty, Cell::Number),
                    Cell::Text(params_text(&f.params)),
                ]
            })
        })
        .collect();
    if !fits.is_empty() {
        tables.push(plain_table(
            "Goodness of Fit",
            2,
            &[
                "Input",
                "Distribution",
                "Test",
                "Statistic",
                "Sig.",
                "Parameters",
            ],
            fits,
        ));
    }
    if !plan.fixed.is_empty() {
        let rows = plan.fixed.iter().map(|f| {
            let values: Vec<String> = f.values.iter().map(|v| short(*v)).collect();
            vec![text(&f.field.name), text(&values.join(" "))]
        });
        tables.push(plain_table(
            "Fixed Inputs",
            1,
            &["Input", "Value"],
            rows.collect(),
        ));
    }
    if let Some(model) = &plan.model {
        let rows = plan.targets.iter().map(|t| {
            let expression = model
                .expressions
                .iter()
                .find(|e| e.0 == t.name)
                .map_or("", |e| e.1.as_str());
            vec![
                text(&t.name),
                text(expression),
                text(&t.level),
                text(t.comment.as_deref().unwrap_or("")),
            ]
        });
        tables.push(plain_table(
            "Targets",
            1,
            &["Target", "Expression", "Level", "Comment"],
            rows.collect(),
        ));
    }
    if !plan.correlations.is_empty() {
        let names: Vec<&str> = plan.inputs.iter().map(|i| i.field.name.as_str()).collect();
        let mut columns = vec![""];
        columns.extend(&names);
        let rows = plan.correlations.iter().zip(&names).map(|(row, n)| {
            let mut cells = vec![text(n)];
            cells.extend(row.iter().map(|v| Cell::Number(*v)));
            cells
        });
        tables.push(plain_table(
            "Correlations of Inputs",
            1,
            &columns,
            rows.collect(),
        ));
    }
    let rule = match &plan.stop {
        None => "none".to_string(),
        Some(s) if s.mean => format!(
            "{}: the {}% confidence interval of the mean is at most {}{} wide",
            s.target,
            short(s.level),
            short(s.thresh),
            if s.relative { "% of the mean" } else { "" }
        ),
        Some(s) => format!(
            "{}: {} cases {} {}{}",
            s.target,
            s.frequency,
            if s.left { "below" } else { "above" },
            if s.relative { "percentile " } else { "" },
            short(s.thresh)
        ),
    };
    tables.push(plain_table(
        "Stopping Criteria",
        1,
        &["Criterion", "Value"],
        vec![
            vec![text("Maximum cases"), Cell::Number(plan.max_cases as f64)],
            vec![text("Stop rule"), text(&rule)],
        ],
    ));
    tables
}

/// The weighted Pearson correlation; NaN without variation.
fn pearson(x: &[f64], y: &[f64], w: Option<&[f64]>) -> f64 {
    let weight = |i: usize| w.map_or(1.0, |w| w[i]);
    let total: f64 = (0..x.len()).map(weight).sum();
    let mx = (0..x.len()).map(|i| weight(i) * x[i]).sum::<f64>() / total;
    let my = (0..y.len()).map(|i| weight(i) * y[i]).sum::<f64>() / total;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for i in 0..x.len() {
        let (dx, dy) = (x[i] - mx, y[i] - my);
        sxy += weight(i) * dx * dy;
        sxx += weight(i) * dx * dx;
        syy += weight(i) * dy * dy;
    }
    sxy / (sxx * syy).sqrt()
}

/// The lower triangle L with L Lᵀ = r; None when r is not positive definite.
fn cholesky(r: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let k = r.len();
    let mut l = vec![vec![0.0; k]; k];
    for i in 0..k {
        for j in 0..=i {
            let s: f64 = r[i][j] - (0..j).map(|m| l[i][m] * l[j][m]).sum::<f64>();
            if i == j {
                if s <= 1e-12 {
                    return None;
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    Some(l)
}

/// One simulated input ready to draw: its distribution and the band of probabilities that
/// MINVAL and MAXVAL leave.
struct Drawn {
    dist: Dist,
    low: f64,
    high: f64,
}

impl Drawn {
    fn new(dist: Dist, minval: Option<f64>, maxval: Option<f64>) -> Drawn {
        let low = minval.map_or(0.0, |m| match dist.discrete() {
            true => dist.cdf(m.ceil() - 1.0),
            false => dist.cdf(m),
        });
        let high = maxval.map_or(1.0, |m| dist.cdf(m));
        Drawn { dist, low, high }
    }

    fn value(&self, z: f64) -> f64 {
        if let (Dist::Normal(m, s), 0.0, 1.0) = (&self.dist, self.low, self.high) {
            return m + s * z;
        }
        let u = self.low + crate::dist::normal_cdf(z) * (self.high - self.low);
        self.dist.quantile(u.clamp(1e-15, 1.0 - 1e-15))
    }
}

/// The correlation of standard normals that gives inputs `a` and `b` the correlation `rho`:
/// bisection on 1000 common draws, to 0.01.
fn normal_correlation(a: &Drawn, b: &Drawn, rho: f64, random: &mut crate::random::Random) -> f64 {
    if matches!((&a.dist, &b.dist), (Dist::Normal(..), Dist::Normal(..))) {
        return rho;
    }
    let z: Vec<f64> = (0..1000).map(|_| random.normal()).collect();
    let e: Vec<f64> = (0..1000).map(|_| random.normal()).collect();
    let x: Vec<f64> = z.iter().map(|z| a.value(*z)).collect();
    let reached = |c: f64| {
        let y: Vec<f64> = z
            .iter()
            .zip(&e)
            .map(|(z, e)| b.value(c * z + (1.0 - c * c).max(0.0).sqrt() * e))
            .collect();
        pearson(&x, &y, None)
    };
    let (mut low, mut high) = (-1.0, 1.0);
    if rho <= reached(low) {
        return low;
    }
    if rho >= reached(high) {
        return high;
    }
    loop {
        let mid = (low + high) / 2.0;
        let got = reached(mid);
        if (rho - got).abs() <= 0.01 || high - low <= 0.01 {
            return mid;
        }
        if rho > got {
            low = mid;
        } else {
            high = mid;
        }
    }
}

/// The summary statistics of a target or input, from its sorted values.
struct Summary {
    n: f64,
    mean: f64,
    sd: f64,
    sorted: Vec<f64>,
}

impl Summary {
    fn new(values: impl Iterator<Item = f64>) -> Summary {
        let mut sorted: Vec<f64> = values.collect();
        sorted.sort_by(f64::total_cmp);
        let n = sorted.len() as f64;
        let mean = sorted.iter().sum::<f64>() / n;
        let sd = (sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        Summary {
            n,
            mean,
            sd,
            sorted,
        }
    }

    /// The p-th percentile (0 to 100) by the weighted average at (n + 1) p.
    fn percentile(&self, p: f64) -> f64 {
        let s = &self.sorted;
        let at = (self.n + 1.0) * p / 100.0;
        match at.floor() as usize {
            _ if s.is_empty() => f64::NAN,
            0 => s[0],
            i if i >= s.len() => s[s.len() - 1],
            i => s[i - 1] + (at - i as f64) * (s[i] - s[i - 1]),
        }
    }
}

/// One simulation: the inputs by plan order (simulated, then fixed) and the targets.
struct Run {
    inputs: Vec<Vec<f64>>,
    targets: Vec<Vec<Option<f64>>>,
}

impl Engine {
    pub(crate) fn simrun(&mut self, c: &Simrun) -> Result<Vec<Table>, Error> {
        let plan = self.read_sim_plan(&c.plan)?;
        let commands = match &plan.model {
            Some(m) => m.compile()?.0,
            None => Vec::new(),
        };
        let seed = c.seed.unwrap_or_else(|| crate::random::time_seed() as f64);
        let mut random = crate::random::Random::default();
        random.mt_active = true;
        random.set_mtindex(seed);
        let iterations = plan
            .inputs
            .iter()
            .filter_map(|i| i.iterate.as_ref().map(|it| it.1.len()))
            .chain(plan.fixed.iter().map(|f| f.values.len()))
            .max()
            .unwrap_or(1);
        let names: Vec<String> = plan
            .inputs
            .iter()
            .map(|i| i.field.name.clone())
            .chain(plan.fixed.iter().map(|f| f.field.name.clone()))
            .collect();
        let mut runs = Vec::new();
        let mut tornado: Vec<(usize, usize, usize, [f64; 4])> = Vec::new();
        for it in 0..iterations {
            let drawn: Vec<Drawn> = plan
                .inputs
                .iter()
                .map(|i| {
                    let mut params = i.params.clone();
                    if let Some((name, values)) = &i.iterate {
                        params
                            .iter_mut()
                            .filter(|p| &p.0 == name)
                            .for_each(|p| p.1 = values[it].clone());
                    }
                    let dist = Dist::new(&i.family, &params, i.trials)
                        .map_err(|e| Error(format!("{}: {e}", i.field.name)))?;
                    Ok(Drawn::new(dist, i.minval, i.maxval))
                })
                .collect::<Result<_, Error>>()?;
            let fixed: Vec<f64> = plan
                .fixed
                .iter()
                .map(|f| f.values[it.min(f.values.len() - 1)])
                .collect();
            let k = drawn.len();
            let mut r = vec![vec![0.0; k]; k];
            for i in 0..k {
                r[i][i] = 1.0;
                for j in 0..i {
                    let rho = plan.correlations.get(i).map_or(0.0, |row| row[j]);
                    if rho != 0.0 {
                        let v = normal_correlation(&drawn[i], &drawn[j], rho, &mut random);
                        (r[i][j], r[j][i]) = (v, v);
                    }
                }
            }
            let l = cholesky(&r).ok_or_else(|| Error("the correlations of the normals behind the inputs are not positive definite; lower the correlations".into()))?;
            let mut draw = |n: usize| -> Vec<Vec<f64>> {
                let mut columns = vec![Vec::with_capacity(n); k];
                for _ in 0..n {
                    let e: Vec<f64> = (0..k).map(|_| random.normal()).collect();
                    for i in 0..k {
                        let z: f64 = (0..=i).map(|j| l[i][j] * e[j]).sum();
                        columns[i].push(drawn[i].value(z));
                    }
                }
                columns
            };
            let with_fixed = |mut columns: Vec<Vec<f64>>| {
                let n = columns.first().map_or(0, Vec::len);
                columns.extend(fixed.iter().map(|v| vec![*v; n]));
                columns
            };
            let stop = plan.stop.as_ref().and_then(|s| {
                plan.targets
                    .iter()
                    .position(|t| t.name.eq_ignore_ascii_case(&s.target))
                    .map(|t| (s, t))
            });
            let mut run = Run {
                inputs: vec![Vec::new(); names.len()],
                targets: vec![Vec::new(); plan.targets.len()],
            };
            let batch = if stop.is_some() { 1000 } else { plan.max_cases };
            while run.inputs.first().map_or(0, Vec::len) < plan.max_cases {
                let have = run.inputs.first().map_or(0, Vec::len);
                let columns = with_fixed(draw(batch.min(plan.max_cases - have)));
                let targets = evaluate(&commands, &names, &columns, &plan.targets)?;
                run.inputs
                    .iter_mut()
                    .zip(columns)
                    .for_each(|(a, b)| a.extend(b));
                run.targets
                    .iter_mut()
                    .zip(targets)
                    .for_each(|(a, b)| a.extend(b));
                if let Some((s, t)) = stop
                    && let Some(n) = stopped_at(s, &plan.targets[t], &run.targets[t], have)
                {
                    run.inputs.iter_mut().for_each(|c| c.truncate(n));
                    run.targets.iter_mut().for_each(|c| c.truncate(n));
                    break;
                }
            }
            // sensitivity measures on continuous targets and the inputs that allow them
            let n = run.inputs.first().map_or(0, Vec::len);
            let eligible = |i: usize, corr: bool| match &drawn[i].dist {
                Dist::Categorical(..) => false,
                Dist::Bernoulli(_) | Dist::Binom(..) | Dist::Poisson(_) | Dist::NegBin(..) => corr,
                _ => true,
            };
            let continuous: Vec<usize> = (0..plan.targets.len())
                .filter(|&t| plan.targets[t].level == "scale" && plan.targets[t].output)
                .collect();
            if !commands.is_empty() && n > 1 {
                let mut deltas: Vec<Vec<[f64; 2]>> =
                    vec![vec![[f64::NAN; 2]; k]; plan.targets.len()];
                if let Some(m) = c.tornado_delta {
                    for i in (0..k).filter(|&i| eligible(i, false)) {
                        let sd = Summary::new(run.inputs[i].iter().copied()).sd;
                        for (side, sign) in [-1.0, 1.0].into_iter().enumerate() {
                            let mut columns = run.inputs.clone();
                            columns[i].iter_mut().for_each(|v| *v += sign * m * sd);
                            let moved = evaluate(&commands, &names, &columns, &plan.targets)?;
                            for &t in &continuous {
                                deltas[t][i][side] =
                                    mean_change(&run.targets[t], &moved[t], &plan.targets[t]);
                            }
                        }
                    }
                }
                let mut shares = vec![vec![f64::NAN; k]; plan.targets.len()];
                if c.tornado_variance {
                    let second = draw(n);
                    for i in (0..k).filter(|&i| eligible(i, false)) {
                        let mut columns = run.inputs.clone();
                        columns[i] = second[i].clone();
                        let swapped = evaluate(&commands, &names, &columns, &plan.targets)?;
                        for &t in &continuous {
                            shares[t][i] =
                                sobol_total(&run.targets[t], &swapped[t], &plan.targets[t]);
                        }
                    }
                }
                for &t in &continuous {
                    for i in (0..k).filter(|&i| eligible(i, true) && plan.inputs[i].field.output) {
                        let pairs: Vec<(f64, f64)> = (0..n)
                            .filter_map(|c| {
                                kept(&plan.targets[t], run.targets[t][c])
                                    .map(|y| (y, run.inputs[i][c]))
                            })
                            .collect();
                        let (y, x): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
                        tornado.push((
                            it,
                            t,
                            i,
                            [
                                pearson(&x, &y, None),
                                deltas[t][i][0],
                                deltas[t][i][1],
                                shares[t][i],
                            ],
                        ));
                    }
                }
            }
            runs.push(run);
        }
        let mut tables = self.simrun_output(c, &plan, &names, &runs, &tornado);
        if let Some(file) = &c.outfile {
            let data = simulated_dataset(&plan, &names, &runs)?;
            if file == "*" {
                self.park_active();
                self.dataset = Some(data);
            } else {
                self.write_data_file(file, data)?;
            }
        }
        tables.retain(|t| !t.rows.is_empty());
        Ok(tables)
    }
}

/// The value of a target that its filter keeps.
fn kept(t: &Field, v: Option<f64>) -> Option<f64> {
    let v = v?;
    let above = t
        .min
        .is_none_or(|m| if t.min_inclusive { v >= m } else { v > m });
    let below = t
        .max
        .is_none_or(|m| if t.max_inclusive { v <= m } else { v < m });
    (above && below).then_some(v)
}

/// The mean change of a target between two evaluations, over the cases both keep.
fn mean_change(before: &[Option<f64>], after: &[Option<f64>], t: &Field) -> f64 {
    let pairs: Vec<f64> = before
        .iter()
        .zip(after)
        .filter_map(|(a, b)| Some(kept(t, *b)? - kept(t, *a)?))
        .collect();
    pairs.iter().sum::<f64>() / pairs.len() as f64
}

/// Sobol's total share of variance of an input: (D - D₋) / D with D₋ the mean product of the
/// target and the target with that input drawn again, less the squared mean.
fn sobol_total(y: &[Option<f64>], swapped: &[Option<f64>], t: &Field) -> f64 {
    let pairs: Vec<(f64, f64)> = y
        .iter()
        .zip(swapped)
        .filter_map(|(a, b)| Some((kept(t, *a)?, kept(t, *b)?)))
        .collect();
    let n = pairs.len() as f64;
    let f0 = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let d = pairs.iter().map(|p| p.0 * p.0).sum::<f64>() / n - f0 * f0;
    let rest = pairs.iter().map(|p| p.0 * p.1).sum::<f64>() / n - f0 * f0;
    (d - rest) / d
}

/// The number of cases at which the stopping rule first holds, checked every 100 cases from
/// `from`; None while it does not.
fn stopped_at(s: &Stop, t: &Field, values: &[Option<f64>], from: usize) -> Option<usize> {
    let mut checkpoint = from.div_ceil(100).max(1) * 100;
    while checkpoint <= values.len() {
        let kept: Vec<f64> = values[..checkpoint]
            .iter()
            .filter_map(|v| kept(t, *v))
            .collect();
        let n = kept.len() as f64;
        let holds = if s.mean {
            let summary = Summary::new(kept.iter().copied());
            let alpha = 1.0 - s.level / 100.0;
            let width =
                || 2.0 * crate::dist::t_quantile(alpha / 2.0, n - 1.0) * summary.sd / n.sqrt();
            let limit = if s.relative {
                s.thresh / 100.0 * summary.mean.abs()
            } else {
                s.thresh
            };
            n > 1.0 && width() <= limit
        } else if s.relative {
            // beyond a percentile lie that share of the cases
            let share = if s.thresh >= 50.0 {
                1.0 - s.thresh / 100.0
            } else {
                s.thresh / 100.0
            };
            (n * share).floor() >= s.frequency as f64
        } else {
            let beyond = kept.iter().filter(|v| {
                if s.left {
                    **v < s.thresh
                } else {
                    **v > s.thresh
                }
            });
            beyond.count() >= s.frequency
        };
        if holds {
            return Some(checkpoint);
        }
        checkpoint += 100;
    }
    None
}

/// The targets of the model for the given input columns, by running its COMPUTE commands on
/// a dataset of those columns.
fn evaluate(
    commands: &[Command],
    names: &[String],
    columns: &[Vec<f64>],
    targets: &[Field],
) -> Result<Vec<Vec<Option<f64>>>, Error> {
    if commands.is_empty() {
        return Ok(Vec::new());
    }
    let n = columns.first().map_or(0, Vec::len);
    let mut data = Dataset::new(n);
    for (name, values) in names.iter().zip(columns) {
        data.set_numeric_values(name, values.iter().map(|v| Some(*v)).collect())?;
    }
    let mut engine = Engine::with_dataset(Some(data));
    for command in commands {
        engine.execute(command)?;
    }
    engine.execute(&Command::Execute)?;
    let data = engine.dataset.as_ref().expect("a dataset");
    targets
        .iter()
        .map(|t| {
            let v = data.variable(&t.name)?;
            Ok(v.numeric()
                .map_err(|_| Error(format!("the target {} is not numeric", t.name)))?
                .iter()
                .collect())
        })
        .collect()
}

impl Engine {
    fn simrun_output(
        &mut self,
        c: &Simrun,
        plan: &Plan,
        names: &[String],
        runs: &[Run],
        tornado: &[(usize, usize, usize, [f64; 4])],
    ) -> Vec<Table> {
        let several = runs.len() > 1;
        let head = |name: &str, it: usize| {
            let mut cells = vec![Cell::Text(name.into())];
            if several {
                cells.push(Cell::Number((it + 1) as f64));
            }
            cells
        };
        let columns = |first: &str, rest: &[&str]| {
            let mut out = vec![first.to_string()];
            if several {
                out.push("Iteration".into());
            }
            out.extend(rest.iter().map(|s| s.to_string()));
            out
        };
        let headers = if several { 2 } else { 1 };
        let has_model = plan.model.is_some();
        let shown: Vec<usize> = (0..plan.targets.len())
            .filter(|&t| plan.targets[t].output)
            .collect();
        let scale = |t: &usize| plan.targets[*t].level == "scale";
        let summaries: Vec<Vec<Summary>> = runs
            .iter()
            .map(|r| {
                (0..plan.targets.len())
                    .map(|t| {
                        Summary::new(
                            r.targets[t]
                                .iter()
                                .filter_map(|v| kept(&plan.targets[t], *v)),
                        )
                    })
                    .collect()
            })
            .collect();
        let mut tables = Vec::new();
        if let Some(level) = c.descriptives.filter(|_| has_model) {
            let mut rows = Vec::new();
            for &t in shown.iter().filter(|t| scale(t)) {
                for (it, s) in summaries.iter().enumerate() {
                    let s = &s[t];
                    let half = crate::dist::t_quantile((1.0 - level / 100.0) / 2.0, s.n - 1.0)
                        * s.sd
                        / s.n.sqrt();
                    let mut row = head(&plan.targets[t].name, it);
                    row.extend(
                        [
                            s.n,
                            s.mean,
                            s.sd,
                            s.percentile(50.0),
                            s.sorted[0],
                            s.sorted[s.sorted.len() - 1],
                            s.mean - half,
                            s.mean + half,
                            s.percentile(5.0),
                            s.percentile(95.0),
                        ]
                        .map(Cell::Number),
                    );
                    rows.push(row);
                }
            }
            let (lower, upper) = (
                format!("{}% CI Lower", short(level)),
                format!("{}% CI Upper", short(level)),
            );
            let cols = columns(
                "Target",
                &[
                    "N",
                    "Mean",
                    "Std. Deviation",
                    "Median",
                    "Minimum",
                    "Maximum",
                    &lower,
                    &upper,
                    "5th Percentile",
                    "95th Percentile",
                ],
            );
            tables.push(plain_table("Target Statistics", headers, &cols, rows));
            let mut rows = Vec::new();
            for &t in shown.iter().filter(|t| !scale(t)) {
                for (it, s) in summaries.iter().enumerate() {
                    let mut values = s[t].sorted.clone();
                    values.dedup();
                    for v in values {
                        let count = s[t].sorted.iter().filter(|x| **x == v).count() as f64;
                        let label = plan.targets[t]
                            .labels
                            .iter()
                            .find(|l| l.0 == v)
                            .map_or_else(|| short(v), |l| l.1.clone());
                        let mut row = head(&plan.targets[t].name, it);
                        row.extend([Cell::Text(label), Cell::Number(100.0 * count / s[t].n)]);
                        rows.push(row);
                    }
                }
            }
            tables.push(plain_table(
                "Target Categories",
                headers + 1,
                &columns("Target", &["Category", "Percent"]),
                rows,
            ));
        }
        if c.descriptives.is_some() {
            let mut rows = Vec::new();
            for (i, input) in plan
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, i)| i.field.output && i.field.level == "scale")
            {
                for (it, run) in runs.iter().enumerate() {
                    let s = Summary::new(run.inputs[i].iter().copied());
                    let mut row = head(&input.field.name, it);
                    row.extend(
                        [s.mean, s.sd, s.sorted[0], s.sorted[s.sorted.len() - 1]].map(Cell::Number),
                    );
                    rows.push(row);
                }
            }
            tables.push(plain_table(
                "Input Statistics",
                headers,
                &columns("Input", &["Mean", "Std. Deviation", "Minimum", "Maximum"]),
                rows,
            ));
        }
        if c.percentiles && has_model {
            let mut points: Vec<f64> = c.ptiles.clone();
            if let Some(k) = c.ntiles {
                points.extend((1..k).map(|j| 100.0 * j as f64 / k as f64));
            }
            points.sort_by(f64::total_cmp);
            points.dedup();
            let labels: Vec<String> = points.iter().map(|p| short(*p)).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            let mut rows = Vec::new();
            for &t in shown.iter().filter(|t| scale(t)) {
                for (it, s) in summaries.iter().enumerate() {
                    let mut row = head(&plan.targets[t].name, it);
                    row.extend(points.iter().map(|p| Cell::Number(s[t].percentile(*p))));
                    rows.push(row);
                }
            }
            tables.push(plain_table(
                "Target Percentiles",
                headers,
                &columns("Target", &labels),
                rows,
            ));
        }
        if c.associations {
            let picked: Vec<usize> = (0..plan.inputs.len())
                .filter(|&i| plan.inputs[i].family != "CATEGORICAL" && plan.inputs[i].field.output)
                .collect();
            let input_names: Vec<&str> = picked
                .iter()
                .map(|&i| plan.inputs[i].field.name.as_str())
                .collect();
            let mut rows = Vec::new();
            for (it, run) in runs.iter().enumerate() {
                for &i in &picked {
                    let mut row = head(&plan.inputs[i].field.name, it);
                    row.extend(
                        picked
                            .iter()
                            .map(|&j| Cell::Number(pearson(&run.inputs[i], &run.inputs[j], None))),
                    );
                    rows.push(row);
                }
            }
            tables.push(plain_table(
                "Correlations of Simulated Inputs",
                headers,
                &columns("Input", &input_names),
                rows,
            ));
        }
        let pick = |m: usize| tornado.iter().filter(move |r| !r.3[m].is_nan());
        let order = |rows: &mut Vec<&(usize, usize, usize, [f64; 4])>,
                     key: fn(&[f64; 4]) -> f64| {
            rows.sort_by(|a, b| {
                (a.0, a.1)
                    .cmp(&(b.0, b.1))
                    .then(key(&b.3).total_cmp(&key(&a.3)))
            });
        };
        let tornado_table = |title: &str, m: usize, cols: &[&str], key: fn(&[f64; 4]) -> f64| {
            let mut rows: Vec<_> = pick(m).collect();
            order(&mut rows, key);
            let rows = rows
                .iter()
                .map(|(it, t, i, v)| {
                    let mut row = head(&plan.targets[*t].name, *it);
                    row.push(Cell::Text(plan.inputs[*i].field.name.clone()));
                    row.extend(
                        match m {
                            1 => vec![v[1], v[2]],
                            _ => vec![v[m]],
                        }
                        .into_iter()
                        .map(Cell::Number),
                    );
                    row
                })
                .collect();
            plain_table(title, headers + 1, &columns("Target", cols), rows)
        };
        if c.tornado_corr {
            tables.push(tornado_table(
                "Tornado: Correlation",
                0,
                &["Input", "Correlation"],
                |v| v[0].abs(),
            ));
        }
        if let Some(m) = c.tornado_delta {
            let (minus, plus) = (format!("-{} SD", short(m)), format!("+{} SD", short(m)));
            tables.push(tornado_table(
                "Tornado: Change in Target",
                1,
                &["Input", &minus, &plus],
                |v| v[1].abs().max(v[2].abs()),
            ));
        }
        if c.tornado_variance {
            tables.push(tornado_table(
                "Tornado: Contribution to Variance",
                3,
                &["Input", "Contribution"],
                |v| v[3],
            ));
        }
        if has_model {
            self.simrun_charts(c, plan, names, runs, &summaries, tornado);
        }
        tables
    }

    fn simrun_charts(
        &mut self,
        c: &Simrun,
        plan: &Plan,
        names: &[String],
        runs: &[Run],
        summaries: &[Vec<Summary>],
        tornado: &[(usize, usize, usize, [f64; 4])],
    ) {
        let suffix = |it: usize| {
            if runs.len() > 1 {
                format!(", iteration {}", it + 1)
            } else {
                String::new()
            }
        };
        let targets = (0..plan.targets.len())
            .filter(|&t| plan.targets[t].output && plan.targets[t].level == "scale");
        for t in targets {
            let name = &plan.targets[t].name;
            for (it, s) in summaries.iter().enumerate() {
                let s = &s[t];
                if s.sorted.is_empty() {
                    continue;
                }
                if c.pdf {
                    let pairs: Vec<(f64, f64)> = s.sorted.iter().map(|v| (*v, 1.0)).collect();
                    let title = format!("Distribution of {name}{}", suffix(it));
                    let h = crate::frequencies::Histogram {
                        title: &title,
                        heading: name,
                        format: "F8.2",
                        name,
                        normal: false,
                        scale: None,
                    };
                    if let Ok(chart) = h.chart(&pairs, &pairs, s.n) {
                        self.charts.push(chart);
                    }
                }
                let views = [
                    (c.cdf && c.cdf_ascending, false),
                    (c.cdf && c.cdf_descending, true),
                ];
                for (_, descending) in views.into_iter().filter(|v| v.0) {
                    let points = (0..=100)
                        .map(|p| Point {
                            x: s.percentile(p as f64),
                            y: if descending {
                                1.0 - p as f64 / 100.0
                            } else {
                                p as f64 / 100.0
                            },
                            z: None,
                            label: None,
                        })
                        .collect();
                    let title = format!("Cumulative Distribution of {name}{}", suffix(it));
                    let groups = vec![PointGroup {
                        name: name.clone(),
                        points,
                    }];
                    let y = if descending {
                        "Probability above"
                    } else {
                        "Cumulative probability"
                    };
                    self.charts.push(Chart::plot(
                        ChartKind::Scatter,
                        title,
                        Axis::titled(name.clone()),
                        Axis::titled(y),
                        Plot::Points {
                            groups,
                            lines: Vec::new(),
                            z_axis: None,
                        },
                    ));
                }
            }
            if c.boxplot {
                let boxes = summaries
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| !s[t].sorted.is_empty())
                    .map(|(it, s)| {
                        let s = &s[t];
                        let (q1, q3) = (s.percentile(25.0), s.percentile(75.0));
                        let reach = 1.5 * (q3 - q1);
                        let inside = s
                            .sorted
                            .iter()
                            .filter(|v| **v >= q1 - reach && **v <= q3 + reach);
                        let (low, high) = inside
                            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                                (a.min(*v), b.max(*v))
                            });
                        BoxSummary {
                            label: if runs.len() > 1 {
                                format!("Iteration {}", it + 1)
                            } else {
                                name.clone()
                            },
                            group: None,
                            n: s.n,
                            lower_whisker: low,
                            lower_hinge: q1,
                            median: s.percentile(50.0),
                            upper_hinge: q3,
                            upper_whisker: high,
                            outliers: Vec::new(),
                            extremes: Vec::new(),
                        }
                    })
                    .collect();
                self.charts.push(Chart::plot(
                    ChartKind::Bar,
                    format!("Boxplot of {name}"),
                    Axis::titled(""),
                    Axis::titled(name.clone()),
                    Plot::Boxes { boxes },
                ));
            }
            if c.scatter {
                for (i, input) in plan
                    .inputs
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| i.field.output)
                {
                    let groups = runs
                        .iter()
                        .enumerate()
                        .map(|(it, r)| PointGroup {
                            name: if runs.len() > 1 {
                                format!("Iteration {}", it + 1)
                            } else {
                                name.clone()
                            },
                            points: (0..r.targets[t].len().min(5000))
                                .filter_map(|c| {
                                    Some(Point {
                                        x: r.inputs[i][c],
                                        y: kept(&plan.targets[t], r.targets[t][c])?,
                                        z: None,
                                        label: None,
                                    })
                                })
                                .collect(),
                        })
                        .collect();
                    let title = format!("{name} by {}", input.field.name);
                    self.charts.push(Chart::plot(
                        ChartKind::Scatter,
                        title,
                        Axis::titled(input.field.name.clone()),
                        Axis::titled(name.clone()),
                        Plot::Points {
                            groups,
                            lines: Vec::new(),
                            z_axis: None,
                        },
                    ));
                }
            }
            for (m, title, statistic) in [
                (0, "Correlation", c.tornado_corr),
                (3, "Contribution to Variance", c.tornado_variance),
                (1, "Change in Target", c.tornado_delta.is_some()),
            ] {
                if !statistic {
                    continue;
                }
                for it in 0..runs.len() {
                    let mut rows: Vec<_> = tornado
                        .iter()
                        .filter(|r| r.0 == it && r.1 == t && !r.3[m].is_nan())
                        .collect();
                    let size = |v: &[f64; 4]| {
                        if m == 1 {
                            v[1].abs().max(v[2].abs())
                        } else {
                            v[m].abs()
                        }
                    };
                    rows.sort_by(|a, b| size(&b.3).total_cmp(&size(&a.3)));
                    if rows.is_empty() {
                        continue;
                    }
                    let categories = rows
                        .iter()
                        .map(|r| Category {
                            label: names[r.2].clone(),
                            value: Cell::Text(names[r.2].clone()),
                            missing: false,
                            bin: None,
                        })
                        .collect();
                    let series_of = |name: &str, j: usize| Series {
                        name: name.into(),
                        statistic: Statistic::Value,
                        variable: None,
                        value: Cell::Empty,
                        values: rows.iter().map(|r| r.3[j]).collect(),
                    };
                    let series = match m {
                        1 => vec![series_of("Minus", 1), series_of("Plus", 2)],
                        _ => vec![series_of(title, m)],
                    };
                    self.charts.push(Chart {
                        group_size: None,
                        kind: ChartKind::Bar,
                        tier: Tier::Presentation,
                        orientation: Some(Orientation::Horizontal),
                        grouping: None,
                        title: format!("Tornado: {title} for {name}{}", suffix(it)),
                        subtitle: None,
                        footnote: None,
                        category_axis: Axis::titled("Input"),
                        value_axis: Axis::titled(title),
                        legend: None,
                        categories,
                        series,
                        normal: None,
                        error_bars: None,
                        second_axis: None,
                        plot: None,
                        outline: None,
                        hidden: false,
                    });
                }
            }
        }
    }
}

/// Every simulated case: Iteration (with a sensitivity analysis), the targets and the inputs.
fn simulated_dataset(plan: &Plan, names: &[String], runs: &[Run]) -> Result<Dataset, Error> {
    let n: usize = runs
        .iter()
        .map(|r| r.inputs.first().map_or(0, Vec::len))
        .sum();
    let mut data = Dataset::new(n);
    if runs.len() > 1 {
        let values = runs.iter().enumerate().flat_map(|(it, r)| {
            std::iter::repeat_n(Some((it + 1) as f64), r.inputs.first().map_or(0, Vec::len))
        });
        data.set_numeric_values("Iteration", values.collect())?;
        data.set_format("Iteration", "F8.0")?;
    }
    let fields = plan
        .targets
        .iter()
        .chain(plan.inputs.iter().map(|i| &i.field))
        .chain(plan.fixed.iter().map(|f| &f.field));
    for (j, field) in fields.enumerate() {
        let values: Vec<Option<f64>> = match j.checked_sub(plan.targets.len()) {
            None => runs
                .iter()
                .flat_map(|r| r.targets[j].iter().copied())
                .collect(),
            Some(i) => runs
                .iter()
                .flat_map(|r| r.inputs[i].iter().map(|v| Some(*v)))
                .collect(),
        };
        let name = if j < plan.targets.len() {
            &field.name
        } else {
            &names[j - plan.targets.len()]
        };
        data.set_numeric_values(name, values)?;
        if let Some(format) = &field.format {
            data.set_format(name, &print_format(format))?;
        }
        data.set_measure(
            name,
            match field.level.as_str() {
                "nominal" => Measure::Nominal,
                "ordinal" => Measure::Ordinal,
                _ => Measure::Scale,
            },
        )?;
        if !field.labels.is_empty() {
            data.set_value_labels(
                name,
                field
                    .labels
                    .iter()
                    .map(|(v, l)| (Value::Number(*v), l.clone()))
                    .collect(),
            )?;
        }
    }
    Ok(data)
}
