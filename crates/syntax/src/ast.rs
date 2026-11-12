//! The syntax tree. Nothing here is resolved against a dictionary: names are kept as written
//! (compare them case-insensitively) and variable lists stay unexpanded.

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Get {
        file: String,
        /// `/KEEP`, `/DROP` and `/RENAME` in the order written.
        options: Vec<FileOption>,
    },
    Save {
        outfile: String,
        /// None when the command does not say.
        compression: Option<Compression>,
        /// `/KEEP`, `/DROP` and `/RENAME` in the order written; the active dataset stays as is.
        options: Vec<FileOption>,
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
    Frequencies(Frequencies),
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
    MultResponse(Box<MultResponse>),
    Means(Means),
    TTest(TTest),
    Oneway(Oneway),
    Correlations(Correlations),
    NonparCorr(NonparCorr),
    NparTests(Box<crate::npar::NparTests>),
    PartialCorr(crate::npar::PartialCorr),
    Examine(Examine),
    Regression(Box<crate::regression::Regression>),
    Reliability(Box<crate::reliability::Reliability>),
    Factor(Box<crate::factor::Factor>),
    QuickCluster(Box<crate::factor::QuickCluster>),
    Logistic(Box<crate::logistic::Logistic>),
    Glm(Box<crate::glm::Glm>),
    Cluster(Box<crate::cluster::Cluster>),
    Discriminant(Box<crate::discriminant::Discriminant>),
    Anova(Box<crate::anova::Anova>),
    Tree(Box<Tree>),
    Rank(Rank),
    AddFiles(Merge),
    MatchFiles(Merge),
    StarJoin(StarJoin),
    DataList(DataList),
    /// The lines between BEGIN DATA and END DATA, each with its line number in the source.
    BeginData(Vec<(u32, String)>),
    InputProgram,
    EndInputProgram,
    EndCase,
    EndFile,
    /// `LEAVE varlist`: variables that keep their value from case to case.
    Leave(Vec<VarItem>),
    /// `REREAD [FILE=file] [COLUMN=expression]`
    Reread {
        file: Option<String>,
        column: Option<Expr>,
    },
    RepeatingData(Box<RepeatingData>),
    FileType(Box<FileType>),
    RecordType(RecordType),
    EndFileType,
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
    /// `FILE HANDLE name /NAME='path'`. `ignored` names the subcommands that have no effect.
    FileHandle {
        name: String,
        path: String,
        encoding: Option<String>,
        ignored: Vec<String>,
    },
    Dataset(DatasetCommand),
    /// OUTPUT NEW, NAME, ACTIVATE, CLOSE, DISPLAY and EXPORT.
    Output(crate::output_docs::OutputCommand),
    MatrixData(Box<crate::matrix_data::MatrixData>),
    /// MATRIX with its statements up to END MATRIX.
    Matrix(Box<crate::matrix::MatrixProgram>),
    Report(Box<crate::report::Report>),
    /// FINISH: the commands after it are not read.
    Finish,
    /// `USE ALL` is None; else the first and last case number, and /PERMANENT.
    Use(Option<UseRange>),
    Display(Display),
    List(List),
    /// Like VALUE LABELS, but the other labels of the variables stay.
    AddValueLabels(Vec<ValueLabels>),
    FileLabel(String),
    /// DOCUMENT or ADD DOCUMENT: the lines to add.
    AddDocument(Vec<String>),
    DropDocuments,
    /// VARIABLE ROLE: each role in lower case (input, target, both, none, partition, split).
    VariableRole(Vec<(String, Vec<VarItem>)>),
    /// VARIABLE ALIGNMENT: left, right or center.
    VariableAlignment(Vec<(Vec<VarItem>, String)>),
    /// VARIABLE ATTRIBUTE, or DATAFILE ATTRIBUTE when `variables` is None.
    Attribute(Vec<AttributeChange>),
    AlterType(Vec<AlterType>),
    ApplyDictionary(ApplyDictionary),
    GetData(GetData),
    SaveTranslate(SaveTranslate),
    /// EXPORT: the active dataset as a portable file.
    Export(Export),
    /// IMPORT: a portable file as the active dataset.
    Import(Import),
    GetTranslate(GetTranslate),
    /// `SORT VARIABLES BY key (A | D)`
    SortVariables {
        key: VariableKey,
        descending: bool,
    },
    Flip(Flip),
    /// UPDATE: the first input is the primary file, the others are transaction files.
    Update(Merge),
    VarsToCases(VarsToCases),
    CasesToVars(CasesToVars),
    /// TITLE, or SUBTITLE when `subtitle`.
    Title {
        text: String,
        subtitle: bool,
    },
    Echo(String),
    NewFile,
    ClearTransformations,
    /// `ERASE FILE='path'`
    Erase(String),
    /// `SYSFILE INFO FILE='path'`
    SysfileInfo(String),
    Permissions {
        file: String,
        read_only: bool,
    },
    /// HOST: the operating system commands, which are never run.
    Host(Vec<String>),
    Date(crate::output::DateSpec),
    /// PRINT, PRINT EJECT or WRITE.
    Print(Box<crate::output::Print>),
    PrintSpace(crate::output::PrintSpace),
    PrintFormats(Vec<(Vec<VarItem>, Format)>),
    WriteFormats(Vec<(Vec<VarItem>, Format)>),
    Xsave(Box<crate::output::Xsave>),
}

/// What SORT VARIABLES sorts by.
#[derive(Debug, Clone, PartialEq)]
pub enum VariableKey {
    Name,
    Type,
    Format,
    Label,
    Values,
    Missing,
    Measure,
    Role,
    Columns,
    Alignment,
    Attribute(String),
}

/// FLIP: None in `variables` transposes them all.
#[derive(Debug, Clone, PartialEq)]
pub struct Flip {
    pub variables: Option<Vec<VarItem>>,
    pub newnames: Option<String>,
}

/// A new variable named on a restructuring subcommand, with its optional label.
#[derive(Debug, Clone, PartialEq)]
pub struct NewVariable {
    pub name: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarsToCases {
    /// `/MAKE new ["label"] FROM varlist`, in order.
    pub makes: Vec<(NewVariable, Vec<VarItem>)>,
    pub index: Vec<VarsToCasesIndex>,
    pub id: Option<NewVariable>,
    /// NULL=KEEP; the default drops rows whose MAKE values are all null.
    pub keep_null: bool,
    pub count: Option<NewVariable>,
    /// `/KEEP` or `/DROP`; at most one of them.
    pub subset: Option<Subset>,
}

/// One variable of VARSTOCASES /INDEX.
#[derive(Debug, Clone, PartialEq)]
pub struct VarsToCasesIndex {
    pub variable: NewVariable,
    pub kind: IndexKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexKind {
    /// `i`: 1, 2, ... for the variables of the FROM lists.
    Sequence,
    /// `i(n)`: one factor of n levels; several make a design whose last factor varies fastest.
    Levels(usize),
    /// `i(make)`: the names of the FROM variables of that MAKE.
    Names(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CasesToVars {
    pub id: Vec<VarItem>,
    pub index: Vec<VarItem>,
    pub fixed: Vec<VarItem>,
    /// AUTOFIX=NO turns it off.
    pub autofix: bool,
    /// `/VIND [ROOT=name]`: the root, "ind" by default.
    pub indicators: Option<String>,
    pub count: Option<NewVariable>,
    /// (variable, root)
    pub rename: Vec<(String, String)>,
    pub separator: String,
    /// GROUPBY=INDEX; else the new variables are grouped by the variable they come from.
    pub group_by_index: bool,
    pub drop: Vec<VarItem>,
}

/// GET DATA /TYPE=TXT.
#[derive(Debug, Clone, PartialEq)]
pub struct GetData {
    pub file: String,
    pub encoding: Option<String>,
    /// ARRANGEMENT=FIXED; else DELIMITED.
    pub fixed: bool,
    /// FIRSTCASE: the 1-based line of the first case.
    pub first_case: u32,
    /// DELCASE=VARIABLES n: n values make a case, across lines; None is DELCASE=LINE.
    pub values_per_case: Option<u32>,
    /// FIXCASE: lines per case of fixed data.
    pub lines_per_case: u32,
    /// None when DELIMITERS is not given.
    pub delimiters: Option<Vec<char>>,
    pub qualifier: Option<char>,
    /// IMPORTCASE=FIRST n
    pub first_cases: Option<u64>,
    /// MULTIPLESPACES IGNORE=YES
    pub ignore_multiple_spaces: bool,
    /// LEADINGSPACES IGNORE=YES
    pub ignore_leading_spaces: bool,
    pub variables: Vec<GetDataVariable>,
    /// TYPE=XLS, XLSX or XLSM; None is TYPE=TXT.
    pub spreadsheet: Option<Spreadsheet>,
}

/// The settings of GET DATA for a workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct Spreadsheet {
    /// TYPE=XLS, the binary workbook of Excel 95 to 2003.
    pub binary: bool,
    pub sheet: Sheet,
    /// CELLRANGE=RANGE as (first row, first column, last row, last column), counted from 0;
    /// None is FULL.
    pub range: Option<(u32, u32, u32, u32)>,
    pub read_names: bool,
    pub assumed_width: Option<u32>,
    /// HIDDEN IGNORE=YES
    pub ignore_hidden: bool,
    /// TRAILINGSPACES IGNORE=YES
    pub ignore_trailing_spaces: bool,
    /// DATATYPEMIN PERCENTAGE
    pub type_min: Option<f64>,
}

/// SAVE TRANSLATE: the active dataset as a text file or a workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveTranslate {
    pub outfile: String,
    pub kind: TranslateType,
    /// FIELDNAMES: None writes no names, Some(false) the names, Some(true) the labels.
    pub field_names: Option<bool>,
    /// CELLS=LABELS
    pub labels: bool,
    /// TEXTOPTIONS DELIMITER; None is the default for the decimal mark.
    pub delimiter: Option<char>,
    /// TEXTOPTIONS QUALIFIER
    pub qualifier: char,
    /// TEXTOPTIONS DECIMAL: Some(true) is COMMA; None follows SET DECIMAL.
    pub decimal_comma: Option<bool>,
    /// TEXTOPTIONS FORMAT=VARIABLE; else PLAIN.
    pub print_formats: bool,
    /// EXCELOPTIONS SHEET
    pub sheet: Option<String>,
    pub replace: bool,
    /// `/KEEP`, `/DROP` and `/RENAME` in the order written; the active dataset stays as is.
    pub options: Vec<FileOption>,
    /// MISSING=RECODE
    pub recode_missing: bool,
    /// UNSELECTED=DELETE
    pub delete_unselected: bool,
    pub encoding: Option<String>,
    /// BOM=NO turns off the byte order mark of a UTF encoding.
    pub bom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateType {
    Csv,
    Tab,
    /// An Excel 2007 workbook (TYPE=XLS /VERSION=12).
    Xlsx,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sheet {
    /// SHEET=INDEX n, from 1.
    Index(u32),
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GetDataVariable {
    pub name: String,
    /// Fixed data: the 1-based record and the 0-based first and last column.
    pub record: u32,
    pub columns: Option<(u32, u32)>,
    /// None is AUTO. A width of 0 means none was given.
    pub format: Option<Format>,
    /// Format X: read past the values and keep no variable.
    pub skip: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeChange {
    pub variables: Option<Vec<VarItem>>,
    /// name, 1-based array index, value
    pub set: Vec<(String, Option<usize>, String)>,
    /// name, 1-based array index (None: the whole attribute)
    pub delete: Vec<(String, Option<usize>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterType {
    pub vars: Vec<VarItem>,
    /// `(F = A8)`: only variables of this format type, and of this width when given.
    pub only: Option<(FormatType, Option<u16>)>,
    pub to: AlterTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTarget {
    Format(Format),
    /// AMIN [+ n[%]]: the narrowest string width that holds every value, plus n bytes or n percent.
    Amin {
        plus: f64,
        percent: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyDictionary {
    /// None is `*`, the active dataset.
    pub from: Option<String>,
    pub source: Option<Vec<VarItem>>,
    pub target: Option<Vec<VarItem>>,
    pub newvars: bool,
    /// FILEINFO keywords in upper case, each with REPLACE or MERGE when given.
    pub fileinfo: Vec<(String, Option<String>)>,
    /// VARINFO keywords; None applies the defaults.
    pub varinfo: Option<Vec<(String, Option<String>)>>,
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
    /// `END=var`: in an input program, the variable that turns 1 at the end of the data.
    pub end: Option<String>,
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

/// A column given as a number or by a variable that holds it per case.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Column(u32),
    Variable(String),
}

/// REPEATING DATA: one case per repeating group of the records an input program reads.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatingData {
    pub file: Option<String>,
    /// STARTS: the first column of the groups on the first record, and the last one.
    pub starts: Place,
    pub ends: Option<Place>,
    /// OCCURS: how many groups an input case has.
    pub occurs: Place,
    /// LENGTH: the columns of one group; without it `group_length`.
    pub length: Option<Place>,
    /// CONTINUED: the first and last column of the groups on continuation records.
    pub continued: Option<(u32, Option<u32>)>,
    /// ID: the columns of the identification variable on continuation records, and the
    /// variable.
    pub id: Option<(u32, u32, String)>,
    /// DATA: the variables, with columns counted within a group.
    pub fields: Vec<DataField>,
    /// The columns from the start of a group to the end of the last variable or skip.
    pub group_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Mixed,
    Grouped,
    Nested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicate {
    Warn,
    NoWarn,
    Case,
}

/// The record or case identifier of FILE TYPE: `[name] first[-last] [(format)]`.
#[derive(Debug, Clone, PartialEq)]
pub struct IdField {
    pub name: Option<String>,
    pub column: u32,
    pub format: Format,
    pub display: Format,
}

/// FILE TYPE: a file of several kinds of records. Options left out are None.
#[derive(Debug, Clone, PartialEq)]
pub struct FileType {
    pub kind: FileKind,
    pub file: Option<String>,
    pub record: IdField,
    pub case: Option<IdField>,
    /// WILD, MISSING: true is WARN.
    pub wild: Option<bool>,
    pub duplicate: Option<Duplicate>,
    pub missing: Option<bool>,
    pub ordered: Option<bool>,
}

/// RECORD TYPE: the record types the DATA LIST after it reads. No values is OTHER.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordType {
    pub values: Vec<Value>,
    pub skip: bool,
    /// CASE: the first and last column of the case identifier on these records.
    pub case: Option<(u32, u32)>,
    pub duplicate: Option<Duplicate>,
    pub missing: Option<bool>,
    pub spread: bool,
}

/// FREQUENCIES.
#[derive(Debug, Clone, PartialEq)]
pub struct Frequencies {
    pub variables: Vec<VarItem>,
    /// `/FORMAT=NOTABLE`: no frequency tables.
    pub notable: bool,
    /// `/FORMAT=LIMIT(n)`: no frequency table for a variable with more than n categories.
    pub limit: Option<usize>,
    /// `/FORMAT=AVALUE | DVALUE | AFREQ | DFREQ`
    pub order: FrequencyOrder,
    /// `/MISSING=INCLUDE`: user-missing values are valid.
    pub include: bool,
    /// In display order, without duplicates.
    pub statistics: Vec<FrequencyStatistic>,
    /// From `/PERCENTILES` and `/NTILES`, ascending, without duplicates.
    pub percentiles: Vec<f64>,
    /// `/GROUPED`: variables whose values stand for groups, with how the groups are bounded.
    pub grouped: Vec<(Vec<VarItem>, Grouping)>,
    /// The chart subcommands given, in the order given; a repeated one replaces the earlier.
    pub charts: Vec<FrequencyChart>,
    /// `/ORDER=VARIABLE`: a statistics table per variable instead of one for all.
    pub by_variable: bool,
}

/// `/BARCHART`, `/PIECHART` or `/HISTOGRAM` of FREQUENCIES with its options.
#[derive(Debug, Clone, PartialEq)]
pub struct FrequencyChart {
    pub kind: FrequencyChartKind,
    /// `MIN(n)` and `MAX(n)`: values outside are not plotted.
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    /// `PERCENT`: the chart shows percentages instead of counts (bar and pie).
    pub percent: bool,
    /// The n of `FREQ(n)` or `PERCENT(n)`: the top of the value axis.
    pub scale: Option<f64>,
    /// Pie: one slice for the missing values (`MISSING`, the default) or none (`NOMISSING`).
    pub missing: bool,
    /// Histogram: `NORMAL`, a normal curve over the bars.
    pub normal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyChartKind {
    Bar,
    Pie,
    Histogram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyOrder {
    AValue,
    DValue,
    AFreq,
    DFreq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrequencyStatistic {
    Mean,
    SeMean,
    Median,
    Mode,
    StdDev,
    Variance,
    Skewness,
    SeSkewness,
    Kurtosis,
    SeKurtosis,
    Range,
    Minimum,
    Maximum,
    Sum,
}

/// How grouped values are bounded.
#[derive(Debug, Clone, PartialEq)]
pub enum Grouping {
    /// The values are the midpoints of the groups.
    Midpoints,
    /// The values are midpoints of groups this wide.
    Width(f64),
    /// The groups lie between these ascending boundaries.
    Boundaries(Vec<f64>),
}

/// RANK.
#[derive(Debug, Clone, PartialEq)]
pub struct Rank {
    /// Each variable item with its direction: true for descending (D).
    pub variables: Vec<(VarItem, bool)>,
    /// The variables after BY: ranks are computed within each combination of their values.
    pub by: Vec<VarItem>,
    /// The function subcommands in the order written, each with the names after INTO.
    pub functions: Vec<(RankFunction, Vec<String>)>,
    pub ties: RankTies,
    /// `/FRACTION`; None when it is not given (Blom's formula).
    pub fraction: Option<RankFraction>,
    /// `/PRINT=YES` (the default): the table of the new variables.
    pub print: bool,
    /// `/MISSING=INCLUDE`: user-missing values are ranked.
    pub include: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankFunction {
    Rank,
    Rfraction,
    Percent,
    Proportion,
    N,
    /// The number of groups.
    Ntiles(u32),
    Savage,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankTies {
    Mean,
    Low,
    High,
    Condense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankFraction {
    Blom,
    Rankit,
    Tukey,
    Vw,
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
    /// `/COUNT`: what is rounded or truncated to whole counts.
    pub count: WeightCount,
    /// `/COUNT TRUNCATE`: truncate instead of rounding.
    pub truncate: bool,
    /// `/BARCHART`: a clustered bar chart per table.
    pub barchart: bool,
}

/// How CROSSTABS turns case weights into counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeightCount {
    /// Weights as they are (without /COUNT); only the exact test takes whole counts.
    #[default]
    Asis,
    /// The weight of every case, before it is counted.
    Case,
    /// The summed weight of every cell, before anything is computed from it.
    Cell,
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

/// MULT RESPONSE: frequency tables and crosstabulations of multiple response groups.
#[derive(Debug, Clone, PartialEq)]
pub struct MultResponse {
    pub groups: Vec<MrGroup>,
    /// VARIABLES: each list with the range of values it tabulates.
    pub variables: Vec<(Vec<VarItem>, f64, f64)>,
    /// FREQUENCIES: groups and variables; TO follows the order of GROUPS and VARIABLES.
    pub frequencies: Vec<VarItem>,
    pub tables: Vec<MrTable>,
    /// MISSING=MDGROUP: a case missing on any variable of a dichotomy group is left out of it.
    pub mdgroup: bool,
    /// MISSING=MRGROUP: the same for category groups, also for a value out of range.
    pub mrgroup: bool,
    /// MISSING=INCLUDE: user-missing values in the range count.
    pub include: bool,
    /// FORMAT=NOLABELS: values instead of value labels.
    pub no_labels: bool,
    /// FORMAT=CONDENSE: the frequency tables of category groups and variables without labels.
    pub condense: bool,
    /// BASE=RESPONSES (Some(true)) or CASES (Some(false)); None when not given.
    pub base_responses: Option<bool>,
    /// CELLS in display order; the count always comes first.
    pub cells: Vec<CrosstabsCell>,
    /// FORMAT keywords that only change the printed layout (DOUBLE, ONEPAGE).
    pub layout: Vec<String>,
}

/// A group of GROUPS: one value (a dichotomy group) or a range (a category group).
#[derive(Debug, Clone, PartialEq)]
pub struct MrGroup {
    pub name: String,
    pub label: Option<String>,
    pub variables: Vec<VarItem>,
    pub value: f64,
    /// The top of the range of a category group; None for a dichotomy group.
    pub max: Option<f64>,
}

/// One table list of TABLES: rows, columns, then control dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct MrTable {
    pub dimensions: Vec<Vec<VarItem>>,
    pub paired: bool,
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
pub struct Tree {
    pub dependent: String,
    /// The level in square brackets after a variable; None takes the dictionary's.
    pub dependent_level: Option<Level>,
    pub predictors: Vec<(VarItem, Option<Level>)>,
    /// `FORCE = var [level]`: the variable of the first split.
    pub force: Option<(String, Option<Level>)>,
    /// `/METHOD TYPE=EXHAUSTIVECHAID`; CHAID otherwise.
    pub exhaustive: bool,
    /// `/TREE DISPLAY`: false for NONE.
    pub display: bool,
    /// `/TREE BRANCHSTATISTICS`
    pub branch_statistics: bool,
    /// `/TREE NODEDEFS`
    pub node_definitions: bool,
    /// `/DEPCATEGORIES USEVALUES`; None uses the valid values.
    pub use_values: Option<Vec<TreeCategory>>,
    /// `/DEPCATEGORIES TARGET`
    pub targets: Vec<Value>,
    pub print: TreePrint,
    /// `/GAIN CATEGORYTABLE`
    pub gain_table: bool,
    /// `/GAIN CUMULATIVE`
    pub gain_cumulative: bool,
    /// `/GAIN SORT=ASCENDING`
    pub gain_ascending: bool,
    /// `/SAVE`: what to save, with the name or root given in parentheses.
    pub save: Vec<(TreeSave, Option<String>)>,
    /// `/GROWTHLIMIT MAXDEPTH`; None is AUTO.
    pub max_depth: Option<usize>,
    pub min_parent: f64,
    pub min_child: f64,
    pub alpha_split: f64,
    pub alpha_merge: f64,
    /// `/CHAID CHISQUARE=LR`
    pub likelihood_ratio: bool,
    pub converge: f64,
    pub max_iterations: usize,
    /// `/CHAID ADJUST=BONFERRONI`
    pub bonferroni: bool,
    /// `/CHAID INTERVALS`: the number for every scale predictor, then lists with their own.
    pub intervals: usize,
    pub variable_intervals: Vec<(Vec<VarItem>, usize)>,
    /// `/SCORES CUSTOM`; None is EQUALINCREMENTS.
    pub scores: Option<Vec<(Value, f64)>>,
    /// `/MISSING NOMINALMISSING=VALID`
    pub nominal_missing_valid: bool,
    /// What the command accepts but does not use, each said in a warning when it runs.
    pub ignored: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeCategory {
    Valid,
    Missing,
    Value(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSave {
    NodeId,
    PredictedValue,
    PredictedProbability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreePrint {
    pub model_summary: bool,
    pub classification: bool,
    pub risk: bool,
    pub case_summary: bool,
    pub tree_table: bool,
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
    /// `/PLOT` BOXPLOT, STEMLEAF and HISTOGRAM, in the order asked; BOXPLOT and STEMLEAF
    /// without a PLOT subcommand.
    pub plots: Vec<String>,
    /// `/PLOT=SPREADLEVEL(n)`: Some(None) without a power, Some(Some(0)) for natural logs.
    pub spread_level: Option<Option<f64>>,
    /// `/COMPARE=VARIABLES`: the boxes of all dependents in one plot per cell, instead of one
    /// plot per dependent with a box per cell (GROUPS).
    pub compare_variables: bool,
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

/// ADD FILES, MATCH FILES and UPDATE.
#[derive(Debug, Clone, PartialEq)]
pub struct Merge {
    pub inputs: Vec<MergeInput>,
    /// Empty when there is no `/BY`.
    pub by: Vec<VarItem>,
    /// `/KEEP`, `/DROP` and `/MAP` in the order written, applied to the result.
    pub subsets: Vec<Subset>,
    /// `/FIRST=var` of ADD FILES and MATCH FILES: 1 for the first case of each key, else 0.
    pub first: Option<String>,
    /// `/LAST=var`: 1 for the last case of each key, else 0.
    pub last: Option<String>,
}

/// STAR JOIN: every case of the FROM source with the fields of the JOIN sources whose keys
/// match it, as an SQL left outer join.
#[derive(Debug, Clone, PartialEq)]
pub struct StarJoin {
    /// The SELECT fields in order; None for `SELECT *`.
    pub select: Option<Vec<JoinField>>,
    /// FROM, then every JOIN in order.
    pub sources: Vec<JoinSource>,
    /// OUTFILE FILE=: a file or a dataset; None is `*`, the active dataset.
    pub outfile: Option<String>,
    /// PASSPROTECT with a PASSWORD other than NONE: the output file is to be encrypted.
    pub encrypt: bool,
}

/// A field of SELECT: `alias.name [AS new]`.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinField {
    /// The index of its source in `StarJoin::sources`.
    pub source: usize,
    pub name: String,
    pub rename: Option<String>,
}

/// The FROM source or a JOIN source: a data file, a dataset or `*`, with its alias.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinSource {
    /// None is `*`, the active dataset.
    pub file: Option<String>,
    pub alias: String,
    /// JOIN: the key pairs of ON, (field of the FROM source, field of this source).
    pub on: Vec<(String, String)>,
    /// JOIN: `IN=name`, a new variable that is 1 when the case found a match here, else 0.
    pub flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeInput {
    /// None is `*`, the active dataset.
    pub file: Option<String>,
    /// `/TABLE` of MATCH FILES: a lookup file that adds variables but no cases.
    pub table: bool,
    /// `/RENAME` groups, as in RENAME VARIABLES.
    pub rename: Vec<(Vec<VarItem>, Vec<VarItem>)>,
    /// `/IN=var`: 1 for the cases this file has, else 0.
    pub flag: Option<String>,
}

/// A subcommand of GET and SAVE that changes which variables the file holds.
#[derive(Debug, Clone, PartialEq)]
pub enum FileOption {
    Keep(Vec<VarItem>),
    Drop(Vec<VarItem>),
    /// Groups as in RENAME VARIABLES.
    Rename(Vec<(Vec<VarItem>, Vec<VarItem>)>),
    /// `/MAP` of EXPORT, IMPORT and GET TRANSLATE: a table of the variables as the options
    /// before it leave them. Changes nothing.
    Map,
}

/// EXPORT: the active dataset as a portable file.
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    pub outfile: String,
    /// `/KEEP`, `/DROP`, `/RENAME` and `/MAP` in the order written; the active dataset stays.
    pub options: Vec<FileOption>,
    /// DIGITS: significant decimal digits of values that are not whole.
    pub digits: Option<u32>,
    /// UNSELECTED=DELETE
    pub delete_unselected: bool,
}

/// IMPORT: a portable file as the active dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub file: String,
    /// `/KEEP`, `/DROP`, `/RENAME` and `/MAP` in the order written.
    pub options: Vec<FileOption>,
}

/// GET TRANSLATE: a tab-delimited text file, a dBASE file or an old workbook as the active
/// dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct GetTranslate {
    pub file: String,
    pub source: TranslateSource,
    /// FIELDNAMES: the first row holds the names.
    pub field_names: bool,
    /// RANGE as (first row, first column, last row, last column), counted from 0.
    pub range: Option<(u32, u32, u32, u32)>,
    /// `/KEEP`, `/DROP`, `/RENAME` and `/MAP` in the order written.
    pub options: Vec<FileOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateSource {
    Tab,
    /// dBASE II, III and IV.
    Dbf,
    /// An Excel workbook that calamine reads (Excel 95 to 2003).
    Xls,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subset {
    Keep(Vec<VarItem>),
    Drop(Vec<VarItem>),
    /// `/MAP` of a merge: a table of the variables at this point and the files they come from.
    Map,
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
