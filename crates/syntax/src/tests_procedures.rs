//! Parser tests of DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, EXAMINE, ADD FILES,
//! MATCH FILES, STAR JOIN, SUMMARIZE and CODEBOOK.

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
            sort: None,
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
            sort: None,
        })
    );
    let Command::Descriptives(d) = one("DESCRIPTIVES x /STATISTICS=DEFAULT VARIANCE ALL.") else {
        panic!()
    };
    assert_eq!(d.statistics.len(), 10);

    let sorted = |text: &str| {
        let Command::Descriptives(d) = one(text) else {
            panic!()
        };
        d.sort.map(|s| (s.by, s.descending))
    };
    assert_eq!(sorted("DESCRIPTIVES x."), None);
    assert_eq!(
        sorted("DESCRIPTIVES x /SORT."),
        Some((Some(DescriptiveStatistic::Mean), false))
    );
    assert_eq!(
        sorted("DESCRIPTIVES x /SORT=MEAN (A)."),
        Some((Some(DescriptiveStatistic::Mean), false))
    );
    assert_eq!(
        sorted("DESCRIPTIVES x /SORT=RANGE (D)."),
        Some((Some(DescriptiveStatistic::Range), true))
    );
    assert_eq!(sorted("DESCRIPTIVES x /SORT=NAME (D)."), Some((None, true)));
    assert_eq!(
        sorted("DESCRIPTIVES x /SORT=(D)."),
        Some((Some(DescriptiveStatistic::Mean), true))
    );
    assert!(
        err("DESCRIPTIVES x /SORT=MEDIAN.")
            .expected
            .contains(&"NAME".to_string())
    );
    assert!(
        err("DESCRIPTIVES x /SORT=MEAN (X).")
            .expected
            .contains(&"D".to_string())
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
            measures: Vec::new(),
            cmh_odds: 1.0,
            include: false,
            descending: false,
            count: WeightCount::Asis,
            truncate: false,
            barchart: false,
            ranges: Vec::new(),
            report: false,
            method: None,
            write: None,
            layout_words: Vec::new(),
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
            measures: Vec::new(),
            cmh_odds: 1.0,
            include: true,
            descending: true,
            count: WeightCount::Asis,
            truncate: false,
            barchart: false,
            ranges: Vec::new(),
            report: false,
            method: None,
            write: None,
            layout_words: Vec::new(),
        })
    );
    // integer mode: a range after a list covers the whole list; fractions are cut off
    let Command::Crosstabs(c) =
        one("CROSSTABS VARIABLES=a b (1,3) c TO e (-2.5, 4.9) /TABLES=a BY c.")
    else {
        panic!()
    };
    assert_eq!(
        c.ranges,
        [
            (vec![var("a"), var("b")], 1.0, 3.0),
            (vec![VarItem::Range("c".into(), "e".into())], -2.0, 4.0)
        ]
    );
    let Command::Crosstabs(c) =
        one("CROSSTABS VARIABLES=a b (1,2) /TABLES=a BY b /MISSING=INCLUDE REPORT.")
    else {
        panic!()
    };
    assert!(c.report && !c.include);
    for (source, message) in [
        (
            "CROSSTABS /TABLES=a BY b /VARIABLES=a b (1,2).",
            "VARIABLES must come before TABLES",
        ),
        (
            "CROSSTABS VARIABLES=a (3,1) /TABLES=a BY a.",
            "the lowest value 3 is above the highest 1",
        ),
        ("CROSSTABS VARIABLES=a /TABLES=a BY a.", "'('"),
        ("CROSSTABS a BY b /MISSING=REPORT.", "only in integer mode"),
    ] {
        let error = err(source).to_string();
        assert!(error.contains(message), "{error}");
    }
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /CELLS=NONE.") else {
        panic!()
    };
    assert!(c.cells.is_empty());

    let Command::Crosstabs(c) =
        one("CROSSTABS a BY b /STATISTICS=CORR PHI GAMMA PHI CC LAMBDA BTAU CTAU.")
    else {
        panic!()
    };
    assert!(!c.chisq);
    use CrosstabsMeasure as M;
    assert_eq!(
        c.measures,
        [
            M::Phi,
            M::Cc,
            M::Lambda,
            M::Btau,
            M::Ctau,
            M::Gamma,
            M::Corr
        ]
    );
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /STATISTICS=RISK KAPPA.") else {
        panic!()
    };
    assert_eq!(c.measures, [M::Kappa, M::Risk]);
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /STATISTICS=MCNEMAR UC.") else {
        panic!()
    };
    assert_eq!(c.measures, [M::Uc, M::Mcnemar]);
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /STATISTICS=ETA D.") else {
        panic!()
    };
    assert_eq!(c.measures, [M::D, M::Eta]);
    let Command::Crosstabs(c) = one("CROSSTABS a BY b BY c /STATISTICS=CMH(2.5) CHISQ.") else {
        panic!()
    };
    assert_eq!(
        (c.measures.as_slice(), c.cmh_odds, c.chisq),
        ([M::Cmh].as_slice(), 2.5, true)
    );
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /STATISTICS=CMH.") else {
        panic!()
    };
    assert_eq!(c.cmh_odds, 1.0);
    assert!(
        err("CROSSTABS a BY b /STATISTICS=CMH(0).")
            .message
            .contains("above 0")
    );
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /STATISTICS=ALL.") else {
        panic!()
    };
    assert!(c.chisq);
    assert_eq!(c.measures.len(), 14);
    let e = err("CROSSTABS a BY b /STATISTICS=CHISQ SOMETHING.");
    assert_eq!(e.expected[0], "CHISQ");
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
    let write = |text: &str| {
        let Command::Crosstabs(c) = one(text) else {
            panic!()
        };
        c.write
    };
    assert_eq!(write("CROSSTABS a BY b."), None);
    assert_eq!(
        write("CROSSTABS a BY b /WRITE."),
        Some(CrosstabsWrite::Cells)
    );
    assert_eq!(
        write("CROSSTABS a BY b /WRITE=CELLS."),
        Some(CrosstabsWrite::Cells)
    );
    assert_eq!(write("CROSSTABS a BY b /WRITE=NONE."), None);
    assert_eq!(
        write("CROSSTABS VARIABLES=a b (1,3) /TABLES=a BY b /WRITE=ALL."),
        Some(CrosstabsWrite::All)
    );
    assert!(
        err("CROSSTABS a BY b /WRITE=ALL.")
            .message
            .contains("integer mode")
    );
    assert!(
        err("CROSSTABS a BY b /WRITE=ROWS.")
            .expected
            .contains(&"CELLS".to_string())
    );
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /BARCHART.") else {
        panic!()
    };
    assert!(c.barchart);
    assert_eq!(err("CROSSTABS a b.").expected, ["BY"]);
    assert_eq!(err("CROSSTABS /TABLES=a.").expected, ["BY"]);
    let Command::Crosstabs(c) = one("CROSSTABS a BY b /FORMAT=DVALUE NOBOX index.") else {
        panic!()
    };
    assert!(c.descending);
    assert_eq!(c.layout_words, ["NOBOX", "INDEX"]);
    assert!(
        err("CROSSTABS a BY b /FORMAT=LINE.")
            .expected
            .contains(&"AVALUE".to_string())
    );
    assert_eq!(
        one("PROCEDURE OUTPUT OUTFILE='cells.txt'."),
        Command::ProcedureOutput("cells.txt".into())
    );
    assert_eq!(
        one("PROCEDURE OUTPUT OUTFILE=cells."),
        Command::ProcedureOutput("cells".into())
    );
    assert!(
        err("PROCEDURE OUTPUT 'x'.")
            .expected
            .contains(&"OUTFILE='file'".to_string())
    );
}

#[test]
fn crosstabs_method() {
    let method = |source: &str| match one(source) {
        Command::Crosstabs(c) => c.method,
        _ => panic!(),
    };
    use CrosstabsMethod as M;
    assert_eq!(method("CROSSTABS a BY b."), None);
    assert_eq!(
        method("CROSSTABS a BY b /METHOD=EXACT."),
        Some(M::Exact { minutes: 5.0 })
    );
    assert_eq!(
        method("CROSSTABS a BY b /METH=EXA TIM(0)."),
        Some(M::Exact { minutes: 0.0 })
    );
    assert_eq!(
        method("CROSSTABS a BY b /METHOD=MC."),
        Some(M::MonteCarlo {
            level: 99.0,
            samples: 10_000
        })
    );
    assert_eq!(
        method("CROSSTABS a BY b /STATISTICS=CHISQ /METHOD=MC SAMPLES(500) CIN(95)."),
        Some(M::MonteCarlo {
            level: 95.0,
            samples: 500
        })
    );
    for (source, message) in [
        (
            "CROSSTABS a BY b /METHOD=MC TIMER(2).",
            "MC takes only CIN and SAMPLES",
        ),
        (
            "CROSSTABS a BY b /METHOD=EXACT CIN(95).",
            "EXACT takes only TIMER",
        ),
        ("CROSSTABS a BY b /METHOD=EXACT MC.", "exclude each other"),
        (
            "CROSSTABS a BY b /METHOD=MC CIN(100).",
            "CIN needs a level from 0.01 to 99.9",
        ),
        (
            "CROSSTABS a BY b /METHOD=MC SAMPLES(0).",
            "SAMPLES needs a whole number",
        ),
        (
            "CROSSTABS a BY b /METHOD=MC SAMPLES(2.5).",
            "SAMPLES needs a whole number",
        ),
        (
            "CROSSTABS a BY b /METHOD=EXACT TIMER(-1).",
            "TIMER needs a whole number",
        ),
        (
            "CROSSTABS a BY b /METHOD=MC CIN(95) CIN(90).",
            "CIN is given twice",
        ),
    ] {
        let error = err(source).message;
        assert!(error.contains(message), "{source}: {error}");
    }
    assert_eq!(
        err("CROSSTABS a BY b /METHOD=ASYMPTOTIC.").expected,
        ["MC", "EXACT"]
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
                    flag: None,
                },
                MergeInput {
                    file: Some("b.sav".into()),
                    table: false,
                    rename: vec![],
                    flag: None,
                },
            ],
            by: vec![var("id")],
            subsets: vec![
                Subset::Keep(vec![var("id"), var("c")]),
                Subset::Drop(vec![var("d")])
            ],
            first: None,
            last: None,
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
    // IN of MATCH FILES, FIRST and LAST need keys
    assert_eq!(
        err("MATCH FILES /FILE=* /FILE='b.sav' /IN=src.").message,
        "MATCH FILES /IN needs /BY key variables"
    );
    assert_eq!(
        err("ADD FILES /FILE=* /FILE='b.sav' /LAST=l.").message,
        "ADD FILES /FIRST or /LAST needs /BY key variables"
    );
    let Command::AddFiles(m) =
        one("ADD FILES /FILE=* /IN=a /FILE='b.sav' /IN b /BY id /LAST=l /FIR=f /MAP.")
    else {
        panic!()
    };
    assert_eq!(m.inputs[0].flag.as_deref(), Some("a"));
    assert_eq!(m.inputs[1].flag.as_deref(), Some("b"));
    assert_eq!(
        (m.first.as_deref(), m.last.as_deref()),
        (Some("f"), Some("l"))
    );
    assert_eq!(m.subsets, [Subset::Map]);
    let Command::MatchFiles(m) =
        one("MATCH FILES /TABLE='t.sav' /RENAME=(a b = c d) (e = f) /IN=t /FILE=* /BY k /FIRST=f.")
    else {
        panic!()
    };
    assert_eq!(m.inputs[0].rename.len(), 2);
    assert_eq!(m.inputs[0].flag.as_deref(), Some("t"));
    assert_eq!(m.first.as_deref(), Some("f"));
    // the order of the manual: FILE, TABLE, RENAME and IN first; BY once, IN, FIRST, LAST once
    assert!(
        err("ADD FILES /FILE=* /BY id /FILE='b.sav'.")
            .message
            .contains("come before BY")
    );
    assert!(
        err("MATCH FILES /FILE=* /FILE='b.sav' /KEEP=id /RENAME=(a=b).")
            .message
            .contains("come before BY")
    );
    assert!(
        err("MATCH FILES /FILE=* /FILE='b.sav' /BY id /BY k.")
            .message
            .contains("takes one /BY")
    );
    assert!(
        err("ADD FILES /FILE=* /IN=a /IN=b /FILE='b.sav'.")
            .message
            .contains("one /IN per /FILE")
    );
    assert!(
        err("ADD FILES /FILE=* /FILE='b.sav' /BY id /FIRST=a /FIRST=b.")
            .message
            .contains("takes one /FIRST")
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
    let polynomial = |text: &str| {
        let Command::Oneway(o) = one(text) else {
            panic!()
        };
        o.polynomial
    };
    let effects = |text: &str| {
        let Command::Oneway(o) = one(text) else {
            panic!()
        };
        (o.effects, o.descriptives)
    };
    assert_eq!(effects("ONEWAY a BY g."), (false, false));
    assert_eq!(effects("ONEWAY a BY g /STATISTICS=EFFECTS."), (true, false));
    assert_eq!(effects("ONEWAY a BY g /STAT=DESC EFF."), (true, true));
    assert_eq!(effects("ONEWAY a BY g /STATISTICS=ALL."), (true, true));
    let post = |text: &str| {
        let Command::Oneway(o) = one(text) else {
            panic!()
        };
        (o.posthoc, o.alpha)
    };
    assert_eq!(
        post("ONEWAY a BY g /RANGES=TUKEYB /RANGES=MODLSD(0.01) /RANGES=SNK."),
        (
            vec![Posthoc::TukeyB, Posthoc::Bonferroni, Posthoc::Snk],
            0.01
        )
    );
    // RANGES adds to POSTHOC, a later POSTHOC replaces only the earlier POSTHOC, tests count once
    assert_eq!(
        post(
            "ONEWAY a BY g /POSTHOC=LSD /RANGES=SCHEFFE /POSTHOC=TUKEY ALPHA(0.1) /RANGES=LSDMOD /RANGES=tukey"
        ),
        (
            vec![Posthoc::Tukey, Posthoc::Scheffe, Posthoc::Bonferroni],
            0.1
        )
    );
    assert_eq!(post("ONEWAY a BY g /RANGES=SCH.").0, vec![Posthoc::Scheffe]);
    assert!(
        err("ONEWAY a BY g /RANGES=TUK.")
            .expected
            .contains(&"TUKEYB".to_string())
    );
    assert!(
        err("ONEWAY a BY g /RANGES=GH.")
            .expected
            .contains(&"SNK".to_string())
    );
    assert!(
        err("ONEWAY a BY g /RANGES=LSD(2).")
            .message
            .contains("between 0 and 1")
    );
    let eleven = "/RANGES=LSD".repeat(11);
    assert!(
        err(&format!("ONEWAY a BY g {eleven}."))
            .message
            .contains("at most 10 RANGES")
    );
    assert_eq!(polynomial("ONEWAY a BY g."), None);
    assert_eq!(polynomial("ONEWAY a BY g /POLYNOMIAL=2."), Some(2));
    // only the last POLYNOMIAL counts
    assert_eq!(
        polynomial("ONEWAY a BY g /POLYNOMIAL=2 /POLYNOMIAL 5."),
        Some(5)
    );
    for bad in ["0", "6", "1.5", "-1"] {
        assert!(
            err(&format!("ONEWAY a BY g /POLYNOMIAL={bad}."))
                .message
                .contains("POLYNOMIAL needs a whole number from 1 to 5")
        );
    }
    assert!(
        err("ONEWAY a BY g /STATISTICS=EFFECT2.")
            .expected
            .contains(&"EFFECTS".to_string())
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
    let Command::Correlations(c) = one("CORRELATIONS a b /MATRIX=OUT('m.sav').") else {
        panic!()
    };
    assert_eq!(c.matrix.output, Some(Some("m.sav".to_string())));
    let Command::NonparCorr(c) = one("NONPAR CORR a b /MATRIX OUT(*).") else {
        panic!()
    };
    assert_eq!(c.matrix.output, Some(None));
    let Command::Correlations(c) = one("CORRELATIONS a b /CI METHOD=FISHER BIAS=TRUE CILEVEL=90.")
    else {
        panic!()
    };
    assert_eq!(
        c.ci,
        Some(CorrelationsCi {
            bias: true,
            level: 90.0
        })
    );
    let Command::NonparCorr(c) = one("NONPAR CORR a b /CI METHOD(CC) CILEVEL(99).") else {
        panic!()
    };
    assert_eq!(
        c.ci,
        Some(NonparCi {
            method: SpearmanSe::Cc,
            level: 99.0
        })
    );
    let Command::NonparCorr(c) = one("NONPAR CORR a b /CI.") else {
        panic!()
    };
    assert_eq!(c.ci.map(|c| c.method), Some(SpearmanSe::Fhp));
    assert!(
        err("CORRELATIONS a b /CI CILEVEL=100.")
            .message
            .contains("0 and 100")
    );
    assert!(
        err("NONPAR CORR a b /CI METHOD(XX).")
            .expected
            .iter()
            .any(|e| e == "FHP")
    );
    // the manual's MATRIX of both commands only writes
    assert!(
        err("CORRELATIONS a b /MATRIX=IN(*).")
            .message
            .contains("OUT")
    );
    assert!(
        err("NONPAR CORR a b /MATRIX=IN(*).")
            .message
            .contains("OUT")
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
    let Command::NonparCorr(c) = one("NONPAR CORR a b /CI METHOD(FHP).") else {
        panic!()
    };
    assert!(c.ci.is_some());
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
    assert_eq!(plots("EXAMINE a.").0, ["BOXPLOT", "STEMLEAF"]);
    assert!(plots("EXAMINE a /PLOT=NPPLOT.").0.is_empty());
    let spread = |source: &str| match one(source) {
        Command::Examine(e) => (e.spread_level, e.compare_variables),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        spread("EXAMINE a BY f /PLOT=SPREADLEVEL(0)."),
        (Some(Some(0.0)), false)
    );
    assert_eq!(
        spread("EXAMINE a BY f /PLOT=SPREADLEVEL /COMPARE VARIABLES."),
        (Some(None), true)
    );
    assert_eq!(
        spread("EXAMINE a BY f /PLOT=SPREADLEVEL(.5) NONE."),
        (None, false)
    );
    assert_eq!(err("EXAMINE a /PLOT=SPREADLEVEL(x).").expected, ["a power"]);
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
    let kinds: Vec<FrequencyChartKind> = f.charts.iter().map(|c| c.kind).collect();
    // a repeated subcommand replaces the earlier one
    assert_eq!(kinds, [FrequencyChartKind::Pie, FrequencyChartKind::Bar]);
    let pie = &f.charts[0];
    assert_eq!((pie.minimum, pie.maximum), (Some(1.0), Some(10.0)));
    assert!(pie.percent && !pie.missing);
    assert_eq!(
        (f.charts[1].percent, f.charts[1].scale),
        (false, Some(50.0))
    );
    let f = freq("FREQUENCIES a /HIST=NORMAL FREQ(20) MINIMUM(0).");
    let histogram = &f.charts[0];
    assert_eq!(histogram.kind, FrequencyChartKind::Histogram);
    assert!(histogram.normal);
    assert_eq!(
        (histogram.scale, histogram.minimum),
        (Some(20.0), Some(0.0))
    );
    // of FREQ and PERCENT, MISSING and NOMISSING, NORMAL and NONORMAL the first counts
    let f = freq("FREQUENCIES a /PIE=FREQ PERCENT NOMISSING MISSING /BAR=PERCENT(40) FREQ.");
    assert!(!f.charts[0].percent && !f.charts[0].missing);
    assert_eq!((f.charts[1].percent, f.charts[1].scale), (true, Some(40.0)));
    assert!(!freq("FREQUENCIES a /HIST=NONORMAL NORMAL.").charts[0].normal);
}

#[test]
fn frequencies_errors() {
    let message = |source: &str| err(source).message;
    assert!(message("FREQUENCIES a /BARCHART /HISTOGRAM.").contains("cannot be used together"));
    assert!(message("FREQUENCIES a /PERCENTILES=101.").contains("between 0 and 100"));
    assert!(message("FREQUENCIES a /NTILES=0.").contains("from 1 to 1000"));
    // n - 1 cut points at once: 16 GB for this one
    assert!(message("FREQUENCIES a /NTILES=2147483648.").contains("from 1 to 1000"));
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
          /PLOT GAIN INDEX ROI INCREMENT=5 /PRIORS EQUAL.",
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
    assert!(t.plot_gain && t.plot_index && !t.plot_response && t.plot_increment == 5);
    // PLOT ROI is kept (it needs PROFITS, which the engine checks); ASSIGNMENT and PRIORS are
    // accepted and said to be ignored (MAXSURROGATES=AUTO is the default and says nothing)
    assert!(t.plot_roi);
    assert_eq!(t.ignored.len(), 2);
}

#[test]
fn tree_crt_settings() {
    let t = tree_of(
        "TREE y BY x /CRT IMPURITY=TWOING MINIMPROVEMENT=.01 /PRIORS CUSTOM=1 [3] 'b' [1] ADJUST=YES \
         /METHOD TYPE=CRT MAXSURROGATES=2 /PRINT IMPORTANCE SURROGATES /PLOT IMPORTANCE.",
    );
    assert!(t.crt && t.print_importance && t.print_surrogates && t.plot_importance);
    assert_eq!(t.impurity, TreeImpurity::Twoing);
    assert_eq!((t.min_improvement, t.max_surrogates), (Some(0.01), Some(2)));
    assert_eq!(
        t.priors,
        TreePriors::Custom(vec![
            (Value::Number(1.0), 3.0),
            (Value::String("b".into()), 1.0)
        ])
    );
    // CRT uses what it was given, and says nothing about it
    assert!(t.ignored.is_empty(), "{:?}", t.ignored);
    // another method ignores the same settings, with a warning for each
    let t = tree_of("TREE y BY x /CRT IMPURITY=GINI /PRIORS EQUAL /GROWTHLIMIT MINIMPROVEMENT=.1.");
    assert!(!t.crt);
    assert_eq!(t.ignored.len(), 2, "{:?}", t.ignored);
    // MINIMPROVEMENT in GROWTHLIMIT counts for CRT as well
    let t = tree_of("TREE y BY x /METHOD TYPE=CRT /GROWTHLIMIT MINIMPROVEMENT=.1.");
    assert_eq!(t.min_improvement, Some(0.1));
    assert_eq!(t.impurity, TreeImpurity::Gini);
    assert_eq!(t.priors, TreePriors::FromData);
    let message = err("TREE y BY x /METHOD TYPE=CRT /CRT MINIMPROVEMENT=0.").message;
    assert_eq!(message, "MINIMPROVEMENT must be above 0");
    let message = err("TREE y BY x /METHOD TYPE=CRT /PRIORS CUSTOM=1 [-1].").message;
    assert_eq!(message, "a prior probability cannot be negative");
}

#[test]
fn tree_quest_settings() {
    let t = tree_of(
        "TREE y BY x /METHOD TYPE=QUEST MAXSURROGATES=1 PRUNE=SE(1) /QUEST ALPHASPLIT=.01.",
    );
    assert!(t.quest && !t.crt && t.binary());
    assert_eq!(
        (t.quest_alpha, t.max_surrogates, t.prune),
        (0.01, Some(1), Some(1.0))
    );
    assert!(t.ignored.is_empty(), "{:?}", t.ignored);
    // QUEST ignores what belongs to the other methods, with a warning for each
    let t = tree_of(
        "TREE y BY x /METHOD TYPE=QUEST /CRT IMPURITY=TWOING /CHAID ALPHASPLIT=.1 \
         /PRINT IMPORTANCE /GROWTHLIMIT MINIMPROVEMENT=.1 /SCORES EQUALINCREMENTS.",
    );
    assert_eq!(t.ignored.len(), 4, "{:?}", t.ignored);
    assert_eq!(t.quest_alpha, 0.05);
    let t = tree_of("TREE y BY x /METHOD TYPE=QUEST /GROWTHLIMIT MINIMPROVEMENT=.1.");
    assert_eq!(t.ignored, ["MINIMPROVEMENT applies to CRT only; ignored"]);
    // the other methods ignore /QUEST
    let t = tree_of("TREE y BY x /METHOD TYPE=CRT /QUEST ALPHASPLIT=.2.");
    assert_eq!(t.ignored, ["QUEST applies to QUEST trees only; ignored"]);
    // the last METHOD TYPE holds
    let t = tree_of("TREE y BY x /METHOD TYPE=QUEST TYPE=CHAID.");
    assert!(!t.quest && !t.crt);
}

#[test]
fn tree_validation_settings() {
    let t = tree_of("TREE y BY x /VALIDATION TYPE=SPLITSAMPLE(30) OUTPUT=TESTSAMPLE.");
    assert_eq!(t.validation, TreeValidation::SplitPercent(30.0));
    assert!(t.test_sample_only);
    let t = tree_of("TREE y BY x /VALIDATION TYPE=SPLITSAMPLE.");
    assert_eq!(t.validation, TreeValidation::SplitPercent(50.0));
    assert!(!t.test_sample_only);
    let t = tree_of("TREE y BY x /VALIDATION TYPE=SPLITSAMPLE(part) OUTPUT=BOTHSAMPLES.");
    assert_eq!(t.validation, TreeValidation::SplitVariable("part".into()));
    let t = tree_of("TREE y BY x /VALIDATION TYPE=NONE.");
    assert_eq!(t.validation, TreeValidation::None);
    let t = tree_of("TREE y BY x /VALIDATION TYPE=CROSSVALIDATION(5).");
    assert_eq!(t.validation, TreeValidation::Cross(5));
    let t = tree_of("TREE y BY x /VALIDATION TYPE=CROSSVALIDATION.");
    assert_eq!(t.validation, TreeValidation::Cross(10));
    // FORCE and cross-validation do not go together: the validation is dropped with a warning
    let t = tree_of("TREE y BY x FORCE=x /VALIDATION TYPE=CROSSVALIDATION(5).");
    assert_eq!(t.validation, TreeValidation::None);
    assert_eq!(t.ignored.len(), 1, "{:?}", t.ignored);
}

#[test]
fn tree_costs() {
    let t = tree_of("TREE y BY x /COSTS CUSTOM=3 2 [2] 3 1 [8] 3 2 [4] 'a' 1 [0].");
    let costs = t.costs.expect("custom costs");
    // the last of a pair holds
    assert_eq!(costs.len(), 3);
    assert_eq!(costs[0], (Value::Number(3.0), Value::Number(1.0), 8.0));
    assert_eq!(costs[1], (Value::Number(3.0), Value::Number(2.0), 4.0));
    assert!(tree_of("TREE y BY x /COSTS EQUAL.").costs.is_none());
    assert!(tree_of("TREE y BY x /PRIORS ADJUST=YES.").adjust_priors);
}

#[test]
fn tree_gain_types() {
    let t = tree_of("TREE y BY x.");
    assert_eq!(
        (t.gain_node, t.gain_ptile, t.gain_increment),
        (true, false, 10)
    );
    let t = tree_of("TREE y BY x /GAIN TYPE=[PTILE] INCREMENT=5.");
    assert_eq!(
        (t.gain_node, t.gain_ptile, t.gain_increment),
        (false, true, 5)
    );
    let t = tree_of("TREE y BY x /GAIN TYPE=[NODE, PTILE].");
    assert_eq!((t.gain_node, t.gain_ptile), (true, true));
    // another increment keeps 10, with a warning
    let t = tree_of("TREE y BY x /GAIN INCREMENT=15.");
    assert_eq!(t.gain_increment, 10);
    assert_eq!(t.ignored.len(), 1, "{:?}", t.ignored);
}

#[test]
fn tree_rules() {
    let t = tree_of("TREE y BY x /RULES.");
    let rules = t.rules.expect("rules");
    assert_eq!(rules.nodes, TreeRuleNodes::Terminal);
    assert_eq!(rules.syntax, TreeRuleSyntax::Internal);
    assert!(!rules.selection && rules.surrogates && rules.labels && rules.outfile.is_none());
    let t = tree_of(
        "TREE y BY x /RULES NODES=ALL SYNTAX=SQL TYPE=SELECTION SURROGATES=EXCLUDE LABELS=NO OUTFILE='r.sql'.",
    );
    let rules = t.rules.expect("rules");
    assert_eq!(rules.nodes, TreeRuleNodes::All);
    assert_eq!(rules.syntax, TreeRuleSyntax::Sql);
    assert!(rules.selection && !rules.surrogates && !rules.labels);
    assert_eq!(rules.outfile.as_deref(), Some("r.sql"));
    let nodes = |source: &str| tree_of(source).rules.expect("rules").nodes;
    assert_eq!(
        nodes("TREE y BY x /RULES NODES=TOPN(3)."),
        TreeRuleNodes::TopN(3)
    );
    assert_eq!(
        nodes("TREE y BY x /RULES NODES=TOPPCT(25)."),
        TreeRuleNodes::TopPercent(25.0)
    );
    assert_eq!(
        nodes("TREE y BY x /RULES NODES=MININDEX(120)."),
        TreeRuleNodes::MinIndex(120.0)
    );
    assert_eq!(nodes("TREE y BY x /RULES NODES=NONE."), TreeRuleNodes::None);
}

#[test]
fn tree_profits() {
    let t = tree_of("TREE y BY x /PROFITS CUSTOM=1 [10 2] 2 [4 3] 1 [8 1] /PLOT PROFIT ROI.");
    assert_eq!(
        t.profits,
        Some(vec![
            (Value::Number(2.0), 4.0, 3.0),
            (Value::Number(1.0), 8.0, 1.0)
        ])
    );
    assert!(t.plot_profit && t.plot_roi);
    assert!(tree_of("TREE y BY x.").profits.is_none());
}

#[test]
fn tree_errors() {
    let message = |source: &str| err(source).message;
    assert_eq!(
        message("TREE y BY x /METHOD TYPE=QUEST /QUEST ALPHASPLIT=1."),
        "ALPHASPLIT must be above 0 and below 1"
    );
    assert_eq!(
        message("TREE y BY x /QUEST DEPTH=1."),
        "unexpected word DEPTH"
    );
    assert_eq!(
        message("TREE y BY x /VALIDATION TYPE=CROSSVALIDATION(1)."),
        "CROSSVALIDATION needs a whole number of folds from 2 to 25"
    );
    assert_eq!(
        message("TREE y BY x /VALIDATION TYPE=SPLITSAMPLE(100)."),
        "SPLITSAMPLE needs a percent above 0 and below 100, or a variable"
    );
    assert_eq!(
        message("TREE y BY x /CHAID SPLITMERGED=YES."),
        "CHAID SPLITMERGED=YES is not supported yet"
    );
    assert!(message("TREE y BY x /RULES NODES=TOPPCT(100).").contains("below 100"));
    assert_eq!(
        message("TREE y BY x /COSTS CUSTOM=1 2 [-3]."),
        "a misclassification cost cannot be negative"
    );
    assert_eq!(
        message("TREE y BY x /PROFITS CUSTOM=1 [10]."),
        "unexpected ']'"
    );
    assert_eq!(
        message("TREE y BY x /PROFITS 1 [1 2]."),
        "unexpected number 1"
    );
    assert_eq!(
        message("TREE y BY x /COSTS CUSTOM=1."),
        "unexpected end of command"
    );
    assert_eq!(
        message("TREE y BY x /GAIN TYPE=[DECILE]."),
        "unexpected word DECILE"
    );
    assert_eq!(
        message("TREE y BY x /GAIN TYPE=[]."),
        "unexpected end of command"
    );
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

#[test]
fn mult_response() {
    let Command::MultResponse(m) = one(
        "MULT RESP GROUPS=mags 'Magazines' (time TO stone (2)) probs (p1 TO p3 (1,9))\n\
         /VAR=sex (1,2) educ (1 3)\n\
         /FREQ=mags TO probs sex\n\
         /TAB=mags BY educ BY sex /educ BY sex (PAIRED)\n\
         /MIS=MRGROUP INCLUDE /FORM=NOLAB CONDENSE ONEPAGE /BASE=RESPONSES /CELLS.",
    ) else {
        panic!("not MULT RESPONSE");
    };
    let name = |n: &str| VarItem::Name(n.into());
    assert_eq!(m.groups.len(), 2);
    assert_eq!(m.groups[0].label.as_deref(), Some("Magazines"));
    assert_eq!((m.groups[0].value, m.groups[0].max), (2.0, None));
    assert_eq!((m.groups[1].value, m.groups[1].max), (1.0, Some(9.0)));
    assert_eq!(m.variables[1], (vec![name("educ")], 1.0, 3.0));
    assert_eq!(
        m.frequencies,
        [VarItem::Range("mags".into(), "probs".into()), name("sex")]
    );
    assert_eq!(m.tables.len(), 2);
    assert_eq!(m.tables[0].dimensions.len(), 3);
    assert!(!m.tables[0].paired && m.tables[1].paired);
    assert!(m.mrgroup && m.include && !m.mdgroup);
    assert!(m.no_labels && m.condense);
    assert_eq!(m.layout, ["ONEPAGE"]);
    assert_eq!(m.base_responses, Some(true));
    assert_eq!(
        m.cells,
        [
            CrosstabsCell::Count,
            CrosstabsCell::Row,
            CrosstabsCell::Column,
            CrosstabsCell::Total
        ]
    );
}

#[test]
fn summarize() {
    use MeansCell as M;
    let Command::Summarize(s) = one(
        "SUMM TAB=spend fee BY branch BY tier /TIT 'Fees' 'by branch' /FOOT='Line one\\nline two'\n\
         /CEL=COUNT MAX /MIS=DEPENDENT TABLE /FORM=LIST LIMIT=25 NOCASENUM NOTOTAL MIS='-' /STAT.",
    ) else {
        panic!()
    };
    assert_eq!(
        s.tables,
        [vec![
            vec![var("spend"), var("fee")],
            vec![var("branch")],
            vec![var("tier")]
        ]]
    );
    assert_eq!(s.title.as_deref(), Some("Feesby branch"));
    assert_eq!(s.footnote.as_deref(), Some("Line one\nline two"));
    assert_eq!(s.cells, [M::Count, M::Max]);
    assert_eq!(s.missing, SummarizeMissing::Dependent);
    assert!(s.tablewise && s.anova && !s.linearity);
    assert_eq!(
        s.list,
        Some(SummarizeList {
            valid_only: false,
            limit: Some(25)
        })
    );
    assert!(!s.case_numbers && !s.total_label);
    assert_eq!(s.missing_text.as_deref(), Some("-"));
    let Command::Summarize(s) = one("SUMMARIZE a /b BY c /CELLS=NONE /CELLS /STATISTICS=ALL.")
    else {
        panic!()
    };
    assert_eq!(s.tables.len(), 2);
    assert_eq!(s.cells, [M::Mean, M::Count, M::StdDev]);
    assert_eq!(s.repeated, ["CELLS"]);
    assert!(s.linearity);
    let Command::Summarize(s) = one("SUMMARIZE a /CELLS=NONE /FORMAT=VALIDLIST.") else {
        panic!()
    };
    assert!(s.cells.is_empty() && s.list.is_some_and(|l| l.valid_only));
    assert!(
        err("SUMMARIZE a /b /FORMAT=LIST.")
            .message
            .contains("one table list")
    );
    assert!(
        err("SUMMARIZE a /FORMAT=LIST LIMIT=0.")
            .message
            .contains("LIMIT")
    );
    let eleven = format!("SUMMARIZE a{}.", " BY b".repeat(11));
    assert!(err(&eleven).message.contains("10 BY"));
    one(&format!("SUMMARIZE a{}.", " BY b".repeat(10)));
}

#[test]
fn codebook() {
    let Command::Codebook(c) = one("CODEBOOK.") else {
        panic!()
    };
    assert!(c.items.is_empty() && c.fileinfo.is_empty() && c.order.is_none());
    assert_eq!(c.varinfo.len(), 9);
    assert_eq!(c.statistics.len(), 5);
    assert_eq!(c.maxcats, 200);
    let Command::Codebook(c) = one(
        "CODE a [N] b TO d [S] $set $ALL ALL /VARI=LABEL MEASURE RESERVED\n\
         /FILE=NAME CASECOUNT /STAT=COUNT MEAN /OPT MAXCATS=12 VARORDER=ATTRIBUTE(unit) SORT=DESC.",
    ) else {
        panic!()
    };
    assert_eq!(
        c.items,
        [
            CodebookItem {
                item: var("a"),
                level: Some(Level::Nominal)
            },
            CodebookItem {
                item: VarItem::Range("b".into(), "d".into()),
                level: Some(Level::Scale)
            },
            CodebookItem {
                item: var("$set"),
                level: None
            },
            CodebookItem {
                item: var("$ALL"),
                level: None
            },
            CodebookItem {
                item: VarItem::All,
                level: None
            },
        ]
    );
    assert_eq!(
        c.varinfo,
        [
            CodebookInfo::Label,
            CodebookInfo::Measure,
            CodebookInfo::ReservedAttributes
        ]
    );
    assert_eq!(c.fileinfo, [CodebookFile::Name, CodebookFile::CaseCount]);
    assert_eq!(
        c.statistics,
        [CodebookStatistic::Count, CodebookStatistic::Mean]
    );
    assert_eq!(c.maxcats, 12);
    assert_eq!(c.order, Some(CodebookOrder::Attribute("unit".into())));
    assert!(c.descending);
    let Command::Codebook(c) = one("CODEBOOK x /STATISTICS=NONE.") else {
        panic!()
    };
    assert!(c.statistics.is_empty());
    assert!(
        err("CODEBOOK /STATISTICS=NONE MEAN.")
            .message
            .contains("alone")
    );
    assert!(
        err("CODEBOOK /OPTIONS SORT=UP /OPTIONS MAXCATS=2.")
            .expected
            .contains(&"DESCENDING".to_string())
    );
    assert!(
        err("CODEBOOK /VARINFO=LABEL /VARINFO=TYPE.")
            .message
            .contains("once")
    );
    assert!(
        err("CODEBOOK ALL [N].")
            .message
            .contains("no measurement level")
    );
    assert!(err("CODEBOOK $a TO $b.").message.contains("TO"));
    assert!(err("CODEBOOK x [Q].").expected.contains(&"N".to_string()));
}

#[test]
fn oneway_matrix_in_out_none_and_never_both() {
    let one = |source: &str| match parse(source).0.remove(0).command {
        Command::Oneway(o) => o,
        other => panic!("{other:?}"),
    };
    assert_eq!(one("ONEWAY y BY g /MATRIX=IN(*).").matrix.input, Some(None));
    assert_eq!(
        one("ONEWAY y BY g /MATRIX=OUT('m.sav').").matrix.output,
        Some(Some("m.sav".to_string()))
    );
    assert_eq!(
        one("ONEWAY y BY g /MATRIX=NONE.").matrix,
        MatrixIo::default()
    );
    let (_, errors) = parse("ONEWAY y BY g /MATRIX=IN(*) OUT(*).");
    assert!(errors[0].message.contains("not both"), "{errors:?}");
}
