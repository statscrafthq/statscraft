//! Parser of NAIVEBAYES.
//!
//! Rules, in our words, from the Command Syntax Reference: the dependent comes first, then BY
//! with the factors and WITH with the covariates. Every subcommand is optional and may be
//! given once, keywords are written in full, a keyword may appear once within its subcommand
//! and the equals signs and parentheses are required. EXCEPT VARIABLES=list; FORCE FACTORS=
//! COVARIATES=; TRAININGSAMPLE PERCENT=p (0 < p < 100) or VARIABLE=name; SUBSET MAXSIZE=AUTO
//! or n with (BESTSUBSET=PSEUDOBIC | TESTDATA), EXACTSIZE=n or NOSELECTION; CRITERIA BINS=10
//! (above 1) MEMALLOCATE=1024 (at least 4) TIMER=5 (at least 0); MISSING USERMISSING=EXCLUDE
//! or INCLUDE; PRINT CPS EXCLUDED SUMMARY SELECTED CLASSIFICATION or NONE alone; SAVE
//! PREDVAL[(name)] PREDPROB[(root[:n])]; OUTFILE MODEL=file. An empty subcommand is ignored.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::twostep::{keyword, names_before_keyword, number_from, whole};

#[derive(Debug, Clone, PartialEq)]
pub enum NbTraining {
    Percent(f64),
    Variable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NbSubset {
    /// MAXSIZE: None for AUTO; `test` is Some(true) for TESTDATA, Some(false) for PSEUDOBIC.
    Max {
        size: Option<usize>,
        test: Option<bool>,
    },
    Exact(usize),
    NoSelection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Naivebayes {
    pub dependent: String,
    /// Some(empty) when BY or WITH is written without variables.
    pub factors: Option<Vec<VarItem>>,
    pub covariates: Option<Vec<VarItem>>,
    pub except: Vec<VarItem>,
    pub force_factors: Vec<VarItem>,
    pub force_covariates: Vec<VarItem>,
    pub training: Option<NbTraining>,
    pub subset: NbSubset,
    pub bins: usize,
    pub memory: f64,
    pub timer: f64,
    pub include_user_missing: bool,
    pub print_cps: bool,
    pub print_excluded: bool,
    pub print_summary: bool,
    pub print_selected: bool,
    pub print_classification: bool,
    /// PREDVAL with its name if given.
    pub save_value: Option<Option<String>>,
    /// PREDPROB with its root name if given and the number of probabilities.
    pub save_probability: Option<(Option<String>, usize)>,
    pub outfile: Option<String>,
}

pub(crate) fn naivebayes(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    let mut command = Naivebayes {
        dependent,
        factors: None,
        covariates: None,
        except: Vec::new(),
        force_factors: Vec::new(),
        force_covariates: Vec::new(),
        training: None,
        subset: NbSubset::Max {
            size: None,
            test: None,
        },
        bins: 10,
        memory: 1024.0,
        timer: 5.0,
        include_user_missing: false,
        print_cps: true,
        print_excluded: true,
        print_summary: true,
        print_selected: true,
        print_classification: true,
        save_value: None,
        save_probability: None,
        outfile: None,
    };
    let more = |p: &Parser, stop: &str| matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case(stop));
    if p.eat_word("BY") {
        command.factors = Some(match more(p, "WITH") {
            true => list_until(p, &["WITH"])?,
            false => Vec::new(),
        });
    }
    if p.eat_word("WITH") {
        command.covariates = Some(match more(p, "") {
            true => p.var_list()?,
            false => Vec::new(),
        });
    }
    if !at_end(p) {
        return Err(SyntaxError::new(
            p.span(),
            "NAIVEBAYES takes one dependent variable; factors follow BY and covariates WITH",
        ));
    }
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let start = p.pos;
        let names = [
            "EXCEPT",
            "FORCE",
            "TRAININGSAMPLE",
            "SUBSET",
            "CRITERIA",
            "MISSING",
            "PRINT",
            "SAVE",
            "OUTFILE",
        ];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("NAIVEBAYES", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("NAIVEBAYES takes /{sub} once"),
            ));
        }
        subcommands.push(sub);
        let mut seen = Vec::new();
        match sub {
            "EXCEPT" => {
                keyword(p, &mut seen, &["VARIABLES"])?;
                p.expect(Punct::Eq)?;
                command.except = p.var_list()?;
            }
            "FORCE" => {
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &["FACTORS", "COVARIATES"])?;
                    p.expect(Punct::Eq)?;
                    let list = names_before_keyword(p)?;
                    match name {
                        "FACTORS" => command.force_factors = list,
                        _ => command.force_covariates = list,
                    }
                }
            }
            "TRAININGSAMPLE" => {
                let name = keyword(p, &mut seen, &["PERCENT", "VARIABLE"])?;
                p.expect(Punct::Eq)?;
                command.training = Some(match name {
                    "PERCENT" => {
                        let at = p.pos;
                        let percent = number_from(p, name, 0.0, 100.0)?;
                        if percent <= 0.0 || percent >= 100.0 {
                            return Err(SyntaxError::new(
                                p.span_from(at),
                                "PERCENT must be above 0 and below 100",
                            ));
                        }
                        NbTraining::Percent(percent)
                    }
                    _ => NbTraining::Variable(p.name("a variable name")?),
                });
            }
            "SUBSET" => {
                let name = keyword(p, &mut seen, &["MAXSIZE", "EXACTSIZE", "NOSELECTION"])?;
                command.subset = match name {
                    "MAXSIZE" => {
                        p.expect(Punct::Eq)?;
                        let size = match p.eat_word("AUTO") {
                            true => None,
                            false => Some(whole(p, name, 1)?),
                        };
                        let mut test = None;
                        if p.eat(Punct::LParen) {
                            keyword(p, &mut seen, &["BESTSUBSET"])?;
                            p.expect(Punct::Eq)?;
                            test = Some(match p.eat_word("TESTDATA") {
                                true => true,
                                false if p.eat_word("PSEUDOBIC") => false,
                                false => return Err(p.unexpected(&["PSEUDOBIC", "TESTDATA"])),
                            });
                            p.expect(Punct::RParen)?;
                        }
                        NbSubset::Max { size, test }
                    }
                    "EXACTSIZE" => {
                        p.expect(Punct::Eq)?;
                        NbSubset::Exact(whole(p, name, 1)?)
                    }
                    _ => NbSubset::NoSelection,
                };
            }
            "CRITERIA" => {
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &["BINS", "MEMALLOCATE", "TIMER"])?;
                    p.expect(Punct::Eq)?;
                    match name {
                        "BINS" => command.bins = whole(p, name, 2)?,
                        "MEMALLOCATE" => command.memory = number_from(p, name, 4.0, f64::MAX)?,
                        _ => command.timer = number_from(p, name, 0.0, f64::MAX)?,
                    }
                }
            }
            "MISSING" => {
                keyword(p, &mut seen, &["USERMISSING"])?;
                p.expect(Punct::Eq)?;
                command.include_user_missing = match p.eat_word("INCLUDE") {
                    true => true,
                    false if p.eat_word("EXCLUDE") => false,
                    false => return Err(p.unexpected(&["EXCLUDE", "INCLUDE"])),
                };
            }
            "PRINT" => print(p, &mut command, &mut seen)?,
            "SAVE" => {
                while !at_end(p) {
                    match keyword(p, &mut seen, &["PREDVAL", "PREDPROB"])? {
                        "PREDVAL" => {
                            let mut name = None;
                            if p.eat(Punct::LParen) {
                                name = Some(p.name("a variable name")?);
                                p.expect(Punct::RParen)?;
                            }
                            command.save_value = Some(name);
                        }
                        _ => {
                            let (mut root, mut count) = (None, 25);
                            if p.eat(Punct::LParen) {
                                if !p.at(Punct::Colon) {
                                    root = Some(p.name("a root name")?);
                                }
                                if p.eat(Punct::Colon) {
                                    count = whole(p, "the number of probabilities", 1)?;
                                }
                                p.expect(Punct::RParen)?;
                            }
                            command.save_probability = Some((root, count));
                        }
                    }
                    p.eat(Punct::Comma);
                }
            }
            _ => {
                keyword(p, &mut seen, &["MODEL"])?;
                p.expect(Punct::Eq)?;
                command.outfile = Some(p.file("a file")?);
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Naivebayes(Box::new(command)))
}

/// PRINT: the keywords given replace the defaults; `keyword=NO` alone only turns one off.
fn print(p: &mut Parser, command: &mut Naivebayes, seen: &mut Vec<&'static str>) -> PResult<()> {
    let names = [
        "CPS",
        "EXCLUDED",
        "SUMMARY",
        "SELECTED",
        "CLASSIFICATION",
        "NONE",
    ];
    let mut chosen: Vec<(&'static str, bool)> = Vec::new();
    while !at_end(p) {
        let name = keyword(p, seen, &names)?;
        let mut on = true;
        if p.eat(Punct::Eq) {
            on = match p.eat_word("YES") {
                true => true,
                false if p.eat_word("NO") => false,
                false => return Err(p.unexpected(&["YES", "NO"])),
            };
        }
        chosen.push((name, on));
        p.eat(Punct::Comma);
    }
    if seen.contains(&"NONE") && seen.len() > 1 {
        return Err(SyntaxError::new(
            p.span(),
            "PRINT NONE cannot be given with other keywords",
        ));
    }
    if chosen.iter().any(|c| c.1) {
        command.print_cps = false;
        command.print_excluded = false;
        command.print_summary = false;
        command.print_selected = false;
        command.print_classification = false;
    }
    for (name, on) in chosen {
        match name {
            "CPS" => command.print_cps = on,
            "EXCLUDED" => command.print_excluded = on,
            "SUMMARY" => command.print_summary = on,
            "SELECTED" => command.print_selected = on,
            "CLASSIFICATION" => command.print_classification = on,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn nb(source: &str) -> Naivebayes {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match parsed.remove(0).command {
            Command::Naivebayes(n) => *n,
            other => panic!("{other:?}"),
        }
    }

    fn err(source: &str) -> String {
        let (_, mut errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors.remove(0).message
    }

    #[test]
    fn defaults() {
        let n = nb("NAIVEBAYES churn.");
        assert_eq!(n.dependent, "churn");
        assert_eq!((n.factors.clone(), n.covariates.clone()), (None, None));
        assert_eq!(
            n.subset,
            NbSubset::Max {
                size: None,
                test: None
            }
        );
        assert_eq!((n.bins, n.memory, n.timer), (10, 1024.0, 5.0));
        assert!(n.print_cps && n.print_excluded && n.print_summary);
        assert!(n.print_selected && n.print_classification);
        assert!(!n.include_user_missing && n.save_value.is_none());
    }

    #[test]
    fn every_subcommand() {
        let n = nb("NAIVEBAYES y BY a b WITH x TO z /EXCEPT VARIABLES=b \
             /FORCE FACTORS=f COVARIATES=g h /TRAININGSAMPLE VARIABLE=t \
             /SUBSET MAXSIZE=3 (BESTSUBSET=TESTDATA) /CRITERIA BINS=4 MEMALLOCATE=64 TIMER=0 \
             /MISSING USERMISSING=INCLUDE /PRINT CPS SELECTED \
             /SAVE PREDVAL(guess) PREDPROB(p:3) /OUTFILE MODEL='m.json'.");
        assert_eq!(n.factors.as_ref().map(Vec::len), Some(2));
        assert_eq!(
            n.covariates,
            Some(vec![VarItem::Range("x".into(), "z".into())])
        );
        assert_eq!(n.force_covariates.len(), 2);
        assert_eq!(n.training, Some(NbTraining::Variable("t".into())));
        assert_eq!(
            n.subset,
            NbSubset::Max {
                size: Some(3),
                test: Some(true)
            }
        );
        assert_eq!((n.bins, n.memory, n.timer), (4, 64.0, 0.0));
        assert!(n.include_user_missing);
        assert!(n.print_cps && n.print_selected && !n.print_summary && !n.print_classification);
        assert_eq!(n.save_value, Some(Some("guess".into())));
        assert_eq!(n.save_probability, Some((Some("p".into()), 3)));
        assert_eq!(n.outfile.as_deref(), Some("m.json"));
    }

    #[test]
    fn short_forms() {
        let n =
            nb("NAIVEBAYES y BY /SUBSET EXACTSIZE=2 /SAVE PREDPROB(:4) /PRINT CLASSIFICATION=NO.");
        assert_eq!(n.factors, Some(Vec::new()));
        assert_eq!(n.subset, NbSubset::Exact(2));
        assert_eq!(n.save_probability, Some((None, 4)));
        // =NO alone keeps the other defaults
        assert!(n.print_cps && n.print_summary && !n.print_classification);
        let n = nb("NAIVEBAYES y /TRAININGSAMPLE PERCENT=70 /SUBSET NOSELECTION /PRINT NONE /.");
        assert_eq!(n.training, Some(NbTraining::Percent(70.0)));
        assert_eq!(n.subset, NbSubset::NoSelection);
        assert!(!n.print_cps && !n.print_classification);
    }

    #[test]
    fn errors() {
        assert!(err("NAIVEBAYES y /PRINT CPS /PRINT SUMMARY.").contains("once"));
        assert!(err("NAIVEBAYES y /CRITERIA BINS=3 BINS=4.").contains("twice"));
        assert!(err("NAIVEBAYES y /PRINT NONE CPS.").contains("NONE"));
        assert!(err("NAIVEBAYES y /TRAININGSAMPLE PERCENT=100.").contains("PERCENT"));
        assert!(!err("NAIVEBAYES y /CRITERIA BINS=1.").is_empty());
        assert!(!err("NAIVEBAYES y /CRITERIA MEMALLOCATE=3.").is_empty());
        assert!(!err("NAIVEBAYES y /CRIT BINS=3.").is_empty());
        assert!(err("NAIVEBAYES y z BY a.").contains("one dependent"));
    }
}
