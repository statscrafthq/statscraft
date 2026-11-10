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
    /// Column widths in the data grid, in characters.
    VariableWidth(Vec<(Vec<VarItem>, u32)>),
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
    Oneway(Oneway),
    Correlations(Correlations),
    NonparCorr(NonparCorr),
    Examine(Examine),
    AddFiles(Merge),
    MatchFiles(Merge),
    DataList(DataList),
    /// The lines between BEGIN DATA and END DATA, each with its line number in the source.
    BeginData(Vec<(u32, String)>),
    /// `SPLIT FILE [LAYERED | SEPARATE] BY varlist`; None is `SPLIT FILE OFF`.
    SplitFile(Option<SplitFile>),
    /// `FILTER BY var`; None is `FILTER OFF`, or `FILTER` alone when `bare`.
    Filter {
        variable: Option<String>,
        bare: bool,
    },
    Temporary,
    /// `N OF CASES n`
    NOfCases(u64),
    Sample(Sample),
    /// DO REPEAT with its body, the commands up to END REPEAT, which are parsed again for each
    /// set of replacements.
    DoRepeat(DoRepeat),
    Loop(LoopHeader),
    /// `END LOOP [IF condition]`
    EndLoop(Option<Expr>),
    Break,
    Vector(Vec<VectorSpec>),
    Autorecode(Autorecode),
    /// `COMPUTE v(index) = expr`, or `IF (condition) v(index) = expr`: an element of a vector.
    VectorAssign {
        condition: Option<Expr>,
        vector: String,
        index: Expr,
        expr: Expr,
    },
    /// `DEFINE name ... !ENDDEFINE`: the macro was taken in before parsing; the name.
    Define(String),
    /// INSERT or INCLUDE: the commands of the file were read in place of this one already.
    Insert {
        file: String,
        include: bool,
    },
    /// `BEGIN PROGRAM ... END PROGRAM`: a program in another language, not run.
    BeginProgram,
    /// SET: each setting in upper case, with its value as written (a string without quotes).
    Set(Vec<(String, String)>),
    /// SHOW: the settings asked for in upper case; empty for the main ones.
    Show(Vec<String>),
    /// PRESERVE, with any words written after it, which mean nothing.
    Preserve(Vec<String>),
    /// RESTORE, with any words written after it, which mean nothing.
    Restore(Vec<String>),
    Cache,
    /// `CD 'folder'`
    Cd(String),
    Dataset(DatasetCommand),
    /// `USE ALL` is None; else the first and last case number, and /PERMANENT.
    Use(Option<UseRange>),
    Display(Display),
    List(List),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatasetCommand {
    Name(String),
    Activate(String),
    Copy(String),
    Declare(String),
    /// None is `*`, the active dataset.
    Close(Option<String>),
    CloseAll,
    Display,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseRange {
    pub first: Option<u64>,
    pub last: Option<u64>,
    pub permanent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    /// NAMES, INDEX, VARIABLES, LABELS, DICTIONARY, ATTRIBUTES, @ATTRIBUTES, SCRATCH, VECTOR,
    /// MACROS or DOCUMENTS.
    pub what: String,
    pub sorted: bool,
    /// `/VARIABLES=varlist`; None for all.
    pub variables: Option<Vec<VarItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// None lists every variable.
    pub variables: Option<Vec<VarItem>>,
    /// 1-based numbers of the first and last case and the step.
    pub from: u64,
    pub to: Option<u64>,
    pub by: u64,
    pub numbered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Autorecode {
    pub variables: Vec<VarItem>,
    /// New names, one per variable.
    pub into: Vec<VarItem>,
    /// `/BLANK=MISSING`: a blank string becomes a user-missing code.
    pub blank_missing: bool,
    /// `/GROUP`: one scheme for all the variables.
    pub group: bool,
    pub descending: bool,
    /// `/PRINT`: a table of old and new values per variable.
    pub print: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoRepeat {
    /// Each stand-in name with its replacement list, in the order written.
    pub lists: Vec<(String, RepeatList)>,
    pub body: Vec<crate::split::RawCommand>,
    /// `END REPEAT PRINT`: show the generated commands.
    pub print: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RepeatList {
    /// Existing variables, or names of new ones (`v1 TO v5` counts up).
    Variables(Vec<VarItem>),
    /// Numbers or strings; `1 TO 5` is already counted out.
    Values(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopHeader {
    pub index: Option<LoopIndex>,
    /// `IF condition`: checked before every iteration.
    pub condition: Option<Expr>,
}

/// `variable = first TO last [BY step]`
#[derive(Debug, Clone, PartialEq)]
pub struct LoopIndex {
    pub variable: String,
    pub first: Expr,
    pub last: Expr,
    pub by: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSpec {
    /// One name for an existing range; one or more prefixes for new variables.
    pub names: Vec<String>,
    pub variables: VectorVariables,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VectorVariables {
    /// `name = first TO last`
    Existing(VarItem),
    /// `prefix(n [format])`: new variables prefix1 to prefixn.
    New { count: u32, format: Option<Format> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitFile {
    pub variables: Vec<VarItem>,
    /// SEPARATE: one table per split group; LAYERED (the default): the groups in one table.
    pub separate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sample {
    /// `SAMPLE p`: each case with probability p, between 0 and 1.
    Fraction(f64),
    /// `SAMPLE n FROM m`: exactly n of the first m cases.
    Exactly { n: u64, from: u64 },
}

/// DATA LIST: how to read text data, inline or from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct DataList {
    /// `FILE='path'`; None for inline data between BEGIN DATA and END DATA.
    pub file: Option<String>,
    pub layout: DataLayout,
    /// FREE and LIST: the characters that separate values (`TAB` is a tab). Empty: blanks and
    /// commas.
    pub delimiters: Vec<char>,
    /// `SKIP=n`: records to skip at the start of the data.
    pub skip: u32,
    /// In the order of the dictionary.
    pub fields: Vec<DataField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataLayout {
    /// Values in fixed columns; `records` lines make one case.
    Fixed { records: u32 },
    /// Values one after the other; a case may span lines and a line may hold several cases.
    Free,
    /// Values one after the other, one case per line.
    List,
}

/// One variable of DATA LIST with its input format.
#[derive(Debug, Clone, PartialEq)]
pub struct DataField {
    pub name: String,
    /// How the data are read.
    pub format: Format,
    /// How the values are shown: the input format with room for what the data may leave out
    /// (a decimal point, grouping commas, a dollar or percent sign).
    pub display: Format,
    /// FIXED: the record (1-based) and the first column (1-based); the width is the format's.
    pub record: u32,
    pub column: u32,
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
    /// The control variables named by `NPCT(var)` and `SPCT(var)`, as written.
    pub within: Vec<String>,
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
    /// `GMEDIAN`: the grouped median.
    GroupedMedian,
    /// `NPCT(var)`: percent of the N of the group of that control variable, an index into
    /// `Means::within`.
    NpctIn(usize),
    /// `SPCT(var)`: percent of the sum of the group of that control variable.
    SpctIn(usize),
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

#[derive(Debug, Clone, PartialEq)]
pub struct Oneway {
    pub dependents: Vec<VarItem>,
    pub factor: String,
    /// `/CONTRAST`: coefficients for the groups in ascending order of the factor.
    pub contrasts: Vec<Vec<f64>>,
    /// `/POSTHOC`: in the order written, without duplicates.
    pub posthoc: Vec<Posthoc>,
    /// `/POSTHOC ... ALPHA(value)`
    pub alpha: f64,
    pub descriptives: bool,
    pub homogeneity: bool,
    pub welch: bool,
    pub brown_forsythe: bool,
    /// `/MISSING=LISTWISE`
    pub listwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// `/CRITERIA=CILEVEL(value)`: the level of the confidence intervals.
    pub ci: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posthoc {
    Lsd,
    Bonferroni,
    Sidak,
    Tukey,
    Scheffe,
    GamesHowell,
    T2,
    Snk,
    /// `BTUKEY`: Tukey's b.
    TukeyB,
    Duncan,
    /// Hochberg's GT2.
    Gt2,
    Gabriel,
    /// Ryan-Einot-Gabriel-Welsch with the F test.
    Fregw,
    /// Ryan-Einot-Gabriel-Welsch with the studentized range.
    Qregw,
    /// Dunnett's T3.
    T3,
    /// Dunnett's C.
    C,
    /// `DUNNETT(refcat)`, `DUNNETTL` and `DUNNETTR`: each group against the reference group,
    /// the 1-based position among the groups in ascending order (None: the last).
    Dunnett(Option<u32>, DunnettSide),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DunnettSide {
    Both,
    /// `DUNNETTL`: is a group's mean smaller than the reference group's?
    Less,
    /// `DUNNETTR`: is it larger?
    Greater,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Correlations {
    /// One entry per variable list: the variables, and the list after WITH.
    pub lists: Vec<(Vec<VarItem>, Option<Vec<VarItem>>)>,
    /// `/PRINT=ONETAIL`
    pub one_tailed: bool,
    /// `/PRINT=NOSIG`: mark significant coefficients with asterisks.
    pub flag: bool,
    pub layout: CorrelationsLayout,
    /// `/STATISTICS=DESCRIPTIVES`
    pub descriptives: bool,
    /// `/STATISTICS=XPROD`: cross-products and covariances.
    pub xprod: bool,
    /// `/MISSING=LISTWISE`
    pub listwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NonparCorr {
    /// One entry per variable list: the variables, and the list after WITH.
    pub lists: Vec<(Vec<VarItem>, Option<Vec<VarItem>>)>,
    /// Spearman's rho: `/PRINT=SPEARMAN` (the default) or BOTH.
    pub spearman: bool,
    /// Kendall's tau-b: `/PRINT=KENDALL` or BOTH.
    pub kendall: bool,
    /// `/PRINT=ONETAIL`
    pub one_tailed: bool,
    /// `/PRINT=NOSIG`: mark significant coefficients with asterisks.
    pub flag: bool,
    pub layout: CorrelationsLayout,
    /// `/MISSING=LISTWISE`: per variable list.
    pub listwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Examine {
    pub dependents: Vec<VarItem>,
    /// The factor terms after BY: one variable, or two crossed with BY.
    pub factors: Vec<Vec<VarItem>>,
    /// `/TOTAL` (the default): also every dependent over all cases when there are factors.
    pub total: bool,
    /// `/ID=var`: labels the extreme cases instead of the case number.
    pub id: Option<String>,
    /// `/PERCENTILES`: the percentiles to show, None without the table.
    pub percentiles: Option<Vec<f64>>,
    /// The method of every percentile, the median and the interquartile range.
    pub method: PercentileMethod,
    /// `/STATISTICS=DESCRIPTIVES` (the default)
    pub descriptives: bool,
    /// `/STATISTICS=EXTREME(n)`: the n highest and lowest values.
    pub extreme: Option<usize>,
    /// `/CINTERVAL`, in percent as written.
    pub cinterval: f64,
    pub mestimators: Vec<MEstimator>,
    /// `/PLOT=NPPLOT`: the tests of normality.
    pub normality: bool,
    /// `/MISSING=PAIRWISE`: each dependent and factor on its own instead of listwise.
    pub pairwise: bool,
    /// `/MISSING=INCLUDE`: user-missing values of the dependents are valid.
    pub include: bool,
    /// `/MISSING=REPORT`: missing values of the factors form categories.
    pub report: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentileMethod {
    Haverage,
    Waverage,
    Round,
    Empirical,
    Aempirical,
}

/// An M-estimator with its constants as written (Andrews' c is still to be multiplied by pi).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MEstimator {
    Huber(f64),
    Tukey(f64),
    Hampel(f64, f64, f64),
    Andrew(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationsLayout {
    Full,
    Lower,
    /// The lower triangle without the diagonal.
    LowerNoDiagonal,
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
