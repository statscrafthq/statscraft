//! Parser of CSCOXREG: Cox proportional hazards regression on a complex sample.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The variable list comes first: the end time, or a start time and an end time, then factors
//!   after BY and covariates after WITH. VARIABLES STATUS=var(values) and PLAN FILE= are
//!   required. The event values are numbers, THRU ranges or quoted strings.
//! - VARIABLES may also name BASELINESTRATA=var (a baseline hazard per value) and ID=var.
//! - MODEL, CUSTOM, STATISTICS, TEST, DOMAIN and MISSING work as for the other complex samples
//!   models; there is no intercept (the baseline hazard absorbs it), so an L row has no
//!   intercept coefficient. CRITERIA: CILEVEL, DF, LCONVERGE, MXITER, MXSTEP, PCONVERGE,
//!   SINGULAR and TIES=EFRON (default) | BRESLOW.
//! - PRINT SAMPLEINFO EVENTINFO RISKINFO HISTORY[(n)] GEF LMATRIX COVB CORB BASELINE NONE; a
//!   PRINT shows only what it names, and NONE gives way to any other keyword.
//! - CUSTOM may repeat; every other subcommand comes at most once and none may be empty.

use super::{CsCriteria, CsCustom, CsModelStatistics, CsPadjust, CsTestType};
use super::{checked, eat_assignment, error, once, plan_file, require_assignment};
use super::{convergence, custom, flag, pick, statistics, whole};
use crate::ast::{Command, MissingValue, Value, VarItem};
use crate::glm::{Term, list_until, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsCoxPrint {
    pub sampleinfo: bool,
    pub eventinfo: bool,
    pub riskinfo: bool,
    /// HISTORY(n): every nth iteration.
    pub history: Option<u32>,
    pub lmatrix: bool,
    pub covb: bool,
    pub corb: bool,
}

/// CSCOXREG.
#[derive(Debug, Clone, PartialEq)]
pub struct CsCox {
    pub start: Option<String>,
    pub end: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub status: String,
    pub events: Vec<MissingValue>,
    /// BASELINESTRATA
    pub strata: Option<String>,
    pub plan: String,
    pub model: Option<Vec<Term>>,
    pub custom: Vec<CsCustom>,
    pub criteria: CsCriteria,
    /// TIES=BRESLOW; Efron otherwise.
    pub breslow: bool,
    /// None when STATISTICS is not given.
    pub statistics: Option<CsModelStatistics>,
    pub test: CsTestType,
    pub padjust: CsPadjust,
    pub domain: Option<(String, Value)>,
    /// CLASSMISSING=INCLUDE
    pub include: bool,
    pub print: CsCoxPrint,
}

const COMMAND: &str = "CSCOXREG";

fn not_yet(p: &Parser, start: usize, what: &str) -> crate::error::SyntaxError {
    error(
        p,
        start,
        format!("{what} of {COMMAND} is not supported yet"),
    )
}

fn variables(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, p.pos, "VARIABLES needs STATUS="));
    }
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "STATUS")? {
            once(p, at, &mut seen, "STATUS")?;
            c.status = p.name("the status variable")?;
            p.expect(Punct::LParen)?;
            while let Some(v) = p.value_or_range()? {
                c.events.push(v);
                p.eat(Punct::Comma);
            }
            if c.events.is_empty() {
                return Err(p.unexpected(&["an event value"]));
            }
            p.expect(Punct::RParen)?;
        } else if eat_assignment(p, "BASELINESTRATA")? {
            once(p, at, &mut seen, "BASELINESTRATA")?;
            c.strata = Some(p.name("the baseline strata variable")?);
        } else if p.at_keyword("ID") {
            return Err(not_yet(p, at, "VARIABLES ID (several records per subject)"));
        } else {
            return Err(p.unexpected(&["STATUS =", "BASELINESTRATA =", "ID ="]));
        }
    }
    Ok(())
}

fn criteria(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let start = p.pos;
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "CRITERIA needs at least one keyword"));
    }
    let k = &mut c.criteria;
    let assign = eat_assignment;
    while !at_end(p) {
        let at = p.pos;
        if assign(p, "CILEVEL")? {
            once(p, at, &mut seen, "CILEVEL")?;
            k.cilevel = checked(p, "CILEVEL", "is at least 0 and below 100", |v| {
                (0.0..100.0).contains(&v)
            })?;
        } else if assign(p, "DF")? {
            once(p, at, &mut seen, "DF")?;
            k.df = Some(checked(p, "DF", "is above 0", |v| v > 0.0)?);
        } else if assign(p, "LCONVERGE")? {
            once(p, at, &mut seen, "LCONVERGE")?;
            k.lconverge = convergence(p, "LCONVERGE")?;
        } else if assign(p, "MXITER")? {
            once(p, at, &mut seen, "MXITER")?;
            k.mxiter = whole(p, "MXITER", 0.0)?;
        } else if assign(p, "MXSTEP")? {
            once(p, at, &mut seen, "MXSTEP")?;
            k.mxstep = whole(p, "MXSTEP", 1.0)?;
        } else if assign(p, "PCONVERGE")? {
            once(p, at, &mut seen, "PCONVERGE")?;
            k.pconverge = convergence(p, "PCONVERGE")?;
        } else if assign(p, "SINGULAR")? {
            once(p, at, &mut seen, "SINGULAR")?;
            k.singular = checked(p, "SINGULAR", "is above 0", |v| v > 0.0)?;
        } else if assign(p, "TIES")? {
            once(p, at, &mut seen, "TIES")?;
            let Some(breslow) = pick(p, &[("EFRON", false), ("BRESLOW", true)])? else {
                return Err(p.unexpected(&["EFRON", "BRESLOW"]));
            };
            c.breslow = breslow;
        } else {
            return Err(p.unexpected(&[
                "CILEVEL =",
                "DF =",
                "LCONVERGE =",
                "MXITER =",
                "MXSTEP =",
                "PCONVERGE =",
                "SINGULAR =",
                "TIES =",
            ]));
        }
    }
    Ok(())
}

fn print(p: &mut Parser) -> PResult<CsCoxPrint> {
    let start = p.pos;
    let mut out = CsCoxPrint {
        sampleinfo: false,
        eventinfo: false,
        riskinfo: false,
        history: None,
        lmatrix: false,
        covb: false,
        corb: false,
    };
    let mut seen = Vec::new();
    if at_end(p) {
        return Err(error(p, start, "PRINT needs at least one keyword"));
    }
    while !at_end(p) {
        let at = p.pos;
        if flag(p, &mut seen, "SAMPLEINFO")? {
            out.sampleinfo = true;
        } else if flag(p, &mut seen, "EVENTINFO")? {
            out.eventinfo = true;
        } else if flag(p, &mut seen, "RISKINFO")? {
            out.riskinfo = true;
        } else if flag(p, &mut seen, "LMATRIX")? {
            out.lmatrix = true;
        } else if flag(p, &mut seen, "COVB")? {
            out.covb = true;
        } else if flag(p, &mut seen, "CORB")? {
            out.corb = true;
        } else if flag(p, &mut seen, "NONE")? {
        } else if flag(p, &mut seen, "HISTORY")? {
            let mut every = 1;
            if p.eat(Punct::LParen) {
                every = whole(p, "HISTORY", 1.0)?;
                p.expect(Punct::RParen)?;
            }
            out.history = Some(every);
        } else if p.at_keyword("GEF") {
            return Err(not_yet(p, at, "PRINT GEF (the general estimable function)"));
        } else if p.at_keyword("BASELINE") {
            return Err(not_yet(p, at, "PRINT BASELINE (the baseline functions)"));
        } else {
            return Err(p.unexpected(&[
                "SAMPLEINFO",
                "EVENTINFO",
                "RISKINFO",
                "HISTORY",
                "GEF",
                "LMATRIX",
                "COVB",
                "CORB",
                "BASELINE",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn test(p: &mut Parser, c: &mut CsCox) -> PResult<()> {
    let start = p.pos;
    if at_end(p) {
        return Err(error(p, start, "TEST needs TYPE= or PADJUST="));
    }
    let mut seen = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        if eat_assignment(p, "TYPE")? {
            once(p, at, &mut seen, "TYPE")?;
            const TYPES: [(&str, CsTestType); 4] = [
                ("F", CsTestType::F),
                ("ADJF", CsTestType::AdjF),
                ("CHISQUARE", CsTestType::ChiSquare),
                ("ADJCHISQUARE", CsTestType::AdjChiSquare),
            ];
            let Some(t) = pick(p, &TYPES)? else {
                return Err(p.unexpected(&["F", "ADJF", "CHISQUARE", "ADJCHISQUARE"]));
            };
            c.test = t;
        } else if eat_assignment(p, "PADJUST")? {
            once(p, at, &mut seen, "PADJUST")?;
            const METHODS: [(&str, CsPadjust); 5] = [
                ("LSD", CsPadjust::Lsd),
                ("BONFERRONI", CsPadjust::Bonferroni),
                ("SEQBONFERRONI", CsPadjust::SeqBonferroni),
                ("SIDAK", CsPadjust::Sidak),
                ("SEQSIDAK", CsPadjust::SeqSidak),
            ];
            let Some(m) = pick(p, &METHODS)? else {
                return Err(p.unexpected(&[
                    "LSD",
                    "BONFERRONI",
                    "SEQBONFERRONI",
                    "SIDAK",
                    "SEQSIDAK",
                ]));
            };
            c.padjust = m;
        } else {
            return Err(p.unexpected(&["TYPE =", "PADJUST ="]));
        }
    }
    Ok(())
}

pub(crate) fn cscoxreg(p: &mut Parser) -> PResult<Command> {
    let end = p.name("the end time variable")?;
    let mut c = CsCox {
        start: None,
        end,
        factors: Vec::new(),
        covariates: Vec::new(),
        status: String::new(),
        events: Vec::new(),
        strata: None,
        plan: String::new(),
        model: None,
        custom: Vec::new(),
        criteria: CsCriteria::default(),
        breslow: false,
        statistics: None,
        test: CsTestType::F,
        padjust: CsPadjust::Lsd,
        domain: None,
        include: false,
        print: CsCoxPrint {
            sampleinfo: true,
            eventinfo: true,
            riskinfo: false,
            history: None,
            lmatrix: false,
            covb: false,
            corb: false,
        },
    };
    let not_by_with = |p: &Parser| {
        matches!(p.peek(), Some(TokenKind::Word(w))
            if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    };
    if not_by_with(p) {
        c.start = Some(std::mem::replace(
            &mut c.end,
            p.name("the end time variable")?,
        ));
    }
    if not_by_with(p) {
        return Err(error(
            p,
            p.pos,
            "CSCOXREG takes one or two time variables; factors follow BY and covariates WITH",
        ));
    }
    if p.eat_word("BY") {
        c.factors = list_until(p, &["WITH"])?;
    }
    if p.eat_word("WITH") {
        c.covariates = p.var_list()?;
    }
    let mut seen: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("VARIABLES") {
            once(p, start, &mut seen, "VARIABLES")?;
            variables(p, &mut c)?;
        } else if p.eat_keyword("PLAN") {
            once(p, start, &mut seen, "PLAN")?;
            c.plan = plan_file(p)?;
        } else if p.eat_keyword("MODEL") {
            once(p, start, &mut seen, "MODEL")?;
            let mut terms = Vec::new();
            while !at_end(p) {
                terms.push(term(p)?);
                p.eat(Punct::Comma);
            }
            if terms.is_empty() {
                return Err(error(p, start, "MODEL needs at least one effect"));
            }
            c.model = Some(terms);
        } else if p.eat_keyword("CUSTOM") {
            c.custom.push(custom(p, false)?);
        } else if p.eat_keyword("CRITERIA") {
            once(p, start, &mut seen, "CRITERIA")?;
            criteria(p, &mut c)?;
        } else if p.eat_keyword("STATISTICS") {
            once(p, start, &mut seen, "STATISTICS")?;
            c.statistics = Some(statistics(p, true)?);
        } else if p.eat_keyword("TEST") {
            once(p, start, &mut seen, "TEST")?;
            test(p, &mut c)?;
        } else if p.eat_keyword("DOMAIN") {
            once(p, start, &mut seen, "DOMAIN")?;
            require_assignment(p, "VARIABLE")?;
            let name = p.name("a variable name")?;
            p.expect(Punct::LParen)?;
            let Some(value) = p.value() else {
                return Err(p.unexpected(&["a value"]));
            };
            p.expect(Punct::RParen)?;
            c.domain = Some((name, value));
        } else if p.eat_keyword("MISSING") {
            once(p, start, &mut seen, "MISSING")?;
            require_assignment(p, "CLASSMISSING")?;
            c.include = match pick(p, &[("EXCLUDE", false), ("INCLUDE", true)])? {
                Some(include) => include,
                None => return Err(p.unexpected(&["EXCLUDE", "INCLUDE"])),
            };
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            c.print = print(p)?;
        } else {
            return Err(p.subcommand_error(
                COMMAND,
                &[
                    "JOINTPROB",
                    "TESTASSUMPTIONS",
                    "SURVIVALMETHOD",
                    "SAVE",
                    "PLOT",
                    "PATTERN",
                    "OUTFILE",
                ],
            ));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'", "BY", "WITH"])?;
    let end = p.pos;
    if c.status.is_empty() {
        return Err(error(
            p,
            end,
            "CSCOXREG needs VARIABLES STATUS=var(values): the status variable and its event values",
        ));
    }
    if c.plan.is_empty() {
        return Err(error(p, end, "CSCOXREG needs PLAN FILE='name'"));
    }
    let intercept_entry = c
        .custom
        .iter()
        .flat_map(|k| &k.rows)
        .flatten()
        .any(|(e, _)| e.as_ref().is_some_and(|t| t.variables.is_empty()));
    if intercept_entry {
        return Err(error(
            p,
            end,
            "CSCOXREG has no intercept, so an LMATRIX row cannot start with a number; name an effect or ALL",
        ));
    }
    if c.custom.iter().any(|k| k.rows.is_empty()) {
        return Err(error(
            p,
            end,
            "CSCOXREG has no intercept, so CUSTOM needs LMATRIX=",
        ));
    }
    Ok(Command::CsCox(Box::new(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn cox(source: &str) -> CsCox {
        let (commands, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        match &commands[0].command {
            Command::CsCox(c) => (**c).clone(),
            other => panic!("{other:?}"),
        }
    }

    fn fails(source: &str) -> String {
        let (_, errors) = parse(source);
        assert!(!errors.is_empty(), "no error for {source}");
        errors[0].message.clone()
    }

    #[test]
    fn one_or_two_times_status_and_defaults() {
        let c = cox(
            "CSCOXREG months BY arm WITH age /VARIABLES STATUS=died(1 3 THRU 5) /PLAN FILE='p.json'.",
        );
        assert_eq!(c.start, None);
        assert_eq!(c.end, "months");
        assert_eq!(c.status, "died");
        assert_eq!(c.events.len(), 2);
        assert!(!c.breslow && c.print.sampleinfo && c.print.eventinfo && !c.print.riskinfo);
        assert_eq!(c.criteria, CsCriteria::default());
        let c = cox(
            "CSCOXREG entry exit /VARIABLES STATUS=s('y') BASELINESTRATA=site /PLAN FILE='p.json' \
             /CRITERIA TIES=BRESLOW MXITER=20 PCONVERGE=[1e-8 ABSOLUTE] /PRINT RISKINFO HISTORY(2) COVB \
             /TEST TYPE=ADJF PADJUST=SIDAK /MISSING CLASSMISSING=INCLUDE /DOMAIN VARIABLE=zone(2) \
             /STATISTICS PARAMETER EXP SE.",
        );
        assert_eq!(c.start.as_deref(), Some("entry"));
        assert_eq!(c.end, "exit");
        assert_eq!(c.strata.as_deref(), Some("site"));
        assert!(c.breslow && c.include);
        assert_eq!(c.criteria.mxiter, 20);
        assert_eq!(c.criteria.pconverge, (1e-8, false));
        assert_eq!(c.print.history, Some(2));
        assert!(c.print.riskinfo && c.print.covb && !c.print.sampleinfo);
        assert_eq!((c.test, c.padjust), (CsTestType::AdjF, CsPadjust::Sidak));
        assert!(c.statistics.is_some_and(|s| s.exp && s.se && !s.ttest));
    }

    #[test]
    fn errors_name_what_is_missing_or_not_supported() {
        let base = "CSCOXREG t WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json'";
        assert!(fails("CSCOXREG t WITH x /PLAN FILE='p.json'.").contains("STATUS"));
        assert!(fails("CSCOXREG t WITH x /VARIABLES STATUS=s(1).").contains("PLAN"));
        assert!(
            fails("CSCOXREG a b c WITH x /VARIABLES STATUS=s(1) /PLAN FILE='p.json'.")
                .contains("one or two time variables")
        );
        for (sub, what) in [
            ("SAVE MARTINGALE", "subcommand SAVE"),
            ("OUTFILE COVB='c.sav'", "subcommand OUTFILE"),
            ("JOINTPROB FILE='j.sav'", "subcommand JOINTPROB"),
            ("PLOT SURVIVAL", "subcommand PLOT"),
            ("PATTERN x(1)", "subcommand PATTERN"),
            (
                "SURVIVALMETHOD BASELINE=BRESLOW",
                "subcommand SURVIVALMETHOD",
            ),
            (
                "TESTASSUMPTIONS PROPHAZARD=KM",
                "subcommand TESTASSUMPTIONS",
            ),
            ("PRINT BASELINE", "PRINT BASELINE"),
            ("PRINT GEF", "PRINT GEF"),
        ] {
            let message = fails(&format!("{base} /{sub}."));
            assert!(
                message.contains(what) && message.contains("not supported yet"),
                "{message}"
            );
        }
        let message =
            fails("CSCOXREG t WITH x /VARIABLES STATUS=s(1) ID=person /PLAN FILE='p.json'.");
        assert!(message.contains("ID") && message.contains("not supported yet"));
        assert!(fails(&format!("{base} /CUSTOM LMATRIX=1 x 1.")).contains("no intercept"));
        assert!(fails(&format!("{base} /CUSTOM KMATRIX=1.")).contains("no intercept"));
        assert!(fails(&format!("{base} /CRITERIA CHKSEP=3.")).contains("CHKSEP"));
        assert!(fails(&format!("{base} /PLAN FILE='q.json'.")).contains("only once"));
    }
}
