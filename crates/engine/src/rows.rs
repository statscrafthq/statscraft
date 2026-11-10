//! Scratch variables, and the data pass run case by case where a case needs the one before.
//!
//! Rules, from the description of scratch variables and of LAG:
//! - A scratch variable (`#name`) is 0 or blank at the first case and keeps its value from
//!   case to case; it is never reset. It is gone when the next command that reads the data
//!   begins, and at TEMPORARY. Procedures never see it, and a file never holds it.
//! - LAG gives a variable's value in an earlier case as that case ended the pass, counting only
//!   the cases that SELECT IF kept.
//!
//! Commands run on whole columns, which gives the same result as case by case as long as no
//! case reads a value an earlier case left behind. When a command would (a scratch variable a
//! case has not assigned yet in this pass, or LAG), the engine keeps that command and the
//! transformations after it and runs them case by case when the pass ends, or when a command
//! that is no transformation comes first. The cases before were exact, so the case-by-case
//! part starts from them: a scratch value a case had assigned stays, the others come from the
//! case before.

use crate::{Engine, Error};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{Command, Expr, VarItem};

pub(crate) fn is_scratch(name: &str) -> bool {
    name.starts_with('#')
}

/// The scratch value of a case, carried to the next case.
#[derive(Clone)]
pub(crate) enum Carried {
    Number(Option<f64>),
    Text(usize, String),
}

/// What an engine that runs one case of a case-by-case pass knows besides the case.
#[derive(Clone, Default)]
pub(crate) struct Row {
    /// The 0-based number of the case in the pass, for $CASENUM.
    pub case: usize,
    /// The cases kept so far as they ended the pass, the latest last, for LAG.
    pub previous: Vec<Dataset>,
    /// The scratch values at the moment SELECT IF dropped the case.
    pub dropped: Option<Vec<(String, Carried)>>,
}

/// The scratch values of a case.
pub(crate) fn scratch_values(data: &Dataset, case: usize) -> Vec<(String, Carried)> {
    let scratch = data.variables().filter(|v| is_scratch(v.name()));
    scratch
        .map(|v| {
            let value = match v.values() {
                Values::Numeric(values) => Carried::Number(values.get(case)),
                Values::String(values) => {
                    Carried::Text(v.string_width().unwrap_or(1), values.get(case).to_string())
                }
            };
            (v.name().to_string(), value)
        })
        .collect()
}

fn set_carried(data: &mut Dataset, name: &str, value: &Carried) -> Result<(), Error> {
    match value {
        Carried::Number(v) => data.set_numeric_values(name, vec![*v])?,
        Carried::Text(width, text) => data.set_string_values(name, *width, vec![text.clone()])?,
    }
    Ok(())
}

/// The variables an expression reads; a range as its two ends.
pub(crate) fn expr_names(expr: &Expr, names: &mut Vec<String>, ranges: &mut Vec<(String, String)>) {
    match expr {
        Expr::Variable(name) => names.push(name.clone()),
        Expr::Range(first, last) => ranges.push((first.clone(), last.clone())),
        Expr::Unary { operand, .. } => expr_names(operand, names, ranges),
        Expr::Binary { left, right, .. } => {
            expr_names(left, names, ranges);
            expr_names(right, names, ranges);
        }
        Expr::Call { name, args } => {
            // a vector element names its vector
            names.push(name.clone());
            args.iter().for_each(|a| expr_names(a, names, ranges));
        }
        Expr::Number(_) | Expr::String(_) => {}
    }
}

/// True when an expression calls LAG.
pub(crate) fn has_lag(expr: &Expr) -> bool {
    match expr {
        Expr::Call { name, args } => name.eq_ignore_ascii_case("LAG") || args.iter().any(has_lag),
        Expr::Unary { operand, .. } => has_lag(operand),
        Expr::Binary { left, right, .. } => has_lag(left) || has_lag(right),
        _ => false,
    }
}

/// The variables a command refers to, by name and by range, or None when it may refer to
/// every variable.
pub(crate) struct Refs {
    pub names: Vec<String>,
    pub ranges: Vec<(String, String)>,
    pub all: bool,
}

impl Refs {
    fn items(&mut self, items: &[VarItem]) {
        for item in items {
            match item {
                VarItem::Name(name) => self.names.push(name.clone()),
                VarItem::Range(a, b) => self.ranges.push((a.clone(), b.clone())),
                VarItem::All => self.all = true,
            }
        }
    }

    fn expr(&mut self, expr: &Expr) {
        expr_names(expr, &mut self.names, &mut self.ranges);
    }

    pub fn of(commands: &[Command]) -> Refs {
        let mut refs = Refs {
            names: Vec::new(),
            ranges: Vec::new(),
            all: false,
        };
        for command in commands {
            refs.command(command);
        }
        refs
    }

    fn command(&mut self, command: &Command) {
        match command {
            Command::Compute { target, expr } => {
                self.names.push(target.clone());
                self.expr(expr);
            }
            Command::If {
                condition,
                target,
                expr,
            } => {
                self.names.push(target.clone());
                self.expr(condition);
                self.expr(expr);
            }
            Command::SelectIf(e) | Command::DoIf(e) | Command::ElseIf(e) => self.expr(e),
            Command::Recode(groups) => {
                for group in groups {
                    self.items(&group.vars);
                    self.items(&group.into);
                }
            }
            Command::Count(counts) => {
                for count in counts {
                    self.names.push(count.target.clone());
                    count.criteria.iter().for_each(|(vars, _)| self.items(vars));
                }
            }
            Command::String(groups) => groups
                .iter()
                .for_each(|(n, _)| self.names.extend(n.clone())),
            Command::Numeric(groups) => groups
                .iter()
                .for_each(|(n, _)| self.names.extend(n.clone())),
            Command::Else | Command::EndIf | Command::Sample(_) => {}
            // anything else may name variables in ways not listed here
            _ => self.all = true,
        }
    }
}

impl Engine {
    /// True when the command, run on whole columns now, would give another result than run
    /// case by case: it reads a scratch value that some case has not assigned in this pass,
    /// writes a scratch variable in a way that keeps the value of an earlier case, or calls
    /// LAG.
    pub(crate) fn needs_rows(&self, command: &Command) -> Result<bool, Error> {
        let Some(data) = self.dataset.as_ref() else {
            return Ok(false);
        };
        let mut reads = Vec::new();
        let mut ranges = Vec::new();
        let mut target = None;
        match command {
            Command::Compute { target: t, expr } => {
                target = Some(t);
                expr_names(expr, &mut reads, &mut ranges);
            }
            Command::If {
                condition,
                target: t,
                expr,
            } => {
                target = Some(t);
                expr_names(condition, &mut reads, &mut ranges);
                expr_names(expr, &mut reads, &mut ranges);
            }
            Command::SelectIf(e) | Command::DoIf(e) | Command::ElseIf(e) => {
                expr_names(e, &mut reads, &mut ranges)
            }
            Command::VectorAssign {
                condition,
                index,
                expr,
                ..
            } => {
                for e in condition.iter().chain([index, expr]) {
                    expr_names(e, &mut reads, &mut ranges);
                }
            }
            Command::Recode(groups) => {
                let items = groups.iter().flat_map(|g| g.vars.iter().chain(&g.into));
                let named = |item: &VarItem| match item {
                    VarItem::Name(n) => is_scratch(n),
                    VarItem::Range(a, b) => is_scratch(a) || is_scratch(b),
                    VarItem::All => false,
                };
                if items.clone().any(named) {
                    return Ok(true);
                }
            }
            Command::Count(counts) => {
                for count in counts {
                    for (vars, _) in &count.criteria {
                        reads.extend(self.expand(vars)?);
                    }
                }
            }
            _ => return Ok(false),
        }
        let lag = match command {
            Command::Compute { expr, .. } => has_lag(expr),
            Command::If {
                condition, expr, ..
            } => has_lag(condition) || has_lag(expr),
            Command::SelectIf(e) | Command::DoIf(e) | Command::ElseIf(e) => has_lag(e),
            Command::VectorAssign {
                condition,
                index,
                expr,
                ..
            } => condition.iter().chain([index, expr]).any(has_lag),
            _ => false,
        };
        if lag {
            return Ok(true);
        }
        // a vector element reads the variables of the vector
        let vectors: Vec<String> = reads
            .iter()
            .filter_map(|n| self.vectors.get(&n.to_lowercase()))
            .flatten()
            .cloned()
            .collect();
        reads.extend(vectors);
        for (first, last) in ranges {
            if is_scratch(&first) || is_scratch(&last) {
                reads.extend(self.expand(&[VarItem::Range(first, last)])?);
            }
        }
        let active = self.active_cases();
        for name in reads.iter().filter(|n| is_scratch(n)) {
            let Some(index) = data.variable_index(name) else {
                // a new scratch variable read by the COMPUTE that creates it starts at 0
                if target.is_some_and(|t| t.eq_ignore_ascii_case(name)) {
                    return Ok(true);
                }
                continue;
            };
            let key = data.variable_at(index).name().to_lowercase();
            let assigned = self.scratch_assigned.get(&key);
            let unassigned = |case: usize| assigned.is_none_or(|mask| !mask[case]);
            let open = |case: &usize| active.is_none_or(|active| active[*case]);
            if (0..data.case_count()).filter(open).any(unassigned) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Records that a scratch variable got a value in the cases of `chosen` (None: all).
    pub(crate) fn mark_assigned(&mut self, name: &str, chosen: Option<&[bool]>) {
        let cases = self.dataset.as_ref().map_or(0, Dataset::case_count);
        let mask = self
            .scratch_assigned
            .entry(name.to_lowercase())
            .or_insert_with(|| vec![false; cases]);
        for (case, assigned) in mask.iter_mut().enumerate() {
            *assigned |= chosen.is_none_or(|chosen| chosen[case]);
        }
    }

    /// The end of a pass: scratch variables are gone.
    pub(crate) fn drop_scratch(&mut self) -> Result<(), Error> {
        self.scratch_assigned.clear();
        let Some(data) = self.dataset.as_mut() else {
            return Ok(());
        };
        let scratch: Vec<String> = data
            .variable_names()
            .filter(|n| is_scratch(n))
            .map(str::to_string)
            .collect();
        if scratch.is_empty() {
            return Ok(());
        }
        if scratch.len() == data.variable_count() {
            *data = Dataset::new(data.case_count());
            return Ok(());
        }
        let names: Vec<&str> = scratch.iter().map(String::as_str).collect();
        data.delete_variables(&names)?;
        Ok(())
    }

    /// Runs the transformations kept for the case-by-case pass, if any.
    pub(crate) fn flush(&mut self) -> Result<(), Error> {
        let Some(commands) = self.pending.take() else {
            return Ok(());
        };
        let base = self.active()?.clone();
        let blocks = std::mem::take(&mut self.blocks);
        let assigned = std::mem::take(&mut self.scratch_assigned);

        // only the variables the commands refer to go case by case
        let refs = Refs::of(&commands);
        let projected = if refs.all || !self.vectors.is_empty() {
            base.clone()
        } else {
            let mut keep = vec![false; base.variable_count()];
            for name in &refs.names {
                if let Some(index) = base.variable_index(name) {
                    keep[index] = true;
                }
            }
            for (first, last) in &refs.ranges {
                if let (Some(a), Some(b)) = (base.variable_index(first), base.variable_index(last))
                {
                    keep[a.min(b)..=a.max(b)].iter_mut().for_each(|k| *k = true);
                }
            }
            // a dataset keeps at least one variable
            keep[0] = keep[0] || !keep.contains(&true);
            let names: Vec<&str> = (0..base.variable_count())
                .filter(|&i| keep[i])
                .map(|i| base.variable_at(i).name())
                .collect();
            let mut projected = base.clone();
            projected.keep_variables(&names)?;
            projected
        };
        let lags = lag_depth(&commands);

        let engine = |data: Dataset, case: Option<usize>, previous: Vec<Dataset>| Engine {
            dataset: Some(data),
            blocks: blocks
                .iter()
                .map(|b| b.slice(case.map_or(0..0, |c| c..c + 1)))
                .collect(),
            vectors: self.vectors.clone(),
            row: Some(Box::new(Row {
                case: case.unwrap_or(0),
                previous,
                dropped: None,
            })),
            ..Engine::default()
        };
        // the commands on no case at all give the dictionary of the result
        let mut first = engine(projected.slice(0, 0), None, Vec::new());
        let mut warnings: Vec<String> = Vec::new();
        let earlier = |e: Error| {
            Error(format!(
                "in the transformations that run case by case before this command: {}",
                e.0
            ))
        };
        for command in &commands {
            self.flushed
                .extend(first.execute(command).map_err(earlier)?);
            warnings.extend(first.warnings.iter().cloned());
        }
        let template = first.dataset.take().expect("the engine keeps its dataset");
        let vectors_after = std::mem::take(&mut first.vectors);

        let mut kept: Vec<Dataset> = Vec::new();
        let mut kept_cases: Vec<Option<usize>> = Vec::new();
        let mut carry: Vec<(String, Carried)> = Vec::new();
        for case in 0..base.case_count() {
            let mut row = projected.slice(case, 1);
            for (name, value) in &carry {
                let own = assigned.get(&name.to_lowercase()).is_some_and(|m| m[case]);
                if !own || row.variable_index(name).is_none() {
                    set_carried(&mut row, name, value)?;
                }
            }
            let previous = kept[kept.len().saturating_sub(lags)..].to_vec();
            let mut engine = engine(row, Some(case), previous);
            for command in &commands {
                engine
                    .execute(command)
                    .map_err(|e| earlier(Error(format!("case {}: {}", case + 1, e.0))))?;
                warnings.extend(engine.warnings.iter().cloned());
            }
            let data = engine.dataset.take().expect("the engine keeps its dataset");
            let dropped = engine.row.and_then(|row| row.dropped);
            carry = dropped.unwrap_or_else(|| scratch_values(&data, 0));
            if data.case_count() == 1 {
                kept.push(data);
                kept_cases.push(Some(case));
            }
        }

        let combined = Dataset::from_parts(&template, &kept)?;
        let mut result = base;
        result.take_cases(&kept_cases)?;
        let all: Vec<Option<usize>> = (0..combined.case_count()).map(Some).collect();
        for variable in combined.variables() {
            let name = variable.name();
            if result.variable_index(name).is_none() {
                result.copy_variable(&combined, name, &all)?;
                continue;
            }
            match variable.values() {
                Values::Numeric(values) => {
                    result.set_numeric_values(name, values.iter().collect())?
                }
                Values::String(values) => {
                    let width = variable.string_width().unwrap_or(1);
                    let values = values.iter().map(str::to_string).collect();
                    result.set_string_values(name, width, values)?
                }
            }
        }
        // every scratch variable now has a value in every case
        let cases = result.case_count();
        for name in result.variable_names().filter(|n| is_scratch(n)) {
            self.scratch_assigned
                .insert(name.to_lowercase(), vec![true; cases]);
        }
        self.dataset = Some(result);
        self.vectors = vectors_after;
        let mut seen = Vec::new();
        for warning in warnings {
            if !seen.contains(&warning) {
                seen.push(warning);
            }
        }
        self.warnings.extend(seen);
        Ok(())
    }
}

/// The most cases back any LAG of the commands reaches.
fn lag_depth(commands: &[Command]) -> usize {
    fn depth(expr: &Expr) -> usize {
        match expr {
            Expr::Call { name, args } => {
                let own = match (name.eq_ignore_ascii_case("LAG"), args.get(1)) {
                    (true, Some(Expr::Number(n))) if *n >= 1.0 => *n as usize,
                    (true, _) => 1,
                    _ => 0,
                };
                args.iter().map(depth).max().unwrap_or(0).max(own)
            }
            Expr::Unary { operand, .. } => depth(operand),
            Expr::Binary { left, right, .. } => depth(left).max(depth(right)),
            _ => 0,
        }
    }
    let exprs = commands.iter().flat_map(|c| -> Vec<&Expr> {
        match c {
            Command::Compute { expr, .. } => vec![expr],
            Command::If {
                condition, expr, ..
            } => vec![condition, expr],
            Command::SelectIf(e) | Command::DoIf(e) | Command::ElseIf(e) => vec![e],
            Command::EndLoop(Some(e)) => vec![e],
            Command::VectorAssign {
                condition,
                index,
                expr,
                ..
            } => condition.iter().chain([index, expr]).collect(),
            _ => Vec::new(),
        }
    });
    exprs.map(depth).max().unwrap_or(0)
}
