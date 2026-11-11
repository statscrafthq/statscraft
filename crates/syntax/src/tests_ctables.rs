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
        total_summaries: None,
    })
}

#[test]
fn nesting_binds_tighter_than_stacking() {
    let c = one("CTABLES /TABLE a + b > c BY (d + e) > f.");
    let t = &c.tables[0];
    assert_eq!(
        t.rows,
        Some(AxisExpr::Stack(
            Box::new(var("a")),
            Box::new(AxisExpr::Nest(Box::new(var("b")), Box::new(var("c"))))
        ))
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
    let Some(AxisExpr::Variable(v)) = &c.tables[0].rows else {
        panic!()
    };
    assert_eq!(v.scale, Some(false));
    let s = v.summaries.as_ref().unwrap();
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].label.as_deref(), Some("N"));
    assert_eq!(s[0].format.as_deref(), Some("F8"));
    let more = one(
        "CTABLES /TABLE q1 [COUNT neqUAL5, COLPCT PCTPAREN8.1, MEAN NEGPAREN9.2, SUM paren6, MEAN DOT10.2, VALIDN N5, MEAN E10.3].",
    );
    let Some(AxisExpr::Variable(v)) = &more.tables[0].rows else {
        panic!()
    };
    let formats: Vec<_> = v
        .summaries
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.format.clone().unwrap())
        .collect();
    assert_eq!(
        formats,
        [
            "NEQUAL5",
            "PCTPAREN8.1",
            "NEGPAREN9.2",
            "PAREN6",
            "DOT10.2",
            "N5",
            "E10.3"
        ]
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
fn totals_in_a_summary_list() {
    let c = one("CTABLES /TABLE q1 [COUNT TOTALS [MEAN 'Avg', VALIDN], COLPCT] BY g.");
    let Some(AxisExpr::Variable(v)) = &c.tables[0].rows else {
        panic!()
    };
    let names = |list: &[Summary]| -> Vec<String> {
        list.iter().map(|s| s.function.name()).collect::<Vec<_>>()
    };
    assert_eq!(
        names(v.summaries.as_ref().unwrap()),
        ["COUNT", "COLPCT.COUNT"]
    );
    let totals = v.total_summaries.as_ref().unwrap();
    assert_eq!(names(totals), ["MEAN", "VALIDN"]);
    assert_eq!(totals[0].label.as_deref(), Some("Avg"));
    // after a parenthesis the totals go to every variable, before their own
    let c = one("CTABLES /TABLE (a + b [TOTALS [MAXIMUM]]) [TOTALS [MEAN]].");
    let Some(AxisExpr::Stack(a, b)) = &c.tables[0].rows else {
        panic!()
    };
    let (AxisExpr::Variable(a), AxisExpr::Variable(b)) = (&**a, &**b) else {
        panic!()
    };
    assert_eq!(a.summaries, None);
    assert_eq!(names(a.total_summaries.as_ref().unwrap()), ["MEAN"]);
    assert_eq!(
        names(b.total_summaries.as_ref().unwrap()),
        ["MEAN", "MAXIMUM"]
    );
    assert!(
        err("CTABLES /TABLE a [COUNT TOTALS [MEAN TOTALS [COUNT]]].")
            .message
            .contains("at most one TOTALS")
    );
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
    assert!(a.descending && a.include_missing && a.total && a.total_before);
    assert_eq!(a.key, CategoryOrder::Label);
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
fn confidence_limits_and_their_level() {
    let c = one(
        "CTABLES /TABLE a [COUNT.LCL, COLPCT.UCL, COLPCT.VALIDN.SE, MEAN.SE, PTILE.LCL 75] /CRITERIA CILEVEL=90\n\
         /TABLE b.",
    );
    assert_eq!((c.tables[0].cilevel, c.tables[1].cilevel), (90.0, 95.0));
    let Some(AxisExpr::Variable(a)) = &c.tables[0].rows else {
        panic!()
    };
    let functions: Vec<(Statistic, Estimate, String)> = a
        .summaries
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| (s.function.statistic, s.function.estimate, s.function.name()))
        .collect();
    assert_eq!(
        functions,
        [
            (Statistic::Count, Estimate::Lower, "COUNT.LCL".into()),
            (
                Statistic::Percent(PercentBase::Column, PercentOf::Count),
                Estimate::Upper,
                "COLPCT.COUNT.UCL".into()
            ),
            (
                Statistic::Percent(PercentBase::Column, PercentOf::Validn),
                Estimate::StdError,
                "COLPCT.VALIDN.SE".into()
            ),
            (Statistic::Mean, Estimate::StdError, "MEAN.SE".into()),
            (Statistic::Ptile, Estimate::Lower, "PTILE.LCL".into()),
        ]
    );
    // only counts, percentages, means, sums and percentiles have them, and weighted only
    for bad in ["MEDIAN.SE", "MODE.LCL", "UCOUNT.LCL", "COLPCT.SUM.LCL"] {
        let e = err(&format!("CTABLES /TABLE a [{bad}]."));
        assert!(
            e.message.contains("unknown summary function"),
            "{bad}: {}",
            e.message
        );
    }
    let e = err("CTABLES /TABLE a /CRITERIA CILEVEL=100.");
    assert!(e.message.contains("less than 100"), "{}", e.message);
    let e = err("CTABLES /CRITERIA CILEVEL=90 /TABLE a.");
    assert!(e.message.contains("must follow a /TABLE"), "{}", e.message);
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
            merge: false,
            style: CompareStyle::Apa,
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
    let e = err("CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES STYLE=BOLD.");
    assert_eq!(e.expected, ["APA", "SIMPLE"]);
}

#[test]
fn merged_letters_default_to_apa_and_showsig_keeps_them_apart() {
    let c = one("CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES.");
    let test = &c.tables[0].comparetests[0];
    assert!(test.merge && test.style == CompareStyle::Apa);
    let c = one("CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES STYLE=SIMPLE.");
    assert_eq!(c.tables[0].comparetests[0].style, CompareStyle::Simple);
    let c = one("CTABLES /TABLE a BY b /COMPARETEST TYPE=PROP MERGE=YES SHOWSIG=YES.");
    assert!(!c.tables[0].comparetests[0].merge);
}

#[test]
fn summaries_after_a_parenthesis_go_to_every_innermost_variable() {
    // the manual's equivalent forms: (SEX + HAPPY [COLPCT])[COUNT] is
    // SEX [COUNT] + HAPPY [COUNT, COLPCT]
    let grouped = one("CTABLES /TABLE (sex + happy [COLPCT]) [COUNT] BY x.");
    let spelled = one("CTABLES /TABLE sex [COUNT] + happy [COUNT, COLPCT] BY x.");
    assert_eq!(grouped.tables[0].rows, spelled.tables[0].rows);
    // in a nesting only the innermost variable gets them
    let nested = one("CTABLES /TABLE (a > b) [COUNT].");
    let spelled = one("CTABLES /TABLE a > b [COUNT].");
    assert_eq!(nested.tables[0].rows, spelled.tables[0].rows);
}

#[test]
fn computed_categories_and_their_properties() {
    let c = one(
        "CTABLES /PCOMPUTE &x = EXPR(([1] + [2]) * 0.8 - [3 THRU HI] / TOTAL ** 2)\n\
         /PCOMPUTE &y = EXPR(-['a'])\n\
         /PPROPERTIES &x &y LABEL='Both' FORMAT=COUNT F8.3 COLPCT PCT5.1 HIDESOURCECATS=YES\n\
         /TABLE a /CATEGORIES VARIABLES=a [1, 2, &x, 3].",
    );
    assert_eq!(c.pcomputes.len(), 2);
    assert_eq!(c.pcomputes[0].name, "x");
    // the text keeps the parentheses that matter and no others
    assert_eq!(
        c.pcomputes[0].expr.to_string(),
        "([1] + [2]) * 0.8 - [3 THRU HI] / TOTAL ** 2"
    );
    assert_eq!(c.pcomputes[1].expr.to_string(), "-['a']");
    let props = &c.pproperties[0];
    assert_eq!(props.names, ["x", "y"]);
    assert_eq!(props.label.as_deref(), Some("Both"));
    assert_eq!(props.formats.len(), 2);
    assert_eq!(props.formats[1].0.name(), "COLPCT.COUNT");
    assert_eq!(props.hide_sources, Some(true));
    assert_eq!(
        c.tables[0].categories[0].values.as_ref().unwrap()[2],
        CategoryItem::Computed("x".into())
    );
    // a - b - c is (a - b) - c
    let c = one("CTABLES /PCOMPUTE &d = EXPR([1] - [2] - [3]) /TABLE a.");
    assert_eq!(c.pcomputes[0].expr.to_string(), "[1] - [2] - [3]");
    let c = one("CTABLES /PCOMPUTE &d = EXPR([1] - ([2] - [3])) /TABLE a.");
    assert_eq!(c.pcomputes[0].expr.to_string(), "[1] - ([2] - [3])");
    let e = err("CTABLES /PCOMPUTE x = EXPR([1]) /TABLE a.");
    assert!(e.expected[0].contains("&diff"), "{e:?}");
}

#[test]
fn hidesmallcounts_takes_a_whole_number_from_two() {
    let hide = |source: &str| one(source).hide_small_counts;
    assert_eq!(
        hide("CTABLES /HIDESMALLCOUNTS COUNT=10 /TABLE a."),
        Some(10)
    );
    assert_eq!(hide("CTABLES /HIDESMALLCOUNTS /TABLE a."), Some(5));
    assert_eq!(hide("CTABLES /TABLE a."), None);
    for bad in ["1", "2.5"] {
        let e = err(&format!("CTABLES /HIDESMALLCOUNTS COUNT={bad} /TABLE a."));
        assert!(e.message.contains("whole number of 2 or more"), "{e}");
    }
}

#[test]
fn clabels_moves_the_labels_of_one_axis() {
    let moved = |source: &str| one(source).tables[0].labels_moved;
    assert_eq!(
        moved("CTABLES /TABLE a + b /CLABELS ROWLABELS=OPPOSITE."),
        LabelsMoved::RowsToColumns
    );
    assert_eq!(
        moved("CTABLES /TABLE a BY b /CLABELS COLLABELS=OPPOSITE."),
        LabelsMoved::ColumnsToRows
    );
    // a later CLABELS replaces an earlier one
    assert_eq!(
        moved("CTABLES /TABLE a BY b /CLABELS ROWLABELS=OPPOSITE /CLABELS AUTO."),
        LabelsMoved::No
    );
}

#[test]
fn a_second_by_gives_the_layers() {
    let c = one("CTABLES /TABLE a BY b BY c > d + e /SLABELS POSITION=LAYER.");
    let t = &c.tables[0];
    assert_eq!(t.columns, Some(var("b")));
    assert!(matches!(t.layers, Some(AxisExpr::Stack(..))));
    assert!(t.summaries_in_layers && !t.summaries_in_rows);
    // the columns may be left out between the two BYs
    let c = one("CTABLES /TABLE a BY BY c.");
    assert_eq!(
        (&c.tables[0].columns, &c.tables[0].layers),
        (&None, &Some(var("c")))
    );
    let e = err("CTABLES /TABLE a BY b BY c BY d.");
    assert!(!e.expected.contains(&"BY".to_string()), "{e:?}");
}

#[test]
fn a_percentage_without_a_suffix_counts_cases() {
    // the manual's own examples write [COLPCT] and [COUNT COLPCT] without commas
    let c = one("CTABLES /TABLE a [COUNT ROWPCT UCOLPCT, LAYERCOLPCT 'L' PCT5.1] BY b.");
    let Some(AxisExpr::Variable(v)) = &c.tables[0].rows else {
        panic!()
    };
    let s = v.summaries.as_ref().unwrap();
    let names: Vec<String> = s.iter().map(|s| s.function.name()).collect();
    assert_eq!(
        names,
        [
            "COUNT",
            "ROWPCT.COUNT",
            "UCOLPCT.COUNT",
            "LAYERCOLPCT.COUNT"
        ]
    );
    assert_eq!(s[3].label.as_deref(), Some("L"));
}

#[test]
fn mistakes_say_what_was_expected() {
    let e = err("CTABLES /TABLE a [COLPCT.CASES].");
    assert!(
        e.message.contains("unknown summary function COLPCT.CASES"),
        "{e}"
    );
    assert!(e.hint.unwrap().contains("COLPCT.VALIDN"));
    let e = err("CTABLES /SLABELS POSITION=ROW /TABLE a.");
    assert!(e.message.contains("must follow a /TABLE"), "{e}");
    let e = err("CTABLES /TABLE a [MEAN A8].");
    assert!(e.message.contains("cannot be used for a summary"), "{e}");
    let e = err("CTABLES /TABLE a [MEAN ADATE10].");
    assert!(
        e.message
            .contains("ADATE10 in CTABLES is not supported yet"),
        "{e}"
    );
    let e = err("CTABLES /TABLE a [MEAN PAREN4.4].");
    assert!(e.message.contains("PAREN4.4 has no room"), "{e}");
    let e = err("CTABLES /TABLE a [MEAN NEGPAREN8.X].");
    assert!(e.message.contains("invalid format NEGPAREN8.X"), "{e}");
    let e = err("CTABLES /TABLE a [PTILE].");
    assert!(e.expected[0].contains("percentile"), "{e}");
    let e = err("CTABLES /TABLE (a + b) [C].");
    assert!(e.message.contains("follows a variable name"), "{e}");
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
    let Some(AxisExpr::Variable(v)) = &c.tables[0].rows else {
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
        estimate: Estimate::Value,
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

#[test]
fn subtotals_in_a_category_list_and_in_a_computed_category() {
    let c = one(
        "CTABLES /PCOMPUTE &x = EXPR(SUBTOTAL[2] - SUBTOTAL) /TABLE a\n\
         /CATEGORIES VARIABLES=a [1, 2, SUBTOTAL='Low', 3, HSUBTOTAL, &x].",
    );
    assert_eq!(
        c.tables[0].categories[0].values,
        Some(vec![
            CategoryItem::Value(Value::Number(1.0)),
            CategoryItem::Value(Value::Number(2.0)),
            CategoryItem::Subtotal {
                label: Some("Low".into()),
                hide: false
            },
            CategoryItem::Value(Value::Number(3.0)),
            CategoryItem::Subtotal {
                label: None,
                hide: true
            },
            CategoryItem::Computed("x".into()),
        ])
    );
    assert_eq!(c.pcomputes[0].expr.to_string(), "SUBTOTAL[2] - SUBTOTAL");
    let e = err("CTABLES /PCOMPUTE &x = EXPR(SUBTOTAL[0]) /TABLE a.");
    assert!(e.message.contains("whole number from 1"), "{}", e.message);
}

#[test]
fn othernm_missing_and_a_summary_as_the_sort_key() {
    let c = one(
        "CTABLES /PCOMPUTE &x = EXPR([OTHERNM] - [MISSING]) /TABLE a BY b\n\
         /CATEGORIES VARIABLES=a [MISSING, 1, OTHERNM, &x]\n\
         /CATEGORIES VARIABLES=b ORDER=D KEY=PTILE(age, 75).",
    );
    let t = &c.tables[0];
    assert_eq!(
        t.categories[0].values,
        Some(vec![
            CategoryItem::Missing,
            CategoryItem::Value(Value::Number(1.0)),
            CategoryItem::OtherNm,
            CategoryItem::Computed("x".into()),
        ])
    );
    assert_eq!(c.pcomputes[0].expr.to_string(), "[OTHERNM] - [MISSING]");
    let CategoryOrder::Summary {
        function,
        variable,
        percentile,
    } = &t.categories[1].key
    else {
        panic!("{:?}", t.categories[1].key)
    };
    assert_eq!(function.statistic, Statistic::Ptile);
    assert_eq!(
        (variable.as_deref(), *percentile),
        (Some("age"), Some(75.0))
    );
    let c = one("CTABLES /TABLE a /CATEGORIES VARIABLES=a KEY=UCOUNT.");
    assert!(matches!(
        &c.tables[0].categories[0].key,
        CategoryOrder::Summary { function, variable: None, .. } if function.unweighted
    ));
    let e = err("CTABLES /TABLE a /CATEGORIES VARIABLES=a KEY=PTILE(age).");
    assert!(
        e.expected.iter().any(|x| x.contains("PTILE(variable")),
        "{e:?}"
    );
}

#[test]
fn an_effective_base_weight_and_the_e_functions() {
    let c = one("CTABLES /WEIGHT VARIABLE=w /TABLE a [ECOUNT, EVALIDN, ETOTALN, COUNT].");
    assert_eq!(c.weight.as_deref(), Some("w"));
    let Some(AxisExpr::Variable(a)) = &c.tables[0].rows else {
        panic!()
    };
    let functions: Vec<Statistic> = a
        .summaries
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.function.statistic)
        .collect();
    assert_eq!(
        functions,
        [
            Statistic::Ecount,
            Statistic::Evalidn,
            Statistic::Etotaln,
            Statistic::Count
        ]
    );
    assert_eq!(
        one("CTABLES /WEIGHT=w /TABLE a.").weight.as_deref(),
        Some("w")
    );
    let e = err("CTABLES /TABLE a /WEIGHT VARIABLE=w.");
    assert!(
        e.message.contains("before the first /TABLE"),
        "{}",
        e.message
    );
    // an effective base needs the weights, so there is no unweighted one
    let e = err("CTABLES /TABLE a [UECOUNT].");
    assert!(
        e.message.contains("unknown summary function UECOUNT"),
        "{}",
        e.message
    );
}

#[test]
fn labels_to_the_layers_and_an_empty_row_axis() {
    let c = one(
        "CTABLES /TABLE a BY b /CLABELS ROWLABELS=LAYER /TABLE BY b /TABLE BY BY c /CLABELS COLLABELS=LAYER.",
    );
    assert_eq!(c.tables[0].labels_moved, LabelsMoved::RowsToLayers);
    assert_eq!(c.tables[1].rows, None);
    assert!(c.tables[1].columns.is_some() && c.tables[1].layers.is_none());
    assert!(c.tables[2].rows.is_none() && c.tables[2].columns.is_none());
    assert!(c.tables[2].layers.is_some());
    assert_eq!(c.tables[2].labels_moved, LabelsMoved::ColumnsToLayers);
}

#[test]
fn smissing_variable_or_listwise() {
    assert!(one("CTABLES /SMISSING LISTWISE /TABLE a.").listwise);
    assert!(!one("CTABLES /SMISSING=VARIABLE /TABLE a.").listwise);
    assert!(!one("CTABLES /TABLE a.").listwise);
    let e = err("CTABLES /SMISSING PAIRWISE /TABLE a.");
    assert!(e.expected.iter().any(|x| x == "LISTWISE"), "{e:?}");
}
