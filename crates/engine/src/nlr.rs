//! NLR: nonlinear least squares by the Levenberg-Marquardt method, for the model that MODEL
//! PROGRAM (and DERIVATIVES) define.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (NLR):
//! - The transformations after MODEL PROGRAM, up to the next command of another kind, are the
//!   model program; DERIVATIVES starts the derivatives program inside it. NLR runs them on a copy
//!   of the data for each trial value of the parameters, which are variables of that copy; the
//!   active data never change. The variable PRED (or the one PRED names) holds the predicted value,
//!   D.parameter the derivative of it. A derivative left out is estimated numerically.
//! - F = sum W R^2 with R = y - f and the weight W, which the model program may recompute for each
//!   case; the weights are not case replicates, so the degrees of freedom count cases.
//! - The estimates stop when a step lowers F by a proportion of at most SSCON, when no parameter
//!   changes by more than the proportion PCON, when the correlation between every derivative and
//!   the weighted residuals is at most RCON, or after ITER iterations (default 100 per
//!   parameter). The manual writes the first test as 1 - F(k-1)/F(k) < SSCON, which a step that
//!   lowers F could never fail; it is read as the proportional reduction (F(k-1) - F(k)) / F(k-1).
//! - Statistics: A = F / (n - p) (J'WJ)^-1 with J the Jacobian of the model at the estimates; the
//!   standard errors are the square roots of its diagonal, the 95% intervals add and subtract
//!   t(0.975, n - p) standard errors (the manual's formula leaves the square root out), the
//!   correlations scale A. The analysis of variance has Regression (p df, SSuncorrected - F), Residual
//!   (n - p, F), Uncorrected Total (n, sum W y^2) and Corrected Total (n - 1, SSuncorrected minus the
//!   squared weighted mean times the sum of W). R squared is 1 - Residual / Corrected Total.
//! - The derivative check compares each derivative the program gives with a numerical one at the
//!   starting values and scores each pair from 0 to 1 by the rule of MINPACK's CHKDER (1 agrees to
//!   machine precision, above 0.5 probably right); the worst score of every parameter is shown
//!   when it is below 1, and a score below CKDER stops the run. The manual scores per derivative
//!   where CHKDER scores per function; here the parameters are perturbed one at a time.
//! - Where the manual is silent: the iteration history numbers the accepted iterations from 0
//!   (the starting values); a case is used when its dependent variable, its weight and its predicted
//!   value at the starting values are all valid; a model program stays until an NLR takes it up.

use crate::descriptives::case_weights;
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::wls::numeric_column;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{Command, ModelParameter, Nlr, NlrSave};

/// Which part of the program the transformations after MODEL PROGRAM go to.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Section {
    Model,
    Derivatives,
    Constraints,
    Closed,
}

/// The programs of the last MODEL PROGRAM.
#[derive(Clone, Debug)]
pub(crate) struct ModelProgram {
    pub parameters: Vec<ModelParameter>,
    pub model: Vec<Command>,
    pub derivatives: Vec<Command>,
    /// The constrained functions program (CNLR).
    pub constraints: Vec<Command>,
    pub section: Section,
}

impl ModelProgram {
    pub(crate) fn new(parameters: Vec<ModelParameter>) -> Self {
        Self {
            parameters,
            model: Vec::new(),
            derivatives: Vec::new(),
            constraints: Vec::new(),
            section: Section::Model,
        }
    }
}

/// What the programs gave for one trial value of the parameters, case by case (NaN where
/// missing).
pub(super) struct Eval {
    pub(super) pred: Vec<f64>,
    pub(super) weight: Vec<f64>,
    /// The derivative of every parameter the derivatives program computed.
    pub(super) derivatives: Vec<Option<Vec<f64>>>,
    /// The format and the label the program gave the predicted values.
    pub(super) format: String,
    pub(super) label: Option<String>,
    /// The loss of every case (CNLR with LOSS).
    pub(super) loss: Option<Vec<f64>>,
    /// The value of every constrained function (CNLR): per case.
    pub(super) constraints: Vec<Vec<f64>>,
}

/// Runs the programs on a copy of the data.
pub(super) struct Runner {
    pub(super) engine: Engine,
    pub(super) base: Dataset,
    pub(super) parameters: Vec<String>,
    pub(super) model: Vec<Command>,
    pub(super) derivatives: Vec<Command>,
    pub(super) pred: String,
    /// The variable of the model program that holds the loss of a case (CNLR).
    pub(super) loss: Option<String>,
    /// The commands that compute the constrained functions and the variables they leave
    /// (CNLR).
    pub(super) constraint_program: Vec<Command>,
    pub(super) constraint_names: Vec<String>,
    pub(super) runs: usize,
    pub(super) derivative_runs: usize,
}

fn derivative_name(parameter: &str) -> String {
    format!("D.{parameter}")
}

impl Runner {
    pub(super) fn run(&mut self, theta: &[f64], derivatives: bool) -> Result<Eval, Error> {
        self.runs += 1;
        let mut data = self.base.clone();
        let cases = data.case_count();
        for (name, value) in self.parameters.iter().zip(theta) {
            data.set_numeric_values(name, vec![Some(*value); cases])?;
        }
        let mut sub = self.engine.clone();
        sub.dataset = Some(data);
        sub.pending = None;
        let fault = |e: Error| Error(format!("NLR, in the model program: {}", e.0));
        let commands = self
            .model
            .iter()
            .chain(if derivatives {
                self.derivatives.iter()
            } else {
                [].iter()
            })
            .chain(self.constraint_program.iter());
        for c in commands {
            sub.execute(c).map_err(fault)?;
        }
        if sub.pending.is_some() {
            return Err(Error(
                "NLR: a case-by-case pass (LAG, $CASENUM, a scratch variable kept from case to case) is not supported in the model program"
                    .into(),
            ));
        }
        if sub.collecting.is_some() || !sub.blocks.is_empty() {
            return Err(Error(
                "NLR: a LOOP or DO IF in the model program is not closed".into(),
            ));
        }
        let data = sub.dataset.take().expect("the data of the run");
        if data.case_count() != cases {
            return Err(Error(
                "NLR: the model program cannot add or drop cases".into(),
            ));
        }
        if data.variable_index(&self.pred).is_none() {
            return Err(Error(format!(
                "NLR: the model program does not compute {}, the variable of the predicted values (name it with PRED)",
                self.pred
            )));
        }
        let pred = numeric_column(&data, &self.pred, true, "NLR")?;
        let weight = match case_weights(&data)? {
            Some(w) => w,
            None => vec![1.0; cases],
        };
        let mut found = Vec::new();
        for name in &self.parameters {
            let d = derivative_name(name);
            found.push(if derivatives && data.variable_index(&d).is_some() {
                self.derivative_runs += 1;
                Some(numeric_column(&data, &d, true, "NLR")?)
            } else {
                None
            });
        }
        let loss = match &self.loss {
            Some(name) => {
                if data.variable_index(name).is_none() {
                    return Err(Error(format!(
                        "CNLR: the model program does not compute {name}, the loss variable"
                    )));
                }
                Some(numeric_column(&data, name, true, "CNLR")?)
            }
            None => None,
        };
        let mut constraints = Vec::new();
        for name in &self.constraint_names {
            if data.variable_index(name).is_none() {
                return Err(Error(format!(
                    "CNLR: the constrained functions program does not compute {name}"
                )));
            }
            constraints.push(numeric_column(&data, name, true, "CNLR")?);
        }
        let variable = data.variable(&self.pred)?;
        Ok(Eval {
            pred,
            weight,
            derivatives: found,
            format: variable.format().to_string(),
            label: variable.label().map(str::to_string),
            loss,
            constraints,
        })
    }
}

/// The weighted sum of squares over the cases `used`; None when a predicted value is missing.
fn sum_of_squares(y: &[f64], e: &Eval, used: &[usize]) -> Option<f64> {
    let mut sum = 0.0;
    for &i in used {
        let f = e.pred[i];
        if !f.is_finite() || !e.weight[i].is_finite() {
            return None;
        }
        let r = y[i] - f;
        sum += e.weight[i].max(0.0) * r * r;
    }
    sum.is_finite().then_some(sum)
}

/// The Jacobian of the weighted residuals sqrt(W) R over the used cases (rows) by the
/// parameters (columns), with the weighted residuals.
pub(super) struct Jacobian {
    pub(super) j: Matrix,
    pub(super) r: Vec<f64>,
}

const CENTRAL_STEP: f64 = 6.055e-6;

fn step_of(x: f64) -> f64 {
    CENTRAL_STEP * if x == 0.0 { 1.0 } else { x.abs() }
}

fn jacobian(
    runner: &mut Runner,
    theta: &[f64],
    at: &Eval,
    y: &[f64],
    used: &[usize],
    weights_vary: bool,
) -> Result<Jacobian, Error> {
    let p = theta.len();
    let n = used.len();
    let mut j = vec![vec![0.0; p]; n];
    let r: Vec<f64> = used
        .iter()
        .map(|&i| at.weight[i].max(0.0).sqrt() * (y[i] - at.pred[i]))
        .collect();
    for k in 0..p {
        // the derivative of the predicted value and, where the program changes the weight, of
        // the weight, by central differences unless the program gives the derivative
        let given = at.derivatives[k].as_ref();
        let mut around: Option<(Eval, Eval)> = None;
        if given.is_none() || weights_vary {
            let h = step_of(theta[k]);
            let (mut up, mut down) = (theta.to_vec(), theta.to_vec());
            up[k] += h;
            down[k] -= h;
            around = Some((runner.run(&up, false)?, runner.run(&down, false)?));
        }
        for (row, &i) in used.iter().enumerate() {
            let sw = at.weight[i].max(0.0).sqrt();
            let df = match (given, &around) {
                (Some(d), _) => d[i],
                (None, Some((up, down))) => (up.pred[i] - down.pred[i]) / (2.0 * step_of(theta[k])),
                (None, None) => unreachable!("one of the two is there"),
            };
            let dw = match (&around, weights_vary) {
                (Some((up, down)), true) => {
                    (up.weight[i] - down.weight[i]) / (2.0 * step_of(theta[k]))
                }
                _ => 0.0,
            };
            let w = at.weight[i];
            let weight_part = if w > 0.0 && dw != 0.0 {
                (y[i] - at.pred[i]) / (2.0 * sw) * dw
            } else {
                0.0
            };
            j[row][k] = -sw * df + weight_part;
            if !j[row][k].is_finite() {
                return Err(Error(format!(
                    "NLR: the derivative of the model with respect to {} is missing for a case",
                    runner.parameters[k]
                )));
            }
        }
    }
    Ok(Jacobian { j, r })
}

fn cross_products(j: &Matrix, p: usize) -> Matrix {
    let mut a = vec![vec![0.0; p]; p];
    for row in j {
        for (u, au) in a.iter_mut().enumerate() {
            for (v, auv) in au.iter_mut().enumerate() {
                *auv += row[u] * row[v];
            }
        }
    }
    a
}

fn gradient(jac: &Jacobian, p: usize) -> Vec<f64> {
    (0..p)
        .map(|k| jac.j.iter().zip(&jac.r).map(|(row, r)| row[k] * r).sum())
        .collect()
}

/// The largest correlation (cosine) between a derivative column and the weighted residuals.
fn largest_correlation(jac: &Jacobian, p: usize) -> f64 {
    let rn: f64 = jac.r.iter().map(|r| r * r).sum::<f64>().sqrt();
    if rn == 0.0 {
        return 0.0;
    }
    (0..p)
        .map(|k| {
            let cn: f64 = jac.j.iter().map(|row| row[k] * row[k]).sum::<f64>().sqrt();
            if cn == 0.0 {
                0.0
            } else {
                let dot: f64 = jac.j.iter().zip(&jac.r).map(|(row, r)| row[k] * r).sum();
                (dot / (cn * rn)).abs()
            }
        })
        .fold(0.0, f64::max)
}

/// The score from 0 to 1 of a derivative against the forward difference, case by case, by the
/// rule of MINPACK's CHKDER; the worst of the cases.
fn derivative_score(f: f64, forward: f64, derivative: f64, x_scale: f64) -> f64 {
    let epsmch = f64::EPSILON;
    let eps = epsmch.sqrt();
    let epsf = 100.0 * epsmch;
    let epslog = eps.log10();
    let predicted = derivative * x_scale;
    let mut temp = 1.0;
    if f != 0.0 && forward != 0.0 && (forward - f).abs() >= epsf * f.abs() {
        // the parameter moved by eps times its scale, so the change over eps is the derivative
        // times that scale
        temp = eps * ((forward - f) / eps - predicted).abs() / (f.abs() + forward.abs());
    }
    if temp >= eps {
        0.0
    } else if temp > epsmch {
        (temp.log10() - epslog) / epslog
    } else {
        1.0
    }
}

/// What NLR and CNLR give `prepare`.
pub(super) struct Setup<'a> {
    pub dependent: &'a str,
    pub pred: &'a Option<String>,
    pub file: &'a Option<String>,
    pub outfile: bool,
    pub ckder: Option<f64>,
    pub loss: Option<&'a str>,
    pub constraint_names: &'a [String],
}

/// The programs run once at the starting values.
pub(super) struct Prepared {
    pub runner: Runner,
    pub y: Vec<f64>,
    pub used: Vec<usize>,
    pub names: Vec<String>,
    pub theta: Vec<f64>,
    pub start: Eval,
    pub pred_name: String,
    pub weights_vary: bool,
    pub n: usize,
}

/// What `statistics_tables` reports.
pub(super) struct Fit<'a> {
    pub who: &'a str,
    pub dependent: &'a str,
    pub names: &'a [String],
    pub theta: &'a [f64],
    /// The residual sum of squares.
    pub value: f64,
    pub y: &'a [f64],
    pub used: &'a [usize],
    pub current: &'a Eval,
    pub jac: &'a Jacobian,
    pub history: &'a [(usize, f64, Vec<f64>)],
    pub iterations: usize,
    pub runs: usize,
    pub stop: &'a str,
}

/// How the iterations ended.
#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Sscon,
    Pcon,
    Rcon,
    Iterations,
    Perfect,
    NoStep,
}

fn stop_text(stop: Stop) -> &'static str {
    match stop {
        Stop::Sscon => {
            "the residual sum of squares changed by a proportion of at most SSCON between iterations"
        }
        Stop::Pcon => "no parameter changed by more than the proportion PCON",
        Stop::Rcon => "the correlation of every derivative with the residuals is at most RCON",
        Stop::Iterations => "the maximum number of iterations was reached",
        Stop::Perfect => "the model fits the data exactly",
        Stop::NoStep => "no step that lowers the residual sum of squares could be found",
    }
}

impl Engine {
    /// The start of MODEL PROGRAM: the parameters, and the transformations after it collect.
    pub(crate) fn begin_model_program(&mut self, parameters: &[ModelParameter]) {
        self.model_program = Some(ModelProgram::new(parameters.to_vec()));
    }

    /// DERIVATIVES: the transformations after it are the derivatives program.
    pub(crate) fn begin_derivatives(&mut self) -> Result<(), Error> {
        match self.model_program.as_mut() {
            Some(program) => {
                program.section = Section::Derivatives;
                Ok(())
            }
            None => Err(Error("DERIVATIVES needs a MODEL PROGRAM before it".into())),
        }
    }

    /// The starting values: MODEL PROGRAM's, else the case of the FILE for the split group.
    fn starting_values(
        &self,
        file: &Option<String>,
        parameters: &[ModelParameter],
    ) -> Result<Vec<f64>, Error> {
        let file = match file {
            Some(f) if parameters.iter().any(|q| q.value.is_none()) => Some(self.data_file(f)?),
            _ => None,
        };
        let case = self.group_number.max(1) - 1;
        parameters
            .iter()
            .map(|q| {
                if let Some(v) = q.value {
                    return Ok(v);
                }
                let Some(data) = &file else {
                    return Err(Error(format!(
                        "NLR: the parameter {} has no starting value: give one in MODEL PROGRAM or read it with FILE",
                        q.name
                    )));
                };
                let missing = || {
                    Error(format!(
                        "NLR /FILE: the file has no value for the parameter {}",
                        q.name
                    ))
                };
                let name = data
                    .variable_names()
                    .find(|n| n.eq_ignore_ascii_case(&q.name))
                    .ok_or_else(missing)?
                    .to_string();
                let values = numeric_column(data, &name, false, "NLR")?;
                let at = case.min(values.len().saturating_sub(1));
                values
                    .get(at)
                    .copied()
                    .filter(|v| v.is_finite())
                    .ok_or_else(missing)
            })
            .collect()
    }

    /// The work both NLR and CNLR start with: the programs, the data they run on, the
    /// starting values, the cases used and the check of the derivatives.
    pub(super) fn prepare(&mut self, who: &str, setup: &Setup) -> Result<Prepared, Error> {
        let Some(program) = self.model_program.clone() else {
            return Err(Error(format!("{who} needs a MODEL PROGRAM before it")));
        };
        if program.model.is_empty() {
            return Err(Error(format!(
                "{who}: the model program is empty: compute the predicted value after MODEL PROGRAM"
            )));
        }
        if program.model.iter().chain(&program.derivatives).any(|c| {
            matches!(
                c,
                Command::Print(_) | Command::PrintSpace(_) | Command::Xsave(_)
            )
        }) {
            return Err(Error(format!(
                "{who}: PRINT, WRITE, PRINT SPACE and XSAVE would act on every case at every trial; they cannot be in the model program"
            )));
        }
        let names: Vec<String> = program.parameters.iter().map(|q| q.name.clone()).collect();
        if setup.outfile {
            for reserved in ["SSE", "NCASES"] {
                if names.iter().any(|n| n.eq_ignore_ascii_case(reserved)) {
                    return Err(Error(format!(
                        "{who} /OUTFILE: a parameter cannot be named {reserved}"
                    )));
                }
            }
        }
        let theta = self.starting_values(setup.file, &program.parameters)?;
        let p = theta.len();
        let data = self.active()?;
        for n in &names {
            if data.variable_index(n).is_some() {
                return Err(Error(format!(
                    "{who}: the parameter {n} has the name of a variable of the data"
                )));
            }
        }
        let y = numeric_column(data, setup.dependent, false, who)?;
        let pred_name = setup.pred.clone().unwrap_or_else(|| "PRED".to_string());
        // only the variables the programs read, the dependent variable and the weight go along
        let mut base = data.clone();
        let all: Vec<Command> = program
            .model
            .iter()
            .chain(&program.derivatives)
            .chain(&program.constraints)
            .cloned()
            .collect();
        let refs = crate::rows::Refs::of(&all);
        if !refs.all && refs.ranges.is_empty() {
            let weight = base.weight_variable().map(str::to_string);
            let wanted: Vec<String> = (0..base.variable_count())
                .map(|i| base.variable_at(i).name().to_string())
                .filter(|n| {
                    refs.names.iter().any(|w| w.eq_ignore_ascii_case(n))
                        || n.eq_ignore_ascii_case(setup.dependent)
                        || weight.as_deref().is_some_and(|w| w.eq_ignore_ascii_case(n))
                })
                .collect();
            if !wanted.is_empty() {
                let keep: Vec<&str> = wanted.iter().map(String::as_str).collect();
                base.keep_variables(&keep)?;
            }
        }
        let weights_vary = base.weight_variable().is_some_and(|w| {
            crate::coxreg::program_targets(&program.model)
                .iter()
                .any(|t| t.eq_ignore_ascii_case(w))
        });
        // the template every trial is cloned from carries neither the data nor the other datasets
        let mut engine = self.clone();
        engine.model_program = None;
        engine.dataset = None;
        engine.session.clear_datasets();
        engine.time_program = None;
        engine.running_program = None;
        engine.case_numbers = None;
        engine.temporary = None;
        let mut runner = Runner {
            engine,
            base,
            parameters: names.clone(),
            model: program.model.clone(),
            derivatives: program.derivatives.clone(),
            pred: pred_name.clone(),
            loss: setup.loss.map(str::to_string),
            constraint_program: program.constraints.clone(),
            constraint_names: setup.constraint_names.to_vec(),
            runs: 0,
            derivative_runs: 0,
        };
        let start = runner.run(&theta, true)?;
        let cases = y.len();
        let mut skipped_weights = 0;
        let used: Vec<usize> = (0..cases)
            .filter(|&i| {
                let ok_y = y[i].is_finite();
                if ok_y && start.weight[i].is_finite() && start.weight[i] <= 0.0 {
                    skipped_weights += 1;
                }
                ok_y && start.weight[i].is_finite()
                    && start.weight[i] > 0.0
                    && start.pred[i].is_finite()
                    && start.loss.as_ref().is_none_or(|l| l[i].is_finite())
            })
            .collect();
        if skipped_weights > 0 {
            self.warn(format!(
                "{who} leaves out {skipped_weights} case(s) with a zero or negative weight"
            ));
        }
        let n = used.len();
        if n <= p {
            return Err(Error(format!(
                "{who} needs more valid cases than parameters: {n} cases for {p} parameters"
            )));
        }
        // the check of the derivatives the program gives
        let critical = setup.ckder.unwrap_or(0.5);
        for (k, given) in start.derivatives.iter().enumerate() {
            let Some(d) = given else { continue };
            let x = theta[k];
            let scale = if x == 0.0 { 1.0 } else { x.abs() };
            let h = f64::EPSILON.sqrt() * scale;
            let mut up = theta.clone();
            up[k] += h;
            let forward = runner.run(&up, false)?;
            // the derivatives are of the loss when there is one, else of the predicted value
            let (f0, f1) = match (&start.loss, &forward.loss) {
                (Some(a), Some(b)) => (a, b),
                _ => (&start.pred, &forward.pred),
            };
            let worst = used
                .iter()
                .map(|&i| derivative_score(f0[i], f1[i], d[i], scale))
                .fold(1.0, f64::min);
            if worst < 1.0 {
                self.warn(format!(
                    "{who} derivative check: the worst agreement score of {} is {:.3}",
                    derivative_name(&names[k]),
                    worst
                ));
            }
            if critical > 0.0 && worst < critical {
                return Err(Error(format!(
                    "{who}: the derivative {} disagrees with the numerical derivative (agreement score {:.3}, below CKDER {critical})",
                    derivative_name(&names[k]),
                    worst
                )));
            }
        }
        Ok(Prepared {
            runner,
            y,
            used,
            names,
            theta,
            start,
            pred_name,
            weights_vary,
            n,
        })
    }

    pub(crate) fn nlr(&mut self, command: &Nlr) -> Result<Vec<Table>, Error> {
        if self
            .model_program
            .as_ref()
            .is_some_and(|p| !p.constraints.is_empty())
        {
            return Err(Error(
                "NLR: CONSTRAINED FUNCTIONS belongs to CNLR; give CLEAR MODEL PROGRAMS or use CNLR"
                    .into(),
            ));
        }
        let setup = Setup {
            dependent: &command.dependent,
            pred: &command.pred,
            file: &command.file,
            outfile: command.outfile.is_some(),
            ckder: command.criteria.ckder,
            loss: None,
            constraint_names: &[],
        };
        let Prepared {
            mut runner,
            y,
            used,
            names,
            mut theta,
            start,
            pred_name,
            weights_vary,
            n,
        } = self.prepare("NLR", &setup)?;
        let p = theta.len();
        let cases = y.len();
        let criteria = &command.criteria;
        let max_iterations = criteria.iterations.map_or(100 * p, |i| i as usize);
        let sscon = criteria.sscon.unwrap_or(1e-8);
        let pcon = criteria.pcon.unwrap_or(1e-8);
        let rcon = criteria.rcon.unwrap_or(1e-8);

        let mut current = start;
        let mut ss = sum_of_squares(&y, &current, &used).ok_or_else(|| {
            Error("NLR: the model cannot be computed at the starting values".into())
        })?;
        let mut history: Vec<(usize, f64, Vec<f64>)> = vec![(0, ss, theta.clone())];
        let mut iteration = 0;
        let mut alpha = 1e-3;
        let stop;
        let mut jac = jacobian(&mut runner, &theta, &current, &y, &used, weights_vary)?;
        loop {
            if ss == 0.0 {
                stop = Stop::Perfect;
                break;
            }
            if rcon > 0.0 && largest_correlation(&jac, p) <= rcon {
                stop = Stop::Rcon;
                break;
            }
            if iteration >= max_iterations {
                stop = Stop::Iterations;
                break;
            }
            let jtj = cross_products(&jac.j, p);
            let g = gradient(&jac, p);
            let diagonal = (0..p).map(|k| jtj[k][k]).fold(0.0, f64::max);
            let mut accepted = None;
            while alpha <= 1e16 {
                let mut damped = jtj.clone();
                for (k, row) in damped.iter_mut().enumerate() {
                    // Marquardt's scaling by the diagonal, kept off zero
                    row[k] += alpha * jtj[k][k].max(1e-12 * diagonal).max(1e-300);
                }
                let Some((inv, _)) = inverse(&damped) else {
                    alpha *= 10.0;
                    continue;
                };
                let h: Vec<f64> = (0..p)
                    .map(|a| -(0..p).map(|b| inv[a][b] * g[b]).sum::<f64>())
                    .collect();
                let trial: Vec<f64> = theta.iter().zip(&h).map(|(t, s)| t + s).collect();
                if let Ok(e) = runner.run(&trial, true)
                    && let Some(new_ss) = sum_of_squares(&y, &e, &used)
                    && new_ss < ss
                {
                    accepted = Some((trial, h, e, new_ss));
                    break;
                }
                alpha *= 10.0;
            }
            let Some((trial, h, e, new_ss)) = accepted else {
                stop = Stop::NoStep;
                break;
            };
            alpha = (alpha / 10.0).max(1e-12);
            iteration += 1;
            let reduction = (ss - new_ss) / ss;
            theta = trial;
            current = e;
            ss = new_ss;
            history.push((iteration, ss, theta.clone()));
            jac = jacobian(&mut runner, &theta, &current, &y, &used, weights_vary)?;
            if sscon > 0.0 && reduction <= sscon {
                stop = Stop::Sscon;
                break;
            }
            if pcon > 0.0 && h.iter().zip(&theta).all(|(s, t)| s.abs() <= pcon * t.abs()) {
                stop = Stop::Pcon;
                break;
            }
        }
        if stop == Stop::Iterations || stop == Stop::NoStep {
            self.warn(format!(
                "NLR stopped because {}; the estimates may not be the least squares ones",
                stop_text(stop)
            ));
        }

        let tables = self.statistics_tables(&Fit {
            who: "NLR",
            dependent: &command.dependent,
            names: &names,
            theta: &theta,
            value: ss,
            y: &y,
            used: &used,
            current: &current,
            jac: &jac,
            history: &history,
            iterations: iteration,
            runs: runner.runs,
            stop: stop_text(stop),
        });
        if command.outfile.is_some() {
            self.outfile_cases(&names, &[(theta.clone(), ss, n as f64)])?;
        }
        self.save_variables(
            "NLR",
            &command.save,
            &mut runner,
            &pred_name,
            &theta,
            &y,
            &current,
        )?;
        Ok(tables)
    }

    /// The tables of a least-squares fit: the iteration history, the analysis of variance, the
    /// estimates with asymptotic standard errors and the correlations.
    pub(super) fn statistics_tables(&mut self, f: &Fit) -> Vec<Table> {
        let Fit {
            who,
            dependent,
            names,
            theta,
            value: ss,
            y,
            used,
            current,
            jac,
            history,
            iterations: iteration,
            runs,
            stop,
        } = f;
        let (ss, iteration, runs) = (*ss, *iteration, *runs);
        let n = used.len();
        let p = theta.len();
        // the statistics at the estimates
        let df_res = (n - p) as f64;
        let sum_w: f64 = used.iter().map(|&i| current.weight[i]).sum();
        let ss_unc: f64 = used.iter().map(|&i| current.weight[i] * y[i] * y[i]).sum();
        let y_bar = used.iter().map(|&i| current.weight[i] * y[i]).sum::<f64>() / sum_w;
        let ss_corrected = ss_unc - y_bar * y_bar * sum_w;
        let jtj = cross_products(&jac.j, p);
        let covariance: Option<Matrix> = inverse(&jtj).map(|(inv, _)| {
            let s2 = ss / df_res;
            inv.iter()
                .map(|r| r.iter().map(|v| v * s2).collect())
                .collect()
        });
        if covariance.is_none() {
            self.warn(format!("{who}: the parameters cannot be told apart at the estimates (singular matrix), so no standard errors are shown"));
        }
        let se: Vec<Option<f64>> = (0..p)
            .map(|k| {
                covariance
                    .as_ref()
                    .map(|c| c[k][k])
                    .filter(|v| *v >= 0.0)
                    .map(f64::sqrt)
            })
            .collect();
        let t_crit = crate::dist::t_quantile(0.025, df_res);
        let mut tables = Vec::new();

        // iteration history
        let mut columns = plain(&["Iteration", "Residual Sum of Squares"]);
        columns.extend(names.iter().map(|n| ("Parameters".to_string(), n.clone())));
        let rows = history
            .iter()
            .map(|(k, s, t)| {
                let mut row = vec![Cell::Number(*k as f64), Cell::Number(*s)];
                row.extend(t.iter().map(|v| Cell::Number(*v)));
                row
            })
            .collect();
        tables.push(table(
            "Iteration History",
            1,
            columns,
            rows,
            Some(format!(
                "Dependent variable: {}. Run stopped after {} iterations ({} model evaluations): {}.",
                dependent,
                iteration,
                runs,
                stop
            )),
        ));

        // analysis of variance
        let ss_regression = ss_unc - ss;
        let number = |v: f64| {
            if v.is_finite() {
                Cell::Number(v)
            } else {
                Cell::Empty
            }
        };
        let rows = vec![
            vec![
                text("Regression"),
                Cell::Number(p as f64),
                number(ss_regression),
                number(ss_regression / p as f64),
            ],
            vec![
                text("Residual"),
                Cell::Number(df_res),
                number(ss),
                number(ss / df_res),
            ],
            vec![
                text("Uncorrected Total"),
                Cell::Number(n as f64),
                number(ss_unc),
                Cell::Empty,
            ],
            vec![
                text("Corrected Total"),
                Cell::Number(n as f64 - 1.0),
                number(ss_corrected),
                Cell::Empty,
            ],
        ];
        let r_square = 1.0 - ss / ss_corrected;
        tables.push(table(
            "Analysis of Variance",
            1,
            plain(&["Source", "df", "Sum of Squares", "Mean Square"]),
            rows,
            Some(if r_square.is_finite() {
                format!(
                    "Dependent variable: {}. R squared = 1 - Residual SS / Corrected SS = {:.3}.",
                    dependent, r_square
                )
            } else {
                format!("Dependent variable: {}.", dependent)
            }),
        ));

        // parameter estimates
        let mut columns = plain(&["Parameter", "Estimate", "Std. Error"]);
        columns.push(("95% Confidence Interval".into(), "Lower Bound".into()));
        columns.push(("95% Confidence Interval".into(), "Upper Bound".into()));
        let rows = (0..p)
            .map(|k| {
                let bound = |sign: f64| match se[k] {
                    Some(s) => number(theta[k] + sign * t_crit * s),
                    None => Cell::Empty,
                };
                vec![
                    text(&names[k]),
                    Cell::Number(theta[k]),
                    se[k].map_or(Cell::Empty, Cell::Number),
                    bound(-1.0),
                    bound(1.0),
                ]
            })
            .collect();
        tables.push(table(
            "Parameter Estimates",
            1,
            columns,
            rows,
            Some("Asymptotic standard errors and confidence intervals.".into()),
        ));
        if let Some(c) = &covariance {
            let rows = (0..p)
                .map(|a| {
                    let mut row = vec![text(&names[a])];
                    row.extend((0..p).map(|b| {
                        let d = (c[a][a] * c[b][b]).sqrt();
                        number(c[a][b] / d)
                    }));
                    row
                })
                .collect();
            let mut columns = vec![(String::new(), String::new())];
            columns.extend(names.iter().map(|n| (String::new(), n.clone())));
            tables.push(table(
                "Asymptotic Correlation Matrix of the Parameter Estimates",
                1,
                columns,
                rows,
                None,
            ));
        }

        tables
    }

    /// The case of the parameters, SSE and NCASES that goes to OUTFILE (with the split values).
    pub(super) fn outfile_cases(
        &mut self,
        names: &[String],
        rows: &[(Vec<f64>, f64, f64)],
    ) -> Result<(), Error> {
        let mut part = Dataset::new(rows.len());
        for (k, name) in names.iter().enumerate() {
            part.set_numeric_values(name, rows.iter().map(|r| Some(r.0[k])).collect())?;
        }
        part.set_numeric_values("SSE", rows.iter().map(|r| Some(r.1)).collect())?;
        part.set_format("SSE", "F10.8")?;
        part.set_numeric_values("NCASES", rows.iter().map(|r| Some(r.2)).collect())?;
        part.set_format("NCASES", "F8.0")?;
        let part = self.with_split_values(part)?;
        self.matrix_parts.1.push(part);
        Ok(())
    }

    /// SAVE: the predicted values, the residuals and the derivatives (and the loss) appended to
    /// the active data.
    pub(super) fn save_variables(
        &mut self,
        who: &str,
        save: &NlrSave,
        runner: &mut Runner,
        pred_name: &str,
        theta: &[f64],
        y: &[f64],
        current: &Eval,
    ) -> Result<(), Error> {
        let p = theta.len();
        let cases = y.len();
        let names = runner.parameters.clone();
        // SAVE
        let mut saved: Vec<(String, Vec<Option<f64>>, bool)> = Vec::new();
        let opt = |v: f64| Some(v).filter(|v| v.is_finite());
        if save.pred {
            saved.push((
                pred_name.to_string(),
                current.pred.iter().map(|v| opt(*v)).collect(),
                true,
            ));
        }
        if let Some(name) = &save.resid {
            let values = (0..cases)
                .map(|i| {
                    if y[i].is_finite() && current.pred[i].is_finite() {
                        Some(y[i] - current.pred[i])
                    } else {
                        None
                    }
                })
                .collect();
            saved.push((name.clone(), values, false));
        }
        if save.derivatives {
            // the derivative of every parameter, those the program did not give numerically
            for k in 0..p {
                let column: Vec<Option<f64>> = match &current.derivatives[k] {
                    Some(d) => d.iter().map(|v| opt(*v)).collect(),
                    None => {
                        let h = step_of(theta[k]);
                        let (mut up, mut down) = (theta.to_vec(), theta.to_vec());
                        up[k] += h;
                        down[k] -= h;
                        let (u, d) = (runner.run(&up, false)?, runner.run(&down, false)?);
                        (0..cases)
                            .map(|i| opt((u.pred[i] - d.pred[i]) / (2.0 * h)))
                            .collect()
                    }
                };
                let short: String = names[k].chars().take(6).collect();
                saved.push((format!("D.{short}"), column, false));
            }
        }
        for (name, values, is_pred) in saved {
            let data = self.active_mut()?;
            if data.variable_index(&name).is_some() {
                self.warn(format!(
                    "{who} /SAVE: the variable {name} exists already, so it is not saved"
                ));
                continue;
            }
            data.set_numeric_values(&name, values)?;
            data.set_measure(&name, Measure::Scale)?;
            // the residuals and the derivatives print like the predicted values
            data.set_format(&name, &current.format)?;
            if is_pred {
                data.set_variable_label(&name, current.label.as_deref())?;
            }
        }
        Ok(())
    }

    /// `part` with the split variables of the group that runs in front of it.
    fn with_split_values(&self, part: Dataset) -> Result<Dataset, Error> {
        let Some(split) = &self.matrix_parts.0 else {
            return Ok(part);
        };
        let group = self.active()?;
        if let Some(name) = split
            .variables
            .iter()
            .find(|name| part.variable_index(name).is_some())
        {
            return Err(Error(format!(
                "NLR /OUTFILE: the split variable {name} has the name of a parameter"
            )));
        }
        let n = part.case_count();
        let mut joined = Dataset::new(n);
        for name in &split.variables {
            joined.copy_variable(group, name, &vec![Some(0); n])?;
        }
        let cases: Vec<Option<usize>> = (0..n).map(Some).collect();
        for name in part.variable_names() {
            joined.copy_variable(&part, name, &cases)?;
        }
        Ok(joined)
    }
}
