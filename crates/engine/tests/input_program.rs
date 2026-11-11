//! INPUT PROGRAM, END CASE, END FILE, LEAVE, REREAD, REPEATING DATA and FILE TYPE: the examples
//! of the manual chapters of these commands, with their data inline or in made-up files, and
//! the results the manual gives.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn ran(source: &str) -> (Session, RunResult) {
    let mut session = Session::default();
    let result = run(&mut session, source);
    (session, result)
}

fn fails(source: &str) -> String {
    match Session::default().run(source, None) {
        Ok(_) => panic!("{source} ran"),
        Err(errors) => errors[0].message.clone(),
    }
}

fn tables(result: &RunResult) -> Vec<&Table> {
    result.commands.iter().flat_map(|c| &c.tables).collect()
}

fn warnings(result: &RunResult) -> Vec<String> {
    result
        .commands
        .iter()
        .flat_map(|c| c.warnings.clone())
        .collect()
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The lines of text output, trailing blanks cut.
fn printed(result: &RunResult) -> Vec<String> {
    tables(result)
        .into_iter()
        .filter(|t| t.title == "Text output")
        .flat_map(|t| t.rows.iter().map(|r| text(&r[0]).trim_end().to_string()))
        .collect()
}

fn names(data: &Dataset) -> Vec<String> {
    data.variable_names().map(str::to_string).collect()
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// Numbers without missing values.
fn whole(data: &Dataset, name: &str) -> Vec<f64> {
    numbers(data, name)
        .into_iter()
        .map(|v| v.unwrap_or_else(|| panic!("{name} has a missing value")))
        .collect()
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(|s| s.trim_end().to_string()).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn folder(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-input-{}-{case}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn end_case_makes_a_case_of_each_data_item() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["X"]);
    assert_eq!(whole(data, "X"), [2.0, 1.0, 1.0, 3.0, 5.0, 1.0]);
    assert_eq!(printed(&result), ["2", "1", "1", "3", "5", "1"]);
}

#[test]
fn leave_spreads_the_order_number_to_each_book() {
    [removed]
         PRINT /' ' ORDER ' ' ISBN '   ' QUANTITY.\n\
         EXECUTE.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["ORDER", "ISBN", "QUANTITY"]);
    let rows: Vec<(f64, f64, f64)> = (0..data.case_count())
        .map(|i| {
            let get = |n: &str| whole(data, n)[i];
            (get("ORDER"), get("ISBN"), get("QUANTITY"))
        })
        .collect();
    // the figure of the manual
    let expected = [
        [removed]
    ];
    let expected: Vec<(f64, f64, f64)> = expected
        .iter()
        .map(|&(a, b, c)| (f64::from(a), f64::from(b), f64::from(c)))
        .collect();
    assert_eq!(rows, expected);
    let lines = printed(&result);
    assert_eq!(lines[0], "Order ISBN Quantity");
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], " 1049   74     1");
    assert_eq!(lines.len(), 19);
}

#[test]
fn an_input_program_without_data_makes_cases_from_a_loop() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 1000);
    let index = whole(data, "I");
    assert_eq!(index, (1..=1000).map(f64::from).collect::<Vec<_>>());
    let scores = whole(data, "SCORE");
    assert!(scores.iter().all(|s| *s > 0.0));
    // a log-normal variable has the median near exp(0) = 1
    let below = scores.iter().filter(|s| **s < 1.0).count();
    assert!((400..600).contains(&below), "{below}");
    assert!(!tables(&result).is_empty());
}

const HEADERS: &str = "BEGIN DATA\n\
     1 AC\n\
     91\n\
     2 CC\n\
     35\n\
     43\n\
     0 XX\n\
     1 BA\n\
     34\n\
     3 BB\n\
     42\n\
     96\n\
     37\n\
     END DATA.\n";

#[test]
fn end_case_spreads_header_records_to_each_individual() {
    let (s, result) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /#RECS 1 HEAD1 HEAD2 3-4(A).\n\
         LEAVE HEAD1 HEAD2.\n\
         LOOP #I=1 TO #RECS.\n\
         DATA LIST /INDIV 1-2(1).\n\
         PRINT /#RECS HEAD1 HEAD2 INDIV.\n\
         END CASE.\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         {HEADERS}LIST."
    ));
    assert_eq!(
        printed(&result),
        [
            [removed]
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["HEAD1", "HEAD2", "INDIV"]);
    assert_eq!(strings(data, "HEAD1"), ["A", "C", "C", "B", "B", "B", "B"]);
    assert_eq!(strings(data, "HEAD2"), ["C", "C", "C", "A", "B", "B", "B"]);
    assert_eq!(whole(data, "INDIV"), [9.1, 3.5, 4.3, 3.4, 4.2, 9.6, 3.7]);
}

#[test]
fn without_end_case_each_run_of_the_program_is_one_case() {
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /#RECS 1 HEAD1 HEAD2 3-4(A).\n\
         LOOP #I=1 TO #RECS.\n\
         DATA LIST /INDIV 1-2(1).\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         {HEADERS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "HEAD1"), ["A", "C", "X", "B", "B"]);
    assert_eq!(strings(data, "HEAD2"), ["C", "C", "X", "A", "B"]);
    assert_eq!(
        numbers(data, "INDIV"),
        [Some(9.1), Some(4.3), None, Some(3.4), Some(3.7)]
    );
}

const PRICES: &str = "1878 1 1234\n1879 2 1301\n1880 3 1420\n1881 4 1530\n1882 1 1600\n";

#[test]
fn end_file_stops_before_the_case_that_runs_it() {
    let dir = folder("prices");
    std::fs::write(dir.join("prices.txt"), PRICES).unwrap();
    let (s, _) = ran(&format!(
        "CD '{}'.\n\
         *Select cases.\n\
         INPUT PROGRAM.\n\
         DATA LIST FILE='prices.txt' /YEAR 1-4 QUARTER 6 PRICE 8-12(2).\n\
         DO IF (YEAR GE 1881).   /*Stop reading before 1881\n\
         END FILE.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         LIST.",
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "YEAR"), [1878.0, 1879.0, 1880.0]);
    assert_eq!(whole(data, "PRICE"), [12.34, 13.01, 14.2]);

    // END CASE before END FILE keeps the case of 1881
    let (s, _) = ran(&format!(
        "CD '{}'.\n\
         INPUT PROGRAM.\n\
         [removed]
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "YEAR"), [1878.0, 1879.0, 1880.0, 1881.0]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_data_list_without_variables_skips_records() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["X"]);
    assert_eq!(whole(data, "X"), [1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn the_end_variable_lets_the_program_print_a_total() {
    let dir = folder("income");
    std::fs::write(
        dir.join("income.txt"),
        "Ann            12000\nBob             8500\nCy              3000\n",
    )
    .unwrap();
    let (s, result) = ran(&format!(
        "CD '{}'.\n\
         FILE HANDLE INCOME /NAME='income.txt'.\n\
         [removed]
        dir.display()
    ));
    assert_eq!(
        printed(&result),
        [
            "Name        Income",
            "Ann         12,000",
            "Bob          8,500",
            "Cy           3,000",
            "TOTAL      $23,500"
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["TINCOME", "NAME", "INCOME"]);
    assert_eq!(whole(data, "TINCOME"), [12000.0, 20500.0, 23500.0]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn end_variables_concatenate_three_files() {
    let dir = folder("three");
    std::fs::write(
        dir.join("one.txt"),
        "Ann                 34 F\nBob                 45 M\n",
    )
    .unwrap();
    std::fs::write(dir.join("two.txt"), "Cy                  29 M\n").unwrap();
    std::fs::write(
        dir.join("three.txt"),
        "Di                      51  F\nEd                      62  M\n",
    )
    .unwrap();
    let (s, _) = ran(&format!(
        "CD '{}'.\n\
         FILE HANDLE ONE /NAME='one.txt'.\n\
         FILE HANDLE TWO /NAME='two.txt'.\n\
         FILE HANDLE THREE /NAME='three.txt'.\n\
         [removed]
         LIST VARIABLES=NAME AGE SEX.",
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["NAME", "AGE", "SEX"]);
    assert_eq!(strings(data, "NAME"), ["Ann", "Bob", "Cy", "Di", "Ed"]);
    assert_eq!(whole(data, "AGE"), [34.0, 45.0, 29.0, 51.0, 62.0]);
    assert_eq!(strings(data, "SEX"), ["F", "M", "M", "F", "M"]);
    std::fs::remove_dir_all(dir).unwrap();
}

[removed]

#[test]
fn reread_reads_a_record_by_its_kind() {
    let (s, _) = ran(&format!(
        [removed]
         {CARS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["PARTNO", "KIND", "PRICE", "QUANTITY"]);
    assert_eq!(whole(data, "PARTNO"), [11.0; 5]);
    assert_eq!(
        strings(data, "KIND"),
        ["FORD", "FORD", "CHEVY", "CHEVY", "CHEVY"]
    );
    assert_eq!(whole(data, "PRICE"), [12.95, 3.95, 1.95, 2.95, 2.1]);
    assert_eq!(whole(data, "QUANTITY"), [100.0, 25.0, 5.0, 15.0, 20.0]);
    assert_eq!(data.variable("PRICE").unwrap().format(), "DOLLAR6.2");
}

#[test]
fn reread_takes_its_column_from_an_expression() {
    let (s, _) = ran(&format!(
        [removed]
         {}LIST.",
        CARS.replace("11       CHEVY 210 20", "12       CHEVY 210 20")
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["KIND", "PRICE", "QUANTITY"]);
    assert_eq!(whole(data, "PRICE"), [12.95, 3.95, 1.95, 2.95, 2.1]);
    assert_eq!(whole(data, "QUANTITY"), [100.0, 25.0, 5.0, 15.0, 20.0]);
}

#[test]
fn of_several_rereads_the_last_counts() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "CDIMAGE"),
        ["1    A    B", "2    C    D", "3    E    F"]
    );
    assert_eq!(strings(data, "INFO"), ["B", "D", "F"]);
}

#[test]
fn reread_reads_the_rest_of_a_fixed_record_as_a_list() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "A"),
        [
            "FIRST RECORD 1",
            "NUMBER 2",
            "THE THIRD",
            "#4",
            "FIFTH AND LAST"
        ]
    );
    // as printed the first record has its 1 in column 14, so column 15 starts with the 2
    assert_eq!(
        numbers(data, "X"),
        [Some(2.0), Some(4.0), Some(6.0), None, Some(9.0)]
    );
    assert_eq!(
        numbers(data, "Y"),
        [Some(3.0), Some(5.0), Some(7.0), None, Some(10.0)]
    );
    assert_eq!(
        numbers(data, "Z"),
        [Some(-1.0), None, Some(8.0), None, Some(11.0)]
    );
    // the missing value on record 2 and the three on record 4 are named
    let warned = warnings(&result);
    assert!(warned.iter().any(|w| w.contains("line 9")), "{warned:?}");
    assert!(warned.iter().any(|w| w.contains("line 11")), "{warned:?}");
}

const VEHICLES: &str = "BEGIN DATA\n\
     1001 02 02 FORD    T8PONTIAC C6\n\
     1002 04 01 CHEVY   C4\n\
     1003 02 03 CADILAC C8FORD    T6VW      C4\n\
     END DATA.\n";

#[test]
fn repeating_data_makes_a_case_per_vehicle() {
    let (s, _) = ran(&format!(
        [removed]
         {VEHICLES}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        names(data),
        ["SEQNUM", "NUMPERS", "NUMVEH", "MAKE", "MODEL", "NUMCYL"]
    );
    assert_eq!(whole(data, "SEQNUM"), [1.0, 1.0, 2.0, 3.0, 3.0, 3.0]);
    assert_eq!(whole(data, "NUMPERS"), [2.0, 2.0, 4.0, 2.0, 2.0, 2.0]);
    assert_eq!(whole(data, "NUMVEH"), [2.0, 2.0, 1.0, 3.0, 3.0, 3.0]);
    assert_eq!(
        strings(data, "MAKE"),
        ["FORD", "PONTIAC", "CHEVY", "CADILAC", "FORD", "VW"]
    );
    assert_eq!(strings(data, "MODEL"), ["T", "C", "C", "C", "T", "C"]);
    assert_eq!(whole(data, "NUMCYL"), [8.0, 6.0, 4.0, 8.0, 6.0, 4.0]);
}

#[test]
fn occurs_and_length_limit_what_is_read() {
    // one group per record however many there are
    let (s, _) = ran(&format!(
        [removed]
         {VEHICLES}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "MAKE"), ["FORD", "CHEVY", "CADILAC"]);
    // only MAKE of groups 10 columns long
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST / SEQNUM 2-4 NUMPERS 6-7 NUMVEH 9-10.\n\
         REPEATING DATA STARTS=12 /OCCURS=NUMVEH /LENGTH=10 /NOTABLE\n\
          /DATA=MAKE 1-8 (A).\n\
         END INPUT PROGRAM.\n\
         {VEHICLES}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["SEQNUM", "NUMPERS", "NUMVEH", "MAKE"]);
    assert_eq!(
        strings(data, "MAKE"),
        ["FORD", "PONTIAC", "CHEVY", "CADILAC", "FORD", "VW"]
    );
}

#[test]
fn repeating_groups_can_start_in_a_column_a_variable_gives() {
    [removed]
         1200 04 01 ...CHEVY   C4\n\
         END DATA.");
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "MAKE"), ["FORD", "PONTIAC", "CHEVY"]);
    assert_eq!(whole(data, "FIRST"), [12.0, 12.0, 15.0]);
}

#[test]
fn transformations_after_the_program_work_on_its_cases() {
    [removed]
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "PARENTID"), [1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 4.0]);
    assert_eq!(
        whole(data, "BIRTHDAY"),
        [1981.0, 1982.0, 1979.0, 1978.0, 1981.0, 1983.0, 1984.0]
    );
    assert_eq!(whole(data, "AGE"), [6.0, 5.0, 9.0, 10.0, 7.0, 5.0, 4.0]);
    assert_eq!(whole(data, "VACAGE"), [2.0, 2.0, 5.0, 3.0, 5.0, 3.0, 3.0]);
    assert_eq!(whole(data, "CHILD"), [1.0, 2.0, 1.0, 1.0, 2.0, 3.0, 1.0]);
}

/// The orders of the CONTINUED examples, as the manual lists them.
fn orders(data: &Dataset) {
    assert_eq!(
        names(data),
        ["ORDERID", "NITEMS", "ITEM", "QUANTITY", "PRICE"]
    );
    assert_eq!(
        whole(data, "ORDERID"),
        [
            10020.0, 10020.0, 10020.0, 10020.0, 10020.0, 10020.0, 10020.0, 20030.0, 20030.0,
            20030.0, 20030.0
        ]
    );
    assert_eq!(
        whole(data, "QUANTITY"),
        [
            1.0, 100.0, 64.0, 25.0, 5.0, 3.0, 14.0, 10.0, 75.0, 54.0, 15.0
        ]
    );
    assert_eq!(
        whole(data, "PRICE"),
        [
            25.99, 101.99, 61.29, 13.95, 56.75, 23.74, 11.46, 10.97, 105.95, 22.99, 75.0
        ]
    );
    assert_eq!(strings(data, "ITEM")[6], "07-350-95");
    assert_eq!(data.variable("PRICE").unwrap().format(), "DOLLAR10.2");
}

/// The items of the CONTINUED examples: item, quantity and price.
const ITEMS: [(&str, &str, &str); 11] = [
    [removed]
    ("04-569-38", "015", "75.00"),
];

/// Records holding the items of `ITEMS` in groups of 21 columns (the price right-aligned in
/// 7, then a blank), so many per record as `per_record` says: the first record starts with
/// the order and its count, the others with the order. As printed in the manual the prices
/// lack the blanks that align them.
fn order_records(per_record: &[&[usize]]) -> String {
    let mut text = String::new();
    for groups in per_record {
        let order = if groups[0] < 7 { "10020" } else { "20030" };
        let first = groups[0] == 0 || groups[0] == 7;
        text.push_str(order);
        text.push_str(if first {
            if order == "10020" { " 07 " } else { " 04 " }
        } else {
            " "
        });
        for &g in *groups {
            let (item, quantity, price) = ITEMS[g];
            text.push_str(&format!("{item} {quantity}{price:>7} "));
        }
        text.push('\n');
    }
    text
}

#[test]
fn continued_groups_go_on_the_next_records() {
    let program = |starts: &str, continued: &str, data: &str| {
        format!(
            [removed]
              /DATA=ITEM 1-9 (A) QUANTITY 11-13 PRICE (DOLLAR7.2,1X).\n\
             END INPUT PROGRAM.\n\
             BEGIN DATA\n{data}END DATA.\n\
             LIST."
        )
    };
    let records = order_records(&[&[0, 1, 2], &[3, 4, 5], &[6], &[7, 8, 9], &[10]]);
    let (s, _) = ran(&program("10", "7", &records));
    orders(s.dataset().unwrap());
    // two groups on the first record, three on the others
    let records = order_records(&[&[0, 1], &[2, 3, 4], &[5, 6], &[7, 8], &[9, 10]]);
    let (s, _) = ran(&program("10-55", "7", &records));
    orders(s.dataset().unwrap());
    // two groups on every record
    let records = order_records(&[&[0, 1], &[2, 3], &[4, 5], &[6], &[7, 8], &[9, 10]]);
    let (s, _) = ran(&program("10-55", "7-55", &records));
    orders(s.dataset().unwrap());
}

#[test]
fn id_checks_the_continuation_records() {
    let program = |data: &str| {
        format!(
            [removed]
             LIST."
        )
    };
    // as printed, the prices lack the blank before them that PRICE 15-20 needs
    let records = "10020 04 45-923-89 001  25.9923-899-56 100 101.99\n\
                   10020 63-780-32 025  13.9554-756-90 005  56.75\n\
                   20030 03 45-781-43 010  10.9789-236-54 075 105.95\n\
                   20030 32-569-38 015  75.00\n";
    let (s, _) = ran(&program(records));
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "ITEM"),
        [
            "45-923-89",
            "23-899-56",
            "63-780-32",
            "54-756-90",
            "45-781-43",
            "89-236-54",
            "32-569-38"
        ]
    );
    assert_eq!(
        whole(data, "PRICE"),
        [25.99, 101.99, 13.95, 56.75, 10.97, 105.95, 75.0]
    );
    let wrong = records.replace("10020 63", "10021 63");
    let message = fails(&program(&wrong));
    assert!(message.contains("not 10020"), "{message}");
}

[removed]

#[test]
fn a_mixed_file_reads_the_listed_record_types() {
    let (s, result) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2.\n\
         RECORD TYPE 23.\n\
         DATA LIST   /SEX 5 AGE 6-7 DOSAGE 8-10 RESULT 12.\n\
         END FILE TYPE.\n\
         {TREATMENT}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["RECID", "SEX", "AGE", "DOSAGE", "RESULT"]);
    // the printed records do not line up with the columns; these are what the columns hold
    assert_eq!(whole(data, "RECID"), [23.0]);
    assert_eq!(whole(data, "SEX"), [7.0]);
    assert_eq!(whole(data, "AGE"), [20.0]);
    assert_eq!(whole(data, "DOSAGE"), [75.0]);
    // a mixed file skips other types without a warning
    assert!(warnings(&result).is_empty(), "{:?}", warnings(&result));

    let (s, _) = ran(&format!(
        [removed]
         {TREATMENT}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        whole(data, "RECID"),
        [21.0, 22.0, 25.0, 24.0, 23.0, 21.0, 25.0]
    );
    assert_eq!(
        whole(data, "DOSAGE"),
        [10.0, 0.0, 0.0, 50.0, 75.0, 50.0, 5.0]
    );
    assert_eq!(whole(data, "AGE"), [50.0, 72.0, 5.0, 51.0, 20.0, 90.0, 4.0]);

    // OTHER reads every other type with its own columns
    let (s, _) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2 WILD=NOWARN.\n\
         [removed]
         {TREATMENT}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        whole(data, "RECID"),
        [21.0, 22.0, 25.0, 35.0, 24.0, 23.0, 21.0, 25.0, 30.0, 32.0]
    );
    assert_eq!(whole(data, "AGE")[3], 7.0);
}

#[test]
fn wild_warn_names_the_undefined_records() {
    let (_, result) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2 WILD=WARN.\n\
         RECORD TYPE 21,22,23,24,25.\n\
         DATA LIST /SEX 5.\n\
         END FILE TYPE.\n{TREATMENT}"
    ));
    let warned = warnings(&result);
    assert_eq!(warned.len(), 3, "{warned:?}");
    assert!(
        warned[0].contains("record type 35 is not defined: 35 167"),
        "{warned:?}"
    );
}

[removed]

#[test]
fn a_grouped_file_makes_one_case_per_student() {
    let (s, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#TEST 6 CASE=STUDENT 1-4.\n{GROUPED}{STUDENTS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["STUDENT", "ENGLISH", "READING", "MATH"]);
    assert_eq!(whole(data, "STUDENT"), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(strings(data, "ENGLISH"), ["B+", "A", "B-", "C"]);
    assert_eq!(whole(data, "READING"), [74.0, 100.0, 88.0, 94.0]);
    assert_eq!(whole(data, "MATH"), [83.0, 71.0, 81.0, 91.0]);
    assert!(warnings(&result).is_empty());
}

#[test]
fn a_grouped_file_with_missing_and_unordered_records() {
    let (s, result) = ran(&format!(
        [removed]
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "STUDENT"), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(strings(data, "ENGLISH"), ["B+", "A", "", "C"]);
    assert_eq!(
        numbers(data, "READING"),
        [Some(74.0), Some(100.0), None, Some(94.0)]
    );
    assert_eq!(
        numbers(data, "MATH"),
        [None, Some(71.0), Some(81.0), Some(91.0)]
    );
    assert!(warnings(&result).is_empty());

    let (s, result) = ran(&format!(
        [removed]
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "ENGLISH"), ["B+", "A", "", "C"]);
    assert_eq!(
        numbers(data, "READING"),
        [Some(74.0), Some(100.0), Some(81.0), Some(94.0)]
    );
    assert_eq!(numbers(data, "MATH"), [None, Some(71.0), None, Some(91.0)]);
    assert!(warnings(&result).is_empty());

    // the defaults warn about missing, duplicate and unordered records
    let (_, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#TEST 6 CASE=STUDENT 1-4.\n{GROUPED}\
         BEGIN DATA\n\
         0001 2 74\n\
         0001 1 B+\n\
         0001 1 B\n\
         0002 1 A\n\
         0002 9 A\n\
         END DATA.\n"
    ));
    let warned = warnings(&result).join("\n");
    assert!(warned.contains("out of the order"), "{warned}");
    assert!(warned.contains("a second record of this type"), "{warned}");
    assert!(
        warned.contains("case 1 has no record of type 3"),
        "{warned}"
    );
    assert!(
        warned.contains("case 2 has no record of type 2"),
        "{warned}"
    );
    assert!(warned.contains("record type 9 is not defined"), "{warned}");
}

[removed]

/// The records of the manual with the blanks the DATA LIST columns need: as printed, the
/// accident record is one column to the left and the others one to the right.
[removed]

#[test]
fn a_nested_file_makes_a_case_per_person() {
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=ACCID 1-4.\n{NESTED}{ACCIDENTS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "ACCID", "ACC_ID", "WEATHER", "STATE", "DATE", "STYLE", "MAKE", "OLD", "LICENSE",
            "INSURNCE", "PSNGR_NO", "AGE", "SEX", "INJURY", "SEAT", "COST"
        ]
    );
    assert_eq!(whole(data, "ACCID"), [1.0; 5]);
    assert_eq!(whole(data, "ACC_ID"), [322.0; 5]);
    assert_eq!(strings(data, "STATE"), ["IL"; 5]);
    assert_eq!(strings(data, "DATE"), ["3/13/88"; 5]);
    assert_eq!(whole(data, "STYLE"), [1.0, 2.0, 2.0, 2.0, 3.0]);
    assert_eq!(strings(data, "LICENSE"), ["MI", "IL", "IL", "IL", "IN"]);
    assert_eq!(whole(data, "PSNGR_NO"), [1.0, 1.0, 2.0, 3.0, 1.0]);
    assert_eq!(whole(data, "AGE"), [34.0, 22.0, 35.0, 59.0, 46.0]);
    assert_eq!(strings(data, "SEAT"), ["FR", "FR", "FR", "BK", "FR"]);
    assert_eq!(whole(data, "COST"), [3.0, 11.0, 5.0, 7.0, 0.0]);
}

#[test]
fn spread_no_gives_the_vehicle_to_its_first_person_only() {
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=#RECID 6 CASE=ACCID 1-4.\n{}{ACCIDENTS}",
        NESTED.replace("RECORD TYPE 2.", "RECORD TYPE 2 SPREAD=NO.")
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        numbers(data, "STYLE"),
        [Some(1.0), Some(2.0), None, None, Some(3.0)]
    );
    assert_eq!(strings(data, "LICENSE"), ["MI", "IL", "", "", "IN"]);
    assert_eq!(whole(data, "ACC_ID"), [322.0; 5]);
}

#[test]
fn duplicate_and_missing_records_of_a_nested_file() {
    let duplicated = ACCIDENTS.replace(
        "0001 2    2 16IL 322F",
        "0001 2    1 31IL 134M      /*   duplicate vehicle record\n0001 2    2 16IL 322F",
    );
    let (s, result) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=ACCID 1-4 DUPLICATE=WARN.\n{NESTED}{duplicated}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 5);
    assert_eq!(strings(data, "LICENSE"), ["MI", "IL", "IL", "IL", "IN"]);
    let warned = warnings(&result);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("0001 2    2 16IL 322F"), "{warned:?}");

    // CASE: the empty vehicle is a case of its own
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=ACCID 1-4 DUPLICATE=CASE.\n{NESTED}{duplicated}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 6);
    assert_eq!(
        strings(data, "LICENSE"),
        ["MI", "IL", "IL", "IL", "IL", "IN"]
    );
    assert_eq!(
        numbers(data, "AGE"),
        [
            Some(34.0),
            None,
            Some(22.0),
            Some(35.0),
            Some(59.0),
            Some(46.0)
        ]
    );

    // a person without a vehicle record
    let (s, result) = ran(&format!(
        [removed]
            ""
        )
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 5);
    assert_eq!(numbers(data, "STYLE")[0], None);
    assert_eq!(numbers(data, "AGE")[0], Some(34.0));
    let warned = warnings(&result);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("no record of type 2"), "{warned:?}");
}

#[test]
fn other_skip_makes_the_vehicle_the_lowest_level() {
    let (s, _) = ran(&format!(
        [removed]
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["ACCID", "WEATHER", "STYLE"]);
    assert_eq!(data.case_count(), 3);
    assert_eq!(whole(data, "WEATHER"), [1.0; 3]);
}

#[test]
fn repeating_data_reads_the_records_of_a_mixed_file() {
    let (s, _) = ran(
        [removed]
    );
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["MAKE", "MODEL", "NUMCYL"]);
    assert_eq!(strings(data, "MAKE"), ["CADILAC", "FORD", "VW"]);
    assert_eq!(strings(data, "MODEL"), ["C", "T", ""]);
    assert_eq!(numbers(data, "NUMCYL"), [Some(8.0), Some(6.0), None]);
}

#[test]
fn leave_keeps_a_running_total() {
    [removed]
    let (mut s, _) = ran(&format!("{source}LIST."));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "TotalVar1"), [1.0, 5.0, 12.0]);
    // a variable that existed before cannot be left
    let message = fails(&format!(
        "{source}*this will change the value of Var2 but LEAVE will fail.\n\
         COMPUTE Var2=Var2+Var2.\n\
         LEAVE Var2."
    ));
    assert!(message.contains("Var2 existed before"), "{message}");
    // LEAVE before the command that defines the variable
    let message = fails(&format!(
        "{source}LEAVE TotalVar3.\n\
         COMPUTE TotalVar3=TotalVar3+Var3."
    ));
    assert!(message.contains("TotalVar3"), "{message}");
    // a value set for some cases only is carried to the next ones
    run(
        &mut s,
        "IF (Var1 = 4) other = 10.\nLEAVE other.\nSTRING tag (A3).\nIF (Var1 > 1) tag = 'x'.\nLEAVE tag.\nEXECUTE.",
    );
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "other"), [0.0, 10.0, 10.0]);
    assert_eq!(strings(data, "tag"), ["", "x", "x"]);
}

#[test]
fn leave_with_lag_sums_per_group() {
    let (s, _) = ran("DATA LIST LIST /DEPT SALARY.\n\
         BEGIN DATA\n\
         2 100\n\
         1 50\n\
         2 30\n\
         1 20\n\
         1 5\n\
         END DATA.\n\
         [removed]
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "TSALARY"), [50.0, 70.0, 75.0, 100.0, 130.0]);
    assert!(
        data.variable("TSALARY")
            .unwrap()
            .format()
            .starts_with("DOLLAR8")
    );
}

#[test]
fn mistakes_in_input_programs_are_named() {
    let message = fails("INPUT PROGRAM.\nCOMPUTE x = 1.\nEND CASE.\nEND INPUT PROGRAM.");
    assert!(message.contains("would never end"), "{message}");
    let message = fails("INPUT PROGRAM.\nDATA LIST /x 1.\nSELECT IF x > 1.");
    assert!(message.contains("SELECT IF"), "{message}");
    let message = fails("END CASE.");
    assert!(
        message.contains("only allowed inside INPUT PROGRAM"),
        "{message}"
    );
    let message = fails("INPUT PROGRAM.\nDATA LIST /x 1.");
    assert!(
        message.contains("INPUT PROGRAM without END INPUT PROGRAM"),
        "{message}"
    );
    let message = fails("INPUT PROGRAM.\nDATA LIST /x 1.\nEND INPUT PROGRAM.\nLIST.");
    assert!(message.contains("no data yet"), "{message}");
    let message = fails(
        "FILE TYPE GROUPED RECORD=#t 6 CASE=id 1-4.\nRECORD TYPE OTHER SKIP.\nEND FILE TYPE.\nBEGIN DATA\n1\nEND DATA.",
    );
    assert!(message.contains("WILD=WARN"), "{message}");
    let message = fails(
        "FILE TYPE MIXED RECORD=r 1 (A).\nRECORD TYPE 1.\nDATA LIST /x 2.\nEND FILE TYPE.\nBEGIN DATA\nA1\nEND DATA.",
    );
    assert!(message.contains("in quotes"), "{message}");
}
