//! Parser of REGRESSION: linear regression with forced entry and removal or stepwise selection.
//!
//! Rules from the Command Syntax Reference (REGRESSION):
//! - VARIABLES (default (COLLECT): the variables of DEPENDENT and METHOD in that order), then
//!   the global subcommands DESCRIPTIVES, MISSING and REGWGT; STATISTICS, CRITERIA and
//!   ORIGIN/NOORIGIN before DEPENDENT; DEPENDENT once, followed by METHOD subcommands; then
//!   RESIDUALS, CASEWISE, SCATTERPLOT, PARTIALPLOT and SAVE. Statscraft takes the subcommands in
//!   any order; the last STATISTICS, CRITERIA and ORIGIN apply.
//! - The word METHOD may be left out: `/ENTER x` is `/METHOD=ENTER x`. One METHOD subcommand may
//!   hold several methods one after the other.
//! - STATISTICS without keywords, or omitted, is R ANOVA COEFF OUTS; ALL is every statistic but
//!   F. DESCRIPTIVES without keywords is MEAN STDDEV CORR.
//! - CRITERIA: PIN(0.05) or FIN(3.84), POUT(0.10) or FOUT(2.71), TOLERANCE(0.0001), MAXSTEPS,
//!   CIN(95); the value in parentheses may be left out and then is the default.
//! - SAVE: temporary variables, each with an optional name in parentheses; FITS saves DFFIT,
//!   SDFIT, DFBETA, SDBETA and COVRATIO under default names.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

#[derive(Debug, Clone, PartialEq)]
pub struct Regression {
    /// `/VARIABLES`; None is (COLLECT).
    pub variables: Option<Vec<VarItem>>,
    /// `/DESCRIPTIVES`; None when not asked for.
    pub descriptives: Option<RegressionDescriptives>,
    pub missing: RegressionMissing,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// `/REGWGT=var`: weights of weighted least squares.
    pub regwgt: Option<String>,
    pub statistics: RegressionStatistics,
    pub criteria: RegressionCriteria,
    /// `/ORIGIN`: no constant.
    pub origin: bool,
    pub dependent: Vec<VarItem>,
    pub methods: Vec<RegressionMethod>,
    /// `/RESIDUALS`: the residuals statistics and what it asks for besides charts.
    pub residuals: Option<RegressionResiduals>,
    pub casewise: Option<Casewise>,
    /// `/SAVE`: each temporary variable with the name given for it.
    pub save: Vec<(TempVar, Option<String>)>,
    /// `/RESIDUALS HISTOGRAM(...)`: a histogram of each, with a normal curve.
    pub histograms: Vec<TempVar>,
    /// `/RESIDUALS NORMPROB(...)`: a normal P-P plot of each, standardized.
    pub normprobs: Vec<TempVar>,
    /// `/SCATTERPLOT`: (vertical, horizontal) pairs.
    pub scatterplots: Vec<(PlotVariable, PlotVariable)>,
    /// `/PARTIALPLOT`: the independents to plot, empty for every one in the equation.
    pub partial: Option<Vec<VarItem>>,
    /// `/TEMPLATE` was given; it styles charts and is ignored.
    pub template: bool,
}

/// A variable of a scatterplot: a temporary variable (written with `*`) or one of the data.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotVariable {
    Temp(TempVar),
    Variable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegressionDescriptives {
    pub mean: bool,
    pub stddev: bool,
    pub variance: bool,
    pub corr: bool,
    pub sig: bool,
    pub badcorr: bool,
    pub cov: bool,
    pub xprod: bool,
    pub n: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionMissing {
    Listwise,
    Pairwise,
    MeanSubstitution,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RegressionStatistics {
    pub r: bool,
    pub anova: bool,
    pub coeff: bool,
    pub outs: bool,
    pub zpp: bool,
    pub change: bool,
    /// `CI(level)`: the confidence level in percent.
    pub ci: Option<f64>,
    pub bcov: bool,
    pub collin: bool,
    pub tol: bool,
    /// F and its significance in place of t.
    pub f: bool,
    /// Standard errors of the standardized coefficients.
    pub ses: bool,
    pub xtx: bool,
    pub selection: bool,
    pub label: bool,
}

impl RegressionStatistics {
    pub const DEFAULTS: Self = Self {
        r: true,
        anova: true,
        coeff: true,
        outs: true,
        zpp: false,
        change: false,
        ci: None,
        bcov: false,
        collin: false,
        tol: false,
        f: false,
        ses: false,
        xtx: false,
        selection: false,
        label: false,
    };
}

/// A criterion of entry or removal: a probability of F or a value of F.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Criterion {
    Probability(f64),
    F(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionCriteria {
    pub entry: Criterion,
    pub removal: Criterion,
    pub tolerance: f64,
    pub max_steps: Option<u32>,
    /// `CIN`: the level in percent of the intervals MCIN and ICIN.
    pub cin: f64,
}

impl Default for RegressionCriteria {
    fn default() -> Self {
        Self {
            entry: Criterion::Probability(0.05),
            removal: Criterion::Probability(0.10),
            tolerance: 0.0001,
            max_steps: None,
            cin: 95.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Enter,
    Remove,
    Stepwise,
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionMethod {
    pub kind: MethodKind,
    /// Empty: the variables of VARIABLES that are not dependents.
    pub variables: Vec<VarItem>,
}

/// The temporary variables of the residuals analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempVar {
    Pred,
    AdjPred,
    Sresid,
    Mahal,
    Resid,
    Zpred,
    Sdresid,
    Cook,
    Dresid,
    Zresid,
    Sepred,
    Lever,
    Dfbeta,
    Sdbeta,
    Dffit,
    Sdfit,
    Covratio,
    Mcin,
    Icin,
}

pub(crate) const TEMP_VARS: [(&str, TempVar); 19] = [
    ("PRED", TempVar::Pred),
    ("ADJPRED", TempVar::AdjPred),
    ("SRESID", TempVar::Sresid),
    ("MAHAL", TempVar::Mahal),
    ("RESID", TempVar::Resid),
    ("ZPRED", TempVar::Zpred),
    ("SDRESID", TempVar::Sdresid),
    ("COOK", TempVar::Cook),
    ("DRESID", TempVar::Dresid),
    ("ZRESID", TempVar::Zresid),
    ("SEPRED", TempVar::Sepred),
    ("LEVER", TempVar::Lever),
    ("DFBETA", TempVar::Dfbeta),
    ("SDBETA", TempVar::Sdbeta),
    ("DFFIT", TempVar::Dffit),
    ("SDFIT", TempVar::Sdfit),
    ("COVRATIO", TempVar::Covratio),
    ("MCIN", TempVar::Mcin),
    ("ICIN", TempVar::Icin),
];

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionResiduals {
    /// `DURBIN`: the Durbin-Watson statistic in the Model Summary.
    pub durbin: bool,
    /// `OUTLIERS(tempvars)`: the ten cases with the largest absolute values of each.
    pub outliers: Vec<TempVar>,
    /// `ID(var)`: labels the cases of the outlier and casewise tables.
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Casewise {
    /// `ALL`: every case; otherwise the cases at least `limit` away from 0 in standardized units.
    pub all: bool,
    pub limit: f64,
    /// `PLOT(tempvar)`
    pub plot: TempVar,
    /// The dependent variable is listed.
    pub dependent: bool,
    /// The temporary variables listed besides it.
    pub listed: Vec<TempVar>,
}

const METHODS: [(&str, MethodKind); 5] = [
    ("ENTER", MethodKind::Enter),
    ("REMOVE", MethodKind::Remove),
    ("STEPWISE", MethodKind::Stepwise),
    ("FORWARD", MethodKind::Forward),
    ("BACKWARD", MethodKind::Backward),
];

fn method_at(p: &Parser) -> Option<MethodKind> {
    METHODS
        .iter()
        .find(|(name, _)| p.at_keyword(name))
        .map(|(_, kind)| *kind)
}

fn temp_var(p: &mut Parser, allowed: &[TempVar], place: &str) -> PResult<TempVar> {
    let found = TEMP_VARS.iter().find(|(name, _)| p.at_keyword(name));
    match found {
        Some((_, var)) if allowed.contains(var) => {
            p.pos += 1;
            Ok(*var)
        }
        Some((name, _)) => Err(SyntaxError::new(
            p.span(),
            format!("{name} cannot be used in {place}"),
        )),
        None => {
            let names: Vec<&str> = TEMP_VARS
                .iter()
                .filter(|(_, v)| allowed.contains(v))
                .map(|(n, _)| *n)
                .collect();
            Err(p.unexpected(&names))
        }
    }
}

/// `(tempvar ...)`, or `default` when no parenthesis follows.
fn temp_vars(
    p: &mut Parser,
    allowed: &[TempVar],
    place: &str,
    default: TempVar,
) -> PResult<Vec<TempVar>> {
    if !p.eat(Punct::LParen) {
        return Ok(vec![default]);
    }
    let mut out = Vec::new();
    while !p.at(Punct::RParen) {
        out.push(temp_var(p, allowed, place)?);
        p.eat(Punct::Comma);
    }
    p.expect(Punct::RParen)?;
    Ok(out)
}

/// `(value)` after a keyword, or None when no parenthesis follows.
fn value(p: &mut Parser) -> PResult<Option<f64>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(Some(n))
}

fn checked(p: &Parser, start: usize, value: f64, ok: bool, what: &str) -> PResult<f64> {
    if ok {
        Ok(value)
    } else {
        Err(SyntaxError::new(
            p.span_from(start),
            format!("{what}, got {value}"),
        ))
    }
}

/// Skips the rest of a subcommand: charts and their settings.
fn skip(p: &mut Parser) {
    while !at_end(p) {
        p.pos += 1;
    }
}

fn statistics(p: &mut Parser) -> PResult<RegressionStatistics> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(RegressionStatistics::DEFAULTS);
    }
    let mut s = RegressionStatistics::default();
    const KEYWORDS: [&str; 17] = [
        "DEFAULTS",
        "ALL",
        "R",
        "ANOVA",
        "COEFF",
        "OUTS",
        "ZPP",
        "LABEL",
        "CHANGE",
        "CI",
        "F",
        "BCOV",
        "SES",
        "XTX",
        "COLLIN",
        "TOL",
        "SELECTION",
    ];
    while !at_end(p) {
        // R and F are single letters, so they are matched exactly
        let word = match p.peek() {
            Some(TokenKind::Word(w)) => w.clone(),
            _ => return Err(p.unexpected(&KEYWORDS)),
        };
        let keyword = if word.eq_ignore_ascii_case("R") || word.eq_ignore_ascii_case("F") {
            Some(word.to_uppercase())
        } else {
            KEYWORDS
                .iter()
                .find(|k| k.len() > 1 && matches_keyword(&word, k))
                .map(|k| k.to_string())
        };
        let Some(keyword) = keyword else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        match keyword.as_str() {
            "DEFAULTS" => {
                (s.r, s.anova, s.coeff, s.outs) = (true, true, true, true);
            }
            "ALL" => {
                let ci = s.ci.or(Some(95.0));
                s = RegressionStatistics {
                    r: true,
                    anova: true,
                    coeff: true,
                    outs: true,
                    zpp: true,
                    change: true,
                    ci,
                    bcov: true,
                    collin: true,
                    tol: true,
                    f: s.f,
                    ses: true,
                    xtx: true,
                    selection: true,
                    label: true,
                };
            }
            "R" => s.r = true,
            "ANOVA" => s.anova = true,
            "COEFF" => s.coeff = true,
            "OUTS" => s.outs = true,
            "ZPP" => s.zpp = true,
            "LABEL" => s.label = true,
            "CHANGE" => s.change = true,
            "CI" => {
                let start = p.pos;
                let level = value(p)?.unwrap_or(95.0);
                let ok = level > 0.0 && level <= 100.0;
                s.ci = Some(checked(
                    p,
                    start,
                    level,
                    ok,
                    "CI must be greater than 0 and at most 100",
                )?);
            }
            "F" => s.f = true,
            "BCOV" => s.bcov = true,
            "SES" => s.ses = true,
            "XTX" => s.xtx = true,
            "COLLIN" => s.collin = true,
            "TOL" => s.tol = true,
            _ => s.selection = true,
        }
        p.eat(Punct::Comma);
    }
    Ok(s)
}

fn criteria(p: &mut Parser, c: &mut RegressionCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 8] = [
        "DEFAULTS",
        "PIN",
        "POUT",
        "FIN",
        "FOUT",
        "TOLERANCE",
        "MAXSTEPS",
        "CIN",
    ];
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        let start = p.pos;
        let given = value(p)?;
        match i {
            0 => *c = RegressionCriteria::default(),
            1 | 2 => {
                let v = given.unwrap_or(if i == 1 { 0.05 } else { 0.10 });
                let v = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v <= 1.0,
                    "a probability must lie above 0 and at most 1",
                )?;
                if i == 1 {
                    c.entry = Criterion::Probability(v);
                } else {
                    c.removal = Criterion::Probability(v);
                }
            }
            3 | 4 => {
                let v = given.unwrap_or(if i == 3 { 3.84 } else { 2.71 });
                let v = checked(p, start, v, v >= 0.0, "an F value cannot be negative")?;
                if i == 3 {
                    c.entry = Criterion::F(v);
                } else {
                    c.removal = Criterion::F(v);
                }
            }
            5 => {
                let v = given.unwrap_or(0.0001);
                c.tolerance = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v <= 1.0,
                    "TOLERANCE must lie above 0 and at most 1",
                )?;
            }
            6 => {
                c.max_steps = match given {
                    None => None,
                    Some(n) if n >= 1.0 && n.fract() == 0.0 => Some(n as u32),
                    Some(n) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            format!("MAXSTEPS must be a positive whole number, got {n}"),
                        ));
                    }
                }
            }
            _ => {
                let v = given.unwrap_or(95.0);
                c.cin = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v < 100.0,
                    "CIN must lie between 0 and 100",
                )?;
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn descriptives(p: &mut Parser) -> PResult<Option<RegressionDescriptives>> {
    p.eat(Punct::Eq);
    let defaults = RegressionDescriptives {
        mean: true,
        stddev: true,
        corr: true,
        ..RegressionDescriptives::default()
    };
    if at_end(p) {
        return Ok(Some(defaults));
    }
    let mut d = RegressionDescriptives::default();
    let mut none = false;
    const KEYWORDS: [&str; 12] = [
        "DEFAULTS", "MEAN", "STDDEV", "VARIANCE", "CORR", "SIG", "BADCORR", "COV", "XPROD", "N",
        "ALL", "NONE",
    ];
    while !at_end(p) {
        let found = match p.peek() {
            Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("N") => Some(9),
            _ => KEYWORDS.iter().position(|k| k.len() > 1 && p.at_keyword(k)),
        };
        let Some(i) = found else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        match i {
            0 => (d.mean, d.stddev, d.corr) = (true, true, true),
            1 => d.mean = true,
            2 => d.stddev = true,
            3 => d.variance = true,
            4 => d.corr = true,
            5 => d.sig = true,
            6 => d.badcorr = true,
            7 => d.cov = true,
            8 => d.xprod = true,
            9 => d.n = true,
            10 => {
                d = RegressionDescriptives {
                    mean: true,
                    stddev: true,
                    variance: true,
                    corr: true,
                    sig: true,
                    badcorr: false,
                    cov: true,
                    xprod: true,
                    n: true,
                }
            }
            _ => none = true,
        }
        p.eat(Punct::Comma);
    }
    Ok((!none || d != RegressionDescriptives::default()).then_some(d))
}

/// A variable list that ends at a method keyword, a slash or the end.
fn method_variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if method_at(p).is_some() || p.at_keyword("TEST") {
            break;
        }
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            if p.eat_word("TO") {
                items.push(VarItem::Range(first, p.name("a variable name")?));
            } else {
                items.push(VarItem::Name(first));
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(items)
}

/// The methods of one METHOD subcommand, at least one.
fn methods(p: &mut Parser, out: &mut Vec<RegressionMethod>) -> PResult<()> {
    let mut any = false;
    while !at_end(p) || !any {
        if p.at_keyword("TEST") {
            return Err(SyntaxError::new(
                p.span(),
                "TEST in REGRESSION /METHOD is not supported yet",
            ));
        }
        let Some(kind) = method_at(p) else {
            return Err(p.unexpected(&["ENTER", "REMOVE", "STEPWISE", "FORWARD", "BACKWARD"]));
        };
        p.pos += 1;
        p.eat(Punct::Eq);
        let variables = method_variables(p)?;
        if kind == MethodKind::Remove && variables.is_empty() {
            return Err(p
                .unexpected(&["a variable name"])
                .hint("REMOVE needs a variable list"));
        }
        out.push(RegressionMethod { kind, variables });
        any = true;
    }
    Ok(())
}

/// Adds `vars` to `list` once each.
fn add_once(list: &mut Vec<TempVar>, vars: Vec<TempVar>) {
    for var in vars {
        if !list.contains(&var) {
            list.push(var);
        }
    }
}

fn residuals(p: &mut Parser, command: &mut Regression) -> PResult<RegressionResiduals> {
    p.eat(Punct::Eq);
    let mut r = RegressionResiduals {
        durbin: false,
        outliers: Vec::new(),
        id: None,
    };
    use TempVar as T;
    let defaults = |r: &mut RegressionResiduals, command: &mut Regression| {
        r.durbin = true;
        r.outliers = vec![T::Zresid];
        add_once(&mut command.histograms, vec![T::Zresid]);
        add_once(&mut command.normprobs, vec![T::Zresid]);
    };
    if at_end(p) {
        defaults(&mut r, command);
        return Ok(r);
    }
    const KEYWORDS: [&str; 9] = [
        "DEFAULTS",
        "DURBIN",
        "OUTLIERS",
        "ID",
        "NORMPROB",
        "HISTOGRAM",
        "SIZE",
        "SEPARATE",
        "POOLED",
    ];
    while !at_end(p) {
        let found = match p.peek() {
            Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("ID") => Some(3),
            _ => KEYWORDS.iter().position(|k| p.at_keyword(k)),
        };
        let Some(i) = found else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        match i {
            0 => defaults(&mut r, command),
            1 => r.durbin = true,
            2 => {
                let allowed = [
                    T::Zresid,
                    T::Resid,
                    T::Sresid,
                    T::Sdresid,
                    T::Dresid,
                    T::Mahal,
                    T::Cook,
                ];
                for var in temp_vars(p, &allowed, "OUTLIERS", T::Zresid)? {
                    if !r.outliers.contains(&var) {
                        r.outliers.push(var);
                    }
                }
            }
            3 => {
                p.expect(Punct::LParen)?;
                r.id = Some(p.name("a variable name")?);
                p.expect(Punct::RParen)?;
            }
            4 => {
                let allowed = [
                    T::Pred,
                    T::Resid,
                    T::Zpred,
                    T::Zresid,
                    T::Dresid,
                    T::Sresid,
                    T::Sdresid,
                ];
                let vars = temp_vars(p, &allowed, "NORMPROB", T::Zresid)?;
                add_once(&mut command.normprobs, vars);
            }
            5 => {
                let vars = temp_vars(p, &PLOTTED, "HISTOGRAM", T::Zresid)?;
                add_once(&mut command.histograms, vars);
            }
            6 => {
                p.expect(Punct::LParen)?;
                if !(p.eat_keyword("SEPARATE") || p.eat_keyword("POOLED")) {
                    return Err(p.unexpected(&["SEPARATE", "POOLED"]));
                }
                p.expect(Punct::RParen)?;
            }
            // without SELECT there is only one group of cases
            _ => {}
        }
        p.eat(Punct::Comma);
    }
    Ok(r)
}

/// The temporary variables that HISTOGRAM and SCATTERPLOT take.
const PLOTTED: [TempVar; 12] = [
    TempVar::Pred,
    TempVar::Resid,
    TempVar::Zpred,
    TempVar::Zresid,
    TempVar::Dresid,
    TempVar::AdjPred,
    TempVar::Sresid,
    TempVar::Sdresid,
    TempVar::Sepred,
    TempVar::Mahal,
    TempVar::Cook,
    TempVar::Lever,
];

/// `(y, x) (y, x) ...`, each a variable or `*tempvar`.
fn scatterplots(p: &mut Parser, out: &mut Vec<(PlotVariable, PlotVariable)>) -> PResult<()> {
    p.eat(Punct::Eq);
    let one = |p: &mut Parser| -> PResult<PlotVariable> {
        if p.eat(Punct::Star) {
            Ok(PlotVariable::Temp(temp_var(p, &PLOTTED, "SCATTERPLOT")?))
        } else {
            Ok(PlotVariable::Variable(p.name("a variable or *tempvar")?))
        }
    };
    let mut any = false;
    while p.eat(Punct::LParen) {
        let y = one(p)?;
        p.eat(Punct::Comma);
        let x = one(p)?;
        p.expect(Punct::RParen)?;
        out.push((y, x));
        any = true;
    }
    if !any {
        return Err(p.unexpected(&["(y, x)"]));
    }
    Ok(())
}

fn casewise(p: &mut Parser) -> PResult<Casewise> {
    p.eat(Punct::Eq);
    use TempVar as T;
    let mut c = Casewise {
        all: false,
        limit: 3.0,
        plot: T::Zresid,
        dependent: true,
        listed: vec![T::Pred, T::Resid],
    };
    let mut listed: Option<(bool, Vec<TempVar>)> = None;
    while !at_end(p) {
        if p.eat_keyword("DEFAULTS") {
            continue;
        }
        if p.eat_word("ALL") {
            c.all = true;
        } else if p.eat_keyword("OUTLIERS") {
            let start = p.pos;
            let limit = value(p)?.unwrap_or(3.0);
            c.limit = checked(p, start, limit, limit > 0.0, "OUTLIERS must be above 0")?;
        } else if p.eat_keyword("PLOT") {
            let allowed = [T::Zresid, T::Resid, T::Dresid, T::Sresid, T::Sdresid];
            c.plot = temp_vars(p, &allowed, "CASEWISE PLOT", T::Zresid)?[0];
        } else if p.eat_keyword("DEPENDENT") {
            listed.get_or_insert((false, Vec::new())).0 = true;
        } else {
            let all: Vec<TempVar> = TEMP_VARS.iter().map(|(_, v)| *v).collect();
            let var = temp_var(p, &all, "CASEWISE")?;
            let list = &mut listed.get_or_insert((false, Vec::new())).1;
            if !list.contains(&var) {
                list.push(var);
            }
        }
        p.eat(Punct::Comma);
    }
    if let Some((dependent, list)) = listed {
        c.dependent = dependent;
        c.listed = list;
    }
    Ok(c)
}

fn save(p: &mut Parser, out: &mut Vec<(TempVar, Option<String>)>) -> PResult<()> {
    p.eat(Punct::Eq);
    use TempVar as T;
    let all: Vec<TempVar> = TEMP_VARS.iter().map(|(_, v)| *v).collect();
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("FITS") {
            for var in [T::Dffit, T::Sdfit, T::Dfbeta, T::Sdbeta, T::Covratio] {
                if !out.iter().any(|(v, _)| *v == var) {
                    out.push((var, None));
                }
            }
            p.eat(Punct::Comma);
            continue;
        }
        let var = temp_var(p, &all, "SAVE")?;
        let name = if p.eat(Punct::LParen) {
            let name = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            if matches!(var, T::Mcin | T::Icin) {
                return Err(SyntaxError::new(
                    p.span_from(p.pos - 3),
                    "MCIN and ICIN save two variables each and take no name",
                ));
            }
            Some(name)
        } else {
            None
        };
        if out.iter().any(|(v, _)| *v == var) {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                "a temporary variable is saved only once",
            ));
        }
        out.push((var, name));
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["PRED", "RESID", "a temporary variable"]));
    }
    Ok(())
}

pub(crate) fn regression(p: &mut Parser) -> PResult<Command> {
    let mut command = Regression {
        variables: None,
        descriptives: None,
        missing: RegressionMissing::Listwise,
        include: false,
        regwgt: None,
        statistics: RegressionStatistics::DEFAULTS,
        criteria: RegressionCriteria::default(),
        origin: false,
        dependent: Vec::new(),
        methods: Vec::new(),
        residuals: None,
        casewise: None,
        save: Vec::new(),
        histograms: Vec::new(),
        normprobs: Vec::new(),
        scatterplots: Vec::new(),
        partial: None,
        template: false,
    };
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if !slash && !first {
            return Err(p.unexpected(&["'/'"]));
        }
        first = false;
        let start = p.pos;
        if method_at(p).is_some() {
            // the word METHOD left out
            methods(p, &mut command.methods)?;
        } else if p.eat_keyword("VARIABLES") {
            if command.variables.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "VARIABLES can be given only once",
                ));
            }
            p.eat(Punct::Eq);
            if p.eat(Punct::LParen) {
                if !p.eat_keyword("COLLECT") {
                    return Err(p.unexpected(&["COLLECT"]));
                }
                p.expect(Punct::RParen)?;
            } else {
                command.variables = Some(p.var_list()?);
            }
        } else if p.eat_keyword("DESCRIPTIVES") {
            command.descriptives = descriptives(p)?;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("LISTWISE") {
                    command.missing = RegressionMissing::Listwise;
                } else if p.eat_keyword("PAIRWISE") {
                    command.missing = RegressionMissing::Pairwise;
                } else if p.eat_keyword("MEANSUBSTITUTION") {
                    command.missing = RegressionMissing::MeanSubstitution;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&[
                        "LISTWISE",
                        "PAIRWISE",
                        "MEANSUBSTITUTION",
                        "INCLUDE",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("REGWGT") {
            p.eat(Punct::Eq);
            command.regwgt = Some(p.name("a variable name")?);
        } else if p.eat_keyword("STATISTICS") {
            command.statistics = statistics(p)?;
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("ORIGIN") {
            command.origin = true;
        } else if p.eat_keyword("NOORIGIN") {
            command.origin = false;
        } else if p.eat_keyword("DEPENDENT") {
            if !command.dependent.is_empty() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "DEPENDENT can be given only once",
                ));
            }
            p.eat(Punct::Eq);
            command.dependent = p.var_list()?;
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            methods(p, &mut command.methods)?;
        } else if p.eat_keyword("RESIDUALS") {
            let mut r = residuals(p, &mut command)?;
            if let Some(old) = command.residuals.take() {
                r.durbin |= old.durbin;
                r.id = r.id.or(old.id);
                for var in old.outliers {
                    if !r.outliers.contains(&var) {
                        r.outliers.push(var);
                    }
                }
            }
            command.residuals = Some(r);
        } else if p.eat_keyword("CASEWISE") {
            command.casewise = Some(casewise(p)?);
        } else if p.eat_keyword("SAVE") {
            save(p, &mut command.save)?;
        } else if p.eat_keyword("SCATTERPLOT") {
            scatterplots(p, &mut command.scatterplots)?;
        } else if p.eat_keyword("PARTIALPLOT") {
            p.eat(Punct::Eq);
            let list = command.partial.get_or_insert_with(Vec::new);
            if !p.eat_word("ALL") && !at_end(p) {
                list.extend(p.var_list()?);
            }
        } else if p.eat_keyword("TEMPLATE") {
            command.template = true;
            skip(p);
        } else {
            return Err(p.subcommand_error("REGRESSION", &["SELECT", "OUTFILE", "MATRIX"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if command.dependent.is_empty() {
        return Err(p.unexpected(&["/DEPENDENT=varlist"]));
    }
    if command.methods.is_empty() {
        return Err(p.unexpected(&["/METHOD=ENTER varlist"]));
    }
    if command.variables.is_none()
        && let Some(m) = command.methods.iter().find(|m| m.variables.is_empty())
    {
        let name = METHODS
            .iter()
            .find(|(_, k)| *k == m.kind)
            .expect("listed")
            .0;
        return Err(SyntaxError::new(
            p.span(),
            format!("METHOD={name} needs a variable list when VARIABLES is (COLLECT) or left out"),
        ));
    }
    Ok(Command::Regression(Box::new(command)))
}
