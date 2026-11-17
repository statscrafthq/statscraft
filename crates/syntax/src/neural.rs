//! Parser of MLP (multilayer perceptron) and RBF (radial basis function network).
//!
//! Rules from the Command Syntax Reference (MLP, RBF), in our words:
//! - `dependent [(MLEVEL=S|O|N)] ... [BY factors] [WITH covariates]` comes first; the
//!   subcommands follow in any order, each at most once, none empty, no keyword twice.
//! - Keywords are written in full; parentheses and equals signs are required.
//! - RESCALE COVARIATE and DEPENDENT: STANDARDIZED, NORMALIZED, ADJNORMALIZED or NONE; for MLP
//!   the dependent's NORMALIZED and ADJNORMALIZED take (CORRECTION=number), 0.02 by default.
//! - PARTITION: TRAINING, TESTING and HOLDOUT shares (70, 30, 0), or VARIABLE=name.
//! - MLP ARCHITECTURE: AUTOMATIC=YES [(MINUNITS=1, MAXUNITS=50)] allows nothing else;
//!   AUTOMATIC=NO with HIDDENLAYERS=1|2 [(NUMUNITS=AUTO|n[,n])], HIDDENFUNCTION=TANH|SIGMOID
//!   and OUTPUTFUNCTION=IDENTITY|SIGMOID|SOFTMAX|TANH.
//! - RBF ARCHITECTURE: MINUNITS and MAXUNITS (AUTO or a whole number, both or neither), or
//!   NUMUNITS=n; HIDDENFUNCTION=NRBF|ORBF.
//! - MLP CRITERIA: TRAINING=BATCH|ONLINE|MINIBATCH, MINIBATCHSIZE, MEMSIZE, OPTIMIZATION,
//!   LEARNINGINITIAL, LEARNINGLOWER, MOMENTUM, LEARNINGEPOCHS, LAMBDAINITIAL, SIGMAINITIAL,
//!   INTERVALCENTER, INTERVALOFFSET. RBF CRITERIA: OVERLAP=AUTO|number.
//! - MLP STOPPINGRULES: ERRORSTEPS=n [(DATA=AUTO|BOTH)], TRAININGTIMER=ON|OFF
//!   [(MAXTIME=n)], MAXEPOCHS=AUTO|n, ERRORCHANGE, ERRORRATIO.
//! - PRINT and PLOT list keywords; NONE stands alone. SAVE PREDVAL[(names)] and
//!   PSEUDOPROB[(root[:n] ...)]. OUTFILE MODEL='file' ... (MLP may also write OUTFILE MODEL).

use crate::ast::{Command, Level, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rescale {
    Standardized,
    /// With the correction.
    Normalized(f64),
    AdjNormalized(f64),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Partition {
    /// Relative shares of training, testing and holdout.
    Shares(f64, f64, f64),
    Variable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Tanh,
    Sigmoid,
    Identity,
    Softmax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Training {
    Batch,
    Online,
    MiniBatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NeuralArchitecture {
    /// MLP: AUTOMATIC=YES. RBF: a range of units is searched (no NUMUNITS).
    pub automatic: bool,
    /// None for AUTO (RBF) or the default.
    pub min_units: Option<usize>,
    pub max_units: Option<usize>,
    /// MLP: 1 or 2.
    pub hidden_layers: usize,
    /// NUMUNITS; empty for AUTO.
    pub units: Vec<usize>,
    /// MLP: TANH or SIGMOID.
    pub hidden: Activation,
    /// MLP OUTPUTFUNCTION; None for the default.
    pub output: Option<Activation>,
    /// RBF HIDDENFUNCTION=ORBF.
    pub orbf: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeuralCriteria {
    pub training: Training,
    /// None for AUTO.
    pub minibatch: Option<usize>,
    pub memsize: usize,
    /// Scaled conjugate gradient; None for the default of the training type.
    pub scaled_conjugate: Option<bool>,
    pub learning_initial: f64,
    pub learning_lower: f64,
    pub momentum: f64,
    pub learning_epochs: usize,
    pub lambda_initial: f64,
    pub sigma_initial: f64,
    pub interval_center: f64,
    pub interval_offset: f64,
    /// RBF OVERLAP; None for AUTO.
    pub overlap: Option<f64>,
}

impl Default for NeuralCriteria {
    fn default() -> Self {
        Self {
            training: Training::Batch,
            minibatch: None,
            memsize: 1000,
            scaled_conjugate: None,
            learning_initial: 0.4,
            learning_lower: 0.001,
            momentum: 0.9,
            learning_epochs: 10,
            lambda_initial: 0.0000005,
            sigma_initial: 0.00005,
            interval_center: 0.0,
            interval_offset: 0.5,
            overlap: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoppingRules {
    pub error_steps: usize,
    /// DATA=BOTH
    pub both: bool,
    /// TRAININGTIMER=ON with MAXTIME minutes; None when OFF.
    pub max_time: Option<f64>,
    /// None for AUTO.
    pub max_epochs: Option<usize>,
    pub error_change: f64,
    pub error_ratio: f64,
}

impl Default for StoppingRules {
    fn default() -> Self {
        Self {
            error_steps: 1,
            both: false,
            max_time: Some(15.0),
            max_epochs: None,
            error_change: 0.0001,
            error_ratio: 0.001,
        }
    }
}

/// PRINT keywords in order: CPS, NETWORKINFO, SUMMARY, CLASSIFICATION, SOLUTION, IMPORTANCE.
pub const NEURAL_PRINT: [&str; 6] = [
    "CPS",
    "NETWORKINFO",
    "SUMMARY",
    "CLASSIFICATION",
    "SOLUTION",
    "IMPORTANCE",
];
/// PLOT keywords in order: NETWORK, PREDICTED, RESIDUAL, ROC, GAIN, LIFT.
pub const NEURAL_PLOT: [&str; 6] = ["NETWORK", "PREDICTED", "RESIDUAL", "ROC", "GAIN", "LIFT"];

#[derive(Debug, Clone, PartialEq)]
pub struct Neural {
    /// RBF; MLP otherwise.
    pub rbf: bool,
    pub dependents: Vec<(String, Option<Level>)>,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub except: Vec<VarItem>,
    pub rescale_covariates: Rescale,
    /// None for the default of the output function.
    pub rescale_dependents: Option<Rescale>,
    pub partition: Partition,
    pub architecture: NeuralArchitecture,
    pub criteria: NeuralCriteria,
    pub stopping: StoppingRules,
    /// USERMISSING=INCLUDE
    pub include: bool,
    /// By NEURAL_PRINT.
    pub print: [bool; 6],
    /// By NEURAL_PLOT.
    pub plot: [bool; 6],
    /// SAVE PREDVAL and its names.
    pub predval: Option<Vec<String>>,
    /// SAVE PSEUDOPROB: root names with their counts (25 by default).
    pub pseudoprob: Option<Vec<(String, usize)>>,
    pub outfile: Vec<String>,
    /// BY or WITH without variables.
    pub warnings: Vec<String>,
}

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message)
}

/// One of `words`, written in full.
fn word_of(p: &mut Parser, words: &[&str]) -> PResult<usize> {
    match words.iter().position(|w| p.eat_word(w)) {
        Some(i) => Ok(i),
        None => Err(p.unexpected(words)),
    }
}

/// `= number`, checked.
fn number(p: &mut Parser, ok: impl Fn(f64) -> bool, what: &str) -> PResult<f64> {
    p.expect(Punct::Eq)?;
    let start = p.pos;
    match p.number() {
        Some(v) if ok(v) => Ok(v),
        Some(v) => Err(error(p, start, format!("{what}, got {v}"))),
        None => Err(p.unexpected(&["a number"])),
    }
}

fn whole(v: f64) -> bool {
    v.fract() == 0.0 && v >= 1.0
}

/// `= AUTO` (None) or a whole number of at least `min`.
fn auto_or(p: &mut Parser, min: f64, what: &str) -> PResult<Option<usize>> {
    if p.at(Punct::Eq)
        && matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("AUTO"))
    {
        p.pos += 2;
        return Ok(None);
    }
    number(p, |v| whole(v) && v >= min, what).map(|v| Some(v as usize))
}

/// Tracks the keywords of one subcommand, so that none comes twice.
struct Seen(Vec<&'static str>);

impl Seen {
    fn next(&mut self, p: &mut Parser, words: &[&'static str]) -> PResult<&'static str> {
        let start = p.pos;
        let Some(&w) = words.iter().find(|w| p.eat_word(w)) else {
            return Err(p.unexpected(words));
        };
        if self.0.contains(&w) {
            return Err(error(p, start, format!("keyword {w} is given twice")));
        }
        self.0.push(w);
        Ok(w)
    }
}

fn rescale(p: &mut Parser, correction: bool) -> PResult<Rescale> {
    p.expect(Punct::Eq)?;
    let kind = word_of(p, &["STANDARDIZED", "NORMALIZED", "ADJNORMALIZED", "NONE"])?;
    let mut c = if correction { 0.02 } else { 0.0 };
    if correction && (kind == 1 || kind == 2) && p.eat(Punct::LParen) {
        if !p.eat_word("CORRECTION") {
            return Err(p.unexpected(&["CORRECTION"]));
        }
        c = number(p, |v| v >= 0.0, "CORRECTION is a number of 0 or more")?;
        p.expect(Punct::RParen)?;
    }
    Ok(match kind {
        0 => Rescale::Standardized,
        1 => Rescale::Normalized(c),
        2 => Rescale::AdjNormalized(c),
        _ => Rescale::None,
    })
}

fn mlp_architecture(p: &mut Parser, a: &mut NeuralArchitecture) -> PResult<()> {
    let mut seen = Seen(Vec::new());
    let words = [
        "AUTOMATIC",
        "HIDDENLAYERS",
        "HIDDENFUNCTION",
        "OUTPUTFUNCTION",
    ];
    let mut others = None;
    while !at_end(p) {
        let start = p.pos;
        match seen.next(p, &words)? {
            "AUTOMATIC" => {
                p.expect(Punct::Eq)?;
                a.automatic = word_of(p, &["YES", "NO"])? == 0;
                if a.automatic && p.eat(Punct::LParen) {
                    let mut range = Seen(Vec::new());
                    for _ in 0..2 {
                        let w = range.next(p, &["MINUNITS", "MAXUNITS"])?;
                        let n = number(p, whole, "the number of units is a positive whole number")?;
                        match w {
                            "MINUNITS" => a.min_units = Some(n as usize),
                            _ => a.max_units = Some(n as usize),
                        }
                        p.eat(Punct::Comma);
                    }
                    p.expect(Punct::RParen)?;
                    if a.min_units >= a.max_units {
                        return Err(error(p, start, "MINUNITS must be less than MAXUNITS"));
                    }
                }
            }
            "HIDDENLAYERS" => {
                others.get_or_insert(start);
                a.hidden_layers =
                    number(p, |v| v == 1.0 || v == 2.0, "HIDDENLAYERS is 1 or 2")? as usize;
                if p.eat(Punct::LParen) {
                    if !p.eat_word("NUMUNITS") {
                        return Err(p.unexpected(&["NUMUNITS"]));
                    }
                    p.expect(Punct::Eq)?;
                    a.units.clear();
                    if !p.eat_word("AUTO") {
                        for i in 0..a.hidden_layers {
                            if i > 0 {
                                p.eat(Punct::Comma);
                            }
                            let at = p.pos;
                            match p.number() {
                                Some(v) if whole(v) => a.units.push(v as usize),
                                _ => {
                                    return Err(error(
                                        p,
                                        at,
                                        "NUMUNITS takes AUTO or one positive whole number per hidden layer",
                                    ));
                                }
                            }
                        }
                    }
                    p.expect(Punct::RParen)?;
                }
            }
            "HIDDENFUNCTION" => {
                others.get_or_insert(start);
                p.expect(Punct::Eq)?;
                a.hidden =
                    [Activation::Tanh, Activation::Sigmoid][word_of(p, &["TANH", "SIGMOID"])?];
            }
            _ => {
                others.get_or_insert(start);
                p.expect(Punct::Eq)?;
                let all = [
                    Activation::Identity,
                    Activation::Sigmoid,
                    Activation::Softmax,
                    Activation::Tanh,
                ];
                a.output = Some(all[word_of(p, &["IDENTITY", "SIGMOID", "SOFTMAX", "TANH"])?]);
            }
        }
    }
    if let (true, Some(start)) = (a.automatic, others) {
        return Err(error(
            p,
            start,
            "with AUTOMATIC=YES no other ARCHITECTURE keyword is allowed; give AUTOMATIC=NO",
        ));
    }
    Ok(())
}

fn rbf_architecture(p: &mut Parser, a: &mut NeuralArchitecture) -> PResult<()> {
    let mut seen = Seen(Vec::new());
    let start = p.pos;
    while !at_end(p) {
        match seen.next(p, &["MINUNITS", "MAXUNITS", "NUMUNITS", "HIDDENFUNCTION"])? {
            "MINUNITS" => {
                a.min_units = auto_or(p, 1.0, "MINUNITS is AUTO or a positive whole number")?
            }
            "MAXUNITS" => {
                a.max_units = auto_or(p, 1.0, "MAXUNITS is AUTO or a positive whole number")?
            }
            "NUMUNITS" => {
                let n = number(p, whole, "NUMUNITS is a positive whole number")?;
                a.units = vec![n as usize];
                a.automatic = false;
            }
            _ => {
                p.expect(Punct::Eq)?;
                a.orbf = word_of(p, &["NRBF", "ORBF"])? == 1;
            }
        }
    }
    let range = seen
        .0
        .iter()
        .filter(|w| w.ends_with("UNITS") && **w != "NUMUNITS")
        .count();
    if range > 0 && !a.automatic {
        return Err(error(
            p,
            start,
            "NUMUNITS cannot be given with MINUNITS or MAXUNITS",
        ));
    }
    if range == 1 {
        return Err(error(p, start, "MINUNITS and MAXUNITS are given together"));
    }
    if let (Some(lo), Some(hi)) = (a.min_units, a.max_units)
        && lo >= hi
    {
        return Err(error(p, start, "MINUNITS must be less than MAXUNITS"));
    }
    Ok(())
}

fn criteria(p: &mut Parser, c: &mut NeuralCriteria, rbf: bool) -> PResult<()> {
    let mut seen = Seen(Vec::new());
    let words: &[&'static str] = if rbf {
        &["OVERLAP"]
    } else {
        &[
            "TRAINING",
            "MINIBATCHSIZE",
            "MEMSIZE",
            "OPTIMIZATION",
            "LEARNINGINITIAL",
            "LEARNINGLOWER",
            "MOMENTUM",
            "LEARNINGEPOCHS",
            "LAMBDAINITIAL",
            "SIGMAINITIAL",
            "INTERVALCENTER",
            "INTERVALOFFSET",
        ]
    };
    let start = p.pos;
    let positive = |v: f64| v > 0.0;
    while !at_end(p) {
        match seen.next(p, words)? {
            "OVERLAP" => {
                c.overlap = if p.at(Punct::Eq)
                    && matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("AUTO"))
                {
                    p.pos += 2;
                    None
                } else {
                    Some(number(p, positive, "OVERLAP is AUTO or a number above 0")?)
                }
            }
            "TRAINING" => {
                p.expect(Punct::Eq)?;
                let all = [Training::Batch, Training::Online, Training::MiniBatch];
                c.training = all[word_of(p, &["BATCH", "ONLINE", "MINIBATCH"])?];
            }
            "MINIBATCHSIZE" => {
                c.minibatch = auto_or(
                    p,
                    2.0,
                    "MINIBATCHSIZE is AUTO or a whole number of 2 or more",
                )?
            }
            "MEMSIZE" => {
                c.memsize = number(
                    p,
                    |v| whole(v) && v >= 2.0,
                    "MEMSIZE is a whole number of 2 or more",
                )? as usize
            }
            "OPTIMIZATION" => {
                p.expect(Punct::Eq)?;
                c.scaled_conjugate =
                    Some(word_of(p, &["GRADIENTDESCENT", "SCALEDCONJUGATE"])? == 1);
            }
            "LEARNINGINITIAL" => {
                c.learning_initial = number(p, positive, "LEARNINGINITIAL is above 0")?
            }
            "LEARNINGLOWER" => c.learning_lower = number(p, positive, "LEARNINGLOWER is above 0")?,
            "MOMENTUM" => c.momentum = number(p, positive, "MOMENTUM is above 0")?,
            "LEARNINGEPOCHS" => {
                c.learning_epochs =
                    number(p, whole, "LEARNINGEPOCHS is a positive whole number")? as usize
            }
            "LAMBDAINITIAL" => {
                c.lambda_initial = number(
                    p,
                    |v| v > 0.0 && v < 1e-6,
                    "LAMBDAINITIAL is above 0 and below 0.000001",
                )?
            }
            "SIGMAINITIAL" => {
                c.sigma_initial = number(
                    p,
                    |v| v > 0.0 && v < 1e-4,
                    "SIGMAINITIAL is above 0 and below 0.0001",
                )?
            }
            "INTERVALCENTER" => c.interval_center = number(p, |_| true, "")?,
            _ => c.interval_offset = number(p, positive, "INTERVALOFFSET is above 0")?,
        }
    }
    if c.learning_lower >= c.learning_initial {
        return Err(error(
            p,
            start,
            "LEARNINGLOWER must be less than LEARNINGINITIAL",
        ));
    }
    if c.training != Training::Batch && c.scaled_conjugate == Some(true) {
        return Err(error(
            p,
            start,
            "OPTIMIZATION=SCALEDCONJUGATE needs TRAINING=BATCH; online and mini-batch training use GRADIENTDESCENT",
        ));
    }
    if c.minibatch.is_some_and(|m| m > c.memsize) {
        return Err(error(p, start, "MINIBATCHSIZE cannot exceed MEMSIZE"));
    }
    Ok(())
}

fn stopping(p: &mut Parser, s: &mut StoppingRules) -> PResult<()> {
    let mut seen = Seen(Vec::new());
    let words = [
        "ERRORSTEPS",
        "TRAININGTIMER",
        "MAXEPOCHS",
        "ERRORCHANGE",
        "ERRORRATIO",
    ];
    while !at_end(p) {
        match seen.next(p, &words)? {
            "ERRORSTEPS" => {
                s.error_steps = number(p, whole, "ERRORSTEPS is a positive whole number")? as usize;
                if p.eat(Punct::LParen) {
                    if !p.eat_word("DATA") {
                        return Err(p.unexpected(&["DATA"]));
                    }
                    p.expect(Punct::Eq)?;
                    s.both = word_of(p, &["AUTO", "BOTH"])? == 1;
                    p.expect(Punct::RParen)?;
                }
            }
            "TRAININGTIMER" => {
                p.expect(Punct::Eq)?;
                let on = word_of(p, &["ON", "OFF"])? == 0;
                let mut minutes = 15.0;
                if p.eat(Punct::LParen) {
                    if !p.eat_word("MAXTIME") {
                        return Err(p.unexpected(&["MAXTIME"]));
                    }
                    minutes = number(p, |v| v > 0.0, "MAXTIME is above 0")?;
                    p.expect(Punct::RParen)?;
                }
                s.max_time = on.then_some(minutes);
            }
            "MAXEPOCHS" => {
                s.max_epochs = auto_or(p, 1.0, "MAXEPOCHS is AUTO or a positive whole number")?
            }
            "ERRORCHANGE" => s.error_change = number(p, |v| v > 0.0, "ERRORCHANGE is above 0")?,
            _ => s.error_ratio = number(p, |v| v > 0.0, "ERRORRATIO is above 0")?,
        }
    }
    Ok(())
}

/// PRINT or PLOT: the keywords asked for; NONE alone.
fn flags(p: &mut Parser, words: &[&'static str; 6]) -> PResult<[bool; 6]> {
    let mut out = [false; 6];
    let mut all: Vec<&'static str> = words.to_vec();
    all.push("NONE");
    let mut seen = Seen(Vec::new());
    let start = p.pos;
    while !at_end(p) {
        let w = seen.next(p, &all)?;
        if let Some(i) = words.iter().position(|k| *k == w) {
            out[i] = true;
        }
    }
    if seen.0.contains(&"NONE") && seen.0.len() > 1 {
        return Err(error(p, start, "NONE cannot be given with other keywords"));
    }
    Ok(out)
}

fn save(p: &mut Parser, command: &mut Neural) -> PResult<()> {
    let mut seen = Seen(Vec::new());
    while !at_end(p) {
        let w = seen.next(p, &["PREDVAL", "PSEUDOPROB"])?;
        let mut names = Vec::new();
        if p.eat(Punct::LParen) {
            while !p.eat(Punct::RParen) {
                let name = p.name("a variable name")?;
                let mut count = 25;
                if w == "PSEUDOPROB" && p.eat(Punct::Colon) {
                    let at = p.pos;
                    count = match p.number() {
                        Some(v) if whole(v) => v as usize,
                        _ => {
                            return Err(error(
                                p,
                                at,
                                "the count after ':' is a positive whole number",
                            ));
                        }
                    };
                }
                if names
                    .iter()
                    .any(|(n, _): &(String, usize)| n.eq_ignore_ascii_case(&name))
                {
                    return Err(error(p, p.pos - 1, format!("{name} is given twice")));
                }
                names.push((name, count));
                p.eat(Punct::Comma);
            }
        }
        if w == "PREDVAL" {
            command.predval = Some(names.into_iter().map(|n| n.0).collect());
        } else {
            command.pseudoprob = Some(names);
        }
    }
    Ok(())
}

fn dependents(p: &mut Parser) -> PResult<Vec<(String, Option<Level>)>> {
    let mut out: Vec<(String, Option<Level>)> = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(w))
        if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    {
        let dependent = p.name("a dependent variable")?;
        let mut level = None;
        if p.eat(Punct::LParen) {
            if !p.eat_word("MLEVEL") {
                return Err(p.unexpected(&["MLEVEL"]));
            }
            p.expect(Punct::Eq)?;
            level =
                Some([Level::Scale, Level::Ordinal, Level::Nominal][word_of(p, &["S", "O", "N"])?]);
            p.expect(Punct::RParen)?;
        }
        out.push((dependent, level));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a dependent variable"]));
    }
    Ok(out)
}

fn neural(p: &mut Parser, rbf: bool) -> PResult<Command> {
    let name = if rbf { "RBF" } else { "MLP" };
    let mut command = Neural {
        rbf,
        dependents: dependents(p)?,
        factors: Vec::new(),
        covariates: Vec::new(),
        except: Vec::new(),
        rescale_covariates: Rescale::Standardized,
        rescale_dependents: None,
        partition: Partition::Shares(70.0, 30.0, 0.0),
        architecture: NeuralArchitecture {
            automatic: true,
            min_units: None,
            max_units: None,
            hidden_layers: 1,
            units: Vec::new(),
            hidden: Activation::Tanh,
            output: None,
            orbf: false,
        },
        criteria: NeuralCriteria::default(),
        stopping: StoppingRules::default(),
        include: false,
        print: [true, true, true, true, false, false],
        plot: [true, false, false, false, false, false],
        predval: None,
        pseudoprob: None,
        outfile: Vec::new(),
        warnings: Vec::new(),
    };
    for (word, list) in [("BY", 0), ("WITH", 1)] {
        if p.eat_word(word) {
            if matches!(p.peek(), Some(TokenKind::Word(w)) if !(list == 0 && w.eq_ignore_ascii_case("WITH")))
            {
                let items = match list {
                    0 => list_until(p, &["WITH"])?,
                    _ => p.var_list()?,
                };
                if list == 0 {
                    command.factors = items;
                } else {
                    command.covariates = items;
                }
            } else {
                command
                    .warnings
                    .push(format!("{name}: {word} has no variables and is ignored"));
            }
        }
    }
    let subcommands = [
        "EXCEPT",
        "RESCALE",
        "PARTITION",
        "ARCHITECTURE",
        "CRITERIA",
        "STOPPINGRULES",
        "MISSING",
        "PRINT",
        "PLOT",
        "SAVE",
        "OUTFILE",
    ];
    let mut seen = Seen(Vec::new());
    while p.eat(Punct::Slash) {
        let at = p.pos;
        let Some(&sub) = subcommands.iter().find(|s| p.eat_word(s)) else {
            return Err(p.subcommand_error(name, &[]));
        };
        if seen.0.contains(&sub) || (sub == "STOPPINGRULES" && rbf) {
            let message = match rbf && sub == "STOPPINGRULES" {
                true => format!("unknown subcommand STOPPINGRULES of {name}"),
                false => format!("subcommand {sub} is given twice"),
            };
            return Err(error(p, at, message));
        }
        seen.0.push(sub);
        let start = p.pos;
        if at_end(p) {
            return Err(error(p, start, format!("subcommand {sub} is empty")));
        }
        match sub {
            "EXCEPT" => {
                if !p.eat_word("VARIABLES") {
                    return Err(p.unexpected(&["VARIABLES"]));
                }
                p.expect(Punct::Eq)?;
                command.except = p.var_list()?;
            }
            "RESCALE" => {
                let mut kw = Seen(Vec::new());
                while !at_end(p) {
                    if kw.next(p, &["COVARIATE", "DEPENDENT"])? == "COVARIATE" {
                        command.rescale_covariates = rescale(p, false)?;
                    } else {
                        command.rescale_dependents = Some(rescale(p, !rbf)?);
                    }
                }
            }
            "PARTITION" => {
                if p.eat_word("VARIABLE") {
                    p.expect(Punct::Eq)?;
                    command.partition = Partition::Variable(p.name("a variable name")?);
                } else {
                    let mut kw = Seen(Vec::new());
                    let mut shares = [0.0; 3];
                    while !at_end(p) {
                        let w = kw.next(p, &["TRAINING", "TESTING", "HOLDOUT"])?;
                        let i = ["TRAINING", "TESTING", "HOLDOUT"]
                            .iter()
                            .position(|k| *k == w);
                        shares[i.expect("listed")] = number(
                            p,
                            |v| v.fract() == 0.0 && v >= 0.0,
                            "the share is a whole number of 0 or more",
                        )?;
                    }
                    if shares[0] <= 0.0 {
                        return Err(error(p, start, "PARTITION TRAINING must be above 0"));
                    }
                    command.partition = Partition::Shares(shares[0], shares[1], shares[2]);
                }
            }
            "ARCHITECTURE" if rbf => rbf_architecture(p, &mut command.architecture)?,
            "ARCHITECTURE" => mlp_architecture(p, &mut command.architecture)?,
            "CRITERIA" => criteria(p, &mut command.criteria, rbf)?,
            "STOPPINGRULES" => stopping(p, &mut command.stopping)?,
            "MISSING" => {
                if !p.eat_word("USERMISSING") {
                    return Err(p.unexpected(&["USERMISSING"]));
                }
                p.expect(Punct::Eq)?;
                command.include = word_of(p, &["EXCLUDE", "INCLUDE"])? == 1;
            }
            "PRINT" => command.print = flags(p, &NEURAL_PRINT)?,
            "PLOT" => command.plot = flags(p, &NEURAL_PLOT)?,
            "SAVE" => save(p, &mut command)?,
            _ => {
                if !p.eat_word("MODEL") {
                    return Err(p.unexpected(&["MODEL"]));
                }
                p.expect(Punct::Eq)?;
                while matches!(p.peek(), Some(TokenKind::Str(_))) {
                    command.outfile.push(p.file("a file name")?);
                    p.eat(Punct::Comma);
                }
                if command.outfile.is_empty() {
                    return Err(p.unexpected(&["a quoted file name"]));
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !command.outfile.is_empty() && command.outfile.len() < command.dependents.len() {
        return Err(SyntaxError::new(
            p.span(),
            "OUTFILE MODEL needs one file per dependent variable",
        ));
    }
    Ok(Command::Neural(Box::new(command)))
}

pub(crate) fn mlp(p: &mut Parser) -> PResult<Command> {
    neural(p, false)
}

pub(crate) fn rbf(p: &mut Parser) -> PResult<Command> {
    neural(p, true)
}
