//! GENLINMIXED: the generalized linear mixed model g(E(y | u)) = X b + Z u + offset, fitted on
//! the working linear mixed model of MIXED.
//!
//! Rules (our notes from the Command Syntax Reference and the Algorithms manual, GENLINMIXED):
//! - Estimation is restricted pseudo likelihood with the predicted random effects in the
//!   expansion. From means mu (start: (y m + 0.5) / (m + 1) for BINOMIAL with m trials, y for
//!   the others) the working target is v = g(mu) - offset + g'(mu) (y - mu) and the working
//!   weight w = prior / (V(mu) g'(mu)^2), the prior weight being the analysis weight times the
//!   trials. MIXED fits v with regression weights w by REML; then mu = g^-1(X b + Z u + offset)
//!   with u the predicted random effects, and again, until the largest relative change of a
//!   covariance parameter, 2 |new - old| / (|new| + |old|), falls below the tolerance.
//! - The residual covariance is phi W^-1: phi is 1 for BINOMIAL and POISSON and a covariance
//!   parameter (the residual variance) for NORMAL, GAMMA and INVERSE_GAUSSIAN.
//! - Every table comes from the final working model: tests, coefficients and their standard
//!   errors from (X'V^-1 X)^-1, covariance parameters with 2 H^-1, information criteria from
//!   its restricted log pseudo likelihood, degrees of freedom the cases less the rank of X
//!   unless DF_METHOD says otherwise.
//! - Effects: a numeric field of scale level is a covariate, any other a factor. A two-valued
//!   target is BINOMIAL: the event is the value that is not the reference (the last value in
//!   TARGET_CATEGORY_ORDER unless REFERENCE names one).

use super::{Fitted, Model, What, dot, fit_model, mat_vec};
use crate::crosstabs::{Key, categories};
use crate::genlin::{
    Family, distribution_name, link_d1, link_inverse, link_name, link_value, mu_valid,
    value_matches, variance,
};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::regression::{plain, table};
use crate::{Engine, Error, Table};
use statscraft_data::{Measure, Values};
use statscraft_syntax::{
    GenLinMixed, GenlinDistribution, GenlinLink, GenlinOffset, GenlinOrder, GenlinTrials, Mixed,
    MixedPrint, RandomEffects, VarItem, default_link, link_allowed,
};

/// What GENLINMIXED adds to the linear mixed model.
pub(super) struct Glmm {
    family: Family,
    link: GenlinLink,
    /// The target of every case of the file (a proportion for events and trials); None leaves
    /// the case out.
    pub(super) y: Vec<Option<f64>>,
    trials: Vec<f64>,
    offset: Vec<f64>,
    /// The scale is 1 rather than a parameter.
    pub(super) unit_scale: bool,
}

/// The messages of MIXED in the words of GENLINMIXED.
pub(super) fn renamed(message: &str) -> String {
    // messages of GENLINMIXED's own name go through here too; leave them as they are
    if message.contains("GENLINMIXED") {
        return message.to_string();
    }
    message
        .replace("MIXED", "GENLINMIXED")
        .replace("MXITER", "MAX_ITERATIONS")
}

/// Start values of the covariance parameters: the weighted residual variance of the fixed part
/// alone, shared out as MIXED does.
fn start_values(model: &Model) -> Vec<f64> {
    let blocks = || model.blocks.iter().enumerate();
    let none: Vec<f64> = blocks()
        .flat_map(|(k, b)| b.start_values(if k == 0 { 1.0 } else { 0.0 }))
        .collect();
    let n = model.y.len();
    let s2 = model.evaluate(&none, super::Depth::Objective).map(|e| {
        (0..n)
            .map(|i| model.g[i] * (model.y[i] - dot(&model.x[i], &e.beta)).powi(2))
            .sum::<f64>()
            / n.saturating_sub(model.p).max(1) as f64
    });
    let share = s2.filter(|v| v.is_finite() && *v > 0.0).unwrap_or(1.0) / model.blocks.len() as f64;
    model
        .blocks
        .iter()
        .flat_map(|b| b.start_values(share))
        .collect()
}

impl Glmm {
    /// The doubly iterative fit: `model` ends as the final working model.
    pub(super) fn fit(
        &self,
        model: &mut Model,
        case: &[usize],
        command: &Mixed,
    ) -> Result<(Fitted, Vec<String>), Error> {
        let n = model.y.len();
        let y: Vec<f64> = case
            .iter()
            .map(|&k| self.y[k].expect("a used case"))
            .collect();
        let off: Vec<f64> = case.iter().map(|&k| self.offset[k]).collect();
        let prior: Vec<f64> = case
            .iter()
            .zip(&model.g)
            .map(|(&k, g)| g * self.trials[k])
            .collect();
        let binomial = self.family == Family::Binomial;
        // a count of 0 has no log: such a case starts half a count up (docs/differences.md)
        let mut mu: Vec<f64> = case
            .iter()
            .zip(&y)
            .map(|(&k, &y)| match binomial {
                true => (y * self.trials[k] + 0.5) / (self.trials[k] + 1.0),
                false if mu_valid(self.family, y) => y,
                false => y + 0.5,
            })
            .collect();
        let tolerance = if command.pconverge.value > 0.0 {
            command.pconverge.value
        } else {
            1e-6
        };
        let mut previous: Option<Vec<f64>> = None;
        let mut iteration = 0;
        loop {
            iteration += 1;
            for i in 0..n {
                let d = link_d1(self.link, mu[i], 0.0);
                model.y[i] = link_value(self.link, mu[i], 0.0) - off[i] + d * (y[i] - mu[i]);
                model.g[i] = prior[i] / (variance(self.family, mu[i], 0.0) * d * d);
                if !(model.y[i].is_finite() && model.g[i].is_finite() && model.g[i] > 0.0) {
                    return Err(Error(
                        "GENLINMIXED: the working target or weight of a case is not finite; the link may not suit the data"
                            .into(),
                    ));
                }
            }
            let start = previous.clone().unwrap_or_else(|| start_values(model));
            let (fitted, mut warnings) = fit_model(model, start, command)?;
            let change = previous.as_ref().map(|old| {
                old.iter()
                    .zip(&fitted.th)
                    .map(|(a, b)| {
                        let size = a.abs() + b.abs();
                        if size == 0.0 {
                            0.0
                        } else {
                            2.0 * (b - a).abs() / size
                        }
                    })
                    .fold(0.0_f64, f64::max)
            });
            if change.is_some_and(|c| c < tolerance) {
                return Ok((fitted, warnings));
            }
            if iteration >= command.mxiter.max(1) {
                warnings.push(format!(
                    "GENLINMIXED reached MAX_ITERATIONS ({}) outer iterations without converging. The estimates may not be final.",
                    command.mxiter
                ));
                return Ok((fitted, warnings));
            }
            // the new means from X b + Z u, where Z u = v - X b - R V^-1 (v - X b)
            let e = &fitted.eval;
            for (gi, cases) in model.groups.iter().enumerate() {
                let r: Vec<f64> = cases
                    .iter()
                    .map(|&i| model.y[i] - dot(&model.x[i], &e.beta))
                    .collect();
                let q = mat_vec(&e.vi[gi], &r);
                for (a, &i) in cases.iter().enumerate() {
                    let eta =
                        model.y[i] - model.residual_entry(&fitted.th, i, i, What::Value) * q[a];
                    mu[i] = link_inverse(self.link, eta + off[i], 0.0);
                    if !mu_valid(self.family, mu[i]) {
                        return Err(Error(
                            "GENLINMIXED: a fitted mean left the range of the distribution; the link may not suit the data"
                                .into(),
                        ));
                    }
                }
            }
            previous = Some(fitted.th);
        }
    }
}

fn family_of(d: GenlinDistribution) -> Family {
    match d {
        GenlinDistribution::Normal => Family::Normal,
        GenlinDistribution::Binomial => Family::Binomial,
        GenlinDistribution::Gamma => Family::Gamma,
        GenlinDistribution::InverseGaussian => Family::InverseGaussian,
        GenlinDistribution::Poisson => Family::Poisson,
        // the parser stops the rest
        _ => Family::NegBin,
    }
}

fn not_yet(what: &str) -> Error {
    Error(format!("GENLINMIXED: {what} is not supported yet"))
}

impl Engine {
    pub(crate) fn genlinmixed(&mut self, command: &GenLinMixed) -> Result<Vec<Table>, Error> {
        if command.random.is_empty() {
            return Err(not_yet(
                "a model without RANDOM (use GENLIN for a model with fixed effects only)",
            ));
        }
        let scale_min = self.scale_min();
        let data = self.active()?;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        // the fields of FIELDS are used nowhere else, nor the subjects in the effects
        let mut fields: Vec<&str> = vec![&command.target];
        if let Some(GenlinTrials::Variable(t)) = &command.trials {
            fields.push(t);
        }
        if let Some(GenlinOffset::Variable(o)) = &command.offset {
            fields.push(o);
        }
        fields.extend(command.analysis_weight.as_deref());
        for (i, f) in fields.iter().enumerate() {
            if fields[..i].iter().any(|g| same(f, g)) {
                return Err(Error(format!(
                    "GENLINMIXED: {f} appears more than once on FIELDS"
                )));
            }
        }
        let effect_fields: Vec<&String> = command
            .fixed
            .iter()
            .chain(command.random.iter().flat_map(|r| &r.terms))
            .flat_map(|t| &t.variables)
            .collect();
        let subject_fields: Vec<&String> = command
            .subjects
            .iter()
            .chain(command.random.iter().flat_map(|r| &r.subjects))
            .collect();
        for name in effect_fields.iter().chain(&subject_fields) {
            if fields.iter().any(|f| same(f, name)) {
                return Err(Error(format!(
                    "GENLINMIXED: {name} is on FIELDS and cannot be used elsewhere"
                )));
            }
        }
        for r in &command.random {
            if let Some(s) = r
                .subjects
                .iter()
                .find(|s| !command.subjects.iter().any(|d| same(d, s)))
            {
                return Err(Error(format!(
                    "GENLINMIXED: the RANDOM subject {s} must also be a SUBJECTS field of DATA_STRUCTURE"
                )));
            }
        }
        if let Some(s) = subject_fields
            .iter()
            .find(|s| effect_fields.iter().any(|e| same(e, s)))
        {
            return Err(Error(format!(
                "GENLINMIXED: the subject field {s} cannot also be an effect"
            )));
        }
        // factors and covariates by measurement level
        let mut factors: Vec<String> = Vec::new();
        let mut covariates: Vec<String> = Vec::new();
        for name in &effect_fields {
            let v = data.variable(name)?;
            if factors.iter().chain(&covariates).any(|n| same(n, v.name())) {
                continue;
            }
            let index = data.variable_index(v.name()).expect("found");
            let measure = match v.measure() {
                Measure::Unknown => default_measure(data, index, scale_min),
                known => known,
            };
            if !v.is_string() && measure == Measure::Scale {
                covariates.push(v.name().to_string());
            } else {
                factors.push(v.name().to_string());
            }
        }
        // the distribution: a scale target is NORMAL, events and trials BINOMIAL, and a
        // categorical target of two values BINOMIAL (a nominal multinomial model of two
        // categories is the binomial logit model; docs/differences.md)
        let target = data.variable(&command.target)?;
        let target_index = data.variable_index(target.name()).expect("found");
        let target_measure = match target.measure() {
            Measure::Unknown => default_measure(data, target_index, scale_min),
            known => known,
        };
        let categorical = target.is_string() || target_measure != Measure::Scale;
        let cases = data.case_count();
        let keys = categories(&target, false);
        let mut values: Vec<&Key> = Vec::new();
        for key in keys.iter().flatten() {
            if !values.iter().any(|v| v.compare(key).is_eq()) {
                values.push(key);
            }
        }
        let distribution = match command.distribution {
            Some(d) => d,
            None if command.trials.is_some() => GenlinDistribution::Binomial,
            None if !categorical => GenlinDistribution::Normal,
            None if values.len() == 2 => GenlinDistribution::Binomial,
            None => {
                return Err(not_yet(&format!(
                    "the multinomial distribution, the default for the categorical target {} with {} values",
                    target.name(),
                    values.len()
                )));
            }
        };
        let link = command.link.unwrap_or(default_link(distribution));
        if !link_allowed(distribution, link) {
            return Err(Error(format!(
                "GENLINMIXED: the {} link cannot be used with the {} distribution",
                link_name(link),
                distribution_name(distribution, None)
            )));
        }
        if command.trials.is_some() && distribution != GenlinDistribution::Binomial {
            return Err(Error(
                "GENLINMIXED: TRIALS needs the BINOMIAL distribution".into(),
            ));
        }
        if target.is_string()
            && !(distribution == GenlinDistribution::Binomial && command.trials.is_none())
        {
            return Err(Error(format!(
                "GENLINMIXED: the string target {} can only be a two-valued BINOMIAL target",
                target.name()
            )));
        }
        let numeric = |name: &str| -> Result<Vec<Option<f64>>, Error> {
            let v = data.variable(name)?;
            let Values::Numeric(values) = v.values() else {
                return Err(Error(format!(
                    "GENLINMIXED needs a numeric field, but {name} is a string field"
                )));
            };
            Ok((0..cases)
                .map(|k| values.get(k).filter(|_| is_valid(&v, k, false)))
                .collect())
        };
        let mut y: Vec<Option<f64>> = vec![None; cases];
        let mut trials = vec![1.0; cases];
        let whole = |v: f64| v.fract() == 0.0;
        if distribution == GenlinDistribution::Binomial && command.trials.is_none() {
            if values.len() != 2 {
                return Err(Error(format!(
                    "GENLINMIXED: a BINOMIAL target without TRIALS has two values, but {} has {}",
                    target.name(),
                    values.len()
                )));
            }
            values.sort_by(|a, b| a.compare(b));
            let reference = match (&command.reference, command.target_order) {
                (Some(r), _) => values
                    .iter()
                    .position(|k| value_matches(&target, k, r))
                    .ok_or_else(|| {
                        Error(format!(
                            "GENLINMIXED: REFERENCE is not a value of {}",
                            target.name()
                        ))
                    })?,
                (None, GenlinOrder::Ascending) => 1,
                (None, GenlinOrder::Descending) => 0,
                // the value met last in the file is the last category
                (None, GenlinOrder::Data) => {
                    let first = keys.iter().flatten().next().expect("two values");
                    usize::from(values[0].compare(first).is_eq())
                }
            };
            for (k, key) in keys.iter().enumerate() {
                y[k] = key
                    .as_ref()
                    .map(|key| f64::from(u8::from(values[reference].compare(key).is_ne())));
            }
        } else {
            let events = numeric(target.name())?;
            let family = family_of(distribution);
            for (k, e) in events.iter().enumerate() {
                y[k] = e.filter(|&e| match family {
                    Family::Binomial => e >= 0.0 && whole(e),
                    Family::Poisson => e >= 0.0 && whole(e),
                    Family::Gamma | Family::InverseGaussian => e > 0.0,
                    _ => true,
                });
            }
            match &command.trials {
                Some(GenlinTrials::Number(m)) => trials = vec![*m; cases],
                Some(GenlinTrials::Variable(t)) => {
                    for (k, m) in numeric(t)?.into_iter().enumerate() {
                        trials[k] = m.unwrap_or(f64::NAN);
                    }
                }
                None => {}
            }
            if family == Family::Binomial {
                for k in 0..cases {
                    let m = trials[k];
                    y[k] = y[k]
                        .filter(|&e| m >= 1.0 && whole(m) && e <= m)
                        .map(|e| e / m);
                }
            }
        }
        let offset = match &command.offset {
            None => vec![0.0; cases],
            Some(GenlinOffset::Value(x)) => vec![*x; cases],
            Some(GenlinOffset::Variable(o)) => {
                let values = numeric(o)?;
                for (k, v) in values.iter().enumerate() {
                    if v.is_none() {
                        y[k] = None;
                    }
                }
                values.into_iter().map(|v| v.unwrap_or(0.0)).collect()
            }
        };
        // a case missing on a subject field of DATA_STRUCTURE is left out
        for name in &command.subjects {
            let keys = categories(&data.variable(name)?, false);
            for (k, key) in keys.iter().enumerate() {
                if key.is_none() {
                    y[k] = None;
                }
            }
        }
        if let Some(w) = &command.analysis_weight {
            numeric(w)?;
        }
        let family = family_of(distribution);
        let glmm = Glmm {
            family,
            link,
            y,
            trials,
            offset,
            unit_scale: matches!(family, Family::Binomial | Family::Poisson),
        };
        let mixed = Mixed {
            dependent: target.name().to_string(),
            factors: factors.into_iter().map(VarItem::Name).collect(),
            covariates: covariates.into_iter().map(VarItem::Name).collect(),
            fixed: command.fixed.clone(),
            intercept: command.intercept,
            sstype: 3,
            ml: false,
            include: false,
            cin: command.confidence,
            mxiter: command.max_iterations,
            mxstep: 10,
            scoring: command.scoring,
            singular: command.singular,
            pconverge: command.pconverge,
            lconverge: command.lconverge,
            hconverge: command.hconverge,
            df_method: command.df_method,
            print: MixedPrint {
                solution: true,
                testcov: true,
                ..MixedPrint::default()
            },
            random: command
                .random
                .iter()
                .map(|r| RandomEffects {
                    terms: r.terms.clone(),
                    subject: r.subjects.clone(),
                    covtype: r.covtype,
                    solution: r.solution,
                })
                .collect(),
            repeated: None,
            regwgt: command.analysis_weight.clone(),
            save: Vec::new(),
            emmeans: Vec::new(),
            tests: Vec::new(),
        };
        let target_text = self.shown().variable(&target);
        let summary = table(
            "Model Summary",
            1,
            plain(&["", "Value"]),
            vec![
                vec![text("Target"), text(target_text)],
                vec![
                    text("Probability Distribution"),
                    text(distribution_name(distribution, None)),
                ],
                vec![text("Link Function"), text(link_name(link))],
            ],
            Some(
                "Fitted by restricted pseudo likelihood: the information criteria belong to the final working linear model."
                    .into(),
            ),
        );
        let mut tables = vec![summary];
        tables.extend(
            self.mixed_fit(&mixed, Some(&glmm))
                .map_err(|e| Error(renamed(&e.0)))?,
        );
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn renamed_once() {
        assert_eq!(
            super::renamed("MIXED reached MXITER"),
            "GENLINMIXED reached MAX_ITERATIONS"
        );
        let own = "GENLINMIXED reached MAX_ITERATIONS (50)";
        assert_eq!(super::renamed(own), own);
    }
}
