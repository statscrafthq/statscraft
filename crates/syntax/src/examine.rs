//! Parser of EXAMINE: descriptives, percentiles, M-estimators, extreme values and tests of
//! normality of dependent variables, overall and within the cells of factors.

use crate::ast::{Command, Examine, MEstimator, PercentileMethod, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error, keywords, names};
use crate::stats::variables;

const DEFAULT_PERCENTILES: [f64; 7] = [5.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0];

const METHODS: [(&str, &[PercentileMethod]); 5] = [
    ("HAVERAGE", &[PercentileMethod::Haverage]),
    ("WAVERAGE", &[PercentileMethod::Waverage]),
    ("ROUND", &[PercentileMethod::Round]),
    ("AEMPIRICAL", &[PercentileMethod::Aempirical]),
    ("EMPIRICAL", &[PercentileMethod::Empirical]),
];

/// `(number, ...)` with exactly `count` numbers, or the defaults when no parenthesis follows.
fn constants(p: &mut Parser, defaults: &[f64]) -> PResult<Vec<f64>> {
    if !p.eat(Punct::LParen) {
        return Ok(defaults.to_vec());
    }
    let start = p.pos - 1;
    let mut values = Vec::new();
    while let Some(n) = p.number() {
        values.push(n);
        p.eat(Punct::Comma);
    }
    p.expect(Punct::RParen)?;
    if values.len() != defaults.len() {
        let message = format!(
            "this M-estimator takes {} constant{} in parentheses",
            defaults.len(),
            if defaults.len() == 1 { "" } else { "s" }
        );
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(values)
}

fn mestimators(p: &mut Parser) -> PResult<Vec<MEstimator>> {
    let all = vec![
        MEstimator::Huber(1.339),
        MEstimator::Tukey(4.685),
        MEstimator::Hampel(1.7, 3.4, 8.5),
        MEstimator::Andrew(1.34),
    ];
    if !p.eat(Punct::Eq) && at_end(p) {
        return Ok(all);
    }
    let mut chosen = Vec::new();
    loop {
        let start = p.pos;
        let estimator = if p.eat_keyword("HUBER") {
            MEstimator::Huber(constants(p, &[1.339])?[0])
        } else if p.eat_keyword("TUKEY") {
            MEstimator::Tukey(constants(p, &[4.685])?[0])
        } else if p.eat_keyword("HAMPEL") {
            let c = constants(p, &[1.7, 3.4, 8.5])?;
            if !(0.0 < c[0] && c[0] <= c[1] && c[1] <= c[2]) {
                let message = "Hampel's constants need 0 < a <= b <= c";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            MEstimator::Hampel(c[0], c[1], c[2])
        } else if p.eat_keyword("ANDREW") {
            MEstimator::Andrew(constants(p, &[1.34])?[0])
        } else if p.at_keyword("NONE")
            || matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("ALL"))
        {
            // ALL replaces what came before with the four defaults, NONE with nothing
            chosen = if p.eat_word("ALL") {
                all.clone()
            } else {
                p.pos += 1;
                Vec::new()
            };
            p.eat(Punct::Comma);
            if at_end(p) {
                break;
            }
            continue;
        } else {
            return Err(p.unexpected(&["HUBER", "ANDREW", "HAMPEL", "TUKEY", "ALL", "NONE"]));
        };
        let constant = match estimator {
            MEstimator::Huber(c) | MEstimator::Tukey(c) | MEstimator::Andrew(c) => c,
            MEstimator::Hampel(a, ..) => a,
        };
        if constant <= 0.0 {
            let message = "the constant of an M-estimator must be positive";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        // the last specification of an estimator wins
        chosen.retain(|c| std::mem::discriminant(c) != std::mem::discriminant(&estimator));
        chosen.push(estimator);
        p.eat(Punct::Comma);
        if at_end(p) {
            break;
        }
    }
    // the table shows them in a fixed order
    chosen.sort_by_key(|c| match c {
        MEstimator::Huber(_) => 0,
        MEstimator::Tukey(_) => 1,
        MEstimator::Hampel(..) => 2,
        MEstimator::Andrew(_) => 3,
    });
    Ok(chosen)
}

/// `dependents [BY term term ...]`, where a term is a factor or two factors joined by BY.
fn variables_subcommand(p: &mut Parser, command: &mut Examine) -> PResult<()> {
    command.dependents.extend(variables(p)?);
    if !p.eat_word("BY") {
        return Ok(());
    }
    loop {
        let start = p.pos;
        let mut term = Vec::new();
        loop {
            term.push(if p.eat_word("ALL") {
                VarItem::All
            } else {
                let first = p.name("a factor variable")?;
                if p.eat_word("TO") {
                    VarItem::Range(first, p.name("a variable name")?)
                } else {
                    VarItem::Name(first)
                }
            });
            if !p.eat_word("BY") {
                break;
            }
        }
        if term.len() > 2 {
            let message = "more than two crossed factors are not supported yet";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        if term.len() == 2 && term.iter().any(|item| !matches!(item, VarItem::Name(_))) {
            let message = "crossed factors are single variable names";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        command.factors.push(term);
        p.eat(Punct::Comma);
        if !matches!(p.peek(), Some(TokenKind::Word(_))) {
            return Ok(());
        }
    }
}

fn percentiles(p: &mut Parser, command: &mut Examine) -> PResult<()> {
    let mut list = DEFAULT_PERCENTILES.to_vec();
    if p.eat(Punct::LParen) {
        let start = p.pos;
        list.clear();
        while let Some(n) = p.number() {
            if !(n > 0.0 && n < 100.0) {
                let message = "a percentile must lie between 0 and 100";
                return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
            }
            list.push(n);
            p.eat(Punct::Comma);
        }
        if list.is_empty() {
            return Err(SyntaxError::new(
                p.span_from(start),
                "a percentile list is empty",
            ));
        }
        p.expect(Punct::RParen)?;
        list.sort_by(f64::total_cmp);
        list.dedup();
    }
    command.percentiles = Some(list);
    if p.eat(Punct::Eq) || !at_end(p) {
        if p.eat_keyword("NONE") {
            command.percentiles = None;
        } else {
            let mut choices = METHODS.to_vec();
            choices.push(("NONE", &[]));
            let chosen = keywords(p, &METHODS, |p| p.unexpected(&names(&choices)))?;
            command.method = *chosen.last().expect("at least one keyword");
        }
    }
    Ok(())
}

pub(crate) fn examine(p: &mut Parser) -> PResult<Command> {
    let mut command = Examine {
        dependents: Vec::new(),
        factors: Vec::new(),
        total: true,
        id: None,
        percentiles: None,
        method: PercentileMethod::Haverage,
        descriptives: true,
        extreme: None,
        cinterval: 95.0,
        mestimators: Vec::new(),
        normality: false,
        pairwise: false,
        include: false,
        report: false,
    };
    p.eat(Punct::Slash);
    let mut more = true;
    // a leading word without '=' starts the variable list, as in `EXAMINE age BY region`
    if !p.at_keyword("VARIABLES") && p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq)) {
        variables_subcommand(p, &mut command)?;
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            variables_subcommand(p, &mut command)?;
        } else if p.eat_keyword("COMPARE") {
            // it arranges boxplots, which are not drawn
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 2] = [("GROUPS", &[0]), ("VARIABLES", &[1])];
            keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?;
        } else if p.eat_keyword("TOTAL") {
            command.total = true;
        } else if p.eat_keyword("NOTOTAL") {
            command.total = false;
        } else if p.eat_keyword("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("PERCENTILES") {
            percentiles(p, &mut command)?;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[bool]); 2] = [("NPPLOT", &[true]), ("NONE", &[false])];
            const PLOTS: [&str; 5] = ["STEMLEAF", "BOXPLOT", "HISTOGRAM", "SPREADLEVEL", "ALL"];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(p, "EXAMINE /PLOT", &PLOTS, &names(&CHOICES))
            })?;
            command.normality = chosen.last() == Some(&true);
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            let (mut descriptives, mut extreme) = (false, None);
            let mut any = false;
            while !at_end(p) {
                if p.eat_keyword("DESCRIPTIVES") {
                    descriptives = true;
                } else if p.eat_keyword("EXTREME") {
                    let mut n = 5;
                    if p.eat(Punct::LParen) {
                        let start = p.pos;
                        n = match p.number() {
                            Some(v) if v >= 1.0 && v.fract() == 0.0 => v as usize,
                            _ => {
                                let message = "EXTREME takes a positive whole number";
                                return Err(SyntaxError::new(p.span_from(start), message));
                            }
                        };
                        p.expect(Punct::RParen)?;
                    }
                    extreme = Some(n);
                } else if p.eat_word("ALL") {
                    descriptives = true;
                    extreme = extreme.or(Some(5));
                } else if p.eat_keyword("NONE") {
                    (descriptives, extreme) = (false, None);
                } else {
                    return Err(p.unexpected(&["DESCRIPTIVES", "EXTREME", "ALL", "NONE"]));
                }
                any = true;
                p.eat(Punct::Comma);
            }
            if !any {
                return Err(p.unexpected(&["DESCRIPTIVES", "EXTREME", "ALL", "NONE"]));
            }
            (command.descriptives, command.extreme) = (descriptives, extreme);
        } else if p.eat_keyword("CINTERVAL") {
            p.eat(Punct::Eq);
            command.cinterval = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
        } else if p.eat_keyword("MESTIMATORS") {
            command.mestimators = mestimators(p)?;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 6] = [
                ("LISTWISE", &[0]),
                ("PAIRWISE", &[1]),
                ("EXCLUDE", &[2]),
                ("INCLUDE", &[3]),
                ("NOREPORT", &[4]),
                ("REPORT", &[5]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 | 1 => command.pairwise = choice == 1,
                    2 | 3 => command.include = choice == 3,
                    _ => command.report = choice == 5,
                }
            }
        } else {
            return Err(p.subcommand_error("EXAMINE", &[]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "BY"])?;
    if command.dependents.is_empty() {
        return Err(p.unexpected(&["VARIABLES=dependents"]));
    }
    Ok(Command::Examine(command))
}
