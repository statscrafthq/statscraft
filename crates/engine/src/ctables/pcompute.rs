//! Computed categories: /PCOMPUTE, /PPROPERTIES and `&name` in an explicit category list.
//!
//! The rules, in our words, from the command reference (CTABLES, PCOMPUTE and PPROPERTIES
//! subcommands, and the notes on computed categories under SIGTEST and COMPARETEST):
//! - A computed category has no cases of its own. For every summary its value is the expression
//!   over the values of the same summary in the categories it names, in the same row, column and
//!   layer otherwise: a difference of counts, of column percentages, of means.
//! - `[value]` is one category, `[low THRU high]` the sum over the listed categories in the
//!   range, `TOTAL` the total of the variable (it needs TOTAL=YES); numbers, + - * / ** and
//!   parentheses combine them. A value that cannot be computed, such as a division by 0, leaves
//!   the cell missing.
//! - Every category the expression names must be in the explicit list; the place of `&name` in
//!   the list is its place in the table. Totals do not count computed categories, and there are
//!   no tests on them.
//! - PPROPERTIES gives the label (else the expression is the label), a display format per
//!   summary, and HIDESOURCECATS=YES, which hides the categories and the total the expression uses
//!   (they are still computed); it cannot go with tests.

use super::{Category, CategoryList, Key, format_text, in_range};
use crate::Error;
use statscraft_syntax::{Ctables, PcomputeExpr, PcomputeOp, SummaryFunction, Value as SyntaxValue};

/// An expression with its categories resolved to positions in the category list.
#[derive(Clone, Debug)]
pub(super) enum Calc {
    Category(usize),
    Sum(Vec<usize>),
    Number(f64),
    Negate(Box<Calc>),
    Binary(PcomputeOp, Box<Calc>, Box<Calc>),
}

/// What a computed category holds besides its label.
#[derive(Clone, Debug)]
pub(super) struct Computed {
    pub calc: Calc,
    /// PPROPERTIES FORMAT: the display format of a summary in this category.
    pub formats: Vec<(SummaryFunction, String)>,
}

impl Calc {
    /// The value, given the value of the same summary in a category of the list.
    pub(super) fn eval(&self, value: &dyn Fn(usize) -> Option<f64>) -> Option<f64> {
        let result = match self {
            Calc::Category(k) => value(*k)?,
            Calc::Sum(ks) => ks.iter().map(|k| value(*k)).sum::<Option<f64>>()?,
            Calc::Number(n) => *n,
            Calc::Negate(inner) => -inner.eval(value)?,
            Calc::Binary(op, left, right) => {
                let (a, b) = (left.eval(value)?, right.eval(value)?);
                match op {
                    PcomputeOp::Add => a + b,
                    PcomputeOp::Subtract => a - b,
                    PcomputeOp::Multiply => a * b,
                    PcomputeOp::Divide => a / b,
                    PcomputeOp::Power => a.powf(b),
                }
            }
        };
        result.is_finite().then_some(result)
    }

    /// The categories the expression uses.
    fn sources(&self, out: &mut Vec<usize>) {
        match self {
            Calc::Category(k) => out.push(*k),
            Calc::Sum(ks) => out.extend(ks),
            Calc::Number(_) => {}
            Calc::Negate(inner) => inner.sources(out),
            Calc::Binary(_, left, right) => {
                left.sources(out);
                right.sources(out);
            }
        }
    }
}

/// A placeholder for `&name` in a list, filled in by `resolve` once the list is complete.
pub(super) fn placeholder() -> Category {
    Category {
        key: None,
        label: String::new(),
        missing: false,
        computed: Some(Computed {
            calc: Calc::Number(0.0),
            formats: Vec::new(),
        }),
        hidden: false,
    }
}

/// Fills in the computed categories of a list, `names` in list order, from the PCOMPUTE and
/// PPROPERTIES subcommands.
pub(super) fn resolve(
    variable: &str,
    numeric: bool,
    list: &mut CategoryList,
    names: &[String],
    command: &Ctables,
) -> Result<(), Error> {
    let places: Vec<usize> = (0..list.categories.len())
        .filter(|&i| list.categories[i].computed.is_some())
        .collect();
    let mut hide = Vec::new();
    for (&place, name) in places.iter().zip(names) {
        let Some(pcompute) = command
            .pcomputes
            .iter()
            .rfind(|p| p.name.eq_ignore_ascii_case(name))
        else {
            return Err(Error(format!(
                "/CATEGORIES for {variable}: &{name} is not defined by a /PCOMPUTE subcommand"
            )));
        };
        let calc = calc(&pcompute.expr, variable, numeric, list, name)?;
        let (mut label, mut formats, mut hide_sources) = (None, Vec::new(), false);
        for props in &command.pproperties {
            if !props.names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                continue;
            }
            label = props.label.clone().or(label);
            for (function, format) in &props.formats {
                formats.retain(|(f, _)| f != function);
                formats.push((*function, format_text(format)));
            }
            hide_sources = props.hide_sources.unwrap_or(hide_sources);
        }
        if hide_sources {
            calc.sources(&mut hide);
        }
        let category = &mut list.categories[place];
        category.label = label.unwrap_or_else(|| pcompute.expr.to_string());
        category.computed = Some(Computed { calc, formats });
    }
    for k in hide {
        list.categories[k].hidden = true;
    }
    Ok(())
}

/// The expression with its categories resolved against the list.
fn calc(
    expr: &PcomputeExpr,
    variable: &str,
    numeric: bool,
    list: &CategoryList,
    name: &str,
) -> Result<Calc, Error> {
    let recurse = |e: &PcomputeExpr| calc(e, variable, numeric, list, name);
    let shown = |key: &Key| {
        list.categories
            .iter()
            .position(|c| c.computed.is_none() && c.key.as_ref() == Some(key))
    };
    Ok(match expr {
        PcomputeExpr::Category(value) => {
            let key = match (value, numeric) {
                (SyntaxValue::Number(n), true) => Key::Number(*n + 0.0),
                (SyntaxValue::String(s), false) => Key::Text(s.trim_end().to_string()),
                (_, true) => {
                    return Err(Error(format!(
                        "/PCOMPUTE &{name}: {variable} is numeric, so {expr} must name a number"
                    )));
                }
                (_, false) => {
                    return Err(Error(format!(
                        "/PCOMPUTE &{name}: {variable} is a string variable, so {expr} must name a quoted string"
                    )));
                }
            };
            let Some(k) = shown(&key) else {
                return Err(Error(format!(
                    "/PCOMPUTE &{name}: the category {} is not in the explicit category list of {variable}; every category a computed category uses must be listed",
                    expr
                )));
            };
            Calc::Category(k)
        }
        PcomputeExpr::Range(low, high) => Calc::Sum(
            (0..list.categories.len())
                .filter(|&k| {
                    let c = &list.categories[k];
                    c.computed.is_none()
                        && c.key.as_ref().is_some_and(|key| in_range(key, low, high))
                })
                .collect(),
        ),
        PcomputeExpr::Total => match list.total() {
            Some(k) => Calc::Category(k),
            None => {
                return Err(Error(format!(
                    "/PCOMPUTE &{name}: TOTAL needs TOTAL=YES for {variable} in /CATEGORIES"
                )));
            }
        },
        PcomputeExpr::Number(n) => Calc::Number(*n),
        PcomputeExpr::Negate(inner) => Calc::Negate(Box::new(recurse(inner)?)),
        PcomputeExpr::Binary(op, left, right) => {
            Calc::Binary(*op, Box::new(recurse(left)?), Box::new(recurse(right)?))
        }
    })
}
