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
