//! Parsers of the Bayesian commands BAYES ONESAMPLE, BAYES RELATED, BAYES INDEPENDENT and
//! BAYES CORRELATION.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - Subcommands come in any order. Inside them every setting is `KEYWORD = value`; keywords
//!   and subcommand names may be shortened to three or more letters (a shortening that fits
//!   two keywords, such as VAR, is an error).
//! - ANALYSIS on INFERENCE is POSTERIOR (the default), BAYESFACTOR or BOTH.
//! - CRITERIA: CILEVEL strictly between 0 and 100 (95); TOL above 0 and at most 0.001; MAXITER a
//!   positive whole number; METHOD=AGL (quadrature, the default) or MCA (Monte Carlo); SEED a
//!   whole number from 1 to 2147483647 or RANDOM; MCSAMPLES and POSTSAMPLES whole numbers from
//!   1000 to 1000000. Each command takes only the keywords it uses.
//! - MISSING SCOPE: ANALYSIS (default) or LISTWISE; BAYES CORRELATION has PAIRWISE (default)
//!   or LISTWISE.
//! - ONESAMPLE: INFERENCE names DISTRIBUTION (NORMAL, BINOMIAL or POISSON, required) and the
//!   VARIABLES. Each DATA subcommand gives settings to a sublist of them: VARIANCE (known,
//!   above 0), NULLVALUE (0) and ALTERGVALUE (above 0) for NORMAL; SUCCESS, NULLPRIORPARAM
//!   (2 2) and ALTERPRIORPARAM for BINOMIAL and POISSON (SUCCESS only BINOMIAL).
//! - PRIOR: VARDIST=DIFFUSE, INVCHISQ(df scale), INVGAMMA(shape scale), JEFFREYS2 or
//!   JEFFREYS4; PRECDIST=GAMMA(shape rate) or CHISQ(df), not with VARDIST; MEANDIST=DIFFUSE or
//!   NORMAL(mean [variance]); KAPPA above 0 (1); TYPE=CONJUGATE(a b) for BINOMIAL (both above
//!   0, or both 0) and POISSON (a above 0, b at least 0).
//! - RELATED: one or more PAIR subcommands, each VARIABLES = two names, with VARIANCE,
//!   NULLVALUE and ALTERGVALUE as on ONESAMPLE.
//! - INDEPENDENT: INFERENCE needs VARIABLES, GROUP (one variable) and SELECT = LEVEL(a b),
//!   MIDPOINT or CUTPOINT(value); DATAVAR gives the two known group variances. ESTBF
//!   COMPUTATION=ROUDER(scale 1), GONEN(mean variance; a lone value is the variance, the mean
//!   then 0) or HYPER(a, above -1 and at most -0.5, default -0.75). PRIOR EQUALDATAVAR, VARDIST
//!   (DIFFUSE, JEFFREYS or INVCHISQ(df scale)) and MEANDIST (DIFFUSE or NORMAL(mean1 mean2
//!   variance1 variance2)).
//! - CORRELATION: INFERENCE VARIABLES (two or more), CVALUE (any number, 0), MAXPLOTS (0 to 50,
//!   10); ESTBF TYPE=JZS or FRACTION(b, strictly between 0 and 1, 0.5) and NULLVALUE strictly
//!   between -1 and 1.
//!
//! Choices where the manual is silent: PRIOR may be left out (its defaults apply); a variable
//! on two DATA subcommands is an error; DISTRIBUTION=NORMAL is accepted on RELATED and
//! INDEPENDENT; LEVEL values may be numbers, quoted strings or bare words.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

/// POSTERIOR, BAYESFACTOR or BOTH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayesAnalysis {
    Posterior,
    BayesFactor,
    Both,
}

impl BayesAnalysis {
    pub fn posterior(self) -> bool {
        self != BayesAnalysis::BayesFactor
    }

    pub fn factor(self) -> bool {
        self != BayesAnalysis::Posterior
    }
}

/// The CRITERIA settings.
#[derive(Debug, Clone, PartialEq)]
pub struct BayesCriteria {
    /// CILEVEL as a fraction.
    pub ci: f64,
    /// METHOD=MCA: Monte Carlo instead of quadrature.
    pub monte_carlo: bool,
    /// None for SEED=RANDOM.
    pub seed: Option<u64>,
    pub tol: f64,
    pub maxiter: usize,
    pub mcsamples: usize,
    pub postsamples: usize,
}

/// The prior of the variance (VARDIST) or of the precision (PRECDIST).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariancePrior {
    Diffuse,
    /// Inverse chi-square with df and scale.
    InvChiSquare(f64, f64),
    /// Inverse gamma with shape and scale.
    InvGamma(f64, f64),
    /// Proportional to 1 / variance.
    Jeffreys2,
    /// Proportional to 1 / variance squared.
    Jeffreys4,
    /// Gamma on the precision with shape and rate.
    Gamma(f64, f64),
    /// Chi-square on the precision with df.
    ChiSquare(f64),
}

/// The PRIOR of BAYES ONESAMPLE and BAYES RELATED.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalPrior {
    pub variance: VariancePrior,
    /// MEANDIST=NORMAL(mean [variance]); None for DIFFUSE.
    pub mean: Option<(f64, Option<f64>)>,
    pub kappa: f64,
    /// TYPE=CONJUGATE(a b) of BINOMIAL and POISSON.
    pub conjugate: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayesDistribution {
    Normal,
    Binomial,
    Poisson,
}

/// Which values count as a success of a binomial variable.
#[derive(Debug, Clone, PartialEq)]
pub enum Success {
    Last,
    First,
    Midpoint,
    Cutpoint(f64),
    Level(Vec<Value>),
}

/// The settings of a normal test: of a DATA sublist or of a PAIR.
#[derive(Debug, Clone, PartialEq)]
pub struct MeanTest {
    /// A known variance.
    pub variance: Option<f64>,
    pub null: f64,
    /// ALTERGVALUE: a fixed g; None integrates g out (with a known variance it is then 1).
    pub g: Option<f64>,
}

/// One DATA subcommand of BAYES ONESAMPLE.
#[derive(Debug, Clone, PartialEq)]
pub struct BayesData {
    pub variables: Vec<VarItem>,
    pub test: MeanTest,
    pub success: Option<Success>,
    pub null_prior: Option<(f64, f64)>,
    pub alter_prior: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesOneSample {
    pub listwise: bool,
    pub criteria: BayesCriteria,
    pub distribution: BayesDistribution,
    pub variables: Vec<VarItem>,
    pub analysis: BayesAnalysis,
    pub data: Vec<BayesData>,
    pub prior: NormalPrior,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesPair {
    pub first: String,
    pub second: String,
    pub test: MeanTest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesRelated {
    pub listwise: bool,
    pub criteria: BayesCriteria,
    pub analysis: BayesAnalysis,
    pub pairs: Vec<BayesPair>,
    pub prior: NormalPrior,
}

/// How SELECT forms the two groups.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupSelect {
    Levels(Value, Value),
    Midpoint,
    Cutpoint(f64),
}

/// ESTBF COMPUTATION of BAYES INDEPENDENT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndependentFactor {
    /// The scale of the Cauchy prior on the effect size.
    Rouder(f64),
    /// The prior mean and variance of the effect size.
    Gonen(f64, f64),
    Hyper(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GroupVariancePrior {
    Diffuse,
    Jeffreys,
    InvChiSquare(f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesIndependent {
    pub listwise: bool,
    pub criteria: BayesCriteria,
    pub variables: Vec<VarItem>,
    pub group: String,
    pub select: GroupSelect,
    pub analysis: BayesAnalysis,
    /// DATAVAR: the known variances of the two groups.
    pub datavar: Option<(f64, f64)>,
    pub factor: IndependentFactor,
    pub equal_variances: bool,
    pub variance: GroupVariancePrior,
    /// MEANDIST=NORMAL(mean1 mean2 variance1 variance2).
    pub mean: Option<[f64; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesCorrelation {
    pub listwise: bool,
    pub criteria: BayesCriteria,
    pub variables: Vec<VarItem>,
    pub analysis: BayesAnalysis,
    /// CVALUE: the prior is proportional to (1 - rho^2)^c.
    pub c: f64,
    pub maxplots: usize,
    /// TYPE=FRACTION(b); None for JZS.
    pub fraction: Option<f64>,
    /// NULLVALUE of the fractional Bayes factor.
    pub null: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bayes {
    OneSample(BayesOneSample),
    Related(BayesRelated),
    Independent(BayesIndependent),
    Correlation(BayesCorrelation),
}

impl Bayes {
    pub fn name(&self) -> &'static str {
        match self {
            Bayes::OneSample(_) => "BAYES ONESAMPLE",
            Bayes::Related(_) => "BAYES RELATED",
            Bayes::Independent(_) => "BAYES INDEPENDENT",
            Bayes::Correlation(_) => "BAYES CORRELATION",
        }
    }
}

fn error<T>(p: &Parser, start: usize, message: impl Into<String>) -> PResult<T> {
    Err(SyntaxError::new(p.span_from(start), message.into()))
}

/// The keyword of `options` at the cursor, consumed. Error when a shortening fits two.
fn which(p: &mut Parser, options: &[&str]) -> PResult<Option<usize>> {
    let Some(TokenKind::Word(word)) = p.peek() else {
        return Ok(None);
    };
    if let Some(i) = options.iter().position(|o| word.eq_ignore_ascii_case(o)) {
        p.pos += 1;
        return Ok(Some(i));
    }
    let fits: Vec<usize> = (0..options.len())
        .filter(|&i| matches_keyword(word, options[i]))
        .collect();
    match fits.as_slice() {
        [] => Ok(None),
        [i] => {
            p.pos += 1;
            Ok(Some(*i))
        }
        many => {
            let names: Vec<&str> = many.iter().map(|&i| options[i]).collect();
            let message = format!(
                "{} could mean {}; write more of the word",
                word.to_uppercase(),
                names.join(" or ")
            );
            Err(SyntaxError::new(p.span(), message))
        }
    }
}

/// One of `options`, required.
fn choice(p: &mut Parser, options: &[&str]) -> PResult<usize> {
    which(p, options)?.ok_or_else(|| p.unexpected(options))
}

fn number(p: &mut Parser, what: &str) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

/// A number that must satisfy `ok`, described by `rule` in the error.
fn checked(p: &mut Parser, what: &str, rule: &str, ok: impl Fn(f64) -> bool) -> PResult<f64> {
    let start = p.pos;
    let value = number(p, what)?;
    if !ok(value) {
        return error(p, start, format!("{what} must be {rule}, got {value}"));
    }
    Ok(value)
}

fn whole(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<usize> {
    let rule = format!("a whole number from {low} to {high}");
    let value = checked(p, what, &rule, |v| {
        v.fract() == 0.0 && v >= low && v <= high
    })?;
    Ok(value as usize)
}

/// Numbers in parentheses, `min` to `max` of them; an empty list when there is no `(`.
fn numbers_in_parens(p: &mut Parser, what: &str, min: usize, max: usize) -> PResult<Vec<f64>> {
    if !p.eat(Punct::LParen) {
        return Ok(Vec::new());
    }
    let start = p.pos;
    let mut values = Vec::new();
    while let Some(v) = p.number() {
        values.push(v);
        p.eat(Punct::Comma);
    }
    p.expect(Punct::RParen)?;
    if values.len() < min || values.len() > max {
        let count = if min == max {
            format!("{min}")
        } else {
            format!("{min} to {max}")
        };
        return error(
            p,
            start,
            format!("{what} takes {count} values in parentheses"),
        );
    }
    Ok(values)
}

/// Exactly two numbers in parentheses.
fn pair_in_parens(p: &mut Parser, what: &str) -> PResult<(f64, f64)> {
    if !p.at(Punct::LParen) {
        return Err(p.unexpected(&["'('"]));
    }
    let v = numbers_in_parens(p, what, 2, 2)?;
    Ok((v[0], v[1]))
}

/// Two numbers, with or without parentheses.
fn two_numbers(p: &mut Parser, what: &str) -> PResult<(f64, f64)> {
    if p.at(Punct::LParen) {
        return pair_in_parens(p, what);
    }
    Ok((number(p, what)?, number(p, what)?))
}

/// A value of LEVEL: a number, a quoted string or a bare word.
fn level_value(p: &mut Parser) -> Option<Value> {
    if let Some(TokenKind::Word(w)) = p.peek() {
        p.pos += 1;
        return Some(Value::String(w.clone()));
    }
    p.value()
}

fn levels(p: &mut Parser) -> PResult<Vec<Value>> {
    p.expect(Punct::LParen)?;
    let mut values = Vec::new();
    while let Some(v) = level_value(p) {
        values.push(v);
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    p.expect(Punct::RParen)?;
    Ok(values)
}

/// A variable list that ends before the next `KEYWORD =`.
fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
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
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

/// The settings of one subcommand: `keyword = value` until the next slash. `read` gets the
/// index of the keyword in `keywords`, after the `=`.
fn settings(
    p: &mut Parser,
    keywords: &[&str],
    mut read: impl FnMut(&mut Parser, usize) -> PResult<()>,
) -> PResult<()> {
    while !at_end(p) {
        let Some(i) = which(p, keywords)? else {
            return Err(p.unexpected(keywords));
        };
        p.expect(Punct::Eq)?;
        read(p, i)?;
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn analysis(p: &mut Parser) -> PResult<BayesAnalysis> {
    Ok(match choice(p, &["POSTERIOR", "BAYESFACTOR", "BOTH"])? {
        0 => BayesAnalysis::Posterior,
        1 => BayesAnalysis::BayesFactor,
        _ => BayesAnalysis::Both,
    })
}

fn default_criteria(tol: f64) -> BayesCriteria {
    BayesCriteria {
        ci: 0.95,
        monte_carlo: false,
        seed: None,
        tol,
        maxiter: 2000,
        mcsamples: 10_000,
        postsamples: 10_000,
    }
}

/// CRITERIA with the keywords of `allowed`.
fn criteria(p: &mut Parser, c: &mut BayesCriteria, allowed: &[&str]) -> PResult<()> {
    settings(p, allowed, |p, i| {
        match allowed[i] {
            "CILEVEL" => {
                c.ci =
                    checked(p, "CILEVEL", "between 0 and 100", |v| v > 0.0 && v < 100.0)? / 100.0;
            }
            "METHOD" => c.monte_carlo = choice(p, &["AGL", "MCA"])? == 1,
            "SEED" => {
                c.seed = if p.eat_keyword("RANDOM") {
                    None
                } else {
                    Some(whole(p, "SEED", 1.0, 2_147_483_647.0)? as u64)
                };
            }
            "TOL" => {
                c.tol = checked(p, "TOL", "above 0 and at most 0.001", |v| {
                    v > 0.0 && v <= 1e-3
                })?;
            }
            "MAXITER" => c.maxiter = whole(p, "MAXITER", 1.0, 1e9)?,
            "MCSAMPLES" => c.mcsamples = whole(p, "MCSAMPLES", 1e3, 1e6)?,
            _ => c.postsamples = whole(p, "POSTSAMPLES", 1e3, 1e6)?,
        }
        Ok(())
    })
}

/// MISSING SCOPE = first | LISTWISE; true for LISTWISE.
fn missing(p: &mut Parser, first: &str) -> PResult<bool> {
    let mut listwise = false;
    settings(p, &["SCOPE"], |p, _| {
        listwise = choice(p, &[first, "LISTWISE"])? == 1;
        Ok(())
    })?;
    Ok(listwise)
}

/// VARIANCE, NULLVALUE and ALTERGVALUE; false when the keyword is none of them.
fn mean_test(p: &mut Parser, keyword: &str, test: &mut MeanTest) -> PResult<bool> {
    match keyword {
        "VARIANCE" => test.variance = Some(checked(p, "VARIANCE", "above 0", |v| v > 0.0)?),
        "NULLVALUE" => test.null = number(p, "NULLVALUE")?,
        "ALTERGVALUE" => test.g = Some(checked(p, "ALTERGVALUE", "above 0", |v| v > 0.0)?),
        _ => return Ok(false),
    }
    Ok(true)
}

const MEAN_TEST: [&str; 3] = ["VARIANCE", "NULLVALUE", "ALTERGVALUE"];

fn data(p: &mut Parser) -> PResult<BayesData> {
    let mut data = BayesData {
        variables: Vec::new(),
        test: MeanTest {
            variance: None,
            null: 0.0,
            g: None,
        },
        success: None,
        null_prior: None,
        alter_prior: None,
    };
    let mut keywords = vec!["VARIABLES", "SUCCESS", "NULLPRIORPARAM", "ALTERPRIORPARAM"];
    keywords.extend(MEAN_TEST);
    let start = p.pos;
    settings(p, &keywords.clone(), |p, i| {
        match keywords[i] {
            "VARIABLES" => data.variables = variables(p)?,
            "SUCCESS" => {
                let options = ["LAST", "FIRST", "MIDPOINT", "CUTPOINT", "LEVEL"];
                data.success = Some(match choice(p, &options)? {
                    0 => Success::Last,
                    1 => Success::First,
                    2 => Success::Midpoint,
                    3 => {
                        p.expect(Punct::LParen)?;
                        let cut = number(p, "a cut point")?;
                        p.expect(Punct::RParen)?;
                        Success::Cutpoint(cut)
                    }
                    _ => Success::Level(levels(p)?),
                });
            }
            "NULLPRIORPARAM" => data.null_prior = Some(two_numbers(p, "NULLPRIORPARAM")?),
            "ALTERPRIORPARAM" => data.alter_prior = Some(two_numbers(p, "ALTERPRIORPARAM")?),
            keyword => {
                mean_test(p, keyword, &mut data.test)?;
            }
        }
        Ok(())
    })?;
    if data.variables.is_empty() {
        return error(p, start, "DATA needs VARIABLES = a list of variables");
    }
    Ok(data)
}

fn normal_prior() -> NormalPrior {
    NormalPrior {
        variance: VariancePrior::Diffuse,
        mean: None,
        kappa: 1.0,
        conjugate: None,
    }
}

/// PRIOR of ONESAMPLE and RELATED.
fn prior(p: &mut Parser, prior: &mut NormalPrior, with_type: bool) -> PResult<()> {
    let mut keywords = vec!["VARDIST", "PRECDIST", "MEANDIST", "KAPPA"];
    if with_type {
        keywords.push("TYPE");
    }
    let (mut vardist, mut precdist) = (false, false);
    let start = p.pos;
    settings(p, &keywords.clone(), |p, i| {
        match keywords[i] {
            "VARDIST" => {
                vardist = true;
                let options = ["DIFFUSE", "INVCHISQ", "INVGAMMA", "JEFFREYS2", "JEFFREYS4"];
                prior.variance = match choice(p, &options)? {
                    0 => VariancePrior::Diffuse,
                    1 => {
                        let (df, scale) = pair_in_parens(p, "INVCHISQ")?;
                        positive_pair(p, "INVCHISQ", df, scale)?;
                        VariancePrior::InvChiSquare(df, scale)
                    }
                    2 => {
                        let (shape, scale) = pair_in_parens(p, "INVGAMMA")?;
                        positive_pair(p, "INVGAMMA", shape, scale)?;
                        VariancePrior::InvGamma(shape, scale)
                    }
                    3 => VariancePrior::Jeffreys2,
                    _ => VariancePrior::Jeffreys4,
                };
            }
            "PRECDIST" => {
                precdist = true;
                prior.variance = match choice(p, &["GAMMA", "CHISQ"])? {
                    0 => {
                        let (shape, rate) = pair_in_parens(p, "GAMMA")?;
                        positive_pair(p, "GAMMA", shape, rate)?;
                        VariancePrior::Gamma(shape, rate)
                    }
                    _ => {
                        let v = numbers_in_parens(p, "CHISQ", 1, 1)?;
                        if v.is_empty() || v[0] <= 0.0 {
                            return error(p, start, "CHISQ needs one df above 0 in parentheses");
                        }
                        VariancePrior::ChiSquare(v[0])
                    }
                };
            }
            "MEANDIST" => {
                prior.mean = match choice(p, &["DIFFUSE", "NORMAL"])? {
                    0 => None,
                    _ => {
                        let v = numbers_in_parens(p, "NORMAL", 1, 2)?;
                        if v.is_empty() {
                            return Err(p.unexpected(&["'('"]));
                        }
                        if v.get(1).is_some_and(|&s| s <= 0.0) {
                            return error(p, start, "the variance of NORMAL must be above 0");
                        }
                        Some((v[0], v.get(1).copied()))
                    }
                };
            }
            "KAPPA" => prior.kappa = checked(p, "KAPPA", "above 0", |v| v > 0.0)?,
            _ => {
                if choice(p, &["CONJUGATE"])? == 0 {
                    prior.conjugate = Some(pair_in_parens(p, "CONJUGATE")?);
                }
            }
        }
        Ok(())
    })?;
    if vardist && precdist {
        return error(p, start, "PRIOR takes VARDIST or PRECDIST, not both");
    }
    Ok(())
}

fn positive_pair(p: &Parser, what: &str, a: f64, b: f64) -> PResult<()> {
    if a > 0.0 && b > 0.0 {
        Ok(())
    } else {
        error(
            p,
            p.pos - 1,
            format!("both values of {what} must be above 0"),
        )
    }
}

/// Checks the two prior parameters of a beta (binomial) or gamma (Poisson) distribution.
fn conjugate_ok(binomial: bool, (a, b): (f64, f64)) -> bool {
    if binomial {
        (a > 0.0 && b > 0.0) || (a == 0.0 && b == 0.0)
    } else {
        a > 0.0 && b >= 0.0
    }
}

/// Every subcommand of a BAYES command: `handle` gets the parser after the subcommand name and
/// reports whether it knew the name.
fn subcommands(
    p: &mut Parser,
    command: &str,
    names: &[&str],
    mut handle: impl FnMut(&mut Parser, usize) -> PResult<()>,
) -> PResult<()> {
    p.eat(Punct::Slash);
    loop {
        if p.peek().is_none() {
            break;
        }
        let Some(i) = which(p, names)? else {
            return Err(p.subcommand_error(command, &[]));
        };
        p.eat(Punct::Eq);
        handle(p, i)?;
        if p.peek().is_none() {
            break;
        }
        p.expect(Punct::Slash)?;
    }
    p.finish(&["'/'"])
}

pub(crate) fn onesample(p: &mut Parser) -> PResult<Command> {
    let mut c = BayesOneSample {
        listwise: false,
        criteria: default_criteria(1e-6),
        distribution: BayesDistribution::Normal,
        variables: Vec::new(),
        analysis: BayesAnalysis::Posterior,
        data: Vec::new(),
        prior: normal_prior(),
    };
    let mut distribution = None;
    let start = p.pos;
    let names = ["MISSING", "CRITERIA", "INFERENCE", "DATA", "PRIOR"];
    subcommands(p, "BAYES ONESAMPLE", &names, |p, i| match i {
        0 => {
            c.listwise = missing(p, "ANALYSIS")?;
            Ok(())
        }
        1 => {
            let allowed = ["CILEVEL", "METHOD", "SEED", "TOL", "MAXITER", "MCSAMPLES"];
            criteria(p, &mut c.criteria, &allowed)
        }
        2 => settings(p, &["DISTRIBUTION", "VARIABLES", "ANALYSIS"], |p, k| {
            match k {
                0 => {
                    distribution = Some(match choice(p, &["NORMAL", "BINOMIAL", "POISSON"])? {
                        0 => BayesDistribution::Normal,
                        1 => BayesDistribution::Binomial,
                        _ => BayesDistribution::Poisson,
                    });
                }
                1 => c.variables = variables(p)?,
                _ => c.analysis = analysis(p)?,
            }
            Ok(())
        }),
        3 => {
            c.data.push(data(p)?);
            Ok(())
        }
        _ => prior(p, &mut c.prior, true),
    })?;
    let Some(distribution) = distribution else {
        return error(
            p,
            start,
            "BAYES ONESAMPLE needs /INFERENCE DISTRIBUTION = NORMAL, BINOMIAL or POISSON",
        );
    };
    c.distribution = distribution;
    if c.variables.is_empty() {
        return error(
            p,
            start,
            "BAYES ONESAMPLE needs /INFERENCE VARIABLES = a list of variables",
        );
    }
    let normal = distribution == BayesDistribution::Normal;
    let binomial = distribution == BayesDistribution::Binomial;
    for d in &c.data {
        let wrong = if normal {
            [
                (d.success.is_some(), "SUCCESS"),
                (d.null_prior.is_some(), "NULLPRIORPARAM"),
                (d.alter_prior.is_some(), "ALTERPRIORPARAM"),
            ]
            .into_iter()
            .find(|w| w.0)
        } else {
            [
                (d.test.variance.is_some(), "VARIANCE"),
                (d.test.g.is_some(), "ALTERGVALUE"),
                (d.test.null != 0.0, "NULLVALUE"),
                (d.success.is_some() && !binomial, "SUCCESS"),
            ]
            .into_iter()
            .find(|w| w.0)
        };
        if let Some((_, keyword)) = wrong {
            let message = format!(
                "DATA {keyword} does not apply to DISTRIBUTION={}",
                ["NORMAL", "BINOMIAL", "POISSON"][distribution as usize]
            );
            return error(p, start, message);
        }
        for (pair, name) in [
            (d.null_prior, "NULLPRIORPARAM"),
            (d.alter_prior, "ALTERPRIORPARAM"),
        ] {
            // the Poisson priors of the Bayes factor need a positive rate as well
            if let Some(pair) = pair
                && !(conjugate_ok(binomial, pair) && (binomial || pair.1 > 0.0))
            {
                let rule = if binomial {
                    "both above 0, or both 0"
                } else {
                    "both above 0"
                };
                return error(p, start, format!("the values of {name} must be {rule}"));
            }
        }
    }
    if let Some(pair) = c.prior.conjugate {
        if normal {
            return error(
                p,
                start,
                "PRIOR TYPE applies only to DISTRIBUTION=BINOMIAL or POISSON",
            );
        }
        if !conjugate_ok(binomial, pair) {
            let rule = if binomial {
                "both above 0, or both 0"
            } else {
                "the first above 0 and the second at least 0"
            };
            return error(p, start, format!("the values of CONJUGATE must be {rule}"));
        }
    }
    Ok(Command::Bayes(Box::new(Bayes::OneSample(c))))
}

pub(crate) fn related(p: &mut Parser) -> PResult<Command> {
    let mut c = BayesRelated {
        listwise: false,
        criteria: default_criteria(1e-6),
        analysis: BayesAnalysis::Posterior,
        pairs: Vec::new(),
        prior: normal_prior(),
    };
    let start = p.pos;
    let names = ["MISSING", "CRITERIA", "INFERENCE", "PAIR", "PRIOR"];
    subcommands(p, "BAYES RELATED", &names, |p, i| match i {
        0 => {
            c.listwise = missing(p, "ANALYSIS")?;
            Ok(())
        }
        1 => {
            let allowed = ["CILEVEL", "METHOD", "SEED", "TOL", "MAXITER", "MCSAMPLES"];
            criteria(p, &mut c.criteria, &allowed)
        }
        2 => settings(p, &["DISTRIBUTION", "ANALYSIS"], |p, k| {
            if k == 0 {
                choice(p, &["NORMAL"])?;
            } else {
                c.analysis = analysis(p)?;
            }
            Ok(())
        }),
        3 => {
            let pair_start = p.pos;
            let mut names = Vec::new();
            let mut test = MeanTest {
                variance: None,
                null: 0.0,
                g: None,
            };
            let mut keywords = vec!["VARIABLES"];
            keywords.extend(MEAN_TEST);
            settings(p, &keywords.clone(), |p, k| {
                if k == 0 {
                    names = variables(p)?;
                } else {
                    mean_test(p, keywords[k], &mut test)?;
                }
                Ok(())
            })?;
            let [VarItem::Name(first), VarItem::Name(second)] = names.as_slice() else {
                return error(p, pair_start, "PAIR needs VARIABLES = two variable names");
            };
            c.pairs.push(BayesPair {
                first: first.clone(),
                second: second.clone(),
                test,
            });
            Ok(())
        }
        _ => prior(p, &mut c.prior, false),
    })?;
    if c.pairs.is_empty() {
        return error(
            p,
            start,
            "BAYES RELATED needs at least one /PAIR VARIABLES = two variables",
        );
    }
    Ok(Command::Bayes(Box::new(Bayes::Related(c))))
}

pub(crate) fn independent(p: &mut Parser) -> PResult<Command> {
    let mut c = BayesIndependent {
        listwise: false,
        criteria: default_criteria(1e-6),
        variables: Vec::new(),
        group: String::new(),
        select: GroupSelect::Midpoint,
        analysis: BayesAnalysis::Posterior,
        datavar: None,
        factor: IndependentFactor::Rouder(1.0),
        equal_variances: false,
        variance: GroupVariancePrior::Diffuse,
        mean: None,
    };
    let mut select = None;
    let start = p.pos;
    let names = ["MISSING", "CRITERIA", "INFERENCE", "ESTBF", "PRIOR"];
    subcommands(p, "BAYES INDEPENDENT", &names, |p, i| match i {
        0 => {
            c.listwise = missing(p, "ANALYSIS")?;
            Ok(())
        }
        1 => criteria(p, &mut c.criteria, &["CILEVEL", "TOL", "MAXITER"]),
        2 => {
            let keywords = [
                "DISTRIBUTION",
                "VARIABLES",
                "GROUP",
                "SELECT",
                "ANALYSIS",
                "DATAVAR",
            ];
            settings(p, &keywords, |p, k| {
                match k {
                    0 => {
                        choice(p, &["NORMAL"])?;
                    }
                    1 => c.variables = variables(p)?,
                    2 => c.group = p.name("a grouping variable")?,
                    3 => {
                        select = Some(match choice(p, &["LEVEL", "MIDPOINT", "CUTPOINT"])? {
                            0 => {
                                let at = p.pos;
                                let values = levels(p)?;
                                let [a, b] = values.as_slice() else {
                                    return error(p, at, "LEVEL takes two values");
                                };
                                GroupSelect::Levels(a.clone(), b.clone())
                            }
                            1 => GroupSelect::Midpoint,
                            _ => {
                                p.expect(Punct::LParen)?;
                                let cut = number(p, "a cut point")?;
                                p.expect(Punct::RParen)?;
                                GroupSelect::Cutpoint(cut)
                            }
                        });
                    }
                    4 => c.analysis = analysis(p)?,
                    _ => {
                        let (a, b) = two_numbers(p, "DATAVAR")?;
                        positive_pair(p, "DATAVAR", a, b)?;
                        c.datavar = Some((a, b));
                    }
                }
                Ok(())
            })
        }
        3 => settings(p, &["COMPUTATION"], |p, _| {
            let at = p.pos;
            c.factor = match choice(p, &["ROUDER", "GONEN", "HYPER"])? {
                0 => {
                    let v = numbers_in_parens(p, "ROUDER", 1, 1)?;
                    let scale = v.first().copied().unwrap_or(1.0);
                    if scale <= 0.0 {
                        return error(p, at, "the scale of ROUDER must be above 0");
                    }
                    IndependentFactor::Rouder(scale)
                }
                1 => {
                    let v = numbers_in_parens(p, "GONEN", 1, 2)?;
                    let (mean, variance) = match v.as_slice() {
                        [variance] => (0.0, *variance),
                        [mean, variance] => (*mean, *variance),
                        _ => return Err(p.unexpected(&["'('"])),
                    };
                    if variance <= 0.0 {
                        return error(p, at, "the variance of GONEN must be above 0");
                    }
                    IndependentFactor::Gonen(mean, variance)
                }
                _ => {
                    let v = numbers_in_parens(p, "HYPER", 1, 1)?;
                    let a = v.first().copied().unwrap_or(-0.75);
                    if !(a > -1.0 && a <= -0.5) {
                        return error(
                            p,
                            at,
                            "the value of HYPER must be above -1 and at most -0.5",
                        );
                    }
                    IndependentFactor::Hyper(a)
                }
            };
            Ok(())
        }),
        _ => settings(p, &["EQUALDATAVAR", "VARDIST", "MEANDIST"], |p, k| {
            match k {
                0 => c.equal_variances = choice(p, &["FALSE", "TRUE"])? == 1,
                1 => {
                    c.variance = match choice(p, &["DIFFUSE", "JEFFREYS", "INVCHISQ"])? {
                        0 => GroupVariancePrior::Diffuse,
                        1 => GroupVariancePrior::Jeffreys,
                        _ => {
                            let (df, scale) = pair_in_parens(p, "INVCHISQ")?;
                            positive_pair(p, "INVCHISQ", df, scale)?;
                            GroupVariancePrior::InvChiSquare(df, scale)
                        }
                    };
                }
                _ => {
                    c.mean = match choice(p, &["DIFFUSE", "NORMAL"])? {
                        0 => None,
                        _ => {
                            let at = p.pos;
                            if !p.at(Punct::LParen) {
                                return Err(p.unexpected(&["'('"]));
                            }
                            let v = numbers_in_parens(p, "NORMAL", 4, 4)?;
                            if v[2] <= 0.0 || v[3] <= 0.0 {
                                return error(p, at, "the variances of NORMAL must be above 0");
                            }
                            Some([v[0], v[1], v[2], v[3]])
                        }
                    };
                }
            }
            Ok(())
        }),
    })?;
    if c.variables.is_empty() {
        return error(
            p,
            start,
            "BAYES INDEPENDENT needs /INFERENCE VARIABLES = a list of variables",
        );
    }
    if c.group.is_empty() {
        return error(
            p,
            start,
            "BAYES INDEPENDENT needs /INFERENCE GROUP = a grouping variable",
        );
    }
    let Some(select) = select else {
        return error(
            p,
            start,
            "BAYES INDEPENDENT needs /INFERENCE SELECT = LEVEL(a b), MIDPOINT or CUTPOINT(value)",
        );
    };
    c.select = select;
    Ok(Command::Bayes(Box::new(Bayes::Independent(c))))
}

pub(crate) fn correlation(p: &mut Parser) -> PResult<Command> {
    let mut c = BayesCorrelation {
        listwise: false,
        criteria: default_criteria(1e-4),
        variables: Vec::new(),
        analysis: BayesAnalysis::Posterior,
        c: 0.0,
        maxplots: 10,
        fraction: None,
        null: 0.0,
    };
    let start = p.pos;
    let names = ["MISSING", "CRITERIA", "INFERENCE", "ESTBF"];
    subcommands(p, "BAYES CORRELATION", &names, |p, i| match i {
        0 => {
            c.listwise = missing(p, "PAIRWISE")?;
            Ok(())
        }
        1 => {
            let allowed = [
                "CILEVEL",
                "SEED",
                "TOL",
                "MAXITER",
                "MCSAMPLES",
                "POSTSAMPLES",
            ];
            criteria(p, &mut c.criteria, &allowed)
        }
        2 => settings(
            p,
            &["VARIABLES", "ANALYSIS", "CVALUE", "MAXPLOTS"],
            |p, k| {
                match k {
                    0 => c.variables = variables(p)?,
                    1 => c.analysis = analysis(p)?,
                    2 => c.c = number(p, "CVALUE")?,
                    _ => c.maxplots = whole(p, "MAXPLOTS", 0.0, 50.0)?,
                }
                Ok(())
            },
        ),
        _ => settings(p, &["TYPE", "NULLVALUE"], |p, k| {
            let at = p.pos;
            if k == 0 {
                c.fraction = match choice(p, &["JZS", "FRACTION"])? {
                    0 => None,
                    _ => {
                        let v = numbers_in_parens(p, "FRACTION", 1, 1)?;
                        let b = v.first().copied().unwrap_or(0.5);
                        if !(b > 0.0 && b < 1.0) {
                            return error(p, at, "the value of FRACTION must lie between 0 and 1");
                        }
                        Some(b)
                    }
                };
            } else {
                c.null = checked(p, "NULLVALUE", "between -1 and 1", |v| v > -1.0 && v < 1.0)?;
            }
            Ok(())
        }),
    })?;
    if c.variables.is_empty() {
        return error(
            p,
            start,
            "BAYES CORRELATION needs /INFERENCE VARIABLES = two or more variables",
        );
    }
    Ok(Command::Bayes(Box::new(Bayes::Correlation(c))))
}
