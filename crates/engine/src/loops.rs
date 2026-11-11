//! DO REPEAT, LOOP with BREAK, and VECTOR.
//!
//! Rules, from the descriptions of these commands:
//! - DO REPEAT runs its commands once per item of its replacement lists, every stand-in
//!   replaced by its item; the lists are stepped through together and must be equally long.
//!   A list holds existing variables or new names, never both.
//! - LOOP runs per case. With an index, the index variable gets the counter at the start of
//!   every iteration; the loop ends for a case when the counter passes the last value (the
//!   index then holds the counter), when the IF of LOOP is not true, after an iteration whose
//!   END LOOP IF is true or missing, or at BREAK (the index then keeps the value of that
//!   iteration). A missing first, last or step value, or a step of 0, means no iteration.
//!   Without an index a loop stops after 40 iterations.
//! - A vector names a range of variables or new ones; v(i) is element i. A case whose index
//!   is missing or out of range reads system-missing and is not assigned. Vectors last until
//!   the next procedure.

use crate::expr::Column;
use crate::rows::{expr_names, has_lag, is_scratch};
use crate::transform::Block;
use crate::variables::new_names;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Values;
use statscraft_syntax::{
    Command, DoRepeat, Expr, LoopHeader, RepeatItem, RepeatList, VarItem, VectorSpec,
    VectorVariables,
};

/// Iterations of a loop without an index, as SET MXLOOPS sets by default.
const MAX_LOOPS: usize = 40;

/// A LOOP whose commands are being collected up to its END LOOP.
#[derive(Clone)]
pub(crate) struct Collecting {
    header: LoopHeader,
    body: Vec<Command>,
    /// LOOPs opened inside and not closed yet.
    nested: usize,
}

impl Engine {
    /// LOOP, END LOOP and the commands between them: collected, then run as one. Returns
    /// None when the command is not part of a loop.
    pub(crate) fn collect_loop(&mut self, command: &Command) -> Option<Result<Vec<Table>, Error>> {
        let Some(collecting) = self.collecting.as_mut() else {
            if let Command::Loop(header) = command {
                if let Err(e) = self.check_block(command) {
                    return Some(Err(e));
                }
                self.collecting = Some(Collecting {
                    header: header.clone(),
                    body: Vec::new(),
                    nested: 0,
                });
                return Some(Ok(Vec::new()));
            }
            return None;
        };
        match command {
            Command::Loop(_) => collecting.nested += 1,
            Command::EndLoop(until) if collecting.nested == 0 => {
                let done = self.collecting.take().expect("checked above");
                return Some(self.finish_loop(done, until.as_ref()));
            }
            Command::EndLoop(_) => collecting.nested -= 1,
            _ => {}
        }
        collecting.body.push(command.clone());
        Some(Ok(Vec::new()))
    }

    fn finish_loop(&mut self, done: Collecting, until: Option<&Expr>) -> Result<Vec<Table>, Error> {
        if self.row.is_none() && self.loop_depth == 0 {
            let flat = || {
                let mut flat = vec![Command::Loop(done.header.clone())];
                flat.extend(done.body.iter().cloned());
                flat.push(Command::EndLoop(until.cloned()));
                flat
            };
            if self.pending.is_none() && self.loop_needs_rows(&done, until)? {
                self.pending = Some(Vec::new());
                self.pending_depth = self.blocks.len();
            }
            if let Some(pending) = self.pending.as_mut() {
                pending.extend(flat());
                return Ok(Vec::new());
            }
        }
        self.run_loop(&done.header, &done.body, until)
    }

    /// True when a loop must run case by case: it calls LAG or reads a scratch variable that
    /// some case has not assigned yet (other than an index, which every iteration sets).
    fn loop_needs_rows(&self, done: &Collecting, until: Option<&Expr>) -> Result<bool, Error> {
        let mut exprs: Vec<&Expr> = until.into_iter().collect();
        let mut indexes = Vec::new();
        let nested = done.body.iter().filter_map(|c| match c {
            Command::Loop(h) => Some(h),
            _ => None,
        });
        for h in std::iter::once(&done.header).chain(nested) {
            if let Some(index) = &h.index {
                indexes.push(index.variable.to_lowercase());
                exprs.extend([&index.first, &index.last]);
                exprs.extend(index.by.as_ref());
            }
            exprs.extend(h.condition.as_ref());
        }
        let mut reads = Vec::new();
        for command in &done.body {
            match command {
                Command::Compute { expr, .. } => exprs.push(expr),
                Command::If {
                    condition, expr, ..
                } => exprs.extend([condition, expr]),
                Command::SelectIf(e) | Command::DoIf(e) | Command::ElseIf(e) => exprs.push(e),
                Command::EndLoop(Some(e)) => exprs.push(e),
                Command::VectorAssign {
                    condition,
                    index,
                    expr,
                    vector,
                } => {
                    exprs.extend(condition.iter().chain([index, expr]));
                    reads.push(vector.clone());
                }
                Command::Count(counts) => {
                    for count in counts {
                        for (vars, _) in &count.criteria {
                            reads.extend(self.expand(vars)?);
                        }
                    }
                }
                // the body of DO REPEAT is only known once it runs
                Command::DoRepeat(_) => return Ok(true),
                Command::Print(_) | Command::PrintSpace(_) | Command::Xsave(_) => {
                    return Ok(true);
                }
                Command::Recode(_) if self.needs_rows(command)? => return Ok(true),
                _ => {}
            }
        }
        if exprs.iter().any(|e| has_lag(e)) {
            return Ok(true);
        }
        let mut ranges = Vec::new();
        for expr in exprs {
            expr_names(expr, &mut reads, &mut ranges);
        }
        for (first, last) in ranges {
            if is_scratch(&first) || is_scratch(&last) {
                reads.extend(self.expand(&[VarItem::Range(first, last)])?);
            }
        }
        // a vector stands for its variables
        let mut names = Vec::new();
        for name in reads {
            match self.vectors.get(&name.to_lowercase()) {
                Some(variables) => names.extend(variables.iter().cloned()),
                None => names.push(name),
            }
        }
        let data = self.active()?;
        let active = self.active_cases();
        for name in names.iter().filter(|n| is_scratch(n)) {
            if indexes.contains(&name.to_lowercase()) {
                continue;
            }
            let Some(index) = data.variable_index(name) else {
                return Ok(true);
            };
            let key = data.variable_at(index).name().to_lowercase();
            let Some(mask) = self.scratch_assigned.get(&key) else {
                return Ok(true);
            };
            if (0..data.case_count()).any(|c| active.is_none_or(|a| a[c]) && !mask[c]) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Runs a loop on all cases at once: each iteration runs the commands for the cases
    /// still looping.
    pub(crate) fn run_loop(
        &mut self,
        header: &LoopHeader,
        body: &[Command],
        until: Option<&Expr>,
    ) -> Result<Vec<Table>, Error> {
        let cases = self.active()?.case_count();
        let outer: Vec<bool> = match self.active_cases() {
            Some(active) => active.to_vec(),
            None => vec![true; cases],
        };
        let mut running = outer.clone();
        let mut counter = Vec::new();
        let (mut last, mut step) = (Vec::new(), Vec::new());
        if let Some(index) = &header.index {
            let numbers = |engine: &Self,
                           expr: &Expr,
                           what: &str|
             -> Result<Vec<Option<f64>>, Error> {
                match engine.evaluator(Some(&outer))?.eval(expr)? {
                    Column::Number(values) => Ok(values),
                    Column::String(_) => Err(Error(format!("the {what} of LOOP must be a number"))),
                }
            };
            counter = numbers(self, &index.first, "first value")?;
            last = numbers(self, &index.last, "last value")?;
            step = match &index.by {
                Some(by) => numbers(self, by, "step")?,
                None => vec![Some(1.0); cases],
            };
            let mut zero = false;
            for case in 0..cases {
                let valid = counter[case].is_some() && last[case].is_some();
                match step[case] {
                    Some(s) if s != 0.0 && valid => {}
                    Some(0.0) if running[case] => {
                        zero = true;
                        running[case] = false;
                    }
                    _ => running[case] = false,
                }
            }
            if zero {
                self.warn("a LOOP step of 0 runs no iteration");
            }
        }
        let mut tables = Vec::new();
        let mut warnings = std::mem::take(&mut self.warnings);
        let mut iterations = 0;
        self.loop_depth += 1;
        let result = (|| -> Result<(), Error> {
            loop {
                if let Some(index) = &header.index {
                    let values = counter.clone();
                    self.put(
                        &index.variable,
                        Column::Number(values),
                        Some(running.clone()),
                    )?;
                    for case in 0..cases {
                        let (Some(c), Some(l), Some(s)) = (counter[case], last[case], step[case])
                        else {
                            continue;
                        };
                        if (s > 0.0 && c > l) || (s < 0.0 && c < l) {
                            running[case] = false;
                        }
                    }
                }
                if let Some(condition) = &header.condition {
                    let truth = self.evaluator(Some(&running))?.condition(condition)?;
                    for (run, truth) in running.iter_mut().zip(truth) {
                        *run &= truth == Some(true);
                    }
                }
                let any = running.contains(&true);
                // the first iteration always runs, if only to create the variables of the body
                if !any && iterations > 0 {
                    break;
                }
                let depth = self.blocks.len();
                self.blocks.push(Block::for_loop(running.clone()));
                for command in body {
                    tables.extend(self.execute(command)?);
                    warnings.append(&mut self.warnings);
                }
                if self.blocks.len() != depth + 1 {
                    return Err(Error(
                        "a DO IF inside LOOP needs its END IF before END LOOP".into(),
                    ));
                }
                let block = self.blocks.pop().expect("pushed above");
                if !any {
                    break;
                }
                for (run, broken) in running.iter_mut().zip(block.broken()) {
                    *run &= !broken;
                }
                if let Some(until) = until {
                    let truth = self.evaluator(Some(&running))?.condition(until)?;
                    for (run, truth) in running.iter_mut().zip(truth) {
                        // a missing condition ends the loop too
                        *run &= truth == Some(false);
                    }
                }
                if header.index.is_some() {
                    for case in (0..cases).filter(|&case| running[case]) {
                        counter[case] = counter[case].zip(step[case]).map(|(c, s)| c + s);
                    }
                }
                iterations += 1;
                if header.index.is_none() && iterations >= MAX_LOOPS {
                    break;
                }
            }
            Ok(())
        })();
        self.loop_depth -= 1;
        self.warnings = warnings;
        result?;
        Ok(tables)
    }

    /// BREAK: the cases of the open branch leave the innermost loop.
    pub(crate) fn break_loop(&mut self) -> Result<(), Error> {
        let Some(at) = self.blocks.iter().rposition(Block::is_loop) else {
            return Err(Error("BREAK is only allowed inside LOOP".into()));
        };
        let leaving = self.active_cases().expect("a loop is open").to_vec();
        for block in &mut self.blocks[at..] {
            block.leave(&leaving);
        }
        self.blocks[at].broke(&leaving);
        Ok(())
    }

    pub(crate) fn vector(&mut self, specs: &[VectorSpec]) -> Result<(), Error> {
        for spec in specs {
            match &spec.variables {
                VectorVariables::Existing(range) => {
                    let names = self.expand(std::slice::from_ref(range))?;
                    let data = self.active()?;
                    let widths: Vec<Option<usize>> = names
                        .iter()
                        .map(|n| data.variable(n).map(|v| v.string_width()))
                        .collect::<Result<_, _>>()?;
                    if widths.iter().any(|w| *w != widths[0]) {
                        return Err(Error(format!(
                            "the variables of vector {} must be all numeric or all strings of one width",
                            spec.names[0]
                        )));
                    }
                    self.vectors.insert(spec.names[0].to_lowercase(), names);
                }
                VectorVariables::New { count, format } => {
                    for prefix in &spec.names {
                        let names: Vec<String> =
                            (1..=*count).map(|i| format!("{prefix}{i}")).collect();
                        self.declare(std::iter::once((&names[..], format.as_ref())))?;
                        self.vectors.insert(prefix.to_lowercase(), names);
                    }
                }
            }
        }
        Ok(())
    }

    /// The variables of a vector.
    fn vector_variables(&self, name: &str) -> Result<Vec<String>, Error> {
        self.vectors
            .get(&name.to_lowercase())
            .cloned()
            .ok_or_else(|| {
                Error(format!(
                    "{name} is not a vector: declare it with VECTOR first"
                ))
            })
    }

    /// `COMPUTE v(i) = expr` and `IF (condition) v(i) = expr`.
    pub(crate) fn vector_assign(
        &mut self,
        condition: Option<&Expr>,
        vector: &str,
        index: &Expr,
        expr: &Expr,
    ) -> Result<(), Error> {
        let variables = self.vector_variables(vector)?;
        let cases = self.active()?.case_count();
        let mut chosen: Vec<bool> = match self.active_cases() {
            Some(active) => active.to_vec(),
            None => vec![true; cases],
        };
        if let Some(condition) = condition {
            let truth = self.evaluator(Some(&chosen))?.condition(condition)?;
            for (c, truth) in chosen.iter_mut().zip(truth) {
                *c &= truth == Some(true);
            }
        }
        let Column::Number(indexes) = self.evaluator(Some(&chosen))?.eval(index)? else {
            return Err(Error(format!("the index of {vector} must be a number")));
        };
        let element = |case: usize| -> Option<usize> {
            let i = indexes[case]?;
            (i.fract() == 0.0 && i >= 1.0 && i <= variables.len() as f64).then(|| i as usize - 1)
        };
        if (0..cases).any(|c| chosen[c] && element(c).is_none()) {
            self.warn(format!(
                "an index of {vector} is missing or outside 1 to {}; those cases are not assigned",
                variables.len()
            ));
        }
        for (k, name) in variables.iter().enumerate() {
            let mask: Vec<bool> = (0..cases)
                .map(|c| chosen[c] && element(c) == Some(k))
                .collect();
            if !mask.contains(&true) && cases > 0 {
                continue;
            }
            let value = self.evaluator(Some(&mask))?.eval(expr)?;
            self.put(name, value, Some(mask))?;
        }
        Ok(())
    }

    /// DO REPEAT: the body once per item of the replacement lists.
    pub(crate) fn do_repeat(&mut self, repeat: &DoRepeat) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut lists: Vec<(String, Vec<RepeatItem>)> = Vec::new();
        for (name, list) in &repeat.lists {
            let items = match list {
                RepeatList::Values(values) => {
                    values.iter().cloned().map(RepeatItem::Value).collect()
                }
                RepeatList::Variables(items) => {
                    let exists = |n: &String| data.variable_index(n).is_some();
                    let known: Vec<bool> = items
                        .iter()
                        .map(|item| match item {
                            VarItem::Name(n) => exists(n),
                            VarItem::Range(a, b) => exists(a) && exists(b),
                            VarItem::All => true,
                        })
                        .collect();
                    let names = if known.iter().all(|k| *k) {
                        self.expand(items)?
                    } else if known.iter().any(|k| *k) {
                        return Err(Error(format!(
                            "the list of {name} mixes existing and new variables; give one kind"
                        )));
                    } else {
                        new_names(items)?
                    };
                    names.into_iter().map(RepeatItem::Name).collect()
                }
            };
            lists.push((name.clone(), items));
        }
        let count = lists[0].1.len();
        if let Some((name, items)) = lists.iter().find(|(_, items)| items.len() != count) {
            return Err(Error(format!(
                "the replacement lists of DO REPEAT differ in length: {} has {count} items, {name} has {}",
                lists[0].0,
                items.len()
            )));
        }
        let mut tables = Vec::new();
        let mut generated = Vec::new();
        let mut warnings = std::mem::take(&mut self.warnings);
        let result = (|| -> Result<(), Error> {
            for i in 0..count {
                let items: Vec<(&str, &RepeatItem)> = lists
                    .iter()
                    .map(|(n, items)| (n.as_str(), &items[i]))
                    .collect();
                let body: Vec<_> = repeat
                    .body
                    .iter()
                    .map(|raw| statscraft_syntax::substitute(raw, &items))
                    .collect();
                if repeat.print {
                    generated.extend(body.iter().map(statscraft_syntax::command_text));
                }
                for (_, parsed) in statscraft_syntax::parse_raws(&body) {
                    let command = parsed
                        .map_err(|e| Error(format!("line {}: {}", e.span.line, e.message)))?;
                    tables.extend(self.execute(&command)?);
                    warnings.append(&mut self.warnings);
                }
            }
            Ok(())
        })();
        self.warnings = warnings;
        result?;
        if repeat.print {
            let rows = generated.into_iter().map(|text| vec![Cell::Text(text)]);
            tables.insert(
                0,
                Table {
                    title: "Commands generated by DO REPEAT".into(),
                    columns: vec!["Command".into()],
                    rows: rows.collect(),
                    ..Table::default()
                },
            );
        }
        Ok(tables)
    }
}

/// The element of a vector a case reads: the value of variable `index - 1`, or missing.
pub(crate) fn element_values(
    data: &statscraft_data::Dataset,
    variables: &[String],
    indexes: &[Option<f64>],
) -> Result<Column, Error> {
    let first = data.variable(&variables[0])?;
    let pick = |case: usize| -> Option<usize> {
        let i = indexes[case]?;
        (i.fract() == 0.0 && i >= 1.0 && i <= variables.len() as f64).then(|| i as usize - 1)
    };
    let cases = indexes.len();
    Ok(match first.values() {
        Values::Numeric(_) => Column::Number(
            (0..cases)
                .map(|case| {
                    let variable = data.variable(&variables[pick(case)?]).ok()?;
                    let value = variable.numeric().ok()?.get(case)?;
                    (!variable.is_user_missing(value)).then_some(value)
                })
                .collect(),
        ),
        Values::String(_) => {
            let width = first.string_width().unwrap_or(0);
            Column::String(
                (0..cases)
                    .map(|case| {
                        let text = pick(case)
                            .and_then(|k| data.variable(&variables[k]).ok())
                            .and_then(|v| v.string().ok().map(|s| s.get(case).to_string()))
                            .unwrap_or_default();
                        format!("{text}{:1$}", "", width.saturating_sub(text.len()))
                    })
                    .collect(),
            )
        }
    })
}
