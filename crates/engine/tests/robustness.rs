//! No input may crash Statscraft. Syntax taken from every command's help diagram and example,
//! the docs and the test corpus is mutated (tokens dropped, repeated, swapped or cut, wrong
//! types, huge numbers, empty lists, unbalanced brackets and quotes) and run like the command
//! line does: `check` without a dataset and `run` on survey_basic.sav. Each run must end in a
//! result or an error within the time limit, never in a panic.
//!
//! The default tests run a small fixed set in a few seconds. The full set is ignored (Unix only):
//! `cargo test -p statscraft-engine --test robustness -- --ignored --nocapture`, with
//! FUZZ_VARIANTS (mutants per seed, default 12), FUZZ_SHARD=i/n, FUZZ_LIMIT (seconds per run,
//! default 60), FUZZ_MEMORY_MIB (default 1500) and FUZZ_TRACE=file (the input that runs now).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use statscraft_data::Dataset;
use statscraft_engine::session::Session;

/// xorshift64: a fixed sequence, so every run checks the same inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const NUMERIC: &[&str] = &[
    "gender",
    "age",
    "region",
    "q1_satisfaction",
    "q2_recommend",
    "monthly_spend",
    "signup_date",
    "q5_1",
    "q5_2",
    "q6_mention1",
    "weight",
];
const STRINGS: &[&str] = &["plan", "q3_comment"];
const ODD_NUMBERS: &[&str] = &[
    "0",
    "-1",
    "1e308",
    "-1e308",
    "1e-320",
    "99999999999999999999",
    "2147483648",
    "4294967297",
    "18446744073709551616",
    "1e999",
    ".",
    "-0",
    "0.5",
    "123456789",
];
const BRACKETS: &[&str] = &[
    "(", ")", "{", "}", "[", "]", "'", "\"", "/", "=", ".", ",", ";",
];

// ---------------------------------------------------------------------------------------------
// Seeds

/// One instance of a help diagram: optional parts kept or left out, one of `a | b` chosen,
/// placeholders replaced by variables, numbers and paths.
fn instantiate(syntax: &str, rng: &mut Rng) -> String {
    // the diagram ends with the first line that ends in a period; notes follow
    let mut diagram = String::new();
    for line in syntax.lines() {
        diagram.push_str(line);
        diagram.push(' ');
        if line.trim_end().ends_with('.') {
            break;
        }
    }
    let chars: Vec<char> = diagram.chars().collect();
    let mut i = 0;
    let text = expand(&chars, &mut i, rng, None);
    let words: Vec<String> = text
        .split(' ')
        .map(|w| placeholder(w, rng).unwrap_or_else(|| w.to_string()))
        .collect();
    words.join(" ")
}

fn expand(t: &[char], i: &mut usize, rng: &mut Rng, end: Option<char>) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut alternative = false;
    while *i < t.len() {
        let c = t[*i];
        if Some(c) == end {
            *i += 1;
            break;
        }
        let piece = if c == '[' {
            *i += 1;
            let inner = expand(t, i, rng, Some(']'));
            if rng.below(2) == 0 {
                inner
            } else {
                String::new()
            }
        } else if c == '|' {
            *i += 1;
            alternative = true;
            continue;
        } else if c.is_whitespace() {
            *i += 1;
            out.push(' ');
            continue;
        } else {
            let start = *i;
            while *i < t.len() && !t[*i].is_whitespace() && !matches!(t[*i], '[' | ']' | '|') {
                *i += 1;
            }
            t[start..*i].iter().collect()
        };
        if alternative {
            alternative = false;
            if rng.below(2) == 0 {
                out.truncate(last);
                out.push_str(&piece);
            }
            continue;
        }
        last = out.len();
        out.push_str(&piece);
    }
    out
}

/// A value for a lower-case word of a diagram, which names what goes there.
fn placeholder(word: &str, rng: &mut Rng) -> Option<String> {
    let core = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'');
    if core.is_empty() || !core.chars().any(|c| c.is_ascii_lowercase()) {
        return None;
    }
    if core.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let value = if core.contains("path") || core.contains("file") {
        "'fuzz_out.txt'".to_string()
    } else if core.contains("list") || core.ends_with("vars") || core.contains("factors") {
        format!("{} {}", rng.pick(NUMERIC), rng.pick(NUMERIC))
    } else if core.contains("var") || core == "dependent" {
        rng.pick(NUMERIC).to_string()
    } else if core.contains("string") || core.contains("label") || core.contains("text") {
        "'text'".to_string()
    } else if core.contains("name") {
        "newvar".to_string()
    } else if core.contains("expr") || core.contains("condition") {
        format!("{} > 1", rng.pick(NUMERIC))
    } else if core.len() <= 2 || core.contains("value") || core.contains("num") {
        rng.pick(&["1", "2", "3", "0.5", "10"]).to_string()
    } else {
        return None;
    };
    Some(word.replace(core, &value))
}

/// The string literals of a Rust source that look like syntax: a word, a period at the end.
fn literals(source: &str) -> Vec<String> {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '/' && bytes.get(i + 1) == Some(&'/') {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\'' {
            // a char literal or a lifetime: skip the quote and the next character
            i += 2;
            continue;
        }
        if c == 'r' && matches!(bytes.get(i + 1), Some('"' | '#')) {
            let mut hashes = 0;
            let mut j = i + 1;
            while bytes.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if bytes.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            let start = j + 1;
            let mut k = start;
            while k < bytes.len()
                && !(bytes[k] == '"' && (1..=hashes).all(|h| bytes.get(k + h) == Some(&'#')))
            {
                k += 1;
            }
            out.push(bytes[start..k.min(bytes.len())].iter().collect());
            i = k + 1 + hashes;
            continue;
        }
        if c == '"' {
            let mut text = String::new();
            let mut k = i + 1;
            while k < bytes.len() && bytes[k] != '"' {
                if bytes[k] == '\\' && k + 1 < bytes.len() {
                    k += 1;
                    match bytes[k] {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        '\n' => {
                            // a continued line: the leading blanks of the next go too
                            while k + 1 < bytes.len() && bytes[k + 1].is_whitespace() {
                                k += 1;
                            }
                        }
                        other => text.push(other),
                    }
                } else {
                    text.push(bytes[k]);
                }
                k += 1;
            }
            out.push(text);
            i = k + 1;
            continue;
        }
        i += 1;
    }
    out.retain(|s| {
        let s = s.trim();
        s.len() >= 6
            && s.ends_with('.')
            && s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && !s.contains("{}")
    });
    out
}

/// The fenced code blocks of a Markdown file that hold syntax (end with a period).
fn code_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut block: Option<String> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match block.take() {
                Some(b) => {
                    if b.trim_end().ends_with('.') && !b.contains("statscraft ") {
                        out.push(b);
                    }
                }
                None => block = Some(String::new()),
            }
        } else if let Some(b) = &mut block {
            b.push_str(line);
            b.push('\n');
        }
    }
    out
}

/// Hand-made inputs aimed at limits: recursion, nesting, size.
fn extremes() -> Vec<String> {
    let deep =
        |open: &str, close: &str, n: usize| format!("{}1{}", open.repeat(n), close.repeat(n));
    let mut out = vec![
        "DEFINE !m() !m !ENDDEFINE.\n!m.".to_string(),
        "DEFINE !m() !m !m !ENDDEFINE.\n!m.".to_string(),
        "DEFINE !m() !m !m !m !m !ENDDEFINE.\n!m.".to_string(),
        "SET MNEST=100000.\nDEFINE !m() !m !ENDDEFINE.\n!m.".to_string(),
        "SET MNEST=100000.\nDEFINE !m() !m !m !ENDDEFINE.\n!m.".to_string(),
        "SET MITERATE=2000000000.\nDEFINE !m() !DO !i=1 !TO 2000000000 x !DOEND !ENDDEFINE.\n!m."
            .to_string(),
        "DEFINE !a() !b !ENDDEFINE.\nDEFINE !b() !a !ENDDEFINE.\n!a.".to_string(),
        "DEFINE !m(!POS !TOKENS(1)) !m !1 !ENDDEFINE.\n!m x.".to_string(),
        "DEFINE !m(!POS !CMDEND) COMPUTE x = !1. !m !1 !ENDDEFINE.\n!m 1.".to_string(),
        "DEFINE !m() !LET !x = !CONCAT(!x, !x, !x, !x) !m !ENDDEFINE.\n!m.".to_string(),
        "DEFINE !m() FREQUENCIES age. !m !ENDDEFINE.\n!m.".to_string(),
        "DEFINE !m(".to_string(),
        "DEFINE !m() !ENDDEFINE".to_string(),
        "SET MNEST=0.\nSET MITERATE=-1.\nDEFINE !m() x !ENDDEFINE.\n!m.".to_string(),
        format!("COMPUTE x = {}.", deep("(", ")", 5000)),
        format!("COMPUTE x = {}.", deep("-", "", 5000)),
        format!("COMPUTE x = {}.", deep("NOT ", "", 5000)),
        format!("COMPUTE x = {}1.", "1 + ".repeat(20000)),
        format!("COMPUTE x = {}.", deep("ABS(", ")", 3000)),
        format!("SELECT IF {}.", deep("(", ")", 5000)),
        format!("MATRIX.\nCOMPUTE a = {}.\nEND MATRIX.", deep("(", ")", 5000)),
        format!("MATRIX.\nCOMPUTE a = {}.\nEND MATRIX.", deep("{", "}", 5000)),
        format!("MATRIX.\nCOMPUTE a = {}.\nEND MATRIX.", deep("-", "", 5000)),
        format!("{}COMPUTE x = 1.\n{}", "DO IF age > 1.\n".repeat(3000), "END IF.\n".repeat(3000)),
        format!(
            "{}COMPUTE x = 1.\n{}",
            "LOOP #i = 1 TO 2.\n".repeat(500),
            "END LOOP.\n".repeat(500)
        ),
        format!(
            "{}COMPUTE x = 1.\n{}",
            "DO REPEAT a = 1 2.\n".repeat(200),
            "END REPEAT.\n".repeat(200)
        ),
        format!("CTABLES /TABLE {}.", vec!["age"; 2000].join(" > ")),
        format!("CTABLES /TABLE {}.", vec!["gender"; 2000].join(" BY ")),
        format!("CTABLES /TABLE {}.", deep("(", ")", 3000)),
        format!("GLM age BY gender /DESIGN={}.", deep("gender(", ")", 2000)),
        format!("COMPUTE x = '{}'.", "a".repeat(1_000_000)),
        format!("STRING s (A{}).", u32::MAX),
        "STRING s (A32767).\nCOMPUTE s = 'x'.\nEXECUTE.".to_string(),
        "STRING s (A99999999999).".to_string(),
        "NUMERIC x (F999999999.999999).".to_string(),
        "INPUT PROGRAM.\nCOMPUTE x = 1.\nEND INPUT PROGRAM.".to_string(),
        "INPUT PROGRAM.\nCOMPUTE x = 1.\nEND CASE.\nEND INPUT PROGRAM.".to_string(),
        "VECTOR v(2000000000).".to_string(),
        "VECTOR v(99999999999999).".to_string(),
        "VECTOR v(100000).\nEXECUTE.".to_string(),
        "NUMERIC v1 TO v999999999.".to_string(),
        "DATA LIST FREE /v1 TO v999999999.".to_string(),
        "DATA LIST /x 1-2000000000.".to_string(),
        "DATA LIST /x 2000000000-1.".to_string(),
        "DATA LIST RECORDS=2000000000 /x 1.\nBEGIN DATA\n1\nEND DATA.".to_string(),
        "DATA LIST /x 1 (F2147483647.0).\nBEGIN DATA\n1\nEND DATA.".to_string(),
        "DATA LIST LIST /x (A2147483647).\nBEGIN DATA\na\nEND DATA.".to_string(),
        "DATA LIST FREE /x.\nBEGIN DATA\n1e999 -1e999 1e-400 nan inf . , ,\nEND DATA.\nDESCRIPTIVES x.".to_string(),
        "MATRIX.\nCOMPUTE a = MAKE(100000, 100000, 1).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = IDENT(2000000000).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1:2000000000}.\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1:1e300}.\nEND MATRIX.".to_string(),
        "MATRIX.\nLOOP i = 1 TO 1e300.\nEND LOOP.\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = UNIFORM(1e9, 1e9).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = RESHAPE({1,2}, 2000000000, 2000000000).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1,2;3,4}.\nCOMPUTE b = a(1e300, 1).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1,2;3,4}.\nCOMPUTE b = a(-1, 0).\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1,2;3,4} ** 1e300.\nEND MATRIX.".to_string(),
        "MATRIX.\nCOMPUTE a = {1,2;3,4} ** -2000000000.\nEND MATRIX.".to_string(),
        "MATRIX DATA VARIABLES=ROWTYPE_ v1 TO v999999.\nBEGIN DATA\nN 1\nEND DATA.".to_string(),
        "FREQUENCIES age /NTILES=2000000000.".to_string(),
        "FREQUENCIES age /PERCENTILES=1e300 -5 200.".to_string(),
        "FREQUENCIES age /FORMAT=LIMIT(-1).".to_string(),
        "FREQUENCIES age /GROUPED=age (0).".to_string(),
        "FREQUENCIES age /GROUPED=age (-1e308).".to_string(),
        "FREQUENCIES age /GROUPED=age (1e-320).".to_string(),
        "RANK age /NTILES(2000000000).".to_string(),
        "RANK age /NTILES(0).".to_string(),
        "SAMPLE 2000000000 FROM 2000000001.".to_string(),
        "SAMPLE 1e300.".to_string(),
        "N OF CASES 1e300.".to_string(),
        "SET SEED=1e300.\nCOMPUTE x = RV.NORMAL(0, 1).\nEXECUTE.".to_string(),
        "COMPUTE x = LAG(age, 2000000000).\nEXECUTE.".to_string(),
        "COMPUTE x = RV.BINOM(2000000000, 0.5).\nEXECUTE.".to_string(),
        "COMPUTE x = RV.POISSON(1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = RV.HYPER(2000000000, 1000000000, 1000000000).\nEXECUTE.".to_string(),
        "COMPUTE x = RV.NEGBIN(2000000000, 1e-300).\nEXECUTE.".to_string(),
        "COMPUTE x = RV.GEOM(1e-300).\nEXECUTE.".to_string(),
        "COMPUTE x = CDF.BINOM(1e9, 1e9, 0.5).\nEXECUTE.".to_string(),
        "COMPUTE x = CDF.POISSON(1e300, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = CDF.HYPER(1e9, 2e9, 1e9, 1e9).\nEXECUTE.".to_string(),
        "COMPUTE x = PDF.BINOM(1e15, 1e15, 0.5).\nEXECUTE.".to_string(),
        "COMPUTE x = IDF.T(0.5, 1e-300).\nEXECUTE.".to_string(),
        "COMPUTE x = IDF.F(0.5, 1e300, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = NCDF.CHISQ(1e300, 1e300, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = NCDF.T(1, 1, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = NCDF.F(1, 1e300, 1, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = NCDF.BETA(0.5, 1e300, 1, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = CDF.SRANGE(1e300, 1e300, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = IDF.SRANGE(0.5, 1e300, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = LGAMMA(1e-320) + EXP(1e300) + LN(-1) + 1/0 + MOD(1, 0) + 1e308 * 10.\nEXECUTE.".to_string(),
        "STRING s (A20).\nCOMPUTE s = SUBSTR('abc', 2000000000, 2000000000).\nCOMPUTE s = RPAD('a', 2000000000).\nCOMPUTE s = LPAD('a', -1, 'x').\nCOMPUTE s = REPLACE('aaa', '', 'b').\nCOMPUTE s = STRING(1e308, F40.20).\nCOMPUTE s = CHAR.SUBSTR('õäö', 0, 1e300).\nEXECUTE.".to_string(),
        "COMPUTE x = DATE.DMY(1e300, 1e300, 1e300) + DATE.YRDAY(2000000000, 1) + TIME.DAYS(1e300) + XDATE.YEAR(-1e308).\nEXECUTE.".to_string(),
        "COMPUTE x = DATEDIFF(1e300, -1e300, 'days') + DATESUM(1e300, 1e300, 'months').\nEXECUTE.".to_string(),
        "FORMATS age (DATETIME40.39) monthly_spend (E40.39) gender (F40.39).\nLIST.".to_string(),
        "FORMATS age (F1.0).\nLIST.".to_string(),
        "WRITE FORMATS age (A9999).".to_string(),
        "PRINT FORMATS age (DOLLAR40.16).\nCOMPUTE age = 1e308.\nLIST.".to_string(),
        "SET DECIMAL=COMMA.\nSET CCA='-,$,,-'.\nFORMATS age (CCA40.16).\nCOMPUTE age = -1e300.\nLIST.".to_string(),
        "SPLIT FILE BY resp_id.\nCTABLES /TABLE age [MEAN] BY gender.".to_string(),
        "SPLIT FILE BY resp_id.\nFREQUENCIES age.".to_string(),
        "SPLIT FILE BY resp_id.\nCROSSTABS age BY gender BY region /STATISTICS=ALL.".to_string(),
        "CROSSTABS q3_comment BY resp_id /CELLS=ALL /STATISTICS=ALL.".to_string(),
        "CROSSTABS resp_id BY monthly_spend /STATISTICS=ALL.".to_string(),
        "CTABLES /TABLE resp_id BY monthly_spend /COMPARETEST TYPE=PROP /SIGTEST TYPE=CHISQUARE.".to_string(),
        "NPAR TESTS /K-S(NORMAL)=age /RUNS(MEDIAN)=age /FRIEDMAN=q5_1 TO q5_5 /COCHRAN=q5_1 TO q5_5 /KENDALL=q5_1 TO q5_5 /M-W=age BY gender(1, 2) /K-W=age BY region(1, 2000000000) /J-T=age BY region(1, 2000000000) /MEDIAN=age BY region(1, 2000000000).".to_string(),
        "NPAR TESTS /K-W=age BY resp_id(1, 2000000000) /J-T=age BY resp_id(1, 2000000000) /MEDIAN=age BY resp_id(1, 2000000000).".to_string(),
        "NPAR TESTS /M-W=resp_id BY gender(1, 2) /METHOD=EXACT TIMER(1).".to_string(),
        "NPAR TESTS /CHISQUARE=age (1, 2000000000) /BINOMIAL=age (1e300).".to_string(),
        "CORRELATIONS resp_id WITH resp_id.\nNONPAR CORR resp_id monthly_spend /PRINT=KENDALL.".to_string(),
        "T-TEST GROUPS=resp_id(1e300) /VARIABLES=age.".to_string(),
        // post hoc tests are slow, not stuck: 200 groups make 19 900 pairs, and DUNNETT, GABRIEL
        // and the range tests take seconds even for 5 groups in a debug build
        "ONEWAY age BY region /POSTHOC=ALL /STATISTICS=ALL.".to_string(),
        "ONEWAY age BY region /POSTHOC=TUKEY GH C /STATISTICS=ALL.".to_string(),
        "EXAMINE age BY resp_id /PLOT=NONE /STATISTICS=ALL /PERCENTILES(1e-300, 99.99999) /MESTIMATORS.".to_string(),
        "REGRESSION /DEPENDENT=age /METHOD=STEPWISE gender TO weight /STATISTICS=ALL.".to_string(),
        "GLM age BY q1_satisfaction region gender /PRINT=DESCRIPTIVE ETASQ OPOWER /POSTHOC=region (TUKEY).".to_string(),
        "LOGISTIC REGRESSION gender WITH resp_id age monthly_spend /CRITERIA=ITERATE(2000000000).".to_string(),
        "FACTOR /VARIABLES=resp_id q5_1 q5_2 q5_3 /CRITERIA=ITERATE(2000000000) FACTORS(2000000000) /ROTATION=PROMAX(2000000000).".to_string(),
        "QUICK CLUSTER age monthly_spend /CRITERIA=CLUSTERS(2000000000) MXITER(2000000000).".to_string(),
        "CLUSTER age monthly_spend /PRINT=CLUSTER(1, 2000000000).".to_string(),
        "DISCRIMINANT /GROUPS=resp_id(1, 2000000000) /VARIABLES=age monthly_spend.".to_string(),
        "RELIABILITY /VARIABLES=resp_id resp_id resp_id /MODEL=ALPHA /STATISTICS=ALL /SUMMARY=ALL /ICC=MODEL(MIXED).".to_string(),
        "TREE age BY gender region /TREE=TOP /GROWTHLIMIT MAXDEPTH(2000000000) MINPARENTSIZE(1) MINCHILDSIZE(1).".to_string(),
        "AGGREGATE /OUTFILE=* /BREAK=resp_id /n = N /p = PIN(age, -1e300, 1e300).".to_string(),
        "CASESTOVARS /ID=resp_id.\nVARSTOCASES /MAKE x FROM q5_1 TO q5_5.".to_string(),
        "CASESTOVARS /INDEX=resp_id.".to_string(),
        "FLIP.\nFLIP.".to_string(),
        "AUTORECODE q3_comment /INTO x.\nAUTORECODE resp_id /INTO y.".to_string(),
        "REPORT /FORMAT=LIST PAGE(2000000000) LENGTH(1, 2000000000) /VARIABLES=age (2000000000) q3_comment /BREAK=resp_id.".to_string(),
        "LIST /CASES FROM -1 TO 2000000000 BY 0.".to_string(),
        "LIST /CASES FROM 2000000000 TO 1 BY -1.".to_string(),
        "SET WIDTH=2000000000.\nSET LENGTH=1.\nLIST.".to_string(),
        "SHOW ALL.\nDISPLAY DICTIONARY.\nSYSFILE INFO FILE='nonexistent.sav'.".to_string(),
        "MRSETS /MDGROUP NAME=$a VARIABLES=q5_1 TO q5_5 VALUE=1e300 /MCGROUP NAME=$b VARIABLES=q6_mention1 TO q6_mention3.\nMULT RESPONSE GROUPS=a (q5_1 TO q5_5 (1)) b (q6_mention1 TO q6_mention3 (1, 2000000000)) /FREQUENCIES=a b /TABLES=a BY b.\nCTABLES /TABLE $a BY $b /SIGTEST TYPE=CHISQUARE.".to_string(),
        "OMS /SELECT ALL /DESTINATION FORMAT=HTML OUTFILE='fuzz_out.html'.\nFREQUENCIES age.\nOMSEND.".to_string(),
        "BEGIN DATA\n1\nEND DATA.".to_string(),
        "END DATA.".to_string(),
        "BEGIN DATA.".to_string(),
        "DATA LIST /x 1.\nBEGIN DATA".to_string(),
        "\u{feff}FREQUENCIES age.".to_string(),
        "FREQUENCIES age.\u{0}\u{1}\u{7f}".to_string(),
        "FREQ\u{0}UENCIES age.".to_string(),
        format!("FREQUENCIES {}.", "age ".repeat(20000)),
        format!("FREQUENCIES {}.", "x".repeat(100000)),
        format!("COMPUTE {} = 1.", "x".repeat(100000)),
        format!("VARIABLE LABELS age '{}'.", "a".repeat(1_000_000)),
        format!("VALUE LABELS age {}.", (0..20000).map(|i| format!("{i} 'l{i}'")).collect::<Vec<_>>().join(" ")),
        format!("{}FREQUENCIES age.", "\n".repeat(200000)),
        ".".repeat(100000),
        ".".repeat(100000),
        format!("FREQUENCIES age /{}.", "/".repeat(100000)),
        format!("COMPUTE x = {}.", "'".repeat(10001)),
        "COMPUTE x = 'unterminated.\nFREQUENCIES age.".to_string(),
        "COMPUTE x = \"unterminated.\nFREQUENCIES age.".to_string(),
        "* unterminated comment".to_string(),
        "/* unterminated comment".to_string(),
        "COMMENT".to_string(),
        "".to_string(),
        " ".to_string(),
        ".".to_string(),
        "-".to_string(),
        "+".to_string(),
        "INSERT FILE='fuzz_self.sps'.".to_string(),
        "INCLUDE FILE='fuzz_self.sps'.".to_string(),
        "INSERT FILE='/'.".to_string(),
        "INSERT FILE=''.".to_string(),
        "GET FILE='/'.".to_string(),
        "GET FILE=''.".to_string(),
        "GET DATA /TYPE=TXT /FILE='/' /VARIABLES=x F1.".to_string(),
        "GET DATA /TYPE=XLSX /FILE='/'.".to_string(),
        "FILE HANDLE h /NAME='h'.\nFILE HANDLE h /NAME='h'.\nGET FILE=h.".to_string(),
        "DATASET NAME a.\nDATASET COPY a.\nDATASET ACTIVATE a.\nDATASET CLOSE ALL.\nFREQUENCIES age.".to_string(),
        "DATASET CLOSE *.\nFREQUENCIES age.".to_string(),
        "NEW FILE.\nFREQUENCIES age.".to_string(),
        "NEW FILE.\nCOMPUTE x = 1.\nEXECUTE.\nLIST.".to_string(),
        "MATCH FILES /FILE=* /FILE=* /BY resp_id.".to_string(),
        "ADD FILES /FILE=* /FILE=* /FILE=* /FILE=*.".to_string(),
        "UPDATE /FILE=* /FILE=* /BY resp_id.".to_string(),
        "SORT CASES BY q3_comment (D) resp_id.\nMATCH FILES /FILE=* /TABLE=* /BY resp_id.".to_string(),
        "WEIGHT BY monthly_spend.\nCOMPUTE monthly_spend = -1e308.\nFREQUENCIES age /STATISTICS=ALL.\nCROSSTABS age BY gender /STATISTICS=ALL.\nCTABLES /TABLE age BY gender.".to_string(),
        "WEIGHT BY weight.\nCOMPUTE weight = 1e300.\nFREQUENCIES age /STATISTICS=ALL.\nDESCRIPTIVES ALL.\nMEANS age BY gender /CELLS=ALL.".to_string(),
        "WEIGHT BY weight.\nCOMPUTE weight = 1e-300.\nFREQUENCIES age /STATISTICS=ALL.\nCROSSTABS age BY gender /STATISTICS=ALL.".to_string(),
        "TEMPORARY.\nSELECT IF 0.\nFREQUENCIES age.\nTEMPORARY.\nTEMPORARY.\nLIST.".to_string(),
        "FILTER BY q3_comment.".to_string(),
        "SELECT IF $SYSMIS.\nFREQUENCIES ALL /STATISTICS=ALL.".to_string(),
    ];
    out.push(
        "DEFINE !m(!POS !ENCLOSE('(', ')')) !IF (!1 !EQ 0) !THEN x !ELSE !m(!1) !IFEND !ENDDEFINE.\n!m(1)."
            .to_string(),
    );
    out
}

/// Every seed: help examples and diagrams, the docs, the test corpus, the extremes.
fn seeds(rng: &mut Rng) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for spec in statscraft_syntax::command_specs() {
        out.push((format!("example {}", spec.name), spec.example.to_string()));
        for k in 0..3 {
            let text = instantiate(spec.syntax, rng);
            out.push((format!("diagram {} #{k}", spec.name), text));
        }
    }
    let mut docs: Vec<PathBuf> = std::fs::read_dir(root().join("docs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    docs.sort();
    for path in docs {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (k, block) in code_blocks(&text).into_iter().enumerate() {
            let name = path.file_name().unwrap().to_string_lossy();
            out.push((format!("{name} block {k}"), block));
        }
    }
    let mut corpus: Vec<PathBuf> = Vec::new();
    for dir in ["crates/engine/tests", "crates/syntax/src"] {
        let files = std::fs::read_dir(root().join(dir))
            .into_iter()
            .flatten()
            .flatten();
        corpus.extend(files.map(|e| e.path()).filter(|p| {
            p.extension().is_some_and(|e| e == "rs")
                && p.file_name().is_some_and(|n| n != "robustness.rs")
        }));
    }
    corpus.sort();
    for path in corpus {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        for (k, literal) in literals(&text).into_iter().enumerate() {
            out.push((format!("{name} literal {k}"), literal));
        }
    }
    for (k, text) in extremes().into_iter().enumerate() {
        out.push((format!("extreme {k}"), text));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Mutation

/// Splits text into tokens that keep every character: blanks, quoted strings, numbers, words
/// and single other characters.
fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        i += 1;
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else if c == '\'' || c == '"' {
            while i < chars.len() && chars[i] != c && chars[i] != '\n' {
                i += 1;
            }
            i = (i + 1).min(chars.len());
        } else if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
        } else if c.is_alphabetic() || "_$#@!".contains(c) {
            while i < chars.len() && (chars[i].is_alphanumeric() || "_$#@.".contains(chars[i])) {
                i += 1;
            }
        }
        out.push(chars[start..i].iter().collect());
    }
    out
}

fn mutate(seed: &str, vocabulary: &[String], rng: &mut Rng) -> String {
    let mut t = tokens(seed);
    if t.is_empty() {
        t.push(String::new());
    }
    for _ in 0..1 + rng.below(3) {
        let solid: Vec<usize> = (0..t.len()).filter(|&i| !t[i].trim().is_empty()).collect();
        let at = if solid.is_empty() {
            rng.below(t.len())
        } else {
            *rng.pick(&solid)
        };
        match rng.below(12) {
            0 => {
                t.remove(at);
            }
            1 => {
                let times = *rng.pick(&[1, 2, 5, 50, 1000]);
                let copy = t[at].clone();
                for _ in 0..times {
                    t.insert(at, format!("{copy} "));
                }
            }
            2 => {
                let other = rng.below(t.len());
                t.swap(at, other);
            }
            3 => {
                let cut = rng.below(t.len() + 1);
                t.truncate(cut);
            }
            4 => {
                // a string where a number belongs, and the other way round
                t[at] = if t[at].starts_with(|c: char| c.is_ascii_digit()) {
                    "'1'".to_string()
                } else if t[at].starts_with(['\'', '"']) {
                    "1".to_string()
                } else {
                    rng.pick(&["1", "'x'", "plan", "q3_comment", "ALL", "TO"])
                        .to_string()
                };
            }
            5 => {
                let numbers: Vec<usize> = (0..t.len())
                    .filter(|&i| t[i].starts_with(|c: char| c.is_ascii_digit()))
                    .collect();
                let at = numbers.get(rng.below(numbers.len())).copied().unwrap_or(at);
                t[at] = rng.pick(ODD_NUMBERS).to_string();
            }
            6 => {
                // an empty list: everything after `=` up to the next slash or period
                let eq: Vec<usize> = (0..t.len()).filter(|&i| t[i] == "=").collect();
                if let Some(&e) = eq.get(rng.below(eq.len())) {
                    let mut j = e + 1;
                    while j < t.len() && t[j] != "/" && t[j] != "." {
                        j += 1;
                    }
                    t.drain(e + 1..j);
                }
            }
            7 => {
                let bracket = rng.pick(BRACKETS).to_string();
                t.insert(at, bracket);
            }
            8 => {
                let word = rng.pick(vocabulary).clone();
                t.insert(at, format!(" {word} "));
            }
            9 => {
                // a subcommand repeated
                let slashes: Vec<usize> = (0..t.len()).filter(|&i| t[i] == "/").collect();
                if let Some(&s) = slashes.get(rng.below(slashes.len())) {
                    let mut j = s + 1;
                    while j < t.len() && t[j] != "/" && t[j] != "." {
                        j += 1;
                    }
                    let part: Vec<String> = t[s..j].to_vec();
                    for _ in 0..*rng.pick(&[1, 3, 100]) {
                        t.splice(s..s, part.iter().cloned());
                    }
                }
            }
            10 => {
                // a variable of the other type
                t[at] = if rng.below(2) == 0 {
                    rng.pick(STRINGS).to_string()
                } else {
                    rng.pick(NUMERIC).to_string()
                };
            }
            _ => {
                // a cut inside a token: half a keyword, a quote without its end
                let chars: Vec<char> = t[at].chars().collect();
                let keep = rng.below(chars.len().max(1));
                t[at] = chars[..keep].iter().collect();
            }
        }
        if t.is_empty() {
            t.push(String::new());
        }
    }
    t.concat()
}

/// The words of the help diagrams and the variable names, for inserted tokens.
fn vocabulary() -> Vec<String> {
    let mut words: Vec<String> = statscraft_syntax::command_specs()
        .iter()
        .flat_map(|s| {
            s.syntax
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        })
        .filter(|w| {
            w.len() > 1
                && w.chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c == '-')
        })
        .map(str::to_string)
        .collect();
    words.extend(
        statscraft_syntax::command_names()
            .iter()
            .map(|s| s.to_string()),
    );
    words.extend(NUMERIC.iter().chain(STRINGS).map(|s| s.to_string()));
    words.extend(
        [
            "!m",
            "!ENDDEFINE",
            "END DATA",
            "BEGIN DATA",
            "TO",
            "BY",
            "ALL",
            "WITH",
        ]
        .map(String::from),
    );
    words.sort();
    words.dedup();
    words
}

// ---------------------------------------------------------------------------------------------
// Running

/// The panics of the worker threads, by thread name.
static PANICS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

fn install_hook() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let name = std::thread::current().name().unwrap_or("").to_string();
            if !name.starts_with("robust-") {
                previous(info);
                return;
            }
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let place = info.location().map(|l| l.to_string()).unwrap_or_default();
            if let Ok(mut panics) = PANICS.lock() {
                panics.insert(name, format!("{place}: {message}"));
            }
        }));
    });
}

/// A folder for the files the runs write, made the working directory once.
fn work_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("statscraft-robustness-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // INSERT of this file inserts itself
        std::fs::write(dir.join("fuzz_self.sps"), "INSERT FILE='fuzz_self.sps'.\n").unwrap();
        std::env::set_current_dir(&dir).unwrap();
        dir
    })
}

fn survey() -> Dataset {
    static DATA: OnceLock<Dataset> = OnceLock::new();
    DATA.get_or_init(|| {
        let path = root().join("tests/fixtures/survey_basic.sav");
        let mut session = Session::default();
        session
            .run(&format!("GET FILE='{}'.", path.display()), None)
            .unwrap();
        session.dataset().unwrap().clone()
    })
    .clone()
}

#[derive(Debug)]
enum Trouble {
    Panic(String),
    Hang,
}

/// Runs `source` like `statscraft check` (no dataset, nothing written) or `statscraft run` on
/// `data`, and renders the result as text and JSON like the command line prints it.
fn run_one(
    source: &str,
    data: Option<Dataset>,
    check: bool,
    limit: Duration,
    tag: usize,
) -> Option<Trouble> {
    install_hook();
    let dir = work_dir().to_path_buf();
    let name = format!("robust-{tag}");
    let source = source.to_string();
    let (send, receive) = mpsc::channel();
    // the window's worker thread has the default stack, so this one has too
    std::thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            let mut session = Session::with_dataset(data);
            session.set_base_dir(Some(dir));
            let result = if check {
                session.check(&source, None)
            } else {
                session.run(&source, None)
            };
            if let Ok(result) = &result {
                let _ = result.output(false, check);
                let _ = result.output(true, check);
            }
            let _ = session.describe(false);
            let _ = send.send(());
        })
        .unwrap();
    match receive.recv_timeout(limit) {
        Ok(()) => None,
        Err(mpsc::RecvTimeoutError::Timeout) => Some(Trouble::Hang),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let message = PANICS.lock().unwrap().remove(&name).unwrap_or_default();
            Some(Trouble::Panic(message))
        }
    }
}

/// Check, then run; a run that may delete or lock files only runs as a check.
fn ways(input: &str) -> &'static [bool] {
    let upper = input.to_ascii_uppercase();
    if upper.contains("ERASE") || upper.contains("PERMISSIONS") {
        &[true]
    } else {
        &[true, false]
    }
}

/// Runs every input both ways; returns the troubles, one per panic place, with the input.
fn run_all(inputs: &[(String, String)], limit: Duration) -> Vec<String> {
    let mut seen = BTreeMap::new();
    for (k, (label, input)) in inputs.iter().enumerate() {
        for &check in ways(input) {
            let data = (!check).then(survey);
            if let Some(trouble) = run_one(input, data, check, limit, k) {
                let key = match &trouble {
                    Trouble::Panic(message) => message.split(": ").next().unwrap_or("").to_string(),
                    Trouble::Hang => format!("hang {label}"),
                };
                let mode = if check { "check" } else { "run" };
                seen.entry(key).or_insert_with(|| {
                    format!("{trouble:?} in {mode} of {label}:\n{}\n", shorten(input))
                });
            }
        }
    }
    seen.into_values().collect()
}

fn shorten(text: &str) -> String {
    if text.len() <= 600 {
        return text.to_string();
    }
    let mut end = 600;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &text[..end], text.len())
}

fn mutants(variants: usize, shard: (usize, usize)) -> Vec<(String, String)> {
    let mut rng = Rng(0x5eed_cafe_f00d_1234);
    let words = vocabulary();
    let mut inputs = Vec::new();
    for (k, (label, seed)) in seeds(&mut rng).into_iter().enumerate() {
        let mut mine = Vec::new();
        mine.push((label.clone(), seed.clone()));
        // a seed of a megabyte is extreme already; its mutants would fill the memory
        for v in 0..if seed.len() > 65_536 { 0 } else { variants } {
            mine.push((
                format!("{label} mutant {v}"),
                mutate(&seed, &words, &mut rng),
            ));
        }
        if k % shard.1 == shard.0 {
            inputs.extend(mine);
        }
    }
    inputs
}

fn env_number(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The full set. Each batch of inputs runs in a child process (this test binary again) under a
/// virtual memory limit, so a hang or a run that eats the memory is found, reported and passed
/// over by starting a new child after that input.
#[test]
#[ignore = "the full set takes many minutes; run by hand"]
fn mutated_syntax_never_crashes() {
    let shard = std::env::var("FUZZ_SHARD")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('/')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((0, 1));
    let inputs = mutants(env_number("FUZZ_VARIANTS", 12), shard);
    if let Ok(from) = std::env::var("FUZZ_CHILD") {
        return child(&inputs, from.parse().unwrap());
    }
    let limit = Duration::from_secs(env_number("FUZZ_LIMIT", 60) as u64);
    let memory_kib = env_number("FUZZ_MEMORY_MIB", 1500) * 1024;
    let mut troubles = BTreeMap::new();
    let mut from = 0;
    while from < inputs.len() {
        let (lines, mut process) = spawn_child(from, memory_kib);
        let mut current = from;
        let mut started = false;
        let ended = loop {
            // a child first makes the inputs again, which takes longer than a run may
            let wait = if started {
                limit
            } else {
                limit.max(Duration::from_secs(600))
            };
            match lines.recv_timeout(wait) {
                Ok(line) if line == "@@end" => break None,
                Ok(line) => {
                    if let Some(k) = line.strip_prefix("@@start ") {
                        current = k.parse().unwrap();
                        started = true;
                    } else if let Some(rest) = line.strip_prefix("@@panic ") {
                        let (mode, message) = rest.split_once(' ').unwrap_or((rest, ""));
                        let place = message.split(": ").next().unwrap_or("").to_string();
                        let (label, input) = &inputs[current];
                        troubles.entry(place).or_insert_with(|| {
                            format!(
                                "panic {message}\nin {mode} of {label}:\n{}\n",
                                shorten(input)
                            )
                        });
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break Some("hang"),
                Err(mpsc::RecvTimeoutError::Disconnected) => break Some("abort or memory"),
            }
        };
        assert!(
            started,
            "the child for input {from} stopped before its first run"
        );
        let _ = process.kill();
        let _ = process.wait();
        match ended {
            None => break,
            Some(kind) => {
                let (label, input) = &inputs[current];
                troubles.insert(
                    format!("{kind} {label}"),
                    format!("{kind} in {label}:\n{}\n", shorten(input)),
                );
                from = current + 1;
            }
        }
    }
    println!("{} inputs, {} troubles", inputs.len(), troubles.len());
    for t in troubles.values() {
        println!("{t}");
    }
    assert!(troubles.is_empty());
}

/// Starts this test again as a child that runs the inputs from `from` on, and the lines it
/// prints.
fn spawn_child(from: usize, memory_kib: usize) -> (mpsc::Receiver<String>, std::process::Child) {
    use std::io::BufRead;
    let exe = std::env::current_exe().unwrap();
    let mut process = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("ulimit -v {memory_kib}; exec \"$0\" \"$@\""))
        .arg(exe)
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "mutated_syntax_never_crashes",
        ])
        .env("FUZZ_CHILD", from.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdout = process.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if line.starts_with("@@") && send.send(line).is_err() {
                break;
            }
        }
    });
    (receive, process)
}

fn child(inputs: &[(String, String)], from: usize) {
    use std::io::Write;
    let trace = std::env::var("FUZZ_TRACE").ok();
    let mut out = std::io::stdout();
    for (k, (label, input)) in inputs.iter().enumerate().skip(from) {
        if let Some(trace) = &trace {
            let _ = std::fs::write(trace, format!("{label}\n{input}"));
        }
        let _ = writeln!(out, "@@start {k}");
        let _ = out.flush();
        for &check in ways(input) {
            let data = (!check).then(survey);
            // the parent stops a run that hangs
            if let Some(Trouble::Panic(message)) =
                run_one(input, data, check, Duration::from_secs(1 << 30), k)
            {
                let mode = if check { "check" } else { "run" };
                let _ = writeln!(out, "@@panic {mode} {}", message.replace('\n', " "));
            }
        }
    }
    let _ = writeln!(out, "@@end");
}

#[test]
fn mutated_syntax_smoke() {
    // every 60th seed with one mutant: about 120 runs
    let inputs: Vec<_> = mutants(1, (0, 60));
    let troubles = run_all(&inputs, Duration::from_secs(60));
    assert!(troubles.is_empty(), "{}", troubles.join("\n"));
}

/// Runs `source` on survey_basic.sav on a thread with the window's stack: the text of the
/// tables and warnings, or of the errors.
fn outcome(source: &str) -> Result<String, String> {
    let source = source.to_string();
    let dir = work_dir().to_path_buf();
    let data = survey();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut session = Session::with_dataset(Some(data));
        session.set_base_dir(Some(dir));
        let result = match session.run(&source, None) {
            Ok(result) => Ok(result.output(false, false)),
            Err(errors) => Err(errors
                .iter()
                .map(|e| e.text())
                .collect::<Vec<_>>()
                .join("\n")),
        };
        let _ = send.send(result);
    });
    receive
        .recv_timeout(Duration::from_secs(60))
        .expect("the run ends without a panic within a minute")
}

/// Inputs the generator found to crash, hang or fill the memory, with a part of what they give
/// now: an error, or (after `ok:`) a part of the output.
const REGRESSIONS: &[(&str, &str)] = &[
    (
        "SET MNEST=100000.\nDEFINE !m() !m !ENDDEFINE.\n!m.",
        "nested more than 100 levels deep",
    ),
    (
        "DEFINE !m() !EVAL(!m) !ENDDEFINE.\n!m.",
        "nested more than 50 levels deep",
    ),
    (
        "DEFINE !m(!POS !CMDEND) !m !1 !1 !ENDDEFINE.\n!m a b c.",
        "grows longer than 1048576 characters",
    ),
    (
        "DEFINE !m(!POS !TOKENS(1)) !IF (!LENGTH(!1) !GT 1) !THEN !m !SUBSTR(!1, 2) \
         !m !SUBSTR(!1, 2) !IFEND !ENDDEFINE.\n!m aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.",
        "makes more than 100000 calls",
    ),
    (
        "DEFINE !m() !BLANKS(99999999999) !ENDDEFINE.\n!m.",
        "!BLANKS takes at most 1048576 blanks",
    ),
    (
        "SET MITERATE=2000000000.\n\
         DEFINE !m() !DO !i = 1 !TO 2000000000 !BREAK !DOEND !ENDDEFINE.\n!m.",
        "ok:",
    ),
    (
        "FREQUENCIES age /NTILES=2147483648.",
        "NTILES takes a whole number from 1 to 1000",
    ),
    ("COMPUTE x = NCDF.T(1, 1, 1e300).\nLIST x /CASES=1.", "ok:"),
    (
        "COMPUTE x = NCDF.CHISQ(1, 1, 1e300).\nLIST x /CASES=1.",
        "ok:",
    ),
    (
        "COMPUTE x = NCDF.F(1, 1, 1, 1e300).\nLIST x /CASES=1.",
        "ok:",
    ),
    (
        "LOOP #i = 1e17 TO 1e17 + 10.\nCOMPUTE x = #i.\nEND LOOP.\nEXECUTE.",
        "the LOOP step is too small to change the index",
    ),
    (
        "MATRIX.\nLOOP i = 1 TO 1e300 BY 4e18.\nEND LOOP.\nPRINT i.\nEND MATRIX.",
        "ok:",
    ),
    ("DATE OBS 9 1e308.", "too large for DATE"),
    ("DATE Y 18446744073709551616 M 7.", "too large for DATE"),
    (
        "MATRIX DATA VARIABLES=ROWTYPE_ b /FORMAT=NODIAGONAL.\nBEGIN DATA\nCOV 1\nEND DATA.",
        "a COV record has no cells to hold",
    ),
    (
        "NPAR TESTS /CHISQUARE=age (1, 2000000000).",
        "holds more than 100 000 categories",
    ),
    (
        "MATRIX.\nCOMPUTE a = MAKE(100000, 100000, 1).\nEND MATRIX.",
        "would hold more than 100000000 elements",
    ),
    (
        "MATRIX.\nCOMPUTE a = MAKE(20000, 1, 1) * MAKE(1, 20000, 1).\nEND MATRIX.",
        "would hold more than 100000000 elements",
    ),
    (
        "COMPUTE x = CDF.HYPER(1e9, 2e9, 1e9, 1e9).\nLIST x /CASES=1.",
        "ok:",
    ),
    (
        "COMPUTE x = IDF.F(0.5, 1e300, 1e300).\nLIST x /CASES=1.",
        "ok:",
    ),
];

/// Nesting that overflowed the stack of a run's thread.
fn deep_regressions() -> Vec<(String, &'static str)> {
    let deep = |open: &str, close: &str, n: usize| format!("{}{}", open.repeat(n), close.repeat(n));
    let too_deep = "nested too deeply (more than 64 levels)";
    let parens = deep("(", ")", 99).replace("()", "(1)");
    let braces = deep("{", "}", 99).replace("{}", "{1}");
    let loops = deep("LOOP #i = 1 TO 1.\n", "END LOOP.\n", 500);
    vec![
        (
            format!("MATRIX.\nCOMPUTE a = {parens}.\nEND MATRIX."),
            too_deep,
        ),
        (
            format!("MATRIX.\nCOMPUTE a = {braces}.\nEND MATRIX."),
            too_deep,
        ),
        (
            format!("GLM age BY gender /DESIGN={}.", deep("gender(", ")", 2000)),
            too_deep,
        ),
        (
            loops.replacen("END LOOP", "COMPUTE x = 1.\nEND LOOP", 1),
            "LOOP is nested more than 100 levels deep",
        ),
        (
            format!(
                "MATRIX.\n{}END MATRIX.",
                deep("DO IF 1.\n", "END IF.\n", 2000)
            ),
            "DO IF is nested more than 100 levels deep",
        ),
        (
            format!(
                "MATRIX.\n{}END MATRIX.",
                deep("LOOP i = 1 TO 1.\n", "END LOOP.\n", 500)
            ),
            "LOOP is nested more than 100 levels deep",
        ),
    ]
}

#[test]
fn a_line_range_from_zero_or_reversed_is_an_error() {
    // the window takes ranges from clients that need not check them as the command line does
    for lines in [(0, 3), (5, 2), (0, 0)] {
        let mut session = Session::with_dataset(Some(survey()));
        let errors = session.run("FREQUENCIES age.", Some(lines)).unwrap_err();
        assert!(errors[0].text().contains("invalid line range"), "{lines:?}");
    }
}

#[test]
fn inputs_that_crashed_before_give_an_answer() {
    let fixed = REGRESSIONS.iter().map(|&(s, e)| (s.to_string(), e));
    for (source, expected) in fixed.chain(deep_regressions()) {
        let text = match outcome(&source) {
            Ok(text) => format!("ok: {text}"),
            Err(text) => text,
        };
        assert!(text.contains(expected), "{source}\ngave {text}");
    }
}

// ---------------------------------------------------------------------------------------------
// Edge data: every procedure with its default subcommands on odd datasets

/// Variables x, y and z (numeric), g (groups 1 and 2), b (0 and 1) and s (a string).
fn edge_data(name: &str, rows: &[&str], width: usize, extra: &str) -> (String, Dataset) {
    let mut syntax = format!("DATA LIST FREE /x y z g b s (A{width}){extra}.\nBEGIN DATA\n");
    for row in rows {
        syntax.push_str(row);
        syntax.push('\n');
    }
    syntax.push_str("END DATA.");
    let mut session = Session::default();
    session.set_base_dir(Some(work_dir().to_path_buf()));
    if let Err(e) = session.run(&syntax, None) {
        panic!("{name}: {e:?}");
    }
    (name.to_string(), session.dataset().unwrap().clone())
}

fn edge_sets() -> Vec<(String, Dataset)> {
    let long_rows: Vec<String> = ["a", "b", "õ"]
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let text = c.repeat(32767 / c.len());
            format!("{i} {} 1 {} {} '{text}'", i * 2, 1 + i % 2, i % 2)
        })
        .collect();
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }
    vec![
        edge_data("zero cases", &[], 8, ""),
        edge_data("one case", &["1 2 3 1 1 a"], 8, ""),
        edge_data(
            "all missing",
            &[". . . . . ''", ". . . . . ''", ". . . . . ''"],
            8,
            "",
        ),
        edge_data(
            "constant",
            &["5 5 5 1 1 a", "5 5 5 1 1 a", "5 5 5 1 1 a", "5 5 5 1 1 a"],
            8,
            "",
        ),
        edge_data(
            "single category",
            &["1 4 2 1 1 a", "2 3 5 1 1 a", "3 9 1 1 1 a", "4 1 7 1 1 a"],
            8,
            "",
        ),
        edge_data("two cases", &["1 2 3 1 0 a", "2 1 5 2 1 b"], 8, ""),
        edge_data(
            "huge values",
            &[
                "1e300 -1e300 1e308 1 0 a",
                "-1e308 1e300 -1e300 2 1 b",
                "1.7976931348623157e308 1e299 1e307 1 1 c",
                "-1.7e308 -1e307 1e306 2 0 d",
                "1e308 1e308 -1e308 1 1 e",
            ],
            8,
            "",
        ),
        edge_data(
            "tiny values",
            &[
                "1e-300 -1e-300 5e-324 1 0 a",
                "-5e-324 1e-310 -1e-308 2 1 b",
                "1e-320 2e-320 3e-320 1 1 c",
                "0 -0 1e-323 2 0 d",
            ],
            8,
            "",
        ),
        edge_data(
            "mixed scale",
            &[
                "1e300 1 1e-300 1 0 a",
                "1 1e-300 1e300 2 1 b",
                "1e-300 1e300 1 1 1 c",
                "2 3 4 2 0 d",
            ],
            8,
            "",
        ),
        edge_data(
            "nan-like input",
            &[
                "nan inf -inf 1 0 a",
                "1e999 -1e999 1e-999 2 1 b",
                "NaN Infinity 0x10 1 1 c",
                "1 2 3 2 0 d",
            ],
            8,
            "",
        ),
        edge_data("very long strings", &refs(&long_rows), 32767, ""),
    ]
}

/// Every procedure with its default subcommands, then with the statistics that divide by
/// counts and variances.
const PROCEDURES: &[&str] = &[
    "FREQUENCIES x g s.",
    "FREQUENCIES x /STATISTICS=ALL /PERCENTILES=1 50 99 /NTILES=4.",
    "FREQUENCIES x /GROUPED=x /PERCENTILES=25 50 /FORMAT=DFREQ.",
    "DESCRIPTIVES x y z.",
    "DESCRIPTIVES x y z /STATISTICS=ALL /SAVE.",
    "CROSSTABS g BY s.",
    "CROSSTABS g BY b BY s /STATISTICS=CHISQ /CELLS=ALL.",
    "MEANS x BY g.",
    "MEANS x y BY g BY b /CELLS=ALL.",
    "T-TEST GROUPS=g(1,2) /VARIABLES=x.",
    "T-TEST TESTVAL=0 /VARIABLES=x y.",
    "T-TEST PAIRS=x WITH y (PAIRED).",
    "ONEWAY x BY g.",
    "ONEWAY x y BY g /STATISTICS=ALL /POSTHOC=TUKEY BONFERRONI GH SCHEFFE.",
    "CORRELATIONS x y z.",
    "NONPAR CORR x y z /PRINT=BOTH.",
    "PARTIAL CORR /VARIABLES=x y BY z.",
    "NPAR TESTS /M-W=x BY g(1,2).",
    "NPAR TESTS /WILCOXON=x WITH y /SIGN=x WITH y /K-S(NORMAL)=x /RUNS(MEDIAN)=x /CHISQUARE=g \
     /BINOMIAL=b /FRIEDMAN=x y z /KENDALL=x y z /COCHRAN=b /K-W=x BY g(1,2) /MEDIAN=x BY g(1,2) \
     /MCNEMAR=b WITH b /J-T=x BY g(1,2) /STATISTICS=ALL.",
    "EXAMINE VARIABLES=x BY g.",
    "EXAMINE VARIABLES=x y /STATISTICS=ALL /MESTIMATORS /PERCENTILES.",
    "REGRESSION /DEPENDENT=y /METHOD=ENTER x z.",
    "REGRESSION /STATISTICS=ALL /DEPENDENT=y /METHOD=STEPWISE x z g.",
    "RELIABILITY /VARIABLES=x y z.",
    "RELIABILITY /VARIABLES=x y z /STATISTICS=ALL /SUMMARY=ALL /ICC=MODEL(MIXED).",
    "FACTOR VARIABLES=x y z.",
    "FACTOR VARIABLES=x y z /EXTRACTION=ML /ROTATION=PROMAX /PRINT=ALL.",
    "QUICK CLUSTER x y.",
    "CLUSTER x y.",
    "DISCRIMINANT GROUPS=g(1,2) /VARIABLES=x y.",
    "LOGISTIC REGRESSION VARIABLES=b WITH x y.",
    "UNIANOVA x BY g WITH y.",
    "UNIANOVA x BY g b /PRINT=DESCRIPTIVE HOMOGENEITY PARAMETER ETASQ OPOWER LOF /POSTHOC=g (TUKEY).",
    "GLM x y z /WSFACTOR=time 3.",
    "ANOVA VARIABLES=x BY g(1,2).",
    "TREE b BY g x.",
    "REPORT FORMAT=LIST /VARIABLES=x y /BREAK=g /SUMMARY=MEAN(x).",
    "MULT RESPONSE GROUPS=m (b g (1)) /FREQUENCIES=m /TABLES=m BY s.",
    "MRSETS /MDGROUP NAME=$m VARIABLES=b g VALUE=1.\nCTABLES /TABLE $m BY s.",
    "CTABLES /TABLE g BY s.",
    "CTABLES /TABLE x [MEAN, STDDEV, MEDIAN, COUNT] BY g /COMPARETEST TYPE=MEAN.",
    "CTABLES /TABLE g BY b /COMPARETEST TYPE=PROP /SIGTEST TYPE=CHISQUARE.",
    "RANK VARIABLES=x.",
    "RANK VARIABLES=x BY g /NTILES(4) /PERCENT /RFRACTION /NORMAL /SAVAGE /PROPORTION /N.",
    "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=g /m = MEAN(x) /n = N.",
    "AGGREGATE /OUTFILE=* /BREAK=g s /m = MEDIAN(x) /sd = SD(y).",
    "AUTORECODE VARIABLES=s /INTO s2.",
    "VARSTOCASES /MAKE v FROM x y z.",
    "CASESTOVARS /ID=g.",
    "FLIP.",
    "COUNT c = x y z (1).",
    "LIST.",
    "DISPLAY DICTIONARY.",
    "SORT CASES BY s x.",
    "SPLIT FILE BY g.\nFREQUENCIES x.",
    "COMPUTE n = x + y * z / g ** b.\nEXECUTE.",
    "RECODE x (LO THRU 0 = 1) (ELSE = 2) INTO r.\nEXECUTE.",
    "WEIGHT BY z.\nFREQUENCIES x /STATISTICS=ALL.\nCROSSTABS g BY b /STATISTICS=CHISQ.",
    "SAVE OUTFILE='edge.sav'.\nGET FILE='edge.sav'.\nLIST.",
    "SAVE TRANSLATE OUTFILE='edge.csv' /TYPE=CSV /FIELDNAMES /REPLACE.",
    "SAVE TRANSLATE OUTFILE='edge.xlsx' /TYPE=XLS /VERSION=12 /FIELDNAMES /REPLACE.\n\
     GET DATA /TYPE=XLSX /FILE='edge.xlsx'.",
    "MATRIX.\nGET m /VARIABLES=x y z.\nCOMPUTE i = INV(T(m) * m).\nPRINT i.\nEND MATRIX.",
];

/// Commands that read every variable, for the wide dataset.
const WIDE: &[&str] = &[
    "FREQUENCIES ALL /FORMAT=NOTABLE /STATISTICS=MEAN.",
    "DESCRIPTIVES ALL.",
    "LIST /CASES=1.",
    "DISPLAY DICTIONARY.",
];

/// The commands a test runs are valid syntax, so a run reaches the engine.
fn assert_parses<'a>(commands: impl Iterator<Item = &'a &'a str>) {
    let wrong: Vec<String> = commands
        .filter_map(|c| {
            let c = c.replace("'F'", "'file.txt'");
            let (_, errors) = statscraft_syntax::parse(&c);
            let first = errors.first()?;
            Some(format!("{c}: {}", first.message))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The troubles of `commands` run one by one on `data`.
fn run_on(name: &str, data: &Dataset, commands: &[&str]) -> Vec<String> {
    let limit = Duration::from_secs(120);
    let troubles = commands.iter().filter_map(|command| {
        let trouble = run_one(command, Some(data.clone()), false, limit, 0)?;
        Some(format!("{trouble:?} on {name}: {command}"))
    });
    troubles.collect()
}

#[test]
fn procedures_on_edge_data_never_crash() {
    assert_parses(PROCEDURES.iter().chain(WIDE));
    let troubles: Vec<String> = edge_sets()
        .iter()
        .flat_map(|(name, data)| run_on(name, data, PROCEDURES))
        .collect();
    assert!(troubles.is_empty(), "{}", troubles.join("\n"));
}

#[test]
#[ignore = "takes a minute in a debug build; run by hand"]
fn procedures_on_10000_variables_never_crash() {
    // built directly: DATA LIST with 10 000 variables takes a minute in a debug build
    let (_, mut wide) = edge_data(
        "wide",
        &["0 0 3 1 0 w0", "1 2 3 2 1 w1", "2 4 3 1 0 w2"],
        8,
        "",
    );
    for i in 1..=9994 {
        let values = (0..3).map(|k| Some(((i * 7 + k) % 13) as f64)).collect();
        wide.set_numeric_values(&format!("v{i}"), values).unwrap();
    }
    let commands: Vec<&str> = PROCEDURES.iter().take(8).chain(WIDE).copied().collect();
    let troubles = run_on("10 000 variables", &wide, &commands);
    assert!(troubles.is_empty(), "{}", troubles.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Readers: text files and workbooks, damaged or extreme

fn text_files() -> Vec<(&'static str, Vec<u8>)> {
    let mut rng = Rng(0x0dd_f11e);
    let garbage: Vec<u8> = (0..4096).map(|_| rng.next() as u8).collect();
    vec![
        ("empty", Vec::new()),
        ("newlines", b"\n\n\n".to_vec()),
        ("garbage", garbage),
        ("one long line", "1,".repeat(200_000).into_bytes()),
        (
            "open quotes",
            b"id,name\n1,\"Ann\n2,\"Bo\"b\"\n3,'x\n4,\"".to_vec(),
        ),
        ("cr and crlf", b"a,b\r\n1,2\r3,4\r\n\r".to_vec()),
        ("bad utf-8", b"a,b\n\xff\xfe,\xc3\n\xc3".to_vec()),
        (
            "wide",
            format!("{}\n", "1,".repeat(20_000)).repeat(3).into_bytes(),
        ),
        (
            "odd numbers",
            b"1e999,-1e999,nan,inf,1e-400,99999999999999999999999\n-,+,.,..,1e,e5\n".to_vec(),
        ),
        ("byte order mark", "\u{feff}a,b\n1,2\n".as_bytes().to_vec()),
        ("nul bytes", b"a\0b,c\n\0\0\0\n".to_vec()),
        ("utf-16", b"\xff\xfea\0,\0b\0\n\x001\0,\x002\0\n\0".to_vec()),
    ]
}

/// What reads the file `F`.
const TEXT_READERS: &[&str] = &[
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=DELIMITED /DELIMITERS=\",\" /QUALIFIER='\"' \
     /FIRSTCASE=2 /VARIABLES=a F8.2 b A20 c F8.0.",
    "GET DATA /TYPE=TXT /FILE='F' /VARIABLES=a AUTO b AUTO c AUTO.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /VARIABLES=a 0-3 F4.0 b 4-9 A6 c 10-2000 A1991.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /FIXCASE=3 /VARIABLES=/1 a 0-1 F2 /2 b 0-1 A2 /3 c 0-1 F2.",
    "GET DATA /TYPE=TXT /FILE='F' /FIRSTCASE=999999999 /IMPORTCASE=FIRST 999999999 /VARIABLES=a F8.2.",
    "GET DATA /TYPE=TXT /FILE='F' /DELCASE=VARIABLES 3 /DELIMITERS=\"\" /QUALIFIER=\"'\" \
     /MULTIPLESPACES IGNORE=YES /LEADINGSPACES IGNORE=YES /VARIABLES=a F8.2 b A2000 c DATE11.",
    "GET DATA /TYPE=TXT /FILE='F' /ENCODING='UTF-16' /VARIABLES=a A8 b A8.",
    "DATA LIST FILE='F' FREE /a b c.",
    "DATA LIST FILE='F' LIST /a b (A5) c.",
    "DATA LIST FILE='F' FIXED /a 1-3 b 4-2000 (A) c 2001-2010 (DOLLAR,2).",
    "DATA LIST FILE='F' RECORDS=3 /a 1-2 /b 1-2 (A) /c 1-2.",
    "DATA LIST FILE='F' FREE (',') /a (A3) b (DATE11) c (COMMA9.2).",
    "INPUT PROGRAM.\nDATA LIST FILE='F' END=#eof /a 1-2 (A).\nDO IF #eof.\nEND FILE.\nEND IF.\n\
     REREAD.\nDATA LIST FILE='F' /b 1-5.\nEND CASE.\nEND INPUT PROGRAM.\nLIST.",
    "INPUT PROGRAM.\nDATA LIST FILE='F' /n 1.\nREPEATING DATA STARTS=2 /OCCURS=n /DATA=v 1 (A).\n\
     END INPUT PROGRAM.\nLIST.",
    "FILE TYPE MIXED FILE='F' RECORD=#r 1 (A).\nRECORD TYPE 'a'.\nDATA LIST /x 2-3.\nRECORD TYPE OTHER.\n\
     DATA LIST /y 2-4 (A).\nEND FILE TYPE.\nLIST.",
    "FILE TYPE GROUPED FILE='F' RECORD=#r 1 CASE=id 2-3.\nRECORD TYPE 1.\nDATA LIST /x 4-5.\n\
     RECORD TYPE 2.\nDATA LIST /y 4-5.\nEND FILE TYPE.\nLIST.",
    "FILE TYPE NESTED FILE='F' RECORD=#r 1.\nRECORD TYPE 1.\nDATA LIST /x 2-3.\nRECORD TYPE 2.\n\
     DATA LIST /y 2-3.\nEND FILE TYPE.\nLIST.",
    "MATRIX DATA VARIABLES=ROWTYPE_ a b c /FILE='F'.\nLIST.",
    "MATRIX DATA VARIABLES=a b c /FILE='F' /FORMAT=FREE FULL /CONTENTS=MEAN SD N CORR.\nLIST.",
];

#[test]
fn damaged_text_files_never_crash() {
    assert_parses(TEXT_READERS.iter());
    let dir = work_dir();
    let mut troubles = Vec::new();
    for (k, (name, bytes)) in text_files().into_iter().enumerate() {
        let file = format!("robust_text_{k}.txt");
        std::fs::write(dir.join(&file), &bytes).unwrap();
        for reader in TEXT_READERS {
            let syntax = reader.replace("'F'", &format!("'{file}'"));
            if let Some(t) = run_one(&syntax, None, false, Duration::from_secs(60), k) {
                troubles.push(format!("{t:?} on {name}: {syntax}"));
            }
        }
    }
    assert!(troubles.is_empty(), "{}", troubles.join("\n"));
}

/// A workbook whose first sheet's XML is replaced by `sheet`.
fn with_sheet(good: &[u8], sheet: &str) -> Vec<u8> {
    use std::io::{Read, Write};
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(good)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            bytes = sheet.as_bytes().to_vec();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&bytes).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn workbooks() -> Vec<(String, Vec<u8>)> {
    // written once: tests that run at the same time would read each other's half-written file
    static GOOD: OnceLock<Vec<u8>> = OnceLock::new();
    let good = GOOD
        .get_or_init(|| {
            let dir = work_dir();
            let mut session = Session::with_dataset(Some(survey()));
            session.set_base_dir(Some(dir.to_path_buf()));
            let save = "SAVE TRANSLATE OUTFILE='robust_good.xlsx' /TYPE=XLS /VERSION=12 \
                        /FIELDNAMES /REPLACE.";
            session.run(save, None).unwrap();
            std::fs::read(dir.join("robust_good.xlsx")).unwrap()
        })
        .clone();
    let mut rng = Rng(0xb00c_5eed);
    let mut out = vec![("good".to_string(), good.clone())];
    let len = good.len();
    for k in 0..12 {
        let cut = k * len / 12 + rng.below(len / 12);
        out.push((format!("cut at {cut}"), good[..cut].to_vec()));
    }
    for _ in 0..40 {
        let mut copy = good.clone();
        // the central directory at the end holds the sizes and offsets
        let at = if rng.below(2) == 0 {
            len - 1 - rng.below(len.min(600))
        } else {
            rng.below(len)
        };
        copy[at] ^= 1 << rng.below(8);
        out.push((format!("bit flip at {at}"), copy));
    }
    let cell = |r: &str| format!("<c r=\"{r}\" t=\"inlineStr\"><is><t>x</t></is></c>");
    let sheet = |dimension: &str, rows: &str| {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet \
             xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
             <dimension ref=\"{dimension}\"/><sheetData>{rows}</sheetData></worksheet>"
        )
    };
    let long = format!(
        "<row r=\"1\">{}</row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>{}</t></is></c></row>",
        cell("A1"),
        "y".repeat(2_000_000)
    );
    let odd = "<row r=\"1\"><c r=\"A1\"><v>NaN</v></c><c r=\"B1\"><v>1e999</v></c>\
               <c r=\"C1\"><v>-inf</v></c><c r=\"D1\" t=\"e\"><v>#DIV/0!</v></c>\
               <c r=\"E1\" t=\"b\"><v>7</v></c><c r=\"F1\" t=\"s\"><v>99999</v></c></row>\
               <row r=\"2\"><c r=\"A2\" s=\"999\"><v>1e308</v></c>\
               <c r=\"B2\" t=\"d\"><v>not a date</v></c></row>";
    let odd_sheets = [
        (
            "huge dimension",
            sheet(
                "A1:XFD1048576",
                &format!("<row r=\"1\">{}</row>", cell("A1")),
            ),
        ),
        (
            "far cell",
            sheet(
                "A1",
                &format!(
                    "<row r=\"1\">{}</row><row r=\"1048576\">{}</row>",
                    cell("A1"),
                    cell("XFD1048576")
                ),
            ),
        ),
        (
            "beyond the sheet",
            sheet(
                "A1",
                &format!("<row r=\"99999999\">{}</row>", cell("ZZZZ99999999")),
            ),
        ),
        (
            "bad reference",
            sheet("??", &format!("<row r=\"x\">{}</row>", cell("1A"))),
        ),
        ("empty sheet", sheet("A1", "")),
        ("not xml", "<<<<".to_string()),
        ("long string", sheet("A1:A2", &long)),
        ("odd values", sheet("A1:F2", odd)),
    ];
    for (name, xml) in odd_sheets {
        out.push((name.to_string(), with_sheet(&good, &xml)));
    }
    out
}

const WORKBOOK_READERS: &[&str] = &[
    "GET DATA /TYPE=XLSX /FILE='F'.",
    "GET DATA /TYPE=XLSX /FILE='F' /READNAMES=OFF /CELLRANGE=RANGE 'A1:XFD1048576' /HIDDEN IGNORE=YES.",
    "GET DATA /TYPE=XLSX /FILE='F' /SHEET=INDEX 999999999.",
    "GET DATA /TYPE=XLSX /FILE='F' /SHEET=NAME '' /ASSUMEDSTRWIDTH=32767 /DATATYPEMIN PERCENTAGE=51.",
    "GET DATA /TYPE=XLS /FILE='F'.",
];

#[test]
fn far_apart_cells_and_a_whole_sheet_range_give_an_answer() {
    let books: Vec<_> = workbooks()
        .into_iter()
        .filter(|(name, _)| name == "far cell" || name == "huge dimension")
        .collect();
    for (name, bytes) in books {
        let file = format!("robust_{}.xlsx", name.replace(' ', "_"));
        std::fs::write(work_dir().join(&file), bytes).unwrap();
        // the cells A1 and XFD1048576 made calamine ask for 550 GB at once
        let far = outcome(&format!("GET DATA /TYPE=XLSX /FILE='{file}'."));
        // a range of the whole sheet was read position by position, 17 billion of them
        let whole = outcome(&format!(
            "GET DATA /TYPE=XLSX /FILE='{file}' /READNAMES=OFF /CELLRANGE=RANGE 'A1:XFD1048576'."
        ));
        if name == "far cell" {
            assert!(far.unwrap_err().contains("a grid too large to read"));
            assert!(whole.unwrap_err().contains("a grid too large to read"));
        } else {
            assert!(whole.is_ok(), "{whole:?}");
        }
    }
}

#[test]
fn damaged_workbooks_never_crash() {
    assert_parses(WORKBOOK_READERS.iter());
    let dir = work_dir();
    let mut troubles = Vec::new();
    for (k, (name, bytes)) in workbooks().into_iter().enumerate() {
        let file = format!("robust_book_{k}.xlsx");
        std::fs::write(dir.join(&file), &bytes).unwrap();
        for reader in WORKBOOK_READERS {
            let syntax = reader.replace("'F'", &format!("'{file}'"));
            if let Some(t) = run_one(&syntax, None, false, Duration::from_secs(60), k) {
                troubles.push(format!("{t:?} on {name}: {syntax}"));
            }
        }
    }
    assert!(troubles.is_empty(), "{}", troubles.join("\n"));
}
