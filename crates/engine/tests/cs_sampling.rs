//! CSPLAN SAMPLE and CSSELECT on an invented frame of orchards: three valleys (strata), four,
//! five and six orchards in them (clusters), two to four trees per orchard (cases) and a
//! yield per tree. Every expected number follows from the frame and the rules: sizes,
//! inclusion probabilities n/N and weights N/n are exact; the unbiasedness check averages the
//! estimated total over many seeds.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// (valley, orchard, tree, yield) of every tree; orchards are numbered 1 to 15 across valleys.
fn frame() -> Vec<(u32, u32, u32, f64)> {
    let mut out = Vec::new();
    let mut orchard = 0;
    for valley in 1..=3 {
        for _ in 0..valley + 3 {
            orchard += 1;
            for tree in 1..=2 + orchard % 3 {
                let y = f64::from(5 * valley + 2 * orchard + tree);
                out.push((valley, orchard, tree, y));
            }
        }
    }
    out
}

/// The frame as inline data and a CD into a fresh folder for plans and sample files.
fn data(folder: &str) -> String {
    let rows: Vec<String> = frame()
        .iter()
        .enumerate()
        .map(|(i, (v, o, t, y))| format!("{} {v} {o} {t} {y}", i + 1))
        .collect();
    let dir =
        std::env::temp_dir().join(format!("statscraft-cssel-{folder}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    format!(
        "DATA LIST FREE /id valley orchard tree yield.\nBEGIN DATA\n{}\nEND DATA.\nCD '{}'.\n",
        rows.join("\n"),
        dir.display()
    )
}

fn run_on(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn run(source: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let tables = run_on(&mut engine, source);
    (engine, tables)
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    let variable = data.variable(name).unwrap_or_else(|_| panic!("no {name}"));
    variable.numeric().unwrap().iter().collect()
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * b.abs().max(1.0)
}

/// Trees per valley in the frame.
fn trees_in(valley: u32) -> usize {
    frame().iter().filter(|r| r.0 == valley).count()
}

#[test]
fn stratified_simple_random_sample_of_trees() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='trees.json' /PRINT PLAN MATRIX
           /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /SIZE MATRIX=valley; 1 3; 2 4; 3 5
           /STAGEVARS POPSIZE SAMPSIZE RATE WEIGHT.
         CSSELECT /PLAN FILE='trees.json' /CRITERIA SEED=20261117 /PRINT SELECTION CPS.\n",
        data("srs")
    );
    let (engine, tables) = run(&source);
    // CSPLAN shows the sampling side, the analysis side and the matrix
    for title in ["Sample Plan", "Analysis Plan", "Plan Matrix"] {
        find(&tables, title);
    }
    let weight = column(&engine, "SampleWeight_Final_");
    let prob = column(&engine, "InclusionProbability_1_");
    let popsize = column(&engine, "PopulationSize_1_");
    let sampsize = column(&engine, "SampleSize_1_");
    let rate = column(&engine, "SamplingRate_1_");
    let stage_weight = column(&engine, "SampleWeight_1_");
    let cum = column(&engine, "SampleWeightCumulative_1_");
    for (valley, n) in [(1, 3), (2, 4), (3, 5)] {
        let big_n = trees_in(valley) as f64;
        let mut drawn = 0;
        for (case, row) in frame().iter().enumerate() {
            if row.0 != valley || weight[case].is_none() {
                continue;
            }
            drawn += 1;
            assert!(close(prob[case].unwrap(), n as f64 / big_n));
            assert!(close(weight[case].unwrap(), big_n / n as f64));
            assert_eq!(cum[case], weight[case]);
            assert_eq!(stage_weight[case], weight[case]);
            assert_eq!(popsize[case], Some(big_n));
            assert_eq!(sampsize[case], Some(n as f64));
            assert!(close(rate[case].unwrap(), n as f64 / big_n));
        }
        assert_eq!(drawn, n, "valley {valley}");
    }
    // a case outside the sample keeps its place with missing values
    assert_eq!(weight.len(), frame().len());
    assert_eq!(
        weight.iter().filter(|w| w.is_none()).count(),
        frame().len() - 12
    );
    let summary = find(&tables, "Selection Summary: Stage 1");
    assert_eq!(summary.rows.len(), 3);
    assert_eq!(summary.rows[1][0], Cell::Text("valley = 2".into()));
    assert_eq!(number(&summary.rows[1][1]), trees_in(2) as f64);
    assert_eq!(number(&summary.rows[1][2]), 4.0);
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(number(&cps.rows[3][1]), 12.0);
}

#[test]
fn rate_rounds_and_minsize_and_maxsize_bound_it() {
    // trees per valley: 12, 15 and 18; 0.3 of them is 3.6, 4.5 and 5.4, which round to 4, 5, 5
    assert_eq!((trees_in(1), trees_in(2), trees_in(3)), (12, 15, 18));
    let sizes = |rate: &str| -> Vec<usize> {
        let source = format!(
            "{}CSPLAN SAMPLE /PLAN FILE='rate.json' /PRINT MATRIX
               /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_CHROMY /RATE {rate}.
             CSSELECT /PLAN FILE='rate.json' /CRITERIA SEED=5.\n",
            data("rate")
        );
        let (engine, _) = run(&source);
        let weight = column(&engine, "SampleWeight_Final_");
        (1..=3)
            .map(|v| {
                frame()
                    .iter()
                    .zip(&weight)
                    .filter(|(r, w)| r.0 == v && w.is_some())
                    .count()
            })
            .collect()
    };
    assert_eq!(sizes("VALUE=0.3"), [4, 5, 5]);
    assert_eq!(sizes("VALUE=0.3 MINSIZE=5 MAXSIZE=5"), [5, 5, 5]);
    // a rate per stratum; 0.1 of 15 is 1.5, which rounds up, and 0.01 of 18 rounds to 0
    assert_eq!(
        sizes("MATRIX=valley; 1 0.5; 2 0.1; 3 0.01 MINSIZE=1"),
        [6, 2, 1]
    );
}

#[test]
fn a_seed_repeats_the_sample_and_another_seed_changes_it() {
    let draw = |criteria: &str| {
        let source = format!(
            "{}CSPLAN SAMPLE /PLAN FILE='seed.json' /PRINT MATRIX
               /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=5.
             {criteria}CSSELECT /PLAN FILE='seed.json'.\n",
            data("seed")
        );
        column(&run(&source).0, "SampleWeight_Final_")
    };
    let a = draw("SET SEED=1234.\n");
    let b = draw("SET SEED=1234.\n");
    let c = draw("SET SEED=4321.\n");
    assert_eq!(a, b);
    assert_ne!(a, c);
    let with_seed = |select: &str| {
        let source = format!(
            "{}CSPLAN SAMPLE /PLAN FILE='seed2.json' /PRINT MATRIX
               /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=5.
             SET SEED=99.
             {select}
             COMPUTE u = UNIFORM(1).
             EXECUTE.\n",
            data("seed2")
        );
        let (engine, _) = run(&source);
        let u = column(&engine, "u");
        match select.is_empty() {
            true => (Vec::new(), u),
            false => (column(&engine, "SampleWeight_Final_"), u),
        }
    };
    let (x, u) = with_seed("CSSELECT /PLAN FILE='seed2.json' /CRITERIA SEED=7.");
    assert_eq!(
        with_seed("CSSELECT /PLAN FILE='seed2.json' /CRITERIA SEED=7.").0,
        x
    );
    assert_ne!(
        with_seed("CSSELECT /PLAN FILE='seed2.json' /CRITERIA SEED=8.").0,
        x
    );
    // CRITERIA SEED leaves the session's generator alone
    assert_eq!(with_seed("").1, u);
}

#[test]
fn with_replacement_writes_one_row_per_draw_to_the_sample_file() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='wr.json' /PRINT MATRIX
           /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WR /SIZE VALUE=30.
         CSSELECT /PLAN FILE='wr.json' /CRITERIA SEED=3 /SAMPLEFILE OUTFILE='wr.sav' KEEP=id valley yield.
         GET FILE='wr.sav'.\n",
        data("wr")
    );
    let (engine, _) = run(&source);
    let drawn = engine.dataset().unwrap();
    let names: Vec<&str> = drawn.variable_names().collect();
    assert_eq!(
        names,
        [
            "id",
            "valley",
            "yield",
            "InclusionProbability_1_",
            "SampleWeightCumulative_1_",
            "Index_1_",
            "SampleWeight_Final_"
        ]
    );
    assert_eq!(drawn.case_count(), 90);
    let ids = column(&engine, "id");
    let valley = column(&engine, "valley");
    let index = column(&engine, "Index_1_");
    let prob = column(&engine, "InclusionProbability_1_");
    let weight = column(&engine, "SampleWeight_Final_");
    let mut repeated = false;
    for case in 0..90 {
        let v = valley[case].unwrap() as u32;
        let big_n = trees_in(v) as f64;
        assert!(close(weight[case].unwrap(), big_n / 30.0));
        assert!(close(
            prob[case].unwrap(),
            1.0 - (1.0 - 1.0 / big_n).powi(30)
        ));
        // a unit drawn again follows its first row with the next index
        let k = index[case].unwrap();
        if k > 1.0 {
            repeated = true;
            assert_eq!(ids[case], ids[case - 1]);
            assert_eq!(index[case - 1], Some(k - 1.0));
        } else if case > 0 {
            assert_ne!(ids[case], ids[case - 1]);
        }
    }
    assert!(repeated);
    for v in 1..=3 {
        assert_eq!(
            valley.iter().filter(|x| **x == Some(f64::from(v))).count(),
            30
        );
    }
    let error = run_error(&format!(
        "{}CSPLAN SAMPLE /PLAN FILE='wr2.json' /PRINT MATRIX /DESIGN /METHOD TYPE=SIMPLE_WR /SIZE VALUE=3.
         CSSELECT /PLAN FILE='wr2.json' /CRITERIA SEED=3.\n",
        data("wr2")
    ));
    assert!(error.contains("SAMPLEFILE"), "{error}");
}

#[test]
fn systematic_draws_are_evenly_spaced_in_file_order() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='sys.json' /PRINT MATRIX
           /DESIGN /METHOD TYPE=SIMPLE_SYSTEMATIC /SIZE VALUE=3.
         SELECT IF id <= 24.
         CSSELECT /PLAN FILE='sys.json' /CRITERIA SEED=8.\n",
        data("sys")
    );
    let (engine, _) = run(&source);
    let weight = column(&engine, "SampleWeight_Final_");
    let at: Vec<usize> = (0..24).filter(|&c| weight[c].is_some()).collect();
    assert_eq!(at.len(), 3);
    assert!(at[0] < 8);
    assert_eq!((at[1] - at[0], at[2] - at[1]), (8, 8));
    assert!(weight.iter().flatten().all(|w| *w == 8.0));
}

/// Orchards per valley and trees per orchard of the frame.
fn orchards_in(valley: u32) -> usize {
    let mut seen: Vec<u32> = frame()
        .iter()
        .filter(|r| r.0 == valley)
        .map(|r| r.1)
        .collect();
    seen.dedup();
    seen.len()
}

fn trees_of(orchard: u32) -> usize {
    frame().iter().filter(|r| r.1 == orchard).count()
}

const TWO_STAGE: &str = "CSPLAN SAMPLE /PLAN FILE='two.json' /PRINT MATRIX
   /DESIGN STAGELABEL='orchards' STRATA=valley CLUSTER=orchard /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2
   /DESIGN STAGELABEL='trees' /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1.\n";

/// The weights of a two-stage draw: the orchard's stratum share times the tree's.
fn check_two_stage(engine: &Engine) {
    let weight = column(engine, "SampleWeight_Final_");
    let first = column(engine, "SampleWeightCumulative_1_");
    let p2 = column(engine, "InclusionProbability_2_");
    let mut orchards_drawn = Vec::new();
    for (case, (valley, orchard, _, _)) in frame().into_iter().enumerate() {
        let w1 = orchards_in(valley) as f64 / 2.0;
        if let Some(w) = first[case] {
            assert!(close(w, w1));
            if !orchards_drawn.contains(&orchard) {
                orchards_drawn.push(orchard);
            }
        }
        if let Some(w) = weight[case] {
            assert!(close(w, w1 * trees_of(orchard) as f64));
            assert!(close(p2[case].unwrap(), 1.0 / trees_of(orchard) as f64));
        }
    }
    assert_eq!(orchards_drawn.len(), 6);
    assert_eq!(weight.iter().flatten().count(), 6);
}

#[test]
fn two_stages_multiply_the_weights() {
    let source = format!(
        "{}{TWO_STAGE}CSSELECT /PLAN FILE='two.json' /CRITERIA SEED=31.\n",
        data("two")
    );
    let (engine, tables) = run(&source);
    check_two_stage(&engine);
    find(&tables, "Selection Summary: orchards");
    let trees = find(&tables, "Selection Summary: trees");
    // a stratum of stage 2 is a drawn orchard
    assert_eq!(trees.rows.len(), 6);
    let Cell::Text(label) = &trees.rows[0][0] else {
        panic!()
    };
    assert!(label.starts_with("valley = 1, orchard = "), "{label}");
}

#[test]
fn stages_run_one_at_a_time_give_the_same_weights() {
    let source = format!(
        "{}{TWO_STAGE}CSSELECT /PLAN FILE='two.json' /CRITERIA STAGES=1 SEED=31.
         CSSELECT /PLAN FILE='two.json' /CRITERIA STAGES=2 SEED=32 /DATA RENAMEVARS.\n",
        data("staged")
    );
    let (engine, _) = run(&source);
    check_two_stage(&engine);
    // the final weight of the first run, which ended after stage 1, makes way for the new one
    assert_eq!(
        column(&engine, "SampleWeight_Final__1"),
        column(&engine, "SampleWeightCumulative_1_")
    );
    let error = run_error(&format!(
        "{}{TWO_STAGE}CSSELECT /PLAN FILE='two.json' /CRITERIA STAGES=2 SEED=32.\n",
        data("staged2")
    ));
    assert!(error.contains("SampleWeightCumulative_1_"), "{error}");
}

/// The text of a file CSSELECT wrote in the folder of `data(folder)`.
fn written(folder: &str, file: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("statscraft-cssel-{folder}-{}", std::process::id()));
    std::fs::read_to_string(dir.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

/// The rule naming each selected unit by `parts` of its first case, the units joined by OR.
fn expected_rule(units: &[Vec<(&str, u32)>]) -> String {
    let clauses: Vec<String> = units
        .iter()
        .map(|parts| {
            let inner: Vec<String> = parts
                .iter()
                .map(|(name, v)| format!("({name} EQ {v})"))
                .collect();
            format!("({})", inner.join(" AND "))
        })
        .collect();
    clauses.join("\nOR ") + "\n"
}

#[test]
fn selectrule_names_the_selected_clusters_of_a_two_stage_draw() {
    // stage 2 draws trees without clusters, so the rule describes the orchards of stage 1
    let source = format!(
        "{}{TWO_STAGE}CSSELECT /PLAN FILE='two.json' /CRITERIA SEED=31 /SELECTRULE OUTFILE='rule.txt'.\n",
        data("rule")
    );
    let (engine, _) = run(&source);
    let first = column(&engine, "SampleWeightCumulative_1_");
    let mut units: Vec<Vec<(&str, u32)>> = Vec::new();
    for (case, (valley, orchard, _, _)) in frame().into_iter().enumerate() {
        if first[case].is_some() && !units.iter().any(|u| u[1].1 == orchard) {
            units.push(vec![("valley", valley), ("orchard", orchard)]);
        }
    }
    assert_eq!(units.len(), 6);
    assert_eq!(written("rule", "rule.txt"), expected_rule(&units));

    // with a cluster in stage 2 too, the rule names the trees inside their orchards
    let plan = "CSPLAN SAMPLE /PLAN FILE='both.json'
       /DESIGN STRATA=valley CLUSTER=orchard /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2
       /DESIGN CLUSTER=tree /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1.\n";
    let source = format!(
        "{}{plan}CSSELECT /PLAN FILE='both.json' /CRITERIA SEED=8 /SELECTRULE OUTFILE='rule.txt' /PRINT CPS.\n",
        data("rule2")
    );
    let (engine, _) = run(&source);
    let weight = column(&engine, "SampleWeight_Final_");
    let units: Vec<Vec<(&str, u32)>> = frame()
        .into_iter()
        .enumerate()
        .filter(|(case, _)| weight[*case].is_some())
        .map(|(_, (valley, orchard, tree, _))| {
            vec![("valley", valley), ("orchard", orchard), ("tree", tree)]
        })
        .collect();
    assert_eq!(units.len(), 6);
    assert_eq!(written("rule2", "rule.txt"), expected_rule(&units));

    // a draw of cases without clusters has no rule: a warning and no file
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='srs.json' /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2.
         CSSELECT /PLAN FILE='srs.json' /CRITERIA SEED=5 /SELECTRULE OUTFILE='none.txt'.\n",
        data("rule3")
    );
    let (engine, _) = run(&source);
    assert!(engine.warnings().join("\n").contains("no SELECTRULE file"));
    let dir = std::env::temp_dir().join(format!("statscraft-cssel-rule3-{}", std::process::id()));
    assert!(!dir.join("none.txt").exists());
}

/// The total of yield estimated by CSDESCRIPTIVES from a drawn sample and the plan.
fn estimated_total(plan: &str, seed: u32, folder: &str) -> f64 {
    let source = format!(
        "{}{plan}CSSELECT /PLAN FILE='est.json' /CRITERIA SEED={seed} /PRINT CPS.
         CSDESCRIPTIVES /PLAN FILE='est.json' /SUMMARY VARIABLES=yield /SUM.\n",
        data(folder)
    );
    let (_, tables) = run(&source);
    let t = find(&tables, "Univariate Statistics");
    let column = t
        .column_titles()
        .iter()
        .position(|c| c == "Estimate")
        .unwrap();
    number(&t.rows[0][column])
}

#[test]
fn the_analysis_side_of_the_plan_estimates_the_total_without_bias() {
    let truth: f64 = frame().iter().map(|r| r.3).sum();
    for (name, plan) in [
        (
            "two-stage",
            "CSPLAN SAMPLE /PLAN FILE='est.json' /PRINT MATRIX
               /DESIGN STRATA=valley CLUSTER=orchard /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2
               /DESIGN /METHOD TYPE=SIMPLE_WOR /RATE VALUE=0.5.\n",
        ),
        (
            "systematic",
            "CSPLAN SAMPLE /PLAN FILE='est.json' /PRINT MATRIX
               /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_SYSTEMATIC /SIZE VALUE=4.\n",
        ),
    ] {
        let estimates: Vec<f64> = (1..=300)
            .map(|seed| estimated_total(plan, seed, name))
            .collect();
        let r = estimates.len() as f64;
        let mean = estimates.iter().sum::<f64>() / r;
        let sd = (estimates.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (r - 1.0)).sqrt();
        // the estimator is unbiased, so the mean of 300 independent estimates lies within a few
        // standard errors of the truth: 4 of them fail by chance about once in 15,000 seed
        // sets, and the seeds are fixed, so the test is deterministic. A wrong weight (say N/n
        // of the wrong stage, or a missing stage factor) moves the mean by a share of the truth,
        // many standard errors.
        let se = sd / r.sqrt();
        assert!(sd > 0.0);
        assert!(
            (mean - truth).abs() < 4.0 * se,
            "{name}: mean {mean}, truth {truth}, se {se}"
        );
    }
}

#[test]
fn a_name_clash_is_an_error_unless_renamevars() {
    let plan = "CSPLAN SAMPLE /PLAN FILE='clash.json' /PRINT MATRIX
       /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=4.\n";
    let error = run_error(&format!(
        "{}{plan}CSSELECT /PLAN FILE='clash.json' /CRITERIA SEED=1.
         CSSELECT /PLAN FILE='clash.json' /CRITERIA SEED=2.\n",
        data("clash")
    ));
    assert!(error.contains("RENAMEVARS"), "{error}");
    let (engine, _) = run(&format!(
        "{}{plan}CSSELECT /PLAN FILE='clash.json' /CRITERIA SEED=1.
         CSSELECT /PLAN FILE='clash.json' /CRITERIA SEED=2 /DATA RENAMEVARS.\n",
        data("clash2")
    ));
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("SampleWeight_Final__1"))
    );
    assert_eq!(
        column(&engine, "SampleWeight_Final__1")
            .iter()
            .flatten()
            .count(),
        4
    );
    assert_eq!(
        column(&engine, "SampleWeight_Final_")
            .iter()
            .flatten()
            .count(),
        4
    );
}

#[test]
fn filtered_and_missing_cases_stay_out_of_the_frame() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='filter.json' /PRINT MATRIX
           /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /RATE VALUE=1.
         COMPUTE keep = valley <> 2.
         IF (id = 1) valley = $SYSMIS.
         FILTER BY keep.
         CSSELECT /PLAN FILE='filter.json' /CRITERIA SEED=4 /PRINT CPS.\n",
        data("filter")
    );
    let (engine, tables) = run(&source);
    let weight = column(&engine, "SampleWeight_Final_");
    // rate 1 draws every unit of the frame: valleys 1 and 3 without case 1
    assert_eq!(weight.iter().flatten().count(), 11 + 18);
    assert!(weight[0].is_none());
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(number(&cps.rows[1][1]), 1.0);
}

#[test]
fn plans_and_options_that_do_not_fit_give_errors_or_warnings() {
    let analysis = "CSPLAN ANALYSIS /PLAN FILE='an.json' /PLANVARS ANALYSISWEIGHT=yield
       /DESIGN /ESTIMATOR TYPE=WR.\n";
    let error = run_error(&format!(
        "{}{analysis}CSSELECT /PLAN FILE='an.json'.\n",
        data("errors")
    ));
    assert!(error.contains("CSPLAN SAMPLE"), "{error}");
    let error = run_error(&format!(
        "{}{TWO_STAGE}CSSELECT /PLAN FILE='two.json' /CRITERIA STAGES=3.\n",
        data("errors2")
    ));
    assert!(error.contains("the plan has 2"), "{error}");
    let (engine, _) = run(&format!(
        "{}CSPLAN SAMPLE /PLAN FILE='j.json' /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=60.
         CSSELECT /PLAN FILE='j.json' /CRITERIA SEED=1 /JOINTPROB OUTFILE='j.sav'.\n",
        data("errors3")
    ));
    let warnings = engine.warnings().join("\n");
    assert!(warnings.contains("JOINTPROB"), "{warnings}");
    // 60 asked of 45 trees: all of them, with a warning
    assert!(warnings.contains("fewer units"), "{warnings}");
    assert_eq!(
        column(&engine, "SampleWeight_Final_")
            .iter()
            .flatten()
            .count(),
        45
    );
}

#[test]
fn csplan_view_shows_a_sample_plan_and_the_file_says_its_kind() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='view.json' /PLANVARS SAMPLEWEIGHT=fw PREVIOUSWEIGHT=yield
           /DESIGN STRATA=valley /METHOD TYPE=SIMPLE_WOR /RATE VALUE=0.2 MINSIZE=2
           /STAGEVARS WEIGHT.
         CSPLAN VIEW /PLAN FILE='view.json'.\n",
        data("view")
    );
    let (_, tables) = run(&source);
    let views: Vec<&Table> = tables.iter().filter(|t| t.title == "Sample Plan").collect();
    assert_eq!(views.len(), 2);
    let text: Vec<String> = views[1].rows.iter().map(|r| format!("{:?}", r)).collect();
    let joined = text.join("\n");
    for expected in ["fw", "Previous weight", "Minimum size", "SampleWeight_1_"] {
        assert!(joined.contains(expected), "{expected}: {joined}");
    }
    let dir = std::env::temp_dir().join(format!("statscraft-cssel-view-{}", std::process::id()));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("view.json")).unwrap()).unwrap();
    assert_eq!(json["kind"], "sample");
    assert_eq!(json["previous_weight"], "yield");
    assert_eq!(json["stages"][0]["estimator"], "EQUAL_WOR");
    assert_eq!(json["stages"][0]["sample"]["method"], "SIMPLE_WOR");
}
