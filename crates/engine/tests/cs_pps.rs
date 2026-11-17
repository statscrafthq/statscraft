//! CSPLAN SAMPLE and CSSELECT with probability proportional to size, and the UNEQUAL_WOR
//! estimator of the analysis commands. The frame is invented: three valleys (strata) of five,
//! six and seven orchards (clusters) with their acres (the measure of size), two to four trees
//! per orchard (cases) and a yield per tree. Inclusion and joint probabilities are checked
//! against closed forms computed here; the UNEQUAL_WOR standard errors against R (package
//! survey, a pps design with the same joint probabilities and variance="YG", run as a black
//! box) and against a hand-computed two-stage formula.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// (valley, orchard, acres, tree, yield) of every tree; orchards are numbered 1 to 18.
fn frame() -> Vec<(u32, u32, f64, u32, f64)> {
    let mut out = Vec::new();
    let mut orchard = 0;
    for valley in 1..=3 {
        for _ in 0..valley + 4 {
            orchard += 1;
            let acres = f64::from(2 + orchard * 7 % 11);
            for tree in 1..=2 + orchard % 3 {
                let y = f64::from(3 * valley + orchard + 2 * tree) + acres / 2.0;
                out.push((valley, orchard, acres, tree, y));
            }
        }
    }
    out
}

/// (orchard, acres) of each orchard of a valley, in frame order.
fn orchards(valley: u32) -> Vec<(u32, f64)> {
    let mut out: Vec<(u32, f64)> = Vec::new();
    for r in frame().iter().filter(|r| r.0 == valley) {
        if out.last().map(|o| o.0) != Some(r.1) {
            out.push((r.1, r.2));
        }
    }
    out
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-cspps-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The frame as inline data and a CD into a fresh folder for plans and files.
fn data(name: &str) -> String {
    let rows: Vec<String> = frame()
        .iter()
        .map(|(v, o, a, t, y)| format!("{v} {o} {a} {t} {y}"))
        .collect();
    format!(
        "DATA LIST FREE /valley orchard acres tree yield.\nBEGIN DATA\n{}\nEND DATA.\nCD '{}'.\n",
        rows.join("\n"),
        folder(name).display()
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

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    let variable = data.variable(name).unwrap_or_else(|_| panic!("no {name}"));
    variable.numeric().unwrap().iter().collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * b.abs().max(1.0)
}

/// The selected orchards of a valley with the inclusion probability and final weight of their
/// first case.
fn selected(engine: &Engine, valley: u32) -> Vec<(u32, f64, f64)> {
    let prob = column(engine, "InclusionProbability_1_");
    let weight = column(engine, "SampleWeight_Final_");
    let mut out: Vec<(u32, f64, f64)> = Vec::new();
    for (case, r) in frame().iter().enumerate() {
        if r.0 == valley
            && let Some(w) = weight[case]
            && out.last().map(|o| o.0) != Some(r.1)
        {
            out.push((r.1, prob[case].unwrap(), w));
        }
    }
    out
}

#[test]
fn pps_wor_gives_n_times_the_size_share_and_handles_a_certainty_unit() {
    // MIN=3 raises the smallest orchards, MAX=45 lowers orchard 12 after it grows by 40 acres
    let source = format!(
        "{}COMPUTE acres = acres + 40 * (orchard = 12).
         CSPLAN SAMPLE /PLAN FILE='wor.json' /DESIGN STRATA=valley CLUSTER=orchard
           /METHOD TYPE=PPS_WOR /SIZE MATRIX=valley; 1 2; 2 3; 3 4 /MOS VARIABLE=acres MIN=3 MAX=45.
         CSSELECT /PLAN FILE='wor.json' /CRITERIA SEED=77.\n",
        data("wor")
    );
    let (engine, _) = run(&source);
    for (valley, n) in [(1u32, 2usize), (2, 3), (3, 4)] {
        let mut sizes: Vec<(u32, f64)> = orchards(valley)
            .into_iter()
            .map(|(o, a)| (o, (a + if o == 12 { 40.0 } else { 0.0 }).clamp(3.0, 45.0)))
            .collect();
        // orchard 12 (valley 3, 45 acres of 92) would have 4 * 45 / 92 > 1: a certainty unit,
        // and the other three draws share the remaining 47 acres
        let mut left = n as f64;
        let mut certain = Vec::new();
        if valley == 3 {
            certain.push(12);
            left -= 1.0;
        }
        sizes.retain(|(o, _)| !certain.contains(o));
        let total: f64 = sizes.iter().map(|s| s.1).sum();
        let got = selected(&engine, valley);
        assert_eq!(got.len(), n, "valley {valley}");
        for (orchard, prob, weight) in got {
            let expected = if certain.contains(&orchard) {
                1.0
            } else {
                left * sizes.iter().find(|s| s.0 == orchard).unwrap().1 / total
            };
            assert!(close(prob, expected), "{orchard}: {prob} {expected}");
            assert!(close(weight, 1.0 / expected));
        }
        if valley == 3 {
            assert!(selected(&engine, 3).iter().any(|s| s.0 == 12));
        }
    }
}

#[test]
fn murthy_writes_its_joint_probabilities_file() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='murthy.json' /DESIGN STRATA=valley CLUSTER=orchard
           /METHOD TYPE=PPS_MURTHY /MOS VARIABLE=acres.
         CSSELECT /PLAN FILE='murthy.json' /CRITERIA SEED=5.\n",
        data("murthy")
    );
    let (mut engine, _) = run(&source);
    let mut picks = Vec::new();
    for valley in 1..=3 {
        let sizes = orchards(valley);
        let total: f64 = sizes.iter().map(|s| s.1).sum();
        let p = |o: u32| sizes.iter().find(|s| s.0 == o).unwrap().1 / total;
        let s: f64 = sizes.iter().map(|o| p(o.0) / (1.0 - p(o.0))).sum();
        let got = selected(&engine, valley);
        assert_eq!(got.len(), 2);
        for &(o, prob, _) in &got {
            let q = p(o);
            assert!(close(prob, q * (1.0 - q / (1.0 - q) + s)));
        }
        let (a, b) = (p(got[0].0), p(got[1].0));
        let joint = a * b * (2.0 - a - b) / ((1.0 - a) * (1.0 - b));
        picks.push((valley, got, joint));
    }
    run_on(&mut engine, "GET FILE='murthy.joint.sav'.");
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(
        names,
        [
            "valley",
            "orchard",
            "Unit_No_",
            "Joint_Prob_1_",
            "Joint_Prob_2_"
        ]
    );
    assert_eq!(data.case_count(), 6);
    let cols: Vec<Vec<Option<f64>>> = names.iter().map(|n| column(&engine, n)).collect();
    for (k, (valley, got, joint)) in picks.iter().enumerate() {
        for (u, (orchard, prob, _)) in got.iter().enumerate() {
            let row = 2 * k + u;
            assert_eq!(cols[0][row], Some(f64::from(*valley)));
            assert_eq!(cols[1][row], Some(f64::from(*orchard)));
            assert_eq!(cols[2][row], Some(u as f64 + 1.0));
            assert!(close(cols[3 + u][row].unwrap(), *prob));
            assert!(close(cols[4 - u][row].unwrap(), *joint));
        }
    }
}

#[test]
fn fromdata_counts_the_cases_and_pps_wr_writes_each_hit() {
    let source = format!(
        "{}CSPLAN SAMPLE /PLAN FILE='wr.json' /DESIGN CLUSTER=orchard
           /METHOD TYPE=PPS_WR /SIZE VALUE=12 /MOS SOURCE=FROMDATA.
         CSSELECT /PLAN FILE='wr.json' /CRITERIA SEED=9 /SAMPLEFILE OUTFILE='wr.sav'.
         GET FILE='wr.sav'.\n",
        data("ppswr")
    );
    let (engine, _) = run(&source);
    let total = frame().len() as f64;
    let orchard = column(&engine, "orchard");
    let prob = column(&engine, "InclusionProbability_1_");
    let weight = column(&engine, "SampleWeight_Final_");
    let index = column(&engine, "Index_1_");
    let mut rows_of = std::collections::HashMap::new();
    for row in 0..orchard.len() {
        let o = orchard[row].unwrap() as u32;
        let trees = frame().iter().filter(|r| r.1 == o).count() as f64;
        let p = trees / total;
        assert!(close(prob[row].unwrap(), 1.0 - (1.0 - p).powi(12)));
        assert!(close(weight[row].unwrap(), 1.0 / (12.0 * p)));
        let entry = rows_of.entry(o).or_insert((0usize, 0.0f64, trees as usize));
        entry.0 += 1;
        entry.1 = entry.1.max(index[row].unwrap());
    }
    // an orchard hit k times writes each of its trees k times, numbered 1 to k; 12 hits in all
    let mut hits = 0;
    for (rows, top, trees) in rows_of.values() {
        assert_eq!(*rows, trees * *top as usize);
        hits += *top as usize;
    }
    assert_eq!(hits, 12);
}

#[test]
fn a_seed_repeats_a_sampford_sample() {
    let plan = "CSPLAN SAMPLE /PLAN FILE='s.json' /DESIGN STRATA=valley CLUSTER=orchard
       /METHOD TYPE=PPS_SAMPFORD /SIZE VALUE=3 /MOS VARIABLE=acres.\n";
    let draw = |seed: u32, name: &str| {
        let (engine, _) = run(&format!(
            "{}{plan}CSSELECT /PLAN FILE='s.json' /CRITERIA SEED={seed}.\n",
            data(name)
        ));
        column(&engine, "SampleWeight_Final_")
    };
    let first = draw(31, "samp1");
    assert_eq!(first, draw(31, "samp2"));
    assert!((32..40).any(|seed| draw(seed, "samp3") != first));
}

/// The total of yield and its standard error estimated by CSDESCRIPTIVES from a drawn sample.
fn estimate(plan: &str, seed: u32, name: &str) -> (f64, f64) {
    let source = format!(
        "{}{plan}CSSELECT /PLAN FILE='est.json' /CRITERIA SEED={seed} /PRINT CPS.
         CSDESCRIPTIVES /PLAN FILE='est.json' /SUMMARY VARIABLES=yield /SUM.\n",
        data(name)
    );
    let (_, tables) = run(&source);
    let t = tables
        .iter()
        .find(|t| t.title == "Univariate Statistics")
        .unwrap();
    let at = |title: &str| {
        let c = t.column_titles().iter().position(|c| c == title).unwrap();
        match t.rows[0][c] {
            Cell::Number(n) => n,
            ref other => panic!("{other:?}"),
        }
    };
    (at("Estimate"), at("Standard Error"))
}

#[test]
fn unequal_wor_estimates_are_unbiased_and_the_variance_follows_the_spread() {
    let truth: f64 = frame().iter().map(|r| r.4).sum();
    for (name, plan) in [
        (
            "pps-two-stage",
            "CSPLAN SAMPLE /PLAN FILE='est.json' /DESIGN STRATA=valley CLUSTER=orchard
               /METHOD TYPE=PPS_WOR /SIZE VALUE=3 /MOS VARIABLE=acres
               /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2.\n",
        ),
        (
            "brewer",
            "CSPLAN SAMPLE /PLAN FILE='est.json' /DESIGN STRATA=valley CLUSTER=orchard
               /METHOD TYPE=PPS_BREWER /MOS VARIABLE=acres.\n",
        ),
    ] {
        let runs: Vec<(f64, f64)> = (1..=300).map(|seed| estimate(plan, seed, name)).collect();
        let r = runs.len() as f64;
        let mean = runs.iter().map(|e| e.0).sum::<f64>() / r;
        let var = runs.iter().map(|e| (e.0 - mean).powi(2)).sum::<f64>() / (r - 1.0);
        let mean_var = runs.iter().map(|e| e.1 * e.1).sum::<f64>() / r;
        // the estimator is unbiased: the mean of 300 independent estimates is within 4
        // standard errors of the truth (a failure by chance about once in 15,000 seed sets;
        // the seeds are fixed, so the test is deterministic)
        assert!(
            (mean - truth).abs() < 4.0 * (var / r).sqrt(),
            "{name}: mean {mean}, truth {truth}"
        );
        // the Yates-Grundy variance is unbiased for the variance of the estimate; with 300
        // runs the two agree within a third (a wrong joint probability or a missing stage
        // term moves the estimated variance by far more)
        assert!(
            (mean_var / var - 1.0).abs() < 1.0 / 3.0,
            "{name}: estimated {mean_var}, spread {var}"
        );
    }
}

/// The units of an invented PPS sample of two strata with their invented inclusion
/// probabilities and a joint probabilities file; the plan analyses them with UNEQUAL_WOR.
fn unequal_sample(name: &str) -> String {
    format!(
        "DATA LIST FREE /h id unit p.
         BEGIN DATA
         1 1 1 .3
         1 2 2 .5
         1 3 3 .6
         1 4 4 .7
         2 5 1 .4
         2 6 2 .5
         2 7 3 .8
         END DATA.
         CD '{}'.
         COMPUTE Joint_Prob_1_ = $SYSMIS.
         RECODE id (1=.3) (2=.12) (3=.15) (4=.18) (5=.4) (6=.15) (7=.30) INTO Joint_Prob_1_.
         RECODE id (1=.12) (2=.5) (3=.28) (4=.33) (5=.15) (6=.5) (7=.38) INTO Joint_Prob_2_.
         RECODE id (1=.15) (2=.28) (3=.6) (4=.40) (5=.30) (6=.38) (7=.8) INTO Joint_Prob_3_.
         RECODE id (1=.18) (2=.33) (3=.40) (4=.7) INTO Joint_Prob_4_.
         SAVE OUTFILE='joint.sav' /KEEP=h id unit Joint_Prob_1_ TO Joint_Prob_4_.
         DATA LIST FREE /h id y x g p.
         BEGIN DATA
         1 1 4 2 0 .3
         1 2 7 3 1 .5
         1 3 5 4 0 .6
         1 4 9 6 1 .7
         2 5 3 1 1 .4
         2 6 8 5 0 .5
         2 7 6 4 1 .8
         END DATA.
         COMPUTE w = 1 / p.
         CSPLAN ANALYSIS /PLAN FILE='uneq.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN STRATA=h CLUSTER=id /ESTIMATOR TYPE=UNEQUAL_WOR.\n",
        folder(name).display()
    )
}

fn number_at(table: &Table, row: usize, title: &str, nth: usize) -> f64 {
    let c = table
        .column_titles()
        .iter()
        .enumerate()
        .filter(|(_, t)| *t == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("{}: no {title} in {:?}", table.title, table.column_titles()))
        .0;
    match table.rows[row][c] {
        Cell::Number(n) => n,
        ref other => panic!("{}: {other:?}", table.title),
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-8 * b.abs()
}

#[test]
fn unequal_wor_standard_errors_match_r() {
    let source = format!(
        "{}CSDESCRIPTIVES /PLAN FILE='uneq.json' /JOINTPROB FILE='joint.sav'
           /SUMMARY VARIABLES=y /MEAN /SUM /RATIO NUMERATOR=y DENOMINATOR=x.
         CSTABULATE /PLAN FILE='uneq.json' /JOINTPROB FILE='joint.sav' /TABLES VARIABLES=g
           /CELLS POPSIZE TABLEPCT.
         CSGLM y WITH x /PLAN FILE='uneq.json' /JOINTPROB FILE='joint.sav'
           /STATISTICS PARAMETER SE.
         CSLOGISTIC g (LOW) WITH x /PLAN FILE='uneq.json' /JOINTPROB FILE='joint.sav'
           /STATISTICS PARAMETER SE.\n",
        unequal_sample("r")
    );
    let (_, tables) = run(&source);
    let find = |title: &str| {
        tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
            panic!(
                "no {title}: {:?}",
                tables.iter().map(|t| &t.title).collect::<Vec<_>>()
            )
        })
    };
    // R: svytotal, svymean, svyratio, svymean(~factor(g)), svytotal(~factor(g)), svyglm with
    // gaussian and quasibinomial families on svydesign(ids=~id, strata=~h, fpc=~p,
    // weights=~w, pps=ppsmat(J), variance="YG"), J holding the joint probabilities above and
    // the products of inclusion probabilities across strata
    let uni = find("Univariate Statistics");
    let rows: Vec<(f64, f64)> = (0..uni.rows.len())
        .map(|r| {
            (
                number_at(uni, r, "Estimate", 0),
                number_at(uni, r, "Standard Error", 0),
            )
        })
        .collect();
    let mean = rows
        .iter()
        .find(|e| near(e.0, 5.60873215785055))
        .expect("the mean");
    assert!(near(mean.1, 0.660030351754934), "{mean:?}");
    let sum = rows
        .iter()
        .find(|e| near(e.0, 79.5238095238095))
        .expect("the sum");
    assert!(near(sum.1, 6.03580328362892), "{sum:?}");
    let ratios = find("Ratios");
    assert!(near(number_at(ratios, 0, "Estimate", 0), 1.75144205558469));
    assert!(near(
        number_at(ratios, 0, "Standard Error", 0),
        0.103414339108053
    ));
    // rows g = 0 and g = 1; the population size, then the table percent
    let tab = find("g");
    for (row, group, estimate, se) in [
        (0, "Population Size", 7.0, 2.65826003649184),
        (1, "Population Size", 7.17857142857143, 2.00434970318138),
        (0, "Table Percent", 49.3702770780856, 15.9246650034078),
        (1, "Table Percent", 50.6297229219144, 15.9246650034078),
    ] {
        assert!(
            near(
                number_at(tab, row, &format!("{group} / Estimate"), 0),
                estimate
            ),
            "{row} {group}"
        );
        assert!(
            near(
                number_at(tab, row, &format!("{group} / Standard Error"), 0),
                se
            ),
            "{row} {group}"
        );
    }
    let glm = tables
        .iter()
        .find(|t| t.title == "Parameter Estimates")
        .unwrap();
    assert!(near(number_at(glm, 0, "Estimate", 0), 1.92829640754121));
    assert!(near(number_at(glm, 0, "Std. Error", 0), 0.248639591203391));
    assert!(near(number_at(glm, 1, "Std. Error", 0), 0.0501850209424946));
    let logit = tables
        .iter()
        .filter(|t| t.title == "Parameter Estimates")
        .nth(1)
        .unwrap();
    assert!(near(number_at(logit, 0, "Std. Error", 0), 1.21273716244411));
    assert!(near(
        number_at(logit, 1, "Std. Error", 0),
        0.241743650210926
    ));
}

/// CSCOXREG on the same design: invented survival times and events added to the seven units.
/// R: svycoxph(Surv(t, s) ~ x) (Efron ties) on the svydesign of the test above.
#[test]
fn cscoxreg_takes_the_joint_probabilities_too() {
    let source = format!(
        "{}RECODE id (1=5) (2=3) (3=8) (4=2) (5=6) (6=4) (7=7) INTO t.
         RECODE id (3, 7=0) (ELSE=1) INTO s.
         CSCOXREG t WITH x /VARIABLES STATUS=s(1) /PLAN FILE='uneq.json'
           /JOINTPROB FILE='joint.sav' /STATISTICS PARAMETER SE.\n",
        unequal_sample("cox")
    );
    let (_, tables) = run(&source);
    let cox = tables
        .iter()
        .find(|t| t.title == "Parameter Estimates")
        .unwrap();
    assert!(near(number_at(cox, 0, "Estimate", 0), 0.141346234181));
    assert!(near(number_at(cox, 0, "Std. Error", 0), 0.123632453558));
}

#[test]
fn unequal_wor_first_stage_and_equal_wor_second_stage_add_up() {
    // three PSUs with inclusion probabilities 0.4, 0.5 and 0.8 (joint 0.15, 0.30, 0.38), half
    // of the cases of each drawn at the second stage
    let dir = folder("two");
    let source = format!(
        "DATA LIST FREE /psu unit Joint_Prob_1_ Joint_Prob_2_ Joint_Prob_3_.
         BEGIN DATA
         1 1 .4 .15 .30
         2 2 .15 .5 .38
         3 3 .30 .38 .8
         END DATA.
         CD '{}'.
         SAVE OUTFILE='two.joint.sav'.
         DATA LIST FREE /psu y.
         BEGIN DATA
         1 2
         1 6
         2 3
         2 5
         2 10
         3 4
         3 8
         END DATA.
         RECODE psu (1=.4) (2=.5) (3=.8) INTO pi.
         COMPUTE w = 1 / (pi * 0.5).
         CSPLAN ANALYSIS /PLAN FILE='two.json' /PLANVARS ANALYSISWEIGHT=w
           /DESIGN CLUSTER=psu /ESTIMATOR TYPE=UNEQUAL_WOR
           /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /INCLPROB VALUE=0.5.
         CSDESCRIPTIVES /PLAN FILE='two.json' /SUMMARY VARIABLES=y /SUM.\n",
        dir.display()
    );
    let (_, tables) = run(&source);
    let uni = tables
        .iter()
        .find(|t| t.title == "Univariate Statistics")
        .unwrap();
    // by hand: the Yates-Grundy sum over the PSU totals of w y, plus, per PSU, its inclusion
    // probability times (1 - 0.5) n s^2 of the values w y of its cases
    let pi = [0.4, 0.5, 0.8];
    let joint = [[0.4, 0.15, 0.30], [0.15, 0.5, 0.38], [0.30, 0.38, 0.8]];
    let cases: [&[f64]; 3] = [&[2.0, 6.0], &[3.0, 5.0, 10.0], &[4.0, 8.0]];
    let z: Vec<Vec<f64>> = (0..3)
        .map(|i| cases[i].iter().map(|y| y / (pi[i] * 0.5)).collect())
        .collect();
    let totals: Vec<f64> = z.iter().map(|v| v.iter().sum()).collect();
    let mut v = 0.0;
    for i in 0..3 {
        for j in i + 1..3 {
            v += (pi[i] * pi[j] / joint[i][j] - 1.0) * (totals[i] - totals[j]).powi(2);
        }
        let n = z[i].len() as f64;
        let mean = totals[i] / n;
        let s2 = z[i].iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        v += pi[i] * 0.5 * n * s2;
    }
    assert!(near(number_at(uni, 0, "Estimate", 0), totals.iter().sum()));
    assert!(near(number_at(uni, 0, "Standard Error", 0), v.sqrt()));
}

#[test]
fn a_missing_joint_file_or_unit_is_an_error() {
    let mut engine = Engine::default();
    run_on(&mut engine, &unequal_sample("missing"));
    let (commands, _) = parse("CSDESCRIPTIVES /PLAN FILE='uneq.json' /SUMMARY VARIABLES=y /SUM.");
    let error = engine.execute(&commands[0].command).unwrap_err().0;
    assert!(error.contains("uneq.joint.sav"), "{error}");
    run_on(
        &mut engine,
        "SAVE OUTFILE='sample.sav'.
         GET FILE='joint.sav'.
         SELECT IF id < 7.
         SAVE OUTFILE='short.sav'.
         GET FILE='sample.sav'.",
    );
    let (commands, _) = parse(
        "CSDESCRIPTIVES /PLAN FILE='uneq.json' /JOINTPROB FILE='short.sav' /SUMMARY VARIABLES=y /SUM.",
    );
    let error = engine.execute(&commands[0].command).unwrap_err().0;
    assert!(
        error.contains("no row for the first-stage unit 2 7"),
        "{error}"
    );
}
