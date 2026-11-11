//! VARSTOCASES, CASESTOVARS, FLIP, UPDATE and SORT VARIABLES on small synthetic datasets.
//! FLIP, UPDATE and SORT VARIABLES are also compared with the reference program in
//! `reference_data.rs`; the expected values here follow from the rules of the manual.

use statscraft_data::{Dataset, Measure, MissingValues, Values};
use statscraft_engine::{Engine, Table};
use statscraft_syntax::parse;

/// Runs every command and returns the tables of the last one.
fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(t) => tables = t,
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

/// The error of the last command; the ones before it must succeed.
fn error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let (last, before) = commands.split_last().unwrap();
    for command in before {
        engine.execute(&command.command).unwrap();
    }
    engine.execute(&last.command).unwrap_err().to_string()
}

fn engine(source: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, source);
    engine
}

fn data(engine: &Engine) -> &Dataset {
    engine.dataset().unwrap()
}

fn names(engine: &Engine) -> String {
    data(engine).variable_names().collect::<Vec<_>>().join(" ")
}

/// Numbers separated by spaces with `.` for system-missing, strings separated by `|`.
fn show(engine: &Engine, name: &str) -> String {
    match data(engine).variable(name).unwrap().values() {
        Values::Numeric(values) => {
            let text = values
                .iter()
                .map(|v| v.map_or(".".into(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    }
}

const REPEATED: &str =
    "DATA LIST LIST /caseid (F3) var1 (F5.2) var2 (F5.2) var3 (F5.2) var4 (F5.2).
BEGIN DATA
1 0 .05 5 3
2 7 1 5 4
3 6 3 6 2
END DATA.
";

#[test]
fn varstocases_makes_a_case_per_variable_of_the_from_list() {
    let mut e = engine(REPEATED);
    run(
        &mut e,
        "VARSTOCASES /MAKE newvar FROM var1 TO var4 /INDEX=ivar.",
    );
    assert_eq!(names(&e), "caseid ivar newvar");
    assert_eq!(show(&e, "caseid"), "1 1 1 1 2 2 2 2 3 3 3 3");
    assert_eq!(show(&e, "ivar"), "1 2 3 4 1 2 3 4 1 2 3 4");
    assert_eq!(show(&e, "newvar"), "0 0.05 5 3 7 1 5 4 6 3 6 2");
    assert_eq!(data(&e).variable("newvar").unwrap().format(), "F6.2");
    assert_eq!(data(&e).variable("ivar").unwrap().format(), "F8.0");

    // two factors, the last varying fastest; the abbreviated command name
    let mut e = engine(REPEATED);
    run(
        &mut e,
        "VARSTO /MAKE newvar FROM var1 TO var4 /INDEX=a(2) b 'Factor B' (2).",
    );
    assert_eq!(names(&e), "caseid a b newvar");
    assert_eq!(show(&e, "a"), "1 1 2 2 1 1 2 2 1 1 2 2");
    assert_eq!(show(&e, "b"), "1 2 1 2 1 2 1 2 1 2 1 2");
    assert_eq!(data(&e).variable("b").unwrap().label(), Some("Factor B"));

    // the names of the FROM variables; a variable listed twice repeats its values
    let mut e = engine(REPEATED);
    run(
        &mut e,
        "VARSTOCASES /MAKE v FROM var1 var2 var2 /INDEX=which (v) /DROP=caseid var3 var4.",
    );
    assert_eq!(names(&e), "which v");
    assert_eq!(
        show(&e, "which"),
        "var1|var2|var2|var1|var2|var2|var1|var2|var2"
    );
    assert_eq!(show(&e, "v"), "0 0.05 0.05 7 1 1 6 3 3");
    assert_eq!(data(&e).variable("which").unwrap().string_width(), Some(4));
}

const NULLS: &str = "DATA LIST LIST /id (F3) x1 (F5.1) x2 (F5.1) s1 (A4) s2 (A6) w (F2) f (F1).
BEGIN DATA
1 10 20 a bb 1 0
2 . . '' '' 2 1
3 14 . c '' 1 1
END DATA.
VARIABLE LABELS x1 'First x' x2 'Second x'.
VALUE LABELS x1 10 'ten'.
MISSING VALUES x2 (20).
WEIGHT BY w.
FILTER BY f.
";

#[test]
fn varstocases_drops_null_cases_and_names_new_variables() {
    let mut e = engine(NULLS);
    run(
        &mut e,
        "SPLIT FILE BY id.
VARSTOCASES /MAKE x 'The x' FROM x1 x2 /MAKE s FROM s1 s2 /ID=row 'Row' /COUNT=n /INDEX=i.",
    );
    // ID, the copied variables, COUNT, INDEX, MAKE; FILTER does not leave cases out
    assert_eq!(names(&e), "row id w f n i x s");
    assert_eq!(show(&e, "row"), "1 1 3");
    assert_eq!(show(&e, "n"), "2 2 1");
    assert_eq!(show(&e, "i"), "1 2 1");
    assert_eq!(show(&e, "x"), "10 20 14");
    assert_eq!(show(&e, "s"), "a|bb|c");
    let d = data(&e);
    let x = d.variable("x").unwrap();
    assert_eq!(x.label(), Some("The x"));
    assert_eq!(x.value_labels().len(), 1);
    assert_eq!(*x.missing_values(), MissingValues::None);
    assert_eq!(d.variable("s").unwrap().format(), "A6");
    assert_eq!(d.variable("row").unwrap().format(), "F8.0");
    assert_eq!(d.variable("row").unwrap().label(), Some("Row"));
    assert_eq!(d.variable("n").unwrap().format(), "F4.0");
    assert_eq!(d.variable("n").unwrap().measure(), Measure::Scale);
    assert_eq!(d.weight_variable(), Some("w"));
    assert_eq!(d.filter_variable(), Some("f"));
    assert_eq!(d.split_file().unwrap().variables, ["id"]);

    let mut e = engine(NULLS);
    run(
        &mut e,
        "VARSTOCASES /MAKE x FROM x1 x2 /NULL=KEEP /COUNT=n /KEEP=w id.",
    );
    assert_eq!(names(&e), "w id n x");
    assert_eq!(show(&e, "x"), "10 20 . . 14 .");
    assert_eq!(show(&e, "n"), "2 2 2 2 2 2");

    // a dropped weight variable ends the weighting
    let mut e = engine(NULLS);
    run(&mut e, "VARSTOCASES /MAKE x FROM x1 x2 /DROP=w f.");
    assert_eq!(names(&e), "id s1 s2 x");
    assert_eq!(data(&e).weight_variable(), None);
}

#[test]
fn varstocases_errors() {
    let cases = [
        (
            "VARSTOCASES /MAKE x FROM x1 x2 /MAKE s FROM s1.",
            "every FROM list needs the same number of variables: MAKE x has 2, MAKE s has 1",
        ),
        (
            "VARSTOCASES /MAKE x FROM x1 s1.",
            "the FROM list of MAKE x mixes numeric and string variables (x1 and s1)",
        ),
        (
            "VARSTOCASES /MAKE x FROM x1 x2 /MAKE y FROM x2 x1.",
            "x2 is on the FROM lists of MAKE x and MAKE y; a variable may serve one MAKE",
        ),
        (
            "VARSTOCASES /MAKE x FROM x1 x2 /KEEP=id x1.",
            "x1 is on a FROM list and cannot be named on KEEP or DROP",
        ),
        (
            "VARSTOCASES /MAKE x FROM x1 x2 /INDEX=a(3) b(2).",
            "the INDEX levels (3 x 2) multiply to 6, but the FROM lists have 2 variables",
        ),
        (
            "VARSTOCASES /MAKE x FROM x1 x2 /INDEX=a (y).",
            "INDEX a (y): y is not the name of a MAKE variable",
        ),
        (
            "VARSTOCASES /MAKE id FROM x1 x2.",
            "VARSTOCASES would make two variables named id; choose another name",
        ),
        (
            "TEMPORARY.\nVARSTOCASES /MAKE x FROM x1 x2.",
            "VARSTOCASES cannot follow TEMPORARY: run a procedure or EXECUTE first",
        ),
    ];
    for (source, message) in cases {
        assert_eq!(error(&format!("{NULLS}{source}")), message, "{source}");
    }
}

const BLOOD: &str = "DATA LIST LIST /insure (A8) caseid (F2) month (F2) bps (F4) bpd (F4) age (F3).
BEGIN DATA
BCBS 1 1 160 100 40
BCBS 2 1 120 70 51
BCBS 2 2 130 86 51
Prucare 1 1 160 94 33
Prucare 1 2 200 105 .
Prucare 1 3 180 105 33
Pruecare 2 1 135 90 70
END DATA.
VARIABLE LABELS bps 'Systolic'.
VALUE LABELS month 1 'Jan'.
SPLIT FILE BY insure.
";

#[test]
fn casestovars_spreads_row_groups_over_variables() {
    let mut e = engine(BLOOD);
    run(
        &mut e,
        "CASESTOVARS /ID=caseid /INDEX=month /VIND /COUNT=cnt 'Rows'.",
    );
    // age does not vary within a group once system-missing is left aside: it is copied
    assert_eq!(
        names(&e),
        "insure caseid age bps.1 bps.2 bps.3 bpd.1 bpd.2 bpd.3 ind1 ind2 ind3 cnt"
    );
    assert_eq!(show(&e, "insure"), "BCBS|BCBS|Prucare|Pruecare");
    assert_eq!(show(&e, "caseid"), "1 2 1 2");
    assert_eq!(show(&e, "age"), "40 51 33 70");
    assert_eq!(show(&e, "bps.1"), "160 120 160 135");
    assert_eq!(show(&e, "bps.2"), ". 130 200 .");
    assert_eq!(show(&e, "bps.3"), ". . 180 .");
    assert_eq!(show(&e, "bpd.2"), ". 86 105 .");
    assert_eq!(show(&e, "ind2"), "0 1 1 0");
    assert_eq!(show(&e, "cnt"), "1 2 3 1");
    let d = data(&e);
    assert_eq!(
        d.variable("bps.1").unwrap().label(),
        Some("Systolic, month = Jan")
    );
    assert_eq!(d.variable("bpd.3").unwrap().label(), Some("bpd, month = 3"));
    assert_eq!(d.variable("bps.2").unwrap().format(), "F4.0");
    assert_eq!(d.variable("ind1").unwrap().format(), "F1.0");
    assert_eq!(d.variable("cnt").unwrap().format(), "F4.0");
    assert_eq!(d.split_file().unwrap().variables, ["insure"]);

    let mut e = engine(BLOOD);
    run(
        &mut e,
        "CASESTOVARS /ID=caseid /INDEX=month /GROUPBY=INDEX /SEPARATOR='' /RENAME bps=sys\n\
         /DROP=insure age.",
    );
    assert_eq!(names(&e), "caseid sys1 bpd1 sys2 bpd2 sys3 bpd3");
    assert_eq!(data(&e).split_file(), None);

    // without INDEX the cases are numbered within their group
    let mut e = engine(BLOOD);
    run(&mut e, "CASESTOVARS /ID=caseid /DROP=age month.");
    assert_eq!(
        names(&e),
        "insure caseid bps.1 bps.2 bps.3 bpd.1 bpd.2 bpd.3"
    );
    assert_eq!(show(&e, "bps.2"), ". 130 200 .");

    // without split and ID every case is one group
    let mut e = engine(&format!("{BLOOD}SPLIT FILE OFF."));
    run(&mut e, "CASESTOVARS /DROP=insure caseid month bpd age.");
    assert_eq!(names(&e), "bps.1 bps.2 bps.3 bps.4 bps.5 bps.6 bps.7");
    assert_eq!(show(&e, "bps.7"), "135");
}

#[test]
fn casestovars_names_variables_by_a_string_index() {
    let scores = "DATA LIST LIST /id (F2) scor (F5) time (A4) h (F2).
BEGIN DATA
1 1014 bef 1
1 864 aft 2
2 684 bef 3
2 636 aft 4
3 5 1st 5
END DATA.
";
    let mut e = engine(scores);
    run(&mut e, "CASESTOVARS /ID=id /INDEX=time /DROP=h.");
    // one variable spread: the values are the names, ascending; 1st cannot be a name
    assert_eq!(names(&e), "id V1 aft bef");
    assert_eq!(show(&e, "aft"), "864 636 .");
    assert_eq!(show(&e, "bef"), "1014 684 .");
    assert_eq!(show(&e, "V1"), ". . 5");

    let mut e = engine(scores);
    run(&mut e, "CASESTOVARS /ID=id /INDEX=time /VIND ROOT=has.");
    assert_eq!(
        names(&e),
        "id scor.1st scor.aft scor.bef h.1st h.aft h.bef has1st hasaft hasbef"
    );
    assert_eq!(show(&e, "h.aft"), "2 4 .");
}

#[test]
fn casestovars_fixed_variables_and_warnings() {
    let mut e = engine(BLOOD);
    run(
        &mut e,
        "CASESTOVARS /ID=caseid /INDEX=month /FIXED=age bpd.",
    );
    assert_eq!(
        e.warnings(),
        ["FIXED variable bpd varies within row groups; it becomes a variable group"]
    );
    assert!(names(&e).starts_with("insure caseid age bps.1"));

    let mut e = engine(BLOOD);
    run(
        &mut e,
        "CASESTOVARS /ID=caseid /INDEX=month /AUTOFIX=NO /DROP=bpd.",
    );
    assert_eq!(
        e.warnings(),
        ["age does not vary within row groups; with AUTOFIX=NO it becomes a variable group"]
    );
    assert_eq!(
        names(&e),
        "insure caseid bps.1 bps.2 bps.3 age.1 age.2 age.3"
    );

    let mut e = engine(&format!("{BLOOD}IF (caseid = 2) caseid = $SYSMIS."));
    run(&mut e, "CASESTOVARS /ID=caseid /INDEX=month /DROP=age.");
    assert_eq!(
        e.warnings(),
        ["CASESTOVARS left out 3 cases with a system-missing or blank value of insure, caseid"]
    );
    assert_eq!(show(&e, "insure"), "BCBS|Prucare");
}

#[test]
fn casestovars_errors() {
    let cases = [
        (
            "CASESTOVARS /ID=caseid /INDEX=month /AUTOFIX=NO /FIXED=bpd.",
            "the FIXED variables bpd vary within row groups; with AUTOFIX=NO CASESTOVARS does \
             not run",
        ),
        (
            "SPLIT FILE OFF.\nCASESTOVARS /ID=month /INDEX=caseid.",
            "the cases are not sorted by month: case 4 continues the group of case 1; sort them \
             with SORT CASES BY month first",
        ),
        (
            "CASESTOVARS /INDEX=caseid.",
            "cases 2 and 3 of one row group have the same index value (2); every case of a group \
             needs its own",
        ),
        (
            "COMPUTE month = -month.\nCASESTOVARS /ID=caseid /INDEX=month.",
            "INDEX variable month has the value -1 in case 1; an index needs whole numbers from 0 \
             up or text that is not blank",
        ),
        (
            "CASESTOVARS /ID=caseid /INDEX=caseid.",
            "CASESTOVARS: caseid is on ID and INDEX; an ID variable cannot be an index",
        ),
        (
            "CASESTOVARS /ID=caseid /RENAME caseid=c.",
            "RENAME caseid: only a variable that becomes a variable group can be renamed, not a \
             split, ID, FIXED, INDEX or DROP variable",
        ),
        (
            "TEMPORARY.\nCASESTOVARS /ID=caseid.",
            "CASESTOVARS cannot follow TEMPORARY: run a procedure or EXECUTE first",
        ),
    ];
    for (source, message) in cases {
        assert_eq!(error(&format!("{BLOOD}{source}")), message, "{source}");
    }
}

const SHEET: &str = "DATA LIST LIST /a (A8) b (F5.1) c (F5.1) d (F5.1).
BEGIN DATA
Income 22 31 43
Price 34 29 50
'Year x' 1970 1971 1972
Price 1 2 3
'1abc' 5 6 7
'' 8 9 10
END DATA.
MISSING VALUES c (29).
VARIABLE LABELS b 'Bee'.
WEIGHT BY b.
";

#[test]
fn flip_transposes_and_names_the_new_variables() {
    let mut e = engine(SHEET);
    let tables = run(&mut e, "FLIP NEWNAMES=a.");
    assert_eq!(
        names(&e),
        "CASE_LBL INCOME PRICE YEAR_X PRICE_A K_1ABC VAR006"
    );
    assert_eq!(tables[0].rows.len(), 7);
    assert_eq!(show(&e, "CASE_LBL"), "b|c|d");
    // user-missing values become system-missing
    assert_eq!(show(&e, "PRICE"), "34 . 50");
    assert_eq!(show(&e, "VAR006"), "8 9 10");
    let d = data(&e);
    assert_eq!(d.variable("CASE_LBL").unwrap().format(), "A8");
    assert_eq!(d.variable("INCOME").unwrap().format(), "F8.2");
    assert_eq!(d.variable("INCOME").unwrap().label(), None);
    assert_eq!(d.weight_variable(), None);

    // back again: the values of CASE_LBL name the variables
    run(&mut e, "FLIP.");
    assert_eq!(names(&e), "CASE_LBL B C D");
    assert_eq!(
        show(&e, "CASE_LBL"),
        "INCOME|PRICE|YEAR_X|PRICE_A|K_1ABC|VAR006"
    );
    assert_eq!(show(&e, "C"), "31 . 1971 2 6 9");

    // numbers get K_; strings become system-missing; default names count from VAR001
    let mut e = engine(SHEET);
    run(&mut e, "FLIP VARIABLES=b c /NEWNAMES=d.");
    assert_eq!(names(&e), "CASE_LBL K_43 K_50 K_1972 K_3 K_7 K_10");
    let mut e = engine(SHEET);
    run(&mut e, "FLIP VARIABLES=a b.");
    assert_eq!(
        names(&e),
        "CASE_LBL VAR001 VAR002 VAR003 VAR004 VAR005 VAR006"
    );
    assert_eq!(show(&e, "VAR001"), ". 22");
}

#[test]
fn flip_makes_temporary_changes_permanent() {
    let mut e = engine(SHEET);
    run(
        &mut e,
        "TEMPORARY.\nSELECT IF b < 30.\nFLIP VARIABLES=b.\nEXECUTE.",
    );
    assert_eq!(names(&e), "CASE_LBL VAR001 VAR002 VAR003 VAR004");
    assert_eq!(show(&e, "VAR004"), "8");
}

const FILES: &str = "DATA LIST LIST /k (F3) x (F4) s (A3) w (F3).
BEGIN DATA
1 10 a 1
2 20 b 2
3 30 c 1
5 50 e 2
END DATA.
VARIABLE LABELS x 'Ex'.
DATASET NAME primary.
DATA LIST LIST /k (F3) x (F4) y (F4) s (A6).
BEGIN DATA
2 . 5 ''
2 22 6 bb
4 40 7 dddd
5 . . ''
END DATA.
VARIABLE LABELS y 'Why'.
DATASET NAME trans1.
DATA LIST LIST /k (F3) y (F4) z (A2).
BEGIN DATA
0 1 zz
2 8 ''
5 9 q
END DATA.
DATASET NAME trans2.
";

#[test]
fn update_takes_the_last_value_that_is_not_missing() {
    let mut e = engine(FILES);
    run(
        &mut e,
        "UPDATE FILE=primary /FILE=trans1 /FILE=trans2 /BY k.",
    );
    assert_eq!(names(&e), "k x s w y z");
    assert_eq!(show(&e, "k"), "0 1 2 3 4 5");
    assert_eq!(show(&e, "x"), ". 10 22 30 40 50");
    assert_eq!(show(&e, "s"), "|a|bb|c|dddd|e");
    assert_eq!(show(&e, "w"), ". 1 2 1 . 2");
    assert_eq!(show(&e, "y"), "1 . 8 . 7 9");
    assert_eq!(show(&e, "z"), "zz||||| q".replace(' ', ""));
    let d = data(&e);
    assert_eq!(d.variable("s").unwrap().string_width(), Some(6));
    assert_eq!(d.variable("x").unwrap().label(), Some("Ex"));
    assert_eq!(d.variable("y").unwrap().label(), Some("Why"));

    // a user-missing value in a transaction is not used either
    let mut e = engine(&format!(
        "{FILES}DATASET ACTIVATE trans1.\nMISSING VALUES x (22).\n"
    ));
    run(
        &mut e,
        "UPDATE FILE=primary /IN=old /FILE=* /IN=new /BY k /MAP /DROP=y.",
    );
    assert_eq!(names(&e), "k x s w old new");
    assert_eq!(show(&e, "x"), "10 20 30 40 50");
    assert_eq!(show(&e, "old"), "1 1 1 0 1");
    assert_eq!(show(&e, "new"), "0 1 0 1 1");
    assert_eq!(data(&e).variable("new").unwrap().format(), "F1.0");
}

#[test]
fn update_map_keeps_the_weight_and_renames() {
    let mut e = engine(&format!("{FILES}DATASET ACTIVATE primary.\nWEIGHT BY w.\n"));
    let tables = run(
        &mut e,
        "UPDATE FILE=* /FILE=trans1 /RENAME=(x y = x2 x) /BY k /KEEP=k x x2 /MAP.",
    );
    assert_eq!(data(&e).weight_variable(), None, "w was not kept");
    let map = &tables[0];
    assert_eq!(map.title, "Variable Map");
    assert_eq!(map.columns, ["Result", "FILE=*", "FILE='trans1'"]);
    let rows: Vec<String> = map.rows.iter().map(|row| format!("{row:?}")).collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows[1].contains("Text(\"x\"), Text(\"x\"), Text(\"y\")"),
        "{}",
        rows[1]
    );
    assert_eq!(show(&e, "x"), "10 6 30 7 50");
    assert_eq!(show(&e, "x2"), ". 22 . 40 .");

    let mut e = engine(&format!("{FILES}DATASET ACTIVATE primary.\nWEIGHT BY w.\n"));
    run(&mut e, "UPDATE FILE=* /FILE=trans2 /BY k.");
    assert_eq!(data(&e).weight_variable(), Some("w"));
}

#[test]
fn update_errors() {
    let cases = [
        (
            "UPDATE FILE=trans1 /FILE=primary /BY k.",
            "the primary file FILE='trans1' holds the same key twice (cases 1 and 2); UPDATE \
             needs each key once in it",
        ),
        (
            "UPDATE FILE=primary /FILE=trans1 /IN=t /BY k /DROP=t.",
            "t is an /IN variable: it goes at the end of the result and cannot be named on KEEP \
             or DROP",
        ),
        (
            "UPDATE FILE=primary /FILE=trans1 /IN=y /BY k.",
            "the /IN variable y has the name of a variable of the result",
        ),
        (
            "TEMPORARY.\nUPDATE FILE=primary /FILE=* /BY k.",
            "UPDATE with FILE=* cannot follow TEMPORARY: run a procedure or EXECUTE first",
        ),
        (
            "SORT CASES BY k (D).\nUPDATE FILE=primary /FILE=* /BY k.",
            "FILE=* is not sorted by k: case 2 has a lower key than case 1; sort the active \
             dataset with SORT CASES BY k first",
        ),
    ];
    for (source, message) in cases {
        assert_eq!(error(&format!("{FILES}{source}")), message, "{source}");
    }
}

#[test]
fn sort_variables_by_each_property() {
    let base = "DATA LIST LIST /v10 (F3) V2 (DOLLAR8.2) b (A5) a1 (A3) zz (DATE11) v2a (F8.2)
  c (COMMA9.1) ab (A3) v1 (F2).
BEGIN DATA
1 2 x y 1-JAN-2000 3 4 z 5
END DATA.
VARIABLE LABELS b 'beta' a1 'Alpha' zz 'alpha' c 'Gamma'.
VALUE LABELS v1 1 'x' / c 1 'x' / b 'x' 'q' / v10 1 'a'.
MISSING VALUES v10 (9) c (1 THRU 3) b ('x') V2 (9).
VARIABLE LEVEL v10 (ORDINAL) / V2 (SCALE) / b a1 ab v1 v2a (NOMINAL) / c (ORDINAL).
VARIABLE ROLE /TARGET v10 /BOTH b /NONE c /PARTITION zz /SPLIT a1.
VARIABLE ALIGNMENT v10 (CENTER) / b (RIGHT) / c (LEFT).
VARIABLE WIDTH v10 (3) b (12) c (1).
VARIABLE ATTRIBUTE VARIABLES=b ATTRIBUTE=k('m') /VARIABLES=c ATTRIBUTE=k[1]('a') k[2]('z')
  /VARIABLES=v1 ATTRIBUTE=k[1]('A') k[2]('b').
";
    let cases = [
        ("NAME", "a1 ab b c v1 V2 v10 v2a zz"),
        ("NAME (D)", "zz v2a v10 V2 v1 c b ab a1"),
        ("TYPE", "v10 V2 zz v2a c v1 a1 ab b"),
        ("TYPE (DOWN)", "b ab a1 v1 c v2a zz V2 v10"),
        ("FORMAT", "a1 ab b c V2 v1 v10 v2a zz"),
        ("LABEL", "v10 V2 v2a ab v1 a1 zz b c"),
        // equal sets of value labels sit together
        ("VALUES", "V2 a1 zz v2a ab v10 c v1 b"),
        ("MISSING", "a1 zz v2a ab v1 v10 V2 c b"),
        ("MEASURE", "b a1 v2a ab v1 v10 c V2 zz"),
        ("ROLE", "V2 v2a ab v1 v10 b c zz a1"),
        ("COLUMNS", "c v10 a1 ab V2 zz v2a v1 b"),
        ("ALIGNMENT", "a1 c ab V2 b zz v2a v1 v10"),
        ("ATTRIBUTE k", "v10 V2 a1 zz v2a ab v1 c b"),
        ("ATTRIBUTE K (D)", "b c v1 ab v2a zz a1 V2 v10"),
    ];
    for (key, expected) in cases {
        let mut e = engine(base);
        run(&mut e, &format!("SORT VARIABLES BY {key}."));
        assert_eq!(names(&e), expected, "{key}");
    }
}
