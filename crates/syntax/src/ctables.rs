//! CTABLES: the syntax tree of custom tables and its parser.

use crate::ast::{Bound, Command, Format, FormatType, MissingValue, Value, VarItem};
use crate::commands::format;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

#[derive(Debug, Clone, PartialEq)]
pub struct Ctables {
    /// `/VLABELS` groups, in source order; a later group overrides an earlier one.
    pub vlabels: Vec<(Vec<VarItem>, VlabelsDisplay)>,
    /// `/FORMAT EMPTY=`: what a cell without cases shows.
    pub empty: EmptyCells,
    /// `/FORMAT MISSING=`: the text of a statistic that cannot be computed. None leaves the
    /// cell empty.
    pub missing: Option<String>,
    /// `/MRSETS COUNTDUPLICATES=YES`: a category set counts a value as often as a case gives
    /// it in RESPONSES and the percentages based on responses.
    pub count_duplicates: bool,
    pub tables: Vec<CtablesTable>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlabelsDisplay {
    Default,
    Name,
    Label,
    Both,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmptyCells {
    Zero,
    Blank,
    Text(String),
}

/// One `/TABLE` with the subcommands that follow it.
#[derive(Debug, Clone, PartialEq)]
pub struct CtablesTable {
    pub rows: AxisExpr,
    pub columns: Option<AxisExpr>,
    /// `/SLABELS POSITION=ROW`: the summaries are listed in the rows instead of the columns.
    pub summaries_in_rows: bool,
    /// `/SLABELS VISIBLE`
    pub summary_labels: bool,
    pub categories: Vec<Categories>,
    pub title: Option<String>,
    pub caption: Option<String>,
    pub corner: Option<String>,
    pub sigtest: Option<Sigtest>,
    /// `/COMPARETEST`s, at most one of each type; a later one of the same type replaces it.
    pub comparetests: Vec<Comparetest>,
}

/// `/SIGTEST TYPE=CHISQUARE`: a chi-square test of independence for every subtable.
#[derive(Debug, Clone, PartialEq)]
pub struct Sigtest {
    pub alpha: f64,
    /// `INCLUDEMRSETS`; it has no effect while tables cannot hold multiple response sets.
    pub include_mrsets: bool,
    /// `CATEGORIES=SUBTOTALS`; it has no effect while category lists cannot hold subtotals.
    pub subtotals: bool,
}

/// `/COMPARETEST`: pairwise comparisons of the columns of every subtable.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparetest {
    pub kind: CompareKind,
    /// One or two significance levels, ascending.
    pub alpha: Vec<f64>,
    pub adjust: Adjust,
    /// `INCLUDEMRSETS`; it has no effect while tables cannot hold multiple response sets.
    pub include_mrsets: bool,
    /// `MEANSVARIANCE=ALLCATS` (the default): the variance of a means test is pooled from all
    /// tested categories; false pools it from the two categories compared.
    pub pool_all: bool,
    /// `CATEGORIES=SUBTOTALS`; it has no effect while category lists cannot hold subtotals.
    pub subtotals: bool,
    /// `SHOWSIG=YES`: the significance of every letter follows it in parentheses.
    pub show_sig: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareKind {
    /// `TYPE=PROP`: column proportions, z-tests.
    Proportions,
    /// `TYPE=MEAN`: column means, t-tests.
    Means,
}

/// `ADJUST=`: the correction of p-values for multiple comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjust {
    Bonferroni,
    BenjaminiHochberg,
    None,
}

/// An axis of a table: variables combined by stacking (`+`) and nesting (`>`).
#[derive(Debug, Clone, PartialEq)]
pub enum AxisExpr {
    Variable(AxisVariable),
    Stack(Box<AxisExpr>, Box<AxisExpr>),
    Nest(Box<AxisExpr>, Box<AxisExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxisVariable {
    pub name: String,
    /// `[S]` is Some(true), `[C]` Some(false); None lets the measurement level decide.
    pub scale: Option<bool>,
    /// The summaries in brackets after the variable, None when there are none.
    pub summaries: Option<Vec<Summary>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub function: SummaryFunction,
    /// The n of `PTILE n`.
    pub percentile: Option<f64>,
    pub label: Option<String>,
    pub format: Option<Format>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryFunction {
    pub statistic: Statistic,
    /// The `U` prefix: every case counts once, whatever its weight.
    pub unweighted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statistic {
    Count,
    Percent(PercentBase, PercentOf),
    Mean,
    Median,
    Mode,
    Minimum,
    Maximum,
    Range,
    Sum,
    Stddev,
    Variance,
    Semean,
    Ptile,
    Validn,
    Totaln,
    Missing,
    /// The number of responses to a multiple response set.
    Responses,
}

/// The area a percentage is taken of: `COLPCT` and the like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentBase {
    Row,
    Column,
    Table,
    Subtable,
    Layer,
    LayerRow,
    LayerColumn,
}

/// What a percentage counts: the `.COUNT` of `COLPCT.COUNT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentOf {
    Count,
    Validn,
    Totaln,
    Sum,
    /// `.RESPONSES`: responses of a multiple response set over the responses of the area.
    Responses,
    /// `.RESPONSES.COUNT`: responses over the count of cases of the area.
    ResponsesCount,
    /// `.COUNT.RESPONSES`: the count of cases over the responses of the area.
    CountResponses,
}

/// One `/CATEGORIES` subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct Categories {
    pub vars: Vec<VarItem>,
    /// An explicit list `[value, low THRU high, ...]`, or variable names for a multiple
    /// dichotomy set; None lists the values found.
    pub values: Option<Vec<CategoryItem>>,
    pub descending: bool,
    /// `KEY=LABEL`: sort by value label instead of value.
    pub by_label: bool,
    /// `MISSING=INCLUDE`: user-missing values are categories too.
    pub include_missing: bool,
    pub total: bool,
    pub total_label: Option<String>,
    pub total_before: bool,
    /// `EMPTY=EXCLUDE`: leave out categories without cases.
    pub exclude_empty: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CategoryItem {
    Value(Value),
    Range(Bound, Bound),
    /// A variable of a multiple dichotomy set.
    Name(String),
}

const SIMPLE_STATISTICS: [(&str, Statistic); 18] = [
    ("COUNT", Statistic::Count),
    ("MEAN", Statistic::Mean),
    ("MEDIAN", Statistic::Median),
    ("MODE", Statistic::Mode),
    ("MINIMUM", Statistic::Minimum),
    ("MAXIMUM", Statistic::Maximum),
    ("RANGE", Statistic::Range),
    ("SUM", Statistic::Sum),
    ("STDDEV", Statistic::Stddev),
    ("VARIANCE", Statistic::Variance),
    ("SEMEAN", Statistic::Semean),
    ("PTILE", Statistic::Ptile),
    ("VALIDN", Statistic::Validn),
    ("TOTALN", Statistic::Totaln),
    ("MISSING", Statistic::Missing),
    ("RESPONSES", Statistic::Responses),
    ("MIN", Statistic::Minimum),
    ("MAX", Statistic::Maximum),
];

const PERCENT_BASES: [(&str, PercentBase); 7] = [
    ("ROWPCT", PercentBase::Row),
    ("COLPCT", PercentBase::Column),
    ("TABLEPCT", PercentBase::Table),
    ("SUBTABLEPCT", PercentBase::Subtable),
    ("LAYERPCT", PercentBase::Layer),
    ("LAYERROWPCT", PercentBase::LayerRow),
    ("LAYERCOLPCT", PercentBase::LayerColumn),
];

const PERCENT_OF: [(&str, PercentOf); 7] = [
    ("COUNT", PercentOf::Count),
    ("VALIDN", PercentOf::Validn),
    ("TOTALN", PercentOf::Totaln),
    ("SUM", PercentOf::Sum),
    ("RESPONSES", PercentOf::Responses),
    ("RESPONSES.COUNT", PercentOf::ResponsesCount),
    ("COUNT.RESPONSES", PercentOf::CountResponses),
];

fn statistic(name: &str) -> Option<Statistic> {
    let upper = name.to_ascii_uppercase();
    if let Some((base, of)) = upper.split_once('.') {
        let base = PERCENT_BASES.iter().find(|(n, _)| *n == base)?.1;
        let of = PERCENT_OF.iter().find(|(n, _)| *n == of)?.1;
        return Some(Statistic::Percent(base, of));
    }
    SIMPLE_STATISTICS
        .iter()
        .find(|(n, _)| *n == upper)
        .map(|(_, s)| *s)
}

impl SummaryFunction {
    /// The function as written in syntax, for example `UCOLPCT.COUNT`.
    pub fn name(&self) -> String {
        let base = match self.statistic {
            Statistic::Percent(base, of) => {
                let base = PERCENT_BASES.iter().find(|(_, b)| *b == base);
                let of = PERCENT_OF.iter().find(|(_, o)| *o == of);
                format!("{}.{}", base.expect("listed").0, of.expect("listed").0)
            }
            simple => SIMPLE_STATISTICS
                .iter()
                .find(|(_, s)| *s == simple)
                .expect("listed")
                .0
                .to_string(),
        };
        if self.unweighted {
            format!("U{base}")
        } else {
            base
        }
    }

    fn parse(word: &str) -> Option<SummaryFunction> {
        if let Some(statistic) = statistic(word) {
            return Some(SummaryFunction {
                statistic,
                unweighted: false,
            });
        }
        let rest = word.strip_prefix(['U', 'u'])?;
        statistic(rest).map(|statistic| SummaryFunction {
            statistic,
            unweighted: true,
        })
    }
}

const SUMMARY_EXPECTED: &str = "a summary function such as COUNT, COLPCT.COUNT or MEAN";

const UNSUPPORTED_SUBCOMMANDS: [&str; 6] = [
    "PCOMPUTE",
    "PPROPERTIES",
    "WEIGHT",
    "HIDESMALLCOUNTS",
    "CRITERIA",
    "SMISSING",
];

pub(crate) fn at_end_of_subcommand(p: &Parser) -> bool {
    matches!(p.peek(), None | Some(TokenKind::Punct(Punct::Slash)))
}

fn not_supported(p: &Parser, start: usize, what: &str) -> SyntaxError {
    SyntaxError::new(
        p.span_from(start),
        format!("{what} in CTABLES is not supported yet"),
    )
}

/// `KEYWORD=` with the keyword already matched by the caller.
fn equals(p: &mut Parser) -> PResult<()> {
    p.expect(Punct::Eq)
}

/// One of `choices` after `=`; returns its index.
pub(crate) fn choice(p: &mut Parser, choices: &[&str]) -> PResult<usize> {
    equals(p)?;
    match choices.iter().position(|c| p.at_keyword(c)) {
        Some(i) => {
            p.pos += 1;
            Ok(i)
        }
        None => Err(p.unexpected(choices)),
    }
}

fn summary_format(p: &mut Parser, word: &str) -> PResult<Format> {
    let upper = word.to_ascii_uppercase();
    if ["NEGPAREN", "NEQUAL", "PAREN", "PCTPAREN"]
        .iter()
        .any(|special| upper.starts_with(special))
    {
        return Err(not_supported(p, p.pos, &format!("format {upper}")));
    }
    let parsed = format(word).map_err(|message| SyntaxError::new(p.span(), message))?;
    let usable = [
        FormatType::F,
        FormatType::Pct,
        FormatType::Comma,
        FormatType::Dollar,
    ];
    if !usable.contains(&parsed.kind) || parsed.width == 0 || parsed.width > 40 {
        let mut error = SyntaxError::new(
            p.span(),
            format!("format {upper} cannot be used for a summary"),
        );
        error.expected =
            vec!["F, PCT, COMMA or DOLLAR with a width of 1 to 40, such as PCT8.1".into()];
        return Err(error);
    }
    if usize::from(parsed.decimals) >= usize::from(parsed.width) && parsed.decimals > 0 {
        return Err(SyntaxError::new(
            p.span(),
            format!("format {upper} has no room for its decimals"),
        ));
    }
    p.pos += 1;
    Ok(parsed)
}

/// The summaries inside `[...]`, with the parser just after the `[`.
fn summaries(p: &mut Parser) -> PResult<Vec<Summary>> {
    let mut list = Vec::new();
    loop {
        let start = p.pos;
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&[SUMMARY_EXPECTED]));
        };
        let upper = word.to_ascii_uppercase();
        if matches!(upper.as_str(), "ECOUNT" | "EVALIDN" | "ETOTALN") {
            p.pos += 1;
            return Err(not_supported(
                p,
                start,
                &format!("summary function {upper}"),
            ));
        }
        let Some(function) = SummaryFunction::parse(word) else {
            let mut error = SyntaxError::new(p.span(), format!("unknown summary function {upper}"));
            error.expected = vec![SUMMARY_EXPECTED.into()];
            return Err(error
                .hint("percentages name their base: ROWPCT.COUNT, COLPCT.VALIDN, TABLEPCT.SUM"));
        };
        p.pos += 1;
        let percentile = if function.statistic == Statistic::Ptile {
            match p.number() {
                Some(n) if (0.0..=100.0).contains(&n) => Some(n),
                _ => return Err(p.unexpected(&["a percentile from 0 to 100 after PTILE"])),
            }
        } else {
            None
        };
        let (mut label, mut format) = (None, None);
        loop {
            match p.peek() {
                Some(TokenKind::Str(_)) if label.is_none() => {
                    label = Some(p.string("a label")?);
                }
                Some(TokenKind::Word(w))
                    if format.is_none() && w.chars().any(|c| c.is_ascii_digit()) =>
                {
                    format = Some(summary_format(p, w)?);
                }
                _ => break,
            }
        }
        list.push(Summary {
            function,
            percentile,
            label,
            format,
        });
        p.eat(Punct::Comma);
        if p.eat(Punct::RBracket) {
            return Ok(list);
        }
    }
}

fn variable(p: &mut Parser) -> PResult<AxisExpr> {
    let name = p.name("a variable name")?;
    let mut scale = None;
    if p.at(Punct::LBracket) {
        let level = match p.peek_at(1) {
            Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("C") => Some(false),
            Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("S") => Some(true),
            _ => None,
        };
        if level.is_some() && p.peek_at(2) == Some(&TokenKind::Punct(Punct::RBracket)) {
            scale = level;
            p.pos += 3;
        }
    }
    let summaries = if p.eat(Punct::LBracket) {
        Some(summaries(p)?)
    } else {
        None
    };
    Ok(AxisExpr::Variable(AxisVariable {
        name,
        scale,
        summaries,
    }))
}

fn primary(p: &mut Parser) -> PResult<AxisExpr> {
    if p.eat(Punct::LParen) {
        let inner = p.nested(stack)?;
        p.expect(Punct::RParen)?;
        if p.at(Punct::LBracket) {
            return Err(SyntaxError::new(
                p.span(),
                "summaries and [C] or [S] follow a variable name, not a parenthesis",
            ));
        }
        return Ok(inner);
    }
    if !matches!(p.peek(), Some(TokenKind::Word(_))) {
        return Err(p.unexpected(&["a variable name", "'('"]));
    }
    variable(p)
}

fn nest(p: &mut Parser) -> PResult<AxisExpr> {
    let mut expr = primary(p)?;
    while p.eat(Punct::Gt) {
        p.count_operator()?;
        expr = AxisExpr::Nest(Box::new(expr), Box::new(primary(p)?));
    }
    Ok(expr)
}

fn stack(p: &mut Parser) -> PResult<AxisExpr> {
    let mut expr = nest(p)?;
    while p.eat(Punct::Plus) {
        p.count_operator()?;
        expr = AxisExpr::Stack(Box::new(expr), Box::new(nest(p)?));
    }
    Ok(expr)
}

fn table(p: &mut Parser) -> PResult<CtablesTable> {
    let rows = stack(p)?;
    let columns = if p.eat_word("BY") {
        Some(stack(p)?)
    } else {
        None
    };
    if p.at_keyword("BY") {
        let start = p.pos;
        p.pos += 1;
        return Err(not_supported(p, start, "a layer axis (a second BY)"));
    }
    if !at_end_of_subcommand(p) {
        return Err(p.unexpected(&["'+'", "'>'", "BY", "'/'", "end of command"]));
    }
    Ok(CtablesTable {
        rows,
        columns,
        summaries_in_rows: false,
        summary_labels: true,
        categories: Vec::new(),
        title: None,
        caption: None,
        corner: None,
        sigtest: None,
        comparetests: Vec::new(),
    })
}

/// A variable list that ends before the next `KEYWORD=`.
pub(crate) fn var_list(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_)))
        && p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq))
    {
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            items.push(if p.eat_word("TO") {
                VarItem::Range(first, p.name("a variable name")?)
            } else {
                VarItem::Name(first)
            });
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

fn vlabels(p: &mut Parser) -> PResult<(Vec<VarItem>, VlabelsDisplay)> {
    if !p.eat_keyword("VARIABLES") {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    equals(p)?;
    let vars = var_list(p)?;
    if !p.eat_keyword("DISPLAY") {
        return Err(p.unexpected(&["DISPLAY"]));
    }
    let display = [
        VlabelsDisplay::Default,
        VlabelsDisplay::Name,
        VlabelsDisplay::Label,
        VlabelsDisplay::Both,
        VlabelsDisplay::None,
    ][choice(p, &["DEFAULT", "NAME", "LABEL", "BOTH", "NONE"])?];
    Ok((vars, display))
}

fn format_subcommand(p: &mut Parser, command: &mut Ctables) -> PResult<()> {
    while !at_end_of_subcommand(p) {
        if p.eat_keyword("EMPTY") {
            equals(p)?;
            command.empty = if p.eat_keyword("ZERO") {
                EmptyCells::Zero
            } else if p.eat_keyword("BLANK") {
                EmptyCells::Blank
            } else if let Some(TokenKind::Str(_)) = p.peek() {
                EmptyCells::Text(p.string("a quoted text")?)
            } else {
                return Err(p.unexpected(&["ZERO", "BLANK", "a quoted text"]));
            };
        } else if p.eat_keyword("MISSING") {
            equals(p)?;
            command.missing = Some(p.string("a quoted text")?);
        } else if p.eat_keyword("MINCOLWIDTH") || p.eat_keyword("MAXCOLWIDTH") {
            // column widths only matter to fixed-width output, which sizes columns itself
            equals(p)?;
            if !p.eat_keyword("DEFAULT") && p.number().is_none() {
                return Err(p.unexpected(&["DEFAULT", "a number"]));
            }
        } else if p.eat_keyword("UNITS") {
            choice(p, &["POINTS", "INCHES", "CM"])?;
        } else {
            return Err(p.unexpected(&["EMPTY", "MISSING", "MINCOLWIDTH", "MAXCOLWIDTH", "UNITS"]));
        }
    }
    Ok(())
}

fn slabels(p: &mut Parser, table: &mut CtablesTable) -> PResult<()> {
    while !at_end_of_subcommand(p) {
        if p.eat_keyword("POSITION") {
            let start = p.pos + 1;
            match choice(p, &["COLUMN", "ROW", "LAYER"])? {
                0 => table.summaries_in_rows = false,
                1 => table.summaries_in_rows = true,
                _ => return Err(not_supported(p, start, "SLABELS POSITION=LAYER")),
            }
        } else if p.eat_keyword("VISIBLE") {
            table.summary_labels = choice(p, &["YES", "NO"])? == 0;
        } else {
            return Err(p.unexpected(&["POSITION", "VISIBLE"]));
        }
    }
    Ok(())
}

fn clabels(p: &mut Parser) -> PResult<()> {
    let start = p.pos;
    if p.eat_keyword("AUTO") && at_end_of_subcommand(p) {
        return Ok(());
    }
    if p.eat_keyword("ROWLABELS") || p.eat_keyword("COLLABELS") {
        while !at_end_of_subcommand(p) {
            p.pos += 1;
        }
        return Err(not_supported(
            p,
            start,
            "moving category labels with CLABELS",
        ));
    }
    Err(p.unexpected(&["AUTO", "ROWLABELS", "COLLABELS"]))
}

fn category_items(p: &mut Parser) -> PResult<Vec<CategoryItem>> {
    let mut items = Vec::new();
    while !p.eat(Punct::RBracket) {
        let start = p.pos;
        if p.at_keyword("OTHERNM")
            || p.at_keyword("MISSING")
            || p.at_keyword("SUBTOTAL")
            || p.at_keyword("HSUBTOTAL")
        {
            let Some(TokenKind::Word(word)) = p.peek() else {
                unreachable!("checked above")
            };
            let word = word.to_uppercase();
            p.pos += 1;
            return Err(not_supported(
                p,
                start,
                &format!("{word} in a category list"),
            ));
        }
        if let Some(TokenKind::Punct(Punct::And)) = p.peek() {
            p.pos += 1;
            return Err(not_supported(p, start, "a computed category (&name)"));
        }
        if matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("LO") && !matches_keyword(w, "LOWEST"))
        {
            items.push(CategoryItem::Name(p.name("a variable name")?));
            p.eat(Punct::Comma);
            continue;
        }
        items.push(match p.value_or_range()? {
            Some(MissingValue::Value(value)) => CategoryItem::Value(value),
            Some(MissingValue::Range(low, high)) => CategoryItem::Range(low, high),
            None => return Err(p.unexpected(&["a value", "a range", "']'"])),
        });
        p.eat(Punct::Comma);
    }
    Ok(items)
}

fn categories(p: &mut Parser) -> PResult<Categories> {
    if !p.eat_keyword("VARIABLES") {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    equals(p)?;
    let vars = var_list(p)?;
    let mut spec = Categories {
        vars,
        values: None,
        descending: false,
        by_label: false,
        include_missing: false,
        total: false,
        total_label: None,
        total_before: false,
        exclude_empty: false,
    };
    if p.eat(Punct::LBracket) {
        spec.values = Some(category_items(p)?);
    }
    while !at_end_of_subcommand(p) {
        if p.eat_keyword("ORDER") {
            spec.descending = choice(p, &["A", "D"])? == 1;
        } else if p.eat_keyword("KEY") {
            equals(p)?;
            if p.eat_keyword("VALUE") {
                spec.by_label = false;
            } else if p.eat_keyword("LABEL") {
                spec.by_label = true;
            } else {
                let start = p.pos;
                p.pos += 1;
                return Err(not_supported(
                    p,
                    start,
                    "sorting categories by a summary (KEY=function)",
                ));
            }
        } else if p.eat_keyword("MISSING") {
            spec.include_missing = choice(p, &["EXCLUDE", "INCLUDE"])? == 1;
        } else if p.eat_keyword("TOTAL") {
            spec.total = choice(p, &["NO", "YES"])? == 1;
        } else if p.eat_keyword("LABEL") {
            equals(p)?;
            spec.total_label = Some(p.string("a quoted label")?);
        } else if p.eat_keyword("POSITION") {
            spec.total_before = choice(p, &["AFTER", "BEFORE"])? == 1;
        } else if p.eat_keyword("EMPTY") {
            spec.exclude_empty = choice(p, &["INCLUDE", "EXCLUDE"])? == 1;
        } else {
            return Err(p.unexpected(&[
                "ORDER", "KEY", "MISSING", "TOTAL", "LABEL", "POSITION", "EMPTY",
            ]));
        }
    }
    Ok(spec)
}

fn titles(p: &mut Parser, table: &mut CtablesTable) -> PResult<()> {
    while !at_end_of_subcommand(p) {
        let slot = if p.eat_keyword("TITLE") {
            &mut table.title
        } else if p.eat_keyword("CAPTION") {
            &mut table.caption
        } else if p.eat_keyword("CORNER") {
            &mut table.corner
        } else {
            return Err(p.unexpected(&["TITLE", "CAPTION", "CORNER"]));
        };
        equals(p)?;
        *slot = Some(p.string("a quoted text")?);
    }
    Ok(())
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    Ok(choice(p, &["NO", "YES"])? == 1)
}

/// A significance level: more than 0 and less than 1.
fn alpha(p: &mut Parser) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(a) if a > 0.0 && a < 1.0 => Ok(a),
        Some(_) => Err(SyntaxError::new(
            p.span_from(start),
            "a significance level must be more than 0 and less than 1, such as 0.05",
        )),
        None => Err(p.unexpected(&["a significance level such as 0.05"])),
    }
}

fn sigtest(p: &mut Parser, start: usize) -> PResult<Sigtest> {
    let mut spec = Sigtest {
        alpha: 0.05,
        include_mrsets: true,
        subtotals: false,
    };
    let mut typed = false;
    while !at_end_of_subcommand(p) {
        if p.eat_keyword("TYPE") {
            choice(p, &["CHISQUARE"])?;
            typed = true;
        } else if p.eat_keyword("ALPHA") {
            equals(p)?;
            spec.alpha = alpha(p)?;
        } else if p.eat_keyword("INCLUDEMRSETS") {
            spec.include_mrsets = yes_no(p)?;
        } else if p.eat_keyword("CATEGORIES") {
            spec.subtotals = choice(p, &["ALLVISIBLE", "SUBTOTALS"])? == 1;
        } else {
            return Err(p.unexpected(&["TYPE", "ALPHA", "INCLUDEMRSETS", "CATEGORIES"]));
        }
    }
    if !typed {
        return Err(SyntaxError::new(
            p.span_from(start),
            "/SIGTEST needs TYPE=CHISQUARE",
        ));
    }
    Ok(spec)
}

fn comparetest(p: &mut Parser, start: usize) -> PResult<Comparetest> {
    let mut spec = Comparetest {
        kind: CompareKind::Proportions,
        alpha: vec![0.05],
        adjust: Adjust::Bonferroni,
        include_mrsets: true,
        pool_all: true,
        subtotals: false,
        show_sig: false,
    };
    let (mut typed, mut merge, mut merge_at) = (false, false, start);
    while !at_end_of_subcommand(p) {
        if p.eat_keyword("TYPE") {
            spec.kind =
                [CompareKind::Proportions, CompareKind::Means][choice(p, &["PROP", "MEAN"])?];
            typed = true;
        } else if p.eat_keyword("ALPHA") {
            equals(p)?;
            let mut levels = vec![alpha(p)?];
            p.eat(Punct::Comma);
            if matches!(p.peek(), Some(TokenKind::Number(_))) {
                levels.push(alpha(p)?);
            }
            levels.sort_by(f64::total_cmp);
            levels.dedup();
            spec.alpha = levels;
        } else if p.eat_keyword("ADJUST") {
            spec.adjust = [Adjust::Bonferroni, Adjust::BenjaminiHochberg, Adjust::None]
                [choice(p, &["BONFERRONI", "BH", "NONE"])?];
        } else if p.eat_keyword("INCLUDEMRSETS") {
            spec.include_mrsets = yes_no(p)?;
        } else if p.eat_keyword("MEANSVARIANCE") {
            spec.pool_all = choice(p, &["ALLCATS", "TESTEDCATS"])? == 0;
        } else if p.eat_keyword("CATEGORIES") {
            spec.subtotals = choice(p, &["ALLVISIBLE", "SUBTOTALS"])? == 1;
        } else if p.at_keyword("MERGE") {
            merge_at = p.pos;
            p.pos += 1;
            merge = yes_no(p)?;
        } else if p.eat_keyword("STYLE") {
            // the style of letters merged into the main table; without MERGE=YES it does nothing
            choice(p, &["APA", "SIMPLE"])?;
        } else if p.eat_keyword("SHOWSIG") {
            spec.show_sig = yes_no(p)?;
        } else if p.eat_keyword("ORIGIN") {
            // a keyword kept for old syntax; it has no effect
            choice(p, &["COLUMN"])?;
        } else {
            return Err(p.unexpected(&[
                "TYPE",
                "ALPHA",
                "ADJUST",
                "INCLUDEMRSETS",
                "MEANSVARIANCE",
                "CATEGORIES",
                "MERGE",
                "STYLE",
                "SHOWSIG",
            ]));
        }
    }
    if !typed {
        return Err(SyntaxError::new(
            p.span_from(start),
            "/COMPARETEST needs TYPE=PROP or TYPE=MEAN",
        ));
    }
    // SHOWSIG=YES keeps the separate table, whatever MERGE says
    if merge && !spec.show_sig {
        let end = p.pos;
        p.pos = merge_at + 3;
        let error = not_supported(
            p,
            merge_at,
            "COMPARETEST MERGE=YES (letters in the main table)",
        )
        .hint("leave out MERGE=YES to get the letters in a table of their own");
        p.pos = end;
        return Err(error);
    }
    Ok(spec)
}

pub(crate) fn ctables(p: &mut Parser) -> PResult<Command> {
    let mut command = Ctables {
        vlabels: Vec::new(),
        empty: EmptyCells::Zero,
        missing: None,
        count_duplicates: false,
        tables: Vec::new(),
    };
    let mut first = true;
    loop {
        if !p.eat(Punct::Slash) && !first {
            break;
        }
        first = false;
        let start = p.pos;
        let table_only = |p: &Parser, tables: &[CtablesTable], name: &str| {
            if tables.is_empty() {
                Err(SyntaxError::new(
                    p.span_from(start),
                    format!("/{name} belongs to a table, so it must follow a /TABLE subcommand"),
                ))
            } else {
                Ok(())
            }
        };
        if p.eat_keyword("TABLE") {
            command.tables.push(table(p)?);
        } else if p.eat_keyword("VLABELS") {
            command.vlabels.push(vlabels(p)?);
        } else if p.eat_keyword("FORMAT") {
            format_subcommand(p, &mut command)?;
        } else if p.eat_keyword("MRSETS") {
            if !p.eat_keyword("COUNTDUPLICATES") {
                return Err(p.unexpected(&["COUNTDUPLICATES"]));
            }
            command.count_duplicates = choice(p, &["NO", "YES"])? == 1;
        } else if p.eat_keyword("SLABELS") {
            table_only(p, &command.tables, "SLABELS")?;
            slabels(p, command.tables.last_mut().expect("checked"))?;
        } else if p.eat_keyword("CLABELS") {
            table_only(p, &command.tables, "CLABELS")?;
            clabels(p)?;
        } else if p.eat_keyword("CATEGORIES") {
            table_only(p, &command.tables, "CATEGORIES")?;
            let spec = categories(p)?;
            command
                .tables
                .last_mut()
                .expect("checked")
                .categories
                .push(spec);
        } else if p.eat_keyword("TITLES") {
            table_only(p, &command.tables, "TITLES")?;
            titles(p, command.tables.last_mut().expect("checked"))?;
        } else if p.eat_keyword("SIGTEST") {
            table_only(p, &command.tables, "SIGTEST")?;
            let spec = sigtest(p, start)?;
            command.tables.last_mut().expect("checked").sigtest = Some(spec);
        } else if p.eat_keyword("COMPARETEST") {
            table_only(p, &command.tables, "COMPARETEST")?;
            let spec = comparetest(p, start)?;
            let tests = &mut command.tables.last_mut().expect("checked").comparetests;
            tests.retain(|t| t.kind != spec.kind);
            tests.push(spec);
        } else if p.at_keyword("SMISSING")
            && matches!(p.peek_at(2), Some(TokenKind::Word(w)) if matches_keyword(w, "VARIABLE"))
        {
            // the default treatment, so there is nothing to record
            p.pos += 3;
        } else if p.at_keyword("TABLES") {
            return Err(p.unexpected(&["TABLE"]));
        } else {
            return Err(p.subcommand_error("CTABLES", &UNSUPPORTED_SUBCOMMANDS));
        }
        if !at_end_of_subcommand(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.tables.is_empty() {
        return Err(p.unexpected(&["/TABLE"]));
    }
    Ok(Command::Ctables(command))
}
