//! The transformation commands that work with expressions: COMPUTE, IF, SELECT IF, STRING,
//! NUMERIC and the DO IF block. Each one changes the dataset when it runs.

use crate::expr::{Column, Evaluator};
use crate::{Engine, Error};
use statscraft_data::Values;
use statscraft_syntax::{Command, Expr, Format, FormatType};

/// One open DO IF block.
#[derive(Clone)]
pub(crate) struct Block {
    /// The cases of the branch that is open now.
    active: Vec<bool>,
    /// The cases a later branch can still take: every condition so far was false for them.
    pending: Vec<bool>,
    else_seen: bool,
    /// A LOOP iteration rather than DO IF: the cases that left the loop by BREAK.
    broken: Option<Vec<bool>>,
}

impl Block {
    /// The block of one LOOP iteration over the cases of `running`.
    pub(crate) fn for_loop(running: Vec<bool>) -> Block {
        let cases = running.len();
        Block {
            active: running,
            pending: vec![false; cases],
            else_seen: false,
            broken: Some(vec![false; cases]),
        }
    }

    pub(crate) fn is_loop(&self) -> bool {
        self.broken.is_some()
    }

    /// The cases that left this loop by BREAK.
    pub(crate) fn broken(&self) -> Vec<bool> {
        self.broken.clone().unwrap_or_default()
    }

    /// The cases of `leaving` run no more commands of this block.
    pub(crate) fn leave(&mut self, leaving: &[bool]) {
        for mask in [&mut self.active, &mut self.pending] {
            for (case, open) in mask.iter_mut().enumerate() {
                *open &= !leaving[case];
            }
        }
    }

    /// Records that the cases of `leaving` left this loop.
    pub(crate) fn broke(&mut self, leaving: &[bool]) {
        if let Some(broken) = self.broken.as_mut() {
            for (case, broke) in broken.iter_mut().enumerate() {
                *broke |= leaving[case];
            }
        }
    }
    /// The block for the cases of `cases` only.
    pub(crate) fn slice(&self, cases: std::ops::Range<usize>) -> Block {
        Block {
            active: self.active[cases.clone()].to_vec(),
            pending: self.pending[cases.clone()].to_vec(),
            else_seen: self.else_seen,
            broken: self.broken.as_ref().map(|b| b[cases].to_vec()),
        }
    }

    /// Keeps the entries of the cases whose `keep` entry is true.
    pub(crate) fn retain(&mut self, keep: &[bool]) {
        for mask in [&mut self.active, &mut self.pending]
            .into_iter()
            .chain(self.broken.as_mut())
        {
            let mut kept = keep.iter();
            mask.retain(|_| *kept.next().unwrap_or(&true));
        }
    }
}

/// A string value cut to the width of its variable, as an assignment does.
pub(crate) fn fit(mut value: String, width: usize) -> String {
    let mut end = width.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

impl Engine {
    /// The cases the open DO IF branch applies to: one entry per case, true where a command
    /// may change the case. None outside DO IF, where every case is active. A command that is
    /// allowed inside DO IF must leave the other cases as they are.
    pub fn active_cases(&self) -> Option<&[bool]> {
        self.blocks.last().map(|block| block.active.as_slice())
    }

    /// To be called when a run ends. An open DO IF block is an error, because the block is not
    /// kept between runs. The end of a run ends the data pass: N OF CASES takes effect and
    /// TEMPORARY changes are undone.
    pub fn finish(&mut self) -> Result<(), Error> {
        if self.collecting.is_some() {
            return Err(Error(
                "LOOP without END LOOP: close the loop with END LOOP in the same run".into(),
            ));
        }
        if let Some(open) = &self.collecting_input {
            return Err(Error(format!(
                "{} without {}: close it in the same run",
                open.opener(),
                open.closer()
            )));
        }
        if self.pending_data.is_some() {
            return Err(Error(
                "DATA LIST or input program without its BEGIN DATA: give the data lines in the \
                 same run"
                    .into(),
            ));
        }
        let open = match self.pending {
            Some(_) => self.pending_depth > 0,
            None => !self.blocks.is_empty(),
        };
        if open {
            return Err(Error(
                "DO IF without END IF: close the block with END IF in the same run".into(),
            ));
        }
        self.begin_pass()?;
        self.end_pass(None);
        self.clean = None;
        Ok(())
    }

    /// Inside DO IF only the commands that honour `active_cases` may run.
    pub(crate) fn check_block(&self, command: &Command) -> Result<(), Error> {
        let allowed = matches!(
            command,
            Command::Compute { .. }
                | Command::VectorAssign { .. }
                | Command::Vector(_)
                | Command::DoRepeat(_)
                | Command::Loop(_)
                | Command::EndLoop(_)
                | Command::Break
                | Command::If { .. }
                | Command::Recode(_)
                | Command::Count(_)
                | Command::Sample(_)
                | Command::DoIf(_)
                | Command::ElseIf(_)
                | Command::Else
                | Command::EndIf
                | Command::Print(_)
                | Command::PrintSpace(_)
                | Command::Xsave(_)
                | Command::EndCase
                | Command::EndFile
                | Command::Reread { .. }
                | Command::RepeatingData(_)
        ) || (self.input.is_some() && matches!(command, Command::DataList(_)));
        let open = match self.pending {
            Some(_) => self.pending_depth > 0,
            None => !self.blocks.is_empty(),
        };
        if !open || allowed {
            return Ok(());
        }
        Err(Error(
            "this command cannot be used inside DO IF or LOOP; only COMPUTE, IF, RECODE, COUNT, \
             SAMPLE, VECTOR, DO REPEAT, BREAK, PRINT, PRINT SPACE, WRITE, XSAVE, nested DO IF \
             and LOOP, and in an input program DATA LIST, END CASE, END FILE, REREAD and \
             REPEATING DATA can"
                .into(),
        ))
    }

    pub(crate) fn evaluator<'a>(
        &'a self,
        active: Option<&'a [bool]>,
    ) -> Result<Evaluator<'a>, Error> {
        let data = self.active()?;
        Ok(Evaluator {
            data,
            active,
            vectors: &self.vectors,
            random: &self.random,
            first_case: self.row.as_ref().map_or(0, |row| row.case),
            previous: self.row.as_ref().map_or(&[], |row| &row.previous[..]),
        })
    }

    /// COMPUTE, or IF when there is a condition.
    pub(crate) fn assign(
        &mut self,
        target: &str,
        expr: &Expr,
        condition: Option<&Expr>,
    ) -> Result<(), Error> {
        // a new scratch variable is 0 until a case gives it a value; any other new variable
        // that its own expression reads is system-missing there
        if self.active()?.variable_index(target).is_none() {
            let (mut names, mut ranges) = (Vec::new(), Vec::new());
            crate::rows::expr_names(expr, &mut names, &mut ranges);
            let scratch = self.carried(target);
            if scratch || names.iter().any(|n| n.eq_ignore_ascii_case(target)) {
                let cases = self.active()?.case_count();
                let start = scratch.then_some(0.0);
                self.active_mut()?
                    .set_numeric_values(target, vec![start; cases])?;
            }
        }
        // the cases that get the value: those of the open DO IF branch where the condition holds
        let mut chosen = self.active_cases().map(<[bool]>::to_vec);
        if let Some(condition) = condition {
            let truth = self.evaluator(chosen.as_deref())?.condition(condition)?;
            let holds = truth.iter().enumerate().map(|(case, truth)| {
                *truth == Some(true) && chosen.as_ref().is_none_or(|chosen| chosen[case])
            });
            chosen = Some(holds.collect());
        }
        let value = self.evaluator(chosen.as_deref())?.eval(expr)?;
        self.put(target, value, chosen)
    }

    /// Stores `value` in `target` for the cases of `chosen` (None: all); the other cases keep
    /// their value, or are missing in a new variable.
    pub(crate) fn put(
        &mut self,
        target: &str,
        value: Column,
        chosen: Option<Vec<bool>>,
    ) -> Result<(), Error> {
        if crate::rows::is_scratch(target) {
            self.mark_assigned(target, chosen.as_deref());
        }
        let chosen = |case: usize| chosen.as_ref().is_none_or(|chosen| chosen[case]);

        let data = self.active_mut()?;
        let existing = data.variable(target).ok();
        let width = existing.and_then(|variable| variable.string_width());
        let merged = match (existing.map(|variable| variable.values()), value) {
            (None, Column::Number(new)) => {
                let values = new.into_iter().enumerate();
                Column::Number(values.map(|(case, v)| v.filter(|_| chosen(case))).collect())
            }
            (Some(Values::Numeric(old)), Column::Number(new)) => {
                let values = new.into_iter().enumerate();
                let merge = |(case, v)| if chosen(case) { v } else { old.get(case) };
                Column::Number(values.map(merge).collect())
            }
            (Some(Values::String(old)), Column::String(new)) => {
                let width = width.unwrap_or(0);
                let merge = |(case, v): (usize, String)| match chosen(case) {
                    true => fit(v, width),
                    false => old.get(case).to_string(),
                };
                Column::String(new.into_iter().enumerate().map(merge).collect())
            }
            (None, Column::String(_)) => {
                return Err(Error(format!(
                    "the expression gives a string, but {target} does not exist: declare it \
                     first, for example STRING {target} (A20)"
                )));
            }
            (Some(Values::String(_)), Column::Number(_)) => {
                return Err(Error(format!(
                    "{target} is a string variable, but the expression gives a number; \
                     convert it with STRING(number, F8.2)"
                )));
            }
            (Some(Values::Numeric(_)), Column::String(_)) => {
                return Err(Error(format!(
                    "{target} is a numeric variable, but the expression gives a string; \
                     convert it with NUMBER(string, F8.2)"
                )));
            }
        };
        match merged {
            Column::Number(values) => data.set_numeric_values(target, values)?,
            Column::String(values) => data.set_string_values(target, width.unwrap_or(0), values)?,
        }
        Ok(())
    }

    pub(crate) fn select_if(&mut self, condition: &Expr) -> Result<(), Error> {
        let truth = self.evaluator(None)?.condition(condition)?;
        let keep: Vec<bool> = truth.iter().map(|truth| *truth == Some(true)).collect();
        if let Some(row) = self.row.as_mut()
            && keep == [false]
        {
            let data = self.dataset.as_ref().expect("checked above");
            row.dropped = Some(crate::rows::scratch_values(data, 0, &self.leave));
        }
        self.select_kept(&keep)
    }

    /// STRING and NUMERIC: new variables, empty or system-missing. Nothing is added when one
    /// of the names cannot be.
    pub(crate) fn declare<'a>(
        &mut self,
        groups: impl Iterator<Item = (&'a [String], Option<&'a Format>)>,
    ) -> Result<(), Error> {
        let mut data = self.active()?.clone();
        let cases = data.case_count();
        for (names, format) in groups {
            for name in names {
                if data.variable_index(name).is_some() {
                    // case by case, a scratch variable arrives with the value of the case before;
                    // in an input program the first pass on no case declared it
                    let input = self.input.is_some() && cases == 1;
                    if (self.row.is_some() && self.carried(name)) || input {
                        continue;
                    }
                    return Err(Error(format!("variable {name} already exists")));
                }
                if format.is_some_and(|format| format.kind == FormatType::A) {
                    let width = format.map_or(0, |format| usize::from(format.width));
                    data.set_string_values(name, width, vec![String::new(); cases])?;
                    continue;
                }
                // a scratch or LEAVE variable starts at 0, any other at system-missing
                let start = self.carried(name).then_some(0.0);
                data.set_numeric_values(name, vec![start; cases])?;
                if let Some(format) = format {
                    let mut text = format!("{}{}", format.kind.name(), format.width);
                    if format.decimals > 0 {
                        text.push_str(&format!(".{}", format.decimals));
                    }
                    data.set_format(name, &text)?;
                }
            }
        }
        self.dataset = Some(data);
        Ok(())
    }

    pub(crate) fn do_if(&mut self, condition: &Expr) -> Result<(), Error> {
        let cases = self.active()?.case_count();
        let pending = match self.active_cases() {
            Some(outer) => outer.to_vec(),
            None => vec![true; cases],
        };
        // DO IF is the first branch of a block in which every case of the outer branch is pending
        self.blocks.push(Block {
            active: Vec::new(),
            pending,
            else_seen: false,
            broken: None,
        });
        let result = self.next_branch(Some(condition));
        if result.is_err() {
            self.blocks.pop();
        }
        result
    }

    /// ELSE IF, or ELSE when there is no condition. A case whose condition is missing leaves
    /// the pending cases without entering the branch, so no later branch runs for it.
    pub(crate) fn next_branch(&mut self, condition: Option<&Expr>) -> Result<(), Error> {
        let name = if condition.is_some() {
            "ELSE IF"
        } else {
            "ELSE"
        };
        let Some(block) = self.blocks.last() else {
            return Err(Error(format!("{name} without DO IF")));
        };
        if block.else_seen {
            return Err(Error(format!(
                "{name} after ELSE: ELSE must be the last branch of its DO IF block"
            )));
        }
        let pending = &block.pending;
        let truth = match condition {
            Some(condition) => self.evaluator(Some(pending))?.condition(condition)?,
            None => vec![Some(true); pending.len()],
        };
        let branch = |wanted: bool| -> Vec<bool> {
            let cases = pending.iter().zip(&truth);
            cases
                .map(|(pending, truth)| *pending && *truth == Some(wanted))
                .collect()
        };
        let next = Block {
            active: branch(true),
            pending: branch(false),
            else_seen: condition.is_none(),
            broken: None,
        };
        *self.blocks.last_mut().expect("checked above") = next;
        Ok(())
    }

    pub(crate) fn end_if(&mut self) -> Result<(), Error> {
        let closed = self.blocks.pop();
        closed
            .map(|_| ())
            .ok_or_else(|| Error("END IF without DO IF".into()))
    }
}
