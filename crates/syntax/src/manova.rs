//! Parser of MANOVA, the older analysis of variance and covariance with its own design notation.
//!
//! Rules noted from the Command Syntax Reference (MANOVA, its univariate and multivariate parts):
//! - `dependents [BY factors (min,max) [factors (min,max) ...]] [WITH covariates]`; a range
//!   closes a group of factors and applies to each of them.
//! - Subcommands other than DESIGN stay in effect for every later DESIGN until replaced; a
//!   DESIGN uses those given before it. When no DESIGN ends the command, one more full
//!   factorial design is added. MISSING is read once for the whole procedure.
//! - DESIGN terms are separated by spaces or commas. A term is effects joined by `+` (pooled
//!   into one test); an effect is items joined by BY, then items after WITHIN (alias W) that
//!   nest it. An item is a factor or continuous variable with an optional `(n)` (a partition,
//!   or a level after WITHIN) or `POOL(varlist)`. `= n` makes the term error term n (1 to 10);
//!   `VS` (or AGAINST, VERSUS) names its error: WITHIN (W), RESIDUAL (R), WR (RW,
//!   WITHIN+RESIDUAL) or a number. CONSTANT is the constant term; MUPLUS joins the constant
//!   to the next term.
//! - ERROR, CONTRAST(f)=type, PARTITION(f)[=(n, m*k ...)], METHOD (UNIQUE, SEQUENTIAL,
//!   CONSTANT, NOCONSTANT, QR, CHOLESKY), PRINT and NOPRINT, OMEANS, PMEANS, ANALYSIS.
//! - POWER: F(a) and T(a) levels strictly between 0 and 1 (.05 each), APPROXIMATE or EXACT; a new
//!   POWER replaces the old one. CINTERVAL MULTIVARIATE needs a type in parentheses. PCOMPS and
//!   DISCRIM: ROTATE without a type means VARIMAX; a PCOMPS rotation lasts until NOROTATE.
//! - MWITHIN f(n) starts a term (the constant within level n of f) or nests like WITHIN.
//! - Repeated measures: WSFACTORS (names, each group closed by its number of levels) must be the
//!   first subcommand; WSDESIGN takes terms of within-subjects factors without MUPLUS,
//!   CONSTANT, POOL or error terms; MEASURE names the measures; covariates in parentheses after
//!   the others are constant over the within-subjects cells. SIGNIF(UNIV) is off by default then.
//! - TRANSFORM[(list [/list ...])] [=] [ORTHONORM] [CONTRAST | BASIS] type; RENAME names every
//!   continuous variable, `*` keeping one.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::factor::Rotation;
use crate::glm::{Contrast, contrast, list_until};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// Factors sharing one `(min,max)` range.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaFactors {
    pub names: Vec<VarItem>,
    pub min: f64,
    pub max: f64,
}

/// The error term a test uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManovaError {
    Within,
    Residual,
    WithinResidual,
    Number(u32),
}

/// A factor or continuous variable of a design effect, with its `(n)`; POOL gives several.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaItem {
    pub names: Vec<VarItem>,
    pub pool: bool,
    pub number: Option<u32>,
}

/// One effect: the crossed items (joined by BY) and the items it is nested within. An empty
/// effect is the constant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManovaEffect {
    pub crossed: Vec<ManovaItem>,
    pub within: Vec<ManovaItem>,
}

/// A term of DESIGN: effects pooled with `+`, as error term `define`, tested against `against`.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaTerm {
    pub effects: Vec<ManovaEffect>,
    pub define: Option<u32>,
    pub against: Option<ManovaError>,
    pub muplus: bool,
    /// The term as written, for the tables.
    pub label: String,
}

impl ManovaEffect {
    /// The constant: no factor and no nesting.
    pub fn is_constant(&self) -> bool {
        self.crossed.is_empty() && self.within.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ManovaPrint {
    pub multiv: bool,
    pub univ: bool,
    pub eigen: bool,
    pub dimenr: bool,
    pub hypoth: bool,
    pub stepdown: bool,
    pub brief: bool,
    pub parameters: bool,
    pub negsum: bool,
    pub error_sscp: bool,
    pub error_cov: bool,
    pub error_cor: bool,
    pub error_stddev: bool,
    pub averf: bool,
    pub avonly: bool,
    pub hf: bool,
    pub gg: bool,
    pub efsize: bool,
    pub singledf: bool,
    pub transform: bool,
    /// PRINT DESIGN(OVERALL) and DESIGN(ONEWAY).
    pub overall: bool,
    pub oneway: bool,
    /// PARAMETERS(ORTHO) and PARAMETERS(COR).
    pub ortho: bool,
    pub param_cor: bool,
}

/// TRANSFORM: the variable lists (empty for all continuous variables) and how the matrix is made.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaTransform {
    pub lists: Vec<Vec<VarItem>>,
    pub basis: bool,
    pub orthonorm: bool,
    pub kind: Contrast,
}

/// PMEANS: the dependents to predict (None for all) and the tables of collapsed means.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManovaPmeans {
    pub variables: Option<Vec<VarItem>>,
    pub tables: Vec<Vec<String>>,
}

/// CINTERVAL: the confidence level and the kind of interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManovaInterval {
    pub level: f64,
    pub joint: bool,
    pub bonferroni: bool,
    /// MULTIVARIATE(type): intervals that hold over all the dependents together.
    pub multivariate: Option<ManovaCriterion>,
}

/// The criterion of CINTERVAL MULTIVARIATE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManovaCriterion {
    Roy,
    Pillai,
    Wilks,
    Hotelling,
    Bonferroni,
}

/// POWER: the significance levels of the F and t tests whose power is shown, and whether
/// the multivariate power uses the exact noncentrality (one hypothesis df only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManovaPower {
    pub f_alpha: f64,
    pub t_alpha: f64,
    pub exact: bool,
}

/// PCOMPS: principal components of the error correlation and covariance matrices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManovaPcomps {
    pub cor: bool,
    pub cov: bool,
    /// Rotation::None for no rotation.
    pub rotate: Rotation,
    /// NCOMP: how many components to rotate; None for all.
    pub ncomp: Option<usize>,
    pub min_eigen: f64,
}

/// What follows ANALYSIS: `[(CONDITIONAL | UNCONDITIONAL)] = deps [WITH covariates]` or
/// `= (deps [WITH covariates] / ...) [WITH covariates]`.
fn analysis(p: &mut Parser) -> PResult<ManovaAnalysis> {
    let mut conditional = false;
    let keyword = |p: &Parser| {
        ["CONDITIONAL", "UNCONDITIONAL"]
            .into_iter()
            .find(|k| matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(k)))
    };
    if p.at(Punct::LParen)
        && let Some(k) = keyword(p)
    {
        p.pos += 2;
        conditional = k == "CONDITIONAL";
        p.expect(Punct::RParen)?;
    }
    p.eat(Punct::Eq);
    let one = |p: &mut Parser| -> PResult<(Vec<VarItem>, Vec<VarItem>)> {
        let deps = list_until(p, &["WITH"])?;
        let covs = match p.eat_word("WITH") {
            true => list_until(p, &[])?,
            false => Vec::new(),
        };
        Ok((deps, covs))
    };
    if !p.eat(Punct::LParen) {
        return Ok(ManovaAnalysis {
            groups: vec![one(p)?],
            conditional,
            covariates: Vec::new(),
        });
    }
    let mut groups = vec![one(p)?];
    while p.eat(Punct::Slash) {
        groups.push(one(p)?);
    }
    p.expect(Punct::RParen)?;
    let covariates = match p.eat_word("WITH") {
        true => list_until(p, &[])?,
        false => Vec::new(),
    };
    Ok(ManovaAnalysis {
        groups,
        conditional,
        covariates,
    })
}

/// DISCRIM: the discriminant analysis of each multivariate effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManovaDiscrim {
    pub raw: bool,
    pub stan: bool,
    pub estim: bool,
    pub cor: bool,
    /// Rotation::None for no rotation of the correlations.
    pub rotate: Rotation,
    /// A function is kept while the test of it and the later ones has Sig. at most this.
    pub alpha: f64,
}

/// ANALYSIS: one or several groups of analyses.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaAnalysis {
    /// Each group: its dependents and its covariates.
    pub groups: Vec<(Vec<VarItem>, Vec<VarItem>)>,
    /// CONDITIONAL: the variables of earlier groups are covariates of later ones.
    pub conditional: bool,
    /// The covariates after the closing parenthesis, which every group has.
    pub covariates: Vec<VarItem>,
}

/// The settings a DESIGN uses.
#[derive(Debug, Clone, PartialEq)]
pub struct ManovaDesign {
    /// None: the full factorial model.
    pub terms: Option<Vec<ManovaTerm>>,
    pub error: Option<ManovaError>,
    pub contrasts: Vec<(String, Contrast)>,
    pub partitions: Vec<(String, Vec<u32>)>,
    pub sequential: bool,
    pub constant: bool,
    pub print: ManovaPrint,
    /// ANALYSIS: the groups of dependents and covariates by name.
    pub analysis: Option<ManovaAnalysis>,
    pub pmeans: Option<ManovaPmeans>,
    /// WSDESIGN; None for the full factorial of the within-subjects factors.
    pub wsdesign: Option<Vec<ManovaTerm>>,
    pub transform: Option<ManovaTransform>,
    /// RENAME: a new name for each continuous variable, None to keep it.
    pub rename: Option<Vec<Option<String>>>,
    pub cinterval: ManovaInterval,
    pub residuals: bool,
    pub power: Option<ManovaPower>,
    pub pcomps: Option<ManovaPcomps>,
    pub discrim: Option<ManovaDiscrim>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManovaOmeans {
    pub variables: Option<Vec<VarItem>>,
    /// Each table: its factors; empty for CONSTANT.
    pub tables: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manova {
    pub dependents: Vec<VarItem>,
    pub factors: Vec<ManovaFactors>,
    pub covariates: Vec<VarItem>,
    /// Covariates in parentheses: constant over the within-subjects cells.
    pub constant_covariates: Vec<VarItem>,
    /// WSFACTORS: name and number of levels.
    pub wsfactors: Vec<(String, usize)>,
    pub measures: Vec<String>,
    pub designs: Vec<ManovaDesign>,
    pub implicit_design: bool,
    pub include: bool,
    /// CELLINFO: means, SSCP, covariance, correlation.
    pub cellinfo: [bool; 4],
    /// HOMOGENEITY: Bartlett-Box F, Cochran's C, Box's M.
    pub homogeneity: [bool; 3],
    pub omeans: Option<ManovaOmeans>,
}

const KNOWN: [&str; 2] = ["PLOT", "MATRIX"];

/// `(VARIMAX | EQUAMAX | QUARTIMAX | NOROTATE)` after ROTATE; VARIMAX when no parentheses follow.
fn rotate(p: &mut Parser) -> PResult<Rotation> {
    if !p.eat(Punct::LParen) {
        return Ok(Rotation::Varimax);
    }
    let kinds = [
        ("VARIMAX", Rotation::Varimax),
        ("EQUAMAX", Rotation::Equamax),
        ("QUARTIMAX", Rotation::Quartimax),
        ("NOROTATE", Rotation::None),
    ];
    let Some(&(_, kind)) = kinds.iter().find(|(k, _)| p.eat_keyword(k)) else {
        return Err(p.unexpected(&kinds.map(|(k, _)| k)));
    };
    p.expect(Punct::RParen)?;
    Ok(kind)
}

/// A number in parentheses that `ok` accepts, else the error `what`.
fn number_in_parens(p: &mut Parser, ok: impl Fn(f64) -> bool, what: &str) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let at = p.pos;
    match p.number() {
        Some(x) if ok(x) => {
            p.expect(Punct::RParen)?;
            Ok(x)
        }
        _ => Err(SyntaxError::new(p.span_from(at), what.to_string())),
    }
}

fn error_term(p: &mut Parser) -> PResult<ManovaError> {
    if let Some(n) = p.number() {
        return error_number(p, n);
    }
    let within = p.eat_keyword("WITHIN") || p.eat_word("W");
    if within && p.eat(Punct::Plus) {
        if p.eat_keyword("RESIDUAL") || p.eat_word("R") {
            return Ok(ManovaError::WithinResidual);
        }
        return Err(p.unexpected(&["RESIDUAL"]));
    }
    if within {
        return Ok(ManovaError::Within);
    }
    if p.eat_keyword("RESIDUAL") || p.eat_word("R") {
        return Ok(ManovaError::Residual);
    }
    if p.eat_word("WR") || p.eat_word("RW") {
        return Ok(ManovaError::WithinResidual);
    }
    Err(p.unexpected(&["WITHIN", "RESIDUAL", "WR", "an error term number"]))
}

fn error_number(p: &Parser, n: f64) -> PResult<ManovaError> {
    if n.fract() == 0.0 && (1.0..=10.0).contains(&n) {
        Ok(ManovaError::Number(n as u32))
    } else {
        Err(SyntaxError::new(
            p.span_from(p.pos - 1),
            "an error term number is a whole number from 1 to 10",
        ))
    }
}

fn whole(p: &mut Parser, what: &str) -> PResult<u32> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as u32),
        Some(_) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is a whole number of at least 1"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn item(p: &mut Parser) -> PResult<ManovaItem> {
    if p.eat_word("POOL") {
        p.expect(Punct::LParen)?;
        let names = list_until(p, &[])?;
        p.expect(Punct::RParen)?;
        return Ok(ManovaItem {
            names,
            pool: true,
            number: None,
        });
    }
    let name = p.name("a factor or variable name")?;
    let mut number = None;
    if p.eat(Punct::LParen) {
        number = Some(whole(p, "the partition or level number")?);
        p.expect(Punct::RParen)?;
    }
    Ok(ManovaItem {
        names: vec![VarItem::Name(name)],
        pool: false,
        number,
    })
}

fn is_within(p: &Parser) -> bool {
    p.at_keyword("WITHIN")
        || p.at_keyword("MWITHIN")
        || matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("W"))
}

fn effect(p: &mut Parser) -> PResult<ManovaEffect> {
    let mut e = ManovaEffect::default();
    if p.eat_keyword("CONSTANT") {
        return Ok(e);
    }
    if !p.at_keyword("MWITHIN") {
        e.crossed.push(item(p)?);
        while p.eat_word("BY") {
            e.crossed.push(item(p)?);
        }
    }
    while is_within(p) {
        p.pos += 1;
        e.within.push(item(p)?);
        while p.eat_word("BY") {
            e.within.push(item(p)?);
        }
    }
    Ok(e)
}

fn design_terms(p: &mut Parser) -> PResult<Vec<ManovaTerm>> {
    let mut terms = Vec::new();
    let mut muplus = false;
    while !at_end(p) {
        if p.eat_keyword("MUPLUS") {
            muplus = true;
            continue;
        }
        let mut effects = vec![effect(p)?];
        while p.eat(Punct::Plus) {
            effects.push(effect(p)?);
        }
        let mut define = None;
        if p.eat(Punct::Eq) {
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["an error term number"]));
            };
            let ManovaError::Number(n) = error_number(p, n)? else {
                unreachable!()
            };
            define = Some(n);
        }
        let against = if p.eat_keyword("VS") || p.eat_keyword("AGAINST") || p.eat_keyword("VERSUS")
        {
            Some(error_term(p)?)
        } else {
            None
        };
        let label = effects
            .iter()
            .map(effect_label)
            .collect::<Vec<_>>()
            .join(" + ");
        terms.push(ManovaTerm {
            effects,
            define,
            against,
            muplus: std::mem::take(&mut muplus),
            label,
        });
        p.eat(Punct::Comma);
    }
    if muplus {
        return Err(p.unexpected(&["a term after MUPLUS"]));
    }
    Ok(terms)
}

/// Keywords in parentheses after a PRINT keyword; empty when there are none.
fn options<'k>(p: &mut Parser, allowed: &[&'k str]) -> PResult<Vec<&'k str>> {
    let mut out = Vec::new();
    if !p.eat(Punct::LParen) {
        return Ok(out);
    }
    while !p.eat(Punct::RParen) {
        let Some(k) = allowed.iter().find(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(allowed));
        };
        p.pos += 1;
        out.push(*k);
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn item_label(i: &ManovaItem) -> String {
    let names: Vec<String> = i
        .names
        .iter()
        .map(|n| match n {
            VarItem::Name(n) => n.to_uppercase(),
            VarItem::Range(a, b) => format!("{} TO {}", a.to_uppercase(), b.to_uppercase()),
            VarItem::All => "ALL".into(),
        })
        .collect();
    let mut s = if i.pool {
        format!("POOL({})", names.join(" "))
    } else {
        names.join(" ")
    };
    if let Some(n) = i.number {
        s.push_str(&format!("({n})"));
    }
    s
}

/// An effect as the tables name it, such as `A BY B` or `C WITHIN A(1)`.
pub fn effect_label(e: &ManovaEffect) -> String {
    if e.is_constant() {
        return "CONSTANT".into();
    }
    if e.crossed.is_empty() {
        let within: Vec<String> = e.within.iter().map(item_label).collect();
        return format!("MWITHIN {}", within.join(" WITHIN "));
    }
    let join = |items: &[ManovaItem]| -> String {
        items
            .iter()
            .map(item_label)
            .collect::<Vec<_>>()
            .join(" BY ")
    };
    let mut s = join(&e.crossed);
    for w in &e.within {
        s.push_str(" WITHIN ");
        s.push_str(&item_label(w));
    }
    s
}

fn print(p: &mut Parser, on: bool, d: &mut ManovaDesign, m: &mut Manova) -> PResult<()> {
    p.eat(Punct::Eq);
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("CELLINFO") {
            let o = options(p, &["MEANS", "SSCP", "COV", "COR", "ALL"])?;
            let all = o.contains(&"ALL");
            for (i, k) in ["MEANS", "SSCP", "COV", "COR"].iter().enumerate() {
                if all || o.contains(k) || (o.is_empty() && i == 0) {
                    m.cellinfo[i] = on;
                }
            }
        } else if p.eat_keyword("HOMOGENEITY") {
            let o = options(p, &["BARTLETT", "COCHRAN", "BOXM", "ALL"])?;
            let all = o.is_empty() || o.contains(&"ALL");
            for (i, k) in ["BARTLETT", "COCHRAN", "BOXM"].iter().enumerate() {
                if all || o.contains(k) {
                    m.homogeneity[i] = on;
                }
            }
        } else if p.eat_keyword("PARAMETERS") {
            let o = options(
                p,
                &[
                    "ESTIM", "NEGSUM", "ALL", "ORTHO", "COR", "EFSIZE", "OPTIMAL",
                ],
            )?;
            if o.contains(&"OPTIMAL") {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "PARAMETERS(OPTIMAL) of MANOVA is not supported yet",
                ));
            }
            let has = |k: &str| o.contains(&k) || o.contains(&"ALL");
            let pr = &mut d.print;
            if o.is_empty() || has("ESTIM") || has("NEGSUM") || has("EFSIZE") {
                pr.parameters = on;
            }
            if has("NEGSUM") {
                pr.negsum = on;
            }
            if has("EFSIZE") {
                pr.efsize = on;
            }
            if has("ORTHO") {
                pr.ortho = on;
            }
            if has("COR") {
                pr.param_cor = on;
            }
        } else if p.eat_keyword("SIGNIF") {
            let o = options(
                p,
                &[
                    "MULTIV", "UNIV", "EIGEN", "DIMENR", "HYPOTH", "STEPDOWN", "BRIEF", "SINGLEDF",
                    "AVERF", "AVONLY", "HF", "GG", "EFSIZE",
                ],
            )?;
            if o.contains(&"AVERF") && o.contains(&"AVONLY") {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "SIGNIF takes AVERF or AVONLY, not both",
                ));
            }
            let pr = &mut d.print;
            for k in &o {
                let flag = match *k {
                    "MULTIV" => &mut pr.multiv,
                    "UNIV" => &mut pr.univ,
                    "EIGEN" => &mut pr.eigen,
                    "DIMENR" => &mut pr.dimenr,
                    "HYPOTH" => &mut pr.hypoth,
                    "STEPDOWN" => &mut pr.stepdown,
                    "BRIEF" => &mut pr.brief,
                    "SINGLEDF" => &mut pr.singledf,
                    "AVERF" => &mut pr.averf,
                    "AVONLY" => &mut pr.avonly,
                    "HF" => &mut pr.hf,
                    "GG" => &mut pr.gg,
                    _ => &mut pr.efsize,
                };
                *flag = on;
            }
            if on && o.contains(&"AVONLY") {
                pr.averf = true;
            }
            if on && o.contains(&"AVERF") {
                pr.avonly = false;
            }
        } else if p.eat_keyword("ERROR") {
            let o = options(p, &["SSCP", "COV", "COR", "STDDEV"])?;
            let flags = &mut d.print;
            if o.is_empty() {
                flags.error_cov = on;
                flags.error_cor = on;
                flags.error_stddev = on;
            }
            for k in &o {
                match *k {
                    "SSCP" => flags.error_sscp = on,
                    "COV" => flags.error_cov = on,
                    "COR" => flags.error_cor = on,
                    _ => flags.error_stddev = on,
                }
            }
        } else if p.eat_keyword("TRANSFORM") {
            d.print.transform = on;
        } else if p.eat_keyword("DESIGN") {
            let o = options(
                p,
                &[
                    "OVERALL",
                    "ONEWAY",
                    "DECOMP",
                    "BIAS",
                    "SOLUTION",
                    "REDUNDANCY",
                    "COLLINEARITY",
                    "ALL",
                ],
            )?;
            if let Some(k) = o.iter().find(|k| !["OVERALL", "ONEWAY"].contains(k)) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!(
                        "PRINT DESIGN({k}) of MANOVA is not supported yet; OVERALL and ONEWAY are"
                    ),
                ));
            }
            if o.is_empty() || o.contains(&"OVERALL") {
                d.print.overall = on;
            }
            if o.contains(&"ONEWAY") {
                d.print.oneway = on;
            }
        } else {
            return Err(p.unexpected(&[
                "CELLINFO",
                "HOMOGENEITY",
                "PARAMETERS",
                "SIGNIF",
                "ERROR",
                "TRANSFORM",
                "DESIGN",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn factor_in_parens(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let name = p.name("a factor name")?;
    p.expect(Punct::RParen)?;
    Ok(name)
}

fn means_tables(p: &mut Parser) -> PResult<Vec<Vec<String>>> {
    p.expect(Punct::LParen)?;
    let mut tables = Vec::new();
    while !p.eat(Punct::RParen) {
        if p.eat_keyword("CONSTANT") {
            tables.push(Vec::new());
        } else {
            let mut t = vec![p.name("a factor name")?];
            while p.eat_word("BY") {
                t.push(p.name("a factor name")?);
            }
            tables.push(t);
        }
        p.eat(Punct::Comma);
    }
    Ok(tables)
}

pub(crate) fn manova(p: &mut Parser) -> PResult<Command> {
    let dependents = list_until(p, &["BY", "WITH"])?;
    let mut factors = Vec::new();
    if p.eat_word("BY") {
        while matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH")) {
            let names = list_until(p, &["WITH"])?;
            let start = p.pos;
            p.expect(Punct::LParen)?;
            let (Some(min), _, Some(max)) = (p.number(), p.eat(Punct::Comma), p.number()) else {
                return Err(p.unexpected(&["the minimum and maximum values"]));
            };
            p.expect(Punct::RParen)?;
            if min.fract() != 0.0 || max.fract() != 0.0 || min > max {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "a factor range is two whole numbers, the minimum first",
                ));
            }
            factors.push(ManovaFactors { names, min, max });
            p.eat(Punct::Comma);
        }
        if factors.is_empty() {
            return Err(p.unexpected(&["a factor name"]));
        }
    }
    let (mut covariates, mut constant_covariates) = (Vec::new(), Vec::new());
    if p.eat_word("WITH") {
        if !p.at(Punct::LParen) {
            covariates = list_until(p, &[])?;
        }
        if p.eat(Punct::LParen) {
            constant_covariates = list_until(p, &[])?;
            p.expect(Punct::RParen)?;
        }
    }
    let mut m = Manova {
        dependents,
        factors,
        covariates,
        constant_covariates,
        wsfactors: Vec::new(),
        measures: Vec::new(),
        designs: Vec::new(),
        implicit_design: false,
        include: false,
        cellinfo: [false; 4],
        homogeneity: [false; 3],
        omeans: None,
    };
    let mut d = ManovaDesign {
        terms: None,
        error: None,
        contrasts: Vec::new(),
        partitions: Vec::new(),
        sequential: false,
        constant: true,
        print: ManovaPrint {
            multiv: true,
            univ: true,
            averf: true,
            ..ManovaPrint::default()
        },
        analysis: None,
        pmeans: None,
        wsdesign: None,
        transform: None,
        rename: None,
        cinterval: ManovaInterval {
            level: 0.95,
            joint: false,
            bonferroni: false,
            multivariate: None,
        },
        residuals: false,
        power: None,
        pcomps: None,
        discrim: None,
    };
    let mut last_design = false;
    let mut first = true;
    while p.eat(Punct::Slash) {
        last_design = false;
        let start = p.pos;
        if p.at_keyword("WSFACTORS") && !first {
            return Err(SyntaxError::new(
                p.span(),
                "WSFACTORS must be the first subcommand of MANOVA, and given once",
            ));
        }
        first = false;
        if p.eat_keyword("WSFACTORS") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let mut names = vec![p.name("a within-subjects factor name")?];
                while !p.at(Punct::LParen) {
                    names.push(p.name("a within-subjects factor name")?);
                }
                p.expect(Punct::LParen)?;
                let levels = whole(p, "the number of levels")?;
                p.expect(Punct::RParen)?;
                if levels < 2 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "a within-subjects factor has at least 2 levels",
                    ));
                }
                m.wsfactors
                    .extend(names.into_iter().map(|n| (n, levels as usize)));
                p.eat(Punct::Comma);
            }
            // repeated measures show the averaged tests, not the univariate ones, by default
            d.print.univ = false;
        } else if p.eat_keyword("WSDESIGN") {
            p.eat(Punct::Eq);
            let terms = design_terms(p)?;
            let bad = terms.iter().any(|t| {
                t.muplus
                    || t.define.is_some()
                    || t.against.is_some()
                    || t.effects.iter().any(|e| {
                        e.is_constant() || e.crossed.iter().chain(&e.within).any(|i| i.pool)
                    })
            });
            if bad {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "WSDESIGN takes within-subjects factors only: no CONSTANT, MUPLUS, POOL or error terms",
                ));
            }
            d.wsdesign = (!terms.is_empty()).then_some(terms);
        } else if p.eat_keyword("MEASURE") {
            p.eat(Punct::Eq);
            m.measures.clear();
            while !at_end(p) {
                m.measures.push(p.name("a measure name")?);
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("RENAME") {
            p.eat(Punct::Eq);
            let mut names = Vec::new();
            while !at_end(p) {
                if p.eat(Punct::Star) {
                    names.push(None);
                } else {
                    names.push(Some(p.name("a new name or '*'")?));
                }
                p.eat(Punct::Comma);
            }
            d.rename = Some(names);
        } else if p.eat_keyword("TRANSFORM") {
            let mut lists = Vec::new();
            if p.eat(Punct::LParen) {
                lists.push(list_until(p, &[])?);
                while p.eat(Punct::Slash) {
                    lists.push(list_until(p, &[])?);
                }
                p.expect(Punct::RParen)?;
            }
            p.eat(Punct::Eq);
            let (mut basis, mut orthonorm) = (false, false);
            loop {
                if p.eat_keyword("ORTHONORM") {
                    orthonorm = true;
                } else if p.eat_keyword("BASIS") {
                    basis = true;
                } else if !p.eat_keyword("CONTRAST") {
                    break;
                }
            }
            let kind = contrast(p, false)?;
            d.transform = Some(ManovaTransform {
                lists,
                basis,
                orthonorm,
                kind,
            });
        } else if p.eat_keyword("CINTERVAL") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let joint = p.at_keyword("JOINT");
                if p.eat_keyword("INDIVIDUAL") || p.eat_keyword("JOINT") {
                    d.cinterval.joint = joint;
                    if p.eat(Punct::LParen) {
                        let at = p.pos;
                        match p.number() {
                            Some(a) if a > 0.0 && a < 1.0 => d.cinterval.level = a,
                            _ => {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "the confidence level is a number between 0 and 1",
                                ));
                            }
                        }
                        p.expect(Punct::RParen)?;
                    }
                } else if p.eat_keyword("UNIVARIATE") {
                    p.expect(Punct::LParen)?;
                    if p.eat_keyword("BONFER") {
                        d.cinterval.bonferroni = true;
                    } else if p.eat_keyword("SCHEFFE") {
                        d.cinterval.bonferroni = false;
                    } else {
                        return Err(p.unexpected(&["SCHEFFE", "BONFER"]));
                    }
                    p.expect(Punct::RParen)?;
                    d.cinterval.multivariate = None;
                } else if p.eat_keyword("MULTIVARIATE") {
                    p.expect(Punct::LParen)?;
                    let kinds = [
                        ("ROY", ManovaCriterion::Roy),
                        ("PILLAI", ManovaCriterion::Pillai),
                        ("WILKS", ManovaCriterion::Wilks),
                        ("HOTELLING", ManovaCriterion::Hotelling),
                        ("BONFER", ManovaCriterion::Bonferroni),
                    ];
                    let Some(&(_, kind)) = kinds.iter().find(|(k, _)| p.eat_keyword(k)) else {
                        return Err(p.unexpected(&kinds.map(|(k, _)| k)));
                    };
                    p.expect(Punct::RParen)?;
                    d.cinterval.multivariate = Some(kind);
                } else {
                    return Err(p.unexpected(&[
                        "INDIVIDUAL",
                        "JOINT",
                        "UNIVARIATE",
                        "MULTIVARIATE",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("POWER") {
            p.eat(Punct::Eq);
            let mut power = ManovaPower {
                f_alpha: 0.05,
                t_alpha: 0.05,
                exact: false,
            };
            while !at_end(p) {
                if p.eat_keyword("APPROXIMATE") {
                    power.exact = false;
                } else if p.eat_keyword("EXACT") {
                    power.exact = true;
                } else {
                    let f = p.eat_word("F");
                    if !f && !p.eat_word("T") {
                        return Err(p.unexpected(&["F", "T", "APPROXIMATE", "EXACT"]));
                    }
                    p.expect(Punct::LParen)?;
                    let at = p.pos;
                    let a = match p.number() {
                        Some(a) if a > 0.0 && a < 1.0 => a,
                        _ => {
                            return Err(SyntaxError::new(
                                p.span_from(at),
                                "the significance level for power is a number between 0 and 1",
                            ));
                        }
                    };
                    p.expect(Punct::RParen)?;
                    if f {
                        power.f_alpha = a;
                    } else {
                        power.t_alpha = a;
                    }
                }
                p.eat(Punct::Comma);
            }
            d.power = Some(power);
        } else if p.eat_keyword("PCOMPS") {
            p.eat(Punct::Eq);
            let mut o = ManovaPcomps {
                cor: false,
                cov: false,
                // a rotation stays until NOROTATE cancels it
                rotate: d.pcomps.map_or(Rotation::None, |o| o.rotate),
                ncomp: None,
                min_eigen: 0.0,
            };
            while !at_end(p) {
                if p.eat_keyword("COR") {
                    o.cor = true;
                } else if p.eat_keyword("COV") {
                    o.cov = true;
                } else if p.eat_keyword("ALL") {
                    (o.cor, o.cov) = (true, true);
                    if o.rotate == Rotation::None {
                        o.rotate = Rotation::Varimax;
                    }
                } else if p.eat_keyword("ROTATE") {
                    o.rotate = rotate(p)?;
                } else if p.eat_keyword("NCOMP") {
                    let n = number_in_parens(
                        p,
                        |x| x >= 1.0 && x.fract() == 0.0,
                        "NCOMP is a whole number of at least 1",
                    )?;
                    o.ncomp = Some(n as usize);
                } else if p.eat_keyword("MINEIGEN") {
                    o.min_eigen =
                        number_in_parens(p, |x| x >= 0.0, "MINEIGEN is a number of at least 0")?;
                } else {
                    return Err(p.unexpected(&["COR", "COV", "ROTATE", "NCOMP", "MINEIGEN", "ALL"]));
                }
                p.eat(Punct::Comma);
            }
            d.pcomps = Some(o);
        } else if p.eat_keyword("DISCRIM") {
            p.eat(Punct::Eq);
            let mut o = ManovaDiscrim {
                raw: false,
                stan: false,
                estim: false,
                cor: false,
                rotate: Rotation::None,
                alpha: 0.25,
            };
            let mut any = false;
            while !at_end(p) {
                let mut shown = true;
                if p.eat_keyword("RAW") {
                    o.raw = true;
                } else if p.eat_keyword("STAN") {
                    o.stan = true;
                } else if p.eat_keyword("ESTIM") {
                    o.estim = true;
                } else if p.eat_keyword("COR") {
                    o.cor = true;
                } else if p.eat_keyword("ROTATE") {
                    o.rotate = rotate(p)?;
                    o.cor = true;
                } else if p.eat_keyword("ALL") {
                    (o.raw, o.stan, o.estim, o.cor) = (true, true, true, true);
                    o.rotate = Rotation::Varimax;
                } else if p.eat_keyword("ALPHA") {
                    o.alpha = number_in_parens(
                        p,
                        |x| (0.0..=1.0).contains(&x),
                        "ALPHA is a number from 0 to 1",
                    )?;
                    shown = false;
                } else {
                    return Err(
                        p.unexpected(&["RAW", "STAN", "ESTIM", "COR", "ROTATE", "ALL", "ALPHA"])
                    );
                }
                any |= shown;
                p.eat(Punct::Comma);
            }
            // DISCRIM alone (or with ALPHA only) shows every table but the rotation
            if !any {
                (o.raw, o.stan, o.estim, o.cor) = (true, true, true, true);
            }
            d.discrim = Some(o);
        } else if p.eat_keyword("RESIDUALS") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.at_keyword("PLOT") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "RESIDUALS PLOT of MANOVA is not supported yet; CASEWISE is",
                    ));
                }
                if !p.eat_keyword("CASEWISE") {
                    return Err(p.unexpected(&["CASEWISE"]));
                }
                p.eat(Punct::Comma);
            }
            d.residuals = true;
        } else if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = design_terms(p)?;
            d.terms = (!terms.is_empty()).then_some(terms);
            m.designs.push(d.clone());
            last_design = true;
        } else if p.eat_keyword("ERROR") {
            p.eat(Punct::Eq);
            d.error = Some(error_term(p)?);
        } else if p.eat_keyword("CONTRAST") {
            let name = factor_in_parens(p)?;
            p.expect(Punct::Eq)?;
            let c = contrast(p, false)?;
            d.contrasts.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            d.contrasts.push((name, c));
        } else if p.eat_keyword("PARTITION") {
            let name = factor_in_parens(p)?;
            let mut sizes = Vec::new();
            if p.eat(Punct::Eq) || p.at(Punct::LParen) {
                p.expect(Punct::LParen)?;
                while !p.eat(Punct::RParen) {
                    let n = whole(p, "a partition size")?;
                    if p.eat(Punct::Star) {
                        let size = whole(p, "a partition size")?;
                        sizes.extend(std::iter::repeat_n(size, n as usize));
                    } else {
                        sizes.push(n);
                    }
                    p.eat(Punct::Comma);
                }
            }
            d.partitions.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            d.partitions.push((name, sizes));
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("UNIQUE") {
                    d.sequential = false;
                } else if p.eat_keyword("SEQUENTIAL") {
                    d.sequential = true;
                } else if p.eat_keyword("CONSTANT") {
                    d.constant = true;
                } else if p.eat_keyword("NOCONSTANT") {
                    d.constant = false;
                } else if !(p.eat_word("QR") || p.eat_keyword("CHOLESKY")) {
                    return Err(p.unexpected(&[
                        "UNIQUE",
                        "SEQUENTIAL",
                        "CONSTANT",
                        "NOCONSTANT",
                        "QR",
                        "CHOLESKY",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            print(p, true, &mut d, &mut m)?;
        } else if p.eat_keyword("NOPRINT") {
            print(p, false, &mut d, &mut m)?;
        } else if p.eat_keyword("OMEANS") {
            p.eat(Punct::Eq);
            let mut o = ManovaOmeans::default();
            while !at_end(p) {
                if p.eat_keyword("VARIABLES") {
                    p.expect(Punct::LParen)?;
                    o.variables = Some(list_until(p, &[])?);
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("TABLES") {
                    o.tables.extend(means_tables(p)?);
                } else {
                    return Err(p.unexpected(&["VARIABLES", "TABLES"]));
                }
                p.eat(Punct::Comma);
            }
            if o.tables.is_empty() {
                if o.variables.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "OMEANS VARIABLES needs TABLES as well",
                    ));
                }
                m.cellinfo[0] = true;
            } else {
                m.omeans = Some(o);
            }
        } else if p.eat_keyword("PMEANS") {
            p.eat(Punct::Eq);
            let mut o = ManovaPmeans::default();
            while !at_end(p) {
                if p.eat_keyword("VARIABLES") {
                    p.expect(Punct::LParen)?;
                    o.variables = Some(list_until(p, &[])?);
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("TABLES") {
                    o.tables.extend(means_tables(p)?);
                } else if p.at_keyword("PLOT") {
                    return Err(SyntaxError::new(
                        p.span(),
                        "PMEANS PLOT of MANOVA is not supported yet; VARIABLES and TABLES are",
                    ));
                } else {
                    return Err(p.unexpected(&["VARIABLES", "TABLES"]));
                }
                p.eat(Punct::Comma);
            }
            d.pmeans = Some(o);
        } else if p.eat_keyword("ANALYSIS") {
            d.analysis = Some(analysis(p)?);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("INCLUDE") {
                    m.include = true;
                } else if p.eat_keyword("EXCLUDE") {
                    m.include = false;
                } else if !p.eat_keyword("LISTWISE") {
                    return Err(p.unexpected(&["LISTWISE", "EXCLUDE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("MANOVA", &KNOWN));
        }
        if !last_design && !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !last_design {
        m.implicit_design = !m.designs.is_empty();
        d.terms = None;
        m.designs.push(d);
    }
    Ok(Command::Manova(Box::new(m)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn manova(source: &str) -> Manova {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        match parsed.remove(0).command {
            Command::Manova(m) => *m,
            other => panic!("{other:?}"),
        }
    }

    fn error(source: &str) -> String {
        let (_, mut errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors.remove(0).message
    }

    #[test]
    fn analysis_groups_conditional_and_global_covariates() {
        let name = |n: &str| VarItem::Name(n.to_string());
        let m = manova("MANOVA a b c BY f(1,2) WITH d /ANALYSIS=a b WITH d /DESIGN.");
        let an = m.designs[0].analysis.as_ref().unwrap();
        assert_eq!(
            an.groups,
            vec![(vec![name("a"), name("b")], vec![name("d")])]
        );
        assert!(!an.conditional && an.covariates.is_empty());
        let m = manova(
            "MANOVA a b c d BY f(1,2) /ANALYSIS(CONDITIONAL)=(a, b WITH c / d) WITH e /DESIGN.",
        );
        let an = m.designs[0].analysis.as_ref().unwrap();
        assert!(an.conditional);
        assert_eq!(
            an.groups,
            vec![
                (vec![name("a"), name("b")], vec![name("c")]),
                (vec![name("d")], Vec::new())
            ]
        );
        assert_eq!(an.covariates, vec![name("e")]);
        let m = manova("MANOVA a b BY f(1,2) /ANALYSIS (UNCONDITIONAL) = (a / b) /DESIGN.");
        assert!(!m.designs[0].analysis.as_ref().unwrap().conditional);
        // one ANALYSIS stays in effect for the designs after it
        let m = manova("MANOVA a b BY f(1,2) /ANALYSIS=(a / b) /DESIGN f /DESIGN.");
        assert!(m.designs.iter().all(|d| d.analysis.is_some()));
        assert!(error("MANOVA a b BY f(1,2) /ANALYSIS=(a / b.").contains("unexpected"));
        assert!(error("MANOVA a b BY f(1,2) /ANALYSIS=(a / ) /DESIGN.").contains("unexpected"));
    }

    #[test]
    fn variable_list_and_settings_per_design() {
        let m = manova(
            "MANOVA y1 y2 BY a b (1,3) c(0,1) WITH x /CONTR(a)=SIMPLE(1) /METH=SEQ NOCONSTANT \
             /DESIGN a, b W a = 1 VS 1, MUPLUS c + a BY c VS WR, CONSTANT /ERROR=R \
             /PRINT=SIGNIF(EIGEN) /NOPRINT=SIGNIF(UNIV).",
        );
        assert_eq!(m.factors.len(), 2);
        assert_eq!(
            (m.factors[0].min, m.factors[0].max, m.factors[1].max),
            (1.0, 3.0, 1.0)
        );
        assert_eq!(m.covariates, vec![VarItem::Name("x".into())]);
        assert_eq!(m.designs.len(), 2);
        assert!(m.implicit_design);
        let d = &m.designs[0];
        assert!(d.sequential && !d.constant);
        let t = d.terms.as_ref().unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[1].label, "B WITHIN A");
        assert_eq!(
            (t[1].define, t[1].against),
            (Some(1), Some(ManovaError::Number(1)))
        );
        assert!(t[2].muplus);
        assert_eq!(t[2].label, "C + A BY C");
        assert_eq!(t[2].against, Some(ManovaError::WithinResidual));
        assert_eq!(t[3].label, "CONSTANT");
        let last = &m.designs[1];
        assert!(last.terms.is_none());
        assert_eq!(last.error, Some(ManovaError::Residual));
        assert!(last.print.eigen && !last.print.univ && last.print.multiv);
    }

    #[test]
    fn partitions_and_unsupported_parts() {
        let m = manova("MANOVA y BY t(1,12) /PARTITION(t)=(3*2,4) /DESIGN t(2).");
        assert_eq!(m.designs[0].partitions[0].1, vec![2, 2, 2, 4]);
        assert!(
            error("MANOVA y BY t(1,3) /PRINT=PARAMETERS(OPTIMAL).").contains("not supported yet")
        );
        assert!(error("MANOVA y BY t(1,3) /PRINT=DESIGN(BIAS).").contains("not supported yet"));
        assert!(error("MANOVA y1 y2 /DESIGN /WSFACTORS=w(2).").contains("first subcommand"));
        assert!(error("MANOVA y1 y2 /WSFACTORS=w(2) /WSDESIGN=CONSTANT.").contains("WSDESIGN"));
        assert!(error("MANOVA y BY t(3,1).").contains("minimum first"));
        assert!(error("MANOVA y BY t(1,3) /DESIGN t = 11.").contains("1 to 10"));
    }

    #[test]
    fn power_levels_and_multivariate_intervals() {
        // POWER alone: both levels .05, approximate; a later POWER replaces the earlier one
        let m = manova(
            "MANOVA y1 y2 BY g(1,2) /POWER /DESIGN /POW=T(.1) EXA F(.01) \
             /CINT=JOINT(.9) MULT(HOT) /DESIGN /CINTERVAL=UNIV(BONFER).",
        );
        let power = |i: usize| m.designs[i].power.unwrap();
        assert_eq!(
            (power(0).f_alpha, power(0).t_alpha, power(0).exact),
            (0.05, 0.05, false)
        );
        assert_eq!(
            (power(1).f_alpha, power(1).t_alpha, power(1).exact),
            (0.01, 0.1, true)
        );
        assert_eq!(m.designs[0].cinterval.multivariate, None);
        let iv = m.designs[1].cinterval;
        assert_eq!((iv.level, iv.joint), (0.9, true));
        assert_eq!(iv.multivariate, Some(ManovaCriterion::Hotelling));
        assert_eq!(m.designs[2].cinterval.multivariate, None);
        assert!(m.designs[2].cinterval.bonferroni);
        for (word, kind) in [
            ("ROY", ManovaCriterion::Roy),
            ("PILL", ManovaCriterion::Pillai),
            ("WIL", ManovaCriterion::Wilks),
            ("BONFER", ManovaCriterion::Bonferroni),
        ] {
            let m = manova(&format!("MANOVA y1 y2 /CINTERVAL MULTIVARIATE({word})."));
            assert_eq!(m.designs[0].cinterval.multivariate, Some(kind));
        }
        assert!(error("MANOVA y /POWER F(1).").contains("between 0 and 1"));
        let e = error("MANOVA y /POWER ALPHA(.1).");
        assert!(e.contains("unexpected word ALPHA"), "{e}");
        assert!(error("MANOVA y /CINTERVAL MULTIVARIATE(SCHEFFE).").contains("SCHEFFE"));
    }

    #[test]
    fn principal_components_and_discriminant_settings() {
        // a rotation carries over to the next PCOMPS until NOROTATE; DISCRIM alone shows all
        // but the rotation, and ROTATE brings the correlations it rotates
        let m = manova(
            "MANOVA y1 y2 BY g(1,3) /PCOMPS COR ROT(EQUAMAX) NCOMP(2) MINEIG(.5) /DISCRIM /DESIGN \
             /PCOMPS COV /DISCRIM RAW ROTATE(QUARTIMAX) ALPHA(1) /DESIGN \
             /PCOMPS=ALL ROTATE(NOROTATE) /DISCRIM=ALL /DESIGN.",
        );
        let pc = |i: usize| m.designs[i].pcomps.unwrap();
        let dc = |i: usize| m.designs[i].discrim.unwrap();
        assert!(pc(0).cor && !pc(0).cov);
        assert_eq!(
            (pc(0).rotate, pc(0).ncomp, pc(0).min_eigen),
            (Rotation::Equamax, Some(2), 0.5)
        );
        assert!(!pc(1).cor && pc(1).cov && pc(1).ncomp.is_none());
        assert_eq!(pc(1).rotate, Rotation::Equamax);
        assert!(pc(2).cor && pc(2).cov);
        assert_eq!(pc(2).rotate, Rotation::None);
        assert!(dc(0).raw && dc(0).stan && dc(0).estim && dc(0).cor);
        assert_eq!((dc(0).rotate, dc(0).alpha), (Rotation::None, 0.25));
        assert!(dc(1).raw && !dc(1).stan && !dc(1).estim && dc(1).cor);
        assert_eq!((dc(1).rotate, dc(1).alpha), (Rotation::Quartimax, 1.0));
        assert_eq!(dc(2).rotate, Rotation::Varimax);
        assert!(error("MANOVA y1 y2 /PCOMPS NCOMP(0).").contains("NCOMP"));
        assert!(error("MANOVA y1 y2 /DISCRIM ALPHA(2).").contains("ALPHA"));
        assert!(error("MANOVA y1 y2 /DISCRIM ROTATE(PROMAX).").contains("unexpected"));
    }

    #[test]
    fn repeated_measures_and_transformations() {
        let m = manova(
            "MANOVA a1 a2 a3 b1 b2 b3 BY g(1,2) WITH x1 x2 x3 (z) /WSFACTORS=t u(3) v w(2) \
             /MEASURE=a b /WSDESIGN=t, MWITHIN u(2) /RENAME=* * c * * * * * * * \
             /PRINT=SIGNIF(AVONLY GG HF EFSIZE SINGLEDF) TRANSFORM PARAM(ORTHO COR) DESIGN(ONEWAY) \
             /PMEANS=VARIABLES(a1) TABLES(g) /CINTERVAL=JOINT(.9) UNIVARIATE(BONFER) /RESIDUALS \
             /TRANSFORM(a1 a2/b1 b2)=ORTHONORM BASIS SPECIAL(1 1 1 -1) /DESIGN=MWITHIN g(1).",
        );
        assert_eq!(m.covariates.len(), 3);
        assert_eq!(m.constant_covariates, vec![VarItem::Name("z".into())]);
        assert_eq!(
            m.wsfactors,
            vec![
                ("t".to_string(), 3),
                ("u".to_string(), 3),
                ("v".to_string(), 2),
                ("w".to_string(), 2)
            ]
        );
        assert_eq!(m.measures, vec!["a".to_string(), "b".to_string()]);
        let d = &m.designs[0];
        let ws = d.wsdesign.as_ref().unwrap();
        assert_eq!(ws[1].label, "MWITHIN U(2)");
        assert_eq!(d.rename.as_ref().unwrap()[2], Some("c".into()));
        let pr = d.print;
        assert!(pr.avonly && pr.averf && pr.gg && pr.hf && pr.efsize && pr.singledf);
        assert!(pr.transform && pr.ortho && pr.param_cor && pr.oneway && !pr.overall);
        assert!(!pr.parameters && !pr.univ && pr.multiv);
        assert_eq!(d.cinterval.level, 0.9);
        assert!(d.cinterval.joint && d.cinterval.bonferroni && d.residuals);
        let t = d.transform.as_ref().unwrap();
        assert_eq!(t.lists.len(), 2);
        assert!(t.basis && t.orthonorm);
        assert_eq!(
            d.pmeans.as_ref().unwrap().tables,
            vec![vec!["g".to_string()]]
        );
        let term = &d.terms.as_ref().unwrap()[0];
        assert!(term.effects[0].crossed.is_empty() && !term.effects[0].is_constant());
        assert!(error("MANOVA y /PMEANS PLOT.").contains("not supported yet"));
        let e = error("MANOVA y /CINTERVAL MULTIVARIATE.");
        assert!(e.contains("unexpected end of command"), "{e}");
    }
}
