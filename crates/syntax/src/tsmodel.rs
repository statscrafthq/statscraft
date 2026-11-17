//! Parsers of TSMODEL and TSAPPLY.
//!
//! Rules noted from the Command Syntax Reference:
//! - Global subcommands (MODELSUMMARY, MODELSTATISTICS, MODELDETAILS, SERIESPLOT, OUTPUTFILTER,
//!   SAVE, AUXILIARY, MISSING) come before the first MODEL, at most once each. TSMODEL's MODEL
//!   opens a block that EXPERTMODELER, EXSMOOTH or ARIMA (one of them) and AUTOOUTLIER may
//!   follow; TSAPPLY's MODEL names a model file and may repeat.
//! - Names and keywords are written in full, the equals signs are required, a keyword is given
//!   once within its subcommand and no subcommand is empty. A list goes in square brackets,
//!   which a single value may leave out.
//! - Defaults: MODELSUMMARY PRINT=MODELFIT, MODELSTATISTICS DISPLAY=YES MODELFIT=SRSQUARE,
//!   OUTPUTFILTER DISPLAY=ALLMODELS MODELFIT=SRSQUARE, CILEVEL=95, MAXACFLAGS=24,
//!   USERMISSING=EXCLUDE, PREFIX='Model', no method subcommand is EXPERTMODELER with both
//!   types and TRYSEASONAL=YES.
//! - INDEPENDENT may mark a variable as an event with [E] after it; events enter linearly,
//!   which is how every predictor enters here, so the mark is accepted and has no effect.
//! - TRANSFERFUNCTION, OUTLIER, AUTOOUTLIER DETECT=ON and OUTPMML are not supported yet.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::ts_models::{error, number, whole};

/// A goodness-of-fit measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMeasure {
    Srsquare,
    Rsquare,
    Rmse,
    Mape,
    Mae,
    Maxape,
    Maxae,
    Normbic,
}

pub const FIT_MEASURES: [(&str, FitMeasure); 8] = [
    ("SRSQUARE", FitMeasure::Srsquare),
    ("RSQUARE", FitMeasure::Rsquare),
    ("RMSE", FitMeasure::Rmse),
    ("MAPE", FitMeasure::Mape),
    ("MAE", FitMeasure::Mae),
    ("MAXAPE", FitMeasure::Maxape),
    ("MAXAE", FitMeasure::Maxae),
    ("NORMBIC", FitMeasure::Normbic),
];

/// The exponential smoothing methods of TSMODEL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsType {
    Simple,
    SimpleSeasonal,
    Holt,
    Brown,
    DampedTrend,
    WintersAdditive,
    WintersMultiplicative,
}

pub const ES_TYPES: [(&str, EsType); 7] = [
    ("SIMPLE", EsType::Simple),
    ("SIMPLESEASONAL", EsType::SimpleSeasonal),
    ("HOLT", EsType::Holt),
    ("BROWN", EsType::Brown),
    ("DAMPEDTREND", EsType::DampedTrend),
    ("WINTERSADDITIVE", EsType::WintersAdditive),
    ("WINTERSMULTIPLICATIVE", EsType::WintersMultiplicative),
];

/// A transformation of the dependent series before it is modeled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TsTransform {
    #[default]
    None,
    Sqrt,
    Ln,
}

/// A custom ARIMA model. The lag lists are as written: one value is an order, two or more are
/// the lags themselves (see the engine).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TsArima {
    pub ar: Vec<u32>,
    pub ma: Vec<u32>,
    pub sar: Vec<u32>,
    pub sma: Vec<u32>,
    pub d: u32,
    pub sd: u32,
    pub constant: bool,
    pub transform: TsTransform,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TsMethod {
    Expert {
        arima: bool,
        exsmooth: bool,
        seasonal: bool,
    },
    Exsmooth(EsType, TsTransform),
    Arima(TsArima),
}

/// BESTFIT or WORSTFIT: a count of models or a percentage of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterCount {
    N(usize),
    Pct(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsSave {
    Predicted,
    Lcl,
    Ucl,
    Nresidual,
}

/// The global subcommands of TSMODEL and TSAPPLY.
#[derive(Debug, Clone, PartialEq)]
pub struct TsOutput {
    pub summary_fit: bool,
    pub summary_acf: bool,
    pub summary_pacf: bool,
    /// MODELSUMMARY PLOT was given (histograms and boxplots).
    pub summary_plots: bool,
    pub statistics: bool,
    pub statistics_fit: Vec<FitMeasure>,
    pub parameters: bool,
    pub residual_acf: bool,
    pub residual_pacf: bool,
    pub forecasts: bool,
    pub plot_acf: bool,
    pub plot_pacf: bool,
    /// SERIESPLOT: OBSERVED, FORECAST, FIT, FORECASTCI, FITCI.
    pub series_plot: [bool; 5],
    pub best: Option<FilterCount>,
    pub worst: Option<FilterCount>,
    pub filter_fit: FitMeasure,
    pub save: Vec<(TsSave, Option<String>)>,
    /// TSAPPLY's AUXILIARY REESTIMATE.
    pub reestimate: bool,
    pub cilevel: f64,
    pub maxacflags: usize,
    pub season_length: Option<usize>,
    pub include_user_missing: bool,
}

impl Default for TsOutput {
    fn default() -> Self {
        TsOutput {
            summary_fit: true,
            summary_acf: false,
            summary_pacf: false,
            summary_plots: false,
            statistics: true,
            statistics_fit: vec![FitMeasure::Srsquare],
            parameters: false,
            residual_acf: false,
            residual_pacf: false,
            forecasts: false,
            plot_acf: false,
            plot_pacf: false,
            series_plot: [false; 5],
            best: None,
            worst: None,
            filter_fit: FitMeasure::Srsquare,
            save: Vec::new(),
            reestimate: false,
            cilevel: 95.0,
            maxacflags: 24,
            season_length: None,
            include_user_missing: false,
        }
    }
}

/// One MODEL block of TSMODEL.
#[derive(Debug, Clone, PartialEq)]
pub struct TsModelBlock {
    pub dependent: Vec<VarItem>,
    pub independent: Vec<VarItem>,
    pub outfile: Option<String>,
    pub prefix: String,
    pub method: TsMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tsmodel {
    pub output: TsOutput,
    pub models: Vec<TsModelBlock>,
}

/// One MODEL subcommand of TSAPPLY.
#[derive(Debug, Clone, PartialEq)]
pub struct TsModelFile {
    pub file: String,
    pub keep: Option<Vec<String>>,
    pub drop: Vec<String>,
    pub outfile: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tsapply {
    pub output: TsOutput,
    pub files: Vec<TsModelFile>,
}

/// The next keyword of `names`, once within the subcommand.
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
        return Err(error(p, start, format!("{name} is given twice")));
    }
    seen.push(name);
    p.expect(Punct::Eq)?;
    Ok(name)
}

/// One of the named words, as its value.
fn one_of<T: Copy>(p: &mut Parser, table: &[(&'static str, T)]) -> PResult<T> {
    match table.iter().find(|(n, _)| p.eat_word(n)) {
        Some((_, v)) => Ok(*v),
        None => Err(p.unexpected(&table.iter().map(|(n, _)| *n).collect::<Vec<_>>())),
    }
}

/// One item, or several in square brackets.
fn bracketed<T>(
    p: &mut Parser,
    mut item: impl FnMut(&mut Parser) -> PResult<T>,
) -> PResult<Vec<T>> {
    if !p.eat(Punct::LBracket) {
        return Ok(vec![item(p)?]);
    }
    let mut out = Vec::new();
    while !p.eat(Punct::RBracket) {
        out.push(item(p)?);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    Ok(out)
}

fn transform(p: &mut Parser) -> PResult<TsTransform> {
    one_of(
        p,
        &[
            ("NONE", TsTransform::None),
            ("SQRT", TsTransform::Sqrt),
            ("LN", TsTransform::Ln),
        ],
    )
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    one_of(p, &[("YES", true), ("NO", false)])
}

fn filter_count(p: &mut Parser) -> PResult<FilterCount> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let count = if p.eat_word("N") {
        p.expect(Punct::Eq)?;
        FilterCount::N(whole(p, 1, "N")? as usize)
    } else if p.eat_word("PCT") {
        p.expect(Punct::Eq)?;
        let v = number(p, "a percentage")?;
        if v <= 0.0 || v >= 100.0 {
            return Err(error(p, start, "PCT is above 0 and below 100"));
        }
        FilterCount::Pct(v)
    } else {
        return Err(p.unexpected(&["N", "PCT"]));
    };
    p.expect(Punct::RParen)?;
    Ok(count)
}

/// The global subcommand `sub`, its name just read.
fn global(p: &mut Parser, sub: &str, out: &mut TsOutput, apply: bool) -> PResult<()> {
    let mut seen = Vec::new();
    match sub {
        "MODELSUMMARY" => {
            while !at_end(p) {
                if keyword(p, &mut seen, &["PRINT", "PLOT"])? == "PRINT" {
                    let start = p.pos;
                    let items = bracketed(p, |p| {
                        one_of(
                            p,
                            &[
                                ("MODELFIT", 0),
                                ("RESIDACF", 1),
                                ("RESIDPACF", 2),
                                ("NONE", 3),
                            ],
                        )
                    })?;
                    if items.contains(&3) && items.len() > 1 {
                        return Err(error(p, start, "PRINT=NONE cannot go with other items"));
                    }
                    out.summary_fit = items.contains(&0);
                    out.summary_acf = items.contains(&1);
                    out.summary_pacf = items.contains(&2);
                } else {
                    let mut table: Vec<(&str, ())> =
                        FIT_MEASURES.iter().map(|(n, _)| (*n, ())).collect();
                    table.extend([("RESIDACF", ()), ("RESIDPACF", ())]);
                    bracketed(p, |p| one_of(p, &table))?;
                    out.summary_plots = true;
                }
            }
        }
        "MODELSTATISTICS" => {
            while !at_end(p) {
                if keyword(p, &mut seen, &["DISPLAY", "MODELFIT"])? == "DISPLAY" {
                    out.statistics = yes_no(p)?;
                } else {
                    out.statistics_fit = bracketed(p, |p| one_of(p, &FIT_MEASURES))?;
                }
            }
        }
        "MODELDETAILS" => {
            while !at_end(p) {
                if keyword(p, &mut seen, &["PRINT", "PLOT"])? == "PRINT" {
                    let items = bracketed(p, |p| {
                        one_of(
                            p,
                            &[
                                ("PARAMETERS", 0),
                                ("RESIDACF", 1),
                                ("RESIDPACF", 2),
                                ("FORECASTS", 3),
                            ],
                        )
                    })?;
                    out.parameters = items.contains(&0);
                    out.residual_acf = items.contains(&1);
                    out.residual_pacf = items.contains(&2);
                    out.forecasts = items.contains(&3);
                } else {
                    let items = bracketed(p, |p| {
                        one_of(p, &[("RESIDACF", true), ("RESIDPACF", false)])
                    })?;
                    out.plot_acf = items.contains(&true);
                    out.plot_pacf = items.contains(&false);
                }
            }
        }
        "SERIESPLOT" => {
            let names = ["OBSERVED", "FORECAST", "FIT", "FORECASTCI", "FITCI"];
            while !at_end(p) {
                let start = p.pos;
                let Some(i) = names.iter().position(|n| p.eat_word(n)) else {
                    return Err(p.unexpected(&names));
                };
                if out.series_plot[i] {
                    return Err(error(p, start, format!("{} is given twice", names[i])));
                }
                out.series_plot[i] = true;
            }
        }
        "OUTPUTFILTER" => {
            while !at_end(p) {
                if keyword(p, &mut seen, &["DISPLAY", "MODELFIT"])? == "DISPLAY" {
                    if p.eat_word("ALLMODELS") {
                        (out.best, out.worst) = (None, None);
                        continue;
                    }
                    let start = p.pos;
                    let bracket = p.eat(Punct::LBracket);
                    loop {
                        if p.eat_word("BESTFIT") && out.best.is_none() {
                            out.best = Some(filter_count(p)?);
                        } else if p.eat_word("WORSTFIT") && out.worst.is_none() {
                            out.worst = Some(filter_count(p)?);
                        } else {
                            return Err(p.unexpected(&["ALLMODELS", "BESTFIT", "WORSTFIT"]));
                        }
                        if !bracket || p.eat(Punct::RBracket) {
                            break;
                        }
                    }
                    if out.best.is_none() && out.worst.is_none() {
                        return Err(error(p, start, "DISPLAY needs BESTFIT or WORSTFIT"));
                    }
                } else {
                    out.filter_fit = one_of(p, &FIT_MEASURES)?;
                }
            }
        }
        "SAVE" => {
            let table = [
                ("PREDICTED", TsSave::Predicted),
                ("LCL", TsSave::Lcl),
                ("UCL", TsSave::Ucl),
                ("NRESIDUAL", TsSave::Nresidual),
            ];
            while !at_end(p) {
                let start = p.pos;
                let what = one_of(p, &table)?;
                if out.save.iter().any(|(w, _)| *w == what) {
                    return Err(error(p, start, "a SAVE keyword is given twice"));
                }
                let root = match p.eat(Punct::LParen) {
                    true => {
                        let name = p.name("a root name")?;
                        p.expect(Punct::RParen)?;
                        Some(name)
                    }
                    false => None,
                };
                out.save.push((what, root));
            }
        }
        "AUXILIARY" => {
            let mut names = vec!["CILEVEL", "MAXACFLAGS", "SEASONLENGTH"];
            if apply {
                names.push("REESTIMATE");
            }
            while !at_end(p) {
                let start = p.pos;
                match keyword(p, &mut seen, &names)? {
                    "CILEVEL" => {
                        let v = number(p, "a confidence level")?;
                        if v <= 0.0 || v >= 100.0 {
                            return Err(error(p, start, "CILEVEL is above 0 and below 100"));
                        }
                        out.cilevel = v;
                    }
                    "MAXACFLAGS" => out.maxacflags = whole(p, 1, "MAXACFLAGS")? as usize,
                    "SEASONLENGTH" => {
                        out.season_length = Some(whole(p, 1, "SEASONLENGTH")? as usize);
                    }
                    _ => out.reestimate = yes_no(p)?,
                }
            }
        }
        _ => {
            keyword(p, &mut seen, &["USERMISSING"])?;
            out.include_user_missing = one_of(p, &[("EXCLUDE", false), ("INCLUDE", true)])?;
        }
    }
    Ok(())
}

const GLOBALS: [&str; 8] = [
    "MODELSUMMARY",
    "MODELSTATISTICS",
    "MODELDETAILS",
    "SERIESPLOT",
    "OUTPUTFILTER",
    "SAVE",
    "AUXILIARY",
    "MISSING",
];

/// A global subcommand, if one comes next; errors after the first MODEL, when repeated or
/// empty.
fn try_global(
    p: &mut Parser,
    seen: &mut Vec<&'static str>,
    out: &mut TsOutput,
    after_model: bool,
    apply: bool,
) -> PResult<bool> {
    let start = p.pos;
    let Some(&sub) = GLOBALS.iter().find(|n| p.eat_word(n)) else {
        return Ok(false);
    };
    if after_model {
        return Err(error(
            p,
            start,
            format!("/{sub} comes before the first /MODEL"),
        ));
    }
    if seen.contains(&sub) {
        return Err(error(p, start, format!("/{sub} is given twice")));
    }
    if at_end(p) {
        return Err(error(p, start, format!("/{sub} is empty")));
    }
    seen.push(sub);
    global(p, sub, out, apply)?;
    Ok(true)
}

/// Variable names until the next keyword (a word before `=`) or the end of the subcommand;
/// an event mark [E] after a name is read and dropped.
fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq))) {
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
        if p.eat(Punct::LBracket) {
            if !p.eat_word("E") {
                return Err(p.unexpected(&["E"]));
            }
            p.expect(Punct::RBracket)?;
        }
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

fn quoted_ids(p: &mut Parser) -> PResult<Vec<String>> {
    bracketed(p, |p| p.string("a quoted model identifier"))
}

fn not_supported(p: &Parser, start: usize, what: &str) -> SyntaxError {
    error(p, start, format!("{what} is not supported yet"))
}

pub(crate) fn tsmodel(p: &mut Parser) -> PResult<Command> {
    let mut output = TsOutput::default();
    let mut globals = Vec::new();
    // each block with whether its method was given
    let mut blocks: Vec<(TsModelBlock, bool)> = Vec::new();
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if try_global(p, &mut globals, &mut output, !blocks.is_empty(), false)? {
            continue;
        }
        let start = p.pos;
        let mut seen = Vec::new();
        if p.eat_word("MODEL") {
            let mut block = TsModelBlock {
                dependent: Vec::new(),
                independent: Vec::new(),
                outfile: None,
                prefix: "Model".into(),
                method: TsMethod::Expert {
                    arima: true,
                    exsmooth: true,
                    seasonal: true,
                },
            };
            while !at_end(p) {
                let at = p.pos;
                match keyword(
                    p,
                    &mut seen,
                    &["DEPENDENT", "INDEPENDENT", "OUTFILE", "OUTPMML", "PREFIX"],
                )? {
                    "DEPENDENT" => block.dependent = variables(p)?,
                    "INDEPENDENT" => block.independent = variables(p)?,
                    "OUTFILE" => block.outfile = Some(p.file("a model file")?),
                    "OUTPMML" => return Err(not_supported(p, at, "OUTPMML")),
                    _ => block.prefix = p.string("a quoted prefix")?,
                }
            }
            if block.dependent.is_empty() {
                return Err(error(p, start, "MODEL needs DEPENDENT="));
            }
            blocks.push((block, false));
            continue;
        }
        let methods = ["EXPERTMODELER", "EXSMOOTH", "ARIMA"];
        let Some(&sub) = methods
            .iter()
            .chain(&["AUTOOUTLIER"])
            .find(|n| p.eat_word(n))
        else {
            return Err(p.subcommand_error("TSMODEL", &["TRANSFERFUNCTION", "OUTLIER"]));
        };
        let Some((block, method_given)) = blocks.last_mut() else {
            return Err(error(p, start, format!("/{sub} comes after a /MODEL")));
        };
        if at_end(p) {
            return Err(error(p, start, format!("/{sub} is empty")));
        }
        if sub == "AUTOOUTLIER" {
            while !at_end(p) {
                let at = p.pos;
                if keyword(p, &mut seen, &["DETECT", "TYPE"])? == "DETECT" {
                    if one_of(p, &[("OFF", false), ("ON", true)])? {
                        return Err(not_supported(p, at, "AUTOOUTLIER DETECT=ON"));
                    }
                } else {
                    let types = [
                        "ADDITIVE",
                        "LEVELSHIFT",
                        "INNOVATIONAL",
                        "TRANSIENT",
                        "SEASONALADDITIVE",
                        "LOCALTREND",
                        "ADDITIVEPATCH",
                    ];
                    let table: Vec<(&str, ())> = types.iter().map(|n| (*n, ())).collect();
                    bracketed(p, |p| one_of(p, &table))?;
                }
            }
            continue;
        }
        if *method_given {
            return Err(error(
                p,
                start,
                "a MODEL block takes one of EXPERTMODELER, EXSMOOTH and ARIMA",
            ));
        }
        *method_given = true;
        block.method = match sub {
            "EXPERTMODELER" => {
                let (mut arima, mut exsmooth, mut seasonal) = (true, true, true);
                while !at_end(p) {
                    if keyword(p, &mut seen, &["TYPE", "TRYSEASONAL"])? == "TYPE" {
                        let types =
                            bracketed(p, |p| one_of(p, &[("ARIMA", true), ("EXSMOOTH", false)]))?;
                        arima = types.contains(&true);
                        exsmooth = types.contains(&false);
                    } else {
                        seasonal = yes_no(p)?;
                    }
                }
                TsMethod::Expert {
                    arima,
                    exsmooth,
                    seasonal,
                }
            }
            "EXSMOOTH" => {
                let (mut kind, mut how) = (None, TsTransform::None);
                while !at_end(p) {
                    if keyword(p, &mut seen, &["TYPE", "TRANSFORM"])? == "TYPE" {
                        kind = Some(one_of(p, &ES_TYPES)?);
                    } else {
                        how = transform(p)?;
                    }
                }
                let Some(kind) = kind else {
                    return Err(error(p, start, "EXSMOOTH needs TYPE="));
                };
                TsMethod::Exsmooth(kind, how)
            }
            _ => {
                let mut a = TsArima {
                    constant: true,
                    ..TsArima::default()
                };
                let names = [
                    "AR",
                    "ARSEASONAL",
                    "MA",
                    "MASEASONAL",
                    "DIFF",
                    "DIFFSEASONAL",
                    "CONSTANT",
                    "TRANSFORM",
                ];
                let lags = |p: &mut Parser| bracketed(p, |p| whole(p, 0, "a lag"));
                while !at_end(p) {
                    match keyword(p, &mut seen, &names)? {
                        "AR" => a.ar = lags(p)?,
                        "ARSEASONAL" => a.sar = lags(p)?,
                        "MA" => a.ma = lags(p)?,
                        "MASEASONAL" => a.sma = lags(p)?,
                        "DIFF" => a.d = whole(p, 0, "DIFF")?,
                        "DIFFSEASONAL" => a.sd = whole(p, 0, "DIFFSEASONAL")?,
                        "CONSTANT" => a.constant = yes_no(p)?,
                        _ => a.transform = transform(p)?,
                    }
                }
                if !seen.contains(&"AR") || !seen.contains(&"MA") {
                    return Err(error(p, start, "ARIMA needs AR= and MA="));
                }
                TsMethod::Arima(a)
            }
        };
    }
    if blocks.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "TSMODEL needs a /MODEL subcommand",
        ));
    }
    Ok(Command::Tsmodel(Box::new(Tsmodel {
        output,
        models: blocks.into_iter().map(|(b, _)| b).collect(),
    })))
}

pub(crate) fn tsapply(p: &mut Parser) -> PResult<Command> {
    let mut output = TsOutput::default();
    let mut globals = Vec::new();
    let mut files: Vec<TsModelFile> = Vec::new();
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if try_global(p, &mut globals, &mut output, !files.is_empty(), true)? {
            continue;
        }
        let start = p.pos;
        if !p.eat_word("MODEL") {
            return Err(p.subcommand_error("TSAPPLY", &[]));
        }
        let mut seen = Vec::new();
        let mut file = TsModelFile {
            file: String::new(),
            keep: None,
            drop: Vec::new(),
            outfile: None,
        };
        while !at_end(p) {
            let at = p.pos;
            match keyword(
                p,
                &mut seen,
                &["FILE", "DROP", "KEEP", "OUTFILE", "OUTPMML"],
            )? {
                "FILE" => file.file = p.file("a model file")?,
                "DROP" => file.drop = quoted_ids(p)?,
                "KEEP" => file.keep = Some(quoted_ids(p)?),
                "OUTFILE" => file.outfile = Some(p.file("a model file")?),
                _ => return Err(not_supported(p, at, "OUTPMML")),
            }
        }
        if !seen.contains(&"FILE") {
            return Err(error(p, start, "MODEL needs FILE="));
        }
        if seen.contains(&"DROP") && seen.contains(&"KEEP") {
            return Err(error(p, start, "DROP and KEEP exclude each other"));
        }
        if files.iter().any(|f| f.file == file.file) {
            return Err(error(p, start, "two MODEL subcommands name the same file"));
        }
        files.push(file);
    }
    if files.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "TSAPPLY needs a /MODEL subcommand",
        ));
    }
    Ok(Command::Tsapply(Box::new(Tsapply { output, files })))
}
