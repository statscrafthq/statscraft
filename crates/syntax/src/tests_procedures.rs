//! Parser tests of DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, ADD FILES and
//! MATCH FILES.

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
            .contains(&"/FILE='path' or /FILE=*".to_string())
    );
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
