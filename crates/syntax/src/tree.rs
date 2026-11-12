//! Parser of TREE: a classification tree grown by CHAID or exhaustive CHAID.
//!
//! Rules from the Command Syntax Reference (TREE): the dependent variable, BY and the predictors,
//! each optionally followed by its level in brackets ([S], [O], [N], [C] for nominal), then
//! FORCE = var. Subcommands come in any order. Keywords are followed by '='; lists of values are
//! in brackets. CRT, QUEST, validation, custom costs, profits, rules, influence and model files
//! are refused as not supported yet; settings that only apply to CRT and QUEST are ignored with
//! a warning, as the manual says.

use crate::ast::{Command, Level, Tree, TreeCategory, TreePrint, TreeSave, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

const SUBCOMMANDS: [&str; 21] = [
    "TREE",
    "DEPCATEGORIES",
    "PRINT",
    "GAIN",
    "PLOT",
    "RULES",
    "SAVE",
    "METHOD",
    "GROWTHLIMIT",
    "VALIDATION",
    "CHAID",
    "CRT",
    "QUEST",
    "COSTS",
    "PRIORS",
    "SCORES",
    "PROFITS",
    "INFLUENCE",
    "OUTFILE",
    "MISSING",
    "TARGETRESPONSE",
];

/// `[S]`, `[O]`, `[N]` or `[C]` after a variable, if there.
fn level(p: &mut Parser) -> PResult<Option<Level>> {
    if !p.eat(Punct::LBracket) {
        return Ok(None);
    }
    let level = match p.peek() {
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("S") => Level::Scale,
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("O") => Level::Ordinal,
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("N") || w.eq_ignore_ascii_case("C") => {
            Level::Nominal
        }
        _ => return Err(p.unexpected(&["S", "O", "N", "C"])),
    };
    p.pos += 1;
    p.expect(Punct::RBracket)?;
    Ok(Some(level))
}

/// One keyword out of `choices`, after an optional '='.
fn choice(p: &mut Parser, choices: &[&str]) -> PResult<usize> {
    p.eat(Punct::Eq);
    match choices.iter().position(|c| p.at_keyword(c)) {
        Some(i) => {
            p.pos += 1;
            Ok(i)
        }
        None => Err(p.unexpected(choices)),
    }
}

fn yes(p: &mut Parser) -> PResult<bool> {
    Ok(choice(p, &["YES", "NO"])? == 0)
}

/// A number after an optional '=', within `low..=high`.
fn number(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<f64> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    if !(low..=high).contains(&n) {
        let message = format!("{what} must lie between {low} and {high}");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(n)
}

/// A positive whole number after an optional '='.
fn count(p: &mut Parser, what: &str, high: f64) -> PResult<usize> {
    let start = p.pos;
    let n = number(p, what, 1.0, high)?;
    if n.fract() != 0.0 {
        let message = format!("{what} must be a whole number");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(n as usize)
}

/// A probability: above 0 and below 1, or up to 1 when `one` allows it.
fn alpha(p: &mut Parser, what: &str, one: bool) -> PResult<f64> {
    let start = p.pos;
    let n = number(p, what, 0.0, 1.0)?;
    if n == 0.0 || (n == 1.0 && !one) {
        let bound = if one { "at most 1" } else { "below 1" };
        let message = format!("{what} must be above 0 and {bound}");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(n)
}

/// `[value value ...]`, with VALID and MISSING where `keywords` allows them.
fn values(p: &mut Parser, keywords: bool) -> PResult<Vec<TreeCategory>> {
    p.eat(Punct::Eq);
    p.expect(Punct::LBracket)?;
    let mut out = Vec::new();
    while !p.eat(Punct::RBracket) {
        if keywords && p.eat_keyword("VALID") {
            out.push(TreeCategory::Valid);
        } else if keywords && p.eat_keyword("MISSING") {
            out.push(TreeCategory::Missing);
        } else if let Some(value) = p.value() {
            out.push(TreeCategory::Value(value));
        } else {
            let mut expected = vec!["a value", "']'"];
            if keywords {
                expected.extend(["VALID", "MISSING"]);
            }
            return Err(p.unexpected(&expected));
        }
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(SyntaxError::new(p.span(), "the list of values is empty"));
    }
    Ok(out)
}

/// A name in parentheses, if there.
fn name_in_parentheses(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

/// Skips the rest of a subcommand.
fn skip(p: &mut Parser) {
    while !at_end(p) {
        p.pos += 1;
    }
}

fn not_supported(p: &Parser, start: usize, what: &str) -> SyntaxError {
    SyntaxError::new(p.span_from(start), format!("{what} is not supported yet"))
}

/// `dependent [level] BY predictor [level] ... [FORCE = var [level]]`
fn model(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    command.dependent = p.name("the dependent variable")?;
    command.dependent_level = level(p)?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("FORCE") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
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
        command.predictors.push((item, level(p)?));
        p.eat(Punct::Comma);
    }
    if command.predictors.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    if p.eat_keyword("FORCE") {
        p.expect(Punct::Eq)?;
        let name = p.name("a variable name")?;
        command.force = Some((name, level(p)?));
    }
    Ok(())
}

fn tree_subcommand(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    while !at_end(p) {
        if p.eat_keyword("DISPLAY") {
            let display = choice(p, &["TOPDOWN", "LEFTTORIGHT", "RIGHTTOLEFT", "NONE"])?;
            command.display = display != 3;
        } else if p.eat_keyword("NODES") {
            // charts in nodes are not drawn: the statistics stand for them
            choice(p, &["STATISTICS", "CHART", "BOTH"])?;
        } else if p.eat_keyword("BRANCHSTATISTICS") {
            command.branch_statistics = yes(p)?;
        } else if p.eat_keyword("NODEDEFS") {
            command.node_definitions = yes(p)?;
        } else if p.eat_keyword("SCALE") {
            // the scale of a drawing
            p.eat(Punct::Eq);
            if !p.eat_keyword("AUTO") {
                number(p, "SCALE", 5.0, 200.0)?;
            }
        } else {
            return Err(p.unexpected(&[
                "DISPLAY",
                "NODES",
                "BRANCHSTATISTICS",
                "NODEDEFS",
                "SCALE",
            ]));
        }
    }
    Ok(())
}

fn print(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut print = TreePrint {
        model_summary: false,
        classification: false,
        risk: false,
        case_summary: false,
        tree_table: false,
    };
    let (mut none, mut other) = (None, false);
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("MODELSUMMARY") {
            print.model_summary = true;
        } else if p.eat_keyword("CLASSIFICATION") {
            print.classification = true;
        } else if p.eat_keyword("RISK") {
            print.risk = true;
        } else if p.eat_keyword("CPS") {
            print.case_summary = true;
        } else if p.eat_keyword("TREETABLE") {
            print.tree_table = true;
        } else if p.eat_keyword("IMPORTANCE") || p.eat_keyword("SURROGATES") {
            let message = "PRINT IMPORTANCE and SURROGATES apply to CRT and QUEST only; ignored";
            command.ignored.push(message.into());
        } else if p.eat_keyword("CATEGORYSPECS") {
            let message = "PRINT CATEGORYSPECS is not shown yet; costs are equal and the scores are \
                           the ones /SCORES gives";
            command.ignored.push(message.into());
        } else if p.eat_keyword("NONE") {
            none = Some(start);
            continue;
        } else {
            return Err(p.unexpected(&[
                "MODELSUMMARY",
                "CLASSIFICATION",
                "RISK",
                "CPS",
                "TREETABLE",
                "IMPORTANCE",
                "SURROGATES",
                "CATEGORYSPECS",
                "NONE",
            ]));
        }
        other = true;
        p.eat(Punct::Comma);
    }
    if let (Some(start), true) = (none, other) {
        let message = "PRINT NONE cannot stand with other keywords";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    command.print = print;
    Ok(())
}

fn gain(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    while !at_end(p) {
        if p.eat_keyword("SUMMARYTABLE") {
            // the summary of a categorical dependent shows profits, which are not supported
            yes(p)?;
        } else if p.eat_keyword("CATEGORYTABLE") {
            command.gain_table = yes(p)?;
        } else if p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            p.expect(Punct::LBracket)?;
            while !p.eat(Punct::RBracket) {
                let start = p.pos;
                if p.eat_keyword("PTILE") {
                    return Err(not_supported(p, start, "GAIN TYPE PTILE"));
                }
                if !p.eat_keyword("NODE") {
                    return Err(p.unexpected(&["NODE", "PTILE", "']'"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("SORT") {
            command.gain_ascending = choice(p, &["DESCENDING", "ASCENDING"])? == 1;
        } else if p.eat_keyword("INCREMENT") {
            // applies to percentile tables only
            number(p, "INCREMENT", 1.0, 25.0)?;
        } else if p.eat_keyword("CUMULATIVE") {
            command.gain_cumulative = yes(p)?;
        } else {
            return Err(p.unexpected(&[
                "SUMMARYTABLE",
                "CATEGORYTABLE",
                "TYPE",
                "SORT",
                "INCREMENT",
                "CUMULATIVE",
            ]));
        }
    }
    Ok(())
}

/// `/PLOT`: GAIN, RESPONSE and INDEX are made; MEAN needs a scale dependent, PROFIT and ROI
/// need profits and IMPORTANCE a CRT tree, so they are ignored with a warning, as the manual
/// says. INCREMENT other than 1, 2, 5, 10, 20 or 25 keeps 10, with a warning.
fn plot(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    p.eat(Punct::Eq);
    while !at_end(p) {
        if p.eat_keyword("GAIN") {
            command.plot_gain = true;
        } else if p.eat_keyword("RESPONSE") {
            command.plot_response = true;
        } else if p.eat_keyword("INDEX") {
            command.plot_index = true;
        } else if p.eat_keyword("MEAN") {
            let message = "PLOT MEAN needs a scale dependent variable; ignored";
            command.ignored.push(message.into());
        } else if p.eat_keyword("PROFIT") || p.eat_keyword("ROI") {
            let message = "PLOT PROFIT and ROI need profits, which are not supported yet; ignored";
            command.ignored.push(message.into());
        } else if p.eat_keyword("IMPORTANCE") {
            let message = "PLOT IMPORTANCE applies to CRT trees only; ignored";
            command.ignored.push(message.into());
        } else if p.eat_keyword("INCREMENT") {
            let n = number(p, "INCREMENT", 1.0, 100.0)?;
            if [1.0, 2.0, 5.0, 10.0, 20.0, 25.0].contains(&n) {
                command.plot_increment = n as usize;
            } else {
                let message = format!("PLOT INCREMENT={n} is not 1, 2, 5, 10, 20 or 25; 10 is used");
                command.ignored.push(message);
            }
        } else {
            return Err(p.unexpected(&[
                "GAIN",
                "RESPONSE",
                "INDEX",
                "MEAN",
                "PROFIT",
                "ROI",
                "IMPORTANCE",
                "INCREMENT",
            ]));
        }
    }
    Ok(())
}

fn save(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut any = false;
    while !at_end(p) {
        let what = if p.eat_keyword("NODEID") {
            TreeSave::NodeId
        } else if p.eat_keyword("PREDVAL") {
            TreeSave::PredictedValue
        } else if p.eat_keyword("PREDPROB") {
            TreeSave::PredictedProbability
        } else if p.eat_keyword("ASSIGNMENT") {
            name_in_parentheses(p)?;
            let message = "SAVE ASSIGNMENT needs split-sample validation; ignored";
            command.ignored.push(message.into());
            any = true;
            continue;
        } else {
            return Err(p.unexpected(&["NODEID", "PREDVAL", "PREDPROB", "ASSIGNMENT"]));
        };
        let name = name_in_parentheses(p)?;
        command.save.retain(|(w, _)| *w != what);
        command.save.push((what, name));
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["NODEID", "PREDVAL", "PREDPROB", "ASSIGNMENT"]));
    }
    Ok(())
}

fn method(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("TYPE") {
            match choice(p, &["CHAID", "EXHAUSTIVECHAID", "CRT", "QUEST"])? {
                0 => command.exhaustive = false,
                1 => command.exhaustive = true,
                2 => return Err(not_supported(p, start, "METHOD TYPE=CRT")),
                _ => return Err(not_supported(p, start, "METHOD TYPE=QUEST")),
            }
        } else if p.eat_keyword("MAXSURROGATES") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("AUTO") {
                number(p, "MAXSURROGATES", 0.0, f64::MAX)?;
            }
            let message = "METHOD MAXSURROGATES applies to CRT and QUEST only; ignored";
            command.ignored.push(message.into());
        } else if p.eat_keyword("PRUNE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("SE") {
                if p.eat(Punct::LParen) {
                    number(p, "SE", 0.0, f64::MAX)?;
                    p.expect(Punct::RParen)?;
                }
                let message = "METHOD PRUNE applies to CRT and QUEST only; ignored";
                command.ignored.push(message.into());
            } else if !p.eat_keyword("NONE") {
                return Err(p.unexpected(&["NONE", "SE"]));
            }
        } else {
            return Err(p.unexpected(&["TYPE", "MAXSURROGATES", "PRUNE"]));
        }
    }
    Ok(())
}

fn growth_limit(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    while !at_end(p) {
        if p.eat_keyword("MAXDEPTH") {
            p.eat(Punct::Eq);
            command.max_depth = if p.eat_keyword("AUTO") {
                None
            } else {
                Some(count(p, "MAXDEPTH", f64::MAX)?)
            };
        } else if p.eat_keyword("MINPARENTSIZE") {
            command.min_parent = count(p, "MINPARENTSIZE", f64::MAX)? as f64;
        } else if p.eat_keyword("MINCHILDSIZE") {
            command.min_child = count(p, "MINCHILDSIZE", f64::MAX)? as f64;
        } else if p.eat_keyword("MINIMPROVEMENT") {
            number(p, "MINIMPROVEMENT", 0.0, f64::MAX)?;
            let message = "GROWTHLIMIT MINIMPROVEMENT applies to CRT only; ignored";
            command.ignored.push(message.into());
        } else {
            return Err(p.unexpected(&[
                "MAXDEPTH",
                "MINPARENTSIZE",
                "MINCHILDSIZE",
                "MINIMPROVEMENT",
            ]));
        }
    }
    Ok(())
}

fn validation(p: &mut Parser) -> PResult<()> {
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("TYPE") {
            match choice(p, &["NONE", "SPLITSAMPLE", "CROSSVALIDATION"])? {
                0 => {}
                1 => return Err(not_supported(p, start, "VALIDATION TYPE=SPLITSAMPLE")),
                _ => return Err(not_supported(p, start, "VALIDATION TYPE=CROSSVALIDATION")),
            }
        } else if p.eat_keyword("OUTPUT") {
            // applies to split-sample validation only
            choice(p, &["BOTHSAMPLES", "TESTSAMPLE"])?;
        } else {
            return Err(p.unexpected(&["TYPE", "OUTPUT"]));
        }
    }
    Ok(())
}

const CHAID_KEYWORDS: [&str; 8] = [
    "ALPHASPLIT",
    "ALPHAMERGE",
    "SPLITMERGED",
    "CHISQUARE",
    "CONVERGE",
    "MAXITERATIONS",
    "ADJUST",
    "INTERVALS",
];

fn chaid(p: &mut Parser, command: &mut Tree) -> PResult<()> {
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("ALPHASPLIT") {
            command.alpha_split = alpha(p, "ALPHASPLIT", false)?;
        } else if p.eat_keyword("ALPHAMERGE") {
            command.alpha_merge = alpha(p, "ALPHAMERGE", true)?;
        } else if p.eat_keyword("SPLITMERGED") {
            if yes(p)? {
                return Err(not_supported(p, start, "CHAID SPLITMERGED=YES"));
            }
        } else if p.eat_keyword("CHISQUARE") {
            command.likelihood_ratio = choice(p, &["PEARSON", "LR"])? == 1;
        } else if p.eat_keyword("CONVERGE") {
            command.converge = alpha(p, "CONVERGE", false)?;
        } else if p.eat_keyword("MAXITERATIONS") {
            command.max_iterations = count(p, "MAXITERATIONS", f64::MAX)?;
        } else if p.eat_keyword("ADJUST") {
            command.bonferroni = choice(p, &["BONFERRONI", "NONE"])? == 0;
        } else if p.eat_keyword("INTERVALS") {
            p.eat(Punct::Eq);
            if matches!(p.peek(), Some(TokenKind::Number(_))) {
                command.intervals = count(p, "INTERVALS", 64.0)?;
                continue;
            }
            // varlist (n) varlist (n) ..., up to the next keyword of the subcommand
            let keyword = |p: &Parser| CHAID_KEYWORDS.iter().any(|k| p.at_keyword(k));
            while matches!(p.peek(), Some(TokenKind::Word(_))) && !keyword(p) {
                let list = p.var_list()?;
                p.expect(Punct::LParen)?;
                let n = count(p, "INTERVALS", 64.0)?;
                p.expect(Punct::RParen)?;
                command.variable_intervals.push((list, n));
            }
        } else {
            return Err(p.unexpected(&CHAID_KEYWORDS));
        }
    }
    Ok(())
}

/// `value [score] value [score] ...` after CUSTOM.
fn custom_scores(p: &mut Parser) -> PResult<Vec<(Value, f64)>> {
    p.eat(Punct::Eq);
    let mut scores: Vec<(Value, f64)> = Vec::new();
    while !at_end(p) {
        let Some(value) = p.value() else {
            return Err(p.unexpected(&["a value"]));
        };
        p.expect(Punct::LBracket)?;
        let Some(score) = p.number() else {
            return Err(p.unexpected(&["a score"]));
        };
        p.expect(Punct::RBracket)?;
        // the last score of a category wins
        scores.retain(|(v, _)| *v != value);
        scores.push((value, score));
    }
    if scores.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    Ok(scores)
}

pub(crate) fn tree(p: &mut Parser) -> PResult<Command> {
    let mut command = Tree {
        dependent: String::new(),
        dependent_level: None,
        predictors: Vec::new(),
        force: None,
        exhaustive: false,
        display: true,
        branch_statistics: true,
        node_definitions: true,
        use_values: None,
        targets: Vec::new(),
        print: TreePrint {
            model_summary: true,
            classification: true,
            risk: true,
            case_summary: true,
            tree_table: false,
        },
        gain_table: true,
        gain_cumulative: true,
        gain_ascending: false,
        plot_gain: false,
        plot_response: false,
        plot_index: false,
        plot_increment: 10,
        save: Vec::new(),
        max_depth: None,
        min_parent: 100.0,
        min_child: 50.0,
        alpha_split: 0.05,
        alpha_merge: 0.05,
        likelihood_ratio: false,
        converge: 0.001,
        max_iterations: 100,
        bonferroni: true,
        intervals: 10,
        variable_intervals: Vec::new(),
        scores: None,
        nominal_missing_valid: false,
        ignored: Vec::new(),
    };
    model(p, &mut command)?;
    let mut seen: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let Some(&name) = SUBCOMMANDS.iter().find(|s| p.at_keyword(s)) else {
            return Err(p.subcommand_error("TREE", &[]));
        };
        if seen.contains(&name) {
            let message = format!("subcommand {name} is given twice");
            return Err(SyntaxError::new(p.span(), message));
        }
        seen.push(name);
        p.pos += 1;
        match name {
            "TREE" => tree_subcommand(p, &mut command)?,
            "DEPCATEGORIES" => {
                while !at_end(p) {
                    if p.eat_keyword("USEVALUES") {
                        command.use_values = Some(values(p, true)?);
                    } else if p.eat_keyword("TARGET") {
                        command.targets = values(p, false)?
                            .into_iter()
                            .filter_map(|v| match v {
                                TreeCategory::Value(v) => Some(v),
                                _ => None,
                            })
                            .collect();
                    } else {
                        return Err(p.unexpected(&["USEVALUES", "TARGET"]));
                    }
                }
            }
            "PRINT" => print(p, &mut command)?,
            "GAIN" => gain(p, &mut command)?,
            "PLOT" => plot(p, &mut command)?,
            "SAVE" => save(p, &mut command)?,
            "METHOD" => method(p, &mut command)?,
            "GROWTHLIMIT" => growth_limit(p, &mut command)?,
            "VALIDATION" => validation(p)?,
            "CHAID" => chaid(p, &mut command)?,
            "CRT" | "QUEST" | "PRIORS" => {
                skip(p);
                let message = format!("{name} applies to CRT and QUEST trees only; ignored");
                command.ignored.push(message);
            }
            "COSTS" => {
                p.eat(Punct::Eq);
                if p.eat_keyword("CUSTOM") {
                    return Err(not_supported(p, start, "COSTS CUSTOM"));
                }
                if !p.eat_keyword("EQUAL") {
                    return Err(p.unexpected(&["EQUAL", "CUSTOM"]));
                }
            }
            "SCORES" => {
                p.eat(Punct::Eq);
                if p.eat_keyword("CUSTOM") {
                    command.scores = Some(custom_scores(p)?);
                } else if !p.eat_keyword("EQUALINCREMENTS") {
                    return Err(p.unexpected(&["EQUALINCREMENTS", "CUSTOM"]));
                }
            }
            "MISSING" => {
                if !p.eat_keyword("NOMINALMISSING") {
                    return Err(p.unexpected(&["NOMINALMISSING"]));
                }
                command.nominal_missing_valid = choice(p, &["MISSING", "VALID"])? == 1;
            }
            other => {
                let message = format!("subcommand {other} of TREE is not supported yet");
                return Err(SyntaxError::new(p.span_from(start), message));
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.min_child >= command.min_parent {
        let message = "GROWTHLIMIT MINCHILDSIZE must be less than MINPARENTSIZE";
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(Command::Tree(Box::new(command)))
}
