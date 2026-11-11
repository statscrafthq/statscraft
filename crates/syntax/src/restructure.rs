//! Parsers of the commands that restructure the active dataset: VARSTOCASES, CASESTOVARS, FLIP
//! and SORT VARIABLES.

use crate::ast::{
    CasesToVars, Command, Flip, IndexKind, NewVariable, Subset, VariableKey, VarsToCases,
    VarsToCasesIndex,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

const VARIABLE_KEYS: [(&str, VariableKey); 10] = [
    ("NAME", VariableKey::Name),
    ("TYPE", VariableKey::Type),
    ("FORMAT", VariableKey::Format),
    ("LABEL", VariableKey::Label),
    ("VALUES", VariableKey::Values),
    ("MISSING", VariableKey::Missing),
    ("MEASURE", VariableKey::Measure),
    ("ROLE", VariableKey::Role),
    ("COLUMNS", VariableKey::Columns),
    ("ALIGNMENT", VariableKey::Alignment),
];

pub(crate) fn sort_variables(p: &mut Parser) -> PResult<Command> {
    p.eat_word("BY");
    let key = if p.eat_keyword("ATTRIBUTE") {
        VariableKey::Attribute(p.name("an attribute name")?)
    } else {
        let known = VARIABLE_KEYS.iter().find(|(word, _)| p.at_keyword(word));
        let Some((_, key)) = known else {
            let mut expected: Vec<&str> = VARIABLE_KEYS.iter().map(|(word, _)| *word).collect();
            expected.push("ATTRIBUTE name");
            return Err(p.unexpected(&expected));
        };
        p.pos += 1;
        key.clone()
    };
    let mut descending = false;
    if p.eat(Punct::LParen) {
        if p.eat_word("D") || p.eat_keyword("DOWN") {
            descending = true;
        } else if !(p.eat_word("A") || p.eat_keyword("UP")) {
            return Err(p.unexpected(&["A", "D"]));
        }
        p.expect(Punct::RParen)?;
    }
    p.finish(&["'('"])?;
    Ok(Command::SortVariables { key, descending })
}

pub(crate) fn flip(p: &mut Parser) -> PResult<Command> {
    let mut flip = Flip {
        variables: None,
        newnames: None,
    };
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            flip.variables = Some(p.var_list()?);
        } else if p.eat_keyword("NEWNAMES") {
            p.eat(Punct::Eq);
            flip.newnames = Some(p.name("a variable name")?);
        } else {
            return Err(p.unexpected(&["VARIABLES", "NEWNAMES", "end of command"]));
        }
    }
    Ok(Command::Flip(flip))
}

/// `name ["label"]`
fn new_variable(p: &mut Parser) -> PResult<NewVariable> {
    let name = p.name("a new variable name")?;
    let label = match p.peek() {
        Some(TokenKind::Str(_)) => Some(p.string("a quoted label")?),
        _ => None,
    };
    Ok(NewVariable { name, label })
}

/// Error for a subcommand given a second time where the command allows one.
fn twice(p: &Parser, start: usize, command: &str, name: &str) -> SyntaxError {
    let message = format!("{command} takes /{name} only once");
    SyntaxError::new(p.span_from(start), message)
}

/// `/INDEX = i ["label"] [(n) | (make)] ...`
fn vars_to_cases_index(p: &mut Parser) -> PResult<Vec<VarsToCasesIndex>> {
    let mut index = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let start = p.pos;
        let variable = new_variable(p)?;
        let kind = if p.eat(Punct::LParen) {
            let kind = match p.peek() {
                Some(TokenKind::Number(n)) if *n >= 1.0 && n.fract() == 0.0 => {
                    let levels = *n as usize;
                    p.pos += 1;
                    IndexKind::Levels(levels)
                }
                Some(TokenKind::Word(_)) => IndexKind::Names(p.name("a MAKE variable")?),
                _ => return Err(p.unexpected(&["a number of levels", "a MAKE variable"])),
            };
            p.expect(Punct::RParen)?;
            kind
        } else {
            IndexKind::Sequence
        };
        if !index.is_empty() && !matches!(kind, IndexKind::Levels(_)) {
            let message =
                "several INDEX variables each need their number of levels, as in a(2) b(3)";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        index.push(VarsToCasesIndex { variable, kind });
    }
    if index.is_empty() {
        return Err(p.unexpected(&["a new variable name"]));
    }
    if index.len() > 1 && !matches!(index[0].kind, IndexKind::Levels(_)) {
        let message = "several INDEX variables each need their number of levels, as in a(2) b(3)";
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(index)
}

pub(crate) fn vars_to_cases(p: &mut Parser) -> PResult<Command> {
    const COMMAND: &str = "VARSTOCASES";
    let mut command = VarsToCases {
        makes: Vec::new(),
        index: Vec::new(),
        id: None,
        keep_null: false,
        count: None,
        subset: None,
    };
    let mut null = None;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("MAKE") {
            p.eat(Punct::Eq);
            let variable = new_variable(p)?;
            p.eat_keyword("FROM");
            command.makes.push((variable, p.var_list()?));
        } else if p.eat_keyword("INDEX") {
            p.eat(Punct::Eq);
            if !command.index.is_empty() {
                return Err(twice(p, start, COMMAND, "INDEX"));
            }
            command.index = vars_to_cases_index(p)?;
        } else if p.eat_keyword("ID") {
            if command.id.is_some() {
                return Err(twice(p, start, COMMAND, "ID"));
            }
            p.eat(Punct::Eq);
            command.id = Some(new_variable(p)?);
        } else if p.eat_keyword("COUNT") {
            if command.count.is_some() {
                return Err(twice(p, start, COMMAND, "COUNT"));
            }
            p.eat(Punct::Eq);
            command.count = Some(new_variable(p)?);
        } else if p.eat_keyword("NULL") {
            if null.is_some() {
                return Err(twice(p, start, COMMAND, "NULL"));
            }
            p.eat(Punct::Eq);
            if p.eat_keyword("KEEP") {
                command.keep_null = true;
            } else if !p.eat_keyword("DROP") {
                return Err(p.unexpected(&["DROP", "KEEP"]));
            }
            null = Some(());
        } else if p.at_keyword("KEEP") || p.at_keyword("DROP") {
            let keep = p.at_keyword("KEEP");
            p.pos += 1;
            if command.subset.is_some() {
                let message = "VARSTOCASES takes one /KEEP or one /DROP, not both or two";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            p.eat(Punct::Eq);
            let vars = p.var_list()?;
            command.subset = Some(if keep {
                Subset::Keep(vars)
            } else {
                Subset::Drop(vars)
            });
        } else {
            return Err(p.unexpected(&["MAKE", "INDEX", "ID", "NULL", "COUNT", "KEEP", "DROP"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.makes.is_empty() {
        let mut error = SyntaxError::new(p.span(), "VARSTOCASES needs at least one /MAKE");
        error.expected = vec!["/MAKE new FROM varlist".to_string()];
        return Err(error);
    }
    Ok(Command::VarsToCases(command))
}

pub(crate) fn cases_to_vars(p: &mut Parser) -> PResult<Command> {
    const COMMAND: &str = "CASESTOVARS";
    let mut command = CasesToVars {
        id: Vec::new(),
        index: Vec::new(),
        fixed: Vec::new(),
        autofix: true,
        indicators: None,
        count: None,
        rename: Vec::new(),
        separator: ".".to_string(),
        group_by_index: false,
        drop: Vec::new(),
    };
    let mut seen: Vec<&str> = Vec::new();
    const SUBCOMMANDS: [&str; 10] = [
        "ID",
        "INDEX",
        "FIXED",
        "AUTOFIX",
        "VIND",
        "COUNT",
        "RENAME",
        "SEPARATOR",
        "GROUPBY",
        "DROP",
    ];
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let Some(&name) = SUBCOMMANDS.iter().find(|name| p.at_keyword(name)) else {
            return Err(p.unexpected(&SUBCOMMANDS));
        };
        p.pos += 1;
        if seen.contains(&name) {
            return Err(twice(p, start, COMMAND, name));
        }
        seen.push(name);
        p.eat(Punct::Eq);
        match name {
            "ID" => command.id = p.var_list()?,
            "INDEX" => command.index = p.var_list()?,
            "FIXED" => command.fixed = p.var_list()?,
            "DROP" => command.drop = p.var_list()?,
            "AUTOFIX" => {
                if p.eat_keyword("NO") {
                    command.autofix = false;
                } else if !p.eat_keyword("YES") {
                    return Err(p.unexpected(&["YES", "NO"]));
                }
            }
            "VIND" => {
                let root = if p.eat_keyword("ROOT") {
                    p.eat(Punct::Eq);
                    p.name("a root name")?
                } else {
                    "ind".to_string()
                };
                command.indicators = Some(root);
            }
            "COUNT" => command.count = Some(new_variable(p)?),
            "RENAME" => {
                while matches!(p.peek(), Some(TokenKind::Word(_))) {
                    let variable = p.name("a variable name")?;
                    p.expect(Punct::Eq)?;
                    command.rename.push((variable, p.name("a root name")?));
                }
                if command.rename.is_empty() {
                    return Err(p.unexpected(&["variable = root"]));
                }
            }
            "SEPARATOR" => command.separator = p.string("a quoted separator")?,
            _ => {
                if p.eat_keyword("INDEX") {
                    command.group_by_index = true;
                } else if !p.eat_keyword("VARIABLE") {
                    return Err(p.unexpected(&["VARIABLE", "INDEX"]));
                }
            }
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::CasesToVars(command))
}
