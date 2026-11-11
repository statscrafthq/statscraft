//! Parser tests of DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, EXAMINE, ADD FILES,
//! MATCH FILES and STAR JOIN.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> SyntaxError {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0)
}

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

#[test]
fn descriptives() {
    use DescriptiveStatistic as S;
    assert_eq!(
        one("DESCRIPTIVES age income."),
        Command::Descriptives(Descriptives {
            variables: vec![(var("age"), None), (var("income"), None)],
            statistics: vec![S::Mean, S::StdDev, S::Min, S::Max],
            listwise: false,
            include: false,
            save: false,
        })
    );
    assert_eq!(
        one(
            "DESCRIPTIVES /VAR=a TO c, d (zd) /STAT=SUM MEAN kurtosis sekurt\n /MISSING=LISTWISE INCLUDE /SAVE."
        ),
        Command::Descriptives(Descriptives {
            variables: vec![
                (VarItem::Range("a".into(), "c".into()), None),
                (var("d"), Some("zd".into()))
            ],
            statistics: vec![S::Mean, S::Kurtosis, S::Sum],
            listwise: true,
            include: true,
            save: true,
        })
    );
    let Command::Descriptives(d) = one("DESCRIPTIVES x /STATISTICS=DEFAULT VARIANCE ALL.") else {
        panic!()
    };
    assert_eq!(d.statistics.len(), 10);

    assert!(
        err("DESCRIPTIVES x /SORT=MEAN (A).")
            .message
            .contains("SORT of DESCRIPTIVES is not supported yet")
    );
    let e = err("DESCRIPTIVES x /STATISTICS=PHI.");
    assert!(e.message.contains("unexpected word PHI"), "{e}");
    assert!(e.expected.contains(&"SEMEAN".to_string()));
    assert!(
        err("DESCRIPTIVES a TO c (z).")
            .message
            .contains("single variable")
    );
    assert!(
        err("DESCRIPTIVES /STATISTICS=MEAN.")
            .expected
            .contains(&"VARIABLES=varlist".to_string())
    );
    assert!(
        err("DESCRIPTIVES x /MISSING=NONE.")
            .expected
            .contains(&"LISTWISE".to_string())
    );
}

#[test]
fn crosstabs() {
    use CrosstabsCell as C;
    assert_eq!(
        one("CROSSTABS a b BY c."),
        Command::Crosstabs(Crosstabs {
            tables: vec![vec![vec![var("a"), var("b")], vec![var("c")]]],
            cells: vec![C::Count],
            chisq: false,
            include: false,
            descending: false,
            count: WeightCount::Asis,
            truncate: false,
        })
    );
    assert_eq!(
        one(
            "CROSSTABS /TABLES=a BY c BY d /TABLES=x BY y\n /CELLS=SRESID count ROW /STAT=CHISQ /MISSING=INCLUDE /FORMAT=DVALUE."
        ),
        Command::Crosstabs(Crosstabs {
            tables: vec![
                vec![vec![var("a")], vec![var("c")], vec![var("d")]],
                vec![vec![var("x")], vec![var("y")]]
            ],
            cells: vec![C::Count, C::Row, C::Sresid],
            chisq: true,
            include: true,
            descending: true,
            count: WeightCount::Asis,
            truncate: false,
        })
    );
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /CELLS=NONE.") else {
        panic!()
    };
    assert!(c.cells.is_empty());

    let e = err("CROSSTABS a BY b /STATISTICS=CHISQ PHI.");
    assert_eq!(
        e.message,
        "PHI in CROSSTABS /STATISTICS is not supported yet"
    );
    assert_eq!(e.expected, ["CHISQ", "NONE"]);
    assert!(
        err("CROSSTABS a BY b /MISSING=REPORT.")
            .message
            .contains("REPORT in CROSSTABS /MISSING")
    );
    assert!(
        err("CROSSTABS a BY b /CELLS=BPROP.")
            .message
            .contains("not supported yet")
    );
    assert!(
        err("CROSSTABS a BY b /CELLS=BOGUS.")
            .message
            .contains("unexpected word BOGUS")
    );
    assert!(
        err("CROSSTABS a BY b /WRITE=CELLS.")
            .message
            .contains("WRITE of CROSSTABS is not supported yet")
    );
    assert_eq!(err("CROSSTABS a b.").expected, ["BY"]);
    assert_eq!(err("CROSSTABS /TABLES=a.").expected, ["BY"]);
    assert!(
        err("CROSSTABS a BY b /FORMAT=NOBOX.")
            .message
            .contains("NOBOX in CROSSTABS /FORMAT is not supported yet")
    );
}

#[test]
fn crosstabs_format_tables_is_the_default_and_notables_is_cells_none() {
    let crosstabs = |source: &str| match one(source) {
        Command::Crosstabs(c) => c,
        other => panic!("{other:?}"),
    };
    let tables = crosstabs("CROSSTABS a BY b /FORMAT=AVALUE TABLES.");
    assert_eq!(tables.cells, [CrosstabsCell::Count]);
    assert!(!tables.descending);
    let none = crosstabs("CROSSTABS a BY b /CELLS=COUNT ROW /FORMAT=DVALUE NOTABLES /STAT=CHISQ.");
    assert!(none.cells.is_empty() && none.descending && none.chisq);
    let count = crosstabs("CROSSTABS a BY b /COUNT=CASE TRUNCATE.");
    assert_eq!((count.count, count.truncate), (WeightCount::Case, true));
    let count = crosstabs("CROSSTABS a BY b /COUNT ROUND.");
    assert_eq!((count.count, count.truncate), (WeightCount::Cell, false));
    assert!(
        err("CROSSTABS a BY b /COUNT=SOME.")
            .message
            .contains("unexpected word SOME")
    );
    // the last of TABLES and NOTABLES counts
    assert!(crosstabs("CROSSTABS a BY b /FORMAT=NOTABLES TABLES.").cells == [CrosstabsCell::Count]);
}

#[test]
fn add_and_match_files() {
    assert_eq!(
        one("ADD FILES /FILE=* /RENAME=(a b = c d) x=y /FILE='b.sav' /BY id /KEEP=id c /DROP=d."),
        Command::AddFiles(Merge {
            inputs: vec![
                MergeInput {
                    file: None,
                    table: false,
                    rename: vec![
                        (vec![var("a"), var("b")], vec![var("c"), var("d")]),
                        (vec![var("x")], vec![var("y")])
                    ],
                },
                MergeInput {
                    file: Some("b.sav".into()),
                    table: false,
                    rename: vec![],
                },
            ],
            by: vec![var("id")],
            subsets: vec![
                Subset::Keep(vec![var("id"), var("c")]),
                Subset::Drop(vec![var("d")])
            ],
        })
    );
    let Command::MatchFiles(m) = one("MATCH FILES FILE=* /TABLE='t.sav' /BY k.") else {
        panic!()
    };
    assert!(m.inputs[1].table && !m.inputs[0].table);

    assert!(
        err("ADD FILES /FILE=* /TABLE='t.sav'.")
            .message
            .contains("unknown subcommand TABLE")
    );
    assert!(
        err("MATCH FILES /FILE=* /TABLE='t.sav'.")
            .message
            .contains("needs /BY")
    );
    assert!(
        err("MATCH FILES /FILE=* /FILE='b.sav' /IN=src.")
            .message
            .contains("IN of MATCH FILES is not supported yet")
    );
    assert!(
        err("MATCH FILES /RENAME=(a=b) /FILE=*.")
            .message
            .contains("must follow")
    );
    assert!(
        err("MATCH FILES /TABLE='t.sav' /BY k.")
            .expected
            .contains(&"/FILE='path', /FILE=dataset or /FILE=*".to_string())
    );
    // a dataset name may stand without quotes
    let Command::MatchFiles(m) = one("MATCH FILES /FILE=wave1 /TABLE=regions /BY k.") else {
        panic!()
    };
    assert_eq!(m.inputs[0].file.as_deref(), Some("wave1"));
    assert_eq!(m.inputs[1].file.as_deref(), Some("regions"));
}

#[test]
fn means() {
    use MeansCell as M;
    assert_eq!(
        one("MEANS age BY gender BY region / income BY gender."),
        Command::Means(Means {
            tables: vec![
                vec![vec![var("age")], vec![var("gender")], vec![var("region")]],
                vec![vec![var("income")], vec![var("gender")]],
            ],
            cells: vec![M::Mean, M::Count, M::StdDev],
            anova: false,
            linearity: false,
            missing: MeansMissing::Separate,
            within: Vec::new(),
        })
    );
    let Command::Means(m) = one(
        "MEANS TABLES=a TO c BY g /CELLS=MEDIAN MEAN MEDIAN SUM /STATISTICS /MISSING=DEPENDENT.",
    ) else {
        panic!()
    };
    assert_eq!(m.cells, [M::Median, M::Mean, M::Sum]);
    assert!(m.anova && !m.linearity);
    assert_eq!(m.missing, MeansMissing::Dependent);
    let Command::Means(m) = one("MEANS x /CELLS=ALL /STATISTICS=LINEARITY.") else {
        panic!()
    };
    assert_eq!(m.cells.len(), 21);
    assert!(m.anova && m.linearity);
    let Command::Means(m) =
        one("MEANS x BY g BY h /CELLS=GMEDIAN NPCT(h) SPCT(H) NPCT SPCT(g), NPCT(h).")
    else {
        panic!()
    };
    assert_eq!(
        m.cells,
        [
            M::GroupedMedian,
            M::NpctIn(0),
            M::SpctIn(0),
            M::Npct,
            M::SpctIn(1)
        ]
    );
    assert_eq!(m.within, ["h", "g"]);
    err("MEANS x BY g /CELLS=NPCT(g.");
}

#[test]
fn t_test() {
    let Command::TTest(t) = one("T-TEST /TESTVAL=322 /VARIABLES=brake /CRITERIA=CIN(.90).") else {
        panic!()
    };
    assert_eq!((t.testval, t.ci), (Some(322.0), 0.9));
    assert!(t.effect_sizes);
    let Command::TTest(t) =
        one("T-TEST GROUPS=insert(0 1) /MISSING=LISTWISE INCLUDE /VARIABLES=d.")
    else {
        panic!()
    };
    assert_eq!(
        t.groups,
        Some((
            "insert".into(),
            TTestGroups::Two(Value::Number(0.0), Value::Number(1.0))
        ))
    );
    assert!(t.listwise && t.include);
    let Command::TTest(t) = one("T-TEST GROUPS=age(50) /VARIABLES=d.") else {
        panic!()
    };
    assert_eq!(t.groups.unwrap().1, TTestGroups::Cut(Value::Number(50.0)));
    let Command::TTest(t) = one(
        "T-TEST PAIRS=a b WITH c d (PAIRED) / e f /ES DISPLAY(FALSE) STANDARDIZER(CORRECTED_SD).",
    ) else {
        panic!()
    };
    assert_eq!(t.pairs.len(), 2);
    assert!(t.pairs[0].paired && t.pairs[1].right.is_none());
    assert!(!t.effect_sizes);
    assert_eq!(t.standardizer, Standardizer::CorrectedSd);
    assert!(err("T-TEST PAIRS=a b (PAIRED).").message.contains("WITH"));
    assert!(err("T-TEST /VARIABLES=a.").message.contains("TESTVAL"));
    assert!(err("T-TEST /TESTVAL=1.").message.contains("VARIABLES"));
    assert!(
        err("T-TEST /TESTVAL=1 /VARIABLES=a /CRITERIA=CI(95).")
            .message
            .contains("between 0 and 1")
    );
}

#[test]
fn oneway() {
    let Command::Oneway(o) = one(
        "ONEWAY a b BY g /CONTRAST=-1 1 2*0 /POSTHOC=TUKEY LSD GH ALPHA(.01) /STATISTICS=ALL /MISSING=LISTWISE.",
    ) else {
        panic!()
    };
    assert_eq!(o.dependents, [var("a"), var("b")]);
    assert_eq!(o.factor, "g");
    assert_eq!(o.contrasts, [vec![-1.0, 1.0, 0.0, 0.0]]);
    assert_eq!(
        o.posthoc,
        [Posthoc::Tukey, Posthoc::Lsd, Posthoc::GamesHowell]
    );
    assert_eq!(o.alpha, 0.01);
    assert!(o.descriptives && o.homogeneity && o.welch && o.brown_forsythe && o.listwise);
    let Command::Oneway(o) = one(
        "ONEWAY a BY g /POSTHOC=SNK BTUK DUNCAN GT2 GAB FREGW QREGW T3 C DUNNETT DUNNETTL(2) DUNNETTR ALPHA=.1.",
    ) else {
        panic!()
    };
    use DunnettSide as D;
    assert_eq!(
        o.posthoc,
        [
            Posthoc::Snk,
            Posthoc::TukeyB,
            Posthoc::Duncan,
            Posthoc::Gt2,
            Posthoc::Gabriel,
            Posthoc::Fregw,
            Posthoc::Qregw,
            Posthoc::T3,
            Posthoc::C,
            Posthoc::Dunnett(None, D::Both),
            Posthoc::Dunnett(Some(2), D::Less),
            Posthoc::Dunnett(None, D::Greater),
        ]
    );
    assert_eq!(o.alpha, 0.1);
    // DUNCAN and the Dunnett keywords must be spelled out
    assert!(err("ONEWAY a BY g /POSTHOC=DUNC.").message.contains("DUNC"));
    assert!(
        err("ONEWAY a BY g /POSTHOC=DUNNET.")
            .message
            .contains("DUNNET")
    );
    assert!(
        err("ONEWAY a BY g /POSTHOC=DUNNETT(0).")
            .message
            .contains("reference category")
    );
    assert!(
        err("ONEWAY a BY g /POSTHOC=WALLER(100).")
            .message
            .contains("WALLER in ONEWAY /POSTHOC is not supported yet")
    );
    assert!(
        err("ONEWAY a BY g /POLYNOMIAL=2.")
            .message
            .contains("POLYNOMIAL of ONEWAY is not supported yet")
    );
    assert!(
        err("ONEWAY a BY g /STATISTICS=EFFECTS.")
            .message
            .contains("EFFECTS")
    );
}

#[test]
fn correlations() {
    let Command::Correlations(c) = one(
        "CORRELATIONS /VARIABLES=a b WITH c /d e /PRINT=ONETAIL NOSIG LOWER /STATISTICS=XPROD /MISSING=LISTWISE.",
    ) else {
        panic!()
    };
    assert_eq!(
        c.lists,
        [
            (vec![var("a"), var("b")], Some(vec![var("c")])),
            (vec![var("d"), var("e")], None)
        ]
    );
    assert!(c.one_tailed && c.flag && c.xprod && c.listwise && !c.descriptives);
    assert_eq!(c.layout, CorrelationsLayout::Lower);
    let Command::Correlations(c) = one("CORRELATIONS a b c.") else {
        panic!()
    };
    assert_eq!(c.lists.len(), 1);
    assert!(
        err("CORRELATIONS a b /MATRIX=OUT(*).")
            .message
            .contains("MATRIX of CORRELATIONS is not supported yet")
    );
}

#[test]
fn nonpar_corr() {
    let Command::NonparCorr(c) = one(
        "NONPAR CORR /VARIABLES=a b WITH c /d e /PRINT=BOTH ONETAIL NOSIG LOWER /MISSING=LISTWISE INCLUDE /SAMPLE.",
    ) else {
        panic!()
    };
    assert_eq!(
        c.lists,
        [
            (vec![var("a"), var("b")], Some(vec![var("c")])),
            (vec![var("d"), var("e")], None)
        ]
    );
    assert!(c.spearman && c.kendall && c.one_tailed && c.flag && c.listwise && c.include);
    assert_eq!(c.layout, CorrelationsLayout::Lower);
    let Command::NonparCorr(c) = one("NONPAR CORR a b c /PRINT=KENDALL.") else {
        panic!()
    };
    assert!(!c.spearman && c.kendall && !c.listwise && !c.flag);
    let Command::NonparCorr(c) = one("NONPAR CORR a b.") else {
        panic!()
    };
    assert!(c.spearman && !c.kendall);
    assert!(
        err("NONPAR CORR a b /CI METHOD(FHP).")
            .message
            .contains("CI of NONPAR CORR is not supported yet")
    );
}

#[test]
fn examine_defaults_and_factor_terms() {
    let Command::Examine(e) = one("EXAMINE VARIABLES=a b BY f, g BY h.") else {
        panic!()
    };
    assert_eq!(e.dependents, [var("a"), var("b")]);
    assert_eq!(e.factors, [vec![var("f")], vec![var("g"), var("h")]]);
    assert!(e.total && e.descriptives && !e.normality && !e.pairwise && !e.include && !e.report);
    assert_eq!((e.percentiles, e.extreme, e.cinterval), (None, None, 95.0));
    assert_eq!(e.method, PercentileMethod::Haverage);
    assert!(e.mestimators.is_empty() && e.id.is_none());
    let Command::Examine(e) = one("EXAMINE a /PERCENTILES /MESTIMATORS /NOTOTAL /ID=name.") else {
        panic!()
    };
    assert_eq!(
        e.percentiles,
        Some(vec![5.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0])
    );
    use MEstimator as M;
    assert_eq!(
        e.mestimators,
        [
            M::Huber(1.339),
            M::Tukey(4.685),
            M::Hampel(1.7, 3.4, 8.5),
            M::Andrew(1.34)
        ]
    );
    assert!(!e.total);
    assert_eq!(e.id.as_deref(), Some("name"));
}

#[test]
fn examine_subcommands() {
    let Command::Examine(e) = one(
        "EXAMINE VARIABLES=a /PERCENTILES(90, 2.5, 50)=WAVERAGE /STATISTICS=EXTREME(3)\n\
         /CINTERVAL 90 /MESTIMATORS=ANDREW(2) HUBER(1.5) /PLOT=NPPLOT /COMPARE=VARIABLES\n\
         /MISSING=PAIRWISE INCLUDE REPORT.",
    ) else {
        panic!()
    };
    assert_eq!(e.percentiles, Some(vec![2.5, 50.0, 90.0]));
    assert_eq!(e.method, PercentileMethod::Waverage);
    assert!(!e.descriptives && e.normality && e.pairwise && e.include && e.report);
    assert_eq!((e.extreme, e.cinterval), (Some(3), 90.0));
    assert_eq!(
        e.mestimators,
        [MEstimator::Huber(1.5), MEstimator::Andrew(2.0)]
    );
    let Command::Examine(e) = one(
        "EXAMINE a /STATISTICS=ALL /PERCENTILES=AEMPIRICAL /MESTIMATORS=HAMPEL(2,4,8) ALL NONE TUKEY.",
    ) else {
        panic!()
    };
    assert!(e.descriptives);
    assert_eq!(e.extreme, Some(5));
    assert_eq!(e.method, PercentileMethod::Aempirical);
    assert_eq!(e.mestimators, [MEstimator::Tukey(4.685)]);
    let Command::Examine(e) = one("EXAMINE a /PERCENTILES NONE /STATISTICS NONE /PLOT NONE.")
    else {
        panic!()
    };
    assert!(e.percentiles.is_none() && !e.descriptives && e.extreme.is_none());
}

#[test]
fn examine_errors() {
    let message = |source: &str| err(source).message;
    // plots are accepted and kept, to warn that they are not drawn
    let plots = |source: &str| match one(source) {
        Command::Examine(e) => (e.plots, e.normality),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        plots("EXAMINE a /PLOT=BOXPLOT HISTOGRAM."),
        (vec!["BOXPLOT".to_string(), "HISTOGRAM".to_string()], false)
    );
    assert!(plots("EXAMINE a /PLOT=STEMLEAF NPPLOT.").1);
    assert_eq!(plots("EXAMINE a /PLOT=ALL.").0.len(), 3);
    assert!(plots("EXAMINE a /PLOT=BOXPLOT NONE.").0.is_empty());
    assert!(message("EXAMINE a /PLOT=SPREADLEVEL(0).").contains("not supported yet"));
    assert!(message("EXAMINE a BY f BY g BY h.").contains("more than two crossed factors"));
    assert!(message("EXAMINE a /PERCENTILES(0, 50).").contains("between 0 and 100"));
    assert!(message("EXAMINE a /MESTIMATORS=HAMPEL(3,2,8).").contains("0 < a <= b <= c"));
    assert!(message("EXAMINE a /MESTIMATORS=HUBER(1,2).").contains("takes 1 constant"));
    assert!(message("EXAMINE a /STATISTICS=EXTREME(0).").contains("positive whole number"));
    assert!(message("EXAMINE a /FOO.").contains("unknown subcommand FOO of EXAMINE"));
    assert_eq!(err("EXAMINE /ID=x.").expected, ["VARIABLES=dependents"]);
}

fn freq(source: &str) -> Frequencies {
    match one(source) {
        Command::Frequencies(f) => f,
        other => panic!("{other:?}"),
    }
}

#[test]
fn frequencies_subcommands_and_abbreviations() {
    use FrequencyStatistic as F;
    let f = freq(
        "FRE VAR=a b /FOR=NOTABLE DFREQ /MIS=INCLUDE /STA=MEAN MED SEKURT\n /PER=10, 90 25 /NTI=4 /ORD=VAR.",
    );
    assert_eq!(f.variables, [var("a"), var("b")]);
    assert!(f.notable && f.include && f.by_variable);
    assert_eq!(f.order, FrequencyOrder::DFreq);
    assert_eq!(f.statistics, [F::Mean, F::Median, F::SeKurtosis]);
    assert_eq!(f.percentiles, [10.0, 25.0, 50.0, 75.0, 90.0]);
    // STATISTICS alone and DEF are the default four; NONE drops what came before; /STATS works
    let default = [F::Mean, F::StdDev, F::Minimum, F::Maximum];
    assert_eq!(freq("FREQUENCIES a /STATISTICS.").statistics, default);
    assert_eq!(
        freq("FREQUENCIES a /STATS=DEF MODE.").statistics,
        [F::Mean, F::Mode, F::StdDev, F::Minimum, F::Maximum]
    );
    assert_eq!(freq("FREQUENCIES a /STATISTICS=ALL.").statistics.len(), 14);
    assert!(
        freq("FREQUENCIES a /STATISTICS=ALL NONE.")
            .statistics
            .is_empty()
    );
    assert_eq!(
        freq("FREQUENCIES a /STATISTICS=SKEW SESKEW KURT VAR SEM RAN SUM.").statistics,
        [
            F::SeMean,
            F::Variance,
            F::Skewness,
            F::SeSkewness,
            F::Kurtosis,
            F::Range,
            F::Sum
        ]
    );
    // several NTILES add up, duplicates once
    assert_eq!(
        freq("FREQUENCIES a /NTILES=4 /NTILES=2 /PERCENTILES=0 100.").percentiles,
        [0.0, 25.0, 50.0, 75.0, 100.0]
    );
    assert_eq!(freq("FREQUENCIES a /FORMAT=LIMIT(10).").limit, Some(10));
    // subcommands before VARIABLES
    assert_eq!(
        freq("FREQUENCIES /FORMAT=AFREQ /VARIABLES=x.").order,
        FrequencyOrder::AFreq
    );
}

#[test]
fn frequencies_grouped_and_charts() {
    let f = freq(
        "FREQUENCIES age income temp /GROUPED=age, income /GROUPED=temp (0.5)\n /GROUPED=w (17.5, 22.5 27.5) / v.",
    );
    assert_eq!(
        f.grouped,
        [
            (vec![var("age"), var("income")], Grouping::Midpoints),
            (vec![var("temp")], Grouping::Width(0.5)),
            (vec![var("w")], Grouping::Boundaries(vec![17.5, 22.5, 27.5])),
            (vec![var("v")], Grouping::Midpoints),
        ]
    );
    let f = freq("FREQUENCIES a /BARCHART /PIE=MIN(1) MAX(10) PERCENT NOMISSING /BAR=FREQ(50).");
    assert_eq!(f.charts, ["BARCHART", "PIECHART"]);
    let f = freq("FREQUENCIES a /HIST=NORMAL FREQ(20) MINIMUM(0).");
    assert_eq!(f.charts, ["HISTOGRAM"]);
}

#[test]
fn frequencies_errors() {
    let message = |source: &str| err(source).message;
    assert!(message("FREQUENCIES a /BARCHART /HISTOGRAM.").contains("cannot be used together"));
    assert!(message("FREQUENCIES a /PERCENTILES=101.").contains("between 0 and 100"));
    assert!(message("FREQUENCIES a /NTILES=0.").contains("at least 1"));
    assert!(message("FREQUENCIES a /NTILES=2.5.").contains("whole number"));
    assert!(message("FREQUENCIES a /GROUPED=a (0).").contains("positive"));
    assert!(message("FREQUENCIES a /GROUPED=a (3 2 1).").contains("ascending"));
    assert!(message("FREQUENCIES a /GROUPED=a ().").contains("width or the group boundaries"));
    assert_eq!(err("FREQUENCIES a /FORMAT=LIMIT.").expected, ["'('"]);
    assert_eq!(err("FREQUENCIES a /MISSING=EXCLUDE.").expected, ["INCLUDE"]);
    assert!(
        err("FREQUENCIES a /STATISTICS=MEANS.")
            .expected
            .contains(&"MEDIAN".to_string())
    );
    assert_eq!(
        err("FREQUENCIES /FORMAT=NOTABLE.").expected,
        ["VARIABLES=varlist"]
    );
    assert_eq!(
        err("FREQUENCIES a /PIECHART=FREQ(3).").message,
        "unexpected '('"
    );
}

fn rank_of(source: &str) -> Rank {
    match one(source) {
        Command::Rank(r) => r,
        other => panic!("{other:?}"),
    }
}

#[test]
fn rank_variables_functions_and_options() {
    let r = rank_of("RANK salary.");
    assert_eq!(r.variables, [(var("salary"), false)]);
    assert_eq!(r.functions, [(RankFunction::Rank, vec![])]);
    assert!(r.print && !r.include && r.fraction.is_none());
    assert_eq!(r.ties, RankTies::Mean);
    // A or D applies back to the previous A or D
    let r = rank_of("RANK VARIABLES=a b (D) c (A) d BY g h.");
    assert_eq!(
        r.variables,
        [
            (var("a"), true),
            (var("b"), true),
            (var("c"), false),
            (var("d"), false)
        ]
    );
    assert_eq!(r.by, [var("g"), var("h")]);
    let r = rank_of(
        "RANK x /NORMAL INTO xn /SAVAGE /NTILES(4) INTO q1, q2 /N /TIES=CONDENSE /FRACTION=VW\n /PRINT=NO /MISSING=INCLUDE.",
    );
    assert_eq!(
        r.functions,
        [
            (RankFunction::Normal, vec!["xn".to_string()]),
            (RankFunction::Savage, vec![]),
            (
                RankFunction::Ntiles(4),
                vec!["q1".to_string(), "q2".to_string()]
            ),
            (RankFunction::N, vec![]),
        ]
    );
    assert_eq!(r.ties, RankTies::Condense);
    assert_eq!(r.fraction, Some(RankFraction::Vw));
    assert!(!r.print && r.include);
    // three-letter abbreviations
    let r = rank_of("RAN VAR=x /RFR /PER /PRO /TIE=HIG /FRA=RANKIT /PRI=YES /MIS=EXC.");
    assert_eq!(
        r.functions.iter().map(|f| f.0).collect::<Vec<_>>(),
        [
            RankFunction::Rfraction,
            RankFunction::Percent,
            RankFunction::Proportion
        ]
    );
    assert_eq!(r.ties, RankTies::High);
    assert_eq!(r.fraction, Some(RankFraction::Rankit));
}

#[test]
fn rank_errors() {
    let message = |source: &str| err(source).message;
    assert!(message("RANK x /RANK /RANK.").contains("given twice"));
    assert!(message("RANK x /NTILES(2) /NTILES(3).").contains("given twice"));
    assert!(message("RANK x /NTILES.").contains("NTILES(4)"));
    assert!(message("RANK x /NTILES(0).").contains("NTILES(4)"));
    assert!(message("RANK x /FRACTION=BLOM /FRACTION=VW.").contains("given twice"));
    assert_eq!(err("RANK x (B).").expected, ["A", "D"]);
    assert_eq!(
        err("RANK x /TIES=MEDIAN.").expected,
        ["MEAN", "LOW", "HIGH", "CONDENSE"]
    );
    assert_eq!(err("RANK x /RANK INTO.").expected, ["a new variable name"]);
    assert!(message("RANK x /FOO.").contains("unknown subcommand FOO of RANK"));
    assert_eq!(err("RANK.").expected, ["a variable name"]);
}

fn tree_of(source: &str) -> Tree {
    match one(source) {
        Command::Tree(tree) => *tree,
        other => panic!("{other:?}"),
    }
}

#[test]
fn tree_defaults_and_levels() {
    let t = tree_of("TREE risk BY income [o] age [S] ALL [n] FORCE = region [c].");
    assert_eq!((t.dependent.as_str(), t.dependent_level), ("risk", None));
    assert_eq!(
        t.predictors,
        [
            (var("income"), Some(Level::Ordinal)),
            (var("age"), Some(Level::Scale)),
            (VarItem::All, Some(Level::Nominal)),
        ]
    );
    assert_eq!(t.force, Some(("region".into(), Some(Level::Nominal))));
    assert!(!t.exhaustive && t.display && t.branch_statistics && t.node_definitions);
    assert!(t.print.model_summary && t.print.classification && t.print.risk);
    assert!(t.print.case_summary && !t.print.tree_table);
    assert_eq!(
        (t.max_depth, t.min_parent, t.min_child),
        (None, 100.0, 50.0)
    );
    assert_eq!(
        (t.alpha_split, t.alpha_merge, t.intervals),
        (0.05, 0.05, 10)
    );
    assert!(t.bonferroni && !t.likelihood_ratio && t.gain_table && t.gain_cumulative);
}

#[test]
fn tree_subcommands() {
    let t = tree_of(
        "TREE churn [n] BY region [n] age_group [o] spend [s]
          /TREE DISPLAY=NONE NODES=BOTH BRANCHSTATISTICS=NO NODEDEFS=NO SCALE=50
          /DEPCATEGORIES USEVALUES=[VALID -1 MISSING] TARGET=[1 2]
          /PRINT MODELSUMMARY TREETABLE
          /GAIN CATEGORYTABLE=YES TYPE=[NODE] SORT=ASCENDING CUMULATIVE=NO
          /SAVE NODEID(node) PREDVAL PREDPROB(prob) ASSIGNMENT
          /METHOD TYPE=EXHAUSTIVECHAID MAXSURROGATES=AUTO PRUNE=NONE
          /GROWTHLIMIT MAXDEPTH=4 MINPARENTSIZE=40 MINCHILDSIZE=20
          /VALIDATION TYPE=NONE OUTPUT=BOTHSAMPLES
          /CHAID ALPHASPLIT=0.01 ALPHAMERGE=1 SPLITMERGED=NO CHISQUARE=LR CONVERGE=0.01
                 MAXITERATIONS=50 ADJUST=NONE INTERVALS=spend (5) age_group (3)
          /COSTS EQUAL /SCORES CUSTOM=1 [1] 2 [2.5] /MISSING NOMINALMISSING=VALID
          /PLOT GAIN /PRIORS EQUAL.",
    );
    assert!(!t.display && !t.branch_statistics && !t.node_definitions && t.exhaustive);
    assert_eq!(
        t.use_values,
        Some(vec![
            TreeCategory::Valid,
            TreeCategory::Value(Value::Number(-1.0)),
            TreeCategory::Missing
        ])
    );
    assert_eq!(t.targets, [Value::Number(1.0), Value::Number(2.0)]);
    assert!(t.print.model_summary && t.print.tree_table && !t.print.risk);
    assert!(t.gain_ascending && !t.gain_cumulative);
    assert_eq!(
        t.save,
        [
            (TreeSave::NodeId, Some("node".into())),
            (TreeSave::PredictedValue, None),
            (TreeSave::PredictedProbability, Some("prob".into())),
        ]
    );
    assert_eq!(
        (t.max_depth, t.min_parent, t.min_child),
        (Some(4), 40.0, 20.0)
    );
    assert_eq!(
        (t.alpha_split, t.alpha_merge, t.converge),
        (0.01, 1.0, 0.01)
    );
    assert!(t.likelihood_ratio && !t.bonferroni && t.nominal_missing_valid);
    assert_eq!(t.max_iterations, 50);
    assert_eq!(
        t.variable_intervals,
        [(vec![var("spend")], 5), (vec![var("age_group")], 3)]
    );
    assert_eq!(
        t.scores,
        Some(vec![(Value::Number(1.0), 1.0), (Value::Number(2.0), 2.5)])
    );
    // MAXSURROGATES, ASSIGNMENT, PLOT and PRIORS are accepted and said to be ignored
    assert_eq!(t.ignored.len(), 4);
}

#[test]
fn tree_errors() {
    let message = |source: &str| err(source).message;
    assert_eq!(
        message("TREE y BY x /METHOD TYPE=QUEST."),
        "METHOD TYPE=QUEST is not supported yet"
    );
    assert_eq!(
        message("TREE y BY x /VALIDATION TYPE=SPLITSAMPLE(50)."),
        "VALIDATION TYPE=SPLITSAMPLE is not supported yet"
    );
    assert_eq!(
        message("TREE y BY x /CHAID SPLITMERGED=YES."),
        "CHAID SPLITMERGED=YES is not supported yet"
    );
    assert_eq!(
        message("TREE y BY x /RULES NODES=ALL."),
        "subcommand RULES of TREE is not supported yet"
    );
    assert!(message("TREE y BY x /COSTS CUSTOM=1 2 [3].").contains("COSTS CUSTOM"));
    assert!(message("TREE y BY x /GAIN TYPE=[PTILE].").contains("PTILE"));
    assert!(message("TREE y BY x /PRINT NONE RISK.").contains("NONE"));
    assert!(message("TREE y BY x /GROWTHLIMIT MINPARENTSIZE=50.").contains("MINCHILDSIZE"));
    assert!(message("TREE y BY x /PRINT RISK /PRINT CPS.").contains("given twice"));
    assert!(message("TREE y BY x /CHAID ALPHASPLIT=1.").contains("below 1"));
    assert!(message("TREE y BY x /FOO.").contains("unknown subcommand FOO of TREE"));
    assert_eq!(err("TREE y BY x [q].").expected, ["S", "O", "N", "C"]);
    assert_eq!(err("TREE y x.").expected, ["BY"]);
}

#[test]
fn star_join() {
    let field = |source: usize, name: &str, rename: Option<&str>| JoinField {
        source,
        name: name.into(),
        rename: rename.map(str::to_string),
    };
    let source =
        |file: Option<&str>, alias: &str, on: &[(&str, &str)], flag: Option<&str>| JoinSource {
            file: file.map(str::to_string),
            alias: alias.into(),
            on: on
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            flag: flag.map(str::to_string),
        };
    // every part: AS, PASSWORD, a key pair written the other way round, AND, IN, PASSPROTECT;
    // a field named twice for one source is taken once
    assert_eq!(
        one("STAR JOIN /SELECT t1.id, t1.K9.1 AS k91, t2.city, t1.id\n\
             /FROM 'fact.sav' PASSWORD='x' AS t1\n\
             /JOIN towns AS t2 ON t2.first = t1.first AND t1.last=t2.last IN=found\n\
             /JOIN * AS t3 ON t1.id = t3.id\n\
             /OUTFILE FILE='out.sav' /PASSPROTECT PASSWORD='secret' ENCRYPTEDPW=YES."),
        Command::StarJoin(StarJoin {
            select: Some(vec![
                field(0, "id", None),
                field(0, "K9.1", Some("k91")),
                field(1, "city", None),
            ]),
            sources: vec![
                source(Some("fact.sav"), "t1", &[], None),
                source(
                    Some("towns"),
                    "t2",
                    &[("first", "first"), ("last", "last")],
                    Some("found")
                ),
                source(None, "t3", &[("id", "id")], None),
            ],
            outfile: Some("out.sav".into()),
            encrypt: true,
        })
    );
    // SELECT *, the first slash left out, no password to protect with
    let Command::StarJoin(join) = one(
        "STAR JOIN SELECT * /FROM * AS a /JOIN 'DataSet4' AS b ON a.k = b.k \
         /OUTFILE FILE=* /PASSPROTECT PASSWORD=NONE.",
    ) else {
        panic!()
    };
    assert_eq!(join.select, None);
    assert_eq!(join.sources[1].file.as_deref(), Some("DataSet4"));
    assert!(join.outfile.is_none() && !join.encrypt);

    let message = |source: &str| err(source).message;
    assert!(
        err("STAR JOIN /SELECT a.x /FROM * AS a /OUTFILE FILE=*.")
            .expected
            .contains(&"/JOIN".to_string())
    );
    // keywords are written in full
    assert!(
        err("STAR JOIN /SEL a.x /FROM * AS a /JOIN b AS b ON a.k = b.k /OUTFILE FILE=*.")
            .expected
            .contains(&"/SELECT".to_string())
    );
    assert_eq!(
        message("STAR JOIN /SELECT c.x /FROM * AS a /JOIN b AS b ON a.k = b.k /OUTFILE FILE=*."),
        "c.x names no field of a source: write alias.name with an alias of FROM or JOIN (a, b)"
    );
    assert_eq!(
        message(
            "STAR JOIN /SELECT a.x /FROM * AS a /JOIN b AS b ON a.k = b.k \
             /JOIN c AS c ON b.k = c.k /OUTFILE FILE=*."
        ),
        "ON compares a field of FROM (a) with one of this JOIN (c): b.k = c.k"
    );
    assert_eq!(
        message("STAR JOIN /SELECT a.x /FROM * AS a /JOIN b AS A ON a.k = A.k /OUTFILE FILE=*."),
        "the alias A is given twice; every FROM and JOIN needs its own"
    );
    assert!(
        err("STAR JOIN /SELECT a.x /FROM * AS a /JOIN b AS b ON a.k = b.k /OUTFILE FILE=* /PASSPROTECT.")
            .expected
            .contains(&"PASSWORD=".to_string())
    );
}
