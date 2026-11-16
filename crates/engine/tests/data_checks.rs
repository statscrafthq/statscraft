//! COMPARE DATASETS and VALIDATEDATA on small invented files: gym members visiting, pupils
//! with test scores, households with ages. Every expected value is counted by hand.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let mut session = Session::default();
    let result = session
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
    let texts: Vec<String> = errors.iter().map(|e| e.text()).collect();
    texts.join(" | ")
}

fn cell(c: &Cell) -> String {
    match c {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{}", (n * 1e6).round() / 1e6),
        Cell::Empty => String::new(),
    }
}

fn expect(table: &Table, expected: &[&[&str]]) {
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|r| r.iter().map(cell).collect())
        .collect();
    let expected: Vec<Vec<String>> = expected
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();
    assert_eq!(rows, expected, "{}", table.title);
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The values of one column of the last LIST.
fn listed(tables: &[Table], column: &str) -> Vec<String> {
    let list = tables
        .iter()
        .rev()
        .find(|t| t.title == "List")
        .expect("a LIST");
    let at = list.columns.iter().position(|c| c == column).expect(column);
    list.rows
        .iter()
        .map(|r| cell(&r[at]).trim().to_string())
        .collect()
}

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-checks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Last month's visits as a named dataset; this month's file, one member more, is active.
const GYM: &str = "DATA LIST FREE /member (F3) visits (F3) plan (A6) fee (F6.2).
BEGIN DATA
101 4 basic 10
102 0 gold 25.5
103 7 basic 10
104 2 silver .
105 9 gold 25.5
END DATA.
DATASET NAME earlier.
DATA LIST FREE /member (F3) visits (F3) plan (A8) fee (F6.2).
BEGIN DATA
101 4 basic 10
102 1 gold 25.5
103 7 basic 10
104 2 silver 15
105 9 gold 25.5
106 3 basic 10
END DATA.
DATASET NAME now.
";

#[test]
fn cases_in_file_order_flag_the_changed_and_the_new() {
    let (tables, _) = run(&format!(
        "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES ALL.\nLIST."
    ));
    expect(
        titled(&tables, "Dataset Comparison"),
        &[
            &["Other dataset", "earlier"],
            &["Variables compared", "4"],
            &["Variables with differences", "2"],
            &["Cases compared", "5"],
            &["Cases with differences", "2"],
            &["Cases only in the active dataset", "1"],
            &["Cases only in the other dataset", "0"],
        ],
    );
    // plan is A6 in one file and A8 in the other: the padding does not count
    expect(
        titled(&tables, "Differences by Variable"),
        &[&["visits", "1"], &["fee", "1"]],
    );
    expect(
        titled(&tables, "Case Comparison"),
        &[
            &["2", "visits", "1", "0"],
            &["4", "fee", "15", "."],
            &["6", "(no matching case)", "", ""],
        ],
    );
    assert!(!tables.iter().any(|t| t.title == "Variables Not Compared"));
    assert_eq!(
        listed(&tables, "CasesCompare"),
        ["0", "1", "0", "1", "0", "-1"]
    );
}

#[test]
fn case_ids_pair_cases_and_the_matches_go_to_their_own_files() {
    let dir = folder("caseid");
    let old = dir.join("old.sav").display().to_string();
    let changed = dir.join("changed.sav").display().to_string();
    let source = format!(
        "DATA LIST FREE /member (F3) visits (F3) plan (F1).
BEGIN DATA
101 4 1
102 0 2
103 7 1
104 2 3
END DATA.
VARIABLE LABELS visits 'Visits in the month'.
VARIABLE LEVEL visits (SCALE) /plan (NOMINAL).
SAVE OUTFILE='{old}'.
DATA LIST FREE /member (F3) visits (F3) plan (A6) fee (F6.2).
BEGIN DATA
101 4 basic 10
102 1 gold 25.5
102 5 gold 25.5
104 2 silver 15
107 1 basic 10
END DATA.
VARIABLE LEVEL visits (ORDINAL) /plan (NOMINAL).
DATASET NAME now.
DATASET DECLARE same.
COMPARE DATASETS /COMPDATASET '{old}' /VARIABLES visits plan fee /CASEID member
  /SAVE MATCHDATASET=YES MATCHNAME=same MISMATCHDATASET=YES MISMATCHNAME='{changed}'
  /OUTPUT VARPROPERTIES=LABEL MEASURE CASETABLE=YES TABLELIMIT=2.
LIST.
DATASET ACTIVATE same.
LIST.
GET FILE='{changed}'.
LIST.
"
    );
    let (tables, _) = run(&source);
    expect(
        titled(&tables, "Dataset Comparison"),
        &[
            &["Other dataset", &format!("'{old}'")],
            &["Variables compared", "1"],
            &["Variables with differences", "1"],
            &["Cases compared", "3"],
            &["Cases with differences", "1"],
            &["Cases only in the active dataset", "2"],
            &["Cases only in the other dataset", "1"],
        ],
    );
    expect(
        titled(&tables, "Variables Not Compared"),
        &[
            &["plan", "String in one dataset, numeric in the other"],
            &["fee", "Not in the other dataset"],
        ],
    );
    let cases = titled(&tables, "Case Comparison");
    expect(
        cases,
        &[
            &["2", "102", "visits", "1", "0"],
            &["3", "102", "(no matching case)", "", ""],
        ],
    );
    assert_eq!(
        cases.caption.as_deref(),
        Some("The first 2 of 3 cases that differ are listed.")
    );
    expect(
        titled(&tables, "Dictionary Differences"),
        &[
            &["visits", "Measurement level", "Ordinal", "Scale"],
            &["visits", "Variable label", "", "Visits in the month"],
        ],
    );
    let lists: Vec<&Table> = tables.iter().filter(|t| t.title == "List").collect();
    assert_eq!(
        listed(&tables[..=index(&tables, lists[0])], "CasesCompare"),
        ["0", "1", "-1", "0", "-1"]
    );
    assert_eq!(
        listed(&tables[..=index(&tables, lists[1])], "member"),
        ["101", "104"]
    );
    assert_eq!(listed(&tables, "member"), ["102", "102", "107"]);
    let _ = std::fs::remove_dir_all(dir);
}

fn index(tables: &[Table], table: &Table) -> usize {
    tables.iter().position(|t| std::ptr::eq(t, table)).unwrap()
}

#[test]
fn compare_refuses_what_it_cannot_do() {
    let unsorted = "DATA LIST FREE /member visits.
BEGIN DATA
102 1
101 2
END DATA.
DATASET NAME a.
DATASET COPY b.
";
    assert_eq!(
        fails(&format!(
            "{unsorted}COMPARE DATASETS /COMPDATASET b /VARIABLES visits /CASEID member."
        )),
        "line 8: COMPARE DATASETS: the active dataset is not sorted by member: case 2 has a lower key than case 1; \
         sort both files with SORT CASES BY member first"
    );
    assert_eq!(
        fails(&format!(
            "{GYM}COMPARE DATASETS /COMPDATASET later /VARIABLES ALL."
        )),
        "line 20: COMPARE DATASETS: there is no open dataset later; quote the name of a .sav file"
    );
    assert!(
        fails(&format!(
            "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES ALL /SAVE MATCHDATASET=YES MATCHNAME=kept."
        ))
        .contains("there is no dataset kept; declare it first")
    );
    assert!(
        fails(&format!(
            "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES ALL\n /SAVE MISMATCHDATASET=YES MISMATCHNAME='x.sav' MISMATCHPASS='secret'."
        ))
        .contains("cannot write encrypted data files")
    );
    assert!(
        fails(&format!(
            "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES ALL /SAVE VARNAME=plan."
        ))
        .contains("plan is a string variable")
    );
    // keywords are written in full; both required subcommands must be there
    assert!(
        fails(&format!(
            "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARS ALL."
        ))
        .contains("unknown subcommand VARS")
    );
    assert!(
        fails(&format!("{GYM}COMPARE DATASETS /COMPDATASET earlier.")).contains("needs /VARIABLES")
    );
}

#[test]
fn compare_can_leave_out_the_flag_and_the_case_table() {
    let (tables, _) = run(&format!(
        "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES visits fee visits
  /SAVE FLAGMISMATCHES=NO /OUTPUT CASETABLE=NO VARPROPERTIES=ALL.\nLIST."
    ));
    assert!(!tables.iter().any(|t| t.title == "Case Comparison"));
    // a variable named twice is compared once
    let summary = titled(&tables, "Dataset Comparison");
    assert_eq!(cell(&summary.rows[1][1]), "2");
    let list = titled(&tables, "List");
    assert!(!list.columns.iter().any(|c| c == "CasesCompare"));
    let properties = titled(&tables, "Dictionary Differences");
    assert!(properties.rows.is_empty());
    assert_eq!(
        properties.caption.as_deref(),
        Some("The requested properties are the same in both datasets.")
    );
    // a second run writes over the flag of the first
    let (tables, _) = run(&format!(
        "{GYM}COMPARE DATASETS /COMPDATASET earlier /VARIABLES ALL /SAVE VARNAME=diff.
COMPARE DATASETS /COMPDATASET earlier /VARIABLES member plan /SAVE VARNAME=diff.\nLIST."
    ));
    assert_eq!(listed(&tables, "diff"), ["0", "0", "0", "0", "0", "-1"]);
}

/// Ten respondents: wave is constant, code unique, income nearly constant, rating mostly
/// missing.
const SURVEY: &str = "DATA LIST FREE /resp wave region code income rating.
BEGIN DATA
1 1 1 501 1000 3
2 1 1 502 1000 .
3 1 1 503 1000 .
4 1 1 504 1000 .
5 1 2 505 1000 3
6 1 2 506 1000 .
7 1 2 507 1000 .
8 1 3 508 1000 .
9 1 3 509 1000 .
10 1 3 510 1001 .
END DATA.
VARIABLE LEVEL wave region code (NOMINAL) /rating (ORDINAL) /income (SCALE).
";

#[test]
fn variable_checks_find_missing_constant_and_unique_variables() {
    let (tables, _) = run(&format!(
        "{SURVEY}VALIDATEDATA VARIABLES=wave region code income rating."
    ));
    // income: mean 1000.1, standard deviation sqrt(0.9 / 9) = sqrt(0.1)
    let cv = format!("{}", ((0.1f64.sqrt() / 1000.1) * 1e6).round() / 1e6);
    expect(
        titled(&tables, "Variable Checks"),
        &[
            &[
                "Largest category above 95% of the valid values",
                "wave",
                "100",
            ],
            &["Categories with one case above 90%", "code", "100"],
            &["Coefficient of variation below 0.001", "income", &cv],
            &["Missing values above 70%", "rating", "80"],
            &[
                "Largest category above 95% of the valid values",
                "rating",
                "100",
            ],
        ],
    );
    let empty = titled(&tables, "Empty Cases");
    assert!(empty.rows.is_empty());
    assert_eq!(empty.caption.as_deref(), Some("No case is empty."));

    let (tables, _) = run(&format!(
        "{SURVEY}VALIDATEDATA VARIABLES=income rating
  /VARCHECKS PCTMISSING=85 PCTEQUAL=100 CV=0 STDDEV=0.5 /CASECHECKS REPORTEMPTY=NO."
    ));
    let sd = format!("{}", (0.1f64.sqrt() * 1e6).round() / 1e6);
    expect(
        titled(&tables, "Variable Checks"),
        &[&["Standard deviation below 0.5", "income", &sd]],
    );
    assert!(!tables.iter().any(|t| t.title == "Empty Cases"));
}

/// Pupils by school: two incomplete IDs (a blank school, a missing pupil), three pairs
/// sharing an ID, two cases without scores.
const PUPILS: &str = "DATA LIST FREE /school (A4) pupil math read.
BEGIN DATA
'N1' 1 50 60
'N1' 2 . .
'N1' 2 70 65
'  ' 3 40 45
'S2' . 55 50
'S2' 4 . .
'S2' 4 80 .
'N1' 1 . 62
END DATA.
COMPUTE EmptyCase = 0.
";

#[test]
fn ids_and_empty_cases_are_listed_and_saved() {
    let (tables, _) = run(&format!(
        "{PUPILS}VALIDATEDATA VARIABLES=math read ID=school pupil /VARCHECKS STATUS=OFF
  /SAVE EMPTYCASE INCOMPLETEID DUPLICATEID(twin).\nLIST."
    ));
    expect(
        titled(&tables, "Incomplete Identifiers"),
        &[&["4", "", "3"], &["5", "S2", "."]],
    );
    expect(
        titled(&tables, "Duplicate Identifiers"),
        &[
            &["1", "1", "N1", "1"],
            &["1", "8", "N1", "1"],
            &["2", "2", "N1", "2"],
            &["2", "3", "N1", "2"],
            &["3", "6", "S2", "4"],
            &["3", "7", "S2", "4"],
        ],
    );
    expect(
        titled(&tables, "Empty Cases"),
        &[&["2", "N1", "2"], &["6", "S2", "4"]],
    );
    assert!(!tables.iter().any(|t| t.title == "Variable Checks"));
    // EmptyCase is taken, so the default name gets a suffix
    assert_eq!(
        listed(&tables, "EmptyCase_1"),
        ["0", "1", "0", "0", "0", "1", "0", "0"]
    );
    assert_eq!(
        listed(&tables, "IncompleteID"),
        ["0", "0", "0", "1", "1", "0", "0", "0"]
    );
    assert_eq!(
        listed(&tables, "twin"),
        ["1", "2", "2", "0", "0", "3", "3", "1"]
    );

    // without analysis variables every variable but the IDs decides
    let (tables, _) = run(&format!(
        "{PUPILS}DELETE VARIABLES EmptyCase.\nVALIDATEDATA ID=school pupil /IDCHECKS NONE."
    ));
    expect(
        titled(&tables, "Empty Cases"),
        &[&["2", "N1", "2"], &["6", "S2", "4"]],
    );
    assert_eq!(
        fails(&format!(
            "{PUPILS}VALIDATEDATA ID=school /IDCHECKS NONE /CASECHECKS REPORTEMPTY=NO."
        )),
        "line 13: VALIDATEDATA: nothing to report or save with these settings"
    );
    assert_eq!(
        fails(&format!(
            "{PUPILS}VALIDATEDATA VARIABLES=math /SAVE EMPTYCASE(read)."
        )),
        "line 13: VALIDATEDATA: /SAVE names read, a variable that already exists; choose another name"
    );
}

#[test]
fn split_groups_number_their_own_duplicates() {
    let (tables, warnings) = run("DATA LIST FREE /class id score.
BEGIN DATA
1 1 5
1 1 6
1 2 7
2 2 8
2 2 9
2 3 .
END DATA.
VARIABLE LEVEL class id (NOMINAL) /score (SCALE).
COMPUTE w = 1.
WEIGHT BY w.
SPLIT FILE LAYERED BY class.
VALIDATEDATA VARIABLES=ALL ID=id /IDCHECKS NONE /SAVE DUPLICATEID.
SPLIT FILE OFF.
LIST.");
    // id 2 is in both classes, yet each class numbers only its own duplicates
    // class is constant within each group but, as a split variable, never checked
    let checks = titled(&tables, "Variable Checks");
    assert!(
        checks
            .rows
            .iter()
            .all(|r| !r.iter().any(|c| cell(c) == "class"))
    );
    assert_eq!(
        listed(&tables, "DuplicateIDGroup"),
        ["1", "1", "0", "1", "1", "0"]
    );
    assert!(warnings.iter().any(|w| w.contains("the weight is ignored")));
}

/// Households: two single-variable rules and one cross-variable rule, with outcome variables
/// made by COMPUTE; one broken rule and two broken links.
const HOMES: &str = "DATA LIST FREE /hid age kids (F2) status (A6).
BEGIN DATA
1 34 2 'single'
2 15 0 'single'
3 70 1 'widow'
4 12 3 'single'
5 40 . 'xx'
END DATA.
DATAFILE ATTRIBUTE ATTRIBUTE=$VD.SRule[1](\"Label='Grown up',Type='Numeric',Minimum='18'\")
  $VD.SRule[2](\"Label='Known status',Type='String'\")
  $VD.SRule[3](\"Type='Numeric'\")
  $VD.CRule[1](\"Label='Young parent',Expression='age < 16 and kids > 0',OutcomeVar='youngpar'\")
  $VD.CRule[2](\"Label='grown up ',Expression='age > 0',OutcomeVar='kids'\")
  $VD.CRule[4](\"Label='Under 18',Expression='age < 18',OutcomeVar='minor'\").
COMPUTE minor = age < 18.
COMPUTE oddstat = NOT ANY(status, 'single', 'widow', 'married').
COMPUTE youngpar = age < 16 and kids > 0.
VARIABLE ATTRIBUTE VARIABLES=minor oddstat youngpar ATTRIBUTE=$VD.RuleOutcomeVar('Yes')
  /VARIABLES=age ATTRIBUTE=$VD.SRuleRef[1](\"Rule='$VD.SRule[1]',OutcomeVar='minor'\")
     $VD.SRuleRef[2](\"Rule='$VD.SRule[3]',OutcomeVar='minor'\")
  /VARIABLES=status ATTRIBUTE=$VD.SRuleRef[1](\"Rule='$VD.SRule[2]',OutcomeVar='oddstat'\")
     $VD.SRuleRef[2](\"Rule='$VD.SRule[1]',OutcomeVar='oddstat'\").
";

#[test]
fn rule_violations_are_summarized_reported_and_counted() {
    let (tables, warnings) = run(&format!(
        "{HOMES}VALIDATEDATA VARIABLES=ALL ID=hid CROSSVARRULES=$VD.CRule[1] 2 3
  /VARCHECKS STATUS=OFF /CASECHECKS REPORTEMPTY=NO /RULESUMMARIES BYRULE BYVARIABLE
  /CASEREPORT CASELIMIT=FIRSTN(2) /SAVE RULEVIOLATIONS.\nLIST."
    ));
    assert_eq!(
        warnings,
        [
            "skips the link $VD.SRuleRef[2] of age: the rule $VD.SRule[3] is skipped: it has no Label",
            "skips the link $VD.SRuleRef[2] of status: a Numeric rule does not fit this variable's type",
            "skips $VD.CRule[2]: another rule has the label grown up",
            "skips $VD.CRule[3]: the data file has no such rule",
        ]
    );
    expect(
        titled(&tables, "Single-Variable Rule Violations by Variable"),
        &[
            &["age", "Grown up", "2"],
            &["age", "Total", "2"],
            &["status", "Known status", "1"],
            &["status", "Total", "1"],
        ],
    );
    expect(
        titled(&tables, "Single-Variable Rule Violations by Rule"),
        &[
            &["Grown up", "age", "2"],
            &["Grown up", "Total", "2"],
            &["Known status", "status", "1"],
            &["Known status", "Total", "1"],
        ],
    );
    expect(
        titled(&tables, "Cross-Variable Rule Violations"),
        &[&["Young parent", "1", "age < 16 and kids > 0"]],
    );
    let report = titled(&tables, "Case Report");
    expect(
        report,
        &[
            &["2", "2", "1", "Grown up (age)"],
            &["4", "4", "2", "Grown up (age); Young parent"],
        ],
    );
    assert_eq!(
        report.caption.as_deref(),
        Some("The first 2 of 3 cases are listed.")
    );
    assert_eq!(
        listed(&tables, "ValidationRuleViolations"),
        ["0", "1", "0", "2", "1"]
    );

    // MINVIOLATIONS, and the summaries left out
    let (tables, _) = run(&format!(
        "{HOMES}VALIDATEDATA VARIABLES=age CROSSVARRULES=1 /RULESUMMARIES NONE
  /CASEREPORT MINVIOLATIONS=2 /VARCHECKS STATUS=OFF /CASECHECKS REPORTEMPTY=NO."
    ));
    assert!(
        !tables
            .iter()
            .any(|t| t.title.starts_with("Single-Variable"))
    );
    expect(
        titled(&tables, "Case Report"),
        &[&["4", "2", "Grown up (age); Young parent"]],
    );
}

#[test]
fn one_outcome_variable_for_two_rules_is_an_error() {
    assert_eq!(
        fails(&format!(
            "{HOMES}VALIDATEDATA VARIABLES=age CROSSVARRULES=4."
        )),
        "line 23: VALIDATEDATA: the outcome variable minor belongs to more than one rule; give \
         every rule its own"
    );
}

#[test]
fn validatedata_syntax_is_checked_before_running() {
    for (source, message) in [
        (
            "VALIDATEDATA.",
            "VALIDATEDATA needs VARIABLES=, ID= or CROSSVARRULES=",
        ),
        ("VALIDATEDATA VAR=math.", "unexpected word VAR"),
        (
            "VALIDATEDATA VARIABLES=math /VARCHECKS.",
            "/VARCHECKS is empty",
        ),
        (
            "VALIDATEDATA ID=pupil /IDCHECKS NONE DUPLICATE.",
            "IDCHECKS NONE stands alone",
        ),
        (
            "VALIDATEDATA ID=pupil /RULESUMMARIES BYRULE NONE.",
            "RULESUMMARIES NONE stands alone",
        ),
        (
            "VALIDATEDATA ID=pupil /SAVE EMPTYCASE EMPTYCASE.",
            "/SAVE allows EMPTYCASE once",
        ),
        (
            "VALIDATEDATA ID=pupil /VARCHECKS PCTMISSING=0.",
            "PCTMISSING must be above 0",
        ),
        (
            "VALIDATEDATA ID=pupil /VARCHECKS CV=-1.",
            "CV must be 0 or more",
        ),
        (
            "VALIDATEDATA ID=pupil /CASEREPORT CASELIMIT=FIRSTN(0).",
            "must be a positive whole",
        ),
        (
            "VALIDATEDATA ID=pupil /IDCHECKS DUP.",
            "unexpected word DUP",
        ),
        (
            "VALIDATEDATA ID=pupil /CASECHECKS SCOPE=ALL.",
            "ANALYSISVARS",
        ),
        (
            "VALIDATEDATA ID=pupil /SAVE EMPTYCASE /SAVE INCOMPLETEID.",
            "allows SAVE once",
        ),
    ] {
        let error = fails(&format!("{PUPILS}{source}"));
        assert!(error.contains(message), "{source}: {error}");
    }
}
