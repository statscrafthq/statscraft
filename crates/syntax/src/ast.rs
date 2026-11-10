//! The syntax tree. Nothing here is resolved against a dictionary: names are kept as written
//! (compare them case-insensitively) and variable lists stay unexpanded.

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Get {
        file: String,
    },
    Save {
        outfile: String,
        /// None when the command does not say.
        compression: Option<Compression>,
    },
    /// (variable, label) pairs in source order.
    VariableLabels(Vec<(String, String)>),
    ValueLabels(Vec<ValueLabels>),
    /// An empty value list clears the missing values of the variables.
    MissingValues(Vec<(Vec<VarItem>, Vec<MissingValue>)>),
    VariableLevel(Vec<(Vec<VarItem>, Level)>),
    Formats(Vec<(Vec<VarItem>, Format)>),
    Frequencies {
        variables: Vec<VarItem>,
    },
    Execute,
    Count(Vec<Count>),
    SortCases(Vec<SortKey>),
    /// (old names, new names) groups. All groups rename at the same moment, so names may swap.
    RenameVariables(Vec<(Vec<VarItem>, Vec<VarItem>)>),
    DeleteVariables(Vec<VarItem>),
    /// `WEIGHT BY var`; None is `WEIGHT OFF`.
    Weight(Option<String>),
    Aggregate(Aggregate),
    Compute {
        target: String,
        expr: Expr,
    },
    If {
        condition: Expr,
        target: String,
        expr: Expr,
    },
    SelectIf(Expr),
    Recode(Vec<Recode>),
    /// `STRING names (A20) / ...`: new string variables, each group with its A format.
    String(Vec<(Vec<String>, Format)>),
    /// `NUMERIC names [(F8.2)] / ...`: new numeric variables; None leaves the default format.
    Numeric(Vec<(Vec<String>, Option<Format>)>),
    DoIf(Expr),
    ElseIf(Expr),
    Else,
    EndIf,
    Ctables(crate::ctables::Ctables),
    Mrsets(Vec<crate::mrsets::Mrsets>),
    Descriptives(Descriptives),
    Crosstabs(Crosstabs),
    Means(Means),
    TTest(TTest),
    AddFiles(Merge),
    MatchFiles(Merge),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Descriptives {
    /// Each variable item with the z-score name written after it, as in `age (zage)`.
    pub variables: Vec<(VarItem, Option<String>)>,
    /// In display order, without duplicates. N is always shown and not listed.
    pub statistics: Vec<DescriptiveStatistic>,
    /// `/MISSING=LISTWISE`: only cases valid on every variable count.
    pub listwise: bool,
    /// `/MISSING=INCLUDE`: user-missing values count as valid.
    pub include: bool,
    /// `/SAVE`: add a z-score variable per variable.
    pub save: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptiveStatistic {
    Mean,
    SeMean,
    StdDev,
    Variance,
    Kurtosis,
    Skewness,
    Range,
    Min,
    Max,
    Sum,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Crosstabs {
    /// One entry per `/TABLES`, each a list of dimensions: rows, columns, then layers.
    pub tables: Vec<Vec<Vec<VarItem>>>,
    /// In display order, without duplicates. Empty for `/CELLS=NONE`.
    pub cells: Vec<CrosstabsCell>,
    pub chisq: bool,
    /// `/MISSING=INCLUDE`: user-missing values form categories of their own.
    pub include: bool,
    /// `/FORMAT=DVALUE`: row categories in descending order.
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrosstabsCell {
    Count,
    Row,
    Column,
    Total,
    Expected,
    Resid,
    Sresid,
    Asresid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Means {
    /// One entry per table list: the dependent variables, then one list per BY dimension.
    pub tables: Vec<Vec<Vec<VarItem>>>,
    /// In display order, without duplicates.
    pub cells: Vec<MeansCell>,
    /// `/STATISTICS=ANOVA`: analysis of variance by the first control variable.
    pub anova: bool,
    /// `/STATISTICS=LINEARITY`: the test of linearity as well.
    pub linearity: bool,
    pub missing: MeansMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeansMissing {
    /// No /MISSING: each dependent with the control variables on its own.
    Separate,
    /// TABLE: a case missing on any variable of the table list is left out.
    Table,
    /// INCLUDE: tablewise, user-missing values are valid.
    Include,
    /// DEPENDENT: tablewise, user-missing values are valid for the control variables only.
    Dependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeansCell {
    Mean,
    Count,
    StdDev,
    Median,
    SeMean,
    Sum,
    Min,
    Max,
    Range,
    Variance,
    Kurtosis,
    SeKurtosis,
    Skewness,
    SeSkewness,
    First,
    Last,
    Npct,
    Spct,
    Harmonic,
    Geometric,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TTest {
    /// `/VARIABLES`: the variables of the one-sample and independent-samples tests.
    pub variables: Vec<VarItem>,
    /// `/TESTVAL`: the one-sample test.
    pub testval: Option<f64>,
    /// `/GROUPS=var(...)`: the independent-samples test.
    pub groups: Option<(String, TTestGroups)>,
    /// `/PAIRS`: the paired-samples tests, one entry per analysis list.
    pub pairs: Vec<TTestPairs>,
    /// `/CRITERIA=CI(value)`, between 0 and 1.
    pub ci: f64,
    /// `/MISSING=LISTWISE`
    pub listwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// `/ES DISPLAY(TRUE)`, the default: the effect sizes table.
    pub effect_sizes: bool,
    pub standardizer: Standardizer,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TTestGroups {
    /// No values: 1 and 2.
    Default,
    /// One value: the cases at or above it against the cases below it.
    Cut(Value),
    /// Two values: the cases with the first against the cases with the second.
    Two(Value, Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TTestPairs {
    pub left: Vec<VarItem>,
    /// The list after WITH.
    pub right: Option<Vec<VarItem>>,
    /// `(PAIRED)`: the n-th variable before WITH with the n-th after it.
    pub paired: bool,
}

/// How the effect size of a paired test is standardized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standardizer {
    /// The standard deviation of the differences.
    Sd,
    /// That standard deviation corrected for the correlation of the pair.
    CorrectedSd,
    /// The root of the average of the two variances.
    Average,
}

/// ADD FILES and MATCH FILES.
#[derive(Debug, Clone, PartialEq)]
pub struct Merge {
    pub inputs: Vec<MergeInput>,
    /// Empty when there is no `/BY`.
    pub by: Vec<VarItem>,
    /// `/KEEP` and `/DROP` in the order written, applied to the result.
    pub subsets: Vec<Subset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeInput {
    /// None is `*`, the active dataset.
    pub file: Option<String>,
    /// `/TABLE` of MATCH FILES: a lookup file that adds variables but no cases.
    pub table: bool,
    /// `/RENAME` groups, as in RENAME VARIABLES.
    pub rename: Vec<(Vec<VarItem>, Vec<VarItem>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subset {
    Keep(Vec<VarItem>),
    Drop(Vec<VarItem>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarItem {
    Name(String),
    /// `a TO b`
    Range(String, String),
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Compressed,
    Uncompressed,
    ZCompressed,
}

/// One group of VALUE LABELS. An empty `labels` list removes the labels of the variables.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueLabels {
    pub vars: Vec<VarItem>,
    pub labels: Vec<(Value, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MissingValue {
    Value(Value),
    /// `low THRU high`
    Range(Bound, Bound),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    Lowest,
    Highest,
    Number(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Scale,
    Ordinal,
    Nominal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub kind: FormatType,
    pub width: u16,
    pub decimals: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatType {
    A,
    Ahex,
    F,
    N,
    E,
    Comma,
    Dot,
    Dollar,
    Pct,
    Z,
    Cca,
    Ccb,
    Ccc,
    Ccd,
    Cce,
    Date,
    Adate,
    Edate,
    Jdate,
    Sdate,
    Qyr,
    Moyr,
    Wkyr,
    Datetime,
    Ymdhms,
    Time,
    Mtime,
    Dtime,
    Wkday,
    Month,
    P,
    Pk,
    Ib,
    Pib,
    Pibhex,
    Rb,
    Rbhex,
}

const FORMAT_TYPES: [(&str, FormatType); 37] = [
    ("A", FormatType::A),
    ("AHEX", FormatType::Ahex),
    ("F", FormatType::F),
    ("N", FormatType::N),
    ("E", FormatType::E),
    ("COMMA", FormatType::Comma),
    ("DOT", FormatType::Dot),
    ("DOLLAR", FormatType::Dollar),
    ("PCT", FormatType::Pct),
    ("Z", FormatType::Z),
    ("CCA", FormatType::Cca),
    ("CCB", FormatType::Ccb),
    ("CCC", FormatType::Ccc),
    ("CCD", FormatType::Ccd),
    ("CCE", FormatType::Cce),
    ("DATE", FormatType::Date),
    ("ADATE", FormatType::Adate),
    ("EDATE", FormatType::Edate),
    ("JDATE", FormatType::Jdate),
    ("SDATE", FormatType::Sdate),
    ("QYR", FormatType::Qyr),
    ("MOYR", FormatType::Moyr),
    ("WKYR", FormatType::Wkyr),
    ("DATETIME", FormatType::Datetime),
    ("YMDHMS", FormatType::Ymdhms),
    ("TIME", FormatType::Time),
    ("MTIME", FormatType::Mtime),
    ("DTIME", FormatType::Dtime),
    ("WKDAY", FormatType::Wkday),
    ("MONTH", FormatType::Month),
    ("P", FormatType::P),
    ("PK", FormatType::Pk),
    ("IB", FormatType::Ib),
    ("PIB", FormatType::Pib),
    ("PIBHEX", FormatType::Pibhex),
    ("RB", FormatType::Rb),
    ("RBHEX", FormatType::Rbhex),
];

impl FormatType {
    /// The name as written in syntax, in upper case.
    pub fn name(self) -> &'static str {
        FORMAT_TYPES
            .iter()
            .find(|(_, kind)| *kind == self)
            .map_or("", |(name, _)| name)
    }

    pub(crate) fn from_name(name: &str) -> Option<FormatType> {
        FORMAT_TYPES
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, kind)| *kind)
    }
}

/// One group of RECODE. `into` is empty when the variables are recoded in place.
#[derive(Debug, Clone, PartialEq)]
pub struct Recode {
    pub vars: Vec<VarItem>,
    pub mappings: Vec<RecodeMapping>,
    pub into: Vec<VarItem>,
}

/// `(inputs = output)`
#[derive(Debug, Clone, PartialEq)]
pub struct RecodeMapping {
    pub inputs: Vec<RecodeInput>,
    pub output: RecodeOutput,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecodeInput {
    Value(Value),
    Range(Bound, Bound),
    Missing,
    Sysmis,
    Else,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecodeOutput {
    Value(Value),
    Sysmis,
    Copy,
}

/// One target of COUNT: `target = varlist (values) [varlist (values) ...]`. The values are
/// never `RecodeInput::Else`.
#[derive(Debug, Clone, PartialEq)]
pub struct Count {
    pub target: String,
    pub criteria: Vec<(Vec<VarItem>, Vec<RecodeInput>)>,
}

/// `varlist [(A|D)]` of SORT CASES and of AGGREGATE /BREAK.
#[derive(Debug, Clone, PartialEq)]
pub struct SortKey {
    pub vars: Vec<VarItem>,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Aggregate {
    /// None is `OUTFILE=*`, the active dataset.
    pub outfile: Option<String>,
    /// `MODE=ADDVARIABLES`: keep the cases and add the results to each of them.
    pub add_variables: bool,
    pub breaks: Vec<SortKey>,
    pub aggregations: Vec<Aggregation>,
}

/// `targets = FUNCTION(sources)`
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregation {
    /// New variable names (`a1 TO a3` counts up), each with the label written after it.
    pub targets: Vec<(VarItem, Option<String>)>,
    pub function: AggregateFunction,
    /// Empty for `N` and `NU` without arguments.
    pub sources: Vec<VarItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    Sum,
    Mean,
    Sd,
    Median,
    Min,
    Max,
    First,
    Last,
    N,
    Nu,
    Nmiss,
    Numiss,
}

/// An expression. Nothing is evaluated and function names are not checked.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Variable(String),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `NAME(args)`. The name is kept as written, including a dotted suffix as in `MEAN.2`.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// `a TO b`, only as a function argument.
    Range(String, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
