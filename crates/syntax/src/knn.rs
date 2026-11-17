//! Parser of KNN, nearest neighbor analysis.
//!
//! Rules, in our words, from the Command Syntax Reference:
//! - `KNN [target [(MLEVEL=S|O|N)]] [BY factors] [WITH covariates]`, then subcommands in any
//!   order, each once. Command, subcommand and keyword names are written in full, the
//!   parentheses and equals signs are required, a keyword appears once within its subcommand
//!   and an empty subcommand is an error.
//! - BY or WITH with no names is ignored with a warning (the engine gives it). At least one
//!   factor or covariate is needed.
//! - EXCEPT VARIABLES=list; CASELABELS VARIABLE=name; FOCALCASES VARIABLE=name; RESCALE
//!   COVARIATE=ADJNORMALIZED|NONE; PARTITION TRAINING=n HOLDOUT=n | VARIABLE=name (TRAINING a
//!   whole number above 0, HOLDOUT one of at least 0, VARIABLE not with either).
//! - MODEL METRIC=EUCLID|CITYBLOCK NEIGHBORS=FIXED[(K=n)] | AUTO[(KMIN=a, KMAX=b)] (both or
//!   neither, KMIN below KMAX) FEATURES=ALL | AUTO[(FORCE=list)].
//! - CRITERIA NUMFEATURES=AUTO | FIXED(n) | ERRORRATIO[(MINCHANGE=v)] PREDICTED=MEAN|MEDIAN
//!   WEIGHTFEATURES=NO|YES. CROSSVALIDATION FOLDS=n (above 1) | VARIABLE=name. MISSING
//!   USERMISSING=EXCLUDE|INCLUDE. VIEWMODEL DISPLAY=YES|NO. PRINT CPS | NONE (NONE alone).
//! - SAVE PREDVAL[(name)] PREDPROB[(root)] PARTITION[(name)] FOLD[(name)] MAXCAT(n).
//!   OUTFILE MODEL='file' FOCALCASES='file' or dataset.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnnLevel {
    Scale,
    Ordinal,
    Nominal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnnNeighbors {
    Fixed(usize),
    Auto { min: usize, max: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnnFeatureCount {
    /// min(20, predictors) less the forced ones.
    Auto,
    Fixed(usize),
    /// Stop when the error changes by less than this share.
    ErrorRatio(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum KnnPartition {
    /// Relative sizes of the random training and holdout samples.
    Ratio { training: f64, holdout: f64 },
    /// Positive values train, others hold out.
    Variable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum KnnFolds {
    Count(usize),
    Variable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnnSave {
    PredVal,
    PredProb,
    Partition,
    Fold,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Knn {
    pub target: Option<String>,
    pub level: Option<KnnLevel>,
    /// None without BY; empty when BY has no names.
    pub factors: Option<Vec<VarItem>>,
    pub covariates: Option<Vec<VarItem>>,
    pub except: Vec<VarItem>,
    pub case_labels: Option<String>,
    pub focal: Option<String>,
    /// COVARIATE=ADJNORMALIZED (the default).
    pub rescale: bool,
    pub partition: KnnPartition,
    pub city_block: bool,
    pub neighbors: KnnNeighbors,
    /// FEATURES=AUTO, with the FORCE list.
    pub feature_selection: Option<Vec<VarItem>>,
    pub feature_count: KnnFeatureCount,
    pub median: bool,
    pub weight_features: bool,
    /// None when CROSSVALIDATION is not given (10 folds).
    pub folds: Option<KnnFolds>,
    pub include: bool,
    pub view: bool,
    pub print_cps: bool,
    /// In the order written, each with its name or root name if given.
    pub save: Vec<(KnnSave, Option<String>)>,
    pub max_categories: usize,
    pub model_file: Option<String>,
    pub focal_file: Option<String>,
}

/// One keyword of `names`, written in full and once within its subcommand.
fn keyword(
    p: &mut Parser,
    seen: &mut Vec<&'static str>,
    names: &[&'static str],
) -> PResult<&'static str> {
    let start = p.pos;
    let Some(&name) = names.iter().find(|n| p.eat_word(n)) else {
        return Err(p.unexpected(names));
    };
    if seen.contains(&name) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{name} is given twice in this subcommand"),
        ));
    }
    seen.push(name);
    Ok(name)
}

fn one_of(p: &mut Parser, names: &[&'static str]) -> PResult<&'static str> {
    names
        .iter()
        .find(|n| p.eat_word(n))
        .copied()
        .ok_or_else(|| p.unexpected(names))
}

fn whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low as f64 && n < 1e9 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number of at least {low}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// `KEY=n` inside parentheses.
fn assigned_whole(p: &mut Parser, key: &'static str, low: usize) -> PResult<usize> {
    if !p.eat_word(key) {
        return Err(p.unexpected(&[key]));
    }
    p.expect(Punct::Eq)?;
    whole(p, key, low)
}

fn variable(p: &mut Parser) -> PResult<String> {
    if !p.eat_word("VARIABLE") {
        return Err(p.unexpected(&["VARIABLE"]));
    }
    p.expect(Punct::Eq)?;
    p.name("a variable name")
}

fn given_name(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

pub(crate) fn knn(p: &mut Parser) -> PResult<Command> {
    let mut command = Knn {
        target: None,
        level: None,
        factors: None,
        covariates: None,
        except: Vec::new(),
        case_labels: None,
        focal: None,
        rescale: true,
        partition: KnnPartition::Ratio {
            training: 70.0,
            holdout: 30.0,
        },
        city_block: false,
        neighbors: KnnNeighbors::Fixed(3),
        feature_selection: None,
        feature_count: KnnFeatureCount::Auto,
        median: false,
        weight_features: false,
        folds: None,
        include: false,
        view: true,
        print_cps: true,
        save: Vec::new(),
        max_categories: 25,
        model_file: None,
        focal_file: None,
    };
    let start = p.pos;
    if matches!(p.peek(), Some(TokenKind::Word(w))
        if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    {
        command.target = Some(p.name("the target variable")?);
        if p.eat(Punct::LParen) {
            if !p.eat_word("MLEVEL") {
                return Err(p.unexpected(&["MLEVEL"]));
            }
            p.expect(Punct::Eq)?;
            command.level = Some(match one_of(p, &["S", "O", "N"])? {
                "S" => KnnLevel::Scale,
                "O" => KnnLevel::Ordinal,
                _ => KnnLevel::Nominal,
            });
            p.expect(Punct::RParen)?;
        }
    }
    if p.eat_word("BY") {
        command.factors = Some(match p.peek() {
            Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH") => {
                list_until(p, &["WITH"])?
            }
            _ => Vec::new(),
        });
    }
    if p.eat_word("WITH") {
        command.covariates = Some(match at_end(p) {
            true => Vec::new(),
            false => p.var_list()?,
        });
    }
    let listed = |l: &Option<Vec<VarItem>>| l.as_ref().is_some_and(|l| !l.is_empty());
    if !listed(&command.factors) && !listed(&command.covariates) {
        return Err(SyntaxError::new(
            p.span_from(start),
            "KNN needs at least one predictor: factors after BY or covariates after WITH",
        ));
    }
    if !matches!(p.peek(), None | Some(TokenKind::Punct(Punct::Slash))) {
        return Err(p.unexpected(&["BY", "WITH", "'/'", "end of command"]));
    }

    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        let at = p.pos;
        let names = [
            "EXCEPT",
            "CASELABELS",
            "FOCALCASES",
            "RESCALE",
            "PARTITION",
            "MODEL",
            "CRITERIA",
            "CROSSVALIDATION",
            "MISSING",
            "VIEWMODEL",
            "PRINT",
            "SAVE",
            "OUTFILE",
        ];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("KNN", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(at),
                format!("KNN takes /{sub} once"),
            ));
        }
        subcommands.push(sub);
        if at_end(p) {
            return Err(SyntaxError::new(
                p.span_from(at),
                format!("/{sub} is empty; give its specifications or leave it out"),
            ));
        }
        let mut seen = Vec::new();
        match sub {
            "EXCEPT" => {
                keyword(p, &mut seen, &["VARIABLES"])?;
                p.expect(Punct::Eq)?;
                command.except = p.var_list()?;
            }
            "CASELABELS" => command.case_labels = Some(variable(p)?),
            "FOCALCASES" => command.focal = Some(variable(p)?),
            "RESCALE" => {
                keyword(p, &mut seen, &["COVARIATE"])?;
                p.expect(Punct::Eq)?;
                command.rescale = one_of(p, &["ADJNORMALIZED", "NONE"])? == "ADJNORMALIZED";
            }
            "PARTITION" => {
                let (mut training, mut holdout) = (70.0, 30.0);
                let mut by_variable = None;
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &["TRAINING", "HOLDOUT", "VARIABLE"])?;
                    p.expect(Punct::Eq)?;
                    match name {
                        "TRAINING" => training = whole(p, "TRAINING", 1)? as f64,
                        "HOLDOUT" => holdout = whole(p, "HOLDOUT", 0)? as f64,
                        _ => by_variable = Some(p.name("a variable name")?),
                    }
                }
                command.partition = match by_variable {
                    Some(_) if seen.len() > 1 => {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "PARTITION takes TRAINING and HOLDOUT or VARIABLE, not both",
                        ));
                    }
                    Some(name) => KnnPartition::Variable(name),
                    None => KnnPartition::Ratio { training, holdout },
                };
            }
            "MODEL" => {
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &["METRIC", "NEIGHBORS", "FEATURES"])?;
                    p.expect(Punct::Eq)?;
                    match name {
                        "METRIC" => {
                            command.city_block = one_of(p, &["EUCLID", "CITYBLOCK"])? == "CITYBLOCK"
                        }
                        "NEIGHBORS" => {
                            command.neighbors = if one_of(p, &["FIXED", "AUTO"])? == "FIXED" {
                                let mut k = 3;
                                if p.eat(Punct::LParen) {
                                    k = assigned_whole(p, "K", 1)?;
                                    p.expect(Punct::RParen)?;
                                }
                                KnnNeighbors::Fixed(k)
                            } else {
                                let (mut min, mut max) = (3, 5);
                                if p.eat(Punct::LParen) {
                                    let from = p.pos;
                                    let first_min = matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("KMIN"));
                                    let (a, b) = match first_min {
                                        true => ("KMIN", "KMAX"),
                                        false => ("KMAX", "KMIN"),
                                    };
                                    let x = assigned_whole(p, a, 1)?;
                                    p.eat(Punct::Comma);
                                    let y = assigned_whole(p, b, 1)?;
                                    p.expect(Punct::RParen)?;
                                    (min, max) = if first_min { (x, y) } else { (y, x) };
                                    if min >= max {
                                        return Err(SyntaxError::new(
                                            p.span_from(from),
                                            format!("KMIN ({min}) must be less than KMAX ({max})"),
                                        ));
                                    }
                                }
                                KnnNeighbors::Auto { min, max }
                            };
                        }
                        _ => {
                            command.feature_selection = if one_of(p, &["ALL", "AUTO"])? == "ALL" {
                                None
                            } else if p.eat(Punct::LParen) {
                                if !p.eat_word("FORCE") {
                                    return Err(p.unexpected(&["FORCE"]));
                                }
                                p.expect(Punct::Eq)?;
                                let force = p.var_list()?;
                                p.expect(Punct::RParen)?;
                                Some(force)
                            } else {
                                Some(Vec::new())
                            };
                        }
                    }
                }
            }
            "CRITERIA" => {
                while !at_end(p) {
                    let name = keyword(
                        p,
                        &mut seen,
                        &["NUMFEATURES", "PREDICTED", "WEIGHTFEATURES"],
                    )?;
                    p.expect(Punct::Eq)?;
                    match name {
                        "NUMFEATURES" => {
                            command.feature_count =
                                match one_of(p, &["AUTO", "FIXED", "ERRORRATIO"])? {
                                    "AUTO" => KnnFeatureCount::Auto,
                                    "FIXED" => {
                                        p.expect(Punct::LParen)?;
                                        let n = whole(p, "the number of features", 1)?;
                                        p.expect(Punct::RParen)?;
                                        KnnFeatureCount::Fixed(n)
                                    }
                                    _ => {
                                        let mut change = 0.01;
                                        if p.eat(Punct::LParen) {
                                            if !p.eat_word("MINCHANGE") {
                                                return Err(p.unexpected(&["MINCHANGE"]));
                                            }
                                            p.expect(Punct::Eq)?;
                                            let from = p.pos;
                                            change = match p.number() {
                                                Some(v) if v > 0.0 => v,
                                                Some(v) => {
                                                    return Err(SyntaxError::new(
                                                        p.span_from(from),
                                                        format!(
                                                            "MINCHANGE must be above 0, got {v}"
                                                        ),
                                                    ));
                                                }
                                                None => return Err(p.unexpected(&["a number"])),
                                            };
                                            p.expect(Punct::RParen)?;
                                        }
                                        KnnFeatureCount::ErrorRatio(change)
                                    }
                                };
                        }
                        "PREDICTED" => command.median = one_of(p, &["MEAN", "MEDIAN"])? == "MEDIAN",
                        _ => command.weight_features = one_of(p, &["NO", "YES"])? == "YES",
                    }
                }
            }
            "CROSSVALIDATION" => {
                let name = keyword(p, &mut seen, &["FOLDS", "VARIABLE"])?;
                p.expect(Punct::Eq)?;
                command.folds = Some(match name {
                    "FOLDS" => KnnFolds::Count(whole(p, "FOLDS", 2)?),
                    _ => KnnFolds::Variable(p.name("a variable name")?),
                });
                if !at_end(p)
                    && matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("FOLDS") || w.eq_ignore_ascii_case("VARIABLE"))
                {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "CROSSVALIDATION takes FOLDS or VARIABLE, not both",
                    ));
                }
            }
            "MISSING" => {
                keyword(p, &mut seen, &["USERMISSING"])?;
                p.expect(Punct::Eq)?;
                command.include = one_of(p, &["EXCLUDE", "INCLUDE"])? == "INCLUDE";
            }
            "VIEWMODEL" => {
                keyword(p, &mut seen, &["DISPLAY"])?;
                p.expect(Punct::Eq)?;
                command.view = one_of(p, &["YES", "NO"])? == "YES";
            }
            "PRINT" => {
                command.print_cps = false;
                while !at_end(p) {
                    if keyword(p, &mut seen, &["CPS", "NONE"])? == "CPS" {
                        command.print_cps = true;
                    }
                }
                if seen.len() > 1 {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "PRINT NONE cannot be given with other keywords",
                    ));
                }
            }
            "SAVE" => {
                let kinds = [
                    ("PREDVAL", Some(KnnSave::PredVal)),
                    ("PREDPROB", Some(KnnSave::PredProb)),
                    ("PARTITION", Some(KnnSave::Partition)),
                    ("FOLD", Some(KnnSave::Fold)),
                    ("MAXCAT", None),
                ];
                let words: Vec<&'static str> = kinds.iter().map(|k| k.0).collect();
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &words)?;
                    match kinds.iter().find(|k| k.0 == name).expect("listed").1 {
                        Some(kind) => {
                            command.save.push((kind, given_name(p)?));
                        }
                        None => {
                            p.expect(Punct::LParen)?;
                            command.max_categories = whole(p, "MAXCAT", 1)?;
                            p.expect(Punct::RParen)?;
                        }
                    }
                }
            }
            _ => {
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &["MODEL", "FOCALCASES"])?;
                    p.expect(Punct::Eq)?;
                    let file = Some(p.file("a file")?);
                    match name {
                        "MODEL" => command.model_file = file,
                        _ => command.focal_file = file,
                    }
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Knn(Box::new(command)))
}
