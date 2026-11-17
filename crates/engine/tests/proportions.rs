//! PROPORTIONS on small invented data. Expected values come from base R run as a black box
//! (prop.test, binom.test, mcnemar.test, pbinom, dbinom, qbeta), or, for the intervals base R
//! lacks (Agresti-Coull, Anscombe, logit, Agresti-Min, Bonett-Price, Newcombe, Agresti-Caffo,
//! Brown-Li-Jeffreys, Hauck-Anderson), from the formulas worked out by hand in R arithmetic.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

/// `line` repeated `times` times, one per data line.
fn lines(spec: &[(&str, usize)]) -> String {
    spec.iter()
        .map(|(line, times)| format!("{line}\n").repeat(*times))
        .collect()
}

/// Twenty valid answers, 7 of them 2; a user-missing 9 and a blank. `score` has the same 7
/// cases at 6 or more (none at 5) and is missing on the last two cases. `other` is missing on
/// the first case.
fn answers() -> String {
    let mut data = String::new();
    let high = [6, 7, 8, 9, 10, 10, 6];
    let low = [0, 1, 2, 3, 4, 4, 3, 2, 1, 0, 2, 3, 4];
    for (i, s) in high.iter().enumerate() {
        let other = if i == 0 { "." } else { "1" };
        data.push_str(&format!("2 {s} {other}\n"));
    }
    for s in low {
        data.push_str(&format!("1 {s} 1\n"));
    }
    data.push_str("9 . 1\n. . 1\n");
    format!(
        "DATA LIST FREE /answer score other.\nBEGIN DATA\n{data}END DATA.\nMISSING VALUES answer (9).\n"
    )
}

/// The cells of a pair, before and after: 5 yes-yes, 6 yes-no, 2 no-yes, 7 no-no, and a case
/// missing after.
fn pairs() -> String {
    let data = lines(&[
        ("1 1 1", 5),
        ("1 0 1", 6),
        ("0 1 0", 2),
        ("0 0 .", 7),
        ("1 . 1", 1),
    ]);
    format!("DATA LIST FREE /before after third.\nBEGIN DATA\n{data}END DATA.\n")
}

/// Group 1: 12 of 20 pass; group 2: 5 of 18; group 3 (ignored by default): 2 of 3; a case
/// without a group and a case without a result. `school` names the groups.
fn groups() -> String {
    let data = lines(&[
        ("1 1 n", 12),
        ("1 0 n", 8),
        ("2 1 s", 5),
        ("2 0 s", 13),
        ("3 1 e", 2),
        ("3 0 e", 1),
        (". 1 x", 1),
        ("1 . n", 1),
    ]);
    format!("DATA LIST FREE /g passed (2F1) school (A1).\nBEGIN DATA\n{data}END DATA.\n")
}

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let result = Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (
        tables.filter(|t| t.title != "Echo").collect(),
        warnings.collect(),
    )
}

fn fails(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title} in {tables:#?}"))
}

fn close(cell: &Cell, expected: f64) {
    match cell {
        Cell::Number(got) => assert!((got - expected).abs() < 1e-9, "{got} != {expected}"),
        other => panic!("{other:?} is no number, expected {expected}"),
    }
}

/// The row whose method column (1) reads `method`, from the rows of `item` on.
fn row<'a>(table: &'a Table, method: &str) -> &'a [Cell] {
    table
        .rows
        .iter()
        .find(|r| r[1] == Cell::Text(method.into()))
        .unwrap_or_else(|| panic!("no row {method} in {table:#?}"))
}

/// Checks the bounds in the last two columns.
fn bounds(r: &[Cell], low: f64, high: f64) {
    let n = r.len();
    close(&r[n - 2], low);
    close(&r[n - 1], high);
}

const ALL_ONE: &str = "TESTTYPES=EXACT MIDP SCORE SCORECC WALD WALDCC CITYPES=AGRESTI_COULL \
                       ANSCOMBE CLOPPER_PEARSON JEFFREYS LOGIT WALD WALDCC WILSON_SCORE WILSON_SCORECC";

/// r = 7 of n = 20 against 0.3, every test and interval.
fn check_seven_of_twenty(tables: &[Table], first: &str) {
    let t = titled(tables, "One-Sample Proportions Tests");
    assert_eq!(t.caption.as_deref(), Some("Test value = 0.3."));
    let r = row(t, "Exact Binomial");
    assert_eq!(r[0], Cell::Text(first.into()));
    close(&r[2], 7.0);
    close(&r[3], 20.0);
    close(&r[4], 0.35);
    close(&r[5], 0.05);
    assert_eq!((&r[6], &r[7]), (&Cell::Empty, &Cell::Empty));
    close(&r[8], 0.783980375598152);
    close(&row(t, "Mid-p Adjusted Binomial")[8], 0.619718390380915);
    let r = row(t, "Score");
    close(&r[6], 0.102469507659596);
    close(&r[7], 0.487950036474267);
    close(&r[8], 0.625585231524325);
    let r = row(t, "Continuity-Corrected Score");
    close(&r[7], 0.243975018237133);
    close(&r[8], 0.807250167932007);
    let r = row(t, "Wald");
    close(&r[6], 0.106653645038508);
    close(&r[7], 0.468807230938495);
    close(&r[8], 0.639207430904606);
    let r = row(t, "Continuity-Corrected Wald");
    close(&r[7], 0.234403615469248);
    close(&r[8], 0.814671652089333);

    let t = titled(tables, "One-Sample Proportions Confidence Intervals");
    assert_eq!(t.column_groups[6], vec!["95% Confidence Interval"]);
    close(&row(t, "Agresti-Coull")[5], 0.106653645038508);
    bounds(
        row(t, "Agresti-Coull"),
        0.179926361438228,
        0.568411185977618,
    );
    bounds(row(t, "Anscombe"), 0.185744895150073, 0.575011072837163);
    bounds(
        row(t, "Clopper-Pearson (Exact)"),
        0.153909204784541,
        0.592188534532828,
    );
    bounds(row(t, "Jeffreys"), 0.172276213631912, 0.567766093841496);
    bounds(row(t, "Logit"), 0.176843017167936, 0.574395110753365);
    bounds(row(t, "Wald"), 0.140962696904606, 0.559037303095394);
    bounds(
        row(t, "Wald (Continuity Corrected)"),
        0.115962696904606,
        0.584037303095394,
    );
    bounds(row(t, "Wilson Score"), 0.181191824101082, 0.567145723314764);
    bounds(
        row(t, "Wilson Score (Continuity Corrected)"),
        0.163086679716309,
        0.590510433848573,
    );
}

#[test]
fn one_sample_every_test_and_interval() {
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /ONESAMPLE answer TESTVAL=0.3 {ALL_ONE}.",
        answers()
    ));
    check_seven_of_twenty(&tables, "answer");
    // the same 7 of 20 by a cut point, and by the middle of the range (5 for score, 1.5 for
    // answer); the types written as subcommands
    for success in ["CUTPOINT(6)", "MIDPOINT"] {
        let (tables, _) = run(&format!(
            "{}PROPORTIONS /TESTVAL=.3 /ONESAMPLE score answer /SUCCESS={success} /{}.",
            answers(),
            ALL_ONE.replace(" CITYPES", " /CITYPES")
        ));
        check_seven_of_twenty(&tables, "score");
        let t = titled(&tables, "One-Sample Proportions Tests");
        assert_eq!(t.rows.len(), 12);
        assert_eq!(t.rows[6][0], Cell::Text("answer".into()));
    }
}

#[test]
fn one_sample_defaults_missing_values_and_level() {
    // defaults: mid-p and score against 0.5; Agresti-Coull, Jeffreys and Wilson
    let (tables, _) = run(&format!("{}PROPORTIONS /ONESAMPLE answer.", answers()));
    let t = titled(&tables, "One-Sample Proportions Tests");
    assert_eq!(t.rows.len(), 2);
    close(&row(t, "Mid-p Adjusted Binomial")[8], 0.189247131347656);
    close(&row(t, "Score")[7], -1.34164078649987);
    close(&row(t, "Score")[8], 0.179712494879);
    let t = titled(&tables, "One-Sample Proportions Confidence Intervals");
    let methods: Vec<Cell> = t.rows.iter().map(|r| r[1].clone()).collect();
    assert_eq!(
        methods,
        ["Agresti-Coull", "Jeffreys", "Wilson Score"].map(|m| Cell::Text(m.into()))
    );

    // USERMISSING=INCLUDE: 9 is valid and, as the highest value, the success: 1 of 21
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /ONESAMPLE answer /MISSING USERMISSING=INCLUDE.",
        answers()
    ));
    let t = titled(&tables, "One-Sample Proportions Tests");
    close(&row(t, "Mid-p Adjusted Binomial")[3], 21.0);
    close(&row(t, "Mid-p Adjusted Binomial")[8], 1.09672546386718e-05);
    close(&row(t, "Score")[7], -4.14613991448386);
    let t = titled(&tables, "One-Sample Proportions Confidence Intervals");
    // Agresti-Coull's lower bound (-0.0091) is cut to 0
    bounds(row(t, "Agresti-Coull"), 0.0, 0.244239497252723);
    bounds(row(t, "Jeffreys"), 0.00518704310790212, 0.201755968321683);
    bounds(
        row(t, "Wilson Score"),
        0.00845598780982638,
        0.226693600741226,
    );

    // LISTWISE: the case missing on other leaves answer with 6 of 19; ANALYSIS keeps it
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /ONESAMPLE answer other /MISSING SCOPE=LISTWISE.",
        answers()
    ));
    let t = titled(&tables, "One-Sample Proportions Tests");
    close(&t.rows[0][2], 6.0);
    close(&t.rows[0][3], 19.0);

    // FIRST: the 13 ones are the successes; LEVEL(1 2) makes every valid case one
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /ONESAMPLE answer /SUCCESSVALUE=FIRST /CRITERIA CILEVEL=90.",
        answers()
    ));
    close(
        &titled(&tables, "One-Sample Proportions Tests").rows[0][2],
        13.0,
    );
    let t = titled(&tables, "One-Sample Proportions Confidence Intervals");
    assert_eq!(t.column_groups[6], vec!["90% Confidence Interval"]);
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /ONESAMPLE answer CITYPES=CLOPPER_PEARSON WILSON_SCORECC LOGIT /SUCCESS=LEVEL(1 2).",
        answers()
    ));
    let t = titled(&tables, "One-Sample Proportions Confidence Intervals");
    close(&t.rows[0][2], 20.0);
    // 20 of 20: Clopper-Pearson reaches 1; the corrected Wilson upper bound is 1; logit has none
    close(&row(t, "Clopper-Pearson (Exact)")[7], 1.0);
    close(&row(t, "Wilson Score (Continuity Corrected)")[7], 1.0);
    assert_eq!(row(t, "Logit")[6], Cell::Empty);
}

#[test]
fn one_sample_string_levels_and_edges() {
    // 2 of 5 and 0 of 5 successes among strings; the corrected Wilson bounds agree with R
    let source =
        "DATA LIST FREE /grade (A2) none (A1).\nBEGIN DATA\nb x\nc x\nB x\nab x\nb x\nEND DATA.\n";
    let (tables, _) = run(&format!(
        "{source}PROPORTIONS /ONESAMPLE grade none CITYPES=WILSON_SCORECC CLOPPER_PEARSON /SUCCESS=LEVEL('b').",
    ));
    let t = titled(&tables, "One-Sample Proportions Confidence Intervals");
    close(&t.rows[0][2], 2.0);
    bounds(&t.rows[0], 0.0725840399856634, 0.829576419356412);
    close(&t.rows[2][2], 0.0);
    bounds(&t.rows[2], 0.0, 0.537056017467523);
    bounds(&t.rows[3], 0.0, 0.521823750104981);
    // Wald with 0 successes has no z
    let (tables, _) = run(&format!(
        "{source}PROPORTIONS /ONESAMPLE none TESTTYPES=WALD /SUCCESS=LEVEL('y')."
    ));
    let t = titled(&tables, "One-Sample Proportions Tests");
    assert_eq!(t.rows[0][7], Cell::Empty);
}

#[test]
fn weights_count_as_frequencies() {
    // the 20 answers collapsed into three weighted cases give the same tables
    let weighted = "DATA LIST FREE /answer score w.\nBEGIN DATA\n2 7 3\n2 6 4\n1 0 13\n9 . 5\nEND DATA.\n\
                    MISSING VALUES answer (9).\nWEIGHT BY w.\n";
    let (tables, _) = run(&format!(
        "{weighted}PROPORTIONS /ONESAMPLE answer TESTVAL=0.3 {ALL_ONE}."
    ));
    check_seven_of_twenty(&tables, "answer");
    // 7.2 of 20: the asymptotic tests use 7.2, the exact test 7 (hand: 1.2 / sqrt(4.2))
    let fraction =
        "DATA LIST FREE /answer w.\nBEGIN DATA\n2 3.2\n2 4\n1 12.8\nEND DATA.\nWEIGHT BY w.\n";
    let (tables, _) = run(&format!(
        "{fraction}PROPORTIONS /ONESAMPLE answer TESTVAL=0.3 TESTTYPES=EXACT SCORE."
    ));
    let t = titled(&tables, "One-Sample Proportions Tests");
    close(&row(t, "Exact Binomial")[2], 7.2);
    close(&row(t, "Exact Binomial")[8], 0.783980375598152);
    close(&row(t, "Score")[7], 1.2 / 4.2f64.sqrt());

    // paired and independent analyses weighted the same way
    let weighted_pairs = "DATA LIST FREE /before after w.\nBEGIN DATA\n1 1 5\n1 0 6\n0 1 2\n0 0 7\n1 . 3\nEND DATA.\nWEIGHT BY w.\n";
    let (tables, _) = run(&format!(
        "{weighted_pairs}PROPORTIONS /PAIREDSAMPLES before WITH after {ALL_PAIRED}."
    ));
    check_pair(&tables);
    let weighted_groups = "DATA LIST FREE /g passed (2F1) w.\nBEGIN DATA\n1 1 12\n1 0 8\n2 1 5\n2 0 13\n3 1 4\nEND DATA.\nWEIGHT BY w.\n";
    let (tables, _) = run(&format!(
        "{weighted_groups}PROPORTIONS /INDEPENDENTSAMPLES passed BY g {ALL_INDEPENDENT}."
    ));
    check_groups(&tables, "1", "2");
}

const ALL_PAIRED: &str = "TESTTYPES=EXACT MIDP MCNEMAR MCNEMARCC WALD WALDCC \
                          CITYPES=AGRESTI_MIN BONETT_PRICE NEWCOMBE WALD WALDCC";

/// The 5, 6, 2, 7 pair of before with after.
fn check_pair(tables: &[Table]) {
    let t = titled(tables, "Paired-Samples Proportions Statistics");
    assert_eq!(t.rows[0][0], Cell::Text("before - after".into()));
    close(&t.rows[0][2], 11.0);
    close(&t.rows[0][3], 20.0);
    close(&t.rows[0][4], 0.55);
    close(&t.rows[0][5], 0.111242977306435);
    close(&t.rows[1][4], 0.35);
    let t = titled(tables, "Paired-Samples Proportions Tests");
    let r = row(t, "Exact Binomial");
    close(&r[2], 0.2);
    close(&r[5], 0.2890625);
    close(&row(t, "Mid-p Adjusted Binomial")[5], 0.1796875);
    let r = row(t, "McNemar");
    close(&r[3], 0.14142135623731);
    close(&r[4], std::f64::consts::SQRT_2); // R: 1.4142135623731 = 4 / sqrt(8)
    close(&r[5], 0.157299207050284);
    let r = row(t, "Continuity-Corrected McNemar");
    close(&r[4], 1.06066017177982);
    close(&r[5], 0.288844366346485);
    let r = row(t, "Wald");
    close(&r[3], 0.134164078649987);
    close(&r[4], 1.49071198499986);
    close(&r[5], 0.136037128114144);
    let r = row(t, "Continuity-Corrected Wald");
    close(&r[4], 1.11803398874989);
    close(&r[5], 0.263552477282973);
    let t = titled(tables, "Paired-Samples Proportions Confidence Intervals");
    close(&row(t, "Agresti-Min")[3], 0.134164078649987);
    bounds(
        row(t, "Agresti-Min"),
        -0.0744234927210941,
        0.438059856357458,
    );
    bounds(
        row(t, "Bonett-Price"),
        -0.0894689246954822,
        0.453105288331846,
    );
    bounds(row(t, "Newcombe"), -0.0793090910218888, 0.43751865643883);
    bounds(row(t, "Wald"), -0.0629567621729744, 0.462956762172974);
    bounds(
        row(t, "Wald (Continuity Corrected)"),
        -0.112956762172974,
        0.512956762172974,
    );
}

#[test]
fn paired_every_test_and_interval() {
    let (tables, warnings) = run(&format!(
        "{}PROPORTIONS /PAIREDSAMPLES before WITH after {ALL_PAIRED}.",
        pairs()
    ));
    check_pair(&tables);
    assert!(warnings.is_empty(), "{warnings:?}");
    // defaults, and FIRST turns the table around: 0 is the success
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /PAIREDSAMPLES before after /SUCCESSVALUE=FIRST.",
        pairs()
    ));
    let t = titled(&tables, "Paired-Samples Proportions Tests");
    assert_eq!(t.rows.len(), 2);
    close(&row(t, "Mid-p Adjusted Binomial")[5], 0.1796875);
    close(&row(t, "McNemar")[4], -std::f64::consts::SQRT_2);
    let t = titled(&tables, "Paired-Samples Proportions Confidence Intervals");
    bounds(
        row(t, "Bonett-Price"),
        -0.453105288331846,
        0.0894689246954822,
    );
    bounds(row(t, "Newcombe"), -0.43751865643883, 0.0793090910218888);
}

#[test]
fn paired_lists_and_listwise() {
    // one list: every variable with every later one
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /PAIREDSAMPLES before after third.",
        pairs()
    ));
    let t = titled(&tables, "Paired-Samples Proportions Statistics");
    let names: Vec<&Cell> = t.rows.iter().step_by(2).map(|r| &r[0]).collect();
    assert_eq!(names.len(), 3);
    assert_eq!(*names[2], Cell::Text("after - third".into()));
    // (PAIRED) with an unmatched variable warns and leaves it out
    let (tables, warnings) = run(&format!(
        "{}PROPORTIONS /PAIREDSAMPLES before third WITH after (PAIRED).",
        pairs()
    ));
    assert_eq!(
        titled(&tables, "Paired-Samples Proportions Statistics")
            .rows
            .len(),
        2
    );
    assert!(
        warnings.iter().any(|w| w.contains("unmatched")),
        "{warnings:?}"
    );
    // LISTWISE drops the cases missing on third from before - after too: 13 cases left
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /PAIREDSAMPLES before WITH after third /MISSING SCOPE=LISTWISE.",
        pairs()
    ));
    let t = titled(&tables, "Paired-Samples Proportions Statistics");
    close(&t.rows[0][3], 13.0);
}

const ALL_INDEPENDENT: &str = "TESTTYPES=HAUCK_ANDERSON WALD WALDCC WALDH0 WALDH0CC \
     CITYPES=AGRESTI_CAFFO BROWN_LI_JEFFREYS HAUCK_ANDERSON NEWCOMBE NEWCOMBECC WALD WALDCC";

/// 12 of 20 against 5 of 18.
fn check_groups(tables: &[Table], first: &str, second: &str) {
    let t = titled(tables, "Independent-Samples Proportions Statistics");
    assert_eq!(t.rows[0][1], Cell::Text(format!("g = {first}")));
    assert_eq!(t.rows[1][1], Cell::Text(format!("g = {second}")));
    close(&t.rows[0][2], 12.0);
    close(&t.rows[0][3], 20.0);
    close(&t.rows[0][5], 0.109544511501033);
    close(&t.rows[1][2], 5.0);
    close(&t.rows[1][3], 18.0);
    close(&t.rows[1][5], 0.105571798620291);
    let t = titled(tables, "Independent-Samples Proportions Tests");
    let r = row(t, "Wald H0");
    close(&r[2], 0.322222222222222);
    close(&r[3], 0.161544084472537);
    close(&r[4], 1.99463956401945);
    close(&r[5], 0.0460822052413855);
    let r = row(t, "Continuity-Corrected Wald H0");
    close(&r[4], 1.66793135956799);
    close(&r[5], 0.0953293535447013);
    let r = row(t, "Wald");
    close(&r[3], 0.152136138586212);
    close(&r[4], 2.1179860696913);
    close(&r[5], 0.0341762474652322);
    let r = row(t, "Continuity-Corrected Wald");
    close(&r[4], 1.77107455827634);
    close(&r[5], 0.0765483041199721);
    let r = row(t, "Hauck-Anderson");
    close(&r[4], 1.88372959371426);
    close(&r[5], 0.0596015612974287);
    let t = titled(
        tables,
        "Independent-Samples Proportions Confidence Intervals",
    );
    bounds(
        row(t, "Agresti-Caffo"),
        0.00360231005103501,
        0.578215871767147,
    );
    bounds(
        row(t, "Brown-Li-Jeffreys"),
        0.00547977479574724,
        0.606049047259391,
    );
    bounds(
        row(t, "Hauck-Anderson"),
        -0.0119161438717981,
        0.656360588316242,
    );
    bounds(row(t, "Newcombe"), 0.00776268839502237, 0.559230398583366);
    bounds(
        row(t, "Newcombe (Continuity Corrected)"),
        -0.0274771692069565,
        0.585307421575831,
    );
    bounds(row(t, "Wald"), 0.0240408698462518, 0.620403574598193);
    bounds(
        row(t, "Wald (Continuity Corrected)"),
        -0.0287369079315259,
        0.67318135237597,
    );
}

#[test]
fn independent_every_test_and_interval() {
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY g {ALL_INDEPENDENT}.",
        groups()
    ));
    check_groups(&tables, "1", "2");
    // string groups chosen by SELECT; the defaults
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY school /SELECT=LEVEL('n', 's').",
        groups()
    ));
    let t = titled(&tables, "Independent-Samples Proportions Tests");
    assert_eq!(t.rows.len(), 1);
    close(&row(t, "Wald H0")[4], 1.99463956401945);
    let t = titled(
        &tables,
        "Independent-Samples Proportions Confidence Intervals",
    );
    assert_eq!(t.rows.len(), 2);
    bounds(row(t, "Newcombe"), 0.00776268839502237, 0.559230398583366);
}

#[test]
fn independent_cut_points() {
    // below 2.5: groups 1 and 2 (17 of 38); at or above: group 3 (2 of 3)
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY g SELECT=CUTPOINT(2.5).",
        groups()
    ));
    let t = titled(&tables, "Independent-Samples Proportions Statistics");
    assert_eq!(t.rows[0][1], Cell::Text("g = < 2.5".into()));
    close(&t.rows[0][2], 17.0);
    close(&t.rows[0][3], 38.0);
    close(&t.rows[1][3], 3.0);
    // the middle of 1 to 3 is 2: group 1 against groups 2 and 3 (7 of 21)
    let (tables, _) = run(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY g SELECT=MIDPOINT.",
        groups()
    ));
    let t = titled(&tables, "Independent-Samples Proportions Statistics");
    close(&t.rows[0][3], 20.0);
    close(&t.rows[1][2], 7.0);
    close(&t.rows[1][3], 21.0);
}

#[test]
fn errors_and_skips() {
    let e = fails(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY school.",
        groups()
    ));
    assert!(e.contains("quoted strings"), "{e}");
    let e = fails(&format!(
        "{}PROPORTIONS /ONESAMPLE school /SUCCESS=MIDPOINT.",
        groups()
    ));
    assert!(e.contains("numeric"), "{e}");
    let e = fails(&format!(
        "{}PROPORTIONS /ONESAMPLE passed /SUCCESS=LEVEL('a').",
        groups()
    ));
    assert!(e.contains("numbers"), "{e}");
    // a group with no cases skips the variable with a warning
    let (tables, warnings) = run(&format!(
        "{}PROPORTIONS /INDEPENDENTSAMPLES passed BY g SELECT=LEVEL(1, 7).",
        groups()
    ));
    assert!(tables.is_empty());
    assert!(
        warnings.iter().any(|w| w.contains("no valid cases")),
        "{warnings:?}"
    );
}

#[test]
fn split_file_runs_each_group() {
    let (tables, _) = run(&format!(
        "{}SORT CASES BY third.\nSPLIT FILE BY third.\nPROPORTIONS /ONESAMPLE before TESTTYPES=SCORE CITYPES=NONE.",
        pairs()
    ));
    let t = titled(&tables, "One-Sample Proportions Tests");
    // third = 0: 2 cases, none before; third = 1: 12 cases, all before (the missing third
    // forms no group)
    assert!(t.rows.len() >= 2, "{t:#?}");
}
