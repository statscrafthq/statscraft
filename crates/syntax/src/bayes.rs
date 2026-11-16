//! Parsers of the Bayesian commands BAYES ONESAMPLE, BAYES RELATED, BAYES INDEPENDENT,
//! BAYES CORRELATION, BAYES ANOVA, BAYES REGRESSION, BAYES REPEATED and BAYES LOGLINEAR.
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
//! - ANOVA: `dependent BY factor` before the subcommands. DESIGN REGWGT names one regression
//!   weight. ESTBF COMPUTATION=JZS (default), ZELLNER(g above 0, required), HYPER(a above 2, 3)
//!   or ROUDER(scale above 0, 1). PRIOR TYPE=REFERENCE (default) or CONJUGATE with SHAPEPARAM
//!   and SCALEPARAM (above 0), MEANVECTOR (numbers) and VMATRIX=IDENTITY or the lower triangle,
//!   rows ending with `;`, row r holding r values with a positive last one. PLOT MEANS names
//!   factor values, ERRORVAR=FALSE|TRUE.
//! - REGRESSION: `dependent [BY factors] [WITH covariates]`, at least one of them. DESIGN
//!   FACTORS and COVARIATES pick sublists for the model. ESTBF adds COMPARE=NULL|FULL. PREDICT
//!   REGRESSORS gives one value per regressor. SAVE MEANPRED, VARIANCE, MODE, CIMEANPREDL and
//!   CIMEANPREDU, each with an optional (name). OUTFILE PARAMETER='file'. PLOT COVARIATES,
//!   FACTORS, MAX_CATEGORY (above 0, 2), INTERCEPT, ERRORVAR, BAYESPRED (FALSE|TRUE; BAYESPRED
//!   needs PREDICT). FTEST, repeatable: LABEL, VARIABLES, INTERCEPT (FALSE; it must be TRUE
//!   when VARIABLES is left out), TESTVALUES.
//! - REPEATED: a list of two or more variables, CRITERIA CILEVEL, SEED, MCSAMPLES (30000), DESIGN
//!   REGWGT, ESTBF COMPUTATION=BIC|ROUDER, PLOT MEANS=sublist.
//! - LOGLINEAR: TABLE row BY column (two different variables, required); CRITERIA CILEVEL,
//!   SEED, TOL, MAXITER, POSTSAMPLES and ORDER=ASCENDING|DESCENDING; ESTBF MODEL=MULTINOMIAL,
//!   POISSON or NONPARAMETRIC, FIX_MARGIN=TOTAL (also when bare), ROW or COLUMN (NONPARAMETRIC
//!   needs ROW or COLUMN), PRIOR_TYPE=CONJUGATE, MIXTURE_DIRICHLET or INTRINSIC, SHAPEPARAMS
//!   (above 0, rows split by `;`), SCALEPARAM=DATA or a value above 0, LAMBDAS (above 0); PRINT
//!   SUPPRESS_TABLE=FALSE|TRUE, CELLS (OBSERVED, ROW, COLUMN, EXPECTED, TOTAL, ALL) and
//!   STATISTICS (CHISQ, LRT; CHISQ when empty).
//! - A keyword with an underscore may be written as two words (MAX CATEGORY) or without the
//!   underscore, and a final S does not matter (FIXMARGINS).
//!
//! Choices where the manual is silent: PRIOR may be left out (its defaults apply); a variable
//! on two DATA subcommands is an error; DISTRIBUTION=NORMAL is accepted on RELATED and
//! INDEPENDENT; LEVEL values may be numbers, quoted strings or bare words; MAXITER of BAYES
//! ANOVA defaults to 2000, as its text says (its diagram shows 500).

use crate::ast::{Command, CrosstabsCell, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
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
    /// ORDER=DESCENDING: categories from the highest value.
    pub descending: bool,
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

/// ESTBF COMPUTATION of BAYES ANOVA and BAYES REGRESSION: the prior on g of the g-prior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GPrior {
    /// Zellner and Siow: g is inverse gamma (1/2, N/2).
    Jzs,
    /// A fixed g.
    Zellner(f64),
    /// The hyper-g prior with parameter a.
    Hyper(f64),
    /// g is inverse gamma (1/2, N s^2 / 2) with the scale s.
    Rouder(f64),
}

/// PRIOR of BAYES ANOVA and BAYES REGRESSION.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearPrior {
    /// TYPE=CONJUGATE; false for REFERENCE.
    pub conjugate: bool,
    pub shape: Option<f64>,
    pub scale: Option<f64>,
    pub means: Vec<f64>,
    /// The rows of the lower triangle of VMATRIX; empty for IDENTITY.
    pub vmatrix: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesAnova {
    pub dependent: String,
    pub factor: String,
    pub criteria: BayesCriteria,
    pub regwgt: Option<String>,
    pub analysis: BayesAnalysis,
    pub g: GPrior,
    pub prior: LinearPrior,
    /// PLOT MEANS: the factor values whose group means are drawn.
    pub plot_means: Vec<Value>,
    pub plot_error: bool,
}

/// A statistic of the predictive distribution that SAVE writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictStat {
    Mean,
    Variance,
    Mode,
    Lower,
    Upper,
}

impl PredictStat {
    pub const KEYWORDS: [&str; 5] = ["MEANPRED", "VARIANCE", "MODE", "CIMEANPREDL", "CIMEANPREDU"];
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionPlot {
    pub covariates: Vec<VarItem>,
    pub factors: Vec<VarItem>,
    pub max_category: usize,
    pub intercept: bool,
    pub error: bool,
    pub predictive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FTest {
    pub label: Option<String>,
    pub variables: Vec<VarItem>,
    pub intercept: bool,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesRegression {
    pub dependent: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub criteria: BayesCriteria,
    pub regwgt: Option<String>,
    /// DESIGN FACTORS and COVARIATES; None keeps the whole list.
    pub design_factors: Option<Vec<VarItem>>,
    pub design_covariates: Option<Vec<VarItem>>,
    pub analysis: BayesAnalysis,
    pub g: GPrior,
    /// COMPARE=FULL: Bayes factors against the full model.
    pub compare_full: bool,
    pub prior: LinearPrior,
    /// PREDICT REGRESSORS.
    pub predict: Vec<Value>,
    pub save: Vec<(PredictStat, Option<String>)>,
    pub outfile: Option<String>,
    pub plot: RegressionPlot,
    pub ftests: Vec<FTest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesRepeated {
    pub variables: Vec<VarItem>,
    pub criteria: BayesCriteria,
    pub analysis: BayesAnalysis,
    pub regwgt: Option<String>,
    /// ESTBF COMPUTATION=ROUDER; false for BIC.
    pub rouder: bool,
    pub plot_means: Vec<VarItem>,
}

/// ESTBF MODEL of BAYES LOGLINEAR: how the counts were sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountModel {
    Multinomial,
    Poisson,
    Nonparametric,
}

/// ESTBF FIX_MARGIN: the totals fixed by the design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedMargin {
    Total,
    Row,
    Column,
}

/// ESTBF PRIOR_TYPE of BAYES LOGLINEAR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePrior {
    Conjugate,
    MixtureDirichlet,
    Intrinsic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BayesLoglinear {
    pub row: String,
    pub column: String,
    pub criteria: BayesCriteria,
    pub analysis: BayesAnalysis,
    pub model: CountModel,
    pub margin: FixedMargin,
    pub prior: TablePrior,
    /// SHAPEPARAMS: one value for every cell, or one row of values per table row.
    pub shapes: Vec<Vec<f64>>,
    /// SCALEPARAM; None for DATA.
    pub scale: Option<f64>,
    pub lambdas: Vec<f64>,
    pub suppress_table: bool,
    pub cells: Vec<CrosstabsCell>,
    pub chisq: bool,
    pub lrt: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bayes {
    OneSample(BayesOneSample),
    Related(BayesRelated),
    Independent(BayesIndependent),
    Correlation(BayesCorrelation),
    Anova(BayesAnova),
    Regression(BayesRegression),
    Repeated(BayesRepeated),
    Loglinear(BayesLoglinear),
}

impl Bayes {
    pub fn name(&self) -> &'static str {
        match self {
            Bayes::OneSample(_) => "BAYES ONESAMPLE",
            Bayes::Related(_) => "BAYES RELATED",
            Bayes::Independent(_) => "BAYES INDEPENDENT",
            Bayes::Correlation(_) => "BAYES CORRELATION",
            Bayes::Anova(_) => "BAYES ANOVA",
            Bayes::Regression(_) => "BAYES REGRESSION",
            Bayes::Repeated(_) => "BAYES REPEATED",
            Bayes::Loglinear(_) => "BAYES LOGLINEAR",
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
    // underscores and one final S do not matter: FIXMARGINS is FIX_MARGIN
    if let Some(i) = options.iter().position(|o| squash(word) == squash(o)) {
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

/// A keyword without underscores and without one final S, in capitals.
fn squash(word: &str) -> String {
    let word = word.replace('_', "").to_uppercase();
    match word.strip_suffix('S') {
        Some(stem) if stem.len() >= 3 => stem.to_string(),
        _ => word,
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

/// Keywords that may be written as two words.
const TWO_WORDS: [&str; 1] = ["MAX_CATEGORY"];

/// A variable list that ends before the next `KEYWORD =`.
fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(word)) = p.peek() {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
            break;
        }
        // the first word of a keyword written as two words, such as MAX CATEGORY =
        if let (Some(TokenKind::Word(next)), Some(TokenKind::Punct(Punct::Eq))) =
            (p.peek_at(1), p.peek_at(2))
            && TWO_WORDS
                .iter()
                .any(|k| k.split_once('_') == Some((&word.to_uppercase(), &next.to_uppercase())))
        {
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
        // a keyword with an underscore may be written as two words, such as MAX CATEGORY
        if let Some((_, rest)) = keywords[i].split_once('_')
            && let Some(TokenKind::Word(word)) = p.peek()
            && (squash(word) == squash(rest) || matches_keyword(word, rest))
        {
            p.pos += 1;
        }
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
        descending: false,
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
            // TRUE and FALSE as the keyword's description words it
            "ORDER" => {
                c.descending = choice(p, &["ASCENDING", "DESCENDING", "TRUE", "FALSE"])? % 2 == 1
            }
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

fn boolean(p: &mut Parser) -> PResult<bool> {
    Ok(choice(p, &["FALSE", "TRUE"])? == 1)
}

/// Numbers up to the next keyword or slash.
fn number_list(p: &mut Parser) -> Vec<f64> {
    let mut values = Vec::new();
    while let Some(v) = p.number() {
        values.push(v);
        p.eat(Punct::Comma);
    }
    values
}

/// Values (numbers, quoted strings or bare words) up to the next `KEYWORD =` or slash.
fn value_list(p: &mut Parser) -> PResult<Vec<Value>> {
    let mut values = Vec::new();
    while !at_end(p) {
        if matches!(p.peek(), Some(TokenKind::Word(_)))
            && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
        {
            break;
        }
        let Some(v) = level_value(p) else {
            return Err(p.unexpected(&["a value"]));
        };
        values.push(v);
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    Ok(values)
}

fn g_prior(p: &mut Parser) -> PResult<GPrior> {
    let at = p.pos;
    let kind = choice(p, &["JZS", "ZELLNER", "HYPER", "ROUDER"])?;
    if kind == 0 {
        return Ok(GPrior::Jzs);
    }
    let name = ["JZS", "ZELLNER", "HYPER", "ROUDER"][kind];
    let v = numbers_in_parens(p, name, 1, 1)?;
    Ok(match (kind, v.first().copied()) {
        (1, None) => return error(p, at, "ZELLNER needs a value of g in parentheses"),
        (1, Some(g)) if g > 0.0 => GPrior::Zellner(g),
        (2, a) if a.unwrap_or(3.0) > 2.0 => GPrior::Hyper(a.unwrap_or(3.0)),
        (3, s) if s.unwrap_or(1.0) > 0.0 => GPrior::Rouder(s.unwrap_or(1.0)),
        _ => {
            let rule = if kind == 2 { "above 2" } else { "above 0" };
            return error(p, at, format!("the value of {name} must be {rule}"));
        }
    })
}

fn linear_prior() -> LinearPrior {
    LinearPrior {
        conjugate: false,
        shape: None,
        scale: None,
        means: Vec::new(),
        vmatrix: Vec::new(),
    }
}

/// PRIOR of ANOVA and REGRESSION.
fn linear_prior_settings(p: &mut Parser, prior: &mut LinearPrior) -> PResult<()> {
    let keywords = ["TYPE", "SHAPEPARAM", "SCALEPARAM", "MEANVECTOR", "VMATRIX"];
    settings(p, &keywords, |p, k| {
        match k {
            0 => prior.conjugate = choice(p, &["REFERENCE", "CONJUGATE"])? == 1,
            1 => prior.shape = Some(checked(p, "SHAPEPARAM", "above 0", |v| v > 0.0)?),
            2 => prior.scale = Some(checked(p, "SCALEPARAM", "above 0", |v| v > 0.0)?),
            3 => {
                prior.means = number_list(p);
                if prior.means.is_empty() {
                    return Err(p.unexpected(&["a number"]));
                }
            }
            _ => {
                prior.vmatrix = Vec::new();
                if p.eat_keyword("IDENTITY") {
                    return Ok(());
                }
                let at = p.pos;
                loop {
                    let row = number_list(p);
                    if row.is_empty() {
                        break;
                    }
                    let r = prior.vmatrix.len() + 1;
                    if row.len() != r || row[r - 1] <= 0.0 {
                        return error(
                            p,
                            at,
                            format!(
                                "row {r} of VMATRIX must hold {r} values, the last above 0, and end with ';'"
                            ),
                        );
                    }
                    prior.vmatrix.push(row);
                    if !p.eat(Punct::Semicolon) {
                        break;
                    }
                }
                if prior.vmatrix.is_empty() {
                    return Err(p.unexpected(&["IDENTITY", "a number"]));
                }
            }
        }
        Ok(())
    })
}

/// The settings TYPE=CONJUGATE needs.
fn check_conjugate(p: &Parser, start: usize, prior: &LinearPrior) -> PResult<()> {
    if prior.conjugate && (prior.shape.is_none() || prior.scale.is_none() || prior.means.is_empty())
    {
        return error(
            p,
            start,
            "PRIOR TYPE=CONJUGATE needs SHAPEPARAM, SCALEPARAM and MEANVECTOR",
        );
    }
    Ok(())
}

fn regwgt(p: &mut Parser, target: &mut Option<String>) -> PResult<()> {
    *target = Some(p.name("a regression weight variable")?);
    Ok(())
}

fn linear_criteria() -> BayesCriteria {
    default_criteria(1e-6)
}

pub(crate) fn anova(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let dependent = p.name("the dependent variable")?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let factor = p.name("the factor")?;
    let mut c = BayesAnova {
        dependent,
        factor,
        criteria: linear_criteria(),
        regwgt: None,
        analysis: BayesAnalysis::Posterior,
        g: GPrior::Jzs,
        prior: linear_prior(),
        plot_means: Vec::new(),
        plot_error: false,
    };
    let names = ["CRITERIA", "DESIGN", "INFERENCE", "ESTBF", "PRIOR", "PLOT"];
    subcommands(p, "BAYES ANOVA", &names, |p, i| match i {
        0 => criteria(p, &mut c.criteria, &["CILEVEL", "TOL", "MAXITER"]),
        1 => settings(p, &["REGWGT"], |p, _| regwgt(p, &mut c.regwgt)),
        2 => settings(p, &["ANALYSIS"], |p, _| {
            c.analysis = analysis(p)?;
            Ok(())
        }),
        3 => settings(p, &["COMPUTATION"], |p, _| {
            c.g = g_prior(p)?;
            Ok(())
        }),
        4 => linear_prior_settings(p, &mut c.prior),
        _ => settings(p, &["MEANS", "ERRORVAR"], |p, k| {
            if k == 0 {
                c.plot_means = value_list(p)?;
            } else {
                c.plot_error = boolean(p)?;
            }
            Ok(())
        }),
    })?;
    check_conjugate(p, start, &c.prior)?;
    Ok(Command::Bayes(Box::new(Bayes::Anova(c))))
}

pub(crate) fn regression(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let dependent = p.name("the dependent variable")?;
    let factors = if p.eat_word("BY") {
        list_until(p, &["WITH"])?
    } else {
        Vec::new()
    };
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    if factors.is_empty() && covariates.is_empty() {
        return Err(p.unexpected(&["BY", "WITH"]));
    }
    let mut c = BayesRegression {
        dependent,
        factors,
        covariates,
        criteria: linear_criteria(),
        regwgt: None,
        design_factors: None,
        design_covariates: None,
        analysis: BayesAnalysis::Posterior,
        g: GPrior::Jzs,
        compare_full: false,
        prior: linear_prior(),
        predict: Vec::new(),
        save: Vec::new(),
        outfile: None,
        plot: RegressionPlot {
            covariates: Vec::new(),
            factors: Vec::new(),
            max_category: 2,
            intercept: false,
            error: false,
            predictive: false,
        },
        ftests: Vec::new(),
    };
    let names = [
        "CRITERIA",
        "DESIGN",
        "INFERENCE",
        "ESTBF",
        "PRIOR",
        "PREDICT",
        "SAVE",
        "OUTFILE",
        "PLOT",
        "FTEST",
    ];
    subcommands(p, "BAYES REGRESSION", &names, |p, i| match i {
        0 => criteria(p, &mut c.criteria, &["CILEVEL", "TOL", "MAXITER"]),
        1 => settings(p, &["REGWGT", "FACTORS", "COVARIATES"], |p, k| {
            match k {
                0 => regwgt(p, &mut c.regwgt)?,
                1 => c.design_factors = Some(variables(p)?),
                _ => c.design_covariates = Some(variables(p)?),
            }
            Ok(())
        }),
        2 => settings(p, &["ANALYSIS"], |p, _| {
            c.analysis = analysis(p)?;
            Ok(())
        }),
        3 => settings(p, &["COMPUTATION", "COMPARE"], |p, k| {
            if k == 0 {
                c.g = g_prior(p)?;
            } else {
                c.compare_full = choice(p, &["NULL", "FULL"])? == 1;
            }
            Ok(())
        }),
        4 => linear_prior_settings(p, &mut c.prior),
        5 => settings(p, &["REGRESSORS"], |p, _| {
            c.predict = value_list(p)?;
            Ok(())
        }),
        6 => {
            while !at_end(p) {
                let stat = match choice(p, &PredictStat::KEYWORDS)? {
                    0 => PredictStat::Mean,
                    1 => PredictStat::Variance,
                    2 => PredictStat::Mode,
                    3 => PredictStat::Lower,
                    _ => PredictStat::Upper,
                };
                let name = if p.eat(Punct::LParen) {
                    let name = p.name("a variable name")?;
                    p.expect(Punct::RParen)?;
                    Some(name)
                } else {
                    None
                };
                c.save.push((stat, name));
                p.eat(Punct::Comma);
            }
            Ok(())
        }
        7 => settings(p, &["PARAMETER"], |p, _| {
            c.outfile = Some(p.file("a file name")?);
            Ok(())
        }),
        8 => {
            let keywords = [
                "COVARIATES",
                "FACTORS",
                "MAX_CATEGORY",
                "INTERCEPT",
                "ERRORVAR",
                "BAYESPRED",
            ];
            let plot = &mut c.plot;
            settings(p, &keywords, |p, k| {
                match k {
                    0 => plot.covariates = variables(p)?,
                    1 => plot.factors = variables(p)?,
                    2 => plot.max_category = whole(p, "MAX_CATEGORY", 1.0, 1e9)?,
                    3 => plot.intercept = boolean(p)?,
                    4 => plot.error = boolean(p)?,
                    _ => plot.predictive = boolean(p)?,
                }
                Ok(())
            })
        }
        _ => {
            let at = p.pos;
            let mut test = FTest {
                label: None,
                variables: Vec::new(),
                intercept: false,
                values: Vec::new(),
            };
            let keywords = ["LABEL", "VARIABLES", "INTERCEPT", "TESTVALUES"];
            settings(p, &keywords, |p, k| {
                match k {
                    0 => test.label = Some(p.string("a label in quotes")?),
                    1 => test.variables = variables(p)?,
                    2 => test.intercept = boolean(p)?,
                    _ => test.values = number_list(p),
                }
                Ok(())
            })?;
            if test.variables.is_empty() && !test.intercept {
                return error(p, at, "FTEST without VARIABLES needs INTERCEPT=TRUE");
            }
            c.ftests.push(test);
            Ok(())
        }
    })?;
    check_conjugate(p, start, &c.prior)?;
    if c.plot.predictive && c.predict.is_empty() {
        return error(p, start, "PLOT BAYESPRED=TRUE needs PREDICT REGRESSORS");
    }
    Ok(Command::Bayes(Box::new(Bayes::Regression(c))))
}

pub(crate) fn repeated(p: &mut Parser) -> PResult<Command> {
    let list = p.var_list()?;
    let mut c = BayesRepeated {
        variables: list,
        criteria: BayesCriteria {
            mcsamples: 30_000,
            ..default_criteria(1e-6)
        },
        analysis: BayesAnalysis::Posterior,
        regwgt: None,
        rouder: false,
        plot_means: Vec::new(),
    };
    let names = ["CRITERIA", "INFERENCE", "DESIGN", "ESTBF", "PLOT"];
    subcommands(p, "BAYES REPEATED", &names, |p, i| match i {
        0 => criteria(p, &mut c.criteria, &["CILEVEL", "SEED", "MCSAMPLES"]),
        1 => settings(p, &["ANALYSIS"], |p, _| {
            c.analysis = analysis(p)?;
            Ok(())
        }),
        2 => settings(p, &["REGWGT"], |p, _| regwgt(p, &mut c.regwgt)),
        3 => settings(p, &["COMPUTATION"], |p, _| {
            c.rouder = choice(p, &["BIC", "ROUDER"])? == 1;
            Ok(())
        }),
        _ => settings(p, &["MEANS"], |p, _| {
            c.plot_means = variables(p)?;
            Ok(())
        }),
    })?;
    Ok(Command::Bayes(Box::new(Bayes::Repeated(c))))
}

/// Positive numbers up to the next keyword, rows ending with `;`.
fn positive_rows(p: &mut Parser, what: &str) -> PResult<Vec<Vec<f64>>> {
    let at = p.pos;
    let mut rows = Vec::new();
    loop {
        let row = number_list(p);
        if row.is_empty() {
            break;
        }
        rows.push(row);
        if !p.eat(Punct::Semicolon) {
            break;
        }
    }
    if rows.is_empty() {
        return Err(p.unexpected(&["a number"]));
    }
    if rows.iter().flatten().any(|&v| v <= 0.0) {
        return error(p, at, format!("the values of {what} must be above 0"));
    }
    Ok(rows)
}

/// The keywords of `options` at the cursor, up to the first word that is none of them.
fn keyword_list(p: &mut Parser, options: &[&str]) -> PResult<Vec<usize>> {
    let mut found = Vec::new();
    while let Some(i) = which(p, options)? {
        found.push(i);
        p.eat(Punct::Comma);
    }
    Ok(found)
}

pub(crate) fn loglinear(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut c = BayesLoglinear {
        row: String::new(),
        column: String::new(),
        criteria: default_criteria(1e-6),
        analysis: BayesAnalysis::Posterior,
        model: CountModel::Multinomial,
        margin: FixedMargin::Total,
        prior: TablePrior::Conjugate,
        shapes: Vec::new(),
        scale: None,
        lambdas: Vec::new(),
        suppress_table: false,
        cells: vec![CrosstabsCell::Count],
        chisq: true,
        lrt: false,
    };
    let mut margin = None;
    let names = ["CRITERIA", "INFERENCE", "TABLE", "ESTBF", "PRINT"];
    subcommands(p, "BAYES LOGLINEAR", &names, |p, i| match i {
        0 => {
            let allowed = ["CILEVEL", "SEED", "TOL", "MAXITER", "POSTSAMPLES", "ORDER"];
            criteria(p, &mut c.criteria, &allowed)
        }
        1 => settings(p, &["ANALYSIS"], |p, _| {
            c.analysis = analysis(p)?;
            Ok(())
        }),
        2 => {
            let at = p.pos;
            c.row = p.name("the row variable")?;
            if !p.eat_word("BY") {
                return Err(p.unexpected(&["BY"]));
            }
            c.column = p.name("the column variable")?;
            if c.row.eq_ignore_ascii_case(&c.column) {
                return error(p, at, "TABLE needs two different variables");
            }
            Ok(())
        }
        3 => {
            let keywords = [
                "MODEL",
                "FIX_MARGIN",
                "PRIOR_TYPE",
                "SHAPEPARAMS",
                "SCALEPARAM",
                "LAMBDAS",
            ];
            settings(p, &keywords, |p, k| {
                match k {
                    0 => {
                        c.model = match choice(p, &["MULTINOMIAL", "POISSON", "NONPARAMETRIC"])? {
                            0 => CountModel::Multinomial,
                            1 => CountModel::Poisson,
                            _ => CountModel::Nonparametric,
                        }
                    }
                    1 => {
                        // a bare FIX_MARGIN= means the grand total
                        margin = Some(match which(p, &["TOTAL", "ROW", "COLUMN"])? {
                            None | Some(0) => FixedMargin::Total,
                            Some(1) => FixedMargin::Row,
                            Some(_) => FixedMargin::Column,
                        })
                    }
                    2 => {
                        let options = ["CONJUGATE", "MIXTURE_DIRICHLET", "INTRINSIC"];
                        c.prior = match choice(p, &options)? {
                            0 => TablePrior::Conjugate,
                            1 => {
                                p.eat_keyword("DIRICHLET");
                                TablePrior::MixtureDirichlet
                            }
                            _ => TablePrior::Intrinsic,
                        }
                    }
                    3 => c.shapes = positive_rows(p, "SHAPEPARAMS")?,
                    4 => {
                        c.scale = if p.eat_keyword("DATA") {
                            None
                        } else {
                            Some(checked(p, "SCALEPARAM", "above 0", |v| v > 0.0)?)
                        }
                    }
                    _ => {
                        let at = p.pos;
                        let rows = positive_rows(p, "LAMBDAS")?;
                        if rows.len() > 1 {
                            return error(p, at, "LAMBDAS takes one list of values, without ';'");
                        }
                        c.lambdas = rows.concat();
                    }
                }
                Ok(())
            })
        }
        _ => settings(p, &["SUPPRESS_TABLE", "CELLS", "STATISTICS"], |p, k| {
            match k {
                0 => c.suppress_table = boolean(p)?,
                1 => {
                    let options = ["OBSERVED", "ROW", "COLUMN", "EXPECTED", "TOTAL", "ALL"];
                    let found = keyword_list(p, &options)?;
                    c.cells = [
                        (0, CrosstabsCell::Count),
                        (3, CrosstabsCell::Expected),
                        (1, CrosstabsCell::Row),
                        (2, CrosstabsCell::Column),
                        (4, CrosstabsCell::Total),
                    ]
                    .into_iter()
                    .filter(|(i, _)| found.contains(i) || found.contains(&5))
                    .map(|(_, cell)| cell)
                    .collect();
                    if c.cells.is_empty() {
                        c.cells.push(CrosstabsCell::Count);
                    }
                }
                _ => {
                    let found = keyword_list(p, &["CHISQ", "LRT"])?;
                    c.chisq = found.is_empty() || found.contains(&0);
                    c.lrt = found.contains(&1);
                }
            }
            Ok(())
        }),
    })?;
    if c.row.is_empty() {
        return error(p, start, "BAYES LOGLINEAR needs /TABLE row BY column");
    }
    c.margin = match (c.model, margin) {
        (CountModel::Nonparametric, None | Some(FixedMargin::Total)) => {
            return error(
                p,
                start,
                "MODEL=NONPARAMETRIC needs FIX_MARGIN=ROW or COLUMN",
            );
        }
        (_, margin) => margin.unwrap_or(FixedMargin::Total),
    };
    Ok(Command::Bayes(Box::new(Bayes::Loglinear(c))))
}
