//! NPTESTS: its syntax tree and parser.
//!
//! Rules from the command's description: every name and keyword is written in full; each
//! subcommand and each keyword inside a subcommand may appear once; at least one of ONESAMPLE,
//! INDEPENDENT and RELATED is needed; a test subcommand without test keywords lets the
//! procedure choose the tests by measurement level.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub struct Nptests {
    pub one_sample: Option<NpOneSample>,
    pub independent: Option<NpIndependent>,
    pub related: Option<NpRelated>,
    /// `/MISSING SCOPE=LISTWISE`
    pub listwise: bool,
    /// `/MISSING USERMISSING=INCLUDE`: user-missing values of categorical fields are valid.
    pub include: bool,
    /// `/CRITERIA ALPHA`, 0.05 by default.
    pub alpha: f64,
    /// `/CRITERIA CILEVEL`, a percentage, 95 by default.
    pub cilevel: f64,
    /// `/CRITERIA SEED`; None for RANDOM.
    pub seed: Option<u64>,
}

/// How COMPARE asks for multiple comparisons after a k-sample test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpCompare {
    None,
    Pairwise,
    Stepwise,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpOneSample {
    pub fields: Vec<VarItem>,
    /// `CHISQUARE`, with the custom (category, frequency) pairs of EXPECTED=CUSTOM.
    pub chisquare: Option<Option<Vec<(Value, f64)>>>,
    pub binomial: Option<NpBinomial>,
    pub ks: Option<NpKs>,
    pub runs: Option<NpRuns>,
    /// `WILCOXON(TESTVALUE=value)`
    pub wilcoxon: Option<f64>,
}

impl NpOneSample {
    /// No test keyword: the tests are chosen from the fields.
    pub fn automatic(&self) -> bool {
        self.chisquare.is_none()
            && self.binomial.is_none()
            && self.ks.is_none()
            && self.runs.is_none()
            && self.wilcoxon.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpBinomial {
    pub test_value: f64,
    /// SUCCESSCATEGORICAL=LIST(values); None for FIRST.
    pub success: Option<Vec<Value>>,
    /// SUCCESSCONTINUOUS=CUTPOINT(value); None for MIDPOINT.
    pub cut: Option<f64>,
    pub clopper_pearson: bool,
    pub jeffreys: bool,
    pub likelihood: bool,
}

/// The parameters of a distribution for the one-sample Kolmogorov-Smirnov test.
#[derive(Debug, Clone, PartialEq)]
pub enum NpParameters {
    /// SAMPLE: estimated from the data; `simulation` is SIMULATION of NORMAL.
    Sample { simulation: bool },
    /// CUSTOM: the values in the order of the syntax (MEAN SD, MIN MAX or MEAN).
    Custom(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpKs {
    pub samples: usize,
    pub mc_cilevel: f64,
    pub normal: Option<NpParameters>,
    pub uniform: Option<NpParameters>,
    pub exponential: Option<NpParameters>,
    pub poisson: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpCut {
    Median,
    Mean,
    Value(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpRuns {
    /// GROUPCATEGORICAL=LIST(values); None for SAMPLE.
    pub group: Option<Vec<Value>>,
    pub cut: NpCut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpIndependent {
    pub fields: Vec<VarItem>,
    pub group: String,
    pub mann_whitney: bool,
    pub wald_wolfowitz: bool,
    pub ks: bool,
    pub hodges_lehmann: bool,
    /// `MOSES`, with TRIMOUTLIERS when given as a number.
    pub moses: Option<Option<f64>>,
    pub kruskal_wallis: Option<NpCompare>,
    /// `MEDIAN`: the TESTVALUE when given, and COMPARE.
    pub median: Option<(Option<f64>, NpCompare)>,
    /// `JONCKHEERE_TERPSTRA`: ORDER=DESCENDING, and COMPARE.
    pub jonckheere: Option<(bool, NpCompare)>,
}

impl NpIndependent {
    pub fn automatic(&self) -> bool {
        !self.mann_whitney
            && !self.wald_wolfowitz
            && !self.ks
            && !self.hodges_lehmann
            && self.moses.is_none()
            && self.kruskal_wallis.is_none()
            && self.median.is_none()
            && self.jonckheere.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpRelated {
    pub fields: Vec<VarItem>,
    /// `MCNEMAR`, with SUCCESS=LIST(values) when given (None is FIRST).
    pub mcnemar: Option<Option<Vec<Value>>>,
    pub wilcoxon: bool,
    pub sign: bool,
    pub marginal_homogeneity: bool,
    pub hodges_lehmann: bool,
    /// `COCHRAN`: SUCCESS=LIST(values) when given, and COMPARE.
    pub cochran: Option<(Option<Vec<Value>>, NpCompare)>,
    pub friedman: Option<NpCompare>,
    pub kendall: Option<NpCompare>,
}

impl NpRelated {
    pub fn automatic(&self) -> bool {
        self.mcnemar.is_none()
            && !self.wilcoxon
            && !self.sign
            && !self.marginal_homogeneity
            && !self.hodges_lehmann
            && self.cochran.is_none()
            && self.friedman.is_none()
            && self.kendall.is_none()
    }
}

/// Remembers the keywords met in one subcommand or keyword group, to refuse a repeat.
struct Seen(Vec<&'static str>);

impl Seen {
    fn new() -> Self {
        Seen(Vec::new())
    }

    /// Eats `word` (spelled in full) and refuses it the second time.
    fn eat(&mut self, p: &mut Parser, word: &'static str) -> PResult<bool> {
        if !p.eat_word(word) {
            return Ok(false);
        }
        if self.0.contains(&word) {
            let message = format!("{word} may be given only once here");
            return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
        }
        self.0.push(word);
        Ok(true)
    }
}

fn number(p: &mut Parser, what: &str) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

/// `= value` with the equals sign required.
fn equals_number(p: &mut Parser, what: &str) -> PResult<f64> {
    p.expect(Punct::Eq)?;
    number(p, what)
}

/// One value or more, numbers or quoted strings, up to a closing parenthesis.
fn value_list(p: &mut Parser) -> PResult<Vec<Value>> {
    let mut values = Vec::new();
    while let Some(value) = p.value() {
        values.push(value);
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    Ok(values)
}

/// `LIST(values)` after the equals sign of a success or group keyword; None for `default`.
fn list_or(p: &mut Parser, default: &str) -> PResult<Option<Vec<Value>>> {
    p.expect(Punct::Eq)?;
    if p.eat_word(default) {
        return Ok(None);
    }
    if !p.eat_word("LIST") {
        return Err(p.unexpected(&[default, "LIST"]));
    }
    p.expect(Punct::LParen)?;
    let values = value_list(p)?;
    p.expect(Punct::RParen)?;
    Ok(Some(values))
}

fn compare(p: &mut Parser) -> PResult<NpCompare> {
    p.expect(Punct::Eq)?;
    if p.eat_word("PAIRWISE") {
        Ok(NpCompare::Pairwise)
    } else if p.eat_word("STEPWISE") {
        Ok(NpCompare::Stepwise)
    } else if p.eat_word("NONE") {
        Ok(NpCompare::None)
    } else {
        Err(p.unexpected(&["PAIRWISE", "STEPWISE", "NONE"]))
    }
}

/// The optional parenthesized settings of a test keyword: `rule` reads one setting and says
/// whether it knew it.
fn settings(
    p: &mut Parser,
    expected: &[&str],
    mut rule: impl FnMut(&mut Parser, &mut Seen) -> PResult<bool>,
) -> PResult<()> {
    if !p.eat(Punct::LParen) {
        return Ok(());
    }
    let mut seen = Seen::new();
    while !p.eat(Punct::RParen) {
        if !rule(p, &mut seen)? {
            let mut also = expected.to_vec();
            also.push("')'");
            return Err(p.unexpected(&also));
        }
    }
    Ok(())
}

/// `TEST(fields)`
fn test_fields(p: &mut Parser) -> PResult<Vec<VarItem>> {
    p.expect(Punct::LParen)?;
    let fields = p.var_list()?;
    p.expect(Punct::RParen)?;
    Ok(fields)
}

pub(crate) fn nptests(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut command = Nptests {
        one_sample: None,
        independent: None,
        related: None,
        listwise: false,
        include: false,
        alpha: 0.05,
        cilevel: 95.0,
        seed: Some(2_000_000),
    };
    let mut subcommands = Seen::new();
    while p.eat(Punct::Slash) {
        if subcommands.eat(p, "MISSING")? {
            missing(p, &mut command)?;
        } else if subcommands.eat(p, "CRITERIA")? {
            criteria(p, &mut command)?;
        } else if subcommands.eat(p, "ONESAMPLE")? {
            command.one_sample = Some(one_sample(p)?);
        } else if subcommands.eat(p, "INDEPENDENT")? {
            command.independent = Some(independent(p)?);
        } else if subcommands.eat(p, "RELATED")? {
            command.related = Some(related(p)?);
        } else {
            let mut error = p.subcommand_error("NPTESTS", &[]);
            error.expected = ["ONESAMPLE", "INDEPENDENT", "RELATED", "MISSING", "CRITERIA"]
                .map(String::from)
                .to_vec();
            return Err(error);
        }
    }
    p.finish(&["'/'"])?;
    if command.one_sample.is_none() && command.independent.is_none() && command.related.is_none() {
        let message = "NPTESTS needs ONESAMPLE, INDEPENDENT or RELATED, such as /ONESAMPLE TEST(v)";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(Command::Nptests(Box::new(command)))
}

fn missing(p: &mut Parser, command: &mut Nptests) -> PResult<()> {
    let mut seen = Seen::new();
    let begin = p.pos;
    while !at_end(p) {
        if seen.eat(p, "SCOPE")? {
            p.expect(Punct::Eq)?;
            command.listwise = if p.eat_word("LISTWISE") {
                true
            } else if p.eat_word("ANALYSIS") {
                false
            } else {
                return Err(p.unexpected(&["ANALYSIS", "LISTWISE"]));
            };
        } else if seen.eat(p, "USERMISSING")? {
            p.expect(Punct::Eq)?;
            command.include = if p.eat_word("INCLUDE") {
                true
            } else if p.eat_word("EXCLUDE") {
                false
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            };
        } else {
            return Err(p.unexpected(&["SCOPE", "USERMISSING"]));
        }
    }
    empty_check(p, begin, "MISSING")
}

fn empty_check(p: &Parser, begin: usize, name: &str) -> PResult<()> {
    if p.pos == begin {
        let message = format!("the {name} subcommand is empty");
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(())
}

fn criteria(p: &mut Parser, command: &mut Nptests) -> PResult<()> {
    let mut seen = Seen::new();
    let begin = p.pos;
    while !at_end(p) {
        let at = p.pos;
        if seen.eat(p, "ALPHA")? {
            command.alpha = equals_number(p, "a significance level")?;
            if !(command.alpha > 0.0 && command.alpha < 1.0) {
                let message = "ALPHA must lie between 0 and 1";
                return Err(SyntaxError::new(p.span_from(at), message));
            }
        } else if seen.eat(p, "CILEVEL")? {
            command.cilevel = equals_number(p, "a confidence level")?;
            if !(command.cilevel > 0.0 && command.cilevel < 100.0) {
                let message = "CILEVEL must lie between 0 and 100";
                return Err(SyntaxError::new(p.span_from(at), message));
            }
        } else if seen.eat(p, "SEED")? {
            // both SEED=n and SEED(n) are accepted
            let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
            command.seed = if p.eat_word("RANDOM") {
                None
            } else {
                let seed = number(p, "a seed or RANDOM")?;
                if seed.fract() != 0.0 || !(1.0..=2_147_483_647.0).contains(&seed) {
                    let message = "SEED must be a whole number from 1 to 2147483647";
                    return Err(SyntaxError::new(p.span_from(at), message));
                }
                Some(seed as u64)
            };
            if parens {
                p.expect(Punct::RParen)?;
            }
        } else {
            return Err(p.unexpected(&["ALPHA", "CILEVEL", "SEED"]));
        }
    }
    empty_check(p, begin, "CRITERIA")
}

fn one_sample(p: &mut Parser) -> PResult<NpOneSample> {
    let mut out = NpOneSample {
        fields: Vec::new(),
        chisquare: None,
        binomial: None,
        ks: None,
        runs: None,
        wilcoxon: None,
    };
    let mut seen = Seen::new();
    while !at_end(p) {
        if seen.eat(p, "TEST")? {
            out.fields = test_fields(p)?;
        } else if seen.eat(p, "CHISQUARE")? {
            out.chisquare = Some(chi_square(p)?);
        } else if seen.eat(p, "BINOMIAL")? {
            out.binomial = Some(binomial(p)?);
        } else if seen.eat(p, "KOLMOGOROV_SMIRNOV")? {
            out.ks = Some(ks(p)?);
        } else if seen.eat(p, "RUNS")? {
            out.runs = Some(runs(p)?);
        } else if seen.eat(p, "WILCOXON")? {
            let mut value = None;
            settings(p, &["TESTVALUE"], |p, seen| {
                Ok(seen.eat(p, "TESTVALUE")? && {
                    value = Some(equals_number(p, "a median")?);
                    true
                })
            })?;
            let Some(value) = value else {
                let message = "WILCOXON needs TESTVALUE=value, the median to test";
                return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
            };
            out.wilcoxon = Some(value);
        } else {
            return Err(p.unexpected(&[
                "TEST",
                "CHISQUARE",
                "BINOMIAL",
                "KOLMOGOROV_SMIRNOV",
                "RUNS",
                "WILCOXON",
            ]));
        }
    }
    if out.fields.is_empty() {
        return Err(SyntaxError::new(p.span(), "ONESAMPLE needs TEST(fields)"));
    }
    Ok(out)
}

fn chi_square(p: &mut Parser) -> PResult<Option<Vec<(Value, f64)>>> {
    let mut custom = None;
    settings(p, &["EXPECTED"], |p, seen| {
        if !seen.eat(p, "EXPECTED")? {
            return Ok(false);
        }
        p.expect(Punct::Eq)?;
        if p.eat_word("EQUAL") {
            return Ok(true);
        }
        if !p.eat_word("CUSTOM") {
            return Err(p.unexpected(&["EQUAL", "CUSTOM"]));
        }
        let at = p.pos;
        p.expect(Punct::LParen)?;
        let (mut frequencies, mut categories) = (None, None);
        let mut inner = Seen::new();
        while !p.eat(Punct::RParen) {
            if inner.eat(p, "FREQUENCIES")? {
                p.expect(Punct::Eq)?;
                let mut list = Vec::new();
                while let Some(n) = p.number() {
                    list.push(n);
                    p.eat(Punct::Comma);
                }
                if list.is_empty() || list.iter().any(|f| *f <= 0.0) {
                    let message = "FREQUENCIES needs values above 0";
                    return Err(SyntaxError::new(p.span_from(at), message));
                }
                frequencies = Some(list);
            } else if inner.eat(p, "CATEGORIES")? {
                p.expect(Punct::Eq)?;
                categories = Some(value_list(p)?);
            } else {
                return Err(p.unexpected(&["FREQUENCIES", "CATEGORIES", "')'"]));
            }
        }
        let (Some(f), Some(c)) = (frequencies, categories) else {
            let message = "EXPECTED=CUSTOM needs both FREQUENCIES and CATEGORIES";
            return Err(SyntaxError::new(p.span_from(at), message));
        };
        if f.len() != c.len() {
            let message = format!(
                "EXPECTED=CUSTOM gives {} frequencies for {} categories",
                f.len(),
                c.len()
            );
            return Err(SyntaxError::new(p.span_from(at), message));
        }
        custom = Some(c.into_iter().zip(f).collect());
        Ok(true)
    })?;
    Ok(custom)
}

fn binomial(p: &mut Parser) -> PResult<NpBinomial> {
    let mut out = NpBinomial {
        test_value: 0.5,
        success: None,
        cut: None,
        clopper_pearson: false,
        jeffreys: false,
        likelihood: false,
    };
    let expected = [
        "TESTVALUE",
        "SUCCESSCATEGORICAL",
        "SUCCESSCONTINUOUS",
        "CLOPPERPEARSON",
        "JEFFREYS",
        "LIKELIHOOD",
    ];
    settings(p, &expected, |p, seen| {
        let at = p.pos;
        if seen.eat(p, "TESTVALUE")? {
            out.test_value = equals_number(p, "a proportion")?;
            if !(out.test_value > 0.0 && out.test_value < 1.0) {
                let message = "TESTVALUE must lie between 0 and 1";
                return Err(SyntaxError::new(p.span_from(at), message));
            }
        } else if seen.eat(p, "SUCCESSCATEGORICAL")? {
            out.success = list_or(p, "FIRST")?;
        } else if seen.eat(p, "SUCCESSCONTINUOUS")? {
            p.expect(Punct::Eq)?;
            if !p.eat_word("CUTPOINT") {
                return Err(p.unexpected(&["CUTPOINT"]));
            }
            p.expect(Punct::LParen)?;
            out.cut = if p.eat_word("MIDPOINT") {
                None
            } else {
                Some(number(p, "MIDPOINT or a cut point")?)
            };
            p.expect(Punct::RParen)?;
        } else if seen.eat(p, "CLOPPERPEARSON")? {
            out.clopper_pearson = true;
        } else if seen.eat(p, "JEFFREYS")? {
            out.jeffreys = true;
        } else if seen.eat(p, "LIKELIHOOD")? {
            out.likelihood = true;
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(out)
}

/// `SAMPLE [(SIMULATION=TRUE|FALSE)]` or `CUSTOM(name=value ...)` with the names in `names`.
fn parameters(p: &mut Parser, names: &[&'static str], simulation: bool) -> PResult<NpParameters> {
    p.expect(Punct::Eq)?;
    if p.eat_word("SAMPLE") {
        let mut on = true;
        if simulation && p.eat(Punct::LParen) {
            if !p.eat_word("SIMULATION") {
                return Err(p.unexpected(&["SIMULATION"]));
            }
            p.expect(Punct::Eq)?;
            on = if p.eat_word("TRUE") {
                true
            } else if p.eat_word("FALSE") {
                false
            } else {
                return Err(p.unexpected(&["TRUE", "FALSE"]));
            };
            p.expect(Punct::RParen)?;
        }
        return Ok(NpParameters::Sample { simulation: on });
    }
    if !p.eat_word("CUSTOM") {
        return Err(p.unexpected(&["SAMPLE", "CUSTOM"]));
    }
    custom(p, names).map(NpParameters::Custom)
}

/// `(name=value ...)` with every name of `names` once, in any order.
fn custom(p: &mut Parser, names: &[&'static str]) -> PResult<Vec<f64>> {
    let at = p.pos;
    p.expect(Punct::LParen)?;
    let mut values = vec![None; names.len()];
    let mut seen = Seen::new();
    while !p.eat(Punct::RParen) {
        let mut found = None;
        for (i, name) in names.iter().enumerate() {
            if seen.eat(p, name)? {
                found = Some(i);
                break;
            }
        }
        let Some(i) = found else {
            let mut also = names.to_vec();
            also.push("')'");
            return Err(p.unexpected(&also));
        };
        values[i] = Some(equals_number(p, "a number")?);
    }
    values
        .into_iter()
        .collect::<Option<Vec<f64>>>()
        .ok_or_else(|| {
            let message = format!("CUSTOM needs {}", names.join(" and "));
            SyntaxError::new(p.span_from(at), message)
        })
}

fn ks(p: &mut Parser) -> PResult<NpKs> {
    let mut out = NpKs {
        samples: 1000,
        mc_cilevel: 99.0,
        normal: None,
        uniform: None,
        exponential: None,
        poisson: None,
    };
    let expected = [
        "NSAMPLES",
        "MC_CILEVEL",
        "NORMAL",
        "UNIFORM",
        "EXPONENTIAL",
        "POISSON",
    ];
    settings(p, &expected, |p, seen| {
        let at = p.pos;
        if seen.eat(p, "NSAMPLES")? {
            let n = equals_number(p, "a number of samples")?;
            if n.fract() != 0.0 || n < 1.0 || n > 1e9 {
                let message = "NSAMPLES must be a whole number from 1 to 1000000000";
                return Err(SyntaxError::new(p.span_from(at), message));
            }
            out.samples = n as usize;
        } else if seen.eat(p, "MC_CILEVEL")? {
            out.mc_cilevel = equals_number(p, "a confidence level")?;
            if !(out.mc_cilevel > 0.0 && out.mc_cilevel < 100.0) {
                let message = "MC_CILEVEL must lie between 0 and 100";
                return Err(SyntaxError::new(p.span_from(at), message));
            }
        } else if seen.eat(p, "NORMAL")? {
            out.normal = Some(parameters(p, &["MEAN", "SD"], true)?);
        } else if seen.eat(p, "UNIFORM")? {
            out.uniform = Some(parameters(p, &["MIN", "MAX"], false)?);
        } else if seen.eat(p, "EXPONENTIAL")? {
            out.exponential = Some(parameters(p, &["MEAN"], false)?);
        } else if seen.eat(p, "POISSON")? {
            p.expect(Punct::Eq)?;
            if !p.eat_word("CUSTOM") {
                return Err(p.unexpected(&["CUSTOM"]));
            }
            out.poisson = Some(custom(p, &["MEAN"])?[0]);
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    if out.normal.is_none()
        && out.uniform.is_none()
        && out.exponential.is_none()
        && out.poisson.is_none()
    {
        out.normal = Some(NpParameters::Sample { simulation: true });
    }
    Ok(out)
}

fn runs(p: &mut Parser) -> PResult<NpRuns> {
    let mut out = NpRuns {
        group: None,
        cut: NpCut::Median,
    };
    settings(p, &["GROUPCATEGORICAL", "GROUPCONTINUOUS"], |p, seen| {
        if seen.eat(p, "GROUPCATEGORICAL")? {
            out.group = list_or(p, "SAMPLE")?;
        } else if seen.eat(p, "GROUPCONTINUOUS")? {
            p.expect(Punct::Eq)?;
            if !p.eat_word("CUTPOINT") {
                return Err(p.unexpected(&["CUTPOINT"]));
            }
            p.expect(Punct::LParen)?;
            out.cut = if p.eat_word("SAMPLEMEDIAN") {
                NpCut::Median
            } else if p.eat_word("SAMPLEMEAN") {
                NpCut::Mean
            } else {
                NpCut::Value(number(p, "SAMPLEMEDIAN, SAMPLEMEAN or a cut point")?)
            };
            p.expect(Punct::RParen)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(out)
}

/// `KEYWORD[(COMPARE=...)]`
fn compare_only(p: &mut Parser) -> PResult<NpCompare> {
    let mut out = NpCompare::Pairwise;
    settings(p, &["COMPARE"], |p, seen| {
        Ok(seen.eat(p, "COMPARE")? && {
            out = compare(p)?;
            true
        })
    })?;
    Ok(out)
}

fn independent(p: &mut Parser) -> PResult<NpIndependent> {
    let mut out = NpIndependent {
        fields: Vec::new(),
        group: String::new(),
        mann_whitney: false,
        wald_wolfowitz: false,
        ks: false,
        hodges_lehmann: false,
        moses: None,
        kruskal_wallis: None,
        median: None,
        jonckheere: None,
    };
    let mut seen = Seen::new();
    while !at_end(p) {
        if seen.eat(p, "TEST")? {
            out.fields = test_fields(p)?;
        } else if seen.eat(p, "GROUP")? {
            p.expect(Punct::LParen)?;
            out.group = p.name("a grouping field")?;
            p.expect(Punct::RParen)?;
        } else if seen.eat(p, "MANN_WHITNEY")? {
            out.mann_whitney = true;
        } else if seen.eat(p, "WALD_WOLFOWITZ")? {
            out.wald_wolfowitz = true;
        } else if seen.eat(p, "KOLMOGOROV_SMIRNOV")? {
            out.ks = true;
        } else if seen.eat(p, "HODGES_LEHMAN")? {
            out.hodges_lehmann = true;
        } else if seen.eat(p, "MOSES")? {
            let mut trim = None;
            settings(p, &["TRIMOUTLIERS"], |p, seen| {
                if !seen.eat(p, "TRIMOUTLIERS")? {
                    return Ok(false);
                }
                p.expect(Punct::Eq)?;
                if !p.eat_word("SAMPLE") {
                    let at = p.pos;
                    let n = number(p, "SAMPLE or a number of records")?;
                    if n.fract() != 0.0 || n < 0.0 {
                        let message = "TRIMOUTLIERS must be a whole number of at least 0";
                        return Err(SyntaxError::new(p.span_from(at), message));
                    }
                    trim = Some(n);
                }
                Ok(true)
            })?;
            out.moses = Some(trim);
        } else if seen.eat(p, "KRUSKAL_WALLIS")? {
            out.kruskal_wallis = Some(compare_only(p)?);
        } else if seen.eat(p, "MEDIAN")? {
            let (mut value, mut how) = (None, NpCompare::Pairwise);
            settings(p, &["TESTVALUE", "COMPARE"], |p, seen| {
                if seen.eat(p, "TESTVALUE")? {
                    p.expect(Punct::Eq)?;
                    if !p.eat_word("SAMPLE") {
                        value = Some(number(p, "SAMPLE or a median")?);
                    }
                } else if seen.eat(p, "COMPARE")? {
                    how = compare(p)?;
                } else {
                    return Ok(false);
                }
                Ok(true)
            })?;
            out.median = Some((value, how));
        } else if seen.eat(p, "JONCKHEERE_TERPSTRA")? {
            let (mut descending, mut how) = (false, NpCompare::Pairwise);
            settings(p, &["ORDER", "COMPARE"], |p, seen| {
                if seen.eat(p, "ORDER")? {
                    p.expect(Punct::Eq)?;
                    descending = if p.eat_word("DESCENDING") {
                        true
                    } else if p.eat_word("ASCENDING") {
                        false
                    } else {
                        return Err(p.unexpected(&["ASCENDING", "DESCENDING"]));
                    };
                } else if seen.eat(p, "COMPARE")? {
                    how = compare(p)?;
                } else {
                    return Ok(false);
                }
                Ok(true)
            })?;
            out.jonckheere = Some((descending, how));
        } else {
            return Err(p.unexpected(&[
                "TEST",
                "GROUP",
                "MANN_WHITNEY",
                "WALD_WOLFOWITZ",
                "KOLMOGOROV_SMIRNOV",
                "HODGES_LEHMAN",
                "MOSES",
                "KRUSKAL_WALLIS",
                "MEDIAN",
                "JONCKHEERE_TERPSTRA",
            ]));
        }
    }
    if out.fields.is_empty() || out.group.is_empty() {
        let message = "INDEPENDENT needs TEST(fields) and GROUP(field)";
        return Err(SyntaxError::new(p.span(), message));
    }
    Ok(out)
}

/// `KEYWORD[(SUCCESS=FIRST|LIST(values) [COMPARE=...])]`
fn success(p: &mut Parser, with_compare: bool) -> PResult<(Option<Vec<Value>>, NpCompare)> {
    let (mut values, mut how) = (None, NpCompare::Pairwise);
    let expected: &[&str] = if with_compare {
        &["SUCCESS", "COMPARE"]
    } else {
        &["SUCCESS"]
    };
    settings(p, expected, |p, seen| {
        if seen.eat(p, "SUCCESS")? {
            values = list_or(p, "FIRST")?;
        } else if with_compare && seen.eat(p, "COMPARE")? {
            how = compare(p)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok((values, how))
}

fn related(p: &mut Parser) -> PResult<NpRelated> {
    let mut out = NpRelated {
        fields: Vec::new(),
        mcnemar: None,
        wilcoxon: false,
        sign: false,
        marginal_homogeneity: false,
        hodges_lehmann: false,
        cochran: None,
        friedman: None,
        kendall: None,
    };
    let mut seen = Seen::new();
    while !at_end(p) {
        if seen.eat(p, "TEST")? {
            out.fields = test_fields(p)?;
        } else if seen.eat(p, "MCNEMAR")? {
            out.mcnemar = Some(success(p, false)?.0);
        } else if seen.eat(p, "WILCOXON")? {
            out.wilcoxon = true;
        } else if seen.eat(p, "SIGN")? {
            out.sign = true;
        } else if seen.eat(p, "MARGINAL_HOMOGENEITY")? {
            out.marginal_homogeneity = true;
        } else if seen.eat(p, "HODGES_LEHMAN")? {
            out.hodges_lehmann = true;
        } else if seen.eat(p, "COCHRAN")? {
            out.cochran = Some(success(p, true)?);
        } else if seen.eat(p, "FRIEDMAN")? {
            out.friedman = Some(compare_only(p)?);
        } else if seen.eat(p, "KENDALL")? {
            out.kendall = Some(compare_only(p)?);
        } else {
            return Err(p.unexpected(&[
                "TEST",
                "MCNEMAR",
                "WILCOXON",
                "SIGN",
                "MARGINAL_HOMOGENEITY",
                "HODGES_LEHMAN",
                "COCHRAN",
                "FRIEDMAN",
                "KENDALL",
            ]));
        }
    }
    if out.fields.is_empty() {
        return Err(SyntaxError::new(p.span(), "RELATED needs TEST(fields)"));
    }
    Ok(out)
}
