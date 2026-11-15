//! MULTIPLE IMPUTATION on an invented allotment garden survey of 40 plots: the gardener's
//! age, the soil type (1 loam, 2 sand, 3 clay), weekly hours of work and the yield in kg.
//! Hours and yield are missing together for some plots, yield alone for others, and the soil
//! type for three. Imputations are random: the tests fix SET SEED and check what holds
//! whatever the draws (observed values kept, bounds, rounding, categories), the long-run
//! behaviour of the draws against expectations computed in a general statistics package, and
//! pooled results against a published implementation of Rubin's rules.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const GARDEN: &str = "SET SEED=20261115.
DATA LIST LIST /plot age soil hours yield.
BEGIN DATA
101 48 2 5.5 34.4
102 48 3 6.4 31.6
103 41 2 6.6 31.0
104 24 1 . .
105 52 1 9.8 51.3
106 44 1 5.1 .
107 61 1 7.5 46.6
108 51 2 6.1 32.1
109 70 2 . .
110 66 1 8.7 54.0
111 68 1 7.6 44.6
112 63 . 5.4 33.0
113 74 1 6.4 40.7
114 66 1 4.5 34.3
115 55 2 5.8 34.9
116 28 2 . .
117 58 2 2.9 19.6
118 65 2 5.9 35.6
119 28 1 4.0 23.1
120 78 1 9.2 .
121 50 1 4.3 27.6
122 78 1 8.0 44.3
123 51 2 . .
124 50 2 7.4 42.6
125 58 . 6.9 39.3
126 48 2 6.1 29.2
127 74 2 6.7 30.3
128 37 3 5.0 .
129 35 2 4.5 26.7
130 39 3 4.2 23.9
131 45 1 . .
132 31 2 2.6 16.7
133 59 2 4.8 31.5
134 41 . 7.0 39.0
135 33 1 2.5 22.5
136 35 2 4.4 20.7
137 28 1 . .
138 46 1 3.9 20.4
139 43 2 6.9 44.0
140 63 1 7.6 36.3
END DATA.
VARIABLE LEVEL soil (NOMINAL) /plot age hours yield (SCALE).
DATASET DECLARE filled.
";

/// Runs `source` after `data`; the tables of the last command, the warnings and the engine.
fn run_on(data: &str, source: &str) -> (Vec<Table>, Vec<String>, Engine) {
    let (commands, errors) = parse(&format!("{data}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    let warnings = engine.warnings().to_vec();
    (tables, warnings, engine)
}

fn run(source: &str) -> (Vec<Table>, Vec<String>, Engine) {
    run_on(GARDEN, source)
}

/// The error of the last command.
fn fails(source: &str) -> String {
    let (commands, errors) = parse(&format!("{GARDEN}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands[..commands.len() - 1] {
        engine.execute(&command.command).unwrap();
    }
    let last = &commands.last().unwrap().command;
    engine.execute(last).unwrap_err().0
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        Cell::Empty => f64::NAN,
        other => panic!("{other:?}"),
    }
}

fn words(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("{other:?}"),
    }
}

/// The values of a numeric variable of the active dataset.
fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn close(got: f64, want: f64, tolerance: f64) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1.0),
        "{got} against {want}"
    );
}

const IMPUTE: &str = "MULTIPLE IMPUTATION age soil hours yield";

#[test]
fn missing_value_summaries() {
    let (tables, _, _) = run(&format!(
        "{IMPUTE} /IMPUTE METHOD=NONE /MISSINGSUMMARIES OVERALL VARIABLES(MINPCTMISSING=10) PATTERNS."
    ));
    // 3 of 4 variables, 12 of 40 plots and 18 of 160 values are incomplete
    let overall = find(&tables, "Overall Summary of Missing Values");
    let nums = |r: usize| overall.rows[r][1..].iter().map(num).collect::<Vec<_>>();
    assert_eq!(nums(0), [1.0, 25.0, 3.0, 75.0, 4.0]);
    assert_eq!(nums(1), [28.0, 70.0, 12.0, 30.0, 40.0]);
    assert_eq!(nums(2), [142.0, 88.75, 18.0, 11.25, 160.0]);
    // yield (9 missing, 22.5%) before hours (6, 15%); soil has 7.5%, under 10
    let summary = find(&tables, "Variable Summary");
    assert_eq!(summary.rows.len(), 2);
    assert_eq!(words(&summary.rows[0][0]), "yield");
    assert_eq!(num(&summary.rows[0][2]), 22.5);
    assert_eq!(num(&summary.rows[1][3]), 34.0);
    close(num(&summary.rows[1][4]), 5.888235294117647, 1e-12);
    close(num(&summary.rows[1][5]), 1.8058431600547642, 1e-12);
    // patterns: columns from fewest to most missing, the complete pattern first
    let patterns = find(&tables, "Missing Value Patterns");
    assert_eq!(patterns.columns[1..5], ["age", "soil", "hours", "yield"]);
    let shape: Vec<(String, f64)> = patterns
        .rows
        .iter()
        .map(|r| {
            let marks: String = r[1..5]
                .iter()
                .map(|c| if *c == Cell::Empty { '.' } else { 'X' })
                .collect();
            (marks, num(&r[5]))
        })
        .collect();
    let want = [("....", 28.0), (".X..", 3.0), ("...X", 3.0), ("..XX", 6.0)];
    assert_eq!(shape.len(), want.len());
    for ((m, n), (wm, wn)) in shape.iter().zip(want) {
        assert_eq!((m.as_str(), *n), (wm, wn));
    }
}

#[test]
fn fcs_fills_every_missing_value_and_keeps_the_observed_ones() {
    let (tables, warnings, mut engine) = run(&format!(
        "{IMPUTE} /IMPUTE NIMPUTATIONS=3 /IMPUTATIONSUMMARIES MODELS DESCRIPTIVES(yield soil) \
         /OUTFILE IMPUTATIONS=filled."
    ));
    assert!(warnings.is_empty(), "{warnings:?}");
    // soil missing on plots that have hours and yield: not monotone, so AUTO takes FCS
    let results = find(&tables, "Imputation Results");
    assert_eq!(
        words(&results.rows[0][1]),
        "Fully Conditional Specification"
    );
    assert_eq!(num(&results.rows[1][1]), 10.0);
    assert_eq!(words(&results.rows[2][1]), "soil hours yield");
    assert_eq!(words(&results.rows[3][1]), "age");
    assert_eq!(words(&results.rows[4][1]), "age soil hours yield");
    let models = find(&tables, "Imputation Models");
    let model = |r: usize| -> Vec<String> {
        models.rows[r][..3]
            .iter()
            .map(|c| words(c).to_string())
            .collect()
    };
    assert_eq!(model(0), ["soil", "Logistic Regression", "age hours yield"]);
    assert_eq!(model(1), ["hours", "Linear Regression", "age soil yield"]);
    assert_eq!(
        models.rows[2][3..].iter().map(num).collect::<Vec<_>>(),
        [9.0, 27.0]
    );
    // descriptives only for the listed variables; the original data are the observed values
    assert!(
        tables
            .iter()
            .all(|t| t.title != "hours: Descriptive Statistics")
    );
    let yield_ = find(&tables, "yield: Descriptive Statistics");
    assert_eq!(yield_.rows.len(), 7);
    close(num(&yield_.rows[0][3]), 33.60645161290323, 1e-12);
    assert_eq!(num(&yield_.rows[1][2]), 9.0);
    assert_eq!(num(&yield_.rows[4][2]), 40.0);
    let soil = find(&tables, "soil: Descriptive Statistics");
    assert_eq!(soil.rows.len(), 21);
    for imputation in 0..3 {
        let imputed: f64 = (0..3)
            .map(|k| num(&soil.rows[3 + 3 * imputation + k][3]))
            .sum();
        assert_eq!(imputed, 3.0);
    }

    engine
        .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
        .unwrap();
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 160);
    assert_eq!(data.variable_at(0).name(), "Imputation_");
    let split = data.split_file().unwrap();
    assert_eq!(
        (split.variables.clone(), split.layered),
        (vec!["Imputation_".to_string()], true)
    );
    let imputation = column(&engine, "Imputation_");
    let (soil, hours, yield_) = (
        column(&engine, "soil"),
        column(&engine, "hours"),
        column(&engine, "yield"),
    );
    for case in 0..40 {
        assert_eq!(imputation[case], Some(0.0));
        for l in 1..=3 {
            let at = 40 * l + case;
            assert_eq!(imputation[at], Some(l as f64));
            for values in [&soil, &hours, &yield_] {
                // observed values are kept, missing ones filled
                match values[case] {
                    Some(v) => assert_eq!(values[at], Some(v)),
                    None => assert!(values[at].is_some()),
                }
            }
            assert!([1.0, 2.0, 3.0].contains(&soil[at].unwrap()));
        }
    }
    // the original data keep their missing values
    assert_eq!(hours[3], None);
}

#[test]
fn the_same_seed_gives_the_same_imputations() {
    let filled = |seed: &str| {
        let (_, _, mut engine) = run(&format!(
            "SET SEED={seed}.\n{IMPUTE} /IMPUTE NIMPUTATIONS=2 /OUTFILE IMPUTATIONS=filled."
        ));
        engine
            .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
            .unwrap();
        column(&engine, "yield")
    };
    assert_eq!(filled("123"), filled("123"));
    assert_ne!(filled("123"), filled("124"));
}

#[test]
fn auto_takes_monotone_for_a_monotone_pattern() {
    // without the soil gaps, hours missing implies yield missing: monotone
    let (tables, _, _) = run(
        "MULTIPLE IMPUTATION age hours yield /IMPUTATIONSUMMARIES MODELS \
         /OUTFILE IMPUTATIONS=filled.",
    );
    let results = find(&tables, "Imputation Results");
    assert_eq!(words(&results.rows[0][1]), "Monotone");
    let models = find(&tables, "Imputation Models");
    // each variable from those before it
    assert_eq!(words(&models.rows[0][2]), "age");
    assert_eq!(words(&models.rows[1][2]), "age hours");
    // an explicit MONOTONE follows the list order and refuses a pattern that is not monotone
    let message = fails(
        "MULTIPLE IMPUTATION age yield hours /IMPUTE METHOD=MONOTONE /OUTFILE IMPUTATIONS=filled.",
    );
    assert!(message.contains("monotone pattern"), "{message}");
    let message = fails(&format!(
        "{IMPUTE} /IMPUTE METHOD=MONOTONE /OUTFILE IMPUTATIONS=filled."
    ));
    assert!(message.contains("monotone pattern"), "{message}");
}

#[test]
fn constraints_bound_and_round_and_set_roles() {
    let (tables, warnings, mut engine) = run(
        "MULTIPLE IMPUTATION age soil hours yield /IMPUTE METHOD=FCS NIMPUTATIONS=4 MAXITER=3 \
         /CONSTRAINTS hours yield (MIN=1 RND=0.5) /CONSTRAINTS hours (MAX=9) \
         /CONSTRAINTS soil (ROLE=IND) /CONSTRAINTS yield (ROLE=DEP) \
         /OUTFILE IMPUTATIONS=filled FCSITERATIONS='trace.sav'.",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let constraints = find(&tables, "Imputation Constraints");
    let row = |r: usize| constraints.rows[r].clone();
    assert_eq!(words(&row(0)[0]), "soil");
    assert_eq!(words(&row(0)[1]), "Predictor only");
    assert_eq!(words(&row(1)[1]), "Dependent and Predictor");
    assert_eq!(
        row(1)[2..].iter().map(num).collect::<Vec<_>>(),
        [1.0, 9.0, 0.5]
    );
    assert_eq!(words(&row(2)[1]), "Dependent only");
    // yield predicts nothing; soil predicts but is not shown as imputed
    let models = find(&tables, "Imputation Models");
    assert_eq!(words(&models.rows[0][2]), "age hours");
    assert_eq!(words(&models.rows[1][2]), "age soil");
    assert_eq!(words(&models.rows[2][2]), "age soil hours");
    let results = find(&tables, "Imputation Results");
    assert_eq!(words(&results.rows[2][1]), "hours yield");
    engine
        .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
        .unwrap();
    let (soil, hours, yield_) = (
        column(&engine, "soil"),
        column(&engine, "hours"),
        column(&engine, "yield"),
    );
    for case in 0..40 {
        for l in 1..=4 {
            let at = 40 * l + case;
            // the predictor-only variable keeps its missing values
            assert_eq!(soil[at], soil[case]);
            if hours[case].is_none() {
                let h = hours[at].unwrap();
                assert!((1.0..=9.0).contains(&h) && (2.0 * h).fract() == 0.0, "{h}");
            }
            if yield_[case].is_none() {
                let y = yield_[at].unwrap();
                assert!(y >= 1.0 && (2.0 * y).fract() == 0.0, "{y}");
            }
        }
    }
    let _ = std::fs::remove_file("trace.sav");
}

#[test]
fn iteration_history() {
    let (_, _, mut engine) = run("DATASET DECLARE trace.\n\
         MULTIPLE IMPUTATION age soil hours yield /IMPUTE METHOD=FCS NIMPUTATIONS=2 MAXITER=4 \
         /OUTFILE IMPUTATIONS=filled FCSITERATIONS=trace.");
    engine
        .execute(&parse("DATASET ACTIVATE trace.").0[0].command)
        .unwrap();
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(
        names,
        [
            "Imputation_",
            "Iteration_",
            "SummaryStatistic_",
            "hours",
            "yield"
        ]
    );
    // a mean and a standard deviation row per imputation and iteration
    assert_eq!(data.case_count(), 16);
    assert_eq!(
        column(&engine, "Iteration_")[..4],
        [Some(1.0), Some(1.0), Some(2.0), Some(2.0)]
    );
    assert_eq!(column(&engine, "Imputation_")[8], Some(2.0));
    let stat = column(&engine, "SummaryStatistic_");
    assert_eq!(stat[..2], [Some(1.0), Some(2.0)]);
    assert!(column(&engine, "yield").iter().all(Option::is_some));
}

#[test]
fn predictive_mean_matching_takes_observed_values() {
    let (tables, _, mut engine) = run(
        "MULTIPLE IMPUTATION age hours yield /IMPUTE METHOD=MONOTONE SCALEMODEL=PMM(3) NIMPUTATIONS=3 \
         /OUTFILE IMPUTATIONS=filled.",
    );
    let models = find(&tables, "Imputation Models");
    assert_eq!(words(&models.rows[0][1]), "Predictive Mean Matching (3)");
    engine
        .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
        .unwrap();
    let yield_ = column(&engine, "yield");
    let observed: Vec<f64> = yield_[..40].iter().flatten().copied().collect();
    for value in &yield_[40..] {
        assert!(observed.contains(&value.unwrap()));
    }
}

/// Invented: a straight line with noise, the value at x = 11 missing.
const LINE: &str = "SET SEED=4242.
DATA LIST LIST /x y.
BEGIN DATA
1 2.19
2 3.61
3 3.23
4 3.62
5 3.38
6 4.74
7 6.83
8 6.51
9 7.74
10 7.3
11 .
12 8.22
13 6.5
14 10.03
15 10.11
16 10.6
17 8.47
18 8.91
19 10.43
20 11.44
21 12.87
END DATA.
VARIABLE LEVEL x y (SCALE).
DATASET DECLARE filled.
";

#[test]
fn linear_draws_follow_the_posterior_predictive_distribution() {
    let (_, _, mut engine) = run_on(
        LINE,
        "MULTIPLE IMPUTATION x y /IMPUTE NIMPUTATIONS=4000 /OUTFILE IMPUTATIONS=filled.\n\
         DATASET ACTIVATE filled.",
    );
    let y = column(&engine, "y");
    let draws: Vec<f64> = (1..=4000).map(|l| y[21 * l + 10].unwrap()).collect();
    let mean = draws.iter().sum::<f64>() / 4000.0;
    let var = draws.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / 3999.0;
    // a general statistics package: prediction 7.3365, predictive variance
    // RSS / (df - 2) (1 + h) = 1.2991
    assert!(
        (mean - 7.3365).abs() < 4.0 * (1.2991_f64 / 4000.0).sqrt(),
        "{mean}"
    );
    close(var, 1.2991062571, 0.08);
    let _ = &mut engine;
}

/// Invented: a binary outcome that grows likelier with x; two outcomes missing.
const BINARY: &str = "SET SEED=99.
DATA LIST LIST /x y.
BEGIN DATA
4.6 1
1.4 0
0.1 0
9.0 .
5.5 0
0.4 0
7.0 1
7.3 1
2.4 1
5.1 0
5.9 0
3.8 0
1.0 .
6.7 0
4.2 1
2.7 1
2.8 0
2.0 0
8.8 1
3.8 0
0.2 0
9.1 1
6.0 1
5.8 0
2.0 1
0.8 0
0.2 1
5.0 0
4.2 1
6.4 1
5.8 1
6.1 0
0.5 0
6.1 1
2.7 0
10.0 1
7.1 0
2.4 0
0.4 0
6.7 1
END DATA.
VARIABLE LEVEL x (SCALE) /y (NOMINAL).
DATASET DECLARE filled.
";

#[test]
fn logistic_draws_follow_the_fitted_probabilities() {
    let (_, _, engine) = run_on(
        BINARY,
        "MULTIPLE IMPUTATION x y /IMPUTE NIMPUTATIONS=3000 /OUTFILE IMPUTATIONS=filled.\n\
         DATASET ACTIVATE filled.",
    );
    let y = column(&engine, "y");
    let share =
        |case: usize| (1..=3000).filter(|l| y[40 * l + case] == Some(1.0)).count() as f64 / 3000.0;
    // a general statistics package: the mean of the fitted probability over the normal
    // approximation of the posterior, 0.7790 at x = 9 and 0.2157 at x = 1
    assert!((share(3) - 0.779010).abs() < 0.025, "{}", share(3));
    assert!((share(12) - 0.215656).abs() < 0.025, "{}", share(12));
}

#[test]
fn errors_and_warnings() {
    let message = fails(&format!(
        "{IMPUTE} /CONSTRAINTS hours (MIN=50 MAX=60) /IMPUTE MAXCASEDRAWS=2 MAXPARAMDRAWS=1 \
         METHOD=FCS /OUTFILE IMPUTATIONS=filled."
    ));
    assert!(
        message.contains("no values within its MIN and MAX"),
        "{message}"
    );
    let message = fails(&format!(
        "{IMPUTE} /IMPUTE METHOD=FCS INTERACTIONS=TWOWAY MAXMODELPARAM=5 /OUTFILE IMPUTATIONS=filled."
    ));
    assert!(message.contains("more than MAXMODELPARAM=5"), "{message}");
    let message = fails(
        "COMPUTE Imputation_ = 1.\nMULTIPLE IMPUTATION age hours /OUTFILE IMPUTATIONS=filled.",
    );
    assert!(
        message.contains("already have a variable named Imputation_"),
        "{message}"
    );
    let message = fails("MULTIPLE IMPUTATION age age /OUTFILE IMPUTATIONS=filled.");
    assert!(message.contains("two or more"), "{message}");
    // AUTO ignores the model settings with a warning; NONE with nothing to show warns
    let (_, warnings, _) = run(&format!(
        "{IMPUTE} /IMPUTE SCALEMODEL=PMM MAXITER=3 /OUTFILE IMPUTATIONS=filled."
    ));
    assert_eq!(
        warnings,
        [
            "SCALEMODEL is ignored with METHOD=AUTO",
            "MAXITER is ignored with METHOD=AUTO"
        ]
    );
    let (_, warnings, _) = run(&format!(
        "{IMPUTE} /IMPUTE METHOD=NONE /MISSINGSUMMARIES NONE."
    ));
    assert!(warnings[0].contains("did nothing"), "{warnings:?}");
    // MAXPCTMISSING leaves yield (22.5% missing) out
    let (tables, _, _) = run(&format!(
        "{IMPUTE} /IMPUTE MAXPCTMISSING=20 /OUTFILE IMPUTATIONS=filled."
    ));
    let results = find(&tables, "Imputation Results");
    assert_eq!(words(&results.rows[2][1]), "soil hours");
    assert_eq!(
        words(&results.rows[3][0]),
        "Not Imputed (Too Many Missing Values)"
    );
    assert_eq!(words(&results.rows[3][1]), "yield");
}

#[test]
fn weights_and_split_files() {
    // a weight that rounds to 0 leaves the plot out; SPLIT FILE imputes each group apart and
    // stacks them imputation by imputation
    let (tables, _, mut engine) = run(
        "COMPUTE w = 1.\nIF (plot = 101) w = 0.4.\nCOMPUTE half = plot > 120.\nWEIGHT BY w.\n\
         SORT CASES BY half.\nSPLIT FILE BY half.\n\
         MULTIPLE IMPUTATION age hours yield /IMPUTE NIMPUTATIONS=2 /OUTFILE IMPUTATIONS=filled.",
    );
    let overall: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Overall Summary of Missing Values")
        .collect();
    assert!(!overall.is_empty());
    engine
        .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
        .unwrap();
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 120);
    let split = data.split_file().unwrap();
    assert_eq!(split.variables, ["Imputation_", "half"]);
    let imputation = column(&engine, "Imputation_");
    let half = column(&engine, "half");
    assert_eq!((imputation[39], imputation[40]), (Some(0.0), Some(1.0)));
    assert_eq!(
        (half[0], half[39], half[40]),
        (Some(0.0), Some(1.0), Some(0.0))
    );
}

/// Invented stacked data as MULTIPLE IMPUTATION writes them: the original cases and three
/// imputations of the last two.
const STACKED: &str = "DATA LIST LIST /Imputation_ ph moisture.
BEGIN DATA
0 4.1 4.0
0 5.0 2.5
0 6.2 5.2
0 3.9 6.6
0 . .
0 . .
1 4.1 4.0
1 5.0 2.5
1 6.2 5.2
1 3.9 6.6
1 5.5 3.0
1 4.8 2.0
2 4.1 4.0
2 5.0 2.5
2 6.2 5.2
2 3.9 6.6
2 7.1 3.9
2 6.0 4.4
3 4.1 4.0
3 5.0 2.5
3 6.2 5.2
3 3.9 6.6
3 4.4 1.8
3 5.2 3.1
END DATA.
SPLIT FILE LAYERED BY Imputation_.
";

#[test]
fn pooled_means_follow_rubins_rules() {
    let (tables, _, _) = run_on(STACKED, "DESCRIPTIVES ph moisture.");
    let pooled = find(&tables, "Pooled Descriptive Statistics");
    // a published implementation of Rubin's rules: estimate, standard error, df, interval,
    // fraction of missing information, relative increase in variance, relative efficiency
    let want = [
        [
            5.03333333333,
            0.545180907793,
            10.9479256092,
            3.83270152118,
            6.23396514549,
            0.50951767459,
            0.746463547334,
            0.854818319258,
        ],
        [
            4.06111111111,
            0.769796349532,
            36.5379275276,
            2.50068945439,
            5.62153276783,
            0.272710359868,
            0.305416173731,
            0.916671403858,
        ],
    ];
    for (row, want) in pooled.rows.iter().zip(want) {
        for (got, want) in row[1..].iter().zip(want) {
            close(num(got), want, 1e-9);
        }
    }
    // the original data and each imputation keep their own rows
    let layered = find(&tables, "Descriptive Statistics");
    assert_eq!(layered.rows.len(), 4 * 4);
    // SET MIOUTPUT leaves out the original data and the diagnostics
    let (tables, _, _) = run_on(
        STACKED,
        "SET MIOUTPUT=[IMPUTED POOLED].\nDESCRIPTIVES ph moisture.",
    );
    assert_eq!(find(&tables, "Descriptive Statistics").rows.len(), 3 * 4);
    assert_eq!(
        find(&tables, "Pooled Descriptive Statistics").columns.len(),
        6
    );
    let (tables, _, _) = run_on(STACKED, "SET MIOUTPUT=[OBSERVED].\nDESCRIPTIVES ph.");
    assert!(
        tables
            .iter()
            .all(|t| t.title != "Pooled Descriptive Statistics")
    );
}

#[test]
fn pooled_frequencies_average_the_imputations() {
    let (tables, _, _) = run_on(
        STACKED,
        "RECODE ph (LO THRU 5=1) (ELSE=2) INTO band.\nFREQUENCIES band.",
    );
    let pooled = find(&tables, "Pooled Frequencies");
    // band 1 in imputations 1, 2, 3: 4, 3, 4 plots of 6
    let first = &pooled.rows[0];
    assert_eq!(num(&first[2]), 1.0);
    close(num(&first[4]), 11.0 / 3.0, 1e-12);
    close(num(&first[5]), 100.0 * 11.0 / 18.0, 1e-12);
    assert_eq!(pooled.rows.len(), 2);
}

/// An invented beekeeping survey of 14 hives, stacked as the original data and three
/// imputations: the apiary site (1, 2), the mean temperature, the honey yield, the mite count
/// and whether the colony swarmed. Temperature is missing for two hives, honey for three.
const HIVES: &str = "DATA LIST LIST /Imputation_ site temp honey mites swarm.
BEGIN DATA
0 1 7.1 10.0 8.0 1
0 2 6.2 9.5 5.3 0
0 1 2.8 . 9.1 1
0 2 6.5 9.9 6.2 1
0 1 . 6.2 10.0 0
0 2 7.2 11.7 5.9 1
0 1 2.0 3.0 9.0 0
0 2 6.5 . 7.6 1
0 1 5.8 9.4 6.9 1
0 2 . 9.5 7.9 1
0 1 5.6 10.5 5.8 0
0 2 7.7 . 6.6 1
0 1 2.5 5.4 8.4 0
0 2 4.8 7.2 8.0 1
1 1 7.1 10.0 8.0 1
1 2 6.2 9.5 5.3 0
1 1 2.8 4.3 9.1 1
1 2 6.5 9.9 6.2 1
1 1 8.8 6.2 10.0 0
1 2 7.2 11.7 5.9 1
1 1 2.0 3.0 9.0 0
1 2 6.5 7.7 7.6 1
1 1 5.8 9.4 6.9 1
1 2 8.5 9.5 7.9 1
1 1 5.6 10.5 5.8 0
1 2 7.7 6.8 6.6 1
1 1 2.5 5.4 8.4 0
1 2 4.8 7.2 8.0 1
2 1 7.1 10.0 8.0 1
2 2 6.2 9.5 5.3 0
2 1 2.8 6.6 9.1 1
2 2 6.5 9.9 6.2 1
2 1 3.2 6.2 10.0 0
2 2 7.2 11.7 5.9 1
2 1 2.0 3.0 9.0 0
2 2 6.5 10.0 7.6 1
2 1 5.8 9.4 6.9 1
2 2 7.0 9.5 7.9 1
2 1 5.6 10.5 5.8 0
2 2 7.7 7.9 6.6 1
2 1 2.5 5.4 8.4 0
2 2 4.8 7.2 8.0 1
3 1 7.1 10.0 8.0 1
3 2 6.2 9.5 5.3 0
3 1 2.8 7.0 9.1 1
3 2 6.5 9.9 6.2 1
3 1 7.0 6.2 10.0 0
3 2 7.2 11.7 5.9 1
3 1 2.0 3.0 9.0 0
3 2 6.5 8.0 7.6 1
3 1 5.8 9.4 6.9 1
3 2 7.1 9.5 7.9 1
3 1 5.6 10.5 5.8 0
3 2 7.7 9.3 6.6 1
3 1 2.5 5.4 8.4 0
3 2 4.8 7.2 8.0 1
END DATA.
SPLIT FILE LAYERED BY Imputation_.
";

/// Checks the numbers of a pooled row from column `from` against a black-box run of a general
/// statistics package (lab/oracles/mi/pool_procedures.R), NaN where the cell is empty.
fn pooled_row(table: &Table, row: usize, from: usize, want: &[f64]) {
    for (column, want) in want.iter().enumerate() {
        let got = num(&table.rows[row][from + column]);
        assert!(
            got.is_nan() && want.is_nan() || (got - want).abs() <= 1e-8 * want.abs().max(1.0),
            "{}, row {row}, column {}: {got} against {want}",
            table.title,
            from + column
        );
    }
}

#[test]
fn pooled_means_and_t_tests() {
    // site 1, site 2, total: estimate, standard error, df, interval, FMI, RIV
    let (tables, _, _) = run_on(HIVES, "MEANS honey BY site.");
    let report = find(&tables, "Pooled Report: honey by site");
    assert_eq!(words(&report.rows[2][0]), "Total");
    let want = [
        [
            7.20952380952,
            1.09773008425,
            869.942590797,
            5.05501484995,
            9.3640327691,
            0.0501291600384,
            0.050362698397,
        ],
        [
            9.19523809524,
            0.660086574408,
            47.2799383339,
            7.86752270713,
            10.5229534833,
            0.237268845792,
            0.258926816484,
        ],
        [
            8.20238095238,
            0.701227089724,
            95.4463411237,
            6.81035313006,
            9.5944087747,
            0.162130427675,
            0.169256405391,
        ],
    ];
    for (row, want) in want.iter().enumerate() {
        pooled_row(report, row, 1, want);
    }
    // mean difference, standard error, t, df, significance, interval, FMI, RIV
    let (tables, _, _) = run_on(HIVES, "T-TEST PAIRS=honey WITH mites (PAIRED).");
    let paired = find(&tables, "Pooled Paired Samples Test");
    assert_eq!(words(&paired.rows[0][1]), "honey - mites");
    pooled_row(
        paired,
        0,
        2,
        &[
            0.72380952381,
            0.998043175249,
            0.725228669219,
            391.671399792,
            0.46874481861,
            -1.23838249748,
            2.6860015451,
            0.076163918546,
            0.0769578209585,
        ],
    );
    let (tables, _, _) = run_on(HIVES, "T-TEST TESTVAL=8 /VARIABLES=honey.");
    pooled_row(
        find(&tables, "Pooled One-Sample Test (Test Value = 8)"),
        0,
        1,
        &[
            0.202380952381,
            0.701227089724,
            0.28860971766,
            95.4463411237,
            0.773505919077,
            -1.18964686994,
            1.5944087747,
            0.162130427675,
            0.169256405391,
        ],
    );
    // equal group sizes: the pooled and the separate standard errors agree
    let (tables, _, _) = run_on(HIVES, "T-TEST GROUPS=site(1 2) /VARIABLES=honey.");
    let groups = find(&tables, "Pooled Independent Samples Test");
    for row in 0..2 {
        pooled_row(
            groups,
            row,
            2,
            &[
                -1.98571428571,
                1.22613268472,
                -1.61949380394,
                44595.5076698,
                0.105348129576,
                -4.38895541427,
                0.41752684284,
                0.00674137681258,
                0.00674198250729,
            ],
        );
    }
}

#[test]
fn pooled_correlations_use_fishers_z() {
    let (tables, _, _) = run_on(HIVES, "CORRELATIONS temp honey mites.");
    let pooled = find(&tables, "Pooled Correlations");
    // each pair once: correlation, significance, df, interval
    let want = [
        (
            "temp",
            "honey",
            [
                0.764098152092,
                0.0511022695584,
                7.48883426007,
                -0.00623245381911,
                0.965291907135,
            ],
        ),
        (
            "temp",
            "mites",
            [
                -0.487214210069,
                0.2951294719,
                5.98618745083,
                -0.931388017422,
                0.539862630854,
            ],
        ),
        (
            "honey",
            "mites",
            [
                -0.746607787177,
                0.00156589770788,
                3809.35891061,
                -0.9159496732,
                -0.351553978399,
            ],
        ),
    ];
    assert_eq!(pooled.rows.len(), 3);
    for (row, (a, b, want)) in want.iter().enumerate() {
        assert_eq!(
            (words(&pooled.rows[row][0]), words(&pooled.rows[row][1])),
            (*a, *b)
        );
        pooled_row(pooled, row, 2, want);
    }
}

#[test]
fn pooled_regressions_and_their_model_tests() {
    let (tables, _, _) = run_on(
        HIVES,
        "REGRESSION /DEPENDENT=honey /METHOD=ENTER temp mites.",
    );
    let coefficients = find(&tables, "Pooled Coefficients: honey");
    // B, standard error, t, df, significance, interval, FMI, RIV
    let want = [
        (
            "(Constant)",
            [
                10.5295584429,
                4.63392279385,
                2.27227748741,
                5.66383128262,
                0.0660587320285,
                -0.974394316637,
                22.0335112024,
                0.687905422321,
                1.46449410256,
            ],
        ),
        (
            "temp",
            [
                0.679956962916,
                0.26218015295,
                2.59347229477,
                13.9030413771,
                0.021338629024,
                0.117268330437,
                1.2426455954,
                0.452724884682,
                0.611032684727,
            ],
        ),
        (
            "mites",
            [
                -0.818279148583,
                0.504787902529,
                -1.62103557649,
                4.50934636744,
                0.172245841573,
                -2.15951647463,
                0.522958177462,
                0.754937777825,
                1.99379165218,
            ],
        ),
    ];
    for (row, (name, want)) in want.iter().enumerate() {
        assert_eq!(words(&coefficients.rows[row][1]), *name);
        pooled_row(coefficients, row, 2, want);
    }
    // F, df1, df2, significance of the joint test of the slopes
    let test = find(&tables, "Pooled Model Test: honey");
    pooled_row(
        test,
        0,
        1,
        &[9.52936813403, 2.0, 12.0052376128, 0.0033235423561],
    );

    // BCON is tight so that the estimates match the package's converged ones
    let (tables, _, _) = run_on(
        HIVES,
        "LOGISTIC REGRESSION swarm WITH temp mites /CRITERIA=BCON(1E-10).",
    );
    let equation = find(&tables, "Pooled Variables in the Equation");
    // B, standard error, t, df, significance, then Exp(B) and its interval
    let want = [
        (
            "temp",
            [
                0.988335485826,
                1.37132862152,
                0.720713817475,
                3.81547674785,
                0.512763264706,
            ],
            (-2.89281286809_f64, 4.86948383974_f64),
        ),
        (
            "mites",
            [
                0.626208416579,
                1.38463209617,
                0.452256175711,
                3.9648496718,
                0.67472256628,
            ],
            (-3.23162526162, 4.48404209478),
        ),
        (
            "Constant",
            [
                -9.16633809088,
                16.7435106271,
                -0.547456163468,
                3.69475455744,
                0.615440515955,
            ],
            (-57.208321592, 38.8756454103),
        ),
    ];
    for (row, (name, want, (low, high))) in want.iter().enumerate() {
        assert_eq!(words(&equation.rows[row][1]), *name);
        pooled_row(equation, row, 2, want);
        pooled_row(equation, row, 7, &[want[0].exp(), low.exp(), high.exp()]);
    }
    pooled_row(
        find(&tables, "Pooled Model Test"),
        0,
        1,
        &[0.410627729501, 2.0, 8.63724994727, 0.675527566118],
    );
}

#[test]
fn pooled_ordinal_and_multinomial_parameters() {
    // PLUM: Rubin's rules by hand on the estimates and standard errors of each imputation
    let (tables, _, _) = run_on(
        HIVES,
        "RECODE honey (LO THRU 7=1) (LO THRU 10=2) (ELSE=3) INTO grade.\n\
         PLUM grade WITH temp mites /PRINT=PARAMETER.",
    );
    let each = find(&tables, "Parameter Estimates");
    let pooled = find(&tables, "Pooled Parameter Estimates");
    assert_eq!(pooled.rows.len(), 4);
    for j in 0..4 {
        let pairs: Vec<(f64, f64)> = (1..=3)
            .map(|i| {
                let row = &each.rows[4 * i + j];
                (num(&row[3]), num(&row[4]))
            })
            .collect();
        let q = pairs.iter().map(|p| p.0).sum::<f64>() / 3.0;
        let u = pairs.iter().map(|p| p.1 * p.1).sum::<f64>() / 3.0;
        let b = pairs.iter().map(|p| (p.0 - q).powi(2)).sum::<f64>() / 2.0;
        let r = 4.0 / 3.0 * b / u;
        let df = 2.0 * (1.0 + 1.0 / r).powi(2);
        let se = (u + 4.0 / 3.0 * b).sqrt();
        pooled_row(pooled, j, 2, &[q, se, q / se, df]);
    }
    // NOMREG with two categories is the logistic model of the first one: signs turn over
    let (tables, _, _) = run_on(HIVES, "NOMREG swarm WITH temp mites.");
    let pooled = find(&tables, "Pooled Parameter Estimates");
    let want = [
        (
            "Intercept",
            [
                9.16633809088,
                16.7435106271,
                0.547456163468,
                3.69475455744,
                0.615440515955,
            ],
        ),
        (
            "temp",
            [
                -0.988335485826,
                1.37132862152,
                -0.720713817475,
                3.81547674785,
                0.512763264706,
            ],
        ),
        (
            "mites",
            [
                -0.626208416579,
                1.38463209617,
                -0.452256175711,
                3.9648496718,
                0.67472256628,
            ],
        ),
    ];
    for (row, (name, want)) in want.iter().enumerate() {
        assert_eq!(words(&pooled.rows[row][1]), *name);
        pooled_row(pooled, row, 2, want);
    }
}

#[test]
fn rows_missing_from_an_imputation_and_mioutput() {
    // one hive of the first imputation alone sits at a third site
    let (tables, warnings, _) = run_on(
        HIVES,
        "COMPUTE place = site.\nIF (Imputation_ = 1 AND mites = 10) place = 3.\n\
         MEANS honey BY place.",
    );
    let report = find(&tables, "Pooled Report: honey by place");
    let places: Vec<&str> = report.rows.iter().map(|r| words(&r[0])).collect();
    assert_eq!(places, ["1", "2", "Total"]);
    assert_eq!(
        warnings,
        [
            "Pooled Report: honey by place: 3 is not pooled because the imputations do not all give it"
        ]
    );
    // without DIAGNOSTICS the last three columns go; without POOLED the pooled tables
    let (tables, _, _) = run_on(
        HIVES,
        "SET MIOUTPUT=[POOLED].\nREGRESSION /DEPENDENT=honey /METHOD=ENTER temp.",
    );
    assert_eq!(find(&tables, "Pooled Coefficients: honey").columns.len(), 9);
    assert_eq!(find(&tables, "Pooled Model Test: honey").columns.len(), 5);
    assert!(tables.iter().all(|t| t.title != "Coefficients"));
    let (tables, _, _) = run_on(HIVES, "SET MIOUTPUT=[IMPUTED].\nCORRELATIONS temp honey.");
    assert!(tables.iter().all(|t| !t.title.starts_with("Pooled")));
}

#[test]
fn strings_interactions_and_analysis_weights() {
    // a string variable is categorical: its user-missing value is filled with an observed one
    let (tables, _, mut engine) = run(
        "STRING bed (A6).\nRECODE soil (1='raised') (2='flat') (ELSE='none') INTO bed.\n\
         MISSING VALUES bed ('none').\nCOMPUTE older = age > 50.\nVARIABLE LEVEL older (NOMINAL).\n\
         COMPUTE aw = 1 + (plot > 130).\nIF (plot = 102) aw = 0.\n\
         MULTIPLE IMPUTATION age older bed hours /IMPUTE METHOD=FCS INTERACTIONS=TWOWAY \
         NIMPUTATIONS=2 MAXITER=2 /ANALYSISWEIGHT aw /OUTFILE IMPUTATIONS=filled.",
    );
    let spec = find(&tables, "Imputation Specifications");
    assert_eq!(words(&spec.rows[3][1]), "Two-way");
    assert_eq!(words(&spec.rows[6][1]), "aw");
    let models = find(&tables, "Imputation Models");
    assert_eq!(words(&models.rows[1][2]), "age older bed older*bed");
    engine
        .execute(&parse("DATASET ACTIVATE filled.").0[0].command)
        .unwrap();
    let data = engine.dataset().unwrap();
    let bed = data.variable("bed").unwrap();
    let values: Vec<String> = bed.string().unwrap().iter().map(str::to_string).collect();
    for case in 0..40 {
        for l in 1..=2 {
            let filled = &values[40 * l + case];
            match values[case].as_str() {
                // plot 102 has analysis weight 0 and stays as it was
                "none" if case == 1 => assert_eq!(filled, "none"),
                "none" => assert!(filled == "raised" || filled == "flat", "{filled}"),
                kept => assert_eq!(filled, kept),
            }
        }
    }
    // the plot with analysis weight 0 is left as it was
    let hours = column(&engine, "hours");
    assert_eq!(hours[40 + 1], hours[1]);
}

#[test]
fn date_limits() {
    let (_, _, engine) = run(
        "COMPUTE sown = DATE.DMY(1, 4, 2026) + hours * 86400.\nFORMATS sown (ADATE10).\n\
         MULTIPLE IMPUTATION age sown /CONSTRAINTS sown (MIN='04/02/2026' MAX='04/12/2026' RND=1) \
         /IMPUTE NIMPUTATIONS=2 /OUTFILE IMPUTATIONS=filled.\nDATASET ACTIVATE filled.",
    );
    let sown = column(&engine, "sown");
    let (low, high) = (
        13_994_380_800.0 + 86_400.0,
        13_994_380_800.0 + 11.0 * 86_400.0,
    );
    for value in &sown[40..] {
        let v = value.unwrap();
        assert!(v >= low && v <= high, "{v}");
    }
    // a quoted limit on a variable without a date format is an error
    let message = fails(
        "MULTIPLE IMPUTATION age hours /CONSTRAINTS hours (MAX='04/12/2026') /OUTFILE IMPUTATIONS=filled.",
    );
    assert!(message.contains("takes a number as its limit"), "{message}");
}

#[test]
fn a_variable_without_valid_values_is_left_out() {
    let (tables, warnings, _) = run(
        "COMPUTE empty = $SYSMIS.\nMULTIPLE IMPUTATION age hours empty /OUTFILE IMPUTATIONS=filled.",
    );
    assert_eq!(warnings, ["empty has no valid values and is not imputed"]);
    let results = find(&tables, "Imputation Results");
    assert_eq!(words(&results.rows[1][1]), "hours");
}
