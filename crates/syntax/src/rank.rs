//! Parser of RANK: ranks, normal and Savage scores and related functions as new variables.
//!
//! Rules from the Command Syntax Reference (RANK):
//! - VARIABLES comes first: `varlist [(A | D)] ... [BY varlist]`; A or D applies to the
//!   variables before it back to the previous A or D. The keyword VARIABLES= may be left out.
//! - Function subcommands RANK (the default when there is none), RFRACTION, PERCENT,
//!   PROPORTION, N, NTILES(k), SAVAGE and NORMAL, each at most once, each with an optional
//!   `INTO names`.
//! - TIES=MEAN | LOW | HIGH | CONDENSE, FRACTION=BLOM | RANKIT | TUKEY | VW (given at most
//!   once), PRINT=YES | NO, MISSING=EXCLUDE | INCLUDE.

use crate::ast::{Command, Rank, RankFraction, RankFunction, RankTies, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, names};

const FUNCTIONS: [(&str, RankFunction); 8] = [
    ("RANK", RankFunction::Rank),
    ("RFRACTION", RankFunction::Rfraction),
    ("PERCENT", RankFunction::Percent),
    ("PROPORTION", RankFunction::Proportion),
    ("N", RankFunction::N),
    ("NTILES", RankFunction::Ntiles(0)),
    ("SAVAGE", RankFunction::Savage),
    ("NORMAL", RankFunction::Normal),
];

/// One keyword of `choices` after an optional '='.
fn choice<T: Copy>(p: &mut Parser, choices: &[(&'static str, T)]) -> PResult<T> {
    p.eat(Punct::Eq);
    let Some((_, value)) = choices.iter().find(|(k, _)| p.at_keyword(k)) else {
        return Err(p.unexpected(&names(choices)));
    };
    p.pos += 1;
    if !at_end(p) {
        return Err(p.unexpected(&["'/'", "end of command"]));
    }
    Ok(*value)
}

/// `varlist [(A | D)] ... [BY varlist]`
fn variables(p: &mut Parser, command: &mut Rank) -> PResult<()> {
    let mut pending = 0;
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("BY") {
            break;
        }
        let item = if p.eat_word("ALL") {
            VarItem::All
        } else {
            let first = p.name("a variable name")?;
            if p.eat_word("TO") {
                VarItem::Range(first, p.name("a variable name")?)
            } else {
                VarItem::Name(first)
            }
        };
        command.variables.push((item, false));
        pending += 1;
        p.eat(Punct::Comma);
        if p.eat(Punct::LParen) {
            let descending = if p.eat_word("D") {
                true
            } else if p.eat_word("A") {
                false
            } else {
                return Err(p.unexpected(&["A", "D"]));
            };
            p.expect(Punct::RParen)?;
            let count = command.variables.len();
            for (_, d) in &mut command.variables[count - pending..] {
                *d = descending;
            }
            pending = 0;
            p.eat(Punct::Comma);
        }
    }
    if command.variables.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    if p.eat_word("BY") {
        command.by = p.var_list()?;
    }
    Ok(())
}

pub(crate) fn rank(p: &mut Parser) -> PResult<Command> {
    let mut command = Rank {
        variables: Vec::new(),
        by: Vec::new(),
        functions: Vec::new(),
        ties: RankTies::Mean,
        fraction: None,
        print: true,
        include: false,
    };
    p.eat(Punct::Slash);
    // VARIABLES= may be left out, as in `RANK salary.`
    if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        p.pos += 2;
    }
    variables(p, &mut command)?;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if let Some((name, function)) = FUNCTIONS.iter().find(|(k, _)| p.at_keyword(k)) {
            p.pos += 1;
            let mut function = *function;
            if function == RankFunction::Ntiles(0) {
                p.eat(Punct::Eq);
                let groups = if p.eat(Punct::LParen) {
                    let value = p.number();
                    p.expect(Punct::RParen)?;
                    value
                } else {
                    None
                };
                match groups {
                    Some(k) if k >= 1.0 && k.fract() == 0.0 && k < 1e9 => {
                        function = RankFunction::Ntiles(k as u32)
                    }
                    _ => {
                        let message =
                            "NTILES takes the number of groups in parentheses, as in NTILES(4)";
                        return Err(SyntaxError::new(p.span_from(start), message));
                    }
                }
            }
            let same =
                |f: &RankFunction| std::mem::discriminant(f) == std::mem::discriminant(&function);
            if command.functions.iter().any(|(f, _)| same(f)) {
                let message =
                    format!("the function {name} is given twice; give each function once");
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            let mut into = Vec::new();
            if p.eat_keyword("INTO") {
                while matches!(p.peek(), Some(TokenKind::Word(_))) {
                    into.push(p.name("a new variable name")?);
                    p.eat(Punct::Comma);
                }
                if into.is_empty() {
                    return Err(p.unexpected(&["a new variable name"]));
                }
            }
            if !at_end(p) {
                return Err(p.unexpected(&["INTO", "'/'", "end of command"]));
            }
            command.functions.push((function, into));
        } else if p.eat_keyword("TIES") {
            const TIES: [(&str, RankTies); 4] = [
                ("MEAN", RankTies::Mean),
                ("LOW", RankTies::Low),
                ("HIGH", RankTies::High),
                ("CONDENSE", RankTies::Condense),
            ];
            command.ties = choice(p, &TIES)?;
        } else if p.eat_keyword("FRACTION") {
            const FRACTIONS: [(&str, RankFraction); 4] = [
                ("BLOM", RankFraction::Blom),
                ("RANKIT", RankFraction::Rankit),
                ("TUKEY", RankFraction::Tukey),
                ("VW", RankFraction::Vw),
            ];
            if command.fraction.is_some() {
                let message = "FRACTION is given twice; give one formula";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            command.fraction = Some(choice(p, &FRACTIONS)?);
        } else if p.eat_keyword("PRINT") {
            command.print = choice(p, &[("YES", true), ("NO", false)])?;
        } else if p.eat_keyword("MISSING") {
            command.include = choice(p, &[("EXCLUDE", false), ("INCLUDE", true)])?;
        } else {
            return Err(p.subcommand_error("RANK", &[]));
        }
    }
    p.finish(&["'/'"])?;
    if command.functions.is_empty() {
        command.functions.push((RankFunction::Rank, Vec::new()));
    }
    Ok(Command::Rank(command))
}
