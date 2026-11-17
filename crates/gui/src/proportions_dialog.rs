//! The Proportions dialog (PROPORTIONS): one proportion against a test value, two related
//! proportions and two independent proportions. Options left as they are add nothing to the
//! syntax, so the command runs with its own defaults.

use crate::analyze_dialogs::keywords;
use crate::reports_dialog::loose_value;
use crate::syntax_text::{names, number};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ProportionsKind {
    OneSample,
    Paired,
    Independent,
}

pub const PROPORTIONS_KINDS: [(ProportionsKind, &str); 3] = [
    (ProportionsKind::OneSample, "One sample"),
    (ProportionsKind::Paired, "Related samples"),
    (ProportionsKind::Independent, "Independent samples"),
];

/// Keyword and label of the tests and intervals of one kind, in the order shown.
type Methods = &'static [(&'static str, &'static str)];

const ONE_TESTS: Methods = &[
    ("EXACT", "Exact"),
    ("MIDP", "Mid-p exact"),
    ("SCORE", "Score"),
    ("SCORECC", "Score, continuity corrected"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
];
const ONE_INTERVALS: Methods = &[
    ("AGRESTI_COULL", "Agresti-Coull"),
    ("ANSCOMBE", "Anscombe"),
    ("CLOPPER_PEARSON", "Clopper-Pearson"),
    ("JEFFREYS", "Jeffreys"),
    ("LOGIT", "Logit"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
    ("WILSON_SCORE", "Wilson score"),
    ("WILSON_SCORECC", "Wilson score, continuity corrected"),
];
const PAIRED_TESTS: Methods = &[
    ("EXACT", "Exact"),
    ("MIDP", "Mid-p exact"),
    ("MCNEMAR", "McNemar"),
    ("MCNEMARCC", "McNemar, continuity corrected"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
];
const PAIRED_INTERVALS: Methods = &[
    ("AGRESTI_MIN", "Agresti-Min"),
    ("BONETT_PRICE", "Bonett-Price"),
    ("NEWCOMBE", "Newcombe"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
];
const INDEPENDENT_TESTS: Methods = &[
    ("HAUCK_ANDERSON", "Hauck-Anderson"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
    ("WALDH0", "Wald, variance under no difference"),
    ("WALDH0CC", "Wald, variance under no difference, corrected"),
];
const INDEPENDENT_INTERVALS: Methods = &[
    ("AGRESTI_CAFFO", "Agresti-Caffo"),
    ("BROWN_LI_JEFFREYS", "Brown-Li-Jeffreys"),
    ("HAUCK_ANDERSON", "Hauck-Anderson"),
    ("NEWCOMBE", "Newcombe"),
    ("NEWCOMBECC", "Newcombe, continuity corrected"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald, continuity corrected"),
];

impl ProportionsKind {
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub fn tests(self) -> Methods {
        [ONE_TESTS, PAIRED_TESTS, INDEPENDENT_TESTS][self.index()]
    }

    pub fn intervals(self) -> Methods {
        [ONE_INTERVALS, PAIRED_INTERVALS, INDEPENDENT_INTERVALS][self.index()]
    }

    /// What the command runs when the dialog says nothing, as indices of `tests`.
    fn default_tests(self) -> &'static [usize] {
        [&[1, 2][..], &[1, 2], &[3]][self.index()]
    }

    /// The same for `intervals`.
    fn default_intervals(self) -> &'static [usize] {
        [&[0, 3, 7][..], &[1, 2], &[0, 3]][self.index()]
    }
}

/// How the two groups of independent samples are picked from the grouping variable.
pub const PROPORTIONS_GROUPS: [(&str, &str); 4] = [
    ("", "The values 1 and 2"),
    ("LEVEL", "Two values"),
    ("MIDPOINT", "Below and above the middle of its range"),
    ("CUTPOINT", "Below and from a cut point"),
];

/// What counts as a success.
pub const PROPORTIONS_SUCCESS: [(&str, &str); 5] = [
    ("", "The highest value"),
    ("FIRST", "The lowest value"),
    ("LEVEL", "Listed values"),
    ("MIDPOINT", "From the middle of the range up"),
    ("CUTPOINT", "From a cut point up"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Proportions {
    pub kind: ProportionsKind,
    pub variables: Vec<String>,
    /// Related samples: the variables each one is paired with; empty pairs the list with itself.
    pub with: Vec<String>,
    /// Related samples with a second list: the n-th with the n-th, not every combination.
    pub in_order: bool,
    /// Independent samples: the grouping variable (one).
    pub group: Vec<String>,
    /// An index into `PROPORTIONS_GROUPS`.
    pub groups: usize,
    pub group_values: [String; 2],
    pub cutpoint: String,
    pub test_value: String,
    /// One flag per test of each kind, in the order of `tests`.
    pub tests: [[bool; 6]; 3],
    pub intervals: [[bool; 9]; 3],
    /// An index into `PROPORTIONS_SUCCESS`.
    pub success: usize,
    pub success_values: String,
    pub success_cutpoint: String,
    pub level: String,
    pub listwise: bool,
    pub include_missing: bool,
}

fn defaults<const N: usize>(indices: &[usize]) -> [bool; N] {
    let mut flags = [false; N];
    for &i in indices {
        flags[i] = true;
    }
    flags
}

impl Default for Proportions {
    fn default() -> Self {
        Self::new(ProportionsKind::OneSample)
    }
}

impl Proportions {
    pub fn new(kind: ProportionsKind) -> Self {
        let kinds = [
            ProportionsKind::OneSample,
            ProportionsKind::Paired,
            ProportionsKind::Independent,
        ];
        Self {
            kind,
            variables: Vec::new(),
            with: Vec::new(),
            in_order: false,
            group: Vec::new(),
            groups: 0,
            group_values: ["1".into(), "2".into()],
            cutpoint: String::new(),
            test_value: "0.5".into(),
            tests: kinds.map(|k| defaults(k.default_tests())),
            intervals: kinds.map(|k| defaults(k.default_intervals())),
            success: 0,
            success_values: String::new(),
            success_cutpoint: String::new(),
            level: "95".into(),
            listwise: false,
            include_missing: false,
        }
    }

    pub fn title(&self) -> &'static str {
        "Proportions"
    }

    pub fn syntax(&self) -> Result<String, String> {
        let k = self.kind;
        let vars = names(&self.variables, "Variables")?;
        let mut text = match k {
            ProportionsKind::OneSample => format!("PROPORTIONS /ONESAMPLE {vars}"),
            ProportionsKind::Paired => {
                let mut t = format!("PROPORTIONS /PAIREDSAMPLES {vars}");
                if !self.with.is_empty() {
                    t.push_str(&format!(" WITH {}", self.with.join(" ")));
                    if self.in_order {
                        if self.with.len() != self.variables.len() {
                            return Err(
                                "pairing in order needs as many variables on each side".into()
                            );
                        }
                        t.push_str(" (PAIRED)");
                    }
                } else if self.variables.len() < 2 {
                    return Err("add a second variable, or a list to pair with".into());
                }
                t
            }
            ProportionsKind::Independent => {
                let [group] = self.group.as_slice() else {
                    return Err("choose one grouping variable".into());
                };
                format!("PROPORTIONS /INDEPENDENTSAMPLES {vars} BY {group}")
            }
        };
        if k == ProportionsKind::OneSample {
            match self.test_value.trim().parse::<f64>() {
                Ok(p) if p > 0.0 && p < 1.0 => {
                    if p != 0.5 {
                        text.push_str(&format!(" TESTVAL={}", number(p)));
                    }
                }
                _ => return Err("the test value must be a number between 0 and 1".into()),
            }
        }
        if k == ProportionsKind::Independent {
            text.push_str(&self.select()?);
        }
        let tests = &self.tests[k.index()];
        let intervals = &self.intervals[k.index()];
        for (flags, methods, defaults, keyword) in [
            (&tests[..], k.tests(), k.default_tests(), "TESTTYPES"),
            (
                &intervals[..],
                k.intervals(),
                k.default_intervals(),
                "CITYPES",
            ),
        ] {
            let chosen = keywords(methods, flags);
            let standard: Vec<&str> = defaults.iter().map(|&i| methods[i].0).collect();
            if chosen == standard {
                continue;
            }
            let words = if chosen.is_empty() {
                "NONE".to_string()
            } else {
                chosen.join(" ")
            };
            text.push_str(&format!(" {keyword}={words}"));
        }
        if let Some(success) = self.success_value()? {
            text.push_str(&format!("\n  /SUCCESSVALUE={success}"));
        }
        let mut missing = Vec::new();
        if self.listwise {
            missing.push("SCOPE=LISTWISE");
        }
        if self.include_missing {
            missing.push("USERMISSING=INCLUDE");
        }
        if !missing.is_empty() {
            text.push_str(&format!("\n  /MISSING {}", missing.join(" ")));
        }
        match self.level.trim().parse::<f64>() {
            Ok(p) if p > 0.0 && p < 100.0 => {
                if p != 95.0 {
                    text.push_str(&format!("\n  /CRITERIA CILEVEL={}", number(p)));
                }
            }
            _ => return Err("the confidence level must be a percentage between 0 and 100".into()),
        }
        text.push('.');
        Ok(text)
    }

    /// ` SELECT=...` for the groups of independent samples, empty for the values 1 and 2.
    fn select(&self) -> Result<String, String> {
        Ok(match PROPORTIONS_GROUPS[self.groups].0 {
            "LEVEL" => {
                let [a, b] = &self.group_values;
                let (a, b) = (a.trim(), b.trim());
                if a.is_empty() || b.is_empty() {
                    return Err("fill in both group values".into());
                }
                format!(" SELECT=LEVEL({}, {})", loose_value(a), loose_value(b))
            }
            "MIDPOINT" => " SELECT=MIDPOINT".into(),
            "CUTPOINT" => format!(" SELECT=CUTPOINT({})", cut(&self.cutpoint)?),
            _ => String::new(),
        })
    }

    fn success_value(&self) -> Result<Option<String>, String> {
        Ok(match PROPORTIONS_SUCCESS[self.success].0 {
            "FIRST" => Some("FIRST".into()),
            "LEVEL" => {
                let values: Vec<String> = self
                    .success_values
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|w| !w.is_empty())
                    .map(loose_value)
                    .collect();
                if values.is_empty() {
                    return Err("list the values that count as a success".into());
                }
                Some(format!("LEVEL({})", values.join(", ")))
            }
            "MIDPOINT" => Some("MIDPOINT".into()),
            "CUTPOINT" => Some(format!("CUTPOINT({})", cut(&self.success_cutpoint)?)),
            _ => None,
        })
    }
}

fn cut(text: &str) -> Result<String, String> {
    match text.trim().parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(number(n)),
        _ => Err("the cut point must be a number".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::{runs, v};

    #[test]
    fn one_sample_writes_only_what_changed() {
        let mut d = Proportions::new(ProportionsKind::OneSample);
        assert_eq!(
            d.syntax().unwrap_err(),
            "add at least one variable to Variables"
        );
        d.variables = v(&["gender"]);
        assert_eq!(d.syntax().unwrap(), "PROPORTIONS /ONESAMPLE gender.");
        d.test_value = "0.4".into();
        d.tests[0] = [true, false, false, false, true, false];
        d.intervals[0] = [false; 9];
        d.success = 1;
        d.level = "90".into();
        d.listwise = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PROPORTIONS /ONESAMPLE gender TESTVAL=0.4 TESTTYPES=EXACT WALD CITYPES=NONE\n  \
             /SUCCESSVALUE=FIRST\n  /MISSING SCOPE=LISTWISE\n  /CRITERIA CILEVEL=90."
        );
        assert!(runs(&syntax) >= 1);
        d.test_value = "1".into();
        assert_eq!(
            d.syntax().unwrap_err(),
            "the test value must be a number between 0 and 1"
        );
    }

    #[test]
    fn paired_and_independent_samples_write_their_own_parts() {
        let mut d = Proportions::new(ProportionsKind::Paired);
        d.variables = v(&["q5_1"]);
        assert_eq!(
            d.syntax().unwrap_err(),
            "add a second variable, or a list to pair with"
        );
        d.with = v(&["q5_2"]);
        d.in_order = true;
        assert_eq!(
            d.syntax().unwrap(),
            "PROPORTIONS /PAIREDSAMPLES q5_1 WITH q5_2 (PAIRED)."
        );
        let mut d = Proportions::new(ProportionsKind::Independent);
        d.variables = v(&["q2_recommend"]);
        assert_eq!(d.syntax().unwrap_err(), "choose one grouping variable");
        d.group = v(&["gender"]);
        d.groups = 1;
        d.group_values = ["1".into(), "2".into()];
        d.tests[2] = [true, false, false, true, false, false];
        d.success = 2;
        d.success_values = "4 5".into();
        d.include_missing = true;
        let syntax = d.syntax().unwrap();
        assert_eq!(
            syntax,
            "PROPORTIONS /INDEPENDENTSAMPLES q2_recommend BY gender SELECT=LEVEL(1, 2) \
             TESTTYPES=HAUCK_ANDERSON WALDH0\n  /SUCCESSVALUE=LEVEL(4, 5)\n  \
             /MISSING USERMISSING=INCLUDE."
        );
        assert!(runs(&syntax) >= 1);
        d.groups = 3;
        d.cutpoint = "x".into();
        assert_eq!(d.syntax().unwrap_err(), "the cut point must be a number");
    }
}
