//! Parser tests of CTABLES.

use super::*;

fn one(source: &str) -> Ctables {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Ctables(c) => c,
        other => panic!("not CTABLES: {other:?}"),
    }
}

fn err(source: &str) -> SyntaxError {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0)
}

fn var(name: &str) -> AxisExpr {
    AxisExpr::Variable(AxisVariable {
        name: name.into(),
        scale: None,
        summaries: None,
    })
}

#[test]
fn nesting_binds_tighter_than_stacking() {
    let c = one("CTABLES /TABLE a + b > c BY (d + e) > f.");
    let t = &c.tables[0];
    assert_eq!(
        t.rows,
        AxisExpr::Stack(
            Box::new(var("a")),
            Box::new(AxisExpr::Nest(Box::new(var("b")), Box::new(var("c"))))
        )
    );
    assert_eq!(
        t.columns,
        Some(AxisExpr::Nest(
            Box::new(AxisExpr::Stack(Box::new(var("d")), Box::new(var("e")))),
            Box::new(var("f"))
        ))
    );
    assert!(t.summary_labels && !t.summaries_in_rows);
}

#[test]
fn summaries_with_labels_formats_and_levels() {
    let c = one(
        "CTABLES /TABLE q1 [C] [COUNT 'N' F8.0, COLPCT.VALIDN PCT8.1, UMEAN, PTILE 25 'Q1'] BY age [S].",
    );
    let AxisExpr::Variable(v) = &c.tables[0].rows else {
        panic!()
    };
    assert_eq!(v.scale, Some(false));
    let s = v.summaries.as_ref().unwrap();
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].label.as_deref(), Some("N"));
    assert_eq!(
        s[0].format,
        Some(Format {
            kind: FormatType::F,
            width: 8,
            decimals: 0
        })
    );
    assert_eq!(
        s[1].function.statistic,
        Statistic::Percent(PercentBase::Column, PercentOf::Validn)
    );
    assert_eq!(s[1].function.name(), "COLPCT.VALIDN");
    assert!(s[2].function.unweighted);
    assert_eq!(s[2].function.name(), "UMEAN");
    assert_eq!(
        (s[3].percentile, s[3].label.as_deref()),
        (Some(25.0), Some("Q1"))
    );
    let Some(AxisExpr::Variable(age)) = &c.tables[0].columns else {
        panic!()
    };
    assert_eq!(age.scale, Some(true));
}

#[test]
fn per_table_and_global_subcommands() {
    let c = one(
        "CTABLES /FORMAT EMPTY=BLANK MISSING='-' /VLABELS VARIABLES=a b DISPLAY=NAME\n\
         /TABLE a BY b /SLABELS POSITION=ROW VISIBLE=NO\n\
         /CATEGORIES VARIABLES=a ORDER=D KEY=LABEL MISSING=INCLUDE TOTAL=YES LABEL='All' POSITION=BEFORE EMPTY=EXCLUDE\n\
         /CATEGORIES VARIABLES=b [3, 1 THRU 2, 'x'] TOTAL=YES\n\
         /TITLES TITLE='T' CAPTION='C' CORNER='K'\n\
         /TABLE b.",
    );
    assert_eq!(c.empty, EmptyCells::Blank);
    assert_eq!(c.missing.as_deref(), Some("-"));
    assert_eq!(c.vlabels[0].1, VlabelsDisplay::Name);
    assert_eq!(c.tables.len(), 2);
    let t = &c.tables[0];
    assert!(t.summaries_in_rows && !t.summary_labels);
    let a = &t.categories[0];
    assert!(a.descending && a.by_label && a.include_missing && a.total && a.total_before);
    assert!(a.exclude_empty);
    assert_eq!(a.total_label.as_deref(), Some("All"));
    assert_eq!(
        t.categories[1].values,
        Some(vec![
            CategoryItem::Value(Value::Number(3.0)),
            CategoryItem::Range(Bound::Number(1.0), Bound::Number(2.0)),
            CategoryItem::Value(Value::String("x".into())),
        ])
    );
    assert_eq!(
        (
            t.title.as_deref(),
            t.caption.as_deref(),
            t.corner.as_deref()
        ),
        (Some("T"), Some("C"), Some("K"))
    );
    assert!(c.tables[1].categories.is_empty());
}

#[test]
fn features_of_the_next_stage_are_named() {
    for (source, feature) in [
        ("CTABLES /TABLE a BY b BY c.", "layer axis"),
        (
            "CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES.",
            "MERGE=YES",
        ),
        ("CTABLES /TABLE a /CRITERIA CILEVEL=90.", "CRITERIA"),
        (
            "CTABLES /PCOMPUTE &x = EXPR([1] + [2]) /TABLE a.",
            "PCOMPUTE",
        ),
        (
            "CTABLES /HIDESMALLCOUNTS COUNT=5 /TABLE a.",
            "HIDESMALLCOUNTS",
        ),
        ("CTABLES /TABLE a [ECOUNT].", "ECOUNT"),
        (
            "CTABLES /TABLE a /CATEGORIES VARIABLES=a [1, SUBTOTAL='s'].",
            "SUBTOTAL",
        ),
        (
            "CTABLES /TABLE a /CATEGORIES VARIABLES=a [1, &x].",
            "computed category",
        ),
        ("CTABLES /TABLE a /CLABELS ROWLABELS=OPPOSITE.", "CLABELS"),
    ] {
        let e = err(source);
        assert!(
            e.message.contains(feature) && e.message.contains("not supported yet"),
            "{source}: {}",
            e.message
        );
    }
}

#[test]
fn significance_tests_with_their_defaults() {
    let c = one("CTABLES /TABLE a BY b /SIGTEST TYPE=CHISQUARE /COMPARETEST TYPE=PROP.");
    let t = &c.tables[0];
    assert_eq!(
        t.sigtest,
        Some(Sigtest {
            alpha: 0.05,
            include_mrsets: true,
            subtotals: false
        })
    );
    assert_eq!(
        t.comparetests,
        [Comparetest {
            kind: CompareKind::Proportions,
            alpha: vec![0.05],
            adjust: Adjust::Bonferroni,
            include_mrsets: true,
            pool_all: true,
            subtotals: false,
            show_sig: false,
        }]
    );
}

#[test]
fn significance_tests_with_every_keyword() {
    let c = one("CTABLES /TABLE a BY b\n\
         /SIGTEST TYPE=CHISQUARE ALPHA=0.01 INCLUDEMRSETS=NO CATEGORIES=SUBTOTALS\n\
         /COMPARETEST TYPE=MEAN ALPHA=0.05, 0.01 ADJUST=BH INCLUDEMRSETS=NO MEANSVARIANCE=TESTEDCATS\n\
         CATEGORIES=SUBTOTALS MERGE=YES STYLE=SIMPLE SHOWSIG=YES ORIGIN=COLUMN\n\
         /COMPARETEST TYPE=PROP ADJUST=NONE /COMPARETEST TYPE=PROP ALPHA=0.1.");
    let t = &c.tables[0];
    let sigtest = t.sigtest.as_ref().unwrap();
    assert!(sigtest.alpha == 0.01 && !sigtest.include_mrsets && sigtest.subtotals);
    // the second PROP test replaces the first; SHOWSIG=YES keeps the separate table
    assert_eq!(t.comparetests.len(), 2);
    let mean = &t.comparetests[0];
    assert_eq!(mean.kind, CompareKind::Means);
    assert_eq!(mean.alpha, [0.01, 0.05]);
    assert_eq!(mean.adjust, Adjust::BenjaminiHochberg);
    assert!(!mean.include_mrsets && !mean.pool_all && mean.subtotals && mean.show_sig);
    let prop = &t.comparetests[1];
    assert_eq!(
        (prop.alpha.as_slice(), prop.adjust),
        (&[0.1][..], Adjust::Bonferroni)
    );
    // keywords may be shortened to three letters
    let c = one("CTABLES /TABLE a BY b /COMPARETEST TYP=MEAN ADJ=NONE MEA=TESTEDCATS.");
    assert_eq!(c.tables[0].comparetests[0].adjust, Adjust::None);
}

#[test]
fn significance_test_mistakes_are_named() {
    let e = err("CTABLES /TABLE a BY b /SIGTEST ALPHA=0.01.");
    assert!(e.message.contains("needs TYPE=CHISQUARE"), "{e}");
    let e = err("CTABLES /TABLE a BY b /COMPARETEST ALPHA=0.01.");
    assert!(e.message.contains("TYPE=PROP or TYPE=MEAN"), "{e}");
    let e = err("CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP ALPHA=1.5.");
    assert!(e.message.contains("more than 0 and less than 1"), "{e}");
    let e = err("CTABLES /TABLE a BY b /SIGTEST TYPE=PROP.");
    assert_eq!(e.expected, ["CHISQUARE"]);
    let e = err("CTABLES /TABLE a BY b /COMPARETEST TYPE=MEAN ADJUST=HOLM.");
    assert_eq!(e.expected, ["BONFERRONI", "BH", "NONE"]);
    let e = err("CTABLES /SIGTEST TYPE=CHISQUARE /TABLE a BY b.");
    assert!(e.message.contains("must follow a /TABLE"), "{e}");
    // MERGE=YES points at itself
    let source = "CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES STYLE=APA.";
    let e = err(source);
    let (from, to) = (e.span.column as usize - 1, e.span.end_column as usize - 1);
    assert_eq!(&source[from..to], "MERGE=YES");
}

#[test]
fn mistakes_say_what_was_expected() {
    let e = err("CTABLES /TABLE a [COLPCT].");
    assert!(e.message.contains("unknown summary function COLPCT"), "{e}");
    assert!(e.hint.unwrap().contains("COLPCT.VALIDN"));
    let e = err("CTABLES /SLABELS POSITION=ROW /TABLE a.");
    assert!(e.message.contains("must follow a /TABLE"), "{e}");
    let e = err("CTABLES /TABLE a [MEAN A8].");
    assert!(e.message.contains("cannot be used for a summary"), "{e}");
    let e = err("CTABLES /TABLE a [PTILE].");
    assert!(e.expected[0].contains("percentile"), "{e}");
    let e = err("CTABLES /TABLE (a + b) [COUNT].");
    assert!(e.message.contains("follow a variable name"), "{e}");
    let e = err("CTABLES /VLABELS VARIABLES=a.");
    assert_eq!(e.expected, ["DISPLAY"]);
    assert!(
        err("CTABLES /FORMAT EMPTY=ZERO.")
            .expected
            .contains(&"/TABLE".to_string())
    );
}

#[test]
fn help_lists_the_summary_functions() {
    let text = help(Some("CTABLES")).unwrap();
    assert!(text.contains("COLPCT.COUNT") && text.contains("PTILE"));
}

#[test]
fn multiple_response_sets_stand_like_variables() {
    let c = one("CTABLES /MRSETS COUNTDUPLICATES=YES\n\
         /TABLE $q5 [COUNT, RESPONSES, COLPCT.RESPONSES, ROWPCT.RESPONSES.COUNT, UCOLPCT.COUNT.RESPONSES] BY gender\n\
         /CATEGORIES VARIABLES=$q5 [q5_3, q5_1] TOTAL=YES.");
    assert!(c.count_duplicates);
    assert_eq!(c.tables[0].columns, Some(var("gender")));
    let AxisExpr::Variable(v) = &c.tables[0].rows else {
        panic!()
    };
    assert_eq!(v.name, "$q5");
    let functions: Vec<SummaryFunction> = v
        .summaries
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.function)
        .collect();
    let f = |statistic, unweighted| SummaryFunction {
        statistic,
        unweighted,
    };
    assert_eq!(
        functions,
        [
            f(Statistic::Count, false),
            f(Statistic::Responses, false),
            f(
                Statistic::Percent(PercentBase::Column, PercentOf::Responses),
                false
            ),
            f(
                Statistic::Percent(PercentBase::Row, PercentOf::ResponsesCount),
                false
            ),
            f(
                Statistic::Percent(PercentBase::Column, PercentOf::CountResponses),
                true
            ),
        ]
    );
    assert_eq!(functions[3].name(), "ROWPCT.RESPONSES.COUNT");
    assert_eq!(functions[4].name(), "UCOLPCT.COUNT.RESPONSES");
    let categories = &c.tables[0].categories[0];
    assert_eq!(categories.vars, [VarItem::Name("$q5".into())]);
    assert_eq!(
        categories.values,
        Some(vec![
            CategoryItem::Name("q5_3".into()),
            CategoryItem::Name("q5_1".into())
        ])
    );
    // a word that starts a range is still a range
    let c = one("CTABLES /TABLE a /CATEGORIES VARIABLES=a [LO THRU 2, 5].");
    assert_eq!(
        c.tables[0].categories[0].values.as_ref().unwrap()[0],
        CategoryItem::Range(Bound::Lowest, Bound::Number(2.0))
    );
    assert_eq!(
        err("CTABLES /MRSETS DUPLICATES=YES /TABLE a.").expected,
        ["COUNTDUPLICATES"]
    );
}
